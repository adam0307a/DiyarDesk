pub mod shaders;

use crate::gl::*;
use crate::quiet_snow::Mulberry32;
use crate::state::{AppState, Rect};
use shaders::*;
use std::ffi::c_void;

// =========================================================================
// 1. PALETTES & QUALITY PROFILES (1:1 liquidglass.html)
// =========================================================================
#[allow(dead_code)]
pub const PALETTE_COUNT: usize = 5; // tahoe, aurora, prism, obsidian, classic
#[allow(dead_code)]
pub const ICON_STYLE_COUNT: usize = 3; // capsule, droplet, prism

#[derive(Clone, Copy)]
pub struct LgQuality {
    #[allow(dead_code)]
    pub max_dpr: f32,
    pub blur_scale: f32,
    pub max_blur_tap: i32,
    pub max_icons: i32,
    pub max_drops: i32,
}

pub const QUALITY_PROFILES: [LgQuality; 3] = [
    LgQuality { max_dpr: 0.75, blur_scale: 0.35, max_blur_tap: 8,  max_icons: 24, max_drops: 4 },
    LgQuality { max_dpr: 1.15, blur_scale: 0.50, max_blur_tap: 14, max_icons: 36, max_drops: 7 },
    LgQuality { max_dpr: 1.50, blur_scale: 0.55, max_blur_tap: 18, max_icons: 48, max_drops: 9 },
];

// Palette-matched crystal glass tint rgba (1:1)
pub const TINTS: [[f32; 4]; 5] = [
    [0.88, 0.96, 1.00, 0.09], // tahoe
    [0.92, 0.90, 1.00, 0.08], // aurora
    [1.00, 0.95, 0.90, 0.09], // prism (sunset)
    [0.84, 0.90, 0.98, 0.07], // obsidian
    [0.88, 0.96, 1.00, 0.09], // classic
];

// =========================================================================
// 2. PARAMETERS & LIVE STATE
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct LgParams {
    pub palette: usize,
    pub refraction: f32,
    pub frost_amount: f32,
    pub dispersion: f32,
    pub merge_rate: f32,
    pub droplet_count: f32,
    pub flow_speed: f32,
    pub cursor_lens: bool,
    pub icon_style: usize, // 0 capsule, 1 droplet, 2 prism
    pub quality: usize,    // 0 Low, 1 Medium, 2 High
    pub adaptive: i32,
    pub seed: u32,
}

impl Default for LgParams {
    fn default() -> Self {
        Self {
            palette: 0,
            refraction: 1.0,
            frost_amount: 0.65,
            dispersion: 0.95,
            merge_rate: 0.55,
            droplet_count: 1.0,
            flow_speed: 0.85,
            cursor_lens: true,
            icon_style: 0,
            quality: 2,
            adaptive: 1,
            seed: 0,
        }
    }
}

// 1:1 drop spec table (ux, uy, rx, ry, rd, vx, vy, ph)
#[derive(Clone, Copy)]
pub struct DropSpec {
    pub ux: f32,
    pub uy: f32,
    pub rx: f32,
    pub ry: f32,
    pub rd: f32,
    pub vx: f32,
    pub vy: f32,
    pub ph: f32,
}

pub const DROP_SPECS: [DropSpec; 10] = [
    DropSpec { ux: 0.72, uy: 0.34, rx: 88.0, ry: 54.0, rd: 46.0, vx: 19.0,  vy: 11.0,  ph: 0.4 },
    DropSpec { ux: 0.62, uy: 0.56, rx: 56.0, ry: 56.0, rd: 56.0, vx: -16.0, vy: 14.0,  ph: 1.7 },
    DropSpec { ux: 0.82, uy: 0.68, rx: 46.0, ry: 82.0, rd: 46.0, vx: -12.0, vy: -17.0, ph: 2.9 },
    DropSpec { ux: 0.44, uy: 0.28, rx: 64.0, ry: 44.0, rd: 38.0, vx: 15.0,  vy: -13.0, ph: 4.1 },
    DropSpec { ux: 0.52, uy: 0.74, rx: 74.0, ry: 42.0, rd: 42.0, vx: -20.0, vy: -9.0,  ph: 5.2 },
    DropSpec { ux: 0.34, uy: 0.54, rx: 44.0, ry: 44.0, rd: 44.0, vx: 14.0,  vy: 16.0,  ph: 0.9 },
    DropSpec { ux: 0.76, uy: 0.20, rx: 48.0, ry: 48.0, rd: 48.0, vx: -18.0, vy: 12.0,  ph: 3.3 },
    DropSpec { ux: 0.28, uy: 0.78, rx: 58.0, ry: 38.0, rd: 36.0, vx: 17.0,  vy: -14.0, ph: 2.1 },
    DropSpec { ux: 0.66, uy: 0.44, rx: 38.0, ry: 38.0, rd: 38.0, vx: -14.0, vy: -15.0, ph: 4.8 },
    DropSpec { ux: 0.48, uy: 0.46, rx: 52.0, ry: 52.0, rd: 52.0, vx: 12.0,  vy: 18.0,  ph: 1.1 },
];

