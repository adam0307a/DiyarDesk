//! TURKISH FLAG (`turkish-flag`) — Native Rust + OpenGL 3.3 wallpaper.
//!
//! Renders an authentic Law No. 2893 Turkish flag flying proudly from a
//! brushed-metal flagpole with a golden finial (alem) against an interactive
//! sky (Daylight, Golden Sunset, Starry Night, or Local Time Auto) with
//! drifting multi-layer clouds and live Verlet cloth wind physics.

pub mod cloth;
pub mod shaders;

use crate::gl::*;
use crate::shaders::FULLSCREEN_VERT;
use crate::state::{AppState, Rect};
use cloth::{FlagCloth, PointerInteraction, BOT_Y, POLE_X, TOP_Y, TRI_COUNT, VERTEX_FLOATS};
use serde_json::Value;
use shaders::{FLAG_FRAG, FLAG_VERT, SKY_AND_POLE_FRAG};
use std::collections::HashMap;
use std::ffi::c_void;

// ============================================================================
// 1. PARAMETERS
// ============================================================================

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TurkishFlagParams {
    /// 0 = daylight, 1 = sunset, 2 = night, 3 = auto (local clock hour)
    pub sky_mode: usize,
    /// Cloud coverage multiplier (0.0 .. 1.5)
    pub clouds: f32,
    /// Atmospheric glow / sun / star brilliance (0.0 .. 1.5)
    pub atmosphere: f32,
    /// Wind strength (0.2 .. 2.0)
    pub wind: f32,
    /// Simulation speed multiplier (0.3 .. 2.0)
    pub speed: f32,
    /// 0 = silk, 1 = cotton, 2 = ceremonial
    pub fabric: usize,
    /// Pointer wind swirl & click gust
    pub mouse_interact: bool,
    /// Subtle readability scrims behind desktop icons
    pub adaptive: bool,
}

impl Default for TurkishFlagParams {
    fn default() -> Self {
        Self {
            sky_mode: 0,
            clouds: 0.85,
            atmosphere: 1.0,
            wind: 1.0,
            speed: 1.0,
            fabric: 0,
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

impl TurkishFlagParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        Self {
            sky_mode: parse_enum(raw, "skyMode", &["daylight", "sunset", "night", "auto"], 0),
            clouds: parse_f32(raw, "clouds", 0.85, 0.0, 1.5),
            atmosphere: parse_f32(raw, "atmosphere", 1.0, 0.0, 1.5),
            wind: parse_f32(raw, "wind", 1.0, 0.2, 2.0),
            speed: parse_f32(raw, "speed", 1.0, 0.3, 2.0),
            fabric: parse_enum(raw, "fabric", &["silk", "cotton", "ceremonial"], 0),
            mouse_interact: parse_bool(raw, "mouseInteract", true),
            adaptive: parse_bool(raw, "adaptive", true),
        }
    }
}

// ============================================================================
// 2. CAMERA & PROJECTION MATH
// ============================================================================

const CAM_POS: [f32; 3] = [0.04, -0.14, 3.45];
const CAM_TARGET: [f32; 3] = [0.06, 0.02, 0.0];

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm3(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Column-major `projection * view` matrix shared identically between the 3D
/// cloth vertex shader and the 2D flagpole projection in `SKY_AND_POLE_FRAG`.
pub fn build_proj_view(aspect: f32) -> [f32; 16] {
    let fov_y = 42.0f32.to_radians();
    let f = 1.0 / (fov_y * 0.5).tan();
    let near = 0.2f32;
    let far = 20.0f32;
    let a = aspect.max(0.25);

    let proj = [
        f / a, 0.0, 0.0, 0.0,
        0.0, f, 0.0, 0.0,
        0.0, 0.0, (far + near) / (near - far), -1.0,
        0.0, 0.0, (2.0 * far * near) / (near - far), 0.0,
    ];

    let fwd = norm3(sub3(CAM_TARGET, CAM_POS));
    let right = norm3(cross3(fwd, [0.0, 1.0, 0.0]));
    let up = cross3(right, fwd);

    let view = [
        right[0], up[0], -fwd[0], 0.0,
        right[1], up[1], -fwd[1], 0.0,
        right[2], up[2], -fwd[2], 0.0,
        -dot3(right, CAM_POS), -dot3(up, CAM_POS), dot3(fwd, CAM_POS), 1.0,
    ];

    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += proj[k * 4 + row] * view[col * 4 + k];
            }
            out[col * 4 + row] = sum;
        }
    }
    out
}

