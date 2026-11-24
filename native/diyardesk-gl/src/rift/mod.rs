pub mod shaders;

use crate::beach::sample_arr;
use crate::gl::*;
use crate::meadow::{fbm, make_noise_image, IconBox, MORPH_EMPTY_SEC, MORPH_SEC, STEP};
use crate::state::{AppState, Rect};
use shaders::*;
use std::f64::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// Constants & Deterministic Hashes (1:1 prototype/rift.html + rift-props.js)
// =========================================================================
pub const RIFT_COLS: usize = 8;
pub const RIFT_ROWS: usize = 3;
pub const RIFT_CELL: usize = 128;

#[inline]
pub fn rift_hash(seed: i32, a: i32, b: i32, k: i32) -> f64 {
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
pub fn rift_river_center_norm_y(seed: i32, u: f64) -> f64 {
    let s_off = (seed % 997) as f64 * 0.37;
    0.56 - 0.18 * (u - 0.5)
        + 0.095 * (u * 4.6 + s_off * 0.7).sin()
        + 0.035 * (u * 9.2 - s_off).cos()
}

#[inline]
pub fn rift_river_dist_px(seed: i32, w: f64, h: f64, x: f64, y: f64) -> f64 {
    let u = x / w.max(1.0);
    let v = y / h.max(1.0);
    (v - rift_river_center_norm_y(seed, u)).abs() * h
}

// =========================================================================
// Parameters & Entities
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct RiftParams {
    pub sigma: f64,
    pub warp: f64,
    pub glow: f64,
    pub wind: f64,
    pub heroes: usize,
    pub turrets: usize,
    pub wards: usize,
    pub wisps: usize,
    pub adaptive: bool,
    pub seed: i32,
    pub debug: bool,
}

impl Default for RiftParams {
    fn default() -> Self {
        Self {
            sigma: 82.0,
            warp: 24.0,
            glow: 0.78,
            wind: 1.0,
            heroes: 5,
            turrets: 4,
            wards: 4,
            wisps: 8,
            adaptive: true,
            seed: 42,
            debug: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct RiftTree {
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
pub struct RiftTurret {
    pub x: f64,
    pub y: f64,
    pub team: usize,
    pub phase: f64,
    pub grow: f64,
}

#[derive(Clone, Copy)]
pub struct RiftWard {
    pub x: f64,
    pub y: f64,
    pub variant: usize,
    pub phase: f64,
    pub grow: f64,
}

#[derive(Clone, Copy)]
pub struct RiftWisp {
    pub bx: f64,
    pub by: f64,
    pub x: f64,
    pub y: f64,
    pub rx: f64,
    pub ry: f64,
    pub spd: f64,
    pub phase: f64,
    pub variant: usize,
    pub size: f64,
}

#[derive(Clone, Copy)]
pub struct RiftHero {
    pub idx: usize,
    pub col: usize,
    pub is_poro: bool,
    pub x: f64,
    pub y: f64,
    pub tx: f64,
    pub ty: f64,
    pub flip: f64,
    pub wait: f64,
    pub step_count: i32,
    pub walk_t: f64,
    pub phase: f64,
}

// =========================================================================
// RiftState (CPU clearing SDF + jungle trees, turrets, wards, wisps, heroes)
// =========================================================================
pub struct RiftState {
    pub params: RiftParams,
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
    pub trees: Vec<RiftTree>,
    pub turrets: Vec<RiftTurret>,
    pub wards: Vec<RiftWard>,
    pub wisps: Vec<RiftWisp>,
    pub heroes: Vec<RiftHero>,
}

impl Default for RiftState {
    fn default() -> Self {
        Self::new(1280.0, 720.0)
    }
}

impl RiftState {
    pub fn new(w: f64, h: f64) -> Self {
        let mut s = Self {
            params: RiftParams::default(),
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
            turrets: Vec::new(),
            wards: Vec::new(),
            wisps: Vec::new(),
            heroes: Vec::new(),
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
                self.nz_wx[i] = (fbm(x * 0.0032, y * 0.0032, 1, seed) - 0.47) as f32;
                self.nz_wy[i] = (fbm(x * 0.0032, y * 0.0032, 2, seed) - 0.47) as f32;
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
        let wa = self.params.warp * 2.1;
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

    fn pick_hero_target(&self, idx: i32, step: i32) -> (f64, f64) {
        let seed = self.params.seed;
        if !self.icons.is_empty() && rift_hash(seed, idx, step, 41) < 0.78 {
            let pick = ((rift_hash(seed, idx, step, 42) * self.icons.len() as f64).floor() as usize)
                % self.icons.len();
            let ic = self.icons[pick];
            let ang = rift_hash(seed, idx, step, 43) * TAU;
            let rad = 42.0 + rift_hash(seed, idx, step, 44) * 24.0;
            let px = (ic.x as f64 + ic.w as f64 * 0.5 + ang.cos() * rad)
                .clamp(40.0, (self.w - 40.0).max(40.0));
            let py = (ic.y as f64 + ic.h as f64 * 0.55 + ang.sin() * rad)
                .clamp(56.0, (self.h - 56.0).max(56.0));
            if rift_river_dist_px(seed, self.w, self.h, px, py) > 36.0 {
                return (px, py);
            }
        }
        let rx = self.w * (0.12 + 0.76 * rift_hash(seed, idx, step, 45));
        let u = rx / self.w.max(1.0);
        let s_off = (seed % 997) as f64 * 0.37;
        let river_y = self.h * (0.56 - 0.18 * (u - 0.5) + 0.095 * (u * 4.6 + s_off * 0.7).sin());
        let side = if rift_hash(seed, idx, step, 46) < 0.5 { -1.0 } else { 1.0 };
        let ry = (river_y + side * (56.0 + rift_hash(seed, idx, step, 47) * 32.0))
            .clamp(60.0, (self.h - 62.0).max(60.0));
        (rx, ry)
    }

    pub fn rebuild_props(&mut self) {
        self.trees.clear();
        let step_x = 56i32;
        let step_y = 46i32;
        let cols = ((self.w + step_x as f64) / step_x as f64).ceil() as i32 + 1;
        let rows = ((self.h + step_y as f64) / step_y as f64).ceil() as i32 + 1;
        let seed = self.params.seed;

        for gy in 0..rows {
            for gx in 0..cols {
                let r1 = rift_hash(seed, gx, gy, 1);
                let r2 = rift_hash(seed, gx, gy, 2);
                let r3 = rift_hash(seed, gx, gy, 3);
                let r4 = rift_hash(seed, gx, gy, 4);
                let r5 = rift_hash(seed, gx, gy, 5);
                if r5 < 0.14 {
                    continue;
                }
                let offset_x = (gy % 2) as f64 * (step_x as f64 * 0.5);
                let x = gx as f64 * step_x as f64 - 20.0 + offset_x + (r1 - 0.5) * step_x as f64 * 0.68;
                let y = gy as f64 * step_y as f64 - 8.0 + (r2 - 0.5) * step_y as f64 * 0.68;
                if x < -28.0 || x > self.w + 28.0 || y < 14.0 || y > self.h + 52.0 {
                    continue;
                }
                if rift_river_dist_px(seed, self.w, self.h, x, y) < 38.0 {
                    continue;
                }
                self.trees.push(RiftTree {
                    x,
                    y,
                    variant: ((r3 * 6.0).floor() as usize).min(5),
                    scale: 0.76 + r4 * 0.44,
                    mirror: if r1 < 0.5 { -1.0 } else { 1.0 },
                    phase: r2 * TAU,
                    grow: 0.0,
                    want: 1.0,
                });
            }
        }
        self.trees.sort_by(|a, b| a.y.total_cmp(&b.y));
        self.sync_wards_and_wisps(true);
        self.update_targets(true);
    }

    pub fn update_targets(&mut self, snap_now: bool) {
        let adaptive = self.params.adaptive;
        for i in 0..self.trees.len() {
            let tx = self.trees[i].x;
            let ty = self.trees[i].y;
            let d = self.land_s(tx, ty);
            let clear_by_turret = self
                .turrets
                .iter()
                .any(|tr| (tx - tr.x).hypot(ty - tr.y) < 38.0);
            let want = if !clear_by_turret && (!adaptive || d > 12.0) {
                1.0
            } else {
                0.0
            };
            self.trees[i].want = want;
            if snap_now {
                self.trees[i].grow = want;
            }
        }
    }

    pub fn sync_wards_and_wisps(&mut self, reset: bool) {
        let seed = self.params.seed;
        let w = self.w;
        let h = self.h;

        // Defense Turrets: Blue Order (top-left of river) & Crimson Chaos (bottom-right)
        let want_turrets = self.params.turrets.min(6);
        self.turrets.clear();
        if want_turrets > 0 {
            #[derive(Clone, Copy)]
            struct TCand {
                x: f64,
                y: f64,
                team: usize,
                score: f64,
                idx: i32,
            }
            let mut cands: [Vec<TCand>; 2] = [Vec::new(), Vec::new()];
            let tstep = 56;
            let mut y = 76;
            while (y as f64) < h - 72.0 {
                let mut x = 68;
                while (x as f64) < w - 68.0 {
                    let xf = x as f64;
                    let yf = y as f64;
                    let s = self.land_s(xf, yf);
                    if s >= 2.0 && s <= 46.0 && rift_river_dist_px(seed, w, h, xf, yf) > 64.0 {
                        let team = if (yf / h.max(1.0)) < rift_river_center_norm_y(seed, xf / w.max(1.0)) {
                            0
                        } else {
                            1
                        };
                        let score = (s - 20.0).abs() + rift_hash(seed, x, y, 61) * 7.0;
                        cands[team].push(TCand {
                            x: xf,
                            y: yf,
                            team,
                            score,
                            idx: x * 11 + y * 17,
                        });
                    }
                    x += tstep;
                }
                y += tstep;
            }
            cands[0].sort_by(|a, b| a.score.total_cmp(&b.score));
            cands[1].sort_by(|a, b| a.score.total_cmp(&b.score));

            let mut chosen: Vec<TCand> = Vec::with_capacity(want_turrets);
            for k in 0..want_turrets {
                let team = k % 2;
                let mut hit = cands[team]
                    .iter()
                    .copied()
                    .find(|c| chosen.iter().all(|p| (p.x - c.x).hypot(p.y - c.y) > 185.0));
                if hit.is_none() {
                    hit = cands[1 - team]
                        .iter()
                        .copied()
                        .find(|c| chosen.iter().all(|p| (p.x - c.x).hypot(p.y - c.y) > 185.0));
                }
                let c = hit.unwrap_or_else(|| {
                    let frac_x = if team == 0 {
                        0.18 + 0.24 * (k as f64 * 0.5)
                    } else {
                        0.58 + 0.24 * (k as f64 * 0.5)
                    };
                    let rx = w * frac_x.clamp(0.12, 0.88);
                    let river_y = h * rift_river_center_norm_y(seed, rx / w.max(1.0));
                    let ry = (river_y + if team == 0 { -105.0 } else { 105.0 })
                        .clamp(84.0, (h - 74.0).max(84.0));
                    TCand {
                        x: rx,
                        y: ry,
                        team,
                        score: 0.0,
                        idx: (k as i32) * 23,
                    }
                });
                chosen.push(c);
            }
            for c in chosen {
                self.turrets.push(RiftTurret {
                    x: c.x,
                    y: c.y,
                    team: c.team,
                    phase: (c.idx % 63) as f64 * 0.1,
                    grow: if reset { 1.0 } else { 0.2 },
                });
            }
        }
        self.update_targets(reset);

        // Hextech Ward Lanterns
        let want_wards = self.params.wards.min(8);
        self.wards.clear();
        if want_wards > 0 {
            #[derive(Clone, Copy)]
            struct WCand {
                x: f64,
                y: f64,
                score: f64,
                idx: i32,
            }
            let mut candidates: Vec<WCand> = Vec::new();
            let step = 44;
            let mut y = 52;
            while (y as f64) < h - 56.0 {
                let mut x = 52;
                while (x as f64) < w - 48.0 {
                    let xf = x as f64;
                    let yf = y as f64;
                    let s = self.land_s(xf, yf);
                    if s >= -2.0 && s <= 32.0 && rift_river_dist_px(seed, w, h, xf, yf) > 54.0 {
                        let score = (s - 12.0).abs() + rift_hash(seed, x, y, 17) * 6.0;
                        candidates.push(WCand {
                            x: xf,
                            y: yf,
                            score,
                            idx: x * 7 + y * 13,
                        });
                    }
                    x += step;
                }
                y += step;
            }
            candidates.sort_by(|a, b| a.score.total_cmp(&b.score));

            let mut picked: Vec<WCand> = Vec::with_capacity(want_wards);
            for c in candidates {
                if picked.iter().all(|p| (p.x - c.x).hypot(p.y - c.y) > 155.0)
                    && self
                        .turrets
                        .iter()
                        .all(|tr| (tr.x - c.x).hypot(tr.y - c.y) > 95.0)
                {
                    picked.push(c);
                    if picked.len() >= want_wards {
                        break;
                    }
                }
            }
            if picked.len() < want_wards {
                let start_i = picked.len();
                for i in start_i..want_wards {
                    let rx = w * (0.14 + 0.72 * rift_hash(seed, i as i32, 91, 3));
                    let ry = h * (0.18 + 0.64 * rift_hash(seed, i as i32, 92, 4));
                    if picked.iter().all(|p| (p.x - rx).hypot(p.y - ry) > 120.0) {
                        picked.push(WCand {
                            x: rx,
                            y: ry,
                            score: 0.0,
                            idx: (i as i32) * 19,
                        });
                    }
                }
            }
            for (i, c) in picked.into_iter().enumerate() {
                self.wards.push(RiftWard {
                    x: c.x,
                    y: c.y,
                    variant: i % 2,
                    phase: (c.idx % 63) as f64 * 0.1,
                    grow: if reset { 1.0 } else { 0.25 },
                });
            }
        }

        // Arcane Mana Wisps
        let want_wisps = self.params.wisps.min(16);
        if reset || self.wisps.len() > want_wisps {
            let keep = self.wisps.len().min(want_wisps);
            self.wisps.truncate(keep);
        }
        while self.wisps.len() < want_wisps {
            let i = self.wisps.len();
            let ii = i as i32;
            let on_river = i % 2 == 0;
            let bx = w * (0.08 + 0.84 * rift_hash(seed, ii, 12, 8));
            let mut by = h * (0.10 + 0.78 * rift_hash(seed, ii, 14, 9));
            if on_river {
                let u = bx / w.max(1.0);
                let s_off = (seed % 997) as f64 * 0.37;
                by = h * (0.56 - 0.18 * (u - 0.5) + 0.095 * (u * 4.6 + s_off * 0.7).sin())
                    + (rift_hash(seed, ii, 19, 2) - 0.5) * 44.0;
            }
            self.wisps.push(RiftWisp {
                bx,
                by,
                x: bx,
                y: by,
                rx: 18.0 + 26.0 * rift_hash(seed, ii, 21, 5),
                ry: 10.0 + 18.0 * rift_hash(seed, ii, 22, 6),
                spd: 0.45 + 0.55 * rift_hash(seed, ii, 23, 7),
                phase: rift_hash(seed, ii, 24, 1) * TAU,
                variant: i % 3,
                size: 24.0 + 14.0 * rift_hash(seed, ii, 25, 3),
            });
        }

        // Roaming Rift Champions + Fluffy Poro
        const HERO_COLS: [usize; 5] = [4, 3, 7, 5, 6];
        let want_heroes = self.params.heroes.min(5);
        if reset || self.heroes.len() > want_heroes {
            let keep = self.heroes.len().min(want_heroes);
            self.heroes.truncate(keep);
        }
        while self.heroes.len() < want_heroes {
            let idx = self.heroes.len();
            let ii = idx as i32;
            let col = HERO_COLS[idx % HERO_COLS.len()];
            let start = self.pick_hero_target(ii, ii * 17);
            let dest = self.pick_hero_target(ii, ii * 17 + 1);
            self.heroes.push(RiftHero {
                idx,
                col,
                is_poro: col == 7,
                x: start.0,
                y: start.1,
                tx: dest.0,
                ty: dest.1,
                flip: if dest.0 >= start.0 { 1.0 } else { -1.0 },
                wait: 0.8 + rift_hash(seed, ii, 31, 4) * 2.4,
                step_count: ii * 17 + 2,
                walk_t: rift_hash(seed, ii, 32, 9) * 10.0,
                phase: rift_hash(seed, ii, 33, 2) * TAU,
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
            self.rebuild_props();
        }
        self.update_targets(false);
        self.sync_wards_and_wisps(false);
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
        self.sync_wards_and_wisps(false);
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
        self.rebuild_props();
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

    pub fn sync_params(&mut self, next: RiftParams) {
        let prev = self.params;
        if prev == next {
            return;
        }
        self.params = next;
        let seed_changed = prev.seed != next.seed;
        let adaptive_changed = prev.adaptive != next.adaptive;
        let field_changed =
            (prev.sigma - next.sigma).abs() > 1e-6 || (prev.warp - next.warp).abs() > 1e-6;
        let props_changed = prev.heroes != next.heroes
            || prev.turrets != next.turrets
            || prev.wards != next.wards
            || prev.wisps != next.wisps;

        if seed_changed {
            self.build_noise();
            self.noise_version = self.noise_version.wrapping_add(1);
            self.refresh();
            self.rebuild_props();
        } else {
            if adaptive_changed {
                self.commit(false);
            } else if field_changed {
                self.refresh();
            }
            if props_changed {
                self.sync_wards_and_wisps(false);
            }
        }
    }

    pub fn step_props(&mut self, dt: f64, time_sec: f64) {
        let rate = dt * 2.8;
        for t in &mut self.trees {
            if t.grow < t.want {
                t.grow = (t.grow + rate).min(t.want);
            } else if t.grow > t.want {
                t.grow = (t.grow - rate * 1.45).max(t.want);
            }
        }
        for w in &mut self.wards {
            if w.grow < 1.0 {
                w.grow = (w.grow + rate).min(1.0);
            }
        }
        for tr in &mut self.turrets {
            if tr.grow < 1.0 {
                tr.grow = (tr.grow + rate).min(1.0);
            }
        }
        let drift_t = time_sec * (0.3 + self.params.wind * 0.7);
        for sp in &mut self.wisps {
            sp.x = sp.bx + (drift_t * sp.spd + sp.phase).sin() * sp.rx;
            sp.y = sp.by + (drift_t * sp.spd * 0.82 + sp.phase * 1.3).cos() * sp.ry;
        }

        let move_spd_base = 26.0 * (0.5 + 0.5 * self.params.wind);
        let seed = self.params.seed;
        for i in 0..self.heroes.len() {
            if self.heroes[i].wait > 0.0 {
                self.heroes[i].wait -= dt;
                self.heroes[i].walk_t = 0.0;
                if self.heroes[i].wait <= 0.0 {
                    let sc = self.heroes[i].step_count;
                    self.heroes[i].step_count += 1;
                    let idx = self.heroes[i].idx as i32;
                    let next = self.pick_hero_target(idx, sc);
                    self.heroes[i].tx = next.0;
                    self.heroes[i].ty = next.1;
                    if (self.heroes[i].tx - self.heroes[i].x).abs() > 2.0 {
                        self.heroes[i].flip = if self.heroes[i].tx >= self.heroes[i].x {
                            1.0
                        } else {
                            -1.0
                        };
                    }
                }
            } else {
                let h = &mut self.heroes[i];
                let dx = h.tx - h.x;
                let dy = h.ty - h.y;
                let dist = dx.hypot(dy);
                let step = (if h.is_poro {
                    move_spd_base * 1.25
                } else {
                    move_spd_base
                }) * dt;
                if dist <= step + 1.0 {
                    h.x = h.tx;
                    h.y = h.ty;
                    h.wait = 2.0 + rift_hash(seed, h.idx as i32, h.step_count, 51) * 3.0;
                    h.walk_t = 0.0;
                } else {
                    h.x += (dx / dist) * step;
                    h.y += (dy / dist) * step;
                    h.walk_t += dt * if h.is_poro { 9.5 } else { 6.8 };
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
        let t = self.time_sec;
        self.step_props(dt, t);
    }
}

// =========================================================================
// Procedural 2x Painter Atlas (8x3 cells of 128x128 -> 2048x768 RGBA8)
// 1:1 port of prototype/rift-props.js
// =========================================================================
fn hex_rgb(hex: u32) -> [f64; 3] {
    [
        ((hex >> 16) & 0xff) as f64 / 255.0,
        ((hex >> 8) & 0xff) as f64 / 255.0,
        (hex & 0xff) as f64 / 255.0,
    ]
}

fn hex_rgba(hex: u32, a: f64) -> [f64; 4] {
    let c = hex_rgb(hex);
    [c[0], c[1], c[2], a]
}

fn eval_stops_rgba(stops: &[(f64, [f64; 4])], t: f64) -> [f64; 4] {
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
                w[0].1[3] + (w[1].1[3] - w[0].1[3]) * f,
            ];
        }
    }
    stops.last().map(|s| s.1).unwrap_or([0.0; 4])
}

#[inline]
fn rot_local(cx: f64, cy: f64, rot: f64, lx: f64, ly: f64) -> (f64, f64) {
    let c = rot.cos();
    let s = rot.sin();
    (cx + lx * c - ly * s, cy + lx * s + ly * c)
}

fn push_quad(out: &mut Vec<(f64, f64)>, p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), steps: usize) {
    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let u = 1.0 - t;
        out.push((
            u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0,
            u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1,
        ));
    }
}

struct RiftCanvas {
    w: usize,
    h: usize,
    scale: f64,
    px: Vec<u8>,
}

impl RiftCanvas {
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
        self.px[idx] = (((r * sa + dr * da * (1.0 - sa)) * inv).clamp(0.0, 1.0) * 255.0).round() as u8;
        self.px[idx + 1] = (((g * sa + dg * da * (1.0 - sa)) * inv).clamp(0.0, 1.0) * 255.0).round() as u8;
        self.px[idx + 2] = (((b * sa + db * da * (1.0 - sa)) * inv).clamp(0.0, 1.0) * 255.0).round() as u8;
        self.px[idx + 3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, rgba: [f64; 4]) {
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
                    self.blend_px(px, py, rgba[0], rgba[1], rgba[2], rgba[3] * cov);
                }
            }
        }
    }

    fn fill_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, rgba: [f64; 4]) {
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
                        self.blend_px(px, py, rgba[0], rgba[1], rgba[2], rgba[3] * cov);
                    }
                }
            }
        }
    }

    fn fill_ellipse_radial(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        gx0: f64,
        gy0: f64,
        r0: f64,
        gx1: f64,
        gy1: f64,
        r1: f64,
        stops: &[(f64, [f64; 4])],
    ) {
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
        let gdx = gx1 - gx0;
        let gdy = gy1 - gy0;
        let dr = r1 - r0;
        let qa = gdx * gdx + gdy * gdy - dr * dr;

        for py in iy0..iy1 {
            let ly_world = (py as f64 + 0.5) / s;
            for px in ix0..ix1 {
                let lx_world = (px as f64 + 0.5) / s;
                let dx = px as f64 + 0.5 - scx;
                let dy = py as f64 + 0.5 - scy;
                let nx = dx / srx;
                let ny = dy / sry;
                let d2 = nx * nx + ny * ny;
                if d2 < 1.35 {
                    let d = d2.sqrt();
                    let grad_len = (nx / srx).hypot(ny / sry).max(1e-6);
                    let signed_px = (d - 1.0) / grad_len;
                    let cov = (0.5 - signed_px).clamp(0.0, 1.0);
                    if cov > 0.001 {
                        let qpx = lx_world - gx0;
                        let qpy = ly_world - gy0;
                        let qb = -(qpx * gdx + qpy * gdy + r0 * dr);
                        let qc = qpx * qpx + qpy * qpy - r0 * r0;
                        let disc = (qb * qb - qa * qc).max(0.0);
                        let t = if qa.abs() > 1e-9 {
                            (-qb - disc.sqrt()) / qa
                        } else if qb.abs() > 1e-9 {
                            -qc / (2.0 * qb)
                        } else {
                            0.0
                        };
                        let col = eval_stops_rgba(stops, t);
                        self.blend_px(px, py, col[0], col[1], col[2], col[3] * cov);
                    }
                }
            }
        }
    }

    fn stroke_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, line_w: f64, rgba: [f64; 4]) {
        let steps = 40usize;
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = (i as f64 / steps as f64) * TAU;
            let (px, py) = rot_local(cx, cy, rot, a.cos() * rx, a.sin() * ry);
            pts.push((px, py));
        }
        self.stroke_polyline(&pts, line_w, rgba);
    }

    fn fill_polygon(&mut self, pts: &[(f64, f64)], rgba: [f64; 4]) {
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
            for (k, &hits) in row_cov.iter().enumerate() {
                if hits == 0 {
                    continue;
                }
                let cov = hits as f64 / (SUB * SUB) as f64;
                self.blend_px(ix0 + k, py, rgba[0], rgba[1], rgba[2], rgba[3] * cov);
            }
        }
    }

    fn stroke_polyline(&mut self, pts: &[(f64, f64)], line_w: f64, rgba: [f64; 4]) {
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
                    self.blend_px(px, py, rgba[0], rgba[1], rgba[2], rgba[3] * cov);
                }
            }
        }
    }

    fn stroke_polygon_closed(&mut self, pts: &[(f64, f64)], line_w: f64, rgba: [f64; 4]) {
        if pts.len() < 2 {
            return;
        }
        let mut closed = Vec::with_capacity(pts.len() + 1);
        closed.extend_from_slice(pts);
        closed.push(pts[0]);
        self.stroke_polyline(&closed, line_w, rgba);
    }

    fn stroke_arc(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64, line_w: f64, rgba: [f64; 4]) {
        let steps = ((r * (a1 - a0).abs()).ceil() as usize).clamp(10, 48);
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = a0 + (a1 - a0) * (i as f64 / steps as f64);
            pts.push((cx + a.cos() * r, cy + a.sin() * r));
        }
        self.stroke_polyline(&pts, line_w, rgba);
    }
}

