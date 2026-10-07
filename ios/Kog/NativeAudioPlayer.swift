import AVFoundation
import Foundation

#if KOG_NATIVE_AUDIO
@_silgen_name("kog_audio_open_reader")
private func decoderOpenReader(
    _ read: @convention(c) (UnsafeMutableRawPointer?, UnsafeMutablePointer<UInt8>?, Int32) -> Int32,
    _ close: @convention(c) (UnsafeMutableRawPointer?) -> Void,
    _ context: UnsafeMutableRawPointer, _ duration: UInt64,
    _ error: UnsafeMutablePointer<CChar>, _ capacity: Int) -> UnsafeMutableRawPointer?
@_silgen_name("kog_audio_open")
private func decoderOpen(_ path: UnsafePointer<CChar>, _ subsong: Int32,
                         _ midiEngine: UnsafePointer<CChar>, _ soundfontPath: UnsafePointer<CChar>,
                         _ sc55RomPath: UnsafePointer<CChar>, _ mt32RomPath: UnsafePointer<CChar>,
                         _ error: UnsafeMutablePointer<CChar>, _ capacity: Int) -> UnsafeMutableRawPointer?
@_silgen_name("kog_audio_duration_ms")
private func decoderDuration(_ handle: UnsafeRawPointer) -> Int64
@_silgen_name("kog_audio_read")
private func decoderRead(_ handle: UnsafeMutableRawPointer, _ output: UnsafeMutablePointer<UInt8>,
                         _ capacity: Int, _ error: UnsafeMutablePointer<CChar>, _ errorCapacity: Int) -> Int
@_silgen_name("kog_audio_seek")
private func decoderSeek(_ handle: UnsafeMutableRawPointer, _ milliseconds: UInt64,
                         _ error: UnsafeMutablePointer<CChar>, _ errorCapacity: Int) -> Bool
