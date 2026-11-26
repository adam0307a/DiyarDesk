//! COSMIC ASCII EYE — Native Rust + OpenGL 3.3 generative ASCII-art wallpaper.
//!
//! Seamless infinite loop choreography:
//!   1. Deep inside the black pupil: starry ASCII cosmos with a 3D spinning
//!      banded gas giant (Jupiter/Saturn) and tilted rings.
//!   2. Smooth exponential zoom-out through the pupil and golden radial iris
//!      until the full anatomical eye, lashes, skin, and eyebrow fill the screen.
//!   3. Alive full-eye phase: natural left/right glances, double blinks, and
//!      subtle cursor tracking (clicking also triggers an immediate blink).
//!   4. Smooth exponential zoom back into the pupil returning seamlessly to the
//!      spinning cosmic planet.

pub mod atlas;
pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use atlas::build_ascii_atlas;
use serde_json::Value;
use shaders::COSMIC_EYE_FRAG;
use std::collections::HashMap;
use std::f64::consts::TAU;
use std::ffi::c_void;

pub const CYCLE_SECS: f64 = 18.0;
pub const ZOOM_WIDE: f64 = 1.0;
pub const ZOOM_INSIDE: f64 = 12.6;

// ============================================================================
// 1. THEME PARAMETERS
// ============================================================================

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CosmicEyeParams {
    /// 0 = amberClassic (1:1 warm gold/sepia), 1 = emeraldCosmos, 2 = cyanNebula
    pub palette: usize,
    /// Animation & planet spin speed multiplier (0.3 .. 2.0)
    pub speed: f32,
    /// Base monospace ASCII cell width in CSS pixels (6.0 .. 16.0)
    pub char_size: f32,
    /// Continuous zoom-out -> gaze/blink -> zoom-in cosmic loop
    pub auto_loop: bool,
    /// Subtle cursor-following gaze during the wide-eye phase
    pub mouse_follow: bool,
    /// Subtle dark rounded scrims behind desktop icons
    pub adaptive: bool,
}

