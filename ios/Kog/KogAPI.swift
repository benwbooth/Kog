import Foundation
import Security

enum Secrets {
    static func read(_ name: String) -> String {
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
                                    kSecAttrService as String: "org.kog.player",
                                    kSecAttrAccount as String: name,
                                    kSecReturnData as String: true,
                                    kSecMatchLimit as String: kSecMatchLimitOne]
        var result: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data else { return "" }
        return String(data: data, encoding: .utf8) ?? ""
    }

    static func write(_ name: String, _ value: String) {
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
                                    kSecAttrService as String: "org.kog.player",
                                    kSecAttrAccount as String: name]
        SecItemDelete(query as CFDictionary)
        guard !value.isEmpty else { return }
        var attributes = query
        attributes[kSecValueData as String] = Data(value.utf8)
        SecItemAdd(attributes as CFDictionary, nil)
    }
}

struct KogAPI {
    var server: String
    var token: String
    var username: String
    var password: String
    var codec: String
    var midiEngine: String = "opl3windows"
    var deviceRoot: String?
    var deviceStorage: String?
    var sessionID: String = ""
    var radioRequest: [String: Any]? = nil

    func url(_ endpoint: String, _ query: [String: String] = [:]) throws -> URL {
        let origin = server.contains("://") ? server : "http://\(server)"
        guard var components = URLComponents(string: origin.trimmingCharacters(in: .whitespacesAndNewlines)),
              components.host != nil else { throw KogError.invalidServer }
        let prefix = components.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        components.path = prefix.isEmpty ? endpoint : "/\(prefix)\(endpoint)"
        components.queryItems = query.isEmpty ? nil : query.sorted { $0.key < $1.key }.map { URLQueryItem(name: $0.key, value: $0.value) }
        guard let url = components.url else { throw KogError.invalidServer }
        return url
    }

    var audioHeaders: String {
        if !token.isEmpty { return "Authorization: Bearer \(token)\r\n" }
        if !username.isEmpty { return "Authorization: Basic \(Data("\(username):\(password)".utf8).base64EncodedString())\r\n" }
        return ""
    }
    func nativeStream(_ track: Track, start: Double = 0) throws -> URL {
        var parts = URLComponents(url: try stream(track), resolvingAgainstBaseURL: false)!
        parts.queryItems = parts.queryItems?.filter { $0.name != "token" }
        if start > 0 { parts.queryItems?.append(URLQueryItem(name: "start_ms", value: String(Int64(start * 1000)))) }
        return parts.url!
    }
    func stream(_ track: Track) throws -> URL {
        if track.isDevice { return URL(fileURLWithPath: track.path) }
        var options = track.locator.merging(["codec": codec, "token": token]) { _, new in new }
        let name = track.kind == "archive" ? track.entry : track.path
        let suffix = URL(fileURLWithPath: name).pathExtension.lowercased()
        if ["kar", "mid", "midi", "rmi", "mids", "mds", "lds", "xmf", "mxmf"].contains(suffix) {
            options["midi_engine"] = midiEngine
        }
        return try url("/api/stream", options)
    }

    func setMidiEngine(_ engine: String) async throws {
        _ = try await request("/api/settings/midi", method: "POST", body: ["engine": engine])
    }

    func serverMidiEngine() async throws -> String {
        let data = try await request("/api/settings/midi")
        let object = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        guard let engine = object?["engine"] as? String else {
            throw KogError.response("MIDI synth setting is missing")
        }
        return engine
    }

    func art(_ track: Track) -> URL? {
        guard !track.isDevice else { return nil }
        return try? url("/api/art", ["kind": track.kind, "path": track.path, "token": token])
    }

    func artData(_ track: Track) async throws -> Data {
        try await request("/api/art", query: ["kind": track.kind, "path": track.path])
    }

    private func authenticatedRequest(_ endpoint: String, query: [String: String] = [:]) throws -> URLRequest {
        var request = URLRequest(url: try url(endpoint, query))
        request.timeoutInterval = 45
        if !token.isEmpty {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        } else if !username.isEmpty {
            let raw = Data("\(username):\(password)".utf8).base64EncodedString()
            request.setValue("Basic \(raw)", forHTTPHeaderField: "Authorization")
        }
        return request
    }

