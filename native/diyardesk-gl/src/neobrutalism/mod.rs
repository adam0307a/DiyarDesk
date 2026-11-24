pub mod shaders;

use crate::gl::*;
use crate::quiet_snow::Mulberry32;
use crate::shaders::FULLSCREEN_VERT;
use crate::state::{AppState, Rect};
use shaders::*;
use std::f32::consts::PI;
use std::ffi::c_void;

// =========================================================================
// 1. PALETTES & QUALITY PROFILES (1:1 neobrutalism.html)
// =========================================================================
#[derive(Clone, Copy)]
pub struct NbPalette {
    pub bg: [f32; 3],
    pub grid: [f32; 4],
    pub dot: [f32; 4],
    pub ink: [f32; 3],
    pub shadow: [f32; 3],
    pub label_plate: [f32; 3],
    pub blocks: [[f32; 3]; 5],
    #[allow(dead_code)]
    pub is_dark: bool,
}

#[inline]
const fn hex(h: u32) -> [f32; 3] {
    [
        ((h >> 16) & 0xff) as f32 / 255.0,
        ((h >> 8) & 0xff) as f32 / 255.0,
        (h & 0xff) as f32 / 255.0,
    ]
}

#[inline]
const fn rgba(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    [r, g, b, a]
}

macro_rules! nb {
    ($bg:expr, $grid:expr, $dot:expr, $ink:expr, $shadow:expr, $plate:expr,
     $b0:expr, $b1:expr, $b2:expr, $b3:expr, $b4:expr, $dark:expr) => {
        NbPalette {
            bg: hex($bg),
            grid: $grid,
            dot: $dot,
            ink: hex($ink),
            shadow: hex($shadow),
            label_plate: hex($plate),
            blocks: [hex($b0), hex($b1), hex($b2), hex($b3), hex($b4)],
            is_dark: $dark,
        }
    };
}

pub const PALETTES: [NbPalette; 4] = [
    // 0: bauhaus
    nb!(
        0xf6f1e7,
        rgba(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0, 0.08),
        rgba(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0, 0.13),
        0x111111, 0x111111, 0x111111,
        0xffe14d, 0x3b82f6, 0xff5c4d, 0x34d399, 0xf4f1ea, false
    ),
    // 1: acid
    nb!(
        0xf4fae8,
        rgba(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0, 0.08),
        rgba(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0, 0.14),
        0x111111, 0x111111, 0x111111,
        0xc8ff3d, 0xff4db8, 0x22d3ee, 0xa855f7, 0xffffff, false
    ),
    // 2: zine
    nb!(
        0xefe8d8,
        rgba(20.0 / 255.0, 18.0 / 255.0, 16.0 / 255.0, 0.09),
        rgba(20.0 / 255.0, 18.0 / 255.0, 16.0 / 255.0, 0.14),
        0x141210, 0x141210, 0x141210,
        0xff4828, 0xf5b700, 0x2347e8, 0xff9f7f, 0xfaf6ee, false
    ),
    // 3: dark
    nb!(
        0x16161c,
        rgba(245.0 / 255.0, 242.0 / 255.0, 235.0 / 255.0, 0.06),
        rgba(245.0 / 255.0, 242.0 / 255.0, 235.0 / 255.0, 0.10),
        0xf5f2eb, 0x000000, 0x09090d,
        0xffe14d, 0x38bdf8, 0xfb7185, 0xa3e635, 0x262632, true
    ),
];

#[derive(Clone, Copy)]
pub struct NbQuality {
    #[allow(dead_code)]
    pub max_dpr: f32,
    pub block_scale: f32,
    pub enable_dots: bool,
    pub enable_sparks: bool,
}

pub const QUALITY_PROFILES: [NbQuality; 3] = [
    NbQuality { max_dpr: 0.75, block_scale: 0.55, enable_dots: false, enable_sparks: false },
    NbQuality { max_dpr: 1.25, block_scale: 0.85, enable_dots: true, enable_sparks: true },
    NbQuality { max_dpr: 1.50, block_scale: 1.00, enable_dots: true, enable_sparks: true },
];

// =========================================================================
// 2. PARAMETERS & STATE
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct NbParams {
    pub palette: usize,
    pub border_weight: f32,
    pub shadow_depth: f32,
    pub shape_count: f32,
    pub motion_speed: f32,
    pub cursor_push: f32,
    pub halftone: bool,
    pub icon_frame: usize, // 0=card, 1=sticker, 2=blueprint
    pub quality: usize,    // 0=Low, 1=Medium, 2=High
    pub adaptive: i32,
    pub seed: u32,
}

