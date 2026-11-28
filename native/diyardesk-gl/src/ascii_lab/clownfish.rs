//! 01 · Clownfish & Anemone — 1:1 faithful interactive Rust port of
//! `sapozhkov/ascii-lab-public/01-clownfish/clownfish.py` + `tail_sway.py`.

use super::grid_gl::{glyph_id, line_glyph_cell, ramp_glyph, AsciiCanvas};
use super::{clamp_u8, scale_rgb, AsciiLabParams};
use crate::state::Rect;
use std::f32::consts::{PI, TAU};

const LOOP_SECS: f32 = 22.0;
const PECT_HZ: f32 = 1.3181818; // round(1.3 * 22) / 22
const STEP: f32 = 1.0 / 60.0;
const TAIL_ROOT: f32 = 0.53;
const CAM_D: f32 = 7.0;

const RAMP: &[u8] = b"$QB%8&0#@oauenjzt[]{}<>rcvx+=1il!;:," ;
const FILL_G: &[u8] = b".::;;ioo08";
const ROCK_G: &[u8] = b" . .:.:;:=+";

const OWN_WATER: u8 = 0;
const OWN_FISH: u8 = 1;
const OWN_FIN: u8 = 2;
const OWN_WEED: u8 = 3;
const OWN_BACK: u8 = 4;

const PROFILE_S: [f32; 11] = [0.0, 0.03, 0.08, 0.16, 0.30, 0.45, 0.60, 0.75, 0.88, 0.96, 1.0];
const PROFILE_H: [f32; 11] = [0.0, 0.075, 0.125, 0.175, 0.222, 0.238, 0.218, 0.165, 0.10, 0.078, 0.075];

const FLOOR_Y: f32 = -1.42;
const MOUND: [f32; 3] = [1.10, 0.36, 0.80];
const LAYOUT_WEED: [f32; 3] = [0.05, -1.02, 0.40];
const PIVOT: [f32; 3] = [0.05, -0.55, 0.40];
const ORBIT: f32 = 0.24;
const TILT: f32 = 0.22;

const ARMS: usize = 50;
const NODES: usize = 14;

