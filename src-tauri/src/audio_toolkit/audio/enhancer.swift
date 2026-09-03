import AVFoundation
import AudioToolbox
import Foundation

public typealias AppleVoiceSamplesCallback = @convention(c) (
    UnsafePointer<Float>?,
    Int32,
    UInt32,
    UnsafeMutableRawPointer?
) -> Void

private let targetSampleRate: Double = 48_000
private let tapBufferSize: AVAudioFrameCount = 480

private enum AppleVoiceError: Error, LocalizedError {
    case unsupportedOS
    case invalidInputFormat
    case voiceProcessingUnavailable
    case graphConfigurationFailed(String)
    case engineStartFailed(String)

    var errorDescription: String? {
        switch self {
        case .unsupportedOS:
            return "Apple Voice Processing requires macOS 10.15 or newer"
        case .invalidInputFormat:
            return "The active microphone returned an invalid audio format"
        case .voiceProcessingUnavailable:
            return "Apple Voice Processing could not be enabled for the active audio route"
        case .graphConfigurationFailed(let message):
            return "Failed to configure Apple voice-processing graph: \(message)"
        case .engineStartFailed(let message):
            return "Failed to start Apple voice-processing engine: \(message)"
        }
    }
}

private final class AppleVoiceEnhancer {
    private let queue = DispatchQueue(label: "audio.apple-voice-enhancer", qos: .userInteractive)

    private var engine: AVAudioEngine?
    private var equalizer: AVAudioUnitEQ?
    private var dynamics: AVAudioUnitEffect?
    private var configurationObserver: NSObjectProtocol?

    private var callback: AppleVoiceSamplesCallback?
    private var callbackContext: UnsafeMutableRawPointer?

    private var wantsToRun = false
    private var rebuilding = false
    private var lastErrorMessage = ""

    func start(
        callback: @escaping AppleVoiceSamplesCallback,
        context: UnsafeMutableRawPointer?
    ) -> Int32 {
        queue.sync {
            self.callback = callback
            self.callbackContext = context
            self.wantsToRun = true

            do {
                try self.startLocked()
                self.lastErrorMessage = ""
                return 0
            } catch {
                self.lastErrorMessage = error.localizedDescription
                self.wantsToRun = false
                self.stopLocked(clearCallback: false)
                return -1
            }
        }
    }

    func stop() {
        queue.sync {
            wantsToRun = false
            stopLocked(clearCallback: true)
        }
    }

    func lastError(into buffer: UnsafeMutablePointer<CChar>?, capacity: Int32) -> Int32 {
        queue.sync {
            let bytes = Array(lastErrorMessage.utf8CString)

            guard let buffer, capacity > 0 else {
                return Int32(bytes.count)
            }

            let writable = min(Int(capacity), bytes.count)
            guard writable > 0 else {
                return Int32(bytes.count)
            }

            for index in 0..<(writable - 1) {
                buffer[index] = bytes[index]
            }

            buffer[writable - 1] = 0
            return Int32(bytes.count)
        }
    }

    func isRunning() -> Bool {
        queue.sync {
            engine?.isRunning == true && wantsToRun
        }
    }

    private func startLocked() throws {
        if engine?.isRunning == true {
            return
        }

        guard #available(macOS 10.15, *) else {
            throw AppleVoiceError.unsupportedOS
        }

        stopLocked(clearCallback: false)

        let engine = AVAudioEngine()
        let input = engine.inputNode

        let hardwareFormat = input.inputFormat(forBus: 0)
        guard hardwareFormat.sampleRate > 0, hardwareFormat.channelCount > 0 else {
            throw AppleVoiceError.invalidInputFormat
        }

        do {
            try input.setVoiceProcessingEnabled(true)
        } catch {
            throw AppleVoiceError.graphConfigurationFailed(
                "setVoiceProcessingEnabled(true) failed: \(error.localizedDescription)"
            )
        }

        guard input.isVoiceProcessingEnabled else {
            throw AppleVoiceError.voiceProcessingUnavailable
        }

        input.isVoiceProcessingBypassed = false
        input.isVoiceProcessingAGCEnabled = true

        guard let processingFormat = AVAudioFormat(
            standardFormatWithSampleRate: targetSampleRate,
            channels: 1
        ) else {
            throw AppleVoiceError.graphConfigurationFailed(
                "Could not create 48 kHz mono processing format"
            )
        }