impl Default for CosmicEyeParams {
    fn default() -> Self {
        Self {
            palette: 0,
            speed: 1.0,
            char_size: 7.0,
            auto_loop: true,
            mouse_follow: true,
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

impl CosmicEyeParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        Self {
            palette: parse_enum(
                raw,
                "palette",
                &["amberclassic", "emeraldcosmos", "cyannebula"],
                0,
            ),
            speed: parse_f32(raw, "speed", 1.0, 0.3, 2.0),
            char_size: parse_f32(raw, "charSize", 7.0, 5.0, 14.0),
            auto_loop: parse_bool(raw, "autoLoop", true),
            mouse_follow: parse_bool(raw, "mouseFollow", true),
            adaptive: parse_bool(raw, "adaptive", true),
        }
    }
}

// ============================================================================
// 2. SEAMLESS LOOP CHOREOGRAPHY (matches reference animation 1:1)
// ============================================================================

#[inline]
fn smootherstep(x: f64) -> f64 {
    let t = x.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn smooth_blink(t: f64, center: f64, half_w: f64, depth: f64) -> f64 {
    let d = ((t - center) / half_w).abs();
    if d >= 1.0 {
        return 1.0;
    }
    // Faster close, slightly gentler open — natural biological lid curve
    let shaped = if t < center {
        1.0 - smootherstep(1.0 - d)
    } else {
        1.0 - smootherstep(1.0 - d).powf(0.85)
    };
    1.0 - depth * (1.0 - shaped)
}

#[derive(Clone, Copy, Debug)]
pub struct ChoreographySample {
    pub zoom: f64,
    pub gaze_x: f64,
    pub gaze_y: f64,
    pub openness: f64,
}

/// Evaluate exact loop pose for normalized loop phase `u in [0.0, 1.0)`.
/// Guaranteed continuous across the wrap boundary `u = 0.0 == u = 1.0`.
pub fn eval_choreography(phase: f64) -> ChoreographySample {
    let u = phase.rem_euclid(1.0);
    let ln_in = ZOOM_INSIDE.ln();
    let ln_out = ZOOM_WIDE.ln();

    // 1. Exponential camera zoom curve
    let zoom = if u < 0.22 {
        // Inside the dark pupil viewing the spinning planet
        let breath = (u / 0.22 * std::f64::consts::PI).sin();
        ZOOM_INSIDE * (1.0 + 0.025 * breath)
    } else if u < 0.37 {
        // Smooth exponential zoom-out: pupil -> golden iris -> full eye
        let e = smootherstep((u - 0.22) / 0.15);
        (ln_in + (ln_out - ln_in) * e).exp()
    } else if u < 0.77 {
        // Wide full-eye showcase
        let w = (u - 0.37) / 0.40;
        ZOOM_WIDE * (1.0 + 0.015 * (w * std::f64::consts::PI).sin())
    } else if u < 0.92 {
        // Smooth exponential zoom-in back into the pupil cosmos
        let e = smootherstep((u - 0.77) / 0.15);
        (ln_out + (ln_in - ln_out) * e).exp()
    } else {
        // Hold seamlessly inside pupil -> wraps cleanly to u = 0.0
        ZOOM_INSIDE
    };

    // 2. Eye gaze (looks right -> blinks -> looks left -> blinks -> glances right -> centers)
    let (gaze_x, gaze_y, openness) = if u >= 0.37 && u <= 0.77 {
        let w = (u - 0.37) / 0.40; // 0..1 across wide-eye hold

        let gx = if w < 0.12 {
            0.0
        } else if w < 0.24 {
            0.64 * smootherstep((w - 0.12) / 0.12)
        } else if w < 0.37 {
            0.64
        } else if w < 0.50 {
            0.64 - 1.32 * smootherstep((w - 0.37) / 0.13) // +0.64 -> -0.68
        } else if w < 0.67 {
            -0.68
        } else if w < 0.78 {
            -0.68 + 1.08 * smootherstep((w - 0.67) / 0.11) // -0.68 -> +0.40
        } else if w < 0.86 {
            0.40
        } else {
            0.40 * (1.0 - smootherstep((w - 0.86) / 0.14)) // return dead center
        };

        let gy = if w < 0.12 {
            0.0
        } else if w < 0.37 {
            0.07 * smootherstep((w - 0.12) / 0.12)
        } else if w < 0.67 {
            0.07 - 0.13 * smootherstep((w - 0.37) / 0.13)
        } else if w < 0.86 {
            -0.06 + 0.11 * smootherstep((w - 0.67) / 0.11)
        } else {
            0.05 * (1.0 - smootherstep((w - 0.86) / 0.14))
        };

        // Two crisp natural blinks matching frames 13 and 16 of the reference sequence
        let b1 = smooth_blink(w, 0.40, 0.045, 0.94);
        let b2 = smooth_blink(w, 0.71, 0.042, 0.90);
        (gx, gy, b1.min(b2))
    } else {
        (0.0, 0.0, 1.0)
    };

    ChoreographySample {
        zoom,
        gaze_x,
        gaze_y,
        openness,
    }
}

// ============================================================================
// 3. STATE
// ============================================================================

pub struct CosmicEyeState {
    pub params: CosmicEyeParams,
    pub w_css: f32,
    pub h_css: f32,
    pub dpr: f32,
    pub low_mode: bool,
    pub rev: u64,

    pub anim_time: f64,
    pub loop_phase: f64,
    pub planet_rot: f64,

    pub zoom: f64,
    pub gaze_x: f64,
    pub gaze_y: f64,
    pub openness: f64,

    pub pointer_x: f32,
    pub pointer_y: f32,
    pub pointer_active: bool,
    smooth_mouse_x: f64,
    smooth_mouse_y: f64,
    click_blink_t: f64, // < 0 means inactive; 0..0.34s runs a crisp manual blink

    pub icons: Vec<Rect>,
    pub icons_visible: bool,
}

impl Default for CosmicEyeState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0, 1.0)
    }
}

impl CosmicEyeState {
    pub fn new(css_w: f32, css_h: f32, dpi: f32) -> Self {
        let mut st = Self {
            params: CosmicEyeParams::default(),
            w_css: css_w.max(64.0),
            h_css: css_h.max(64.0),
            dpr: dpi.max(0.5),
            low_mode: false,
            rev: 1,
            anim_time: 0.0,
            loop_phase: 0.04, // Start cleanly inside the pupil showing the spinning planet
            planet_rot: 0.0,
            zoom: ZOOM_INSIDE,
            gaze_x: 0.0,
            gaze_y: 0.0,
            openness: 1.0,
            pointer_x: css_w * 0.5,
            pointer_y: css_h * 0.5,
            pointer_active: false,
            smooth_mouse_x: 0.0,
            smooth_mouse_y: 0.0,
            click_blink_t: -1.0,
            icons: Vec::new(),
            icons_visible: false,
        };
        st.step(0.001);
        st
    }

