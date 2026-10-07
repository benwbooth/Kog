import Foundation

/// The song as Kog MML (docs/KOG_MML.md), split into bars, following playback.
struct MmlDocument: Decodable {
    struct Span: Decodable { let track: Int; let start: Int64; let end: Int64; let from: Int; let to: Int; let kind: String; let sound: Int64? }
    struct Bar: Decodable { let index: Int; let start: Int64; let end: Int64; let from: Int; let to: Int }
    struct Style { let from: Int; let to: Int; let style: Int }
    let text: String
    let tickSeconds: Double
    let spans: [Span]
    let bars: [Bar]
    /// Colour runs in text order; `style` indexes `palette`.
    let styles: [Style]
    let palette: [String]
    /// Tick and seconds pairs for songs whose tempo drifts; linear between them.
    let timing: [(tick: Int64, seconds: Double)]
    /// The text is ASCII, so byte offsets are character offsets.
    let bytes: [UInt8]

    enum CodingKeys: String, CodingKey { case text, tickSeconds = "tick_seconds", spans, bars, styles, palette, timing }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        text = try container.decode(String.self, forKey: .text)
        tickSeconds = try container.decode(Double.self, forKey: .tickSeconds)
        spans = try container.decode([Span].self, forKey: .spans)
        bars = try container.decode([Bar].self, forKey: .bars)
        styles = try container.decode([[Int]].self, forKey: .styles).map { Style(from: $0[0], to: $0[1], style: $0[2]) }
        palette = try container.decode([String].self, forKey: .palette)
        let points = try container.decodeIfPresent([[Double]].self, forKey: .timing) ?? []
        timing = points.map { (tick: Int64($0[0]), seconds: $0[1]) }
        bytes = Array(text.utf8)
    }

    func slice(_ from: Int, _ to: Int) -> String {
        String(decoding: bytes[max(0, from)..<min(bytes.count, max(from, to))], as: UTF8.self)
    }

    /// Colour runs overlapping from..to, found by binary search.
    func styles(from: Int, to: Int) -> ArraySlice<Style> {
        var low = 0, high = styles.count
        while low < high { let mid = (low + high) / 2; if styles[mid].to <= from { low = mid + 1 } else { high = mid } }
        var end = low
        while end < styles.count && styles[end].from < to { end += 1 }
        return styles[low..<end]
    }

    /// The tick playing `seconds` into the song (Rust's Document::tick_at).
    func tick(at seconds: Double) -> Int64 {
        let at = max(0, seconds)
        let after = timing.firstIndex { $0.seconds > at } ?? timing.count
        let before = after > 0 ? timing[after - 1] : nil
        let next = after < timing.count ? timing[after] : nil
        let tick: Double
        if let before, let next {
            tick = Double(before.tick) + (at - before.seconds) / max(next.seconds - before.seconds, .leastNonzeroMagnitude) * Double(next.tick - before.tick)
        } else if let before {
            tick = Double(before.tick) + (at - before.seconds) / tickSeconds
        } else if let next {
            tick = max(0, Double(next.tick) - (next.seconds - at) / tickSeconds)
        } else {
            tick = at / tickSeconds
        }
        return Int64(max(0, tick))
    }

    /// Every piece of each sounding note, plus the playing bar.
    func active(at seconds: Double) -> (spans: Set<Int>, bar: Int) {
        let tick = tick(at: seconds)
        var sounding = Set<String>()
        for span in spans where span.sound != nil && span.start <= tick && tick < span.end {
            sounding.insert("\(span.track):\(span.sound!)")
        }
        var lit = Set<Int>()
        for (index, span) in spans.enumerated() where span.sound != nil && sounding.contains("\(span.track):\(span.sound!)") {
            lit.insert(index)
        }
        return (lit, bars.first { $0.start <= tick && tick < $0.end }?.index ?? -1)
    }
}

struct MmlScoreReply: Decodable {
    let status: String
    let revision: Int64
    let recordedMs: Int64
    let totalMs: Int64
    let detail: String?
    let document: MmlDocument?
    enum CodingKeys: String, CodingKey {
        case status, revision, recordedMs = "recorded_ms", totalMs = "total_ms", detail, document
    }
}
