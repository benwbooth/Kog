import AVFoundation
import Combine
import MediaPlayer
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class KogStore: ObservableObject {
    @Published var server = UserDefaults.standard.string(forKey: "server") ?? ""
    @Published var username = UserDefaults.standard.string(forKey: "username") ?? ""
    @Published var token = Secrets.read("token")
    @Published var password = Secrets.read("password")
    @Published var codec = UserDefaults.standard.string(forKey: "codec") ?? "aac"
    @Published var connected = false
    @Published var listing: Listing?
    @Published var libraryRoot = ""
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
    @Published var shuffle = UserDefaults.standard.bool(forKey: "shuffle") {
        didSet { UserDefaults.standard.set(shuffle, forKey: "shuffle") }
    }
    @Published var repeatQueue = UserDefaults.standard.bool(forKey: "repeat") {
        didSet { UserDefaults.standard.set(repeatQueue, forKey: "repeat") }
    }
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

    private var player: AVPlayer?
    private var assetLoader: AuthenticatedAssetLoader?
    #if KOG_NATIVE_AUDIO
    private var nativePlayer: NativeAudioPlayer?
    private var nativeTimer: Timer?
    #endif
    private var timeObserver: Any?
    private var finishObserver: NSObjectProtocol?
    private var statusObserver: NSKeyValueObservation?
    private var searchTask: Task<Void, Never>?
    private var importScanTask: Task<Void, Never>?
    private var shuffleHistory = [Int]()
    private var suppressSave = false
    private var nowPlayingArtTask: Task<Void, Never>?
    private var nowPlayingArtwork: MPMediaItemArtwork?

    var current: Track? { queue.indices.contains(currentIndex) ? queue[currentIndex] : nil }
    var api: KogAPI { KogAPI(server: server, token: token, username: username, password: password, codec: codec) }
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
        scanImports()
        do {
            try AVAudioSession.sharedInstance().setCategory(.playback, mode: .default)
            try AVAudioSession.sharedInstance().setActive(true)
        } catch { self.error = "Audio session: \(error.localizedDescription)" }
        registerRemoteCommands()
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

    private func saveQueue() {
        guard !suppressSave else { return }
        UserDefaults.standard.set(try? JSONEncoder().encode(queue), forKey: "queue")
        UserDefaults.standard.set(currentIndex, forKey: "index")
        UserDefaults.standard.set(shuffle, forKey: "shuffle")
        UserDefaults.standard.set(repeatQueue, forKey: "repeat")
    }

    private func report(_ failure: Error) { error = failure.localizedDescription }

    func refresh() async {
        guard !server.isEmpty else { connected = false; return }
        do {
            let client = api
            try await client.health()
            let listing = try await client.browse()
            self.listing = listing
            libraryRoot = listing.path
            playlists = try await client.playlists()
            stars = try await client.stars()
            connected = true
            error = nil
        } catch { connected = false; report(error) }
    }

    func browse(_ path: String) async {
        do {
            listing = try await api.browse(path)
            searchText = ""; searchTracks = []; searchFolders = []
        } catch { report(error) }
    }

    func search(_ text: String) {
        searchText = text
        searchTask?.cancel()
        if text.isEmpty { searchTracks = []; searchFolders = []; searching = false; return }
        searchTask = Task {
            do {
                try await Task.sleep(for: .milliseconds(250))
                searching = true
                let client = api
                var page = try await client.search(text)
                guard !Task.isCancelled else { return }
                searchFolders = page.results.filter { $0.is_dir == true }.map(\.folder)
                searchTracks = page.results.filter { $0.is_dir != true }.map(\.track)
                searchScanned = page.scanned
                while !page.done && !Task.isCancelled {
                    try await Task.sleep(for: .milliseconds(180))
                    page = try await client.more(page.generation, offset: searchFolders.count + searchTracks.count)
                    guard !Task.isCancelled else { break }
                    searchFolders += page.results.filter { $0.is_dir == true }.map(\.folder)
                    searchTracks += page.results.filter { $0.is_dir != true }.map(\.track)
                    searchScanned = page.scanned
                }
            } catch is CancellationError {} catch { report(error) }
            if searchText == text { searching = false }
        }
    }

    func addFile(_ track: Track, play: Bool = false) async {
        do {
            if track.isDevice {
                #if KOG_NATIVE_AUDIO
                var tracks = try await Task.detached(priority: .userInitiated) {
                    try NativeAudioCatalog.expand(path: track.path)
                }.value
                if tracks.count == 1 {
                    if tracks[0].title.isEmpty { tracks[0].title = track.title }
                    if tracks[0].artist.isEmpty { tracks[0].artist = track.artist }
                    if tracks[0].album.isEmpty { tracks[0].album = track.album }
                }
                add(tracks, play: play)
                #endif
            } else { add(try await api.expand(track), play: play) }
        } catch { report(error) }
    }

    func addDeviceFolder(_ folder: Folder) async {
        #if KOG_NATIVE_AUDIO
        do {
            let tracks = try await Task.detached(priority: .userInitiated) {
                try NativeAudioCatalog.expand(path: folder.path)
            }.value
            add(tracks)
        } catch { report(error) }
        #endif
    }

    func addFolder(_ folder: Folder, play: Bool = false) async {
        do { add(try await api.collect(folder.path), play: play) }
        catch { report(error) }
    }

    func add(_ tracks: [Track], play: Bool = false) {
        guard !tracks.isEmpty else { return }
        let start = queue.count
        queue += tracks
        if play { playIndex(start) }
    }

    func playIndex(_ index: Int) {
        guard queue.indices.contains(index) else { return }
        currentIndex = index
        startPlayer()
    }

    private func startPlayer() {
        guard let track = current else { return }
        do {
            removePlayerObservers()
            player?.pause()
            player = nil
            assetLoader = nil
            #if KOG_NATIVE_AUDIO
            nativePlayer?.stop()
            nativePlayer = nil
            if NativeAudioPlayer.useFor(track) {
                let decoder = try NativeAudioPlayer(path: track.path,
                    subsong: Int32(track.fragment) ?? -1, onEnd: { [weak self] in
                    Task { @MainActor [weak self] in self?.next() }
                }, onError: { [weak self] message in
                    Task { @MainActor [weak self] in self?.error = message; self?.playing = false }
                })
                nativePlayer = decoder
                duration = decoder.duration
                position = 0
                decoder.play()
                playing = true
                nativeTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
                    Task { @MainActor [weak self] in
                        guard let self, let decoder = self.nativePlayer else { return }
                        self.position = decoder.position
                        self.updateNowPlaying()
                    }
                }
                loadNowPlayingArt(for: track)
                updateNowPlaying()
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
            player = AVPlayer(playerItem: item)
            statusObserver = item.observe(\.status, options: [.new]) { [weak self] item, _ in
                if item.status == .failed {
                    Task { @MainActor [weak self] in
                        self?.error = item.error?.localizedDescription ?? "Cannot play this file"
                        self?.playing = false
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
                Task { @MainActor [weak self] in self?.next() }
            }
            player?.play()
            playing = true
            position = 0
            loadNowPlayingArt(for: track)
            updateNowPlaying()
        } catch { report(error) }
    }

    private func removePlayerObservers() {
        if let observer = timeObserver { player?.removeTimeObserver(observer); timeObserver = nil }
        if let observer = finishObserver { NotificationCenter.default.removeObserver(observer); finishObserver = nil }
        statusObserver = nil
        #if KOG_NATIVE_AUDIO
        nativeTimer?.invalidate(); nativeTimer = nil
        #endif
    }

    func togglePlayback() {
        if playing {
            player?.pause()
            #if KOG_NATIVE_AUDIO
            nativePlayer?.pause()
            #endif
            playing = false
        }
        else {
            if player != nil { player?.play(); playing = true }
            #if KOG_NATIVE_AUDIO
            if nativePlayer != nil { nativePlayer?.play(); playing = true }
            #endif
            if !playing && !queue.isEmpty { playIndex(max(0, currentIndex)) }
        }
        updateNowPlaying()
    }

    func stop() {
        player?.pause()
        #if KOG_NATIVE_AUDIO
        nativePlayer?.pause()
        #endif
        playing = false; seek(0); updateNowPlaying()
    }
    func seek(_ seconds: Double) {
        guard seconds.isFinite else { return }
        player?.seek(to: CMTime(seconds: max(0, seconds), preferredTimescale: 600))
        #if KOG_NATIVE_AUDIO
        nativePlayer?.seek(seconds)
        #endif
        position = max(0, seconds)
        updateNowPlaying()
    }

    func next() {
        guard !queue.isEmpty else { return }
        if shuffle && queue.count > 1 {
            shuffleHistory.append(currentIndex)
            let candidates = queue.indices.filter { $0 != currentIndex }
            playIndex(candidates.randomElement() ?? 0)
        } else if currentIndex + 1 < queue.count { playIndex(currentIndex + 1) }
        else if repeatQueue { playIndex(0) }
        else if radio { Task { await advanceRadio() } }
        else { stop() }
    }

    func previous() {
        if position > 3 { seek(0); return }
        if shuffle, let index = shuffleHistory.popLast() { playIndex(index) }
        else { playIndex(max(0, currentIndex - 1)) }
    }

    func remove(_ offsets: IndexSet) {
        let removedCurrent = offsets.contains(currentIndex)
        let removedBefore = offsets.filter { $0 < currentIndex }.count
        queue.remove(atOffsets: offsets)
        if queue.isEmpty {
            removePlayerObservers(); player?.pause(); player = nil; assetLoader = nil
            #if KOG_NATIVE_AUDIO
            nativePlayer?.stop(); nativePlayer = nil
            #endif
            currentIndex = -1; playing = false; position = 0; duration = 0
            shuffleHistory = []
            nowPlayingArtTask?.cancel(); nowPlayingArtwork = nil
            MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        }
        else if removedCurrent { playIndex(min(max(0, currentIndex - removedBefore), queue.count - 1)) }
        else { currentIndex -= removedBefore }
    }

    func move(_ source: IndexSet, to destination: Int) {
        var ordering = Array(queue.indices)
        ordering.move(fromOffsets: source, toOffset: destination)
        let nextIndex = ordering.firstIndex(of: currentIndex) ?? currentIndex
        queue.move(fromOffsets: source, toOffset: destination)
        currentIndex = nextIndex
    }

    func clearQueue() {
        removePlayerObservers(); player?.pause(); player = nil; assetLoader = nil
        #if KOG_NATIVE_AUDIO
        nativePlayer?.stop(); nativePlayer = nil
        #endif
        queue = []; currentIndex = -1; playing = false; position = 0; duration = 0
        shuffleHistory = []
        nowPlayingArtTask?.cancel(); nowPlayingArtwork = nil
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
    }

    func sortQueue(_ key: String) {
        let selectedIndex = currentIndex
        let ordered = queue.enumerated().sorted {
            let lhs: String; let rhs: String
            switch key {
            case "Artist": lhs = $0.element.artist; rhs = $1.element.artist
            case "Album": lhs = $0.element.album; rhs = $1.element.album
            default: lhs = $0.element.label; rhs = $1.element.label
            }
            let comparison = lhs.localizedStandardCompare(rhs)
            return comparison == .orderedSame ? $0.offset < $1.offset : comparison == .orderedAscending
        }
        queue = ordered.map(\.element)
        if selectedIndex >= 0 { currentIndex = ordered.firstIndex { $0.offset == selectedIndex } ?? selectedIndex }
    }

    func toggleStar(_ track: Track) async {
        guard !track.isDevice else { return }
        do {
            let enabled = !stars.contains(track.id)
            try await api.star(track, enabled: enabled)
            if enabled { stars.insert(track.id) } else { stars.remove(track.id) }
        } catch { report(error) }
    }

    func toggleRadio() async {
        do {
            let enabled = !radio
            let tracks = try await api.radio(enabled, root: libraryRoot)
            radio = enabled
            if enabled { add(tracks, play: queue.isEmpty) }
        } catch { report(error) }
    }

    private func advanceRadio() async {
        do {
            let tracks = try await api.radioAdvance(root: libraryRoot)
            if !tracks.isEmpty { add(tracks, play: true) }
            else { stop() }
        } catch { report(error); stop() }
    }

    func loadPlaylists() async {
        guard connected else { return }
        do { playlists = try await api.playlists() } catch { report(error) }
    }
    func openPlaylist(_ playlist: SavedPlaylist) async {
        do { playlistTracks = try await api.playlist(playlist.id); selectedPlaylist = playlist }
        catch { report(error) }
    }
    func createPlaylist(_ name: String) async {
        do { try await api.createPlaylist(name); await loadPlaylists() } catch { report(error) }
    }
    func renamePlaylist(_ playlist: SavedPlaylist, name: String) async {
        do { try await api.renamePlaylist(playlist.id, name: name); await loadPlaylists() } catch { report(error) }
    }
    func deletePlaylist(_ playlist: SavedPlaylist) async {
        do { try await api.deletePlaylist(playlist.id); selectedPlaylist = nil; await loadPlaylists() }
        catch { report(error) }
    }
    func appendToPlaylist(_ playlist: SavedPlaylist, tracks: [Track]) async {
        do { try await api.appendPlaylist(playlist.id, tracks: tracks); await loadPlaylists() }
        catch { report(error) }
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

    func deleteDeviceFile(_ track: Track) {
        do {
            try FileManager.default.removeItem(atPath: track.path)
            let removed = IndexSet(queue.indices.filter { queue[$0].isDevice && queue[$0].path == track.path })
            if !removed.isEmpty { remove(removed) }
            scanImports()
        } catch { report(error) }
    }

    func scanImports() {
        browseDevice(devicePath.isEmpty ? importsURL.path : devicePath)
    }

    func browseDevice(_ path: String) {
        #if KOG_NATIVE_AUDIO
        importScanTask?.cancel()
        importing = true
        let root = importsURL
        importScanTask = Task {
            do {
                let listing = try await Task.detached(priority: .userInitiated) {
                    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
                    return try NativeAudioCatalog.browse(root: root.path, path: path)
                }.value
                guard !Task.isCancelled else { return }
                var resolved = [Track]()
                for var track in listing.files {
                    if Task.isCancelled { return }
                    // AVFoundation handles common local tags. An archive member
                    // is probed through Rust only when selected for the queue.
                    if !track.path.hasPrefix("kog-archive:") {
                        let asset = AVURLAsset(url: URL(fileURLWithPath: track.path))
                        if let items = try? await asset.load(.commonMetadata) {
                            for item in items {
                                switch item.commonKey {
                                case .commonKeyTitle: track.title = (try? await item.load(.stringValue)) ?? ""
                                case .commonKeyArtist: track.artist = (try? await item.load(.stringValue)) ?? ""
                                case .commonKeyAlbumName: track.album = (try? await item.load(.stringValue)) ?? ""
                                default: break
                                }
                            }
                        }
                        if let length = try? await asset.load(.duration), length.seconds.isFinite {
                            track.duration = Int64(max(0, length.seconds) * 1000)
                        }
                    }
                    resolved.append(track)
                }
                guard !Task.isCancelled else { return }
                deviceListing = listing
                devicePath = path
                deviceFiles = resolved
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
        commands.playCommand.addTarget { [weak self] _ in
            Task { @MainActor [weak self] in if self?.playing == false { self?.togglePlayback() } }
            return .success
        }
        commands.pauseCommand.addTarget { [weak self] _ in
            Task { @MainActor [weak self] in if self?.playing == true { self?.togglePlayback() } }
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
