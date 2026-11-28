//! 03 · Shark Intercept — 1:1 faithful interactive Rust port of
//! `sapozhkov/ascii-lab-public/03-shark-intercept/source/proto/shark_studies/{dive_pose,cutoff_motion,cutoff_render}.py`
//! + `tail_reaction.py`.

use super::grid_gl::{glyph_id, ramp_glyph, AsciiCanvas};
use super::AsciiLabParams;
use crate::state::Rect;
use std::f32::consts::{PI, TAU};

const FPS: f32 = 24.0;
const DT: f32 = 1.0 / FPS;
const STEP: f32 = 0.84 * DT;
const LOOP_SECONDS: f32 = 20.0;
const SUB_SAMPLES: usize = 6;

const SHARK_RAMP: &[u8] = b" .:;i!l|/txcoahkO8B%#@";
const BG_RAMP: &[u8] = b"  ..,,::;==++xsoO8@";
const FISH_RAMP: &[u8] = b"-~=+xoO8%#@";

const AXIAL_START: f32 = 0.505;
const AXIAL_END: f32 = -0.505;
const JOINTS_N: usize = 27;
const SEGMENT: f32 = (AXIAL_START - AXIAL_END) / (JOINTS_N as f32 - 1.0);
const SEG_COUNT: usize = JOINTS_N - 1;

const STATIONS: [f32; 14] = [
    -0.292, -0.260, -0.205, -0.125, -0.035, 0.060, 0.155, 0.245, 0.330, 0.395, 0.448, 0.480, 0.498, 0.505,
];
const HALF_WIDTH: [f32; 14] = [
    0.000, 0.014, 0.023, 0.036, 0.050, 0.067, 0.077, 0.078, 0.068, 0.057, 0.044, 0.032, 0.016, 0.000,
];
const BACK_H: [f32; 14] = [
    0.005, 0.023, 0.028, 0.040, 0.054, 0.067, 0.075, 0.073, 0.061, 0.048, 0.034, 0.023, 0.012, 0.000,
];
const BELLY_H: [f32; 14] = [
    0.000, 0.018, 0.022, 0.031, 0.046, 0.060, 0.068, 0.069, 0.059, 0.046, 0.030, 0.016, 0.008, 0.000,
];

const DORSAL: [[f32; 3]; 9] = [
    [0.186, 0.0, 0.063], [0.154, 0.0, 0.091], [0.105, 0.0, 0.148],
    [0.082, 0.0, 0.165], [0.076, 0.0, 0.154], [0.077, 0.0, 0.106],
    [0.065, 0.0, 0.082], [0.035, 0.0, 0.061], [0.019, 0.0, 0.056],
];
const SECOND: [[f32; 3]; 5] = [
    [-0.150, 0.0, 0.028], [-0.180, 0.0, 0.063], [-0.189, 0.0, 0.065],
    [-0.189, 0.0, 0.041], [-0.209, 0.0, 0.026],
];
const ANAL: [[f32; 3]; 5] = [
    [-0.153, 0.0, -0.024], [-0.187, 0.0, -0.064], [-0.194, 0.0, -0.057],
    [-0.188, 0.0, -0.037], [-0.207, 0.0, -0.027],
];
const UPPER: [[f32; 3]; 11] = [
    [-0.248, 0.0, 0.016], [-0.299, 0.0, 0.036], [-0.396, 0.0, 0.093],
    [-0.496, 0.0, 0.126], [-0.502, 0.0, 0.122], [-0.478, 0.0, 0.099],
    [-0.454, 0.0, 0.082], [-0.447, 0.0, 0.084], [-0.404, 0.0, 0.050],
    [-0.335, 0.0, 0.007], [-0.295, 0.0, -0.012],
];
const LOWER: [[f32; 3]; 7] = [
    [-0.252, 0.0, -0.010], [-0.304, 0.0, -0.024], [-0.347, 0.0, -0.101],
    [-0.340, 0.0, -0.114], [-0.323, 0.0, -0.100], [-0.296, 0.0, -0.065],
    [-0.276, 0.0, -0.029],
];
const PECTORAL: [[f32; 3]; 9] = [
    [0.281, 0.047, -0.008], [0.267, 0.086, -0.020], [0.218, 0.145, -0.050],
    [0.157, 0.205, -0.089], [0.139, 0.219, -0.102], [0.140, 0.200, -0.096],
    [0.169, 0.125, -0.063], [0.170, 0.080, -0.037], [0.150, 0.056, -0.014],
];
const PELVIC: [[f32; 3]; 5] = [
    [-0.060, 0.032, -0.021], [-0.087, 0.063, -0.049], [-0.124, 0.087, -0.063],
    [-0.117, 0.055, -0.038], [-0.139, 0.027, -0.027],
];

#[inline]
fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let a = ((x - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0);
    a * a * (3.0 - 2.0 * a)
}

#[inline]
fn norm3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[inline]
fn unit3(v: [f32; 3]) -> [f32; 3] {
    let l = norm3(v).max(1e-8);
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

#[inline]
fn interp_tbl(xs: &[f32], ys: &[f32], x: f32) -> f32 {
    if x <= xs[0] {
        return ys[0];
    }
    let n = xs.len();
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    for i in 0..(n - 1) {
        if x <= xs[i + 1] {
            let f = (x - xs[i]) / (xs[i + 1] - xs[i]).max(1e-6);
            return ys[i] + f * (ys[i + 1] - ys[i]);
        }
    }
    ys[n - 1]
}

/// Exact `tail_reaction.py` / `dive_pose.py` axial tail reaction per unit mass.
#[inline]
pub fn caudal_reaction(sweep: f32, slope: f32, advance: f32) -> f32 {
    let inflow = advance * slope;
    0.75 * (sweep * sweep - inflow * inflow)
}

#[inline]
fn water_acceleration(vel: [f32; 3], fwd: [f32; 3], thrust: f32, speed: f32) -> [f32; 3] {
    let vf = dot3(vel, fwd);
    let lat = [vel[0] - fwd[0] * vf, vel[1] - fwd[1] * vf, vel[2] - fwd[2] * vf];
    [
        thrust * fwd[0] - 2.45 * speed * vel[0] - 2.9 * lat[0],
        thrust * fwd[1] - 2.45 * speed * vel[1] - 2.9 * lat[1],
        thrust * fwd[2] - 2.45 * speed * vel[2] - 2.9 * lat[2],
    ]
}

struct Rng64(u64);
impl Rng64 {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }
    fn f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (((self.0 >> 11) as f64) / ((1u64 << 53) as f64)) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
    fn normal(&mut self, sd: f32) -> f32 {
        let u1 = self.f32().max(1e-7);
        let u2 = self.f32();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos() * sd
    }
}

#[derive(Clone)]
struct SwimPose {
    center: [f32; 3],
    forward: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    phase: f32,
    speed: f32,
    spine: [[f32; 3]; JOINTS_N],
    tangent: [[f32; 3]; JOINTS_N],
    fin_angle: [f32; 2],
    gape: f32,
    tail_speed: f32,
}

fn centreline(yaw: &[f32; SEG_COUNT], pitch: &[f32; SEG_COUNT]) -> ([[f32; 3]; JOINTS_N], [[f32; 3]; JOINTS_N]) {
    let mut segs = [[0.0f32; 3]; SEG_COUNT];
    let mut spine = [[0.0f32; 3]; JOINTS_N];
    for i in 0..SEG_COUNT {
        let s = [
            yaw[i].cos() * pitch[i].cos(),
            yaw[i].sin() * pitch[i].cos(),
            pitch[i].sin(),
        ];
        segs[i] = s;
        spine[i + 1] = [
            spine[i][0] - SEGMENT * s[0],
            spine[i][1] - SEGMENT * s[1],
            spine[i][2] - SEGMENT * s[2],
        ];
    }
    let mid = spine[JOINTS_N / 2];
    for sp in &mut spine {
        sp[0] -= mid[0];
        sp[1] -= mid[1];
        sp[2] -= mid[2];
    }
    let mut tangent = [[0.0f32; 3]; JOINTS_N];
    tangent[0] = unit3(segs[0]);
    for i in 1..SEG_COUNT {
        tangent[i] = unit3([
            segs[i - 1][0] + segs[i][0],
            segs[i - 1][1] + segs[i][1],
            segs[i - 1][2] + segs[i][2],
        ]);
    }
    tangent[SEG_COUNT] = unit3(segs[SEG_COUNT - 1]);
    (spine, tangent)
}

fn muscle_wave(phase: f32, drive: f32, turn: f32) -> [f32; SEG_COUNT] {
    let mut out = [0.0f32; SEG_COUNT];
    for i in 0..SEG_COUNT {
        let st = (i as f32 + 0.5) / SEG_COUNT as f32;
        let recoil = (0.07 + 0.04 * drive) * (phase + 0.5).sin() * (-4.0 * st).exp();
        let wave = (0.079 * st + 0.59 * st * st) * (0.75 + 0.73 * drive);
        let bend = -turn * (st - 0.43) * 1.28;
        out[i] = recoil + wave * (phase - 4.55 * st).sin() + bend;
    }
    out
}

fn entry_pose(pose: &SwimPose, weight: f32) -> SwimPose {
    let mut yaw = [0.0f32; SEG_COUNT];
    let mut pitch = [0.0f32; SEG_COUNT];
    for i in 0..SEG_COUNT {
        let lk = [
            pose.spine[i][0] - pose.spine[i + 1][0],
            pose.spine[i][1] - pose.spine[i + 1][1],
            pose.spine[i][2] - pose.spine[i + 1][2],
        ];
        yaw[i] = lk[1].atan2(lk[0]);
        pitch[i] = lk[2].atan2(lk[0].hypot(lk[1]));
    }
    let shoulder = yaw[(0.29 * SEG_COUNT as f32) as usize];
    for i in 0..SEG_COUNT {
        let st = (i as f32 + 0.5) / SEG_COUNT as f32;
        let forebody = 1.0 - smoothstep(0.07, 0.47, st);
        yaw[i] += weight * forebody * (0.30 * shoulder - yaw[i]);
    }
    let (spine, tangent) = centreline(&yaw, &pitch);
    let mut out = pose.clone();
    out.spine = spine;
    out.tangent = tangent;
    out
}

fn deform_pt(p: [f32; 3], pose: &SwimPose) -> [f32; 3] {
    let idx = ((AXIAL_START - p[0]) / SEGMENT).clamp(0.0, JOINTS_N as f32 - 1.0001);
    let j = idx as usize;
    let b = idx - j as f32;
    let sp = [
        pose.spine[j][0] + b * (pose.spine[j + 1][0] - pose.spine[j][0]),
        pose.spine[j][1] + b * (pose.spine[j + 1][1] - pose.spine[j][1]),
        pose.spine[j][2] + b * (pose.spine[j + 1][2] - pose.spine[j][2]),
    ];
    let tang = unit3([
        pose.tangent[j][0] + b * (pose.tangent[j + 1][0] - pose.tangent[j][0]),
        pose.tangent[j][1] + b * (pose.tangent[j + 1][1] - pose.tangent[j][1]),
        pose.tangent[j][2] + b * (pose.tangent[j + 1][2] - pose.tangent[j][2]),
    ]);
    let side = unit3([-tang[1], tang[0], 0.0]);
    let dorsal = cross3(tang, side);

    let (x, mut y, mut z) = (p[0], p[1], p[2]);
    if x > 0.11 && x < 0.29 && y.abs() > 0.065 {
        let rad = (y.abs() - 0.065).max(0.0);
        let ang = if y < 0.0 { pose.fin_angle[0] } else { pose.fin_angle[1] };
        y -= y.signum() * rad * (1.0 - ang.cos());
        z += rad * ang.sin();
    }
    let ventral_depth = (-z).max(0.0) * ((-x - 0.245) / 0.065).clamp(0.0, 1.0);
    let twist = (-0.72 * pose.tail_speed).clamp(-0.80, 0.80);
    y += ventral_depth * twist.sin();
    z += ventral_depth * (1.0 - twist.cos());
    let jaw = ((x - 0.335) / 0.115).clamp(0.0, 1.0) * ((-z - 0.007) / 0.019).clamp(0.0, 1.0);
    z -= 0.063 * pose.gape * jaw;

    [
        sp[0] + y * side[0] + z * dorsal[0],
        sp[1] + y * side[1] + z * dorsal[1],
        sp[2] + y * side[2] + z * dorsal[2],
    ]
}

fn world_pt(p: [f32; 3], pose: &SwimPose) -> [f32; 3] {
    let d = deform_pt(p, pose);
    [
        pose.center[0] + d[0] * pose.forward[0] + d[1] * pose.right[0] + d[2] * pose.up[0],
        pose.center[1] + d[0] * pose.forward[1] + d[1] * pose.right[1] + d[2] * pose.up[1],
        pose.center[2] + d[0] * pose.forward[2] + d[1] * pose.right[2] + d[2] * pose.up[2],
    ]
}

