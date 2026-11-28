mod ai_mascots;
#[cfg(any(target_os = "android", target_os = "linux"))]
mod android_jni;
mod ascii_cat_knight;
mod ascii_lab;
mod audio_reactive;
mod aurora;
mod beach;
mod cat_shore;
mod claymorph;
mod cosmic_eye;
mod custom_glsl;
mod custom_plugin;
mod desert;
mod eye_within_words;
mod gl;
mod glade;
mod icon_islands;
mod istanbul;
mod liquidglass;
mod meadow;
mod neobrutalism;
mod neumorph;
mod papercut;
mod persephone;
mod pocket;
mod polarnight;
mod quiet_snow;
mod renderer;
mod rift;
mod roach;
mod rosebrook;
mod shaders;
mod software_world;
mod solar_system;
mod state;
mod strike;
mod studio_glass;
mod text;
mod themes;
mod thinking_orbs;
mod turkish_flag;
mod zen_archipelago;
mod voxelcraft;
mod winter;

use gl::{DefaultFboGuard, GlFns};
#[cfg(not(target_os = "windows"))]
use gl::EglHeadlessContext;
#[cfg(target_os = "windows")]
use gl::WglContext;
use renderer::Renderer;
use state::AppState;
use std::ffi::{c_char, c_void, CStr};
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
#[cfg(target_os = "windows")]
use std::sync::Arc;
#[cfg(target_os = "windows")]
use std::thread::{self, JoinHandle};
#[cfg(target_os = "windows")]
use std::time::{Duration, Instant};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::HWND;

pub const NATIVE_GL_THEMES: &[&str] = &[
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
    "ascii-clownfish",
    "ascii-jellyfish",
    "ascii-shark",
    "ascii-lotus",
    "ascii-cat-knight",
    "turkish-flag",
];

fn apply_low_mode_to_state(st: &mut AppState, is_low: bool) {
    st.low_mode = is_low;
    st.quiet_snow.set_low_mode(is_low);
    st.claymorph.set_low_mode(is_low);
    st.liquidglass.set_low_mode(is_low);
    st.neobrutalism.set_low_mode(is_low);
    st.solar_system.set_low_mode(is_low);
    st.thinking_orbs.set_low_mode(is_low);
    st.ai_mascots.set_low_mode(is_low);
    st.cat_shore.set_low_mode(is_low);
    st.persephone.set_low_mode(is_low);
    st.eye_within_words.set_low_mode(is_low);
    st.cosmic_eye.set_low_mode(is_low);
    st.ascii_clownfish.set_low_mode(is_low);
    st.ascii_jellyfish.set_low_mode(is_low);
    st.ascii_shark.set_low_mode(is_low);
    st.ascii_lotus.set_low_mode(is_low);
    st.ascii_cat_knight.set_low_mode(is_low);
    st.turkish_flag.set_low_mode(is_low);
}

fn flip_rgba_vertical(src: &[u8], dst: &mut [u8], width: usize, height: usize) {
    let row_bytes = width * 4;
    for y in 0..height {
        let src_off = (height - 1 - y) * row_bytes;
        let dst_off = y * row_bytes;
        dst[dst_off..dst_off + row_bytes].copy_from_slice(&src[src_off..src_off + row_bytes]);
    }
}

