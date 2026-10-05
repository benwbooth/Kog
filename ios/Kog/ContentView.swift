import MediaPlayer
import SwiftUI
import UniformTypeIdentifiers

enum Palette {
    static let window = Color(red: 0.106, green: 0.118, blue: 0.125)
    static let panel = Color(red: 0.137, green: 0.153, blue: 0.165)
    static let raised = Color(red: 0.169, green: 0.188, blue: 0.204)
    static let border = Color(red: 0.204, green: 0.224, blue: 0.239)
    static let muted = Color(red: 0.604, green: 0.627, blue: 0.651)
    static let accent = Color(red: 0.239, green: 0.682, blue: 0.914)
}

private enum Tab: String, CaseIterable { case library = "Library", queue = "Queue", playlists = "Playlists" }

struct ContentView: View {
    @EnvironmentObject private var store: KogStore
    @Environment(\.scenePhase) private var scenePhase
    @State private var tab: Tab = .queue
    @State private var showSettings = false
    @State private var showPlayer = false
    @State private var showFilePicker = false
    @State private var showFolderPicker = false
    @State private var showSoundfontPicker = false
    @State private var showSc55Picker = false
    @State private var showMt32Picker = false
    @State private var showCreatePlaylist = false
    @State private var newPlaylistName = ""
    @State private var renameTarget: SavedPlaylist?
    @State private var renamedName = ""
    private var deviceMode: Bool { get { store.libraryOnDevice } nonmutating set { store.libraryOnDevice = newValue } }
    @State private var showTreeRootPicker = false
    @State private var pickRootAfterSettings = false
    @State private var pendingPicker = ""
    @State private var detailsTrack: Track?
    @State private var deletingPlaylist: SavedPlaylist?
    @State private var shareURL: URL?
    @State private var showURL = false
    @State private var urlText = ""
    @State private var showAbout = false
    @State private var confirmEditClear = false

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(Palette.border).frame(height: 1)
            Group {
                switch tab {
                case .library: library
                case .queue: QueueView(openLibrary: { tab = .library }, showDetails: { detailsTrack = $0 })
                case .playlists: playlists
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            miniPlayer
            tabs
        }
        .background(Palette.window.ignoresSafeArea())
        .tint(Palette.accent)
        .overlay(alignment: .top) {
            if let notice = store.downloadNotice {
                Label(notice, systemImage: "checkmark.circle.fill")
                    .font(.subheadline).lineLimit(2)
                    .padding(.horizontal, 14).padding(.vertical, 10)
                    .background(Palette.raised, in: RoundedRectangle(cornerRadius: 12))
                    .padding(.horizontal, 16).padding(.top, 60)
            }
        }
        .task(id: scenePhase) {
            guard scenePhase == .active else { return }
            while !Task.isCancelled {
                await store.checkConnection()
                do { try await Task.sleep(for: .seconds(10)) }
                catch { return }
            }
        }
        .sheet(isPresented: $showSettings, onDismiss: {
            if pickRootAfterSettings {
                pickRootAfterSettings = false
                showTreeRootPicker = true
            }
            switch pendingPicker {
            case "files": showFilePicker = true
            case "folder": showFolderPicker = true
            case "soundfont": showSoundfontPicker = true
            case "sc55": showSc55Picker = true
            case "mt32": showMt32Picker = true
            default: break
            }
            pendingPicker = ""
        }) { settings }
        .sheet(isPresented: $showTreeRootPicker) {
            NavigationStack { ServerRootPicker(store: store) }
        }
        .sheet(isPresented: $showPlayer) { NowPlayingView() }
        .sheet(item: $detailsTrack) { TrackDetailsView(track: $0) }
        .sheet(isPresented: Binding(get: { shareURL != nil }, set: { if !$0 { shareURL = nil } })) {
            if let url = shareURL { ShareSheet(items: [url]) }
        }
        .sheet(isPresented: $showAbout) { AboutView() }
        .onOpenURL { url in if url.isFileURL { Task { await store.importFiles([url]); deviceMode = true; tab = .library } } }
        .alert("Add music URL", isPresented: $showURL) {
            TextField("https://…", text: $urlText).keyboardType(.URL).textInputAutocapitalization(.never)
            Button("Add") { let text = urlText; Task { await store.addURL(text) }; urlText = "" }
            Button("Cancel", role: .cancel) { urlText = "" }
        }
        .confirmationDialog("Delete this playlist?", isPresented: Binding(get: { deletingPlaylist != nil }, set: { if !$0 { deletingPlaylist = nil } }), titleVisibility: .visible) {
            if let playlist = deletingPlaylist {
                Button("Delete \(playlist.name)", role: .destructive) { Task { await store.deletePlaylist(playlist) }; deletingPlaylist = nil }
            }
        }
        .fileImporter(isPresented: $showFilePicker, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
            if case .success(let urls) = result { Task { await store.importFiles(urls) } }
            else if case .failure(let error) = result { store.error = error.localizedDescription }
        }
        .fileImporter(isPresented: $showFolderPicker, allowedContentTypes: [.folder]) { result in
            if case .success(let url) = result { Task { await store.importFiles([url]) } }
            else if case .failure(let error) = result { store.error = error.localizedDescription }
        }
        .fileImporter(isPresented: $showSoundfontPicker, allowedContentTypes: [.item]) { result in
            if case .success(let url) = result { Task { await store.importMidiAsset(url, kind: "soundfont") } }
            else if case .failure(let error) = result { store.error = error.localizedDescription }
        }
        .fileImporter(isPresented: $showSc55Picker, allowedContentTypes: [.folder]) { result in
            if case .success(let url) = result { Task { await store.importMidiAsset(url, kind: "sc55") } }
            else if case .failure(let error) = result { store.error = error.localizedDescription }
        }
        .fileImporter(isPresented: $showMt32Picker, allowedContentTypes: [.folder]) { result in
            if case .success(let url) = result { Task { await store.importMidiAsset(url, kind: "mt32") } }
            else if case .failure(let error) = result { store.error = error.localizedDescription }
        }
        .alert("New playlist", isPresented: $showCreatePlaylist) {
            TextField("Playlist name", text: $newPlaylistName)
            Button("Create") {
                let name = newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines)
                if !name.isEmpty { Task { await store.createPlaylist(name) } }
                newPlaylistName = ""
            }
            Button("Cancel", role: .cancel) { newPlaylistName = "" }
        }
        .alert("Rename playlist", isPresented: Binding(get: { renameTarget != nil }, set: { if !$0 { renameTarget = nil } })) {
            TextField("Playlist name", text: $renamedName)
            Button("Save") {
                if let target = renameTarget {
                    let name = renamedName.trimmingCharacters(in: .whitespacesAndNewlines)
                    if !name.isEmpty { Task { await store.renamePlaylist(target, name: name) } }
                }
                renameTarget = nil
            }
            Button("Cancel", role: .cancel) { renameTarget = nil }
        }
        .alert("Kog", isPresented: Binding(get: { store.error != nil }, set: { if !$0 { store.error = nil } })) {
            Button("OK") { store.error = nil }
        } message: { Text(store.error ?? "") }
        .confirmationDialog("Clear all tracks from the queue?", isPresented: $confirmEditClear, titleVisibility: .visible) {
            Button("Clear Play Queue", role: .destructive) { store.workspaceCommand(["op": "clear"]) }
        }
    }

    private var header: some View {
        HStack(spacing: 6) {
            if tab == .playlists && store.selectedPlaylist != nil {
                Button { store.selectedPlaylist = nil } label: { Image(systemName: "chevron.left").frame(width: 36, height: 44) }
            }
            Text(tab == .playlists ? (store.selectedPlaylist?.name ?? "Playlists") : tab.rawValue)
                .font(.title3.bold()).lineLimit(1)
            Spacer()
            if store.pendingAdds > 0 || store.radioBusy { ProgressView().accessibilityLabel("Preparing tracks") }
            Menu {
                Button("Add URL…", systemImage: "link") { showURL = true }
                Button("Import files…", systemImage: "square.and.arrow.down") { showFilePicker = true }
                Button("Import folder…", systemImage: "folder.badge.plus") { showFolderPicker = true }
                Divider()
                Menu("Edit", systemImage: "pencil") {
                    PlaylistEditCommands {
                        if store.workspace.active == "queue" { confirmEditClear = true }
                        else { store.workspaceCommand(["op": "clear"]) }
                    }
                }.disabled(tab != .queue)
                Divider()
                Button(store.playing ? "Pause" : "Play", systemImage: store.playing ? "pause.fill" : "play.fill") { store.togglePlayback() }
                Button("Stop", systemImage: "stop.fill") { store.stop() }
                Button("Previous", systemImage: "backward.end.fill") { store.previous() }
                Button("Next", systemImage: "forward.end.fill") { store.next() }
                Picker("Shuffle", selection: Binding(get: { store.shuffle }, set: { store.selectShuffle($0) })) { ForEach(ShuffleMode.allCases) { Text($0.label).tag($0) } }
                Picker("Repeat", selection: Binding(get: { store.repeatMode }, set: { store.selectRepeat($0) })) { ForEach(RepeatMode.allCases) { Text($0.label).tag($0) } }
                Button { Task { await store.toggleRadio() } } label: {
                    Label(store.radio ? "Turn Random Radio off" : "Random Radio", systemImage: store.radio ? "checkmark" : "die.face.5")
                }
                if store.radio { Button("Reshuffle radio", systemImage: "shuffle") { Task { await store.reshuffleRadio() } } }
                Divider()
                Button("About Kog", systemImage: "info.circle") { showAbout = true }
            } label: { Image(systemName: "line.3.horizontal").frame(width: 44, height: 44) }.accessibilityLabel("Player menu")
            if tab == .playlists && store.selectedPlaylist == nil {
                Button { showCreatePlaylist = true } label: { Image(systemName: "plus").frame(width: 44, height: 44) }
                    .accessibilityLabel("Create playlist")
            }
            if tab == .playlists, let playlist = store.selectedPlaylist, playlist.id != 0 {
                Menu { playlistActions(playlist) } label: { Image(systemName: "ellipsis").frame(width: 44, height: 44) }
                    .accessibilityLabel("Playlist actions")
            }

            Button { showSettings = true } label: {
                Image(systemName: "gearshape.fill")
                    .foregroundStyle(store.connected ? Palette.accent : Palette.muted)
                    .frame(width: 44, height: 44)
            }.accessibilityLabel("Settings")
        }
        .padding(.horizontal, 12).frame(height: 54).background(Palette.panel)
    }

    private var tabs: some View {
        HStack(spacing: 0) {
            tabButton(.library, symbol: "folder")
            tabButton(.queue, symbol: "music.note.list")
            tabButton(.playlists, symbol: "list.bullet.rectangle")
        }
        .frame(height: 58).background(Palette.panel)
        .overlay(alignment: .top) { Rectangle().fill(Palette.border).frame(height: 1) }
    }

    private func tabButton(_ target: Tab, symbol: String) -> some View {
        Button {
            tab = target
            if target == .playlists { Task { await store.loadPlaylists() } }
        } label: {
            VStack(spacing: 3) {
                Image(systemName: symbol).font(.system(size: 20, weight: .medium))
                Text(target.rawValue).font(.caption2.weight(.semibold))
            }
            .foregroundStyle(tab == target ? Palette.accent : Palette.muted)
            .frame(maxWidth: .infinity).frame(height: 54)
            .contentShape(Rectangle())
        }.buttonStyle(.plain)
    }

    private var library: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                modeButton("Server", selected: !deviceMode) { deviceMode = false }
                modeButton("On this device", selected: deviceMode) { deviceMode = true; store.scanImports() }
                Spacer()
                if store.importing { ProgressView().padding(.trailing, 8) }
            }
            .padding(.horizontal, 12).frame(height: 42).background(Palette.panel)
            if deviceMode { deviceLibrary }
            else { serverLibrary }
        }
    }

    private func modeButton(_ title: String, selected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) { Text(title).font(.subheadline.weight(selected ? .semibold : .regular))
            .foregroundStyle(selected ? Palette.accent : Palette.muted).frame(minHeight: 42) }
    }

    private var serverLibrary: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass").foregroundStyle(Palette.muted)
                TextField("Search files and folders", text: Binding(get: { store.searchText }, set: store.search))
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                if !store.searchText.isEmpty {
                    Button { store.search("") } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(Palette.muted) }
                        .accessibilityLabel("Clear search")
                }
            }
            .padding(.horizontal, 12).frame(height: 40).background(Palette.raised, in: RoundedRectangle(cornerRadius: 10))
            .padding(.horizontal, 12).padding(.vertical, 8)
            HStack(spacing: 6) {
                if store.searchText.isEmpty, let listing = store.listing,
                   !listing.parent.isEmpty && listing.path != store.activeTreeRoot {
                    Button { Task { await store.browse(listing.parent) } } label: {
                        Image(systemName: "chevron.left").frame(width: 38, height: 40)
                    }.accessibilityLabel("Parent folder")
                }
                Text(store.searchText.isEmpty ? (store.listing?.path.components(separatedBy: "/").last ?? "Library") : "Search results")
                    .lineLimit(1).font(.caption.weight(.semibold)).foregroundStyle(Palette.muted)
                Spacer()
                if store.searching {
                    Button { Task { await store.toggleSearchPause() } } label: {
                        Label("\(store.searchScanned) scanned", systemImage: store.searchPaused ? "play.fill" : "pause.fill").font(.caption2)
                    }.frame(minHeight: 44).accessibilityLabel(store.searchPaused ? "Resume search" : "Pause search")
                }
                Menu {
                    Button("Choose tree root…", systemImage: "folder.badge.gearshape") { showTreeRootPicker = true }
                    if let listing = store.listing, !listing.isArchive {
                        Button("Use this folder as tree root", systemImage: "folder.badge.checkmark") {
                            Task { await store.setTreeRoot(listing.path) }
                        }
                    }
                    if !store.treeRoot.isEmpty {
                        Button("Reset to server library", systemImage: "arrow.uturn.backward") {
                            Task { await store.setTreeRoot("") }
                        }
                    }
                } label: { Image(systemName: "folder.badge.gearshape").frame(width: 44, height: 42) }
                    .accessibilityLabel("Library tree root")
                Button { Task { await store.browse(store.listing?.path ?? store.activeTreeRoot) } } label: {
                    Image(systemName: "arrow.clockwise").frame(width: 44, height: 44)
                }.accessibilityLabel("Refresh folder")
            }
            .padding(.horizontal, 10).frame(height: 42).background(Palette.panel)
            if store.server.isEmpty && store.listing == nil {
                emptyView("Connect to a Kog server", detail: "You can also play files stored on this iPhone.") {
                    deviceMode = true
                }
            } else {
                let folders = store.searchText.isEmpty ? (store.listing?.directories ?? []) : store.searchFolders
                let files = store.searchText.isEmpty ? (store.listing?.files ?? []) : store.searchTracks
                List {
                    ForEach(folders) { folder in
                        HStack(spacing: 10) {
                            Button { Task { await store.browse(folder.path) } } label: {
                                Image("kog_folder").resizable().scaledToFit().frame(width: 23, height: 23)
                                Text(folder.name).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                            }.buttonStyle(.plain)
                            Button { Task { await store.addFolder(folder) } } label: { Image(systemName: "plus").frame(width: 44, height: 44) }
                                .accessibilityLabel("Add folder to queue")
                        }.frame(minHeight: 46).listRowBackground(Palette.window)
                        .contextMenu {
                            if !folder.isArchive {
                                Button("Use as tree root", systemImage: "folder.badge.checkmark") {
                                    Task { await store.setTreeRoot(folder.path) }
                                }
                            }
                        }
                    }
                    ForEach(files) { track in
                        TrackRow(track: track, action: { Task { await store.addFile(track, play: true) }; tab = .queue }, add: {
                            Task { await store.addFile(track) }
                        })
                        .contextMenu {
                            Button("Track details", systemImage: "info.circle") { Task { detailsTrack = (try? await store.api.metadata([track]))?.first ?? track } }
                            downloadMenuItem(track)
                        }
                    }
                }.listStyle(.plain).scrollContentBackground(.hidden)
            }
        }
    }

    private var deviceTitle: String {
        let path = store.devicePath
        if path.isEmpty || path == store.importsURL.path { return "Files on this iPhone" }
        if path.hasPrefix("kog-archive:") {
            let parameters = URLComponents(string: path)?.queryItems ?? []
            let entry = parameters.first { $0.name == "entry" }?.value ?? ""
            let archive = parameters.first { $0.name == "archive" }?.value ?? ""
            return URL(fileURLWithPath: entry.isEmpty ? archive : entry).lastPathComponent
        }
        return URL(fileURLWithPath: path).lastPathComponent
    }

    private var deviceLibrary: some View {
        VStack(spacing: 0) {
            HStack {
                Image(systemName: "magnifyingglass").foregroundStyle(Palette.muted)
                TextField("Search device files", text: Binding(get: { store.searchText }, set: store.search)).autocorrectionDisabled().textInputAutocapitalization(.never)
                if !store.searchText.isEmpty { Button { store.search("") } label: { Image(systemName: "xmark.circle.fill").frame(width: 44, height: 44) }.accessibilityLabel("Clear search") }
            }.padding(.horizontal, 12).frame(minHeight: 44).background(Palette.raised)
            if store.searching {
                Button { Task { await store.toggleSearchPause() } } label: {
                    Label("\(store.searchScanned) items searched", systemImage: store.searchPaused ? "play.fill" : "pause.fill").font(.caption)
                }.frame(minHeight: 44)
            }
            HStack(spacing: 4) {
                if let parent = store.deviceListing?.parent, !parent.isEmpty, store.devicePath != store.activeDeviceRoot {
                    Button { store.search(""); store.browseDevice(parent) } label: {
                        Image(systemName: "chevron.left").frame(width: 40, height: 44)
                    }.accessibilityLabel("Parent folder")
                }
                Menu {
                    if store.deviceListing?.isArchive == false {
                        Button("Use this folder as tree root") { store.setDeviceRoot(store.devicePath) }
                    }
                    Button("Reset tree root") { store.setDeviceRoot(store.importsURL.path) }
                    Button("Refresh folder") { store.browseDevice(store.devicePath) }
                } label: { Image(systemName: "folder.badge.gearshape").frame(width: 44, height: 44) }.accessibilityLabel("Device tree root")
                Text(deviceTitle).lineLimit(1).font(.caption.weight(.semibold)).foregroundStyle(Palette.muted)
                Spacer()
                Button { showFolderPicker = true } label: { Image(systemName: "folder.badge.plus").frame(width: 44, height: 44) }
                    .accessibilityLabel("Import folder")
                Button { showFilePicker = true } label: { Image(systemName: "plus").frame(width: 44, height: 44) }
                    .accessibilityLabel("Import files")
            }.padding(.horizontal, 10).frame(height: 46).background(Palette.panel)
            let folders = store.searchText.isEmpty ? (store.deviceListing?.directories ?? []) : store.searchFolders
            let files = store.searchText.isEmpty ? store.deviceFiles : store.searchTracks
            if folders.isEmpty && files.isEmpty && !store.importing {
                emptyView("No imported music", detail: "Import files or a folder from Files to play offline.") { showFilePicker = true }
            } else {
                List {
                    ForEach(folders) { folder in
                        HStack(spacing: 10) {
                            Button { store.search(""); store.browseDevice(folder.path) } label: {
                                if ["zip", "7z", "rar", "rsn"].contains(URL(fileURLWithPath: folder.name).pathExtension.lowercased()) {
                                    FormatIcon(track: Track(kind: "device", path: folder.path, name: folder.name))
                                } else {
                                    Image("kog_folder").resizable().scaledToFit().frame(width: 23, height: 23)
                                }
                                Text(folder.name).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                            }.buttonStyle(.plain)
                            Button { Task { await store.addDeviceFolder(folder) } } label: {
                                Image(systemName: "plus").frame(width: 44, height: 44)
                            }.accessibilityLabel("Add folder to queue")
                        }.frame(minHeight: 46).listRowBackground(Palette.window)
                    }
                    ForEach(files) { track in
                        TrackRow(track: track, action: {
                            Task { await store.addFile(track, play: true); tab = .queue }
                        }, add: { Task { await store.addFile(track) } })
                        .contextMenu {
                            Button("Track details", systemImage: "info.circle") { Task { detailsTrack = (try? await store.deviceAPI.metadata([track]))?.first ?? track } }
                            Button(store.isStarred(track) ? "Unstar" : "Star", systemImage: "star") { Task { await store.toggleStar(track) } }
                            if !track.path.hasPrefix("kog-archive:") {
                                Button("Delete imported file", systemImage: "trash", role: .destructive) {
                                    store.deleteDeviceFile(track)
                                }
                            }
                        }
                    }
                }.listStyle(.plain).scrollContentBackground(.hidden)
            }
        }
    }

    @ViewBuilder private func downloadMenuItem(_ track: Track) -> some View {
        if track.kind == "local" || track.kind == "archive" {
            Button("Save to iPhone", systemImage: "square.and.arrow.down") {
                Task { await store.saveFromServer(track) }
            }.disabled(store.downloading.contains(track.id))
        }
    }

    private var playlists: some View {
        VStack(spacing: 0) {
            Picker("Playlist location", selection: $store.playlistOnDevice) { Text("Server").tag(false); Text("On this iPhone").tag(true) }
                .pickerStyle(.segmented).padding(10)
            if let selected = store.selectedPlaylist {
                HStack {
                    Text("\(store.playlistTracks.count) tracks").font(.caption).foregroundStyle(Palette.muted)
                    Spacer()
                    Button("Play") { store.replaceQueue(store.playlistTracks); tab = .queue }
                        .font(.subheadline.weight(.semibold)).frame(minHeight: 40)
                    Button("Add all") { store.add(store.playlistTracks); tab = .queue }
                        .font(.subheadline.weight(.semibold)).frame(minHeight: 40)
                }.padding(.horizontal, 16).background(Palette.panel)
                List {
                    ForEach(Array(store.playlistTracks.enumerated()), id: \.offset) { _, track in
                        TrackRow(track: track, action: { store.add([track], play: true); tab = .queue }, add: { store.add([track]) })
                            .contextMenu { Button("Track details", systemImage: "info.circle") { detailsTrack = track }; downloadMenuItem(track) }
                    }.onDelete { offsets in Task { await store.removePlaylistTracks(offsets) } }
                }.listStyle(.plain).scrollContentBackground(.hidden)
                .contextMenu {
                    if selected.id != 0 {
                        Button("Rename") { beginRename(selected) }
                        Button("Delete \(selected.name)", role: .destructive) { deletingPlaylist = selected }
                    }
                }
            } else if store.playlists.isEmpty {
                emptyView("No playlists", detail: "Create a playlist on your server or on this iPhone.") { showCreatePlaylist = true }
            } else {
                List {
                    ForEach(store.playlists) { playlist in
                        Button { store.openPlaylistTab(playlist); tab = .queue } label: {
                            HStack(spacing: 12) {
                                Image(systemName: "music.note.list").foregroundStyle(Palette.accent).frame(width: 28)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(playlist.name).foregroundStyle(.white).lineLimit(1)
                                    Text("\(playlist.entryCount) songs").font(.caption).foregroundStyle(Palette.muted)
                                }
                                Spacer()
                                Image(systemName: "chevron.right").foregroundStyle(Palette.muted)
                            }.frame(minHeight: 52)
                        }.listRowBackground(Palette.window)
                        .contextMenu { playlistActions(playlist) }
                    }
                }.listStyle(.plain).scrollContentBackground(.hidden)
            }
        }
    }

    @ViewBuilder private func playlistActions(_ playlist: SavedPlaylist) -> some View {
        Button("Open in tab", systemImage: "rectangle.on.rectangle") { store.openPlaylistTab(playlist); tab = .queue }
        Button("Export M3U…", systemImage: "square.and.arrow.up") { Task { shareURL = await store.exportPlaylist(playlist) } }
        if playlist.id != 0 {
            Button("Add queue to playlist", systemImage: "text.badge.plus") { Task { await store.appendToPlaylist(playlist, tracks: store.queue) } }
            Button("Duplicate", systemImage: "doc.on.doc") { Task { await store.duplicatePlaylist(playlist) } }
            Button("Remove missing files", systemImage: "doc.badge.ellipsis") { Task { await store.prunePlaylist(playlist) } }
            Button("Rename", systemImage: "pencil") { beginRename(playlist) }
            Button("Delete", systemImage: "trash", role: .destructive) { deletingPlaylist = playlist }
        }
    }

    private func beginRename(_ playlist: SavedPlaylist) {
        renamedName = playlist.name
        renameTarget = playlist
    }

    private func emptyView(_ title: String, detail: String, action: @escaping () -> Void) -> some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: "music.note.list").font(.system(size: 38)).foregroundStyle(Palette.muted)
            Text(title).font(.headline)
            Text(detail).font(.subheadline).foregroundStyle(Palette.muted).multilineTextAlignment(.center)
            Button("Get started", action: action).buttonStyle(.borderedProminent)
            Spacer()
        }.padding(24).frame(maxWidth: .infinity)
    }

    private var miniPlayer: some View {
        Group {
            if let track = store.current {
                HStack(spacing: 10) {
                    Button { showPlayer = true } label: {
                        KogArtwork(track: track)
                            .frame(width: 42, height: 42).background(Palette.raised, in: RoundedRectangle(cornerRadius: 6)).clipped()
                    }.buttonStyle(.plain)
                    Button { showPlayer = true } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(track.label).font(.subheadline.weight(.semibold)).lineLimit(1)
                            Text(track.artist.isEmpty ? "Kog" : track.artist).font(.caption).foregroundStyle(Palette.muted).lineLimit(1)
                        }.frame(maxWidth: .infinity, alignment: .leading)
                    }.buttonStyle(.plain)
                    Button { store.togglePlayback() } label: {
                        Image(systemName: store.playing ? "pause.fill" : "play.fill").frame(width: 44, height: 50)
                    }.accessibilityLabel(store.playing ? "Pause" : "Play")
                    Button { store.next() } label: { Image(systemName: "forward.end.fill").frame(width: 44, height: 50) }
                        .accessibilityLabel("Next")
                }
                .padding(.horizontal, 10).frame(height: 58).background(Palette.raised)
                .overlay(alignment: .top) { Rectangle().fill(Palette.border).frame(height: 1) }
            }
        }
    }

    private var settings: some View {
        NavigationStack {
            Form {
                Section("Kog server") {
                    TextField("http://host:8420", text: $store.server)
                        .textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                    SecureField("Access token", text: $store.token)
                    TextField("Username (optional)", text: $store.username).textInputAutocapitalization(.never)
                    SecureField("Password (optional)", text: $store.password)
                    Picker("Stream codec", selection: Binding(get: { store.codec }, set: store.selectCodec)) {
                        Text("AAC").tag("aac"); Text("Opus").tag("opus"); Text("FLAC").tag("flac")
                    }
                    Button("Connect") { store.saveSettings(); showSettings = false }
                    if store.connected { Label("Connected", systemImage: "checkmark.circle.fill").foregroundStyle(.green) }
                }
                Section("On this iPhone") {
                    Button("Import files") { pendingPicker = "files"; showSettings = false }
                    Button("Import folder") { pendingPicker = "folder"; showSettings = false }
                    Text("Imported music stays in Kog's Documents and can play without a server.")
                        .font(.caption).foregroundStyle(Palette.muted)
                }
                Section("Library tree root") {
                    Text(store.activeTreeRoot.isEmpty ? "Server library" : store.activeTreeRoot)
                        .font(.caption).textSelection(.enabled)
                    Button("Choose folder…") {
                        pickRootAfterSettings = true
                        showSettings = false
                    }.disabled(store.server.isEmpty)
                    if !store.treeRoot.isEmpty {
                        Button("Reset to server library") { Task { await store.setTreeRoot("") } }
                    }
                    Text("Saved for this server. Search and Random Radio use this folder and its subfolders.")
                        .font(.caption).foregroundStyle(Palette.muted)
                }
                Section("Playback") {
                    Picker("Shuffle", selection: Binding(get: { store.shuffle }, set: { store.selectShuffle($0) })) { ForEach(ShuffleMode.allCases) { Text($0.label).tag($0) } }
                    Picker("Repeat", selection: Binding(get: { store.repeatMode }, set: { store.selectRepeat($0) })) { ForEach(RepeatMode.allCases) { Text($0.label).tag($0) } }
                    Toggle("Track notifications", isOn: Binding(get: { store.notifyTracks }, set: { value in Task { await store.setNotifications(value) } }))
                }
                Section("MIDI synthesis") {
                    Picker("Server synth", selection: Binding(get: { store.midiEngine },
                                                             set: { store.selectMidiEngine($0) })) {
                        Text("OPL3").tag("opl3windows")
                        Text("SoundFont").tag("rustysynth-sf2")
                        Text("SC-55").tag("nuked-sc55")
                        Text("MT-32").tag("munt-mt32")
                    }
                    .disabled(!store.connected)
                    Picker("On-device synth", selection: Binding(get: { store.localMidiEngine },
                                                               set: { store.selectLocalMidiEngine($0) })) {
                        Text("OPL3").tag("opl3windows")
                        Text("SoundFont").tag("rustysynth-sf2")
                        Text("SC-55").tag("nuked-sc55")
                        Text("MT-32").tag("munt-mt32")
                    }
                    Button("Import SF2 SoundFont") { pendingPicker = "soundfont"; showSettings = false }
                    Button("Import SC-55 ROM folder") { pendingPicker = "sc55"; showSettings = false }
                    Button("Import MT-32 ROM folder") { pendingPicker = "mt32"; showSettings = false }
                    if store.soundfontReady { Text("SF2 ready").font(.caption).foregroundStyle(Palette.muted) }
                    if store.sc55RomsReady { Text("SC-55 ROMs ready").font(.caption).foregroundStyle(Palette.muted) }
                    if store.mt32RomsReady { Text("MT-32 ROMs ready").font(.caption).foregroundStyle(Palette.muted) }
                    Text("Device files use imported assets. SC-55 and MT-32 need their own ROM folders.")
                        .font(.caption).foregroundStyle(Palette.muted)
                }
            }
            .scrollContentBackground(.hidden).background(Palette.window)
            .navigationTitle("Settings").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .topBarTrailing) { Button("Done") { store.saveSettings(); showSettings = false } } }
        }
    }
}

