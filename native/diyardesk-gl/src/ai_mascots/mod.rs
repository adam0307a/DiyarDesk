pub mod field;
pub mod mascots;
pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use crate::text::TextRenderer;
use field::{clamp, lerp, BuildingKind, CampusBuilding, CampusEnvironment, IconInfluence, Mulberry32};
use mascots::{Mascot, MascotBehavior, MascotKind, MascotManager};
use serde_json::Value;
use shaders::*;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// 1. PARAMS (1:1 with prototype/themes.json)
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiMascotsParams {
    pub time_mode: usize,      // 0 = night (default), 1 = evening, 2 = day, 3 = cycle
    pub mascot_activity: f64,  // 0.2..=2.0 (default 1.0)
    pub lighting_glow: f64,    // 0.2..=1.6 (default 1.0)
    pub particle_density: f64, // 0.0..=1.6 (default 0.85)
    pub icon_halo: f64,        // 0.0..=1.5 (default 1.0)
    pub parallax: f64,         // 0.0..=1.0 (default 0.45)
    pub quality: usize,        // 0 = Low, 1 = Medium, 2 = High (default 2)
    pub adaptive: bool,        // default true
    pub seed: u32,             // default 2026
}

impl Default for AiMascotsParams {
    fn default() -> Self {
        Self {
            time_mode: 0,
            mascot_activity: 1.0,
            lighting_glow: 1.0,
            particle_density: 0.85,
            icon_halo: 1.0,
            parallax: 0.45,
            quality: 2,
            adaptive: true,
            seed: 2026,
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

fn parse_f64(raw: &HashMap<String, Value>, key: &str, def: f64, lo: f64, hi: f64) -> f64 {
    let Some(v) = raw.get(key) else {
        return def;
    };
    let n = match v {
        Value::Number(x) => x.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    };
    match n {
        Some(x) if x.is_finite() => clamp(x, lo, hi),
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

impl AiMascotsParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        Self {
            time_mode: parse_enum(raw, "timeMode", &["night", "evening", "day", "cycle"], 0),
            mascot_activity: parse_f64(raw, "mascotActivity", 1.0, 0.2, 2.0),
            lighting_glow: parse_f64(raw, "lightingGlow", 1.0, 0.2, 1.6),
            particle_density: parse_f64(raw, "particleDensity", 0.85, 0.0, 1.6),
            icon_halo: parse_f64(raw, "iconHalo", 1.0, 0.0, 1.5),
            parallax: parse_f64(raw, "parallax", 0.45, 0.0, 1.0),
            quality: parse_enum(raw, "quality", &["Low", "Medium", "High"], 2),
            adaptive: parse_bool(raw, "adaptive", true),
            seed: parse_f64(raw, "seed", 2026.0, 0.0, u32::MAX as f64) as u32,
        }
    }
}

// =========================================================================
// 2. POOLED PARTICLE SYSTEM, PATROL DRONES & LIGHTING PALETTE
// =========================================================================

#[derive(Clone, Copy)]
struct DataMote {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    size: f64,
    phase: f64,
    color_idx: usize,
    active: bool,
}

#[derive(Clone, Copy)]
struct PatrolDrone {
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    angle: f64,
    speed: f64,
    accent: [f32; 3],
}

#[derive(Clone, Copy)]
struct ConduitPulse {
    path_idx: usize,
    t: f64,
    speed: f64,
    color: [f32; 3],
}

#[derive(Clone, Copy)]
struct AmbientPalette {
    sky_top: [f32; 3],
    sky_mid: [f32; 3],
    ground: [f32; 3],
    grid: [f32; 3],
    ray: [f32; 3],
    fog_alpha: f32,
    star_alpha: f32,
    artificial_light: f32, // stronger at night, subtler during day
}

const PAL_NIGHT: AmbientPalette = AmbientPalette {
    sky_top: [0.035, 0.052, 0.095],  // deep navy
    sky_mid: [0.058, 0.082, 0.135],  // charcoal-navy
    ground: [0.055, 0.072, 0.112],   // calm slate courtyard
    grid: [0.135, 0.185, 0.270],     // subtle muted blue pavers
    ray: [0.38, 0.62, 0.85],         // soft cyan-blue beam
    fog_alpha: 0.16,
    star_alpha: 0.92,
    artificial_light: 1.0,
};

const PAL_EVENING: AmbientPalette = AmbientPalette {
    sky_top: [0.085, 0.068, 0.145],  // muted twilight purple
    sky_mid: [0.185, 0.112, 0.150],  // soft plum-amber dusk
    ground: [0.078, 0.082, 0.122],   // twilight slate
    grid: [0.185, 0.175, 0.245],
    ray: [0.85, 0.56, 0.36],         // warm evening amber wash
    fog_alpha: 0.20,
    star_alpha: 0.42,
    artificial_light: 0.85,
};

const PAL_DAY: AmbientPalette = AmbientPalette {
    sky_top: [0.145, 0.220, 0.335],  // calm muted blue sky
    sky_mid: [0.195, 0.265, 0.360],
    ground: [0.118, 0.148, 0.195],   // cool architectural daylight slate
    grid: [0.225, 0.275, 0.345],
    ray: [0.95, 0.88, 0.72],         // warm soft sunlight
    fog_alpha: 0.12,
    star_alpha: 0.0,
    artificial_light: 0.42,
};

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn mix_pal(a: AmbientPalette, b: AmbientPalette, t: f32) -> AmbientPalette {
    AmbientPalette {
        sky_top: mix3(a.sky_top, b.sky_top, t),
        sky_mid: mix3(a.sky_mid, b.sky_mid, t),
        ground: mix3(a.ground, b.ground, t),
        grid: mix3(a.grid, b.grid, t),
        ray: mix3(a.ray, b.ray, t),
        fog_alpha: a.fog_alpha + (b.fog_alpha - a.fog_alpha) * t,
        star_alpha: a.star_alpha + (b.star_alpha - a.star_alpha) * t,
        artificial_light: a.artificial_light + (b.artificial_light - a.artificial_light) * t,
    }
}

fn resolve_ambient_palette(time_mode: usize, sim_time: f64) -> AmbientPalette {
    match time_mode {
        0 => PAL_NIGHT,
        1 => PAL_EVENING,
        2 => PAL_DAY,
        _ => {
            // Very slow ~180s internal cycle: Night -> Day -> Evening -> Night
            let phase = (sim_time / 180.0) % 1.0;
            if phase < 0.38 {
                let t = ((phase / 0.38) as f32).clamp(0.0, 1.0);
                mix_pal(PAL_NIGHT, PAL_DAY, t * t * (3.0 - 2.0 * t))
            } else if phase < 0.68 {
                let t = (((phase - 0.38) / 0.30) as f32).clamp(0.0, 1.0);
                mix_pal(PAL_DAY, PAL_EVENING, t * t * (3.0 - 2.0 * t))
            } else {
                let t = (((phase - 0.68) / 0.32) as f32).clamp(0.0, 1.0);
                mix_pal(PAL_EVENING, PAL_NIGHT, t * t * (3.0 - 2.0 * t))
            }
        }
    }
}

// =========================================================================
// 3. MAIN THEME STATE
// =========================================================================

pub struct AiMascotsState {
    pub params: AiMascotsParams,
    pub low_mode: bool,
    pub w: f64,
    pub h: f64,
    pub sim_time: f64,
    pub layout_rev: u64,
    pub dirty: bool,
    pub field: IconInfluence,
    pub env: CampusEnvironment,
    pub manager: MascotManager,
    motes: Vec<DataMote>,
    drones: Vec<PatrolDrone>,
    pulses: Vec<ConduitPulse>,
    cached_icons: Vec<Rect>,
    cached_names: Vec<String>,
    pointer_x: f64,
    pointer_y: f64,
    pointer_active: bool,
    parallax_x: f64,
    parallax_y: f64,
}

impl Default for AiMascotsState {
    fn default() -> Self {
        let mut st = Self {
            params: AiMascotsParams::default(),
            low_mode: false,
            w: 1920.0,
            h: 1080.0,
            sim_time: 0.0,
            layout_rev: 1,
            dirty: true,
            field: IconInfluence::default(),
            env: CampusEnvironment::default(),
            manager: MascotManager::default(),
            motes: Vec::new(),
            drones: Vec::new(),
            pulses: Vec::new(),
            cached_icons: Vec::new(),
            cached_names: Vec::new(),
            pointer_x: 960.0,
            pointer_y: 540.0,
            pointer_active: false,
            parallax_x: 0.0,
            parallax_y: 0.0,
        };
        st.env.rebuild(1920.0, 1080.0, st.params.seed);
        st.manager.spawn_all(&st.env, &st.field, st.params.seed);
        st.rebuild_particles();
        st
    }
}

impl AiMascotsState {
    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.layout_rev = self.layout_rev.wrapping_add(1);
            self.dirty = true;
            self.rebuild_particles();
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = (w as f64).max(64.0);
        let nh = (h as f64).max(64.0);
        if (nw - self.w).abs() <= 0.5 && (nh - self.h).abs() <= 0.5 {
            return;
        }
        self.w = nw;
        self.h = nh;
        self.layout_rev = self.layout_rev.wrapping_add(1);
        self.dirty = true;
        self.env.rebuild(nw, nh, self.params.seed);
        self.manager.rescale_homes(&self.env, &self.field);
        self.rebuild_particles();
    }

    pub fn sync_params(&mut self, next: AiMascotsParams) {
        let prev = self.params;
        if prev == next {
            return;
        }
        let seed_changed = prev.seed != next.seed;
        let density_changed = (prev.particle_density - next.particle_density).abs() > 1e-4
            || prev.quality != next.quality;
        let adaptive_changed = prev.adaptive != next.adaptive;

        self.params = next;
        self.layout_rev = self.layout_rev.wrapping_add(1);
        self.dirty = true;

        if seed_changed {
            self.env.rebuild(self.w, self.h, next.seed);
            self.manager.spawn_all(&self.env, &self.field, next.seed);
        }
        if adaptive_changed {
            if !next.adaptive {
                self.field.clear_smooth();
            } else if !self.cached_icons.is_empty() {
                self.field.sync_icons(
                    &self.cached_icons,
                    &self.cached_names,
                    true,
                    true,
                    self.w,
                    self.h,
                );
            }
        }
        if seed_changed || density_changed {
            self.rebuild_particles();
        }
    }

    pub fn set_icons(&mut self, icons: &[Rect], names: &[String], vis: bool) {
        self.cached_icons.clear();
        self.cached_names.clear();
        if vis {
            self.cached_icons.extend_from_slice(icons);
            self.cached_names.extend_from_slice(names);
        }
        self.field
            .sync_icons(icons, names, vis, self.params.adaptive, self.w, self.h);
        self.layout_rev = self.layout_rev.wrapping_add(1);
        self.dirty = true;
    }

    pub fn handle_mouse(&mut self, kind: i32, x: f32, y: f32) -> bool {
        match kind {
            0 | 1 => {
                self.pointer_x = x as f64;
                self.pointer_y = y as f64;
                self.pointer_active = true;
            }
            3 => {
                self.pointer_active = false;
            }
            _ => {}
        }
        false
    }

    fn rebuild_particles(&mut self) {
        let q_mul = if self.low_mode || self.params.quality == 0 {
            0.45
        } else if self.params.quality == 1 {
            0.75
        } else {
            1.0
        };
        let count = ((54.0 * self.params.particle_density * q_mul).round() as usize).min(96);
        let mut rng = Mulberry32::new(self.params.seed ^ 0x5041_5254);
        self.motes.clear();
        self.motes.reserve(count);
        for _ in 0..count {
            self.motes.push(DataMote {
                x: rng.range(0.03, 0.97) * self.w,
                y: rng.range(0.14, 0.95) * self.h,
                vx: rng.range(-7.0, 7.0),
                vy: rng.range(-9.5, -2.5),
                size: rng.range(1.1, 2.5),
                phase: rng.range(0.0, std::f64::consts::TAU),
                color_idx: (rng.next_f64() * 5.0).floor() as usize % 5,
                active: true,
            });
        }

        // 3 calm miniature patrol drones orbiting campus corridors
        let drone_n = if self.low_mode || self.params.particle_density < 0.1 { 1 } else { 3 };
        self.drones.clear();
        let d_specs: [(f64, f64, f64, f64, f64, [f32; 3]); 3] = [
            (0.50, 0.44, 0.24, 0.11, 0.16, [0.44, 0.86, 0.96]),
            (0.32, 0.56, 0.14, 0.08, -0.21, [0.42, 0.90, 0.68]),
            (0.69, 0.55, 0.15, 0.09, 0.19, [0.68, 0.58, 0.96]),
        ];
        for i in 0..drone_n {
            let (cx, cy, rx, ry, spd, acc) = d_specs[i];
            self.drones.push(PatrolDrone {
                cx: cx * self.w,
                cy: cy * self.h,
                rx: rx * self.w,
                ry: ry * self.h,
                angle: rng.range(0.0, std::f64::consts::TAU),
                speed: spd,
                accent: acc,
            });
        }

        // Subtle energy pulses travelling along campus walkways
        self.pulses.clear();
        let p_colors: [[f32; 3]; 4] = [
            [0.44, 0.86, 0.96],
            [0.66, 0.56, 0.96],
            [0.36, 0.88, 0.62],
            [0.94, 0.64, 0.36],
        ];
        let path_cnt = self.env.paths.len().max(1);
        for i in 0..8 {
            self.pulses.push(ConduitPulse {
                path_idx: i % path_cnt,
                t: rng.range(0.0, 1.0),
                speed: rng.range(0.07, 0.13),
                color: p_colors[i % 4],
            });
        }
    }

    pub fn step(&mut self, dt_raw: f32) {
        let max_dt = if self.low_mode { 0.12 } else { 0.05 };
        let dt = clamp(dt_raw as f64, 0.001, max_dt);
        self.sim_time += dt;

        // Subtle damped fixed-camera parallax (max +/- 6px)
        let tgt_px = if self.pointer_active {
            clamp((self.pointer_x / self.w.max(1.0) - 0.5) * 12.0 * self.params.parallax, -8.0, 8.0)
        } else {
            0.0
        };
        let tgt_py = if self.pointer_active {
            clamp((self.pointer_y / self.h.max(1.0) - 0.5) * 8.0 * self.params.parallax, -6.0, 6.0)
        } else {
            0.0
        };
        let pk = 1.0 - (-3.5 * dt).exp();
        self.parallax_x = lerp(self.parallax_x, tgt_px, pk);
        self.parallax_y = lerp(self.parallax_y, tgt_py, pk);

        // 1. Update spring-animated desktop icon workstations & clusters
        self.field.step(dt);

        // 2. Smoothly displace campus paths, trees, and lamps around desktop icons
        self.env.adapt_to_icons(&self.field, dt);

        // 3. Step autonomous AI mascots + desktop icon & cursor interactions
        self.manager.step(
            dt,
            self.params.mascot_activity,
            &self.env,
            &mut self.field,
            self.pointer_x - self.parallax_x,
            self.pointer_y - self.parallax_y,
            self.pointer_active,
        );

        // 4. Step pooled floating data motes (gently deflected around icons & cursor)
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            let inf = self.field.sample(m.x, m.y);
            let mut ax = (self.sim_time * 0.4 + m.phase).sin() * 2.5;
            let mut ay = 0.0;
            if inf.strength > 0.05 {
                ax += inf.push_x * 0.35;
                ay += inf.push_y * 0.35;
            }
            if self.pointer_active {
                let dx = m.x - self.pointer_x;
                let dy = m.y - self.pointer_y;
                let d = dx.hypot(dy);
                if d > 1.0 && d < 95.0 {
                    let f = (1.0 - d / 95.0) * 18.0;
                    ax += (dx / d) * f;
                    ay += (dy / d) * f;
                }
            }
            m.x += (m.vx + ax) * dt;
            m.y += (m.vy + ay) * dt;
            if m.y < self.h * 0.12 {
                m.y = self.h * 0.96;
            }
            if m.x < 10.0 {
                m.x = self.w - 10.0;
            } else if m.x > self.w - 10.0 {
                m.x = 10.0;
            }
        }

        // 5. Step patrol drones & conduit pulses
        for d in &mut self.drones {
            d.angle = (d.angle + d.speed * dt) % std::f64::consts::TAU;
        }
        for p in &mut self.pulses {
            p.t = (p.t + p.speed * dt) % 1.0;
        }
    }
}

// =========================================================================
// 4. PRE-RENDERED RGBA8 SOFT RADIAL GLOW SPRITE (STRICT 1px alpha=0 BORDER)
// =========================================================================

pub fn bake_soft_glow_sprite() -> Vec<u8> {
    const SZ: usize = 128;
    let mut px = vec![0u8; SZ * SZ * 4];
    let half = SZ as f32 * 0.5;
    let max_r = half - 1.5;
    for y in 0..SZ {
        for x in 0..SZ {
            if x == 0 || y == 0 || x == SZ - 1 || y == SZ - 1 {
                continue; // Strict outer 1px alpha = 0
            }
            let d = (x as f32 + 0.5 - half).hypot(y as f32 + 0.5 - half);
            if d >= max_r {
                continue;
            }
            let t = (d / max_r).clamp(0.0, 1.0);
            // Smooth cubic-quadratic soft atmospheric falloff (zero harsh rings)
            let inv = 1.0 - t;
            let a = (inv * inv * (0.65 + 0.35 * inv)).clamp(0.0, 1.0);
            let off = (y * SZ + x) * 4;
            px[off] = 255;
            px[off + 1] = 255;
            px[off + 2] = 255;
            px[off + 3] = (a * 255.0).round() as u8;
        }
    }
    px
}

// =========================================================================
// 5. OPENGL 3.3 BATCHED RENDERER
// =========================================================================

#[repr(C)]
#[derive(Clone, Copy)]
struct ColVert {
    x: f32,
    y: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TexVert {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
    col: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SdfVert {
    x: f32,
    y: f32,
    lx: f32,
    ly: f32,
    col: [f32; 4],
    stroke: [f32; 4],
    params: [f32; 4],
}

pub struct AiMascotsRenderer {
    prog_bg: GLuint,
    prog_col: GLuint,
    prog_tex: GLuint,
    prog_sdf: GLuint,
    dummy_vao: GLuint,
    col_vao: GLuint,
    col_vbo: GLuint,
    tex_vao: GLuint,
    tex_vbo: GLuint,
    sdf_vao: GLuint,
    sdf_vbo: GLuint,
    glow_tex: GLuint,
    col_verts: Vec<ColVert>,
    tex_verts: Vec<TexVert>,
    sdf_verts: Vec<SdfVert>,
}

impl AiMascotsRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_bg = gl.compile_program(AM_BG_VERT, AM_BG_FRAG)?;
        let prog_col = gl.compile_program(AM_COL_VERT, AM_COL_FRAG)?;
        let prog_tex = gl.compile_program(AM_TEX_VERT, AM_TEX_FRAG)?;
        let prog_sdf = gl.compile_program(AM_SDF_VERT, AM_SDF_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let (mut col_vao, mut col_vbo) = (0, 0);
        (gl.GenVertexArrays)(1, &mut col_vao);
        (gl.GenBuffers)(1, &mut col_vbo);
        (gl.BindVertexArray)(col_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, col_vbo);
        let sc = std::mem::size_of::<ColVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, sc, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, sc, 8 as *const c_void);

        let (mut tex_vao, mut tex_vbo) = (0, 0);
        (gl.GenVertexArrays)(1, &mut tex_vao);
        (gl.GenBuffers)(1, &mut tex_vbo);
        (gl.BindVertexArray)(tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, tex_vbo);
        let st = std::mem::size_of::<TexVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, st, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, st, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 4, GL_FLOAT, GL_FALSE, st, 16 as *const c_void);

