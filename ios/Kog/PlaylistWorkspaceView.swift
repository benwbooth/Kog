import SwiftUI

struct PlaylistWorkspaceTabs: View {
    @EnvironmentObject private var store: KogStore
    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                ForEach(store.workspace.tabs) { item in
                    HStack(spacing: 0) {
                        Button { store.workspaceCommand(["op": "focus", "key": item.key]) } label: {
                            Text(item.name + (item.dirty ? " •" : "")).lineLimit(1).padding(.horizontal, 12).frame(minHeight: 44)
                        }
                        if item.key != "queue" {
                            Button { store.workspaceCommand(["op": "close", "key": item.key]) } label: {
                                Image(systemName: "xmark").font(.caption).frame(width: 36, height: 44)
                            }.accessibilityLabel("Close \(item.name)")
                        }
                    }.foregroundStyle(store.workspace.active == item.key ? Palette.accent : Palette.muted)
                        .background(store.workspace.active == item.key ? Palette.raised : Palette.panel)
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                }
            }.padding(.horizontal, 6).padding(.vertical, 4)
        }.background(Palette.panel)
            .alert("Save playlist changes?", isPresented: Binding(
                get: { store.workspace.pending_close != nil },
                set: { if !$0 && store.workspace.pending_close != nil { store.workspaceCommand(["op": "resolve_close", "choice": "cancel"]) } })) {
                Button("Save") { store.workspaceCommand(["op": "resolve_close", "choice": "save"]) }
                Button("Discard", role: .destructive) { store.workspaceCommand(["op": "resolve_close", "choice": "discard"]) }
                Button("Cancel", role: .cancel) { store.workspaceCommand(["op": "resolve_close", "choice": "cancel"]) }
            } message: { Text("The playlist has unsaved changes.") }
    }
}

struct PlaylistWorkspaceEditor: View {
    @EnvironmentObject private var store: KogStore
    var queueSelection: [Track] = []
    private var tab: PlaylistWorkspaceTab? { store.workspace.activeTab }
    var body: some View {
        VStack(spacing: 0) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    Button("Play Now") { queue("play_now") }
                    Button("Play Next") { queue("play_next") }
                    Button("Add to Queue") { queue("add_to_queue") }
                }.frame(minHeight: 44).padding(.horizontal, 12)
            }.disabled(store.workspace.actions["queue"] != true)
            HStack {
                Text(store.workspace.selected.isEmpty ? "\(store.workspace.entries.count) tracks · whole playlist" : "\(store.workspace.selected.count) selected")
                    .font(.caption).foregroundStyle(Palette.muted)
                Spacer()
                Button(tab?.saving == true ? "Saving…" : "Save") { send("save") }
                    .disabled(store.workspace.actions["save"] != true)
                Menu {
                    Button("Select all") { store.workspaceCommand(["op": "selection", "command": ["op": "all"]]) }.disabled(store.workspace.actions["select_all"] != true)
                    Button("Clear selection") { store.workspaceCommand(["op": "selection", "command": ["op": "clear"]]) }.disabled(store.workspace.actions["clear_selection"] != true)
                    if tab?.readonly != true {
                        Button("Add Play Queue") { store.workspaceAppend(store.queue) }.disabled(store.workspace.actions["add_play_queue"] != true)
                        Button("Add Queue Selection") { store.workspaceAppend(queueSelection) }.disabled(store.workspace.actions["add_queue_selection"] != true)
                        Button("Remove selected", role: .destructive) { send("remove") }.disabled(store.workspace.actions["remove"] != true)
                        Button("Move Up") { store.workspaceCommand(["op": "nudge", "delta": -1]) }.disabled(store.workspace.actions["move_up"] != true)
                        Button("Move Down") { store.workspaceCommand(["op": "nudge", "delta": 1]) }.disabled(store.workspace.actions["move_down"] != true)
                        Button("Undo") { send("undo") }.disabled(store.workspace.actions["undo"] != true)
                        Button("Redo") { send("redo") }.disabled(store.workspace.actions["redo"] != true)
                    }
                    Button("Reload") { send("reload") }.disabled(store.workspace.actions["reload"] != true)
                } label: { Image(systemName: "ellipsis.circle").frame(width: 44, height: 44) }.accessibilityLabel("Playlist editor actions")
            }.padding(.horizontal, 12).background(Palette.panel)
            if let error = store.workspace.error { Text(error).font(.caption).foregroundStyle(.red).padding(10) }
            if tab?.loading == true { ProgressView("Loading playlist…").padding() }
            if tab?.readonly == true { Text("Favorites · use star controls to change this list").font(.caption).foregroundStyle(Palette.muted).padding(8) }
            List {
                ForEach(store.workspace.entries.indices, id: \.self) { index in
                    let track = store.workspace.entries[index]
                    Button { store.workspaceSelect(index) } label: {
                        HStack(spacing: 10) {
                            Image(systemName: store.workspace.selected.contains(index) ? "checkmark.circle.fill" : "circle")
                                .foregroundStyle(Palette.accent)
                            FormatIcon(track: track)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(track.label).foregroundStyle(.white).lineLimit(1)
                                Text(track.detail).font(.caption).foregroundStyle(Palette.muted).lineLimit(1)
                            }
                        }.frame(minHeight: 44).contentShape(Rectangle())
                    }.buttonStyle(.plain).listRowBackground(Palette.window)
                }
            }.listStyle(.plain).scrollContentBackground(.hidden)
        }.background(Palette.window)
    }
    private func send(_ op: String) { store.workspaceCommand(["op": op]) }
    private func queue(_ action: String) { store.workspaceCommand(["op": "queue", "action": action]) }
}
