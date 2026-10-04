import AVFoundation
import Combine
import MediaPlayer
import SwiftUI
import UniformTypeIdentifiers
import UserNotifications

@MainActor
final class KogStore: ObservableObject {
    @Published var server = UserDefaults.standard.string(forKey: "server") ?? ""
    @Published var username = UserDefaults.standard.string(forKey: "username") ?? ""
    @Published var token = Secrets.read("token")
    @Published var password = Secrets.read("password")
    @Published var codec = UserDefaults.standard.string(forKey: "codec") ?? "aac"
    @Published var midiEngine = UserDefaults.standard.string(forKey: "midi_engine") ?? "opl3windows"
    @Published var localMidiEngine = UserDefaults.standard.string(forKey: "local_midi_engine") ?? "opl3windows"
    @Published var soundfontPath = UserDefaults.standard.string(forKey: "midi_soundfont") ?? ""
    @Published var sc55RomPath = UserDefaults.standard.string(forKey: "midi_sc55_roms") ?? ""
    @Published var mt32RomPath = UserDefaults.standard.string(forKey: "midi_mt32_roms") ?? ""
    @Published var connected = false
    @Published var listing: Listing?
    @Published var libraryRoot = ""
    @Published private(set) var treeRoot = ""
    @Published var searchText = ""
    @Published var searchFolders = [Folder]()
    @Published var searchTracks = [Track]()
    @Published var searchScanned = 0
    @Published var searching = false
    @Published var queue = [Track]() { didSet { saveQueue() } }
    @Published var currentIndex = -1 { didSet { saveQueue() } }
    @Published var playing = false
    @Published var position = 0.0
    @Published var duration = 0.0
    @Published private(set) var shuffle = ShuffleMode(rawValue: UserDefaults.standard.string(forKey: "shuffle_mode") ?? "") ?? (UserDefaults.standard.bool(forKey: "shuffle") ? .all : .off)
    @Published private(set) var repeatMode = RepeatMode(rawValue: UserDefaults.standard.string(forKey: "repeat_mode") ?? "") ?? (UserDefaults.standard.bool(forKey: "repeat") ? .all : .off) {
        didSet { UserDefaults.standard.set(repeatMode.rawValue, forKey: "repeat_mode") }
    }
    @Published var volume = UserDefaults.standard.object(forKey: "player_volume") as? Double ?? 1.0 {
        didSet { UserDefaults.standard.set(volume, forKey: "player_volume"); applyVolume() }
    }
    @Published var muted = false { didSet { applyVolume() } }
    @Published var libraryOnDevice = false { didSet { search("") } }
    @Published var playlistOnDevice = (UserDefaults.standard.string(forKey: "server") ?? "").isEmpty { didSet { selectedPlaylist = nil; Task { await loadPlaylists() } } }
    @Published var localStars = Set<String>()
    @Published var searchPaused = false
    @Published var radioBusy = false
    @Published var notifyTracks = UserDefaults.standard.bool(forKey: "track_notifications")
    @Published var queueFilter = ""
    @Published var sortKey = "title"
    @Published var sortDescending = false
    @Published var deviceTreeRoot = UserDefaults.standard.string(forKey: "device_tree_root") ?? ""
    private var radioOnDevice = false
    private var radioTask: Task<Void, Never>?
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
    #if KOG_NATIVE_AUDIO
    private var nativePlayer: NativeAudioPlayer?
    private var nativeTimer: Timer?
    private var nativeStartTask: Task<Void, Never>?
    private var nativeGeneration = 0
    #endif
    private var timeObserver: Any?
    private var finishObserver: NSObjectProtocol?
    private var statusObserver: NSKeyValueObservation?
    private var searchTask: Task<Void, Never>?
    private var importScanTask: Task<Void, Never>?
    private let policy = SharedPlaybackPolicy()
    @Published private(set) var queueSelection = Set<Int>()
    @Published private(set) var workspace = PlaylistWorkspaceSnapshot.empty
    private var lastWorkspaceData: Data?
    private var workspaceQueueGeneration = 0
    private var workspaceQueueTask: Task<Void, Never>?
    private var startingPrevious: Int?
    @Published private(set) var queuedIndices = [Int]()
    @Published private(set) var stopAfterIndices = Set<Int>()
    private var suppressSave = false
    private var downloadNoticeTask: Task<Void, Never>?
    private var nowPlayingArtTask: Task<Void, Never>?
    private var nowPlayingArtwork: MPMediaItemArtwork?

