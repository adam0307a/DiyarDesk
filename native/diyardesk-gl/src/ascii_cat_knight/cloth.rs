//! Smooth 24×16 2D Verlet cloth simulation + per-vertex fold lighting + trailing
//! wind wisps for the Cat Knight's cape (`640×820` reference space).

use super::portrait::{REF_H, REF_W};

pub const NX: usize = 24;
pub const NY: usize = 16;
pub const VERT_COUNT: usize = NX * NY;

const CAPE_MIN_X: f32 = 318.0;
const CAPE_MAX_X: f32 = 626.0;
const CAPE_TOP_Y: f32 = 450.0;
const CAPE_BOT_Y: f32 = 738.0;

#[derive(Clone, Copy)]
struct Constraint {
    a: usize,
    b: usize,
    rest_len: f32,
}

#[derive(Clone, Copy)]
pub struct WindWisp {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max_life: f32,
    pub seed: f32,
}

pub struct CapeCloth {
    pub rest: Vec<[f32; 2]>,
    pub pos: Vec<[f32; 2]>,
    pub prev: Vec<[f32; 2]>,
    pub pin: Vec<bool>,
    pub reach: Vec<f32>,
    constraints: Vec<Constraint>,
    pub wisps: Vec<WindWisp>,
}

#[inline]
fn wind_gust(t: f32, scale: f32) -> f32 {
    let base = 0.88 + 0.42 * (t * 0.95).sin() + 0.22 * (t * 2.15 + 0.7).cos() + 0.14 * (t * 3.4).sin();
    (base * scale).clamp(0.18, 2.4)
}

#[inline]
pub fn cape_wind_scalar(t: f32, wind_scale: f32) -> f32 {
    wind_gust(t, wind_scale)
}

impl Default for CapeCloth {
    fn default() -> Self {
        let mut rest = Vec::with_capacity(VERT_COUNT);
        let mut pin = Vec::with_capacity(VERT_COUNT);
        let mut reach = Vec::with_capacity(VERT_COUNT);

        for r in 0..NY {
            let v = r as f32 / (NY - 1) as f32;
            for c in 0..NX {
                let u = c as f32 / (NX - 1) as f32;
                // Natural arched wind-swept knight cloak curve (slopes smoothly down from shoulder,
                // then lifts & billows in a graceful S-curve out to the right tail)
                let flare_x = u * (238.0 + 64.0 * v.powf(0.84));
                let top_arc = 42.0 * u.powf(0.90) - 28.0 * (u * std::f32::consts::PI).sin();
                let drape_y = top_arc * (1.0 - v * 0.45) + v * (152.0 + 96.0 * u.powf(0.75));
                let x = CAPE_MIN_X + flare_x - 20.0 * v * (1.0 - u);
                let y = CAPE_TOP_Y + drape_y;
                rest.push([x.clamp(CAPE_MIN_X - 24.0, CAPE_MAX_X), y.clamp(CAPE_TOP_Y - 12.0, CAPE_BOT_Y)]);
                pin.push(c == 0 || (r == 0 && c <= 2));
                reach.push(u.powf(0.82));
            }
        }

        let mut constraints = Vec::with_capacity(VERT_COUNT * 4);
        let add_c = |out: &mut Vec<Constraint>, r0: usize, c0: usize, r1: usize, c1: usize| {
            let a = r0 * NX + c0;
            let b = r1 * NX + c1;
            let dx = rest[b][0] - rest[a][0];
            let dy = rest[b][1] - rest[a][1];
            out.push(Constraint {
                a,
                b,
                rest_len: (dx * dx + dy * dy).sqrt().max(1.0),
            });
        };

        for r in 0..NY {
            for c in 0..NX {
                if c + 1 < NX {
                    add_c(&mut constraints, r, c, r, c + 1);
                }
                if r + 1 < NY {
                    add_c(&mut constraints, r, c, r + 1, c);
                }
                if r + 1 < NY && c + 1 < NX {
                    add_c(&mut constraints, r, c, r + 1, c + 1);
                }
                if r + 1 < NY && c > 0 {
                    add_c(&mut constraints, r, c, r + 1, c - 1);
                }
            }
        }

        let pos = rest.clone();
        let prev = rest.clone();
        let mut wisps = Vec::with_capacity(28);
        for i in 0..28 {
            let frac = i as f32 / 28.0;
            let hr = (i * 5) % NY;
            let p = rest[hr * NX + (NX - 1)];
            wisps.push(WindWisp {
                x: p[0] + frac * 48.0,
                y: p[1] + (frac - 0.5) * 16.0,
                vx: 46.0 + frac * 30.0,
                vy: -8.0,
                life: frac * 1.4,
                max_life: 1.4,
                seed: i as f32 * 1.37,
            });
        }

        let mut cloth = Self {
            rest,
            pos,
            prev,
            pin,
            reach,
            constraints,
            wisps,
        };
        for k in 0..24 {
            cloth.step(1.0 / 60.0, k as f32 * (1.0 / 60.0), 1.0, 0.0, 0.0, &[]);
        }
        cloth
    }
}

