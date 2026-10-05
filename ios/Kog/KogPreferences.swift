import Foundation

#if KOG_NATIVE_AUDIO
@_silgen_name("kog_preferences_request")
private func preferencesRequest(_ input: UnsafePointer<CChar>, _ error: UnsafeMutablePointer<CChar>, _ capacity: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("kog_audio_string_free")
private func preferencesFree(_ string: UnsafeMutablePointer<CChar>)
#endif

/// Per-key preferences share the device library's SQLite database. UserDefaults
/// is read only for one-time migration; credentials remain in the Keychain.
final class KogPreferences {
    static let standard = KogPreferences()
    private let database: String
    private let legacy: UserDefaults
    private(set) var lastError: String?

    init(database: String? = nil, legacy: UserDefaults = .standard) {
        self.database = database ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Kog/library.sqlite").path
        self.legacy = legacy
    }

    private func request(_ input: [String: Any]) throws -> [String: Any] {
        #if KOG_NATIVE_AUDIO
        var input = input
        input["database"] = database
        let encoded = String(decoding: try JSONSerialization.data(withJSONObject: input), as: UTF8.self)
        var message = [CChar](repeating: 0, count: 2048)
        guard let pointer = encoded.withCString({ preferencesRequest($0, &message, message.count) }) else {
            throw NSError(domain: "KogPreferences", code: 1, userInfo: [NSLocalizedDescriptionKey: String(cString: message)])
        }
        defer { preferencesFree(pointer) }
        return try JSONSerialization.jsonObject(with: Data(String(cString: pointer).utf8)) as? [String: Any] ?? [:]
        #else
        throw NSError(domain: "KogPreferences", code: 1, userInfo: [NSLocalizedDescriptionKey: "Kog's native preferences backend is unavailable"])
        #endif
    }

    private func encoded(_ value: Any?) -> Any {
        if let data = value as? Data { return ["$data": data.base64EncodedString()] }
        return value ?? NSNull()
    }

    func object(forKey key: String) -> Any? {
        do {
            var reply = try request(["op": "get", "key": key])
            if reply["found"] as? Bool != true {
                reply = try request(["op": "import", "key": key, "value": encoded(legacy.object(forKey: key))])
            }
            let value = reply["value"]
            if let envelope = value as? [String: String], let text = envelope["$data"] { return Data(base64Encoded: text) }
            return value is NSNull ? nil : value
        } catch { record(error); return nil }
    }

    func string(forKey key: String) -> String? { object(forKey: key) as? String }
    func getOrCreateString(_ key: String, default value: String) -> String {
        if let saved = string(forKey: key) { return saved }
        do { return try request(["op": "default", "key": key, "value": value])["value"] as? String ?? value }
        catch { record(error); return value }
    }
    func treeRoot(for server: String) -> String {
        let key = "server_tree_root.\(server)"
        do {
            var reply = try request(["op": "get", "key": key])
            if reply["found"] as? Bool != true {
                let old = legacy.dictionary(forKey: "server_tree_roots")?[server] as? String ?? ""
                reply = try request(["op": "import", "key": key, "value": old])
            }
            return reply["value"] as? String ?? ""
        } catch { record(error); return "" }
    }
    func bool(forKey key: String) -> Bool { (object(forKey: key) as? NSNumber)?.boolValue ?? false }
    func integer(forKey key: String) -> Int { (object(forKey: key) as? NSNumber)?.intValue ?? 0 }
    func data(forKey key: String) -> Data? { object(forKey: key) as? Data }
    func dictionary(forKey key: String) -> [String: Any]? { object(forKey: key) as? [String: Any] }
    func set(_ value: Any?, forKey key: String) {
        do { _ = try request(["op": "write", "values": [key: encoded(value)]]) }
        catch { record(error) }
    }
    func removeObject(forKey key: String) { set(nil, forKey: key) }
    private func record(_ error: Error) {
        lastError = error.localizedDescription
        NSLog("Kog preferences: %@", error.localizedDescription)
    }
}