        let equalizer = AVAudioUnitEQ(numberOfBands: 2)
        configureEqualizer(equalizer)

        let dynamics = AVAudioUnitEffect(
            audioComponentDescription: AudioComponentDescription(
                componentType: kAudioUnitType_Effect,
                componentSubType: kAudioUnitSubType_DynamicsProcessor,
                componentManufacturer: kAudioUnitManufacturer_Apple,
                componentFlags: 0,
                componentFlagsMask: 0
            )
        )

        engine.attach(equalizer)
        engine.attach(dynamics)

        engine.connect(input, to: equalizer, format: processingFormat)
        engine.connect(equalizer, to: dynamics, format: processingFormat)
        engine.connect(dynamics, to: engine.mainMixerNode, format: processingFormat)

        engine.mainMixerNode.outputVolume = 0

        configureDynamics(dynamics)

        guard let callback = self.callback else {
            throw AppleVoiceError.graphConfigurationFailed(
                "No audio callback was registered"
            )
        }

        let context = self.callbackContext

        dynamics.installTap(
            onBus: 0,
            bufferSize: tapBufferSize,
            format: processingFormat
        ) { buffer, _ in
            guard buffer.frameLength > 0 else {
                return
            }

            guard let channelData = buffer.floatChannelData else {
                return
            }

            let samples = channelData[0]
            let count = Int32(buffer.frameLength)

            callback(samples, count, UInt32(targetSampleRate), context)
        }

        engine.prepare()

        do {
            try engine.start()
        } catch {
            dynamics.removeTap(onBus: 0)
            throw AppleVoiceError.engineStartFailed(error.localizedDescription)
        }

        self.engine = engine
        self.equalizer = equalizer
        self.dynamics = dynamics

        installConfigurationObserver(for: engine)
    }

    private func stopLocked(clearCallback: Bool) {
        if let observer = configurationObserver {
            NotificationCenter.default.removeObserver(observer)
            configurationObserver = nil
        }

        if let dynamics {
            dynamics.removeTap(onBus: 0)
        }

        engine?.stop()
        engine?.reset()

        engine = nil
        equalizer = nil
        dynamics = nil

        if clearCallback {
            callback = nil
            callbackContext = nil
        }
    }

    private func installConfigurationObserver(for engine: AVAudioEngine) {
        configurationObserver = NotificationCenter.default.addObserver(
            forName: .AVAudioEngineConfigurationChange,
            object: engine,
            queue: nil
        ) { [weak self] _ in
            guard let self else {
                return
            }

            self.queue.async {
                guard self.wantsToRun, !self.rebuilding else {
                    return
                }

                self.rebuilding = true
                self.stopLocked(clearCallback: false)

                self.queue.asyncAfter(deadline: .now() + .milliseconds(120)) {
                    defer {
                        self.rebuilding = false
                    }

                    guard self.wantsToRun else {
                        return
                    }

                    do {
                        try self.startLocked()
                        self.lastErrorMessage = ""
                    } catch {
                        self.lastErrorMessage = error.localizedDescription
                        self.stopLocked(clearCallback: false)
                    }
                }
            }
        }
    }

    private func configureEqualizer(_ equalizer: AVAudioUnitEQ) {
        let presence = equalizer.bands[0]
        presence.filterType = .parametric
        presence.frequency = 3_200
        presence.bandwidth = 1.0
        presence.gain = 1.25
        presence.bypass = false

        let air = equalizer.bands[1]
        air.filterType = .highShelf
        air.frequency = 6_500
        air.bandwidth = 0.7
        air.gain = 0.75
        air.bypass = false

        equalizer.globalGain = 0
        equalizer.bypass = false
    }

    private func configureDynamics(_ dynamics: AVAudioUnitEffect) {
        let unit = dynamics.audioUnit

        setParameter(
            unit,
            id: kDynamicsProcessorParam_Threshold,
            value: -26
        )
        setParameter(
            unit,
            id: kDynamicsProcessorParam_HeadRoom,
            value: 4
        )
        setParameter(
            unit,
            id: kDynamicsProcessorParam_ExpansionThreshold,
            value: -52
        )
        setParameter(
            unit,
            id: kDynamicsProcessorParam_ExpansionRatio,
            value: 1.35
        )
        setParameter(
            unit,
            id: kDynamicsProcessorParam_AttackTime,
            value: 0.004
        )
        setParameter(
            unit,
            id: kDynamicsProcessorParam_ReleaseTime,
            value: 0.12
        )

        setParameter(
            unit,
            id: kDynamicsProcessorParam_OverallGain,
            value: 4.0
        )
    }

    private func setParameter(
        _ unit: AudioUnit,
        id: AudioUnitParameterID,
        value: AudioUnitParameterValue
    ) {
        let status = AudioUnitSetParameter(
            unit,
            id,
            kAudioUnitScope_Global,
            0,
            value,
            0
        )

        if status != noErr {
            NSLog(
                "AppleVoiceEnhancer: AudioUnitSetParameter(%u) failed with OSStatus %d",
                id,
                status
            )
        }
    }
}

