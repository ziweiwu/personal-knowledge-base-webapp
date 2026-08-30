// Where the app keeps things, and where it finds the server binary.

import Foundation

enum Paths {
    static let bundleIdentifier = "com.ziweiwu.kbviewer"

    /// The account the app signs in as. It is never emailed; it exists because the
    /// server identifies accounts by address and refuses to start without one.
    static let accountEmail = "kbviewer-app@localhost"

    /// Set only by smoke-test.sh. Everything the app stores hangs off this directory -
    /// config, account store, the generated credential - so redirecting it is enough to
    /// keep a test run away from the real setup. Absent in normal use.
    private static var supportOverride: String? {
        ProcessInfo.processInfo.environment["KBVIEWER_APP_SUPPORT"]
    }

    /// State lives outside the bundle so that rebuilding the app never destroys the
    /// vault config or the account store.
    static var supportDirectory: URL {
        if let supportOverride { return URL(fileURLWithPath: supportOverride, isDirectory: true) }
        return FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("KBViewer", isDirectory: true)
    }

    static var configFile: URL {
        supportDirectory.appendingPathComponent("kbviewer.config.json")
    }

    static var dataDirectory: URL {
        supportDirectory.appendingPathComponent("data", isDirectory: true)
    }

    static var logDirectory: URL {
        if supportOverride != nil {
            return supportDirectory.appendingPathComponent("logs", isDirectory: true)
        }
        return FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Logs", isDirectory: true)
            .appendingPathComponent("KBViewer", isDirectory: true)
    }

    static var logFile: URL {
        logDirectory.appendingPathComponent("server.log")
    }

    /// The bundled server. Named `kbviewer-server` rather than `kbviewer` because the
    /// wrapper's own executable is `KBViewer`, and the Mac's filesystem is case
    /// insensitive: `kbviewer` and `KBViewer` are one file, so copying the server in beside
    /// the wrapper under that name overwrites one with the other. When the server won it
    /// ran with no arguments against whatever config the working directory held; when
    /// the wrapper won it spawned itself, once per generation, without limit.
    static let serverBinaryName = "kbviewer-server"

    /// Resolved from the running executable rather than from `Bundle.main.bundleURL`,
    /// so the binary is still found when the wrapper is run straight out of the build
    /// directory instead of from an assembled bundle.
    static var serverBinary: URL {
        let executableDirectory =
            Bundle.main.executableURL?.resolvingSymlinksInPath().deletingLastPathComponent()
            ?? URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()
        return executableDirectory.appendingPathComponent(serverBinaryName)
    }

    static func createDirectories() throws {
        try FileManager.default.createDirectory(
            at: supportDirectory, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(
            at: dataDirectory, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(
            at: logDirectory, withIntermediateDirectories: true)
    }
}