fn backbone_world(pose: &SwimPose) -> [[f32; 3]; JOINTS_N] {
    let mut out = [[0.0f32; 3]; JOINTS_N];
    for (i, s) in pose.spine.iter().enumerate() {
        out[i] = [
            pose.center[0] + s[0] * pose.forward[0] + s[1] * pose.right[0] + s[2] * pose.up[0],
            pose.center[1] + s[0] * pose.forward[1] + s[1] * pose.right[1] + s[2] * pose.up[1],
            pose.center[2] + s[0] * pose.forward[2] + s[1] * pose.right[2] + s[2] * pose.up[2],
        ];
    }
    out
}

fn shark_mouth(pose: &SwimPose) -> [f32; 3] {
    let a = world_pt([0.451, 0.0, -0.006], pose);
    let b = world_pt([0.451, 0.0, -0.029], pose);
    [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1]), 0.5 * (a[2] + b[2])]
}

struct SwimmerSim {
    center: [f32; 3],
    azimuth: f32,
    pitch: f32,
    velocity: [f32; 3],
    speed: f32,
    phase: f32,
    drive: f32,
    yaw_rate: f32,
    pitch_rate: f32,
    roll: f32,
    roll_rate: f32,
    fins: [f32; 2],
    gape: f32,
    thrust: f32,
    tail_speed: f32,
    flow_weight: f32,
    joints: [f32; SEG_COUNT],
    joint_rates: [f32; SEG_COUNT],
    pose: SwimPose,
}

impl SwimmerSim {
    fn new(center: [f32; 3], forward: [f32; 3], speed: f32) -> Self {
        let dir = unit3(forward);
        let azimuth = dir[1].atan2(dir[0]);
        let pitch = dir[2].clamp(-0.999, 0.999).asin();
        let phase = 0.52;
        let drive = 0.22;
        let target = muscle_wave(phase, drive, 0.0);
        let mut joints = [0.0f32; SEG_COUNT];
        joints[0] = target[0];
        for i in 1..SEG_COUNT {
            joints[i] = target[i] - target[i - 1];
        }
        let mut s = Self {
            center,
            azimuth,
            pitch,
            velocity: [speed * dir[0], speed * dir[1], speed * dir[2]],
            speed,
            phase,
            drive,
            yaw_rate: 0.0,
            pitch_rate: 0.0,
            roll: 0.0,
            roll_rate: 0.0,
            fins: [0.0, 0.0],
            gape: 0.0,
            thrust: 0.08,
            tail_speed: 0.0,
            flow_weight: 0.0,
            joints,
            joint_rates: [0.0; SEG_COUNT],
            pose: SwimPose {
                center,
                forward: dir,
                right: [0.0, -1.0, 0.0],
                up: [0.0, 0.0, 1.0],
                phase,
                speed,
                spine: [[0.0; 3]; JOINTS_N],
                tangent: [[0.0; 3]; JOINTS_N],
                fin_angle: [0.0, 0.0],
                gape: 0.0,
                tail_speed: 0.0,
            },
        };
        s.pose = s.snapshot(&target);
        s
    }

    fn snapshot(&self, cum_yaw: &[f32; SEG_COUNT]) -> SwimPose {
        let (cs, sn) = (self.azimuth.cos(), self.azimuth.sin());
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let forward = [cs * cp, sn * cp, sp];
        let side = [sn, -cs, 0.0];
        let dorsal = cross3(side, forward);
        let (cr, sr) = (self.roll.cos(), self.roll.sin());
        let right = [
            side[0] * cr - dorsal[0] * sr,
            side[1] * cr - dorsal[1] * sr,
            side[2] * cr - dorsal[2] * sr,
        ];
        let up = [
            dorsal[0] * cr + side[0] * sr,
            dorsal[1] * cr + side[1] * sr,
            dorsal[2] * cr + side[2] * sr,
        ];
        let mut vbend = [0.0f32; SEG_COUNT];
        for i in 0..SEG_COUNT {
            let st = (i as f32 + 0.5) / SEG_COUNT as f32;
            vbend[i] = -0.76 * self.pitch_rate * (st - 0.46);
        }
        let (spine, tangent) = centreline(cum_yaw, &vbend);
        SwimPose {
            center: self.center,
            forward,
            right,
            up,
            phase: self.phase,
            speed: self.speed,
            spine,
            tangent,
            fin_angle: self.fins,
            gape: self.gape,
            tail_speed: self.tail_speed,
        }
    }

    fn step(
        &mut self,
        aim_in: [f32; 3],
        effort: f32,
        dt: f32,
        gape: f32,
        use_tail_flow: bool,
        burst: f32,
        coord_bank: bool,
    ) -> SwimPose {
        let aim = unit3(aim_in);
        let pieces = (dt * 96.0).ceil().max(1.0) as usize;
        let sub_dt = dt / pieces as f32;
        let mut last_cum = [0.0f32; SEG_COUNT];
        for _ in 0..pieces {
            last_cum = self.advance(
                aim,
                effort.clamp(0.0, 1.0),
                sub_dt,
                gape.clamp(0.0, 1.0),
                use_tail_flow,
                burst,
                coord_bank,
            );
        }
        self.pose = self.snapshot(&last_cum);
        self.pose.clone()
    }

    fn advance(
        &mut self,
        aim: [f32; 3],
        effort: f32,
        dt: f32,
        gape: f32,
        use_tail_flow: bool,
        burst: f32,
        coord_bank: bool,
    ) -> [f32; SEG_COUNT] {
        let resp = 2.5 + 5.0 * burst;
        self.drive += dt * resp * (effort + burst - self.drive);
        self.phase += dt * TAU * (0.44 + 0.73 * self.drive.min(1.15));
        let mut yaw_err = self.azimuth - aim[1].atan2(aim[0]);
        yaw_err = yaw_err.sin().atan2(yaw_err.cos());
        let pitch_err = aim[2].clamp(-0.98, 0.98).asin() - self.pitch;
        let rudder = (2.15 * yaw_err).clamp(-1.0, 1.0);
        let elev = (2.50 * pitch_err).clamp(-1.0, 1.0);
        let tgt0 = 0.31 * elev - 0.39 * (-rudder).max(0.0);
        let tgt1 = 0.31 * elev - 0.39 * rudder.max(0.0);
        self.fins[0] += dt * 5.2 * (tgt0 - self.fins[0]);
        self.fins[1] += dt * 5.2 * (tgt1 - self.fins[1]);
        let fin_asym = self.fins[0] - self.fins[1];
        let auth = 0.52 + 1.8 * self.speed.min(0.65).powi(2) + 0.20 * self.drive;
        self.yaw_rate += dt * (auth * (1.85 * rudder + 1.55 * fin_asym) - 2.15 * self.yaw_rate);
        self.pitch_rate += dt * (auth * (1.70 * elev + 0.625 * (self.fins[0] + self.fins[1])) - 2.3 * self.pitch_rate);
        self.azimuth -= self.yaw_rate * dt;
        self.pitch = (self.pitch + self.pitch_rate * dt).clamp(-1.13, 1.13);
        if coord_bank {
            let bank_goal = 0.38 * self.yaw_rate + 0.68 * fin_asym;
            self.roll_rate += dt * (22.0 * (bank_goal - self.roll) - 8.8 * self.roll_rate);
        } else {
            self.roll_rate += dt * (9.0 * (0.66 * self.yaw_rate - self.roll) - 4.7 * self.roll_rate);
        }
        self.roll += dt * self.roll_rate;
        self.gape += (1.0 - (-dt * if gape > self.gape { 18.0 } else { 23.0 }).exp()) * (gape - self.gape);

        let target = muscle_wave(self.phase, self.drive, self.yaw_rate);
        let mut cum = [0.0f32; SEG_COUNT];
        for i in 0..SEG_COUNT {
            let st = (i as f32 + 0.5) / SEG_COUNT as f32;
            let neut = if i == 0 { target[0] } else { target[i] - target[i - 1] };
            let omega = 18.4 - 3.8 * st;
            let torque = omega * omega * (neut - self.joints[i]) - 1.22 * omega * self.joint_rates[i];
            self.joint_rates[i] += dt * torque;
            self.joints[i] += dt * self.joint_rates[i];
            cum[i] = if i == 0 { self.joints[0] } else { cum[i - 1] + self.joints[i] };
        }
        let mut vbend = [0.0f32; SEG_COUNT];
        for i in 0..SEG_COUNT {
            let st = (i as f32 + 0.5) / SEG_COUNT as f32;
            vbend[i] = -0.76 * self.pitch_rate * (st - 0.46);
        }
        let prev_tail_y = self.pose.spine[JOINTS_N - 1][1];
        let (spine, tangent) = centreline(&cum, &vbend);
        self.tail_speed = (spine[JOINTS_N - 1][1] - prev_tail_y) / dt.max(1e-9);
        self.pose.spine = spine;
        self.pose.tangent = tangent;

        let forward = [
            self.azimuth.cos() * self.pitch.cos(),
            self.azimuth.sin() * self.pitch.cos(),
            self.pitch.sin(),
        ];
        let adv = dot3(self.velocity, forward);
        self.flow_weight += dt * 8.0 * ((if use_tail_flow { 1.0 } else { 0.0 }) - self.flow_weight);
        let tt = tangent[JOINTS_N - 1];
        let tail_slope = tt[1] / tt[0].max(0.35);
        let tail_load = 0.62 * self.tail_speed * self.tail_speed;
        let react = caudal_reaction(self.tail_speed, tail_slope, adv);
        let eff_load = tail_load + self.flow_weight * (react - tail_load);
        let load_resp = 3.0 + 6.0 * self.flow_weight;
        self.thrust += dt * load_resp * (eff_load - self.thrust);
        let acc = water_acceleration(self.velocity, forward, self.thrust, self.speed);
        for k in 0..3 {
            self.velocity[k] += dt * acc[k];
            self.center[k] += dt * self.velocity[k];
        }
        self.speed = norm3(self.velocity);
        cum
    }
}

fn body_gap(pt: [f32; 3], shark: &SwimPose) -> (f32, [f32; 3]) {
    let rel = [
        pt[0] - shark.center[0],
        pt[1] - shark.center[1],
        pt[2] - shark.center[2],
    ];
    let st = dot3(rel, shark.forward).clamp(-0.44, 0.45);
    let near = [
        shark.center[0] + st * shark.forward[0],
        shark.center[1] + st * shark.forward[1],
        shark.center[2] + st * shark.forward[2],
    ];
    let off = [pt[0] - near[0], pt[1] - near[1], pt[2] - near[2]];
    let r = interp_tbl(
        &[-0.44, -0.31, -0.16, 0.01, 0.18, 0.32, 0.45],
        &[0.15, 0.17, 0.15, 0.25, 0.23, 0.15, 0.09],
        st,
    );
    (norm3(off) - r, unit3(off))
}

#[derive(Clone)]
struct RawFrame {
    shark: SwimPose,
    pos: Vec<[f32; 3]>,
    vel: Vec<[f32; 3]>,
    phase: Vec<f32>,
    sizes: Vec<f32>,
    alerted: Vec<bool>,
    alive: Vec<bool>,
    target: Option<usize>,
    caught: bool,
}

#[derive(Clone)]
struct SharkFrame {
    shark: SwimPose,
    fish_pos: Vec<[f32; 3]>,
    fish_vel: Vec<[f32; 3]>,
    fish_phase: Vec<f32>,
    fish_sizes: Vec<f32>,
    fish_alive: Vec<bool>,
    swimmer_t: f32,
    #[cfg_attr(not(test), allow(dead_code))]
    caught: bool,
}

#[derive(Clone, Copy)]
struct SharkVert {
    rest: [f32; 3],
}

struct SharkQuad {
    v: [SharkVert; 4],
    rest_mid: [f32; 3],
    part: u8, // 0=trunk, 1=dorsal/anal, 2=caudal, 3=pec/pelvic
}

struct Camera {
    focus: [f32; 3],
    axes: [[f32; 3]; 3],
    scale: f32,
    aim: [f32; 2],
}

impl Camera {
    fn new(focus: [f32; 3], azimuth: f32, elevation: f32, scale: f32, aim: [f32; 2]) -> Self {
        let (c, s) = (azimuth.cos(), azimuth.sin());
        let (h, v) = (elevation.cos(), elevation.sin());
        Self {
            focus,
            axes: [[c, s, 0.0], [s * v, -c * v, -h], [s * h, -c * h, v]],
            scale,
            aim,
        }
    }

