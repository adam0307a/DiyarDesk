import Foundation
import WebKit
import DiyarDeskCore

final class LocalSchemeHandler: NSObject, WKURLSchemeHandler {
    private let store: ThemeStore

    init(store: ThemeStore) {
        self.store = store
        super.init()
    }

    func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url else {
            respondError(task: urlSchemeTask, url: URL(string: "diyardesk://app/")!, code: 400)
            return
        }
        let host = (url.host ?? "").lowercased()
        var relPath = url.path
        if relPath.hasPrefix("/") { relPath.removeFirst() }
        relPath = relPath.removingPercentEncoding ?? relPath

        if host == DiyarSecurity.appHost && relPath == "themes.json" {
            let data = store.mergedThemesJSON()
            respondData(task: urlSchemeTask, url: url, data: data, mime: "application/json; charset=utf-8")
            return
        }

        let baseDir: URL
        if host == DiyarSecurity.appHost {
            baseDir = store.prototypeDir
            if relPath.isEmpty { relPath = "papercut.html" }
        } else if host == DiyarSecurity.customHost {
            baseDir = store.customThemesDir
        } else {
            respondError(task: urlSchemeTask, url: url, code: 403)
            return
        }

        if !DiyarSecurity.isSafeRelativePath(relPath) {
            respondError(task: urlSchemeTask, url: url, code: 403)
            return
        }

        let target = baseDir.appendingPathComponent(relPath).standardizedFileURL
        let baseStd = baseDir.standardizedFileURL.path
        if target.path != baseStd && !target.path.hasPrefix(baseStd + "/") {
            respondError(task: urlSchemeTask, url: url, code: 403)
            return
        }

        var isDir: ObjCBool = false
        if !FileManager.default.fileExists(atPath: target.path, isDirectory: &isDir) || isDir.boolValue {
            respondError(task: urlSchemeTask, url: url, code: 404)
            return
        }

        guard let data = try? Data(contentsOf: target) else {
            respondError(task: urlSchemeTask, url: url, code: 500)
            return
        }
        respondData(task: urlSchemeTask, url: url, data: data, mime: DiyarSecurity.mimeType(forPath: target.path))
    }

    func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {
        // Synchronous local reads complete immediately.
    }

    private func respondData(task: WKURLSchemeTask, url: URL, data: Data, mime: String) {
        let headers: [String: String] = [
            "Content-Type": mime,
            "Content-Length": String(data.count),
            "Cache-Control": "no-store",
            "Content-Security-Policy": DiyarSecurity.cspHeader,
            "X-Content-Type-Options": "nosniff",
            "Referrer-Policy": "no-referrer"
        ]
        if let resp = HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers) {
            task.didReceive(resp)
            task.didReceive(data)
            task.didFinish()
        }
    }

    private func respondError(task: WKURLSchemeTask, url: URL, code: Int) {
        let data = Data("Error \(code)".utf8)
        let headers: [String: String] = [
            "Content-Type": "text/plain; charset=utf-8",
            "Content-Length": String(data.count),
            "Content-Security-Policy": DiyarSecurity.cspHeader
        ]
        if let resp = HTTPURLResponse(url: url, statusCode: code, httpVersion: "HTTP/1.1", headerFields: headers) {
            task.didReceive(resp)
            task.didReceive(data)
            task.didFinish()
        }
    }

    static let bridgeJS: String = """
    (function(){
      if (window.__diyarMacBridge) return;
      window.__diyarMacBridge = true;
      var listeners = [];
      var pending = [];
      window.__diyarDispatch = function(raw) {
        var str = (typeof raw === 'string') ? raw : JSON.stringify(raw);
        if (listeners.length === 0) { pending.push(str); return; }
        for (var i = 0; i < listeners.length; i++) {
          try { listeners[i]({ data: str }); } catch (e) {}
        }
      };
      window.chrome = window.chrome || {};
      window.chrome.webview = {
        postMessage: function(msg) {
          var s = (typeof msg === 'string') ? msg : JSON.stringify(msg);
          try { window.webkit.messageHandlers.diyardesk.postMessage(s); } catch (e) {}
        },
        addEventListener: function(type, fn) {
          if (type !== 'message' || typeof fn !== 'function') return;
          listeners.push(fn);
          if (pending.length) {
            var q = pending.slice();
            pending.length = 0;
            for (var i = 0; i < q.length; i++) {
              try { fn({ data: q[i] }); } catch (e) {}
            }
          }
        },
        removeEventListener: function(type, fn) {
          if (type !== 'message') return;
          listeners = listeners.filter(function(x){ return x !== fn; });
        }
      };
    })();
    """
}
