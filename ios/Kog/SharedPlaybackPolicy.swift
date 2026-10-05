import Foundation

@_silgen_name("kog_session_json")
private func sessionJSON(_ input: UnsafePointer<CChar>, _ error: UnsafeMutablePointer<CChar>, _ capacity: Int) -> UnsafeMutablePointer<CChar>?

/// A frontend holds one opaque instance. Session identities partition persistence
/// and every asynchronous completion; no queue state is supplied by the view.
final class SharedBackendSession {
    let id: String
    private let incarnation: UInt32
    private var state: String?
    private(set) var snapshot = [String: Any]()
    init(id: String, incarnation: UInt32 = UInt32.random(in: 1...UInt32.max)) { self.id = id; self.incarnation = incarnation }
    func send(_ command: [String: Any]? = nil, restore: Any? = nil) throws -> [String: Any] {
        var request: [String: Any] = ["session_id": id, "incarnation": incarnation]
        if let state { request["state"] = state }
        if let command { request["command"] = command }
        if let restore { request["restore"] = restore }
        let input = String(decoding: try JSONSerialization.data(withJSONObject: request), as: UTF8.self)
        var error = [CChar](repeating: 0, count: 2048)
        let capacity = error.count
        guard let reply = input.withCString({ sessionJSON($0, &error, capacity) }) else { throw KogError.response(String(cString: error)) }
        defer { playbackPolicyFree(reply) }
        guard let decoded = try JSONSerialization.jsonObject(with: Data(String(cString: reply).utf8)) as? [String: Any],
              let next = decoded["state"] as? String, let view = decoded["snapshot"] as? [String: Any] else { throw KogError.response("Invalid session reply") }
        state = next; snapshot = view; return decoded
    }
    var waiting: Bool { snapshot["radio_waiting"] as? Bool ?? false }
}

@_silgen_name("kog_policy_json")
private func playbackPolicy(_ input: UnsafePointer<CChar>, _ error: UnsafeMutablePointer<CChar>, _ capacity: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("kog_policy_string_free")
private func playbackPolicyFree(_ string: UnsafeMutablePointer<CChar>)

/// Serialization only. All ordering, radio lifecycle, and comparison rules
/// execute in the Rust policy also used by Qt, terminal, Web, and Android.
final class SharedPlaybackPolicy {
    private var state: String?
    private(set) var snapshot = [String: Any]()

    @discardableResult
    func send(_ command: [String: Any]) throws -> [String: Any] {
        var request: [String: Any] = ["command": command]
        if let state { request["state"] = state }
        let input = String(decoding: try JSONSerialization.data(withJSONObject: request), as: UTF8.self)
        var error = [CChar](repeating: 0, count: 2048)
        let capacity = error.count
        guard let reply = input.withCString({ playbackPolicy($0, &error, capacity) }) else {
            throw KogError.response(String(cString: error))
        }
        defer { playbackPolicyFree(reply) }
        guard let decoded = try JSONSerialization.jsonObject(with: Data(String(cString: reply).utf8)) as? [String: Any],
              let next = decoded["state"] as? String else {
            throw KogError.response("Invalid playback policy reply")
        }
        state = next; snapshot = decoded
        return decoded
    }

    func sync(_ tracks: [Track], current: Int, oldToNew: [Int?]? = nil) throws {
        var command: [String: Any] = ["op": "sync", "current": Self.index(current), "tracks": tracks.map { track in
            ["id": track.id, "album": track.album,
             "disc_number": Self.number(track.metadata["discNumber"]),
             "track_number": Self.number(track.metadata["trackNumber"])] as [String: Any]
        }]
        if let oldToNew { command["old_to_new"] = oldToNew.map { $0.map { $0 as Any } ?? NSNull() } }
        try send(command)
    }

    static func index(_ index: Int) -> Any { index >= 0 ? index as Any : NSNull() }
    private static func number(_ text: String?) -> Any {
        text.flatMap { Int($0.split(separator: "/").first.map(String.init) ?? "") }.map { $0 as Any } ?? NSNull()
    }
    var radio: [String: Any] { snapshot["radio"] as? [String: Any] ?? [:] }
    var generation: Int { radio["generation"] as? Int ?? 0 }
    var waiting: Bool { radio["waiting"] as? Bool ?? false }
    var needsRefill: Bool { radio["needs_refill"] as? Bool ?? false }
}
