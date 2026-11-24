mod atlas;
mod render;
pub mod shaders;

pub use render::RosebrookRenderer;

use crate::state::Rect;
use std::collections::HashMap;
use std::f64::consts::TAU;

// =========================================================================
// Constants — 1:1 prototype/rosebrook.html + rosebrook-blooms.js
// =========================================================================
pub const RB_STEP: f64 = 6.0;
pub const RB_MORPH_MS: f64 = 1200.0;
pub const RB_FLOWER_VARIANTS: usize = 8;
pub const RB_PETAL_VARIANTS: usize = 4;
pub const RB_ATLAS_COLS: usize = RB_FLOWER_VARIANTS + RB_PETAL_VARIANTS;
pub const RB_SPRITE_SZ: usize = 128;

// =========================================================================
// Deterministic hashes & noise (exact JS f64 / Float32Array semantics)
// =========================================================================
#[inline]
fn imul(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}

#[inline]
fn rb_hash(seed: i32, a: i32, b: i32, k: i32) -> f64 {
    let h = imul(seed.wrapping_add(imul(a, 374_761_393)), 668_265_263)
        ^ imul(b.wrapping_add(imul(k, 97)), 2_246_822_519u32 as i32);
    let h = imul(h ^ ((h as u32) >> 13) as i32, 1_274_126_177);
    let u = (h as u32) ^ ((h as u32) >> 16);
    u as f64 / 4_294_967_296.0
}

#[inline]
fn js_hash2(x: i32, y: i32, seed: i32, k: i32) -> f64 {
    let h = imul(x, 374_761_393) ^ imul(y, 668_265_263) ^ imul(seed.wrapping_add(k), 2_246_822_519u32 as i32);
    let h = imul(h ^ ((h as u32) >> 13) as i32, 1_274_126_177);
    let u = (h as u32) ^ ((h as u32) >> 16);
    u as f64 / 4_294_967_296.0
}

#[inline]
fn vnoise(x: f64, y: f64, k: i32, seed: i32) -> f64 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = js_hash2(ix as i32, iy as i32, seed, k);
    let b = js_hash2(ix as i32 + 1, iy as i32, seed, k);
    let c = js_hash2(ix as i32, iy as i32 + 1, seed, k);
    let d = js_hash2(ix as i32 + 1, iy as i32 + 1, seed, k);
    a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy
}

fn fbm(x0: f64, y0: f64, k: i32, seed: i32) -> f64 {
    let mut a = 0.5;
    let mut s = 0.0;
    let (mut x, mut y) = (x0, y0);
    for i in 0..4 {
        s += a * vnoise(x, y, k + i * 7, seed);
        x = x * 2.03 + 17.1;
        y = y * 2.03 + 17.1;
        a *= 0.5;
    }
    s
}

#[inline]
fn pmod(v: i32, p: i32) -> i32 {
    ((v % p) + p) % p
}

fn tile_noise(x: f64, y: f64, px: i32, py: i32, k: i32, seed: i32) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f64;
    let fy = y - iy as f64;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let x0 = pmod(ix, px);
    let x1 = pmod(ix + 1, px);
    let y0 = pmod(iy, py);
    let y1 = pmod(iy + 1, py);
    let a = js_hash2(x0, y0, seed, k);
    let b = js_hash2(x1, y0, seed, k);
    let c = js_hash2(x0, y1, seed, k);
    let d = js_hash2(x1, y1, seed, k);
    a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy
}

fn tile_fbm(u: f64, v: f64, periods: &[i32], k: i32, seed: i32) -> f64 {
    let mut s = 0.0;
    let mut w = 0.5;
    let mut tw = 0.0;
    for (i, &p) in periods.iter().enumerate() {
        s += w * tile_noise(u * p as f64, v * p as f64, p, p, k + (i as i32) * 13, seed);
        tw += w;
        w *= 0.5;
    }
    s / tw
}

fn make_noise_image(seed: i32) -> (usize, Vec<u8>) {
    const S: usize = 256;
    let chans: [&[i32]; 4] = [
        &[8, 16, 32, 64],
        &[8, 16, 32, 64],
        &[8, 16, 32],
        &[16, 32, 64, 128],
    ];
    let mut d = vec![0u8; S * S * 4];
    for y in 0..S {
        for x in 0..S {
            let u = x as f64 / S as f64;
            let v = y as f64 / S as f64;
            let i = (y * S + x) * 4;
            for c in 0..4 {
                let n = tile_fbm(u, v, chans[c], 100 + (c as i32) * 50, seed);
                let vv = (((n - 0.5) * 1.9 + 0.5).clamp(0.0, 1.0)) * 255.0;
                d[i + c] = vv as u8;
            }
        }
    }
    (S, d)
}

// =========================================================================
// Parameters & Entities
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct RosebrookParams {
    pub sigma: f64,
    pub stream: f64,
    pub flow: f64,
    pub sunlight: f64,
    pub roses: f64,
    pub petals: usize,
    pub breeze: f64,
    pub adaptive: bool,
    pub seed: i32,
    pub debug: bool,
}

