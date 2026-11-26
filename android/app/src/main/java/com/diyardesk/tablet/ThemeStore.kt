package com.diyardesk.tablet

import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.CopyOnWriteArraySet
import kotlin.math.max
import kotlin.math.min

/** Normalized `[0f..1f]` tablet icon/widget box used across themes. */
data class NormalizedBox(
    val left: Float,
    val top: Float,
    val right: Float,
    val bottom: Float,
    val isWidget: Boolean = false
) {
    fun toJson(): JSONObject = JSONObject().apply {
        put("l", left.toDouble())
        put("t", top.toDouble())
        put("r", right.toDouble())
        put("b", bottom.toDouble())
        put("w", isWidget)
    }

    companion object {
        fun clamped(l: Float, t: Float, r: Float, b: Float, isWidget: Boolean = false): NormalizedBox {
            val nl = min(max(l, 0.01f), 0.94f)
            val nt = min(max(t, 0.01f), 0.94f)
            val minW = if (isWidget) 0.14f else 0.055f
            val minH = if (isWidget) 0.12f else 0.065f
            val nr = min(max(r, nl + minW), 0.99f)
            val nb = min(max(b, nt + minH), 0.99f)
            return NormalizedBox(nl, nt, nr, nb, isWidget)
        }

        fun fromJson(obj: JSONObject): NormalizedBox = clamped(
            obj.optDouble("l", 0.05).toFloat(),
            obj.optDouble("t", 0.05).toFloat(),
            obj.optDouble("r", 0.13).toFloat(),
            obj.optDouble("b", 0.15).toFloat(),
            obj.optBoolean("w", false)
        )
    }
}

/** Pure math + default tablet icon presets (unit-testable on JVM without Android deps). */
object IconLayoutMath {
    fun presetTablet5x6(): List<NormalizedBox> {
        val list = ArrayList<NormalizedBox>(14)
        // Top-left weather/clock style widget + left-side & top-right tablet icon cluster
        list += NormalizedBox.clamped(0.04f, 0.06f, 0.30f, 0.28f, isWidget = true)
        val iconCoords = listOf(
            0.04f to 0.33f, 0.13f to 0.33f, 0.22f to 0.33f,
            0.04f to 0.48f, 0.13f to 0.48f, 0.22f to 0.48f,
            0.04f to 0.63f, 0.13f to 0.63f,
            0.78f to 0.08f, 0.87f to 0.08f,
            0.78f to 0.23f, 0.87f to 0.23f,
            0.78f to 0.38f, 0.87f to 0.38f
        )
        for ((x, y) in iconCoords) {
            list += NormalizedBox.clamped(x, y, x + 0.072f, y + 0.115f, isWidget = false)
        }
        return list
    }

    fun presetGrid4x5(): List<NormalizedBox> {
        val list = ArrayList<NormalizedBox>(12)
        for (col in 0 until 2) {
            for (row in 0 until 4) {
                val x = 0.05f + col * 0.095f
                val y = 0.07f + row * 0.15f
                list += NormalizedBox.clamped(x, y, x + 0.075f, y + 0.115f, isWidget = false)
            }
        }
        for (row in 0 until 2) {
            val x = 0.85f
            val y = 0.10f + row * 0.16f
            list += NormalizedBox.clamped(x, y, x + 0.075f, y + 0.115f, isWidget = false)
        }
        return list
    }

    fun toPhysicalRects(boxes: List<NormalizedBox>, widthPx: Int, heightPx: Int): List<IntArray> {
        if (widthPx <= 0 || heightPx <= 0) return emptyList()
        return boxes.map { box ->
            intArrayOf(
                (box.left * widthPx).toInt().coerceIn(0, widthPx),
                (box.top * heightPx).toInt().coerceIn(0, heightPx),
                (box.right * widthPx).toInt().coerceIn(0, widthPx),
                (box.bottom * heightPx).toInt().coerceIn(0, heightPx)
            )
        }
    }

