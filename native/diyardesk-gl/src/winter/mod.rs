pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use shaders::*;
use std::f32::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// Deterministic Mulberry32 PRNG (Bit-for-bit identical to JS mulberry32)
// =========================================================================
#[derive(Clone, Copy)]
pub struct Mulberry32 {
    a: i32,
}

impl Mulberry32 {
    pub fn new(seed: i32) -> Self {
        Self { a: seed }
    }

    pub fn next_f64(&mut self) -> f64 {
        self.a = self.a.wrapping_add(0x6D2B79F5u32 as i32);
        let a_u = self.a as u32;
        let mut t = (a_u ^ (a_u >> 15)).wrapping_mul(1 | a_u);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }
}

#[inline]
pub fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
#[allow(dead_code)]
pub fn smoothstep_f32(e0: f32, e1: f32, x: f32) -> f32 {
    let denom = if (e1 - e0).abs() < 1e-9 { 1.0 } else { e1 - e0 };
    let t = clamp_f32((x - e0) / denom, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// FNV-1a string hash (identical to winter.html hashStr, which folds UTF-16 code units)
pub fn hash_str(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for unit in s.encode_utf16() {
        h ^= unit as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

#[inline]
fn hex_rgba(hex: u32, a: f32) -> [f32; 4] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        a.clamp(0.0, 1.0),
    ]
}

// =========================================================================
// Settings & Quality Profiles (1:1 winter.html)
// =========================================================================
#[derive(Clone, Copy)]
pub struct QualityProfile {
    pub snow_scale: f32,
    pub smoke_scale: f32,
    pub tree_scale: f32,
    pub soft_glow: bool,
}

pub const QUALITY_LOW: QualityProfile = QualityProfile {
    snow_scale: 0.38,
    smoke_scale: 0.45,
    tree_scale: 0.65,
    soft_glow: false,
};

pub const QUALITY_MEDIUM: QualityProfile = QualityProfile {
    snow_scale: 0.70,
    smoke_scale: 0.75,
    tree_scale: 0.85,
    soft_glow: true,
};

pub const QUALITY_HIGH: QualityProfile = QualityProfile {
    snow_scale: 1.00,
    smoke_scale: 1.00,
    tree_scale: 1.00,
    soft_glow: true,
};

pub const SMOKE_MAX: usize = 72;
pub const SNOW_MAX: usize = 260;
pub const TRAIL_MAX: usize = 64;
pub const BURST_MAX: usize = 96;
pub const DRIFT_POINTS: usize = 14;

#[derive(Clone, Copy)]
pub struct WinterParams {
    pub adaptive: i32,
    pub snow_intensity: f32,
    pub snow_speed: f32,
    pub wind_strength: f32,
    pub smoke_amount: f32,
    pub atmosphere_density: f32,
    pub tree_density: f32,
    pub mountain_visibility: f32,
    pub icon_snow_amount: f32,
    pub anim_speed: f32,
    pub quality: i32, // 0 Low, 1 Medium, 2 High
    pub seed: u32,
}

impl Default for WinterParams {
    fn default() -> Self {
        Self {
            adaptive: 1,
            snow_intensity: 0.75,
            snow_speed: 0.85,
            wind_strength: 0.18,
            smoke_amount: 0.85,
            atmosphere_density: 0.75,
            tree_density: 0.80,
            mountain_visibility: 0.85,
            icon_snow_amount: 0.90,
            anim_speed: 1.0,
            quality: 2,
            seed: 20261117,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Star {
    pub nx: f32,
    pub ny: f32,
    pub r: f32,
    pub a: f32,
    pub tw: f32,
    pub ph: f32,
}

#[derive(Clone, Copy)]
pub struct Pine {
    pub x: f32,
    pub y: f32,
    pub h: f32,
    pub depth: f32,
    pub seed: u32,
}

#[derive(Clone, Copy)]
pub struct HouseAnchor {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

impl Default for HouseAnchor {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale: 1.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SmokePuff {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub r: f32,
    pub max_r: f32,
    pub age: f32,
    pub life: f32,
    pub phase: f32,
    pub alpha: f32,
}

impl Default for SmokePuff {
    fn default() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            r: 4.0,
            max_r: 26.0,
            age: 0.0,
            life: 4.0,
            phase: 0.0,
            alpha: 0.25,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Flake {
    pub x: f32,
    pub y: f32,
    pub layer: i32,
    pub r: f32,
    pub vy: f32,
    pub alpha: f32,
    pub phase: f32,
    pub sway: f32,
    pub vx_extra: f32,
}

impl Default for Flake {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            layer: 0,
            r: 1.0,
            vy: 20.0,
            alpha: 0.4,
            phase: 0.0,
            sway: 6.0,
            vx_extra: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Trail {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub rx: f32,
    pub ry: f32,
    pub age: f32,
    pub life: f32,
}

impl Default for Trail {
    fn default() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            rx: 22.0,
            ry: 8.0,
            age: 0.0,
            life: 2.2,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Burst {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub r: f32,
    pub age: f32,
    pub life: f32,
}

impl Default for Burst {
    fn default() -> Self {
        Self {
            active: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            r: 1.5,
            age: 0.0,
            life: 0.7,
        }
    }
}

#[derive(Clone)]
pub struct IconState {
    pub key: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub tx: f32,
    pub ty: f32,
    pub tw: f32,
    pub th: f32,
    pub vx: f32,
    pub vy: f32,
    pub speed: f32,
    pub last_trail_x: f32,
    pub last_trail_y: f32,
    pub fade: f32,
    pub snow_build: f32,
    pub cap_build: f32,
    pub removing: bool,
    pub is_folder: bool,
    pub drift: [(f32, f32); DRIFT_POINTS],
    pub cap_jitter: f32,
}

pub struct WinterState {
    pub p: WinterParams,
    pub w: f32,
    pub h: f32,
    pub low_mode: bool,
    pub sim_time: f32,
    pub seed: u32,
    pub horizon_y: f32,
    pub house: HouseAnchor,
    pub stars: Vec<Star>,
    pub trees: Vec<Pine>,
    pub fg_pines: Vec<Pine>,
    pub icons: Vec<IconState>,
    pub icons_seen: bool,
    pub moving_icon_count: usize,
    pub smoke: Vec<SmokePuff>,
    pub smoke_spawn_acc: f32,
    pub snow: Vec<Flake>,
    pub trails: Vec<Trail>,
    pub bursts: Vec<Burst>,
    pub cache_dirty: bool,
    pub rng: Mulberry32,
}

impl Default for WinterState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0, WinterParams::default(), 20261117)
    }
}

impl WinterState {
    pub fn new(w: f32, h: f32, params: WinterParams, seed: u32) -> Self {
        let mut st = Self {
            p: params,
            w: w.max(240.0),
            h: h.max(160.0),
            low_mode: false,
            sim_time: 0.0,
            seed: if seed == 0 { 20261117 } else { seed },
            horizon_y: 0.0,
            house: HouseAnchor::default(),
            stars: Vec::new(),
            trees: Vec::new(),
            fg_pines: Vec::new(),
            icons: Vec::new(),
            icons_seen: false,
            moving_icon_count: 0,
            smoke: vec![SmokePuff::default(); SMOKE_MAX],
            smoke_spawn_acc: 0.0,
            snow: vec![Flake::default(); SNOW_MAX],
            trails: vec![Trail::default(); TRAIL_MAX],
            bursts: vec![Burst::default(); BURST_MAX],
            cache_dirty: true,
            rng: Mulberry32::new(0x51A7C3),
        };
        st.build_stars();
        st.resize(w, h);
        st
    }

    pub fn profile(&self) -> QualityProfile {
        if self.low_mode {
            return QUALITY_LOW;
        }
        match self.p.quality {
            0 => QUALITY_LOW,
            1 => QUALITY_MEDIUM,
            _ => QUALITY_HIGH,
        }
    }

    pub fn set_low_mode(&mut self, v: bool) {
        if v != self.low_mode {
            self.low_mode = v;
            self.cache_dirty = true;
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let prev_w = self.w;
        let prev_h = self.h;
        self.w = w.max(240.0);
        self.h = h.max(160.0);
        if (prev_w - self.w).abs() > 0.5 || (prev_h - self.h).abs() > 0.5 {
            self.horizon_y = clamp_f32(self.h * 0.48, 150.0, (self.h - 180.0).max(150.0));
            self.pick_house_placement();
            self.cache_dirty = true;
            self.init_snow();
        } else if self.horizon_y <= 0.0 {
            self.horizon_y = clamp_f32(self.h * 0.48, 150.0, (self.h - 180.0).max(150.0));
            self.pick_house_placement();
            self.cache_dirty = true;
            self.init_snow();
        }
    }

    pub fn sync_params(&mut self, next: WinterParams) {
        let seed_changed = next.seed != self.p.seed && next.seed != 0;
        let quality_changed = next.quality != self.p.quality;
        let static_changed = (next.atmosphere_density - self.p.atmosphere_density).abs() > 1e-4
            || (next.tree_density - self.p.tree_density).abs() > 1e-4
            || (next.mountain_visibility - self.p.mountain_visibility).abs() > 1e-4
            || next.adaptive != self.p.adaptive;
        self.p = next;
        if seed_changed {
            self.seed = if self.p.seed == 0 { 20261117 } else { self.p.seed };
            self.build_stars();
            self.cache_dirty = true;
        }
        if static_changed {
            self.cache_dirty = true;
        }
        if quality_changed {
            self.init_snow();
            self.cache_dirty = true;
        }
    }

    // =====================================================================
    // Static scenery: stars, house placement, forest layout
    // =====================================================================
    pub fn build_stars(&mut self) {
        self.stars.clear();
        let mut rng = Mulberry32::new((self.seed ^ 0x91A2B3) as i32);
        for _ in 0..48 {
            self.stars.push(Star {
                nx: rng.next_f32(),
                ny: 0.03 + rng.next_f32() * 0.42,
                r: 0.55 + rng.next_f32() * 1.1,
                a: 0.14 + rng.next_f32() * 0.36,
                tw: 0.5 + rng.next_f32() * 1.4,
                ph: rng.next_f32() * TAU,
            });
        }
    }

    pub fn pick_house_placement(&mut self) {
        let candidates = [0.76f32, 0.28, 0.54, 0.84, 0.18];
        let base_h = clamp_f32(self.w.min(self.h) * 0.125, 74.0, 140.0);
        let hy = self.horizon_y + (34.0f32).min(self.h * 0.03);
        let mut best_x = self.w * 0.76;
        let mut best_score = -1e9f32;
        for (c, cand) in candidates.iter().enumerate() {
            let cx = self.w * cand;
            let mut min_dist = 1e9f32;
            for ic in &self.icons {
                if ic.removing {
                    continue;
                }
                let d = ((ic.x + ic.w * 0.5) - cx).hypot((ic.y + ic.h * 0.5) - hy);
                if d < min_dist {
                    min_dist = d;
                }
            }
            let score = min_dist.min(280.0) - if c == 0 { 0.0 } else { 12.0 * c as f32 };
            if score > best_score {
                best_score = score;
                best_x = cx;
            }
        }
        self.house.x = clamp_f32(best_x, 110.0, (self.w - 110.0).max(110.0));
        self.house.y = hy;
        self.house.scale = base_h / 84.0;
    }

    pub fn build_forest_layout(&mut self) {
        self.trees.clear();
        self.fg_pines.clear();
        let prof = self.profile();
        let density = clamp_f32(self.p.tree_density, 0.0, 1.5) * prof.tree_scale;
        if density <= 0.02 {
            return;
        }
        let mut rng = Mulberry32::new((self.seed ^ 0x4F2A11) as i32);
        let w = self.w as f64;
        let h = self.h as f64;
        let horizon = self.horizon_y as f64;
        let house_x = self.house.x as f64;
        let house_y = self.house.y as f64;
        let house_scale = self.house.scale as f64;
        let count = ((w / 44.0) * density as f64).clamp(0.0, 68.0).round() as usize;
        for i in 0..count {
            let depth = rng.next_f64();
            let x = (i as f64 / (count.max(2) - 1) as f64) * (w + 40.0) - 20.0
                + (rng.next_f64() - 0.5) * 34.0;
            let y = horizon
                + (-12.0 + (26.0f64.max(h * 0.06) + 12.0) * depth)
                + (rng.next_f64() - 0.5) * 14.0;
            if (x - house_x).abs() < 58.0 * house_scale && (y - house_y).abs() < 42.0 {
                continue;
            }
            let th = (24.0 + (62.0 - 24.0) * depth) * (0.82 + rng.next_f64() * 0.36);
            self.trees.push(Pine {
                x: x as f32,
                y: y as f32,
                h: th as f32,
                depth: depth as f32,
                seed: self.seed.wrapping_add((i as u32).wrapping_mul(7919)),
            });
        }
        self.trees.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));

        let fg_count = ((w / 420.0) * density as f64).clamp(0.0, 6.0).round() as usize;
        for i in 0..fg_count {
            let side = if i % 2 == 0 {
                0.04 + rng.next_f64() * 0.14
            } else {
                0.84 + rng.next_f64() * 0.13
            };
            let px = w * side;
            let py = horizon + h * (0.14 + rng.next_f64() * 0.22);
            let mut clear = true;
            if self.p.adaptive == 1 {
                for ic in &self.icons {
                    if ic.removing {
                        continue;
                    }
                    let dx = (ic.x + ic.w * 0.5) as f64 - px;
                    let dy = (ic.y + ic.h * 0.5) as f64 - py;
                    if dx.hypot(dy) < 110.0 {
                        clear = false;
                        break;
                    }
                }
            }
            if clear && (px - house_x).hypot(py - house_y) > 105.0 {
                let th = (h * 0.085).clamp(58.0, 108.0) * (0.85 + rng.next_f64() * 0.3);
                self.fg_pines.push(Pine {
                    x: px as f32,
                    y: py as f32,
                    h: th as f32,
                    depth: 1.0,
                    seed: self.seed.wrapping_add(997u32.wrapping_mul(i as u32 + 1)),
                });
            }
        }
    }

    // =====================================================================
    // Icon tracking (1:1 syncIncomingIcons / stepIconEffects)
    // =====================================================================
    pub fn set_icons(&mut self, rects: &[Rect], names: &[String], vis: bool) {
        let incoming: Vec<(Rect, String)> = if vis {
            rects
                .iter()
                .enumerate()
                .map(|(i, r)| (*r, names.get(i).cloned().unwrap_or_default()))
                .collect()
        } else {
            Vec::new()
        };

        let active_idx: Vec<usize> = self
            .icons
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.removing)
            .map(|(i, _)| i)
            .collect();

        let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
        for (i, (r, name)) in incoming.iter().enumerate() {
            let icx = r.x + r.w * 0.5;
            let icy = r.y + r.h * 0.5;
            for &s in &active_idx {
                let st = &self.icons[s];
                let scx = st.tx + st.tw * 0.5;
                let scy = st.ty + st.th * 0.5;
                let mut d = (icx - scx).hypot(icy - scy);
                if !name.is_empty() && !st.key.is_empty() && name == &st.key {
                    d *= 0.25;
                }
                pairs.push((d, i, s));
            }
        }
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut matched_in = vec![false; incoming.len()];
        let mut matched_state: Vec<bool> = vec![false; self.icons.len()];

        for &(d, i, s) in &pairs {
            if d > 22.0 {
                break;
            }
            if matched_in[i] || matched_state[s] {
                continue;
            }
            let (r, name) = &incoming[i];
            let st = &mut self.icons[s];
            st.tx = r.x;
            st.ty = r.y;
            st.tw = r.w;
            st.th = r.h;
            if !name.is_empty() {
                st.key = name.clone();
                st.is_folder = guess_is_folder(r, name);
            }
            matched_in[i] = true;
            matched_state[s] = true;
        }
        for &(_, i, s) in &pairs {
            if matched_in[i] || matched_state[s] {
                continue;
            }
            let (r, name) = &incoming[i];
            let st = &mut self.icons[s];
            st.tx = r.x;
            st.ty = r.y;
            st.tw = r.w;
            st.th = r.h;
            if !name.is_empty() {
                st.key = name.clone();
                st.is_folder = guess_is_folder(r, name);
            }
            matched_in[i] = true;
            matched_state[s] = true;
        }

        for &s in &active_idx {
            if !matched_state[s] {
                self.icons[s].removing = true;
            }
        }

        let mut new_states: Vec<IconState> = Vec::new();
        for (i, (r, name)) in incoming.iter().enumerate() {
            if matched_in[i] {
                continue;
            }
            let mut st = create_icon_state(r, name);
            if !self.icons_seen {
                st.fade = 1.0;
                st.snow_build = 1.0;
                st.cap_build = 1.0;
            } else {
                let bx = r.x + r.w * 0.5;
                let by = r.y + r.h * 0.72;
                self.spawn_snow_burst(bx, by, 6, 0.0);
            }
            new_states.push(st);
        }
        self.icons.extend(new_states);
        self.icons_seen = true;

        if self.p.adaptive == 1 {
            let prev_hx = self.house.x;
            self.pick_house_placement();
            if (prev_hx - self.house.x).abs() > 8.0 || !self.fg_pines.is_empty() {
                self.cache_dirty = true;
            }
        }
    }

    pub fn spawn_trail(&mut self, x: f32, y: f32, rx: f32, ry: f32) {
        for t in self.trails.iter_mut() {
            if !t.active {
                t.active = true;
                t.x = x;
                t.y = y;
                t.rx = rx;
                t.ry = ry;
                t.age = 0.0;
                t.life = 2.0;
                return;
            }
        }
    }

    pub fn spawn_snow_burst(&mut self, cx: f32, cy: f32, count: usize, spread_vx: f32) {
        let cap = if self.low_mode { count.min(3) } else { count };
        let mut spawned = 0usize;
        for b in self.bursts.iter_mut() {
            if spawned >= cap {
                break;
            }
            if !b.active {
                let r1 = self.rng.next_f32();
                let r2 = self.rng.next_f32();
                let r3 = self.rng.next_f32();
                let r4 = self.rng.next_f32();
                b.active = true;
                b.x = cx + (r1 - 0.5) * 32.0;
                b.y = cy + (r2 - 0.5) * 14.0;
                b.vx = spread_vx + (r3 - 0.5) * 34.0;
                b.vy = -(8.0 + r4 * 20.0);
                b.r = 1.1 + self.rng.next_f32() * 1.4;
                b.age = 0.0;
                b.life = 0.55 + self.rng.next_f32() * 0.45;
                spawned += 1;
            }
        }
    }

    fn step_icon_effects(&mut self, dt: f32) {
        let k_lerp = 1.0 - (-11.0 * dt).exp();
        self.moving_icon_count = 0;
        let mut i = self.icons.len();
        while i > 0 {
            i -= 1;
            let removing = self.icons[i].removing;
            if removing {
                let st = &mut self.icons[i];
                st.fade = (st.fade - dt * 2.8).max(0.0);
                st.snow_build = (st.snow_build - dt * 2.2).max(0.0);
                st.cap_build = (st.cap_build - dt * 2.6).max(0.0);
                if st.fade <= 0.01 {
                    self.icons.remove(i);
                }
                continue;
            }

            let mut trail_spawn: Option<(f32, f32, f32, f32, f32)> = None;
            let mut burst_spawn: Option<(f32, f32, usize, f32)> = None;
            let mut burst_spawn2: Option<(f32, f32, usize, f32)> = None;
            {
                let st = &mut self.icons[i];
                st.fade = (st.fade + dt * 2.4).min(1.0);
                let prev_x = st.x;
                let prev_y = st.y;
                st.x = lerp_f32(st.x, st.tx, k_lerp);
                st.y = lerp_f32(st.y, st.ty, k_lerp);
                st.w = lerp_f32(st.w, st.tw, k_lerp);
                st.h = lerp_f32(st.h, st.th, k_lerp);
                if (st.tx - st.x).hypot(st.ty - st.y) < 0.2 {
                    st.x = st.tx;
                    st.y = st.ty;
                }
                let inv_dt = 1.0 / dt.max(0.001);
                st.vx = (st.x - prev_x) * inv_dt;
                st.vy = (st.y - prev_y) * inv_dt;
                st.speed = st.vx.hypot(st.vy);

                if st.speed > 20.0 {
                    self.moving_icon_count += 1;
                    st.snow_build = (st.snow_build - dt * 0.7).max(0.45);
                    st.cap_build = (st.cap_build - dt * 1.45).max(0.12);
                    let d_trail = (st.x - st.last_trail_x).hypot(st.y - st.last_trail_y);
                    if d_trail > 22.0 {
                        st.last_trail_x = st.x;
                        st.last_trail_y = st.y;
                        trail_spawn =
                            Some((st.x + st.w * 0.5, st.y + st.h * 0.76, st.w * 0.34, 9.0, 0.0));
                        burst_spawn =
                            Some((st.x + st.w * 0.5, st.y + st.h * 0.74, 3, -st.vx * 0.08));
                        burst_spawn2 = Some((st.x + st.w * 0.5, st.y + 6.0, 2, -st.vx * 0.05));
                    }
                } else {
                    st.snow_build = (st.snow_build + dt * 0.65).min(1.0);
                    let snow_rate = 0.14 + 0.24 * clamp_f32(self.p.snow_intensity, 0.0, 1.5);
                    st.cap_build = (st.cap_build + dt * snow_rate).min(1.0);
                }
            }
            if let Some((x, y, rx, ry, _)) = trail_spawn {
                self.spawn_trail(x, y, rx, ry);
            }
            if let Some((x, y, n, vx)) = burst_spawn {
                self.spawn_snow_burst(x, y, n, vx);
            }
            if let Some((x, y, n, vx)) = burst_spawn2 {
                self.spawn_snow_burst(x, y, n, vx);
            }
        }

        for t in self.trails.iter_mut() {
            if !t.active {
                continue;
            }
            t.age += dt;
            if t.age >= t.life {
                t.active = false;
            }
        }
        for b in self.bursts.iter_mut() {
            if !b.active {
                continue;
            }
            b.age += dt;
            if b.age >= b.life {
                b.active = false;
                continue;
            }
            b.vy += 42.0 * dt;
            b.vx *= (-2.2 * dt).exp();
            b.x += b.vx * dt;
            b.y += b.vy * dt;
        }
    }

    // =====================================================================
    // Chimney smoke system
    // =====================================================================
    fn spawn_smoke_puff(&mut self) {
        let cap = ((SMOKE_MAX as f32) * self.profile().smoke_scale).floor() as usize;
        let s = self.house.scale;
        let hx = self.house.x;
        let hy = self.house.y;
        for i in 0..cap.min(SMOKE_MAX) {
            if !self.smoke[i].active {
                let r1 = self.rng.next_f32();
                let r2 = self.rng.next_f32();
                let r3 = self.rng.next_f32();
                let r4 = self.rng.next_f32();
                let r5 = self.rng.next_f32();
                let r6 = self.rng.next_f32();
                let p = &mut self.smoke[i];
                p.active = true;
                p.x = hx + 21.5 * s + (r1 - 0.5) * 2.5 * s;
                p.y = hy - 57.0 * s;
                p.vx = (r2 - 0.5) * 3.0;
                p.vy = -(14.0 + r3 * 7.0) * (0.75 + 0.25 * s);
                p.r = 3.2 * s;
                p.max_r = (16.0 + r4 * 14.0) * s;
                p.age = 0.0;
                p.life = 4.2 + r5 * 1.8;
                p.phase = r6 * TAU;
                p.alpha = 0.18 + self.rng.next_f32() * 0.12;
                return;
            }
        }
    }

    fn step_smoke(&mut self, dt: f32) {
        let amt = clamp_f32(self.p.smoke_amount, 0.0, 1.5);
        let speed = clamp_f32(self.p.anim_speed, 0.1, 2.5);
        let eff_dt = dt * speed;
        if amt > 0.02 {
            self.smoke_spawn_acc += eff_dt * (3.2 * amt);
            while self.smoke_spawn_acc >= 1.0 {
                self.smoke_spawn_acc -= 1.0;
                self.spawn_smoke_puff();
            }
        }
        let wind_px = self.p.wind_strength * 26.0;
        for p in self.smoke.iter_mut() {
            if !p.active {
                continue;
            }
            p.age += eff_dt;
            if p.age >= p.life {
                p.active = false;
                continue;
            }
            let k = p.age / p.life;
            let sway = (p.age * 1.9 + p.phase).sin() * (5.0 + k * 9.0);
            p.x += (p.vx + wind_px * (0.4 + k * 0.9) + sway) * eff_dt;
            p.y += p.vy * (1.0 - k * 0.35) * eff_dt;
            p.r = lerp_f32(p.r, p.max_r, (eff_dt * 0.95).min(1.0));
        }
    }

    // =====================================================================
    // Depth snowfall system
    // =====================================================================
    pub fn target_snow_count(&self) -> usize {
        let prof = self.profile();
        clamp_f32(
            SNOW_MAX as f32 * (self.p.snow_intensity / 1.5) * prof.snow_scale,
            0.0,
            SNOW_MAX as f32,
        )
        .round() as usize
    }

    fn reset_flake(&mut self, idx: usize, top_only: bool) {
        let w = self.w;
        let h = self.h;
        let r1 = self.rng.next_f32();
        let r2 = self.rng.next_f32();
        let r3 = self.rng.next_f32();
        let r4 = self.rng.next_f32();
        let r5 = self.rng.next_f32();
        let r6 = self.rng.next_f32();
        let r7 = self.rng.next_f32();
        let r8 = self.rng.next_f32();
        let f = &mut self.snow[idx];
        f.layer = if r1 < 0.45 {
            0
        } else if r2 < 0.65 {
            1
        } else {
            2
        };
        f.x = r3 * (w + 120.0) - 60.0;
        f.y = if top_only {
            -8.0 - r4 * 24.0
        } else {
            r4 * h
        };
        if f.layer == 0 {
            f.r = 0.8 + r5 * 0.65;
            f.vy = 13.0 + r6 * 10.0;
            f.alpha = 0.16 + r7 * 0.16;
            f.sway = 3.0 + r8 * 4.0;
        } else if f.layer == 1 {
            f.r = 1.45 + r5 * 0.85;
            f.vy = 24.0 + r6 * 14.0;
            f.alpha = 0.34 + r7 * 0.22;
            f.sway = 6.0 + r8 * 7.0;
        } else {
            f.r = 2.3 + r5 * 1.15;
            f.vy = 40.0 + r6 * 22.0;
            f.alpha = 0.54 + r7 * 0.24;
            f.sway = 10.0 + r8 * 9.0;
        }
        f.phase = self.rng.next_f32() * TAU;
        f.vx_extra = 0.0;
    }

    pub fn init_snow(&mut self) {
        for i in 0..SNOW_MAX {
            self.reset_flake(i, false);
        }
    }

    fn step_snow(&mut self, dt: f32) {
        let active_count = self.target_snow_count();
        let spd = clamp_f32(self.p.snow_speed, 0.1, 2.5) * clamp_f32(self.p.anim_speed, 0.1, 2.5);
        let wind_base = self.p.wind_strength * 36.0;
        let eff_dt = dt * spd;
        let w = self.w;
        let h = self.h;
        let moving = self.moving_icon_count > 0;
        let mut resets: Vec<usize> = Vec::new();

        for i in 0..active_count.min(SNOW_MAX) {
            let f = &mut self.snow[i];
            let depth_mul = match f.layer {
                0 => 0.55,
                1 => 0.85,
                _ => 1.18,
            };
            f.phase += eff_dt * (0.9 + f.layer as f32 * 0.35);
            f.vx_extra *= (-3.5 * dt).exp();
            f.x += (wind_base * depth_mul + f.phase.sin() * f.sway + f.vx_extra) * eff_dt;
            f.y += f.vy * eff_dt;

            if f.layer >= 1 && moving {
                for ic in self.icons.iter() {
                    if ic.speed < 25.0 || ic.removing {
                        continue;
                    }
                    let dx = f.x - (ic.x + ic.w * 0.5);
                    let dy = f.y - (ic.y + ic.h * 0.5);
                    if dx.abs() < 62.0 && dy.abs() < 62.0 {
                        f.vx_extra += clamp_f32(
                            ic.vx * 0.14 + if dx > 0.0 { 18.0 } else { -18.0 },
                            -65.0,
                            65.0,
                        ) * dt
                            * 4.0;
                        break;
                    }
                }
            }

            if f.y > h + 10.0 || f.x < -80.0 || f.x > w + 80.0 {
                resets.push(i);
            }
        }
        for i in resets {
            self.reset_flake(i, true);
        }
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.08);
        self.sim_time += dt * clamp_f32(self.p.anim_speed, 0.1, 2.5);
        self.step_icon_effects(dt);
        self.step_smoke(dt);
        self.step_snow(dt);
    }
}

pub fn guess_is_folder(rect: &Rect, name: &str) -> bool {
    let clean = name.trim();
    if !clean.is_empty() {
        let bytes = clean.as_bytes();
        let mut ext_len = 0usize;
        let mut has_dot = false;
        for (i, b) in bytes.iter().enumerate().rev() {
            if *b == b'.' {
                has_dot = i > 0;
                break;
            }
            if !b.is_ascii_alphanumeric() {
                ext_len = usize::MAX;
                break;
            }
            ext_len += 1;
        }
        if has_dot && ext_len >= 1 && ext_len <= 5 {
            return false;
        }
        return true;
    }
    rect.w >= rect.h * 0.88
}

fn build_drift_profile(seed_val: u32) -> [(f32, f32); DRIFT_POINTS] {
    let mut rng = Mulberry32::new(seed_val as i32);
    let mut pts = [(0.0f32, 0.0f32); DRIFT_POINTS];
    for (i, p) in pts.iter_mut().enumerate() {
        let ang = (i as f32 / DRIFT_POINTS as f32) * TAU;
        let wob = 0.84 + rng.next_f32() * 0.30 + 0.08 * (ang * 3.0 + rng.next_f32()).sin();
        *p = (ang.cos() * wob, ang.sin() * wob);
    }
    pts
}

pub fn create_icon_state(r: &Rect, name: &str) -> IconState {
    let s = if !name.trim().is_empty() {
        hash_str(name)
    } else {
        hash_str(&format!(
            "p:{}:{}",
            (r.x / 24.0).round() as i32,
            (r.y / 24.0).round() as i32
        ))
    };
    IconState {
        key: name.to_string(),
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
        tx: r.x,
        ty: r.y,
        tw: r.w,
        th: r.h,
        vx: 0.0,
        vy: 0.0,
        speed: 0.0,
        last_trail_x: r.x,
        last_trail_y: r.y,
        fade: 0.0,
        snow_build: 0.15,
        cap_build: 0.25,
        removing: false,
        is_folder: guess_is_folder(r, name),
        drift: build_drift_profile(s),
        cap_jitter: ((s & 255) as f32 / 255.0 - 0.5) * 4.5,
    }
}

// =========================================================================
// Batched 2D vector renderer (single color draw call per flush)
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
        Self {
            verts: Vec::with_capacity(32768),
        }
    }

    fn clear(&mut self) {
        self.verts.clear();
    }

    #[inline]
    fn push_tri(&mut self, a: ColorVert, b: ColorVert, c: ColorVert) {
        self.verts.push(a);
        self.verts.push(b);
        self.verts.push(c);
    }

    #[inline]
    fn push_quad(&mut self, a: ColorVert, b: ColorVert, c: ColorVert, d: ColorVert) {
        self.push_tri(a, b, c);
        self.push_tri(a, c, d);
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        if col[3] <= 0.001 || w <= 0.0 || h <= 0.0 {
            return;
        }
        self.push_quad(
            ColorVert { pos: [x, y], col },
            ColorVert { pos: [x + w, y], col },
            ColorVert { pos: [x + w, y + h], col },
            ColorVert { pos: [x, y + h], col },
        );
    }

    fn fill_rect_vgrad(&mut self, x: f32, y: f32, w: f32, h: f32, top: [f32; 4], bot: [f32; 4]) {
        if w <= 0.0 || h <= 0.0 || (top[3] <= 0.001 && bot[3] <= 0.001) {
            return;
        }
        self.push_quad(
            ColorVert { pos: [x, y], col: top },
            ColorVert { pos: [x + w, y], col: top },
            ColorVert { pos: [x + w, y + h], col: bot },
            ColorVert { pos: [x, y + h], col: bot },
        );
    }

    fn fill_rect_stops(&mut self, x: f32, y: f32, w: f32, h: f32, stops: &[(f32, [f32; 4])]) {
        if stops.len() < 2 || w <= 0.0 || h <= 0.0 {
            return;
        }
        for pair in stops.windows(2) {
            let (t0, c0) = pair[0];
            let (t1, c1) = pair[1];
            let y0 = y + h * t0;
            let y1 = y + h * t1;
            self.fill_rect_vgrad(x, y0, w, y1 - y0, c0, c1);
        }
    }

    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, segs: usize, col: [f32; 4]) {
        if col[3] <= 0.001 || rx <= 0.05 || ry <= 0.05 {
            return;
        }
        let center = ColorVert { pos: [cx, cy], col };
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            self.push_tri(
                center,
                ColorVert {
                    pos: [cx + a0.cos() * rx, cy + a0.sin() * ry],
                    col,
                },
                ColorVert {
                    pos: [cx + a1.cos() * rx, cy + a1.sin() * ry],
                    col,
                },
            );
        }
    }

    // Canvas createRadialGradient equivalent as concentric colored rings
    fn fill_radial_ellipse(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        stops: &[(f32, [f32; 4])],
        segs: usize,
    ) {
        if stops.len() < 2 || rx <= 0.05 || ry <= 0.05 {
            return;
        }
        for pair in stops.windows(2) {
            let (t0, c0) = pair[0];
            let (t1, c1) = pair[1];
            if c0[3] <= 0.001 && c1[3] <= 0.001 {
                continue;
            }
            let r0x = rx * t0;
            let r0y = ry * t0;
            let r1x = rx * t1;
            let r1y = ry * t1;
            for i in 0..segs {
                let a0 = (i as f32 / segs as f32) * TAU;
                let a1 = ((i + 1) as f32 / segs as f32) * TAU;
                let (s0, k0) = a0.sin_cos();
                let (s1, k1) = a1.sin_cos();
                self.push_quad(
                    ColorVert { pos: [cx + k0 * r0x, cy + s0 * r0y], col: c0 },
                    ColorVert { pos: [cx + k0 * r1x, cy + s0 * r1y], col: c1 },
                    ColorVert { pos: [cx + k1 * r1x, cy + s1 * r1y], col: c1 },
                    ColorVert { pos: [cx + k1 * r0x, cy + s1 * r0y], col: c0 },
                );
            }
        }
    }

    fn fill_polygon(&mut self, pts: &[(f32, f32)], col: [f32; 4]) {
        if pts.len() < 3 || col[3] <= 0.001 {
            return;
        }
        let mut cx = 0.0;
        let mut cy = 0.0;
        for p in pts {
            cx += p.0;
            cy += p.1;
        }
        cx /= pts.len() as f32;
        cy /= pts.len() as f32;
        let center = ColorVert { pos: [cx, cy], col };
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            self.push_tri(
                center,
                ColorVert { pos: [a.0, a.1], col },
                ColorVert { pos: [b.0, b.1], col },
            );
        }
    }

    fn fill_polygon_shaded<F>(&mut self, pts: &[(f32, f32)], shade: F)
    where
        F: Fn(f32) -> [f32; 4],
    {
        if pts.len() < 3 {
            return;
        }
        let mut cx = 0.0;
        let mut cy = 0.0;
        for p in pts {
            cx += p.0;
            cy += p.1;
        }
        cx /= pts.len() as f32;
        cy /= pts.len() as f32;
        let center = ColorVert {
            pos: [cx, cy],
            col: shade(cy),
        };
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            self.push_tri(
                center,
                ColorVert {
                    pos: [a.0, a.1],
                    col: shade(a.1),
                },
                ColorVert {
                    pos: [b.0, b.1],
                    col: shade(b.1),
                },
            );
        }
    }

    fn stroke_polyline(&mut self, pts: &[(f32, f32)], width: f32, col: [f32; 4]) {
        if pts.len() < 2 || col[3] <= 0.001 || width <= 0.05 {
            return;
        }
        let hw = width * 0.5;
        for w in pts.windows(2) {
            let (x0, y0) = w[0];
            let (x1, y1) = w[1];
            let dx = x1 - x0;
            let dy = y1 - y0;
            let len = dx.hypot(dy);
            if len < 1e-4 {
                continue;
            }
            let nx = -dy / len * hw;
            let ny = dx / len * hw;
            self.push_quad(
                ColorVert { pos: [x0 + nx, y0 + ny], col },
                ColorVert { pos: [x0 - nx, y0 - ny], col },
                ColorVert { pos: [x1 - nx, y1 - ny], col },
                ColorVert { pos: [x1 + nx, y1 + ny], col },
            );
        }
    }
}