impl Default for NbParams {
    fn default() -> Self {
        Self {
            palette: 0,
            border_weight: 3.0,
            shadow_depth: 7.0,
            shape_count: 1.0,
            motion_speed: 0.85,
            cursor_push: 1.0,
            halftone: true,
            icon_frame: 0,
            quality: 2,
            adaptive: 1,
            seed: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct NbShape {
    pub kind: usize, // 0=card,1=pill,2=circle,3=star,4=striped
    pub w: f32,
    pub h: f32,
    pub points: usize,
    pub color_idx: usize,
    pub home_x: f32,
    pub home_y: f32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub rot: f32,
    pub rot_base: f32,
    #[allow(dead_code)]
    pub rot_speed: f32,
    pub amp_x: f32,
    pub amp_y: f32,
    pub freq_x: f32,
    pub freq_y: f32,
    pub phase: f32,
    pub press: f32,
    pub press_vel: f32,
}

#[derive(Clone, Copy)]
pub struct NbShockwave {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub max_r: f32,
    pub square: bool,
}

#[derive(Clone, Copy)]
pub struct NbSpark {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub size: f32,
    pub rot: f32,
    pub color_idx: usize,
    pub life: f32,
    pub max_life: f32,
}

#[derive(Clone, Copy)]
pub struct NbIconState {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub tx: f32,
    pub ty: f32,
    pub tw: f32,
    pub th: f32,
    pub opacity: f32,
    pub color_idx: usize,
    pub tilt: f32,
    pub press: f32,
    pub press_vel: f32,
    pub removing: bool,
}

pub struct NeobrutalismState {
    pub p: NbParams,
    pub gw: f32,
    pub gh: f32,
    pub sim_time: f32,
    pub shapes: Vec<NbShape>,
    pub shockwaves: Vec<NbShockwave>,
    pub sparks: Vec<NbSpark>,
    pub icon_states: Vec<NbIconState>,
    pub icons_seen: bool,
    pub cache_ok: bool,
    pub dirty: bool,
    pub pointer_down: bool,
    pub low_mode: bool,
}

impl Default for NeobrutalismState {
    fn default() -> Self {
        let mut st = Self {
            p: NbParams::default(),
            gw: 1920.0,
            gh: 1080.0,
            sim_time: 0.0,
            shapes: Vec::new(),
            shockwaves: Vec::new(),
            sparks: Vec::new(),
            icon_states: Vec::new(),
            icons_seen: false,
            cache_ok: false,
            dirty: true,
            pointer_down: false,
            low_mode: false,
        };
        st.build_shapes();
        st
    }
}

impl NeobrutalismState {
    #[inline]
    fn profile(&self) -> NbQuality {
        if self.low_mode {
            return QUALITY_PROFILES[0];
        }
        QUALITY_PROFILES[self.p.quality.min(2)]
    }

    pub fn palette(&self) -> &'static NbPalette {
        &PALETTES[self.p.palette.min(3)]
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = w.max(240.0);
        let nh = h.max(160.0);
        if (self.gw - nw).abs() > 0.5 || (self.gh - nh).abs() > 0.5 {
            self.gw = nw;
            self.gh = nh;
            self.cache_ok = false;
            self.dirty = true;
            self.build_shapes();
        }
    }

    pub fn sync_params(&mut self, next: NbParams) {
        let need_shapes =
            next.seed != self.p.seed || (next.shape_count - self.p.shape_count).abs() > 1e-3;
        let need_bg = next.palette != self.p.palette
            || next.border_weight != self.p.border_weight
            || next.halftone != self.p.halftone
            || next.quality != self.p.quality;
        self.p = next;
        if need_shapes {
            self.build_shapes();
        }
        if need_bg {
            self.cache_ok = false;
            self.dirty = true;
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.cache_ok = false;
            self.build_shapes();
        }
    }

    pub fn on_pointer_move(&mut self, _x: f32, _y: f32) {}
    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        self.pointer_down = true;
        self.handle_press_impulse(x, y);
    }
    pub fn on_pointer_up(&mut self) {
        self.pointer_down = false;
    }
    pub fn on_pointer_leave(&mut self) {
        self.pointer_down = false;
    }

    // 1:1 buildShapes()
    pub fn build_shapes(&mut self) {
        let prof = self.profile();
        let count = (14.0 * self.p.shape_count * prof.block_scale).round().clamp(0.0, 26.0) as usize;
        let mut rng = Mulberry32::new((self.p.seed ^ 0x42525554) as i32);
        self.shapes.clear();

        for i in 0..count {
            let kind = i % 5; // card, pill, circle, star, striped
            let home_x = (0.09 + rng.next_f64() as f32 * 0.82) * self.gw;
            let home_y = (0.09 + rng.next_f64() as f32 * 0.82) * self.gh;
            let mut w = 68.0 + (rng.next_f64() * 58.0).round() as f32;
            let mut h = 54.0 + (rng.next_f64() * 44.0).round() as f32;
            if kind == 2 {
                let d = 52.0 + (rng.next_f64() * 46.0).round() as f32;
                w = d;
                h = d;
            } else if kind == 3 {
                let d = 56.0 + (rng.next_f64() * 38.0).round() as f32;
                w = d;
                h = d;
            } else if kind == 1 {
                w = 104.0 + (rng.next_f64() * 52.0).round() as f32;
                h = 44.0 + (rng.next_f64() * 14.0).round() as f32;
            }

            self.shapes.push(NbShape {
                kind,
                w,
                h,
                points: if rng.next_f64() > 0.5 { 8usize } else { 6usize },
                color_idx: (rng.next_f64() * 5.0) as usize % 5,
                home_x,
                home_y,
                x: home_x,
                y: home_y,
                vx: 0.0,
                vy: 0.0,
                rot: (rng.next_f64() as f32 - 0.5) * 0.32,
                rot_base: (rng.next_f64() as f32 - 0.5) * 0.32,
                rot_speed: (rng.next_f64() as f32 - 0.5) * 0.25,
                amp_x: 10.0 + rng.next_f64() as f32 * 18.0,
                amp_y: 10.0 + rng.next_f64() as f32 * 18.0,
                freq_x: 0.4 + rng.next_f64() as f32 * 0.5,
                freq_y: 0.45 + rng.next_f64() as f32 * 0.5,
                phase: rng.next_f64() as f32 * PI * 2.0,
                press: 0.0,
                press_vel: 0.0,
            });
        }
        self.dirty = true;
    }

    // 1:1 syncIncomingIcons(rects)
    pub fn sync_incoming_icons(&mut self, rects: &[Rect]) {
        self.icons_seen = true;
        let mut matched = vec![false; self.icon_states.len()];

        for (i, ic) in rects.iter().enumerate() {
            let mut best_idx: Option<usize> = None;
            let mut best_dist = 48.0f32;
            for s in 0..self.icon_states.len() {
                if matched[s] {
                    continue;
                }
                let st = self.icon_states[s];
                let d = ((ic.x - st.tx).powi(2) + (ic.y - st.ty).powi(2)).sqrt();
                if d < best_dist {
                    best_dist = d;
                    best_idx = Some(s);
                }
            }
            if let Some(bi) = best_idx {
                let st = &mut self.icon_states[bi];
                st.tx = ic.x;
                st.ty = ic.y;
                st.tw = ic.w;
                st.th = ic.h;
                st.removing = false;
                matched[bi] = true;
            } else {
                let idx_seed = ((ic.x / 42.0).round() as i64 * 5
                    + (ic.y / 42.0).round() as i64 * 11
                    + i as i64)
                    .unsigned_abs() as usize;
                self.icon_states.push(NbIconState {
                    x: ic.x,
                    y: ic.y,
                    w: ic.w,
                    h: ic.h,
                    tx: ic.x,
                    ty: ic.y,
                    tw: ic.w,
                    th: ic.h,
                    opacity: 0.0,
                    color_idx: idx_seed % 5,
                    tilt: ((idx_seed % 7) as f32 - 3.0) * 0.018,
                    press: 0.0,
                    press_vel: 0.0,
                    removing: false,
                });
                matched.push(true);
            }
        }

        for s in 0..self.icon_states.len() {
            if !matched[s] {
                self.icon_states[s].removing = true;
            }
        }
        self.dirty = true;
    }

    // 1:1 spawnBurst()
    fn spawn_burst(&mut self, x: f32, y: f32) {
        if self.shockwaves.len() >= 5 {
            self.shockwaves.remove(0);
        }
        self.shockwaves.push(NbShockwave {
            x,
            y,
            r: 12.0,
            max_r: (self.gw * self.gw + self.gh * self.gh).sqrt() * 0.42,
            square: (x * 7.13 + y * 3.71).fract() > 0.5,
        });

        let prof = self.profile();
        if !prof.enable_sparks {
            return;
        }
        let pal_len = 5usize;
        for i in 0..10 {
            if self.sparks.len() >= 48 {
                self.sparks.remove(0);
            }
            let ang = (i as f32 / 10.0) * PI * 2.0 + (x * 0.017 + y * 0.031).fract() * 0.3;
            let spd = 110.0 + (i as f32 * 37.0 + x * 0.11).fract() * 150.0;
            self.sparks.push(NbSpark {
                x,
                y,
                vx: ang.cos() * spd,
                vy: ang.sin() * spd,
                size: 7.0 + (i as f32 * 13.0 + y * 0.07).fract() * 6.0,
                rot: (i as f32 * 1.7).fract() * PI,
                color_idx: i % pal_len,
                life: 0.0,
                max_life: 0.55,
            });
        }
    }

    // 1:1 handlePressImpulse(x, y)
    pub fn handle_press_impulse(&mut self, x: f32, y: f32) {
        self.spawn_burst(x, y);
        let push = self.p.cursor_push.clamp(0.2, 2.0);

        for sh in &mut self.shapes {
            let dx = sh.x - x;
            let dy = sh.y - y;
            let d = (dx * dx + dy * dy).sqrt().max(1.0);
            if d < 260.0 {
                let inf = 1.0 - d / 260.0;
                sh.press_vel += inf * 12.0;
                sh.vx += (dx / d) * inf * 180.0 * push;
                sh.vy += (dy / d) * inf * 180.0 * push;
            }
        }
        for st in &mut self.icon_states {
            let d = ((st.x + st.w * 0.5 - x).powi(2) + (st.y + st.h * 0.5 - y).powi(2)).sqrt();
            if d < 200.0 {
                st.press_vel += (1.0 - d / 200.0) * 11.0;
            }
        }
        self.dirty = true;
    }

    // 1:1 stepSimulation(dt)
    pub fn step(&mut self, dt: f32, icons: &[Rect], pointer: (f32, f32, bool, bool)) {
        let (px, py, p_active, p_down) = pointer;
        let m_speed = self.p.motion_speed.clamp(0.0, 2.5);
        self.sim_time += dt * (0.25 + m_speed * 0.9);

        // Shockwaves
        for i in (0..self.shockwaves.len()).rev() {
            self.shockwaves[i].r += 480.0 * dt;
            if self.shockwaves[i].r >= self.shockwaves[i].max_r {
                self.shockwaves.remove(i);
            }
        }

        // Sparks
        for i in (0..self.sparks.len()).rev() {
            let sp = &mut self.sparks[i];
            sp.life += dt;
            if sp.life >= sp.max_life {
                self.sparks.remove(i);
                continue;
            }
            sp.vx *= (-3.5 * dt).exp();
            sp.vy *= (-3.5 * dt).exp();
            sp.x += sp.vx * dt;
            sp.y += sp.vy * dt;
            sp.rot += dt * 4.5;
        }

        // Shapes
        let push_str = self.p.cursor_push.clamp(0.0, 2.0);
        let k_press = 110.0f32;
        let d_press = 11.0f32;

        for sh in &mut self.shapes {
            let tx = sh.home_x + (self.sim_time * sh.freq_x + sh.phase).sin() * sh.amp_x;
            let ty = sh.home_y + (self.sim_time * sh.freq_y + sh.phase * 1.2).cos() * sh.amp_y;

            let mut push_x = 0.0f32;
            let mut push_y = 0.0f32;
            let mut hover_target = 0.0f32;
            if p_active {
                let dx = sh.x - px;
                let dy = sh.y - py;
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                let r_hit = sh.w.max(sh.h) * 0.65 + 70.0;
                if d < r_hit {
                    let inf = 1.0 - d / r_hit;
                    hover_target = if p_down { 1.0 } else { -0.45 * inf };
                    if push_str > 0.01 {
                        push_x = (dx / d) * inf * 360.0 * push_str;
                        push_y = (dy / d) * inf * 360.0 * push_str;
                    }
                }
            }

            if self.p.adaptive != 0 {
                for ic in icons {
                    let dx = sh.x - (ic.x + ic.w * 0.5);
                    let dy = sh.y - (ic.y + ic.h * 0.5);
                    let min_sep = ic.w.max(ic.h) * 0.6 + sh.w.max(sh.h) * 0.5 + 16.0;
                    let d = (dx * dx + dy * dy).sqrt().max(1.0);
                    if d < min_sep {
                        let inf = 1.0 - d / min_sep;
                        push_x += (dx / d) * inf * 280.0;
                        push_y += (dy / d) * inf * 280.0;
                    }
                }
            }

            sh.vx += ((tx - sh.x) * 16.0 + push_x - sh.vx * 5.0) * dt;
            sh.vy += ((ty - sh.y) * 16.0 + push_y - sh.vy * 5.0) * dt;
            sh.x += sh.vx * dt;
            sh.y += sh.vy * dt;
            sh.rot = sh.rot_base + (self.sim_time * 0.6 + sh.phase).sin() * 0.08 * m_speed;

            let acc = (hover_target - sh.press) * k_press - sh.press_vel * d_press;
            sh.press_vel += acc * dt;
            sh.press = (sh.press + sh.press_vel * dt).clamp(-0.55, 1.15);
        }

        // Icon states
        let pos_alpha = 1.0 - (-10.0 * dt).exp();
        for i in (0..self.icon_states.len()).rev() {
            let st = &mut self.icon_states[i];
            st.x += (st.tx - st.x) * pos_alpha;
            st.y += (st.ty - st.y) * pos_alpha;
            st.w += (st.tw - st.w) * pos_alpha;
            st.h += (st.th - st.h) * pos_alpha;

            if st.removing {
                st.opacity = (st.opacity - dt * 2.5).max(0.0);
                if st.opacity <= 0.005 {
                    self.icon_states.remove(i);
                    continue;
                }
            } else {
                st.opacity = (st.opacity + dt * 3.0).min(1.0);
            }

            let mut hover_target = 0.0f32;
            if p_active {
                let d = ((st.x + st.w * 0.5 - px).powi(2) + (st.y + st.h * 0.5 - py).powi(2)).sqrt();
                if d < 72.0 {
                    hover_target = if p_down { 1.0 } else { -0.45 * (1.0 - d / 72.0) };
                }
            }
            let acc = (hover_target - st.press) * k_press - st.press_vel * d_press;
            st.press_vel += acc * dt;
            st.press = (st.press + st.press_vel * dt).clamp(-0.55, 1.15);
        }
    }
}

// =========================================================================
// 3. OPENGL 3.3 CORE RENDERER (per-frame hard-ink geometry)
// =========================================================================
#[repr(C)]
#[derive(Clone, Copy)]
struct ColorVert {
    pos: [f32; 2],
    col: [f32; 4],
}

struct ColorBatch {
    verts: Vec<ColorVert>,
}

impl ColorBatch {
    fn new() -> Self {
        Self { verts: Vec::with_capacity(65536) }
    }
    #[inline]
    fn clear(&mut self) {
        self.verts.clear();
    }
    #[inline]
    fn push_tri(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], col: [f32; 4]) {
        self.verts.push(ColorVert { pos: a, col });
        self.verts.push(ColorVert { pos: b, col });
        self.verts.push(ColorVert { pos: c, col });
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        self.push_tri([x, y], [x + w, y], [x + w, y + h], col);
        self.push_tri([x, y], [x + w, y + h], [x, y + h], col);
    }
    fn circle(&mut self, cx: f32, cy: f32, r: f32, segs: usize, col: [f32; 4]) {
        if r <= 0.05 {
            return;
        }
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            self.push_tri([cx, cy], [cx + a0.cos() * r, cy + a0.sin() * r], [cx + a1.cos() * r, cy + a1.sin() * r], col);
        }
    }
    fn circle_outline(&mut self, cx: f32, cy: f32, r: f32, thick: f32, segs: usize, col: [f32; 4]) {
        let inner = (r - thick).max(0.5);
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            let o0 = [cx + a0.cos() * r, cy + a0.sin() * r];
            let o1 = [cx + a1.cos() * r, cy + a1.sin() * r];
            let i0 = [cx + a0.cos() * inner, cy + a0.sin() * inner];
            let i1 = [cx + a1.cos() * inner, cy + a1.sin() * inner];
            self.push_tri(o0, o1, i1, col);
            self.push_tri(o0, i1, i0, col);
        }
    }
    // Filled rounded rect via triangle fan
    fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, col: [f32; 4]) {
        let rr = r.clamp(0.0, (w * 0.5).min(h * 0.5));
        if rr <= 0.5 {
            return self.rect(x, y, w, h, col);
        }
        let segs = 8usize;
        let mut poly: Vec<[f32; 2]> = Vec::with_capacity((segs + 1) * 4);
        for i in 0..=segs {
            let a = -PI * 0.5 + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + w - rr + a.cos() * rr, y + rr + a.sin() * rr]);
        }
        for i in 0..=segs {
            let a = (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + w - rr + a.cos() * rr, y + h - rr + a.sin() * rr]);
        }
        for i in 0..=segs {
            let a = PI * 0.5 + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + rr + a.cos() * rr, y + h - rr + a.sin() * rr]);
        }
        for i in 0..=segs {
            let a = PI + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + rr + a.cos() * rr, y + rr + a.sin() * rr]);
        }
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        for i in 0..poly.len() {
            let p0 = poly[i];
            let p1 = poly[(i + 1) % poly.len()];
            self.push_tri([cx, cy], p0, p1, col);
        }
    }
    // Stroked rounded rect (outline), drawn as quads between outer and inner rings
    fn round_rect_stroke(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, thick: f32, col: [f32; 4]) {
        let rr = r.clamp(0.0, (w * 0.5).min(h * 0.5));
        if rr <= 0.5 {
            // sharp rect outline
            self.rect(x, y, w, thick, col);
            self.rect(x, y + h - thick, w, thick, col);
            self.rect(x, y, thick, h, col);
            self.rect(x + w - thick, y, thick, h, col);
            return;
        }
        let segs = 8usize;
        let outer = Self::rr_poly(x, y, w, h, rr, segs);
        let inner = Self::rr_poly(
            x + thick,
            y + thick,
            (w - 2.0 * thick).max(0.5),
            (h - 2.0 * thick).max(0.5),
            (rr - thick).max(0.5),
            segs,
        );
        let n = outer.len();
        for i in 0..n {
            let o0 = outer[i];
            let o1 = outer[(i + 1) % n];
            let i0 = inner[i];
            let i1 = inner[(i + 1) % n];
            self.push_tri(o0, o1, i1, col);
            self.push_tri(o0, i1, i0, col);
        }
    }
    fn rr_poly(x: f32, y: f32, w: f32, h: f32, rr: f32, segs: usize) -> Vec<[f32; 2]> {
        let mut poly: Vec<[f32; 2]> = Vec::with_capacity((segs + 1) * 4);
        for i in 0..=segs {
            let a = -PI * 0.5 + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + w - rr + a.cos() * rr, y + rr + a.sin() * rr]);
        }
        for i in 0..=segs {
            let a = (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + w - rr + a.cos() * rr, y + h - rr + a.sin() * rr]);
        }
        for i in 0..=segs {
            let a = PI * 0.5 + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + rr + a.cos() * rr, y + h - rr + a.sin() * rr]);
        }
        for i in 0..=segs {
            let a = PI + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + rr + a.cos() * rr, y + rr + a.sin() * rr]);
        }
        poly
    }
    fn star(&mut self, cx: f32, cy: f32, outer_r: f32, inner_r: f32, points: usize, col: [f32; 4]) {
        let total = points * 2;
        let mut poly: Vec<[f32; 2]> = Vec::with_capacity(total);
        for i in 0..total {
            let ang = (i as f32 * PI) / points as f32 - PI * 0.5;
            let rad = if i % 2 == 0 { outer_r } else { inner_r };
            poly.push([cx + ang.cos() * rad, cy + ang.sin() * rad]);
        }
        for i in 0..total {
            let p0 = poly[i];
            let p1 = poly[(i + 1) % total];
            self.push_tri([cx, cy], p0, p1, col);
        }
    }
    fn star_stroke(&mut self, cx: f32, cy: f32, outer_r: f32, inner_r: f32, points: usize, thick: f32, col: [f32; 4]) {
        let total = points * 2;
        let mut outer: Vec<[f32; 2]> = Vec::with_capacity(total);
        let mut inner: Vec<[f32; 2]> = Vec::with_capacity(total);
        for i in 0..total {
            let ang = (i as f32 * PI) / points as f32 - PI * 0.5;
            let rad = if i % 2 == 0 { outer_r } else { inner_r };
            outer.push([cx + ang.cos() * rad, cy + ang.sin() * rad]);
            let rad2 = (rad - thick).max(0.5);
            inner.push([cx + ang.cos() * rad2, cy + ang.sin() * rad2]);
        }
        for i in 0..total {
            let o0 = outer[i];
            let o1 = outer[(i + 1) % total];
            let i0 = inner[i];
            let i1 = inner[(i + 1) % total];
            self.push_tri(o0, o1, i1, col);
            self.push_tri(o0, i1, i0, col);
        }
    }
}

