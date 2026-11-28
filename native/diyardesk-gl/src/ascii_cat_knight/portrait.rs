//! High-detail procedural 640×820 reference portrait, sculpted steel armor,
//! diagonal leather strap, snug upright greatsword, 3/4 Scottish Fold kitten
//! head, glossy dark eyes, and foreground grass for `ascii-cat-knight`.
//!
//! Precomputed once into a static `OnceLock<KnightPortrait>` with zero external
//! image assets so startup is fast and background frames stay silky smooth.

use std::sync::OnceLock;

pub const REF_W: usize = 640;
pub const REF_H: usize = 820;

pub const HEAD_CX: f32 = 290.0;
pub const HEAD_CY: f32 = 368.0;

pub const LEFT_EYE_X: f32 = 244.0;
pub const LEFT_EYE_Y: f32 = 377.0;
pub const RIGHT_EYE_X: f32 = 304.0;
pub const RIGHT_EYE_Y: f32 = 376.0;
pub const EYE_RX: f32 = 17.5;
pub const EYE_RY: f32 = 16.0;

pub const SWORD_X: f32 = 186.0;
pub const SWORD_TOP_Y: f32 = 76.0;
pub const SWORD_GUARD_Y: f32 = 486.0;
pub const SWORD_POMMEL_Y: f32 = 548.0;

pub struct KnightPortrait {
    pub rgb_open: Vec<[f32; 3]>,
    pub rgb_closed: Vec<[f32; 3]>,
    pub body_mask: Vec<f32>,
    pub fur_weight: Vec<f32>,
    pub sword_mask: Vec<f32>,
    pub eye_mask: Vec<f32>,
    pub grass_rgb: Vec<[f32; 3]>,
    pub grass_mask: Vec<f32>,
    pub grass_radius: Vec<f32>,
}

static PORTRAIT: OnceLock<KnightPortrait> = OnceLock::new();

pub fn portrait() -> &'static KnightPortrait {
    PORTRAIT.get_or_init(build_portrait)
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let denom = edge1 - edge0;
    let t = if denom.abs() < 1e-6 {
        if x >= edge1 { 1.0 } else { 0.0 }
    } else {
        ((x - edge0) / denom).clamp(0.0, 1.0)
    };
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let k = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * k,
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
    ]
}

#[inline]
fn scale3(c: [f32; 3], s: f32) -> [f32; 3] {
    [
        (c[0] * s).clamp(0.0, 1.0),
        (c[1] * s).clamp(0.0, 1.0),
        (c[2] * s).clamp(0.0, 1.0),
    ]
}

#[inline]
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        (a[0] + b[0]).clamp(0.0, 1.0),
        (a[1] + b[1]).clamp(0.0, 1.0),
        (a[2] + b[2]).clamp(0.0, 1.0),
    ]
}

#[inline]
fn hash2(x: f32, y: f32) -> f32 {
    let h = (x * 127.1 + y * 311.7).sin() * 43758.547;
    h - h.floor()
}

fn value_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash2(ix, iy);
    let b = hash2(ix + 1.0, iy);
    let c = hash2(ix, iy + 1.0);
    let d = hash2(ix + 1.0, iy + 1.0);
    a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy
}

fn fbm(x: f32, y: f32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut fx = x;
    let mut fy = y;
    for _ in 0..4 {
        sum += amp * value_noise(fx, fy);
        fx *= 2.07;
        fy *= 2.03;
        amp *= 0.5;
    }
    sum
}

#[inline]
fn sd_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let pax = px - ax;
    let pay = py - ay;
    let bax = bx - ax;
    let bay = by - ay;
    let denom = (bax * bax + bay * bay).max(1e-6);
    let h = ((pax * bax + pay * bay) / denom).clamp(0.0, 1.0);
    let dx = pax - bax * h;
    let dy = pay - bay * h;
    (dx * dx + dy * dy).sqrt()
}

#[inline]
fn sd_capsule(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, ra: f32, rb: f32) -> f32 {
    let pax = px - ax;
    let pay = py - ay;
    let bax = bx - ax;
    let bay = by - ay;
    let denom = (bax * bax + bay * bay).max(1e-6);
    let h = ((pax * bax + pay * bay) / denom).clamp(0.0, 1.0);
    let r = ra + (rb - ra) * h;
    let dx = pax - bax * h;
    let dy = pay - bay * h;
    (dx * dx + dy * dy).sqrt() - r
}