    fun toIconsMessage(boxes: List<NormalizedBox>, widthPx: Int, heightPx: Int): JSONObject {
        val rects = JSONArray()
        for (rect in toPhysicalRects(boxes, widthPx, heightPx)) {
            rects.put(JSONArray().apply {
                put(rect[0]); put(rect[1]); put(rect[2]); put(rect[3])
            })
        }
        return JSONObject().apply {
            put("t", "icons")
            put("vis", true)
            put("r", rects)
        }
    }

    fun hitTestBox(boxes: List<NormalizedBox>, nx: Float, ny: Float): Int {
        for (i in boxes.indices.reversed()) {
            val b = boxes[i]
            if (nx in b.left..b.right && ny in b.top..b.bottom) return i
        }
        return -1
    }

    fun hitTestResizeHandle(
        boxes: List<NormalizedBox>,
        nx: Float,
        ny: Float,
        radiusX: Float,
        radiusY: Float
    ): Int {
        for (i in boxes.indices.reversed()) {
            val b = boxes[i]
            val dx = kotlin.math.abs(nx - b.right)
            val dy = kotlin.math.abs(ny - b.bottom)
            if (dx <= radiusX && dy <= radiusY) return i
        }
        return -1
    }

    fun moveBox(box: NormalizedBox, newLeft: Float, newTop: Float): NormalizedBox {
        val bw = box.right - box.left
        val bh = box.bottom - box.top
        val nl = min(max(newLeft, 0.005f), 0.995f - bw)
        val nt = min(max(newTop, 0.005f), 0.995f - bh)
        return NormalizedBox(nl, nt, nl + bw, nt + bh, box.isWidget)
    }

    fun resizeBox(box: NormalizedBox, newRight: Float, newBottom: Float): NormalizedBox {
        val minW = if (box.isWidget) 0.10f else 0.045f
        val minH = if (box.isWidget) 0.09f else 0.055f
        val nr = min(max(newRight, box.left + minW), 0.995f)
        val nb = min(max(newBottom, box.top + minH), 0.995f)
        return NormalizedBox(box.left, box.top, nr, nb, box.isWidget)
    }

    fun createBoxCenteredAt(nx: Float, ny: Float, isWidget: Boolean): NormalizedBox {
        val w = if (isWidget) 0.22f else 0.072f
        val h = if (isWidget) 0.18f else 0.115f
        val l = min(max(nx - w / 2f, 0.005f), 0.995f - w)
        val t = min(max(ny - h / 2f, 0.005f), 0.995f - h)
        return NormalizedBox(l, t, l + w, t + h, isWidget)
    }
}

/** Persistent JSON preferences & live observer hub shared between Settings UI and WallpaperService. */
class ThemeStore(private val storageDir: File) {

    interface Listener {
        fun onSettingsChanged(store: ThemeStore) {}
        fun onIconsChanged(store: ThemeStore) {}
        fun onThemesRegistryChanged(store: ThemeStore) {}
    }

    private val lock = Any()
    private val prefsFile = File(storageDir, "prefs.json")
    private val listeners = CopyOnWriteArraySet<Listener>()

    var activeTheme: String = DEFAULT_THEME
        private set
    var lang: String = "auto"
        private set
    var virtualBarMode: String = "auto"
        private set
    var lowMode: Boolean = false
        private set
    var disableAutoPause: Boolean = false
        private set
    var qualityProfile: String = "balanced"
        private set
    var rustOpenGl: Boolean = true
        private set
    private var paramsByTheme: MutableMap<String, JSONObject> = mutableMapOf()
    private var iconBoxes: MutableList<NormalizedBox> = IconLayoutMath.presetTablet5x6().toMutableList()

    init {
        storageDir.mkdirs()
        load()
    }

    fun addListener(listener: Listener) {
        listeners.add(listener)
    }

    fun removeListener(listener: Listener) {
        listeners.remove(listener)
    }