private struct ServerRootPicker: View {
    @ObservedObject var store: KogStore
    @Environment(\.dismiss) private var dismiss
    @State private var listing: Listing?
    @State private var path = ""
    @State private var loading = false
    @State private var failure: String?

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                TextField("Server folder path", text: $path)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                    .submitLabel(.go).onSubmit { Task { await load(path) } }
                    .accessibilityLabel("Server folder path")
                Button { Task { await load(path) } } label: {
                    Image(systemName: "arrow.right.circle.fill").frame(width: 44, height: 44)
                }.accessibilityLabel("Go to folder").disabled(loading)
            }.padding(.horizontal, 16)
            if let failure {
                Text(failure).font(.callout).foregroundStyle(.red).padding(.horizontal, 16)
            }
            List {
                Button { Task { await load("") } } label: {
                    Label("Server library", systemImage: "house")
                }
                if let listing, listing.path != store.libraryRoot, !listing.parent.isEmpty {
                    Button { Task { await load(listing.parent) } } label: {
                        Label("Parent folder", systemImage: "arrow.up")
                    }
                }
                if let listing {
                    ForEach(listing.directories.filter { !$0.isArchive }) { folder in
                        Button { Task { await load(folder.path) } } label: {
                            HStack {
                                Label(folder.name, systemImage: "folder")
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                if folder.path == store.activeTreeRoot {
                                    Image(systemName: "checkmark").foregroundStyle(Palette.accent)
                                }
                                Image(systemName: "chevron.right").foregroundStyle(Palette.muted)
                            }.frame(minHeight: 32)
                        }
                    }
                }
            }.disabled(loading).scrollContentBackground(.hidden)
            Button {
                guard let listing else { return }
                loading = true
                Task {
                    if await store.setTreeRoot(listing.path) { dismiss() }
                    else { failure = store.error; store.error = nil }
                    loading = false
                }
            } label: {
                Text("Use this folder").font(.headline).frame(maxWidth: .infinity, minHeight: 44)
            }
            .buttonStyle(.borderedProminent).disabled(listing == nil || loading)
            .padding(16)
        }
        .overlay { if loading { ProgressView().controlSize(.large) } }
        .background(Palette.window)
        .navigationTitle("Library tree root").navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .task { await load(store.activeTreeRoot) }
    }

    @MainActor private func load(_ requested: String) async {
        guard !loading else { return }
        loading = true; failure = nil
        defer { loading = false }
        do {
            let directory = try await store.api.browse(requested.trimmingCharacters(in: .whitespacesAndNewlines))
            guard !directory.isArchive else {
                throw KogError.response("Choose a folder rather than an archive.")
            }
            listing = directory
            path = directory.path
        } catch { failure = error.localizedDescription }
    }
}