pub fn project_ndc(m: &[f32; 16], p: [f32; 3]) -> [f32; 2] {
    let x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
    let y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
    let w = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
    let inv_w = if w.abs() > 1e-5 { 1.0 / w } else { 1.0 };
    [x * inv_w, y * inv_w]
}

pub fn screen_to_world_z0(css_x: f32, css_y: f32, w_css: f32, h_css: f32) -> [f32; 2] {
    let aspect = (w_css / h_css.max(1.0)).max(0.25);
    let ndc_x = (css_x / w_css.max(1.0)) * 2.0 - 1.0;
    let ndc_y = 1.0 - (css_y / h_css.max(1.0)) * 2.0;
    let tan_half = (42.0f32.to_radians() * 0.5).tan();

    let fwd = norm3(sub3(CAM_TARGET, CAM_POS));
    let right = norm3(cross3(fwd, [0.0, 1.0, 0.0]));
    let up = cross3(right, fwd);

    let ray_dir = norm3([
        fwd[0] + right[0] * ndc_x * tan_half * aspect + up[0] * ndc_y * tan_half,
        fwd[1] + right[1] * ndc_x * tan_half * aspect + up[1] * ndc_y * tan_half,
        fwd[2] + right[2] * ndc_x * tan_half * aspect + up[2] * ndc_y * tan_half,
    ]);

    let denom = if ray_dir[2].abs() > 1e-4 {
        ray_dir[2]
    } else {
        -1.0
    };
    let t = -CAM_POS[2] / denom;
    [CAM_POS[0] + ray_dir[0] * t, CAM_POS[1] + ray_dir[1] * t]
}

// ============================================================================
// 3. STATE
// ============================================================================

pub struct TurkishFlagState {
    pub params: TurkishFlagParams,
    pub w_css: f32,
    pub h_css: f32,
    pub dpr: f32,
    pub low_mode: bool,
    pub env_hour: f32,
    pub rev: u64,

    pub anim_time: f32,
    pub cloth: FlagCloth,
    gust_impulse: [f32; 3],

    pub pointer_x: f32,
    pub pointer_y: f32,
    pub pointer_active: bool,
    pointer_world: [f32; 2],
    prev_pointer_world: [f32; 2],
    pointer_vel: [f32; 2],
    pointer_has_prev: bool,
    pointer_down: bool,
    grab_node: Option<usize>,
    ripple_pos: [f32; 2],
    ripple_age: f32,

    pub icons: Vec<Rect>,
    pub icons_visible: bool,
}

impl Default for TurkishFlagState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0, 1.0)
    }
}

impl TurkishFlagState {
    pub fn new(css_w: f32, css_h: f32, dpi: f32) -> Self {
        Self {
            params: TurkishFlagParams::default(),
            w_css: css_w.max(64.0),
            h_css: css_h.max(64.0),
            dpr: dpi.max(0.5),
            low_mode: false,
            env_hour: 14.0,
            rev: 1,
            anim_time: 0.0,
            cloth: FlagCloth::new(),
            gust_impulse: [0.0; 3],
            pointer_x: css_w * 0.5,
            pointer_y: css_h * 0.5,
            pointer_active: false,
            pointer_world: [0.0, 0.0],
            prev_pointer_world: [0.0, 0.0],
            pointer_vel: [0.0, 0.0],
            pointer_has_prev: false,
            pointer_down: false,
            grab_node: None,
            ripple_pos: [0.0, 0.0],
            ripple_age: -1.0,
            icons: Vec::new(),
            icons_visible: false,
        }
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

    pub fn set_env_hour(&mut self, hour: f32) {
        if hour.is_finite() {
            self.env_hour = hour.rem_euclid(24.0);
        }
    }

    pub fn sync_params(&mut self, p: TurkishFlagParams) {
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
            self.pointer_down = false;
            self.grab_node = None;
            self.pointer_has_prev = false;
            return false;
        }

        self.pointer_x = x;
        self.pointer_y = y;
        self.pointer_active = true;
        let world = screen_to_world_z0(x, y, self.w_css, self.h_css);
        self.pointer_world = world;

        if !self.params.mouse_interact {
            self.pointer_down = false;
            self.grab_node = None;
            self.pointer_has_prev = false;
            return false;
        }

        match kind {
            0 => {
                if self.pointer_has_prev {
                    let dx = (world[0] - self.prev_pointer_world[0]).clamp(-0.35, 0.35);
                    let dy = (world[1] - self.prev_pointer_world[1]).clamp(-0.35, 0.35);
                    let inst_vx = (dx * 26.0).clamp(-6.0, 6.0);
                    let inst_vy = (dy * 26.0).clamp(-6.0, 6.0);
                    self.pointer_vel[0] = self.pointer_vel[0] * 0.45 + inst_vx * 0.55;
                    self.pointer_vel[1] = self.pointer_vel[1] * 0.45 + inst_vy * 0.55;
                }
                self.prev_pointer_world = world;
                self.pointer_has_prev = true;
                self.pointer_down
            }
            1 => {
                self.pointer_down = true;
                self.prev_pointer_world = world;
                self.pointer_has_prev = true;
                self.ripple_pos = world;
                self.ripple_age = 0.0;
                self.gust_impulse = [6.5, 2.4, 7.2];
                self.grab_node = self.cloth.find_nearest_free_node(world[0], world[1], 0.42);
                true
            }
            2 => {
                self.pointer_down = false;
                let was_grabbed = self.grab_node.take().is_some();
                if was_grabbed {
                    self.gust_impulse[2] += 4.5;
                }
                was_grabbed
            }
            _ => false,
        }
    }

