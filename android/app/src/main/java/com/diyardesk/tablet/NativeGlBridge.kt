package com.diyardesk.tablet

import android.util.Log
import android.view.MotionEvent
import org.json.JSONObject

/**
 * JNI bridge to the native Rust + OpenGL ES 3.0 wallpaper engine (`libdiyardesk_gl.so`).
 *
 * All `native*` calls (except [nativeIsNativeTheme], [nativeThemesCsv], and [nativeLastError])
 * must be invoked from the thread that owns the current EGL / OpenGL ES 3.0 context.
 */
object NativeGlBridge {
    private const val TAG = "DiyarDeskGL"

    /** Canonical built-in native Rust GL theme IDs (mirrors `NATIVE_GL_THEMES` in `native/diyardesk-gl/src/lib.rs`). */
    val FALLBACK_NATIVE_GL_THEMES: Set<String> = setOf(
        "material-impulse-gl",
        "material-impulse",
        "liquidglass",
        "thinking-orbs",
        "solar-system",
        "studio-glass",
        "beach",
        "meadow",
        "desert",
        "rosebrook",
        "glade",
        "zen-archipelago",
        "aurora",
        "polarnight",
        "quietsnow",
        "winter",
        "neumorph",
        "claymorph",
        "neobrutalism",
        "voxelcraft",
        "papercut",
        "istanbul",
        "software-world",
        "rift",
        "strike",
        "pocket",
        "roach",
        "ai-mascots",
        "cat-shore",
        "persephone",
        "eye-within-words",
        "cosmic-eye",
    )

    /** Built-in themes that exist only in the native Rust + OpenGL engine (no full HTML/Canvas runtime). */
    val GL_ONLY_THEMES: Set<String> = setOf(
        "material-impulse-gl",
        "ai-mascots",
        "cat-shore",
        "persephone",
        "eye-within-words",
        "cosmic-eye",
    )

    @Volatile
    var isLibraryLoaded: Boolean = false
        private set

    @Volatile
    var loadError: String? = null
        private set

    init {
        try {
            System.loadLibrary("diyardesk_gl")
            isLibraryLoaded = true
            loadError = null
        } catch (t: UnsatisfiedLinkError) {
            isLibraryLoaded = false
            loadError = t.message ?: "libdiyardesk_gl.so not bundled"
            try {
                Log.w(TAG, "Native OpenGL ES 3.0 library unavailable; using WebView fallback: ${loadError}")
            } catch (_: Throwable) {
                // Safe under JVM unit tests where android.util.Log is stubbed.
            }
        }
    }

    /** Returns true if [themeId] is a built-in native Rust GL theme. */
    fun isNativeTheme(themeId: String): Boolean {
        val clean = themeId.trim()
        if (clean.isEmpty()) return false
        if (isLibraryLoaded) {
            return try {
                nativeIsNativeTheme(clean)
            } catch (_: Throwable) {
                clean.lowercase() in FALLBACK_NATIVE_GL_THEMES
            }
        }
        return clean.lowercase() in FALLBACK_NATIVE_GL_THEMES
    }

    /** Returns true if the native `.so` is loaded AND supports [themeId]. */
    fun isAvailableForTheme(themeId: String): Boolean {
        if (!isLibraryLoaded) return false
        return isNativeTheme(themeId)
    }

    /** Returns true if [themeId] is a GL-only native theme with no HTML/WebView wallpaper fallback. */
    fun isGlOnlyTheme(themeId: String): Boolean {
        val clean = themeId.trim().lowercase()
        return clean.isNotEmpty() && clean in GL_ONLY_THEMES
    }

    /** Returns the list of native themes reported by Rust (or fallback list if running on JVM). */
    fun supportedThemes(): Set<String> {
        if (!isLibraryLoaded) return FALLBACK_NATIVE_GL_THEMES
        return try {
            nativeThemesCsv()
                .split(',')
                .map { it.trim().lowercase() }
                .filter { it.isNotEmpty() }
                .toSet()
                .ifEmpty { FALLBACK_NATIVE_GL_THEMES }
        } catch (_: Throwable) {
            FALLBACK_NATIVE_GL_THEMES
        }
    }

    @JvmStatic
    external fun nativeIsNativeTheme(themeId: String): Boolean