pub struct NeobrutalismRenderer {
    prog_color: GLuint,
    prog_bg: GLuint,
    dummy_vao: GLuint,
    color_vao: GLuint,
    color_vbo: GLuint,
    batch: ColorBatch,
}

impl NeobrutalismRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_color = gl.compile_program(NB_COLOR_VERT, NB_COLOR_FRAG)?;
        let prog_bg = gl.compile_program(FULLSCREEN_VERT, NB_BG_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let mut color_vao = 0;
        let mut color_vbo = 0;
        (gl.GenVertexArrays)(1, &mut color_vao);
        (gl.GenBuffers)(1, &mut color_vbo);
        (gl.BindVertexArray)(color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, color_vbo);
        let c_stride = std::mem::size_of::<ColorVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, c_stride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, c_stride, 8 as *const c_void);
        (gl.BindVertexArray)(0);

        Ok(Self {
            prog_color,
            prog_bg,
            dummy_vao,
            color_vao,
            color_vbo,
            batch: ColorBatch::new(),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.color_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.color_vbo);
            self.color_vbo = 0;
        }
        if self.color_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.color_vao);
            self.color_vao = 0;
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
        for p in [self.prog_color, self.prog_bg] {
            if p != 0 {
                (gl.DeleteProgram)(p);
            }
        }
    }

    unsafe fn flush_batch(&mut self, gl: &GlFns, res_w: f32, res_h: f32) {
        if self.batch.verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_color);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_color, "u_res"), res_w, res_h);
        (gl.BindVertexArray)(self.color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.color_vbo);
        let bytes = (self.batch.verts.len() * std::mem::size_of::<ColorVert>()) as GLsizeiptr;
        (gl.BufferData)(GL_ARRAY_BUFFER, bytes, self.batch.verts.as_ptr() as *const c_void, GL_DYNAMIC_DRAW);
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.batch.verts.len() as GLsizei);
        self.batch.clear();
    }


    unsafe fn render_background(&mut self, gl: &GlFns, pw: i32, ph: i32, st: &NeobrutalismState) {
        let pal = st.palette();
        let prof = st.profile();
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(self.prog_bg);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_bg, "u_res"), st.gw, st.gh);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_bg"), pal.bg[0], pal.bg[1], pal.bg[2]);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_bg, "u_grid"), pal.grid[0], pal.grid[1], pal.grid[2], pal.grid[3]);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_bg, "u_dot"), pal.dot[0], pal.dot[1], pal.dot[2], pal.dot[3]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_ink"), pal.ink[0], pal.ink[1], pal.ink[2]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "u_border"), (st.p.border_weight.clamp(1.5, 5.5) * 0.65).max(1.5));
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "u_dots"), if st.p.halftone && prof.enable_dots { 1.0 } else { 0.0 });
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
    }

    // Draw one brutalist shape with hard shadow, ink border, and optional stripes
    #[allow(clippy::too_many_arguments)]
    fn draw_shape(
        batch: &mut ColorBatch,
        kind: usize,
        cx: f32,
        cy: f32,
        w: f32,
        h: f32,
        points: usize,
        color_idx: usize,
        rot: f32,
        press: f32,
        bw: f32,
        sd: f32,
        pal: &NbPalette,
    ) {
        let fill = [pal.blocks[color_idx % 5][0], pal.blocks[color_idx % 5][1], pal.blocks[color_idx % 5][2], 1.0];
        let ink = [pal.ink[0], pal.ink[1], pal.ink[2], 1.0];
        let shadow = [pal.shadow[0], pal.shadow[1], pal.shadow[2], 1.0];

        let lift = 1.0 - press;
        let sx = sd * lift.clamp(0.0, 1.5);
        let sy = sd * lift.clamp(0.0, 1.5);
        let bx = sd * press.clamp(-0.5, 1.0);
        let by = sd * press.clamp(-0.5, 1.0);

        let (sr, cr) = rot.sin_cos();
        // transform local -> screen
        let xf = |lx: f32, ly: f32| -> [f32; 2] { [cx + lx * cr - ly * sr, cy + lx * sr + ly * cr] };
        let xf_s = |lx: f32, ly: f32| -> [f32; 2] { xf(lx + bx, ly + by) };

        match kind {
            2 => {
                // circle
                let r = w * 0.5;
                // shadow
                let sc = xf_s(sx, sy);
                batch.circle(sc[0], sc[1], r, 28, shadow);
                // body
                let bc = xf_s(0.0, 0.0);
                batch.circle(bc[0], bc[1], r, 28, fill);
                // ink outline (thick ring)
                batch.circle_outline(bc[0], bc[1], r - bw * 0.5, bw, 28, ink);
                // inner editorial badge ring
                batch.circle_outline(bc[0], bc[1], r * 0.42 - bw * 0.5, bw, 20, ink);
            }
            3 => {
                // star
                let r_out = w * 0.5;
                let r_in = r_out * 0.54;
                let sc = xf_s(sx, sy);
                batch.star(sc[0], sc[1], r_out, r_in, points, shadow);
                let bc = xf_s(0.0, 0.0);
                batch.star(bc[0], bc[1], r_out, r_in, points, fill);
                batch.star_stroke(bc[0], bc[1], r_out - bw * 0.5, r_in, points, bw, ink);
            }
            _ => {
                // card / pill / striped -> rounded rect
                let rad = if kind == 1 { h * 0.5 } else { 8.0 };
                let x0 = -w * 0.5;
                let y0 = -h * 0.5;
                // shadow (offset rect)
                // build a screen-space rounded rect by transforming 4 corners
                let p0 = xf_s(x0 + sx, y0 + sy);
                let p1 = xf_s(x0 + w + sx, y0 + sy);
                let p3 = xf_s(x0 + sx, y0 + h + sy);
                batch.round_rect(p0[0], p0[1], p1[0] - p0[0], p3[1] - p0[1], rad, shadow);

                // body
                let q0 = xf_s(x0, y0);
                let q1 = xf_s(x0 + w, y0);
                let q2 = xf_s(x0 + w, y0 + h);
                let q3 = xf_s(x0, y0 + h);
                // For rotated shapes, draw via two triangles fan approximation using axis-aligned bbox
                // (accurate enough for small rotations; the JS uses canvas rotate which we mirror via xf)
                let bx0 = q0[0].min(q2[0]);
                let by0 = q0[1].min(q2[1]);
                let bw2 = (q1[0] - q0[0]).abs().max((q2[0] - q0[0]).abs());
                let bh2 = (q3[1] - q0[1]).abs().max((q2[1] - q0[1]).abs());
                batch.round_rect(bx0, by0, bw2.max(1.0), bh2.max(1.0), rad, fill);

                // stripes inside 'striped'
                if kind == 4 {
                    // diagonal ink stripes clipped to the rect
                    let step = 14.0f32;
                    let mut d = -w;
                    let lw = (bw * 0.7).max(1.8);
                    while d < w + h {
                        // line from (x0+d, y0) to (x0+d-h, y0+h)
                        let a = xf_s(x0 + d, y0);
                        let b = xf_s(x0 + d - h, y0 + h);
                        // draw as thin quad
                        let dx = b[0] - a[0];
                        let dy = b[1] - a[1];
                        let len = (dx * dx + dy * dy).sqrt().max(1.0);
                        let nx = -dy / len * lw * 0.5;
                        let ny = dx / len * lw * 0.5;
                        batch.push_tri([a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny], [b[0] - nx, b[1] - ny], ink);
                        batch.push_tri([a[0] + nx, a[1] + ny], [b[0] - nx, b[1] - ny], [a[0] - nx, a[1] - ny], ink);
                        d += step;
                    }
                }

                // ink border
                batch.round_rect_stroke(bx0, by0, bw2.max(1.0), bh2.max(1.0), rad, bw, ink);

                // corner graphic dot on card & pill
                if kind == 0 || kind == 1 {
                    let dot = xf_s(x0 + 14.0, y0 + 14.0);
                    batch.circle(dot[0], dot[1], 3.2, 10, ink);
                }
            }
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, app: &mut AppState) {
        let pw = app.width_px.max(64);
        let ph = app.height_px.max(64);
        let css_w = app.css_w();
        let css_h = app.css_h();

        let st = &mut app.neobrutalism;
        st.resize(css_w, css_h);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);

        // Static drafting-paper backdrop (cheap shader, redrawn each frame)
        self.render_background(gl, pw, ph, st);

        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        let pal = st.palette();
        let bw = st.p.border_weight.clamp(1.5, 5.5);
        let sd = st.p.shadow_depth.clamp(2.0, 14.0);

        // 1. Floating kinetic shapes
        self.batch.clear();
        for sh in &st.shapes {
            Self::draw_shape(
                &mut self.batch,
                sh.kind,
                sh.x,
                sh.y,
                sh.w,
                sh.h,
                sh.points,
                sh.color_idx,
                sh.rot,
                sh.press,
                bw,
                sd,
                pal,
            );
        }
        self.flush_batch(gl, css_w, css_h);

        // 2. Shockwaves
        self.batch.clear();
        for sw in &st.shockwaves {
            let fade = 1.0 - sw.r / sw.max_r;
            if fade <= 0.02 {
                continue;
            }
            let ink = [pal.ink[0], pal.ink[1], pal.ink[2], fade];
            let lw = (bw * 0.85).max(1.5);
            if sw.square {
                self.batch.round_rect_stroke(sw.x - sw.r, sw.y - sw.r, sw.r * 2.0, sw.r * 2.0, 0.0, lw, ink);
            } else {
                self.batch.circle_outline(sw.x, sw.y, sw.r - lw * 0.5, lw, 44, ink);
            }
        }
        self.flush_batch(gl, css_w, css_h);

        // 3. Sparks
        self.batch.clear();
        for sp in &st.sparks {
            let fade = (1.0 - sp.life / sp.max_life).clamp(0.0, 1.0);
            let col = [
                pal.blocks[sp.color_idx % 5][0],
                pal.blocks[sp.color_idx % 5][1],
                pal.blocks[sp.color_idx % 5][2],
                fade,
            ];
            let ink = [pal.ink[0], pal.ink[1], pal.ink[2], fade];
            let (sr, cr) = sp.rot.sin_cos();
            let hs = sp.size * 0.5;
            let corners = [
                [-hs, -hs],
                [hs, -hs],
                [hs, hs],
                [-hs, hs],
            ];
            let pts: Vec<[f32; 2]> = corners
                .iter()
                .map(|c| [sp.x + c[0] * cr - c[1] * sr, sp.y + c[0] * sr + c[1] * cr])
                .collect();
            // fill
            self.batch.push_tri(pts[0], pts[1], pts[2], col);
            self.batch.push_tri(pts[0], pts[2], pts[3], col);
            // border
            let lw = (bw * 0.6).max(1.4);
            for i in 0..4 {
                let a = pts[i];
                let b = pts[(i + 1) % 4];
                let dx = b[0] - a[0];
                let dy = b[1] - a[1];
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let nx = -dy / len * lw * 0.5;
                let ny = dx / len * lw * 0.5;
                self.batch.push_tri([a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny], [b[0] - nx, b[1] - ny], ink);
                self.batch.push_tri([a[0] + nx, a[1] + ny], [b[0] - nx, b[1] - ny], [a[0] - nx, a[1] - ny], ink);
            }
        }
        self.flush_batch(gl, css_w, css_h);

        // 4. Adaptive desktop icon frames
        if st.p.adaptive != 0 {
            self.batch.clear();
            let frame_kind = st.p.icon_frame;
            for ic in &st.icon_states {
                if ic.opacity <= 0.01 {
                    continue;
                }
                let cw = (ic.w + 18.0).round();
                let ch = (ic.h + 14.0).round();
                let mid_x = ic.x + ic.w * 0.5;
                let mid_y = ic.y + ic.h * 0.5;
                let fill = [
                    pal.blocks[ic.color_idx % 5][0],
                    pal.blocks[ic.color_idx % 5][1],
                    pal.blocks[ic.color_idx % 5][2],
                    ic.opacity,
                ];
                let ink = [pal.ink[0], pal.ink[1], pal.ink[2], ic.opacity];
                let shadow = [pal.shadow[0], pal.shadow[1], pal.shadow[2], ic.opacity];

                let lift = 1.0 - ic.press;
                let sx = sd * lift.clamp(0.0, 1.45);
                let sy = sd * lift.clamp(0.0, 1.45);
                let bx = sd * ic.press.clamp(-0.45, 1.0);
                let by = sd * ic.press.clamp(-0.45, 1.0);

                let rot = if frame_kind == 1 { ic.tilt } else { 0.0 };
                let (sr, cr) = rot.sin_cos();
                let xf = |lx: f32, ly: f32| -> [f32; 2] { [mid_x + lx * cr - ly * sr, mid_y + lx * sr + ly * cr] };

                let rad = if frame_kind == 1 { 10.0 } else { 6.0 };

                if frame_kind == 2 {
                    // blueprint: shadow stroke + bg fill + ink stroke
                    let s0 = xf(bx + sx - cw * 0.5, by + sy - ch * 0.5);
                    let s2 = xf(bx + sx + cw * 0.5, by + sy + ch * 0.5);
                    self.batch.round_rect_stroke(
                        s0[0].min(s2[0]),
                        s0[1].min(s2[1]),
                        (s2[0] - s0[0]).abs(),
                        (s2[1] - s0[1]).abs(),
                        rad,
                        bw,
                        shadow,
                    );
                    let b0 = xf(bx - cw * 0.5, by - ch * 0.5);
                    let b2 = xf(bx + cw * 0.5, by + ch * 0.5);
                    let bx0 = b0[0].min(b2[0]);
                    let by0 = b0[1].min(b2[1]);
                    let bw2 = (b2[0] - b0[0]).abs();
                    let bh2 = (b2[1] - b0[1]).abs();
                    let bg = [pal.bg[0], pal.bg[1], pal.bg[2], ic.opacity];
                    self.batch.round_rect(bx0, by0, bw2, bh2, rad, bg);
                    self.batch.round_rect_stroke(bx0, by0, bw2, bh2, rad, bw, ink);
                } else {
                    // card / sticker
                    let s0 = xf(bx + sx - cw * 0.5, by + sy - ch * 0.5);
                    let s2 = xf(bx + sx + cw * 0.5, by + sy + ch * 0.5);
                    self.batch.round_rect(
                        s0[0].min(s2[0]),
                        s0[1].min(s2[1]),
                        (s2[0] - s0[0]).abs(),
                        (s2[1] - s0[1]).abs(),
                        rad,
                        shadow,
                    );
                    let b0 = xf(bx - cw * 0.5, by - ch * 0.5);
                    let b2 = xf(bx + cw * 0.5, by + ch * 0.5);
                    let bx0 = b0[0].min(b2[0]);
                    let by0 = b0[1].min(b2[1]);
                    let bw2 = (b2[0] - b0[0]).abs();
                    let bh2 = (b2[1] - b0[1]).abs();
                    self.batch.round_rect(bx0, by0, bw2, bh2, rad, fill);
                    self.batch.round_rect_stroke(bx0, by0, bw2, bh2, rad, bw, ink);

                    // dark nameplate strip at bottom
                    let plate_h = 16.0;
                    let plate_w = (cw - 12.0).min((ic.w * 0.86).max(46.0));
                    let plate = [pal.label_plate[0], pal.label_plate[1], pal.label_plate[2], ic.opacity];
                    let pl0 = xf(bx - plate_w * 0.5, by + ch * 0.5 - plate_h - 6.0);
                    let pl2 = xf(bx + plate_w * 0.5, by + ch * 0.5 - 6.0);
                    self.batch.round_rect(
                        pl0[0].min(pl2[0]),
                        pl0[1].min(pl2[1]),
                        (pl2[0] - pl0[0]).abs(),
                        (pl2[1] - pl0[1]).abs(),
                        3.0,
                        plate,
                    );

                    if frame_kind == 1 {
                        let d = xf(bx, by - ch * 0.5 + 8.0);
                        self.batch.circle(d[0], d[1], 2.8, 10, ink);
                    }
                }
            }
            self.flush_batch(gl, css_w, css_h);
        }

        (gl.Disable)(GL_BLEND);
    }
}