fn draw_leaf_cluster(
    cv: &mut RiftCanvas,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    base_col: u32,
    mid_col: u32,
    hi_col: u32,
    rot: f64,
) {
    let p_base = rot_local(cx, cy, rot, 0.0, 3.0);
    cv.fill_ellipse(p_base.0, p_base.1, rx, ry, rot, hex_rgba(base_col, 1.0));

    let p_mid = rot_local(cx, cy, rot, -rx * 0.06, -1.0);
    cv.fill_ellipse(p_mid.0, p_mid.1, rx * 0.9, ry * 0.86, rot, hex_rgba(mid_col, 1.0));

    let p_hi = rot_local(cx, cy, rot, -rx * 0.18, -ry * 0.24);
    cv.fill_ellipse(
        p_hi.0,
        p_hi.1,
        rx * 0.56,
        ry * 0.48,
        rot - 0.25,
        hex_rgba(hi_col, 1.0),
    );

    let p_rim = rot_local(cx, cy, rot, 0.0, 1.5);
    cv.stroke_ellipse(
        p_rim.0,
        p_rim.1,
        rx * 0.96,
        ry * 0.94,
        rot,
        1.8,
        [13.0 / 255.0, 28.0 / 255.0, 22.0 / 255.0, 0.36],
    );
}

