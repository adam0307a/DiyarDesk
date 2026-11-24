pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use serde_json::Value;
use shaders::*;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::ffi::c_void;

// =========================================================================
// 1. DETERMINISTIC 3D ORB ENGINE (1:1 with prototype/thinking-orbs-engine.js)
// =========================================================================

pub const DESIGN_IDS: [&str; 9] = [
    "working",    // 0 -> orbits
    "searching",  // 1 -> globe (default)
    "solving",    // 2 -> rubik
    "listening",  // 3 -> wave
    "connecting", // 4 -> web
    "weaving",    // 5 -> braid
    "composing",  // 6 -> ribbon
    "breathing",  // 7 -> ring (ribbon with faceOn=1)
    "shaping",    // 8 -> morph
];

#[inline]
pub fn frac(x: f64) -> f64 {
    x - x.floor()
}

#[inline]
fn lerp(a: f64, b: f64, f: f64) -> f64 {
    a + (b - a) * f
}

#[inline]
fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
pub fn hash_d(a: f64, b: f64) -> f64 {
    let h = (a * 12.9898 + b * 78.233).sin() * 43758.5453;
    h - h.floor()
}

pub fn vnoise(x: f64, y: f64) -> f64 {
    let xi = x.floor();
    let yi = y.floor();
    let mut fx = x - xi;
    let mut fy = y - yi;
    fx = fx * fx * (3.0 - 2.0 * fx);
    fy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash_d(xi, yi);
    let b = hash_d(xi + 1.0, yi);
    let c = hash_d(xi, yi + 1.0);
    let d = hash_d(xi + 1.0, yi + 1.0);
    a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy
}

fn golden_angle() -> f64 {
    PI * (3.0 - 5.0_f64.sqrt())
}

pub fn fib_dir(i: usize, n: f64) -> [f64; 3] {
    let y = 1.0 - (2.0 * (i as f64 + 0.5)) / n;
    let rad = (1.0 - y * y).max(0.0).sqrt();
    let a = i as f64 * golden_angle();
    [rad * a.cos(), y, rad * a.sin()]
}

#[inline]
fn angle_delta(a: f64, b: f64) -> f64 {
    (a - b).sin().atan2((a - b).cos())
}

#[inline]
fn radius_scale(size: f64, p: f64) -> f64 {
    (size / 300.0).powf(p)
}

#[inline]
fn below(n: f64) -> usize {
    if n > 0.0 {
        n.ceil() as usize
    } else {
        0
    }
}

#[inline]
fn through(n: f64) -> usize {
    if n >= 0.0 {
        n.floor() as usize + 1
    } else {
        0
    }
}

struct Projector {
    st: f64,
    ct: f64,
    sy: f64,
    cyw: f64,
    sr: f64,
    cr: f64,
    has_roll: bool,
    cx: f64,
    cy: f64,
    scale: f64,
}

impl Projector {
    fn new(yaw: f64, tilt: f64, cx: f64, cy: f64, scale: f64, roll: f64) -> Self {
        let has_roll = roll.abs() > 1e-12;
        Self {
            st: tilt.sin(),
            ct: tilt.cos(),
            sy: yaw.sin(),
            cyw: yaw.cos(),
            sr: if has_roll { roll.sin() } else { 0.0 },
            cr: if has_roll { roll.cos() } else { 1.0 },
            has_roll,
            cx,
            cy,
            scale,
        }
    }

    #[inline]
    fn project(&self, x: f64, y: f64, z: f64) -> [f64; 3] {
        let mut x1 = x * self.cyw + z * self.sy;
        let z1 = -x * self.sy + z * self.cyw;
        let mut y1 = y * self.ct - z1 * self.st;
        let z2 = y * self.st + z1 * self.ct;
        if self.has_roll {
            let rx = x1 * self.cr - y1 * self.sr;
            let ry = x1 * self.sr + y1 * self.cr;
            x1 = rx;
            y1 = ry;
        }
        [self.cx + x1 * self.scale, self.cy - y1 * self.scale, z2]
    }
}

#[derive(Clone, Copy)]
pub struct OrbDot {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub r: f64,
    pub white: f64,
    pub a: f64,
    pub boost: f64,
    idx: usize,
}

#[derive(Clone, Copy)]
pub struct OrbLine {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub white: f64,
    pub a: f64,
    pub w: f64,
}

pub struct OrbFrame {
    pub dots: Vec<OrbDot>,
    pub lines: Vec<OrbLine>,
}

#[derive(Clone, Copy)]
struct ResolvedOpts {
    design: usize,
    speed: f64,
    lat_rings: f64,
    lon_density: f64,
    rings: f64,
    orbit_n: f64,
    ghost_n: f64,
    particles: f64,
    move_count: f64,
    node_n: f64,
    thr: f64,
    signals: f64,
    strand_n: f64,
    turns: f64,
    lanes: f64,
    segs: f64,
    face_on: bool,
    spin: f64,
    band_mul: f64,
    wob_mul: f64,
    scan_mul: f64,
    dim_base: f64,
    spread: f64,
    r_base: f64,
    r_depth: f64,
    r_boost: f64,
    r_active: f64,
    r_dot: f64,
    icon_d: f64,
    ghost_r: f64,
    ghost_a: f64,
    part_r: f64,
    part_r_depth: f64,
    node_r: f64,
    node_r_depth: f64,
    line_w: f64,
    ink_far: f64,
    ink_span: f64,
    rs_pow: f64,
    r_min: f64,
}

fn resolve_design(design: usize, density_mul: f64, size_mul: f64) -> ResolvedOpts {
    let mut o = ResolvedOpts {
        design: design.min(8),
        speed: 1.75,
        lat_rings: 0.0,
        lon_density: 0.0,
        rings: 0.0,
        orbit_n: 0.0,
        ghost_n: 0.0,
        particles: 3.0,
        move_count: 14.0,
        node_n: 0.0,
        thr: 0.72,
        signals: 0.0,
        strand_n: 0.0,
        turns: 3.0,
        lanes: 0.0,
        segs: 0.0,
        face_on: false,
        spin: 1.0,
        band_mul: 1.0,
        wob_mul: 1.0,
        scan_mul: 1.0,
        dim_base: 1.0,
        spread: 1.0,
        r_base: 0.6,
        r_depth: 1.7,
        r_boost: 1.0,
        r_active: 0.3,
        r_dot: 0.021,
        icon_d: 1.0,
        ghost_r: 0.9,
        ghost_a: 0.5,
        part_r: 1.2,
        part_r_depth: 1.6,
        node_r: 1.4,
        node_r_depth: 1.8,
        line_w: 0.8,
        ink_far: 0.62,
        ink_span: 0.54,
        rs_pow: 0.6,
        r_min: 0.3,
    };

    let (preset_count, preset_size) = match o.design {
        0 => {
            // working / orbits
            o.speed = 1.45;
            o.orbit_n = 12.0;
            o.ghost_n = 40.0;
            o.ghost_r = 0.9;
            o.ghost_a = 0.5;
            o.particles = 3.0;
            o.part_r = 1.2;
            o.part_r_depth = 1.6;
            (1.0, 1.05)
        }
        1 => {
            // searching / globe
            o.speed = 1.75;
            o.lat_rings = 17.0;
            o.lon_density = 44.0;
            (1.0, 1.05)
        }
        2 => {
            // solving / rubik
            o.speed = 1.55;
            o.lat_rings = 15.0;
            o.lon_density = 40.0;
            o.move_count = 14.0;
            (1.0, 1.05)
        }
        3 => {
            // listening / wave
            o.speed = 3.4;
            o.rings = 15.0;
            o.lon_density = 40.0;
            (1.0, 1.02)
        }
        4 => {
            // connecting / web
            o.speed = 2.6;
            o.node_n = 30.0;
            o.thr = 0.72;
            o.signals = 5.0;
            o.node_r = 1.4;
            o.node_r_depth = 1.8;
            o.line_w = 0.8;
            (1.35, 0.95)
        }
        5 => {
            // weaving / braid
            o.speed = 1.45;
            o.strand_n = 52.0;
            o.turns = 3.0;
            o.ghost_n = 150.0;
            o.r_base = 1.2;
            o.r_depth = 1.8;
            (0.95, 1.0)
        }
        6 => {
            // composing / ribbon
            o.speed = 2.0;
            o.lanes = 5.0;
            o.segs = 88.0;
            o.ghost_n = 150.0;
            o.r_base = 1.1;
            o.r_depth = 1.7;
            (0.55, 0.95)
        }
        7 => {
            // breathing / ring
            o.speed = 2.65;
            o.lanes = 5.0;
            o.segs = 88.0;
            o.ghost_n = 0.0;
            o.face_on = true;
            o.r_base = 1.1;
            o.r_depth = 1.7;
            (0.55, 0.98)
        }
        _ => {
            // shaping / morph
            o.speed = 2.0;
            o.r_dot = 0.021;
            o.icon_d = 1.0;
            o.r_min = 0.25;
            (1.15, 0.34)
        }
    };

    let total_count = preset_count * density_mul;
    let total_size = preset_size * size_mul;

    if (total_count - 1.0).abs() > 1e-12 {
        let rt = total_count.max(0.05).sqrt();
        if o.lat_rings > 0.0 && o.lon_density > 0.0 {
            o.lat_rings = (o.lat_rings * rt).round().max(2.0);
            o.lon_density = (o.lon_density * rt).round().max(2.0);
        } else if o.rings > 0.0 && o.lon_density > 0.0 {
            o.rings = (o.rings * rt).round().max(2.0);
            o.lon_density = (o.lon_density * rt).round().max(2.0);
        } else if o.lanes > 0.0 && o.segs > 0.0 {
            o.lanes = (o.lanes * rt).round().max(2.0);
            o.segs = (o.segs * rt).round().max(2.0);
        }
        if o.orbit_n > 0.0 { o.orbit_n = (o.orbit_n * total_count).round().max(1.0); }
        if o.ghost_n > 0.0 { o.ghost_n = (o.ghost_n * total_count).round().max(1.0); }
        if o.node_n > 0.0 { o.node_n = (o.node_n * total_count).round().max(1.0); }
        if o.strand_n > 0.0 { o.strand_n = (o.strand_n * total_count).round().max(1.0); }
        if o.signals > 0.0 { o.signals = (o.signals * total_count).round().max(1.0); }
        if o.design == 8 {
            o.icon_d = (o.icon_d * total_count).max(0.08);
        }
    }

    if (total_size - 1.0).abs() > 1e-12 {
        o.r_base *= total_size;
        o.r_depth *= total_size;
        o.r_active *= total_size;
        o.r_dot *= total_size;
        o.ghost_r *= total_size;
        o.part_r *= total_size;
        o.part_r_depth *= total_size;
        o.node_r *= total_size;
        o.node_r_depth *= total_size;
    }

    // Apply preset.extra after scaling (1:1 with JS)
    match o.design {
        1 => {
            o.scan_mul = 4.08;
            o.dim_base = 0.42;
        }
        6 => {
            o.spin = 0.0;
            o.band_mul = 3.9;
            o.wob_mul = 1.0;
        }
        7 => {
            o.spin = 0.0;
            o.band_mul = 3.627;
            o.wob_mul = 0.368;
        }
        8 => {
            o.spread = 1.45;
        }
        _ => {}
    }

    o
}