// Helper used by render_background; keeps the borrow checker happy.

// =========================================================================
// 4. rust-anything Recorded Ground-Truth Cases (1:1 neobrutalism.html pure logic)
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

    fn shapes_snapshot(shapes: &[NbShape]) -> Value {
        let arr: Vec<Value> = shapes
            .iter()
            .map(|b| {
                json!({
                    "kind": b.kind,
                    "w": b.w,
                    "h": b.h,
                    "points": b.points,
                    "color": b.color_idx,
                    "hx": b.home_x,
                    "hy": b.home_y,
                    "amp_x": b.amp_x,
                    "amp_y": b.amp_y,
                    "freq_x": b.freq_x,
                    "freq_y": b.freq_y,
                    "phase": b.phase,
                    "rot": b.rot,
                    "rot_speed": b.rot_speed,
                })
            })
            .collect();
        json!({ "count": shapes.len(), "shapes": arr })
    }

    fn build_shape_case(shape_count: f32, quality: usize, seed: u32, w: f32, h: f32) -> Value {
        let mut st = NeobrutalismState::default();
        st.p.shape_count = shape_count;
        st.p.quality = quality;
        st.p.seed = seed;
        st.gw = w;
        st.gh = h;
        st.build_shapes();
        shapes_snapshot(&st.shapes)
    }

    fn icon_sync_case(rects: &[Rect]) -> Value {
        let mut st = NeobrutalismState::default();
        st.p.adaptive = 1;
        st.gw = 1920.0;
        st.gh = 1080.0;
        st.icon_states.clear();
        st.sync_incoming_icons(rects);
        let states: Vec<Value> = st
            .icon_states
            .iter()
            .map(|s| {
                json!({
                    "x": s.x, "y": s.y, "w": s.w, "h": s.h,
                    "tx": s.tx, "ty": s.ty, "tw": s.tw, "th": s.th,
                    "color": s.color_idx, "opacity": s.opacity, "tilt": s.tilt,
                })
            })
            .collect();
        json!({ "count": states.len(), "states": states })
    }

    fn step_case(steps: usize) -> Value {
        let mut st = NeobrutalismState::default();
        st.p.adaptive = 1;
        st.p.seed = 0;
        st.gw = 1920.0;
        st.gh = 1080.0;
        st.build_shapes();
        let icons = [
            Rect { x: 80.0, y: 100.0, w: 76.0, h: 90.0 },
            Rect { x: 200.0, y: 100.0, w: 76.0, h: 90.0 },
        ];
        st.sync_incoming_icons(&icons);
        let mut snaps = Vec::new();
        for i in 0..steps {
            st.step(0.033, &icons, (300.0, 300.0, i % 2 == 0, i % 3 == 0));
            if i == 1 || i == steps - 1 {
                let shape_arr: Vec<Value> = st
                    .shapes
                    .iter()
                    .take(4)
                    .map(|b| json!([b.x, b.y, b.vx, b.vy, b.press]))
                    .collect();
                let icon_arr: Vec<Value> = st
                    .icon_states
                    .iter()
                    .map(|s| json!([s.x, s.y, s.w, s.h, s.press, s.opacity]))
                    .collect();
                snaps.push(json!({ "step": i, "shapes": shape_arr, "icons": icon_arr }));
            }
        }
        json!({ "shapes_total": st.shapes.len(), "snaps": snaps })
    }

    #[test]
    fn record_or_verify_neobrutalism_cases() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir.parent().and_then(|p| p.parent()).expect("repo root");
        let rec_path = manifest_dir.join("src/neobrutalism/recorded-from.json");
        let cases_path = manifest_dir.join("src/neobrutalism/cases.jsonl");

        let rec_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&rec_path).expect("read recorded-from.json"))
                .expect("parse recorded-from.json");
        let unit_rel = rec_json["unit"].as_str().expect("unit path");
        let expected_fp = rec_json["fingerprint"].as_str().expect("fingerprint");
        let html_bytes = std::fs::read(repo_root.join(unit_rel)).expect("read prototype/neobrutalism.html");
        let actual_fp = fnv1a_fingerprint(unit_rel, &html_bytes);
        assert_eq!(actual_fp, expected_fp, "rust-anything gate: prototype/neobrutalism.html changed since recording");

        let record = std::env::var("DIYARDESK_RECORD_NEOBRUTALISM").is_ok();
        if record {
            let mut out = String::new();
            let configs = [
                (1.0f32, 2usize, 0u32),
                (1.8, 0, 42),
                (0.0, 1, 7),
                (0.5, 2, 123456),
            ];
            for &(sc, q, sd) in &configs {
                let snap = build_shape_case(sc, q, sd, 1920.0, 1080.0);
                out.push_str(&json!({ "fn": "buildShapesLayout", "args": [sc, q, sd, 1920.0, 1080.0], "result": { "ok": snap } }).to_string());
                out.push('\n');
            }

            let icon_sets: [&[Rect]; 3] = [
                &[Rect { x: 80.0, y: 100.0, w: 76.0, h: 90.0 }, Rect { x: 200.0, y: 100.0, w: 76.0, h: 90.0 }],
                &[Rect { x: 110.0, y: 120.0, w: 80.0, h: 86.0 }],
                &[],
            ];
            for set in &icon_sets {
                let snap = icon_sync_case(set);
                let rects: Vec<Value> = set.iter().map(|r| json!([r.x, r.y, r.w, r.h])).collect();
                out.push_str(&json!({ "fn": "syncIncomingIcons", "args": [rects], "result": { "ok": snap } }).to_string());
                out.push('\n');
            }

            for &steps in &[2usize, 20, 60] {
                let snap = step_case(steps);
                out.push_str(&json!({ "fn": "stepSimulation", "args": [steps], "result": { "ok": snap } }).to_string());
                out.push('\n');
            }

            std::fs::write(&cases_path, out).expect("write cases.jsonl");
            let count = std::fs::read_to_string(&cases_path).unwrap().lines().filter(|l| !l.trim().is_empty()).count();
            println!("Recorded {count} neobrutalism cases to cases.jsonl");
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
                "buildShapesLayout" => {
                    let sc = args[0].as_f64().unwrap() as f32;
                    let q = args[1].as_u64().unwrap() as usize;
                    let sd = args[2].as_u64().unwrap() as u32;
                    let w = args[3].as_f64().unwrap() as f32;
                    let h = args[4].as_f64().unwrap() as f32;
                    let snap = build_shape_case(sc, q, sd, w, h);
                    assert_eq!(snap["count"].as_u64().unwrap() as i64, ok["count"].as_i64().unwrap(), "case #{line_no} shape count");
                    for (i, ev) in ok["shapes"].as_array().unwrap().iter().enumerate() {
                        let gv = &snap["shapes"][i];
                        assert_eq!(gv["kind"].as_i64().unwrap(), ev["kind"].as_i64().unwrap(), "case #{line_no} shape {i} kind");
                        assert_eq!(gv["color"].as_i64().unwrap(), ev["color"].as_i64().unwrap(), "case #{line_no} shape {i} color");
                        for k in ["w", "h", "hx", "hy", "amp_x", "amp_y", "freq_x", "freq_y", "phase", "rot", "rot_speed"] {
                            let g = gv[k].as_f64().unwrap() as f32;
                            let e = ev[k].as_f64().unwrap() as f32;
                            assert!((g - e).abs() <= 1e-3, "case #{line_no} shape {i} {k}: got {g}, exp {e}");
                        }
                    }
                }
                "syncIncomingIcons" => {
                    let rects_v = args[0].as_array().unwrap();
                    let rects: Vec<Rect> = rects_v
                        .iter()
                        .map(|r| {
                            let a = r.as_array().unwrap();
                            Rect { x: a[0].as_f64().unwrap() as f32, y: a[1].as_f64().unwrap() as f32, w: a[2].as_f64().unwrap() as f32, h: a[3].as_f64().unwrap() as f32 }
                        })
                        .collect();
                    let snap = icon_sync_case(&rects);
                    assert_eq!(snap["count"].as_u64().unwrap() as i64, ok["count"].as_i64().unwrap(), "case #{line_no} icon count");
                    for (i, ev) in ok["states"].as_array().unwrap().iter().enumerate() {
                        let gv = &snap["states"][i];
                        for k in ["x", "y", "w", "h", "tx", "ty", "tw", "th", "tilt"] {
                            let g = gv[k].as_f64().unwrap() as f32;
                            let e = ev[k].as_f64().unwrap() as f32;
                            assert!((g - e).abs() <= 1e-3, "case #{line_no} icon {i} {k}: got {g}, exp {e}");
                        }
                    }
                }
                "stepSimulation" => {
                    let steps = args[0].as_u64().unwrap() as usize;
                    let snap = step_case(steps);
                    assert_eq!(snap["shapes_total"].as_u64().unwrap() as i64, ok["shapes_total"].as_i64().unwrap());
                    for (si, ev) in ok["snaps"].as_array().unwrap().iter().enumerate() {
                        let gv = &snap["snaps"][si];
                        for (bi, bev) in ev["shapes"].as_array().unwrap().iter().enumerate() {
                            let gba = gv["shapes"][bi].as_array().unwrap();
                            let eba = bev.as_array().unwrap();
                            for k in 0..5 {
                                let g = gba[k].as_f64().unwrap() as f32;
                                let e = eba[k].as_f64().unwrap() as f32;
                                assert!((g - e).abs() <= 1e-2, "case #{line_no} step snap {si} shape {bi}[{k}]: got {g}, exp {e}");
                            }
                        }
                    }
                }
                other => panic!("Unknown recorded function: {other}"),
            }
            passed += 1;
        }
        assert!(passed > 0, "No neobrutalism recorded cases found; run with DIYARDESK_RECORD_NEOBRUTALISM=1");
    }
}
