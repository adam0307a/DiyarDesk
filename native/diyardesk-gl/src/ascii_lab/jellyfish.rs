//! 02 · Jellyfish Heart — 1:1 faithful interactive Rust port of
//! `sapozhkov/ascii-lab-public/02-jellyfish-heart/source/{duet,reef,renderer,swimmer,heart_pose,heart_height}.py`.

use super::grid_gl::{glyph_id, line_glyph_underscore, ramp_glyph, AsciiCanvas};
use super::{clamp_u8, scale_rgb, AsciiLabParams};
use crate::state::Rect;
use std::f32::consts::{PI, TAU};

const SECONDS: f32 = 20.0;
const DT: f32 = 1.0 / 30.0;
const WARM: f32 = 20.0;
const PRE_ORB: f32 = 9.0;
const PRE_STEPS: usize = 270;
const PULSE_RATE: f32 = 0.60;
const CLOCK: f32 = 30.25;
const BEAT: f32 = 2.0;
const FIRST_BEAT: f32 = 0.25;
const WHEN_WHITE: f32 = FIRST_BEAT + BEAT;
const DOME_RADIUS: f32 = 0.465;
const DOME_HEIGHT: f32 = 0.58;
const ROLL: f32 = 1.62;
const LIP: f32 = 0.04;
const TURN: f32 = 0.38;
const PITCH: f32 = 0.28;
const CAM_D: f32 = 7.0;
const EDGE_DELAY: f32 = 0.20;
const SCALLOPS: f32 = 16.0;
const LIMBS: usize = 4;
const LIMB_REACH: f32 = 1.02;
const LIMB_HALF: f32 = 0.145;
const LIMB_DELAY: f32 = 0.45;
const STRANDS: usize = 14;
const STRAND_NODES: usize = 34;
const STRAND_SUB: usize = 4;
const FRONT_SPEED: f32 = 2.4;
const JOIN_LIMB: f32 = 0.45;
const BELL_HEIGHT_SCALE: f32 = 0.70;

const RAMP: &[u8] = b"$QB%8&0#@oauenjzt[]{}<>rcvx+=1il!;:," ;
const INNER_G: &[u8] = b"|1:;|!l:";
const BOULDER_RAMP: &[u8] = b"..',:;=+";

const OWN_EMPTY: u8 = 0;
const OWN_SHELL: u8 = 1;
const OWN_INSIDE: u8 = 2;
const OWN_HEM: u8 = 3;
const OWN_LIMB: u8 = 4;
const OWN_STRAND: u8 = 5;
const OWN_REEF: u8 = 6;

#[inline]
fn ease(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    (3.0 - 2.0 * t) * t * t
}

#[inline]
fn tint(col: [f32; 3], amount: f32) -> [f32; 3] {
    [
        col[0] * (1.0 - amount) + amount,
        col[1] * (1.0 - amount) + amount,
        col[2] * (1.0 - amount) + amount,
    ]
}

#[inline]
fn hash01(a: f32, b: f32) -> f32 {
    ((a * 127.1 + b * 311.7).sin() * 43758.5453).abs().fract()
}

#[inline]
fn spin_x(p: [f32; 3], a: f32) -> [f32; 3] {
    let (c, s) = (a.cos(), a.sin());
    [p[0], c * p[1] - s * p[2], s * p[1] + c * p[2]]
}