    pub fn resize(&mut self, w: f32, h: f32, dpi: f32) {
        let nw = w.max(64.0);
        let nh = h.max(64.0);
        let ndpi = dpi.max(0.5);
        if (nw - self.w_css).abs() > 0.5
            || (nh - self.h_css).abs() > 0.5
            || (ndpi - self.dpr).abs() > 0.01
        {
            self.w_css = nw;
            self.h_css = nh;
            self.dpr = ndpi;
            self.rev = self.rev.wrapping_add(1);
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.rev = self.rev.wrapping_add(1);
        }
    }

    pub fn sync_params(&mut self, p: CosmicEyeParams) {
        if self.params != p {
            self.params = p;
            self.rev = self.rev.wrapping_add(1);
        }
    }

    pub fn set_icons(&mut self, icons: &[Rect], visible: bool) {
        let same = self.icons_visible == visible
            && self.icons.len() == icons.len()
            && self.icons.iter().zip(icons.iter()).all(|(a, b)| {
                (a.x - b.x).abs() < 0.5
                    && (a.y - b.y).abs() < 0.5
                    && (a.w - b.w).abs() < 0.5
                    && (a.h - b.h).abs() < 0.5
            });
        if same {
            return;
        }
        self.icons_visible = visible;
        self.icons.clear();
        if visible {
            self.icons.extend_from_slice(&icons[..icons.len().min(24)]);
        }
        self.rev = self.rev.wrapping_add(1);
    }

    pub fn handle_mouse(&mut self, kind: i32, x: f32, y: f32) -> bool {
        if kind == 3 {
            self.pointer_active = false;
            return false;
        }
        self.pointer_x = x;
        self.pointer_y = y;
        self.pointer_active = true;
        if kind == 1 {
            self.click_blink_t = 0.0;
            return true;
        }
        false
    }

    pub fn step(&mut self, dt_raw: f32) {
        let dt = (dt_raw as f64).clamp(0.001, 0.05);
        let spd = self.params.speed as f64;
        self.anim_time += dt;
        // Continuous axial rotation of the gas giant (~1 full spin every 11s at 1.0x)
        self.planet_rot = (self.planet_rot + dt * 0.58 * spd).rem_euclid(TAU);

        if self.params.auto_loop {
            self.loop_phase = (self.loop_phase + (dt * spd) / CYCLE_SECS).rem_euclid(1.0);
        }

        // Smooth mouse offset in [-1, 1]
        let (target_mx, target_my) = if self.pointer_active && self.params.mouse_follow {
            (
                ((self.pointer_x / self.w_css.max(1.0)) as f64 - 0.5) * 2.0,
                ((self.pointer_y / self.h_css.max(1.0)) as f64 - 0.5) * 2.0,
            )
        } else {
            (0.0, 0.0)
        };
        let m_alpha = (dt * 4.5).min(1.0);
        self.smooth_mouse_x += (target_mx - self.smooth_mouse_x) * m_alpha;
        self.smooth_mouse_y += (target_my - self.smooth_mouse_y) * m_alpha;

        // Manual click blink overlay
        let click_open = if self.click_blink_t >= 0.0 {
            self.click_blink_t += dt;
            if self.click_blink_t >= 0.34 {
                self.click_blink_t = -1.0;
                1.0
            } else {
                smooth_blink(self.click_blink_t, 0.14, 0.16, 0.96)
            }
        } else {
            1.0
        };

        if self.params.auto_loop {
            let pose = eval_choreography(self.loop_phase);
            self.zoom = pose.zoom;
            // Fade mouse gaze smoothly to 0 whenever zooming into the pupil so
            // the camera always enters dead-center onto the spinning planet.
            let wide_weight = (1.0 - ((self.zoom - ZOOM_WIDE) / 1.2).clamp(0.0, 1.0)).powi(2);
            self.gaze_x = (pose.gaze_x + self.smooth_mouse_x * 0.28 * wide_weight).clamp(-0.92, 0.92);
            self.gaze_y = (pose.gaze_y + self.smooth_mouse_y * 0.28 * wide_weight).clamp(-0.65, 0.65);
            self.openness = pose.openness.min(1.0 - (1.0 - click_open) * wide_weight);
        } else {
            // Static full-eye interactive mode when loop is disabled
            self.zoom += (ZOOM_WIDE - self.zoom) * (dt * 4.0).min(1.0);
            let idle_x = (self.anim_time * 0.42).sin() * 0.22;
            let idle_y = (self.anim_time * 0.31).cos() * 0.06;
            self.gaze_x = (idle_x + self.smooth_mouse_x * 0.55).clamp(-0.92, 0.92);
            self.gaze_y = (idle_y + self.smooth_mouse_y * 0.45).clamp(-0.65, 0.65);
            // Periodic gentle blink every ~4.5s
            let bt = self.anim_time.rem_euclid(4.5);
            let auto_b = smooth_blink(bt, 3.8, 0.18, 0.94);
            self.openness = auto_b.min(click_open);
        }
    }
}

