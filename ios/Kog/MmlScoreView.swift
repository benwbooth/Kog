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

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !message.isEmpty { Text(message).font(.caption2).foregroundStyle(Palette.muted) }
            if let document {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 6) {
                            Text(document.slice(0, document.bars.first?.from ?? 0))
                                .font(.system(.caption2, design: .monospaced)).foregroundStyle(Palette.muted)
                            ForEach(document.bars, id: \.index) { bar in
                                let playing = bar.index == active.bar
                                Text(attributed(document, bar, playing ? active.spans : []))
                                    .font(.system(.caption, design: .monospaced))
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .padding(8)
                                    .background(playing ? Color(red: 0.09, green: 0.22, blue: 0.27) : Color(red: 0.07, green: 0.12, blue: 0.15))
                                    .clipShape(RoundedRectangle(cornerRadius: 6))
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

    private func attributed(_ document: MmlDocument, _ bar: MmlDocument.Bar, _ lit: Set<Int>) -> AttributedString {
        var result = AttributedString()
        var cursor = bar.from
        for (index, span) in document.spans.enumerated() where span.from >= bar.from && span.to <= bar.to {
            var piece = AttributedString(document.slice(span.from, span.to))
            if lit.contains(index) {
                piece.backgroundColor = Palette.accent; piece.foregroundColor = .black
                piece.inlinePresentationIntent = .stronglyEmphasized
            } else if span.kind == "rest" {
                piece.foregroundColor = Palette.muted
            } else if span.kind == "command" {
                piece.foregroundColor = Color(red: 0.51, green: 0.83, blue: 0.73)
            } else {
                continue
            }
            result += AttributedString(document.slice(cursor, span.from))
            result += piece
            cursor = span.to
        }
        result += AttributedString(document.slice(cursor, bar.to).trimmingCharacters(in: .newlines))
        return result
    }

    @MainActor private func update() async {
        guard let track = store.current else {
            document = nil; message = "Play a song to see its MML score."; identity = ""; return
        }
        let stream = store.channelInspectionStream ?? ""
        let key = track.id + stream
        if identity != key {
            identity = key; document = nil; revision = -1; done = false; lastRequest = .distantPast
            message = "Recording every channel of this song…"
        }
        if !done && Date().timeIntervalSince(lastRequest) > 1 {
            lastRequest = Date()
            do {
                let reply: MmlScoreReply? = track.isDevice
                    ? await store.localMmlScore(have: revision)
                    : try await store.api.mmlScore(stream: stream, have: revision)
                guard identity == key, let reply else { return }
                if let next = reply.document { document = next; revision = reply.revision }
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
