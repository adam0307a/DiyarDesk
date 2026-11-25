pub mod contour;
pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use crate::text::TextRenderer;
use contour::{
    extract_contours_from_rgba, ExtractedArtwork, TraceVert, TRACE_START,
};
use serde_json::Value;
use shaders::{
    PERS_BG_FRAG, PERS_BG_VERT, PERS_SPARK_FRAG, PERS_SPARK_VERT, PERS_TRACE_FRAG, PERS_TRACE_VERT,
};
use std::collections::HashMap;
use std::f32::consts::PI;
use std::ffi::c_void;

// ============================================================================
// 1. EXACT TIMELINE CONSTANTS & EASING FROM `code.jpg`
// ============================================================================

pub const ZOOM_START: f32 = 1.22;
pub const FILL_START: f32 = 8.0;
pub const FILL_END: f32 = 11.4;
pub const SHIMMER_START: f32 = 11.6;
pub const SHIMMER_DUR: f32 = 1.8;
pub const TEXT_START: f32 = 12.9;
pub const WORD_GAP: f32 = 0.35;
pub const WORD_DUR: f32 = 0.9;
pub const DURATION: f32 = 16.5;
pub const HOLD_AFTER: f32 = 8.5;
pub const FADE_OUT_DUR: f32 = 1.4;
pub const LOOP_TOTAL: f32 = DURATION + HOLD_AFTER + FADE_OUT_DUR;
pub const DUST_COUNT: usize = 140;

const ARTWORK_BYTES: &[u8] = include_bytes!("artwork.jpg");

#[inline]
pub fn ease_in_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x < 0.5 {
        4.0 * x * x * x
    } else {
        1.0 - (-2.0 * x + 2.0).powi(3) * 0.5
    }
}

#[inline]
pub fn ease_sine(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    -((PI * x).cos() - 1.0) * 0.5
}

#[inline]
pub fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x).powi(3)
}

// ============================================================================
// 2. PARAMS (1:1 with prototype/themes.json)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PersephoneParams {
    pub palette: usize,       // 0=moonlight, 1=roseGold, 2=iceCyan
    pub play_mode: usize,     // 0=loop, 1=once
    pub quote_mode: usize,    // 0=pt, 1=tr, 2=en, 3=off
    pub show_code_panel: bool,// default false (true = split-screen code panel like video)
    pub trace_speed: f32,     // 0.4..=2.0 (default 1.0)
    pub spark_intensity: f32, // 0.2..=1.8 (default 1.0)
    pub dust_density: f32,    // 0.0..=1.6 (default 1.0)
    pub parallax: f32,        // 0.0..=1.0 (default 0.45)
    pub adaptive: bool,       // default true
    pub seed: u32,            // default 7 (exact `random.seed(7)` from code.jpg)
}

impl Default for PersephoneParams {
    fn default() -> Self {
        Self {
            palette: 0,
            play_mode: 0,
            quote_mode: 0,
            show_code_panel: false,
            trace_speed: 1.0,
            spark_intensity: 1.0,
            dust_density: 1.0,
            parallax: 0.45,
            adaptive: true,
            seed: 7,
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

impl PersephoneParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        Self {
            palette: parse_enum(raw, "palette", &["moonlight", "rosegold", "icecyan"], 0),
            play_mode: parse_enum(raw, "playMode", &["loop", "once"], 0),
            quote_mode: parse_enum(raw, "quoteMode", &["pt", "tr", "en", "off"], 0),
            show_code_panel: parse_bool(raw, "showCodePanel", false),
            trace_speed: parse_f32(raw, "traceSpeed", 1.0, 0.4, 2.0),
            spark_intensity: parse_f32(raw, "sparkIntensity", 1.0, 0.2, 1.8),
            dust_density: parse_f32(raw, "dustDensity", 1.0, 0.0, 1.6),
            parallax: parse_f32(raw, "parallax", 0.45, 0.0, 1.0),
            adaptive: parse_bool(raw, "adaptive", true),
            seed: parse_f32(raw, "seed", 7.0, 0.0, 999999.0) as u32,
        }
    }
}

// ============================================================================
// 3. STATE (Timeline, 140 Stardust Motes, Click Bursts, Parallax, Icons)
// ============================================================================

#[derive(Clone, Copy)]
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed.wrapping_add(0x9E3779B9))
    }
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }
    fn f32(&mut self) -> f32 {
        (self.next_u32() as f32) / 4294967296.0
    }
}

#[derive(Clone, Copy)]
struct DustMote {
    x: f32, // 0..1 screen norm
    y: f32, // 0..1 screen norm
    vx: f32,
    vy: f32,
    radius: f32,
    phase: f32,
    speed: f32,
    tint: u8,
}

#[derive(Clone, Copy)]
struct ClickSpark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    dur: f32,
    rad: f32,
}

pub struct PersephoneState {
    pub params: PersephoneParams,
    pub w: f32,
    pub h: f32,
    pub dpi: f32,
    pub low_mode: bool,
    pub rev: u64,

    pub timeline_t: f32,
    pub anim_time: f32,
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub pointer_active: bool,
    pub par_x: f32,
    pub par_y: f32,

    pub icons: Vec<Rect>,
    pub icons_visible: bool,

    dust: Vec<DustMote>,
    click_sparks: Vec<ClickSpark>,
}

impl Default for PersephoneState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0, 1.0)
    }
}

impl PersephoneState {
    pub fn new(w: f32, h: f32, dpi: f32) -> Self {
        let mut st = Self {
            params: PersephoneParams::default(),
            w: w.max(64.0),
            h: h.max(64.0),
            dpi: dpi.max(0.5),
            low_mode: false,
            rev: 1,
            timeline_t: 0.0,
            anim_time: 0.0,
            pointer_x: w * 0.5,
            pointer_y: h * 0.5,
            pointer_active: false,
            par_x: 0.0,
            par_y: 0.0,
            icons: Vec::new(),
            icons_visible: false,
            dust: Vec::with_capacity(DUST_COUNT),
            click_sparks: Vec::with_capacity(32),
        };
        st.rebuild_dust(st.params.seed);
        st
    }

