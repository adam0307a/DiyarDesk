pub mod shaders;

use crate::beach::sample_arr;
use crate::gl::*;
use crate::meadow::{
    fbm, make_noise_image, IconBox, MORPH_EMPTY_SEC, MORPH_SEC, STEP, TASKBAR,
};
use crate::state::{AppState, Rect};
use shaders::*;
use std::f64::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// Constants & Deterministic Hashes (1:1 prototype/aurora.html + aurora-pines.js)
// =========================================================================
const AURORA_PINE_VARIANTS: usize = 4;
const AURORA_LANTERN_VARIANT: usize = 4;
const AURORA_ATLAS_COLS: usize = 5;
const AURORA_SPRITE_W: usize = 160;
const AURORA_SPRITE_H: usize = 160;

#[inline]
pub fn aurora_rand(seed: i32, i: i32, k: i32) -> f64 {
    let mut h = seed
        .wrapping_add(i.wrapping_mul(131))
        .wrapping_mul(374_761_393)
        ^ k.wrapping_add(19).wrapping_mul(668_265_263);
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
// Parameters & Props
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct AuroraParams {
    pub sigma: f64,
    pub warp: f64,
    pub shoal: f64,
    pub aurora: f64,
    pub flow: f64,
    pub snow: f64,
    pub pines: usize,
    pub lanterns: usize,
    pub adaptive: bool,
    pub seed: i32,
    pub debug: bool,
}

impl Default for AuroraParams {
    fn default() -> Self {
        Self {
            sigma: 82.0,
            warp: 32.0,
            shoal: 90.0,
            aurora: 0.78,
            flow: 1.0,
            snow: 0.45,
            pines: 6,
            lanterns: 2,
            adaptive: true,
            seed: 42,
            debug: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct AuroraTree {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub variant: usize,
    pub mirror: f64,
    pub grow: f64,
    pub want: f64,
}

#[derive(Clone, Copy)]
pub struct AuroraLantern {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub variant: usize,
    pub phase: f64,
    pub grow: f64,
    pub want: f64,
}

#[derive(Clone, Copy)]
pub struct AuroraRipple {
    pub x: f64,
    pub y: f64,
    pub t0: f64,
    pub strength: f64,
}

pub fn aurora_spot_clear(icons: &[Rect], x: f64, y: f64, pad: f64) -> bool {
    icons.iter().all(|ic| {
        let l = ic.x as f64 - pad;
        let t = ic.y as f64 - pad;
        let r = (ic.x + ic.w) as f64 + pad;
        let b = (ic.y + ic.h) as f64 + pad;
        x < l || x > r || y < t || y > b
    })
}

pub fn lantern_flicker(time_sec: f64, phase: f64) -> f64 {
    0.93 + 0.07 * (time_sec * 3.1 + phase).sin() * (time_sec * 1.7 + phase).cos()
}

// =========================================================================
// Aurora State Machine
// =========================================================================
pub struct AuroraState {
    pub p: AuroraParams,
    pub w: f64,
    pub h: f64,
    pub gw: usize,
    pub gh: usize,
    pub wx: Vec<f32>,
    pub wy: Vec<f32>,
    pub fa: Vec<f32>,
    pub fb: Vec<f32>,
    pub field_dirty_a: bool,
    pub field_dirty_b: bool,
    pub noise_dirty: bool,
    pub morph_elapsed: f64,
    pub morph_dur: f64,
    pub cur_mix: f64,
    pub time: f64,
    pub flow_time: f64,
    pub icons: Vec<Rect>,
    pub trees: Vec<AuroraTree>,
    pub lanterns: Vec<AuroraLantern>,
    pub ripples: Vec<AuroraRipple>,
    pub icons_seen: bool,
    pub first_icons: bool,
}

impl Default for AuroraState {
    fn default() -> Self {
        let mut st = Self {
            p: AuroraParams::default(),
            w: 1280.0,
            h: 720.0,
            gw: 1,
            gh: 1,
            wx: Vec::new(),
            wy: Vec::new(),
            fa: Vec::new(),
            fb: Vec::new(),
            field_dirty_a: true,
            field_dirty_b: true,
            noise_dirty: true,
            morph_elapsed: 10.0,
            morph_dur: MORPH_SEC,
            cur_mix: 1.0,
            time: 0.0,
            flow_time: 0.0,
            icons: Vec::new(),
            trees: Vec::new(),
            lanterns: Vec::new(),
            ripples: Vec::new(),
            icons_seen: false,
            first_icons: true,
        };
        st.resize(1280.0, 720.0);
        st.refresh();
        st
    }
}

impl AuroraState {
    pub fn build_noise(&mut self) {
        let n = self.gw * self.gh;
        self.wx.resize(n, 0.0);
        self.wy.resize(n, 0.0);
        let seed = self.p.seed;
        for gy in 0..self.gh {
            for gx in 0..self.gw {
                let i = gy * self.gw + gx;
                let x = gx as f64 * STEP;
                let y = gy as f64 * STEP;
                self.wx[i] = (fbm(x * 0.0028, y * 0.0028, 1, seed) - 0.47) as f32;
                self.wy[i] = (fbm(x * 0.0028, y * 0.0028, 2, seed) - 0.47) as f32;
            }
        }
    }

    pub fn compute_field_raw(&self, rects: &[IconBox]) -> Vec<f32> {
        let n = self.gw * self.gh;
        let mut f1 = vec![0.0f32; n];
        let sig = self.p.sigma;
        let is1 = 1.0 / (sig * sig);
        let wa = self.p.warp * 2.2;
        let wmax = wa * 0.5;
        for r in rects {
            let rad = 5.5 * sig + r.hw.max(r.hh) * 0.5 + wmax;
            let gx0 = ((r.cx - rad) / STEP).floor().max(0.0) as usize;
            let gx1 = (((r.cx + rad) / STEP).ceil() as isize)
                .clamp(0, self.gw as isize - 1) as usize;
            let gy0 = ((r.cy - rad) / STEP).floor().max(0.0) as usize;
            let gy1 = (((r.cy + rad) / STEP).ceil() as isize)
                .clamp(0, self.gh as isize - 1) as usize;
            let chw = r.hw * 0.5;
            let chh = r.hh * 0.5;
            for gy in gy0..=gy1 {
                for gx in gx0..=gx1 {
                    let i = gy * self.gw + gx;
                    let qx = gx as f64 * STEP + self.wx[i] as f64 * wa;
                    let qy = gy as f64 * STEP + self.wy[i] as f64 * wa;
                    let dx = ((qx - r.cx).abs() - chw).max(0.0);
                    let dy = ((qy - r.cy).abs() - chh).max(0.0);
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

    #[inline]
    pub fn cur_s(&self, x: f64, y: f64) -> f64 {
        if self.fb.is_empty() {
            return 999.0;
        }
        sample_arr(&self.fa, self.gw, self.gh, x, y) * (1.0 - self.cur_mix)
            + sample_arr(&self.fb, self.gw, self.gh, x, y) * self.cur_mix
    }

    #[inline]
    pub fn land_s(&self, x: f64, y: f64) -> f64 {
        if self.fb.is_empty() {
            return 999.0;
        }
        sample_arr(&self.fb, self.gw, self.gh, x, y)
    }

    fn icon_boxes(&self) -> Vec<IconBox> {
        if !self.p.adaptive {
            return Vec::new();
        }
        self.icons.iter().map(IconBox::from_rect).collect()
    }

    pub fn rebuild_props(&mut self) {
        self.trees.clear();
        self.lanterns.clear();
        let want_pines = self.p.pines.min(12);
        let want_lanterns = self.p.lanterns.min(6);
        let seed = self.p.seed;
        let span_x = (self.w - 88.0).max(40.0);
        let span_y = (self.h - TASKBAR - 88.0).max(40.0);

        for attempt in 0..900i32 {
            if self.trees.len() >= want_pines {
                break;
            }
            let x = 44.0 + aurora_rand(seed, attempt, 1) * span_x;
            let y = 44.0 + aurora_rand(seed, attempt, 2) * span_y;
            let s = self.land_s(x, y);
            if s > -14.0 {
                continue;
            }
            if !aurora_spot_clear(&self.icons, x, y, 22.0) {
                continue;
            }
            if self.trees.iter().any(|p| (p.x - x).hypot(p.y - y) < 58.0) {
                continue;
            }
            let scale = 0.72 + aurora_rand(seed, attempt, 3) * 0.42;
            let variant = (aurora_rand(seed, attempt, 4) * AURORA_PINE_VARIANTS as f64).floor()
                as usize;
            let mirror = if aurora_rand(seed, attempt, 5) < 0.5 {
                -1.0
            } else {
                1.0
            };
            self.trees.push(AuroraTree {
                x,
                y,
                scale,
                variant: variant.min(AURORA_PINE_VARIANTS - 1),
                mirror,
                grow: 0.0,
                want: 1.0,
            });
        }

        let lspan_x = (self.w - 96.0).max(40.0);
        let lspan_y = (self.h - TASKBAR - 96.0).max(40.0);
        for attempt in 0..2200i32 {
            if self.lanterns.len() >= want_lanterns {
                break;
            }
            let x = 48.0 + aurora_rand(seed, attempt + 1200, 1) * lspan_x;
            let y = 48.0 + aurora_rand(seed, attempt + 1200, 2) * lspan_y;
            let s = self.land_s(x, y);
            if s > -2.0 || s < -54.0 {
                continue;
            }
            if !aurora_spot_clear(&self.icons, x, y, 16.0) {
                continue;
            }
            if self.lanterns.iter().any(|p| (p.x - x).hypot(p.y - y) < 90.0) {
                continue;
            }
            if self.trees.iter().any(|p| (p.x - x).hypot(p.y - y) < 38.0) {
                continue;
            }
            let scale = 0.92 + aurora_rand(seed, attempt + 1200, 3) * 0.2;
            let phase = aurora_rand(seed, attempt + 1200, 4) * TAU;
            self.lanterns.push(AuroraLantern {
                x,
                y,
                scale,
                variant: AURORA_LANTERN_VARIANT,
                phase,
                grow: 0.0,
                want: 1.0,
            });
        }
    }

    pub fn commit(&mut self, from_empty: bool) {
        let boxes = self.icon_boxes();
        let nf = self.compute_field_raw(&boxes);
        if from_empty {
            self.fa = self.compute_field_raw(&[]);
        } else {
            self.fa = self.fb.clone();
        }
        self.fb = nf;
        self.field_dirty_a = true;
        self.field_dirty_b = true;
        self.morph_elapsed = 0.0;
        self.morph_dur = if from_empty { MORPH_EMPTY_SEC } else { MORPH_SEC };
        self.cur_mix = 0.0;
        self.rebuild_props();
    }

    pub fn refresh(&mut self) {
        let boxes = self.icon_boxes();
        let nf = self.compute_field_raw(&boxes);
        self.fa = nf.clone();
        self.fb = nf;
        self.field_dirty_a = true;
        self.field_dirty_b = true;
        self.morph_elapsed = self.morph_dur;
        self.cur_mix = 1.0;
        self.rebuild_props();
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = (w as f64).max(320.0);
        let nh = (h as f64).max(200.0);
        let ngw = (nw / STEP).ceil() as usize + 2;
        let ngh = (nh / STEP).ceil() as usize + 2;
        if (self.w - nw).abs() < 0.5 && (self.h - nh).abs() < 0.5 && self.gw == ngw && self.gh == ngh
        {
            return;
        }
        self.w = nw;
        self.h = nh;
        self.gw = ngw;
        self.gh = ngh;
        self.build_noise();
        self.refresh();
    }

    pub fn set_icons(&mut self, icons: &[Rect], vis: bool) {
        let next: Vec<Rect> = if vis { icons.to_vec() } else { Vec::new() };
        let same = next.len() == self.icons.len()
            && next.iter().zip(self.icons.iter()).all(|(a, b)| {
                (a.x - b.x).abs() < 0.5
                    && (a.y - b.y).abs() < 0.5
                    && (a.w - b.w).abs() < 0.5
                    && (a.h - b.h).abs() < 0.5
            });
        if same && self.icons_seen {
            return;
        }
        self.icons = next;
        self.icons_seen = true;
        let first = self.first_icons;
        self.first_icons = false;
        self.commit(first);
    }

    pub fn add_ripple_if_ice(&mut self, x: f32, y: f32, strength: f64) {
        let xf = x as f64;
        let yf = y as f64;
        if self.cur_s(xf, yf) > 4.0 {
            self.ripples.push(AuroraRipple {
                x: xf,
                y: yf,
                t0: self.time,
                strength,
            });
            if self.ripples.len() > 6 {
                self.ripples.remove(0);
            }
        }
    }

    pub fn sync_params(&mut self, next: AuroraParams) {
        let seed_changed = next.seed != self.p.seed;
        let adaptive_changed = next.adaptive != self.p.adaptive;
        let field_changed =
            (next.sigma - self.p.sigma).abs() > 1e-4 || (next.warp - self.p.warp).abs() > 1e-4;
        let props_changed = next.pines != self.p.pines || next.lanterns != self.p.lanterns;
        self.p = next;
        if seed_changed {
            self.build_noise();
            self.noise_dirty = true;
            if self.icons_seen {
                self.commit(true);
            } else {
                self.refresh();
            }
            return;
        }
        if adaptive_changed {
            self.commit(false);
        } else if field_changed {
            self.refresh();
        } else if props_changed {
            self.rebuild_props();
        }
    }

    pub fn step_props(&mut self, dt: f64) {
        for t in &mut self.trees {
            t.grow = (t.grow + dt * 1.4).min(t.want);
        }
        for l in &mut self.lanterns {
            l.grow = (l.grow + dt * 1.6).min(l.want);
        }
    }

    pub fn step(&mut self, dt_f32: f32) {
        let dt = (dt_f32 as f64).clamp(0.001, 0.1);
        self.time += dt;
        self.flow_time += dt * self.p.flow;

        if self.morph_elapsed < self.morph_dur {
            self.morph_elapsed = (self.morph_elapsed + dt).min(self.morph_dur);
        }
        let e = (self.morph_elapsed / self.morph_dur.max(0.001)).clamp(0.0, 1.0);
        self.cur_mix = if e < 0.5 {
            2.0 * e * e
        } else {
            1.0 - (-2.0 * e + 2.0).powi(2) * 0.5
        };

        self.step_props(dt);
        self.ripples.retain(|r| (self.time - r.t0) <= 4.0);
    }
}

// =========================================================================
// Procedural 2x Pine & Lantern Sprite Atlas (1:1 makeAuroraSpriteAtlas)
// =========================================================================
#[inline]
fn blend_src_over(dst: &mut [f64; 4], src: [f64; 4]) {
    let sa = src[3].clamp(0.0, 1.0);
    if sa <= 1e-5 {
        return;
    }
    let da = dst[3];
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-6 {
        *dst = [0.0; 4];
        return;
    }
    for c in 0..3 {
        dst[c] = (src[c] * sa + dst[c] * da * (1.0 - sa)) / out_a;
    }
    dst[3] = out_a;
}

fn poly_signed_dist(px: f64, py: f64, pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    let mut inside = false;
    let mut min_d2 = f64::INFINITY;
    let mut j = n - 1;
    for i in 0..n {
        let (ax, ay) = (pts[j][0], pts[j][1]);
        let (bx, by) = (pts[i][0], pts[i][1]);
        if ((ay > py) != (by > py)) && (px < (bx - ax) * (py - ay) / (by - ay) + ax) {
            inside = !inside;
        }
        let vx = bx - ax;
        let vy = by - ay;
        let denom = vx * vx + vy * vy;
        let t = if denom > 1e-12 {
            (((px - ax) * vx + (py - ay) * vy) / denom).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dx = px - (ax + vx * t);
        let dy = py - (ay + vy * t);
        min_d2 = min_d2.min(dx * dx + dy * dy);
        j = i;
    }
    let d = min_d2.sqrt();
    if inside { -d } else { d }
}

fn rotated_ellipse_cov(px: f64, py: f64, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64) -> f64 {
    let dx = px - cx;
    let dy = py - cy;
    let (c, s) = (rot.cos(), rot.sin());
    let lx = c * dx + s * dy;
    let ly = -s * dx + c * dy;
    let u = lx / rx;
    let v = ly / ry;
    let d = (u * u + v * v).sqrt();
    let avg_r = (rx + ry) * 0.5;
    let aa = 0.65 / avg_r.max(1.0);
    (1.0 - ((d - 1.0) / aa + 0.5).clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

fn build_aurora_sprite_atlas() -> (u32, u32, Vec<u8>) {
    let scale = 2usize;
    let tile_w = AURORA_SPRITE_W * scale; // 320
    let tile_h = AURORA_SPRITE_H * scale; // 320
    let atlas_w = AURORA_ATLAS_COLS * tile_w; // 1600
    let atlas_h = tile_h; // 320
    let mut canvas = vec![[0.0f64; 4]; atlas_w * atlas_h];

    // Variants 0..3: Snow-dusted Nordic pines
    for v in 0..AURORA_PINE_VARIANTS {
        let x_off = v * tile_w;
        let cx = AURORA_SPRITE_W as f64 * 0.5;
        let cy = AURORA_SPRITE_H as f64 * 0.5;
        let base_r = 26.0 + v as f64 * 2.5;

        // Precompute the 4 tiers' polygons & snow circles
        struct TierData {
            tx: f64,
            ty: f64,
            r: f64,
            pts: Vec<[f64; 2]>,
            c0: [f64; 3],
            c1: [f64; 3],
            snow_circles: Vec<[f64; 3]>,
            snow_alpha: f64,
        }
        let mut tiers = Vec::with_capacity(4);
        for tier in 0..4usize {
            let f = tier as f64 / 3.0;
            let r = base_r * (1.0 - f * 0.62);
            let arms = 11 + (v % 3) * 2 + (tier % 2) * 2;
            let tier_rot = v as f64 * 0.9 + tier as f64 * 0.43;
            let tx = cx - f * 3.5;
            let ty = cy - f * 4.5;
            let mut pts = Vec::with_capacity(arms * 2);
            for i in 0..(arms * 2) {
                let ang = tier_rot + (i as f64 * PI) / arms as f64;
                let j = atlas_hash((v * 97 + tier * 31) as i32, i as i32);
                let rad = if i % 2 == 0 {
                    r * (0.88 + j * 0.26)
                } else {
                    r * (0.54 + j * 0.14)
                };
                pts.push([tx + ang.cos() * rad, ty + ang.sin() * rad]);
            }
            let shade = (tier as f64 * 12.0).round() as i32;
            let c0 = [
                (42 + shade) as f64 / 255.0,
                (78 + shade) as f64 / 255.0,
                (76 + shade) as f64 / 255.0,
            ];
            let c1 = [
                (20 + (shade >> 1)) as f64 / 255.0,
                (44 + (shade >> 1)) as f64 / 255.0,
                (52 + (shade >> 1)) as f64 / 255.0,
            ];
            let mut snow_circles = Vec::new();
            for i in 0..arms {
                let ang = tier_rot + (i as f64 * TAU) / arms as f64;
                if ang.cos() * -0.6 + ang.sin() * -0.8 < -0.1 {
                    continue;
                }
                let j = atlas_hash((v * 53 + tier * 17) as i32, (i + 40) as i32);
                let rad = r * (0.62 + j * 0.25);
                let sx = tx + ang.cos() * rad;
                let sy = ty + ang.sin() * rad;
                let sr = r * (0.19 + j * 0.11);
                snow_circles.push([sx, sy, sr]);
            }
            tiers.push(TierData {
                tx,
                ty,
                r,
                pts,
                c0,
                c1,
                snow_circles,
                snow_alpha: 0.42 + f * 0.26,
            });
        }

        for py in 0..tile_h {
            let y = (py as f64 + 0.5) / scale as f64;
            for px in 0..tile_w {
                let x = (px as f64 + 0.5) / scale as f64;
                let mut col = [0.0f64; 4];

                // 3 ground shadow ellipses
                for t in 0..3 {
                    let sr = base_r * (0.98 - t as f64 * 0.2);
                    let cov = rotated_ellipse_cov(
                        x,
                        y,
                        cx + 12.0 + t as f64 * 2.0,
                        cy + 16.0 + t as f64 * 2.0,
                        sr * 0.95,
                        sr * 0.85,
                        0.4,
                    );
                    if cov > 0.001 {
                        blend_src_over(
                            &mut col,
                            [14.0 / 255.0, 24.0 / 255.0, 46.0 / 255.0, 0.34 * cov],
                        );
                    }
                }

                // 4 foliage tiers + snow patches
                for td in &tiers {
                    let d = poly_signed_dist(x, y, &td.pts);
                    let fill_cov = (0.5 - d * 1.4).clamp(0.0, 1.0);
                    if fill_cov > 0.001 {
                        // Radial gradient centered at (tx - r*0.25, ty - r*0.25) -> (tx, ty)
                        let gx = x - (td.tx - td.r * 0.25);
                        let gy = y - (td.ty - td.r * 0.25);
                        let gd = gx.hypot(gy);
                        let gt = ((gd - td.r * 0.1) / (td.r * 0.9)).clamp(0.0, 1.0);
                        let r_col = td.c0[0] + (td.c1[0] - td.c0[0]) * gt;
                        let g_col = td.c0[1] + (td.c1[1] - td.c0[1]) * gt;
                        let b_col = td.c0[2] + (td.c1[2] - td.c0[2]) * gt;
                        blend_src_over(&mut col, [r_col, g_col, b_col, fill_cov]);
                    }
                    // 0.9px dark outline stroke
                    let stroke_cov = (1.0 - (d.abs() - 0.45).max(0.0) * 2.0).clamp(0.0, 1.0);
                    if stroke_cov > 0.001 {
                        blend_src_over(
                            &mut col,
                            [11.0 / 255.0, 23.0 / 255.0, 34.0 / 255.0, 0.45 * stroke_cov],
                        );
                    }
                    // Union of snow patches in this tier
                    let mut snow_cov = 0.0f64;
                    for sc in &td.snow_circles {
                        let cd = (x - sc[0]).hypot(y - sc[1]) - sc[2];
                        let cov = (0.5 - cd * 1.4).clamp(0.0, 1.0);
                        if cov > snow_cov {
                            snow_cov = cov;
                        }
                    }
                    if snow_cov > 0.001 {
                        blend_src_over(
                            &mut col,
                            [
                                182.0 / 255.0,
                                212.0 / 255.0,
                                238.0 / 255.0,
                                td.snow_alpha * snow_cov,
                            ],
                        );
                    }
                }

                canvas[py * atlas_w + x_off + px] = col;
            }
        }
    }

    // Variant 4: Cozy warm Nordic lantern with wide golden glow pool
    {
        let x_off = AURORA_LANTERN_VARIANT * tile_w;
        let cx = AURORA_SPRITE_W as f64 * 0.5;
        let cy = AURORA_SPRITE_H as f64 * 0.5;
        for py in 0..tile_h {
            let y = (py as f64 + 0.5) / scale as f64;
            for px in 0..tile_w {
                let x = (px as f64 + 0.5) / scale as f64;
                let mut col = [0.0f64; 4];

                // Wide amber ground halo (r = 2..74)
                let dist = (x - cx).hypot(y - cy);
                if dist < 74.5 {
                    let t = ((dist - 2.0) / 72.0).clamp(0.0, 1.0);
                    let halo = if t < 0.18 {
                        let u = t / 0.18;
                        [
                            1.0,
                            (224.0 + (178.0 - 224.0) * u) / 255.0,
                            (140.0 + (76.0 - 140.0) * u) / 255.0,
                            0.85 + (0.52 - 0.85) * u,
                        ]
                    } else if t < 0.48 {
                        let u = (t - 0.18) / 0.30;
                        [
                            (255.0 + (232.0 - 255.0) * u) / 255.0,
                            (178.0 + (130.0 - 178.0) * u) / 255.0,
                            (76.0 + (52.0 - 76.0) * u) / 255.0,
                            0.52 + (0.20 - 0.52) * u,
                        ]
                    } else {
                        let u = (t - 0.48) / 0.52;
                        [
                            232.0 / 255.0,
                            130.0 / 255.0,
                            52.0 / 255.0,
                            0.20 * (1.0 - u),
                        ]
                    };
                    let edge_aa = (74.5 - dist).clamp(0.0, 1.0);
                    blend_src_over(&mut col, [halo[0], halo[1], halo[2], halo[3] * edge_aa]);
                }

                // Subtle dark lantern shadow
                let sh_cov = rotated_ellipse_cov(x, y, cx + 4.0, cy + 5.0, 6.5, 5.5, 0.3);
                if sh_cov > 0.001 {
                    blend_src_over(
                        &mut col,
                        [12.0 / 255.0, 18.0 / 255.0, 34.0 / 255.0, 0.45 * sh_cov],
                    );
                }

                // Dark iron roof cap (#202636, r = 6.8) + golden rim stroke (#ffd27a, 1.2px)
                let cap_d = dist - 6.8;
                let cap_cov = (0.5 - cap_d * 1.4).clamp(0.0, 1.0);
                if cap_cov > 0.001 {
                    blend_src_over(
                        &mut col,
                        [0x20 as f64 / 255.0, 0x26 as f64 / 255.0, 0x36 as f64 / 255.0, cap_cov],
                    );
                }
                let rim_cov = (1.0 - (cap_d.abs() - 0.6).max(0.0) * 1.8).clamp(0.0, 1.0);
                if rim_cov > 0.001 {
                    blend_src_over(
                        &mut col,
                        [1.0, 0xd2 as f64 / 255.0, 0x7a as f64 / 255.0, rim_cov],
                    );
                }

                // Bright warm flame core (#fffdf0 -> #ffd466 -> #f08a24, r = 4.6)
                let core_cov = (0.5 - (dist - 4.6) * 1.4).clamp(0.0, 1.0);
                if core_cov > 0.001 {
                    let ct = ((dist - 0.4) / 4.2).clamp(0.0, 1.0);
                    let flame = if ct < 0.55 {
                        let u = ct / 0.55;
                        [
                            1.0,
                            (253.0 + (212.0 - 253.0) * u) / 255.0,
                            (240.0 + (102.0 - 240.0) * u) / 255.0,
                        ]
                    } else {
                        let u = (ct - 0.55) / 0.45;
                        [
                            (255.0 + (240.0 - 255.0) * u) / 255.0,
                            (212.0 + (138.0 - 212.0) * u) / 255.0,
                            (102.0 + (36.0 - 102.0) * u) / 255.0,
                        ]
                    };
                    blend_src_over(&mut col, [flame[0], flame[1], flame[2], core_cov]);
                }

                canvas[py * atlas_w + x_off + px] = col;
            }
        }
    }

    let mut rgba = vec![0u8; atlas_w * atlas_h * 4];
    for (i, c) in canvas.iter().enumerate() {
        rgba[i * 4] = (c[0] * 255.0).round().clamp(0.0, 255.0) as u8;
        rgba[i * 4 + 1] = (c[1] * 255.0).round().clamp(0.0, 255.0) as u8;
        rgba[i * 4 + 2] = (c[2] * 255.0).round().clamp(0.0, 255.0) as u8;
        rgba[i * 4 + 3] = (c[3] * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    (atlas_w as u32, atlas_h as u32, rgba)
}

// =========================================================================
// OpenGL 3.3 Core Aurora Renderer
// =========================================================================
pub struct AuroraRenderer {
    dummy_vao: GLuint,
    prog_main: GLuint,
    prog_sprite: GLuint,
    prog_snow: GLuint,
    noise_tex: GLuint,
    ftex: [GLuint; 2],
    field_w: usize,
    field_h: usize,
    atlas_tex: GLuint,
}

impl AuroraRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let prog_main = gl.compile_program(AURORA_VERT, AURORA_FRAG)?;
        let prog_sprite = gl.compile_program(AURORA_SPRITE_VERT, AURORA_SPRITE_FRAG)?;
        let prog_snow = gl.compile_program(AURORA_SNOW_VERT, AURORA_SNOW_FRAG)?;

        let mut noise_tex = 0;
        (gl.GenTextures)(1, &mut noise_tex);
        let mut ftex = [0u32; 2];
        (gl.GenTextures)(2, ftex.as_mut_ptr());

        let (aw, ah, rgba) = build_aurora_sprite_atlas();
        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        (gl.ActiveTexture)(GL_TEXTURE6);
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            aw as GLsizei,
            ah as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            rgba.as_ptr() as *const c_void,
        );
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            dummy_vao,
            prog_main,
            prog_sprite,
            prog_snow,
            noise_tex,
            ftex,
            field_w: 0,
            field_h: 0,
            atlas_tex,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.atlas_tex != 0 {
            (gl.DeleteTextures)(1, &self.atlas_tex);
            self.atlas_tex = 0;
        }
        (gl.DeleteTextures)(2, self.ftex.as_ptr());
        if self.noise_tex != 0 {
            (gl.DeleteTextures)(1, &self.noise_tex);
            self.noise_tex = 0;
        }
        for p in [self.prog_main, self.prog_sprite, self.prog_snow] {
            if p != 0 {
                (gl.DeleteProgram)(p);
            }
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
    }

    unsafe fn sync_noise_tex(&mut self, gl: &GlFns, a: &mut AuroraState) {
        if !a.noise_dirty {
            return;
        }
        a.noise_dirty = false;
        let (s, data) = make_noise_image(a.p.seed);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            s as GLsizei,
            s as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            data.as_ptr() as *const c_void,
        );
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);
    }

    unsafe fn sync_field_textures(&mut self, gl: &GlFns, a: &mut AuroraState) {
        if self.field_w != a.gw || self.field_h != a.gh {
            self.field_w = a.gw;
            self.field_h = a.gh;
            for &tex in &self.ftex {
                (gl.BindTexture)(GL_TEXTURE_2D, tex);
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_R32F as GLint,
                    a.gw as GLsizei,
                    a.gh as GLsizei,
                    0,
                    GL_RED,
                    GL_FLOAT,
                    std::ptr::null(),
                );
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            }
            a.field_dirty_a = true;
            a.field_dirty_b = true;
        }
        if a.field_dirty_a && a.fa.len() == a.gw * a.gh {
            a.field_dirty_a = false;
            (gl.ActiveTexture)(GL_TEXTURE1);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[0]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            (gl.TexSubImage2D)(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                a.gw as GLsizei,
                a.gh as GLsizei,
                GL_RED,
                GL_FLOAT,
                a.fa.as_ptr() as *const c_void,
            );
        }
        if a.field_dirty_b && a.fb.len() == a.gw * a.gh {
            a.field_dirty_b = false;
            (gl.ActiveTexture)(GL_TEXTURE2);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[1]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            (gl.TexSubImage2D)(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                a.gw as GLsizei,
                a.gh as GLsizei,
                GL_RED,
                GL_FLOAT,
                a.fb.as_ptr() as *const c_void,
            );
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.5);

        st.aurora.resize(css_w, css_h);
        self.sync_noise_tex(gl, &mut st.aurora);
        self.sync_field_textures(gl, &mut st.aurora);

        let a = &st.aurora;
        let res_x = pw as f32 / dpi;
        let res_y = ph as f32 / dpi;
        let seed_u = ((a.p.seed.rem_euclid(997)) as f32) * 0.37;

        let mut rip_buf = [0.0f32; 24];
        let mut rip_count = 0i32;
        for r in a.ripples.iter().rev() {
            let age = a.time - r.t0;
            if age <= 4.0 && rip_count < 6 {
                let i = rip_count as usize;
                rip_buf[i * 4] = r.x as f32;
                rip_buf[i * 4 + 1] = r.y as f32;
                rip_buf[i * 4 + 2] = age as f32;
                rip_buf[i * 4 + 3] = r.strength as f32;
                rip_count += 1;
            }
        }

        // Pass 1: Frozen fjord, aurora curtains, stars, ripples & twilight snow islands
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_SCISSOR_TEST);

        (gl.UseProgram)(self.prog_main);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_main, "uNoise"), 0);

        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[0]);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_main, "uFA"), 1);

        (gl.ActiveTexture)(GL_TEXTURE2);
        (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[1]);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_main, "uFB"), 2);

        (gl.Uniform2f)(gl.uniform_loc(self.prog_main, "uRes"), res_x, res_y);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_main, "uGrid"), a.gw as f32, a.gh as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "uScale"), dpi);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "uSeed"), seed_u);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "uStep"), STEP as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "uMix"), a.cur_mix as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "uTime"), a.time as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "uFlowTime"), a.flow_time as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "uDebug"), if a.p.debug { 1.0 } else { 0.0 });
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "uShoal"), a.p.shoal as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_main, "uAurora"), a.p.aurora as f32);
        (gl.Uniform4fv)(gl.uniform_loc(self.prog_main, "uRip"), 6, rip_buf.as_ptr());
        (gl.Uniform1i)(gl.uniform_loc(self.prog_main, "uRipN"), rip_count);

        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Pass 2: Snow-dusted Nordic pines & glowing warm lanterns
        if !a.trees.is_empty() || !a.lanterns.is_empty() {
            (gl.UseProgram)(self.prog_sprite);
            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
            (gl.ActiveTexture)(GL_TEXTURE6);
            (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
            (gl.Uniform1i)(gl.uniform_loc(self.prog_sprite, "uAtlas"), 6);
            (gl.Uniform2f)(gl.uniform_loc(self.prog_sprite, "uRes"), res_x, res_y);
            (gl.Uniform1f)(gl.uniform_loc(self.prog_sprite, "uTime"), a.time as f32);

            for t in &a.trees {
                if t.grow <= 0.01 {
                    continue;
                }
                let ease = t.grow * t.grow * (3.0 - 2.0 * t.grow);
                (gl.Uniform4f)(
                    gl.uniform_loc(self.prog_sprite, "uSprite"),
                    t.x as f32,
                    t.y as f32,
                    (t.scale * ease) as f32,
                    t.variant as f32,
                );
                (gl.Uniform1f)(gl.uniform_loc(self.prog_sprite, "uMirror"), t.mirror as f32);
                (gl.Uniform1f)(gl.uniform_loc(self.prog_sprite, "uSway"), 1.0);
                (gl.Uniform1f)(gl.uniform_loc(self.prog_sprite, "uOpacity"), (0.96 * ease) as f32);
                (gl.Uniform1f)(gl.uniform_loc(self.prog_sprite, "uPulse"), 1.0);
                (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);
            }

            for l in &a.lanterns {
                if l.grow <= 0.01 {
                    continue;
                }
                let ease = l.grow * l.grow * (3.0 - 2.0 * l.grow);
                let flicker = lantern_flicker(a.time, l.phase);
                (gl.Uniform4f)(
                    gl.uniform_loc(self.prog_sprite, "uSprite"),
                    l.x as f32,
                    l.y as f32,
                    (l.scale * ease) as f32,
                    l.variant as f32,
                );
                (gl.Uniform1f)(gl.uniform_loc(self.prog_sprite, "uMirror"), 1.0);
                (gl.Uniform1f)(gl.uniform_loc(self.prog_sprite, "uSway"), 0.0);
                (gl.Uniform1f)(gl.uniform_loc(self.prog_sprite, "uOpacity"), (0.95 * ease) as f32);
                (gl.Uniform1f)(gl.uniform_loc(self.prog_sprite, "uPulse"), flicker as f32);
                (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);
            }
            (gl.Disable)(GL_BLEND);
        }

        // Pass 3: Lightweight GPU falling snowflakes
        let snow_count = (a.p.snow.clamp(0.0, 1.0) * 80.0).round() as GLsizei;
        if snow_count > 0 {
            (gl.UseProgram)(self.prog_snow);
            (gl.Enable)(GL_PROGRAM_POINT_SIZE);
            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
            (gl.Uniform2f)(gl.uniform_loc(self.prog_snow, "uRes"), res_x, res_y);
            (gl.Uniform1f)(
                gl.uniform_loc(self.prog_snow, "uTime"),
                (a.time * (0.4 + 0.6 * a.p.flow)) as f32,
            );
            (gl.Uniform1f)(gl.uniform_loc(self.prog_snow, "uScale"), dpi);
            (gl.Uniform1f)(gl.uniform_loc(self.prog_snow, "uSeed"), seed_u);
            (gl.BindVertexArray)(self.dummy_vao);
            (gl.DrawArrays)(GL_POINTS, 0, snow_count);
            (gl.Disable)(GL_BLEND);
            (gl.Disable)(GL_PROGRAM_POINT_SIZE);
        }
    }
}

