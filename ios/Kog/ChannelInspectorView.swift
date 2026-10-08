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
                    Text("Keyboards").tag(0); Text("Tracker").tag(1); Text("Both").tag(2); Text("MML").tag(3)
                }.pickerStyle(.segmented)
                Text(snapshot.description.backend).font(.caption.bold()).foregroundStyle(Palette.accent)
                Text(snapshot.description.detail).font(.caption2).foregroundStyle(Palette.muted)
                if !snapshot.global.isEmpty { Text(snapshot.global.map(\.label).joined(separator: " · ")).font(.caption2) }
                if mode == 3 {
                    Toggle("Follow playback", isOn: $follow).font(.caption)
                    MmlScoreView(follow: follow).frame(maxHeight: .infinity)
                }
                if mode == 0 || mode == 2 {
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 14) {
                            ForEach(snapshot.channels) { channel in ChannelKeyboardView(channel: channel) }
                        }
                    }.frame(maxHeight: .infinity)
                }
                if mode == 1 || mode == 2 {
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
            ChannelLevelMeter(channel: channel)
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
                                if let note, !channel.fields.contains(where: { $0.name == "Pitch basis" && $0.value.hasPrefix("Relative") }), abs(note.key - note.key.rounded()) > 0.02 {
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

private struct ChannelLevelMeter: View {
    let channel: MusicalChannel
    private var level: Double { channel.level.isFinite ? min(1, max(0, channel.level)) : 0 }
    var body: some View {
        HStack(spacing: 8) {
            Text("Level").font(.caption2).foregroundStyle(Palette.muted)
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    Capsule().fill(Palette.muted.opacity(0.25))
                    Capsule().fill(Palette.accent).frame(width: geometry.size.width * level)
                }
            }.frame(height: 8)
            Text("\(Int((level * 100).rounded()))%")
                .font(.system(.caption2, design: .monospaced)).foregroundStyle(Palette.muted)
                .frame(width: 36, alignment: .trailing)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(channel.name) level")
        .accessibilityValue("\(Int((level * 100).rounded())) percent")
    }
}

private struct ChannelTrackerView: View {
    let snapshot: ChannelSnapshot
    let follow: Bool
    /// Column widths in characters per channel; they grow to fit the rows
    /// shown so columns stay put, and start over for a new source.
    @State private var widths = [Int: [Int]]()
    @State private var source = ""
    // The Spleen 6x12 pixel font, as in the desktop and web trackers.
    private let pixel = Font.custom("Spleen 6x12", fixedSize: 12)
    private let char: CGFloat = 6
    private let colours: [Color] = [Color(red: 0.93, green: 0.96, blue: 0.97), Color(red: 0.35, green: 0.78, blue: 0.94),
                                    Color(red: 0.56, green: 0.86, blue: 0.49), Color(red: 0.89, green: 0.61, blue: 0.95)]
    private func chars(_ channel: Int) -> [Int] { widths[channel] ?? TrackerColumns.limits.map(\.0) }
    private func columnWidth(_ channel: Int) -> CGFloat { CGFloat(chars(channel).reduce(0, +) + 3) * char + 10 }
    private func fit() {
        let next = "\(snapshot.description.backend)/\(snapshot.channels.count)"
        var grown = next == source ? widths : [:]
        for row in snapshot.rows {
            for channel in snapshot.channels {
                let parts = TrackerColumns.parts(row.cells.filter { $0.channel == channel.id })
                let current = grown[channel.id] ?? TrackerColumns.limits.map(\.0)
                grown[channel.id] = current.indices.map { max(current[$0], min(TrackerColumns.limits[$0].1, parts[$0].count)) }
            }
        }
        source = next
        if grown != widths { widths = grown }
    }
    var body: some View {
        ScrollViewReader { reader in
            ScrollView([.horizontal,.vertical]) {
                LazyVStack(alignment:.leading,spacing:0) {
                    HStack(spacing:0) {
                        Text("ROW").foregroundStyle(Color(red: 0.42, green: 0.52, blue: 0.58)).frame(width:char * 13,alignment:.leading)
                        ForEach(Array(snapshot.channels.enumerated()),id:\.element.id) { index,channel in
                            Text(String(format:"%02d ",index + 1) + channel.name).lineLimit(1)
                                .foregroundStyle(Color(red: 0.66, green: 0.77, blue: 0.82))
                                .frame(width:columnWidth(channel.id),alignment:.leading)
                        }
                        Text("SONG").foregroundStyle(Color(red: 0.42, green: 0.52, blue: 0.58))
                    }.padding(.vertical,3).padding(.leading,5).background(Color(red: 0.06, green: 0.1, blue: 0.13))
                    ForEach(Array(snapshot.rows.enumerated()),id:\.offset) { index,row in
                        let current = snapshot.currentRow == index
                        HStack(spacing:0) {
                            Text(row.label).foregroundStyle(current ? .white : Color(red: 0.85, green: 0.72, blue: 0.35))
                                .frame(width:char * 13,alignment:.leading)
                            ForEach(snapshot.channels) { channel in
                                let parts = TrackerColumns.parts(row.cells.filter { $0.channel == channel.id })
                                let chars = chars(channel.id)
                                HStack(spacing:char) {
                                    ForEach(0..<4,id:\.self) { column in
                                        let blank = parts[column].isEmpty
                                        Text(blank ? (column == 0 ? "---" : "..") : parts[column]).lineLimit(1)
                                            .foregroundStyle(blank ? Color(red: 0.2, green: 0.27, blue: 0.31) : colours[column])
                                            .frame(width:CGFloat(chars[column]) * char,alignment:.leading).clipped()
                                    }
                                }
                                .padding(.leading,5)
                                .frame(width:columnWidth(channel.id),alignment:.leading)
                                .overlay(alignment:.leading) { Rectangle().fill(Color(red: 0.16, green: 0.23, blue: 0.27)).frame(width:1) }
                            }
                            Text(row.global.map { "\($0.name) \($0.value)" }.joined(separator:" ")).lineLimit(1)
                                .foregroundStyle(Color(red: 0.51, green: 0.83, blue: 0.73))
                        }
                        .padding(.leading,5).frame(height:15)
                        .background(current ? Color(red: 0.11, green: 0.29, blue: 0.37) : index % 4 == 0 ? Color(red: 0.06, green: 0.09, blue: 0.11) : Color.clear)
                        .id(index)
                    }
                }
                .font(pixel)
            }
            .background(Color(red: 0.03, green: 0.04, blue: 0.05))
            .onAppear(perform: fit)
            .onChange(of:snapshot.rows) { _,_ in fit() }
            .onChange(of:snapshot.currentRow) { _,index in if follow,let index { reader.scrollTo(index,anchor:.center) } }
            .onChange(of:snapshot.rows.first?.time) { _,_ in if follow,let index=snapshot.currentRow { reader.scrollTo(index,anchor:.center) } }
        }
    }
}