struct TrackRow: View {
    let track: Track
    let action: () -> Void
    let add: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Button(action: action) {
                FormatIcon(track: track).frame(width: 25)
                VStack(alignment: .leading, spacing: 2) {
                    Text(track.label).foregroundStyle(.white).lineLimit(1)
                    if !track.detail.isEmpty { Text(track.detail).font(.caption).foregroundStyle(Palette.muted).lineLimit(1) }
                }.frame(maxWidth: .infinity, alignment: .leading)
            }.buttonStyle(.plain)
            Button(action: add) { Image(systemName: "plus").frame(width: 44, height: 44) }
                .accessibilityLabel("Add to queue")
        }
        .frame(minHeight: 46).listRowBackground(Palette.window)
    }

}

struct KogArtwork: View {
    @EnvironmentObject private var store: KogStore
    let track: Track
    @State private var image: UIImage?

    var body: some View {
        Group {
            if let image { Image(uiImage: image).resizable().scaledToFit() }
            else { Image(systemName: "music.note").foregroundStyle(Palette.muted) }
        }
        .task(id: track.id) {
            image = nil
            if track.isDevice {
                #if KOG_NATIVE_AUDIO
                let path = track.path
                let data = await Task.detached(priority: .utility) {
                    NativeAudioCatalog.artwork(path: path)
                }.value
                if !Task.isCancelled, let data { image = UIImage(data: data) }
                #endif
                return
            }
            let client = store.api
            if let data = try? await client.artData(track), !Task.isCancelled {
                image = UIImage(data: data)
            }
        }
    }
}