fn finalize(raw_dots: Vec<OrbDot>, raw_lines: Vec<OrbLine>, r_min: f64) -> OrbFrame {
    let mut visible: Vec<OrbDot> = Vec::with_capacity(raw_dots.len());
    for (i, mut d) in raw_dots.into_iter().enumerate() {
        if d.a < 0.02 {
            continue;
        }
        if d.r < r_min {
            d.r = r_min;
        }
        d.idx = i;
        visible.push(d);
    }
    visible.sort_by(|a, b| a.z.total_cmp(&b.z).then_with(|| a.idx.cmp(&b.idx)));
    let vis_lines: Vec<OrbLine> = raw_lines.into_iter().filter(|l| l.a >= 0.02).collect();
    OrbFrame {
        dots: visible,
        lines: vis_lines,
    }
}

fn eval_orb_frame(o: &ResolvedOpts, size: f64, t: f64, hover_yaw: f64, hover_tilt: f64, hover_roll: f64) -> OrbFrame {
    match o.design {
        0 => frame_orbits(o, size, t, hover_yaw, hover_tilt, hover_roll),
        1 => frame_globe(o, size, t, hover_yaw, hover_tilt, hover_roll),
        2 => frame_rubik(o, size, t, hover_yaw, hover_tilt, hover_roll),
        3 => frame_wave(o, size, t, hover_yaw, hover_tilt, hover_roll),
        4 => frame_web(o, size, t, hover_yaw, hover_tilt, hover_roll),
        5 => frame_braid(o, size, t, hover_yaw, hover_tilt, hover_roll),
        6 | 7 => frame_ribbon(o, size, t, hover_yaw, hover_tilt, hover_roll),
        _ => frame_morph(o, size, t, hover_yaw, hover_tilt),
    }
}

fn frame_orbits(o: &ResolvedOpts, size: f64, t: f64, hy: f64, ht: f64, hr: f64) -> OrbFrame {
    let cx = size * 0.5;
    let cy = size * 0.5;
    let r_big = (size * 0.5) * 0.82;
    let pt = Projector::new(t * 0.12 + hy, 0.3 + ht, cx, cy, 1.0, hr);
    let rs = radius_scale(size, o.rs_pow);
    let n_orbits = below(o.orbit_n);
    let n_ghost = below(o.ghost_n);
    let n_part = below(o.particles);
    let mut dots = Vec::with_capacity(n_orbits * (n_ghost + n_part));

    for orb in 0..n_orbits {
        let h1 = hash_d(orb as f64, 1.7);
        let h2 = hash_d(orb as f64, 5.2);
        let h3 = hash_d(orb as f64, 8.9);
        let ro = r_big * (0.45 + 0.52 * h1);
        let th = h1 * 2.0 * PI;
        let phi = (2.0 * h2 - 1.0).acos();
        let nx = phi.sin() * th.cos();
        let ny = phi.cos();
        let nz = phi.sin() * th.sin();
        let mut ux = -ny;
        let mut uy = nx;
        let uz = 0.0;
        let ul = (ux * ux + uy * uy).sqrt().max(1e-6);
        ux /= ul;
        uy /= ul;
        let vx = ny * uz - nz * uy;
        let vy = nz * ux - nx * uz;
        let vz = nx * uy - ny * ux;
        let speed = (0.25 + 0.55 * h3) * if h3 > 0.5 { 1.0 } else { -1.0 };

        for k in 0..n_ghost {
            let a = (k as f64 / o.ghost_n) * 2.0 * PI;
            let (ca, sa) = (a.cos(), a.sin());
            let p = pt.project((ux * ca + vx * sa) * ro, (uy * ca + vy * sa) * ro, (uz * ca + vz * sa) * ro);
            let depth = (p[2] / ro + 1.0) * 0.5;
            dots.push(OrbDot {
                x: p[0],
                y: p[1],
                z: p[2],
                r: o.ghost_r * rs,
                white: 0.72,
                a: o.ghost_a * (0.4 + 0.6 * depth),
                boost: 0.0,
                idx: 0,
            });
        }
        for m in 0..n_part {
            let ap = t * speed + (m as f64 / o.particles) * 2.0 * PI + h2 * 6.0;
            let (cap, sap) = (ap.cos(), ap.sin());
            let pp = pt.project((ux * cap + vx * sap) * ro, (uy * cap + vy * sap) * ro, (uz * cap + vz * sap) * ro);
            let pdepth = (pp[2] / ro + 1.0) * 0.5;
            dots.push(OrbDot {
                x: pp[0],
                y: pp[1],
                z: pp[2],
                r: (o.part_r + o.part_r_depth * pdepth) * rs,
                white: 0.3 - 0.22 * pdepth,
                a: 1.0,
                boost: 0.0,
                idx: 0,
            });
        }
    }
    finalize(dots, Vec::new(), o.r_min)
}

fn frame_globe(o: &ResolvedOpts, size: f64, t: f64, hy: f64, ht: f64, hr: f64) -> OrbFrame {
    let spin = 0.5;
    let cx = size * 0.5;
    let cy = size * 0.5;
    let radius = (size * 0.5) * 0.82;
    let tilt = 0.4 + 0.06 * (t * 0.35).sin() + ht;
    let pt = Projector::new(t * spin + hy, tilt, cx, cy, radius, hr);
    let scan = t * (spin + (1.7 - spin) * o.scan_mul);
    let rs = radius_scale(size, o.rs_pow);

    let n_lat = through(o.lat_rings);
    let mut dots = Vec::with_capacity(n_lat * 32);
    for li in 0..n_lat {
        let lat = -PI * 0.5 + (li as f64 / o.lat_rings) * PI;
        let cos_lat = lat.cos();
        let sin_lat = lat.sin();
        let lon_count = (cos_lat.abs() * o.lon_density).round().max(1.0) as usize;
        for lj in 0..lon_count {
            let lon = (lj as f64 / lon_count as f64) * 2.0 * PI;
            let p = pt.project(cos_lat * lon.cos(), sin_lat, cos_lat * lon.sin());
            let z = p[2];
            let depth = (z + 1.0) * 0.5;
            let d = angle_delta(lon + t * spin, scan);
            let boost = (-(d * d) / 0.18).exp() * z.max(0.0);
            dots.push(OrbDot {
                x: p[0],
                y: p[1],
                z,
                r: (o.r_base + o.r_depth * depth + o.r_boost * boost) * rs,
                white: o.ink_far - o.ink_span * depth,
                a: o.dim_base + (1.0 - o.dim_base) * boost.min(1.0),
                boost,
                idx: 0,
            });
        }
    }
    finalize(dots, Vec::new(), o.r_min)
}

#[derive(Clone, Copy)]
struct RubikMove {
    axis: usize,
    lo: f64,
    hi: f64,
    ang: f64,
}

fn rubik_moves(count: usize) -> Vec<RubikMove> {
    let mut list = Vec::with_capacity(count);
    for i in 0..count {
        let axis = ((hash_d(i as f64, 2.3) * 3.0).floor() as usize).min(2);
        let lo = -1.0 + 0.5 * ((hash_d(i as f64, 5.9) * 4.0).floor().min(3.0));
        let dir = if hash_d(i as f64, 7.7) < 0.5 { 1.0 } else { -1.0 };
        list.push(RubikMove {
            axis,
            lo,
            hi: lo + 0.5,
            ang: dir * PI * 0.5,
        });
    }
    list
}

fn solve_cycle(time: f64, count: usize, slot_dur: f64, rest: f64) -> (Vec<f64>, i32) {
    let cyc = 2.0 * count as f64 * slot_dur + rest;
    let mut tc = time % cyc;
    if tc < 0.0 {
        tc += cyc;
    }
    let mut amount = vec![0.0; count];
    let mut active = -1i32;
    if tc < 2.0 * count as f64 * slot_dur {
        let slot = (tc / slot_dur).floor() as usize;
        let p = (tc - slot as f64 * slot_dur) / slot_dur;
        let cl = (p / 0.7).min(1.0);
        let ep = 1.0 - (1.0 - cl).powi(3);
        if slot < count {
            for j in 0..slot {
                amount[j] = 1.0;
            }
            amount[slot] = ep;
            active = slot as i32;
        } else {
            let u = 2 * count - 1 - slot;
            for k in 0..u {
                amount[k] = 1.0;
            }
            amount[u] = 1.0 - ep;
            active = u as i32;
        }
    }
    (amount, active)
}

fn apply_moves(mut x: f64, mut y: f64, mut z: f64, moves: &[RubikMove], amount: &[f64], active: i32) -> (f64, f64, f64, bool) {
    let mut in_active = false;
    for (i, mv) in moves.iter().enumerate() {
        if amount[i] <= 0.0 {
            continue;
        }
        let coord = match mv.axis {
            0 => x,
            1 => y,
            _ => z,
        };
        if coord < mv.lo || coord >= mv.hi {
            continue;
        }
        if i as i32 == active {
            in_active = true;
        }
        let a = mv.ang * amount[i];
        let (ca, sa) = (a.cos(), a.sin());
        if mv.axis == 0 {
            let y2 = y * ca - z * sa;
            z = y * sa + z * ca;
            y = y2;
        } else if mv.axis == 1 {
            let x2 = x * ca + z * sa;
            z = -x * sa + z * ca;
            x = x2;
        } else {
            let x3 = x * ca - y * sa;
            y = x * sa + y * ca;
            x = x3;
        }
    }
    (x, y, z, in_active)
}

