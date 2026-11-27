package com.diyardesk.tablet

import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.provider.Settings
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * Tablet icon & widget layout editor.
 * Provides both an in-app layout canvas (with drag & corner resize) and a one-tap
 * "Place on Home Screen" live overlay mode ([IconOverlayService]) so users can align boxes
 * directly over their real launcher icons and widgets without trial-and-error.
 */
class IconLayoutActivity : AppCompatActivity(), ThemeStore.Listener {

    private lateinit var store: ThemeStore
    private lateinit var canvasView: InteractiveIconCanvasView
    private var waitingForOverlayPermission = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = DiyarAppState.store(this)
        store.addListener(this)

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.parseColor("#0E1D19"))
        }

        val topBar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(Color.parseColor("#132A24"))
            setPadding(24, 16, 24, 16)
            gravity = Gravity.CENTER_VERTICAL
        }

        val title = TextView(this).apply {
            text = getString(R.string.icon_layout_title)
            setTextColor(Color.WHITE)
            textSize = 18f
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
        }
        topBar.addView(title)

        fun actionBtn(labelRes: Int, bgHex: String = "#1F6F5B", fgHex: String = "#FFFFFF", onClick: () -> Unit): Button = Button(this).apply {
            text = getString(labelRes)
            isAllCaps = false
            setTextColor(Color.parseColor(fgHex))
            setBackgroundColor(Color.parseColor(bgHex))
            val lp = LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            lp.marginStart = 10
            layoutParams = lp
            setOnClickListener { onClick() }
        }

        val buttonsRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }

        buttonsRow.addView(actionBtn(R.string.btn_live_overlay, "#D9822B", "#111111") {
            launchLiveHomeOverlay()
        })
        buttonsRow.addView(actionBtn(R.string.preset_5x6) {
            canvasView.setBoxes(IconLayoutMath.presetTablet5x6())
        })
        buttonsRow.addView(actionBtn(R.string.preset_4x5) {
            canvasView.setBoxes(IconLayoutMath.presetGrid4x5())
        })
        buttonsRow.addView(actionBtn(R.string.action_add_icon) {
            canvasView.addBox(isWidget = false)
        })
        buttonsRow.addView(actionBtn(R.string.action_add_widget) {
            canvasView.addBox(isWidget = true)
        })
        buttonsRow.addView(actionBtn(R.string.action_clear) {
            canvasView.setBoxes(emptyList())
        })
        buttonsRow.addView(actionBtn(R.string.action_save) {
            store.setIconBoxes(canvasView.getBoxes())
            Toast.makeText(this, R.string.toast_layout_saved, Toast.LENGTH_SHORT).show()
            finish()
        })

        val buttonScroller = HorizontalScrollView(this).apply {
            isHorizontalScrollBarEnabled = false
            addView(buttonsRow)
        }
        topBar.addView(buttonScroller)

        val hint = TextView(this).apply {
            text = getString(R.string.icon_layout_hint)
            setTextColor(Color.parseColor("#C7DBD4"))
            textSize = 13f
            setPadding(28, 12, 28, 12)
        }

        canvasView = InteractiveIconCanvasView(this, overlayMode = false).apply {
            setBoxes(store.getIconBoxes())
        }

        val frame = FrameLayout(this).apply {
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f)
            setPadding(28, 8, 28, 28)
            addView(canvasView, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        }

        root.addView(topBar)
        root.addView(hint)
        root.addView(frame)
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            insets
        }
        setContentView(root)
    }

    override fun onResume() {
        super.onResume()
        if (waitingForOverlayPermission && Settings.canDrawOverlays(this)) {
            waitingForOverlayPermission = false
            startOverlayAndGoHome()
        } else {
            waitingForOverlayPermission = false
        }
    }

    override fun onDestroy() {
        store.removeListener(this)
        super.onDestroy()
    }

    override fun onIconsChanged(store: ThemeStore) {
        runOnUiThread {
            canvasView.setBoxes(store.getIconBoxes())
        }
    }

    private fun launchLiveHomeOverlay() {
        // Persist any unsaved local canvas tweaks before starting the overlay
        store.setIconBoxes(canvasView.getBoxes())
        if (Settings.canDrawOverlays(this)) {
            startOverlayAndGoHome()
            return
        }
        AlertDialog.Builder(this)
            .setTitle(R.string.overlay_permission_title)
            .setMessage(R.string.overlay_permission_body)
            .setPositiveButton(android.R.string.ok) { _, _ ->
                waitingForOverlayPermission = true
                val intent = Intent(
                    Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                    Uri.parse("package:$packageName")
                )
                try {
                    startActivity(intent)
                } catch (_: Exception) {
                    startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION))
                }
            }
            .setNegativeButton(R.string.action_cancel, null)
            .show()
    }

    private fun startOverlayAndGoHome() {
        ContextCompat.startForegroundService(this, Intent(this, IconOverlayService::class.java))
        startActivity(
            Intent(Intent.ACTION_MAIN)
                .addCategory(Intent.CATEGORY_HOME)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        )
        finish()
    }
}
