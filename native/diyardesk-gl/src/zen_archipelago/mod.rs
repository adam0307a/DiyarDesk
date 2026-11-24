pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use shaders::*;
use std::f64::consts::PI;
use std::ffi::c_void;

pub const SDF_SCALE: f64 = 6.0;
const BODY_WIDTHS: [f64; 9] = [5.5, 6.8, 6.4, 5.4, 4.2, 3.0, 2.0, 1.2, 0.6];

#[derive(Clone, Copy)]
pub struct ZenPalette {
    pub deep: [f64; 3],
    pub mid: [f64; 3],
    pub shallow: [f64; 3],
    pub reef: [f64; 3],
    pub foam_rgb: [f32; 3],
    pub sand: [f64; 3],
    pub grass_outer: [f64; 3],
    pub grass_inner: [f64; 3],
    pub sakura_main: [f32; 4],
    pub sakura_light: [f32; 4],
    pub bridge_wood: [f32; 4],
    pub bridge_rail: [f32; 4],
    pub ambient_shadow: [f32; 4],
    pub sun_dir: [f64; 2],
}

const fn rgba_u8(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

const fn rgb_u8(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

// Order matches TIME_MODES = ["night", "sunset", "day"] and themes.json default 0 = Night
pub const PALETTES: [ZenPalette; 3] = [
    // 0: Bioluminescent Night
    ZenPalette {
        deep: [6.0, 17.0, 30.0],
        mid: [10.0, 33.0, 54.0],
        shallow: [16.0, 74.0, 102.0],
        reef: [28.0, 128.0, 148.0],
        foam_rgb: rgb_u8(95, 245, 255),
        sand: [39.0, 59.0, 74.0],         // #273b4a
        grass_outer: [27.0, 67.0, 60.0],  // #1b433c
        grass_inner: [40.0, 97.0, 83.0],  // #286153
        sakura_main: rgba_u8(255, 133, 179, 1.0),  // #ff85b3
        sakura_light: rgba_u8(255, 194, 217, 1.0), // #ffc2d9
        bridge_wood: rgba_u8(110, 43, 43, 1.0),    // #6e2b2b
        bridge_rail: rgba_u8(158, 59, 51, 1.0),    // #9e3b33
        ambient_shadow: rgba_u8(2, 8, 16, 0.55),
        sun_dir: [0.65, 0.55],
    },
    // 1: Golden Sunset
    ZenPalette {
        deep: [22.0, 20.0, 42.0],
        mid: [44.0, 34.0, 64.0],
        shallow: [86.0, 62.0, 88.0],
        reef: [145.0, 96.0, 104.0],
        foam_rgb: rgb_u8(255, 215, 175),
        sand: [140.0, 109.0, 83.0],       // #8c6d53
        grass_outer: [61.0, 90.0, 59.0],  // #3d5a3b
        grass_inner: [92.0, 122.0, 70.0], // #5c7a46
        sakura_main: rgba_u8(255, 122, 158, 1.0),  // #ff7a9e
        sakura_light: rgba_u8(255, 209, 220, 1.0), // #ffd1dc
        bridge_wood: rgba_u8(122, 48, 40, 1.0),    // #7a3028
        bridge_rail: rgba_u8(176, 70, 54, 1.0),    // #b04636
        ambient_shadow: rgba_u8(18, 10, 24, 0.45),
        sun_dir: [0.85, 0.45],
    },
    // 2: Emerald Day
    ZenPalette {
        deep: [11.0, 58.0, 78.0],
        mid: [18.0, 92.0, 114.0],
        shallow: [38.0, 148.0, 158.0],
        reef: [78.0, 198.0, 185.0],
        foam_rgb: rgb_u8(235, 252, 255),
        sand: [216.0, 195.0, 152.0],      // #d8c398
        grass_outer: [73.0, 132.0, 72.0], // #498448
        grass_inner: [106.0, 173.0, 89.0],// #6aad59
        sakura_main: rgba_u8(255, 148, 184, 1.0),  // #ff94b8
        sakura_light: rgba_u8(255, 227, 237, 1.0), // #ffe3ed
        bridge_wood: rgba_u8(140, 56, 43, 1.0),    // #8c382b
        bridge_rail: rgba_u8(189, 79, 60, 1.0),    // #bd4f3c
        ambient_shadow: rgba_u8(8, 32, 42, 0.34),
        sun_dir: [0.55, 0.65],
    },
];

#[derive(Clone, Copy)]
struct KoiPattern {
    base: [f32; 3],
    spot0: [f32; 3],
    spot1: [f32; 3],
    head_spot: bool,
}

const KOI_PATTERNS: [KoiPattern; 4] = [
    // Kohaku (White + Red)
    KoiPattern {
        base: rgb_u8(247, 249, 252),  // #f7f9fc
        spot0: rgb_u8(232, 63, 37),   // #e83f25
        spot1: rgb_u8(240, 84, 35),   // #f05423
        head_spot: true,
    },
    // Sanke (White + Red + Black)
    KoiPattern {
        base: rgb_u8(247, 249, 252),  // #f7f9fc
        spot0: rgb_u8(232, 63, 37),   // #e83f25
        spot1: rgb_u8(28, 34, 43),    // #1c222b
        head_spot: false,
    },
    // Yamabuki / Gold
    KoiPattern {
        base: rgb_u8(255, 158, 44),   // #ff9e2c
        spot0: rgb_u8(255, 255, 255), // #ffffff
        spot1: rgb_u8(217, 56, 30),   // #d9381e
        head_spot: true,
    },
    // Showa
    KoiPattern {
        base: rgb_u8(35, 42, 54),     // #232a36
        spot0: rgb_u8(255, 89, 38),   // #ff5926
        spot1: rgb_u8(247, 249, 252), // #f7f9fc
        head_spot: false,
    },
];

#[derive(Clone, Copy, PartialEq)]
pub struct ZenArchipelagoParams {
    pub adaptive: bool,
    pub time_of_day: usize, // 0 = night, 1 = sunset, 2 = day
    pub koi_count: usize,
    pub lantern_count: usize,
    pub island_margin: f64,
    pub bridge_distance: f64,
    pub wave_intensity: f64,
    pub sakura_density: f64,
    pub fireflies: usize,
}

impl Default for ZenArchipelagoParams {
    fn default() -> Self {
        Self {
            adaptive: true,
            time_of_day: 0, // night
            koi_count: 14,
            lantern_count: 10,
            island_margin: 44.0,
            bridge_distance: 230.0,
            wave_intensity: 1.0,
            sakura_density: 1.0,
            fireflies: 45,
        }
    }
}

/// Exact match to prototype/zen-archipelago.html:140-146 `hash2(x, y)`
#[inline]
pub fn hash2(x: i32, y: i32) -> f64 {
    let mut h: u32 = ((x as u32) ^ 0x27d4_eb2d).wrapping_mul(0x85eb_ca6b)
        ^ ((y as u32) ^ 0x1656_67b1).wrapping_mul(0xc2b2_ae35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h as f64) / 4294967296.0
}

#[inline]
pub fn value_noise(x: f64, y: f64) -> f64 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - (xi as f64);
    let yf = y - (yi as f64);
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let a = hash2(xi, yi);
    let b = hash2(xi + 1, yi);
    let c = hash2(xi, yi + 1);
    let d = hash2(xi + 1, yi + 1);
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

#[inline]
pub fn fbm(x: f64, y: f64) -> f64 {
    value_noise(x, y) * 0.55
        + value_noise(x * 2.03, y * 2.03) * 0.30
        + value_noise(x * 4.07, y * 4.07) * 0.15
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[derive(Clone, Debug)]
struct IconNode {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    cx: f64,
    cy: f64,
    r: f64,
    tx: f64,
    ty: f64,
    tw: f64,
    th: f64,
    tcx: f64,
    tcy: f64,
    tr: f64,
    alpha: f64,
    has_target: bool,
}

#[derive(Clone, Copy, Debug)]
struct Bridge {
    x1: f64,
    y1: f64,
    ang: f64,
    len: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FeatureKind {
    Sakura,
    Toro,
    Rock,
}

#[derive(Clone, Copy, Debug)]
struct IslandFeature {
    kind: FeatureKind,
    x: f64,
    y: f64,
    size: f64,
    seed: f64,
}

#[derive(Clone, Copy, Debug)]
struct LilyPad {
    x: f64,
    y: f64,
    r: f64,
    rot: f64,
    has_flower: bool,
    flower_color: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
struct Koi {
    x: f64,
    y: f64,
    ang: f64,
    target_ang: f64,
    speed: f64,
    scale: f64,
    seg_len: f64,
    spine: [[f64; 2]; 9],
    swim_phase: f64,
    pattern_idx: usize,
    seed: f64,
}

#[derive(Clone, Copy, Debug)]
struct Lantern {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    rot: f64,
    vrot: f64,
    pulse: f64,
    size: f64,
}

#[derive(Clone, Copy, Debug)]
struct Ripple {
    x: f64,
    y: f64,
    r: f64,
    speed: f64,
    alpha: f64,
    fade: f64,
}

#[derive(Clone, Copy, Debug)]
struct Petal {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    rot: f64,
    phase: f64,
    size: f64,
    is_firefly: bool,
}

#[derive(Clone, Copy, Debug)]
struct Bait {
    x: f64,
    y: f64,
    life: f64,
}

struct Rng64(u64);

impl Rng64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    #[inline]
    fn next_f64(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        ((x >> 11) as f64) * (1.0 / ((1u64 << 53) as f64))
    }
}

pub struct ZenArchipelagoState {
    pub params: ZenArchipelagoParams,
    pub w: f64,
    pub h: f64,
    pub time: f64,
    sdf_w: usize,
    sdf_h: usize,
    sdf_grid: Vec<f32>,
    sdf_rgba: Vec<u8>,
    terrain_dirty: bool,
    terrain_rev: u64,
    last_raw_icons: Vec<Rect>,
    last_icons_vis: bool,
    current_icons: Vec<IconNode>,
    bridges: Vec<Bridge>,
    island_features: Vec<IslandFeature>,
    lily_pads: Vec<LilyPad>,
    kois: Vec<Koi>,
    lanterns: Vec<Lantern>,
    ripples: Vec<Ripple>,
    petals: Vec<Petal>,
    baits: Vec<Bait>,
    rng: Rng64,
}

impl Default for ZenArchipelagoState {
    fn default() -> Self {
        let mut s = Self {
            params: ZenArchipelagoParams::default(),
            w: 1920.0,
            h: 1080.0,
            time: 0.0,
            sdf_w: 0,
            sdf_h: 0,
            sdf_grid: Vec::new(),
            sdf_rgba: Vec::new(),
            terrain_dirty: true,
            terrain_rev: 0,
            last_raw_icons: Vec::new(),
            last_icons_vis: false,
            current_icons: Vec::new(),
            bridges: Vec::new(),
            island_features: Vec::new(),
            lily_pads: Vec::new(),
            kois: Vec::new(),
            lanterns: Vec::new(),
            ripples: Vec::new(),
            petals: Vec::new(),
            baits: Vec::new(),
            rng: Rng64::new(0x5A45_4E41_5243_4849),
        };
        s.rebuild_terrain();
        s.sync_entity_counts();
        s
    }
}

impl ZenArchipelagoState {
    /// Only marks terrain dirty when dimensions actually change (avoids per-frame SDF recompute).
    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = (w as f64).max(64.0);
        let nh = (h as f64).max(64.0);
        if (nw - self.w).abs() > 0.5 || (nh - self.h).abs() > 0.5 {
            let sx = if self.w > 1.0 { nw / self.w } else { 1.0 };
            let sy = if self.h > 1.0 { nh / self.h } else { 1.0 };
            self.w = nw;
            self.h = nh;
            for k in &mut self.kois {
                k.x *= sx;
                k.y *= sy;
                for p in &mut k.spine {
                    p[0] *= sx;
                    p[1] *= sy;
                }
            }
            for l in &mut self.lanterns {
                l.x *= sx;
                l.y *= sy;
            }
            for p in &mut self.petals {
                p.x *= sx;
                p.y *= sy;
            }
            self.terrain_dirty = true;
        }
    }

    pub fn sync_params(&mut self, next: ZenArchipelagoParams) {
        let prev = self.params;
        let adaptive_changed = prev.adaptive != next.adaptive;
        let terrain_changed = prev.time_of_day != next.time_of_day
            || (prev.island_margin - next.island_margin).abs() > 1e-4
            || (prev.bridge_distance - next.bridge_distance).abs() > 1e-4
            || (prev.sakura_density - next.sakura_density).abs() > 1e-4;
        self.params = next;

        if adaptive_changed {
            let raw = self.last_raw_icons.clone();
            let vis = self.last_icons_vis;
            self.apply_icons_internal(&raw, vis);
        }
        if terrain_changed {
            for ic in &mut self.current_icons {
                ic.tr = ic.tw.max(ic.th) * 0.48 + self.params.island_margin;
                ic.has_target = true;
            }
            self.terrain_dirty = true;
        }
        self.sync_entity_counts();
    }

    pub fn set_icons(&mut self, raw: &[Rect], vis: bool) {
        self.last_raw_icons.clear();
        if vis {
            self.last_raw_icons.extend_from_slice(raw);
        }
        self.last_icons_vis = vis;
        self.apply_icons_internal(raw, vis);
    }

    fn apply_icons_internal(&mut self, raw: &[Rect], vis: bool) {
        let effective: &[Rect] = if self.params.adaptive && vis { raw } else { &[] };

        // Quick check: if targets are already identical, skip dirtying.
        if effective.len() == self.current_icons.len()
            && self.current_icons.iter().zip(effective.iter()).all(|(c, r)| {
                (c.tx - r.x as f64).abs() < 0.25
                    && (c.ty - r.y as f64).abs() < 0.25
                    && (c.tw - r.w as f64).abs() < 0.25
                    && (c.th - r.h as f64).abs() < 0.25
            })
        {
            return;
        }

        let targets: Vec<IconNode> = effective
            .iter()
            .map(|r| {
                let x = r.x as f64;
                let y = r.y as f64;
                let w = (r.w as f64).max(16.0);
                let h = (r.h as f64).max(16.0);
                let cx = x + w * 0.5;
                let cy = y + h * 0.5;
                let rad = w.max(h) * 0.48 + self.params.island_margin;
                IconNode {
                    x,
                    y,
                    w,
                    h,
                    cx,
                    cy,
                    r: rad,
                    tx: x,
                    ty: y,
                    tw: w,
                    th: h,
                    tcx: cx,
                    tcy: cy,
                    tr: rad,
                    alpha: 1.0,
                    has_target: false,
                }
            })
            .collect();

        if self.current_icons.is_empty() {
            self.current_icons = targets;
            self.terrain_dirty = true;
            return;
        }

        // Match existing icons by nearest center (< 350px) for smooth morphing (matching HTML)
        let mut next: Vec<IconNode> = Vec::with_capacity(targets.len());
        let mut used = vec![false; self.current_icons.len()];
        for t in targets {
            let mut best_idx: Option<usize> = None;
            let mut best_dist = 1e9_f64;
            for (i, c) in self.current_icons.iter().enumerate() {
                if used[i] {
                    continue;
                }
                let d = (c.cx - t.cx).hypot(c.cy - t.cy);
                if d < best_dist {
                    best_dist = d;
                    best_idx = Some(i);
                }
            }
            if let Some(bi) = best_idx {
                if best_dist < 350.0 {
                    used[bi] = true;
                    let mut node = self.current_icons[bi].clone();
                    node.tx = t.x;
                    node.ty = t.y;
                    node.tw = t.w;
                    node.th = t.h;
                    node.tcx = t.cx;
                    node.tcy = t.cy;
                    node.tr = t.r;
                    node.has_target = true;
                    next.push(node);
                    continue;
                }
            }
            let mut spawned = t;
            spawned.alpha = 0.05;
            spawned.has_target = true;
            next.push(spawned);
        }

        self.current_icons = next;
        self.terrain_dirty = true;
    }

    pub fn on_pointer_down(&mut self, x: f64, y: f64) {
        self.add_ripple(x, y, 52.0, 0.75);
        if self.sample_sdf(x, y) < -5.0 {
            for _ in 0..3 {
                let bx = x + (self.rng.next_f64() - 0.5) * 18.0;
                let by = y + (self.rng.next_f64() - 0.5) * 18.0;
                let life = 6.0 + self.rng.next_f64() * 3.0;
                self.baits.push(Bait { x: bx, y: by, life });
            }
        }
    }

    #[inline]
    pub fn sample_sdf(&self, px: f64, py: f64) -> f64 {
        if self.sdf_w == 0 || self.sdf_h == 0 {
            return -100.0;
        }
        let gx = ((px / SDF_SCALE) as isize).clamp(0, self.sdf_w as isize - 1) as usize;
        let gy = ((py / SDF_SCALE) as isize).clamp(0, self.sdf_h as isize - 1) as usize;
        self.sdf_grid[gy * self.sdf_w + gx] as f64
    }

    #[inline]
    fn sample_sdf_gradient(&self, px: f64, py: f64) -> (f64, f64) {
        let eps = SDF_SCALE * 1.5;
        let dx = self.sample_sdf(px + eps, py) - self.sample_sdf(px - eps, py);
        let dy = self.sample_sdf(px, py + eps) - self.sample_sdf(px, py - eps);
        let len = dx.hypot(dy);
        if len > 1e-9 {
            (dx / len, dy / len)
        } else {
            (dx, dy)
        }
    }

    fn step_icon_interpolation(&mut self, dt: f64) {
        let mut moved = false;
        let speed = (dt * 6.5).min(1.0);
        for ic in &mut self.current_icons {
            if ic.has_target {
                let dx = ic.tx - ic.x;
                let dy = ic.ty - ic.y;
                let dw = ic.tw - ic.w;
                let dh = ic.th - ic.h;
                let dr = ic.tr - ic.r;
                if dx.abs() > 0.35
                    || dy.abs() > 0.35
                    || dw.abs() > 0.35
                    || dh.abs() > 0.35
                    || dr.abs() > 0.35
                    || ic.alpha < 0.99
                {
                    ic.x += dx * speed;
                    ic.y += dy * speed;
                    ic.w += dw * speed;
                    ic.h += dh * speed;
                    ic.cx = ic.x + ic.w * 0.5;
                    ic.cy = ic.y + ic.h * 0.5;
                    ic.r += dr * speed;
                    ic.alpha = (ic.alpha + dt * 2.5).min(1.0);
                    moved = true;
                } else {
                    ic.x = ic.tx;
                    ic.y = ic.ty;
                    ic.w = ic.tw;
                    ic.h = ic.th;
                    ic.cx = ic.tcx;
                    ic.cy = ic.tcy;
                    ic.r = ic.tr;
                    ic.alpha = 1.0;
                    ic.has_target = false;
                    moved = true; // snap final frame cleanly
                }
            }
        }
        if moved || self.terrain_dirty {
            self.rebuild_terrain();
            self.terrain_dirty = false;
        }
    }

    pub fn ensure_terrain_fresh(&mut self) {
        if self.terrain_dirty || self.sdf_w == 0 || self.sdf_h == 0 {
            self.rebuild_terrain();
            self.terrain_dirty = false;
        }
    }

    pub fn rebuild_terrain(&mut self) {
        if self.w <= 0.0 || self.h <= 0.0 {
            return;
        }
        let sw = (self.w / SDF_SCALE).ceil().max(1.0) as usize;
        let sh = (self.h / SDF_SCALE).ceil().max(1.0) as usize;
        self.sdf_w = sw;
        self.sdf_h = sh;
        let total = sw * sh;
        if self.sdf_grid.len() != total {
            self.sdf_grid.resize(total, -100.0);
        }
        if self.sdf_rgba.len() != total * 4 {
            self.sdf_rgba.resize(total * 4, 255);
        }

        let pal = PALETTES[self.params.time_of_day.min(2)];
        let k = 38.0;

        // 1. Compute Smooth Metaball SDF Grid around desktop icons
        for gy in 0..sh {
            let py = (gy as f64 + 0.5) * SDF_SCALE;
            let row = gy * sw;
            for gx in 0..sw {
                let px = (gx as f64 + 0.5) * SDF_SCALE;
                let mut res = 0.0_f64;
                for ic in &self.current_icons {
                    let dx = px - ic.cx;
                    let dy = py - ic.cy;
                    let qx = dx.abs() - ic.w * 0.28;
                    let qy = dy.abs() - ic.h * 0.30;
                    let box_dist = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0);
                    let rad = self.params.island_margin * ic.alpha;
                    let d = box_dist - rad;
                    res += (-d / k).exp();
                }
                let raw_dist = if res > 1e-9 { -k * res.ln() } else { 500.0 };
                let n = (fbm(px * 0.014, py * 0.014) - 0.5) * 26.0;
                self.sdf_grid[row + gx] = -(raw_dist + n) as f32;
            }
        }

        // 2. Render Low-Res Pixel Map for Deep Water -> Reef -> Sand -> Mossy Island
        for gy in 0..sh {
            let row = gy * sw;
            for gx in 0..sw {
                let idx = row + gx;
                let v = self.sdf_grid[idx] as f64;
                let n = value_noise(gx as f64 * 0.12, gy as f64 * 0.12);
                let (mut r, mut g, mut b) = if v < -95.0 {
                    let t = ((-v - 95.0) / 180.0).min(1.0);
                    (
                        lerp(pal.mid[0], pal.deep[0], t),
                        lerp(pal.mid[1], pal.deep[1], t),
                        lerp(pal.mid[2], pal.deep[2], t),
                    )
                } else if v < -32.0 {
                    let t = (v + 95.0) / 63.0;
                    (
                        lerp(pal.mid[0], pal.shallow[0], t),
                        lerp(pal.mid[1], pal.shallow[1], t),
                        lerp(pal.mid[2], pal.shallow[2], t),
                    )
                } else if v < 0.0 {
                    let t = (v + 32.0) / 32.0;
                    (
                        lerp(pal.shallow[0], pal.reef[0], t),
                        lerp(pal.shallow[1], pal.reef[1], t),
                        lerp(pal.shallow[2], pal.reef[2], t),
                    )
                } else if v < 7.0 {
                    let t = v / 7.0;
                    let u = t * 0.85 + 0.15;
                    (
                        lerp(pal.reef[0], pal.sand[0], u),
                        lerp(pal.reef[1], pal.sand[1], u),
                        lerp(pal.reef[2], pal.sand[2], u),
                    )
                } else if v < 22.0 {
                    let t = (v - 7.0) / 15.0;
                    (
                        lerp(pal.sand[0], pal.grass_outer[0], t),
                        lerp(pal.sand[1], pal.grass_outer[1], t),
                        lerp(pal.sand[2], pal.grass_outer[2], t),
                    )
                } else {
                    let t = ((v - 22.0) / 30.0).min(1.0) * 0.7 + n * 0.3;
                    (
                        lerp(pal.grass_outer[0], pal.grass_inner[0], t),
                        lerp(pal.grass_outer[1], pal.grass_inner[1], t),
                        lerp(pal.grass_outer[2], pal.grass_inner[2], t),
                    )
                };

                // Subtle directional hill-shading on land
                if v > 0.0 && gx > 0 && gy > 0 {
                    let slope = (self.sdf_grid[idx] as f64 - self.sdf_grid[idx - 1] as f64)
                        * pal.sun_dir[0]
                        + (self.sdf_grid[idx] as f64 - self.sdf_grid[idx - sw] as f64)
                            * pal.sun_dir[1];
                    let shade = (slope * 2.2).clamp(-18.0, 18.0);
                    r = (r + shade).clamp(0.0, 255.0);
                    g = (g + shade).clamp(0.0, 255.0);
                    b = (b + shade).clamp(0.0, 255.0);
                }

                let p = idx * 4;
                self.sdf_rgba[p] = r.clamp(0.0, 255.0) as u8;
                self.sdf_rgba[p + 1] = g.clamp(0.0, 255.0) as u8;
                self.sdf_rgba[p + 2] = b.clamp(0.0, 255.0) as u8;
                self.sdf_rgba[p + 3] = 255;
            }
        }

        // 3. Build MST Bridges, Island Features & Lily Pads
        self.build_bridges_and_features();
        self.terrain_rev = self.terrain_rev.wrapping_add(1);
    }

    fn build_bridges_and_features(&mut self) {
        self.bridges.clear();
        self.island_features.clear();
        self.lily_pads.clear();

        let n = self.current_icons.len();
        let max_dist = self.params.bridge_distance;

        let mut edges: Vec<(usize, usize, f64)> = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                let a = &self.current_icons[i];
                let b = &self.current_icons[j];
                let d = (a.cx - b.cx).hypot(a.cy - b.cy);
                if d > 85.0 && d < max_dist {
                    edges.push((i, j, d));
                }
            }
        }
        edges.sort_by(|a, b| a.2.total_cmp(&b.2));
        let mut degree = vec![0i32; n];

        for (ei, ej, _) in edges {
            if degree[ei] >= 2 || degree[ej] >= 2 {
                continue;
            }
            let a = &self.current_icons[ei];
            let b = &self.current_icons[ej];
            let mid_x = (a.cx + b.cx) * 0.5;
            let mid_y = (a.cy + b.cy) * 0.5;
            if self.sample_sdf(mid_x, mid_y) < 16.0 {
                let ang = (b.cy - a.cy).atan2(b.cx - a.cx);
                let sx = a.cx + ang.cos() * (a.w * 0.42);
                let sy = a.cy + ang.sin() * (a.h * 0.42);
                let ex = b.cx - ang.cos() * (b.w * 0.42);
                let ey = b.cy - ang.sin() * (b.h * 0.42);
                self.bridges.push(Bridge {
                    x1: sx,
                    y1: sy,
                    ang,
                    len: (ex - sx).hypot(ey - sy),
                });
                degree[ei] += 1;
                degree[ej] += 1;
            }
        }

        // Deterministic Sakura trees, stone lanterns (Toro), and coastal rocks per icon
        for (i, ic) in self.current_icons.iter().enumerate() {
            let seed = (ic.cx * 0.13 + ic.cy * 0.29).floor() as i32 + (i as i32) * 17;
            let h1 = hash2(seed, 1);
            let h2 = hash2(seed, 2);
            let h3 = hash2(seed, 3);

            let corner_ang = h1 * PI * 2.0;
            let dist = ic.w.max(ic.h) * 0.54 + self.params.island_margin * 0.35;
            let fx = ic.cx + corner_ang.cos() * dist;
            let fy = ic.cy + corner_ang.sin() * dist;

            if self.params.sakura_density > 0.0 && h2 < 0.72 * self.params.sakura_density {
                self.island_features.push(IslandFeature {
                    kind: FeatureKind::Sakura,
                    x: fx,
                    y: fy,
                    size: 16.0 + h3 * 12.0,
                    seed: h1 * 100.0,
                });
            } else {
                self.island_features.push(IslandFeature {
                    kind: FeatureKind::Toro,
                    x: fx,
                    y: fy,
                    size: 9.0 + h3 * 4.0,
                    seed: h2 * 100.0,
                });
            }

            let rock_ang = corner_ang + 2.2;
            let rx = ic.cx + rock_ang.cos() * (dist + 6.0);
            let ry = ic.cy + rock_ang.sin() * (dist + 6.0);
            self.island_features.push(IslandFeature {
                kind: FeatureKind::Rock,
                x: rx,
                y: ry,
                size: 6.0 + h1 * 6.0,
                seed: seed as f64,
            });
        }

        // Lily Pads in shallow reef zones (-48 < SDF < -10)
        for i in 0..90i32 {
            let px = hash2(i, 77) * (self.w - 80.0).max(1.0) + 40.0;
            let py = hash2(i, 99) * (self.h - 80.0).max(1.0) + 40.0;
            let sdf = self.sample_sdf(px, py);
            if sdf > -48.0 && sdf < -10.0 {
                let too_close = self
                    .lily_pads
                    .iter()
                    .any(|lp| (lp.x - px).hypot(lp.y - py) < 26.0);
                if !too_close {
                    let flower_color = if hash2(i, 88) < 0.5 {
                        rgba_u8(255, 158, 187, 1.0) // #ff9ebb
                    } else {
                        rgba_u8(255, 243, 214, 1.0) // #fff3d6
                    };
                    self.lily_pads.push(LilyPad {
                        x: px,
                        y: py,
                        r: 6.0 + hash2(i, 12) * 6.0,
                        rot: hash2(i, 34) * PI * 2.0,
                        has_flower: hash2(i, 56) < 0.28,
                        flower_color,
                    });
                }
            }
        }
    }

    fn create_koi(&mut self, i: usize) -> Koi {
        let mut x = hash2(i as i32, 301) * self.w;
        let mut y = hash2(i as i32, 302) * self.h;
        for _ in 0..15 {
            if self.sample_sdf(x, y) < -18.0 {
                break;
            }
            x = self.rng.next_f64() * self.w;
            y = self.rng.next_f64() * self.h;
        }
        let ang = self.rng.next_f64() * PI * 2.0;
        let scale = 0.78 + ((i % 5) as f64) * 0.11;
        let seg_len = 5.2 * scale;
        let mut spine = [[0.0; 2]; 9];
        for s in 0..9 {
            spine[s] = [
                x - ang.cos() * (s as f64) * seg_len,
                y - ang.sin() * (s as f64) * seg_len,
            ];
        }
        Koi {
            x,
            y,
            ang,
            target_ang: ang,
            speed: (28.0 + ((i % 4) as f64) * 7.0) * scale,
            scale,
            seg_len,
            spine,
            swim_phase: self.rng.next_f64() * PI * 2.0,
            pattern_idx: i % KOI_PATTERNS.len(),
            seed: (i as f64) * 31.0,
        }
    }

    fn create_lantern(&mut self, i: usize) -> Lantern {
        let x = hash2(i as i32, 401) * self.w;
        let y = hash2(i as i32, 402) * self.h;
        Lantern {
            x,
            y,
            vx: (self.rng.next_f64() - 0.5) * 8.0,
            vy: (self.rng.next_f64() - 0.5) * 8.0,
            rot: self.rng.next_f64() * PI * 2.0,
            vrot: (self.rng.next_f64() - 0.5) * 0.25,
            pulse: self.rng.next_f64() * PI * 2.0,
            size: 8.0 + ((i % 3) as f64) * 2.0,
        }
    }

    fn create_petal(&mut self, i: usize) -> Petal {
        Petal {
            x: self.rng.next_f64() * self.w,
            y: self.rng.next_f64() * self.h,
            vx: 10.0 + self.rng.next_f64() * 14.0,
            vy: 6.0 + self.rng.next_f64() * 10.0,
            rot: self.rng.next_f64() * PI * 2.0,
            phase: self.rng.next_f64() * PI * 2.0,
            size: 2.5 + self.rng.next_f64() * 2.5,
            is_firefly: i % 2 == 0,
        }
    }

    fn sync_entity_counts(&mut self) {
        while self.kois.len() < self.params.koi_count {
            let idx = self.kois.len();
            let k = self.create_koi(idx);
            self.kois.push(k);
        }
        self.kois.truncate(self.params.koi_count);

        while self.lanterns.len() < self.params.lantern_count {
            let idx = self.lanterns.len();
            let l = self.create_lantern(idx);
            self.lanterns.push(l);
        }
        self.lanterns.truncate(self.params.lantern_count);

        let total_particles =
            (self.params.fireflies as f64 + self.params.sakura_density * 25.0).round() as usize;
        while self.petals.len() < total_particles {
            let idx = self.petals.len();
            let p = self.create_petal(idx);
            self.petals.push(p);
        }
        self.petals.truncate(total_particles);
    }

    fn add_ripple(&mut self, x: f64, y: f64, max_r: f64, alpha: f64) {
        if self.ripples.len() > 45 {
            self.ripples.remove(0);
        }
        self.ripples.push(Ripple {
            x,
            y,
            r: 2.0,
            speed: max_r * 0.85,
            alpha,
            fade: alpha * 0.7,
        });
    }

    pub fn step(&mut self, dt_in: f32) {
        let dt = (dt_in as f64).clamp(0.0, 0.1);
        self.time += dt;
        self.step_icon_interpolation(dt);
        self.sync_entity_counts();

        // Update Ripples
        for rp in &mut self.ripples {
            rp.r += rp.speed * dt;
            rp.alpha -= dt * rp.fade;
        }
        self.ripples.retain(|rp| rp.alpha > 0.0);

        // Update Baits
        for b in &mut self.baits {
            b.life -= dt;
        }
        self.baits.retain(|b| b.life > 0.0);

        // Update Koi Fish with Island Avoidance & Verlet Spine Kinematics
        let w = self.w;
        let h = self.h;
        let time = self.time;
        let mut pending_ripples: Vec<(f64, f64, f64, f64)> = Vec::new();
        let has_baits = !self.baits.is_empty();

        for i in 0..self.kois.len() {
            let mut k = self.kois[i];
            k.swim_phase += dt * (4.8 + k.speed * 0.05);

            let look_dist = 38.0 * k.scale;
            let ahead_x = k.x + k.ang.cos() * look_dist;
            let ahead_y = k.y + k.ang.sin() * look_dist;
            let sdf_ahead = self.sample_sdf(ahead_x, ahead_y);

            if sdf_ahead > -16.0 {
                let (gx, gy) = self.sample_sdf_gradient(ahead_x, ahead_y);
                let escape_ang = (-gy).atan2(-gx);
                k.target_ang = escape_ang + (time + k.seed).sin() * 0.35;
            } else if has_baits {
                let mut best_idx: Option<usize> = None;
                let mut best_d = 320.0;
                for (bi, b) in self.baits.iter().enumerate() {
                    let d = (b.x - k.x).hypot(b.y - k.y);
                    if d < best_d {
                        best_d = d;
                        best_idx = Some(bi);
                    }
                }
                if let Some(bi) = best_idx {
                    let bx = self.baits[bi].x;
                    let by = self.baits[bi].y;
                    k.target_ang = (by - k.y).atan2(bx - k.x);
                    if best_d < 18.0 {
                        self.baits[bi].life -= dt * 2.5;
                        if self.rng.next_f64() < 0.15 {
                            pending_ripples.push((bx, by, 22.0, 0.55));
                        }
                    }
                }
            } else if self.rng.next_f64() < 0.02 {
                k.target_ang += (self.rng.next_f64() - 0.5) * 1.1;
            }

            // Screen boundary steering
            let margin = 45.0;
            if k.x < margin {
                k.target_ang = 0.0;
            } else if k.x > w - margin {
                k.target_ang = PI;
            }
            if k.y < margin {
                k.target_ang = PI * 0.5;
            } else if k.y > h - margin {
                k.target_ang = -PI * 0.5;
            }

            // Smooth angle turn
            let diff = ((k.target_ang - k.ang + PI * 3.0).rem_euclid(PI * 2.0)) - PI;
            k.ang += diff * (dt * 3.8).min(1.0);

            let wiggle = k.swim_phase.sin() * 0.14;
            let move_ang = k.ang + wiggle;
            let cur_speed = if has_baits { k.speed * 1.45 } else { k.speed };

            k.x += move_ang.cos() * cur_speed * dt;
            k.y += move_ang.sin() * cur_speed * dt;

            // Emergency push out if trapped inside newly moved icon island
            let cur_sdf = self.sample_sdf(k.x, k.y);
            if cur_sdf > -6.0 {
                let (gx, gy) = self.sample_sdf_gradient(k.x, k.y);
                k.x -= gx * (cur_sdf + 10.0);
                k.y -= gy * (cur_sdf + 10.0);
            }

            // Inverse kinematics for spine segments
            k.spine[0] = [k.x, k.y];
            for s in 1..9 {
                let prev = k.spine[s - 1];
                let cur = k.spine[s];
                let sway = (k.swim_phase - (s as f64) * 0.65).sin() * ((s as f64) * 0.42 * k.scale);
                let dx = cur[0] - prev[0];
                let dy = cur[1] - prev[1];
                let seg_ang = dy.atan2(dx);
                let perp_x = -seg_ang.sin() * sway * dt * 12.0;
                let perp_y = seg_ang.cos() * sway * dt * 12.0;
                let nx = dx + perp_x;
                let ny = dy + perp_y;
                let dist = nx.hypot(ny);
                let inv = if dist > 1e-9 { k.seg_len / dist } else { k.seg_len };
                k.spine[s] = [prev[0] + nx * inv, prev[1] + ny * inv];
            }

            if self.rng.next_f64() < 0.015 {
                pending_ripples.push((k.x, k.y, 18.0 * k.scale, 0.22));
            }
            self.kois[i] = k;
        }
        for (rx, ry, mr, al) in pending_ripples {
            self.add_ripple(rx, ry, mr, al);
        }

        // Update Floating Water Lanterns
        for i in 0..self.lanterns.len() {
            let mut l = self.lanterns[i];
            l.pulse += dt * 2.2;
            l.rot += l.vrot * dt;
            l.vx += ((time * 0.3 + l.y * 0.01).cos() * 4.0 - l.vx) * dt;
            l.vy += ((time * 0.25 + l.x * 0.01).sin() * 4.0 - l.vy) * dt;

            let nx = l.x + l.vx * dt;
            let ny = l.y + l.vy * dt;
            let sdf = self.sample_sdf(nx, ny);
            if sdf > -12.0 {
                let (gx, gy) = self.sample_sdf_gradient(nx, ny);
                l.vx = -gx * 12.0;
                l.vy = -gy * 12.0;
            } else {
                l.x = nx.rem_euclid(w.max(1.0));
                l.y = ny.rem_euclid(h.max(1.0));
            }
            self.lanterns[i] = l;
        }

        // Update Sakura Petals & Bioluminescent Fireflies
        for p in &mut self.petals {
            p.phase += dt * 2.4;
            p.rot += dt * 1.2;
            p.x += (p.vx + p.phase.sin() * 8.0) * dt;
            p.y += (p.vy + (p.phase * 0.8).cos() * 6.0) * dt;
            if p.x > w + 20.0 {
                p.x = -20.0;
            }
            if p.y > h + 20.0 {
                p.y = -20.0;
            }
        }
    }
}

