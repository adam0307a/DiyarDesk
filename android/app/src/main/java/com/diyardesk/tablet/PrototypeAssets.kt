package com.diyardesk.tablet

import android.content.Context
import android.net.Uri
import android.webkit.JsPromptResult
import android.webkit.JsResult
import android.webkit.WebChromeClient
import android.webkit.WebResourceResponse
import android.webkit.WebView
import androidx.webkit.WebViewAssetLoader
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.FileInputStream
import java.io.InputStream

/**
 * Serves synced `assets/prototype/`, dynamic merged `/themes.json`, and imported `/__custom/...`
 * over `https://appassets.androidplatform.net/` cleanly without `file://` CORS issues.
 */
object PrototypeAssets {

    const val ORIGIN = "https://appassets.androidplatform.net"

    const val SECURITY_CSP =
        "default-src 'self' data: blob:; " +
            "script-src 'self' 'unsafe-inline' 'unsafe-eval' data: blob:; " +
            "style-src 'self' 'unsafe-inline' data: blob:; " +
            "img-src 'self' data: blob:; " +
            "media-src 'self' data: blob:; " +
            "font-src 'self' data: blob:; " +
            "connect-src 'self' data: blob:; " +
            "frame-src 'self' data: blob:; " +
            "object-src 'none'; " +
            "base-uri 'self'; " +
            "form-action 'none'"

    private val SECURITY_HEADERS: Map<String, String> = mapOf(
        "Content-Security-Policy" to SECURITY_CSP,
        "X-Content-Type-Options" to "nosniff",
        "Referrer-Policy" to "no-referrer"
    )

    private fun withSecurityHeaders(resp: WebResourceResponse): WebResourceResponse {
        val merged = LinkedHashMap<String, String>()
        resp.responseHeaders?.let { merged.putAll(it) }
        merged.putAll(SECURITY_HEADERS)
        resp.responseHeaders = merged
        return resp
    }

    fun blockedResponse(): WebResourceResponse = WebResourceResponse(
        "text/plain",
        "utf-8",
        403,
        "Blocked",
        SECURITY_HEADERS,
        ByteArrayInputStream(ByteArray(0))
    )

    fun createSilentWebChromeClient(): WebChromeClient = object : WebChromeClient() {
        override fun onJsAlert(view: WebView?, url: String?, message: String?, result: JsResult?): Boolean {
            result?.cancel()
            return true
        }

        override fun onJsConfirm(view: WebView?, url: String?, message: String?, result: JsResult?): Boolean {
            result?.cancel()
            return true
        }

        override fun onJsPrompt(
            view: WebView?,
            url: String?,
            message: String?,
            defaultValue: String?,
            result: JsPromptResult?
        ): Boolean {
            result?.cancel()
            return true
        }

        override fun onJsBeforeUnload(view: WebView?, url: String?, message: String?, result: JsResult?): Boolean {
            result?.cancel()
            return true
        }
    }

    /** Installed before any HTML `<script>` runs so `window.chrome.webview` works identically to Windows/Linux hosts. */
    val WEBVIEW_BRIDGE_JS = """
        (function() {
          if (window.__diyarBridgeReady) return;
          window.__diyarBridgeReady = true;
          const listeners = new Set();
          const pending = [];
          window.__diyarDispatch = function(raw) {
            const evt = { data: typeof raw === 'string' ? raw : JSON.stringify(raw) };
            for (const fn of Array.from(listeners)) {
              try { fn(evt); } catch (e) { console.error('[DiyarDesk bridge]', e); }
            }
          };
          const webviewApi = {
            postMessage(msg) {
              if (window.DiyarNative && typeof window.DiyarNative.post === 'function') {
                window.DiyarNative.post(typeof msg === 'string' ? msg : JSON.stringify(msg));
              } else {
                pending.push(msg);
              }
            },
            addEventListener(type, fn) {
              if (type === 'message' && typeof fn === 'function') listeners.add(fn);
            },
            removeEventListener(type, fn) {
              if (type === 'message') listeners.delete(fn);
            }
          };
          window.chrome = window.chrome || {};
          window.chrome.webview = webviewApi;
          window.__diyarFlushPending = function() {
            while (pending.length && window.DiyarNative) {
              const m = pending.shift();
              webviewApi.postMessage(m);
            }
          };
        })();
    """.trimIndent()

