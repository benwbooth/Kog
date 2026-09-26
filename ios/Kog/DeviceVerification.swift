#if KOG_DEVICE_TESTS && KOG_NATIVE_AUDIO
import AVFoundation
import Foundation

/// Explicit development-build diagnostic. Never runs in a normal launch and
/// uses an isolated cache library, leaving the user's queue and playlists alone.
@MainActor enum DeviceVerification {
    static func runIfRequested(_ store: KogStore) async {
        guard ProcessInfo.processInfo.arguments.contains("--verify-kog-device") else { return }
        let root = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("Kog Verification")
        let report = root.appendingPathComponent("result.json")
        try? FileManager.default.removeItem(at: report)
        var checks: [String: Any] = [:]
        do {
            try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
            let music = root.appendingPathComponent("music")
            try FileManager.default.createDirectory(at: music, withIntermediateDirectories: true)
            let song = music.appendingPathComponent("tone.wav")
            try wave().write(to: song)
            let api = KogAPI(server: "", token: "", username: "", password: "", codec: "aac", deviceRoot: music.path, deviceStorage: root.appendingPathComponent("state").path)
            let listing = try await api.browse()
            guard let file = listing.files.first else { throw KogError.response("Offline browse did not find the fixture") }
            let tracks = try await api.expand(file)
            guard let track = tracks.first, track.isDevice, track.duration > 1000 else { throw KogError.response("Offline metadata expansion failed") }
            checks["browse_expand_metadata"] = true
            let id = try await api.createPlaylist("Verification \(UUID().uuidString)")
            try await api.appendPlaylist(id, tracks: tracks)
            let saved = try await api.playlist(id)
            guard saved.first?.id == track.id else { throw KogError.response("Saved playlist changed track identity") }
            try await api.star(track, enabled: true)
            guard try await api.stars().contains(track.id) else { throw KogError.response("Offline favorite did not persist") }
            try await api.star(track, enabled: false)
            try await api.replacePlaylist(id, tracks: [])
            guard try await api.playlist(id).isEmpty else { throw KogError.response("Removing final playlist row failed") }
            try await api.deletePlaylist(id)
            checks["offline_playlists_favorites"] = true
            var page = try await api.search("tone", root: music.path)
            var hits = page.results.count
            while !page.done {
                try await Task.sleep(for: .milliseconds(100))
                page = try await api.more(page.generation, offset: hits); hits += page.results.count
            }
            guard hits == 1 else { throw KogError.response("Offline search returned \(hits) results") }
            checks["offline_search"] = true
            let radio = try await api.radio(true, root: music.path)
            guard !radio.isEmpty, radio.allSatisfy(\.isDevice) else { throw KogError.response("Offline radio produced no local tracks") }
            _ = try await api.radio(false, root: music.path)
            checks["offline_radio"] = true
            let source = try await Task.detached {
                try NativeAudioSource(path: song.path, midiEngine: "opl3windows", soundfontPath: "", sc55RomPath: "", mt32RomPath: "")
            }.value
            let visualization = AudioVisualization()
            var audioFailure: String?
            let player = try NativeAudioPlayer(source: source, visualization: visualization, onEnd: {}, onError: { audioFailure = $0 })
            player.volume = 0
            player.play()
            try await Task.sleep(for: .milliseconds(700))
            guard player.position > 0.2, audioFailure == nil else { throw KogError.response(audioFailure ?? "Core Audio clock did not advance") }
            // The mixer tap can contain silence while muted; it must still receive real PCM buffers.
            guard !visualization.snapshot().samples.isEmpty else { throw KogError.response("No live PCM reached the waveform tap") }
            checks["native_playback_muted_pcm_tap"] = true
            player.seek(2)
            try await Task.sleep(for: .milliseconds(350))
            guard player.position >= 2 else { throw KogError.response("Local seek failed") }
            player.pause()
            try await Task.sleep(for: .milliseconds(100))
            let paused = player.position
            try await Task.sleep(for: .milliseconds(200))
            guard abs(player.position - paused) < 0.05 else { throw KogError.response("Local pause did not hold position") }
            player.play()
            try await Task.sleep(for: .milliseconds(250))
            guard player.position > paused else { throw KogError.response("Local resume failed") }
            player.stop()
            checks["native_seek_pause_resume"] = true
            if let remote = store.current, !remote.isDevice, !store.server.isEmpty {
                try await store.api.health()
                let seekTo = min(10.0, Double(remote.duration) / 4_000)
                let url = try store.api.nativeStream(remote, start: seekTo).absoluteString
                let headers = store.api.audioHeaders
                let source = try await Task.detached {
                    try NativeAudioSource(stream: url, headers: headers, durationMilliseconds: max(0, remote.duration - Int64(seekTo * 1000)))
                }.value
                let remoteVisualization = AudioVisualization()
                let stream = try NativeAudioPlayer(source: source, visualization: remoteVisualization, onEnd: {}, onError: { _ in })
                stream.volume = 0; stream.play()
                for _ in 0..<120 {
                    if stream.position > 0.3 { break }
                    try await Task.sleep(for: .milliseconds(250))
                }
                defer { stream.stop() }
                guard stream.position > 0.3 else { throw KogError.response("Server stream did not start within 30 seconds") }
                guard !remoteVisualization.snapshot().samples.isEmpty else { throw KogError.response("No PCM from the server stream") }
                checks["server_stream_core_audio"] = true
                checks["server_stream_pcm_tap"] = true
                checks["server_stream_start_seconds"] = seekTo
            }
            checks["passed"] = true
        } catch { checks["passed"] = false; checks["error"] = error.localizedDescription }
        checks["device"] = ProcessInfo.processInfo.operatingSystemVersionString
        checks["verifiedAt"] = ISO8601DateFormatter().string(from: Date())
        if let data = try? JSONSerialization.data(withJSONObject: checks, options: [.prettyPrinted, .sortedKeys]) { try? data.write(to: report, options: .atomic) }
    }
    private static func wave() -> Data {
        let frames = 48_000 * 5
        var data = Data()
        func text(_ value: String) { data.append(contentsOf: value.utf8) }
        func word<T: FixedWidthInteger>(_ value: T) { var le = value.littleEndian; withUnsafeBytes(of: &le) { data.append(contentsOf: $0) } }
        text("RIFF"); word(UInt32(36 + frames * 4)); text("WAVEfmt "); word(UInt32(16)); word(UInt16(1)); word(UInt16(2))
        word(UInt32(48_000)); word(UInt32(192_000)); word(UInt16(4)); word(UInt16(16)); text("data"); word(UInt32(frames * 4))
        for i in 0..<frames { let value = Int16(sin(Double(i) * 2 * .pi * 440 / 48_000) * 8192); word(value); word(value) }
        return data
    }
}
#endif
