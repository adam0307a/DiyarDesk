pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use shaders::*;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::ffi::c_void;
use std::path::PathBuf;

// =========================================================================
// Constants (1:1 prototype/meadow.html, meadow-boy.js, meadow-cat.js)
// =========================================================================
pub const STEP: f64 = 5.0;
pub const TASKBAR: f64 = 48.0;
pub const CELL_G: usize = 48;
pub const CIRC_K: usize = 36;
pub const MORPH_SEC: f64 = 1.8;
pub const MORPH_EMPTY_SEC: f64 = 2.6;
pub const BOY_RUN_DURATION: f64 = 2.0;
pub const BOY_TURN_DURATION: f64 = 0.22;

// =========================================================================
// Deterministic Noise & Tileable FBM Texture (bit-exact with JS)
// =========================================================================
#[inline]
pub fn hash2(x: i32, y: i32, k: i32, seed: i32) -> f64 {
    let mut h = x
        .wrapping_mul(374_761_393)
        ^ y.wrapping_mul(668_265_263)
        ^ seed.wrapping_add(k).wrapping_mul(2_246_822_519u32 as i32);
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    let u = (h as u32) ^ ((h as u32) >> 16);
    u as f64 / 4_294_967_296.0
}

#[inline]
pub fn sstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn vnoise(x: f64, y: f64, k: i32, seed: i32) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f64;
    let fy = y - iy as f64;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash2(ix, iy, k, seed);
    let b = hash2(ix + 1, iy, k, seed);
    let c = hash2(ix, iy + 1, k, seed);
    let d = hash2(ix + 1, iy + 1, k, seed);
    a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy
}

pub fn fbm(mut x: f64, mut y: f64, k: i32, seed: i32) -> f64 {
    let mut a = 0.5;
    let mut s = 0.0;
    for i in 0..4 {
        s += a * vnoise(x, y, k + i * 7, seed);
        x = x * 2.03 + 17.1;
        y = y * 2.03 + 17.1;
        a *= 0.5;
    }
    s
}

pub fn tile_noise(x: f64, y: f64, px: i32, py: i32, k: i32, seed: i32) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f64;
    let fy = y - iy as f64;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let m = |v: i32, p: i32| ((v % p) + p) % p;
    let x0 = m(ix, px);
    let x1 = m(ix + 1, px);
    let y0 = m(iy, py);
    let y1 = m(iy + 1, py);
    let a = hash2(x0, y0, k, seed);
    let b = hash2(x1, y0, k, seed);
    let c = hash2(x0, y1, k, seed);
    let d = hash2(x1, y1, k, seed);
    a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy
}

pub fn tile_fbm(u: f64, v: f64, periods: &[i32], k: i32, seed: i32) -> f64 {
    let mut s = 0.0;
    let mut w = 0.5;
    let mut tw = 0.0;
    for (i, &p) in periods.iter().enumerate() {
        let pf = p as f64;
        s += w * tile_noise(u * pf, v * pf, p, p, k + (i as i32) * 13, seed);
        tw += w;
        w *= 0.5;
    }
    s / tw
}