    @JvmStatic
    external fun nativeThemesCsv(): String

    @JvmStatic
    external fun nativeCreate(width: Int, height: Int, dpiScale: Float): Long

    @JvmStatic
    external fun nativeDestroy(handle: Long)

    @JvmStatic
    external fun nativeResize(handle: Long, width: Int, height: Int, dpiScale: Float)

    @JvmStatic
    external fun nativeSetPaused(handle: Long, paused: Boolean)

    @JvmStatic
    external fun nativeSetLowMode(handle: Long, low: Boolean)

    @JvmStatic
    external fun nativePostJson(handle: Long, json: String)

    @JvmStatic
    external fun nativePointer(handle: Long, kind: Int, cssX: Float, cssY: Float): Boolean

    @JvmStatic
    external fun nativeRender(handle: Long, dt: Float, targetFbo: Int): Boolean

    @JvmStatic
    external fun nativePollEvent(handle: Long): String?

    @JvmStatic
    external fun nativeRenderPngToPath(handle: Long, targetFbo: Int, outPath: String): Boolean

    @JvmStatic
    external fun nativeLastError(): String?
}

/** Pure-JVM helpers for native GL pacing, pointer mapping, and JSON protocol formatting. */
object NativeGlProtocol {
    const val POINTER_MOVE = 0
    const val POINTER_DOWN = 1
    const val POINTER_UP = 2
    const val POINTER_LEAVE = 3

    fun targetFrameIntervalMs(
        qualityProfile: String,
        lowMode: Boolean,
        onBattery: Boolean,
        reducedMotion: Boolean
    ): Long {
        if (reducedMotion || lowMode || qualityProfile == "eco") return 62L // ~16 fps
        if (qualityProfile == "high" && !onBattery) return 16L // ~60 fps
        return 33L // ~30 fps default (balanced or high-on-battery)
    }

    fun mapMotionActionToPointerKind(actionMasked: Int): Int = when (actionMasked) {
        MotionEvent.ACTION_MOVE -> POINTER_MOVE
        MotionEvent.ACTION_DOWN -> POINTER_DOWN
        MotionEvent.ACTION_UP -> POINTER_UP
        MotionEvent.ACTION_CANCEL -> POINTER_LEAVE
        else -> -1
    }

    fun buildThemeMessage(themeId: String): JSONObject = JSONObject().apply {
        put("t", "theme")
        put("id", themeId)
    }

    fun buildLowModeMessage(lowMode: Boolean): JSONObject = JSONObject().apply {
        put("t", "lowMode")
        put("v", lowMode)
    }

    fun buildPauseMessage(paused: Boolean): JSONObject = JSONObject().apply {
        put("t", "pause")
        put("v", paused)
    }

    /**
     * Returns true when the native engine must receive `{"t":"reset"}` before applying params:
     * either the theme changed, or a previously customized parameter key was removed/reset.
     * Avoids spurious resets on resize, lowMode, or qualityProfile updates when using defaults.
     */
    fun shouldSendReset(
        themeChanged: Boolean,
        previousParamKeys: Set<String>,
        currentParamKeys: Set<String>
    ): Boolean {
        if (themeChanged) return true
        return previousParamKeys.any { it !in currentParamKeys }
    }

    /**
     * Decides whether the wallpaper surface should run the native Rust + OpenGL ES 3.0 backend
     * (zero WebView) instead of WebView Presentation.
     */
    fun shouldUseNativeWallpaper(
        themeId: String,
        rustOpenGl: Boolean,
        libLoaded: Boolean,
        failedTheme: String?
    ): Boolean {
        if (!libLoaded) return false
        if (failedTheme != null && failedTheme == themeId) return false
        if (!NativeGlBridge.isNativeTheme(themeId)) return false
        return rustOpenGl || NativeGlBridge.isGlOnlyTheme(themeId)
    }

    /**
     * Decides whether [SettingsActivity] should mount the pure native Android UI (zero WebView,
     * static preview image) instead of loading `settings.html` in a WebView.
     */
    fun shouldUseNativeSettings(rustOpenGl: Boolean, libLoaded: Boolean): Boolean {
        return rustOpenGl && libLoaded
    }
}