fn frame_rubik(o: &ResolvedOpts, size: f64, t: f64, hy: f64, ht: f64, hr: f64) -> OrbFrame {
    let cx = size * 0.5;
    let cy = size * 0.5;
    let r_big = (size * 0.5) * 0.82;
    let pt = Projector::new(t * 0.55 + hy, 0.35 + 0.1 * (t * 0.9).sin() + ht, cx, cy, r_big, hr);
    let rs = radius_scale(size, o.rs_pow);
    let move_count = (o.move_count.floor() as usize).max(1);
    let moves = rubik_moves(move_count);
    let (amount, active) = solve_cycle(t, move_count, 0.42, 1.2);

    let n_lat = through(o.lat_rings);
    let mut dots = Vec::with_capacity(n_lat * 28);
    for li in 0..n_lat {
        let lat = -PI * 0.5 + (li as f64 / o.lat_rings) * PI;
        let cos_lat = lat.cos();
        let sin_lat = lat.sin();
        let lon_count = (cos_lat.abs() * o.lon_density).round().max(1.0) as usize;
        for lj in 0..lon_count {
            let lon = (lj as f64 / lon_count as f64) * 2.0 * PI;
            let (mx, my, mz, in_act) = apply_moves(cos_lat * lon.cos(), sin_lat, cos_lat * lon.sin(), &moves, &amount, active);
            let p = pt.project(mx, my, mz);
            let zr = p[2];
            let depth = (zr + 1.0) * 0.5;
            dots.push(OrbDot {
                x: p[0],
                y: p[1],
                z: zr,
                r: (o.r_base + o.r_depth * depth + if in_act { o.r_active } else { 0.0 }) * rs,
                white: o.ink_far - o.ink_span * depth - if in_act { 0.14 } else { 0.0 },
                a: 1.0,
                boost: if in_act { 0.65 } else { 0.0 },
                idx: 0,
            });
        }
    }
    finalize(dots, Vec::new(), o.r_min)
}

fn frame_wave(o: &ResolvedOpts, size: f64, t: f64, hy: f64, ht: f64, hr: f64) -> OrbFrame {
    let cx = size * 0.5;
    let cy = size * 0.5;
    let r_big = (size * 0.5) * 0.874;
    let pt = Projector::new(t * 0.18 + hy, 0.38 + ht, cx, cy, 1.0, hr);
    let rs = radius_scale(size, o.rs_pow);
    let n_rings = through(o.rings);
    let mut dots = Vec::with_capacity(n_rings * 28);

    for ri in 0..n_rings {
        let lat = -PI * 0.5 + (ri as f64 / o.rings) * PI;
        let cos_lat = lat.cos();
        let sin_lat = lat.sin();
        let w = 0.62 * (t * 2.1 - ri as f64 * 0.52).sin() + 0.38 * (t * 1.27 + ri as f64 * 0.83).sin();
        let rr = r_big * (0.88 + 0.105 * w);
        let lon_count = (cos_lat.abs() * o.lon_density).round().max(1.0) as usize;
        let crest = w.max(0.0);
        for lj in 0..lon_count {
            let lon = (lj as f64 / lon_count as f64) * 2.0 * PI;
            let p = pt.project(cos_lat * lon.cos() * rr, sin_lat * rr, cos_lat * lon.sin() * rr);
            let depth = (p[2] / r_big + 1.0) * 0.5;
            dots.push(OrbDot {
                x: p[0],
                y: p[1],
                z: p[2],
                r: (o.r_base + o.r_depth * depth) * (1.0 + 0.4 * crest) * rs,
                white: 0.66 - 0.56 * depth - 0.1 * crest,
                a: 1.0,
                boost: crest * 0.55,
                idx: 0,
            });
        }
    }
    finalize(dots, Vec::new(), o.r_min)
}

fn frame_web(o: &ResolvedOpts, size: f64, t: f64, hy: f64, ht: f64, hr: f64) -> OrbFrame {
    let cx = size * 0.5;
    let cy = size * 0.5;
    let r_big = (size * 0.5) * 0.8 * o.spread;
    let pt = Projector::new(t * 0.12 + hy, 0.32 + ht, cx, cy, r_big, hr);
    let rs = radius_scale(size, o.rs_pow);
    let n_nodes = below(o.node_n);

    let mut nodes = Vec::with_capacity(n_nodes);
    let mut proj = Vec::with_capacity(n_nodes);
    for i in 0..n_nodes {
        let d = fib_dir(i, o.node_n);
        let x = d[0] + 0.3 * (vnoise(i as f64 * 0.31 + 9.0, t * 0.24) - 0.5) * 2.0;
        let y = d[1] + 0.3 * (vnoise(i as f64 * 0.53 + 27.0, t * 0.21) - 0.5) * 2.0;
        let z = d[2] + 0.3 * (vnoise(i as f64 * 0.77 + 55.0, t * 0.27) - 0.5) * 2.0;
        let l = (x * x + y * y + z * z).sqrt().max(1e-6);
        let n = [x / l, y / l, z / l];
        proj.push(pt.project(n[0], n[1], n[2]));
        nodes.push(n);
    }

    let mut lines = Vec::new();
    for a in 0..n_nodes {
        for b in (a + 1)..n_nodes {
            let dx = nodes[a][0] - nodes[b][0];
            let dy = nodes[a][1] - nodes[b][1];
            let dz = nodes[a][2] - nodes[b][2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist >= o.thr {
                continue;
            }
            let p1 = proj[a];
            let p2 = proj[b];
            let ldepth = ((p1[2] + p2[2]) * 0.5 + 1.0) * 0.5;
            lines.push(OrbLine {
                x1: p1[0],
                y1: p1[1],
                x2: p2[0],
                y2: p2[1],
                white: 0.42,
                a: (1.0 - dist / o.thr) * (0.3 + 0.55 * ldepth),
                w: (o.line_w * rs).max(0.6),
            });
        }
    }

    let n_sig = below(o.signals);
    let mut dots = Vec::with_capacity(n_nodes + n_sig);
    for k in 0..n_nodes {
        let pk = proj[k];
        let depth = (pk[2] + 1.0) * 0.5;
        let pulse = 1.0 + 0.25 * (t * 1.4 + k as f64 * 2.7).sin();
        dots.push(OrbDot {
            x: pk[0],
            y: pk[1],
            z: pk[2],
            r: (o.node_r + o.node_r_depth * depth) * pulse * rs,
            white: 0.55 - 0.45 * depth,
            a: 1.0,
            boost: 0.0,
            idx: 0,
        });
    }

    for s in 0..n_sig {
        let val = t * 0.55 + s as f64 * 7.31;
        let seg = val.floor();
        let sa = (hash_d(seg, s as f64 * 3.1 + 1.7) * o.node_n).floor() as usize;
        let sb = (hash_d(seg, s as f64 * 5.7 + 4.2) * o.node_n).floor() as usize;
        if sa == sb || sa >= n_nodes || sb >= n_nodes {
            continue;
        }
        let f = frac(val);
        let sx = lerp(nodes[sa][0], nodes[sb][0], f);
        let sy = lerp(nodes[sa][1], nodes[sb][1], f);
        let sz = lerp(nodes[sa][2], nodes[sb][2], f);
        let sl = (sx * sx + sy * sy + sz * sz).sqrt().max(1e-6);
        let ps = pt.project(sx / sl, sy / sl, sz / sl);
        let sdepth = (ps[2] + 1.0) * 0.5;
        dots.push(OrbDot {
            x: ps[0],
            y: ps[1],
            z: ps[2],
            r: (o.node_r * 1.5 + o.node_r_depth * sdepth) * rs,
            white: 0.05,
            a: 0.5 + 0.5 * sdepth,
            boost: 0.9,
            idx: 0,
        });
    }

    finalize(dots, lines, o.r_min)
}

fn frame_braid(o: &ResolvedOpts, size: f64, t: f64, hy: f64, ht: f64, hr: f64) -> OrbFrame {
    let cx = size * 0.5;
    let cy = size * 0.5;
    let r_big = (size * 0.5) * 0.76;
    let pt = Projector::new(t * 0.4 + hy, 0.3 + ht, cx, cy, 1.0, hr);
    let rs = radius_scale(size, o.rs_pow);
    let n_ghost = below(o.ghost_n);
    let n_strand = below(o.strand_n);
    let mut dots = Vec::with_capacity(n_ghost + 3 * n_strand);

    for i in 0..n_ghost {
        let d = fib_dir(i, o.ghost_n);
        let pg = pt.project(d[0] * r_big, d[1] * r_big, d[2] * r_big);
        let gdepth = (pg[2] / r_big + 1.0) * 0.5;
        dots.push(OrbDot {
            x: pg[0],
            y: pg[1],
            z: pg[2],
            r: 0.8 * rs,
            white: 0.78,
            a: 0.1 + 0.22 * gdepth,
            boost: 0.0,
            idx: 0,
        });
    }

    for s in 0..3 {
        let phase = (s as f64 / 3.0) * 2.0 * PI;
        for k in 0..n_strand {
            let u = (frac(k as f64 / o.strand_n + t * 0.045) * 2.0 - 1.0) * 0.96;
            let surf = (1.0 - u * u).max(0.0).sqrt();
            let end_fade = ((1.0 - u.abs()) / 0.1).min(1.0);
            let a = u * PI * o.turns + phase;
            let weave = 1.0 + 0.075 * (u * PI * o.turns * 2.0 + phase * 2.0 + t * 0.8).sin();
            let rr = surf * r_big * weave;
            let ps = pt.project(a.cos() * rr, u * r_big * weave, a.sin() * rr);
            let depth = (ps[2] / r_big + 1.0) * 0.5;
            dots.push(OrbDot {
                x: ps[0],
                y: ps[1],
                z: ps[2],
                r: (o.r_base + o.r_depth * depth) * rs,
                white: 0.55 - 0.45 * depth,
                a: end_fade * (0.45 + 0.55 * depth),
                boost: 0.0,
                idx: 0,
            });
        }
    }
    finalize(dots, Vec::new(), o.r_min)
}

fn frame_ribbon(o: &ResolvedOpts, size: f64, t: f64, hy: f64, ht: f64, hr: f64) -> OrbFrame {
    let cx = size * 0.5;
    let cy = size * 0.5;
    let r_big = (size * 0.5) * 0.78;
    let cam_tilt = 0.3;
    let hm = if o.face_on { 0.65 } else { 1.0 };
    let pt = Projector::new(t * 0.1 * o.spin + hy * hm, cam_tilt + ht * hm, cx, cy, 1.0, hr);
    let rs = radius_scale(size, o.rs_pow);

    let n_ghost = below(o.ghost_n);
    let lanes = ((o.lanes * o.band_mul).round() as usize).max(1);
    let n_segs = below(o.segs);
    let mut dots = Vec::with_capacity(n_ghost + lanes * n_segs);

    for i in 0..n_ghost {
        let d = fib_dir(i, o.ghost_n);
        let pg = pt.project(d[0] * r_big, d[1] * r_big, d[2] * r_big);
        let gdepth = (pg[2] / r_big + 1.0) * 0.5;
        dots.push(OrbDot {
            x: pg[0],
            y: pg[1],
            z: pg[2],
            r: 0.8 * rs,
            white: 0.78,
            a: 0.1 + 0.22 * gdepth,
            boost: 0.0,
            idx: 0,
        });
    }

    let ya = t * 0.24 * o.spin;
    let ta = if o.face_on {
        -cam_tilt
    } else {
        0.55 + 0.3 * (t * 0.18).sin() * o.spin
    };
    let (ux, uy, uz) = (ya.cos(), 0.0, ya.sin());
    let (vx, vy, vz) = (-uz * ta.sin(), ta.cos(), ux * ta.sin());
    let (nx, ny, nz) = (uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx);

    let wob_amp = 0.23 * o.wob_mul;
    let base_r = if o.face_on { r_big / (1.0 + 0.85 * wob_amp) } else { r_big };
    let mid = (lanes as f64 - 1.0) * 0.5;

    for w in 0..lanes {
        let lane_off = (w as f64 - mid) * 0.075;
        let edge = (w as f64 - mid).abs() / mid.max(1.0);
        for k in 0..n_segs {
            let a = (k as f64 / o.segs) * 2.0 * PI;
            let (ca, sa) = (a.cos(), a.sin());
            let wob = (0.16 * (a * 3.0 - t * 1.7 + w as f64 * 0.22).sin()
                + 0.07 * (a * 5.0 + t * 1.1).sin())
                * o.wob_mul;
            let radial = if o.face_on { 1.0 + wob } else { 1.0 };
            let off = if o.face_on { lane_off } else { lane_off + wob };
            let x = ux * ca + vx * sa + nx * off;
            let y = uy * ca + vy * sa + ny * off;
            let z = uz * ca + vz * sa + nz * off;
            let l = (x * x + y * y + z * z).sqrt();
            let inv_l = if l > 0.0 { 1.0 / l } else { 1.0 };
            let rr = base_r * radial;
            let pr = pt.project(x * inv_l * rr, y * inv_l * rr, z * inv_l * rr);
            let depth = (pr[2] / r_big + 1.0) * 0.5;
            dots.push(OrbDot {
                x: pr[0],
                y: pr[1],
                z: pr[2],
                r: (o.r_base + o.r_depth * depth) * (1.0 - 0.25 * edge) * rs,
                white: 0.52 - 0.44 * depth + 0.18 * edge,
                a: 0.4 + 0.6 * depth,
                boost: 0.0,
                idx: 0,
            });
        }
    }
    finalize(dots, Vec::new(), o.r_min)
}

fn poly_path_at(verts: &[[f64; 2]], f: f64) -> [f64; 2] {
    let n = verts.len();
    let mut lengths = [0.0f64; 8];
    let mut total = 0.0;
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        lengths[i] = l;
        total += l;
    }
    let mut target = f * total;
    let mut i = 0usize;
    while target > lengths[i] && i < n - 1 {
        target -= lengths[i];
        i += 1;
    }
    let a = verts[i];
    let b = verts[(i + 1) % n];
    let ff = if lengths[i] != 0.0 { (target / lengths[i]).min(1.0) } else { 0.0 };
    [a[0] + (b[0] - a[0]) * ff, a[1] + (b[1] - a[1]) * ff]
}

