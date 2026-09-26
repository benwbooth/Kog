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

/// File browsing and track expansion use the same Rust rules as the server.
/// These synchronous calls belong on a background task, outside SwiftUI.
enum NativeAudioCatalog {
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
}
#endif