fn paint_tree_cell(cv: &mut RiftCanvas, ox: f64, oy: f64, variant: usize) {
    let cx = ox + 64.0;
    let by = oy + 108.0;

    // Ground AO shadow
    cv.fill_ellipse(
        cx + 5.0,
        by + 3.0,
        30.0,
        12.0,
        0.0,
        [10.0 / 255.0, 22.0 / 255.0, 18.0 / 255.0, 0.42],
    );

    // Gnarled root/trunk silhouette
    let mut trunk = Vec::with_capacity(24);
    trunk.push((cx - 8.0, by + 2.0));
    push_quad(&mut trunk, (cx - 8.0, by + 2.0), (cx - 5.0, by - 16.0), (cx - 4.0, by - 28.0), 8);
    trunk.push((cx + 5.0, by - 28.0));
    push_quad(&mut trunk, (cx + 5.0, by - 28.0), (cx + 6.0, by - 14.0), (cx + 10.0, by + 2.0), 8);
    cv.fill_polygon(&trunk, hex_rgba(0x33251f, 1.0));

    // Root moss touch
    let moss = hex_rgba(0x376143, 1.0);
    cv.fill_ellipse(cx - 6.0, by, 7.0, 4.0, -0.2, moss);
    cv.fill_ellipse(cx + 7.0, by + 1.0, 6.0, 3.5, 0.25, moss);

    match variant {
        0 => {
            draw_leaf_cluster(cv, cx - 18.0, by - 36.0, 22.0, 18.0, 0x132a21, 0x1e4635, 0x3e734c, -0.2);
            draw_leaf_cluster(cv, cx + 18.0, by - 34.0, 21.0, 17.0, 0x132a21, 0x1b4030, 0x386b46, 0.2);
            draw_leaf_cluster(cv, cx, by - 52.0, 27.0, 23.0, 0x173428, 0x295b41, 0x6b9848, 0.0);
            draw_leaf_cluster(cv, cx - 5.0, by - 64.0, 17.0, 13.0, 0x224a36, 0x4a7f46, 0x8db355, -0.15);
        }
        1 => {
            let tiers = [
                (cx, by - 30.0, 32.0, 14.0, 0x11261f, 0x1c4435, 0x34694d),
                (cx, by - 48.0, 25.0, 13.0, 0x142f26, 0x22523f, 0x467f58),
                (cx, by - 65.0, 19.0, 12.0, 0x18372c, 0x2d634a, 0x649a5c),
                (cx - 1.0, by - 80.0, 12.0, 11.0, 0x1e4535, 0x427b52, 0x89b462),
            ];
            for (x, y, rx, ry, a, b, c) in tiers {
                draw_leaf_cluster(cv, x, y, rx, ry, a, b, c, 0.0);
            }
        }
        2 => {
            draw_leaf_cluster(cv, cx - 15.0, by - 46.0, 23.0, 21.0, 0x122921, 0x214b39, 0x4f824e, -0.25);
            draw_leaf_cluster(cv, cx + 14.0, by - 50.0, 22.0, 20.0, 0x122921, 0x1f4736, 0x48794a, 0.22);
            draw_leaf_cluster(cv, cx, by - 64.0, 20.0, 17.0, 0x1a3b2c, 0x3b7149, 0x7ea653, 0.0);
            let mana = hex_rgba(0x4ff3e6, 1.0);
            for (bx, byp, br) in [
                (cx - 16.0, by - 48.0, 2.2),
                (cx + 12.0, by - 56.0, 2.0),
                (cx - 2.0, by - 71.0, 1.8),
            ] {
                cv.fill_ellipse(bx, byp, br, br, 0.0, mana);
            }
        }
        3 => {
            draw_leaf_cluster(cv, cx - 16.0, by - 22.0, 18.0, 14.0, 0x11251e, 0x1e4637, 0x386e4c, -0.18);
            draw_leaf_cluster(cv, cx + 16.0, by - 21.0, 18.0, 14.0, 0x11251e, 0x1a3e30, 0x316244, 0.2);
            draw_leaf_cluster(cv, cx, by - 31.0, 22.0, 17.0, 0x163228, 0x2a5a41, 0x5c8d4d, 0.0);
        }
        4 => {
            draw_leaf_cluster(cv, cx - 15.0, by - 38.0, 20.0, 17.0, 0x222b20, 0x395534, 0x6d8a42, -0.2);
            draw_leaf_cluster(cv, cx + 16.0, by - 37.0, 20.0, 17.0, 0x222b20, 0x334d2f, 0x5d7c3c, 0.18);
            draw_leaf_cluster(cv, cx, by - 55.0, 25.0, 21.0, 0x263421, 0x4c6d37, 0x9eb14f, 0.0);
            draw_leaf_cluster(cv, cx - 3.0, by - 66.0, 15.0, 11.0, 0x3b4e27, 0x738e42, 0xbfd062, -0.1);
        }
        _ => {
            let spire = [
                (cx - 24.0, by + 2.0),
                (cx - 18.0, by - 38.0),
                (cx - 4.0, by - 58.0),
                (cx + 15.0, by - 46.0),
                (cx + 23.0, by + 2.0),
            ];
            cv.fill_polygon(&spire, hex_rgba(0x242b2f, 1.0));
            let facet = [
                (cx - 18.0, by - 38.0),
                (cx - 4.0, by - 58.0),
                (cx + 2.0, by - 32.0),
                (cx - 12.0, by + 2.0),
                (cx - 24.0, by + 2.0),
            ];
            cv.fill_polygon(&facet, hex_rgba(0x475257, 1.0));
            cv.stroke_polyline(
                &[
                    (cx - 7.0, by - 40.0),
                    (cx - 3.0, by - 25.0),
                    (cx - 9.0, by - 14.0),
                ],
                2.0,
                [54.0 / 255.0, 240.0 / 255.0, 228.0 / 255.0, 0.72],
            );
            draw_leaf_cluster(cv, cx + 4.0, by - 48.0, 17.0, 11.0, 0x163127, 0x2f6043, 0x6b9749, 0.15);
        }
    }
}

