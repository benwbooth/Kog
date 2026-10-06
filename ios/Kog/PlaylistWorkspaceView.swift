import SwiftUI

/// Shared contents for the application's Edit section and playlist menus.
struct PlaylistEditCommands: View {
    @EnvironmentObject private var store: KogStore
    var clearPlaylist: () -> Void
    private var isQueue: Bool { store.workspace.active == "queue" }
    private func enabled(_ action: String) -> Bool { store.workspace.actions[action] == true }
    private func send(_ op: String) { store.workspaceCommand(["op": op]) }
    var body: some View {
        Button(isQueue ? "Undo Append" : "Undo", systemImage: "arrow.uturn.backward") { send("undo") }.disabled(!enabled("undo"))
        Button(isQueue ? "Redo Append" : "Redo", systemImage: "arrow.uturn.forward") { send("redo") }.disabled(!enabled("redo"))
        Divider()
        Button("Select All", systemImage: "checkmark.circle") { store.workspaceCommand(["op": "selection", "command": ["op": "all"]]) }.disabled(!enabled("select_all"))
        Button("Clear Selection") { store.workspaceCommand(["op": "selection", "command": ["op": "clear"]]) }.disabled(!enabled("clear_selection"))
        Button("Remove Selected", systemImage: "minus.circle", role: .destructive) { send("remove") }.disabled(!enabled("remove"))
        Button(isQueue ? "Clear Play Queue" : "Clear Playlist", systemImage: "trash", role: .destructive, action: clearPlaylist).disabled(!enabled("clear"))
        Button("Move Up", systemImage: "arrow.up") { store.workspaceCommand(["op": "nudge", "delta": -1]) }.disabled(!enabled("move_up"))
        Button("Move Down", systemImage: "arrow.down") { store.workspaceCommand(["op": "nudge", "delta": 1]) }.disabled(!enabled("move_down"))
        if isQueue {
            Menu("Sort", systemImage: "arrow.up.arrow.down") {
                ForEach(TrackSort.all) { field in
                    Button { store.sortQueue(field.key) } label: {
                        Label(field.label, systemImage: store.sortKey == field.key ? (store.sortDescending ? "arrow.down" : "arrow.up") : "arrow.up.arrow.down")
                    }
                }
            }.disabled(store.queue.isEmpty)
        }
        Divider()
        Button("Save Changes", systemImage: "square.and.arrow.down") { send("save") }.disabled(!enabled("save"))
        Button("Reload Saved Playlist", systemImage: "arrow.clockwise") { send("reload") }.disabled(!enabled("reload"))
        Button("Add Play Queue") { store.workspaceAppendQueue() }.disabled(!enabled("add_play_queue"))
        Button("Add Queue Selection") { store.workspaceAppendQueue(selectedOnly: true) }.disabled(!enabled("add_queue_selection"))
    }
}

private struct PlaylistTabFrames: PreferenceKey {
    static let defaultValue: [String: CGRect] = [:]
    static func reduce(value: inout [String: CGRect], nextValue: () -> [String: CGRect]) {
        value.merge(nextValue(), uniquingKeysWith: { _, next in next })
    }
}

struct PlaylistWorkspaceTabs: View {
    @EnvironmentObject private var store: KogStore
    @State private var frames: [String: CGRect] = [:]
    @State private var dragging: String?
    @State private var dragPoint = CGPoint.zero
    @GestureState private var gestureActive = false
    private var validDrop: Bool {
        dragging != nil && store.workspace.pending_close == nil && frames["__strip"]?.contains(dragPoint) == true
    }
    private var dropBefore: String? {
        store.workspace.tabs.first { item in
            item.key != dragging && frames[item.key].map { dragPoint.x < $0.midX } == true
        }?.key
    }
    private func measure(_ key: String) -> some View {
        GeometryReader { geometry in
            Color.clear.preference(key: PlaylistTabFrames.self, value: [key: geometry.frame(in: .global)])
        }
    }
    private func reorder(_ key: String) -> some Gesture {
        LongPressGesture(minimumDuration: 0.35)
            .sequenced(before: DragGesture(minimumDistance: 0, coordinateSpace: .global))
            .updating($gestureActive) { value, active, _ in
                if case .second(true, _) = value { active = true }
            }
            .onChanged { value in
                if case .second(true, let drag?) = value, store.workspace.pending_close == nil {
                    dragging = key
                    dragPoint = drag.location
                }
            }
            .onEnded { value in
                if case .second(true, let drag?) = value {
                    dragPoint = drag.location
                    if validDrop {
                        store.workspaceCommand(["op": "move_tab", "key": key, "before": dropBefore as Any? ?? NSNull()])
                    }
                }
                dragging = nil
            }
    }
    private func scrollDuringDrag(_ proxy: ScrollViewProxy) {
        guard validDrop, let strip = frames["__strip"] else { return }
        let tabs = store.workspace.tabs
        if dragPoint.x < strip.minX + 28,
           let item = tabs.last(where: { frames[$0.key].map { $0.minX < strip.minX - 1 } == true }) {
            withAnimation(.linear(duration: 0.15)) { proxy.scrollTo(item.key, anchor: .leading) }
        } else if dragPoint.x > strip.maxX - 28,
                  let item = tabs.first(where: { frames[$0.key].map { $0.maxX > strip.maxX + 1 } == true }) {
            withAnimation(.linear(duration: 0.15)) { proxy.scrollTo(item.key, anchor: .trailing) }
        }
    }
    var body: some View {
        Group {
            if store.workspace.tabs.count > 1 {
                ScrollViewReader { proxy in
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
                                .opacity(dragging == item.key ? 0.5 : 1)
                                .background(measure(item.key))
                                .contentShape(Rectangle())
                                .highPriorityGesture(reorder(item.key))
                                .id(item.key)
                        }
                    }.padding(.horizontal, 6).padding(.vertical, 4)
                }.background(Palette.panel)
                    .background(measure("__strip"))
                    .scrollDisabled(dragging != nil)
                    .overlay(alignment: .topLeading) {
                        if validDrop, let strip = frames["__strip"] {
                            let edge = dropBefore.flatMap { frames[$0]?.minX }
                                ?? store.workspace.tabs.last(where: { $0.key != dragging }).flatMap { frames[$0.key]?.maxX }
                                ?? strip.minX
                            Rectangle().fill(Palette.accent).frame(width: 3, height: max(0, strip.height - 8))
                                .offset(x: max(0, min(strip.width - 3, edge - strip.minX)), y: 4)
                                .allowsHitTesting(false)
                        }
                    }
                    .onPreferenceChange(PlaylistTabFrames.self) { frames = $0 }
                    .onChange(of: gestureActive) { _, active in if !active { dragging = nil } }
                    .task(id: dragging) {
                        while dragging != nil && !Task.isCancelled {
                            scrollDuringDrag(proxy)
                            try? await Task.sleep(for: .milliseconds(180))
                        }
                    }
                }
            }
        }
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
                    PlaylistEditCommands { send("clear") }
                } label: { Label("Edit", systemImage: "pencil").frame(minHeight: 44) }.accessibilityLabel("Edit playlist")
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
                    }.buttonStyle(.plain)
                        .highPriorityGesture(TapGesture(count: 2).onEnded {
                            store.workspaceCommand(["op": "activate", "index": index])
                        })
                        .listRowBackground(Palette.window)
                }
            }.listStyle(.plain).scrollContentBackground(.hidden)
        }.background(Palette.window)
    }
    private func send(_ op: String) { store.workspaceCommand(["op": op]) }
    private func queue(_ action: String) { store.workspaceCommand(["op": "queue", "action": action]) }
}