    var current: Track? { queue.indices.contains(currentIndex) ? queue[currentIndex] : nil }
    var deviceAPI: KogAPI {
        KogAPI(server: "", token: "", username: "", password: "", codec: codec,
               deviceRoot: importsURL.path,
               deviceStorage: FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Kog").path)
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
                            password: password, codec: codec, midiEngine: midiEngine) }
    var importsURL: URL {
        let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        return documents.appendingPathComponent("Kog Imports", isDirectory: true)
    }

    init() {
        if let data = UserDefaults.standard.data(forKey: "queue"),
           let tracks = try? JSONDecoder().decode([Track].self, from: data) { queue = tracks }
        currentIndex = UserDefaults.standard.integer(forKey: "index")
        if queue.isEmpty { currentIndex = -1 }
        else { currentIndex = min(max(0, currentIndex), queue.count - 1) }
        // iOS may move an app container during installation. Persisted local
        // paths must follow Documents, including archive and synth paths.
        queue = queue.enumerated().map { index, track in var copy = rebaseDeviceTrack(track); if copy.queueOrder == nil { copy.queueOrder = Int64(index) }; return copy }
        soundfontPath = rebaseDevicePath(soundfontPath)
        sc55RomPath = rebaseDevicePath(sc55RomPath)
        mt32RomPath = rebaseDevicePath(mt32RomPath)
        let restoredWorkspace = UserDefaults.standard.data(forKey: "playlist_workspace")
        _ = policyCommand(["op": "init", "seed": UInt32.random(in: 1...UInt32.max), "shuffle": shuffle.rawValue, "repeat": repeatMode.rawValue], sync: false)
        if let restoredWorkspace, let value = try? JSONSerialization.jsonObject(with: restoredWorkspace) {
            _ = policyCommand(["op": "workspace_restore", "value": rebaseWorkspace(value)], sync: false)
        }
        syncPolicy()
        scanImports()
        Task { if let saved = try? await deviceAPI.stars() { localStars = saved } }
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
        UserDefaults.standard.set(server, forKey: "server")
        UserDefaults.standard.set(username, forKey: "username")
        UserDefaults.standard.set(codec, forKey: "codec")
        Secrets.write("token", token)
        Secrets.write("password", password)
        Task { await refresh() }
    }

    private func isMidi(_ track: Track) -> Bool {
        let name = track.entry.isEmpty ? track.path : track.entry
        return ["kar", "mid", "midi", "rmi", "mids", "mds", "lds", "xmf", "mxmf"]
            .contains(URL(fileURLWithPath: name).pathExtension.lowercased())
    }

    private func restartCurrentMidi() {
        guard let track = current, isMidi(track) else { return }
        startPlayer(resumeAt: position, shouldPlay: playing)
    }

