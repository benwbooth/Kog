#if KOG_NATIVE_AUDIO
import Foundation

/// URLSession honors the phone's proxy, VPN, and local-network routing. A bounded
/// producer/consumer buffer feeds the shared decoder without downloading the
/// whole track or doing network work on the audio render thread.
final class HTTPAudioStream: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    private let condition = NSCondition()
    private var chunks = [Data]()
    private var offset = 0
    private var buffered = 0
    private var complete = false
    private var cancelled = false
    private var failure: String?
    private var session: URLSession!
    private var task: URLSessionDataTask!

    init(url: URL, headers: String) {
        super.init()
        var request = URLRequest(url: url)
        request.timeoutInterval = 45
        for line in headers.components(separatedBy: "\r\n") {
            guard let colon = line.firstIndex(of: ":") else { continue }
            request.setValue(String(line[line.index(after: colon)...]).trimmingCharacters(in: .whitespaces),
                             forHTTPHeaderField: String(line[..<colon]))
        }
        let queue = OperationQueue()
        queue.name = "org.kog.player.network"; queue.maxConcurrentOperationCount = 1
        session = URLSession(configuration: .default, delegate: self, delegateQueue: queue)
        task = session.dataTask(with: request)
        task.resume()
    }

    var errorMessage: String? { condition.lock(); defer { condition.unlock() }; return failure }

    func cancel() {
        condition.lock(); cancelled = true; condition.broadcast(); condition.unlock()
        task.cancel(); session.invalidateAndCancel()
    }

    func read(_ output: UnsafeMutablePointer<UInt8>, capacity: Int) -> Int32 {
        condition.lock(); defer { condition.unlock() }
        let deadline = Date().addingTimeInterval(45)
        while chunks.isEmpty && !complete && !cancelled {
            if !condition.wait(until: deadline) {
                failure = "The audio stream stopped responding"; complete = true
            }
        }
        if cancelled { return -1 }
        guard let first = chunks.first else { return failure == nil ? 0 : -1 }
        let count = min(capacity, first.count - offset)
        first.withUnsafeBytes { bytes in
            output.update(from: bytes.baseAddress!.assumingMemoryBound(to: UInt8.self).advanced(by: offset), count: count)
        }
        offset += count; buffered -= count
        if offset == first.count { chunks.removeFirst(); offset = 0 }
        condition.broadcast()
        return Int32(count)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        guard let response = response as? HTTPURLResponse, (200..<300).contains(response.statusCode) else {
            condition.lock()
            failure = "Audio download failed (HTTP \((response as? HTTPURLResponse)?.statusCode ?? 0))"
            complete = true; condition.broadcast(); condition.unlock()
            completionHandler(.cancel); return
        }
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard !data.isEmpty else { return }
        condition.lock(); defer { condition.unlock() }
        // Only this dedicated delegate queue waits; the main thread and audio
        // thread never do. Reading or cancelling wakes it immediately.
        while buffered >= 1_048_576 && !cancelled { condition.wait() }
        guard !cancelled else { return }
        chunks.append(data); buffered += data.count; condition.broadcast()
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        condition.lock()
        if let error, failure == nil, !cancelled { failure = error.localizedDescription }
        complete = true; condition.broadcast(); condition.unlock()
        session.finishTasksAndInvalidate()
    }
}
#endif
