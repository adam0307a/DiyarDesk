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
import android.util.Log
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import org.json.JSONObject
import java.io.File

/**
 * Hardware-accelerated Android Tablet Live Wallpaper Service.
 *
 * Primary path: renders all built-in themes natively via Rust + OpenGL ES 3.0 (`libdiyardesk_gl.so`)
 * directly onto [WallpaperService.Engine]'s [SurfaceHolder] using EGL 1.4 with zero WebView overhead.
 *
 * Fallback path: seamlessly renders imported custom HTML/image/video themes (or falls back if
 * native GL initialization fails) inside a Presentation-backed [VirtualDisplay] WebView.
 */
class DiyarWallpaperService : WallpaperService() {

    companion object {
        private const val TAG = "DiyarWallpaperService"
    }

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

        // Active SurfaceHolder state
        private var currentHolder: SurfaceHolder? = null
        private var activeNativeSurface: Surface? = null

        // Native Rust + OpenGL ES 3.0 backend
        private var nativeRenderer: NativeGlWallpaperRenderer? = null
        private var nativeFailedForTheme: String? = null
        private var nativeRecoveryAttempts: Int = 0
        private var lastNativeParamKeys: Set<String> = emptySet()

        // Zero-WebView hardware MediaPlayer backend for imported video wallpapers
        private var nativeVideoPlayer: NativeVideoWallpaperPlayer? = null
        private var activeVideoSurface: Surface? = null
        private var videoFailedForTheme: String? = null

        // WebView fallback backend (custom HTML/image/video themes or devices without native lib)
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

        private fun shouldUseNativeGl(themeId: String): Boolean {
            return NativeGlProtocol.shouldUseNativeWallpaper(
                themeId = themeId,
                rustOpenGl = store.rustOpenGl,
                libLoaded = NativeGlBridge.isLibraryLoaded,
                failedTheme = nativeFailedForTheme
            )
        }

        private fun resolveNativeVideoFile(themeId: String): File? {
            if (videoFailedForTheme == themeId) return null
            return importer.resolveCustomVideoFile(themeId)
        }

