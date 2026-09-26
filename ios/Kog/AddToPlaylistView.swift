import SwiftUI

struct AddToPlaylistView: View {
    @EnvironmentObject private var store: KogStore
    @Environment(\.dismiss) private var dismiss
    var tracks: [Track]
    @State private var playlists = [SavedPlaylist]()
    @State private var name = ""
    @State private var loading = false
    @State private var failure: String?
    private var device: Bool { tracks.first?.isDevice ?? false }
    private var compatible: Bool { !tracks.isEmpty && tracks.allSatisfy { $0.isDevice == device } }
    private var api: KogAPI { device ? store.deviceAPI : store.api }
    var body: some View {
        NavigationStack {
            List {
                Section(device ? "On this iPhone" : "Server playlists") {
                    HStack {
                        TextField("New playlist name", text: $name)
                        Button("Create") { Task { await create() } }.disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || loading || !compatible)
                    }
                    ForEach(playlists.filter { $0.id != 0 }) { playlist in
                        Button { Task { await append(playlist.id) } } label: {
                            HStack { Text(playlist.name); Spacer(); Text("\(playlist.entryCount)").foregroundStyle(.secondary) }.frame(minHeight: 36)
                        }.disabled(loading)
                    }
                }
                if let failure { Text(failure).foregroundStyle(.red) }
            }.navigationTitle("Add to playlist").navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
                .overlay { if loading { ProgressView() } }
                .task {
                    guard tracks.allSatisfy({ $0.isDevice == device }) else { failure = "Select tracks from one source to save them together."; return }
                    loading = true; defer { loading = false }
                    do { playlists = try await api.playlists() } catch { failure = error.localizedDescription }
                }
        }
    }
    private func create() async {
        loading = true; defer { loading = false }
        do { let id = try await api.createPlaylist(name); try await api.appendPlaylist(id, tracks: tracks); await store.loadPlaylists(); dismiss() }
        catch { failure = error.localizedDescription }
    }
    private func append(_ id: Int64) async {
        loading = true; defer { loading = false }
        do { try await api.appendPlaylist(id, tracks: tracks); await store.loadPlaylists(); dismiss() }
        catch { failure = error.localizedDescription }
    }
}
