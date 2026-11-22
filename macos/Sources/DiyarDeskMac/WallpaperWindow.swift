import Cocoa
import WebKit
import DiyarDeskCore

final class WallpaperWindow: NSWindow, WKScriptMessageHandler, WKNavigationDelegate, WKUIDelegate {
    let targetScreen: NSScreen
    private let store: ThemeStore
    private let webView: WKWebView
    private var ready = false
    private var pendingSnapshot: ((String?, String?, String?) -> Void)?

    var onReady: (() -> Void)?

    init(screen: NSScreen, store: ThemeStore, schemeHandler: LocalSchemeHandler) {
        self.targetScreen = screen
        self.store = store

        let config = WKWebViewConfiguration()
        config.setURLSchemeHandler(schemeHandler, forURLScheme: DiyarSecurity.scheme)
        let ucc = WKUserContentController()
        ucc.addUserScript(WKUserScript(source: LocalSchemeHandler.bridgeJS, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        let lowJsURL = store.prototypeDir.appendingPathComponent("low-mode.js")
        if let lowJs = try? String(contentsOf: lowJsURL, encoding: .utf8) {
            ucc.addUserScript(WKUserScript(source: lowJs, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        }
        config.userContentController = ucc
        config.mediaTypesRequiringUserActionForPlayback = []

        self.webView = WKWebView(frame: NSRect(origin: .zero, size: screen.frame.size), configuration: config)
        super.init(
            contentRect: screen.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )

        ucc.add(self, name: "diyardesk")
        self.webView.navigationDelegate = self
        self.webView.uiDelegate = self
        self.webView.setValue(false, forKey: "drawsBackground")

        self.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.desktopWindow)) + 1)
        self.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]
        self.ignoresMouseEvents = true
        self.isOpaque = false
        self.backgroundColor = .clear
        self.hasShadow = false
        self.contentView = self.webView
        self.setFrame(screen.frame, display: true)
    }

    func loadPage(_ pageRel: String) {
        ready = false
        let urlStr: String
        if pageRel.hasPrefix("__custom/") {
            let sub = String(pageRel.dropFirst("__custom/".count))
            urlStr = "\(DiyarSecurity.scheme)://\(DiyarSecurity.customHost)/\(sub)?wallpaper=1"
        } else {
            urlStr = "\(DiyarSecurity.scheme)://\(DiyarSecurity.appHost)/\(pageRel)?wallpaper=1"
        }
        if let url = URL(string: urlStr) {
            webView.load(URLRequest(url: url))
        }
    }

    func reloadPage() {
        loadPage(store.pageFor(themeId: store.settings.theme))
    }

    func postJSON(_ obj: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: obj),
              let str = String(data: data, encoding: .utf8) else { return }
        postRaw(str)
    }

    func postRaw(_ jsonStr: String) {
        guard let wrapped = try? JSONSerialization.data(withJSONObject: [jsonStr]),
              let lit = String(data: wrapped, encoding: .utf8) else { return }
        let js = "window.__diyarDispatch && window.__diyarDispatch((\(lit))[0]);"
        DispatchQueue.main.async { [weak self] in
            self?.webView.evaluateJavaScript(js, completionHandler: nil)
        }
    }

    func requestSnapshot(completion: @escaping (String?, String?, String?) -> Void) {
        pendingSnapshot = completion
        postJSON(["t": "snapshot"])
    }

    func teardown() {
        webView.configuration.userContentController.removeScriptMessageHandler(forName: "diyardesk")
        orderOut(nil)
    }

    // MARK: - WKScriptMessageHandler
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let text = message.body as? String else { return }
        if text == "ready" {
            ready = true
            onReady?()
            return
        }
        if text.hasPrefix("snapshot|") {
            let parts = text.split(separator: "|", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
            let cb = pendingSnapshot
            pendingSnapshot = nil
            if parts.count >= 3 && parts[1] != "error" {
                cb?(parts[1], parts[2], nil)
            } else {
                cb?(nil, nil, parts.count >= 3 ? parts[2] : "Snapshot failed")
            }
        }
    }

    // MARK: - WKNavigationDelegate (strict local-only sandbox)
    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        decisionHandler(DiyarSecurity.isAllowedLocalURL(navigationAction.request.url) ? .allow : .cancel)
    }

    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        return nil
    }
}