    #[inline]
    fn project(&self, pt: [f32; 3]) -> [f32; 2] {
        let rel = [pt[0] - self.focus[0], pt[1] - self.focus[1], pt[2] - self.focus[2]];
        let rx = dot3(rel, self.axes[0]);
        let ry = dot3(rel, self.axes[1]);
        [rx * self.scale + self.aim[0], ry * self.scale + self.aim[1]]
    }

    #[inline]
    fn depth(&self, pt: [f32; 3]) -> f32 {
        let rel = [pt[0] - self.focus[0], pt[1] - self.focus[1], pt[2] - self.focus[2]];
        dot3(rel, self.axes[2])
    }

    #[inline]
    fn plane_zero(&self, cell_x: f32, cell_y_15: f32) -> (f32, f32) {
        let u = (cell_x - self.aim[0]) / self.scale;
        let v = (cell_y_15 - self.aim[1]) / self.scale;
        let depth = (-self.focus[2] - v * self.axes[1][2]) / self.axes[2][2];
        let x = self.focus[0] + u * self.axes[0][0] + v * self.axes[1][0] + depth * self.axes[2][0];
        let y = self.focus[1] + u * self.axes[0][1] + v * self.axes[1][1] + depth * self.axes[2][1];
        (x, y)
    }
}

fn build_shark_quads() -> Vec<SharkQuad> {
    let mut quads = Vec::with_capacity(1450);
    let mut push_strip = |rings: &[Vec<[f32; 3]>], part: u8| {
        for r in 0..(rings.len().saturating_sub(1)) {
            let m = rings[r].len();
            for j in 0..m {
                let jn = (j + 1) % m;
                let a = rings[r][j];
                let b = rings[r][jn];
                let c = rings[r + 1][jn];
                let d = rings[r + 1][j];
                let mid = [
                    0.25 * (a[0] + b[0] + c[0] + d[0]),
                    0.25 * (a[1] + b[1] + c[1] + d[1]),
                    0.25 * (a[2] + b[2] + c[2] + d[2]),
                ];
                quads.push(SharkQuad {
                    v: [
                        SharkVert { rest: a },
                        SharkVert { rest: b },
                        SharkVert { rest: c },
                        SharkVert { rest: d },
                    ],
                    rest_mid: mid,
                    part,
                });
            }
        }
    };

    // Trunk: sorted deduplicated union of STATIONS and linspace(-0.28, 0.50, 37) x 28 circumferential points
    let mut axial = STATIONS.to_vec();
    for k in 0..37 {
        axial.push(-0.28 + (k as f32 / 36.0) * (0.50 - (-0.28)));
    }
    axial.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    axial.dedup_by(|a, b| (*a - *b).abs() < 1e-5);

    let mut rings = Vec::with_capacity(axial.len());
    for &x in &axial {
        let w = interp_tbl(&STATIONS, &HALF_WIDTH, x);
        let bh = interp_tbl(&STATIONS, &BACK_H, x);
        let bl = interp_tbl(&STATIONS, &BELLY_H, x);
        let mut ring = Vec::with_capacity(28);
        for ip in 0..28 {
            let ang = (ip as f32 / 28.0) * TAU;
            let z = ang.sin();
            ring.push([x, w * ang.cos(), if z > 0.0 { bh } else { bl } * z]);
        }
        rings.push(ring);
    }
    push_strip(&rings, 0);

    let add_fin = |push_fn: &mut dyn FnMut(&[Vec<[f32; 3]>], u8), outline: &[[f32; 3]], part: u8, span_axis: usize, thickness: f32| {
        let mut min_s = f32::INFINITY;
        let mut max_s = f32::NEG_INFINITY;
        for pt in outline {
            min_s = min_s.min(pt[span_axis]);
            max_s = max_s.max(pt[span_axis]);
        }
        let mut f_rings = Vec::new();
        for k in 0..13 {
            let level = min_s + 1e-6 + (k as f32 / 12.0) * (max_s - min_s - 2e-6);
            let mut ends = Vec::new();
            for i in 0..outline.len() {
                let p = outline[i];
                let q = outline[(i + 1) % outline.len()];
                if (p[span_axis] <= level && level < q[span_axis]) || (q[span_axis] <= level && level < p[span_axis]) {
                    let t = (level - p[span_axis]) / (q[span_axis] - p[span_axis]);
                    ends.push([
                        p[0] + (q[0] - p[0]) * t,
                        p[1] + (q[1] - p[1]) * t,
                        p[2] + (q[2] - p[2]) * t,
                    ]);
                }
            }
            if ends.len() < 2 {
                continue;
            }
            ends.sort_by(|a, b| b[0].partial_cmp(&a[0]).unwrap_or(std::cmp::Ordering::Equal));
            let leading = ends[0];
            let trailing = *ends.last().unwrap();
            let chord = norm3([leading[0] - trailing[0], leading[1] - trailing[1], leading[2] - trailing[2]]);
            let mut ring = Vec::with_capacity(6);
            for &(u, camber) in &[(0.0f32, 0.0f32), (0.27, 1.0), (0.78, 0.35), (1.0, 0.0), (0.78, -0.35), (0.27, -1.0)] {
                let mut pt = [
                    leading[0] * (1.0 - u) + trailing[0] * u,
                    leading[1] * (1.0 - u) + trailing[1] * u,
                    leading[2] * (1.0 - u) + trailing[2] * u,
                ];
                pt[3 - span_axis] += camber * chord * thickness;
                ring.push(pt);
            }
            f_rings.push(ring);
        }
        push_fn(&f_rings, part);
    };

    add_fin(&mut push_strip, &DORSAL, 1, 2, 0.115);
    add_fin(&mut push_strip, &SECOND, 1, 2, 0.115);
    add_fin(&mut push_strip, &ANAL, 1, 2, 0.115);
    add_fin(&mut push_strip, &UPPER, 2, 2, 0.115);
    add_fin(&mut push_strip, &LOWER, 2, 2, 0.115);
    for side in [-1.0f32, 1.0f32] {
        let mut p_pec = PECTORAL;
        for pt in &mut p_pec {
            pt[1] *= side;
        }
        add_fin(&mut push_strip, &p_pec, 3, 1, 0.105);
        let mut p_pel = PELVIC;
        for pt in &mut p_pel {
            pt[1] *= side;
        }
        add_fin(&mut push_strip, &p_pel, 3, 1, 0.105);
    }
    quads
}

fn cheek_pt(x: f32, z: f32, side: f32) -> [f32; 3] {
    let w = interp_tbl(&STATIONS, &HALF_WIDTH, x);
    let half = if z >= 0.0 {
        interp_tbl(&STATIONS, &BACK_H, x)
    } else {
        interp_tbl(&STATIONS, &BELLY_H, x)
    };
    let ratio = (z / half.max(0.004)).clamp(-1.0, 1.0);
    let y = side * w * (1.0 - ratio * ratio).max(0.0).sqrt();
    [x, y, z]
}

