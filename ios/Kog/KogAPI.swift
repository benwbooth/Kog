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

    func stream(_ track: Track) throws -> URL {
        if track.isDevice { return URL(fileURLWithPath: track.path) }
        return try url("/api/stream", track.locator.merging(["codec": codec, "token": token]) { _, new in new })
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
                         method: String = "GET", body: Any? = nil) async throws -> Data {
        var request = try authenticatedRequest(endpoint, query: query)
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

    func health() async throws { _ = try await request("/api/health") }
    func browse(_ path: String = "") async throws -> Listing {
        try JSONDecoder().decode(Listing.self, from: await request("/api/library", query: ["path": path]))
    }
    func collect(_ path: String) async throws -> [Track] {
        let data = try await request("/api/library/collect", query: ["path": path])
        let rows = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["tracks"] ?? []
        return try await metadata(decodeTracks(rows))
    }
    func expand(_ track: Track) async throws -> [Track] {
        let data = try await request("/api/expand", method: "POST", body: [track.locator.merging(["name": track.name]) { _, new in new }])
        let tracks = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["tracks"] as? [Any]
        return try await metadata(decodeTracks(tracks?.first ?? []))
    }
    func metadata(_ tracks: [Track]) async throws -> [Track] {
        guard !tracks.isEmpty else { return [] }
        var result = [Track]()
        for start in stride(from: 0, to: tracks.count, by: 100) {
            let chunk = Array(tracks[start..<min(start + 100, tracks.count)])
            let data = try await request("/api/metadata", method: "POST", body: chunk.map(\.locator))
            let rows = (try JSONSerialization.jsonObject(with: data) as? [[String: Any]]) ?? []
            for (index, var track) in chunk.enumerated() {
                if index < rows.count {
                    let row = rows[index]
                    track.title = row["title"] as? String ?? ""
                    track.artist = row["artist"] as? String ?? ""
                    track.album = row["album"] as? String ?? ""
                    track.duration = Int64(((row["duration"] as? NSNumber)?.doubleValue ?? 0) * 1000)
                }
                result.append(track)
            }
        }
        return result
    }

    func search(_ term: String) async throws -> SearchPage {
        try JSONDecoder().decode(SearchPage.self, from: await request("/api/library/search", query: ["q": term]))
    }
    func more(_ generation: Int64, offset: Int) async throws -> SearchPage {
        try JSONDecoder().decode(SearchPage.self, from: await request("/api/library/search/more", query: ["g": "\(generation)", "offset": "\(offset)"]))
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
    func createPlaylist(_ name: String) async throws { _ = try await request("/api/playlists", method: "POST", body: ["name": name]) }
    func renamePlaylist(_ id: Int64, name: String) async throws { _ = try await request("/api/playlists/\(id)/rename", method: "POST", body: ["name": name]) }
    func deletePlaylist(_ id: Int64) async throws { _ = try await request("/api/playlists/\(id)", method: "DELETE") }
    func appendPlaylist(_ id: Int64, tracks: [Track]) async throws {
        let entries = tracks.filter { !$0.isDevice }.map(\.locator)
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
    func radio(_ enabled: Bool, root: String) async throws -> [Track] {
        let data = try await request("/api/radio/enabled", query: ["root": root], method: "POST", body: ["enabled": enabled])
        let rows = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["entries"] ?? []
        return enabled ? try await metadata(decodeTracks(rows)) : []
    }
    func radioAdvance(root: String) async throws -> [Track] {
        let data = try await request("/api/radio/advance", query: ["root": root], method: "POST")
        let rows = (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["entries"] ?? []
        return try await metadata(decodeTracks(rows))
    }

    private func decodeTracks(_ object: Any) -> [Track] {
        guard let rows = object as? [[String: Any]],
              let data = try? JSONSerialization.data(withJSONObject: rows) else { return [] }
        return (try? JSONDecoder().decode([Track].self, from: data)) ?? []
    }
}

private extension Data {
    func withDecoded<T: Decodable>(_ type: T.Type) throws -> T { try JSONDecoder().decode(type, from: self) }
}
