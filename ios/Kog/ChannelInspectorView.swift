import SwiftUI

struct ChannelInspectorView: View {
    @EnvironmentObject private var store: KogStore
    @Environment(\.dismiss) private var dismiss
    @State private var snapshot = ChannelSnapshot()
    @State private var mode = 2
    @State private var follow = true
    @State private var identity = ""
    @State private var windows = [ChannelWindow]()
    @State private var pending: Task<Void, Never>?
    @State private var requestGeneration = 0
    @State private var lastRequest = Date.distantPast

    var body: some View {
        NavigationStack {
            VStack(spacing: 10) {
                Picker("Inspector view", selection: $mode) {
                    Text("Keyboards").tag(0); Text("Tracker").tag(1); Text("Both").tag(2)
                }.pickerStyle(.segmented)
                Text(snapshot.description.backend).font(.caption.bold()).foregroundStyle(Palette.accent)
                Text(snapshot.description.detail).font(.caption2).foregroundStyle(Palette.muted)
                if !snapshot.global.isEmpty { Text(snapshot.global.map(\.label).joined(separator: " · ")).font(.caption2) }
                if mode != 1 {
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 14) {
                            ForEach(snapshot.channels) { channel in ChannelKeyboardView(channel: channel) }
                        }
                    }.frame(maxHeight: .infinity)
                }
                if mode != 0 {
                    Toggle("Follow playback", isOn: $follow).font(.caption)
                    ChannelTrackerView(snapshot: snapshot, follow: follow).frame(maxHeight: .infinity)
                }
            }
            .padding(12).background(Palette.window)
            .navigationTitle("Channel Inspector").navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button(store.playing ? "Pause" : "Play") { store.togglePlayback() } }
                ToolbarItem(placement: .topBarTrailing) { Button("Done") { dismiss() } }
            }
            .task {
                while !Task.isCancelled {
                    await update()
                    try? await Task.sleep(nanoseconds: 33_000_000)
                }
            }
            .onDisappear { requestGeneration += 1; pending?.cancel(); pending = nil }
        }
    }

    @MainActor private func update() async {
        guard store.channelInspectionActive, let track = store.current else { snapshot = ChannelSnapshot(); return }
        let stream = store.channelInspectionStream ?? ""
        let key = track.id + stream
        if identity != key {
            identity = key; windows.removeAll(); snapshot = ChannelSnapshot()
            requestGeneration += 1; pending?.cancel(); pending = nil; lastRequest = .distantPast
        }
        if track.isDevice {
            let result = await store.localChannelSnapshot()
            if store.current?.id == track.id { snapshot = result }
            return
        }
        let position = store.channelInspectionPosition
        let current = windows.first { $0.contains(position) }
        if let current { snapshot = current.snapshot(at: position, playing: store.playing) }
        else { snapshot = ChannelSnapshot(description: ChannelDescription(backend: "Remote decoder", kind: "pending", detail: "Waiting for channel data from the streaming decoder…")) }
        let requested: Double?
        if let current {
            requested = position > current.end - 0.3 && !windows.contains(where: { $0.start >= current.end }) ? current.end + 0.001 : nil
        } else { requested = position }
        guard let requested, pending == nil, Date().timeIntervalSince(lastRequest) >= 0.4 else { return }
        lastRequest = Date(); let generation = requestGeneration
        pending = Task { @MainActor in
            defer { if requestGeneration == generation { pending = nil } }
            do {
                let reply = try await store.api.channelWindow(stream: stream, position: requested)
                guard !Task.isCancelled, identity == key, requestGeneration == generation else { return }
                if let window = reply.window {
                    windows.removeAll { $0.start == window.start }; windows.append(window)
                    if windows.count > 3 { windows.removeFirst(windows.count - 3) }
                } else if windows.isEmpty { snapshot.description.detail = reply.detail ?? "Waiting for channel data…" }
            } catch {
                if !Task.isCancelled, identity == key, windows.isEmpty { snapshot.description.detail = error.localizedDescription }
            }
        }
    }
}