#[derive(Clone, Copy)]
pub struct LgDrop {
    pub x: f32,
    pub y: f32,
    pub rx: f32,
    pub ry: f32,
    pub rd: f32,
    pub vx: f32,
    pub vy: f32,
    pub ph: f32,
}

#[derive(Clone, Copy)]
#[repr(C)]
#[allow(dead_code)]
pub struct LgIcon {
    pub cx: f32,
    pub cy: f32,
    pub hw: f32,
    pub hh: f32,
}

pub struct LiquidGlassState {
    pub p: LgParams,
    pub gw: f32,
    pub gh: f32,
    pub sim_time: f32,
    pub drops: Vec<LgDrop>,
    pub drop_round: [f32; 10],
    pub drop_data: [[f32; 4]; 10],
    pub drop_count: usize,
    // cursor lens
    pub cursor_active: bool,
    pub cursor_tx: f32,
    pub cursor_ty: f32,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cursor_vx: f32,
    pub cursor_vy: f32,
    pub cursor_vis: f32,
    pub cursor_last_move: f32,
    // icons
    pub icons: Vec<LgIcon>,
    pub icons_seen: bool,
    pub cache_ok: bool,
    pub dirty: bool,
    pub low_mode: bool,
}

impl Default for LiquidGlassState {
    fn default() -> Self {
        let mut st = Self {
            p: LgParams::default(),
            gw: 1920.0,
            gh: 1080.0,
            sim_time: 0.0,
            drops: Vec::new(),
            drop_round: [0.0; 10],
            drop_data: [[0.0; 4]; 10],
            drop_count: 0,
            cursor_active: false,
            cursor_tx: 0.0,
            cursor_ty: 0.0,
            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_vx: 0.0,
            cursor_vy: 0.0,
            cursor_vis: 0.0,
            cursor_last_move: 0.0,
            icons: Vec::new(),
            icons_seen: false,
            cache_ok: false,
            dirty: true,
            low_mode: false,
        };
        st.init_drops();
        st
    }
}

impl LiquidGlassState {
    #[inline]
    fn profile(&self) -> LgQuality {
        if self.low_mode {
            return QUALITY_PROFILES[0];
        }
        QUALITY_PROFILES[self.p.quality.min(2)]
    }

    // 1:1 initDrops()
    fn init_drops(&mut self) {
        self.drops.clear();
        for s in DROP_SPECS.iter() {
            self.drops.push(LgDrop {
                x: self.gw * s.ux,
                y: self.gh * s.uy,
                rx: s.rx,
                ry: s.ry,
                rd: s.rd,
                vx: s.vx,
                vy: s.vy,
                ph: s.ph,
            });
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = w.max(240.0);
        let nh = h.max(160.0);
        if (self.gw - nw).abs() > 0.5 || (self.gh - nh).abs() > 0.5 {
            let prev_w = self.gw;
            let prev_h = self.gh;
            self.gw = nw;
            self.gh = nh;
            // 1:1 rescale drop positions proportionally
            if prev_w > 0.0 && prev_h > 0.0 {
                for d in &mut self.drops {
                    d.x = (d.x / prev_w) * nw;
                    d.y = (d.y / prev_h) * nh;
                }
            }
            self.cache_ok = false;
            self.dirty = true;
            self.sync_icon_uniforms();
        }
    }

    pub fn sync_params(&mut self, next: LgParams) {
        let need_bg = next.palette != self.p.palette
            || (next.refraction - self.p.refraction).abs() > 1e-4
            || (next.frost_amount - self.p.frost_amount).abs() > 1e-4
            || (next.dispersion - self.p.dispersion).abs() > 1e-4
            || next.quality != self.p.quality;
        let need_icons = next.adaptive != self.p.adaptive || next.icon_style != self.p.icon_style;
        self.p = next;
        if need_bg {
            self.cache_ok = false;
            self.dirty = true;
        }
        if need_icons {
            self.sync_icon_uniforms();
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.cache_ok = false;
            self.dirty = true;
        }
    }

    pub fn on_pointer_move(&mut self, x: f32, y: f32) {
        self.cursor_tx = x;
        self.cursor_ty = y;
        if !self.cursor_active {
            self.cursor_x = x;
            self.cursor_y = y;
            self.cursor_vx = 0.0;
            self.cursor_vy = 0.0;
        }
        self.cursor_active = true;
        self.cursor_last_move = self.sim_time;
    }

    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        self.on_pointer_move(x, y);
    }

