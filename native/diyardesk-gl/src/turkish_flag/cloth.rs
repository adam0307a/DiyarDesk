//! Verlet + spring-constraint 3D cloth simulation for the Turkish Flag.
//!
//! Coordinate system (right-handed, Y-up, X-right, Z-toward-viewer):
//! - Hoist edge (`ix == 0`) is pinned vertically along `x = POLE_X`, `z = 0.0`
//!   between `TOP_Y` and `BOT_Y`.
//! - Flag body stretches rightward along `+X` in wind with multi-harmonic
//!   traveling pressure waves, aerodynamic normal lift/drag, gravity sag at low
//!   wind, and pointer/click gusts.
//! - Mesh triangles are sorted back-to-front by view-space/world Z so
//!   self-overlapping folds render cleanly without requiring a depth buffer.

use std::cmp::Ordering;

pub const NX: usize = 36;
pub const NY: usize = 24;
pub const NODE_COUNT: usize = NX * NY;
pub const CELL_COUNT: usize = (NX - 1) * (NY - 1);
pub const TRI_COUNT: usize = CELL_COUNT * 2;
pub const VERTEX_FLOATS: usize = 8; // pos(3) + normal(3) + uv(2)

pub const POLE_X: f32 = -0.84;
pub const TOP_Y: f32 = 0.72;
pub const FLAG_H: f32 = 1.24;
pub const BOT_Y: f32 = TOP_Y - FLAG_H;
/// Total length = (1/30 + 1.5) * FLAG_H = 1.533333 * FLAG_H (uçkurluk + 2:3 red body).
pub const FLAG_W: f32 = FLAG_H * (1.5 + 1.0 / 30.0);

#[derive(Clone, Copy)]
struct Node {
    pos: [f32; 3],
    prev: [f32; 3],
    pinned: bool,
}

#[derive(Clone, Copy)]
struct Constraint {
    a: usize,
    b: usize,
    rest: f32,
    stiffness: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PointerInteraction {
    pub active: bool,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub dragging: bool,
    pub grab_node: Option<usize>,
    pub ripple_pos: [f32; 2],
    pub ripple_age: f32,
}

impl Default for PointerInteraction {
    fn default() -> Self {
        Self {
            active: false,
            pos: [0.0, 0.0],
            vel: [0.0, 0.0],
            dragging: false,
            grab_node: None,
            ripple_pos: [0.0, 0.0],
            ripple_age: -1.0,
        }
    }
}

pub struct FlagCloth {
    nodes: Vec<Node>,
    normals: Vec<[f32; 3]>,
    constraints: Vec<Constraint>,
    tri_order: Vec<(f32, usize, bool)>,
}

impl Default for FlagCloth {
    fn default() -> Self {
        Self::new()
    }
}

impl FlagCloth {
    pub fn new() -> Self {
        let dx_rest = FLAG_W / (NX - 1) as f32;
        let dy_rest = FLAG_H / (NY - 1) as f32;

        let mut nodes = Vec::with_capacity(NODE_COUNT);
        for iy in 0..NY {
            let v = iy as f32 / (NY - 1) as f32;
            let y = TOP_Y - v * FLAG_H;
            for ix in 0..NX {
                let u = ix as f32 / (NX - 1) as f32;
                let x = POLE_X + u * FLAG_W;
                // Gentle initial wave so even frame 0 has natural curvature
                let z = (u * 5.2).sin() * 0.045 * u;
                let pinned = ix == 0;
                nodes.push(Node {
                    pos: [x, y, z],
                    prev: [x, y, z],
                    pinned,
                });
            }
        }

        let mut constraints = Vec::with_capacity(NODE_COUNT * 6);
        let idx = |ix: usize, iy: usize| iy * NX + ix;

        // Structural constraints (horizontal & vertical)
        for iy in 0..NY {
            for ix in 0..NX {
                if ix + 1 < NX {
                    constraints.push(Constraint {
                        a: idx(ix, iy),
                        b: idx(ix + 1, iy),
                        rest: dx_rest,
                        stiffness: 0.96,
                    });
                }
                if iy + 1 < NY {
                    constraints.push(Constraint {
                        a: idx(ix, iy),
                        b: idx(ix, iy + 1),
                        rest: dy_rest,
                        stiffness: 0.96,
                    });
                }
            }
        }

        // Shear constraints (diagonals)
        let diag_rest = (dx_rest * dx_rest + dy_rest * dy_rest).sqrt();
        for iy in 0..(NY - 1) {
            for ix in 0..(NX - 1) {
                constraints.push(Constraint {
                    a: idx(ix, iy),
                    b: idx(ix + 1, iy + 1),
                    rest: diag_rest,
                    stiffness: 0.72,
                });
                constraints.push(Constraint {
                    a: idx(ix + 1, iy),
                    b: idx(ix, iy + 1),
                    rest: diag_rest,
                    stiffness: 0.72,
                });
            }
        }

        // Bending constraints (2-step skip) for smooth fabric folds
        for iy in 0..NY {
            for ix in 0..NX {
                if ix + 2 < NX {
                    constraints.push(Constraint {
                        a: idx(ix, iy),
                        b: idx(ix + 2, iy),
                        rest: dx_rest * 2.0,
                        stiffness: 0.36,
                    });
                }
                if iy + 2 < NY {
                    constraints.push(Constraint {
                        a: idx(ix, iy),
                        b: idx(ix, iy + 2),
                        rest: dy_rest * 2.0,
                        stiffness: 0.40,
                    });
                }
            }
        }

        let mut tri_order = Vec::with_capacity(TRI_COUNT);
        for iy in 0..(NY - 1) {
            for ix in 0..(NX - 1) {
                let cell = iy * (NX - 1) + ix;
                tri_order.push((0.0, cell, false));
                tri_order.push((0.0, cell, true));
            }
        }

        let mut cloth = Self {
            nodes,
            normals: vec![[0.0, 0.0, 1.0]; NODE_COUNT],
            constraints,
            tri_order,
        };

        // Pre-warm so the flag starts gracefully unfurled and billowing
        let idle_ptr = PointerInteraction::default();
        let mut t = 0.0f32;
        for _ in 0..72 {
            cloth.step(1.0 / 45.0, t, 1.0, 1.0, [0.0; 3], &idle_ptr, false);
            t += 1.0 / 45.0;
        }
        cloth.compute_normals();
        cloth
    }

