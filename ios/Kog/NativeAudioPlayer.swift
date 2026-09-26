import AVFoundation
import Foundation

#if KOG_NATIVE_AUDIO
@_silgen_name("kog_audio_open")
private func decoderOpen(_ path: UnsafePointer<CChar>, _ subsong: Int32,
                         _ error: UnsafeMutablePointer<CChar>, _ capacity: Int) -> UnsafeMutableRawPointer?
@_silgen_name("kog_audio_duration_ms")
private func decoderDuration(_ handle: UnsafeRawPointer) -> Int64
@_silgen_name("kog_audio_read")
private func decoderRead(_ handle: UnsafeMutableRawPointer, _ output: UnsafeMutablePointer<UInt8>,
                         _ capacity: Int, _ error: UnsafeMutablePointer<CChar>, _ errorCapacity: Int) -> Int
@_silgen_name("kog_audio_seek")
private func decoderSeek(_ handle: UnsafeMutableRawPointer, _ milliseconds: UInt64,
                         _ error: UnsafeMutablePointer<CChar>, _ errorCapacity: Int) -> Bool
@_silgen_name("kog_audio_close")
private func decoderClose(_ handle: UnsafeMutableRawPointer)

/// Opening some emulators takes time. This owns the Rust decoder while it
/// crosses from a worker to the main actor before the audio engine is set up.
final class NativeAudioSource: @unchecked Sendable {
    private var handle: UnsafeMutableRawPointer?
    let duration: Double

    init(path: String, subsong: Int32 = -1) throws {
        var message = [CChar](repeating: 0, count: 1024)
        let opened = path.withCString { decoderOpen($0, subsong, &message, message.count) }
        guard let opened else { throw KogError.response(String(cString: message)) }
        handle = opened
        let milliseconds = decoderDuration(opened)
        duration = milliseconds < 0 ? 0 : Double(milliseconds) / 1000
    }

    func takeHandle() -> UnsafeMutableRawPointer? {
        defer { handle = nil }
        return handle
    }

    deinit { if let handle { decoderClose(handle) } }
}

/// Kog's pull decoder connected to Core Audio. All decoder calls share one queue.
/// The output is the same 48 kHz stereo mix used by the server and Android.
final class NativeAudioPlayer {
    private let queue = DispatchQueue(label: "org.kog.player.decoder")
    private let engine = AVAudioEngine()
    private let node = AVAudioPlayerNode()
    private let format = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48_000,
                                       channels: 2, interleaved: true)!
    private var handle: UnsafeMutableRawPointer?
    private var generation = 0
    private var outstanding = 0
    private var ended = false
    private var playing = false
    private var offset = 0.0
    private let onEnd: () -> Void
    private let onError: (String) -> Void
    let duration: Double

    static func useFor(_ track: Track) -> Bool { track.isDevice }

    init(source: NativeAudioSource, onEnd: @escaping () -> Void, onError: @escaping (String) -> Void) throws {
        self.onEnd = onEnd; self.onError = onError
        guard let opened = source.takeHandle() else {
            throw KogError.response("Audio source has already been opened")
        }
        handle = opened
        duration = source.duration
        engine.attach(node)
        engine.connect(node, to: engine.mainMixerNode, format: format)
        engine.prepare()
        do { try engine.start() }
        catch { decoderClose(opened); handle = nil; throw error }
    }

    deinit {
        // The last strong reference may be released by a decoder queue task;
        // dispatching synchronously back to that queue would deadlock teardown.
        node.stop()
        engine.stop()
        if let handle { decoderClose(handle) }
    }

    func play() {
        queue.async { [weak self] in
            guard let self else { return }
            self.playing = true
            if self.outstanding == 0 && !self.ended {
                for _ in 0..<4 { self.schedule() }
            }
            self.node.play()
        }
    }

    func pause() { queue.async { [weak self] in self?.node.pause(); self?.playing = false } }

    func seek(_ seconds: Double) {
        queue.async { [weak self] in
            guard let self, let handle = self.handle else { return }
            let resume = self.playing
            self.generation += 1
            self.node.stop()
            self.outstanding = 0; self.ended = false
            var message = [CChar](repeating: 0, count: 1024)
            let milliseconds = UInt64(max(0, seconds) * 1000)
            guard decoderSeek(handle, milliseconds, &message, message.count) else {
                self.onError(String(cString: message)); return
            }
            self.offset = Double(milliseconds) / 1000
            if resume {
                for _ in 0..<4 { self.schedule() }
                self.node.play()
            }
        }
    }

    func stop() {
        queue.async { [weak self] in
            guard let self else { return }
            self.generation += 1
            self.node.stop()
            self.outstanding = 0
            self.playing = false
        }
    }

    var position: Double {
        queue.sync {
            guard let render = node.lastRenderTime,
                  let played = node.playerTime(forNodeTime: render) else { return offset }
            return offset + Double(played.sampleTime) / played.sampleRate
        }
    }

    private func schedule() {
        guard let handle, !ended else { return }
        let capacity = AVAudioFrameCount(4096)
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: capacity),
              let output = buffer.audioBufferList.pointee.mBuffers.mData else {
            onError("Could not allocate an audio buffer"); return
        }
        var message = [CChar](repeating: 0, count: 1024)
        let bytes = decoderRead(handle, output.assumingMemoryBound(to: UInt8.self), Int(capacity) * 8,
                                &message, message.count)
        if bytes < 0 { ended = true; onError(String(cString: message)); return }
        if bytes == 0 { ended = true; if outstanding == 0 { onEnd() }; return }
        guard bytes % 8 == 0 else { ended = true; onError("Decoder returned a partial stereo frame"); return }
        buffer.frameLength = AVAudioFrameCount(bytes / 8)
        outstanding += 1
        let scheduledGeneration = generation
        node.scheduleBuffer(buffer, completionCallbackType: .dataPlayedBack) { [weak self] _ in
            self?.queue.async { [weak self] in
                guard let self, self.generation == scheduledGeneration else { return }
                self.outstanding -= 1
                if self.ended {
                    if self.outstanding == 0 { self.playing = false; self.onEnd() }
                } else if self.playing { self.schedule() }
            }
        }
    }
}
#endif