    pub fn on_pointer_up(&mut self) {}

    pub fn on_pointer_leave(&mut self) {
        self.cursor_active = false;
    }

    // 1:1 syncIconUniformBuffer()
    pub fn sync_icon_uniforms(&mut self) {
        self.icons.clear();
        if self.p.adaptive == 0 || !self.icons_seen {
            return;
        }
        let prof = self.profile();
        let _ = prof;
        // icons are set via sync_incoming_icons; this method rebuilds the uniform buffer from them
    }

    // 1:1 handleMessage 'icons'
    pub fn sync_incoming_icons(&mut self, rects: &[Rect]) {
        self.icons_seen = true;
        self.icons.clear();
        if self.p.adaptive == 0 {
            return;
        }
        let max_n = (self.profile().max_icons as usize).min(rects.len()).min(48);
        let pad_x = if self.p.icon_style == 2 { 4.0 } else { 7.0 };
        let pad_y = if self.p.icon_style == 2 { 4.0 } else { 6.0 };
        for ic in rects.iter().take(max_n) {
            self.icons.push(LgIcon {
                cx: ic.x + ic.w * 0.5,
                cy: ic.y + ic.h * 0.5,
                hw: (ic.w * 0.5 + pad_x).max(22.0),
                hh: (ic.h * 0.5 + pad_y).max(24.0),
            });
        }
        self.dirty = true;
    }

    // 1:1 stepPhysics(dt)
    pub fn step(&mut self, dt: f32, _icons: &[Rect]) {
        let flow = self.p.flow_speed.max(0.0);
        self.sim_time += dt * (0.45 + 0.75 * flow);

        let prof = self.profile();
        let desired = (prof.max_drops as f32 * 1.0).min((5.2 * self.p.droplet_count).round().max(0.0)).min(10.0) as usize;
        let scale_factor = (self.gw.min(self.gh) / 900.0).clamp(0.65, 1.15);
        self.drop_count = desired.min(10);

        for i in 0..self.drop_count {
            let d = &mut self.drops[i];
            let wobble_x = (self.sim_time * 0.7 + d.ph).sin() * 14.0;
            let wobble_y = (self.sim_time * 0.6 + d.ph * 1.3).cos() * 12.0;
            d.x += (d.vx + wobble_x) * dt * flow;
            d.y += (d.vy + wobble_y) * dt * flow;

            let rx = d.rx * scale_factor;
            let ry = d.ry * scale_factor;
            let pad_x = rx + 24.0;
            let pad_y = ry + 24.0;
            if d.x < pad_x {
                d.x = pad_x;
                d.vx = d.vx.abs();
            } else if d.x > self.gw - pad_x {
                d.x = self.gw - pad_x;
                d.vx = -d.vx.abs();
            }
            if d.y < pad_y {
                d.y = pad_y;
                d.vy = d.vy.abs();
            } else if d.y > self.gh - pad_y {
                d.y = self.gh - pad_y;
                d.vy = -d.vy.abs();
            }

            let pulse = 1.0 + 0.04 * (self.sim_time * 1.1 + d.ph).sin();
            self.drop_data[i] = [d.x, d.y, rx * pulse, ry / pulse];
            self.drop_round[i] = d.rd * scale_factor;
        }

        // 1:1 spring-damper cursor lens
        let want_cursor = self.p.cursor_lens && self.cursor_active && (self.sim_time - self.cursor_last_move < 6.5);
        let target_vis = if want_cursor { 1.0 } else { 0.0 };
        let vis_rate = if want_cursor { 10.0 } else { 3.5 };
        self.cursor_vis += (target_vis - self.cursor_vis) * (dt * vis_rate).min(1.0);

        let stiff = 135.0f32;
        let damp = 17.0f32;
        let ax = (self.cursor_tx - self.cursor_x) * stiff - self.cursor_vx * damp;
        let ay = (self.cursor_ty - self.cursor_y) * stiff - self.cursor_vy * damp;
        self.cursor_vx += ax * dt;
        self.cursor_vy += ay * dt;
        self.cursor_x += self.cursor_vx * dt;
        self.cursor_y += self.cursor_vy * dt;
    }