@_cdecl("apple_voice_enhancer_create")
public func apple_voice_enhancer_create() -> UnsafeMutableRawPointer? {
    let enhancer = AppleVoiceEnhancer()
    return Unmanaged.passRetained(enhancer).toOpaque()
}

@_cdecl("apple_voice_enhancer_destroy")
public func apple_voice_enhancer_destroy(
    _ handle: UnsafeMutableRawPointer?
) {
    guard let handle else {
        return
    }

    let enhancer = Unmanaged<AppleVoiceEnhancer>
        .fromOpaque(handle)
        .takeRetainedValue()

    enhancer.stop()
}

@_cdecl("apple_voice_enhancer_start")
public func apple_voice_enhancer_start(
    _ handle: UnsafeMutableRawPointer?,
    _ callback: AppleVoiceSamplesCallback?,
    _ context: UnsafeMutableRawPointer?
) -> Int32 {
    guard let handle, let callback else {
        return -1
    }

    let enhancer = Unmanaged<AppleVoiceEnhancer>
        .fromOpaque(handle)
        .takeUnretainedValue()

    return enhancer.start(
        callback: callback,
        context: context
    )
}

@_cdecl("apple_voice_enhancer_stop")
public func apple_voice_enhancer_stop(
    _ handle: UnsafeMutableRawPointer?
) {
    guard let handle else {
        return
    }

    let enhancer = Unmanaged<AppleVoiceEnhancer>
        .fromOpaque(handle)
        .takeUnretainedValue()

    enhancer.stop()
}

@_cdecl("apple_voice_enhancer_is_running")
public func apple_voice_enhancer_is_running(
    _ handle: UnsafeMutableRawPointer?
) -> Int32 {
    guard let handle else {
        return 0
    }

    let enhancer = Unmanaged<AppleVoiceEnhancer>
        .fromOpaque(handle)
        .takeUnretainedValue()

    return enhancer.isRunning() ? 1 : 0
}

@_cdecl("apple_voice_enhancer_last_error")
public func apple_voice_enhancer_last_error(
    _ handle: UnsafeMutableRawPointer?,
    _ buffer: UnsafeMutablePointer<CChar>?,
    _ capacity: Int32
) -> Int32 {
    guard let handle else {
        return 0
    }

    let enhancer = Unmanaged<AppleVoiceEnhancer>
        .fromOpaque(handle)
        .takeUnretainedValue()

    return enhancer.lastError(
        into: buffer,
        capacity: capacity
    )
}

@_cdecl("apple_voice_enhancer_active_microphone_mode")
public func apple_voice_enhancer_active_microphone_mode() -> Int32 {
    if #available(macOS 12.0, *) {
        return Int32(AVCaptureDevice.activeMicrophoneMode.rawValue)
    }

    return -1
}

@_cdecl("apple_voice_enhancer_preferred_microphone_mode")
public func apple_voice_enhancer_preferred_microphone_mode() -> Int32 {
    if #available(macOS 12.0, *) {
        return Int32(AVCaptureDevice.preferredMicrophoneMode.rawValue)
    }

    return -1
}

@_cdecl("apple_voice_enhancer_show_microphone_modes")
public func apple_voice_enhancer_show_microphone_modes() {
    guard #available(macOS 12.0, *) else {
        return
    }

    DispatchQueue.main.async {
        AVCaptureDevice.showSystemUserInterface(.microphoneModes)
    }
}
