package com.diyardesk.tablet

import android.view.MotionEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class NativeGlBridgeTest {

    @Test
    fun recognizesAllBuiltInNativeThemesIncludingNewOnes() {
        val expected = listOf(
            "papercut",
            "meadow",
            "beach",
            "desert",
            "aurora",
            "polarnight",
            "istanbul",
            "liquidglass",
            "studio-glass",
            "voxelcraft",
            "cat-shore",
            "persephone",
            "eye-within-words",
            "cosmic-eye"
        )
        for (id in expected) {
            assertTrue("Expected native GL theme: $id", NativeGlBridge.isNativeTheme(id))
            assertTrue("Expected case-insensitive match: $id", NativeGlBridge.isNativeTheme(id.uppercase()))
        }
        assertFalse(NativeGlBridge.isNativeTheme(""))
        assertFalse(NativeGlBridge.isNativeTheme("custom-my-html-1234"))
        assertFalse(NativeGlBridge.isNativeTheme("custom-video-abcd"))
    }

    @Test
    fun fallbackThemeListMatchesRustNativeGlThemes() {
        val candidates = listOf(
            File("../../native/diyardesk-gl/src/lib.rs"),
            File("../native/diyardesk-gl/src/lib.rs"),
            File("native/diyardesk-gl/src/lib.rs")
        )
        val rustLib = candidates.firstOrNull { it.isFile } ?: return
        val src = rustLib.readText(Charsets.UTF_8)
        val block = Regex("""pub const NATIVE_GL_THEMES:\s*&\[&str\]\s*=\s*&\[(.*?)];""", RegexOption.DOT_MATCHES_ALL)
            .find(src)
            ?.groupValues
            ?.get(1)
            ?: throw AssertionError("NATIVE_GL_THEMES not found in ${rustLib.path}")
        val rustThemes = Regex(""""([^"]+)"""")
            .findAll(block)
            .map { it.groupValues[1].trim().lowercase() }
            .toSet()

        assertEquals(
            "Kotlin FALLBACK_NATIVE_GL_THEMES drifted from Rust NATIVE_GL_THEMES",
            rustThemes,
            NativeGlBridge.FALLBACK_NATIVE_GL_THEMES
        )
    }

    @Test
    fun framePacingRespectsQualityBatteryAndReducedMotion() {
        assertEquals(62L, NativeGlProtocol.targetFrameIntervalMs("balanced", lowMode = true, onBattery = false, reducedMotion = false))
        assertEquals(62L, NativeGlProtocol.targetFrameIntervalMs("high", lowMode = false, onBattery = false, reducedMotion = true))
        assertEquals(62L, NativeGlProtocol.targetFrameIntervalMs("eco", lowMode = false, onBattery = false, reducedMotion = false))
        assertEquals(33L, NativeGlProtocol.targetFrameIntervalMs("balanced", lowMode = false, onBattery = false, reducedMotion = false))
        assertEquals(33L, NativeGlProtocol.targetFrameIntervalMs("high", lowMode = false, onBattery = true, reducedMotion = false))
        assertEquals(16L, NativeGlProtocol.targetFrameIntervalMs("high", lowMode = false, onBattery = false, reducedMotion = false))
    }

    @Test
    fun mapsMotionActionsAndFormatsJsonMessages() {
        assertEquals(NativeGlProtocol.POINTER_MOVE, NativeGlProtocol.mapMotionActionToPointerKind(MotionEvent.ACTION_MOVE))
        assertEquals(NativeGlProtocol.POINTER_DOWN, NativeGlProtocol.mapMotionActionToPointerKind(MotionEvent.ACTION_DOWN))
        assertEquals(NativeGlProtocol.POINTER_UP, NativeGlProtocol.mapMotionActionToPointerKind(MotionEvent.ACTION_UP))
        assertEquals(NativeGlProtocol.POINTER_LEAVE, NativeGlProtocol.mapMotionActionToPointerKind(MotionEvent.ACTION_CANCEL))
        assertEquals(-1, NativeGlProtocol.mapMotionActionToPointerKind(999))

        val themeMsg = NativeGlProtocol.buildThemeMessage("cosmic-eye")
        assertEquals("theme", themeMsg.getString("t"))
        assertEquals("cosmic-eye", themeMsg.getString("id"))

        val lowMsg = NativeGlProtocol.buildLowModeMessage(true)
        assertEquals("lowMode", lowMsg.getString("t"))
        assertTrue(lowMsg.getBoolean("v"))

        val pauseMsg = NativeGlProtocol.buildPauseMessage(false)
        assertEquals("pause", pauseMsg.getString("t"))
        assertFalse(pauseMsg.getBoolean("v"))
    }

    @Test
    fun resetDecisionAvoidsSpuriousResetsAndDetectsKeyRemoval() {
        // Default params on same theme (resize, lowMode, qualityProfile) must NOT send reset
        assertFalse(NativeGlProtocol.shouldSendReset(false, emptySet(), emptySet()))
        // Adding or updating a param on same theme must NOT send reset
        assertFalse(NativeGlProtocol.shouldSendReset(false, setOf("speed"), setOf("speed", "scale")))
        // Theme switch MUST send reset
        assertTrue(NativeGlProtocol.shouldSendReset(true, emptySet(), emptySet()))
        // Full reset (all keys cleared) MUST send reset
        assertTrue(NativeGlProtocol.shouldSendReset(false, setOf("speed", "scale"), emptySet()))
        // Removing a single key while another remains MUST send reset
        assertTrue(NativeGlProtocol.shouldSendReset(false, setOf("speed", "scale"), setOf("scale")))
    }

    @Test
    fun nativeSettingsAndWallpaperRoutingShutsDownWebViewInOpenGlMode() {
        // GL-only themes
        assertTrue(NativeGlBridge.isGlOnlyTheme("cosmic-eye"))
        assertTrue(NativeGlBridge.isGlOnlyTheme("eye-within-words"))
        assertTrue(NativeGlBridge.isGlOnlyTheme("material-impulse-gl"))
        assertFalse(NativeGlBridge.isGlOnlyTheme("papercut"))
        assertFalse(NativeGlBridge.isGlOnlyTheme("custom-video-1"))

        // Settings UI must use zero-WebView native controller iff OpenGL is enabled and library loaded
        assertTrue(NativeGlProtocol.shouldUseNativeSettings(rustOpenGl = true, libLoaded = true))
        assertFalse(NativeGlProtocol.shouldUseNativeSettings(rustOpenGl = false, libLoaded = true))
        assertFalse(NativeGlProtocol.shouldUseNativeSettings(rustOpenGl = true, libLoaded = false))

        // Wallpaper backend routing
        assertTrue(NativeGlProtocol.shouldUseNativeWallpaper("papercut", rustOpenGl = true, libLoaded = true, failedTheme = null))
        assertFalse(NativeGlProtocol.shouldUseNativeWallpaper("papercut", rustOpenGl = false, libLoaded = true, failedTheme = null))
        // GL-only theme still uses native GL when library is loaded even if global switch is off
        assertTrue(NativeGlProtocol.shouldUseNativeWallpaper("cosmic-eye", rustOpenGl = false, libLoaded = true, failedTheme = null))
        // Custom themes always fall back to WebView
        assertFalse(NativeGlProtocol.shouldUseNativeWallpaper("custom-html-1", rustOpenGl = true, libLoaded = true, failedTheme = null))
        // Failed theme falls back cleanly
        assertFalse(NativeGlProtocol.shouldUseNativeWallpaper("papercut", rustOpenGl = true, libLoaded = true, failedTheme = "papercut"))
        // Missing native library always falls back
        assertFalse(NativeGlProtocol.shouldUseNativeWallpaper("cosmic-eye", rustOpenGl = true, libLoaded = false, failedTheme = null))
    }
}