// ============================================================================
// 4. OPENGL 3.3 CORE RENDERER
// ============================================================================

pub struct CosmicEyeRenderer {
    prog: GLuint,
    dummy_vao: GLuint,
    atlas_tex: GLuint,

    u_res: GLint,
    u_time: GLint,
    u_zoom: GLint,
    u_openness: GLint,
    u_gaze: GLint,
    u_planet_rot: GLint,
    u_cell_px: GLint,
    u_palette: GLint,
    u_adaptive: GLint,
    u_icon_count: GLint,
    u_icons: GLint,
    u_atlas: GLint,
}

impl CosmicEyeRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog = gl.compile_program(crate::shaders::FULLSCREEN_VERT, COSMIC_EYE_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let atlas = build_ascii_atlas();
        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            atlas.width as GLsizei,
            atlas.height as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            atlas.rgba.as_ptr() as *const c_void,
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            u_res: gl.uniform_loc(prog, "u_res"),
            u_time: gl.uniform_loc(prog, "u_time"),
            u_zoom: gl.uniform_loc(prog, "u_zoom"),
            u_openness: gl.uniform_loc(prog, "u_openness"),
            u_gaze: gl.uniform_loc(prog, "u_gaze"),
            u_planet_rot: gl.uniform_loc(prog, "u_planet_rot"),
            u_cell_px: gl.uniform_loc(prog, "u_cell_px"),
            u_palette: gl.uniform_loc(prog, "u_palette"),
            u_adaptive: gl.uniform_loc(prog, "u_adaptive"),
            u_icon_count: gl.uniform_loc(prog, "u_icon_count"),
            u_icons: gl.uniform_loc(prog, "u_icons"),
            u_atlas: gl.uniform_loc(prog, "u_atlas"),
            prog,
            dummy_vao,
            atlas_tex,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.atlas_tex != 0 {
            (gl.DeleteTextures)(1, &self.atlas_tex);
            self.atlas_tex = 0;
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
        if self.prog != 0 {
            (gl.DeleteProgram)(self.prog);
            self.prog = 0;
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let dpr = st.dpi_scale.max(0.5);
        let css_w = st.css_w();
        let css_h = st.css_h();

        let cs = &mut st.cosmic_eye;
        cs.resize(css_w, css_h, dpr);

        // Scale ASCII cell cleanly with DPI (slightly coarser in low_mode to save fillrate)
        let low_mult = if cs.low_mode { 1.25 } else { 1.0 };
        let cell_w = (cs.params.char_size * dpr * low_mult).clamp(5.0, 28.0);
        let cell_h = (cell_w * 1.62).round().max(8.0);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.UseProgram)(self.prog);

        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
        (gl.Uniform1i)(self.u_atlas, 0);

        (gl.Uniform2f)(self.u_res, pw as f32, ph as f32);
        (gl.Uniform1f)(self.u_time, cs.anim_time as f32);
        (gl.Uniform1f)(self.u_zoom, cs.zoom as f32);
        (gl.Uniform1f)(self.u_openness, cs.openness as f32);
        (gl.Uniform2f)(self.u_gaze, cs.gaze_x as f32, cs.gaze_y as f32);
        (gl.Uniform1f)(self.u_planet_rot, cs.planet_rot as f32);
        (gl.Uniform2f)(self.u_cell_px, cell_w, cell_h);
        (gl.Uniform1i)(self.u_palette, cs.params.palette as GLint);
        (gl.Uniform1i)(self.u_adaptive, if cs.params.adaptive { 1 } else { 0 });

        let ic_cnt = if cs.icons_visible { cs.icons.len().min(24) } else { 0 };
        (gl.Uniform1i)(self.u_icon_count, ic_cnt as GLint);
        if ic_cnt > 0 {
            let mut ibuf = [0.0f32; 24 * 4];
            for (i, ic) in cs.icons.iter().take(24).enumerate() {
                ibuf[i * 4] = ic.x * dpr;
                ibuf[i * 4 + 1] = ic.y * dpr;
                ibuf[i * 4 + 2] = ic.w * dpr;
                ibuf[i * 4 + 3] = ic.h * dpr;
            }
            (gl.Uniform4fv)(self.u_icons, ic_cnt as GLsizei, ibuf.as_ptr());
        }

        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
        (gl.BindVertexArray)(0);
    }
}