#[inline]
fn push_quad_bezier(
    out: &mut Vec<(f32, f32)>,
    x0: f32,
    y0: f32,
    cx: f32,
    cy: f32,
    x1: f32,
    y1: f32,
    steps: usize,
) {
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let mt = 1.0 - t;
        let x = mt * mt * x0 + 2.0 * mt * t * cx + t * t * x1;
        let y = mt * mt * y0 + 2.0 * mt * t * cy + t * t * y1;
        out.push((x, y));
    }
}

#[inline]
fn grad_eval(stops: &[(f32, [f32; 4])], t: f32) -> [f32; 4] {
    let t = clamp_f32(t, 0.0, 1.0);
    if stops.is_empty() {
        return [0.0; 4];
    }
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for pair in stops.windows(2) {
        let (t0, c0) = pair[0];
        let (t1, c1) = pair[1];
        if t <= t1 {
            let u = if (t1 - t0).abs() < 1e-6 {
                0.0
            } else {
                (t - t0) / (t1 - t0)
            };
            return [
                lerp_f32(c0[0], c1[0], u),
                lerp_f32(c0[1], c1[1], u),
                lerp_f32(c0[2], c1[2], u),
                lerp_f32(c0[3], c1[3], u),
            ];
        }
    }
    stops[stops.len() - 1].1
}