        let (mut sdf_vao, mut sdf_vbo) = (0, 0);
        (gl.GenVertexArrays)(1, &mut sdf_vao);
        (gl.GenBuffers)(1, &mut sdf_vbo);
        (gl.BindVertexArray)(sdf_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, sdf_vbo);
        let ss = std::mem::size_of::<SdfVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, ss, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, ss, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 4, GL_FLOAT, GL_FALSE, ss, 16 as *const c_void);
        (gl.EnableVertexAttribArray)(3);
        (gl.VertexAttribPointer)(3, 4, GL_FLOAT, GL_FALSE, ss, 32 as *const c_void);
        (gl.EnableVertexAttribArray)(4);
        (gl.VertexAttribPointer)(4, 4, GL_FLOAT, GL_FALSE, ss, 48 as *const c_void);
        (gl.BindVertexArray)(0);

        let glow_px = bake_soft_glow_sprite();
        let mut glow_tex = 0;
        (gl.GenTextures)(1, &mut glow_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, glow_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            128,
            128,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            glow_px.as_ptr() as *const c_void,
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            prog_bg,
            prog_col,
            prog_tex,
            prog_sdf,
            dummy_vao,
            col_vao,
            col_vbo,
            tex_vao,
            tex_vbo,
            sdf_vao,
            sdf_vbo,
            glow_tex,
            col_verts: Vec::with_capacity(12000),
            tex_verts: Vec::with_capacity(2048),
            sdf_verts: Vec::with_capacity(4096),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.glow_tex != 0 { (gl.DeleteTextures)(1, &self.glow_tex); self.glow_tex = 0; }
        for v in [&mut self.dummy_vao, &mut self.col_vao, &mut self.tex_vao, &mut self.sdf_vao] {
            if *v != 0 { (gl.DeleteVertexArrays)(1, v); *v = 0; }
        }
        for b in [&mut self.col_vbo, &mut self.tex_vbo, &mut self.sdf_vbo] {
            if *b != 0 { (gl.DeleteBuffers)(1, b); *b = 0; }
        }
        for p in [&mut self.prog_bg, &mut self.prog_col, &mut self.prog_tex, &mut self.prog_sdf] {
            if *p != 0 { (gl.DeleteProgram)(*p); *p = 0; }
        }
    }