pub fn make_noise_image(seed: i32) -> (usize, Vec<u8>) {
    let s = 256usize;
    let mut d = vec![0u8; s * s * 4];
    let chans: [&[i32]; 4] = [
        &[8, 16, 32, 64],
        &[8, 16, 32, 64],
        &[8, 16, 32],
        &[16, 32, 64, 128],
    ];
    for y in 0..s {
        for x in 0..s {
            let u = x as f64 / s as f64;
            let v = y as f64 / s as f64;
            let idx = (y * s + x) * 4;
            for c in 0..4 {
                let n = tile_fbm(u, v, chans[c], 100 + (c as i32) * 50, seed);
                let val = (((n - 0.5) * 1.9 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                d[idx + c] = val;
            }
        }
    }
    (s, d)
}

// =========================================================================
// Parameters & Field Structures
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct MeadowParams {
    pub sigma: f64,
    pub warp: f64,
    pub edge: f64,
    pub cover: f64,
    pub flow: f64,
    pub show_boy: bool,
    pub show_cat: bool,
    pub adaptive: bool,
    pub seed: i32,
    pub debug: bool,
}

impl Default for MeadowParams {
    fn default() -> Self {
        Self {
            sigma: 72.0,
            warp: 40.0,
            edge: 10.0,
            cover: 0.3,
            flow: 1.0,
            show_boy: true,
            show_cat: true,
            adaptive: true,
            seed: 42,
            debug: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct IconBox {
    pub cx: f64,
    pub cy: f64,
    pub hw: f64,
    pub hh: f64,
}

impl IconBox {
    pub fn from_rect(r: &Rect) -> Self {
        Self {
            cx: (r.x + r.w * 0.5) as f64,
            cy: (r.y + r.h * 0.5) as f64,
            hw: (r.w * 0.5 - 4.0) as f64,
            hh: (r.h * 0.5 - 2.0) as f64,
        }
    }
}

#[derive(Clone)]
pub struct SkyCloud {
    pub x: f64,
    pub y: f64,
    pub circles: Vec<[f64; 4]>, // dx, dy, r, ph
    pub v: f64,
    pub grow: f64,
    pub hw: f64,
    pub dying: bool,
}

#[inline]
pub fn wob_x(x: f64, y: f64, r: f64, ph: f64, flow_t: f64) -> f64 {
    (4.5 + 0.12 * r)
        * (0.85 * (flow_t * 0.24 + x * 0.011 + y * 0.004).sin()
            + 0.3 * (flow_t * 0.3 + ph * 6.283).sin())
}

#[inline]
pub fn wob_y(x: f64, y: f64, r: f64, ph: f64, flow_t: f64) -> f64 {
    (4.5 + 0.12 * r)
        * (0.65 * (flow_t * 0.2 + y * 0.013 - x * 0.006 + 1.7).sin()
            + 0.25 * (flow_t * 0.28 + ph * 6.283).cos())
}

// Fast Mulberry32 PRNG (identical to JS)
#[derive(Clone, Copy)]
pub struct Mulberry32 {
    state: i32,
}

impl Mulberry32 {
    pub fn new(seed: i32) -> Self {
        Self { state: seed }
    }

    pub fn next(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6D2B_79F5u32 as i32);
        let a = self.state;
        let mut t = (a ^ ((a as u32) >> 15) as i32).wrapping_mul(1 | a);
        t ^= t.wrapping_add((t ^ ((t as u32) >> 7) as i32).wrapping_mul(61 | t));
        let u = (t as u32) ^ ((t as u32) >> 14);
        u as f64 / 4_294_967_296.0
    }
}

// =========================================================================
// Kite Boy Pure Kinematics (1:1 prototype/meadow-boy.js)
// =========================================================================
#[derive(Clone, Copy, Default)]
pub struct TripPose {
    pub frame: i32,
    pub back: bool,
    pub progress: f64,
    pub running: bool,
    pub squeeze: f64,
    pub phase: f64,
    pub gait: f64,
    pub bob: f64,
    pub done: bool,
}

pub fn meadow_boy_trip_pose(
    elapsed: f64,
    cycles: f64,
    duration: f64,
    enter: bool,
    exit: bool,
) -> TripPose {
    let ease = |t: f64| {
        let u = t.clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    let turn = if enter { 0.0 } else { BOY_TURN_DURATION };
    let run_end = turn + duration;
    let turn_back_at = run_end + 0.18;
    let stride_lead = if enter { 0.0 } else { 0.07 };

    if elapsed < turn - stride_lead {
        let t = (elapsed / turn).max(0.0);
        return TripPose {
            frame: if t < 0.25 {
                3
            } else if t < 0.68 {
                0
            } else {
                3
            },
            back: t >= 0.25,
            progress: 0.0,
            running: false,
            squeeze: 1.0 - 0.2 * (t * PI).sin(),
            phase: 0.0,
            gait: 0.0,
            bob: 0.0,
            done: false,
        };
    }
    if elapsed < run_end {
        let run_time = elapsed - turn;
        let step_time = (run_time + stride_lead).max(0.0);
        let plant_time = if enter { 0.0 } else { 0.02 };
        let move_time = (run_time - plant_time).max(0.0);
        let move_duration = duration - plant_time;
        let accelerate = 0.16;
        let decelerate = 0.22;
        let ramp_integral = |t: f64, r: f64| {
            if t >= r {
                t - r * 0.5
            } else {
                let u = (t / r).max(0.0);
                r * (u * u * u - 0.5 * u * u * u * u)
            }
        };
        let progress = ((ramp_integral(move_time, accelerate)
            - ramp_integral((move_time - move_duration + decelerate).max(0.0), decelerate))
            / (move_duration - (accelerate + decelerate) * 0.5))
            .clamp(0.0, 1.0);
        let lead = if enter { 0.0 } else { 0.18 };
        let phase = (lead * ease(step_time / 0.10) + (cycles - lead) * progress) * PI * 2.0;
        let gait = ease(step_time / 0.07) * ease((duration - run_time) / 0.12);
        let squeeze = if elapsed < turn {
            1.0 - 0.2 * (elapsed / turn * PI).sin()
        } else {
            1.0
        };
        return TripPose {
            frame: 3,
            back: true,
            progress,
            running: true,
            squeeze,
            phase,
            gait,
            bob: -(1.0 - (phase * 2.0).cos()) * 0.65 * gait,
            done: false,
        };
    }
    if exit {
        return TripPose {
            frame: 3,
            back: true,
            progress: 1.0,
            running: false,
            squeeze: 1.0,
            phase: 0.0,
            gait: 0.0,
            bob: 0.0,
            done: true,
        };
    }
    if elapsed < turn_back_at {
        return TripPose {
            frame: 3,
            back: true,
            progress: 1.0,
            running: false,
            squeeze: 1.0,
            phase: 0.0,
            gait: 0.0,
            bob: 0.0,
            done: false,
        };
    }
    if elapsed < turn_back_at + BOY_TURN_DURATION {
        let t = (elapsed - turn_back_at) / BOY_TURN_DURATION;
        return TripPose {
            frame: if t < 0.32 { 3 } else { 0 },
            back: t < 0.75,
            progress: 1.0,
            running: false,
            squeeze: 1.0 - 0.2 * (t * PI).sin(),
            phase: 0.0,
            gait: 0.0,
            bob: 0.0,
            done: false,
        };
    }
    TripPose {
        frame: 3,
        back: false,
        progress: 1.0,
        running: false,
        squeeze: 1.0,
        phase: 0.0,
        gait: 0.0,
        bob: 0.0,
        done: true,
    }
}

#[derive(Clone, Copy)]
pub struct LegRig {
    pub hip: [f64; 2],
    pub knee: [f64; 2],
    pub ankle: [f64; 2],
    pub angle: f64,
    pub contact: bool,
    pub lift: f64,
}

#[derive(Clone, Copy)]
pub struct RunningRig {
    pub left: LegRig,
    pub right: LegRig,
    pub arm: f64,
    pub reel_arm: f64,
}

pub fn meadow_boy_running_rig(activity: &TripPose) -> RunningRig {
    let weight = activity.gait;
    let phase = activity.phase;
    let bob = activity.bob;
    let solve = |hip: [f64; 2],
                 knee: [f64; 2],
                 ankle: [f64; 2],
                 offset: f64,
                 front: [f64; 2],
                 back: [f64; 2],
                 lift: f64,
                 kick: f64|
     -> LegRig {
        let u = ((phase / (PI * 2.0) + offset) % 1.0 + 1.0) % 1.0;
        let swing = u >= 0.5;
        let t = if swing { (u - 0.5) * 2.0 } else { u * 2.0 };
        let recovery = ((t - 0.18) / 0.82).max(0.0);
        let step = if swing {
            recovery * recovery * (3.0 - 2.0 * recovery)
        } else {
            t
        };
        let fold = if swing { (PI * t).sin().powi(2) } else { 0.0 };
        let (a, b) = if swing { (back, front) } else { (front, back) };
        let x = a[0] + (b[0] - a[0]) * step + kick * fold;
        let y = a[1] + (b[1] - a[1]) * step;
        let height = if swing {
            (fold * lift).min((y - 5.0).max(0.0)) * weight
        } else {
            0.0
        };
        let mut foot = [
            ankle[0] + (hip[0] + x - ankle[0]) * weight,
            ankle[1] + (hip[1] + y - ankle[1]) * weight - bob - height,
        ];
        let upper = (knee[0] - hip[0]).hypot(knee[1] - hip[1]);
        let lower = (ankle[0] - knee[0]).hypot(ankle[1] - knee[1]);
        let dx = foot[0] - hip[0];
        let dy = foot[1] - hip[1];
        let raw = dx.hypot(dy);
        let distance = raw.clamp((upper - lower).abs() + 0.001, upper + lower - 0.001);
        let ux = dx / raw.max(0.001);
        let uy = dy / raw.max(0.001);
        foot[0] = hip[0] + ux * distance;
        foot[1] = hip[1] + uy * distance;
        let along = (upper * upper - lower * lower + distance * distance) / (2.0 * distance);
        let bend = (upper * upper - along * along).max(0.0).sqrt();
        let joint = [hip[0] + ux * along - uy * bend, hip[1] + uy * along + ux * bend];
        LegRig {
            hip,
            knee: joint,
            ankle: foot,
            angle: -fold * 1.05 * weight,
            contact: !swing,
            lift: height,
        }
    };
    RunningRig {
        left: solve(
            [-12.0, 37.0],
            [-14.0, 47.0],
            [-15.0, 57.0],
            0.0,
            [-7.5, 8.5],
            [1.4, 19.0],
            19.0,
            9.0,
        ),
        right: solve(
            [14.0, 35.0],
            [18.0, 44.0],
            [21.0, 51.0],
            0.5,
            [0.5, 7.2],
            [7.0, 15.8],
            16.0,
            5.0,
        ),
        arm: -phase.sin() * 0.42 * weight,
        reel_arm: (phase.sin() * 0.26 + (phase * 2.0).sin() * 0.035) * weight,
    }
}

#[derive(Clone, Copy, Default)]
pub struct TugPose {
    pub active: bool,
    pub reach: f64,
    pub pull: f64,
}

pub fn meadow_boy_tug_pose(age: f64) -> TugPose {
    let ease = |t: f64| {
        let u = t.clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    if !(0.0..1.12).contains(&age) {
        return TugPose::default();
    }
    let reach = ease(age / 0.16) * (1.0 - ease((age - 0.16) / 0.20));
    let pull = ease((age - 0.16) / 0.20) * (1.0 - ease((age - 0.50) / 0.62));
    TugPose {
        active: true,
        reach,
        pull,
    }
}

#[derive(Clone, Copy, Default)]
pub struct KiteReaction {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub angle: f64,
    pub spin: f64,
}

pub fn meadow_boy_kite_reaction(r: &mut KiteReaction, dt: f64, impulse: f64) {
    r.vx -= 100.0 * impulse;
    r.vy -= 120.0 * impulse;
    r.spin += 2.8 * impulse;
    r.vx += (-18.0 * r.x - 3.4 * r.vx) * dt;
    r.vy += (-22.0 * r.y - 4.0 * r.vy) * dt;
    r.spin += (-24.0 * r.angle - 3.8 * r.spin) * dt;
    r.x += r.vx * dt;
    r.y += r.vy * dt;
    r.angle += r.spin * dt;
}

pub fn meadow_boy_wave_pull(time: f64, tug: &TugPose) -> f64 {
    if tug.active {
        return 0.0;
    }
    let wave = (((time % 9.0) - 4.0) / 0.6).clamp(0.0, 1.0);
    (PI * wave).sin().powi(2)
}

#[derive(Clone, Copy)]
pub struct BoyPose {
    pub head: f64,
    pub look: f64,
    pub nod: f64,
    pub reel: f64,
    pub pull: f64,
    pub tug_offset: [f64; 2],
    pub body: [f64; 2],
    pub legs: [f64; 2],
    pub hand: [f64; 2],
    pub reel_hand: [f64; 2],
}

pub fn meadow_boy_pose(time: f64, tug: &TugPose) -> BoyPose {
    let pull = (0.5 + 0.5 * (time * 1.35).sin()).powi(3);
    let shift = (time * 1.05).sin();
    let body = [
        (time * 0.85).sin() * 1.7 - tug.pull * 1.8,
        (time * 1.7).sin() * 0.65 - tug.pull * 0.8,
    ];
    let tug_offset = [
        tug.reach * 4.0 - tug.pull * 12.0,
        tug.reach * 3.0 - tug.pull * 8.0,
    ];
    let look = 1.0;
    let reel = -0.18
        + if tug.active {
            -tug.pull * 0.22
        } else {
            meadow_boy_wave_pull(time, tug) * 1.14
        };
    let angle = reel + 0.18;
    let (c, s) = (angle.cos(), angle.sin());
    let arm_point = |x: f64, y: f64| -> [f64; 2] {
        let q = [
            -21.0 + c * (x + 21.0) - s * (y + 9.0) - tug_offset[0] * 0.35,
            -9.0 + s * (x + 21.0) + c * (y + 9.0) - tug_offset[1] * 0.35,
        ];
        let py = (q[1] + body[1] * 78.0 / 170.0) / (1.0 + body[1] / 170.0);
        [q[0] + body[0] * (78.0 - py) / 170.0, py]
    };
    let reel_hand = arm_point(-49.0, -45.5);
    BoyPose {
        head: 0.0,
        look,
        nod: (time * 0.9 + 0.4).sin() * 1.5,
        reel,
        pull,
        tug_offset,
        body,
        legs: [shift.max(0.0) * 3.8, (-shift).max(0.0) * 3.2],
        hand: reel_hand,
        reel_hand,
    }
}

#[derive(Clone, Copy)]
pub struct GroundPose {
    pub line: [f64; 2],
    pub contacts: [[f64; 3]; 2],
    #[allow(dead_code)]
    pub soles: [[f64; 2]; 2],
}

pub fn meadow_boy_ground_pose(
    activity: &TripPose,
    rig: &RunningRig,
    pose: &BoyPose,
) -> GroundPose {
    let (feet, soles) = if activity.running {
        let heel = |leg: &LegRig, offset: [f64; 2]| -> [f64; 2] {
            let (c, s) = (leg.angle.cos(), leg.angle.sin());
            [
                leg.ankle[0] + c * offset[0] - s * offset[1],
                leg.ankle[1] + s * offset[0] + c * offset[1] + activity.bob - 10.0,
            ]
        };
        let soles = [
            heel(&rig.left, [-0.52, 15.94]),
            heel(&rig.right, [-3.42, 15.63]),
        ];
        let feet = [
            [soles[0][0], soles[0][1] + rig.left.lift],
            [soles[1][0], soles[1][1] + rig.right.lift],
        ];
        (feet, soles)
    } else if activity.back {
        let rest = if activity.frame == 0 {
            [[-10.06, 78.11], [23.76, 72.66]]
        } else {
            [[-15.52, 72.94], [17.58, 66.63]]
        };
        let feet = [
            [rest[0][0] * activity.squeeze, rest[0][1] + activity.bob - 10.0],
            [rest[1][0] * activity.squeeze, rest[1][1] + activity.bob - 10.0],
        ];
        (feet, feet)
    } else {
        let feet = [
            [-22.5 * activity.squeeze, 72.0],
            [37.8 * activity.squeeze, 63.3],
        ];
        let soles = [
            [
                feet[0][0] - pose.legs[0] * 0.24 * activity.squeeze,
                feet[0][1] - pose.legs[0],
            ],
            [
                feet[1][0] + pose.legs[1] * 0.24 * activity.squeeze,
                feet[1][1] - pose.legs[1],
            ],
        ];
        (feet, soles)
    };

    let dx = feet[1][0] - feet[0][0];
    let sign = if dx == 0.0 {
        1.0
    } else if dx > 0.0 {
        1.0
    } else {
        -1.0
    };
    let spacing = sign * dx.abs().max(12.0);
    let slope = if activity.running {
        -0.19
    } else {
        ((feet[1][1] - feet[0][1]) / spacing).clamp(-0.8, 0.5)
    };
    let intercept = if activity.running {
        let sup = if rig.left.contact { soles[0] } else { soles[1] };
        sup[1] - slope * sup[0]
    } else {
        (feet[0][1] + feet[1][1] - slope * (feet[0][0] + feet[1][0])) * 0.5
    };
    let contacts = [
        [
            feet[0][0],
            feet[0][1] - 0.5,
            if activity.running {
                if rig.left.contact { 1.0 } else { 0.22 }
            } else {
                1.0
            },
        ],
        [
            feet[1][0],
            feet[1][1] - 0.5,
            if activity.running {
                if rig.right.contact { 1.0 } else { 0.22 }
            } else {
                1.0
            },
        ],
    ];
    GroundPose {
        line: [slope, intercept],
        contacts,
        soles,
    }
}

#[derive(Clone, Copy)]
pub struct RunDestination {
    pub x: f64,
    pub y: f64,
    pub exit: bool,
}

pub fn meadow_boy_run_destination(
    from: [f64; 2],
    s: f64,
    _width: f64,
    height: f64,
    taskbar: f64,
) -> RunDestination {
    let distance = ((height - taskbar) * 0.27).max(165.0).min(315.0);
    let exit = from[1] - distance < 116.0 * s + 24.0;
    let y = if exit {
        -610.0 * s - 24.0
    } else {
        from[1] - distance
    };
    let x = (96.0 * s + 12.0).max(from[0] - (from[1] - y) * 0.65);
    RunDestination { x, y, exit }
}

// =========================================================================
// Black Kitten Pure Placement & Tail Animation (1:1 prototype/meadow-cat.js)
// =========================================================================
pub fn meadow_cat_segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let denom = dx * dx + dy * dy;
    let t = if denom == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / denom).clamp(0.0, 1.0)
    };
    (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dy * t)
}

pub fn meadow_cat_run_lanes(
    boy_pos: [f64; 2],
    route_target: Option<[f64; 2]>,
    width: f64,
    height: f64,
    taskbar: f64,
) -> (Vec<([f64; 2], [f64; 2])>, f64) {
    let scale = (width / 1379.0)
        .min((height - taskbar) / 1141.0)
        .min(1.25)
        .max(0.24)
        * 0.35;
    let mut lanes = Vec::new();
    let finish = |mut from: [f64; 2], lanes: &mut Vec<([f64; 2], [f64; 2])>| {
        for _ in 0..64 {
            let to = meadow_boy_run_destination(from, scale, width, height, taskbar);
            let to_pt = [to.x, to.y];
            lanes.push((from, to_pt));
            from = to_pt;
            if to.exit {
                break;
            }
        }
    };
    let current = if let Some(rt) = route_target {
        lanes.push((boy_pos, rt));
        rt
    } else {
        boy_pos
    };
    finish(current, &mut lanes);
    let bottom = [
        (96.0 * scale + 12.0).max((width * 0.70).min(width - 612.0 * scale - 12.0)),
        height + 116.0 * scale + 24.0,
    ];
    let y = (height - taskbar) * 0.70;
    let entry = [(96.0 * scale + 12.0).max(bottom[0] - (bottom[1] - y) * 0.65), y];
    lanes.push((bottom, entry));
    finish(entry, &mut lanes);
    (lanes, scale)
}

#[derive(Clone, Copy)]
pub struct CatPlace {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    #[allow(dead_code)]
    pub clearance: f64,
    #[allow(dead_code)]
    pub route_distance: f64,
    #[allow(dead_code)]
    pub lane_count: usize,
}

pub fn meadow_cat_choose_place(
    boy_pos: [f64; 2],
    route_target: Option<[f64; 2]>,
    width: f64,
    height: f64,
    taskbar: f64,
    scene_seed: i32,
    desktop_icons: &[Rect],
    cloud_size: f64,
) -> Option<CatPlace> {
    let (lanes, scale) = meadow_cat_run_lanes(boy_pos, route_target, width, height, taskbar);
    let size = ((width / 1379.0).min((height - taskbar) / 1141.0) * 78.0)
        .min(90.0)
        .max(28.0)
        * 0.578;
    let radius = size * 0.58;
    let pad = radius + 12.0;
    let clearance = radius + 96.0 * scale + 14.0;
    if width <= pad * 2.0 || height - taskbar <= pad * 2.0 {
        return None;
    }
    let mut rng = Mulberry32::new(scene_seed ^ 0x71A4_3D25);
    let mut best: Option<CatPlace> = None;
    let mut best_score = f64::INFINITY;
    for i in 0..337 {
        let (x, y) = if i < 256 {
            (rng.next(), rng.next())
        } else {
            let idx = i - 256;
            (((idx % 9) as f64 + 0.5) / 9.0, ((idx / 9) as f64 + 0.5) / 9.0)
        };
        let p = [
            pad + x * (width - pad * 2.0),
            pad + y * (height - taskbar - pad * 2.0),
        ];
        let mut route_distance = f64::INFINITY;
        for &(a, b) in &lanes {
            route_distance = route_distance.min(meadow_cat_segment_distance(p, a, b));
        }
        if route_distance < clearance {
            continue;
        }
        let mut score = 0.0;
        for ic in desktop_icons {
            let dx = (ic.x as f64 - p[0]).max(0.0).max(p[0] - (ic.x + ic.w) as f64);
            let dy = (ic.y as f64 - p[1]).max(0.0).max(p[1] - (ic.y + ic.h) as f64);
            score += (radius + cloud_size + 25.0 - dx.hypot(dy)).max(0.0);
        }
        if score < best_score {
            best = Some(CatPlace {
                x: p[0],
                y: p[1],
                size,
                clearance,
                route_distance,
                lane_count: lanes.len(),
            });
            best_score = score;
        }
        if score == 0.0 {
            break;
        }
    }
    best
}

pub fn meadow_cat_tail_angle(time: f64, scene_seed: i32) -> f64 {
    let period = 10.0 + (scene_seed % 4).abs() as f64;
    let offset = (scene_seed % 1300).abs() as f64 / 1000.0;
    let age = ((time + offset) % period + period) % period - 3.0;
    if !(0.0..1.6).contains(&age) {
        return 0.0;
    }
    let t = age / 1.6;
    (t * PI * 4.0).sin() * (t * PI).sin().powi(2) * 0.34
}

// =========================================================================
// Live Boy & Cat Simulation State
// =========================================================================
#[derive(Clone)]
struct BoyTrip {
    start: f64,
    from: [f64; 2],
    to: [f64; 2],
    duration: f64,
    cycles: f64,
    enter: bool,
    exit: bool,
    after: Option<[f64; 2]>,
}

#[derive(Clone)]
pub struct BoyFrameData {
    pub position: [f64; 2],
    pub route_target: [f64; 2],
    pub scale: f64,
    pub kx: f64,
    pub ky: f64,
    pub kite_rock: f64,
    pub run_wind: f64,
    pub line_pull: f64,
    pub pose: BoyPose,
    pub activity: TripPose,
    pub rig: RunningRig,
    pub ground: GroundPose,
    pub time: f64,
    pub layout_sig: (i32, i32),
}

pub struct MeadowBoyState {
    pub time: f64,
    position: Option<[f64; 2]>,
    target: Option<[f64; 2]>,
    layout_sig: (i32, i32),
    trip: Option<BoyTrip>,
    next_run_at: f64,
    kite_position: Option<[f64; 2]>,
    last_scale: f64,
    return_at: Option<f64>,
    next_pull_at: f64,
    pull_started: f64,
    last_line_pull: f64,
    kite_reaction: KiteReaction,
    pub last_frame: Option<BoyFrameData>,
}

impl Default for MeadowBoyState {
    fn default() -> Self {
        Self {
            time: 0.0,
            position: None,
            target: None,
            layout_sig: (0, 0),
            trip: None,
            next_run_at: 7.0,
            kite_position: None,
            last_scale: 0.0,
            return_at: None,
            next_pull_at: 2.2,
            pull_started: f64::NEG_INFINITY,
            last_line_pull: 0.0,
            kite_reaction: KiteReaction::default(),
            last_frame: None,
        }
    }
}

impl MeadowBoyState {
    fn start_trip(&self, from: [f64; 2], to: [f64; 2], s: f64, enter: bool, exit: bool) -> BoyTrip {
        let distance = (to[0] - from[0]).hypot(to[1] - from[1]);
        let cycles = (distance / (s * 80.0)).round().max(2.0);
        let duration = BOY_RUN_DURATION.max(distance / 55.0).max(cycles * 0.52);
        BoyTrip {
            start: self.time,
            from,
            to,
            duration,
            cycles,
            enter,
            exit,
            after: None,
        }
    }

    fn choose_position(&self, s: f64, w: f64, h: f64) -> [f64; 2] {
        let mut best: Option<[f64; 2]> = None;
        let mut best_score = f64::INFINITY;
        let mut candidates = Vec::with_capacity(21);
        if let Some(p) = self.position {
            candidates.push(p);
        }
        for &fy in &[0.48, 0.38, 0.58, 0.28] {
            for &fx in &[0.50, 0.62, 0.38, 0.70, 0.28] {
                candidates.push([w * fx, (h - TASKBAR) * fy]);
            }
        }
        let preferred = self.position.unwrap_or([w * 0.5, (h - TASKBAR) * 0.48]);
        for c in candidates {
            if c[0] - 96.0 * s < 12.0
                || c[0] + 612.0 * s > w - 12.0
                || c[1] - 116.0 * s < 12.0
                || c[1] + 610.0 * s > h - TASKBAR - 12.0
            {
                continue;
            }
            let score = (c[0] - preferred[0]).hypot(c[1] - preferred[1]) * 0.01;
            if score < best_score {
                best = Some(c);
                best_score = score;
            }
        }
        best.unwrap_or([w * 0.48, (h - TASKBAR) * 0.40])
    }

    pub fn step(&mut self, dt: f64, w: f64, h: f64) -> Option<BoyFrameData> {
        let dt = dt.clamp(0.0, 0.05);
        self.time += dt;
        let s = (w / 1379.0)
            .min((h - TASKBAR) / 1141.0)
            .min(1.25)
            .max(0.24)
            * 0.35;
        let sig = (w.round() as i32, h.round() as i32);
        if self.layout_sig != sig {
            self.layout_sig = sig;
            let tgt = self.choose_position(s, w, h);
            self.target = Some(tgt);
            if self.trip.is_some() {
                if let Some(pos) = self.position {
                    self.trip = Some(BoyTrip {
                        start: self.time - (BOY_TURN_DURATION + BOY_RUN_DURATION),
                        from: pos,
                        to: pos,
                        duration: BOY_RUN_DURATION,
                        cycles: 4.0,
                        enter: false,
                        exit: false,
                        after: Some(tgt),
                    });
                }
            }
            self.next_run_at = self.time + 7.0;
        }
        if let Some(ret_t) = self.return_at {
            if self.time < ret_t {
                self.last_frame = None;
                return None;
            }
            let pos = [
                (96.0 * s + 12.0).max((w * 0.70).min(w - 612.0 * s - 12.0)),
                h + 116.0 * s + 24.0,
            ];
            let y = (h - TASKBAR) * 0.70;
            let dest = [(96.0 * s + 12.0).max(pos[0] - (pos[1] - y) * 0.65), y];
            self.position = Some(pos);
            self.target = Some(dest);
            self.trip = Some(self.start_trip(pos, dest, s, true, false));
            self.kite_position = None;
            self.return_at = None;
            self.pull_started = f64::NEG_INFINITY;
            self.last_line_pull = 0.0;
            self.kite_reaction = KiteReaction::default();
        }
        let tgt = self.target.unwrap_or_else(|| self.choose_position(s, w, h));
        if self.position.is_none() {
            self.position = Some(tgt);
        }
        let mut pos = self.position.unwrap();
        if self.trip.is_none() {
            let mv = 1.0 - (-dt * 0.7).exp();
            pos[0] += (tgt[0] - pos[0]) * mv;
            pos[1] += (tgt[1] - pos[1]) * mv;
            if self.time >= self.next_run_at && (pos[0] - tgt[0]).hypot(pos[1] - tgt[1]) < 2.0 {
                let dest = meadow_boy_run_destination(pos, s, w, h, TASKBAR);
                self.trip = Some(self.start_trip(pos, [dest.x, dest.y], s, false, dest.exit));
                self.pull_started = f64::NEG_INFINITY;
            }
        }
        let activity = if let Some(ref tr) = self.trip {
            meadow_boy_trip_pose(self.time - tr.start, tr.cycles, tr.duration, tr.enter, tr.exit)
        } else {
            meadow_boy_trip_pose(6.0, 4.0, BOY_RUN_DURATION, false, false)
        };
        let mut route_target = tgt;
        if let Some(tr) = self.trip.clone() {
            pos[0] = tr.from[0] + (tr.to[0] - tr.from[0]) * activity.progress;
            pos[1] = tr.from[1] + (tr.to[1] - tr.from[1]) * activity.progress;
            route_target = tr.to;
            if activity.done {
                self.position = Some(pos);
                if tr.exit {
                    self.trip = None;
                    self.return_at = Some(self.time + 6.0);
                    self.last_frame = None;
                    return None;
                }
                self.target = Some(tr.after.unwrap_or(pos));
                self.trip = None;
                self.next_run_at = self.time + 18.0;
                self.next_pull_at = self.next_pull_at.max(self.time + 2.2);
            }
        }
        self.position = Some(pos);

        let wave_time = self.time % 9.0;
        if self.trip.is_none()
            && !activity.back
            && self.time >= self.next_pull_at
            && self.next_run_at - self.time > 3.6
            && (wave_time < 2.8 || wave_time > 5.0)
        {
            self.pull_started = self.time;
            self.next_pull_at = self.time + 7.8;
        }
        let tug = meadow_boy_tug_pose(self.time - self.pull_started);
        let line_pull = if activity.back {
            0.0
        } else {
            tug.pull.max(meadow_boy_wave_pull(self.time, &tug))
        };
        meadow_boy_kite_reaction(
            &mut self.kite_reaction,
            dt,
            (line_pull - self.last_line_pull).max(0.0),
        );
        self.last_line_pull = line_pull;

        let run_wind = if activity.running {
            (PI * activity.progress).sin()
        } else {
            0.0
        };
        let gust = ((self.time * 4.8).sin() * 60.0 + (self.time * 7.1).sin() * 8.0) * run_wind;
        let base_kx =
            215.0 + (self.time * 0.72).sin() * 21.0 + (self.time * 1.23).sin() * 5.0 + gust;
        let base_ky = 205.0 + (self.time * 0.49 + 0.6).sin() * 10.0;
        let desired_kite = [pos[0] + base_kx * s, pos[1] + base_ky * s];
        if self.kite_position.is_none() || (self.last_scale - s).abs() > 1e-9 {
            self.kite_position = Some(desired_kite);
        }
        self.last_scale = s;
        let follow = 1.0 - (-dt / 0.23).exp();
        let kp = self.kite_position.as_mut().unwrap();
        kp[0] += (desired_kite[0] - kp[0]) * follow;
        kp[1] += (desired_kite[1] - kp[1]) * follow;

        let kx = base_kx
            + ((kp[0] - desired_kite[0]) / s).clamp(-30.0, 32.0)
            + self.kite_reaction.x
            - line_pull * 10.0;
        let ky = base_ky
            + ((kp[1] - desired_kite[1]) / s).clamp(-30.0, 32.0)
            + self.kite_reaction.y
            - line_pull * 12.0;
        let kite_rock = self.kite_reaction.angle + line_pull * 0.10;
        let mut pose = meadow_boy_pose(self.time, &tug);
        let rig = meadow_boy_running_rig(&activity);
        let ground = meadow_boy_ground_pose(&activity, &rig, &pose);
        if activity.back {
            let grips = [[44.0, -23.0], [37.0, -23.0], [49.0, -25.0], [37.0, -23.0]];
            let grip = grips[(activity.frame as usize).min(3)];
            let angle = if activity.running { rig.reel_arm } else { 0.0 };
            let (c, sn) = (angle.cos(), angle.sin());
            pose.hand = [
                (22.0 + c * (grip[0] - 22.0) - sn * (grip[1] + 7.0)) * activity.squeeze,
                -7.0 + sn * (grip[0] - 22.0) + c * (grip[1] + 7.0) - 10.0 + activity.bob,
            ];
            pose.reel_hand = pose.hand;
        } else {
            pose.hand[0] *= activity.squeeze;
            pose.reel_hand[0] *= activity.squeeze;
        }

        let frame = BoyFrameData {
            position: pos,
            route_target,
            scale: s,
            kx,
            ky,
            kite_rock,
            run_wind,
            line_pull,
            pose,
            activity,
            rig,
            ground,
            time: self.time,
            layout_sig: sig,
        };
        self.last_frame = Some(frame.clone());
        Some(frame)
    }
}

pub struct MeadowCatState {
    pub time: f64,
    layout_sig: (i32, i32),
    place_sig: (i32, i32, i32, i32),
    route_pos: Option<[f64; 2]>,
    route_target: Option<[f64; 2]>,
    pub place: Option<CatPlace>,
}

impl Default for MeadowCatState {
    fn default() -> Self {
        Self {
            time: 0.0,
            layout_sig: (0, 0),
            place_sig: (0, 0, 0, 0),
            route_pos: None,
            route_target: None,
            place: None,
        }
    }
}

impl MeadowCatState {
    pub fn step(
        &mut self,
        dt: f64,
        w: f64,
        h: f64,
        seed: i32,
        sigma: f64,
        icons: &[Rect],
        boy: Option<&BoyFrameData>,
    ) {
        self.time += dt.clamp(0.0, 0.05);
        let sig = (w.round() as i32, h.round() as i32);
        if self.layout_sig != sig {
            self.layout_sig = sig;
            self.route_pos = None;
            self.route_target = None;
            self.place = None;
            self.place_sig = (0, 0, 0, 0);
        }
        if let Some(b) = boy {
            if b.layout_sig == sig {
                self.route_pos = Some(b.position);
                self.route_target = Some(b.route_target);
            }
        }
        let Some(rp) = self.route_pos else {
            return;
        };
        let psig = (sig.0, sig.1, seed, sigma.round() as i32);
        if self.place_sig != psig || self.place.is_none() {
            self.place_sig = psig;
            self.place =
                meadow_cat_choose_place(rp, self.route_target, w, h, TASKBAR, seed, icons, sigma);
        }
    }
}

// =========================================================================
// Meadow World State (Field, Morphing, Cloud Circles, Boy & Cat)
// =========================================================================
pub struct MeadowState {
    pub p: MeadowParams,
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
    pub flow_t: f64,
    pub icons: Vec<Rect>,
    pub sky_clouds: Vec<SkyCloud>,
    pub mass_list: Vec<f64>,
    pub mass_t: f64,
    pub mass_w: f64,
    pub mass_h: f64,
    pub rng: Mulberry32,
    pub boy: MeadowBoyState,
    pub cat: MeadowCatState,
    pub icons_seen: bool,
    pub first_icons: bool,
}

impl Default for MeadowState {
    fn default() -> Self {
        let mut st = Self {
            p: MeadowParams::default(),
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
            flow_t: 0.0,
            icons: Vec::new(),
            sky_clouds: Vec::new(),
            mass_list: Vec::new(),
            mass_t: -1e9,
            mass_w: 0.0,
            mass_h: 0.0,
            rng: Mulberry32::new(42 ^ 0x4D45_4144),
            boy: MeadowBoyState::default(),
            cat: MeadowCatState::default(),
            icons_seen: false,
            first_icons: true,
        };
        st.resize(1280.0, 720.0);
        st.refresh();
        st
    }
}

impl MeadowState {
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

    pub fn sample_arr(a: &[f32], gw: usize, gh: usize, x: f64, y: f64) -> f64 {
        if a.is_empty() || gw < 2 || gh < 2 {
            return 999.0;
        }
        let fx = (x / STEP).clamp(0.0, gw as f64 - 1.001);
        let fy = (y / STEP).clamp(0.0, gh as f64 - 1.001);
        let ix = fx.floor() as usize;
        let iy = fy.floor() as usize;
        let tx = fx - ix as f64;
        let ty = fy - iy as f64;
        let i = iy * gw + ix;
        let v00 = a[i] as f64;
        let v10 = a[i + 1] as f64;
        let v01 = a[i + gw] as f64;
        let v11 = a[i + gw + 1] as f64;
        (v00 * (1.0 - tx) + v10 * tx) * (1.0 - ty) + (v01 * (1.0 - tx) + v11 * tx) * ty
    }

    #[inline]
    pub fn cur_s(&self, x: f64, y: f64) -> f64 {
        if self.fb.is_empty() {
            return 999.0;
        }
        Self::sample_arr(&self.fa, self.gw, self.gh, x, y) * (1.0 - self.cur_mix)
            + Self::sample_arr(&self.fb, self.gw, self.gh, x, y) * self.cur_mix
    }

    fn icon_boxes(&self) -> Vec<IconBox> {
        if !self.p.adaptive {
            return Vec::new();
        }
        self.icons.iter().map(IconBox::from_rect).collect()
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
        self.mass_t = -1e9;
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
        self.mass_t = -1e9;
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
        self.sky_clouds.clear();
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

    pub fn sync_params(&mut self, next: MeadowParams) {
        let seed_changed = next.seed != self.p.seed;
        let adaptive_changed = next.adaptive != self.p.adaptive;
        let sigma_changed = (next.sigma - self.p.sigma).abs() > 1e-4;
        self.p = next;
        if seed_changed {
            self.rng = Mulberry32::new(self.p.seed ^ 0x4D45_4144);
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
        } else if sigma_changed {
            self.refresh();
        }
    }

    fn make_sky_cloud(&mut self, x: f64) -> SkyCloud {
        let r_base = 95.0 * 1.2;
        let n = 3 + (self.rng.next() * 3.0).floor() as usize;
        let w = r_base * (1.0 + n as f64 * 0.5);
        let mut body: Vec<[f64; 3]> = Vec::new();
        for i in 0..n {
            let t = if n == 1 {
                0.0
            } else {
                i as f64 / (n - 1) as f64 - 0.5
            };
            let r = r_base * (0.6 - t.abs() * 0.45 + self.rng.next() * 0.12);
            body.push([t * w, -r * 0.55, r]);
        }
        for i in 0..(n - 1) {
            let t = if n == 2 {
                0.0
            } else {
                i as f64 / (n - 2) as f64 - 0.5
            };
            let r = r_base * (0.55 + self.rng.next() * 0.25);
            body.push([
                t * w * 0.6 + (self.rng.next() - 0.5) * r_base * 0.4,
                -r_base * (0.5 + self.rng.next() * 0.35),
                r,
            ]);
        }
        if self.rng.next() < 0.6 {
            body.push([
                (self.rng.next() - 0.5) * w * 0.3,
                -r_base * 1.05,
                r_base * (0.4 + self.rng.next() * 0.15),
            ]);
        }
        let ci_of = |px: f64, py: f64| -> f64 {
            let mut d = 1e9f64;
            for c in &body {
                d = d.min((px - c[0]).hypot(py - c[1]) - c[2]);
            }
            d
        };
        let (mut x0, mut x1, mut y0, mut y1) = (1e9f64, -1e9f64, 1e9f64, -1e9f64);
        for c in &body {
            x0 = x0.min(c[0] - c[2]);
            x1 = x1.max(c[0] + c[2]);
            y0 = y0.min(c[1] - c[2]);
            y1 = y1.max(c[1] + c[2]);
        }
        let mut circles: Vec<[f64; 4]> = Vec::new();
        let s_ratio = 0.62 / 1.2;
        let rm = r_base * s_ratio;
        let add_layer = |rng: &mut Mulberry32,
                         circles: &mut Vec<[f64; 4]>,
                         g: f64,
                         size: [f64; 2],
                         band: f64,
                         band_fn: &dyn Fn(f64) -> f64,
                         attach: bool| {
            let gy0 = ((y0 - g) / g).floor() as i32;
            let gy1 = ((y1 + g) / g).ceil() as i32;
            let gx0 = ((x0 - g) / g).floor() as i32;
            let gx1 = ((x1 + g) / g).ceil() as i32;
            for gy in gy0..=gy1 {
                for gx in gx0..=gx1 {
                    let px = (gx as f64 + 0.1 + rng.next() * 0.8) * g;
                    let py = (gy as f64 + 0.1 + rng.next() * 0.8) * g;
                    let ci = ci_of(px, py);
                    if ci > 0.0 || ci < band {
                        continue;
                    }
                    let mut r = g * (size[0] + (size[1] - size[0]) * rng.next()) * band_fn(ci);
                    if attach {
                        let mut att = 0.0f64;
                        for cc in circles.iter() {
                            att = att.max(sstep(
                                0.2 * r,
                                0.6 * r,
                                cc[2] + r - (cc[0] - px).hypot(cc[1] - py),
                            ));
                        }
                        r *= att;
                    }
                    r *= sstep(5.0, 11.0, r);
                    if r > 0.5 {
                        circles.push([px, py, r, rng.next()]);
                    }
                }
            }
        };
        let gb = 90.0;
        add_layer(
            &mut self.rng,
            &mut circles,
            gb,
            [0.55, 0.85],
            -1e9,
            &|ci| sstep(-0.2 * gb, -0.65 * gb, ci),
            false,
        );
        add_layer(
            &mut self.rng,
            &mut circles,
            rm,
            [0.6, 0.95],
            -1e9,
            &|ci| sstep(-0.1 * rm, -0.7 * rm, ci),
            false,
        );
        for c in &body {
            if c[2] > 0.75 * rm {
                circles.push([c[0], c[1], c[2] - 0.75 * rm, self.rng.next()]);
            }
        }
        add_layer(
            &mut self.rng,
            &mut circles,
            34.0 * 0.85,
            [0.5, 0.9],
            -1.0 * rm,
            &|ci| sstep(-0.05 * rm, -0.35 * rm, ci) * (1.0 - sstep(-0.7 * rm, -1.0 * rm, ci)),
            true,
        );
        add_layer(
            &mut self.rng,
            &mut circles,
            24.0 * 0.85,
            [0.45, 0.8],
            -0.6 * rm,
            &|ci| sstep(0.0, -0.2 * rm, ci) * (1.0 - sstep(-0.4 * rm, -0.6 * rm, ci)),
            true,
        );
        let y = 80.0 + self.rng.next() * (self.h - TASKBAR - 120.0).max(40.0);
        SkyCloud {
            x,
            y,
            circles,
            v: 0.7 + self.rng.next() * 0.6,
            grow: 0.0,
            hw: (-x0).max(x1) + 20.0,
            dying: false,
        }
    }

    pub fn build_mass_circles(&self) -> Vec<f64> {
        let mut list: Vec<f64> = Vec::new();
        if self.fb.is_empty() {
            return list;
        }
        let r_big = 95.0;
        let seed = self.p.seed;
        let mut big: HashMap<(i32, i32), [f64; 3]> = HashMap::new();

        let mut layer = |g: f64,
                         k0: i32,
                         size: [f64; 2],
                         band_min: f64,
                         band_fn: &dyn Fn(f64) -> f64,
                         is_big: bool| {
            let cy0 = -((2.0 * r_big) / g).ceil() as i32;
            let cy1 = ((self.h + 2.0 * r_big) / g).ceil() as i32;
            let cx0 = -((2.0 * r_big) / g).ceil() as i32;
            let cx1 = ((self.w + 2.0 * r_big) / g).ceil() as i32;
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    let ox = hash2(cx, cy, k0, seed);
                    let oy = hash2(cx, cy, k0 + 1, seed);
                    let rr = hash2(cx, cy, k0 + 2, seed);
                    let ph = hash2(cx, cy, k0 + 3, seed);
                    let x = (cx as f64 + 0.1 + ox * 0.8) * g;
                    let y = (cy as f64 + 0.1 + oy * 0.8) * g;
                    let ci = self.cur_s(x, y) - self.p.edge;
                    if ci > 0.0 || ci < band_min - 10.0 {
                        continue;
                    }
                    let mut r = g * (size[0] + (size[1] - size[0]) * rr) * band_fn(ci);
                    let gap = ci + 0.6 * r_big;
                    let mut att = sstep(gap - 4.0, gap + 12.0, r);
                    if !is_big {
                        let bx = (x / r_big).floor() as i32;
                        let by = (y / r_big).floor() as i32;
                        for j in (by - 1)..=(by + 1) {
                            for i in (bx - 1)..=(bx + 1) {
                                if let Some(o) = big.get(&(i, j)) {
                                    let ov = o[2] + r - (o[0] - x).hypot(o[1] - y);
                                    att = att.max(sstep(0.2 * r, 0.6 * r, ov));
                                }
                            }
                        }
                    }
                    r *= att;
                    r *= sstep(5.0, 11.0, r);
                    if r < 0.5 {
                        continue;
                    }
                    if is_big {
                        big.insert((cx, cy), [x, y, r]);
                    }
                    let breathed =
                        r * (1.0 + 0.04 * (self.flow_t * (0.09 + 0.035 * ph) + ph * 6.283).sin());
                    list.push(x);
                    list.push(y);
                    list.push(breathed);
                    list.push(ph);
                }
            }
        };

        layer(
            r_big,
            1,
            [0.55, 0.9],
            -1e9,
            &|ci| sstep(-0.15 * r_big, -0.8 * r_big, ci),
            true,
        );
        layer(
            34.0,
            11,
            [0.5, 0.9],
            -1.0 * r_big,
            &|ci| {
                sstep(-0.05 * r_big, -0.3 * r_big, ci)
                    * (1.0 - sstep(-0.7 * r_big, -1.0 * r_big, ci))
            },
            false,
        );
        layer(
            24.0,
            21,
            [0.45, 0.8],
            -0.6 * r_big,
            &|ci| {
                sstep(0.0, -0.15 * r_big, ci) * (1.0 - sstep(-0.4 * r_big, -0.6 * r_big, ci))
            },
            false,
        );
        list
    }

    pub fn step(&mut self, dt_f32: f32) {
        let dt = (dt_f32 as f64).clamp(0.001, 0.1);
        self.time += dt;
        self.flow_t += dt * self.p.flow;

        // Smooth morph between FA and FB
        if self.morph_elapsed < self.morph_dur {
            self.morph_elapsed = (self.morph_elapsed + dt).min(self.morph_dur);
        }
        let e = (self.morph_elapsed / self.morph_dur.max(0.001)).clamp(0.0, 1.0);
        self.cur_mix = if e < 0.5 {
            2.0 * e * e
        } else {
            1.0 - (-2.0 * e + 2.0).powi(2) * 0.5
        };

        // Sky clouds
        let want = (self.p.cover * 10.0).round() as usize;
        let mut alive_indices = Vec::new();
        for (i, c) in self.sky_clouds.iter().enumerate() {
            if !c.dying {
                alive_indices.push(i);
            }
        }
        for &idx in alive_indices.iter().skip(want) {
            self.sky_clouds[idx].dying = true;
        }
        let alive_count = alive_indices.len();
        for _ in alive_count..want {
            let mut best: Option<SkyCloud> = None;
            let mut best_d = -1.0f64;
            for _ in 0..20 {
                let rx = self.rng.next() * self.w;
                let cand = self.make_sky_cloud(rx);
                let mut dmin = 1e9f64;
                for o in &self.sky_clouds {
                    dmin = dmin.min(((o.x - cand.x) / 1.6).hypot(o.y - cand.y));
                }
                if dmin > best_d {
                    best_d = dmin;
                    best = Some(cand);
                }
            }
            if let Some(c) = best {
                self.sky_clouds.push(c);
            }
        }
        let mut i = self.sky_clouds.len();
        while i > 0 {
            i -= 1;
            let c = &mut self.sky_clouds[i];
            c.x += self.p.flow * 10.0 * c.v * dt;
            c.grow = if c.dying {
                c.grow - dt * 0.5
            } else {
                (c.grow + dt * 0.5).min(1.0)
            };
            if c.grow <= 0.0 {
                self.sky_clouds.remove(i);
                continue;
            }
            if c.x - c.hw > self.w + 50.0 {
                let hw = c.hw;
                let mut nc = self.make_sky_cloud(-hw - 50.0);
                nc.grow = 1.0;
                self.sky_clouds[i] = nc;
            }
        }

        // Kite boy & kitten simulation (boy route always updates so kitten avoids boy path)
        let boy_frame = self.boy.step(dt, self.w, self.h);
        if self.p.show_cat {
            self.cat.step(
                dt,
                self.w,
                self.h,
                self.p.seed,
                self.p.sigma,
                &self.icons,
                boy_frame.as_ref(),
            );
        }
    }
}

// =========================================================================
// Asset Resolution & OpenGL 3.3 Core Meadow Renderer
// =========================================================================
fn find_prototype_asset(filename: &str) -> Option<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            bases.push(dir.to_path_buf());
            if let Some(p1) = dir.parent() {
                bases.push(p1.to_path_buf());
                if let Some(p2) = p1.parent() {
                    bases.push(p2.to_path_buf());
                }
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd.clone());
        if let Some(p1) = cwd.parent() {
            bases.push(p1.to_path_buf());
            if let Some(p2) = p1.parent() {
                bases.push(p2.to_path_buf());
            }
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(p1) = manifest.parent() {
        if let Some(p2) = p1.parent() {
            bases.push(p2.to_path_buf());
        }
    }
    for base in bases {
        let p = base.join("prototype").join("assets").join(filename);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

unsafe fn load_sprite_texture(gl: &GlFns, filename: &str, size: u32) -> Option<GLuint> {
    let path = find_prototype_asset(filename)?;
    let img = image::open(&path).ok()?;
    let resized = img
        .resize_exact(size, size, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let mut tex = 0u32;
    (gl.GenTextures)(1, &mut tex);
    (gl.BindTexture)(GL_TEXTURE_2D, tex);
    (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
    (gl.TexImage2D)(
        GL_TEXTURE_2D,
        0,
        GL_RGBA8 as GLint,
        size as GLsizei,
        size as GLsizei,
        0,
        GL_RGBA,
        GL_UNSIGNED_BYTE,
        resized.as_raw().as_ptr() as *const c_void,
    );
    (gl.GenerateMipmap)(GL_TEXTURE_2D);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    Some(tex)
}

pub struct MeadowRenderer {
    dummy_vao: GLuint,
    prog_base: GLuint,
    prog_actor: GLuint,
    prog_cat: GLuint,
    prog_cat_actor: GLuint,
    prog_boy: GLuint,
    noise_tex: GLuint,
    ftex: [GLuint; 2],
    field_w: usize,
    field_h: usize,
    circ_tex: GLuint,
    circ_buf: Vec<f32>,
    cg_w: usize,
    cg_h: usize,
    // Boy & Cat sprite textures + Boy 2-layer FBO
    boy_char_tex: Option<GLuint>,
    boy_run_tex: Option<GLuint>,
    cat_tex: Option<GLuint>,
    actor_fbo: GLuint,
    actor_layers: [GLuint; 2], // 0: shadow, 1: color
    actor_w: i32,
    actor_h: i32,
}

impl MeadowRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let prog_base = gl.compile_program(MEADOW_VERT, &meadow_frag_source(false, false))?;
        let prog_actor = gl.compile_program(MEADOW_VERT, &meadow_frag_source(true, false))?;
        let prog_cat = gl.compile_program(MEADOW_VERT, &meadow_frag_source(false, true))?;
        let prog_cat_actor = gl.compile_program(MEADOW_VERT, &meadow_frag_source(true, true))?;
        let prog_boy = gl.compile_program(MEADOW_VERT, MEADOW_BOY_FRAG)?;

        let mut noise_tex = 0;
        (gl.GenTextures)(1, &mut noise_tex);
        let mut ftex = [0u32; 2];
        (gl.GenTextures)(2, ftex.as_mut_ptr());
        let mut circ_tex = 0;
        (gl.GenTextures)(1, &mut circ_tex);

        let mut actor_fbo = 0;
        (gl.GenFramebuffers)(1, &mut actor_fbo);
        let mut actor_layers = [0u32; 2];
        (gl.GenTextures)(2, actor_layers.as_mut_ptr());

        let boy_char_tex = load_sprite_texture(gl, "meadow-kite-boy-level-face.png", 384);
        let boy_run_tex = load_sprite_texture(gl, "meadow-kite-boy-diagonal-run.png", 768);
        let cat_tex = load_sprite_texture(gl, "meadow-black-kitten.png", 384);

        Ok(Self {
            dummy_vao,
            prog_base,
            prog_actor,
            prog_cat,
            prog_cat_actor,
            prog_boy,
            noise_tex,
            ftex,
            field_w: 0,
            field_h: 0,
            circ_tex,
            circ_buf: Vec::new(),
            cg_w: 0,
            cg_h: 0,
            boy_char_tex,
            boy_run_tex,
            cat_tex,
            actor_fbo,
            actor_layers,
            actor_w: 0,
            actor_h: 0,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if let Some(t) = self.boy_char_tex.take() {
            (gl.DeleteTextures)(1, &t);
        }
        if let Some(t) = self.boy_run_tex.take() {
            (gl.DeleteTextures)(1, &t);
        }
        if let Some(t) = self.cat_tex.take() {
            (gl.DeleteTextures)(1, &t);
        }
        (gl.DeleteTextures)(2, self.actor_layers.as_ptr());
        if self.actor_fbo != 0 {
            (gl.DeleteFramebuffers)(1, &self.actor_fbo);
            self.actor_fbo = 0;
        }
        if self.circ_tex != 0 {
            (gl.DeleteTextures)(1, &self.circ_tex);
            self.circ_tex = 0;
        }
        (gl.DeleteTextures)(2, self.ftex.as_ptr());
        if self.noise_tex != 0 {
            (gl.DeleteTextures)(1, &self.noise_tex);
            self.noise_tex = 0;
        }
        for p in [
            self.prog_base,
            self.prog_actor,
            self.prog_cat,
            self.prog_cat_actor,
            self.prog_boy,
        ] {
            if p != 0 {
                (gl.DeleteProgram)(p);
            }
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
    }

    unsafe fn sync_noise_tex(&mut self, gl: &GlFns, m: &mut MeadowState) {
        if !m.noise_dirty {
            return;
        }
        m.noise_dirty = false;
        let (s, data) = make_noise_image(m.p.seed);
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

    unsafe fn sync_field_textures(&mut self, gl: &GlFns, m: &mut MeadowState) {
        if self.field_w != m.gw || self.field_h != m.gh {
            self.field_w = m.gw;
            self.field_h = m.gh;
            for &tex in &self.ftex {
                (gl.BindTexture)(GL_TEXTURE_2D, tex);
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_R32F as GLint,
                    m.gw as GLsizei,
                    m.gh as GLsizei,
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
            m.field_dirty_a = true;
            m.field_dirty_b = true;
        }
        if m.field_dirty_a && m.fa.len() == m.gw * m.gh {
            m.field_dirty_a = false;
            (gl.ActiveTexture)(GL_TEXTURE1);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[0]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            (gl.TexSubImage2D)(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                m.gw as GLsizei,
                m.gh as GLsizei,
                GL_RED,
                GL_FLOAT,
                m.fa.as_ptr() as *const c_void,
            );
        }
        if m.field_dirty_b && m.fb.len() == m.gw * m.gh {
            m.field_dirty_b = false;
            (gl.ActiveTexture)(GL_TEXTURE2);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[1]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            (gl.TexSubImage2D)(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                m.gw as GLsizei,
                m.gh as GLsizei,
                GL_RED,
                GL_FLOAT,
                m.fb.as_ptr() as *const c_void,
            );
        }
    }

    unsafe fn upload_circles(&mut self, gl: &GlFns, m: &mut MeadowState) {
        let gw = (m.w / CELL_G as f64).ceil() as usize + 7;
        let gh = (m.h / CELL_G as f64).ceil() as usize + 7;
        if gw != self.cg_w || gh != self.cg_h {
            self.cg_w = gw;
            self.cg_h = gh;
            self.circ_buf.resize(gw * CIRC_K * gh * 4, 0.0);
            (gl.ActiveTexture)(GL_TEXTURE3);
            (gl.BindTexture)(GL_TEXTURE_2D, self.circ_tex);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA32F as GLint,
                (gw * CIRC_K) as GLsizei,
                gh as GLsizei,
                0,
                GL_RGBA,
                GL_FLOAT,
                std::ptr::null(),
            );
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        }

        let morphing = m.cur_mix < 1.0
            || m.mass_list.is_empty()
            || (m.mass_w - m.w).abs() > 0.5
            || (m.mass_h - m.h).abs() > 0.5;
        if morphing || (m.time - m.mass_t) > 0.5 {
            m.mass_list = m.build_mass_circles();
            m.mass_w = m.w;
            m.mass_h = m.h;
            m.mass_t = m.time;
        }

        let mut list: Vec<[f64; 4]> = Vec::with_capacity(m.mass_list.len() / 4 + 256);
        let flow_t = m.flow_t;
        for chunk in m.mass_list.chunks_exact(4) {
            let (x, y, r, ph) = (chunk[0], chunk[1], chunk[2], chunk[3]);
            list.push([
                x + wob_x(x, y, r, ph, flow_t),
                y + wob_y(x, y, r, ph, flow_t),
                r,
                ph,
            ]);
        }
        for c in &m.sky_clouds {
            let g = c.grow * c.grow * (3.0 - 2.0 * c.grow);
            for (i, &[dx, dy, r, ph]) in c.circles.iter().enumerate() {
                let rb = r
                    * g
                    * (1.0
                        + 0.04
                            * (flow_t * (0.09 + 0.035 * ph) + ph * 6.283 + i as f64).sin());
                if rb > 0.5 {
                    list.push([
                        c.x + dx + wob_x(dx, dy, r, ph, flow_t),
                        c.y + dy + wob_y(dx, dy, r, ph, flow_t),
                        rb,
                        ph,
                    ]);
                }
            }
        }

        list.sort_by(|a, b| b[2].total_cmp(&a[2]));
        self.circ_buf.fill(0.0);
        let mut cnt = vec![0u8; gw * gh];
        let cg = CELL_G as f64;
        for c in &list {
            let (x, y, r_orig, ph) = (c[0], c[1], c[2], c[3]);
            let r = r_orig + 6.0;
            let x0 = (((x - r) / cg).floor() as isize + 4).clamp(0, gw as isize - 1) as usize;
            let x1 = (((x + r) / cg).floor() as isize + 4).clamp(0, gw as isize - 1) as usize;
            let y0 = (((y - r) / cg).floor() as isize + 4).clamp(0, gh as isize - 1) as usize;
            let y1 = (((y + r) / cg).floor() as isize + 4).clamp(0, gh as isize - 1) as usize;
            for gy in y0..=y1 {
                for gx in x0..=x1 {
                    let ci = gy * gw + gx;
                    if cnt[ci] as usize >= CIRC_K {
                        continue;
                    }
                    let nx = x.clamp((gx as isize - 4) as f64 * cg, (gx as isize - 3) as f64 * cg);
                    let ny = y.clamp((gy as isize - 4) as f64 * cg, (gy as isize - 3) as f64 * cg);
                    if (nx - x) * (nx - x) + (ny - y) * (ny - y) > r * r {
                        continue;
                    }
                    let slot = cnt[ci] as usize;
                    cnt[ci] += 1;
                    let o = ((gy * gw + gx) * CIRC_K + slot) * 4;
                    self.circ_buf[o] = x as f32;
                    self.circ_buf[o + 1] = y as f32;
                    self.circ_buf[o + 2] = r_orig as f32;
                    self.circ_buf[o + 3] = ph as f32;
                }
            }
        }

        (gl.ActiveTexture)(GL_TEXTURE3);
        (gl.BindTexture)(GL_TEXTURE_2D, self.circ_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexSubImage2D)(
            GL_TEXTURE_2D,
            0,
            0,
            0,
            (gw * CIRC_K) as GLsizei,
            gh as GLsizei,
            GL_RGBA,
            GL_FLOAT,
            self.circ_buf.as_ptr() as *const c_void,
        );
    }

    unsafe fn render_boy_layers(
        &mut self,
        gl: &GlFns,
        bf: &BoyFrameData,
        k: f64,
    ) -> Option<[f32; 4]> {
        let (char_tex, run_tex) = (self.boy_char_tex?, self.boy_run_tex?);
        let s = bf.scale;
        let left = ((bf.position[0] - 96.0 * s) * k).floor() as i32 - 1;
        let top = ((bf.position[1] - 116.0 * s) * k).floor() as i32 - 1;
        let width = ((708.0 * s * k).ceil() as i32 + 2).max(16);
        let height = ((726.0 * s * k).ceil() as i32 + 2).max(16);

        if width != self.actor_w || height != self.actor_h {
            self.actor_w = width;
            self.actor_h = height;
            (gl.ActiveTexture)(GL_TEXTURE6);
            for &layer in &self.actor_layers {
                (gl.BindTexture)(GL_TEXTURE_2D, layer);
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_RGBA8 as GLint,
                    width,
                    height,
                    0,
                    GL_RGBA,
                    GL_UNSIGNED_BYTE,
                    std::ptr::null(),
                );
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            }
        }

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.actor_fbo);
        (gl.Viewport)(0, 0, width, height);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.UseProgram)(self.prog_boy);

        (gl.ActiveTexture)(GL_TEXTURE4);
        (gl.BindTexture)(GL_TEXTURE_2D, char_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_boy, "uCharacter"), 4);

        (gl.ActiveTexture)(GL_TEXTURE7);
        (gl.BindTexture)(GL_TEXTURE_2D, run_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_boy, "uRunSprite"), 7);

        let kf = k as f32;
        (gl.Uniform2f)(gl.uniform_loc(self.prog_boy, "uRes"), width as f32 / kf, height as f32 / kf);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_boy, "uOffset"), left as f32 / kf, top as f32 / kf);
        (gl.Uniform3f)(
            gl.uniform_loc(self.prog_boy, "uPlace"),
            bf.position[0] as f32,
            bf.position[1] as f32,
            s as f32,
        );
        (gl.Uniform2f)(gl.uniform_loc(self.prog_boy, "uKite"), bf.kx as f32, bf.ky as f32);
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_boy, "uPose"),
            bf.pose.head as f32,
            bf.pose.nod as f32,
            bf.pose.reel as f32,
            bf.pose.pull as f32,
        );
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_boy, "uMotion"),
            bf.activity.frame as f32,
            if bf.activity.back { 1.0 } else { 0.0 },
            bf.activity.squeeze as f32,
            bf.activity.bob as f32,
        );
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_boy, "uRig"),
            if bf.activity.running { 1.0 } else { 0.0 },
            bf.rig.arm as f32,
            bf.rig.reel_arm as f32,
            0.0,
        );
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_boy, "uLeftHipKnee"),
            bf.rig.left.hip[0] as f32,
            bf.rig.left.hip[1] as f32,
            bf.rig.left.knee[0] as f32,
            bf.rig.left.knee[1] as f32,
        );
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_boy, "uLeftAnkle"),
            bf.rig.left.ankle[0] as f32,
            bf.rig.left.ankle[1] as f32,
        );
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_boy, "uRightHipKnee"),
            bf.rig.right.hip[0] as f32,
            bf.rig.right.hip[1] as f32,
            bf.rig.right.knee[0] as f32,
            bf.rig.right.knee[1] as f32,
        );
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_boy, "uRightAnkle"),
            bf.rig.right.ankle[0] as f32,
            bf.rig.right.ankle[1] as f32,
        );
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_boy, "uFootAngles"),
            bf.rig.left.angle as f32,
            bf.rig.right.angle as f32,
        );
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_boy, "uGround"),
            bf.ground.line[0] as f32,
            bf.ground.line[1] as f32,
        );
        (gl.Uniform3f)(
            gl.uniform_loc(self.prog_boy, "uContactL"),
            bf.ground.contacts[0][0] as f32,
            bf.ground.contacts[0][1] as f32,
            bf.ground.contacts[0][2] as f32,
        );
        (gl.Uniform3f)(
            gl.uniform_loc(self.prog_boy, "uContactR"),
            bf.ground.contacts[1][0] as f32,
            bf.ground.contacts[1][1] as f32,
            bf.ground.contacts[1][2] as f32,
        );
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_boy, "uBody"),
            bf.pose.body[0] as f32,
            bf.pose.body[1] as f32,
        );
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_boy, "uLegs"),
            bf.pose.legs[0] as f32,
            bf.pose.legs[1] as f32,
        );
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_boy, "uHand"),
            bf.pose.hand[0] as f32,
            bf.pose.hand[1] as f32,
        );
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_boy, "uTug"),
            bf.pose.tug_offset[0] as f32,
            bf.pose.tug_offset[1] as f32,
        );
        (gl.Uniform1f)(gl.uniform_loc(self.prog_boy, "uTension"), bf.line_pull as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_boy, "uKiteRock"), bf.kite_rock as f32);
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_boy, "uReelHand"),
            bf.pose.reel_hand[0] as f32,
            bf.pose.reel_hand[1] as f32,
        );
        (gl.Uniform1f)(gl.uniform_loc(self.prog_boy, "uLook"), bf.pose.look as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_boy, "uScale"), kf);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_boy, "uTime"), bf.time as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_boy, "uRunWind"), bf.run_wind as f32);

        let regions_shadow: &[[f64; 4]] = &[[-70.0, 25.0, 185.0, 230.0], [105.0, 140.0, 612.0, 610.0]];
        let regions_actor: &[[f64; 4]] = &[
            [-90.0, -110.0, 116.0, 90.0],
            [12.0, -35.0, 205.0, 260.0],
            [12.0, 65.0, 514.0, 476.0],
        ];
        let regions_all = [regions_shadow, regions_actor];
        (gl.BindVertexArray)(self.dummy_vao);
        for (i, regs) in regions_all.iter().enumerate() {
            (gl.FramebufferTexture2D)(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                self.actor_layers[i],
                0,
            );
            (gl.Disable)(GL_SCISSOR_TEST);
            (gl.ClearColor)(0.0, 0.0, 0.0, 0.0);
            (gl.Clear)(GL_COLOR_BUFFER_BIT);
            (gl.Uniform1f)(
                gl.uniform_loc(self.prog_boy, "uShadow"),
                if i == 0 { 1.0 } else { 0.0 },
            );
            (gl.Enable)(GL_SCISSOR_TEST);
            for &[x0, y0, x1, y1] in *regs {
                let l = (((bf.position[0] + x0 * s) * k).floor() as i32 - left - 2).max(0);
                let r = (((bf.position[0] + x1 * s) * k).ceil() as i32 - left + 2).min(width);
                let t = (((bf.position[1] + y0 * s) * k).floor() as i32 - top - 2).max(0);
                let b = (((bf.position[1] + y1 * s) * k).ceil() as i32 - top + 2).min(height);
                (gl.Scissor)(l, height - b, (r - l).max(0), (b - t).max(0));
                (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
            }
        }
        (gl.Disable)(GL_SCISSOR_TEST);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);

        Some([
            left as f32 / kf,
            top as f32 / kf,
            width as f32 / kf,
            height as f32 / kf,
        ])
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.5);

        st.meadow.resize(css_w, css_h);
        self.sync_noise_tex(gl, &mut st.meadow);
        self.sync_field_textures(gl, &mut st.meadow);
        self.upload_circles(gl, &mut st.meadow);

        let actor_rect = if st.meadow.p.show_boy {
            if let Some(bf) = st.meadow.boy.last_frame.clone() {
                self.render_boy_layers(gl, &bf, dpi as f64)
            } else {
                None
            }
        } else {
            None
        };

        let cat_draw = if st.meadow.p.show_cat {
            self.cat_tex.and_then(|tex| {
                st.meadow.cat.place.map(|pl| {
                    let sz = pl.size;
                    let rect = [
                        (pl.x - sz * 0.5) as f32,
                        (pl.y - sz * 0.5) as f32,
                        sz as f32,
                        sz as f32,
                    ];
                    let sh = [(sz * 0.045) as f32, (sz * 0.06) as f32];
                    let bounds = [
                        rect[0] - 2.0,
                        rect[1] - 2.0,
                        rect[2] + sh[0] + 4.0,
                        rect[3] + sh[1] + 4.0,
                    ];
                    let tail = meadow_cat_tail_angle(st.meadow.cat.time, st.meadow.p.seed) as f32;
                    (tex, rect, sh, tail, bounds)
                })
            })
        } else {
            None
        };

        let m = &st.meadow;
        let seed_u = ((m.p.seed.rem_euclid(997)) as f32) * 0.37;
        let bind_and_draw = |prog: GLuint, with_cat: bool| {
            (gl.UseProgram)(prog);
            (gl.Viewport)(0, 0, pw, ph);
            (gl.ActiveTexture)(GL_TEXTURE0);
            (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
            (gl.Uniform1i)(gl.uniform_loc(prog, "uNoise"), 0);

            (gl.ActiveTexture)(GL_TEXTURE1);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[0]);
            (gl.Uniform1i)(gl.uniform_loc(prog, "uFA"), 1);

            (gl.ActiveTexture)(GL_TEXTURE2);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[1]);
            (gl.Uniform1i)(gl.uniform_loc(prog, "uFB"), 2);

            (gl.ActiveTexture)(GL_TEXTURE3);
            (gl.BindTexture)(GL_TEXTURE_2D, self.circ_tex);
            (gl.Uniform1i)(gl.uniform_loc(prog, "uCirc"), 3);

            (gl.Uniform2f)(gl.uniform_loc(prog, "uRes"), pw as f32 / dpi, ph as f32 / dpi);
            (gl.Uniform2f)(gl.uniform_loc(prog, "uGrid"), m.gw as f32, m.gh as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uScale"), dpi);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uSeed"), seed_u);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uStep"), STEP as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uMix"), m.cur_mix as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uTime"), m.time as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uDebug"), if m.p.debug { 1.0 } else { 0.0 });
            (gl.Uniform2f)(gl.uniform_loc(prog, "uCGrid"), self.cg_w as f32, self.cg_h as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uCellG"), CELL_G as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uEdge"), m.p.edge as f32);

            if let Some(ar) = actor_rect {
                (gl.ActiveTexture)(GL_TEXTURE5);
                (gl.BindTexture)(GL_TEXTURE_2D, self.actor_layers[1]);
                (gl.Uniform1i)(gl.uniform_loc(prog, "uActor"), 5);

                (gl.ActiveTexture)(GL_TEXTURE6);
                (gl.BindTexture)(GL_TEXTURE_2D, self.actor_layers[0]);
                (gl.Uniform1i)(gl.uniform_loc(prog, "uActorShadow"), 6);

                (gl.Uniform4f)(gl.uniform_loc(prog, "uActorRect"), ar[0], ar[1], ar[2], ar[3]);
            }
            if with_cat {
                if let Some((ctex, crect, csh, ctail, _)) = cat_draw {
                    (gl.ActiveTexture)(GL_TEXTURE8);
                    (gl.BindTexture)(GL_TEXTURE_2D, ctex);
                    (gl.Uniform1i)(gl.uniform_loc(prog, "uCat"), 8);
                    (gl.Uniform4f)(gl.uniform_loc(prog, "uCatRect"), crect[0], crect[1], crect[2], crect[3]);
                    (gl.Uniform2f)(gl.uniform_loc(prog, "uCatShadow"), csh[0], csh[1]);
                    (gl.Uniform1f)(gl.uniform_loc(prog, "uCatTail"), ctail);
                }
            }
            (gl.BindVertexArray)(self.dummy_vao);
            (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
        };

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_SCISSOR_TEST);
        bind_and_draw(
            if actor_rect.is_some() {
                self.prog_actor
            } else {
                self.prog_base
            },
            false,
        );

        if let Some((_, _, _, _, bounds)) = cat_draw {
            let left = ((bounds[0] * dpi).floor() as i32).max(0);
            let top = ((bounds[1] * dpi).floor() as i32).max(0);
            let right = (((bounds[0] + bounds[2]) * dpi).ceil() as i32).min(pw);
            let bottom = (((bounds[1] + bounds[3]) * dpi).ceil() as i32).min(ph);
            (gl.Enable)(GL_SCISSOR_TEST);
            (gl.Scissor)(left, ph - bottom, (right - left).max(0), (bottom - top).max(0));
            bind_and_draw(
                if actor_rect.is_some() {
                    self.prog_cat_actor
                } else {
                    self.prog_cat
                },
                true,
            );
            (gl.Disable)(GL_SCISSOR_TEST);
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
    fn verify_rust_anything_recorded_meadow_cases() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root");
        let rec_path = manifest_dir.join("src/meadow/recorded-from.json");
        let rec: Value =
            serde_json::from_str(&std::fs::read_to_string(&rec_path).expect("read recorded-from"))
                .expect("parse recorded-from");
        for src in rec["sources"].as_array().expect("sources array") {
            let rel = src["path"].as_str().unwrap();
            let exp = src["fnv1a"].as_str().unwrap();
            let bytes = std::fs::read(repo_root.join(rel)).expect("read source");
            assert_eq!(
                fnv1a_fingerprint(rel, &bytes),
                exp,
                "Source fingerprint drift in {rel}"
            );
        }

        let cases_raw =
            std::fs::read_to_string(manifest_dir.join("src/meadow/cases.jsonl")).expect("cases");
        let mut checked = 0usize;
        for line in cases_raw.lines().filter(|l| !l.trim().is_empty()) {
            let c: Value = serde_json::from_str(line).expect("parse case");
            let fname = c["fn"].as_str().unwrap();
            let inp = &c["input"];
            let out = &c["output"];

            match fname {
                "noiseAndTileImage" => {
                    let seed = inp["seed"].as_i64().unwrap() as i32;
                    let h_in = [(0, 0, 0), (13, -7, 5), (1024, 512, 100), (-99, 42, 250)];
                    for (i, &(x, y, k)) in h_in.iter().enumerate() {
                        assert_near(
                            hash2(x, y, k, seed),
                            out["hashes"][i].as_f64().unwrap(),
                            1e-12,
                            "hash2",
                        );
                    }
                    let v_in = [(1.25, 2.75, 1), (-3.4, 5.1, 2)];
                    for (i, &(x, y, k)) in v_in.iter().enumerate() {
                        assert_near(
                            vnoise(x, y, k, seed),
                            out["vnoises"][i].as_f64().unwrap(),
                            1e-12,
                            "vnoise",
                        );
                    }
                    let f_in = [(0.35, 0.72, 1), (1.82, 0.91, 2)];
                    for (i, &(x, y, k)) in f_in.iter().enumerate() {
                        assert_near(
                            fbm(x, y, k, seed),
                            out["fbms"][i].as_f64().unwrap(),
                            1e-12,
                            "fbm",
                        );
                    }
                    let (s, data) = make_noise_image(seed);
                    let coords = [(0usize, 0usize), (17, 29), (128, 64), (255, 255)];
                    for (i, &(x, y)) in coords.iter().enumerate() {
                        let off = (y * s + x) * 4;
                        let exp_px = out["imgSamples"][i].as_array().unwrap();
                        for ch in 0..4 {
                            assert_eq!(
                                data[off + ch] as i64,
                                exp_px[ch].as_i64().unwrap(),
                                "noiseImg[{i}][{ch}]"
                            );
                        }
                    }
                }
                "computeField" => {
                    let mut st = MeadowState::default();
                    st.p.seed = inp["seed"].as_i64().unwrap() as i32;
                    st.p.sigma = inp["sigma"].as_f64().unwrap();
                    st.p.warp = inp["warp"].as_f64().unwrap();
                    st.p.edge = inp["edge"].as_f64().unwrap();
                    st.resize(
                        inp["W"].as_f64().unwrap() as f32,
                        inp["H"].as_f64().unwrap() as f32,
                    );
                    let rects: Vec<IconBox> = inp["rects"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| IconBox {
                            cx: r["cx"].as_f64().unwrap(),
                            cy: r["cy"].as_f64().unwrap(),
                            hw: r["hw"].as_f64().unwrap(),
                            hh: r["hh"].as_f64().unwrap(),
                        })
                        .collect();
                    let field = st.compute_field_raw(&rects);
                    for (i, p) in inp["probes"].as_array().unwrap().iter().enumerate() {
                        let px = p[0].as_f64().unwrap();
                        let py = p[1].as_f64().unwrap();
                        let got = MeadowState::sample_arr(&field, st.gw, st.gh, px, py);
                        assert_near(got, out["sampled"][i].as_f64().unwrap(), 1e-4, "computeField");
                    }
                }
                "massCirclesAndWobble" => {
                    let mut st = MeadowState::default();
                    st.p.seed = inp["seed"].as_i64().unwrap() as i32;
                    st.flow_t = inp["flowT"].as_f64().unwrap();
                    st.resize(
                        inp["W"].as_f64().unwrap() as f32,
                        inp["H"].as_f64().unwrap() as f32,
                    );
                    let rects: Vec<IconBox> = inp["rects"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| IconBox {
                            cx: r["cx"].as_f64().unwrap(),
                            cy: r["cy"].as_f64().unwrap(),
                            hw: r["hw"].as_f64().unwrap(),
                            hh: r["hh"].as_f64().unwrap(),
                        })
                        .collect();
                    let field = st.compute_field_raw(&rects);
                    st.fa = field.clone();
                    st.fb = field;
                    st.cur_mix = 1.0;
                    let list = st.build_mass_circles();
                    assert_eq!(list.len() / 4, out["count"].as_u64().unwrap() as usize);
                    for (i, row) in out["first3"].as_array().unwrap().iter().enumerate() {
                        let (x, y, r, ph) =
                            (list[i * 4], list[i * 4 + 1], list[i * 4 + 2], list[i * 4 + 3]);
                        assert_near(x, row[0].as_f64().unwrap(), 1e-5, "circ.x");
                        assert_near(y, row[1].as_f64().unwrap(), 1e-5, "circ.y");
                        assert_near(r, row[2].as_f64().unwrap(), 1e-4, "circ.r");
                        assert_near(ph, row[3].as_f64().unwrap(), 1e-9, "circ.ph");
                        assert_near(wob_x(x, y, r, ph, st.flow_t), row[4].as_f64().unwrap(), 1e-4, "wobX");
                        assert_near(wob_y(x, y, r, ph, st.flow_t), row[5].as_f64().unwrap(), 1e-4, "wobY");
                    }
                }
                "meadowBoyKinematics" => {
                    let cases_t = [
                        (0.08, 4.0, 2.0, false, false),
                        (0.65, 4.0, 2.0, false, false),
                        (1.50, 4.0, 2.0, false, false),
                        (2.30, 4.0, 2.0, false, false),
                        (2.50, 4.0, 2.0, false, false),
                        (0.45, 3.0, 2.2, true, false),
                        (2.30, 3.0, 2.2, false, true),
                    ];
                    let mut poses = Vec::new();
                    for (i, &(el, cy, dur, en, ex)) in cases_t.iter().enumerate() {
                        let got = meadow_boy_trip_pose(el, cy, dur, en, ex);
                        let exp = &out["tripSamples"][i];
                        assert_eq!(got.frame as i64, exp["frame"].as_i64().unwrap());
                        assert_eq!(got.back, exp["back"].as_bool().unwrap());
                        assert_eq!(got.running, exp["running"].as_bool().unwrap());
                        assert_eq!(got.done, exp["done"].as_bool().unwrap());
                        assert_near(got.progress, exp["progress"].as_f64().unwrap(), 1e-9, "progress");
                        assert_near(got.squeeze, exp["squeeze"].as_f64().unwrap(), 1e-9, "squeeze");
                        assert_near(got.bob, exp["bob"].as_f64().unwrap(), 1e-9, "bob");
                        poses.push(got);
                    }
                    let rig = meadow_boy_running_rig(&poses[1]);
                    assert_near(rig.left.knee[0], out["rig"]["left"]["knee"][0].as_f64().unwrap(), 1e-9, "lk.x");
                    assert_near(rig.left.ankle[1], out["rig"]["left"]["ankle"][1].as_f64().unwrap(), 1e-9, "la.y");
                    assert_near(rig.right.angle, out["rig"]["right"]["angle"].as_f64().unwrap(), 1e-9, "ra");
                    assert_near(rig.reel_arm, out["rig"]["reelArm"].as_f64().unwrap(), 1e-9, "reelArm");

                    let tug = meadow_boy_tug_pose(0.28);
                    assert_near(tug.reach, out["tug"]["reach"].as_f64().unwrap(), 1e-9, "tug.reach");
                    assert_near(tug.pull, out["tug"]["pull"].as_f64().unwrap(), 1e-9, "tug.pull");

                    let pose = meadow_boy_pose(4.35, &tug);
                    assert_near(pose.hand[0], out["pose"]["hand"][0].as_f64().unwrap(), 1e-9, "hand.x");
                    assert_near(pose.hand[1], out["pose"]["hand"][1].as_f64().unwrap(), 1e-9, "hand.y");

                    let gr = meadow_boy_ground_pose(&poses[1], &rig, &pose);
                    assert_near(gr.line[0], out["groundRun"]["line"][0].as_f64().unwrap(), 1e-9, "gr.slope");
                    assert_near(gr.line[1], out["groundRun"]["line"][1].as_f64().unwrap(), 1e-9, "gr.int");

                    let dn = meadow_boy_run_destination([640.0, 500.0], 0.32, 1280.0, 720.0, 48.0);
                    assert_near(dn.x, out["destNormal"]["x"].as_f64().unwrap(), 1e-9, "dn.x");
                    assert_eq!(dn.exit, out["destNormal"]["exit"].as_bool().unwrap());

                    let de = meadow_boy_run_destination([300.0, 120.0], 0.32, 1280.0, 720.0, 48.0);
                    assert_near(de.y, out["destExit"]["y"].as_f64().unwrap(), 1e-9, "de.y");
                    assert_eq!(de.exit, out["destExit"]["exit"].as_bool().unwrap());

                    let mut kr = KiteReaction::default();
                    for i in 0..10 {
                        meadow_boy_kite_reaction(&mut kr, 0.033, if i == 0 { 0.65 } else { 0.0 });
                    }
                    assert_near(kr.x, out["reaction"]["x"].as_f64().unwrap(), 1e-9, "kr.x");
                    assert_near(kr.angle, out["reaction"]["angle"].as_f64().unwrap(), 1e-9, "kr.angle");
                }
                "meadowCatPlacementAndTail" => {
                    let bpos = [
                        inp["state"]["position"]["x"].as_f64().unwrap(),
                        inp["state"]["position"]["y"].as_f64().unwrap(),
                    ];
                    let btgt = Some([
                        inp["state"]["routeTarget"]["x"].as_f64().unwrap(),
                        inp["state"]["routeTarget"]["y"].as_f64().unwrap(),
                    ]);
                    let icons: Vec<Rect> = inp["icons"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| Rect {
                            x: r["x"].as_f64().unwrap() as f32,
                            y: r["y"].as_f64().unwrap() as f32,
                            w: r["w"].as_f64().unwrap() as f32,
                            h: r["h"].as_f64().unwrap() as f32,
                        })
                        .collect();
                    let p42 = meadow_cat_choose_place(bpos, btgt, 1280.0, 720.0, 48.0, 42, &icons, 72.0)
                        .expect("cat place 42");
                    assert_near(p42.x, out["place42"]["x"].as_f64().unwrap(), 1e-9, "cat42.x");
                    assert_near(p42.y, out["place42"]["y"].as_f64().unwrap(), 1e-9, "cat42.y");
                    assert_near(p42.size, out["place42"]["size"].as_f64().unwrap(), 1e-9, "cat42.size");
                    assert_eq!(p42.lane_count, out["place42"]["laneCount"].as_u64().unwrap() as usize);

                    let p999 = meadow_cat_choose_place(bpos, btgt, 1280.0, 720.0, 48.0, 999, &icons, 90.0)
                        .expect("cat place 999");
                    assert_near(p999.x, out["place999"]["x"].as_f64().unwrap(), 1e-9, "cat999.x");
                    assert_near(p999.y, out["place999"]["y"].as_f64().unwrap(), 1e-9, "cat999.y");

                    let ts = [0.0, 2.5, 3.1, 3.4, 3.8, 4.2, 4.7, 13.5];
                    for (i, &t) in ts.iter().enumerate() {
                        assert_near(
                            meadow_cat_tail_angle(t, 42),
                            out["tailAngles"][i].as_f64().unwrap(),
                            1e-9,
                            "tailAngle",
                        );
                    }
                }
                other => panic!("Unexpected case fn: {other}"),
            }
            checked += 1;
        }
        assert_eq!(checked, 5);
    }
}
