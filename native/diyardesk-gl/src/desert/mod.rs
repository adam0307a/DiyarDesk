// DiyarDesk Desert Oasis (desert) — native Rust + OpenGL 3.3 Core port.
// Ported 1:1 from prototype/desert.html (WebGL2 sand shader + CPU icon distance field)
// and prototype/desert-props.js (instanced camel / traveler / tent / palm sprite batch).

pub mod atlas;
pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use atlas::bake_atlas;
use shaders::{DS_BG_FRAG, DS_BG_VERT, DS_SPRITE_FRAG, DS_SPRITE_VERT};
use std::ffi::c_void;
use std::f32::consts::PI;

/// Bit-exact port of the prototype's `rng32(s)` closure (desert.html:186).
pub struct Rng32 {
    s: i32,
}

impl Rng32 {
    pub fn new(seed: i32) -> Self {
        Self { s: seed }
    }

    pub fn next_f64(&mut self) -> f64 {
        self.s = self.s.wrapping_add(0x6D2B79F5u32 as i32);
        let au = self.s as u32;
        let im1 = (au ^ (au >> 15)).wrapping_mul(1 | au);
        let im2 = (im1 ^ (im1 >> 7)).wrapping_mul(61 | im1);
        self.s = (im1.wrapping_add(im2)) as i32 ^ (im1 as i32);
        let v = (self.s as u32) ^ ((self.s as u32) >> 14);
        v as f64 / 4294967296.0
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }
}

const FIELD_STEP: f32 = 14.0;
const FIELD_K: f32 = 56.0;
const MORPH_SEC: f32 = 0.95;
const MORPH_FIRST_SEC: f32 = 1.5;
const MAX_INST: usize = 600;

/// Unclamped CSS size (the prototype uses `cv.clientWidth/clientHeight`, no 320px floor).
pub fn css_dims(st: &AppState) -> (f32, f32) {
    let s = st.dpi_scale.max(0.5);
    (
        (st.width_px as f32 / s).max(1.0).round(),
        (st.height_px as f32 / s).max(1.0).round(),
    )
}