fn run_states(seconds: f32, wider_escape: bool, evasive_prey: bool, seed: u64) -> Vec<RawFrame> {
    let mut rng = Rng64::new(seed);
    let ranks = 20usize;
    let lanes = 4usize;
    let count = ranks * lanes;
    let mut pos = vec![[0.0f32; 3]; count];
    let mut z0 = vec![0.0f32; count];
    let mut sizes = vec![0.0f32; count];
    let mut phase = vec![0.0f32; count];
    let mut cruise = vec![0.0f32; count];
    let mut turn_rate = vec![0.0f32; count];

    for i in 0..count {
        let rk = (i / lanes) as f32;
        let across = (i % lanes) as f32 - 1.5;
        let x = 0.15 + 0.069 * rk + 0.054 * (0.52 * rk).sin() + rng.normal(0.023);
        let width = 0.052 + 0.034 * (0.36 * rk).sin().powi(2);
        let cline = 0.070 * (0.42 * rk).sin() + 0.035 * (0.83 * rk).sin();
        let y = width * across + cline + rng.normal(0.011);
        let z = 0.70 + rng.normal(0.012) + 0.012 * (0.42 * rk).sin();
        pos[i] = [x, y, z];
        z0[i] = z;
        sizes[i] = rng.range(0.049, 0.071);
        phase[i] = rng.range(0.0, TAU);
        cruise[i] = rng.range(0.176, 0.184);
        turn_rate[i] = 1.33 + 0.03 * across + rng.normal(0.035);
    }

    let mut speed = cruise.clone();
    let mut heading = vec![PI; count];
    let mut omega = vec![0.0f32; count];
    let mut alerted = vec![false; count];
    let mut alive = vec![true; count];
    let mut turn_side = vec![1.0f32; count];
    let mut crease = vec![0.0f32; count];

    let mut swimmer = SwimmerSim::new([-1.55, -0.168, 0.735], [1.0, 0.095, 0.0], 0.13);
    let mut chase: Option<usize> = None;
    let mut captured = false;
    let mut escape_age: Option<f32> = None;
    let mut prey_vel = [0.0f32; 3];
    let mut prey_yaw = 0.0f32;
    let mut prey_turn = 0.0f32;
    let mut escape_axis = [1.0f32, 0.0];

    let total_steps = (seconds * FPS).round() as usize;
    let mut raw_frames = Vec::with_capacity(total_steps);

    for f_idx in 0..total_steps {
        let pose = swimmer.pose.clone();
        let lips = shark_mouth(&pose);

        let mut gaps = vec![0.0f32; count];
        let mut outwards = vec![[0.0f32; 3]; count];
        let mut occluded = false;
        let mut min_rank0_gap = f32::INFINITY;

        for i in 0..count {
            let (g, o) = body_gap(pos[i], &pose);
            gaps[i] = g;
            outwards[i] = o;
            if i < lanes {
                min_rank0_gap = min_rank0_gap.min(g);
            }
        }

        let mut start_mask = vec![false; count];
        for i in 0..count {
            if !alerted[i] {
                let r = i / lanes;
                let trig = if r == 0 {
                    gaps[i] < 0.43
                } else {
                    let ahead = i - lanes;
                    alerted[ahead] && pos[i][0] <= crease[ahead] + 0.018
                };
                if trig {
                    start_mask[i] = true;
                }
            }
        }
        for i in 0..count {
            if start_mask[i] {
                let r = i / lanes;
                if r == 0 {
                    let cross_y = lips[1] + (pos[i][0] - lips[0]) * pose.forward[1] / pose.forward[0].max(1e-4);
                    turn_side[i] = if pos[i][1] >= cross_y { 1.0 } else { -1.0 };
                } else {
                    let mut sum_s = 0.0f32;
                    for l in 0..lanes {
                        sum_s += turn_side[(r - 1) * lanes + l];
                    }
                    turn_side[i] = if sum_s >= 0.0 { 1.0 } else { -1.0 };
                }
                crease[i] = pos[i][0];
                alerted[i] = true;
            }
        }

        let mut vel = vec![[0.0f32; 3]; count];
        for i in 0..count {
            let rem = (PI - (PI - heading[i]) * turn_side[i]).max(0.0);
            let des_w = if alerted[i] {
                -turn_side[i] * turn_rate[i].min(3.1 * rem)
            } else {
                0.0
            };
            omega[i] += STEP * 4.4 * (des_w - omega[i]);
            heading[i] += STEP * omega[i];
            let finished = alerted[i] && rem < 0.20;
            let exit_d = if captured && wider_escape { 0.19 } else { 0.0 };
            let goal_spd = cruise[i] + if alerted[i] { 0.068 } else { 0.0 } - if finished { 0.154 - exit_d } else { 0.0 };
            speed[i] += STEP * 1.8 * (goal_spd - speed[i]);
            vel[i] = [
                speed[i] * heading[i].cos(),
                speed[i] * heading[i].sin() + 0.034 * (5.8 * pos[i][0] + 0.4 * f_idx as f32 * STEP).sin(),
                1.4 * (z0[i] - pos[i][2]),
            ];
        }

        for i in 0..count {
            if !alive[i] {
                continue;
            }
            for j in (i + 1)..count {
                if !alive[j] {
                    continue;
                }
                let d = [pos[i][0] - pos[j][0], pos[i][1] - pos[j][1], pos[i][2] - pos[j][2]];
                let dist = norm3(d);
                if dist < 0.053 {
                    let f = 1.5 * (0.053 - dist) / dist.max(1e-4);
                    for k in 0..3 {
                        vel[i][k] += f * d[k];
                        vel[j][k] -= f * d[k];
                    }
                }
            }
            if alerted[i] && (turn_side[i] - 1.0).abs() > 0.1 {
                let mut best_d = f32::INFINITY;
                let mut best_home = [0.0f32; 3];
                for j in 0..count {
                    if alive[j] && (turn_side[j] - 1.0).abs() < 0.1 {
                        let d = [pos[j][0] - pos[i][0], pos[j][1] - pos[i][1], pos[j][2] - pos[i][2]];
                        let n = norm3(d);
                        if n < best_d {
                            best_d = n;
                            best_home = d;
                        }
                    }
                }
                let mut min_clr = f32::INFINITY;
                for s_k in 0..=8 {
                    let fk = s_k as f32 / 8.0;
                    let pt = [
                        pos[i][0] + fk * best_home[0],
                        pos[i][1] + fk * best_home[1],
                        pos[i][2] + fk * best_home[2],
                    ];
                    min_clr = min_clr.min(body_gap(pt, &pose).0);
                }
                if min_clr < 0.01 {
                    occluded = true;
                }
                let rem = (PI - (PI - heading[i]) * turn_side[i]).max(0.0);
                if rem < 0.85 {
                    let threat = min_clr < 0.18 && best_d > 0.13;
                    let fwd = [heading[i].cos(), heading[i].sin(), 0.0];
                    let away = unit3([pos[i][0] - lips[0], pos[i][1] - lips[1], pos[i][2] - lips[2]]);
                    let aim = if threat {
                        unit3([1.15 * fwd[0] + 0.32 * away[0], 1.15 * fwd[1] + 0.32 * away[1], 0.0])
                    } else {
                        unit3(best_home)
                    };
                    let ang = aim[1].atan2(aim[0]);
                    let err = (ang - heading[i]).sin().atan2((ang - heading[i]).cos());
                    heading[i] += STEP * (2.5 * err).clamp(-1.5, 1.5);
                    let sp = if threat { 0.31 } else { 0.20 };
                    vel[i][0] = sp * heading[i].cos();
                    vel[i][1] = sp * heading[i].sin();
                }
            } else if (turn_side[i] - 1.0).abs() < 0.1 {
                let mut standoff = 0.08;
                if wider_escape {
                    standoff += (1.8 * (pose.speed - 0.19).max(0.0)).min(0.14);
                }
                let spring = (standoff - gaps[i]).max(0.0);
                for k in 0..3 {
                    vel[i][k] += 4.5 * spring * outwards[i][k];
                }
            }
        }

        if evasive_prey {
            if let Some(ci) = chase {
                if !captured {
                    let off = [pos[ci][0] - lips[0], pos[ci][1] - lips[1], pos[ci][2] - lips[2]];
                    if escape_age.is_none() && norm3(off) < 0.175 && dot3(off, pose.forward) > 0.10 {
                        escape_age = Some(0.0);
                        prey_vel = vel[ci];
                        prey_yaw = prey_vel[1].atan2(prey_vel[0]);
                        prey_turn = 0.0;
                        let mut maj_m = [0.0f32; 2];
                        let mut mc: f32 = 0.0;
                        for j in 0..count {
                            if (turn_side[j] - 1.0).abs() < 0.1 {
                                maj_m[0] += pos[j][0];
                                maj_m[1] += pos[j][1];
                                mc += 1.0;
                            }
                        }
                        maj_m[0] /= mc.max(1.0);
                        maj_m[1] /= mc.max(1.0);
                        let pv_l = prey_vel[0].hypot(prey_vel[1]).max(1e-6);
                        let fw_l = (pos[ci][0] - maj_m[0]).hypot(pos[ci][1] - maj_m[1]).max(1e-6);
                        let ex = prey_vel[0] / pv_l + 0.85 * (pos[ci][0] - maj_m[0]) / fw_l;
                        let ey = prey_vel[1] / pv_l + 0.85 * (pos[ci][1] - maj_m[1]) / fw_l;
                        let el = ex.hypot(ey).max(1e-6);
                        escape_axis = [ex / el, ey / el];
                    }
                    if let Some(ea) = escape_age.as_mut() {
                        let kick_t = ((*ea - 0.10) / 0.37).clamp(0.0, 1.0);
                        let kick = (PI * kick_t).sin().powi(2);
                        let rel = ((*ea - 0.42) / 0.42).clamp(0.0, 1.0);
                        let vl = vel[ci][0].hypot(vel[ci][1]).max(1e-6);
                        let dx = (1.0 - rel) * escape_axis[0] + rel * vel[ci][0] / vl;
                        let dy = (1.0 - rel) * escape_axis[1] + rel * vel[ci][1] / vl;
                        let err = (dy.atan2(dx) - prey_yaw).sin().atan2((dy.atan2(dx) - prey_yaw).cos());
                        prey_turn += STEP * 16.0 * ((9.0 * err).clamp(-6.0, 6.0) - prey_turn);
                        prey_yaw += STEP * prey_turn;
                        let ax = [prey_yaw.cos(), prey_yaw.sin()];
                        let sp_xy = prey_vel[0].hypot(prey_vel[1]);
                        let ft = 2.4 * vl * vl + 0.85 * kick;
                        let dot_a = prey_vel[0] * ax[0] + prey_vel[1] * ax[1];
                        let slip = [prey_vel[0] - dot_a * ax[0], prey_vel[1] - dot_a * ax[1]];
                        prey_vel[0] += STEP * (ft * ax[0] - 2.4 * sp_xy * prey_vel[0] - 7.0 * slip[0]);
                        prey_vel[1] += STEP * (ft * ax[1] - 2.4 * sp_xy * prey_vel[1] - 7.0 * slip[1]);
                        prey_vel[2] = vel[ci][2];
                        vel[ci] = prey_vel;
                        *ea += STEP;
                    }
                }
            }
        }

        for i in 0..count {
            for k in 0..3 {
                pos[i][k] += STEP * vel[i][k];
            }
        }

        if chase.is_none() && occluded {
            let mut best = None;
            let mut best_iso = -1.0f32;
            for i in 0..count {
                let b = [pos[i][0] - lips[0], pos[i][1] - lips[1], pos[i][2] - lips[2]];
                if dot3(b, pose.forward) > -0.12 && norm3(b) < 0.95 {
                    let mut iso = f32::INFINITY;
                    for j in 0..count {
                        if i != j {
                            iso = iso.min(norm3([pos[i][0] - pos[j][0], pos[i][1] - pos[j][1], pos[i][2] - pos[j][2]]));
                        }
                    }
                    if iso > 0.33 && iso > best_iso {
                        best_iso = iso;
                        best = Some(i);
                    }
                }
            }
            chase = best;
        }

        let (aim, effort, gape, burst) = if let Some(ci) = chase {
            if captured {
                let mut a = pose.forward;
                a[2] = 0.5 * (0.73 - pose.center[2]);
                (a, 0.09, 0.0, 0.0)
            } else {
                let disp = [pos[ci][0] - lips[0], pos[ci][1] - lips[1], pos[ci][2] - lips[2]];
                let dist = norm3(disp);
                let mut lt = (dist / pose.speed.max(0.28)).min(0.75);
                if escape_age.is_some() {
                    lt = lt.min(0.32);
                }
                let intercept = [
                    pos[ci][0] + lt * vel[ci][0],
                    pos[ci][1] + lt * vel[ci][1],
                    pos[ci][2] + lt * vel[ci][2],
                ];
                let a = [
                    intercept[0] - pose.center[0] + (pose.center[0] + 0.451 * pose.forward[0] - lips[0]),
                    intercept[1] - pose.center[1] + (pose.center[1] + 0.451 * pose.forward[1] - lips[1]),
                    intercept[2] - pose.center[2] + (pose.center[2] + 0.451 * pose.forward[2] - lips[2]),
                ];
                let mut eff = 0.66;
                let mut br = 0.0;
                if let Some(ea) = escape_age {
                    let align = dot3(unit3(a), pose.forward);
                    eff += 0.34 * ((align - 0.93) / 0.065).clamp(0.0, 1.0);
                    let stroke = ((ea - 0.06) / 0.36).clamp(0.0, 1.0);
                    br = 1.7 * (PI * stroke).sin().powi(2);
                }
                (a, eff, ((0.23 - dist) / 0.15).clamp(0.0, 1.0), br)
            }
        } else {
            let any_al = alerted.iter().any(|&a| a);
            ([1.0, 0.095, 0.0], if any_al { 0.29 } else { 0.12 }, 0.0, 0.0)
        };

        let mut next_pose = swimmer.step(aim, effort, STEP, gape, escape_age.is_some(), burst, evasive_prey);
        if let Some(ci) = chase {
            if !captured {
                let nl = shark_mouth(&next_pose);
                let ax = unit3(vel[ci]);
                let al = dot3([nl[0] - pos[ci][0], nl[1] - pos[ci][1], nl[2] - pos[ci][2]], ax)
                    .clamp(-sizes[ci] * 0.5, sizes[ci] * 0.5);
                let d_mouth = norm3([
                    nl[0] - pos[ci][0] - al * ax[0],
                    nl[1] - pos[ci][1] - al * ax[1],
                    nl[2] - pos[ci][2] - al * ax[2],
                ]);
                if d_mouth < 0.040 && next_pose.gape > 0.30 {
                    alive[ci] = false;
                    captured = true;
                }
            } else {
                pos[ci] = shark_mouth(&next_pose);
            }
        }

        for i in 0..count {
            phase[i] += STEP * (4.4 + 18.0 * norm3(vel[i]) + 0.55 * omega[i].abs());
        }

        if evasive_prey && chase.is_none() {
            let ew = smoothstep(0.55, 0.86, min_rank0_gap);
            if ew > 0.0 {
                next_pose = entry_pose(&next_pose, ew);
            }
        }

        raw_frames.push(RawFrame {
            shark: next_pose,
            pos: pos.clone(),
            vel,
            phase: phase.clone(),
            sizes: sizes.clone(),
            alerted: alerted.clone(),
            alive: alive.clone(),
            target: chase,
            caught: captured,
        });
    }

    raw_frames
}

fn quantile_sorted(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = q * (sorted.len() - 1) as f32;
    let lo = idx.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    let w = idx - lo as f32;
    sorted[lo] + w * (sorted[hi] - sorted[lo])
}

fn fixed_camera(base_frames: &[RawFrame]) -> Camera {
    let target = base_frames.iter().find_map(|f| f.target);
    let mut points: Vec<[f32; 3]> = Vec::new();
    let mut active_idx = 0usize;
    for f in base_frames {
        if f.alerted.iter().any(|&a| a) && !f.caught {
            if let Some(ti) = target {
                points.push(f.pos[ti]);
            }
            if active_idx % 4 == 0 {
                let bb = backbone_world(&f.shark);
                points.push(bb[0]);
                points.push(bb[12]);
            }
            if active_idx % 8 == 0 {
                let mut xs: Vec<f32> = f.pos.iter().map(|p| p[0]).collect();
                let mut ys: Vec<f32> = f.pos.iter().map(|p| p[1]).collect();
                let mut zs: Vec<f32> = f.pos.iter().map(|p| p[2]).collect();
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                points.push([quantile_sorted(&xs, 0.08), quantile_sorted(&ys, 0.08), quantile_sorted(&zs, 0.08)]);
                points.push([quantile_sorted(&xs, 0.92), quantile_sorted(&ys, 0.92), quantile_sorted(&zs, 0.92)]);
            }
            active_idx += 1;
        }
    }
    if points.is_empty() {
        return Camera::new([-0.08, -0.02, 0.71], -PI + 0.68, 83.0f32.to_radians(), 45.0, [45.0, 54.0]);
    }
    let mut mean = [0.0f32; 3];
    for p in &points {
        mean[0] += p[0];
        mean[1] += p[1];
        mean[2] += p[2];
    }
    let inv = 1.0 / points.len() as f32;
    mean = [mean[0] * inv, mean[1] * inv, mean[2] * inv];

    let elev = 83.0f32.to_radians();
    let mut best_scale = -1.0f32;
    let mut best_az = -PI + 0.68;
    let mut best_mid = [0.0f32; 2];

    for k in 0..31 {
        let az = 0.33 + (1.22 - 0.33) * (k as f32 / 30.0) - PI;
        let trial = Camera::new(mean, az, elev, 1.0, [0.0, 0.0]);
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for &p in &points {
            let xy = trial.project(p);
            min_x = min_x.min(xy[0]);
            max_x = max_x.max(xy[0]);
            min_y = min_y.min(xy[1]);
            max_y = max_y.max(xy[1]);
        }
        let sc = 58.0f32
            .min(81.0 / (max_x - min_x).max(1e-4))
            .min(96.0 / (max_y - min_y).max(1e-4));
        if sc > best_scale {
            best_scale = sc;
            best_az = az;
            best_mid = [0.5 * (min_x + max_x), 0.5 * (min_y + max_y)];
        }
    }

    Camera::new(
        mean,
        best_az,
        elev,
        best_scale,
        [45.0 - best_scale * best_mid[0], 54.0 - best_scale * best_mid[1]],
    )
}