fn paint_ward_cell(cv: &mut RiftCanvas, ox: f64, oy: f64, variant: usize) {
    let cx = ox + 64.0;
    let by = oy + 104.0;

    let glow_stops = if variant == 0 {
        [
            (0.0, [72.0 / 255.0, 248.0 / 255.0, 236.0 / 255.0, 0.55]),
            (0.45, [34.0 / 255.0, 184.0 / 255.0, 188.0 / 255.0, 0.22]),
            (1.0, [0.0, 0.0, 0.0, 0.0]),
        ]
    } else {
        [
            (0.0, [1.0, 214.0 / 255.0, 108.0 / 255.0, 0.55]),
            (0.45, [208.0 / 255.0, 146.0 / 255.0, 56.0 / 255.0, 0.22]),
            (1.0, [0.0, 0.0, 0.0, 0.0]),
        ]
    };
    cv.fill_ellipse_radial(cx, by - 36.0, 52.0, 52.0, cx, by - 44.0, 4.0, cx, by - 30.0, 52.0, &glow_stops);

    cv.fill_ellipse(
        cx + 2.0,
        by + 2.0,
        22.0,
        8.0,
        0.0,
        [8.0 / 255.0, 16.0 / 255.0, 18.0 / 255.0, 0.48],
    );

    let ped = [
        (cx - 17.0, by),
        (cx - 12.0, by - 24.0),
        (cx + 12.0, by - 24.0),
        (cx + 17.0, by),
    ];
    cv.fill_polygon(&ped, hex_rgba(0x333b40, 1.0));
    let ped_hi = [
        (cx - 17.0, by),
        (cx - 12.0, by - 24.0),
        (cx, by - 24.0),
        (cx - 2.0, by),
    ];
    cv.fill_polygon(&ped_hi, hex_rgba(0x536067, 1.0));

    let brass = hex_rgba(0xc89e4c, 1.0);
    cv.stroke_polyline(&[(cx - 14.0, by - 24.0), (cx + 14.0, by - 24.0)], 3.0, brass);
    cv.stroke_polyline(
        &[(cx - 10.0, by - 24.0), (cx - 14.0, by - 46.0), (cx - 6.0, by - 55.0)],
        2.4,
        brass,
    );
    cv.stroke_polyline(
        &[(cx + 10.0, by - 24.0), (cx + 14.0, by - 46.0), (cx + 6.0, by - 55.0)],
        2.4,
        brass,
    );

    let (c_top, c_mid, c_dark) = if variant == 0 {
        (0xa8fff7, 0x2ae3d8, 0x128c96)
    } else {
        (0xfff0b2, 0xf2b749, 0xa86c1e)
    };
    let cy = by - 50.0;
    cv.fill_polygon(
        &[(cx, cy - 22.0), (cx + 11.0, cy), (cx, cy + 20.0), (cx - 11.0, cy)],
        hex_rgba(c_dark, 1.0),
    );
    cv.fill_polygon(
        &[(cx, cy - 22.0), (cx + 3.0, cy), (cx, cy + 20.0), (cx - 11.0, cy)],
        hex_rgba(c_mid, 1.0),
    );
    cv.fill_polygon(
        &[(cx, cy - 20.0), (cx + 2.0, cy - 4.0), (cx - 7.0, cy - 2.0)],
        hex_rgba(c_top, 1.0),
    );
}

fn paint_wisp_cell(cv: &mut RiftCanvas, ox: f64, oy: f64, variant: usize) {
    let cx = ox + 64.0;
    let cy = oy + 64.0;
    let stops = match variant % 3 {
        0 => [
            (0.0, [195.0 / 255.0, 1.0, 250.0 / 255.0, 0.98]),
            (0.26, [46.0 / 255.0, 232.0 / 255.0, 216.0 / 255.0, 0.68]),
            (1.0, [30.0 / 255.0, 170.0 / 255.0, 180.0 / 255.0, 0.0]),
        ],
        1 => [
            (0.0, [215.0 / 255.0, 240.0 / 255.0, 1.0, 0.98]),
            (0.26, [82.0 / 255.0, 174.0 / 255.0, 1.0, 0.65]),
            (1.0, [40.0 / 255.0, 108.0 / 255.0, 215.0 / 255.0, 0.0]),
        ],
        _ => [
            (0.0, [1.0, 244.0 / 255.0, 196.0 / 255.0, 0.98]),
            (0.26, [248.0 / 255.0, 196.0 / 255.0, 76.0 / 255.0, 0.68]),
            (1.0, [204.0 / 255.0, 132.0 / 255.0, 34.0 / 255.0, 0.0]),
        ],
    };
    cv.fill_ellipse_radial(cx, cy, 44.0, 44.0, cx, cy, 2.0, cx, cy, 44.0, &stops);
    cv.fill_polygon(
        &[(cx, cy - 8.0), (cx + 6.0, cy), (cx, cy + 8.0), (cx - 6.0, cy)],
        [1.0, 1.0, 1.0, 1.0],
    );
}

fn paint_hero_base(cv: &mut RiftCanvas, cx: f64, by: f64, glow_rgb: Option<[f64; 3]>) {
    cv.fill_ellipse(
        cx,
        by + 2.0,
        22.0,
        8.0,
        0.0,
        [6.0 / 255.0, 14.0 / 255.0, 14.0 / 255.0, 0.45],
    );
    if let Some(rgb) = glow_rgb {
        let stops = [
            (0.0, [rgb[0], rgb[1], rgb[2], 0.42]),
            (1.0, [rgb[0], rgb[1], rgb[2], 0.0]),
        ];
        cv.fill_ellipse_radial(cx, by - 2.0, 28.0, 13.0, cx, by - 2.0, 2.0, cx, by - 2.0, 28.0, &stops);
    }
}

fn paint_chibi_head(cv: &mut RiftCanvas, cx: f64, hy: f64, skin_hex: u32, hair_hex: u32, eye_hex: u32) {
    let ink = hex_rgba(0x181e24, 1.0);
    cv.fill_ellipse(cx, hy, 13.5, 13.5, 0.0, hex_rgba(skin_hex, 1.0));
    cv.stroke_ellipse(cx, hy, 13.5, 13.5, 0.0, 2.2, ink);

    let blush = [244.0 / 255.0, 130.0 / 255.0, 135.0 / 255.0, 0.42];
    cv.fill_ellipse(cx - 7.5, hy + 3.5, 3.0, 1.8, 0.0, blush);
    cv.fill_ellipse(cx + 7.5, hy + 3.5, 3.0, 1.8, 0.0, blush);

    let eye = hex_rgba(eye_hex, 1.0);
    cv.fill_ellipse(cx - 3.6, hy + 1.2, 2.0, 2.7, 0.0, eye);
    cv.fill_ellipse(cx + 5.2, hy + 1.2, 2.0, 2.7, 0.0, eye);

    let white = [1.0, 1.0, 1.0, 1.0];
    cv.fill_ellipse(cx - 3.1, hy + 0.3, 0.85, 0.85, 0.0, white);
    cv.fill_ellipse(cx + 5.7, hy + 0.3, 0.85, 0.85, 0.0, white);

    // Hair cap bangs
    let mut bangs = Vec::with_capacity(44);
    let steps = 20;
    let a0 = PI * 0.98;
    let a1 = PI * 2.04;
    for i in 0..=steps {
        let a = a0 + (a1 - a0) * (i as f64 / steps as f64);
        bangs.push((cx + a.cos() * 14.2, hy - 3.5 + a.sin() * 14.2));
    }
    let last = *bangs.last().unwrap();
    push_quad(&mut bangs, last, (cx + 8.0, hy - 3.0), (cx + 2.0, hy - 7.0), 8);
    push_quad(&mut bangs, (cx + 2.0, hy - 7.0), (cx - 5.0, hy - 2.0), (cx - 14.0, hy - 3.0), 8);
    cv.fill_polygon(&bangs, hex_rgba(hair_hex, 1.0));
    cv.stroke_polygon_closed(&bangs, 2.2, ink);
}