    fn rebuild_dust(&mut self, seed: u32) {
        let mut rng = Rng::new(seed ^ 0x51C0FFEE);
        self.dust.clear();
        for _ in 0..DUST_COUNT {
            self.dust.push(DustMote {
                x: rng.f32(),
                y: rng.f32(),
                vx: (rng.f32() - 0.5) * 0.010,
                vy: -0.004 - rng.f32() * 0.011,
                radius: 1.6 + rng.f32() * 3.8,
                phase: rng.f32() * 6.2831853,
                speed: 0.8 + rng.f32() * 1.9,
                tint: (rng.next_u32() % 3) as u8,
            });
        }
    }

    pub fn resize(&mut self, w: f32, h: f32, dpi: f32) {
        let nw = w.max(64.0);
        let nh = h.max(64.0);
        let ndpi = dpi.max(0.5);
        if (nw - self.w).abs() > 0.5 || (nh - self.h).abs() > 0.5 || (ndpi - self.dpi).abs() > 0.01 {
            self.w = nw;
            self.h = nh;
            self.dpi = ndpi;
            self.rev = self.rev.wrapping_add(1);
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        self.low_mode = low;
    }

    pub fn sync_params(&mut self, p: PersephoneParams) {
        if self.params.seed != p.seed {
            self.rebuild_dust(p.seed);
        }
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

    pub fn restart_animation(&mut self) {
        self.timeline_t = 0.0;
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
            // Burst of silver sparkles at click + replay engraving trace when past initial trace phase!
            let mut rng = Rng::new(((x as u32) << 16) ^ (y as u32) ^ (self.anim_time * 100.0) as u32);
            for _ in 0..16 {
                let ang = rng.f32() * 6.2831853;
                let spd = 28.0 + rng.f32() * 95.0;
                if self.click_sparks.len() >= 64 {
                    self.click_sparks.remove(0);
                }
                self.click_sparks.push(ClickSpark {
                    x,
                    y,
                    vx: ang.cos() * spd,
                    vy: ang.sin() * spd,
                    age: 0.0,
                    dur: 0.55 + rng.f32() * 0.55,
                    rad: 4.0 + rng.f32() * 7.0,
                });
            }
            if self.timeline_t > FILL_START {
                self.restart_animation();
            }
            return true;
        }
        false
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.anim_time += dt;
        self.timeline_t += dt * self.params.trace_speed;

        if self.params.play_mode == 0 {
            if self.timeline_t >= LOOP_TOTAL {
                self.timeline_t -= LOOP_TOTAL;
            }
        } else if self.timeline_t > DURATION + 1.0 {
            self.timeline_t = DURATION + 1.0;
        }

        // Smooth mouse parallax
        let (tx, ty) = if self.pointer_active && !self.low_mode {
            (
                ((self.pointer_x / self.w.max(1.0)) - 0.5) * 2.0,
                ((self.pointer_y / self.h.max(1.0)) - 0.5) * 2.0,
            )
        } else {
            (
                (self.anim_time * 0.22).sin() * 0.18,
                (self.anim_time * 0.17).cos() * 0.12,
            )
        };
        let pk = 1.0 - (-dt * 4.5).exp();
        self.par_x += (tx - self.par_x) * pk;
        self.par_y += (ty - self.par_y) * pk;

        // Drift stardust gently upward
        for m in &mut self.dust {
            m.x = (m.x + m.vx * dt).rem_euclid(1.0);
            m.y = (m.y + m.vy * dt).rem_euclid(1.0);
        }

        // Update click sparkles
        for sp in &mut self.click_sparks {
            sp.age += dt;
            sp.x += sp.vx * dt;
            sp.y += sp.vy * dt;
            sp.vx *= 1.0 - dt * 2.4;
            sp.vy *= 1.0 - dt * 2.4;
        }
        self.click_sparks.retain(|s| s.age < s.dur);
    }

    /// Computes `(fill_prog, shimmer_prog, shimmer_amp, zoom, cycle_fade)` from `timeline_t`.
    pub fn phase_values(&self) -> (f32, f32, f32, f32, f32) {
        let t = self.timeline_t;
        let fill_raw = ((t - FILL_START) / (FILL_END - FILL_START)).clamp(0.0, 1.0);
        let fill_prog = ease_in_out(fill_raw);

        let (shimmer_prog, shimmer_amp) = if t >= SHIMMER_START && t <= SHIMMER_START + SHIMMER_DUR {
            let sp = (t - SHIMMER_START) / SHIMMER_DUR;
            let amp = (sp * PI).sin();
            (-0.25 + sp * 1.55, amp)
        } else {
            (-1.0, 0.0)
        };

        // Smooth zoom-out from ZOOM_START (1.22) -> 1.0 over 0..12.8s (matching code.jpg)
        let zoom_u = ease_sine((t / 12.8).clamp(0.0, 1.0));
        let zoom = ZOOM_START + (1.0 - ZOOM_START) * zoom_u;

        // Smooth fade-out at end of loop cycle
        let fade = if self.params.play_mode == 0 && t > DURATION + HOLD_AFTER {
            let f = 1.0 - ((t - (DURATION + HOLD_AFTER)) / FADE_OUT_DUR).clamp(0.0, 1.0);
            ease_sine(f)
        } else {
            1.0
        };

        (fill_prog, shimmer_prog, shimmer_amp, zoom, fade)
    }

    /// Computes portrait destination rectangle `(x, y, w, h)` in CSS px after zoom & parallax.
    pub fn art_rect(&self, art_aspect: f32, zoom: f32) -> [f32; 4] {
        let show_code = self.params.show_code_panel && self.w >= 760.0;
        let stage_w = if show_code { self.w * 0.62 } else { self.w };
        let stage_cx = stage_w * 0.5;
        let stage_cy = self.h * 0.54;

        let max_h = self.h * 0.90;
        let max_w = stage_w * 0.88;
        let mut base_h = max_h;
        let mut base_w = base_h * art_aspect;
        if base_w > max_w {
            base_w = max_w;
            base_h = base_w / art_aspect.max(0.1);
        }

        let w = base_w * zoom;
        let h = base_h * zoom;
        let px_off = self.par_x * 16.0 * self.params.parallax;
        let py_off = self.par_y * 12.0 * self.params.parallax;
        let x = stage_cx - w * 0.5 + px_off;
        let y = stage_cy - h * 0.5 + py_off;
        [x, y, w, h]
    }
}

// ============================================================================
// 4. OPENGL 3.3 CORE RENDERER
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy)]
struct SparkInst {
    x: f32,
    y: f32,
    rad: f32,
    kind: f32, // 0=tip spark, 1=4-pt twinkle, 2=stardust
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

pub struct PersephoneRenderer {
    prog_bg: GLuint,
    prog_trace: GLuint,
    prog_spark: GLuint,
    dummy_vao: GLuint,
    art_tex: GLuint,
    art_aspect: f32,