    fun load() {
        synchronized(lock) {
            if (!prefsFile.exists()) return
            try {
                val root = JSONObject(prefsFile.readText(Charsets.UTF_8))
                activeTheme = root.optString("theme", DEFAULT_THEME).ifBlank { DEFAULT_THEME }
                lang = root.optString("lang", "auto").ifBlank { "auto" }
                virtualBarMode = root.optString("virtualBarMode", "auto").ifBlank { "auto" }
                lowMode = root.optBoolean("lowMode", false)
                disableAutoPause = root.optBoolean("disableAutoPause", false)
                val qp = root.optString("qualityProfile", "balanced")
                qualityProfile = if (qp in setOf("eco", "balanced", "high")) qp else "balanced"
                rustOpenGl = root.optBoolean("rustOpenGl", true)

                paramsByTheme = mutableMapOf()
                val paramsObj = root.optJSONObject("params")
                if (paramsObj != null) {
                    val keys = paramsObj.keys()
                    while (keys.hasNext()) {
                        val k = keys.next()
                        val v = paramsObj.optJSONObject(k)
                        if (v != null) paramsByTheme[k] = JSONObject(v.toString())
                    }
                }

                val boxesArr = root.optJSONArray("iconBoxes")
                if (boxesArr != null) {
                    val parsed = ArrayList<NormalizedBox>(boxesArr.length())
                    for (i in 0 until boxesArr.length()) {
                        boxesArr.optJSONObject(i)?.let { parsed += NormalizedBox.fromJson(it) }
                    }
                    iconBoxes = parsed
                }
            } catch (_: Exception) {
                // Corrupt file falls back cleanly to safe defaults
            }
        }
    }

    private fun saveLocked() {
        val root = JSONObject().apply {
            put("theme", activeTheme)
            put("lang", lang)
            put("virtualBarMode", virtualBarMode)
            put("lowMode", lowMode)
            put("disableAutoPause", disableAutoPause)
            put("qualityProfile", qualityProfile)
            put("rustOpenGl", rustOpenGl)
            put("params", JSONObject().apply {
                for ((k, v) in paramsByTheme) put(k, v)
            })
            put("iconBoxes", JSONArray().apply {
                for (box in iconBoxes) put(box.toJson())
            })
        }
        storageDir.mkdirs()
        prefsFile.writeText(root.toString(2), Charsets.UTF_8)
    }

    fun getIconBoxes(): List<NormalizedBox> = synchronized(lock) { iconBoxes.toList() }

    fun setIconBoxes(boxes: List<NormalizedBox>) {
        synchronized(lock) {
            iconBoxes = boxes.toMutableList()
            saveLocked()
        }
        listeners.forEach { it.onIconsChanged(this) }
    }

    fun setActiveTheme(themeId: String) {
        if (themeId.isBlank()) return
        synchronized(lock) {
            activeTheme = themeId
            saveLocked()
        }
        listeners.forEach { it.onSettingsChanged(this) }
    }

    fun setParam(themeId: String, key: String, value: Any?) {
        if (themeId.isBlank() || key.isBlank()) return
        synchronized(lock) {
            val map = paramsByTheme.getOrPut(themeId) { JSONObject() }
            if (value == null || value === JSONObject.NULL) {
                map.remove(key)
            } else {
                map.put(key, value)
            }
            saveLocked()
        }
        listeners.forEach { it.onSettingsChanged(this) }
    }

    fun resetThemeParams(themeId: String) {
        synchronized(lock) {
            paramsByTheme.remove(themeId)
            saveLocked()
        }
        listeners.forEach { it.onSettingsChanged(this) }
    }

    fun setLanguage(newLang: String) {
        synchronized(lock) {
            lang = newLang.ifBlank { "auto" }
            saveLocked()
        }
    }

    fun setVirtualBarModeValue(mode: String) {
        synchronized(lock) {
            virtualBarMode = mode.ifBlank { "auto" }
            saveLocked()
        }
        listeners.forEach { it.onSettingsChanged(this) }
    }