fn paint_hero_cell(cv: &mut RiftCanvas, ox: f64, oy: f64, kind: usize) {
    let cx = ox + 64.0;
    let by = oy + 108.0;

    match kind {
        0 => {
            // Wind Ronin (Yasuo-inspired)
            paint_hero_base(cv, cx, by, Some([90.0 / 255.0, 225.0 / 255.0, 220.0 / 255.0]));
            cv.stroke_arc(
                cx - 2.0,
                by - 36.0,
                32.0,
                PI * 0.72,
                PI * 1.48,
                3.0,
                [130.0 / 255.0, 245.0 / 255.0, 240.0 / 255.0, 0.65],
            );

            let ink = hex_rgba(0x11161d, 1.0);
            let mut pony = Vec::with_capacity(20);
            pony.push((cx - 8.0, by - 58.0));
            push_quad(&mut pony, (cx - 8.0, by - 58.0), (cx - 30.0, by - 64.0), (cx - 34.0, by - 42.0), 8);
            push_quad(&mut pony, (cx - 34.0, by - 42.0), (cx - 21.0, by - 46.0), (cx - 10.0, by - 50.0), 8);
            cv.fill_polygon(&pony, hex_rgba(0x1e2736, 1.0));
            cv.stroke_polygon_closed(&pony, 2.0, ink);

            let tunic = [
                (cx - 12.0, by - 40.0),
                (cx + 12.0, by - 40.0),
                (cx + 16.0, by - 8.0),
                (cx - 14.0, by - 8.0),
            ];
            cv.fill_polygon(&tunic, hex_rgba(0x263448, 1.0));
            cv.stroke_polygon_closed(&tunic, 2.0, ink);

            cv.fill_ellipse(cx - 12.0, by - 36.0, 8.0, 6.0, -0.35, hex_rgba(0x8b99a8, 1.0));
            cv.stroke_ellipse(cx - 12.0, by - 36.0, 8.0, 6.0, -0.35, 2.0, ink);

            let mut sash = Vec::with_capacity(12);
            sash.push((cx - 14.0, by - 34.0));
            push_quad(&mut sash, (cx - 14.0, by - 34.0), (cx - 32.0, by - 22.0), (cx - 26.0, by - 12.0), 8);
            sash.push((cx - 10.0, by - 25.0));
            cv.fill_polygon(&sash, hex_rgba(0x38b6ff, 1.0));

            cv.stroke_polyline(
                &[(cx + 8.0, by - 26.0), (cx + 38.0, by - 46.0)],
                3.2,
                hex_rgba(0xd9eef7, 1.0),
            );
            cv.stroke_polyline(
                &[(cx + 6.0, by - 23.0), (cx + 12.0, by - 30.0)],
                4.0,
                hex_rgba(0xd4a64a, 1.0),
            );

            paint_chibi_head(cv, cx, by - 52.0, 0xf4cbb2, 0x1e2736, 0x25394f);
            cv.fill_polygon(
                &[
                    (cx - 8.0, by - 64.0),
                    (cx - 2.0, by - 75.0),
                    (cx + 5.0, by - 65.0),
                    (cx + 12.0, by - 71.0),
                    (cx + 13.0, by - 60.0),
                ],
                hex_rgba(0x1e2736, 1.0),
            );
        }
        1 => {
            // Nine-Tail Fox Mage (Ahri-inspired)
            paint_hero_base(cv, cx, by, Some([72.0 / 255.0, 238.0 / 255.0, 1.0]));

            let tails = [
                (-22.0, -34.0, -0.7),
                (-14.0, -42.0, -0.35),
                (-2.0, -45.0, 0.0),
                (10.0, -40.0, 0.35),
                (-26.0, -22.0, -1.0),
            ];
            for (tx, ty, rot) in tails {
                let tcx = cx + tx;
                let tcy = by + ty;
                cv.fill_ellipse(tcx, tcy, 9.0, 17.0, rot, hex_rgba(0xfff5ea, 1.0));
                cv.stroke_ellipse(tcx, tcy, 9.0, 17.0, rot, 1.6, hex_rgba(0x332124, 1.0));
                let tip = rot_local(tcx, tcy, rot, 0.0, -10.0);
                cv.fill_ellipse(tip.0, tip.1, 5.5, 6.5, rot, hex_rgba(0xffd480, 1.0));
            }

            let robe = [
                (cx - 10.0, by - 38.0),
                (cx + 10.0, by - 38.0),
                (cx + 17.0, by - 6.0),
                (cx - 15.0, by - 6.0),
            ];
            cv.fill_polygon(&robe, hex_rgba(0xc82d46, 1.0));
            cv.stroke_polygon_closed(&robe, 2.0, hex_rgba(0x241418, 1.0));

            let skirt = [
                (cx - 9.0, by - 22.0),
                (cx + 11.0, by - 22.0),
                (cx + 15.0, by - 7.0),
                (cx - 13.0, by - 7.0),
            ];
            cv.fill_polygon(&skirt, hex_rgba(0xfff6ec, 1.0));
            cv.stroke_polyline(
                &[(cx - 10.0, by - 22.0), (cx + 11.0, by - 22.0)],
                2.0,
                hex_rgba(0xdfae4f, 1.0),
            );

            paint_chibi_head(cv, cx, by - 51.0, 0xfde0d2, 0x3a2220, 0xd88c28);

            for dir in [-1.0, 1.0] {
                let ear = [
                    (cx + dir * 4.0, by - 62.0),
                    (cx + dir * 13.0, by - 79.0),
                    (cx + dir * 15.0, by - 58.0),
                ];
                cv.fill_polygon(&ear, hex_rgba(0x2d1a18, 1.0));
                cv.stroke_polygon_closed(&ear, 1.8, hex_rgba(0x181010, 1.0));
                let inner = [
                    (cx + dir * 7.0, by - 62.0),
                    (cx + dir * 12.0, by - 73.0),
                    (cx + dir * 13.0, by - 60.0),
                ];
                cv.fill_polygon(&inner, hex_rgba(0xf8a5a8, 1.0));
            }

            let orb_stops = [
                (0.0, [1.0, 1.0, 1.0, 1.0]),
                (0.38, hex_rgba(0x5bf5ff, 1.0)),
                (1.0, [40.0 / 255.0, 210.0 / 255.0, 1.0, 0.0]),
            ];
            cv.fill_ellipse_radial(
                cx + 25.0,
                by - 26.0,
                15.0,
                15.0,
                cx + 25.0,
                by - 26.0,
                2.0,
                cx + 25.0,
                by - 26.0,
                15.0,
                &orb_stops,
            );
        }
        2 => {
            // Frost Archer (Ashe-inspired)
            paint_hero_base(cv, cx, by, Some([110.0 / 255.0, 215.0 / 255.0, 1.0]));

            let cloak = [
                (cx - 12.0, by - 42.0),
                (cx + 12.0, by - 42.0),
                (cx + 19.0, by - 6.0),
                (cx - 21.0, by - 6.0),
            ];
            cv.fill_polygon(&cloak, hex_rgba(0x1c447c, 1.0));
            cv.stroke_polygon_closed(&cloak, 2.0, hex_rgba(0x10223d, 1.0));

            let crest = [
                (cx, by - 36.0),
                (cx + 7.0, by - 28.0),
                (cx, by - 20.0),
                (cx - 7.0, by - 28.0),
            ];
            cv.fill_polygon(&crest, hex_rgba(0xe5be5c, 1.0));

            cv.fill_ellipse(cx - 12.0, by - 42.0, 4.5, 11.0, 0.2, hex_rgba(0xeef6fc, 1.0));
            cv.stroke_ellipse(cx - 12.0, by - 42.0, 4.5, 11.0, 0.2, 1.6, hex_rgba(0x4e6b88, 1.0));

            paint_chibi_head(cv, cx, by - 51.0, 0xfbe3d8, 0xeef6fc, 0x2b9be8);

            cv.stroke_arc(cx, by - 52.0, 15.5, PI * 0.9, PI * 2.1, 2.6, hex_rgba(0xe5be5c, 1.0));
            cv.fill_polygon(
                &[(cx, by - 73.0), (cx + 4.0, by - 64.0), (cx - 4.0, by - 64.0)],
                hex_rgba(0x6feaff, 1.0),
            );

            cv.stroke_arc(cx + 16.0, by - 28.0, 18.0, -PI * 0.45, PI * 0.45, 3.5, hex_rgba(0x7bf5ff, 1.0));
            cv.stroke_polyline(
                &[
                    (cx + 18.0, by - 45.0),
                    (cx + 14.0, by - 28.0),
                    (cx + 18.0, by - 11.0),
                ],
                1.4,
                [1.0, 1.0, 1.0, 1.0],
            );
        }
        3 => {
            // Starlight Mage (Lux-inspired)
            paint_hero_base(cv, cx, by, Some([1.0, 224.0 / 255.0, 128.0 / 255.0]));

            let cape = [
                (cx - 14.0, by - 40.0),
                (cx + 14.0, by - 40.0),
                (cx + 19.0, by - 8.0),
                (cx - 19.0, by - 8.0),
            ];
            cv.fill_polygon(&cape, hex_rgba(0x255fb0, 1.0));

            let dress = [
                (cx - 10.0, by - 40.0),
                (cx + 10.0, by - 40.0),
                (cx + 13.0, by - 11.0),
                (cx - 13.0, by - 11.0),
            ];
            cv.fill_polygon(&dress, hex_rgba(0xf2f4f8, 1.0));
            cv.stroke_polygon_closed(&dress, 2.0, hex_rgba(0x172844, 1.0));

            cv.fill_rect(cx - 13.0, by - 40.0, 26.0, 6.0, hex_rgba(0xeac258, 1.0));

            paint_chibi_head(cv, cx, by - 52.0, 0xfde2d0, 0xf7cf56, 0x3388e8);

            let blonde = hex_rgba(0xf7cf56, 1.0);
            cv.fill_ellipse(cx - 12.0, by - 44.0, 4.0, 10.0, 0.15, blonde);
            cv.fill_ellipse(cx + 12.0, by - 44.0, 4.0, 10.0, -0.15, blonde);

            cv.stroke_polyline(
                &[(cx + 22.0, by - 4.0), (cx + 26.0, by - 66.0)],
                3.2,
                hex_rgba(0xe5b84b, 1.0),
            );
            let sg_stops = [
                (0.0, [1.0, 1.0, 1.0, 1.0]),
                (0.35, hex_rgba(0xfff09c, 1.0)),
                (1.0, [1.0, 220.0 / 255.0, 110.0 / 255.0, 0.0]),
            ];
            cv.fill_ellipse_radial(
                cx + 26.0,
                by - 70.0,
                16.0,
                16.0,
                cx + 26.0,
                by - 70.0,
                2.0,
                cx + 26.0,
                by - 70.0,
                16.0,
                &sg_stops,
            );
        }
        _ => {
            // Fluffy Rift Poro
            paint_hero_base(cv, cx, by, None);
            let py = by - 26.0;

            cv.fill_ellipse(cx, py, 24.0, 24.0, 0.0, hex_rgba(0xfafcff, 1.0));
            cv.stroke_ellipse(cx, py, 24.0, 24.0, 0.0, 2.2, hex_rgba(0x536270, 1.0));

            // Underbelly lower half-ellipse
            let mut belly = Vec::with_capacity(22);
            for i in 0..=20 {
                let a = (i as f64 / 20.0) * PI;
                belly.push((cx + a.cos() * 18.0, py + 11.0 + a.sin() * 9.0));
            }
            cv.fill_polygon(&belly, hex_rgba(0xdce6f2, 1.0));

            // Curled brown horns
            for dir in [-1.0, 1.0] {
                cv.fill_ellipse(cx + dir * 16.0, py - 18.0, 7.0, 5.0, dir * 0.55, hex_rgba(0x9c6e4b, 1.0));
                cv.stroke_ellipse(cx + dir * 16.0, py - 18.0, 7.0, 5.0, dir * 0.55, 1.8, hex_rgba(0x472f1e, 1.0));
            }

            // Wide eyes
            let dark = hex_rgba(0x1d2228, 1.0);
            cv.fill_ellipse(cx - 8.0, py - 4.0, 3.3, 3.3, 0.0, dark);
            cv.fill_ellipse(cx + 9.0, py - 4.0, 3.3, 3.3, 0.0, dark);
            let white = [1.0, 1.0, 1.0, 1.0];
            cv.fill_ellipse(cx - 9.0, py - 5.2, 1.2, 1.2, 0.0, white);
            cv.fill_ellipse(cx + 8.0, py - 5.2, 1.2, 1.2, 0.0, white);

            // Happy giant pink Poro tongue
            cv.fill_ellipse(cx + 1.0, py + 11.0, 7.5, 9.5, 0.08, hex_rgba(0xff7fa3, 1.0));
            cv.stroke_ellipse(cx + 1.0, py + 11.0, 7.5, 9.5, 0.08, 1.6, hex_rgba(0xb54666, 1.0));
            cv.stroke_polyline(
                &[(cx + 1.0, py + 4.0), (cx + 1.0, py + 14.0)],
                1.6,
                hex_rgba(0xb54666, 1.0),
            );
        }
    }
}