    trace_vao: GLuint,
    trace_vbo: GLuint,
    trace_vert_count: GLsizei,

    spark_vao: GLuint,
    spark_vbo: GLuint,
    spark_buf: Vec<SparkInst>,

    extracted: ExtractedArtwork,
    built_seed: u32,
    art_rgba_small: Vec<u8>,
    art_small_w: usize,
    art_small_h: usize,

    // Cached BG uniforms
    u_bg_art: GLint,
    u_bg_res: GLint,
    u_bg_art_rect: GLint,
    u_bg_focus_uv: GLint,
    u_bg_halo_uv: GLint,
    u_bg_time: GLint,
    u_bg_anim_time: GLint,
    u_bg_fill_prog: GLint,
    u_bg_shimmer_prog: GLint,
    u_bg_shimmer_amp: GLint,
    u_bg_fade: GLint,
    u_bg_palette: GLint,
    u_bg_adaptive: GLint,
    u_bg_icon_count: GLint,
    u_bg_icons: GLint,

    // Cached Trace uniforms
    u_tr_res: GLint,
    u_tr_art_rect: GLint,
    u_tr_half_w: GLint,
    u_tr_time: GLint,
    u_tr_fill_prog: GLint,
    u_tr_shimmer_prog: GLint,
    u_tr_shimmer_amp: GLint,
    u_tr_fade: GLint,
    u_tr_palette: GLint,

    // Cached Spark uniform
    u_sp_res: GLint,
}

impl PersephoneRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_bg = gl.compile_program(PERS_BG_VERT, PERS_BG_FRAG)?;
        let prog_trace = gl.compile_program(PERS_TRACE_VERT, PERS_TRACE_FRAG)?;
        let prog_spark = gl.compile_program(PERS_SPARK_VERT, PERS_SPARK_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        // Decode embedded high-detail silver-engraving artwork
        let img = image::load_from_memory(ARTWORK_BYTES)
            .map_err(|e| format!("Failed to decode persephone artwork: {e}"))?;
        let rgba_img = img.to_rgba8();
        let (aw, ah) = rgba_img.dimensions();
        let art_aspect = (aw as f32) / (ah.max(1) as f32);

        let mut art_tex = 0;
        (gl.GenTextures)(1, &mut art_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, art_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            aw as GLsizei,
            ah as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            rgba_img.as_raw().as_ptr() as *const c_void,
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        // Downscale cleanly to ~560px wide for crisp, fast Canny contour extraction
        let target_w = aw.min(560);
        let target_h = ((target_w as f32 / art_aspect).round() as u32).max(64);
        let small = image::imageops::resize(
            &rgba_img,
            target_w,
            target_h,
            image::imageops::FilterType::Triangle,
        );
        let art_small_w = target_w as usize;
        let art_small_h = target_h as usize;
        let art_rgba_small = small.into_raw();

        let focus_uv = [0.60, 0.295];
        let extracted = extract_contours_from_rgba(&art_rgba_small, art_small_w, art_small_h, focus_uv, 7);

        // Build contour ribbon VAO + VBO
        let mut trace_vao = 0;
        let mut trace_vbo = 0;
        (gl.GenVertexArrays)(1, &mut trace_vao);
        (gl.GenBuffers)(1, &mut trace_vbo);
        (gl.BindVertexArray)(trace_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, trace_vbo);
        let t_stride = std::mem::size_of::<TraceVert>() as GLsizei;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (extracted.verts.len() * std::mem::size_of::<TraceVert>()) as GLsizeiptr,
            extracted.verts.as_ptr() as *const c_void,
            GL_STATIC_DRAW,
        );
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, t_stride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, t_stride, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 1, GL_FLOAT, GL_FALSE, t_stride, 16 as *const c_void);
        (gl.EnableVertexAttribArray)(3);
        (gl.VertexAttribPointer)(3, 1, GL_FLOAT, GL_FALSE, t_stride, 20 as *const c_void);
        (gl.EnableVertexAttribArray)(4);
        (gl.VertexAttribPointer)(4, 1, GL_FLOAT, GL_FALSE, t_stride, 24 as *const c_void);
        (gl.BindVertexArray)(0);

        let trace_vert_count = extracted.verts.len() as GLsizei;

