import AVFoundation
import Combine
import MediaPlayer
import SwiftUI
import UniformTypeIdentifiers
import UserNotifications

@MainActor
final class KogStore: ObservableObject {
    @Published var server = KogPreferences.standard.string(forKey: "server") ?? ""
    @Published var username = KogPreferences.standard.string(forKey: "username") ?? ""
    @Published var token = Secrets.read("token")
    @Published var password = Secrets.read("password")
    @Published var codec = KogPreferences.standard.string(forKey: "codec") ?? "aac"
    @Published var midiEngine = KogPreferences.standard.string(forKey: "midi_engine") ?? "opl3windows"
    @Published var localMidiEngine = KogPreferences.standard.string(forKey: "local_midi_engine") ?? "opl3windows"
    @Published var soundfontPath = KogPreferences.standard.string(forKey: "midi_soundfont") ?? ""
    @Published var sc55RomPath = KogPreferences.standard.string(forKey: "midi_sc55_roms") ?? ""
    @Published var mt32RomPath = KogPreferences.standard.string(forKey: "midi_mt32_roms") ?? ""
    @Published var connected = false
    @Published var listing: Listing?
    @Published var libraryRoot = ""
    @Published private(set) var treeRoot = ""
    @Published var searchText = ""
    @Published var searchFolders = [Folder]()
    @Published var searchTracks = [Track]()
    @Published var searchScanned = 0
    @Published var searching = false
    @Published private(set) var queue = [Track]()
    @Published private(set) var currentIndex = -1
    @Published var playing = false
    @Published private(set) var channelInspectionActive = false
    @Published var position = 0.0
    @Published var duration = 0.0
    @Published private(set) var shuffle = ShuffleMode(rawValue: KogPreferences.standard.string(forKey: "shuffle_mode") ?? "") ?? (KogPreferences.standard.bool(forKey: "shuffle") ? .all : .off)
    @Published private(set) var repeatMode = RepeatMode(rawValue: KogPreferences.standard.string(forKey: "repeat_mode") ?? "") ?? (KogPreferences.standard.bool(forKey: "repeat") ? .all : .off) {
        didSet { KogPreferences.standard.set(repeatMode.rawValue, forKey: "repeat_mode") }
    }
    @Published var volume = KogPreferences.standard.object(forKey: "player_volume") as? Double ?? 1.0 {
        didSet { if !applyingSession { sessionCommand(["op": "volume", "value": volume]) } }
    }
    @Published var muted = false { didSet { applyVolume() } }
    @Published var libraryOnDevice = false { didSet { search("") } }
    @Published var playlistOnDevice = (KogPreferences.standard.string(forKey: "server") ?? "").isEmpty { didSet { selectedPlaylist = nil; Task { await loadPlaylists() } } }
    @Published var localStars = Set<String>()
    @Published var searchPaused = false
    @Published var radioBusy = false
    @Published var notifyTracks = KogPreferences.standard.bool(forKey: "track_notifications")
    @Published var queueFilter = "" { didSet { if !applyingSession { sessionCommand(["op": "filter", "query": queueFilter]) } } }
    @Published var sortKey = "title"
    @Published var sortDescending = false
    @Published var deviceTreeRoot = KogPreferences.standard.string(forKey: "device_tree_root") ?? ""
    private var radioOnDevice = false
    private var interruptedPlayback = false
    private var audioObservers = [NSObjectProtocol]()
    private var exportURL: URL?
    @Published var radio = false
    @Published var stars = Set<String>()
    @Published var playlists = [SavedPlaylist]()
    @Published var selectedPlaylist: SavedPlaylist?
    @Published var playlistTracks = [Track]()
    @Published var deviceFiles = [Track]()
    @Published var deviceListing: Listing?
    @Published var devicePath = ""
    @Published var error: String?
    @Published var importing = false
    @Published var pendingAdds = 0
    @Published var downloading = Set<String>()
    @Published var downloadNotice: String?

    let visualization = AudioVisualization()
    private var visualizationTask: Task<Void, Never>?
    private var player: AVPlayer?
    private var assetLoader: AuthenticatedAssetLoader?
    private var inspectionStreamURL: String?
    private var inspectionStreamOffset = 0.0
    #if KOG_NATIVE_AUDIO
    private var nativePlayer: NativeAudioPlayer?
    private var nativeTimer: Timer?
    private var nativeStartTask: Task<Void, Never>?
    private var nativeGeneration = 0
    #endif
    var channelInspectionPosition: Double {
        #if KOG_NATIVE_AUDIO
        if let decoder = nativePlayer { return inspectionStreamOffset + decoder.position }
        #endif
        return position
    }
    var channelInspectionStream: String? { inspectionStreamURL }
    func localChannelSnapshot() async -> ChannelSnapshot {
        #if KOG_NATIVE_AUDIO
        if let decoder = nativePlayer { return await decoder.channelSnapshot(playing: playing) }
        #endif
        return ChannelSnapshot()
    }
    private var timeObserver: Any?
    private var finishObserver: NSObjectProtocol?
    private var statusObserver: NSKeyValueObservation?
    private var searchTask: Task<Void, Never>?
    private var importScanTask: Task<Void, Never>?
    private let session: SharedBackendSession
    private var applyingSession = false
    private var outputToken: [String: Any]?
    private var sessionStorageKey: String { "backend_session.\(session.id)" }
    private var storageRevision: Int64?
    private var lastSavedCheckpoint: Data?
    @Published private(set) var queueSelection = Set<Int>()
    @Published private(set) var workspace = PlaylistWorkspaceSnapshot.empty
    @Published private(set) var queuedIndices = [Int]()
    @Published private(set) var stopAfterIndices = Set<Int>()
    private var downloadNoticeTask: Task<Void, Never>?
    private var nowPlayingArtTask: Task<Void, Never>?
    private var nowPlayingArtwork: MPMediaItemArtwork?

