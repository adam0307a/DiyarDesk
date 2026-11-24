pub mod shaders;

use crate::beach::sample_arr;
use crate::gl::*;
use crate::meadow::{
    fbm, make_noise_image, IconBox, Mulberry32, MORPH_EMPTY_SEC, MORPH_SEC, STEP, TASKBAR,
};
use crate::state::{AppState, Rect};
use shaders::*;
use std::f64::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// Constants & Deterministic Hashes (1:1 prototype/glade.html + glade-trees.js)
// =========================================================================
pub const GLADE_TREE_VARIANTS: usize = 8;
pub const GLADE_BIRD_VARIANTS: usize = 3;
pub const GLADE_ATLAS_COLS: usize = GLADE_TREE_VARIANTS + GLADE_BIRD_VARIANTS;
pub const GLADE_SPRITE_W: usize = 128;
pub const GLADE_SPRITE_H: usize = 168;
pub const GLADE_BASE_Y: usize = 146;

#[inline]
pub fn glade_rand(seed: i32, a: i32, b: i32, k: i32) -> f64 {
    let mut h = seed
        .wrapping_add(a.wrapping_mul(374_761_393))
        .wrapping_mul(668_265_263)
        ^ b.wrapping_add(k.wrapping_mul(97))
            .wrapping_mul(2_246_822_519u32 as i32);
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    let u = (h as u32) ^ ((h as u32) >> 16);
    u as f64 / 4_294_967_296.0
}

