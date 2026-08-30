// Reading and writing kbviewer.config.json.
//
// Rewrites go through JSONSerialization on the parsed object rather than through a
// typed struct, so a field the app does not model - readOnly, folderNotes, a second
// root added by hand - survives the app changing the port.

import Foundation

enum VaultConfig {
    static let defaultPort = 4321

    enum Failure: LocalizedError {
        case unreadable(String)

        var errorDescription: String? {
            switch self {
            case .unreadable(let detail): return detail
            }
        }
    }

    static var exists: Bool {
        FileManager.default.fileExists(atPath: Paths.configFile.path)
    }

    /// Where the server keeps `users.json`. The credential the app holds is stored
    /// beside it, so the password and the account it opens can never end up in different
    /// directories - which is what happens when the config is re-pointed at another data
    /// directory, and leaves the app holding a password for a store it no longer reads.
    static func dataDirectory() -> URL {
        guard let path = load()?["dataDir"] as? String, !path.isEmpty else {
            return Paths.dataDirectory
        }
        let url = URL(fileURLWithPath: path, isDirectory: true)
        return url.path.hasPrefix("/") ? url : Paths.dataDirectory
    }

    static func port() -> Int {
        guard let object = load(), let port = object["port"] as? Int else { return defaultPort }
        return port
    }

    /// The configured vault folders, for the "does this still exist?" check at launch.
    static func rootPaths() -> [String] {
        guard let roots = load()?["roots"] as? [[String: Any]] else { return [] }
        return roots.compactMap { $0["path"] as? String }
    }

    static func setPort(_ port: Int) throws {
        guard var object = load() else {
            throw Failure.unreadable("Could not read \(Paths.configFile.path)")
        }
        object["port"] = port
        try write(object)
    }

    /// The first-run config. `dataDir` is absolute on purpose: the server resolves a
    /// relative one against the working directory, which for a launched app is `/`.
    static func create(vaultPath: URL, name: String) throws {
        let object: [String: Any] = [
            "host": "127.0.0.1",
            "port": defaultPort,
            "dataDir": Paths.dataDirectory.path,
            "roots": [
                [
                    "id": "kb",
                    "name": name,
                    "path": vaultPath.path,
                ]
            ],
        ]
        try write(object)
    }

    private static func load() -> [String: Any]? {
        guard let configJSON = try? Data(contentsOf: Paths.configFile),
            let object = try? JSONSerialization.jsonObject(with: configJSON) as? [String: Any]
        else { return nil }
        return object
    }

    private static func write(_ object: [String: Any]) throws {
        let configJSON = try JSONSerialization.data(
            withJSONObject: object, options: [.prettyPrinted, .sortedKeys])
        try configJSON.write(to: Paths.configFile, options: .atomic)
    }
}
