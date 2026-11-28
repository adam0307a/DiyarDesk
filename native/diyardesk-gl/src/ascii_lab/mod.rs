//! ASCII Lab — Suite of 4 procedural interactive ASCII-art wallpapers ported
//! cleanly to Native Rust + OpenGL 3.3 Core / GLES 3.0 from `sapozhkov/ascii-lab-public`:
//!   1. `ascii-clownfish`: Clownfish & Anemone
//!   2. `ascii-jellyfish`: Jellyfish Heart
//!   3. `ascii-shark`:     Shark Intercept
//!   4. `ascii-lotus`:     Lotus After Rain

pub mod clownfish;
pub mod grid_gl;
pub mod jellyfish;
pub mod lotus;
pub mod lotus_palette;
pub mod shark;

use crate::gl::*;
use crate::state::AppState;
use grid_gl::AsciiGridRenderer;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AsciiLabParams {
    pub palette: usize,
    pub char_size: f32,
    pub speed: f32,
    pub glow: f32,
    pub mouse_interact: bool,
    pub adaptive: bool,
}

impl Default for AsciiLabParams {
    fn default() -> Self {
        Self {
            palette: 0,
            char_size: 7.5,
            speed: 1.0,
            glow: 1.0,
            mouse_interact: true,
            adaptive: true,
        }
    }
}

fn parse_enum(raw: &HashMap<String, Value>, key: &str, names: &[&str], def: usize) -> usize {
    let Some(v) = raw.get(key) else {
        return def;
    };
    match v {
        Value::String(s) => {
            let l = s.trim().to_ascii_lowercase();
            for (i, n) in names.iter().enumerate() {
                if n.eq_ignore_ascii_case(&l) {
                    return i;
                }
            }
            if let Ok(idx) = l.parse::<usize>() {
                return idx.min(names.len() - 1);
            }
            def
        }
        Value::Number(n) => (n.as_i64().unwrap_or(def as i64).max(0) as usize).min(names.len() - 1),
        _ => def,
    }
}

fn parse_f32(raw: &HashMap<String, Value>, key: &str, def: f32, lo: f32, hi: f32) -> f32 {
    let Some(v) = raw.get(key) else {
        return def;
    };
    let n = match v {
        Value::Number(x) => x.as_f64().map(|f| f as f32),
        Value::String(s) => s.trim().parse::<f32>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    };
    match n {
        Some(x) if x.is_finite() => x.clamp(lo, hi),
        _ => def,
    }
}

fn parse_bool(raw: &HashMap<String, Value>, key: &str, def: bool) -> bool {
    let Some(v) = raw.get(key) else {
        return def;
    };
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => {
            let l = s.trim().to_ascii_lowercase();
            l == "true" || l == "1"
        }
        _ => def,
    }
}

impl AsciiLabParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        Self {
            palette: parse_enum(
                raw,
                "palette",
                &["original", "variant1", "variant2", "amber", "sapphire", "neon", "emerald", "sunset", "indigo"],
                0,
            ) % 3,
            char_size: parse_f32(raw, "charSize", 7.5, 5.0, 14.0),
            speed: parse_f32(raw, "speed", 1.0, 0.3, 2.0),
            glow: parse_f32(raw, "glow", 1.0, 0.0, 1.5),
            mouse_interact: parse_bool(raw, "mouseInteract", true),
            adaptive: parse_bool(raw, "adaptive", true),
        }
    }
}

#[inline]
pub(crate) fn clamp_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[inline]
pub(crate) fn scale_rgb(c: [f32; 3], s: f32) -> [f32; 3] {
    [c[0] * s, c[1] * s, c[2] * s]
}

pub struct AsciiLabRenderer {
    grid: AsciiGridRenderer,
}