#[derive(Clone, Copy)]
struct Rng64(u64);
impl Rng64 {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let x = self.0;
        x ^ (x >> 27)
    }
    fn f32(&mut self) -> f32 {
        ((self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
    fn normal_approx(&mut self, sd: f32) -> f32 {
        let s = self.f32() + self.f32() + self.f32() - 1.5;
        s * sd * 1.15
    }
}

#[inline]
fn ease(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    (3.0 - 2.0 * t) * t * t
}

#[inline]
fn hash01(a: f32, b: f32) -> f32 {
    ((a * 127.1 + b * 311.7).sin() * 43758.5453).abs().fract()
}

#[inline]
fn interp_profile(s: f32) -> f32 {
    let x = s.clamp(0.0, 1.0);
    for i in 0..(PROFILE_S.len() - 1) {
        if x <= PROFILE_S[i + 1] {
            let t = (x - PROFILE_S[i]) / (PROFILE_S[i + 1] - PROFILE_S[i]).max(1e-6);
            return PROFILE_H[i] + t * (PROFILE_H[i + 1] - PROFILE_H[i]);
        }
    }
    PROFILE_H[PROFILE_H.len() - 1]
}

#[inline]
fn half_height(s: f32) -> f32 {
    interp_profile(s)
}

#[inline]
fn half_width(s: f32) -> f32 {
    half_height(s) * (0.58 - 0.22 * s)
}

#[inline]
fn profile_slope(s: f32) -> f32 {
    let ds = 0.01;
    (interp_profile((s + ds).min(1.0)) - interp_profile((s - ds).max(0.0))) / (2.0 * ds)
}

/// Exact `tail_sway.py` wave + turn curvature.
#[inline]
pub fn fish_sway(s: f32, phase: f32, dt_back: f32, hz: f32, effort: f32, turn: f32) -> f32 {
    let beat = phase - TAU * hz.min(1.0) * dt_back;
    let wave = (0.014 + 0.105 * s * s) * (beat - 3.4 * s).sin();
    let lag = s - 0.25;
    let bend = lag * lag.abs();
    effort * wave + turn * bend
}

#[inline]
fn rot_y(p: [f32; 3], a: f32) -> [f32; 3] {
    let (c, s) = (a.cos(), a.sin());
    [c * p[0] + s * p[2], p[1], -s * p[0] + c * p[2]]
}

#[inline]
fn rot_x(p: [f32; 3], a: f32) -> [f32; 3] {
    let (c, s) = (a.cos(), a.sin());
    [p[0], c * p[1] - s * p[2], s * p[1] + c * p[2]]
}

#[inline]
fn rot_z(p: [f32; 3], a: f32) -> [f32; 3] {
    let (c, s) = (a.cos(), a.sin());
    [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
}

#[inline]
fn norm3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[inline]
fn unit3(v: [f32; 3]) -> [f32; 3] {
    let l = norm3(v).max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}

#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[derive(Clone, Copy)]
struct TrackSample {
    place: [f32; 3],
    a: f32,
    th: f32,
    v: f32,
    e: f32,
    phi: f32,
    hz: f32,
    spin: f32,
    slowing: f32,
    cam_x: f32,
    cam_y: f32,
    cam_z: f32,
    lead: f32,
    zoom: f32,
    prey: [f32; 3],
    food: f32,
}

#[derive(Clone)]
struct RockPt {
    p: [f32; 3],
    col: [f32; 3],
    lum: f32,
}

#[derive(Clone)]
struct CoralDef {
    world: [f32; 3],
    size: f32,
    col: [f32; 3],
    seed: u64,
    twigs: Vec<[f32; 5]>, // x0, y0, x1, y1, level
    buds: Vec<(f32, f32, bool)>,
}

struct UserMorsel {
    world: [f32; 3],
    age: f32,
    active: bool,
    eaten_timer: f32,
    eaten_at: [f32; 3],
}

pub struct AsciiClownfishState {
    pub params: AsciiLabParams,
    pub canvas: AsciiCanvas,
    pub anim_time: f32,
    pub low_mode: bool,
    pub icons: Vec<Rect>,
    pub icons_vis: bool,
    pub css_w: f32,
    pub css_h: f32,
    pub dpi_scale: f32,
    pointer_norm: [f32; 2],
    pointer_vel: [f32; 2],
    pointer_active: bool,
    pointer_impulse: f32,
    // Precomputed loop track & static world geometry
    track: Vec<TrackSample>,
    rock_pts: Vec<RockPt>,
    corals: Vec<CoralDef>,
    motes: Vec<[f32; 3]>,
    dots: Vec<[f32; 3]>,
    float_motes: Vec<[f32; 4]>,
    // Live anemone arms & bioluminescence
    arm_rest: Vec<[[f32; 3]; NODES]>,
    arm_side: Vec<[f32; 3]>,
    arm_fore: Vec<[f32; 3]>,
    arm_x: Vec<[[f32; 3]; NODES]>,
    arm_v: Vec<[[f32; 3]; NODES]>,
    arm_link: Vec<[f32; NODES - 1]>,
    arm_beat: Vec<[i32; 2]>,
    arm_phase: Vec<[f32; 2]>,
    arm_back: Vec<f32>,
    arm_glow: Vec<[f32; NODES]>,
    mote_lit: Vec<f32>,
    mote_off: Vec<[f32; 3]>,
    // Interactive user food & fish chase offset
    user_morsel: UserMorsel,
    fish_chase_blend: f32,
    fish_chase_offset: [f32; 3],
    fish_boost: f32,
    last_st_idx: usize,
    frame_idx: u64,
}

impl Default for AsciiClownfishState {
    fn default() -> Self {
        Self::new(1280.0, 720.0, 1.0)
    }
}

fn build_swim_track() -> Vec<TrackSample> {
    // Faithful port of `swim_round()` from `clownfish.py`
    const ROUND: [([f32; 3], f32, f32, f32); 13] = [
        ([-0.50, 0.42, 0.02], 0.55, 0.0, 0.30),
        ([-1.30, 0.14, -0.38], 0.65, 0.0, 0.34),
        ([-1.72, -0.28, -0.80], 0.60, 0.0, 0.30),
        ([-1.30, -0.58, -1.08], 0.80, 0.0, 0.30),
        ([-0.70, -0.50, -1.14], 2.10, 0.0, 0.30),
        ([0.30, -0.42, -1.10], 1.15, 0.0, 0.34),
        ([1.50, -0.10, -0.60], 0.95, 0.0, 0.34),
        ([1.85, 0.35, 0.10], 0.60, 0.0, 0.30),
        ([1.25, 0.66, 0.62], 0.75, 0.0, 0.34),
        ([0.16, 0.05, 0.27], 0.65, 0.0, 0.16),
        ([0.04, 0.01, 0.10], 0.55, 0.0, 0.15),
        ([0.22, 0.00, 0.04], 0.55, 0.0, 0.15),
        ([0.30, 0.04, 0.24], 0.55, 0.25, 0.15),
    ];
    let morsel_at = |age: f32| -> [f32; 3] {
        [
            -0.02 - 0.020 * age + 0.035 * (2.3 * age).sin(),
            -0.17 - 0.055 * age,
            -1.14 + 0.02 * (1.7 * age + 1.0).sin(),
        ]
    };

    let mut p = ROUND[0].0;
    let mut a = PI + 0.5;
    let mut th = 0.0f32;
    let mut v = 0.0f32;
    let mut e = 0.0f32;
    let mut phi = 0.0f32;
    let mut k = 1usize;
    let mut rest = 0.0f32;
    let mut circling = 0.0f32;
    let mut food = -1.0f32;
    let mut stare = 0.0f32;

    #[derive(Clone, Copy)]
    struct LogRow {
        vals: [f32; 12], // x,y,z, a,th,v,e,phi, px,py,pz, food
    }

    let mut log: Vec<LogRow> = Vec::with_capacity(60 * 120);
    let mut homes: Vec<usize> = Vec::new();
    while homes.len() < 3 && log.len() < 60 * 240 {
        let mark = if rest > 0.0 { ROUND[(k + 1) % ROUND.len()] } else { ROUND[k] };
        let hunting = k == 4 && food >= 0.0 && rest <= 0.0;
        if food >= 0.0 {
            food += STEP;
        }
        let target = if hunting { morsel_at(food) } else { mark.0 };
        let to = [target[0] - p[0], target[1] - p[1], target[2] - p[2]];
        let dist = norm3(to);
        let dir = [a.cos() * th.cos(), th.sin(), a.sin() * th.cos()];
        let mouth = [p[0] + 0.45 * dir[0], p[1] + 0.45 * dir[1], p[2] + 0.45 * dir[2]];
        let mut want;
        if rest > 0.0 {
            rest -= STEP;
            want = 0.0;
            if rest <= 0.0 {
                k = (k + 1) % ROUND.len();
            }
        } else {
            want = mark.1 * if mark.2 > 0.0 { (dist / 0.35).clamp(0.5, 1.0) } else { 1.0 };
            if hunting && stare > 0.0 {
                stare -= STEP;
                want = 0.05;
            }
            let ahead = to[0] * a.cos() + to[2] * a.sin();
            circling = if dist < 3.0 * mark.3 { circling + STEP } else { 0.0 };
            let caught = if hunting {
                let m = morsel_at(food);
                norm3([m[0] - mouth[0], m[1] - mouth[1], m[2] - mouth[2]]) < 0.12
            } else {
                dist < mark.3
            };
            if caught || (mark.2 > 0.0 && dist < 2.5 * mark.3 && ahead < 0.0) || circling > 1.4 {
                circling = 0.0;
                if hunting {
                    food = -1.0;
                } else if k == 1 {
                    food = 0.0;
                }
                if k == 0 {
                    homes.push(log.len());
                }
                rest = mark.2;
                if rest <= 0.0 {
                    k = (k + 1) % ROUND.len();
                    stare = if k == 4 && food >= 0.0 { 0.55 } else { 0.0 };
                }
            }
        }

        let swing = (to[2].atan2(to[0]) - a + PI).rem_euclid(TAU) - PI;
        let mut eager = if rest > 0.0 { 0.35 } else { 0.5 + 2.4 * e };
        if stare > 0.0 {
            eager = eager.max(2.2);
        }
        want *= 0.40 + 0.60 * swing.cos().max(0.0).powi(2);
        a += (swing * eager).clamp(-1.9, 1.9) * STEP;
        let climb_target = 0.8 * (to[1] / dist.max(1e-6)).clamp(-0.6, 0.6).asin()
            * if rest > 0.0 { 0.25 } else { 1.0 };
        th += (climb_target - th) * 0.7 * eager * STEP;
        let rate_e = if want < e { 2.2 } else { 1.1 + 2.2 * (want - 1.2).max(0.0) };
        e += (want - e) * rate_e * STEP;
        let hz = 0.70 + 1.05 * e;
        phi += TAU * hz * STEP;
        let push = 2.1 * e * (0.30 + 0.70 * (phi - 0.9).sin().powi(2));
        v += (push - 2.2 * v * v.abs() - 0.6 * v) * STEP;
        let step_dir = [a.cos() * th.cos(), th.sin(), a.sin() * th.cos()];
        p[0] += v * STEP * step_dir[0];
        p[1] += v * STEP * step_dir[1];
        p[2] += v * STEP * step_dir[2];

        let m_pos = if food >= 0.0 { morsel_at(food) } else { p };
        log.push(LogRow {
            vals: [
                p[0], p[1], p[2], a, th, v, e, phi, m_pos[0], m_pos[1], m_pos[2], food,
            ],
        });
    }

    let i0 = *homes.get(1).unwrap_or(&0);
    let i1 = *homes.get(2).unwrap_or(&(log.len().saturating_sub(1)));
    let raw_lap = &log[i0..=i1.min(log.len() - 1)];
    let n = (LOOP_SECS / STEP).round() as usize;
    let m = raw_lap.len().max(2);

    let mut lap = Vec::with_capacity(n);
    for idx in 0..n {
        let at = (idx as f32 / n as f32) * (m - 1) as f32;
        let lo = (at.floor() as usize).min(m - 2);
        let w = at - lo as f32;
        let mut vals = [0.0f32; 12];
        for c in 0..12 {
            vals[c] = raw_lap[lo].vals[c] * (1.0 - w) + raw_lap[lo + 1].vals[c] * w;
        }
        if raw_lap[lo].vals[11] < 0.0 || raw_lap[lo + 1].vals[11] < 0.0 {
            vals[11] = -1.0;
        }
        lap.push(vals);
    }

    // Close tiny loop seam smoothly
    let gap_a = (lap[n - 1][3] - lap[0][3] + PI).rem_euclid(TAU) - PI;
    let gap_phi_rem = lap[n - 1][7] - lap[0][7];
    let gap_phi = gap_phi_rem - TAU * (gap_phi_rem / TAU).round();
    for (idx, row) in lap.iter_mut().enumerate() {
        let ramp = idx as f32 / n as f32;
        row[3] -= ramp * gap_a;
        row[7] -= ramp * gap_phi;
    }

    let follow = |src: &[f32], tau: f32| -> Vec<f32> {
        let mut val = src[0];
        let mut out = vec![0.0f32; n];
        for pass in 0..2 {
            for i in 0..n {
                val += (src[i] - val) * STEP / tau;
                if pass == 1 {
                    out[i] = val;
                }
            }
        }
        out
    };

    let mut raw_cx = Vec::with_capacity(n);
    let mut raw_cy = Vec::with_capacity(n);
    let mut raw_cz = Vec::with_capacity(n);
    let mut raw_lead = Vec::with_capacity(n);
    let mut raw_z1 = Vec::with_capacity(n);
    let mut raw_z2 = Vec::with_capacity(n);
    for row in &lap {
        let eyes_on = ease(((row[11] - 0.5) / 2.6).clamp(0.0, 1.0));
        let gap = [
            (row[8] - row[0]) * eyes_on,
            (row[9] - row[1]) * eyes_on,
            (row[10] - row[2]) * eyes_on,
        ];
        let wide = 1.0 - 0.36 * (norm3(gap) / 1.2).clamp(0.0, 1.0);
        raw_cx.push(row[0] + 0.5 * gap[0]);
        raw_cy.push(row[1] + 0.5 * gap[1]);
        raw_cz.push(row[2] + 0.5 * gap[2]);
        raw_lead.push(row[3].cos());
        let zv = 1.34 - 0.52 * (row[5] / 0.75).clamp(0.0, 1.0) - 0.22 * ((row[5] - 0.58) / 0.28).clamp(0.0, 1.0);
        raw_z1.push(zv);
        raw_z2.push(wide);
    }
    let cam_x = follow(&raw_cx, 0.55);
    let cam_y = follow(&raw_cy, 0.8);
    let cam_z = follow(&raw_cz, 0.9);
    let lead = follow(&raw_lead, 0.9);
    let z1 = follow(&raw_z1, 1.0);
    let z2 = follow(&raw_z2, 0.25);

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let ip = (i + 1) % n;
        let im = (i + n - 1) % n;
        let dphi = lap[ip][7] - lap[im][7];
        let hz = (dphi / (2.0 * STEP)) / TAU;
        let da = (lap[ip][3] - lap[im][3] + PI).rem_euclid(TAU) - PI;
        let spin = da / (2.0 * STEP);
        let dv = lap[ip][5] - lap[im][5];
        let slowing = (-dv / (2.0 * STEP)).max(0.0);
        out.push(TrackSample {
            place: [lap[i][0], lap[i][1], lap[i][2]],
            a: lap[i][3],
            th: lap[i][4],
            v: lap[i][5],
            e: lap[i][6],
            phi: lap[i][7],
            hz,
            spin,
            slowing,
            cam_x: cam_x[i],
            cam_y: cam_y[i],
            cam_z: cam_z[i],
            lead: lead[i],
            zoom: z1[i] * z2[i],
            prey: [lap[i][8], lap[i][9], lap[i][10]],
            food: lap[i][11],
        });
    }
    out
}

fn build_corals() -> Vec<CoralDef> {
    const DEFS: [([f32; 3], f32, [f32; 3], u64); 6] = [
        ([-2.25, FLOOR_Y, 0.85], 0.34, [236.0 / 255.0, 96.0 / 255.0, 132.0 / 255.0], 3),
        ([-2.85, FLOOR_Y, 0.45], 0.24, [240.0 / 255.0, 150.0 / 255.0, 90.0 / 255.0], 8),
        ([2.60, FLOOR_Y + 0.72, 1.25], 0.27, [80.0 / 255.0, 200.0 / 255.0, 200.0 / 255.0], 5),
        ([1.0, FLOOR_Y, 2.3], 0.30, [180.0 / 255.0, 110.0 / 255.0, 220.0 / 255.0], 12),
        ([-0.9, FLOOR_Y + 0.5, 3.4], 0.30, [236.0 / 255.0, 96.0 / 255.0, 132.0 / 255.0], 21),
        ([4.3, FLOOR_Y, 0.9], 0.25, [240.0 / 255.0, 150.0 / 255.0, 90.0 / 255.0], 30),
    ];
    let mut out = Vec::with_capacity(DEFS.len());
    for (world, size, col, seed) in DEFS {
        let mut rng = Rng64::new(seed);
        let mut twigs = Vec::new();
        let mut buds = Vec::new();
        fn grow(
            rng: &mut Rng64,
            twigs: &mut Vec<[f32; 5]>,
            buds: &mut Vec<(f32, f32, bool)>,
            x: f32,
            y: f32,
            ang: f32,
            len: f32,
            level: i32,
        ) {
            if level == 0 {
                buds.push((x, y, rng.f32() < 0.6));
                return;
            }
            let x1 = x + len * ang.sin();
            let y1 = y + len * ang.cos();
            twigs.push([x, y, x1, y1, level as f32]);
            let spread = 0.35 + 0.3 * rng.f32();
            let l1 = len * (0.62 + 0.2 * rng.f32());
            let sp2 = spread * (0.7 + 0.5 * rng.f32());
            let l2 = len * (0.62 + 0.2 * rng.f32());
            grow(rng, twigs, buds, x1, y1, ang - spread, l1, level - 1);
            grow(rng, twigs, buds, x1, y1, ang + sp2, l2, level - 1);
            if rng.f32() < 0.3 {
                let da = rng.normal_approx(0.1);
                grow(rng, twigs, buds, x1, y1, ang + da, len * 0.6, level - 1);
            }
        }
        grow(&mut rng, &mut twigs, &mut buds, 0.0, 0.0, 0.0, 1.0, 5);
        out.push(CoralDef {
            world,
            size,
            col,
            seed,
            twigs,
            buds,
        });
    }
    out
}

fn build_rocks(stem_col: [f32; 3], light: [f32; 3]) -> Vec<RockPt> {
    const ROCKS: [(f32, f32, f32, f32, f32, f32); 7] = [
        (0.05, 0.42, 1.50, 0.44, 1.10, 0.5),
        (2.25, 0.80, 0.95, 0.80, 0.85, 1.0),
        (-3.3, 1.5, 0.8, 0.5, 0.7, 2.0),
        (1.2, 2.9, 1.4, 0.9, 0.8, 3.0),
        (-1.2, 3.3, 1.1, 0.65, 0.7, 4.0),
        (4.6, 1.9, 0.9, 0.6, 0.7, 5.0),
        (-4.9, 0.4, 0.7, 0.45, 0.6, 6.0),
    ];
    let mut pts = Vec::with_capacity(7 * 18 * 42 + 16 * 48);
    for (cx, cz, rx, ry, rz, seed) in ROCKS {
        for it in 0..18 {
            let th = 0.03 + (it as f32 / 17.0) * 1.52;
            for ip in 0..42 {
                let ph = (ip as f32 / 42.0) * TAU;
                let lump = 1.0 + 0.10 * (5.0 * ph + seed).sin() * (3.0 * th + seed).sin()
                    + 0.06 * (11.0 * ph + 2.0 * seed).sin();
                let n = [th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()];
                let p = [
                    cx + n[0] * lump * rx,
                    FLOOR_Y + n[1] * lump * ry,
                    cz + n[2] * lump * rz,
                ];
                let nu = unit3([n[0] / rx, n[1] / ry, n[2] / rz]);
                let lit = dot3(nu, light).clamp(0.0, 1.0);
                let far = (1.0 - (cz + 1.0) / 6.0).clamp(0.35, 1.0);
                let base = [104.0 / 255.0, 116.0 / 255.0, 150.0 / 255.0];
                let f = (0.20 + 0.50 * lit) * far;
                pts.push(RockPt {
                    p,
                    col: [base[0] * f, base[1] * f, base[2] * f],
                    lum: 0.05 + 0.75 * lit.powf(1.5),
                });
            }
        }
    }
    // Fleshy foot beneath anemone
    for it in 0..16 {
        let th = 0.03 + (it as f32 / 15.0) * 1.52;
        for ip in 0..46 {
            let ph = (ip as f32 / 46.0) * TAU;
            let n = [th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()];
            let p = [
                LAYOUT_WEED[0] + n[0] * MOUND[0],
                LAYOUT_WEED[1] + n[1] * MOUND[1] - 0.06,
                LAYOUT_WEED[2] + n[2] * MOUND[2],
            ];
            let nu = unit3([n[0] / MOUND[0], n[1] / MOUND[1], n[2] / MOUND[2]]);
            let lit = dot3(nu, light).clamp(0.0, 1.0);
            let f = 0.14 + 0.30 * lit;
            pts.push(RockPt {
                p,
                col: [stem_col[0] * f, stem_col[1] * f, stem_col[2] * f],
                lum: 0.10 + 0.25 * lit,
            });
        }
    }
    pts
}

impl AsciiClownfishState {
    pub fn new(css_w: f32, css_h: f32, dpi_scale: f32) -> Self {
        let params = AsciiLabParams::default();
        let light = unit3([-0.45, 0.75, -0.48]);
        let stem_col = [112.0 / 255.0, 94.0 / 255.0, 160.0 / 255.0];

        let track = build_swim_track();
        let rock_pts = build_rocks(stem_col, light);
        let corals = build_corals();

        let mut rng_m = Rng64::new(23);
        let mut motes = Vec::with_capacity(420);
        for _ in 0..270 {
            motes.push([
                rng_m.range(-3.0, 3.1),
                rng_m.range(-1.25, 1.0),
                rng_m.range(-1.7, 1.2),
            ]);
        }
        for _ in 0..150 {
            motes.push([
                rng_m.range(-1.9, 1.9),
                rng_m.range(-0.85, -0.10),
                rng_m.range(-1.55, -0.70),
            ]);
        }

        let mut rng_w = Rng64::new(5);
        let mut dots = Vec::with_capacity(180);
        for _ in 0..180 {
            dots.push([rng_w.f32(), rng_w.f32(), rng_w.f32()]);
        }
        let mut float_motes = Vec::with_capacity(64);
        for _ in 0..64 {
            float_motes.push([rng_w.f32(), rng_w.f32(), rng_w.f32(), rng_w.f32()]);
        }

        let mut rng_a = Rng64::new(11);
        let mut rng_b = Rng64::new(31);
        let mut arm_rest = Vec::with_capacity(ARMS);
        let mut arm_side = Vec::with_capacity(ARMS);
        let mut arm_fore = Vec::with_capacity(ARMS);
        let mut arm_link = Vec::with_capacity(ARMS);
        let mut arm_beat = Vec::with_capacity(ARMS);
        let mut arm_phase = Vec::with_capacity(ARMS);
        let mut arm_back = Vec::with_capacity(ARMS);

        let lon_vals: Vec<f32> = (0..ARMS).map(|_| rng_a.f32() * TAU).collect();
        let lat_vals: Vec<f32> = (0..ARMS).map(|_| 0.06 + 1.12 * rng_a.f32().powf(1.15)).collect();
        let len_vals: Vec<f32> = (0..ARMS).map(|_| 0.60 + 0.42 * rng_a.f32()).collect();
        let _ph_vals: Vec<f32> = (0..ARMS).map(|_| rng_a.f32() * TAU).collect();

        for i in 0..ARMS {
            let curl0 = rng_a.normal_approx(0.42);
            let curl1 = rng_a.normal_approx(0.42);
            let lon = lon_vals[i];
            let lat = lat_vals[i];
            let normal = [lat.sin() * lon.cos(), lat.cos(), lat.sin() * lon.sin()];
            let root = [
                LAYOUT_WEED[0] + normal[0] * MOUND[0],
                LAYOUT_WEED[1] + normal[1] * MOUND[1],
                LAYOUT_WEED[2] + normal[2] * MOUND[2],
            ];
            let aim = unit3([
                normal[0] * 1.05,
                (normal[1] * 0.7 + 0.42).max(0.30),
                normal[2],
            ]);
            let side = unit3(cross3(aim, [0.0, 0.0, 1.0]));
            let fore = cross3(aim, side);
            let length = len_vals[i];
            let mut chain = [[0.0f32; 3]; NODES];
            for j in 0..NODES {
                let u = j as f32 / (NODES - 1) as f32;
                let curve = 2.2 * (u * u - 0.55 * u * u * u);
                let bend = [
                    (side[0] * curl0 + fore[0] * curl1) * curve,
                    (side[1] * curl0 + fore[1] * curl1) * curve,
                    (side[2] * curl0 + fore[2] * curl1) * curve,
                ];
                chain[j] = [
                    root[0] + length * (aim[0] * u + bend[0]),
                    root[1] + length * (aim[1] * u + bend[1] + 0.20 * u * u),
                    root[2] + length * (aim[2] * u + bend[2]),
                ];
            }
            let mut links = [0.0f32; NODES - 1];
            for j in 0..(NODES - 1) {
                links[j] = norm3([
                    chain[j + 1][0] - chain[j][0],
                    chain[j + 1][1] - chain[j][1],
                    chain[j + 1][2] - chain[j][2],
                ]);
            }
            arm_rest.push(chain);
            arm_side.push(side);
            arm_fore.push(fore);
            arm_link.push(links);
            arm_beat.push([4 + (rng_b.next_u64() % 5) as i32, 4 + (rng_b.next_u64() % 5) as i32]);
            arm_phase.push([rng_b.f32() * TAU, rng_b.f32() * TAU]);
            arm_back.push((root[2] - LAYOUT_WEED[2] + MOUND[2]) / (2.0 * MOUND[2]));
        }

        let arm_x = arm_rest.clone();
        let arm_v = vec![[[0.0f32; 3]; NODES]; ARMS];
        let arm_glow = vec![[0.0f32; NODES]; ARMS];
        let mote_lit = vec![0.0f32; motes.len()];
        let mote_off = vec![[0.0f32; 3]; motes.len()];

        let mut canvas = AsciiCanvas::default();
        canvas.ensure_layout(css_w, css_h, params.char_size, false);

        let mut st = Self {
            params,
            canvas,
            anim_time: 8.4,
            low_mode: false,
            icons: Vec::new(),
            icons_vis: false,
            css_w,
            css_h,
            dpi_scale,
            pointer_norm: [0.5, 0.5],
            pointer_vel: [0.0, 0.0],
            pointer_active: false,
            pointer_impulse: 0.0,
            track,
            rock_pts,
            corals,
            motes,
            dots,
            float_motes,
            arm_rest,
            arm_side,
            arm_fore,
            arm_x,
            arm_v,
            arm_link,
            arm_beat,
            arm_phase,
            arm_back,
            arm_glow,
            mote_lit,
            mote_off,
            user_morsel: UserMorsel {
                world: [0.0, 0.0, -0.6],
                age: 0.0,
                active: false,
                eaten_timer: 0.0,
                eaten_at: [0.0; 3],
            },
            fish_chase_blend: 0.0,
            fish_chase_offset: [0.0; 3],
            fish_boost: 0.0,
            last_st_idx: 0,
            frame_idx: 0,
        };
        // Warm up anemone physics & populate initial frame
        for _ in 0..45 {
            st.step_physics(STEP);
            st.anim_time += STEP;
        }
        st.rebuild_grid();
        st
    }

    pub fn resize(&mut self, css_w: f32, css_h: f32, dpi: f32) {
        self.css_w = css_w;
        self.css_h = css_h;
        self.dpi_scale = dpi;
        if self.canvas.ensure_layout(css_w, css_h, self.params.char_size, self.low_mode) {
            self.rebuild_grid();
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.canvas.ensure_layout(self.css_w, self.css_h, self.params.char_size, low);
            self.rebuild_grid();
        }
    }

    pub fn sync_params(&mut self, next: AsciiLabParams) {
        let changed = self.params != next;
        self.params = next;
        self.canvas
            .ensure_layout(self.css_w, self.css_h, self.params.char_size, self.low_mode);
        if changed {
            self.rebuild_grid();
        }
    }

    pub fn set_icons(&mut self, icons: &[Rect], vis: bool) {
        self.icons.clear();
        self.icons.extend_from_slice(icons);
        self.icons_vis = vis;
    }

    pub fn handle_mouse(&mut self, kind: i32, css_x: f32, css_y: f32) -> bool {
        if !self.params.mouse_interact {
            return false;
        }
        if kind == 3 {
            self.pointer_active = false;
            return false;
        }
        let nx = (css_x / self.css_w.max(1.0)).clamp(0.0, 1.0);
        let ny = (css_y / self.css_h.max(1.0)).clamp(0.0, 1.0);
        let dx = nx - self.pointer_norm[0];
        let dy = ny - self.pointer_norm[1];
        self.pointer_vel = [dx * 28.0, dy * 28.0];
        self.pointer_norm = [nx, ny];
        self.pointer_active = true;
        self.pointer_impulse = (self.pointer_impulse + (dx * dx + dy * dy).sqrt() * 8.0).min(1.5);

        if kind == 1 {
            // Left click: drop glowing plankton morsel in 3D space at cursor
            let st = self.sample_track(self.anim_time);
            let wx = (nx - 0.5) * 3.4 + st.cam_x;
            let wy = (0.5 - ny) * 2.0 + 0.12;
            let wz = -0.65;
            self.user_morsel.world = [wx, wy, wz];
            self.user_morsel.age = 0.0;
            self.user_morsel.active = true;
            self.fish_boost = 1.0;
            self.pointer_impulse = 1.4;
            return true;
        }
        false
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.anim_time = 8.4;
        self.user_morsel.active = false;
        self.fish_chase_blend = 0.0;
        self.fish_chase_offset = [0.0; 3];
        self.fish_boost = 0.0;
        self.rebuild_grid();
    }

    pub fn step(&mut self, dt: f32) {
        let dt_eff = dt.clamp(0.0, 0.05) * self.params.speed;
        self.anim_time = (self.anim_time + dt_eff) % LOOP_SECS;
        self.pointer_impulse *= (-dt * 4.5).exp();
        self.pointer_vel[0] *= (-dt * 6.0).exp();
        self.pointer_vel[1] *= (-dt * 6.0).exp();
        self.fish_boost *= (-dt * 1.4).exp();
        self.frame_idx = self.frame_idx.wrapping_add(1);

        let sub = if self.low_mode { 1 } else { 2 };
        let sub_dt = dt_eff / sub as f32;
        for _ in 0..sub {
            self.step_physics(sub_dt);
        }
        self.rebuild_grid();
    }

    fn sample_track(&self, t: f32) -> TrackSample {
        let n = self.track.len();
        let f = (t.rem_euclid(LOOP_SECS) / LOOP_SECS) * n as f32;
        let lo = (f.floor() as usize) % n;
        let hi = (lo + 1) % n;
        let w = f - f.floor();
        let a = &self.track[lo];
        let b = &self.track[hi];
        let lerp = |x: f32, y: f32| x + (y - x) * w;
        TrackSample {
            place: [
                lerp(a.place[0], b.place[0]),
                lerp(a.place[1], b.place[1]),
                lerp(a.place[2], b.place[2]),
            ],
            a: lerp(a.a, b.a),
            th: lerp(a.th, b.th),
            v: lerp(a.v, b.v),
            e: lerp(a.e, b.e),
            phi: lerp(a.phi, b.phi),
            hz: lerp(a.hz, b.hz),
            spin: lerp(a.spin, b.spin),
            slowing: lerp(a.slowing, b.slowing),
            cam_x: lerp(a.cam_x, b.cam_x),
            cam_y: lerp(a.cam_y, b.cam_y),
            cam_z: lerp(a.cam_z, b.cam_z),
            lead: lerp(a.lead, b.lead),
            zoom: lerp(a.zoom, b.zoom),
            prey: [
                lerp(a.prey[0], b.prey[0]),
                lerp(a.prey[1], b.prey[1]),
                lerp(a.prey[2], b.prey[2]),
            ],
            food: if a.food >= 0.0 && b.food >= 0.0 {
                lerp(a.food, b.food)
            } else {
                -1.0
            },
        }
    }

    fn effective_fish_pose(&self, t: f32) -> ([f32; 3], f32, f32, TrackSample) {
        let mut st = self.sample_track(t);
        let wag = 0.05 * (0.4 + st.e) * (st.phi + 0.8).sin();
        let mut yaw = PI - st.a + wag;
        let mut pitch = -st.th;
        let place = [
            st.place[0] + self.fish_chase_offset[0],
            st.place[1] + self.fish_chase_offset[1],
            st.place[2] + self.fish_chase_offset[2],
        ];
        if self.user_morsel.active && self.fish_chase_blend > 0.02 {
            let to = [
                self.user_morsel.world[0] - place[0],
                self.user_morsel.world[1] - place[1],
                self.user_morsel.world[2] - place[2],
            ];
            let target_yaw = PI - to[2].atan2(to[0]);
            let d = norm3(to).max(1e-4);
            let target_pitch = -(to[1] / d).clamp(-0.6, 0.6).asin();
            let dyaw = (target_yaw - yaw + PI).rem_euclid(TAU) - PI;
            yaw += dyaw * self.fish_chase_blend * 0.75;
            pitch += (target_pitch - pitch) * self.fish_chase_blend * 0.75;
            st.e = (st.e + self.fish_boost * 0.9).min(2.2);
            st.hz = 0.70 + 1.05 * st.e;
        }
        st.place = place;
        (place, yaw, pitch, st)
    }

    fn step_physics(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let t = self.anim_time;
        let w1 = 3.0 * TAU / LOOP_SECS;
        let w2 = 4.0 * TAU / LOOP_SECS;
        let w3 = 8.0 * TAU / LOOP_SECS;

        // Interactive user morsel physics + fish attraction
        if self.user_morsel.active {
            self.user_morsel.age += dt;
            self.user_morsel.world[1] -= 0.065 * dt;
            self.user_morsel.world[0] += 0.03 * (2.2 * self.user_morsel.age).sin() * dt;
            let base_st = self.sample_track(t);
            let target_off = [
                (self.user_morsel.world[0] - base_st.place[0]).clamp(-1.8, 1.8),
                (self.user_morsel.world[1] - base_st.place[1]).clamp(-1.1, 1.1),
                (self.user_morsel.world[2] - base_st.place[2]).clamp(-0.9, 0.9),
            ];
            self.fish_chase_blend = (self.fish_chase_blend + dt * 2.2).min(1.0);
            for k in 0..3 {
                self.fish_chase_offset[k] += (target_off[k] - self.fish_chase_offset[k]) * (dt * 2.6).min(1.0);
            }
            let fish_now = [
                base_st.place[0] + self.fish_chase_offset[0],
                base_st.place[1] + self.fish_chase_offset[1],
                base_st.place[2] + self.fish_chase_offset[2],
            ];
            let d = norm3([
                fish_now[0] - self.user_morsel.world[0],
                fish_now[1] - self.user_morsel.world[1],
                fish_now[2] - self.user_morsel.world[2],
            ]);
            if d < 0.36 || self.user_morsel.age > 8.5 || self.user_morsel.world[1] < FLOOR_Y + 0.1 {
                self.user_morsel.active = false;
                self.user_morsel.eaten_timer = 0.85;
                self.user_morsel.eaten_at = self.user_morsel.world;
            }
        } else {
            self.fish_chase_blend = (self.fish_chase_blend - dt * 1.4).max(0.0);
            let k_ret = (dt * 2.4).min(1.0);
            for c in &mut self.fish_chase_offset {
                *c *= 1.0 - k_ret;
            }
        }
        if self.user_morsel.eaten_timer > 0.0 {
            self.user_morsel.eaten_timer = (self.user_morsel.eaten_timer - dt).max(0.0);
        }

        let (place, yaw, roll, st) = self.effective_fish_pose(t);
        let prev_t = (t - dt).rem_euclid(LOOP_SECS);
        let prev_st = self.sample_track(prev_t);
        let earlier = [
            prev_st.place[0] + self.fish_chase_offset[0],
            prev_st.place[1] + self.fish_chase_offset[1],
            prev_st.place[2] + self.fish_chase_offset[2],
        ];
        let fish_vel = [
            (place[0] - earlier[0]) / dt.max(1e-4),
            (place[1] - earlier[1]) / dt.max(1e-4),
            (place[2] - earlier[2]) / dt.max(1e-4),
        ];
        let speed = norm3(fish_vel);

        // Pointer water-current world target
        let ptr_world = [
            (self.pointer_norm[0] - 0.5) * 3.4 + st.cam_x,
            (0.5 - self.pointer_norm[1]) * 2.1 - 0.25,
            0.25,
        ];

        let mut touched_any = [[false; NODES]; ARMS];
        let mut rush_vals = [[0.0f32; NODES]; ARMS];

        for i in 0..ARMS {
            let side = self.arm_side[i];
            let fore = self.arm_fore[i];
            let beat = self.arm_beat[i];
            let phase = self.arm_phase[i];
            for j in 1..NODES {
                let u = j as f32 / (NODES - 1) as f32;
                let p = self.arm_x[i][j];
                let push = 0.62 * (w1 * t - 1.3 * p[0]).sin() + 0.16 * (w3 * t + 2.0 * p[2]).sin();
                let mut flow = [
                    push,
                    0.10 * (w1 * t - 1.3 * p[0]).cos(),
                    0.08 * (w2 * t + 3.0 * p[0]).cos(),
                ];

                let dp = [p[0] - place[0], p[1] - place[1], p[2] - place[2]];
                let d2 = dot3(dp, dp);
                let near = (-d2 / 0.10).exp();
                flow[0] += 0.9 * near * fish_vel[0];
                flow[1] += 0.9 * near * fish_vel[1];
                flow[2] += 0.9 * near * fish_vel[2];

                // Interactive mouse swirl bends & illuminates arms!
                if self.pointer_active && self.params.mouse_interact {
                    let mp = [p[0] - ptr_world[0], p[1] - ptr_world[1], p[2] - ptr_world[2]];
                    let md2 = mp[0] * mp[0] + mp[1] * mp[1] + 0.35 * mp[2] * mp[2];
                    let mw = (-md2 / 0.32).exp();
                    let push_x = self.pointer_vel[0] * 0.22 + (mp[0] / (md2.sqrt() + 0.15)) * (0.45 + self.pointer_impulse);
                    let push_y = -self.pointer_vel[1] * 0.22 + (mp[1] / (md2.sqrt() + 0.15)) * 0.35;
                    flow[0] += mw * push_x * 1.45;
                    flow[1] += mw * push_y * 1.45;
                    if mw > 0.32 {
                        rush_vals[i][j] = rush_vals[i][j].max(mw * (0.55 + 0.45 * self.pointer_impulse));
                    }
                }

                let b0 = beat[0] as f32 * TAU / LOOP_SECS * t - 5.0 * u + phase[0];
                let b1 = beat[1] as f32 * TAU / LOOP_SECS * t - 5.0 * u + phase[1];
                let u_pow = u.powf(0.7);
                let own_x = b0.sin() * side[0] + 0.7 * b1.sin() * fore[0];
                let own_y = b0.sin() * side[1] + 0.7 * b1.sin() * fore[1];
                let own_z = b0.sin() * side[2] + 0.7 * b1.sin() * fore[2];
                flow[0] += 0.34 * u_pow * own_x;
                flow[1] += 0.34 * u_pow * own_y;
                flow[2] += 0.34 * u_pow * own_z;

                let drag = 1.2 + 4.2 * u.powf(1.2);
                for k in 0..3 {
                    self.arm_v[i][j][k] += drag * (flow[k] - self.arm_v[i][j][k]) * dt;
                    self.arm_x[i][j][k] += self.arm_v[i][j][k] * dt;
                }
                rush_vals[i][j] = rush_vals[i][j]
                    .max(((speed - 0.55) * 3.0).clamp(0.0, 1.0) * (-d2 / 0.16).exp() * 0.8);
            }

            // Springs + fish ellipsoid pushout + link length preservation
            for _ in 0..2 {
                for j in 1..NODES {
                    let u = j as f32 / (NODES - 1) as f32;
                    let stiff = 0.20 * (1.0 - u).powf(2.2) + 0.012;
                    let offset = [
                        self.arm_rest[i][j][0] - self.arm_rest[i][j - 1][0],
                        self.arm_rest[i][j][1] - self.arm_rest[i][j - 1][1],
                        self.arm_rest[i][j][2] - self.arm_rest[i][j - 1][2],
                    ];
                    for k in 0..3 {
                        let want = self.arm_x[i][j - 1][k] + offset[k];
                        self.arm_x[i][j][k] += stiff * (want - self.arm_x[i][j][k]);
                    }
                    // Fish shell pushout
                    let rel = [
                        self.arm_x[i][j][0] - place[0],
                        self.arm_x[i][j][1] - place[1],
                        self.arm_x[i][j][2] - place[2],
                    ];
                    let loc = rot_z(rot_y(rel, -yaw), -roll);
                    let e = [
                        (loc[0] - 0.08) / 0.643,
                        loc[1] / 0.343,
                        loc[2] / 0.213,
                    ];
                    let d = norm3(e);
                    if d < 1.0 {
                        touched_any[i][j] = true;
                        let inv_d = 1.0 / d.max(1e-6);
                        let out = [
                            e[0] * inv_d * 0.643 + 0.08,
                            e[1] * inv_d * 0.343,
                            e[2] * inv_d * 0.213,
                        ];
                        let world_out = rot_y(rot_z(out, roll), yaw);
                        for k in 0..3 {
                            let tgt = world_out[k] + place[k];
                            self.arm_x[i][j][k] += 0.5 * (tgt - self.arm_x[i][j][k]);
                        }
                    }
                }
                self.arm_x[i][0] = self.arm_rest[i][0];
                for j in 1..NODES {
                    let prev = self.arm_x[i][j - 1];
                    let cur = self.arm_x[i][j];
                    let dir = unit3([cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]]);
                    let ln = self.arm_link[i][j - 1];
                    self.arm_x[i][j] = [
                        prev[0] + dir[0] * ln,
                        prev[1] + dir[1] * ln,
                        prev[2] + dir[2] * ln,
                    ];
                }
            }

            // Bioluminescent touch wave along arm
            let decay = (-dt / 0.7).exp();
            for j in 0..NODES {
                self.arm_glow[i][j] *= decay;
            }
            for j in 1..NODES {
                let dglow = 13.0 * dt * (self.arm_glow[i][j - 1] - self.arm_glow[i][j]);
                self.arm_glow[i][j] = (self.arm_glow[i][j] + dglow)
                    .max(if touched_any[i][j] { 1.0 } else { 0.0 })
                    .max(rush_vals[i][j])
                    .min(1.0);
            }
        }

        // Plankton motes flash & wake drift
        let m_fade = (-dt / 1.6).exp();
        let m_ret = (-dt / 2.2).exp();
        for (i, mp) in self.motes.iter().enumerate() {
            let pos = [
                mp[0] + self.mote_off[i][0],
                mp[1] + self.mote_off[i][1],
                mp[2] + self.mote_off[i][2],
            ];
            let gap = (pos[0] - place[0]).powi(2)
                + (pos[1] - place[1]).powi(2)
                + (pos[2] - place[2]).powi(2);
            let mut flash = ((speed - 0.42) * 2.4).clamp(0.0, 1.0) * (-gap / 0.22).exp()
                + 0.35 * (-gap / 0.03).exp();
            if self.pointer_active && self.params.mouse_interact {
                let pg = (pos[0] - ptr_world[0]).powi(2) + (pos[1] - ptr_world[1]).powi(2);
                flash += 0.6 * (-pg / 0.18).exp() * (0.3 + self.pointer_impulse);
            }
            self.mote_lit[i] = (self.mote_lit[i] * m_fade).max(flash.clamp(0.0, 1.0));
            let drag_f = 0.55 * (-gap / 0.10).exp();
            for k in 0..3 {
                self.mote_off[i][k] =
                    self.mote_off[i][k] * m_ret + drag_f * (place[k] - earlier[k]);
            }
        }
        self.last_st_idx = ((t / STEP).round() as usize) % self.track.len();
    }

    fn palette_colors(&self) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3]) {
        match self.params.palette {
            // 1: Sapphire Reef (electric blue clownfish + golden anemone)
            1 => (
                [42.0 / 255.0, 152.0 / 255.0, 255.0 / 255.0],  // primary flank
                [24.0 / 255.0, 88.0 / 255.0, 224.0 / 255.0],   // deep back
                [112.0 / 255.0, 204.0 / 255.0, 255.0 / 255.0], // face
                [244.0 / 255.0, 250.0 / 255.0, 255.0 / 255.0], // white stripe
                [28.0 / 255.0, 44.0 / 255.0, 102.0 / 255.0],   // dark margin
                [185.0 / 255.0, 238.0 / 255.0, 255.0 / 255.0], // rim
                [255.0 / 255.0, 212.0 / 255.0, 130.0 / 255.0], // anemone tip
                [108.0 / 255.0, 84.0 / 255.0, 146.0 / 255.0],  // anemone stem
            ),
            // 2: Bio Neon (coral magenta & cyan glow)
            2 => (
                [255.0 / 255.0, 86.0 / 255.0, 148.0 / 255.0],
                [205.0 / 255.0, 36.0 / 255.0, 118.0 / 255.0],
                [255.0 / 255.0, 142.0 / 255.0, 188.0 / 255.0],
                [245.0 / 255.0, 255.0 / 255.0, 252.0 / 255.0],
                [66.0 / 255.0, 32.0 / 255.0, 98.0 / 255.0],
                [150.0 / 255.0, 255.0 / 255.0, 240.0 / 255.0],
                [132.0 / 255.0, 252.0 / 255.0, 226.0 / 255.0],
                [88.0 / 255.0, 72.0 / 255.0, 168.0 / 255.0],
            ),
            // 0: Original `clownfish.py` exact palette (1:1 reference)
            _ => (
                [255.0 / 255.0, 122.0 / 255.0, 22.0 / 255.0],  // ORANGE
                [238.0 / 255.0, 66.0 / 255.0, 18.0 / 255.0],   // DEEP
                [255.0 / 255.0, 168.0 / 255.0, 52.0 / 255.0],  // FACE
                [250.0 / 255.0, 248.0 / 255.0, 240.0 / 255.0], // WHITE
                [84.0 / 255.0, 62.0 / 255.0, 112.0 / 255.0],   // DARK
                [205.0 / 255.0, 222.0 / 255.0, 255.0 / 255.0], // RIM
                [186.0 / 255.0, 228.0 / 255.0, 236.0 / 255.0], // TIP
                [112.0 / 255.0, 94.0 / 255.0, 160.0 / 255.0],  // STEM
            ),
        }
    }

    pub fn rebuild_grid(&mut self) {
        let cols = self.canvas.cols;
        let rows = self.canvas.rows;
        if cols == 0 || rows == 0 {
            return;
        }
        self.canvas.clear([2, 4, 8], 99.0);

        let cw = 12.0f32;
        let ch = 18.0f32;
        let virt_w = cols as f32 * cw;
        let virt_h = rows as f32 * ch;
        let mid_y = virt_h * 0.50;

        let t = self.anim_time;
        let (place, yaw, pitch, st) = self.effective_fish_pose(t);
        let effort = (0.5 + 1.25 * st.e).min(2.0);
        let turn = (0.11 * st.spin).clamp(-0.2, 0.2);
        let brake = (1.6 * st.slowing).clamp(0.0, 0.7);
        let mut cam_x = st.cam_x;
        let mut cam_y = 0.62 * st.cam_y - 0.10;
        if self.pointer_active && self.params.mouse_interact {
            cam_x += (self.pointer_norm[0] - 0.5) * 0.16;
            cam_y -= (self.pointer_norm[1] - 0.5) * 0.10;
        }
        let lead = 0.11 * st.lead;
        // Responsive scale matching widescreen & portrait screens cleanly without clipping
        let base_scale = (virt_w / 1080.0).min(virt_h / 1220.0).max(0.45) * 760.0;
        let mut scale = base_scale * st.zoom;
        let swing = ORBIT * (cam_x - PIVOT[0]);
        let tilt = -TILT * (cam_y - 0.15);
        let aim = [cam_x + lead, cam_y, st.cam_z];

        let view = |v: [f32; 3]| -> [f32; 3] {
            let shifted = [v[0] - PIVOT[0], v[1] - PIVOT[1], v[2] - PIVOT[2]];
            let r = rot_x(rot_y(shifted, swing), tilt);
            [r[0] + PIVOT[0], r[1] + PIVOT[1], r[2] + PIVOT[2]]
        };
        let centre = view(aim);
        let near = view([cam_x, cam_y, st.cam_z])[2];
        scale *= ((CAM_D + near) / CAM_D).clamp(0.25, 2.0).powf(0.85);

        let project = |v: [f32; 3]| -> (f32, f32, f32, f32) {
            let vp = view(v);
            let denom = (CAM_D + vp[2]).max(0.35);
            let shrink = scale * CAM_D / denom;
            let sx = virt_w * 0.5 + shrink * (vp[0] - centre[0]);
            let sy = mid_y - shrink * (vp[1] - centre[1]);
            (sx, sy, shrink, vp[2])
        };

        // 1. Background parallax water specks & floating motes
        let slide_c = cam_x * scale / cw;
        let slide_r = cam_y * scale / ch;
        for d in &self.dots {
            let rr = (d[1] * rows as f32 + 0.18 * t * (0.5 + d[2]) + slide_r * 0.3)
                .rem_euclid(rows as f32) as i32;
            let cc = (d[0] * cols as f32 + 0.25 * (0.4 * t + d[2] * 9.0).sin() - slide_c * 0.3)
                .rem_euclid(cols as f32) as i32;
            let f = 0.5 + 0.5 * d[2];
            let rgb = scale_rgb([64.0 / 255.0, 84.0 / 255.0, 112.0 / 255.0], f);
            self.canvas.set_cell(
                cc,
                rr,
                glyph_id('.'),
                [super::clamp_u8(rgb[0]), super::clamp_u8(rgb[1]), super::clamp_u8(rgb[2])],
                0,
                OWN_WATER,
            );
        }
        for m in &self.float_motes {
            let rr = (m[1] * rows as f32 - 0.3 * t * (0.4 + m[2]) + slide_r * 0.55)
                .rem_euclid(rows as f32) as i32;
            let cc = (m[0] * cols as f32 + 0.8 * (0.5 * t + m[3] * 20.0).sin() - slide_c * 0.55)
                .rem_euclid(cols as f32) as i32;
            let tw = (t * (1.0 + 2.0 * m[2]) + m[3] * 40.0).sin();
            let f = 0.5 + 0.5 * (tw + 0.4).clamp(0.0, 1.0);
            let rgb = scale_rgb([70.0 / 255.0, 118.0 / 255.0, 160.0 / 255.0], f);
            self.canvas.set_cell(
                cc,
                rr,
                glyph_id(if tw > 0.7 { '*' } else { '+' }),
                [super::clamp_u8(rgb[0]), super::clamp_u8(rgb[1]), super::clamp_u8(rgb[2])],
                25,
                OWN_WATER,
            );
        }

        // 2. Caustic rippled sand floor (expanded dynamically across widescreen view)
        let w2 = 4.0 * TAU / LOOP_SECS;
        let w3 = 8.0 * TAU / LOOP_SECS;
        let cam_ix = (cam_x / 0.13) as i32;
        let half_sand_x = ((cols as f32 / 90.0) * 34.0).ceil() as i32 + 4;
        for ix in (-half_sand_x + cam_ix)..=(half_sand_x + cam_ix) {
            for iz in -12..42 {
                let h = hash01(ix as f32, iz as f32);
                let sx_w = ix as f32 * 0.13 + 0.09 * hash01(iz as f32, ix as f32);
                let sz_w = iz as f32 * 0.13 + 0.09 * h;
                let ripple = (sx_w * 4.2 + 1.6 * (sz_w * 2.1).sin()).sin() > 0.72;
                let keep = h > 0.52 || (ripple && hash01(ix as f32 + 7.0, iz as f32) > 0.35);
                if !keep {
                    continue;
                }
                let pt = [sx_w, FLOOR_Y + 0.03 * (sx_w * 1.7 + sz_w).sin(), sz_w];
                let (sx, sy, _, _) = project(pt);
                let c = (sx / cw).floor() as i32;
                let r = (sy / ch).floor() as i32;
                if c >= 0 && c < cols as i32 && r >= 0 && r < rows as i32 {
                    let mut fade = (1.0 - (sz_w + 1.5) / 6.5).clamp(0.15, 1.0);
                    let net = (sx_w * 5.0 + 1.3 * (sz_w * 3.0 + w3 * t).sin()).sin()
                        * (sz_w * 4.0 - w2 * t + sx_w).sin();
                    fade *= 1.0 + 1.1 * net.clamp(0.0, 1.0).powi(2);
                    let amp = if ripple { 0.82 } else { 0.56 } * fade;
                    let rgb = scale_rgb([168.0 / 255.0, 140.0 / 255.0, 98.0 / 255.0], amp);
                    self.canvas.set_cell(
                        c,
                        r,
                        glyph_id(if ripple { '~' } else { '.' }),
                        [super::clamp_u8(rgb[0]), super::clamp_u8(rgb[1]), super::clamp_u8(rgb[2])],
                        0,
                        OWN_WATER,
                    );
                }
            }
        }

        // 3. Branching background corals
        let w1 = 3.0 * TAU / LOOP_SECS;
        for coral in &self.corals {
            let (bx, by, shrink, _) = project(coral.world);
            if bx < -300.0 || bx > virt_w + 300.0 {
                continue;
            }
            let c_size = coral.size * shrink;
            let far_fade = (0.75 - 0.12 * coral.world[2]).clamp(0.3, 0.7);
            let lean_amp = 0.06 * (w1 * t + 0.7 * coral.seed as f32).sin();
            for tw in &coral.twigs {
                let x0 = bx + c_size * (tw[0] + lean_amp * tw[1].max(0.0).powf(1.5));
                let y0 = by - c_size * tw[1];
                let x1 = bx + c_size * (tw[2] + lean_amp * tw[3].max(0.0).powf(1.5));
                let y1 = by - c_size * tw[3];
                let lv = 0.55 + 0.1 * (5.0 - tw[4]);
                let rgb_f = scale_rgb(coral.col, far_fade * lv);
                let rgb = [super::clamp_u8(rgb_f[0]), super::clamp_u8(rgb_f[1]), super::clamp_u8(rgb_f[2])];
                let gl = line_glyph_cell(x1 - x0, y1 - y0, cw, ch);
                let steps = ((x1 - x0).abs() / cw)
                    .max((y1 - y0).abs() / ch)
                    .mul_add(1.6, 2.0) as usize;
                for s in 0..steps {
                    let f = s as f32 / (steps - 1).max(1) as f32;
                    let cc = ((x0 + (x1 - x0) * f) / cw).floor() as i32;
                    let rr = ((y0 + (y1 - y0) * f) / ch).floor() as i32;
                    self.canvas.set_cell(cc, rr, gl, rgb, 20, OWN_BACK);
                }
            }
            for &(bx0, by0, is_o) in &coral.buds {
                let px = bx + c_size * (bx0 + lean_amp * by0.max(0.0).powf(1.5));
                let py = by - c_size * by0;
                let rgb_f = scale_rgb(coral.col, far_fade * 1.45);
                let rgb = [super::clamp_u8(rgb_f[0]), super::clamp_u8(rgb_f[1]), super::clamp_u8(rgb_f[2])];
                self.canvas.set_cell(
                    (px / cw).floor() as i32,
                    (py / ch).floor() as i32,
                    glyph_id(if is_o { 'o' } else { '*' }),
                    rgb,
                    45,
                    OWN_BACK,
                );
            }
        }

        // 4. Static reef boulders + anemone fleshy foot
        for rp in &self.rock_pts {
            let (sx, sy, _, depth) = project(rp.p);
            let c = (sx / cw).floor() as i32;
            let r = (sy / ch).floor() as i32;
            let gl = ramp_glyph(ROCK_G, rp.lum);
            let rgb = [clamp_u8(rp.col[0]), clamp_u8(rp.col[1]), clamp_u8(rp.col[2])];
            self.canvas.plot_z_less(c, r, depth, gl, rgb, 0, OWN_BACK);
        }

        // 5. Clownfish 3D Body, Stripes, Fins, Eyes (with spine arc-length correction)
        let (orange, deep, face, white, dark, rim, tip_col, stem_col) = self.palette_colors();
        let light = unit3([-0.45, 0.75, -0.48]);
        let half_light = unit3([light[0], light[1], light[2] - 1.0]);

        // Spine reach table for constant body length
        const ALONG_N: usize = 48;
        let mut reach = [0.0f32; ALONG_N];
        let ds_along = 1.5 / (ALONG_N - 1) as f32;
        for i in 1..ALONG_N {
            let s0 = (i - 1) as f32 * ds_along;
            let s1 = i as f32 * ds_along;
            let y0 = fish_sway(s0, st.phi, 0.0, st.hz, effort, turn);
            let y1 = fish_sway(s1, st.phi, 0.0, st.hz, effort, turn);
            let sl = (y1 - y0) / ds_along;
            reach[i] = reach[i - 1] + ds_along / (1.0 + sl * sl).sqrt();
        }
        let map_reach = |s_in: f32| -> f32 {
            let f = (s_in / ds_along).clamp(0.0, (ALONG_N - 1) as f32 - 1e-4);
            let i0 = f as usize;
            let w = f - i0 as f32;
            reach[i0] + w * (reach[i0 + 1] - reach[i0])
        };
        let body_end = map_reach(0.98) - 0.45;
        let posed_pos = |p: [f32; 3]| -> [f32; 3] {
            let mut mp = p;
            mp[0] = if mp[0] > TAIL_ROOT {
                body_end + mp[0] - TAIL_ROOT
            } else {
                map_reach(mp[0] + 0.45) - 0.45
            };
            let r = rot_y(rot_z(mp, pitch), yaw);
            [r[0] + place[0], r[1] + place[1], r[2] + place[2]]
        };
        let posed_vec = |n: [f32; 3]| -> [f32; 3] { rot_y(rot_z(n, pitch), yaw) };
        let facing_front = (-posed_vec([-1.0, 0.0, 0.0])[2] - 0.35) / 0.45;
        let facing_f = ease(facing_front);

        // 5a. Body surface points
        let n_s = if self.low_mode { 92 } else { 145 };
        let n_th = if self.low_mode { 44 } else { 68 };
        for is in 0..n_s {
            let s = is as f32 / (n_s - 1) as f32;
            let h = half_height(s);
            let w = half_width(s);
            let sw = fish_sway(s, st.phi, 0.0, st.hz, effort, turn);
            let sl = profile_slope(s);
            for ith in 0..n_th {
                let th = (ith as f32 / n_th as f32) * TAU;
                let v = th.sin();
                let cv = th.cos();
                let local_p = [s - 0.45, h * v, w * cv + sw];
                let wp = posed_pos(local_p);
                let (sx, sy, _, depth) = project(wp);
                let c = (sx / cw).floor() as i32;
                let r = (sy / ch).floor() as i32;
                if c < 0 || c >= cols as i32 || r < 0 || r >= rows as i32 {
                    continue;
                }
                let n = unit3(posed_vec([-sl * w, w * v, h * cv]));
                let lit = dot3(n, light).clamp(0.0, 1.0);
                let shine = dot3(n, half_light).clamp(0.0, 1.0).powi(14) * (0.25 + 0.75 * facing_f);

                // Stripes
                let bulge = (-((v / 0.55).powi(2))).exp();
                let bands = [
                    (0.215 + 0.035 * v * v, 0.335 + 0.02 * v * v),
                    (0.515 - 0.055 * bulge, 0.655 - 0.02 * bulge),
                    (0.905, 0.985),
                ];
                let mut kind = 0u8;
                for (a, b) in bands {
                    if s > a - 0.016 && s < b + 0.016 {
                        kind = 2;
                    }
                    if s > a && s < b {
                        kind = 1;
                    }
                }

                let ev = ease(v * 0.9);
                let mut base = [
                    orange[0] * (1.0 - 0.5 * ev) + deep[0] * 0.5 * ev,
                    orange[1] * (1.0 - 0.5 * ev) + deep[1] * 0.5 * ev,
                    orange[2] * (1.0 - 0.5 * ev) + deep[2] * 0.5 * ev,
                ];
                if s < 0.2 {
                    for k in 0..3 {
                        base[k] = base[k] * 0.45 + face[k] * 0.55;
                    }
                }
                let raw_col = match kind {
                    1 => white,
                    2 => dark,
                    _ => base,
                };
                let shade = if kind == 2 {
                    1.0
                } else if kind == 1 {
                    0.82 + 0.18 * lit
                } else {
                    0.42 + 0.58 * lit
                };
                let mut col = [
                    raw_col[0] * shade,
                    raw_col[1] * shade,
                    raw_col[2] * shade,
                ];
                if kind != 2 {
                    for k in 0..3 {
                        col[k] = (col[k] + 0.75 * shine).clamp(0.0, 1.0);
                    }
                }
                let mut lum = match kind {
                    2 => 0.42,
                    1 => 0.86 + 0.14 * lit,
                    _ => 0.66 + 0.3 * lit,
                };
                lum = (lum + 0.3 * shine).clamp(0.0, 1.0);
                let gl = ramp_glyph(RAMP, (1.0 - lum).powf(1.3));
                let rgb = [clamp_u8(col[0]), clamp_u8(col[1]), clamp_u8(col[2])];
                let bloom = if kind == 1 {
                    205
                } else if lum > 0.70 {
                    165
                } else {
                    95
                };
                self.canvas.plot_z_less(c, r, depth, gl, rgb, bloom, OWN_FISH);
            }
        }

        // 5b. Fins helper
        let plot_fin = |canvas: &mut AsciiCanvas, p_loc: [f32; 3], q_loc: [f32; 3], b: f32, margin: f32, rim_t: f32| {
            let wp = posed_pos(p_loc);
            let wq = posed_pos(q_loc);
            let (sx, sy, _, depth) = project(wp);
            let (qx, qy, _, _) = project(wq);
            let c = (sx / cw).floor() as i32;
            let r = (sy / ch).floor() as i32;
            let gl = line_glyph_cell(qx - sx, qy - sy, cw, ch);
            let col = if b > rim_t {
                rim
            } else if b > margin {
                dark
            } else {
                [
                    orange[0] * 0.94 + face[0] * 0.06,
                    orange[1] * 0.94 + face[1] * 0.06,
                    orange[2] * 0.94 + face[2] * 0.06,
                ]
            };
            let rgb = [
                clamp_u8(col[0] * 0.92),
                clamp_u8(col[1] * 0.92),
                clamp_u8(col[2] * 0.92),
            ];
            let bloom = if b > rim_t { 140 } else { 70 };
            canvas.plot_z_less(c, r, depth, gl, rgb, bloom, OWN_FIN);
        };

        // Dorsal fins (front spiny & rear soft)
        for &(s0, s1, tall, lean) in &[(0.25f32, 0.57f32, 0.125f32, 0.45f32), (0.53, 0.88, 0.105, 0.85)] {
            let na = if self.low_mode { 42 } else { 64 };
            let nb = if self.low_mode { 14 } else { 20 };
            for ia in 0..na {
                let a = ia as f32 / (na - 1) as f32;
                let s = s0 + (s1 - s0) * a;
                let reach_f = tall * (PI * a.powf(0.8)).sin().powf(0.8);
                let out = [lean * reach_f, reach_f, 0.0];
                let uout = unit3(out);
                let root = [s - 0.45, half_height(s) * 0.93, 0.0];
                for ib in 0..nb {
                    let b = ib as f32 / (nb - 1) as f32;
                    let mut p = [root[0] + out[0] * b, root[1] + out[1] * b, 0.0];
                    p[2] = fish_sway(p[0] + 0.45, st.phi, 0.16 * b, st.hz, effort, turn)
                        + 0.05 * b.powf(1.5) * (TAU * 1.1 * t - 8.0 * a - if s0 > 0.4 { 3.0 } else { 0.0 }).sin();
                    let q = [p[0] + uout[0] * 0.03, p[1] + uout[1] * 0.03, p[2] + uout[2] * 0.03];
                    plot_fin(&mut self.canvas, p, q, b, 0.84, 0.96);
                }
            }
        }

        // Anal fin
        {
            let na = if self.low_mode { 36 } else { 52 };
            let nb = if self.low_mode { 12 } else { 18 };
            for ia in 0..na {
                let a = ia as f32 / (na - 1) as f32;
                let s = 0.62 + 0.25 * a;
                let reach_f = 0.135 * (PI * (a * 0.9 + 0.1).clamp(0.0, 1.0)).sin().powf(0.6);
                let out = [0.7 * reach_f, -reach_f, 0.0];
                let uout = unit3(out);
                let root = [s - 0.45, -half_height(s) * 0.93, 0.0];
                for ib in 0..nb {
                    let b = ib as f32 / (nb - 1) as f32;
                    let mut p = [root[0] + out[0] * b, root[1] + out[1] * b, 0.0];
                    p[2] = fish_sway(p[0] + 0.45, st.phi, 0.18 * b, st.hz, effort, turn)
                        + 0.045 * b.powf(1.5) * (TAU * 1.1 * t - 7.0 * a + 1.0).sin();
                    let q = [p[0] + uout[0] * 0.03, p[1] + uout[1] * 0.03, p[2] + uout[2] * 0.03];
                    plot_fin(&mut self.canvas, p, q, b, 0.72, 0.93);
                }
            }
        }

        // Pelvic fins
        for side_v in [-1.0f32, 1.0f32] {
            let na = 28;
            let nb = 14;
            for ia in 0..na {
                let a = ia as f32 / (na - 1) as f32;
                let s = 0.37 + 0.09 * a;
                let reach_f = 0.17 * (0.55 + 0.45 * (PI * a).sin().powf(0.7));
                let out = [0.55 * reach_f, -reach_f, side_v * 0.25 * reach_f];
                let uout = unit3(out);
                let root = [s - 0.45, -half_height(s) * 0.9, side_v * 0.04];
                for ib in 0..nb {
                    let b = ib as f32 / (nb - 1) as f32;
                    let mut p = [root[0] + out[0] * b, root[1] + out[1] * b, root[2] + out[2] * b];
                    p[2] += fish_sway(s, st.phi, 0.2 * b, st.hz, effort, turn)
                        + 0.04 * b * b * (TAU * 0.8 * t + side_v + 2.0 * a).sin();
                    p[0] += 0.05 * b * b * (TAU * 0.8 * t + 1.5 + side_v).sin();
                    let q = [p[0] + uout[0] * 0.03, p[1] + uout[1] * 0.03, p[2] + uout[2] * 0.03];
                    plot_fin(&mut self.canvas, p, q, b, 0.78, 2.0);
                }
            }
        }

        // Caudal tail fan (fixed blade length along traveling body wave)
        {
            const NB: usize = 28;
            let na = if self.low_mode { 44 } else { 66 };
            let mut far = [0.0f32; NB];
            let mut side_off = [0.0f32; NB];
            let d_out = 1.0 / (NB - 1) as f32;
            let tail_base_z = fish_sway(0.98, st.phi, 0.0, st.hz, effort, turn);
            for j in 1..NB {
                let out_f = j as f32 * d_out;
                let here = 0.98 + 0.31 * out_f;
                let s_a = fish_sway(here - 0.005, st.phi, 0.12 * out_f, st.hz, effort, turn);
                let s_b = fish_sway(here + 0.005, st.phi, 0.12 * out_f, st.hz, effort, turn);
                let sl = ((s_b - s_a) / 0.01) * (1.0 + 0.3 * out_f);
                let tr = sl.atan().clamp(-0.95, 0.95);
                far[j] = far[j - 1] + tr.cos() * d_out;
                side_off[j] = side_off[j - 1] + tr.sin() * d_out;
            }
            for ia in 0..na {
                let a = -1.0 + 2.0 * (ia as f32 / (na - 1) as f32);
                let phi_a = 0.95 * a;
                let span = 0.31 * (1.0 - 0.13 * a * a);
                let ray = [phi_a.cos(), phi_a.sin(), 0.0];
                for ib in 0..NB {
                    let b = ib as f32 * d_out;
                    let r = b * span;
                    let p = [
                        TAIL_ROOT + span * phi_a.cos() * far[ib],
                        0.075 * a * (1.0 - b) + r * phi_a.sin(),
                        tail_base_z
                            + span * side_off[ib]
                            + 0.03 * b * b * (TAU * 0.95 * t - 4.2 + 2.5 * a.abs()).sin(),
                    ];
                    let q = [p[0] + ray[0] * 0.03, p[1] + ray[1] * 0.03, p[2]];
                    plot_fin(&mut self.canvas, p, q, b, 0.76, 0.94);
                }
            }
        }

        // Pectoral oar fins
        for side_v in [-1.0f32, 1.0f32] {
            let na = if self.low_mode { 28 } else { 42 };
            let nb = if self.low_mode { 16 } else { 22 };
            let hinge = [0.375 - 0.45, -0.07, side_v * half_width(0.375) * 0.9];
            let back = [0.72f32, -0.69, 0.0];
            let outw = [0.0f32, 0.0, side_v];
            let up = [0.69f32, 0.72, 0.0];
            for ia in 0..na {
                let a = -1.0 + 2.0 * (ia as f32 / (na - 1) as f32);
                let phi_a = 0.8 * a;
                for ib in 0..nb {
                    let b = ib as f32 / (nb - 1) as f32;
                    let beat = TAU * PECT_HZ * t + if side_v > 0.0 { 0.0 } else { 2.2 }
                        - 2.1 * b
                        - 0.5 * a.abs();
                    let stroke = beat.sin() + 0.25 * (2.0 * beat + 0.6).sin();
                    let tucked = (effort - 0.55).min(1.0);
                    let open_a = 0.72 - 0.20 * tucked + 0.55 * brake + (0.48 - 0.14 * tucked) * stroke;
                    let axis = [
                        back[0] * open_a.cos() + outw[0] * open_a.sin(),
                        back[1] * open_a.cos() + outw[1] * open_a.sin(),
                        back[2] * open_a.cos() + outw[2] * open_a.sin(),
                    ];
                    let ray = [
                        axis[0] * phi_a.cos() + up[0] * phi_a.sin(),
                        axis[1] * phi_a.cos() + up[1] * phi_a.sin(),
                        axis[2] * phi_a.cos() + up[2] * phi_a.sin(),
                    ];
                    let rad = b * 0.21 * (1.0 - 0.12 * a * a);
                    let cup = 0.07 * b * b * beat.cos();
                    let p = [
                        hinge[0] + ray[0] * rad - outw[0] * cup,
                        hinge[1] + ray[1] * rad - outw[1] * cup,
                        hinge[2] + ray[2] * rad - outw[2] * cup,
                    ];
                    let q = [p[0] + ray[0] * 0.03, p[1] + ray[1] * 0.03, p[2] + ray[2] * 0.03];
                    plot_fin(&mut self.canvas, p, q, b, 0.74, 0.93);
                }
            }
        }

        // 5c. Eyes (deep pupil + amber sclera + bright specular sparkle)
        for side_v in [-1.0f32, 1.0f32] {
            for iu in 0..8 {
                let u = iu as f32 / 7.0;
                let r = 0.041 * u;
                for ik in 0..22 {
                    let k = (ik as f32 / 22.0) * TAU;
                    let s = 0.115 + r * k.cos();
                    let y = 0.05 + r * k.sin();
                    let z = side_v
                        * (half_width(s) * (1.0 - (y / half_height(s).max(1e-4)).powi(2)).clamp(0.05, 1.0).sqrt()
                            + 0.03);
                    let wp = posed_pos([s - 0.45, y, z + fish_sway(s, st.phi, 0.0, st.hz, effort, turn)]);
                    let (sx, sy, _, depth) = project([wp[0], wp[1], wp[2] - 0.004]);
                    let c = (sx / cw).floor() as i32;
                    let r_c = (sy / ch).floor() as i32;
                    let (col, lum) = if u > 0.15 && u < 0.42 && (k - 2.2).abs() < 0.45 {
                        ([235.0 / 255.0, 225.0 / 255.0, 215.0 / 255.0], 0.92)
                    } else if u < 0.6 {
                        ([58.0 / 255.0, 34.0 / 255.0, 30.0 / 255.0], 0.62)
                    } else {
                        ([255.0 / 255.0, 190.0 / 255.0, 96.0 / 255.0], 0.72)
                    };
                    let gl = ramp_glyph(RAMP, 1.0 - lum);
                    self.canvas.plot_z_less(
                        c,
                        r_c,
                        depth,
                        gl,
                        [clamp_u8(col[0]), clamp_u8(col[1]), clamp_u8(col[2])],
                        110,
                        OWN_FISH,
                    );
                }
            }
        }

        // 5d. Outline pass around clownfish flank
        {
            let prev_own = self.canvas.owner.clone();
            for r in 1..(rows - 1) {
                for c in 1..(cols - 1) {
                    let i = r * cols + c;
                    if prev_own[i] != OWN_FISH {
                        continue;
                    }
                    let u_f = prev_own[(r - 1) * cols + c] == OWN_FISH;
                    let d_f = prev_own[(r + 1) * cols + c] == OWN_FISH;
                    let l_f = prev_own[r * cols + c - 1] == OWN_FISH;
                    let r_f = prev_own[r * cols + c + 1] == OWN_FISH;
                    let touch_fin = prev_own[(r - 1) * cols + c] == OWN_FIN
                        || prev_own[(r + 1) * cols + c] == OWN_FIN
                        || prev_own[r * cols + c - 1] == OWN_FIN
                        || prev_own[r * cols + c + 1] == OWN_FIN;
                    if touch_fin || hash01(c as f32 + (t * 3.0).floor(), r as f32) <= 0.12 {
                        continue;
                    }
                    let edge_g = if !l_f && !u_f {
                        Some(glyph_id('/'))
                    } else if !l_f && !d_f {
                        Some(glyph_id('\\'))
                    } else if !r_f && !u_f {
                        Some(glyph_id('\\'))
                    } else if !r_f && !d_f {
                        Some(glyph_id('/'))
                    } else if !u_f {
                        Some(glyph_id('-'))
                    } else if !d_f {
                        Some(glyph_id('_'))
                    } else if !l_f || !r_f {
                        Some(glyph_id('('))
                    } else {
                        None
                    };
                    if let Some(g) = edge_g {
                        let p = i * 4;
                        self.canvas.cells[p + 3] = g;
                        for k in 0..3 {
                            let v = self.canvas.cells[p + k] as f32 * 0.78 + 25.0;
                            self.canvas.cells[p + k] = v.clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }

        // 6. Anemone 50 swaying tubular arms + bioluminescent tips & desktop icon awareness
        let icon_rects_cells: Vec<[f32; 4]> = if self.icons_vis {
            self.icons
                .iter()
                .map(|rc| {
                    let x0 = (rc.x / self.css_w.max(1.0)) * cols as f32;
                    let y0 = (rc.y / self.css_h.max(1.0)) * rows as f32;
                    let x1 = ((rc.x + rc.w) / self.css_w.max(1.0)) * cols as f32;
                    let y1 = ((rc.y + rc.h) / self.css_h.max(1.0)) * rows as f32;
                    [x0, y0, x1, y1]
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut arm_order: Vec<usize> = (0..ARMS).collect();
        arm_order.sort_by(|&a, &b| {
            let za = self.arm_x[a].iter().map(|p| p[2]).sum::<f32>();
            let zb = self.arm_x[b].iter().map(|p| p[2]).sum::<f32>();
            zb.partial_cmp(&za).unwrap_or(std::cmp::Ordering::Equal)
        });
        let fine_n = if self.low_mode { 28 } else { 44 };
        for arm_i in arm_order {
            let depth_dim = (1.1 - 0.7 * self.arm_back[arm_i]).clamp(0.30, 1.0);
            for s_idx in 0..fine_n {
                let u = s_idx as f32 / (fine_n - 1) as f32;
                let f = u * (NODES - 1) as f32;
                let j0 = (f.floor() as usize).min(NODES - 2);
                let frac = f - j0 as f32;
                let mut wp = [0.0f32; 3];
                for k in 0..3 {
                    wp[k] = self.arm_x[arm_i][j0][k] * (1.0 - frac) + self.arm_x[arm_i][j0 + 1][k] * frac;
                }
                let cap = (1.0 - ((u - 0.90) / 0.10).powi(2)).clamp(0.0, 1.0).sqrt();
                let thick = if u > 0.90 {
                    0.049 * cap + 0.004
                } else {
                    0.051 - 0.014 * u + 0.012 * (-((u - 0.86) / 0.08).powi(2)).exp()
                };
                let (mut sx, sy, shrink, zdepth) = project(wp);
                // Gently bend arm points away from desktop icons
                if !icon_rects_cells.is_empty() && u > 0.15 {
                    let gc = sx / cw;
                    let gr = sy / ch;
                    for ic in &icon_rects_cells {
                        let icx = 0.5 * (ic[0] + ic[2]);
                        let icy = 0.5 * (ic[1] + ic[3]);
                        let hw = 0.5 * (ic[2] - ic[0]) + 1.5;
                        let hh = 0.5 * (ic[3] - ic[1]) + 1.5;
                        let dx = gc - icx;
                        let dy = gr - icy;
                        if dx.abs() < hw && dy.abs() < hh {
                            let push_dir_x = if dx >= 0.0 { 1.0 } else { -1.0 };
                            sx += push_dir_x * (hw - dx.abs()) * cw * u * 0.7;
                        }
                    }
                }
                let rad_px = (thick * shrink).max(4.0);
                let c0 = ((sx - rad_px) / cw).floor() as i32;
                let c1 = ((sx + rad_px) / cw).ceil() as i32;
                let r0 = ((sy - rad_px) / ch).floor() as i32;
                let r1 = ((sy + rad_px) / ch).ceil() as i32;
                let glow_val = self.arm_glow[arm_i][j0] * (1.0 - frac) + self.arm_glow[arm_i][j0 + 1] * frac;
                let tipness = ease((u - 0.30) / 0.62);
                let tone = [
                    (stem_col[0] * (1.0 - tipness) + tip_col[0] * tipness) * depth_dim,
                    (stem_col[1] * (1.0 - tipness) + tip_col[1] * tipness) * depth_dim,
                    (stem_col[2] * (1.0 - tipness) + tip_col[2] * tipness) * depth_dim,
                ];

                for rr in r0.max(0)..r1.min(rows as i32) {
                    for cc in c0.max(0)..c1.min(cols as i32) {
                        let ox = (cc as f32 + 0.5) * cw - sx;
                        let oy = (rr as f32 + 0.5) * ch - sy;
                        let d = (ox * ox + oy * oy).sqrt() / rad_px;
                        if d >= 1.0 {
                            continue;
                        }
                        let lit = (0.5 - 0.5 * (ox * 0.8 + oy * 0.6) / rad_px.max(1.0)).clamp(0.0, 1.0);
                        let rim_arm = d > 0.70;
                        let broken = ((s_idx as f32) * 1.6 + arm_i as f32 * 2.1).sin() > -0.86;
                        let shine = glow_val * (0.55 + 0.45 * depth_dim) * (0.25 + 0.75 * tipness);
                        let val = 0.14 + 0.24 * lit + 0.42 * tipness * tipness + 0.25 * shine;
                        let gl = if rim_arm {
                            line_glyph_cell(-oy, ox, cw, ch)
                        } else {
                            ramp_glyph(FILL_G, (val * 1.25).clamp(0.0, 1.0))
                        };
                        let mult = if rim_arm {
                            (0.50 + 0.38 * lit + 0.22 * tipness) * if broken { 1.0 } else { 0.45 }
                        } else {
                            0.16 + 0.26 * lit + 0.5 * tipness * tipness
                        };
                        let g_boost = shine * if rim_arm { 0.62 } else { 0.34 };
                        let col = [
                            tone[0] * mult + 0.80 * g_boost,
                            tone[1] * mult + 1.00 * g_boost,
                            tone[2] * mult + 0.97 * g_boost,
                        ];
                        let bloom = (55.0 + shine * 180.0).clamp(0.0, 240.0) as u8;
                        self.canvas.plot_z_less(
                            cc,
                            rr,
                            zdepth,
                            gl,
                            [clamp_u8(col[0]), clamp_u8(col[1]), clamp_u8(col[2])],
                            bloom,
                            OWN_WEED,
                        );
                    }
                }
            }
        }

        // 7. Bioluminescent plankton motes (only in open water)
        for (i, mp) in self.motes.iter().enumerate() {
            let wp = [
                mp[0] + self.mote_off[i][0] + 0.03 * (w2 * t + mp[2] * 9.0).sin(),
                mp[1] + self.mote_off[i][1] + 0.03 * (w2 * t + mp[1] * 9.0).sin(),
                mp[2] + self.mote_off[i][2] + 0.03 * (w2 * t + mp[0] * 9.0).sin(),
            ];
            let (sx, sy, _, _) = project(wp);
            let c = (sx / cw).floor() as i32;
            let r = (sy / ch).floor() as i32;
            if let Some(idx) = self.canvas.idx(c, r) {
                if self.canvas.zbuf[idx] < 99.0 {
                    continue;
                }
                let lit = self.mote_lit[i];
                let gl = if lit > 0.55 {
                    glyph_id('*')
                } else if lit > 0.18 {
                    glyph_id('+')
                } else {
                    glyph_id('.')
                };
                let b = 0.16 + 0.84 * lit;
                let hot = (lit - 0.5).max(0.0) * 0.5;
                let col = [
                    0.50 * b + hot,
                    0.92 * b + hot,
                    1.00 * b + hot,
                ];
                self.canvas.set_cell(
                    c,
                    r,
                    gl,
                    [clamp_u8(col[0]), clamp_u8(col[1]), clamp_u8(col[2])],
                    (35.0 + lit * 210.0).min(245.0) as u8,
                    OWN_WATER,
                );
            }
        }

        // 8. Diagonal underwater sunlight shafts
        const BEAMS: [f32; 6] = [-2.6, -1.35, -0.2, 0.95, 2.1, 3.2];
        for r in 0..rows {
            let cy = (r as f32 + 0.5) * ch;
            let depth_fade = (1.15 - 0.55 * cy / virt_h).clamp(0.3, 1.0);
            for c in 0..cols {
                let cx = (c as f32 + 0.5) * cw;
                let mut lit = 0.0f32;
                for (k, &x0) in BEAMS.iter().enumerate() {
                    let foot = virt_w * 0.5
                        + 0.85 * scale * (x0 + 0.22 * (w1 * t + 1.7 * k as f32).sin() - (cam_x + lead + 1.6 * swing));
                    let lean = foot - 0.30 * (cy - mid_y);
                    let wide = 60.0 + 22.0 * (w2 * t + 2.3 * k as f32).sin();
                    let amp = 0.65 + 0.35 * (w3 * t + 1.1 * k as f32).sin();
                    lit += amp * (-((cx - lean) / wide).powi(2)).exp();
                }
                lit *= depth_fade;
                let p = (r * cols + c) * 4;
                if self.canvas.cells[p + 3] == 0 {
                    if lit > 0.30 && (r * 2 + c) % 5 == 0 {
                        self.canvas.cells[p] = clamp_u8((46.0 / 255.0) * lit);
                        self.canvas.cells[p + 1] = clamp_u8((84.0 / 255.0) * lit);
                        self.canvas.cells[p + 2] = clamp_u8((108.0 / 255.0) * lit);
                        self.canvas.cells[p + 3] = glyph_id('/');
                    }
                } else {
                    let boost = 1.0 + 0.42 * lit.min(1.5);
                    for k in 0..3 {
                        self.canvas.cells[p + k] = (self.canvas.cells[p + k] as f32 * boost).min(255.0) as u8;
                    }
                }
            }
        }

        // 9. Prey / user-dropped glowing morsel & eat burst
        if st.food >= 0.0 || self.user_morsel.active {
            let (mpos, mage) = if self.user_morsel.active {
                (self.user_morsel.world, self.user_morsel.age)
            } else {
                (st.prey, st.food)
            };
            let tone = (mage / 0.6).clamp(0.25, 1.0) * (0.85 + 0.15 * (9.0 * t).sin());
            let (sx, sy, _, _) = project(mpos);
            let mc = (sx / cw).floor() as i32;
            let mr = (sy / ch).floor() as i32;
            let ink_p = [clamp_u8(1.0 * tone), clamp_u8(0.84 * tone), clamp_u8(0.42 * tone)];
            self.canvas.plot_z_less(mc, mr, mpos[2] - 0.05, glyph_id('@'), ink_p, 240, OWN_FISH);
            let flick = ((t * 5.0) as usize) % 2;
            for (dc, dr, ch_m, dim) in [
                (-1, 0, '8', 1.0f32),
                (1, 0, '8', 1.0),
                (-2, 0, if flick == 0 { '~' } else { '-' }, 0.7),
                (2, 0, if flick == 0 { '-' } else { '~' }, 0.7),
                (0, -1, 'o', 0.8),
                (0, 1, 'o', 0.6),
            ] {
                let col_m = [
                    clamp_u8(tone * dim),
                    clamp_u8(0.84 * tone * dim),
                    clamp_u8(0.42 * tone * dim),
                ];
                self.canvas.plot_z_less(mc + dc, mr + dr, mpos[2] - 0.05, glyph_id(ch_m), col_m, 180, OWN_FISH);
            }
        }
        if self.user_morsel.eaten_timer > 0.0 {
            let since = 0.85 - self.user_morsel.eaten_timer;
            let fade = (1.0 - since / 0.85).clamp(0.0, 1.0);
            for k in 0..7 {
                let way = [
                    (2.4 * k as f32).cos(),
                    (2.4 * k as f32).sin() - 0.3,
                    0.4 * (1.3 * k as f32).sin(),
                ];
                let dpos = [
                    self.user_morsel.eaten_at[0] + way[0] * 0.20 * since.powf(0.6),
                    self.user_morsel.eaten_at[1] + way[1] * 0.20 * since.powf(0.6),
                    self.user_morsel.eaten_at[2] + way[2] * 0.20 * since.powf(0.6),
                ];
                let (sx, sy, _, _) = project(dpos);
                self.canvas.set_cell(
                    (sx / cw).floor() as i32,
                    (sy / ch).floor() as i32,
                    glyph_id('*'),
                    [clamp_u8(fade), clamp_u8(0.86 * fade), clamp_u8(0.48 * fade)],
                    (220.0 * fade) as u8,
                    OWN_WATER,
                );
            }
        }

        // Gentle pointer sparkle halo in water
        if self.pointer_active && self.params.mouse_interact && self.pointer_impulse > 0.08 {
            let pc = (self.pointer_norm[0] * cols as f32) as i32;
            let pr = (self.pointer_norm[1] * rows as f32) as i32;
            for (dc, dr, g) in [(-1, 0, '.'), (1, 0, '.'), (0, -1, '+'), (0, 1, '+')] {
                if let Some(idx) = self.canvas.idx(pc + dc, pr + dr) {
                    if self.canvas.cells[idx * 4 + 3] == 0 {
                        let f = self.pointer_impulse.min(1.0);
                        self.canvas.set_cell(
                            pc + dc,
                            pr + dr,
                            glyph_id(g),
                            [clamp_u8(0.55 * f), clamp_u8(0.95 * f), clamp_u8(1.0 * f)],
                            180,
                            OWN_WATER,
                        );
                    }
                }
            }
        }

        self.canvas.mark_dirty();
    }
}
