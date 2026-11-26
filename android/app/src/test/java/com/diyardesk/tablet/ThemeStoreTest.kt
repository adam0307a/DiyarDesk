package com.diyardesk.tablet

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class ThemeStoreTest {

    @get:Rule
    val tmp = TemporaryFolder()

    @Test
    fun persistsThemeParamsLanguageAndVirtualBar() {
        val dir = tmp.newFolder("store")
        val s1 = ThemeStore(dir)
        s1.setActiveTheme("istanbul")
        s1.setParam("istanbul", "lanternWarmth", 0.85)
        s1.setLanguage("en")
        s1.setVirtualBarModeValue("on")

        val s2 = ThemeStore(dir)
        assertEquals("istanbul", s2.activeTheme)
        assertEquals("en", s2.lang)
        assertEquals("on", s2.virtualBarMode)
        assertEquals(0.85, s2.getParamsCopy("istanbul").getDouble("lanternWarmth"), 0.001)

        s2.resetThemeParams("istanbul")
        val s3 = ThemeStore(dir)
        assertTrue(!s3.getParamsCopy("istanbul").has("lanternWarmth"))
    }

    @Test
    fun buildsIconsPhysicalRectsAndInitPayload() {
        val dir = tmp.newFolder("store2")
        val s = ThemeStore(dir)
        val customBoxes = listOf(
            NormalizedBox.clamped(0.10f, 0.20f, 0.30f, 0.40f, isWidget = true)
        )
        s.setIconBoxes(customBoxes)

        val msg = s.buildIconsMessage(2560, 1600)
        assertEquals("icons", msg.getString("t"))
        assertTrue(msg.getBoolean("vis"))
        val rect = msg.getJSONArray("r").getJSONArray(0)
        assertEquals(256, rect.getInt(0))
        assertEquals(320, rect.getInt(1))
        assertEquals(768, rect.getInt(2))
        assertEquals(640, rect.getInt(3))

        val init = s.buildInitMessage(2560, 1600, "en")
        assertEquals("init", init.getString("t"))
        assertEquals("en", init.getString("langEff"))
        assertTrue(init.getBoolean("rustOpenGl"))
        assertTrue(init.has("rustGlAvailable"))
        assertTrue(init.has("nativeActive"))
    }

    @Test
    fun persistsRustOpenGlPreferenceAcrossRestarts() {
        val dir = tmp.newFolder("store_gl")
        val s1 = ThemeStore(dir)
        assertTrue(s1.rustOpenGl)

        s1.setRustOpenGlEnabled(false)
        val s2 = ThemeStore(dir)
        assertFalse(s2.rustOpenGl)

        val initOff = s2.buildInitMessage(1920, 1200, "tr")
        assertFalse(initOff.getBoolean("rustOpenGl"))

        s2.setRustOpenGlEnabled(true)
        val s3 = ThemeStore(dir)
        assertTrue(s3.rustOpenGl)
    }

    @Test
    fun interactiveIconMathMovesResizesAndHitTests() {
        val box = NormalizedBox.clamped(0.20f, 0.30f, 0.30f, 0.45f, isWidget = false)
        val boxes = listOf(box)

        assertEquals(0, IconLayoutMath.hitTestBox(boxes, 0.25f, 0.35f))
        assertEquals(-1, IconLayoutMath.hitTestBox(boxes, 0.05f, 0.05f))

        assertEquals(0, IconLayoutMath.hitTestResizeHandle(boxes, 0.31f, 0.44f, 0.03f, 0.03f))
        assertEquals(-1, IconLayoutMath.hitTestResizeHandle(boxes, 0.20f, 0.30f, 0.03f, 0.03f))

        val moved = IconLayoutMath.moveBox(box, 0.50f, 0.50f)
        assertEquals(0.50f, moved.left, 0.001f)
        assertEquals(0.60f, moved.right, 0.001f)

        val resized = IconLayoutMath.resizeBox(box, 0.45f, 0.70f)
        assertEquals(0.20f, resized.left, 0.001f)
        assertEquals(0.45f, resized.right, 0.001f)
        assertEquals(0.70f, resized.bottom, 0.001f)

        val centered = IconLayoutMath.createBoxCenteredAt(0.50f, 0.50f, isWidget = true)
        assertEquals(0.50f, (centered.left + centered.right) / 2f, 0.001f)
        assertTrue(centered.isWidget)
    }

    @Test
    fun resolvesTurkishAndDefaultThemeCorrectly() {
        val dir = tmp.newFolder("store3")
        val s = ThemeStore(dir)
        assertEquals("papercut", s.activeTheme)
        assertEquals("tr", ThemeStore.resolveEffectiveLang("auto", "tr"))
        assertEquals("tr", ThemeStore.resolveEffectiveLang("auto", "tr-TR"))
        assertEquals("tr", ThemeStore.resolveEffectiveLang("tr", "en"))
        assertEquals("de", ThemeStore.resolveEffectiveLang("auto", "de-DE"))
        assertEquals("ja", ThemeStore.resolveEffectiveLang("auto", "ja-JP"))
        assertEquals("zh-TW", ThemeStore.resolveEffectiveLang("auto", "zh-TW"))
    }
}
