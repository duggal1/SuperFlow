import Foundation
import CoreAudio

@_silgen_name("meeting_system_audio_samples")
private func meetingSystemAudioSamples(
    _ samples: UnsafePointer<Float>,
    _ sampleCount: Int,
    _ channelCount: Int,
    _ sampleRate: Int
)

@available(macOS 14.2, *)
final class MeetingSystemAudioCapturer {
    private let queue = DispatchQueue(label: "com.superflow.meeting.system-audio")
    private var tapID = AudioObjectID(kAudioObjectUnknown)
    private var aggregateID = AudioObjectID(kAudioObjectUnknown)
    private var ioProcID: AudioDeviceIOProcID?
    private var sampleRate = 48_000
    private(set) var isCapturing = false

    func start() -> Bool {
        guard !isCapturing else { return true }

        // A process tap observes the system mix without becoming an output
        // device, muting playback, or forcing the current output route to
        // change. Mono is exactly what STT consumes and avoids treating planar
        // stereo buffers as sequential audio.
        let tapDescription = CATapDescription(monoGlobalTapButExcludeProcesses: [])
        tapDescription.name = "SuperFlow Meeting Audio"
        tapDescription.isPrivate = true
        tapDescription.muteBehavior = .unmuted

        var newTapID = AudioObjectID(kAudioObjectUnknown)
        guard AudioHardwareCreateProcessTap(tapDescription, &newTapID) == noErr else {
            return false
        }
        tapID = newTapID
        sampleRate = readTapSampleRate(tapID: tapID)

        let aggregateUID = "com.superflow.meeting.\(UUID().uuidString)"
        let aggregateDescription: [String: Any] = [
            kAudioAggregateDeviceNameKey: "SuperFlow Meeting Capture",
            kAudioAggregateDeviceUIDKey: aggregateUID,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceTapAutoStartKey: true,
            kAudioAggregateDeviceTapListKey: [
                [kAudioSubTapUIDKey: tapDescription.uuid.uuidString]
            ],
        ]

        var newAggregateID = AudioObjectID(kAudioObjectUnknown)
        guard AudioHardwareCreateAggregateDevice(
            aggregateDescription as CFDictionary,
            &newAggregateID
        ) == noErr else {
            AudioHardwareDestroyProcessTap(tapID)
            tapID = AudioObjectID(kAudioObjectUnknown)
            return false
        }
        aggregateID = newAggregateID

        var newIOProcID: AudioDeviceIOProcID?
        let status = AudioDeviceCreateIOProcIDWithBlock(
            &newIOProcID,
            aggregateID,
            queue
        ) { [weak self] _, inputData, _, _, _ in
            guard let self else { return }
            let mutableInputData = UnsafeMutablePointer(mutating: inputData)
            for buffer in UnsafeMutableAudioBufferListPointer(mutableInputData) {
                guard let data = buffer.mData else { continue }
                let count = Int(buffer.mDataByteSize) / MemoryLayout<Float>.size
                guard count > 0 else { continue }
                meetingSystemAudioSamples(
                    data.assumingMemoryBound(to: Float.self),
                    count,
                    max(1, Int(buffer.mNumberChannels)),
                    self.sampleRate
                )
            }
        }
        guard status == noErr, let newIOProcID else {
            cleanup()
            return false
        }
        ioProcID = newIOProcID

        guard AudioDeviceStart(aggregateID, ioProcID) == noErr else {
            cleanup()
            return false
        }
        isCapturing = true
        return true
    }

    func stop() -> Bool {
        guard isCapturing else {
            cleanup()
            return false
        }
        if aggregateID != AudioObjectID(kAudioObjectUnknown) {
            AudioDeviceStop(aggregateID, ioProcID)
        }
        isCapturing = false
        cleanup()
        return true
    }

    private func cleanup() {
        if let ioProcID, aggregateID != AudioObjectID(kAudioObjectUnknown) {
            AudioDeviceDestroyIOProcID(aggregateID, ioProcID)
        }
        ioProcID = nil
        if aggregateID != AudioObjectID(kAudioObjectUnknown) {
            AudioHardwareDestroyAggregateDevice(aggregateID)
            aggregateID = AudioObjectID(kAudioObjectUnknown)
        }
        if tapID != AudioObjectID(kAudioObjectUnknown) {
            AudioHardwareDestroyProcessTap(tapID)
            tapID = AudioObjectID(kAudioObjectUnknown)
        }
    }

    private func readTapSampleRate(tapID: AudioObjectID) -> Int {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioTapPropertyFormat,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var format = AudioStreamBasicDescription()
        var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
        let status = AudioObjectGetPropertyData(
            tapID,
            &address,
            0,
            nil,
            &size,
            &format
        )
        guard status == noErr,
              format.mFormatID == kAudioFormatLinearPCM,
              format.mFormatFlags & kAudioFormatFlagIsFloat != 0,
              format.mBitsPerChannel == 32,
              format.mSampleRate >= 16_000 else {
            return 48_000
        }
        return Int(format.mSampleRate.rounded())
    }
}

