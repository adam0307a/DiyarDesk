package com.diyardesk.tablet

import android.media.AudioAttributes
import android.media.MediaPlayer
import android.media.PlaybackParams
import android.os.Build
import android.util.Log
import android.view.Surface
import java.io.File
import java.io.FileInputStream
import kotlin.math.abs

/**
 * Hardware-accelerated video wallpaper playback directly on [WallpaperService.Engine]'s
 * [Surface] via Android's platform [MediaPlayer] — zero WebView / zero Chromium processes.
 */
class NativeVideoWallpaperPlayer(
    val file: File,
    initialSurface: Surface,
    initialPaused: Boolean,
    initialSpeed: Float,
    private val onError: (String) -> Unit
) {
    companion object {
        private const val TAG = "NativeVideoPlayer"
    }

    private var player: MediaPlayer? = null
    private var currentSurface: Surface? = initialSurface
    private var paused: Boolean = initialPaused
    private var targetSpeed: Float = initialSpeed.coerceIn(0.25f, 2.0f)
    private var speedAppliedNonDefault: Boolean = false
    private var prepared: Boolean = false
    private var released: Boolean = false

    init {
        startAsync()
    }

    private fun startAsync() {
        try {
            val mp = MediaPlayer()
            player = mp
            mp.setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_MOVIE)
                    .build()
            )
            mp.isLooping = true
            mp.setVolume(0f, 0f)
            currentSurface?.let { surf ->
                if (surf.isValid) {
                    mp.setSurface(surf)
                }
            }
            mp.setOnPreparedListener { readyMp ->
                synchronized(this) {
                    if (released || player !== readyMp) return@synchronized
                    prepared = true
                    try {
                        readyMp.setVideoScalingMode(MediaPlayer.VIDEO_SCALING_MODE_SCALE_TO_FIT_WITH_CROPPING)
                    } catch (_: Exception) {
                    }
                    if (paused) {
                        try {
                            readyMp.seekTo(0)
                        } catch (_: Exception) {
                        }
                    } else {
                        try {
                            readyMp.start()
                            applySpeedIfPlaying(readyMp)
                        } catch (t: Throwable) {
                            reportFailure("start failed: ${t.message}")
                        }
                    }
                }
            }
            mp.setOnErrorListener { _, what, extra ->
                reportFailure("MediaPlayer error what=$what extra=$extra")
                true
            }
            FileInputStream(file).use { fis ->
                mp.setDataSource(fis.fd)
            }
            mp.prepareAsync()
        } catch (t: Throwable) {
            reportFailure("init failed: ${t.message}")
        }
    }

    @Synchronized
    fun updateSurface(surface: Surface) {
        if (released) return
        if (currentSurface === surface) return
        currentSurface = surface
        val mp = player ?: return
        try {
            mp.setSurface(if (surface.isValid) surface else null)
        } catch (t: Throwable) {
            Log.w(TAG, "updateSurface failed: ${t.message}")
        }
    }

    @Synchronized
    fun setSpeed(speed: Float) {
        if (released) return
        val clamped = speed.coerceIn(0.25f, 2.0f)
        if (abs(clamped - targetSpeed) < 0.01f) return
        targetSpeed = clamped
        val mp = player ?: return
        if (prepared && !paused) {
            applySpeedIfPlaying(mp)
        }
    }

    @Synchronized
    fun setPaused(pause: Boolean) {
        if (released || paused == pause) return
        paused = pause
        val mp = player ?: return
        if (!prepared) return
        try {
            if (pause) {
                if (mp.isPlaying) {
                    mp.pause()
                }
            } else {
                if (!mp.isPlaying) {
                    mp.start()
                    applySpeedIfPlaying(mp)
                }
            }
        } catch (t: Throwable) {
            Log.w(TAG, "setPaused($pause) failed: ${t.message}")
        }
    }

    private fun applySpeedIfPlaying(mp: MediaPlayer) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) return
        if (paused || !prepared) return
        if (!speedAppliedNonDefault && abs(targetSpeed - 1.0f) <= 0.01f) return
        try {
            val params: PlaybackParams = mp.playbackParams
            params.speed = targetSpeed
            mp.playbackParams = params
            speedAppliedNonDefault = abs(targetSpeed - 1.0f) > 0.01f
        } catch (t: Throwable) {
            Log.w(TAG, "setSpeed($targetSpeed) failed: ${t.message}")
        }
    }

    private fun reportFailure(reason: String) {
        val shouldNotify = synchronized(this) {
            if (released) return
            Log.e(TAG, reason)
            destroyLocked()
            true
        }
        if (shouldNotify) {
            onError(reason)
        }
    }

    @Synchronized
    fun destroy() {
        destroyLocked()
    }

    private fun destroyLocked() {
        if (released) return
        released = true
        prepared = false
        currentSurface = null
        val mp = player ?: return
        player = null
        try {
            mp.setOnPreparedListener(null)
            mp.setOnErrorListener(null)
        } catch (_: Exception) {
        }
        try {
            if (mp.isPlaying) mp.stop()
        } catch (_: Exception) {
        }
        try {
            mp.setSurface(null)
        } catch (_: Exception) {
        }
        try {
            mp.reset()
        } catch (_: Exception) {
        }
        try {
            mp.release()
        } catch (_: Exception) {
        }
    }
}
