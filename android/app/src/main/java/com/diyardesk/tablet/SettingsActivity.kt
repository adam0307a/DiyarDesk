package com.diyardesk.tablet

import android.annotation.SuppressLint
import android.app.WallpaperManager
import android.content.ComponentName
import android.content.ContentValues
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Debug
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.text.TextUtils
import android.util.Base64
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Button
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.Executors
import kotlin.math.roundToInt

/**
 * Main tablet configuration UI.
 *
 * When Rust + OpenGL ES 3.0 mode is active, mounts [NativeSettingsController] (pure native
 * Android Views + static preview thumbnails, completely closing and avoiding [WebView]).
 * When switched to WebView mode, hosts the shared `prototype/settings.html` interface.
 */
class SettingsActivity : AppCompatActivity(), ThemeStore.Listener {

    private lateinit var store: ThemeStore
    private lateinit var importer: CustomThemeImporter
    private lateinit var brandLabel: TextView
    private lateinit var contentHost: FrameLayout
    private var webView: WebView? = null
    private var nativeSettings: NativeSettingsController? = null
    private var lastSavedShareUri: Uri? = null
    private val mainHandler = Handler(Looper.getMainLooper())
    private val memExecutor = Executors.newSingleThreadExecutor()
    private var webMemPolling = false
    private val webMemRunnable = object : Runnable {
        override fun run() {
            if (!webMemPolling || webView == null) return
            dispatchWebMemStats()
            mainHandler.postDelayed(this, 3000L)
        }
    }

