import Foundation

@main
struct APIContract {
    static func main() throws {
        let client = KogAPI(server: "http://music.local:8420", token: "a b/c+",
                            username: "", password: "", codec: "aac")
        let health = try client.url("/api/health")
        precondition(health.absoluteString == "http://music.local:8420/api/health")
        let file = Track(kind: "archive", path: "/Music/a.zip", entry: "sub/song.nsf", fragment: "2")
        let stream = try client.stream(file)
        precondition(stream.path == "/api/stream")
        let query = URLComponents(url: stream, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let values = Dictionary(uniqueKeysWithValues: query.map { ($0.name, $0.value ?? "") })
        precondition(values["token"] == "a b/c+" && values["entry"] == "sub/song.nsf")
        precondition(values["fragment"] == "2" && values["codec"] == "aac")

        let listing = try JSONDecoder().decode(Listing.self, from: Data(#"{"path":"/","parent":null,"directories":[],"files":[{"kind":"local","path":"/a.mp3","name":"a.mp3","fragment":null}]}"#.utf8))
        precondition(listing.parent.isEmpty && listing.files.count == 1)
        precondition(listing.files[0].fragment.isEmpty && listing.files[0].label == "a.mp3")
        let roundTrip = try JSONDecoder().decode([Track].self, from: JSONEncoder().encode([file]))
        precondition(roundTrip == [file])
        print("iOS API locator and decoding contract passed")
    }
}
