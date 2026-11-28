//! 04 · Lotus After Rain — 1:1 faithful interactive Rust port of
//! `sapozhkov/ascii-lab-public/04-lotus/runtime/` (`model.py`, `living.py`,
//! `day.py`, `petal_light.py`, `passing_light.py`, `visitor_leaves.py`,
//! `broad_leaves.py`, `bank_grass.py`, `dew.py`, `visitor.py`, `ascii_preview.py`).

use super::grid_gl::{glyph_id, AsciiCanvas};
use super::lotus_palette::sample_petal_pigment;
use super::AsciiLabParams;
use crate::state::Rect;
use std::f32::consts::{PI, TAU};

const PERIOD: f32 = 20.0;
const SIM_RATE: f32 = 30.0;
const CYCLE_RATE: f32 = 60.0;
const NU: usize = 27;
const NV: usize = 17;
const PETAL_COUNT: usize = 14;
const STAMEN_N: usize = 156;
const REF_DEG: f32 = 31.0;

const RAMP: &[u8] = b" .,:-+=xo08%#@";
const PAPER: [f32; 3] = [8.0, 16.0, 25.0];

const PHASE_SEED: [f32; PETAL_COUNT] = [
    1.82, -2.41, 0.74, -1.18, 2.66, -0.39, 1.09,
    -2.79, 0.14, 2.18, -1.62, 0.52, -0.84, 1.44,
];

#[derive(Clone, Copy)]
struct PetalDef {
    root: [f32; 2],
    tip: [f32; 2],
    left: [[f32; 2]; 2],
    right: [[f32; 2]; 2],
    travel: [f32; 2],
    open_l: [f32; 2],
    open_r: [f32; 2],
    swell: f32,
    lag: f32,
    depth: f32,
}

const PETALS: [PetalDef; PETAL_COUNT] = [
    PetalDef {
        root: [478.0, 346.0], tip: [294.0, 284.0],
        left: [[393.0, 257.0], [317.0, 256.0]], right: [[425.0, 373.0], [334.0, 325.0]],
        travel: [-156.0, 21.0], open_l: [-40.0, -40.0], open_r: [-64.0, 54.0],
        swell: 0.05, lag: 0.0, depth: -38.0,
    },
    PetalDef {
        root: [485.0, 343.0], tip: [496.0, 115.0],
        left: [[404.0, 305.0], [398.0, 135.0]], right: [[572.0, 256.0], [577.0, 129.0]],
        travel: [18.0, 9.0], open_l: [26.0, 68.0], open_r: [-76.0, 2.0],
        swell: 0.05, lag: 0.0, depth: -150.0,
    },
    PetalDef {
        root: [485.0, 346.0], tip: [600.0, 54.0],
        left: [[467.0, 194.0], [482.0, 86.0]], right: [[627.0, 249.0], [642.0, 99.0]],
        travel: [61.0, 10.0], open_l: [34.0, 0.0], open_r: [124.0, 22.0],
        swell: 0.05, lag: 0.0, depth: -115.0,
    },
    PetalDef {
        root: [493.0, 346.0], tip: [737.0, 146.0],
        left: [[581.0, 236.0], [598.0, 99.0]], right: [[629.0, 314.0], [707.0, 243.0]],
        travel: [129.0, 89.0], open_l: [54.0, 0.0], open_r: [24.0, 40.0],
        swell: 0.05, lag: 0.0, depth: -48.0,
    },
    PetalDef {
        root: [490.0, 353.0], tip: [784.0, 364.0],
        left: [[625.0, 279.0], [728.0, 292.0]], right: [[644.0, 472.0], [739.0, 383.0]],
        travel: [30.0, 49.0], open_l: [-8.0, 0.0], open_r: [32.0, 54.0],
        swell: 0.05, lag: 0.0, depth: -37.0,
    },
    PetalDef {
        root: [486.0, 360.0], tip: [287.0, 386.0],
        left: [[404.0, 383.0], [339.0, 369.0]], right: [[390.0, 510.0], [325.0, 422.0]],
        travel: [-22.0, 96.0], open_l: [58.0, -90.0], open_r: [44.0, 80.0],
        swell: 0.05, lag: 0.0, depth: 62.0,
    },
    PetalDef {
        root: [479.0, 348.0], tip: [350.0, 92.0],
        left: [[374.0, 308.0], [275.0, 187.0]], right: [[523.0, 244.0], [469.0, 138.0]],
        travel: [-6.0, -2.0], open_l: [-48.0, -42.0], open_r: [58.0, 0.0],
        swell: 0.05, lag: 0.0, depth: -87.0,
    },
    PetalDef {
        root: [477.0, 357.0], tip: [173.0, 312.0],
        left: [[331.0, 242.0], [239.0, 244.0]], right: [[315.0, 422.0], [225.0, 345.0]],
        travel: [-39.0, 81.0], open_l: [-48.0, 32.0], open_r: [8.0, 54.0],
        swell: 0.05, lag: 0.0, depth: -1.0,
    },
    PetalDef {
        root: [474.0, 355.0], tip: [253.0, 191.0],
        left: [[378.0, 242.0], [353.0, 158.0]], right: [[385.0, 368.0], [249.0, 253.0]],
        travel: [-108.0, 51.0], open_l: [-14.0, -26.0], open_r: [-40.0, 0.0],
        swell: 0.05, lag: 0.0, depth: -55.0,
    },
    PetalDef {
        root: [492.0, 348.0], tip: [647.0, 496.0],
        left: [[513.0, 451.0], [574.0, 491.0]], right: [[630.0, 343.0], [713.0, 438.0]],
        travel: [50.0, 80.0], open_l: [-94.0, 12.0], open_r: [-30.0, -48.0],
        swell: 0.05, lag: 0.0, depth: 225.0,
    },
    PetalDef {
        root: [507.0, 356.0], tip: [351.0, 414.0],
        left: [[434.0, 342.0], [293.0, 323.0]], right: [[472.0, 437.0], [388.0, 458.0]],
        travel: [-22.0, 112.0], open_l: [-22.0, 22.0], open_r: [102.0, 40.0],
        swell: 0.05, lag: 0.0, depth: 155.0,
    },
    PetalDef {
        root: [497.0, 350.0], tip: [536.0, 166.0],
        left: [[563.0, 306.0], [659.0, 151.0]], right: [[667.0, 292.0], [606.0, 89.0]],
        travel: [97.0, 86.0], open_l: [0.0, 0.0], open_r: [0.0, 0.0],
        swell: 0.22, lag: 0.14, depth: 53.0,
    },
    PetalDef {
        root: [480.0, 359.0], tip: [472.0, 210.0],
        left: [[417.0, 358.0], [400.0, 270.0]], right: [[537.0, 326.0], [533.0, 263.0]],
        travel: [-63.0, 94.0], open_l: [0.0, 0.0], open_r: [0.0, 0.0],
        swell: 0.45, lag: 0.02, depth: 119.0,
    },
    PetalDef {
        root: [506.0, 358.0], tip: [596.0, 221.0],
        left: [[438.0, 254.0], [574.0, 256.0]], right: [[592.0, 379.0], [624.0, 269.0]],
        travel: [51.0, 123.0], open_l: [0.0, 0.0], open_r: [0.0, 0.0],
        swell: 0.38, lag: 0.10, depth: 137.0,
    },
];

struct GrassBladeDef {
    root: [f32; 3],
    length: f32,
    lean: [f32; 2],
    half_width: f32,
    heading_deg: f32,
    stiffness: f32,
    drag: f32,
    damping: f32,
    pigment: [f32; 3],
}

const GRASS_BLADES: [GrassBladeDef; 5] = [
    GrassBladeDef { root: [-475.0, 35.0, -714.0], length: 414.0, lean: [-20.0, 18.0], half_width: 12.0, heading_deg: 19.0, stiffness: 0.71, drag: 0.18, damping: 3.0, pigment: [87.0, 113.0, 55.0] },
    GrassBladeDef { root: [-434.0, 78.0, -728.0], length: 350.0, lean: [50.0, 12.0], half_width: 10.7, heading_deg: 36.0, stiffness: 0.63, drag: 0.18, damping: 3.6, pigment: [72.0, 103.0, 57.0] },
    GrassBladeDef { root: [-359.0, 116.0, -735.0], length: 292.0, lean: [44.0, -16.0], half_width: 9.0, heading_deg: 26.0, stiffness: 0.57, drag: 0.10, damping: 3.9, pigment: [76.0, 106.0, 63.0] },
    GrassBladeDef { root: [350.0, 135.0, -734.0], length: 353.0, lean: [-50.0, 27.0], half_width: 10.0, heading_deg: -26.0, stiffness: 0.76, drag: 0.16, damping: 3.3, pigment: [74.0, 100.0, 53.0] },
    GrassBladeDef { root: [412.0, 129.0, -765.0], length: 409.0, lean: [35.0, -26.0], half_width: 11.8, heading_deg: -11.0, stiffness: 0.65, drag: 0.16, damping: 3.5, pigment: [90.0, 113.0, 64.0] },
];