impl CapeCloth {
    pub fn step(
        &mut self,
        dt: f32,
        t: f32,
        wind_scale: f32,
        gust_x: f32,
        gust_y: f32,
        obstacles_ref: &[[f32; 4]],
    ) {
        let clamped_dt = dt.clamp(0.001, 0.045);
        let sub_dt = clamped_dt * 0.25;
        let sub_dt2 = sub_dt * sub_dt;
        let mut sim_t = t;

        for _ in 0..4 {
            let gust = wind_gust(sim_t, wind_scale);
            for i in 0..VERT_COUNT {
                if self.pin[i] {
                    self.pos[i] = self.rest[i];
                    self.prev[i] = self.rest[i];
                    continue;
                }
                let reach = self.reach[i];
                let row_f = (i / NX) as f32 / (NY - 1) as f32;
                let wave = (sim_t * 6.5 - reach * 5.2 - row_f * 1.2).sin();
                let push_x = 290.0 * (gust - 0.85) * reach + 195.0 * reach * wave + gust_x * reach * 1.75;
                let push_y = -170.0 * reach * (sim_t * 6.5 - reach * 5.2).cos()
                    + 46.0 * (1.0 - reach * 0.5)
                    + gust_y * reach * 1.5;

                let rx = self.rest[i][0];
                let ry = self.rest[i][1];
                let px = self.pos[i][0];
                let py = self.pos[i][1];
                let vx = (px - self.prev[i][0]) * 0.952;
                let vy = (py - self.prev[i][1]) * 0.952;

                let mut ax = 5.4 * (rx - px) + push_x;
                let mut ay = 5.4 * (ry - py) + push_y;

                for obs in obstacles_ref {
                    let dx = px - obs[0];
                    let dy = py - obs[1];
                    let rad = obs[2].max(16.0);
                    let d2 = dx * dx + dy * dy;
                    if d2 < rad * rad && d2 > 1e-3 {
                        let d = d2.sqrt();
                        let f = (1.0 - d / rad) * 960.0 * reach;
                        ax += (dx / d) * f;
                        ay += (dy / d) * f;
                    }
                }

                self.prev[i] = [px, py];
                let nx = (px + vx + ax * sub_dt2).clamp(CAPE_MIN_X - 32.0, REF_W as f32 - 8.0);
                let ny = (py + vy + ay * sub_dt2).clamp(CAPE_TOP_Y - 76.0, REF_H as f32 - 20.0);
                self.pos[i] = [nx, ny];
            }

            for _ in 0..6 {
                for c in &self.constraints {
                    let pa = self.pos[c.a];
                    let pb = self.pos[c.b];
                    let dx = pb[0] - pa[0];
                    let dy = pb[1] - pa[1];
                    let d = (dx * dx + dy * dy).sqrt().max(1e-4);
                    let corr = (d - c.rest_len) / d * 0.5;
                    let cx = dx * corr;
                    let cy = dy * corr;
                    match (self.pin[c.a], self.pin[c.b]) {
                        (true, true) => {}
                        (true, false) => {
                            self.pos[c.b][0] -= cx * 2.0;
                            self.pos[c.b][1] -= cy * 2.0;
                        }
                        (false, true) => {
                            self.pos[c.a][0] += cx * 2.0;
                            self.pos[c.a][1] += cy * 2.0;
                        }
                        (false, false) => {
                            self.pos[c.a][0] += cx;
                            self.pos[c.a][1] += cy;
                            self.pos[c.b][0] -= cx;
                            self.pos[c.b][1] -= cy;
                        }
                    }
                }
            }
            sim_t += sub_dt;
        }

        let g_now = wind_gust(t, wind_scale);
        for (wi, w) in self.wisps.iter_mut().enumerate() {
            w.life += clamped_dt;
            if w.life >= w.max_life {
                w.life = 0.0;
                let hr = (wi * 7 + ((t * 13.0) as usize)) % NY;
                let p = self.pos[hr * NX + (NX - 1)];
                w.x = p[0];
                w.y = p[1];
                w.vx = (40.0 + 32.0 * g_now + ((w.seed * 3.1).sin() * 12.0)) + gust_x * 0.14;
                w.vy = ((w.seed + t * 2.0).sin() * 15.0 - 6.0) + gust_y * 0.12;
            } else {
                w.x += w.vx * clamped_dt;
                w.y += (w.vy + (t * 3.8 + w.seed).sin() * 10.0) * clamped_dt;
                for obs in obstacles_ref {
                    let dx = w.x - obs[0];
                    let dy = w.y - obs[1];
                    let r = obs[2] * 0.9;
                    let d2 = dx * dx + dy * dy;
                    if d2 < r * r && d2 > 1e-3 {
                        let d = d2.sqrt();
                        w.x += (dx / d) * (r - d) * 0.35;
                        w.y += (dy / d) * (r - d) * 0.35;
                    }
                }
            }
        }
    }