// ============================================================================
// 5. UNIT & GPU RENDER TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_choreography_is_seamless_and_covers_all_phases() {
        let p0 = eval_choreography(0.0);
        let p1 = eval_choreography(0.999999);
        assert!(
            (p0.zoom - p1.zoom).abs() < 1e-3,
            "loop zoom seam must be continuous: {} vs {}",
            p0.zoom,
            p1.zoom
        );
        assert!((p0.gaze_x - p1.gaze_x).abs() < 1e-6);
        assert!((p0.openness - p1.openness).abs() < 1e-6);

        // Inside pupil cosmos phase
        let inside = eval_choreography(0.10);
        assert!(inside.zoom > 12.0, "expected deep pupil zoom, got {}", inside.zoom);

        // Mid zoom-out iris transition
        let mid_out = eval_choreography(0.30);
        assert!(mid_out.zoom > 2.0 && mid_out.zoom < 10.0);

        // Wide eye looking right, blinking, looking left
        let right = eval_choreography(0.37 + 0.40 * 0.30);
        assert!((right.zoom - 1.0).abs() < 0.05);
        assert!(right.gaze_x > 0.5, "expected right gaze, got {}", right.gaze_x);

        let blink = eval_choreography(0.37 + 0.40 * 0.40);
        assert!(blink.openness < 0.15, "expected blink closure, got {}", blink.openness);

        let left = eval_choreography(0.37 + 0.40 * 0.58);
        assert!(left.gaze_x < -0.5, "expected left gaze, got {}", left.gaze_x);

        // Mid zoom-in back to planet
        let mid_in = eval_choreography(0.845);
        assert!(mid_in.zoom > 2.0 && mid_in.zoom < 10.0);
    }

    #[test]
    fn params_parsing_and_resize_threshold() {
        let mut raw = HashMap::new();
        raw.insert("palette".into(), Value::String("cyanNebula".into()));
        raw.insert("speed".into(), Value::from(1.4));
        raw.insert("charSize".into(), Value::from(11));
        raw.insert("autoLoop".into(), Value::from(0));
        raw.insert("mouseFollow".into(), Value::Bool(false));
        raw.insert("adaptive".into(), Value::from(1));
        let p = CosmicEyeParams::from_raw_params(&raw);
        assert_eq!(p.palette, 2);
        assert!((p.speed - 1.4).abs() < 1e-5);
        assert!((p.char_size - 11.0).abs() < 1e-5);
        assert!(!p.auto_loop);
        assert!(!p.mouse_follow);
        assert!(p.adaptive);

        let mut st = CosmicEyeState::new(1280.0, 720.0, 1.0);
        let r0 = st.rev;
        st.resize(1280.2, 720.2, 1.0);
        assert_eq!(st.rev, r0, "sub-0.5px resize must not bump rev");
        st.resize(1920.0, 1080.0, 1.0);
        assert_ne!(st.rev, r0);
    }

    #[test]
    fn ascii_atlas_is_sorted_and_padded() {
        let a = build_ascii_atlas();
        assert_eq!(a.chars_sorted[0], ' ');
        assert_eq!(a.width as usize, atlas::GLYPH_COUNT * atlas::TILE_W);
        assert_eq!(a.height as usize, atlas::TILE_H);
        // Verify 1px top/bottom zero border on every tile
        for x in 0..(a.width as usize) {
            assert_eq!(a.rgba[x * 4 + 3], 0);
            let bot = ((a.height as usize - 1) * (a.width as usize) + x) * 4 + 3;
            assert_eq!(a.rgba[bot], 0);
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn gpu_render_cosmic_eye_all_phases() {
        use std::ffi::CString;
        let _guard = crate::LINUX_EGL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let eng = crate::ddgl_headless_create(1280, 720, 1.0);
            assert!(!eng.is_null());
            let theme_msg = CString::new(r#"{"t":"theme","id":"cosmic-eye"}"#).unwrap();
            crate::ddgl_headless_post_json(eng, theme_msg.as_ptr());

            let mut scratch = vec![0u8; 1280 * 720 * 4];

            // Phase 1: Inside pupil — spinning Jupiter/Saturn (~1s)
            for _ in 0..20 {
                let n = crate::ddgl_headless_render_rgba(eng, 0.05, scratch.as_mut_ptr(), scratch.len() as i32);
                assert_eq!(n, scratch.len() as i32);
            }
            let out_planet = std::env::temp_dir().join("ddgl_cosmic_eye_planet.png");
            let cp = CString::new(out_planet.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.03, cp.as_ptr()), 1);
            assert!(scratch.chunks_exact(4).all(|px| px[3] == 255));
            let warm_planet = scratch
                .chunks_exact(4)
                .filter(|px| px[0] > 110 && px[0] > px[2].saturating_add(20))
                .count();
            assert!(warm_planet > 2_000, "expected warm amber/rust ASCII planet pixels, got {warm_planet}");

            // Phase 2: Mid zoom-out — golden iris & starry pupil (~4.8s total -> u ≈ 0.305)
            for _ in 0..76 {
                let _ = crate::ddgl_headless_render_rgba(eng, 0.05, scratch.as_mut_ptr(), scratch.len() as i32);
            }
            let out_iris = std::env::temp_dir().join("ddgl_cosmic_eye_iris.png");
            let ci = CString::new(out_iris.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.03, ci.as_ptr()), 1);

            // Phase 3: Full wide eye looking right (~7.9s total -> u ≈ 0.48, w ≈ 0.28)
            for _ in 0..62 {
                let _ = crate::ddgl_headless_render_rgba(eng, 0.05, scratch.as_mut_ptr(), scratch.len() as i32);
            }
            let out_wide = std::env::temp_dir().join("ddgl_cosmic_eye_wide.png");
            let cw = CString::new(out_wide.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.03, cw.as_ptr()), 1);

            // Phase 4: Mid-blink (~8.8s total -> u ≈ 0.53, w ≈ 0.40)
            for _ in 0..18 {
                let _ = crate::ddgl_headless_render_rgba(eng, 0.05, scratch.as_mut_ptr(), scratch.len() as i32);
            }
            let out_blink = std::env::temp_dir().join("ddgl_cosmic_eye_blink.png");
            let cb = CString::new(out_blink.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.03, cb.as_ptr()), 1);

            // Phase 5: Looking left (~10.1s total -> u ≈ 0.60, w ≈ 0.58)
            for _ in 0..26 {
                let _ = crate::ddgl_headless_render_rgba(eng, 0.05, scratch.as_mut_ptr(), scratch.len() as i32);
            }
            let out_left = std::env::temp_dir().join("ddgl_cosmic_eye_left.png");
            let cl = CString::new(out_left.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.03, cl.as_ptr()), 1);

            let f_planet = std::fs::read(&out_planet).unwrap();
            let f_iris = std::fs::read(&out_iris).unwrap();
            let f_wide = std::fs::read(&out_wide).unwrap();
            let f_blink = std::fs::read(&out_blink).unwrap();
            let f_left = std::fs::read(&out_left).unwrap();
            assert_ne!(f_planet, f_iris);
            assert_ne!(f_iris, f_wide);
            assert_ne!(f_wide, f_blink);
            assert_ne!(f_wide, f_left);

            crate::ddgl_headless_destroy(eng);
        }
    }
}