fn simulate_cutoff_loop() -> (Vec<SharkFrame>, Camera) {
    let period = (LOOP_SECONDS * FPS) as usize;
    let mut chosen_seed = 3794u64;
    let mut chosen_base = Vec::new();
    let mut chosen_raw = Vec::new();

    for cand in 3794u64..3900 {
        let base = run_states(12.0, true, false, cand);
        let has_base_target = base.iter().any(|f| f.target.is_some());
        if !has_base_target {
            continue;
        }
        let raw = run_states(LOOP_SECONDS * 1.5, true, true, cand);
        let has_catch_in_period = raw.iter().take(period).any(|f| f.caught);
        chosen_seed = cand;
        chosen_base = base;
        chosen_raw = raw;
        if has_catch_in_period {
            break;
        }
    }
    if chosen_raw.is_empty() {
        chosen_base = run_states(12.0, true, false, chosen_seed);
        chosen_raw = run_states(LOOP_SECONDS * 1.5, true, true, chosen_seed);
    }
    let cam = fixed_camera(&chosen_base);

    // Build incoming approach frames (-10s .. 0s)
    let half_p = period / 2;
    let first = &chosen_raw[0];
    let count = first.pos.len();
    let mut approach_frames: Vec<RawFrame> = Vec::with_capacity(half_p);
    for idx in 0..half_p {
        let time = (idx as f32 - half_p as f32) * STEP;
        let mut ap_pos = vec![[0.0f32; 3]; count];
        let mut ap_vel = vec![[0.0f32; 3]; count];
        let mut ap_ph = vec![0.0f32; count];
        for i in 0..count {
            let cr = first.vel[i][0].hypot(first.vel[i][1]);
            let beat = 4.4 + 18.0 * cr;
            let rate = 5.8 * first.vel[i][0] + 0.4;
            let ang = 5.8 * first.pos[i][0];
            let init_c = 0.034 * ang.sin();
            let cur_c = 0.034 * (ang + rate * time).sin();
            ap_pos[i] = [
                first.pos[i][0] + time * first.vel[i][0],
                first.pos[i][1] + time * first.vel[i][1] + 0.034 * (ang.cos() - (ang + rate * time).cos()) / rate - time * init_c,
                first.pos[i][2],
            ];
            ap_vel[i] = [first.vel[i][0], first.vel[i][1] + cur_c - init_c, first.vel[i][2]];
            ap_ph[i] = first.phase[i] + time * beat;
        }
        let ph_s = first.shark.phase + time * TAU * (0.44 + 0.73 * 0.22);
        let (spine, tangent) = centreline(&muscle_wave(ph_s, 0.22, 0.0), &[0.0; SEG_COUNT]);
        let mut p_s = first.shark.clone();
        p_s.center = [
            p_s.center[0] + time * p_s.speed * p_s.forward[0],
            p_s.center[1] + time * p_s.speed * p_s.forward[1],
            p_s.center[2] + time * p_s.speed * p_s.forward[2],
        ];
        p_s.phase = ph_s;
        p_s.spine = spine;
        p_s.tangent = tangent;
        p_s = entry_pose(&p_s, 1.0);
        approach_frames.push(RawFrame {
            shark: p_s,
            pos: ap_pos,
            vel: ap_vel,
            phase: ap_ph,
            sizes: first.sizes.clone(),
            alerted: vec![false; count],
            alive: vec![true; count],
            target: None,
            caught: false,
        });
    }

    // Build closed 20s seamless loop frames by joining primary & secondary shoal
    let mut out = Vec::with_capacity(period);
    for i in 0..period {
        let mut prim = &chosen_raw[i];
        let mut sec = if i < half_p {
            &chosen_raw[i + period]
        } else {
            &approach_frames[i - half_p]
        };
        let mut st_time = i as f32 / FPS;
        if i >= half_p {
            let bb = backbone_world(&prim.shark);
            let max_y = bb.iter().map(|&wp| cam.project(wp)[1]).fold(f32::NEG_INFINITY, f32::max);
            if max_y < -16.0 {
                std::mem::swap(&mut prim, &mut sec);
                st_time = i as f32 / FPS - LOOP_SECONDS;
            }
        }
        let mut fish_pos = Vec::with_capacity(count * 2);
        let mut fish_vel = Vec::with_capacity(count * 2);
        let mut fish_phase = Vec::with_capacity(count * 2);
        let mut fish_sizes = Vec::with_capacity(count * 2);
        let mut fish_alive = Vec::with_capacity(count * 2);
        for k in 0..count {
            fish_pos.push(prim.pos[k]);
            fish_vel.push(prim.vel[k]);
            fish_phase.push(prim.phase[k]);
            fish_sizes.push(prim.sizes[k]);
            fish_alive.push(prim.alive[k] && !(prim.caught && prim.target == Some(k)));
        }
        for k in 0..count {
            let alive_sec = sec.alive[k] && !(sec.caught && sec.target == Some(k));
            if alive_sec {
                fish_pos.push(sec.pos[k]);
                fish_vel.push(sec.vel[k]);
                fish_phase.push(sec.phase[k]);
                fish_sizes.push(sec.sizes[k]);
                fish_alive.push(true);
            }
        }
        out.push(SharkFrame {
            shark: prim.shark.clone(),
            fish_pos,
            fish_vel,
            fish_phase,
            fish_sizes,
            fish_alive,
            swimmer_t: st_time,
            caught: prim.caught,
        });
    }
    (out, cam)
}

#[inline]
fn water_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let u = smoothstep(0.0, 1.0, x - ix);
    let v = smoothstep(0.0, 1.0, y - iy);
    let h = |a: f32, b: f32| -> f32 {
        let q = (71.31 * (ix + a) + 127.73 * (iy + b)).sin() * 43758.5;
        q - q.floor()
    };
    (1.0 - v) * ((1.0 - u) * h(0.0, 0.0) + u * h(1.0, 0.0)) + v * ((1.0 - u) * h(0.0, 1.0) + u * h(1.0, 1.0))
}

#[inline]
fn caustics(x: f32, y: f32, t: f32) -> f32 {
    let fund = TAU / LOOP_SECONDS;
    let snap = |r: f32| fund * (r / fund).round();
    let (a, b, c, d, e, f) = (
        snap(0.65) * t,
        snap(0.33) * t,
        snap(0.57) * t,
        snap(0.48) * t,
        snap(0.70) * t,
        snap(0.55) * t,
    );
    let u = 8.6 * x + 5.0 * y + 1.7 * (3.1 * y - a).sin() + 2.8 * (2.0 * x + 1.2 * y + b).sin();
    let v = 3.7 * x - 11.9 * y + 2.1 * (3.0 * x - 1.3 * y + c).sin() - 1.4 * (2.0 * y - d).sin();
    let fold = (u - e).cos() + 0.78 * (v + f).cos();
    (-8.0 * (fold - 0.23).powi(2)).exp()
}

#[derive(Clone)]
struct RenderQuad {
    z_mean: f32,
    s_pts: [[f32; 2]; 4],
    rgba: [u8; 4],
}

fn fill_tri_sub(buf: &mut [[u8; 4]], bw: usize, bh: usize, a: [f32; 2], b: [f32; 2], c: [f32; 2], col: [u8; 4]) {
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area.abs() < 1e-5 {
        return;
    }
    let min_x = (a[0].min(b[0]).min(c[0]).floor() as i32).max(0);
    let max_x = (a[0].max(b[0]).max(c[0]).ceil() as i32).min(bw as i32 - 1);
    let min_y = (a[1].min(b[1]).min(c[1]).floor() as i32).max(0);
    let max_y = (a[1].max(b[1]).max(c[1]).ceil() as i32).min(bh as i32 - 1);
    if min_x > max_x || min_y > max_y {
        return;
    }
    let (v0, v1, v2) = if area > 0.0 { (a, b, c) } else { (a, c, b) };
    let e0x = v1[0] - v0[0];
    let e0y = v1[1] - v0[1];
    let e1x = v2[0] - v1[0];
    let e1y = v2[1] - v1[1];
    let e2x = v0[0] - v2[0];
    let e2y = v0[1] - v2[1];
    for sy in min_y..=max_y {
        let py = sy as f32 + 0.5;
        for sx in min_x..=max_x {
            let px = sx as f32 + 0.5;
            let c0 = e0x * (py - v0[1]) - e0y * (px - v0[0]);
            let c1 = e1x * (py - v1[1]) - e1y * (px - v1[0]);
            let c2 = e2x * (py - v2[1]) - e2y * (px - v2[0]);
            if c0 >= -1e-4 && c1 >= -1e-4 && c2 >= -1e-4 {
                buf[sy as usize * bw + sx as usize] = col;
            }
        }
    }
}

fn fill_poly_sub(buf: &mut [[u8; 4]], bw: usize, bh: usize, poly: &[[f32; 2]], col: [u8; 4]) {
    if poly.len() < 3 {
        return;
    }
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for p in poly {
        min_x = min_x.min(p[0].floor() as i32);
        max_x = max_x.max(p[0].ceil() as i32);
        min_y = min_y.min(p[1].floor() as i32);
        max_y = max_y.max(p[1].ceil() as i32);
    }
    min_x = min_x.max(0);
    max_x = max_x.min(bw as i32 - 1);
    min_y = min_y.max(0);
    max_y = max_y.min(bh as i32 - 1);
    if min_x > max_x || min_y > max_y {
        return;
    }
    let n = poly.len();
    for sy in min_y..=max_y {
        let py = sy as f32 + 0.5;
        for sx in min_x..=max_x {
            let px = sx as f32 + 0.5;
            let mut inside = false;
            let mut j = n - 1;
            for i in 0..n {
                let pi = poly[i];
                let pj = poly[j];
                if ((pi[1] > py) != (pj[1] > py))
                    && (px < (pj[0] - pi[0]) * (py - pi[1]) / (pj[1] - pi[1]) + pi[0])
                {
                    inside = !inside;
                }
                j = i;
            }
            if inside {
                buf[sy as usize * bw + sx as usize] = col;
            }
        }
    }
}

fn stroke_polyline_sub(buf: &mut [[u8; 4]], bw: usize, bh: usize, pts: &[[f32; 2]], radius: f32, col: [u8; 4]) {
    if pts.len() < 2 {
        return;
    }
    let r2 = radius * radius;
    let pad = radius.ceil() as i32 + 1;
    for seg in pts.windows(2) {
        let a = seg[0];
        let b = seg[1];
        let min_x = (a[0].min(b[0]).floor() as i32 - pad).max(0);
        let max_x = (a[0].max(b[0]).ceil() as i32 + pad).min(bw as i32 - 1);
        let min_y = (a[1].min(b[1]).floor() as i32 - pad).max(0);
        let max_y = (a[1].max(b[1]).ceil() as i32 + pad).min(bh as i32 - 1);
        let vx = b[0] - a[0];
        let vy = b[1] - a[1];
        let vv = (vx * vx + vy * vy).max(1e-6);
        for sy in min_y..=max_y {
            let py = sy as f32 + 0.5;
            for sx in min_x..=max_x {
                let px = sx as f32 + 0.5;
                let t = (((px - a[0]) * vx + (py - a[1]) * vy) / vv).clamp(0.0, 1.0);
                let dx = px - (a[0] + t * vx);
                let dy = py - (a[1] + t * vy);
                if dx * dx + dy * dy <= r2 {
                    buf[sy as usize * bw + sx as usize] = col;
                }
            }
        }
    }
}

pub struct AsciiSharkState {
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
    pointer_active: bool,
    shock_timer: f32,
    shock_origin: [f32; 2],
    shark_burst: f32,
    frames: Vec<SharkFrame>,
    cam: Camera,
    quads: Vec<SharkQuad>,
    // Scratch buffers
    shark_cov: Vec<f32>,
    shark_rgb: Vec<[f32; 3]>,
    sub_rgba: Vec<[u8; 4]>,
    shadow_a: Vec<f32>,
    shadow_b: Vec<f32>,
    under_r: Vec<u8>,
}

impl Default for AsciiSharkState {
    fn default() -> Self {
        Self::new(1280.0, 720.0, 1.0)
    }
}

impl AsciiSharkState {
    pub fn new(css_w: f32, css_h: f32, dpi_scale: f32) -> Self {
        let (frames, cam) = simulate_cutoff_loop();
        let quads = build_shark_quads();
        let mut canvas = AsciiCanvas::default();
        let cs = (AsciiLabParams::default().char_size * 0.85).clamp(4.4, 12.0);
        canvas.ensure_layout(css_w, css_h, cs, false);

        let mut st = Self {
            params: AsciiLabParams::default(),
            canvas,
            anim_time: 5.2,
            low_mode: false,
            icons: Vec::new(),
            icons_vis: false,
            css_w,
            css_h,
            dpi_scale,
            pointer_norm: [0.5, 0.5],
            pointer_active: false,
            shock_timer: 0.0,
            shock_origin: [0.5, 0.5],
            shark_burst: 0.0,
            frames,
            cam,
            quads,
            shark_cov: Vec::new(),
            shark_rgb: Vec::new(),
            sub_rgba: Vec::new(),
            shadow_a: Vec::new(),
            shadow_b: Vec::new(),
            under_r: Vec::new(),
        };
        st.rebuild_grid();
        st
    }

