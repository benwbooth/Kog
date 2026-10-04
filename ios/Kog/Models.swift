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
    var metadata: [String: String] = [:]
    var queueOrder: Int64?

    var id: String { "\(kind)|\(path)|\(entry)|\(fragment)" }
    var label: String { title.isEmpty ? (name.isEmpty ? URL(fileURLWithPath: path).lastPathComponent : name) : title }
    var detail: String { [artist, album].filter { !$0.isEmpty }.joined(separator: " · ") }
    var isDevice: Bool { kind == "device" }
    var locator: [String: String] {
        if isDevice {
            if let parts = URLComponents(string: path), parts.scheme == "kog-archive" {
                let items = parts.queryItems ?? []
                return ["kind": "archive", "path": items.first { $0.name == "archive" }?.value ?? "",
                        "entry": items.first { $0.name == "entry" }?.value ?? entry, "fragment": fragment]
            }
            return ["kind": "local", "path": path, "entry": entry, "fragment": fragment]
        }
        return ["kind": kind, "path": path, "entry": entry, "fragment": fragment]
    }
    var displayPath: String {
        let location = locator
        return (location["path"] ?? path) + ((location["entry"] ?? "").isEmpty ? "" : "/" + (location["entry"] ?? ""))
    }
    var filename: String { URL(fileURLWithPath: displayPath).lastPathComponent }
    func onDevice() -> Track {
        var copy = self
        if kind == "archive" {
            var url = URLComponents(); url.scheme = "kog-archive"
            url.queryItems = [URLQueryItem(name: "archive", value: path), URLQueryItem(name: "entry", value: entry), URLQueryItem(name: "directory", value: "0")]
            copy.path = url.string ?? path
        }
        copy.kind = "device"
        return copy
    }
    var detailRows: [(String, String)] {
        var rows = [("Title", label), ("Artist", artist), ("Album", album)]
        for field in TrackSort.metadataFields { rows.append((field.label, metadata[field.key] ?? "")) }
        if let size = Int64(metadata["fileSizeBytes"] ?? "") {
            rows.append(("File size", ByteCountFormatter.string(fromByteCount: size, countStyle: .file)))
        }
        rows.append(("Duration", String(format: "%.3f seconds", Double(duration) / 1000)))
        rows.append(("Path", displayPath))
        if !fragment.isEmpty { rows.append(("Subsong", fragment)) }
        return rows.filter { !$0.1.isEmpty }
    }

    enum CodingKeys: String, CodingKey { case kind, path, entry, fragment, name, title, artist, album, duration, metadata, queueOrder }
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
        metadata = try row.decodeIfPresent([String: String].self, forKey: .metadata) ?? [:]
        queueOrder = try row.decodeIfPresent(Int64.self, forKey: .queueOrder)
    }
}

struct Folder: Codable, Identifiable, Hashable {
    var name: String
    var path: String
    var isArchive: Bool = false
    var id: String { path }

    enum CodingKeys: String, CodingKey { case name, path, isArchive }
    init(name: String, path: String, isArchive: Bool = false) {
        self.name = name; self.path = path; self.isArchive = isArchive
    }
    init(from decoder: Decoder) throws {
        let row = try decoder.container(keyedBy: CodingKeys.self)
        name = try row.decode(String.self, forKey: .name)
        path = try row.decode(String.self, forKey: .path)
        isArchive = try row.decodeIfPresent(Bool.self, forKey: .isArchive) ?? path.hasPrefix("kog-archive:")
    }
}

struct Listing: Decodable {
    var path: String
    var parent: String
    var directories: [Folder]
    var files: [Track]
    var isArchive: Bool

    enum CodingKeys: String, CodingKey { case path, parent, directories, files, isArchive }
    init(from decoder: Decoder) throws {
        let row = try decoder.container(keyedBy: CodingKeys.self)
        path = try row.decode(String.self, forKey: .path)
        parent = try row.decodeIfPresent(String.self, forKey: .parent) ?? ""
        directories = try row.decodeIfPresent([Folder].self, forKey: .directories) ?? []
        files = try row.decodeIfPresent([Track].self, forKey: .files) ?? []
        isArchive = try row.decodeIfPresent(Bool.self, forKey: .isArchive) ?? path.hasPrefix("kog-archive:")
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


enum ShuffleMode: String, CaseIterable, Identifiable {
    case off, albums, all
    var id: String { rawValue }
    var label: String { switch self { case .off: "Off"; case .albums: "Albums"; case .all: "All tracks" } }
}

enum RepeatMode: String, CaseIterable, Identifiable {
    case off, one, album, all
    var id: String { rawValue }
    var label: String { switch self { case .off: "Off"; case .one: "One track"; case .album: "Album"; case .all: "All tracks" } }
}

struct TrackSort: Identifiable {
    var key: String
    var label: String
    var numeric = false
    var id: String { key }
    static let metadataFields: [TrackSort] = [
        .init(key: "albumArtist", label: "Album artist"), .init(key: "composer", label: "Composer"),
        .init(key: "year", label: "Year", numeric: true), .init(key: "genre", label: "Genre"),
        .init(key: "trackNumber", label: "Track number", numeric: true),
        .init(key: "discNumber", label: "Disc number", numeric: true),
        .init(key: "fileSizeBytes", label: "File size (bytes)", numeric: true),
        .init(key: "codec", label: "Codec"), .init(key: "sampleRate", label: "Sample rate", numeric: true),
        .init(key: "bitsPerSample", label: "Bits per sample", numeric: true),
        .init(key: "bitrate", label: "Bitrate", numeric: true), .init(key: "channels", label: "Channels", numeric: true)]
    static let all: [TrackSort] = [.init(key: "original", label: "Original order", numeric: true), .init(key: "title", label: "Title"), .init(key: "artist", label: "Artist"),
        .init(key: "album", label: "Album"), .init(key: "duration", label: "Length", numeric: true)] + metadataFields + [
        .init(key: "path", label: "Path"), .init(key: "filename", label: "Filename"), .init(key: "star", label: "Star", numeric: true)]
}

struct RadioBatch { var tracks: [Track]; var exhausted: Bool }
