import Cocoa
import WebKit
import UniformTypeIdentifiers
import DiyarDeskCore

final class SettingsWindowController: NSWindowController, WKScriptMessageHandler, WKNavigationDelegate, WKUIDelegate, NSWindowDelegate {
    private let store: ThemeStore
    private let webView: WKWebView
    private unowned let coordinator: AppDelegate

    init(store: ThemeStore, schemeHandler: LocalSchemeHandler, coordinator: AppDelegate) {
        self.store = store
        self.coordinator = coordinator

        let config = WKWebViewConfiguration()
        config.setURLSchemeHandler(schemeHandler, forURLScheme: DiyarSecurity.scheme)
        let ucc = WKUserContentController()
        ucc.addUserScript(WKUserScript(source: LocalSchemeHandler.bridgeJS, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        config.userContentController = ucc

        let frame = NSRect(x: 0, y: 0, width: 1120, height: 740)
        self.webView = WKWebView(frame: frame, configuration: config)
        let win = NSWindow(
            contentRect: frame,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        win.title = "DiyarDesk"
        win.center()
        win.contentView = self.webView
        win.minSize = NSSize(width: 820, height: 560)

        super.init(window: win)
        win.delegate = self
        ucc.add(self, name: "diyardesk")
        webView.navigationDelegate = self
        webView.uiDelegate = self

        if let url = URL(string: "\(DiyarSecurity.scheme)://\(DiyarSecurity.appHost)/settings.html") {
            webView.load(URLRequest(url: url))
        }
    }

    required init?(coder: NSCoder) { return nil }

    func postJSON(_ obj: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: obj),
              let str = String(data: data, encoding: .utf8),
              let wrapped = try? JSONSerialization.data(withJSONObject: [str]),
              let lit = String(data: wrapped, encoding: .utf8) else { return }
        let js = "window.__diyarDispatch && window.__diyarDispatch((\(lit))[0]);"
        DispatchQueue.main.async { [weak self] in
            self?.webView.evaluateJavaScript(js, completionHandler: nil)
        }
    }

    func windowWillClose(_ notification: Notification) {
        webView.configuration.userContentController.removeScriptMessageHandler(forName: "diyardesk")
        coordinator.onSettingsWindowClosed()
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let raw = message.body as? String,
              let data = raw.data(using: .utf8),
              let msg = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let kind = msg["t"] as? String else { return }
        coordinator.handleSettingsMessage(kind: kind, msg: msg, controller: self)
    }

    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        let url = navigationAction.request.url
        if DiyarSecurity.isAllowedLocalURL(url) {
            decisionHandler(.allow)
            return
        }
        if let u = url, (u.scheme == "https" || u.scheme == "http") {
            NSWorkspace.shared.open(u)
        }
        decisionHandler(.cancel)
    }

    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if let u = navigationAction.request.url, (u.scheme == "https" || u.scheme == "http") {
            NSWorkspace.shared.open(u)
        }
        return nil
    }

    func openImportDialog(completion: @escaping (URL?) -> Void) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.title = "Import DiyarDesk Theme"
        panel.beginSheetModal(for: window!) { resp in
            completion(resp == .OK ? panel.url : nil)
        }
    }
}