        // Build instanced spark/twinkle/stardust VAO + VBO
        let mut spark_vao = 0;
        let mut spark_vbo = 0;
        (gl.GenVertexArrays)(1, &mut spark_vao);
        (gl.GenBuffers)(1, &mut spark_vbo);
        (gl.BindVertexArray)(spark_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, spark_vbo);
        let s_stride = std::mem::size_of::<SparkInst>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 4, GL_FLOAT, GL_FALSE, s_stride, 0 as *const c_void);
        (gl.VertexAttribDivisor)(0, 1);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, s_stride, 16 as *const c_void);
        (gl.VertexAttribDivisor)(1, 1);
        (gl.BindVertexArray)(0);

        Ok(Self {
            u_bg_art: gl.uniform_loc(prog_bg, "u_art_tex"),
            u_bg_res: gl.uniform_loc(prog_bg, "u_res"),
            u_bg_art_rect: gl.uniform_loc(prog_bg, "u_art_rect"),
            u_bg_focus_uv: gl.uniform_loc(prog_bg, "u_focus_uv"),
            u_bg_halo_uv: gl.uniform_loc(prog_bg, "u_halo_uv"),
            u_bg_time: gl.uniform_loc(prog_bg, "u_time"),
            u_bg_anim_time: gl.uniform_loc(prog_bg, "u_anim_time"),
            u_bg_fill_prog: gl.uniform_loc(prog_bg, "u_fill_prog"),
            u_bg_shimmer_prog: gl.uniform_loc(prog_bg, "u_shimmer_prog"),
            u_bg_shimmer_amp: gl.uniform_loc(prog_bg, "u_shimmer_amp"),
            u_bg_fade: gl.uniform_loc(prog_bg, "u_fade"),
            u_bg_palette: gl.uniform_loc(prog_bg, "u_palette"),
            u_bg_adaptive: gl.uniform_loc(prog_bg, "u_adaptive"),
            u_bg_icon_count: gl.uniform_loc(prog_bg, "u_icon_count"),
            u_bg_icons: gl.uniform_loc(prog_bg, "u_icons"),

            u_tr_res: gl.uniform_loc(prog_trace, "u_res"),
            u_tr_art_rect: gl.uniform_loc(prog_trace, "u_art_rect"),
            u_tr_half_w: gl.uniform_loc(prog_trace, "u_line_half_w"),
            u_tr_time: gl.uniform_loc(prog_trace, "u_time"),
            u_tr_fill_prog: gl.uniform_loc(prog_trace, "u_fill_prog"),
            u_tr_shimmer_prog: gl.uniform_loc(prog_trace, "u_shimmer_prog"),
            u_tr_shimmer_amp: gl.uniform_loc(prog_trace, "u_shimmer_amp"),
            u_tr_fade: gl.uniform_loc(prog_trace, "u_fade"),
            u_tr_palette: gl.uniform_loc(prog_trace, "u_palette"),

            u_sp_res: gl.uniform_loc(prog_spark, "u_res"),

            prog_bg,
            prog_trace,
            prog_spark,
            dummy_vao,
            art_tex,
            art_aspect,
            trace_vao,
            trace_vbo,
            trace_vert_count,
            spark_vao,
            spark_vbo,
            spark_buf: Vec::with_capacity(1024),
            extracted,
            built_seed: 7,
            art_rgba_small,
            art_small_w,
            art_small_h,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.art_tex != 0 { (gl.DeleteTextures)(1, &self.art_tex); self.art_tex = 0; }
        if self.trace_vbo != 0 { (gl.DeleteBuffers)(1, &self.trace_vbo); self.trace_vbo = 0; }
        if self.trace_vao != 0 { (gl.DeleteVertexArrays)(1, &self.trace_vao); self.trace_vao = 0; }
        if self.spark_vbo != 0 { (gl.DeleteBuffers)(1, &self.spark_vbo); self.spark_vbo = 0; }
        if self.spark_vao != 0 { (gl.DeleteVertexArrays)(1, &self.spark_vao); self.spark_vao = 0; }
        if self.dummy_vao != 0 { (gl.DeleteVertexArrays)(1, &self.dummy_vao); self.dummy_vao = 0; }
        if self.prog_bg != 0 { (gl.DeleteProgram)(self.prog_bg); self.prog_bg = 0; }
        if self.prog_trace != 0 { (gl.DeleteProgram)(self.prog_trace); self.prog_trace = 0; }
        if self.prog_spark != 0 { (gl.DeleteProgram)(self.prog_spark); self.prog_spark = 0; }
    }

    unsafe fn ensure_seed(&mut self, gl: &GlFns, seed: u32) {
        if self.built_seed == seed {
            return;
        }
        self.extracted = extract_contours_from_rgba(
            &self.art_rgba_small,
            self.art_small_w,
            self.art_small_h,
            [0.60, 0.295],
            seed,
        );
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.trace_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.extracted.verts.len() * std::mem::size_of::<TraceVert>()) as GLsizeiptr,
            self.extracted.verts.as_ptr() as *const c_void,
            GL_STATIC_DRAW,
        );
        self.trace_vert_count = self.extracted.verts.len() as GLsizei;
        self.built_seed = seed;
    }

    fn iri_rgb(palette: usize, idx: u8) -> [f32; 3] {
        match palette {
            1 => match idx % 3 {
                0 => [1.00, 0.82, 0.74],
                1 => [1.00, 0.78, 0.90],
                _ => [0.98, 0.92, 0.80],
            },
            2 => match idx % 3 {
                0 => [0.72, 0.94, 1.00],
                1 => [0.80, 0.88, 1.00],
                _ => [0.90, 1.00, 0.98],
            },
            _ => match idx % 3 {
                // Exact code.jpg IRI = [(255,222,240), (226,214,255), (210,232,255)]
                0 => [1.00, 222.0 / 255.0, 240.0 / 255.0],
                1 => [226.0 / 255.0, 214.0 / 255.0, 1.00],
                _ => [210.0 / 255.0, 232.0 / 255.0, 1.00],
            },
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState, text: &mut TextRenderer) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.5);

        let ps = &mut st.persephone;
        ps.resize(css_w, css_h, dpi);
        self.ensure_seed(gl, ps.params.seed);

        let (fill_prog, shimmer_prog, shimmer_amp, zoom, fade) = ps.phase_values();
        let art_rect = ps.art_rect(self.art_aspect, zoom);
        let t = ps.timeline_t;
        let pal = ps.params.palette;

        // --------------------------------------------------------------------
        // PASS 1: Obsidian sky + Volumetric God-Rays + Sculpted Silver Fill
        // --------------------------------------------------------------------
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.UseProgram)(self.prog_bg);

        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.art_tex);
        (gl.Uniform1i)(self.u_bg_art, 0);
        (gl.Uniform2f)(self.u_bg_res, css_w, css_h);
        (gl.Uniform4f)(self.u_bg_art_rect, art_rect[0], art_rect[1], art_rect[2], art_rect[3]);
        (gl.Uniform2f)(self.u_bg_focus_uv, self.extracted.focus_uv[0], self.extracted.focus_uv[1]);
        (gl.Uniform2f)(self.u_bg_halo_uv, 0.50, 0.275);
        (gl.Uniform1f)(self.u_bg_time, t);
        (gl.Uniform1f)(self.u_bg_anim_time, ps.anim_time);
        (gl.Uniform1f)(self.u_bg_fill_prog, fill_prog);
        (gl.Uniform1f)(self.u_bg_shimmer_prog, shimmer_prog);
        (gl.Uniform1f)(self.u_bg_shimmer_amp, shimmer_amp);
        (gl.Uniform1f)(self.u_bg_fade, fade);
        (gl.Uniform1i)(self.u_bg_palette, pal as GLint);
        (gl.Uniform1i)(self.u_bg_adaptive, if ps.params.adaptive { 1 } else { 0 });

        let ic_cnt = if ps.icons_visible { ps.icons.len().min(24) } else { 0 };
        (gl.Uniform1i)(self.u_bg_icon_count, ic_cnt as GLint);
        if ic_cnt > 0 {
            let mut ibuf = [0.0f32; 24 * 4];
            for (i, ic) in ps.icons.iter().take(24).enumerate() {
                ibuf[i * 4] = ic.x + ic.w * 0.5;
                ibuf[i * 4 + 1] = ic.y + ic.h * 0.5;
                ibuf[i * 4 + 2] = ic.w * 0.5;
                ibuf[i * 4 + 3] = ic.h * 0.5;
            }
            (gl.Uniform4fv)(self.u_bg_icons, ic_cnt as GLsizei, ibuf.as_ptr());
        }

        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // --------------------------------------------------------------------
        // PASS 2: Progressive Silver Contour Line-Ribbons (all segments in 1 call!)
        // --------------------------------------------------------------------
        if self.trace_vert_count > 0 && t >= TRACE_START {
            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
            (gl.UseProgram)(self.prog_trace);
            (gl.Uniform2f)(self.u_tr_res, css_w, css_h);
            (gl.Uniform4f)(self.u_tr_art_rect, art_rect[0], art_rect[1], art_rect[2], art_rect[3]);
            let lw = (art_rect[3] / 820.0).clamp(0.75, 1.45) * 1.15;
            (gl.Uniform1f)(self.u_tr_half_w, lw);
            (gl.Uniform1f)(self.u_tr_time, t);
            (gl.Uniform1f)(self.u_tr_fill_prog, fill_prog);
            (gl.Uniform1f)(self.u_tr_shimmer_prog, shimmer_prog);
            (gl.Uniform1f)(self.u_tr_shimmer_amp, shimmer_amp);
            (gl.Uniform1f)(self.u_tr_fade, fade);
            (gl.Uniform1i)(self.u_tr_palette, pal as GLint);
            (gl.BindVertexArray)(self.trace_vao);
            (gl.DrawArrays)(GL_TRIANGLES, 0, self.trace_vert_count);
            (gl.BindVertexArray)(0);
        }

        // --------------------------------------------------------------------
        // PASS 3: Additive Starlight (140 Stardust + Blazing Trace Tips + 45 Twinkles!)
        // Strictly preserves destination alpha = 1.0 via BlendFuncSeparate(SRC_ALPHA, ONE, ZERO, ONE)!
        // --------------------------------------------------------------------
        self.spark_buf.clear();

        // 3a. 140 floating iridescent stardust motes (`DUST = 140`)
        let dust_lim = ((ps.dust.len() as f32) * ps.params.dust_density.clamp(0.0, 1.6))
            .round() as usize;
        let dust_lim = dust_lim.min(ps.dust.len());
        for m in ps.dust.iter().take(dust_lim) {
            let twinkle = 0.45 + 0.55 * (ps.anim_time * m.speed + m.phase).sin();
            let c = Self::iri_rgb(pal, m.tint);
            let px = m.x * css_w + ps.par_x * 26.0 * ps.params.parallax;
            let py = m.y * css_h + ps.par_y * 20.0 * ps.params.parallax;
            self.spark_buf.push(SparkInst {
                x: px,
                y: py,
                rad: m.radius * (0.85 + 0.35 * twinkle),
                kind: 2.0,
                r: c[0],
                g: c[1],
                b: c[2],
                a: (0.20 + 0.42 * twinkle) * ps.params.spark_intensity.clamp(0.2, 1.8),
            });
        }

        // 3b. Active Contour-Tracing Blazing Star Sparks (frames 3, 8, 12 magic!)
        let spark_scale = (art_rect[3] / 820.0).clamp(0.65, 1.4) * ps.params.spark_intensity;
        if t >= TRACE_START && t < FILL_END + 0.5 {
            for seg in &self.extracted.segments {
                if t < seg.t_start || t > seg.t_start + seg.t_dur + 0.25 {
                    continue;
                }
                let u = ((t - seg.t_start) / seg.t_dur).clamp(0.0, 1.0);
                let tail_fade = if t > seg.t_start + seg.t_dur {
                    1.0 - ((t - (seg.t_start + seg.t_dur)) / 0.25).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let head_fade = ((t - seg.t_start) / 0.08).clamp(0.0, 1.0);
                let fidx = u * ((seg.pts.len() - 1) as f32);
                let i0 = (fidx.floor() as usize).min(seg.pts.len() - 1);
                let i1 = (i0 + 1).min(seg.pts.len() - 1);
                let frac = fidx - (i0 as f32);
                let px_u = seg.pts[i0][0] + (seg.pts[i1][0] - seg.pts[i0][0]) * frac;
                let px_v = seg.pts[i0][1] + (seg.pts[i1][1] - seg.pts[i0][1]) * frac;

                let sx = art_rect[0] + px_u * art_rect[2];
                let sy = art_rect[1] + px_v * art_rect[3];
                let a = head_fade
                    * tail_fade
                    * (0.34 + 0.34 * seg.strength)
                    * (1.0 - fill_prog * 0.72)
                    * fade;
                let col = match pal {
                    1 => [1.00, 0.90, 0.93],
                    2 => [0.85, 0.96, 1.00],
                    _ => [235.0 / 255.0, 242.0 / 255.0, 1.00],
                };
                self.spark_buf.push(SparkInst {
                    x: sx,
                    y: sy,
                    rad: (3.6 + 4.4 * seg.strength) * spark_scale,
                    kind: 0.0,
                    r: col[0],
                    g: col[1],
                    b: col[2],
                    a: a.clamp(0.0, 1.0),
                });
            }
        }

        // 3c. 45 Celestial 4-Point Twinkle Stars locked to bright highlights (`TWINKLES = 45`)
        if fill_prog > 0.15 {
            for tw in &self.extracted.twinkles {
                let wave = (ps.anim_time * tw.speed + tw.phase).sin() * 0.5 + 0.5;
                let pulse = wave.powf(3.2);
                if pulse < 0.04 {
                    continue;
                }
                let c = Self::iri_rgb(pal, tw.tint_idx);
                self.spark_buf.push(SparkInst {
                    x: art_rect[0] + tw.u * art_rect[2],
                    y: art_rect[1] + tw.v * art_rect[3],
                    rad: (6.0 + 9.0 * tw.size * pulse) * spark_scale,
                    kind: 1.0,
                    r: c[0],
                    g: c[1],
                    b: c[2],
                    a: pulse * fill_prog * 0.88 * fade,
                });
            }
        }

        // 3d. Interactive Click Sparkles
        for sp in &ps.click_sparks {
            let life = 1.0 - (sp.age / sp.dur).clamp(0.0, 1.0);
            let c = Self::iri_rgb(pal, 1);
            self.spark_buf.push(SparkInst {
                x: sp.x,
                y: sp.y,
                rad: sp.rad * (0.6 + 0.7 * life),
                kind: 1.0,
                r: c[0],
                g: c[1],
                b: c[2],
                a: life * life * 0.95,
            });
        }

        if !self.spark_buf.is_empty() {
            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE, GL_ZERO, GL_ONE);
            (gl.UseProgram)(self.prog_spark);
            (gl.Uniform2f)(self.u_sp_res, css_w, css_h);
            (gl.BindVertexArray)(self.spark_vao);
            (gl.BindBuffer)(GL_ARRAY_BUFFER, self.spark_vbo);
            (gl.BufferData)(
                GL_ARRAY_BUFFER,
                (self.spark_buf.len() * std::mem::size_of::<SparkInst>()) as GLsizeiptr,
                self.spark_buf.as_ptr() as *const c_void,
                GL_DYNAMIC_DRAW,
            );
            (gl.DrawArraysInstanced)(
                GL_TRIANGLE_STRIP,
                0,
                4,
                self.spark_buf.len() as GLsizei,
            );
            (gl.BindVertexArray)(0);
        }

        // --------------------------------------------------------------------
        // PASS 4: Poetic Word-by-Word Quote (`TEXT_START = 12.9`) + Optional Code Sidebar
        // --------------------------------------------------------------------
        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
        text.begin();

        if ps.params.quote_mode != 3 && t >= TEXT_START {
            let lines: [&str; 2] = match ps.params.quote_mode {
                1 => ["Sürgün dediler.", "O buna krallık dedi."],
                2 => ["They called it exile.", "She called it a kingdom."],
                _ => ["Chamaram de sequestro.", "Ela chamou de reino."],
            };
            let font_sz = (art_rect[3] * 0.038).clamp(18.0, 34.0);
            let quote_cx = art_rect[0] + art_rect[2] * 0.5;
            let quote_y0 = (art_rect[1] + art_rect[3] * 0.065).max(font_sz * 1.45);
            let line_h = font_sz * 1.38;

            let mut word_idx = 0usize;
            for (li, line) in lines.iter().enumerate() {
                let words: Vec<&str> = line.split_whitespace().collect();
                let space_w = text.measure(gl, " ", font_sz, dpi, false);
                let mut widths = Vec::with_capacity(words.len());
                let mut total_w = 0.0f32;
                for (wi, w_str) in words.iter().enumerate() {
                    let ww = text.measure(gl, w_str, font_sz, dpi, false);
                    widths.push(ww);
                    total_w += ww + if wi + 1 < words.len() { space_w } else { 0.0 };
                }

                let mut pen_x = quote_cx - total_w * 0.5;
                let base_y = quote_y0 + (li as f32) * line_h;
                for (wi, w_str) in words.iter().enumerate() {
                    let wt = TEXT_START + (word_idx as f32) * WORD_GAP;
                    let prog = ease_out(((t - wt) / WORD_DUR).clamp(0.0, 1.0));
                    if prog > 0.005 {
                        let dy = (1.0 - prog) * 10.0;
                        let a = prog * fade;
                        // Subtle silver-violet drop shadow for legibility against god-rays
                        text.push(
                            gl,
                            w_str,
                            pen_x,
                            base_y + dy + 2.0,
                            font_sz,
                            dpi,
                            false,
                            [0.04, 0.03, 0.09, a * 0.72],
                            0,
                            None,
                        );
                        text.push(
                            gl,
                            w_str,
                            pen_x,
                            base_y + dy,
                            font_sz,
                            dpi,
                            false,
                            [0.92, 0.94, 0.99, a * 0.96],
                            0,
                            None,
                        );
                    }
                    pen_x += widths[wi] + space_w;
                    word_idx += 1;
                }
            }
        }

        // Optional Split-Screen Live Code Sidebar (`showCodePanel = true`)
        if ps.params.show_code_panel && css_w >= 760.0 {
            self.render_code_sidebar(gl, ps, text, css_w, css_h, dpi);
        }

        text.flush(gl, css_w, css_h);
        (gl.Disable)(GL_BLEND);
    }

    unsafe fn render_code_sidebar(
        &self,
        gl: &GlFns,
        ps: &PersephoneState,
        text: &mut TextRenderer,
        css_w: f32,
        css_h: f32,
        dpi: f32,
    ) {
        let sx = css_w * 0.64;
        let top_y = css_h * 0.08;
        let fs = (css_h * 0.0155).clamp(11.0, 15.0);
        let lh = fs * 1.48;
        let t = ps.timeline_t;

        let active_phase = if t < FILL_START {
            1 // tracing
        } else if t < SHIMMER_START {
            2 // fill
        } else if t < TEXT_START {
            3 // shimmer
        } else {
            4 // quote
        };

        let code_lines: [(usize, &str, [f32; 3]); 24] = [
            (0, "// DiyarDesk · Rust + OpenGL 3.3 Core", [0.46, 0.52, 0.60]),
            (0, "const BG: (u8, u8, u8) = (8, 5, 15);", [0.68, 0.80, 0.94]),
            (0, "const ZOOM: f32 = 1.22;", [0.82, 0.72, 0.96]),
            (1, "const TRACE_START: f32 = 0.25;", [0.56, 0.84, 0.78]),
            (1, "const TRACE_SPREAD: f32 = 6.0;", [0.56, 0.84, 0.78]),
            (1, "const SEG_LEN: usize = 140;", [0.56, 0.84, 0.78]),
            (1, "const SEG_DUR: (f32, f32) = (2.0, 3.2);", [0.56, 0.84, 0.78]),
            (2, "const FILL_START: f32 = 8.0;", [0.95, 0.82, 0.62]),
            (2, "const FILL_END: f32 = 11.4;", [0.95, 0.82, 0.62]),
            (3, "const SHIMMER_START: f32 = 11.6;", [0.96, 0.76, 0.88]),
            (3, "const SHIMMER_DUR: f32 = 1.8;", [0.96, 0.76, 0.88]),
            (4, "const TEXT_START: f32 = 12.9;", [0.80, 0.88, 0.98]),
            (4, "const WORD_GAP: f32 = 0.35;", [0.80, 0.88, 0.98]),
            (0, "const TWINKLES: usize = 45;", [0.70, 0.76, 0.88]),
            (0, "const DUST: usize = 140;", [0.70, 0.76, 0.88]),
            (0, "const DURATION: f32 = 16.5;", [0.70, 0.76, 0.88]),
            (0, "", [0.5, 0.5, 0.5]),
            (0, "fn ease_in_out(x: f32) -> f32 {", [0.76, 0.64, 0.94]),
            (0, "  if x < 0.5 { 4.0*x*x*x }", [0.74, 0.78, 0.84]),
            (0, "  else { 1.0 - (-2.0*x + 2.0).powi(3)/2.0 }", [0.74, 0.78, 0.84]),
            (0, "}", [0.76, 0.64, 0.94]),
            (0, "fn ease_sine(x: f32) -> f32 {", [0.76, 0.64, 0.94]),
            (0, "  -((PI * x).cos() - 1.0) * 0.5", [0.74, 0.78, 0.84]),
            (0, "}", [0.76, 0.64, 0.94]),
        ];

        for (i, (phase_id, line, col)) in code_lines.iter().enumerate() {
            let y = top_y + (i as f32) * lh;
            if y > css_h - 40.0 {
                break;
            }
            let is_active = *phase_id != 0 && *phase_id == active_phase;
            let a = if is_active { 0.96 } else { 0.52 };
            let num = format!("{:2}", i + 1);
            text.push(
                gl,
                &num,
                sx,
                y,
                fs * 0.9,
                dpi,
                false,
                [0.36, 0.40, 0.48, a * 0.75],
                0,
                None,
            );
            text.push(
                gl,
                line,
                sx + fs * 2.2,
                y,
                fs,
                dpi,
                is_active,
                [col[0], col[1], col[2], a],
                0,
                Some(css_w - sx - fs * 3.0),
            );
        }

        // Live timeline counter at bottom of code sidebar
        let status = format!("t = {:.2}s / {:.1}s  ·  segs = {}", t.min(DURATION), DURATION, self.extracted.segments.len());
        text.push(
            gl,
            &status,
            sx + fs * 2.2,
            (top_y + 25.5 * lh).min(css_h - 24.0),
            fs * 0.92,
            dpi,
            true,
            [0.74, 0.82, 0.96, 0.78],
            0,
            None,
        );
    }
}

