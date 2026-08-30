// The generated account password.
//
// It lives in a 0600 file beside the account store rather than in the login keychain.
// The keychain would be the better home for a secret that outlives the app, but this app
// is ad-hoc signed and so gets a new code identity on every rebuild, which makes macOS
// treat each build as a stranger and prompt before handing the item over. A prompt the
// user has to dismiss on every rebuild is worse than a file sitting next to `users.json`,
// which is already 0600 and already holds the hash of this very password.

import Foundation
import Security

enum Credentials {
    enum Failure: LocalizedError {
        case randomness(OSStatus)
        case notWritten(String)

        var errorDescription: String? {
            switch self {
            case .randomness(let status):
                return "Could not generate a password (OSStatus \(status))."
            case .notWritten(let path):
                return "Could not write the credential to \(path)."
            }
        }
    }

    /// Beside `users.json`, wherever the config puts it - not in a fixed directory of
    /// the app's own. A config that names a shared data directory then keeps the
    /// password and the account it opens together.
    private static var file: URL {
        VaultConfig.dataDirectory().appendingPathComponent("app-credentials.json")
    }

    /// 32 random bytes, base64-encoded to 44 characters. Comfortably over the server's
    /// 12-character minimum, and free of the newline that `--password-stdin` treats as
    /// the end of the password.
    static func generatePassword() throws -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard status == errSecSuccess else { throw Failure.randomness(status) }
        return Data(bytes).base64EncodedString()
    }

    static func save(password: String) throws {
        let payload = ["email": Paths.accountEmail, "password": password]
        let credentialJSON = try JSONSerialization.data(withJSONObject: payload, options: [.prettyPrinted])

        // Created with its mode rather than chmod-ed afterwards: writing first would
        // leave the secret readable by everyone for as long as that takes.
        try? FileManager.default.createDirectory(
            at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? FileManager.default.removeItem(at: file)
        guard
            FileManager.default.createFile(
                atPath: file.path, contents: credentialJSON, attributes: [.posixPermissions: 0o600])
        else { throw Failure.notWritten(file.path) }
    }

    static func load() -> String? {
        guard let storedJSON = try? Data(contentsOf: file),
            let payload = try? JSONSerialization.jsonObject(with: storedJSON) as? [String: Any],
            let password = payload["password"] as? String,
            payload["email"] as? String == Paths.accountEmail
        else { return nil }
        return password
    }

    static func delete() {
        try? FileManager.default.removeItem(at: file)
    }
}