impl AsciiLabRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        Ok(Self {
            grid: AsciiGridRenderer::new(gl)?,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        self.grid.destroy(gl);
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let dpi = st.dpi_scale.max(0.5);
        let css_w = st.css_w();
        let css_h = st.css_h();

        match st.theme_id.as_str() {
            "ascii-clownfish" => {
                let s = &mut st.ascii_clownfish;
                s.resize(css_w, css_h, dpi);
                self.grid.render(
                    gl,
                    &s.canvas,
                    1,
                    pw,
                    ph,
                    dpi,
                    s.params.glow,
                    0.35,
                    s.params.adaptive,
                    s.icons_vis,
                    &s.icons,
                );
            }
            "ascii-jellyfish" => {
                let s = &mut st.ascii_jellyfish;
                s.resize(css_w, css_h, dpi);
                self.grid.render(
                    gl,
                    &s.canvas,
                    2,
                    pw,
                    ph,
                    dpi,
                    s.params.glow * 1.18,
                    0.30,
                    s.params.adaptive,
                    s.icons_vis,
                    &s.icons,
                );
            }
            "ascii-shark" => {
                let s = &mut st.ascii_shark;
                s.resize(css_w, css_h, dpi);
                self.grid.render(
                    gl,
                    &s.canvas,
                    3,
                    pw,
                    ph,
                    dpi,
                    s.params.glow * 0.48,
                    0.18,
                    s.params.adaptive,
                    s.icons_vis,
                    &s.icons,
                );
            }
            _ => {
                let s = &mut st.ascii_lotus;
                s.resize(css_w, css_h, dpi);
                self.grid.render(
                    gl,
                    &s.canvas,
                    4,
                    pw,
                    ph,
                    dpi,
                    s.params.glow * 0.42,
                    0.12,
                    s.params.adaptive,
                    s.icons_vis,
                    &s.icons,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Rect;

    #[test]
    fn params_parsing_and_clamping() {
        let mut raw = HashMap::new();
        raw.insert("palette".into(), Value::String("variant2".into()));
        raw.insert("charSize".into(), Value::from(22.0));
        raw.insert("speed".into(), Value::from(0.1));
        raw.insert("glow".into(), Value::from(1.25));
        raw.insert("mouseInteract".into(), Value::Bool(false));
        raw.insert("adaptive".into(), Value::from(0));
        let p = AsciiLabParams::from_raw_params(&raw);
        assert_eq!(p.palette, 2);
        assert!((p.char_size - 14.0).abs() < 1e-5);
        assert!((p.speed - 0.3).abs() < 1e-5);
        assert!((p.glow - 1.25).abs() < 1e-5);
        assert!(!p.mouse_interact);
        assert!(!p.adaptive);
    }

    #[test]
    fn math_primitives_match_python_sources() {
        // clownfish.py sway(s=0.85, phase=1.8, effort=0.7, turn=0.12)
        let sw = clownfish::fish_sway(0.85, 1.8, 0.0, 1.0, 0.7, 0.12);
        assert!((sw - (-0.012572158)).abs() < 1e-4, "sway={sw}");

        // jellyfish parametric heart_height(1.2)
        let hh = jellyfish::heart_height_fn(1.2);
        assert!((hh - 0.6314773).abs() < 1e-4, "heart_height={hh}");

        // shark/tail_reaction.py: tail_reaction(0.8, 0.5, 1.2) == 0.21
        let tr = shark::caudal_reaction(0.8, 0.5, 1.2);
        assert!((tr - 0.21).abs() < 1e-6, "tail_reaction={tr}");
    }

    #[test]
    fn all_four_scenes_are_deterministic_and_interactive() {
        let sample_icons = vec![
            Rect { x: 40.0, y: 40.0, w: 72.0, h: 90.0 },
            Rect { x: 40.0, y: 150.0, w: 72.0, h: 90.0 },
        ];

        // 1. Clownfish
        let mut c1 = clownfish::AsciiClownfishState::new(1280.0, 720.0, 1.0);
        let mut c2 = clownfish::AsciiClownfishState::new(1280.0, 720.0, 1.0);
        c1.step(0.033);
        c2.step(0.033);
        assert_eq!(c1.canvas.cells, c2.canvas.cells);
        assert!(c1.handle_mouse(1, 700.0, 340.0));
        c1.set_icons(&sample_icons, true);
        c1.step(0.05);
        assert_ne!(c1.canvas.cells, c2.canvas.cells);

        // 2. Jellyfish
        let mut j1 = jellyfish::AsciiJellyfishState::new(1280.0, 720.0, 1.0);
        let mut j2 = jellyfish::AsciiJellyfishState::new(1280.0, 720.0, 1.0);
        j1.step(0.033);
        j2.step(0.033);
        assert_eq!(j1.canvas.cells, j2.canvas.cells);
        assert!(j1.handle_mouse(1, 640.0, 360.0));
        j1.set_icons(&sample_icons, true);
        j1.step(0.05);
        assert_ne!(j1.canvas.cells, j2.canvas.cells);

        // 3. Shark
        let mut s1 = shark::AsciiSharkState::new(1280.0, 720.0, 1.0);
        let mut s2 = shark::AsciiSharkState::new(1280.0, 720.0, 1.0);
        let (has_catch, cam_scale) = s1.has_capture_and_scale();
        assert!(has_catch, "shark intercept loop must reach chase + capture");
        assert!(cam_scale >= 40.0, "shark cam_scale={cam_scale} < 40");
        s1.step(0.033);
        s2.step(0.033);
        assert_eq!(s1.canvas.cells, s2.canvas.cells);
        assert!(s1.handle_mouse(1, 560.0, 320.0));
        s1.set_icons(&sample_icons, true);
        s1.step(0.05);
        assert_ne!(s1.canvas.cells, s2.canvas.cells);

        // 4. Lotus
        let mut l1 = lotus::AsciiLotusState::new(1280.0, 720.0, 1.0);
        let mut l2 = lotus::AsciiLotusState::new(1280.0, 720.0, 1.0);
        l1.step(0.033);
        l2.step(0.033);
        assert_eq!(l1.canvas.cells, l2.canvas.cells);
        assert!(l1.handle_mouse(1, 640.0, 520.0));
        l1.set_icons(&sample_icons, true);
        l1.step(0.05);
        assert_ne!(l1.canvas.cells, l2.canvas.cells);
    }

    #[test]
    fn resize_edge_cases_and_sub_half_px_guard() {
        let mut st = clownfish::AsciiClownfishState::new(1280.0, 720.0, 1.0);
        let rev0 = st.canvas.rev;
        st.resize(1280.3, 720.3, 1.0);
        assert_eq!(st.canvas.rev, rev0);
        for (w, h) in [(64.0, 64.0), (3440.0, 1440.0), (3840.0, 2160.0)] {
            st.resize(w, h, 1.5);
            st.step(0.02);
            assert!(st.canvas.cols >= 58 && st.canvas.rows >= 38);
        }
    }

    #[test]
    fn atlas_has_clean_padding_and_96_glyphs() {
        let rgba = grid_gl::build_ascii_96_atlas();
        let w = grid_gl::GLYPH_COUNT * grid_gl::TILE_W;
        let h = grid_gl::TILE_H;
        assert_eq!(rgba.len(), w * h * 4);
        // Top & bottom 1 px border strictly zero alpha
        for x in 0..w {
            assert_eq!(rgba[x * 4 + 3], 0);
            assert_eq!(rgba[((h - 1) * w + x) * 4 + 3], 0);
        }
    }

    #[test]
    fn render_speed_budget_and_non_empty_output() {
        let t0 = std::time::Instant::now();
        let mut c = clownfish::AsciiClownfishState::new(1920.0, 1080.0, 1.0);
        c.step(0.033);
        let mut j = jellyfish::AsciiJellyfishState::new(1920.0, 1080.0, 1.0);
        j.step(0.033);
        let mut s = shark::AsciiSharkState::new(1920.0, 1080.0, 1.0);
        s.step(0.033);
        let mut l = lotus::AsciiLotusState::new(1920.0, 1080.0, 1.0);
        l.step(0.033);
        let elapsed_ms = t0.elapsed().as_secs_f32() * 1000.0;
        assert!(elapsed_ms < 2000.0, "init+step took {elapsed_ms:.1}ms");

        for (name, cells) in [
            ("clownfish", &c.canvas.cells),
            ("jellyfish", &j.canvas.cells),
            ("shark", &s.canvas.cells),
            ("lotus", &l.canvas.cells),
        ] {
            let active = cells.chunks_exact(4).filter(|p| p[3] > 0 && (p[0] > 5 || p[1] > 5 || p[2] > 5)).count();
            assert!(active > 400, "scene '{name}' active glyph count too low: {active}");
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn gpu_render_ascii_lab_all_four_pngs() {
        use crate::gl::WglContext;
        use crate::renderer::Renderer;
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExA, DefWindowProcA, DestroyWindow, RegisterClassA, WNDCLASSA, WS_POPUP,
        };

        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarAsciiLabGpuTest\0";
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
                b"asciitest\0".as_ptr(),
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
            let ctx = WglContext::create(hwnd).expect("WglContext failed");
            assert!(ctx.make_current());
            let gl = GlFns::load().expect("GlFns::load failed");

            let mut fbo = 0u32;
            let mut tex = 0u32;
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
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, fbo);
            (gl.FramebufferTexture2D)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, tex, 0);
            assert_eq!((gl.CheckFramebufferStatus)(GL_FRAMEBUFFER), GL_FRAMEBUFFER_COMPLETE);

            let mut renderer = Renderer::new(&gl).expect("Renderer::new failed");
            let mut st = AppState::new(pw, ph, 1.0);
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[48,48,116,136],[48,160,116,248],[144,48,212,136]]}"#);

            let out_dir = std::env::var_os("DIYARDESK_ASCII_VERIFY_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let _ = std::fs::create_dir_all(&out_dir);

            let mut raw = vec![0u8; (pw * ph * 4) as usize];
            let mut flipped = vec![0u8; (pw * ph * 4) as usize];

            let mut prev_frames: Vec<(&str, Vec<u8>)> = Vec::new();
            for tid in ["ascii-clownfish", "ascii-jellyfish", "ascii-shark", "ascii-lotus"] {
                while (gl.GetError)() != GL_NO_ERROR {}
                st.set_theme(tid);
                st.handle_mouse(0, 680.0, 320.0);
                st.handle_mouse(1, 720.0, 340.0);
                st.step(0.05);
                st.step(0.05);
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
                drop(_guard);
                assert!(raw.chunks_exact(4).all(|px| px[3] == 255), "{tid} leaked non-opaque alpha");
                let lit_px = raw.chunks_exact(4).filter(|px| px[0] > 28 || px[1] > 28 || px[2] > 28).count();
                assert!(lit_px > 4000, "{tid} rendered too few lit pixels ({lit_px})");
                for (prev_id, prev_buf) in &prev_frames {
                    assert_ne!(prev_buf.as_slice(), raw.as_slice(), "{tid} identical to {prev_id}");
                }
                prev_frames.push((tid, raw.clone()));

                crate::flip_rgba_vertical(&raw, &mut flipped, pw as usize, ph as usize);
                let png_path = out_dir.join(format!("verify_{}.png", tid.replace('-', "_")));
                image::save_buffer(&png_path, &flipped, pw as u32, ph as u32, image::ColorType::Rgba8)
                    .expect("save verify png");
            }

            if std::env::var_os("DIYARDESK_ASCII_VERIFY_DIR").is_some() {
                st.set_theme("ascii-lotus");
                st.ascii_lotus.reset();
                for &(tag, t_sec) in &[("t0", 0.2f32), ("t3", 3.2), ("t9", 9.2), ("t12", 12.0)] {
                    st.ascii_lotus.anim_time = t_sec;
                    st.ascii_lotus.rebuild_grid();
                    let _guard = DefaultFboGuard::new(fbo);
                    (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
                    renderer.render(&gl, &mut st);
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
                    drop(_guard);
                    crate::flip_rgba_vertical(&raw, &mut flipped, pw as usize, ph as usize);
                    let png_path = out_dir.join(format!("verify_ascii_lotus_{tag}.png"));
                    image::save_buffer(&png_path, &flipped, pw as u32, ph as u32, image::ColorType::Rgba8)
                        .expect("save lotus phase png");
                }

                st.set_theme("ascii-jellyfish");
                st.ascii_jellyfish.reset();
                for &(tag, t_sec) in &[("t0", 0.15f32), ("t5", 5.0), ("t9", 9.0), ("t16", 16.5)] {
                    st.ascii_jellyfish.anim_time = t_sec;
                    st.ascii_jellyfish.rebuild_grid();
                    let _guard = DefaultFboGuard::new(fbo);
                    (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
                    renderer.render(&gl, &mut st);
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
                    drop(_guard);
                    crate::flip_rgba_vertical(&raw, &mut flipped, pw as usize, ph as usize);
                    let png_path = out_dir.join(format!("verify_ascii_jellyfish_{tag}.png"));
                    image::save_buffer(&png_path, &flipped, pw as u32, ph as u32, image::ColorType::Rgba8)
                        .expect("save jellyfish phase png");
                }

                st.set_theme("ascii-shark");
                st.ascii_shark.reset();
                for &(tag, t_sec) in &[("t0", 1.5f32), ("t5", 5.2), ("t7", 7.4), ("t14", 14.0)] {
                    st.ascii_shark.anim_time = t_sec;
                    st.ascii_shark.rebuild_grid();
                    let _guard = DefaultFboGuard::new(fbo);
                    (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
                    renderer.render(&gl, &mut st);
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
                    drop(_guard);
                    crate::flip_rgba_vertical(&raw, &mut flipped, pw as usize, ph as usize);
                    let png_path = out_dir.join(format!("verify_ascii_shark_{tag}.png"));
                    image::save_buffer(&png_path, &flipped, pw as u32, ph as u32, image::ColorType::Rgba8)
                        .expect("save shark phase png");
                }
            }

            renderer.destroy(&gl);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
            (gl.DeleteTextures)(1, &tex);
            (gl.DeleteFramebuffers)(1, &fbo);
            drop(ctx);
            DestroyWindow(hwnd);
        }
    }
}
