pub mod shaders;

use crate::beach::sample_arr;
use crate::gl::*;
use crate::meadow::{fbm, make_noise_image, IconBox, STEP};
use crate::state::{AppState, Rect};
use shaders::*;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::ffi::c_void;

// =========================================================================
// Constants & Deterministic Hash (1:1 prototype/strike.html + strike-props.js)
// =========================================================================
pub const STRIKE_COLS: usize = 8;
pub const STRIKE_ROWS: usize = 3;
pub const STRIKE_CELL: usize = 128;
pub const STRIKE_MORPH_SEC: f64 = 1.2;
pub const STRIKE_MORPH_EMPTY_SEC: f64 = 1.6;

#[inline]
pub fn strike_hash(seed: i32, a: i32, b: i32, k: i32) -> f64 {
    let mut h = a.wrapping_add(11).wrapping_mul(374_761_393)
        ^ b.wrapping_add(29).wrapping_mul(668_265_263)
        ^ seed
            .wrapping_add(k.wrapping_mul(19))
            .wrapping_mul(2_246_822_519u32 as i32);
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    let u = (h as u32) ^ ((h as u32) >> 16);
    u as f64 / 4_294_967_296.0
}

// =========================================================================
// Parameters & Entities
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct StrikeParams {
    pub sigma: f64,
    pub warp: f64,
    pub holes: f64,
    pub smoke: f64,
    pub wind: f64,
    pub ball: usize,
    pub chickens: usize,
    pub crates: f64,
    pub adaptive: bool,
    pub seed: i32,
    pub debug: bool,
}