    fun createAssetLoader(context: Context, importer: CustomThemeImporter): WebViewAssetLoader {
        val assetManager = context.assets
        val rootHandler = WebViewAssetLoader.PathHandler { path ->
            val clean = path.trimStart('/').ifEmpty { "settings.html" }
            if (clean == "themes.json") {
                return@PathHandler try {
                    val base = assetManager.open("prototype/themes.json").bufferedReader(Charsets.UTF_8).use { it.readText() }
                    val merged = importer.mergedThemesJson(base).toByteArray(Charsets.UTF_8)
                    withSecurityHeaders(WebResourceResponse("application/json", "utf-8", ByteArrayInputStream(merged)))
                } catch (_: Exception) {
                    null
                }
            }
            if (clean.startsWith("__custom/")) {
                val rel = clean.removePrefix("__custom/")
                val target = importer.resolveCustomAsset(rel) ?: return@PathHandler null
                if (!target.isFile) return@PathHandler null
                return@PathHandler try {
                    val mime = CustomThemeImporter.guessMimeType(target.name)
                    withSecurityHeaders(WebResourceResponse(mime.substringBefore(';'), "utf-8", FileInputStream(target)))
                } catch (_: Exception) {
                    null
                }
            }
            if (clean.contains("..")) return@PathHandler null
            return@PathHandler try {
                val stream: InputStream = assetManager.open("prototype/$clean")
                val mime = CustomThemeImporter.guessMimeType(clean)
                withSecurityHeaders(WebResourceResponse(mime.substringBefore(';'), "utf-8", stream))
            } catch (_: Exception) {
                null
            }
        }

        return WebViewAssetLoader.Builder()
            .setDomain("appassets.androidplatform.net")
            .addPathHandler("/", rootHandler)
            .build()
    }

    fun installBridge(webView: WebView) {
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            WebViewCompat.addDocumentStartJavaScript(webView, WEBVIEW_BRIDGE_JS, setOf(ORIGIN))
        }
    }

    fun dispatchMessage(webView: WebView, payload: JSONObject) {
        val quoted = JSONObject.quote(payload.toString())
        webView.evaluateJavascript("window.__diyarDispatch && window.__diyarDispatch($quoted);", null)
    }

    fun themeUrl(page: String, params: String = "wallpaper=1"): String {
        val clean = page.trimStart('/')
        val sep = if ('?' in clean) '&' else '?'
        return "$ORIGIN/$clean$sep$params"
    }

    fun resolvePageFromThemes(context: Context, importer: CustomThemeImporter, themeId: String): String {
        val base = context.assets.open("prototype/themes.json").bufferedReader(Charsets.UTF_8).use { it.readText() }
        val merged = JSONObject(importer.mergedThemesJson(base))
        val arr = merged.optJSONArray("themes") ?: return "flat.html"
        for (i in 0 until arr.length()) {
            val obj = arr.optJSONObject(i) ?: continue
            if (obj.optString("id") == themeId) {
                return obj.optString("page", "flat.html")
            }
        }
        return "flat.html"
    }

    fun isLocalAppUrl(uri: Uri?): Boolean =
        uri?.scheme.equals("https", ignoreCase = true) &&
            uri?.host.equals("appassets.androidplatform.net", ignoreCase = true)

    fun isAllowedInWebView(uri: Uri?): Boolean {
        if (uri == null) return false
        if (isLocalAppUrl(uri)) return true
        val scheme = uri.scheme?.lowercase() ?: return false
        return scheme == "about" || scheme == "data" || scheme == "blob"
    }

    fun isSafeExternalHttpUrl(uri: Uri?): Boolean {
        if (uri == null || isLocalAppUrl(uri)) return false
        val scheme = uri.scheme?.lowercase() ?: return false
        return scheme == "http" || scheme == "https"
    }
}