fn shape_point(shape: usize, f: f64) -> [f64; 2] {
    if shape == 0 {
        let a = -PI * 0.5 + f * 2.0 * PI;
        [a.cos() * 0.24, a.sin() * 0.24]
    } else if shape == 1 {
        poly_path_at(&[[0.0, -0.26], [0.24, 0.16], [-0.24, 0.16]], f)
    } else {
        poly_path_at(&[[0.0, -0.2], [0.2, -0.2], [0.2, 0.2], [-0.2, 0.2], [-0.2, -0.2]], f)
    }
}

fn frame_morph(o: &ResolvedOpts, size: f64, t: f64, hy: f64, ht: f64) -> OrbFrame {
    const K: usize = 3;
    const HOLD: f64 = 1.4;
    const MORPH_T: f64 = 0.9;
    let seg_dur = HOLD + MORPH_T;
    let mut tc = t % (seg_dur * K as f64);
    if tc < 0.0 {
        tc += seg_dur * K as f64;
    }
    let k = ((tc / seg_dur).floor() as usize).min(K - 1);
    let local = tc - k as f64 * seg_dur;
    let m = if local > HOLD {
        let mx = (local - HOLD) / MORPH_T;
        mx * mx * (3.0 - 2.0 * mx)
    } else {
        0.0
    };

    const M: usize = 160;
    let mut pts = [[0.0f64; 2]; M];
    for i in 0..M {
        let f = i as f64 / M as f64;
        let a = shape_point(k, f);
        let b = shape_point((k + 1) % K, f);
        pts[i] = [
            (a[0] + (b[0] - a[0]) * m) * o.spread,
            (a[1] + (b[1] - a[1]) * m) * o.spread,
        ];
    }
    let mut l_arr = [0.0f64; M];
    let mut total = 0.0;
    for j in 0..M {
        let pa = pts[j];
        let pb = pts[(j + 1) % M];
        let l = (pb[0] - pa[0]).hypot(pb[1] - pa[1]);
        l_arr[j] = l;
        total += l;
    }

    let n = ((34.0 * o.icon_d).round() as usize).max(6);
    let re = o.r_dot * 1.35 * o.spread;
    let pulse = 1.0 + 0.02 * (local * 3.1).sin();
    let c_yaw = (hy * 0.55).cos();
    let c_tilt = (ht * 0.55).cos();
    let s_yaw = (hy * 0.55).sin();

    let mut dots = Vec::with_capacity(n);
    let c2 = size * 0.5;
    let mut seg = 0usize;
    let mut acc = 0.0;
    for k2 in 0..n {
        let target = (k2 as f64 / n as f64) * total;
        while acc + l_arr[seg] < target && seg < M - 1 {
            acc += l_arr[seg];
            seg += 1;
        }
        let s0 = pts[seg];
        let s1 = pts[(seg + 1) % M];
        let sf = if l_arr[seg] != 0.0 { ((target - acc) / l_arr[seg]).min(1.0) } else { 0.0 };
        let px = (s0[0] + (s1[0] - s0[0]) * sf) * pulse;
        let py = (s0[1] + (s1[1] - s0[1]) * sf) * pulse;
        let tx = px * c_yaw;
        let ty = py * c_tilt;
        let tz = px * s_yaw;
        dots.push(OrbDot {
            x: c2 + tx * size,
            y: c2 + ty * size,
            z: tz,
            r: (re * size).max(0.35),
            white: 0.1 + (-tz * 0.35).clamp(-0.08, 0.18),
            a: 1.0,
            boost: 0.0,
            idx: 0,
        });
    }
    finalize(dots, Vec::new(), o.r_min)
}

// =========================================================================
// 2. PALETTES & PARAMS (1:1 with prototype/thinking-orbs.html & themes.json)
// =========================================================================

#[derive(Clone, Copy)]
struct OrbPalette {
    bg_inner: [f32; 3],
    bg_mid: [f32; 3],
    bg_outer: [f32; 3],
    near_rgb: [f32; 3],
    far_rgb: [f32; 3],
    accent_rgb: [f32; 3],
    halo_rgb: [f32; 3],
    icon_bg: [f32; 4],
    icon_stroke: [f32; 4],
    link_rgb: [f32; 3],
}