impl Default for StrikeParams {
    fn default() -> Self {
        Self {
            sigma: 82.0,
            warp: 20.0,
            holes: 1.0,
            smoke: 0.45,
            wind: 1.0,
            ball: 1,
            chickens: 4,
            crates: 1.0,
            adaptive: true,
            seed: 42,
            debug: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct StrikeCrate {
    pub x: f64,
    pub y: f64,
    pub col: usize,
    pub size: f64,
    pub rot: f64,
    pub phase: f64,
    pub grow: f64,
    pub want: f64,
    pub is_case: bool,
}

#[derive(Clone, Copy)]
pub struct StrikeDecal {
    pub x: f64,
    pub y: f64,
    pub row: usize,
    pub col: usize,
    pub size: f64,
    pub rot: f64,
    pub alpha: f64,
}

#[derive(Clone, Copy)]
pub struct StrikeCanister {
    pub x: f64,
    pub y: f64,
    pub rot: f64,
    pub size: f64,
}

#[derive(Clone, Copy)]
pub struct StrikeSmokePuff {
    pub bx: f64,
    pub by: f64,
    pub x: f64,
    pub y: f64,
    pub col: usize,
    pub size: f64,
    pub spd: f64,
    pub phase: f64,
    pub rx: f64,
    pub ry: f64,
}

#[derive(Clone, Copy)]
pub struct StrikeChicken {
    pub idx: usize,
    pub base_col: usize,
    pub x: f64,
    pub y: f64,
    pub tx: f64,
    pub ty: f64,
    pub flip: f64,
    pub wait: f64,
    pub panic_t: f64,
    pub step_count: i32,
    pub walk_t: f64,
}

#[derive(Clone, Copy)]
pub struct StrikeBall {
    pub idx: usize,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub r: f64,
    pub rot: f64,
    pub spin: f64,
}

#[derive(Clone, Copy)]
pub struct StrikeImpactBurst {
    pub x: f64,
    pub y: f64,
    pub age: f64,
    pub dur: f64,
    pub size: f64,
    pub rot: f64,
}

// =========================================================================
// StrikeState (CPU smooth-min SDF + crates, decals, smoke, balls & chickens)
// =========================================================================
pub struct StrikeState {
    pub params: StrikeParams,
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
    pub prev_icon_pos: Vec<(f64, f64)>,
    pub crates: Vec<StrikeCrate>,
    pub canisters: Vec<StrikeCanister>,
    pub site_decals: Vec<StrikeDecal>,
    pub bullet_decals: Vec<StrikeDecal>,
    pub smoke_puffs: Vec<StrikeSmokePuff>,
    pub chickens: Vec<StrikeChicken>,
    pub balls: Vec<StrikeBall>,
    pub impact_bursts: Vec<StrikeImpactBurst>,
    last_ptr_x: f64,
    last_ptr_y: f64,
    last_ptr_t: f64,
}

impl Default for StrikeState {
    fn default() -> Self {
        Self::new(1280.0, 720.0)
    }
}

impl StrikeState {
    pub fn new(w: f64, h: f64) -> Self {
        let mut s = Self {
            params: StrikeParams::default(),
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
            morph_dur: STRIKE_MORPH_SEC,
            cur_mix: 1.0,
            time_sec: 0.0,
            icons: Vec::new(),
            prev_icon_pos: Vec::new(),
            crates: Vec::new(),
            canisters: Vec::new(),
            site_decals: Vec::new(),
            bullet_decals: Vec::new(),
            smoke_puffs: Vec::new(),
            chickens: Vec::new(),
            balls: Vec::new(),
            impact_bursts: Vec::new(),
            last_ptr_x: 0.0,
            last_ptr_y: 0.0,
            last_ptr_t: -1.0,
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
                let x = gx as f64 * STEP;
                let y = gy as f64 * STEP;
                let i = gy * self.gw + gx;
                self.nz_wx[i] = ((fbm(x / 160.0, y / 160.0, 11, seed) - 0.5) * 2.0) as f32;
                self.nz_wy[i] = ((fbm(x / 160.0, y / 160.0, 29, seed) - 0.5) * 2.0) as f32;
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
        if rects.is_empty() {
            return vec![999.0f32; n];
        }
        let mut s = vec![0.0f32; n];
        let sig = self.params.sigma;
        let k = sig * 0.4;
        let w = self.params.warp;
        let max_d = sig * 1.9 + w + k * 3.0;
        for gy in 0..self.gh {
            let py = gy as f64 * STEP;
            for gx in 0..self.gw {
                let px = gx as f64 * STEP;
                let i = gy * self.gw + gx;
                let sx = px + self.nz_wx[i] as f64 * w;
                let sy = py + self.nz_wy[i] as f64 * w;
                let mut sum = 0.0f64;
                let mut dmin = 1e9f64;
                for r in rects {
                    let dx = ((sx - r.cx).abs() - r.hw).max(0.0);
                    let dy = ((sy - r.cy).abs() - r.hh).max(0.0);
                    let d = dx.hypot(dy);
                    if d < dmin {
                        dmin = d;
                    }
                    if d < max_d {
                        sum += (-(d - sig) / k).exp();
                    }
                }
                s[i] = if sum > 0.0 {
                    (-k * sum.ln()) as f32
                } else {
                    (dmin - sig).max(60.0) as f32
                };
            }
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

    fn point_in_icon(&self, px: f64, py: f64, pad: f64) -> bool {
        self.icons.iter().any(|ic| {
            px >= ic.x as f64 - pad
                && px <= (ic.x + ic.w) as f64 + pad
                && py >= ic.y as f64 - pad
                && py <= (ic.y + ic.h) as f64 + pad
        })
    }

    fn distance_to_icons(&self, px: f64, py: f64) -> f64 {
        if self.icons.is_empty() {
            return 999.0;
        }
        let mut best = 999.0f64;
        for ic in &self.icons {
            let dx = (ic.x as f64 - px).max(0.0).max(px - (ic.x + ic.w) as f64);
            let dy = (ic.y as f64 - py).max(0.0).max(py - (ic.y + ic.h) as f64);
            let d = dx.hypot(dy);
            if d < best {
                best = d;
            }
        }
        best
    }

    fn is_clear_for_prop(&self, px: f64, py: f64, min_dist: f64) -> bool {
        self.land_s(px, py) > min_dist && self.distance_to_icons(px, py) > 32.0
    }

    fn find_open_spot(&self, idx: i32, salt: i32) -> (f64, f64) {
        let seed = self.params.seed;
        let mut best = (self.w * 0.55, self.h * 0.55);
        let mut best_score = -1e9f64;
        let span_x = (self.w - 112.0).max(80.0);
        let span_y = (self.h - 116.0).max(80.0);
        for t in 0..24 {
            let px = 56.0 + strike_hash(seed, idx + t * 3, salt, 61) * span_x;
            let py = 56.0 + strike_hash(seed, idx + t * 5, salt, 62) * span_y;
            let d_icons = self.distance_to_icons(px, py);
            let s_val = self.land_s(px, py);
            let score = d_icons.min(140.0) + s_val.min(80.0) * 0.5;
            if score > best_score {
                best_score = score;
                best = (px, py);
            }
        }
        best
    }

    pub fn rebuild_props(&mut self) {
        let seed = self.params.seed;
        self.crates.clear();
        self.canisters.clear();
        self.site_decals.clear();
        self.smoke_puffs.clear();

        // Site A & Site B stencil markings
        self.site_decals.push(StrikeDecal {
            x: self.w * (0.24 + 0.08 * strike_hash(seed, 1, 2, 1)),
            y: self.h * (0.28 + 0.10 * strike_hash(seed, 2, 3, 2)),
            row: 1,
            col: 3,
            size: 86.0,
            rot: -0.15 + strike_hash(seed, 3, 4, 3) * 0.28,
            alpha: 0.72,
        });
        self.site_decals.push(StrikeDecal {
            x: self.w * (0.72 + 0.08 * strike_hash(seed, 4, 5, 4)),
            y: self.h * (0.64 + 0.08 * strike_hash(seed, 5, 6, 5)),
            row: 2,
            col: 6,
            size: 84.0,
            rot: 0.1 + strike_hash(seed, 6, 7, 6) * 0.25,
            alpha: 0.72,
        });

        let cell = 78.0 / self.params.crates.max(0.25).sqrt();
        let cols = (self.w / cell).ceil() as i32;
        let rows = ((self.h - 44.0).max(1.0) / cell).ceil() as i32;

        for r in 0..rows {
            for c in 0..cols {
                if strike_hash(seed, c, r, 11) > 0.62 {
                    continue;
                }
                let x = (c as f64 + 0.2 + 0.6 * strike_hash(seed, c, r, 12)) * cell;
                let y = (r as f64 + 0.2 + 0.6 * strike_hash(seed, c, r, 13)) * cell;
                if x < 34.0 || x > self.w - 34.0 || y < 34.0 || y > self.h - 72.0 {
                    continue;
                }
                let start = self.crates.len().saturating_sub(16);
                if self.crates[start..]
                    .iter()
                    .any(|prev| (prev.x - x).hypot(prev.y - y) < 46.0)
                {
                    continue;
                }

                let rv = strike_hash(seed, c, r, 14);
                let mut size = 44.0 + strike_hash(seed, c, r, 15) * 18.0;
                let sprite_col = if rv < 0.28 {
                    0
                } else if rv < 0.48 {
                    size += 10.0;
                    1
                } else if rv < 0.66 {
                    size = 38.0 + strike_hash(seed, c, r, 16) * 10.0;
                    2
                } else if rv < 0.82 {
                    size = 54.0 + strike_hash(seed, c, r, 17) * 14.0;
                    3
                } else if rv < 0.92 {
                    size = 48.0 + strike_hash(seed, c, r, 18) * 12.0;
                    5
                } else {
                    size = 40.0;
                    4
                };

                let ok = if self.is_clear_for_prop(x, y, 26.0) {
                    1.0
                } else {
                    0.0
                };
                self.crates.push(StrikeCrate {
                    x,
                    y,
                    col: sprite_col,
                    size,
                    rot: (strike_hash(seed, c, r, 19) - 0.5) * 1.15,
                    phase: strike_hash(seed, c, r, 20) * TAU,
                    grow: ok,
                    want: ok,
                    is_case: sprite_col == 4,
                });
            }
        }

        let smoke_spots = ((6.0 + self.params.smoke * 12.0).round() as i32).clamp(4, 18) as usize;
        for i in 0..smoke_spots {
            let ii = i as i32;
            let sx = self.w * (0.08 + 0.84 * strike_hash(seed, ii, 71, 31));
            let sy = self.h * (0.08 + 0.78 * strike_hash(seed, ii, 72, 32));
            if i < 5 {
                self.canisters.push(StrikeCanister {
                    x: sx,
                    y: sy,
                    rot: strike_hash(seed, ii, 73, 33) * TAU,
                    size: 30.0,
                });
            }
            for k in 0..2 {
                self.smoke_puffs.push(StrikeSmokePuff {
                    bx: sx + (strike_hash(seed, ii, k, 34) - 0.5) * 56.0,
                    by: sy + (strike_hash(seed, ii, k, 35) - 0.5) * 44.0,
                    x: sx,
                    y: sy,
                    col: if (i + k as usize) % 2 == 0 { 2 } else { 3 },
                    size: 92.0 + strike_hash(seed, ii, k, 36) * 68.0,
                    spd: 0.25 + strike_hash(seed, ii, k, 37) * 0.45,
                    phase: strike_hash(seed, ii, k, 38) * TAU,
                    rx: 28.0 + strike_hash(seed, ii, k, 39) * 34.0,
                    ry: 16.0 + strike_hash(seed, ii, k, 40) * 20.0,
                });
            }
        }

        self.sync_actors(true);
    }

    pub fn sync_bullet_decals(&mut self, trigger_impacts: bool) {
        self.bullet_decals.clear();
        let density = self.params.holes;
        let seed = self.params.seed;
        if self.icons.is_empty() || density <= 0.02 {
            self.prev_icon_pos = self.icons.iter().map(|ic| (ic.x as f64, ic.y as f64)).collect();
            return;
        }

        if trigger_impacts && !self.prev_icon_pos.is_empty() {
            for (i, ic) in self.icons.iter().enumerate() {
                let ix = ic.x as f64;
                let iy = ic.y as f64;
                let matched = self
                    .prev_icon_pos
                    .iter()
                    .any(|&(ox, oy)| (ix - ox).hypot(iy - oy) < 16.0);
                if !matched {
                    self.impact_bursts.push(StrikeImpactBurst {
                        x: ix + ic.w as f64 * 0.5,
                        y: iy + ic.h as f64 * 0.45,
                        age: 0.0,
                        dur: 0.48,
                        size: 74.0,
                        rot: strike_hash(seed, i as i32, ix.round() as i32, 91) * TAU,
                    });
                }
            }
        }
        self.prev_icon_pos = self.icons.iter().map(|ic| (ic.x as f64, ic.y as f64)).collect();

        let per_icon = ((3.5 * density).round() as i32).clamp(1, 7) as usize;
        let max_rad = (self.params.sigma * 0.58).max(22.0);
        for (i, ic) in self.icons.iter().enumerate() {
            let ix = ic.x as f64;
            let iy = ic.y as f64;
            let cx = ix + ic.w as f64 * 0.5;
            let cy = iy + ic.h as f64 * 0.44;
            let rx_key = (ix * 0.25).round() as i32;
            let ry_key = (iy * 0.25).round() as i32;
            for k in 0..per_icon {
                let ii = i as i32;
                let kk = k as i32;
                let h1 = strike_hash(seed, ii * 13 + kk, rx_key, 51);
                let h2 = strike_hash(seed, ii * 17 + kk, ry_key, 52);
                let h3 = strike_hash(seed, ii * 23 + kk, kk, 53);
                let ang = (k as f64 / per_icon as f64) * TAU + h1 * 1.4;
                let rad = 18.0 + h2 * max_rad;
                let dx = cx + ang.cos() * rad;
                let dy = cy + ang.sin() * rad * 0.88;
                if dx < 8.0 || dx > self.w - 8.0 || dy < 8.0 || dy > self.h - 40.0 {
                    continue;
                }
                if k == per_icon - 1 && h3 < 0.75 {
                    self.bullet_decals.push(StrikeDecal {
                        x: dx,
                        y: dy,
                        row: 0,
                        col: 7,
                        size: 28.0 + h1 * 8.0,
                        rot: h3 * TAU,
                        alpha: 0.92,
                    });
                } else {
                    let d_col = if h3 < 0.46 {
                        0
                    } else if h3 < 0.8 {
                        1
                    } else {
                        2
                    };
                    self.bullet_decals.push(StrikeDecal {
                        x: dx,
                        y: dy,
                        row: 1,
                        col: d_col,
                        size: 32.0 + h1 * 16.0,
                        rot: h2 * TAU,
                        alpha: 0.86,
                    });
                }
            }
        }
    }

    fn kick_ball_away(&mut self, b_idx: usize, from_x: f64, from_y: f64, extra_speed: f64) {
        let seed = self.params.seed;
        let b = self.balls[b_idx];
        let mut best_dir_x = b.x - from_x;
        let mut best_dir_y = b.y - from_y;
        let raw_len = best_dir_x.hypot(best_dir_y);
        if raw_len < 2.0 {
            let ang = strike_hash(seed, b.idx as i32, (b.x + b.y).round() as i32, 77) * TAU;
            best_dir_x = ang.cos();
            best_dir_y = ang.sin();
        } else {
            best_dir_x /= raw_len;
            best_dir_y /= raw_len;
        }

        let mut best_score = -1e9f64;
        let base_ang = best_dir_y.atan2(best_dir_x);
        let init_x = best_dir_x;
        let init_y = best_dir_y;
        for i in 0..12 {
            let a = base_ang + (i as f64 / 12.0) * TAU;
            let dx = a.cos();
            let dy = a.sin();
            let tx = b.x + dx * 135.0;
            let ty = b.y + dy * 135.0;
            if tx < 42.0 || tx > self.w - 42.0 || ty < 42.0 || ty > self.h - 82.0 {
                continue;
            }
            let d_icon = self.distance_to_icons(tx, ty);
            // Note: JS uses live bestDirX/bestDirY in alignment calculation
            let alignment = dx * best_dir_x + dy * best_dir_y;
            let score = d_icon + alignment * 28.0;
            if score > best_score {
                best_score = score;
                best_dir_x = dx;
                best_dir_y = dy;
            }
        }
        let _ = (init_x, init_y);

        let base_spd = if extra_speed > 0.0 { extra_speed } else { 330.0 };
        let speed = base_spd + strike_hash(seed, b.idx as i32, from_x.round() as i32, 79) * 95.0;
        let nb = &mut self.balls[b_idx];
        nb.vx = best_dir_x * speed;
        nb.vy = best_dir_y * speed;
        nb.vz = 155.0 + strike_hash(seed, b.idx as i32, from_y.round() as i32, 80) * 65.0;
        nb.spin = (if nb.vx >= 0.0 { 1.0 } else { -1.0 }) * (4.5 + speed * 0.015);
        nb.x = (nb.x + best_dir_x * 14.0).clamp(36.0, (self.w - 36.0).max(36.0));
        nb.y = (nb.y + best_dir_y * 14.0).clamp(36.0, (self.h - 72.0).max(36.0));
        let bx = nb.x;
        let by = nb.y;

        self.impact_bursts.push(StrikeImpactBurst {
            x: bx,
            y: by,
            age: 0.0,
            dur: 0.36,
            size: 54.0,
            rot: best_dir_y.atan2(best_dir_x),
        });
    }

    fn check_ball_icon_overlap(&mut self, allow_teleport_fallback: bool) {
        if self.icons.is_empty() {
            return;
        }
        let seed = self.params.seed;
        let icons = self.icons.clone();
        for bi in 0..self.balls.len() {
            let pad = self.balls[bi].r + 14.0;
            for ic in &icons {
                let bx = self.balls[bi].x;
                let by = self.balls[bi].y;
                let cx = ic.x as f64 + ic.w as f64 * 0.5;
                let cy = ic.y as f64 + ic.h as f64 * 0.5;
                let nx = bx.clamp(ic.x as f64, (ic.x + ic.w) as f64);
                let ny = by.clamp(ic.y as f64, (ic.y + ic.h) as f64);
                if (bx - nx).hypot(by - ny) < pad {
                    self.kick_ball_away(bi, cx, cy, 340.0);
                    let r_pad = self.balls[bi].r + 6.0;
                    if allow_teleport_fallback
                        && self.point_in_icon(self.balls[bi].x, self.balls[bi].y, r_pad)
                    {
                        let free = self.find_open_spot(
                            self.balls[bi].idx as i32 + 3,
                            (ic.x as f64 + ic.y as f64).round() as i32,
                        );
                        let b = &mut self.balls[bi];
                        let fdx = free.0 - b.x;
                        let fdy = free.1 - b.y;
                        let fd = fdx.hypot(fdy).max(1.0);
                        let kick_spd =
                            380.0 + strike_hash(seed, b.idx as i32, (ic.x as f64).round() as i32, 81) * 70.0;
                        b.x = (b.x + (fdx / fd) * (ic.w as f64 * 0.65 + b.r + 12.0))
                            .clamp(36.0, (self.w - 36.0).max(36.0));
                        b.y = (b.y + (fdy / fd) * (ic.h as f64 * 0.65 + b.r + 12.0))
                            .clamp(36.0, (self.h - 72.0).max(36.0));
                        b.vx = (fdx / fd) * kick_spd;
                        b.vy = (fdy / fd) * kick_spd;
                        b.vz = 195.0;
                        b.spin = if b.vx >= 0.0 { 8.5 } else { -8.5 };
                    }
                    break;
                }
            }
        }
    }

    pub fn sync_actors(&mut self, reset_all: bool) {
        let seed = self.params.seed;
        let want_balls = self.params.ball.min(3);
        if reset_all {
            self.balls.clear();
        }
        if self.balls.len() > want_balls {
            self.balls.truncate(want_balls);
        }
        while self.balls.len() < want_balls {
            let idx = self.balls.len();
            let ii = idx as i32;
            let spot = self.find_open_spot(ii, 101 + ii * 17);
            let kick_ang = strike_hash(seed, ii, 19, 83) * TAU;
            self.balls.push(StrikeBall {
                idx,
                x: spot.0,
                y: spot.1,
                z: 0.0,
                vx: kick_ang.cos() * 58.0,
                vy: kick_ang.sin() * 58.0,
                vz: 0.0,
                r: 21.0,
                rot: strike_hash(seed, ii, 20, 84) * TAU,
                spin: 2.2,
            });
        }

        let want_chickens = self.params.chickens.min(10);
        if reset_all {
            self.chickens.clear();
        }
        if self.chickens.len() > want_chickens {
            self.chickens.truncate(want_chickens);
        }
        while self.chickens.len() < want_chickens {
            let idx = self.chickens.len();
            let ii = idx as i32;
            let start = self.find_open_spot(ii + 10, 211 + ii * 23);
            let dest = self.find_open_spot(ii + 10, 212 + ii * 23);
            let variant_roll = strike_hash(seed, ii, 25, 86);
            let base_col = if variant_roll < 0.62 {
                4
            } else if variant_roll < 0.88 {
                6
            } else {
                7
            };
            self.chickens.push(StrikeChicken {
                idx,
                base_col,
                x: start.0,
                y: start.1,
                tx: dest.0,
                ty: dest.1,
                flip: if dest.0 >= start.0 { 1.0 } else { -1.0 },
                wait: 0.6 + strike_hash(seed, ii, 27, 87) * 2.0,
                panic_t: 0.0,
                step_count: ii * 19 + 3,
                walk_t: strike_hash(seed, ii, 28, 88) * 8.0,
            });
        }

        for i in 0..self.chickens.len() {
            let cx = self.chickens[i].x;
            let cy = self.chickens[i].y;
            if self.distance_to_icons(cx, cy) < 34.0 {
                let sc = self.chickens[i].step_count;
                self.chickens[i].step_count += 1;
                let safe = self.find_open_spot(self.chickens[i].idx as i32, sc);
                self.chickens[i].tx = safe.0;
                self.chickens[i].ty = safe.1;
                self.chickens[i].flip = if safe.0 >= cx { 1.0 } else { -1.0 };
                self.chickens[i].wait = 0.0;
                self.chickens[i].panic_t = 1.4;
            }
        }

        self.check_ball_icon_overlap(true);
    }

    pub fn update_targets(&mut self, snap: bool) {
        for i in 0..self.crates.len() {
            let ok = if self.is_clear_for_prop(self.crates[i].x, self.crates[i].y, 26.0) {
                1.0
            } else {
                0.0
            };
            self.crates[i].want = ok;
            if snap {
                self.crates[i].grow = ok;
            }
        }
        self.sync_bullet_decals(!snap);
        self.sync_actors(false);
    }

    pub fn pointer_kick(&mut self, px: f64, py: f64, mvx: f64, mvy: f64) -> bool {
        let mut kicked = false;
        for bi in 0..self.balls.len() {
            let dx = self.balls[bi].x - px;
            let dy = self.balls[bi].y - py;
            if dx.hypot(dy) < self.balls[bi].r + 18.0 {
                let speed = (mvx.hypot(mvy) * 1.4 + 220.0).clamp(210.0, 460.0);
                self.kick_ball_away(bi, px, py, speed);
                kicked = true;
            }
        }
        for i in 0..self.chickens.len() {
            let cx = self.chickens[i].x;
            let cy = self.chickens[i].y;
            if (cx - px).hypot(cy - py) < 42.0 {
                let sc = self.chickens[i].step_count;
                self.chickens[i].step_count += 1;
                let safe = self.find_open_spot(self.chickens[i].idx as i32, sc);
                self.chickens[i].tx = safe.0;
                self.chickens[i].ty = safe.1;
                self.chickens[i].flip = if safe.0 >= cx { 1.0 } else { -1.0 };
                self.chickens[i].wait = 0.0;
                self.chickens[i].panic_t = 1.3;
            }
        }
        kicked
    }

    pub fn handle_pointer(&mut self, is_click: bool, px: f32, py: f32) -> bool {
        let x = px as f64;
        let y = py as f64;
        if is_click {
            return self.pointer_kick(x, y, 0.0, 0.0);
        }
        let now = self.time_sec;
        if self.last_ptr_t < 0.0 {
            self.last_ptr_x = x;
            self.last_ptr_y = y;
            self.last_ptr_t = now;
            return false;
        }
        let dt = (now - self.last_ptr_t).max(0.01);
        if now - self.last_ptr_t > 0.024 {
            let mvx = (x - self.last_ptr_x) / dt * 0.35;
            let mvy = (y - self.last_ptr_y) / dt * 0.35;
            self.last_ptr_x = x;
            self.last_ptr_y = y;
            self.last_ptr_t = now;
            return self.pointer_kick(x, y, mvx, mvy);
        }
        false
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
            STRIKE_MORPH_EMPTY_SEC
        } else {
            STRIKE_MORPH_SEC
        };
        self.cur_mix = 0.0;
        self.field_version = self.field_version.wrapping_add(1);
        if self.crates.is_empty() {
            self.rebuild_props();
        }
        self.update_targets(from_empty);
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

    pub fn sync_params(&mut self, next: StrikeParams) {
        let prev = self.params;
        if prev == next {
            return;
        }
        self.params = next;
        let seed_changed = prev.seed != next.seed;
        let adaptive_changed = prev.adaptive != next.adaptive;
        let field_changed =
            (prev.sigma - next.sigma).abs() > 1e-6 || (prev.warp - next.warp).abs() > 1e-6;
        let rebuild_changed = (prev.crates - next.crates).abs() > 1e-6;
        let props_changed = (prev.holes - next.holes).abs() > 1e-6
            || (prev.smoke - next.smoke).abs() > 1e-6
            || prev.ball != next.ball
            || prev.chickens != next.chickens;

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
            if rebuild_changed {
                self.rebuild_props();
                self.sync_actors(false);
            } else if props_changed {
                self.sync_bullet_decals(false);
                self.sync_actors(false);
            }
        }
    }

    pub fn step_props(&mut self, dt: f64, time_sec: f64) {
        let rate = dt * 3.0;
        for c in &mut self.crates {
            if c.grow < c.want {
                c.grow = (c.grow + rate).min(c.want);
            } else if c.grow > c.want {
                c.grow = (c.grow - rate * 1.5).max(c.want);
            }
        }

        for i in (0..self.impact_bursts.len()).rev() {
            self.impact_bursts[i].age += dt;
            if self.impact_bursts[i].age >= self.impact_bursts[i].dur {
                self.impact_bursts.remove(i);
            }
        }

        let drift_t = time_sec * (0.28 + 0.72 * self.params.wind);
        for s in &mut self.smoke_puffs {
            s.x = s.bx + (drift_t * s.spd + s.phase).sin() * s.rx;
            s.y = s.by + (drift_t * s.spd * 0.8 + s.phase * 1.3).cos() * s.ry;
        }

        let friction = (-dt * 1.38).exp();
        let w = self.w;
        let h = self.h;
        for bi in 0..self.balls.len() {
            {
                let b = &mut self.balls[bi];
                b.x += b.vx * dt;
                b.y += b.vy * dt;
                b.z += b.vz * dt;
                b.vz -= 720.0 * dt;
                if b.z <= 0.0 {
                    b.z = 0.0;
                    if b.vz < -45.0 {
                        b.vz = -b.vz * 0.52;
                    } else {
                        b.vz = 0.0;
                    }
                }
                let speed = b.vx.hypot(b.vy);
                b.rot += (if b.vx >= 0.0 { 1.0 } else { -1.0 }) * (speed / b.r.max(12.0)) * dt
                    + b.spin * dt * 0.22;
                b.spin *= friction;
                b.vx *= friction;
                b.vy *= friction;
                if speed < 3.0 && b.z == 0.0 {
                    b.vx = 0.0;
                    b.vy = 0.0;
                }

                let min_x = b.r + 12.0;
                let max_x = (w - b.r - 12.0).max(min_x + 10.0);
                let min_y = b.r + 12.0;
                let max_y = (h - b.r - 58.0).max(min_y + 10.0);
                if b.x < min_x {
                    b.x = min_x;
                    b.vx = b.vx.abs() * 0.82;
                }
                if b.x > max_x {
                    b.x = max_x;
                    b.vx = -b.vx.abs() * 0.82;
                }
                if b.y < min_y {
                    b.y = min_y;
                    b.vy = b.vy.abs() * 0.82;
                }
                if b.y > max_y {
                    b.y = max_y;
                    b.vy = -b.vy.abs() * 0.82;
                }

                for c in &self.crates {
                    if c.want < 0.5 || c.grow < 0.6 {
                        continue;
                    }
                    let cr = c.size * 0.38 + b.r;
                    let dx = b.x - c.x;
                    let dy = b.y - c.y;
                    let d = dx.hypot(dy);
                    if d > 0.1 && d < cr {
                        let nx = dx / d;
                        let ny = dy / d;
                        b.x = c.x + nx * cr;
                        b.y = c.y + ny * cr;
                        let dot = b.vx * nx + b.vy * ny;
                        if dot < 0.0 {
                            b.vx -= 1.76 * dot * nx;
                            b.vy -= 1.76 * dot * ny;
                            b.vz = b.vz.max(65.0);
                        }
                    }
                }
            }

            // Bounce / kick away from icons
            let mut hit_center: Option<(f64, f64)> = None;
            let bx = self.balls[bi].x;
            let by = self.balls[bi].y;
            let br = self.balls[bi].r;
            for ic in &self.icons {
                let nx = bx.clamp(ic.x as f64 - 4.0, (ic.x + ic.w) as f64 + 4.0);
                let ny = by.clamp(ic.y as f64 - 4.0, (ic.y + ic.h) as f64 + 4.0);
                if (bx - nx).hypot(by - ny) < br + 6.0 {
                    hit_center = Some((
                        ic.x as f64 + ic.w as f64 * 0.5,
                        ic.y as f64 + ic.h as f64 * 0.5,
                    ));
                    break;
                }
            }
            if let Some((cx, cy)) = hit_center {
                self.kick_ball_away(bi, cx, cy, 310.0);
            }
        }

        // Ball-to-ball collision
        for i in 0..self.balls.len() {
            for j in (i + 1)..self.balls.len() {
                let dx = self.balls[j].x - self.balls[i].x;
                let dy = self.balls[j].y - self.balls[i].y;
                let d = dx.hypot(dy);
                let min_d = self.balls[i].r + self.balls[j].r;
                if d > 0.1 && d < min_d {
                    let nx = dx / d;
                    let ny = dy / d;
                    let overlap = (min_d - d) * 0.5;
                    self.balls[i].x -= nx * overlap;
                    self.balls[i].y -= ny * overlap;
                    self.balls[j].x += nx * overlap;
                    self.balls[j].y += ny * overlap;
                    let dv = (self.balls[i].vx - self.balls[j].vx) * nx
                        + (self.balls[i].vy - self.balls[j].vy) * ny;
                    if dv > 0.0 {
                        self.balls[i].vx -= nx * dv * 0.85;
                        self.balls[i].vy -= ny * dv * 0.85;
                        self.balls[j].vx += nx * dv * 0.85;
                        self.balls[j].vy += ny * dv * 0.85;
                    }
                }
            }
        }

        // Chicken patrol + panic scurry when a ball rolls nearby
        let ch_speed = 28.0 * (0.55 + 0.45 * self.params.wind);
        let seed = self.params.seed;
        for i in 0..self.chickens.len() {
            if self.chickens[i].panic_t > 0.0 {
                self.chickens[i].panic_t = (self.chickens[i].panic_t - dt).max(0.0);
            }
            let cx = self.chickens[i].x;
            let cy = self.chickens[i].y;
            let startled = self.chickens[i].panic_t <= 0.1
                && self
                    .balls
                    .iter()
                    .any(|b| (cx - b.x).hypot(cy - b.y) < 48.0 && b.vx.hypot(b.vy) > 35.0);
            if startled {
                let sc = self.chickens[i].step_count;
                self.chickens[i].step_count += 1;
                let safe = self.find_open_spot(self.chickens[i].idx as i32, sc);
                self.chickens[i].tx = safe.0;
                self.chickens[i].ty = safe.1;
                self.chickens[i].flip = if safe.0 >= cx { 1.0 } else { -1.0 };
                self.chickens[i].wait = 0.0;
                self.chickens[i].panic_t = 1.4;
            }

            if self.chickens[i].wait > 0.0 {
                self.chickens[i].wait -= dt;
                self.chickens[i].walk_t = 0.0;
                if self.chickens[i].wait <= 0.0 {
                    let sc = self.chickens[i].step_count;
                    self.chickens[i].step_count += 1;
                    let next = self.find_open_spot(self.chickens[i].idx as i32, sc);
                    self.chickens[i].tx = next.0;
                    self.chickens[i].ty = next.1;
                    if (next.0 - self.chickens[i].x).abs() > 2.0 {
                        self.chickens[i].flip = if next.0 >= self.chickens[i].x { 1.0 } else { -1.0 };
                    }
                }
            } else {
                let ch = &mut self.chickens[i];
                let dx = ch.tx - ch.x;
                let dy = ch.ty - ch.y;
                let dist = dx.hypot(dy);
                let step = (if ch.panic_t > 0.0 {
                    ch_speed * 3.1
                } else {
                    ch_speed
                }) * dt;
                if dist <= step + 1.0 {
                    ch.x = ch.tx;
                    ch.y = ch.ty;
                    ch.wait = 1.4 + strike_hash(seed, ch.idx as i32, ch.step_count, 93) * 2.6;
                    ch.walk_t = 0.0;
                } else {
                    ch.x += (dx / dist) * step;
                    ch.y += (dy / dist) * step;
                    ch.walk_t += dt * if ch.panic_t > 0.0 { 15.0 } else { 7.5 };
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
// Procedural 2x Painter Atlas (8x3 cells -> 2048x768 RGBA8)
// 1:1 port of prototype/strike-props.js
// =========================================================================
fn hex_rgba(hex: u32, a: f64) -> [f64; 4] {
    [
        ((hex >> 16) & 0xff) as f64 / 255.0,
        ((hex >> 8) & 0xff) as f64 / 255.0,
        (hex & 0xff) as f64 / 255.0,
        a,
    ]
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

fn round_rect_pts(x: f64, y: f64, w: f64, h: f64, r: f64) -> Vec<(f64, f64)> {
    let r = r.min(w * 0.5).min(h * 0.5);
    let mut pts = Vec::with_capacity(32);
    let corners = [
        (x + w - r, y + r, -FRAC_PI_2, 0.0),
        (x + w - r, y + h - r, 0.0, FRAC_PI_2),
        (x + r, y + h - r, FRAC_PI_2, PI),
        (x + r, y + r, PI, PI + FRAC_PI_2),
    ];
    for (cx, cy, a0, a1) in corners {
        for i in 0..=6 {
            let a = a0 + (a1 - a0) * (i as f64 / 6.0);
            pts.push((cx + a.cos() * r, cy + a.sin() * r));
        }
    }
    pts
}

fn transform_pts(pts: &[(f64, f64)], cx: f64, cy: f64, rot: f64) -> Vec<(f64, f64)> {
    pts.iter()
        .map(|&(lx, ly)| rot_local(cx, cy, rot, lx, ly))
        .collect()
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

struct StrikeCanvas {
    w: usize,
    h: usize,
    scale: f64,
    px: Vec<u8>,
    clip_circle: Option<(f64, f64, f64)>,
}

impl StrikeCanvas {
    fn new(logical_w: usize, logical_h: usize, scale: usize) -> Self {
        let w = logical_w * scale;
        let h = logical_h * scale;
        Self {
            w,
            h,
            scale: scale as f64,
            px: vec![0u8; w * h * 4],
            clip_circle: None,
        }
    }

    #[inline]
    fn blend_px(&mut self, x: usize, y: usize, r: f64, g: f64, b: f64, mut a: f64) {
        if a <= 0.001 || x >= self.w || y >= self.h {
            return;
        }
        if let Some((ccx, ccy, cr)) = self.clip_circle {
            let lx = (x as f64 + 0.5) / self.scale;
            let ly = (y as f64 + 0.5) / self.scale;
            let d = (lx - ccx).hypot(ly - ccy);
            let cov = ((cr - d) * self.scale + 0.5).clamp(0.0, 1.0);
            if cov <= 0.001 {
                return;
            }
            a *= cov;
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
        let pts = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
        self.fill_polygon_fn(&pts, |_, _| rgba);
    }

    fn fill_ellipse_fn<F>(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, color_fn: F)
    where
        F: Fn(f64, f64) -> [f64; 4],
    {
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
            let ly_w = (py as f64 + 0.5) / s;
            for px in ix0..ix1 {
                let lx_w = (px as f64 + 0.5) / s;
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
                        let col = color_fn(lx_w, ly_w);
                        self.blend_px(px, py, col[0], col[1], col[2], col[3] * cov);
                    }
                }
            }
        }
    }

    fn fill_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, rgba: [f64; 4]) {
        self.fill_ellipse_fn(cx, cy, rx, ry, rot, |_, _| rgba);
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
        let gdx = gx1 - gx0;
        let gdy = gy1 - gy0;
        let dr = r1 - r0;
        let qa = gdx * gdx + gdy * gdy - dr * dr;
        self.fill_ellipse_fn(cx, cy, rx, ry, 0.0, |x, y| {
            let px = x - gx0;
            let py = y - gy0;
            let qb = -(px * gdx + py * gdy + r0 * dr);
            let qc = px * px + py * py - r0 * r0;
            let disc = (qb * qb - qa * qc).max(0.0);
            let t = if qa.abs() > 1e-9 {
                (-qb - disc.sqrt()) / qa
            } else if qb.abs() > 1e-9 {
                -qc / (2.0 * qb)
            } else {
                0.0
            };
            eval_stops_rgba(stops, t)
        });
    }

    fn fill_polygon_fn<F>(&mut self, pts: &[(f64, f64)], color_fn: F)
    where
        F: Fn(f64, f64) -> [f64; 4],
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
                let col = color_fn(lx, ly);
                self.blend_px(px, py, col[0], col[1], col[2], col[3] * cov);
            }
        }
    }

    fn fill_polygon_linear(
        &mut self,
        pts: &[(f64, f64)],
        gx0: f64,
        gy0: f64,
        gx1: f64,
        gy1: f64,
        stops: &[(f64, [f64; 4])],
    ) {
        let gdx = gx1 - gx0;
        let gdy = gy1 - gy0;
        let glen2 = (gdx * gdx + gdy * gdy).max(1e-6);
        self.fill_polygon_fn(pts, |x, y| {
            let t = ((x - gx0) * gdx + (y - gy0) * gdy) / glen2;
            eval_stops_rgba(stops, t)
        });
    }

    fn fill_polygon(&mut self, pts: &[(f64, f64)], rgba: [f64; 4]) {
        self.fill_polygon_fn(pts, |_, _| rgba);
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

    fn stroke_dashed_closed(&mut self, pts: &[(f64, f64)], line_w: f64, rgba: [f64; 4], dash: f64, gap: f64) {
        if pts.len() < 2 {
            return;
        }
        let period = dash + gap;
        let mut acc = 0.0;
        let n = pts.len();
        for i in 0..n {
            let (ax, ay) = pts[i];
            let (bx, by) = pts[(i + 1) % n];
            let seg_len = (bx - ax).hypot(by - ay);
            if seg_len < 1e-6 {
                continue;
            }
            let mut pos = 0.0;
            while pos < seg_len {
                let phase = (acc + pos) % period;
                if phase < dash {
                    let take = (dash - phase).min(seg_len - pos);
                    let t0 = pos / seg_len;
                    let t1 = (pos + take) / seg_len;
                    self.stroke_polyline(
                        &[
                            (ax + (bx - ax) * t0, ay + (by - ay) * t0),
                            (ax + (bx - ax) * t1, ay + (by - ay) * t1),
                        ],
                        line_w,
                        rgba,
                    );
                    pos += take;
                } else {
                    pos += (period - phase).min(seg_len - pos);
                }
            }
            acc += seg_len;
        }
    }

    fn stroke_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, line_w: f64, rgba: [f64; 4]) {
        let steps = 44usize;
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = (i as f64 / steps as f64) * TAU;
            pts.push(rot_local(cx, cy, rot, a.cos() * rx, a.sin() * ry));
        }
        self.stroke_polyline(&pts, line_w, rgba);
    }

    fn stroke_arc(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64, line_w: f64, rgba: [f64; 4]) {
        let steps = ((r * (a1 - a0).abs()).ceil() as usize).clamp(12, 48);
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = a0 + (a1 - a0) * (i as f64 / steps as f64);
            pts.push((cx + a.cos() * r, cy + a.sin() * r));
        }
        self.stroke_polyline(&pts, line_w, rgba);
    }
}

fn paint_crate(cv: &mut StrikeCanvas, ox: f64, oy: f64, variant: usize) {
    let cx = ox + 64.0;
    let cy = oy + 66.0;

    let sh = round_rect_pts(cx - 25.0, cy - 23.0, 68.0, 68.0, 6.0);
    cv.fill_polygon(&sh, [28.0 / 255.0, 19.0 / 255.0, 12.0 / 255.0, 0.42]);

    let w = if variant == 1 { 74.0 } else { 64.0 };
    let h = if variant == 1 { 66.0 } else { 64.0 };
    let x0 = cx - w * 0.5;
    let y0 = cy - h * 0.5;

    let body = round_rect_pts(x0, y0, w, h, 4.0);
    let wood_stops = [
        (0.0, hex_rgba(0xc29258, 1.0)),
        (0.55, hex_rgba(0xa4743f, 1.0)),
        (1.0, hex_rgba(0x7a5228, 1.0)),
    ];
    cv.fill_polygon_linear(&body, x0, y0, x0 + w, y0 + h, &wood_stops);

    // Horizontal wooden slats
    for i in 1..4 {
        let sy = y0 + (h * i as f64) / 4.0;
        cv.stroke_polyline(
            &[(x0 + 4.0, sy), (x0 + w - 4.0, sy)],
            1.6,
            [58.0 / 255.0, 34.0 / 255.0, 14.0 / 255.0, 0.55],
        );
    }

    // Diagonal bracing plank
    cv.fill_polygon(
        &[
            (x0 + 7.0, y0 + h - 12.0),
            (x0 + 13.0, y0 + h - 6.0),
            (x0 + w - 7.0, y0 + 12.0),
            (x0 + w - 13.0, y0 + 6.0),
        ],
        [176.0 / 255.0, 128.0 / 255.0, 72.0 / 255.0, 0.82],
    );

    // Outer timber frame border
    let frame = round_rect_pts(x0 + 2.5, y0 + 2.5, w - 5.0, h - 5.0, 3.0);
    cv.stroke_polygon_closed(&frame, 5.0, hex_rgba(0x68431e, 1.0));

    // Top-left bevel highlight
    cv.stroke_polyline(
        &[(x0 + 3.0, y0 + h - 4.0), (x0 + 3.0, y0 + 3.0), (x0 + w - 4.0, y0 + 3.0)],
        1.5,
        [1.0, 224.0 / 255.0, 176.0 / 255.0, 0.42],
    );

    // Steel corner brackets & rivets
    for (bx, by) in [
        (x0 + 1.0, y0 + 1.0),
        (x0 + w - 11.0, y0 + 1.0),
        (x0 + 1.0, y0 + h - 11.0),
        (x0 + w - 11.0, y0 + h - 11.0),
    ] {
        cv.fill_rect(bx, by, 10.0, 10.0, hex_rgba(0x36393c, 1.0));
        cv.fill_ellipse(bx + 5.0, by + 5.0, 1.5, 1.5, 0.0, hex_rgba(0x8b9196, 1.0));
    }

    if variant == 1 {
        let tsh = round_rect_pts(cx - 12.0, cy - 22.0, 42.0, 38.0, 4.0);
        cv.fill_polygon(&tsh, [24.0 / 255.0, 16.0 / 255.0, 10.0 / 255.0, 0.34]);
        let tarp = round_rect_pts(cx - 16.0, cy - 26.0, 40.0, 36.0, 4.0);
        cv.fill_polygon_linear(
            &tarp,
            cx - 16.0,
            cy - 26.0,
            cx + 24.0,
            cy + 12.0,
            &[(0.0, hex_rgba(0x5a6b48, 1.0)), (1.0, hex_rgba(0x39452c, 1.0))],
        );
        cv.stroke_polygon_closed(&tarp, 2.0, hex_rgba(0x28301e, 1.0));
        cv.stroke_polyline(&[(cx - 14.0, cy - 8.0), (cx + 22.0, cy - 8.0)], 2.2, hex_rgba(0xd3b27b, 1.0));
    } else {
        let chev = [42.0 / 255.0, 26.0 / 255.0, 12.0 / 255.0, 0.56];
        cv.stroke_polyline(&[(cx - 8.0, cy - 4.0), (cx, cy - 11.0), (cx + 8.0, cy - 4.0)], 2.6, chev);
        cv.stroke_polyline(&[(cx - 8.0, cy + 4.0), (cx, cy - 3.0), (cx + 8.0, cy + 4.0)], 2.6, chev);
    }
}

fn paint_oil_drum(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 66.0;
    cv.fill_ellipse(cx + 7.0, cy + 8.0, 29.0, 27.0, 0.0, [26.0 / 255.0, 18.0 / 255.0, 12.0 / 255.0, 0.44]);
    cv.fill_ellipse_radial(
        cx,
        cy,
        28.0,
        28.0,
        cx - 8.0,
        cy - 8.0,
        4.0,
        cx,
        cy,
        30.0,
        &[
            (0.0, hex_rgba(0x587856, 1.0)),
            (0.75, hex_rgba(0x3b543a, 1.0)),
            (1.0, hex_rgba(0x223122, 1.0)),
        ],
    );
    cv.stroke_ellipse(cx, cy, 23.0, 23.0, 0.0, 4.2, hex_rgba(0xd27d2d, 1.0));
    cv.fill_ellipse(cx, cy, 19.0, 19.0, 0.0, hex_rgba(0x475e45, 1.0));
    cv.stroke_arc(
        cx,
        cy,
        18.5,
        PI * 0.85,
        PI * 1.75,
        1.6,
        [220.0 / 255.0, 235.0 / 255.0, 200.0 / 255.0, 0.34],
    );
    let cap = hex_rgba(0x252a2b, 1.0);
    cv.fill_ellipse(cx - 8.0, cy - 6.0, 3.6, 3.6, 0.0, cap);
    cv.fill_ellipse(cx + 7.0, cy + 7.0, 2.4, 2.4, 0.0, cap);
}

fn paint_sandbags(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 66.0;
    cv.fill_ellipse(cx + 5.0, cy + 7.0, 44.0, 22.0, -0.15, [28.0 / 255.0, 20.0 / 255.0, 12.0 / 255.0, 0.38]);
    let bags = [
        (-22.0, 5.0, 19.0, 10.0, 0.12, 0xaa946b, 0x87724d),
        (0.0, 7.0, 20.0, 10.0, -0.05, 0xb69f74, 0x8e7953),
        (22.0, 4.0, 19.0, 10.0, -0.20, 0xa89268, 0x836e49),
        (-12.0, -5.0, 19.0, 10.0, 0.08, 0xc5af82, 0x9c865d),
        (12.0, -6.0, 19.0, 10.0, -0.12, 0xcbb68a, 0x9f8a62),
    ];
    for (bx, by, rx, ry, rot, c1, c2) in bags {
        let bcx = cx + bx;
        let bcy = cy + by;
        let g0 = rot_local(bcx, bcy, rot, -rx, -ry);
        let g1 = rot_local(bcx, bcy, rot, rx, ry);
        let gdx = g1.0 - g0.0;
        let gdy = g1.1 - g0.1;
        let glen2 = (gdx * gdx + gdy * gdy).max(1e-6);
        let stops = [(0.0, hex_rgba(c1, 1.0)), (1.0, hex_rgba(c2, 1.0))];
        cv.fill_ellipse_fn(bcx, bcy, rx, ry, rot, |x, y| {
            let t = ((x - g0.0) * gdx + (y - g0.1) * gdy) / glen2;
            eval_stops_rgba(&stops, t)
        });
        cv.stroke_ellipse(bcx, bcy, rx, ry, rot, 1.6, [54.0 / 255.0, 42.0 / 255.0, 24.0 / 255.0, 0.52]);
        let s0 = rot_local(bcx, bcy, rot, -rx * 0.65, 0.0);
        let s1 = rot_local(bcx, bcy, rot, rx * 0.65, 0.0);
        cv.stroke_polyline(&[s0, s1], 1.2, [72.0 / 255.0, 56.0 / 255.0, 32.0 / 255.0, 0.36]);
    }
}

fn paint_weapon_case(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 66.0;
    let sh = round_rect_pts(cx - 21.0, cy - 12.0, 56.0, 38.0, 5.0);
    cv.fill_polygon(&sh, [20.0 / 255.0, 15.0 / 255.0, 10.0 / 255.0, 0.46]);

    let body = round_rect_pts(cx - 27.0, cy - 18.0, 54.0, 36.0, 5.0);
    cv.fill_polygon_linear(
        &body,
        cx - 28.0,
        cy - 18.0,
        cx + 28.0,
        cy + 18.0,
        &[(0.0, hex_rgba(0x464d52, 1.0)), (1.0, hex_rgba(0x262b2f, 1.0))],
    );
    cv.stroke_polygon_closed(&body, 2.0, hex_rgba(0x181c1f, 1.0));

    for dx in [-16.0, -6.0, 6.0, 16.0] {
        cv.stroke_polyline(
            &[(cx + dx, cy - 12.0), (cx + dx, cy + 12.0)],
            2.0,
            [170.0 / 255.0, 185.0 / 255.0, 196.0 / 255.0, 0.2],
        );
    }
    let latch = hex_rgba(0xc89b3c, 1.0);
    cv.fill_rect(cx - 18.0, cy + 12.0, 8.0, 5.0, latch);
    cv.fill_rect(cx + 10.0, cy + 12.0, 8.0, 5.0, latch);
    cv.fill_ellipse(cx, cy - 2.0, 4.2, 4.2, 0.0, hex_rgba(0xff3828, 1.0));
    cv.fill_ellipse(cx - 1.0, cy - 3.0, 1.5, 1.5, 0.0, hex_rgba(0xffd2c8, 1.0));
}

fn paint_desert_planter(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 66.0;
    cv.fill_ellipse(cx + 6.0, cy + 7.0, 30.0, 26.0, 0.0, [24.0 / 255.0, 18.0 / 255.0, 12.0 / 255.0, 0.36]);
    cv.fill_ellipse(cx, cy, 24.0, 24.0, 0.0, hex_rgba(0xb4744c, 1.0));
    cv.fill_ellipse(cx, cy, 18.0, 18.0, 0.0, hex_rgba(0x5c402c, 1.0));

    let stops = [(0.0, hex_rgba(0x3a5e2f, 1.0)), (1.0, hex_rgba(0x7aa453, 1.0))];
    for i in 0..8 {
        let a = (i as f64 / 8.0) * TAU + 0.2;
        let mut loc = vec![(0.0, -4.0)];
        push_quad(&mut loc, (0.0, -4.0), (20.0, -8.0), (35.0, 0.0), 10);
        push_quad(&mut loc, (35.0, 0.0), (20.0, 8.0), (0.0, 4.0), 10);
        let pts = transform_pts(&loc, cx, cy, a);
        let g1 = rot_local(cx, cy, a, 34.0, 0.0);
        cv.fill_polygon_linear(&pts, cx, cy, g1.0, g1.1, &stops);
        cv.stroke_polygon_closed(&pts, 1.1, [24.0 / 255.0, 42.0 / 255.0, 18.0 / 255.0, 0.5]);
    }
    cv.fill_ellipse(cx, cy, 5.5, 5.5, 0.0, hex_rgba(0x99be68, 1.0));
}

fn paint_smoke_canister(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 64.0;
    let rot = -0.48;
    let sh = transform_pts(&round_rect_pts(-14.0, -4.0, 34.0, 16.0, 4.0), cx, cy, rot);
    cv.fill_polygon(&sh, [20.0 / 255.0, 15.0 / 255.0, 10.0 / 255.0, 0.38]);

    let body = transform_pts(&round_rect_pts(-16.0, -7.5, 32.0, 15.0, 4.0), cx, cy, rot);
    let g0 = rot_local(cx, cy, rot, -16.0, -8.0);
    let g1 = rot_local(cx, cy, rot, 16.0, 8.0);
    cv.fill_polygon_linear(
        &body,
        g0.0,
        g0.1,
        g1.0,
        g1.1,
        &[
            (0.0, hex_rgba(0x8d998d, 1.0)),
            (0.5, hex_rgba(0x667267, 1.0)),
            (1.0, hex_rgba(0x3e463f, 1.0)),
        ],
    );
    let stripe = transform_pts(&[(-5.0, -7.5), (3.0, -7.5), (3.0, 7.5), (-5.0, 7.5)], cx, cy, rot);
    cv.fill_polygon(&stripe, hex_rgba(0x4ea6d6, 1.0));
    let cap = transform_pts(&[(14.0, -4.5), (19.0, -4.5), (19.0, 4.5), (14.0, 4.5)], cx, cy, rot);
    cv.fill_polygon(&cap, hex_rgba(0xc8ced2, 1.0));
    let ring = rot_local(cx, cy, rot, 23.0, -8.0);
    cv.stroke_ellipse(ring.0, ring.1, 4.2, 4.2, 0.0, 1.6, hex_rgba(0xd4d9dc, 1.0));
}

fn paint_shell_casings(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let shells = [
        (52.0, 60.0, -0.55),
        (68.0, 54.0, 0.72),
        (63.0, 73.0, 0.24),
        (77.0, 67.0, -1.05),
    ];
    let stops = [
        (0.0, hex_rgba(0xf7da78, 1.0)),
        (0.45, hex_rgba(0xcfa038, 1.0)),
        (1.0, hex_rgba(0x845f1a, 1.0)),
    ];
    for (sx, sy, rot) in shells {
        let cx = ox + sx;
        let cy = oy + sy;
        let sh = transform_pts(&[(-4.5, -0.5), (8.5, -0.5), (8.5, 4.5), (-4.5, 4.5)], cx, cy, rot);
        cv.fill_polygon(&sh, [18.0 / 255.0, 12.0 / 255.0, 8.0 / 255.0, 0.36]);
        let casing = transform_pts(&round_rect_pts(-6.5, -2.6, 13.0, 5.2, 1.4), cx, cy, rot);
        let g0 = rot_local(cx, cy, rot, -6.0, -2.5);
        let g1 = rot_local(cx, cy, rot, -6.0, 2.5);
        cv.fill_polygon_linear(&casing, g0.0, g0.1, g1.0, g1.1, &stops);
        let mouth = transform_pts(&[(5.5, -1.9), (7.3, -1.9), (7.3, 1.9), (5.5, 1.9)], cx, cy, rot);
        cv.fill_polygon(&mouth, hex_rgba(0x3a2b14, 1.0));
    }
}

fn draw_single_crater(cv: &mut StrikeCanvas, cx: f64, cy: f64, r: f64, seed_off: f64) {
    cv.fill_ellipse_radial(
        cx,
        cy,
        r * 2.5,
        r * 2.5,
        cx,
        cy,
        r * 0.5,
        cx,
        cy,
        r * 2.5,
        &[
            (0.0, [18.0 / 255.0, 14.0 / 255.0, 11.0 / 255.0, 0.52]),
            (0.55, [38.0 / 255.0, 32.0 / 255.0, 26.0 / 255.0, 0.24]),
            (1.0, [38.0 / 255.0, 32.0 / 255.0, 26.0 / 255.0, 0.0]),
        ],
    );
    let crack_col = [28.0 / 255.0, 22.0 / 255.0, 18.0 / 255.0, 0.78];
    for i in 0..6 {
        let fi = i as f64;
        let a = (fi / 6.0) * TAU + (seed_off + fi * 2.3).sin() * 0.45;
        let len = r * (1.7 + 0.9 * (((fi * 37.0 + seed_off * 13.0).rem_euclid(5.0)) / 4.0));
        let lw = if i % 2 == 0 { 1.5 } else { 1.0 };
        let mid_a = a + (fi * 4.1 + seed_off).sin() * 0.22;
        cv.stroke_polyline(
            &[
                (cx + a.cos() * r * 0.85, cy + a.sin() * r * 0.85),
                (cx + mid_a.cos() * len * 0.58, cy + mid_a.sin() * len * 0.58),
                (cx + (a - 0.08).cos() * len, cy + (a - 0.08).sin() * len),
            ],
            lw,
            crack_col,
        );
    }

    let mut spall = Vec::with_capacity(15);
    for i in 0..=14 {
        let a = (i as f64 / 14.0) * TAU;
        let rad = r * (1.28 + 0.24 * (i as f64 * 3.7 + seed_off).sin());
        spall.push((cx + a.cos() * rad, cy + a.sin() * rad));
    }
    cv.fill_polygon(&spall, hex_rgba(0xb9ae9c, 1.0));
    cv.fill_ellipse(cx, cy, r * 0.92, r * 0.92, 0.0, hex_rgba(0x4c443c, 1.0));
    cv.fill_ellipse(cx - r * 0.08, cy - r * 0.08, r * 0.62, r * 0.62, 0.0, hex_rgba(0x120e0b, 1.0));
    cv.stroke_arc(
        cx,
        cy,
        r * 0.95,
        -0.35,
        PI * 0.65,
        1.4,
        [244.0 / 255.0, 232.0 / 255.0, 210.0 / 255.0, 0.58],
    );
}

fn paint_bullet_decal(cv: &mut StrikeCanvas, ox: f64, oy: f64, variant: usize) {
    match variant {
        0 => {
            draw_single_crater(cv, ox + 50.0, oy + 56.0, 7.2, 1.3);
            draw_single_crater(cv, ox + 73.0, oy + 52.0, 6.5, 4.1);
            draw_single_crater(cv, ox + 64.0, oy + 75.0, 7.8, 7.6);
        }
        1 => {
            draw_single_crater(cv, ox + 64.0, oy + 64.0, 11.5, 2.9);
            let chip = hex_rgba(0xc4b8a5, 1.0);
            for (dx, dy, s) in [(-19.0, 10.0, 3.2), (20.0, -12.0, 2.8), (13.0, 21.0, 2.6), (-16.0, -18.0, 2.4)] {
                cv.fill_ellipse(ox + 64.0 + dx, oy + 64.0 + dy, s, s, 0.0, chip);
            }
        }
        _ => {
            let p1 = rot_local(ox + 64.0, oy + 64.0, -0.42, -6.0, 0.0);
            cv.fill_ellipse(p1.0, p1.1, 26.0, 5.5, -0.42, [30.0 / 255.0, 24.0 / 255.0, 18.0 / 255.0, 0.34]);
            let p2 = rot_local(ox + 64.0, oy + 64.0, -0.42, -8.0, 0.0);
            cv.fill_ellipse(p2.0, p2.1, 22.0, 3.8, -0.42, hex_rgba(0xbfb3a0, 1.0));
            draw_single_crater(cv, ox + 78.0, oy + 56.0, 6.6, 5.4);
        }
    }
}

fn paint_site_stencil(cv: &mut StrikeCanvas, ox: f64, oy: f64, is_a: bool) {
    let cx = ox + 64.0;
    let cy = oy + 64.0;
    let rot = if is_a { -0.18 } else { 0.14 };
    let main_col = if is_a {
        [182.0 / 255.0, 54.0 / 255.0, 42.0 / 255.0, 0.72]
    } else {
        [210.0 / 255.0, 154.0 / 255.0, 46.0 / 255.0, 0.72]
    };

    let frame = transform_pts(&round_rect_pts(-34.0, -34.0, 68.0, 68.0, 8.0), cx, cy, rot);
    cv.stroke_dashed_closed(&frame, 4.2, main_col, 12.0, 5.0);

    // Bold stenciled vector letter 'A' or 'B'
    if is_a {
        let left_leg = transform_pts(&[(-18.0, 17.0), (-5.0, -17.0), (2.5, -17.0), (-8.5, 17.0)], cx, cy, rot);
        let right_leg = transform_pts(&[(18.0, 17.0), (5.0, -17.0), (-2.5, -17.0), (8.5, 17.0)], cx, cy, rot);
        let crossbar = transform_pts(&[(-10.0, 3.0), (10.0, 3.0), (12.0, 10.0), (-12.0, 10.0)], cx, cy, rot);
        cv.fill_polygon(&left_leg, main_col);
        cv.fill_polygon(&right_leg, main_col);
        cv.fill_polygon(&crossbar, main_col);
    } else {
        // Bold 'B' made of thick vertical stem + two rounded D-strokes
        let stem = transform_pts(&[(-14.0, -17.0), (-5.5, -17.0), (-5.5, 17.0), (-14.0, 17.0)], cx, cy, rot);
        cv.fill_polygon(&stem, main_col);
        let top_loop = [(-6.0, -13.5), (6.0, -13.5), (12.0, -9.5), (12.0, -4.5), (6.0, -0.5), (-6.0, -0.5)];
        let bot_loop = [(-6.0, 0.5), (7.5, 0.5), (13.5, 5.0), (13.5, 10.5), (7.5, 14.0), (-6.0, 14.0)];
        cv.stroke_polyline(&transform_pts(&top_loop, cx, cy, rot), 6.5, main_col);
        cv.stroke_polyline(&transform_pts(&bot_loop, cx, cy, rot), 6.5, main_col);
    }

    // Scuff dust streaks across stencil paint
    let scuff = [214.0 / 255.0, 188.0 / 255.0, 148.0 / 255.0, 0.42];
    cv.stroke_polyline(&transform_pts(&[(-26.0, 14.0), (28.0, -10.0)], cx, cy, rot), 2.6, scuff);
    cv.stroke_polyline(&transform_pts(&[(-18.0, -22.0), (20.0, 22.0)], cx, cy, rot), 2.6, scuff);
}

fn paint_chicken_cell(cv: &mut StrikeCanvas, ox: f64, oy: f64, variant: usize) {
    let cx = ox + 64.0;
    let base_cy = oy + 66.0;
    let flap = variant == 1;
    let body_a = hex_rgba(if variant == 2 { 0xe6c398 } else { 0xfcfaf2 }, 1.0);
    let body_b = hex_rgba(if variant == 2 { 0xa57146 } else { 0xcfc7b8 }, 1.0);
    let outline = hex_rgba(if variant == 2 { 0x59371e } else { 0x6c6458 }, 1.0);

    cv.fill_ellipse(
        cx + 3.0,
        base_cy + if flap { 10.0 } else { 5.0 },
        19.0,
        13.0,
        0.0,
        [22.0 / 255.0, 16.0 / 255.0, 10.0 / 255.0, 0.34],
    );

    let cy = base_cy + if flap { -5.0 } else { 0.0 };

    if flap {
        for dir in [-1.0, 1.0] {
            cv.fill_ellipse(cx + dir * 6.0, cy - 18.0, 12.0, 7.5, -0.45 * dir, body_a);
            cv.stroke_ellipse(cx + dir * 6.0, cy - 18.0, 12.0, 7.5, -0.45 * dir, 1.8, outline);
        }
    }

    // Tail feathers
    let tail_col = hex_rgba(if variant == 2 { 0x8f5a33 } else { 0xe4ddd0 }, 1.0);
    for (ang, len) in [(-0.3, 14.0), (0.0, 16.0), (0.28, 13.0)] {
        let p = rot_local(cx - 15.0, cy, ang, -len * 0.45, 0.0);
        cv.fill_ellipse(p.0, p.1, len * 0.55, 4.2, ang, tail_col);
        cv.stroke_ellipse(p.0, p.1, len * 0.55, 4.2, ang, 1.6, outline);
    }

    // Plump oval body
    cv.fill_ellipse_radial(
        cx - 1.0,
        cy,
        19.0,
        14.0,
        cx + 2.0,
        cy - 4.0,
        3.0,
        cx - 1.0,
        cy,
        22.0,
        &[(0.0, body_a), (1.0, body_b)],
    );
    cv.stroke_ellipse(cx - 1.0, cy, 19.0, 14.0, 0.0, 1.8, outline);

    if !flap {
        cv.fill_ellipse(
            cx - 3.0,
            cy - 6.0,
            11.0,
            5.5,
            -0.1,
            hex_rgba(if variant == 2 { 0xb88254 } else { 0xece6da }, 1.0),
        );
        cv.stroke_ellipse(
            cx - 3.0,
            cy - 6.0,
            11.0,
            5.5,
            -0.1,
            1.2,
            [85.0 / 255.0, 70.0 / 255.0, 54.0 / 255.0, 0.45],
        );
    }

    // Head
    cv.fill_ellipse(cx + 14.0, cy - 2.0, 10.0, 10.0, 0.0, body_a);
    cv.stroke_ellipse(cx + 14.0, cy - 2.0, 10.0, 10.0, 0.0, 1.8, outline);

    // Tactical headband easter egg
    if variant == 3 {
        let red = hex_rgba(0xd1362c, 1.0);
        cv.fill_rect(cx + 6.0, cy - 8.0, 16.0, 4.5, red);
        cv.fill_polygon(&[(cx + 6.0, cy - 6.0), (cx - 2.0, cy - 10.0), (cx, cy - 4.0)], red);
    }

    // Beak
    let beak = [(cx + 22.0, cy - 4.0), (cx + 31.0, cy - 1.5), (cx + 22.0, cy + 2.2)];
    cv.fill_polygon(&beak, hex_rgba(0xf28b20, 1.0));
    cv.stroke_polygon_closed(&beak, 1.2, hex_rgba(0x994b08, 1.0));

    // Crimson comb
    let comb = hex_rgba(0xdf2929, 1.0);
    cv.fill_ellipse(cx + 11.0, cy - 9.0, 3.6, 3.6, 0.0, comb);
    cv.fill_ellipse(cx + 15.0, cy - 10.0, 4.0, 4.0, 0.0, comb);
    cv.fill_ellipse(cx + 18.0, cy - 8.0, 3.2, 3.2, 0.0, comb);

    // Eye
    cv.fill_ellipse(cx + 18.0, cy - 4.5, 1.8, 1.8, 0.0, hex_rgba(0x181818, 1.0));
    cv.fill_ellipse(cx + 18.6, cy - 5.1, 0.7, 0.7, 0.0, [1.0, 1.0, 1.0, 1.0]);

    if flap {
        for (fx, fy, fr) in [(-24.0, -18.0, 0.4), (-14.0, 22.0, -0.5), (22.0, -21.0, 0.7)] {
            cv.fill_ellipse(cx + fx, cy + fy, 4.5, 1.8, fr, body_a);
        }
    }
}

fn paint_soccer_ball(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 64.0;
    let r = 42.0;

    cv.clip_circle = Some((cx, cy, r));
    cv.fill_ellipse_radial(
        cx,
        cy,
        r + 2.0,
        r + 2.0,
        cx - 14.0,
        cy - 14.0,
        4.0,
        cx + 2.0,
        cy + 2.0,
        r * 1.15,
        &[
            (0.0, [1.0, 1.0, 1.0, 1.0]),
            (0.55, hex_rgba(0xede7db, 1.0)),
            (0.88, hex_rgba(0xbab1a0, 1.0)),
            (1.0, hex_rgba(0x7c7364, 1.0)),
        ],
    );

    let draw_pent = |cv: &mut StrikeCanvas, px: f64, py: f64, pr: f64, rot: f64, tilt: f64| {
        let mut pts = Vec::with_capacity(5);
        for i in 0..5 {
            let a = (i as f64 / 5.0) * TAU - FRAC_PI_2;
            let lx = a.cos() * pr;
            let ly = a.sin() * pr * tilt;
            pts.push(rot_local(px, py, rot, lx, ly));
        }
        cv.fill_polygon(&pts, hex_rgba(0x202226, 1.0));
    };

    draw_pent(cv, cx, cy - 1.0, 14.0, 0.0, 1.0);
    let seam = [42.0 / 255.0, 42.0 / 255.0, 46.0 / 255.0, 0.72];
    for i in 0..5 {
        let a = (i as f64 / 5.0) * TAU - FRAC_PI_2;
        cv.stroke_polyline(
            &[
                (cx + a.cos() * 13.0, cy + a.sin() * 13.0 - 1.0),
                (cx + a.cos() * 32.0, cy + a.sin() * 32.0),
            ],
            2.2,
            seam,
        );
        draw_pent(cv, cx + a.cos() * 35.0, cy + a.sin() * 35.0, 11.5, a + PI, 0.72);
    }

    cv.fill_ellipse(cx + 10.0, cy + 14.0, 18.0, 10.0, -0.4, [182.0 / 255.0, 146.0 / 255.0, 98.0 / 255.0, 0.16]);
    cv.fill_ellipse(cx - 15.0, cy - 16.0, 13.0, 8.0, -0.65, [1.0, 1.0, 1.0, 0.34]);
    cv.clip_circle = None;

    cv.stroke_ellipse(cx, cy, r, r, 0.0, 2.2, [28.0 / 255.0, 24.0 / 255.0, 20.0 / 255.0, 0.7]);
}

fn paint_ground_shadow(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 64.0;
    cv.fill_ellipse_radial(
        cx,
        cy,
        44.0,
        34.0,
        cx,
        cy,
        6.0,
        cx,
        cy,
        44.0,
        &[
            (0.0, [24.0 / 255.0, 16.0 / 255.0, 10.0 / 255.0, 0.56]),
            (0.6, [24.0 / 255.0, 16.0 / 255.0, 10.0 / 255.0, 0.28]),
            (1.0, [24.0 / 255.0, 16.0 / 255.0, 10.0 / 255.0, 0.0]),
        ],
    );
}

fn paint_smoke_puff(cv: &mut StrikeCanvas, ox: f64, oy: f64, warm: bool) {
    let cx = ox + 64.0;
    let cy = oy + 64.0;
    let blobs = [
        (0.0, 0.0, 34.0, if warm { [232.0 / 255.0, 212.0 / 255.0, 182.0 / 255.0, 0.32] } else { [218.0 / 255.0, 224.0 / 255.0, 230.0 / 255.0, 0.36] }),
        (-16.0, -8.0, 24.0, if warm { [240.0 / 255.0, 222.0 / 255.0, 194.0 / 255.0, 0.28] } else { [234.0 / 255.0, 238.0 / 255.0, 242.0 / 255.0, 0.32] }),
        (16.0, 9.0, 22.0, if warm { [212.0 / 255.0, 190.0 / 255.0, 158.0 / 255.0, 0.26] } else { [186.0 / 255.0, 194.0 / 255.0, 204.0 / 255.0, 0.28] }),
    ];
    for (bx, by, br, c) in blobs {
        cv.fill_ellipse_radial(
            cx + bx,
            cy + by,
            br,
            br,
            cx + bx,
            cy + by,
            2.0,
            cx + bx,
            cy + by,
            br,
            &[
                (0.0, c),
                (0.7, [c[0], c[1], c[2], 0.12]),
                (1.0, [220.0 / 255.0, 220.0 / 255.0, 220.0 / 255.0, 0.0]),
            ],
        );
    }
}

fn paint_impact_dust(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 64.0;
    cv.fill_ellipse_radial(
        cx,
        cy,
        48.0,
        48.0,
        cx,
        cy,
        10.0,
        cx,
        cy,
        48.0,
        &[
            (0.0, [1.0, 228.0 / 255.0, 178.0 / 255.0, 0.0]),
            (0.45, [242.0 / 255.0, 216.0 / 255.0, 172.0 / 255.0, 0.45]),
            (0.8, [196.0 / 255.0, 164.0 / 255.0, 118.0 / 255.0, 0.24]),
            (1.0, [196.0 / 255.0, 164.0 / 255.0, 118.0 / 255.0, 0.0]),
        ],
    );
    let spark = [1.0, 232.0 / 255.0, 180.0 / 255.0, 0.72];
    for i in 0..8 {
        let a = (i as f64 / 8.0) * TAU + 0.2;
        cv.fill_ellipse(cx + a.cos() * 28.0, cy + a.sin() * 28.0, 2.4, 2.4, 0.0, spark);
    }
}

fn paint_spark_flare(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 64.0;
    cv.fill_ellipse_radial(
        cx,
        cy,
        32.0,
        32.0,
        cx,
        cy,
        2.0,
        cx,
        cy,
        32.0,
        &[
            (0.0, [1.0, 252.0 / 255.0, 232.0 / 255.0, 0.95]),
            (0.35, [1.0, 194.0 / 255.0, 84.0 / 255.0, 0.65]),
            (1.0, [1.0, 140.0 / 255.0, 36.0 / 255.0, 0.0]),
        ],
    );
}

fn paint_target_chalk(cv: &mut StrikeCanvas, ox: f64, oy: f64) {
    let cx = ox + 64.0;
    let cy = oy + 64.0;
    let chalk = [244.0 / 255.0, 236.0 / 255.0, 216.0 / 255.0, 0.48];
    cv.stroke_ellipse(cx, cy, 26.0, 26.0, 0.0, 2.4, chalk);
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        cv.stroke_polyline(
            &[(cx + dx * 16.0, cy + dy * 16.0), (cx + dx * 34.0, cy + dy * 34.0)],
            2.4,
            chalk,
        );
    }
}

pub fn build_strike_atlas() -> (u32, u32, Vec<u8>) {
    let mut cv = StrikeCanvas::new(STRIKE_COLS * STRIKE_CELL, STRIKE_ROWS * STRIKE_CELL, 2);
    let c = STRIKE_CELL as f64;

    // Row 0
    paint_crate(&mut cv, 0.0 * c, 0.0, 0);
    paint_crate(&mut cv, 1.0 * c, 0.0, 1);
    paint_oil_drum(&mut cv, 2.0 * c, 0.0);
    paint_sandbags(&mut cv, 3.0 * c, 0.0);
    paint_weapon_case(&mut cv, 4.0 * c, 0.0);
    paint_desert_planter(&mut cv, 5.0 * c, 0.0);
    paint_smoke_canister(&mut cv, 6.0 * c, 0.0);
    paint_shell_casings(&mut cv, 7.0 * c, 0.0);

    // Row 1
    paint_bullet_decal(&mut cv, 0.0 * c, c, 0);
    paint_bullet_decal(&mut cv, 1.0 * c, c, 1);
    paint_bullet_decal(&mut cv, 2.0 * c, c, 2);
    paint_site_stencil(&mut cv, 3.0 * c, c, true);
    paint_chicken_cell(&mut cv, 4.0 * c, c, 0);
    paint_chicken_cell(&mut cv, 5.0 * c, c, 1);
    paint_chicken_cell(&mut cv, 6.0 * c, c, 2);
    paint_chicken_cell(&mut cv, 7.0 * c, c, 3);

    // Row 2
    paint_soccer_ball(&mut cv, 0.0 * c, 2.0 * c);
    paint_ground_shadow(&mut cv, 1.0 * c, 2.0 * c);
    paint_smoke_puff(&mut cv, 2.0 * c, 2.0 * c, false);
    paint_smoke_puff(&mut cv, 3.0 * c, 2.0 * c, true);
    paint_impact_dust(&mut cv, 4.0 * c, 2.0 * c);
    paint_spark_flare(&mut cv, 5.0 * c, 2.0 * c);
    paint_site_stencil(&mut cv, 6.0 * c, 2.0 * c, false);
    paint_target_chalk(&mut cv, 7.0 * c, 2.0 * c);

    (cv.w as u32, cv.h as u32, cv.px)
}

// =========================================================================
// OpenGL 3.3 Core Renderer
// =========================================================================
#[derive(Clone, Copy)]
struct StrikeInst {
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

pub struct StrikeRenderer {
    ground_prog: GLuint,
    inst_prog: GLuint,
    bg_vao: GLuint,
    inst_vao: GLuint,
    quad_vbo: GLuint,
    inst_vbo: GLuint,
    inst_cap: usize,
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
    u_holes: GLint,
    u_smoke: GLint,
    u_wind: GLint,
    u_debug: GLint,
    // Instanced uniforms
    ui_atlas: GLint,
    ui_res: GLint,
    ui_grid_atlas: GLint,
    // Reusable staging buffers
    floor_items: Vec<StrikeInst>,
    entity_items: Vec<StrikeInst>,
    smoke_items: Vec<StrikeInst>,
    fx_items: Vec<StrikeInst>,
    inst_data: Vec<f32>,
}

impl StrikeRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let ground_prog = gl.compile_program(STRIKE_VERT, STRIKE_FRAG)?;
        let inst_prog = gl.compile_program(STRIKE_INST_VERT, STRIKE_INST_FRAG)?;

        let mut bg_vao = 0;
        (gl.GenVertexArrays)(1, &mut bg_vao);

        let mut inst_vao = 0;
        (gl.GenVertexArrays)(1, &mut inst_vao);
        (gl.BindVertexArray)(inst_vao);

        let quad: [f32; 12] = [
            -0.5, -0.5, 0.5, -0.5, -0.5, 0.5,
            0.5, -0.5, 0.5, 0.5, -0.5, 0.5,
        ];
        let mut quad_vbo = 0;
        (gl.GenBuffers)(1, &mut quad_vbo);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, quad_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (quad.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
            quad.as_ptr() as *const c_void,
            GL_STATIC_DRAW,
        );
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, 0, std::ptr::null());

        let mut inst_vbo = 0;
        (gl.GenBuffers)(1, &mut inst_vbo);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_vbo);
        let stride = (8 * std::mem::size_of::<f32>()) as GLsizei;
        for &(loc, cnt, off) in &[
            (1u32, 2i32, 0usize),
            (2, 1, 8),
            (3, 1, 12),
            (4, 2, 16),
            (5, 1, 24),
            (6, 1, 28),
        ] {
            (gl.EnableVertexAttribArray)(loc);
            (gl.VertexAttribPointer)(loc, cnt, GL_FLOAT, GL_FALSE, stride, off as *const c_void);
            (gl.VertexAttribDivisor)(loc, 1);
        }
        (gl.BindVertexArray)(0);

        let mut noise_tex = 0;
        (gl.GenTextures)(1, &mut noise_tex);
        let mut field_tex = [0u32; 2];
        (gl.GenTextures)(2, field_tex.as_mut_ptr());

        let (aw, ah, apx) = build_strike_atlas();
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
            quad_vbo,
            inst_vbo,
            inst_cap: 0,
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
            u_holes: gl.uniform_loc(ground_prog, "uHoles"),
            u_smoke: gl.uniform_loc(ground_prog, "uSmoke"),
            u_wind: gl.uniform_loc(ground_prog, "uWind"),
            u_debug: gl.uniform_loc(ground_prog, "uDebug"),
            ui_atlas: gl.uniform_loc(inst_prog, "uAtlas"),
            ui_res: gl.uniform_loc(inst_prog, "uRes"),
            ui_grid_atlas: gl.uniform_loc(inst_prog, "uGridAtlas"),
            floor_items: Vec::with_capacity(256),
            entity_items: Vec::with_capacity(256),
            smoke_items: Vec::with_capacity(64),
            fx_items: Vec::with_capacity(32),
            inst_data: Vec::new(),
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
        let bufs = [self.quad_vbo, self.inst_vbo];
        (gl.DeleteBuffers)(2, bufs.as_ptr());
        let texs = [
            self.noise_tex,
            self.field_tex[0],
            self.field_tex[1],
            self.atlas_tex,
        ];
        (gl.DeleteTextures)(4, texs.as_ptr());
    }