#[inline]
fn clamp_f(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = clamp_f((x - a) / (b - a), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Shader-side smoothstep helper kept for parity checks against DS_BG_FRAG.
#[allow(dead_code)]
pub fn smoothstep_ref(a: f32, b: f32, x: f32) -> f32 {
    smoothstep(a, b, x)
}

// =========================================================================
// Parameters (1:1 with DEFAULTS in desert.html)
// =========================================================================
#[derive(Clone, Copy)]
pub struct DesertParams {
    pub sigma: f32,
    pub warp: f32,
    pub oases: usize,
    pub warmth: f32,
    pub wind: f32,
    pub camels: usize,
    pub travelers: bool,
    pub palms: usize,
    pub adaptive: bool,
    pub debug: bool,
    pub seed: u32,
    pub quality: usize,
}

impl Default for DesertParams {
    fn default() -> Self {
        Self {
            sigma: 88.0,
            warp: 36.0,
            oases: 2,
            warmth: 0.78,
            wind: 1.0,
            camels: 6,
            travelers: true,
            palms: 10,
            adaptive: true,
            debug: false,
            seed: 73194,
            quality: 2,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Oasis {
    pub x: f32,
    pub y: f32,
    pub rx: f32,
    pub ry: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Prop {
    pub x: f32,
    pub y: f32,
    pub col: usize,
    pub size: f32,
    pub rot: f32,
    pub sway_phase: f32,
    pub sway_amt: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Caravan {
    pub base_x: f32,
    pub start_y: f32,
    pub base_speed: f32,
    pub path_amp: f32,
    pub freq: f32,
    pub phase: f32,
    pub camels: usize,
    pub has_guide: bool,
}

#[derive(Clone, Copy, Default)]
pub struct Dust {
    pub x: f32,
    pub y: f32,
    pub spd: f32,
    pub size: f32,
    pub alpha: f32,
    pub phase: f32,
}

// =========================================================================
// CPU distance field (icon -> raised sand dune)
// =========================================================================
fn hash2(ix: f64, iy: f64, seed: f64) -> f64 {
    let n = ((ix * 127.1 + iy * 311.7 + seed * 0.013) * 43758.5453).sin();
    n - n.floor()
}

fn vnoise(x: f64, y: f64, seed: f64) -> f64 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash2(ix, iy, seed);
    let b = hash2(ix + 1.0, iy, seed);
    let c = hash2(ix, iy + 1.0, seed);
    let d = hash2(ix + 1.0, iy + 1.0, seed);
    a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy
}

fn fbm_cpu(x: f64, y: f64, seed: f64) -> f64 {
    vnoise(x, y, seed) * 0.65 + vnoise(x * 2.05 + 4.1, y * 2.05 - 2.7, seed) * 0.35
}

pub struct DesertState {
    pub p: DesertParams,
    pub w: f32,
    pub h: f32,
    pub gw: usize,
    pub gh: usize,
    pub field_a: Vec<f32>,
    pub field_b: Vec<f32>,
    pub from_i: usize,
    pub to_i: usize,
    pub dirty_slots: [bool; 2],
    pub morph_t0: f32,
    pub morph_dur: f32,
    pub time: f32,
    pub oases: Vec<Oasis>,
    pub props: Vec<Prop>,
    pub caravans: Vec<Caravan>,
    pub dust: Vec<Dust>,
    pub pulse: [f32; 4],
    pub icons: Vec<Rect>,
    pub field_dirty: bool,
    pub props_dirty: bool,
    pub caravans_dirty: bool,
    pub noise_dirty: bool,
    pub low_mode: bool,
}

impl Default for DesertState {
    fn default() -> Self {
        let mut s = Self {
            p: DesertParams::default(),
            w: 0.0,
            h: 0.0,
            gw: 1,
            gh: 1,
            field_a: Vec::new(),
            field_b: Vec::new(),
            from_i: 0,
            to_i: 0,
            dirty_slots: [false, false],
            morph_t0: -1.0e9,
            morph_dur: MORPH_SEC,
            time: 0.0,
            oases: Vec::new(),
            props: Vec::new(),
            caravans: Vec::new(),
            dust: Vec::new(),
            pulse: [0.0, 0.0, -99.0, 0.0],
            icons: Vec::new(),
            field_dirty: true,
            props_dirty: true,
            caravans_dirty: true,
            noise_dirty: true,
            low_mode: false,
        };
        s.rebuild_dust();
        s
    }
}

impl DesertState {
    pub fn resize(&mut self, w: f32, h: f32) {
        let w = w.max(1.0);
        let h = h.max(1.0);
        if (self.w - w).abs() < 0.5 && (self.h - h).abs() < 0.5 && self.gw > 1 {
            return;
        }
        self.w = w;
        self.h = h;
        self.gw = ((w / FIELD_STEP).ceil() as usize + 2).max(8);
        self.gh = ((h / FIELD_STEP).ceil() as usize + 2).max(8);
        self.field_dirty = true;
        self.props_dirty = true;
        self.caravans_dirty = true;
    }

    pub fn sync_params(&mut self, p: DesertParams) {
        let old = self.p;
        if old.seed != p.seed {
            self.noise_dirty = true;
            self.field_dirty = true;
            self.props_dirty = true;
            self.caravans_dirty = true;
            self.rebuild_dust();
        }
        if (old.sigma - p.sigma).abs() > 1e-4 || (old.warp - p.warp).abs() > 1e-4 || old.adaptive != p.adaptive {
            self.field_dirty = true;
        }
        if old.oases != p.oases || old.palms != p.palms || old.travelers != p.travelers {
            self.props_dirty = true;
        }
        if old.camels != p.camels {
            self.caravans_dirty = true;
        }
        self.p = p;
    }

    pub fn set_icons(&mut self, rects: &[Rect], vis: bool) {
        let next: Vec<Rect> = if vis { rects.to_vec() } else { Vec::new() };
        let changed = next.len() != self.icons.len()
            || next
                .iter()
                .zip(self.icons.iter())
                .any(|(a, b)| (a.x - b.x).abs() > 0.5 || (a.y - b.y).abs() > 0.5 || (a.w - b.w).abs() > 0.5 || (a.h - b.h).abs() > 0.5);
        self.icons = next;
        if changed {
            self.field_dirty = true;
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.time += dt.clamp(0.0, 0.25);
    }

    pub fn tap(&mut self, x: f32, y: f32, on_water: bool) {
        self.pulse = [x, y, self.time, if on_water { 1.0 } else { 0.0 }];
    }

    pub fn is_on_water(&self, x: f32, y: f32) -> bool {
        for oa in &self.oases {
            let dx = (x - oa.x) / oa.rx.max(1.0);
            let dy = (y - oa.y) / oa.ry.max(1.0);
            if (dx * dx + dy * dy).sqrt() < 1.0 {
                return true;
            }
        }
        false
    }

    fn icon_rects(&self) -> Vec<[f32; 4]> {
        if !self.p.adaptive {
            return Vec::new();
        }
        self.icons
            .iter()
            .map(|r| [r.x + r.w * 0.5, r.y + r.h * 0.5, r.w * 0.5 - 4.0, r.h * 0.5 - 2.0])
            .collect()
    }

    pub fn compute_field(&self, rects: &[[f32; 4]]) -> Vec<f32> {
        let gw = self.gw.max(1);
        let gh = self.gh.max(1);
        let mut out = vec![0.0f32; gw * gh];
        let seed = self.p.seed as f64;
        let warp_amt = self.p.warp;
        let sigma = self.p.sigma;
        for j in 0..gh {
            let y = j as f32 * FIELD_STEP;
            for i in 0..gw {
                let x = i as f32 * FIELD_STEP;
                let mut sum = 0.0f32;
                for b in rects {
                    let dx = ((x - b[0]).abs() - b[2]).max(0.0);
                    let dy = ((y - b[1]).abs() - b[3]).max(0.0);
                    let d = (dx * dx + dy * dy).sqrt();
                    sum += (-d / FIELD_K).exp();
                }
                let d = if sum > 1e-8 { -FIELD_K * sum.ln() } else { 1030.0 } - sigma;
                let warp = ((fbm_cpu(x as f64 / 185.0, y as f64 / 185.0, seed) - 0.48) as f32) * warp_amt;
                out[j * gw + i] = -(d + warp);
            }
        }
        out
    }

    pub fn field_at(&self, x: f32, y: f32) -> f32 {
        if self.field_b.is_empty() {
            return -1030.0;
        }
        let gw = self.gw.max(1);
        let gh = self.gh.max(1);
        let i = ((x / FIELD_STEP).round() as i32).clamp(0, gw as i32 - 1) as usize;
        let j = ((y / FIELD_STEP).round() as i32).clamp(0, gh as i32 - 1) as usize;
        self.field_b[j * gw + i]
    }

    pub fn commit(&mut self, from_empty: bool) {
        let rects = self.icon_rects();
        let nf = self.compute_field(&rects);
        let target = 1 - self.to_i;
        if from_empty {
            self.field_a = self.compute_field(&[]);
            self.dirty_slots[self.to_i] = true;
        } else {
            self.field_a = self.field_b.clone();
        }
        self.field_b = nf;
        self.dirty_slots[target] = true;
        self.from_i = self.to_i;
        self.to_i = target;
        self.morph_t0 = self.time;
        self.morph_dur = if from_empty { MORPH_FIRST_SEC } else { MORPH_SEC };
        self.field_dirty = false;
        if self.props.is_empty() && self.oases.is_empty() && self.p.oases > 0 {
            self.rebuild_props();
        }
    }

    pub fn refresh(&mut self) {
        let rects = self.icon_rects();
        let f = self.compute_field(&rects);
        self.field_a = f.clone();
        self.field_b = f;
        self.dirty_slots[self.to_i] = true;
        self.from_i = self.to_i;
        self.morph_t0 = -1.0e9;
        self.field_dirty = false;
        self.rebuild_props();
    }

    /// Force a full re-bake (used by tests and by theme switches).
    pub fn rebake(&mut self) {
        self.refresh();
    }

    // ---------------------------------------------------------------------
    // Oases, palms, Bedouin camps, caravans, dust
    // ---------------------------------------------------------------------
    fn pick_oases(&mut self) {
        let mut r = Rng32::new((self.p.seed as i32).wrapping_add(411));
        let wc = self.w;
        let hc = self.h;
        let mut cand = Vec::with_capacity(4);
        for (fx, fy, rx0, ry0, mx, my, mrx, mry) in [
            (0.64f32, 0.28f32, 74.0f32, 52.0f32, 0.14f32, 0.16f32, 24.0f32, 18.0f32),
            (0.34, 0.62, 66.0, 48.0, 0.14, 0.14, 22.0, 16.0),
            (0.76, 0.68, 56.0, 42.0, 0.12, 0.14, 18.0, 14.0),
            (0.24, 0.24, 58.0, 40.0, 0.12, 0.14, 18.0, 14.0),
        ] {
            let x = wc * (fx + r.next_f32() * mx);
            let y = hc * (fy + r.next_f32() * my);
            let rx = rx0 + r.next_f32() * mrx;
            let ry = ry0 + r.next_f32() * mry;
            cand.push(Oasis { x, y, rx, ry });
        }
        let mut out = Vec::new();
        let count = self.p.oases.min(cand.len());
        for i in 0..count {
            let o = cand[i];
            if self.p.adaptive && !self.field_b.is_empty() {
                let mut best = o;
                let mut best_val = self.field_at(o.x, o.y);
                let mut t = 0;
                while t < 8 && best_val > -18.0 {
                    let nx = clamp_f(o.x + (r.next_f32() - 0.5) * 200.0, 120.0, (wc - 120.0).max(120.0));
                    let ny = clamp_f(o.y + (r.next_f32() - 0.5) * 160.0, 100.0, (hc - 120.0).max(100.0));
                    let fv = self.field_at(nx, ny);
                    if fv < best_val {
                        best_val = fv;
                        best = Oasis { x: nx, y: ny, rx: o.rx, ry: o.ry };
                    }
                    t += 1;
                }
                out.push(best);
            } else {
                out.push(o);
            }
        }
        self.oases = out;
    }

    pub fn rebuild_props(&mut self) {
        self.pick_oases();
        self.props.clear();
        let mut r = Rng32::new((self.p.seed as i32).wrapping_add(887));
        let wc = self.w;
        let hc = self.h;
        let count = self.oases.len();

        if count > 0 {
            let total_palms = self.p.palms.max(0);
            for i in 0..total_palms {
                let oa = self.oases[i % count];
                let ang = (i as f32 / (total_palms as f32 / count as f32).max(1.0)) * 2.0 * PI + r.next_f32() * 0.7;
                let dist_mul = 1.06 + r.next_f32() * 0.34;
                let x = oa.x + ang.cos() * oa.rx * dist_mul;
                let y = oa.y + ang.sin() * oa.ry * dist_mul;
                self.props.push(Prop {
                    x,
                    y,
                    col: if i % 3 == 0 { 6 } else { 5 },
                    size: (if i % 3 == 0 { 52.0 } else { 64.0 }) * (0.88 + r.next_f32() * 0.25),
                    rot: r.next_f32() * 2.0 * PI,
                    sway_phase: r.next_f32() * 6.28,
                    sway_amt: 0.045,
                });
            }

            for oi in 0..count {
                let oa = self.oases[oi];
                for b in 0..5 {
                    let ang = b as f32 * 1.25 + r.next_f32() * 0.6;
                    self.props.push(Prop {
                        x: oa.x + ang.cos() * oa.rx * (1.02 + r.next_f32() * 0.14),
                        y: oa.y + ang.sin() * oa.ry * (1.02 + r.next_f32() * 0.14),
                        col: 8,
                        size: 32.0 + r.next_f32() * 14.0,
                        rot: r.next_f32() * 2.0 * PI,
                        sway_phase: r.next_f32() * 6.28,
                        sway_amt: 0.02,
                    });
                }

                if self.p.travelers {
                    let tx = clamp_f(oa.x - oa.rx * 1.48, 90.0, (wc - 90.0).max(90.0));
                    let ty = clamp_f(oa.y + oa.ry * 0.45, 80.0, (hc - 100.0).max(80.0));
                    self.props.push(Prop {
                        x: tx,
                        y: ty,
                        col: 4,
                        size: 92.0,
                        rot: -0.25 + (oi % 2) as f32 * 0.5,
                        sway_phase: 0.0,
                        sway_amt: 0.0,
                    });
                    self.props.push(Prop {
                        x: tx + 22.0,
                        y: ty + 38.0,
                        col: 3,
                        size: 44.0,
                        rot: (oa.y - ty).atan2(oa.x - tx),
                        sway_phase: r.next_f32() * 6.28,
                        sway_amt: 0.03,
                    });
                }
            }
        }

        // Scattered sandstone outcrops across open dunes
        for _ in 0..9 {
            let x = 70.0 + r.next_f32() * (wc - 140.0).max(100.0);
            let y = 70.0 + r.next_f32() * (hc - 160.0).max(100.0);
            if self.p.adaptive && !self.field_b.is_empty() && self.field_at(x, y) > -8.0 {
                continue;
            }
            let mut in_lake = false;
            for oa in &self.oases {
                let dx = (x - oa.x) / oa.rx.max(1.0);
                let dy = (y - oa.y) / oa.ry.max(1.0);
                if (dx * dx + dy * dy).sqrt() < 1.35 {
                    in_lake = true;
                    break;
                }
            }
            if in_lake {
                continue;
            }
            self.props.push(Prop {
                x,
                y,
                col: 7,
                size: 42.0 + r.next_f32() * 20.0,
                rot: r.next_f32() * 6.28,
                sway_phase: 0.0,
                sway_amt: 0.0,
            });
        }

        self.rebuild_caravans();
        self.rebuild_dust();
        self.props_dirty = false;
    }

    pub fn rebuild_caravans(&mut self) {
        self.caravans.clear();
        let total = self.p.camels;
        if total == 0 {
            self.caravans_dirty = false;
            return;
        }
        let mut r = Rng32::new((self.p.seed as i32).wrapping_add(1303));
        let group_count = if total >= 4 { 2 } else { 1 };
        let per_group = (total + group_count - 1) / group_count;
        let mut created = 0usize;
        for g in 0..group_count {
            let start_y = self.h * if g == 0 { 0.42 } else { 0.74 } + (r.next_f32() - 0.5) * 80.0;
            let base_speed = (16.0 + r.next_f32() * 8.0) * if g == 0 { 1.0 } else { -1.0 };
            let path_amp = 34.0 + r.next_f32() * 26.0;
            let freq = 0.0045 + r.next_f32() * 0.0025;
            let phase = r.next_f32() * 2.0 * PI;
            let base_x = r.next_f32() * self.w;
            let camels = per_group.min(total - created);
            created += camels;
            self.caravans.push(Caravan {
                base_x,
                start_y,
                base_speed,
                path_amp,
                freq,
                phase,
                camels,
                has_guide: self.p.travelers,
            });
        }
        self.caravans_dirty = false;
    }

    pub fn rebuild_dust(&mut self) {
        let mut r = Rng32::new((self.p.seed as i32).wrapping_add(2207));
        self.dust.clear();
        let n = if self.low_mode || self.p.quality == 0 { 24 } else { 42 };
        for _ in 0..n {
            self.dust.push(Dust {
                x: r.next_f32() * self.w.max(1.0),
                y: r.next_f32() * self.h.max(1.0),
                spd: 42.0 + r.next_f32() * 58.0,
                size: 48.0 + r.next_f32() * 64.0,
                alpha: 0.14 + r.next_f32() * 0.20,
                phase: r.next_f32() * 6.28,
            });
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.rebuild_dust();
        }
    }
}

// =========================================================================
// GPU renderer
// =========================================================================
pub struct DesertRenderer {
    prog_bg: GLuint,
    prog_sprite: GLuint,
    empty_vao: GLuint,
    quad_vao: GLuint,
    quad_vbo: GLuint,
    inst_a: GLuint,
    inst_b: GLuint,
    field_tex: [GLuint; 2],
    noise_tex: GLuint,
    atlas_tex: GLuint,
    u_res: GLint,
    u_grid: GLint,
    u_time: GLint,
    u_scale: GLint,
    u_morph: GLint,
    u_warmth: GLint,
    u_wind: GLint,
    u_debug: GLint,
    u_seed: GLint,
    u_adaptive: GLint,
    u_oases_count: GLint,
    u_oases: GLint,
    u_pulse: GLint,
    u_field_a: GLint,
    u_field_b: GLint,
    u_noise: GLint,
    s_res: GLint,
    s_atlas: GLint,
    inst_data_a: Vec<f32>,
    inst_data_b: Vec<f32>,
    noise_seed: u32,
}

fn f32_to_f16(v: f32) -> u16 {
    let x = v.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let mut e = (((x >> 23) & 0xFF) as i32) - 112;
    let m = x & 0x7F_FFFF;
    if e <= 0 {
        return sign;
    }
    if e >= 31 {
        return sign | 0x7C00;
    }
    e = e.min(31);
    sign | ((e as u16) << 10) | ((m >> 13) as u16)
}

pub fn build_noise(seed: u32) -> Vec<u8> {
    const S: usize = 256;
    const N: usize = S * S;
    let mut chans: Vec<Vec<f32>> = Vec::with_capacity(3);
    for c in 0..3u32 {
        let mut r = Rng32::new((seed as i32).wrapping_add(((c + 1) * 100) as i32));
        let mut a = vec![0.0f32; N];
        for v in a.iter_mut() {
            *v = r.next_f32();
        }
        // one 5-tap blur pass, exactly like makeNoiseImage()
        let mut b = vec![0.0f32; N];
        for y in 0..S {
            for x in 0..S {
                let up = ((y + S - 1) & 255) * S + x;
                let dn = ((y + 1) & 255) * S + x;
                let lf = y * S + ((x + S - 1) & 255);
                let rt = y * S + ((x + 1) & 255);
                b[y * S + x] = (a[up] + a[lf] + a[y * S + x] * 2.0 + a[rt] + a[dn]) / 6.0;
            }
        }
        chans.push(b);
    }
    let mut d = vec![0u8; N * 4];
    for i in 0..N {
        d[i * 4] = (chans[0][i] * 255.0).round().clamp(0.0, 255.0) as u8;
        d[i * 4 + 1] = (chans[1][i] * 255.0).round().clamp(0.0, 255.0) as u8;
        d[i * 4 + 2] = (chans[2][i] * 255.0).round().clamp(0.0, 255.0) as u8;
        d[i * 4 + 3] = 255;
    }
    d
}

impl DesertRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_bg = gl.compile_program(DS_BG_VERT, DS_BG_FRAG)?;
        let prog_sprite = gl.compile_program(DS_SPRITE_VERT, DS_SPRITE_FRAG)?;

        let mut empty_vao = 0;
        (gl.GenVertexArrays)(1, &mut empty_vao);

        let mut quad_vao = 0;
        let mut quad_vbo = 0;
        (gl.GenVertexArrays)(1, &mut quad_vao);
        (gl.GenBuffers)(1, &mut quad_vbo);
        (gl.BindVertexArray)(quad_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, quad_vbo);
        let quad: [f32; 8] = [-0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, 0.5];
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            std::mem::size_of_val(&quad) as GLsizeiptr,
            quad.as_ptr() as *const c_void,
            GL_STATIC_DRAW,
        );
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, 8, 0 as *const c_void);

        let mut inst_a = 0;
        let mut inst_b = 0;
        (gl.GenBuffers)(1, &mut inst_a);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_a);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (MAX_INST * 16) as GLsizeiptr,
            std::ptr::null(),
            GL_DYNAMIC_DRAW,
        );
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, 16, 0 as *const c_void);
        (gl.VertexAttribDivisor)(1, 1);

        (gl.GenBuffers)(1, &mut inst_b);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_b);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (MAX_INST * 16) as GLsizeiptr,
            std::ptr::null(),
            GL_DYNAMIC_DRAW,
        );
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 4, GL_FLOAT, GL_FALSE, 16, 0 as *const c_void);
        (gl.VertexAttribDivisor)(2, 1);
        (gl.BindVertexArray)(0);

        let mut field_tex = [0u32; 2];
        (gl.GenTextures)(2, field_tex.as_mut_ptr());
        for t in field_tex.iter() {
            (gl.BindTexture)(GL_TEXTURE_2D, *t);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        }

        let mut noise_tex = 0;
        (gl.GenTextures)(1, &mut noise_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, noise_tex);
        let no_mip = std::env::var("DDGL_DESERT_NO_MIP").is_ok();
        (gl.TexParameteri)(
            GL_TEXTURE_2D,
            GL_TEXTURE_MIN_FILTER,
            if no_mip { GL_LINEAR } else { GL_LINEAR_MIPMAP_LINEAR },
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);

        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        let baked = bake_atlas();
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            baked.w,
            baked.h,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            baked.px.as_ptr() as *const c_void,
        );
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        (gl.BindTexture)(GL_TEXTURE_2D, 0);

        Ok(Self {
            prog_bg,
            prog_sprite,
            empty_vao,
            quad_vao,
            quad_vbo,
            inst_a,
            inst_b,
            field_tex,
            noise_tex,
            atlas_tex,
            u_res: gl.uniform_loc(prog_bg, "uRes"),
            u_grid: gl.uniform_loc(prog_bg, "uGrid"),
            u_time: gl.uniform_loc(prog_bg, "uTime"),
            u_scale: gl.uniform_loc(prog_bg, "uScale"),
            u_morph: gl.uniform_loc(prog_bg, "uMorph"),
            u_warmth: gl.uniform_loc(prog_bg, "uWarmth"),
            u_wind: gl.uniform_loc(prog_bg, "uWind"),
            u_debug: gl.uniform_loc(prog_bg, "uDebug"),
            u_seed: gl.uniform_loc(prog_bg, "uSeed"),
            u_adaptive: gl.uniform_loc(prog_bg, "uAdaptive"),
            u_oases_count: gl.uniform_loc(prog_bg, "uOasesCount"),
            u_oases: gl.uniform_loc(prog_bg, "uOases"),
            u_pulse: gl.uniform_loc(prog_bg, "uPulse"),
            u_field_a: gl.uniform_loc(prog_bg, "uFieldA"),
            u_field_b: gl.uniform_loc(prog_bg, "uFieldB"),
            u_noise: gl.uniform_loc(prog_bg, "uNoise"),
            s_res: gl.uniform_loc(prog_sprite, "uRes"),
            s_atlas: gl.uniform_loc(prog_sprite, "uAtlas"),
            inst_data_a: vec![0.0; MAX_INST * 4],
            inst_data_b: vec![0.0; MAX_INST * 4],
            noise_seed: u32::MAX,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.prog_bg != 0 {
            (gl.DeleteProgram)(self.prog_bg);
            self.prog_bg = 0;
        }
        if self.prog_sprite != 0 {
            (gl.DeleteProgram)(self.prog_sprite);
            self.prog_sprite = 0;
        }
        if self.empty_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.empty_vao);
            self.empty_vao = 0;
        }
        if self.quad_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.quad_vao);
            self.quad_vao = 0;
        }
        if self.quad_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.quad_vbo);
            self.quad_vbo = 0;
        }
        if self.inst_a != 0 {
            (gl.DeleteBuffers)(1, &self.inst_a);
            self.inst_a = 0;
        }
        if self.inst_b != 0 {
            (gl.DeleteBuffers)(1, &self.inst_b);
            self.inst_b = 0;
        }
        (gl.DeleteTextures)(2, self.field_tex.as_ptr());
        self.field_tex = [0, 0];
        if self.noise_tex != 0 {
            (gl.DeleteTextures)(1, &self.noise_tex);
            self.noise_tex = 0;
        }
        if self.atlas_tex != 0 {
            (gl.DeleteTextures)(1, &self.atlas_tex);
            self.atlas_tex = 0;
        }
    }

    unsafe fn upload_field(&self, gl: &GlFns, slot: usize, gw: usize, gh: usize, field: &[f32]) {
        let mut half = vec![0u16; gw * gh];
        for (i, v) in field.iter().enumerate() {
            half[i] = f32_to_f16(*v);
        }
        (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[slot]);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_R16F as GLint,
            gw as GLsizei,
            gh as GLsizei,
            0,
            GL_RED,
            GL_HALF_FLOAT,
            half.as_ptr() as *const c_void,
        );
    }

    unsafe fn upload_noise(&mut self, gl: &GlFns, seed: u32) {
        let data = build_noise(seed);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            256,
            256,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            data.as_ptr() as *const c_void,
        );
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        self.noise_seed = seed;
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let (w, h) = css_dims(st);
        let scale = st.dpi_scale.max(0.5);

        st.desert.resize(w, h);
        st.desert.set_low_mode(st.low_mode);

        if st.desert.noise_dirty || self.noise_seed != st.desert.p.seed {
            self.upload_noise(gl, st.desert.p.seed);
            st.desert.noise_dirty = false;
        }

        if st.desert.field_dirty {
            st.desert.commit(false);
        }

        let gw = st.desert.gw.max(1);
        let gh = st.desert.gh.max(1);
        if st.desert.dirty_slots[0] {
            let f = st.desert.field_a.clone();
            self.upload_field(gl, 0, gw, gh, &f);
            st.desert.dirty_slots[0] = false;
        }
        if st.desert.dirty_slots[1] {
            let f = st.desert.field_b.clone();
            self.upload_field(gl, 1, gw, gh, &f);
            st.desert.dirty_slots[1] = false;
        }
        if st.desert.props_dirty {
            st.desert.rebuild_props();
        }
        if st.desert.caravans_dirty {
            st.desert.rebuild_caravans();
        }

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);

        let t = st.desert.time;
        let morph = if st.desert.morph_dur > 0.0 {
            clamp_f((t - st.desert.morph_t0) / st.desert.morph_dur, 0.0, 1.0)
        } else {
            1.0
        };
        let morph = morph * morph * (3.0 - 2.0 * morph);

        (gl.UseProgram)(self.prog_bg);
        (gl.BindVertexArray)(self.empty_vao);
        (gl.Uniform2f)(self.u_res, pw as f32, ph as f32);
        (gl.Uniform2f)(self.u_grid, gw as f32 * FIELD_STEP, gh as f32 * FIELD_STEP);
        (gl.Uniform1f)(self.u_time, t);
        (gl.Uniform1f)(self.u_scale, scale);
        (gl.Uniform1f)(self.u_morph, morph);
        (gl.Uniform1f)(self.u_warmth, st.desert.p.warmth);
        (gl.Uniform1f)(self.u_wind, st.desert.p.wind);
        (gl.Uniform1f)(self.u_debug, if st.desert.p.debug { 1.0 } else { 0.0 });
        (gl.Uniform1f)(self.u_seed, (st.desert.p.seed % 1000) as f32 * 0.37);
        (gl.Uniform1f)(self.u_adaptive, if st.desert.p.adaptive { 1.0 } else { 0.0 });
        (gl.Uniform4f)(
            self.u_pulse,
            st.desert.pulse[0],
            st.desert.pulse[1],
            st.desert.pulse[2],
            st.desert.pulse[3],
        );

        let mut flat = [0.0f32; 16];
        for (i, oa) in st.desert.oases.iter().take(4).enumerate() {
            flat[i * 4] = oa.x;
            flat[i * 4 + 1] = oa.y;
            flat[i * 4 + 2] = oa.rx;
            flat[i * 4 + 3] = oa.ry;
        }
        (gl.Uniform1i)(self.u_oases_count, st.desert.oases.len().min(4) as GLint);
        (gl.Uniform4fv)(self.u_oases, 4, flat.as_ptr());

        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[st.desert.from_i]);
        (gl.Uniform1i)(self.u_field_a, 0);
        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[st.desert.to_i]);
        (gl.Uniform1i)(self.u_field_b, 1);
        (gl.ActiveTexture)(GL_TEXTURE2);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.Uniform1i)(self.u_noise, 2);
        (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);

        // ---- instanced sprite batch (same ordering as desert.html draw()) ----
        let mut n = 0usize;
        {
            let a = &mut self.inst_data_a;
            let b = &mut self.inst_data_b;
            let mut push = |x: f32, y: f32, size: f32, rot: f32, col: usize, alpha: f32| {
                if n >= MAX_INST || alpha <= 0.01 {
                    return;
                }
                let i4 = n * 4;
                a[i4] = x * scale;
                a[i4 + 1] = y * scale;
                a[i4 + 2] = size * scale;
                a[i4 + 3] = rot;
                b[i4] = col as f32;
                b[i4 + 1] = alpha;
                b[i4 + 2] = 0.0;
                b[i4 + 3] = 0.0;
                n += 1;
            };
            let wind = st.desert.p.wind;

            for sp in st.desert.props.iter() {
                if sp.col == 5 || sp.col == 6 {
                    continue;
                }
                let sw = if sp.sway_amt != 0.0 {
                    (t * (1.2 + wind * 0.7) + sp.sway_phase).sin() * sp.sway_amt
                } else {
                    0.0
                };
                push(sp.x, sp.y, sp.size, sp.rot + sw, sp.col, 1.0);
            }

            for (ci, cvn) in st.desert.caravans.iter().enumerate() {
                let dir = if cvn.base_speed >= 0.0 { 1.0 } else { -1.0 };
                let span = w + 340.0;
                let step_spacing = 62.0;
                let lead = ((cvn.base_x + t * cvn.base_speed * wind.max(0.0) + 170.0) % span + span) % span - 170.0;
                let guide_x = if dir > 0.0 { lead } else { w - lead };
                let guide_y = cvn.start_y + (guide_x * cvn.freq + cvn.phase).sin() * cvn.path_amp;
                let slope = (guide_x * cvn.freq + cvn.phase).cos() * cvn.path_amp * cvn.freq;
                let heading = if dir > 0.0 { slope.atan() } else { PI + slope.atan() };

                if cvn.has_guide {
                    let g_bob = (t * 4.5 * wind.max(0.2) + ci as f32).sin() * 0.08;
                    push(guide_x, guide_y, 48.0, heading + g_bob, 2, 1.0);
                }

                for k in 0..cvn.camels {
                    let offset = (k as f32 + 1.0) * step_spacing;
                    let cx = guide_x - heading.cos() * offset;
                    let cy = cvn.start_y + (cx * cvn.freq + cvn.phase).sin() * cvn.path_amp;
                    let c_slope = (cx * cvn.freq + cvn.phase).cos() * cvn.path_amp * cvn.freq;
                    let c_head = if dir > 0.0 { c_slope.atan() } else { PI + c_slope.atan() };
                    let stride_flip = ((t * (2.6 * wind.max(0.2)) + k as f32) as i32).rem_euclid(2);
                    let wobble = (t * 3.1 + k as f32 * 1.4).sin() * 0.04;
                    push(cx, cy, 72.0, c_head + wobble, (k as i32 + stride_flip).rem_euclid(2) as usize, 1.0);
                }
            }

            for sp in st.desert.props.iter() {
                if sp.col != 5 && sp.col != 6 {
                    continue;
                }
                let sw = (t * (1.1 + wind * 0.9) + sp.sway_phase).sin() * sp.sway_amt * wind.max(0.3);
                push(sp.x, sp.y, sp.size, sp.rot + sw, sp.col, 1.0);
            }

            for dw in st.desert.dust.iter() {
                let x = ((dw.x + t * dw.spd * wind.max(0.15)) % (w + 140.0)) - 70.0;
                let y = ((dw.y + (t * 0.6 + dw.phase).sin() * 18.0 + t * 8.0 * wind.max(0.1)) % (h + 100.0)) - 50.0;
                push(
                    x,
                    y,
                    dw.size,
                    -0.18 + (t * 0.8 + dw.phase).sin() * 0.05,
                    9,
                    dw.alpha * (0.4 + 0.6 * st.desert.p.warmth),
                );
            }
        }

        if n > 0 {
            (gl.Enable)(GL_BLEND);
            (gl.BlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            (gl.UseProgram)(self.prog_sprite);
            (gl.BindVertexArray)(self.quad_vao);
            (gl.Uniform2f)(self.s_res, pw as f32, ph as f32);
            (gl.ActiveTexture)(GL_TEXTURE0);
            (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
            (gl.Uniform1i)(self.s_atlas, 0);

            (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_a);
            (gl.BufferSubData)(
                GL_ARRAY_BUFFER,
                0,
                (n * 16) as GLsizeiptr,
                self.inst_data_a.as_ptr() as *const c_void,
            );
            (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_b);
            (gl.BufferSubData)(
                GL_ARRAY_BUFFER,
                0,
                (n * 16) as GLsizeiptr,
                self.inst_data_b.as_ptr() as *const c_void,
            );
            (gl.DrawArraysInstanced)(GL_TRIANGLE_STRIP, 0, 4, n as GLsizei);
            (gl.BindVertexArray)(0);
            (gl.Disable)(GL_BLEND);
        }
        (gl.BindVertexArray)(0);
    }
}