private func audioDeviceIDs() -> [AudioObjectID] {
    var address = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(
        AudioObjectID(kAudioObjectSystemObject),
        &address,
        0,
        nil,
        &size
    ) == noErr else { return [] }

    let count = Int(size) / MemoryLayout<AudioObjectID>.size
    guard count > 0 else { return [] }
    var devices = [AudioObjectID](repeating: AudioObjectID(kAudioObjectUnknown), count: count)
    guard AudioObjectGetPropertyData(
        AudioObjectID(kAudioObjectSystemObject),
        &address,
        0,
        nil,
        &size,
        &devices
    ) == noErr else { return [] }
    return devices
}

private func defaultInputDeviceID() -> AudioObjectID? {
    var address = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    var deviceID = AudioObjectID(kAudioObjectUnknown)
    var size = UInt32(MemoryLayout<AudioObjectID>.size)
    guard AudioObjectGetPropertyData(
        AudioObjectID(kAudioObjectSystemObject),
        &address,
        0,
        nil,
        &size,
        &deviceID
    ) == noErr, deviceID != AudioObjectID(kAudioObjectUnknown) else { return nil }
    return deviceID
}

private func deviceName(_ deviceID: AudioObjectID) -> String? {
    var address = AudioObjectPropertyAddress(
        mSelector: kAudioObjectPropertyName,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    var value: CFString = "" as CFString
    var size = UInt32(MemoryLayout<CFString>.size)
    guard AudioObjectGetPropertyData(
        deviceID,
        &address,
        0,
        nil,
        &size,
        &value
    ) == noErr else { return nil }
    return value as String
}

private func deviceTransport(_ deviceID: AudioObjectID) -> UInt32? {
    var address = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyTransportType,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    var transport: UInt32 = 0
    var size = UInt32(MemoryLayout<UInt32>.size)
    guard AudioObjectGetPropertyData(
        deviceID,
        &address,
        0,
        nil,
        &size,
        &transport
    ) == noErr else { return nil }
    return transport
}

private func hasInputStreams(_ deviceID: AudioObjectID) -> Bool {
    var address = AudioObjectPropertyAddress(
        mSelector: kAudioDevicePropertyStreams,
        mScope: kAudioDevicePropertyScopeInput,
        mElement: kAudioObjectPropertyElementMain
    )
    var size: UInt32 = 0
    return AudioObjectGetPropertyDataSize(deviceID, &address, 0, nil, &size) == noErr
        && size >= UInt32(MemoryLayout<AudioStreamID>.size)
}

private func isBluetoothTransport(_ transport: UInt32?) -> Bool {
    transport == kAudioDeviceTransportTypeBluetooth
        || transport == kAudioDeviceTransportTypeBluetoothLE
}

private func safeInputRank(_ transport: UInt32?) -> Int {
    switch transport {
    case kAudioDeviceTransportTypeBuiltIn: return 0
    case kAudioDeviceTransportTypeUSB: return 1
    case kAudioDeviceTransportTypeThunderbolt: return 2
    case kAudioDeviceTransportTypeContinuityCaptureWired: return 3
    default: return 10
    }
}

/// Returns a non-Bluetooth input only when the requested (or system-default)
/// microphone is Bluetooth. Classic Bluetooth cannot provide headset-mic input
/// and high-fidelity playback simultaneously; selecting the built-in/USB mic
/// preserves the user's earbuds as an A2DP output while still capturing voice.
@_cdecl("superflow_safe_input_device_name")
public func superflowSafeInputDeviceName(
    _ requestedName: UnsafePointer<CChar>?,
    _ output: UnsafeMutablePointer<CChar>?,
    _ outputCapacity: Int
) -> Bool {
    guard let output, outputCapacity > 1 else { return false }
    let devices = audioDeviceIDs().filter(hasInputStreams)
    let requestedDevice: AudioObjectID?
    if let requestedName {
        let requested = String(cString: requestedName)
        requestedDevice = devices.first {
            deviceName($0)?.caseInsensitiveCompare(requested) == .orderedSame
        }
    } else {
        requestedDevice = defaultInputDeviceID()
    }

    guard let requestedDevice,
          isBluetoothTransport(deviceTransport(requestedDevice)) else { return false }
    let safeDevice = devices
        .filter { !isBluetoothTransport(deviceTransport($0)) }
        .min { safeInputRank(deviceTransport($0)) < safeInputRank(deviceTransport($1)) }
    guard let safeDevice, let safeName = deviceName(safeDevice) else { return false }

    let utf8 = safeName.utf8CString
    guard utf8.count <= outputCapacity else { return false }
    utf8.withUnsafeBufferPointer { buffer in
        guard let source = buffer.baseAddress else { return }
        output.update(from: source, count: buffer.count)
    }
    return true
}

@available(macOS 14.2, *)
private let sharedCapturer = MeetingSystemAudioCapturer()

@_cdecl("meeting_system_audio_start")
public func meetingSystemAudioStart() -> Bool {
    if #available(macOS 14.2, *) {
        return sharedCapturer.start()
    }
    return false
}

@_cdecl("meeting_system_audio_stop")
public func meetingSystemAudioStop() -> Bool {
    if #available(macOS 14.2, *) {
        return sharedCapturer.stop()
    }
    return false
}

@_cdecl("meeting_system_audio_is_capturing")
public func meetingSystemAudioIsCapturing() -> Bool {
    if #available(macOS 14.2, *) {
        return sharedCapturer.isCapturing
    }
    return false
}