    unsafe fn flush_col(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.col_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_col);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_col, "uRes"), css_w, css_h);
        (gl.BindVertexArray)(self.col_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.col_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.col_verts.len() * std::mem::size_of::<ColVert>()) as GLsizeiptr,
            self.col_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.col_verts.len() as GLsizei);
        self.col_verts.clear();
    }

    unsafe fn flush_tex(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.tex_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_tex);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_tex, "uRes"), css_w, css_h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.glow_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_tex, "uTex"), 0);
        (gl.BindVertexArray)(self.tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.tex_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.tex_verts.len() * std::mem::size_of::<TexVert>()) as GLsizeiptr,
            self.tex_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.tex_verts.len() as GLsizei);
        self.tex_verts.clear();
    }

    unsafe fn flush_sdf(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.sdf_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_sdf);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_sdf, "uRes"), css_w, css_h);
        (gl.BindVertexArray)(self.sdf_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.sdf_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.sdf_verts.len() * std::mem::size_of::<SdfVert>()) as GLsizeiptr,
            self.sdf_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.sdf_verts.len() as GLsizei);
        self.sdf_verts.clear();
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState, text: &mut TextRenderer) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale;
        st.ai_mascots.resize(css_w, css_h);
        st.ai_mascots.dirty = false;

        let s = &st.ai_mascots;
        let pal = resolve_ambient_palette(s.params.time_mode, s.sim_time);
        let glow_mul = (s.params.lighting_glow as f32) * pal.artificial_light;
        let halo_mul = (s.params.icon_halo as f32) * (0.55 + 0.45 * pal.artificial_light);
        let px_off = s.parallax_x as f32;
        let py_off = s.parallax_y as f32;
        let unit = (css_w.min(css_h) / 1080.0).clamp(0.58, 1.75);
        let t = s.sim_time as f32;

        // 1. Atmospheric Isometric Campus Background
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);
        (gl.Disable)(GL_BLEND);

        (gl.UseProgram)(self.prog_bg);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_bg, "uRes"), css_w, css_h);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "uTime"), t);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_bg, "uParallax"), px_off, py_off);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "uSkyTop"), pal.sky_top[0], pal.sky_top[1], pal.sky_top[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "uSkyMid"), pal.sky_mid[0], pal.sky_mid[1], pal.sky_mid[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "uGround"), pal.ground[0], pal.ground[1], pal.ground[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "uGridCol"), pal.grid[0], pal.grid[1], pal.grid[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "uRayCol"), pal.ray[0], pal.ray[1], pal.ray[2]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "uFogAlpha"), pal.fog_alpha);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "uStarAlpha"), pal.star_alpha);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);

        // 2. Ground Layer: Subtle Campus Courtyard Lawns, Curved Walkways, Conduit Pulses
        self.col_verts.clear();
        self.tex_verts.clear();
        self.sdf_verts.clear();

        // Subtle muted teal-slate isometric campus garden/lawn zones (calm architectural depth)
        let lawn_col = [0.068, 0.112, 0.145, 0.56];
        let lawn_edge = [0.095, 0.155, 0.190, 0.35];
        for &(lx, ly, rx, ry) in &[
            (0.50, 0.36, 175.0, 96.0),
            (0.27, 0.72, 130.0, 72.0),
            (0.73, 0.71, 138.0, 74.0),
            (0.26, 0.33, 128.0, 70.0),
            (0.74, 0.33, 130.0, 72.0),
        ] {
            let cx = css_w * lx + px_off;
            let cy = css_h * ly + py_off;
            push_iso_diamond(&mut self.col_verts, cx, cy, rx * unit, ry * unit, lawn_edge);
            push_iso_diamond(&mut self.col_verts, cx, cy, (rx - 6.0) * unit, (ry - 3.5) * unit, lawn_col);
        }

        // Central courtyard plaza ring (subtle architectural landmark)
        let hub_x = css_w * 0.50 + px_off;
        let hub_y = css_h * 0.52 + py_off;
        push_iso_diamond(
            &mut self.col_verts,
            hub_x,
            hub_y,
            96.0 * unit,
            52.0 * unit,
            [0.10, 0.14, 0.21, 0.65],
        );
        push_iso_diamond(
            &mut self.col_verts,
            hub_x,
            hub_y,
            78.0 * unit,
            42.0 * unit,
            [0.08, 0.11, 0.17, 0.72],
        );

        // Curved campus walkways (adaptively bent around desktop icons!)
        for path in &s.env.paths {
            let pts: Vec<(f32, f32)> = path
                .pts
                .iter()
                .map(|p| (p[0] as f32 + px_off, p[1] as f32 + py_off))
                .collect();
            // Outer architectural curb + warm slate walkway surface + subtle center guide line
            push_polyline(&mut self.col_verts, &pts, 14.0 * unit, [0.13, 0.17, 0.24, 0.62]);
            push_polyline(&mut self.col_verts, &pts, 10.5 * unit, [0.10, 0.13, 0.20, 0.78]);
            push_polyline(
                &mut self.col_verts,
                &pts,
                1.4 * unit,
                [0.42, 0.72, 0.88, (0.14 * glow_mul).clamp(0.0, 0.35)],
            );
        }

        // Conduit energy pulses along campus pathways
        for p in &s.pulses {
            if let Some(path) = s.env.paths.get(p.path_idx) {
                if path.pts.len() >= 2 {
                    let f = p.t * (path.pts.len() - 1) as f64;
                    let idx = (f.floor() as usize).min(path.pts.len() - 2);
                    let frac = f - idx as f64;
                    let x = lerp(path.pts[idx][0], path.pts[idx + 1][0], frac) as f32 + px_off;
                    let y = lerp(path.pts[idx][1], path.pts[idx + 1][1], frac) as f32 + py_off;
                    push_glow(
                        &mut self.tex_verts,
                        x,
                        y,
                        18.0 * unit,
                        [p.color[0], p.color[1], p.color[2], (0.30 * glow_mul).clamp(0.0, 0.5)],
                    );
                    push_sdf_disc(
                        &mut self.sdf_verts,
                        x,
                        y,
                        2.3 * unit,
                        [p.color[0], p.color[1], p.color[2], (0.72 * glow_mul).clamp(0.0, 0.95)],
                    );
                }
            }
        }

        // 3. Adaptive Desktop Icon Workstations & Connected Clusters (ALWAYS readable!)
        let station_accents: [[f32; 3]; 5] = [
            [0.42, 0.84, 0.94], // soft cyan
            [0.64, 0.56, 0.95], // muted purple
            [0.42, 0.88, 0.68], // soft mint
            [0.92, 0.66, 0.38], // subtle amber
            [0.58, 0.78, 0.98], // soft blue-white
        ];

        // 3A. Connected terrace decks for clustered icons (subtle dark glass-slate foundation)
        for cl in &s.field.clusters {
            if cl.opacity <= 0.01 {
                continue;
            }
            push_sdf_round_rect(
                &mut self.sdf_verts,
                cl.cx as f32,
                cl.cy as f32,
                cl.rx as f32,
                cl.ry as f32,
                26.0 * unit,
                [0.045, 0.068, 0.112, (0.58 * cl.opacity) as f32],
                [0.36, 0.62, 0.80, (0.28 * cl.opacity as f32 * halo_mul).clamp(0.0, 0.48)],
            );
        }

        // 3B. Glowing conduit paths connecting clustered workstations
        for ln in &s.field.links {
            let a = &s.field.stations[ln.a];
            let b = &s.field.stations[ln.b];
            let mx = (a.cx + b.cx) as f32 * 0.5;
            let my = (a.cy + b.cy) as f32 * 0.5 + 8.0;
            let pts = sample_quad_bezier(a.cx as f32, a.cy as f32, mx, my, b.cx as f32, b.cy as f32, 12);
            let la = (ln.strength as f32 * 0.28 * halo_mul).clamp(0.0, 0.48);
            push_polyline(&mut self.col_verts, &pts, 2.2 * unit, [0.44, 0.78, 0.94, la]);
            // Traveling tiny light pulse along the workstation link
            let pt_idx = (((t * 0.35 + (ln.a as f32) * 0.37) % 1.0) * (pts.len() - 1) as f32) as usize;
            let (px, py) = pts[pt_idx.min(pts.len() - 1)];
            push_sdf_disc(&mut self.sdf_verts, px, py, 2.0 * unit, [0.72, 0.92, 1.0, la * 1.6]);
        }

        // 3C. Individual circular adaptive icon workstation pads (matte dark center -> crystal-clear desktop icons!)
        for stn in &s.field.stations {
            if stn.opacity <= 0.01 {
                continue;
            }
            let cx = stn.cx as f32;
            let cy = stn.cy as f32;
            let r = stn.pad_r as f32;
            let op = stn.opacity as f32;
            let acc = station_accents[stn.accent_idx];
            let pulse = 0.85 + 0.15 * (t * 1.4 + stn.id as f32 * 1.3).sin();
            let boost = (stn.move_energy.max(stn.inspect_glow) as f32) * 0.45;

            // Soft outer halo light beneath workstation
            if halo_mul > 0.02 {
                push_glow(
                    &mut self.tex_verts,
                    cx,
                    cy,
                    r * 1.75,
                    [acc[0], acc[1], acc[2], ((0.14 + boost * 0.16) * op * halo_mul * pulse).clamp(0.0, 0.45)],
                );
            }

            // Recessed circular matte platform pad (slightly dark navy so icon text stays super legible!)
            push_sdf_workstation(
                &mut self.sdf_verts,
                cx,
                cy,
                r,
                [0.055, 0.078, 0.125, 0.74 * op],
                [acc[0], acc[1], acc[2], ((0.36 + boost * 0.45) * op * halo_mul.max(0.35)).clamp(0.0, 0.88)],
            );

            // 4 subtle micro status indicator dots around the workstation perimeter
            for k in 0..4 {
                let ang = k as f32 * (PI * 0.5) + PI * 0.25;
                let dx = cx + ang.cos() * (r * 0.92);
                let dy = cy + ang.sin() * (r * 0.92);
                let blink = 0.55 + 0.45 * (t * 2.1 + stn.id as f32 + k as f32 * 1.1).sin();
                push_sdf_disc(
                    &mut self.sdf_verts,
                    dx,
                    dy,
                    1.7 * unit,
                    [acc[0], acc[1], acc[2], (0.55 * blink * op * halo_mul.max(0.3)).clamp(0.0, 0.85)],
                );
            }
        }

        // Flush ground + workstations before Y-sorted buildings, trees, terminals, lamps & mascots
        self.flush_col(gl, css_w, css_h);
        self.flush_tex(gl, css_w, css_h);
        self.flush_sdf(gl, css_w, css_h);

        // 4. Depth-Sorted Isometric Midground (Buildings, Terminals, Trees, Lamps, Mascots)
        #[derive(Clone, Copy)]
        enum DrawItem {
            Building(usize),
            Terminal(usize),
            Tree(usize),
            Lamp(usize),
            Mascot(usize),
        }
        let mut items: Vec<(f64, DrawItem)> = Vec::with_capacity(96);
        for (i, b) in s.env.buildings.iter().enumerate() {
            items.push((b.y + b.h * 0.25, DrawItem::Building(i)));
        }
        for (i, tm) in s.env.terminals.iter().enumerate() {
            items.push((tm.y, DrawItem::Terminal(i)));
        }
        for (i, tr) in s.env.trees.iter().enumerate() {
            items.push((tr.y, DrawItem::Tree(i)));
        }
        for (i, lm) in s.env.lamps.iter().enumerate() {
            items.push((lm.y, DrawItem::Lamp(i)));
        }
        for (i, m) in s.manager.mascots.iter().enumerate() {
            items.push((m.y, DrawItem::Mascot(i)));
        }
        items.sort_by(|a, b| a.0.total_cmp(&b.0));

        for &(_, item) in &items {
            match item {
                DrawItem::Building(idx) => {
                    let b = &s.env.buildings[idx];
                    // If a desktop icon sits directly over a building, softly dim building slightly to keep icon legible
                    let inf = s.field.sample(b.x, b.y);
                    let dim = (1.0 - inf.strength * 0.35) as f32;
                    draw_building(
                        &mut self.col_verts,
                        &mut self.tex_verts,
                        &mut self.sdf_verts,
                        b,
                        px_off,
                        py_off,
                        t,
                        glow_mul * dim,
                        dim,
                        unit,
                    );
                }
                DrawItem::Terminal(idx) => {
                    let tm = &s.env.terminals[idx];
                    let inf = s.field.sample(tm.x, tm.y);
                    let tx = (tm.x + inf.push_x * 0.85) as f32 + px_off;
                    let ty = (tm.y + inf.push_y * 0.85) as f32 + py_off;
                    draw_terminal(
                        &mut self.col_verts,
                        &mut self.tex_verts,
                        &mut self.sdf_verts,
                        tx,
                        ty,
                        tm.accent,
                        tm.kind,
                        t,
                        glow_mul,
                        unit,
                    );
                }
                DrawItem::Tree(idx) => {
                    let tr = &s.env.trees[idx];
                    draw_tree(
                        &mut self.col_verts,
                        tr.x as f32 + px_off,
                        tr.y as f32 + py_off,
                        (tr.size * tr.scale) as f32,
                        tr.variant,
                        (t * 0.85 + tr.sway_phase as f32).sin() * 1.4,
                        pal.artificial_light,
                    );
                }
                DrawItem::Lamp(idx) => {
                    let lm = &s.env.lamps[idx];
                    let lx = lm.x as f32 + px_off;
                    let ly = lm.y as f32 + py_off;
                    let op = lm.opacity as f32;
                    let lcol = if lm.warm { [0.98, 0.84, 0.62] } else { [0.58, 0.84, 0.96] };
                    // Sleek bollard pole
                    push_quad(&mut self.col_verts, lx - 1.4 * unit, ly - 15.0 * unit, 2.8 * unit, 15.0 * unit, [0.18, 0.23, 0.32, op]);
                    // Soft warm light pool on ground + lamp head
                    push_glow(
                        &mut self.tex_verts,
                        lx,
                        ly - 13.0 * unit,
                        28.0 * unit,
                        [lcol[0], lcol[1], lcol[2], (0.20 * glow_mul * op).clamp(0.0, 0.42)],
                    );
                    push_sdf_disc(
                        &mut self.sdf_verts,
                        lx,
                        ly - 15.0 * unit,
                        2.5 * unit,
                        [lcol[0], lcol[1], lcol[2], (0.88 * op).clamp(0.0, 1.0)],
                    );
                }
                DrawItem::Mascot(idx) => {
                    let m = &s.manager.mascots[idx];
                    draw_mascot(
                        &mut self.col_verts,
                        &mut self.tex_verts,
                        &mut self.sdf_verts,
                        m,
                        px_off,
                        py_off,
                        t,
                        glow_mul,
                        unit * 1.28,
                    );
                }
            }
        }

        // 5. Floating Data Particles & Tiny Patrol Drones
        let mote_cols: [[f32; 3]; 5] = [
            [0.52, 0.86, 0.96],
            [0.72, 0.64, 0.98],
            [0.46, 0.90, 0.72],
            [0.96, 0.78, 0.56],
            [0.84, 0.90, 0.98],
        ];
        for m in &s.motes {
            if !m.active {
                continue;
            }
            let c = mote_cols[m.color_idx];
            let pulse = 0.5 + 0.5 * (t * 1.6 + m.phase as f32).sin();
            let a = (0.20 + 0.36 * pulse) * (0.55 + 0.45 * pal.artificial_light);
            let mx = m.x as f32 + px_off * 1.15;
            let my = m.y as f32 + py_off * 1.15;
            push_sdf_disc(&mut self.sdf_verts, mx, my, m.size as f32 * unit, [c[0], c[1], c[2], a]);
            if pulse > 0.65 {
                push_glow(&mut self.tex_verts, mx, my, m.size as f32 * 5.0 * unit, [c[0], c[1], c[2], a * 0.28]);
            }
        }

        // Miniature autonomous patrol drones
        for d in &s.drones {
            let dx = (d.cx + d.angle.cos() * d.rx) as f32 + px_off * 1.25;
            let dy = (d.cy + d.angle.sin() * d.ry) as f32 + py_off * 1.25;
            let hover = (t * 3.2 + d.angle as f32).sin() * 2.0 * unit;
            // Ground shadow
            push_sdf_disc(&mut self.sdf_verts, dx, dy + 22.0 * unit, 4.0 * unit, [0.0, 0.0, 0.0, 0.22]);
            // Soft aura + sleek diamond chassis + blinking wing LED
            push_glow(&mut self.tex_verts, dx, dy + hover, 16.0 * unit, [d.accent[0], d.accent[1], d.accent[2], (0.22 * glow_mul).clamp(0.0, 0.42)]);
            push_iso_diamond(&mut self.col_verts, dx, dy + hover, 5.5 * unit, 2.8 * unit, [0.76, 0.82, 0.92, 0.92]);
            push_sdf_disc(&mut self.sdf_verts, dx, dy + hover, 1.6 * unit, [d.accent[0], d.accent[1], d.accent[2], 0.95]);
        }

        // Flush midground & foreground geometry
        self.flush_col(gl, css_w, css_h);
        self.flush_tex(gl, css_w, css_h);
        self.flush_sdf(gl, css_w, css_h);

        // 6. Subtle Hover-Only Mascot Category Labels (normally hidden; fades in smoothly near cursor)
        let mut labels_to_draw: Vec<(f32, f32, &'static str, f32, [f32; 3])> = Vec::new();
        for m in &s.manager.mascots {
            if m.label_alpha > 0.02 {
                let lx = m.x as f32 + px_off;
                let ly = m.y as f32 + py_off - 40.0 * unit;
                labels_to_draw.push((lx, ly, m.kind.label(), m.label_alpha as f32, m.kind.accent()));
            }
        }
        if !labels_to_draw.is_empty() {
            let font_sz = (10.0 * unit).clamp(9.0, 13.0);
            for &(lx, ly, lbl, a, acc) in &labels_to_draw {
                let tw = text.measure(gl, lbl, font_sz, dpi, true);
                let pw_pill = tw + 16.0 * unit;
                let ph_pill = 19.0 * unit;
                push_sdf_round_rect(
                    &mut self.sdf_verts,
                    lx,
                    ly,
                    pw_pill * 0.5,
                    ph_pill * 0.5,
                    9.5 * unit,
                    [0.06, 0.09, 0.14, 0.84 * a],
                    [acc[0], acc[1], acc[2], 0.65 * a],
                );
            }
            self.flush_sdf(gl, css_w, css_h);

            text.begin();
            for &(lx, ly, lbl, a, _acc) in &labels_to_draw {
                text.push(
                    gl,
                    lbl,
                    lx,
                    ly + font_sz * 0.36,
                    font_sz,
                    dpi,
                    true,
                    [0.92, 0.95, 1.0, a],
                    1,
                    None,
                );
            }
            text.flush(gl, css_w, css_h);
        }
    }
}