// ============================================================================
// OpenGL 3.3 Renderer for Zen Archipelago (1:1 HTML Fidelity)
// ============================================================================

fn build_radial_glow_rgba(
    size: usize,
    inner: [f64; 4],
    mid: Option<(f64, [f64; 4])>,
    outer: [f64; 4],
) -> Vec<u8> {
    let mut buf = vec![0u8; size * size * 4];
    let r = (size as f64) * 0.5;
    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            // Ensure 1px outer border is strictly transparent so GL_CLAMP_TO_EDGE never bleeds a box edge
            if x == 0 || y == 0 || x + 1 == size || y + 1 == size {
                buf[idx] = outer[0] as u8;
                buf[idx + 1] = outer[1] as u8;
                buf[idx + 2] = outer[2] as u8;
                buf[idx + 3] = 0;
                continue;
            }
            let dx = (x as f64 + 0.5) - r;
            let dy = (y as f64 + 0.5) - r;
            let d = dx.hypot(dy);
            let t = ((d - 1.0) / (r - 1.0)).clamp(0.0, 1.0);
            let col = if let Some((mt, mc)) = mid {
                if t <= mt {
                    let u = t / mt.max(1e-6);
                    [
                        lerp(inner[0], mc[0], u),
                        lerp(inner[1], mc[1], u),
                        lerp(inner[2], mc[2], u),
                        lerp(inner[3], mc[3], u),
                    ]
                } else {
                    let u = (t - mt) / (1.0 - mt).max(1e-6);
                    [
                        lerp(mc[0], outer[0], u),
                        lerp(mc[1], outer[1], u),
                        lerp(mc[2], outer[2], u),
                        lerp(mc[3], outer[3], u),
                    ]
                }
            } else {
                [
                    lerp(inner[0], outer[0], t),
                    lerp(inner[1], outer[1], t),
                    lerp(inner[2], outer[2], t),
                    lerp(inner[3], outer[3], t),
                ]
            };
            buf[idx] = col[0].clamp(0.0, 255.0).round() as u8;
            buf[idx + 1] = col[1].clamp(0.0, 255.0).round() as u8;
            buf[idx + 2] = col[2].clamp(0.0, 255.0).round() as u8;
            buf[idx + 3] = (col[3].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    buf
}

unsafe fn create_rgba_tex(gl: &GlFns, w: i32, h: i32, rgba: &[u8]) -> GLuint {
    let mut tex = 0;
    (gl.GenTextures)(1, &mut tex);
    (gl.BindTexture)(GL_TEXTURE_2D, tex);
    (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
    (gl.TexImage2D)(
        GL_TEXTURE_2D,
        0,
        GL_RGBA8 as GLint,
        w,
        h,
        0,
        GL_RGBA,
        GL_UNSIGNED_BYTE,
        rgba.as_ptr() as *const c_void,
    );
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    tex
}

pub struct ZenArchipelagoRenderer {
    col_prog: GLuint,
    tex_prog: GLuint,
    koi_prog: GLuint,
    col_vao: GLuint,
    col_vbo: GLuint,
    tex_vao: GLuint,
    tex_vbo: GLuint,
    koi_vao: GLuint,
    koi_vbo: GLuint,
    sdf_tex: GLuint,
    sdf_tex_w: i32,
    sdf_tex_h: i32,
    uploaded_rev: u64,
    lantern_glow_tex: GLuint,
    toro_glow_tex: GLuint,
    col_verts: Vec<f32>,
    tex_verts: Vec<f32>,
    koi_verts: Vec<f32>,
}

impl ZenArchipelagoRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let col_prog = gl.compile_program(ZEN_COLOR_VS, ZEN_COLOR_FS)?;
        let tex_prog = gl.compile_program(ZEN_TEX_VS, ZEN_TEX_FS)?;
        let koi_prog = gl.compile_program(ZEN_KOI_VS, ZEN_KOI_FS)?;

        let mut col_vao = 0;
        let mut col_vbo = 0;
        (gl.GenVertexArrays)(1, &mut col_vao);
        (gl.GenBuffers)(1, &mut col_vbo);
        (gl.BindVertexArray)(col_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, col_vbo);
        let stride_col = (6 * std::mem::size_of::<f32>()) as GLsizei;
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

        let mut tex_vao = 0;
        let mut tex_vbo = 0;
        (gl.GenVertexArrays)(1, &mut tex_vao);
        (gl.GenBuffers)(1, &mut tex_vbo);
        (gl.BindVertexArray)(tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, tex_vbo);
        let stride_tex = (5 * std::mem::size_of::<f32>()) as GLsizei;
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

        let mut koi_vao = 0;
        let mut koi_vbo = 0;
        (gl.GenVertexArrays)(1, &mut koi_vao);
        (gl.GenBuffers)(1, &mut koi_vbo);
        (gl.BindVertexArray)(koi_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, koi_vbo);
        let stride_koi = (2 * std::mem::size_of::<f32>()) as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride_koi, std::ptr::null());

        (gl.BindVertexArray)(0);

        // Matches LANTERN_GLOW_SPRITE & TORO_GLOW_SPRITE in prototype/zen-archipelago.html:126-135
        let lantern_rgba = build_radial_glow_rgba(
            64,
            [255.0, 220.0, 140.0, 0.95],
            Some((0.35, [255.0, 150.0, 60.0, 0.42])),
            [255.0, 120.0, 30.0, 0.0],
        );
        let lantern_glow_tex = create_rgba_tex(gl, 64, 64, &lantern_rgba);

        let toro_rgba = build_radial_glow_rgba(
            64,
            [255.0, 210.0, 120.0, 0.75],
            None,
            [255.0, 160.0, 60.0, 0.0],
        );
        let toro_glow_tex = create_rgba_tex(gl, 64, 64, &toro_rgba);

        let mut sdf_tex = 0;
        (gl.GenTextures)(1, &mut sdf_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, sdf_tex);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            col_prog,
            tex_prog,
            koi_prog,
            col_vao,
            col_vbo,
            tex_vao,
            tex_vbo,
            koi_vao,
            koi_vbo,
            sdf_tex,
            sdf_tex_w: 0,
            sdf_tex_h: 0,
            uploaded_rev: u64::MAX,
            lantern_glow_tex,
            toro_glow_tex,
            col_verts: Vec::with_capacity(16384),
            tex_verts: Vec::with_capacity(2048),
            koi_verts: Vec::with_capacity(256),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        for p in [&mut self.col_prog, &mut self.tex_prog, &mut self.koi_prog] {
            if *p != 0 {
                (gl.DeleteProgram)(*p);
                *p = 0;
            }
        }
        for v in [&mut self.col_vao, &mut self.tex_vao, &mut self.koi_vao] {
            if *v != 0 {
                (gl.DeleteVertexArrays)(1, v);
                *v = 0;
            }
        }
        for b in [&mut self.col_vbo, &mut self.tex_vbo, &mut self.koi_vbo] {
            if *b != 0 {
                (gl.DeleteBuffers)(1, b);
                *b = 0;
            }
        }
        for t in [
            &mut self.sdf_tex,
            &mut self.lantern_glow_tex,
            &mut self.toro_glow_tex,
        ] {
            if *t != 0 {
                (gl.DeleteTextures)(1, t);
                *t = 0;
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
    fn push_quad(
        buf: &mut Vec<f32>,
        a: [f32; 2],
        b: [f32; 2],
        c: [f32; 2],
        d: [f32; 2],
        col: [f32; 4],
    ) {
        Self::push_tri(buf, a, b, c, col);
        Self::push_tri(buf, a, c, d, col);
    }

    fn push_rot_rect(
        buf: &mut Vec<f32>,
        ox: f32,
        oy: f32,
        ang: f32,
        lx: f32,
        ly: f32,
        lw: f32,
        lh: f32,
        col: [f32; 4],
    ) {
        let ca = ang.cos();
        let sa = ang.sin();
        let xf = |x: f32, y: f32| -> [f32; 2] { [ox + x * ca - y * sa, oy + x * sa + y * ca] };
        let p0 = xf(lx, ly);
        let p1 = xf(lx + lw, ly);
        let p2 = xf(lx + lw, ly + lh);
        let p3 = xf(lx, ly + lh);
        Self::push_quad(buf, p0, p1, p2, p3, col);
    }

    fn push_ellipse(
        buf: &mut Vec<f32>,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rot: f32,
        col: [f32; 4],
        segs: usize,
    ) {
        let ca = rot.cos();
        let sa = rot.sin();
        let center = [cx, cy];
        let mut prev = [cx + rx * ca, cy + rx * sa];
        for i in 1..=segs {
            let a = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let lx = a.cos() * rx;
            let ly = a.sin() * ry;
            let cur = [cx + lx * ca - ly * sa, cy + lx * sa + ly * ca];
            Self::push_tri(buf, center, prev, cur, col);
            prev = cur;
        }
    }

    fn push_arc_pie(
        buf: &mut Vec<f32>,
        ox: f32,
        oy: f32,
        rot: f32,
        lcx: f32,
        lcy: f32,
        r: f32,
        a0: f32,
        a1: f32,
        col: [f32; 4],
        segs: usize,
    ) {
        let ca = rot.cos();
        let sa = rot.sin();
        let xf = |x: f32, y: f32| -> [f32; 2] { [ox + x * ca - y * sa, oy + x * sa + y * ca] };
        let center = xf(lcx, lcy);
        let mut prev = xf(lcx + a0.cos() * r, lcy + a0.sin() * r);
        for i in 1..=segs {
            let t = i as f32 / segs as f32;
            let a = a0 + (a1 - a0) * t;
            let cur = xf(lcx + a.cos() * r, lcy + a.sin() * r);
            Self::push_tri(buf, center, prev, cur, col);
            prev = cur;
        }
    }

    fn push_segment(
        buf: &mut Vec<f32>,
        p0: [f32; 2],
        p1: [f32; 2],
        half_w: f32,
        col: [f32; 4],
    ) {
        let dx = p1[0] - p0[0];
        let dy = p1[1] - p0[1];
        let len = dx.hypot(dy);
        if len < 1e-4 {
            return;
        }
        let nx = -dy / len * half_w;
        let ny = dx / len * half_w;
        Self::push_quad(
            buf,
            [p0[0] + nx, p0[1] + ny],
            [p1[0] + nx, p1[1] + ny],
            [p1[0] - nx, p1[1] - ny],
            [p0[0] - nx, p0[1] - ny],
            col,
        );
    }

    fn push_ring(
        buf: &mut Vec<f32>,
        cx: f32,
        cy: f32,
        r: f32,
        line_w: f32,
        col: [f32; 4],
        segs: usize,
    ) {
        let r_in = (r - line_w * 0.5).max(0.0);
        let r_out = r + line_w * 0.5;
        let mut prev_in = [cx + r_in, cy];
        let mut prev_out = [cx + r_out, cy];
        for i in 1..=segs {
            let a = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let ca = a.cos();
            let sa = a.sin();
            let cur_in = [cx + ca * r_in, cy + sa * r_in];
            let cur_out = [cx + ca * r_out, cy + sa * r_out];
            Self::push_quad(buf, prev_in, prev_out, cur_out, cur_in, col);
            prev_in = cur_in;
            prev_out = cur_out;
        }
    }

    fn push_tex_quad(
        buf: &mut Vec<f32>,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        alpha: f32,
    ) {
        buf.extend_from_slice(&[
            x0, y0, 0.0, 0.0, alpha,
            x1, y0, 1.0, 0.0, alpha,
            x1, y1, 1.0, 1.0, alpha,
            x0, y0, 0.0, 0.0, alpha,
            x1, y1, 1.0, 1.0, alpha,
            x0, y1, 0.0, 1.0, alpha,
        ]);
    }

    unsafe fn flush_col(&mut self, gl: &GlFns, w: f32, h: f32) {
        if self.col_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.col_prog);
        (gl.Uniform2f)(gl.uniform_loc(self.col_prog, "uRes"), w, h);
        (gl.BindVertexArray)(self.col_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.col_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.col_verts.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            self.col_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, (self.col_verts.len() / 6) as GLsizei);
        self.col_verts.clear();
    }

    unsafe fn flush_tex(&mut self, gl: &GlFns, tex: GLuint, w: f32, h: f32) {
        if self.tex_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.tex_prog);
        (gl.Uniform2f)(gl.uniform_loc(self.tex_prog, "uRes"), w, h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, tex);
        (gl.Uniform1i)(gl.uniform_loc(self.tex_prog, "uTex"), 0);
        (gl.BindVertexArray)(self.tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.tex_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.tex_verts.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            self.tex_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, (self.tex_verts.len() / 5) as GLsizei);
        self.tex_verts.clear();
    }

    /// Build clean ribbon triangulation of Koi silhouette (never folds/spikes on curved spines!)
    fn build_koi_geometry(
        k: &Koi,
        ox: f64,
        oy: f64,
        out_pts: &mut Vec<f32>,
    ) {
        out_pts.clear();
        let sp = &k.spine;
        let sc = k.scale;

        let mut left = [[0.0f32; 2]; 9];
        let mut right = [[0.0f32; 2]; 9];
        for i in 0..9 {
            let (dx, dy) = if i == 0 {
                (sp[0][0] - sp[1][0], sp[0][1] - sp[1][1])
            } else {
                (sp[i - 1][0] - sp[i][0], sp[i - 1][1] - sp[i][1])
            };
            let len = dx.hypot(dy);
            let inv = if len > 1e-9 { 1.0 / len } else { 1.0 };
            let nx = -dy * inv;
            let ny = dx * inv;
            let w = BODY_WIDTHS[i] * sc;
            left[i] = [(sp[i][0] + nx * w + ox) as f32, (sp[i][1] + ny * w + oy) as f32];
            right[i] = [(sp[i][0] - nx * w + ox) as f32, (sp[i][1] - ny * w + oy) as f32];
        }

        let head_ang = (sp[0][1] - sp[1][1]).atan2(sp[0][0] - sp[1][0]);
        let nose = [
            (sp[0][0] + head_ang.cos() * 4.5 * sc + ox) as f32,
            (sp[0][1] + head_ang.sin() * 4.5 * sc + oy) as f32,
        ];

        let tail_ang = (sp[7][1] - sp[8][1]).atan2(sp[7][0] - sp[8][0]);
        let tail_sway = (k.swim_phase - 2.2).sin() * 4.0 * sc;
        let tail_tip = [
            (sp[8][0] - tail_ang.cos() * 8.0 * sc - tail_ang.sin() * tail_sway + ox) as f32,
            (sp[8][1] - tail_ang.sin() * 8.0 * sc + tail_ang.cos() * tail_sway + oy) as f32,
        ];

        let push = |v: &mut Vec<f32>, a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
            v.extend_from_slice(&[a[0], a[1], b[0], b[1], c[0], c[1]]);
        };

        // Head triangle
        push(out_pts, nose, left[0], right[0]);
        // Spine ribbon quads (8 segments = 16 triangles)
        for i in 0..8 {
            push(out_pts, left[i], left[i + 1], right[i]);
            push(out_pts, left[i + 1], right[i + 1], right[i]);
        }
        // Tail fin triangle
        push(out_pts, left[8], tail_tip, right[8]);
    }

    fn push_koi_fins(buf: &mut Vec<f32>, k: &Koi, ox: f64, oy: f64, col: [f32; 4]) {
        let sp = &k.spine;
        let sc = k.scale as f32;
        let fin_ang = (sp[0][1] - sp[2][1]).atan2(sp[0][0] - sp[2][0]) as f32;
        let fin_flutter = ((k.swim_phase * 1.3).sin() * 0.22) as f32;
        let fx = (sp[2][0] + ox) as f32;
        let fy = (sp[2][1] + oy) as f32;

        for side in [-1.0f32, 1.0f32] {
            let rot = fin_ang + side * (1.1 + fin_flutter);
            let ca = rot.cos();
            let sa = rot.sin();
            // Ellipse center at local (0, side * 4 * sc)
            let ly = side * 4.0 * sc;
            let cx = fx - ly * sa;
            let cy = fy + ly * ca;
            Self::push_ellipse(buf, cx, cy, 7.5 * sc, 3.2 * sc, rot, col, 14);
        }
    }

    fn push_koi_dorsal(buf: &mut Vec<f32>, k: &Koi) {
        let p0 = [k.spine[2][0] as f32, k.spine[2][1] as f32];
        let p1 = [k.spine[4][0] as f32, k.spine[4][1] as f32];
        let p2 = [k.spine[6][0] as f32, k.spine[6][1] as f32];
        let half_w = 0.7 * k.scale as f32;
        let col = [1.0, 1.0, 1.0, 0.55];
        let steps = 8;
        let mut prev = p0;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let mt = 1.0 - t;
            let cur = [
                mt * mt * p0[0] + 2.0 * mt * t * p1[0] + t * t * p2[0],
                mt * mt * p0[1] + 2.0 * mt * t * p1[1] + t * t * p2[1],
            ];
            Self::push_segment(buf, prev, cur, half_w, col);
            prev = cur;
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();

        st.zen_archipelago.resize(css_w, css_h);
        st.zen_archipelago.ensure_terrain_fresh();

        // Re-upload SDF terrain texture ONLY when terrain_rev changes
        if self.uploaded_rev != st.zen_archipelago.terrain_rev
            || self.sdf_tex_w != st.zen_archipelago.sdf_w as i32
            || self.sdf_tex_h != st.zen_archipelago.sdf_h as i32
        {
            let sw = st.zen_archipelago.sdf_w as i32;
            let sh = st.zen_archipelago.sdf_h as i32;
            if sw > 0 && sh > 0 {
                (gl.BindTexture)(GL_TEXTURE_2D, self.sdf_tex);
                (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_RGBA8 as GLint,
                    sw,
                    sh,
                    0,
                    GL_RGBA,
                    GL_UNSIGNED_BYTE,
                    st.zen_archipelago.sdf_rgba.as_ptr() as *const c_void,
                );
                self.sdf_tex_w = sw;
                self.sdf_tex_h = sh;
                self.uploaded_rev = st.zen_archipelago.terrain_rev;
            }
        }

        let zen = &st.zen_archipelago;
        let pal = PALETTES[zen.params.time_of_day.min(2)];
        let time = zen.time;

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);
        (gl.Disable)(GL_SCISSOR_TEST);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(
            GL_SRC_ALPHA,
            GL_ONE_MINUS_SRC_ALPHA,
            GL_ONE,
            GL_ONE_MINUS_SRC_ALPHA,
        );

        // 1. Blit Upscaled Smooth SDF Terrain (Deep Water -> Reef -> Sand -> Mossy Island)
        self.tex_verts.clear();
        Self::push_tex_quad(&mut self.tex_verts, 0.0, 0.0, css_w, css_h, 1.0);
        self.flush_tex(gl, self.sdf_tex, css_w, css_h);

        // 1b. Static Wooden Bridges (part of terrainCache in HTML)
        self.col_verts.clear();
        let dark_plank = rgba_u8(88, 32, 32, 1.0); // #582020
        let post_col = rgba_u8(255, 207, 150, 1.0); // #ffcf96
        for br in &zen.bridges {
            let ox = br.x1 as f32;
            let oy = br.y1 as f32;
            let ang = br.ang as f32;
            let len = br.len as f32;
            let bw = 14.0f32;
            // Drop shadow on water
            Self::push_rot_rect(
                &mut self.col_verts,
                ox,
                oy,
                ang,
                2.0,
                -bw * 0.5 + 5.0,
                len,
                bw,
                pal.ambient_shadow,
            );
            // Curved planks
            let steps = ((len / 6.0).floor() as usize).max(5);
            let step_len = len / steps as f32;
            for i in 0..steps {
                let t = i as f32 / (steps - 1).max(1) as f32;
                let arch = (t * std::f32::consts::PI).sin() * 2.5;
                let col = if i % 2 == 0 { pal.bridge_wood } else { dark_plank };
                Self::push_rot_rect(
                    &mut self.col_verts,
                    ox,
                    oy,
                    ang,
                    i as f32 * step_len,
                    -bw * 0.5 - arch,
                    (step_len - 1.0).max(1.0),
                    bw,
                    col,
                );
            }
            // Side railings
            Self::push_rot_rect(&mut self.col_verts, ox, oy, ang, 0.0, -bw * 0.5 - 2.0, len, 2.5, pal.bridge_rail);
            Self::push_rot_rect(&mut self.col_verts, ox, oy, ang, 0.0, bw * 0.5 - 1.5, len, 2.5, pal.bridge_rail);
            // Posts
            let posts = ((len / 28.0).floor() as usize).max(2);
            let ca = ang.cos();
            let sa = ang.sin();
            for i in 0..=posts {
                let px = (i as f32 / posts as f32) * len;
                let y_top = -bw * 0.5 - 1.0;
                let y_bot = bw * 0.5;
                Self::push_ellipse(
                    &mut self.col_verts,
                    ox + px * ca - y_top * sa,
                    oy + px * sa + y_top * ca,
                    2.0,
                    2.0,
                    0.0,
                    post_col,
                    8,
                );
                Self::push_ellipse(
                    &mut self.col_verts,
                    ox + px * ca - y_bot * sa,
                    oy + px * sa + y_bot * ca,
                    2.0,
                    2.0,
                    0.0,
                    post_col,
                    8,
                );
            }
        }

        // 1c. Static Island Features (Rocks, Toro stone base, Sakura ground shadow)
        let rock_col = rgba_u8(75, 92, 102, 1.0); // #4b5c66
        let grass_out_col = [
            pal.grass_outer[0] as f32 / 255.0,
            pal.grass_outer[1] as f32 / 255.0,
            pal.grass_outer[2] as f32 / 255.0,
            1.0,
        ];
        let toro_base = rgba_u8(122, 134, 140, 1.0); // #7a868c
        let toro_roof = rgba_u8(154, 166, 172, 1.0); // #9aa6ac

        for f in &zen.island_features {
            let fx = f.x as f32;
            let fy = f.y as f32;
            let fs = f.size as f32;
            match f.kind {
                FeatureKind::Rock => {
                    Self::push_ellipse(&mut self.col_verts, fx + 3.0, fy + 3.0, fs, fs, 0.0, pal.ambient_shadow, 14);
                    Self::push_ellipse(&mut self.col_verts, fx, fy, fs * 1.15, fs * 0.85, f.seed as f32, rock_col, 16);
                    Self::push_ellipse(&mut self.col_verts, fx - 1.0, fy - 1.0, fs * 0.55, fs * 0.55, 0.0, grass_out_col, 12);
                }
                FeatureKind::Toro => {
                    Self::push_ellipse(&mut self.col_verts, fx + 4.0, fy + 4.0, fs * 1.1, fs * 0.8, 0.0, pal.ambient_shadow, 14);
                    Self::push_ellipse(&mut self.col_verts, fx, fy, fs, fs, 0.0, toro_base, 14);
                    let r = fs * 1.1;
                    Self::push_quad(
                        &mut self.col_verts,
                        [fx, fy - r],
                        [fx + r, fy],
                        [fx, fy + r],
                        [fx - r, fy],
                        toro_roof,
                    );
                }
                FeatureKind::Sakura => {
                    Self::push_ellipse(&mut self.col_verts, fx + 6.0, fy + 6.0, fs * 1.05, fs * 1.05, 0.0, pal.ambient_shadow, 18);
                }
            }
        }

        // 2. Animated Shoreline Bioluminescent / Foam Waves around Islands
        let wave_amp = zen.params.wave_intensity;
        if wave_amp > 0.05 {
            for ring in 0..2 {
                let phase = (time * 0.42 + (ring as f64) * 0.5).rem_euclid(1.0);
                let alpha = ((phase * PI).sin() * 0.38 * wave_amp.min(1.5)) as f32;
                let col = [pal.foam_rgb[0], pal.foam_rgb[1], pal.foam_rgb[2], alpha];
                for ic in &zen.current_icons {
                    let steps = 28;
                    let base_r = ic.w.max(ic.h) * 0.42 + zen.params.island_margin + phase * 18.0 * wave_amp;
                    let mut prev: Option<[f32; 2]> = None;
                    for s in 0..=steps {
                        let a = (s as f64 / steps as f64) * PI * 2.0;
                        let wobble = (a * 4.0 + time * 1.8 + ic.cx).sin() * 3.5;
                        let px = ic.cx + a.cos() * (base_r + wobble);
                        let py = ic.cy + a.sin() * (base_r + wobble);
                        if zen.sample_sdf(px, py) < 2.0 {
                            let cur = [px as f32, py as f32];
                            if let Some(p) = prev {
                                Self::push_segment(&mut self.col_verts, p, cur, 0.8, col);
                            }
                            prev = Some(cur);
                        } else {
                            prev = None;
                        }
                    }
                }
            }
        }

        // 3. Koi Shadows on Lagoon Floor
        for k in &zen.kois {
            Self::push_koi_fins(&mut self.col_verts, k, 6.0, 8.0, pal.ambient_shadow);
            Self::build_koi_geometry(k, 6.0, 8.0, &mut self.koi_verts);
            for tri in self.koi_verts.chunks_exact(6) {
                Self::push_tri(
                    &mut self.col_verts,
                    [tri[0], tri[1]],
                    [tri[2], tri[3]],
                    [tri[4], tri[5]],
                    pal.ambient_shadow,
                );
            }
        }
        self.flush_col(gl, css_w, css_h);

        // 4. Swimming Koi Fish (exact per-koi painter's order: fins -> clipped body+spots -> dorsal ridge)
        let fin_col = [1.0, 1.0, 1.0, 0.65];
        for k in &zen.kois {
            // 4a. Pectoral fins
            self.col_verts.clear();
            Self::push_koi_fins(&mut self.col_verts, k, 0.0, 0.0, fin_col);
            self.flush_col(gl, css_w, css_h);

            // 4b. Body silhouette with exact fragment-clipped Koi spots
            Self::build_koi_geometry(k, 0.0, 0.0, &mut self.koi_verts);
            let pat = KOI_PATTERNS[k.pattern_idx % KOI_PATTERNS.len()];
            let sc = k.scale as f32;

            (gl.UseProgram)(self.koi_prog);
            (gl.Uniform2f)(gl.uniform_loc(self.koi_prog, "uRes"), css_w, css_h);
            (gl.Uniform3f)(
                gl.uniform_loc(self.koi_prog, "uBaseCol"),
                pat.base[0],
                pat.base[1],
                pat.base[2],
            );
            (gl.Uniform3f)(
                gl.uniform_loc(self.koi_prog, "uSpotCol0"),
                pat.spot0[0],
                pat.spot0[1],
                pat.spot0[2],
            );
            (gl.Uniform3f)(
                gl.uniform_loc(self.koi_prog, "uSpotCol1"),
                pat.spot1[0],
                pat.spot1[1],
                pat.spot1[2],
            );
            (gl.Uniform3f)(
                gl.uniform_loc(self.koi_prog, "uSpot0"),
                k.spine[1][0] as f32,
                k.spine[1][1] as f32,
                if pat.head_spot { 4.5 * sc } else { -1.0 },
            );
            (gl.Uniform3f)(
                gl.uniform_loc(self.koi_prog, "uSpot1"),
                k.spine[3][0] as f32 + 1.5 * sc,
                k.spine[3][1] as f32,
                5.2 * sc,
            );
            (gl.Uniform3f)(
                gl.uniform_loc(self.koi_prog, "uSpot2"),
                k.spine[5][0] as f32 - 1.2 * sc,
                k.spine[5][1] as f32,
                4.0 * sc,
            );

            (gl.BindVertexArray)(self.koi_vao);
            (gl.BindBuffer)(GL_ARRAY_BUFFER, self.koi_vbo);
            (gl.BufferData)(
                GL_ARRAY_BUFFER,
                (self.koi_verts.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
                self.koi_verts.as_ptr() as *const c_void,
                GL_DYNAMIC_DRAW,
            );
            (gl.DrawArrays)(GL_TRIANGLES, 0, (self.koi_verts.len() / 2) as GLsizei);

            // 4c. Dorsal ridge fin
            self.col_verts.clear();
            Self::push_koi_dorsal(&mut self.col_verts, k);
            self.flush_col(gl, css_w, css_h);
        }

        // 5. Water Ripples & Fish Bait
        self.col_verts.clear();
        for rp in &zen.ripples {
            let a = rp.alpha.max(0.0) as f32;
            let col = [pal.foam_rgb[0], pal.foam_rgb[1], pal.foam_rgb[2], a];
            Self::push_ring(&mut self.col_verts, rp.x as f32, rp.y as f32, rp.r as f32, 1.4, col, 28);
        }
        let bait_col = rgba_u8(255, 210, 125, 1.0); // #ffd27d
        for b in &zen.baits {
            Self::push_ellipse(&mut self.col_verts, b.x as f32, b.y as f32, 2.8, 2.8, 0.0, bait_col, 10);
        }

        // 6. Lily Pads & Lotus Flowers in Shallow Reefs
        let pad_col = if zen.params.time_of_day == 0 {
            rgba_u8(30, 89, 78, 1.0) // #1e594e
        } else {
            rgba_u8(63, 143, 84, 1.0) // #3f8f54
        };
        let stamen_col = rgba_u8(255, 223, 107, 1.0); // #ffdf6b
        for lp in &zen.lily_pads {
            let bob = (time * 1.5 + lp.x * 0.05).sin() * 0.06;
            let lx = lp.x as f32;
            let ly = lp.y as f32;
            let lr = lp.r as f32;
            let rot = (lp.rot + bob) as f32;

            // Shadow notched disc at local (2, 3)
            Self::push_arc_pie(
                &mut self.col_verts,
                lx,
                ly,
                rot,
                2.0,
                3.0,
                lr,
                0.25,
                std::f32::consts::TAU - 0.25,
                pal.ambient_shadow,
                20,
            );
            // Lily pad notched disc at local (0, 0)
            Self::push_arc_pie(
                &mut self.col_verts,
                lx,
                ly,
                rot,
                0.0,
                0.0,
                lr,
                0.22,
                std::f32::consts::TAU - 0.22,
                pad_col,
                20,
            );
            if lp.has_flower {
                for p in 0..6 {
                    let pa = rot + (p as f32 / 6.0) * std::f32::consts::TAU;
                    let pcx = lx + pa.cos() * (lr * 0.35);
                    let pcy = ly + pa.sin() * (lr * 0.35);
                    Self::push_ellipse(&mut self.col_verts, pcx, pcy, lr * 0.38, lr * 0.18, pa, lp.flower_color, 10);
                }
                Self::push_ellipse(&mut self.col_verts, lx, ly, lr * 0.16, lr * 0.16, 0.0, stamen_col, 8);
            }
        }
        self.flush_col(gl, css_w, css_h);

        // 7. Floating Wooden Lanterns (Tōrō Nagashi) — Soft Radial Glow Sprite + Wooden Body
        self.tex_verts.clear();
        for l in &zen.lanterns {
            let glow_r = (l.size * (2.6 + l.pulse.sin() * 0.35)) as f32;
            let lx = l.x as f32;
            let ly = l.y as f32;
            Self::push_tex_quad(
                &mut self.tex_verts,
                lx - glow_r,
                ly - glow_r,
                lx + glow_r,
                ly + glow_r,
                1.0,
            );
        }
        self.flush_tex(gl, self.lantern_glow_tex, css_w, css_h);

        self.col_verts.clear();
        let lantern_wood = rgba_u8(92, 44, 29, 1.0);   // #5c2c1d
        let lantern_paper = rgba_u8(255, 239, 194, 1.0); // #ffefc2
        for l in &zen.lanterns {
            let lx = l.x as f32;
            let ly = l.y as f32;
            let ls = l.size as f32;
            let rot = l.rot as f32;
            Self::push_rot_rect(&mut self.col_verts, lx, ly, rot, -ls * 0.7, -ls * 0.7, ls * 1.4, ls * 1.4, lantern_wood);
            Self::push_rot_rect(&mut self.col_verts, lx, ly, rot, -ls * 0.48, -ls * 0.48, ls * 0.96, ls * 0.96, lantern_paper);
        }

        // 8a. Animated Sakura Canopies on Islands
        for f in &zen.island_features {
            if f.kind == FeatureKind::Sakura {
                let sway_x = (time * 1.3 + f.seed).sin() * 2.2;
                let sway_y = (time * 1.1 + f.seed).cos() * 1.8;
                let bx = (f.x + sway_x) as f32;
                let by = (f.y + sway_y) as f32;
                let fs = f.size as f32;
                for c in 0..5 {
                    let a = (c as f64 / 5.0) * PI * 2.0 + f.seed;
                    let cx = bx + (a.cos() * f.size * 0.38) as f32;
                    let cy = by + (a.sin() * f.size * 0.38) as f32;
                    let col = if c % 2 == 0 { pal.sakura_main } else { pal.sakura_light };
                    Self::push_ellipse(&mut self.col_verts, cx, cy, fs * 0.62, fs * 0.62, 0.0, col, 16);
                }
                Self::push_ellipse(&mut self.col_verts, bx - 2.0, by - 2.0, fs * 0.45, fs * 0.45, 0.0, pal.sakura_light, 14);
            }
        }
        self.flush_col(gl, css_w, css_h);

        // 8b. Stone Lantern Warm Glows (night & sunset, matching `timeOfDay !== "day"`)
        if zen.params.time_of_day != 2 {
            self.tex_verts.clear();
            for f in &zen.island_features {
                if f.kind == FeatureKind::Toro {
                    let r = (f.size * 2.8) as f32;
                    let fx = f.x as f32;
                    let fy = f.y as f32;
                    Self::push_tex_quad(&mut self.tex_verts, fx - r, fy - r, fx + r, fy + r, 1.0);
                }
            }
            self.flush_tex(gl, self.toro_glow_tex, css_w, css_h);
        }

        // 9. Drifting Sakura Petals & Bioluminescent Spores / Fireflies
        self.col_verts.clear();
        let is_night = zen.params.time_of_day == 0;
        for p in &zen.petals {
            if p.is_firefly {
                let alpha = ((p.phase.sin() * 0.5 + 0.5) * 0.85) as f32;
                let col = if is_night {
                    rgba_u8(110, 250, 255, alpha)
                } else {
                    rgba_u8(255, 240, 170, alpha)
                };
                let r = (p.size * 0.75) as f32;
                Self::push_ellipse(&mut self.col_verts, p.x as f32, p.y as f32, r, r, 0.0, col, 10);
            } else if zen.params.sakura_density > 0.0 {
                let ps = p.size as f32;
                Self::push_ellipse(
                    &mut self.col_verts,
                    p.x as f32,
                    p.y as f32,
                    ps,
                    ps * 0.55,
                    p.rot as f32,
                    pal.sakura_light,
                    10,
                );
            }
        }
        self.flush_col(gl, css_w, css_h);
        (gl.Disable)(GL_BLEND);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash2_matches_js_prototype_exactly() {
        // Measured against prototype/zen-archipelago.html:140-146
        assert!((hash2(0, 301) - 0.721177955_f64).abs() < 1e-8, "got {}", hash2(0, 301));
        assert!((hash2(1, 301) - 0.709604827_f64).abs() < 1e-8, "got {}", hash2(1, 301));
        assert!((hash2(0, 401) - 0.040740950_f64).abs() < 1e-8, "got {}", hash2(0, 401));
        assert!((hash2(5, 77) - 0.428958607_f64).abs() < 1e-8, "got {}", hash2(5, 77));
    }

    #[test]
    fn terrain_rebuilds_only_when_needed_and_tracks_icon_moves_and_palette() {
        let mut st = ZenArchipelagoState::default();
        st.resize(1280.0, 720.0);
        st.ensure_terrain_fresh();
        let rev0 = st.terrain_rev;

        // Same size must NOT dirty terrain or increment terrain_rev
        st.resize(1280.0, 720.0);
        st.step(0.033);
        assert_eq!(st.terrain_rev, rev0, "unchanged resize should not recompute SDF");

        // Adding icons updates terrain
        let icons_a = vec![
            Rect { x: 100.0, y: 100.0, w: 76.0, h: 86.0 },
            Rect { x: 260.0, y: 120.0, w: 76.0, h: 86.0 },
        ];
        st.set_icons(&icons_a, true);
        st.step(0.033);
        let rev1 = st.terrain_rev;
        assert!(rev1 > rev0, "adding icons must rebuild terrain");

        // Moving an icon (same icon count!) MUST rebuild terrain & move the island SDF
        let sdf_before = st.sample_sdf(540.0, 340.0);
        let icons_b = vec![
            Rect { x: 500.0, y: 300.0, w: 76.0, h: 86.0 },
            Rect { x: 260.0, y: 120.0, w: 76.0, h: 86.0 },
        ];
        st.set_icons(&icons_b, true);
        for _ in 0..40 {
            st.step(0.033);
        }
        assert!(st.terrain_rev > rev1, "moving icon with same count must rebuild terrain");
        let sdf_after = st.sample_sdf(540.0, 340.0);
        assert!(
            sdf_after > 0.0 && sdf_before < 0.0,
            "island SDF must move with icon (before={sdf_before}, after={sdf_after})"
        );

        // Changing time_of_day MUST rebuild terrain colors & increment terrain_rev
        let rev2 = st.terrain_rev;
        let night_px = [st.sdf_rgba[0], st.sdf_rgba[1], st.sdf_rgba[2]];
        let mut p = st.params;
        p.time_of_day = 2; // day
        st.sync_params(p);
        st.ensure_terrain_fresh();
        assert!(st.terrain_rev > rev2, "time_of_day change must invalidate terrain");
        let day_px = [st.sdf_rgba[0], st.sdf_rgba[1], st.sdf_rgba[2]];
        assert_ne!(night_px, day_px, "day and night terrain colors must differ");
    }

    #[test]
    fn glow_sprite_edges_are_fully_transparent() {
        let rgba = build_radial_glow_rgba(
            64,
            [255.0, 220.0, 140.0, 0.95],
            Some((0.35, [255.0, 150.0, 60.0, 0.42])),
            [255.0, 120.0, 30.0, 0.0],
        );
        // Center must be bright and opaque
        let center_a = rgba[(32 * 64 + 32) * 4 + 3];
        assert!(center_a > 230, "center alpha should be high, got {center_a}");
        // All border pixels must have alpha == 0 (no white squares!)
        for i in 0..64 {
            assert_eq!(rgba[i * 4 + 3], 0);
            assert_eq!(rgba[((63 * 64) + i) * 4 + 3], 0);
            assert_eq!(rgba[(i * 64) * 4 + 3], 0);
            assert_eq!(rgba[(i * 64 + 63) * 4 + 3], 0);
        }
    }
}
