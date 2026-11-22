package com.diyardesk.tablet

import android.annotation.SuppressLint
import android.app.Presentation
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Color
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.os.BatteryManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import android.service.wallpaper.WallpaperService
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import org.json.JSONObject

/**
 * Hardware-accelerated Android Tablet Live Wallpaper Service.
 * Renders the active HTML5/Canvas/WebGL prototype theme inside a Presentation-backed VirtualDisplay
 * directly onto WallpaperService's SurfaceHolder, pausing animation whenever hidden.
 */
class DiyarWallpaperService : WallpaperService() {

    override fun onCreateEngine(): Engine = TabletWallpaperEngine()

    inner class TabletWallpaperEngine : Engine(), ThemeStore.Listener {
        private val mainHandler = Handler(Looper.getMainLooper())
        private val store by lazy { DiyarAppState.store(this@DiyarWallpaperService) }
        private val importer by lazy { DiyarAppState.importer(this@DiyarWallpaperService) }
        private val assetLoader by lazy { PrototypeAssets.createAssetLoader(this@DiyarWallpaperService, importer) }
        private val lowModeJs: String by lazy {
            try {
                assets.open("prototype/low-mode.js").bufferedReader(Charsets.UTF_8).use { it.readText() }
            } catch (_: Exception) {
                ""
            }
        }

        private var virtualDisplay: VirtualDisplay? = null
        private var presentation: WallpaperPresentation? = null
        private var webView: WebView? = null
        private var loadedThemeId: String = ""
        private var pageReady: Boolean = false
        private var surfaceW: Int = 1
        private var surfaceH: Int = 1
        private var engineVisible: Boolean = true
        private var screenOff: Boolean = false
        private var powerReceiverRegistered: Boolean = false

        private val powerReceiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context?, intent: Intent?) {
                when (intent?.action) {
                    Intent.ACTION_SCREEN_OFF -> screenOff = true
                    Intent.ACTION_SCREEN_ON, Intent.ACTION_USER_PRESENT -> screenOff = false
                }
                mainHandler.post { applyPowerAndPauseState() }
            }
        }

        private fun isPowerSaveMode(): Boolean {
            val pm = getSystemService(Context.POWER_SERVICE) as? PowerManager
            return pm?.isPowerSaveMode == true
        }

        private fun isOnBattery(): Boolean {
            val bm = getSystemService(Context.BATTERY_SERVICE) as? BatteryManager
            return if (bm != null) !bm.isCharging else true
        }

        private fun isReducedMotion(): Boolean {
            return try {
                val scale = Settings.Global.getFloat(contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
                scale == 0f
            } catch (_: Exception) {
                false
            }
        }

        private fun shouldPause(): Boolean {
            if (!engineVisible || screenOff) return true
            if (!store.disableAutoPause && isPowerSaveMode()) return true
            return false
        }

        private fun applyPowerAndPauseState() {
            val wv = webView ?: return
            val paused = shouldPause()
            if (paused) wv.onPause() else wv.onResume()
            if (pageReady) {
                PrototypeAssets.dispatchMessage(wv, JSONObject().apply {
                    put("t", "perf")
                    put("adaptiveFps", true)
                    put("onBattery", isOnBattery())
                    put("reducedMotion", isReducedMotion())
                })
                PrototypeAssets.dispatchMessage(wv, JSONObject().apply {
                    put("t", "pause")
                    put("v", paused)
                })
            }
        }

        override fun onCreate(surfaceHolder: SurfaceHolder) {
            super.onCreate(surfaceHolder)
            setTouchEventsEnabled(true)
            store.addListener(this)
            val filter = IntentFilter().apply {
                addAction(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED)
                addAction(Intent.ACTION_SCREEN_OFF)
                addAction(Intent.ACTION_SCREEN_ON)
                addAction(Intent.ACTION_USER_PRESENT)
            }
            try {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    registerReceiver(powerReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
                } else {
                    @Suppress("DEPRECATION")
                    registerReceiver(powerReceiver, filter)
                }
                powerReceiverRegistered = true
            } catch (_: Exception) {
            }
        }

        override fun onDestroy() {
            if (powerReceiverRegistered) {
                try {
                    unregisterReceiver(powerReceiver)
                } catch (_: Exception) {
                }
                powerReceiverRegistered = false
            }
            store.removeListener(this)
            mainHandler.post { tearDownPresentation() }
            super.onDestroy()
        }

        override fun onSurfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
            super.onSurfaceChanged(holder, format, width, height)
            surfaceW = width.coerceAtLeast(1)
            surfaceH = height.coerceAtLeast(1)
            mainHandler.post { rebuildPresentation(holder, surfaceW, surfaceH) }
        }

        override fun onSurfaceDestroyed(holder: SurfaceHolder) {
            mainHandler.post { tearDownPresentation() }
            super.onSurfaceDestroyed(holder)
        }

        override fun onVisibilityChanged(visible: Boolean) {
            engineVisible = visible
            if (visible) screenOff = false
            mainHandler.post { applyPowerAndPauseState() }
        }

        override fun onTouchEvent(event: MotionEvent) {
            super.onTouchEvent(event)
            val copy = MotionEvent.obtain(event)
            mainHandler.post {
                val wv = webView
                if (wv != null) wv.dispatchTouchEvent(copy)
                copy.recycle()
            }
        }

        override fun onSettingsChanged(store: ThemeStore) {
            mainHandler.post {
                val wv = webView ?: return@post
                val active = store.activeTheme
                if (active != loadedThemeId) {
                    loadActiveTheme(wv)
                } else if (pageReady) {
                    pushAllState(wv)
                }
            }
        }

        override fun onIconsChanged(store: ThemeStore) {
            mainHandler.post {
                val wv = webView ?: return@post
                if (pageReady) {
                    PrototypeAssets.dispatchMessage(wv, store.buildIconsMessage(surfaceW, surfaceH))
                }
            }
        }

        override fun onThemesRegistryChanged(store: ThemeStore) {
            onSettingsChanged(store)
        }

        private fun rebuildPresentation(holder: SurfaceHolder, width: Int, height: Int) {
            tearDownPresentation()
            val surface = holder.surface ?: return
            if (!surface.isValid) return

            val dm = getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
            val dpi = resources.displayMetrics.densityDpi
            val flags = DisplayManager.VIRTUAL_DISPLAY_FLAG_OWN_CONTENT_ONLY or
                DisplayManager.VIRTUAL_DISPLAY_FLAG_PRESENTATION
            val vd = dm.createVirtualDisplay("DiyarDeskWallpaper", width, height, dpi, surface, flags)
            virtualDisplay = vd

            val pres = WallpaperPresentation(this@DiyarWallpaperService, vd.display)
            presentation = pres
            val wv = createWallpaperWebView(pres.context)
            webView = wv
            pres.setContentView(wv)
            pres.show()
            loadActiveTheme(wv)
        }

        @SuppressLint("SetJavaScriptEnabled")
        private fun createWallpaperWebView(ctx: Context): WebView {
            return WebView(ctx).apply {
                setBackgroundColor(Color.parseColor("#0E1714"))
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
                        if (raw == "ready" || raw.contains("\"ready\"")) {
                            mainHandler.post {
                                pageReady = true
                                webView?.let { pushAllState(it) }
                            }
                        }
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

                    override fun shouldOverrideUrlLoading(
                        view: WebView,
                        request: WebResourceRequest
                    ): Boolean {
                        return !PrototypeAssets.isAllowedInWebView(request.url)
                    }

                    override fun onPageStarted(view: WebView, url: String?, favicon: android.graphics.Bitmap?) {
                        pageReady = false
                        view.evaluateJavascript(PrototypeAssets.WEBVIEW_BRIDGE_JS, null)
                        if (lowModeJs.isNotEmpty()) {
                            view.evaluateJavascript(lowModeJs, null)
                        }
                    }

                    override fun onPageFinished(view: WebView, url: String?) {
                        view.evaluateJavascript(
                            "window.__diyarFlushPending && window.__diyarFlushPending();",
                            null
                        )
                        if (!pageReady) {
                            pageReady = true
                            pushAllState(view)
                        }
                    }
                }
            }
        }

        private fun loadActiveTheme(wv: WebView) {
            val active = store.activeTheme
            loadedThemeId = active
            pageReady = false
            val page = PrototypeAssets.resolvePageFromThemes(this@DiyarWallpaperService, importer, active)
            val query = if (store.lowMode) "wallpaper=1&low=1" else "wallpaper=1"
            wv.loadUrl(PrototypeAssets.themeUrl(page, query))
        }

        private fun pushAllState(wv: WebView) {
            PrototypeAssets.dispatchMessage(wv, JSONObject().apply {
                put("t", "lowMode")
                put("v", store.lowMode)
            })
            PrototypeAssets.dispatchMessage(wv, JSONObject().apply {
                put("t", "qualityProfile")
                put("v", store.qualityProfile)
            })
            PrototypeAssets.dispatchMessage(wv, JSONObject().apply {
                put("t", "perf")
                put("adaptiveFps", true)
                put("onBattery", isOnBattery())
                put("reducedMotion", isReducedMotion())
            })
            PrototypeAssets.dispatchMessage(wv, store.buildWallpaperSettingsMessage())
            PrototypeAssets.dispatchMessage(wv, store.buildIconsMessage(surfaceW, surfaceH))
            val paused = shouldPause()
            if (paused) wv.onPause() else wv.onResume()
            PrototypeAssets.dispatchMessage(wv, JSONObject().apply {
                put("t", "pause")
                put("v", paused)
            })
        }

        private fun tearDownPresentation() {
            pageReady = false
            webView?.let { wv ->
                wv.stopLoading()
                wv.destroy()
            }
            webView = null
            presentation?.dismiss()
            presentation = null
            virtualDisplay?.release()
            virtualDisplay = null
        }
    }

    private class WallpaperPresentation(
        outerContext: Context,
        display: android.view.Display
    ) : Presentation(outerContext, display)
}