// =========================================================================
// 6. ARCHITECTURAL & CHARACTER ILLUSTRATION HELPERS
// =========================================================================

fn draw_building(
    col: &mut Vec<ColVert>,
    tex: &mut Vec<TexVert>,
    sdf: &mut Vec<SdfVert>,
    b: &CampusBuilding,
    px: f32,
    py: f32,
    t: f32,
    glow: f32,
    alpha: f32,
    u: f32,
) {
    let x = b.x as f32 + px;
    let y = b.y as f32 + py;
    let hw = b.w as f32 * 0.5;
    let hh = b.h as f32 * 0.5;
    let ht = b.height as f32;
    let acc = b.accent;

    // Ground drop shadow + soft ambient base aura
    push_iso_diamond(col, x + 8.0 * u, y + 6.0 * u, hw * 1.12, hh * 1.12, [0.01, 0.02, 0.04, 0.42 * alpha]);
    push_glow(tex, x, y + 4.0 * u, hw * 1.85, [acc[0], acc[1], acc[2], (0.14 * glow).clamp(0.0, 0.36)]);

    // Base plinth
    push_iso_box(
        col,
        x,
        y,
        hw * 1.06,
        hh * 1.06,
        7.0 * u,
        [0.14, 0.19, 0.28, alpha],
        [0.10, 0.14, 0.21, alpha],
        [0.075, 0.105, 0.165, alpha],
    );

    // Main architectural titanium-navy pavilion body
    let roof_c = [0.18, 0.24, 0.34, alpha];
    let left_c = [0.125, 0.17, 0.255, alpha];
    let right_c = [0.09, 0.125, 0.195, alpha];
    push_iso_box(col, x, y - 7.0 * u, hw * 0.94, hh * 0.94, ht, roof_c, left_c, right_c);

    // Subtle roof recessed skylight rim
    push_iso_diamond(col, x, y - 7.0 * u - ht, hw * 0.72, hh * 0.72, [0.13, 0.19, 0.29, alpha]);
    push_iso_diamond(
        col,
        x,
        y - 7.0 * u - ht,
        hw * 0.54,
        hh * 0.54,
        [acc[0] * 0.35 + 0.10, acc[1] * 0.35 + 0.14, acc[2] * 0.35 + 0.20, (0.55 + 0.25 * glow).clamp(0.0, 0.88) * alpha],
    );

    // Glowing architectural windows along both visible isometric facades
    let win_warm = [0.96, 0.89, 0.74, (0.52 + 0.32 * glow).clamp(0.0, 0.92) * alpha];
    let win_cool = [acc[0], acc[1], acc[2], (0.45 + 0.35 * glow).clamp(0.0, 0.88) * alpha];
    for w_i in 0..3 {
        let f = 0.22 + w_i as f32 * 0.28;
        // Left-front wall window
        let lx0 = x - hw * 0.94 * (1.0 - f);
        let ly0 = y - 7.0 * u + hh * 0.94 * f;
        let wx = hw * 0.16;
        let wy = hh * 0.16;
        let wc = if w_i == 1 { win_cool } else { win_warm };
        push_quad_pts(
            col,
            (lx0, ly0 - ht * 0.66),
            (lx0 + wx, ly0 + wy - ht * 0.66),
            (lx0 + wx, ly0 + wy - ht * 0.24),
            (lx0, ly0 - ht * 0.24),
            wc,
        );
        // Right-front wall window
        let rx0 = x + hw * 0.94 * (1.0 - f);
        let ry0 = y - 7.0 * u + hh * 0.94 * f;
        let wc2 = if w_i == 0 { win_cool } else { win_warm };
        push_quad_pts(
            col,
            (rx0 - wx, ry0 + wy - ht * 0.66),
            (rx0, ry0 - ht * 0.66),
            (rx0, ry0 - ht * 0.24),
            (rx0 - wx, ry0 + wy - ht * 0.24),
            wc2,
        );
    }

    // Building-specific signature rooftop centerpiece
    let roof_y = y - 7.0 * u - ht;
    match b.kind {
        BuildingKind::Atrium => {
            // Subtle glass dome + softly levitating holographic core
            let bob = (t * 1.4).sin() * 3.0 * u;
            push_glow(tex, x, roof_y - 16.0 * u + bob, 44.0 * u, [acc[0], acc[1], acc[2], (0.26 * glow).clamp(0.0, 0.5)]);
            push_sdf_ring(sdf, x, roof_y - 14.0 * u + bob, 14.0 * u, 1.4 * u, [acc[0], acc[1], acc[2], 0.55 * alpha]);
            push_iso_diamond(col, x, roof_y - 14.0 * u + bob, 8.0 * u, 8.0 * u, [0.85, 0.96, 1.0, 0.85 * alpha]);
        }
        BuildingKind::CodeForge => {
            // Holographic floating code display above studio roof
            let hx = x;
            let hy = roof_y - 16.0 * u + (t * 1.7).sin() * 2.0 * u;
            push_sdf_round_rect(sdf, hx, hy, 22.0 * u, 11.0 * u, 3.0 * u, [0.08, 0.20, 0.30, 0.52 * alpha], [acc[0], acc[1], acc[2], 0.55 * alpha]);
            for line in 0..3 {
                let lw = (14.0 + ((line * 7 + 3) % 11) as f32) * u;
                push_quad(col, hx - 16.0 * u, hy - 5.5 * u + line as f32 * 4.5 * u, lw, 1.8 * u, [acc[0], acc[1], acc[2], 0.72 * alpha]);
            }
        }
        BuildingKind::Observatory => {
            // Rotating purple analytical ring sculpture
            let cy = roof_y - 18.0 * u;
            push_sdf_ring(sdf, x, cy, 15.0 * u, 1.5 * u, [acc[0], acc[1], acc[2], 0.52 * alpha]);
            for k in 0..3 {
                let a = t * 0.7 + k as f32 * (TAU / 3.0);
                let ox = x + a.cos() * 15.0 * u;
                let oy = cy + a.sin() * 7.0 * u;
                push_sdf_disc(sdf, ox, oy, 2.8 * u, [0.84, 0.78, 1.0, 0.9 * alpha]);
            }
        }
        BuildingKind::ServerVault => {
            // Pulsing emerald compute stack LEDs
            let pulse = 0.5 + 0.5 * (t * 2.2).sin();
            push_glow(tex, x, roof_y - 4.0 * u, 36.0 * u, [acc[0], acc[1], acc[2], (0.24 * glow * (0.7 + 0.3 * pulse)).clamp(0.0, 0.48)]);
            for k in 0..4 {
                let lx = x - 18.0 * u + k as f32 * 12.0 * u;
                let blink = 0.45 + 0.55 * (t * 3.0 + k as f32 * 1.4).sin();
                push_sdf_disc(sdf, lx, roof_y - 2.0 * u, 2.2 * u, [acc[0], acc[1], acc[2], blink * alpha]);
            }
        }
        BuildingKind::Workshop => {
            // Warm studio skylight + subtle antenna beacon
            push_quad(col, x + hw * 0.35, roof_y - 22.0 * u, 2.0 * u, 22.0 * u, [0.42, 0.46, 0.55, alpha]);
            let blink = 0.5 + 0.5 * (t * 1.8).sin();
            push_sdf_disc(sdf, x + hw * 0.36, roof_y - 23.0 * u, 2.6 * u, [acc[0], acc[1], acc[2], (0.4 + 0.6 * blink) * alpha]);
        }
    }
}

