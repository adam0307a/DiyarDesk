pub mod atlas;
pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use atlas::{
    build_marble_pattern, build_plank_pattern, build_radial_glow_rgba, build_static_city_and_mask,
    build_tile_pattern, build_wave_pattern, ist_hash, pal, qb, IstanbulGeo, Mulberry32,
    WaterGlint,
};
use chrono::{Local, Timelike};
use serde_json::Value;
use shaders::*;
use std::collections::HashMap;
use std::ffi::{c_void, CString};

const GL_ONE_MINUS_SRC_COLOR: GLenum = 0x0301;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct IstanbulParams {
    pub time: u8,       // 0=Auto, 1=Morning, 2=Noon, 3=Sunset, 4=Night
    pub glow: f32,      // 0.0..=1.5
    pub ferries: usize, // 0..=6
    pub flow: f32,      // 0.0..=3.0
    pub gulls: u8,      // 0=None, 1=Few, 2=Many -> [0, 5, 12]
    pub pad: f32,       // 6.0..=50.0
    pub tulips: bool,
    pub cats: bool,
    pub landmarks: bool,
    pub adaptive: bool,
}

impl Default for IstanbulParams {
    fn default() -> Self {
        Self {
            time: 3,
            glow: 1.0,
            ferries: 3,
            flow: 1.0,
            gulls: 1,
            pad: 14.0,
            tulips: true,
            cats: true,
            landmarks: true,
            adaptive: true,
        }
    }
}

fn parse_f32_val(v: &Value) -> Option<f32> {
    match v {
        Value::Number(n) => n.as_f64().map(|x| x as f32),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.trim().parse::<f32>().ok(),
        _ => None,
    }
}

fn parse_time_val(v: &Value, default: u8) -> u8 {
    if let Value::String(s) = v {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => return 0,
            "morning" | "dawn" => return 1,
            "noon" => return 2,
            "sunset" => return 3,
            "night" => return 4,
            other => {
                if let Ok(n) = other.parse::<f32>() {
                    return (n.round() as i32).clamp(0, 4) as u8;
                }
                return default;
            }
        }
    }
    parse_f32_val(v)
        .map(|n| (n.round() as i32).clamp(0, 4) as u8)
        .unwrap_or(default)
}

fn parse_gulls_val(v: &Value, default: u8) -> u8 {
    if let Value::String(s) = v {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => return 0,
            "few" => return 1,
            "many" => return 2,
            other => {
                if let Ok(n) = other.parse::<f32>() {
                    return (n.round() as i32).clamp(0, 2) as u8;
                }
                return default;
            }
        }
    }
    parse_f32_val(v)
        .map(|n| (n.round() as i32).clamp(0, 2) as u8)
        .unwrap_or(default)
}

fn parse_toggle_val(v: &Value, default: bool) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "show" | "on" | "true" | "1" | "yes" => true,
            "hide" | "off" | "false" | "0" | "no" => false,
            other => other.parse::<f64>().map(|x| x != 0.0).unwrap_or(default),
        },
        _ => default,
    }
}

