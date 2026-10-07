// Compile together with ios/Kog/MmlDocument.swift. Pass the JSON written by
// `KOG_MML_FIXTURE=… cargo test -p kog-inspection export_active`.
import Foundation

@main
struct MmlDocumentContract {
    struct Expected: Decodable { let seconds: Double; let spans: [Int]; let bar: Int }
    struct Fixture: Decodable { let expected: [Expected] }

    static func main() throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
        let reply = try JSONDecoder().decode(MmlScoreReply.self, from: data)
        let expected = try JSONDecoder().decode(Fixture.self, from: data).expected
        guard let document = reply.document else { fatalError("the fixture has no document") }
        precondition(reply.status == "ready" && reply.revision == 3)
        precondition(document.slice(0, 11) == "#KOG-MML 1\n")
        for check in expected {
            let active = document.active(at: check.seconds)
            precondition(active.spans == Set(check.spans), "spans at \(check.seconds): \(active.spans) != \(check.spans)")
            precondition(active.bar == check.bar, "bar at \(check.seconds)")
        }
        print("Swift MML document decoding and highlighting match Rust at \(expected.count) positions: PASS")
    }
}