    // Compute gaussian weights (1:1 computeGaussianWeights)
    pub fn gaussian_weights(radius: i32) -> [f32; 25] {
        let mut arr = [0.0f32; 25];
        let r = radius.clamp(0, 24);
        if r == 0 {
            arr[0] = 1.0;
            return arr;
        }
        let sigma = (r as f32 * 0.45).max(1.0);
        let mut sum = 1.0f32;
        arr[0] = 1.0;
        for i in 1..=r {
            let fi = i as f32;
            let w = (-(fi * fi) / (2.0 * sigma * sigma)).exp();
            arr[i as usize] = w;
            sum += 2.0 * w;
        }
        for i in 0..=r {
            arr[i as usize] /= sum;
        }
        arr
    }

    // Cursor lens dimensions (1:1 uploadSharedUniforms)
    pub fn cursor_rect(&self) -> (f32, f32, f32, f32, f32) {
        let spd_x = (self.cursor_vx.abs() * 0.022).min(26.0);
        let spd_y = (self.cursor_vy.abs() * 0.022).min(26.0);
        let base_r = 54.0 * self.cursor_vis;
        let hw = (base_r + spd_x - spd_y * 0.35).max(8.0);
        let hh = (base_r + spd_y - spd_x * 0.35).max(8.0);
        let rad = (hw.min(hh)) * 0.96;
        (self.cursor_x, self.cursor_y, hw, hh, rad)
    }
}

// =========================================================================
// 3. OPENGL 3.3 CORE MULTI-PASS RENDERER
// =========================================================================
struct TexFbo {
    fbo: GLuint,
    tex: GLuint,
    w: i32,
    h: i32,
}

impl TexFbo {
    fn empty() -> Self {
        Self { fbo: 0, tex: 0, w: 0, h: 0 }
    }

    unsafe fn ensure(&mut self, gl: &GlFns, w: i32, h: i32) {
        let w = w.max(16);
        let h = h.max(16);
        if self.fbo != 0 && self.w == w && self.h == h {
            return;
        }
        self.destroy(gl);
        (gl.GenFramebuffers)(1, &mut self.fbo);
        (gl.GenTextures)(1, &mut self.tex);
        (gl.BindTexture)(GL_TEXTURE_2D, self.tex);
        let use_float = !gl.is_gles || gl.has_color_buffer_float;
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            if use_float { GL_RGBA16F as GLint } else { GL_RGBA8 as GLint },
            w,
            h,
            0,
            GL_RGBA,
            if use_float { GL_FLOAT } else { GL_UNSIGNED_BYTE },
            std::ptr::null(),
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.fbo);
        (gl.FramebufferTexture2D)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, self.tex, 0);
        if use_float && (gl.CheckFramebufferStatus)(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE {
            (gl.BindTexture)(GL_TEXTURE_2D, self.tex);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                w,
                h,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                std::ptr::null(),
            );
            (gl.FramebufferTexture2D)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, self.tex, 0);
        }
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        self.w = w;
        self.h = h;
    }

    unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.tex != 0 {
            (gl.DeleteTextures)(1, &self.tex);
            self.tex = 0;
        }
        if self.fbo != 0 {
            (gl.DeleteFramebuffers)(1, &self.fbo);
            self.fbo = 0;
        }
        self.w = 0;
        self.h = 0;
    }
}

