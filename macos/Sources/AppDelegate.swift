// App lifecycle: the menu bar item, the menus, and the order things start in.

import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let server = ServerProcess()
    private var vaultWindow: VaultWindow?
    private var statusItem: NSStatusItem?
    private var statusLabel: NSMenuItem?
    private var restartItem: NSMenuItem?
    private var terminationSignals: [DispatchSourceSignal] = []

    func applicationDidFinishLaunching(_ notification: Notification) {
        buildMainMenu()
        buildStatusItem()
        installTerminationSignalHandlers()

        server.onUnexpectedExit = { [weak self] detail in
            self?.reportServerStopped(detail)
        }

        do {
            try FirstRun.prepare()
        } catch FirstRun.Failure.cancelled {
            NSApp.terminate(nil)
            return
        } catch {
            presentFatal("KBView could not set itself up", detail: error.localizedDescription)
            return
        }

        warnAboutMissingRoots()
        startServer()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        // Closing the window leaves the server up; the menu bar item brings it back.
        false
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        openVault()
        return true
    }

    func applicationWillTerminate(_ notification: Notification) {
        // Only ever stops a server this app started. One that was already running when
        // we launched is somebody else's to stop.
        if server.ownership == .owned {
            server.stop()
        }
    }

    /// AppKit does not turn a signal into applicationWillTerminate, so a plain `kill`
    /// or a Ctrl-C would take the app down and leave the server child running.
    private func installTerminationSignalHandlers() {
        for number in [SIGTERM, SIGINT] {
            signal(number, SIG_IGN)
            let source = DispatchSource.makeSignalSource(signal: number, queue: .main)
            source.setEventHandler { NSApp.terminate(nil) }
            source.resume()
            terminationSignals.append(source)
        }
    }

    // MARK: - Startup

    private func startServer() {
        server.start { [weak self] result in
            guard let self else { return }
            switch result {
            case .success:
                self.refreshStatusMenu()
                self.openVault()
            case .failure(let error):
                self.presentFatal(
                    "KBView could not start the server", detail: error.localizedDescription)
            }
        }
    }

    /// The server only warns about a folder that is not there and then serves nothing,
    /// which looks exactly like an empty vault. Say so before that happens.
    private func warnAboutMissingRoots() {
        let missing = VaultConfig.rootPaths().filter {
            !FileManager.default.fileExists(atPath: $0)
        }
        guard !missing.isEmpty else { return }

        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "A configured folder is missing"
        alert.informativeText =
            "KBView is set up to serve:\n\n\(missing.joined(separator: "\n"))\n\n"
            + "That folder is not there, so the vault will look empty."
        // Choosing a folder rewrites the config to that one root, so it is only offered
        // when there is a single root to replace. With several, picking one would throw
        // the others away to fix the one that broke.
        let canReplace = VaultConfig.rootPaths().count == 1
        if canReplace { alert.addButton(withTitle: "Choose Folder…") }
        alert.addButton(withTitle: "Continue Anyway")
        alert.addButton(withTitle: "Quit")

        let choice = alert.runModal()
        if canReplace && choice == .alertFirstButtonReturn {
            if let folder = try? FirstRun.chooseVaultFolder() {
                try? VaultConfig.create(vaultPath: folder, name: folder.lastPathComponent)
            }
            return
        }
        let quit: NSApplication.ModalResponse =
            canReplace ? .alertThirdButtonReturn : .alertSecondButtonReturn
        if choice == quit { NSApp.terminate(nil) }
    }

    // MARK: - Menu bar

    private func buildStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        item.button?.image = NSImage(
            systemSymbolName: "books.vertical", accessibilityDescription: "KBView")
        item.button?.toolTip = "KBView"

        let menu = NSMenu()
        menu.autoenablesItems = false

        let label = NSMenuItem(title: "Starting…", action: nil, keyEquivalent: "")
        label.isEnabled = false
        menu.addItem(label)
        statusLabel = label
        menu.addItem(.separator())

        menu.addItem(
            actionItem("Open Knowledge Base", #selector(openVault)))
        menu.addItem(actionItem("Open in Browser", #selector(openInBrowser)))
        menu.addItem(actionItem("Copy Sign-in Details", #selector(copySignInDetails)))
        menu.addItem(.separator())
        menu.addItem(actionItem("Reveal Config in Finder", #selector(revealConfig)))
        menu.addItem(actionItem("Show Server Log", #selector(showLog)))

        let restart = actionItem("Restart Server", #selector(restartServer))
        menu.addItem(restart)
        restartItem = restart

        menu.addItem(.separator())
        menu.addItem(actionItem("Quit KBView", #selector(quit)))

        item.menu = menu
        statusItem = item
    }

    private func actionItem(_ title: String, _ action: Selector, key: String = "") -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        item.isEnabled = true
        return item
    }

    private func refreshStatusMenu() {
        switch server.ownership {
        case .owned:
            statusLabel?.title = "Running on port \(server.port)"
            restartItem?.isEnabled = true
            restartItem?.title = "Restart Server"
        case .external:
            statusLabel?.title = "Port \(server.port) — server started outside the app"
            restartItem?.isEnabled = false
            restartItem?.title = "Restart Server (not ours to restart)"
        }
    }

    // MARK: - The application menu bar
    //
    // Built by hand because there is no nib. The Edit menu is not optional: without
    // it, Cut/Copy/Paste do nothing inside the editor. Nothing here claims Cmd-K,
    // which the web app already binds to its search palette.

    private func buildMainMenu() {
        let mainMenu = NSMenu()
        let windowMenu = makeWindowMenu()

        // The app menu has to be first; macOS titles whichever menu is.
        for menu in [makeAppMenu(), makeEditMenu(), makeViewMenu(), windowMenu] {
            let item = NSMenuItem()
            item.submenu = menu
            mainMenu.addItem(item)
        }

        NSApp.mainMenu = mainMenu
        NSApp.windowsMenu = windowMenu
    }

    private func makeAppMenu() -> NSMenu {
        let appMenu = NSMenu()
        appMenu.addItem(
            withTitle: "About KBView", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)),
            keyEquivalent: "")
        appMenu.addItem(.separator())
        // Target left nil so it travels the responder chain to NSApp; the delegate
        // does not implement hide: and would silently disable the item.
        appMenu.addItem(
            withTitle: "Hide KBView", action: #selector(NSApplication.hide(_:)),
            keyEquivalent: "h")
        appMenu.addItem(.separator())
        appMenu.addItem(actionItem("Quit KBView", #selector(quit), key: "q"))
        return appMenu
    }

    private func makeEditMenu() -> NSMenu {
        let editMenu = NSMenu(title: "Edit")
        editMenu.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        let redo = editMenu.addItem(
            withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "z")
        redo.keyEquivalentModifierMask = [.command, .shift]
        editMenu.addItem(.separator())
        editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(
            withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        return editMenu
    }

    private func makeViewMenu() -> NSMenu {
        let viewMenu = NSMenu(title: "View")
        viewMenu.addItem(actionItem("Reload", #selector(reload), key: "r"))
        viewMenu.addItem(actionItem("Open Knowledge Base", #selector(openVault), key: "0"))
        return viewMenu
    }

    private func makeWindowMenu() -> NSMenu {
        let windowMenu = NSMenu(title: "Window")
        windowMenu.addItem(
            withTitle: "Minimise", action: #selector(NSWindow.performMiniaturize(_:)),
            keyEquivalent: "m")
        windowMenu.addItem(
            withTitle: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        return windowMenu
    }

    // MARK: - Actions

    @objc private func openVault() {
        let window =
            vaultWindow
            ?? {
                let created = VaultWindow(port: server.port)
                created.presetSession = server.adoptedSession
                created.onLoadFailure = { [weak self] detail in
                    self?.presentWarning("The vault did not load", detail: detail)
                }
                vaultWindow = created
                return created
            }()
        window.show()
    }

    @objc private func openInBrowser() {
        NSWorkspace.shared.open(URL(string: "http://127.0.0.1:\(server.port)/")!)
    }

    /// The password is generated, so nobody knows it. A browser or a phone needs it to
    /// sign in to the same server, and this is the only place to get it.
    @objc private func copySignInDetails() {
        guard let password = Credentials.load() else {
            presentWarning(
                "No stored password",
                detail: "KBView has no saved credential for \(Paths.accountEmail).")
            return
        }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString("\(Paths.accountEmail)\n\(password)", forType: .string)
        presentWarning(
            "Sign-in details copied",
            detail: "Email and password for \(Paths.accountEmail) are on the clipboard.")
    }

    @objc private func revealConfig() {
        NSWorkspace.shared.activateFileViewerSelecting([Paths.configFile])
    }

    @objc private func showLog() {
        NSWorkspace.shared.activateFileViewerSelecting([Paths.logFile])
    }

    @objc private func reload() {
        vaultWindow?.reload()
    }

    @objc private func restartServer() {
        statusLabel?.title = "Restarting…"
        server.restart { [weak self] result in
            guard let self else { return }
            switch result {
            case .success:
                self.refreshStatusMenu()
                self.vaultWindow?.repoint(toPort: self.server.port)
            case .failure(let error):
                self.refreshStatusMenu()
                self.presentWarning(
                    "The server did not restart", detail: error.localizedDescription)
            }
        }
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }

    // MARK: - Reporting

    private func reportServerStopped(_ detail: String) {
        statusLabel?.title = "Stopped"
        let alert = NSAlert()
        alert.alertStyle = .critical
        alert.messageText = "The KBView server stopped"
        alert.informativeText = detail
        alert.addButton(withTitle: "Restart")
        alert.addButton(withTitle: "Show Log")
        alert.addButton(withTitle: "Quit")

        switch alert.runModal() {
        case .alertFirstButtonReturn: restartServer()
        case .alertSecondButtonReturn: showLog()
        default: NSApp.terminate(nil)
        }
    }

    private func presentFatal(_ title: String, detail: String) {
        let alert = NSAlert()
        alert.alertStyle = .critical
        alert.messageText = title
        alert.informativeText = detail
        alert.addButton(withTitle: "Show Log")
        alert.addButton(withTitle: "Quit")
        if alert.runModal() == .alertFirstButtonReturn {
            showLog()
        }
        NSApp.terminate(nil)
    }

    private func presentWarning(_ title: String, detail: String) {
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = title
        alert.informativeText = detail
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
}