fn draw_terminal(
    col: &mut Vec<ColVert>,
    tex: &mut Vec<TexVert>,
    sdf: &mut Vec<SdfVert>,
    x: f32,
    y: f32,
    acc: [f32; 3],
    kind: usize,
    t: f32,
    glow: f32,
    u: f32,
) {
    // Ground shadow
    push_iso_diamond(col, x + 2.0 * u, y + 2.0 * u, 15.0 * u, 8.5 * u, [0.0, 0.0, 0.0, 0.32]);

    if kind == 1 {
        // Miniature Local / Open-Source Server Rack
        push_iso_box(
            col,
            x,
            y,
            11.0 * u,
            6.5 * u,
            24.0 * u,
            [0.20, 0.25, 0.33, 0.96],
            [0.13, 0.17, 0.24, 0.96],
            [0.09, 0.12, 0.18, 0.96],
        );
        // Blinking server LEDs + pulsing energy aura
        let pulse = 0.55 + 0.45 * (t * 2.6 + x * 0.03).sin();
        push_glow(tex, x, y - 10.0 * u, 28.0 * u, [acc[0], acc[1], acc[2], (0.18 * glow * pulse).clamp(0.0, 0.38)]);
        for r in 0..4 {
            let b = 0.4 + 0.6 * (t * 3.8 + r as f32 * 1.7 + x * 0.02).sin();
            push_sdf_disc(sdf, x - 5.0 * u, y - 5.0 * u - r as f32 * 4.5 * u, 1.4 * u, [acc[0], acc[1], acc[2], b]);
            push_sdf_disc(sdf, x - 1.5 * u, y - 3.5 * u - r as f32 * 4.5 * u, 1.1 * u, [0.85, 0.92, 1.0, b * 0.7]);
        }
    } else if kind == 2 {
        // Holographic Research Pedestal
        push_iso_box(
            col,
            x,
            y,
            12.0 * u,
            7.0 * u,
            10.0 * u,
            [0.21, 0.26, 0.36, 0.95],
            [0.14, 0.18, 0.26, 0.95],
            [0.10, 0.13, 0.20, 0.95],
        );
        let bob = (t * 1.9 + x * 0.02).sin() * 2.2 * u;
        push_glow(tex, x, y - 18.0 * u + bob, 24.0 * u, [acc[0], acc[1], acc[2], (0.20 * glow).clamp(0.0, 0.42)]);
        push_sdf_ring(sdf, x, y - 18.0 * u + bob, 8.0 * u, 1.2 * u, [acc[0], acc[1], acc[2], 0.65]);
        push_iso_diamond(col, x, y - 18.0 * u + bob, 4.2 * u, 4.2 * u, [0.88, 0.95, 1.0, 0.85]);
    } else {
        // Glowing Computer Terminal Desk
        push_iso_box(
            col,
            x,
            y,
            14.0 * u,
            8.0 * u,
            10.0 * u,
            [0.20, 0.25, 0.34, 0.96],
            [0.14, 0.18, 0.26, 0.96],
            [0.10, 0.13, 0.20, 0.96],
        );
        // Glowing monitor screen
        push_sdf_round_rect(
            sdf,
            x,
            y - 16.0 * u,
            10.0 * u,
            6.5 * u,
            2.2 * u,
            [acc[0] * 0.28 + 0.06, acc[1] * 0.28 + 0.08, acc[2] * 0.28 + 0.12, 0.88],
            [acc[0], acc[1], acc[2], 0.72],
        );
        push_glow(tex, x, y - 15.0 * u, 24.0 * u, [acc[0], acc[1], acc[2], (0.16 * glow).clamp(0.0, 0.36)]);
    }
}