fn paint_turret_cell(cv: &mut RiftCanvas, ox: f64, oy: f64, team: usize) {
    let cx = ox + 64.0;
    let by = oy + 112.0;
    let is_blue = team == 0;

    let stone_shadow = hex_rgba(if is_blue { 0x28343d } else { 0x251d24 }, 1.0);
    let stone_main = hex_rgba(if is_blue { 0x4e5f6b } else { 0x40343e }, 1.0);
    let stone_light = hex_rgba(if is_blue { 0x7a8f9a } else { 0x655462 }, 1.0);
    let metal_col = hex_rgba(if is_blue { 0xd4ad52 } else { 0xc67d38 }, 1.0);
    let metal_dark = hex_rgba(if is_blue { 0x876726 } else { 0x734219 }, 1.0);
    let orb_core = hex_rgba(if is_blue { 0xc6ffff } else { 0xffe3a8 }, 1.0);
    let orb_mid = hex_rgba(if is_blue { 0x2ee6df } else { 0xff5e3a }, 1.0);
    let orb_dark = hex_rgba(if is_blue { 0x11798a } else { 0x9c1b1e }, 1.0);
    let halo_rgb = if is_blue {
        [48.0 / 255.0, 232.0 / 255.0, 224.0 / 255.0]
    } else {
        [1.0, 96.0 / 255.0, 56.0 / 255.0]
    };

    // Wide foundation shadow
    cv.fill_ellipse(
        cx + 3.0,
        by + 3.0,
        34.0,
        13.0,
        0.0,
        [7.0 / 255.0, 12.0 / 255.0, 14.0 / 255.0, 0.52],
    );

    let orb_y = by - 78.0;
    let halo_stops = [
        (0.0, [halo_rgb[0], halo_rgb[1], halo_rgb[2], 0.58]),
        (0.5, [halo_rgb[0], halo_rgb[1], halo_rgb[2], 0.18]),
        (1.0, [halo_rgb[0], halo_rgb[1], halo_rgb[2], 0.0]),
    ];
    cv.fill_ellipse_radial(cx, orb_y, 46.0, 46.0, cx, orb_y, 4.0, cx, orb_y, 46.0, &halo_stops);

    let ink = hex_rgba(0x14181d, 1.0);
    let plinth = [
        (cx - 31.0, by),
        (cx - 26.0, by - 16.0),
        (cx + 26.0, by - 16.0),
        (cx + 31.0, by),
    ];
    cv.fill_polygon(&plinth, stone_shadow);
    cv.stroke_polygon_closed(&plinth, 2.2, ink);

    let plinth_hi = [
        (cx - 31.0, by),
        (cx - 26.0, by - 16.0),
        (cx - 4.0, by - 16.0),
        (cx - 7.0, by),
    ];
    cv.fill_polygon(&plinth_hi, stone_light);

    let shaft = [
        (cx - 22.0, by - 16.0),
        (cx - 15.0, by - 62.0),
        (cx + 15.0, by - 62.0),
        (cx + 22.0, by - 16.0),
    ];
    cv.fill_polygon(&shaft, stone_main);
    cv.stroke_polygon_closed(&shaft, 2.2, ink);

    let shaft_hi = [
        (cx - 22.0, by - 16.0),
        (cx - 15.0, by - 62.0),
        (cx - 2.0, by - 62.0),
        (cx - 4.0, by - 16.0),
    ];
    cv.fill_polygon(&shaft_hi, stone_light);

    // Carved team rune inlay
    cv.stroke_polyline(&[(cx, by - 55.0), (cx, by - 25.0)], 2.4, orb_mid);
    cv.stroke_polyline(
        &[(cx - 6.0, by - 44.0), (cx, by - 50.0), (cx + 6.0, by - 44.0)],
        2.4,
        orb_mid,
    );

    // Brass/bronze armor collar & cradle prongs
    cv.fill_rect(cx - 18.0, by - 64.0, 36.0, 7.0, metal_dark);
    cv.fill_rect(cx - 19.0, by - 66.0, 38.0, 4.5, metal_col);

    let mut prong_l = vec![(cx - 14.0, by - 64.0)];
    push_quad(&mut prong_l, (cx - 14.0, by - 64.0), (cx - 22.0, by - 78.0), (cx - 10.0, by - 92.0), 12);
    cv.stroke_polyline(&prong_l, 3.6, metal_col);

    let mut prong_r = vec![(cx + 14.0, by - 64.0)];
    push_quad(&mut prong_r, (cx + 14.0, by - 64.0), (cx + 22.0, by - 78.0), (cx + 10.0, by - 92.0), 12);
    cv.stroke_polyline(&prong_r, 3.6, metal_col);

    // Floating turret crystal orb
    cv.fill_ellipse(cx, orb_y, 12.5, 12.5, 0.0, orb_dark);
    let orb_stops = [(0.0, orb_core), (0.55, orb_mid), (1.0, orb_dark)];
    cv.fill_ellipse_radial(cx, orb_y, 11.5, 11.5, cx - 3.5, orb_y - 4.0, 2.0, cx, orb_y, 12.5, &orb_stops);
    cv.stroke_arc(cx - 3.0, orb_y - 3.0, 4.2, PI * 0.9, PI * 1.65, 1.1, [1.0, 1.0, 1.0, 1.0]);
}

pub fn build_rift_atlas() -> (u32, u32, Vec<u8>) {
    // Bake at 2x supersampled resolution (2048x768) for razor-sharp mipmapped sprites
    let mut cv = RiftCanvas::new(RIFT_COLS * RIFT_CELL, RIFT_ROWS * RIFT_CELL, 2);
    let c = RIFT_CELL as f64;

    // Row 0: 6 trees/cliff spire + 2 ward lanterns
    for i in 0..6 {
        paint_tree_cell(&mut cv, i as f64 * c, 0.0, i);
    }
    paint_ward_cell(&mut cv, 6.0 * c, 0.0, 0);
    paint_ward_cell(&mut cv, 7.0 * c, 0.0, 1);

    // Row 1: 3 arcane wisps + 5 champions & Poro
    for i in 0..3 {
        paint_wisp_cell(&mut cv, i as f64 * c, c, i);
    }
    for i in 0..5 {
        paint_hero_cell(&mut cv, (3 + i) as f64 * c, c, i);
    }

    // Row 2: Order Blue Turret & Chaos Crimson Turret
    paint_turret_cell(&mut cv, 0.0, 2.0 * c, 0);
    paint_turret_cell(&mut cv, c, 2.0 * c, 1);

    (cv.w as u32, cv.h as u32, cv.px)
}

// =========================================================================
// OpenGL 3.3 Core Renderer
// =========================================================================
struct RiftInstItem {
    sort_y: f64,
    x: f32,
    y: f32,
    size: f32,
    rot: f32,
    col: f32,
    row: f32,
    alpha: f32,
    flip: f32,
}

