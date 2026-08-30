// The vault window: a WKWebView pointed at the local server, signed in before it loads.

import AppKit
import WebKit

final class VaultWindow: NSObject, WKNavigationDelegate, WKUIDelegate {
    private let window: NSWindow
    private let webView: WKWebView
    private var port: Int
    private var hasAttemptedSignIn = false

    /// Handed over when the app adopted a server it had already signed in to.
    var presetSession: HTTPCookie?

    var onLoadFailure: ((String) -> Void)?

    init(port: Int) {
        self.port = port

        let configuration = WKWebViewConfiguration()
        // The persistent store keeps the session cookie between launches, so most
        // launches need no sign-in at all - sessions last 30 days.
        configuration.websiteDataStore = .default()
        webView = WKWebView(frame: .zero, configuration: configuration)
        webView.allowsBackForwardNavigationGestures = true

        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1280, height: 840),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false)
        window.title = "KBViewer"
        window.isReleasedWhenClosed = false
        window.minSize = NSSize(width: 520, height: 420)
        window.contentView = webView
        // Centre first: setting the autosave name restores the remembered frame over
        // the top of it, so doing this the other way round loses the saved position.
        window.center()
        window.setFrameAutosaveName("KBViewerVaultWindow")

        super.init()
        webView.navigationDelegate = self
        webView.uiDelegate = self
    }

    var baseURL: URL {
        URL(string: "http://127.0.0.1:\(port)/")!
    }

    func show() {
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        if webView.url == nil {
            loadSignedIn()
        }
    }

    func reload() {
        if webView.url == nil {
            loadSignedIn()
        } else {
            webView.reload()
        }
    }

    /// Called after a restart that may have landed on a different port.
    func repoint(toPort port: Int) {
        self.port = port
        hasAttemptedSignIn = false
        loadSignedIn()
    }

    // MARK: - Signing in

    private var cookieStore: WKHTTPCookieStore {
        webView.configuration.websiteDataStore.httpCookieStore
    }

    private func loadSignedIn() {
        let target = baseURL

        // A session established while adopting a running server is already valid; using
        // it saves a second sign-in, which matters because the server rate-limits them.
        if let preset = presetSession {
            presetSession = nil
            cookieStore.setCookie(preset) { _ = self.webView.load(URLRequest(url: target)) }
            return
        }

        cookieStore.getAllCookies { [weak self] cookies in
            guard let self else { return }
            let stored = cookies.first {
                $0.name == SessionLogin.cookieName && $0.domain.contains("127.0.0.1")
            }
            guard let stored, !self.hasAttemptedSignIn else {
                if self.hasAttemptedSignIn { self.show(target) } else { self.signIn(then: target) }
                return
            }
            SessionLogin.isValid(stored, port: self.port) { valid in
                if valid { self.show(target) } else { self.signIn(then: target) }
            }
        }
    }

    private func show(_ target: URL) {
        _ = webView.load(URLRequest(url: target))
    }

    /// A failed sign-in degrades to the ordinary login page rather than failing outright:
    /// the account may have been changed by hand, and that page still works.
    private func signIn(then target: URL) {
        hasAttemptedSignIn = true
        SessionLogin.attempt(port: port) { [weak self] cookie in
            guard let self else { return }
            guard let cookie else {
                self.show(target)
                return
            }
            self.cookieStore.setCookie(cookie) { self.show(target) }
        }
    }

    // MARK: - Navigation

    /// Anything that is not the local server - an external link in a note - belongs in
    /// the browser, not in this window.
    func webView(
        _ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        guard let url = navigationAction.request.url else {
            decisionHandler(.allow)
            return
        }
        if url.host == "127.0.0.1" || url.isFileURL || url.scheme == "about" {
            decisionHandler(.allow)
        } else {
            decisionHandler(.cancel)
            NSWorkspace.shared.open(url)
        }
    }

    func webView(
        _ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        if let url = navigationAction.request.url, url.host != "127.0.0.1" {
            NSWorkspace.shared.open(url)
        } else if let url = navigationAction.request.url {
            webView.load(URLRequest(url: url))
        }
        return nil
    }

    func webView(
        _ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!,
        withError error: Error
    ) {
        let failure = error as NSError
        guard failure.code != NSURLErrorCancelled else { return }
        onLoadFailure?(failure.localizedDescription)
    }
}
