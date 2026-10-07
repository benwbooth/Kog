import Foundation

struct ChannelField: Codable, Equatable, Sendable {
    var name: String
    var value: String
    var label: String { "\(name): \(value)" }
}
struct ChannelNote: Codable, Equatable, Sendable {
    var key: Double
    var velocity: Double
    var held: Bool
    var name: String {
        guard key.isFinite else { return "—" }
        let note = Int(key.rounded())
        return ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"][(note % 12 + 12) % 12] + String(Int(floor(Double(note) / 12)) - 1)
    }
}
struct MusicalChannel: Codable, Equatable, Identifiable, Sendable {
    var id: Int
    var name: String
    var kind: String
    var notes: [ChannelNote]
    var instrument: String
    var level: Double
    var pan: Double
    var active: Bool
    var fields: [ChannelField]
}
struct ChannelCell: Codable, Equatable, Sendable {
    var channel: Int
    var notes: String
    var instrument: String
    var volume: String
    var effects: [ChannelField]
}
struct ChannelRow: Codable, Equatable, Sendable {
    var time: Double
    var label: String
    var cells: [ChannelCell]
    var global: [ChannelField]
    func sameContents(as other: ChannelRow) -> Bool {
        label == other.label && cells == other.cells && global == other.global
    }
}
struct ChannelDescription: Codable, Sendable {
    var backend = ""
    var kind = "unavailable"
    var detail = "Play a track to inspect its channels."
}
struct ChannelSnapshot: Codable, Sendable {
    var version = 1
    var description = ChannelDescription()
    var position = 0.0
    var playing = false
    var seeking = false
    var channels = [MusicalChannel]()
    var rows = [ChannelRow]()
    var currentRow: Int?
    var global = [ChannelField]()
    static func decode(_ data: Data) throws -> Self {
        let decoder = JSONDecoder(); decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(Self.self, from: data)
    }
}
struct ChannelFrame: Codable, Sendable {
    var channels: [MusicalChannel]
    var row: ChannelRow?
    var global: [ChannelField]
}
struct ChannelDelta: Codable, Sendable {
    var time: Double
    var channels: [MusicalChannel]?
    var removed: [Int]?
    var global: [ChannelField]?
    var row: ChannelRow?
}
struct ChannelWindow: Codable, Sendable {
    var version: Int
    var description: ChannelDescription
    var start: Double
    var end: Double
    var initial: ChannelFrame
    var rows: [ChannelRow]
    var frames: [ChannelDelta]
    func contains(_ time: Double) -> Bool { time >= start && time < end }
    func snapshot(at position: Double, playing: Bool) -> ChannelSnapshot {
        guard contains(position) else { return ChannelSnapshot() }
        var channels = Dictionary(uniqueKeysWithValues: initial.channels.map { ($0.id, $0) })
        var global = initial.global
        var rows = self.rows
        for delta in frames {
            if delta.time <= position + 0.000001 {
                for id in delta.removed ?? [] { channels.removeValue(forKey: id) }
                for channel in delta.channels ?? [] { channels[channel.id] = channel }
                if let fields = delta.global { global = fields }
            }
            if let row = delta.row, rows.last?.sameContents(as: row) != true { rows.append(row) }
        }
        let cursor = rows.lastIndex(where: { $0.time <= position + 0.000001 })
        let begin = max(0, (cursor ?? -1) + 1 - 24)
        return ChannelSnapshot(version: version, description: description, position: position, playing: playing,
            channels: channels.values.sorted { $0.id < $1.id }, rows: Array(rows.dropFirst(begin).prefix(48)),
            currentRow: cursor.map { $0 - begin }, global: global)
    }
}
struct ChannelWindowReply: Decodable, Sendable {
    var status: String
    var window: ChannelWindow?
    var detail: String?
}