    var current: Track? { queue.indices.contains(currentIndex) ? queue[currentIndex] : nil }
    var deviceAPI: KogAPI {
        KogAPI(server: "", token: "", username: "", password: "", codec: codec,
               deviceRoot: importsURL.path,
               deviceStorage: FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Kog").path, sessionID: session.id)
    }
    var playlistAPI: KogAPI { playlistOnDevice ? deviceAPI : api }
    var searchAPI: KogAPI { libraryOnDevice ? deviceAPI : api }
    var activeDeviceRoot: String { deviceTreeRoot.isEmpty ? importsURL.path : importsURL.appendingPathComponent(deviceTreeRoot).path }
    var activeTreeRoot: String { treeRoot.isEmpty ? libraryRoot : treeRoot }
    private var serverKey: String { (try? api.url("/").absoluteString) ?? server }
    var soundfontReady: Bool { !soundfontPath.isEmpty && FileManager.default.fileExists(atPath: soundfontPath) }
    var sc55RomsReady: Bool { !sc55RomPath.isEmpty && FileManager.default.fileExists(atPath: sc55RomPath) }
    var mt32RomsReady: Bool { !mt32RomPath.isEmpty && FileManager.default.fileExists(atPath: mt32RomPath) }
    var api: KogAPI { KogAPI(server: server, token: token, username: username,
                            password: password, codec: codec, midiEngine: midiEngine, sessionID: session.id) }
    var importsURL: URL {
        let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        return documents.appendingPathComponent("Kog Imports", isDirectory: true)
    }

    init(sessionID: String? = nil) {
        let defaults = KogPreferences.standard
        let identifier = sessionID ?? defaults.getOrCreateString("backend_session_id", default: "ios:\(UUID().uuidString)")
        session = SharedBackendSession(id: identifier)
        soundfontPath = rebaseDevicePath(soundfontPath)
        sc55RomPath = rebaseDevicePath(sc55RomPath)
        mt32RomPath = rebaseDevicePath(mt32RomPath)
        do {
            let stored = try deviceAPI.localState("sessions", id: session.id)
            storageRevision = (stored["revision"] as? NSNumber)?.int64Value
            var checkpoint = stored["value"] as? [String: Any]
            if storageRevision == nil || (storageRevision != 0 && checkpoint == nil) { throw KogError.response("Invalid saved session; the original has been preserved") }
            if checkpoint == nil, storageRevision == 0, let data = UserDefaults.standard.data(forKey: sessionStorageKey) {
                checkpoint = try JSONSerialization.jsonObject(with: data) as? [String: Any]
            }
            if var saved = checkpoint {
                let tracks = try workspaceTracks(saved["queue"]).map(rebaseDeviceTrack)
                saved["queue"] = try trackValues(tracks)
                if let workspace = saved["workspace"] { saved["workspace"] = rebaseWorkspace(workspace) }
                applySessionReply(try session.send(restore: saved))
            } else {
                // Import the old frontend format once; it is never authoritative again.
                let legacyShuffle = shuffle, legacyRepeat = repeatMode, legacyVolume = volume
                let legacy = sessionID == nil ? UserDefaults.standard.data(forKey: "queue") : nil
                let tracks = try legacy.map { try JSONDecoder().decode([Track].self, from: $0) }.map { $0.map(rebaseDeviceTrack) } ?? []
                sessionCommand(["op": "replace", "tracks": try trackValues(tracks), "current": SharedPlaybackPolicy.index(tracks.isEmpty ? -1 : defaults.integer(forKey: "index"))])
                sessionCommand(["op": "shuffle", "mode": legacyShuffle.rawValue])
                sessionCommand(["op": "repeat", "mode": legacyRepeat.rawValue])
                sessionCommand(["op": "volume", "value": legacyVolume])
                if sessionID == nil, let data = UserDefaults.standard.data(forKey: "playlist_workspace"), let value = try? JSONSerialization.jsonObject(with: data) {
                    sessionCommand(["op": "workspace_restore", "value": rebaseWorkspace(value)])
                }
            }
        } catch { storageRevision = nil; report(error) }
        updateSessionScopes()
        if let checkpoint = try? session.send()["checkpoint"] { persistCheckpoint(checkpoint) }
        if let saved = try? session.send()["checkpoint"] as? [String: Any], radio {
            radioOnDevice = saved["radio_scope"] as? String == "device"
            sessionCommand(["op": "radio", "enabled": true, "scope": saved["radio_scope"] ?? "device", "root": saved["radio_root"] ?? ""])
        }
        if let failure = KogPreferences.standard.lastError { self.error = failure }
        scanImports()
        Task { if let saved = try? await deviceAPI.stars() { localStars = saved; updateSessionMetadata() } }
        do {
            try AVAudioSession.sharedInstance().setCategory(.playback, mode: .default)
            try AVAudioSession.sharedInstance().setActive(true)
        } catch { self.error = "Audio session: \(error.localizedDescription)" }
        registerRemoteCommands()
        observeAudioSession()
        if !server.isEmpty { Task { await refresh() } }
    }

    func saveSettings() {
        server = server.trimmingCharacters(in: .whitespacesAndNewlines).trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        KogPreferences.standard.set(server, forKey: "server")
        KogPreferences.standard.set(username, forKey: "username")
        KogPreferences.standard.set(codec, forKey: "codec")
        Secrets.write("token", token)
        Secrets.write("password", password)
        updateSessionScopes()
        Task { await refresh() }
    }

    private func isMidi(_ track: Track) -> Bool {
        let name = track.entry.isEmpty ? track.path : track.entry
        return ["kar", "mid", "midi", "rmi", "mids", "mds", "lds", "xmf", "mxmf"]
            .contains(URL(fileURLWithPath: name).pathExtension.lowercased())
    }

    private func restartCurrentMidi() {
        guard let track = current, isMidi(track) else { return }
        sessionCommand(["op": "reload_output"])
    }

    func selectMidiEngine(_ engine: String) {
        midiEngine = engine
        KogPreferences.standard.set(engine, forKey: "midi_engine")
        if connected {
            Task {
                do {
                    try await api.setMidiEngine(engine)
                    if current?.isDevice == false { restartCurrentMidi() }
                } catch { report(error) }
            }
        }
    }

    func selectLocalMidiEngine(_ engine: String) {
        guard ["opl3windows", "rustysynth-sf2", "nuked-sc55", "munt-mt32"].contains(engine) else { return }
        guard engine != localMidiEngine else { return }
        localMidiEngine = engine
        KogPreferences.standard.set(engine, forKey: "local_midi_engine")
        if current?.isDevice == true { restartCurrentMidi() }
    }

    func importMidiAsset(_ source: URL, kind: String) async {
        guard ["soundfont", "sc55", "mt32"].contains(kind) else { return }
        if kind == "soundfont" && source.pathExtension.lowercased() != "sf2" {
            error = "Choose an .sf2 SoundFont file"
            return
        }
        let destination = importsURL.deletingLastPathComponent()
            .appendingPathComponent("Kog MIDI", isDirectory: true)
            .appendingPathComponent(kind == "soundfont" ? "soundfont.sf2" : kind,
                                    isDirectory: kind != "soundfont")
        do {
            try await Task.detached(priority: .userInitiated) {
                let access = source.startAccessingSecurityScopedResource()
                defer { if access { source.stopAccessingSecurityScopedResource() } }
                let manager = FileManager.default
                try manager.createDirectory(at: destination.deletingLastPathComponent(),
                                            withIntermediateDirectories: true)
                let temporary = destination.deletingLastPathComponent()
                    .appendingPathComponent(".\(kind)-\(UUID().uuidString)")
                defer { try? manager.removeItem(at: temporary) }
                try manager.copyItem(at: source, to: temporary)
                if manager.fileExists(atPath: destination.path) { try manager.removeItem(at: destination) }
                try manager.moveItem(at: temporary, to: destination)
            }.value
            switch kind {
            case "soundfont": soundfontPath = destination.path
            case "sc55": sc55RomPath = destination.path
            default: mt32RomPath = destination.path
            }
            KogPreferences.standard.set(destination.path, forKey: "midi_\(kind == "soundfont" ? "soundfont" : kind + "_roms")")
            if current?.isDevice == true { restartCurrentMidi() }
        } catch { report(error) }
    }

    private func report(_ failure: Error) {
        if let failure = failure as? URLError, failure.code != .cancelled {
            connected = false
        }
        error = failure.localizedDescription
    }

    // Health is checked while the app is active; a successful browse from earlier
    // must not leave the connection indicator green after the server exits.
    func checkConnection() async {
        guard !server.isEmpty else { connected = false; return }
        let client = api
        let reachable = (try? await client.connection()) != nil
        guard client.server == server, !Task.isCancelled else { return }
        connected = reachable
    }

    private func rememberTreeRoot() {
        KogPreferences.standard.set(treeRoot, forKey: "server_tree_root.\(serverKey)")
    }

    @discardableResult
    func setTreeRoot(_ path: String) async -> Bool {
        do {
            guard !path.hasPrefix("kog-archive:") else {
                throw KogError.response("Choose a folder as the library tree root.")
            }
            let client = api
            let directory = try await client.browse(path)
            guard !directory.isArchive else { throw KogError.response("Choose a folder rather than an archive.") }
            guard client.server == server else { return false }
            treeRoot = directory.path == libraryRoot ? "" : directory.path
            rememberTreeRoot()
            if radio && !radioOnDevice { prepareRadio() }
            search("")
            listing = directory
            connected = true
            return true
        } catch { report(error); return false }
    }

    func refresh() async {
        guard !server.isEmpty else { connected = false; return }
        error = nil
        do {
            let client = api
            try await client.health()
            let listing = try await client.browse()
            guard client.server == server else { return }
            self.listing = listing
            libraryRoot = listing.path
            treeRoot = KogPreferences.standard.treeRoot(for: serverKey)
            if !treeRoot.isEmpty {
                do { self.listing = try await client.browse(treeRoot) }
                catch let failure as KogError {
                    treeRoot = ""; rememberTreeRoot()
                    error = "The saved tree root is unavailable: \(failure.localizedDescription)"
                }
            }
            search("")
            playlists = try await client.playlists()
            stars = try await client.stars()
            updateSessionMetadata()
            if let serverEngine = try? await client.serverMidiEngine(), serverEngine != midiEngine {
                midiEngine = serverEngine
                KogPreferences.standard.set(serverEngine, forKey: "midi_engine")
                if current?.isDevice == false { restartCurrentMidi() }
            }
            connected = true
        } catch { connected = false; report(error) }
    }

    func browse(_ path: String) async {
        do {
            listing = try await api.browse(path)
            search("")
            connected = true
        } catch { report(error) }
    }

    func search(_ text: String) {
        searchText = text
        searchTask?.cancel()
        searchPaused = false
        if text.isEmpty { searchTracks = []; searchFolders = []; searching = false; return }
        searchTask = Task {
            do {
                try await Task.sleep(for: .milliseconds(250))
                searching = true
                let client = searchAPI
                let device = libraryOnDevice
                var page = try await client.search(text, root: device ? activeDeviceRoot : activeTreeRoot)
                guard !Task.isCancelled else { return }
                searchFolders = page.results.filter { $0.is_dir == true }.map(\.folder)
                searchTracks = page.results.filter { $0.is_dir != true }.map { device ? $0.track.onDevice() : $0.track }
                searchScanned = page.scanned
                while !page.done && !Task.isCancelled {
                    try await Task.sleep(for: .milliseconds(180))
                    page = try await client.more(page.generation, offset: searchFolders.count + searchTracks.count)
                    guard !Task.isCancelled else { break }
                    searchFolders += page.results.filter { $0.is_dir == true }.map(\.folder)
                    searchTracks += page.results.filter { $0.is_dir != true }.map { device ? $0.track.onDevice() : $0.track }
                    searchScanned = page.scanned
                }
            } catch is CancellationError {} catch { report(error) }
            if searchText == text { searching = false }
        }
    }

    private func trackValues(_ tracks: [Track]) throws -> Any { try JSONSerialization.jsonObject(with: JSONEncoder().encode(tracks)) }
    func addFile(_ track: Track, play: Bool = false) async {
        do { sessionCommand(["op": "expand", "scope": track.isDevice ? "device" : "server:\(serverKey)", "entries": try trackValues([track]), "action": play ? "play_now" : "add_to_queue"]) }
        catch { report(error) }
    }
    func addDeviceFolder(_ folder: Folder) async {
        sessionCommand(["op": "collect", "scope": "device", "path": folder.path, "query": searchText, "root": activeDeviceRoot])
    }
    func addFolder(_ folder: Folder, play: Bool = false) async {
        sessionCommand(["op": "collect", "scope": "server:\(serverKey)", "path": folder.path, "query": searchText, "root": activeTreeRoot, "action": play ? "play_now" : "add_to_queue"])
    }
    func add(_ tracks: [Track], play: Bool = false) {
        do { sessionCommand(["op": "append", "tracks": try trackValues(tracks), "action": play ? "play_now" : "add_to_queue"]) }
        catch { report(error) }
    }
    @discardableResult
    private func sessionCommand(_ command: [String: Any]) -> [String: Any]? {
        do { let reply = try session.send(command); applySessionReply(reply); return reply }
        catch { report(error); return nil }
    }
    private func applySessionReply(_ reply: [String: Any]) {
        let view = session.snapshot
        applyingSession = true
        if let tracks = try? workspaceTracks(view["queue"]) { queue = tracks }
        currentIndex = view["current"] as? Int ?? -1
        playing = ["playing", "starting"].contains(view["transport"] as? String ?? "")
        channelInspectionActive = ["playing", "starting", "paused"].contains(view["transport"] as? String ?? "")
        position = view["position"] as? Double ?? 0; duration = view["duration"] as? Double ?? 0
        volume = view["volume"] as? Double ?? 1
        shuffle = ShuffleMode(rawValue: view["shuffle"] as? String ?? "") ?? .off
        repeatMode = RepeatMode(rawValue: view["repeat"] as? String ?? "") ?? .off
        radio = view["radio_enabled"] as? Bool ?? false; radioBusy = view["radio_pending"] as? Bool ?? false
        queueFilter = view["filter"] as? String ?? ""; sortKey = view["sort_column"] as? String ?? "index"
        sortDescending = view["descending"] as? Bool ?? false
        queueSelection = Set((view["selection"] as? [String: Any])?["indices"] as? [Int] ?? [])
        queuedIndices = view["queued"] as? [Int] ?? []; stopAfterIndices = Set(view["stop_after"] as? [Int] ?? [])
        if let value = view["workspace"], let data = try? JSONSerialization.data(withJSONObject: value),
           let snapshot = try? JSONDecoder().decode(PlaylistWorkspaceSnapshot.self, from: data) { workspace = snapshot }
        if let message = view["error"] as? String { error = message }
        applyingSession = false
        for effect in reply["effects"] as? [[String: Any]] ?? [] { executeSessionEffect(effect) }
        updateNowPlaying()
    }
    private func persistCheckpoint(_ value: Any) {
        guard let revision = storageRevision else { return }
        do {
            let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
            if data == lastSavedCheckpoint { return }
            let reply = try deviceAPI.localState("sessions", id: session.id, value: value, revision: revision)
            guard let next = (reply["revision"] as? NSNumber)?.int64Value else { throw KogError.response("Missing checkpoint revision") }
            storageRevision = next
            lastSavedCheckpoint = data
        } catch { report(error) }
    }

    private func executeSessionEffect(_ effect: [String: Any]) {
        switch effect["action"] as? String {
        case "persist": if let value = effect["value"] { persistCheckpoint(value) }
        case "play":
            outputToken = effect["token"] as? [String: Any]
            startPlayer(resumeAt: effect["seconds"] as? Double ?? 0, shouldPlay: effect["playing"] as? Bool ?? true)
        case "pause":
            player?.pause()
            #if KOG_NATIVE_AUDIO
            nativePlayer?.pause()
            #endif
        case "resume":
            player?.play()
            #if KOG_NATIVE_AUDIO
            nativePlayer?.play()
            #endif
        case "stop": stopOutput()
        case "seek": seekOutput(effect["seconds"] as? Double ?? 0)
        case "volume": applyVolume()
        case "load", "save", "expand", "collect", "radio":
            guard let token = effect["token"] as? [String: Any] else { return }
            pendingAdds += 1
            Task {
                defer { pendingAdds -= 1 }
                do {
                    var client = try workspaceAPI(effect["scope"] as? String ?? "")
                    client.sessionID = session.id
                    client.radioRequest = token
                    var result: [String: Any]
                    switch effect["action"] as? String {
                    case "load": result = ["kind": "loaded", "entries": try trackValues(await client.playlist((effect["playlist_id"] as? NSNumber)?.int64Value ?? 0))]
                    case "save":
                        try await client.replacePlaylist((effect["playlist_id"] as? NSNumber)?.int64Value ?? 0, tracks: workspaceTracks(effect["entries"]), expected: workspaceTracks(effect["expected_entries"]))
                        result = ["kind": "saved"]; await loadPlaylists()
                    case "expand": result = ["kind": "expanded", "tracks": try trackValues(await client.expand(workspaceTracks(effect["entries"])))]
                    case "collect": result = ["kind": "expanded", "tracks": try trackValues(await client.collect(effect["path"] as? String ?? "", query: effect["query"] as? String ?? "", root: effect["root"] as? String ?? ""))]
                    default:
                        let root = effect["root"] as? String ?? ""
                        let batch: RadioBatch
                        if effect["reshuffle"] as? Bool == true { batch = try await client.reshuffleRadio(root: root) }
                        else if effect["reset"] as? Bool == true { batch = try await client.radio(effect["enabled"] as? Bool ?? false, root: root) }
                        else { batch = try await client.radioAdvance(root: root) }
                        result = ["kind": "radio", "tracks": try trackValues(batch.tracks), "exhausted": batch.exhausted]
                    }
                    updateSessionScopes()
                    sessionCommand(["op": "complete", "token": token, "result": result])
                } catch { sessionCommand(["op": "complete", "token": token, "result": ["kind": "failed", "error": error.localizedDescription]]) }
            }
        default: break
        }
    }
    private func updateSessionScopes() {
        sessionCommand(["op": "scopes", "scopes": ["device", "server:\(serverKey)"]])
    }
    private func updateSessionMetadata() { sessionCommand(["op": "metadata", "rows": policyRows()]) }
    private func outputEvent(_ event: [String: Any], token: [String: Any]) {
        sessionCommand(["op": "output", "token": token, "event": event])
    }
    func selectShuffle(_ mode: ShuffleMode) { sessionCommand(["op": "shuffle", "mode": mode.rawValue]) }
    func cycleShuffle() { sessionCommand(["op": "cycle_shuffle"]) }
    func selectRepeat(_ mode: RepeatMode) { sessionCommand(["op": "repeat", "mode": mode.rawValue]) }
    func cycleRepeat() { sessionCommand(["op": "cycle_repeat"]) }
    func toggleQueued(_ index: Int) { sessionCommand(["op": "toggle_queued", "indices": [index]]) }
    func toggleStopAfter(_ index: Int) { sessionCommand(["op": "toggle_stop_after", "indices": [index]]) }
    func playIndex(_ index: Int) { sessionCommand(["op": "play", "index": index]) }

    private func startPlayer(resumeAt: Double = 0, shouldPlay: Bool = true) {
        guard let track = current, let requestToken = outputToken else { return }
        do {
            removePlayerObservers()
            player?.pause()
            player = nil
            assetLoader = nil
            #if KOG_NATIVE_AUDIO
            nativeGeneration += 1
            nativeStartTask?.cancel()
            nativePlayer?.stop()
            nativePlayer = nil
            if NativeAudioPlayer.useFor(track) {
                let generation = nativeGeneration
                let path = track.path
                let streamOffset = track.isDevice ? 0 : resumeAt
                let stream = track.isDevice ? nil : try api.nativeStream(track, start: streamOffset).absoluteString
                inspectionStreamURL = stream
                inspectionStreamOffset = streamOffset
                let headers = api.audioHeaders
                let subsong = Int32(track.fragment) ?? -1
                let engine = localMidiEngine
                let soundfont = soundfontPath
                let sc55 = sc55RomPath
                let mt32 = mt32RomPath
                outputEvent(["event": "progress", "seconds": resumeAt, "duration": Double(track.duration) / 1000], token: requestToken)
                updateNowPlaying()
                nativeStartTask = Task { [weak self] in
                    do {
                        let source = try await Task.detached(priority: .userInitiated) {
                            if let stream { return try NativeAudioSource(stream: stream, headers: headers, durationMilliseconds: max(0, track.duration - Int64(streamOffset * 1000))) }
                            return try NativeAudioSource(path: path, subsong: subsong, midiEngine: engine,
                                                         soundfontPath: soundfont, sc55RomPath: sc55, mt32RomPath: mt32)
                        }.value
                        guard let self, !Task.isCancelled,
                              self.nativeGeneration == generation else { return }
                        let decoder = try NativeAudioPlayer(source: source, visualization: self.visualization, onEnd: { [weak self] in
                            Task { @MainActor [weak self] in
                                guard let self, self.nativeGeneration == generation else { return }
                                self.outputEvent(["event": "ended"], token: requestToken)
                            }
                        }, onError: { [weak self] message in
                            Task { @MainActor [weak self] in
                                guard let self, self.nativeGeneration == generation else { return }
                                self.outputEvent(["event": "failed", "error": message], token: requestToken)
                            }
                        })
                        self.nativePlayer = decoder
                        self.outputEvent(["event": "started"], token: requestToken)
                        self.applyVolume()
                        self.outputEvent(["event": "progress", "seconds": resumeAt, "duration": streamOffset + decoder.duration], token: requestToken)
                        if track.isDevice && resumeAt > 0 { decoder.seek(resumeAt) }
                        if self.playing { decoder.play() }
                        self.nativeTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
                            Task { @MainActor [weak self] in
                                guard let self, self.nativeGeneration == generation,
                                      let decoder = self.nativePlayer else { return }
                                self.outputEvent(["event": "progress", "seconds": streamOffset + decoder.position, "duration": streamOffset + decoder.duration], token: requestToken)
                                self.updateNowPlaying()
                            }
                        }
                        self.postTrackNotification(track)
                        self.loadNowPlayingArt(for: track)
                        self.updateNowPlaying()
                    } catch {
                        guard let self, !Task.isCancelled,
                              self.nativeGeneration == generation else { return }
                        self.report(error)
                        self.outputEvent(["event": "failed", "error": self.error ?? "Playback failed"], token: requestToken)
                    }
                }
                return
            }
            #endif
            let stream = try api.stream(track)
            let item: AVPlayerItem
            if !track.isDevice && token.isEmpty && !username.isEmpty {
                let asset = AVURLAsset(url: stream)
                let loader = AuthenticatedAssetLoader(url: stream, username: username, password: password)
                asset.resourceLoader.setDelegate(loader, queue: loader.queue)
                assetLoader = loader
                item = AVPlayerItem(asset: asset)
            } else {
                item = AVPlayerItem(url: stream)
            }
            visualizationTask = Task { await visualization.attach(to: item) }
            player = AVPlayer(playerItem: item)
            applyVolume()
            statusObserver = item.observe(\.status, options: [.initial, .new]) { [weak self] item, _ in
                if item.status == .readyToPlay {
                    Task { @MainActor [weak self] in
                        guard let self, self.player?.currentItem === item else { return }
                        self.outputEvent(["event": "started"], token: requestToken)
                    }
                } else if item.status == .failed {
                    Task { @MainActor [weak self] in
                        guard let self, self.player?.currentItem === item else { return }
                        let failure = item.error?.localizedDescription ?? "Cannot play this file"
                        if !track.isDevice {
                            await self.checkConnection()
                            guard self.player?.currentItem === item else { return }
                        }
                        self.error = !track.isDevice && !self.connected
                            ? "The Kog server is no longer reachable. Check that Kog is running on the server, then tap Play to retry."
                            : failure
                        self.outputEvent(["event": "failed", "error": self.error ?? "Playback failed"], token: requestToken)
                        self.updateNowPlaying()
                    }
                }
            }
            timeObserver = player?.addPeriodicTimeObserver(forInterval: CMTime(seconds: 0.5, preferredTimescale: 600), queue: .main) { [weak self] time in
                Task { @MainActor [weak self] in
                    guard let self, self.player?.currentItem === item else { return }
                    let seconds = self.player?.currentItem?.duration.seconds ?? 0
                    self.outputEvent(["event": "progress", "seconds": time.seconds.isFinite ? time.seconds : 0, "duration": seconds.isFinite ? seconds : Double(track.duration) / 1000], token: requestToken)
                    self.updateNowPlaying()
                }
            }
            finishObserver = NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime,
                object: item, queue: .main) { [weak self] _ in
                Task { @MainActor [weak self] in
                    guard let self, self.player?.currentItem === item else { return }
                    self.outputEvent(["event": "ended"], token: requestToken)
                }
            }
            if resumeAt > 0 { player?.seek(to: CMTime(seconds: resumeAt, preferredTimescale: 600)) }
            if shouldPlay { player?.play() }
            outputEvent(["event": "progress", "seconds": resumeAt, "duration": Double(track.duration) / 1000], token: requestToken)
            loadNowPlayingArt(for: track)
            updateNowPlaying()
            postTrackNotification(track)
        } catch { outputEvent(["event": "failed", "error": error.localizedDescription], token: requestToken) }
    }

    private func removePlayerObservers() {
        visualizationTask?.cancel(); visualization.reset()
        if let observer = timeObserver { player?.removeTimeObserver(observer); timeObserver = nil }
        if let observer = finishObserver { NotificationCenter.default.removeObserver(observer); finishObserver = nil }
        statusObserver = nil
        #if KOG_NATIVE_AUDIO
        nativeTimer?.invalidate(); nativeTimer = nil
        #endif
    }

    func activateQueueIndex(_ index: Int) { sessionCommand(["op": "activate", "index": index]) }
    func togglePlayback() { sessionCommand(["op": "toggle"]) }
    func stop() { sessionCommand(["op": "stop"]) }
    private func stopOutput() {
        outputToken = nil
        removePlayerObservers(); player?.pause(); player = nil; assetLoader = nil
        #if KOG_NATIVE_AUDIO
        nativeGeneration += 1; nativeStartTask?.cancel(); nativePlayer?.stop(); nativePlayer = nil
        #endif
    }
    func seek(_ seconds: Double) { sessionCommand(["op": "seek", "seconds": seconds]) }
    private func seekOutput(_ seconds: Double) {
        #if KOG_NATIVE_AUDIO
        if let track = current, !track.isDevice { sessionCommand(["op": "reload_output"]); return }
        nativePlayer?.seek(seconds)
        #endif
        player?.seek(to: CMTime(seconds: seconds, preferredTimescale: 600))
    }
    func next() { sessionCommand(["op": "navigate", "event": "next"]) }
    func previous() { sessionCommand(["op": "navigate", "event": "previous"]) }
    func remove(_ offsets: IndexSet) { sessionCommand(["op": "remove", "indices": Array(offsets)]) }
    func move(_ source: IndexSet, to destination: Int) { sessionCommand(["op": "move", "indices": Array(source), "target": destination]) }
    func clearQueue() { sessionCommand(["op": "clear"]) }

    private func policyRows() -> [[String: Any]] {
        queue.map { track in
            var row: [String: Any] = ["original": track.queueOrder.map { $0 as Any } ?? NSNull(),
                "title": track.label, "artist": track.artist, "album": track.album,
                "duration": track.duration > 0 ? (Double(track.duration) / 1000) as Any : NSNull(),
                "path": track.displayPath, "filename": track.filename, "star": isStarred(track)]
            for field in TrackSort.metadataFields {
                let value = track.metadata[field.key] ?? ""
                row[field.key] = field.numeric ? (Double(value).map { $0 as Any } ?? NSNull()) : value
            }
            return row
        }
    }
    func filteredQueueIndices() -> [Int] { session.snapshot["visible"] as? [Int] ?? [] }
    func sortQueue(_ key: String) {
        sessionCommand(["op": "metadata", "rows": policyRows()])
        sessionCommand(["op": "sort", "column": key, "descending": sortKey == key ? !sortDescending : false, "physical": true])
    }

    func isStarred(_ track: Track) -> Bool { (track.isDevice ? localStars : stars).contains(track.id) }
    func toggleStar(_ track: Track) async {
        do {
            let enabled = !isStarred(track)
            try await (track.isDevice ? deviceAPI : api).star(track, enabled: enabled)
            if track.isDevice {
                if enabled { localStars.insert(track.id) } else { localStars.remove(track.id) }
            } else {
                if enabled { stars.insert(track.id) } else { stars.remove(track.id) }
            }
            updateSessionMetadata()
            if selectedPlaylist?.id == 0 { playlistTracks.removeAll { $0.id == track.id && !enabled } }
        } catch { report(error) }
    }

    func toggleRadio() async { prepareRadio(selectSource: true, enabled: !radio) }
    private func prepareRadio(selectSource: Bool = false, reshuffle: Bool = false, enabled: Bool? = nil) {
        if selectSource { radioOnDevice = libraryOnDevice }
        sessionCommand(["op": "radio", "enabled": enabled ?? radio, "scope": radioOnDevice ? "device" : "server:\(serverKey)",
            "root": radioOnDevice ? activeDeviceRoot : activeTreeRoot, "reshuffle": reshuffle])
    }
    func reshuffleRadio() async { prepareRadio(reshuffle: true, enabled: true) }
    private var workspaceScope: String { playlistOnDevice ? "device" : "server:\(serverKey)" }
    func openPlaylistTab(_ playlist: SavedPlaylist) {
        workspaceCommand(["op": "open", "key": "\(workspaceScope):\(playlist.id)", "scope": workspaceScope,
            "playlist_id": playlist.id, "name": playlist.name, "readonly": playlist.id == 0])
    }
    private func workspaceAPI(_ scope: String) throws -> KogAPI {
        if scope == "device" { return deviceAPI }
        guard scope == "server:\(serverKey)" else { throw KogError.response("Reconnect to this playlist's server to load or save it.") }
        return api
    }
    private func workspaceTracks(_ value: Any?) throws -> [Track] {
        let data = try JSONSerialization.data(withJSONObject: value ?? [])
        return try JSONDecoder().decode([Track].self, from: data)
    }
    func workspaceAppend(_ tracks: [Track]) {
        do {
            let entries = try JSONSerialization.jsonObject(with: JSONEncoder().encode(tracks))
            workspaceCommand(["op": "append", "entries": entries])
        } catch { report(error) }
    }
    func workspaceSelect(_ index: Int) {
        workspaceCommand(["op": "selection", "command": ["op": "choose", "index": index, "gesture": "toggle"]])
    }
    func selectQueue(_ command: [String: Any]) {
        sessionCommand(["op": "select", "command": command])
    }

    func workspaceCommand(_ command: [String: Any]) { sessionCommand(["op": "workspace", "command": command]) }
    func workspaceAppendQueue(selectedOnly: Bool = false) { sessionCommand(["op": "append_queue_to_workspace", "selected_only": selectedOnly]) }

    func loadPlaylists() async {
        do { playlists = try await playlistAPI.playlists() } catch { report(error) }
    }
    @discardableResult func openPlaylist(_ playlist: SavedPlaylist) async -> Bool {
        let device = playlistOnDevice
        do {
            let tracks = try await playlistAPI.playlist(playlist.id)
            guard device == playlistOnDevice else { return false }
            playlistTracks = tracks; selectedPlaylist = playlist; return true
        } catch { report(error); return false }
    }
    func createPlaylist(_ name: String, saveQueue: Bool = false) async {
        do {
            if saveQueue { try checkPlaylistSource(queue) }
            _ = try await playlistAPI.createPlaylist(name, tracks: saveQueue ? queue : [])
            await loadPlaylists()
        } catch { report(error) }
    }
    func renamePlaylist(_ playlist: SavedPlaylist, name: String) async {
        let key = "\(workspaceScope):\(playlist.id)"
        do { try await playlistAPI.renamePlaylist(playlist.id, name: name); workspaceCommand(["op": "renamed", "key": key, "name": name]); await loadPlaylists()
            if selectedPlaylist?.id == playlist.id { selectedPlaylist?.name = name }
        } catch { report(error) }
    }
    func deletePlaylist(_ playlist: SavedPlaylist) async {
        let key = "\(workspaceScope):\(playlist.id)"
        do { try await playlistAPI.deletePlaylist(playlist.id); workspaceCommand(["op": "deleted", "key": key]); selectedPlaylist = nil; await loadPlaylists() }
        catch { report(error) }
    }
    private func checkPlaylistSource(_ tracks: [Track]) throws {
        guard tracks.allSatisfy({ $0.isDevice == playlistOnDevice }) else {
            throw KogError.response(playlistOnDevice ? "Save server tracks to this iPhone before adding them to an offline playlist." : "Choose On this iPhone to save device tracks in a playlist.")
        }
    }
    func appendToPlaylist(_ playlist: SavedPlaylist, tracks: [Track]) async {
        do { try checkPlaylistSource(tracks); try await playlistAPI.appendPlaylist(playlist.id, tracks: tracks)
            await loadPlaylists(); if selectedPlaylist?.id == playlist.id { await openPlaylist(playlist) }
        } catch { report(error) }
    }
    func duplicatePlaylist(_ playlist: SavedPlaylist) async {
        do {
            let names = Set(try await playlistAPI.playlists().map(\.name))
            var name = playlist.name + " copy", suffix = 2
            while names.contains(name) { name = playlist.name + " copy \(suffix)"; suffix += 1 }
            try await playlistAPI.duplicatePlaylist(playlist.id, name: name); await loadPlaylists()
        } catch { report(error) }
    }
    func prunePlaylist(_ playlist: SavedPlaylist) async {
        do { try await playlistAPI.prunePlaylist(playlist.id); await loadPlaylists(); await openPlaylist(playlist) }
        catch { report(error) }
    }
    func removePlaylistTracks(_ offsets: IndexSet) async {
        guard let selected = selectedPlaylist else { return }
        if selected.id == 0 { let tracks = offsets.map { playlistTracks[$0] }; for track in tracks { await toggleStar(track) }; await openPlaylist(selected); return }
        var tracks = playlistTracks; tracks.remove(atOffsets: offsets)
        do { try await playlistAPI.replacePlaylist(selected.id, tracks: tracks); playlistTracks = tracks; await loadPlaylists() }
        catch { report(error) }
    }
    func replaceQueue(_ tracks: [Track], play: Bool = true) { clearQueue(); add(tracks, play: play) }
    func exportPlaylist(_ playlist: SavedPlaylist) async -> URL? {
        do {
            let text = try await playlistAPI.exportPlaylist(playlist.id)
            let url = FileManager.default.temporaryDirectory.appendingPathComponent("Kog-\(playlist.id).m3u8")
            try text.write(to: url, atomically: true, encoding: .utf8); return url
        } catch { report(error); return nil }
    }

    func importFiles(_ urls: [URL]) async {
        importing = true
        let destinationRoot = importsURL
        do {
            try await Task.detached(priority: .userInitiated) {
                let manager = FileManager.default
                try manager.createDirectory(at: destinationRoot, withIntermediateDirectories: true)
                for source in urls {
                    let access = source.startAccessingSecurityScopedResource()
                    defer { if access { source.stopAccessingSecurityScopedResource() } }
                    var isDirectory: ObjCBool = false
                    guard manager.fileExists(atPath: source.path, isDirectory: &isDirectory) else { continue }
                    let name = source.lastPathComponent
                    var destination = destinationRoot.appendingPathComponent(name)
                    var suffix = 2
                    while manager.fileExists(atPath: destination.path) {
                        destination = destinationRoot.appendingPathComponent("\(suffix)-\(name)")
                        suffix += 1
                    }
                    try manager.copyItem(at: source, to: destination)
                }
            }.value
            scanImports()
        } catch { importing = false; report(error) }
    }

    func saveFromServer(_ track: Track) async {
        guard !downloading.contains(track.id) else { return }
        downloading.insert(track.id)
        defer { downloading.remove(track.id) }
        do {
            // Preserve companion sample banks and nested archives for offline playback.
            let download = track.kind == "archive" ? Track(kind: "local", path: track.path) : track
            let (temporary, filename) = try await api.download(download)
            let root = importsURL
            try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
            let stem = (filename as NSString).deletingPathExtension
            let ext = (filename as NSString).pathExtension
            var destination = root.appendingPathComponent(filename)
            var suffix = 2
            while FileManager.default.fileExists(atPath: destination.path) {
                let uniqueName = "\(stem) (\(suffix))" + (ext.isEmpty ? "" : ".\(ext)")
                destination = root.appendingPathComponent(uniqueName)
                suffix += 1
            }
            try FileManager.default.moveItem(at: temporary, to: destination)
            if devicePath.isEmpty || devicePath == root.path { scanImports() }
            let notice = "Saved \(destination.lastPathComponent) on this iPhone"
            downloadNotice = notice
            downloadNoticeTask?.cancel()
            downloadNoticeTask = Task { [weak self] in
                try? await Task.sleep(for: .seconds(3))
                if !Task.isCancelled, self?.downloadNotice == notice { self?.downloadNotice = nil }
            }
        } catch { report(error) }
    }

    func deleteDeviceFile(_ track: Track) {
        do {
            try FileManager.default.removeItem(atPath: track.path)
            let removed = IndexSet(queue.indices.filter { queue[$0].isDevice && queue[$0].path == track.path })
            if !removed.isEmpty { remove(removed) }
            scanImports()
        } catch { report(error) }
    }

    func scanImports() {
        browseDevice(devicePath.isEmpty ? activeDeviceRoot : devicePath)
    }

    func browseDevice(_ path: String) {
        #if KOG_NATIVE_AUDIO
        importScanTask?.cancel()
        importing = true
        importScanTask = Task {
            do {
                let listing = try await deviceAPI.browse(path)
                guard !Task.isCancelled else { return }
                deviceListing = listing
                devicePath = path
                deviceFiles = listing.files
                importing = false
            } catch {
                if !Task.isCancelled { importing = false; report(error) }
            }
        }
        #endif
    }

    private func loadNowPlayingArt(for track: Track) {
        nowPlayingArtTask?.cancel()
        nowPlayingArtwork = nil
        nowPlayingArtTask = Task { [weak self] in
            let data: Data?
            if track.isDevice {
                #if KOG_NATIVE_AUDIO
                let path = track.path
                data = await Task.detached(priority: .utility) {
                    NativeAudioCatalog.artwork(path: path)
                }.value
                #else
                data = nil
                #endif
            } else {
                data = try? await self?.api.artData(track)
            }
            guard !Task.isCancelled, self?.current?.id == track.id,
                  let data, let image = UIImage(data: data) else { return }
            self?.nowPlayingArtwork = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
            self?.updateNowPlaying()
        }
    }

    private func updateNowPlaying() {
        guard let track = current else { return }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: track.label,
            MPMediaItemPropertyArtist: track.artist,
            MPMediaItemPropertyAlbumTitle: track.album,
            MPMediaItemPropertyPlaybackDuration: duration,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: position,
            MPNowPlayingInfoPropertyPlaybackRate: playing ? 1.0 : 0.0,
        ]
        if let nowPlayingArtwork { info[MPMediaItemPropertyArtwork] = nowPlayingArtwork }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
    }

    private func registerRemoteCommands() {
        let commands = MPRemoteCommandCenter.shared()
        commands.stopCommand.addTarget { [weak self] _ in
            Task { @MainActor [weak self] in self?.stop() }; return .success
        }
        commands.playCommand.addTarget { [weak self] _ in
            Task { @MainActor [weak self] in if self?.playing == false { self?.togglePlayback() } }
            return .success
        }
        commands.pauseCommand.addTarget { [weak self] _ in
            Task { @MainActor [weak self] in if self?.playing == true || self?.session.waiting == true { self?.togglePlayback() } }
            return .success
        }
        commands.togglePlayPauseCommand.addTarget { [weak self] _ in
            Task { @MainActor [weak self] in self?.togglePlayback() }; return .success
        }
        commands.nextTrackCommand.addTarget { [weak self] _ in
            Task { @MainActor [weak self] in self?.next() }; return .success
        }
        commands.previousTrackCommand.addTarget { [weak self] _ in
            Task { @MainActor [weak self] in self?.previous() }; return .success
        }
        commands.changePlaybackPositionCommand.addTarget { [weak self] event in
            guard let event = event as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            Task { @MainActor [weak self] in self?.seek(event.positionTime) }; return .success
        }
    }
}