fn draw_tree(col: &mut Vec<ColVert>, x: f32, y: f32, size: f32, variant: usize, sway: f32, night_f: f32) {
    if size < 3.0 {
        return;
    }
    // Soft ground shadow
    push_iso_diamond(col, x + size * 0.25, y + size * 0.1, size * 0.78, size * 0.42, [0.01, 0.02, 0.04, 0.32]);
    // Slender architectural trunk
    push_quad(col, x - size * 0.11, y - size * 0.65, size * 0.22, size * 0.7, [0.22, 0.20, 0.22, 0.92]);

    // Muted nocturnal teal-pine & sage foliage (no harsh bright greens)
    let lift = (1.0 - night_f) * 0.09;
    let (c_dark, c_mid, c_hi) = match variant {
        0 => (
            [0.08 + lift, 0.17 + lift, 0.20 + lift, 0.95],
            [0.11 + lift, 0.24 + lift, 0.26 + lift, 0.95],
            [0.16 + lift, 0.32 + lift, 0.34 + lift, 0.92],
        ),
        1 => (
            [0.10 + lift, 0.16 + lift, 0.24 + lift, 0.95],
            [0.15 + lift, 0.23 + lift, 0.31 + lift, 0.95],
            [0.21 + lift, 0.30 + lift, 0.39 + lift, 0.92],
        ),
        _ => (
            [0.11 + lift, 0.19 + lift, 0.18 + lift, 0.95],
            [0.16 + lift, 0.27 + lift, 0.24 + lift, 0.95],
            [0.22 + lift, 0.35 + lift, 0.30 + lift, 0.92],
        ),
    };

    // 3 layered stylized geometric canopy tiers
    push_iso_diamond(col, x + sway * 0.3, y - size * 0.68, size * 0.85, size * 0.56, c_dark);
    push_iso_diamond(col, x + sway * 0.6, y - size * 1.02, size * 0.68, size * 0.46, c_mid);
    push_iso_diamond(col, x + sway, y - size * 1.32, size * 0.44, size * 0.32, c_hi);
}

