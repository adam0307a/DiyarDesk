import Cocoa
import WebKit
import DiyarDeskCore

final class WallpaperWindow: NSWindow, WKScriptMessageHandler, WKNavigationDelegate, WKUIDelegate {
    let targetScreen: NSScreen
    private let store: ThemeStore
    private let schemeHandler: LocalSchemeHandler
    private var webView: WKWebView?
    private var nativeGlView: NativeGlView?
    private var nativeVideoView: NativeVideoView?
    private var currentThemeId: String = ""
    private var currentPage: String = ""
    private var ready = false
    private var pendingSnapshot: ((String?, String?, String?) -> Void)?

    var onReady: (() -> Void)?

    var activeBackend: String {
        if nativeGlView != nil { return "rust-gl" }
        if nativeVideoView != nil { return "native-video" }
        return "webview"
    }

    var isNativeActive: Bool {
        return nativeGlView != nil || nativeVideoView != nil
    }

    init(screen: NSScreen, store: ThemeStore, schemeHandler: LocalSchemeHandler) {
        self.targetScreen = screen
        self.store = store
        self.schemeHandler = schemeHandler

        super.init(
            contentRect: screen.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )

        self.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.desktopWindow)) + 1)
        self.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]
        self.ignoresMouseEvents = true
        self.isOpaque = false
        self.backgroundColor = .clear
        self.hasShadow = false
        self.setFrame(screen.frame, display: true)
    }

    private func ensureWebView() -> WKWebView {
        if let existing = webView { return existing }
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

        let wv = WKWebView(frame: NSRect(origin: .zero, size: targetScreen.frame.size), configuration: config)
        ucc.add(self, name: "diyardesk")
        wv.navigationDelegate = self
        wv.uiDelegate = self
        wv.setValue(false, forKey: "drawsBackground")
        self.webView = wv
        return wv
    }

    private func releaseWebView() {
        guard let wv = webView else { return }
        webView = nil
        wv.navigationDelegate = nil
        wv.uiDelegate = nil
        wv.configuration.userContentController.removeScriptMessageHandler(forName: "diyardesk")
        wv.loadHTMLString("", baseURL: nil)
        wv.removeFromSuperview()
    }

    private func releaseNativeGl() {
        guard let gl = nativeGlView else { return }
        nativeGlView = nil
        gl.teardown()
    }

    private func releaseNativeVideo() {
        guard let vid = nativeVideoView else { return }
        nativeVideoView = nil
        vid.teardown()
    }

    private func startNativeGl(themeId: String) -> Bool {
        guard let lib = NativeGlLibrary.shared else { return false }
        releaseNativeVideo()
        releaseWebView()

        let glView: NativeGlView
        if let existing = nativeGlView {
            glView = existing
        } else if let created = NativeGlView(frame: NSRect(origin: .zero, size: targetScreen.frame.size), lib: lib) {
            nativeGlView = created
            glView = created
            self.contentView = glView
        } else {
            return false
        }

        glView.setLowMode(store.settings.lowMode)
        glView.postJSON(["t": "lowMode", "v": store.settings.lowMode])
        glView.postJSON(["t": "theme", "id": themeId])
        glView.postJSON(["t": "settings", "v": store.paramsFor(themeId)])
        ready = true
        onReady?()
        return true
    }

    private func startNativeVideo(themeId: String, videoURL: URL) -> Bool {
        releaseNativeGl()
        releaseWebView()

        let p = store.paramsFor(themeId)
        let spd = (p["speed"] as? NSNumber)?.doubleValue ?? 1.0
        let bright = (p["brightness"] as? NSNumber)?.doubleValue ?? 1.0

        if let existing = nativeVideoView, existing.matchesURL(videoURL) {
            existing.updateParams(speed: spd, brightness: bright)
            ready = true
            onReady?()
            return true
        }

        releaseNativeVideo()
        let vidView = NativeVideoView(
            frame: NSRect(origin: .zero, size: targetScreen.frame.size),
            videoURL: videoURL,
            speed: spd,
            brightness: bright
        )
        vidView.onFailure = { [weak self] _ in
            guard let self = self else { return }
            self.releaseNativeVideo()
            self.loadWebViewPage(self.currentPage)
        }
        nativeVideoView = vidView
        self.contentView = vidView
        ready = true
        onReady?()
        return true
    }

    private func loadWebViewPage(_ pageRel: String) {
        releaseNativeGl()
        releaseNativeVideo()
        ready = false
        let wv = ensureWebView()
        if self.contentView !== wv {
            self.contentView = wv
        }
        let urlStr: String
        if pageRel.hasPrefix("__custom/") {
            let sub = String(pageRel.dropFirst("__custom/".count))
            urlStr = "\(DiyarSecurity.scheme)://\(DiyarSecurity.customHost)/\(sub)?wallpaper=1"
        } else {
            urlStr = "\(DiyarSecurity.scheme)://\(DiyarSecurity.appHost)/\(pageRel)?wallpaper=1"
        }
        if let url = URL(string: urlStr) {
            wv.load(URLRequest(url: url))
        }
    }

    func loadPage(_ pageRel: String, themeId: String? = nil) {
        let tid = (themeId?.isEmpty == false) ? themeId! : store.settings.theme
        currentPage = pageRel
        currentThemeId = tid

        if NativeGlSupport.shouldUseNativeGl(
            page: pageRel,
            themeId: tid,
            rustOpenGl: store.settings.rustOpenGl,
            glAvailable: NativeGlLibrary.isAvailable
        ) {
            if startNativeGl(themeId: tid) {
                return
            }
        }

        if (store.settings.nativeVideo || store.settings.rustOpenGl),
           let videoURL = store.resolveAVPlayerVideoURL(themeId: tid),
           startNativeVideo(themeId: tid, videoURL: videoURL) {
            return
        }

        loadWebViewPage(pageRel)
    }

    func reloadPage() {
        let tid = store.settings.theme
        loadPage(store.pageFor(themeId: tid), themeId: tid)
    }

    func postJSON(_ obj: [String: Any]) {
        if let gl = nativeGlView {
            if let kind = obj["t"] as? String {
                if kind == "pause", let v = obj["v"] as? Bool {
                    gl.setPaused(v)
                } else if kind == "lowMode", let v = obj["v"] as? Bool {
                    gl.setLowMode(v)
                }
            }
            gl.postJSON(obj)
            return
        }

        if let vid = nativeVideoView {
            guard let kind = obj["t"] as? String else { return }
            if kind == "pause", let v = obj["v"] as? Bool {
                vid.setPaused(v)
            } else if kind == "reset" {
                vid.updateParams(speed: 1.0, brightness: 1.0)
            } else if kind == "settings" {
                let saved = store.paramsFor(currentThemeId)
                let spd = (saved["speed"] as? NSNumber)?.doubleValue ?? 1.0
                let bright = (saved["brightness"] as? NSNumber)?.doubleValue ?? 1.0
                vid.updateParams(speed: spd, brightness: bright)
            }
            return
        }

        guard let data = try? JSONSerialization.data(withJSONObject: obj),
              let str = String(data: data, encoding: .utf8) else { return }
        postRaw(str)
    }

    func postRaw(_ jsonStr: String) {
        if let gl = nativeGlView {
            gl.postRaw(jsonStr)
            return
        }
        guard nativeVideoView == nil, let wv = webView else { return }
        guard let wrapped = try? JSONSerialization.data(withJSONObject: [jsonStr]),
              let lit = String(data: wrapped, encoding: .utf8) else { return }
        let js = "window.__diyarDispatch && window.__diyarDispatch((\(lit))[0]);"
        DispatchQueue.main.async { [weak wv] in
            wv?.evaluateJavaScript(js, completionHandler: nil)
        }
    }

    func requestSnapshot(completion: @escaping (String?, String?, String?) -> Void) {
        if isNativeActive {
            completion(nil, nil, "Snapshot is only available on WebKit themes.")
            return
        }
        pendingSnapshot = completion
        postJSON(["t": "snapshot"])
    }

    func teardown() {
        releaseNativeGl()
        releaseNativeVideo()
        releaseWebView()
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