#[inline]
fn smooth(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn norm3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[inline]
fn unit3(v: [f32; 3]) -> [f32; 3] {
    let l = norm3(v).max(1e-7);
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
fn bezier2(a: [f32; 2], h1: [f32; 2], h2: [f32; 2], b: [f32; 2], u: f32) -> [f32; 2] {
    let w = 1.0 - u;
    [
        w * w * w * a[0] + 3.0 * w * w * u * h1[0] + 3.0 * w * u * u * h2[0] + u * u * u * b[0],
        w * w * w * a[1] + 3.0 * w * w * u * h1[1] + 3.0 * w * u * u * h2[1] + u * u * u * b[1],
    ]
}

fn wind_at(t: f32) -> f32 {
    let phase = TAU * t / PERIOD;
    let flow = 0.27 * (3.0 * phase + 0.6).sin()
        + 0.16 * (7.0 * phase - 1.5).sin()
        + 0.09 * (12.0 * phase).sin();
    const PACKETS: [(f32, f32, f32); 4] = [
        (3.8, 0.85, 0.45),
        (8.1, 1.0, 0.58),
        (12.2, 1.05, -1.0),
        (15.0, 1.2, 0.96),
    ];
    let mut sum = flow;
    for &(center, width, strength) in &PACKETS {
        let d = (t - center + PERIOD * 0.5).rem_euclid(PERIOD) - PERIOD * 0.5;
        sum += strength * (-0.5 * (d / width).powi(2)).exp();
    }
    sum
}

/// Exact logistic fit from `model.py::fit_modes` for outward `spread` and downward `fall`.
fn fit_modes() -> [(f32, f32); 2] {
    const TIMES: [f32; 5] = [6.70, 7.30, 7.60, 7.80, 8.00];
    const TRACKS: [[[f32; 2]; 5]; 5] = [
        [[737.0, 146.0], [774.0, 167.0], [832.0, 190.0], [851.0, 220.0], [866.0, 235.0]],
        [[253.0, 191.0], [216.0, 204.0], [166.0, 223.0], [151.0, 236.0], [145.0, 242.0]],
        [[173.0, 312.0], [162.0, 330.0], [139.0, 355.0], [134.0, 377.0], [134.0, 393.0]],
        [[350.0, 92.0], [344.0, 94.0], [341.0, 82.0], [348.0, 88.0], [344.0, 90.0]],
        [[600.0, 54.0], [613.0, 55.0], [634.0, 54.0], [650.0, 60.0], [661.0, 64.0]],
    ];
    let response = |t: f32, mid: f32, rate: f32| -> f32 {
        let q = |x: f32| 1.0 / (1.0 + (-rate * (x - mid)).exp());
        (q(t) - q(6.7)) / (q(8.0) - q(6.7))
    };
    let mut modes = [(7.5f32, 5.0f32); 2];
    for axis in 0..2 {
        let mut best_err = f32::INFINITY;
        let mut mid = 7.30f32;
        while mid <= 7.755 {
            let mut rate = 3.0f32;
            while rate <= 8.95 {
                let mut err = 0.0f32;
                for tr in &TRACKS {
                    let o0 = tr[0][axis];
                    let o4 = tr[4][axis];
                    for k in 0..5 {
                        let pred = o0 + (o4 - o0) * response(TIMES[k], mid, rate);
                        let diff = pred - tr[k][axis];
                        err += diff * diff;
                    }
                }
                if err < best_err {
                    best_err = err;
                    modes[axis] = (mid, rate);
                }
                rate += 0.2;
            }
            mid += 0.01;
        }
    }
    modes
}

#[inline]
fn model_response(t: f32, mid: f32, rate: f32) -> f32 {
    let q = |x: f32| 1.0 / (1.0 + (-rate * (x - mid)).exp());
    (q(t) - q(6.7)) / (q(8.0) - q(6.7))
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn raster_tri(
    a: ([f32; 3], [f32; 3]),
    b: ([f32; 3], [f32; 3]),
    c: ([f32; 3], [f32; 3]),
    w: usize,
    h: usize,
    depth: &mut [f32],
    rgb: &mut [[f32; 3]],
) {
    let ux = b.0[0] - a.0[0];
    let uy = b.0[1] - a.0[1];
    let vx = c.0[0] - a.0[0];
    let vy = c.0[1] - a.0[1];
    let area = ux * vy - uy * vx;
    if area.abs() < 1e-5 {
        return;
    }
    let inv_a = 1.0 / area;
    let x0 = a.0[0].min(b.0[0]).min(c.0[0]).floor().max(0.0) as i32;
    let y0 = a.0[1].min(b.0[1]).min(c.0[1]).floor().max(0.0) as i32;
    let x1 = a.0[0].max(b.0[0]).max(c.0[0]).ceil().min(w as f32 - 1.0) as i32;
    let y1 = a.0[1].max(b.0[1]).max(c.0[1]).ceil().min(h as f32 - 1.0) as i32;

    for y in y0..=y1 {
        let py = y as f32 + 0.5;
        for x in x0..=x1 {
            let px = x as f32 + 0.5;
            let dx = px - a.0[0];
            let dy = py - a.0[1];
            let s = (dx * vy - dy * vx) * inv_a;
            let t = (ux * dy - uy * dx) * inv_a;
            let p = 1.0 - s - t;
            if s < -1e-4 || t < -1e-4 || p < -1e-4 {
                continue;
            }
            let z = p * a.0[2] + s * b.0[2] + t * c.0[2];
            let idx = y as usize * w + x as usize;
            if z <= depth[idx] {
                continue;
            }
            depth[idx] = z;
            rgb[idx] = [
                (p * a.1[0] + s * b.1[0] + t * c.1[0]).clamp(0.0, 255.0),
                (p * a.1[1] + s * b.1[1] + t * c.1[1]).clamp(0.0, 255.0),
                (p * a.1[2] + s * b.1[2] + t * c.1[2]).clamp(0.0, 255.0),
            ];
        }
    }
}

#[inline]
fn raster_stamen(
    a: [f32; 3],
    b: [f32; 3],
    col: [f32; 3],
    w: usize,
    h: usize,
    depth: &mut [f32],
    rgb: &mut [[f32; 3]],
) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len = dx * dx + dy * dy + 0.001;
    let x0 = (a[0].min(b[0]) - 2.0).floor().max(0.0) as i32;
    let x1 = (a[0].max(b[0]) + 2.0).ceil().min(w as f32 - 1.0) as i32;
    let y0 = (a[1].min(b[1]) - 2.0).floor().max(0.0) as i32;
    let y1 = (a[1].max(b[1]) + 2.0).ceil().min(h as f32 - 1.0) as i32;
    for y in y0..=y1 {
        let py = y as f32 + 0.5;
        for x in x0..=x1 {
            let px = x as f32 + 0.5;
            let s = (((px - a[0]) * dx + (py - a[1]) * dy) / len).clamp(0.0, 1.0);
            let ex = px - a[0] - s * dx;
            let ey = py - a[1] - s * dy;
            let rad = if s > 0.80 { 1.45 } else { 0.72 };
            if ex * ex + ey * ey > rad * rad {
                continue;
            }
            let z = (a[2] + s * (b[2] - a[2])).min(50.0);
            let idx = y as usize * w + x as usize;
            if z <= depth[idx] {
                continue;
            }
            depth[idx] = z;
            let fac = 0.72 + 0.28 * s;
            rgb[idx] = [
                (col[0] * fac).min(255.0),
                (col[1] * fac).min(255.0),
                (col[2] * fac).min(255.0),
            ];
        }
    }
}

#[inline]
fn raster_line_disk(
    a: [f32; 3],
    b: [f32; 3],
    radius_px: f32,
    col: [f32; 3],
    w: usize,
    h: usize,
    depth: &mut [f32],
    rgb: &mut [[f32; 3]],
) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len = dx * dx + dy * dy + 0.001;
    let r2 = radius_px * radius_px;
    let x0 = (a[0].min(b[0]) - radius_px - 1.0).floor().max(0.0) as i32;
    let x1 = (a[0].max(b[0]) + radius_px + 1.0).ceil().min(w as f32 - 1.0) as i32;
    let y0 = (a[1].min(b[1]) - radius_px - 1.0).floor().max(0.0) as i32;
    let y1 = (a[1].max(b[1]) + radius_px + 1.0).ceil().min(h as f32 - 1.0) as i32;
    for y in y0..=y1 {
        let py = y as f32 + 0.5;
        for x in x0..=x1 {
            let px = x as f32 + 0.5;
            let s = (((px - a[0]) * dx + (py - a[1]) * dy) / len).clamp(0.0, 1.0);
            let ex = px - a[0] - s * dx;
            let ey = py - a[1] - s * dy;
            if ex * ex + ey * ey > r2 {
                continue;
            }
            let z = a[2] + s * (b[2] - a[2]);
            let idx = y as usize * w + x as usize;
            if z < depth[idx] - 1.0 {
                continue;
            }
            depth[idx] = depth[idx].max(z);
            rgb[idx] = col;
        }
    }
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
}

#[derive(Clone)]
struct PetalSheet {
    bud: Vec<[f32; 3]>,
    opened: Vec<[f32; 3]>,
    rest_norm: Vec<[f32; 3]>,
}

const LEAF_R: usize = 20;
const LEAF_A: usize = 48;

#[derive(Clone)]
struct LeafMesh {
    center: [f32; 3],
    reach: [f32; 3],
    x: Vec<f32>,
    y: Vec<f32>,
    height: Vec<f32>,
    r_vals: Vec<f32>,
    a_vals: Vec<f32>,
    color: Vec<[f32; 3]>,
    breeze: Vec<f32>,
    contact_xy: [f32; 2],
    radius_base: f32,
}

struct UserDrop {
    active: bool,
    uv: [f32; 2],
    age: f32,
}

pub struct AsciiLotusState {
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
    breeze_gust: f32,
    user_drops: Vec<UserDrop>,
    // Botanical models
    phases: [f32; PETAL_COUNT],
    sheets: Vec<PetalSheet>,
    open_cycle: Vec<[[f32; NU]; PETAL_COUNT]>,
    flex_cycle: Vec<[f32; PETAL_COUNT]>,
    stem_cycle: Vec<[f32; 2]>,
    leaves: [LeafMesh; 2],
    grass_axes: Vec<Vec<[f32; 3]>>, // [tick * 5][27]
    anther_dir: Vec<[f32; 3]>,
    anther_col: Vec<[f32; 3]>,
    anther_len: Vec<f32>,
    anther_r: Vec<f32>,
    anther_ang: Vec<f32>,
    canopy_dist: Vec<f32>,
    // Cached defocused pond + reeds + distant pads
    pond_w: usize,
    pond_h: usize,
    pond_base: Vec<[f32; 3]>,
    // Per-frame rasterization & Sobel buffers
    rast_rgb: Vec<[f32; 3]>,
    rast_z: Vec<f32>,
    rast_petal: Vec<u8>,
    shade_buf: Vec<f32>,
    light_buf: Vec<f32>,
    // Visitor bumblebee
    bee_lure: [f32; 2],
    bee_lure_blend: f32,
}

impl Default for AsciiLotusState {
    fn default() -> Self {
        Self::new(1280.0, 720.0, 1.0)
    }
}

fn compute_sheet_normals(pts: &[[f32; 3]], nu: usize, nv: usize) -> Vec<[f32; 3]> {
    let mut out = vec![[0.0f32, 0.0, 1.0]; nu * nv];
    for iu in 0..nu {
        let u0 = iu.saturating_sub(1);
        let u1 = (iu + 1).min(nu - 1);
        for iv in 0..nv {
            let v0 = iv.saturating_sub(1);
            let v1 = (iv + 1).min(nv - 1);
            let du = sub3(pts[u1 * nv + iv], pts[u0 * nv + iv]);
            let dv = sub3(pts[iu * nv + v1], pts[iu * nv + v0]);
            out[iu * nv + iv] = unit3(cross3(du, dv));
        }
    }
    out
}

fn build_shield_leaf(index: usize, ncycle: usize) -> LeafMesh {
    let scale = if index == 0 { 1.24 } else { 1.18 };
    let phi = 0.83 * index as f32;
    let rad = (if index == 0 { 147.0 } else { 114.0 }) * scale;
    let center = if index == 0 {
        [-179.0f32, -58.0, -337.0]
    } else {
        // Right leaf positioned as FlowGarden's receiving catch leaf
        [178.0, 282.0, -282.0]
    };
    let reach = if index == 0 {
        [59.0f32, -165.0, -1080.0]
    } else {
        [62.0, 48.0, -1190.0]
    };
    let contact_xy = if index == 0 { [-45.0, 24.0] } else { [10.0, 15.0] };

    let len = LEAF_R * LEAF_A;
    let mut xs = vec![0.0f32; len];
    let mut ys = vec![0.0f32; len];
    let mut heights = vec![0.0f32; len];
    let mut r_vals = vec![0.0f32; len];
    let mut a_vals = vec![0.0f32; len];
    let mut colors = vec![[0.0f32; 3]; len];

    for ir in 0..LEAF_R {
        let r = 0.006 + 0.994 * (ir as f32 / (LEAF_R - 1) as f32);
        for ia in 0..LEAF_A {
            let a = (ia as f32 / LEAF_A as f32) * TAU;
            let idx = ir * LEAF_A + ia;
            let edge = 1.0
                + 0.048 * (5.0 * a + 0.5 + phi).sin()
                + 0.026 * (11.0 * a + phi).sin()
                + 0.018 * (3.0 * a - phi).cos();
            let lx = rad * r * edge * a.cos();
            let ly = rad * r * edge * a.sin() * if index == 0 { 0.89 } else { 0.82 };

            let sector = (a + 0.052 * (3.0 * a + phi).sin() * r + 0.021 * (7.0 * a).sin() * r * r) * 13.0 / TAU;
            let phase = (sector + 0.5).fract() - 0.5;
            let rib = (-((phase / (0.047 + 0.073 / (1.0 + 5.0 * r))).powi(2))).exp();
            let side = (r * 13.0 + phase.abs() * 1.46 + 0.17 * (3.0 * a + 2.0 * r).sin()).fract();
            let side_d = side.min(1.0 - side);
            let branches = (-((side_d / 0.054).powi(2))).exp() * (PI * phase.abs().min(0.5) * 2.0).sin();
            let bloom = 0.5 + 0.5 * (2.0 * a + 0.7 + phi).cos();
            let fleck = (97.0 * r + 1.8 * (31.0 * a).cos()).sin() * (57.0 * r + 4.0 * (29.0 * a).sin()).sin();
            let margin = (-(((1.0 - r) / 0.017).powi(2))).exp();
            let hub = (-((r / 0.082).powi(2))).exp();

            let mut h = (13.0 * r.powf(1.5)
                + 9.0 * r.powi(3) * (0.45 + (4.0 * a + 0.81 + phi).sin())
                + 4.0 * rib * r
                + 3.9 * r.powi(4) * (8.0 * a + phi).sin())
                * scale;
            h += if index == 0 {
                -0.12 * lx - 0.40 * ly
            } else {
                // FlowGarden Tilt: baseline + .30 x - .30 y + (.97 x + .53 y) - (.60 x + .03 y)
                0.67 * lx + 0.20 * ly
            };

            let mut col = [
                47.0 + bloom * 17.0 + rib * 41.0 + branches * 8.0 + margin * 20.0 + fleck * 2.0 + hub * 33.0,
                91.0 + bloom * 23.0 + rib * 40.0 + branches * 11.0 + margin * 25.0 + fleck * 2.7 + hub * 39.0,
                74.0 + bloom * 17.0 + rib * 25.0 + branches * 6.0 + margin * 15.0 + fleck * 1.7 + hub * 16.0,
            ];
            if index == 1 {
                col = [col[0] * 0.92, col[1] * 0.93, col[2] * 0.79];
            }

            xs[idx] = lx;
            ys[idx] = ly;
            heights[idx] = h;
            r_vals[idx] = r;
            a_vals[idx] = a;
            colors[idx] = col;
        }
    }

    let mut breeze = vec![0.0f32; ncycle];
    let mut tilt = 0.0f32;
    let mut vel = 0.0f32;
    for k in 0..(ncycle * 2) {
        let t = (k % ncycle) as f32 / CYCLE_RATE;
        let target = wind_at(t - 0.28 * index as f32) * 0.73;
        vel += (13.0 * (target - tilt) - 4.1 * vel) / CYCLE_RATE;
        tilt += vel / CYCLE_RATE;
        if k >= ncycle {
            breeze[k % ncycle] = tilt;
        }
    }

    LeafMesh {
        center,
        reach,
        x: xs,
        y: ys,
        height: heights,
        r_vals,
        a_vals,
        color: colors,
        breeze,
        contact_xy,
        radius_base: if index == 0 { 4.8 } else { 3.7 },
    }
}

impl AsciiLotusState {
    pub fn new(css_w: f32, css_h: f32, dpi_scale: f32) -> Self {
        let modes = fit_modes();
        let ref_ang = REF_DEG.to_radians();
        let phases = PHASE_SEED;

        // 1. Precompute exact `model.py::Surface` + `living.py::bud_and_open` for all 14 petals
        let mut sheets = Vec::with_capacity(PETAL_COUNT);
        for (i, pd) in PETALS.iter().enumerate() {
            let mut x_op = vec![0.0f32; NU * NV];
            let mut y_op = vec![0.0f32; NU * NV];
            let mut open_t_pts = vec![([0.0f32; 2], [0.0f32; 2], [0.0f32; 2]); NU];

            for iu in 0..NU {
                let u = iu as f32 / (NU - 1) as f32;
                let lo = bezier2(pd.root, pd.left[0], pd.left[1], pd.tip, u);
                let hi = bezier2(pd.root, pd.right[0], pd.right[1], pd.tip, u);
                let mid = [(hi[0] + lo[0]) * 0.5, (hi[1] + lo[1]) * 0.5];
                let half = [(hi[0] - lo[0]) * 0.5, (hi[1] - lo[1]) * 0.5];
                open_t_pts[iu] = (mid, half, lo);
            }

            let t_eval: f32 = if i >= 11 { 7.14 } else { 7.27 };
            let spread_q = model_response(t_eval, modes[0].0 + pd.lag, modes[0].1);
            let fall_q = model_response(t_eval, modes[1].0 + pd.lag, modes[1].1);
            let widen = pd.swell * spread_q;
            let dx = pd.travel[0] * spread_q;
            let dy = pd.travel[1] * fall_q;

            for iu in 0..NU {
                let u = iu as f32 / (NU - 1) as f32;
                let u0 = iu.saturating_sub(1);
                let u1 = (iu + 1).min(NU - 1);
                let du_step = ((u1 - u0) as f32 / (NU - 1) as f32).max(1e-4);
                let tx = (open_t_pts[u1].0[0] - open_t_pts[u0].0[0]) / du_step;
                let ty = (open_t_pts[u1].0[1] - open_t_pts[u0].0[1]) / du_step;
                let deriv = 1.55 * u.max(1e-4).powf(0.55);
                let turn = 0.78
                    * (deriv * (tx * dy - ty * dx))
                        .atan2(tx * tx + ty * ty + deriv * (tx * dx + ty * dy));
                let c = turn.cos() * (1.0 + widen);
                let s = turn.sin() * (1.0 + widen);
                let lever = u.powf(1.55);
                let bend_h = 3.0 * u * u * (1.0 - u);

                for iv in 0..NV {
                    let v = -1.0 + 2.0 * (iv as f32 / (NV - 1) as f32);
                    let cup = (PI * u).sin().max(0.0) * (1.0 - v * v);
                    let bx = open_t_pts[iu].0[0] + open_t_pts[iu].1[0] * v + 7.0 * cup;
                    let by = open_t_pts[iu].0[1] + open_t_pts[iu].1[1] * v - 10.0 * cup;
                    let side_x = open_t_pts[iu].1[0] * v;
                    let side_y = open_t_pts[iu].1[1] * v;
                    let unroll_x = bend_h * ((1.0 - v) * pd.open_l[0] + (1.0 + v) * pd.open_r[0]) * 0.5;
                    let unroll_y = bend_h * ((1.0 - v) * pd.open_l[1] + (1.0 + v) * pd.open_r[1]) * 0.5;

                    x_op[iu * NV + iv] = bx + lever * dx + (c - 1.0) * side_x - s * side_y + spread_q * unroll_x;
                    y_op[iu * NV + iv] = by + lever * dy + s * side_x + (c - 1.0) * side_y + spread_q * unroll_y;
                }
            }

            let phi = pd.depth.atan2(pd.tip[0] - pd.root[0]);
            let mut mean_span_x = 0.0f32;
            for iu in 0..NU {
                mean_span_x += x_op[iu * NV + (NV - 1)] - x_op[iu * NV];
            }
            mean_span_x /= NU as f32;
            let hand_probe = mean_span_x * (-phi.sin());
            let hand = if hand_probe == 0.0 { 1.0 } else { hand_probe.signum() };

            let mut bud = vec![[0.0f32; 3]; NU * NV];
            let mut opened = vec![[0.0f32; 3]; NU * NV];
            let mut long = 238.0 + 27.0 * (phi - 1.0).sin() + 14.0 * (phi * 2.1).cos() + 9.0 * (i as f32 * 3.1).sin();
            if i >= 11 {
                long -= 22.0;
            }

            for iu in 0..NU {
                let u = iu as f32 / (NU - 1) as f32;
                let crest = (PI * u).sin().max(0.0);
                let width = 0.5
                    * (x_op[iu * NV + NV - 1] - x_op[iu * NV])
                        .hypot(y_op[iu * NV + NV - 1] - y_op[iu * NV]);
                let rad_b = 11.0 * (1.0 - u) + 86.0 * crest.powf(0.82) + u * (8.0 + 16.0 * (phi + 0.3).sin().powi(2));
                for iv in 0..NV {
                    let v = -1.0 + 2.0 * (iv as f32 / (NV - 1) as f32);
                    let ripple = crest.powf(0.52)
                        * v.abs().powi(3)
                        * (2.8 * (15.0 * u + phases[i] + 3.0 * v).sin() + 1.1 * (38.0 * u + 2.0 * v).sin());
                    let idx = iu * NV + iv;
                    let zdepth = pd.depth * u.powf(0.96)
                        + hand * width * v * phi.cos() * 0.68
                        + 8.0 * crest * (1.0 - v * v);
                    let zheight = (zdepth * ref_ang.sin() - (y_op[idx] - 350.0)) / ref_ang.cos() + ripple;
                    opened[idx] = [x_op[idx] - 490.0, zdepth, zheight];

                    let theta_b = phi
                        + 0.36 * (1.0 - u)
                        + 0.41 * u * u
                        + hand * v * (0.87 + 0.16 * (i as f32).sin()) * (0.37 + 0.63 * crest.powf(0.40));
                    let off = 3.5 * (8.0 * u + phases[i]).sin() * crest * v * v;
                    let zb = long * u + 6.0 * v * crest * phi.cos() + off;
                    bud[idx] = [
                        rad_b * theta_b.cos() - 16.0 * u.powf(1.3),
                        rad_b * theta_b.sin(),
                        zb + ripple * 0.35,
                    ];
                }
            }

            let mid_u = NU / 2;
            let b_span = sub3(bud[mid_u * NV + NV - 1], bud[mid_u * NV]);
            let o_span = sub3(opened[mid_u * NV + NV - 1], opened[mid_u * NV]);
            if dot3(b_span, o_span) < 0.0 {
                for iu in 0..NU {
                    bud[iu * NV..(iu + 1) * NV].reverse();
                }
            }

            let rest_norm = compute_sheet_normals(&opened, NU, NV);
            sheets.push(PetalSheet { bud, opened, rest_norm });
        }

        // 2. Integrate 20s periodic spring `Cycle` (`day.py:39-106`)
        let ncycle = (PERIOD * CYCLE_RATE) as usize;
        let mut open_cycle = vec![[[0.0f32; NU]; PETAL_COUNT]; ncycle];
        let mut flex_cycle = vec![[0.0f32; PETAL_COUNT]; ncycle];
        let mut stem_cycle = vec![[0.0f32; 2]; ncycle];

        let mut delay = [0.0f32; PETAL_COUNT];
        let mut rise = [0.0f32; PETAL_COUNT];
        let mut closing = [0.0f32; PETAL_COUNT];
        let mut fall = [0.0f32; PETAL_COUNT];
        for i in 0..PETAL_COUNT {
            let ph = phases[i];
            let outer = i < 11;
            delay[i] = if outer { -0.55 + 0.54 * ph.sin() } else { 1.32 + 0.85 * ph.sin() };
            rise[i] = 1.45 + 0.59 * (1.0 + ph.cos());
            closing[i] = if outer { 16.1 + 0.85 * (ph * 1.4).sin() } else { 15.1 + 0.68 * ph.cos() };
            fall[i] = 2.4 + 0.36 * ph.cos();
        }

        let mut state_o = [[0.0f32; NU]; PETAL_COUNT];
        let mut vel_o = [[0.0f32; NU]; PETAL_COUNT];
        let mut angle_f = [0.0f32; PETAL_COUNT];
        let mut ang_v = [0.0f32; PETAL_COUNT];
        let mut stem = [0.0f32; 2];
        let mut stem_v = [0.0f32; 2];

        for step in 0..(ncycle * 2) {
            let fr = step % ncycle;
            let t = fr as f32 / CYCLE_RATE;
            let dawn = if t < 17.0 { t } else { t - PERIOD };
            let force = wind_at(t);
            let mut mean_exp = 0.0f32;
            let mut moment_bias = 0.0f32;
            for i in 0..PETAL_COUNT {
                let ph = phases[i];
                let mut exp_i = 0.0f32;
                for iu in 0..NU {
                    exp_i += state_o[i][iu];
                }
                exp_i /= NU as f32;
                mean_exp += exp_i;
                moment_bias += exp_i * ph.sin();

                let target = force * (0.25 + 0.75 * exp_i) * (0.56 + 0.53 * (ph + 0.7).sin());
                ang_v[i] += (18.0 * (target - angle_f[i]) - (3.9 + 0.5 * ph.cos()) * ang_v[i]) / CYCLE_RATE;
                angle_f[i] += ang_v[i] / CYCLE_RATE;

                for iu in 0..NU {
                    let u = iu as f32 / (NU - 1) as f32;
                    let opening = smooth((dawn - delay[i] - 0.36 * u) / rise[i]);
                    let day_g = 1.0 - smooth((t - closing[i] - 0.28 * u) / fall[i]);
                    let goal = if t < 17.0 { day_g.min(opening) } else { day_g.max(opening) };
                    let stiff = 27.0 + 4.0 * ph.cos() - 9.0 * u;
                    let drag = 2.0 * stiff.sqrt() * (0.78 + 0.09 * ph.cos());
                    vel_o[i][iu] += (stiff * (goal - state_o[i][iu]) - drag * vel_o[i][iu]) / CYCLE_RATE;
                    state_o[i][iu] += vel_o[i][iu] / CYCLE_RATE;
                }
            }
            mean_exp /= PETAL_COUNT as f32;
            moment_bias /= PETAL_COUNT as f32;
            let m0 = 49.0 * force * (0.58 + 0.42 * mean_exp) + 9.0 * moment_bias;
            let m1 = 19.0 * wind_at(t - 0.58) * (0.58 + 0.42 * mean_exp);
            stem_v[0] += (11.0 * (m0 - stem[0]) - 3.5 * stem_v[0]) / CYCLE_RATE;
            stem_v[1] += (11.0 * (m1 - stem[1]) - 3.5 * stem_v[1]) / CYCLE_RATE;
            stem[0] += stem_v[0] / CYCLE_RATE;
            stem[1] += stem_v[1] / CYCLE_RATE;
            if step >= ncycle {
                open_cycle[fr] = state_o;
                flex_cycle[fr] = angle_f;
                stem_cycle[fr] = stem;
            }
        }

        // 3. Shield leaves & elastic grass blades
        let leaves = [build_shield_leaf(0, ncycle), build_shield_leaf(1, ncycle)];
        const GRASS_SEG: usize = 16;
        let sim_ticks = (PERIOD * SIM_RATE) as usize;
        let mut grass_axes = vec![Vec::new(); sim_ticks];
        for gt in &mut grass_axes {
            *gt = vec![[0.0; 3]; 5 * GRASS_SEG];
        }
        for (bi, gd) in GRASS_BLADES.iter().enumerate() {
            let mut p = [0.0f32; 2];
            let mut v = [0.0f32; 2];
            for k in 0..(sim_ticks * 2) {
                let tick = k % sim_ticks;
                let t = tick as f32 / SIM_RATE;
                let w = wind_at(t - 0.14 * bi as f32);
                let fx = gd.drag * 52.0 * w;
                let fy = gd.drag * 18.0 * wind_at(t - 0.45);
                v[0] += ((fx - gd.stiffness * 36.0 * p[0]) - gd.damping * v[0]) / SIM_RATE;
                v[1] += ((fy - gd.stiffness * 36.0 * p[1]) - gd.damping * v[1]) / SIM_RATE;
                p[0] += v[0] / SIM_RATE;
                p[1] += v[1] / SIM_RATE;
                if k >= sim_ticks {
                    let base_i = bi * GRASS_SEG;
                    for si in 0..GRASS_SEG {
                        let s = si as f32 / (GRASS_SEG - 1) as f32;
                        let curve = s * s * (1.5 - 0.5 * s);
                        grass_axes[tick][base_i + si] = [
                            gd.root[0] + s.powf(1.45) * gd.lean[0] + curve * p[0],
                            gd.root[1] + s.powf(1.45) * gd.lean[1] + curve * p[1],
                            gd.root[2] + gd.length * s,
                        ];
                    }
                }
            }
        }

        // 4. 156 Golden Stamens (`living.py:33-37`)
        let mut grng = Rng64::new(47);
        let mut anther_dir = Vec::with_capacity(STAMEN_N);
        let mut anther_col = Vec::with_capacity(STAMEN_N);
        let mut anther_len = Vec::with_capacity(STAMEN_N);
        let mut anther_r = Vec::with_capacity(STAMEN_N);
        let mut anther_ang = Vec::with_capacity(STAMEN_N);
        for _ in 0..STAMEN_N {
            let a = grng.range(0.0, TAU);
            let fl = grng.range(23.0, 49.0);
            let fp = grng.range(0.65, 1.1);
            let ft = grng.range(-0.16, 0.17);
            let fr = 0.50 + 0.50 * grng.f32().sqrt();
            anther_ang.push(a);
            anther_len.push(fl);
            anther_r.push(fr);
            anther_dir.push([fp, ft, 0.0]);
            anther_col.push([
                236.0 + 16.0 * fr,
                144.0 + 49.0 * fp,
                54.0 + 44.0 * a.sin().powi(2),
            ]);
        }

        // 5. PassingLight canopy sweep travel (`passing_light.py:22-25`)
        let mut canopy_dist = Vec::with_capacity(sim_ticks + 1);
        let mut acc = -270.0f32;
        for step in 0..=sim_ticks {
            let t = step as f32 / SIM_RATE;
            canopy_dist.push(acc);
            let gust = 35.0 * wind_at(t);
            let spd = 48.0 + 0.10 * gust;
            acc += spd / SIM_RATE;
        }

        let params = AsciiLabParams::default();
        let mut canvas = AsciiCanvas::default();
        canvas.ensure_layout(css_w, css_h, (params.char_size * 0.72).clamp(4.2, 12.0), false);

        let mut st = Self {
            params,
            canvas,
            anim_time: 9.2, // Full blooming lotus + sweeping sunbeam + bee + dew
            low_mode: false,
            icons: Vec::new(),
            icons_vis: false,
            css_w,
            css_h,
            dpi_scale,
            pointer_norm: [0.5, 0.5],
            pointer_active: false,
            breeze_gust: 0.0,
            user_drops: Vec::new(),
            phases,
            sheets,
            open_cycle,
            flex_cycle,
            stem_cycle,
            leaves,
            grass_axes,
            anther_dir,
            anther_col,
            anther_len,
            anther_r,
            anther_ang,
            canopy_dist,
            pond_w: 0,
            pond_h: 0,
            pond_base: Vec::new(),
            rast_rgb: Vec::new(),
            rast_z: Vec::new(),
            rast_petal: Vec::new(),
            shade_buf: Vec::new(),
            light_buf: Vec::new(),
            bee_lure: [0.5, 0.5],
            bee_lure_blend: 0.0,
        };
        st.rebuild_grid();
        st
    }

    pub fn resize(&mut self, css_w: f32, css_h: f32, dpi: f32) {
        self.css_w = css_w;
        self.css_h = css_h;
        self.dpi_scale = dpi;
        let eff_cs = (self.params.char_size * 0.72).clamp(4.2, 12.0);
        if self.canvas.ensure_layout(css_w, css_h, eff_cs, self.low_mode) {
            self.rebuild_grid();
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            let eff_cs = (self.params.char_size * 0.72).clamp(4.2, 12.0);
            self.canvas.ensure_layout(self.css_w, self.css_h, eff_cs, low);
            self.rebuild_grid();
        }
    }

    pub fn sync_params(&mut self, next: AsciiLabParams) {
        let changed = self.params != next;
        self.params = next;
        let eff_cs = (self.params.char_size * 0.72).clamp(4.2, 12.0);
        self.canvas.ensure_layout(self.css_w, self.css_h, eff_cs, self.low_mode);
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
        self.pointer_norm = [nx, ny];
        self.pointer_active = true;
        self.breeze_gust = (self.breeze_gust + dx * 6.5).clamp(-1.8, 1.8);
        self.bee_lure = [nx, ny];
        if kind == 1 {
            if self.user_drops.len() >= 6 {
                self.user_drops.remove(0);
            }
            self.user_drops.push(UserDrop {
                active: true,
                uv: [nx, ny.clamp(0.72, 0.96)],
                age: 0.0,
            });
            return true;
        }
        false
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.anim_time = 9.2;
        self.breeze_gust = 0.0;
        self.user_drops.clear();
        self.bee_lure_blend = 0.0;
        self.rebuild_grid();
    }

    pub fn step(&mut self, dt: f32) {
        let dt_eff = dt.clamp(0.0, 0.05) * self.params.speed;
        self.anim_time = (self.anim_time + dt_eff) % PERIOD;
        self.breeze_gust *= (-dt * 2.2).exp();
        if self.pointer_active && self.params.mouse_interact {
            self.bee_lure_blend = (self.bee_lure_blend + dt * 1.8).min(0.65);
        } else {
            self.bee_lure_blend = (self.bee_lure_blend - dt * 1.4).max(0.0);
        }
        for d in &mut self.user_drops {
            d.age += dt;
            if d.age > 5.4 {
                d.active = false;
            }
        }
        self.user_drops.retain(|d| d.active);
        self.rebuild_grid();
    }

    fn palette_tint(&self, base_rgb: [f32; 3]) -> [f32; 3] {
        match self.params.palette {
            1 => [base_rgb[1] * 0.72 + 28.0, base_rgb[0] * 0.85, base_rgb[2] * 1.12],
            2 => [(base_rgb[0] * 1.18).min(255.0), base_rgb[1] * 0.92 + 16.0, base_rgb[2] * 0.82],
            _ => base_rgb,
        }
    }

    fn ensure_pond_base(&mut self, rw: usize, rh: usize) {
        if self.pond_w == rw && self.pond_h == rh && self.pond_base.len() == rw * rh {
            return;
        }
        self.pond_w = rw;
        self.pond_h = rh;
        self.pond_base.resize(rw * rh, [4.0, 8.0, 12.0]);
        let mut rng = Rng64::new(528);

        for py in 0..rh {
            let ny = py as f32 / rh as f32;
            let sky = (-((0.5f32 - 0.18) / 0.78).powi(2) - ((ny - 0.36) / 0.63).powi(2)).exp();
            let water = smooth((ny - 0.65) / 0.30);
            for px in 0..rw {
                let nx = px as f32 / rw as f32;
                let sky_p = (-((nx - 0.18) / 0.78).powi(2) - ((ny - 0.36) / 0.63).powi(2)).exp();
                let s = 0.5 * sky + 0.5 * sky_p;
                self.pond_base[py * rw + px] = [
                    4.0 + s * 10.0 + water * 3.0,
                    8.0 + s * 19.0 + water * 8.0,
                    12.0 + s * 17.0 + water * 13.0,
                ];
            }
        }

        // Distant soft unfocused bank reeds (`day.py:120-128`)
        let mut reed_buf = vec![[0.0f32; 3]; rw * rh];
        for side in [-1.0f32, 1.0f32] {
            for _ in 0..18 {
                let x = if side < 0.0 {
                    rng.range(-0.08 * rw as f32, 0.24 * rw as f32)
                } else {
                    rng.range(0.76 * rw as f32, 1.08 * rw as f32)
                };
                let y = rng.range(0.35 * rh as f32, 0.80 * rh as f32);
                let tilt = rng.range(-0.10 * rw as f32, 0.10 * rw as f32);
                let f = rng.range(0.4, 1.1);
                let col = [8.0 * f, 16.0 * f, 7.0 * f];
                let steps = 48usize;
                for s_k in 0..steps {
                    let u = s_k as f32 / (steps - 1) as f32;
                    let py_f = y + (1.0 - u) * (0.20 * rh as f32) - u * (0.25 * rh as f32);
                    let px_f = x + u * tilt * 1.5;
                    let half_w = (4.0 * (1.0 - 0.7 * u)).max(1.0) * (rw as f32 / 220.0);
                    let y0 = (py_f.round() as i32).clamp(0, rh as i32 - 1);
                    let x0 = ((px_f - half_w).floor() as i32).max(0);
                    let x1 = ((px_f + half_w).ceil() as i32).min(rw as i32 - 1);
                    for px_i in x0..=x1 {
                        let idx = y0 as usize * rw + px_i as usize;
                        reed_buf[idx][0] = reed_buf[idx][0].max(col[0]);
                        reed_buf[idx][1] = reed_buf[idx][1].max(col[1]);
                        reed_buf[idx][2] = reed_buf[idx][2].max(col[2]);
                    }
                }
            }
        }
        // Soft box blur on reeds + fold into pond_base
        for py in 1..(rh - 1) {
            for px in 1..(rw - 1) {
                let mut sum = [0.0f32; 3];
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let s = reed_buf[(py as i32 + dy) as usize * rw + (px as i32 + dx) as usize];
                        sum[0] += s[0];
                        sum[1] += s[1];
                        sum[2] += s[2];
                    }
                }
                let idx = py * rw + px;
                self.pond_base[idx][0] += sum[0] / 9.0;
                self.pond_base[idx][1] += sum[1] / 9.0;
                self.pond_base[idx][2] += sum[2] / 9.0;
            }
        }

        // Distant blurred water lily pads (`day.py:129-140`)
        let pad_defs = [
            (0.08 * rw as f32, 0.77 * rh as f32, 0.16 * rw as f32, 0.046 * rh as f32, 0.08f32),
            (0.87 * rw as f32, 0.73 * rh as f32, 0.10 * rw as f32, 0.022 * rh as f32, -0.10),
            (0.92 * rw as f32, 0.90 * rh as f32, 0.18 * rw as f32, 0.070 * rh as f32, -0.05),
            (0.16 * rw as f32, 0.94 * rh as f32, 0.14 * rw as f32, 0.044 * rh as f32, 0.12),
        ];
        for &(cx, cy, rx, ry, ang) in &pad_defs {
            let ca = ang.cos();
            let sa = ang.sin();
            let x0 = ((cx - rx * 1.15).floor() as i32).max(0);
            let x1 = ((cx + rx * 1.15).ceil() as i32).min(rw as i32 - 1);
            let y0 = ((cy - ry * 1.15).floor() as i32).max(0);
            let y1 = ((cy + ry * 1.15).ceil() as i32).min(rh as i32 - 1);
            for py in y0..=y1 {
                for px in x0..=x1 {
                    let dx = px as f32 - cx;
                    let dy = py as f32 - cy;
                    let lx = (dx * ca + dy * sa) / rx.max(1.0);
                    let ly = (-dx * sa + dy * ca) / ry.max(1.0);
                    let a = ly.atan2(lx);
                    let r = lx.hypot(ly) / (1.0 + 0.031 * (5.0 * a + 1.0).sin() + 0.017 * (13.0 * a).sin());
                    if r <= 1.0 && a.rem_euclid(TAU) > 0.2 && a.rem_euclid(TAU) < TAU - 0.16 {
                        let rim = smooth((r - 0.88) / 0.12);
                        let vein = (0.5 + 0.5 * (a * 16.0).cos()).powi(6) * 0.25;
                        let idx = py as usize * rw + px as usize;
                        let f = 1.0 - 0.35 * smooth((r - 0.92) / 0.08);
                        let pr = 20.0 + rim * 13.0 + vein * 6.0;
                        let pg = 42.0 + rim * 14.0 + vein * 8.0;
                        let pb = 36.0 + rim * 9.0 + vein * 4.0;
                        self.pond_base[idx][0] = self.pond_base[idx][0] * (1.0 - f) + pr * f;
                        self.pond_base[idx][1] = self.pond_base[idx][1] * (1.0 - f) + pg * f;
                        self.pond_base[idx][2] = self.pond_base[idx][2] * (1.0 - f) + pb * f;
                    }
                }
            }
        }
    }

    pub fn rebuild_grid(&mut self) {
        let cols = self.canvas.cols;
        let rows = self.canvas.rows;
        if cols == 0 || rows == 0 {
            return;
        }

        // Supersampled 2x cell rasterizer in normal mode (1x in low_mode) matching `ascii_preview.py`
        let ss = if self.low_mode { 1usize } else { 2usize };
        let rw = cols * ss;
        let rh = rows * ss;
        let rlen = rw * rh;
        self.ensure_pond_base(rw, rh);
        self.rast_rgb.resize(rlen, [0.0; 3]);
        self.rast_z.resize(rlen, -1e6);
        self.rast_petal.resize(rlen, 0);
        self.rast_rgb.copy_from_slice(&self.pond_base);
        self.rast_z.fill(-1e6);
        self.rast_petal.fill(0);

        let t = self.anim_time;
        let theta = TAU * t / PERIOD;
        let cyc_step = ((t * CYCLE_RATE).round() as usize) % self.open_cycle.len();
        let sim_tick = ((t * SIM_RATE).round() as usize) % self.grass_axes.len();
        let stem_load = self.stem_cycle[cyc_step];
        let load = [stem_load[0] + self.breeze_gust * 18.0, stem_load[1]];

        // `VisitGarden('catch', 2)` camera (`az = -15°`, `elev = 37°`, `roll = -4°`)
        let az = (-15.0f32).to_radians()
            + if self.pointer_active && self.params.mouse_interact {
                (self.pointer_norm[0] - 0.5) * 0.16
            } else {
                0.0
            };
        let elev = (37.0f32).to_radians()
            + if self.pointer_active && self.params.mouse_interact {
                (self.pointer_norm[1] - 0.5) * 0.08
            } else {
                0.0
            };
        let roll = (-4.0f32).to_radians();
        let scale = (rh as f32 / 550.0).min(rw as f32 / 460.0) * 0.62;
        let xmid = rw as f32 * 0.50;
        let ymid = rh as f32 * 0.41;
        let (ca, sa) = (az.cos(), az.sin());
        let (ce, se) = (elev.cos(), elev.sin());
        let (cr, sr) = (roll.cos(), roll.sin());
        let eye = [az.sin() * elev.cos(), az.cos() * elev.cos(), elev.sin()];

        let project = |p: [f32; 3]| -> [f32; 3] {
            let bend_f = ((p[2] + 760.0) / 760.0).clamp(0.0, 1.0);
            let bend = bend_f * bend_f * (3.0 - bend_f) * 0.5 + p[2].max(0.0) / 510.0;
            let x = p[0] + load[0] * bend;
            let d = p[1] + load[1] * bend;
            let xx = x * ca - d * sa;
            let dd = x * sa + d * ca;
            let yy = dd * se - p[2] * ce;
            let dep = dd * ce + p[2] * se;
            [
                xmid + scale * (cr * xx - sr * yy),
                ymid + scale * (sr * xx + cr * yy),
                dep,
            ]
        };

        // Distant pond wave shimmer on bottom water zone (`day.py:248-250`)
        for py in 0..rh {
            let ny = py as f32 / rh as f32;
            let water_w = smooth((ny - 0.65) / 0.25);
            if water_w < 0.01 {
                continue;
            }
            let warp = 0.8 * (ny * 68.0 + theta * 4.0).sin() + 1.25 * (ny * 34.0 - theta * 3.0).sin();
            let row_off = py * rw;
            let src_row = self.pond_base[row_off..row_off + rw].to_vec();
            for px in 0..rw {
                let sx = ((px as f32 + warp * water_w).round() as i32).clamp(0, rw as i32 - 1) as usize;
                let s = src_row[sx];
                self.rast_rgb[row_off + px] = [
                    self.pond_base[row_off + px][0] * (1.0 - water_w) + s[0] * water_w,
                    self.pond_base[row_off + px][1] * (1.0 - water_w) + s[1] * water_w,
                    self.pond_base[row_off + px][2] * (1.0 - water_w) + s[2] * water_w,
                ];
            }
        }

        // 1. 3D Cylindrical Lotus Stem (`living.py:243-250` + `day.py:191-202`)
        {
            const NS: usize = 30;
            const NR: usize = 8;
            let mut s_pts = vec![([0.0f32; 3], [0.0f32; 3]); NS * (NR + 1)];
            for ia in 0..NS {
                let a = (ia as f32 / (NS - 1) as f32) * 1.85;
                let radius = if a <= 1.0 {
                    6.1 + 3.0 * a + 2.4 * (-(((a - 0.025) / 0.040).powi(2))).exp()
                } else {
                    9.1 + 2.2 * (a - 1.0)
                };
                let (cx, cy, cz) = if a <= 1.0 {
                    (
                        26.0 * (3.8 * a + 0.08).sin() - 2.0 + 91.0 * a.powf(1.27),
                        9.0 * a - 12.0,
                        -815.0 * a,
                    )
                } else {
                    let s = a - 1.0;
                    (
                        26.0 * 3.88f32.sin() - 2.0 + 91.0 + s * (26.0 * 3.8 * 3.88f32.cos() + 91.0 * 1.27),
                        -3.0 + 9.0 * s,
                        -815.0 * (1.0 + s),
                    )
                };
                for it in 0..=NR {
                    let th = (it as f32 / NR as f32) * TAU;
                    let c_th = 0.38 + 0.54 * (0.5 + 0.5 * (th - 2.2).cos()) + 0.07 * (th * 5.0).sin();
                    let col = [99.0 * c_th, 121.0 * c_th, 59.0 * c_th];
                    s_pts[ia * (NR + 1) + it] = (project([cx + radius * th.cos(), cy + radius * th.sin(), cz]), col);
                }
            }
            for ia in 0..(NS - 1) {
                for it in 0..NR {
                    let a = s_pts[ia * (NR + 1) + it];
                    let b = s_pts[(ia + 1) * (NR + 1) + it];
                    let c = s_pts[ia * (NR + 1) + it + 1];
                    let d = s_pts[(ia + 1) * (NR + 1) + it + 1];
                    raster_tri(a, b, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    raster_tri(b, d, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                }
            }
        }

        // 2. Five `BankGrass` Blades (`bank_grass.py:10-48`)
        {
            const GRASS_SEG: usize = 16;
            let leaf_sun = unit3([-0.46, 0.20, 0.86]);
            let axes_slice = &self.grass_axes[sim_tick];
            for (bi, gd) in GRASS_BLADES.iter().enumerate() {
                let axis = &axes_slice[bi * GRASS_SEG..(bi + 1) * GRASS_SEG];
                let ang = gd.heading_deg.to_radians();
                let lat = [ang.cos(), ang.sin(), 0.0];
                let col_lit = [gd.pigment[0] * 0.66, gd.pigment[1] * 0.94, gd.pigment[2] * 0.74];
                let mut ribbon = vec![([0.0f32; 3], [0.0f32; 3]); GRASS_SEG * 3];
                for si in 0..GRASS_SEG {
                    let s = si as f32 / (GRASS_SEG - 1) as f32;
                    let s0 = si.saturating_sub(1);
                    let s1 = (si + 1).min(GRASS_SEG - 1);
                    let tang = unit3(sub3(axis[s1], axis[s0]));
                    let dot_tl = dot3(tang, lat);
                    let across = unit3([lat[0] - dot_tl * tang[0], lat[1] - dot_tl * tang[1], lat[2] - dot_tl * tang[2]]);
                    let fold_n = cross3(tang, across);
                    let w_s = if si == GRASS_SEG - 1 {
                        0.0
                    } else {
                        gd.half_width * (PI * (0.16 + 0.84 * s)).sin().powf(0.8)
                    };
                    let shade = 0.72 + 0.36 * dot3(fold_n, leaf_sun).max(0.0);
                    let col = [col_lit[0] * shade, col_lit[1] * shade, col_lit[2] * shade];
                    let fold = [fold_n[0] * w_s * 0.13, fold_n[1] * w_s * 0.13, fold_n[2] * w_s * 0.13];
                    let edge = [across[0] * w_s, across[1] * w_s, across[2] * w_s];
                    ribbon[si * 3] = (project(sub3(axis[si], edge)), col);
                    ribbon[si * 3 + 1] = (project(add3(axis[si], fold)), [col[0] * 1.12, col[1] * 1.12, col[2] * 1.12]);
                    ribbon[si * 3 + 2] = (project(add3(axis[si], edge)), col);
                }
                for si in 0..(GRASS_SEG - 1) {
                    let l0 = ribbon[si * 3];
                    let m0 = ribbon[si * 3 + 1];
                    let r0 = ribbon[si * 3 + 2];
                    let l1 = ribbon[(si + 1) * 3];
                    let m1 = ribbon[(si + 1) * 3 + 1];
                    let r1 = ribbon[(si + 1) * 3 + 2];
                    raster_tri(l0, l1, m0, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    raster_tri(m0, l1, m1, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    raster_tri(m0, m1, r0, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    raster_tri(r0, m1, r1, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                }
            }
        }

        // Leaf load on right leaf from falling dew bead (t ~ 9.1..10.6s)
        let leaf_load_r = if t >= 9.1 && t <= 11.2 {
            let f = ((t - 9.1) / 2.1).clamp(0.0, 1.0);
            (PI * f).sin().powi(2) * 0.85
        } else {
            0.0
        };

        // 3. Two Broad Veined Shield Leaves + Cylindrical Stalks (`visitor_leaves.py` + `broad_leaves.py`)
        let leaf_sun = unit3([-0.46, 0.20, 0.86]);
        let mut leaf_r_surface_pt = [0.0f32; 3];
        for (l_idx, leaf) in self.leaves.iter().enumerate() {
            let beta = leaf.breeze[cyc_step] + self.breeze_gust * 0.32;
            let leaf_load = if l_idx == 1 { leaf_load_r } else { 0.0 };
            let center = [
                leaf.center[0] + beta * (17.0 - 2.0 * l_idx as f32),
                leaf.center[1] + beta * 5.0,
                leaf.center[2] - 6.0 * leaf_load,
            ];

            // Stalk cylinder
            const KS: usize = 16;
            const KTH: usize = 7;
            let mut stalk_pts = vec![([0.0f32; 3], [0.0f32; 3]); KS * (KTH + 1)];
            for ik in 0..KS {
                let s = ik as f32 / (KS - 1) as f32;
                let rad = leaf.radius_base * (1.14 - 0.28 * s);
                let ax = [
                    leaf.reach[0] * (1.0 - s) + center[0] * s + (PI * s).sin() * if l_idx == 0 { 30.0 } else { -27.0 },
                    leaf.reach[1] * (1.0 - s) + center[1] * s - 14.0 * (PI * s).sin(),
                    leaf.reach[2] * (1.0 - s) + center[2] * s,
                ];
                for it in 0..=KTH {
                    let th = (it as f32 / KTH as f32) * TAU;
                    let tint = 0.56 + 0.39 * (0.5 + 0.5 * (th - 2.0).cos()) + 0.04 * (th * 6.0).sin();
                    stalk_pts[ik * (KTH + 1) + it] = (
                        project([ax[0] + rad * th.cos(), ax[1] + rad * th.sin(), ax[2]]),
                        [79.0 * tint, 112.0 * tint, 72.0 * tint],
                    );
                }
            }
            for ik in 0..(KS - 1) {
                for it in 0..KTH {
                    let a = stalk_pts[ik * (KTH + 1) + it];
                    let b = stalk_pts[(ik + 1) * (KTH + 1) + it];
                    let c = stalk_pts[ik * (KTH + 1) + it + 1];
                    let d = stalk_pts[(ik + 1) * (KTH + 1) + it + 1];
                    raster_tri(a, b, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    raster_tri(b, d, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                }
            }

            // World vertices for blade & normals
            let len = LEAF_R * LEAF_A;
            let mut world_pts = vec![[0.0f32; 3]; len];
            for idx in 0..len {
                let lx = leaf.x[idx];
                let ly = leaf.y[idx];
                let r = leaf.r_vals[idx];
                let a = leaf.a_vals[idx];
                let deform = (0.10 * beta * lx + 0.055 * beta * ly) * r;
                let ld_patch = (-((lx - leaf.contact_xy[0]) / 58.0).powi(2) - ((ly - leaf.contact_xy[1]) / 49.0).powi(2)).exp();
                let lz = leaf.height[idx] + deform - leaf_load * (22.0 * ld_patch + 3.0 * r) + 1.8 * beta * r * r * (6.0 * a).sin();
                world_pts[idx] = [center[0] + lx, center[1] + ly, center[2] + lz];
            }
            if l_idx == 1 {
                leaf_r_surface_pt = world_pts[10 * LEAF_A + 4];
            }

            let mut l_verts = vec![([0.0f32; 3], [0.0f32; 3]); len];
            for ir in 0..LEAF_R {
                let r0 = ir.saturating_sub(1);
                let r1 = (ir + 1).min(LEAF_R - 1);
                for ia in 0..LEAF_A {
                    let ia0 = (ia + LEAF_A - 1) % LEAF_A;
                    let ia1 = (ia + 1) % LEAF_A;
                    let idx = ir * LEAF_A + ia;
                    let dr = sub3(world_pts[r1 * LEAF_A + ia], world_pts[r0 * LEAF_A + ia]);
                    let da = sub3(world_pts[ir * LEAF_A + ia1], world_pts[ir * LEAF_A + ia0]);
                    let n = unit3(cross3(dr, da));
                    let sun_dot = dot3(n, leaf_sun).abs(); // normal might flip under cross order
                    let shade = 0.78 + 0.32 * sun_dot;
                    let c = leaf.color[idx];
                    l_verts[idx] = (project(world_pts[idx]), [c[0] * shade, c[1] * shade, c[2] * shade]);
                }
            }
            for ir in 0..(LEAF_R - 1) {
                for ia in 0..LEAF_A {
                    let ia_next = (ia + 1) % LEAF_A;
                    let a = l_verts[ir * LEAF_A + ia];
                    let b = l_verts[(ir + 1) * LEAF_A + ia];
                    let c = l_verts[ir * LEAF_A + ia_next];
                    let d = l_verts[(ir + 1) * LEAF_A + ia_next];
                    raster_tri(a, b, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    raster_tri(b, d, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                }
            }
        }

        // 4. 14 Purple Lotus Petals (Exact SLERP `bend()`) + Stamens + Receptacle + `PetalLight`
        let sun = unit3([-0.53 + 0.34 * (theta - 0.6).sin(), 0.37, 0.87]);
        let key = unit3([sun[0] - 0.40, sun[1] - 0.73, sun[2] - 0.16]);
        let half = unit3(add3(key, eye));
        let light_base = unit3([-0.46, 0.35, 0.82]);

        let mut opening_mean = 0.0f32;
        for i in 0..PETAL_COUNT {
            for iu in 0..NU {
                opening_mean += self.open_cycle[cyc_step][i][iu];
            }
        }
        opening_mean /= (PETAL_COUNT * NU) as f32;
        let closed_frac = 1.0 - smooth(opening_mean / 0.76);

        let mid_v = NV / 2;
        let mut deformed_pts = vec![[0.0f32; 3]; NU * NV];
        let mut p_pts = vec![([0.0f32; 3], [0.0f32; 3]); NU * NV];
        let mut petal9_rim = [0.0f32; 3];
        let mut petal10_world = [0.0f32; 3];

        for i in 0..PETAL_COUNT {
            if i == 11 {
                // Receptacle Cushion + 156 Golden Stamens (`living.py:252-272`)
                let spread = 0.32 + 0.68 * smooth(opening_mean * 1.6);
                for s_k in 0..STAMEN_N {
                    let a = self.anther_ang[s_k];
                    let (ex, ey) = (a.cos(), a.sin());
                    let fr = self.anther_r[s_k];
                    let fp = self.anther_dir[s_k][0];
                    let ft = self.anther_dir[s_k][1];
                    let radial = fp * (0.14 + 0.86 * opening_mean);
                    let root = [
                        (75.0 * ex * fr - 20.0) * spread,
                        (51.0 * ey * fr + 7.0) * spread,
                        76.0 + 7.0 * spread + spread * fr * (30.0 * ex - 7.0 * ey),
                    ];
                    let fl = self.anther_len[s_k];
                    let tip = [
                        root[0] + fl * 0.7 * radial * (a + ft).cos(),
                        root[1] + fl * 0.7 * radial * (a + ft).sin(),
                        root[2] + fl * 0.93,
                    ];
                    let mut p0 = project(root);
                    let mut p1 = project(tip);
                    p0[2] += 68.0 * opening_mean;
                    p1[2] += 68.0 * opening_mean;
                    raster_stamen(
                        p0,
                        p1,
                        self.anther_col[s_k],
                        rw,
                        rh,
                        &mut self.rast_z,
                        &mut self.rast_rgb,
                    );
                }

                // Golden Receptacle dome (`living.py:268-271`)
                const RR: usize = 8;
                const RTH: usize = 20;
                let mut rec_verts = vec![([0.0f32; 3], [0.0f32; 3]); RR * (RTH + 1)];
                for ir in 0..RR {
                    let r = ir as f32 / (RR - 1) as f32;
                    let shade = 0.8 + 0.2 * (1.0 - r * r);
                    let col = [224.0 * shade, 171.0 * shade, 67.0 * shade];
                    for it in 0..=RTH {
                        let th = (it as f32 / RTH as f32) * TAU;
                        let x = (44.0 * r * th.cos() - 16.0) * spread;
                        let d = (33.0 * r * th.sin() - 11.0) * spread;
                        let z = 74.0 + 7.0 * (1.0 - r * r) + 0.39 * x;
                        rec_verts[ir * (RTH + 1) + it] = (project([x, d, z]), col);
                    }
                }
                for ir in 0..(RR - 1) {
                    for it in 0..RTH {
                        let a = rec_verts[ir * (RTH + 1) + it];
                        let b = rec_verts[(ir + 1) * (RTH + 1) + it];
                        let c = rec_verts[ir * (RTH + 1) + it + 1];
                        let d = rec_verts[(ir + 1) * (RTH + 1) + it + 1];
                        raster_tri(a, b, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                        raster_tri(b, d, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    }
                }
            }

            let angle_f = self.flex_cycle[cyc_step][i] + self.breeze_gust * 0.32;
            let ph = self.phases[i];
            let sh = &self.sheets[i];
            let qs = &self.open_cycle[cyc_step][i];

            // Exact SLERP Midrib rebuild (`living.py:140-156`)
            let q0 = qs[0];
            let b0 = sh.bud[mid_v];
            let o0 = sh.opened[mid_v];
            let mut midrib = [[0.0f32; 3]; NU];
            midrib[0] = [
                (1.0 - q0) * b0[0] + q0 * o0[0],
                (1.0 - q0) * b0[1] + q0 * o0[1],
                (1.0 - q0) * b0[2] + q0 * o0[2],
            ];
            for iu in 0..(NU - 1) {
                let bt = sub3(sh.bud[(iu + 1) * NV + mid_v], sh.bud[iu * NV + mid_v]);
                let ot = sub3(sh.opened[(iu + 1) * NV + mid_v], sh.opened[iu * NV + mid_v]);
                let bl = norm3(bt);
                let ol = norm3(ot);
                let bb = unit3(bt);
                let oo = unit3(ot);
                let f = qs[iu + 1];
                let angle = dot3(bb, oo).clamp(-0.9999, 0.9999).acos();
                let tang = if angle < 1e-4 {
                    unit3([
                        (1.0 - f) * bb[0] + f * oo[0],
                        (1.0 - f) * bb[1] + f * oo[1],
                        (1.0 - f) * bb[2] + f * oo[2],
                    ])
                } else {
                    let s_ang = angle.sin();
                    let w0 = ((1.0 - f) * angle).sin() / s_ang;
                    let w1 = (f * angle).sin() / s_ang;
                    [
                        w0 * bb[0] + w1 * oo[0],
                        w0 * bb[1] + w1 * oo[1],
                        w0 * bb[2] + w1 * oo[2],
                    ]
                };
                let seg_len = (1.0 - f) * bl + f * ol;
                midrib[iu + 1] = [
                    midrib[iu][0] + tang[0] * seg_len,
                    midrib[iu][1] + tang[1] * seg_len,
                    midrib[iu][2] + tang[2] * seg_len,
                ];
            }

            for iu in 0..NU {
                let u = iu as f32 / (NU - 1) as f32;
                let q = qs[iu];
                let sq = q.clamp(0.0, 1.0);
                let bm = sh.bud[iu * NV + mid_v];
                let om = sh.opened[iu * NV + mid_v];
                for iv in 0..NV {
                    let v = -1.0 + 2.0 * (iv as f32 / (NV - 1) as f32);
                    let edge = (q - (0.17 + 0.05 * ph.sin()) * v.abs() * (PI * sq).sin()).clamp(0.0, 1.2);
                    let bi = sub3(sh.bud[iu * NV + iv], bm);
                    let op = sub3(sh.opened[iu * NV + iv], om);
                    let mut p = [
                        midrib[iu][0] + bi[0] * (1.0 - edge) + op[0] * edge,
                        midrib[iu][1] + bi[1] * (1.0 - edge) + op[1] * edge,
                        midrib[iu][2] + bi[2] * (1.0 - edge) + op[2] * edge,
                    ];
                    if i >= 11 {
                        let cup_x = [-10.0f32, -29.0, 35.0][i - 11];
                        let cup_z = [8.0f32, 13.0, 4.0][i - 11];
                        let sm_q = smooth(q) * u;
                        p[0] += cup_x * sm_q;
                        p[2] -= cup_z * sm_q;
                    }
                    let free = u.powf(1.85) * (0.74 + 0.26 * v.abs());
                    p[0] += 36.0 * angle_f * free;
                    p[1] += 16.0 * ph.cos() * angle_f * free;
                    p[2] += (30.0 * (ph + 0.5).sin() * angle_f + 22.0 * angle_f * v) * free;
                    // Dew drop weight sag on petal 9 (`day.py:179-185`)
                    if i == 9 {
                        let elapsed = t - 8.45;
                        let stress = if elapsed < 0.0 {
                            smooth((t - 1.3) / 4.8)
                        } else {
                            (-2.2 * elapsed).exp() * (6.7 * elapsed).cos()
                        };
                        p[2] -= 45.0 * stress * free;
                    }
                    deformed_pts[iu * NV + iv] = p;
                }
            }

            if i == 9 {
                petal9_rim = deformed_pts[(NU * 3 / 4) * NV + (NV / 2)];
            } else if i == 10 {
                let k = (NU * 2 / 3) * NV + (NV / 2);
                petal10_world = deformed_pts[k];
            }

            let cur_norms = compute_sheet_normals(&deformed_pts, NU, NV);
            for iu in 0..NU {
                let u = iu as f32 / (NU - 1) as f32;
                let q = qs[iu];
                for iv in 0..NV {
                    let v = -1.0 + 2.0 * (iv as f32 / (NV - 1) as f32);
                    let idx = iu * NV + iv;
                    let p = deformed_pts[idx];
                    let mut n = cur_norms[idx];
                    if dot3(n, eye) < 0.0 {
                        n = [-n[0], -n[1], -n[2]];
                    }
                    let rn = sh.rest_norm[idx];
                    let inc_s = dot3(n, sun).abs();
                    let gain = ((0.59 + 0.57 * inc_s) / (0.66 + 0.45 * dot3(rn, light_base).abs())).clamp(0.59, 1.46);
                    let tex = self.palette_tint(sample_petal_pigment(i, u, v));
                    let mut pig = [
                        tex[0] * gain * (0.84 + 0.16 * q + 0.10 * u),
                        tex[1] * gain * (0.84 + 0.16 * q + 0.10 * u),
                        tex[2] * gain * (0.84 + 0.16 * q + 0.10 * u),
                    ];

                    // Closed bud sky reflection (`day.py:232-237`)
                    let face = (0.66 + 0.34 * u) * (0.48 + 0.52 * inc_s);
                    let lum_tex = (tex[0] + tex[1] + tex[2]) / (3.0 * 220.0);
                    let sky_ref = (closed_frac * face * (0.49 + 0.52 * lum_tex)).min(0.76);
                    pig[0] += sky_ref * (249.0 - pig[0]);
                    pig[1] += sky_ref * (207.0 - pig[1]);
                    pig[2] += sky_ref * (247.0 - pig[2]);

                    // `PetalLight` (`petal_light.py:68-83`)
                    let inc_k = dot3(n, key);
                    let diffuse = inc_k.max(0.0);
                    let through = (-inc_k).max(0.0);
                    let sky_z = 0.5 + 0.5 * n[2];
                    let wax = dot3(n, half).max(0.0).powi(9);
                    let fresnel = (1.0 - dot3(n, eye).clamp(0.0, 1.0)).powi(2);
                    let fill = 0.56 + 0.16 * sky_z;
                    let energy = fill + 0.40 + 0.34 * diffuse + 0.18 * through + 0.12 * wax + 0.14 * fresnel;
                    let lit_col = [
                        pig[0] * energy + wax * 18.0,
                        pig[1] * energy + wax * 12.0,
                        pig[2] * energy + wax * 17.0,
                    ];

                    p_pts[idx] = (project(p), lit_col);
                }
            }

            // Rasterize petal triangles + mark petal pixels for canopy bounce/AO pass
            for iu in 0..(NU - 1) {
                for iv in 0..(NV - 1) {
                    let a = p_pts[iu * NV + iv];
                    let b = p_pts[(iu + 1) * NV + iv];
                    let c = p_pts[iu * NV + iv + 1];
                    let d = p_pts[(iu + 1) * NV + iv + 1];
                    raster_tri(a, b, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    raster_tri(b, d, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                }
            }
        }

        // Screen-space self-shadow AO between overlapping petals (`day.py:242-246`)
        self.shade_buf.resize(rlen, 0.0);
        self.shade_buf.fill(0.0);
        let d_off = if ss > 1 { 5usize } else { 3usize };
        for py in d_off..rh {
            for px in d_off..rw {
                let idx = py * rw + px;
                if self.rast_z[idx] <= -1e5 {
                    continue;
                }
                let near1 = self.rast_z[(py - d_off) * rw + (px - d_off)];
                let sh1 = ((near1 - self.rast_z[idx] - 17.0) / 56.0).clamp(0.0, 1.0);
                let mut sh2 = 0.0f32;
                let d2 = d_off * 2;
                if py >= d2 && px >= d2 {
                    let near2 = self.rast_z[(py - d2) * rw + (px - d2)];
                    sh2 = ((near2 - self.rast_z[idx] - 17.0) / 56.0).clamp(0.0, 1.0);
                }
                self.shade_buf[idx] = sh1 + sh2;
            }
        }
        for idx in 0..rlen {
            let s = self.shade_buf[idx];
            if s > 0.001 {
                let f = 1.0 - 0.075 * s.min(1.6);
                self.rast_rgb[idx][0] *= f;
                self.rast_rgb[idx][1] *= f;
                self.rast_rgb[idx][2] *= f;
            }
        }

        // 5. `PassingLight::canopy` Sweeping Diagonal Warm Sunbeam (`passing_light.py:11-52`)
        let ray_dir = {
            let k_proj = project(key);
            let o_proj = project([0.0, 0.0, 0.0]);
            let dy = (k_proj[1] - o_proj[1]).abs().max(0.2);
            (k_proj[0] - o_proj[0]) / dy
        };
        let slope = ray_dir.clamp(-0.65, -0.22); // warm upper-left to lower-right diagonal shaft
        let gap_frac = (t / PERIOD) * (self.canopy_dist.len() - 1) as f32;
        let g0 = (gap_frac.floor() as usize).min(self.canopy_dist.len() - 2);
        let gf = gap_frac - g0 as f32;
        let gap_ref = self.canopy_dist[g0] * (1.0 - gf) + self.canopy_dist[g0 + 1] * gf;
        // Map Python's 405x720 canopy aperture width smoothly onto dynamic `(rw, rh)`
        let shaft_scale = rw as f32 / 405.0;
        let gap_x = (gap_ref + 90.0) * shaft_scale + self.breeze_gust * 24.0;

        for py in 0..rh {
            let y_ref = (py as f32 / rh as f32) * 720.0;
            let air = 0.46 + 0.54 * (-(((y_ref - 280.0) / 235.0).powi(2))).exp();
            for px in 0..rw {
                let idx = py * rw + px;
                let occ = self.rast_z[idx] > -1e5;
                let depth_val = if occ { self.rast_z[idx].clamp(-400.0, 400.0) } else { 0.0 };
                let across = px as f32 - slope * (py as f32 - rh as f32 * 0.44);
                let offset = across - gap_x + 0.065 * depth_val * shaft_scale;
                let core = (-((offset + 5.0 * shaft_scale) / (35.0 * shaft_scale)).powi(2)).exp();
                let fringe = (-((offset - 29.0 * shaft_scale) / (49.0 * shaft_scale)).powi(2)).exp();
                let aperture = (0.76 * core + 0.33 * fringe).min(1.0);

                let c = self.rast_rgb[idx];
                let is_petal = occ && c[0] > 1.12 * c[1] && c[2] > c[1];
                let ambient = if !occ { 1.0 } else if is_petal { 1.025 } else { 0.86 };
                let mut lit = [c[0] * ambient, c[1] * ambient, c[2] * ambient];
                if aperture > 0.005 {
                    for k in 0..3 {
                        let headroom = (1.0 - lit[k] / 255.0).max(0.0);
                        let add = if occ {
                            lit[k] * [0.77, 0.69, 0.55][k]
                        } else {
                            air * [106.0, 90.0, 62.0][k]
                        };
                        lit[k] = (lit[k] + aperture * headroom * add).min(255.0);
                    }
                }
                if is_petal {
                    let bounce = 1.0 - aperture;
                    lit[0] = (lit[0] * (1.0 + bounce * 0.065)).min(255.0);
                    lit[1] = (lit[1] * (1.0 + bounce * 0.135)).min(255.0);
                    lit[2] = (lit[2] * (1.0 + bounce * 0.078)).min(255.0);
                }
                self.rast_rgb[idx] = lit;
            }
        }

        // 6. Refractive Dew Bead (`dew.py:68-133`) & Capillary Pond Ripples (`lotus_weather.py:74-105`)
        {
            let draw_bead = |cx: f32, cy: f32, rad: f32, aspect: f32, rast_rgb: &mut [[f32; 3]], rast_z: &[f32]| {
                let x0 = ((cx - rad - 1.0).floor() as i32).max(0);
                let x1 = ((cx + rad + 1.0).ceil() as i32).min(rw as i32 - 1);
                let y0 = ((cy - rad * aspect - 1.0).floor() as i32).max(0);
                let y1 = ((cy + rad * aspect + 1.0).ceil() as i32).min(rh as i32 - 1);
                for py in y0..=y1 {
                    for px in x0..=x1 {
                        let nx = (px as f32 - cx) / rad;
                        let ny = (py as f32 - cy) / (rad * aspect);
                        let r2 = nx * nx + ny * ny;
                        if r2 > 1.0 {
                            continue;
                        }
                        let nz = (1.0 - r2).sqrt();
                        let sx = ((px as f32 - nx * rad * 0.45).round() as i32).clamp(0, rw as i32 - 1);
                        let sy = ((py as f32 - ny * rad * 0.45).round() as i32).clamp(0, rh as i32 - 1);
                        let idx = py as usize * rw + px as usize;
                        let _ = rast_z[idx];
                        let refract = rast_rgb[sy as usize * rw + sx as usize];
                        let rx = 2.0 * nz * nx;
                        let ry = 2.0 * nz * ny;
                        let sky = (0.38 - 0.66 * ry + 0.18 * rx).clamp(0.0, 1.0);
                        let env = [24.0 + sky * 143.0, 31.0 + sky * 165.0, 27.0 + sky * 181.0];
                        let fresnel = 0.0204 + 0.9796 * (1.0 - nz).powi(5);
                        let refl_sky = (-((rx + 0.34) / 0.64).powi(2) - ((ry + 0.72) / 0.31).powi(2)).exp();
                        let sunlight = (-((nx + 0.25) / 0.28).powi(2) - ((ny + 0.46) / 0.19).powi(2)).exp();
                        for k in 0..3 {
                            let base_l = refract[k] * (0.85 + 0.15 * nz) * (1.0 - fresnel) + env[k] * fresnel;
                            let val = base_l
                                + refl_sky * [100.0, 114.0, 122.0][k]
                                + sunlight * 260.0 * [1.0, 0.96, 0.84][k];
                            rast_rgb[idx][k] = val.min(255.0);
                        }
                    }
                }
            };

            let draw_ripples = |cx: f32, cy: f32, age: f32, rast_rgb: &mut [[f32; 3]]| {
                if age < 0.0 || age >= 5.4 {
                    return;
                }
                let max_d = 85.0 * age.powf(0.73) * (rw as f32 / 405.0);
                let x0 = ((cx - max_d - 12.0).floor() as i32).max(0);
                let x1 = ((cx + max_d + 13.0).ceil() as i32).min(rw as i32 - 1);
                let y0 = ((cy - max_d * 0.24 - 10.0).floor() as i32).max(0);
                let y1 = ((cy + max_d * 0.24 + 10.0).ceil() as i32).min(rh as i32 - 1);
                let fade = (-age * 0.18).exp() * smooth((5.4 - age) / 1.0);
                for py in y0..=y1 {
                    for px in x0..=x1 {
                        let dx = px as f32 - cx;
                        let dy = (py as f32 - cy) / 0.23;
                        let rad = dx.hypot(dy);
                        let ang = dy.atan2(dx);
                        let glints = (0.22 + 0.78 * (0.5 + 0.5 * (ang * 2.0 + 0.55).cos()).powi(2))
                            * (0.60 + 0.40 * (ang * 5.0 - 0.7).cos().powi(2));
                        let mut waves = 0.0f32;
                        for &(delay, amp) in &[(0.0f32, 1.0f32), (0.30, 0.75), (0.65, 0.45), (1.05, 0.24)] {
                            let dt_w = age - delay;
                            if dt_w > 0.0 {
                                let front = 85.0 * dt_w.powf(0.73) * (rw as f32 / 405.0);
                                let width = 3.2 + 0.02 * front;
                                waves += amp * (-((rad - front) / width).powi(2)).exp() * (dt_w / 0.14).min(1.0);
                            }
                        }
                        let wv = waves * glints * fade;
                        if wv > 0.01 {
                            let idx = py as usize * rw + px as usize;
                            rast_rgb[idx][0] = (rast_rgb[idx][0] + wv * 115.0).min(255.0);
                            rast_rgb[idx][1] = (rast_rgb[idx][1] + wv * 165.0).min(255.0);
                            rast_rgb[idx][2] = (rast_rgb[idx][2] + wv * 172.0).min(255.0);
                        }
                    }
                }
            };

            let rim_p = project(petal9_rim);
            let leaf_p = project(leaf_r_surface_pt);
            if t >= 1.2 && t < 8.45 {
                let rad_b = (4.2 + 3.0 * smooth((t - 1.2) / 5.5)) * (scale * 2.0);
                draw_bead(rim_p[0], rim_p[1], rad_b, 0.90, &mut self.rast_rgb, &self.rast_z);
            } else if t >= 8.45 && t < 9.25 {
                let f = (t - 8.45) / 0.80;
                let bx = rim_p[0] + (leaf_p[0] - rim_p[0]) * f;
                let by = rim_p[1] + (leaf_p[1] - rim_p[1]) * f * f;
                draw_bead(bx, by, 7.1 * scale * 2.0, 1.15, &mut self.rast_rgb, &self.rast_z);
            } else if t >= 9.25 && t < 11.2 {
                let f = (t - 9.25) / 1.95;
                draw_bead(leaf_p[0] + f * 18.0 * scale, leaf_p[1] + f * 8.0 * scale, 6.8 * scale * 2.0, 0.82, &mut self.rast_rgb, &self.rast_z);
            } else if t >= 11.2 && t < 12.15 {
                let f = (t - 11.2) / 0.95;
                let bx = leaf_p[0] + 18.0 * scale;
                let by = leaf_p[1] + 8.0 * scale + f * f * (rh as f32 * 0.22);
                draw_bead(bx, by, 7.0 * scale * 2.0, 1.20, &mut self.rast_rgb, &self.rast_z);
            } else if t >= 12.15 && t < 17.55 {
                draw_ripples(leaf_p[0] + 18.0 * scale, (leaf_p[1] + rh as f32 * 0.22).min(rh as f32 * 0.89), t - 12.15, &mut self.rast_rgb);
            }

            for ud in &self.user_drops {
                draw_ripples(ud.uv[0] * rw as f32, ud.uv[1] * rh as f32, ud.age, &mut self.rast_rgb);
            }
        }

        // 7. 3D Bumblebee Visitor (`visitor.py` + `foraging.py`)
        if t >= 2.3 && t <= 18.2 {
            let (bx_w, by_w, bz_w, yaw, pitch, wings_open) = if t < 6.8 {
                // Incoming curved approach from left side (`cicek_000007.jpg`, `011.jpg`)
                let f = smooth((t - 2.3) / 4.5);
                let px = -560.0 * (1.0 - f) + (petal10_world[0] - 28.0) * f + 24.0 * (TAU * f).sin();
                let py = -85.0 * (1.0 - f) + petal10_world[1] * f - 40.0 * (PI * f).sin();
                let pz = 140.0 * (1.0 - f) + (petal10_world[2] + 22.0) * f + 24.0 * (t * 4.5).sin();
                (px, py, pz, 0.16, -0.15 * (1.0 - f), 1.0 - 0.85 * smooth((f - 0.8) / 0.2))
            } else if t <= 13.8 {
                // Foraging gently along inner petal & stamen crown
                let w = t - 6.8;
                let probe = (w * 1.4).sin();
                (
                    petal10_world[0] - 18.0 + 18.0 * probe,
                    petal10_world[1] + 8.0 * (w * 2.1).cos(),
                    petal10_world[2] + 16.0,
                    0.22 + 0.35 * probe,
                    -0.18,
                    0.12,
                )
            } else {
                // Exiting smoothly toward left bank (`cicek_000036.jpg`)
                let f = smooth((t - 13.8) / 4.4);
                let px = (petal10_world[0] - 18.0) * (1.0 - f) - 580.0 * f;
                let py = petal10_world[1] * (1.0 - f) - 60.0 * f + 22.0 * (PI * f).sin();
                let pz = (petal10_world[2] + 20.0) * (1.0 - f) + 160.0 * f;
                (px, py, pz, PI - 0.24, 0.12, 1.0)
            };

            let mut world_bee = [bx_w, by_w, bz_w];
            if self.bee_lure_blend > 0.02 {
                let lx = (self.bee_lure[0] - 0.5) * 440.0;
                let ly = (self.bee_lure[1] - 0.45) * 320.0;
                world_bee[0] = world_bee[0] * (1.0 - self.bee_lure_blend) + lx * self.bee_lure_blend;
                world_bee[1] = world_bee[1] * (1.0 - self.bee_lure_blend) + ly * self.bee_lure_blend;
            }
            let fwd = [yaw.cos() * pitch.cos(), yaw.sin() * pitch.cos(), pitch.sin()];
            let lat = [-yaw.sin(), yaw.cos(), 0.0];
            let up = cross3(fwd, lat);
            let bee_pt = |loc: [f32; 3]| -> [f32; 3] {
                let s = 1.23;
                project([
                    world_bee[0] + s * (loc[0] * fwd[0] + loc[1] * lat[0] + loc[2] * up[0]),
                    world_bee[1] + s * (loc[0] * fwd[1] + loc[1] * lat[1] + loc[2] * up[1]),
                    world_bee[2] + s * (loc[0] * fwd[2] + loc[1] * lat[2] + loc[2] * up[2]),
                ])
            };

            // Bee legs (`visitor.py:254-260`)
            for sign in [-1.0f32, 1.0f32] {
                for &(hip, toe) in &[(9.0f32, 24.0f32), (0.0, -1.0), (-10.0, -31.0)] {
                    let p0 = bee_pt([hip, sign * 7.0, -4.0]);
                    let p1 = bee_pt([hip - 4.0, sign * 11.0, -7.0]);
                    let p2 = bee_pt([toe - 4.0, sign * 15.0, -16.0]);
                    raster_line_disk(p0, p1, 1.1 * scale * 2.0, [58.0, 46.0, 43.0], rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    raster_line_disk(p1, p2, 1.0 * scale * 2.0, [58.0, 46.0, 43.0], rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                }
            }

            // Striped copper/black/buff abdomen, gold-collared thorax, head (`visitor.py:264-267`)
            for &(origin, radii, kind) in &[
                ([-23.0f32, 0.0, 0.0], [25.4f32, 15.5, 14.2], 0u8),
                ([1.0, 0.0, 2.0], [17.5, 16.7, 17.3], 1u8),
                ([20.0, 0.0, -2.6], [11.0, 12.2, 11.8], 2u8),
            ] {
                let n_p = 10usize;
                let n_a = 14usize;
                let mut part_v = vec![([0.0f32; 3], [0.0f32; 3]); n_p * (n_a + 1)];
                for ip in 0..n_p {
                    let pol = PI * (ip as f32 / (n_p - 1) as f32);
                    for ia in 0..=n_a {
                        let azb = TAU * (ia as f32 / n_a as f32);
                        let nx = pol.cos();
                        let ny = pol.sin() * azb.cos();
                        let nz = pol.sin() * azb.sin();
                        let world_n = unit3([
                            nx * fwd[0] + ny * lat[0] + nz * up[0],
                            nx * fwd[1] + ny * lat[1] + nz * up[1],
                            nx * fwd[2] + ny * lat[2] + nz * up[2],
                        ]);
                        let l_dot = dot3(world_n, [-0.43, 0.42, 0.80]).max(0.0);
                        let shade = 0.62 + 0.48 * l_dot + 0.22 * world_n[2].max(0.0);
                        let base_c = match kind {
                            0 => {
                                let band = ((nx - 0.10) * 5.5).clamp(0.0, 1.0);
                                let tail = ((-0.42 - nx) * 5.3).clamp(0.0, 1.0);
                                let mut c = [
                                    31.0 + (218.0 - 31.0) * band,
                                    26.0 + (145.0 - 26.0) * band,
                                    32.0 + (44.0 - 32.0) * band,
                                ];
                                c[0] += (220.0 - c[0]) * tail;
                                c[1] += (206.0 - c[1]) * tail;
                                c[2] += (176.0 - c[2]) * tail;
                                c
                            }
                            1 => {
                                let collar = ((nx + 0.12) * 3.2).clamp(0.0, 1.0);
                                [
                                    49.0 + (233.0 - 49.0) * collar,
                                    36.0 + (172.0 - 36.0) * collar,
                                    29.0 + (55.0 - 29.0) * collar,
                                ]
                            }
                            _ => [42.0, 40.0, 44.0],
                        };
                        let col = [
                            (base_c[0] * shade + l_dot * 13.0).min(255.0),
                            (base_c[1] * shade + l_dot * 10.0).min(255.0),
                            (base_c[2] * shade + l_dot * 5.0).min(255.0),
                        ];
                        let wp = bee_pt([
                            origin[0] + nx * radii[0],
                            origin[1] + ny * radii[1],
                            origin[2] + nz * radii[2],
                        ]);
                        part_v[ip * (n_a + 1) + ia] = (wp, col);
                    }
                }
                for ip in 0..(n_p - 1) {
                    for ia in 0..n_a {
                        let a = part_v[ip * (n_a + 1) + ia];
                        let b = part_v[(ip + 1) * (n_a + 1) + ia];
                        let c = part_v[ip * (n_a + 1) + ia + 1];
                        let d = part_v[(ip + 1) * (n_a + 1) + ia + 1];
                        raster_tri(a, b, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                        raster_tri(b, d, c, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                    }
                }
            }

            // Translucent fan wings (`visitor.py:298-342`)
            for sign in [-1.0f32, 1.0f32] {
                for &delta in &[-23.0f32, 22.0, 0.0] {
                    if delta != 0.0 && wings_open < 0.08 {
                        continue;
                    }
                    let a_ang = (11.0 + wings_open * (65.0 + delta + 4.0 * (3.4 * t).sin())).to_radians();
                    let w0 = bee_pt([5.0, sign * 9.5, 10.0]);
                    let w1 = bee_pt([
                        5.0 - 42.0 * a_ang.cos(),
                        sign * (9.5 + 42.0 * a_ang.sin()),
                        10.0 + 42.0 * (0.18 + 0.32 * wings_open),
                    ]);
                    let alpha = if delta == 0.0 { 0.55 } else { 0.25 };
                    let w_col = [183.0 * alpha + 60.0, 195.0 * alpha + 60.0, 192.0 * alpha + 60.0];
                    raster_line_disk(w0, w1, (if delta == 0.0 { 3.0 } else { 2.4 }) * scale * 2.0, w_col, rw, rh, &mut self.rast_z, &mut self.rast_rgb);
                }
            }
        }

        // 8. `AsciiCamera.grid` Conversion: Box Downsample + Sobel Contour Edges (`| _ / \`)
        self.light_buf.resize(cols * rows, 0.0);
        for r in 0..rows {
            for c in 0..cols {
                let mut rgb_sum = [0.0f32; 3];
                for dy in 0..ss {
                    for dx in 0..ss {
                        let s = self.rast_rgb[(r * ss + dy) * rw + (c * ss + dx)];
                        rgb_sum[0] += s[0];
                        rgb_sum[1] += s[1];
                        rgb_sum[2] += s[2];
                    }
                }
                let inv_area = 1.0 / (ss * ss) as f32;
                let m = [rgb_sum[0] * inv_area, rgb_sum[1] * inv_area, rgb_sum[2] * inv_area];
                let lum = (0.2126 * m[0] + 0.7152 * m[1] + 0.0722 * m[2]) / 255.0;
                self.light_buf[r * cols + c] = lum;

                let ink = [
                    (m[0] * 1.24 + 16.0).clamp(0.0, 255.0) as u8,
                    (m[1] * 1.24 + 16.0).clamp(0.0, 255.0) as u8,
                    (m[2] * 1.24 + 16.0).clamp(0.0, 255.0) as u8,
                ];
                let back = [
                    (PAPER[0] + 0.24 * (m[0] - 32.0).max(0.0)).clamp(0.0, 255.0) as u8,
                    (PAPER[1] + 0.24 * (m[1] - 32.0).max(0.0)).clamp(0.0, 255.0) as u8,
                    (PAPER[2] + 0.24 * (m[2] - 32.0).max(0.0)).clamp(0.0, 255.0) as u8,
                ];
                self.canvas.set_bg_rgb(c as i32, r as i32, back);
                let lvl = (lum.sqrt() * RAMP.len() as f32 - 2.4).clamp(0.0, (RAMP.len() - 1) as f32);
                let gl = RAMP[lvl as usize].saturating_sub(32);
                let bloom = if lum > 0.30 { 150 } else { 25 };
                self.canvas.set_cell(c as i32, r as i32, gl, ink, bloom, 1);
            }
        }

        // Exact perpendicular Sobel contour edges from `ascii_preview.py:48-54`
        for r in 1..(rows - 1) {
            for c in 1..(cols - 1) {
                let lum = self.light_buf[r * cols + c];
                if lum <= 0.15 {
                    continue;
                }
                let dx = 0.5 * (self.light_buf[r * cols + c + 1] - self.light_buf[r * cols + c - 1]);
                let dy = 0.5 * (self.light_buf[(r + 1) * cols + c] - self.light_buf[(r - 1) * cols + c]);
                if dx.hypot(dy) > 0.105 {
                    let eg = if dx.abs() > 2.0 * dy.abs() {
                        glyph_id('|')
                    } else if dy.abs() > 2.0 * dx.abs() {
                        glyph_id('_')
                    } else if dx * dy > 0.0 {
                        glyph_id('/')
                    } else {
                        glyph_id('\\')
                    };
                    let p = (r * cols + c) * 4;
                    self.canvas.cells[p + 3] = eg;
                }
            }
        }

        self.canvas.mark_dirty();
    }
}