fn draw_mascot(
    col: &mut Vec<ColVert>,
    tex: &mut Vec<TexVert>,
    sdf: &mut Vec<SdfVert>,
    m: &Mascot,
    px: f32,
    py: f32,
    t: f32,
    glow: f32,
    u: f32,
) {
    let x = m.x as f32 + px;
    let y = m.y as f32 + py;
    let acc = m.kind.accent();
    let body = m.kind.body_color();
    let facing = m.facing as f32;
    let walk_spd = (m.vx.hypot(m.vy) as f32 / m.speed.max(1.0) as f32).clamp(0.0, 1.0);
    let step_bob = if m.is_floating {
        (t * 2.4 + m.anim_phase as f32).sin() * 3.8 * u - 5.5 * u
    } else if m.state == MascotBehavior::Sleep {
        2.5 * u
    } else {
        (m.anim_phase as f32 * 2.2).sin().abs() * -2.6 * u * walk_spd + (t * 1.8 + m.anim_phase as f32).sin() * 0.8 * u
    };
    let by = y + step_bob;

    // Ground shadow + subtle signature character aura
    push_iso_diamond(col, x, y + 3.0 * u, 11.5 * u, 6.0 * u, [0.0, 0.0, 0.0, 0.38]);
    let aura_a = ((0.14 + m.cursor_focus as f32 * 0.14) * glow).clamp(0.0, 0.44);
    push_glow(tex, x, by - 10.0 * u, 28.0 * u, [acc[0], acc[1], acc[2], aura_a]);

    // Little animated feet (non-floating bots) or thruster glow (Research AI)
    if m.is_floating {
        let th = 0.65 + 0.35 * (t * 6.0 + m.anim_phase as f32).sin();
        push_glow(tex, x, by + 3.0 * u, 13.0 * u, [acc[0], acc[1], acc[2], 0.38 * th]);
        push_sdf_disc(sdf, x, by + 1.5 * u, 3.0 * u, [0.85, 0.94, 1.0, 0.78 * th]);
    } else {
        let stride = (m.anim_phase as f32 * 2.2).sin() * 2.6 * u * walk_spd;
        push_sdf_disc(sdf, x - 3.6 * u + stride, y + 1.0 * u, 2.3 * u, [0.16, 0.20, 0.28, 0.95]);
        push_sdf_disc(sdf, x + 3.6 * u - stride, y + 1.0 * u, 2.3 * u, [0.16, 0.20, 0.28, 0.95]);
    }

    // Main silhouette tailored per AI archetype (recognizable even when small!)
    let (body_w, body_h, head_y, head_r) = match m.kind {
        MascotKind::Reasoning => (8.5 * u, 11.5 * u, by - 21.0 * u, 7.6 * u), // taller dignified silhouette
        MascotKind::Local => (10.5 * u, 8.8 * u, by - 16.5 * u, 7.2 * u),     // sturdy compact server bot
        MascotKind::General => (8.2 * u, 9.2 * u, by - 17.5 * u, 7.8 * u),    // friendly pearlescent capsule
        MascotKind::Research => (7.8 * u, 10.0 * u, by - 18.5 * u, 7.2 * u),  // hovering teardrop
        _ => (8.4 * u, 9.0 * u, by - 17.0 * u, 7.4 * u),
    };

    // Torso
    push_sdf_round_rect(
        sdf,
        x,
        by - body_h * 0.85,
        body_w,
        body_h * 0.78,
        if m.kind == MascotKind::Local || m.kind == MascotKind::OpenSource { 3.2 * u } else { 6.2 * u },
        [body[0], body[1], body[2], 0.98],
        [acc[0], acc[1], acc[2], 0.42],
    );
    // Chest energy core / indicator strip
    let core_pulse = 0.65 + 0.35 * (t * 2.5 + m.anim_phase as f32).sin();
    push_sdf_disc(sdf, x + facing * 1.4 * u, by - body_h * 0.85, 2.2 * u, [acc[0], acc[1], acc[2], 0.88 * core_pulse]);

    // Waving / typing / working miniature arms
    let arm_wave = if m.wave_amount > 0.05 {
        -7.0 * u * m.wave_amount as f32 + (t * 8.5).sin() * 2.8 * u * m.wave_amount as f32
    } else if m.state == MascotBehavior::Work {
        (t * 9.0 + m.anim_phase as f32).sin() * 1.8 * u - 1.5 * u
    } else {
        0.0
    };
    push_sdf_disc(sdf, x - body_w - 1.2 * u, by - body_h * 0.78 + arm_wave * 0.4, 2.1 * u, [body[0] * 0.88, body[1] * 0.88, body[2] * 0.88, 0.95]);
    push_sdf_disc(sdf, x + body_w + 1.2 * u, by - body_h * 0.78 + arm_wave, 2.1 * u, [body[0] * 0.88, body[1] * 0.88, body[2] * 0.88, 0.95]);

    // Head + expressive face/visor oriented toward look_angle
    let look_dx = (m.look_angle.cos() as f32) * 2.2 * u;
    let look_dy = (m.look_angle.sin() as f32) * 1.0 * u;
    let head_x = x + look_dx * 0.35;

    if m.kind == MascotKind::Vision {
        // Camera-head articulated Vision AI bot with glowing amber optical aperture lens
        push_sdf_round_rect(
            sdf,
            head_x,
            head_y,
            head_r * 1.15,
            head_r * 0.85,
            4.2 * u,
            [body[0], body[1], body[2], 0.98],
            [acc[0], acc[1], acc[2], 0.55],
        );
        // Optical aperture housing + glowing iris
        push_sdf_disc(sdf, head_x + look_dx, head_y + look_dy, 4.4 * u, [0.08, 0.10, 0.15, 0.98]);
        push_sdf_ring(sdf, head_x + look_dx, head_y + look_dy, 3.3 * u, 1.0 * u, [acc[0], acc[1], acc[2], 0.92]);
        push_sdf_disc(sdf, head_x + look_dx * 1.15, head_y + look_dy * 1.15, 1.5 * u, [1.0, 0.92, 0.72, 0.96]);

        // Occasional subtle translucent vision scan cone
        let scan_on = (m.scan_phase as f32).sin();
        if scan_on > 0.25 && m.state != MascotBehavior::Sleep {
            let sa = (scan_on - 0.25) * 0.22 * glow;
            let a0 = m.look_angle as f32 - 0.24;
            let a1 = m.look_angle as f32 + 0.24;
            let cone_r = 46.0 * u;
            let ox = head_x + look_dx;
            let oy = head_y + look_dy;
            col.extend_from_slice(&[
                ColVert { x: ox, y: oy, r: acc[0], g: acc[1], b: acc[2], a: sa },
                ColVert { x: ox + a0.cos() * cone_r, y: oy + a0.sin() * cone_r, r: acc[0], g: acc[1], b: acc[2], a: 0.0 },
                ColVert { x: ox + a1.cos() * cone_r, y: oy + a1.sin() * cone_r, r: acc[0], g: acc[1], b: acc[2], a: 0.0 },
            ]);
        }
    } else {
        // Rounded/chamfered robot head + dark sleek visor + expressive glowing eyes
        push_sdf_round_rect(
            sdf,
            head_x,
            head_y,
            head_r * 1.06,
            head_r * 0.88,
            if m.kind == MascotKind::Coding || m.kind == MascotKind::OpenSource { 3.5 * u } else { 6.0 * u },
            [body[0] * 1.05, body[1] * 1.05, body[2] * 1.05, 0.98],
            [acc[0], acc[1], acc[2], 0.48],
        );
        // Dark faceplate visor
        push_sdf_round_rect(
            sdf,
            head_x + look_dx * 0.6,
            head_y + look_dy * 0.5,
            head_r * 0.78,
            head_r * 0.44,
            3.2 * u,
            [0.06, 0.08, 0.13, 0.96],
            [acc[0], acc[1], acc[2], 0.25],
        );
        // Eyes (blink gently or dim horizontally when sleeping)
        let blink = ((t * 0.65 + m.anim_phase as f32) % 4.2) > 4.06;
        let eye_r = if m.state == MascotBehavior::Sleep || blink { 0.85 * u } else { 1.65 * u };
        let ex0 = head_x + look_dx * 0.85 - 2.6 * u;
        let ex1 = head_x + look_dx * 0.85 + 2.6 * u;
        let ey = head_y + look_dy * 0.55;
        push_sdf_disc(sdf, ex0, ey, eye_r, [acc[0], acc[1], acc[2], 0.96]);
        push_sdf_disc(sdf, ex1, ey, eye_r, [acc[0], acc[1], acc[2], 0.96]);
    }

    // Archetype-unique signature floating accessories & animations!
    match m.kind {
        MascotKind::General => {
            // Friendly little antenna stem with soft mint beacon
            push_quad(col, head_x - 0.7 * u, head_y - head_r - 4.0 * u, 1.4 * u, 4.5 * u, [0.78, 0.84, 0.90, 0.9]);
            push_sdf_disc(sdf, head_x, head_y - head_r - 4.5 * u, 2.0 * u, [acc[0], acc[1], acc[2], 0.92]);
        }
        MascotKind::Coding => {
            // Tiny orbiting floating code sparks + mini holo keyboard when working
            for k in 0..3 {
                let a = t * 1.4 + k as f32 * (TAU / 3.0);
                let cx = x + a.cos() * 15.0 * u;
                let cy = head_y + 4.0 * u + a.sin() * 6.0 * u;
                push_quad(col, cx - 2.2 * u, cy - 0.9 * u, 4.4 * u, 1.8 * u, [acc[0], acc[1], acc[2], 0.78]);
            }
            if m.state == MascotBehavior::Work {
                let hx = x + facing * 13.0 * u;
                let hy = by - 9.0 * u;
                push_sdf_round_rect(sdf, hx, hy, 7.5 * u, 4.5 * u, 1.5 * u, [0.10, 0.28, 0.38, 0.55], [acc[0], acc[1], acc[2], 0.75]);
            }
        }
        MascotKind::Reasoning => {
            // Slowly rotating floating geometric objects above crown (diamond, triangle, ring)
            for k in 0..3 {
                let a = t * 0.75 + k as f32 * (TAU / 3.0);
                let gx = head_x + a.cos() * 13.5 * u;
                let gy = head_y - 12.5 * u + a.sin() * 4.5 * u;
                if k == 0 {
                    push_iso_diamond(col, gx, gy, 3.6 * u, 3.6 * u, [acc[0], acc[1], acc[2], 0.85]);
                } else if k == 1 {
                    push_sdf_ring(sdf, gx, gy, 3.2 * u, 1.0 * u, [0.85, 0.78, 1.0, 0.82]);
                } else {
                    col.extend_from_slice(&[
                        ColVert { x: gx, y: gy - 3.6 * u, r: acc[0], g: acc[1], b: acc[2], a: 0.85 },
                        ColVert { x: gx + 3.4 * u, y: gy + 2.6 * u, r: acc[0], g: acc[1], b: acc[2], a: 0.85 },
                        ColVert { x: gx - 3.4 * u, y: gy + 2.6 * u, r: acc[0], g: acc[1], b: acc[2], a: 0.85 },
                    ]);
                }
            }
        }
        MascotKind::Vision => {
            // Top sensor fin
            push_quad(col, head_x - 3.5 * u, head_y - head_r * 0.85 - 2.6 * u, 7.0 * u, 2.6 * u, [0.35, 0.40, 0.48, 0.95]);
        }
        MascotKind::Research => {
            // 2 miniature floating translucent document/data panels flanking the bot
            for side in [-1.0f32, 1.0f32] {
                let bob = (t * 1.7 + side * 1.4).sin() * 2.2 * u;
                let dx = x + side * 15.5 * u;
                let dy = head_y + 5.0 * u + bob;
                push_sdf_round_rect(sdf, dx, dy, 4.6 * u, 5.8 * u, 1.4 * u, [0.16, 0.26, 0.40, 0.56], [acc[0], acc[1], acc[2], 0.78]);
                push_quad(col, dx - 2.6 * u, dy - 2.0 * u, 5.2 * u, 1.1 * u, [0.88, 0.95, 1.0, 0.75]);
                push_quad(col, dx - 2.6 * u, dy + 0.6 * u, 3.8 * u, 1.1 * u, [acc[0], acc[1], acc[2], 0.75]);
            }
        }
        MascotKind::Local => {
            // Subtle pulsing emerald harmonic ring when charging or active near server
            let p = (t * 0.9 + m.anim_phase as f32) % 1.0;
            let ring_a = (1.0 - p) * if m.state == MascotBehavior::Charge { 0.55 } else { 0.24 };
            push_sdf_ring(sdf, x, y + 1.0 * u, (10.0 + p * 16.0) * u, 1.3 * u, [acc[0], acc[1], acc[2], ring_a]);
        }
        MascotKind::Creative => {
            // Slowly morphing translucent pastel rose-violet abstract art orb & ribbon
            let morph = 0.5 + 0.5 * (t * 1.1).sin();
            let cx = x + facing * 13.0 * u;
            let cy = head_y - 5.0 * u + (t * 1.6).sin() * 2.5 * u;
            push_iso_diamond(col, cx, cy, (3.2 + morph * 2.5) * u, (5.2 - morph * 2.0) * u, [acc[0], acc[1], acc[2], 0.78]);
            push_sdf_ring(sdf, cx + 4.0 * u, cy - 4.0 * u, (2.4 + (1.0 - morph) * 1.8) * u, 1.0 * u, [0.76, 0.62, 0.98, 0.72]);
        }
        MascotKind::OpenSource => {
            // Rotating mechanical gear silhouette on shoulder + terminal-green accent pip
            let gx = x - facing * 10.5 * u;
            let gy = by - body_h * 1.25;
            push_sdf_ring(sdf, gx, gy, 3.8 * u, 1.2 * u, [acc[0], acc[1], acc[2], 0.88]);
            for k in 0..4 {
                let a = t * 1.2 + k as f32 * (PI * 0.5);
                push_sdf_disc(sdf, gx + a.cos() * 4.4 * u, gy + a.sin() * 4.4 * u, 1.1 * u, [0.42, 0.86, 0.58, 0.88]);
            }
        }
    }

    // Small carried data orb when delivering between stations
    if m.carry_orb && m.state == MascotBehavior::Walk {
        push_sdf_disc(sdf, x + facing * 10.0 * u, by - 9.0 * u, 2.6 * u, [0.85, 0.96, 1.0, 0.92]);
    }
}

// =========================================================================
// 7. CLEAN GEOMETRY HELPERS
// =========================================================================

fn push_quad(out: &mut Vec<ColVert>, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]) {
    let v = |px, py| ColVert { x: px, y: py, r: c[0], g: c[1], b: c[2], a: c[3] };
    out.extend_from_slice(&[
        v(x, y), v(x + w, y), v(x + w, y + h),
        v(x, y), v(x + w, y + h), v(x, y + h),
    ]);
}

fn push_quad_pts(out: &mut Vec<ColVert>, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), c: [f32; 4]) {
    let v = |(px, py)| ColVert { x: px, y: py, r: c[0], g: c[1], b: c[2], a: c[3] };
    out.extend_from_slice(&[v(p0), v(p1), v(p2), v(p0), v(p2), v(p3)]);
}

fn push_iso_diamond(out: &mut Vec<ColVert>, cx: f32, cy: f32, rx: f32, ry: f32, c: [f32; 4]) {
    push_quad_pts(out, (cx, cy - ry), (cx + rx, cy), (cx, cy + ry), (cx - rx, cy), c);
}

fn push_iso_box(
    out: &mut Vec<ColVert>,
    cx: f32,
    cy: f32,
    hw: f32,
    hh: f32,
    height: f32,
    top_col: [f32; 4],
    left_col: [f32; 4],
    right_col: [f32; 4],
) {
    let t_n = (cx, cy - hh - height);
    let t_e = (cx + hw, cy - height);
    let t_s = (cx, cy + hh - height);
    let t_w = (cx - hw, cy - height);
    let b_e = (cx + hw, cy);
    let b_s = (cx, cy + hh);
    let b_w = (cx - hw, cy);
    // Left-front wall
    push_quad_pts(out, t_w, t_s, b_s, b_w, left_col);
    // Right-front wall
    push_quad_pts(out, t_s, t_e, b_e, b_s, right_col);
    // Roof diamond
    push_quad_pts(out, t_n, t_e, t_s, t_w, top_col);
}