impl IstanbulParams {
    pub fn apply_kv(&mut self, key: &str, val: &Value) {
        match key {
            "time" => self.time = parse_time_val(val, self.time),
            "glow" => {
                if let Some(f) = parse_f32_val(val) {
                    self.glow = f.clamp(0.0, 1.5);
                }
            }
            "ferries" => {
                if let Some(f) = parse_f32_val(val) {
                    self.ferries = (f.round() as i32).clamp(0, 6) as usize;
                }
            }
            "flow" => {
                if let Some(f) = parse_f32_val(val) {
                    self.flow = f.clamp(0.0, 3.0);
                }
            }
            "gulls" => self.gulls = parse_gulls_val(val, self.gulls),
            "pad" => {
                if let Some(f) = parse_f32_val(val) {
                    self.pad = f.clamp(6.0, 50.0);
                }
            }
            "tulips" => self.tulips = parse_toggle_val(val, self.tulips),
            "cats" => self.cats = parse_toggle_val(val, self.cats),
            "landmarks" => self.landmarks = parse_toggle_val(val, self.landmarks),
            "adaptive" => self.adaptive = parse_toggle_val(val, self.adaptive),
            _ => {}
        }
    }

    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        let mut p = Self::default();
        for (k, v) in raw {
            p.apply_kv(k, v);
        }
        p
    }

    pub fn from_json_map(map: &serde_json::Map<String, Value>) -> Self {
        let mut p = Self::default();
        for (k, v) in map {
            p.apply_kv(k, v);
        }
        p
    }

    /// Exact time index resolution (`prototype/istanbul.html:28`):
    /// `5..=9` -> 1 (Morning), `10..=16` -> 2 (Noon), `17..=19` -> 3 (Sunset), else -> 4 (Night).
    pub fn resolved_time_index(&self, env_hour: f32) -> usize {
        if self.time > 0 {
            (self.time as usize).clamp(1, 4)
        } else {
            let h = env_hour.rem_euclid(24.0).floor() as u32;
            match h {
                5..=9 => 1,
                10..=16 => 2,
                17..=19 => 3,
                _ => 4,
            }
        }
    }

    #[inline]
    pub fn gull_count(&self) -> usize {
        match self.gulls {
            0 => 0,
            1 => 5,
            2 => 12,
            _ => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ferry {
    pub u: f64,
    pub dir: f64,
    pub lane: f64,
    pub sp: f64,
    pub x: f64,
    pub y: f64,
    pub a: f64,
    pub len: f64,
}

#[derive(Clone, Debug)]
pub struct Gull {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
    pub sp: f64,
    pub d: f64,
    pub ph: f64,
    pub fl: f64,
    pub s: f64,
}

#[derive(Clone, Debug)]
pub struct Plaza {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub tx: f64,
    pub ty: f64,
    pub tw: f64,
    pub th: f64,
    pub k: f64,
    pub used: bool,
    pub sea: bool,
}

fn mk_ferries(n: usize) -> Vec<Ferry> {
    let mut r = Mulberry32::new(7);
    const LANES: [f64; 6] = [-0.45, 0.4, 0.0, 0.6, -0.2, 0.2];
    (0..n)
        .map(|i| {
            let u = r.next_f64();
            let dir = if i % 2 != 0 { -1.0 } else { 1.0 };
            let lane = LANES[i % 6] * dir;
            let sp = 0.016 + r.next_f64() * 0.008;
            Ferry {
                u,
                dir,
                lane,
                sp,
                x: 0.0,
                y: 0.0,
                a: 0.0,
                len: 60.0,
            }
        })
        .collect()
}

fn step_ferries(fs: &mut [Ferry], geo: &IstanbulGeo, flow: f64, dt: f64) {
    let f_len = (geo.w * 0.05).clamp(46.0, 96.0);
    for f in fs {
        f.u += f.dir * f.sp * dt * flow * 2.0;
        if f.u > 1.12 {
            f.u = -0.12;
        }
        if f.u < -0.12 {
            f.u = 1.12;
        }
        let at = |u: f64| -> (f64, f64) {
            let x = u * geo.w;
            (x, geo.cy(x) + f.lane * geo.hw(x) * 0.62)
        };
        let (x, y) = at(f.u);
        let (x2, y2) = at(f.u + 0.01 * f.dir);
        f.x = x;
        f.y = y;
        f.a = (y2 - y).atan2(x2 - x);
        f.len = f_len;
    }
}

fn mk_gulls(n: usize, w: f64, h: f64) -> Vec<Gull> {
    let mut r = Mulberry32::new(31);
    (0..n)
        .map(|_| Gull {
            cx: r.next_f64() * w,
            cy: r.next_f64() * h,
            rx: 60.0 + r.next_f64() * 120.0,
            ry: 30.0 + r.next_f64() * 60.0,
            sp: 0.25 + r.next_f64() * 0.3,
            d: if r.next_f64() < 0.5 { 1.0 } else { -1.0 },
            ph: r.next_f64() * 6.3,
            fl: r.next_f64() * 6.3,
            s: 6.0 + r.next_f64() * 4.0,
        })
        .collect()
}

pub struct IstanbulState {
    pub params: IstanbulParams,
    pub env_hour: f32,
    pub w: f32,
    pub h: f32,
    pub time: f64,
    pub geo: IstanbulGeo,
    pub glints: Vec<WaterGlint>,
    pub ferries: Vec<Ferry>,
    pub fer_n: isize,
    pub gulls: Vec<Gull>,
    pub gull_n: isize,
    pub plazas: Vec<Plaza>,
    pub pid: u64,
    pub last_raw_icons: Vec<Rect>,
    pub last_icons_vis: bool,
    pub static_rgba: Vec<u8>,
    pub water_mask_r8: Vec<u8>,
    pub static_w: i32,
    pub static_h: i32,
    pub static_dirty: bool,
    pub static_rev: u64,
    pub built_time_idx: usize,
    pub built_landmarks: bool,
    pub wave_rgba: Vec<u8>,
    pub wave_w: i32,
    pub wave_h: i32,
    pub wave_dirty: bool,
    pub wave_rev: u64,
    pub wave_time_idx: usize,
}

impl Default for IstanbulState {
    fn default() -> Self {
        Self::new()
    }
}

impl IstanbulState {
    pub fn new() -> Self {
        let now = Local::now();
        let env_hour = now.hour() as f32 + now.minute() as f32 / 60.0;
        let params = IstanbulParams::default();
        let t_idx = params.resolved_time_index(env_hour);
        Self {
            params,
            env_hour,
            w: 0.0,
            h: 0.0,
            time: 0.0,
            geo: IstanbulGeo::new(1280.0, 720.0),
            glints: Vec::new(),
            ferries: Vec::new(),
            fer_n: -1,
            gulls: Vec::new(),
            gull_n: -1,
            plazas: Vec::new(),
            pid: 0,
            last_raw_icons: Vec::new(),
            last_icons_vis: false,
            static_rgba: Vec::new(),
            water_mask_r8: Vec::new(),
            static_w: 0,
            static_h: 0,
            static_dirty: true,
            static_rev: 0,
            built_time_idx: t_idx,
            built_landmarks: params.landmarks,
            wave_rgba: Vec::new(),
            wave_w: 0,
            wave_h: 0,
            wave_dirty: true,
            wave_rev: 0,
            wave_time_idx: t_idx,
        }
    }

    pub fn set_env_hour(&mut self, hour: f32) {
        self.env_hour = hour;
        if self.params.time == 0 {
            let t_idx = self.params.resolved_time_index(hour);
            if t_idx != self.built_time_idx {
                self.static_dirty = true;
            }
            if t_idx != self.wave_time_idx {
                self.wave_dirty = true;
            }
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = w.max(64.0);
        let nh = h.max(64.0);
        if (nw - self.w).abs() > 0.5 || (nh - self.h).abs() > 0.5 {
            self.w = nw;
            self.h = nh;
            self.geo = IstanbulGeo::new(nw as f64, nh as f64);
            self.gull_n = -1;
            self.static_dirty = true;
            self.recompute_plaza_sea();
        }
    }

    pub fn sync_params(&mut self, params: IstanbulParams) {
        let new_t = params.resolved_time_index(self.env_hour);
        if new_t != self.built_time_idx {
            self.static_dirty = true;
        }
        if new_t != self.wave_time_idx {
            self.wave_dirty = true;
        }
        if params.landmarks != self.built_landmarks {
            self.static_dirty = true;
        }
        let adaptive_changed = params.adaptive != self.params.adaptive;
        self.params = params;
        if adaptive_changed {
            self.reapply_icons();
        }
    }

    pub fn set_icons(&mut self, rects: &[Rect], vis: bool) {
        self.last_raw_icons = rects.to_vec();
        self.last_icons_vis = vis;
        self.reapply_icons();
    }

    fn recompute_plaza_sea(&mut self) {
        for p in &mut self.plazas {
            p.sea = self.geo.in_water(p.tx + p.tw * 0.5, p.ty + p.th * 0.5);
        }
    }

    fn reapply_icons(&mut self) {
        let active: &[Rect] = if self.params.adaptive && self.last_icons_vis {
            &self.last_raw_icons
        } else {
            &[]
        };
        for p in &mut self.plazas {
            p.used = false;
        }
        for ic in active {
            let ix = ic.x as f64;
            let iy = ic.y as f64;
            let iw = ic.w as f64;
            let ih = ic.h as f64;
            let mut best_idx: Option<usize> = None;
            let mut best_d2 = 8100.0;
            for (idx, p) in self.plazas.iter().enumerate() {
                if p.used {
                    continue;
                }
                let dx = p.tx - ix;
                let dy = p.ty - iy;
                let d2 = dx * dx + dy * dy;
                if d2 < best_d2 {
                    best_d2 = d2;
                    best_idx = Some(idx);
                }
            }
            let target_idx = match best_idx {
                Some(idx) => idx,
                None => {
                    self.pid += 1;
                    self.plazas.push(Plaza {
                        id: self.pid,
                        x: ix,
                        y: iy,
                        w: iw,
                        h: ih,
                        tx: ix,
                        ty: iy,
                        tw: iw,
                        th: ih,
                        k: 0.0,
                        used: false,
                        sea: false,
                    });
                    self.plazas.len() - 1
                }
            };
            let p = &mut self.plazas[target_idx];
            p.used = true;
            p.tx = ix;
            p.ty = iy;
            p.tw = iw;
            p.th = ih;
        }
        self.recompute_plaza_sea();
    }

    pub fn ensure_actors_fresh(&mut self) {
        if self.fer_n != self.params.ferries as isize {
            self.ferries = mk_ferries(self.params.ferries);
            step_ferries(&mut self.ferries, &self.geo, self.params.flow as f64, 0.0);
            self.fer_n = self.params.ferries as isize;
        }
        if self.gull_n != self.params.gulls as isize {
            self.gulls = mk_gulls(self.params.gull_count(), self.geo.w, self.geo.h);
            self.gull_n = self.params.gulls as isize;
        }
    }

    pub fn ensure_static_fresh(&mut self) {
        if self.w < 64.0 || self.h < 64.0 {
            self.w = 1280.0;
            self.h = 720.0;
            self.geo = IstanbulGeo::new(1280.0, 720.0);
            self.static_dirty = true;
        }
        let t_idx = self.params.resolved_time_index(self.env_hour);
        if self.static_dirty
            || self.static_rev == 0
            || t_idx != self.built_time_idx
            || self.params.landmarks != self.built_landmarks
        {
            let out = build_static_city_and_mask(&self.geo, t_idx, self.params.landmarks);
            self.static_w = out.w;
            self.static_h = out.h;
            self.static_rgba = out.static_rgba;
            self.water_mask_r8 = out.water_mask_r8;
            self.glints = out.glints;
            self.built_time_idx = t_idx;
            self.built_landmarks = self.params.landmarks;
            self.static_dirty = false;
            self.static_rev = self.static_rev.wrapping_add(1);
        }
        if self.wave_dirty || self.wave_rev == 0 || t_idx != self.wave_time_idx {
            let (ww, wh, rgba) = build_wave_pattern(t_idx);
            self.wave_w = ww;
            self.wave_h = wh;
            self.wave_rgba = rgba;
            self.wave_time_idx = t_idx;
            self.wave_dirty = false;
            self.wave_rev = self.wave_rev.wrapping_add(1);
        }
    }

    pub fn step(&mut self, dt: f32) {
        let dt = (dt as f64).clamp(0.0, 0.1);
        self.time += dt;
        self.ensure_actors_fresh();
        step_ferries(&mut self.ferries, &self.geo, self.params.flow as f64, dt);

        let f = (dt * 9.0).min(1.0);
        let fk = (dt * 4.0).min(1.0);
        for p in &mut self.plazas {
            p.x += (p.tx - p.x) * f;
            p.y += (p.ty - p.y) * f;
            p.w += (p.tw - p.w) * f;
            p.h += (p.th - p.h) * f;
            let target_k = if p.used { 1.0 } else { 0.0 };
            p.k += (target_k - p.k) * fk;
            p.sea = self.geo.in_water(p.tx + p.tw * 0.5, p.ty + p.th * 0.5);
        }
        self.plazas.retain(|p| p.used || p.k > 0.02);
    }

    pub fn handle_mouse(&mut self, _kind: i32, _x: f32, _y: f32) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// OpenGL 3.3 Renderer (`IstanbulRenderer`)
// ---------------------------------------------------------------------------

unsafe fn uloc(gl: &GlFns, prog: GLuint, name: &str) -> GLint {
    let cs = CString::new(name).unwrap();
    (gl.GetUniformLocation)(prog, cs.as_ptr())
}

unsafe fn create_tex_rgba8(
    gl: &GlFns,
    w: i32,
    h: i32,
    wrap: GLint,
    data: &[u8],
) -> GLuint {
    let mut tex = 0;
    (gl.GenTextures)(1, &mut tex);
    (gl.BindTexture)(GL_TEXTURE_2D, tex);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, wrap);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, wrap);
    (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
    (gl.TexImage2D)(
        GL_TEXTURE_2D,
        0,
        GL_RGBA8 as GLint,
        w,
        h,
        0,
        GL_RGBA,
        GL_UNSIGNED_BYTE,
        if data.is_empty() {
            std::ptr::null()
        } else {
            data.as_ptr() as *const c_void
        },
    );
    tex
}

#[inline]
fn hsla_to_rgba(h_deg: f32, s: f32, l: f32, a: f32) -> [f32; 4] {
    let h = h_deg.rem_euclid(360.0) / 360.0;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue2rgb = |mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 0.5 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };
    [
        hue2rgb(h + 1.0 / 3.0),
        hue2rgb(h),
        hue2rgb(h - 1.0 / 3.0),
        a,
    ]
}

#[inline]
fn hex_f32(hex: u32, a: f32) -> [f32; 4] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        a,
    ]
}

pub struct IstanbulRenderer {
    prog_tex: GLuint,
    prog_water: GLuint,
    prog_color: GLuint,
    prog_pat_rect: GLuint,
    prog_post: GLuint,

    vao_tex: GLuint,
    vbo_tex: GLuint,
    vao_water: GLuint,
    vbo_water: GLuint,
    vao_col: GLuint,
    vbo_col: GLuint,
    vao_pos: GLuint,
    vbo_pos: GLuint,

    static_tex: GLuint,
    water_mask_tex: GLuint,
    wave_pat_tex: GLuint,
    tile_tex: GLuint,
    marble_tex: GLuint,
    plank_tex: GLuint,
    plaza_glow_tex: GLuint,

    uploaded_static_rev: u64,
    uploaded_static_w: i32,
    uploaded_static_h: i32,
    uploaded_wave_rev: u64,

    // Uniform locations
    u_tex_res: GLint,
    u_tex_sam: GLint,
    u_water_res: GLint,
    u_water_mask: GLint,
    u_water_wave: GLint,
    u_water_off0: GLint,
    u_water_off1: GLint,
    u_water_a0: GLint,
    u_water_a1: GLint,
    u_col_res: GLint,
    u_pat_res: GLint,
    u_pat_sam: GLint,
    u_pat_use_tex: GLint,
    u_pat_tile_size: GLint,
    u_pat_rect: GLint,
    u_pat_radius: GLint,
    u_pat_alpha: GLint,
    u_pat_fill_col: GLint,
    u_pat_stroke_w: GLint,
    u_pat_stroke_col: GLint,
    u_post_res: GLint,
    u_post_mode: GLint,
    u_post_color: GLint,
    u_post_center: GLint,
    u_post_radii: GLint,

