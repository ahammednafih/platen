import Foundation
import Network

struct Found: Hashable {
    let name: String
    let base: String
}

final class Discovery: @unchecked Sendable {
    private let queue = DispatchQueue(label: "platen.discovery")
    private var browser: NWBrowser?
    private var connections: [NWConnection] = []
    private var found: [String: Found] = [:]
    private var stopAtFirst = false
    private var continuation: CheckedContinuation<[Found], Never>?

    static func first(within seconds: Double) async -> Found? {
        await Discovery().run(seconds: seconds, stopAtFirst: true).first
    }

    static func all(within seconds: Double) async -> [Found] {
        await Discovery().run(seconds: seconds, stopAtFirst: false)
    }

    private func run(seconds: Double, stopAtFirst: Bool) async -> [Found] {
        await withCheckedContinuation { continuation in
            queue.async {
                self.continuation = continuation
                self.stopAtFirst = stopAtFirst
                self.start()
                self.queue.asyncAfter(deadline: .now() + seconds) { self.finish() }
            }
        }
    }

    private func start() {
        let browser = NWBrowser(for: .bonjourWithTXTRecord(type: "_uscan._tcp", domain: nil), using: .tcp)
        browser.browseResultsChangedHandler = { [weak self] results, _ in
            results.forEach { self?.resolve($0) }
        }
        browser.stateUpdateHandler = { [weak self] state in
            if case .failed = state { self?.finish() }
        }
        self.browser = browser
        browser.start(queue: queue)
    }

    private func resolve(_ result: NWBrowser.Result) {
        guard continuation != nil, case let .service(name, _, _, _) = result.endpoint else { return }
        var txt: [String: String] = [:]
        if case let .bonjour(record) = result.metadata { txt = record.dictionary }
        let params = NWParameters.tcp
        if let ip = params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options { ip.version = .v4 }
        let connection = NWConnection(to: result.endpoint, using: params)
        connections.append(connection)
        connection.stateUpdateHandler = { [weak self, weak connection] state in
            guard let self, let connection else { return }
            switch state {
            case .ready:
                if case let .hostPort(host, port)? = connection.currentPath?.remoteEndpoint, case let .ipv4(address) = host {
                    let ip = "\(address)".split(separator: "%").first.map(String.init) ?? "\(address)"
                    let rs = (txt["rs"] ?? "eSCL").trimmingCharacters(in: CharacterSet(charactersIn: "/"))
                    let f = Found(name: txt["ty"] ?? name, base: "http://\(ip):\(port.rawValue)/\(rs)")
                    self.found[f.base] = f
                    if self.stopAtFirst { self.finish() }
                }
                connection.cancel()
            case .failed, .waiting:
                connection.cancel()
            default:
                break
            }
        }
        connection.start(queue: queue)
    }

    private func finish() {
        guard let continuation else { return }
        self.continuation = nil
        browser?.cancel()
        connections.forEach { $0.cancel() }
        connections = []
        continuation.resume(returning: found.values.sorted { $0.name < $1.name })
    }
}