    func download(_ track: Track) async throws -> (URL, String) {
        guard track.kind == "local" || track.kind == "archive" else {
            throw KogError.response("Only server files and archive members can be saved on this iPhone")
        }
        var request = try authenticatedRequest("/api/media/download", query: [
            "kind": track.kind, "path": track.path, "entry": track.entry,
        ])
        request.timeoutInterval = 3600
        let (temporary, response) = try await URLSession.shared.download(for: request)
        guard let response = response as? HTTPURLResponse else {
            throw KogError.response("No server response")
        }
        guard (200..<300).contains(response.statusCode) else {
            throw KogError.response("Download failed (HTTP \(response.statusCode))")
        }
        let sourceName = track.kind == "archive" ? track.entry : track.path
        let filename = URL(fileURLWithPath: sourceName).lastPathComponent
        guard !filename.isEmpty && filename != "." && filename != ".." else {
            throw KogError.response("The server file has no usable name")
        }
        return (temporary, filename)
    }

    private func request(_ endpoint: String, query: [String: String] = [:],
                         method: String = "GET", body: Any? = nil, timeout: TimeInterval = 45) async throws -> Data {
        #if KOG_NATIVE_AUDIO
        if let deviceRoot, let deviceStorage {
            var components = URLComponents(); components.path = endpoint
            components.queryItems = query.isEmpty ? nil : query.map { URLQueryItem(name: $0.key, value: $0.value) }
            let uri = components.string ?? endpoint
            let data = try body.map { try JSONSerialization.data(withJSONObject: $0) }
            return try await Task.detached(priority: .userInitiated) {
                try NativeAudioCatalog.request(root: deviceRoot, storage: deviceStorage, uri: uri, method: method, body: data)
            }.value
        }
        #endif
        var request = try authenticatedRequest(endpoint, query: query)
        request.timeoutInterval = timeout
        request.httpMethod = method
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }
        let (data, response) = try await URLSession.shared.data(for: request)
        guard let response = response as? HTTPURLResponse else { throw KogError.response("No server response") }
        guard (200..<300).contains(response.statusCode) else {
            let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
            throw KogError.response((object?["error"] as? String) ?? "HTTP \(response.statusCode)")
        }
        return data
    }

    func health() async throws { _ = try await request("/api/health", timeout: 5) }
    func connection() async throws { _ = try await request("/api/config", timeout: 5) }
    func browse(_ path: String = "") async throws -> Listing {
        var listing = try JSONDecoder().decode(Listing.self, from: await request("/api/library", query: ["path": path]))
        if deviceRoot != nil { listing.files = listing.files.map { $0.onDevice() } }
        return listing
    }
    func collect(_ path: String, query: String = "", root: String = "") async throws -> [Track] {
        let data = try await request("/api/library/collect", query: ["path": path, "q": query, "root": root], timeout: 300)
        let rows = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["tracks"] ?? []
        return try await metadata(decodeTracks(rows))
    }
    func expand(_ track: Track) async throws -> [Track] { try await expand([track]) }
    func expand(_ entries: [Track]) async throws -> [Track] {
        let data = try await request("/api/expand", method: "POST", body: entries.map { $0.locator.merging(["name": $0.name]) { _, new in new } })
        let groups = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["tracks"] as? [[Any]] ?? []
        return try await metadata(decodeTracks(groups.flatMap { $0 }))
    }
    func metadata(_ tracks: [Track]) async throws -> [Track] {
        guard !tracks.isEmpty else { return [] }
        var result = [Track]()
        for start in stride(from: 0, to: tracks.count, by: 100) {
            let chunk = Array(tracks[start..<min(start + 100, tracks.count)])
            let data = try await request("/api/metadata", method: "POST", body: chunk.map(\.locator), timeout: 300)
            let rows = (try JSONSerialization.jsonObject(with: data) as? [[String: Any]]) ?? []
            for (index, var track) in chunk.enumerated() {
                if index < rows.count {
                    let row = rows[index]
                    track.title = row["title"] as? String ?? ""
                    track.artist = row["artist"] as? String ?? ""
                    track.album = row["album"] as? String ?? ""
                    for field in TrackSort.metadataFields {
                        if let value = row[field.key], !(value is NSNull) { track.metadata[field.key] = String(describing: value) }
                    }
                    track.duration = Int64(((row["duration"] as? NSNumber)?.doubleValue ?? 0) * 1000)
                }
                result.append(track)
            }
        }
        return result
    }

    func search(_ term: String, root: String) async throws -> SearchPage {
        try JSONDecoder().decode(SearchPage.self, from: await request("/api/library/search", query: ["q": term, "root": root, "session": sessionID]))
    }
    func more(_ generation: Int64, offset: Int) async throws -> SearchPage {
        try JSONDecoder().decode(SearchPage.self, from: await request("/api/library/search/more", query: ["g": "\(generation)", "offset": "\(offset)", "session": sessionID]))
    }
    func playlists() async throws -> [SavedPlaylist] {
        let data = try await request("/api/playlists")
        let rows = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["playlists"] ?? []
        let decodedData = try JSONSerialization.data(withJSONObject: rows)
        return try decodedData.withDecoded([SavedPlaylist].self)
    }
    func playlist(_ id: Int64) async throws -> [Track] {
        let data = try await request("/api/playlists/\(id)")
        let rows = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["entries"] ?? []
        return try await metadata(decodeTracks(rows))
    }
    func createPlaylist(_ name: String) async throws -> Int64 {
        let data = try await request("/api/playlists", method: "POST", body: ["name": name])
        guard let id = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["id"] as? Int64 else {
            throw KogError.response("Playlist ID is missing")
        }
        return id
    }
    func duplicatePlaylist(_ id: Int64, name: String) async throws {
        _ = try await request("/api/playlists/\(id)/duplicate", method: "POST", body: ["name": name])
    }
    func exportPlaylist(_ id: Int64) async throws -> String {
        let data = try await request("/api/playlists/\(id)/export")
        guard let text = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["text"] as? String else { throw KogError.response("No playlist export was returned") }
        return text
    }
    func prunePlaylist(_ id: Int64) async throws {
        _ = try await request("/api/playlists/\(id)/prune-missing", method: "POST")
    }
    func replacePlaylist(_ id: Int64, tracks: [Track], expected: [Track]? = nil) async throws {
        guard tracks.allSatisfy({ $0.kind == "remote" || $0.isDevice == (deviceRoot != nil) }) else { throw KogError.response("Choose a playlist in the same library as these tracks.") }
        var body: [String: Any] = ["entries": tracks.map(\.locator)]
        if let expected { body["expected_entries"] = expected.map(\.locator) }
        _ = try await request("/api/playlists/\(id)/entries", method: "PUT", body: body)
    }
    func pauseSearch(_ paused: Bool) async throws {
        _ = try await request("/api/library/search/pause", query: ["session": sessionID], method: "POST", body: ["paused": paused])
    }
    private func radioQuery(_ root: String) -> [String: String] {
        var query = ["incremental": "true", "session": sessionID]
        if let token = radioRequest {
            query["incarnation"] = (token["incarnation"] as? NSNumber)?.stringValue
            query["serial"] = (token["serial"] as? NSNumber)?.stringValue
        }
        if !root.isEmpty { query["root"] = root }
        return query
    }
    func reshuffleRadio(root: String) async throws -> RadioBatch {
        let data = try await request("/api/radio/reshuffle", query: radioQuery(root), method: "POST")
        return try await radioBatch(data)
    }
    func renamePlaylist(_ id: Int64, name: String) async throws { _ = try await request("/api/playlists/\(id)/rename", method: "POST", body: ["name": name]) }
    func deletePlaylist(_ id: Int64) async throws { _ = try await request("/api/playlists/\(id)", method: "DELETE") }
    func appendPlaylist(_ id: Int64, tracks: [Track]) async throws {
        let entries = tracks.map(\.locator)
        if !entries.isEmpty { _ = try await request("/api/playlists/\(id)/entries", method: "POST", body: ["entries": entries]) }
    }
    func stars() async throws -> Set<String> {
        let data = try await request("/api/stars")
        let rows = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["entries"] ?? []
        return Set(decodeTracks(rows).map(\.id))
    }
    func star(_ track: Track, enabled: Bool) async throws {
        var body: [String: Any] = track.locator
        body["starred"] = enabled
        _ = try await request("/api/stars", method: "POST", body: body)
    }
    func radio(_ enabled: Bool, root: String) async throws -> RadioBatch {
        let data = try await request("/api/radio/enabled", query: radioQuery(root), method: "POST", body: ["enabled": enabled])
        return try await radioBatch(data)
    }
    func radioAdvance(root: String) async throws -> RadioBatch {
        let data = try await request("/api/radio/advance", query: radioQuery(root), method: "POST")
        return try await radioBatch(data)
    }
    private func radioBatch(_ data: Data) async throws -> RadioBatch {
        let reply = try JSONSerialization.jsonObject(with: data) as? [String: Any] ?? [:]
        let tracks = try await metadata(decodeTracks(reply["entries"] ?? []))
        return RadioBatch(tracks: tracks, exhausted: reply["exhausted"] as? Bool ?? tracks.isEmpty)
    }

    private func decodeTracks(_ object: Any) -> [Track] {
        guard let rows = object as? [[String: Any]],
              let data = try? JSONSerialization.data(withJSONObject: rows) else { return [] }
        let tracks = (try? JSONDecoder().decode([Track].self, from: data)) ?? []
        return deviceRoot == nil ? tracks : tracks.map { $0.onDevice() }
    }
}

private extension Data {
    func withDecoded<T: Decodable>(_ type: T.Type) throws -> T { try JSONDecoder().decode(type, from: self) }
}