    func selectMidiEngine(_ engine: String) {
        midiEngine = engine
        UserDefaults.standard.set(engine, forKey: "midi_engine")
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
        UserDefaults.standard.set(engine, forKey: "local_midi_engine")
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
            UserDefaults.standard.set(destination.path, forKey: "midi_\(kind == "soundfont" ? "soundfont" : kind + "_roms")")
            if current?.isDevice == true { restartCurrentMidi() }
        } catch { report(error) }
    }

    private func saveQueue() {
        guard !suppressSave else { return }
        UserDefaults.standard.set(try? JSONEncoder().encode(queue), forKey: "queue")
        UserDefaults.standard.set(currentIndex, forKey: "index")
        UserDefaults.standard.set(shuffle.rawValue, forKey: "shuffle_mode")
        UserDefaults.standard.set(repeatMode.rawValue, forKey: "repeat_mode")
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
        var roots = UserDefaults.standard.dictionary(forKey: "server_tree_roots") as? [String: String] ?? [:]
        roots[serverKey] = treeRoot.isEmpty ? nil : treeRoot
        UserDefaults.standard.set(roots, forKey: "server_tree_roots")
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
            let roots = UserDefaults.standard.dictionary(forKey: "server_tree_roots") as? [String: String] ?? [:]
            treeRoot = roots[serverKey] ?? ""
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
            if let serverEngine = try? await client.serverMidiEngine(), serverEngine != midiEngine {
                midiEngine = serverEngine
                UserDefaults.standard.set(serverEngine, forKey: "midi_engine")
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

    func addFile(_ track: Track, play: Bool = false) async {
        pendingAdds += 1; defer { pendingAdds -= 1 }
        do {
            add(try await (track.isDevice ? deviceAPI : api).expand(track), play: play)
        } catch { report(error) }
    }

    func addDeviceFolder(_ folder: Folder) async {
        pendingAdds += 1; defer { pendingAdds -= 1 }
        do { add(try await deviceAPI.collect(folder.path, query: searchText, root: activeDeviceRoot)) }
        catch { report(error) }
    }

    func addFolder(_ folder: Folder, play: Bool = false) async {
        pendingAdds += 1; defer { pendingAdds -= 1 }
        do { add(try await api.collect(folder.path, query: searchText, root: activeTreeRoot), play: play) }
        catch { report(error) }
    }

    func add(_ tracks: [Track], play: Bool = false) {
        guard !tracks.isEmpty else { return }
        let start = queue.count
        let order = (queue.compactMap(\.queueOrder).max() ?? -1) + 1
        queue += tracks.enumerated().map { offset, track in var copy = track; copy.queueOrder = order + Int64(offset); return copy }
        syncPolicy()
        if play { playIndex(start) }
    }

    private func syncPolicy(oldToNew: [Int?]? = nil) {
        do { try policy.sync(queue, current: currentIndex, oldToNew: oldToNew) }
        catch { report(error) }
        applyPolicySnapshot()
    }

    private func applyPolicySnapshot() {
        shuffle = ShuffleMode(rawValue: policy.snapshot["shuffle"] as? String ?? "") ?? .off
        repeatMode = RepeatMode(rawValue: policy.snapshot["repeat"] as? String ?? "") ?? .off
        UserDefaults.standard.set(shuffle.rawValue, forKey: "shuffle_mode")
        radio = policy.radio["enabled"] as? Bool ?? false
        radioBusy = policy.radio["pending"] as? Bool ?? false
        queueSelection = Set((policy.snapshot["queue_selection"] as? [String: Any])?["indices"] as? [Int] ?? [])
        queuedIndices = policy.snapshot["queued"] as? [Int] ?? []
        stopAfterIndices = Set(policy.snapshot["stop_after"] as? [Int] ?? [])
        if let value = policy.snapshot["workspace"], let data = try? JSONSerialization.data(withJSONObject: value),
           let snapshot = try? JSONDecoder().decode(PlaylistWorkspaceSnapshot.self, from: data) { workspace = snapshot }
        if let value = policy.snapshot["workspace_state"], let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]), data != lastWorkspaceData {
            UserDefaults.standard.set(data, forKey: "playlist_workspace"); lastWorkspaceData = data
        }
    }

    @discardableResult
    private func policyCommand(_ command: [String: Any], sync: Bool = true) -> [String: Any]? {
        do {
            if sync { try policy.sync(queue, current: currentIndex) }
            let reply = try policy.send(command)
            applyPolicySnapshot()
            if let message = reply["error"] as? String { self.error = message }
            return reply
        } catch { report(error); return nil }
    }

    func selectShuffle(_ mode: ShuffleMode) {
        _ = policyCommand(["op": "set_shuffle", "mode": mode.rawValue, "current": SharedPlaybackPolicy.index(currentIndex)])
        if !radio { radioTask?.cancel() }
    }
    func cycleShuffle() {
        _ = policyCommand(["op": "cycle_shuffle", "current": SharedPlaybackPolicy.index(currentIndex)])
        if !radio { radioTask?.cancel() }
    }
    func selectRepeat(_ mode: RepeatMode) {
        _ = policyCommand(["op": "set_repeat", "mode": mode.rawValue])
        if !radio { radioTask?.cancel() }
    }
    func cycleRepeat() {
        _ = policyCommand(["op": "cycle_repeat"])
        if !radio { radioTask?.cancel() }
    }
    func toggleQueued(_ index: Int) { _ = policyCommand(["op": "toggle_queue", "indices": [index]]) }
    func toggleStopAfter(_ index: Int) { _ = policyCommand(["op": "toggle_stop_after", "indices": [index]]) }

    func playIndex(_ index: Int) {
        guard queue.indices.contains(index) else { return }
        _ = policyCommand(["op": "cancel_navigation"])
        _ = policyCommand(["op": "cancel_waiting"])
        activateIndex(index)
    }

    private func activateIndex(_ index: Int) {
        guard queue.indices.contains(index) else { return }
        startingPrevious = currentIndex
        currentIndex = index
        startPlayer()
    }

    private func startPlayer(resumeAt: Double = 0, shouldPlay: Bool = true) {
        guard let track = current else { return }
        let previous = startingPrevious ?? currentIndex
        startingPrevious = nil
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
                let headers = api.audioHeaders
                let subsong = Int32(track.fragment) ?? -1
                let engine = localMidiEngine
                let soundfont = soundfontPath
                let sc55 = sc55RomPath
                let mt32 = mt32RomPath
                playing = shouldPlay; position = resumeAt; duration = Double(track.duration) / 1000
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
                                self.finishedTrack()
                            }
                        }, onError: { [weak self] message in
                            Task { @MainActor [weak self] in
                                guard let self, self.nativeGeneration == generation else { return }
                                self.error = message; self.navigate("failed")
                            }
                        })
                        self.nativePlayer = decoder
                        _ = self.policyCommand(["op": "started", "previous": SharedPlaybackPolicy.index(previous), "index": self.currentIndex])
                        self.applyVolume()
                        self.duration = streamOffset + decoder.duration
                        if track.isDevice && resumeAt > 0 { decoder.seek(resumeAt) }
                        if self.playing { decoder.play() }
                        self.nativeTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
                            Task { @MainActor [weak self] in
                                guard let self, self.nativeGeneration == generation,
                                      let decoder = self.nativePlayer else { return }
                                self.position = streamOffset + decoder.position
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
                        self.navigate("failed")
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
                        _ = self.policyCommand(["op": "started", "previous": SharedPlaybackPolicy.index(previous), "index": self.currentIndex])
                    }
                } else if item.status == .failed {
                    Task { @MainActor [weak self] in
                        guard let self, self.player?.currentItem === item else { return }
                        self.playing = false
                        let failure = item.error?.localizedDescription ?? "Cannot play this file"
                        if !track.isDevice {
                            await self.checkConnection()
                            guard self.player?.currentItem === item else { return }
                        }
                        self.error = !track.isDevice && !self.connected
                            ? "The Kog server is no longer reachable. Check that Kog is running on the server, then tap Play to retry."
                            : failure
                        self.navigate("failed")
                        self.updateNowPlaying()
                    }
                }
            }
            timeObserver = player?.addPeriodicTimeObserver(forInterval: CMTime(seconds: 0.5, preferredTimescale: 600), queue: .main) { [weak self] time in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    self.position = time.seconds.isFinite ? time.seconds : 0
                    let seconds = self.player?.currentItem?.duration.seconds ?? 0
                    self.duration = seconds.isFinite ? seconds : Double(track.duration) / 1000
                    self.updateNowPlaying()
                }
            }
            finishObserver = NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime,
                object: item, queue: .main) { [weak self] _ in
                Task { @MainActor [weak self] in self?.finishedTrack() }
            }
            if resumeAt > 0 { player?.seek(to: CMTime(seconds: resumeAt, preferredTimescale: 600)) }
            if shouldPlay { player?.play() }
            playing = shouldPlay
            position = resumeAt
            loadNowPlayingArt(for: track)
            updateNowPlaying()
            postTrackNotification(track)
        } catch { report(error); navigate("failed") }
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

    func activateIndex(_ index: Int) {
        guard let activation = policyCommand(["op": "activate", "index": index, "current": SharedPlaybackPolicy.index(currentIndex)])?["activation"] as? [String: Any] else { return }
        if activation["action"] as? String == "toggle_playback" { togglePlayback() }
        else if let index = activation["index"] as? Int { playIndex(index) }
    }

    func togglePlayback() {
        if policy.waiting { stop(); return }
        if queue.isEmpty && radio { navigate("next"); return }
        if playing {
            player?.pause()
            #if KOG_NATIVE_AUDIO
            nativePlayer?.pause()
            #endif
            playing = false
        }
        else {
            if player?.currentItem?.status == .failed {
                startPlayer(resumeAt: position)
                return
            }
            if player != nil { player?.play(); playing = true }
            #if KOG_NATIVE_AUDIO
            if nativePlayer != nil { nativePlayer?.play(); playing = true }
            #endif
            if !playing && !queue.isEmpty { playIndex(max(0, currentIndex)) }
        }
        updateNowPlaying()
    }

    func stop() {
        _ = policyCommand(["op": "cancel_navigation"])
        _ = policyCommand(["op": "cancel_waiting"])
        removePlayerObservers()
        player?.pause(); player = nil; assetLoader = nil
        #if KOG_NATIVE_AUDIO
        nativeGeneration += 1; nativeStartTask?.cancel()
        nativePlayer?.stop(); nativePlayer = nil
        #endif
        playing = false; position = 0; updateNowPlaying()
    }
    private func finishedTrack() {
        navigate("ended")
    }
    func seek(_ seconds: Double) {
        guard seconds.isFinite else { return }
        let target = min(max(0, seconds), duration > 0 ? max(0, duration - 0.01) : max(0, seconds))
        #if KOG_NATIVE_AUDIO
        if let track = current, !track.isDevice {
            startPlayer(resumeAt: target, shouldPlay: playing)
            return
        }
        nativePlayer?.seek(target)
        #endif
        player?.seek(to: CMTime(seconds: target, preferredTimescale: 600))
        position = target
        updateNowPlaying()
    }

    private func navigate(_ event: String) {
        guard let decision = policyCommand(["op": "navigate", "event": event,
            "current": SharedPlaybackPolicy.index(currentIndex)])?["decision"] as? [String: Any] else { return }
        switch decision["action"] as? String {
        case "play": if let index = decision["index"] as? Int { activateIndex(index) }
        case "radio": advanceRadio()
        default: stop()
        }
    }
    func next() { navigate("next") }
    func previous() { navigate("previous") }

    func remove(_ offsets: IndexSet) {
        let remaining = queue.indices.filter { !offsets.contains($0) }
        let remap = queue.indices.map { remaining.firstIndex(of: $0) }
        let removedCurrent = offsets.contains(currentIndex)
        if removedCurrent { stop() }
        queue.remove(atOffsets: offsets)
        currentIndex = remap.indices.contains(currentIndex) ? (remap[currentIndex] ?? -1) : -1
        syncPolicy(oldToNew: remap)
        if queue.isEmpty {
            removePlayerObservers(); player?.pause(); player = nil; assetLoader = nil
            #if KOG_NATIVE_AUDIO
            nativeGeneration += 1; nativeStartTask?.cancel()
            nativePlayer?.stop(); nativePlayer = nil
            #endif
            currentIndex = -1; playing = false; position = 0; duration = 0
            nowPlayingArtTask?.cancel(); nowPlayingArtwork = nil
            MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        }

    }

    func move(_ source: IndexSet, to destination: Int) {
        var ordering = Array(queue.indices)
        ordering.move(fromOffsets: source, toOffset: destination)
        let nextIndex = ordering.firstIndex(of: currentIndex) ?? currentIndex
        queue.move(fromOffsets: source, toOffset: destination)
        currentIndex = nextIndex
        syncPolicy(oldToNew: ordering.indices.map { ordering.firstIndex(of: $0) })
    }

    func clearQueue() {
        workspaceQueueGeneration += 1
        workspaceQueueTask = nil
        stop()
        removePlayerObservers(); player?.pause(); player = nil; assetLoader = nil
        #if KOG_NATIVE_AUDIO
        nativeGeneration += 1; nativeStartTask?.cancel()
        nativePlayer?.stop(); nativePlayer = nil
        #endif
        queue = []; currentIndex = -1; playing = false; position = 0; duration = 0
        syncPolicy()
        nowPlayingArtTask?.cancel(); nowPlayingArtwork = nil
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
    }

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
    func filteredQueueIndices() -> [Int] {
        guard !queueFilter.isEmpty else { return Array(queue.indices) }
        do {
            return try policy.send(["op": "filter_rows", "rows": policyRows(), "query": queueFilter])["indices"] as? [Int] ?? []
        } catch { return [] }
    }

    func sortQueue(_ key: String) {
        if sortKey == key { sortDescending.toggle() } else { sortKey = key; sortDescending = false }
        let selectedIndex = currentIndex
        let rows = policyRows()
        guard let ordering = policyCommand(["op": "sort_rows", "rows": rows, "column": key, "descending": sortDescending])?["indices"] as? [Int] else { return }
        queue = ordering.map { queue[$0] }
        if selectedIndex >= 0 { currentIndex = ordering.firstIndex(of: selectedIndex) ?? -1 }
        syncPolicy(oldToNew: ordering.indices.map { ordering.firstIndex(of: $0) })
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
            if selectedPlaylist?.id == 0 { playlistTracks.removeAll { $0.id == track.id && !enabled } }
        } catch { report(error) }
    }

    func toggleRadio() async {
        radio.toggle()
        prepareRadio(selectSource: true)
    }
    private func prepareRadio(selectSource: Bool = false, reshuffle: Bool = false) {
        radioTask?.cancel()
        if radio && selectSource { radioOnDevice = libraryOnDevice }
        _ = policyCommand(["op": "radio_reset", "enabled": radio, "current": SharedPlaybackPolicy.index(currentIndex)])
        requestRadio(initial: true, reshuffle: reshuffle)
    }
    private func requestRadio(initial: Bool = false, reshuffle: Bool = false) {
        guard initial || policy.needsRefill else { return }
        _ = policyCommand(["op": "radio_begin"])
        let generation = policy.generation, enabled = radio
        let client = radioOnDevice ? deviceAPI : api
        let root = radioOnDevice ? activeDeviceRoot : activeTreeRoot
        radioTask = Task {
            do {
                let response: RadioBatch
                if reshuffle { response = try await client.reshuffleRadio(root: root) }
                else if initial { response = try await client.radio(enabled, root: root) }
                else { response = try await client.radioAdvance(root: root) }
                guard !Task.isCancelled, policy.generation == generation else { return }
                let entries = try JSONSerialization.jsonObject(with: JSONEncoder().encode(response.tracks))
                _ = policyCommand(["op": "radio_accept", "generation": generation, "entries": entries, "exhausted": response.exhausted])
                consumeRadio("radio_pending")
                requestRadio()
            } catch {
                guard !Task.isCancelled, policy.generation == generation else { return }
                _ = policyCommand(["op": "radio_fail", "generation": generation])
                report(error)
            }
        }
    }
    func reshuffleRadio() async {
        radio = true
        prepareRadio(reshuffle: true)
    }
    private func consumeRadio(_ operation: String) {
        guard let reply = policyCommand(["op": operation]) else { return }
        if let entry = reply["entry"] as? [String: Any],
           let data = try? JSONSerialization.data(withJSONObject: entry),
           let track = try? JSONDecoder().decode(Track.self, from: data) {
            add([track])
            _ = policyCommand(["op": "radio_candidate", "index": queue.count - 1])
            activateIndex(queue.count - 1)
        } else if operation == "radio_next", !policy.waiting { stop() }
    }
    private func advanceRadio() {
        consumeRadio("radio_next")
        requestRadio()
    }
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
            let onDevice = workspace.activeTab?.scope == "device"
            guard tracks.allSatisfy({ $0.kind == "remote" || $0.isDevice == onDevice }) else { throw KogError.response("Choose a playlist in the same library as these tracks.") }
            let entries = try JSONSerialization.jsonObject(with: JSONEncoder().encode(tracks))
            workspaceCommand(["op": "append", "entries": entries])
        } catch { report(error) }
    }
    func workspaceSelect(_ index: Int) {
        workspaceCommand(["op": "selection", "command": ["op": "choose", "index": index, "gesture": "toggle"]])
    }
    func selectQueue(_ command: [String: Any]) {
        _ = policyCommand(["op": "select_queue", "command": command])
    }

    func workspaceCommand(_ command: [String: Any]) {
        guard let reply = policyCommand(["op": "workspace", "command": command], sync: false),
              let effect = reply["workspace_effect"] as? [String: Any], let action = effect["action"] as? String else { return }
        switch action {
        case "load":
            let key = effect["key"] as? String ?? "", generation = effect["generation"] as? Int ?? 0
            Task {
                do {
                    let client = try workspaceAPI(effect["scope"] as? String ?? "")
                    let tracks = try await client.playlist((effect["playlist_id"] as? NSNumber)?.int64Value ?? 0)
                    let entries = try JSONSerialization.jsonObject(with: JSONEncoder().encode(tracks))
                    workspaceCommand(["op": "loaded", "key": key, "generation": generation, "entries": entries])
                } catch { workspaceCommand(["op": "load_failed", "key": key, "generation": generation, "error": error.localizedDescription]) }
            }
        case "save":
            let key = effect["key"] as? String ?? "", revision = effect["revision"] as? Int ?? 0
            Task {
                do {
                    let client = try workspaceAPI(effect["scope"] as? String ?? "")
                    try await client.replacePlaylist((effect["playlist_id"] as? NSNumber)?.int64Value ?? 0,
                        tracks: workspaceTracks(effect["entries"]), expected: workspaceTracks(effect["expected_entries"]))
                    workspaceCommand(["op": "saved", "key": key, "revision": revision]); await loadPlaylists()
                } catch { workspaceCommand(["op": "save_failed", "key": key, "revision": revision, "error": error.localizedDescription]) }
            }
        case "queue":
            let generation = workspaceQueueGeneration
            let source = effect["scope"] as? String ?? ""
            let previous = workspaceQueueTask
            workspaceQueueTask = Task {
                await previous?.value
                guard generation == workspaceQueueGeneration else { return }
                do {
                    let client = try workspaceAPI(source)
                    let selected = try workspaceTracks(effect["entries"])
                    var expanded = [Track]()
                    for track in selected { expanded += try await client.expand(track) }
                    guard generation == workspaceQueueGeneration, source == "device" || source == "server:\(serverKey)" else { return }
                    let start = queue.count; add(expanded)
                    let reply = policyCommand(["op": "apply_queue_action", "action": effect["mode"] as? String ?? "add_to_queue", "start": start, "count": expanded.count])
                    if let decision = reply?["decision"] as? [String: Any], let index = decision["index"] as? Int { playIndex(index) }
                } catch { report(error) }
            }
        default: break
        }
    }

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
            let id = try await playlistAPI.createPlaylist(name)
            if saveQueue && !queue.isEmpty { try await playlistAPI.appendPlaylist(id, tracks: queue) }
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
            Task { @MainActor [weak self] in if self?.playing == true || self?.policy.waiting == true { self?.togglePlayback() } }
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
        codec = value; UserDefaults.standard.set(value, forKey: "codec")
        if current?.isDevice == false { startPlayer(resumeAt: position, shouldPlay: playing) }
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
        UserDefaults.standard.set(deviceTreeRoot, forKey: "device_tree_root")
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
        UserDefaults.standard.set(notifyTracks, forKey: "track_notifications")
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
                        self.startPlayer(resumeAt: self.position)
                    }
                }
            }
        })
        audioObservers.append(NotificationCenter.default.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] note in
            let reason = (note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt) ?? 0
            if reason == AVAudioSession.RouteChangeReason.oldDeviceUnavailable.rawValue {
                Task { @MainActor [weak self] in if self?.playing == true || self?.policy.waiting == true { self?.togglePlayback() } }
            }
        })
        audioObservers.append(NotificationCenter.default.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self else { return }
                try? AVAudioSession.sharedInstance().setCategory(.playback)
                try? AVAudioSession.sharedInstance().setActive(true)
                if self.current != nil { self.startPlayer(resumeAt: self.position, shouldPlay: self.playing) }
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