const fn c3(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}
const fn c4(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

const PALETTES: [OrbPalette; 5] = [
    // 0: monochrome-dark
    OrbPalette {
        bg_inner: c3(0x18, 0x1b, 0x24),
        bg_mid: c3(0x0d, 0x0f, 0x15),
        bg_outer: c3(0x06, 0x07, 0x0a),
        near_rgb: c3(250, 251, 255),
        far_rgb: c3(72, 78, 90),
        accent_rgb: c3(255, 255, 255),
        halo_rgb: c3(215, 226, 248),
        icon_bg: c4(255, 255, 255, 0.045),
        icon_stroke: c4(220, 230, 248, 0.20),
        link_rgb: c3(210, 222, 242),
    },
    // 1: monochrome-light
    OrbPalette {
        bg_inner: c3(0xff, 0xff, 0xff),
        bg_mid: c3(0xf2, 0xf3, 0xf5),
        bg_outer: c3(0xdf, 0xe3, 0xe8),
        near_rgb: c3(18, 20, 24),
        far_rgb: c3(168, 174, 184),
        accent_rgb: c3(0, 0, 0),
        halo_rgb: c3(25, 32, 48),
        icon_bg: c4(18, 22, 30, 0.055),
        icon_stroke: c4(24, 30, 42, 0.22),
        link_rgb: c3(36, 44, 58),
    },
    // 2: cobalt
    OrbPalette {
        bg_inner: c3(0x0f, 0x20, 0x40),
        bg_mid: c3(0x08, 0x12, 0x26),
        bg_outer: c3(0x03, 0x07, 0x10),
        near_rgb: c3(216, 244, 255),
        far_rgb: c3(46, 98, 176),
        accent_rgb: c3(103, 232, 249),
        halo_rgb: c3(56, 189, 248),
        icon_bg: c4(56, 189, 248, 0.065),
        icon_stroke: c4(125, 211, 252, 0.26),
        link_rgb: c3(125, 211, 252),
    },
    // 3: amber
    OrbPalette {
        bg_inner: c3(0x24, 0x1a, 0x11),
        bg_mid: c3(0x12, 0x0d, 0x09),
        bg_outer: c3(0x07, 0x05, 0x04),
        near_rgb: c3(255, 242, 210),
        far_rgb: c3(130, 82, 42),
        accent_rgb: c3(251, 191, 36),
        halo_rgb: c3(245, 158, 11),
        icon_bg: c4(245, 158, 11, 0.06),
        icon_stroke: c4(252, 211, 77, 0.24),
        link_rgb: c3(252, 211, 77),
    },
    // 4: emerald
    OrbPalette {
        bg_inner: c3(0x0a, 0x24, 0x1e),
        bg_mid: c3(0x05, 0x13, 0x10),
        bg_outer: c3(0x02, 0x08, 0x06),
        near_rgb: c3(220, 255, 240),
        far_rgb: c3(36, 114, 88),
        accent_rgb: c3(52, 211, 153),
        halo_rgb: c3(52, 211, 153),
        icon_bg: c4(52, 211, 153, 0.06),
        icon_stroke: c4(110, 231, 183, 0.24),
        link_rgb: c3(110, 231, 183),
    },
];

#[derive(Clone, Copy)]
struct QualityProf {
    density_mul: f64,
    glow_steps: bool,
}

const QUALITY_PROFILES: [QualityProf; 3] = [
    QualityProf { density_mul: 0.65, glow_steps: false },
    QualityProf { density_mul: 0.88, glow_steps: true },
    QualityProf { density_mul: 1.00, glow_steps: true },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThinkingOrbsParams {
    pub orb_design: usize,     // 0..=8 (default 1 = searching)
    pub auto_cycle: bool,      // default false
    pub orb_scale: f64,        // 0.55..=1.45 (default 1.0)
    pub dot_density: f64,      // 0.4..=1.8 (default 1.0)
    pub anim_speed: f64,       // 0.25..=2.5 (default 1.0)
    pub hover_reaction: usize, // 0=off, 1=soft, 2=strong (default 2)
    pub dot_glow: f64,         // 0.0..=1.5 (default 0.65)
    pub palette: usize,        // 0..=4 (default 0 = monochrome-dark)
    pub icon_links: bool,      // default true
    pub quality: usize,        // 0=Low, 1=Medium, 2=High (default 2)
    pub adaptive: bool,        // default true
    pub seed: u32,             // default 0
}

impl Default for ThinkingOrbsParams {
    fn default() -> Self {
        Self {
            orb_design: 1,
            auto_cycle: false,
            orb_scale: 1.0,
            dot_density: 1.0,
            anim_speed: 1.0,
            hover_reaction: 2,
            dot_glow: 0.65,
            palette: 0,
            icon_links: true,
            quality: 2,
            adaptive: true,
            seed: 0,
        }
    }
}

fn parse_enum(raw: &HashMap<String, Value>, key: &str, names: &[&str], def: usize) -> usize {
    let Some(v) = raw.get(key) else {
        return def;
    };
    match v {
        Value::String(s) => {
            let l = s.trim().to_ascii_lowercase();
            for (i, n) in names.iter().enumerate() {
                if n.eq_ignore_ascii_case(&l) {
                    return i;
                }
            }
            if let Ok(idx) = l.parse::<usize>() {
                return idx.min(names.len() - 1);
            }
            def
        }
        Value::Number(n) => (n.as_i64().unwrap_or(def as i64).max(0) as usize).min(names.len() - 1),
        _ => def,
    }
}

fn parse_f64(raw: &HashMap<String, Value>, key: &str, def: f64, lo: f64, hi: f64) -> f64 {
    let Some(v) = raw.get(key) else {
        return def;
    };
    let n = match v {
        Value::Number(x) => x.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    };
    match n {
        Some(x) if x.is_finite() => clamp(x, lo, hi),
        _ => def,
    }
}

fn parse_bool(raw: &HashMap<String, Value>, key: &str, def: bool) -> bool {
    let Some(v) = raw.get(key) else {
        return def;
    };
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => {
            let l = s.trim().to_ascii_lowercase();
            l == "true" || l == "1"
        }
        _ => def,
    }
}

impl ThinkingOrbsParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        Self {
            orb_design: parse_enum(raw, "orbDesign", &DESIGN_IDS, 1),
            auto_cycle: parse_bool(raw, "autoCycle", false),
            orb_scale: parse_f64(raw, "orbScale", 1.0, 0.55, 1.45),
            dot_density: parse_f64(raw, "dotDensity", 1.0, 0.4, 1.8),
            anim_speed: parse_f64(raw, "animSpeed", 1.0, 0.25, 2.5),
            hover_reaction: parse_enum(raw, "hoverReaction", &["off", "soft", "strong"], 2),
            dot_glow: parse_f64(raw, "dotGlow", 0.65, 0.0, 1.5),
            palette: parse_enum(
                raw,
                "palette",
                &["monochrome-dark", "monochrome-light", "cobalt", "amber", "emerald"],
                0,
            ),
            icon_links: parse_bool(raw, "iconLinks", true),
            quality: parse_enum(raw, "quality", &["Low", "Medium", "High"], 2),
            adaptive: parse_bool(raw, "adaptive", true),
            seed: parse_f64(raw, "seed", 0.0, 0.0, u32::MAX as f64) as u32,
        }
    }
}

// =========================================================================
// 3. STATE & SIMULATION
// =========================================================================

#[derive(Clone, Copy)]
struct Ripple {
    x: f64,
    y: f64,
    age: f64,
    max_age: f64,
}

pub struct ThinkingOrbsState {
    pub params: ThinkingOrbsParams,
    pub low_mode: bool,
    pub w: f64,
    pub h: f64,
    pub bg_rev: u64,
    pub palette_rev: u64,
    pub layout_rev: u64,
    pub dirty: bool,
    pub orb_clock: f64,
    cycle_timer: f64,
    active_design: usize,
    prev_design: Option<usize>,
    transition_t: f64,
    icons: Vec<Rect>,
    // Pointer state
    target_x: f64,
    target_y: f64,
    pointer_x: f64,
    pointer_y: f64,
    pointer_active: bool,
    pointer_down: bool,
    touch_decay: f64,
    energy: f64,
    inside_energy: f64,
    ripples: Vec<Ripple>,
}

impl Default for ThinkingOrbsState {
    fn default() -> Self {
        Self {
            params: ThinkingOrbsParams::default(),
            low_mode: false,
            w: 1920.0,
            h: 1080.0,
            bg_rev: 1,
            palette_rev: 1,
            layout_rev: 1,
            dirty: true,
            orb_clock: 0.0,
            cycle_timer: 0.0,
            active_design: 1,
            prev_design: None,
            transition_t: 1.0,
            icons: Vec::new(),
            target_x: 960.0,
            target_y: 540.0,
            pointer_x: 960.0,
            pointer_y: 540.0,
            pointer_active: false,
            pointer_down: false,
            touch_decay: 0.0,
            energy: 0.0,
            inside_energy: 0.0,
            ripples: Vec::new(),
        }
    }
}