@_silgen_name("kog_audio_channel_snapshot")
private func decoderChannels(_ handle: UnsafeRawPointer, _ milliseconds: UInt64, _ playing: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("kog_audio_mml")
private func decoderMml(_ handle: UnsafeRawPointer, _ have: Int64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("kog_audio_string_free")
private func decoderFreeString(_ string: UnsafeMutablePointer<CChar>)
@_silgen_name("kog_audio_close")
private func decoderClose(_ handle: UnsafeMutableRawPointer)

/// Opening some emulators takes time. This owns the Rust decoder while it
/// crosses from a worker to the main actor before the audio engine is set up.
final class NativeAudioSource: @unchecked Sendable {
    private var handle: UnsafeMutableRawPointer?
    private(set) var transport: HTTPAudioStream?
    let duration: Double

    init(path: String, subsong: Int32 = -1, midiEngine: String,
         soundfontPath: String, sc55RomPath: String, mt32RomPath: String) throws {
        var message = [CChar](repeating: 0, count: 1024)
        let opened = path.withCString { pathPointer in
            midiEngine.withCString { enginePointer in
                soundfontPath.withCString { soundfontPointer in
                    sc55RomPath.withCString { sc55Pointer in
                        mt32RomPath.withCString { mt32Pointer in
                            decoderOpen(pathPointer, subsong, enginePointer, soundfontPointer,
                                        sc55Pointer, mt32Pointer, &message, message.count)
                        }
                    }
                }
            }
        }
        guard let opened else { throw KogError.response(String(cString: message)) }
        handle = opened
        let milliseconds = decoderDuration(opened)
        duration = milliseconds < 0 ? 0 : Double(milliseconds) / 1000
    }

    init(stream: String, headers: String, durationMilliseconds: Int64) throws {
        guard let url = URL(string: stream) else { throw KogError.invalidServer }
        let input = HTTPAudioStream(url: url, headers: headers)
        var message = [CChar](repeating: 0, count: 1024)
        let opened = decoderOpenReader({ context, output, capacity in
            guard let context, let output else { return -1 }
            return Unmanaged<HTTPAudioStream>.fromOpaque(context).takeUnretainedValue().read(output, capacity: Int(capacity))
        }, { context in
            guard let context else { return }
            let stream = Unmanaged<HTTPAudioStream>.fromOpaque(context).takeRetainedValue()
            stream.cancel()
        }, Unmanaged.passRetained(input).toOpaque(), UInt64(max(0, durationMilliseconds)), &message, message.count)
        guard let opened else { throw KogError.response(input.errorMessage ?? String(cString: message)) }
        transport = input
        handle = opened
        duration = Double(max(0, decoderDuration(opened))) / 1000
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
                                       channels: 2, interleaved: false)!
    // Rust emits interleaved stereo; AVAudioPlayerNode requires separate
    // Float32 channel planes. Keep one decode buffer on the serial queue.
    private var decodeBuffer = [Float](repeating: 0, count: 4096 * 2)
    private var handle: UnsafeMutableRawPointer?
    private let transport: HTTPAudioStream?
    private var generation = 0
    private var outstanding = 0
    private var ended = false
    private var playing = false
    private var offset = 0.0
    private var pendingSeek: Double?
    private var desiredPlaying = false
    private let clockLock = NSLock()
    private let onEnd: () -> Void
    private let onError: (String) -> Void
    let duration: Double

    static func useFor(_ track: Track) -> Bool { true }

    init(source: NativeAudioSource, visualization: AudioVisualization, onEnd: @escaping () -> Void, onError: @escaping (String) -> Void) throws {
        self.onEnd = onEnd; self.onError = onError
        guard let opened = source.takeHandle() else {
            throw KogError.response("Audio source has already been opened")
        }
        handle = opened
        transport = source.transport
        duration = source.duration
        engine.attach(node)
        engine.connect(node, to: engine.mainMixerNode, format: format)
        engine.mainMixerNode.installTap(onBus: 0, bufferSize: 1024, format: nil) { buffer, _ in
            visualization.accept(buffer.audioBufferList, frames: Int(buffer.frameLength), format: buffer.format.streamDescription.pointee)
        }
        engine.prepare()
        do { try engine.start() }
        catch { decoderClose(opened); handle = nil; throw error }
    }

    deinit {
        // The last strong reference may be released by a decoder queue task;
        // dispatching synchronously back to that queue would deadlock teardown.
        transport?.cancel()
        node.stop()
        engine.stop()
        if let handle { decoderClose(handle) }
    }

    func play() {
        clockLock.lock(); desiredPlaying = true; clockLock.unlock()
        queue.async { [weak self] in
            guard let self, self.wantsPlayback else { return }
            self.playing = true
            if self.outstanding == 0 && !self.ended {
                for _ in 0..<4 { self.schedule() }
            }
            if self.wantsPlayback { self.node.play() }
        }
    }

    private var wantsPlayback: Bool {
        clockLock.lock(); defer { clockLock.unlock() }; return desiredPlaying
    }

    func pause() {
        clockLock.lock(); desiredPlaying = false; clockLock.unlock()
        node.pause(); queue.async { [weak self] in self?.node.pause(); self?.playing = false }
    }

    func seek(_ seconds: Double) {
        clockLock.lock(); pendingSeek = max(0, seconds); clockLock.unlock()
        queue.async { [weak self] in
            guard let self, let handle = self.handle else { return }
            let resume = self.wantsPlayback
            self.generation += 1
            self.node.stop()
            self.outstanding = 0; self.ended = false
            var message = [CChar](repeating: 0, count: 1024)
            let milliseconds = UInt64(max(0, seconds) * 1000)
            guard decoderSeek(handle, milliseconds, &message, message.count) else {
                self.clockLock.lock(); self.pendingSeek = nil; self.clockLock.unlock()
                self.onError(String(cString: message)); return
            }
            self.clockLock.lock()
            self.offset = Double(milliseconds) / 1000; self.pendingSeek = nil
            self.clockLock.unlock()
            if resume {
                for _ in 0..<4 { self.schedule() }
                if self.wantsPlayback { self.node.play() }
            }
        }
    }

    func stop() {
        clockLock.lock(); desiredPlaying = false; clockLock.unlock()
        transport?.cancel()
        node.stop()
        queue.async { [weak self] in
            guard let self else { return }
            self.generation += 1
            self.node.stop()
            self.outstanding = 0
            self.playing = false
        }
    }

    var volume: Float {
        get { node.volume }
        set { node.volume = newValue }
    }

    var position: Double {
        clockLock.lock(); let offset = offset, pending = pendingSeek; clockLock.unlock()
        if let pending { return pending }
        guard let render = node.lastRenderTime, let played = node.playerTime(forNodeTime: render) else { return offset }
        return offset + Double(played.sampleTime) / played.sampleRate
    }

    func channelSnapshot(playing: Bool) async -> ChannelSnapshot {
        clockLock.lock(); let pending = pendingSeek; clockLock.unlock()
        if let pending { return ChannelSnapshot(position: pending, playing: playing, seeking: true) }
        let time = position
        return await withCheckedContinuation { continuation in
            queue.async { [weak self] in
                guard let self, let handle = self.handle,
                      let json = decoderChannels(handle, UInt64(max(0, time) * 1000), playing) else {
                    continuation.resume(returning: ChannelSnapshot()); return
                }
                let data = Data(String(cString: json).utf8)
                decoderFreeString(json)
                continuation.resume(returning: (try? ChannelSnapshot.decode(data)) ?? ChannelSnapshot())
            }
        }
    }

    /// The local track's MML score; nil for a stream.
    func mmlScore(have: Int64) async -> MmlScoreReply? {
        await withCheckedContinuation { continuation in
            queue.async { [weak self] in
                guard let self, let handle = self.handle, let json = decoderMml(handle, have) else {
                    continuation.resume(returning: nil); return
                }
                let data = Data(String(cString: json).utf8)
                decoderFreeString(json)
                continuation.resume(returning: try? JSONDecoder().decode(MmlScoreReply.self, from: data))
            }
        }
    }

    private func schedule() {
        guard let handle, !ended, wantsPlayback else { return }
        let capacity = AVAudioFrameCount(4096)
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: capacity),
              let channels = buffer.floatChannelData else {
            onError("Could not allocate an audio buffer"); return
        }
        var message = [CChar](repeating: 0, count: 1024)
        let bytes = decodeBuffer.withUnsafeMutableBytes { output in
            decoderRead(handle, output.baseAddress!.assumingMemoryBound(to: UInt8.self), output.count,
                        &message, message.count)
        }
        if bytes < 0 { ended = true; onError(String(cString: message)); return }
        if bytes == 0 { ended = true; if outstanding == 0 { onEnd() }; return }
        guard bytes % 8 == 0 else { ended = true; onError("Decoder returned a partial stereo frame"); return }
        let frames = bytes / 8
        for frame in 0..<frames {
            channels[0][frame] = decodeBuffer[frame * 2]
            channels[1][frame] = decodeBuffer[frame * 2 + 1]
        }
        buffer.frameLength = AVAudioFrameCount(frames)
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