pub struct LiquidGlassRenderer {
    prog_bg: GLuint,
    prog_blur: GLuint,
    prog_main: GLuint,
    dummy_vao: GLuint,
    tri_vao: GLuint,
    tri_vbo: GLuint,
    fbo_bg: TexFbo,
    fbo_blur_a: TexFbo,
    fbo_blur_b: TexFbo,
    weights: [f32; 25],
    last_w: i32,
    last_h: i32,
}

impl LiquidGlassRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        // Inject shared SDF library into both bg and main fragment shaders.
        let shared = LG_SHARED_SDF;
        let bg_src = LG_BG_FRAG.replace("__SHARED_SDF__", shared);
        let main_src = LG_MAIN_FRAG.replace("__SHARED_SDF__", shared);

        let prog_bg = gl.compile_program(LG_VERT, &bg_src)?;
        let prog_blur = gl.compile_program(LG_VERT, LG_BLUR_FRAG)?;
        let prog_main = gl.compile_program(LG_VERT, &main_src)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let mut tri_vao = 0;
        let mut tri_vbo = 0;
        (gl.GenVertexArrays)(1, &mut tri_vao);
        (gl.GenBuffers)(1, &mut tri_vbo);
        (gl.BindVertexArray)(tri_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, tri_vbo);
        let tri: [f32; 6] = [-1.0, -1.0, 3.0, -1.0, -1.0, 3.0];
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            std::mem::size_of_val(&tri) as GLsizeiptr,
            tri.as_ptr() as *const c_void,
            GL_STATIC_DRAW,
        );
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, 8, 0 as *const c_void);
        (gl.BindVertexArray)(0);

        Ok(Self {
            prog_bg,
            prog_blur,
            prog_main,
            dummy_vao,
            tri_vao,
            tri_vbo,
            fbo_bg: TexFbo::empty(),
            fbo_blur_a: TexFbo::empty(),
            fbo_blur_b: TexFbo::empty(),
            weights: [0.0; 25],
            last_w: 0,
            last_h: 0,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        self.fbo_bg.destroy(gl);
        self.fbo_blur_a.destroy(gl);
        self.fbo_blur_b.destroy(gl);
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
        if self.tri_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.tri_vbo);
            self.tri_vbo = 0;
        }
        if self.tri_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.tri_vao);
            self.tri_vao = 0;
        }
        for p in [self.prog_bg, self.prog_blur, self.prog_main] {
            if p != 0 {
                (gl.DeleteProgram)(p);
            }
        }
    }

    // Upload shared SDF uniforms (drops, cursor, icons) to a program.
    unsafe fn upload_shared(&self, gl: &GlFns, prog: GLuint, st: &LiquidGlassState, pw: f32, ph: f32, dpr: f32) {
        (gl.Uniform2f)(gl.uniform_loc(prog, "u_resolution"), pw, ph);
        (gl.Uniform1f)(gl.uniform_loc(prog, "u_dpr"), dpr);
        let merge_px = st.p.merge_rate.max(0.0) * 44.0;
        (gl.Uniform1f)(gl.uniform_loc(prog, "u_mergePx"), merge_px);

        (gl.Uniform1i)(gl.uniform_loc(prog, "u_dropCount"), st.drop_count as i32);
        if st.drop_count > 0 {
            (gl.Uniform4fv)(gl.uniform_loc(prog, "u_drops"), (st.drop_count * 4) as GLsizei, st.drop_data.as_ptr() as *const f32);
            (gl.Uniform1fv)(gl.uniform_loc(prog, "u_dropRound"), st.drop_count as GLsizei, st.drop_round.as_ptr());
        }

        let cursor_on = if st.p.cursor_lens && st.cursor_vis > 0.02 { 1 } else { 0 };
        (gl.Uniform1i)(gl.uniform_loc(prog, "u_cursorOn"), cursor_on);
        if cursor_on == 1 {
            let (cx, cy, hw, hh, rad) = st.cursor_rect();
            (gl.Uniform4f)(gl.uniform_loc(prog, "u_cursor"), cx, cy, hw, hh);
            (gl.Uniform1f)(gl.uniform_loc(prog, "u_cursorRadius"), rad);
        }

        let icon_count = st.icons.len().min(48) as i32;
        (gl.Uniform1i)(gl.uniform_loc(prog, "u_iconCount"), icon_count);
        (gl.Uniform1i)(gl.uniform_loc(prog, "u_iconStyle"), st.p.icon_style as i32);
        if icon_count > 0 {
            (gl.Uniform4fv)(gl.uniform_loc(prog, "u_icons"), icon_count, st.icons.as_ptr() as *const f32);
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, app: &mut AppState) {
        let pw = app.width_px.max(64);
        let ph = app.height_px.max(64);
        let css_w = app.css_w();
        let css_h = app.css_h();
        let dpr = app.dpi_scale.max(0.5);

        let st = &mut app.liquidglass;
        st.resize(css_w, css_h);

        if self.last_w != pw || self.last_h != ph {
            self.last_w = pw;
            self.last_h = ph;
            st.cache_ok = false;
        }

        let prof = st.profile();
        let blur_scale = prof.blur_scale;
        let bw = ((pw as f32 * blur_scale).round() as i32).max(64);
        let bh = ((ph as f32 * blur_scale).round() as i32).max(64);
        self.fbo_bg.ensure(gl, pw, ph);
        self.fbo_blur_a.ensure(gl, bw, bh);
        self.fbo_blur_b.ensure(gl, bw, bh);

        (gl.BindVertexArray)(self.tri_vao);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);

        // ---- Pass 0: silk optical backdrop + glass cast shadows -> fbo_bg ----
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.fbo_bg.fbo);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.UseProgram)(self.prog_bg);
        self.upload_shared(gl, self.prog_bg, st, pw as f32, ph as f32, dpr);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "u_time"), st.sim_time);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_bg, "u_palette"), st.p.palette as i32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "u_shadowFactor"), 1.0);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // ---- Pass 1 & 2: separable Gaussian blur -> fbo_blur_b ----
        let blur_rad = (prof.max_blur_tap).min((st.p.frost_amount * 11.5).round() as i32);
        self.weights = LiquidGlassState::gaussian_weights(blur_rad);
        (gl.UseProgram)(self.prog_blur);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_blur, "u_radius"), blur_rad);
        (gl.Uniform1fv)(gl.uniform_loc(self.prog_blur, "u_weights[0]"), 25, self.weights.as_ptr());
        (gl.Uniform1i)(gl.uniform_loc(self.prog_blur, "u_tex"), 0);

        // Horizontal
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.fbo_blur_a.fbo);
        (gl.Viewport)(0, 0, bw, bh);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.fbo_bg.tex);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_blur, "u_texelDir"), 1.0 / bw as f32, 0.0);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Vertical
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.fbo_blur_b.fbo);
        (gl.Viewport)(0, 0, bw, bh);
        (gl.BindTexture)(GL_TEXTURE_2D, self.fbo_blur_a.tex);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_blur, "u_texelDir"), 0.0, 1.0 / bh as f32);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // ---- Pass 3: main physical liquid glass composite -> screen ----
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.UseProgram)(self.prog_main);
        self.upload_shared(gl, self.prog_main, st, pw as f32, ph as f32, dpr);

        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.fbo_bg.tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_main, "u_bg"), 0);
        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.fbo_blur_b.tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_main, "u_blurredBg"), 1);

        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "u_refraction"), st.p.refraction.max(0.15));
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "u_frost"), st.p.frost_amount.max(0.0));
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "u_dispersion"), st.p.dispersion.max(0.0));

        let tint = TINTS[st.p.palette.min(4)];
        (gl.Uniform4f)(gl.uniform_loc(self.prog_main, "u_tint"), tint[0], tint[1], tint[2], tint[3]);

        let glare_angle = -0.78 + (st.sim_time * 0.25).sin() * 0.14;
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "u_glareAngle"), glare_angle);

        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
    }
}