    #[cfg(test)]
    pub fn node_pos(&self, ix: usize, iy: usize) -> [f32; 3] {
        self.nodes[iy * NX + ix].pos
    }

    /// Finds the nearest non-hoist cloth node (`ix >= 2`) within `max_dist` in XY.
    pub fn find_nearest_free_node(&self, wx: f32, wy: f32, max_dist: f32) -> Option<usize> {
        let mut best_idx = None;
        let mut best_d2 = max_dist * max_dist;
        for iy in 0..NY {
            for ix in 2..NX {
                let idx = iy * NX + ix;
                let p = self.nodes[idx].pos;
                let dx = p[0] - wx;
                let dy = p[1] - wy;
                let d2 = dx * dx + dy * dy;
                if d2 <= best_d2 {
                    best_d2 = d2;
                    best_idx = Some(idx);
                }
            }
        }
        best_idx
    }

    pub fn step(
        &mut self,
        dt_raw: f32,
        time: f32,
        wind: f32,
        speed: f32,
        gust_impulse: [f32; 3],
        pointer: &PointerInteraction,
        low_mode: bool,
    ) {
        let dt = dt_raw.clamp(0.001, 0.036) * speed.clamp(0.25, 2.2);
        let sub_steps = if low_mode { 1 } else { 2 };
        let sub_dt = dt / sub_steps as f32;
        let sub_dt2 = sub_dt * sub_dt;
        let relax_iters = if low_mode { 3 } else { 5 };
        let wind_k = wind.clamp(0.15, 2.4);

        let grab_coord = pointer.grab_node.and_then(|idx| {
            if idx < NODE_COUNT && (idx % NX) >= 2 {
                Some((idx, idx % NX, idx / NX))
            } else {
                None
            }
        });

        for sub in 0..sub_steps {
            let t = time + sub as f32 * sub_dt;
            self.compute_normals();

            // Natural wind envelope + primary/secondary gusts
            let gust_wave = 1.0
                + 0.24 * (t * 0.95).sin()
                + 0.14 * (t * 2.15 + 1.3).sin()
                + 0.08 * (t * 3.7 - 0.7).cos();
            let base_wind_x = (7.2 * wind_k * gust_wave + gust_impulse[0]).max(0.6);
            let gravity_y = -4.2;

            let damping = 0.986;
            let ptr_speed = (pointer.vel[0] * pointer.vel[0] + pointer.vel[1] * pointer.vel[1])
                .sqrt()
                .min(6.0);

            for iy in 0..NY {
                let v = iy as f32 / (NY - 1) as f32;
                for ix in 0..NX {
                    let i = iy * NX + ix;
                    if self.nodes[i].pinned {
                        continue;
                    }
                    let u = ix as f32 / (NX - 1) as f32;
                    let p = self.nodes[i].pos;
                    let prev = self.nodes[i].prev;
                    let vx = (p[0] - prev[0]).clamp(-0.22, 0.22) * damping;
                    let vy = (p[1] - prev[1]).clamp(-0.22, 0.22) * damping;
                    let vz = (p[2] - prev[2]).clamp(-0.22, 0.22) * damping;

                    let n = self.normals[i];

                    // Traveling pressure ripples from hoist to fly edge
                    let travel_1 = (u * 5.6 - t * 4.1 * (0.65 + 0.45 * wind_k) + v * 1.1).sin();
                    let travel_2 = (u * 10.2 - t * 7.2 * (0.65 + 0.45 * wind_k) - v * 1.9).sin();
                    let flutter = (u * 16.5 - t * 11.4 + v * 3.1).cos();
                    let flutter_amp = 0.20 + 0.80 * u * u;

                    let wind_y = (1.25 * wind_k * (t * 1.4 + u * 3.0).sin()
                        + 0.70 * (1.0 - v) * wind_k)
                        + gust_impulse[1];
                    let wind_z = wind_k
                        * flutter_amp
                        * (2.45 * travel_1 + 1.15 * travel_2 + 0.46 * flutter)
                        + gust_impulse[2] * (0.4 + 0.6 * u);

                    // Relative wind vs cloth velocity projected onto surface normal
                    let rel_x = base_wind_x - vx * 6.0;
                    let rel_y = wind_y - vy * 6.0;
                    let rel_z = wind_z - vz * 6.0;
                    let normal_dot = rel_x * n[0] + rel_y * n[1] + rel_z * n[2];
                    let aero_scale = 0.44;

                    let mut fx = base_wind_x * 0.52 + n[0] * normal_dot * aero_scale;
                    let mut fy = gravity_y + wind_y * 0.35 + n[1] * normal_dot * aero_scale;
                    let mut fz = wind_z * 0.62 + n[2] * normal_dot * aero_scale;

                    // Subtle lift on upper fly corner so flag holds a proud silhouette
                    fy += (1.0 - v) * u * 1.15 * wind_k.min(1.5);

                    // 1. Interactive pointer wake / hover breeze
                    if pointer.active {
                        let dx = p[0] - pointer.pos[0];
                        let dy = p[1] - pointer.pos[1];
                        let d2 = dx * dx + dy * dy;
                        let r2 = 0.56 * 0.56;
                        if d2 < r2 {
                            let w = (1.0 - d2 / r2).powi(2);
                            fx += pointer.vel[0] * 16.0 * w;
                            fy += pointer.vel[1] * 16.0 * w;
                            fz += (14.0 + ptr_speed * 9.0)
                                * w
                                * (0.55 + 0.45 * (t * 5.2 + u * 7.0).sin());
                        }
                    }

                    // 2. Click expanding radial shockwave ripple
                    if pointer.ripple_age >= 0.0 && pointer.ripple_age < 0.95 {
                        let dx_r = p[0] - pointer.ripple_pos[0];
                        let dy_r = p[1] - pointer.ripple_pos[1];
                        let d = (dx_r * dx_r + dy_r * dy_r).sqrt();
                        let ring_r = 0.12 + pointer.ripple_age * 2.15;
                        let band = 0.24;
                        let fade = (1.0 - pointer.ripple_age / 0.95).powi(2);
                        let band_d = (d - ring_r) / band;
                        let ring = (-band_d * band_d).exp() * fade;
                        if ring > 1e-4 {
                            let inv = 1.0 / (d + 0.08);
                            fx += dx_r * inv * 6.5 * ring;
                            fy += dy_r * inv * 6.5 * ring;
                            fz += 38.0 * ring * ((d - ring_r) * 7.0).cos();
                        }
                    }

                    // 3. Soft fabric grab & drag spring pull
                    if pointer.dragging {
                        if let Some((_, g_ix, g_iy)) = grab_coord {
                            if ix >= 2 {
                                let dix = ix as isize - g_ix as isize;
                                let diy = iy as isize - g_iy as isize;
                                if dix.abs() <= 3 && diy.abs() <= 3 {
                                    let gd2 = (dix * dix + diy * diy) as f32;
                                    let pull_w = (-gd2 * 0.28).exp();
                                    fx += (pointer.pos[0] - p[0]) * 68.0 * pull_w;
                                    fy += (pointer.pos[1] - p[1]) * 68.0 * pull_w;
                                    fz += (0.22 - p[2]) * 26.0 * pull_w;
                                }
                            }
                        }
                    }

                    let nx_pos = p[0] + vx + fx * sub_dt2;
                    let ny_pos = p[1] + vy + fy * sub_dt2;
                    let nz_pos = p[2] + vz + fz * sub_dt2;

                    self.nodes[i].prev = p;
                    self.nodes[i].pos = [nx_pos, ny_pos, nz_pos];
                }
            }

            // Distance constraint relaxation
            for _ in 0..relax_iters {
                for c in &self.constraints {
                    let na = self.nodes[c.a];
                    let nb = self.nodes[c.b];
                    let dx = nb.pos[0] - na.pos[0];
                    let dy = nb.pos[1] - na.pos[1];
                    let dz = nb.pos[2] - na.pos[2];
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    if dist < 1e-6 {
                        continue;
                    }
                    let diff = (dist - c.rest) / dist * c.stiffness;
                    match (na.pinned, nb.pinned) {
                        (true, true) => {}
                        (true, false) => {
                            self.nodes[c.b].pos[0] -= dx * diff;
                            self.nodes[c.b].pos[1] -= dy * diff;
                            self.nodes[c.b].pos[2] -= dz * diff;
                        }
                        (false, true) => {
                            self.nodes[c.a].pos[0] += dx * diff;
                            self.nodes[c.a].pos[1] += dy * diff;
                            self.nodes[c.a].pos[2] += dz * diff;
                        }
                        (false, false) => {
                            let h = 0.5 * diff;
                            self.nodes[c.a].pos[0] += dx * h;
                            self.nodes[c.a].pos[1] += dy * h;
                            self.nodes[c.a].pos[2] += dz * h;
                            self.nodes[c.b].pos[0] -= dx * h;
                            self.nodes[c.b].pos[1] -= dy * h;
                            self.nodes[c.b].pos[2] -= dz * h;
                        }
                    }
                }

                if pointer.dragging {
                    if let Some((g_idx, _, _)) = grab_coord {
                        let tx = pointer.pos[0].max(POLE_X + 0.06);
                        let ty = pointer.pos[1];
                        let cur = self.nodes[g_idx].pos;
                        self.nodes[g_idx].pos[0] = cur[0] + (tx - cur[0]) * 0.35;
                        self.nodes[g_idx].pos[1] = cur[1] + (ty - cur[1]) * 0.35;
                        self.nodes[g_idx].pos[2] = cur[2] + (0.18 - cur[2]) * 0.25;
                    }
                }

                // Prevent cloth from crossing to the left of the flagpole
                for node in &mut self.nodes {
                    if !node.pinned {
                        if node.pos[0] < POLE_X + 0.008 {
                            node.pos[0] = POLE_X + 0.008;
                        }
                        node.pos[2] = node.pos[2].clamp(-0.85, 0.85);
                    }
                }
            }
        }
        self.compute_normals();
    }

