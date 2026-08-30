// Signing in to a local kbview, shared by the server adoption check and the window.

import Foundation

enum SessionLogin {
    static let cookieName = "kbv_session"

    /// The adoption check must outlast an Argon2 hash on a machine that is already busy.
    private static let loginWaitTimeout: TimeInterval = 25
    private static let okStatusCode = 200

    /// Its own session with no cookie storage: the Set-Cookie header has to be read here
    /// and handed to whoever asked, not swallowed by a shared jar.
    private static func session() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpCookieStorage = nil
        configuration.httpShouldSetCookies = false
        return URLSession(configuration: configuration)
    }

    private static func loginRequest(port: Int, password: String) -> URLRequest? {
        guard let url = URL(string: "http://127.0.0.1:\(port)/api/auth/login"),
            let body = try? JSONSerialization.data(withJSONObject: [
                "email": Paths.accountEmail, "password": password,
            ])
        else { return nil }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body
        request.timeoutInterval = 15
        return request
    }

    /// Blocking. Only for the startup path, which already runs off the main thread.
    static func attemptSynchronously(port: Int) -> HTTPCookie? {
        guard let password = Credentials.load(),
            let request = loginRequest(port: port, password: password),
            let url = request.url
        else { return nil }

        var cookie: HTTPCookie?
        let done = DispatchSemaphore(value: 0)
        session().dataTask(with: request) { _, response, _ in
            cookie = sessionCookie(from: response, requestURL: url)
            done.signal()
        }.resume()
        _ = done.wait(timeout: .now() + loginWaitTimeout)
        return cookie
    }

    /// Completes on the main queue.
    static func attempt(port: Int, completion: @escaping (HTTPCookie?) -> Void) {
        // Reading the credential and hashing the password both touch the disk and the
        // network, so neither happens on the thread that draws the window.
        DispatchQueue.global(qos: .userInitiated).async {
            let cookie = attemptSynchronously(port: port)
            DispatchQueue.main.async { completion(cookie) }
        }
    }

    /// A cookie in the web view's store is not the same thing as a session on the server.
    /// That store outlives any particular server's account data, so a cookie can sit
    /// there unexpired long after the session it names is gone - and treating its mere
    /// presence as proof lands on the login page having skipped the sign-in that would
    /// have avoided it. Completes on the main queue.
    static func isValid(_ cookie: HTTPCookie, port: Int, completion: @escaping (Bool) -> Void) {
        guard let url = URL(string: "http://127.0.0.1:\(port)/api/auth/session") else {
            DispatchQueue.main.async { completion(false) }
            return
        }
        var request = URLRequest(url: url)
        request.setValue("\(cookie.name)=\(cookie.value)", forHTTPHeaderField: "Cookie")
        request.timeoutInterval = 10
        request.cachePolicy = .reloadIgnoringLocalCacheData

        session().dataTask(with: request) { _, response, _ in
            let valid = (response as? HTTPURLResponse)?.statusCode == okStatusCode
            DispatchQueue.main.async { completion(valid) }
        }.resume()
    }

    static func sessionCookie(from response: URLResponse?, requestURL: URL) -> HTTPCookie? {
        guard let http = response as? HTTPURLResponse, http.statusCode == okStatusCode,
            let fields = http.allHeaderFields as? [String: String]
        else { return nil }
        return HTTPCookie.cookies(withResponseHeaderFields: fields, for: requestURL)
            .first { $0.name == cookieName }
    }
}
