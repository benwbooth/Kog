import Foundation

/// The song as Kog MML (docs/KOG_MML.md), split into bars, following playback.
struct MmlDocument: Decodable {
    struct Span: Decodable { let track: Int; let start: Int64; let end: Int64; let from: Int; let to: Int; let kind: String; let sound: Int64? }
    struct Bar: Decodable { let index: Int; let start: Int64; let end: Int64; let from: Int; let to: Int }
    let text: String
    let tickSeconds: Double
    let spans: [Span]
    let bars: [Bar]

    enum CodingKeys: String, CodingKey { case text, tickSeconds = "tick_seconds", spans, bars }

    /// The text is ASCII, so byte offsets are character offsets.
    func slice(_ from: Int, _ to: Int) -> String {
        let bytes = Array(text.utf8)
        return String(decoding: bytes[max(0, from)..<min(bytes.count, to)], as: UTF8.self)
    }

    /// Every piece of each sounding note, plus the playing bar.
    func active(at seconds: Double) -> (spans: Set<Int>, bar: Int) {
        let tick = Int64(max(0, seconds) / tickSeconds)
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
