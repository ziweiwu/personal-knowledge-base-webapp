// Starting, adopting and stopping the kbview server.
//
// The app cannot assume the port is free: the LaunchAgent in deploy/ and a manual
// `./target/release/kbview` both take 4321. So the port is probed first and the
// server is adopted rather than duplicated when one is already there.

import Foundation

final class ServerProcess {
    enum Ownership {
        /// Spawned by this app, and stopped when it quits.
        case owned
        /// Already running when the app launched; left alone on quit.
        case external
    }

    enum Failure: LocalizedError {
        case noFreePort
        case didNotStart(String)

        var errorDescription: String? {
            switch self {
            case .noFreePort:
                return "No free port available near \(VaultConfig.port())."
            case .didNotStart(let detail):
                return detail.isEmpty ? "The server did not start." : detail
            }
        }
    }

    /// Printed unconditionally by the server once the listener is bound. Waiting for
    /// this is more reliable than polling the socket, which can accept a connection
    /// before the index has finished building.
    private static let readyMarker = "kbview listening on http://"

    private static let startTimeout: TimeInterval = 30
    private static let recentLineLimit = 40
    private static let portScanRange = 20
    private static let maximumLogBytes: UInt64 = 1_048_576

    /// How long a terminate is given before the signal stops being polite.
    private static let terminationGracePeriod: TimeInterval = 3
    private static let terminationPollInterval: UInt32 = 50_000

    /// A probe has to outlast a server busy building its index, but not hold up launch.
    private static let probeRequestTimeout: TimeInterval = 3
    private static let probeWaitTimeout: TimeInterval = 5
    private static let unauthorizedStatusCode = 401

    private(set) var port = VaultConfig.port()
    private(set) var ownership = Ownership.external

    /// Set when an already-running server was adopted, so the window can use the session
    /// that adoption already established instead of spending a second password hash.
    private(set) var adoptedSession: HTTPCookie?

    private var process: Process?
    private var stopping = false
    private var recentLines: [String] = []
    private var pendingOutput = ""
    private let lock = NSLock()

    /// Called on the main queue when a server we own exits without being asked to.
    var onUnexpectedExit: ((String) -> Void)?

    var isRunning: Bool {
        process?.isRunning ?? false
    }

    // MARK: - Starting

    func start(completion: @escaping (Result<Ownership, Error>) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async {
            let result = self.startSynchronously()
            DispatchQueue.main.async { completion(result) }
        }
    }

    private func startSynchronously() -> Result<Ownership, Error> {
        let configured = VaultConfig.port()

        var mustRelocate = false

        switch Self.probe(port: configured) {
        case .kbview:
            // A kbview is there, but it need not be serving this app's configuration.
            // The launch agent in deploy/ runs the repository's config against its own
            // account store; adopting that would quietly ignore the folder this app was
            // told to serve and land on a login screen the app cannot fill in. Sharing
            // an account is the usable definition of "the same server", so that is the
            // test - and the session it returns is kept, rather than signing in twice.
            if let session = SessionLogin.attemptSynchronously(port: configured) {
                adoptedSession = session
                port = configured
                ownership = .external
                return .success(.external)
            }
            mustRelocate = true
        case .absent:
            port = configured
        case .foreign:
            mustRelocate = true
        }

        if mustRelocate {
            guard let free = Self.firstFreePort(from: configured + 1) else {
                return .failure(Failure.noFreePort)
            }
            do {
                try VaultConfig.setPort(free)
            } catch {
                return .failure(error)
            }
            port = free
        }

        return spawn().map { .owned }
    }

    private func spawn() -> Result<Void, Error> {
        lock.lock()
        stopping = false
        lock.unlock()

        let process = makeServerProcess()

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe

        // Extra signals are harmless: the one waiter takes the first and the semaphore is
        // then discarded, so readiness and exit can both signal without coordinating.
        let ready = DispatchSemaphore(value: 0)
        attachHandlers(process: process, pipe: pipe, ready: ready)

        do {
            try process.run()
        } catch {
            return .failure(error)
        }
        self.process = process

        if ready.wait(timeout: .now() + Self.startTimeout) == .timedOut {
            stop()
            return .failure(Failure.didNotStart("The server did not start within 30 seconds."))
        }

        guard process.isRunning else {
            // It exited instead of listening. Its own stderr says why - a missing
            // config, a corrupt account store, a folder that is not there.
            self.process = nil
            return .failure(Failure.didNotStart(recentOutput()))
        }

        ownership = .owned
        return .success(())
    }

    private func makeServerProcess() -> Process {
        let process = Process()
        process.executableURL = Paths.serverBinary
        process.arguments = ["--config", Paths.configFile.path]
        process.currentDirectoryURL = Paths.supportDirectory

        var environment = ProcessInfo.processInfo.environment
        environment["RUST_LOG"] = environment["RUST_LOG"] ?? "kbview=info"
        process.environment = environment
        return process
    }

    /// Streams the server's output to the log and the ring buffer, signalling `ready`
    /// when it announces its listener or exits, whichever happens first.
    private func attachHandlers(process: Process, pipe: Pipe, ready: DispatchSemaphore) {
        let log = Self.openLog()

        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let chunk = handle.availableData
            guard !chunk.isEmpty else { return }
            try? log?.write(contentsOf: chunk)
            guard let self, let text = String(data: chunk, encoding: .utf8) else { return }
            if self.absorb(text) { ready.signal() }
        }