    pub fn step(&mut self, dt_raw: f32) {
        let dt = dt_raw.clamp(0.001, 0.05);
        self.anim_time += dt * self.params.speed;

        let decay = (-dt * 3.6).exp();
        self.gust_impulse[0] *= decay;
        self.gust_impulse[1] *= decay;
        self.gust_impulse[2] *= decay;

        let vel_decay = (-dt * 9.0).exp();
        self.pointer_vel[0] *= vel_decay;
        self.pointer_vel[1] *= vel_decay;

        if self.ripple_age >= 0.0 {
            self.ripple_age += dt;
            if self.ripple_age >= 0.95 {
                self.ripple_age = -1.0;
            }
        }

        let (gust, pointer) = if self.params.mouse_interact {
            (
                self.gust_impulse,
                PointerInteraction {
                    active: self.pointer_active,
                    pos: self.pointer_world,
                    vel: self.pointer_vel,
                    dragging: self.pointer_down && self.grab_node.is_some(),
                    grab_node: self.grab_node,
                    ripple_pos: self.ripple_pos,
                    ripple_age: self.ripple_age,
                },
            )
        } else {
            ([0.0; 3], PointerInteraction::default())
        };

        self.cloth.step(
            dt,
            self.anim_time,
            self.params.wind,
            self.params.speed,
            gust,
            &pointer,
            self.low_mode,
        );
    }
}

// ============================================================================
// 4. OPENGL 3.3 CORE RENDERER
// ============================================================================

pub struct TurkishFlagRenderer {
    prog_sky: GLuint,
    prog_flag: GLuint,
    dummy_vao: GLuint,
    flag_vao: GLuint,
    flag_vbo: GLuint,
    mesh_scratch: Vec<f32>,

    // Sky & pole uniforms
    s_res: GLint,
    s_time: GLint,
    s_sky_mode: GLint,
    s_env_hour: GLint,
    s_clouds: GLint,
    s_atmosphere: GLint,
    s_wind: GLint,
    s_adaptive: GLint,
    s_icon_count: GLint,
    s_icons: GLint,
    s_pole_top_bot: GLint,
    s_flag_hoist_y: GLint,
    s_pole_rad_px: GLint,

    // Flag uniforms
    f_proj_view: GLint,
    f_time: GLint,
    f_sky_mode: GLint,
    f_env_hour: GLint,
    f_atmosphere: GLint,
    f_fabric: GLint,
    f_cam_pos: GLint,
}