unsafe fn restore_host_gl_state(gl: &GlFns, target_fbo: gl::GLuint) {
    (gl.Disable)(gl::GL_SCISSOR_TEST);
    (gl.Disable)(gl::GL_DEPTH_TEST);
    (gl.Disable)(gl::GL_CULL_FACE);
    (gl.Disable)(gl::GL_BLEND);
    (gl.DepthMask)(gl::GL_TRUE);
    (gl.BindVertexArray)(0);
    (gl.BindBuffer)(gl::GL_ARRAY_BUFFER, 0);
    (gl.UseProgram)(0);
    (gl.ActiveTexture)(gl::GL_TEXTURE0);
    (gl.BindTexture)(gl::GL_TEXTURE_2D, 0);
    (gl.PixelStorei)(gl::GL_UNPACK_ALIGNMENT, 4);
    (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, target_fbo);
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_is_native_theme(theme_id: *const c_char) -> i32 {
    if theme_id.is_null() {
        return 0;
    }
    let Ok(id) = CStr::from_ptr(theme_id).to_str() else {
        return 0;
    };
    let clean = id.trim();
    if NATIVE_GL_THEMES.iter().any(|t| t.eq_ignore_ascii_case(clean)) {
        1
    } else {
        0
    }
}

#[cfg(target_os = "windows")]
pub struct GlEngine {
    state: Arc<Mutex<AppState>>,
    stop_flag: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn ddgl_create(
    hwnd: isize,
    width: i32,
    height: i32,
    dpi_scale: f32,
) -> *mut GlEngine {
    if hwnd == 0 {
        return std::ptr::null_mut();
    }
    let state = Arc::new(Mutex::new(AppState::new(width, height, dpi_scale)));
    let stop_flag = Arc::new(AtomicBool::new(false));

    let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let st_bg = Arc::clone(&state);
    let stop_bg = Arc::clone(&stop_flag);

    let worker = thread::spawn(move || {
        let hwnd_ptr = hwnd as HWND;
        let ctx = match WglContext::create(hwnd_ptr) {
            Ok(c) => c,
            Err(e) => {
                let _ = init_tx.send(Err(e));
                return;
            }
        };
        let gl = match GlFns::load() {
            Ok(g) => g,
            Err(e) => {
                let _ = init_tx.send(Err(e));
                return;
            }
        };
        let mut renderer = match Renderer::new(&gl) {
            Ok(r) => r,
            Err(e) => {
                let _ = init_tx.send(Err(e));
                return;
            }
        };

        let _ = init_tx.send(Ok(()));

        let mut last = Instant::now();
        while !stop_bg.load(Ordering::Relaxed) {
            let (paused, low_mode) = {
                if let Ok(mut guard) = st_bg.lock() {
                    let now = Instant::now();
                    let dt = now.duration_since(last).as_secs_f32().min(0.08);
                    last = now;
                    if !guard.paused {
                        guard.step(dt);
                        if ctx.make_current() {
                            renderer.render(&gl, &mut guard);
                            ctx.swap();
                        }
                    }
                    (guard.paused, guard.low_mode)
                } else {
                    (true, true)
                }
            };

            let sleep_ms = if paused { 120 } else if low_mode { 62 } else { 28 };
            thread::sleep(Duration::from_millis(sleep_ms));
        }

        if ctx.make_current() {
            renderer.destroy(&gl);
        }
    });

    match init_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => Box::into_raw(Box::new(GlEngine {
            state,
            stop_flag,
            worker: Some(worker),
        })),
        _ => {
            stop_flag.store(true, Ordering::Relaxed);
            let _ = worker.join();
            std::ptr::null_mut()
        }
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn ddgl_destroy(engine: *mut GlEngine) {
    if engine.is_null() {
        return;
    }
    let mut eng = Box::from_raw(engine);
    eng.stop_flag.store(true, Ordering::Relaxed);
    if let Some(h) = eng.worker.take() {
        let _ = h.join();
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn ddgl_resize(engine: *mut GlEngine, width: i32, height: i32, dpi_scale: f32) {
    if engine.is_null() { return; }
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        st.width_px = width.max(64);
        st.height_px = height.max(64);
        st.dpi_scale = dpi_scale.max(0.5);
        st.recompute_layout();
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn ddgl_set_paused(engine: *mut GlEngine, paused: i32) {
    if engine.is_null() { return; }
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        st.paused = paused != 0;
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn ddgl_set_low_mode(engine: *mut GlEngine, low: i32) {
    if engine.is_null() { return; }
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        apply_low_mode_to_state(&mut st, low != 0);
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn ddgl_post_json(engine: *mut GlEngine, utf8_ptr: *const c_char) {
    if engine.is_null() || utf8_ptr.is_null() { return; }
    let Ok(text) = CStr::from_ptr(utf8_ptr).to_str() else { return };
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        st.handle_json(text);
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn ddgl_mouse(engine: *mut GlEngine, kind: i32, css_x: f32, css_y: f32) -> i32 {
    if engine.is_null() { return 0; }
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        return if st.handle_mouse(kind, css_x, css_y) { 1 } else { 0 };
    }
    0
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn ddgl_key(engine: *mut GlEngine, kind: i32, vk: i32, utf8_ptr: *const c_char) {
    if engine.is_null() { return; }
    let text = if utf8_ptr.is_null() {
        ""
    } else {
        CStr::from_ptr(utf8_ptr).to_str().unwrap_or("")
    };
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        st.handle_key(kind, vk, text);
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn ddgl_poll_event(
    engine: *mut GlEngine,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    if engine.is_null() || buf.is_null() || buf_len <= 1 { return 0; }
    let eng = &*engine;
    let msg = {
        let Ok(mut st) = eng.state.lock() else { return 0 };
        st.outbox.pop_front()
    };
    let Some(s) = msg else { return 0 };
    let bytes = s.as_bytes();
    let copy_len = bytes.len().min((buf_len - 1) as usize);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
    *buf.add(copy_len) = 0;
    copy_len as i32
}

// ---------------------------------------------------------------------------
// Cross-platform Embedded OpenGL 3.3 Core Engine API (Gtk.GLArea, Qt6 FBO, etc.)
// Caller owns the OpenGL 3.3 Core context and keeps it current during create,
// render, read/png, and destroy calls.
// ---------------------------------------------------------------------------

pub struct GlEmbeddedEngine {
    gl: GlFns,
    renderer: Renderer,
    state: AppState,
    destroyed_gl: bool,
}

pub(crate) unsafe fn create_embedded_engine_checked(
    width: i32,
    height: i32,
    dpi_scale: f32,
) -> Result<*mut GlEmbeddedEngine, String> {
    let gl = GlFns::load()?;
    let renderer = Renderer::new(&gl)?;
    let state = AppState::new(width, height, dpi_scale);
    Ok(Box::into_raw(Box::new(GlEmbeddedEngine {
        gl,
        renderer,
        state,
        destroyed_gl: false,
    })))
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_create(
    width: i32,
    height: i32,
    dpi_scale: f32,
) -> *mut GlEmbeddedEngine {
    create_embedded_engine_checked(width, height, dpi_scale).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_destroy(engine: *mut GlEmbeddedEngine) {
    if engine.is_null() {
        return;
    }
    let mut eng = Box::from_raw(engine);
    if !eng.destroyed_gl {
        eng.renderer.destroy(&eng.gl);
        eng.destroyed_gl = true;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_resize(
    engine: *mut GlEmbeddedEngine,
    width: i32,
    height: i32,
    dpi_scale: f32,
) {
    if engine.is_null() {
        return;
    }
    let eng = &mut *engine;
    eng.state.width_px = width.max(64);
    eng.state.height_px = height.max(64);
    eng.state.dpi_scale = dpi_scale.max(0.5);
    eng.state.recompute_layout();
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_set_paused(engine: *mut GlEmbeddedEngine, paused: i32) {
    if engine.is_null() {
        return;
    }
    (&mut *engine).state.paused = paused != 0;
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_set_low_mode(engine: *mut GlEmbeddedEngine, low: i32) {
    if engine.is_null() {
        return;
    }
    apply_low_mode_to_state(&mut (&mut *engine).state, low != 0);
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_post_json(
    engine: *mut GlEmbeddedEngine,
    utf8_ptr: *const c_char,
) {
    if engine.is_null() || utf8_ptr.is_null() {
        return;
    }
    let Ok(text) = CStr::from_ptr(utf8_ptr).to_str() else {
        return;
    };
    (&mut *engine).state.handle_json(text);
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_mouse(
    engine: *mut GlEmbeddedEngine,
    kind: i32,
    css_x: f32,
    css_y: f32,
) -> i32 {
    if engine.is_null() {
        return 0;
    }
    if (&mut *engine).state.handle_mouse(kind, css_x, css_y) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_key(
    engine: *mut GlEmbeddedEngine,
    kind: i32,
    vk: i32,
    utf8_ptr: *const c_char,
) {
    if engine.is_null() {
        return;
    }
    let text = if utf8_ptr.is_null() {
        ""
    } else {
        CStr::from_ptr(utf8_ptr).to_str().unwrap_or("")
    };
    (&mut *engine).state.handle_key(kind, vk, text);
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_poll_event(
    engine: *mut GlEmbeddedEngine,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    if engine.is_null() || buf.is_null() || buf_len <= 1 {
        return 0;
    }
    let Some(s) = (&mut *engine).state.outbox.pop_front() else {
        return 0;
    };
    let bytes = s.as_bytes();
    let copy_len = bytes.len().min((buf_len - 1) as usize);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
    *buf.add(copy_len) = 0;
    copy_len as i32
}

/// Renders one frame into `target_fbo` (pass `u32::MAX` to auto-detect the currently bound FBO).
/// Steps simulation by `dt` seconds when not paused and `dt > 0`.
#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_render(
    engine: *mut GlEmbeddedEngine,
    dt: f32,
    target_fbo: u32,
) -> i32 {
    if engine.is_null() {
        return 0;
    }
    let eng = &mut *engine;
    let resolved_fbo = if target_fbo == u32::MAX {
        let mut cur: gl::GLint = 0;
        (eng.gl.GetIntegerv)(gl::GL_FRAMEBUFFER_BINDING, &mut cur);
        cur.max(0) as gl::GLuint
    } else {
        target_fbo
    };

    if !eng.state.paused && dt > 0.0 {
        eng.state.step(dt.clamp(0.0, 0.08));
    }

    let _guard = DefaultFboGuard::new(resolved_fbo);
    (eng.gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
    // Reset host-inherited GL state (e.g. Gtk.GLArea enables GL_DEPTH_TEST when a
    // depth buffer is attached, which would discard 2D themes that don't toggle depth).
    (eng.gl.Disable)(gl::GL_DEPTH_TEST);
    (eng.gl.Disable)(gl::GL_SCISSOR_TEST);
    (eng.gl.Disable)(gl::GL_CULL_FACE);
    (eng.gl.Disable)(gl::GL_BLEND);
    (eng.gl.DepthMask)(gl::GL_TRUE);
    eng.renderer.render(&eng.gl, &mut eng.state);
    drop(_guard);
    restore_host_gl_state(&eng.gl, resolved_fbo);
    1
}

/// Reads top-down RGBA8 pixels from `target_fbo` (`u32::MAX` = currently bound FBO).
/// Returns number of bytes written (`w * h * 4`), or 0 on error.
#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_read_rgba(
    engine: *mut GlEmbeddedEngine,
    target_fbo: u32,
    out_rgba: *mut u8,
    buf_len: i32,
) -> i32 {
    if engine.is_null() || out_rgba.is_null() || buf_len <= 0 {
        return 0;
    }
    let eng = &mut *engine;
    let w = eng.state.width_px.max(1) as usize;
    let h = eng.state.height_px.max(1) as usize;
    let needed = w.saturating_mul(h).saturating_mul(4);
    if (buf_len as usize) < needed {
        return 0;
    }
    let resolved_fbo = if target_fbo == u32::MAX {
        let mut cur: gl::GLint = 0;
        (eng.gl.GetIntegerv)(gl::GL_FRAMEBUFFER_BINDING, &mut cur);
        cur.max(0) as gl::GLuint
    } else {
        target_fbo
    };
    let _guard = DefaultFboGuard::new(resolved_fbo);
    (eng.gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
    (eng.gl.Finish)();
    let mut raw = vec![0u8; needed];
    (eng.gl.ReadPixels)(
        0,
        0,
        w as gl::GLsizei,
        h as gl::GLsizei,
        gl::GL_RGBA,
        gl::GL_UNSIGNED_BYTE,
        raw.as_mut_ptr() as *mut c_void,
    );
    drop(_guard);
    let dst = std::slice::from_raw_parts_mut(out_rgba, needed);
    flip_rgba_vertical(&raw, dst, w, h);
    needed as i32
}

/// Captures the current `target_fbo` (`u32::MAX` = currently bound FBO) and writes a PNG file.
/// Returns 1 on success, 0 on failure.
#[no_mangle]
pub unsafe extern "C" fn ddgl_embedded_render_png(
    engine: *mut GlEmbeddedEngine,
    target_fbo: u32,
    out_path: *const c_char,
) -> i32 {
    if engine.is_null() || out_path.is_null() {
        return 0;
    }
    let Ok(path) = CStr::from_ptr(out_path).to_str() else {
        return 0;
    };
    let eng = &mut *engine;
    let w = eng.state.width_px.max(1) as usize;
    let h = eng.state.height_px.max(1) as usize;
    let mut rgba = vec![0u8; w * h * 4];
    let n = ddgl_embedded_read_rgba(engine, target_fbo, rgba.as_mut_ptr(), rgba.len() as i32);
    if n <= 0 {
        return 0;
    }
    if image::save_buffer(path, &rgba, w as u32, h as u32, image::ColorType::Rgba8).is_ok() {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Linux Headless EGL Offscreen Engine API
// Used by Linux static fallback, offscreen snapshots, Qt6 non-GL RHI fallback,
// and Linux GPU unit tests.
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
struct HeadlessInner {
    ctx: EglHeadlessContext,
    gl: GlFns,
    renderer: Renderer,
    state: AppState,
    fbo: gl::GLuint,
    color_tex: gl::GLuint,
    depth_rb: gl::GLuint,
    fbo_w: i32,
    fbo_h: i32,
    scratch: Vec<u8>,
}

#[cfg(not(target_os = "windows"))]
impl HeadlessInner {
    unsafe fn ensure_fbo(&mut self, w: i32, h: i32) -> bool {
        let w = w.max(64);
        let h = h.max(64);
        if self.fbo != 0 && self.fbo_w == w && self.fbo_h == h {
            return true;
        }
        self.destroy_fbo();

        let mut fbo = 0;
        let mut tex = 0;
        let mut depth_rb = 0;
        (self.gl.GenFramebuffers)(1, &mut fbo);
        (self.gl.GenTextures)(1, &mut tex);
        (self.gl.BindTexture)(gl::GL_TEXTURE_2D, tex);
        (self.gl.TexImage2D)(
            gl::GL_TEXTURE_2D,
            0,
            gl::GL_RGBA8 as gl::GLint,
            w,
            h,
            0,
            gl::GL_RGBA,
            gl::GL_UNSIGNED_BYTE,
            std::ptr::null(),
        );
        (self.gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MIN_FILTER, gl::GL_LINEAR);
        (self.gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MAG_FILTER, gl::GL_LINEAR);
        (self.gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_S, gl::GL_CLAMP_TO_EDGE);
        (self.gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_T, gl::GL_CLAMP_TO_EDGE);

        (self.gl.GenRenderbuffers)(1, &mut depth_rb);
        (self.gl.BindRenderbuffer)(gl::GL_RENDERBUFFER, depth_rb);
        (self.gl.RenderbufferStorage)(gl::GL_RENDERBUFFER, gl::GL_DEPTH_COMPONENT24, w, h);

        let _zero = DefaultFboGuard::new(0);
        (self.gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, fbo);
        (self.gl.FramebufferTexture2D)(
            gl::GL_FRAMEBUFFER,
            gl::GL_COLOR_ATTACHMENT0,
            gl::GL_TEXTURE_2D,
            tex,
            0,
        );
        (self.gl.FramebufferRenderbuffer)(
            gl::GL_FRAMEBUFFER,
            gl::GL_DEPTH_ATTACHMENT,
            gl::GL_RENDERBUFFER,
            depth_rb,
        );
        let status = (self.gl.CheckFramebufferStatus)(gl::GL_FRAMEBUFFER);
        (self.gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);

        if status != gl::GL_FRAMEBUFFER_COMPLETE {
            (self.gl.DeleteRenderbuffers)(1, &depth_rb);
            (self.gl.DeleteTextures)(1, &tex);
            (self.gl.DeleteFramebuffers)(1, &fbo);
            return false;
        }
        self.fbo = fbo;
        self.color_tex = tex;
        self.depth_rb = depth_rb;
        self.fbo_w = w;
        self.fbo_h = h;
        true
    }

    unsafe fn destroy_fbo(&mut self) {
        if self.depth_rb != 0 {
            (self.gl.DeleteRenderbuffers)(1, &self.depth_rb);
            self.depth_rb = 0;
        }
        if self.color_tex != 0 {
            (self.gl.DeleteTextures)(1, &self.color_tex);
            self.color_tex = 0;
        }
        if self.fbo != 0 {
            (self.gl.DeleteFramebuffers)(1, &self.fbo);
            self.fbo = 0;
        }
        self.fbo_w = 0;
        self.fbo_h = 0;
    }

    unsafe fn render_to_scratch(&mut self, dt: f32) -> Option<(usize, usize)> {
        if !self.ctx.make_current() {
            return None;
        }
        let w = self.state.width_px.max(64);
        let h = self.state.height_px.max(64);
        if !self.ensure_fbo(w, h) {
            self.ctx.release_current();
            return None;
        }
        if !self.state.paused && dt > 0.0 {
            self.state.step(dt.clamp(0.0, 0.08));
        }
        let _guard = DefaultFboGuard::new(self.fbo);
        (self.gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
        self.renderer.render(&self.gl, &mut self.state);
        (self.gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
        (self.gl.Finish)();

        let needed = (w as usize) * (h as usize) * 4;
        if self.scratch.len() != needed {
            self.scratch.resize(needed, 0);
        }
        (self.gl.ReadPixels)(
            0,
            0,
            w,
            h,
            gl::GL_RGBA,
            gl::GL_UNSIGNED_BYTE,
            self.scratch.as_mut_ptr() as *mut c_void,
        );
        drop(_guard);
        self.ctx.release_current();
        Some((w as usize, h as usize))
    }
}

#[cfg(not(target_os = "windows"))]
pub struct GlHeadlessEngine {
    inner: Mutex<HeadlessInner>,
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_create(
    width: i32,
    height: i32,
    dpi_scale: f32,
) -> *mut GlHeadlessEngine {
    let ctx = match EglHeadlessContext::create() {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };
    if !ctx.make_current() {
        return std::ptr::null_mut();
    }
    let gl = match GlFns::load() {
        Ok(g) => g,
        Err(_) => {
            ctx.release_current();
            return std::ptr::null_mut();
        }
    };
    let renderer = match Renderer::new(&gl) {
        Ok(r) => r,
        Err(_) => {
            ctx.release_current();
            return std::ptr::null_mut();
        }
    };
    let mut state = AppState::new(width, height, dpi_scale);
    state.sync_bake = true;
    let mut inner = HeadlessInner {
        ctx,
        gl,
        renderer,
        state,
        fbo: 0,
        color_tex: 0,
        depth_rb: 0,
        fbo_w: 0,
        fbo_h: 0,
        scratch: Vec::new(),
    };
    let ok = inner.ensure_fbo(width, height);
    inner.ctx.release_current();
    if !ok {
        if inner.ctx.make_current() {
            inner.destroy_fbo();
            inner.renderer.destroy(&inner.gl);
            inner.ctx.release_current();
        }
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(GlHeadlessEngine {
        inner: Mutex::new(inner),
    }))
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_destroy(engine: *mut GlHeadlessEngine) {
    if engine.is_null() {
        return;
    }
    let eng = Box::from_raw(engine);
    if let Ok(mut inner) = eng.inner.into_inner() {
        if inner.ctx.make_current() {
            inner.destroy_fbo();
            let HeadlessInner { ref gl, ref mut renderer, ref ctx, .. } = inner;
            renderer.destroy(gl);
            ctx.release_current();
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_resize(
    engine: *mut GlHeadlessEngine,
    width: i32,
    height: i32,
    dpi_scale: f32,
) {
    if engine.is_null() {
        return;
    }
    if let Ok(mut inner) = (&*engine).inner.lock() {
        inner.state.width_px = width.max(64);
        inner.state.height_px = height.max(64);
        inner.state.dpi_scale = dpi_scale.max(0.5);
        inner.state.recompute_layout();
    }
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_set_paused(engine: *mut GlHeadlessEngine, paused: i32) {
    if engine.is_null() {
        return;
    }
    if let Ok(mut inner) = (&*engine).inner.lock() {
        inner.state.paused = paused != 0;
    }
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_set_low_mode(engine: *mut GlHeadlessEngine, low: i32) {
    if engine.is_null() {
        return;
    }
    if let Ok(mut inner) = (&*engine).inner.lock() {
        apply_low_mode_to_state(&mut inner.state, low != 0);
    }
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_post_json(
    engine: *mut GlHeadlessEngine,
    utf8_ptr: *const c_char,
) {
    if engine.is_null() || utf8_ptr.is_null() {
        return;
    }
    let Ok(text) = CStr::from_ptr(utf8_ptr).to_str() else {
        return;
    };
    if let Ok(mut inner) = (&*engine).inner.lock() {
        inner.state.handle_json(text);
    }
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_mouse(
    engine: *mut GlHeadlessEngine,
    kind: i32,
    css_x: f32,
    css_y: f32,
) -> i32 {
    if engine.is_null() {
        return 0;
    }
    if let Ok(mut inner) = (&*engine).inner.lock() {
        return if inner.state.handle_mouse(kind, css_x, css_y) { 1 } else { 0 };
    }
    0
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_key(
    engine: *mut GlHeadlessEngine,
    kind: i32,
    vk: i32,
    utf8_ptr: *const c_char,
) {
    if engine.is_null() {
        return;
    }
    let text = if utf8_ptr.is_null() {
        ""
    } else {
        CStr::from_ptr(utf8_ptr).to_str().unwrap_or("")
    };
    if let Ok(mut inner) = (&*engine).inner.lock() {
        inner.state.handle_key(kind, vk, text);
    }
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_poll_event(
    engine: *mut GlHeadlessEngine,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    if engine.is_null() || buf.is_null() || buf_len <= 1 {
        return 0;
    }
    let msg = {
        let Ok(mut inner) = (&*engine).inner.lock() else {
            return 0;
        };
        inner.state.outbox.pop_front()
    };
    let Some(s) = msg else { return 0 };
    let bytes = s.as_bytes();
    let copy_len = bytes.len().min((buf_len - 1) as usize);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
    *buf.add(copy_len) = 0;
    copy_len as i32
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_render_rgba(
    engine: *mut GlHeadlessEngine,
    dt: f32,
    out_rgba: *mut u8,
    buf_len: i32,
) -> i32 {
    if engine.is_null() || out_rgba.is_null() || buf_len <= 0 {
        return 0;
    }
    let Ok(mut inner) = (&*engine).inner.lock() else {
        return 0;
    };
    let Some((w, h)) = inner.render_to_scratch(dt) else {
        return 0;
    };
    let needed = w * h * 4;
    if (buf_len as usize) < needed {
        return 0;
    }
    let dst = std::slice::from_raw_parts_mut(out_rgba, needed);
    flip_rgba_vertical(&inner.scratch, dst, w, h);
    needed as i32
}

#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn ddgl_headless_render_png(
    engine: *mut GlHeadlessEngine,
    dt: f32,
    out_path: *const c_char,
) -> i32 {
    if engine.is_null() || out_path.is_null() {
        return 0;
    }
    let Ok(path) = CStr::from_ptr(out_path).to_str() else {
        return 0;
    };
    let Ok(mut inner) = (&*engine).inner.lock() else {
        return 0;
    };
    let Some((w, h)) = inner.render_to_scratch(dt) else {
        return 0;
    };
    let mut flipped = vec![0u8; w * h * 4];
    flip_rgba_vertical(&inner.scratch, &mut flipped, w, h);
    if image::save_buffer(path, &flipped, w as u32, h as u32, image::ColorType::Rgba8).is_ok() {
        1
    } else {
        0
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::shaders::{
        BACKDROP_FRAG, BLIT_FRAG, BLUR_FRAG, FULLSCREEN_VERT, SDF_UI_FRAG, SDF_UI_VERT, TEXT_FRAG,
        TEXT_VERT,
    };
    use crate::themes::resolve_shader_and_params;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExA, DefWindowProcA, DestroyWindow, RegisterClassA, WNDCLASSA, WS_POPUP,
    };

    const ALL_SHADER_THEMES: &[&str] = &[
        "liquidglass", "thinking-orbs", "solar-system", "studio-glass",
        "beach", "meadow", "desert", "rosebrook", "glade", "zen-archipelago",
        "aurora", "polarnight", "quietsnow", "winter",
        "neumorph", "claymorph", "neobrutalism", "voxelcraft", "papercut", "istanbul",
        "software-world", "rift", "strike", "pocket", "roach",
    ];

    #[test]
    fn verify_all_25_glsl_shaders_compile_on_gpu() {
        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarGlShaderTest\0";
            let wc = WNDCLASSA {
                style: 0x0020, // CS_OWNDC
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassA(&wc);
            let hwnd = CreateWindowExA(
                0, cls.as_ptr(), b"t\0".as_ptr(), WS_POPUP,
                0, 0, 64, 64, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
            );
            assert!(!hwnd.is_null(), "Failed to create dummy test HWND");
            let ctx = WglContext::create(hwnd).expect("WGL context failed");
            assert!(ctx.make_current());
            let gl = GlFns::load().expect("OpenGL load failed");

            // Verify core Impulse shaders (Backdrop, Blur, Blit, SDF UI + Vector Icons, Text)
            for (name, vert, frag) in [
                ("backdrop", FULLSCREEN_VERT, BACKDROP_FRAG),
                ("blur", FULLSCREEN_VERT, BLUR_FRAG),
                ("blit", FULLSCREEN_VERT, BLIT_FRAG),
                ("sdf_ui", SDF_UI_VERT, SDF_UI_FRAG),
                ("text", TEXT_VERT, TEXT_FRAG),
            ] {
                let p = gl.compile_program(vert, frag)
                    .unwrap_or_else(|e| panic!("Core shader '{name}' GLSL failed: {e}"));
                (gl.DeleteProgram)(p);
            }

            // Verify Studio Glass 3D PBR + Multi-Pass Refractive Glass + HUD Icons shaders on GPU!
            for (name, vert, frag) in [
                ("sg_pbr", studio_glass::shaders::PBR_VERT, studio_glass::shaders::PBR_FRAG),
                ("sg_window", studio_glass::shaders::WINDOW_VERT, studio_glass::shaders::WINDOW_FRAG),
                ("sg_monitor", studio_glass::shaders::WINDOW_VERT, studio_glass::shaders::MONITOR_FRAG),
                ("sg_shaft", studio_glass::shaders::WINDOW_VERT, studio_glass::shaders::SHAFT_FRAG),
                ("sg_dust", studio_glass::shaders::DUST_VERT, studio_glass::shaders::DUST_FRAG),
                ("sg_blur", FULLSCREEN_VERT, studio_glass::shaders::POST_BLUR_FRAG),
                ("sg_glass", FULLSCREEN_VERT, studio_glass::shaders::POST_GLASS_FRAG),
                ("sg_icon", studio_glass::shaders::HUD_ICON_VERT, studio_glass::shaders::HUD_ICON_FRAG),
                ("qs_radial", FULLSCREEN_VERT, quiet_snow::shaders::QS_RADIAL_FRAG),
                ("qs_color", quiet_snow::shaders::QS_COLOR_VERT, quiet_snow::shaders::QS_COLOR_FRAG),
                ("qs_sprite", quiet_snow::shaders::QS_SPRITE_VERT, quiet_snow::shaders::QS_SPRITE_FRAG),
                ("pc_color", papercut::shaders::PC_COLOR_VERT, papercut::shaders::PC_COLOR_FRAG),
                ("vc_color", voxelcraft::shaders::VC_COLOR_VERT, voxelcraft::shaders::VC_COLOR_FRAG),
                ("vc_sprite", voxelcraft::shaders::VC_SPRITE_VERT, voxelcraft::shaders::VC_SPRITE_FRAG),
                ("ds_bg", desert::shaders::DS_BG_VERT, desert::shaders::DS_BG_FRAG),
                ("ds_sprite", desert::shaders::DS_SPRITE_VERT, desert::shaders::DS_SPRITE_FRAG),
                ("pc_paper", FULLSCREEN_VERT, papercut::shaders::PC_PAPER_APPLY_FRAG),
                ("pc_niche_hole", FULLSCREEN_VERT, papercut::shaders::PC_NICHE_HOLE_FRAG),
                ("pc_post", FULLSCREEN_VERT, papercut::shaders::PC_POST_GRAIN_VIG_FRAG),
                ("pc_sprite", papercut::shaders::PC_SPRITE_VERT, papercut::shaders::PC_SPRITE_FRAG),
                ("cm_color", claymorph::shaders::CM_COLOR_VERT, claymorph::shaders::CM_COLOR_FRAG),
                ("cm_clay", claymorph::shaders::CM_CLAY_VERT, claymorph::shaders::CM_CLAY_FRAG),
                ("nb_color", neobrutalism::shaders::NB_COLOR_VERT, neobrutalism::shaders::NB_COLOR_FRAG),
                ("nb_bg", FULLSCREEN_VERT, neobrutalism::shaders::NB_BG_FRAG),
                ("sw_color", software_world::shaders::SW_COLOR_VERT, software_world::shaders::SW_COLOR_FRAG),
                ("meadow_boy", meadow::shaders::MEADOW_VERT, meadow::shaders::MEADOW_BOY_FRAG),
                ("aurora_main", aurora::shaders::AURORA_VERT, aurora::shaders::AURORA_FRAG),
                ("aurora_sprite", aurora::shaders::AURORA_SPRITE_VERT, aurora::shaders::AURORA_SPRITE_FRAG),
                ("aurora_snow", aurora::shaders::AURORA_SNOW_VERT, aurora::shaders::AURORA_SNOW_FRAG),
                ("glade_ground", glade::shaders::GLADE_VERT, glade::shaders::GLADE_FRAG),
                ("glade_inst", glade::shaders::GLADE_INST_VERT, glade::shaders::GLADE_INST_FRAG),
                ("rift_ground", rift::shaders::RIFT_VERT, rift::shaders::RIFT_FRAG),
                ("rift_inst", rift::shaders::RIFT_INST_VERT, rift::shaders::RIFT_INST_FRAG),
                ("strike_ground", strike::shaders::STRIKE_VERT, strike::shaders::STRIKE_FRAG),
                ("strike_inst", strike::shaders::STRIKE_INST_VERT, strike::shaders::STRIKE_INST_FRAG),
                ("pocket_ground", pocket::shaders::POCKET_VERT, pocket::shaders::POCKET_FRAG),
                ("pocket_inst", pocket::shaders::POCKET_INST_VERT, pocket::shaders::POCKET_INST_FRAG),
                ("roach_ground", roach::shaders::ROACH_BG_VS, roach::shaders::ROACH_BG_FS),
                ("roach_inst", roach::shaders::ROACH_INST_VS, roach::shaders::ROACH_INST_FS),
                ("rosebrook_ground", rosebrook::shaders::ROSEBROOK_VERT, rosebrook::shaders::ROSEBROOK_FRAG),
                ("rosebrook_inst", rosebrook::shaders::ROSEBROOK_INST_VERT, rosebrook::shaders::ROSEBROOK_INST_FRAG),
                ("pn_col", polarnight::shaders::PN_COLOR_VS, polarnight::shaders::PN_COLOR_FS),
                ("pn_tex", polarnight::shaders::PN_TEX_VS, polarnight::shaders::PN_TEX_FS),
                ("winter_color", winter::shaders::WINTER_COLOR_VERT, winter::shaders::WINTER_COLOR_FRAG),
                ("neumorph_sprite", neumorph::shaders::NEU_SPRITE_VERT, neumorph::shaders::NEU_SPRITE_FRAG),
                ("neumorph_color", neumorph::shaders::NEU_COLOR_VERT, neumorph::shaders::NEU_COLOR_FRAG),
                ("ist_tex", istanbul::shaders::IST_TEX_VS, istanbul::shaders::IST_TEX_FS),
                ("ist_water", istanbul::shaders::IST_WATER_VS, istanbul::shaders::IST_WATER_FS),
                ("ist_color", istanbul::shaders::IST_COLOR_VS, istanbul::shaders::IST_COLOR_FS),
                ("ist_pat_rect", istanbul::shaders::IST_PATTERN_RECT_VS, istanbul::shaders::IST_PATTERN_RECT_FS),
                ("ist_post", istanbul::shaders::IST_POST_VS, istanbul::shaders::IST_POST_FS),
                ("ss_bg", solar_system::shaders::SS_BG_VERT, solar_system::shaders::SS_BG_FRAG),
                ("ss_tex", solar_system::shaders::SS_TEX_VERT, solar_system::shaders::SS_TEX_FRAG),
                ("ss_col", solar_system::shaders::SS_COLOR_VERT, solar_system::shaders::SS_COLOR_FS),
                ("ss_disc", solar_system::shaders::SS_DISC_VERT, solar_system::shaders::SS_DISC_FRAG),
                ("to_bg", thinking_orbs::shaders::TO_BG_VERT, thinking_orbs::shaders::TO_BG_FRAG),
                ("to_tex", thinking_orbs::shaders::TO_TEX_VERT, thinking_orbs::shaders::TO_TEX_FRAG),
                ("to_prim", thinking_orbs::shaders::TO_PRIM_VERT, thinking_orbs::shaders::TO_PRIM_FRAG),
                ("am_bg", ai_mascots::shaders::AM_BG_VERT, ai_mascots::shaders::AM_BG_FRAG),
                ("am_col", ai_mascots::shaders::AM_COL_VERT, ai_mascots::shaders::AM_COL_FRAG),
                ("am_tex", ai_mascots::shaders::AM_TEX_VERT, ai_mascots::shaders::AM_TEX_FRAG),
                ("am_sdf", ai_mascots::shaders::AM_SDF_VERT, ai_mascots::shaders::AM_SDF_FRAG),
            ] {
                let p = gl.compile_program(vert, frag)
                    .unwrap_or_else(|e| panic!("Studio Glass shader '{name}' GLSL failed: {e}"));
                (gl.DeleteProgram)(p);
            }

            let mut st = AppState::new(1920, 1080, 1.0);
            for &tid in ALL_SHADER_THEMES {
                st.set_theme(tid);
                let (frag, _) = resolve_shader_and_params(&st);
                let prog = gl.compile_program(FULLSCREEN_VERT, &frag)
                    .unwrap_or_else(|e| panic!("Theme '{tid}' GLSL failed: {e}"));
                (gl.DeleteProgram)(prog);
            }

            // Verify 1:1 Quiet Snow (Sessiz Kar) GPU rendering across all 4 modes & 4 atmospheres
            let mut renderer = Renderer::new(&gl).expect("Renderer::new failed");
            st.set_theme("quietsnow");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[60,80,136,170],[60,210,136,300],[180,80,256,170]]}"#);
            st.handle_json(r#"{"t":"windows","r":[[1001,280,160,920,620],[1002,520,260,1180,720]]}"#);
            for mode in 0..4 {
                for atm in 0..4 {
                    st.handle_json(&format!(
                        r#"{{"t":"settings","v":{{"mode":{mode},"atmosphere":{atm},"accumulate":2,"windows":1,"cursor":2}}}}"#
                    ));
                    st.handle_mouse(0, 320.0, 162.0);
                    st.handle_mouse(1, 355.0, 162.0);
                    st.step(0.033);
                    renderer.render(&gl, &mut st);
                }
            }
            // Verify 1:1 Kâğıt Bahçe (papercut) GPU rendering across all 4 palettes & 4 times of day
            st.set_theme("papercut");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[60,80,136,170],[60,210,136,300],[180,80,256,170]]}"#);
            for pal in 0..4 {
                for tm in 1..=4 {
                    st.handle_json(&format!(
                        r#"{{"t":"settings","v":{{"palette":{pal},"time":{tm},"critters":2,"trees":1,"skyline":1,"frame":1}}}}"#
                    ));
                    st.step(0.033);
                    renderer.render(&gl, &mut st);
                }
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_PAPERCUT_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut fbo = 0u32;
                let mut tex = 0u32;
                (gl.GenFramebuffers)(1, &mut fbo);
                (gl.GenTextures)(1, &mut tex);
                (gl.BindTexture)(gl::GL_TEXTURE_2D, tex);
                (gl.TexImage2D)(
                    gl::GL_TEXTURE_2D, 0, gl::GL_RGBA8 as gl::GLint, dw, dh, 0,
                    gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, std::ptr::null(),
                );
                (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, fbo);
                (gl.FramebufferTexture2D)(gl::GL_FRAMEBUFFER, gl::GL_COLOR_ATTACHMENT0, gl::GL_TEXTURE_2D, tex, 0);

                // Resize test window & state to 1280x720
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("papercut");
                st2.handle_json(r#"{"t":"settings","v":{"palette":0,"time":2,"critters":1,"trees":1,"skyline":1,"frame":1,"adaptive":1,"flowers":0.6}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[51,43,127,133],[51,144,127,234],[51,245,127,335],[192,43,268,133],[192,144,268,234],[384,432,460,522]]}"#);
                for _ in 0..60 {
                    st2.step(0.033);
                }
                // Create a 1280x720 temporary window so default framebuffer 0 is 1280x720
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                // Flip vertically for PNG
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
                (gl.DeleteTextures)(1, &tex);
                (gl.DeleteFramebuffers)(1, &fbo);
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_QUIETSNOW_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("quietsnow");
                st2.handle_json(r#"{"t":"settings","v":{"mode":0,"atmosphere":0,"accumulate":2,"windows":1,"cursor":2}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[51,43,127,133],[51,144,127,234],[51,245,127,335],[192,43,268,133],[192,144,268,234]]}"#);
                for _ in 0..90 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_qs\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Claymorph (Q Harfi 3D Claymorphism) GPU rendering across palettes, cradles & qualities
            st.set_theme("claymorph");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[60,80,136,170],[60,210,136,300],[180,80,256,170],[320,430,400,520]]}"#);
            for pal in 0..4 {
                for cradle in 0..3 {
                    for q in 0..3 {
                        st.handle_json(&format!(
                            r#"{{"t":"settings","v":{{"palette":"{}","puffiness":1.0,"squishStrength":1.0,"blobCount":1.0,"floatSpeed":0.8,"cursorRepel":1.0,"iconCradle":"{}","quality":"{}","adaptive":1,"seed":20261118}}}}"#,
                            ["pastel", "candy", "matcha", "dusk"][pal],
                            ["puffy", "cloud", "ring"][cradle],
                            ["Low", "Medium", "High"][q],
                        ));
                        for _ in 0..8 {
                            st.step(0.033);
                        }
                        st.handle_mouse(1, 200.0, 200.0);
                        st.handle_mouse(2, 200.0, 200.0);
                        renderer.render(&gl, &mut st);
                    }
                }
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_CLAYMORPH_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("claymorph");
                st2.handle_json(r#"{"t":"settings","v":{"palette":"pastel","puffiness":1.0,"squishStrength":1.0,"blobCount":1.0,"floatSpeed":0.8,"cursorRepel":1.0,"iconCradle":"puffy","quality":"High","adaptive":1,"seed":20261118}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[60,80,136,170],[60,210,136,300],[180,80,256,170],[320,430,400,520]]}"#);
                for _ in 0..60 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_claymorph\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save claymorph png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Zen Archipelago GPU rendering across time-of-day modes
            st.set_theme("zen-archipelago");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[60,80,136,170],[60,210,136,300],[180,80,256,170],[320,430,400,520]]}"#);
            for tod in 0..4 {
                st.handle_json(&format!(
                    r#"{{"t":"settings","v":{{"timeOfDay":"{}","koiCount":14,"lanternCount":10,"islandMargin":44,"bridgeDistance":230,"waveIntensity":1.0,"sakuraDensity":1.0,"fireflies":45,"adaptive":1,"seed":0}}}}"#,
                    ["day", "sunset", "night", "dawn"][tod],
                ));
                for _ in 0..12 {
                    st.step(0.033);
                }
                st.handle_mouse(1, 200.0, 200.0);
                st.handle_mouse(2, 200.0, 200.0);
                renderer.render(&gl, &mut st);
            }
            // Verify 1:1 Software World GPU rendering across quality modes and adaptive states
            st.set_theme("software-world");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[48,48,124,138],[48,176,124,266],[48,304,124,394],[188,48,264,138],[188,176,264,266],[440,320,516,410]]}"#);
            for q in 0..3 {
                st.handle_json(&format!(
                    r#"{{"t":"settings","v":{{"adaptive":1,"animSpeed":1.0,"networkActivity":0.65,"connectionIntensity":0.55,"particleDensity":0.55,"codeParticleDensity":0.45,"envDetail":0.7,"quality":{q},"seed":42}}}}"#
                ));
                for _ in 0..20 {
                    st.step(0.033);
                }
                renderer.render(&gl, &mut st);
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_ZEN_ARCHIPELAGO_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("zen-archipelago");
                st2.handle_json(r#"{"t":"settings","v":{"timeOfDay":"night","koiCount":14,"lanternCount":10,"islandMargin":44,"bridgeDistance":230,"waveIntensity":1.0,"sakuraDensity":1.0,"fireflies":45,"adaptive":1,"seed":0}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[51,43,127,133],[51,144,127,234],[192,43,268,133],[192,144,268,234],[384,432,460,522]]}"#);
                for _ in 0..60 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_za\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save za png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_SW_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("software-world");
                st2.handle_json(r#"{"t":"settings","v":{"adaptive":1,"animSpeed":1.0,"networkActivity":0.80,"connectionIntensity":0.75,"particleDensity":0.65,"codeParticleDensity":0.60,"envDetail":0.85,"quality":2,"seed":42}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[60,48,136,138],[60,180,136,270],[60,312,136,402],[60,444,136,534],[200,80,276,170],[200,260,276,350],[460,310,536,400]]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_sw\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save sw png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Meadow (Çayır) GPU shaders & rendering (clouds, shadows, kite boy & black kitten)
            for &(a, c) in &[(false, false), (true, false), (false, true), (true, true)] {
                let frag = meadow::shaders::meadow_frag_source(a, c);
                let p = gl.compile_program(meadow::shaders::MEADOW_VERT, &frag)
                    .unwrap_or_else(|e| panic!("Meadow shader ({a},{c}) GLSL failed: {e}"));
                (gl.DeleteProgram)(p);
            }
            st.set_theme("meadow");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[48,48,124,138],[48,176,124,266],[48,304,124,394],[188,48,264,138],[188,176,264,266]]}"#);
            st.handle_json(r#"{"t":"settings","v":{"sigma":72,"cover":0.45,"flow":1.0,"showBoy":1,"showCat":1,"adaptive":1,"seed":42}}"#);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_MEADOW_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("meadow");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":72,"cover":0.3,"flow":1.0,"showBoy":1,"showCat":1,"adaptive":1,"seed":42}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,164,128,254],[52,284,128,374],[52,404,128,494],[176,44,252,134],[176,164,252,254],[940,84,1016,174],[1060,84,1136,174]]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_meadow\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save meadow png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Beach (Sahil) GPU shaders & rendering (ocean, waves, foam lace, kayaks, birds & palms)
            let p_bm = gl.compile_program(beach::shaders::BEACH_VERT, &beach::shaders::beach_frag_source())
                .unwrap_or_else(|e| panic!("Beach main shader GLSL failed: {e}"));
            (gl.DeleteProgram)(p_bm);
            let p_bp = gl.compile_program(beach::shaders::BEACH_PALM_VERT, &beach::shaders::beach_palm_frag_source())
                .unwrap_or_else(|e| panic!("Beach palm shader GLSL failed: {e}"));
            (gl.DeleteProgram)(p_bp);

            st.set_theme("beach");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[52,374,128,464],[168,44,244,134],[168,154,244,244],[960,84,1036,174],[1080,84,1156,174]]}"#);
            st.handle_json(r#"{"t":"settings","v":{"sigma":85,"warp":34,"shoal":110,"shallow":260,"palms":4,"kayaks":2,"birds":3,"seed":42}}"#);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_BEACH_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("beach");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":85,"warp":34,"shoal":110,"shallow":260,"palms":5,"kayaks":2,"birds":4,"seed":42}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[52,374,128,464],[168,44,244,134],[168,154,244,244],[960,84,1036,174],[1080,84,1156,174]]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_beach\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save beach png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Aurora Lake (Aurora Gölü) GPU rendering (frozen fjord, aurora, snow islands, pines, lanterns & snowfall)
            st.set_theme("aurora");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[52,374,128,464],[168,44,244,134],[168,154,244,244],[960,84,1036,174],[1080,84,1156,174]]}"#);
            st.handle_json(r#"{"t":"settings","v":{"sigma":82,"warp":32,"shoal":90,"aurora":0.78,"flow":1.0,"pines":6,"lanterns":2,"snow":0.45,"adaptive":1,"seed":42}}"#);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_AURORA_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("aurora");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":82,"warp":32,"shoal":90,"aurora":0.85,"flow":1.0,"pines":8,"lanterns":3,"snow":0.55,"adaptive":1,"seed":42}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[52,374,128,464],[168,44,244,134],[168,154,244,244],[960,84,1036,174],[1080,84,1156,174]]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_aurora\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save aurora png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Rose Brook (Gül Deresi) GPU rendering (emerald brook, 12 instanced rose/daisy/petal/butterfly variants)
            st.set_theme("rosebrook");
            st.handle_json(r#"{"t":"settings","v":{"sigma":72,"stream":1.0,"flow":1.0,"sunlight":0.78,"roses":0.85,"petals":12,"breeze":1.0,"adaptive":1,"seed":42}}"#);
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[480,260,556,350]]}"#);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_ROSEBROOK_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("rosebrook");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":72,"stream":1.0,"flow":1.0,"sunlight":0.78,"roses":0.85,"petals":12,"breeze":1.0,"adaptive":1,"seed":42}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[480,260,556,350]]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_rosebrook\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save rosebrook png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Woodland Glade (Orman Açıklığı) GPU rendering (watercolor clearing, 8 instanced tree variants & 3 birds)
            st.set_theme("glade");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[480,260,556,350]]}"#);
            st.handle_json(r#"{"t":"settings","v":{"sigma":78,"warp":28,"density":1.0,"wind":1.0,"birds":5,"adaptive":1,"seed":42}}"#);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_GLADE_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("glade");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":78,"warp":28,"density":1.0,"wind":1.0,"birds":6,"adaptive":1,"seed":42}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[480,260,556,350]]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_glade\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save glade png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Runic Rift (Rünik Vadi) GPU rendering (serpentine river, sanctuaries, turrets, wards, wisps & 5 champions/Poro)
            st.set_theme("rift");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[460,240,536,330],[860,310,936,400]]}"#);
            st.handle_json(r#"{"t":"settings","v":{"sigma":82,"warp":24,"glow":0.82,"wind":1.0,"heroes":5,"turrets":4,"wards":4,"wisps":8,"adaptive":1,"seed":42}}"#);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_RIFT_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("rift");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":82,"warp":24,"glow":0.85,"wind":1.0,"heroes":5,"turrets":4,"wards":4,"wisps":10,"adaptive":1,"seed":42}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,460,128,550],[168,44,244,134],[420,160,496,250],[380,480,456,570],[820,250,896,340],[920,490,996,580]]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_rift\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save rift png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Bombsite Dust (Bomba Alanı Tozu / strike) GPU rendering (sandstone, concrete slabs, hazard curb, craters, crates, A/B stencils, chickens & soccer ball)
            st.set_theme("strike");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[168,44,244,134],[168,154,244,244],[300,90,376,180],[860,510,936,600]]}"#);
            st.handle_json(r#"{"t":"settings","v":{"sigma":82,"warp":20,"holes":1.0,"smoke":0.45,"wind":1.0,"ball":1,"chickens":4,"crates":1.0,"adaptive":1,"seed":42}}"#);
            st.handle_mouse(1, 560.0, 340.0);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_STRIKE_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("strike");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":85,"warp":20,"holes":1.15,"smoke":0.50,"wind":1.0,"ball":1,"chickens":5,"crates":1.05,"adaptive":1,"seed":42}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[168,264,244,354],[304,90,380,180],[304,210,380,300],[440,140,516,230],[900,520,976,610]]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_strike\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save strike png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Frontline Pocket (Cephe Kuşatması / pocket) GPU rendering (HOI4 strategy map, crimson cross-hatch pockets, TR/DE/EN division counters, arrows & artillery)
            st.set_theme("pocket");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[168,44,244,134],[420,140,496,230],[420,250,496,340],[860,460,936,550]],"n":["Belgelerim","Projeler","İndirilenler","Büro_Entwürfe","Übersicht.docx","Screenshots"]}"#);
            st.handle_json(r#"{"t":"settings","v":{"sigma":78,"warp":22,"arrows":0.78,"divisions":12,"faction":0,"pace":1.0,"skirmish":0.65,"supply":0.55,"adaptive":1,"seed":42}}"#);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_POCKET_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("pocket");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":78,"warp":22,"arrows":0.82,"divisions":14,"faction":0,"pace":1.0,"skirmish":0.70,"supply":0.65,"adaptive":1,"seed":42}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[48,54,124,144],[48,164,124,254],[48,470,124,560],[194,250,270,340],[400,110,476,200],[400,220,476,310],[620,260,696,350],[820,490,896,580]],"n":["Belgelerim","Projeler","İndirilenler","Screenshots","Büro_Entwürfe","Übersicht.docx","Çalışmalar","Projects"]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_pocket\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save pocket png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Roach Town (Hamamböceği Kasabası / roach) GPU rendering (cobblestone street, patio tiles, streetlamps, hospital beds, outlets, 18 roach meme leads, jets, bombs, explosions & Turkish speech bubbles)
            st.set_theme("roach");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[168,44,244,134],[420,140,496,230],[420,250,496,340],[860,460,936,550]]}"#);
            st.handle_json(r#"{"t":"settings","v":{"sigma":78,"warp":24,"glow":0.78,"crowd":12,"speed":1.0,"props":0.9,"vibe":0.0,"adaptive":1,"seed":41297}}"#);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_ROACH_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("roach");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":82,"warp":24,"glow":0.82,"crowd":14,"speed":1.0,"props":0.95,"vibe":0.0,"adaptive":1,"seed":41297}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[48,48,124,138],[48,158,124,248],[48,268,124,358],[164,48,240,138],[164,158,240,248],[280,100,356,190],[280,210,356,300],[400,150,476,240]]}"#);
                for _ in 0..115 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_roach\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save roach png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Polar Night Aurora (Kutup Gecesi Aurorası / polarnight) GPU rendering (sky FBO, triple aurora curtains, stars, meteors, 3 mountain ridges, snow forest clearing, warm cabin & 10-slice lake reflection)
            st.set_theme("polarnight");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,580,128,670],[168,580,244,670],[420,560,496,650],[540,560,616,650],[860,570,936,660]]}"#);
            st.handle_json(r#"{"t":"settings","v":{"sigma":90,"density":0.6,"glow":0.75,"flow":1.0,"palette":0,"snow":110,"meteor":1,"cabin":1,"adaptive":1,"seed":424242}}"#);
            for _ in 0..85 {
                st.step(0.033);
            }
            renderer.render(&gl, &mut st);
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_POLARNIGHT_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("polarnight");
                st2.handle_json(r#"{"t":"settings","v":{"sigma":90,"density":0.65,"glow":0.82,"flow":1.0,"palette":0,"snow":120,"meteor":1,"cabin":1,"adaptive":1,"seed":424242}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[48,490,124,580],[164,490,240,580],[420,485,496,575],[540,485,616,575],[860,490,936,580],[980,490,1056,580]]}"#);
                for _ in 0..105 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_polarnight\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save polarnight png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Winter Cabin (Kış Kulübesi) GPU rendering (snowfall, chimney smoke, icon drifts)
            st.set_theme("winter");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[480,260,556,350]]}"#);
            for q in ["Low", "Medium", "High"] {
                st.handle_json(&format!(
                    r#"{{"t":"settings","v":{{"adaptive":1,"snowIntensity":0.75,"snowSpeed":0.85,"windStrength":0.18,"smokeAmount":0.85,"atmosphereDensity":0.75,"treeDensity":0.8,"mountainVisibility":0.85,"iconSnowAmount":0.9,"animSpeed":1.0,"quality":"{q}","seed":20261117}}}}"#
                ));
                for _ in 0..40 {
                    st.step(0.033);
                }
                renderer.render(&gl, &mut st);
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_WINTER_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("winter");
                st2.handle_json(r#"{"t":"settings","v":{"adaptive":1,"snowIntensity":0.75,"snowSpeed":0.85,"windStrength":0.18,"smokeAmount":0.85,"atmosphereDensity":0.75,"treeDensity":0.8,"mountainVisibility":0.85,"iconSnowAmount":0.9,"animSpeed":1.0,"quality":"High","seed":20261117}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[480,260,556,350]]}"#);
                for _ in 0..100 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_winter\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save winter png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Neumorphic Tactile Surface GPU rendering (baked sprites, tiles, widgets, cradles)
            st.set_theme("neumorph");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[480,260,556,350]]}"#);
            for pal in ["alabaster", "sand", "slate", "sage"] {
                for q in ["Low", "Medium", "High"] {
                    st.handle_json(&format!(
                        r#"{{"t":"settings","v":{{"palette":"{pal}","accentColor":"cobalt","reliefDepth":1.0,"lightFollowCursor":false,"waveSpeed":0.75,"tileDensity":1.0,"rippleStrength":1.0,"iconStyle":"cradle","quality":"{q}","adaptive":1,"seed":20261118}}}}"#
                    ));
                    for _ in 0..24 {
                        st.step(0.033);
                    }
                    renderer.render(&gl, &mut st);
                }
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_NEUMORPH_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("neumorph");
                st2.handle_json(r#"{"t":"settings","v":{"palette":"alabaster","accentColor":"cobalt","reliefDepth":1.0,"lightFollowCursor":false,"waveSpeed":0.75,"tileDensity":1.0,"rippleStrength":1.0,"iconStyle":"cradle","quality":"High","adaptive":1,"seed":20261118}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[480,260,556,350]]}"#);
                for _ in 0..90 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_neumorph\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save neumorph png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Istanbul (Boğaz'da Gün Batımı / istanbul) GPU rendering
            st.set_theme("istanbul");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[640,360,716,450],[820,490,896,580]]}"#);
            for t_mode in 0..=4 {
                for lm in 0..=1 {
                    st.handle_json(&format!(
                        r#"{{"t":"settings","v":{{"time":{t_mode},"glow":1.0,"ferries":3,"flow":1.0,"gulls":1,"pad":14,"tulips":1,"cats":1,"landmarks":{lm},"adaptive":1}}}}"#
                    ));
                    for _ in 0..20 {
                        st.step(0.033);
                    }
                    renderer.render(&gl, &mut st);
                }
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_ISTANBUL_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("istanbul");
                st2.handle_json(r#"{"t":"settings","v":{"time":3,"glow":1.0,"ferries":3,"flow":1.0,"gulls":1,"pad":14,"tulips":1,"cats":1,"landmarks":1,"adaptive":1}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[52,44,128,134],[52,154,128,244],[52,264,128,354],[168,44,244,134],[168,154,244,244],[640,360,716,450],[820,490,896,580]]}"#);
                for _ in 0..90 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_istanbul\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save istanbul png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 Solar System & Black Hole (solar-system) GPU rendering
            st.set_theme("solar-system");
            st.handle_json(r#"{"t":"icons","vis":true,"n":["Chrome","VSCode","Documents","Projects","Recycle Bin"],"r":[[240,150,316,226],[960,150,1036,226],[260,500,336,576],[900,490,976,566],[420,280,496,356]]}"#);
            for cb in ["BlackHole", "Sun", "None"] {
                st.handle_json(&format!(
                    r#"{{"t":"settings","v":{{"centerBodyType":"{cb}","blackHoleSize":1.0,"accretionIntensity":1.0,"iconOrbitMode":"Orbit","globalOrbitSpeed":0.65,"planetCount":3,"orbitSize":1.0,"orbitSpeed":1.0,"planetSize":1.0,"starDensity":1.0,"nebulaIntensity":0.65,"asteroidDensity":0.8,"shootingStars":true,"gravityStrength":0.6,"systemAttraction":0.55,"quality":"High","adaptive":1}}}}"#
                ));
                for _ in 0..20 {
                    st.step(0.033);
                }
                st.handle_mouse(0, 640.0, 360.0);
                st.handle_mouse(1, 640.0, 360.0);
                st.handle_mouse(2, 640.0, 360.0);
                renderer.render(&gl, &mut st);
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_SOLAR_SYSTEM_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("solar-system");
                st2.handle_json(r#"{"t":"settings","v":{"centerBodyType":"BlackHole","blackHoleSize":1.0,"accretionIntensity":1.0,"iconOrbitMode":"Orbit","globalOrbitSpeed":0.65,"planetCount":3,"orbitSize":1.0,"orbitSpeed":1.0,"planetSize":1.0,"starDensity":1.0,"nebulaIntensity":0.65,"asteroidDensity":0.8,"shootingStars":true,"gravityStrength":0.6,"systemAttraction":0.55,"quality":"High","adaptive":1}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"n":["Chrome","VSCode","Documents","Projects","Recycle Bin"],"r":[[244,150,320,226],[960,142,1036,218],[270,495,346,571],[910,480,986,556],[422,279,498,355]]}"#);
                for _ in 0..90 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_solar_system\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save solar_system png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify 1:1 ThinkingOrbs (thinking-orbs) GPU rendering across designs & palettes
            st.set_theme("thinking-orbs");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[64,100,144,186],[64,216,136,302],[64,332,136,418],[180,100,252,186],[1024,158,1104,244],[1024,274,1096,360]]}"#);
            for design in ["working", "searching", "solving", "listening", "connecting", "weaving", "composing", "breathing", "shaping"] {
                st.handle_json(&format!(
                    r#"{{"t":"settings","v":{{"orbDesign":"{design}","autoCycle":false,"orbScale":1.0,"dotDensity":1.0,"animSpeed":1.0,"hoverReaction":"strong","dotGlow":0.65,"palette":"monochrome-dark","iconLinks":true,"quality":"High","adaptive":1}}}}"#
                ));
                for _ in 0..8 {
                    st.step(0.033);
                }
                st.handle_mouse(0, 620.0, 350.0);
                st.handle_mouse(1, 620.0, 350.0);
                st.handle_mouse(2, 620.0, 350.0);
                renderer.render(&gl, &mut st);
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_THINKING_ORBS_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("thinking-orbs");
                st2.handle_json(r#"{"t":"settings","v":{"orbDesign":"searching","autoCycle":false,"orbScale":1.0,"dotDensity":1.0,"animSpeed":1.0,"hoverReaction":"strong","dotGlow":0.65,"palette":"monochrome-dark","iconLinks":true,"quality":"High","adaptive":1}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"r":[[64,100,144,186],[64,216,136,302],[64,332,136,418],[64,448,136,534],[180,100,252,186],[180,216,252,302],[1024,158,1104,244],[1024,274,1096,360],[1024,504,1096,590]]}"#);
                for _ in 0..90 {
                    st2.step(0.033);
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_thinking_orbs\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save thinking_orbs png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            // Verify AI Mascot Lab (ai-mascots) GPU rendering across time modes & icons
            st.set_theme("ai-mascots");
            st.handle_json(r#"{"t":"icons","vis":true,"n":["VS Code","Firefox","Terminal","Documents","Downloads","GitHub","Pictures"],"r":[[62,84,138,170],[62,196,138,282],[62,308,138,394],[62,420,138,506],[172,84,248,170],[1120,110,1196,196],[1120,224,1196,310]]}"#);
            for tm in ["night", "evening", "day", "cycle"] {
                st.handle_json(&format!(
                    r#"{{"t":"settings","v":{{"timeMode":"{tm}","mascotActivity":1.0,"lightingGlow":1.0,"particleDensity":0.85,"iconHalo":1.0,"parallax":0.45,"quality":"High","adaptive":1,"seed":2026}}}}"#
                ));
                for _ in 0..15 {
                    st.step(0.033);
                }
                renderer.render(&gl, &mut st);
            }
            if let Ok(out_png) = std::env::var("DIYARDESK_DUMP_AI_MASCOTS_PNG") {
                let dw = 1280i32;
                let dh = 720i32;
                let mut st2 = AppState::new(dw, dh, 1.0);
                st2.set_theme("ai-mascots");
                st2.handle_json(r#"{"t":"settings","v":{"timeMode":"night","mascotActivity":1.0,"lightingGlow":1.0,"particleDensity":0.95,"iconHalo":1.0,"parallax":0.45,"quality":"High","adaptive":1,"seed":2026}}"#);
                st2.handle_json(r#"{"t":"icons","vis":true,"n":["VS Code","Firefox","Terminal","Documents","Downloads","GitHub","Pictures"],"r":[[58,74,134,160],[58,186,134,272],[58,298,134,384],[58,410,134,496],[168,74,244,160],[1126,112,1202,198],[1126,226,1202,312]]}"#);
                for _ in 0..90 {
                    st2.step(0.033);
                }
                // Hover near the first mascot so we also verify the crisp floating category label pill
                if let Some(m0) = st2.ai_mascots.manager.mascots.first() {
                    let (mx, my) = (m0.x as f32, m0.y as f32);
                    st2.handle_mouse(0, mx, my);
                    for _ in 0..25 {
                        st2.step(0.033);
                    }
                }
                let hwnd2 = CreateWindowExA(
                    0, cls.as_ptr(), b"dump_ai_mascots\0".as_ptr(), WS_POPUP,
                    0, 0, dw, dh, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
                );
                let ctx2 = WglContext::create(hwnd2).expect("WGL ctx2 failed");
                assert!(ctx2.make_current());
                let gl2 = GlFns::load().expect("gl2 load");
                let mut r2 = Renderer::new(&gl2).expect("r2 new");
                r2.render(&gl2, &mut st2);
                (gl2.Finish)();
                let mut pixels = vec![0u8; (dw * dh * 4) as usize];
                (gl2.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl2.ReadPixels)(0, 0, dw, dh, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut std::ffi::c_void);
                let mut flipped = vec![0u8; pixels.len()];
                let row_bytes = (dw * 4) as usize;
                for y in 0..(dh as usize) {
                    let src_off = (dh as usize - 1 - y) * row_bytes;
                    let dst_off = y * row_bytes;
                    flipped[dst_off..dst_off + row_bytes].copy_from_slice(&pixels[src_off..src_off + row_bytes]);
                }
                image::save_buffer(&out_png, &flipped, dw as u32, dh as u32, image::ColorType::Rgba8).expect("save ai_mascots png");
                r2.destroy(&gl2);
                drop(ctx2);
                DestroyWindow(hwnd2);
                assert!(ctx.make_current());
            }
            renderer.destroy(&gl);

            drop(ctx);
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn dump_voxelcraft_frame() {
        let Ok(out) = std::env::var("DDGL_DUMP") else { return };
        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarGlVoxelDump\0";
            let wc = WNDCLASSA {
                style: 0x0020,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassA(&wc);
            let (w, h) = (1280i32, 720i32);
            let hwnd = CreateWindowExA(
                0, cls.as_ptr(), b"t\0".as_ptr(), WS_POPUP,
                0, 0, w, h, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let ctx = WglContext::create(hwnd).expect("WGL context failed");
            assert!(ctx.make_current());
            let gl = GlFns::load().expect("OpenGL load failed");

            let mut st = AppState::new(w, h, 1.0);
            st.theme_id = "voxelcraft".to_string();
            let tod = std::env::var("DDGL_TOD").unwrap_or_else(|_| "day".to_string());
            let biome = std::env::var("DDGL_BIOME").unwrap_or_else(|_| "overworld".to_string());
            let style = std::env::var("DDGL_STYLE").unwrap_or_else(|_| "island".to_string());
            let params = format!(
                r#"{{"t":"settings","theme":"voxelcraft","params":{{"timeOfDay":"{tod}","biome":"{biome}","iconBlockStyle":"{style}"}}}}"#
            );
            st.handle_json(&params);
            st.handle_json(
                r#"{"t":"icons","vis":true,"r":[[64,100,144,186],[64,216,144,302],[64,332,144,418],[576,490,656,576],[704,490,784,576],[1024,518,1104,604]]}"#,
            );
            for _ in 0..40 {
                st.step(0.033);
            }
            let mut r = Renderer::new(&gl).expect("renderer");
            r.render(&gl, &mut st);

            let mut buf = vec![0u8; (w * h * 4) as usize];
            (gl.ReadPixels)(
                0, 0, w, h, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
            );
            let mut flipped = vec![0u8; buf.len()];
            let row = (w * 4) as usize;
            for y in 0..h as usize {
                let src = (h as usize - 1 - y) * row;
                flipped[y * row..(y + 1) * row].copy_from_slice(&buf[src..src + row]);
            }
            image::save_buffer(
                &out, &flipped, w as u32, h as u32, image::ColorType::Rgba8,
            )
            .expect("save png");
            r.destroy(&gl);
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn dump_liquidglass_frame() {
        let out = std::env::var("DDGL_DUMP").ok();
        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarGlLiquidDump\0";
            let wc = WNDCLASSA {
                style: 0x0020,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassA(&wc);
            let w: i32 = std::env::var("DDGL_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1280);
            let h: i32 = std::env::var("DDGL_H").ok().and_then(|v| v.parse().ok()).unwrap_or(720);
            let hwnd = CreateWindowExA(
                0, cls.as_ptr(), b"t\0".as_ptr(), WS_POPUP,
                0, 0, w, h, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let ctx = WglContext::create(hwnd).expect("WGL context failed");
            assert!(ctx.make_current());
            let gl = GlFns::load().expect("OpenGL load failed");

            let mut st = AppState::new(w, h, 1.25);
            let mut r = Renderer::new(&gl).expect("renderer");
            if let Ok(pre) = std::env::var("DDGL_PRE_THEME") {
                st.handle_json(&format!(
                    r#"{{"t":"settings","theme":"{pre}","params":{{}}}}"#
                ));
                for _ in 0..10 {
                    st.step(0.033);
                    r.render(&gl, &mut st);
                }
                println!("pre_theme={pre}");
            }
            st.theme_id = "liquidglass".to_string();
            let extra = std::env::var("DDGL_LG_PARAMS").unwrap_or_default();
            let params = if extra.is_empty() {
                r#"{"t":"settings","theme":"liquidglass","params":{}}"#.to_string()
            } else {
                format!(r#"{{"t":"settings","theme":"liquidglass","params":{extra}}}"#)
            };
            st.handle_json(&params);
            st.handle_json(
                r#"{"t":"icons","vis":true,"r":[[25,62.5,100,162.5],[25,187.5,100,287.5],[25,312.5,100,412.5],[312.5,468.75,412.5,568.75],[312.5,593.75,412.5,693.75],[156.25,750,256.25,850]]}"#,
            );
            for _ in 0..40 {
                st.step(0.033);
            }
            let frames: usize = std::env::var("DDGL_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
            let t0 = Instant::now();
            for _ in 0..frames {
                st.step(0.033);
                r.render(&gl, &mut st);
                (gl.Finish)();
            }
            let per = t0.elapsed().as_secs_f64() * 1000.0 / frames as f64;
            println!("liquidglass frames={frames} avg_ms={per:.2}");
            println!(
                "drops={} {:?}",
                st.liquidglass.drop_count,
                &st.liquidglass.drop_data[..st.liquidglass.drop_count]
            );
            println!("sim_time={:.6}", st.liquidglass.sim_time);
            if let Some(out) = out {
                let mut buf = vec![0u8; (w * h * 4) as usize];
                (gl.ReadPixels)(
                    0, 0, w, h, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE,
                    buf.as_mut_ptr() as *mut std::ffi::c_void,
                );
                let mut flipped = vec![0u8; buf.len()];
                let row = (w * 4) as usize;
                for y in 0..h as usize {
                    let src = (h as usize - 1 - y) * row;
                    flipped[y * row..(y + 1) * row].copy_from_slice(&buf[src..src + row]);
                }
                image::save_buffer(
                    &out, &flipped, w as u32, h as u32, image::ColorType::Rgba8,
                )
                .expect("save png");
            }
            r.destroy(&gl);
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn dump_studio_glass_frame() {
        let out = std::env::var("DDGL_DUMP").ok();
        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarGlStudioDump\0";
            let wc = WNDCLASSA {
                style: 0x0020,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassA(&wc);
            let w: i32 = std::env::var("DDGL_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1280);
            let h: i32 = std::env::var("DDGL_H").ok().and_then(|v| v.parse().ok()).unwrap_or(720);
            let hwnd = CreateWindowExA(
                0, cls.as_ptr(), b"t\0".as_ptr(), WS_POPUP,
                0, 0, w, h, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let ctx = WglContext::create(hwnd).expect("WGL context failed");
            assert!(ctx.make_current());
            let gl = GlFns::load().expect("OpenGL load failed");

            let mut st = AppState::new(w, h, 1.0);
            let mut r = Renderer::new(&gl).expect("renderer");
            st.handle_json(r#"{"t":"theme","id":"studio-glass"}"#);
            st.handle_json(r#"{"t":"settings","theme":"studio-glass","params":{"showHud":1,"refraction":0.85,"frost":0.5,"iconGlass":0}}"#);
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[28,36,104,126],[28,144,104,234],[28,252,104,342],[28,360,104,450]]}"#);
            for _ in 0..30 {
                st.step(0.033);
                r.render(&gl, &mut st);
                (gl.Finish)();
            }
            if let Some(out) = out {
                let mut buf = vec![0u8; (w * h * 4) as usize];
                (gl.ReadPixels)(
                    0, 0, w, h, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE,
                    buf.as_mut_ptr() as *mut std::ffi::c_void,
                );
                let mut flipped = vec![0u8; buf.len()];
                let row = (w * 4) as usize;
                for y in 0..h as usize {
                    let src = (h as usize - 1 - y) * row;
                    flipped[y * row..(y + 1) * row].copy_from_slice(&buf[src..src + row]);
                }
                image::save_buffer(
                    &out, &flipped, w as u32, h as u32, image::ColorType::Rgba8,
                )
                .expect("save png");
            }
            r.destroy(&gl);
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn dump_zen_frame() {
        let out = std::env::var("DDGL_DUMP").ok();
        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarGlZenDump\0";
            let wc = WNDCLASSA {
                style: 0x0020,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassA(&wc);
            let w: i32 = std::env::var("DDGL_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1280);
            let h: i32 = std::env::var("DDGL_H").ok().and_then(|v| v.parse().ok()).unwrap_or(720);
            let hwnd = CreateWindowExA(
                0, cls.as_ptr(), b"t\0".as_ptr(), WS_POPUP,
                0, 0, w, h, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let ctx = WglContext::create(hwnd).expect("WGL context failed");
            assert!(ctx.make_current());
            let gl = GlFns::load().expect("OpenGL load failed");

            let mut st = AppState::new(w, h, 1.25);
            st.theme_id = "zen-archipelago".to_string();
            let extra = std::env::var("DDGL_ZA_PARAMS").unwrap_or_default();
            let params = if extra.is_empty() {
                r#"{"t":"settings","theme":"zen-archipelago","params":{}}"#.to_string()
            } else {
                format!(r#"{{"t":"settings","theme":"zen-archipelago","params":{extra}}}"#)
            };
            st.handle_json(&params);
            st.handle_json(
                r#"{"t":"icons","vis":true,"r":[[25,62.5,100,162.5],[25,187.5,100,287.5],[25,312.5,100,412.5],[312.5,468.75,412.5,568.75],[312.5,593.75,412.5,693.75],[156.25,750,256.25,850]]}"#,
            );
            let mut r = Renderer::new(&gl).expect("renderer");
            let frames: usize = std::env::var("DDGL_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
            let t0 = Instant::now();
            for _ in 0..frames {
                st.step(0.033);
                r.render(&gl, &mut st);
                (gl.Finish)();
            }
            let per = t0.elapsed().as_secs_f64() * 1000.0 / frames as f64;
            println!("zen frames={frames} avg_ms={per:.2}");
            println!("zen sim_time={:.6}", st.zen_archipelago.time);
            if let Some(out) = out {
                let mut buf = vec![0u8; (w * h * 4) as usize];
                (gl.ReadPixels)(
                    0, 0, w, h, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE,
                    buf.as_mut_ptr() as *mut std::ffi::c_void,
                );
                let mut flipped = vec![0u8; buf.len()];
                let row = (w * 4) as usize;
                for y in 0..h as usize {
                    let src = (h as usize - 1 - y) * row;
                    flipped[y * row..(y + 1) * row].copy_from_slice(&buf[src..src + row]);
                }
                image::save_buffer(
                    &out, &flipped, w as u32, h as u32, image::ColorType::Rgba8,
                )
                .expect("save png");
            }
            r.destroy(&gl);
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn dump_desert_atlas() {
        let Ok(out) = std::env::var("DDGL_ATLAS") else { return };
        let c = crate::desert::atlas::bake_atlas();
        image::save_buffer(
            &out, &c.px, c.w as u32, c.h as u32, image::ColorType::Rgba8,
        )
        .expect("save atlas png");
    }

    #[test]
    fn dump_desert_rng() {
        let mut r = crate::desert::Rng32::new(73294);
        let mut s = String::new();
        for _ in 0..6 {
            s.push_str(&format!("{:.17} ", r.next_f64()));
        }
        println!("rng32: {s}");
    }

    #[test]
    fn dump_desert_noise() {
        let d = crate::desert::build_noise(73194);
        let a: Vec<u8> = (0..8).map(|i| d[i * 4]).collect();
        let b: Vec<u8> = (0..4).map(|i| d[i * 4 + 1]).collect();
        let c: Vec<u8> = (0..4).map(|i| d[i * 4 + 2]).collect();
        println!("noise a={a:?} b={b:?} c={c:?}");
    }

    #[test]
    fn dump_desert_frame() {
        let Ok(out) = std::env::var("DDGL_DUMP") else { return };
        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarGlDesertDump\0";
            let wc = WNDCLASSA {
                style: 0x0020,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassA(&wc);
            let w: i32 = std::env::var("DDGL_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1280);
            let h: i32 = std::env::var("DDGL_H").ok().and_then(|v| v.parse().ok()).unwrap_or(720);
            let hwnd = CreateWindowExA(
                0, cls.as_ptr(), b"t\0".as_ptr(), WS_POPUP,
                0, 0, w, h, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let ctx = WglContext::create(hwnd).expect("WGL context failed");
            assert!(ctx.make_current());
            let gl = GlFns::load().expect("OpenGL load failed");

            let mut st = AppState::new(w, h, 1.25);
            st.theme_id = "desert".to_string();
            let seed = std::env::var("DDGL_DESERT_SEED").unwrap_or_else(|_| "73194".to_string());
            let oases = std::env::var("DDGL_DESERT_OASES").unwrap_or_else(|_| "2".to_string());
            let dbg = std::env::var("DDGL_DESERT_DEBUG").unwrap_or_else(|_| "false".to_string());
            let extra = std::env::var("DDGL_DESERT_PARAMS").unwrap_or_default();
            let params = if extra.is_empty() {
                format!(
                    r#"{{"t":"settings","theme":"desert","params":{{"seed":{seed},"oases":{oases},"debug":{dbg}}}}}"#
                )
            } else {
                format!(r#"{{"t":"settings","theme":"desert","params":{extra}}}"#)
            };
            st.handle_json(&params);
            st.handle_json(
                r#"{"t":"icons","vis":true,"r":[[20,50,80,130],[20,150,80,230],[20,250,80,330],[250,375,330,455],[250,475,330,555],[125,600,205,680]]}"#,
            );
            for _ in 0..40 {
                st.step(0.033);
            }
            let mut r = Renderer::new(&gl).expect("renderer");
            r.render(&gl, &mut st);
            for _ in 0..70 {
                st.step(0.033);
            }
            if let Ok(tv) = std::env::var("DDGL_DESERT_TIME") {
                if let Ok(tf) = tv.parse::<f32>() {
                    st.desert.time = tf;
                }
            }
            if std::env::var("DDGL_DESERT_DUMP_OASES").is_ok() {
                println!(
                    "oases={:?} props={} car={} dust={} gw={} gh={} w={} h={}",
                    st.desert
                        .oases
                        .iter()
                        .map(|o| (
                            (o.x * 1000.0).round() / 1000.0,
                            (o.y * 1000.0).round() / 1000.0,
                            (o.rx * 1000.0).round() / 1000.0,
                            (o.ry * 1000.0).round() / 1000.0
                        ))
                        .collect::<Vec<_>>(),
                    st.desert.props.len(),
                    st.desert.caravans.len(),
                    st.desert.dust.len(),
                    st.desert.gw,
                    st.desert.gh,
                    st.desert.w,
                    st.desert.h
                );
            }
            r.render(&gl, &mut st);

            let mut buf = vec![0u8; (w * h * 4) as usize];
            (gl.ReadPixels)(
                0, 0, w, h, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
            );
            let mut flipped = vec![0u8; buf.len()];
            let row = (w * 4) as usize;
            for y in 0..h as usize {
                let src = (h as usize - 1 - y) * row;
                flipped[y * row..(y + 1) * row].copy_from_slice(&buf[src..src + row]);
            }
            image::save_buffer(
                &out, &flipped, w as u32, h as u32, image::ColorType::Rgba8,
            )
            .expect("save png");
            r.destroy(&gl);
            DestroyWindow(hwnd);
        }
    }
}

#[cfg(all(test, not(target_os = "windows")))]
pub(crate) static LINUX_EGL_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(all(test, not(target_os = "windows")))]
mod linux_tests {
    use super::*;
    use crate::shaders::{
        BACKDROP_FRAG, BLIT_FRAG, BLUR_FRAG, FULLSCREEN_VERT, SDF_UI_FRAG, SDF_UI_VERT, TEXT_FRAG,
        TEXT_VERT,
    };
    use crate::themes::resolve_shader_and_params;
    use std::ffi::CString;

    #[test]
    fn native_theme_registry_matches_expected() {
        unsafe {
            for &tid in NATIVE_GL_THEMES {
                let c = CString::new(tid).unwrap();
                assert_eq!(ddgl_is_native_theme(c.as_ptr()), 1, "expected native theme: {tid}");
            }
            for unknown in ["", "custom-html", "video-123", "my_zip_theme"] {
                let c = CString::new(unknown).unwrap();
                assert_eq!(ddgl_is_native_theme(c.as_ptr()), 0, "unexpected native theme: {unknown}");
            }
            assert_eq!(ddgl_is_native_theme(std::ptr::null()), 0);
        }
    }

    #[test]
    fn linux_egl_compiles_all_shaders_and_renders_all_native_themes() {
        let _guard = LINUX_EGL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let ctx = EglHeadlessContext::create().expect("Linux EGL headless context failed");
            assert!(ctx.make_current(), "EGL make_current failed");
            let gl = GlFns::load().expect("Linux GlFns::load failed");

            // 1. Core shaders
            for (name, vert, frag) in [
                ("backdrop", FULLSCREEN_VERT, BACKDROP_FRAG),
                ("blur", FULLSCREEN_VERT, BLUR_FRAG),
                ("blit", FULLSCREEN_VERT, BLIT_FRAG),
                ("sdf_ui", SDF_UI_VERT, SDF_UI_FRAG),
                ("text", TEXT_VERT, TEXT_FRAG),
            ] {
                let p = gl
                    .compile_program(vert, frag)
                    .unwrap_or_else(|e| panic!("Core shader '{name}' GLSL failed on Linux: {e}"));
                (gl.DeleteProgram)(p);
            }

            // 2. Resolved theme fragment shaders
            let mut st = AppState::new(640, 360, 1.0);
            for &tid in NATIVE_GL_THEMES {
                st.set_theme(tid);
                let (frag, _) = resolve_shader_and_params(&st);
                let p = gl
                    .compile_program(FULLSCREEN_VERT, &frag)
                    .unwrap_or_else(|e| panic!("Theme '{tid}' fallback shader failed on Linux: {e}"));
                (gl.DeleteProgram)(p);
            }

            // 3. Test ddgl_embedded_* into a non-zero offscreen FBO (same path as Gtk.GLArea / Qt6)
            let mut fbo = 0u32;
            let mut tex = 0u32;
            let mut depth_rb = 0u32;
            let (ew, eh) = (320i32, 180i32);
            (gl.GenFramebuffers)(1, &mut fbo);
            (gl.GenTextures)(1, &mut tex);
            (gl.BindTexture)(gl::GL_TEXTURE_2D, tex);
            (gl.TexImage2D)(
                gl::GL_TEXTURE_2D,
                0,
                gl::GL_RGBA8 as gl::GLint,
                ew,
                eh,
                0,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE,
                std::ptr::null(),
            );
            (gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MIN_FILTER, gl::GL_LINEAR);
            (gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MAG_FILTER, gl::GL_LINEAR);
            (gl.GenRenderbuffers)(1, &mut depth_rb);
            (gl.BindRenderbuffer)(gl::GL_RENDERBUFFER, depth_rb);
            (gl.RenderbufferStorage)(gl::GL_RENDERBUFFER, gl::GL_DEPTH_COMPONENT24, ew, eh);
            (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, fbo);
            (gl.FramebufferTexture2D)(
                gl::GL_FRAMEBUFFER,
                gl::GL_COLOR_ATTACHMENT0,
                gl::GL_TEXTURE_2D,
                tex,
                0,
            );
            (gl.FramebufferRenderbuffer)(
                gl::GL_FRAMEBUFFER,
                gl::GL_DEPTH_ATTACHMENT,
                gl::GL_RENDERBUFFER,
                depth_rb,
            );
            assert_eq!(
                (gl.CheckFramebufferStatus)(gl::GL_FRAMEBUFFER),
                gl::GL_FRAMEBUFFER_COMPLETE,
                "Test offscreen FBO incomplete"
            );
            assert!(fbo != 0, "Expected non-zero test FBO id");

            let eng = ddgl_embedded_create(ew, eh, 1.0);
            assert!(!eng.is_null(), "ddgl_embedded_create failed");
            let icons_msg = CString::new(
                r#"{"t":"icons","vis":true,"r":[[24,24,72,84],[24,104,72,164],[140,48,188,108]]}"#,
            )
            .unwrap();
            ddgl_embedded_post_json(eng, icons_msg.as_ptr());

            let mut rgba = vec![0u8; (ew * eh * 4) as usize];
            for &tid in NATIVE_GL_THEMES {
                let theme_msg = CString::new(format!(r#"{{"t":"theme","id":"{tid}"}}"#)).unwrap();
                ddgl_embedded_post_json(eng, theme_msg.as_ptr());
                // Simulate Gtk.GLArea enabling GL_DEPTH_TEST before render callback
                (gl.Enable)(gl::GL_DEPTH_TEST);
                // Bind our non-zero FBO and pass u32::MAX to test GL_FRAMEBUFFER_BINDING auto-detect
                (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, fbo);
                assert_eq!(ddgl_embedded_render(eng, 0.033, u32::MAX), 1);
                assert_eq!(ddgl_embedded_render(eng, 0.033, u32::MAX), 1);
            }
            // Verify actual non-zero pixels rendered into the non-zero FBO
            let papercut_msg = CString::new(r#"{"t":"theme","id":"papercut"}"#).unwrap();
            ddgl_embedded_post_json(eng, papercut_msg.as_ptr());
            (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, fbo);
            assert_eq!(ddgl_embedded_render(eng, 0.033, u32::MAX), 1);
            let n = ddgl_embedded_read_rgba(eng, fbo, rgba.as_mut_ptr(), rgba.len() as i32);
            assert_eq!(n, rgba.len() as i32);
            let non_zero_rgb = rgba
                .chunks_exact(4)
                .filter(|px| px[0] > 8 || px[1] > 8 || px[2] > 8)
                .count();
            assert!(
                non_zero_rgb > (ew * eh / 4) as usize,
                "Expected visible rendered pixels in non-zero FBO, got {non_zero_rgb}"
            );

            ddgl_embedded_destroy(eng);
            (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
            (gl.DeleteRenderbuffers)(1, &depth_rb);
            (gl.DeleteTextures)(1, &tex);
            (gl.DeleteFramebuffers)(1, &fbo);
            drop(ctx);
        }
    }

    #[test]
    fn linux_headless_renders_png_and_rgba() {
        let _guard = LINUX_EGL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let eng = ddgl_headless_create(320, 180, 1.0);
            assert!(!eng.is_null(), "ddgl_headless_create failed");
            let theme = CString::new(r#"{"t":"theme","id":"meadow"}"#).unwrap();
            let icons = CString::new(r#"{"t":"icons","vis":true,"r":[[20,20,76,86]]}"#).unwrap();
            ddgl_headless_post_json(eng, theme.as_ptr());
            ddgl_headless_post_json(eng, icons.as_ptr());
            ddgl_headless_set_low_mode(eng, 1);
            ddgl_headless_set_low_mode(eng, 0);
            let _ = ddgl_headless_mouse(eng, 0, 100.0, 90.0);

            let mut buf = vec![0u8; 320 * 180 * 4];
            let n = ddgl_headless_render_rgba(eng, 0.033, buf.as_mut_ptr(), buf.len() as i32);
            assert_eq!(n, buf.len() as i32);
            assert!(buf.chunks_exact(4).any(|px| px[0] > 10 || px[1] > 10 || px[2] > 10));

            let tmp_png = std::env::temp_dir().join("diyardesk_gl_linux_test.png");
            let cpath = CString::new(tmp_png.to_string_lossy().as_ref()).unwrap();
            assert_eq!(ddgl_headless_render_png(eng, 0.033, cpath.as_ptr()), 1);
            assert!(tmp_png.is_file());
            let meta = std::fs::metadata(&tmp_png).unwrap();
            assert!(meta.len() > 1000, "PNG unexpectedly tiny: {} bytes", meta.len());
            let _ = std::fs::remove_file(&tmp_png);

            ddgl_headless_destroy(eng);
        }
    }

    #[test]
    fn gles3_egl_compiles_all_shaders_and_renders_all_native_themes() {
        let _guard = LINUX_EGL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let ctx = EglHeadlessContext::create_gles3()
                .expect("OpenGL ES 3.0 headless EGL context failed");
            assert!(ctx.make_current(), "GLES3 EGL make_current failed");
            let gl = GlFns::load().expect("GLES3 GlFns::load failed");
            assert!(gl.is_gles, "Expected OpenGL ES context, got desktop OpenGL");

            // 1. Core shaders must compile & link under GLSL ES 3.00
            gl::start_gles_shader_recording();
            for (name, vert, frag) in [
                ("backdrop", FULLSCREEN_VERT, BACKDROP_FRAG),
                ("blur", FULLSCREEN_VERT, BLUR_FRAG),
                ("blit", FULLSCREEN_VERT, BLIT_FRAG),
                ("sdf_ui", SDF_UI_VERT, SDF_UI_FRAG),
                ("text", TEXT_VERT, TEXT_FRAG),
            ] {
                let p = gl
                    .compile_program(vert, frag)
                    .unwrap_or_else(|e| panic!("Core shader '{name}' failed on GLES 3.0: {e}"));
                (gl.DeleteProgram)(p);
            }

            // 2. Directly instantiate EVERY specialized theme renderer under GLES 3.0
            //    so no shader compilation failure can hide behind Renderer's fallback path.
            macro_rules! check_renderer {
                ($name:literal, $ty:ty) => {{
                    let mut r = <$ty>::new(&gl)
                        .unwrap_or_else(|e| panic!("Specialized renderer '{}' failed on GLES 3.0: {e}", $name));
                    r.destroy(&gl);
                }};
            }
            check_renderer!("studio-glass", crate::studio_glass::StudioGlassRenderer);
            check_renderer!("quietsnow", crate::quiet_snow::QuietSnowRenderer);
            check_renderer!("voxelcraft", crate::voxelcraft::VoxelcraftRenderer);
            check_renderer!("desert", crate::desert::DesertRenderer);
            check_renderer!("papercut", crate::papercut::PapercutRenderer);
            check_renderer!("claymorph", crate::claymorph::ClaymorphRenderer);
            check_renderer!("neobrutalism", crate::neobrutalism::NeobrutalismRenderer);
            check_renderer!("liquidglass", crate::liquidglass::LiquidGlassRenderer);
            check_renderer!("zen-archipelago", crate::zen_archipelago::ZenArchipelagoRenderer);
            check_renderer!("software-world", crate::software_world::SoftwareWorldRenderer);
            check_renderer!("meadow", crate::meadow::MeadowRenderer);
            check_renderer!("beach", crate::beach::BeachRenderer);
            check_renderer!("aurora", crate::aurora::AuroraRenderer);
            check_renderer!("glade", crate::glade::GladeRenderer);
            check_renderer!("rift", crate::rift::RiftRenderer);
            check_renderer!("strike", crate::strike::StrikeRenderer);
            check_renderer!("pocket", crate::pocket::PocketRenderer);
            check_renderer!("roach", crate::roach::RoachRenderer);
            check_renderer!("polarnight", crate::polarnight::PolarNightRenderer);
            check_renderer!("rosebrook", crate::rosebrook::RosebrookRenderer);
            check_renderer!("neumorph", crate::neumorph::NeumorphRenderer);
            check_renderer!("winter", crate::winter::WinterRenderer);
            check_renderer!("istanbul", crate::istanbul::IstanbulRenderer);
            check_renderer!("solar-system", crate::solar_system::SolarSystemRenderer);
            check_renderer!("thinking-orbs", crate::thinking_orbs::ThinkingOrbsRenderer);
            check_renderer!("ai-mascots", crate::ai_mascots::AiMascotsRenderer);
            check_renderer!("cat-shore", crate::cat_shore::CatShoreRenderer);
            check_renderer!("persephone", crate::persephone::PersephoneRenderer);
            check_renderer!("eye-within-words", crate::eye_within_words::EyeWithinWordsRenderer);
            check_renderer!("cosmic-eye", crate::cosmic_eye::CosmicEyeRenderer);
            check_renderer!("ascii-lab", crate::ascii_lab::AsciiLabRenderer);
            check_renderer!("ascii-cat-knight", crate::ascii_cat_knight::AsciiCatKnightRenderer);
            check_renderer!("turkish-flag", crate::turkish_flag::TurkishFlagRenderer);
            check_renderer!("custom-glsl", crate::custom_glsl::CustomGlslRenderer);

            // 3. Resolved fallback shaders for all themes under GLSL ES 3.00
            let mut st = AppState::new(640, 360, 1.0);
            for &tid in NATIVE_GL_THEMES {
                st.set_theme(tid);
                let (frag, _) = resolve_shader_and_params(&st);
                let p = gl
                    .compile_program(FULLSCREEN_VERT, &frag)
                    .unwrap_or_else(|e| panic!("Theme '{tid}' fallback shader failed on GLES 3.0: {e}"));
                (gl.DeleteProgram)(p);
            }

            // 3b. Validate EVERY recorded translated GLSL ES 3.00 shader with strict Khronos
            //     glslangValidator when installed (catches Adreno/Mali spec violations Mesa accepts).
            let recorded_shaders = gl::take_recorded_gles_shaders();
            assert!(
                recorded_shaders.len() > 100,
                "Expected >100 translated GLES 3.00 shaders, got {}",
                recorded_shaders.len()
            );
            if std::process::Command::new("glslangValidator")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                let tmp_dir = std::env::temp_dir().join(format!("diyardesk_glsl_{}", std::process::id()));
                let _ = std::fs::create_dir_all(&tmp_dir);
                let mut seen = std::collections::HashSet::new();
                for (idx, (kind, src)) in recorded_shaders.iter().enumerate() {
                    if !seen.insert((*kind, src.as_str())) {
                        continue;
                    }
                    let ext = if *kind == gl::GL_VERTEX_SHADER { "vert" } else { "frag" };
                    let path = tmp_dir.join(format!("shader_{idx}.{ext}"));
                    std::fs::write(&path, src).expect("write shader for glslangValidator");
                    let out = std::process::Command::new("glslangValidator")
                        .arg(&path)
                        .output()
                        .expect("run glslangValidator");
                    let _ = std::fs::remove_file(&path);
                    if !out.status.success() {
                        let _ = std::fs::remove_dir_all(&tmp_dir);
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        panic!(
                            "Strict Khronos glslangValidator rejected GLES 3.00 shader #{idx} (.{ext}):\n{stdout}\n{stderr}"
                        );
                    }
                }
                let _ = std::fs::remove_dir_all(&tmp_dir);
            }

            // 4. Render every single native theme through ddgl_embedded_* into an FBO under GLES 3.0
            let mut fbo = 0u32;
            let mut tex = 0u32;
            let mut depth_rb = 0u32;
            let (ew, eh) = (320i32, 180i32);
            (gl.GenFramebuffers)(1, &mut fbo);
            (gl.GenTextures)(1, &mut tex);
            (gl.BindTexture)(gl::GL_TEXTURE_2D, tex);
            (gl.TexImage2D)(
                gl::GL_TEXTURE_2D,
                0,
                gl::GL_RGBA8 as gl::GLint,
                ew,
                eh,
                0,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE,
                std::ptr::null(),
            );
            (gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MIN_FILTER, gl::GL_LINEAR);
            (gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MAG_FILTER, gl::GL_LINEAR);
            (gl.GenRenderbuffers)(1, &mut depth_rb);
            (gl.BindRenderbuffer)(gl::GL_RENDERBUFFER, depth_rb);
            (gl.RenderbufferStorage)(gl::GL_RENDERBUFFER, gl::GL_DEPTH_COMPONENT24, ew, eh);
            (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, fbo);
            (gl.FramebufferTexture2D)(
                gl::GL_FRAMEBUFFER,
                gl::GL_COLOR_ATTACHMENT0,
                gl::GL_TEXTURE_2D,
                tex,
                0,
            );
            (gl.FramebufferRenderbuffer)(
                gl::GL_FRAMEBUFFER,
                gl::GL_DEPTH_ATTACHMENT,
                gl::GL_RENDERBUFFER,
                depth_rb,
            );
            assert_eq!(
                (gl.CheckFramebufferStatus)(gl::GL_FRAMEBUFFER),
                gl::GL_FRAMEBUFFER_COMPLETE,
                "GLES3 test offscreen FBO incomplete"
            );

            let eng = ddgl_embedded_create(ew, eh, 1.0);
            assert!(!eng.is_null(), "ddgl_embedded_create failed under GLES 3.0");
            let icons_msg = CString::new(
                r#"{"t":"icons","vis":true,"r":[[24,24,72,84],[24,104,72,164],[140,48,188,108]]}"#,
            )
            .unwrap();
            ddgl_embedded_post_json(eng, icons_msg.as_ptr());

            let mut rgba = vec![0u8; (ew * eh * 4) as usize];
            for &tid in NATIVE_GL_THEMES {
                // Drain any prior errors
                while (gl.GetError)() != gl::GL_NO_ERROR {}
                let theme_msg = CString::new(format!(r#"{{"t":"theme","id":"{tid}"}}"#)).unwrap();
                ddgl_embedded_post_json(eng, theme_msg.as_ptr());
                (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, fbo);
                assert_eq!(ddgl_embedded_render(eng, 0.033, u32::MAX), 1);
                assert_eq!(ddgl_embedded_render(eng, 0.033, u32::MAX), 1);
                let err = (gl.GetError)();
                assert_eq!(
                    err,
                    gl::GL_NO_ERROR,
                    "GLES 3.0 error 0x{:X} while rendering theme '{tid}'",
                    err
                );
                let n = ddgl_embedded_read_rgba(eng, fbo, rgba.as_mut_ptr(), rgba.len() as i32);
                assert_eq!(n, rgba.len() as i32);
                let non_zero = rgba
                    .chunks_exact(4)
                    .filter(|px| px[0] > 4 || px[1] > 4 || px[2] > 4)
                    .count();
                assert!(
                    non_zero > 100,
                    "Theme '{tid}' rendered blank/black frame under GLES 3.0 (non_zero={non_zero})"
                );
            }

            ddgl_embedded_destroy(eng);
            (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
            (gl.DeleteRenderbuffers)(1, &depth_rb);
            (gl.DeleteTextures)(1, &tex);
            (gl.DeleteFramebuffers)(1, &fbo);
            drop(ctx);
        }
    }

    #[test]
    fn gles3_minimal_caps_fallbacks_render_cleanly() {
        let _guard = LINUX_EGL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let ctx = EglHeadlessContext::create_gles3()
                .expect("OpenGL ES 3.0 headless EGL context failed");
            assert!(ctx.make_current(), "GLES3 EGL make_current failed");
            let mut gl = GlFns::load().expect("GLES3 GlFns::load failed");
            assert!(gl.is_gles);
            // Simulate a strict baseline OpenGL ES 3.0 mobile GPU without
            // GL_OES_texture_float_linear or GL_EXT_color_buffer_float.
            gl.force_gles_minimal_caps();

            let (ew, eh) = (320i32, 180i32);
            let mut fbo = 0u32;
            let mut tex = 0u32;
            let mut depth_rb = 0u32;
            (gl.GenFramebuffers)(1, &mut fbo);
            (gl.GenTextures)(1, &mut tex);
            (gl.BindTexture)(gl::GL_TEXTURE_2D, tex);
            (gl.TexImage2D)(
                gl::GL_TEXTURE_2D,
                0,
                gl::GL_RGBA8 as gl::GLint,
                ew,
                eh,
                0,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE,
                std::ptr::null(),
            );
            (gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MIN_FILTER, gl::GL_LINEAR);
            (gl.TexParameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MAG_FILTER, gl::GL_LINEAR);
            (gl.GenRenderbuffers)(1, &mut depth_rb);
            (gl.BindRenderbuffer)(gl::GL_RENDERBUFFER, depth_rb);
            (gl.RenderbufferStorage)(gl::GL_RENDERBUFFER, gl::GL_DEPTH_COMPONENT24, ew, eh);
            (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, fbo);
            (gl.FramebufferTexture2D)(
                gl::GL_FRAMEBUFFER,
                gl::GL_COLOR_ATTACHMENT0,
                gl::GL_TEXTURE_2D,
                tex,
                0,
            );
            (gl.FramebufferRenderbuffer)(
                gl::GL_FRAMEBUFFER,
                gl::GL_DEPTH_ATTACHMENT,
                gl::GL_RENDERBUFFER,
                depth_rb,
            );
            assert_eq!(
                (gl.CheckFramebufferStatus)(gl::GL_FRAMEBUFFER),
                gl::GL_FRAMEBUFFER_COMPLETE
            );

            let mut renderer = Renderer::new(&gl).expect("Renderer::new failed under minimal GLES3");
            let mut st = AppState::new(ew, eh, 1.0);
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[24,24,72,84],[140,48,188,108]]}"#);

            let fallback_sensitive = [
                "liquidglass",
                "studio-glass",
                "meadow",
                "beach",
                "aurora",
                "glade",
                "rift",
                "strike",
                "pocket",
                "rosebrook",
                "roach",
                "eye-within-words",
                "cosmic-eye",
                "ascii-clownfish",
                "ascii-jellyfish",
                "ascii-shark",
                "ascii-lotus",
                "ascii-cat-knight",
                "turkish-flag",
            ];
            let mut rgba = vec![0u8; (ew * eh * 4) as usize];
            for tid in fallback_sensitive {
                while (gl.GetError)() != gl::GL_NO_ERROR {}
                st.set_theme(tid);
                st.step(0.033);
                let _fbo_guard = DefaultFboGuard::new(fbo);
                (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl.Disable)(gl::GL_DEPTH_TEST);
                (gl.Disable)(gl::GL_SCISSOR_TEST);
                (gl.Disable)(gl::GL_CULL_FACE);
                (gl.Disable)(gl::GL_BLEND);
                (gl.DepthMask)(gl::GL_TRUE);
                renderer.render(&gl, &mut st);
                st.step(0.033);
                renderer.render(&gl, &mut st);
                let err = (gl.GetError)();
                assert_eq!(
                    err,
                    gl::GL_NO_ERROR,
                    "Minimal-caps GLES 3.0 error 0x{:X} on theme '{tid}'",
                    err
                );
                (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
                (gl.Finish)();
                (gl.ReadPixels)(
                    0,
                    0,
                    ew,
                    eh,
                    gl::GL_RGBA,
                    gl::GL_UNSIGNED_BYTE,
                    rgba.as_mut_ptr() as *mut c_void,
                );
                drop(_fbo_guard);
                let non_zero = rgba
                    .chunks_exact(4)
                    .filter(|px| px[0] > 4 || px[1] > 4 || px[2] > 4)
                    .count();
                assert!(
                    non_zero > 100,
                    "Minimal-caps GLES 3.0 theme '{tid}' rendered blank (non_zero={non_zero})"
                );
            }

            renderer.destroy(&gl);
            (gl.BindFramebuffer)(gl::GL_FRAMEBUFFER, 0);
            (gl.DeleteRenderbuffers)(1, &depth_rb);
            (gl.DeleteTextures)(1, &tex);
            (gl.DeleteFramebuffers)(1, &fbo);
            drop(ctx);
        }
    }

    #[test]
    fn glsl_330_to_300_es_translator_edge_cases() {
        // 1. Standard 330 core shader
        let s1 = gl::translate_glsl_330_to_300_es("#version 330 core\nin vec2 v_uv;\nvoid main(){}");
        assert!(s1.starts_with(
            "#version 300 es\nprecision highp float;\nprecision highp int;\nprecision highp sampler2D;\nprecision highp isampler2D;\nprecision highp usampler2D;\n"
        ));
        assert!(s1.contains("in vec2 v_uv;"));

        // 2. Preserves #extension before precision declarations
        let s2 = gl::translate_glsl_330_to_300_es(
            "#version 330 core\n#extension GL_OES_standard_derivatives : enable\nuniform float t;\n",
        );
        let ext_pos = s2.find("#extension").unwrap();
        let prec_pos = s2.find("precision highp float;").unwrap();
        assert!(ext_pos < prec_pos, "#extension must appear before precision: {s2}");

        // 3. Already 300 es passthrough (with UTF-8 BOM stripped)
        let s3 = gl::translate_glsl_330_to_300_es("\u{feff}#version 300 es\nprecision mediump float;\nvoid main(){}");
        assert!(s3.starts_with("#version 300 es\nprecision mediump float;"));

        // 4. Missing #version header
        let s4 = gl::translate_glsl_330_to_300_es("void main(){}");
        assert!(s4.starts_with("#version 300 es\nprecision highp float;"));
        assert!(s4.contains("precision highp usampler2D;"));
        assert!(s4.contains("void main(){}"));
    }
}

#[cfg(all(test, target_os = "windows"))]
mod thumb_gen_tests {
    use super::*;
    use crate::gl::*;
    use crate::renderer::Renderer;
    use crate::state::AppState;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExA, DefWindowProcA, DestroyWindow, RegisterClassA, WNDCLASSA, WS_POPUP,
    };

    #[test]
    #[ignore]
    fn render_missing_rust_theme_thumbs() {
        let out_dir = std::env::var_os("DIYARDESK_THUMB_OUT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("ddgl_thumbs"));
        std::fs::create_dir_all(&out_dir).expect("create thumb out dir");

        let only_filter: Option<Vec<String>> = std::env::var("DIYARDESK_THUMB_ONLY")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            })
            .filter(|v: &Vec<String>| !v.is_empty());

        let targets: &[(&str, usize, &str)] = &[
            ("cat-shore", 40, r#"{"t":"settings","v":{"notchMode":"off"}}"#),
            ("persephone", 290, ""),
            ("eye-within-words", 1, ""),
            ("cosmic-eye", 144, ""),
            ("ascii-clownfish", 40, ""),
            ("ascii-jellyfish", 50, ""),
            ("ascii-shark", 40, ""),
            ("ascii-lotus", 50, ""),
            ("ascii-cat-knight", 50, ""),
            ("turkish-flag", 36, r#"{"t":"settings","v":{"wind":1.15}}"#),
        ];

        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarThumbGenTest\0";
            let wc = WNDCLASSA {
                style: 0x0020,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassA(&wc);
            let (pw, ph) = (1280i32, 720i32);
            let hwnd = CreateWindowExA(
                0,
                cls.as_ptr(),
                b"thumbgen\0".as_ptr(),
                WS_POPUP,
                0,
                0,
                64,
                64,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinst,
                std::ptr::null(),
            );
            assert!(!hwnd.is_null(), "CreateWindowExA failed");
            let ctx = WglContext::create(hwnd).expect("WglContext::create failed");
            assert!(ctx.make_current(), "make_current failed");
            let gl = GlFns::load().expect("GlFns::load failed");

            let mut fbo = 0u32;
            let mut tex = 0u32;
            let mut depth_rb = 0u32;
            (gl.GenFramebuffers)(1, &mut fbo);
            (gl.GenTextures)(1, &mut tex);
            (gl.BindTexture)(GL_TEXTURE_2D, tex);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                pw,
                ph,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                std::ptr::null(),
            );
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as GLint);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as GLint);
            (gl.GenRenderbuffers)(1, &mut depth_rb);
            (gl.BindRenderbuffer)(GL_RENDERBUFFER, depth_rb);
            (gl.RenderbufferStorage)(GL_RENDERBUFFER, GL_DEPTH_COMPONENT24, pw, ph);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, fbo);
            (gl.FramebufferTexture2D)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, tex, 0);
            (gl.FramebufferRenderbuffer)(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER, depth_rb);
            assert_eq!((gl.CheckFramebufferStatus)(GL_FRAMEBUFFER), GL_FRAMEBUFFER_COMPLETE);

            let mut renderer = Renderer::new(&gl).expect("Renderer::new failed");
            let mut raw = vec![0u8; (pw * ph * 4) as usize];
            let mut flipped = vec![0u8; (pw * ph * 4) as usize];

            for &(tid, steps, extra_json) in targets {
                if let Some(ref flt) = only_filter {
                    if !flt.iter().any(|x| x == tid) {
                        continue;
                    }
                }
                while (gl.GetError)() != GL_NO_ERROR {}

                let mut st = AppState::new(pw, ph, 1.0);
                st.set_theme(tid);
                st.handle_json(r#"{"t":"icons","vis":false,"r":[]}"#);
                if !extra_json.is_empty() {
                    st.handle_json(extra_json);
                }

                if tid == "eye-within-words" {
                    let mut ready = false;
                    for _ in 0..40 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        st.step(0.05);
                        {
                            let _guard = DefaultFboGuard::new(fbo);
                            (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
                            renderer.render(&gl, &mut st);
                            let err = (gl.GetError)();
                            assert_eq!(err, GL_NO_ERROR, "GL error 0x{:X} on {tid}", err);
                            (gl.Finish)();
                            (gl.ReadPixels)(
                                0,
                                0,
                                pw,
                                ph,
                                GL_RGBA,
                                GL_UNSIGNED_BYTE,
                                raw.as_mut_ptr() as *mut std::ffi::c_void,
                            );
                        }
                        let mut bright_eye = 0usize;
                        for y in 220..500usize {
                            let row_off = y * (pw as usize);
                            for x in 360..920usize {
                                let i = (row_off + x) * 4;
                                if raw[i + 1] >= 180 || raw[i + 2] >= 200 {
                                    bright_eye += 1;
                                }
                            }
                        }
                        if bright_eye > 800 {
                            ready = true;
                            break;
                        }
                    }
                    assert!(ready, "eye-within-words baker did not produce bright eye typography in time");
                } else {
                    for _ in 0..steps {
                        st.step(0.05);
                    }
                    let _guard = DefaultFboGuard::new(fbo);
                    (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
                    renderer.render(&gl, &mut st);
                    let err = (gl.GetError)();
                    assert_eq!(err, GL_NO_ERROR, "GL error 0x{:X} on {tid}", err);
                    (gl.Finish)();
                    (gl.ReadPixels)(
                        0,
                        0,
                        pw,
                        ph,
                        GL_RGBA,
                        GL_UNSIGNED_BYTE,
                        raw.as_mut_ptr() as *mut std::ffi::c_void,
                    );
                }

                crate::flip_rgba_vertical(&raw, &mut flipped, pw as usize, ph as usize);
                let out_path = out_dir.join(format!("{tid}.png"));
                image::save_buffer(&out_path, &flipped, pw as u32, ph as u32, image::ColorType::Rgba8)
                    .unwrap_or_else(|e| panic!("save {tid}.png failed: {e}"));
                eprintln!("[thumb-gen] wrote {}", out_path.display());
            }

            renderer.destroy(&gl);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
            (gl.DeleteRenderbuffers)(1, &depth_rb);
            (gl.DeleteTextures)(1, &tex);
            (gl.DeleteFramebuffers)(1, &fbo);
            drop(ctx);
            DestroyWindow(hwnd);
        }
    }
}