// ============================================================================
// 5. UNIT & GPU RENDER TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_and_params_match_spec() {
        assert!((ease_in_out(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_in_out(0.5) - 0.5).abs() < 1e-6);
        assert!((ease_in_out(1.0) - 1.0).abs() < 1e-6);
        assert!((ease_sine(0.5) - 0.5).abs() < 1e-6);
        assert!((ease_out(1.0) - 1.0).abs() < 1e-6);

        let mut raw = HashMap::new();
        raw.insert("palette".into(), Value::String("roseGold".into()));
        raw.insert("playMode".into(), Value::String("once".into()));
        raw.insert("quoteMode".into(), Value::String("tr".into()));
        raw.insert("showCodePanel".into(), Value::Bool(true));
        raw.insert("traceSpeed".into(), Value::from(1.5));
        raw.insert("sparkIntensity".into(), Value::from(1.4));
        raw.insert("dustDensity".into(), Value::from(0.8));
        raw.insert("parallax".into(), Value::from(0.7));
        raw.insert("adaptive".into(), Value::Bool(false));
        raw.insert("seed".into(), Value::from(42));

        let p = PersephoneParams::from_raw_params(&raw);
        assert_eq!(p.palette, 1);
        assert_eq!(p.play_mode, 1);
        assert_eq!(p.quote_mode, 1);
        assert!(p.show_code_panel);
        assert!((p.trace_speed - 1.5).abs() < 1e-6);
        assert!((p.spark_intensity - 1.4).abs() < 1e-6);
        assert!((p.dust_density - 0.8).abs() < 1e-6);
        assert!((p.parallax - 0.7).abs() < 1e-6);
        assert!(!p.adaptive);
        assert_eq!(p.seed, 42);
    }

    #[test]
    fn contour_extraction_is_deterministic_and_rich() {
        let img = image::load_from_memory(ARTWORK_BYTES).expect("artwork decode");
        let small = image::imageops::resize(&img.to_rgba8(), 360, 480, image::imageops::FilterType::Triangle);
        let a = extract_contours_from_rgba(small.as_raw(), 360, 480, [0.50, 0.34], 7);
        let b = extract_contours_from_rgba(small.as_raw(), 360, 480, [0.50, 0.34], 7);
        assert_eq!(a.segments.len(), b.segments.len());
        assert_eq!(a.verts.len(), b.verts.len());
        assert!(a.segments.len() >= 120, "Expected rich contours, got {}", a.segments.len());
        assert!(!a.twinkles.is_empty());
        // Verify segments span TRACE_START..TRACE_START+TRACE_SPREAD
        assert!(a.segments.iter().all(|s| s.t_start >= TRACE_START - 0.01));
        assert!(a.segments.iter().any(|s| s.t_start > TRACE_START + contour::TRACE_SPREAD * 0.4));
    }

    #[test]
    fn timeline_phases_click_replay_and_resize_threshold() {
        let mut st = PersephoneState::new(1920.0, 1080.0, 1.0);
        let (f0, _, _, z0, _) = st.phase_values();
        assert_eq!(f0, 0.0);
        assert!((z0 - ZOOM_START).abs() < 1e-4);

        st.timeline_t = 12.2;
        let (f1, _, amp1, z1, _) = st.phase_values();
        assert!((f1 - 1.0).abs() < 1e-4);
        assert!(amp1 > 0.5, "Shimmer should be active at 12.2s");
        assert!(z1 < 1.05);

        // Click replays animation when past FILL_START
        assert!(st.handle_mouse(1, 500.0, 400.0));
        assert_eq!(st.timeline_t, 0.0);

        // Resize threshold (> 0.5 px)
        let r0 = st.rev;
        st.resize(1920.2, 1080.1, 1.0);
        assert_eq!(st.rev, r0);
        st.resize(1280.0, 720.0, 1.0);
        assert_ne!(st.rev, r0);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn gpu_render_persephone_trace_and_completed() {
        use std::ffi::CString;
        let _guard = crate::LINUX_EGL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let eng = crate::ddgl_headless_create(1280, 720, 1.0);
            assert!(!eng.is_null());
            let theme_msg = CString::new(r#"{"t":"theme","id":"persephone"}"#).unwrap();
            crate::ddgl_headless_post_json(eng, theme_msg.as_ptr());

            let mut scratch = vec![0u8; 1280 * 720 * 4];
            // 1. Mid-trace spark frame (~4.0s = 50 * 0.08s)
            for _ in 0..49 {
                let n = crate::ddgl_headless_render_rgba(
                    eng,
                    0.08,
                    scratch.as_mut_ptr(),
                    scratch.len() as i32,
                );
                assert_eq!(n, scratch.len() as i32);
            }
            let out1 = std::env::temp_dir().join("ddgl_persephone_trace.png");
            let c1 = CString::new(out1.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.08, c1.as_ptr()), 1);
            assert!(std::fs::metadata(&out1).map(|m| m.len() > 20_000).unwrap_or(false));

            // 2. Completed frame with silver fill + god-rays + quote (~16.0s = +150 * 0.08s)
            for _ in 0..149 {
                let n = crate::ddgl_headless_render_rgba(
                    eng,
                    0.08,
                    scratch.as_mut_ptr(),
                    scratch.len() as i32,
                );
                assert_eq!(n, scratch.len() as i32);
            }
            let out2 = std::env::temp_dir().join("ddgl_persephone_final.png");
            let c2 = CString::new(out2.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.08, c2.as_ptr()), 1);
            assert!(std::fs::metadata(&out2).map(|m| m.len() > 20_000).unwrap_or(false));

            crate::ddgl_headless_destroy(eng);
        }
    }
}