impl ThinkingOrbsState {
    fn profile(&self) -> QualityProf {
        if self.low_mode {
            QUALITY_PROFILES[0]
        } else {
            QUALITY_PROFILES[self.params.quality.min(2)]
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.layout_rev = self.layout_rev.wrapping_add(1);
            self.dirty = true;
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = (w as f64).max(64.0);
        let nh = (h as f64).max(64.0);
        if (nw - self.w).abs() <= 0.5 && (nh - self.h).abs() <= 0.5 {
            return;
        }
        self.w = nw;
        self.h = nh;
        self.bg_rev = self.bg_rev.wrapping_add(1);
        self.layout_rev = self.layout_rev.wrapping_add(1);
        self.dirty = true;
    }

    fn set_design(&mut self, next: usize, immediate: bool) {
        let n = next.min(8);
        if n == self.active_design {
            return;
        }
        if immediate {
            self.prev_design = None;
            self.active_design = n;
            self.transition_t = 1.0;
        } else {
            self.prev_design = Some(self.active_design);
            self.active_design = n;
            self.transition_t = 0.0;
        }
    }

    pub fn sync_params(&mut self, next: ThinkingOrbsParams) {
        let prev = self.params;
        if prev == next {
            return;
        }
        if prev.orb_design != next.orb_design {
            self.set_design(next.orb_design, false);
            self.cycle_timer = 0.0;
        }
        if prev.palette != next.palette {
            self.bg_rev = self.bg_rev.wrapping_add(1);
            self.palette_rev = self.palette_rev.wrapping_add(1);
        }
        self.params = next;
        self.layout_rev = self.layout_rev.wrapping_add(1);
        self.dirty = true;
    }

    pub fn set_icons(&mut self, icons: &[Rect], vis: bool) {
        self.icons.clear();
        if vis {
            self.icons.extend_from_slice(icons);
        }
        self.layout_rev = self.layout_rev.wrapping_add(1);
        self.dirty = true;
    }

    fn spawn_ripple(&mut self, x: f64, y: f64) {
        if self.params.hover_reaction == 0 {
            return;
        }
        self.ripples.push(Ripple {
            x,
            y,
            age: 0.0,
            max_age: 0.95,
        });
        if self.ripples.len() > 5 {
            self.ripples.remove(0);
        }
    }

    pub fn handle_mouse(&mut self, kind: i32, x: f32, y: f32) -> bool {
        let fx = x as f64;
        let fy = y as f64;
        match kind {
            0 => {
                self.target_x = fx;
                self.target_y = fy;
                self.pointer_active = true;
            }
            1 => {
                self.target_x = fx;
                self.target_y = fy;
                self.pointer_active = true;
                self.pointer_down = true;
                self.spawn_ripple(fx, fy);
            }
            2 => {
                self.pointer_down = false;
                self.touch_decay = 0.95;
            }
            3 => {
                self.pointer_down = false;
                self.pointer_active = false;
            }
            _ => {}
        }
        false
    }

    fn resolved(&self, design_id: usize) -> ResolvedOpts {
        let prof = self.profile();
        let eff_density = clamp(self.params.dot_density * prof.density_mul, 0.25, 2.0);
        resolve_design(design_id, eff_density, 1.0)
    }

    pub fn step(&mut self, dt_raw: f32) {
        let max_dt = if self.low_mode { 0.12 } else { 0.08 };
        let dt = clamp(dt_raw as f64, 0.001, max_dt);

        if self.params.auto_cycle {
            self.cycle_timer += dt * clamp(self.params.anim_speed, 0.25, 2.5);
            if self.cycle_timer >= 14.0 {
                self.cycle_timer -= 14.0;
                let next_id = (self.active_design + 1) % DESIGN_IDS.len();
                self.set_design(next_id, false);
            }
        }

        if self.transition_t < 1.0 {
            self.transition_t = (self.transition_t + dt / 0.55).min(1.0);
        }

        if self.touch_decay > 0.0 {
            self.touch_decay = (self.touch_decay - dt).max(0.0);
        }

        let has_signal = self.pointer_active || self.pointer_down || self.touch_decay > 0.0;
        let hr_enabled = self.params.hover_reaction != 0;
        let target_energy = if hr_enabled && has_signal {
            if self.touch_decay > 0.0 && !self.pointer_active && !self.pointer_down {
                clamp(self.touch_decay / 0.9, 0.0, 1.0)
            } else {
                1.0
            }
        } else {
            0.0
        };

        let spring_k = 1.0 - (-13.0 * dt).exp();
        self.pointer_x = lerp(self.pointer_x, self.target_x, spring_k);
        self.pointer_y = lerp(self.pointer_y, self.target_y, spring_k);
        let rate = if target_energy > self.energy { 10.0 } else { 4.2 };
        self.energy = lerp(self.energy, target_energy, 1.0 - (-rate * dt).exp());

        let cx = self.w * 0.5;
        let cy = self.h * 0.5;
        let orb_size = self.w.min(self.h) * 0.56 * self.params.orb_scale;
        let dist_center = (self.pointer_x - cx).hypot(self.pointer_y - cy);
        let target_inside = if hr_enabled && has_signal && dist_center <= orb_size * 0.58 {
            clamp(1.0 - (dist_center / (orb_size * 0.62)) * 0.35, 0.4, 1.0) * self.energy
        } else {
            0.0
        };
        self.inside_energy = lerp(self.inside_energy, target_inside, 1.0 - (-7.0 * dt).exp());

        self.ripples.retain_mut(|r| {
            r.age += dt;
            r.age < r.max_age
        });

        let hr_mul = match self.params.hover_reaction {
            2 => 0.72,
            1 => 0.35,
            _ => 0.0,
        };
        let res = self.resolved(self.active_design);
        let speed_mul = self.params.anim_speed * res.speed * (1.0 + self.inside_energy * hr_mul);
        self.orb_clock += dt * speed_mul;
    }
}

// =========================================================================
// 4. PRE-RENDERED RGBA8 GLOW HALO SPRITE (STRICT 1px alpha=0 BORDER)
// =========================================================================

pub fn bake_orb_halo(halo_rgb: [f32; 3]) -> Vec<u8> {
    const SZ: usize = 256;
    let mut px = vec![0u8; SZ * SZ * 4];
    let half = 128.0f32;
    let r0 = 6.0f32;
    let r1 = 126.0f32;
    let span = r1 - r0;
    let stops: [(f32, f32); 4] = [(0.0, 0.28), (0.38, 0.11), (0.72, 0.03), (1.0, 0.0)];

    let cr = (halo_rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8;
    let cg = (halo_rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8;
    let cb = (halo_rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8;

    for y in 0..SZ {
        for x in 0..SZ {
            if x == 0 || y == 0 || x == SZ - 1 || y == SZ - 1 {
                continue; // Strict outer 1px alpha=0 border
            }
            let dist = (x as f32 + 0.5 - half).hypot(y as f32 + 0.5 - half);
            if dist >= r1 {
                continue;
            }
            let t = ((dist - r0) / span).clamp(0.0, 1.0);
            let mut a = 0.0f32;
            for w in stops.windows(2) {
                if t >= w[0].0 && t <= w[1].0 {
                    let f = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-5);
                    a = w[0].1 + (w[1].1 - w[0].1) * f;
                    break;
                }
            }
            let off = (y * SZ + x) * 4;
            px[off] = cr;
            px[off + 1] = cg;
            px[off + 2] = cb;
            px[off + 3] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    px
}

// =========================================================================
// 5. OPENGL 3.3 RENDERER
// =========================================================================

#[repr(C)]
#[derive(Clone, Copy)]
struct TexVert {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
    a: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PrimVert {
    x: f32,
    y: f32,
    lx: f32,
    ly: f32,
    col: [f32; 4],
    stroke: [f32; 4],
    params: [f32; 4],
}

pub struct ThinkingOrbsRenderer {
    prog_bg: GLuint,
    prog_tex: GLuint,
    prog_prim: GLuint,
    dummy_vao: GLuint,
    tex_vao: GLuint,
    tex_vbo: GLuint,
    prim_vao: GLuint,
    prim_vbo: GLuint,
    bg_fbo: GLuint,
    bg_tex: GLuint,
    bg_w: i32,
    bg_h: i32,
    cached_bg_rev: u64,
    halo_tex: GLuint,
    cached_pal_rev: u64,
    tex_verts: Vec<TexVert>,
    prim_verts: Vec<PrimVert>,
}

impl ThinkingOrbsRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_bg = gl.compile_program(TO_BG_VERT, TO_BG_FRAG)?;
        let prog_tex = gl.compile_program(TO_TEX_VERT, TO_TEX_FRAG)?;
        let prog_prim = gl.compile_program(TO_PRIM_VERT, TO_PRIM_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let (mut tex_vao, mut tex_vbo) = (0, 0);
        (gl.GenVertexArrays)(1, &mut tex_vao);
        (gl.GenBuffers)(1, &mut tex_vbo);
        (gl.BindVertexArray)(tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, tex_vbo);
        let st_t = std::mem::size_of::<TexVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, st_t, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, st_t, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 1, GL_FLOAT, GL_FALSE, st_t, 16 as *const c_void);

        let (mut prim_vao, mut prim_vbo) = (0, 0);
        (gl.GenVertexArrays)(1, &mut prim_vao);
        (gl.GenBuffers)(1, &mut prim_vbo);
        (gl.BindVertexArray)(prim_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, prim_vbo);
        let st_p = std::mem::size_of::<PrimVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, st_p, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, st_p, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 4, GL_FLOAT, GL_FALSE, st_p, 16 as *const c_void);
        (gl.EnableVertexAttribArray)(3);
        (gl.VertexAttribPointer)(3, 4, GL_FLOAT, GL_FALSE, st_p, 32 as *const c_void);
        (gl.EnableVertexAttribArray)(4);
        (gl.VertexAttribPointer)(4, 4, GL_FLOAT, GL_FALSE, st_p, 48 as *const c_void);
        (gl.BindVertexArray)(0);

        let mut halo_tex = 0;
        (gl.GenTextures)(1, &mut halo_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, halo_tex);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            prog_bg,
            prog_tex,
            prog_prim,
            dummy_vao,
            tex_vao,
            tex_vbo,
            prim_vao,
            prim_vbo,
            bg_fbo: 0,
            bg_tex: 0,
            bg_w: 0,
            bg_h: 0,
            cached_bg_rev: 0,
            halo_tex,
            cached_pal_rev: 0,
            tex_verts: Vec::with_capacity(16),
            prim_verts: Vec::with_capacity(8192),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.bg_tex != 0 { (gl.DeleteTextures)(1, &self.bg_tex); self.bg_tex = 0; }
        if self.bg_fbo != 0 { (gl.DeleteFramebuffers)(1, &self.bg_fbo); self.bg_fbo = 0; }
        if self.halo_tex != 0 { (gl.DeleteTextures)(1, &self.halo_tex); self.halo_tex = 0; }
        for v in [&mut self.dummy_vao, &mut self.tex_vao, &mut self.prim_vao] {
            if *v != 0 { (gl.DeleteVertexArrays)(1, v); *v = 0; }
        }
        for b in [&mut self.tex_vbo, &mut self.prim_vbo] {
            if *b != 0 { (gl.DeleteBuffers)(1, b); *b = 0; }
        }
        for p in [&mut self.prog_bg, &mut self.prog_tex, &mut self.prog_prim] {
            if *p != 0 { (gl.DeleteProgram)(*p); *p = 0; }
        }
    }

    unsafe fn ensure_caches(&mut self, gl: &GlFns, s: &ThinkingOrbsState) {
        let pal = PALETTES[s.params.palette.min(4)];
        if self.cached_pal_rev != s.palette_rev {
            let px = bake_orb_halo(pal.halo_rgb);
            (gl.BindTexture)(GL_TEXTURE_2D, self.halo_tex);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                256,
                256,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                px.as_ptr() as *const c_void,
            );
            self.cached_pal_rev = s.palette_rev;
        }

        let bw = ((s.w * 0.35).round() as i32).max(64);
        let bh = ((s.h * 0.35).round() as i32).max(36);
        let size_changed = self.bg_fbo == 0 || self.bg_w != bw || self.bg_h != bh;
        if !size_changed && self.cached_bg_rev == s.bg_rev {
            return;
        }
        if size_changed {
            if self.bg_tex != 0 { (gl.DeleteTextures)(1, &self.bg_tex); }
            if self.bg_fbo != 0 { (gl.DeleteFramebuffers)(1, &self.bg_fbo); }
            (gl.GenFramebuffers)(1, &mut self.bg_fbo);
            (gl.GenTextures)(1, &mut self.bg_tex);
            (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
            (gl.TexImage2D)(
                GL_TEXTURE_2D, 0, GL_RGBA8 as GLint, bw, bh, 0,
                GL_RGBA, GL_UNSIGNED_BYTE, std::ptr::null(),
            );
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
            (gl.FramebufferTexture2D)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, self.bg_tex, 0);
            self.bg_w = bw;
            self.bg_h = bh;
        } else {
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
        }

        (gl.Viewport)(0, 0, bw, bh);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(self.prog_bg);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_bg, "uRes"), bw as f32, bh as f32);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "uBgInner"), pal.bg_inner[0], pal.bg_inner[1], pal.bg_inner[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "uBgMid"), pal.bg_mid[0], pal.bg_mid[1], pal.bg_mid[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "uBgOuter"), pal.bg_outer[0], pal.bg_outer[1], pal.bg_outer[2]);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        self.cached_bg_rev = s.bg_rev;
    }

    unsafe fn draw_tex(&mut self, gl: &GlFns, css_w: f32, css_h: f32, tex: GLuint, x: f32, y: f32, w: f32, h: f32, alpha: f32, v_flip: bool) {
        if alpha <= 0.002 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let (v0, v1) = if v_flip { (1.0, 0.0) } else { (0.0, 1.0) };
        self.tex_verts.clear();
        self.tex_verts.extend_from_slice(&[
            TexVert { x, y, u: 0.0, v: v0, a: alpha },
            TexVert { x: x + w, y, u: 1.0, v: v0, a: alpha },
            TexVert { x: x + w, y: y + h, u: 1.0, v: v1, a: alpha },
            TexVert { x, y, u: 0.0, v: v0, a: alpha },
            TexVert { x: x + w, y: y + h, u: 1.0, v: v1, a: alpha },
            TexVert { x, y: y + h, u: 0.0, v: v1, a: alpha },
        ]);
        (gl.UseProgram)(self.prog_tex);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_tex, "uRes"), css_w, css_h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_tex, "uTex"), 0);
        (gl.BindVertexArray)(self.tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.tex_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.tex_verts.len() * std::mem::size_of::<TexVert>()) as GLsizeiptr,
            self.tex_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, 6);
    }

    unsafe fn flush_prims(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.prim_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_prim);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_prim, "uRes"), css_w, css_h);
        (gl.BindVertexArray)(self.prim_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.prim_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.prim_verts.len() * std::mem::size_of::<PrimVert>()) as GLsizeiptr,
            self.prim_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.prim_verts.len() as GLsizei);
        self.prim_verts.clear();
    }

    fn push_single_orb(
        &mut self,
        s: &ThinkingOrbsState,
        design_id: usize,
        alpha_mul: f64,
        scale_mul: f64,
        orb_size: f64,
        cx: f64,
        cy: f64,
        hr: f64,
        yaw: f64,
        tilt: f64,
        roll: f64,
    ) {
        if alpha_mul <= 0.01 {
            return;
        }
        let res = s.resolved(design_id);
        let box_size = orb_size * scale_mul;
        let frame = eval_orb_frame(&res, box_size, s.orb_clock, yaw, tilt, roll);
        let ox = cx - box_size * 0.5;
        let oy = cy - box_size * 0.5;

        let pal = PALETTES[s.params.palette.min(4)];
        let prof = s.profile();
        let energy = s.energy;
        let mx = s.pointer_x;
        let my = s.pointer_y;
        let field_r = box_size * (0.21 + 0.07 * hr);
        let max_push = box_size * 0.032 * hr;

        // 1. Constellation lines (connecting / web mode)
        for ln in &frame.lines {
            let x1 = ox + ln.x1;
            let y1 = oy + ln.y1;
            let x2 = ox + ln.x2;
            let y2 = oy + ln.y2;
            let mut lalpha = ln.a * alpha_mul;
            if energy > 0.01 && hr > 0.0 {
                let mid_x = (x1 + x2) * 0.5;
                let mid_y = (y1 + y2) * 0.5;
                let ld = (mid_x - mx).hypot(mid_y - my);
                if ld < field_r {
                    let lf = 1.0 - ld / field_r;
                    lalpha = (lalpha + lf * lf * 0.38 * energy * hr).min(1.0);
                }
            }
            let f = clamp(ln.white, 0.0, 1.0) as f32;
            let rgb = mix_rgb(pal.near_rgb, pal.far_rgb, f);
            push_capsule_line(
                &mut self.prim_verts,
                x1 as f32,
                y1 as f32,
                x2 as f32,
                y2 as f32,
                ln.w as f32,
                [rgb[0], rgb[1], rgb[2], lalpha as f32],
            );
        }

        // 2. Z-sorted dots (far -> near)
        let has_ripples = !s.ripples.is_empty();
        for dot in &frame.dots {
            let mut px = ox + dot.x;
            let mut py = oy + dot.y;
            let mut pr = dot.r;
            let mut pa = dot.a * alpha_mul;
            let mut accent_blend = 0.0f64;

            if energy > 0.008 && hr > 0.0 {
                let dx = px - mx;
                let dy = py - my;
                let dist = dx.hypot(dy);
                if dist < field_r {
                    let u = 1.0 - dist / field_r;
                    let fall = u * u * (3.0 - 2.0 * u) * energy;
                    let inv_d = if dist > 0.5 { 1.0 / dist } else { 0.0 };
                    px += dx * inv_d * max_push * fall;
                    py += dy * inv_d * max_push * fall;
                    pr *= 1.0 + fall * 0.72 * hr;
                    pa = (pa + fall * 0.42 * alpha_mul).min(1.0);
                    accent_blend = accent_blend.max(fall * 0.85 * hr);
                }
            }

            if has_ripples {
                let rip_w = box_size * 0.12;
                for rip in &s.ripples {
                    let prog = rip.age / rip.max_age;
                    let ring_r = prog * box_size * 0.68;
                    let rdx = px - rip.x;
                    let rdy = py - rip.y;
                    let r_dist = rdx.hypot(rdy);
                    let delta = (r_dist - ring_r).abs();
                    if delta < rip_w {
                        let wave = (1.0 - delta / rip_w) * (1.0 - prog);
                        let r_inv = if r_dist > 0.5 { 1.0 / r_dist } else { 0.0 };
                        px += rdx * r_inv * box_size * 0.024 * wave;
                        py += rdy * r_inv * box_size * 0.024 * wave;
                        pr *= 1.0 + wave * 0.62;
                        pa = (pa + wave * 0.45 * alpha_mul).min(1.0);
                        accent_blend = accent_blend.max(wave * 0.95);
                    }
                }
            }

            if pa < 0.015 {
                continue;
            }

            let base_w = clamp(dot.white, 0.0, 1.0) as f32;
            let mut rgb = mix_rgb(pal.near_rgb, pal.far_rgb, base_w);
            if accent_blend > 0.06 {
                rgb = mix_rgb(rgb, pal.accent_rgb, accent_blend as f32);
            }

            if prof.glow_steps && s.params.dot_glow > 0.08 && (dot.boost > 0.25 || accent_blend > 0.25) {
                let gf = dot.boost.max(accent_blend) * clamp(s.params.dot_glow, 0.0, 1.5);
                let ga = (pa * 0.16 * gf.min(1.0)) as f32;
                push_disc(&mut self.prim_verts, px as f32, py as f32, (pr * 2.15) as f32, [rgb[0], rgb[1], rgb[2], ga]);
            }

            push_disc(&mut self.prim_verts, px as f32, py as f32, pr as f32, [rgb[0], rgb[1], rgb[2], pa as f32]);
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        st.thinking_orbs.resize(css_w, css_h);
        st.thinking_orbs.dirty = false;

        self.ensure_caches(gl, &st.thinking_orbs);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        // 1. Cached radial gradient background (FBO is bottom-up, v_flip=true)
        self.draw_tex(gl, css_w, css_h, self.bg_tex, 0.0, 0.0, css_w, css_h, 1.0, true);

        let s = &st.thinking_orbs;
        let cx = s.w * 0.5;
        let cy = s.h * 0.5;
        let orb_size = s.w.min(s.h) * 0.56 * s.params.orb_scale;
        let orb_radius = orb_size * 0.42;

        // 2. Ambient halo behind center orb + cursor halo
        if s.params.dot_glow > 0.02 {
            let glow_mul = clamp(s.params.dot_glow, 0.0, 1.5);
            let hover_boost = 1.0 + s.inside_energy * 0.35;
            let g_size = (orb_size * (1.18 + 0.14 * s.inside_energy) * (0.75 + 0.25 * glow_mul)) as f32;
            let ga = clamp(0.45 * glow_mul * hover_boost, 0.0, 0.95) as f32;
            self.draw_tex(
                gl,
                css_w,
                css_h,
                self.halo_tex,
                cx as f32 - g_size * 0.5,
                cy as f32 - g_size * 0.5,
                g_size,
                g_size,
                ga,
                false,
            );

            if s.energy > 0.02 && s.params.hover_reaction != 0 {
                let cg = (orb_size * 0.46) as f32;
                let ca = (0.26 * s.energy * glow_mul) as f32;
                self.draw_tex(
                    gl,
                    css_w,
                    css_h,
                    self.halo_tex,
                    s.pointer_x as f32 - cg * 0.5,
                    s.pointer_y as f32 - cg * 0.5,
                    cg,
                    cg,
                    ca,
                    false,
                );
            }
        }

        // 3. Adaptive Icon Cradles & Telemetry Links
        self.prim_verts.clear();
        let pal = PALETTES[s.params.palette.min(4)];
        if s.params.adaptive && !s.icons.is_empty() {
            let max_dim = s.w.max(s.h);
            for (i, ic) in s.icons.iter().enumerate() {
                let pad = 6.0f32;
                push_round_rect(
                    &mut self.prim_verts,
                    ic.x - pad,
                    ic.y - pad,
                    ic.w + pad * 2.0,
                    ic.h + pad * 2.0,
                    12.0,
                    pal.icon_bg,
                    pal.icon_stroke,
                );

                if s.params.icon_links {
                    let ix = ic.x as f64 + ic.w as f64 * 0.5;
                    let iy = ic.y as f64 + ic.h as f64 * 0.5;
                    let dx = cx - ix;
                    let dy = cy - iy;
                    let dist = dx.hypot(dy);
                    if dist > orb_radius * 0.92 && dist < max_dim * 0.65 {
                        let nx = dx / dist;
                        let ny = dy / dist;
                        let start_x = ix + nx * (ic.w.max(ic.h) as f64 * 0.55);
                        let start_y = iy + ny * (ic.w.max(ic.h) as f64 * 0.55);
                        let end_x = cx - nx * (orb_radius * 0.88);
                        let end_y = cy - ny * (orb_radius * 0.88);
                        let span = (end_x - start_x).hypot(end_y - start_y);
                        if span > 18.0 {
                            let dots_n = ((span / 26.0).floor() as usize).clamp(4, 18);
                            let phase = frac(s.orb_clock * 0.35 + i as f64 * 0.27);
                            for k in 0..dots_n {
                                let mut f = (k as f64 + phase) / dots_n as f64;
                                if f > 1.0 {
                                    f -= 1.0;
                                }
                                let px = start_x + (end_x - start_x) * f;
                                let py = start_y + (end_y - start_y) * f;
                                let edge_fade = (f * PI).sin();
                                let a = (0.28 * edge_fade) as f32;
                                let r = (1.35 + 0.7 * edge_fade) as f32;
                                push_disc(
                                    &mut self.prim_verts,
                                    px as f32,
                                    py as f32,
                                    r,
                                    [pal.link_rgb[0], pal.link_rgb[1], pal.link_rgb[2], a],
                                );
                            }
                        }
                    }
                }
            }
        }

        // 4. 3D Orb(s) with smooth mode cross-fade transition
        let hr = match s.params.hover_reaction {
            2 => 1.0,
            1 => 0.52,
            _ => 0.0,
        };
        let nx = clamp((s.pointer_x - cx) / orb_radius.max(60.0), -1.5, 1.5);
        let ny = clamp((s.pointer_y - cy) / orb_radius.max(60.0), -1.5, 1.5);
        let yaw = nx * 0.42 * s.energy * hr;
        let tilt = -ny * 0.34 * s.energy * hr;
        let roll = nx * ny * 0.07 * s.energy * hr;

        if s.transition_t < 1.0 {
            if let Some(prev) = s.prev_design {
                if prev != s.active_design {
                    let ease = s.transition_t * s.transition_t * (3.0 - 2.0 * s.transition_t);
                    self.push_single_orb(s, prev, 1.0 - ease, 1.0 - 0.1 * ease, orb_size, cx, cy, hr, yaw, tilt, roll);
                    self.push_single_orb(s, s.active_design, ease, 0.9 + 0.1 * ease, orb_size, cx, cy, hr, yaw, tilt, roll);
                } else {
                    self.push_single_orb(s, s.active_design, 1.0, 1.0, orb_size, cx, cy, hr, yaw, tilt, roll);
                }
            } else {
                self.push_single_orb(s, s.active_design, 1.0, 1.0, orb_size, cx, cy, hr, yaw, tilt, roll);
            }
        } else {
            self.push_single_orb(s, s.active_design, 1.0, 1.0, orb_size, cx, cy, hr, yaw, tilt, roll);
        }

        self.flush_prims(gl, css_w, css_h);
    }
}