        process.terminationHandler = { [weak self] finished in
            pipe.fileHandleForReading.readabilityHandler = nil
            // The reason a server is exiting is the last thing it writes, and it can
            // still be sitting in the pipe when this fires. Draining it here is what
            // makes the alert say why instead of saying nothing.
            let remaining = pipe.fileHandleForReading.readDataToEndOfFile()
            if !remaining.isEmpty {
                try? log?.write(contentsOf: remaining)
                if let text = String(data: remaining, encoding: .utf8) { _ = self?.absorb(text) }
            }
            try? log?.close()
            self?.reportExit(process: finished, ready: ready)
        }
    }

    /// Unblocks a start that is still waiting, then reports the exit unless it was a
    /// stop this app asked for.
    private func reportExit(process finished: Process, ready: DispatchSemaphore) {
        lock.lock()
        let wasStopping = stopping
        lock.unlock()

        // The exit is the answer to whatever that start was waiting for.
        ready.signal()
        guard !wasStopping else { return }

        let detail = recentOutput()
        DispatchQueue.main.async {
            self.process = nil
            self.onUnexpectedExit?(
                detail.isEmpty
                    ? "The server stopped unexpectedly (exit code \(finished.terminationStatus))."
                    : detail)
        }
    }

    // MARK: - Stopping

    func stop() {
        guard let process, process.isRunning else {
            self.process = nil
            return
        }
        lock.lock()
        stopping = true
        lock.unlock()

        process.terminate()
        let deadline = Date().addingTimeInterval(Self.terminationGracePeriod)
        while process.isRunning && Date() < deadline {
            usleep(Self.terminationPollInterval)
        }
        if process.isRunning {
            kill(process.processIdentifier, SIGKILL)
        }
        self.process = nil
        // `stopping` is deliberately left set. The termination handler runs on its own
        // queue and can arrive after the wait above has already seen the process go, so
        // clearing it here would let a deliberate stop be reported as a crash. The next
        // spawn clears it instead.
    }

    func restart(completion: @escaping (Result<Ownership, Error>) -> Void) {
        stop()
        adoptedSession = nil
        lock.lock()
        recentLines.removeAll()
        pendingOutput = ""
        lock.unlock()
        start(completion: completion)
    }

    /// Adds a chunk of output to the ring buffer, reporting whether it announced the
    /// listener. Split on newlines so a line arriving in two reads is not counted twice.
    private func absorb(_ text: String) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        pendingOutput += text
        var lines = pendingOutput.components(separatedBy: "\n")
        pendingOutput = lines.removeLast()
        recentLines.append(contentsOf: lines)
        if recentLines.count > Self.recentLineLimit {
            recentLines.removeFirst(recentLines.count - Self.recentLineLimit)
        }
        return text.contains(Self.readyMarker)
    }

    func recentOutput() -> String {
        lock.lock()
        defer { lock.unlock() }
        return (recentLines + [pendingOutput])
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
            .suffix(Self.recentLineLimit)
            .joined(separator: "\n")
    }

    // MARK: - Probing

    enum Probe {
        case kbview
        case foreign
        case absent
    }

    /// A kbview answers this route with a JSON 401: it sits behind the session gate
    /// that covers the whole /api group. Any other server on the port answers
    /// differently, which is what distinguishes "ours" from "someone else's".
    static func probe(port: Int) -> Probe {
        guard let url = URL(string: "http://127.0.0.1:\(port)/api/auth/session") else {
            return .absent
        }
        var request = URLRequest(url: url)
        request.timeoutInterval = probeRequestTimeout
        request.cachePolicy = .reloadIgnoringLocalCacheData

        var outcome = Probe.absent
        let done = DispatchSemaphore(value: 0)
        URLSession.shared.dataTask(with: request) { data, response, _ in
            defer { done.signal() }
            guard let http = response as? HTTPURLResponse else { return }
            let body = data.flatMap { String(data: $0, encoding: .utf8) } ?? ""
            outcome =
                (http.statusCode == unauthorizedStatusCode && body.contains("\"error\":\"unauthorized\""))
                ? .kbview : .foreign
        }.resume()
        _ = done.wait(timeout: .now() + probeWaitTimeout)
        return outcome
    }

    private static func firstFreePort(from start: Int) -> Int? {
        (start..<(start + portScanRange)).first { isPortFree($0) }
    }

    /// Asks the kernel rather than the network: a port held by a process that never
    /// answers HTTP would look free to a probe but still refuse the bind.
    private static func isPortFree(_ port: Int) -> Bool {
        let descriptor = socket(AF_INET, SOCK_STREAM, 0)
        guard descriptor >= 0 else { return false }
        defer { close(descriptor) }

        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = UInt16(port).bigEndian
        address.sin_addr.s_addr = inet_addr("127.0.0.1")

        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        return bound == 0
    }

    // MARK: - Logging

    private static func openLog() -> FileHandle? {
        let path = Paths.logFile.path
        let manager = FileManager.default
        if let size = try? manager.attributesOfItem(atPath: path)[.size] as? UInt64,
            size > maximumLogBytes
        {
            try? manager.removeItem(atPath: path)
        }
        if !manager.fileExists(atPath: path) {
            manager.createFile(atPath: path, contents: nil)
        }
        let handle = try? FileHandle(forWritingTo: Paths.logFile)
        _ = try? handle?.seekToEnd()
        return handle
    }
}
