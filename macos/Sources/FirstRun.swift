// First-launch setup, and repair of it afterwards.
//
// The server has no signup route by design, so the account is created the same way a
// person would create it: by running the CLI that ships in the bundle.

import AppKit
import Foundation

enum FirstRun {
    enum Failure: LocalizedError {
        case cancelled
        case command(String)

        var errorDescription: String? {
            switch self {
            case .cancelled: return "Setup was cancelled."
            case .command(let detail): return detail
            }
        }
    }

    /// Everything that must be true before the server is worth starting: a config, a
    /// data directory, and an account whose password we still hold.
    static func prepare() throws {
        try Paths.createDirectories()
        if !VaultConfig.exists {
            let folder = try chooseVaultFolder()
            try VaultConfig.create(vaultPath: folder, name: folder.lastPathComponent)
        }
        try ensureAccount()
    }

    static func chooseVaultFolder() throws -> URL {
        NSApp.activate(ignoringOtherApps: true)

        let panel = NSOpenPanel()
        panel.title = "Choose your knowledge base folder"
        panel.message = "Pick the folder KBView should serve. Your Obsidian vault is a good choice."
        panel.prompt = "Use This Folder"
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true

        guard panel.runModal() == .OK, let folder = panel.url else {
            throw Failure.cancelled
        }
        return folder
    }

    /// Reconciles the account store with the stored credential. Either can be thrown
    /// away independently, so the app repairs whichever is missing instead of landing on
    /// a login screen it has no password to fill in.
    private static func ensureAccount() throws {
        let accounts = try run(["user", "list"]).output
        let exists = accounts
            .split(separator: "\n")
            .contains { $0.trimmingCharacters(in: .whitespaces) == Paths.accountEmail }

        if exists && Credentials.load() != nil { return }

        let password = try Credentials.generatePassword()
        let action = exists ? "passwd" : "add"
        _ = try run(["user", action, Paths.accountEmail, "--password-stdin"], input: password)

        do {
            try Credentials.save(password: password)
        } catch {
            // An account whose password was never stored is worse than no account: it
            // cannot be used and it cannot be recovered. Undo it.
            if !exists { _ = try? run(["user", "rm", Paths.accountEmail]) }
            throw error
        }
    }

    struct CommandResult {
        let status: Int32
        let output: String
    }

    @discardableResult
    static func run(_ arguments: [String], input: String? = nil) throws -> CommandResult {
        let process = Process()
        process.executableURL = Paths.serverBinary
        process.arguments = ["--config", Paths.configFile.path] + arguments
        process.currentDirectoryURL = Paths.supportDirectory

        let outputPipe = Pipe()
        process.standardOutput = outputPipe
        process.standardError = outputPipe

        let inputPipe = Pipe()
        process.standardInput = inputPipe

        try process.run()

        if let input {
            // The server reads exactly one line and trims the newline.
            try? inputPipe.fileHandleForWriting.write(contentsOf: Data((input + "\n").utf8))
        }
        try? inputPipe.fileHandleForWriting.close()

        // Drained before waiting: a full pipe would block the child forever.
        let outputBytes = outputPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        let output = String(data: outputBytes, encoding: .utf8) ?? ""
        guard process.terminationStatus == 0 else {
            throw Failure.command(
                output.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "`kbview \(arguments.joined(separator: " "))` failed with exit code \(process.terminationStatus)."
                    : output.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        return CommandResult(status: process.terminationStatus, output: output)
    }
}
