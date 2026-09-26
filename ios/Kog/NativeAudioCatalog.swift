import Foundation

#if KOG_NATIVE_AUDIO
@_silgen_name("kog_audio_browse")
private func catalogBrowse(_ root: UnsafePointer<CChar>, _ path: UnsafePointer<CChar>,
                           _ error: UnsafeMutablePointer<CChar>, _ capacity: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("kog_audio_expand")
private func catalogExpand(_ path: UnsafePointer<CChar>, _ error: UnsafeMutablePointer<CChar>,
                           _ capacity: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("kog_audio_string_free")
private func catalogFree(_ string: UnsafeMutablePointer<CChar>)
@_silgen_name("kog_audio_artwork")
private func catalogArtwork(_ path: UnsafePointer<CChar>, _ length: UnsafeMutablePointer<Int>) -> UnsafeMutablePointer<UInt8>?
@_silgen_name("kog_audio_bytes_free")
private func catalogBytesFree(_ bytes: UnsafeMutablePointer<UInt8>, _ length: Int)

/// File browsing and track expansion use the same Rust rules as the server.
/// These synchronous calls belong on a background task, outside SwiftUI.
enum NativeAudioCatalog {
    private static let artworkCache: NSCache<NSString, NSData> = {
        let cache = NSCache<NSString, NSData>()
        cache.totalCostLimit = 32 * 1024 * 1024
        return cache
    }()

    private static func decode<T: Decodable>(_ pointer: UnsafeMutablePointer<CChar>?,
                                             error: [CChar], as type: T.Type) throws -> T {
        guard let pointer else { throw KogError.response(String(cString: error)) }
        defer { catalogFree(pointer) }
        return try JSONDecoder().decode(T.self, from: Data(String(cString: pointer).utf8))
    }

    static func browse(root: String, path: String) throws -> Listing {
        var message = [CChar](repeating: 0, count: 1024)
        let result = root.withCString { root in
            path.withCString { path in catalogBrowse(root, path, &message, message.count) }
        }
        return try decode(result, error: message, as: Listing.self)
    }

    static func expand(path: String) throws -> [Track] {
        var message = [CChar](repeating: 0, count: 1024)
        let result = path.withCString { catalogExpand($0, &message, message.count) }
        return try decode(result, error: message, as: [Track].self)
    }

    static func artwork(path: String) -> Data? {
        let key = path as NSString
        if let cached = artworkCache.object(forKey: key) { return cached as Data }
        var length = 0
        let pointer = path.withCString { catalogArtwork($0, &length) }
        guard let pointer, length > 0 else { return nil }
        defer { catalogBytesFree(pointer, length) }
        let data = Data(bytes: pointer, count: length)
        artworkCache.setObject(data as NSData, forKey: key, cost: data.count)
        return data
    }
}
#endif