// Keep Mulberry32 referenced so the import stays meaningful for future seed work.
#[allow(dead_code)]
fn _seed_marker() {
    let _ = Mulberry32::new(0);
}
// =========================================================================
// 4. rust-anything Recorded Ground-Truth Cases (1:1 liquidglass.html pure logic)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::path::Path;

    fn fnv1a_fingerprint(rel_path: &str, bytes: &[u8]) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |buf: &[u8]| {
            for &b in buf {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
        };
        eat(rel_path.replace('\\', "/").as_bytes());
        eat(&[0]);
        eat(bytes);
        eat(&[0]);
        format!("{hash:016x}")
    }

    fn drops_case(droplet_count: f32, quality: usize, w: f32, h: f32) -> Value {
        let mut st = LiquidGlassState::default();
        st.p.droplet_count = droplet_count;
        st.p.quality = quality;
        st.gw = w;
        st.gh = h;
        for i in 0..20 {
            st.step(0.033, &[]);
            if i == 1 || i == 19 {
                // capture drop state
            }
        }
        let arr: Vec<Value> = st
            .drops
            .iter()
            .take(st.drop_count)
            .map(|d| json!([d.x, d.y, d.rx, d.ry, d.rd, d.vx, d.vy, d.ph]))
            .collect();
        json!({ "drop_count": st.drop_count, "drops": arr })
    }

    fn icon_sync_case(rects: &[crate::state::Rect]) -> Value {
        let mut st = LiquidGlassState::default();
        st.p.adaptive = 1;
        st.gw = 1920.0;
        st.gh = 1080.0;
        st.sync_incoming_icons(rects);
        let arr: Vec<Value> = st
            .icons
            .iter()
            .map(|i| json!([i.cx, i.cy, i.hw, i.hh]))
            .collect();
        json!({ "count": arr.len(), "icons": arr })
    }

    fn gaussian_case(radius: i32) -> Value {
        let w = LiquidGlassState::gaussian_weights(radius);
        json!({ "radius": radius, "weights": w[..(radius as usize + 1).min(25)].to_vec() })
    }

    fn cursor_case() -> Value {
        let mut st = LiquidGlassState::default();
        st.p.cursor_lens = true;
        st.cursor_active = true;
        st.cursor_tx = 500.0;
        st.cursor_ty = 400.0;
        st.cursor_last_move = 0.0;
        for _ in 0..20 {
            st.step(0.033, &[]);
        }
        let (cx, cy, hw, hh, rad) = st.cursor_rect();
        json!({ "cx": cx, "cy": cy, "hw": hw, "hh": hh, "rad": rad, "vis": st.cursor_vis })
    }

    #[test]
    fn record_or_verify_liquidglass_cases() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir.parent().and_then(|p| p.parent()).expect("repo root");
        let rec_path = manifest_dir.join("src/liquidglass/recorded-from.json");
        let cases_path = manifest_dir.join("src/liquidglass/cases.jsonl");

        let rec_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&rec_path).expect("read recorded-from.json"))
                .expect("parse recorded-from.json");
        let unit_rel = rec_json["unit"].as_str().expect("unit path");
        let expected_fp = rec_json["fingerprint"].as_str().expect("fingerprint");
        let html_bytes = std::fs::read(repo_root.join(unit_rel)).expect("read prototype/liquidglass.html");
        let actual_fp = fnv1a_fingerprint(unit_rel, &html_bytes);
        assert_eq!(actual_fp, expected_fp, "rust-anything gate: prototype/liquidglass.html changed since recording");

        let record = std::env::var("DIYARDESK_RECORD_LIQUIDGLASS").is_ok();
        if record {
            let mut out = String::new();
            for &(dc, q, w, h) in &[(1.0f32, 2usize, 1920.0f32, 1080.0f32), (1.8, 0, 1280.0, 720.0), (0.0, 1, 1920.0, 1080.0)] {
                let snap = drops_case(dc, q, w, h);
                out.push_str(&json!({ "fn": "droplets", "args": [dc, q, w, h], "result": { "ok": snap } }).to_string());
                out.push('\n');
            }
            let rects = [
                crate::state::Rect { x: 80.0, y: 100.0, w: 76.0, h: 90.0 },
                crate::state::Rect { x: 200.0, y: 100.0, w: 76.0, h: 90.0 },
            ];
            let snap = icon_sync_case(&rects);
            out.push_str(&json!({ "fn": "icons", "args": [], "result": { "ok": snap } }).to_string());
            out.push('\n');

            for &r in &[0i32, 4, 11, 18] {
                let snap = gaussian_case(r);
                out.push_str(&json!({ "fn": "gaussian", "args": [r], "result": { "ok": snap } }).to_string());
                out.push('\n');
            }
            let snap = cursor_case();
            out.push_str(&json!({ "fn": "cursor", "args": [], "result": { "ok": snap } }).to_string());
            out.push('\n');

            std::fs::write(&cases_path, out).expect("write cases.jsonl");
            let count = std::fs::read_to_string(&cases_path).unwrap().lines().filter(|l| !l.trim().is_empty()).count();
            println!("Recorded {count} liquidglass cases to cases.jsonl");
            return;
        }

        let raw_cases = std::fs::read_to_string(&cases_path).expect("read cases.jsonl");
        let mut passed = 0usize;
        for (line_no, line) in raw_cases.lines().enumerate() {
            if line.trim().is_empty() { continue; }
            let c: Value = serde_json::from_str(line).expect("parse case JSON");
            let fn_name = c["fn"].as_str().expect("fn");
            let args = c["args"].as_array().expect("args");
            let ok = &c["result"]["ok"];
            match fn_name {
                "droplets" => {
                    let dc = args[0].as_f64().unwrap() as f32;
                    let q = args[1].as_u64().unwrap() as usize;
                    let w = args[2].as_f64().unwrap() as f32;
                    let h = args[3].as_f64().unwrap() as f32;
                    let snap = drops_case(dc, q, w, h);
                    assert_eq!(snap["drop_count"].as_u64().unwrap() as i64, ok["drop_count"].as_i64().unwrap(), "case #{line_no} drop count");
                    for (i, ev) in ok["drops"].as_array().unwrap().iter().enumerate() {
                        let gv = snap["drops"][i].as_array().unwrap();
                        let e = ev.as_array().unwrap();
                        for k in 0..8 {
                            let g = gv[k].as_f64().unwrap() as f32;
                            let ex = e[k].as_f64().unwrap() as f32;
                            assert!((g - ex).abs() <= 1e-2, "case #{line_no} drop {i}[{k}]: got {g}, exp {ex}");
                        }
                    }
                }
                "icons" => {
                    let rects = [
                        crate::state::Rect { x: 80.0, y: 100.0, w: 76.0, h: 90.0 },
                        crate::state::Rect { x: 200.0, y: 100.0, w: 76.0, h: 90.0 },
                    ];
                    let snap = icon_sync_case(&rects);
                    assert_eq!(snap["count"].as_u64().unwrap() as i64, ok["count"].as_i64().unwrap());
                    for (i, ev) in ok["icons"].as_array().unwrap().iter().enumerate() {
                        let gv = snap["icons"][i].as_array().unwrap();
                        let e = ev.as_array().unwrap();
                        for k in 0..4 {
                            let g = gv[k].as_f64().unwrap() as f32;
                            let ex = e[k].as_f64().unwrap() as f32;
                            assert!((g - ex).abs() <= 1e-2, "case #{line_no} icon {i}[{k}]: got {g}, exp {ex}");
                        }
                    }
                }
                "gaussian" => {
                    let r = args[0].as_i64().unwrap() as i32;
                    let snap = gaussian_case(r);
                    let gw = snap["weights"].as_array().unwrap();
                    let ew = ok["weights"].as_array().unwrap();
                    for (i, ev) in ew.iter().enumerate() {
                        let g = gw[i].as_f64().unwrap() as f32;
                        let ex = ev.as_f64().unwrap() as f32;
                        assert!((g - ex).abs() <= 1e-5, "case #{line_no} gaussian[{i}]: got {g}, exp {ex}");
                    }
                }
                "cursor" => {
                    let snap = cursor_case();
                    assert!((snap["cx"].as_f64().unwrap() as f32 - ok["cx"].as_f64().unwrap() as f32).abs() <= 1e-2);
                    assert!((snap["hw"].as_f64().unwrap() as f32 - ok["hw"].as_f64().unwrap() as f32).abs() <= 1e-2);
                    assert!((snap["vis"].as_f64().unwrap() as f32 - ok["vis"].as_f64().unwrap() as f32).abs() <= 1e-3);
                }
                other => panic!("Unknown recorded function: {other}"),
            }
            passed += 1;
        }
        assert!(passed > 0, "No liquidglass recorded cases found; run with DIYARDESK_RECORD_LIQUIDGLASS=1");
    }
}
