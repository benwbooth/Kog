import Foundation

struct Track: Codable, Identifiable, Hashable {
    var kind: String
    var path: String
    var entry: String = ""
    var fragment: String = ""
    var name: String = ""
    var title: String = ""
    var artist: String = ""
    var album: String = ""
    var duration: Int64 = 0

    var id: String { "\(kind)|\(path)|\(entry)|\(fragment)" }
    var label: String { title.isEmpty ? (name.isEmpty ? URL(fileURLWithPath: path).lastPathComponent : name) : title }
    var detail: String { [artist, album].filter { !$0.isEmpty }.joined(separator: " · ") }
    var isDevice: Bool { kind == "device" }
    var locator: [String: String] { ["kind": kind, "path": path, "entry": entry, "fragment": fragment] }

    enum CodingKeys: String, CodingKey { case kind, path, entry, fragment, name, title, artist, album, duration }
    init(kind: String, path: String, entry: String = "", fragment: String = "", name: String = "", title: String = "", artist: String = "", album: String = "", duration: Int64 = 0) {
        self.kind = kind; self.path = path; self.entry = entry; self.fragment = fragment
        self.name = name; self.title = title; self.artist = artist; self.album = album; self.duration = duration
    }
    init(from decoder: Decoder) throws {
        let row = try decoder.container(keyedBy: CodingKeys.self)
        kind = try row.decodeIfPresent(String.self, forKey: .kind) ?? "local"
        path = try row.decode(String.self, forKey: .path)
        entry = try row.decodeIfPresent(String.self, forKey: .entry) ?? ""
        fragment = try row.decodeIfPresent(String.self, forKey: .fragment) ?? ""
        name = try row.decodeIfPresent(String.self, forKey: .name) ?? ""
        title = try row.decodeIfPresent(String.self, forKey: .title) ?? ""
        artist = try row.decodeIfPresent(String.self, forKey: .artist) ?? ""
        album = try row.decodeIfPresent(String.self, forKey: .album) ?? ""
        duration = try row.decodeIfPresent(Int64.self, forKey: .duration) ?? 0
    }
}

struct Folder: Codable, Identifiable, Hashable {
    var name: String
    var path: String
    var id: String { path }
}

struct Listing: Decodable {
    var path: String
    var parent: String
    var directories: [Folder]
    var files: [Track]

    enum CodingKeys: String, CodingKey { case path, parent, directories, files }
    init(from decoder: Decoder) throws {
        let row = try decoder.container(keyedBy: CodingKeys.self)
        path = try row.decode(String.self, forKey: .path)
        parent = try row.decodeIfPresent(String.self, forKey: .parent) ?? ""
        directories = try row.decodeIfPresent([Folder].self, forKey: .directories) ?? []
        files = try row.decodeIfPresent([Track].self, forKey: .files) ?? []
    }
}

struct SavedPlaylist: Decodable, Identifiable {
    var id: Int64
    var name: String
    var entryCount: Int
}

struct SearchPage: Decodable {
    var results: [SearchResult]
    var generation: Int64
    var total: Int
    var scanned: Int
    var done: Bool
}

struct SearchResult: Decodable {
    var is_dir: Bool?
    var kind: String?
    var path: String
    var entry: String?
    var fragment: String?
    var name: String?
    var title: String?
    var artist: String?
    var album: String?

    var folder: Folder { Folder(name: name ?? URL(fileURLWithPath: path).lastPathComponent, path: path) }
    var track: Track { Track(kind: kind ?? "local", path: path, entry: entry ?? "", fragment: fragment ?? "", name: name ?? "", title: title ?? "", artist: artist ?? "", album: album ?? "") }
}

enum KogError: LocalizedError {
    case invalidServer
    case response(String)
    var errorDescription: String? {
        switch self {
        case .invalidServer: return "Enter the address of your Kog server."
        case .response(let message): return message
        }
    }
}