// =========================================================================
// rust-anything Recorded Ground-Truth Diff & Gate Verification Suite
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
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

    fn assert_near(got: f64, exp: f64, eps: f64, label: &str) {
        assert!(
            (got - exp).abs() <= eps,
            "{label}: got {got}, expected {exp} (diff {})",
            (got - exp).abs()
        );
    }

    #[test]
    fn verify_rust_anything_recorded_aurora_cases() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root");
        let rec: Value = serde_json::from_str(
            &std::fs::read_to_string(manifest_dir.join("src/aurora/recorded-from.json"))
                .expect("read recorded-from"),
        )
        .expect("parse recorded-from");
        for src in rec["sources"].as_array().unwrap() {
            let rel = src["path"].as_str().unwrap();
            let exp = src["fnv1a"].as_str().unwrap();
            let bytes = std::fs::read(repo_root.join(rel)).expect("read source");
            assert_eq!(fnv1a_fingerprint(rel, &bytes), exp, "Drift in {rel}");
        }

        let raw = std::fs::read_to_string(manifest_dir.join("src/aurora/cases.jsonl")).unwrap();
        let mut checked = 0usize;
        let mut cached_icons: Vec<Rect> = Vec::new();
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            let c: Value = serde_json::from_str(line).unwrap();
            let fname = c["fn"].as_str().unwrap();
            let inp = &c["input"];
            let out = &c["output"];

            let parse_icons = |arr: &Value| -> Vec<Rect> {
                arr.as_array()
                    .unwrap()
                    .iter()
                    .map(|r| Rect {
                        x: r["x"].as_f64().unwrap() as f32,
                        y: r["y"].as_f64().unwrap() as f32,
                        w: r["w"].as_f64().unwrap() as f32,
                        h: r["h"].as_f64().unwrap() as f32,
                    })
                    .collect()
            };

            match fname {
                "computeFieldAurora" => {
                    let mut st = AuroraState::default();
                    st.p.seed = inp["seed"].as_i64().unwrap() as i32;
                    st.p.sigma = inp["sigma"].as_f64().unwrap();
                    st.p.warp = inp["warp"].as_f64().unwrap();
                    st.resize(inp["W"].as_f64().unwrap() as f32, inp["H"].as_f64().unwrap() as f32);
                    st.build_noise();
                    cached_icons = parse_icons(&inp["icons"]);
                    st.icons = cached_icons.clone();
                    st.refresh();
                    for (i, pr) in inp["probes"].as_array().unwrap().iter().enumerate() {
                        let got = sample_arr(
                            &st.fb,
                            st.gw,
                            st.gh,
                            pr[0].as_f64().unwrap(),
                            pr[1].as_f64().unwrap(),
                        );
                        assert_near(got, out["sampled"][i].as_f64().unwrap(), 1e-4, "aurora field");
                    }
                }
                "auroraPropPlacement" => {
                    let icons = parse_icons(&inp["icons"]);
                    for (seed_val, pines, lanterns, key) in
                        [(42, 6, 2, "s42"), (1337, 10, 4, "s1337")]
                    {
                        let mut st = AuroraState::default();
                        st.p.seed = seed_val;
                        st.p.sigma = inp["sigma"].as_f64().unwrap();
                        st.p.warp = inp["warp"].as_f64().unwrap();
                        st.p.pines = pines;
                        st.p.lanterns = lanterns;
                        st.resize(inp["W"].as_f64().unwrap() as f32, inp["H"].as_f64().unwrap() as f32);
                        st.build_noise();
                        st.icons = icons.clone();
                        st.refresh();

                        let rands = [
                            aurora_rand(seed_val, 0, 1),
                            aurora_rand(seed_val, 17, 4),
                            aurora_rand(seed_val, 1205, 2),
                        ];
                        for (i, &rv) in rands.iter().enumerate() {
                            assert_near(rv, out[key]["rands"][i].as_f64().unwrap(), 1e-12, "auroraRand");
                        }
                        let exp_t = out[key]["trees"].as_array().unwrap();
                        assert_eq!(st.trees.len(), exp_t.len(), "{key} trees len");
                        for (i, e) in exp_t.iter().enumerate() {
                            assert_near(st.trees[i].x, e["x"].as_f64().unwrap(), 1e-6, "tree.x");
                            assert_near(st.trees[i].y, e["y"].as_f64().unwrap(), 1e-6, "tree.y");
                            assert_near(st.trees[i].scale, e["scale"].as_f64().unwrap(), 1e-6, "tree.scale");
                            assert_eq!(st.trees[i].variant, e["variant"].as_u64().unwrap() as usize);
                            assert_near(st.trees[i].mirror, e["mirror"].as_f64().unwrap(), 1e-6, "tree.mirror");
                        }
                        let exp_l = out[key]["lanterns"].as_array().unwrap();
                        assert_eq!(st.lanterns.len(), exp_l.len(), "{key} lanterns len");
                        for (i, e) in exp_l.iter().enumerate() {
                            assert_near(st.lanterns[i].x, e["x"].as_f64().unwrap(), 1e-6, "lantern.x");
                            assert_near(st.lanterns[i].y, e["y"].as_f64().unwrap(), 1e-6, "lantern.y");
                            assert_near(st.lanterns[i].scale, e["scale"].as_f64().unwrap(), 1e-6, "lantern.scale");
                            assert_near(st.lanterns[i].phase, e["phase"].as_f64().unwrap(), 1e-6, "lantern.phase");
                        }
                    }
                }
                "auroraPropStepAndFlicker" => {
                    let mut st = AuroraState::default();
                    st.p.seed = 42;
                    st.resize(1280.0, 720.0);
                    st.build_noise();
                    st.icons = cached_icons.clone();
                    st.refresh();
                    let dt = inp["dt"].as_f64().unwrap();
                    let trace = out["growTrace"].as_array().unwrap();
                    for (i, e) in trace.iter().enumerate() {
                        st.step_props(dt);
                        assert_near(st.trees[0].grow, e["tg"].as_f64().unwrap(), 1e-9, "tree.grow");
                        assert_near(st.lanterns[0].grow, e["lg"].as_f64().unwrap(), 1e-9, "lantern.grow");
                        let fl = lantern_flicker((i + 1) as f64 * 0.45, st.lanterns[0].phase);
                        assert_near(fl, e["flicker"].as_f64().unwrap(), 1e-9, "flicker");
                    }
                }
                "pineAtlasHash" => {
                    for (i, pair) in inp["pairs"].as_array().unwrap().iter().enumerate() {
                        let a = pair[0].as_i64().unwrap() as i32;
                        let b = pair[1].as_i64().unwrap() as i32;
                        assert_near(
                            atlas_hash(a, b),
                            out["values"][i].as_f64().unwrap(),
                            1e-12,
                            "atlas_hash",
                        );
                    }
                }
                other => panic!("Unexpected case fn: {other}"),
            }
            checked += 1;
        }
        assert_eq!(checked, 4);
    }
}
