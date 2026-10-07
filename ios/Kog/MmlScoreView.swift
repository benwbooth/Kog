import SwiftUI

struct MmlScoreView: View {
    @EnvironmentObject private var store: KogStore
    let follow: Bool
    @State private var document: MmlDocument?
    @State private var message = "Recording every channel of this song…"
    @State private var active = (spans: Set<Int>(), bar: -1)
    @State private var identity = ""
    @State private var revision: Int64 = -1
    @State private var done = false
    @State private var lastRequest = Date.distantPast
    @State private var barCache = [Int: AttributedString]()
    @AppStorage("mmlBarsPerLine") private var bars = 4
    @State private var showGuide = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Button("Guide") { showGuide = true }.font(.caption)
                Spacer()
            }
            .sheet(isPresented: $showGuide) { MmlGuideView() }
            Stepper("Bars per line: \(bars)", value: $bars, in: 1...16)
                .font(.caption)
                .onChange(of: bars) { _, _ in
                    // A new width needs the whole text again; the score is kept.
                    revision = -1; done = false; lastRequest = .distantPast
                }
            if !message.isEmpty { Text(message).font(.caption2).foregroundStyle(Palette.muted) }
            if let document {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            Text(styled(document, 0, document.bars.first?.from ?? 0, []))
                                .font(.system(.caption2, design: .monospaced)).padding(8)
                            ForEach(document.bars, id: \.index) { bar in
                                let playing = bar.index == active.bar
                                HStack(spacing: 0) {
                                    Rectangle().fill(playing ? Palette.accent : .clear).frame(width: 3)
                                    // Other bars come from the cache; only the playing bar is restyled.
                                    Text(playing ? styled(document, bar.from, bar.to, active.spans) : cached(document, bar))
                                        .font(.system(.caption, design: .monospaced))
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                        .padding(.horizontal, 8).padding(.vertical, 6)
                                }
                                .background(playing ? Color(red: 0.07, green: 0.19, blue: 0.23)
                                            : bar.index % 2 == 0 ? Color(red: 0.06, green: 0.09, blue: 0.11) : Color(red: 0.07, green: 0.11, blue: 0.13))
                                .id(bar.index)
                            }
                        }
                    }
                    .onChange(of: active.bar) { _, bar in
                        if follow && bar >= 0 { withAnimation { proxy.scrollTo(bar, anchor: .top) } }
                    }
                }
            }
        }
        .task {
            while !Task.isCancelled {
                await update()
                try? await Task.sleep(nanoseconds: 50_000_000)
            }
        }
    }

    private func cached(_ document: MmlDocument, _ bar: MmlDocument.Bar) -> AttributedString {
        if let text = barCache[bar.index] { return text }
        let text = styled(document, bar.from, bar.to, [])
        DispatchQueue.main.async { barCache[bar.index] = text }
        return text
    }

    private func colour(_ hex: String) -> Color {
        let value = UInt32(hex.dropFirst(), radix: 16) ?? 0xdce3e8
        return Color(red: Double(value >> 16 & 0xff) / 255, green: Double(value >> 8 & 0xff) / 255, blue: Double(value & 0xff) / 255)
    }

    private func styled(_ document: MmlDocument, _ from: Int, _ to: Int, _ lit: Set<Int>) -> AttributedString {
        let ranges = lit.compactMap { document.spans.indices.contains($0) ? document.spans[$0] : nil }
            .filter { $0.from >= from && $0.to <= to }.map { $0.from..<$0.to }
        var result = AttributedString()
        var cursor = from
        for run in document.styles(from: from, to: to) {
            let start = max(run.from, from), end = min(run.to, to)
            result += AttributedString(document.slice(cursor, start))
            var piece = AttributedString(document.slice(start, end))
            if ranges.contains(where: { $0.contains(start) && end <= $0.upperBound }) {
                piece.backgroundColor = Palette.accent; piece.foregroundColor = .black
                piece.inlinePresentationIntent = .stronglyEmphasized
            } else {
                piece.foregroundColor = colour(document.palette[min(run.style, document.palette.count - 1)])
                if run.style == 2 || run.style == 4 { piece.inlinePresentationIntent = .stronglyEmphasized }
            }
            result += piece
            cursor = end
        }
        result += AttributedString(document.slice(cursor, to).trimmingCharacters(in: .newlines))
        return result
    }

    @MainActor private func update() async {
        guard let track = store.current else {
            document = nil; message = "Play a song to see its MML score."; identity = ""; return
        }
        let stream = store.channelInspectionStream ?? ""
        let key = track.id + stream
        if identity != key {
            identity = key; document = nil; revision = -1; done = false; lastRequest = .distantPast; barCache = [:]
            message = "Recording every channel of this song…"
        }
        if !done && Date().timeIntervalSince(lastRequest) > 1 {
            lastRequest = Date()
            do {
                let reply: MmlScoreReply? = track.isDevice
                    ? await store.localMmlScore(have: revision, bars: bars)
                    : try await store.api.mmlScore(stream: stream, have: revision, bars: bars)
                guard identity == key, let reply else { return }
                if let next = reply.document { document = next; revision = reply.revision; barCache = [:] }
                let time = { (ms: Int64) in String(format: "%d:%02d", ms / 60_000, ms / 1000 % 60) }
                switch reply.status {
                case "ready": done = true; message = ""
                case "error": done = true; message = reply.detail ?? "The score could not be recorded."
                default: message = "Still recording… \(time(reply.recordedMs)) of \(time(reply.totalMs))"
                }
            } catch {
                message = error.localizedDescription
            }
        }
        if let document {
            var seconds = store.channelInspectionPosition
            if let components = URLComponents(string: stream),
               let start = components.queryItems?.first(where: { $0.name == "start_ms" })?.value.flatMap(Double.init) {
                seconds += start / 1000
            }
            active = document.active(at: seconds)
        }
    }
}
