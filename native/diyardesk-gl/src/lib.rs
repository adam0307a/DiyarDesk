mod ai_mascots;
mod aurora;
mod beach;
mod claymorph;
mod desert;
mod gl;
mod glade;
mod istanbul;
mod liquidglass;
mod meadow;
mod neobrutalism;
mod neumorph;
mod papercut;
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
mod zen_archipelago;
mod voxelcraft;
mod winter;

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
        let is_low = low != 0;
        st.low_mode = is_low;
        st.quiet_snow.set_low_mode(is_low);
        st.claymorph.set_low_mode(is_low);
        st.liquidglass.set_low_mode(is_low);
        st.neobrutalism.set_low_mode(is_low);
        st.solar_system.set_low_mode(is_low);
        st.thinking_orbs.set_low_mode(is_low);
        st.ai_mascots.set_low_mode(is_low);
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
