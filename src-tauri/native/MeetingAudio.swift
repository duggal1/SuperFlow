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

        let tapDescription = CATapDescription(stereoGlobalTapButExcludeProcesses: [])
        tapDescription.name = "SuperFlow Meeting Audio"
        tapDescription.isPrivate = true
        tapDescription.muteBehavior = .unmuted

        var newTapID = AudioObjectID(kAudioObjectUnknown)
        guard AudioHardwareCreateProcessTap(tapDescription, &newTapID) == noErr else {
            return false
        }
        tapID = newTapID

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
        sampleRate = readSampleRate(deviceID: aggregateID)

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

    private func readSampleRate(deviceID: AudioObjectID) -> Int {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyNominalSampleRate,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var value = Float64(48_000)
        var size = UInt32(MemoryLayout<Float64>.size)
        let status = AudioObjectGetPropertyData(
            deviceID,
            &address,
            0,
            nil,
            &size,
            &value
        )
        return status == noErr ? max(16_000, Int(value.rounded())) : 48_000
    }
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
