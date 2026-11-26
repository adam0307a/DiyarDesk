package com.diyardesk.tablet

import android.opengl.EGL14
import android.opengl.EGLConfig
import android.opengl.EGLContext
import android.opengl.EGLDisplay
import android.opengl.EGLExt
import android.opengl.EGLSurface
import android.os.SystemClock
import android.util.Log
import android.view.Surface
import java.util.concurrent.ConcurrentLinkedQueue

/**
 * Dedicated EGL 1.4 + OpenGL ES 3.0 wallpaper renderer thread drawing directly onto
 * `WallpaperService.SurfaceHolder`'s [Surface] via `libdiyardesk_gl.so` JNI bindings.
 *
 * Eliminates `VirtualDisplay`, `Presentation`, and `WebView` overhead for all native Rust themes.
 */
class NativeGlWallpaperRenderer(
    private val surface: Surface,
    width: Int,
    height: Int,
    density: Float,
    private val initialMessages: List<String>,
    initialPaused: Boolean,
    initialLowMode: Boolean,
    initialQualityProfile: String,
    initialOnBattery: Boolean,
    initialReducedMotion: Boolean,
    private val onError: (reason: String, recoverable: Boolean) -> Unit
) {
    companion object {
        private const val TAG = "DiyarDeskGL"
        private const val EGL_CONTEXT_CLIENT_VERSION = 0x3098
        private const val SURFACE_ATTACH_MAX_ATTEMPTS = 4
        private const val SURFACE_ATTACH_RETRY_MS = 40L
    }

    private val commandQueue = ConcurrentLinkedQueue<() -> Unit>()
    private val wakeLock = Object()

    @Volatile private var running = true
    @Volatile private var paused = initialPaused
    @Volatile private var lowMode = initialLowMode
    @Volatile private var qualityProfile = initialQualityProfile
    @Volatile private var onBattery = initialOnBattery
    @Volatile private var reducedMotion = initialReducedMotion
    @Volatile private var surfaceW = width.coerceAtLeast(64)
    @Volatile private var surfaceH = height.coerceAtLeast(64)
    @Volatile private var dpiScale = density.coerceAtLeast(0.5f)
    @Volatile private var dirtyWhilePaused = true

    private var glThread: Thread? = null
    private var engineHandle: Long = 0L

    fun start() {
        if (glThread != null) return
        val thread = Thread({ runLoop() }, "DiyarDeskGL-ES3").apply {
            priority = Thread.NORM_PRIORITY
        }
        glThread = thread
        thread.start()
    }

    fun postJson(json: String) {
        if (!running) return
        commandQueue.offer {
            val h = engineHandle
            if (h != 0L) {
                NativeGlBridge.nativePostJson(h, json)
                dirtyWhilePaused = true
            }
        }
        wake()
    }

    fun resize(width: Int, height: Int, density: Float) {
        if (!running) return
        val w = width.coerceAtLeast(64)
        val h = height.coerceAtLeast(64)
        val d = density.coerceAtLeast(0.5f)
        surfaceW = w
        surfaceH = h
        dpiScale = d
        commandQueue.offer {
            val handle = engineHandle
            if (handle != 0L) {
                NativeGlBridge.nativeResize(handle, w, h, d)
                dirtyWhilePaused = true
            }
        }
        wake()
    }

    fun setPaused(isPaused: Boolean) {
        paused = isPaused
        if (!running) return
        commandQueue.offer {
            val handle = engineHandle
            if (handle != 0L) {
                NativeGlBridge.nativeSetPaused(handle, isPaused)
            }
        }
        wake()
    }

    fun setLowMode(isLow: Boolean) {
        lowMode = isLow
        if (!running) return
        commandQueue.offer {
            val handle = engineHandle
            if (handle != 0L) {
                NativeGlBridge.nativeSetLowMode(handle, isLow)
                dirtyWhilePaused = true
            }
        }
        wake()
    }

    fun setPowerState(profile: String, battery: Boolean, reduced: Boolean) {
        qualityProfile = profile
        onBattery = battery
        reducedMotion = reduced
        wake()
    }

    fun dispatchTouch(actionMasked: Int, rawPxX: Float, rawPxY: Float) {
        if (!running || paused) return
        val kind = NativeGlProtocol.mapMotionActionToPointerKind(actionMasked)
        if (kind < 0) return
        val scale = dpiScale.coerceAtLeast(0.5f)
        val cssX = rawPxX / scale
        val cssY = rawPxY / scale
        commandQueue.offer {
            val handle = engineHandle
            if (handle != 0L) {
                NativeGlBridge.nativePointer(handle, kind, cssX, cssY)
            }
        }
        wake()
    }

    /** Stops the render loop and waits for EGL/GL teardown so the surface can be released safely. */
    fun destroySync(timeoutMs: Long = 2000L) {
        running = false
        wake()
        val t = glThread
        glThread = null
        if (t != null && t !== Thread.currentThread()) {
            try {
                t.join(timeoutMs)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
            }
        }
    }

    private fun wake() {
        synchronized(wakeLock) {
            wakeLock.notifyAll()
        }
    }

    private fun runLoop() {
        var display: EGLDisplay = EGL14.EGL_NO_DISPLAY
        var eglContext: EGLContext = EGL14.EGL_NO_CONTEXT
        var eglSurface: EGLSurface = EGL14.EGL_NO_SURFACE

        try {
            if (!surface.isValid) {
                if (running) onError("Wallpaper surface is invalid", true)
                return
            }

            display = EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY)
            if (display == EGL14.EGL_NO_DISPLAY) {
                if (running) onError("eglGetDisplay failed", false)
                return
            }

            val version = IntArray(2)
            if (!EGL14.eglInitialize(display, version, 0, version, 1)) {
                if (running) onError("eglInitialize failed (0x${Integer.toHexString(EGL14.eglGetError())})", false)
                return
            }

            val config = chooseEglConfig(display)
            if (config == null) {
                if (running) onError("eglChooseConfig failed for OpenGL ES 3.0", false)
                return
            }

            val ctxAttribs = intArrayOf(
                EGL_CONTEXT_CLIENT_VERSION, 3,
                EGL14.EGL_NONE
            )
            eglContext = EGL14.eglCreateContext(display, config, EGL14.EGL_NO_CONTEXT, ctxAttribs, 0)
            if (eglContext == null || eglContext == EGL14.EGL_NO_CONTEXT) {
                if (running) onError("eglCreateContext(GLES 3.0) failed (0x${Integer.toHexString(EGL14.eglGetError())})", false)
                return
            }

            // Retry briefly if a previous VirtualDisplay producer is still finishing asynchronous detach.
            val surfAttribs = intArrayOf(EGL14.EGL_NONE)
            var lastSurfErr = EGL14.EGL_SUCCESS
            for (attempt in 0 until SURFACE_ATTACH_MAX_ATTEMPTS) {
                if (!running || !surface.isValid) {
                    if (running) onError("Wallpaper surface invalidated during attach", true)
                    return
                }
                eglSurface = EGL14.eglCreateWindowSurface(display, config, surface, surfAttribs, 0)
                if (eglSurface != null && eglSurface != EGL14.EGL_NO_SURFACE) {
                    break
                }
                lastSurfErr = EGL14.eglGetError()
                val transientAttach = lastSurfErr == EGL14.EGL_BAD_ALLOC || lastSurfErr == EGL14.EGL_BAD_NATIVE_WINDOW
                if (!transientAttach || attempt + 1 >= SURFACE_ATTACH_MAX_ATTEMPTS) {
                    if (running) {
                        onError(
                            "eglCreateWindowSurface failed (0x${Integer.toHexString(lastSurfErr)})",
                            transientAttach
                        )
                    }
                    return
                }
                try {
                    Thread.sleep(SURFACE_ATTACH_RETRY_MS)
                } catch (_: InterruptedException) {
                    return
                }
            }

            if (!EGL14.eglMakeCurrent(display, eglSurface, eglSurface, eglContext)) {
                val err = EGL14.eglGetError()
                if (running) {
                    onError(
                        "eglMakeCurrent failed (0x${Integer.toHexString(err)})",
                        err == EGL14.EGL_CONTEXT_LOST || err == EGL14.EGL_BAD_SURFACE
                    )
                }
                return
            }

            // Enable VSync swap pacing when supported by the compositor.
            EGL14.eglSwapInterval(display, 1)

            val handle = NativeGlBridge.nativeCreate(surfaceW, surfaceH, dpiScale)
            if (handle == 0L) {
                val err = NativeGlBridge.nativeLastError() ?: "nativeCreate returned null"
                if (running) onError("Native GL engine init failed: $err", false)
                return
            }
            engineHandle = handle

            // Apply initial theme, settings, icons, lowMode, and pause state before first draw.
            NativeGlBridge.nativeSetLowMode(handle, lowMode)
            for (msg in initialMessages) {
                NativeGlBridge.nativePostJson(handle, msg)
            }
            NativeGlBridge.nativeSetPaused(handle, paused)

            var lastFrameNs = SystemClock.elapsedRealtimeNanos()
            var renderedFrames = 0L

            while (running) {
                drainCommands()
                if (!running) break

                val isPausedNow = paused
                if (isPausedNow && renderedFrames > 0L && !dirtyWhilePaused) {
                    var interrupted = false
                    synchronized(wakeLock) {
                        if (running && paused && commandQueue.isEmpty() && !dirtyWhilePaused) {
                            try {
                                wakeLock.wait()
                            } catch (_: InterruptedException) {
                                interrupted = true
                            }
                        }
                    }
                    if (interrupted) break
                    lastFrameNs = SystemClock.elapsedRealtimeNanos()
                    continue
                }

                val nowNs = SystemClock.elapsedRealtimeNanos()
                val dt = if (isPausedNow) {
                    0.0f
                } else {
                    ((nowNs - lastFrameNs) / 1_000_000_000.0f).coerceIn(0.001f, 0.08f)
                }
                lastFrameNs = nowNs

                val frameStartMs = SystemClock.elapsedRealtime()
                if (!NativeGlBridge.nativeRender(handle, dt, 0)) {
                    val err = NativeGlBridge.nativeLastError() ?: "nativeRender returned false"
                    Log.w(TAG, "Native GL frame render failed: $err")
                }

                if (!EGL14.eglSwapBuffers(display, eglSurface)) {
                    val eglErr = EGL14.eglGetError()
                    if (eglErr == EGL14.EGL_CONTEXT_LOST ||
                        eglErr == EGL14.EGL_BAD_SURFACE ||
                        eglErr == EGL14.EGL_BAD_NATIVE_WINDOW
                    ) {
                        Log.w(TAG, "EGL surface/context lost (0x${Integer.toHexString(eglErr)}); stopping loop")
                        if (running) {
                            onError("EGL lost (0x${Integer.toHexString(eglErr)})", true)
                        }
                        break
                    }
                }

                renderedFrames++
                dirtyWhilePaused = false

                // Drain any native outbox events so the queue never grows unbounded.
                var pollCount = 0
                while (pollCount < 8 && NativeGlBridge.nativePollEvent(handle) != null) {
                    pollCount++
                }

                if (!isPausedNow) {
                    val targetMs = NativeGlProtocol.targetFrameIntervalMs(
                        qualityProfile = qualityProfile,
                        lowMode = lowMode,
                        onBattery = onBattery,
                        reducedMotion = reducedMotion
                    )
                    val elapsedMs = SystemClock.elapsedRealtime() - frameStartMs
                    val sleepMs = targetMs - elapsedMs
                    if (sleepMs > 1L) {
                        var interrupted = false
                        synchronized(wakeLock) {
                            if (running && !paused && commandQueue.isEmpty()) {
                                try {
                                    wakeLock.wait(sleepMs)
                                } catch (_: InterruptedException) {
                                    interrupted = true
                                }
                            }
                        }
                        if (interrupted) break
                    }
                }
            }
        } catch (t: Throwable) {
            Log.e(TAG, "Native GL render loop crashed", t)
            if (running) {
                onError(t.message ?: t.javaClass.simpleName, false)
            }
        } finally {
            val h = engineHandle
            engineHandle = 0L
            if (h != 0L) {
                try {
                    if (display != EGL14.EGL_NO_DISPLAY &&
                        eglSurface != EGL14.EGL_NO_SURFACE &&
                        eglContext != EGL14.EGL_NO_CONTEXT
                    ) {
                        EGL14.eglMakeCurrent(display, eglSurface, eglSurface, eglContext)
                    }
                    NativeGlBridge.nativeDestroy(h)
                } catch (_: Throwable) {
                }
            }
            if (display != EGL14.EGL_NO_DISPLAY) {
                try {
                    EGL14.eglMakeCurrent(
                        display,
                        EGL14.EGL_NO_SURFACE,
                        EGL14.EGL_NO_SURFACE,
                        EGL14.EGL_NO_CONTEXT
                    )
                } catch (_: Throwable) {
                }
                if (eglSurface != EGL14.EGL_NO_SURFACE) {
                    EGL14.eglDestroySurface(display, eglSurface)
                }
                if (eglContext != EGL14.EGL_NO_CONTEXT) {
                    EGL14.eglDestroyContext(display, eglContext)
                }
                // IMPORTANT: Never call EGL14.eglTerminate(display) on EGL_DEFAULT_DISPLAY.
                // EGL_DEFAULT_DISPLAY is a process-wide singleton shared with concurrent
                // WallpaperService.Engine preview/home instances and SettingsActivity WebView;
                // terminating it invalidates their active EGLContexts and EGLSurfaces.
                try {
                    EGL14.eglReleaseThread()
                } catch (_: Throwable) {
                }
            }
        }
    }

    private fun drainCommands() {
        while (true) {
            val cmd = commandQueue.poll() ?: break
            try {
                cmd()
            } catch (t: Throwable) {
                Log.w(TAG, "Error executing native GL command", t)
            }
        }
    }

    private fun chooseEglConfig(display: EGLDisplay): EGLConfig? {
        val candidates = arrayOf(
            // Preferred: OpenGL ES 3.0 + RGBA8 + 24-bit depth (needed for 3D themes like voxelcraft/studio-glass)
            intArrayOf(
                EGL14.EGL_SURFACE_TYPE, EGL14.EGL_WINDOW_BIT,
                EGL14.EGL_RENDERABLE_TYPE, EGLExt.EGL_OPENGL_ES3_BIT_KHR,
                EGL14.EGL_RED_SIZE, 8,
                EGL14.EGL_GREEN_SIZE, 8,
                EGL14.EGL_BLUE_SIZE, 8,
                EGL14.EGL_ALPHA_SIZE, 8,
                EGL14.EGL_DEPTH_SIZE, 24,
                EGL14.EGL_NONE
            ),
            // Fallback 1: ES3 + 16-bit depth
            intArrayOf(
                EGL14.EGL_SURFACE_TYPE, EGL14.EGL_WINDOW_BIT,
                EGL14.EGL_RENDERABLE_TYPE, EGLExt.EGL_OPENGL_ES3_BIT_KHR,
                EGL14.EGL_RED_SIZE, 8,
                EGL14.EGL_GREEN_SIZE, 8,
                EGL14.EGL_BLUE_SIZE, 8,
                EGL14.EGL_ALPHA_SIZE, 8,
                EGL14.EGL_DEPTH_SIZE, 16,
                EGL14.EGL_NONE
            ),
            // Fallback 2: ES2 renderable bit (many Android drivers expose ES3 context on ES2 config) + 24-bit depth
            intArrayOf(
                EGL14.EGL_SURFACE_TYPE, EGL14.EGL_WINDOW_BIT,
                EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES2_BIT,
                EGL14.EGL_RED_SIZE, 8,
                EGL14.EGL_GREEN_SIZE, 8,
                EGL14.EGL_BLUE_SIZE, 8,
                EGL14.EGL_ALPHA_SIZE, 8,
                EGL14.EGL_DEPTH_SIZE, 24,
                EGL14.EGL_NONE
            ),
            // Fallback 3: ES2 renderable bit + 16-bit depth
            intArrayOf(
                EGL14.EGL_SURFACE_TYPE, EGL14.EGL_WINDOW_BIT,
                EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES2_BIT,
                EGL14.EGL_RED_SIZE, 8,
                EGL14.EGL_GREEN_SIZE, 8,
                EGL14.EGL_BLUE_SIZE, 8,
                EGL14.EGL_ALPHA_SIZE, 8,
                EGL14.EGL_DEPTH_SIZE, 16,
                EGL14.EGL_NONE
            ),
            // Fallback 4: ES2 renderable bit without depth buffer
            intArrayOf(
                EGL14.EGL_SURFACE_TYPE, EGL14.EGL_WINDOW_BIT,
                EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES2_BIT,
                EGL14.EGL_RED_SIZE, 8,
                EGL14.EGL_GREEN_SIZE, 8,
                EGL14.EGL_BLUE_SIZE, 8,
                EGL14.EGL_ALPHA_SIZE, 8,
                EGL14.EGL_NONE
            )
        )

        val configs = arrayOfNulls<EGLConfig>(1)
        val numConfigs = IntArray(1)
        for (attribs in candidates) {
            if (EGL14.eglChooseConfig(display, attribs, 0, configs, 0, 1, numConfigs, 0) &&
                numConfigs[0] > 0 && configs[0] != null
            ) {
                return configs[0]
            }
        }
        return null
    }
}