impl TurkishFlagRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_sky = gl.compile_program(FULLSCREEN_VERT, SKY_AND_POLE_FRAG)?;
        let prog_flag = match gl.compile_program(FLAG_VERT, FLAG_FRAG) {
            Ok(p) => p,
            Err(e) => {
                (gl.DeleteProgram)(prog_sky);
                return Err(e);
            }
        };

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let mut flag_vao = 0;
        let mut flag_vbo = 0;
        (gl.GenVertexArrays)(1, &mut flag_vao);
        (gl.GenBuffers)(1, &mut flag_vbo);

        (gl.BindVertexArray)(flag_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, flag_vbo);
        let byte_cap = (TRI_COUNT * 3 * VERTEX_FLOATS * std::mem::size_of::<f32>()) as GLsizeiptr;
        (gl.BufferData)(GL_ARRAY_BUFFER, byte_cap, std::ptr::null(), GL_DYNAMIC_DRAW);

        let stride = (VERTEX_FLOATS * std::mem::size_of::<f32>()) as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 3, GL_FLOAT, GL_FALSE, stride, std::ptr::null());
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(
            1,
            3,
            GL_FLOAT,
            GL_FALSE,
            stride,
            (3 * std::mem::size_of::<f32>()) as *const c_void,
        );
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(
            2,
            2,
            GL_FLOAT,
            GL_FALSE,
            stride,
            (6 * std::mem::size_of::<f32>()) as *const c_void,
        );
        (gl.BindVertexArray)(0);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, 0);

        Ok(Self {
            s_res: gl.uniform_loc(prog_sky, "u_res"),
            s_time: gl.uniform_loc(prog_sky, "u_time"),
            s_sky_mode: gl.uniform_loc(prog_sky, "u_sky_mode"),
            s_env_hour: gl.uniform_loc(prog_sky, "u_env_hour"),
            s_clouds: gl.uniform_loc(prog_sky, "u_clouds"),
            s_atmosphere: gl.uniform_loc(prog_sky, "u_atmosphere"),
            s_wind: gl.uniform_loc(prog_sky, "u_wind"),
            s_adaptive: gl.uniform_loc(prog_sky, "u_adaptive"),
            s_icon_count: gl.uniform_loc(prog_sky, "u_icon_count"),
            s_icons: gl.uniform_loc(prog_sky, "u_icons"),
            s_pole_top_bot: gl.uniform_loc(prog_sky, "u_pole_top_bot"),
            s_flag_hoist_y: gl.uniform_loc(prog_sky, "u_flag_hoist_y"),
            s_pole_rad_px: gl.uniform_loc(prog_sky, "u_pole_rad_px"),

            f_proj_view: gl.uniform_loc(prog_flag, "u_proj_view"),
            f_time: gl.uniform_loc(prog_flag, "u_time"),
            f_sky_mode: gl.uniform_loc(prog_flag, "u_sky_mode"),
            f_env_hour: gl.uniform_loc(prog_flag, "u_env_hour"),
            f_atmosphere: gl.uniform_loc(prog_flag, "u_atmosphere"),
            f_fabric: gl.uniform_loc(prog_flag, "u_fabric"),
            f_cam_pos: gl.uniform_loc(prog_flag, "u_cam_pos"),

            prog_sky,
            prog_flag,
            dummy_vao,
            flag_vao,
            flag_vbo,
            mesh_scratch: Vec::with_capacity(TRI_COUNT * 3 * VERTEX_FLOATS),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.flag_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.flag_vbo);
            self.flag_vbo = 0;
        }
        if self.flag_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.flag_vao);
            self.flag_vao = 0;
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
        if self.prog_flag != 0 {
            (gl.DeleteProgram)(self.prog_flag);
            self.prog_flag = 0;
        }
        if self.prog_sky != 0 {
            (gl.DeleteProgram)(self.prog_sky);
            self.prog_sky = 0;
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let dpr = st.dpi_scale.max(0.5);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let aspect = (pw as f32 / ph as f32).max(0.25);

        let tf = &mut st.turkish_flag;
        tf.resize(css_w, css_h, dpr);

        let proj_view = build_proj_view(aspect);
        let pole_top_ndc = project_ndc(&proj_view, [POLE_X, TOP_Y + 0.08, 0.0]);
        let pole_bot_ndc = project_ndc(&proj_view, [POLE_X, -2.5, 0.0]);
        let flag_top_ndc = project_ndc(&proj_view, [POLE_X, TOP_Y, 0.0]);
        let flag_bot_ndc = project_ndc(&proj_view, [POLE_X, BOT_Y, 0.0]);
        let pole_rad_px = (ph as f32 * 0.0062).clamp(3.2, 16.0);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);

        // Pass 1: Sky, drifting clouds, distant ridge, 3D flagpole + golden alem + icon scrims
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(self.prog_sky);
        (gl.Uniform2f)(self.s_res, pw as f32, ph as f32);
        (gl.Uniform1f)(self.s_time, tf.anim_time);
        (gl.Uniform1i)(self.s_sky_mode, tf.params.sky_mode as GLint);
        (gl.Uniform1f)(self.s_env_hour, tf.env_hour);
        (gl.Uniform1f)(self.s_clouds, tf.params.clouds);
        (gl.Uniform1f)(self.s_atmosphere, tf.params.atmosphere);
        (gl.Uniform1f)(self.s_wind, tf.params.wind);
        (gl.Uniform1i)(self.s_adaptive, if tf.params.adaptive { 1 } else { 0 });
        (gl.Uniform4f)(
            self.s_pole_top_bot,
            pole_top_ndc[0],
            pole_top_ndc[1],
            pole_bot_ndc[0],
            pole_bot_ndc[1],
        );
        (gl.Uniform4f)(
            self.s_flag_hoist_y,
            flag_top_ndc[0],
            flag_top_ndc[1],
            flag_bot_ndc[0],
            flag_bot_ndc[1],
        );
        (gl.Uniform1f)(self.s_pole_rad_px, pole_rad_px);

        let ic_cnt = if tf.icons_visible { tf.icons.len().min(24) } else { 0 };
        (gl.Uniform1i)(self.s_icon_count, ic_cnt as GLint);
        if ic_cnt > 0 {
            let mut ibuf = [0.0f32; 24 * 4];
            for (i, ic) in tf.icons.iter().take(24).enumerate() {
                ibuf[i * 4] = ic.x * dpr;
                ibuf[i * 4 + 1] = ic.y * dpr;
                ibuf[i * 4 + 2] = ic.w * dpr;
                ibuf[i * 4 + 3] = ic.h * dpr;
            }
            (gl.Uniform4fv)(self.s_icons, ic_cnt as GLsizei, ibuf.as_ptr());
        }

        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Pass 2: 3D Verlet waving Turkish Flag cloth mesh (alpha-preserving blend)
        tf.cloth.build_mesh_vertices(CAM_POS, &mut self.mesh_scratch);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.flag_vbo);
        (gl.BufferSubData)(
            GL_ARRAY_BUFFER,
            0,
            (self.mesh_scratch.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            self.mesh_scratch.as_ptr() as *const c_void,
        );
        (gl.BindBuffer)(GL_ARRAY_BUFFER, 0);

        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
        (gl.UseProgram)(self.prog_flag);
        (gl.UniformMatrix4fv)(self.f_proj_view, 1, GL_FALSE, proj_view.as_ptr());
        (gl.Uniform1f)(self.f_time, tf.anim_time);
        (gl.Uniform1i)(self.f_sky_mode, tf.params.sky_mode as GLint);
        (gl.Uniform1f)(self.f_env_hour, tf.env_hour);
        (gl.Uniform1f)(self.f_atmosphere, tf.params.atmosphere);
        (gl.Uniform1i)(self.f_fabric, tf.params.fabric as GLint);
        (gl.Uniform3f)(self.f_cam_pos, CAM_POS[0], CAM_POS[1], CAM_POS[2]);

        (gl.BindVertexArray)(self.flag_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, (TRI_COUNT * 3) as GLsizei);
        (gl.BindVertexArray)(0);
        (gl.Disable)(GL_BLEND);
    }
}

