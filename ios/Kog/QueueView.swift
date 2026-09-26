import SwiftUI

struct QueueView: View {
    @EnvironmentObject private var store: KogStore
    var openLibrary: () -> Void
    var showDetails: (Track) -> Void
    @State private var showVisualizer = false
    @State private var playlistTracks: [Track]?
    @State private var selecting = false
    @State private var selected = Set<Int>()
    @State private var showSave = false
    @State private var name = ""
    @State private var saveOnDevice = false
    @State private var confirmClear = false
    private var rows: [Int] {
        store.queue.indices.filter { index in
            let track = store.queue[index]
            return store.queueFilter.isEmpty || [track.label, track.detail, track.displayPath].joined(separator: " ").localizedStandardContains(store.queueFilter)
        }
    }
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Image(systemName: "magnifyingglass").foregroundStyle(Palette.muted)
                TextField("Search playlist", text: $store.queueFilter).autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
                if !store.queueFilter.isEmpty {
                    Button { store.queueFilter = "" } label: { Image(systemName: "xmark.circle.fill").frame(width: 44, height: 44) }
                        .accessibilityLabel("Clear playlist search")
                }
            }.padding(.horizontal, 12).background(Palette.raised)
            HStack(spacing: 12) {
                Text(selecting ? "\(selected.count) selected" : "\(rows.count) / \(store.queue.count) tracks")
                    .font(.caption).foregroundStyle(Palette.muted)
                Spacer()
                Button(selecting ? "Done" : "Select") { selecting.toggle(); selected = [] }.frame(minHeight: 44)
                if !selecting && store.queueFilter.isEmpty { EditButton().frame(minHeight: 44) }
                Menu {
                    if selecting {
                        Button("Select all results", systemImage: "checkmark.circle") { selected = Set(rows) }
                        Button("Add selected to playlist…", systemImage: "text.badge.plus") { playlistTracks = selected.sorted().map { store.queue[$0] } }.disabled(selected.isEmpty)
                        Button("Remove selected", systemImage: "trash", role: .destructive) { store.remove(IndexSet(selected)); selected = [] }
                            .disabled(selected.isEmpty)
                    }
                    Menu("Sort", systemImage: "arrow.up.arrow.down") {
                        ForEach(TrackSort.all) { field in
                            Button { store.sortQueue(field.key) } label: {
                                Label(field.label, systemImage: store.sortKey == field.key ? (store.sortDescending ? "arrow.down" : "arrow.up") : "arrow.up.arrow.down")
                            }
                        }
                    }
                    Button("Save queue as playlist…", systemImage: "square.and.arrow.down") {
                        saveOnDevice = store.queue.allSatisfy(\.isDevice); showSave = true
                    }.disabled(store.queue.isEmpty)
                    Button("Clear queue", systemImage: "trash", role: .destructive) { confirmClear = true }.disabled(store.queue.isEmpty)
                } label: { Image(systemName: "ellipsis.circle").frame(width: 44, height: 44) }.accessibilityLabel("Queue actions")
            }.padding(.horizontal, 12).background(Palette.panel)
            if store.queue.isEmpty {
                ContentUnavailableView {
                    Label("Ready to play", systemImage: "music.note.list")
                } description: { Text("Add tracks from your server or import music to play offline.") }
                actions: { Button("Browse music", action: openLibrary).buttonStyle(.borderedProminent) }
            } else {
                List {
                    ForEach(rows, id: \.self) { index in
                        let track = store.queue[index]
                        HStack(spacing: 8) {
                            if selecting {
                                Button { toggle(index) } label: {
                                    Image(systemName: selected.contains(index) ? "checkmark.circle.fill" : "circle").frame(width: 44, height: 44)
                                }.buttonStyle(.plain).accessibilityLabel("Select \(track.label)")
                            }
                            if index == store.currentIndex && store.playing && !selecting {
                                Button { showVisualizer = true } label: {
                                    WaveformView(audio: store.visualization, playing: store.playing).frame(width: 32, height: 44)
                                }.buttonStyle(.plain).accessibilityLabel("Show audio visualizer")
                            }
                            Button {
                                if selecting { toggle(index) }
                                else if index == store.currentIndex { store.togglePlayback() }
                                else { store.playIndex(index) }
                            } label: {
                                HStack(spacing: 8) {
                                    if index != store.currentIndex || !store.playing {
                                        Image(systemName: index == store.currentIndex ? "pause.fill" : "music.note").foregroundStyle(Palette.muted).frame(width: 26)
                                    }
                                    FormatIcon(track: track)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(track.label).foregroundStyle(.white).lineLimit(1)
                                        if !track.detail.isEmpty { Text(track.detail).font(.caption).foregroundStyle(Palette.muted).lineLimit(1) }
                                    }.frame(maxWidth: .infinity, alignment: .leading)
                                }.frame(minHeight: 44).contentShape(Rectangle())
                            }.buttonStyle(.plain)
                            if !selecting {
                                Button { Task { await store.toggleStar(track) } } label: {
                                    Image(systemName: store.isStarred(track) ? "star.fill" : "star")
                                        .foregroundStyle(store.isStarred(track) ? .yellow : Palette.muted).frame(width: 44, height: 44)
                                }.buttonStyle(.plain).accessibilityLabel(store.isStarred(track) ? "Unstar track" : "Star track")
                            }
                        }
                        .listRowInsets(EdgeInsets(top: 2, leading: 10, bottom: 2, trailing: 8))
                        .listRowBackground(index == store.currentIndex ? Palette.raised : Palette.window)
                        .contextMenu {
                            Button("Play", systemImage: "play.fill") { store.playIndex(index) }
                            Button("Track details", systemImage: "info.circle") { showDetails(track) }
                            Button("Reveal in library", systemImage: "folder") { Task { await store.reveal(track); openLibrary() } }
                            Button(store.isStarred(track) ? "Unstar" : "Star", systemImage: "star") { Task { await store.toggleStar(track) } }
                            if !track.isDevice {
                                Button("Save to iPhone", systemImage: "square.and.arrow.down") { Task { await store.saveFromServer(track) } }
                            }
                            Button("Add to playlist…", systemImage: "text.badge.plus") { playlistTracks = [track] }
                            Button("Select", systemImage: "checkmark.circle") { selecting = true; selected.insert(index) }
                            Button("Remove", systemImage: "trash", role: .destructive) { store.remove(IndexSet(integer: index)) }
                        }
                    }
                    .onDelete { offsets in store.remove(IndexSet(offsets.map { rows[$0] })) }
                    .onMove { source, destination in if store.queueFilter.isEmpty { store.move(source, to: destination) } }
                    .moveDisabled(!store.queueFilter.isEmpty || selecting)
                }.listStyle(.plain).scrollContentBackground(.hidden).scrollDismissesKeyboard(.interactively)
            }
        }
        .sheet(isPresented: $showVisualizer) { NowPlayingView(initialVisualization: true) }
        .sheet(isPresented: Binding(get: { playlistTracks != nil }, set: { if !$0 { playlistTracks = nil } })) {
            if let tracks = playlistTracks { AddToPlaylistView(tracks: tracks) }
        }
        .onChange(of: store.queue) { _, _ in selected = [] }
        .confirmationDialog("Clear all tracks from the queue?", isPresented: $confirmClear, titleVisibility: .visible) {
            Button("Clear queue", role: .destructive) { store.clearQueue() }
        }
        .sheet(isPresented: $showSave) {
            NavigationStack {
                Form {
                    TextField("Playlist name", text: $name)
                    Picker("Save to", selection: $saveOnDevice) { Text("Server").tag(false); Text("On this iPhone").tag(true) }
                    Text("Device playlists work offline. Server playlists are available on your other devices.").font(.caption)
                }.navigationTitle("Save queue").toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button("Cancel") { showSave = false } }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Save") { let title = name; store.playlistOnDevice = saveOnDevice; Task { await store.createPlaylist(title, saveQueue: true) }; showSave = false; name = "" }
                            .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                }
            }.presentationDetents([.medium, .large])
        }
    }
    private func toggle(_ index: Int) { if selected.contains(index) { selected.remove(index) } else { selected.insert(index) } }
}

struct TrackDetailsView: View {
    var track: Track
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
                ForEach(Array(track.detailRows.enumerated()), id: \.offset) { _, row in
                    VStack(alignment: .leading, spacing: 5) {
                        Text(row.0).font(.caption).foregroundStyle(.secondary)
                        Text(row.1).textSelection(.enabled)
                    }.padding(.vertical, 3)
                }
            }.navigationTitle("Track details").navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }
}
