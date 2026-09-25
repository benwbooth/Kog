import AVFoundation
import Foundation

/// Answer AVFoundation's HTTP Basic challenge for a server stream. Credentials
/// are only handed to challenges from the configured Kog origin.
final class AuthenticatedAssetLoader: NSObject, AVAssetResourceLoaderDelegate {
    private let host: String
    private let port: Int?
    private let credential: URLCredential
    let queue = DispatchQueue(label: "org.kog.player.stream-auth")

    init(url: URL, username: String, password: String) {
        host = url.host ?? ""
        port = url.port
        credential = URLCredential(user: username, password: password, persistence: .forSession)
    }

    func resourceLoader(_ resourceLoader: AVAssetResourceLoader,
                        shouldWaitForResponseTo challenge: URLAuthenticationChallenge) -> Bool {
        let space = challenge.protectionSpace
        guard space.host == host, space.port == (port ?? (space.protocol == "https" ? 443 : 80)),
              space.authenticationMethod == NSURLAuthenticationMethodHTTPBasic,
              challenge.previousFailureCount == 0 else { return false }
        challenge.sender?.use(credential, for: challenge)
        return true
    }
}