#[inline]
fn spin_z(p: [f32; 3], a: f32) -> [f32; 3] {
    let (c, s) = (a.cos(), a.sin());
    [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
}

#[inline]
fn yaw_y(p: [f32; 3], a: f32) -> [f32; 3] {
    let (c, s) = (a.cos(), a.sin());
    [c * p[0] - s * p[2], p[1], s * p[0] + c * p[2]]
}

#[inline]
fn around(r: f32, angle: f32, h: f32) -> [f32; 3] {
    [r * angle.cos(), h, r * angle.sin()]
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
fn pulse(t: f32) -> f32 {
    if t < WHEN_WHITE {
        return 0.0;
    }
    let phase = (t - FIRST_BEAT).rem_euclid(BEAT);
    let shut = 0.65 * ease((phase - 0.2) / 0.3) * (1.0 - ease((phase - 0.5) / 0.6));
    let wide = 0.9 * (-((((phase + 1.0).rem_euclid(BEAT) - 1.03) / 0.2).powi(2))).exp();
    shut - wide
}

#[inline]
fn spread(t: f32) -> f32 {
    ease((t - 0.35) / 4.2) * (1.0 - ease((t - 6.0) / 10.0))
}

#[inline]
fn duet_clock(t: f32, index: usize) -> f32 {
    CLOCK + PULSE_RATE * t + index as f32 * 0.18 * spread(t)
}

#[inline]
fn orient(body: [f32; 3], tilt: f32, angle: f32) -> [f32; 3] {
    let fwd = spin_x(spin_z(body, -tilt), 0.15);
    spin_z(yaw_y(fwd, angle), 0.045)
}

/// Parametric heart height from `duet.py` / `heart_height.py`.
#[inline]
pub fn heart_height_fn(angle: f32) -> f32 {
    (13.0 * angle.cos()
        - 5.0 * (2.0 * angle).cos()
        - 2.0 * (3.0 * angle).cos()
        - (4.0 * angle).cos())
        / 16.0
}

#[inline]
pub fn heart_edges(u: f32) -> (f32, f32) {
    let a = u.clamp(0.0, 1.0).cbrt().asin();
    (heart_height_fn(PI - a), heart_height_fn(a))
}

#[inline]
fn dome_profile(v: f32) -> (f32, f32) {
    let arc = v * ROLL;
    let floor = ROLL.cos();
    (
        DOME_RADIUS * arc.sin().max(0.0).sqrt(),
        DOME_HEIGHT * (arc.cos() - floor) / (1.0 - floor),
    )
}

#[inline]
fn dome_pt(v: f32, phi: f32, t: f32) -> [f32; 3] {
    let late = t - EDGE_DELAY * v;
    let grip = pulse(late);
    let (mut girth, rise) = dome_profile(v);
    let hem = v.powi(6);
    girth *= (1.0 + 0.06 * hem * (SCALLOPS * phi).cos()) * (1.0 + LIP * hem - 0.34 * grip * v.powf(1.6));
    around(girth, phi + TURN * t, rise * (1.0 + 0.15 * grip))
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
struct OrbitSample {
    radius: f32,
    angle: f32,
    height: f32,
    tilt: f32,
    camera_angle: f32,
    nodes: [[[f32; 3]; STRAND_NODES]; 2 * STRANDS],
}

#[derive(Clone)]
struct ReefPt {
    world: [f32; 3],
    glyph: u8,
    rgb: [f32; 3],
}

#[derive(Clone, Copy)]
struct PenCell {
    station: f32,
    span: f32,
    normal: f32,
    lift: f32,
    glyph: u8,
    ink: [f32; 3],
    polyp: bool,
}

#[derive(Clone)]
struct SwimmerPt {
    pos: [f32; 3],
    glyph: u8,
    col: [f32; 3],
    kind: u8,
    meta: u16, // encodes strand_group / limb_group for overlap & tangent strokes
}

pub struct AsciiJellyfishState {
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
    pointer_pulse: f32,
    heart_wave_timer: f32,
    // Pre-integrated Orbit & Sea Pen water wake LUT across [0..20s]
    orbit: Vec<OrbitSample>,
    start_angle: f32,
    #[allow(dead_code)]
    strand_roots: [f32; STRANDS],
    heart_bottom: [f32; 129],
    heart_x: [f32; 129],
    dots: Vec<[f32; 3]>,
    reef_pts: Vec<ReefPt>,
    pen_cells: Vec<PenCell>,
    pen_bend: Vec<[f32; 2]>,
    pen_stim: Vec<f32>,
    anemone_arms: Vec<(f32, f32, f32, f32)>,
    anemone_disc: Vec<ReefPt>,
    clear_mask: Vec<bool>,
}

impl Default for AsciiJellyfishState {
    fn default() -> Self {
        Self::new(1280.0, 720.0, 1.0)
    }
}

impl AsciiJellyfishState {
    pub fn new(css_w: f32, css_h: f32, dpi_scale: f32) -> Self {
        let mut rng_s = Rng64::new(5);
        let strand_span: Vec<f32> = (0..STRANDS).map(|_| 1.0 + 0.5 * rng_s.f32()).collect();
        let mean_span = strand_span.iter().sum::<f32>() / STRANDS as f32;
        for _ in 0..STRANDS {
            let _ = rng_s.f32();
        }
        let mut strand_roots = [0.0f32; STRANDS];
        for i in 0..STRANDS {
            strand_roots[i] = (TAU / STRANDS as f32) * (i as f32 + 0.5 * rng_s.f32());
        }
        let strand_len: Vec<f32> = strand_span
            .iter()
            .map(|&s| (s / mean_span) * (3.6 * DOME_RADIUS))
            .collect();

        // Pre-integrate joint orbit + strands matching `duet.py:42-102`
        let warm_steps = (WARM / DT).round() as usize;
        let run_steps = (SECONDS / DT).round() as usize;
        let total = warm_steps + run_steps + 2;
        let mut hist_r = Vec::with_capacity(total);
        let mut hist_a = Vec::with_capacity(total);
        let mut hist_h = Vec::with_capacity(total);
        let mut hist_tilt = Vec::with_capacity(total);
        let mut hist_nodes = Vec::with_capacity(total);

        let mut radius = 0.43f32;
        let mut radial = 0.0f32;
        let mut omega = 1.7f32;
        let mut height = 0.0f32;
        let mut climb = 0.55f32;
        let mut angle = 0.0f32;
        let mut wake = 0.8f32;
        let mut chains: Option<[[[f32; 3]; STRAND_NODES]; 2 * STRANDS]> = None;

        let edge_steps = (EDGE_DELAY / (PULSE_RATE * DT)).round() as usize;
        for k in 0..total {
            let t = -WARM + k as f32 * DT;
            let open_ = spread(t);
            let ph0 = duet_clock(t, 0);
            let ph1 = duet_clock(t, 1);
            let p0 = ((pulse(ph0) - pulse(ph0 - PULSE_RATE * DT)) / DT).max(0.0);
            let p1 = ((pulse(ph1) - pulse(ph1 - PULSE_RATE * DT)) / DT).max(0.0);
            let pump = 0.5 * (p0 + p1);

            let target = 0.43 + 0.53 * open_;
            radial += ((target - radius) * 9.0 - radial * 5.0) * DT;
            radius += radial * DT;
            omega += ((0.54 - 0.16 * open_) * pump - 0.62 * omega) * DT;
            climb += (0.20 * pump - 0.65 * climb - 0.6 * climb * climb.abs()) * DT;
            height += climb * DT;
            angle += omega * DT;
            let tilt = 0.44 - 0.21 * open_;
            wake += (pump - wake) * DT / 0.75;

            hist_r.push(radius);
            hist_a.push(angle);
            hist_h.push(height);
            hist_tilt.push(tilt);

            let rk = k.saturating_sub(edge_steps);
            let rim_r = hist_r[rk];
            let rim_a = hist_a[rk];
            let rim_h = hist_h[rk];
            let rim_tilt = hist_tilt[rk];

            let mut roots = [[0.0f32; 3]; 2 * STRANDS];
            for idx in 0..2 {
                let ph = if idx == 0 { ph0 } else { ph1 };
                let bearing = rim_a + idx as f32 * PI;
                let center = [rim_r * bearing.cos(), rim_h, rim_r * bearing.sin()];
                for s in 0..STRANDS {
                    let loc = dome_pt(1.0, strand_roots[s], ph);
                    let w = orient(loc, rim_tilt, bearing);
                    roots[idx * STRANDS + s] = [w[0] + center[0], w[1] + center[1], w[2] + center[2]];
                }
            }

            let ch = chains.get_or_insert_with(|| {
                let mut init = [[[0.0f32; 3]; STRAND_NODES]; 2 * STRANDS];
                for s in 0..(2 * STRANDS) {
                    let link = strand_len[s % STRANDS] / (STRAND_NODES - 1) as f32;
                    for j in 0..STRAND_NODES {
                        init[s][j] = [roots[s][0], roots[s][1] - link * j as f32, roots[s][2]];
                    }
                }
                init
            });

            let radial_exp = (-(0.15 + 1.7 * wake) * (1.0 - open_).powi(2) * DT).exp();
            for s in 0..(2 * STRANDS) {
                let link = strand_len[s % STRANDS] / (STRAND_NODES - 1) as f32;
                for j in 0..STRAND_NODES {
                    ch[s][j][0] = ch[s][j][0] * radial_exp + 0.023 * DT;
                    ch[s][j][2] *= radial_exp;
                    ch[s][j][1] -= (0.09 + 0.34 * wake) * DT;
                }
                ch[s][0] = roots[s];
                for j in 1..STRAND_NODES {
                    let prev = ch[s][j - 1];
                    let cur = ch[s][j];
                    let dir = unit3([cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]]);
                    ch[s][j] = [
                        prev[0] + dir[0] * link,
                        prev[1] + dir[1] * link,
                        prev[2] + dir[2] * link,
                    ];
                }
            }
            hist_nodes.push(*ch);
        }

        let start_angle = hist_a[warm_steps];
        let mut cam_angles = vec![start_angle; total];
        for k in 1..total {
            let t = -WARM + k as f32 * DT;
            let eng = ease((t - 12.0) / 5.0);
            cam_angles[k] = cam_angles[k - 1] + (hist_a[k] - hist_a[k - 1]) * eng;
        }
        let err = hist_a[total - 1] - cam_angles[total - 1];
        let err_wrapped = err - TAU * (err / TAU).round();
        for k in 0..total {
            let t = -WARM + k as f32 * DT;
            let eng = ease((t - 12.0) / 5.0);
            cam_angles[k] += eng * err_wrapped;
        }

        let mut orbit = Vec::with_capacity(PRE_STEPS + run_steps + 2);
        for k in (warm_steps - PRE_STEPS)..total {
            orbit.push(OrbitSample {
                radius: hist_r[k],
                angle: hist_a[k],
                height: hist_h[k],
                tilt: hist_tilt[k],
                camera_angle: cam_angles[k],
                nodes: hist_nodes[k],
            });
        }

        let mut heart_x = [0.0f32; 129];
        let mut heart_bottom = [0.0f32; 129];
        for i in 0..129 {
            let u = i as f32 / 128.0;
            heart_x[i] = u;
            heart_bottom[i] = heart_edges(u).0;
        }

        let mut rng_d = Rng64::new(21);
        let mut dots = Vec::with_capacity(140);
        for _ in 0..140 {
            dots.push([rng_d.f32(), rng_d.f32(), rng_d.f32()]);
        }

        // Deep-sea reef seabed + boulders matching `reef.py:179-327`
        let mut rng_r = Rng64::new(184);
        let mut reef_pts = Vec::with_capacity(3800);
        let bed_y = |x: f32, z: f32| -> f32 {
            -2.02 + 0.12 * (2.25 * x + 0.69 * z).sin() + 0.035 * z
        };
        let surf_light = unit3([-0.52, 0.92, -0.78]);
        for iz in 0..22 {
            let z = -0.6 + (iz as f32 / 21.0) * 5.6;
            let mut x = -4.2f32;
            while x <= 4.2 {
                let rz = z + 0.10 * (2.7 * x + 1.4 * z).sin() + 0.045 * (5.1 * x - z).sin();
                let patch = 0.54 + 0.30 * (x * 3.6 + z).sin() + 0.16 * (z * 5.5 - x * 0.7).sin();
                if rng_r.f32() <= 0.62 * patch {
                    let g_idx = rng_r.f32();
                    let ch = if g_idx < 0.34 {
                        ','
                    } else if g_idx < 0.61 {
                        '.'
                    } else if g_idx < 0.77 {
                        '\''
                    } else if g_idx < 0.88 {
                        '~'
                    } else {
                        '_'
                    };
                    let f = (0.63 + 0.47 * patch) * rng_r.range(0.61, 1.0);
                    reef_pts.push(ReefPt {
                        world: [x, bed_y(x, rz) + rng_r.range(-0.01, 0.01), rz],
                        glyph: glyph_id(ch),
                        rgb: [35.0 * f / 255.0, 72.0 * f / 255.0, 79.0 * f / 255.0],
                    });
                }
                x += 0.045;
            }
        }
        const BOULDERS: [(f32, f32, [f32; 3], u64); 6] = [
            (-1.27, 0.63, [0.80, 0.63, 0.56], 4),
            (-0.78, 0.86, [0.36, 0.41, 0.29], 7),
            (1.15, 0.50, [0.68, 0.91, 0.62], 3),
            (1.61, 0.73, [0.48, 0.50, 0.41], 11),
            (0.62, 1.22, [0.33, 0.28, 0.24], 8),
            (-1.95, 1.56, [0.66, 0.40, 0.37], 13),
        ];
        for (bx, bz, radii, seed) in BOULDERS {
            let mut brng = Rng64::new(seed);
            for ip in 0..20 {
                let p = 0.005 + (ip as f32 / 19.0) * 1.68;
                for ia in 0..44 {
                    let a = (ia as f32 / 44.0) * TAU;
                    let n = [p.sin() * a.cos(), p.cos(), p.sin() * a.sin()];
                    let facets = 1.0
                        + 0.095 * (3.0 * a + seed as f32).sin() * (2.0 * p).sin()
                        + 0.047 * (7.0 * a + 2.0 * p + seed as f32).sin();
                    let mut st = [
                        n[0] * radii[0] * facets,
                        n[1] * radii[1] * facets,
                        n[2] * radii[2] * facets,
                    ];
                    st[1] += 0.10 * st[0] + bed_y(bx, bz) + 0.13;
                    st[0] += bx;
                    st[2] += bz;
                    let nu = unit3([n[0] / radii[0], n[1] / radii[1], n[2] / radii[2]]);
                    let lit = dot3(nu, surf_light).max(0.0);
                    let grain = brng.range(0.81, 1.13);
                    let mut level = ((0.16 + 0.84 * lit) * grain).clamp(0.0, 1.0);
                    let seam = (17.0 * st[1] + 2.7 * st[0] + 0.7 * (3.0 * st[2] + seed as f32).sin()).sin();
                    level *= 0.72 + 0.28 * ease((seam + 0.68) / 0.65);
                    reef_pts.push(ReefPt {
                        world: st,
                        glyph: ramp_glyph(BOULDER_RAMP, level),
                        rgb: [
                            ((32.0 + level * 64.0) * grain) / 255.0,
                            ((38.0 + level * 70.0) * grain) / 255.0,
                            ((56.0 + level * 81.0) * grain) / 255.0,
                        ],
                    });
                }
            }
        }

        // Exact Sea Pen colony + water wake integration (`reef.py:26-176`)
        let pen_site = (-0.82f32, 0.52f32, 1.25f32, -0.71f32, 0.31f32, 51u64);
        let pen_root_y = -1.49f32;
        let mut pen_cells = Vec::with_capacity(620);
        let mut prng = Rng64::new(pen_site.5);
        for s_i in 0..84 {
            let st_u = s_i as f32 / 83.0;
            let rad = 0.004 + 0.015 * (PI * st_u).sin().powf(0.6);
            for flank in [-1i32, 0, 1] {
                let mark = if flank != 0 { '|' } else { 'l' };
                let shade = if flank != 0 { 0.75 } else { 1.1 } * (0.76 + 0.24 * st_u);
                pen_cells.push(PenCell {
                    station: st_u,
                    span: rad * flank as f32,
                    normal: -0.4 * rad * (1 - flank * flank) as f32,
                    lift: 0.0,
                    glyph: glyph_id(mark),
                    ink: [97.0 * shade / 255.0, 153.0 * shade / 255.0, 132.0 * shade / 255.0],
                    polyp: false,
                });
            }
        }
        let pairs = 5usize;
        for sign in [-1.0f32, 1.0f32] {
            for row in 0..pairs {
                let age = (row as f32 + 0.5) / pairs as f32;
                let st_u = 0.17 + 0.72 * age + sign * 0.095 / pairs as f32;
                let len = (0.043 + 0.218 * (1.0 - age).powf(0.55)) * prng.range(0.88, 1.10);
                let skew = prng.range(-0.21, 0.21);
                let blade = (PI * age).sin().powf(0.60);
                for iu in 0..18 {
                    let u = iu as f32 / 17.0;
                    let vein = (PI * u).sin().powf(0.9);
                    let width = (0.008 + 0.019 * blade) * vein;
                    let rise = (0.11 + 0.092 * (1.0 - age)) * (u * 0.69 + u * u * 0.31);
                    let curl = (0.019 * u * u + skew * len * u) * sign;
                    let span = sign * len * u;
                    for rim in [-1i32, 0, 1] {
                        let ch = if rim >= 0 {
                            if sign > 0.0 { '/' } else { '\\' }
                        } else {
                            ','
                        };
                        let sh = 0.47 + 0.17 * u + if rim == 1 { 0.36 } else { 0.0 };
                        pen_cells.push(PenCell {
                            station: st_u,
                            span,
                            normal: curl + 0.38 * width * (1 - rim * rim) as f32,
                            lift: rise + rim as f32 * width,
                            glyph: glyph_id(ch),
                            ink: [68.0 * sh / 255.0, 133.0 * sh / 255.0, 115.0 * sh / 255.0],
                            polyp: false,
                        });
                    }
                }
                for u in [0.74f32, 0.95] {
                    let rise = (0.11 + 0.092 * (1.0 - age)) * (u * 0.69 + u * u * 0.31);
                    let curl = (0.019 * u * u + skew * len * u) * sign;
                    let sh = 0.74 + 0.11 * u;
                    pen_cells.push(PenCell {
                        station: st_u,
                        span: sign * len * u,
                        normal: curl,
                        lift: rise + 0.014,
                        glyph: glyph_id('*'),
                        ink: [139.0 * sh / 255.0, 164.0 * sh / 255.0, 120.0 * sh / 255.0],
                        polyp: true,
                    });
                }
            }
        }

        // Pre-integrate Sea Pen water bend & wake pulse (`reef.py:106-145`)
        let pen_n = orbit.len();
        let mut pen_bend = vec![[0.0f32; 2]; pen_n];
        let mut pen_stim = vec![0.0f32; pen_n];
        let mut pb = [0.0f32; 2];
        let mut pv = [0.0f32; 2];
        let mut p_act = 0.0f32;
        let p_phase = 0.52 * pen_site.0 + 0.83 * pen_site.1;
        for k in 0..pen_n {
            let t = (k as f32 - PRE_STEPS as f32) * DT;
            let o = &orbit[k];
            let gap = (o.height - orbit[PRE_STEPS].height - (pen_root_y + 0.62 * pen_site.2)).max(0.12);
            let late = t - gap / 0.94;
            let l_idx = (((late + PRE_ORB) / DT) as isize).clamp(0, (pen_n - 1) as isize) as usize;
            let lo = &orbit[l_idx];
            let ang = lo.angle - start_angle;
            let mut disp = [0.0f32; 2];
            let mut stress = 0.0f32;
            for idx in 0..2 {
                let px = lo.radius * (ang + idx as f32 * PI).cos();
                let pz = lo.radius * (ang + idx as f32 * PI).sin();
                let ox = pen_site.0 - px;
                let oz = pen_site.1 - pz;
                let dist = ox.hypot(oz);
                let ph = duet_clock(late, idx);
                let imp = ((pulse(ph) - pulse(ph - PULSE_RATE * DT)) / DT).max(0.0);
                let wt = imp * (-(dist * dist) / 1.75 - gap / 2.2).exp();
                disp[0] += ox / (dist + 0.25) * wt;
                disp[1] += oz / (dist + 0.25) * wt;
                stress += wt;
            }
            let curr = [
                0.11 + 0.28 * (0.75 * t - p_phase).sin() + 0.07 * (1.53 * t + 0.71 * p_phase).sin(),
                0.08 * (0.69 * t + 0.51 * p_phase).sin(),
            ];
            let tgt = [curr[0] + 0.94 * disp[0], curr[1] + 0.94 * disp[1]];
            pv[0] += (8.0 * (tgt[0] - pb[0]) - 3.1 * pv[0]) * DT;
            pv[1] += (8.0 * (tgt[1] - pb[1]) - 3.1 * pv[1]) * DT;
            pb[0] += pv[0] * DT;
            pb[1] += pv[1] * DT;
            p_act += (stress - p_act) * (1.0 - (-DT / 0.36).exp());
            pen_bend[k] = pb;
            pen_stim[k] = p_act;
        }

        // Coral Anemone arms + low oral disc (`reef.py:335-384`)
        let mut arng = Rng64::new(2519);
        let mut anemone_arms = Vec::with_capacity(18 * 24);
        for arm in 0..18 {
            let ring = arm < 11;
            let a = if ring {
                arm as f32 * TAU / 11.0
            } else {
                ((arm - 11) as f32 + 0.4) * TAU / 7.0
            };
            let len = if ring { 0.42 } else { 0.27 } * arng.range(0.80, 1.20);
            let rad = if ring { 0.10 } else { 0.042 };
            for iu in 0..24 {
                let u = iu as f32 / 23.0;
                anemone_arms.push((a, u, len, rad));
            }
        }
        let mut anemone_disc = Vec::with_capacity(10 * 32);
        for ir in 0..10 {
            let r = 0.02 + 0.19 * (ir as f32 / 9.0);
            for ia in 0..32 {
                let a = (ia as f32 / 32.0) * TAU;
                let lx = r * a.cos();
                let lift = 0.19 + 0.055 * (1.0 - r / 0.21);
                let lz = r * a.sin();
                let wp = [
                    1.06 + lx,
                    -0.96 + 0.70 * lift + 0.714 * lz,
                    0.44 - 0.714 * lift + 0.70 * lz,
                ];
                let dsh = 0.5 + 0.35 * (1.0 + (12.0 * a).cos()) * 0.5;
                anemone_disc.push(ReefPt {
                    world: wp,
                    glyph: glyph_id(':'),
                    rgb: [154.0 * dsh / 255.0, 90.0 * dsh / 255.0, 63.0 * dsh / 255.0],
                });
            }
        }

        let params = AsciiLabParams::default();
        let mut canvas = AsciiCanvas::default();
        canvas.ensure_layout(css_w, css_h, (params.char_size * 0.78).clamp(4.4, 12.0), false);

        let mut st = Self {
            params,
            canvas,
            anim_time: 16.8, // Starts in iconic full-heart lock matching the video opening
            low_mode: false,
            icons: Vec::new(),
            icons_vis: false,
            css_w,
            css_h,
            dpi_scale,
            pointer_norm: [0.5, 0.5],
            pointer_active: false,
            pointer_pulse: 0.0,
            heart_wave_timer: 0.0,
            orbit,
            start_angle,
            strand_roots,
            heart_bottom,
            heart_x,
            dots,
            reef_pts,
            pen_cells,
            pen_bend,
            pen_stim,
            anemone_arms,
            anemone_disc,
            clear_mask: Vec::new(),
        };
        st.rebuild_grid();
        st
    }

    pub fn resize(&mut self, css_w: f32, css_h: f32, dpi: f32) {
        self.css_w = css_w;
        self.css_h = css_h;
        self.dpi_scale = dpi;
        let eff_cs = (self.params.char_size * 0.78).clamp(4.4, 12.0);
        if self.canvas.ensure_layout(css_w, css_h, eff_cs, self.low_mode) {
            self.rebuild_grid();
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            let eff_cs = (self.params.char_size * 0.78).clamp(4.4, 12.0);
            self.canvas.ensure_layout(self.css_w, self.css_h, eff_cs, low);
            self.rebuild_grid();
        }
    }

    pub fn sync_params(&mut self, next: AsciiLabParams) {
        let changed = self.params != next;
        self.params = next;
        let eff_cs = (self.params.char_size * 0.78).clamp(4.4, 12.0);
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
        self.pointer_norm = [
            (css_x / self.css_w.max(1.0)).clamp(0.0, 1.0),
            (css_y / self.css_h.max(1.0)).clamp(0.0, 1.0),
        ];
        self.pointer_active = true;
        self.pointer_pulse = (self.pointer_pulse + 0.12).min(1.0);
        if kind == 1 {
            self.heart_wave_timer = 1.0;
            self.pointer_pulse = 1.0;
            return true;
        }
        false
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.anim_time = 16.8;
        self.heart_wave_timer = 0.0;
        self.pointer_pulse = 0.0;
        self.rebuild_grid();
    }

    pub fn step(&mut self, dt: f32) {
        let dt_eff = dt.clamp(0.0, 0.05) * self.params.speed;
        self.anim_time = (self.anim_time + dt_eff) % SECONDS;
        self.pointer_pulse = (self.pointer_pulse - dt * 1.6).max(0.0);
        self.heart_wave_timer = (self.heart_wave_timer - dt * 0.9).max(0.0);
        self.rebuild_grid();
    }

    fn sample_orbit(&self, t: f32) -> (f32, f32, f32, f32, f32) {
        let n = self.orbit.len();
        let f = ((t + PRE_ORB) / DT).clamp(0.0, (n - 2) as f32);
        let i = f as usize;
        let w = f - i as f32;
        let a = &self.orbit[i];
        let b = &self.orbit[i + 1];
        let lerp = |x: f32, y: f32| x + (y - x) * w;
        (
            lerp(a.radius, b.radius),
            lerp(a.angle, b.angle),
            lerp(a.height, b.height),
            lerp(a.tilt, b.tilt),
            lerp(a.camera_angle, b.camera_angle),
        )
    }

    fn palette_pair(&self) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3]) {
        match self.params.palette {
            1 => (
                [48.0 / 255.0, 224.0 / 255.0, 178.0 / 255.0],
                [128.0 / 255.0, 255.0 / 255.0, 212.0 / 255.0],
                [255.0 / 255.0, 214.0 / 255.0, 92.0 / 255.0],
                [96.0 / 255.0, 210.0 / 255.0, 152.0 / 255.0],
            ),
            2 => (
                [255.0 / 255.0, 134.0 / 255.0, 186.0 / 255.0],
                [255.0 / 255.0, 220.0 / 255.0, 198.0 / 255.0],
                [152.0 / 255.0, 128.0 / 255.0, 255.0 / 255.0],
                [242.0 / 255.0, 108.0 / 255.0, 122.0 / 255.0],
            ),
            // Exact `swimmer.py` / `duet.py` icy cyan <-> magenta pink duet
            _ => (
                [44.0 / 255.0, 174.0 / 255.0, 233.0 / 255.0],
                [134.0 / 255.0, 219.0 / 255.0, 254.0 / 255.0],
                [86.0 / 255.0, 144.0 / 255.0, 203.0 / 255.0],
                [198.0 / 255.0, 255.0 / 255.0, 243.0 / 255.0],
            ),
        }
    }

    fn interp_heart_width(&self, level: f32) -> f32 {
        if level <= self.heart_bottom[0] {
            return self.heart_x[0];
        }
        let n = self.heart_bottom.len();
        if level >= self.heart_bottom[n - 1] {
            return self.heart_x[n - 1];
        }
        for i in 0..(n - 1) {
            if level <= self.heart_bottom[i + 1] {
                let f = (level - self.heart_bottom[i]) / (self.heart_bottom[i + 1] - self.heart_bottom[i]).max(1e-6);
                return self.heart_x[i] + f * (self.heart_x[i + 1] - self.heart_x[i]);
            }
        }
        self.heart_x[n - 1]
    }

    pub fn rebuild_grid(&mut self) {
        let cols = self.canvas.cols;
        let rows = self.canvas.rows;
        if cols == 0 || rows == 0 {
            return;
        }
        self.canvas.clear([2, 5, 10], 99.0);

        let cw = 12.0f32;
        let ch = 18.0f32;
        let virt_w = cols as f32 * cw;
        let virt_h = rows as f32 * ch;
        let t = self.anim_time;

        let (_, _, cur_h, _, mut cam_a) = self.sample_orbit(t);
        if self.pointer_active && self.params.mouse_interact {
            cam_a += (self.pointer_norm[0] - 0.5) * 0.28;
        }
        let pitch_eff = PITCH
            + if self.pointer_active && self.params.mouse_interact {
                (self.pointer_norm[1] - 0.5) * 0.16
            } else {
                0.0
            };

        let unit_scale = (virt_w / 1080.0).min(virt_h / 1050.0).max(0.45);
        let lens_scale = 500.0 * unit_scale;
        let mid_x = virt_w * 0.50;
        let mid_y = virt_h * 0.45;
        let duet_scale = 1.0 / (1.0 + 0.58 * spread(t));

        let on_screen = |p: [f32; 3]| -> (f32, f32, f32) {
            let seen = spin_x(p, pitch_eff);
            let shrink = lens_scale * CAM_D / (CAM_D + seen[2]).max(0.5);
            (mid_x + shrink * seen[0], mid_y - shrink * seen[1], seen[2])
        };

        // 1. Construct Both Jellyfish (`duet.py:266-304`) + Opening Light Arc
        let (c_dome, c_strand, c_inner, c_hem) = self.palette_pair();
        let surf_l = unit3([-0.52, 0.92, -0.78]);
        let mut key_rot = yaw_y(surf_l, self.start_angle - cam_a);
        let mut opening_val = 0.0f32;
        if t < 6.0 {
            opening_val = 1.0 - spread(t);
            let end_cam = self.orbit[self.orbit.len() - 1].camera_angle;
            let turn_a = self.start_angle - end_cam;
            key_rot = yaw_y(key_rot, opening_val * turn_a.sin().atan2(turn_a.cos()));
        }

        let orb_idx = (((t + PRE_ORB) / DT) as isize).clamp(0, (self.orbit.len() - 1) as isize) as usize;
        let orb_samp = &self.orbit[orb_idx];
        let mut pts: Vec<SwimmerPt> = Vec::with_capacity(if self.low_mode { 5200 } else { 11200 });

        let swap_swimmer_col = |c: [f32; 3], j_idx: usize| -> [f32; 3] {
            if j_idx == 1 {
                [c[2], c[0] * 0.94, c[1] * 0.96]
            } else {
                c
            }
        };

        for j_idx in 0..2 {
            let body_time = duet_clock(t, j_idx);
            let beat_phase = (body_time - FIRST_BEAT).rem_euclid(BEAT);
            let front_pos = beat_phase * FRONT_SPEED;
            let click_glow = self.heart_wave_timer * 0.38;

            let move_pt = |body_p: [f32; 3], when: f32| -> [f32; 3] {
                let scene_t = t + (when - body_time) / PULSE_RATE;
                let (r, a_o, h, tilt, _) = self.sample_orbit(scene_t);
                let ang = a_o + j_idx as f32 * PI;
                let rot = yaw_y(orient(body_p, tilt, ang), -cam_a);
                let diff_a = ang - cam_a;
                [
                    (rot[0] + r * diff_a.cos()) * duet_scale,
                    (rot[1] + h - cur_h) * duet_scale,
                    (rot[2] + r * diff_a.sin()) * duet_scale,
                ]
            };
            let rot_vec = |v: [f32; 3]| -> [f32; 3] {
                let (_, a_o, _, tilt, _) = self.sample_orbit(t);
                let ang = a_o + j_idx as f32 * PI;
                yaw_y(orient(v, tilt, ang), -cam_a)
            };

            // 1a. Dense Bell Dome & Inner Wall (`swimmer.py:30-60`)
            let nv = if self.low_mode { 28 } else { 46 };
            let nphi = if self.low_mode { 72 } else { 128 };
            for iv in 0..nv {
                let v = 0.004 + 0.996 * (iv as f32 / (nv - 1) as f32);
                let tide_base = 0.77;
                let heat = (-((front_pos - 0.75 * v) / 0.18).powi(2)).exp();
                let late = body_time - EDGE_DELAY * v;
                for iphi in 0..nphi {
                    let phi = (iphi as f32 / nphi as f32) * TAU;
                    let tide = tide_base + 0.07 * (5.0 * phi + 0.4).sin() + 0.025 * (2.0 * phi).sin();
                    let band = (-((v - tide) / 0.075).powi(2)).exp();
                    let grain = hash01((v * 35.0).floor(), (phi / TAU * 112.0).floor());
                    let pearl = (-((v - 0.45 - 0.09 * (3.0 * phi).sin()) / 0.12).powi(2)).exp()
                        * ease((grain - 0.68) / 0.24);
                    let shell_c_raw = [
                        c_dome[0] + (0.22 * (1.0 - v) + 0.14 * grain) * 96.0 / 255.0,
                        c_dome[1] + (0.22 * (1.0 - v) + 0.14 * grain) * 138.0 / 255.0,
                        c_dome[2] + (0.22 * (1.0 - v) + 0.14 * grain) * 125.0 / 255.0,
                    ];
                    let shell_col = tint(shell_c_raw, (0.88 * band + 0.52 * pearl).min(1.0));
                    let shell_w = 0.30 + 0.22 * (1.0 - v) + 0.17 * grain + 0.52 * band + 0.22 * pearl;

                    let mut loc = dome_pt(v, phi, late);
                    loc[1] *= BELL_HEIGHT_SCALE;
                    let spot = move_pt(loc, late);

                    let normal = unit3(rot_vec(around(
                        (v * ROLL).sin(),
                        phi + TURN * body_time,
                        (v * ROLL).cos() / BELL_HEIGHT_SCALE,
                    )));
                    let front = spin_x(normal, pitch_eff)[2] < 0.03;
                    let inc = dot3(normal, key_rot);
                    let light = 0.31 + 1.38 * ease((inc + 0.14) / 0.98) + 0.28 * inc.max(0.0).powi(8);

                    if front {
                        let col = scale_rgb(tint(shell_col, (0.40 * heat + click_glow).min(1.0)), light);
                        let wt = (shell_w * light + 0.20 * heat + click_glow * 0.35).clamp(0.0, 1.0);
                        pts.push(SwimmerPt {
                            pos: spot,
                            glyph: ramp_glyph(RAMP, 1.0 - wt),
                            col: swap_swimmer_col(col, j_idx),
                            kind: OWN_SHELL,
                            meta: 0,
                        });
                    } else {
                        let inner_c = scale_rgb(c_inner, 0.17 + 0.41 * light);
                        let ig = INNER_G[((hash01(iphi as f32, iv as f32) * INNER_G.len() as f32) as usize)
                            .min(INNER_G.len() - 1)];
                        pts.push(SwimmerPt {
                            pos: spot,
                            glyph: ig.saturating_sub(32),
                            col: swap_swimmer_col(inner_c, j_idx),
                            kind: OWN_INSIDE,
                            meta: 0,
                        });
                    }
                }
            }

            // Bell Scalloped Hem (`swimmer.py:49-60`)
            let nhem = if self.low_mode { 72 } else { 112 };
            for ih in 0..nhem {
                let phi = (ih as f32 / nhem as f32) * TAU;
                let phi2 = ((ih + 1) as f32 / nhem as f32) * TAU;
                let mut l1 = dome_pt(1.0, phi, body_time);
                l1[1] *= BELL_HEIGHT_SCALE;
                let mut l2 = dome_pt(1.0, phi2, body_time);
                l2[1] *= BELL_HEIGHT_SCALE;
                let w1 = move_pt(l1, body_time);
                let w2 = move_pt(l2, body_time);
                let (x1, y1, z1) = on_screen(w1);
                let (x2, y2, _) = on_screen(w2);
                if z1 < 0.15 {
                    let hn = unit3(rot_vec(around(1.0, phi + TURN * body_time, 0.0)));
                    let hf = 0.32 + 0.83 * ease((dot3(hn, key_rot) + 0.4) / 1.15);
                    pts.push(SwimmerPt {
                        pos: w1,
                        glyph: line_glyph_underscore(x2 - x1, y2 - y1, cw, ch),
                        col: swap_swimmer_col(scale_rgb(c_hem, hf + click_glow), j_idx),
                        kind: OWN_HEM,
                        meta: 0,
                    });
                }
            }

            // 1b. 4 Frilly Oral Ribbons with edge directional strokes (`renderer.py:155-211`)
            let l_rows = if self.low_mode { 32 } else { 58 };
            let l_cols = if self.low_mode { 7 } else { 9 };
            for limb_i in 0..LIMBS {
                let mut limb_pos = vec![[0.0f32; 3]; l_rows * l_cols];
                for ir in 1..l_rows {
                    let down = ir as f32 / (l_rows - 1) as f32;
                    let late = body_time - EDGE_DELAY - LIMB_DELAY * down;
                    let grip = pulse(late);
                    let wander = 0.30 * (0.9 * body_time + limb_i as f32 - 2.2 * down).sin();
                    let bearing = limb_i as f32 * TAU / LIMBS as f32 + 0.4 + TURN * late + 1.4 * down + wander;
                    let bow = (1.0 - 0.45 * grip) * (0.04 + 0.11 * (PI * down.powf(0.8)).sin());
                    let spine = around(bow, bearing, 0.10 - (1.0 + 0.10 * grip) * LIMB_REACH * down);
                    let curl = bearing + 1.2 + 2.6 * down;
                    let taper = (1.0 - down).clamp(0.0, 1.0).powf(0.55) * ease(0.25 + 6.0 * down);
                    let half = LIMB_HALF * taper;
                    for ic in 0..l_cols {
                        let side = -1.0 + 2.0 * (ic as f32 / (l_cols - 1) as f32);
                        let ruffle = 0.05 * side.abs().powf(1.5) * (13.0 * down + 2.6 * side + 2.0 * limb_i as f32 - 3.2 * body_time).sin();
                        let flutter = 0.025 * side.abs() * (9.0 * down + limb_i as f32 - 2.0 * body_time).sin();
                        let s1 = around(half * side, curl, flutter);
                        let s2 = around(ruffle, curl + TAU * 0.25, 0.0);
                        limb_pos[ir * l_cols + ic] = move_pt([spine[0] + s1[0] + s2[0], spine[1] + s1[1] + s2[1], spine[2] + s1[2] + s2[2]], late);
                    }
                }
                for ir in 1..l_rows {
                    let down = ir as f32 / (l_rows - 1) as f32;
                    let route = JOIN_LIMB + 0.5 * down;
                    let heat = (-((front_pos - route) / 0.18).powi(2)).exp();
                    for ic in 0..l_cols {
                        let side = -1.0 + 2.0 * (ic as f32 / (l_cols - 1) as f32);
                        let wp = limb_pos[ir * l_cols + ic];
                        let prev_wp = limb_pos[(ir - 1) * l_cols + ic];
                        let is_rim = side.abs() > 0.88;
                        let margin = ease((side.abs() - 0.55) / 0.4);
                        let lbase = [
                            (86.0 + margin * 110.0) / 255.0,
                            (158.0 + margin * 79.0) / 255.0,
                            (207.0 + margin * 41.0) / 255.0,
                        ];
                        let ld = (down * 42.0).floor();
                        let ls = ((side + 1.0) * 3.5).floor();
                        let ltone = (0.25 + 0.75 * (0.31 * ld + 0.37 * ls + limb_i as f32 * 0.4).fract().max(0.0).powf(0.8)
                            - 0.3 * hash01(ld + limb_i as f32 * 9.0, ls))
                        .clamp(0.0, 1.0);
                        let glow_val = if is_rim { 0.80 } else { 0.62 + 0.38 * ltone } + 0.22 * heat + click_glow * 0.3;
                        let col = scale_rgb(tint(lbase, (0.55 * heat + click_glow).min(1.0)), glow_val);
                        let gl = if is_rim {
                            let (sx, sy, _) = on_screen(wp);
                            let (px, py, _) = on_screen(prev_wp);
                            line_glyph_underscore(sx - px, sy - py, cw, ch)
                        } else {
                            ramp_glyph(RAMP, 1.0 - ltone)
                        };
                        let chosen = (limb_i % 2 == 0) && (ic == 0 || ic == l_cols - 1);
                        let meta = (if chosen { 0x8000 } else { 0 }) | ((j_idx as u16) << 12) | ((limb_i as u16) << 9) | (ic as u16);
                        pts.push(SwimmerPt {
                            pos: wp,
                            glyph: gl,
                            col: swap_swimmer_col(col, j_idx),
                            kind: OWN_LIMB,
                            meta,
                        });
                    }
                }
            }

            // 1c. 14 Continuous Sub-Interpolated Trailing Strands (`duet.py:131-145`)
            let sub = if self.low_mode { 2 } else { STRAND_SUB };
            for s_idx in 0..STRANDS {
                let raw_pts = &orb_samp.nodes[j_idx * STRANDS + s_idx];
                let total_pts = (STRAND_NODES - 1) * sub + 1;
                let mut world_s = Vec::with_capacity(total_pts);
                for seg in 0..(STRAND_NODES - 1) {
                    let p0 = raw_pts[seg];
                    let p1 = raw_pts[seg + 1];
                    for m in 0..sub {
                        let f = m as f32 / sub as f32;
                        let w = [p0[0] + (p1[0] - p0[0]) * f, p0[1] + (p1[1] - p0[1]) * f, p0[2] + (p1[2] - p0[2]) * f];
                        let shifted = [w[0], w[1] - cur_h, w[2]];
                        let yawed = yaw_y(shifted, -cam_a);
                        world_s.push([yawed[0] * duet_scale, yawed[1] * duet_scale, yawed[2] * duet_scale]);
                    }
                }
                let plast = raw_pts[STRAND_NODES - 1];
                let yawed_l = yaw_y([plast[0], plast[1] - cur_h, plast[2]], -cam_a);
                world_s.push([yawed_l[0] * duet_scale, yawed_l[1] * duet_scale, yawed_l[2] * duet_scale]);

                for idx in 0..world_s.len() {
                    let p0 = world_s[idx.saturating_sub(1)];
                    let p1 = world_s[(idx + 1).min(world_s.len() - 1)];
                    let (sx0, sy0, _) = on_screen(p0);
                    let (sx1, sy1, _) = on_screen(p1);
                    let (_, _, z) = on_screen(world_s[idx]);
                    let down_u = idx as f32 / (world_s.len() - 1).max(1) as f32;
                    let glow_f = (0.80 - 0.16 * down_u) * (0.86 + 0.14 * ease(0.5 - z)) + click_glow * 0.28;
                    let chosen = s_idx % 3 == 1;
                    let meta = (if chosen { 0x8000 } else { 0 }) | ((j_idx as u16) << 12) | ((s_idx as u16) << 7) | (idx as u16);
                    pts.push(SwimmerPt {
                        pos: world_s[idx],
                        glyph: line_glyph_underscore(sx1 - sx0, sy1 - sy0, cw, ch),
                        col: swap_swimmer_col(scale_rgb(c_strand, glow_f), j_idx),
                        kind: OWN_STRAND,
                        meta,
                    });
                }
            }
        }

        // 2. Parametric Heart Pose + Interior Overlap Weave + Directional Strokes (`duet.py:162-235`)
        let closed_amt = 1.0 - spread(t);
        let bearing = orb_samp.angle - cam_a;
        if closed_amt > 0.005 && !pts.is_empty() {
            let mut side_pts: Vec<[f32; 3]> = Vec::with_capacity(pts.len());
            let mut min_xp = f32::INFINITY;
            let mut max_xp = f32::NEG_INFINITY;
            let mut max_abs_left = 1e-4f32;
            let mut max_abs_right = 1e-4f32;

            for sp in &pts {
                let s = spin_x(yaw_y(sp.pos, -bearing), pitch_eff);
                let persp = CAM_D / (CAM_D + s[2]).max(0.5);
                let xp = s[0] * persp;
                min_xp = min_xp.min(xp);
                max_xp = max_xp.max(xp);
                if s[0] < 0.0 {
                    max_abs_left = max_abs_left.max(xp.abs());
                } else {
                    max_abs_right = max_abs_right.max(xp.abs());
                }
                side_pts.push(s);
            }
            let radius = (min_xp.abs() + max_xp) * 0.5;
            const KNOTS: usize = 49;
            let mut bot_l = [f32::INFINITY; KNOTS];
            let mut top_l = [f32::NEG_INFINITY; KNOTS];
            let mut bot_r = [f32::INFINITY; KNOTS];
            let mut top_r = [f32::NEG_INFINITY; KNOTS];
            for s in &side_pts {
                let persp = CAM_D / (CAM_D + s[2]).max(0.5);
                let up = s[1] * persp;
                if s[0] < 0.0 {
                    let u = ((s[0] * persp).abs() / max_abs_left).clamp(0.0, 1.0);
                    let bi = ((u * (KNOTS - 1) as f32).round() as usize).min(KNOTS - 1);
                    bot_l[bi] = bot_l[bi].min(up);
                    top_l[bi] = top_l[bi].max(up);
                } else {
                    let u = ((s[0] * persp).abs() / max_abs_right).clamp(0.0, 1.0);
                    let bi = ((u * (KNOTS - 1) as f32).round() as usize).min(KNOTS - 1);
                    bot_r[bi] = bot_r[bi].min(up);
                    top_r[bi] = top_r[bi].max(up);
                }
            }

            let sample_bound = |arr: &[f32; KNOTS], u: f32| -> f32 {
                let f = u.clamp(0.0, 1.0) * (KNOTS - 1) as f32;
                let mut i0 = (f as usize).min(KNOTS - 1);
                if !arr[i0].is_finite() {
                    for d in 1..KNOTS {
                        if i0 + d < KNOTS && arr[i0 + d].is_finite() {
                            i0 += d;
                            break;
                        }
                        if i0 >= d && arr[i0 - d].is_finite() {
                            i0 -= d;
                            break;
                        }
                    }
                }
                arr[i0]
            };

            let pulse_heart = 1.0 + self.heart_wave_timer * 0.08 * (1.0 - self.heart_wave_timer).sin();
            let mut turn_vals = vec![0.0f32; pts.len()];
            for (i, sp) in pts.iter_mut().enumerate() {
                let mut s = side_pts[i];
                let persp = (CAM_D / (CAM_D + s[2]).max(0.5)).max(0.15);
                let xp = s[0] * persp;
                let up = s[1] * persp;
                let (low, high, max_a) = if s[0] < 0.0 {
                    let u = (xp.abs() / max_abs_left).clamp(0.0, 1.0);
                    (sample_bound(&bot_l, u), sample_bound(&top_l, u), max_abs_left)
                } else {
                    let u = (xp.abs() / max_abs_right).clamp(0.0, 1.0);
                    (sample_bound(&bot_r, u), sample_bound(&top_r, u), max_abs_right)
                };
                let u = (xp.abs() / max_a).clamp(0.0, 1.0);
                let within = ((up - low) / (high - low).max(1e-5)).clamp(0.0, 1.0);
                let (h_lo, h_hi) = heart_edges(u);
                let target_y = radius * (h_lo + (h_hi - h_lo) * within) - 0.16;
                let sign_x = if s[0] < 0.0 { -1.0 } else { 1.0 };
                s[0] += (sign_x * radius * u - xp) * closed_amt / persp;
                s[1] += (target_y - up) * closed_amt / persp;
                s[0] *= pulse_heart;
                s[1] *= pulse_heart;

                // Exact `overlap()` weave for chosen strands & chosen oral ribbons
                if (sp.kind == OWN_STRAND || sp.kind == OWN_LIMB) && (sp.meta & 0x8000 != 0) {
                    let fx = s[0] * (CAM_D / (CAM_D + s[2]).max(0.5));
                    let fy = s[1] * (CAM_D / (CAM_D + s[2]).max(0.5));
                    let level = (fy + 0.16) / radius.max(1e-4);
                    let w_h = radius * self.interp_heart_width(level);
                    let u_rel = (fx / w_h.max(1e-5)).clamp(-1.0, 1.0);
                    let lag = 7.8 * ease((0.36 - level) / 1.10);
                    let (_, a_lag, _, _, _) = self.sample_orbit(t - lag);
                    let mut turn_o = (a_lag - orb_samp.angle)
                        * closed_amt
                        * ease((1.0 - u_rel.abs()) / 0.26)
                        * ease((level - self.heart_bottom[0]) / 0.15);
                    if self.pointer_active && self.params.mouse_interact {
                        turn_o += 0.35 * self.pointer_pulse * (self.pointer_norm[0] - 0.5);
                    }
                    turn_vals[i] = turn_o;
                    let c = turn_o.cos();
                    let sn = turn_o.sin();
                    let v_rel = (1.0 - u_rel * u_rel).max(0.0).sqrt() * (s[2] / 0.18).tanh();
                    let nx = fx + w_h * (u_rel * (c - 1.0) - v_rel * sn);
                    let nz = s[2] + 0.65 * w_h * (u_rel * sn + v_rel * (c - 1.0));
                    let n_persp = (CAM_D / (CAM_D + nz).max(0.5)).max(0.15);
                    s[0] = nx / n_persp;
                    s[1] = fy / n_persp;
                    s[2] = nz;
                    let col_mul = 1.0 + 0.20 * (turn_o * 0.5).sin().abs();
                    sp.col = [sp.col[0] * col_mul, sp.col[1] * col_mul, sp.col[2] * col_mul];
                }
                sp.pos = yaw_y(spin_x(s, -pitch_eff), bearing);
            }

            // Recompute tangent strokes on twisted interior strands so lacy lines orient accurately
            for i in 1..(pts.len().saturating_sub(1)) {
                if pts[i].kind == OWN_STRAND && turn_vals[i].abs() > 0.01 && pts[i - 1].kind == OWN_STRAND && pts[i + 1].kind == OWN_STRAND && (pts[i].meta >> 7) == (pts[i + 1].meta >> 7) {
                    let (x0, y0, _) = on_screen(pts[i - 1].pos);
                    let (x1, y1, _) = on_screen(pts[i + 1].pos);
                    pts[i].glyph = line_glyph_underscore(x1 - x0, y1 - y0, cw, ch);
                }
            }
        }

        // 3. `shade_tentacles` Orthographic Shadow Occlusion (`duet.py:241-264`)
        let release = ease((t - 8.0) / 8.0) * (1.0 - spread(t)).max(opening_val);
        if release < 0.99 {
            let right_ax = unit3(cross3(key_rot, [0.0, 1.0, 0.0]));
            let up_ax = cross3(right_ax, key_rot);
            const SM_W: usize = 96;
            const SM_H: usize = 96;
            let mut sm = [f32::NEG_INFINITY; SM_W * SM_H];
            for sp in &pts {
                if sp.kind != OWN_LIMB && sp.kind != OWN_STRAND {
                    let u = ((dot3(sp.pos, right_ax) * 24.0) + 48.0).round() as i32;
                    let v = ((dot3(sp.pos, up_ax) * 24.0) + 48.0).round() as i32;
                    if u >= 0 && u < SM_W as i32 && v >= 0 && v < SM_H as i32 {
                        let d = dot3(sp.pos, key_rot);
                        let idx = v as usize * SM_W + u as usize;
                        sm[idx] = sm[idx].max(d);
                    }
                }
            }
            for sp in &mut pts {
                if sp.kind == OWN_LIMB || sp.kind == OWN_STRAND {
                    let u = ((dot3(sp.pos, right_ax) * 24.0) + 48.0).round() as i32;
                    let v = ((dot3(sp.pos, up_ax) * 24.0) + 48.0).round() as i32;
                    let d = dot3(sp.pos, key_rot);
                    let mut occ = 0.0f32;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let sx = u + dx;
                            let sy = v + dy;
                            if sx >= 0 && sx < SM_W as i32 && sy >= 0 && sy < SM_H as i32 {
                                let sd = sm[sy as usize * SM_W + sx as usize];
                                if sd.is_finite() {
                                    occ += ease((sd - d - 0.015) / 0.13);
                                }
                            }
                        }
                    }
                    let (_, _, z) = on_screen(sp.pos);
                    let near = ease((0.48 - z) / 0.96);
                    let gain = (0.46 + 0.35 * near) * (1.0 - 0.52 * (occ / 9.0));
                    let f = gain + (1.0 - gain) * release;
                    sp.col = [sp.col[0] * f, sp.col[1] * f, sp.col[2] * f];
                }
            }
        }

        // 4. Rasterize Swimmers First (preserving crisp outlines + `DISPLAY_INK` pow 0.70 gamma)
        let ptr_px = self.pointer_norm[0] * virt_w;
        let ptr_py = self.pointer_norm[1] * virt_h;
        let gamma_u8 = |v: f32| -> u8 { clamp_u8(v.clamp(0.0, 1.0).powf(0.70)) };

        for sp in &pts {
            let (mut sx, mut sy, mut sz) = on_screen(sp.pos);
            if sp.kind == OWN_HEM {
                sz -= 0.03;
            } else if sp.kind == OWN_INSIDE {
                sz += 0.004;
            }
            if self.pointer_active && self.params.mouse_interact && (sp.kind == OWN_STRAND || sp.kind == OWN_LIMB) {
                let dx = sx - ptr_px;
                let dy = sy - ptr_py;
                let d2 = dx * dx + dy * dy;
                let push = (-d2 / (120.0 * 120.0)).exp() * (22.0 + 36.0 * self.pointer_pulse);
                let inv = 1.0 / d2.sqrt().max(12.0);
                sx += dx * inv * push;
                sy += dy * inv * push;
            }
            let c = (sx / cw).floor() as i32;
            let r = (sy / ch).floor() as i32;
            let glow_b = if sp.kind == OWN_SHELL || sp.kind == OWN_HEM {
                205
            } else if sp.kind == OWN_LIMB {
                135
            } else {
                98
            };
            self.canvas.plot_z_less(
                c,
                r,
                sz,
                sp.glyph,
                [gamma_u8(sp.col[0]), gamma_u8(sp.col[1]), gamma_u8(sp.col[2])],
                glow_b,
                sp.kind,
            );
        }

        // Bell silhouette outline pass
        let prev_own = self.canvas.owner.clone();
        let is_bell = |o: u8| o == OWN_SHELL || o == OWN_INSIDE || o == OWN_HEM;
        for r in 1..(rows - 1) {
            for c in 1..(cols - 1) {
                let i = r * cols + c;
                if prev_own[i] != OWN_SHELL {
                    continue;
                }
                let u_o = !is_bell(prev_own[(r - 1) * cols + c]);
                let d_o = !is_bell(prev_own[(r + 1) * cols + c]);
                let l_o = !is_bell(prev_own[r * cols + c - 1]);
                let r_o = !is_bell(prev_own[r * cols + c + 1]);
                let edge = if l_o && u_o {
                    Some(glyph_id('/'))
                } else if r_o && u_o {
                    Some(glyph_id('\\'))
                } else if l_o && d_o && !u_o {
                    Some(glyph_id('\\'))
                } else if r_o && d_o && !u_o {
                    Some(glyph_id('/'))
                } else if u_o {
                    Some(glyph_id('-'))
                } else if l_o || r_o {
                    Some(glyph_id('|'))
                } else {
                    None
                };
                if let Some(g) = edge {
                    let p = i * 4;
                    self.canvas.cells[p + 3] = g;
                    for k in 0..3 {
                        let v = (self.canvas.cells[p + k] as f32 / 255.0) * 0.65 + 0.35;
                        self.canvas.cells[p + k] = clamp_u8(v * 0.98);
                    }
                    self.canvas.bg[p + 3] = 242;
                }
            }
        }

        // Build 1-cell `reef_clearance` mask around lit foreground swimmers (`reef.py:224-255`)
        self.clear_mask.resize(cols * rows, false);
        self.clear_mask.fill(false);
        for r in 0..rows {
            for c in 0..cols {
                let p = (r * cols + c) * 4;
                let max_c = self.canvas.cells[p].max(self.canvas.cells[p + 1]).max(self.canvas.cells[p + 2]);
                if self.canvas.cells[p + 3] != 0 && max_c > 70 {
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let rr = r as i32 + dy;
                            let cc = c as i32 + dx;
                            if rr >= 0 && rr < rows as i32 && cc >= 0 && cc < cols as i32 {
                                self.clear_mask[rr as usize * cols + cc as usize] = true;
                            }
                        }
                    }
                }
            }
        }

        // 5. Render Diagonal Water Light Shafts into clear background cells (`reef.py:261-304`)
        const CRESTS: [(f32, f32, f32, f32); 6] = [
            (-0.13, 0.47, 0.033, 0.62),
            (0.025, 0.40, 0.049, 1.00),
            (0.145, 0.32, 0.022, 0.68),
            (0.240, 0.31, 0.046, 0.52),
            (0.730, 0.24, 0.036, 0.41),
            (0.900, 0.33, 0.050, 0.58),
        ];
        let phase_l = TAU * (t % SECONDS) / SECONDS;
        for r in 0..rows {
            let depth_y = (r as f32 + 0.5) / rows as f32;
            for c in 0..cols {
                let idx = r * cols + c;
                if self.clear_mask[idx] || self.canvas.cells[idx * 4 + 3] != 0 {
                    continue;
                }
                let grain = hash01(r as f32 + 7.0, c as f32 + 39.0);
                let hatch = ((2 * r as i32 - c as i32).rem_euclid(5) == 0) || (grain > 0.89 && r % 2 == 0);
                if !hatch {
                    continue;
                }
                let x = (c as f32 + 0.5) / cols as f32;
                let mut light = 0.0f32;
                for &(origin, lean, width, trans) in &CRESTS {
                    let swell = phase_l - 6.7 * origin;
                    let ripple = 2.0 * phase_l + 13.0 * origin;
                    let slope = lean + 0.072 * swell.cos() + 0.032 * ripple.cos();
                    let crest = origin + 0.022 * swell.sin() + 0.010 * ripple.sin();
                    let rad = width * (1.0 + 0.20 * ripple.sin()) + 0.024 * depth_y;
                    let across = (x - crest - slope * depth_y) / rad;
                    let focus = 0.83 + 0.17 * (ripple + 0.6).cos();
                    light += trans * focus * (-(across * across)).exp();
                }
                light *= (-1.58 * depth_y).exp();
                light *= 1.0 - 0.60 * (-((x - 0.5) / 0.205).powi(2) - ((depth_y - 0.5) / 0.34).powi(2)).exp();
                let intensity = light.min(1.0) * (0.65 + 0.35 * grain);
                if intensity > 0.018 {
                    let ch_g = if grain < 0.14 { '\'' } else { '\\' };
                    self.canvas.set_cell(
                        c as i32,
                        r as i32,
                        glyph_id(ch_g),
                        [
                            clamp_u8(intensity * 92.0 / 255.0),
                            clamp_u8(intensity * 152.0 / 255.0),
                            clamp_u8(intensity * 181.0 / 255.0),
                        ],
                        0,
                        OWN_EMPTY,
                    );
                }
            }
        }

        // 6. Render Reef Seabed, Boulders, Wake-Driven Sea Pen & 3-Layer Coral Anemone
        let h0 = self.orbit[PRE_STEPS].height;
        let reef_fade = if t > 16.0 {
            1.0 - ease((t - 16.0) / 3.9)
        } else {
            1.0
        };
        let reef_arrive = if t > 16.0 {
            ease((t - 16.0) / 3.9)
        } else {
            0.0
        };
        let sample_orbit_slice = |orbit: &[OrbitSample], ts: f32| -> (f32, f32) {
            let n = orbit.len();
            let f = ((ts + PRE_ORB) / DT).clamp(0.0, (n - 2) as f32);
            let i = f as usize;
            let w = f - i as f32;
            let a = &orbit[i];
            let b = &orbit[i + 1];
            (a.height + (b.height - a.height) * w, a.camera_angle + (b.camera_angle - a.camera_angle) * w)
        };
        let orbit_ref = &self.orbit;
        let start_angle_val = self.start_angle;
        let reef_pts_ref = &self.reef_pts;
        let pen_cells_ref = &self.pen_cells;
        let pen_bend_ref = &self.pen_bend;
        let pen_stim_ref = &self.pen_stim;
        let anemone_disc_ref = &self.anemone_disc;
        let anemone_arms_ref = &self.anemone_arms;

        let plot_reef_time = |canvas: &mut AsciiCanvas,
                              clear_mask: &[bool],
                              t_sample: f32,
                              alpha_gain: f32| {
            let (r_h, r_cam_a) = sample_orbit_slice(orbit_ref, t_sample);
            let r_h_shift = ((r_h - h0) * 0.28).clamp(-0.38, 0.38);
            let r_scale = 1.0 / (1.0 + 0.58 * spread(t_sample));
            let r_phase = TAU * t_sample.rem_euclid(SECONDS) / SECONDS;
            let k_pen = (((t_sample + PRE_ORB) / DT) as isize).clamp(0, (pen_bend_ref.len() - 1) as isize) as usize;

            let try_plot_reef = |cv: &mut AsciiCanvas, world_p: [f32; 3], gl: u8, col: [f32; 3], bloom: u8| {
                let mut p = [world_p[0], world_p[1] + 0.44 - r_h_shift, world_p[2]];
                p = yaw_y(p, start_angle_val - r_cam_a);
                p = [p[0] * r_scale, p[1] * r_scale, p[2] * r_scale];
                let (sx, sy, sz) = on_screen(p);
                let c = (sx / cw).floor() as i32;
                let r = (sy / ch).floor() as i32;
                if c >= 0 && c < cols as i32 && r >= 0 && r < rows as i32 {
                    let idx = r as usize * cols + c as usize;
                    if !clear_mask[idx] {
                        cv.plot_z_less(
                            c,
                            r,
                            sz + 1.2,
                            gl,
                            [
                                clamp_u8(col[0] * alpha_gain),
                                clamp_u8(col[1] * alpha_gain),
                                clamp_u8(col[2] * alpha_gain),
                            ],
                            bloom,
                            OWN_REEF,
                        );
                    }
                }
            };

            for rp in reef_pts_ref {
                try_plot_reef(canvas, rp.world, rp.glyph, rp.rgb, 0);
            }

            // Sea Pen along smooth spline from root
            const PEN_SPINE_N: usize = 24;
            let mut spine = [[0.0f32; 3]; PEN_SPINE_N];
            let mut tangent = [[0.0f32; 3]; PEN_SPINE_N];
            spine[0] = [-0.82, -1.49, 0.52];
            for si in 0..PEN_SPINE_N {
                let s = si as f32 / (PEN_SPINE_N - 1) as f32;
                let del_k = ((((t_sample - 0.68 * s.powf(1.7)) + PRE_ORB) / DT) as isize)
                    .clamp(0, (pen_bend_ref.len() - 1) as isize) as usize;
                let b = pen_bend_ref[del_k];
                let tg = unit3([
                    -0.71 * (0.55 + 0.86 * s) + b[0] * s.powf(1.4),
                    1.0,
                    0.09 * 0.31f32.sin() + b[1] * s.powf(1.4),
                ]);
                tangent[si] = tg;
                if si > 0 {
                    let step = 1.25 / (PEN_SPINE_N - 1) as f32;
                    spine[si] = [
                        spine[si - 1][0] + 0.5 * (tangent[si - 1][0] + tg[0]) * step,
                        spine[si - 1][1] + 0.5 * (tangent[si - 1][1] + tg[1]) * step,
                        spine[si - 1][2] + 0.5 * (tangent[si - 1][2] + tg[2]) * step,
                    ];
                }
            }
            for pc in pen_cells_ref {
                let f = pc.station * (PEN_SPINE_N - 1) as f32;
                let i0 = (f as usize).min(PEN_SPINE_N - 2);
                let w = f - i0 as f32;
                let pos_base = [
                    spine[i0][0] + (spine[i0 + 1][0] - spine[i0][0]) * w,
                    spine[i0][1] + (spine[i0 + 1][1] - spine[i0][1]) * w,
                    spine[i0][2] + (spine[i0 + 1][2] - spine[i0][2]) * w,
                ];
                let ax = tangent[i0];
                let plane = 0.31 + 0.38 * (3.0 * pc.station + 51.0).sin() + 0.09 * pc.station + 0.31 * pen_bend_ref[k_pen][0];
                let w_raw = [plane.cos(), 0.0, plane.sin()];
                let d_wa = dot3(w_raw, ax);
                let wd = unit3([w_raw[0] - d_wa * ax[0], w_raw[1] - d_wa * ax[1], w_raw[2] - d_wa * ax[2]]);
                let nrm = cross3(wd, ax);
                let h = 1.25;
                let pos_w = [
                    pos_base[0] + h * (pc.span * wd[0] + pc.normal * nrm[0] + pc.lift * ax[0]),
                    pos_base[1] + h * (pc.span * wd[1] + pc.normal * nrm[1] + pc.lift * ax[1]),
                    pos_base[2] + h * (pc.span * wd[2] + pc.normal * nrm[2] + pc.lift * ax[2]),
                ];
                let mut col = pc.ink;
                if pc.polyp {
                    let drv_k = ((((t_sample - 0.90 * (1.0 - pc.station)) + PRE_ORB) / DT) as isize)
                        .clamp(0, (pen_stim_ref.len() - 1) as isize) as usize;
                    let glw = 1.55 * ease((pen_stim_ref[drv_k] - 0.075) / 0.44);
                    col[0] += glw * 76.0 / 255.0;
                    col[1] += glw * 72.0 / 255.0;
                    col[2] += glw * 36.0 / 255.0;
                }
                try_plot_reef(canvas, pos_w, pc.glyph, col, if pc.polyp { 95 } else { 15 });
            }

            // Anemone oral disc & 3-layer thick arms (`reef.py:348-383`)
            for dp in anemone_disc_ref {
                try_plot_reef(canvas, dp.world, dp.glyph, dp.rgb, 10);
            }
            for &(a, u, len, rad) in anemone_arms_ref {
                let bend = 0.08 * (3.0 * r_phase - 0.8 * u + 0.4 * a.sin()).sin() * u.powf(1.7);
                let span = rad + len * (0.76 * u + 0.22 * (PI * u).sin());
                let aa = a + 0.32 * u * u * (3.0 * a + 1.1).sin();
                let lx = span * aa.cos() + bend;
                let lz = span * aa.sin();
                let ly = len * (0.78 * u + 0.55 * (0.91 * PI * u).sin()) + 0.025 * (2.0 * a + r_phase).sin() * u;
                let width = (0.016 + 0.036 * (PI * u).sin().powf(0.65)) * (1.0 - 0.40 * u);
                let rx = -aa.sin() * width;
                let rz = aa.cos() * width;

                let tip = ease((u - 0.80) / 0.17);
                let lit = 0.73 + 0.21 * (a + 1.1).cos() + 0.06 * (r_phase + a).sin();
                let col = [
                    ((129.0 + u * 26.0 + tip * 53.0) * lit) / 255.0,
                    ((77.0 + u * 36.0 + tip * 44.0) * lit) / 255.0,
                    ((74.0 + u * 22.0 + tip * 30.0) * lit) / 255.0,
                ];
                let gl_ch = if tip > 0.6 { 'o' } else if a.cos() > 0.18 { '/' } else { '\\' };
                let gl = glyph_id(gl_ch);
                let bl = (tip * 85.0) as u8;
                for (dx, dz, mul) in [(-rx, -rz, 0.74f32), (rx, rz, 0.86), (0.0, 0.0, 1.0)] {
                    let wp = [
                        1.06 + lx + dx,
                        -0.96 + 0.70 * ly + 0.714 * (lz + dz),
                        0.44 - 0.714 * ly + 0.70 * (lz + dz),
                    ];
                    try_plot_reef(canvas, wp, gl, [col[0] * mul, col[1] * mul, col[2] * mul], bl);
                }
            }
        };

        if reef_fade > 0.02 {
            plot_reef_time(&mut self.canvas, &self.clear_mask, t, reef_fade);
        }
        if reef_arrive > 0.02 {
            plot_reef_time(&mut self.canvas, &self.clear_mask, (t - SECONDS).rem_euclid(SECONDS), reef_arrive);
        }

        // 7. Ambient Deep-Water Plankton Spores (`duet.py:307-315`)
        for d in &self.dots {
            let mut bearing = d[0] * TAU - cam_a;
            let r_w = 1.1 + 2.0 * d[2];
            let h_w = (d[1] * 5.0 - cur_h + 2.5).rem_euclid(5.0) - 2.5;
            if self.pointer_active && self.params.mouse_interact {
                bearing += (self.pointer_norm[0] - 0.5) * 0.18 * (0.5 + d[2]);
            }
            let dust = around(r_w * duet_scale, bearing, h_w * duet_scale);
            let (sx, sy, sz) = on_screen(dust);
            let c = (sx / cw).floor() as i32;
            let r = (sy / ch).floor() as i32;
            if let Some(idx) = self.canvas.idx(c, r) {
                if self.canvas.cells[idx * 4 + 3] == 0 {
                    let dim = (0.8 - 0.1 * sz).clamp(0.25, 1.0);
                    self.canvas.set_cell(
                        c,
                        r,
                        glyph_id(if d[2] > 0.8 { '+' } else { '.' }),
                        [clamp_u8(107.0 / 255.0 * dim), clamp_u8(144.0 / 255.0 * dim), clamp_u8(162.0 / 255.0 * dim)],
                        35,
                        OWN_EMPTY,
                    );
                }
            }
        }

        // 8. Interactive Click Bioluminescent Heart Ripple Wave
        if self.heart_wave_timer > 0.02 {
            let age = 1.0 - self.heart_wave_timer;
            let rad_c = 4.0 + age * (cols as f32 * 0.28);
            let fade = self.heart_wave_timer;
            let ring_col = [clamp_u8(0.95 * fade), clamp_u8(0.65 * fade), clamp_u8(0.98 * fade)];
            for ia in 0..48 {
                let a = ia as f32 * TAU / 48.0;
                let hx = 16.0 * a.sin().powi(3) / 16.0;
                let hy = -heart_height_fn(a);
                let cc = (self.pointer_norm[0] * cols as f32 + hx * rad_c).round() as i32;
                let rr = (self.pointer_norm[1] * rows as f32 + hy * rad_c * 0.64).round() as i32;
                if let Some(idx) = self.canvas.idx(cc, rr) {
                    if self.canvas.cells[idx * 4 + 3] == 0 {
                        self.canvas.set_cell(cc, rr, glyph_id('+'), ring_col, 230, OWN_EMPTY);
                    }
                }
            }
        }

        self.canvas.mark_dirty();
    }
}
