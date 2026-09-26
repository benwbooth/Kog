import AVFoundation
import MediaToolbox
import SwiftUI

/// The audio callbacks only copy a bounded window. Drawing and spectrum work
/// run on the UI clock, and a busy UI never blocks the audio render thread.
final class AudioVisualization: @unchecked Sendable {
    private let lock = NSLock()
    private var samples = [Float](repeating: 0, count: 1024)
    private var sampleCount = 0
    private var updated = Date.distantPast
    private var sampleRate = 48_000.0
    private var processingFormat = AudioStreamBasicDescription()

    func reset() { lock.lock(); sampleCount = 0; lock.unlock() }
    func accept(_ buffers: UnsafePointer<AudioBufferList>, frames: Int, format: AudioStreamBasicDescription) {
        guard frames > 0, format.mFormatID == kAudioFormatLinearPCM,
              lock.try() else { return }
        defer { lock.unlock() }
        let list = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: buffers))
        guard let buffer = list.first, let data = buffer.mData else { return }
        let channels = max(1, Int(buffer.mNumberChannels))
        let count = min(frames, 1024)
        if format.mFormatFlags & kAudioFormatFlagIsFloat != 0 && format.mBitsPerChannel == 32 {
            let source = data.assumingMemoryBound(to: Float.self)
            for i in 0..<count { samples[i] = source[i * channels] }
        } else if format.mFormatFlags & kAudioFormatFlagIsSignedInteger != 0 && format.mBitsPerChannel == 16 {
            let source = data.assumingMemoryBound(to: Int16.self)
            for i in 0..<count { samples[i] = Float(source[i * channels]) / 32768 }
        } else { return }
        sampleCount = count; sampleRate = format.mSampleRate; updated = Date()
    }
    func snapshot() -> (samples: [Float], rate: Double) {
        lock.lock(); defer { lock.unlock() }
        return (Date().timeIntervalSince(updated) < 0.4 ? Array(samples.prefix(sampleCount)) : [], sampleRate)
    }
    @MainActor func attach(to item: AVPlayerItem) async {
        guard let track = try? await item.asset.loadTracks(withMediaType: .audio).first, !Task.isCancelled else { return }
        let retained = Unmanaged.passRetained(self).toOpaque()
        var callbacks = MTAudioProcessingTapCallbacks(version: kMTAudioProcessingTapCallbacksVersion_0,
            clientInfo: retained,
            init: { _, client, storage in storage.pointee = client },
            finalize: { tap in Unmanaged<AudioVisualization>.fromOpaque(MTAudioProcessingTapGetStorage(tap)).release() },
            prepare: { tap, _, format in
                Unmanaged<AudioVisualization>.fromOpaque(MTAudioProcessingTapGetStorage(tap)).takeUnretainedValue().processingFormat = format.pointee
            }, unprepare: nil,
            process: { tap, count, _, buffers, provided, flags in
                guard MTAudioProcessingTapGetSourceAudio(tap, count, buffers, flags, nil, provided) == noErr else { return }
                let visualization = Unmanaged<AudioVisualization>.fromOpaque(MTAudioProcessingTapGetStorage(tap)).takeUnretainedValue()
                visualization.accept(buffers, frames: provided.pointee, format: visualization.processingFormat)
            })
        var tap: MTAudioProcessingTap?
        guard MTAudioProcessingTapCreate(kCFAllocatorDefault, &callbacks, kMTAudioProcessingTapCreationFlag_PostEffects, &tap) == noErr,
              let tap else { Unmanaged<AudioVisualization>.fromOpaque(retained).release(); return }
        let parameters = AVMutableAudioMixInputParameters(track: track)
        parameters.audioTapProcessor = tap
        let mix = AVMutableAudioMix(); mix.inputParameters = [parameters]
        item.audioMix = mix
    }
}

struct WaveformView: View {
    let audio: AudioVisualization
    var spectrum = false
    var playing = true
    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30, paused: !playing)) { _ in
            Canvas { context, size in
                let snapshot = audio.snapshot()
                let values = playing ? snapshot.samples : []
                guard values.count > 1 else { return }
                if spectrum {
                    let n = min(values.count, 512)
                    for bar in 0..<36 {
                        let frequency = 40.0 * pow(400.0, Double(bar) / 35)
                        guard frequency < snapshot.rate / 2 else { continue }
                        let omega = 2 * Double.pi * frequency / snapshot.rate
                        var real = 0.0, imaginary = 0.0
                        for i in 0..<n {
                            let window = 0.5 - 0.5 * cos(2 * Double.pi * Double(i) / Double(n - 1))
                            let value = Double(values[i]) * window
                            real += value * cos(omega * Double(i)); imaginary -= value * sin(omega * Double(i))
                        }
                        let amplitude = sqrt(real * real + imaginary * imaginary) * 4 / Double(n)
                        let height = max(0, min(1, (20 * log10(max(amplitude, 0.00001)) + 60) / 60)) * size.height
                        let width = size.width / 36
                        context.fill(Path(CGRect(x: Double(bar) * width, y: size.height - height, width: max(1, width - 2), height: height)), with: .color(Palette.accent))
                    }
                } else {
                    let columns = max(2, min(Int(size.width), 256))
                    var path = Path()
                    for column in 0..<columns {
                        let start = column * values.count / columns
                        let end = min(values.count, max(start + 1, (column + 1) * values.count / columns))
                        let section = values[start..<end]
                        let low = Double(section.min() ?? 0), high = Double(section.max() ?? 0)
                        let x = Double(column) * size.width / Double(columns - 1)
                        path.move(to: CGPoint(x: x, y: size.height * (0.5 - min(1, high * 1.5) * 0.46)))
                        path.addLine(to: CGPoint(x: x, y: size.height * (0.5 - max(-1, low * 1.5) * 0.46)))
                    }
                    context.stroke(path, with: .color(Palette.accent), lineWidth: 1.5)
                }
            }
        }.accessibilityLabel(spectrum ? "Live audio spectrum" : "Live audio waveform")
    }
}