// ============================================================================
// 5. UNIT & GPU RENDER TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use cloth::{FLAG_W, NX, NY};

    #[test]
    fn params_parsing_and_resize_threshold() {
        let mut raw = HashMap::new();
        raw.insert("skyMode".into(), Value::String("sunset".into()));
        raw.insert("clouds".into(), Value::from(1.25));
        raw.insert("atmosphere".into(), Value::from(1.3));
        raw.insert("wind".into(), Value::from(1.6));
        raw.insert("speed".into(), Value::from(1.2));
        raw.insert("fabric".into(), Value::String("ceremonial".into()));
        raw.insert("mouseInteract".into(), Value::Bool(false));
        raw.insert("adaptive".into(), Value::from(1));

        let p = TurkishFlagParams::from_raw_params(&raw);
        assert_eq!(p.sky_mode, 1);
        assert!((p.clouds - 1.25).abs() < 1e-5);
        assert!((p.atmosphere - 1.3).abs() < 1e-5);
        assert!((p.wind - 1.6).abs() < 1e-5);
        assert!((p.speed - 1.2).abs() < 1e-5);
        assert_eq!(p.fabric, 2);
        assert!(!p.mouse_interact);
        assert!(p.adaptive);

        let mut st = TurkishFlagState::new(1280.0, 720.0, 1.0);
        let r0 = st.rev;
        st.resize(1280.2, 720.2, 1.0);
        assert_eq!(st.rev, r0, "sub-0.5px resize must not bump rev");
        st.resize(1920.0, 1080.0, 1.0);
        assert_ne!(st.rev, r0);
    }

    #[test]
    fn flag_cloth_determinism_and_pinning() {
        let mut st_a = TurkishFlagState::new(1280.0, 720.0, 1.0);
        let mut st_b = TurkishFlagState::new(1280.0, 720.0, 1.0);

        for _ in 0..30 {
            st_a.step(0.033);
            st_b.step(0.033);
        }

        // Deterministic simulation
        for iy in 0..NY {
            for ix in 0..NX {
                let pa = st_a.cloth.node_pos(ix, iy);
                let pb = st_b.cloth.node_pos(ix, iy);
                assert!((pa[0] - pb[0]).abs() < 1e-6);
                assert!((pa[1] - pb[1]).abs() < 1e-6);
                assert!((pa[2] - pb[2]).abs() < 1e-6);
                assert!(pa[0].is_finite() && pa[1].is_finite() && pa[2].is_finite());
            }
        }

        // Left hoist edge remains strictly pinned to flagpole
        for iy in 0..NY {
            let p = st_a.cloth.node_pos(0, iy);
            assert!((p[0] - POLE_X).abs() < 1e-5);
            assert!((p[2]).abs() < 1e-5);
        }

        // Fly edge extends cleanly to the right of the pole
        let fly_mid = st_a.cloth.node_pos(NX - 1, NY / 2);
        assert!(
            fly_mid[0] > POLE_X + FLAG_W * 0.55,
            "flag fly edge collapsed unexpectedly: {}",
            fly_mid[0]
        );

        // Low wind sags lower than strong wind
        let mut low_wind = TurkishFlagState::new(1280.0, 720.0, 1.0);
        low_wind.params.wind = 0.25;
        let mut high_wind = TurkishFlagState::new(1280.0, 720.0, 1.0);
        high_wind.params.wind = 1.9;
        for _ in 0..60 {
            low_wind.step(0.033);
            high_wind.step(0.033);
        }
        let low_y = low_wind.cloth.node_pos(NX - 1, NY / 2)[1];
        let high_y = high_wind.cloth.node_pos(NX - 1, NY / 2)[1];
        assert!(
            low_y < high_y,
            "expected low wind to sag below high wind (low_y={low_y}, high_y={high_y})"
        );

        // Verify mesh vertex emission & low_mode
        let mut verts = Vec::new();
        st_a.cloth.build_mesh_vertices(CAM_POS, &mut verts);
        assert_eq!(verts.len(), TRI_COUNT * 3 * VERTEX_FLOATS);
        st_a.set_low_mode(true);
        st_a.step(0.033);
    }

    #[test]
    fn pointer_interaction_moves_grabs_and_ripples_cloth() {
        // 1. Hover swipe produces distinct 3D cloth motion compared to idle flag
        let mut idle = TurkishFlagState::new(1280.0, 720.0, 1.0);
        let mut hovered = TurkishFlagState::new(1280.0, 720.0, 1.0);
        hovered.handle_mouse(0, 620.0, 320.0);
        for i in 1..=12 {
            hovered.handle_mouse(0, 620.0 + i as f32 * 18.0, 320.0 - i as f32 * 6.0);
            hovered.step(0.025);
            idle.step(0.025);
        }
        let mut max_delta = 0.0f32;
        for iy in 0..NY {
            for ix in 0..NX {
                let pa = idle.cloth.node_pos(ix, iy);
                let pb = hovered.cloth.node_pos(ix, iy);
                assert!(pb[0].is_finite() && pb[1].is_finite() && pb[2].is_finite());
                let d = ((pa[0] - pb[0]).powi(2) + (pa[1] - pb[1]).powi(2) + (pa[2] - pb[2]).powi(2)).sqrt();
                max_delta = max_delta.max(d);
            }
        }
        assert!(max_delta > 0.015, "expected noticeable hover swipe displacement, got {max_delta}");

        // 2. Click-grab pulls nearest cloth region and leaves hoist edge pinned; release clears grab
        let mut dragged = TurkishFlagState::new(1280.0, 720.0, 1.0);
        let mut baseline = TurkishFlagState::new(1280.0, 720.0, 1.0);
        let grab_x = 640.0;
        let grab_y = 330.0;
        assert!(dragged.handle_mouse(1, grab_x, grab_y));
        assert!(dragged.grab_node.is_some(), "expected left down on flag to grab a cloth node");
        let grabbed_idx = dragged.grab_node.unwrap();
        let g_ix = grabbed_idx % NX;
        let g_iy = grabbed_idx / NX;

        for step_i in 1..=14 {
            dragged.handle_mouse(0, grab_x + step_i as f32 * 8.0, grab_y - step_i as f32 * 9.0);
            dragged.step(0.025);
            baseline.step(0.025);
        }
        let p_drag = dragged.cloth.node_pos(g_ix, g_iy);
        let p_base = baseline.cloth.node_pos(g_ix, g_iy);
        assert!(
            p_drag[1] > p_base[1] + 0.05,
            "expected upward drag on grabbed node (drag_y={}, base_y={})",
            p_drag[1],
            p_base[1]
        );
        for iy in 0..NY {
            let hoist = dragged.cloth.node_pos(0, iy);
            assert!((hoist[0] - POLE_X).abs() < 1e-5);
            assert!(hoist[2].abs() < 1e-5);
        }
        assert!(dragged.handle_mouse(2, grab_x + 112.0, grab_y - 126.0));
        assert!(dragged.grab_node.is_none());

        // 3. Disabled mouse_interact does not disturb cloth
        let mut disabled_a = TurkishFlagState::new(1280.0, 720.0, 1.0);
        let mut disabled_b = TurkishFlagState::new(1280.0, 720.0, 1.0);
        disabled_b.params.mouse_interact = false;
        disabled_a.params.mouse_interact = false;
        assert!(!disabled_b.handle_mouse(1, 640.0, 330.0));
        disabled_b.handle_mouse(0, 760.0, 260.0);
        disabled_a.step(0.033);
        disabled_b.step(0.033);
        let pa = disabled_a.cloth.node_pos(NX / 2, NY / 2);
        let pb = disabled_b.cloth.node_pos(NX / 2, NY / 2);
        assert!((pa[0] - pb[0]).abs() < 1e-6 && (pa[1] - pb[1]).abs() < 1e-6 && (pa[2] - pb[2]).abs() < 1e-6);
    }

    fn sd_star5_left_cpu(qx: f32, qy: f32, r: f32, rf: f32) -> f32 {
        let mut px = qy;
        let mut py = -qx;
        let k1x = 0.809016994375f32;
        let k1y = -0.587785252292f32;
        let k2x = -k1x;
        let k2y = k1y;
        px = px.abs();
        let d1 = (k1x * px + k1y * py).max(0.0);
        px -= 2.0 * d1 * k1x;
        py -= 2.0 * d1 * k1y;
        let d2 = (k2x * px + k2y * py).max(0.0);
        px -= 2.0 * d2 * k2x;
        py -= 2.0 * d2 * k2y;
        px = px.abs();
        py -= r;
        let bax = rf * (-k1y);
        let bay = rf * k1x - 1.0;
        let h = ((px * bax + py * bay) / (bax * bax + bay * bay)).clamp(0.0, r);
        let dx = px - bax * h;
        let dy = py - bay * h;
        let sgn = (py * bax - px * bay).signum();
        (dx * dx + dy * dy).sqrt() * sgn
    }

    #[test]
    fn turkish_flag_official_geometry_checks() {
        // Crescent spine (left side): inside outer (r=0.25 at x=0.5), outside inner (r=0.20 at x=0.5625)
        let d_spine = ((0.30f32 - 0.5).abs() - 0.25).max(-((0.30f32 - 0.5625).abs() - 0.20));
        assert!(d_spine < -0.02, "crescent spine must be white, d={d_spine}");

        // Crescent right opening at (0.72, 0.5): inside inner cutout -> red field
        let d_mouth = ((0.72f32 - 0.5).abs() - 0.25).max(-((0.72f32 - 0.5625).abs() - 0.20));
        assert!(d_mouth > 0.02, "crescent mouth must be open red, d={d_mouth}");

        // Star center (0.8208333, 0.5): strictly inside star
        let _sx = 0.82083333f32;
        let r_star = 0.125f32;
        let rf = 0.381966011f32;
        assert!(sd_star5_left_cpu(0.0, 0.0, r_star, rf) < -0.04);

        // Star left tip at x = sx - r_star (pointing straight at the crescent)
        let tip_d = sd_star5_left_cpu(-r_star, 0.0, r_star, rf);
        assert!(tip_d.abs() < 1e-4, "left tip must lie on star boundary, got {tip_d}");
        // Slightly inside left tip
        assert!(sd_star5_left_cpu(-r_star + 0.01, 0.0, r_star, rf) < 0.0);
        // Right side at +r_star is a valley (outside star!)
        assert!(sd_star5_left_cpu(r_star * 0.75, 0.0, r_star, rf) > 0.02);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn gpu_render_turkish_flag_pngs() {
        use crate::gl::WglContext;
        use crate::renderer::Renderer;
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExA, DefWindowProcA, DestroyWindow, RegisterClassA, WNDCLASSA, WS_POPUP,
        };

        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarTurkishFlagGpuTest\0";
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
                b"turkishflagtest\0".as_ptr(),
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
            assert!(!hwnd.is_null());
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
            st.set_theme("turkish-flag");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[48,48,116,136],[48,160,116,248]]}"#);

            let out_dir = std::env::var_os("DIYARDESK_FLAG_VERIFY_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let _ = std::fs::create_dir_all(&out_dir);

            let mut raw = vec![0u8; (pw * ph * 4) as usize];
            let mut flipped = vec![0u8; (pw * ph * 4) as usize];

            let mut render_shot = |name: &str, st: &mut AppState| -> Vec<u8> {
                while (gl.GetError)() != GL_NO_ERROR {}
                let _guard = DefaultFboGuard::new(fbo);
                (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
                renderer.render(&gl, st);
                let err = (gl.GetError)();
                assert_eq!(err, GL_NO_ERROR, "GL error 0x{:X} on {name}", err);
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
                assert!(raw.chunks_exact(4).all(|px| px[3] == 255), "{name} leaked non-opaque alpha");
                let crimson_px = raw
                    .chunks_exact(4)
                    .filter(|px| px[0] > 75 && px[0] > px[1].saturating_add(45) && px[0] > px[2].saturating_add(40))
                    .count();
                assert!(crimson_px > 12_000, "{name} too few crimson flag pixels ({crimson_px})");
                crate::flip_rgba_vertical(&raw, &mut flipped, pw as usize, ph as usize);
                if std::env::var_os("DIYARDESK_FLAG_VERIFY_DIR").is_some() {
                    let png_path = out_dir.join(name);
                    image::save_buffer(&png_path, &flipped, pw as u32, ph as u32, image::ColorType::Rgba8)
                        .expect("save turkish flag png");
                }
                raw.clone()
            };

            // 1. Daylight sky + drifting clouds + icons
            st.step(0.25);
            let day_buf = render_shot("verify_turkish_flag_daylight.png", &mut st);

            // 2. Golden sunset
            st.handle_json(r#"{"t":"settings","v":{"skyMode":"sunset","fabric":"silk"}}"#);
            st.step(0.15);
            let sunset_buf = render_shot("verify_turkish_flag_sunset.png", &mut st);
            assert_ne!(day_buf, sunset_buf);

            // 3. Starry night
            st.handle_json(r#"{"t":"settings","v":{"skyMode":"night","fabric":"ceremonial"}}"#);
            st.step(0.15);
            let night_buf = render_shot("verify_turkish_flag_night.png", &mut st);
            assert_ne!(sunset_buf, night_buf);

            // 4. Strong wind wave motion check (back to daylight, advanced time + strong wind)
            st.handle_json(r#"{"t":"settings","v":{"skyMode":"daylight","wind":1.75,"fabric":"cotton"}}"#);
            for _ in 0..24 {
                st.step(0.04);
            }
            let wind_buf = render_shot("verify_turkish_flag_wind_wave.png", &mut st);
            assert_ne!(day_buf, wind_buf);

            renderer.destroy(&gl);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
            (gl.DeleteTextures)(1, &tex);
            (gl.DeleteFramebuffers)(1, &fbo);
            drop(ctx);
            DestroyWindow(hwnd);
        }
    }
}