    #[cfg(test)]
    pub(crate) fn has_capture_and_scale(&self) -> (bool, f32) {
        (self.frames.iter().any(|f| f.caught), self.cam.scale)
    }

    pub fn resize(&mut self, css_w: f32, css_h: f32, dpi: f32) {
        self.css_w = css_w;
        self.css_h = css_h;
        self.dpi_scale = dpi;
        let cs = (self.params.char_size * 0.85).clamp(4.4, 12.0);
        if self.canvas.ensure_layout(css_w, css_h, cs, self.low_mode) {
            self.rebuild_grid();
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            let cs = (self.params.char_size * 0.85).clamp(4.4, 12.0);
            self.canvas.ensure_layout(self.css_w, self.css_h, cs, low);
            self.rebuild_grid();
        }
    }

    pub fn sync_params(&mut self, next: AsciiLabParams) {
        let changed = self.params != next;
        self.params = next;
        let cs = (self.params.char_size * 0.85).clamp(4.4, 12.0);
        self.canvas.ensure_layout(self.css_w, self.css_h, cs, self.low_mode);
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
        self.pointer_norm = [
            (css_x / self.css_w.max(1.0)).clamp(0.0, 1.0),
            (css_y / self.css_h.max(1.0)).clamp(0.0, 1.0),
        ];
        self.pointer_active = true;
        if kind == 1 {
            self.shock_origin = self.pointer_norm;
            self.shock_timer = 1.0;
            self.shark_burst = 1.0;
            return true;
        }
        false
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.anim_time = 5.2;
        self.shock_timer = 0.0;
        self.shark_burst = 0.0;
        self.rebuild_grid();
    }

    pub fn step(&mut self, dt: f32) {
        let dt_eff = dt.clamp(0.0, 0.05) * self.params.speed * (1.0 + 0.55 * self.shark_burst);
        self.anim_time = (self.anim_time + dt_eff) % LOOP_SECONDS;
        self.shock_timer = (self.shock_timer - dt * 1.15).max(0.0);
        self.shark_burst = (self.shark_burst - dt * 1.4).max(0.0);
        self.rebuild_grid();
    }