#[inline]
fn mix_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn push_disc(out: &mut Vec<PrimVert>, cx: f32, cy: f32, r: f32, col: [f32; 4]) {
    let pad = r + 1.25;
    let lim = pad / r.max(0.1);
    let p = [0.0, 0.0, 0.0, 0.0];
    let z = [0.0; 4];
    let mk = |dx, dy, lx, ly| PrimVert { x: cx + dx, y: cy + dy, lx, ly, col, stroke: z, params: p };
    out.extend_from_slice(&[
        mk(-pad, -pad, -lim, -lim), mk(pad, -pad, lim, -lim), mk(pad, pad, lim, lim),
        mk(-pad, -pad, -lim, -lim), mk(pad, pad, lim, lim), mk(-pad, pad, -lim, lim),
    ]);
}

fn push_capsule_line(out: &mut Vec<PrimVert>, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, col: [f32; 4]) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = dx.hypot(dy);
    if len < 0.01 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let nx = -uy;
    let ny = ux;
    let half_len = len * 0.5;
    let half_w = width * 0.5;
    let pad = half_w + 1.5;
    let mx = (x1 + x2) * 0.5;
    let my = (y1 + y2) * 0.5;
    let ext_x = half_len + pad;
    let ext_y = pad;
    let p = [1.0, half_len, half_w, 0.0];
    let z = [0.0; 4];
    let mk = |lx: f32, ly: f32| PrimVert {
        x: mx + ux * lx + nx * ly,
        y: my + uy * lx + ny * ly,
        lx,
        ly,
        col,
        stroke: z,
        params: p,
    };
    out.extend_from_slice(&[
        mk(-ext_x, -ext_y), mk(ext_x, -ext_y), mk(ext_x, ext_y),
        mk(-ext_x, -ext_y), mk(ext_x, ext_y), mk(-ext_x, ext_y),
    ]);
}