    pub fn compute_normals(&mut self) {
        for n in &mut self.normals {
            *n = [0.0, 0.0, 0.0];
        }
        for iy in 0..(NY - 1) {
            for ix in 0..(NX - 1) {
                let i00 = iy * NX + ix;
                let i10 = i00 + 1;
                let i01 = (iy + 1) * NX + ix;
                let i11 = i01 + 1;

                let p00 = self.nodes[i00].pos;
                let p10 = self.nodes[i10].pos;
                let p01 = self.nodes[i01].pos;
                let p11 = self.nodes[i11].pos;

                // Tri 1: (p00, p01, p10) -> outward +Z when flat
                let e1 = [p01[0] - p00[0], p01[1] - p00[1], p01[2] - p00[2]];
                let e2 = [p10[0] - p00[0], p10[1] - p00[1], p10[2] - p00[2]];
                let n1 = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                for &k in &[i00, i01, i10] {
                    self.normals[k][0] += n1[0];
                    self.normals[k][1] += n1[1];
                    self.normals[k][2] += n1[2];
                }

                // Tri 2: (p10, p01, p11)
                let f1 = [p01[0] - p10[0], p01[1] - p10[1], p01[2] - p10[2]];
                let f2 = [p11[0] - p10[0], p11[1] - p10[1], p11[2] - p10[2]];
                let n2 = [
                    f1[1] * f2[2] - f1[2] * f2[1],
                    f1[2] * f2[0] - f1[0] * f2[2],
                    f1[0] * f2[1] - f1[1] * f2[0],
                ];
                for &k in &[i10, i01, i11] {
                    self.normals[k][0] += n2[0];
                    self.normals[k][1] += n2[1];
                    self.normals[k][2] += n2[2];
                }
            }
        }
        for n in &mut self.normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 1e-6 {
                n[0] /= len;
                n[1] /= len;
                n[2] /= len;
            } else {
                *n = [0.0, 0.0, 1.0];
            }
        }
    }