    /// Computes smooth per-vertex fold luminance and barycentrically interpolates
    /// across deformed cloth triangles for seamless silk-like ASCII folds.
    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_to_cells(
        &self,
        cols: usize,
        rows: usize,
        ox: f32,
        oy: f32,
        sx: f32,
        sy: f32,
        t: f32,
        iris_cx: f32,
        iris_cy: f32,
        iris_r2: f32,
        out_lum: &mut [f32],
        out_alpha: &mut [f32],
    ) {
        let len = cols * rows;
        out_lum[..len].fill(0.0);
        out_alpha[..len].fill(0.0);

        let mut vert_lum = [0.2f32; VERT_COUNT];
        for r in 0..NY {
            let v = r as f32 / (NY - 1) as f32;
            for c in 0..NX {
                let u = c as f32 / (NX - 1) as f32;
                let idx = r * NX + c;
                let p = self.pos[idx];
                let pr = self.rest[idx];

                let c_next = (c + 1).min(NX - 1);
                let c_prev = c.saturating_sub(1);
                let r_next = (r + 1).min(NY - 1);
                let r_prev = r.saturating_sub(1);

                let dhx = self.pos[r * NX + c_next][0] - self.pos[r * NX + c_prev][0];
                let dhy = self.pos[r * NX + c_next][1] - self.pos[r * NX + c_prev][1];
                let rdhx = self.rest[r * NX + c_next][0] - self.rest[r * NX + c_prev][0];
                let rdhy = self.rest[r * NX + c_next][1] - self.rest[r * NX + c_prev][1];
                let compress = (1.0
                    - (dhx * dhx + dhy * dhy).sqrt()
                        / (rdhx * rdhx + rdhy * rdhy).sqrt().max(1.0))
                .clamp(-0.6, 0.8);

                let dvy = self.pos[r_next * NX + c][1] - self.pos[r_prev * NX + c][1];
                let rdvy = (self.rest[r_next * NX + c][1] - self.rest[r_prev * NX + c][1]).abs().max(1.0);
                let v_slope = ((dvy - rdvy) / rdvy).clamp(-0.7, 0.7);

                let disp = ((p[0] - pr[0]) * 0.008 - (p[1] - pr[1]) * 0.006).clamp(-0.22, 0.28);
                let wave_phase = t * 6.5 - u * 6.4 - v * 2.4;
                let crease = (0.5 + 0.5 * wave_phase.cos()).powf(1.45);
                let rim_sheen = (1.0 - v).powf(2.1) * 0.24 + u * 0.08;
                vert_lum[idx] = (0.15 + 0.44 * crease + 0.24 * compress - 0.14 * v_slope + rim_sheen + disp)
                    .clamp(0.11, 0.86);
            }
        }

        for r in 0..(NY - 1) {
            let v0 = r as f32 / (NY - 1) as f32;
            for c in 0..(NX - 1) {
                let u1 = (c + 1) as f32 / (NX - 1) as f32;

                let i00 = r * NX + c;
                let i10 = r * NX + (c + 1);
                let i01 = (r + 1) * NX + c;
                let i11 = (r + 1) * NX + (c + 1);

                let to_cell = |p: [f32; 2]| -> (f32, f32) {
                    ((p[0] - ox) / sx - 0.5, (p[1] - oy) / sy - 0.5)
                };
                let c00 = to_cell(self.pos[i00]);
                let c10 = to_cell(self.pos[i10]);
                let c01 = to_cell(self.pos[i01]);
                let c11 = to_cell(self.pos[i11]);

                let l00 = vert_lum[i00];
                let l10 = vert_lum[i10];
                let l01 = vert_lum[i01];
                let l11 = vert_lum[i11];

                let min_cx = (c00.0.min(c10.0).min(c01.0).min(c11.0).floor() as i32 - 1).clamp(0, cols as i32 - 1);
                let max_cx = (c00.0.max(c10.0).max(c01.0).max(c11.0).ceil() as i32 + 1).clamp(0, cols as i32 - 1);
                let min_cy = (c00.1.min(c10.1).min(c01.1).min(c11.1).floor() as i32 - 1).clamp(0, rows as i32 - 1);
                let max_cy = (c00.1.max(c10.1).max(c01.1).max(c11.1).ceil() as i32 + 1).clamp(0, rows as i32 - 1);

                let edge_t = ((1.0 - (v0 - 0.45).abs() * 0.22) * (0.94 + 0.06 * u1)).clamp(0.76, 1.0);

                for cy_i in min_cy..=max_cy {
                    let cy_f = cy_i as f32;
                    let ref_y = oy + (cy_f + 0.5) * sy;
                    for cx_i in min_cx..=max_cx {
                        let cx_f = cx_i as f32;
                        let ref_x = ox + (cx_f + 0.5) * sx;
                        if iris_r2 > 0.0 {
                            let dx_i = ref_x - iris_cx;
                            let dy_i = ref_y - iris_cy;
                            if dx_i * dx_i + dy_i * dy_i > iris_r2 {
                                continue;
                            }
                        }
                        if let Some((w0, w1, w2)) = bary_tri(cx_f, cy_f, c00, c10, c11) {
                            let idx = cy_i as usize * cols + cx_i as usize;
                            out_lum[idx] = w0 * l00 + w1 * l10 + w2 * l11;
                            out_alpha[idx] = out_alpha[idx].max(edge_t);
                        } else if let Some((w0, w1, w2)) = bary_tri(cx_f, cy_f, c00, c11, c01) {
                            let idx = cy_i as usize * cols + cx_i as usize;
                            out_lum[idx] = w0 * l00 + w1 * l11 + w2 * l01;
                            out_alpha[idx] = out_alpha[idx].max(edge_t);
                        }
                    }
                }
            }
        }
    }
}

#[inline]
fn bary_tri(px: f32, py: f32, a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> Option<(f32, f32, f32)> {
    let v0x = b.0 - a.0;
    let v0y = b.1 - a.1;
    let v1x = c.0 - a.0;
    let v1y = c.1 - a.1;
    let v2x = px - a.0;
    let v2y = py - a.1;
    let den = v0x * v1y - v1x * v0y;
    if den.abs() < 1e-4 {
        return None;
    }
    let inv = 1.0 / den;
    let v = (v2x * v1y - v1x * v2y) * inv;
    let w = (v0x * v2y - v2x * v0y) * inv;
    let u = 1.0 - v - w;
    if u >= -0.06 && v >= -0.06 && w >= -0.06 {
        let su = u.max(0.0);
        let sv = v.max(0.0);
        let sw = w.max(0.0);
        let sum = (su + sv + sw).max(1e-5);
        Some((su / sum, sv / sum, sw / sum))
    } else {
        None
    }
}