#[inline]
pub fn atlas_hash(a: i32, b: i32) -> f64 {
    let mut x = a.wrapping_mul(374_761_393) ^ b.wrapping_mul(668_265_263);
    x = (x ^ ((x as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    let u = (x as u32) ^ ((x as u32) >> 16);
    u as f64 / 4_294_967_296.0
}

// =========================================================================
// Parameters & Simulation Entities
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct GladeParams {
    pub sigma: f64,
    pub warp: f64,
    pub density: f64,
    pub wind: f64,
    pub birds: usize,
    pub adaptive: bool,
    pub seed: i32,
    pub debug: bool,
}

impl Default for GladeParams {
    fn default() -> Self {
        Self {
            sigma: 78.0,
            warp: 28.0,
            density: 1.0,
            wind: 1.0,
            birds: 4,
            adaptive: true,
            seed: 42,
            debug: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct GladeTree {
    pub x: f64,
    pub y: f64,
    pub variant: usize,
    pub scale: f64,
    pub mirror: f64,
    pub phase: f64,
    pub grow: f64,
    pub want: f64,
}

#[derive(Clone, Copy)]
pub struct BirdFlight {
    pub sx: f64,
    pub sy: f64,
    pub tx: f64,
    pub ty: f64,
    pub to_idx: usize,
    pub t: f64,
    pub dur: f64,
}

#[derive(Clone, Copy)]
pub struct GladeBird {
    pub tree_idx: Option<usize>,
    pub x: f64,
    pub y: f64,
    pub variant: usize,
    pub mirror: f64,
    pub wait: f64,
    pub fly: Option<BirdFlight>,
}

// =========================================================================
// GladeState (CPU clearing SDF + forest + birds + morph)
// =========================================================================
pub struct GladeState {
    pub params: GladeParams,
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
    pub trees: Vec<GladeTree>,
    pub birds: Vec<GladeBird>,
    rng: Mulberry32,
}

impl Default for GladeState {
    fn default() -> Self {
        Self::new(1280.0, 720.0)
    }
}

impl GladeState {
    pub fn new(w: f64, h: f64) -> Self {
        let mut s = Self {
            params: GladeParams::default(),
            w: w.max(1.0),
            h: h.max(1.0),
            gw: 1,
            gh: 1,
            nz_wx: Vec::new(),
            nz_wy: Vec::new(),
            field_a: Vec::new(),
            field_b: Vec::new(),
            field_version: 0,
            noise_version: 1,
            morph_t0: -1e9,
            morph_dur: MORPH_SEC,
            cur_mix: 1.0,
            time_sec: 0.0,
            icons: Vec::new(),
            trees: Vec::new(),
            birds: Vec::new(),
            rng: Mulberry32::new(2026_0928),
        };
        s.resize(w as f32, h as f32);
        s.commit(true);
        s
    }

    pub fn build_noise(&mut self) {
        let n = self.gw * self.gh;
        self.nz_wx.resize(n, 0.0);
        self.nz_wy.resize(n, 0.0);
        let seed = self.params.seed;
        for gy in 0..self.gh {
            for gx in 0..self.gw {
                let i = gy * self.gw + gx;
                let x = gx as f64 * STEP;
                let y = gy as f64 * STEP;
                self.nz_wx[i] = (fbm(x * 0.0028, y * 0.0028, 1, seed) - 0.47) as f32;
                self.nz_wy[i] = (fbm(x * 0.0028, y * 0.0028, 2, seed) - 0.47) as f32;
            }
        }
    }

    pub fn icon_boxes(&self) -> Vec<IconBox> {
        self.icons
            .iter()
            .map(|ic| IconBox {
                cx: ic.x as f64 + ic.w as f64 * 0.5,
                cy: ic.y as f64 + ic.h as f64 * 0.5,
                hw: ic.w as f64 * 0.5 - 4.0,
                hh: ic.h as f64 * 0.5 - 2.0,
            })
            .collect()
    }

    pub fn compute_field_raw(&self, rects: &[IconBox]) -> Vec<f32> {
        let n = self.gw * self.gh;
        let mut f1 = vec![0.0f32; n];
        let sig = self.params.sigma;
        let is1 = 1.0 / (sig * sig);
        let wa = self.params.warp * 2.2;
        let wmax = wa * 0.5;
        for r in rects {
            let rad = 5.5 * sig + r.hw.max(r.hh) * 0.5 + wmax;
            let gx0 = ((r.cx - rad) / STEP).floor().max(0.0) as usize;
            let gx1 = (((r.cx + rad) / STEP).ceil() as isize)
                .clamp(0, (self.gw as isize) - 1) as usize;
            let gy0 = ((r.cy - rad) / STEP).floor().max(0.0) as usize;
            let gy1 = (((r.cy + rad) / STEP).ceil() as isize)
                .clamp(0, (self.gh as isize) - 1) as usize;
            let chw = r.hw * 0.5;
            let chh = r.hh * 0.5;
            for gy in gy0..=gy1 {
                for gx in gx0..=gx1 {
                    let i = gy * self.gw + gx;
                    let qx = gx as f64 * STEP + self.nz_wx[i] as f64 * wa;
                    let qy = gy as f64 * STEP + self.nz_wy[i] as f64 * wa;
                    let mut dx = (qx - r.cx).abs() - chw;
                    let mut dy = (qy - r.cy).abs() - chh;
                    if dx < 0.0 {
                        dx = 0.0;
                    }
                    if dy < 0.0 {
                        dy = 0.0;
                    }
                    f1[i] += (-(dx * dx + dy * dy) * is1).exp() as f32;
                }
            }
        }
        let c0 = (-0.35f64.ln()).sqrt();
        let f_min = (-30.0f64).exp();
        let mut s = vec![0.0f32; n];
        for i in 0..n {
            let g = (f1[i] as f64).max(f_min).ln();
            s[i] = (sig * ((-g).max(0.0).sqrt() - c0) - sig * 0.6 * g.max(0.0)) as f32;
        }
        s
    }

    pub fn land_s(&self, x: f64, y: f64) -> f64 {
        if self.field_b.is_empty() {
            999.0
        } else {
            sample_arr(&self.field_b, self.gw, self.gh, x, y)
        }
    }

    pub fn rebuild_forest(&mut self) {
        self.trees.clear();
        let dens = self.params.density.clamp(0.2, 1.5);
        let step_x = (52.0 / dens.sqrt()).round() as i32;
        let step_y = (42.0 / dens.sqrt()).round() as i32;
        let cols = ((self.w + step_x as f64) / step_x as f64).ceil() as i32 + 1;
        let rows = ((self.h + step_y as f64) / step_y as f64).ceil() as i32 + 1;
        let seed = self.params.seed;

        for gy in 0..rows {
            for gx in 0..cols {
                let r1 = glade_rand(seed, gx, gy, 1);
                let r2 = glade_rand(seed, gx, gy, 2);
                let r3 = glade_rand(seed, gx, gy, 3);
                let r4 = glade_rand(seed, gx, gy, 4);
                let r5 = glade_rand(seed, gx, gy, 5);
                if r5 < 0.08 {
                    continue;
                }
                let offset_x = (gy % 2) as f64 * (step_x as f64 * 0.5);
                let x = gx as f64 * step_x as f64 - 24.0 + offset_x + (r1 - 0.5) * step_x as f64 * 0.72;
                let y = gy as f64 * step_y as f64 - 10.0 + (r2 - 0.5) * step_y as f64 * 0.72;
                if x < -30.0 || x > self.w + 30.0 || y < 12.0 || y > self.h + 56.0 {
                    continue;
                }
                let variant = (r3 * GLADE_TREE_VARIANTS as f64).floor() as usize;
                let scale = 0.72 + r4 * 0.48;
                self.trees.push(GladeTree {
                    x,
                    y,
                    variant: variant.min(GLADE_TREE_VARIANTS - 1),
                    scale,
                    mirror: if r1 < 0.5 { -1.0 } else { 1.0 },
                    phase: r2 * TAU,
                    grow: 0.0,
                    want: 1.0,
                });
            }
        }
        self.trees.sort_by(|a, b| a.y.total_cmp(&b.y));
        self.update_targets(true);
        self.sync_birds(true);
    }

    pub fn update_targets(&mut self, instant: bool) {
        let adaptive = self.params.adaptive;
        for i in 0..self.trees.len() {
            let tx = self.trees[i].x;
            let ty = self.trees[i].y;
            let ts = self.trees[i].scale;
            let s_foot = self.land_s(tx, ty);
            let s_crown = self.land_s(tx, ty - 24.0 * ts);
            let s = s_foot.min(s_crown);
            let want = if !adaptive || s > 4.0 { 1.0 } else { 0.0 };
            self.trees[i].want = want;
            if instant {
                self.trees[i].grow = want;
            }
        }
    }

    pub fn sync_birds(&mut self, reset: bool) {
        if reset {
            self.birds.clear();
        }
        let want = self.params.birds.min(10);
        while self.birds.len() > want {
            self.birds.pop();
        }
        let grown: Vec<usize> = self
            .trees
            .iter()
            .enumerate()
            .filter_map(|(idx, t)| {
                if t.want > 0.5
                    && t.x > 40.0
                    && t.x < self.w - 40.0
                    && t.y > 60.0
                    && t.y < self.h - TASKBAR - 20.0
                {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        while self.birds.len() < want && !grown.is_empty() {
            let pick = ((self.rng.next() * grown.len() as f64).floor() as usize)
                .min(grown.len() - 1);
            let tidx = grown[pick];
            let t = self.trees[tidx];
            let variant = ((self.rng.next() * GLADE_BIRD_VARIANTS as f64).floor() as usize)
                .min(GLADE_BIRD_VARIANTS - 1);
            let mirror = if self.rng.next() < 0.5 { -1.0 } else { 1.0 };
            let wait = 2.0 + self.rng.next() * 5.0;
            self.birds.push(GladeBird {
                tree_idx: Some(tidx),
                x: t.x,
                y: t.y - 82.0 * t.scale,
                variant,
                mirror,
                wait,
                fly: None,
            });
        }
    }

    pub fn commit(&mut self, from_empty: bool) {
        let rects = if self.params.adaptive {
            self.icon_boxes()
        } else {
            Vec::new()
        };
        let nf = self.compute_field_raw(&rects);
        if from_empty || self.field_b.len() != nf.len() {
            self.field_a = self.compute_field_raw(&[]);
        } else {
            std::mem::swap(&mut self.field_a, &mut self.field_b);
        }
        self.field_b = nf;
        self.morph_t0 = self.time_sec;
        self.morph_dur = if from_empty {
            MORPH_EMPTY_SEC
        } else {
            MORPH_SEC
        };
        self.cur_mix = 0.0;
        self.field_version = self.field_version.wrapping_add(1);
        if self.trees.is_empty() {
            self.rebuild_forest();
        }
        self.update_targets(false);
        self.sync_birds(false);
    }

    pub fn refresh(&mut self) {
        let rects = if self.params.adaptive {
            self.icon_boxes()
        } else {
            Vec::new()
        };
        let nf = self.compute_field_raw(&rects);
        self.field_a = nf.clone();
        self.field_b = nf;
        self.morph_t0 = -1e9;
        self.cur_mix = 1.0;
        self.field_version = self.field_version.wrapping_add(1);
        self.update_targets(true);
        self.sync_birds(false);
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let w = (w as f64).max(1.0);
        let h = (h as f64).max(1.0);
        let new_gw = (w / STEP).ceil() as usize + 2;
        let new_gh = (h / STEP).ceil() as usize + 2;
        if (w - self.w).abs() < 0.5
            && (h - self.h).abs() < 0.5
            && new_gw == self.gw
            && new_gh == self.gh
            && !self.nz_wx.is_empty()
        {
            return;
        }
        self.w = w;
        self.h = h;
        self.gw = new_gw;
        self.gh = new_gh;
        self.build_noise();
        self.refresh();
        self.rebuild_forest();
    }

    pub fn set_icons(&mut self, icons: &[Rect], vis: bool) {
        let next: Vec<Rect> = if vis { icons.to_vec() } else { Vec::new() };
        self.set_icons_raw(&next, true);
    }

    pub fn set_icons_raw(&mut self, icons: &[Rect], animate: bool) {
        let same = self.icons.len() == icons.len()
            && self.icons.iter().zip(icons.iter()).all(|(a, b)| {
                (a.x - b.x).abs() < 0.5
                    && (a.y - b.y).abs() < 0.5
                    && (a.w - b.w).abs() < 0.5
                    && (a.h - b.h).abs() < 0.5
            });
        if same {
            return;
        }
        let was_empty = self.icons.is_empty();
        self.icons = icons.to_vec();
        if animate {
            self.commit(was_empty);
        } else {
            self.refresh();
        }
    }

    pub fn sync_params(&mut self, next: GladeParams) {
        let prev = self.params;
        if prev == next {
            return;
        }
        self.params = next;
        let seed_changed = prev.seed != next.seed;
        let field_changed = (prev.sigma - next.sigma).abs() > 1e-6
            || (prev.warp - next.warp).abs() > 1e-6
            || prev.adaptive != next.adaptive;
        let density_changed = (prev.density - next.density).abs() > 1e-6;
        let birds_changed = prev.birds != next.birds;

        if seed_changed {
            self.build_noise();
            self.noise_version = self.noise_version.wrapping_add(1);
            self.refresh();
            self.rebuild_forest();
        } else {
            if field_changed {
                self.refresh();
            }
            if density_changed {
                self.rebuild_forest();
            } else if birds_changed {
                self.sync_birds(false);
            }
        }
    }

    pub fn step_forest(&mut self, dt: f64) {
        for t in &mut self.trees {
            if (t.grow - t.want).abs() < 1e-12 {
                continue;
            }
            if t.grow < t.want {
                t.grow = (t.grow + dt * 1.45).min(t.want);
            } else {
                t.grow = (t.grow - dt * 1.75).max(t.want);
            }
        }

        let w = self.w;
        let h = self.h;
        for i in (0..self.birds.len()).rev() {
            if let Some(mut fly) = self.birds[i].fly {
                fly.t += dt / fly.dur;
                if fly.t >= 1.0 {
                    let dst_idx = fly.to_idx.min(self.trees.len().saturating_sub(1));
                    if let Some(dst) = self.trees.get(dst_idx).copied() {
                        self.birds[i].tree_idx = Some(dst_idx);
                        self.birds[i].x = dst.x;
                        self.birds[i].y = dst.y - 82.0 * dst.scale;
                    }
                    self.birds[i].fly = None;
                    self.birds[i].wait = 2.5 + self.rng.next() * 6.0;
                } else {
                    let u = fly.t;
                    let arc = (u * PI).sin() * 26.0;
                    self.birds[i].x = fly.sx + (fly.tx - fly.sx) * u;
                    self.birds[i].y = fly.sy + (fly.ty - fly.sy) * u - arc;
                    self.birds[i].fly = Some(fly);
                }
                continue;
            }

            let cur_tree = self.birds[i].tree_idx.and_then(|idx| self.trees.get(idx).copied());
            match cur_tree {
                Some(t) if t.want >= 0.5 && t.grow >= 0.6 => {
                    self.birds[i].x = t.x;
                    self.birds[i].y = t.y - 82.0 * t.scale * t.grow;
                    self.birds[i].wait -= dt;
                }
                _ => {
                    self.birds[i].wait = 0.0;
                }
            }

            if self.birds[i].wait <= 0.0 {
                let cur_idx = self.birds[i].tree_idx;
                let cands: Vec<usize> = self
                    .trees
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, t)| {
                        if Some(idx) != cur_idx
                            && t.want > 0.5
                            && t.grow > 0.8
                            && t.x > 30.0
                            && t.x < w - 30.0
                            && t.y > 50.0
                            && t.y < h - TASKBAR - 20.0
                        {
                            Some(idx)
                        } else {
                            None
                        }
                    })
                    .collect();

                if !cands.is_empty() {
                    let pick = ((self.rng.next() * cands.len() as f64).floor() as usize)
                        .min(cands.len() - 1);
                    let dst_idx = cands[pick];
                    let dst = self.trees[dst_idx];
                    let tx = dst.x;
                    let ty = dst.y - 82.0 * dst.scale;
                    let bx = self.birds[i].x;
                    let by = self.birds[i].y;
                    self.birds[i].mirror = if tx >= bx { 1.0 } else { -1.0 };
                    let dist = (tx - bx).hypot(ty - by);
                    self.birds[i].fly = Some(BirdFlight {
                        sx: bx,
                        sy: by,
                        tx,
                        ty,
                        to_idx: dst_idx,
                        t: 0.0,
                        dur: (dist / 160.0).clamp(0.7, 2.6),
                    });
                } else {
                    self.birds[i].wait = 2.0;
                }
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        let dt = (dt as f64).clamp(0.0, 0.1);
        self.time_sec += dt;
        let e = ((self.time_sec - self.morph_t0) / self.morph_dur).clamp(0.0, 1.0);
        self.cur_mix = if e < 0.5 {
            2.0 * e * e
        } else {
            1.0 - (-2.0 * e + 2.0).powi(2) * 0.5
        };
        self.step_forest(dt);
    }
}

// =========================================================================
// Procedural 2x Gouache Sprite Atlas (1:1 prototype/glade-trees.js)
// =========================================================================
fn hex_rgb(hex: u32) -> [f64; 3] {
    [
        ((hex >> 16) & 0xff) as f64 / 255.0,
        ((hex >> 8) & 0xff) as f64 / 255.0,
        (hex & 0xff) as f64 / 255.0,
    ]
}

fn eval_stops(stops: &[(f64, [f64; 3])], t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        if t <= w[1].0 {
            let f = if (w[1].0 - w[0].0).abs() < 1e-9 {
                0.0
            } else {
                (t - w[0].0) / (w[1].0 - w[0].0)
            };
            return [
                w[0].1[0] + (w[1].1[0] - w[0].1[0]) * f,
                w[0].1[1] + (w[1].1[1] - w[0].1[1]) * f,
                w[0].1[2] + (w[1].1[2] - w[0].1[2]) * f,
            ];
        }
    }
    stops.last().map(|s| s.1).unwrap_or([0.0, 0.0, 0.0])
}

struct Canvas2D {
    w: usize,
    h: usize,
    scale: f64,
    px: Vec<u8>,
}

impl Canvas2D {
    fn new(logical_w: usize, logical_h: usize, scale: usize) -> Self {
        let w = logical_w * scale;
        let h = logical_h * scale;
        Self {
            w,
            h,
            scale: scale as f64,
            px: vec![0u8; w * h * 4],
        }
    }

    #[inline]
    fn blend_px(&mut self, x: usize, y: usize, r: f64, g: f64, b: f64, a: f64) {
        if a <= 0.001 || x >= self.w || y >= self.h {
            return;
        }
        let idx = (y * self.w + x) * 4;
        let sa = a.clamp(0.0, 1.0);
        let da = self.px[idx + 3] as f64 / 255.0;
        let out_a = sa + da * (1.0 - sa);
        if out_a <= 1e-6 {
            return;
        }
        let dr = self.px[idx] as f64 / 255.0;
        let dg = self.px[idx + 1] as f64 / 255.0;
        let db = self.px[idx + 2] as f64 / 255.0;
        let inv = 1.0 / out_a;
        let nr = (r * sa + dr * da * (1.0 - sa)) * inv;
        let ng = (g * sa + dg * da * (1.0 - sa)) * inv;
        let nb = (b * sa + db * da * (1.0 - sa)) * inv;
        self.px[idx] = (nr.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.px[idx + 1] = (ng.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.px[idx + 2] = (nb.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.px[idx + 3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, rgb: [f64; 3], a: f64) {
        let s = self.scale;
        let x0 = x * s;
        let y0 = y * s;
        let x1 = (x + w) * s;
        let y1 = (y + h) * s;
        let ix0 = (x0.floor() as isize).max(0) as usize;
        let iy0 = (y0.floor() as isize).max(0) as usize;
        let ix1 = (x1.ceil() as isize).min(self.w as isize) as usize;
        let iy1 = (y1.ceil() as isize).min(self.h as isize) as usize;
        for py in iy0..iy1 {
            let cy = ((py as f64 + 1.0).min(y1) - (py as f64).max(y0)).max(0.0);
            for px in ix0..ix1 {
                let cx = ((px as f64 + 1.0).min(x1) - (px as f64).max(x0)).max(0.0);
                let cov = cx * cy;
                if cov > 0.001 {
                    self.blend_px(px, py, rgb[0], rgb[1], rgb[2], a * cov);
                }
            }
        }
    }

    fn fill_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, rgb: [f64; 3], a: f64) {
        let s = self.scale;
        let scx = cx * s;
        let scy = cy * s;
        let srx = (rx * s).max(0.5);
        let sry = (ry * s).max(0.5);
        let max_r = srx.max(sry) + 2.0;
        let ix0 = ((scx - max_r).floor() as isize).max(0) as usize;
        let iy0 = ((scy - max_r).floor() as isize).max(0) as usize;
        let ix1 = ((scx + max_r).ceil() as isize).min(self.w as isize) as usize;
        let iy1 = ((scy + max_r).ceil() as isize).min(self.h as isize) as usize;
        let cos_r = rot.cos();
        let sin_r = rot.sin();
        for py in iy0..iy1 {
            for px in ix0..ix1 {
                let dx = px as f64 + 0.5 - scx;
                let dy = py as f64 + 0.5 - scy;
                let lx = dx * cos_r + dy * sin_r;
                let ly = -dx * sin_r + dy * cos_r;
                let nx = lx / srx;
                let ny = ly / sry;
                let d2 = nx * nx + ny * ny;
                if d2 < 1.35 {
                    let d = d2.sqrt();
                    let grad_len = (nx / srx).hypot(ny / sry).max(1e-6);
                    let signed_px = (d - 1.0) / grad_len;
                    let cov = (0.5 - signed_px).clamp(0.0, 1.0);
                    if cov > 0.001 {
                        self.blend_px(px, py, rgb[0], rgb[1], rgb[2], a * cov);
                    }
                }
            }
        }
    }

    fn fill_polygon<F>(&mut self, pts: &[(f64, f64)], color_fn: F)
    where
        F: Fn(f64, f64) -> ([f64; 3], f64),
    {
        if pts.len() < 3 {
            return;
        }
        let s = self.scale;
        let spts: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| (x * s, y * s)).collect();
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for &(x, y) in &spts {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let ix0 = (min_x.floor() as isize - 1).max(0) as usize;
        let ix1 = (max_x.ceil() as isize + 1).min(self.w as isize) as usize;
        let iy0 = (min_y.floor() as isize - 1).max(0) as usize;
        let iy1 = (max_y.ceil() as isize + 1).min(self.h as isize) as usize;

        // 4x4 subpixel scanline anti-aliasing for silky-smooth Canvas2D-grade edges
        const SUB: usize = 4;
        let inv_sub = 1.0 / SUB as f64;
        let n = spts.len();
        let mut xs: Vec<f64> = Vec::with_capacity(16);

        for py in iy0..iy1 {
            let mut row_cov = vec![0u8; ix1.saturating_sub(ix0)];
            for sy in 0..SUB {
                let sample_y = py as f64 + (sy as f64 + 0.5) * inv_sub;
                xs.clear();
                for i in 0..n {
                    let (x0, y0) = spts[i];
                    let (x1, y1) = spts[(i + 1) % n];
                    if (y0 <= sample_y && y1 > sample_y) || (y1 <= sample_y && y0 > sample_y) {
                        let t = (sample_y - y0) / (y1 - y0);
                        xs.push(x0 + t * (x1 - x0));
                    }
                }
                if xs.len() < 2 {
                    continue;
                }
                xs.sort_by(|a, b| a.total_cmp(b));
                for pair in xs.chunks_exact(2) {
                    let sx0 = pair[0];
                    let sx1 = pair[1];
                    let px0 = (sx0.floor() as isize).max(ix0 as isize) as usize;
                    let px1 = (sx1.ceil() as isize).min(ix1 as isize) as usize;
                    for px in px0..px1 {
                        let mut hits = 0u8;
                        for sx in 0..SUB {
                            let sample_x = px as f64 + (sx as f64 + 0.5) * inv_sub;
                            if sample_x >= sx0 && sample_x <= sx1 {
                                hits += 1;
                            }
                        }
                        row_cov[px - ix0] += hits;
                    }
                }
            }
            let ly = (py as f64 + 0.5) / s;
            for (k, &hits) in row_cov.iter().enumerate() {
                if hits == 0 {
                    continue;
                }
                let px = ix0 + k;
                let lx = (px as f64 + 0.5) / s;
                let cov = hits as f64 / (SUB * SUB) as f64;
                let (rgb, a) = color_fn(lx, ly);
                self.blend_px(px, py, rgb[0], rgb[1], rgb[2], a * cov);
            }
        }
    }

    fn stroke_polyline(&mut self, pts: &[(f64, f64)], line_w: f64, rgb: [f64; 3], a: f64) {
        if pts.len() < 2 {
            return;
        }
        let s = self.scale;
        let half = (line_w * 0.5 * s).max(0.5);
        let pad = half + 1.5;
        let spts: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| (x * s, y * s)).collect();
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for &(x, y) in &spts {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let ix0 = ((min_x - pad).floor() as isize).max(0) as usize;
        let ix1 = ((max_x + pad).ceil() as isize).min(self.w as isize) as usize;
        let iy0 = ((min_y - pad).floor() as isize).max(0) as usize;
        let iy1 = ((max_y + pad).ceil() as isize).min(self.h as isize) as usize;

        for py in iy0..iy1 {
            let fy = py as f64 + 0.5;
            for px in ix0..ix1 {
                let fx = px as f64 + 0.5;
                let mut min_d = f64::INFINITY;
                for seg in spts.windows(2) {
                    let (ax, ay) = seg[0];
                    let (bx, by) = seg[1];
                    let bax = bx - ax;
                    let bay = by - ay;
                    let denom = bax * bax + bay * bay;
                    let t = if denom > 1e-9 {
                        (((fx - ax) * bax + (fy - ay) * bay) / denom).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let d = (fx - (ax + bax * t)).hypot(fy - (ay + bay * t));
                    if d < min_d {
                        min_d = d;
                    }
                }
                let cov = (half + 0.5 - min_d).clamp(0.0, 1.0);
                if cov > 0.001 {
                    self.blend_px(px, py, rgb[0], rgb[1], rgb[2], a * cov);
                }
            }
        }
    }

    fn stroke_arc(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64, line_w: f64, rgb: [f64; 3], a: f64) {
        let steps = ((r * (a1 - a0).abs()).ceil() as usize).clamp(8, 32);
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let ang = a0 + (a1 - a0) * (i as f64 / steps as f64);
            pts.push((cx + ang.cos() * r, cy + ang.sin() * r));
        }
        self.stroke_polyline(&pts, line_w, rgb, a);
    }
}

fn push_quad_bezier(out: &mut Vec<(f64, f64)>, p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), steps: usize) {
    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let u = 1.0 - t;
        let x = u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0;
        let y = u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1;
        out.push((x, y));
    }
}

fn push_cubic_bezier(
    out: &mut Vec<(f64, f64)>,
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    steps: usize,
) {
    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let u = 1.0 - t;
        let x = u * u * u * p0.0
            + 3.0 * u * u * t * p1.0
            + 3.0 * u * t * t * p2.0
            + t * t * t * p3.0;
        let y = u * u * u * p0.1
            + 3.0 * u * u * t * p1.1
            + 3.0 * u * t * t * p2.1
            + t * t * t * p3.1;
        out.push((x, y));
    }
}

fn draw_shadow(cv: &mut Canvas2D, cx: f64, by: f64, rx: f64, ry: f64) {
    cv.fill_ellipse(
        cx + 12.0,
        by + 3.0,
        rx,
        ry,
        0.08,
        [54.0 / 255.0, 88.0 / 255.0, 44.0 / 255.0],
        0.28,
    );
}

fn draw_trunk(cv: &mut Canvas2D, cx: f64, by: f64, w: f64, h: f64, hex: u32) {
    let col = hex_rgb(hex);
    let pts = [
        (cx - w * 0.5, by),
        (cx - w * 0.36, by - h),
        (cx + w * 0.36, by - h),
        (cx + w * 0.5, by),
    ];
    cv.fill_polygon(&pts, |_, _| (col, 1.0));
    cv.fill_rect(cx, by - h, w * 0.42, h, [0.0, 0.0, 0.0], 0.16);
}

fn paint_conifer(
    cv: &mut Canvas2D,
    cx: f64,
    by: f64,
    width: f64,
    height: f64,
    tiers: i32,
    c_top: u32,
    c_bot: u32,
    c_hi: u32,
    seed_base: i32,
) {
    draw_shadow(cv, cx, by, width * 0.48, 7.5);
    draw_trunk(cv, cx, by, (width * 0.14).max(6.0), 22.0, 0x624634);
    let trunk_top = by - 14.0;
    let stops = [(0.0, hex_rgb(c_hi)), (0.35, hex_rgb(c_top)), (1.0, hex_rgb(c_bot))];
    let stroke_rgb = [18.0 / 255.0, 42.0 / 255.0, 30.0 / 255.0];

    for t in 0..tiers {
        let f0 = t as f64 / tiers as f64;
        let f1 = (t as f64 + 1.18) / tiers as f64;
        let y_bot = trunk_top - height * f0 * 0.78;
        let y_top = trunk_top - height * f1.min(1.0);
        let half_w = width * 0.5 * (1.0 - f0 * 0.52);

        let mut pts = Vec::with_capacity(32);
        let apex = (cx + (atlas_hash(seed_base, t * 3) - 0.5) * 3.0, y_top);
        pts.push(apex);
        let steps = 6;
        for i in 1..=steps {
            let u = i as f64 / steps as f64;
            let px = cx + half_w * u + (atlas_hash(seed_base + 11, t * 13 + i) - 0.5) * 4.0;
            let py = y_top
                + (y_bot - y_top) * u
                + if i < steps {
                    (atlas_hash(seed_base + 21, t * 17 + i) - 0.5) * 3.0
                } else {
                    0.0
                };
            pts.push((px, py));
        }
        let last_r = *pts.last().unwrap();
        push_quad_bezier(&mut pts, last_r, (cx, y_bot + 5.0), (cx - half_w, y_bot), 12);
        for i in (1..steps).rev() {
            let u = i as f64 / steps as f64;
            let px = cx - half_w * u + (atlas_hash(seed_base + 31, t * 19 + i) - 0.5) * 4.0;
            let py = y_top + (y_bot - y_top) * u + (atlas_hash(seed_base + 41, t * 23 + i) - 0.5) * 3.0;
            pts.push((px, py));
        }

        let gx0 = cx - half_w;
        let gy0 = y_top;
        let gx1 = cx + half_w * 0.7;
        let gy1 = y_bot;
        let gdx = gx1 - gx0;
        let gdy = gy1 - gy0;
        let glen2 = (gdx * gdx + gdy * gdy).max(1e-6);
        cv.fill_polygon(&pts, |x, y| {
            let proj = ((x - gx0) * gdx + (y - gy0) * gdy) / glen2;
            (eval_stops(&stops, proj), 1.0)
        });

        for k in 0..3 {
            let sy = y_top + (y_bot - y_top) * (0.38 + k as f64 * 0.22);
            let sw = half_w * (0.28 + k as f64 * 0.16);
            cv.stroke_arc(cx - sw * 0.2, sy, sw * 0.5, 0.2, 1.3, 1.2, stroke_rgb, 0.16);
        }
    }
}

fn paint_round_crown(
    cv: &mut Canvas2D,
    cx: f64,
    by: f64,
    rx: f64,
    ry: f64,
    lobes: i32,
    c_top: u32,
    c_bot: u32,
    c_hi: u32,
    scallops: bool,
    seed_base: i32,
) {
    draw_shadow(cv, cx, by, rx * 0.85, 8.5);
    draw_trunk(cv, cx, by, (rx * 0.24).max(7.0), 32.0, 0x6d4c37);
    let cy = by - 22.0 - ry * 0.85;

    let mut pts = Vec::with_capacity((lobes as usize) * 10 + 1);
    for i in 0..lobes {
        let a0 = (i as f64 / lobes as f64) * TAU;
        let a1 = ((i as f64 + 0.5) / lobes as f64) * TAU;
        let a2 = ((i as f64 + 1.0) / lobes as f64) * TAU;
        let r0 = 0.88 + atlas_hash(seed_base, i * 7) * 0.18;
        let r1 = 1.02 + atlas_hash(seed_base + 5, i * 11) * 0.16;
        let p0 = (cx + a0.cos() * rx * r0, cy + a0.sin() * ry * r0);
        let p1 = (cx + a1.cos() * rx * r1, cy + a1.sin() * ry * r1);
        let p2 = (cx + a2.cos() * rx * r0, cy + a2.sin() * ry * r0);
        if i == 0 {
            pts.push(p0);
        }
        let prev = *pts.last().unwrap();
        push_quad_bezier(&mut pts, prev, p1, p2, 10);
    }

    let stops = [(0.0, hex_rgb(c_hi)), (0.5, hex_rgb(c_top)), (1.0, hex_rgb(c_bot))];
    let gx0 = cx - rx * 0.32;
    let gy0 = cy - ry * 0.38;
    let r0 = rx * 0.1;
    let gx1 = cx;
    let gy1 = cy;
    let r1 = rx.max(ry) * 1.15;
    let dx = gx1 - gx0;
    let dy = gy1 - gy0;
    let dr = r1 - r0;
    let qa = dx * dx + dy * dy - dr * dr;

    cv.fill_polygon(&pts, |x, y| {
        let px = x - gx0;
        let py = y - gy0;
        let qb = -(px * dx + py * dy + r0 * dr);
        let qc = px * px + py * py - r0 * r0;
        let disc = (qb * qb - qa * qc).max(0.0);
        let t = if qa.abs() > 1e-9 {
            (-qb - disc.sqrt()) / qa
        } else if qb.abs() > 1e-9 {
            -qc / (2.0 * qb)
        } else {
            0.0
        };
        (eval_stops(&stops, t), 1.0)
    });

    if scallops {
        let stroke_rgb = [20.0 / 255.0, 48.0 / 255.0, 28.0 / 255.0];
        for row in 0..4 {
            let sy = cy - ry * 0.45 + row as f64 * (ry * 0.34);
            let span = rx * (0.55 + if row == 1 || row == 2 { 0.25 } else { 0.0 });
            let count = 3 + (row % 2);
            for k in 0..count {
                let sx = cx - span * 0.65 + (k as f64 / (count - 1).max(1) as f64) * span * 1.3;
                cv.stroke_arc(sx, sy, 6.2, 0.2, PI - 0.2, 1.25, stroke_rgb, 0.16);
            }
        }
    }
}

fn paint_cypress(
    cv: &mut Canvas2D,
    cx: f64,
    by: f64,
    rx: f64,
    height: f64,
    c_top: u32,
    c_bot: u32,
    c_hi: u32,
    seed_base: i32,
) {
    draw_shadow(cv, cx, by, rx * 0.9, 6.5);
    draw_trunk(cv, cx, by, (rx * 0.28).max(5.5), 24.0, 0x604535);
    let bot = by - 16.0;
    let top = bot - height;

    let mut pts = Vec::with_capacity(50);
    pts.push((cx, top));
    push_cubic_bezier(
        &mut pts,
        (cx, top),
        (cx + rx * 0.9, top + height * 0.24),
        (cx + rx * 1.15, bot - height * 0.18),
        (cx, bot + 3.0),
        24,
    );
    push_cubic_bezier(
        &mut pts,
        (cx, bot + 3.0),
        (cx - rx * 1.15, bot - height * 0.18),
        (cx - rx * 0.9, top + height * 0.24),
        (cx, top),
        24,
    );

    let stops = [(0.0, hex_rgb(c_hi)), (0.42, hex_rgb(c_top)), (1.0, hex_rgb(c_bot))];
    let gx0 = cx - rx;
    let gy0 = top;
    let gx1 = cx + rx;
    let gy1 = bot;
    let gdx = gx1 - gx0;
    let gdy = gy1 - gy0;
    let glen2 = (gdx * gdx + gdy * gdy).max(1e-6);
    cv.fill_polygon(&pts, |x, y| {
        let proj = ((x - gx0) * gdx + (y - gy0) * gdy) / glen2;
        (eval_stops(&stops, proj), 1.0)
    });

    let stroke_rgb = [15.0 / 255.0, 38.0 / 255.0, 28.0 / 255.0];
    for k in 0..4 {
        let yy = top + height * (0.28 + k as f64 * 0.16);
        let off = (atlas_hash(seed_base, k * 9) - 0.5) * rx * 0.5;
        let mut spts = Vec::with_capacity(11);
        spts.push((cx + off - 4.0, yy));
        push_quad_bezier(
            &mut spts,
            (cx + off - 4.0, yy),
            (cx + off, yy + 4.0),
            (cx + off + 5.0, yy - 1.0),
            10,
        );
        cv.stroke_polyline(&spts, 1.1, stroke_rgb, 0.15);
    }
}

pub fn build_glade_sprite_atlas() -> (u32, u32, Vec<u8>) {
    let mut cv = Canvas2D::new(GLADE_ATLAS_COLS * GLADE_SPRITE_W, GLADE_SPRITE_H, 2);
    let sw = GLADE_SPRITE_W as f64;
    let by = GLADE_BASE_Y as f64;

    // 8 tree variants (exact palette & params from prototype/glade-trees.js lines 187-194)
    paint_conifer(&mut cv, 0.0 * sw + 64.0, by, 64.0, 118.0, 3, 0x255c47, 0x16382d, 0x33745b, 10);
    paint_conifer(&mut cv, 1.0 * sw + 64.0, by, 56.0, 108.0, 3, 0x97b552, 0x708f3b, 0xb4cf67, 20);
    paint_round_crown(&mut cv, 2.0 * sw + 64.0, by, 36.0, 38.0, 9, 0x62a45d, 0x3c7744, 0x86c16c, true, 30);
    paint_round_crown(&mut cv, 3.0 * sw + 64.0, by, 32.0, 34.0, 8, 0x286446, 0x184031, 0x3d8058, false, 40);
    paint_cypress(&mut cv, 4.0 * sw + 64.0, by, 21.0, 108.0, 0x4e8a7e, 0x32635f, 0x6ca494, 50);
    paint_conifer(&mut cv, 5.0 * sw + 64.0, by, 62.0, 112.0, 4, 0x36756a, 0x1f4b48, 0x4e9183, 60);
    paint_round_crown(&mut cv, 6.0 * sw + 64.0, by, 37.0, 33.0, 10, 0x85ad58, 0x5b833e, 0xa3c66c, true, 70);
    paint_cypress(&mut cv, 7.0 * sw + 64.0, by, 22.0, 114.0, 0x2c5b43, 0x19392a, 0x427858, 80);

    // 3 woodland bird variants (8: white tit, 9: red-breasted robin, 10: magpie)
    struct BirdPal {
        body: u32,
        wing: u32,
        chest: u32,
        head: u32,
        beak: u32,
    }
    let birds = [
        BirdPal { body: 0xf7f8f3, wing: 0xd9e0d2, chest: 0xf7f8f3, head: 0xeef1e6, beak: 0xd99838 },
        BirdPal { body: 0x6e5242, wing: 0x523c30, chest: 0xdf633c, head: 0x624839, beak: 0x332820 },
        BirdPal { body: 0x232832, wing: 0xf1f4f6, chest: 0xf1f4f6, head: 0x1c2028, beak: 0x404856 },
    ];
    for (b, pal) in birds.iter().enumerate() {
        let ox = (GLADE_TREE_VARIANTS + b) as f64 * sw + 64.0;
        let oy = GLADE_SPRITE_H as f64 * 0.5;
        // Tail
        let tail = [(ox - 6.0, oy + 1.0), (ox - 16.0, oy + 5.0), (ox - 13.0, oy - 1.0)];
        let wing_rgb = hex_rgb(pal.wing);
        cv.fill_polygon(&tail, |_, _| (wing_rgb, 1.0));
        // Body
        cv.fill_ellipse(ox, oy, 8.5, 5.5, -0.2, hex_rgb(pal.body), 1.0);
        // Chest accent
        cv.fill_ellipse(ox + 3.0, oy + 1.5, 5.0, 4.0, -0.1, hex_rgb(pal.chest), 1.0);
        // Wing highlight
        cv.fill_ellipse(ox - 2.0, oy - 1.0, 6.0, 3.3, -0.25, wing_rgb, 1.0);
        // Head
        cv.fill_ellipse(ox + 7.0, oy - 4.0, 4.2, 4.2, 0.0, hex_rgb(pal.head), 1.0);
        // Beak
        let beak = [(ox + 10.5, oy - 5.0), (ox + 15.0, oy - 3.5), (ox + 10.5, oy - 2.2)];
        let beak_rgb = hex_rgb(pal.beak);
        cv.fill_polygon(&beak, |_, _| (beak_rgb, 1.0));
        // Eye
        cv.fill_ellipse(ox + 8.3, oy - 4.6, 0.9, 0.9, 0.0, hex_rgb(0x111111), 1.0);
    }

    (cv.w as u32, cv.h as u32, cv.px)
}

// =========================================================================
// OpenGL 3.3 Core Renderer
// =========================================================================
pub struct GladeRenderer {
    ground_prog: GLuint,
    inst_prog: GLuint,
    bg_vao: GLuint,
    inst_vao: GLuint,
    inst_buf_a: GLuint,
    inst_buf_b: GLuint,
    max_inst: usize,
    noise_tex: GLuint,
    field_tex: [GLuint; 2],
    atlas_tex: GLuint,
    field_gw: usize,
    field_gh: usize,
    uploaded_field_version: u64,
    uploaded_noise_version: u64,
    // Uniform locations: ground
    u_noise: GLint,
    u_fa: GLint,
    u_fb: GLint,
    u_res: GLint,
    u_grid: GLint,
    u_scale: GLint,
    u_seed: GLint,
    u_step: GLint,
    u_mix: GLint,
    u_debug: GLint,
    // Uniform locations: instanced forest + birds
    ui_atlas: GLint,
    ui_res: GLint,
    ui_time: GLint,
    ui_wind: GLint,
    // Reusable CPU staging buffers
    buf_a: Vec<f32>,
    buf_b: Vec<f32>,
}

impl GladeRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let ground_prog = gl.compile_program(GLADE_VERT, GLADE_FRAG)?;
        let inst_prog = gl.compile_program(GLADE_INST_VERT, GLADE_INST_FRAG)?;

        let mut bg_vao = 0;
        (gl.GenVertexArrays)(1, &mut bg_vao);

        let mut inst_vao = 0;
        (gl.GenVertexArrays)(1, &mut inst_vao);
        (gl.BindVertexArray)(inst_vao);

        let mut inst_buf_a = 0;
        let mut inst_buf_b = 0;
        (gl.GenBuffers)(1, &mut inst_buf_a);
        (gl.GenBuffers)(1, &mut inst_buf_b);

        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_buf_a);
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 4, GL_FLOAT, GL_FALSE, 0, std::ptr::null());
        (gl.VertexAttribDivisor)(0, 1);

        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_buf_b);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, 0, std::ptr::null());
        (gl.VertexAttribDivisor)(1, 1);
        (gl.BindVertexArray)(0);

        let mut noise_tex = 0;
        (gl.GenTextures)(1, &mut noise_tex);
        let mut field_tex = [0u32; 2];
        (gl.GenTextures)(2, field_tex.as_mut_ptr());

        let (aw, ah, apx) = build_glade_sprite_atlas();
        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        (gl.ActiveTexture)(GL_TEXTURE6);
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            aw as GLsizei,
            ah as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            apx.as_ptr() as *const c_void,
        );
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        let u_noise = gl.uniform_loc(ground_prog, "uNoise");
        let u_fa = gl.uniform_loc(ground_prog, "uFA");
        let u_fb = gl.uniform_loc(ground_prog, "uFB");
        let u_res = gl.uniform_loc(ground_prog, "uRes");
        let u_grid = gl.uniform_loc(ground_prog, "uGrid");
        let u_scale = gl.uniform_loc(ground_prog, "uScale");
        let u_seed = gl.uniform_loc(ground_prog, "uSeed");
        let u_step = gl.uniform_loc(ground_prog, "uStep");
        let u_mix = gl.uniform_loc(ground_prog, "uMix");
        let u_debug = gl.uniform_loc(ground_prog, "uDebug");

        let ui_atlas = gl.uniform_loc(inst_prog, "uAtlas");
        let ui_res = gl.uniform_loc(inst_prog, "uRes");
        let ui_time = gl.uniform_loc(inst_prog, "uTime");
        let ui_wind = gl.uniform_loc(inst_prog, "uWind");

        Ok(Self {
            ground_prog,
            inst_prog,
            bg_vao,
            inst_vao,
            inst_buf_a,
            inst_buf_b,
            max_inst: 0,
            noise_tex,
            field_tex,
            atlas_tex,
            field_gw: 0,
            field_gh: 0,
            uploaded_field_version: u64::MAX,
            uploaded_noise_version: u64::MAX,
            u_noise,
            u_fa,
            u_fb,
            u_res,
            u_grid,
            u_scale,
            u_seed,
            u_step,
            u_mix,
            u_debug,
            ui_atlas,
            ui_res,
            ui_time,
            ui_wind,
            buf_a: Vec::new(),
            buf_b: Vec::new(),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.ground_prog != 0 {
            (gl.DeleteProgram)(self.ground_prog);
        }
        if self.inst_prog != 0 {
            (gl.DeleteProgram)(self.inst_prog);
        }
        if self.bg_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.bg_vao);
        }
        if self.inst_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.inst_vao);
        }
        let bufs = [self.inst_buf_a, self.inst_buf_b];
        (gl.DeleteBuffers)(2, bufs.as_ptr());
        let texs = [
            self.noise_tex,
            self.field_tex[0],
            self.field_tex[1],
            self.atlas_tex,
        ];
        (gl.DeleteTextures)(4, texs.as_ptr());
    }

    unsafe fn sync_noise_tex(&mut self, gl: &GlFns, gs: &GladeState) {
        if self.uploaded_noise_version == gs.noise_version {
            return;
        }
        let (s, d) = make_noise_image(gs.params.seed);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            s as GLsizei,
            s as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            d.as_ptr() as *const c_void,
        );
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);
        self.uploaded_noise_version = gs.noise_version;
    }

    unsafe fn sync_field_textures(&mut self, gl: &GlFns, gs: &GladeState) {
        if self.uploaded_field_version == gs.field_version
            && self.field_gw == gs.gw
            && self.field_gh == gs.gh
        {
            return;
        }
        let realloc = self.field_gw != gs.gw || self.field_gh != gs.gh;
        for (i, data) in [&gs.field_a, &gs.field_b].iter().enumerate() {
            (gl.ActiveTexture)(if i == 0 { GL_TEXTURE1 } else { GL_TEXTURE2 });
            (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[i]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            if realloc {
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_R32F as GLint,
                    gs.gw as GLsizei,
                    gs.gh as GLsizei,
                    0,
                    GL_RED,
                    GL_FLOAT,
                    data.as_ptr() as *const c_void,
                );
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            } else {
                (gl.TexSubImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    0,
                    0,
                    gs.gw as GLsizei,
                    gs.gh as GLsizei,
                    GL_RED,
                    GL_FLOAT,
                    data.as_ptr() as *const c_void,
                );
            }
        }
        self.field_gw = gs.gw;
        self.field_gh = gs.gh;
        self.uploaded_field_version = gs.field_version;
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.5);

        st.glade.resize(css_w, css_h);
        let gs = &st.glade;
        self.sync_noise_tex(gl, gs);
        self.sync_field_textures(gl, gs);

        let scale = dpi;
        let res_x = pw as f32 / dpi;
        let res_y = ph as f32 / dpi;

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_SCISSOR_TEST);

        // Pass 1: Sunlit watercolor clearing & mossy forest floor
        (gl.UseProgram)(self.ground_prog);
        (gl.BindVertexArray)(self.bg_vao);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[0]);
        (gl.ActiveTexture)(GL_TEXTURE2);
        (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[1]);

        (gl.Uniform1i)(self.u_noise, 0);
        (gl.Uniform1i)(self.u_fa, 1);
        (gl.Uniform1i)(self.u_fb, 2);
        (gl.Uniform2f)(self.u_res, res_x, res_y);
        (gl.Uniform2f)(self.u_grid, gs.gw as f32, gs.gh as f32);
        (gl.Uniform1f)(self.u_scale, scale);
        (gl.Uniform1f)(self.u_seed, (gs.params.seed.rem_euclid(997) as f32) * 0.37);
        (gl.Uniform1f)(self.u_step, STEP as f32);
        (gl.Uniform1f)(self.u_mix, gs.cur_mix as f32);
        (gl.Uniform1f)(self.u_debug, if gs.params.debug { 1.0 } else { 0.0 });
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Pass 2: Single instanced draw call for all trees & woodland birds
        self.buf_a.clear();
        self.buf_b.clear();
        let mut count = 0usize;

        for t in &gs.trees {
            if t.grow <= 0.015 {
                continue;
            }
            let e = t.grow * t.grow * (3.0 - 2.0 * t.grow);
            self.buf_a.push(t.x as f32);
            self.buf_a.push(t.y as f32);
            self.buf_a.push((t.scale * e) as f32);
            self.buf_a.push(t.variant as f32);
            self.buf_b.push(t.mirror as f32);
            self.buf_b.push(t.phase as f32);
            self.buf_b.push(1.0);
            self.buf_b.push((e * 1.15).min(1.0) as f32);
            count += 1;
        }

        let time_sec = gs.time_sec;
        let wind = gs.params.wind;
        for b in &gs.birds {
            let perch = b.tree_idx.and_then(|idx| gs.trees.get(idx));
            if b.fly.is_none() && perch.map_or(true, |t| t.grow < 0.6) {
                continue;
            }
            let mut bx = b.x;
            let by = b.y;
            if b.fly.is_none() {
                if let Some(t) = perch {
                    let gust = (time_sec * (0.95 + 0.55 * wind)
                        + t.phase
                        + t.x * 0.004
                        + t.y * 0.002)
                        .sin();
                    bx += gust * 4.5 * (0.25 + 0.75 * wind.min(2.5)) * t.scale;
                }
            }
            self.buf_a.push(bx as f32);
            self.buf_a.push((by + 55.0) as f32);
            self.buf_a.push(0.88);
            self.buf_a.push((GLADE_TREE_VARIANTS + b.variant) as f32);
            self.buf_b.push(b.mirror as f32);
            self.buf_b.push(0.0);
            self.buf_b.push(0.0);
            self.buf_b.push(0.96);
            count += 1;
        }

        if count > 0 {
            (gl.UseProgram)(self.inst_prog);
            (gl.BindVertexArray)(self.inst_vao);
            (gl.Enable)(GL_BLEND);
            // Keep framebuffer alpha = 1.0 (avoid semi-transparent DWM / ReadPixels artifacts)
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);

            (gl.ActiveTexture)(GL_TEXTURE6);
            (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
            (gl.Uniform1i)(self.ui_atlas, 6);
            (gl.Uniform2f)(self.ui_res, res_x, res_y);
            (gl.Uniform1f)(self.ui_time, time_sec as f32);
            (gl.Uniform1f)(self.ui_wind, wind as f32);

            let bytes = (count * 4 * std::mem::size_of::<f32>()) as GLsizeiptr;
            if count > self.max_inst {
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_a);
                (gl.BufferData)(
                    GL_ARRAY_BUFFER,
                    bytes,
                    self.buf_a.as_ptr() as *const c_void,
                    GL_DYNAMIC_DRAW,
                );
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_b);
                (gl.BufferData)(
                    GL_ARRAY_BUFFER,
                    bytes,
                    self.buf_b.as_ptr() as *const c_void,
                    GL_DYNAMIC_DRAW,
                );
                self.max_inst = count;
            } else {
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_a);
                (gl.BufferSubData)(GL_ARRAY_BUFFER, 0, bytes, self.buf_a.as_ptr() as *const c_void);
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_b);
                (gl.BufferSubData)(GL_ARRAY_BUFFER, 0, bytes, self.buf_b.as_ptr() as *const c_void);
            }

            (gl.DrawArraysInstanced)(GL_TRIANGLE_STRIP, 0, 4, count as GLsizei);
            (gl.Disable)(GL_BLEND);
            (gl.BindVertexArray)(0);
        }
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
        eat(&mut h, rel_path.as_bytes());
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
            Rect { x: 52.0, y: 44.0, w: 76.0, h: 90.0 },
            Rect { x: 52.0, y: 154.0, w: 76.0, h: 90.0 },
            Rect { x: 52.0, y: 264.0, w: 76.0, h: 90.0 },
            Rect { x: 168.0, y: 44.0, w: 76.0, h: 90.0 },
            Rect { x: 168.0, y: 154.0, w: 76.0, h: 90.0 },
            Rect { x: 480.0, y: 260.0, w: 76.0, h: 90.0 },
        ]
    }

    #[test]
    fn verify_rust_anything_recorded_glade_cases() {
        let root = repo_root();
        let rec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/glade/recorded-from.json");
        let rec: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&rec_path).unwrap()).unwrap();
        for src in rec["sources"].as_array().unwrap() {
            let rel = src["path"].as_str().unwrap();
            let expected = src["fnv1a"].as_str().unwrap();
            let actual = fnv1a_fingerprint(rel, &std::fs::read(root.join(rel)).unwrap());
            assert_eq!(actual, expected, "Source fingerprint mismatch for {rel}");
        }

        let cases_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/glade/cases.jsonl");
        let lines: Vec<serde_json::Value> = std::fs::read_to_string(&cases_path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 3);

        // Case 1: computeFieldGlade
        {
            let c = &lines[0];
            assert_eq!(c["fn"], "computeFieldGlade");
            let mut gs = GladeState::new(1280.0, 720.0);
            gs.set_icons_raw(&sample_icons(), false);
            let probes = c["input"]["probes"].as_array().unwrap();
            let expected = c["output"]["sampled"].as_array().unwrap();
            for (k, (p, e)) in probes.iter().zip(expected.iter()).enumerate() {
                let x = p[0].as_f64().unwrap();
                let y = p[1].as_f64().unwrap();
                let got = gs.land_s(x, y);
                assert_close(got, e.as_f64().unwrap(), 1e-4, &format!("field probe {k}"));
            }
        }

        // Case 2: gladeForestBuild across 3 configurations
        {
            let c = &lines[1];
            assert_eq!(c["fn"], "gladeForestBuild");
            let configs = [
                ("s42_d1", 42, 1.0, true),
                ("s1337_d06", 1337, 0.6, true),
                ("s42_noadapt", 42, 1.0, false),
            ];
            for (key, seed, density, adaptive) in configs {
                let exp = &c["output"][key];
                let exp_rands = exp["rands"].as_array().unwrap();
                assert_close(glade_rand(seed, 0, 0, 1), exp_rands[0].as_f64().unwrap(), 1e-12, "rand0");
                assert_close(glade_rand(seed, 7, 11, 3), exp_rands[1].as_f64().unwrap(), 1e-12, "rand1");
                assert_close(glade_rand(seed, 23, 5, 5), exp_rands[2].as_f64().unwrap(), 1e-12, "rand2");

                let mut gs = GladeState::new(1280.0, 720.0);
                let mut p = gs.params;
                p.seed = seed;
                p.density = density;
                p.adaptive = adaptive;
                gs.sync_params(p);
                gs.set_icons_raw(&sample_icons(), false);

                let total = gs.trees.len();
                let grown = gs.trees.iter().filter(|t| t.want > 0.5).count();
                let cleared = total - grown;
                assert_eq!(total, exp["total"].as_u64().unwrap() as usize, "{key} total");
                assert_eq!(grown, exp["grown"].as_u64().unwrap() as usize, "{key} grown");
                assert_eq!(cleared, exp["cleared"].as_u64().unwrap() as usize, "{key} cleared");

                for s in exp["samples"].as_array().unwrap() {
                    let idx = s["i"].as_u64().unwrap() as usize;
                    let t = &gs.trees[idx];
                    assert_close(t.x, s["x"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].x"));
                    assert_close(t.y, s["y"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].y"));
                    assert_eq!(t.variant, s["variant"].as_u64().unwrap() as usize, "{key}[{idx}].variant");
                    assert_close(t.scale, s["scale"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].scale"));
                    assert_close(t.mirror, s["mirror"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].mirror"));
                    assert_close(t.phase, s["phase"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].phase"));
                    assert_close(t.want, s["want"].as_f64().unwrap(), 1e-9, &format!("{key}[{idx}].want"));
                }
            }
        }

        // Case 3: gladeStepGrowthAndBirdArc
        {
            let c = &lines[2];
            assert_eq!(c["fn"], "gladeStepGrowthAndBirdArc");
            let dt = c["input"]["dt"].as_f64().unwrap();
            let t_shrink_idx = c["input"]["tShrinkIdx"].as_u64().unwrap() as usize;
            let t_grow_idx = c["input"]["tGrowIdx"].as_u64().unwrap() as usize;

            let mut gs = GladeState::new(1280.0, 720.0);
            gs.set_icons_raw(&sample_icons(), false);
            gs.trees[t_shrink_idx].grow = 1.0;
            gs.trees[t_shrink_idx].want = 0.0;
            gs.trees[t_grow_idx].grow = 0.0;
            gs.trees[t_grow_idx].want = 1.0;
            gs.birds = vec![GladeBird {
                tree_idx: Some(0),
                x: 200.0,
                y: 300.0,
                variant: 1,
                mirror: 1.0,
                wait: 5.0,
                fly: Some(BirdFlight {
                    sx: 200.0,
                    sy: 300.0,
                    tx: 420.0,
                    ty: 260.0,
                    to_idx: t_grow_idx,
                    t: 0.0,
                    dur: 1.2,
                }),
            }];

            for (step_i, tr) in c["output"]["trace"].as_array().unwrap().iter().enumerate() {
                gs.step_forest(dt);
                assert_close(
                    gs.trees[t_shrink_idx].grow,
                    tr["shrinkGrow"].as_f64().unwrap(),
                    1e-9,
                    &format!("step {step_i} shrinkGrow"),
                );
                assert_close(
                    gs.trees[t_grow_idx].grow,
                    tr["growGrow"].as_f64().unwrap(),
                    1e-9,
                    &format!("step {step_i} growGrow"),
                );
                assert_close(gs.birds[0].x, tr["bx"].as_f64().unwrap(), 1e-9, &format!("step {step_i} bx"));
                assert_close(gs.birds[0].y, tr["by"].as_f64().unwrap(), 1e-9, &format!("step {step_i} by"));
                assert_eq!(gs.birds[0].fly.is_some(), tr["flying"].as_bool().unwrap());
            }
        }
    }
}