    unsafe fn sync_noise_tex(&mut self, gl: &GlFns, ss: &StrikeState) {
        if self.uploaded_noise_version == ss.noise_version {
            return;
        }
        let (s, d) = make_noise_image(ss.params.seed);
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
        self.uploaded_noise_version = ss.noise_version;
    }

    unsafe fn sync_field_textures(&mut self, gl: &GlFns, ss: &StrikeState) {
        if self.uploaded_field_version == ss.field_version
            && self.field_gw == ss.gw
            && self.field_gh == ss.gh
        {
            return;
        }
        let realloc = self.field_gw != ss.gw || self.field_gh != ss.gh;
        for (i, data) in [&ss.field_a, &ss.field_b].iter().enumerate() {
            (gl.ActiveTexture)(if i == 0 { GL_TEXTURE1 } else { GL_TEXTURE2 });
            (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[i]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            if realloc {
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_R32F as GLint,
                    ss.gw as GLsizei,
                    ss.gh as GLsizei,
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
                    ss.gw as GLsizei,
                    ss.gh as GLsizei,
                    GL_RED,
                    GL_FLOAT,
                    data.as_ptr() as *const c_void,
                );
            }
        }
        self.field_gw = ss.gw;
        self.field_gh = ss.gh;
        self.uploaded_field_version = ss.field_version;
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.5);

