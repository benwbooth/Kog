import MediaPlayer
import SwiftUI
import UniformTypeIdentifiers

private enum Palette {
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
    @State private var deviceMode = false

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(Palette.border).frame(height: 1)
            Group {
                switch tab {
                case .library: library
                case .queue: queue
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
        .sheet(isPresented: $showSettings) { settings }
        .sheet(isPresented: $showPlayer) { fullPlayer }
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
    }

    private var header: some View {
        HStack(spacing: 6) {
            if tab == .playlists && store.selectedPlaylist != nil {
                Button { store.selectedPlaylist = nil } label: { Image(systemName: "chevron.left").frame(width: 36, height: 44) }
            }
            Text(tab == .playlists ? (store.selectedPlaylist?.name ?? "Playlists") : tab.rawValue)
                .font(.title3.bold()).lineLimit(1)
            Spacer()
            if tab == .queue {
                Menu {
                    ForEach(["Title", "Artist", "Album"], id: \.self) { key in
                        Button("Sort by \(key)") { store.sortQueue(key) }
                    }
                    Divider()
                    Button("Clear queue", systemImage: "trash", role: .destructive) { store.clearQueue() }
                } label: { Image(systemName: "arrow.up.arrow.down").frame(width: 44, height: 44) }
                .accessibilityLabel("Sort or clear queue")
            }
            if tab == .playlists && store.selectedPlaylist == nil {
                Button { showCreatePlaylist = true } label: { Image(systemName: "plus").frame(width: 44, height: 44) }
                    .accessibilityLabel("Create playlist")
            }
            if tab == .playlists, let playlist = store.selectedPlaylist, playlist.id != 0 {
                Menu {
                    Button("Rename", systemImage: "pencil") { beginRename(playlist) }
                    Button("Add queue", systemImage: "text.badge.plus") {
                        Task { await store.appendToPlaylist(playlist, tracks: store.queue) }
                    }
                    Button("Delete playlist", systemImage: "trash", role: .destructive) {
                        Task { await store.deletePlaylist(playlist) }
                    }
                } label: { Image(systemName: "ellipsis").frame(width: 44, height: 44) }
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
                   !listing.parent.isEmpty && listing.path != store.libraryRoot {
                    Button { Task { await store.browse(listing.parent) } } label: {
                        Image(systemName: "chevron.left").frame(width: 38, height: 40)
                    }.accessibilityLabel("Parent folder")
                }
                Text(store.searchText.isEmpty ? (store.listing?.path.components(separatedBy: "/").last ?? "Library") : "Search results")
                    .lineLimit(1).font(.caption.weight(.semibold)).foregroundStyle(Palette.muted)
                Spacer()
                if store.searching { Text("\(store.searchScanned) scanned").font(.caption2).foregroundStyle(Palette.muted) }
                Button { showFilePicker = true } label: { Image(systemName: "plus").frame(width: 40, height: 40) }
                    .accessibilityLabel("Import device files")
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
                    }
                    ForEach(files) { track in
                        TrackRow(track: track, action: { Task { await store.addFile(track, play: true) }; tab = .queue }, add: {
                            Task { await store.addFile(track) }
                        })
                        .contextMenu { downloadMenuItem(track) }
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
            HStack(spacing: 4) {
                if let parent = store.deviceListing?.parent, !parent.isEmpty {
                    Button { store.browseDevice(parent) } label: {
                        Image(systemName: "chevron.left").frame(width: 40, height: 44)
                    }.accessibilityLabel("Parent folder")
                }
                Text(deviceTitle).lineLimit(1).font(.caption.weight(.semibold)).foregroundStyle(Palette.muted)
                Spacer()
                Button { showFolderPicker = true } label: { Image(systemName: "folder.badge.plus").frame(width: 44, height: 44) }
                    .accessibilityLabel("Import folder")
                Button { showFilePicker = true } label: { Image(systemName: "plus").frame(width: 44, height: 44) }
                    .accessibilityLabel("Import files")
            }.padding(.horizontal, 10).frame(height: 46).background(Palette.panel)
            let folders = store.deviceListing?.directories ?? []
            if folders.isEmpty && store.deviceFiles.isEmpty && !store.importing {
                emptyView("No imported music", detail: "Import files or a folder from Files to play offline.") { showFilePicker = true }
            } else {
                List {
                    ForEach(folders) { folder in
                        HStack(spacing: 10) {
                            Button { store.browseDevice(folder.path) } label: {
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
                    ForEach(store.deviceFiles) { track in
                        TrackRow(track: track, action: {
                            Task { await store.addFile(track, play: true); tab = .queue }
                        }, add: { Task { await store.addFile(track) } })
                        .contextMenu {
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

    private var queue: some View {
        VStack(spacing: 0) {
            if store.queue.isEmpty {
                emptyView("Ready to play", detail: "Add tracks from Library or import music from Files.") { tab = .library }
            } else {
                HStack {
                    Text("\(store.queue.count) tracks").font(.caption).foregroundStyle(Palette.muted)
                    Spacer()
                    EditButton().font(.subheadline)
                }.padding(.horizontal, 16).frame(height: 38).background(Palette.panel)
                List {
                    ForEach(Array(store.queue.enumerated()), id: \.offset) { index, track in
                        HStack(spacing: 10) {
                            Button { if index == store.currentIndex { store.togglePlayback() } else { store.playIndex(index) } } label: {
                                Image(systemName: index == store.currentIndex ? (store.playing ? "waveform" : "pause.fill") : "music.note")
                                    .font(.system(size: 14)).foregroundStyle(index == store.currentIndex ? Palette.accent : Palette.muted).frame(width: 20)
                                FormatIcon(track: track)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(track.label).lineLimit(1).foregroundStyle(.white)
                                    if !track.detail.isEmpty { Text(track.detail).font(.caption).foregroundStyle(Palette.muted).lineLimit(1) }
                                }.frame(maxWidth: .infinity, alignment: .leading)
                            }.buttonStyle(.plain)
                            if !track.isDevice {
                                Button { Task { await store.toggleStar(track) } } label: {
                                    Image(systemName: store.stars.contains(track.id) ? "star.fill" : "star")
                                        .foregroundStyle(store.stars.contains(track.id) ? .yellow : Palette.muted)
                                        .frame(width: 40, height: 44)
                                }.buttonStyle(.plain).accessibilityLabel("Star track")
                            }
                        }
                        .frame(minHeight: 48).listRowBackground(index == store.currentIndex ? Palette.raised : Palette.window)
                        .contextMenu { queueMenu(track, at: index) }
                    }
                    .onDelete(perform: store.remove).onMove(perform: store.move)
                }.listStyle(.plain).scrollContentBackground(.hidden)
            }
        }
    }

    @ViewBuilder private func queueMenu(_ track: Track, at index: Int) -> some View {
        if !track.isDevice {
            Button(store.stars.contains(track.id) ? "Unstar" : "Star", systemImage: "star") { Task { await store.toggleStar(track) } }
            downloadMenuItem(track)
            Menu("Add to playlist", systemImage: "text.badge.plus") {
                ForEach(store.playlists) { playlist in
                    Button(playlist.name) { Task { await store.appendToPlaylist(playlist, tracks: [track]) } }
                }
            }
        }
        Button("Remove from queue", systemImage: "trash", role: .destructive) {
            if store.queue.indices.contains(index) { store.remove(IndexSet(integer: index)) }
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
            if let selected = store.selectedPlaylist {
                HStack {
                    Text("\(store.playlistTracks.count) tracks").font(.caption).foregroundStyle(Palette.muted)
                    Spacer()
                    Button("Play") { store.add(store.playlistTracks, play: true); tab = .queue }
                        .font(.subheadline.weight(.semibold)).frame(minHeight: 40)
                    Button("Add all") { store.add(store.playlistTracks); tab = .queue }
                        .font(.subheadline.weight(.semibold)).frame(minHeight: 40)
                }.padding(.horizontal, 16).background(Palette.panel)
                List {
                    ForEach(store.playlistTracks) { track in
                        TrackRow(track: track, action: { store.add([track], play: true); tab = .queue }, add: { store.add([track]) })
                            .contextMenu { downloadMenuItem(track) }
                    }
                }.listStyle(.plain).scrollContentBackground(.hidden)
                .contextMenu {
                    if selected.id != 0 {
                        Button("Rename") { beginRename(selected) }
                        Button("Delete \(selected.name)", role: .destructive) { Task { await store.deletePlaylist(selected) } }
                    }
                }
            } else if store.playlists.isEmpty {
                emptyView("No playlists", detail: "Create a playlist to save tracks on your server.") { showCreatePlaylist = true }
            } else {
                List {
                    ForEach(store.playlists) { playlist in
                        Button { Task { await store.openPlaylist(playlist) } } label: {
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
                        .contextMenu {
                            if playlist.id != 0 {
                                Button("Add queue to playlist") { Task { await store.appendToPlaylist(playlist, tracks: store.queue) } }
                                Button("Rename", systemImage: "pencil") { beginRename(playlist) }
                                Button("Delete", systemImage: "trash", role: .destructive) { Task { await store.deletePlaylist(playlist) } }
                            }
                        }
                    }
                }.listStyle(.plain).scrollContentBackground(.hidden)
            }
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

    private var fullPlayer: some View {
        VStack(spacing: 24) {
            HStack {
                Button { showPlayer = false } label: { Image(systemName: "chevron.down").frame(width: 44, height: 44) }
                Spacer(); Text("NOW PLAYING").font(.caption.weight(.bold)).foregroundStyle(Palette.muted)
                Spacer(); Color.clear.frame(width: 44, height: 44)
            }
            Spacer(minLength: 4)
            Group {
                if let track = store.current { KogArtwork(track: track) }
                else { artworkPlaceholder }
            }
            .frame(maxWidth: 330).aspectRatio(1, contentMode: .fit)
            .background(Palette.raised, in: RoundedRectangle(cornerRadius: 16))
            .clipShape(RoundedRectangle(cornerRadius: 16))
            Spacer(minLength: 4)
            VStack(spacing: 5) {
                Text(store.current?.label ?? "Ready to play").font(.title2.bold()).lineLimit(2).multilineTextAlignment(.center)
                Text(store.current?.detail ?? "").font(.subheadline).foregroundStyle(Palette.muted).lineLimit(2).multilineTextAlignment(.center)
            }
            Slider(value: Binding(get: { min(store.position, max(store.duration, 0.01)) }, set: store.seek), in: 0...max(store.duration, 0.01))
            HStack { Text(time(store.position)); Spacer(); Text(time(store.duration)) }
                .font(.caption.monospacedDigit()).foregroundStyle(Palette.muted).padding(.top, -18)
            HStack(spacing: 16) {
                control("shuffle", active: store.shuffle, label: "Shuffle") { store.shuffle.toggle() }
                control("backward.end.fill", label: "Previous") { store.previous() }
                Button { store.togglePlayback() } label: {
                    Image(systemName: store.playing ? "pause.fill" : "play.fill")
                        .font(.system(size: 26)).foregroundStyle(Palette.window)
                        .frame(width: 68, height: 68).background(.white, in: Circle())
                }.accessibilityLabel(store.playing ? "Pause" : "Play")
                control("forward.end.fill", label: "Next") { store.next() }
                control("repeat", active: store.repeatQueue, label: "Repeat") { store.repeatQueue.toggle() }
            }
            HStack {
                Button { Task { await store.toggleRadio() } } label: {
                    Label("Random radio", systemImage: "die.face.5.fill")
                        .foregroundStyle(store.radio ? Palette.accent : Palette.muted)
                }.frame(minHeight: 44)
                Spacer()
                if let track = store.current, !track.isDevice {
                    if track.kind == "local" || track.kind == "archive" {
                        Button { Task { await store.saveFromServer(track) } } label: {
                            if store.downloading.contains(track.id) { ProgressView().frame(width: 44, height: 44) }
                            else { Image(systemName: "square.and.arrow.down").frame(width: 44, height: 44) }
                        }
                        .disabled(store.downloading.contains(track.id))
                        .accessibilityLabel("Save to iPhone")
                    }
                    Button { Task { await store.toggleStar(track) } } label: {
                        Image(systemName: store.stars.contains(track.id) ? "star.fill" : "star")
                            .foregroundStyle(store.stars.contains(track.id) ? .yellow : Palette.muted)
                            .frame(width: 44, height: 44)
                    }.accessibilityLabel("Star track")
                }
            }
            SystemVolumeView().frame(height: 38)
        }
        .padding(24).background(Palette.window.ignoresSafeArea()).presentationDragIndicator(.visible)
    }

    private var artworkPlaceholder: some View {
        Image(systemName: "music.note").font(.system(size: 86)).foregroundStyle(Palette.muted)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func control(_ symbol: String, active: Bool = false, label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: symbol).font(.system(size: 21))
            .foregroundStyle(active ? Palette.accent : .white).frame(width: 46, height: 52) }
            .accessibilityLabel(label)
    }

    private func time(_ seconds: Double) -> String {
        guard seconds.isFinite else { return "0:00" }
        let whole = max(0, Int(seconds))
        return whole >= 3600 ? String(format: "%d:%02d:%02d", whole / 3600, (whole / 60) % 60, whole % 60) :
            String(format: "%d:%02d", whole / 60, whole % 60)
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
                    Picker("Stream codec", selection: $store.codec) {
                        Text("AAC").tag("aac"); Text("Opus").tag("opus"); Text("FLAC").tag("flac")
                    }
                    Button("Connect") { store.saveSettings(); showSettings = false }
                    if store.connected { Label("Connected", systemImage: "checkmark.circle.fill").foregroundStyle(.green) }
                }
                Section("On this iPhone") {
                    Button("Import files") { showSettings = false; showFilePicker = true }
                    Button("Import folder") { showSettings = false; showFolderPicker = true }
                    Text("Imported music stays in Kog's Documents and can play without a server.")
                        .font(.caption).foregroundStyle(Palette.muted)
                }
                Section("Playback") {
                    Toggle("Shuffle", isOn: $store.shuffle)
                    Toggle("Repeat queue", isOn: $store.repeatQueue)
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
                    Button("Import SF2 SoundFont") { showSettings = false; showSoundfontPicker = true }
                    Button("Import SC-55 ROM folder") { showSettings = false; showSc55Picker = true }
                    Button("Import MT-32 ROM folder") { showSettings = false; showMt32Picker = true }
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

private struct TrackRow: View {
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

private struct SystemVolumeView: UIViewRepresentable {
    func makeUIView(context: Context) -> MPVolumeView {
        let view = MPVolumeView(frame: .zero)
        return view
    }
    func updateUIView(_ uiView: MPVolumeView, context: Context) {}
}

private struct KogArtwork: View {
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