// =========================================================================
// OpenGL 3.3 Core Winter Cabin Renderer
// =========================================================================
pub struct WinterRenderer {
    prog_color: GLuint,
    dummy_vao: GLuint,
    color_vao: GLuint,
    color_vbo: GLuint,
    batch: ColorBatch,

    bg_fbo: GLuint,
    bg_tex: GLuint,
    bg_w: i32,
    bg_h: i32,
    batch_w: f32,
    batch_h: f32,
}

impl WinterRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_color = gl.compile_program(WINTER_COLOR_VERT, WINTER_COLOR_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let mut color_vao = 0;
        let mut color_vbo = 0;
        (gl.GenVertexArrays)(1, &mut color_vao);
        (gl.GenBuffers)(1, &mut color_vbo);
        (gl.BindVertexArray)(color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, color_vbo);
        let stride = std::mem::size_of::<ColorVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, stride, 8 as *const c_void);
        (gl.BindVertexArray)(0);

        Ok(Self {
            prog_color,
            dummy_vao,
            color_vao,
            color_vbo,
            batch: ColorBatch::new(),
            bg_fbo: 0,
            bg_tex: 0,
            bg_w: 0,
            bg_h: 0,
            batch_w: 1920.0,
            batch_h: 1080.0,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.bg_tex != 0 {
            (gl.DeleteTextures)(1, &self.bg_tex);
            self.bg_tex = 0;
        }
        if self.bg_fbo != 0 {
            (gl.DeleteFramebuffers)(1, &self.bg_fbo);
            self.bg_fbo = 0;
        }
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
        if self.prog_color != 0 {
            (gl.DeleteProgram)(self.prog_color);
            self.prog_color = 0;
        }
    }

    unsafe fn flush(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.batch.verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_color);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_color, "u_res"), css_w, css_h);
        (gl.BindVertexArray)(self.color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.color_vbo);
        let bytes = (self.batch.verts.len() * std::mem::size_of::<ColorVert>()) as GLsizeiptr;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            bytes,
            self.batch.verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.batch.verts.len() as GLsizei);
        self.batch.clear();
    }

    unsafe fn ensure_bg_fbo(
        &mut self,
        gl: &GlFns,
        pw: i32,
        ph: i32,
        css_w: f32,
        css_h: f32,
        st: &mut WinterState,
    ) {
        let size_changed = self.bg_w != pw || self.bg_h != ph;
        if self.bg_tex != 0 && !size_changed && !st.cache_dirty {
            return;
        }
        if self.bg_tex == 0 || size_changed {
            if self.bg_tex != 0 {
                (gl.DeleteTextures)(1, &self.bg_tex);
                self.bg_tex = 0;
            }
            if self.bg_fbo != 0 {
                (gl.DeleteFramebuffers)(1, &self.bg_fbo);
                self.bg_fbo = 0;
            }
            (gl.GenTextures)(1, &mut self.bg_tex);
            (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
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
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            (gl.GenFramebuffers)(1, &mut self.bg_fbo);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
            (gl.FramebufferTexture2D)(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                self.bg_tex,
                0,
            );
            self.bg_w = pw;
            self.bg_h = ph;
        } else {
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
        }

        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);
        (gl.ClearColor)(0.055, 0.094, 0.149, 1.0);
        (gl.Clear)(GL_COLOR_BUFFER_BIT);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        st.build_forest_layout();
        self.draw_static_layer(gl, st, css_w, css_h);
        st.cache_dirty = false;
    }

    unsafe fn draw_static_layer(&mut self, gl: &GlFns, st: &WinterState, css_w: f32, css_h: f32) {
        let w = st.w;
        let h = st.h;
        let horizon = st.horizon_y;
        let atmo = clamp_f32(st.p.atmosphere_density, 0.0, 1.5);
        let m_vis = clamp_f32(st.p.mountain_visibility, 0.0, 1.0);

        // 1. Calm dark winter twilight sky
        self.batch.fill_rect_stops(
            0.0,
            0.0,
            w,
            horizon + 40.0,
            &[
                (0.0, hex_rgba(0x0a121f, 1.0)),
                (0.52, hex_rgba(0x142237, 1.0)),
                (0.84, hex_rgba(0x22354f, 1.0)),
                (1.0, hex_rgba(0x314763, 1.0)),
            ],
        );

        // Subtle moon / winter horizon glow
        let moon_x = w * 0.22;
        let moon_y = horizon * 0.34;
        let halo_r = w.min(h) * 0.28;
        self.batch.fill_radial_ellipse(
            moon_x,
            moon_y,
            halo_r,
            halo_r,
            &[
                (0.0, [218.0 / 255.0, 232.0 / 255.0, 250.0 / 255.0, 0.22]),
                (0.28, [130.0 / 255.0, 166.0 / 255.0, 206.0 / 255.0, 0.08]),
                (1.0, [18.0 / 255.0, 30.0 / 255.0, 48.0 / 255.0, 0.0]),
            ],
            48,
        );

        // Soft pale moon disc
        let moon_r = clamp_f32(w.min(h) * 0.02, 10.0, 22.0);
        self.batch.fill_ellipse(
            moon_x,
            moon_y,
            moon_r,
            moon_r,
            28,
            [232.0 / 255.0, 241.0 / 255.0, 252.0 / 255.0, 0.72],
        );

        // 2. Layered snowy mountains
        if m_vis > 0.02 {
            let m_h = clamp_f32(h * 0.24, 90.0, 240.0);
            self.draw_mountain_ridge(
                horizon + 8.0,
                m_h * 0.92,
                5.4,
                11.7,
                (st.seed % 100) as f32 * 0.07,
                hex_rgba(0x22344b, 1.0),
                hex_rgba(0x6d85a1, 1.0),
                0.22 * m_vis,
            );
            if atmo > 0.05 {
                let top = horizon - m_h * 0.55;
                self.batch.fill_rect_stops(
                    0.0,
                    top,
                    w,
                    m_h * 0.65,
                    &[
                        (0.0, [42.0 / 255.0, 62.0 / 255.0, 86.0 / 255.0, 0.0]),
                        (
                            1.0,
                            [
                                60.0 / 255.0,
                                84.0 / 255.0,
                                112.0 / 255.0,
                                clamp_f32(0.32 * atmo, 0.0, 0.52),
                            ],
                        ),
                    ],
                );
            }
            self.draw_mountain_ridge(
                horizon + 14.0,
                m_h * 0.66,
                4.1,
                9.2,
                (st.seed % 97) as f32 * 0.11 + 1.4,
                hex_rgba(0x1a293c, 1.0),
                hex_rgba(0x8ea6c2, 1.0),
                0.28 * m_vis,
            );
        }

        // 3. Rolling snow hills & winter snow plain
        let mut ground: Vec<(f32, f32)> = Vec::with_capacity(44);
        ground.push((0.0, h));
        for i in 0..=40 {
            let t = i as f32 / 40.0;
            let x = t * w;
            let wave = (t * 4.4 + 0.6).sin() * 14.0 + (t * 9.0).cos() * 6.0;
            ground.push((x, horizon + wave));
        }
        ground.push((w, h));
        let gy0 = horizon - 12.0;
        let gy1 = h;
        self.batch.fill_polygon_shaded(&ground, |y| {
            grad_eval(
                &[
                    (0.0, hex_rgba(0x5a7390, 1.0)),
                    (0.08, hex_rgba(0x415975, 1.0)),
                    (0.28, hex_rgba(0x2b3e56, 1.0)),
                    (0.68, hex_rgba(0x1d2b3e, 1.0)),
                    (1.0, hex_rgba(0x142030, 1.0)),
                ],
                (y - gy0) / (gy1 - gy0),
            )
        });

        // Subtle moonlit ridge line on upper snow drift
        let ridge: Vec<(f32, f32)> = ground[1..ground.len() - 1].to_vec();
        self.batch.stroke_polyline(
            &ridge,
            2.0,
            [205.0 / 255.0, 223.0 / 255.0, 242.0 / 255.0, 0.34],
        );

        // Subtle layered snow contour banks in midground
        let mut rng_ground = Mulberry32::new((st.seed ^ 0x33C7) as i32);
        for k in 0..4 {
            let y_off = horizon + 26.0 + k as f32 * (h - horizon) * 0.19;
            let alpha = 0.06 - k as f32 * 0.01;
            let mut bank: Vec<(f32, f32)> = Vec::with_capacity(28);
            bank.push((0.0, h));
            for i in 0..=24 {
                let t = i as f32 / 24.0;
                let x = t * w;
                let dy = (t * (3.1 + k as f32 * 0.8) + rng_ground.next_f32() * 0.2 + k as f32 * 1.9)
                    .sin()
                    * 16.0;
                bank.push((x, y_off + dy));
            }
            bank.push((w, h));
            self.batch
                .fill_polygon(&bank, [150.0 / 255.0, 178.0 / 255.0, 208.0 / 255.0, alpha]);
        }

        // 4. Horizon mist & pine forest
        if atmo > 0.05 {
            self.batch.fill_rect_stops(
                0.0,
                horizon - 45.0,
                w,
                100.0,
                &[
                    (0.0, [54.0 / 255.0, 78.0 / 255.0, 106.0 / 255.0, 0.0]),
                    (
                        0.5,
                        [
                            116.0 / 255.0,
                            146.0 / 255.0,
                            178.0 / 255.0,
                            clamp_f32(0.18 * atmo, 0.0, 0.34),
                        ],
                    ),
                    (1.0, [38.0 / 255.0, 56.0 / 255.0, 78.0 / 255.0, 0.0]),
                ],
            );
        }

        for tr in &st.trees {
            let (fol, snw) = if tr.depth < 0.45 {
                (hex_rgba(0x16282a, 1.0), hex_rgba(0x8ca4be, 1.0))
            } else {
                (hex_rgba(0x1b312d, 1.0), hex_rgba(0xb7cbe0, 1.0))
            };
            self.draw_pine(tr.x, tr.y, tr.h, fol, snw, tr.seed);
        }

        // 5. Cabin structure
        self.draw_static_cabin(gl, st, css_w, css_h);

        for tr in &st.fg_pines {
            self.draw_pine(
                tr.x,
                tr.y,
                tr.h,
                hex_rgba(0x172b26, 1.0),
                hex_rgba(0xaec3da, 1.0),
                tr.seed,
            );
        }

        self.flush(gl, css_w, css_h);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_mountain_ridge(
        &mut self,
        y_base: f32,
        amp: f32,
        freq_a: f32,
        freq_b: f32,
        phase: f32,
        fill: [f32; 4],
        snow_color: [f32; 4],
        snow_mix: f32,
    ) {
        let w = self.batch_w;
        let h = self.batch_h;
        let steps = ((w / 28.0).ceil() as usize).max(24);
        let mut pts: Vec<(f32, f32)> = Vec::with_capacity(steps + 1);
        let mut peak_y = y_base;
        for i in 0..=steps {
            let x = (i as f32 / steps as f32) * w;
            let n = i as f32 / steps as f32;
            let h1 = (n * freq_a + phase).sin() * 0.52
                + (n * freq_b + phase * 1.7).sin() * 0.31
                + (n * (freq_a * 2.1) - phase * 0.6).cos() * 0.17;
            let ridge = y_base - h1.abs() * amp;
            if ridge < peak_y {
                peak_y = ridge;
            }
            pts.push((x, ridge));
        }
        let mut poly: Vec<(f32, f32)> = Vec::with_capacity(steps + 3);
        poly.push((0.0, h));
        poly.extend_from_slice(&pts);
        poly.push((w, h));

        let span = (y_base + 16.0 - peak_y).max(1.0);
        self.batch.fill_polygon_shaded(&poly, |y| {
            let k = (1.0 - clamp_f32((y - peak_y) / span, 0.0, 1.0)) * snow_mix * 0.72;
            [
                lerp_f32(fill[0], snow_color[0], k),
                lerp_f32(fill[1], snow_color[1], k),
                lerp_f32(fill[2], snow_color[2], k),
                fill[3],
            ]
        });

        if snow_mix > 0.01 {
            let crest = [
                snow_color[0],
                snow_color[1],
                snow_color[2],
                (snow_mix * 1.45).min(0.65),
            ];
            self.batch.stroke_polyline(&pts, 2.2, crest);
        }
    }

    fn draw_pine(&mut self, x: f32, y: f32, h: f32, foliage: [f32; 4], snow: [f32; 4], seed: u32) {
        let mut rng = Mulberry32::new(seed as i32);
        let tiers = if h > 54.0 { 4 } else { 3 };
        let w_base = h * (0.44 + rng.next_f32() * 0.12);

        self.batch.fill_ellipse(
            x + h * 0.06,
            y + 1.5,
            w_base * 0.58,
            (h * 0.065).max(2.0),
            18,
            [10.0 / 255.0, 18.0 / 255.0, 30.0 / 255.0, 0.30],
        );

        let tw = (h * 0.085).max(2.0);
        self.batch.fill_rect(
            x - tw * 0.5,
            y - h * 0.18,
            tw,
            h * 0.2,
            hex_rgba(0x1b1615, 1.0),
        );

        for t in 0..tiers {
            let frac = t as f32 / tiers as f32;
            let next_frac = (t + 1) as f32 / tiers as f32;
            let tier_bottom = y - h * (0.12 + frac * 0.66);
            let tier_top = y - h * (0.36 + next_frac * 0.62);
            let half_w = w_base * (0.56 - frac * 0.34);
            let sag = (tier_bottom - tier_top) * 0.14;

            let mut leaf: Vec<(f32, f32)> = Vec::with_capacity(28);
            leaf.push((x, tier_top));
            push_quad_bezier(
                &mut leaf,
                x,
                tier_top,
                x + half_w * 0.55,
                lerp_f32(tier_top, tier_bottom, 0.55),
                x + half_w,
                tier_bottom,
                6,
            );
            push_quad_bezier(
                &mut leaf,
                x + half_w,
                tier_bottom,
                x,
                tier_bottom + sag,
                x - half_w,
                tier_bottom,
                6,
            );
            push_quad_bezier(
                &mut leaf,
                x - half_w,
                tier_bottom,
                x - half_w * 0.55,
                lerp_f32(tier_top, tier_bottom, 0.55),
                x,
                tier_top,
                6,
            );
            self.batch.fill_polygon(&leaf, foliage);

            let snow_span = half_w * (0.76 + rng.next_f32() * 0.14);
            let snow_depth = (tier_bottom - tier_top) * 0.42;
            let mut cap: Vec<(f32, f32)> = Vec::with_capacity(24);
            cap.push((x, tier_top));
            push_quad_bezier(
                &mut cap,
                x,
                tier_top,
                x + snow_span * 0.5,
                tier_top + snow_depth * 0.45,
                x + snow_span,
                tier_top + snow_depth,
                5,
            );
            push_quad_bezier(
                &mut cap,
                x + snow_span,
                tier_top + snow_depth,
                x + snow_span * 0.25,
                tier_top + snow_depth * 0.85,
                x - snow_span * 0.65,
                tier_top + snow_depth * 0.92,
                5,
            );
            push_quad_bezier(
                &mut cap,
                x - snow_span * 0.65,
                tier_top + snow_depth * 0.92,
                x - snow_span * 0.38,
                tier_top + snow_depth * 0.45,
                x,
                tier_top,
                5,
            );
            self.batch.fill_polygon(&cap, snow);
        }
    }

    unsafe fn draw_static_cabin(&mut self, gl: &GlFns, st: &WinterState, css_w: f32, css_h: f32) {
        let cx = st.house.x;
        let cy = st.house.y;
        let s = st.house.scale;
        let p = |lx: f32, ly: f32| -> (f32, f32) { (cx + lx * s, cy + ly * s) };

        let (sx, sy) = p(4.0, 14.0);
        self.batch.fill_ellipse(
            sx,
            sy,
            56.0 * s,
            12.0 * s,
            28,
            [11.0 / 255.0, 19.0 / 255.0, 31.0 / 255.0, 0.42],
        );

        // Cozy warm light spill cast onto the snow below the windows
        let (gx, gy) = p(-2.0, 18.0);
        self.batch.fill_radial_ellipse(
            gx,
            gy,
            62.0 * s,
            22.0 * s,
            &[
                (0.0, [246.0 / 255.0, 168.0 / 255.0, 86.0 / 255.0, 0.24]),
                (0.55, [220.0 / 255.0, 134.0 / 255.0, 66.0 / 255.0, 0.09]),
                (1.0, [0.0, 0.0, 0.0, 0.0]),
            ],
            32,
        );

        // Chimney
        let (chx, chy) = p(16.0, -54.0);
        self.batch
            .fill_rect(chx, chy, 11.0 * s, 24.0 * s, hex_rgba(0x382d2b, 1.0));
        let (c2x, c2y) = p(14.0, -56.0);
        self.batch
            .fill_rect(c2x, c2y, 15.0 * s, 4.0 * s, hex_rgba(0x261e1d, 1.0));
        let (c3x, c3y) = p(21.5, -56.0);
        self.batch
            .fill_ellipse(c3x, c3y, 7.5 * s, 2.2 * s, 20, hex_rgba(0xb9cbe0, 1.0));

        // Cabin main timber body
        let body = [
            p(-36.0, 12.0),
            p(-36.0, -18.0),
            p(0.0, -40.0),
            p(36.0, -18.0),
            p(36.0, 12.0),
        ];
        self.batch.fill_polygon(&body, hex_rgba(0x2b201c, 1.0));

        // Horizontal log plank seams
        let mut py = -12.0;
        while py <= 8.0 {
            let a = p(-35.0, py);
            let b = p(35.0, py);
            self.batch.stroke_polyline(
                &[a, b],
                1.0 * s,
                [14.0 / 255.0, 10.0 / 255.0, 9.0 / 255.0, 0.48],
            );
            py += 5.0;
        }

        // Eave shadow
        let eave = [
            p(-37.0, -17.0),
            p(0.0, -39.0),
            p(37.0, -17.0),
            p(37.0, -11.0),
            p(0.0, -33.0),
            p(-37.0, -11.0),
        ];
        self.batch
            .fill_polygon(&eave, [10.0 / 255.0, 12.0 / 255.0, 16.0 / 255.0, 0.42]);

        // Steep pitched roof
        let roof = [
            p(-45.0, -14.0),
            p(0.0, -45.0),
            p(45.0, -14.0),
            p(41.0, -11.0),
            p(0.0, -39.0),
            p(-41.0, -11.0),
        ];
        self.batch.fill_polygon(&roof, hex_rgba(0x1b1d23, 1.0));

        // Thick organic snow cap on the roof
        let mut cap: Vec<(f32, f32)> = Vec::with_capacity(40);
        cap.push(p(-47.0, -14.0));
        push_quad_bezier(
            &mut cap,
            cx - 47.0 * s,
            cy - 14.0 * s,
            cx - 24.0 * s,
            cy - 33.0 * s,
            cx,
            cy - 47.0 * s,
            6,
        );
        push_quad_bezier(
            &mut cap,
            cx,
            cy - 47.0 * s,
            cx + 24.0 * s,
            cy - 33.0 * s,
            cx + 47.0 * s,
            cy - 14.0 * s,
            6,
        );
        push_quad_bezier(
            &mut cap,
            cx + 47.0 * s,
            cy - 14.0 * s,
            cx + 44.0 * s,
            cy - 9.0 * s,
            cx + 36.0 * s,
            cy - 12.0 * s,
            4,
        );
        push_quad_bezier(
            &mut cap,
            cx + 36.0 * s,
            cy - 12.0 * s,
            cx + 18.0 * s,
            cy - 24.0 * s,
            cx,
            cy - 37.0 * s,
            5,
        );
        push_quad_bezier(
            &mut cap,
            cx,
            cy - 37.0 * s,
            cx - 18.0 * s,
            cy - 24.0 * s,
            cx - 36.0 * s,
            cy - 12.0 * s,
            5,
        );
        push_quad_bezier(
            &mut cap,
            cx - 36.0 * s,
            cy - 12.0 * s,
            cx - 44.0 * s,
            cy - 9.0 * s,
            cx - 47.0 * s,
            cy - 14.0 * s,
            4,
        );
        self.batch.fill_polygon(&cap, hex_rgba(0xc8d8ea, 1.0));

        let mut cap2: Vec<(f32, f32)> = Vec::with_capacity(32);
        cap2.push(p(-44.0, -15.0));
        push_quad_bezier(
            &mut cap2,
            cx - 44.0 * s,
            cy - 15.0 * s,
            cx - 21.0 * s,
            cy - 33.0 * s,
            cx,
            cy - 46.0 * s,
            6,
        );
        push_quad_bezier(
            &mut cap2,
            cx,
            cy - 46.0 * s,
            cx + 21.0 * s,
            cy - 33.0 * s,
            cx + 44.0 * s,
            cy - 15.0 * s,
            6,
        );
        push_quad_bezier(
            &mut cap2,
            cx + 44.0 * s,
            cy - 15.0 * s,
            cx + 22.0 * s,
            cy - 28.0 * s,
            cx,
            cy - 41.0 * s,
            5,
        );
        push_quad_bezier(
            &mut cap2,
            cx,
            cy - 41.0 * s,
            cx - 22.0 * s,
            cy - 28.0 * s,
            cx - 44.0 * s,
            cy - 15.0 * s,
            5,
        );
        self.batch.fill_polygon(&cap2, hex_rgba(0xe5eef8, 1.0));

        // Wooden door
        let (dx, dy) = p(-5.0, -8.0);
        self.batch
            .fill_rect(dx, dy, 10.0 * s, 20.0 * s, hex_rgba(0x1d1512, 1.0));
        self.batch.stroke_polyline(
            &[
                (dx, dy),
                (dx + 10.0 * s, dy),
                (dx + 10.0 * s, dy + 20.0 * s),
                (dx, dy + 20.0 * s),
                (dx, dy),
            ],
            1.0 * s,
            hex_rgba(0x47362e, 1.0),
        );

        // Window frames (warm glow drawn dynamically)
        for (lx, ly, lw, lh) in [
            (-26.0f32, -12.0f32, 13.0f32, 12.0f32),
            (13.0, -12.0, 13.0, 12.0),
            (-4.5, -26.0, 9.0, 8.0),
        ] {
            let (wx, wy) = p(lx, ly);
            self.batch
                .fill_rect(wx, wy, lw * s, lh * s, hex_rgba(0x1d1512, 1.0));
        }

        self.flush(gl, css_w, css_h);
    }

    fn draw_stars(&mut self, st: &WinterState) {
        let t = st.sim_time;
        for s in &st.stars {
            let a = s.a * (0.72 + 0.28 * (t * s.tw + s.ph).sin());
            self.batch.fill_rect(
                s.nx * st.w,
                s.ny * st.horizon_y,
                s.r,
                s.r,
                [226.0 / 255.0, 236.0 / 255.0, 248.0 / 255.0, a],
            );
        }
    }

    fn draw_cabin_lights(&mut self, st: &WinterState) {
        let cx = st.house.x;
        let cy = st.house.y;
        let s = st.house.scale;
        let t = st.sim_time;
        let flicker = 0.88 + 0.07 * (t * 3.1).sin() + 0.05 * (t * 5.7).cos();
        let prof = st.profile();

        if prof.soft_glow {
            let gx = cx - 2.0 * s;
            let gy = cy - 6.0 * s;
            self.batch.fill_radial_ellipse(
                gx,
                gy,
                44.0 * s,
                44.0 * s,
                &[
                    (0.0, [1.0, 190.0 / 255.0, 96.0 / 255.0, 0.26 * flicker]),
                    (
                        0.55,
                        [240.0 / 255.0, 142.0 / 255.0, 62.0 / 255.0, 0.09 * flicker],
                    ),
                    (1.0, [0.0, 0.0, 0.0, 0.0]),
                ],
                32,
            );
        }

        for (px, py, pw, ph) in [
            (-24.5f32, -10.5f32, 10.0f32, 9.0f32),
            (14.5, -10.5, 10.0, 9.0),
            (-3.0, -24.5, 6.0, 5.0),
        ] {
            let x = cx + px * s;
            let y = cy + py * s;
            self.batch.fill_rect_vgrad(
                x,
                y,
                pw * s,
                ph * s,
                [1.0, 224.0 / 255.0, 152.0 / 255.0, 0.92 * flicker],
                [242.0 / 255.0, 148.0 / 255.0, 62.0 / 255.0, 0.88 * flicker],
            );
        }

        let mull = hex_rgba(0x251a17, 1.0);
        let (a, b) = ((cx - 20.0 * s, cy - 10.5 * s), (cx - 20.0 * s, cy - 1.5 * s));
        self.batch.stroke_polyline(&[a, b], 1.0 * s, mull);
        let (a, b) = ((cx - 24.5 * s, cy - 6.5 * s), (cx - 14.5 * s, cy - 6.5 * s));
        self.batch.stroke_polyline(&[a, b], 1.0 * s, mull);
        let (a, b) = ((cx + 19.0 * s, cy - 10.5 * s), (cx + 19.0 * s, cy - 1.5 * s));
        self.batch.stroke_polyline(&[a, b], 1.0 * s, mull);
        let (a, b) = ((cx + 14.5 * s, cy - 6.5 * s), (cx + 24.5 * s, cy - 6.5 * s));
        self.batch.stroke_polyline(&[a, b], 1.0 * s, mull);
    }

    fn draw_smoke(&mut self, st: &WinterState) {
        let amt = clamp_f32(st.p.smoke_amount, 0.0, 1.5);
        if amt <= 0.02 {
            return;
        }
        for p in &st.smoke {
            if !p.active {
                continue;
            }
            let k = p.age / p.life;
            let env = ((k * 4.5).min(1.0) * PI * 0.5).sin() * (1.0 - k) * (1.0 - k);
            let a = p.alpha * clamp_f32(amt, 0.2, 1.2) * env;
            if a <= 0.004 {
                continue;
            }
            self.batch.fill_ellipse(
                p.x,
                p.y,
                p.r,
                p.r,
                14,
                [202.0 / 255.0, 214.0 / 255.0, 228.0 / 255.0, a],
            );
        }
    }

    fn draw_icon_snow_mounds(&mut self, st: &WinterState) {
        if st.p.adaptive != 1 || st.p.icon_snow_amount <= 0.02 {
            return;
        }
        let amt = clamp_f32(st.p.icon_snow_amount, 0.0, 1.5);

        for tr in &st.trails {
            if !tr.active {
                continue;
            }
            let k = 1.0 - tr.age / tr.life;
            self.batch.fill_ellipse(
                tr.x,
                tr.y,
                tr.rx * (0.85 + 0.2 * (1.0 - k)),
                tr.ry,
                20,
                [
                    182.0 / 255.0,
                    204.0 / 255.0,
                    228.0 / 255.0,
                    0.16 * k * k * amt.min(1.0),
                ],
            );
        }

        for st_icon in &st.icons {
            let alpha = st_icon.fade * clamp_f32(amt, 0.25, 1.15);
            if alpha <= 0.02 {
                continue;
            }
            let cx = st_icon.x + st_icon.w * 0.5;
            let base_y = st_icon.y + st_icon.h * 0.72;
            let folder_boost = if st_icon.is_folder { 1.18 } else { 0.94 };
            let mound_rx = (st_icon.w * 0.50).max(26.0)
                * folder_boost
                * (0.82 + 0.18 * st_icon.snow_build)
                * clamp_f32(0.75 + amt * 0.22, 0.7, 1.15);
            let mound_ry =
                (st_icon.h * 0.16).max(10.0) * folder_boost * (0.78 + 0.22 * st_icon.snow_build);
            let in_sky = base_y < st.horizon_y + 8.0;
            let ga = alpha.min(1.0);

            let shadow_col = if in_sky {
                [9.0 / 255.0, 15.0 / 255.0, 25.0 / 255.0, 0.34 * ga]
            } else {
                [10.0 / 255.0, 17.0 / 255.0, 28.0 / 255.0, 0.44 * ga]
            };
            self.batch.fill_radial_ellipse(
                cx,
                st_icon.y + st_icon.h * 0.84,
                st_icon.w * 0.64,
                (st_icon.h * 0.24).max(12.0),
                &[
                    (0.0, shadow_col),
                    (1.0, [10.0 / 255.0, 17.0 / 255.0, 28.0 / 255.0, 0.0]),
                ],
                28,
            );

            if in_sky {
                self.batch.fill_radial_ellipse(
                    cx,
                    base_y - 2.0,
                    mound_rx * 1.12,
                    mound_ry * 0.95,
                    &[
                        (
                            0.0,
                            [
                                198.0 / 255.0,
                                218.0 / 255.0,
                                240.0 / 255.0,
                                0.22 * st_icon.snow_build * ga,
                            ],
                        ),
                        (
                            0.55,
                            [
                                132.0 / 255.0,
                                162.0 / 255.0,
                                196.0 / 255.0,
                                0.10 * st_icon.snow_build * ga,
                            ],
                        ),
                        (1.0, [132.0 / 255.0, 162.0 / 255.0, 196.0 / 255.0, 0.0]),
                    ],
                    28,
                );
                let mut poly: Vec<(f32, f32)> = Vec::with_capacity(DRIFT_POINTS);
                for p in st_icon.drift.iter() {
                    poly.push((cx + p.0 * mound_rx * 0.88, base_y + p.1 * mound_ry * 0.42));
                }
                self.batch.fill_polygon(
                    &poly,
                    [
                        218.0 / 255.0,
                        232.0 / 255.0,
                        246.0 / 255.0,
                        0.24 * st_icon.snow_build * ga,
                    ],
                );
            } else {
                let mut poly: Vec<(f32, f32)> = Vec::with_capacity(DRIFT_POINTS);
                for p in st_icon.drift.iter() {
                    poly.push((cx + p.0 * mound_rx, base_y + p.1 * mound_ry));
                }
                let y0 = base_y - mound_ry;
                let y1 = base_y + mound_ry;
                let stops = [
                    (0.0, [222.0 / 255.0, 235.0 / 255.0, 248.0 / 255.0, 0.72 * ga]),
                    (
                        0.55,
                        [154.0 / 255.0, 178.0 / 255.0, 206.0 / 255.0, 0.48 * ga],
                    ),
                    (1.0, [72.0 / 255.0, 94.0 / 255.0, 122.0 / 255.0, 0.18 * ga]),
                ];
                self.batch.fill_polygon_shaded(&poly, |y| {
                    grad_eval(&stops, (y - y0) / (y1 - y0).max(1e-4))
                });

                let mut crest: Vec<(f32, f32)> = Vec::with_capacity(DRIFT_POINTS);
                for p in st_icon.drift.iter() {
                    crest.push((
                        cx + p.0 * mound_rx * 0.74,
                        (base_y - mound_ry * 0.28) + p.1 * mound_ry * 0.50,
                    ));
                }
                self.batch.fill_polygon(
                    &crest,
                    [
                        238.0 / 255.0,
                        245.0 / 255.0,
                        252.0 / 255.0,
                        0.44 * st_icon.snow_build * ga,
                    ],
                );

                self.batch.fill_ellipse(
                    cx + 1.5,
                    base_y - mound_ry * 0.12,
                    mound_rx * 0.56,
                    (mound_ry * 0.36).max(3.5),
                    20,
                    [12.0 / 255.0, 20.0 / 255.0, 32.0 / 255.0, 0.34 * ga],
                );
            }

            if st_icon.cap_build > 0.04 {
                let cap_amt = st_icon.cap_build * clamp_f32(0.55 + amt * 0.45, 0.4, 1.2);
                let cap_half_w = st_icon.w.clamp(20.0, 30.0) * 0.40
                    * if st_icon.is_folder { 1.12 } else { 0.92 };
                let cap_half_w = cap_half_w.min(30.0).max(20.0);
                let cap_y = st_icon.y + 7.0;
                let crest_h = (if st_icon.is_folder { 9.5 } else { 8.0 })
                    * clamp_f32(0.65 + amt * 0.35, 0.6, 1.25)
                    * st_icon.cap_build;
                let jx = st_icon.cap_jitter;

                let mut cap: Vec<(f32, f32)> = Vec::with_capacity(24);
                cap.push((cx - cap_half_w, cap_y + 3.0));
                push_quad_bezier(
                    &mut cap,
                    cx - cap_half_w,
                    cap_y + 3.0,
                    cx - cap_half_w * 0.5 + jx,
                    cap_y - crest_h * 1.15,
                    cx + jx * 0.5,
                    cap_y - crest_h * 0.85,
                    5,
                );
                push_quad_bezier(
                    &mut cap,
                    cx + jx * 0.5,
                    cap_y - crest_h * 0.85,
                    cx + cap_half_w * 0.5 + jx,
                    cap_y - crest_h * 1.05,
                    cx + cap_half_w,
                    cap_y + 3.0,
                    5,
                );
                push_quad_bezier(
                    &mut cap,
                    cx + cap_half_w,
                    cap_y + 3.0,
                    cx + cap_half_w * 0.4,
                    cap_y + 6.5,
                    cx,
                    cap_y + 5.0,
                    4,
                );
                push_quad_bezier(
                    &mut cap,
                    cx,
                    cap_y + 5.0,
                    cx - cap_half_w * 0.4,
                    cap_y + 6.5,
                    cx - cap_half_w,
                    cap_y + 3.0,
                    4,
                );
                let y0 = cap_y - crest_h;
                let y1 = cap_y + 6.0;
                let stops = [
                    (0.0, [240.0 / 255.0, 247.0 / 255.0, 1.0, 0.88 * cap_amt * ga]),
                    (
                        0.65,
                        [
                            196.0 / 255.0,
                            216.0 / 255.0,
                            238.0 / 255.0,
                            0.76 * cap_amt * ga,
                        ],
                    ),
                    (
                        1.0,
                        [
                            128.0 / 255.0,
                            154.0 / 255.0,
                            184.0 / 255.0,
                            0.45 * cap_amt * ga,
                        ],
                    ),
                ];
                self.batch.fill_polygon_shaded(&cap, |y| {
                    grad_eval(&stops, (y - y0) / (y1 - y0).max(1e-4))
                });

                let mut crest: Vec<(f32, f32)> = Vec::with_capacity(16);
                crest.push((cx - cap_half_w * 0.85, cap_y + 1.0));
                push_quad_bezier(
                    &mut crest,
                    cx - cap_half_w * 0.85,
                    cap_y + 1.0,
                    cx - cap_half_w * 0.4 + jx,
                    cap_y - crest_h * 0.98,
                    cx + jx * 0.5,
                    cap_y - crest_h * 0.72,
                    5,
                );
                push_quad_bezier(
                    &mut crest,
                    cx + jx * 0.5,
                    cap_y - crest_h * 0.72,
                    cx + cap_half_w * 0.45 + jx,
                    cap_y - crest_h * 0.88,
                    cx + cap_half_w * 0.85,
                    cap_y + 1.0,
                    5,
                );
                self.batch.stroke_polyline(
                    &crest,
                    1.4,
                    [246.0 / 255.0, 250.0 / 255.0, 1.0, 0.70 * cap_amt * ga],
                );
            }
        }

        for b in &st.bursts {
            if !b.active {
                continue;
            }
            let k = 1.0 - b.age / b.life;
            self.batch.fill_ellipse(
                b.x,
                b.y,
                b.r * (0.7 + 0.3 * k),
                b.r * (0.7 + 0.3 * k),
                12,
                [224.0 / 255.0, 236.0 / 255.0, 248.0 / 255.0, 0.52 * k],
            );
        }
    }

    fn draw_snow(&mut self, st: &WinterState) {
        let active_count = st.target_snow_count().min(SNOW_MAX);
        if active_count == 0 {
            return;
        }
        for layer in 0..=2 {
            for f in st.snow.iter().take(active_count) {
                if f.layer != layer {
                    continue;
                }
                self.batch.fill_ellipse(
                    f.x,
                    f.y,
                    f.r,
                    f.r,
                    10,
                    [228.0 / 255.0, 238.0 / 255.0, 248.0 / 255.0, f.alpha],
                );
            }
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState, prog_blit: GLuint) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();

        st.winter.resize(css_w, css_h);
        self.batch_w = css_w;
        self.batch_h = css_h;

        self.ensure_bg_fbo(gl, pw, ph, css_w, css_h, &mut st.winter);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(prog_blit);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
        (gl.Uniform1i)(gl.uniform_loc(prog_blit, "u_tex"), 0);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        self.draw_stars(&st.winter);
        self.draw_cabin_lights(&st.winter);
        self.draw_smoke(&st.winter);
        self.draw_icon_snow_mounds(&st.winter);
        self.draw_snow(&st.winter);
        self.flush(gl, css_w, css_h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const CASES: &str = include_str!("cases.jsonl");
    const RECORDED: &str = include_str!("recorded-from.json");

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-4
    }

    fn num(v: &Value) -> f64 {
        v.as_f64().unwrap_or(f64::NAN)
    }

    fn rect_from(v: &Value) -> Rect {
        Rect {
            x: num(&v["x"]) as f32,
            y: num(&v["y"]) as f32,
            w: num(&v["w"]) as f32,
            h: num(&v["h"]) as f32,
        }
    }

    fn icon_from(v: &Value) -> IconState {
        let r = rect_from(v);
        let mut st = create_icon_state(&r, "");
        st.removing = v["removing"].as_bool().unwrap_or(false);
        st
    }

    fn params_for(quality: &str, density: f32, adaptive: i32) -> WinterParams {
        WinterParams {
            quality: match quality {
                "Low" => 0,
                "Medium" => 1,
                _ => 2,
            },
            tree_density: density,
            adaptive,
            ..WinterParams::default()
        }
    }

    #[test]
    fn recorded_fingerprint_matches_source() {
        let meta: Value = serde_json::from_str(RECORDED).expect("recorded-from.json");
        assert_eq!(meta["schema"], "rust-anything/recorded-from@1");
        assert_eq!(meta["unit"], "prototype/winter.html");
        assert_eq!(meta["cases"].as_u64().unwrap() as usize, CASES.lines().count());
        let fp = meta["fingerprint"].as_str().unwrap();
        assert_eq!(fp.len(), 16);
    }

    #[test]
    fn replays_recorded_cases() {
        let mut checked = 0usize;
        for line in CASES.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let case: Value = serde_json::from_str(line).expect("case json");
            let name = case["fn"].as_str().unwrap();
            let args = case["args"].as_array().unwrap();
            let ok = &case["result"]["ok"];
            match name {
                "clamp" => {
                    let got =
                        clamp_f32(num(&args[0]) as f32, num(&args[1]) as f32, num(&args[2]) as f32);
                    assert!(close(got as f64, num(ok)), "clamp {:?}", args);
                }
                "lerp" => {
                    let got =
                        lerp_f32(num(&args[0]) as f32, num(&args[1]) as f32, num(&args[2]) as f32);
                    assert!(close(got as f64, num(ok)), "lerp {:?}", args);
                }
                "smoothstep" => {
                    let got = smoothstep_f32(
                        num(&args[0]) as f32,
                        num(&args[1]) as f32,
                        num(&args[2]) as f32,
                    );
                    assert!(close(got as f64, num(ok)), "smoothstep {:?}", args);
                }
                "mulberry32" => {
                    let mut rng = Mulberry32::new(num(&args[0]) as i32);
                    for want in ok.as_array().unwrap() {
                        let got = rng.next_f64();
                        assert!(close(got, num(want)), "mulberry32 {:?}", args);
                    }
                }
                "hashStr" => {
                    let got = hash_str(args[0].as_str().unwrap());
                    assert_eq!(got as u64, ok.as_u64().unwrap(), "hashStr {:?}", args);
                }
                "buildDriftProfile" => {
                    let got = build_drift_profile(num(&args[0]) as u32);
                    let want = ok.as_array().unwrap();
                    assert_eq!(got.len(), want.len());
                    for (i, w) in want.iter().enumerate() {
                        assert!(close(got[i].0 as f64, num(&w[0])), "drift c {}", i);
                        assert!(close(got[i].1 as f64, num(&w[1])), "drift s {}", i);
                    }
                }
                "guessIsFolder" => {
                    let r = Rect {
                        x: 0.0,
                        y: 0.0,
                        w: num(&args[0]) as f32,
                        h: num(&args[1]) as f32,
                    };
                    let got = guess_is_folder(&r, args[2].as_str().unwrap());
                    assert_eq!(got, ok.as_bool().unwrap(), "guessIsFolder {:?}", args);
                }
                "createIconState" => {
                    let r = Rect {
                        x: num(&args[0]) as f32,
                        y: num(&args[1]) as f32,
                        w: num(&args[2]) as f32,
                        h: num(&args[3]) as f32,
                    };
                    let st = create_icon_state(&r, args[4].as_str().unwrap());
                    assert!(close(st.cap_jitter as f64, num(&ok["capJitter"])));
                    assert_eq!(st.is_folder, ok["isFolder"].as_bool().unwrap());
                    for (key, idx) in [("drift0", 0usize), ("drift7", 7), ("drift13", 13)] {
                        assert!(close(st.drift[idx].0 as f64, num(&ok[key][0])), "{} c", key);
                        assert!(close(st.drift[idx].1 as f64, num(&ok[key][1])), "{} s", key);
                    }
                }
                "pickHousePlacement" => {
                    let w = num(&args[0]) as f32;
                    let h = num(&args[1]) as f32;
                    let hy = num(&args[2]) as f32;
                    let mut st = WinterState::new(w, h, WinterParams::default(), 20261117);
                    st.horizon_y = hy;
                    st.icons = args[3].as_array().unwrap().iter().map(icon_from).collect();
                    st.pick_house_placement();
                    assert!(close(st.house.x as f64, num(&ok["x"])), "house x {:?}", args);
                    assert!(close(st.house.y as f64, num(&ok["y"])), "house y {:?}", args);
                    assert!(close(st.house.scale as f64, num(&ok["scale"])), "house scale");
                }
                "buildForestLayout" => {
                    let w = num(&args[0]) as f32;
                    let h = num(&args[1]) as f32;
                    let hy = num(&args[2]) as f32;
                    let density = num(&args[3]) as f32;
                    let quality = args[4].as_str().unwrap();
                    let adaptive = num(&args[5]) as i32;
                    let icons: Vec<IconState> =
                        args[6].as_array().unwrap().iter().map(icon_from).collect();
                    let mut st =
                        WinterState::new(w, h, params_for(quality, density, adaptive), 20261117);
                    st.horizon_y = hy;
                    st.icons = icons;
                    st.pick_house_placement();
                    st.build_forest_layout();
                    assert_eq!(st.trees.len() as u64, ok["treeCount"].as_u64().unwrap());
                    assert_eq!(st.fg_pines.len() as u64, ok["fgCount"].as_u64().unwrap());
                    let mut by_seed: std::collections::HashMap<u64, (f32, f32, f32)> =
                        std::collections::HashMap::new();
                    for t in &st.trees {
                        by_seed.insert(t.seed as u64, (t.x, t.y, t.h));
                    }
                    let first = ok["first"].as_array().unwrap();
                    let last = ok["last"].as_array().unwrap();
                    for want in first.iter().chain(last.iter()) {
                        let seed = want[3].as_u64().unwrap();
                        let got = by_seed
                            .get(&seed)
                            .unwrap_or_else(|| panic!("missing tree seed {}", seed));
                        assert!(close(got.0 as f64, num(&want[0])), "tree {} x", seed);
                        assert!(close(got.1 as f64, num(&want[1])), "tree {} y", seed);
                        assert!(close(got.2 as f64, num(&want[2])), "tree {} h", seed);
                    }
                    let mut fg_by_seed: std::collections::HashMap<u64, (f32, f32, f32)> =
                        std::collections::HashMap::new();
                    for t in &st.fg_pines {
                        fg_by_seed.insert(t.seed as u64, (t.x, t.y, t.h));
                    }
                    let fg = ok["fg"].as_array().unwrap();
                    for want in fg.iter() {
                        let seed = want[3].as_u64().unwrap();
                        let got = fg_by_seed
                            .get(&seed)
                            .unwrap_or_else(|| panic!("missing fg pine seed {}", seed));
                        assert!(close(got.0 as f64, num(&want[0])), "fg {} x", seed);
                        assert!(close(got.1 as f64, num(&want[1])), "fg {} y", seed);
                        assert!(close(got.2 as f64, num(&want[2])), "fg {} h", seed);
                    }
                }
                "getProfile" => {
                    let quality = args[0].as_str().unwrap();
                    let low = args[1].as_bool().unwrap();
                    let mut st =
                        WinterState::new(1920.0, 1080.0, params_for(quality, 0.8, 1), 20261117);
                    st.set_low_mode(low);
                    let prof = st.profile();
                    assert!(close(prof.snow_scale as f64, num(&ok["snowScale"])));
                    assert!(close(prof.smoke_scale as f64, num(&ok["smokeScale"])));
                    assert!(close(prof.tree_scale as f64, num(&ok["treeScale"])));
                    assert_eq!(prof.soft_glow, ok["softGlow"].as_bool().unwrap());
                }
                other => panic!("unhandled case fn {}", other),
            }
            checked += 1;
        }
        assert_eq!(
            checked,
            CASES.lines().filter(|l| !l.trim().is_empty()).count()
        );
        assert!(checked >= 50);
    }

    #[test]
    fn snow_and_smoke_pools_stay_in_bounds() {
        let mut st = WinterState::new(1280.0, 720.0, WinterParams::default(), 20261117);
        st.set_icons(
            &[
                Rect {
                    x: 40.0,
                    y: 40.0,
                    w: 64.0,
                    h: 64.0,
                },
                Rect {
                    x: 160.0,
                    y: 300.0,
                    w: 64.0,
                    h: 64.0,
                },
            ],
            &["Belgeler".to_string(), "winter.html".to_string()],
            true,
        );
        for _ in 0..240 {
            st.step(1.0 / 30.0);
        }
        assert!(st.snow.len() <= SNOW_MAX);
        assert!(st.smoke.len() <= SMOKE_MAX);
        assert!(st.icons.len() <= 8);
        assert!(st.smoke.iter().all(|p| p.r.is_finite() && p.y.is_finite()));
        assert!(st.snow.iter().all(|f| f.x.is_finite() && f.y.is_finite()));
        assert!(st.target_snow_count() <= SNOW_MAX);
    }

    #[test]
    fn icon_motion_wakes_snow_and_spawns_trails() {
        let mut st = WinterState::new(1280.0, 720.0, WinterParams::default(), 20261117);
        st.set_icons(
            &[Rect {
                x: 40.0,
                y: 40.0,
                w: 64.0,
                h: 64.0,
            }],
            &["Belgeler".to_string()],
            true,
        );
        st.set_icons(
            &[Rect {
                x: 420.0,
                y: 260.0,
                w: 64.0,
                h: 64.0,
            }],
            &["Belgeler".to_string()],
            true,
        );
        let mut saw_trail = false;
        for _ in 0..60 {
            st.step(1.0 / 30.0);
            if st.trails.iter().any(|t| t.active) {
                saw_trail = true;
            }
        }
        assert!(saw_trail, "moving icon should leave a snow trail");
        assert!(st.icons[0].snow_build <= 1.0 && st.icons[0].cap_build <= 1.0);
    }
}
