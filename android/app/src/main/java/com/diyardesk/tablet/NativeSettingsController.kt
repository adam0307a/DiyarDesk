package com.diyardesk.tablet

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Debug
import android.os.Handler
import android.os.Looper
import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.SeekBar
import android.widget.Spinner
import android.widget.TextView
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.widget.SwitchCompat
import org.json.JSONArray
import org.json.JSONObject
import java.io.FileInputStream
import java.util.Locale
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.roundToInt

/**
 * Pure native Android Settings UI mounted when Rust + OpenGL ES 3.0 mode is active.
 *
 * Uses zero [android.webkit.WebView] and zero HTML/iframe previews: displays static theme
 * thumbnail bitmaps (or a clean dark native card when no thumbnail image exists), theme cards,
 * dynamic theme parameter controls from `prototype/themes.json`, and global toggles.
 */
class NativeSettingsController(
    private val context: Context,
    private val store: ThemeStore,
    private val importer: CustomThemeImporter,
    private val onToggleRustOpenGl: (Boolean) -> Unit,
    private val onImportTheme: () -> Unit,
    private val onRemoveTheme: (String) -> Unit,
    private val onExportPreset: () -> Unit = {},
    private val onImportPreset: () -> Unit = {},
    private val isLiveWallpaperActive: () -> Boolean = { true },
    private val onOpenWallpaperPicker: () -> Unit = {},
    private val onOpenIconLayout: () -> Unit = {}
) {

    val rootView: ScrollView = ScrollView(context).apply {
        isFillViewport = true
        setBackgroundColor(Color.parseColor("#0E1D19"))
    }

    private val body = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        val pad = dp(16)
        setPadding(pad, pad, pad, dp(28))
    }

    private val i18nCache = HashMap<String, JSONObject>()
    private val thumbCache = LinkedHashMap<String, Bitmap?>()
    private val urlPattern = Regex("""url\(\s*['"]?([^)'"]+)['"]?\s*\)""")
    private val mainHandler = Handler(Looper.getMainLooper())
    private val memExecutor = Executors.newSingleThreadScheduledExecutor()
    private var memFuture: ScheduledFuture<*>? = null
    @Volatile private var pollingActive = false
    @Volatile private var destroyed = false
    private var liveRamBadgeView: TextView? = null
    private var glRamBadgeView: TextView? = null
    private var engineSubtitleView: TextView? = null
    private var previewBadgeView: TextView? = null
    private var lastPssMb: Int = 0
    private var lastGlMb: Int = 0

    private var themesList: List<JSONObject> = emptyList()
    private var effLang: String = "en"
    private var activeCategory: String = "all"

    init {
        rootView.addView(
            body,
            ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
        )
        refresh()
        setPollingActive(true)
    }

    fun refresh() {
        effLang = ThemeStore.resolveEffectiveLang(store.lang, DiyarAppState.systemLangCode())
        themesList = loadMergedThemes()
        liveRamBadgeView = null
        glRamBadgeView = null
        engineSubtitleView = null
        previewBadgeView = null
        body.removeAllViews()

        val activeThemeObj = themesList.firstOrNull { it.optString("id") == store.activeTheme }
            ?: themesList.firstOrNull()

        body.addView(buildEngineBannerCard())
        body.addView(spacer(12))
        if (!isLiveWallpaperActive()) {
            body.addView(buildSetupGuideCard())
            body.addView(spacer(12))
        }
        if (activeThemeObj != null) {
            body.addView(buildStaticPreviewCard(activeThemeObj))
            body.addView(spacer(14))
        }
        body.addView(buildThemePickerCard())
        body.addView(spacer(14))
        if (activeThemeObj != null) {
            body.addView(buildParametersCard(activeThemeObj))
            body.addView(spacer(14))
        }
        body.addView(buildGlobalSettingsCard())
        updateLiveRamViews()
    }

    fun setPollingActive(active: Boolean) {
        if (destroyed || pollingActive == active) return
        pollingActive = active
        memFuture?.cancel(false)
        memFuture = null
        if (active && !memExecutor.isShutdown) {
            try {
                memFuture = memExecutor.scheduleWithFixedDelay({
                    if (!pollingActive || destroyed) return@scheduleWithFixedDelay
                    val (pssMb, glMb) = readProcessMemoryMb()
                    if (pssMb <= 0 || !pollingActive || destroyed) return@scheduleWithFixedDelay
                    mainHandler.post {
                        if (destroyed) return@post
                        lastPssMb = pssMb
                        lastGlMb = glMb
                        updateLiveRamViews()
                    }
                }, 0L, 2500L, TimeUnit.MILLISECONDS)
            } catch (_: Exception) {
            }
        }
    }

    private fun readProcessMemoryMb(): Pair<Int, Int> {
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

    private fun updateLiveRamViews() {
        if (lastPssMb <= 0) return
        liveRamBadgeView?.text = ui("ramLiveBadge", "App Total RAM: {mb} MB").replace("{mb}", lastPssMb.toString())
        glRamBadgeView?.let { gv ->
            if (lastGlMb > 0) {
                gv.text = ui("ramOpenGlBadge", "OpenGL (GPU): {mb} MB").replace("{mb}", lastGlMb.toString())
                gv.visibility = View.VISIBLE
            } else {
                gv.visibility = View.GONE
            }
        }
        val fast = ui("rustOpenGlFast", "Fast & light")
        engineSubtitleView?.text = "$fast · WebView OFF"
        val activeNative = NativeGlBridge.isNativeTheme(store.activeTheme)
        if (activeNative) {
            previewBadgeView?.text = "Rust OpenGL ES 3.0 · ${lastPssMb} MB"
        }
    }

    fun destroy() {
        destroyed = true
        setPollingActive(false)
        mainHandler.removeCallbacksAndMessages(null)
        try {
            memExecutor.shutdownNow()
        } catch (_: Exception) {
        }
        liveRamBadgeView = null
        glRamBadgeView = null
        engineSubtitleView = null
        previewBadgeView = null
        body.removeAllViews()
        thumbCache.clear()
        i18nCache.clear()
    }

    // ------------------------------------------------------------------ UI Cards

    private fun buildEngineBannerCard(): View {
        val card = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            background = roundedBg("#15342C", 14, "#297D67", 1)
            setPadding(dp(16), dp(14), dp(16), dp(14))
        }

        val textCol = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply {
                marginEnd = dp(12)
            }
        }

        val title = TextView(context).apply {
            text = "${ui("rustOpenGl", "Rust OpenGL")} ES 3.0"
            setTextColor(Color.WHITE)
            textSize = 16f
            setTypeface(typeface, Typeface.BOLD)
        }
        val pillsRow = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(6), 0, dp(2))
        }
        val ramPill = TextView(context).apply {
            val valTxt = if (lastPssMb > 0) lastPssMb.toString() else "—"
            text = ui("ramLiveBadge", "App Total RAM: {mb} MB").replace("{mb}", valTxt)
            setTextColor(Color.parseColor("#BAE6FD"))
            textSize = 11f
            setTypeface(typeface, Typeface.BOLD)
            background = roundedBg("#0E4A5F", 999, "#38BDF8", 1)
            setPadding(dp(9), dp(3), dp(9), dp(3))
        }
        val glPill = TextView(context).apply {
            val valTxt = if (lastGlMb > 0) lastGlMb.toString() else "0"
            text = ui("ramOpenGlBadge", "OpenGL (GPU): {mb} MB").replace("{mb}", valTxt)
            setTextColor(Color.parseColor("#A7F3D0"))
            textSize = 11f
            setTypeface(typeface, Typeface.BOLD)
            background = roundedBg("#124A3A", 999, "#34D399", 1)
            setPadding(dp(9), dp(3), dp(9), dp(3))
            visibility = if (lastGlMb > 0) View.VISIBLE else View.GONE
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply { marginStart = dp(8) }
        }
        liveRamBadgeView = ramPill
        glRamBadgeView = glPill
        pillsRow.addView(ramPill)
        pillsRow.addView(glPill)

        val subtitle = TextView(context).apply {
            val fast = ui("rustOpenGlFast", "Fast & light")
            text = "$fast · WebView OFF"
            setTextColor(Color.parseColor("#9AD7C5"))
            textSize = 12f
            setPadding(0, dp(4), 0, 0)
        }
        engineSubtitleView = subtitle
        textCol.addView(title)
        textCol.addView(pillsRow)
        textCol.addView(subtitle)
        card.addView(textCol)

        val toggle = SwitchCompat(context).apply {
            isChecked = store.rustOpenGl
            setOnCheckedChangeListener { _, isChecked ->
                if (isChecked != store.rustOpenGl) {
                    onToggleRustOpenGl(isChecked)
                }
            }
        }
        card.addView(toggle)
        return card
    }

    private fun buildStaticPreviewCard(themeObj: JSONObject): View {
        val id = themeObj.optString("id")
        val name = themeName(id, themeObj.optString("name", id))
        val desc = themeDesc(id, themeObj.optString("desc", ""))
        val isNative = NativeGlBridge.isNativeTheme(id)
        val isNativeVideo = !isNative && importer.resolveCustomVideoFile(id) != null
        val rawTier = themeObj.optString("memoryTier", "")
        val memTier = if (rawTier.isNotBlank()) rawTier else if (isNativeVideo) "light" else ""
        val isCustom = id.startsWith("custom-")

        val wrapper = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = roundedBg("#132A24", 16, "#21463D", 1)
            setPadding(dp(14), dp(14), dp(14), dp(14))
        }

        // Static image frame (zero WebView / zero iframe)
        val frame = FrameLayout(context).apply {
            background = roundedBg("#0A1714", 12, "#245447", 1)
            clipToOutline = true
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(210))
        }

        val bmp = loadThemeThumbnail(themeObj)
        if (bmp != null) {
            val img = ImageView(context).apply {
                setImageBitmap(bmp)
                scaleType = ImageView.ScaleType.CENTER_CROP
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
            }
            frame.addView(img)
        }

        // Dark readability scrim at bottom
        val scrim = View(context).apply {
            background = GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(
                    Color.argb(30, 8, 20, 17),
                    Color.argb(120, 8, 20, 17),
                    Color.argb(230, 8, 20, 17)
                )
            )
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        frame.addView(scrim)

        makeTierPill(memTier, compact = false)?.let { tierBadge ->
            tierBadge.layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.TOP or Gravity.START
            ).apply {
                setMargins(dp(12), dp(12), dp(12), dp(12))
            }
            frame.addView(tierBadge)
        }

        // Engine badge pill (top-right)
        val badge = TextView(context).apply {
            text = when {
                isNative -> if (lastPssMb > 0) "Rust OpenGL ES 3.0 · ${lastPssMb} MB" else "Rust OpenGL ES 3.0 · Static Preview"
                isNativeVideo -> ui("nativeVideoBadge", "Hardware Video · MediaPlayer (0 WebView)")
                else -> "Custom Theme"
            }
            setTextColor(Color.parseColor("#E6FFF6"))
            textSize = 11f
            setTypeface(typeface, Typeface.BOLD)
            background = roundedBg(if (isNative || isNativeVideo) "#1E6D58" else "#3C4C66", 999)
            setPadding(dp(10), dp(4), dp(10), dp(4))
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.TOP or Gravity.END
            ).apply {
                setMargins(dp(12), dp(12), dp(12), dp(12))
            }
        }
        previewBadgeView = badge
        frame.addView(badge)

        // Title + description overlay (bottom-left)
        val metaCol = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM
            )
            setPadding(dp(14), dp(12), dp(14), dp(12))
        }
        val titleTv = TextView(context).apply {
            text = name
            setTextColor(Color.WHITE)
            textSize = 18f
            setTypeface(typeface, Typeface.BOLD)
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
        }
        metaCol.addView(titleTv)
        if (desc.isNotBlank()) {
            val descTv = TextView(context).apply {
                text = desc
                setTextColor(Color.parseColor("#CDE5DD"))
                textSize = 12.5f
                maxLines = 2
                ellipsize = TextUtils.TruncateAt.END
                setPadding(0, dp(3), 0, 0)
            }
            metaCol.addView(descTv)
        }
        frame.addView(metaCol)
        wrapper.addView(frame)

        // Action row (Reset, Import, Export/Import preset, optional Remove) inside HorizontalScrollView
        val actionsScroll = HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            setPadding(0, dp(12), 0, 0)
        }
        val actions = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }

        val btnReset = styledButton(ui("resetAll", "Reset all"), "#22453B") {
            store.resetThemeParams(id)
            refresh()
        }
        actions.addView(btnReset)

        val btnImport = styledButton("+ ${ui("importTheme", "Import theme…")}", "#1F6F5B") {
            onImportTheme()
        }.apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply { marginStart = dp(10) }
        }
        actions.addView(btnImport)

        val btnExportPreset = styledButton(ui("exportPreset", "Export preset"), "#22453B") {
            onExportPreset()
        }.apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply { marginStart = dp(10) }
        }
        actions.addView(btnExportPreset)

        val btnImportPreset = styledButton(ui("importPreset", "Import preset"), "#22453B") {
            onImportPreset()
        }.apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply { marginStart = dp(10) }
        }
        actions.addView(btnImportPreset)

        if (isCustom) {
            val btnRemove = styledButton(ui("removeTheme", "Remove"), "#7A2E34") {
                val title = ui("removeThemeConfirmTitle", "Remove imported theme?")
                val body = ui("removeThemeConfirmBody", "Remove “{name}” and its saved settings? This cannot be undone.")
                    .replace("{name}", name)
                AlertDialog.Builder(context)
                    .setTitle(title)
                    .setMessage(body)
                    .setPositiveButton(ui("removeTheme", "Remove")) { _, _ -> onRemoveTheme(id) }
                    .setNegativeButton(ui("cancel", "Cancel"), null)
                    .show()
            }.apply {
                layoutParams = LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                ).apply { marginStart = dp(10) }
            }
            actions.addView(btnRemove)
        }

        actionsScroll.addView(actions)
        wrapper.addView(actionsScroll)
        return wrapper
    }

    private fun buildSetupGuideCard(): View {
        val card = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = roundedBg("#18332B", 16, "#2E6A58", 1)
            setPadding(dp(16), dp(14), dp(16), dp(14))
        }
        val titleTv = TextView(context).apply {
            text = ui("androidSetupTitle", "Activate DiyarDesk Live Wallpaper")
            setTextColor(Color.WHITE)
            textSize = 15.5f
            setTypeface(typeface, Typeface.BOLD)
        }
        val bodyTv = TextView(context).apply {
            text = ui(
                "androidSetupBody",
                "1. Tap Set Live Wallpaper → 2. Pick DiyarDesk in the system picker → 3. Match your tablet icons in Icon Layout."
            )
            setTextColor(Color.parseColor("#C7E3DA"))
            textSize = 12.5f
            setPadding(0, dp(4), 0, dp(10))
        }
        val row = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        val ctaWallpaper = styledButton(ui("androidSetupCta", "Set Live Wallpaper"), "#1F6F5B") {
            onOpenWallpaperPicker()
        }
        val ctaIcons = styledButton(ui("androidSetupIconsCta", "Open Icon Layout"), "#22453B") {
            onOpenIconLayout()
        }.apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply { marginStart = dp(10) }
        }
        row.addView(ctaWallpaper)
        row.addView(ctaIcons)
        card.addView(titleTv)
        card.addView(bodyTv)
        card.addView(row)
        return card
    }

    private fun buildThemePickerCard(): View {
        val card = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = roundedBg("#132A24", 16, "#21463D", 1)
            setPadding(dp(14), dp(14), dp(14), dp(14))
        }

        val hasCustom = themesList.any {
            it.optBoolean("imported", false) || it.optString("id").startsWith("custom-")
        }
        val filtered = themesList.filter { th ->
            if (activeCategory == "all") true
            else {
                val cat = if (th.optBoolean("imported", false) || th.optString("id").startsWith("custom-")) {
                    "custom"
                } else {
                    th.optString("category", "art")
                }
                cat == activeCategory
            }
        }

        val header = TextView(context).apply {
            text = "${ui("themes", "Themes")} (${filtered.size})"
            setTextColor(Color.WHITE)
            textSize = 15f
            setTypeface(typeface, Typeface.BOLD)
            setPadding(0, 0, 0, dp(8))
        }
        card.addView(header)

        val catScroll = HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            setPadding(0, 0, 0, dp(10))
        }
        val catRow = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
        }
        val catDefs = mutableListOf(
            "all" to ui("catAll", "All"),
            "widgets" to ui("catWidgets", "Widgets"),
            "design3d" to ui("catDesign3d", "3D & Design"),
            "nature" to ui("catNature", "Nature"),
            "art" to ui("catArt", "Art")
        )
        if (hasCustom) catDefs.add("custom" to ui("catCustom", "Custom"))
        for ((catId, catLabel) in catDefs) {
            val isOn = activeCategory == catId
            val chip = styledButton(catLabel, if (isOn) "#1F6F5B" else "#10221D").apply {
                setPadding(dp(12), dp(5), dp(12), dp(5))
                setOnClickListener {
                    if (activeCategory != catId) {
                        activeCategory = catId
                        refresh()
                    }
                }
            }
            catRow.addView(chip, LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply { marginEnd = dp(8) })
        }
        catScroll.addView(catRow)
        card.addView(catScroll)

        val widthDp = context.resources.configuration.screenWidthDp
        val cols = when {
            widthDp >= 900 -> 4
            widthDp >= 600 -> 3
            else -> 2
        }
        val gridContainer = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
        }

        val activeId = store.activeTheme
        var currentRow: LinearLayout? = null
        var colIndex = 0
        for (th in filtered) {
            val id = th.optString("id")
            if (id.isBlank()) continue
            if (colIndex == 0) {
                currentRow = LinearLayout(context).apply {
                    orientation = LinearLayout.HORIZONTAL
                }
                gridContainer.addView(currentRow, LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                ).apply { bottomMargin = dp(10) })
            }
            val selected = id == activeId
            val name = themeName(id, th.optString("name", id))

            val tile = FrameLayout(context).apply {
                background = roundedBg(
                    if (selected) "#1C4F40" else "#0E201B",
                    12,
                    if (selected) "#4EE0B5" else "#23493E",
                    if (selected) 2 else 1
                )
                clipToOutline = true
                layoutParams = LinearLayout.LayoutParams(0, dp(104), 1f).apply {
                    if (colIndex < cols - 1) marginEnd = dp(10)
                }
                setOnClickListener {
                    if (store.activeTheme != id) {
                        store.setActiveTheme(id)
                        refresh()
                    }
                }
            }

            val bmp = loadThemeThumbnail(th)
            if (bmp != null) {
                val img = ImageView(context).apply {
                    setImageBitmap(bmp)
                    scaleType = ImageView.ScaleType.CENTER_CROP
                    layoutParams = FrameLayout.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        ViewGroup.LayoutParams.MATCH_PARENT
                    )
                }
                tile.addView(img)
            }

            val overlay = View(context).apply {
                background = GradientDrawable(
                    GradientDrawable.Orientation.TOP_BOTTOM,
                    intArrayOf(
                        Color.argb(40, 6, 16, 14),
                        Color.argb(225, 6, 16, 14)
                    )
                )
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
            }
            tile.addView(overlay)

            val memTier = th.optString("memoryTier", "")
            makeTierPill(memTier, compact = true)?.let { tierPill ->
                tierPill.layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    Gravity.TOP or Gravity.START
                ).apply {
                    setMargins(dp(6), dp(6), dp(6), dp(6))
                }
                tile.addView(tierPill)
            }

            if (selected) {
                val pill = TextView(context).apply {
                    text = ui("badgeOn", "In use")
                    setTextColor(Color.parseColor("#06261E"))
                    textSize = 10f
                    setTypeface(typeface, Typeface.BOLD)
                    background = roundedBg("#5EE8BE", 999)
                    setPadding(dp(7), dp(2), dp(7), dp(2))
                    layoutParams = FrameLayout.LayoutParams(
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                        Gravity.TOP or Gravity.END
                    ).apply {
                        setMargins(dp(6), dp(6), dp(6), dp(6))
                    }
                }
                tile.addView(pill)
            }

            val label = TextView(context).apply {
                text = name
                setTextColor(Color.WHITE)
                textSize = 12.5f
                setTypeface(typeface, if (selected) Typeface.BOLD else Typeface.NORMAL)
                maxLines = 2
                ellipsize = TextUtils.TruncateAt.END
                setPadding(dp(8), dp(6), dp(8), dp(7))
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    Gravity.BOTTOM
                )
            }
            tile.addView(label)
            currentRow?.addView(tile)
            colIndex = (colIndex + 1) % cols
        }

        if (colIndex != 0 && currentRow != null) {
            for (rem in colIndex until cols) {
                val spacerView = View(context).apply {
                    layoutParams = LinearLayout.LayoutParams(0, dp(104), 1f).apply {
                        if (rem < cols - 1) marginEnd = dp(10)
                    }
                }
                currentRow.addView(spacerView)
            }
        }

        card.addView(gridContainer)
        return card
    }

    private fun buildParametersCard(themeObj: JSONObject): View {
        val themeId = themeObj.optString("id")
        val card = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = roundedBg("#132A24", 16, "#21463D", 1)
            setPadding(dp(16), dp(14), dp(16), dp(14))
        }

        val savedParams = store.getParamsCopy(themeId)
        val presets = themeObj.optJSONArray("presets")
        val groups = themeObj.optJSONArray("groups")
        if ((groups == null || groups.length() == 0) && (presets == null || presets.length() == 0)) {
            val emptyTv = TextView(context).apply {
                text = ui("noParams", "This theme has no settings yet")
                setTextColor(Color.parseColor("#9BB9AF"))
                textSize = 13.5f
            }
            card.addView(emptyTv)
            return card
        }

        var addedCount = 0
        if (presets != null && presets.length() > 0) {
            val prTitle = TextView(context).apply {
                text = ui("quickPresets", "Quick styles")
                setTextColor(Color.parseColor("#7FE0C2"))
                textSize = 13.5f
                setTypeface(typeface, Typeface.BOLD)
                setPadding(0, 0, 0, dp(8))
            }
            card.addView(prTitle)

            val prScroll = HorizontalScrollView(context).apply {
                isHorizontalScrollBarEnabled = false
            }
            val prRow = LinearLayout(context).apply {
                orientation = LinearLayout.HORIZONTAL
            }
            val lang = if (store.getLang() == "auto") Locale.getDefault().language else store.getLang()
            for (pi in 0 until presets.length()) {
                val pr = presets.optJSONObject(pi) ?: continue
                val values = pr.optJSONObject("values") ?: continue
                val label = if (lang == "tr" && pr.has("nameTr")) {
                    pr.optString("nameTr", pr.optString("name", ""))
                } else {
                    pr.optString("name", "")
                }
                if (label.isBlank()) continue
                val chipBtn = styledButton(label, accent = false).apply {
                    setPadding(dp(12), dp(6), dp(12), dp(6))
                    setOnClickListener {
                        val keys = values.keys()
                        while (keys.hasNext()) {
                            val k = keys.next()
                            store.setParam(themeId, k, values.opt(k))
                        }
                        refresh()
                    }
                }
                prRow.addView(chipBtn, LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    marginEnd = dp(8)
                })
            }
            prScroll.addView(prRow)
            card.addView(prScroll)
            if (groups != null && groups.length() > 0) {
                card.addView(divider())
            }
            addedCount++
        }

        if (groups == null) {
            return card
        }
        for (gi in 0 until groups.length()) {
            val g = groups.optJSONObject(gi) ?: continue
            val params = g.optJSONArray("params") ?: continue
            if (params.length() == 0) continue

            if (addedCount > 0) {
                card.addView(divider())
            }

            val gTitle = TextView(context).apply {
                text = groupName(themeId, gi, g.optString("name", ""))
                setTextColor(Color.parseColor("#7FE0C2"))
                textSize = 13.5f
                setTypeface(typeface, Typeface.BOLD)
                setPadding(0, if (addedCount == 0) 0 else dp(6), 0, dp(8))
            }
            card.addView(gTitle)

            for (pi in 0 until params.length()) {
                val p = params.optJSONObject(pi) ?: continue
                val key = p.optString("key")
                if (key.isBlank()) continue
                val row = buildParamControl(themeId, p, savedParams) ?: continue
                card.addView(row)
                addedCount++
            }
        }

        if (addedCount == 0) {
            val emptyTv = TextView(context).apply {
                text = ui("noParams", "This theme has no settings yet")
                setTextColor(Color.parseColor("#9BB9AF"))
                textSize = 13.5f
            }
            card.addView(emptyTv)
        }
        return card
    }

    private fun buildParamControl(themeId: String, p: JSONObject, savedParams: JSONObject): View? {
        val key = p.optString("key")
        val type = p.optString("type", "")
        if (type == "audio" || type == "color") {
            return null
        }

        val label = paramLabel(themeId, key, p.optString("label", key))
        val hint = paramHint(themeId, key, p.optString("hint", ""))
        val options = p.optJSONArray("options")
        val values = p.optJSONArray("values")
        val defaultVal = p.opt("default")
        val rawCurrent = if (savedParams.has(key)) savedParams.opt(key) else defaultVal

        val min = p.optDouble("min", Double.NaN)
        val max = p.optDouble("max", Double.NaN)
        val step = p.optDouble("step", 1.0)
        val isBoolToggle = type == "toggle" ||
            (options == null && !min.isNaN() && !max.isNaN() && min == 0.0 && max == 1.0 && step == 1.0)

        if (isBoolToggle) {
            val checked = when (rawCurrent) {
                is Boolean -> rawCurrent
                is Number -> rawCurrent.toDouble() >= 0.5
                is String -> rawCurrent == "1" || rawCurrent.equals("true", ignoreCase = true)
                else -> false
            }
            val storeAsBoolean = defaultVal is Boolean
            return buildToggleRow(label, hint, checked) { next ->
                val out: Any = if (storeAsBoolean) next else if (next) 1 else 0
                store.setParam(themeId, key, out)
            }
        }

        if (options != null && options.length() > 0) {
            val labels = ArrayList<String>(options.length())
            val optionVals = ArrayList<Any>(options.length())
            for (i in 0 until options.length()) {
                labels.add(paramOption(themeId, key, i, options.optString(i, "$i")))
                if (values != null && i < values.length()) {
                    optionVals.add(values.opt(i) ?: i)
                } else {
                    optionVals.add(i)
                }
            }
            var selectedIdx = optionVals.indexOfFirst { valuesEqual(it, rawCurrent) }
            if (selectedIdx < 0) selectedIdx = 0

            return buildSpinnerRow(label, hint, labels, selectedIdx) { nextIdx ->
                if (nextIdx in optionVals.indices) {
                    store.setParam(themeId, key, optionVals[nextIdx])
                }
            }
        }

        if (!min.isNaN() && !max.isNaN() && max > min) {
            val safeStep = if (step > 0.0) step else (max - min) / 100.0
            val curNum = when (rawCurrent) {
                is Number -> rawCurrent.toDouble()
                is String -> rawCurrent.toDoubleOrNull() ?: min
                else -> (defaultVal as? Number)?.toDouble() ?: min
            }.coerceIn(min, max)
            val totalSteps = max(1, ((max - min) / safeStep).roundToInt())
            val curStep = ((curNum - min) / safeStep).roundToInt().coerceIn(0, totalSteps)
            val format = p.optString("format", "")
            val unit = p.optString("unit", "")
            val isIntParam = abs(safeStep - safeStep.roundToInt()) < 1e-6 && abs(min - min.roundToInt()) < 1e-6

            val container = LinearLayout(context).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(0, dp(6), 0, dp(10))
            }

            val topRow = LinearLayout(context).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
            }
            val labelTv = TextView(context).apply {
                text = label
                setTextColor(Color.WHITE)
                textSize = 14f
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            }
            val valTv = TextView(context).apply {
                text = formatNumeric(curNum, format, unit, isIntParam)
                setTextColor(Color.parseColor("#8FE5CA"))
                textSize = 13f
                setTypeface(typeface, Typeface.BOLD)
            }
            topRow.addView(labelTv)
            topRow.addView(valTv)
            container.addView(topRow)

            if (hint.isNotBlank()) {
                val hintTv = TextView(context).apply {
                    text = hint
                    setTextColor(Color.parseColor("#8EAE9F"))
                    textSize = 11.5f
                    setPadding(0, dp(2), 0, dp(4))
                }
                container.addView(hintTv)
            }

            val seek = SeekBar(context).apply {
                this.max = totalSteps
                progress = curStep
                setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                    override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                        val v = (min + progress * safeStep).coerceIn(min, max)
                        valTv.text = formatNumeric(v, format, unit, isIntParam)
                        if (fromUser) {
                            val out: Any = if (isIntParam) v.roundToInt() else ((v * 1000.0).roundToInt() / 1000.0)
                            store.setParam(themeId, key, out)
                        }
                    }

                    override fun onStartTrackingTouch(seekBar: SeekBar?) {}
                    override fun onStopTrackingTouch(seekBar: SeekBar?) {}
                })
            }
            container.addView(seek)
            return container
        }

        return null
    }

    private fun buildGlobalSettingsCard(): View {
        val card = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = roundedBg("#132A24", 16, "#21463D", 1)
            setPadding(dp(16), dp(14), dp(16), dp(14))
        }

        val header = TextView(context).apply {
            text = "${ui("language", "Language")} & ${ui("qualityProfileTitle", "Performance")}"
            setTextColor(Color.parseColor("#7FE0C2"))
            textSize = 13.5f
            setTypeface(typeface, Typeface.BOLD)
            setPadding(0, 0, 0, dp(8))
        }
        card.addView(header)

        // Language selector
        val langCodes = listOf("auto", "en", "tr", "zh-CN", "zh-TW", "de", "fr", "ja", "ko")
        val langLabels = listOf(
            ui("langAuto", "System default"),
            "English",
            "Türkçe",
            "简体中文",
            "繁體中文",
            "Deutsch",
            "Français",
            "日本語",
            "한국어"
        )
        val curLangIdx = langCodes.indexOf(store.lang).coerceAtLeast(0)
        card.addView(buildSpinnerRow(ui("language", "Language"), "", langLabels, curLangIdx) { idx ->
            val next = langCodes[idx]
            if (next != store.lang) {
                store.setLanguage(next)
                refresh()
            }
        })

        // Quality profile selector
        val qVals = listOf("eco", "balanced", "high")
        val qLabels = listOf(
            ui("qualityEco", "Eco (15 FPS)"),
            ui("qualityBalanced", "Balanced (30 FPS)"),
            ui("qualityHigh", "High (60 FPS)")
        )
        val curQIdx = qVals.indexOf(store.qualityProfile).coerceAtLeast(1)
        card.addView(buildSpinnerRow(ui("qualityProfileTitle", "Quality & FPS"), "", qLabels, curQIdx) { idx ->
            val next = qVals[idx]
            if (next != store.qualityProfile) {
                store.setQualityProfileValue(next)
            }
        })

        // Virtual bar mode
        val vbVals = listOf("auto", "always", "off")
        val vbLabels = listOf(
            ui("virtualBarAuto", "Icons: Auto"),
            ui("virtualBarAlways", "Icons: Always show"),
            ui("virtualBarOff", "Icons: Hidden")
        )
        val curVbIdx = vbVals.indexOf(store.virtualBarMode).coerceAtLeast(0)
        card.addView(buildSpinnerRow(ui("virtualBarTitle", "Virtual icon bar"), "", vbLabels, curVbIdx) { idx ->
            val next = vbVals[idx]
            if (next != store.virtualBarMode) {
                store.setVirtualBarModeValue(next)
            }
        })

        // Low mode toggle
        card.addView(
            buildToggleRow(
                ui("lowMode", "Eco mode (Low)"),
                ui("lowModeTitle", ""),
                store.lowMode
            ) { next ->
                if (next != store.lowMode) store.setLowModeEnabled(next)
            }
        )

        // Disable auto-pause toggle
        card.addView(
            buildToggleRow(
                ui("disableAutoPause", "Disable auto-pause"),
                ui("disableAutoPauseTitle", ""),
                store.disableAutoPause
            ) { next ->
                if (next != store.disableAutoPause) store.setDisableAutoPauseEnabled(next)
            }
        )

        return card
    }

    // ------------------------------------------------------------------ Row Widgets

    private fun buildToggleRow(
        label: String,
        hint: String,
        initial: Boolean,
        onChange: (Boolean) -> Unit
    ): View {
        val row = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(8), 0, dp(8))
        }
        val textCol = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply {
                marginEnd = dp(12)
            }
        }
        val labelTv = TextView(context).apply {
            text = label
            setTextColor(Color.WHITE)
            textSize = 14f
        }
        textCol.addView(labelTv)
        if (hint.isNotBlank()) {
            val hintTv = TextView(context).apply {
                text = hint
                setTextColor(Color.parseColor("#8EAE9F"))
                textSize = 11.5f
                setPadding(0, dp(2), 0, 0)
            }
            textCol.addView(hintTv)
        }
        row.addView(textCol)

        val sw = SwitchCompat(context).apply {
            isChecked = initial
            setOnCheckedChangeListener { _, checked -> onChange(checked) }
        }
        row.addView(sw)
        return row
    }

    private fun buildSpinnerRow(
        label: String,
        hint: String,
        items: List<String>,
        initialIndex: Int,
        onSelected: (Int) -> Unit
    ): View {
        val container = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, dp(6), 0, dp(10))
        }

        val top = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }

        val labelCol = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply {
                marginEnd = dp(12)
            }
        }
        val labelTv = TextView(context).apply {
            text = label
            setTextColor(Color.WHITE)
            textSize = 14f
        }
        labelCol.addView(labelTv)
        if (hint.isNotBlank()) {
            val hintTv = TextView(context).apply {
                text = hint
                setTextColor(Color.parseColor("#8EAE9F"))
                textSize = 11.5f
                setPadding(0, dp(2), 0, 0)
            }
            labelCol.addView(hintTv)
        }
        top.addView(labelCol)

        val adapter = object : ArrayAdapter<String>(context, android.R.layout.simple_spinner_item, items) {
            override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
                val v = super.getView(position, convertView, parent) as TextView
                v.setTextColor(Color.WHITE)
                v.textSize = 13.5f
                v.setPadding(dp(10), dp(6), dp(10), dp(6))
                return v
            }

            override fun getDropDownView(position: Int, convertView: View?, parent: ViewGroup): View {
                val v = super.getDropDownView(position, convertView, parent) as TextView
                v.setTextColor(Color.WHITE)
                v.setBackgroundColor(Color.parseColor("#16322B"))
                v.textSize = 14f
                v.setPadding(dp(14), dp(10), dp(14), dp(10))
                return v
            }
        }
        adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item)

        var lastSelected = initialIndex
        val spinner = Spinner(context).apply {
            this.adapter = adapter
            background = roundedBg("#1C3D34", 10, "#2E6959", 1)
            setSelection(initialIndex, false)
            onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
                override fun onItemSelected(parent: AdapterView<*>?, view: View?, position: Int, id: Long) {
                    if (position != lastSelected) {
                        lastSelected = position
                        onSelected(position)
                    }
                }

                override fun onNothingSelected(parent: AdapterView<*>?) {}
            }
        }
        top.addView(spinner)
        container.addView(top)
        return container
    }

    private fun makeTierPill(tier: String, compact: Boolean): TextView? {
        val (label, bgHex, fgHex) = when (tier) {
            "light" -> Triple(ui("ramTierLight", "Light"), "#134E3A", "#7CF5C8")
            "medium" -> Triple(ui("ramTierMedium", "Medium"), "#593D10", "#FFD67A")
            "heavy" -> Triple(ui("ramTierHeavy", "Heavy"), "#661E2B", "#FFB3C0")
            else -> return null
        }
        return TextView(context).apply {
            text = label
            setTextColor(Color.parseColor(fgHex))
            textSize = if (compact) 9.5f else 11f
            setTypeface(typeface, Typeface.BOLD)
            background = roundedBg(bgHex, 999)
            if (compact) setPadding(dp(6), dp(2), dp(6), dp(2))
            else setPadding(dp(10), dp(4), dp(10), dp(4))
        }
    }

    private fun styledButton(label: String, bgHex: String, onClick: () -> Unit): Button {
        return Button(context).apply {
            text = label
            isAllCaps = false
            setTextColor(Color.WHITE)
            textSize = 13f
            background = roundedBg(bgHex, 10)
            setPadding(dp(14), dp(8), dp(14), dp(8))
            minHeight = 0
            minimumHeight = 0
            setOnClickListener { onClick() }
        }
    }

    private fun spacer(heightDp: Int): View = View(context).apply {
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(heightDp))
    }

    private fun divider(): View = View(context).apply {
        setBackgroundColor(Color.parseColor("#21463D"))
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(1)).apply {
            topMargin = dp(8)
            bottomMargin = dp(8)
        }
    }

    private fun roundedBg(
        fillHex: String,
        radiusDp: Int,
        strokeHex: String? = null,
        strokeDp: Int = 1
    ): GradientDrawable = GradientDrawable().apply {
        shape = GradientDrawable.RECTANGLE
        cornerRadius = dp(radiusDp).toFloat()
        setColor(Color.parseColor(fillHex))
        if (strokeHex != null) {
            setStroke(dp(strokeDp), Color.parseColor(strokeHex))
        }
    }

    private fun dp(v: Int): Int = (v * context.resources.displayMetrics.density).roundToInt()

    // ------------------------------------------------------------------ Data & Assets

    private fun loadMergedThemes(): List<JSONObject> {
        return try {
            val base = context.assets.open("prototype/themes.json").bufferedReader(Charsets.UTF_8).use { it.readText() }
            val merged = JSONObject(importer.mergedThemesJson(base))
            val arr = merged.optJSONArray("themes") ?: JSONArray()
            buildList {
                for (i in 0 until arr.length()) {
                    val obj = arr.optJSONObject(i) ?: continue
                    if (obj.optString("status", "ready") != "soon") {
                        add(obj)
                    }
                }
            }
        } catch (_: Exception) {
            emptyList()
        }
    }

    private fun loadThemeThumbnail(themeObj: JSONObject): Bitmap? {
        val thumbCss = themeObj.optString("thumb", "")
        val match = urlPattern.find(thumbCss) ?: return null
        val rawPath = match.groupValues[1].substringBefore('?').trim()
        if (rawPath.isEmpty() || rawPath.contains("..")) return null
        if (thumbCache.containsKey(rawPath)) return thumbCache[rawPath]

        val bmp: Bitmap? = try {
            val opts = BitmapFactory.Options().apply {
                inPreferredConfig = Bitmap.Config.ARGB_8888
            }
            val clean = rawPath.trimStart('/')
            if (clean.startsWith("__custom/")) {
                val target = importer.resolveCustomAsset(clean.removePrefix("__custom/"))
                if (target != null && target.isFile) {
                    FileInputStream(target).use { BitmapFactory.decodeStream(it, null, opts) }
                } else {
                    null
                }
            } else {
                context.assets.open("prototype/$clean").use { BitmapFactory.decodeStream(it, null, opts) }
            }
        } catch (_: Exception) {
            null
        }

        if (thumbCache.size >= 64) {
            val firstKey = thumbCache.keys.firstOrNull()
            if (firstKey != null) {
                thumbCache.remove(firstKey)
            }
        }
        thumbCache[rawPath] = bmp
        return bmp
    }

    // ------------------------------------------------------------------ i18n

    private fun langChain(): List<String> {
        val out = ArrayList<String>(3)
        for (c in listOf(effLang, "en", "zh-CN")) {
            if (c !in out) out.add(c)
        }
        return out
    }

    private fun i18nTable(code: String): JSONObject {
        return i18nCache.getOrPut(code) {
            try {
                val raw = context.assets.open("prototype/i18n/$code.json")
                    .bufferedReader(Charsets.UTF_8)
                    .use { it.readText() }
                JSONObject(raw)
            } catch (_: Exception) {
                JSONObject()
            }
        }
    }

    private fun ui(key: String, fallback: String): String {
        for (c in langChain()) {
            val v = i18nTable(c).optJSONObject("ui")?.optString(key, "") ?: ""
            if (v.isNotEmpty()) return v
        }
        return fallback
    }

    private fun themeEntry(themeId: String, code: String): JSONObject? {
        return i18nTable(code).optJSONObject("themes")?.optJSONObject(themeId)
    }

    private fun themeName(themeId: String, fallback: String): String {
        for (c in langChain()) {
            val v = themeEntry(themeId, c)?.optString("name", "") ?: ""
            if (v.isNotEmpty()) return v
        }
        return fallback.ifBlank { themeId }
    }

    private fun themeDesc(themeId: String, fallback: String): String {
        for (c in langChain()) {
            val v = themeEntry(themeId, c)?.optString("desc", "") ?: ""
            if (v.isNotEmpty()) return v
        }
        return fallback
    }

    private fun groupName(themeId: String, groupIdx: Int, fallback: String): String {
        for (c in langChain()) {
            val arr = themeEntry(themeId, c)?.optJSONArray("groups") ?: continue
            val v = arr.optString(groupIdx, "")
            if (v.isNotEmpty()) return v
        }
        return fallback
    }

    private fun paramEntry(themeId: String, key: String, code: String): JSONObject? {
        return themeEntry(themeId, code)?.optJSONObject("params")?.optJSONObject(key)
    }

    private fun paramLabel(themeId: String, key: String, fallback: String): String {
        for (c in langChain()) {
            val v = paramEntry(themeId, key, c)?.optString("label", "") ?: ""
            if (v.isNotEmpty()) return v
        }
        return fallback
    }

    private fun paramHint(themeId: String, key: String, fallback: String): String {
        for (c in langChain()) {
            val v = paramEntry(themeId, key, c)?.optString("hint", "") ?: ""
            if (v.isNotEmpty()) return v
        }
        return fallback
    }

    private fun paramOption(themeId: String, key: String, idx: Int, fallback: String): String {
        for (c in langChain()) {
            val arr = paramEntry(themeId, key, c)?.optJSONArray("options") ?: continue
            val v = arr.optString(idx, "")
            if (v.isNotEmpty()) return v
        }
        return fallback
    }

    private fun valuesEqual(a: Any?, b: Any?): Boolean {
        if (a == b) return true
        if (a is Number && b is Number) {
            return abs(a.toDouble() - b.toDouble()) < 1e-6
        }
        return a?.toString() == b?.toString()
    }

    private fun formatNumeric(v: Double, format: String, unit: String, isInt: Boolean): String {
        return when (format) {
            "percent" -> "${(v * 100.0).roundToInt()}%"
            "times" -> String.format(Locale.US, "%.2fx", v)
            else -> {
                val base = if (isInt) v.roundToInt().toString() else String.format(Locale.US, "%.2f", v).trimEnd('0').trimEnd('.')
                if (unit.isNotEmpty()) "$base $unit" else base
            }
        }
    }
}