private struct ChannelKeyboardView: View {
    let channel: MusicalChannel
    @State private var details = false
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(channel.name).font(.subheadline.bold())
            Text(channel.instrument).font(.caption).foregroundStyle(Palette.muted)
            Text(channel.notes.isEmpty ? (channel.active ? channel.kind.uppercased() : "—") : channel.notes.map { $0.name + ($0.held ? "" : "~") }.joined(separator: " "))
                .font(.system(.caption, design: .monospaced))
            ProgressView(value: min(1, max(0, channel.level)))
            ScrollView(.horizontal) {
                Canvas { context, size in
                    let whiteWidth = size.width / 75
                    for pass in [false, true] {
                        var white = 0
                        for key in 0...127 {
                            let black = [1,3,6,8,10].contains(key % 12)
                            if black == pass {
                                let note = channel.notes.first { Int($0.key.rounded()) == key }
                                let x = Double(white) * whiteWidth - (black ? whiteWidth * 0.32 : 0)
                                let width = black ? whiteWidth * 0.64 : whiteWidth - 1
                                let height = black ? size.height * 0.62 : size.height
                                let color: Color = note.map { $0.held ? Color(red:0.31,green:0.76,blue:0.97) : Color(red:0.44,green:0.85,blue:0.67) } ?? (black ? Color(red:0.09,green:0.11,blue:0.13) : Color(red:0.91,green:0.92,blue:0.94))
                                context.fill(Path(CGRect(x:x,y:0,width:width,height:height)),with:.color(color))
                                if !black && key % 12 == 0 {
                                    context.draw(Text("C\(key / 12 - 1)").font(.system(size:8)).foregroundColor(.black),at:CGPoint(x:x+4,y:size.height-5))
                                }
                                if let note, abs(note.key - note.key.rounded()) > 0.02 {
                                    let bend = x + width/2 + (note.key-note.key.rounded())*width
                                    context.fill(Path(CGRect(x:bend-1,y:2,width:2,height:height-4)),with:.color(.orange))
                                }
                            }
                            if !black { white += 1 }
                        }
                    }
                }.frame(width:760,height:64).accessibilityLabel("\(channel.name) active piano keys")
            }
            DisclosureGroup("Controls and effects", isExpanded: $details) {
                Text(channel.fields.map(\.label).joined(separator:" · ")).font(.caption2).frame(maxWidth:.infinity,alignment:.leading)
            }.font(.caption)
        }.padding(8).background(Palette.panel,in:RoundedRectangle(cornerRadius:8))
    }
}

private struct ChannelTrackerView: View {
    let snapshot: ChannelSnapshot
    let follow: Bool
    var body: some View {
        ScrollViewReader { reader in
            ScrollView([.horizontal,.vertical]) {
                LazyVStack(alignment:.leading,spacing:0) {
                    HStack(spacing:0) {
                        Text("Time / row").frame(width:90,alignment:.leading)
                        ForEach(snapshot.channels) { channel in Text(channel.name).frame(width:190,alignment:.leading) }
                        Text("Song data").frame(width:190,alignment:.leading)
                    }.font(.caption.bold()).padding(.vertical,4)
                    ForEach(Array(snapshot.rows.enumerated()),id:\.offset) { index,row in
                        HStack(alignment:.top,spacing:0) {
                            Text(row.label).frame(width:90,alignment:.leading)
                            ForEach(snapshot.channels) { channel in
                                let cells = row.cells.filter { $0.channel == channel.id }
                                VStack(alignment:.leading,spacing:3) {
                                    Text(cells.map { "\($0.notes) \($0.instrument) \($0.volume)" }.joined(separator:" · ")).foregroundStyle(Palette.accent)
                                    Text(cells.flatMap(\.effects).map(\.label).joined(separator:" · ")).font(.caption2)
                                }.frame(width:180,alignment:.leading).padding(.horizontal,5)
                            }
                            Text(row.global.map(\.label).joined(separator:" · ")).frame(width:190,alignment:.leading)
                        }.font(.system(.caption,design:.monospaced)).padding(.vertical,4)
                            .background(snapshot.currentRow == index ? Palette.accent.opacity(0.22) : Color.clear).id(index)
                    }
                }
            }
            .onChange(of:snapshot.currentRow) { _,index in if follow,let index { reader.scrollTo(index,anchor:.center) } }
            .onChange(of:snapshot.rows.first?.time) { _,_ in if follow,let index=snapshot.currentRow { reader.scrollTo(index,anchor:.center) } }
        }
    }
}