    fn ocean_palette(&self) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3]) {
        match self.params.palette {
            // 1: Abyssal Indigo & Bioluminescent Silver
            1 => (
                [10.0, 22.0, 48.0],
                [68.0, 146.0, 212.0],
                [8.0, 16.0, 38.0],
                [36.0, 56.0, 94.0],
            ),
            // 2: Sunset Warm Lagoon
            2 => (
                [24.0, 46.0, 52.0],
                [168.0, 184.0, 152.0],
                [16.0, 32.0, 38.0],
                [52.0, 74.0, 62.0],
            ),
            // 0: Exact `cutoff_render.py` turquoise reef water
            _ => (
                [18.0, 53.0, 63.0],
                [130.0, 175.0, 162.0],
                [10.0, 35.0, 42.0],
                [37.0, 63.0, 58.0],
            ),
        }
    }

    pub fn rebuild_grid(&mut self) {
        let cols = self.canvas.cols;
        let rows = self.canvas.rows;
        if cols == 0 || rows == 0 {
            return;
        }
        let len = cols * rows;
        self.shark_cov.resize(len, 0.0);
        self.shark_cov.fill(0.0);
        self.shark_rgb.resize(len, [0.0; 3]);
        self.shark_rgb.fill([0.0; 3]);
        self.shadow_a.resize(len, 0.0);
        self.shadow_b.resize(len, 0.0);
        self.under_r.resize(len, 0);

        let t = self.anim_time.rem_euclid(LOOP_SECONDS);
        let f_idx = (((t / LOOP_SECONDS) * self.frames.len() as f32) as usize) % self.frames.len();
        let frame = &self.frames[f_idx];
        let pose = &frame.shark;

        let scale_to_screen = (cols as f32 / 90.0).min(rows as f32 / 72.0);
        let off_x = (cols as f32 - 90.0 * scale_to_screen) * 0.5;
        let off_y = (rows as f32 - 72.0 * scale_to_screen) * 0.5;
        let to_screen_col = |rx: f32| off_x + rx * scale_to_screen;
        let to_screen_row = |ry15: f32| off_y + (ry15 / 1.5) * scale_to_screen;
        let to_ref_x = |sc: f32| (sc - off_x) / scale_to_screen;
        let to_ref_y15 = |sr: f32| ((sr - off_y) / scale_to_screen) * 1.5;

        let ptr_bias = if self.pointer_active && self.params.mouse_interact {
            [(self.pointer_norm[0] - 0.5) * 2.0, (self.pointer_norm[1] - 0.5) * 1.5]
        } else {
            [0.0, 0.0]
        };

        // 1. Rasterize 3D Articulated Shark with Painter's Algorithm into 6x6 Supersampled Subpixel Buffer
        let sh_axes = [pose.forward, pose.right, pose.up];
        let mut rot = [[0.0f32; 3]; 3];
        for r_i in 0..3 {
            for c_i in 0..3 {
                rot[r_i][c_i] = dot3(sh_axes[r_i], self.cam.axes[c_i]);
            }
        }
        let rot_fwd = [rot[0][0], rot[0][1], rot[0][2]];
        let key_l = [-0.45f32, -0.54, 0.71];
        let half_v = unit3([key_l[0], key_l[1], key_l[2] + 1.0]);
        let sh_center_scr = self.cam.project(pose.center);

        let mut render_quads = Vec::with_capacity(self.quads.len());
        let mut bb_c0 = cols as i32;
        let mut bb_c1 = -1i32;
        let mut bb_r0 = rows as i32;
        let mut bb_r1 = -1i32;

        for quad in &self.quads {
            let mut w_pts = [[0.0f32; 3]; 4];
            let mut s_pts = [[0.0f32; 2]; 4];
            let mut z_sum = 0.0f32;
            for k in 0..4 {
                w_pts[k] = world_pt(quad.v[k].rest, pose);
                let sp = self.cam.project(w_pts[k]);
                let sc = to_screen_col(sp[0]) + ptr_bias[0];
                let sr = to_screen_row(sp[1]) + ptr_bias[1];
                s_pts[k] = [sc, sr];
                z_sum += self.cam.depth(w_pts[k]);
                bb_c0 = bb_c0.min(sc.floor() as i32);
                bb_c1 = bb_c1.max(sc.ceil() as i32);
                bb_r0 = bb_r0.min(sr.floor() as i32);
                bb_r1 = bb_r1.max(sr.ceil() as i32);
            }
            let z_mean = z_sum * 0.25;

            let d02 = [w_pts[2][0] - w_pts[0][0], w_pts[2][1] - w_pts[0][1], w_pts[2][2] - w_pts[0][2]];
            let d13 = [w_pts[3][0] - w_pts[1][0], w_pts[3][1] - w_pts[1][1], w_pts[3][2] - w_pts[1][2]];
            let wn = cross3(d02, d13);
            let mut cn = unit3([
                dot3(wn, self.cam.axes[0]),
                dot3(wn, self.cam.axes[1]),
                dot3(wn, self.cam.axes[2]),
            ]);
            if cn[2] < 0.0 {
                cn = [-cn[0], -cn[1], -cn[2]];
            }

            let rx = quad.rest_mid[0];
            let ry = quad.rest_mid[1];
            let rz = quad.rest_mid[2];
            let half = interp_tbl(&STATIONS, &BACK_H, rx).max(0.004);
            let side_z = rz / half;
            let mut base = if quad.part == 0 {
                let p_t = ((0.40 - side_z) / 0.80).clamp(0.0, 1.0);
                let pale = p_t * p_t * (3.0 - 2.0 * p_t);
                let mut b = [
                    26.0 + pale * 176.0,
                    48.0 + pale * 161.0,
                    65.0 + pale * 137.0,
                ];
                let band = (-((side_z - 0.03 - 0.065 * ((rx - 0.10) * 12.0).cos()) / 0.14).powi(2)).exp();
                b[0] += band * 23.0;
                b[1] += band * 29.0;
                b[2] += band * 30.0;
                b
            } else {
                let leading = dot3(cn, rot_fwd).max(0.0).powf(1.4);
                let mut b = [
                    55.0 + leading * 56.0,
                    82.0 + leading * 70.0,
                    96.0 + leading * 70.0,
                ];
                if (quad.part == 2 && rx < -0.345) || (quad.part == 3 && ry.abs() > 0.169) {
                    b[0] *= 0.58;
                    b[1] *= 0.58;
                    b[2] *= 0.58;
                }
                b
            };

            let diffuse = dot3(cn, key_l).max(0.0);
            let facing = cn[2].max(0.0);
            let fresnel = (1.0 - facing).powf(2.2);
            let cam_rest_x = rx * rot[0][0] + ry * rot[1][0] + rz * rot[2][0];
            let cam_rest_y = rx * rot[0][1] + ry * rot[1][1] + rz * rot[2][1];
            let pos_xy = [
                cam_rest_x * self.cam.scale + sh_center_scr[0],
                cam_rest_y * self.cam.scale + sh_center_scr[1],
            ];
            let wave = 0.24 * pos_xy[0] - 0.13 * pos_xy[1] + 1.6 * (0.065 * pos_xy[1] + 0.46 * frame.swimmer_t).sin()
                - 0.96 * frame.swimmer_t;
            let caustic = (-4.9 * (1.0 - wave.cos())).exp();
            let refl = dot3(cn, half_v).max(0.0);
            let sky = 0.60 + 0.44 * diffuse + 0.13 * fresnel;
            let r5 = refl.powi(5) * (0.28 + 0.28 * caustic);
            let r22 = refl.powi(22) * (0.12 + 0.98 * caustic);
            let fr_b = (0.19 + 0.28 * caustic) * fresnel * diffuse;
            let lateral = if quad.part == 0 {
                (-((side_z - 0.16) / 0.45).powi(2)).exp() * (0.45 + 0.55 * diffuse)
            } else {
                0.0
            };
            base[0] = (base[0] * sky + r5 * 97.0 + r22 * 239.0 + fr_b * 97.0 + lateral * 49.0).clamp(0.0, 255.0);
            base[1] = (base[1] * sky + r5 * 155.0 + r22 * 247.0 + fr_b * 155.0 + lateral * 58.0).clamp(0.0, 255.0);
            base[2] = (base[2] * sky + r5 * 177.0 + r22 * 241.0 + fr_b * 177.0 + lateral * 59.0).clamp(0.0, 255.0);

            render_quads.push(RenderQuad {
                z_mean,
                s_pts,
                rgba: [base[0].round() as u8, base[1].round() as u8, base[2].round() as u8, 255],
            });
        }

        // Sort ascending by z_mean: cam.axes[2] points up toward viewer, so belly paints first and dorsal back paints last.
        render_quads.sort_by(|a, b| a.z_mean.partial_cmp(&b.z_mean).unwrap_or(std::cmp::Ordering::Equal));

        bb_c0 = (bb_c0 - 1).max(0);
        bb_c1 = (bb_c1 + 1).min(cols as i32 - 1);
        bb_r0 = (bb_r0 - 1).max(0);
        bb_r1 = (bb_r1 + 1).min(rows as i32 - 1);

        if bb_c0 <= bb_c1 && bb_r0 <= bb_r1 {
            let cell_w = (bb_c1 - bb_c0 + 1) as usize;
            let cell_h = (bb_r1 - bb_r0 + 1) as usize;
            let bw = cell_w * SUB_SAMPLES;
            let bh = cell_h * SUB_SAMPLES;
            self.sub_rgba.resize(bw * bh, [0, 0, 0, 0]);
            self.sub_rgba.fill([0, 0, 0, 0]);
            let sub_f = SUB_SAMPLES as f32;

            for q in &render_quads {
                let p0 = [(q.s_pts[0][0] - bb_c0 as f32) * sub_f, (q.s_pts[0][1] - bb_r0 as f32) * sub_f];
                let p1 = [(q.s_pts[1][0] - bb_c0 as f32) * sub_f, (q.s_pts[1][1] - bb_r0 as f32) * sub_f];
                let p2 = [(q.s_pts[2][0] - bb_c0 as f32) * sub_f, (q.s_pts[2][1] - bb_r0 as f32) * sub_f];
                let p3 = [(q.s_pts[3][0] - bb_c0 as f32) * sub_f, (q.s_pts[3][1] - bb_r0 as f32) * sub_f];
                fill_tri_sub(&mut self.sub_rgba, bw, bh, p0, p1, p2, q.rgba);
                fill_tri_sub(&mut self.sub_rgba, bw, bh, p0, p2, p3, q.rgba);
            }

            // Paint visible side's gill slits and open/closed jaw contours onto the supersampled buffer
            let proj_sub = |w_pt: [f32; 3]| -> [f32; 2] {
                let sp = self.cam.project(w_pt);
                let sc = to_screen_col(sp[0]) + ptr_bias[0];
                let sr = to_screen_row(sp[1]) + ptr_bias[1];
                [(sc - bb_c0 as f32) * sub_f, (sr - bb_r0 as f32) * sub_f]
            };
            let length_s = self.cam.scale * scale_to_screen;
            for side in [-1.0f32, 1.0f32] {
                if side * rot[1][2] > 0.18 {
                    let gill_r = (length_s * 0.003 * sub_f).max(1.0) * 0.5;
                    for gill in [0.265f32, 0.283, 0.301, 0.319, 0.335] {
                        let s_pts = [
                            proj_sub(world_pt(cheek_pt(gill, 0.019, side), pose)),
                            proj_sub(world_pt(cheek_pt(gill - 0.007, -0.001, side), pose)),
                            proj_sub(world_pt(cheek_pt(gill - 0.005, -0.022, side), pose)),
                        ];
                        stroke_polyline_sub(&mut self.sub_rgba, bw, bh, &s_pts, gill_r, [29, 44, 50, 255]);
                    }

                    const REACH: [f32; 6] = [0.0, 0.22, 0.52, 0.81, 0.96, 1.0];
                    const UZ: [f32; 6] = [-0.017, -0.016, -0.011, -0.006, -0.002, -0.001];
                    const LZ: [f32; 6] = [-0.017, -0.021, -0.026, -0.028, -0.024, -0.014];
                    let mut roof = [[0.0f32; 2]; 6];
                    let mut jaw = [[0.0f32; 2]; 6];
                    let mut lower_edge = [[0.0f32; 2]; 6];
                    for k in 0..6 {
                        let rk = REACH[k];
                        let rx = 0.342 + 0.144 * rk;
                        let u_w = world_pt(cheek_pt(rx, UZ[k], side), pose);
                        let l_w = world_pt(cheek_pt(rx - 0.013 * rk * pose.gape, LZ[k], side), pose);
                        let rf_w = [
                            u_w[0] + 0.007 * pose.gape * rk * pose.up[0],
                            u_w[1] + 0.007 * pose.gape * rk * pose.up[1],
                            u_w[2] + 0.007 * pose.gape * rk * pose.up[2],
                        ];
                        let jw_w = [
                            l_w[0] - 0.014 * pose.gape * rk * pose.up[0],
                            l_w[1] - 0.014 * pose.gape * rk * pose.up[1],
                            l_w[2] - 0.014 * pose.gape * rk * pose.up[2],
                        ];
                        let le_w = [
                            jw_w[0] - 0.012 * rk * pose.up[0],
                            jw_w[1] - 0.012 * rk * pose.up[1],
                            jw_w[2] - 0.012 * rk * pose.up[2],
                        ];
                        roof[k] = proj_sub(rf_w);
                        jaw[k] = proj_sub(jw_w);
                        lower_edge[k] = proj_sub(le_w);
                    }
                    if pose.gape > 0.065 {
                        let mut opening = Vec::with_capacity(12);
                        opening.extend_from_slice(&roof);
                        for k in (0..6).rev() {
                            opening.push(jaw[k]);
                        }
                        fill_poly_sub(&mut self.sub_rgba, bw, bh, &opening, [4, 9, 13, 255]);
                        let mut lip_poly = Vec::with_capacity(12);
                        lip_poly.extend_from_slice(&jaw);
                        for k in (0..6).rev() {
                            lip_poly.push(lower_edge[k]);
                        }
                        fill_poly_sub(&mut self.sub_rgba, bw, bh, &lip_poly, [141, 156, 152, 255]);
                        stroke_polyline_sub(
                            &mut self.sub_rgba,
                            bw,
                            bh,
                            &jaw[1..],
                            (length_s * 0.0045 * sub_f).max(1.0) * 0.5,
                            [192, 199, 180, 255],
                        );
                        stroke_polyline_sub(
                            &mut self.sub_rgba,
                            bw,
                            bh,
                            &roof[1..],
                            (length_s * 0.0035 * sub_f).max(1.0) * 0.5,
                            [154, 170, 167, 255],
                        );
                    } else {
                        stroke_polyline_sub(&mut self.sub_rgba, bw, bh, &jaw, gill_r, [40, 49, 52, 255]);
                    }
                }
            }

            // Box-downsample 6x6 subpixels into per-cell coverage + average RGB
            let block_area = (SUB_SAMPLES * SUB_SAMPLES) as f32;
            for cr in 0..cell_h {
                for cc in 0..cell_w {
                    let mut sum_r = 0u32;
                    let mut sum_g = 0u32;
                    let mut sum_b = 0u32;
                    let mut cnt = 0u32;
                    let sy0 = cr * SUB_SAMPLES;
                    let sx0 = cc * SUB_SAMPLES;
                    for dy in 0..SUB_SAMPLES {
                        let row_off = (sy0 + dy) * bw + sx0;
                        for dx in 0..SUB_SAMPLES {
                            let px = self.sub_rgba[row_off + dx];
                            if px[3] > 0 {
                                sum_r += px[0] as u32;
                                sum_g += px[1] as u32;
                                sum_b += px[2] as u32;
                                cnt += 1;
                            }
                        }
                    }
                    if cnt > 0 {
                        let idx = (bb_r0 as usize + cr) * cols + (bb_c0 as usize + cc);
                        let inv_c = 1.0 / cnt as f32;
                        self.shark_cov[idx] = cnt as f32 / block_area;
                        self.shark_rgb[idx] = [sum_r as f32 * inv_c, sum_g as f32 * inv_c, sum_b as f32 * inv_c];
                    }
                }
            }
        }

        // 2. Separable Blurred Seabed Shadow (`sigma = 0.6`) + Ocean Environment
        let disp = self.cam.project([
            pose.center[0] + pose.center[2] * 0.19,
            pose.center[1] - pose.center[2] * 0.14,
            0.0,
        ]);
        let disp_c = to_screen_col(disp[0]) - to_screen_col(sh_center_scr[0]);
        let disp_r = to_screen_row(disp[1]) - to_screen_row(sh_center_scr[1]);
        let sample_cov = |c: f32, r: f32| -> f32 {
            let ic = c.floor() as i32;
            let ir = r.floor() as i32;
            let fx = c - ic as f32;
            let fy = r - ir as f32;
            let get = |x: i32, y: i32| -> f32 {
                if x >= 0 && x < cols as i32 && y >= 0 && y < rows as i32 {
                    self.shark_cov[y as usize * cols + x as usize]
                } else {
                    0.0
                }
            };
            let v00 = get(ic, ir);
            let v10 = get(ic + 1, ir);
            let v01 = get(ic, ir + 1);
            let v11 = get(ic + 1, ir + 1);
            (1.0 - fy) * ((1.0 - fx) * v00 + fx * v10) + fy * ((1.0 - fx) * v01 + fx * v11)
        };
        for r in 0..rows {
            for c in 0..cols {
                self.shadow_a[r * cols + c] = sample_cov(c as f32 - disp_c, r as f32 - disp_r);
            }
        }
        const KERNEL: [f32; 5] = [0.036, 0.242, 0.444, 0.242, 0.036];
        for r in 0..rows {
            let row_off = r * cols;
            for c in 0..cols {
                let mut acc = 0.0f32;
                for (ki, &kw) in KERNEL.iter().enumerate() {
                    let cc = (c as i32 + ki as i32 - 2).clamp(0, cols as i32 - 1) as usize;
                    acc += self.shadow_a[row_off + cc] * kw;
                }
                self.shadow_b[row_off + c] = acc;
            }
        }
        for r in 0..rows {
            for c in 0..cols {
                let mut acc = 0.0f32;
                for (ki, &kw) in KERNEL.iter().enumerate() {
                    let rr = (r as i32 + ki as i32 - 2).clamp(0, rows as i32 - 1) as usize;
                    acc += self.shadow_b[rr * cols + c] * kw;
                }
                self.shadow_a[r * cols + c] = (0.54 * acc).clamp(0.0, 1.0);
            }
        }

        let omega = TAU / LOOP_SECONDS;
        let drift_x = 0.06 * (omega * t).sin() / omega;
        let drift_y = (-0.03 * (omega * t).sin() + 0.05 * (1.0 - (omega * t).cos())) / omega;

        let (ocean_ink_b, ocean_ink_s, ocean_pap_b, ocean_pap_s) = self.ocean_palette();
        let shock_c = self.shock_origin[0] * cols as f32;
        let shock_r = self.shock_origin[1] * rows as f32;
        let shock_front = (1.0 - self.shock_timer) * (cols as f32 * 0.42);

        for r in 0..rows {
            let ref_y15 = to_ref_y15(r as f32 + 0.5);
            let vy = ((r as f32 + 0.5) / rows as f32 - 0.5) * 2.0;
            for c in 0..cols {
                let ref_x = to_ref_x(c as f32 + 0.5);
                let bank_ref_x = ((c as f32 + 0.5) / cols as f32) * 90.0;
                let vx = ((c as f32 + 0.5) / cols as f32 - 0.5) * 2.0;
                let (mut wx, mut wy) = self.cam.plane_zero(ref_x, ref_y15);
                let (bx, by) = self.cam.plane_zero(bank_ref_x, ref_y15);
                if self.pointer_active && self.params.mouse_interact {
                    let dx = (c as f32 / cols as f32) - self.pointer_norm[0];
                    let dy = (r as f32 / rows as f32) - self.pointer_norm[1];
                    let d2 = dx * dx + dy * dy;
                    let rip = (-d2 / 0.015).exp() * 0.08 * (20.0 * d2.sqrt() - t * 4.0).sin();
                    wx += rip;
                    wy += rip;
                }
                let macro_n = water_noise(1.15 * wx + 2.0, 1.15 * wy);
                let b_macro = water_noise(1.15 * bx + 2.0, 1.15 * by);
                let b_stone = 0.50 * b_macro + 0.30 * water_noise(3.6 * bx, 3.6 * by) + 0.20 * water_noise(11.0 * bx, 11.0 * by);
                let edge_b = 0.38 * (1.1 * bx + 0.5).sin() - 0.24 * bx + (b_stone - 0.5) * 0.85;
                let bank = smoothstep(-0.12, 0.15, by - edge_b - 0.57);
                let slope = 0.5 + 0.5 * (48.0 * wy + 12.0 * wx + 2.2 * (5.0 * wx + 3.0 * wy).sin()).sin();
                let sand = 0.48 + 0.18 * macro_n + 0.067 * slope;
                let shafts = 0.24 + 0.76 * water_noise(1.7 * wx + drift_x, 1.7 * wy + drift_y);
                let mut light = caustics(wx, wy, t) * shafts * 0.57;

                if self.shock_timer > 0.02 {
                    let dc = c as f32 - shock_c;
                    let dr = (r as f32 - shock_r) * 1.56;
                    let dist = dc.hypot(dr);
                    let wave = (-((dist - shock_front) / 4.0).powi(2)).exp() * self.shock_timer;
                    light += wave * 0.85;
                }

                let lit = sand + 0.52 * light;
                let mut ink = [
                    ocean_ink_b[0] + lit * ocean_ink_s[0],
                    ocean_ink_b[1] + lit * ocean_ink_s[1],
                    ocean_ink_b[2] + lit * ocean_ink_s[2],
                ];
                let reef = [
                    16.0 + b_stone * b_stone * 147.0,
                    32.0 + b_stone * b_stone * 166.0,
                    39.0 + b_stone * b_stone * 117.0,
                ];
                for k in 0..3 {
                    ink[k] += bank * (reef[k] - ink[k]);
                }
                let mut paper = [
                    ocean_pap_b[0] + (sand + 0.25 * light) * (1.0 - bank) * ocean_pap_s[0],
                    ocean_pap_b[1] + (sand + 0.25 * light) * (1.0 - bank) * ocean_pap_s[1],
                    ocean_pap_b[2] + (sand + 0.25 * light) * (1.0 - bank) * ocean_pap_s[2],
                ];
                let val = (0.20 + 0.33 * sand + 0.39 * light) * (1.0 - bank) + bank * (0.05 + 0.64 * b_stone.powf(1.4));

                let shadow = self.shadow_a[r * cols + c];
                for k in 0..3 {
                    ink[k] *= 1.0 - 0.77 * shadow;
                    paper[k] *= 1.0 - 0.73 * shadow;
                }
                let edge_v = (1.04 - 0.20 * vx * vx - 0.10 * vy * vy).clamp(0.72, 1.0);
                for k in 0..3 {
                    ink[k] *= edge_v;
                    paper[k] *= edge_v;
                }

                let paper_toned = [paper[0] * 0.65 + 1.0, paper[1] * 0.89 + 5.0, paper[2] * 0.97 + 12.0];
                let ink_toned = [
                    0.50 * (ink[0] * 0.53 + 3.0) + 0.50 * paper_toned[0],
                    0.50 * (ink[1] * 0.70 + 8.0) + 0.50 * paper_toned[1],
                    0.50 * (ink[2] * 0.79 + 14.0) + 0.50 * paper_toned[2],
                ];
                let gl = ramp_glyph(BG_RAMP, val * (1.0 - 0.66 * shadow));
                let ci = c as i32;
                let ri = r as i32;
                self.canvas.set_bg_rgb(
                    ci,
                    ri,
                    [
                        paper_toned[0].clamp(0.0, 255.0) as u8,
                        paper_toned[1].clamp(0.0, 255.0) as u8,
                        paper_toned[2].clamp(0.0, 255.0) as u8,
                    ],
                );
                self.canvas.set_cell(
                    ci,
                    ri,
                    gl,
                    [
                        ink_toned[0].clamp(0.0, 255.0) as u8,
                        ink_toned[1].clamp(0.0, 255.0) as u8,
                        ink_toned[2].clamp(0.0, 255.0) as u8,
                    ],
                    0,
                    0,
                );
            }
        }

        // Icon obstacle rects in grid cells
        let icon_cells: Vec<[f32; 4]> = if self.icons_vis {
            self.icons
                .iter()
                .map(|rc| {
                    [
                        (rc.x / self.css_w.max(1.0)) * cols as f32,
                        (rc.y / self.css_h.max(1.0)) * rows as f32,
                        ((rc.x + rc.w) / self.css_w.max(1.0)) * cols as f32,
                        ((rc.y + rc.h) / self.css_h.max(1.0)) * rows as f32,
                    ]
                })
                .collect()
        } else {
            Vec::new()
        };

        // 3. Silver Fish School + Penumbra Underside + Scales Gleam
        let ptr_sc = self.pointer_norm[0] * cols as f32;
        let ptr_sr = self.pointer_norm[1] * rows as f32;
        let mut order: Vec<usize> = (0..frame.fish_pos.len()).filter(|&i| frame.fish_alive[i]).collect();
        let f_depths: Vec<f32> = frame.fish_pos.iter().map(|&p| self.cam.depth(p)).collect();
        order.sort_by(|&a, &b| f_depths[a].partial_cmp(&f_depths[b]).unwrap_or(std::cmp::Ordering::Equal));
        let shark_depth = self.cam.depth(pose.center);

        let draw_school = |canvas: &mut AsciiCanvas, under_r: &mut Vec<u8>, front_pass: bool| {
            for idx in 0..len {
                under_r[idx] = canvas.cells[idx * 4];
            }
            for &i in &order {
                let d_rel = (f_depths[i] - shark_depth) >= 0.0;
                if d_rel != front_pass {
                    continue;
                }
                let p_xy = self.cam.project(frame.fish_pos[i]);
                let mut cx = to_screen_col(p_xy[0]);
                let mut cy15 = to_screen_row(p_xy[1]) * 1.5;

                let hv = unit3(frame.fish_vel[i]);
                let hd = [
                    dot3(hv, self.cam.axes[0]),
                    dot3(hv, self.cam.axes[1]),
                    dot3(hv, self.cam.axes[2]),
                ];
                let hl = hd[0].hypot(hd[1]).max(1e-4);
                let mut tang = [hd[0] / hl, hd[1] / hl];
                let length = (self.cam.scale * frame.fish_sizes[i] * hl * scale_to_screen).max(1.8);
                let sheen = (0.58 + 0.21 * hd[0].abs() + 0.10 * (frame.fish_phase[i] * 0.34).sin()).clamp(0.0, 1.0);

                if self.pointer_active && self.params.mouse_interact {
                    let dx = cx - ptr_sc;
                    let dy = cy15 / 1.5 - ptr_sr;
                    let dist = dx.hypot(dy);
                    let radius = if self.shock_timer > 0.1 { 18.0 } else { 10.0 };
                    if dist < radius && dist > 0.1 {
                        let push = (1.0 - dist / radius).powi(2) * (6.0 + 12.0 * self.shock_timer);
                        cx += (dx / dist) * push;
                        cy15 += (dy / dist) * push * 1.5;
                        tang = [dx / dist, dy / dist];
                    }
                }
                for ic in &icon_cells {
                    let icx = 0.5 * (ic[0] + ic[2]);
                    let icy = 0.5 * (ic[1] + ic[3]);
                    let hw = 0.5 * (ic[2] - ic[0]) + 2.0;
                    let hh = 0.5 * (ic[3] - ic[1]) + 2.0;
                    let dx = cx - icx;
                    let dy = cy15 / 1.5 - icy;
                    if dx.abs() < hw && dy.abs() < hh {
                        let push_y = if dy >= 0.0 { hh - dy } else { -(hh + dy) };
                        cy15 += push_y * 1.5;
                    }
                }

                let x0 = ((cx - length).floor() as i32).max(0);
                let x1 = ((cx + length).ceil() as i32).min(cols as i32 - 1);
                let y0 = (((cy15 - length) / 1.5).floor() as i32).max(0);
                let y1 = (((cy15 + length) / 1.5).ceil() as i32).min(rows as i32 - 1);

                for rr in y0..=y1 {
                    let py = (rr as f32 + 0.5) * 1.5;
                    for cc in x0..=x1 {
                        let px = cc as f32 + 0.5;
                        let dx = px - cx;
                        let dy = py - cy15;
                        let side = -tang[1] * dx + tang[0] * dy;
                        let along = (tang[0] * dx + tang[1] * dy) / length;
                        let radius = (length * 0.128).max(0.38);
                        let width = radius * (1.0 - ((along - 0.06) / 0.46).powi(2)).max(0.0).sqrt();
                        let tail = (-along - 0.24).max(0.0) * length * 0.72;
                        let wobble = frame.fish_phase[i].sin() * 0.30 * tail;
                        let dist = ((-0.57 - along).max(along - 0.52) * length)
                            .max((side - wobble).abs() - width.max(tail));
                        let idx = rr as usize * cols + cc as usize;
                        let p = idx * 4;

                        // Underside penumbra around sunlit belly (`cutoff_render.py:192-194`)
                        let border = (-2.5 * (dist - 0.63).powi(2)).exp() * (side / radius + 0.20).clamp(0.0, 1.0);
                        if border > 0.005 {
                            for k in 0..3 {
                                canvas.bg[p + k] = (canvas.bg[p + k] as f32 * (1.0 - 0.35 * border)).clamp(0.0, 255.0) as u8;
                                canvas.cells[p + k] = (canvas.cells[p + k] as f32 * (1.0 - 0.28 * border)).clamp(0.0, 255.0) as u8;
                            }
                        }

                        let cov = (0.48 - dist).clamp(0.0, 1.0);
                        let loc_sheen = (sheen + 0.13 * (1.0 - side.abs() / radius) - 0.12 * (-along).max(0.0)).clamp(0.0, 1.0);
                        let feather = smoothstep(0.13, 0.88, cov);
                        if feather > 0.005 {
                            let target_p = [
                                28.0 + loc_sheen * 134.0,
                                43.0 + loc_sheen * 132.0,
                                52.0 + loc_sheen * 115.0,
                            ];
                            for k in 0..3 {
                                let bgv = canvas.bg[p + k] as f32;
                                canvas.bg[p + k] = (bgv + feather * (target_p[k] - bgv)).clamp(0.0, 255.0) as u8;
                            }
                            canvas.bg[p + 3] = canvas.bg[p + 3].max((145.0 * feather).clamp(0.0, 220.0) as u8);
                        }

                        if cov <= 0.21 {
                            continue;
                        }
                        let back = ((side - wobble) / radius).clamp(-1.0, 1.0);
                        let stripe = 0.74 + 0.26 * (-((back + 0.10) / 0.50).powi(2)).exp();
                        let hue = [
                            ((61.0 + loc_sheen * 190.0) * stripe + 37.0).min(255.0),
                            ((110.0 + loc_sheen * 143.0) * stripe + 24.0).min(255.0),
                            ((129.0 + loc_sheen * 119.0) * stripe + 3.0).min(255.0),
                        ];
                        let gl = if length > 4.8 && along < -0.32 {
                            glyph_id(if tang[0] * tang[1] < 0.0 { '/' } else { '\\' })
                        } else {
                            ramp_glyph(FISH_RAMP, (0.22 + 0.76 * loc_sheen).clamp(0.0, 1.0))
                        };
                        canvas.cells[p] = hue[0] as u8;
                        canvas.cells[p + 1] = hue[1] as u8;
                        canvas.cells[p + 2] = hue[2] as u8;
                        canvas.cells[p + 3] = gl;
                        canvas.owner[idx] = 1;
                    }
                }
            }

            // Scales gleam on bright silver flanks (`cutoff_render.py:457-458`)
            for idx in 0..len {
                let p = idx * 4;
                let ir = canvas.cells[p];
                if ir > under_r[idx].saturating_add(10) && ir > 120 {
                    for k in 0..3 {
                        let ink_k = canvas.cells[p + k] as f32;
                        let pap_k = canvas.bg[p + k] as f32;
                        canvas.bg[p + k] = (pap_k + 0.42 * (ink_k - pap_k)).clamp(0.0, 255.0) as u8;
                    }
                    canvas.bg[p + 3] = canvas.bg[p + 3].max(195);
                }
            }
        };

        // Back half of school (behind shark depth)
        draw_school(&mut self.canvas, &mut self.under_r, false);

        // Composite Shark Mesh Cells onto Canvas (`cell_ink` + `grid`)
        for r in 0..rows {
            for c in 0..cols {
                let idx = r * cols + c;
                let cov = self.shark_cov[idx];
                if cov <= 0.12 {
                    continue;
                }
                let rgb = self.shark_rgb[idx];
                let lum = 0.23 * rgb[0] + 0.65 * rgb[1] + 0.12 * rgb[2];
                let strength = ((lum - 17.0) / 206.0).clamp(0.0, 1.0);
                let mut gl = ramp_glyph(SHARK_RAMP, 0.12 + 0.88 * strength.powf(0.82));
                if cov > 0.12 && cov < 0.67 && r > 0 && r + 1 < rows && c > 0 && c + 1 < cols {
                    let dx = (self.shark_cov[idx + 1] - self.shark_cov[idx - 1]) * 0.5;
                    let dy = (self.shark_cov[idx + cols] - self.shark_cov[idx - cols]) * 0.5;
                    if dx.abs() > 1.7 * dy.abs() {
                        gl = glyph_id('|');
                    } else if dy.abs() > 1.7 * dx.abs() {
                        gl = glyph_id('_');
                    } else if dx * dy > 0.0 {
                        gl = glyph_id('/');
                    } else {
                        gl = glyph_id('\\');
                    }
                }
                let weight = smoothstep(0.08, 0.91, cov);
                let p = idx * 4;
                for k in 0..3 {
                    let s_ink = (1.24 * rgb[k] + [9.0, 15.0, 19.0][k]).clamp(0.0, 255.0);
                    let s_pap = (rgb[k] * (0.09 + 0.37 * strength * strength)).clamp(0.0, 255.0);
                    let c_ink = self.canvas.cells[p + k] as f32;
                    let c_pap = self.canvas.bg[p + k] as f32;
                    self.canvas.cells[p + k] = (c_ink + weight * (s_ink - c_ink)).clamp(0.0, 255.0) as u8;
                    self.canvas.bg[p + k] = (c_pap + weight * (s_pap - c_pap)).clamp(0.0, 255.0) as u8;
                }
                if cov > 0.28 {
                    self.canvas.cells[p + 3] = gl;
                }
                self.canvas.bg[p + 3] = self.canvas.bg[p + 3].max((130.0 * strength * weight).clamp(0.0, 255.0) as u8);
            }
        }

        // Front half of school (in front of shark depth)
        draw_school(&mut self.canvas, &mut self.under_r, true);

        // Silver-print tonal shoulder LUT matching `silver_print_lut()` across BOTH ink and paper
        for px in self.canvas.cells.chunks_exact_mut(4) {
            for k in 0..3 {
                let l = px[k] as f32 / 255.0;
                let s = (l + 0.18 * smoothstep(0.18, 0.63, l) * (1.0 - l)).min(1.0);
                px[k] = (s * 255.0).round() as u8;
            }
        }
        for px in self.canvas.bg.chunks_exact_mut(4) {
            for k in 0..3 {
                let l = px[k] as f32 / 255.0;
                let s = (l + 0.18 * smoothstep(0.18, 0.63, l) * (1.0 - l)).min(1.0);
                px[k] = (s * 255.0).round() as u8;
            }
        }

        self.canvas.mark_dirty();
    }
}