    fun setLowModeEnabled(enabled: Boolean) {
        synchronized(lock) {
            lowMode = enabled
            saveLocked()
        }
        listeners.forEach { it.onSettingsChanged(this) }
    }

    fun setDisableAutoPauseEnabled(enabled: Boolean) {
        synchronized(lock) {
            disableAutoPause = enabled
            saveLocked()
        }
        listeners.forEach { it.onSettingsChanged(this) }
    }

    fun setQualityProfileValue(profile: String) {
        val clean = if (profile in setOf("eco", "balanced", "high")) profile else "balanced"
        synchronized(lock) {
            qualityProfile = clean
            saveLocked()
        }
        listeners.forEach { it.onSettingsChanged(this) }
    }

    fun setRustOpenGlEnabled(enabled: Boolean) {
        synchronized(lock) {
            rustOpenGl = enabled
            saveLocked()
        }
        listeners.forEach { it.onSettingsChanged(this) }
    }

    fun notifyThemesRegistryChanged() {
        listeners.forEach { it.onThemesRegistryChanged(this) }
    }

    fun getParamsCopy(themeId: String): JSONObject = synchronized(lock) {
        paramsByTheme[themeId]?.let { JSONObject(it.toString()) } ?: JSONObject()
    }

    fun getAllParamsCopy(): JSONObject = synchronized(lock) {
        JSONObject().apply {
            for ((k, v) in paramsByTheme) put(k, JSONObject(v.toString()))
        }
    }

    fun buildWallpaperSettingsMessage(): JSONObject = synchronized(lock) {
        val merged = getParamsCopy(activeTheme)
        merged.put("virtualBarMode", virtualBarMode)
        merged.put("lowMode", lowMode)
        JSONObject().apply {
            put("t", "settings")
            put("theme", activeTheme)
            put("params", merged)
        }
    }

    fun buildIconsMessage(widthPx: Int, heightPx: Int): JSONObject = synchronized(lock) {
        IconLayoutMath.toIconsMessage(iconBoxes, widthPx, heightPx)
    }

    fun buildInitMessage(widthPx: Int, heightPx: Int, systemLang: String): JSONObject = synchronized(lock) {
        val effLang = resolveEffectiveLang(lang, systemLang)
        JSONObject().apply {
            put("t", "init")
            put("theme", activeTheme)
            put("params", getAllParamsCopy())
            put("screen", JSONObject().apply {
                put("w", widthPx)
                put("h", heightPx)
            })
            put("autostart", false)
            put("autostartWarn", false)
            put("virtualBarMode", virtualBarMode)
            put("lowMode", lowMode)
            put("disableAutoPause", disableAutoPause)
            put("qualityProfile", qualityProfile)
            val glAvailable = NativeGlBridge.isLibraryLoaded
            val nativeActive = NativeGlProtocol.shouldUseNativeWallpaper(activeTheme, rustOpenGl, glAvailable, null)
            put("rustOpenGl", rustOpenGl)
            put("rustGlAvailable", glAvailable)
            put("nativeActive", nativeActive)
            put("lang", lang)
            put("langEff", effLang)
            put("langSys", systemLang)
        }
    }

    companion object {
        const val DEFAULT_THEME = "papercut"
        private val SUPPORTED_LANGS = setOf("en", "zh-CN", "zh-TW", "tr", "de", "fr", "ja", "ko")

        fun resolveEffectiveLang(setting: String, systemLang: String): String {
            if (setting in SUPPORTED_LANGS) return setting
            val two = systemLang.take(2).lowercase()
            val zhHant = systemLang.startsWith("zh", ignoreCase = true) &&
                (systemLang.contains("Hant", ignoreCase = true) || listOf("-tw", "-hk", "-mo").any { systemLang.endsWith(it, true) })
            return when {
                zhHant -> "zh-TW"
                two == "zh" -> "zh-CN"
                two in SUPPORTED_LANGS -> two
                else -> "en"
            }
        }
    }
}