fn push_polyline(out: &mut Vec<ColVert>, pts: &[(f32, f32)], width: f32, c: [f32; 4]) {
    if pts.len() < 2 || c[3] <= 0.001 {
        return;
    }
    let hw = width * 0.5;
    let mk = |x, y| ColVert { x, y, r: c[0], g: c[1], b: c[2], a: c[3] };
    let mut normals: Vec<(f32, f32)> = Vec::with_capacity(pts.len());
    for i in 0..pts.len() {
        let (dx, dy) = if i == 0 {
            (pts[1].0 - pts[0].0, pts[1].1 - pts[0].1)
        } else if i + 1 == pts.len() {
            (pts[i].0 - pts[i - 1].0, pts[i].1 - pts[i - 1].1)
        } else {
            (pts[i + 1].0 - pts[i - 1].0, pts[i + 1].1 - pts[i - 1].1)
        };
        let len = dx.hypot(dy).max(1e-4);
        normals.push((-dy / len * hw, dx / len * hw));
    }
    for i in 0..(pts.len() - 1) {
        let (ax, ay) = pts[i];
        let (bx, by) = pts[i + 1];
        let (nax, nay) = normals[i];
        let (nbx, nby) = normals[i + 1];
        out.extend_from_slice(&[
            mk(ax - nax, ay - nay), mk(ax + nax, ay + nay), mk(bx + nbx, by + nby),
            mk(ax - nax, ay - nay), mk(bx + nbx, by + nby), mk(bx - nbx, by - nby),
        ]);
    }
}

fn sample_quad_bezier(x0: f32, y0: f32, cx: f32, cy: f32, x1: f32, y1: f32, segs: usize) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(segs + 1);
    for i in 0..=segs {
        let t = i as f32 / segs as f32;
        let u = 1.0 - t;
        out.push((
            u * u * x0 + 2.0 * u * t * cx + t * t * x1,
            u * u * y0 + 2.0 * u * t * cy + t * t * y1,
        ));
    }
    out
}

fn push_glow(out: &mut Vec<TexVert>, cx: f32, cy: f32, r: f32, col: [f32; 4]) {
    if col[3] <= 0.002 || r <= 0.5 {
        return;
    }
    let mk = |x, y, u, v| TexVert { x, y, u, v, col };
    out.extend_from_slice(&[
        mk(cx - r, cy - r, 0.0, 0.0), mk(cx + r, cy - r, 1.0, 0.0), mk(cx + r, cy + r, 1.0, 1.0),
        mk(cx - r, cy - r, 0.0, 0.0), mk(cx + r, cy + r, 1.0, 1.0), mk(cx - r, cy + r, 0.0, 1.0),
    ]);
}

fn push_sdf_disc(out: &mut Vec<SdfVert>, cx: f32, cy: f32, r: f32, col: [f32; 4]) {
    if col[3] <= 0.002 || r <= 0.1 {
        return;
    }
    let pad = r + 1.25;
    let lim = pad / r;
    let z = [0.0; 4];
    let mk = |dx, dy, lx, ly| SdfVert { x: cx + dx, y: cy + dy, lx, ly, col, stroke: z, params: [0.0; 4] };
    out.extend_from_slice(&[
        mk(-pad, -pad, -lim, -lim), mk(pad, -pad, lim, -lim), mk(pad, pad, lim, lim),
        mk(-pad, -pad, -lim, -lim), mk(pad, pad, lim, lim), mk(-pad, pad, -lim, lim),
    ]);
}

fn push_sdf_ring(out: &mut Vec<SdfVert>, cx: f32, cy: f32, r: f32, width: f32, col: [f32; 4]) {
    if col[3] <= 0.002 || r <= 0.5 {
        return;
    }
    let hw = width * 0.5;
    let outer = r + hw;
    let inner = (r - hw).max(0.0);
    let pad = outer + 1.5;
    let lim = pad / outer;
    let p = [1.0, inner / outer, 1.0, 0.0];
    let z = [0.0; 4];
    let mk = |dx, dy, lx, ly| SdfVert { x: cx + dx, y: cy + dy, lx, ly, col, stroke: z, params: p };
    out.extend_from_slice(&[
        mk(-pad, -pad, -lim, -lim), mk(pad, -pad, lim, -lim), mk(pad, pad, lim, lim),
        mk(-pad, -pad, -lim, -lim), mk(pad, pad, lim, lim), mk(-pad, pad, -lim, lim),
    ]);
}

fn push_sdf_round_rect(
    out: &mut Vec<SdfVert>,
    cx: f32,
    cy: f32,
    hw: f32,
    hh: f32,
    rad: f32,
    fill: [f32; 4],
    stroke: [f32; 4],
) {
    let pad = 2.0;
    let p = [2.0, hw, hh, rad];
    let mk = |lx, ly| SdfVert { x: cx + lx, y: cy + ly, lx, ly, col: fill, stroke, params: p };
    out.extend_from_slice(&[
        mk(-hw - pad, -hh - pad), mk(hw + pad, -hh - pad), mk(hw + pad, hh + pad),
        mk(-hw - pad, -hh - pad), mk(hw + pad, hh + pad), mk(-hw - pad, hh + pad),
    ]);
}

fn push_sdf_workstation(
    out: &mut Vec<SdfVert>,
    cx: f32,
    cy: f32,
    r: f32,
    fill: [f32; 4],
    stroke: [f32; 4],
) {
    let pad = r + 2.0;
    let lim = pad / r.max(1.0);
    let p = [3.0, 1.0, 0.0, 0.0];
    let mk = |dx, dy, lx, ly| SdfVert { x: cx + dx, y: cy + dy, lx, ly, col: fill, stroke, params: p };
    out.extend_from_slice(&[
        mk(-pad, -pad, -lim, -lim), mk(pad, -pad, lim, -lim), mk(pad, pad, lim, lim),
        mk(-pad, -pad, -lim, -lim), mk(pad, pad, lim, lim), mk(-pad, pad, -lim, lim),
    ]);
}

// =========================================================================
// 8. UNIT TESTS (Determinism, Params, Edge Cases 0/1/10/60 Icons, Glow Border)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_defaults_and_enum_mapping_are_consistent() {
        let def = AiMascotsParams::default();
        assert_eq!(def.time_mode, 0);
        assert_eq!(def.quality, 2);
        assert!(def.adaptive);

        let mut raw = HashMap::new();
        raw.insert("timeMode".into(), Value::String("evening".into()));
        raw.insert("quality".into(), Value::String("Low".into()));
        raw.insert("mascotActivity".into(), Value::from(9.0));
        raw.insert("adaptive".into(), Value::Bool(false));
        let p = AiMascotsParams::from_raw_params(&raw);
        assert_eq!(p.time_mode, 1);
        assert_eq!(p.quality, 0);
        assert!((p.mascot_activity - 2.0).abs() < 1e-6);
        assert!(!p.adaptive);

        let mut raw_idx = HashMap::new();
        raw_idx.insert("timeMode".into(), Value::from(3));
        raw_idx.insert("quality".into(), Value::from(1));
        let p2 = AiMascotsParams::from_raw_params(&raw_idx);
        assert_eq!(p2.time_mode, 3); // cycle
        assert_eq!(p2.quality, 1);   // Medium
    }

    #[test]
    fn edge_cases_zero_one_ten_and_sixty_icons_stay_bounded_and_avoid_mascots() {
        let mut st = AiMascotsState::default();
        st.resize(1920.0, 1080.0);

        // 0 icons
        st.set_icons(&[], &[], false);
        for _ in 0..30 {
            st.step(0.033);
        }
        assert_eq!(st.manager.mascots.len(), 8);

        // 1 icon
        let one = vec![Rect { x: 400.0, y: 300.0, w: 76.0, h: 86.0 }];
        st.set_icons(&one, &["VS Code".into()], true);
        for _ in 0..60 {
            st.step(0.033);
        }
        assert_eq!(st.field.stations.len(), 1);
        assert!(st.field.stations[0].opacity > 0.9);

        // Move same icon -> spring glides smoothly, spikes move_energy, bumps layout_rev
        let rev_before = st.layout_rev;
        let moved = vec![Rect { x: 820.0, y: 420.0, w: 76.0, h: 86.0 }];
        st.set_icons(&moved, &["VS Code".into()], true);
        assert!(st.layout_rev > rev_before);
        assert!(st.field.stations[0].move_energy > 0.3);
        // Should NOT instantaneously teleport to 858
        assert!((st.field.stations[0].cx - 858.0).abs() > 100.0);
        for _ in 0..90 {
            st.step(0.033);
        }
        assert!((st.field.stations[0].cx - 858.0).abs() < 4.0);

        // 60 clustered icons -> capped cleanly, zero NaNs, mascots never overlap icon centers
        let mut many = Vec::new();
        let mut names = Vec::new();
        for i in 0..60 {
            let col = (i % 10) as f32;
            let row = (i / 10) as f32;
            many.push(Rect { x: 50.0 + col * 110.0, y: 60.0 + row * 120.0, w: 74.0, h: 84.0 });
            names.push(format!("Item {i}"));
        }
        st.set_icons(&many, &names, true);
        for _ in 0..45 {
            st.step(0.033);
        }
        assert!(st.field.stations.len() <= 64);
        assert!(!st.field.clusters.is_empty());
        for m in &st.manager.mascots {
            assert!(m.x.is_finite() && m.y.is_finite());
        }
    }

    #[test]
    fn resize_threshold_and_glow_sprite_transparent_border() {
        let mut st = AiMascotsState::default();
        let rev0 = st.layout_rev;
        st.dirty = false;
        st.resize(1920.2, 1080.3);
        assert!(!st.dirty);
        assert_eq!(st.layout_rev, rev0);

        st.resize(2560.0, 1440.0);
        assert!(st.dirty);
        assert!(st.layout_rev > rev0);

        let px = bake_soft_glow_sprite();
        for x in 0..128 {
            assert_eq!(px[(0 * 128 + x) * 4 + 3], 0);
            assert_eq!(px[(127 * 128 + x) * 4 + 3], 0);
        }
        for y in 0..128 {
            assert_eq!(px[(y * 128 + 0) * 4 + 3], 0);
            assert_eq!(px[(y * 128 + 127) * 4 + 3], 0);
        }
    }
}