    tex_verts: Vec<f32>,
    col_verts: Vec<f32>,
    pos_verts: Vec<f32>,
}

impl IstanbulRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_tex = gl.compile_program(IST_TEX_VS, IST_TEX_FS)?;
        let prog_water = gl.compile_program(IST_WATER_VS, IST_WATER_FS)?;
        let prog_color = gl.compile_program(IST_COLOR_VS, IST_COLOR_FS)?;
        let prog_pat_rect = gl.compile_program(IST_PATTERN_RECT_VS, IST_PATTERN_RECT_FS)?;
        let prog_post = gl.compile_program(IST_POST_VS, IST_POST_FS)?;

        let stride_tex = (5 * std::mem::size_of::<f32>()) as GLsizei;
        let mut vao_tex = 0;
        let mut vbo_tex = 0;
        (gl.GenVertexArrays)(1, &mut vao_tex);
        (gl.GenBuffers)(1, &mut vbo_tex);
        (gl.BindVertexArray)(vao_tex);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, vbo_tex);
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride_tex, std::ptr::null());
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(
            1,
            2,
            GL_FLOAT,
            GL_FALSE,
            stride_tex,
            (2 * std::mem::size_of::<f32>()) as *const c_void,
        );
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(
            2,
            1,
            GL_FLOAT,
            GL_FALSE,
            stride_tex,
            (4 * std::mem::size_of::<f32>()) as *const c_void,
        );

        let stride_water = (4 * std::mem::size_of::<f32>()) as GLsizei;
        let mut vao_water = 0;
        let mut vbo_water = 0;
        (gl.GenVertexArrays)(1, &mut vao_water);
        (gl.GenBuffers)(1, &mut vbo_water);
        (gl.BindVertexArray)(vao_water);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, vbo_water);
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride_water, std::ptr::null());
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(
            1,
            2,
            GL_FLOAT,
            GL_FALSE,
            stride_water,
            (2 * std::mem::size_of::<f32>()) as *const c_void,
        );

        let stride_col = (6 * std::mem::size_of::<f32>()) as GLsizei;
        let mut vao_col = 0;
        let mut vbo_col = 0;
        (gl.GenVertexArrays)(1, &mut vao_col);
        (gl.GenBuffers)(1, &mut vbo_col);
        (gl.BindVertexArray)(vao_col);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, vbo_col);
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride_col, std::ptr::null());
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(
            1,
            4,
            GL_FLOAT,
            GL_FALSE,
            stride_col,
            (2 * std::mem::size_of::<f32>()) as *const c_void,
        );

        let stride_pos = (2 * std::mem::size_of::<f32>()) as GLsizei;
        let mut vao_pos = 0;
        let mut vbo_pos = 0;
        (gl.GenVertexArrays)(1, &mut vao_pos);
        (gl.GenBuffers)(1, &mut vbo_pos);
        (gl.BindVertexArray)(vao_pos);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, vbo_pos);
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride_pos, std::ptr::null());

        (gl.BindBuffer)(GL_ARRAY_BUFFER, 0);
        (gl.BindVertexArray)(0);

        let static_tex = create_tex_rgba8(gl, 1, 1, GL_CLAMP_TO_EDGE, &[0, 0, 0, 255]);
        let mut water_mask_tex = 0;
        (gl.GenTextures)(1, &mut water_mask_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, water_mask_tex);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        let one_r8: [u8; 1] = [0];
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_R8 as GLint,
            1,
            1,
            0,
            GL_RED,
            GL_UNSIGNED_BYTE,
            one_r8.as_ptr() as *const c_void,
        );
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);

        let wave_pat_tex = create_tex_rgba8(gl, 1, 1, GL_REPEAT, &[0, 0, 0, 0]);
        let (tw, th, tile_px) = build_tile_pattern();
        let tile_tex = create_tex_rgba8(gl, tw, th, GL_REPEAT, &tile_px);
        let (mw, mh, marble_px) = build_marble_pattern();
        let marble_tex = create_tex_rgba8(gl, mw, mh, GL_REPEAT, &marble_px);
        let (pw, ph, plank_px) = build_plank_pattern();
        let plank_tex = create_tex_rgba8(gl, pw, ph, GL_REPEAT, &plank_px);
        let glow_px = build_radial_glow_rgba(128, [255, 200, 110]);
        let plaza_glow_tex = create_tex_rgba8(gl, 128, 128, GL_CLAMP_TO_EDGE, &glow_px);
        (gl.BindTexture)(GL_TEXTURE_2D, 0);

        Ok(Self {
            u_tex_res: uloc(gl, prog_tex, "uRes"),
            u_tex_sam: uloc(gl, prog_tex, "uTex"),
            u_water_res: uloc(gl, prog_water, "uRes"),
            u_water_mask: uloc(gl, prog_water, "uMaskTex"),
            u_water_wave: uloc(gl, prog_water, "uWaveTex"),
            u_water_off0: uloc(gl, prog_water, "uOffset0"),
            u_water_off1: uloc(gl, prog_water, "uOffset1"),
            u_water_a0: uloc(gl, prog_water, "uAlpha0"),
            u_water_a1: uloc(gl, prog_water, "uAlpha1"),
            u_col_res: uloc(gl, prog_color, "uRes"),
            u_pat_res: uloc(gl, prog_pat_rect, "uRes"),
            u_pat_sam: uloc(gl, prog_pat_rect, "uPatTex"),
            u_pat_use_tex: uloc(gl, prog_pat_rect, "uUseTex"),
            u_pat_tile_size: uloc(gl, prog_pat_rect, "uTileSize"),
            u_pat_rect: uloc(gl, prog_pat_rect, "uRect"),
            u_pat_radius: uloc(gl, prog_pat_rect, "uRadius"),
            u_pat_alpha: uloc(gl, prog_pat_rect, "uAlpha"),
            u_pat_fill_col: uloc(gl, prog_pat_rect, "uFillCol"),
            u_pat_stroke_w: uloc(gl, prog_pat_rect, "uStrokeWidth"),
            u_pat_stroke_col: uloc(gl, prog_pat_rect, "uStrokeCol"),
            u_post_res: uloc(gl, prog_post, "uRes"),
            u_post_mode: uloc(gl, prog_post, "uMode"),
            u_post_color: uloc(gl, prog_post, "uColor"),
            u_post_center: uloc(gl, prog_post, "uCenter"),
            u_post_radii: uloc(gl, prog_post, "uRadii"),

            prog_tex,
            prog_water,
            prog_color,
            prog_pat_rect,
            prog_post,
            vao_tex,
            vbo_tex,
            vao_water,
            vbo_water,
            vao_col,
            vbo_col,
            vao_pos,
            vbo_pos,
            static_tex,
            water_mask_tex,
            wave_pat_tex,
            tile_tex,
            marble_tex,
            plank_tex,
            plaza_glow_tex,
            uploaded_static_rev: u64::MAX,
            uploaded_static_w: 0,
            uploaded_static_h: 0,
            uploaded_wave_rev: u64::MAX,
            tex_verts: Vec::with_capacity(64),
            col_verts: Vec::with_capacity(4096),
            pos_verts: Vec::with_capacity(24),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        let texs = [
            self.static_tex,
            self.water_mask_tex,
            self.wave_pat_tex,
            self.tile_tex,
            self.marble_tex,
            self.plank_tex,
            self.plaza_glow_tex,
        ];
        (gl.DeleteTextures)(texs.len() as GLsizei, texs.as_ptr());
        let vbos = [self.vbo_tex, self.vbo_water, self.vbo_col, self.vbo_pos];
        (gl.DeleteBuffers)(vbos.len() as GLsizei, vbos.as_ptr());
        let vaos = [self.vao_tex, self.vao_water, self.vao_col, self.vao_pos];
        (gl.DeleteVertexArrays)(vaos.len() as GLsizei, vaos.as_ptr());
        for p in [
            self.prog_tex,
            self.prog_water,
            self.prog_color,
            self.prog_pat_rect,
            self.prog_post,
        ] {
            if p != 0 {
                (gl.DeleteProgram)(p);
            }
        }
    }

    #[inline]
    fn push_tri(buf: &mut Vec<f32>, a: [f32; 2], b: [f32; 2], c: [f32; 2], col: [f32; 4]) {
        buf.extend_from_slice(&[
            a[0], a[1], col[0], col[1], col[2], col[3],
            b[0], b[1], col[0], col[1], col[2], col[3],
            c[0], c[1], col[0], col[1], col[2], col[3],
        ]);
    }

    #[inline]
    fn push_tri_vcols(
        buf: &mut Vec<f32>,
        a: [f32; 2],
        ca: [f32; 4],
        b: [f32; 2],
        cb: [f32; 4],
        c: [f32; 2],
        cc: [f32; 4],
    ) {
        buf.extend_from_slice(&[
            a[0], a[1], ca[0], ca[1], ca[2], ca[3],
            b[0], b[1], cb[0], cb[1], cb[2], cb[3],
            c[0], c[1], cc[0], cc[1], cc[2], cc[3],
        ]);
    }

    #[inline]
    fn push_quad(buf: &mut Vec<f32>, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        let a = [x, y];
        let b = [x + w, y];
        let c = [x + w, y + h];
        let d = [x, y + h];
        Self::push_tri(buf, a, b, c, col);
        Self::push_tri(buf, a, c, d, col);
    }

    #[inline]
    fn push_rot_rect(
        buf: &mut Vec<f32>,
        ox: f32,
        oy: f32,
        cos_a: f32,
        sin_a: f32,
        lx: f32,
        ly: f32,
        lw: f32,
        lh: f32,
        col: [f32; 4],
    ) {
        let xform = |px: f32, py: f32| -> [f32; 2] {
            [ox + px * cos_a - py * sin_a, oy + px * sin_a + py * cos_a]
        };
        let a = xform(lx, ly);
        let b = xform(lx + lw, ly);
        let c = xform(lx + lw, ly + lh);
        let d = xform(lx, ly + lh);
        Self::push_tri(buf, a, b, c, col);
        Self::push_tri(buf, a, c, d, col);
    }

    fn push_convex_poly(buf: &mut Vec<f32>, pts: &[[f32; 2]], col: [f32; 4]) {
        if pts.len() < 3 {
            return;
        }
        let p0 = pts[0];
        for i in 1..(pts.len() - 1) {
            Self::push_tri(buf, p0, pts[i], pts[i + 1], col);
        }
    }

    fn push_ellipse(
        buf: &mut Vec<f32>,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        cos_a: f32,
        sin_a: f32,
        col: [f32; 4],
        segs: usize,
    ) {
        let mut pts = Vec::with_capacity(segs);
        for i in 0..segs {
            let a = i as f32 / segs as f32 * std::f32::consts::TAU;
            let lx = a.cos() * rx;
            let ly = a.sin() * ry;
            pts.push([cx + lx * cos_a - ly * sin_a, cy + lx * sin_a + ly * cos_a]);
        }
        Self::push_convex_poly(buf, &pts, col);
    }

    fn push_ellipse_ring(
        buf: &mut Vec<f32>,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        width: f32,
        col: [f32; 4],
        segs: usize,
    ) {
        let hw = width * 0.5;
        let rx0 = (rx - hw).max(0.1);
        let ry0 = (ry - hw).max(0.1);
        let rx1 = rx + hw;
        let ry1 = ry + hw;
        for i in 0..segs {
            let a0 = i as f32 / segs as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / segs as f32 * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            let i0 = [cx + c0 * rx0, cy + s0 * ry0];
            let o0 = [cx + c0 * rx1, cy + s0 * ry1];
            let i1 = [cx + c1 * rx0, cy + s1 * ry0];
            let o1 = [cx + c1 * rx1, cy + s1 * ry1];
            Self::push_tri(buf, i0, o0, o1, col);
            Self::push_tri(buf, i0, o1, i1, col);
        }
    }

    fn push_seg(
        buf: &mut Vec<f32>,
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        ca: [f32; 4],
        cb: [f32; 4],
    ) {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = dx.hypot(dy);
        if len <= 1e-5 {
            return;
        }
        let hw = width * 0.5;
        let nx = -dy / len * hw;
        let ny = dx / len * hw;
        let a0 = [a[0] + nx, a[1] + ny];
        let a1 = [a[0] - nx, a[1] - ny];
        let b0 = [b[0] + nx, b[1] + ny];
        let b1 = [b[0] - nx, b[1] - ny];
        Self::push_tri_vcols(buf, a0, ca, b0, cb, b1, cb);
        Self::push_tri_vcols(buf, a0, ca, b1, cb, a1, ca);
    }

    fn push_closed_stroke(
        buf: &mut Vec<f32>,
        pts: &[[f32; 2]],
        width: f32,
        col: [f32; 4],
    ) {
        let n = pts.len();
        for i in 0..n {
            Self::push_seg(buf, pts[i], pts[(i + 1) % n], width, col, col);
        }
    }

    unsafe fn flush_col(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.col_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_color);
        (gl.Uniform2f)(self.u_col_res, css_w, css_h);
        (gl.BindVertexArray)(self.vao_col);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.vbo_col);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.col_verts.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            self.col_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, (self.col_verts.len() / 6) as GLsizei);
        self.col_verts.clear();
    }

    unsafe fn flush_tex(&mut self, gl: &GlFns, tex: GLuint, css_w: f32, css_h: f32) {
        if self.tex_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_tex);
        (gl.Uniform2f)(self.u_tex_res, css_w, css_h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, tex);
        (gl.Uniform1i)(self.u_tex_sam, 0);
        (gl.BindVertexArray)(self.vao_tex);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.vbo_tex);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.tex_verts.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            self.tex_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, (self.tex_verts.len() / 5) as GLsizei);
        self.tex_verts.clear();
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_pattern_rect(
        &mut self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        pat_tex: GLuint,
        use_tex: bool,
        tile_w: f32,
        tile_h: f32,
        rect: [f32; 4],
        radius: f32,
        alpha: f32,
        fill_col: [f32; 4],
        stroke_w: f32,
        stroke_col: [f32; 4],
    ) {
        let pad = stroke_w * 0.5 + 2.0;
        let x0 = rect[0] - pad;
        let y0 = rect[1] - pad;
        let x1 = rect[0] + rect[2] + pad;
        let y1 = rect[1] + rect[3] + pad;
        self.pos_verts.clear();
        self.pos_verts.extend_from_slice(&[
            x0, y0, x1, y0, x1, y1,
            x0, y0, x1, y1, x0, y1,
        ]);
        (gl.UseProgram)(self.prog_pat_rect);
        (gl.Uniform2f)(self.u_pat_res, css_w, css_h);
        if use_tex {
            (gl.ActiveTexture)(GL_TEXTURE0);
            (gl.BindTexture)(GL_TEXTURE_2D, pat_tex);
            (gl.Uniform1i)(self.u_pat_sam, 0);
        }
        (gl.Uniform1i)(self.u_pat_use_tex, if use_tex { 1 } else { 0 });
        (gl.Uniform2f)(self.u_pat_tile_size, tile_w, tile_h);
        (gl.Uniform4f)(self.u_pat_rect, rect[0], rect[1], rect[2], rect[3]);
        (gl.Uniform1f)(self.u_pat_radius, radius);
        (gl.Uniform1f)(self.u_pat_alpha, alpha);
        (gl.Uniform4f)(
            self.u_pat_fill_col,
            fill_col[0],
            fill_col[1],
            fill_col[2],
            fill_col[3],
        );
        (gl.Uniform1f)(self.u_pat_stroke_w, stroke_w);
        (gl.Uniform4f)(
            self.u_pat_stroke_col,
            stroke_col[0],
            stroke_col[1],
            stroke_col[2],
            stroke_col[3],
        );
        (gl.BindVertexArray)(self.vao_pos);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.vbo_pos);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.pos_verts.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            self.pos_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, 6);
    }

    unsafe fn draw_post(
        &mut self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        mode: i32,
        color: [f32; 4],
        center: [f32; 2],
        radii: [f32; 2],
    ) {
        self.pos_verts.clear();
        self.pos_verts.extend_from_slice(&[
            0.0, 0.0, css_w, 0.0, css_w, css_h,
            0.0, 0.0, css_w, css_h, 0.0, css_h,
        ]);
        (gl.UseProgram)(self.prog_post);
        (gl.Uniform2f)(self.u_post_res, css_w, css_h);
        (gl.Uniform1i)(self.u_post_mode, mode);
        (gl.Uniform4f)(self.u_post_color, color[0], color[1], color[2], color[3]);
        (gl.Uniform2f)(self.u_post_center, center[0], center[1]);
        (gl.Uniform2f)(self.u_post_radii, radii[0], radii[1]);
        (gl.BindVertexArray)(self.vao_pos);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.vbo_pos);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.pos_verts.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            self.pos_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, 6);
    }

    fn emit_ferry(buf: &mut Vec<f32>, f: &Ferry) {
        let l = f.len as f32;
        let ox = f.x as f32;
        let oy = f.y as f32;
        let (sin_a, cos_a) = (f.a as f32).sin_cos();
        let xform = |lx: f32, ly: f32| -> [f32; 2] {
            [ox + lx * cos_a - ly * sin_a, oy + lx * sin_a + ly * cos_a]
        };

        // 1. Wake fill
        let wf0 = xform(-0.45 * l, -0.10 * l);
        let wf1 = xform(-2.40 * l, -0.34 * l);
        let wf2 = xform(-2.40 * l, 0.34 * l);
        let wf3 = xform(-0.45 * l, 0.10 * l);
        let wca = [1.0, 1.0, 1.0, 0.16];
        let wcb = [1.0, 1.0, 1.0, 0.0];
        Self::push_tri_vcols(buf, wf0, wca, wf1, wcb, wf2, wcb);
        Self::push_tri_vcols(buf, wf0, wca, wf2, wcb, wf3, wca);

        // 2. Wake side strokes
        let wla = [1.0, 1.0, 1.0, 0.60];
        Self::push_seg(buf, wf0, wf1, 1.6, wla, wcb);
        Self::push_seg(buf, wf3, wf2, 1.6, wla, wcb);

        // Build hull symmetric Bezier samples (`top[0..=10]`, `bot[0..=10]`)
        let mut top_curve = [[0.0f32; 2]; 11];
        let mut bot_curve = [[0.0f32; 2]; 11];
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let u = 1.0 - t;
            let x = u * u * u * (0.5 * l)
                + 3.0 * u * u * t * (0.38 * l)
                + 3.0 * u * t * t * (0.20 * l)
                + t * t * t * (0.10 * l);
            let y = 3.0 * u * u * t * (-0.17 * l)
                + 3.0 * u * t * t * (-0.17 * l)
                + t * t * t * (-0.17 * l);
            top_curve[i] = [x, y];
            bot_curve[i] = [x, -y];
        }

        let push_hull = |buf: &mut Vec<f32>, dx: f32, dy: f32, col: [f32; 4]| {
            let xf = |p: [f32; 2]| xform(p[0] + dx, p[1] + dy);
            let bow = xf(top_curve[0]);
            Self::push_tri(buf, bow, xf(top_curve[1]), xf(bot_curve[1]), col);
            for i in 1..10 {
                let t0 = xf(top_curve[i]);
                let t1 = xf(top_curve[i + 1]);
                let b0 = xf(bot_curve[i]);
                let b1 = xf(bot_curve[i + 1]);
                Self::push_tri(buf, t0, t1, b0, col);
                Self::push_tri(buf, t1, b1, b0, col);
            }
            let stern_strips = [
                (top_curve[10], [-0.45 * l, -0.17 * l], bot_curve[10], [-0.45 * l, 0.17 * l]),
                ([-0.45 * l, -0.17 * l], [-0.50 * l, -0.10 * l], [-0.45 * l, 0.17 * l], [-0.50 * l, 0.10 * l]),
            ];
            for (ta, tb, ba, bb) in stern_strips {
                let pta = xf(ta);
                let ptb = xf(tb);
                let pba = xf(ba);
                let pbb = xf(bb);
                Self::push_tri(buf, pta, ptb, pba, col);
                Self::push_tri(buf, ptb, pbb, pba, col);
            }
        };

        // 3. Hull drop shadow (ferry-local (+3, +4))
        push_hull(buf, 3.0, 4.0, [0.0, 0.0, 0.0, 0.25]);

        // 4. Hull fill + outline stroke
        push_hull(buf, 0.0, 0.0, hex_f32(0xf4f1ea, 1.0));
        let mut perimeter = Vec::with_capacity(25);
        for p in &top_curve {
            perimeter.push(xform(p[0], p[1]));
        }
        perimeter.push(xform(-0.45 * l, -0.17 * l));
        perimeter.push(xform(-0.50 * l, -0.10 * l));
        perimeter.push(xform(-0.50 * l, 0.10 * l));
        perimeter.push(xform(-0.45 * l, 0.17 * l));
        for p in bot_curve[1..=10].iter().rev() {
            perimeter.push(xform(p[0], p[1]));
        }
        Self::push_closed_stroke(buf, &perimeter, 1.0, hex_f32(0x3d4a58, 1.0));

        // 5. Deckhouse & wheelhouse
        Self::push_rot_rect(
            buf,
            ox,
            oy,
            cos_a,
            sin_a,
            -0.38 * l,
            -0.11 * l,
            0.50 * l,
            0.22 * l,
            hex_f32(0xc9d2d8, 1.0),
        );
        Self::push_rot_rect(
            buf,
            ox,
            oy,
            cos_a,
            sin_a,
            -0.10 * l,
            -0.09 * l,
            0.20 * l,
            0.18 * l,
            hex_f32(0xe9e5dc, 1.0),
        );

        // 6. Twin funnels
        for fx in [-0.28f32, -0.14f32] {
            let fc = xform(l * fx, 0.0);
            Self::push_ellipse(
                buf,
                fc[0],
                fc[1],
                0.075 * l,
                0.075 * l,
                1.0,
                0.0,
                hex_f32(0xe45d2a, 1.0),
                16,
            );
            Self::push_ellipse(
                buf,
                fc[0],
                fc[1],
                0.042 * l,
                0.042 * l,
                1.0,
                0.0,
                hex_f32(0x2a2a2a, 1.0),
                12,
            );
        }

        // 7. Chord-closed bow foam arc (-1.4..=1.4 rad around (0.5L, 0))
        let foam_steps = 12usize;
        let mut foam_pts = Vec::with_capacity(foam_steps + 1);
        for i in 0..=foam_steps {
            let ang = -1.4 + 2.8 * (i as f32 / foam_steps as f32);
            let lx = 0.5 * l + ang.cos() * (0.05 * l);
            let ly = ang.sin() * (0.05 * l);
            foam_pts.push(xform(lx, ly));
        }
        Self::push_convex_poly(buf, &foam_pts, [1.0, 1.0, 1.0, 0.55]);
    }

    fn emit_ferry_windows(buf: &mut Vec<f32>, f: &Ferry, alpha: f32) {
        let l = f.len as f32;
        let ox = f.x as f32;
        let oy = f.y as f32;
        let (sin_a, cos_a) = (f.a as f32).sin_cos();
        let col = [1.0, 214.0 / 255.0, 130.0 / 255.0, alpha];
        for i in 0..7 {
            let wx = -0.4 * l + i as f32 * 0.13 * l;
            Self::push_rot_rect(buf, ox, oy, cos_a, sin_a, wx, -0.15 * l, 2.0, 2.0, col);
            Self::push_rot_rect(buf, ox, oy, cos_a, sin_a, wx, 0.13 * l, 2.0, 2.0, col);
        }
    }

    fn emit_gull(buf: &mut Vec<f32>, q: &Gull, t: f64) {
        let a = q.ph + t * q.sp * q.d;
        let x = q.cx + a.cos() * q.rx;
        let y = q.cy + a.sin() * q.ry;
        let hd = (a.cos() * q.ry * q.d).atan2(-a.sin() * q.rx * q.d) as f32;
        let fl = (t * 8.0 + q.fl).sin();
        let (sin_h, cos_h) = hd.sin_cos();

        for sh in [1.0f64, 0.0f64] {
            let ox = (x + sh * 14.0) as f32;
            let oy = (y + sh * 20.0) as f32;
            let s = (q.s * if sh != 0.0 { 0.8 } else { 1.0 }) as f32;
            let wy = s * (1.5 + fl as f32 * 0.5);
            let xf = |lx: f32, ly: f32| -> [f32; 2] {
                [ox + lx * cos_h - ly * sin_h, oy + lx * sin_h + ly * cos_h]
            };
            let v0 = xf(s * 0.7, 0.0);
            let v1 = xf(-s * 0.1, -s * 0.25);
            let v2 = xf(-s * 0.4, -wy);
            let v3 = xf(-s * 0.85, -wy * 0.8);
            let v4 = xf(-s * 0.55, 0.0);
            let v5 = xf(-s * 0.85, wy * 0.8);
            let v6 = xf(-s * 0.4, wy);
            let v7 = xf(-s * 0.1, s * 0.25);

            let col = if sh != 0.0 {
                [0.0, 0.0, 0.0, 0.13]
            } else {
                hex_f32(0xf5f4ef, 1.0)
            };
            // 6 non-overlapping triangles (R3 clean convex decomposition)
            Self::push_tri(buf, v0, v1, v7, col);
            Self::push_tri(buf, v1, v4, v7, col);
            Self::push_tri(buf, v1, v2, v4, col);
            Self::push_tri(buf, v2, v3, v4, col);
            Self::push_tri(buf, v7, v4, v6, col);
            Self::push_tri(buf, v4, v5, v6, col);

            if sh == 0.0 {
                let stroke_col = [70.0 / 255.0, 80.0 / 255.0, 90.0 / 255.0, 0.55];
                let outline = [v0, v1, v2, v3, v4, v5, v6, v7];
                Self::push_closed_stroke(buf, &outline, 0.7, stroke_col);
            }
        }
    }

    fn emit_tulips(buf: &mut Vec<f32>, x: f32, y: f32, w: f32, h: f32, t: f64, alpha: f32) {
        const TULIP_COLS: [u32; 5] = [0xd8322d, 0xf2c230, 0xc43a8f, 0xf6f1e6, 0xf08a2c];
        let leaf_col = hex_f32(0x3f7a3a, alpha);
        let st = 14.0f32;
        let mut pts = Vec::new();
        let mut u = 0.0f32;
        while u <= w {
            pts.push((x + u, y - 6.0));
            pts.push((x + u, y + h + 6.0));
            u += st;
        }
        let mut v = st;
        while v < h {
            pts.push((x - 6.0, y + v));
            pts.push((x + w + 6.0, y + v));
            v += st;
        }
        for (i, (px, py)) in pts.into_iter().enumerate() {
            let sw = (t * 1.4 + i as f64).sin() as f32 * 0.9;
            Self::push_ellipse(buf, px, py + 2.5, 1.6, 3.4, 1.0, 0.0, leaf_col, 10);
            let bcol = hex_f32(TULIP_COLS[i % 5], alpha);
            Self::push_ellipse(buf, px + sw, py - 1.0, 2.5, 3.3, 1.0, 0.0, bcol, 10);
        }
    }

    fn emit_cat(
        buf: &mut Vec<f32>,
        cx: f32,
        cy: f32,
        angle: f32,
        coat: [f32; 4],
        t: f64,
        alpha: f32,
    ) {
        let (sin_a, cos_a) = angle.sin_cos();
        let xf = |lx: f32, ly: f32| -> [f32; 2] {
            [cx + lx * cos_a - ly * sin_a, cy + lx * sin_a + ly * cos_a]
        };
        let wig = (t * 1.6).sin() as f32;

        // Shadow ellipse at local (1.5, 2.0)
        let sh_c = xf(1.5, 2.0);
        Self::push_ellipse(
            buf,
            sh_c[0],
            sh_c[1],
            7.0,
            4.2,
            cos_a,
            sin_a,
            [0.0, 0.0, 0.0, 0.2 * alpha],
            14,
        );

        // Swaying tail quadratic Bezier (-6,0) -> (-11, w*4+2) -> (-13, -3+w*3)
        let p0 = [-6.0f64, 0.0];
        let p1 = [-11.0, wig as f64 * 4.0 + 2.0];
        let p2 = [-13.0, -3.0 + wig as f64 * 3.0];
        let steps = 10usize;
        let mut prev = {
            let pt = qb(p0, p1, p2, 0.0);
            xf(pt[0] as f32, pt[1] as f32)
        };
        Self::push_ellipse(buf, prev[0], prev[1], 1.0, 1.0, 1.0, 0.0, coat, 8);
        for i in 1..=steps {
            let pt = qb(p0, p1, p2, i as f64 / steps as f64);
            let cur = xf(pt[0] as f32, pt[1] as f32);
            Self::push_seg(buf, prev, cur, 2.0, coat, coat);
            Self::push_ellipse(buf, cur[0], cur[1], 1.0, 1.0, 1.0, 0.0, coat, 8);
            prev = cur;
        }

        // Body ellipse
        Self::push_ellipse(buf, cx, cy, 7.0, 4.2, cos_a, sin_a, coat, 14);
        // Head circle
        let hc = xf(7.0, 0.0);
        Self::push_ellipse(buf, hc[0], hc[1], 3.4, 3.4, 1.0, 0.0, coat, 12);
        // Ears
        Self::push_tri(buf, xf(7.6, -2.4), xf(9.0, -5.0), xf(10.0, -2.0), coat);
        Self::push_tri(buf, xf(7.6, 2.4), xf(9.0, 5.0), xf(10.0, 2.0), coat);
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();

        st.istanbul.set_env_hour(st.env_hour);
        st.istanbul.resize(css_w, css_h);
        st.istanbul.ensure_static_fresh();
        st.istanbul.ensure_actors_fresh();

        // Re-upload static city + water mask ONLY when static_rev changes
        if self.uploaded_static_rev != st.istanbul.static_rev
            || self.uploaded_static_w != st.istanbul.static_w
            || self.uploaded_static_h != st.istanbul.static_h
        {
            let sw = st.istanbul.static_w;
            let sh = st.istanbul.static_h;
            if sw > 0 && sh > 0 {
                (gl.BindTexture)(GL_TEXTURE_2D, self.static_tex);
                (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_RGBA8 as GLint,
                    sw,
                    sh,
                    0,
                    GL_RGBA,
                    GL_UNSIGNED_BYTE,
                    st.istanbul.static_rgba.as_ptr() as *const c_void,
                );

                (gl.BindTexture)(GL_TEXTURE_2D, self.water_mask_tex);
                (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_R8 as GLint,
                    sw,
                    sh,
                    0,
                    GL_RED,
                    GL_UNSIGNED_BYTE,
                    st.istanbul.water_mask_r8.as_ptr() as *const c_void,
                );
                (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);

                self.uploaded_static_rev = st.istanbul.static_rev;
                self.uploaded_static_w = sw;
                self.uploaded_static_h = sh;
            }
        }

        // Re-upload wave pattern ONLY when wave_rev changes
        if self.uploaded_wave_rev != st.istanbul.wave_rev && st.istanbul.wave_w > 0 {
            (gl.BindTexture)(GL_TEXTURE_2D, self.wave_pat_tex);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                st.istanbul.wave_w,
                st.istanbul.wave_h,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                st.istanbul.wave_rgba.as_ptr() as *const c_void,
            );
            self.uploaded_wave_rev = st.istanbul.wave_rev;
        }

        let ist = &st.istanbul;
        let t_idx = ist.params.resolved_time_index(ist.env_hour);
        let pal = pal(t_idx);
        let t = ist.time;
        let glow = ist.params.glow;
        let flow = ist.params.flow as f64;

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);
        (gl.Disable)(GL_SCISSOR_TEST);
        (gl.ClearColor)(0.0, 0.0, 0.0, 1.0);
        (gl.Clear)(GL_COLOR_BUFFER_BIT);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(
            GL_SRC_ALPHA,
            GL_ONE_MINUS_SRC_ALPHA,
            GL_ONE,
            GL_ONE_MINUS_SRC_ALPHA,
        );

        // Layer 1: Blit static city panorama
        self.tex_verts.clear();
        self.tex_verts.extend_from_slice(&[
            0.0, 0.0, 0.0, 0.0, 1.0,
            css_w, 0.0, 1.0, 0.0, 1.0,
            css_w, css_h, 1.0, 1.0, 1.0,
            0.0, 0.0, 0.0, 0.0, 1.0,
            css_w, css_h, 1.0, 1.0, 1.0,
            0.0, css_h, 0.0, 1.0, 1.0,
        ]);
        self.flush_tex(gl, self.static_tex, css_w, css_h);

        // Layer 2a: Two clipped scrolling water wave passes
        let ox0 = ((t * 14.0 * flow * 0.2_f64.cos()) % 256.0) as f32;
        let oy0 = ((t * 14.0 * flow * 0.2_f64.sin()) % 256.0) as f32;
        let ox1 = ((t * 9.0 * flow * 2.9_f64.cos()) % 256.0) as f32;
        let oy1 = ((t * 9.0 * flow * 2.9_f64.sin()) % 256.0) as f32;
        let water_quad: [f32; 24] = [
            0.0, 0.0, 0.0, 0.0,
            css_w, 0.0, 1.0, 0.0,
            css_w, css_h, 1.0, 1.0,
            0.0, 0.0, 0.0, 0.0,
            css_w, css_h, 1.0, 1.0,
            0.0, css_h, 0.0, 1.0,
        ];
        (gl.UseProgram)(self.prog_water);
        (gl.Uniform2f)(self.u_water_res, css_w, css_h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.water_mask_tex);
        (gl.Uniform1i)(self.u_water_mask, 0);
        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.wave_pat_tex);
        (gl.Uniform1i)(self.u_water_wave, 1);
        (gl.Uniform2f)(self.u_water_off0, ox0, oy0);
        (gl.Uniform2f)(self.u_water_off1, ox1, oy1);
        (gl.Uniform1f)(self.u_water_a0, 0.55);
        (gl.Uniform1f)(self.u_water_a1, 0.35);
        (gl.BindVertexArray)(self.vao_water);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.vbo_water);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (water_quad.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            water_quad.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, 6);
        (gl.ActiveTexture)(GL_TEXTURE0);

        // Layer 2b & 2c: 90 water glints + night Bosphorus bridge rainbow lights
        self.col_verts.clear();
        let gr = pal.glint[0] as f32 / 255.0;
        let gg = pal.glint[1] as f32 / 255.0;
        let gb = pal.glint[2] as f32 / 255.0;
        let ga_scale = pal.ga as f32 * glow.min(1.5) * 0.8;
        for q in &ist.glints {
            let a = (0.5 + 0.5 * (t * 2.2 + q.p).sin() as f32) * ga_scale;
            Self::push_quad(
                &mut self.col_verts,
                q.x as f32,
                q.y as f32,
                q.l as f32,
                1.4,
                [gr, gg, gb, a],
            );
        }
        if pal.win > 0.5 && glow > 0.0 {
            let ba = 0.8 * glow.min(1.0);
            let bx = ist.geo.bx as f32;
            let by0 = ist.geo.by0 as f32;
            let span = (ist.geo.by1 - ist.geo.by0) as f32;
            for i in 0..30 {
                let hue = (i as f32 * 12.0 + t as f32 * 50.0) % 360.0;
                let col = hsla_to_rgba(hue, 0.90, 0.62, ba);
                let cy = by0 + span * (i as f32 / 29.0);
                Self::push_ellipse(&mut self.col_verts, bx, cy, 2.2, 2.2, 1.0, 0.0, col, 10);
            }
        }
        self.flush_col(gl, css_w, css_h);

        // Layer 3a: Ferries (wake, shadow, hull, cabin, funnels, foam)
        for f in &ist.ferries {
            Self::emit_ferry(&mut self.col_verts, f);
        }
        self.flush_col(gl, css_w, css_h);

        // Layer 3b: Ferry 14 cabin windows ('lighter' additive)
        if pal.win > 0.1 {
            let wa = 0.8 * pal.win as f32 * glow.min(1.0);
            if wa > 0.001 {
                for f in &ist.ferries {
                    Self::emit_ferry_windows(&mut self.col_verts, f, wa);
                }
                (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE, GL_ZERO, GL_ONE);
                self.flush_col(gl, css_w, css_h);
                (gl.BlendFuncSeparate)(
                    GL_SRC_ALPHA,
                    GL_ONE_MINUS_SRC_ALPHA,
                    GL_ONE,
                    GL_ONE_MINUS_SRC_ALPHA,
                );
            }
        }

        // Layer 4: Adaptive Land & Sea Icon Plazas
        const CAT_COATS: [u32; 4] = [0xd98a3d, 0x2c2c33, 0x9aa0aa, 0xf0ede6];
        for p in &ist.plazas {
            let k = (p.k * 1.25).min(1.0) as f32;
            if k <= 0.01 {
                continue;
            }
            let e = ist.params.pad as f64 * p.k;
            let x = (p.x - e) as f32;
            let y = (p.y - e * 0.6) as f32;
            let w = (p.w + 2.0 * e) as f32;
            let h = (p.h + 1.6 * e) as f32;
            let cx = x + w * 0.5;
            let cy = y + h * 0.5;

            if !p.sea {
                // 1. Tulip perimeter
                if ist.params.tulips && p.k > 0.35 {
                    Self::emit_tulips(&mut self.col_verts, x, y, w, h, t, k);
                    self.flush_col(gl, css_w, css_h);
                }
                // 2. Drop shadow (r=11)
                self.draw_pattern_rect(
                    gl,
                    css_w,
                    css_h,
                    0,
                    false,
                    1.0,
                    1.0,
                    [x + pal.sh[0] as f32, y + pal.sh[1] as f32, w, h],
                    11.0,
                    k,
                    [0.0, 0.0, 0.0, 0.3],
                    0.0,
                    [0.0; 4],
                );
                // 3. Outer Iznik tile frame (r=11)
                self.draw_pattern_rect(
                    gl,
                    css_w,
                    css_h,
                    self.tile_tex,
                    true,
                    14.0,
                    14.0,
                    [x, y, w, h],
                    11.0,
                    k,
                    [1.0; 4],
                    0.0,
                    [0.0; 4],
                );
                // 4. Inner Seljuk marble court (r=6, 1px stroke)
                if w > 18.0 && h > 18.0 {
                    self.draw_pattern_rect(
                        gl,
                        css_w,
                        css_h,
                        self.marble_tex,
                        true,
                        24.0,
                        24.0,
                        [x + 8.0, y + 8.0, w - 16.0, h - 16.0],
                        6.0,
                        k,
                        [1.0; 4],
                        1.0,
                        [80.0 / 255.0, 50.0 / 255.0, 30.0 / 255.0, 0.35],
                    );
                }
                // 5. Street cat
                if ist.params.cats && ist_hash(p.id as f64, 3.3) > 0.45 {
                    let q = ist_hash(p.id as f64, 9.1);
                    let coat_idx = ((q * 4.0).floor() as usize).min(3);
                    let coat = hex_f32(CAT_COATS[coat_idx], k);
                    let cat_x = if q < 0.5 { x + 22.0 } else { x + w - 22.0 };
                    let cat_y = y + h - 20.0;
                    Self::emit_cat(
                        &mut self.col_verts,
                        cat_x,
                        cat_y,
                        (q * 12.0) as f32,
                        coat,
                        t,
                        k,
                    );
                    self.flush_col(gl, css_w, css_h);
                }
            } else {
                // 1. Two expanding water rings
                for i in 0..2 {
                    let u = ((t * 0.3 + i as f64 * 0.5).rem_euclid(1.0)) as f32;
                    let ring_a = (1.0 - u) * 0.35 * k;
                    Self::push_ellipse_ring(
                        &mut self.col_verts,
                        cx,
                        cy,
                        w * 0.5 + 6.0 + u * 26.0,
                        h * 0.5 + 6.0 + u * 26.0,
                        1.5,
                        [1.0, 1.0, 1.0, ring_a],
                        44,
                    );
                }
                self.flush_col(gl, css_w, css_h);

                // 2. Pier shadow (r=6, (+3, +4))
                self.draw_pattern_rect(
                    gl,
                    css_w,
                    css_h,
                    0,
                    false,
                    1.0,
                    1.0,
                    [x + 3.0, y + 4.0, w, h],
                    6.0,
                    k,
                    [0.0, 10.0 / 255.0, 30.0 / 255.0, 0.35],
                    0.0,
                    [0.0; 4],
                );

                // 3. Wooden plank pier (r=6, 2px stroke #c9a06a)
                self.draw_pattern_rect(
                    gl,
                    css_w,
                    css_h,
                    self.plank_tex,
                    true,
                    14.0,
                    7.0,
                    [x, y, w, h],
                    6.0,
                    k,
                    [1.0; 4],
                    2.0,
                    hex_f32(0xc9a06a, 1.0),
                );

                // 4. Four corner bollards
                let b_dark = hex_f32(0x2b2b30, k);
                let b_spec = [1.0, 1.0, 1.0, 0.35 * k];
                for (bx, by) in [(x, y), (x + w, y), (x, y + h), (x + w, y + h)] {
                    Self::push_ellipse(&mut self.col_verts, bx, by, 3.6, 3.6, 1.0, 0.0, b_dark, 12);
                    Self::push_ellipse(
                        &mut self.col_verts,
                        bx - 1.0,
                        by - 1.0,
                        1.2,
                        1.2,
                        1.0,
                        0.0,
                        b_spec,
                        10,
                    );
                }

                // 5. Moored bobbing rowboat
                let boat_x = x + w + 15.0;
                let boat_y = y + h * 0.4 + (t * 1.3 + p.id as f64).sin() as f32 * 1.3;
                let boat_a: f32 = 1.57 + (t + p.id as f64).sin() as f32 * 0.05;
                let (bsin, bcos) = boat_a.sin_cos();
                let bxf = |lx: f32, ly: f32| -> [f32; 2] {
                    [boat_x + lx * bcos - ly * bsin, boat_y + lx * bsin + ly * bcos]
                };
                let bsh = bxf(2.0, 2.0);
                Self::push_ellipse(
                    &mut self.col_verts,
                    bsh[0],
                    bsh[1],
                    15.0,
                    5.0,
                    bcos,
                    bsin,
                    [0.0, 0.0, 0.0, 0.25 * k],
                    18,
                );
                Self::push_ellipse(
                    &mut self.col_verts,
                    boat_x,
                    boat_y,
                    15.0,
                    5.0,
                    bcos,
                    bsin,
                    hex_f32(0xc4553a, k),
                    18,
                );
                Self::push_rot_rect(
                    &mut self.col_verts,
                    boat_x,
                    boat_y,
                    bcos,
                    bsin,
                    -9.0,
                    -1.0,
                    18.0,
                    2.0,
                    hex_f32(0xf0e6d0, k),
                );
                self.flush_col(gl, css_w, css_h);
            }

            // 6. Additive plaza lantern glow ('lighter')
            if pal.win > 0.1 && glow > 0.0 {
                let ga = 0.34 * pal.win as f32 * glow.min(1.5) * k;
                if ga > 0.001 {
                    let r = w.max(h) * 0.8;
                    self.tex_verts.clear();
                    self.tex_verts.extend_from_slice(&[
                        cx - r, cy - r, 0.0, 0.0, ga,
                        cx + r, cy - r, 1.0, 0.0, ga,
                        cx + r, cy + r, 1.0, 1.0, ga,
                        cx - r, cy - r, 0.0, 0.0, ga,
                        cx + r, cy + r, 1.0, 1.0, ga,
                        cx - r, cy + r, 0.0, 1.0, ga,
                    ]);
                    (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE, GL_ZERO, GL_ONE);
                    self.flush_tex(gl, self.plaza_glow_tex, css_w, css_h);
                    (gl.BlendFuncSeparate)(
                        GL_SRC_ALPHA,
                        GL_ONE_MINUS_SRC_ALPHA,
                        GL_ONE,
                        GL_ONE_MINUS_SRC_ALPHA,
                    );
                }
            }
        }

        // Layer 5: Seagulls
        for q in &ist.gulls {
            Self::emit_gull(&mut self.col_verts, q, t);
        }
        self.flush_col(gl, css_w, css_h);

        // Layer 6: Time-of-day tint
        if pal.tint[3] > 0.001 {
            self.draw_post(
                gl,
                css_w,
                css_h,
                0,
                [
                    pal.tint[0] as f32,
                    pal.tint[1] as f32,
                    pal.tint[2] as f32,
                    pal.tint[3] as f32,
                ],
                [0.0, 0.0],
                [0.0, 1.0],
            );
        }

        // Layer 7: Sun / Moon radial glow ('screen' = GL_ONE, GL_ONE_MINUS_SRC_COLOR)
        let sun_a = 0.30 * glow.min(1.5);
        if sun_a > 0.001 {
            let sun_col = [
                pal.sun_col[0] as f32 / 255.0,
                pal.sun_col[1] as f32 / 255.0,
                pal.sun_col[2] as f32 / 255.0,
                sun_a,
            ];
            let sx = css_w * pal.sun[0] as f32;
            let sy = css_h * pal.sun[1] as f32;
            (gl.BlendFuncSeparate)(GL_ONE, GL_ONE_MINUS_SRC_COLOR, GL_ZERO, GL_ONE);
            self.draw_post(gl, css_w, css_h, 1, sun_col, [sx, sy], [0.0, css_w * 0.75]);
            (gl.BlendFuncSeparate)(
                GL_SRC_ALPHA,
                GL_ONE_MINUS_SRC_ALPHA,
                GL_ONE,
                GL_ONE_MINUS_SRC_ALPHA,
            );
        }

        // Layer 8: Edge vignette
        self.draw_post(
            gl,
            css_w,
            css_h,
            2,
            [0.0, 0.0, 0.0, 0.32],
            [css_w * 0.5, css_h * 0.5],
            [css_w.min(css_h) * 0.45, css_w.max(css_h) * 0.75],
        );

        // Restore GL state cleanly
        (gl.BlendFuncSeparate)(
            GL_SRC_ALPHA,
            GL_ONE_MINUS_SRC_ALPHA,
            GL_ONE,
            GL_ONE_MINUS_SRC_ALPHA,
        );
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, 0);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, 0);
        (gl.BindVertexArray)(0);
        (gl.UseProgram)(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn mulberry32_matches_js_prototype_exactly() {
        let expected: &[(i32, [f64; 10])] = &[
            (
                1453,
                [
                    0.9695693424437195,
                    0.9384238470811397,
                    0.5413096779957414,
                    0.8470855802297592,
                    0.18500308715738356,
                    0.5469123334623873,
                    0.24768057139590383,
                    0.8983349094633013,
                    0.7703755223192275,
                    0.6104473960585892,
                ],
            ),
            (
                99,
                [
                    0.2604658124037087,
                    0.8048227655235678,
                    0.5408715349622071,
                    0.6902434257790446,
                    0.001138708321377635,
                    0.8175430877599865,
                    0.011350846383720636,
                    0.10543650342151523,
                    0.3965785033069551,
                    0.6709415460936725,
                ],
            ),
            (
                7,
                [
                    0.011704753153026104,
                    0.06195825757458806,
                    0.97690763277933,
                    0.6990287057124078,
                    0.5214452685322613,
                    0.4055216880515218,
                    0.4662326325196773,
                    0.23992518591694534,
                    0.5533256039489061,
                    0.729822089895606,
                ],
            ),
            (
                31,
                [
                    0.6722561160568148,
                    0.8359460374340415,
                    0.7496604530606419,
                    0.04891713825054467,
                    0.41673933155834675,
                    0.3456277728546411,
                    0.07525175716727972,
                    0.9315404777880758,
                    0.8124719588086009,
                    0.6635727062821388,
                ],
            ),
        ];

        for &(seed, ref exp) in expected {
            let mut rng = Mulberry32::new(seed);
            for (i, &want) in exp.iter().enumerate() {
                let got = rng.next_f64();
                assert!(
                    (got - want).abs() < 1e-12,
                    "seed {seed} idx {i}: got {got}, expected {want}"
                );
            }
        }
    }

    #[test]
    fn ist_hash_matches_js_prototype_exactly() {
        let cases = [
            (1.0, 3.3, 0.5138315898439032),
            (1.0, 9.1, 0.4030036209951504),
            (42.0, 3.3, 0.3505696498505131),
            (42.0, 9.1, 0.2895261081721401),
        ];
        for (a, b, want) in cases {
            let got = ist_hash(a, b);
            assert!(
                (got - want).abs() < 1e-12,
                "ist_hash({a}, {b}): got {got}, expected {want}"
            );
        }
    }

    #[test]
    fn params_defaults_clamping_enum_parsing_and_auto_hour_mapping() {
        let def = IstanbulParams::default();
        assert_eq!(def.time, 3);
        assert!((def.glow - 1.0).abs() < 1e-6);
        assert_eq!(def.ferries, 3);
        assert!((def.flow - 1.0).abs() < 1e-6);
        assert_eq!(def.gulls, 1);
        assert!((def.pad - 14.0).abs() < 1e-6);
        assert!(def.tulips);
        assert!(def.cats);
        assert!(def.landmarks);
        assert!(def.adaptive);

        // Clamping via AppState::handle_json
        let mut app = AppState::new(1280, 720, 1.0);
        app.set_theme("istanbul");
        app.handle_json(
            r#"{"t":"settings","v":{"glow":-1.0,"ferries":-2,"flow":-0.5,"pad":1.0}}"#,
        );
        assert!((app.istanbul.params.glow - 0.0).abs() < 1e-6);
        assert_eq!(app.istanbul.params.ferries, 0);
        assert!((app.istanbul.params.flow - 0.0).abs() < 1e-6);
        assert!((app.istanbul.params.pad - 6.0).abs() < 1e-6);

        app.handle_json(
            r#"{"t":"settings","v":{"glow":9.0,"ferries":99,"flow":10.0,"pad":120.0}}"#,
        );
        assert!((app.istanbul.params.glow - 1.5).abs() < 1e-6);
        assert_eq!(app.istanbul.params.ferries, 6);
        assert!((app.istanbul.params.flow - 3.0).abs() < 1e-6);
        assert!((app.istanbul.params.pad - 50.0).abs() < 1e-6);

        // String & numeric enum parsing
        for (val, idx) in [
            (serde_json::json!(0), 0u8),
            (serde_json::json!("auto"), 0),
            (serde_json::json!(1), 1),
            (serde_json::json!("morning"), 1),
            (serde_json::json!("DAWN"), 1),
            (serde_json::json!(2), 2),
            (serde_json::json!("noon"), 2),
            (serde_json::json!(3), 3),
            (serde_json::json!("sunset"), 3),
            (serde_json::json!(4), 4),
            (serde_json::json!("Night"), 4),
        ] {
            let mut p = IstanbulParams::default();
            p.apply_kv("time", &val);
            assert_eq!(p.time, idx, "time value {val}");
        }

        for (val, idx, count) in [
            (serde_json::json!(0), 0u8, 0usize),
            (serde_json::json!("none"), 0, 0),
            (serde_json::json!(1), 1, 5),
            (serde_json::json!("few"), 1, 5),
            (serde_json::json!(2), 2, 12),
            (serde_json::json!("MANY"), 2, 12),
        ] {
            let mut p = IstanbulParams::default();
            p.apply_kv("gulls", &val);
            assert_eq!(p.gulls, idx);
            assert_eq!(p.gull_count(), count);
        }

        for key in ["tulips", "cats", "landmarks", "adaptive"] {
            for (v, expected) in [
                (serde_json::json!(0), false),
                (serde_json::json!(false), false),
                (serde_json::json!("hide"), false),
                (serde_json::json!("off"), false),
                (serde_json::json!(1), true),
                (serde_json::json!(true), true),
                (serde_json::json!("show"), true),
                (serde_json::json!("ON"), true),
            ] {
                let mut p = IstanbulParams::default();
                p.apply_kv(key, &v);
                let got = match key {
                    "tulips" => p.tulips,
                    "cats" => p.cats,
                    "landmarks" => p.landmarks,
                    "adaptive" => p.adaptive,
                    _ => unreachable!(),
                };
                assert_eq!(got, expected, "key {key} val {v}");
            }
        }

        // Auto hour mapping across 24 hours
        let auto = IstanbulParams::from_json_map(
            serde_json::json!({"time": "auto"}).as_object().unwrap(),
        );
        for &(hr, expected) in &[
            (0.0f32, 4usize),
            (4.9, 4),
            (5.0, 1),
            (7.5, 1),
            (9.9, 1),
            (10.0, 2),
            (13.0, 2),
            (16.9, 2),
            (17.0, 3),
            (18.5, 3),
            (19.9, 3),
            (20.0, 4),
            (23.5, 4),
        ] {
            assert_eq!(auto.resolved_time_index(hr), expected, "hour {hr}");
        }
    }

    #[test]
    fn cache_invalidation_resize_params_icon_move_and_adaptive() {
        let mut st = IstanbulState::new();
        st.resize(1280.0, 720.0);
        st.ensure_static_fresh();
        let srev0 = st.static_rev;
        let wrev0 = st.wave_rev;
        assert!(srev0 > 0 && wrev0 > 0);
        assert_eq!(st.water_mask_r8.len(), (1280 * 720) as usize);

        // Resize <= 0.5px on both axes must NOT bump static_rev or wave_rev
        st.resize(1280.4, 719.6);
        st.ensure_static_fresh();
        assert_eq!(st.static_rev, srev0);
        assert_eq!(st.wave_rev, wrev0);

        // Resize > 0.5px MUST bump static_rev (and keep wave_rev unchanged for same time)
        st.resize(1281.0, 720.0);
        st.ensure_static_fresh();
        let srev1 = st.static_rev;
        assert!(srev1 > srev0);
        assert_eq!(st.wave_rev, wrev0);

        // Toggling landmarks MUST bump static_rev
        let mut p = st.params;
        p.landmarks = false;
        st.sync_params(p);
        st.ensure_static_fresh();
        let srev2 = st.static_rev;
        assert!(srev2 > srev1);
        assert_eq!(st.wave_rev, wrev0);

        // Changing time 3 -> 4 MUST bump BOTH static_rev and wave_rev
        p.time = 4;
        st.sync_params(p);
        st.ensure_static_fresh();
        let srev3 = st.static_rev;
        let wrev1 = st.wave_rev;
        assert!(srev3 > srev2);
        assert!(wrev1 > wrev0);

        // Auto time (time = 0) + env_hour change across buckets bumps both
        p.time = 0;
        st.set_env_hour(12.0);
        st.sync_params(p);
        st.ensure_static_fresh();
        let srev4 = st.static_rev;
        let wrev2 = st.wave_rev;
        assert!(srev4 > srev3);
        assert!(wrev2 > wrev1);

        st.set_env_hour(22.0);
        st.ensure_static_fresh();
        assert!(st.static_rev > srev4);
        assert!(st.wave_rev > wrev2);

        // Same-count icon move & land-to-sea flip
        st.set_icons(
            &[Rect {
                x: 80.0,
                y: 60.0,
                w: 76.0,
                h: 90.0,
            }],
            true,
        );
        assert_eq!(st.plazas.len(), 1);
        assert!(!st.plazas[0].sea);

        st.set_icons(
            &[Rect {
                x: 600.0,
                y: 390.0,
                w: 76.0,
                h: 90.0,
            }],
            true,
        );
        assert!(
            st.plazas
                .iter()
                .any(|pl| pl.used && (pl.tx - 600.0).abs() < 0.1 && pl.sea),
            "moving 1 icon into Bosphorus should update plaza and flip sea=true"
        );

        // Adaptive toggle off -> clears active plazas, then on -> restores from cached raw icons
        p.adaptive = false;
        st.sync_params(p);
        assert!(st.plazas.iter().all(|pl| !pl.used));
        for _ in 0..40 {
            st.step(0.1);
        }
        assert!(st.plazas.is_empty());

        p.adaptive = true;
        st.sync_params(p);
        assert!(st
            .plazas
            .iter()
            .any(|pl| pl.used && (pl.tx - 600.0).abs() < 0.1 && pl.sea));
    }

    #[test]
    fn glow_sprite_outer_border_is_strictly_transparent() {
        let size = 128usize;
        let rgba = build_radial_glow_rgba(size, [255, 200, 110]);
        assert_eq!(rgba.len(), size * size * 4);
        let center_a = rgba[((size / 2) * size + (size / 2)) * 4 + 3];
        assert!(center_a > 200, "center alpha should be bright, got {center_a}");
        for y in 0..size {
            for x in 0..size {
                if x == 0 || y == 0 || x + 1 == size || y + 1 == size {
                    let a = rgba[(y * size + x) * 4 + 3];
                    assert_eq!(a, 0, "outer 1px border at ({x}, {y}) had non-zero alpha {a}");
                }
            }
        }
    }
}
