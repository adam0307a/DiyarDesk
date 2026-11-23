mod gl;
mod renderer;
mod shaders;
mod state;
mod studio_glass;
mod text;
mod themes;

use gl::{GlFns, WglContext};
use renderer::Renderer;
use state::AppState;
use std::ffi::{c_char, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::HWND;

pub struct GlEngine {
    state: Arc<Mutex<AppState>>,
    stop_flag: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

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

#[no_mangle]
pub unsafe extern "C" fn ddgl_set_paused(engine: *mut GlEngine, paused: i32) {
    if engine.is_null() { return; }
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        st.paused = paused != 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_set_low_mode(engine: *mut GlEngine, low: i32) {
    if engine.is_null() { return; }
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        st.low_mode = low != 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_post_json(engine: *mut GlEngine, utf8_ptr: *const c_char) {
    if engine.is_null() || utf8_ptr.is_null() { return; }
    let Ok(text) = CStr::from_ptr(utf8_ptr).to_str() else { return };
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        st.handle_json(text);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_mouse(engine: *mut GlEngine, kind: i32, css_x: f32, css_y: f32) -> i32 {
    if engine.is_null() { return 0; }
    let eng = &*engine;
    if let Ok(mut st) = eng.state.lock() {
        return if st.handle_mouse(kind, css_x, css_y) { 1 } else { 0 };
    }
    0
}

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

#[cfg(test)]
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
            drop(ctx);
            DestroyWindow(hwnd);
        }
    }
}