    private val importPicker = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri == null) return@registerForActivityResult
        handlePickedThemeUri(uri)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = DiyarAppState.store(this)
        importer = DiyarAppState.importer(this)
        store.addListener(this)

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.parseColor("#0E1D19"))
        }

        // Top tablet action bar (two-row layout on narrow/split screens <600dp so brandLabel never squeezes contentHost to 0px)
        val compactBar = resources.configuration.screenWidthDp < 600
        val bar = LinearLayout(this).apply {
            orientation = if (compactBar) LinearLayout.VERTICAL else LinearLayout.HORIZONTAL
            setBackgroundColor(Color.parseColor("#132A24"))
            setPadding(24, 14, 24, 14)
            gravity = if (compactBar) Gravity.START else Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
        }

        brandLabel = TextView(this).apply {
            text = getString(R.string.app_name) + " Tablet"
            setTextColor(Color.WHITE)
            textSize = 18f
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
            layoutParams = if (compactBar) {
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                ).apply { bottomMargin = 10 }
            } else {
                LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            }
        }

        val btnIcons = Button(this).apply {
            text = getString(R.string.btn_icon_layout)
            isAllCaps = false
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
            setTextColor(Color.WHITE)
            setBackgroundColor(Color.parseColor("#24594B"))
            if (compactBar) {
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            }
            setOnClickListener { startActivity(Intent(this@SettingsActivity, IconLayoutActivity::class.java)) }
        }

        val btnWallpaper = Button(this).apply {
            text = getString(R.string.btn_set_wallpaper)
            isAllCaps = false
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
            setTextColor(Color.WHITE)
            setBackgroundColor(Color.parseColor("#1F6F5B"))
            val lp = if (compactBar) {
                LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            } else {
                LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            }
            lp.marginStart = 12
            layoutParams = lp
            setOnClickListener { openLiveWallpaperChooser() }
        }

        bar.addView(brandLabel)
        if (compactBar) {
            val btnRow = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                layoutParams = LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
            }
            btnRow.addView(btnIcons)
            btnRow.addView(btnWallpaper)
            bar.addView(btnRow)
        } else {
            bar.addView(btnIcons)
            bar.addView(btnWallpaper)
        }

        root.addView(bar)

        // Show non-blocking notice banner only if opened on a phone-sized (<600dp) screen
        if (resources.configuration.smallestScreenWidthDp < 600) {
            val banner = TextView(this).apply {
                text = getString(R.string.tablet_notice)
                setTextColor(Color.parseColor("#FFE4B8"))
                setBackgroundColor(Color.parseColor("#4D3518"))
                textSize = 12f
                setPadding(24, 10, 24, 10)
            }
            root.addView(banner)
        }

        contentHost = FrameLayout(this).apply {
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f)
        }
        root.addView(contentHost)
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            insets
        }
        setContentView(root)

        syncSettingsEngine()
    }

    override fun onResume() {
        super.onResume()
        nativeSettings?.setPollingActive(true)
        if (webView != null) startWebMemPolling()
    }

    override fun onPause() {
        nativeSettings?.setPollingActive(false)
        stopWebMemPolling()
        super.onPause()
    }

    override fun onDestroy() {
        stopWebMemPolling()
        try {
            memExecutor.shutdownNow()
        } catch (_: Exception) {
        }
        store.removeListener(this)
        destroyNativeSettings()
        destroyWebView()
        super.onDestroy()
    }

    private fun readProcessMemMb(): Pair<Int, Int> {
        return try {
            val mi = Debug.MemoryInfo()
            Debug.getMemoryInfo(mi)
            val pssMb = (mi.totalPss / 1024.0).roundToInt().coerceAtLeast(1)
            val gfxKb = mi.getMemoryStat("summary.graphics")?.toIntOrNull() ?: 0
            val glMb = (gfxKb / 1024.0).roundToInt()
            pssMb to glMb
        } catch (_: Exception) {
            0 to 0
        }
    }

    private fun dispatchWebMemStats() {
        if (isFinishing || isDestroyed || webView == null || memExecutor.isShutdown) return
        try {
            memExecutor.execute {
                val (pssMb, glMb) = readProcessMemMb()
                if (pssMb <= 0) return@execute
                runOnUiThread {
                    if (isFinishing || isDestroyed || webView == null) return@runOnUiThread
                    dispatchToWebView(JSONObject().apply {
                        put("t", "memStats")
                        put("ramMb", pssMb)
                        put("glMb", glMb)
                        put("engine", "webview")
                    })
                }
            }
        } catch (_: Exception) {
        }
    }

    private fun startWebMemPolling() {
        if (webView == null) return
        if (webMemPolling) return
        webMemPolling = true
        mainHandler.removeCallbacks(webMemRunnable)
        mainHandler.postDelayed(webMemRunnable, 3000L)
    }

    private fun stopWebMemPolling() {
        webMemPolling = false
        mainHandler.removeCallbacks(webMemRunnable)
    }

    override fun onIconsChanged(store: ThemeStore) {
        runOnUiThread {
            val wv = webView ?: return@runOnUiThread
            val (w, h) = DiyarAppState.screenPhysicalSize(this)
            PrototypeAssets.dispatchMessage(wv, store.buildIconsMessage(w, h))
        }
    }

    override fun onThemesRegistryChanged(store: ThemeStore) {
        runOnUiThread {
            nativeSettings?.refresh()
        }
    }

    private fun updateBrandLabel(nativeOpenGlActive: Boolean) {
        val engineTag = when {
            nativeOpenGlActive -> " · OpenGL ES 3.0"
            NativeGlBridge.isLibraryLoaded -> " · WebView"
            else -> ""
        }
        brandLabel.text = getString(R.string.app_name) + " Tablet" + engineTag
    }

    private fun dispatchToWebView(payload: JSONObject) {
        val wv = webView ?: return
        PrototypeAssets.dispatchMessage(wv, payload)
    }

    private fun syncSettingsEngine() {
        val useNative = NativeGlProtocol.shouldUseNativeSettings(store.rustOpenGl, NativeGlBridge.isLibraryLoaded)
        updateBrandLabel(useNative)
        if (useNative) {
            // Completely shut down WebView in OpenGL mode (zero WebView / zero iframe preview)
            destroyWebView()
            if (nativeSettings == null) {
                contentHost.removeAllViews()
                val controller = NativeSettingsController(
                    context = this,
                    store = store,
                    importer = importer,
                    onToggleRustOpenGl = { enabled ->
                        store.setRustOpenGlEnabled(enabled)
                        contentHost.post { syncSettingsEngine() }
                    },
                    onImportTheme = { importPicker.launch(arrayOf("*/*")) },
                    onRemoveTheme = { id -> removeCustomTheme(id) }
                )
                nativeSettings = controller
                contentHost.addView(
                    controller.rootView,
                    FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
                )
            } else {
                nativeSettings?.refresh()
            }
        } else {
            destroyNativeSettings()
            if (webView == null) {
                contentHost.removeAllViews()
                val wv = createSettingsWebView()
                webView = wv
                contentHost.addView(
                    wv,
                    FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
                )
                wv.loadUrl("${PrototypeAssets.ORIGIN}/settings.html")
                startWebMemPolling()
            }
        }
    }

    private fun destroyWebView() {
        stopWebMemPolling()
        val wv = webView ?: return
        webView = null
        try {
            wv.stopLoading()
            wv.onPause()
            wv.removeJavascriptInterface("DiyarNative")
            wv.loadUrl("about:blank")
            wv.clearHistory()
            (wv.parent as? ViewGroup)?.removeView(wv)
            wv.removeAllViews()
        } catch (_: Exception) {
        }
        wv.destroy()
    }

    private fun destroyNativeSettings() {
        val ns = nativeSettings ?: return
        nativeSettings = null
        try {
            (ns.rootView.parent as? ViewGroup)?.removeView(ns.rootView)
        } catch (_: Exception) {
        }
        ns.destroy()
    }

    private fun removeCustomTheme(id: String) {
        if (importer.removeTheme(id)) {
            if (store.activeTheme == id) {
                store.setActiveTheme(ThemeStore.DEFAULT_THEME)
            } else {
                store.notifyThemesRegistryChanged()
            }
            nativeSettings?.refresh()
            dispatchToWebView(JSONObject().apply {
                put("t", "themeRemoved")
                put("id", id)
                put("active", store.activeTheme)
            })
        }
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun createSettingsWebView(): WebView {
        val assetLoader = PrototypeAssets.createAssetLoader(this, importer)
        return WebView(this).apply {
            settings.apply {
                javaScriptEnabled = true
                domStorageEnabled = true
                mediaPlaybackRequiresUserGesture = false
                allowFileAccess = false
                allowContentAccess = false
                cacheMode = WebSettings.LOAD_DEFAULT
            }
            PrototypeAssets.installBridge(this)
            addJavascriptInterface(object {
                @JavascriptInterface
                fun post(raw: String) {
                    runOnUiThread { handleUiMessage(raw) }
                }
            }, "DiyarNative")
            webChromeClient = PrototypeAssets.createSilentWebChromeClient()
            webViewClient = object : WebViewClient() {
                override fun shouldInterceptRequest(
                    view: WebView,
                    request: WebResourceRequest
                ): WebResourceResponse? {
                    if (!PrototypeAssets.isAllowedInWebView(request.url)) {
                        return PrototypeAssets.blockedResponse()
                    }
                    return assetLoader.shouldInterceptRequest(request.url)
                }

                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    val uri = request.url
                    if (PrototypeAssets.isSafeExternalHttpUrl(uri)) {
                        try {
                            startActivity(Intent(Intent.ACTION_VIEW, uri))
                        } catch (_: Exception) {
                        }
                        return true
                    }
                    return !PrototypeAssets.isAllowedInWebView(uri)
                }

                override fun onPageStarted(view: WebView, url: String?, favicon: android.graphics.Bitmap?) {
                    view.evaluateJavascript(PrototypeAssets.WEBVIEW_BRIDGE_JS, null)
                }

                override fun onPageFinished(view: WebView, url: String?) {
                    view.evaluateJavascript(
                        "window.__diyarFlushPending && window.__diyarFlushPending();",
                        null
                    )
                }
            }
        }
    }

    private fun handleUiMessage(raw: String) {
        val msg = try { JSONObject(raw) } catch (_: Exception) { return }
        when (msg.optString("t")) {
            "hello" -> {
                val (w, h) = DiyarAppState.screenPhysicalSize(this)
                val sysLang = DiyarAppState.systemLangCode()
                dispatchToWebView(store.buildInitMessage(w, h, sysLang))
                dispatchToWebView(store.buildIconsMessage(w, h))
                dispatchWebMemStats()
                startWebMemPolling()
            }
            "set" -> {
                val th = msg.optString("theme", store.activeTheme)
                val k = msg.optString("key")
                val v = msg.opt("value")
                store.setParam(th, k, v)
            }
            "reset" -> {
                store.resetThemeParams(msg.optString("theme", store.activeTheme))
            }
            "theme" -> {
                store.setActiveTheme(msg.optString("id"))
            }
            "lang" -> {
                val v = msg.optString("v", "auto")
                store.setLanguage(v)
                val sysLang = DiyarAppState.systemLangCode()
                dispatchToWebView(JSONObject().apply {
                    put("t", "lang")
                    put("v", v)
                    put("sys", sysLang)
                    put("eff", ThemeStore.resolveEffectiveLang(v, sysLang))
                })
            }
            "virtualBarMode" -> {
                store.setVirtualBarModeValue(msg.optString("v", "auto"))
            }
            "lowMode" -> {
                store.setLowModeEnabled(msg.optBoolean("v", false))
            }
            "disableAutoPause" -> {
                store.setDisableAutoPauseEnabled(msg.optBoolean("v", false))
            }
            "qualityProfile" -> {
                store.setQualityProfileValue(msg.optString("v", "balanced"))
            }
            "rustOpenGl" -> {
                val wantGl = msg.optBoolean("v", true) && NativeGlBridge.isLibraryLoaded
                store.setRustOpenGlEnabled(wantGl)
                if (NativeGlProtocol.shouldUseNativeSettings(store.rustOpenGl, NativeGlBridge.isLibraryLoaded)) {
                    // Defer WebView destruction to next UI tick so the JS bridge call unwinds cleanly.
                    contentHost.post { syncSettingsEngine() }
                } else {
                    dispatchToWebView(JSONObject().apply {
                        put("t", "rustOpenGl")
                        put("v", store.rustOpenGl)
                        put(
                            "nativeActive",
                            NativeGlProtocol.shouldUseNativeWallpaper(
                                store.activeTheme,
                                store.rustOpenGl,
                                NativeGlBridge.isLibraryLoaded,
                                null
                            )
                        )
                    })
                }
            }
            "importTheme" -> {
                importPicker.launch(arrayOf("*/*"))
            }
            "removeTheme" -> {
                removeCustomTheme(msg.optString("id"))
            }
            "share" -> savePngSnapshot(msg.optString("dataUrl"))
            "lanSyncConnect" -> {
                val input = msg.optString("uri").ifBlank { msg.optString("address") }
                val pin = msg.optString("pin")
                syncFromLanDesktop(input, pin)
            }
            "openShare" -> {
                lastSavedShareUri?.let { uri ->
                    startActivity(Intent(Intent.ACTION_VIEW).setDataAndType(uri, "image/png").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION))
                }
            }
            "openWebsite" -> {
                val url = msg.optString("url")
                if (url.startsWith("https://")) {
                    startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                }
            }
            "pickAudio" -> {
                Toast.makeText(this, R.string.toast_audio_not_supported, Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun syncFromLanDesktop(rawInput: String, fallbackPin: String) {
        val target = LanSyncClient.parsePairingInput(rawInput, fallbackPin)
        if (target == null) {
            dispatchToWebView(JSONObject().apply {
                put("t", "lanSyncResult")
                put("ok", false)
                put("msg", "Invalid private LAN address or 4-digit PIN.")
            })
            return
        }
        Thread {
            try {
                val info = LanSyncClient.fetchPairInfo(target)
                var activeTheme = info.optString("activeTheme", ThemeStore.DEFAULT_THEME)
                var importedObj: JSONObject? = null
                if (activeTheme.startsWith("custom-")) {
                    importedObj = LanSyncClient.downloadCustomTheme(target, activeTheme, importer)
                    activeTheme = importedObj.optString("id", activeTheme)
                }
                val params = info.optJSONObject("params")
                if (params != null) {
                    val keys = params.keys()
                    while (keys.hasNext()) {
                        val k = keys.next()
                        store.setParam(activeTheme, k, params.opt(k))
                    }
                }
                store.setActiveTheme(activeTheme)
                if (importedObj != null) {
                    store.notifyThemesRegistryChanged()
                }
                runOnUiThread {
                    nativeSettings?.refresh()
                    if (importedObj != null) {
                        dispatchToWebView(JSONObject().apply {
                            put("t", "themeImported")
                            put("theme", importedObj)
                            put("select", true)
                        })
                    } else {
                        dispatchToWebView(JSONObject().apply {
                            put("t", "themeChanged")
                            put("id", activeTheme)
                        })
                    }
                    dispatchToWebView(JSONObject().apply {
                        put("t", "lanSyncResult")
                        put("ok", true)
                        put("theme", activeTheme)
                    })
                }
            } catch (e: Exception) {
                runOnUiThread {
                    dispatchToWebView(JSONObject().apply {
                        put("t", "lanSyncResult")
                        put("ok", false)
                        put("msg", e.message ?: "LAN sync failed")
                    })
                }
            }
        }.start()
    }

    private fun handlePickedThemeUri(uri: Uri) {
        val displayName = queryDisplayName(uri) ?: "imported.zip"
        try {
            val entry = contentResolver.openInputStream(uri)?.use { stream ->
                importer.importStream(stream, displayName)
            } ?: throw IllegalStateException("Cannot open selected file")
            val newId = entry.optString("id")
            store.setActiveTheme(newId)
            store.notifyThemesRegistryChanged()
            nativeSettings?.refresh()
            dispatchToWebView(JSONObject().apply {
                put("t", "themeImported")
                put("theme", entry)
                put("select", true)
            })
        } catch (t: Throwable) {
            val err = t.message ?: "Import failed"
            if (webView == null) {
                Toast.makeText(this, err, Toast.LENGTH_LONG).show()
            }
            dispatchToWebView(JSONObject().apply {
                put("t", "importError")
                put("msg", err)
            })
        }
    }

    private fun queryDisplayName(uri: Uri): String? {
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) return c.getString(0)
        }
        return uri.lastPathSegment
    }

    private fun savePngSnapshot(dataUrl: String) {
        val b64 = dataUrl.substringAfter("base64,", "")
        if (b64.isBlank()) return
        try {
            val bytes = Base64.decode(b64, Base64.DEFAULT)
            val filename = "DiyarDesk-${System.currentTimeMillis()}.png"
            val values = ContentValues().apply {
                put(MediaStore.Images.Media.DISPLAY_NAME, filename)
                put(MediaStore.Images.Media.MIME_TYPE, "image/png")
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    put(MediaStore.Images.Media.RELATIVE_PATH, "${Environment.DIRECTORY_PICTURES}/DiyarDesk")
                }
            }
            val uri = contentResolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values)
            if (uri != null) {
                contentResolver.openOutputStream(uri)?.use { it.write(bytes) }
                lastSavedShareUri = uri
                Toast.makeText(this, R.string.toast_share_saved, Toast.LENGTH_SHORT).show()
                dispatchToWebView(JSONObject().apply {
                    put("t", "shareResult")
                    put("ok", true)
                    put("path", "${Environment.DIRECTORY_PICTURES}/DiyarDesk/$filename")
                })
            }
        } catch (t: Throwable) {
            Toast.makeText(this, R.string.toast_share_failed, Toast.LENGTH_SHORT).show()
            dispatchToWebView(JSONObject().apply {
                put("t", "shareResult")
                put("ok", false)
                put("error", t.message ?: "Save failed")
            })
        }
    }

    private fun openLiveWallpaperChooser() {
        val component = ComponentName(this, DiyarWallpaperService::class.java)
        val direct = Intent(WallpaperManager.ACTION_CHANGE_LIVE_WALLPAPER).apply {
            putExtra(WallpaperManager.EXTRA_LIVE_WALLPAPER_COMPONENT, component)
        }
        try {
            startActivity(direct)
        } catch (_: Exception) {
            try {
                startActivity(Intent(WallpaperManager.ACTION_LIVE_WALLPAPER_CHOOSER))
            } catch (_: Exception) {
                Toast.makeText(this, R.string.toast_wallpaper_picker_unavailable, Toast.LENGTH_LONG).show()
            }
        }
    }
}