pub struct RiftRenderer {
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
    // Ground uniforms
    u_noise: GLint,
    u_fa: GLint,
    u_fb: GLint,
    u_res: GLint,
    u_grid: GLint,
    u_scale: GLint,
    u_seed: GLint,
    u_step: GLint,
    u_mix: GLint,
    u_time: GLint,
    u_glow: GLint,
    u_wind: GLint,
    u_debug: GLint,
    // Instanced uniforms
    ui_atlas: GLint,
    ui_res: GLint,
    // Reusable staging buffers
    items: Vec<RiftInstItem>,
    buf_a: Vec<f32>,
    buf_b: Vec<f32>,
}

impl RiftRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let ground_prog = gl.compile_program(RIFT_VERT, RIFT_FRAG)?;
        let inst_prog = gl.compile_program(RIFT_INST_VERT, RIFT_INST_FRAG)?;

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

        let (aw, ah, apx) = build_rift_atlas();
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
            u_noise: gl.uniform_loc(ground_prog, "uNoise"),
            u_fa: gl.uniform_loc(ground_prog, "uFA"),
            u_fb: gl.uniform_loc(ground_prog, "uFB"),
            u_res: gl.uniform_loc(ground_prog, "uRes"),
            u_grid: gl.uniform_loc(ground_prog, "uGrid"),
            u_scale: gl.uniform_loc(ground_prog, "uScale"),
            u_seed: gl.uniform_loc(ground_prog, "uSeed"),
            u_step: gl.uniform_loc(ground_prog, "uStep"),
            u_mix: gl.uniform_loc(ground_prog, "uMix"),
            u_time: gl.uniform_loc(ground_prog, "uTime"),
            u_glow: gl.uniform_loc(ground_prog, "uGlow"),
            u_wind: gl.uniform_loc(ground_prog, "uWind"),
            u_debug: gl.uniform_loc(ground_prog, "uDebug"),
            ui_atlas: gl.uniform_loc(inst_prog, "uAtlas"),
            ui_res: gl.uniform_loc(inst_prog, "uRes"),
            items: Vec::with_capacity(512),
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

    unsafe fn sync_noise_tex(&mut self, gl: &GlFns, rs: &RiftState) {
        if self.uploaded_noise_version == rs.noise_version {
            return;
        }
        let (s, d) = make_noise_image(rs.params.seed);
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
        self.uploaded_noise_version = rs.noise_version;
    }

    unsafe fn sync_field_textures(&mut self, gl: &GlFns, rs: &RiftState) {
        if self.uploaded_field_version == rs.field_version
            && self.field_gw == rs.gw
            && self.field_gh == rs.gh
        {
            return;
        }
        let realloc = self.field_gw != rs.gw || self.field_gh != rs.gh;
        for (i, data) in [&rs.field_a, &rs.field_b].iter().enumerate() {
            (gl.ActiveTexture)(if i == 0 { GL_TEXTURE1 } else { GL_TEXTURE2 });
            (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[i]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            if realloc {
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_R32F as GLint,
                    rs.gw as GLsizei,
                    rs.gh as GLsizei,
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
                    rs.gw as GLsizei,
                    rs.gh as GLsizei,
                    GL_RED,
                    GL_FLOAT,
                    data.as_ptr() as *const c_void,
                );
            }
        }
        self.field_gw = rs.gw;
        self.field_gh = rs.gh;
        self.uploaded_field_version = rs.field_version;
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.5);

        st.rift.resize(css_w, css_h);
        let rs = &st.rift;
        self.sync_noise_tex(gl, rs);
        self.sync_field_textures(gl, rs);

        let res_x = pw as f32 / dpi;
        let res_y = ph as f32 / dpi;
        let time_sec = rs.time_sec;
        let glow = rs.params.glow;
        let wind = rs.params.wind;

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_SCISSOR_TEST);

        // Pass 1: Jungle wilderness, serpentine river & carved stone Hextech sanctuaries
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
        (gl.Uniform2f)(self.u_grid, rs.gw as f32, rs.gh as f32);
        (gl.Uniform1f)(self.u_scale, dpi);
        (gl.Uniform1f)(self.u_seed, (rs.params.seed % 997) as f32 * 0.37);
        (gl.Uniform1f)(self.u_step, STEP as f32);
        (gl.Uniform1f)(self.u_mix, rs.cur_mix as f32);
        (gl.Uniform1f)(self.u_time, time_sec as f32);
        (gl.Uniform1f)(self.u_glow, glow as f32);
        (gl.Uniform1f)(self.u_wind, wind as f32);
        (gl.Uniform1f)(self.u_debug, if rs.params.debug { 1.0 } else { 0.0 });
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Pass 2: Y-sorted single instanced draw call for trees, wards, turrets, heroes & wisps
        self.items.clear();

        for t in &rs.trees {
            if t.grow <= 0.01 {
                continue;
            }
            let ease = t.grow * t.grow * (3.0 - 2.0 * t.grow);
            let sway = (time_sec * 1.5 + t.phase + t.x * 0.01).sin() * 0.038 * wind;
            self.items.push(RiftInstItem {
                sort_y: t.y,
                x: t.x as f32,
                y: t.y as f32,
                size: (88.0 * t.scale * (0.28 + 0.72 * ease)) as f32,
                rot: sway as f32,
                col: t.variant as f32,
                row: 0.0,
                alpha: (ease * 1.2).min(1.0) as f32,
                flip: t.mirror as f32,
            });
        }

        for w in &rs.wards {
            let ease = w.grow * w.grow * (3.0 - 2.0 * w.grow);
            let hover = (time_sec * 2.2 + w.phase).sin() * 2.5 * wind;
            let pulse = 0.68 + 0.32 * glow + 0.14 * (time_sec * 3.1 + w.phase).sin();
            self.items.push(RiftInstItem {
                sort_y: w.y + 4.0,
                x: w.x as f32,
                y: (w.y + hover) as f32,
                size: (70.0 * (0.4 + 0.6 * ease)) as f32,
                rot: 0.0,
                col: (6 + w.variant) as f32,
                row: 0.0,
                alpha: (ease * pulse).min(1.0) as f32,
                flip: 1.0,
            });
        }

        for tr in &rs.turrets {
            let ease = tr.grow * tr.grow * (3.0 - 2.0 * tr.grow);
            self.items.push(RiftInstItem {
                sort_y: tr.y + 6.0,
                x: tr.x as f32,
                y: tr.y as f32,
                size: (98.0 * (0.42 + 0.58 * ease)) as f32,
                rot: 0.0,
                col: tr.team as f32,
                row: 2.0,
                alpha: (ease * 1.15).min(1.0) as f32,
                flip: if tr.team == 1 { -1.0 } else { 1.0 },
            });
            let orb_pulse = (0.42 + 0.58 * glow) * (0.72 + 0.28 * (time_sec * 3.4 + tr.phase).sin());
            self.items.push(RiftInstItem {
                sort_y: tr.y + 7.0,
                x: tr.x as f32,
                y: (tr.y - 58.0 * (0.42 + 0.58 * ease)) as f32,
                size: (34.0 * (0.75 + 0.35 * glow)) as f32,
                rot: (time_sec * 0.7 + tr.phase) as f32,
                col: if tr.team == 0 { 0.0 } else { 2.0 },
                row: 1.0,
                alpha: (ease * orb_pulse).min(1.0) as f32,
                flip: 1.0,
            });
        }

        for h in &rs.heroes {
            let walking = h.wait <= 0.0;
            let hop = if walking {
                h.walk_t.sin().abs() * if h.is_poro { 5.5 } else { 3.2 }
            } else {
                (time_sec * 2.8 + h.phase).sin() * 1.4
            };
            let tilt = if walking { h.walk_t.sin() * 0.055 } else { 0.0 };
            self.items.push(RiftInstItem {
                sort_y: h.y + 8.0,
                x: h.x as f32,
                y: (h.y - hop) as f32,
                size: if h.is_poro { 44.0 } else { 66.0 },
                rot: tilt as f32,
                col: h.col as f32,
                row: 1.0,
                alpha: 1.0,
                flip: h.flip as f32,
            });
        }

        for sp in &rs.wisps {
            let twinkle = (0.42 + 0.58 * glow) * (0.72 + 0.28 * (time_sec * 3.6 + sp.phase).sin());
            self.items.push(RiftInstItem {
                sort_y: sp.y + 80.0,
                x: sp.x as f32,
                y: sp.y as f32,
                size: (sp.size * (0.85 + 0.35 * glow)) as f32,
                rot: (time_sec * 0.4 + sp.phase) as f32,
                col: sp.variant as f32,
                row: 1.0,
                alpha: twinkle.min(1.0) as f32,
                flip: 1.0,
            });
        }

        if !self.items.is_empty() {
            self.items.sort_by(|a, b| a.sort_y.total_cmp(&b.sort_y));
            let count = self.items.len();
            self.buf_a.clear();
            self.buf_b.clear();
            self.buf_a.reserve(count * 4);
            self.buf_b.reserve(count * 4);
            for it in &self.items {
                self.buf_a.push(it.x);
                self.buf_a.push(it.y);
                self.buf_a.push(it.size);
                self.buf_a.push(it.rot);
                self.buf_b.push(it.col / RIFT_COLS as f32);
                self.buf_b.push(it.row / RIFT_ROWS as f32);
                self.buf_b.push(it.alpha);
                self.buf_b.push(it.flip);
            }

            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
            (gl.UseProgram)(self.inst_prog);
            (gl.BindVertexArray)(self.inst_vao);
            (gl.ActiveTexture)(GL_TEXTURE6);
            (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
            (gl.Uniform1i)(self.ui_atlas, 6);
            (gl.Uniform2f)(self.ui_res, res_x, res_y);

            let bytes = (count * 4 * std::mem::size_of::<f32>()) as GLsizeiptr;
            if count > self.max_inst {
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_a);
                (gl.BufferData)(GL_ARRAY_BUFFER, bytes, self.buf_a.as_ptr() as *const c_void, GL_DYNAMIC_DRAW);
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_b);
                (gl.BufferData)(GL_ARRAY_BUFFER, bytes, self.buf_b.as_ptr() as *const c_void, GL_DYNAMIC_DRAW);
                self.max_inst = count;
            } else {
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_a);
                (gl.BufferSubData)(GL_ARRAY_BUFFER, 0, bytes, self.buf_a.as_ptr() as *const c_void);
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_b);
                (gl.BufferSubData)(GL_ARRAY_BUFFER, 0, bytes, self.buf_b.as_ptr() as *const c_void);
            }

            (gl.DrawArraysInstanced)(GL_TRIANGLES, 0, 6, count as GLsizei);
            (gl.BindVertexArray)(0);
            (gl.Disable)(GL_BLEND);
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
            Rect { x: 460.0, y: 240.0, w: 76.0, h: 90.0 },
            Rect { x: 860.0, y: 310.0, w: 76.0, h: 90.0 },
        ]
    }

    #[test]
    fn verify_rust_anything_recorded_rift_cases() {
        let root = repo_root();
        let rec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/rift/recorded-from.json");
        let rec: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&rec_path).unwrap()).unwrap();
        for src in rec["sources"].as_array().unwrap() {
            let rel = src["path"].as_str().unwrap();
            let expected = src["fnv1a"].as_str().unwrap();
            let actual = fnv1a_fingerprint(rel, &std::fs::read(root.join(rel)).unwrap());
            assert_eq!(actual, expected, "Source fingerprint mismatch for {rel}");
        }

        let cases_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/rift/cases.jsonl");
        let lines: Vec<serde_json::Value> = std::fs::read_to_string(&cases_path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 3);

        // Case 1: computeFieldRift + riverDist
        {
            let c = &lines[0];
            assert_eq!(c["fn"], "computeFieldRift");
            let mut rs = RiftState::new(1280.0, 720.0);
            rs.set_icons_raw(&sample_icons(), false);
            let probes = c["input"]["probes"].as_array().unwrap();
            let exp_s = c["output"]["sampled"].as_array().unwrap();
            let exp_r = c["output"]["riverDist"].as_array().unwrap();
            for (k, p) in probes.iter().enumerate() {
                let x = p[0].as_f64().unwrap();
                let y = p[1].as_f64().unwrap();
                assert_close(rs.land_s(x, y), exp_s[k].as_f64().unwrap(), 1e-4, &format!("field[{k}]"));
                assert_close(
                    rift_river_dist_px(42, 1280.0, 720.0, x, y),
                    exp_r[k].as_f64().unwrap(),
                    1e-9,
                    &format!("riverDist[{k}]"),
                );
            }
        }

        // Case 2: riftPropsBuild across 3 configs (matching single-session lifecycle in record_rift.mjs)
        {
            let c = &lines[1];
            assert_eq!(c["fn"], "riftPropsBuild");
            let configs = [
                ("s42", 42, true, 4, 4, 8, 5),
                ("s1337", 1337, true, 6, 6, 10, 5),
                ("s42_noadapt", 42, false, 4, 4, 8, 5),
            ];
            let mut rs = RiftState::new(1280.0, 720.0);
            rs.wisps.clear();
            rs.heroes.clear();
            for (key, seed, adaptive, turrets_n, wards_n, wisps_n, heroes_n) in configs {
                let exp = &c["output"][key];
                let exp_h = exp["hashes"].as_array().unwrap();
                assert_close(rift_hash(seed, 0, 0, 1), exp_h[0].as_f64().unwrap(), 1e-12, "hash0");
                assert_close(rift_hash(seed, 9, 14, 3), exp_h[1].as_f64().unwrap(), 1e-12, "hash1");
                assert_close(rift_hash(seed, 25, 7, 5), exp_h[2].as_f64().unwrap(), 1e-12, "hash2");

                let mut p = rs.params;
                p.seed = seed;
                p.adaptive = adaptive;
                p.turrets = turrets_n;
                p.wards = wards_n;
                p.wisps = wisps_n;
                p.heroes = heroes_n;
                rs.params = p;
                rs.build_noise();
                rs.icons = sample_icons();
                rs.refresh();
                rs.rebuild_props();

                assert_eq!(rs.trees.len(), exp["treeTotal"].as_u64().unwrap() as usize, "{key} treeTotal");
                let grown = rs.trees.iter().filter(|t| t.want > 0.5).count();
                assert_eq!(grown, exp["treeGrown"].as_u64().unwrap() as usize, "{key} treeGrown");
                for s in exp["treeSamples"].as_array().unwrap() {
                    let i = s["i"].as_u64().unwrap() as usize;
                    let t = &rs.trees[i];
                    assert_close(t.x, s["x"].as_f64().unwrap(), 1e-9, &format!("{key} t[{i}].x"));
                    assert_close(t.y, s["y"].as_f64().unwrap(), 1e-9, &format!("{key} t[{i}].y"));
                    assert_eq!(t.variant, s["variant"].as_u64().unwrap() as usize);
                    assert_close(t.scale, s["scale"].as_f64().unwrap(), 1e-9, "scale");
                    assert_close(t.mirror, s["mirror"].as_f64().unwrap(), 1e-9, "mirror");
                    assert_close(t.phase, s["phase"].as_f64().unwrap(), 1e-9, "phase");
                    assert_close(t.want, s["want"].as_f64().unwrap(), 1e-9, "want");
                }

                let exp_tr = exp["turrets"].as_array().unwrap();
                assert_eq!(rs.turrets.len(), exp_tr.len(), "{key} turrets len");
                for (i, e) in exp_tr.iter().enumerate() {
                    assert_close(rs.turrets[i].x, e["x"].as_f64().unwrap(), 1e-6, "turret.x");
                    assert_close(rs.turrets[i].y, e["y"].as_f64().unwrap(), 1e-6, "turret.y");
                    assert_eq!(rs.turrets[i].team, e["team"].as_u64().unwrap() as usize);
                    assert_close(rs.turrets[i].phase, e["phase"].as_f64().unwrap(), 1e-9, "turret.phase");
                }

                let exp_w = exp["wards"].as_array().unwrap();
                assert_eq!(rs.wards.len(), exp_w.len(), "{key} wards len");
                for (i, e) in exp_w.iter().enumerate() {
                    assert_close(rs.wards[i].x, e["x"].as_f64().unwrap(), 1e-6, "ward.x");
                    assert_close(rs.wards[i].y, e["y"].as_f64().unwrap(), 1e-6, "ward.y");
                    assert_eq!(rs.wards[i].variant, e["variant"].as_u64().unwrap() as usize);
                }

                let exp_sp = exp["wisps"].as_array().unwrap();
                assert_eq!(rs.wisps.len(), exp_sp.len(), "{key} wisps len");
                for (i, e) in exp_sp.iter().enumerate() {
                    assert_close(rs.wisps[i].bx, e["bx"].as_f64().unwrap(), 1e-6, "wisp.bx");
                    assert_close(rs.wisps[i].by, e["by"].as_f64().unwrap(), 1e-6, "wisp.by");
                    assert_close(rs.wisps[i].size, e["size"].as_f64().unwrap(), 1e-6, "wisp.size");
                }

                let exp_h = exp["heroes"].as_array().unwrap();
                assert_eq!(rs.heroes.len(), exp_h.len(), "{key} heroes len");
                for (i, e) in exp_h.iter().enumerate() {
                    assert_eq!(rs.heroes[i].col, e["col"].as_u64().unwrap() as usize);
                    assert_eq!(rs.heroes[i].is_poro, e["isPoro"].as_bool().unwrap());
                    assert_close(rs.heroes[i].x, e["x"].as_f64().unwrap(), 1e-6, "hero.x");
                    assert_close(rs.heroes[i].y, e["y"].as_f64().unwrap(), 1e-6, "hero.y");
                    assert_close(rs.heroes[i].tx, e["tx"].as_f64().unwrap(), 1e-6, "hero.tx");
                    assert_close(rs.heroes[i].ty, e["ty"].as_f64().unwrap(), 1e-6, "hero.ty");
                }
            }
        }

        // Case 3: riftStepSimulation
        {
            let c = &lines[2];
            assert_eq!(c["fn"], "riftStepSimulation");
            let mut rs = RiftState::new(1280.0, 720.0);
            rs.wisps.clear();
            rs.heroes.clear();
            rs.icons = sample_icons();
            rs.refresh();
            rs.rebuild_props();
            rs.trees[0].grow = 1.0;
            rs.trees[0].want = 0.0;
            rs.trees[3].grow = 0.0;
            rs.trees[3].want = 1.0;

            let dt = c["input"]["dt"].as_f64().unwrap();
            let steps = c["input"]["steps"].as_u64().unwrap() as usize;
            let trace = c["output"]["trace"].as_array().unwrap();
            let mut ti = 0usize;
            let mut t_sec = 0.0;
            for s in 0..steps {
                t_sec += dt;
                rs.step_props(dt, t_sec);
                if ti < trace.len() && trace[ti]["step"].as_u64().unwrap() as usize == s {
                    let tr = &trace[ti];
                    assert_close(rs.trees[0].grow, tr["t0Grow"].as_f64().unwrap(), 1e-9, "t0Grow");
                    assert_close(rs.trees[3].grow, tr["t3Grow"].as_f64().unwrap(), 1e-9, "t3Grow");
                    assert_close(rs.wisps[0].x, tr["wisp0"][0].as_f64().unwrap(), 1e-6, "wisp0.x");
                    assert_close(rs.wisps[0].y, tr["wisp0"][1].as_f64().unwrap(), 1e-6, "wisp0.y");
                    assert_close(rs.heroes[0].x, tr["hero0"][0].as_f64().unwrap(), 1e-6, "hero0.x");
                    assert_close(rs.heroes[0].y, tr["hero0"][1].as_f64().unwrap(), 1e-6, "hero0.y");
                    assert_close(rs.heroes[0].wait, tr["hero0"][2].as_f64().unwrap(), 1e-9, "hero0.wait");
                    assert_close(rs.heroes[0].walk_t, tr["hero0"][3].as_f64().unwrap(), 1e-6, "hero0.walkT");
                    assert_close(rs.heroes[2].x, tr["hero2"][0].as_f64().unwrap(), 1e-6, "hero2.x");
                    assert_close(rs.heroes[2].y, tr["hero2"][1].as_f64().unwrap(), 1e-6, "hero2.y");
                    ti += 1;
                }
            }
            assert_eq!(ti, trace.len());
        }
    }
}