    /// Emits non-indexed triangles sorted back-to-front relative to `cam_pos`
    /// so self-overlapping waves compose cleanly without relying on a host depth buffer.
    pub fn build_mesh_vertices(&mut self, cam_pos: [f32; 3], out: &mut Vec<f32>) {
        out.clear();
        let needed = TRI_COUNT * 3 * VERTEX_FLOATS;
        if out.capacity() < needed {
            out.reserve(needed - out.capacity());
        }

        let mut t_idx = 0usize;
        for iy in 0..(NY - 1) {
            for ix in 0..(NX - 1) {
                let i00 = iy * NX + ix;
                let i10 = i00 + 1;
                let i01 = (iy + 1) * NX + ix;
                let i11 = i01 + 1;

                let p00 = self.nodes[i00].pos;
                let p10 = self.nodes[i10].pos;
                let p01 = self.nodes[i01].pos;
                let p11 = self.nodes[i11].pos;

                let c1 = [
                    (p00[0] + p01[0] + p10[0]) / 3.0 - cam_pos[0],
                    (p00[1] + p01[1] + p10[1]) / 3.0 - cam_pos[1],
                    (p00[2] + p01[2] + p10[2]) / 3.0 - cam_pos[2],
                ];
                let d1 = c1[0] * c1[0] + c1[1] * c1[1] + c1[2] * c1[2];

                let c2 = [
                    (p10[0] + p01[0] + p11[0]) / 3.0 - cam_pos[0],
                    (p10[1] + p01[1] + p11[1]) / 3.0 - cam_pos[1],
                    (p10[2] + p01[2] + p11[2]) / 3.0 - cam_pos[2],
                ];
                let d2 = c2[0] * c2[0] + c2[1] * c2[1] + c2[2] * c2[2];

                let cell = iy * (NX - 1) + ix;
                self.tri_order[t_idx] = (d1, cell, false);
                self.tri_order[t_idx + 1] = (d2, cell, true);
                t_idx += 2;
            }
        }

        // Back-to-front: largest squared distance to camera first
        self.tri_order
            .sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

        let inv_nx = 1.0 / (NX - 1) as f32;
        let inv_ny = 1.0 / (NY - 1) as f32;

        let mut push_vert = |ix: usize, iy: usize| {
            let idx = iy * NX + ix;
            let p = self.nodes[idx].pos;
            let n = self.normals[idx];
            out.extend_from_slice(&[
                p[0],
                p[1],
                p[2],
                n[0],
                n[1],
                n[2],
                ix as f32 * inv_nx,
                iy as f32 * inv_ny,
            ]);
        };

        for &(_, cell, second) in &self.tri_order {
            let iy = cell / (NX - 1);
            let ix = cell % (NX - 1);
            if !second {
                push_vert(ix, iy);
                push_vert(ix, iy + 1);
                push_vert(ix + 1, iy);
            } else {
                push_vert(ix + 1, iy);
                push_vert(ix, iy + 1);
                push_vert(ix + 1, iy + 1);
            }
        }
    }
}