impl Default for RosebrookParams {
    fn default() -> Self {
        Self {
            sigma: 72.0,
            stream: 1.0,
            flow: 1.0,
            sunlight: 0.78,
            roses: 0.85,
            petals: 12,
            breeze: 1.0,
            adaptive: true,
            seed: 42,
            debug: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Bloom {
    pub x: f64,
    pub y: f64,
    pub ix: f64,
    pub iy: f64,
    pub variant: usize,
    pub rot: f64,
    pub phase: f64,
    pub scale: f64,
    pub sway: f64,
    pub cur: f64,
    pub want: f64,
    pub born: f64,
}

#[derive(Clone, Copy)]
pub struct Petal {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub rot: f64,
    pub vrot: f64,
    pub scale: f64,
    pub variant: usize,
    pub phase: f64,
    pub life: f64,
    pub max_life: f64,
    pub burst: bool,
}

#[derive(Clone, Copy)]
struct Lcg {
    state: u32,
}

impl Lcg {
    fn new() -> Self {
        Self { state: 0x1357_9bdf }
    }
    fn next_f64(&mut self) -> f64 {
        let v = (self.state as i64).wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.state = v as u32;
        self.state as f64 / 4_294_967_296.0
    }
}

// =========================================================================
// RosebrookState — SDF field, brook, garden, petals (CPU ground truth)
// =========================================================================
pub struct RosebrookState {
    pub params: RosebrookParams,
    pub w: f64,
    pub h: f64,
    pub gw: usize,
    pub gh: usize,
    pub nz_wx: Vec<f32>,
    pub nz_wy: Vec<f32>,
    pub field_a: Vec<f32>,
    pub field_b: Vec<f32>,
    pub field_version: u64,
    pub noise_version: u64,
    pub morph_t0: f64,
    pub morph_dur: f64,
    pub cur_mix: f64,
    pub time_sec: f64,
    pub icons: Vec<Rect>,
    pub ambient_blooms: Vec<Bloom>,
    pub icon_blooms: Vec<Bloom>,
    pub drifting_petals: Vec<Petal>,
    prev_icon_list: Vec<(f64, f64)>,
    rng: Lcg,
}

impl Default for RosebrookState {
    fn default() -> Self {
        Self::new(1280.0, 720.0)
    }
}

impl RosebrookState {
    pub fn new(w: f64, h: f64) -> Self {
        let mut s = Self {
            params: RosebrookParams::default(),
            w,
            h,
            gw: (w / RB_STEP).ceil() as usize + 2,
            gh: (h / RB_STEP).ceil() as usize + 2,
            nz_wx: Vec::new(),
            nz_wy: Vec::new(),
            field_a: Vec::new(),
            field_b: Vec::new(),
            field_version: 0,
            noise_version: 0,
            morph_t0: -1e9,
            morph_dur: RB_MORPH_MS,
            cur_mix: 1.0,
            time_sec: 0.0,
            icons: Vec::new(),
            ambient_blooms: Vec::new(),
            icon_blooms: Vec::new(),
            drifting_petals: Vec::new(),
            prev_icon_list: Vec::new(),
            rng: Lcg::new(),
        };
        s.build_noise();
        s
    }

    pub fn resize(&mut self, w: f64, h: f64) {
        if (w - self.w).abs() < 0.5 && (h - self.h).abs() < 0.5 {
            return;
        }
        self.w = w;
        self.h = h;
        self.gw = (w / RB_STEP).ceil() as usize + 2;
        self.gh = (h / RB_STEP).ceil() as usize + 2;
        self.build_noise();
        self.field_a.clear();
        self.field_b.clear();
        self.field_version = self.field_version.wrapping_add(1);
    }

    pub fn sync_params(&mut self, next: RosebrookParams) {
        let old = self.params;
        self.params = next;
        if next.seed != old.seed {
            self.build_noise();
        }
        if next.sigma != old.sigma || next.adaptive != old.adaptive || next.seed != old.seed {
            self.refresh();
        }
        if next.roses != old.roses || next.stream != old.stream || next.seed != old.seed {
            self.rebuild_garden();
        }
        if next.petals != old.petals {
            self.sync_ambient_petals();
        }
    }

    fn build_noise(&mut self) {
        let n = self.gw * self.gh;
        let mut wx = vec![0f32; n];
        let mut wy = vec![0f32; n];
        let seed = self.params.seed;
        for gy in 0..self.gh {
            for gx in 0..self.gw {
                let i = gy * self.gw + gx;
                let x = gx as f64 * RB_STEP;
                let y = gy as f64 * RB_STEP;
                wx[i] = (fbm(x * 0.0032, y * 0.0032, 1, seed) - 0.47) as f32;
                wy[i] = (fbm(x * 0.0032, y * 0.0032, 2, seed) - 0.47) as f32;
            }
        }
        self.nz_wx = wx;
        self.nz_wy = wy;
        self.noise_version = self.noise_version.wrapping_add(1);
    }

    pub fn brook_params(&self) -> (f64, f64, f64) {
        let seed = self.params.seed;
        let s = (seed % 997) as f64 * 0.37;
        let slope = if seed % 2 == 0 { -0.22 } else { 0.22 };
        let cy = 0.54 + 0.08 * (s * 0.7).sin();
        (s, slope, cy)
    }

    pub fn brook_center_y(&self, x: f64) -> f64 {
        let (s, slope, cy) = self.brook_params();
        let nx = x / self.w.max(1.0);
        let wave = 0.115 * (nx * 5.2 + s * 1.1).sin() + 0.042 * (nx * 11.4 + s * 2.3).sin();
        self.h * (cy + slope * (nx - 0.5) + wave)
    }

    pub fn brook_half_width(&self, x: f64) -> f64 {
        let (s, _, _) = self.brook_params();
        let nx = x / self.w.max(1.0);
        (34.0 + 9.0 * (nx * 8.0 + s).sin()) * self.params.stream
    }

    fn icon_boxes(&self) -> Vec<(f64, f64, f64, f64)> {
        self.icons
            .iter()
            .map(|r| {
                (
                    r.x as f64 + r.w as f64 / 2.0,
                    r.y as f64 + r.h as f64 / 2.0,
                    r.w as f64 / 2.0 - 4.0,
                    r.h as f64 / 2.0 - 2.0,
                )
            })
            .collect()
    }

    pub fn compute_field_raw(&self, rects: &[(f64, f64, f64, f64)]) -> Vec<f32> {
        let n = self.gw * self.gh;
        let mut f1 = vec![0f32; n];
        let sig = self.params.sigma;
        let is1 = 1.0 / (sig * sig);
        let wa = 44.0;
        let wmax = wa * 0.5;
        for &(cx, cy, hw, hh) in rects {
            let r = 5.2 * sig + hw.max(hh) * 0.5 + wmax;
            let gx0 = (((cx - r) / RB_STEP).floor() as isize).max(0) as usize;
            let gx1 = (((cx + r) / RB_STEP).ceil() as isize).min(self.gw as isize - 1) as usize;
            let gy0 = (((cy - r) / RB_STEP).floor() as isize).max(0) as usize;
            let gy1 = (((cy + r) / RB_STEP).ceil() as isize).min(self.gh as isize - 1) as usize;
            let chw = hw * 0.5;
            let chh = hh * 0.5;
            for gy in gy0..=gy1 {
                for gx in gx0..=gx1 {
                    let i = gy * self.gw + gx;
                    let qx = gx as f64 * RB_STEP + self.nz_wx[i] as f64 * wa;
                    let qy = gy as f64 * RB_STEP + self.nz_wy[i] as f64 * wa;
                    let mut dx = (qx - cx).abs() - chw;
                    let mut dy = (qy - cy).abs() - chh;
                    if dx < 0.0 {
                        dx = 0.0;
                    }
                    if dy < 0.0 {
                        dy = 0.0;
                    }
                    let e = (-(dx * dx + dy * dy) * is1).exp();
                    f1[i] = (f1[i] as f64 + e) as f32;
                }
            }
        }
        let c0 = (-0.35f64.ln()).sqrt();
        let mut s = vec![0f32; n];
        let f_min = (-30.0f64).exp();
        for i in 0..n {
            let g = (f1[i] as f64).max(f_min).ln();
            let v = sig * ((-g).max(0.0).sqrt() - c0) - sig * 0.6 * g.max(0.0);
            s[i] = v as f32;
        }
        s
    }

    pub fn sample_field(&self, x: f64, y: f64) -> f64 {
        let fx = (x / RB_STEP).clamp(0.0, self.gw as f64 - 1.001);
        let fy = (y / RB_STEP).clamp(0.0, self.gh as f64 - 1.001);
        let ix = fx.floor() as usize;
        let iy = fy.floor() as usize;
        let tx = fx - ix as f64;
        let ty = fy - iy as f64;
        let i = iy * self.gw + ix;
        let a = self.field_b[i] as f64 * (1.0 - tx) + self.field_b[i + 1] as f64 * tx;
        let b = self.field_b[i + self.gw] as f64 * (1.0 - tx) + self.field_b[i + self.gw + 1] as f64 * tx;
        a * (1.0 - ty) + b * ty
    }

    fn commit_from(&mut self, from_empty: bool) {
        let rects = if self.params.adaptive {
            self.icon_boxes()
        } else {
            Vec::new()
        };
        if from_empty {
            self.field_a = self.compute_field_raw(&[]);
            self.field_b = self.compute_field_raw(&rects);
            self.field_a.clone_from(&self.field_b);
        } else {
            self.field_a.clone_from(&self.field_b);
            self.field_b = self.compute_field_raw(&rects);
        }
        self.field_version = self.field_version.wrapping_add(1);
        self.morph_t0 = 0.0;
        self.morph_dur = if from_empty { 1600.0 } else { RB_MORPH_MS };
        self.cur_mix = 1.0;
        if self.ambient_blooms.is_empty() {
            self.rebuild_garden();
        }
        // JS commit() always calls syncRoseBrookBlooms() after updating the field:
        // rebuild icon wreaths at the new positions and retire the old ones.
        self.sync_blooms(from_empty, None);
    }

    pub fn refresh(&mut self) {
        let rects = if self.params.adaptive {
            self.icon_boxes()
        } else {
            Vec::new()
        };
        let f = self.compute_field_raw(&rects);
        self.field_a = f.clone();
        self.field_b = f;
        self.field_version = self.field_version.wrapping_add(1);
        self.cur_mix = 1.0;
        self.morph_t0 = -1e9;
        self.rebuild_garden();
    }

    pub fn set_icons(&mut self, icons: &[Rect], vis: bool) {
        let from_empty = self.icons.is_empty();
        self.icons = if vis { icons.to_vec() } else { Vec::new() };
        self.commit_from(from_empty);
    }

    #[cfg(test)]
    pub fn set_icons_raw(&mut self, icons: &[Rect]) {
        self.icons = icons.to_vec();
        let rects = self.icon_boxes();
        self.field_a = self.compute_field_raw(&rects);
        self.field_b = self.field_a.clone();
        self.field_version = self.field_version.wrapping_add(1);
        self.cur_mix = 1.0;
    }

    // ---------- Petals / RNG ----------
    #[cfg(test)]
    pub fn reset_rng(&mut self) {
        self.rng = Lcg::new();
    }

    pub fn spawn_petal_burst(&mut self, cx: f64, cy: f64, count: usize) {
        for i in 0..count {
            let ang = (i as f64 / count.max(1) as f64) * TAU + self.rng.next_f64() * 0.7;
            let dist = 16.0 + self.rng.next_f64() * 28.0;
            let spd = 22.0 + self.rng.next_f64() * 34.0;
            let rot = self.rng.next_f64() * TAU;
            let vrot = (self.rng.next_f64() - 0.5) * 4.2;
            let scale = 0.55 + self.rng.next_f64() * 0.45;
            let variant = if self.rng.next_f64() < 0.55 { 8 } else { 9 };
            let phase = self.rng.next_f64() * 6.28;
            let max_life = 2.2 + self.rng.next_f64() * 1.4;
            self.drifting_petals.push(Petal {
                x: cx + ang.cos() * dist,
                y: cy + ang.sin() * dist,
                vx: ang.cos() * spd + 14.0,
                vy: ang.sin() * spd - 6.0,
                rot,
                vrot,
                scale,
                variant,
                phase,
                life: 0.0,
                max_life,
                burst: true,
            });
        }
        while self.drifting_petals.len() > 90 {
            self.drifting_petals.remove(0);
        }
    }

    pub fn sync_ambient_petals(&mut self) {
        let want = self.params.petals as i64;
        let cur = self.drifting_petals.iter().filter(|p| !p.burst).count() as i64;
        if cur < want {
            for i in cur..want {
                let is_butterfly = i % 5 == 0;
                let x = self.rng.next_f64() * self.w;
                let y = self.rng.next_f64() * self.h;
                let vx = (if is_butterfly { 22.0 } else { 16.0 }) + self.rng.next_f64() * 18.0;
                let vy = (self.rng.next_f64() - 0.35) * 14.0;
                let rot = self.rng.next_f64() * 6.28;
                let vrot = if is_butterfly {
                    0.0
                } else {
                    (self.rng.next_f64() - 0.5) * 1.8
                };
                let scale = if is_butterfly {
                    0.48 + self.rng.next_f64() * 0.22
                } else {
                    0.42 + self.rng.next_f64() * 0.34
                };
                let phase = self.rng.next_f64() * 6.28;
                self.drifting_petals.push(Petal {
                    x,
                    y,
                    vx,
                    vy,
                    rot,
                    vrot,
                    scale,
                    variant: if is_butterfly { 10 + (i as usize % 2) } else { 8 + (i as usize % 2) },
                    phase,
                    life: 0.0,
                    max_life: 1e9,
                    burst: false,
                });
            }
        } else if cur > want {
            let mut remove = cur - want;
            let mut i = self.drifting_petals.len();
            while i > 0 && remove > 0 {
                i -= 1;
                if !self.drifting_petals[i].burst {
                    self.drifting_petals.remove(i);
                    remove -= 1;
                }
            }
        }
    }

    // ---------- Garden / Wreaths ----------
    pub fn rebuild_garden(&mut self) {
        self.ambient_blooms.clear();
        let dens = self.params.roses.clamp(0.05, 1.5);
        let step_x = (68.0 / dens.sqrt()).round().max(1.0) as usize;
        let step_y = (62.0 / dens.sqrt()).round().max(1.0) as usize;
        let cols = ((self.w + step_x as f64) / step_x as f64).ceil() as usize + 1;
        let rows = ((self.h + step_y as f64) / step_y as f64).ceil() as usize + 1;
        let seed = self.params.seed;
        for gy in 0..rows {
            for gx in 0..cols {
                let r0 = rb_hash(seed, gx as i32, gy as i32, 1);
                let r1 = rb_hash(seed, gx as i32, gy as i32, 2);
                let r2 = rb_hash(seed, gx as i32, gy as i32, 3);
                let r3 = rb_hash(seed, gx as i32, gy as i32, 4);
                let x = (gx as f64 - 0.5) * step_x as f64 + (r0 - 0.5) * step_x as f64 * 0.85;
                let y = (gy as f64 - 0.5) * step_y as f64 + (r1 - 0.5) * step_y as f64 * 0.85;
                let rd = (y - self.brook_center_y(x)).abs() - self.brook_half_width(x);
                if rd < 10.0 {
                    continue;
                }
                let bank_boost = if rd < 74.0 { 0.92 } else { 0.45 };
                if r2 > dens * bank_boost {
                    continue;
                }
                let variant = if rd < 62.0 {
                    if r3 < 0.42 {
                        ((r0 * 4.0).floor() as usize) % 4
                    } else if r3 < 0.74 {
                        7
                    } else {
                        4 + (((r1 * 3.0).floor() as usize) % 3)
                    }
                } else if r3 < 0.28 {
                    ((r0 * 4.0).floor() as usize) % 4
                } else {
                    4 + (((r2 * 4.0).floor() as usize) % 4)
                };
                self.ambient_blooms.push(Bloom {
                    x,
                    y,
                    ix: x,
                    iy: y,
                    variant,
                    rot: (r1 - 0.5) * 1.6,
                    phase: r0 * TAU,
                    scale: (if variant < 4 { 0.48 } else { 0.42 }) + r2 * 0.34,
                    sway: 0.6 + r3 * 0.6,
                    cur: 0.0,
                    want: 0.0,
                    born: 0.0,
                });
            }
        }
        self.sync_blooms(true, None);
    }

    fn key_of(x: f64, y: f64) -> (i64, i64) {
        ((x / 6.0).round() as i64, (y / 6.0).round() as i64)
    }

    pub fn sync_blooms(&mut self, from_empty: bool, forced_drops: Option<&[(f64, f64)]>) {
        let roses = self.params.roses;
        let stream = self.params.stream;
        let field_ready = !self.field_b.is_empty();
        let (bs, slope, bcy) = self.brook_params();
        let w = self.w;
        let h = self.h;
        for i in 0..self.ambient_blooms.len() {
            let (bx, by) = (self.ambient_blooms[i].x, self.ambient_blooms[i].y);
            let f = if field_ready { self.sample_field(bx, by) } else { 40.0 };
            let nx = bx / w.max(1.0);
            let cy = h * (bcy + slope * (nx - 0.5)
                + 0.115 * (nx * 5.2 + bs * 1.1).sin()
                + 0.042 * (nx * 11.4 + bs * 2.3).sin());
            let hw = (34.0 + 9.0 * (nx * 8.0 + bs).sin()) * stream;
            let rd = (by - cy).abs() - hw;
            let on_bank = rd > 12.0;
            let clear = if f > -10.0 { 1.0 } else { 0.0 };
            let want = (if on_bank { clear } else { 0.0 }) * (if roses > 0.02 { 1.0 } else { 0.0 });
            self.ambient_blooms[i].want = want;
            if from_empty {
                self.ambient_blooms[i].cur = want;
            }
        }

        let mut old_by_key: HashMap<(i64, i64), f64> = HashMap::new();
        for ib in &self.icon_blooms {
            old_by_key.entry(Self::key_of(ib.ix, ib.iy)).or_insert(ib.born);
        }

        let drop_list: Vec<(f64, f64, bool)> = if let Some(fd) = forced_drops {
            if !fd.is_empty() {
                self.icons
                    .iter()
                    .map(|ic| {
                        let cx = ic.x as f64 + ic.w as f64 * 0.5;
                        let cy = ic.y as f64 + ic.h as f64 * 0.5;
                        let is_new = fd
                            .iter()
                            .any(|d| ((d.0 - cx).powi(2) + (d.1 - cy).powi(2)).sqrt() < 24.0);
                        (cx, cy, is_new)
                    })
                    .collect()
            } else {
                self.detect_dropped_icons(from_empty)
            }
        } else {
            self.detect_dropped_icons(from_empty)
        };

        self.icon_blooms.clear();
        let rose_scale = 0.35 + roses * 0.75;
        let per_icon = (6.0 + roses * 7.0).round().max(4.0) as usize;
        let seed = self.params.seed;
        for (cx, cy, is_new) in drop_list {
            let rd_center = (cy - self.brook_center_y(cx)).abs() - self.brook_half_width(cx);
            if rd_center < -8.0 {
                continue;
            }
            let key = Self::key_of(cx, cy);
            let now_ms = self.time_sec * 1000.0;
            let born_base = if is_new {
                now_ms
            } else {
                *old_by_key.get(&key).unwrap_or(&(now_ms - 1800.0))
            };
            if is_new && !from_empty {
                let n = (5.0 + roses * 7.0).round() as usize;
                self.spawn_petal_burst(cx, cy, n);
            }
            let base_rad = self
                .icons
                .iter()
                .find(|ic| {
                    (ic.x as f64 + ic.w as f64 * 0.5 - cx).abs() < 0.001
                        && (ic.y as f64 + ic.h as f64 * 0.5 - cy).abs() < 0.001
                })
                .map(|ic| ic.w.max(ic.h) as f64 * 0.56 + 10.0)
                .unwrap_or(52.0);
            let ci = cx.round() as i32;
            let cj = cy.round() as i32;
            for k in 0..per_icon {
                let h0 = rb_hash(seed, ci, cj, (k as i32) * 7 + 11);
                let h1 = rb_hash(seed, ci, cj, (k as i32) * 13 + 29);
                let ang = (k as f64 / per_icon as f64) * TAU + (h0 - 0.5) * 0.42;
                let rad = base_rad + h1 * 18.0;
                let bx = cx + ang.cos() * rad;
                let by = cy + ang.sin() * rad;
                if ((by - self.brook_center_y(bx)).abs() - self.brook_half_width(bx)) < 6.0 {
                    continue;
                }
                let variant = if k % 2 == 0 || h0 < 0.58 {
                    ((h1 * 4.0).floor() as usize) % 4
                } else {
                    4 + (((h0 * 4.0).floor() as usize) % 4)
                };
                self.icon_blooms.push(Bloom {
                    x: bx,
                    y: by,
                    ix: cx,
                    iy: cy,
                    variant,
                    rot: (h1 - 0.5) * 1.4,
                    phase: h0 * 6.28,
                    scale: ((if variant < 4 { 0.52 } else { 0.44 }) + h0 * 0.26) * rose_scale.min(1.15),
                    sway: 0.7 + h1 * 0.5,
                    cur: 0.0,
                    want: 0.0,
                    born: born_base + k as f64 * 42.0,
                });
            }
        }
        self.sync_ambient_petals();
    }

    fn detect_dropped_icons(&mut self, from_empty: bool) -> Vec<(f64, f64, bool)> {
        let mut dropped = Vec::new();
        if from_empty {
            for ic in &self.icons {
                dropped.push((
                    ic.x as f64 + ic.w as f64 * 0.5,
                    ic.y as f64 + ic.h as f64 * 0.5,
                    true,
                ));
            }
            self.prev_icon_list = dropped.iter().map(|&(x, y, _)| (x, y)).collect();
            return dropped;
        }
        let mut used = vec![false; self.prev_icon_list.len()];
        for ic in &self.icons {
            let cx = ic.x as f64 + ic.w as f64 * 0.5;
            let cy = ic.y as f64 + ic.h as f64 * 0.5;
            let mut best_idx: isize = -1;
            let mut best_d = f64::INFINITY;
            for (i, &(px, py)) in self.prev_icon_list.iter().enumerate() {
                if used[i] {
                    continue;
                }
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                if d < best_d {
                    best_d = d;
                    best_idx = i as isize;
                }
            }
            if best_idx >= 0 && best_d < 8.0 {
                used[best_idx as usize] = true;
                dropped.push((cx, cy, false));
            } else {
                if best_idx >= 0 {
                    used[best_idx as usize] = true;
                }
                dropped.push((cx, cy, true));
            }
        }
        self.prev_icon_list = dropped.iter().map(|&(x, y, _)| (x, y)).collect();
        dropped
    }

    pub fn step(&mut self, dt: f64) {
        self.time_sec += dt;
        let morph_rate = (dt * 3.8).min(1.0);
        for b in self.ambient_blooms.iter_mut() {
            b.cur += (b.want - b.cur) * morph_rate;
        }
        let breeze = self.params.breeze;
        let mut i = self.drifting_petals.len();
        while i > 0 {
            i -= 1;
            let p = &mut self.drifting_petals[i];
            p.life += dt;
            if p.burst && p.life >= p.max_life {
                self.drifting_petals.remove(i);
                continue;
            }
            let flutter = (p.life * 2.6 + p.phase).sin();
            p.x += (p.vx * (0.55 + 0.45 * breeze) + flutter * 10.0) * dt;
            p.y += (p.vy + (p.life * 2.1 + p.phase).cos() * 12.0) * dt;
            if p.burst {
                p.vx *= 0.45f64.powf(dt);
                p.vy += 8.0 * dt;
            }
            p.rot += p.vrot * dt;
            if !p.burst {
                if p.x > self.w + 40.0 {
                    p.x = -36.0;
                }
                if p.x < -40.0 {
                    p.x = self.w + 36.0;
                }
                if p.y > self.h + 40.0 {
                    p.y = -36.0;
                }
                if p.y < -40.0 {
                    p.y = self.h + 36.0;
                }
            }
        }
        let e = ((0.0 - self.morph_t0) / self.morph_dur).min(1.0).max(0.0);
        self.cur_mix = if e < 0.5 { 2.0 * e * e } else { 1.0 - (-2.0 * e + 2.0).powi(2) / 2.0 };
    }

    pub fn back_out_ease(t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        let c1 = 1.70158;
        let c3 = c1 + 1.0;
        let x = t - 1.0;
        1.0 + c3 * x * x * x + c1 * x * x
    }

    pub fn on_pointer_down(&mut self, x: f64, y: f64) {
        self.spawn_petal_burst(x, y, 8);
    }
}

// =========================================================================
// rust-anything Ground-Truth Verification Tests
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fnv1a_fingerprint(rel_path: &str, bytes: &[u8]) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        let eat = |h: &mut u64, data: &[u8]| {
            for &b in data {
                *h ^= b as u64;
                *h = h.wrapping_mul(0x100000001b3);
            }
        };
        eat(&mut h, rel_path.replace('\\', "/").as_bytes());
        eat(&mut h, &[0]);
        eat(&mut h, bytes);
        eat(&mut h, &[0]);
        format!("{h:016x}")
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn assert_close(a: f64, b: f64, tol: f64, label: &str) {
        assert!(
            (a - b).abs() <= tol,
            "{label}: got {a}, expected {b} (diff {}, tol {tol})",
            (a - b).abs()
        );
    }

    fn sample_icons() -> Vec<Rect> {
        vec![
            Rect { x: 40.0, y: 40.0, w: 76.0, h: 82.0 },
            Rect { x: 40.0, y: 138.0, w: 76.0, h: 82.0 },
            Rect { x: 40.0, y: 236.0, w: 76.0, h: 82.0 },
            Rect { x: 126.0, y: 40.0, w: 76.0, h: 82.0 },
            Rect { x: 640.0, y: 220.0, w: 76.0, h: 82.0 },
            Rect { x: 726.0, y: 220.0, w: 76.0, h: 82.0 },
        ]
    }

    fn fresh_state(seed: i32, sigma: f64, stream: f64, roses: f64, petals: usize, breeze: f64) -> RosebrookState {
        let mut s = RosebrookState::new(1280.0, 720.0);
        s.reset_rng();
        let mut p = s.params;
        p.seed = seed;
        p.sigma = sigma;
        p.stream = stream;
        p.roses = roses;
        p.petals = petals;
        p.breeze = breeze;
        p.adaptive = true;
        s.sync_params(p);
        s
    }

    #[test]
    fn verify_rust_anything_recorded_rosebrook_cases() {
        let root = repo_root();
        let rec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/rosebrook/recorded-from.json");
        let rec: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&rec_path).unwrap()).unwrap();
        for src in rec["sources"].as_array().unwrap() {
            let rel = src["path"].as_str().unwrap();
            let expected = src["fnv1a"].as_str().unwrap();
            let actual = fnv1a_fingerprint(rel, &std::fs::read(root.join(rel)).unwrap());
            assert_eq!(actual, expected, "Source fingerprint mismatch for {rel}");
        }

        let cases_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/rosebrook/cases.jsonl");
        let lines: Vec<serde_json::Value> = std::fs::read_to_string(&cases_path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 7);

        // Case 1: computeFieldRosebrook
        {
            let c = &lines[0];
            assert_eq!(c["fn"], "computeFieldRosebrook");
            let mut s = fresh_state(42, 72.0, 1.0, 0.85, 12, 1.0);
            s.set_icons_raw(&sample_icons());
            let probes = c["input"]["probes"].as_array().unwrap();
            let expected = c["output"]["sampled"].as_array().unwrap();
            for (k, (p, e)) in probes.iter().zip(expected.iter()).enumerate() {
                let got = s.sample_field(p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
                assert_close(got, e.as_f64().unwrap(), 1e-4, &format!("field probe {k}"));
            }
        }

        // Case 2: brookCurveRosebrook
        {
            let c = &lines[1];
            assert_eq!(c["fn"], "brookCurveRosebrook");
            for key in ["s42", "s1337", "s777777"] {
                let seed = match key { "s42" => 42, "s1337" => 1337, _ => 777777 };
                let s = fresh_state(seed, 72.0, 1.0, 0.85, 12, 1.0);
                let exp = &c["output"][key];
                let params = s.brook_params();
                assert_close(params.0, exp["params"]["s"].as_f64().unwrap(), 1e-12, "brook s");
                assert_close(params.1, exp["params"]["slope"].as_f64().unwrap(), 1e-12, "brook slope");
                assert_close(params.2, exp["params"]["cy"].as_f64().unwrap(), 1e-12, "brook cy");
                for ((_k, got), e) in exp["centers"].as_array().unwrap().iter().enumerate().zip(
                    [0.0, 160.0, 320.0, 640.0, 960.0, 1120.0, 1280.0].iter(),
                ) {
                    assert_close(s.brook_center_y(*e), got.as_f64().unwrap(), 1e-9, "brook center");
                }
                for (got, e) in exp["widths"].as_array().unwrap().iter().zip(
                    [0.0, 160.0, 320.0, 640.0, 960.0, 1120.0, 1280.0].iter(),
                ) {
                    assert_close(s.brook_half_width(*e), got.as_f64().unwrap(), 1e-9, "brook width");
                }
            }
        }

        // Case 3: rosebrookNoiseAtlas
        {
            let c = &lines[2];
            assert_eq!(c["fn"], "rosebrookNoiseAtlas");
            let (sz, d) = make_noise_image(42);
            assert_eq!(sz, c["input"]["S"].as_u64().unwrap() as usize);
            let mut sum: u32 = 0;
            for &b in &d {
                sum = sum.wrapping_add(b as u32);
            }
            assert_eq!(sum as u64, c["output"]["byteSum"].as_u64().unwrap());
            for (k, s) in c["output"]["samples"].as_array().unwrap().iter().enumerate() {
                let x = [0usize, 64, 128, 255][k];
                let y = [0usize, 128, 64, 255][k];
                let i = (y * sz + x) * 4;
                for ch in 0..4 {
                    assert_eq!(d[i + ch], s[ch].as_u64().unwrap() as u8, "noise sample {k}.{ch}");
                }
            }
        }

        // Case 4: rosebrookGardenBuild
        {
            let c = &lines[3];
            assert_eq!(c["fn"], "rosebrookGardenBuild");
            let configs = [("s42_r085", 42, 0.85), ("s1337_r050", 1337, 0.5)];
            for (key, seed, roses) in configs {
                let exp = &c["output"][key];
                let mut s = fresh_state(seed, 72.0, 1.0, roses, 12, 1.0);
                s.set_icons_raw(&sample_icons());
                s.rebuild_garden();
                let rands = exp["rands"].as_array().unwrap();
                assert_close(rb_hash(seed, 0, 0, 1), rands[0].as_f64().unwrap(), 1e-12, "rand0");
                assert_close(rb_hash(seed, 7, 11, 2), rands[1].as_f64().unwrap(), 1e-12, "rand1");
                assert_close(rb_hash(seed, 23, 5, 4), rands[2].as_f64().unwrap(), 1e-12, "rand2");
                assert_eq!(s.ambient_blooms.len(), exp["total"].as_u64().unwrap() as usize, "{key} total");
                for x in exp["samples"].as_array().unwrap() {
                    let idx = x["i"].as_u64().unwrap() as usize;
                    let b = &s.ambient_blooms[idx];
                    assert_close(b.x, x["x"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].x"));
                    assert_close(b.y, x["y"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].y"));
                    assert_eq!(b.variant, x["variant"].as_u64().unwrap() as usize, "{key} variant");
                    assert_close(b.phase, x["phase"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].phase"));
                    assert_close(b.scale, x["scale"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].scale"));
                    assert_close(b.sway, x["sway"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].sway"));
                    assert_close(b.want, x["want"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].want"));
                }
            }
        }

        // Case 5: rosebrookIconWreaths
        {
            let c = &lines[4];
            assert_eq!(c["fn"], "rosebrookIconWreaths");
            let exp = &c["output"];
            let mut s = fresh_state(42, 72.0, 1.0, 0.85, 12, 1.0);
            s.set_icons_raw(&sample_icons());
            s.rebuild_garden();
            s.sync_blooms(true, None);
            assert_eq!(s.ambient_blooms.len(), exp["ambientTotal"].as_u64().unwrap() as usize);
            assert_eq!(
                s.ambient_blooms.iter().filter(|b| b.want > 0.5).count(),
                exp["wantGrown"].as_u64().unwrap() as usize
            );
            assert_eq!(s.icon_blooms.len(), exp["iconTotal"].as_u64().unwrap() as usize);
            for x in exp["iconSamples"].as_array().unwrap() {
                let idx = x["i"].as_u64().unwrap() as usize;
                let b = &s.icon_blooms[idx];
                assert_close(b.ix, x["ix"].as_f64().unwrap(), 1e-9, "ix");
                assert_close(b.iy, x["iy"].as_f64().unwrap(), 1e-9, "iy");
                assert_close(b.x, x["x"].as_f64().unwrap(), 1e-9, "bx");
                assert_close(b.y, x["y"].as_f64().unwrap(), 1e-9, "by");
                assert_eq!(b.variant, x["variant"].as_u64().unwrap() as usize, "variant");
                assert_close(b.scale, x["scale"].as_f64().unwrap(), 1e-9, "scale");
                assert_close(b.phase, x["phase"].as_f64().unwrap(), 1e-9, "phase");
            }
            assert_eq!(
                s.drifting_petals.iter().filter(|p| !p.burst).count(),
                exp["petalTotal"].as_u64().unwrap() as usize
            );
        }

        // Case 6: rosebrookPetalStep
        {
            let c = &lines[5];
            assert_eq!(c["fn"], "rosebrookPetalStep");
            let dt = c["input"]["dt"].as_f64().unwrap();
            let mut s = fresh_state(42, 72.0, 1.0, 0.85, 12, 1.0);
            s.drifting_petals.clear();
            s.sync_ambient_petals();
            s.spawn_petal_burst(500.0, 300.0, 6);
            for (step_i, tr) in c["output"]["trace"].as_array().unwrap().iter().enumerate() {
                s.step(dt);
                for (k, p) in s.drifting_petals.iter().take(4).enumerate() {
                    let e = &tr[k];
                    assert_close(p.x, e["x"].as_f64().unwrap(), 1e-8, &format!("step {step_i} petal {k} x"));
                    assert_close(p.y, e["y"].as_f64().unwrap(), 1e-8, &format!("step {step_i} petal {k} y"));
                    assert_close(p.rot, e["rot"].as_f64().unwrap(), 1e-8, &format!("step {step_i} petal {k} rot"));
                    assert_close(p.life, e["life"].as_f64().unwrap(), 1e-8, &format!("step {step_i} petal {k} life"));
                    assert_eq!(p.burst, e["burst"].as_bool().unwrap(), "burst");
                    assert_eq!(p.variant, e["variant"].as_u64().unwrap() as usize, "variant");
                }
            }
        }

        // Case 7: rosebrookBackOutEase
        {
            let c = &lines[6];
            assert_eq!(c["fn"], "rosebrookBackOutEase");
            for (t, e) in c["input"]["ts"].as_array().unwrap().iter().zip(
                c["output"]["eased"].as_array().unwrap().iter(),
            ) {
                assert_close(
                    RosebrookState::back_out_ease(t.as_f64().unwrap()),
                    e.as_f64().unwrap(),
                    1e-12,
                    "backOutEase",
                );
            }
        }
    }

    // Regression: moving a desktop icon must retire the old flower wreath and
    // grow a fresh one at the new position (JS commit() -> syncRoseBrookBlooms).
    #[test]
    fn icon_move_rebuilds_wreath_at_new_position() {
        let mut s = RosebrookState::new(1280.0, 720.0);
        let a = Rect { x: 200.0, y: 200.0, w: 76.0, h: 82.0 };
        s.set_icons(&[a], true);
        for _ in 0..10 {
            s.step(0.033);
        }
        assert!(!s.icon_blooms.is_empty(), "initial wreath exists");
        let old_cx = a.x as f64 + a.w as f64 / 2.0;
        assert!(s.icon_blooms.iter().all(|b| (b.ix - old_cx).abs() < 0.001));

        let b = Rect { x: 900.0, y: 120.0, w: 76.0, h: 82.0 };
        s.set_icons(&[b], true);
        let new_cx = b.x as f64 + b.w as f64 / 2.0;
        let new_cy = b.y as f64 + b.h as f64 / 2.0;
        assert!(!s.icon_blooms.is_empty(), "new wreath exists");
        assert!(
            s.icon_blooms.iter().all(|ib| (ib.ix - new_cx).abs() < 0.001 && (ib.iy - new_cy).abs() < 0.001),
            "old wreath removed, all blooms anchored to the moved icon"
        );
        assert!(
            s.icon_blooms.iter().all(|ib| ib.born > 0.0),
            "new blooms start their unfurl animation now"
        );
        assert!(
            s.drifting_petals.iter().any(|p| p.burst),
            "dropping an icon emits a petal burst"
        );
    }
}