        private fun displayDensity(): Float {
            return resources.displayMetrics.density.coerceAtLeast(0.5f)
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
            val paused = shouldPause()
            val onBattery = isOnBattery()
            val reducedMotion = isReducedMotion()

            nativeRenderer?.let { nr ->
                nr.setPowerState(store.qualityProfile, onBattery, reducedMotion)
                nr.setLowMode(store.lowMode)
                nr.setPaused(paused)
            }

            nativeVideoPlayer?.setPaused(paused)

            val wv = webView ?: return
            if (paused) wv.onPause() else wv.onResume()
            if (pageReady) {
                PrototypeAssets.dispatchMessage(wv, JSONObject().apply {
                    put("t", "perf")
                    put("adaptiveFps", true)
                    put("onBattery", onBattery)
                    put("reducedMotion", reducedMotion)
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
            tearDownNative()
            tearDownVideo()
            tearDownPresentation()
            currentHolder = null
            super.onDestroy()
        }

        override fun onSurfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
            super.onSurfaceChanged(holder, format, width, height)
            val newSurface = holder.surface
            if (newSurface != null && newSurface !== activeNativeSurface) {
                nativeFailedForTheme = null
                nativeRecoveryAttempts = 0
            }
            if (newSurface != null && newSurface !== activeVideoSurface) {
                videoFailedForTheme = null
            }
            currentHolder = holder
            surfaceW = width.coerceAtLeast(1)
            surfaceH = height.coerceAtLeast(1)
            syncRendererBackend(holder, surfaceW, surfaceH)
        }

        override fun onSurfaceDestroyed(holder: SurfaceHolder) {
            // Detach EGL / MediaPlayer synchronously BEFORE Android invalidates the underlying ANativeWindow.
            tearDownNative()
            tearDownVideo()
            tearDownPresentation()
            if (currentHolder === holder) {
                currentHolder = null
            }
            super.onSurfaceDestroyed(holder)
        }

        override fun onVisibilityChanged(visible: Boolean) {
            engineVisible = visible
            if (visible) screenOff = false
            mainHandler.post { applyPowerAndPauseState() }
        }

        override fun onTouchEvent(event: MotionEvent) {
            super.onTouchEvent(event)
            val nr = nativeRenderer
            if (nr != null) {
                nr.dispatchTouch(event.actionMasked, event.x, event.y)
                return
            }
            val copy = MotionEvent.obtain(event)
            mainHandler.post {
                val wv = webView
                if (wv != null) wv.dispatchTouchEvent(copy)
                copy.recycle()
            }
        }

        override fun onSettingsChanged(store: ThemeStore) {
            mainHandler.post {
                val active = store.activeTheme
                if (nativeFailedForTheme != null && nativeFailedForTheme != active) {
                    nativeFailedForTheme = null
                    nativeRecoveryAttempts = 0
                }
                if (videoFailedForTheme != null && videoFailedForTheme != active) {
                    videoFailedForTheme = null
                }
                val holder = currentHolder ?: return@post
                val wantNative = shouldUseNativeGl(active)

                if (wantNative) {
                    val nr = nativeRenderer
                    if (nr != null) {
                        pushAllStateToNative(nr)
                    } else {
                        syncRendererBackend(holder, surfaceW, surfaceH)
                    }
                    return@post
                }

                val videoFile = resolveNativeVideoFile(active)
                if (videoFile != null) {
                    val vp = nativeVideoPlayer
                    if (vp != null && active == loadedThemeId && vp.file == videoFile) {
                        val speed = store.getParamsCopy(active).optDouble("speed", 1.0).toFloat()
                        vp.setSpeed(speed)
                        vp.setPaused(shouldPause())
                    } else {
                        syncRendererBackend(holder, surfaceW, surfaceH)
                    }
                    return@post
                }

                // WebView path required (custom HTML/image/archive theme or fallback)
                if (nativeRenderer != null || nativeVideoPlayer != null || webView == null) {
                    syncRendererBackend(holder, surfaceW, surfaceH)
                    return@post
                }
                val wv = webView ?: return@post
                if (active != loadedThemeId) {
                    loadActiveTheme(wv)
                } else if (pageReady) {
                    pushAllState(wv)
                }
            }
        }

        override fun onIconsChanged(store: ThemeStore) {
            mainHandler.post {
                val nr = nativeRenderer
                if (nr != null) {
                    nr.postJson(store.buildIconsMessage(surfaceW, surfaceH).toString())
                    return@post
                }
                val wv = webView ?: return@post
                if (pageReady) {
                    PrototypeAssets.dispatchMessage(wv, store.buildIconsMessage(surfaceW, surfaceH))
                }
            }
        }

        override fun onThemesRegistryChanged(store: ThemeStore) {
            onSettingsChanged(store)
        }

        private fun syncRendererBackend(holder: SurfaceHolder, width: Int, height: Int) {
            val active = store.activeTheme
            if (shouldUseNativeGl(active)) {
                val surface = holder.surface
                if (surface == null || !surface.isValid) return

                // Reuse the running EGL context if the Surface instance did not change (e.g. resize/rotation).
                val existing = nativeRenderer
                if (existing != null && activeNativeSurface === surface) {
                    existing.resize(width, height, displayDensity())
                    pushAllStateToNative(existing)
                    return
                }

                tearDownVideo()
                tearDownPresentation()
                tearDownNative()
                startNativeRenderer(surface, width, height)
            } else {
                val videoFile = resolveNativeVideoFile(active)
                if (videoFile != null) {
                    val surface = holder.surface
                    if (surface == null || !surface.isValid) return
                    val existingVp = nativeVideoPlayer
                    val speed = store.getParamsCopy(active).optDouble("speed", 1.0).toFloat()
                    if (existingVp != null && loadedThemeId == active && existingVp.file == videoFile) {
                        activeVideoSurface = surface
                        existingVp.updateSurface(surface)
                        existingVp.setSpeed(speed)
                        existingVp.setPaused(shouldPause())
                        return
                    }
                    tearDownPresentation()
                    tearDownNative()
                    tearDownVideo()
                    startNativeVideo(active, videoFile, surface, speed)
                    return
                }
                tearDownVideo()
                tearDownNative()
                rebuildPresentation(holder, width, height)
            }
        }

        private fun currentParamKeySet(themeId: String): Set<String> {
            val obj = store.getParamsCopy(themeId)
            val keys = LinkedHashSet<String>()
            val it = obj.keys()
            while (it.hasNext()) {
                keys.add(it.next())
            }
            return keys
        }

        private fun buildNativeStateMessages(includeReset: Boolean): List<String> = buildList {
            add(NativeGlProtocol.buildThemeMessage(store.activeTheme).toString())
            if (includeReset) {
                add("{\"t\":\"reset\"}")
            }
            add(NativeGlProtocol.buildLowModeMessage(store.lowMode).toString())
            add(store.buildWallpaperSettingsMessage().toString())
            add(store.buildIconsMessage(surfaceW, surfaceH).toString())
        }

        private fun pushAllStateToNative(nr: NativeGlWallpaperRenderer) {
            val active = store.activeTheme
            val themeChanged = active != loadedThemeId
            val currentKeys = currentParamKeySet(active)
            val needsReset = NativeGlProtocol.shouldSendReset(
                themeChanged = themeChanged,
                previousParamKeys = lastNativeParamKeys,
                currentParamKeys = currentKeys
            )
            loadedThemeId = active
            lastNativeParamKeys = currentKeys
            nr.setPowerState(store.qualityProfile, isOnBattery(), isReducedMotion())
            nr.setLowMode(store.lowMode)
            for (msg in buildNativeStateMessages(includeReset = needsReset)) {
                nr.postJson(msg)
            }
            nr.setPaused(shouldPause())
        }

        private fun startNativeRenderer(surface: Surface, width: Int, height: Int) {
            val startingTheme = store.activeTheme
            loadedThemeId = startingTheme
            lastNativeParamKeys = currentParamKeySet(startingTheme)
            activeNativeSurface = surface
            lateinit var renderer: NativeGlWallpaperRenderer
            renderer = NativeGlWallpaperRenderer(
                surface = surface,
                width = width,
                height = height,
                density = displayDensity(),
                initialMessages = buildNativeStateMessages(includeReset = false),
                initialPaused = shouldPause(),
                initialLowMode = store.lowMode,
                initialQualityProfile = store.qualityProfile,
                initialOnBattery = isOnBattery(),
                initialReducedMotion = isReducedMotion(),
                onError = { reason, recoverable ->
                    mainHandler.post {
                        if (nativeRenderer !== renderer) return@post
                        val holder = currentHolder
                        val holderSurface = holder?.surface
                        if (recoverable && nativeRecoveryAttempts < 1 && holderSurface != null && holderSurface.isValid) {
                            nativeRecoveryAttempts++
                            Log.w(TAG, "Native GL ES 3.0 transient loss for '$startingTheme': $reason; recreating EGL surface")
                            tearDownNative()
                            startNativeRenderer(holderSurface, surfaceW, surfaceH)
                            return@post
                        }
                        Log.e(TAG, "Native GL ES 3.0 failed for '$startingTheme': $reason; falling back to WebView")
                        nativeFailedForTheme = startingTheme
                        tearDownNative()
                        if (holder != null && holderSurface?.isValid == true) {
                            rebuildPresentation(holder, surfaceW, surfaceH)
                        }
                    }
                }
            )
            nativeRenderer = renderer
            renderer.start()
        }

        private fun startNativeVideo(themeId: String, videoFile: File, surface: Surface, speed: Float) {
            loadedThemeId = themeId
            activeVideoSurface = surface
            lateinit var player: NativeVideoWallpaperPlayer
            player = NativeVideoWallpaperPlayer(
                file = videoFile,
                initialSurface = surface,
                initialPaused = shouldPause(),
                initialSpeed = speed,
                onError = { reason ->
                    mainHandler.post {
                        if (nativeVideoPlayer !== player) return@post
                        Log.e(TAG, "Hardware MediaPlayer failed for '$themeId': $reason; falling back to WebView")
                        videoFailedForTheme = themeId
                        tearDownVideo()
                        val holder = currentHolder
                        if (holder != null && holder.surface?.isValid == true) {
                            rebuildPresentation(holder, surfaceW, surfaceH)
                        }
                    }
                }
            )
            nativeVideoPlayer = player
        }

        private fun tearDownVideo() {
            val vp = nativeVideoPlayer
            nativeVideoPlayer = null
            activeVideoSurface = null
            vp?.destroy()
        }

        private fun tearDownNative() {
            val nr = nativeRenderer
            nativeRenderer = null
            activeNativeSurface = null
            lastNativeParamKeys = emptySet()
            nr?.destroySync()
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
                try {
                    wv.stopLoading()
                    wv.onPause()
                    wv.removeJavascriptInterface("DiyarNative")
                    wv.loadUrl("about:blank")
                    wv.clearHistory()
                    wv.removeAllViews()
                } catch (_: Exception) {
                }
                wv.destroy()
            }
            webView = null
            presentation?.dismiss()
            presentation = null
            virtualDisplay?.let { vd ->
                try {
                    vd.surface = null
                } catch (_: Exception) {
                }
                vd.release()
            }
            virtualDisplay = null
        }
    }

    private class WallpaperPresentation(
        outerContext: Context,
        display: android.view.Display
    ) : Presentation(outerContext, display)
}