fn push_round_rect(
    out: &mut Vec<PrimVert>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    fill: [f32; 4],
    stroke: [f32; 4],
) {
    let hw = w * 0.5;
    let hh = h * 0.5;
    let cx = x + hw;
    let cy = y + hh;
    let pad = 2.0f32;
    let p = [2.0, hw, hh, radius];
    let mk = |lx: f32, ly: f32| PrimVert { x: cx + lx, y: cy + ly, lx, ly, col: fill, stroke, params: p };
    out.extend_from_slice(&[
        mk(-hw - pad, -hh - pad), mk(hw + pad, -hh - pad), mk(hw + pad, hh + pad),
        mk(-hw - pad, -hh - pad), mk(hw + pad, hh + pad), mk(-hw - pad, hh + pad),
    ]);
}

// =========================================================================
// 6. UNIT TESTS (Determinism, All 9 Modes, Params/Palette, Cache Rev, Halo Border)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_d_vnoise_and_all_9_orb_designs_are_deterministic() {
        let h0 = hash_d(0.0, 1.7);
        let expected = ((1.7_f64 * 78.233).sin() * 43758.5453) - ((1.7_f64 * 78.233).sin() * 43758.5453).floor();
        assert!((h0 - expected).abs() < 1e-12);

        let vn = vnoise(3.4, 2.1);
        assert!(vn >= 0.0 && vn <= 1.0);

        for d in 0..9 {
            let opt = resolve_design(d, 1.0, 1.0);
            let f1 = eval_orb_frame(&opt, 400.0, 2.5, 0.1, -0.05, 0.01);
            let f2 = eval_orb_frame(&opt, 400.0, 2.5, 0.1, -0.05, 0.01);
            assert!(!f1.dots.is_empty(), "design {d} produced 0 dots");
            assert_eq!(f1.dots.len(), f2.dots.len());
            assert!((f1.dots[0].x - f2.dots[0].x).abs() < 1e-12);
            if d == 4 {
                assert!(!f1.lines.is_empty(), "connecting/web design must produce constellation lines");
            }
        }
    }

    #[test]
    fn params_defaults_enums_and_palette_mapping_match_themes_json() {
        let def = ThinkingOrbsParams::default();
        assert_eq!(def.orb_design, 1); // searching
        assert_eq!(def.palette, 0); // monochrome-dark
        assert_eq!(def.hover_reaction, 2); // strong
        assert_eq!(def.quality, 2); // High
        assert!(def.icon_links);
        assert!(!def.auto_cycle);

        let mut raw = HashMap::new();
        raw.insert("orbDesign".into(), Value::String("connecting".into()));
        raw.insert("palette".into(), Value::String("emerald".into()));
        raw.insert("hoverReaction".into(), Value::String("soft".into()));
        raw.insert("quality".into(), Value::String("Low".into()));
        raw.insert("autoCycle".into(), Value::Bool(true));
        raw.insert("iconLinks".into(), Value::Bool(false));
        let p = ThinkingOrbsParams::from_raw_params(&raw);
        assert_eq!(p.orb_design, 4);
        assert_eq!(p.palette, 4);
        assert_eq!(p.hover_reaction, 1);
        assert_eq!(p.quality, 0);
        assert!(p.auto_cycle);
        assert!(!p.icon_links);

        // Numeric indices match exact themes.json order
        let mut raw_n = HashMap::new();
        raw_n.insert("orbDesign".into(), Value::from(8));
        raw_n.insert("palette".into(), Value::from(2));
        raw_n.insert("hoverReaction".into(), Value::from(0));
        let pn = ThinkingOrbsParams::from_raw_params(&raw_n);
        assert_eq!(pn.orb_design, 8); // shaping
        assert_eq!(pn.palette, 2); // cobalt
        assert_eq!(pn.hover_reaction, 0); // off
    }

    #[test]
    fn cache_rev_invalidates_on_palette_resize_and_icon_move() {
        let mut st = ThinkingOrbsState::default();
        let bg0 = st.bg_rev;
        let pal0 = st.palette_rev;
        let lay0 = st.layout_rev;

        // Sub-0.5px resize does not dirty
        st.dirty = false;
        st.resize(1920.2, 1080.3);
        assert!(!st.dirty);
        assert_eq!(st.bg_rev, bg0);

        // Real resize bumps bg_rev & layout_rev
        st.resize(1280.0, 720.0);
        assert!(st.dirty);
        assert!(st.bg_rev > bg0);
        assert!(st.layout_rev > lay0);

        // Palette change bumps both bg_rev & palette_rev
        let bg1 = st.bg_rev;
        let mut p = st.params;
        p.palette = 2; // cobalt
        st.sync_params(p);
        assert!(st.bg_rev > bg1);
        assert!(st.palette_rev > pal0);

        // Moving same count of icons bumps layout_rev & updates icon rects
        let a = vec![Rect { x: 40.0, y: 40.0, w: 72.0, h: 86.0 }];
        st.set_icons(&a, true);
        let l1 = st.layout_rev;
        let b = vec![Rect { x: 400.0, y: 240.0, w: 72.0, h: 86.0 }];
        st.set_icons(&b, true);
        assert!(st.layout_rev > l1);
        assert!((st.icons[0].x - 400.0).abs() < 1e-3);
    }

    #[test]
    fn halo_sprite_has_strictly_transparent_1px_outer_border() {
        for pal in &PALETTES {
            let px = bake_orb_halo(pal.halo_rgb);
            for x in 0..256 {
                assert_eq!(px[(0 * 256 + x) * 4 + 3], 0);
                assert_eq!(px[(255 * 256 + x) * 4 + 3], 0);
            }
            for y in 0..256 {
                assert_eq!(px[(y * 256 + 0) * 4 + 3], 0);
                assert_eq!(px[(y * 256 + 255) * 4 + 3], 0);
            }
        }
    }
}