extension KogStore {
    private func applyVolume() {
        let value = Float(muted ? 0 : min(1, max(0, volume)))
        player?.volume = value
        #if KOG_NATIVE_AUDIO
        nativePlayer?.volume = value
        #endif
    }
    func selectCodec(_ value: String) {
        guard value != codec else { return }
        codec = value; KogPreferences.standard.set(value, forKey: "codec")
        if current?.isDevice == false { sessionCommand(["op": "reload_output"]) }
    }
    func toggleSearchPause() async {
        do { try await searchAPI.pauseSearch(!searchPaused); searchPaused.toggle() }
        catch { report(error) }
    }
    func addURL(_ text: String) async {
        guard let url = URL(string: text.trimmingCharacters(in: .whitespacesAndNewlines)),
              ["http", "https"].contains(url.scheme?.lowercased() ?? "") else {
            error = "Enter an HTTP or HTTPS music URL."; return
        }
        await addFile(Track(kind: "remote", path: url.absoluteString), play: true)
    }
    func setDeviceRoot(_ path: String) {
        guard path == importsURL.path || path.hasPrefix(importsURL.path + "/") else { return }
        deviceTreeRoot = path == importsURL.path ? "" : String(path.dropFirst(importsURL.path.count + 1))
        KogPreferences.standard.set(deviceTreeRoot, forKey: "device_tree_root")
        if radio && radioOnDevice { prepareRadio() }
        search(""); browseDevice(path)
    }
    func reveal(_ track: Track) async {
        libraryOnDevice = track.isDevice
        let location = track.locator
        let path = location["path"] ?? track.path
        let parent = location["kind"] == "archive" ? path : URL(fileURLWithPath: path).deletingLastPathComponent().path
        if track.isDevice { browseDevice(parent) } else { await browse(parent) }
    }
    func setNotifications(_ enabled: Bool) async {
        if enabled {
            do { notifyTracks = try await UNUserNotificationCenter.current().requestAuthorization(options: [.alert]) }
            catch { report(error); notifyTracks = false }
        } else { notifyTracks = false }
        KogPreferences.standard.set(notifyTracks, forKey: "track_notifications")
    }
    private func postTrackNotification(_ track: Track) {
        guard notifyTracks, UIApplication.shared.applicationState != .active else { return }
        let content = UNMutableNotificationContent(); content.title = track.label; content.body = track.detail
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: "kog-track", content: content, trigger: nil))
    }
    private func observeAudioSession() {
        audioObservers.append(NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
            let type = (note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt) ?? 0
            let options = (note.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt) ?? 0
            Task { @MainActor [weak self] in
                guard let self else { return }
                if type == AVAudioSession.InterruptionType.began.rawValue {
                    self.interruptedPlayback = self.playing
                    if self.playing { self.togglePlayback() }
                } else if self.interruptedPlayback {
                    self.interruptedPlayback = false
                    if options & AVAudioSession.InterruptionOptions.shouldResume.rawValue != 0 {
                        try? AVAudioSession.sharedInstance().setActive(true)
                        self.sessionCommand(["op": "resume"])
                    }
                }
            }
        })
        audioObservers.append(NotificationCenter.default.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] note in
            let reason = (note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt) ?? 0
            if reason == AVAudioSession.RouteChangeReason.oldDeviceUnavailable.rawValue {
                Task { @MainActor [weak self] in if self?.playing == true || self?.session.waiting == true { self?.togglePlayback() } }
            }
        })
        audioObservers.append(NotificationCenter.default.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self else { return }
                try? AVAudioSession.sharedInstance().setCategory(.playback)
                try? AVAudioSession.sharedInstance().setActive(true)
                if self.current != nil { self.sessionCommand(["op": "reload_output"]) }
            }
        })
    }
    private func rebaseWorkspace(_ value: Any) -> Any {
        func entries(_ value: Any) -> Any {
            if let array = value as? [Any] { return array.map(entries) }
            if var object = value as? [String: Any] {
                for (key, child) in object { object[key] = entries(child) }
                if let path = object["path"] as? String { object["path"] = rebaseDevicePath(path) }
                return object
            }
            return value
        }
        guard var object = value as? [String: Any], let tabs = object["tabs"] as? [[String: Any]] else { return value }
        object["tabs"] = tabs.map { tab in tab["scope"] as? String == "device" ? entries(tab) : tab }
        return object
    }

    private func rebaseDevicePath(_ path: String) -> String {
        if path.hasPrefix("kog-archive:"), var url = URLComponents(string: path) {
            url.queryItems = url.queryItems?.map { item in
                item.name == "archive" ? URLQueryItem(name: item.name, value: rebaseDevicePath(item.value ?? "")) : item
            }
            return url.string ?? path
        }
        guard let range = path.range(of: "/Documents/"), path.hasPrefix("/var/mobile/") || path.hasPrefix("/private/var/mobile/") else { return path }
        return importsURL.deletingLastPathComponent().appendingPathComponent(String(path[range.upperBound...])).path
    }
    private func rebaseDeviceTrack(_ track: Track) -> Track {
        guard track.isDevice else { return track }
        var copy = track; copy.path = rebaseDevicePath(copy.path); return copy
    }
}
