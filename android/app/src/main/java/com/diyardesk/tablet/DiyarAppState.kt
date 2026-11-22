package com.diyardesk.tablet

import android.content.Context
import android.os.Build
import android.util.DisplayMetrics
import android.view.WindowManager
import java.io.File
import java.util.Locale

/** Process-wide holder so WallpaperService, SettingsActivity, and IconLayoutActivity share state immediately. */
object DiyarAppState {
    @Volatile private var storeInstance: ThemeStore? = null
    @Volatile private var importerInstance: CustomThemeImporter? = null

    fun store(context: Context): ThemeStore {
        return storeInstance ?: synchronized(this) {
            storeInstance ?: ThemeStore(File(context.applicationContext.filesDir, "deskland")).also {
                storeInstance = it
            }
        }
    }

    fun importer(context: Context): CustomThemeImporter {
        return importerInstance ?: synchronized(this) {
            importerInstance ?: CustomThemeImporter(File(context.applicationContext.filesDir, "custom_themes")).also {
                importerInstance = it
            }
        }
    }

    fun screenPhysicalSize(context: Context): Pair<Int, Int> {
        val wm = context.getSystemService(Context.WINDOW_SERVICE) as WindowManager
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val bounds = wm.currentWindowMetrics.bounds
            bounds.width().coerceAtLeast(1) to bounds.height().coerceAtLeast(1)
        } else {
            @Suppress("DEPRECATION")
            val dm = DisplayMetrics().also { wm.defaultDisplay.getRealMetrics(it) }
            dm.widthPixels.coerceAtLeast(1) to dm.heightPixels.coerceAtLeast(1)
        }
    }

    fun systemLangCode(): String {
        val loc = Locale.getDefault()
        val lang = loc.language.lowercase()
        if (lang == "zh") {
            val hant = loc.script == "Hant" || loc.country in setOf("TW", "HK", "MO")
            return if (hant) "zh-TW" else "zh-CN"
        }
        return if (lang in setOf("en", "tr", "de", "fr", "ja", "ko")) lang else "en"
    }
}
