import Foundation

@main struct UiContract {
    static func lookup(_ value: Any, _ path: String) -> Any? {
        path.split(separator: ".").reduce(value as Any?) { value, part in
            if let dictionary = value as? [String: Any] { return dictionary[String(part)] }
            if let array = value as? [Any], let index = Int(part), array.indices.contains(index) { return array[index] }
            return nil
        }
    }
    static func main() throws {
        let input = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
        let fixture = try JSONSerialization.jsonObject(with: input) as! [String: Any]
        let policy = SharedPlaybackPolicy()
        var load = [String: Any](), save = [String: Any]()
        for (index, step) in (fixture["steps"] as! [[String: Any]]).enumerated() {
            var command = step["command"] as? [String: Any] ?? [:]
            if let entries = step["load"] {
                command = ["op": "workspace", "command": ["op": "loaded", "key": load["key"]!, "generation": load["generation"]!, "entries": entries]]
            } else if step["save_ok"] != nil {
                command = ["op": "workspace", "command": ["op": "saved", "key": save["key"]!, "revision": save["revision"]!]]
            }
            let reply = try policy.send(command)
            _ = try JSONDecoder().decode(PlaylistWorkspaceSnapshot.self, from: JSONSerialization.data(withJSONObject: reply["workspace"]!))
            if let effect = reply["workspace_effect"] as? [String: Any] {
                if effect["action"] as? String == "load" { load = effect }
                if effect["action"] as? String == "save" { save = effect }
            }
            for (path, expected) in step["expect"] as! [String: Any] {
                guard let actual = lookup(reply, path), NSDictionary(dictionary: ["value": actual]).isEqual(to: ["value": expected]) else {
                    fatalError("Swift UI contract step \(index) \(path): expected \(expected), got \(String(describing: lookup(reply,path)))")
                }
            }
        }
        print("Swift UI contract: \((fixture["steps"] as! [Any]).count) steps passed through the production Swift/C bridge")
    }
}