#[inline]
fn sd_round_box(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, rad: f32) -> f32 {
    let qx = (px - cx).abs() - (hw - rad).max(0.0);
    let qy = (py - cy).abs() - (hh - rad).max(0.0);
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - rad
}

#[inline]
fn sd_ellipse(px: f32, py: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> f32 {
    let nx = (px - cx) / rx.max(1e-4);
    let ny = (py - cy) / ry.max(1e-4);
    let r = (nx * nx + ny * ny).sqrt();
    (r - 1.0) * rx.min(ry)
}

#[inline]
fn sd_triangle(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32) -> f32 {
    let e0x = bx - ax;
    let e0y = by - ay;
    let e1x = cx - bx;
    let e1y = by - by;
    let _ = e1y;
    let e1y = cy - by;
    let e2x = ax - cx;
    let e2y = ay - cy;

    let v0x = px - ax;
    let v0y = py - ay;
    let v1x = px - bx;
    let v1y = py - by;
    let v2x = px - cx;
    let v2y = py - cy;

    let pq0 = ((v0x * e0x + v0y * e0y) / (e0x * e0x + e0y * e0y).max(1e-6)).clamp(0.0, 1.0);
    let pq1 = ((v1x * e1x + v1y * e1y) / (e1x * e1x + e1y * e1y).max(1e-6)).clamp(0.0, 1.0);
    let pq2 = ((v2x * e2x + v2y * e2y) / (e2x * e2x + e2y * e2y).max(1e-6)).clamp(0.0, 1.0);

    let d0x = v0x - e0x * pq0;
    let d0y = v0y - e0y * pq0;
    let d1x = v1x - e1x * pq1;
    let d1y = v1y - e1y * pq1;
    let d2x = v2x - e2x * pq2;
    let d2y = v2y - e2y * pq2;

    let s = (e0x * e2y - e0y * e2x).signum();
    let dd0 = d0x * d0x + d0y * d0y;
    let dd1 = d1x * d1x + d1y * d1y;
    let dd2 = d2x * d2x + d2y * d2y;
    let sd0 = s * (v0x * e0y - v0y * e0x);
    let sd1 = s * (v1x * e1y - v1y * e1x);
    let sd2 = s * (v2x * e2y - v2y * e2x);

    let min_d = dd0.min(dd1).min(dd2).sqrt();
    let min_s = sd0.min(sd1).min(sd2);
    if min_s > 0.0 { -min_d } else { min_d }
}

fn eval_steel_shade(nx: f32, ny: f32, fine_n: f32, ridge: f32) -> [f32; 3] {
    // High-contrast studio steel: upper-left specular highlight + deep dark crevices
    let lx = -0.56f32;
    let ly = -0.68f32;
    let lz = 0.47f32;
    let nz = (1.0 - (nx * nx + ny * ny).min(0.90)).sqrt();
    let ndl = (nx * lx + ny * ly + nz * lz).max(0.0);
    let spec = ndl.powf(5.5) * 0.56 + ridge * 0.36;
    let rim = (1.0 - nz).powf(2.4) * 0.16;
    let lum = (0.09 + 0.58 * ndl + spec + rim + (fine_n - 0.5) * 0.08).clamp(0.06, 0.99);
    let cool = [lum * 0.94, lum * 0.97, lum * 1.01];
    let warm = [lum * 1.03, lum * 0.97, lum * 0.88];
    mix3(cool, warm, (0.35 + (1.0 - ndl) * 0.4).clamp(0.0, 1.0))
}

fn build_portrait() -> KnightPortrait {
    let total = REF_W * REF_H;
    let mut rgb_open = vec![[0.0f32; 3]; total];
    let mut rgb_closed = vec![[0.0f32; 3]; total];
    let mut body_mask = vec![0.0f32; total];
    let mut fur_weight = vec![0.0f32; total];
    let mut sword_mask = vec![0.0f32; total];
    let mut eye_mask = vec![0.0f32; total];
    let mut grass_rgb = vec![[0.0f32; 3]; total];
    let mut grass_mask = vec![0.0f32; total];
    let mut grass_radius = vec![0.0f32; total];

    // Warm golden-olive parchment tabby fur palette matching reference close-up
    let fur_cream = [0.92f32, 0.87, 0.77];
    let fur_warm = [0.76f32, 0.69, 0.54];
    let fur_taupe = [0.47f32, 0.43, 0.38];
    let fur_dark = [0.19f32, 0.17, 0.15];
    let nose_pink = [0.68f32, 0.46, 0.42];
    let strap_olive = [0.46f32, 0.39, 0.24];

    for y in 0..REF_H {
        let py = y as f32;
        for x in 0..REF_W {
            let px = x as f32;
            let idx = y * REF_W + x;

            // --- 1. Upright Vertical Greatsword (snug along left cheek & shoulder) ---
            let blade_hw = if py < 126.0 {
                let t = ((py - SWORD_TOP_Y) / 50.0).clamp(0.0, 1.0);
                1.5 + t * 9.8
            } else {
                11.3 - ((py - 126.0) / 360.0).clamp(0.0, 1.0) * 1.1
            };
            let d_blade = if py >= SWORD_TOP_Y - 2.0 && py <= SWORD_GUARD_Y + 2.0 {
                let dx = (px - SWORD_X).abs() - blade_hw;
                let dy = if py < SWORD_TOP_Y {
                    SWORD_TOP_Y - py
                } else if py > SWORD_GUARD_Y {
                    py - SWORD_GUARD_Y
                } else {
                    -14.0
                };
                dx.max(dy)
            } else {
                99.0
            };
            let guard_y_curve = SWORD_GUARD_Y + ((px - SWORD_X) * 0.042).powi(2) * 2.0;
            let d_guard = sd_round_box(px, py, SWORD_X, guard_y_curve, 36.0, 5.8, 3.2);
            let d_grip = sd_round_box(px, py, SWORD_X, 518.0, 5.8, 25.0, 2.2);
            let d_pommel = ((px - SWORD_X).powi(2) + (py - SWORD_POMMEL_Y).powi(2)).sqrt() - 9.8;
            let d_sword = d_blade.min(d_guard).min(d_grip).min(d_pommel);
            let a_sword = 1.0 - smoothstep(-1.5, 1.3, d_sword);

            let mut sword_col = [0.0f32; 3];
            let mut is_blade = 0.0f32;
            if a_sword > 0.01 {
                if d_blade < d_guard.min(d_grip).min(d_pommel) {
                    let rx = (px - SWORD_X) / blade_hw.max(1.0);
                    let fuller = (-((rx * 2.9).powi(2))).exp()
                        * smoothstep(132.0, 175.0, py)
                        * (1.0 - smoothstep(452.0, 482.0, py));
                    let left_bevel = smoothstep(-0.08, -0.88, rx);
                    let right_bevel = smoothstep(0.18, 0.90, rx);
                    let ridge_line = (-((rx.abs() - 0.16) * 5.2).powi(2)).exp() * 0.14;
                    let steel_grain = value_noise(px * 0.45, py * 0.07) * 0.06;
                    let base = 0.56 + left_bevel * 0.40 - right_bevel * 0.22 - fuller * 0.34 + ridge_line + steel_grain;
                    let lum = base.clamp(0.18, 0.98);
                    sword_col = [lum * 0.96, lum * 0.97, lum * 1.0];
                    is_blade = a_sword;
                } else if d_guard <= d_grip.min(d_pommel) {
                    let gx = (px - SWORD_X) / 36.0;
                    let gy = (py - guard_y_curve) / 5.8;
                    sword_col = eval_steel_shade(gx * 0.4, gy * 0.7, value_noise(px * 0.2, py * 0.2), (1.0 - gy.abs()).max(0.0) * 0.6);
                    is_blade = a_sword * 0.65;
                } else if d_pommel < d_grip {
                    let nx = (px - SWORD_X) / 9.8;
                    let ny = (py - SWORD_POMMEL_Y) / 9.8;
                    sword_col = eval_steel_shade(nx * 0.75, ny * 0.75, 0.5, 0.45);
                } else {
                    let wrap = ((py - 492.0) * 1.2 + (px - SWORD_X) * 0.2).sin() * 0.5 + 0.5;
                    let shade = 0.22 + 0.18 * wrap - ((px - SWORD_X) / 5.8) * 0.08;
                    sword_col = [shade * 1.08, shade * 0.92, shade * 0.78];
                }
            }

            // --- 2. High-Contrast Medieval Plate Armor + Diagonal Leather Cloak Strap ---
            let fine_metal = fbm(px * 0.065, py * 0.065);

            // Gorget collar & neck rim
            let d_gorget = sd_ellipse(px, py, 290.0, 448.0, 68.0, 26.0);
            // Layered pauldrons flaring smoothly past shoulders
            let d_pauld_l = sd_ellipse(px - (py - 486.0) * 0.16, py, 214.0, 486.0, 44.0, 40.0);
            let d_pauld_r = sd_ellipse(px + (py - 486.0) * 0.18, py, 366.0, 488.0, 48.0, 42.0);
            // Sculpted cuirass breastplate
            let waist_w = if py < 530.0 {
                60.0 - ((py - 455.0) / 75.0).clamp(0.0, 1.0) * 14.0
            } else {
                46.0 + ((py - 530.0) / 80.0).clamp(0.0, 1.0) * 18.0
            };
            let d_torso = sd_round_box(px, py, 292.0, 526.0, waist_w, 70.0, 22.0);
            let d_faulds = sd_capsule(px, py, 291.0, 558.0, 292.0, 608.0, 52.0, 64.0);
            // Left armored arm & gauntlet gripping sword hilt at SWORD_X=186
            let d_arm_l = sd_capsule(px, py, 206.0, 494.0, 194.0, 530.0, 18.0, 15.0);
            let d_hand_l = sd_round_box(px, py, 188.0, 526.0, 17.0, 14.5, 6.5);
            // Right armored arm & gauntlet resting on waist
            let d_arm_r = sd_capsule(px, py, 368.0, 504.0, 376.0, 574.0, 21.0, 15.5);
            let d_hand_r = sd_ellipse(px, py, 372.0, 584.0, 16.0, 14.0);
            // Armored thighs, poleyn knee guards, greaves & boots
            let d_leg_l = sd_capsule(px, py, 256.0, 598.0, 244.0, 708.0, 24.5, 19.0);
            let d_knee_l = sd_ellipse(px, py, 250.0, 654.0, 22.5, 15.0);
            let d_boot_l = sd_capsule(px, py, 244.0, 706.0, 230.0, 720.0, 19.0, 16.0);

            let d_leg_r = sd_capsule(px, py, 328.0, 598.0, 340.0, 708.0, 25.0, 19.5);
            let d_knee_r = sd_ellipse(px, py, 334.0, 654.0, 23.0, 15.0);
            let d_boot_r = sd_capsule(px, py, 340.0, 706.0, 355.0, 720.0, 19.5, 16.0);

            let d_armor = d_gorget
                .min(d_pauld_l)
                .min(d_pauld_r)
                .min(d_torso)
                .min(d_faulds)
                .min(d_arm_l)
                .min(d_hand_l)
                .min(d_arm_r)
                .min(d_hand_r)
                .min(d_leg_l)
                .min(d_knee_l)
                .min(d_boot_l)
                .min(d_leg_r)
                .min(d_knee_r)
                .min(d_boot_r);
            let a_armor = 1.0 - smoothstep(-1.6, 1.6, d_armor);

            let mut armor_col = [0.0f32; 3];
            if a_armor > 0.01 {
                let mut nx = ((px - 292.0) / 68.0).clamp(-0.95, 0.95);
                let mut ny = ((py - 515.0) / 160.0).clamp(-0.85, 0.85);
                let mut seam = 0.0f32;
                let ridge: f32;

                if d_hand_l < 2.2 {
                    nx = (px - 188.0) / 18.0;
                    ny = (py - 526.0) / 16.0;
                    seam = seam.max((-((py - 520.0).abs() * 0.9).powi(2)).exp() * 0.62);
                    seam = seam.max((-((py - 531.0).abs() * 0.9).powi(2)).exp() * 0.62);
                    ridge = 0.65;
                } else if d_pauld_l < 1.8 && py < 522.0 {
                    nx = (px - 214.0) / 44.0;
                    ny = (py - 486.0) / 40.0;
                    let l1 = (-(((py - 472.0 - (px - 214.0) * 0.16).abs()) * 0.72).powi(2)).exp();
                    let l2 = (-(((py - 494.0 - (px - 214.0) * 0.14).abs()) * 0.72).powi(2)).exp();
                    seam = (l1 + l2).min(1.0) * 0.85;
                    ridge = (1.0 - ny.abs()).max(0.0) * 0.82;
                } else if d_pauld_r < 1.8 && py < 525.0 {
                    nx = (px - 366.0) / 48.0;
                    ny = (py - 488.0) / 42.0;
                    let l1 = (-(((py - 473.0 + (px - 366.0) * 0.16).abs()) * 0.72).powi(2)).exp();
                    let l2 = (-(((py - 496.0 + (px - 366.0) * 0.14).abs()) * 0.72).powi(2)).exp();
                    seam = (l1 + l2).min(1.0) * 0.85;
                    ridge = (1.0 - ny.abs()).max(0.0) * 0.62;
                } else if d_gorget < 2.0 && py < 466.0 {
                    nx = (px - 290.0) / 64.0;
                    ny = (py - 448.0) / 26.0;
                    seam = (-((py - 456.0 - (px - 290.0).powi(2) * 0.0018).abs() * 0.75).powi(2)).exp() * 0.72;
                    ridge = (-((py - 444.0).abs() * 0.30).powi(2)).exp() * 0.72;
                } else if d_leg_l.min(d_leg_r) < 2.2 && py > 602.0 {
                    let is_l = d_leg_l < d_leg_r;
                    let lcx = if is_l { 250.0 } else { 334.0 };
                    nx = (px - lcx) / 25.0;
                    ny = (py - 660.0) / 68.0;
                    let knee = (-((py - 654.0).abs() * 0.18).powi(2)).exp();
                    let k_seam = (-((py - 642.0).abs() * 0.70).powi(2)).exp()
                        + (-((py - 667.0).abs() * 0.70).powi(2)).exp();
                    seam = k_seam.min(1.0) * 0.82;
                    ridge = knee * 0.85 + (1.0 - (nx.abs() * 1.4).min(1.0)) * 0.42;
                    if (px - 292.0).abs() < 16.0 {
                        seam = seam.max(0.86);
                    }
                } else if d_arm_r.min(d_hand_r) < 2.2 && px > 350.0 {
                    nx = (px - 372.0) / 24.0;
                    ny = (py - 546.0) / 42.0;
                    seam = (-((py - 538.0).abs() * 0.72).powi(2)).exp() * 0.75;
                    ridge = 0.48;
                } else {
                    let keel = (-((px - 292.0).abs() * 0.14).powi(2)).exp()
                        * (1.0 - smoothstep(548.0, 574.0, py));
                    let belt = (-((px - 292.0).abs() * 0.15).powi(2)).exp()
                        * (-((py - 560.0).abs() * 0.22).powi(2)).exp();
                    ridge = (keel * 0.88).max(belt * 0.92);
                    if py > 546.0 {
                        for fy in [552.0f32, 572.0, 592.0] {
                            let arc = fy + ((px - 292.0).powi(2)) * 0.0020;
                            seam = seam.max((-((py - arc).abs() * 0.80).powi(2)).exp() * 0.86);
                        }
                    }
                }

                // Deep neck/shoulder crevice shadow + gleaming metallic edge highlights
                let neck_shadow = (-((px - 290.0) / 56.0).powi(2)).exp()
                    * (1.0 - smoothstep(436.0, 466.0, py));
                let shadow = (1.0 - seam * 0.68) * (1.0 - neck_shadow * 0.64);
                armor_col = scale3(eval_steel_shade(nx, ny, fine_metal, ridge), (shadow * 1.22).clamp(0.14, 1.0));

                // Diagonal warm olive-bronze leather cape strap across breastplate (matches screenshot!)
                let d_strap = sd_segment(px, py, 224.0, 460.0, 342.0, 566.0) - 9.0;
                let a_strap = (1.0 - smoothstep(-1.5, 1.5, d_strap))
                    * (1.0 - smoothstep(-1.5, 1.5, d_torso.min(d_gorget)));
                if a_strap > 0.01 && py > 448.0 {
                    let strap_tex = 0.75 + 0.35 * fbm(px * 0.18, py * 0.18);
                    let buckle = (-((px - 280.0) / 9.0).powi(2) - ((py - 510.0) / 9.0).powi(2)).exp();
                    let s_col = mix3(
                        scale3(strap_olive, strap_tex),
                        [0.92, 0.86, 0.72],
                        buckle * 0.75,
                    );
                    armor_col = mix3(armor_col, s_col, a_strap * 0.92);
                }
            }

            // --- 3. 3/4 Scottish Fold Kitten Head (round dome, snug folded ears, deep dark eyes) ---
            let dx_h = px - HEAD_CX;
            let dy_h = py - HEAD_CY;
            let ang_h = dy_h.atan2(dx_h);

            // Anisotropic radial fur coordinates from nose bridge `(268, 396)`
            let rx_f = px - 268.0;
            let ry_f = py - 396.0;
            let f_ang = ry_f.atan2(rx_f);
            let f_rad = (rx_f * rx_f + ry_f * ry_f).sqrt();
            let fur_strands = fbm(f_ang * 14.0, f_rad * 0.095) * 0.65
                + fbm(f_ang * 26.0 + 4.2, f_rad * 0.18) * 0.35;
            let outer_fluff = (fur_strands - 0.5) * 7.5 + (f_ang * 36.0).sin() * 1.4;

            // Smooth domed Scottish Fold / kitten head with snug tucked ear flaps
            // (angled smoothly downward into the dome so silhouette is 100% kitten, never bear ears)
            let cheek_w = if dy_h > -2.0 {
                1.0 + 0.11 * smoothstep(-2.0, 46.0, dy_h)
            } else {
                1.0 - 0.04 * smoothstep(-2.0, -68.0, dy_h)
            };
            let r_head = ((dx_h / (83.0 * cheek_w)).powi(2) + (dy_h / 76.0).powi(2)).sqrt();
            let d_head = (r_head - 1.0) * 76.0 + outer_fluff * 0.72;

            // True Scottish Fold folded ears tucked cleanly BELOW the rounded central dome
            let d_ear_l = sd_triangle(px, py, 206.0, 342.0, 214.0, 306.0, 248.0, 310.0) - 6.0 + outer_fluff * 0.35;
            let d_ear_r = sd_triangle(px, py, 372.0, 342.0, 364.0, 305.0, 330.0, 309.0) - 6.0 + outer_fluff * 0.35;
            let d_cat_head = d_head.min(d_ear_l).min(d_ear_r);
            let a_head = 1.0 - smoothstep(-2.0, 2.5, d_cat_head);

            let mut head_open = [0.0f32; 3];
            let mut head_closed = [0.0f32; 3];
            let mut eye_w = 0.0f32;
            let mut fur_w = 0.0f32;

            if a_head > 0.005 {
                // Spherical illumination from upper-left + soft shadow wrapping around right cheek
                let hnx = (dx_h / 84.0).clamp(-1.0, 1.0);
                let hny = (dy_h / 76.0).clamp(-1.0, 1.0);
                let hnz = (1.0 - (hnx * hnx + hny * hny).min(0.95)).sqrt();
                let key = (hnx * -0.42 + hny * -0.38 + hnz * 0.82).clamp(0.0, 1.0);

                // Forehead highlight sheen + dark crown & tabby "M" stripes
                let crown_dark = smoothstep(8.0, -64.0, dy_h) * 0.62;
                let right_shade = smoothstep(12.0, 78.0, dx_h) * 0.42;
                let fh_glow = (-(((dx_h + 4.0) / 30.0).powi(2)) - ((dy_h + 26.0) / 14.0).powi(2)).exp();

                let m_center = (-((dx_h + 10.0) / 4.8).powi(2)).exp();
                let m_sides = (-(((dx_h + 10.0).abs() - 18.0 - (dy_h + 46.0) * 0.14) / 5.2).powi(2)).exp();
                let tabby_m = m_center.max(m_sides)
                    * smoothstep(-64.0, -38.0, dy_h)
                    * (1.0 - smoothstep(-32.0, -10.0, dy_h));

                let cheek_puffs = (-(((dx_h + 10.0).abs() - 28.0) / 32.0).powi(2)
                    - ((dy_h - 26.0) / 24.0).powi(2))
                    .exp()
                    .clamp(0.0, 1.0);
                let muzzle_pad = (-(((dx_h + 20.0) / 26.0).powi(2)) - ((dy_h - 36.0) / 22.0).powi(2))
                    .exp()
                    .clamp(0.0, 1.0);

                // Base fur color blending warm cream cheeks, olive-golden bridge, cool taupe shadow
                let mut base = mix3(fur_cream, fur_warm, (crown_dark * 0.65 + (1.0 - key) * 0.42).clamp(0.0, 1.0));
                base = mix3(base, fur_taupe, right_shade);
                base = mix3(base, fur_dark, (tabby_m * 0.42).clamp(0.0, 0.65));
                base = mix3(base, fur_cream, (cheek_puffs * 0.58 + muzzle_pad * 0.78 + fh_glow * 0.45).clamp(0.0, 0.94));

                // Ear crease shading (subtle Scottish Fold crease line)
                if d_ear_l < d_head + 2.0 {
                    let fold = (-((sd_segment(px, py, 204.0, 326.0, 244.0, 302.0)) / 3.5).powi(2)).exp();
                    base = mix3(base, fur_taupe, 0.28 + fold * 0.34);
                } else if d_ear_r < d_head + 2.0 {
                    let fold = (-((sd_segment(px, py, 372.0, 326.0, 332.0, 301.0)) / 3.5).powi(2)).exp();
                    base = mix3(base, fur_taupe, 0.34 + fold * 0.34);
                }

                // Nose triangle (3/4 shifted left) + adorable ":3" muzzle + crisp pale whiskers
                let d_nose = sd_triangle(px, py, 259.0, 396.0, 277.0, 396.0, 268.0, 406.0);
                let a_nose = 1.0 - smoothstep(-1.4, 1.6, d_nose);
                let nose_hl = (-(((px - 266.0) / 4.2).powi(2)) - ((py - 397.5) / 2.4).powi(2)).exp();
                base = mix3(base, add3(nose_pink, [0.16 * nose_hl; 3]), a_nose * 0.94);

                let d_m_stem = sd_segment(px, py, 268.0, 406.0, 268.0, 415.0);
                let d_m_l = sd_segment(px, py, 268.0, 415.0, 253.0, 418.0);
                let d_m_r = sd_segment(px, py, 268.0, 415.0, 285.0, 418.0);
                let a_mouth = 1.0 - smoothstep(0.6, 2.5, d_m_stem.min(d_m_l).min(d_m_r));
                base = mix3(base, fur_dark, a_mouth * 0.78);

                // Subtle fine hair strand modulation + silhouette rim falloff
                let fur_detail = 0.86 + 0.24 * fur_strands + (ang_h * 40.0).sin() * 0.03;
                let rim_dark = smoothstep(-20.0, 1.0, d_cat_head);
                let shaded = scale3(base, fur_detail * (0.72 + 0.36 * key) * (1.0 - 0.36 * rim_dark));
                head_open = shaded;
                head_closed = shaded;

                // --- Big Expressive Dark Glossy Kitten Eyes (matching screenshot depth!) ---
                for &(ex, ey, erx, ery) in &[
                    (LEFT_EYE_X, LEFT_EYE_Y, EYE_RX * 0.88, EYE_RY * 0.95),
                    (RIGHT_EYE_X, RIGHT_EYE_Y, EYE_RX * 1.08, EYE_RY * 1.04),
                ] {
                    let edx = (px - ex) / erx;
                    let edy = (py - ey) / ery;
                    let er = (edx * edx + edy * edy).sqrt();
                    let in_eye = 1.0 - smoothstep(0.84, 1.12, er);
                    eye_w = eye_w.max(1.0 - smoothstep(0.95, 1.28, er));

                    if in_eye > 0.005 {
                        // Velvety obsidian black core + subtle warm iris bounce + crisp pinpoint catchlight
                        let iris_bounce = (-((edx) / 0.60).powi(2) - ((edy - 0.45) / 0.30).powi(2)).exp();
                        let rim_liner = smoothstep(0.70, 1.02, er);
                        let mut open_eye_col = [
                            0.014 + 0.11 * iris_bounce,
                            0.013 + 0.09 * iris_bounce,
                            0.012 + 0.07 * iris_bounce,
                        ];
                        open_eye_col = scale3(open_eye_col, 1.0 - 0.72 * rim_liner);
                        // Small sharp glossy catchlight at upper-right/top-left
                        let sp1 = (-(((px - (ex + erx * 0.22)) / 3.4).powi(2))
                            - ((py - (ey - ery * 0.30)) / 3.0).powi(2))
                            .exp();
                        let sp2 = (-(((px - (ex - erx * 0.32)) / 2.0).powi(2))
                            - ((py - (ey + ery * 0.28)) / 2.0).powi(2))
                            .exp();
                        open_eye_col = add3(open_eye_col, [sp1 * 0.82 + sp2 * 0.25; 3]);
                        head_open = mix3(head_open, open_eye_col, in_eye);

                        // Closed sleepy eyelid curve for blink
                        let lid_y = ey + 3.0 + edx * edx * 4.2;
                        let d_lid = (py - lid_y).abs();
                        let a_lid = (1.0 - smoothstep(0.7, 2.8, d_lid)) * (1.0 - smoothstep(0.82, 1.04, edx.abs()));
                        let lid_skin = scale3(shaded, 0.90);
                        head_closed = mix3(head_closed, lid_skin, in_eye * 0.96);
                        head_closed = mix3(head_closed, [0.12, 0.11, 0.10], a_lid * 0.94);
                    }
                }

                let rim_fur = smoothstep(-32.0, 2.0, d_cat_head) * a_head;
                fur_w = (rim_fur * 0.88 + 0.28 * a_head) * (1.0 - eye_w);
            }

            // Composite character silhouette: Sword behind left gauntlet hand, head over gorget
            let hand_over_grip = 1.0 - smoothstep(-1.6, 1.6, d_hand_l);
            let mut col = [0.0f32; 3];
            let mut b_mask = 0.0f32;

            if a_armor > 0.005 {
                col = mix3(col, armor_col, a_armor);
                b_mask = b_mask.max(a_armor);
            }
            let sword_front = a_sword * (1.0 - hand_over_grip * 0.94);
            if sword_front > 0.005 {
                col = mix3(col, sword_col, sword_front);
                b_mask = b_mask.max(sword_front);
            }
            let mut col_closed = col;
            if a_head > 0.005 {
                col = mix3(col, head_open, a_head);
                col_closed = mix3(col_closed, head_closed, a_head);
                b_mask = b_mask.max(a_head);
            }

            rgb_open[idx] = col;
            rgb_closed[idx] = col_closed;
            body_mask[idx] = b_mask.clamp(0.0, 1.0);
            fur_weight[idx] = fur_w.clamp(0.0, 1.0);
            sword_mask[idx] = is_blade.clamp(0.0, 1.0);
            eye_mask[idx] = eye_w.clamp(0.0, 1.0);

            // --- 4. Foreground Grass Tuft Patch (around boots & cape hem) ---
            let g_rad = (((px - 296.0) / 168.0).powi(2) + ((py - 722.0) / 40.0).powi(2)).sqrt();
            grass_radius[idx] = g_rad;

            if py >= 662.0 && py <= 764.0 && px >= 122.0 && px <= 535.0 {
                let cell_x = (px / 7.2).round();
                let mut best_g = 0.0f32;
                let mut best_h_t = 0.0f32;
                for off in [-1.0f32, 0.0, 1.0] {
                    let cx_i = cell_x + off;
                    let sx = cx_i * 7.2 + (hash2(cx_i, 19.0) - 0.5) * 3.4;
                    let bh = 22.0 + hash2(cx_i, 47.0) * 40.0;
                    let ln = (hash2(cx_i, 73.0) - 0.42) * 16.0;
                    let by = 724.0 + (hash2(cx_i, 91.0) - 0.5) * 16.0;
                    let edge_fade = 1.0 - smoothstep(0.62, 1.04, ((sx - 304.0) / 192.0).abs());
                    if edge_fade <= 0.01 {
                        continue;
                    }
                    let d_b = sd_capsule(px, py, sx, by, sx + ln, by - bh, 2.5, 0.75);
                    let a_b = (1.0 - smoothstep(-0.8, 1.0, d_b)) * edge_fade;
                    if a_b > best_g {
                        best_g = a_b;
                        best_h_t = ((by - py) / bh.max(1.0)).clamp(0.0, 1.0);
                    }
                }
                let ground_band = (-(((px - 298.0) / 158.0).powi(2)) - ((py - 722.0) / 11.0).powi(2)).exp() * 0.68;
                let g_alpha = best_g.max(ground_band).clamp(0.0, 1.0);
                if g_alpha > 0.01 {
                    let tip_lum = 0.26 + 0.48 * best_h_t;
                    let shimmer = 0.88 + 0.24 * hash2(px * 0.25, py * 0.25);
                    let lum = (tip_lum * shimmer).clamp(0.20, 0.80);
                    grass_rgb[idx] = [lum * 0.78, lum * 0.94, lum * 0.58];
                    grass_mask[idx] = g_alpha;
                }
            }
        }
    }

    KnightPortrait {
        rgb_open,
        rgb_closed,
        body_mask,
        fur_weight,
        sword_mask,
        eye_mask,
        grass_rgb,
        grass_mask,
        grass_radius,
    }
}
