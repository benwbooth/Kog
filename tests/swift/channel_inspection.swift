// Compile together with ios/Kog/ChannelInspection.swift. Pass native snapshot
// and stream-window JSON files exported by KOG_INSPECTION_FIXTURES.
import Foundation

@main
struct ChannelInspectionContract {
    static func main() throws {
        var snapshots = 0
        var windows = 0
        for path in CommandLine.arguments.dropFirst() {
            let data = try Data(contentsOf: URL(fileURLWithPath: path))
            let object = try JSONSerialization.jsonObject(with: data) as! [String: Any]
            if object["frames"] != nil {
                let decoder = JSONDecoder()
                decoder.keyDecodingStrategy = .convertFromSnakeCase
                let window = try decoder.decode(ChannelWindow.self, from: data)
                let position = window.start + min(0.25, (window.end - window.start) / 2)
                let playing = window.snapshot(at: position, playing: true)
                let paused = window.snapshot(at: position, playing: false)
                precondition(!playing.channels.isEmpty)
                precondition(playing.channels == paused.channels)
                precondition(playing.rows == paused.rows)
                precondition(window.snapshot(at: window.start - 1, playing: true).channels.isEmpty)
                precondition(playing.currentRow.map { playing.rows.indices.contains($0) } ?? true)
                windows += 1
            } else {
                let snapshot = try ChannelSnapshot.decode(data)
                precondition(!snapshot.channels.isEmpty)
                precondition(Set(snapshot.channels.map(\.id)).count == snapshot.channels.count)
                precondition(snapshot.currentRow.map { snapshot.rows.indices.contains($0) } ?? true)
                snapshots += 1
            }
        }
        precondition(snapshots > 0)
        precondition(ChannelNote(key: 60, velocity: 1, held: true).name == "C4")
        print("Decoded \(snapshots) native snapshots and \(windows) stream windows; pause and clock selection passed")
    }
}