        st.strike.resize(css_w, css_h);
        let ss = &st.strike;
        self.sync_noise_tex(gl, ss);
        self.sync_field_textures(gl, ss);

        let res_x = pw as f32 / dpi;
        let res_y = ph as f32 / dpi;
        let time_sec = ss.time_sec;

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_SCISSOR_TEST);

        // Pass 1: Sandstone courtyard + bullet-riddled concrete zone + hazard curb
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
        (gl.Uniform2f)(self.u_grid, ss.gw as f32, ss.gh as f32);
        (gl.Uniform1f)(self.u_scale, dpi);
        (gl.Uniform1f)(self.u_seed, (ss.params.seed % 997) as f32 * 0.37);
        (gl.Uniform1f)(self.u_step, STEP as f32);
        (gl.Uniform1f)(self.u_mix, ss.cur_mix as f32);
        (gl.Uniform1f)(self.u_time, time_sec as f32);
        (gl.Uniform1f)(self.u_holes, ss.params.holes as f32);
        (gl.Uniform1f)(self.u_smoke, ss.params.smoke as f32);
        (gl.Uniform1f)(self.u_wind, ss.params.wind as f32);
        (gl.Uniform1f)(self.u_debug, if ss.params.debug { 1.0 } else { 0.0 });
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Pass 2: Layered instanced sprites (floor -> Y-sorted entities -> smoke -> FX)
        self.floor_items.clear();
        self.entity_items.clear();
        self.smoke_items.clear();
        self.fx_items.clear();

        for d in &ss.site_decals {
            self.floor_items.push(StrikeInst {
                sort_y: 0.0,
                x: d.x as f32,
                y: d.y as f32,
                size: d.size as f32,
                rot: d.rot as f32,
                col: d.col as f32,
                row: d.row as f32,
                alpha: d.alpha as f32,
                flip: 1.0,
            });
        }
        for bd in &ss.bullet_decals {
            self.floor_items.push(StrikeInst {
                sort_y: 0.0,
                x: bd.x as f32,
                y: bd.y as f32,
                size: bd.size as f32,
                rot: bd.rot as f32,
                col: bd.col as f32,
                row: bd.row as f32,
                alpha: bd.alpha as f32,
                flip: 1.0,
            });
        }
        for cn in &ss.canisters {
            self.floor_items.push(StrikeInst {
                sort_y: 0.0,
                x: cn.x as f32,
                y: cn.y as f32,
                size: cn.size as f32,
                rot: cn.rot as f32,
                col: 6.0,
                row: 0.0,
                alpha: 0.92,
                flip: 1.0,
            });
        }

        for c in &ss.crates {
            if c.grow <= 0.01 {
                continue;
            }
            let ease = c.grow * c.grow * (3.0 - 2.0 * c.grow);
            self.entity_items.push(StrikeInst {
                sort_y: c.y,
                x: c.x as f32,
                y: c.y as f32,
                size: (c.size * (0.28 + 0.72 * ease)) as f32,
                rot: c.rot as f32,
                col: c.col as f32,
                row: 0.0,
                alpha: (ease * 1.1).min(1.0) as f32,
                flip: 1.0,
            });
            if c.is_case && ease > 0.75 {
                let pulse = 0.5 + 0.5 * (time_sec * 6.2 + c.phase).sin();
                if pulse > 0.6 {
                    self.fx_items.push(StrikeInst {
                        sort_y: 0.0,
                        x: c.x as f32,
                        y: c.y as f32,
                        size: (16.0 + pulse * 10.0) as f32,
                        rot: (time_sec * 0.4) as f32,
                        col: 5.0,
                        row: 2.0,
                        alpha: ((pulse - 0.55) * 1.5) as f32,
                        flip: 1.0,
                    });
                }
            }
        }

        for ch in &ss.chickens {
            let hop = if ch.walk_t > 0.0 {
                ch.walk_t.sin().abs() * if ch.panic_t > 0.0 { 6.0 } else { 2.6 }
            } else {
                0.0
            };
            let waddle = if ch.walk_t > 0.0 {
                ch.walk_t.sin() * 0.08
            } else {
                (time_sec * 2.2 + ch.idx as f64).sin() * 0.03
            };
            let col = if ch.panic_t > 0.05 { 5 } else { ch.base_col };
            self.entity_items.push(StrikeInst {
                sort_y: ch.y,
                x: ch.x as f32,
                y: (ch.y - hop) as f32,
                size: 48.0,
                rot: waddle as f32,
                col: col as f32,
                row: 1.0,
                alpha: 0.99,
                flip: ch.flip as f32,
            });
        }

        for b in &ss.balls {
            let shadow_scale = (1.0 - b.z * 0.004).max(0.55);
            self.floor_items.push(StrikeInst {
                sort_y: 0.0,
                x: (b.x + 5.0 + b.z * 0.14) as f32,
                y: (b.y + 6.0 + b.z * 0.16) as f32,
                size: (b.r * 2.55 * shadow_scale) as f32,
                rot: 0.0,
                col: 1.0,
                row: 2.0,
                alpha: (0.72 - b.z * 0.003).max(0.28) as f32,
                flip: 1.0,
            });
            self.entity_items.push(StrikeInst {
                sort_y: b.y + 2.0,
                x: b.x as f32,
                y: (b.y - b.z) as f32,
                size: (b.r * 2.55 * (1.0 + b.z * 0.0028)) as f32,
                rot: b.rot as f32,
                col: 0.0,
                row: 2.0,
                alpha: 1.0,
                flip: 1.0,
            });
        }

        self.entity_items.sort_by(|a, b| a.sort_y.total_cmp(&b.sort_y));

        let smoke_alpha = ss.params.smoke;
        if smoke_alpha > 0.02 {
            for sp in &ss.smoke_puffs {
                self.smoke_items.push(StrikeInst {
                    sort_y: 0.0,
                    x: sp.x as f32,
                    y: sp.y as f32,
                    size: sp.size as f32,
                    rot: ((time_sec * 0.25 + sp.phase).sin() * 0.4) as f32,
                    col: sp.col as f32,
                    row: 2.0,
                    alpha: (smoke_alpha * 0.65) as f32,
                    flip: 1.0,
                });
            }
        }

        for imp in &ss.impact_bursts {
            let t = imp.age / imp.dur;
            self.fx_items.push(StrikeInst {
                sort_y: 0.0,
                x: imp.x as f32,
                y: imp.y as f32,
                size: (imp.size * (0.45 + 0.85 * t)) as f32,
                rot: (imp.rot + t * 0.35) as f32,
                col: 4.0,
                row: 2.0,
                alpha: ((1.0 - t) * 0.9) as f32,
                flip: 1.0,
            });
        }

        let total = self.floor_items.len()
            + self.entity_items.len()
            + self.smoke_items.len()
            + self.fx_items.len();
        if total == 0 {
            return;
        }

        self.inst_data.clear();
        self.inst_data.reserve(total * 8);
        for slice in [
            &self.floor_items[..],
            &self.entity_items[..],
            &self.smoke_items[..],
            &self.fx_items[..],
        ] {
            for it in slice {
                self.inst_data.push(it.x);
                self.inst_data.push(it.y);
                self.inst_data.push(it.size);
                self.inst_data.push(it.rot);
                self.inst_data.push(it.col);
                self.inst_data.push(it.row);
                self.inst_data.push(it.alpha);
                self.inst_data.push(it.flip);
            }
        }

        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
        (gl.UseProgram)(self.inst_prog);
        (gl.BindVertexArray)(self.inst_vao);
        (gl.ActiveTexture)(GL_TEXTURE6);
        (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
        (gl.Uniform1i)(self.ui_atlas, 6);
        (gl.Uniform2f)(self.ui_res, res_x, res_y);
        (gl.Uniform2f)(self.ui_grid_atlas, STRIKE_COLS as f32, STRIKE_ROWS as f32);

        let bytes = (total * 8 * std::mem::size_of::<f32>()) as GLsizeiptr;
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_vbo);
        if total > self.inst_cap {
            self.inst_cap = total.max(64);
            (gl.BufferData)(
                GL_ARRAY_BUFFER,
                (self.inst_cap * 32) as GLsizeiptr,
                std::ptr::null(),
                GL_DYNAMIC_DRAW,
            );
        }
        (gl.BufferSubData)(GL_ARRAY_BUFFER, 0, bytes, self.inst_data.as_ptr() as *const c_void);
        (gl.DrawArraysInstanced)(GL_TRIANGLES, 0, 6, total as GLsizei);
        (gl.BindVertexArray)(0);
        (gl.Disable)(GL_BLEND);
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
        ]
    }

    #[test]
    fn verify_rust_anything_recorded_strike_cases() {
        let root = repo_root();
        let rec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/strike/recorded-from.json");
        let rec: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&rec_path).unwrap()).unwrap();
        for src in rec["sources"].as_array().unwrap() {
            let rel = src["path"].as_str().unwrap();
            let expected = src["fnv1a"].as_str().unwrap();
            let actual = fnv1a_fingerprint(rel, &std::fs::read(root.join(rel)).unwrap());
            assert_eq!(actual, expected, "Source fingerprint mismatch for {rel}");
        }

        let cases_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/strike/cases.jsonl");
        let lines: Vec<serde_json::Value> = std::fs::read_to_string(&cases_path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 3);

        // Case 1: computeFieldStrike
        {
            let c = &lines[0];
            assert_eq!(c["fn"], "computeFieldStrike");
            let mut ss = StrikeState::new(1280.0, 720.0);
            ss.set_icons_raw(&sample_icons(), false);
            let probes = c["input"]["probes"].as_array().unwrap();
            let exp_s = c["output"]["sampled"].as_array().unwrap();
            for (k, p) in probes.iter().enumerate() {
                let x = p[0].as_f64().unwrap();
                let y = p[1].as_f64().unwrap();
                assert_close(ss.land_s(x, y), exp_s[k].as_f64().unwrap(), 1e-4, &format!("field[{k}]"));
            }
        }

        // Case 2: strikePropsBuild
        {
            let c = &lines[1];
            assert_eq!(c["fn"], "strikePropsBuild");
            let configs = [("s42", 42, 1.0, 1.0, 1, 4), ("s1337", 1337, 0.7, 1.5, 2, 6)];
            for (key, seed, crates_d, holes_d, ball_n, chickens_n) in configs {
                let exp = &c["output"][key];
                let exp_h = exp["hashes"].as_array().unwrap();
                assert_close(strike_hash(seed, 0, 0, 1), exp_h[0].as_f64().unwrap(), 1e-12, "h0");
                assert_close(strike_hash(seed, 7, 13, 14), exp_h[1].as_f64().unwrap(), 1e-12, "h1");
                assert_close(strike_hash(seed, 21, 5, 51), exp_h[2].as_f64().unwrap(), 1e-12, "h2");

                let mut ss = StrikeState::new(1280.0, 720.0);
                ss.params.seed = seed;
                ss.params.crates = crates_d;
                ss.params.holes = holes_d;
                ss.params.ball = ball_n;
                ss.params.chickens = chickens_n;
                ss.build_noise();
                ss.icons = sample_icons();
                ss.refresh();
                ss.rebuild_props();
                ss.update_targets(true);

                assert_eq!(ss.crates.len(), exp["crateTotal"].as_u64().unwrap() as usize, "{key} crateTotal");
                let grown = ss.crates.iter().filter(|x| x.want > 0.5).count();
                assert_eq!(grown, exp["crateGrown"].as_u64().unwrap() as usize, "{key} crateGrown");
                for s in exp["crateSamples"].as_array().unwrap() {
                    let i = s["i"].as_u64().unwrap() as usize;
                    let cr = &ss.crates[i];
                    assert_close(cr.x, s["x"].as_f64().unwrap(), 1e-9, "crate.x");
                    assert_close(cr.y, s["y"].as_f64().unwrap(), 1e-9, "crate.y");
                    assert_eq!(cr.col, s["col"].as_u64().unwrap() as usize);
                    assert_close(cr.size, s["size"].as_f64().unwrap(), 1e-9, "crate.size");
                    assert_close(cr.want, s["want"].as_f64().unwrap(), 1e-9, "crate.want");
                }
                assert_eq!(ss.bullet_decals.len(), exp["bulletCount"].as_u64().unwrap() as usize, "{key} bulletCount");
                assert_eq!(ss.canisters.len(), exp["canisterCount"].as_u64().unwrap() as usize);
                assert_eq!(ss.smoke_puffs.len(), exp["smokeCount"].as_u64().unwrap() as usize);

                let exp_b = exp["balls"].as_array().unwrap();
                assert_eq!(ss.balls.len(), exp_b.len());
                for (i, eb) in exp_b.iter().enumerate() {
                    assert_close(ss.balls[i].x, eb["x"].as_f64().unwrap(), 1e-6, "ball.x");
                    assert_close(ss.balls[i].y, eb["y"].as_f64().unwrap(), 1e-6, "ball.y");
                    assert_close(ss.balls[i].vx, eb["vx"].as_f64().unwrap(), 1e-6, "ball.vx");
                    assert_close(ss.balls[i].vy, eb["vy"].as_f64().unwrap(), 1e-6, "ball.vy");
                }

                let exp_ch = exp["chickens"].as_array().unwrap();
                assert_eq!(ss.chickens.len(), exp_ch.len());
                for (i, ec) in exp_ch.iter().enumerate() {
                    assert_eq!(ss.chickens[i].base_col, ec["baseCol"].as_u64().unwrap() as usize);
                    assert_close(ss.chickens[i].x, ec["x"].as_f64().unwrap(), 1e-6, "ch.x");
                    assert_close(ss.chickens[i].y, ec["y"].as_f64().unwrap(), 1e-6, "ch.y");
                }
            }
        }

        // Case 3: strikeBallAndChickenPhysics
        {
            let c = &lines[2];
            assert_eq!(c["fn"], "strikeBallAndChickenPhysics");
            let mut ss = StrikeState::new(1280.0, 720.0);
            ss.icons = sample_icons();
            ss.refresh();
            ss.rebuild_props();
            ss.update_targets(true);
            let b0 = ss.balls[0];
            let kicked = ss.pointer_kick(b0.x - 8.0, b0.y - 4.0, 240.0, 60.0);
            assert_eq!(kicked, c["output"]["kicked"].as_bool().unwrap());

            let dt = c["input"]["dt"].as_f64().unwrap();
            let steps = c["input"]["steps"].as_u64().unwrap() as usize;
            let trace = c["output"]["trace"].as_array().unwrap();
            let mut ti = 0usize;
            let mut t_sec = 0.0;
            for s in 0..steps {
                t_sec += dt;
                ss.step_props(dt, t_sec);
                if ti < trace.len() && trace[ti]["step"].as_u64().unwrap() as usize == s {
                    let tr = &trace[ti];
                    let b = &ss.balls[0];
                    let eb = tr["ball"].as_array().unwrap();
                    assert_close(b.x, eb[0].as_f64().unwrap(), 1e-5, &format!("step {s} b.x"));
                    assert_close(b.y, eb[1].as_f64().unwrap(), 1e-5, &format!("step {s} b.y"));
                    assert_close(b.z, eb[2].as_f64().unwrap(), 1e-5, &format!("step {s} b.z"));
                    assert_close(b.vx, eb[3].as_f64().unwrap(), 1e-5, &format!("step {s} b.vx"));
                    assert_close(b.vy, eb[4].as_f64().unwrap(), 1e-5, &format!("step {s} b.vy"));
                    assert_close(b.vz, eb[5].as_f64().unwrap(), 1e-5, &format!("step {s} b.vz"));
                    assert_close(b.rot, eb[6].as_f64().unwrap(), 1e-5, &format!("step {s} b.rot"));

                    let ch = &ss.chickens[0];
                    let ec = tr["ch0"].as_array().unwrap();
                    assert_close(ch.x, ec[0].as_f64().unwrap(), 1e-5, "ch.x");
                    assert_close(ch.y, ec[1].as_f64().unwrap(), 1e-5, "ch.y");
                    assert_close(ch.wait, ec[2].as_f64().unwrap(), 1e-6, "ch.wait");
                    assert_eq!(ss.impact_bursts.len(), tr["bursts"].as_u64().unwrap() as usize);
                    ti += 1;
                }
            }
            assert_eq!(ti, trace.len());
        }
    }
}
