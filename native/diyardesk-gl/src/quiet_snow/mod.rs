pub mod shaders;

use crate::gl::*;
use crate::shaders::FULLSCREEN_VERT;
use crate::state::{AppState, Rect};
use shaders::*;
use std::f32::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// Deterministic Mulberry32 PRNG (Bit-for-bit identical to JS mulberry32)
// =========================================================================
#[derive(Clone, Copy)]
pub struct Mulberry32 {
    a: i32,
}

impl Mulberry32 {
    pub fn new(seed: i32) -> Self {
        Self { a: seed }
    }

    pub fn next_f64(&mut self) -> f64 {
        self.a = self.a.wrapping_add(0x6D2B79F5u32 as i32);
        let a_u = self.a as u32;
        let mut t = (a_u ^ (a_u >> 15)).wrapping_mul(1 | a_u);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }
}

#[inline]
pub fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub fn smoothstep_f32(e0: f32, e1: f32, x: f32) -> f32 {
    let denom = if (e1 - e0).abs() < 1e-9 { 1.0 } else { e1 - e0 };
    let t = clamp_f32((x - e0) / denom, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn mix_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t).round().clamp(0.0, 255.0) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t).round().clamp(0.0, 255.0) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t).round().clamp(0.0, 255.0) as u8,
    ]
}

#[inline]
fn rgba_f32(c: [u8; 3], a: f32) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        a.clamp(0.0, 1.0),
    ]
}

// =========================================================================
// Atmospheric Palettes & Seasonal Colors (1:1 quietsnow-engine.js)
// =========================================================================
#[derive(Clone, Copy)]
pub struct AtmospherePalette {
    pub sky: [[u8; 3]; 4],
    pub haze: [u8; 3],
    pub mtn_far: [u8; 3],
    pub mtn_near: [u8; 3],
    pub snow_top: [u8; 3],
    pub snow_mid: [u8; 3],
    pub snow_deep: [u8; 3],
    pub snow_outline: [u8; 3],
    pub shadow_alpha: f32,
    pub flake_tint: [u8; 3],
    pub rain_tint: [u8; 3],
    pub lamp: bool,
    pub stars: f32,
    pub light_bg: bool,
}

pub const ATMOS: [AtmospherePalette; 4] = [
    // 0: Quiet Night
    AtmospherePalette {
        sky: [[0x07, 0x0d, 0x18], [0x0e, 0x1a, 0x2d], [0x17, 0x29, 0x40], [0x21, 0x37, 0x50]],
        haze: [24, 42, 64],
        mtn_far: [14, 26, 44],
        mtn_near: [10, 20, 35],
        snow_top: [242, 248, 255],
        snow_mid: [212, 226, 242],
        snow_deep: [132, 158, 190],
        snow_outline: [180, 204, 232],
        shadow_alpha: 0.44,
        flake_tint: [236, 245, 255],
        rain_tint: [195, 220, 248],
        lamp: false,
        stars: 0.9,
        light_bg: false,
    },
    // 1: Blue Hour
    AtmospherePalette {
        sky: [[0x0c, 0x1c, 0x35], [0x1b, 0x35, 0x59], [0x35, 0x59, 0x80], [0x60, 0x7e, 0x9c]],
        haze: [76, 108, 138],
        mtn_far: [33, 56, 84],
        mtn_near: [21, 39, 62],
        snow_top: [246, 250, 255],
        snow_mid: [218, 230, 244],
        snow_deep: [138, 165, 198],
        snow_outline: [190, 210, 235],
        shadow_alpha: 0.36,
        flake_tint: [242, 248, 255],
        rain_tint: [210, 230, 252],
        lamp: false,
        stars: 0.25,
        light_bg: false,
    },
    // 2: Misty Morning
    AtmospherePalette {
        sky: [[0x7f, 0x95, 0xaa], [0x9f, 0xb3, 0xc4], [0xbc, 0xcc, 0xd9], [0xd2, 0xdd, 0xe7]],
        haze: [198, 212, 224],
        mtn_far: [122, 142, 162],
        mtn_near: [95, 115, 136],
        snow_top: [252, 254, 255],
        snow_mid: [232, 240, 248],
        snow_deep: [164, 184, 206],
        snow_outline: [136, 160, 186],
        shadow_alpha: 0.28,
        flake_tint: [252, 254, 255],
        rain_tint: [230, 240, 250],
        lamp: false,
        stars: 0.0,
        light_bg: true,
    },
    // 3: Streetlamp Warmth
    AtmospherePalette {
        sky: [[0x0a, 0x0f, 0x1a], [0x13, 0x1d, 0x2d], [0x1c, 0x2a, 0x3e], [0x26, 0x34, 0x48]],
        haze: [36, 46, 62],
        mtn_far: [20, 28, 42],
        mtn_near: [14, 20, 32],
        snow_top: [255, 246, 232],
        snow_mid: [228, 226, 230],
        snow_deep: [132, 148, 174],
        snow_outline: [220, 206, 190],
        shadow_alpha: 0.48,
        flake_tint: [248, 244, 238],
        rain_tint: [232, 222, 204],
        lamp: true,
        stars: 0.45,
        light_bg: false,
    },
];

pub const MODE_TINT: [Option<([u8; 3], f32)>; 4] = [
    None,
    Some(([16, 28, 42], 0.16)),
    Some(([68, 42, 24], 0.18)),
    Some(([82, 48, 66], 0.14)),
];

pub const AUTUMN_COLORS: [([u8; 3], [u8; 3]); 6] = [
    ([188, 52, 36], [134, 28, 22]),   // Crimson maple
    ([218, 96, 32], [156, 54, 18]),   // Burnt orange
    ([232, 162, 44], [174, 108, 24]), // Golden amber
    ([196, 122, 46], [132, 72, 26]),  // Ochre rust
    ([158, 66, 42], [102, 38, 24]),   // Chestnut
    ([212, 134, 52], [148, 84, 30]),  // Warm honey
];

pub const BLOSSOM_COLORS: [([u8; 3], [u8; 3]); 4] = [
    ([255, 228, 238], [240, 178, 202]),
    ([255, 242, 247], [244, 198, 216]),
    ([250, 204, 222], [222, 146, 176]),
    ([255, 218, 232], [234, 162, 192]),
];

// =========================================================================
// Pre-Baked Offscreen Sprite Atlas Rasterizer (32 Sprites in 512x256 RGBA8)
// =========================================================================
const ATLAS_W: usize = 512;
const ATLAS_H: usize = 256;
const CELL_SZ: usize = 64;
const ATLAS_COLS: usize = 8;

#[inline]
pub fn sprite_uv_rect(slot: usize) -> [f32; 4] {
    let col = (slot % ATLAS_COLS) as f32;
    let row = (slot / ATLAS_COLS) as f32;
    let pad = 1.5;
    let u0 = (col * CELL_SZ as f32 + pad) / ATLAS_W as f32;
    let v0 = (row * CELL_SZ as f32 + pad) / ATLAS_H as f32;
    let u1 = ((col + 1.0) * CELL_SZ as f32 - pad) / ATLAS_W as f32;
    let v1 = ((row + 1.0) * CELL_SZ as f32 - pad) / ATLAS_H as f32;
    [u0, v0, u1, v1]
}

#[inline]
pub fn snow_soft_slot(idx: usize) -> usize {
    idx & 3
}

#[inline]
pub fn snow_crystal_slot(idx: usize) -> usize {
    4 + (idx % 3)
}

#[inline]
pub fn snow_bokeh_slot(idx: usize) -> usize {
    7 + (idx & 1)
}

#[inline]
pub fn leaf_slot(species: usize, color_idx: usize) -> usize {
    9 + (species % 3) * 6 + (color_idx % 6)
}

#[inline]
pub fn petal_slot(color_idx: usize) -> usize {
    27 + (color_idx % 4)
}

pub const FLOWER_SLOT: usize = 31;

struct MiniCanvas {
    w: usize,
    h: usize,
    rgba: Vec<[f32; 4]>, // straight RGBA in 0..1
}

impl MiniCanvas {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            rgba: vec![[0.0; 4]; w * h],
        }
    }

    fn blend_pixel(&mut self, x: usize, y: usize, src: [f32; 4]) {
        if x >= self.w || y >= self.h || src[3] <= 0.0005 {
            return;
        }
        let idx = y * self.w + x;
        let dst = self.rgba[idx];
        let sa = src[3].clamp(0.0, 1.0);
        let da = dst[3];
        let out_a = sa + da * (1.0 - sa);
        if out_a <= 1e-6 {
            self.rgba[idx] = [0.0; 4];
            return;
        }
        let r = (src[0] * sa + dst[0] * da * (1.0 - sa)) / out_a;
        let g = (src[1] * sa + dst[1] * da * (1.0 - sa)) / out_a;
        let b = (src[2] * sa + dst[2] * da * (1.0 - sa)) / out_a;
        self.rgba[idx] = [r, g, b, out_a];
    }

    fn eval_stops(stops: &[(f32, [f32; 4])], t: f32) -> [f32; 4] {
        let t = t.clamp(0.0, 1.0);
        if stops.is_empty() {
            return [0.0; 4];
        }
        if t <= stops[0].0 {
            return stops[0].1;
        }
        for w in stops.windows(2) {
            let (t0, c0) = w[0];
            let (t1, c1) = w[1];
            if t <= t1 {
                let u = ((t - t0) / (t1 - t0).max(1e-5)).clamp(0.0, 1.0);
                return [
                    lerp_f32(c0[0], c1[0], u),
                    lerp_f32(c0[1], c1[1], u),
                    lerp_f32(c0[2], c1[2], u),
                    lerp_f32(c0[3], c1[3], u),
                ];
            }
        }
        stops[stops.len() - 1].1
    }

    fn fill_radial_disc(&mut self, cx: f32, cy: f32, r_inner: f32, r_outer: f32, stops: &[(f32, [f32; 4])]) {
        for y in 0..self.h {
            for x in 0..self.w {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                if d <= r_outer + 0.75 {
                    let aa = (r_outer + 0.5 - d).clamp(0.0, 1.0);
                    let t = ((d - r_inner) / (r_outer - r_inner).max(1e-4)).clamp(0.0, 1.0);
                    let mut c = Self::eval_stops(stops, t);
                    c[3] *= aa;
                    self.blend_pixel(x, y, c);
                }
            }
        }
    }

    fn stroke_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, col: [f32; 4]) {
        let hw = width * 0.5;
        let dx = x1 - x0;
        let dy = y1 - y0;
        let l2 = dx * dx + dy * dy;
        for y in 0..self.h {
            for x in 0..self.w {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let t = if l2 > 1e-6 {
                    (((px - x0) * dx + (py - y0) * dy) / l2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let qx = x0 + t * dx;
                let qy = y0 + t * dy;
                let d = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt();
                let cov = (hw + 0.55 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    let mut c = col;
                    c[3] *= cov;
                    self.blend_pixel(x, y, c);
                }
            }
        }
    }

    fn fill_and_stroke_poly<F>(
        &mut self,
        poly: &[(f32, f32)],
        fill_fn: F,
        stroke_col: Option<([f32; 4], f32)>,
    ) where
        F: Fn(f32, f32) -> [f32; 4],
    {
        if poly.len() < 3 {
            return;
        }
        // 2x2 supersampled point-in-polygon fill
        for y in 0..self.h {
            for x in 0..self.w {
                let mut inside_cnt = 0;
                for sy in 0..2 {
                    for sx in 0..2 {
                        let px = x as f32 + (sx as f32 + 0.5) * 0.5;
                        let py = y as f32 + (sy as f32 + 0.5) * 0.5;
                        if point_in_poly(px, py, poly) {
                            inside_cnt += 1;
                        }
                    }
                }
                if inside_cnt > 0 {
                    let cov = inside_cnt as f32 * 0.25;
                    let mut c = fill_fn(x as f32 + 0.5, y as f32 + 0.5);
                    c[3] *= cov;
                    self.blend_pixel(x, y, c);
                }
            }
        }
        if let Some((scol, swidth)) = stroke_col {
            for i in 0..poly.len() {
                let (x0, y0) = poly[i];
                let (x1, y1) = poly[(i + 1) % poly.len()];
                self.stroke_line(x0, y0, x1, y1, swidth, scol);
            }
        }
    }

    fn fill_rotated_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, col: [f32; 4]) {
        let (s, c) = rot.sin_cos();
        for y in 0..self.h {
            for x in 0..self.w {
                let dx = (x as f32 + 0.5) - cx;
                let dy = (y as f32 + 0.5) - cy;
                let lx = dx * c + dy * s;
                let ly = -dx * s + dy * c;
                let norm = ((lx / rx.max(0.1)).powi(2) + (ly / ry.max(0.1)).powi(2)).sqrt();
                if norm <= 1.15 {
                    let cov = ((1.08 - norm) * 4.0).clamp(0.0, 1.0);
                    let mut out = col;
                    out[3] *= cov;
                    self.blend_pixel(x, y, out);
                }
            }
        }
    }

    fn blit_into_atlas(&self, atlas: &mut [u8], slot: usize) {
        let col_idx = slot % ATLAS_COLS;
        let row_idx = slot / ATLAS_COLS;
        let ox = col_idx * CELL_SZ;
        let oy = row_idx * CELL_SZ;
        let pad = 2usize;
        let dst_sz = CELL_SZ - pad * 2;
        for dy in 0..dst_sz {
            for dx in 0..dst_sz {
                let u = (dx as f32 + 0.5) / dst_sz as f32 * self.w as f32 - 0.5;
                let v = (dy as f32 + 0.5) / dst_sz as f32 * self.h as f32 - 0.5;
                let c = self.sample_bilinear(u, v);
                let ax = ox + pad + dx;
                let ay = oy + pad + dy;
                let a_idx = (ay * ATLAS_W + ax) * 4;
                atlas[a_idx] = (c[0].clamp(0.0, 1.0) * 255.0).round() as u8;
                atlas[a_idx + 1] = (c[1].clamp(0.0, 1.0) * 255.0).round() as u8;
                atlas[a_idx + 2] = (c[2].clamp(0.0, 1.0) * 255.0).round() as u8;
                atlas[a_idx + 3] = (c[3].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }

    fn sample_bilinear(&self, x: f32, y: f32) -> [f32; 4] {
        let x0 = x.floor() as isize;
        let y0 = y.floor() as isize;
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let get = |ix: isize, iy: isize| -> [f32; 4] {
            let cx = ix.clamp(0, self.w as isize - 1) as usize;
            let cy = iy.clamp(0, self.h as isize - 1) as usize;
            self.rgba[cy * self.w + cx]
        };
        let c00 = get(x0, y0);
        let c10 = get(x0 + 1, y0);
        let c01 = get(x0, y0 + 1);
        let c11 = get(x0 + 1, y0 + 1);
        let mut out = [0.0; 4];
        for k in 0..4 {
            let top = lerp_f32(c00[k], c10[k], fx);
            let bot = lerp_f32(c01[k], c11[k], fx);
            out[k] = lerp_f32(top, bot, fy);
        }
        out
    }
}

fn point_in_poly(px: f32, py: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi + 1e-12) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn push_quad_bezier(pts: &mut Vec<(f32, f32)>, cx: f32, cy: f32, x1: f32, y1: f32, steps: usize) {
    let (x0, y0) = *pts.last().unwrap_or(&(0.0, 0.0));
    for s in 1..=steps {
        let t = s as f32 / steps as f32;
        let it = 1.0 - t;
        let x = it * it * x0 + 2.0 * it * t * cx + t * t * x1;
        let y = it * it * y0 + 2.0 * it * t * cy + t * t * y1;
        pts.push((x, y));
    }
}

fn push_cubic_bezier(
    pts: &mut Vec<(f32, f32)>,
    c1x: f32,
    c1y: f32,
    c2x: f32,
    c2y: f32,
    x1: f32,
    y1: f32,
    steps: usize,
) {
    let (x0, y0) = *pts.last().unwrap_or(&(0.0, 0.0));
    for s in 1..=steps {
        let t = s as f32 / steps as f32;
        let it = 1.0 - t;
        let x = it * it * it * x0 + 3.0 * it * it * t * c1x + 3.0 * it * t * t * c2x + t * t * t * x1;
        let y = it * it * it * y0 + 3.0 * it * it * t * c1y + 3.0 * it * t * t * c2y + t * t * t * y1;
        pts.push((x, y));
    }
}

fn build_leaf_polygon(species: usize, sz: f32, center: f32) -> Vec<(f32, f32)> {
    let s = sz * 0.42;
    let mut pts = Vec::with_capacity(96);
    if species == 0 {
        // Maple-like 5-lobe leaf (exact quietsnow-engine.js coordinates)
        pts.push((0.0, s * 0.92));
        push_quad_bezier(&mut pts, -s * 0.22, s * 0.56, -s * 0.78, s * 0.48, 8);
        push_quad_bezier(&mut pts, -s * 0.55, s * 0.2, -s * 0.96, -s * 0.04, 8);
        push_quad_bezier(&mut pts, -s * 0.52, -s * 0.14, -s * 0.62, -s * 0.62, 8);
        push_quad_bezier(&mut pts, -s * 0.24, -s * 0.42, 0.0, -s * 1.0, 8);
        push_quad_bezier(&mut pts, s * 0.24, -s * 0.42, s * 0.62, -s * 0.62, 8);
        push_quad_bezier(&mut pts, s * 0.52, -s * 0.14, s * 0.96, -s * 0.04, 8);
        push_quad_bezier(&mut pts, s * 0.55, s * 0.2, s * 0.78, s * 0.48, 8);
        push_quad_bezier(&mut pts, s * 0.22, s * 0.56, 0.0, s * 0.92, 8);
    } else if species == 1 {
        // Oak-like scalloped leaf
        pts.push((0.0, s * 0.95));
        push_cubic_bezier(&mut pts, -s * 0.36, s * 0.72, -s * 0.68, s * 0.42, -s * 0.42, s * 0.22, 10);
        push_cubic_bezier(&mut pts, -s * 0.82, s * 0.02, -s * 0.72, -s * 0.34, -s * 0.36, -s * 0.44, 10);
        push_cubic_bezier(&mut pts, -s * 0.54, -s * 0.74, -s * 0.22, -s * 0.98, 0.0, -s * 0.96, 10);
        push_cubic_bezier(&mut pts, s * 0.22, -s * 0.98, s * 0.54, -s * 0.74, s * 0.36, -s * 0.44, 10);
        push_cubic_bezier(&mut pts, s * 0.72, -s * 0.34, s * 0.82, s * 0.02, s * 0.42, s * 0.22, 10);
        push_cubic_bezier(&mut pts, s * 0.68, s * 0.42, s * 0.36, s * 0.72, 0.0, s * 0.95, 10);
    } else {
        // Birch / elm tapered leaf
        pts.push((0.0, s * 0.92));
        push_cubic_bezier(&mut pts, -s * 0.62, s * 0.52, -s * 0.68, -s * 0.28, 0.0, -s * 0.98, 16);
        push_cubic_bezier(&mut pts, s * 0.68, -s * 0.28, s * 0.62, s * 0.52, 0.0, s * 0.92, 16);
    }
    for p in &mut pts {
        p.0 += center;
        p.1 += center;
    }
    pts
}

fn build_petal_polygon(center: f32) -> Vec<(f32, f32)> {
    let mut pts = Vec::with_capacity(64);
    pts.push((0.0, 11.0));
    push_cubic_bezier(&mut pts, -8.5, 6.0, -9.5, -5.5, -3.5, -11.2, 12);
    push_quad_bezier(&mut pts, -1.2, -12.4, 0.0, -9.0, 8); // sakura apical notch
    push_quad_bezier(&mut pts, 1.2, -12.4, 3.5, -11.2, 8);
    push_cubic_bezier(&mut pts, 9.5, -5.5, 8.5, 6.0, 0.0, 11.0, 12);
    for p in &mut pts {
        p.0 += center;
        p.1 += center;
    }
    pts
}

pub fn build_sprite_atlas() -> Vec<u8> {
    let mut atlas = vec![0u8; ATLAS_W * ATLAS_H * 4];

    // 1. Soft granular snow puffs (slots 0..4, sz = 28)
    for v in 0..4 {
        let sz = 28usize;
        let mut c = MiniCanvas::new(sz, sz);
        let mut r = Mulberry32::new(101 + (v as i32) * 37);
        let cx = sz as f32 * 0.5;
        let cy = sz as f32 * 0.5;
        let stops = [
            (0.0, [1.0, 1.0, 1.0, 0.92]),
            (0.45, [244.0 / 255.0, 250.0 / 255.0, 1.0, 0.68]),
            (0.78, [220.0 / 255.0, 236.0 / 255.0, 252.0 / 255.0, 0.22]),
            (1.0, [220.0 / 255.0, 236.0 / 255.0, 252.0 / 255.0, 0.0]),
        ];
        for i in 0..4 {
            let ox = if i == 0 { 0.0 } else { (r.next_f32() - 0.5) * 4.2 };
            let oy = if i == 0 { 0.0 } else { (r.next_f32() - 0.5) * 4.2 };
            let rad = if i == 0 { 10.5 } else { 5.5 + r.next_f32() * 4.0 };
            c.fill_radial_disc(cx + ox, cy + oy, 0.0, rad, &stops);
        }
        c.blit_into_atlas(&mut atlas, snow_soft_slot(v));
    }

    // 2. Delicate 6-arm snow crystals (slots 4..7, sz = 36)
    for v in 0..3 {
        let sz = 36usize;
        let mut c = MiniCanvas::new(sz, sz);
        let cx = sz as f32 * 0.5;
        let cy = sz as f32 * 0.5;
        let glow_stops = [
            (0.0, [1.0, 1.0, 1.0, 0.65]),
            (0.5, [235.0 / 255.0, 246.0 / 255.0, 1.0, 0.24]),
            (1.0, [220.0 / 255.0, 238.0 / 255.0, 1.0, 0.0]),
        ];
        c.fill_radial_disc(cx, cy, 0.0, sz as f32 * 0.46, &glow_stops);
        let stroke_col = [1.0, 1.0, 1.0, 0.88];
        let b1 = 5.2 + v as f32 * 0.7;
        let b2 = 9.5 + (v % 2) as f32 * 0.8;
        for a in 0..6 {
            let ang = (a as f32) * PI / 3.0;
            let (sa, ca) = ang.sin_cos();
            let rot = |lx: f32, ly: f32| -> (f32, f32) {
                (cx + lx * ca - ly * sa, cy + lx * sa + ly * ca)
            };
            let (p0x, p0y) = rot(0.0, 0.0);
            let (p1x, p1y) = rot(0.0, -13.5);
            c.stroke_line(p0x, p0y, p1x, p1y, 1.35, stroke_col);
            for s in [-1.0f32, 1.0f32] {
                let (a0x, a0y) = rot(0.0, -b1);
                let (a1x, a1y) = rot(s * 3.8, -b1 - 2.6);
                c.stroke_line(a0x, a0y, a1x, a1y, 0.95, stroke_col);
                let (c0x, c0y) = rot(0.0, -b2);
                let (c1x, c1y) = rot(s * 2.8, -b2 - 2.2);
                c.stroke_line(c0x, c0y, c1x, c1y, 0.95, stroke_col);
            }
        }
        c.blit_into_atlas(&mut atlas, snow_crystal_slot(v));
    }

    // 3. Near-camera bokeh snow (slots 7..9, sz = 48)
    for v in 0..2 {
        let sz = 48usize;
        let mut c = MiniCanvas::new(sz, sz);
        let center = sz as f32 * 0.5;
        let c0 = if v == 0 {
            [1.0, 1.0, 1.0, 0.48]
        } else {
            [1.0, 252.0 / 255.0, 244.0 / 255.0, 0.36]
        };
        let stops = [
            (0.0, c0),
            (0.55, [240.0 / 255.0, 248.0 / 255.0, 1.0, 0.22]),
            (0.82, [225.0 / 255.0, 240.0 / 255.0, 1.0, 0.06]),
            (1.0, [225.0 / 255.0, 240.0 / 255.0, 1.0, 0.0]),
        ];
        c.fill_radial_disc(center, center, sz as f32 * 0.08, sz as f32 * 0.48, &stops);
        c.blit_into_atlas(&mut atlas, snow_bokeh_slot(v));
    }

    // 4. Autumn leaf sprites (3 species x 6 colors = slots 9..27, sz = 44)
    for sp in 0..3 {
        let sz = 44usize;
        let center = sz as f32 * 0.5;
        let poly = build_leaf_polygon(sp, sz as f32, center);
        for (ci, &(hi, lo)) in AUTUMN_COLORS.iter().enumerate() {
            let mut c = MiniCanvas::new(sz, sz);
            let mid = mix_rgb(hi, lo, 0.52);
            let stops = [
                (0.0, rgba_f32(hi, 0.98)),
                (0.65, rgba_f32(mid, 0.96)),
                (1.0, rgba_f32(lo, 0.95)),
            ];
            // Linear gradient from (-12, -16) to (12, 18) relative to center
            let gx0 = center - 12.0;
            let gy0 = center - 16.0;
            let gdx = 24.0f32;
            let gdy = 34.0f32;
            let gl2 = gdx * gdx + gdy * gdy;
            c.fill_and_stroke_poly(
                &poly,
                |px, py| {
                    let t = (((px - gx0) * gdx + (py - gy0) * gdy) / gl2).clamp(0.0, 1.0);
                    MiniCanvas::eval_stops(&stops, t)
                },
                Some((rgba_f32(lo, 0.55), 0.8)),
            );
            // Stem & delicate midrib veins
            let vein_col = rgba_f32(mix_rgb(lo, [50, 24, 12], 0.45), 0.62);
            let sz_f = sz as f32;
            c.stroke_line(center, center + sz_f * 0.44, center, center - sz_f * 0.34, 1.05, vein_col);
            for k in [-1.0f32, 1.0f32] {
                c.stroke_line(
                    center,
                    center + sz_f * 0.12,
                    center + k * sz_f * 0.2,
                    center - sz_f * 0.04,
                    0.65,
                    vein_col,
                );
                c.stroke_line(
                    center,
                    center - sz_f * 0.08,
                    center + k * sz_f * 0.16,
                    center - sz_f * 0.22,
                    0.65,
                    vein_col,
                );
            }
            c.blit_into_atlas(&mut atlas, leaf_slot(sp, ci));
        }
    }

    // 5. Cherry blossom petals (4 colors = slots 27..31, sz = 32)
    let mut petal_canvases = Vec::with_capacity(4);
    {
        let sz = 32usize;
        let center = sz as f32 * 0.5;
        let poly = build_petal_polygon(center);
        for (ci, &(hi, lo)) in BLOSSOM_COLORS.iter().enumerate() {
            let mut c = MiniCanvas::new(sz, sz);
            let mid = mix_rgb(hi, lo, 0.45);
            let stops = [
                (0.0, rgba_f32(hi, 0.98)),
                (0.7, rgba_f32(mid, 0.92)),
                (1.0, rgba_f32(lo, 0.86)),
            ];
            c.fill_and_stroke_poly(
                &poly,
                |px, py| {
                    let d = ((px - (center - 1.0)).powi(2) + (py - (center - 0.5)).powi(2)).sqrt();
                    let t = ((d - 1.0) / 12.0).clamp(0.0, 1.0);
                    MiniCanvas::eval_stops(&stops, t)
                },
                None,
            );
            // Subtle translucent highlight spoon sheen
            c.fill_rotated_ellipse(center - 2.2, center - 1.5, 2.2, 5.2, -0.25, [1.0, 1.0, 1.0, 0.34]);
            c.blit_into_atlas(&mut atlas, petal_slot(ci));
            petal_canvases.push(c);
        }
    }

    // 6. Whole cherry blossom flower on slender stalk (slot 31, sz = 44)
    {
        let sz = 44usize;
        let center = sz as f32 * 0.5;
        let mut c = MiniCanvas::new(sz, sz);
        // Thin quadratic stalk: moveTo(0, 2), quadraticCurveTo(5, 11, 9, 17)
        let mut stalk = vec![(center, center + 2.0)];
        push_quad_bezier(&mut stalk, center + 5.0, center + 11.0, center + 9.0, center + 17.0, 8);
        for w in stalk.windows(2) {
            c.stroke_line(w[0].0, w[0].1, w[1].0, w[1].1, 1.2, [134.0 / 255.0, 88.0 / 255.0, 96.0 / 255.0, 0.72]);
        }
        // 5 rotated petals drawn at (-8, -17, 16, 17) in rotated local frame
        for p in 0..5 {
            let ang = (p as f32 * TAU) / 5.0;
            let (sa, ca) = ang.sin_cos();
            let pet = &petal_canvases[p % petal_canvases.len()];
            for y in 0..sz {
                for x in 0..sz {
                    let dx = (x as f32 + 0.5) - center;
                    let dy = (y as f32 + 0.5) - center;
                    // Inverse rotate into petal's local frame
                    let lx = dx * ca + dy * sa;
                    let ly = -dx * sa + dy * ca;
                    if lx >= -8.0 && lx <= 8.0 && ly >= -17.0 && ly <= 0.0 {
                        let u = ((lx + 8.0) / 16.0) * pet.w as f32 - 0.5;
                        let v = ((ly + 17.0) / 17.0) * pet.h as f32 - 0.5;
                        let sc = pet.sample_bilinear(u, v);
                        c.blend_pixel(x, y, sc);
                    }
                }
            }
        }
        // Golden-rose stamen core
        let core_stops = [
            (0.0, [1.0, 224.0 / 255.0, 130.0 / 255.0, 0.95]),
            (0.55, [226.0 / 255.0, 126.0 / 255.0, 152.0 / 255.0, 0.85]),
            (1.0, [226.0 / 255.0, 126.0 / 255.0, 152.0 / 255.0, 0.0]),
        ];
        c.fill_radial_disc(center, center, 0.0, 4.5, &core_stops);
        c.blit_into_atlas(&mut atlas, FLOWER_SLOT);
    }

    atlas
}

// =========================================================================
// Quiet Snow Engine State & 1:1 Physics Simulation
// =========================================================================
#[derive(Clone, Copy)]
pub struct QuietSnowParams {
    pub adaptive: i32,
    pub mode: i32,
    pub intensity: f32,
    pub wind: f32,
    pub gusts: f32,
    pub accumulate: i32,
    pub windows: i32,
    pub melt: f32,
    pub cursor: i32,
    pub depth: f32,
    pub atmosphere: i32,
    pub sparkle: f32,
    pub backdrop: f32,
    pub quality: i32,
}

impl Default for QuietSnowParams {
    fn default() -> Self {
        Self {
            adaptive: 1,
            mode: 0,
            intensity: 0.85,
            wind: 0.22,
            gusts: 0.45,
            accumulate: 2,
            windows: 1,
            melt: 0.35,
            cursor: 2,
            depth: 0.85,
            atmosphere: 0,
            sparkle: 0.55,
            backdrop: 0.75,
            quality: 1,
        }
    }
}

#[derive(Clone, Copy)]
pub struct GroundSparkle {
    pub u: f32,
    pub depth_frac: f32,
    pub phase: f32,
    pub freq: f32,
    pub size: f32,
}

#[derive(Clone)]
pub struct IconSurface {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub vx: f32,
    pub vy: f32,
    pub cap_n: usize,
    pub cap_h: Vec<f32>,
    pub wet: f32,
}

#[derive(Clone, Copy)]
pub struct WindowInput {
    pub id: i64,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone)]
pub struct WindowSurface {
    pub id: u32,
    pub hwnd: i64,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub vx: f32,
    pub vy: f32,
    pub cap_n: usize,
    pub cap_h: Vec<f32>,
    pub wet: f32,
    pub z_order: usize,
}

#[derive(Clone, Copy)]
pub struct Flake {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub size: f32,
    pub rot: f32,
    pub vrot: f32,
    pub pitch: f32,
    pub vpitch: f32,
    pub sway_phase: f32,
    pub sway_freq: f32,
    pub sway_amp: f32,
    pub species: usize,
    pub color_idx: usize,
    pub kind: usize, // 0=soft, 1=crystal, 2=bokeh
    pub whole_flower: bool,
    pub alpha: f32,
}

#[derive(Clone, Copy)]
pub struct RainStreak {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub speed: f32,
    pub len: f32,
    pub thick: f32,
    pub alpha: f32,
}

#[derive(Clone, Copy)]
pub struct RainBead {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub vx: f32,
    pub vy: f32,
    pub stuck: f32,
    pub surf_id: u32,
    pub is_window: bool,
    pub hanging: bool,
    pub mass: f32,
    pub wobble: f32,
}

#[derive(Clone, Copy)]
pub struct RainTrail {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub a: f32,
}

#[derive(Clone, Copy)]
pub struct Splash {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub r: f32,
    pub life: f32,
    pub max_life: f32,
}

#[derive(Clone, Copy)]
pub struct SnowLump {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub r: f32,
    pub mass: f32,
    pub rot: f32,
    pub vrot: f32,
    pub src_id: u32,
    pub src_is_window: bool,
}

#[derive(Clone, Copy)]
pub struct Powder {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub r: f32,
    pub life: f32,
    pub max_life: f32,
    pub alpha: f32,
}

#[derive(Clone, Copy)]
pub struct HeapItem {
    pub mode: i32,
    pub x: f32,
    pub y: f32,
    pub surf_id: u32,
    pub is_window: bool,
    pub ox: f32,
    pub oy: f32,
    pub rot: f32,
    pub scale: f32,
    pub squash: f32,
    pub species: usize,
    pub color_idx: usize,
    pub age: f32,
    pub alpha: f32,
}

#[derive(Clone, Copy)]
pub struct PointerState {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub px: f32,
    pub py: f32,
    pub vx: f32,
    pub vy: f32,
    pub speed: f32,
    pub down: bool,
    pub last_move: f32,
}

impl Default for PointerState {
    fn default() -> Self {
        Self {
            active: false,
            x: -9999.0,
            y: -9999.0,
            px: -9999.0,
            py: -9999.0,
            vx: 0.0,
            vy: 0.0,
            speed: 0.0,
            down: false,
            last_move: 0.0,
        }
    }
}

pub struct QuietSnowState {
    pub p: QuietSnowParams,
    pub user_mode: i32,
    pub seasonal_mode_override: Option<i32>,
    pub w: f32,
    pub h: f32,
    pub low_mode: bool,
    pub sim_time: f32,
    pub rng: Mulberry32,

    pub cell_w: f32,
    pub ground_n: usize,
    pub ground_h: Vec<f32>,
    pub ground_sparkles: Vec<GroundSparkle>,

    pub surfaces: Vec<IconSurface>,
    pub win_surfaces: Vec<WindowSurface>,
    pub next_surf_id: u32,

    pub flakes: Vec<Flake>,
    pub rain_streaks: Vec<RainStreak>,
    pub rain_beads: Vec<RainBead>,
    pub rain_trails: Vec<RainTrail>,
    pub splashes: Vec<Splash>,
    pub lumps: Vec<SnowLump>,
    pub powders: Vec<Powder>,
    pub heaps: Vec<HeapItem>,

    pub gust_val: f32,
    pub gust_target: f32,
    pub gust_timer: f32,
    pub ptr: PointerState,
}

impl Default for QuietSnowState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0, QuietSnowParams::default(), 123456789)
    }
}

impl QuietSnowState {
    pub fn new(w: f32, h: f32, params: QuietSnowParams, rng_seed: i32) -> Self {
        let mut st = Self {
            p: params,
            user_mode: params.mode,
            seasonal_mode_override: None,
            w: w.max(240.0),
            h: h.max(160.0),
            low_mode: false,
            sim_time: 0.0,
            rng: Mulberry32::new(rng_seed),
            cell_w: 4.0,
            ground_n: 0,
            ground_h: Vec::new(),
            ground_sparkles: Vec::new(),
            surfaces: Vec::new(),
            win_surfaces: Vec::new(),
            next_surf_id: 1,
            flakes: Vec::new(),
            rain_streaks: Vec::new(),
            rain_beads: Vec::new(),
            rain_trails: Vec::new(),
            splashes: Vec::new(),
            lumps: Vec::new(),
            powders: Vec::new(),
            heaps: Vec::new(),
            gust_val: 0.0,
            gust_target: 0.0,
            gust_timer: 2.0,
            ptr: PointerState::default(),
        };
        st.alloc_ground(true);
        st.reconcile_pools(true);
        st
    }

    #[inline]
    fn rand(&mut self) -> f32 {
        self.rng.next_f32()
    }

    pub fn set_low_mode(&mut self, v: bool) {
        if v != self.low_mode {
            self.low_mode = v;
            self.reconcile_pools(false);
        }
    }

    pub fn get_quality_factor(&self) -> f32 {
        if self.low_mode {
            0.45
        } else if self.p.quality == 0 {
            0.58
        } else if self.p.quality == 2 {
            1.25
        } else {
            0.92
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let prev_w = self.w;
        self.w = w.max(240.0);
        self.h = h.max(160.0);
        if (prev_w - self.w).abs() > 12.0 || self.ground_n == 0 {
            self.alloc_ground(self.ground_n == 0);
        }
        self.reconcile_pools(false);
    }

    pub fn set_seasonal_override(&mut self, mode: Option<i32>) {
        let normalized = mode.map(|m| m.clamp(0, 3));
        if self.seasonal_mode_override == normalized {
            return;
        }
        self.seasonal_mode_override = normalized;
        let effective_mode = normalized.unwrap_or(self.user_mode).clamp(0, 3);
        if self.p.mode != effective_mode {
            self.p.mode = effective_mode;
            self.on_mode_switch();
            self.reconcile_pools(false);
        }
    }

    pub fn sync_params(&mut self, next: QuietSnowParams) {
        self.user_mode = next.mode.clamp(0, 3);
        let mut effective = next;
        if let Some(ov) = self.seasonal_mode_override {
            effective.mode = ov.clamp(0, 3);
        }
        let mode_changed = self.p.mode != effective.mode;
        let acc_changed = self.p.accumulate != effective.accumulate
            || self.p.adaptive != effective.adaptive
            || self.p.windows != effective.windows;
        self.p = effective;

        if mode_changed {
            self.on_mode_switch();
        }
        if acc_changed {
            if self.p.accumulate == 0 {
                self.ground_h.fill(0.0);
                for s in &mut self.surfaces {
                    s.cap_h.fill(0.0);
                    s.wet = 0.0;
                }
                for w in &mut self.win_surfaces {
                    w.cap_h.fill(0.0);
                    w.wet = 0.0;
                }
                self.heaps.clear();
            } else {
                if self.p.accumulate == 1 || self.p.adaptive == 0 {
                    for s in &mut self.surfaces {
                        s.cap_h.fill(0.0);
                        s.wet = 0.0;
                    }
                    self.heaps.retain(|h| h.surf_id == 0 || h.is_window);
                }
                if self.p.windows == 0 {
                    for w in &mut self.win_surfaces {
                        w.cap_h.fill(0.0);
                        w.wet = 0.0;
                    }
                    self.heaps.retain(|h| !h.is_window);
                }
            }
        }
        self.reconcile_pools(false);
    }

    pub fn on_mode_switch(&mut self) {
        self.flakes.clear();
        self.rain_streaks.clear();
        self.rain_beads.clear();
        self.rain_trails.clear();
        self.splashes.clear();
        self.lumps.clear();
        self.powders.clear();
        self.heaps.clear();
        self.ground_h.fill(0.0);
        for s in &mut self.surfaces {
            s.cap_h.fill(0.0);
            s.wet = 0.0;
        }
        for w in &mut self.win_surfaces {
            w.cap_h.fill(0.0);
            w.wet = 0.0;
        }
        if self.p.mode == 0 && self.p.accumulate > 0 {
            self.seed_initial_snow();
        } else if (self.p.mode == 2 || self.p.mode == 3) && self.p.accumulate > 0 {
            self.seed_initial_heaps();
        } else if self.p.mode == 1 {
            self.seed_initial_rain_beads();
        }
        self.reconcile_pools(true);
    }

    pub fn alloc_ground(&mut self, warm_start: bool) {
        let old_h = std::mem::take(&mut self.ground_h);
        let old_n = self.ground_n;
        self.cell_w = if self.w > 1600.0 { 5.0 } else { 4.0 };
        self.ground_n = 40usize.max((self.w / self.cell_w).ceil() as usize + 1);
        self.ground_h = vec![0.0; self.ground_n];
        if old_n > 0 && !warm_start {
            for i in 0..self.ground_n {
                let oi = (old_n - 1).min(
                    ((i as f32 / (self.ground_n - 1).max(1) as f32) * (old_n - 1) as f32).round()
                        as usize,
                );
                self.ground_h[i] = old_h[oi];
            }
        } else if warm_start && self.p.mode == 0 && self.p.accumulate > 0 {
            self.seed_initial_snow();
        }

        self.ground_sparkles.clear();
        let mut r = Mulberry32::new(409);
        for _ in 0..56 {
            self.ground_sparkles.push(GroundSparkle {
                u: r.next_f32(),
                depth_frac: 0.08 + r.next_f32() * 0.52,
                phase: r.next_f32() * TAU,
                freq: 1.4 + r.next_f32() * 2.6,
                size: 0.9 + r.next_f32() * 1.4,
            });
        }
    }

    pub fn seed_initial_snow(&mut self) {
        let base_max = (10.0 + 18.0 * self.p.depth) * (self.h / 900.0).min(1.15);
        let denom = (self.ground_n.saturating_sub(1)).max(1) as f32;
        for i in 0..self.ground_n {
            let u = i as f32 / denom;
            let drift = 0.52
                + 0.26 * (u * 6.4 + 0.7).sin()
                + 0.14 * (u * 15.2 + 2.1).sin()
                + 0.08 * (u * 27.0).cos();
            self.ground_h[i] = 3.0f32.max(base_max * clamp_f32(drift, 0.25, 1.0));
        }
        let mode = self.p.mode;
        let acc = self.p.accumulate;
        let adap = self.p.adaptive;
        let wins = self.p.windows;
        let depth = self.p.depth;
        for s in &mut self.surfaces {
            Self::seed_surface_cap_inner(s, mode, acc, adap, depth);
        }
        for w in &mut self.win_surfaces {
            Self::seed_window_cap_inner(w, mode, acc, wins, depth);
        }
    }

    fn seed_surface_cap_inner(s: &mut IconSurface, mode: i32, acc: i32, adap: i32, depth: f32) {
        if mode != 0 || acc < 2 || adap == 0 {
            return;
        }
        let cap_max = 4.5 + 6.5 * depth;
        let denom = (s.cap_n.saturating_sub(1)).max(1) as f32;
        for i in 0..s.cap_n {
            let u = i as f32 / denom;
            let env = (u * PI).sin().max(0.0).powf(0.58);
            let lump = 0.82 + 0.18 * (u * 9.3 + s.id as f32 * 1.7).sin();
            s.cap_h[i] = cap_max * env * lump;
        }
    }

    fn seed_window_cap_inner(w: &mut WindowSurface, mode: i32, acc: i32, wins: i32, depth: f32) {
        if mode != 0 || acc == 0 || wins == 0 {
            return;
        }
        let cap_max = 4.0 + 5.5 * depth;
        let denom = (w.cap_n.saturating_sub(1)).max(1) as f32;
        let hwnd_mod = (w.hwnd.rem_euclid(17)) as f32;
        for i in 0..w.cap_n {
            let u = i as f32 / denom;
            let env = Self::window_edge_env(u, w.w);
            let wave = 0.72 + 0.18 * (u * 7.5 + hwnd_mod).sin() + 0.10 * (u * 19.0).cos();
            w.cap_h[i] = cap_max * env * wave;
        }
    }

    pub fn window_edge_env(u: f32, width: f32) -> f32 {
        let pad = 0.18f32.min(16.0 / width.max(60.0));
        let left = smoothstep_f32(0.0, pad, u);
        let right = smoothstep_f32(0.0, pad, 1.0 - u);
        (left * right).max(0.0).powf(0.72)
    }

    pub fn is_occluded_by_higher_window(&self, x: f32, y: f32, max_idx: usize) -> bool {
        let lim = max_idx.min(self.win_surfaces.len());
        for i in 0..lim {
            let w = &self.win_surfaces[i];
            if x >= w.x && x <= w.x + w.w && y >= w.y && y <= w.y + w.h {
                return true;
            }
        }
        false
    }

    fn seed_initial_heaps(&mut self) {
        if self.p.accumulate == 0 {
            return;
        }
        let mode = self.p.mode;
        let ground_count = (36.0 * self.p.depth).round() as usize;
        for _ in 0..ground_count {
            let x = self.rand() * self.w;
            let y = self.h - 4.0 - self.rand() * (10.0 + 14.0 * self.p.depth);
            let item = self.make_heap_item(mode, x, y, 0, 0.0, 0.0, false);
            self.heaps.push(item);
        }
        if self.p.accumulate >= 2 && self.p.adaptive != 0 {
            let surf_snap: Vec<(u32, f32, f32, f32)> =
                self.surfaces.iter().map(|s| (s.id, s.x, s.y, s.w)).collect();
            for (sid, sx, sy, sw) in surf_snap {
                let cnt = ((sw / 14.0).round() as usize).clamp(3, 7);
                for _ in 0..cnt {
                    let ox = sw * (0.14 + 0.72 * self.rand());
                    let oy = -1.0 - self.rand() * 4.5;
                    let item = self.make_heap_item(mode, sx + ox, sy + oy, sid, ox, oy, false);
                    self.heaps.push(item);
                }
            }
        }
        if self.p.windows != 0 {
            let win_snap: Vec<(u32, f32, f32, f32)> =
                self.win_surfaces.iter().map(|w| (w.id, w.x, w.y, w.w)).collect();
            for (wid, wx, wy, ww) in win_snap {
                let cnt = ((ww / 32.0).round() as usize).clamp(6, 28);
                for _ in 0..cnt {
                    let ox = 12.0 + (ww - 24.0) * self.rand();
                    let oy = -1.5 - self.rand() * 5.5;
                    let item = self.make_heap_item(mode, wx + ox, wy + oy, wid, ox, oy, true);
                    self.heaps.push(item);
                }
            }
        }
    }

    fn seed_initial_rain_beads(&mut self) {
        let cnt = (28.0 * self.get_quality_factor()).round() as usize;
        for _ in 0..cnt {
            let x = self.rand() * self.w;
            let y = self.rand() * self.h * 0.85;
            let r = 1.4 + self.rand() * 2.8;
            let stuck = self.rand() * 3.5;
            let mass = 0.4 + self.rand() * 0.6;
            let wobble = self.rand() * TAU;
            self.rain_beads.push(RainBead {
                x,
                y,
                r,
                vy: 0.0,
                vx: 0.0,
                stuck,
                surf_id: 0,
                is_window: false,
                hanging: false,
                mass,
                wobble,
            });
        }
    }

    fn make_heap_item(
        &mut self,
        mode: i32,
        x: f32,
        y: f32,
        surf_id: u32,
        ox: f32,
        oy: f32,
        is_window: bool,
    ) -> HeapItem {
        let is_leaf = mode == 2;
        let rot = (self.rand() - 0.5) * if is_leaf { 2.4 } else { TAU };
        let scale = if is_leaf {
            0.52 + self.rand() * 0.42
        } else {
            0.55 + self.rand() * 0.42
        };
        let squash = if is_leaf {
            0.45 + self.rand() * 0.38
        } else {
            0.52 + self.rand() * 0.36
        };
        let species = if is_leaf { (self.rand() * 3.0) as usize } else { 0 };
        let color_idx = if is_leaf {
            (self.rand() * AUTUMN_COLORS.len() as f32) as usize
        } else {
            (self.rand() * BLOSSOM_COLORS.len() as f32) as usize
        };
        let age = self.rand() * 6.0;
        HeapItem {
            mode,
            x,
            y,
            surf_id,
            is_window,
            ox,
            oy,
            rot,
            scale,
            squash,
            species,
            color_idx,
            age,
            alpha: 0.92,
        }
    }

    // =========================================================================
    // Icon & Open Window Surface Tracking + Shedding Physics
    // =========================================================================
    pub fn set_icons(&mut self, list: &[Rect]) {
        let prev = std::mem::take(&mut self.surfaces);
        let mut used_prev = vec![false; prev.len()];
        let mut next = Vec::new();

        if self.p.adaptive != 0 {
            for ic in list {
                if ic.w < 12.0 || ic.h < 12.0 {
                    continue;
                }
                let mut best_idx: Option<usize> = None;
                let mut best_d2 = 160.0 * 160.0;
                for (pi, p) in prev.iter().enumerate() {
                    if used_prev[pi] {
                        continue;
                    }
                    let dx = p.x - ic.x;
                    let dy = p.y - ic.y;
                    let d2 = dx * dx + dy * dy;
                    if d2 < best_d2 {
                        best_d2 = d2;
                        best_idx = Some(pi);
                    }
                }
                if let Some(bi) = best_idx {
                    used_prev[bi] = true;
                    let mut best = prev[bi].clone();
                    let dx = ic.x - best.x;
                    let dy = ic.y - best.y;
                    let moved = dx.hypot(dy);
                    if moved > 5.0 {
                        let factor = if moved > 24.0 { 1.0 } else { moved / 24.0 };
                        self.shed_surface_generic(
                            best.id,
                            false,
                            best.x,
                            best.y,
                            best.w,
                            best.cap_n,
                            &mut best.cap_h,
                            dx,
                            dy,
                            factor,
                            false,
                        );
                    }
                    best.vx = dx;
                    best.vy = dy;
                    best.x = ic.x;
                    best.y = ic.y;
                    if (best.w - ic.w).abs() > 6.0 {
                        let new_n = 8usize.max((ic.w / 4.0).ceil() as usize + 1);
                        let mut new_h = vec![0.0; new_n];
                        for i in 0..new_n {
                            let oi = (best.cap_n - 1).min(
                                ((i as f32 / (new_n - 1).max(1) as f32) * (best.cap_n - 1) as f32)
                                    .round() as usize,
                            );
                            new_h[i] = best.cap_h[oi];
                        }
                        best.cap_n = new_n;
                        best.cap_h = new_h;
                    }
                    best.w = ic.w;
                    best.h = ic.h;
                    next.push(best);
                } else {
                    let cap_n = 8usize.max((ic.w / 4.0).ceil() as usize + 1);
                    let id = self.next_surf_id;
                    self.next_surf_id += 1;
                    let mut s = IconSurface {
                        id,
                        x: ic.x,
                        y: ic.y,
                        w: ic.w,
                        h: ic.h,
                        vx: 0.0,
                        vy: 0.0,
                        cap_n,
                        cap_h: vec![0.0; cap_n],
                        wet: 0.0,
                    };
                    if self.sim_time < 1.2 {
                        Self::seed_surface_cap_inner(
                            &mut s,
                            self.p.mode,
                            self.p.accumulate,
                            self.p.adaptive,
                            self.p.depth,
                        );
                    }
                    next.push(s);
                }
            }
        }

        for (pi, mut p) in prev.into_iter().enumerate() {
            if !used_prev[pi] {
                self.shed_surface_generic(
                    p.id,
                    false,
                    p.x,
                    p.y,
                    p.w,
                    p.cap_n,
                    &mut p.cap_h,
                    0.0,
                    12.0,
                    1.0,
                    true,
                );
            }
        }

        self.surfaces = next;
    }

    pub fn set_windows(&mut self, list: &[WindowInput]) {
        let prev = std::mem::take(&mut self.win_surfaces);
        let mut used_prev = vec![false; prev.len()];
        let mut next = Vec::new();

        for (idx, w) in list.iter().enumerate() {
            if w.w < 60.0 || w.h < 40.0 {
                continue;
            }
            let found_idx = prev.iter().position(|p| p.hwnd == w.id);
            if let Some(pi) = found_idx {
                used_prev[pi] = true;
                let mut existing = prev[pi].clone();
                let dx = w.x - existing.x;
                let dy = w.y - existing.y;
                let moved = dx.hypot(dy);
                let shook = (existing.vx * dx < -24.0) || (existing.vy * dy < -24.0);
                if dy > 3.5 || moved > 9.0 || shook {
                    let shed_factor = if shook {
                        0.92
                    } else if dy > 4.0 {
                        clamp_f32(0.45 + dy / 22.0, 0.45, 0.96)
                    } else {
                        clamp_f32((moved - 6.0) / 22.0, 0.25, 0.92)
                    };
                    let full_drop = shed_factor > 0.88;
                    self.shed_surface_generic(
                        existing.id,
                        true,
                        existing.x,
                        existing.y,
                        existing.w,
                        existing.cap_n,
                        &mut existing.cap_h,
                        dx,
                        dy,
                        shed_factor,
                        full_drop,
                    );
                } else if moved > 1.5 && self.p.mode == 0 {
                    let shift = (-dx * 0.08).round().clamp(-2.0, 2.0) as isize;
                    if shift != 0 {
                        let n = existing.cap_n;
                        if shift > 0 {
                            let s = shift as usize;
                            for i in (s..n).rev() {
                                existing.cap_h[i] = existing.cap_h[i - s] * 0.96;
                            }
                            for i in 0..s.min(n) {
                                existing.cap_h[i] *= 0.6;
                            }
                        } else {
                            let s_abs = (-shift) as usize;
                            for i in 0..n.saturating_sub(s_abs) {
                                existing.cap_h[i] = existing.cap_h[i + s_abs] * 0.96;
                            }
                            for i in n.saturating_sub(s_abs)..n {
                                existing.cap_h[i] *= 0.6;
                            }
                        }
                    }
                }
                existing.vx = dx;
                existing.vy = dy;
                existing.x = w.x;
                existing.y = w.y;
                if moved > 0.2 {
                    for h in &mut self.heaps {
                        if h.is_window && h.surf_id == existing.id {
                            h.x = existing.x + h.ox;
                            h.y = existing.y + h.oy;
                        }
                    }
                    for b in &mut self.rain_beads {
                        if b.is_window && b.surf_id == existing.id {
                            b.x += dx;
                            b.y += dy;
                        }
                    }
                }
                if (existing.w - w.w).abs() > 8.0 {
                    let new_n = ((w.w / 6.0).ceil() as usize + 1).clamp(20, 220);
                    let mut new_h = vec![0.0; new_n];
                    for i in 0..new_n {
                        let oi = (existing.cap_n - 1).min(
                            ((i as f32 / (new_n - 1).max(1) as f32)
                                * (existing.cap_n - 1) as f32)
                                .round() as usize,
                        );
                        new_h[i] = existing.cap_h[oi];
                    }
                    existing.cap_n = new_n;
                    existing.cap_h = new_h;
                }
                existing.w = w.w;
                existing.h = w.h;
                existing.z_order = idx;
                next.push(existing);
            } else {
                let cap_n = ((w.w / 6.0).ceil() as usize + 1).clamp(20, 220);
                let id = self.next_surf_id;
                self.next_surf_id += 1;
                let mut surf = WindowSurface {
                    id,
                    hwnd: w.id,
                    x: w.x,
                    y: w.y,
                    w: w.w,
                    h: w.h,
                    vx: 0.0,
                    vy: 0.0,
                    cap_n,
                    cap_h: vec![0.0; cap_n],
                    wet: 0.0,
                    z_order: idx,
                };
                if self.p.mode == 0 && self.p.accumulate > 0 && self.p.windows != 0 {
                    Self::seed_window_cap_inner(
                        &mut surf,
                        self.p.mode,
                        self.p.accumulate,
                        self.p.windows,
                        self.p.depth,
                    );
                }
                next.push(surf);
            }
        }

        for (pi, mut p) in prev.into_iter().enumerate() {
            if !used_prev[pi] {
                self.shed_surface_generic(
                    p.id,
                    true,
                    p.x,
                    p.y,
                    p.w,
                    p.cap_n,
                    &mut p.cap_h,
                    0.0,
                    16.0,
                    1.0,
                    true,
                );
            }
        }

        self.win_surfaces = next;
    }

    #[allow(clippy::too_many_arguments)]
    fn shed_surface_generic(
        &mut self,
        surf_id: u32,
        is_window: bool,
        sx: f32,
        sy: f32,
        sw: f32,
        cap_n: usize,
        cap_h: &mut [f32],
        dx: f32,
        dy: f32,
        factor: f32,
        full_drop: bool,
    ) {
        if self.p.mode == 0 {
            let step = if is_window {
                1usize.max(cap_n / 16)
            } else {
                1usize.max(cap_n / 7)
            };
            let denom = (cap_n.saturating_sub(1)).max(1) as f32;
            let mut i = 0;
            while i < cap_n {
                let h = cap_h[i];
                if h >= 1.2 {
                    let drop_amt = if full_drop {
                        h
                    } else {
                        h * clamp_f32(factor * 0.72, 0.25, 0.92)
                    };
                    cap_h[i] = (h - drop_amt).max(0.0);
                    let u = i as f32 / denom;
                    let lx = sx + u * sw;
                    let ly = sy - h * 0.45;
                    if self.lumps.len() < 180 {
                        let r1 = self.rand();
                        let r2 = self.rand();
                        let r3 = self.rand();
                        let r4 = self.rand();
                        self.lumps.push(SnowLump {
                            x: lx,
                            y: ly,
                            vx: clamp_f32(dx * 0.35, -90.0, 90.0) + (r1 - 0.5) * 48.0,
                            vy: clamp_f32(dy * 0.22, -30.0, 45.0) - r2 * 32.0,
                            r: clamp_f32(2.4 + drop_amt * 0.42, 2.2, 9.2),
                            mass: drop_amt,
                            rot: r3 * TAU,
                            vrot: (r4 - 0.5) * 5.0,
                            src_id: if full_drop { 0 } else { surf_id },
                            src_is_window: is_window,
                        });
                    }
                }
                i += step;
            }
            if full_drop {
                cap_h.fill(0.0);
            }
        } else if self.p.mode == 1 {
            let n_beads = self.rain_beads.len();
            for bi in 0..n_beads {
                if self.rain_beads[bi].surf_id == surf_id
                    && self.rain_beads[bi].is_window == is_window
                {
                    let r1 = self.rand();
                    let r2 = self.rand();
                    let b = &mut self.rain_beads[bi];
                    b.surf_id = 0;
                    b.is_window = false;
                    b.hanging = false;
                    b.stuck = 0.0;
                    b.vx += clamp_f32(dx * 0.6, -120.0, 120.0) + (r1 - 0.5) * 35.0;
                    b.vy = b.vy.max(60.0 + r2 * 80.0);
                }
            }
        } else {
            let old_heaps = std::mem::take(&mut self.heaps);
            let mut kept = Vec::with_capacity(old_heaps.len());
            for mut h in old_heaps {
                if h.surf_id == surf_id && h.is_window == is_window {
                    if full_drop || self.rand() < factor * 0.85 {
                        let vy = -30.0 - self.rand() * 60.0;
                        self.spawn_flake_from_heap(&h, clamp_f32(dx * 1.4, -140.0, 140.0), vy);
                        continue;
                    } else {
                        h.x = sx + h.ox;
                        h.y = sy + h.oy;
                    }
                }
                kept.push(h);
            }
            self.heaps = kept;
        }
    }

    fn spawn_flake_from_heap(&mut self, h: &HeapItem, vx: f32, vy: f32) {
        if self.flakes.len() > 520 {
            return;
        }
        let z = 0.72 + self.rand() * 0.26;
        let vx_jit = (self.rand() - 0.5) * 55.0;
        let size = if h.mode == 2 {
            16.0 + self.rand() * 11.0
        } else {
            10.0 + self.rand() * 8.0
        };
        let vrot = (self.rand() - 0.5) * 3.2;
        let pitch = self.rand() * TAU;
        let vpitch = 1.8 + self.rand() * 3.2;
        let sway_phase = self.rand() * TAU;
        let sway_freq = 1.1 + self.rand() * 1.2;
        let sway_amp = 24.0 + self.rand() * 28.0;
        self.flakes.push(Flake {
            x: h.x,
            y: h.y - 2.0,
            z,
            vx: vx + vx_jit,
            vy,
            size,
            rot: h.rot,
            vrot,
            pitch,
            vpitch,
            sway_phase,
            sway_freq,
            sway_amp,
            species: h.species,
            color_idx: h.color_idx,
            kind: 0,
            whole_flower: false,
            alpha: 0.95,
        });
    }

    // =========================================================================
    // Pointer / Cursor Interaction (Sweeping snow, tossing leaves, smearing rain)
    // =========================================================================
    pub fn on_pointer_move(&mut self, x: f32, y: f32, down: Option<bool>) {
        let now = self.sim_time;
        if !self.ptr.active {
            self.ptr.px = x;
            self.ptr.py = y;
        } else {
            self.ptr.px = self.ptr.x;
            self.ptr.py = self.ptr.y;
        }
        self.ptr.x = x;
        self.ptr.y = y;
        let raw_dt = now - self.ptr.last_move;
        let dt_base = if raw_dt.abs() < 1e-6 { 0.016 } else { raw_dt };
        let dt = dt_base.clamp(0.012, 0.08);
        self.ptr.vx = lerp_f32(
            self.ptr.vx,
            clamp_f32((self.ptr.x - self.ptr.px) / dt, -1400.0, 1400.0),
            0.55,
        );
        self.ptr.vy = lerp_f32(
            self.ptr.vy,
            clamp_f32((self.ptr.y - self.ptr.py) / dt, -1400.0, 1400.0),
            0.55,
        );
        self.ptr.speed = self.ptr.vx.hypot(self.ptr.vy);
        self.ptr.active = true;
        if let Some(d) = down {
            self.ptr.down = d;
        }
        self.ptr.last_move = now;

        if self.p.cursor > 0 && self.ptr.speed > 18.0 {
            self.interact_at_pointer();
        }
    }

    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        self.on_pointer_move(x, y, Some(true));
        if self.p.cursor > 0 {
            self.ptr.speed = self.ptr.speed.max(260.0);
            self.interact_at_pointer();
        }
    }

    pub fn on_pointer_up(&mut self) {
        self.ptr.down = false;
    }

    pub fn on_pointer_leave(&mut self) {
        self.ptr.active = false;
        self.ptr.down = false;
        self.ptr.vx = 0.0;
        self.ptr.vy = 0.0;
        self.ptr.speed = 0.0;
    }

    fn interact_at_pointer(&mut self) {
        let px = self.ptr.x;
        let py = self.ptr.y;
        let pvx = self.ptr.vx;
        let pvy = self.ptr.vy;
        let pspeed = self.ptr.speed;
        let pdown = self.ptr.down;

        let strength = if self.p.cursor == 2 { 1.0 } else { 0.55 };
        let rad = (if pdown { 52.0 } else { 38.0 }) * (0.75 + 0.35 * strength);
        let speed_norm = clamp_f32(pspeed / 420.0, 0.18, 2.2) * strength;

        if self.p.mode == 0 {
            // 1. Sweep ground heightfield
            if self.p.accumulate > 0 && py > self.h - (110.0 + rad) {
                let i_min = ((px - rad) / self.cell_w).floor().max(0.0) as usize;
                let i_max = (((px + rad) / self.cell_w).ceil().max(0.0) as usize)
                    .min(self.ground_n.saturating_sub(1));
                let dir: isize = if pvx >= 0.0 { 1 } else { -1 };
                for i in i_min..=i_max {
                    let cx = i as f32 * self.cell_w;
                    let cy = self.h - self.ground_h[i];
                    let dist = (cx - px).hypot(cy - py);
                    if dist < rad && self.ground_h[i] > 0.5 {
                        let falloff = 1.0 - dist / rad;
                        let cut = self.ground_h[i].min(falloff * 5.2 * speed_norm);
                        self.ground_h[i] -= cut;
                        let offset = dir * (2.0 + falloff * 4.0).round() as isize;
                        let push_idx = (i as isize + offset)
                            .clamp(0, self.ground_n as isize - 1)
                            as usize;
                        self.ground_h[push_idx] += cut * 0.42;
                        if cut > 0.5
                            && self.rand() < 0.45 * strength
                            && self.powders.len() < 220
                        {
                            let r1 = self.rand();
                            let r2 = self.rand();
                            let r3 = self.rand();
                            self.spawn_powder(
                                cx,
                                cy - 2.0,
                                pvx * 0.24 + (r1 - 0.5) * 90.0,
                                -50.0 - r2 * 110.0 * speed_norm,
                                1.4 + r3 * 2.4,
                            );
                        }
                    }
                }
            }

            // 2. Sweep icon snow caps
            if self.p.accumulate >= 2 && self.p.adaptive != 0 {
                let n_surf = self.surfaces.len();
                for si in 0..n_surf {
                    let (sx, sy, sw, cap_n, sid) = {
                        let s = &self.surfaces[si];
                        (s.x, s.y, s.w, s.cap_n, s.id)
                    };
                    if px + rad < sx || px - rad > sx + sw || py + rad < sy - 44.0 || py - rad > sy + 24.0 {
                        continue;
                    }
                    let cell_w = sw / (cap_n.saturating_sub(1)).max(1) as f32;
                    let dir: isize = if pvx >= 0.0 { 1 } else { -1 };
                    for i in 0..cap_n {
                        let cx = sx + i as f32 * cell_w;
                        let cy = sy - self.surfaces[si].cap_h[i];
                        let dist = (cx - px).hypot(cy - py);
                        if dist < rad * 0.9 && self.surfaces[si].cap_h[i] > 0.4 {
                            let falloff = 1.0 - dist / (rad * 0.9);
                            let cut = self.surfaces[si].cap_h[i].min(falloff * 5.0 * speed_norm);
                            self.surfaces[si].cap_h[i] -= cut;
                            let push_idx = i as isize + dir * 2;
                            if push_idx >= 0 && (push_idx as usize) < cap_n {
                                self.surfaces[si].cap_h[push_idx as usize] += cut * 0.32;
                            } else if cut > 0.8 && self.lumps.len() < 180 {
                                let r1 = self.rand();
                                let r2 = self.rand();
                                let r3 = self.rand();
                                self.lumps.push(SnowLump {
                                    x: cx,
                                    y: cy,
                                    vx: pvx * 0.25 + dir as f32 * 40.0,
                                    vy: -20.0 - r1 * 40.0,
                                    r: clamp_f32(2.0 + cut * 0.5, 2.0, 5.8),
                                    mass: cut,
                                    rot: r2 * TAU,
                                    vrot: (r3 - 0.5) * 6.0,
                                    src_id: sid,
                                    src_is_window: false,
                                });
                            }
                            if cut > 0.4 && self.rand() < 0.55 && self.powders.len() < 240 {
                                let r1 = self.rand();
                                let r2 = self.rand();
                                let r3 = self.rand();
                                self.spawn_powder(
                                    cx,
                                    cy - 1.0,
                                    pvx * 0.28 + (r1 - 0.5) * 80.0,
                                    -40.0 - r2 * 95.0 * speed_norm,
                                    1.3 + r3 * 2.2,
                                );
                            }
                        }
                    }
                }
            }

            // 3. Sweep open window snow caps
            if self.p.accumulate > 0 && self.p.windows != 0 {
                let n_win = self.win_surfaces.len();
                for wi in 0..n_win {
                    let (wx, wy, ww, cap_n, wid) = {
                        let w = &self.win_surfaces[wi];
                        (w.x, w.y, w.w, w.cap_n, w.id)
                    };
                    if px + rad < wx || px - rad > wx + ww || py + rad < wy - 44.0 || py - rad > wy + 24.0 {
                        continue;
                    }
                    let cell_w = ww / (cap_n.saturating_sub(1)).max(1) as f32;
                    let dir: isize = if pvx >= 0.0 { 1 } else { -1 };
                    for i in 0..cap_n {
                        let cx = wx + i as f32 * cell_w;
                        let cy = wy - self.win_surfaces[wi].cap_h[i];
                        let dist = (cx - px).hypot(cy - py);
                        if dist < rad * 0.9 && self.win_surfaces[wi].cap_h[i] > 0.4 {
                            let falloff = 1.0 - dist / (rad * 0.9);
                            let cut = self.win_surfaces[wi].cap_h[i].min(falloff * 5.0 * speed_norm);
                            self.win_surfaces[wi].cap_h[i] -= cut;
                            let push_idx = i as isize + dir * 2;
                            if push_idx >= 0 && (push_idx as usize) < cap_n {
                                self.win_surfaces[wi].cap_h[push_idx as usize] += cut * 0.32;
                            } else if cut > 0.8 && self.lumps.len() < 180 {
                                let r1 = self.rand();
                                let r2 = self.rand();
                                let r3 = self.rand();
                                self.lumps.push(SnowLump {
                                    x: cx,
                                    y: cy,
                                    vx: pvx * 0.25 + dir as f32 * 40.0,
                                    vy: -20.0 - r1 * 40.0,
                                    r: clamp_f32(2.0 + cut * 0.5, 2.0, 5.8),
                                    mass: cut,
                                    rot: r2 * TAU,
                                    vrot: (r3 - 0.5) * 6.0,
                                    src_id: wid,
                                    src_is_window: true,
                                });
                            }
                            if cut > 0.4 && self.rand() < 0.55 && self.powders.len() < 240 {
                                let r1 = self.rand();
                                let r2 = self.rand();
                                let r3 = self.rand();
                                self.spawn_powder(
                                    cx,
                                    cy - 1.0,
                                    pvx * 0.28 + (r1 - 0.5) * 80.0,
                                    -40.0 - r2 * 95.0 * speed_norm,
                                    1.3 + r3 * 2.2,
                                );
                            }
                        }
                    }
                }
            }
        } else if self.p.mode == 1 {
            let n_beads = self.rain_beads.len();
            for bi in 0..n_beads {
                let (bx, by) = (self.rain_beads[bi].x, self.rain_beads[bi].y);
                let dist = (bx - px).hypot(by - py);
                if dist < rad * 0.95 {
                    let f = (1.0 - dist / (rad * 0.95)) * speed_norm;
                    let r1 = self.rand();
                    let r2 = self.rand();
                    let b = &mut self.rain_beads[bi];
                    b.stuck = 0.0;
                    b.hanging = false;
                    b.vx += pvx * 0.38 * f + (r1 - 0.5) * 50.0;
                    b.vy += pvy * 0.35 * f + 30.0 * f;
                    b.r = 1.0f32.max(b.r * (1.0 - 0.14 * f));
                    let br = b.r;
                    if r2 < 0.35 && self.rain_trails.len() < 240 {
                        self.rain_trails.push(RainTrail {
                            x: bx,
                            y: by,
                            r: 0.7f32.max(br * 0.45),
                            a: 0.48,
                        });
                    }
                }
            }
        } else {
            let old_heaps = std::mem::take(&mut self.heaps);
            let mut kept = Vec::with_capacity(old_heaps.len());
            for h in old_heaps {
                let dist = (h.x - px).hypot(h.y - py);
                if dist < rad * 1.15 && self.rand() < (1.0 - dist / (rad * 1.15)) * 0.92 * strength
                {
                    let away_x = (h.x - px) / if dist < 1e-4 { 1.0 } else { dist };
                    let r1 = self.rand();
                    let r2 = self.rand();
                    self.spawn_flake_from_heap(
                        &h,
                        pvx * 0.32 + away_x * 95.0 * speed_norm + (r1 - 0.5) * 50.0,
                        -70.0 - r2 * 110.0 * speed_norm,
                    );
                } else {
                    kept.push(h);
                }
            }
            self.heaps = kept;
        }
    }

    pub fn spawn_powder(&mut self, x: f32, y: f32, vx: f32, vy: f32, r: f32) {
        let life = 0.45 + self.rand() * 0.75;
        let alpha = 0.55 + self.rand() * 0.35;
        self.powders.push(Powder {
            x,
            y,
            vx,
            vy,
            r,
            life,
            max_life: life,
            alpha,
        });
    }

    // =========================================================================
    // Particle Pool Management
    // =========================================================================
    pub fn target_count(&self) -> usize {
        let q = self.get_quality_factor();
        let area_scale = clamp_f32((self.w * self.h) / (1920.0 * 1080.0), 0.45, 1.35);
        let inten = clamp_f32(self.p.intensity, 0.1, 1.6);
        let base = match self.p.mode {
            0 => 380.0,
            1 => 240.0,
            2 => 84.0,
            _ => 145.0,
        };
        (base * q * area_scale * inten).round() as usize
    }

    pub fn reconcile_pools(&mut self, scatter_y: bool) {
        let target = self.target_count();
        if self.p.mode == 1 {
            while self.rain_streaks.len() < target {
                let s = self.make_rain_streak(scatter_y);
                self.rain_streaks.push(s);
            }
            if self.rain_streaks.len() > target {
                self.rain_streaks.truncate(target);
            }
        } else {
            while self.flakes.len() < target {
                let f = self.make_flake(scatter_y);
                self.flakes.push(f);
            }
            if self.flakes.len() > target {
                self.flakes.truncate(target);
            }
        }
    }

    fn make_flake(&mut self, scatter_y: bool) -> Flake {
        let mode = self.p.mode;
        let z = self.rand().powf(0.85) * 0.85 + 0.15;
        let x = self.rand() * (self.w + 240.0) - 120.0;
        let y = if scatter_y {
            self.rand() * self.h
        } else {
            -20.0 - self.rand() * 90.0
        };

        if mode == 0 {
            let mut kind = 0usize;
            let mut size = lerp_f32(2.2, 8.2, z);
            if z > 0.88 && self.rand() < 0.34 {
                kind = 2;
                size = lerp_f32(14.0, 28.0, self.rand());
            } else if z > 0.56 && self.rand() < 0.24 {
                kind = 1;
                size = lerp_f32(6.5, 13.5, self.rand());
            }
            let vx = (self.rand() - 0.5) * 14.0;
            let vy = lerp_f32(24.0, 92.0, z) * (0.82 + self.rand() * 0.36);
            let rot = self.rand() * TAU;
            let vrot = (self.rand() - 0.5) * if kind == 1 { 1.4 } else { 0.6 };
            let sway_phase = self.rand() * TAU;
            let sway_freq = 0.55 + self.rand() * 1.15;
            let sway_amp = lerp_f32(10.0, 36.0, z);
            let species = (self.rand() * 4.0) as usize;
            let color_idx = (self.rand() * 3.0) as usize;
            let alpha = if kind == 2 {
                lerp_f32(0.24, 0.46, self.rand())
            } else {
                lerp_f32(0.32, 0.94, z)
            };
            Flake {
                x,
                y,
                z,
                vx,
                vy,
                size,
                rot,
                vrot,
                pitch: 0.0,
                vpitch: 0.0,
                sway_phase,
                sway_freq,
                sway_amp,
                species,
                color_idx,
                kind,
                whole_flower: false,
                alpha,
            }
        } else if mode == 2 {
            let vx = (self.rand() - 0.5) * 24.0;
            let vy = lerp_f32(34.0, 78.0, z) * (0.8 + self.rand() * 0.4);
            let size = lerp_f32(14.0, 30.0, z);
            let rot = self.rand() * TAU;
            let vrot = (self.rand() - 0.5) * 2.2;
            let pitch = self.rand() * TAU;
            let vpitch = 1.4 + self.rand() * 2.6;
            let sway_phase = self.rand() * TAU;
            let sway_freq = 0.75 + self.rand() * 0.85;
            let sway_amp = lerp_f32(26.0, 68.0, z);
            let species = (self.rand() * 3.0) as usize;
            let color_idx = (self.rand() * AUTUMN_COLORS.len() as f32) as usize;
            Flake {
                x,
                y,
                z,
                vx,
                vy,
                size,
                rot,
                vrot,
                pitch,
                vpitch,
                sway_phase,
                sway_freq,
                sway_amp,
                species,
                color_idx,
                kind: 0,
                whole_flower: false,
                alpha: lerp_f32(0.55, 0.98, z),
            }
        } else {
            let whole = z > 0.45 && self.rand() < 0.08;
            let vx = (self.rand() - 0.5) * 20.0;
            let vy = lerp_f32(26.0, 64.0, z)
                * if whole {
                    1.12
                } else {
                    0.8 + self.rand() * 0.4
                };
            let size = if whole {
                lerp_f32(18.0, 28.0, z)
            } else {
                lerp_f32(8.0, 17.0, z)
            };
            let rot = self.rand() * TAU;
            let vrot = (self.rand() - 0.5) * if whole { 3.2 } else { 2.4 };
            let pitch = self.rand() * TAU;
            let vpitch = 1.8 + self.rand() * 3.4;
            let sway_phase = self.rand() * TAU;
            let sway_freq = 0.9 + self.rand() * 1.1;
            let sway_amp = lerp_f32(22.0, 58.0, z);
            let color_idx = (self.rand() * BLOSSOM_COLORS.len() as f32) as usize;
            Flake {
                x,
                y,
                z,
                vx,
                vy,
                size,
                rot,
                vrot,
                pitch,
                vpitch,
                sway_phase,
                sway_freq,
                sway_amp,
                species: 0,
                color_idx,
                kind: 0,
                whole_flower: whole,
                alpha: lerp_f32(0.52, 0.96, z),
            }
        }
    }

    fn make_rain_streak(&mut self, scatter_y: bool) -> RainStreak {
        let z = self.rand() * 0.85 + 0.15;
        let x = self.rand() * (self.w + 300.0) - 150.0;
        let y = if scatter_y {
            self.rand() * self.h
        } else {
            -30.0 - self.rand() * 140.0
        };
        let speed = lerp_f32(460.0, 1080.0, z) * (0.85 + self.rand() * 0.3);
        RainStreak {
            x,
            y,
            z,
            speed,
            len: lerp_f32(14.0, 42.0, z),
            thick: lerp_f32(0.65, 1.55, z),
            alpha: lerp_f32(0.14, 0.52, z),
        }
    }

    // =========================================================================
    // Simulation Step (Snow, Rain, Autumn Leaves, Cherry Blossom)
    // =========================================================================
    pub fn step(&mut self, dt: f32) {
        self.sim_time += dt;
        if self.sim_time - self.ptr.last_move > 0.09 {
            let decay = 0.04f32.powf(dt);
            self.ptr.vx *= decay;
            self.ptr.vy *= decay;
            self.ptr.speed = self.ptr.vx.hypot(self.ptr.vy);
        }

        self.gust_timer -= dt;
        if self.gust_timer <= 0.0 {
            self.gust_timer = 2.4 + self.rand() * 4.8;
            let g_amp = self.p.gusts;
            self.gust_target = if g_amp > 0.02 {
                let sign_w = if self.p.wind.abs() < 1e-5 {
                    1.0
                } else {
                    self.p.wind.signum()
                };
                let dir_val = if self.rand() < 0.65 {
                    sign_w * (0.4 + self.rand() * 0.9)
                } else {
                    (self.rand() - 0.5) * 1.4
                };
                dir_val * g_amp
            } else {
                0.0
            };
        }
        self.gust_val = lerp_f32(self.gust_val, self.gust_target, clamp_f32(dt * 0.85, 0.0, 1.0));
        let wind_px = self.p.wind * 72.0 + self.gust_val * 115.0;

        if self.p.mode == 0 {
            self.step_snow(dt, wind_px);
        } else if self.p.mode == 1 {
            self.step_rain(dt, wind_px);
        } else {
            self.step_foliage(dt, wind_px);
        }

        for i in (0..self.powders.len()).rev() {
            self.powders[i].life -= dt;
            if self.powders[i].life <= 0.0 {
                self.powders.remove(i);
                continue;
            }
            let pt = &mut self.powders[i];
            pt.vx = lerp_f32(pt.vx, wind_px * 0.75, clamp_f32(dt * 2.2, 0.0, 1.0));
            pt.vy += 125.0 * dt;
            pt.x += pt.vx * dt;
            pt.y += pt.vy * dt;
        }
    }

    fn step_snow(&mut self, dt: f32, wind_px: f32) {
        let (w, h, t) = (self.w, self.h, self.sim_time);
        let max_ground = (22.0 + 54.0 * self.p.depth) * clamp_f32(h / 900.0, 0.55, 1.25);
        let max_cap = 9.0 + 14.0 * self.p.depth;
        let max_win_cap = 12.0 + 16.0 * self.p.depth;
        let acc_ground = self.p.accumulate >= 1;
        let acc_icons =
            self.p.accumulate >= 2 && self.p.adaptive != 0 && !self.surfaces.is_empty();
        let acc_wins =
            self.p.accumulate >= 1 && self.p.windows != 0 && !self.win_surfaces.is_empty();
        let ptr_on = self.p.cursor > 0 && self.ptr.active && self.ptr.speed > 20.0;
        let ptr_rad = if self.p.cursor == 2 { 78.0 } else { 52.0 };

        let n_flakes = self.flakes.len();
        for i in 0..n_flakes {
            let mut f = self.flakes[i];
            let turb = (t * f.sway_freq + f.sway_phase + f.y * 0.008).sin() * f.sway_amp;
            let target_vx = wind_px * (0.45 + 0.65 * f.z) + turb;
            f.vx = lerp_f32(f.vx, target_vx, clamp_f32(dt * 2.8, 0.0, 1.0));

            if ptr_on && f.z > 0.35 {
                let dx = f.x - self.ptr.x;
                let dy = f.y - self.ptr.y;
                let d2 = dx * dx + dy * dy;
                if d2 < ptr_rad * ptr_rad {
                    let d = d2.sqrt().max(1.0);
                    let push =
                        (1.0 - d / ptr_rad) * if self.p.cursor == 2 { 1.0 } else { 0.55 };
                    f.vx += (self.ptr.vx * 0.38 + (dx / d) * 110.0) * push * dt * 8.0;
                    f.vy += (self.ptr.vy * 0.25 + (dy / d) * 65.0) * push * dt * 5.0;
                }
            }

            let natural_vy = lerp_f32(24.0, 92.0, f.z);
            f.vy = lerp_f32(f.vy, natural_vy, clamp_f32(dt * 1.4, 0.0, 1.0));
            f.x += f.vx * dt;
            f.y += f.vy * dt;
            f.rot += f.vrot * dt;

            if f.x < -120.0 {
                f.x = w + 110.0;
            } else if f.x > w + 120.0 {
                f.x = -110.0;
            }

            let mut settled = false;
            if acc_wins && f.kind != 2 && f.z > 0.20 && f.vy > 0.0 {
                let n_wins = self.win_surfaces.len();
                for wi in 0..n_wins {
                    let (wx, wy, ww, cap_n) = {
                        let win = &self.win_surfaces[wi];
                        (win.x, win.y, win.w, win.cap_n)
                    };
                    if f.x < wx + 5.0 || f.x > wx + ww - 5.0 {
                        continue;
                    }
                    let u = (f.x - wx) / ww;
                    let ci = ((u * (cap_n - 1) as f32).round() as isize)
                        .clamp(0, cap_n as isize - 1) as usize;
                    let surf_y = wy - self.win_surfaces[wi].cap_h[ci];
                    if f.y >= surf_y - 3.0 && f.y <= wy + 10.0 {
                        if !self.is_occluded_by_higher_window(f.x, wy, wi) {
                            let env = Self::window_edge_env(u, ww);
                            if self.win_surfaces[wi].cap_h[ci] < max_win_cap * env {
                                let add =
                                    (1.55 + 1.85 * f.z) * (0.65 + 0.72 * self.p.depth);
                                Self::deposit_window_cap_inner(
                                    &mut self.win_surfaces[wi],
                                    ci,
                                    add,
                                    max_win_cap,
                                );
                            }
                            settled = true;
                            break;
                        }
                    }
                }
            }

            if !settled && acc_icons && f.kind != 2 && f.z > 0.24 && f.vy > 0.0 {
                let n_surfs = self.surfaces.len();
                let n_wins = self.win_surfaces.len();
                for si in 0..n_surfs {
                    let (sx, sy, sw, cap_n) = {
                        let s = &self.surfaces[si];
                        (s.x, s.y, s.w, s.cap_n)
                    };
                    if f.x < sx + 2.0 || f.x > sx + sw - 2.0 {
                        continue;
                    }
                    let u = (f.x - sx) / sw;
                    let ci = ((u * (cap_n - 1) as f32).round() as isize)
                        .clamp(0, cap_n as isize - 1) as usize;
                    let surf_y = sy - self.surfaces[si].cap_h[ci];
                    if f.y >= surf_y - 2.0 && f.y <= sy + 8.0 {
                        if acc_wins && self.is_occluded_by_higher_window(f.x, sy, n_wins) {
                            continue;
                        }
                        let env = (u * PI).sin().max(0.0).powf(0.55);
                        let limit = max_cap * env;
                        if self.surfaces[si].cap_h[ci] < limit {
                            let add = (1.05 + 1.35 * f.z) * (0.60 + 0.70 * self.p.depth);
                            Self::deposit_cap_inner(&mut self.surfaces[si], ci, add, max_cap);
                        }
                        settled = true;
                        break;
                    }
                }
            }

            if !settled {
                let gi = ((f.x / self.cell_w).round() as isize)
                    .clamp(0, self.ground_n as isize - 1) as usize;
                let ground_y = if acc_ground {
                    h - self.ground_h[gi]
                } else {
                    h + 20.0
                };
                if f.y >= ground_y {
                    if acc_ground && f.kind != 2 && f.z > 0.22 {
                        let add = (1.15 + 1.45 * f.z) * (0.60 + 0.72 * self.p.depth);
                        self.deposit_ground(gi, add, max_ground);
                    }
                    settled = true;
                }
            }

            if settled || f.y > h + 40.0 {
                self.flakes[i] = self.make_flake(false);
            } else {
                self.flakes[i] = f;
            }
        }

        // Step falling snow lumps
        for i in (0..self.lumps.len()).rev() {
            let mut l = self.lumps[i];
            l.vy += 680.0 * dt;
            l.vx = lerp_f32(l.vx, wind_px * 0.4, clamp_f32(dt * 1.2, 0.0, 1.0));
            l.x += l.vx * dt;
            l.y += l.vy * dt;
            l.rot += l.vrot * dt;

            let mut hit = false;
            if acc_wins && l.vy > 40.0 {
                let n_wins = self.win_surfaces.len();
                for wi in 0..n_wins {
                    let (wid, wx, wy, ww, cap_n) = {
                        let win = &self.win_surfaces[wi];
                        (win.id, win.x, win.y, win.w, win.cap_n)
                    };
                    if l.src_is_window && wid == l.src_id {
                        continue;
                    }
                    if l.x >= wx + 6.0 && l.x <= wx + ww - 6.0 && l.y >= wy - 8.0 && l.y <= wy + 12.0
                    {
                        if !self.is_occluded_by_higher_window(l.x, wy, wi) {
                            let ci = ((((l.x - wx) / ww) * (cap_n - 1) as f32).round() as isize)
                                .clamp(0, cap_n as isize - 1)
                                as usize;
                            Self::deposit_window_cap_inner(
                                &mut self.win_surfaces[wi],
                                ci,
                                l.mass * 0.65,
                                max_win_cap,
                            );
                            let burst_y = wy - self.win_surfaces[wi].cap_h[ci];
                            self.burst_powder(l.x, burst_y, 9usize.min((4.0 + l.r).round() as usize));
                            hit = true;
                            break;
                        }
                    }
                }
            }
            if !hit && acc_icons && l.vy > 40.0 {
                let n_surfs = self.surfaces.len();
                for si in 0..n_surfs {
                    let (sid, sx, sy, sw, cap_n) = {
                        let s = &self.surfaces[si];
                        (s.id, s.x, s.y, s.w, s.cap_n)
                    };
                    if !l.src_is_window && sid == l.src_id {
                        continue;
                    }
                    if l.x >= sx + 3.0 && l.x <= sx + sw - 3.0 && l.y >= sy - 6.0 && l.y <= sy + 10.0
                    {
                        let ci = ((((l.x - sx) / sw) * (cap_n - 1) as f32).round() as isize)
                            .clamp(0, cap_n as isize - 1) as usize;
                        Self::deposit_cap_inner(&mut self.surfaces[si], ci, l.mass * 0.55, max_cap);
                        let burst_y = sy - self.surfaces[si].cap_h[ci];
                        self.burst_powder(l.x, burst_y, 8usize.min((3.0 + l.r).round() as usize));
                        hit = true;
                        break;
                    }
                }
            }
            let gi = ((l.x / self.cell_w).round() as isize)
                .clamp(0, self.ground_n as isize - 1) as usize;
            let gy = h - self.ground_h[gi];
            if !hit && l.y >= gy {
                if acc_ground {
                    self.deposit_ground(gi, l.mass * 0.75, max_ground);
                }
                self.burst_powder(l.x, gy - 2.0, 10usize.min((4.0 + l.r).round() as usize));
                hit = true;
            }
            if hit || l.y > h + 40.0 {
                self.lumps.remove(i);
            } else {
                self.lumps[i] = l;
            }
        }

        let melt_rate = if self.p.melt > 0.01 {
            (0.016 + self.p.melt * 0.095) * dt
        } else {
            0.0
        };
        if acc_ground {
            Self::relax_array(&mut self.ground_h, 1.35, melt_rate, max_ground);
        }
        if acc_icons {
            for s in &mut self.surfaces {
                Self::relax_cap_inner(s, 1.05, melt_rate, max_cap);
            }
        }
        if acc_wins {
            let n_wins = self.win_surfaces.len();
            for wi in 0..n_wins {
                self.relax_window_cap(wi, 1.25, melt_rate, max_win_cap);
            }
        }

        if acc_ground
            && wind_px.abs() > 48.0
            && self.p.gusts > 0.15
            && self.rand() < clamp_f32(wind_px.abs() / 260.0, 0.08, 0.55)
            && self.powders.len() < 180
        {
            let gi = ((self.rand() * self.ground_n as f32) as usize).min(self.ground_n - 1);
            if self.ground_h[gi] > 5.0 {
                let r1 = self.rand();
                let r2 = self.rand();
                let r3 = self.rand();
                let py = h - self.ground_h[gi] - 1.0;
                self.spawn_powder(
                    gi as f32 * self.cell_w,
                    py,
                    wind_px * (0.9 + r1 * 0.5),
                    -18.0 - r2 * 34.0,
                    1.2 + r3 * 1.6,
                );
            }
        }
    }

    pub fn deposit_ground(&mut self, ci: usize, amount: f32, max_h: f32) {
        let n = self.ground_n as isize;
        let weights = [0.16f32, 0.34, 0.52, 0.34, 0.16];
        for k in -2..=2 {
            let idx = ci as isize + k;
            if idx >= 0 && idx < n {
                let u = idx as usize;
                self.ground_h[u] = max_h.min(self.ground_h[u] + amount * weights[(k + 2) as usize]);
            }
        }
    }

    fn deposit_cap_inner(s: &mut IconSurface, ci: usize, amount: f32, max_cap: f32) {
        let n = s.cap_n as isize;
        let denom = (s.cap_n.saturating_sub(1)).max(1) as f32;
        let weights = [0.24f32, 0.56, 0.24];
        for k in -1..=1 {
            let idx = ci as isize + k;
            if idx >= 0 && idx < n {
                let ui = idx as usize;
                let u = ui as f32 / denom;
                let env = (u * PI).sin().max(0.0).powf(0.55);
                s.cap_h[ui] = (max_cap * env).min(s.cap_h[ui] + amount * weights[(k + 1) as usize]);
            }
        }
    }

    fn deposit_window_cap_inner(w: &mut WindowSurface, ci: usize, amount: f32, max_cap: f32) {
        let n = w.cap_n as isize;
        let denom = (w.cap_n.saturating_sub(1)).max(1) as f32;
        let weights = [0.10f32, 0.24, 0.44, 0.64, 0.44, 0.24, 0.10];
        for k in -3..=3 {
            let idx = ci as isize + k;
            if idx >= 0 && idx < n {
                let ui = idx as usize;
                let u = ui as f32 / denom;
                let env = Self::window_edge_env(u, w.w);
                w.cap_h[ui] = (max_cap * env).min(w.cap_h[ui] + amount * weights[(k + 3) as usize]);
            }
        }
    }

    fn relax_window_cap(&mut self, win_idx: usize, max_slope: f32, melt: f32, max_cap: f32) {
        let (wx, wy, ww, n) = {
            let w = &self.win_surfaces[win_idx];
            (w.x, w.y, w.w, w.cap_n)
        };
        {
            let arr = &mut self.win_surfaces[win_idx].cap_h;
            for i in 0..n.saturating_sub(1) {
                let diff = arr[i] - arr[i + 1];
                if diff.abs() > max_slope {
                    let move_amt = (diff - diff.signum() * max_slope) * 0.36;
                    arr[i] -= move_amt;
                    arr[i + 1] += move_amt;
                }
            }
            for i in (0..n.saturating_sub(1)).rev() {
                let diff = arr[i] - arr[i + 1];
                if diff.abs() > max_slope {
                    let move_amt = (diff - diff.signum() * max_slope) * 0.36;
                    arr[i] -= move_amt;
                    arr[i + 1] += move_amt;
                }
            }
        }
        let step = ww / (n.saturating_sub(1)).max(1) as f32;
        let denom = (n.saturating_sub(1)).max(1) as f32;
        for i in 0..n {
            let u = i as f32 / denom;
            let occ = win_idx > 0
                && self.is_occluded_by_higher_window(wx + i as f32 * step, wy, win_idx);
            let arr = &mut self.win_surfaces[win_idx].cap_h;
            if occ {
                arr[i] *= 0.72;
                if arr[i] < 0.25 {
                    arr[i] = 0.0;
                }
                continue;
            }
            let lim = max_cap * Self::window_edge_env(u, ww);
            if arr[i] > lim {
                arr[i] = lerp_f32(arr[i], lim, 0.25);
            }
            if melt > 0.0 && arr[i] > 0.0 {
                let depth_ratio = smoothstep_f32(lim * 0.30, lim * 0.85, arr[i]);
                arr[i] = (arr[i] - melt * (0.08 + 1.25 * depth_ratio)).max(0.0);
            }
        }
    }

    pub fn relax_array(arr: &mut [f32], max_slope: f32, melt: f32, cap: f32) {
        let n = arr.len();
        if n < 3 {
            return;
        }
        for i in 0..(n - 1) {
            let diff = arr[i] - arr[i + 1];
            if diff.abs() > max_slope {
                let move_amt = (diff - diff.signum() * max_slope) * 0.34;
                arr[i] -= move_amt;
                arr[i + 1] += move_amt;
            }
        }
        for i in (0..(n - 1)).rev() {
            let diff = arr[i] - arr[i + 1];
            if diff.abs() > max_slope {
                let move_amt = (diff - diff.signum() * max_slope) * 0.34;
                arr[i] -= move_amt;
                arr[i + 1] += move_amt;
            }
        }
        if melt > 0.0 {
            for v in arr.iter_mut() {
                if *v > 0.0 {
                    let depth_ratio = smoothstep_f32(cap * 0.30, cap * 0.85, *v);
                    *v = (*v - melt * (0.08 + 1.35 * depth_ratio)).max(0.0);
                }
            }
        }
    }

    fn relax_cap_inner(s: &mut IconSurface, max_slope: f32, melt: f32, max_cap: f32) {
        let n = s.cap_n;
        let arr = &mut s.cap_h;
        for i in 0..n.saturating_sub(1) {
            let diff = arr[i] - arr[i + 1];
            if diff.abs() > max_slope {
                let move_amt = (diff - diff.signum() * max_slope) * 0.36;
                arr[i] -= move_amt;
                arr[i + 1] += move_amt;
            }
        }
        let denom = (n.saturating_sub(1)).max(1) as f32;
        for i in 0..n {
            let u = i as f32 / denom;
            let env = (u * PI).sin().max(0.0).powf(0.52);
            let lim = max_cap * env;
            if arr[i] > lim {
                arr[i] = lerp_f32(arr[i], lim, 0.25);
            }
            if melt > 0.0 && arr[i] > 0.0 {
                let depth_ratio = smoothstep_f32(lim * 0.30, lim * 0.85, arr[i]);
                arr[i] = (arr[i] - melt * (0.08 + 1.25 * depth_ratio)).max(0.0);
            }
        }
    }

    fn burst_powder(&mut self, x: f32, y: f32, count: usize) {
        for _ in 0..count {
            if self.powders.len() >= 240 {
                break;
            }
            let ang = -PI * (0.1 + self.rand() * 0.8);
            let spd = 28.0 + self.rand() * 88.0;
            let ox = (self.rand() - 0.5) * 10.0;
            let r = 1.3 + self.rand() * 2.5;
            self.spawn_powder(x + ox, y, ang.cos() * spd, ang.sin() * spd, r);
        }
    }

    // =========================================================================
    // Rain Simulation (Streaks, Splashes, Beaded Glass Drops & Hanging Drops)
    // =========================================================================
    fn step_rain(&mut self, dt: f32, wind_px: f32) {
        let (w, h) = (self.w, self.h);
        let acc_icons =
            self.p.accumulate >= 2 && self.p.adaptive != 0 && !self.surfaces.is_empty();
        let acc_wins =
            self.p.accumulate >= 1 && self.p.windows != 0 && !self.win_surfaces.is_empty();
        let angle_x = wind_px * 1.35;

        let n_streaks = self.rain_streaks.len();
        for i in 0..n_streaks {
            let mut r = self.rain_streaks[i];
            let vx = angle_x * (0.45 + 0.55 * r.z);
            r.x += vx * dt;
            r.y += r.speed * dt;
            if r.x < -160.0 {
                r.x = w + 140.0;
            } else if r.x > w + 160.0 {
                r.x = -140.0;
            }

            let mut hit = false;
            if acc_wins && r.z > 0.30 {
                let n_wins = self.win_surfaces.len();
                for wi in 0..n_wins {
                    let (wid, wx, wy, ww, wh) = {
                        let win = &self.win_surfaces[wi];
                        (win.id, win.x, win.y, win.w, win.h)
                    };
                    if r.x >= wx + 6.0 && r.x <= wx + ww - 6.0 && r.y >= wy - 2.0 && r.y <= wy + 18.0
                    {
                        if !self.is_occluded_by_higher_window(r.x, wy, wi) {
                            self.win_surfaces[wi].wet = (self.win_surfaces[wi].wet + 0.06).min(1.0);
                            self.spawn_splash(r.x, wy - 1.0, 0.88);
                            if self.rand() < 0.08 && self.rain_beads.len() < 165 {
                                let bx = wx + 14.0 + self.rand() * (ww - 28.0).max(10.0);
                                let br = 1.5 + self.rand() * 1.4;
                                let bwob = self.rand() * TAU;
                                self.rain_beads.push(RainBead {
                                    x: bx,
                                    y: wy + wh + 1.0,
                                    r: br,
                                    vx: 0.0,
                                    vy: 0.0,
                                    stuck: 0.0,
                                    surf_id: wid,
                                    is_window: true,
                                    hanging: true,
                                    mass: 0.45,
                                    wobble: bwob,
                                });
                            }
                            hit = true;
                            break;
                        }
                    }
                }
            }
            if !hit && acc_icons && r.z > 0.35 {
                let n_surfs = self.surfaces.len();
                let n_wins = self.win_surfaces.len();
                for si in 0..n_surfs {
                    let (sid, sx, sy, sw) = {
                        let s = &self.surfaces[si];
                        (s.id, s.x, s.y, s.w)
                    };
                    if r.x >= sx + 2.0 && r.x <= sx + sw - 2.0 && r.y >= sy && r.y <= sy + 18.0 {
                        if acc_wins && self.is_occluded_by_higher_window(r.x, sy, n_wins) {
                            continue;
                        }
                        self.surfaces[si].wet = (self.surfaces[si].wet + 0.08).min(1.0);
                        self.spawn_splash(r.x, sy, 0.72);
                        if self.rand() < 0.24 && self.rain_beads.len() < 165 {
                            let by = sy + 2.0 + self.rand() * 6.0;
                            let br = 1.3 + self.rand() * 2.1;
                            let bstuck = 0.4 + self.rand() * 2.2;
                            let bmass = 0.4 + self.rand() * 0.5;
                            let bwob = self.rand() * TAU;
                            self.rain_beads.push(RainBead {
                                x: r.x,
                                y: by,
                                r: br,
                                vx: 0.0,
                                vy: 0.0,
                                stuck: bstuck,
                                surf_id: sid,
                                is_window: false,
                                hanging: false,
                                mass: bmass,
                                wobble: bwob,
                            });
                        }
                        hit = true;
                        break;
                    }
                }
            }
            if !hit && r.y >= h - 3.0 {
                if r.z > 0.32 && self.rand() < 0.65 {
                    self.spawn_splash(r.x, h - 2.0, 0.95);
                }
                hit = true;
            }
            if hit {
                self.rain_streaks[i] = self.make_rain_streak(false);
            } else {
                self.rain_streaks[i] = r;
            }
        }

        let max_beads = ((if self.low_mode { 55.0 } else { 130.0 })
            * clamp_f32(self.p.intensity, 0.3, 1.4))
        .round() as usize;
        if self.rain_beads.len() < max_beads && self.rand() < dt * 16.0 * self.p.intensity {
            let bx = self.rand() * w;
            let by = self.rand() * h * 0.78;
            let br = 1.1 + self.rand() * 2.5;
            let bstuck = 0.5 + self.rand() * 3.0;
            let bmass = 0.35 + self.rand() * 0.65;
            let bwob = self.rand() * TAU;
            self.rain_beads.push(RainBead {
                x: bx,
                y: by,
                r: br,
                vx: 0.0,
                vy: 0.0,
                stuck: bstuck,
                surf_id: 0,
                is_window: false,
                hanging: false,
                mass: bmass,
                wobble: bwob,
            });
        }

        let mut i = self.rain_beads.len();
        while i > 0 {
            i -= 1;
            if i >= self.rain_beads.len() {
                continue;
            }
            self.rain_beads[i].wobble += dt * 5.0;
            if self.rain_beads[i].hanging {
                self.rain_beads[i].r += dt * (0.28 + 0.35 * self.p.intensity);
                let thresh = 4.2 + self.rand() * 1.4;
                if self.rain_beads[i].r > thresh {
                    let b = &mut self.rain_beads[i];
                    b.hanging = false;
                    b.surf_id = 0;
                    b.is_window = false;
                    b.vy = 45.0;
                }
                continue;
            }

            if self.rain_beads[i].stuck > 0.0 {
                let r_cur = self.rain_beads[i].r;
                self.rain_beads[i].stuck -= dt * (0.7 + 0.5 * r_cur * 0.35);
                if self.rand() < dt * 0.8 * self.p.intensity {
                    self.rain_beads[i].r = (self.rain_beads[i].r + 0.25).min(5.2);
                }
                if self.rain_beads[i].r > 3.1 {
                    self.rain_beads[i].stuck = 0.0;
                }
                continue;
            }

            let grav = if self.rain_beads[i].surf_id != 0 {
                140.0
            } else {
                210.0
            };
            {
                let b = &mut self.rain_beads[i];
                let mass_factor = 0.5 + b.r * 0.25 + b.mass * 0.01;
                b.vy = clamp_f32(b.vy + grav * dt * mass_factor, 0.0, 260.0 + b.r * 45.0);
                b.vx = lerp_f32(
                    b.vx,
                    b.wobble.sin() * 14.0 + wind_px * 0.08,
                    clamp_f32(dt * 4.0, 0.0, 1.0),
                );
                b.x += b.vx * dt;
                b.y += b.vy * dt;
            }

            if self.rain_beads[i].vy > 32.0
                && self.rand() < dt * 15.0
                && self.rain_trails.len() < 260
            {
                let jit = (self.rand() - 0.5) * 1.2;
                let b = &mut self.rain_beads[i];
                self.rain_trails.push(RainTrail {
                    x: b.x + jit,
                    y: b.y - b.r,
                    r: 0.65f32.max(b.r * 0.34),
                    a: 0.42,
                });
                b.r = 1.1f32.max(b.r - 0.04);
            }

            if self.rain_beads[i].r < 3.0 && self.rand() < dt * 1.4 {
                let st = 0.25 + self.rand() * 1.1;
                let b = &mut self.rain_beads[i];
                b.vy *= 0.12;
                b.stuck = st;
            }

            // Coalesce smaller beads along path
            let mut j = i;
            while j > 0 {
                j -= 1;
                if self.rain_beads[j].hanging {
                    continue;
                }
                let dx = self.rain_beads[i].x - self.rain_beads[j].x;
                let dy = self.rain_beads[i].y - self.rain_beads[j].y;
                let sr = self.rain_beads[i].r + self.rain_beads[j].r + 1.5;
                if dx * dx + dy * dy < sr * sr {
                    let nr = self.rain_beads[i].r.hypot(self.rain_beads[j].r).min(5.8);
                    self.rain_beads[i].r = nr;
                    self.rain_beads[i].vy += 22.0;
                    self.rain_beads[i].stuck = 0.0;
                    self.rain_beads.remove(j);
                    i -= 1;
                }
            }

            let sid = self.rain_beads[i].surf_id;
            let is_win = self.rain_beads[i].is_window;
            if sid != 0 && !is_win {
                if let Some(s) = self.surfaces.iter().find(|u| u.id == sid) {
                    if self.rain_beads[i].y >= s.y + s.h - 2.0 {
                        let b = &mut self.rain_beads[i];
                        b.y = s.y + s.h + 1.0;
                        b.vy = 0.0;
                        b.vx = 0.0;
                        b.hanging = true;
                    }
                } else {
                    self.rain_beads[i].surf_id = 0;
                }
            }

            if self.rain_beads[i].y >= h - 2.0 {
                let bx = self.rain_beads[i].x;
                self.spawn_splash(bx, h - 2.0, 0.55);
                self.rain_beads.remove(i);
            }
        }

        let evap = (0.14 + self.p.melt * 0.35) * dt;
        for i in (0..self.rain_trails.len()).rev() {
            self.rain_trails[i].a -= evap;
            if self.rain_trails[i].a <= 0.02 {
                self.rain_trails.remove(i);
            }
        }
        for s in &mut self.surfaces {
            s.wet = (s.wet - dt * 0.18).max(0.0);
        }
        for w in &mut self.win_surfaces {
            w.wet = (w.wet - dt * 0.16).max(0.0);
        }

        for i in (0..self.splashes.len()).rev() {
            self.splashes[i].life -= dt;
            if self.splashes[i].life <= 0.0 {
                self.splashes.remove(i);
                continue;
            }
            let sp = &mut self.splashes[i];
            sp.vy += 420.0 * dt;
            sp.x += sp.vx * dt;
            sp.y += sp.vy * dt;
        }
    }

    fn spawn_splash(&mut self, x: f32, y: f32, scale: f32) {
        let cnt = if self.low_mode { 2 } else { 3 };
        for _ in 0..cnt {
            if self.splashes.len() > 160 {
                break;
            }
            let ang = -PI * (0.15 + self.rand() * 0.7);
            let spd = (45.0 + self.rand() * 95.0) * scale;
            let r = (0.9 + self.rand() * 1.1) * scale;
            let life = 0.18 + self.rand() * 0.18;
            self.splashes.push(Splash {
                x,
                y,
                vx: ang.cos() * spd,
                vy: ang.sin() * spd,
                r,
                life,
                max_life: 0.32,
            });
        }
    }

    // =========================================================================
    // Autumn Leaves & Cherry Blossom Simulation
    // =========================================================================
    fn step_foliage(&mut self, dt: f32, wind_px: f32) {
        let (w, h, t) = (self.w, self.h, self.sim_time);
        let mode = self.p.mode;
        let acc_ground = self.p.accumulate >= 1;
        let acc_icons =
            self.p.accumulate >= 2 && self.p.adaptive != 0 && !self.surfaces.is_empty();
        let acc_wins =
            self.p.accumulate >= 1 && self.p.windows != 0 && !self.win_surfaces.is_empty();
        let max_heaps = ((if self.low_mode { 140.0 } else { 380.0 })
            * (0.5 + 0.75 * self.p.depth))
            .round() as usize;
        let ptr_on = self.p.cursor > 0 && self.ptr.active && self.ptr.speed > 20.0;
        let ptr_rad = if self.p.cursor == 2 { 90.0 } else { 60.0 };

        let n_flakes = self.flakes.len();
        for i in 0..n_flakes {
            let mut f = self.flakes[i];
            let glide = (t * f.sway_freq + f.sway_phase).sin() * f.sway_amp;
            let lift =
                (t * f.sway_freq * 2.0 + f.sway_phase).cos() * if mode == 2 { 18.0 } else { 14.0 };
            let target_vx = wind_px * (0.55 + 0.65 * f.z) + glide;
            f.vx = lerp_f32(f.vx, target_vx, clamp_f32(dt * 2.4, 0.0, 1.0));
            let base_vy = (if mode == 2 {
                lerp_f32(34.0, 76.0, f.z)
            } else {
                lerp_f32(24.0, 60.0, f.z)
            }) + lift;
            f.vy = lerp_f32(f.vy, base_vy, clamp_f32(dt * 1.8, 0.0, 1.0));

            if ptr_on && f.z > 0.35 {
                let dx = f.x - self.ptr.x;
                let dy = f.y - self.ptr.y;
                let d2 = dx * dx + dy * dy;
                if d2 < ptr_rad * ptr_rad {
                    let d = d2.sqrt().max(1.0);
                    let push =
                        (1.0 - d / ptr_rad) * if self.p.cursor == 2 { 1.0 } else { 0.55 };
                    f.vx += (self.ptr.vx * 0.45 + (dx / d) * 140.0) * push * dt * 8.0;
                    f.vy += (self.ptr.vy * 0.35 - 90.0) * push * dt * 5.0;
                    f.vrot += (self.rand() - 0.5) * 6.0 * push;
                }
            }

            f.x += f.vx * dt;
            f.y += f.vy * dt;
            f.rot += f.vrot * dt;
            f.pitch += f.vpitch * dt;

            if f.x < -120.0 {
                f.x = w + 110.0;
            } else if f.x > w + 120.0 {
                f.x = -110.0;
            }

            let mut settled = false;
            if acc_wins && f.z > 0.36 && f.vy > 0.0 {
                let n_wins = self.win_surfaces.len();
                for wi in 0..n_wins {
                    let (wid, wx, wy, ww) = {
                        let win = &self.win_surfaces[wi];
                        (win.id, win.x, win.y, win.w)
                    };
                    if f.x >= wx + 10.0
                        && f.x <= wx + ww - 10.0
                        && f.y >= wy - 10.0
                        && f.y <= wy + 8.0
                    {
                        if !self.is_occluded_by_higher_window(f.x, wy, wi) {
                            if self.heaps.len() >= max_heaps && !self.heaps.is_empty() {
                                self.heaps.remove(0);
                            }
                            let ox = f.x - wx;
                            let oy = -1.5 - self.rand() * (5.0 + 8.0 * self.p.depth);
                            let squash = 0.45 + self.rand() * 0.35;
                            self.heaps.push(HeapItem {
                                mode,
                                x: wx + ox,
                                y: wy + oy,
                                surf_id: wid,
                                is_window: true,
                                ox,
                                oy,
                                rot: f.rot,
                                scale: clamp_f32(
                                    f.size / if mode == 2 { 30.0 } else { 19.0 },
                                    0.5,
                                    0.96,
                                ),
                                squash,
                                species: f.species,
                                color_idx: f.color_idx,
                                age: 0.0,
                                alpha: 0.95,
                            });
                            settled = true;
                            break;
                        }
                    }
                }
            }

            if !settled && acc_icons && f.z > 0.42 && f.vy > 0.0 {
                let n_surfs = self.surfaces.len();
                let n_wins = self.win_surfaces.len();
                for si in 0..n_surfs {
                    let (sid, sx, sy, sw) = {
                        let s = &self.surfaces[si];
                        (s.id, s.x, s.y, s.w)
                    };
                    if f.x >= sx + 6.0
                        && f.x <= sx + sw - 6.0
                        && f.y >= sy - 6.0
                        && f.y <= sy + 8.0
                    {
                        if acc_wins && self.is_occluded_by_higher_window(f.x, sy, n_wins) {
                            continue;
                        }
                        if self.heaps.len() < max_heaps {
                            let ox = f.x - sx;
                            let oy = -1.0 - self.rand() * (4.0 + 6.0 * self.p.depth);
                            let squash = 0.45 + self.rand() * 0.35;
                            self.heaps.push(HeapItem {
                                mode,
                                x: sx + ox,
                                y: sy + oy,
                                surf_id: sid,
                                is_window: false,
                                ox,
                                oy,
                                rot: f.rot,
                                scale: clamp_f32(
                                    f.size / if mode == 2 { 32.0 } else { 20.0 },
                                    0.48,
                                    0.95,
                                ),
                                squash,
                                species: f.species,
                                color_idx: f.color_idx,
                                age: 0.0,
                                alpha: 0.94,
                            });
                        }
                        settled = true;
                        break;
                    }
                }
            }

            let ground_thresh = h - 4.0 - self.rand() * (12.0 * self.p.depth);
            if !settled && f.y >= ground_thresh {
                if acc_ground && f.z > 0.35 {
                    if self.heaps.len() >= max_heaps && !self.heaps.is_empty() {
                        self.heaps.remove(0);
                    }
                    let s_sin = (f.x * 0.008).sin();
                    let pile_depth =
                        5.0 + 22.0 * self.p.depth * (0.4 + 0.6 * s_sin * s_sin);
                    let hy = h - 4.0 - self.rand() * pile_depth;
                    let squash = 0.42 + self.rand() * 0.38;
                    self.heaps.push(HeapItem {
                        mode,
                        x: f.x,
                        y: hy,
                        surf_id: 0,
                        is_window: false,
                        ox: 0.0,
                        oy: 0.0,
                        rot: f.rot,
                        scale: clamp_f32(
                            f.size / if mode == 2 { 30.0 } else { 18.0 },
                            0.5,
                            0.98,
                        ),
                        squash,
                        species: f.species,
                        color_idx: f.color_idx,
                        age: 0.0,
                        alpha: 0.95,
                    });
                }
                settled = true;
            }

            if settled || f.y > h + 40.0 {
                self.flakes[i] = self.make_flake(false);
            } else {
                self.flakes[i] = f;
            }
        }

        if self.p.melt > 0.01 {
            let rate = (0.2 + self.p.melt * 1.1) * dt;
            for i in (0..self.heaps.len()).rev() {
                self.heaps[i].age += rate;
                if self.heaps[i].age > 14.0 {
                    self.heaps[i].alpha -= dt * (0.14 + self.p.melt * 0.25);
                    if self.heaps[i].alpha <= 0.04 {
                        self.heaps.remove(i);
                    }
                }
            }
        }
    }
}

// =========================================================================
// OpenGL 3.3 Core Batched 2D Vector & Sprite Renderer for Quiet Snow
// =========================================================================
#[repr(C)]
#[derive(Clone, Copy)]
struct ColorVert {
    pos: [f32; 2],
    col: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SpriteVert {
    pos: [f32; 2],
    uv: [f32; 2],
    col: [f32; 4],
}

struct ColorBatch {
    verts: Vec<ColorVert>,
}

impl ColorBatch {
    fn new() -> Self {
        Self {
            verts: Vec::with_capacity(16384),
        }
    }

    fn clear(&mut self) {
        self.verts.clear();
    }

    #[inline]
    fn push_tri(&mut self, a: ColorVert, b: ColorVert, c: ColorVert) {
        self.verts.push(a);
        self.verts.push(b);
        self.verts.push(c);
    }

    #[inline]
    fn push_quad(&mut self, a: ColorVert, b: ColorVert, c: ColorVert, d: ColorVert) {
        self.push_tri(a, b, c);
        self.push_tri(a, c, d);
    }

    fn fill_rect_vgrad(&mut self, x: f32, y: f32, w: f32, h: f32, top_col: [f32; 4], bot_col: [f32; 4]) {
        self.push_quad(
            ColorVert { pos: [x, y], col: top_col },
            ColorVert { pos: [x + w, y], col: top_col },
            ColorVert { pos: [x + w, y + h], col: bot_col },
            ColorVert { pos: [x, y + h], col: bot_col },
        );
    }

    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, segs: usize, col: [f32; 4]) {
        if col[3] <= 0.001 || rx <= 0.05 || ry <= 0.05 {
            return;
        }
        let center = ColorVert { pos: [cx, cy], col };
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            let p0 = ColorVert {
                pos: [cx + a0.cos() * rx, cy + a0.sin() * ry],
                col,
            };
            let p1 = ColorVert {
                pos: [cx + a1.cos() * rx, cy + a1.sin() * ry],
                col,
            };
            self.push_tri(center, p0, p1);
        }
    }

    fn fill_rotated_ellipse(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rot: f32,
        segs: usize,
        col: [f32; 4],
    ) {
        if col[3] <= 0.001 || rx <= 0.05 || ry <= 0.05 {
            return;
        }
        let (sr, cr) = rot.sin_cos();
        let center = ColorVert { pos: [cx, cy], col };
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            let lx0 = a0.cos() * rx;
            let ly0 = a0.sin() * ry;
            let lx1 = a1.cos() * rx;
            let ly1 = a1.sin() * ry;
            let p0 = ColorVert {
                pos: [cx + lx0 * cr - ly0 * sr, cy + lx0 * sr + ly0 * cr],
                col,
            };
            let p1 = ColorVert {
                pos: [cx + lx1 * cr - ly1 * sr, cy + lx1 * sr + ly1 * cr],
                col,
            };
            self.push_tri(center, p0, p1);
        }
    }

    fn stroke_arc(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        a_start: f32,
        a_end: f32,
        width: f32,
        segs: usize,
        col: [f32; 4],
    ) {
        if col[3] <= 0.001 || r <= 0.05 {
            return;
        }
        let hw = width * 0.5;
        let r_in = (r - hw).max(0.0);
        let r_out = r + hw;
        for i in 0..segs {
            let t0 = i as f32 / segs as f32;
            let t1 = (i + 1) as f32 / segs as f32;
            let a0 = lerp_f32(a_start, a_end, t0);
            let a1 = lerp_f32(a_start, a_end, t1);
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            self.push_quad(
                ColorVert { pos: [cx + c0 * r_in, cy + s0 * r_in], col },
                ColorVert { pos: [cx + c0 * r_out, cy + s0 * r_out], col },
                ColorVert { pos: [cx + c1 * r_out, cy + s1 * r_out], col },
                ColorVert { pos: [cx + c1 * r_in, cy + s1 * r_in], col },
            );
        }
    }

    fn stroke_round_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, col: [f32; 4]) {
        if col[3] <= 0.001 || width <= 0.05 {
            return;
        }
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = dx.hypot(dy);
        if len < 1e-4 {
            self.fill_ellipse(x0, y0, width * 0.5, width * 0.5, 8, col);
            return;
        }
        let hw = width * 0.5;
        let nx = -dy / len * hw;
        let ny = dx / len * hw;
        self.push_quad(
            ColorVert { pos: [x0 + nx, y0 + ny], col },
            ColorVert { pos: [x0 - nx, y0 - ny], col },
            ColorVert { pos: [x1 - nx, y1 - ny], col },
            ColorVert { pos: [x1 + nx, y1 + ny], col },
        );
    }

    fn stroke_polyline(&mut self, pts: &[(f32, f32)], width: f32, col: [f32; 4]) {
        if pts.len() < 2 || col[3] <= 0.001 {
            return;
        }
        let hw = width * 0.5;
        for w in pts.windows(2) {
            let (x0, y0) = w[0];
            let (x1, y1) = w[1];
            let dx = x1 - x0;
            let dy = y1 - y0;
            let len = dx.hypot(dy);
            if len < 1e-4 {
                continue;
            }
            let nx = -dy / len * hw;
            let ny = dx / len * hw;
            self.push_quad(
                ColorVert { pos: [x0 + nx, y0 + ny], col },
                ColorVert { pos: [x0 - nx, y0 - ny], col },
                ColorVert { pos: [x1 - nx, y1 - ny], col },
                ColorVert { pos: [x1 + nx, y1 + ny], col },
            );
        }
    }
}

struct SpriteBatch {
    verts: Vec<SpriteVert>,
}

impl SpriteBatch {
    fn new() -> Self {
        Self {
            verts: Vec::with_capacity(16384),
        }
    }

    fn clear(&mut self) {
        self.verts.clear();
    }

    #[allow(clippy::too_many_arguments)]
    fn push_sprite(
        &mut self,
        slot: usize,
        cx: f32,
        cy: f32,
        w: f32,
        h: f32,
        rot: f32,
        scale_x: f32,
        scale_y: f32,
        col: [f32; 4],
    ) {
        if col[3] <= 0.001 {
            return;
        }
        let [u0, v0, u1, v1] = sprite_uv_rect(slot);
        let hw = w * 0.5 * scale_x;
        let hh = h * 0.5 * scale_y;
        let (sr, cr) = rot.sin_cos();
        let xform = |lx: f32, ly: f32| -> [f32; 2] {
            [cx + lx * cr - ly * sr, cy + lx * sr + ly * cr]
        };
        let p0 = SpriteVert { pos: xform(-hw, -hh), uv: [u0, v0], col };
        let p1 = SpriteVert { pos: xform(hw, -hh), uv: [u1, v0], col };
        let p2 = SpriteVert { pos: xform(hw, hh), uv: [u1, v1], col };
        let p3 = SpriteVert { pos: xform(-hw, hh), uv: [u0, v1], col };
        self.verts.push(p0);
        self.verts.push(p1);
        self.verts.push(p2);
        self.verts.push(p0);
        self.verts.push(p2);
        self.verts.push(p3);
    }
}

pub struct QuietSnowRenderer {
    prog_radial: GLuint,
    prog_color: GLuint,
    prog_sprite: GLuint,
    dummy_vao: GLuint,

    atlas_tex: GLuint,

    bg_fbo: GLuint,
    bg_tex: GLuint,
    bg_w: i32,
    bg_h: i32,
    bg_sig: (i32, i32, i32, i32, i32, i32), // (pw, ph, css_w*10, css_h*10, atm, mode, backdrop*1000 packed)

    color_vao: GLuint,
    color_vbo: GLuint,
    color_batch: ColorBatch,

    sprite_vao: GLuint,
    sprite_vbo: GLuint,
    sprite_batch: SpriteBatch,
}

impl QuietSnowRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_radial = gl.compile_program(FULLSCREEN_VERT, QS_RADIAL_FRAG)?;
        let prog_color = gl.compile_program(QS_COLOR_VERT, QS_COLOR_FRAG)?;
        let prog_sprite = gl.compile_program(QS_SPRITE_VERT, QS_SPRITE_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        // Build & upload 32-sprite pre-baked texture atlas (512x256 RGBA8)
        let atlas_bytes = build_sprite_atlas();
        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            ATLAS_W as GLsizei,
            ATLAS_H as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            atlas_bytes.as_ptr() as *const c_void,
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        // Dynamic Color VAO/VBO
        let mut color_vao = 0;
        let mut color_vbo = 0;
        (gl.GenVertexArrays)(1, &mut color_vao);
        (gl.GenBuffers)(1, &mut color_vbo);
        (gl.BindVertexArray)(color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, color_vbo);
        let c_stride = std::mem::size_of::<ColorVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, c_stride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, c_stride, 8 as *const c_void);

        // Dynamic Sprite VAO/VBO
        let mut sprite_vao = 0;
        let mut sprite_vbo = 0;
        (gl.GenVertexArrays)(1, &mut sprite_vao);
        (gl.GenBuffers)(1, &mut sprite_vbo);
        (gl.BindVertexArray)(sprite_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, sprite_vbo);
        let s_stride = std::mem::size_of::<SpriteVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, s_stride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, s_stride, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 4, GL_FLOAT, GL_FALSE, s_stride, 16 as *const c_void);
        (gl.BindVertexArray)(0);

        Ok(Self {
            prog_radial,
            prog_color,
            prog_sprite,
            dummy_vao,
            atlas_tex,
            bg_fbo: 0,
            bg_tex: 0,
            bg_w: 0,
            bg_h: 0,
            bg_sig: (0, 0, 0, 0, -1, -1),
            color_vao,
            color_vbo,
            color_batch: ColorBatch::new(),
            sprite_vao,
            sprite_vbo,
            sprite_batch: SpriteBatch::new(),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.bg_tex != 0 {
            (gl.DeleteTextures)(1, &self.bg_tex);
            self.bg_tex = 0;
        }
        if self.bg_fbo != 0 {
            (gl.DeleteFramebuffers)(1, &self.bg_fbo);
            self.bg_fbo = 0;
        }
        if self.atlas_tex != 0 {
            (gl.DeleteTextures)(1, &self.atlas_tex);
            self.atlas_tex = 0;
        }
        if self.color_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.color_vbo);
            self.color_vbo = 0;
        }
        if self.color_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.color_vao);
            self.color_vao = 0;
        }
        if self.sprite_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.sprite_vbo);
            self.sprite_vbo = 0;
        }
        if self.sprite_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.sprite_vao);
            self.sprite_vao = 0;
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
        for p in [self.prog_radial, self.prog_color, self.prog_sprite] {
            if p != 0 {
                (gl.DeleteProgram)(p);
            }
        }
    }

    unsafe fn flush_color_batch(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.color_batch.verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_color);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_color, "u_res"), css_w, css_h);
        (gl.BindVertexArray)(self.color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.color_vbo);
        let bytes = (self.color_batch.verts.len() * std::mem::size_of::<ColorVert>()) as GLsizeiptr;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            bytes,
            self.color_batch.verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.color_batch.verts.len() as GLsizei);
        self.color_batch.clear();
    }

    unsafe fn flush_sprite_batch(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.sprite_batch.verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_sprite);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_sprite, "u_res"), css_w, css_h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_sprite, "u_atlas"), 0);

        (gl.BindVertexArray)(self.sprite_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.sprite_vbo);
        let bytes =
            (self.sprite_batch.verts.len() * std::mem::size_of::<SpriteVert>()) as GLsizeiptr;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            bytes,
            self.sprite_batch.verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.sprite_batch.verts.len() as GLsizei);
        self.sprite_batch.clear();
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_radial_gradient(
        &self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        c0: [f32; 3],
        c1: [f32; 3],
        stop_count: i32,
        stops: [f32; 4],
        col0: [f32; 4],
        col1: [f32; 4],
        col2: [f32; 4],
        col3: [f32; 4],
    ) {
        (gl.UseProgram)(self.prog_radial);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_radial, "u_res"), css_w, css_h);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_radial, "u_c0"), c0[0], c0[1], c0[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_radial, "u_c1"), c1[0], c1[1], c1[2]);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_radial, "u_stop_count"), stop_count);
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_radial, "u_stops"),
            stops[0],
            stops[1],
            stops[2],
            stops[3],
        );
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_radial, "u_col0"),
            col0[0],
            col0[1],
            col0[2],
            col0[3],
        );
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_radial, "u_col1"),
            col1[0],
            col1[1],
            col1[2],
            col1[3],
        );
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_radial, "u_col2"),
            col2[0],
            col2[1],
            col2[2],
            col2[3],
        );
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_radial, "u_col3"),
            col3[0],
            col3[1],
            col3[2],
            col3[3],
        );
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
    }

    unsafe fn ensure_backdrop_fbo(&mut self, gl: &GlFns, pw: i32, ph: i32, qs: &QuietSnowState) {
        let pw = pw.max(64);
        let ph = ph.max(64);
        if self.bg_fbo == 0 || self.bg_w != pw || self.bg_h != ph {
            if self.bg_tex != 0 {
                (gl.DeleteTextures)(1, &self.bg_tex);
            }
            if self.bg_fbo != 0 {
                (gl.DeleteFramebuffers)(1, &self.bg_fbo);
            }
            (gl.GenFramebuffers)(1, &mut self.bg_fbo);
            (gl.GenTextures)(1, &mut self.bg_tex);
            (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                pw,
                ph,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                std::ptr::null(),
            );
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
            (gl.FramebufferTexture2D)(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                self.bg_tex,
                0,
            );
            self.bg_w = pw;
            self.bg_h = ph;
            self.bg_sig = (0, 0, 0, 0, -1, -1);
        }

        let sig = (
            pw,
            ph,
            (qs.w * 10.0) as i32,
            (qs.h * 10.0) as i32,
            qs.p.atmosphere * 16 + qs.p.mode,
            (qs.p.backdrop * 1000.0).round() as i32,
        );
        if sig == self.bg_sig {
            return;
        }
        self.bg_sig = sig;

        // Render _buildBackdrop() 1:1 into self.bg_fbo
        let w = qs.w;
        let h = qs.h;
        let atm_idx = (qs.p.atmosphere.clamp(0, 3)) as usize;
        let atm = ATMOS[atm_idx];
        let bd = clamp_f32(qs.p.backdrop, 0.0, 1.0);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        // 1. 4-stop vertical sky gradient (0.0 -> 0.42 -> 0.76 -> 1.0)
        self.color_batch.clear();
        let c0 = rgba_f32(atm.sky[0], 1.0);
        let c1 = rgba_f32(atm.sky[1], 1.0);
        let c2 = rgba_f32(atm.sky[2], 1.0);
        let c3 = rgba_f32(atm.sky[3], 1.0);
        self.color_batch.fill_rect_vgrad(0.0, 0.0, w, h * 0.42, c0, c1);
        self.color_batch.fill_rect_vgrad(0.0, h * 0.42, w, h * 0.34, c1, c2);
        self.color_batch.fill_rect_vgrad(0.0, h * 0.76, w, h * 0.24, c2, c3);

        // 2. Gentle seasonal mood tint
        let mode_idx = (qs.p.mode.clamp(0, 3)) as usize;
        if let Some((mt_rgb, mt_a)) = MODE_TINT[mode_idx] {
            let mc = rgba_f32(mt_rgb, mt_a);
            self.color_batch.fill_rect_vgrad(0.0, 0.0, w, h, mc, mc);
        }

        // 3. Deterministic distant stars (mulberry32(2026 + atmosphere * 97))
        let mut r = Mulberry32::new(2026 + qs.p.atmosphere * 97);
        if atm.stars > 0.0 && bd > 0.1 {
            let n_stars = (95.0 * atm.stars * bd).round() as usize;
            for _ in 0..n_stars {
                let sx = r.next_f32() * w;
                let sy = r.next_f32() * h * 0.56;
                let sr = 0.5 + r.next_f32() * 1.15;
                let sa = (0.14 + r.next_f32() * 0.42) * atm.stars * (1.0 - sy / (h * 0.65));
                self.color_batch.fill_ellipse(
                    sx,
                    sy,
                    sr,
                    sr,
                    8,
                    [235.0 / 255.0, 244.0 / 255.0, 1.0, sa],
                );
            }
        }
        self.flush_color_batch(gl, w, h);

        // 4. Soft horizon atmospheric radial glow
        let h_alpha = if atm.light_bg { 0.32 } else { 0.24 };
        self.draw_radial_gradient(
            gl,
            w,
            h,
            [w * 0.52, h * 0.78, w * 0.05],
            [w * 0.52, h * 0.78, w.max(h) * 0.78],
            2,
            [0.0, 1.0, 1.0, 1.0],
            rgba_f32(atm.haze, h_alpha),
            rgba_f32(atm.haze, 0.0),
            [0.0; 4],
            [0.0; 4],
        );

        // 5. Distant atmospheric ridge & misty pine silhouette
        if bd > 0.04 {
            let draw_ridge = |batch: &mut ColorBatch,
                              base_y: f32,
                              amp: f32,
                              freq: f32,
                              phase: f32,
                              color: [u8; 3],
                              alpha: f32| {
                let col = rgba_f32(color, alpha * bd);
                let step = 12.0f32.max((w / 90.0).round());
                let mut prev_x = 0.0f32;
                let eval_y = |x: f32| -> f32 {
                    let u = x / w;
                    base_y
                        - (u * freq + phase).sin() * amp
                        - (u * freq * 2.3 + phase * 1.7).sin() * (amp * 0.42)
                        - (u * freq * 5.1 + phase * 0.6).cos() * (amp * 0.16)
                };
                let mut prev_y = eval_y(0.0);
                let mut x = step;
                while x <= w + step {
                    let cur_x = x.min(w);
                    let cur_y = eval_y(cur_x);
                    batch.push_quad(
                        ColorVert { pos: [prev_x, prev_y], col },
                        ColorVert { pos: [cur_x, cur_y], col },
                        ColorVert { pos: [cur_x, h], col },
                        ColorVert { pos: [prev_x, h], col },
                    );
                    prev_x = cur_x;
                    prev_y = cur_y;
                    if (prev_x - w).abs() < 0.01 {
                        break;
                    }
                    x += step;
                }
            };

            draw_ridge(&mut self.color_batch, h * 0.74, h * 0.075, 4.2, 0.8, atm.mtn_far, 0.55);

            // Fog veil 1 between far and near ridges (H*0.62 .. H*0.94, stops 0 -> 0.6 -> 1.0)
            let fog1_mid = rgba_f32(atm.haze, (if atm.light_bg { 0.34 } else { 0.22 }) * bd);
            let fog1_top = rgba_f32(atm.haze, 0.0);
            let fog1_bot = rgba_f32(atm.haze, 0.04);
            let f1_y0 = h * 0.62;
            let f1_h = h * 0.32;
            self.color_batch
                .fill_rect_vgrad(0.0, f1_y0, w, f1_h * 0.6, fog1_top, fog1_mid);
            self.color_batch
                .fill_rect_vgrad(0.0, f1_y0 + f1_h * 0.6, w, f1_h * 0.4, fog1_mid, fog1_bot);

            draw_ridge(&mut self.color_batch, h * 0.84, h * 0.042, 6.5, 2.3, atm.mtn_near, 0.68);

            // Soft distant evergreen edge along lower horizon
            let tree_col = rgba_f32(atm.mtn_near, 0.48 * bd);
            let tree_step = 14.0f32.max(w / 78.0);
            let h_scale = clamp_f32(h / 900.0, 0.6, 1.25);
            let mut tx = -10.0f32;
            while tx < w + 20.0 {
                let th = (14.0 + r.next_f32() * 28.0) * h_scale;
                let tw = th * (0.28 + r.next_f32() * 0.12);
                let ty = h * 0.875 + (r.next_f32() - 0.5) * h * 0.03;
                self.color_batch.push_tri(
                    ColorVert { pos: [tx, ty - th], col: tree_col },
                    ColorVert { pos: [tx - tw, ty + 18.0], col: tree_col },
                    ColorVert { pos: [tx + tw, ty + 18.0], col: tree_col },
                );
                tx += tree_step;
            }

            // Low ground mist (fog2: H*0.78 .. H)
            let fog2_top = rgba_f32(atm.haze, 0.0);
            let fog2_bot = rgba_f32(atm.haze, (if atm.light_bg { 0.42 } else { 0.26 }) * bd);
            self.color_batch
                .fill_rect_vgrad(0.0, h * 0.78, w, h * 0.22, fog2_top, fog2_bot);
            self.flush_color_batch(gl, w, h);
        }

        // 6. Streetlamp warm cone & bokeh halo (Atmosphere 3)
        if atm.lamp {
            let lx = w * 0.80;
            let ly = h * 0.12;
            self.draw_radial_gradient(
                gl,
                w,
                h,
                [lx, ly, 6.0],
                [lx - w * 0.06, ly + h * 0.32, w.max(h) * 0.68],
                4,
                [0.0, 0.16, 0.45, 1.0],
                [1.0, 224.0 / 255.0, 168.0 / 255.0, 0.44],
                [1.0, 196.0 / 255.0, 118.0 / 255.0, 0.24],
                [242.0 / 255.0, 164.0 / 255.0, 88.0 / 255.0, 0.08],
                [242.0 / 255.0, 164.0 / 255.0, 88.0 / 255.0, 0.0],
            );
            // Soft distant streetlamp orb
            self.draw_radial_gradient(
                gl,
                w,
                h,
                [lx, ly, 0.0],
                [lx, ly, 26.0],
                3,
                [0.0, 0.35, 1.0, 1.0],
                [1.0, 248.0 / 255.0, 226.0 / 255.0, 0.92],
                [1.0, 212.0 / 255.0, 140.0 / 255.0, 0.45],
                [1.0, 190.0 / 255.0, 110.0 / 255.0, 0.0],
                [0.0; 4],
            );
        }

        // 7. Subtle photographic vignette
        let vig_col = if atm.light_bg {
            [30.0 / 255.0, 48.0 / 255.0, 68.0 / 255.0, 0.16]
        } else {
            [2.0 / 255.0, 6.0 / 255.0, 14.0 / 255.0, 0.34]
        };
        self.draw_radial_gradient(
            gl,
            w,
            h,
            [w * 0.5, h * 0.48, w.min(h) * 0.35],
            [w * 0.5, h * 0.5, w.max(h) * 0.78],
            2,
            [0.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 0.0],
            vig_col,
            [0.0; 4],
            [0.0; 4],
        );
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState, prog_blit: GLuint) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();

        st.quiet_snow.resize(css_w, css_h);

        self.ensure_backdrop_fbo(gl, pw, ph, &st.quiet_snow);

        // Blit cached backdrop FBO to screen
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(prog_blit);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
        (gl.Uniform1i)(gl.uniform_loc(prog_blit, "u_tex"), 0);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        let qs = &st.quiet_snow;
        let atm_idx = (qs.p.atmosphere.clamp(0, 3)) as usize;
        let atm = ATMOS[atm_idx];

        match qs.p.mode {
            0 => self.draw_snow_scene(gl, qs, &atm, css_w, css_h),
            1 => self.draw_rain_scene(gl, qs, &atm, css_w, css_h),
            _ => self.draw_foliage_scene(gl, qs, &atm, css_w, css_h),
        }

        // Shared powder & spindrift layer
        if !qs.powders.is_empty() {
            let slot = snow_soft_slot(0);
            for pt in &qs.powders {
                let k = clamp_f32(pt.life / pt.max_life.max(1e-4), 0.0, 1.0);
                let alpha = pt.alpha * k;
                let d = pt.r * 2.8;
                self.sprite_batch.push_sprite(
                    slot,
                    pt.x,
                    pt.y,
                    d,
                    d,
                    0.0,
                    1.0,
                    1.0,
                    [1.0, 1.0, 1.0, alpha],
                );
            }
            self.flush_sprite_batch(gl, css_w, css_h);
        }
    }

    unsafe fn draw_snow_scene(
        &mut self,
        gl: &GlFns,
        qs: &QuietSnowState,
        atm: &AtmospherePalette,
        w: f32,
        h: f32,
    ) {
        let lamp_x = w * 0.80;
        let lamp_y = h * 0.12;

        // 1. Far & Mid snowflakes behind icon snow caps (z < 0.72)
        for f in &qs.flakes {
            if f.z >= 0.72 {
                continue;
            }
            self.queue_flake_sprite(f, atm, w, lamp_x, lamp_y);
        }
        self.flush_sprite_batch(gl, w, h);

        // 2. Icon snow caps
        if qs.p.accumulate >= 2 && qs.p.adaptive != 0 {
            for s in &qs.surfaces {
                self.queue_icon_snow_cap(s, atm);
            }
            self.flush_color_batch(gl, w, h);
        }

        // 2b. Open window top-edge snow drifts (drawn back-to-front)
        if qs.p.accumulate >= 1 && qs.p.windows != 0 {
            for w_surf in qs.win_surfaces.iter().rev() {
                self.queue_window_snow_cap(w_surf, qs, atm);
            }
            self.flush_color_batch(gl, w, h);
        }

        // 3. Falling snow lumps shed from icons & windows
        if !qs.lumps.is_empty() {
            for (i, l) in qs.lumps.iter().enumerate() {
                let slot = snow_soft_slot(i & 3);
                let d = l.r * 2.5;
                self.sprite_batch.push_sprite(
                    slot,
                    l.x,
                    l.y,
                    d,
                    d,
                    0.0,
                    1.0,
                    1.0,
                    [1.0, 1.0, 1.0, 0.92],
                );
            }
            self.flush_sprite_batch(gl, w, h);
        }

        // 4. Ground snow drift heightfield with subsurface shading & sparkles
        if qs.p.accumulate >= 1 {
            self.queue_ground_snow(qs, atm, w, h);
            self.flush_color_batch(gl, w, h);
        }

        // 5. Near-foreground snowflakes & bokeh (z >= 0.72)
        for f in &qs.flakes {
            if f.z < 0.72 {
                continue;
            }
            self.queue_flake_sprite(f, atm, w, lamp_x, lamp_y);
        }
        self.flush_sprite_batch(gl, w, h);
    }

    fn queue_flake_sprite(
        &mut self,
        f: &Flake,
        atm: &AtmospherePalette,
        w: f32,
        lamp_x: f32,
        lamp_y: f32,
    ) {
        let slot = if f.kind == 2 {
            snow_bokeh_slot(f.species & 1)
        } else if f.kind == 1 {
            snow_crystal_slot(f.color_idx % 3)
        } else {
            snow_soft_slot(f.species & 3)
        };
        let mut alpha = f.alpha;
        if atm.lamp {
            let d = (f.x - lamp_x).hypot(f.y - lamp_y);
            let boost = smoothstep_f32(w * 0.45, 30.0, d);
            alpha = (alpha + boost * 0.32).min(1.0);
        }
        let rot = if f.kind == 1 { f.rot } else { 0.0 };
        let ft = rgba_f32(atm.flake_tint, alpha);
        self.sprite_batch.push_sprite(
            slot,
            f.x,
            f.y,
            f.size,
            f.size,
            rot,
            1.0,
            1.0,
            ft,
        );
    }

    fn sample_heightfield_curve(
        x_start: f32,
        step: f32,
        base_y: f32,
        arr: &[f32],
        y_offset: f32,
    ) -> Vec<(f32, f32)> {
        let n = arr.len();
        if n == 0 {
            return Vec::new();
        }
        if n == 1 {
            return vec![(x_start, base_y - arr[0] + y_offset)];
        }
        let mut pts = Vec::with_capacity(n * 2);
        pts.push((x_start, base_y - arr[0] + y_offset));
        let mut cur_x = x_start;
        let mut cur_y = base_y - arr[0] + y_offset;
        for i in 1..(n - 1) {
            let cx = x_start + i as f32 * step;
            let cy = base_y - arr[i] + y_offset;
            let xm = cx + step * 0.5;
            let ym = base_y - (arr[i] + arr[i + 1]) * 0.5 + y_offset;
            // Subdivide quadratic curve into 2 segments
            let t = 0.5f32;
            let qx = 0.25 * cur_x + 0.5 * cx + 0.25 * xm;
            let qy = 0.25 * cur_y + 0.5 * cy + 0.25 * ym;
            let _ = t;
            pts.push((qx, qy));
            pts.push((xm, ym));
            cur_x = xm;
            cur_y = ym;
        }
        pts.push((
            x_start + (n - 1) as f32 * step,
            base_y - arr[n - 1] + y_offset,
        ));
        pts
    }

    fn eval_3stop_grad(
        y: f32,
        y_top: f32,
        y_bot: f32,
        mid_t: f32,
        c_top: [f32; 4],
        c_mid: [f32; 4],
        c_bot: [f32; 4],
    ) -> [f32; 4] {
        let t = ((y - y_top) / (y_bot - y_top).max(1e-3)).clamp(0.0, 1.0);
        if t < mid_t {
            let u = t / mid_t.max(1e-4);
            [
                lerp_f32(c_top[0], c_mid[0], u),
                lerp_f32(c_top[1], c_mid[1], u),
                lerp_f32(c_top[2], c_mid[2], u),
                lerp_f32(c_top[3], c_mid[3], u),
            ]
        } else {
            let u = (t - mid_t) / (1.0 - mid_t).max(1e-4);
            [
                lerp_f32(c_mid[0], c_bot[0], u),
                lerp_f32(c_mid[1], c_bot[1], u),
                lerp_f32(c_mid[2], c_bot[2], u),
                lerp_f32(c_mid[3], c_bot[3], u),
            ]
        }
    }

    fn queue_ground_snow(&mut self, qs: &QuietSnowState, atm: &AtmospherePalette, w: f32, h: f32) {
        let arr = &qs.ground_h;
        let n = qs.ground_n;
        let max_h = arr.iter().copied().fold(0.0f32, f32::max);
        if max_h < 0.5 || n < 2 {
            return;
        }

        let mut contour = Self::sample_heightfield_curve(0.0, qs.cell_w, h, arr, 0.0);
        if let Some(last) = contour.last_mut() {
            last.0 = w;
        }
        let y_top = h - max_h - 4.0;
        let y_bot = h;
        let c_top = rgba_f32(atm.snow_top, 0.98);
        let c_mid = rgba_f32(atm.snow_mid, 0.98);
        let c_bot = rgba_f32(atm.snow_deep, 0.99);
        let y_mid = lerp_f32(y_top, y_bot, 0.38);

        for seg in contour.windows(2) {
            let (x0, ty0) = seg[0];
            let (x1, ty1) = seg[1];
            let ct0 = Self::eval_3stop_grad(ty0, y_top, y_bot, 0.38, c_top, c_mid, c_bot);
            let ct1 = Self::eval_3stop_grad(ty1, y_top, y_bot, 0.38, c_top, c_mid, c_bot);
            let ym0 = ty0.max(y_mid);
            let ym1 = ty1.max(y_mid);
            let cm0 = Self::eval_3stop_grad(ym0, y_top, y_bot, 0.38, c_top, c_mid, c_bot);
            let cm1 = Self::eval_3stop_grad(ym1, y_top, y_bot, 0.38, c_top, c_mid, c_bot);

            self.color_batch.push_quad(
                ColorVert { pos: [x0, ty0], col: ct0 },
                ColorVert { pos: [x1, ty1], col: ct1 },
                ColorVert { pos: [x1, ym1], col: cm1 },
                ColorVert { pos: [x0, ym0], col: cm0 },
            );
            self.color_batch.push_quad(
                ColorVert { pos: [x0, ym0], col: cm0 },
                ColorVert { pos: [x1, ym1], col: cm1 },
                ColorVert { pos: [x1, h + 4.0], col: c_bot },
                ColorVert { pos: [x0, h + 4.0], col: c_bot },
            );
        }

        // Cool rim-shadow depth ridge just below sunlit crest
        let rim_off = 10.0f32.min(max_h * 0.24);
        let rim_col = rgba_f32(atm.snow_deep, 0.16);
        for seg in contour.windows(2) {
            let (x0, ty0) = seg[0];
            let (x1, ty1) = seg[1];
            let ry0 = (ty0 + rim_off).min(h + 2.0);
            let ry1 = (ty1 + rim_off).min(h + 2.0);
            self.color_batch.push_quad(
                ColorVert { pos: [x0, ry0], col: rim_col },
                ColorVert { pos: [x1, ry1], col: rim_col },
                ColorVert { pos: [x1, h + 4.0], col: rim_col },
                ColorVert { pos: [x0, h + 4.0], col: rim_col },
            );
        }

        // Delicate sunlit crest line
        let crest_col = if atm.light_bg {
            rgba_f32(atm.snow_outline, 0.55)
        } else {
            [1.0, 1.0, 1.0, 0.52]
        };
        self.color_batch.stroke_polyline(&contour, 1.25, crest_col);

        // Microscopic ice crystal sparkles on snow drift surface
        if qs.p.sparkle > 0.02 && !qs.low_mode {
            let t = qs.sim_time;
            let sp_amp = qs.p.sparkle;
            for sp in &qs.ground_sparkles {
                let gi = ((sp.u * (n - 1) as f32).floor() as isize)
                    .clamp(0, n as isize - 1) as usize;
                let sh = arr[gi];
                if sh < 4.0 {
                    continue;
                }
                let tw = (t * sp.freq + sp.phase).sin().max(0.0).powi(12);
                if tw < 0.08 {
                    continue;
                }
                let sx = gi as f32 * qs.cell_w;
                let sy = h - sh * (1.0 - sp.depth_frac);
                let r = sp.size * (0.7 + 0.6 * tw);
                let col = if atm.lamp {
                    [1.0, 244.0 / 255.0, 210.0 / 255.0, (tw * sp_amp * 0.92).clamp(0.0, 1.0)]
                } else {
                    [1.0, 1.0, 1.0, (tw * sp_amp * 0.92).clamp(0.0, 1.0)]
                };
                self.color_batch.fill_ellipse(sx, sy, r, r, 8, col);
            }
        }
    }

    fn queue_icon_snow_cap(&mut self, s: &IconSurface, atm: &AtmospherePalette) {
        let arr = &s.cap_h;
        let n = s.cap_n;
        let max_h = arr.iter().copied().fold(0.0f32, f32::max);
        if max_h < 0.6 || n < 2 {
            return;
        }
        let step = s.w / (n - 1).max(1) as f32;
        let base_y = s.y + 2.5;

        // Soft contact shadow cast onto top of icon
        self.color_batch.fill_ellipse(
            s.x + s.w * 0.5,
            base_y + 1.5,
            s.w * 0.44,
            4.2f32.min(1.5 + max_h * 0.18),
            16,
            [10.0 / 255.0, 18.0 / 255.0, 32.0 / 255.0, atm.shadow_alpha * 0.45],
        );

        let contour = Self::sample_heightfield_curve(s.x, step, base_y, arr, 0.0);
        let y_top = base_y - max_h - 1.0;
        let y_bot = base_y + 3.0;
        let c_top = rgba_f32(atm.snow_top, 0.98);
        let c_mid = rgba_f32(atm.snow_mid, 0.96);
        let c_bot = rgba_f32(atm.snow_deep, 0.95);
        let lip_amp = 3.2f32.min(max_h * 0.22);

        for seg in contour.windows(2) {
            let (x0, ty0) = seg[0];
            let (x1, ty1) = seg[1];
            let u0 = ((x0 - s.x) / s.w.max(1.0)).clamp(0.0, 1.0);
            let u1 = ((x1 - s.x) / s.w.max(1.0)).clamp(0.0, 1.0);
            let by0 = base_y + 2.0 * u0 * (1.0 - u0) * lip_amp;
            let by1 = base_y + 2.0 * u1 * (1.0 - u1) * lip_amp;
            let ct0 = Self::eval_3stop_grad(ty0, y_top, y_bot, 0.55, c_top, c_mid, c_bot);
            let ct1 = Self::eval_3stop_grad(ty1, y_top, y_bot, 0.55, c_top, c_mid, c_bot);
            let cb0 = Self::eval_3stop_grad(by0, y_top, y_bot, 0.55, c_top, c_mid, c_bot);
            let cb1 = Self::eval_3stop_grad(by1, y_top, y_bot, 0.55, c_top, c_mid, c_bot);

            self.color_batch.push_quad(
                ColorVert { pos: [x0, ty0], col: ct0 },
                ColorVert { pos: [x1, ty1], col: ct1 },
                ColorVert { pos: [x1, by1], col: cb1 },
                ColorVert { pos: [x0, by0], col: cb0 },
            );
        }

        let stroke_col = if atm.light_bg {
            rgba_f32(atm.snow_outline, 0.5)
        } else {
            [1.0, 1.0, 1.0, 0.48]
        };
        self.color_batch.stroke_polyline(&contour, 0.9, stroke_col);
    }

    fn queue_window_snow_cap(
        &mut self,
        w: &WindowSurface,
        qs: &QuietSnowState,
        atm: &AtmospherePalette,
    ) {
        let arr = &w.cap_h;
        let n = w.cap_n;
        let max_h = arr.iter().copied().fold(0.0f32, f32::max);
        if max_h < 0.7 || n < 2 {
            return;
        }
        let step = w.w / (n - 1).max(1) as f32;
        let base_y = w.y + 2.5;
        let vis_alpha = smoothstep_f32(0.65, 2.2, max_h);

        let mut contour = Self::sample_heightfield_curve(w.x, step, base_y, arr, 0.0);
        if let Some(first) = contour.first_mut() {
            first.0 = w.x + 4.0;
        }
        if let Some(last) = contour.last_mut() {
            last.0 = w.x + w.w - 4.0;
        }

        let y_top = base_y - max_h - 2.0;
        let y_bot = base_y + 2.0;
        let c_top = rgba_f32(atm.snow_top, 0.99 * vis_alpha);
        let c_mid = rgba_f32(atm.snow_mid, 0.98 * vis_alpha);
        let c_bot = rgba_f32(atm.snow_deep, 0.98 * vis_alpha);

        for seg in contour.windows(2) {
            let (x0, ty0) = seg[0];
            let (x1, ty1) = seg[1];
            let ct0 = Self::eval_3stop_grad(ty0, y_top, y_bot, 0.48, c_top, c_mid, c_bot);
            let ct1 = Self::eval_3stop_grad(ty1, y_top, y_bot, 0.48, c_top, c_mid, c_bot);
            self.color_batch.push_quad(
                ColorVert { pos: [x0, ty0], col: ct0 },
                ColorVert { pos: [x1, ty1], col: ct1 },
                ColorVert { pos: [x1, base_y + 1.0], col: c_bot },
                ColorVert { pos: [x0, base_y + 1.0], col: c_bot },
            );
        }

        if max_h > 1.8 {
            let stroke_col = if atm.light_bg {
                rgba_f32(atm.snow_outline, 0.55 * vis_alpha)
            } else {
                [1.0, 1.0, 1.0, 0.56 * vis_alpha]
            };
            self.color_batch.stroke_polyline(&contour, 1.1, stroke_col);
        }

        if qs.p.sparkle > 0.05 && !qs.low_mode && max_h > 3.0 {
            let t = qs.sim_time;
            let sp_count = ((w.w / 110.0).floor() as usize).clamp(3, 10);
            let hwnd_mod = (w.hwnd.rem_euclid(11)) as f32;
            for k in 0..sp_count {
                let u = (k as f32 + 0.5) / sp_count as f32;
                let ci = ((u * (n - 1) as f32).floor() as isize)
                    .clamp(0, n as isize - 1) as usize;
                let sh = arr[ci];
                if sh < 3.0 {
                    continue;
                }
                let tw = (t * (1.8 + (k % 3) as f32 * 0.7) + k as f32 * 2.1 + hwnd_mod)
                    .sin()
                    .max(0.0)
                    .powi(12);
                if tw < 0.1 {
                    continue;
                }
                let col = if atm.lamp {
                    [1.0, 244.0 / 255.0, 210.0 / 255.0, (tw * qs.p.sparkle * 0.9).clamp(0.0, 1.0)]
                } else {
                    [1.0, 1.0, 1.0, (tw * qs.p.sparkle * 0.9).clamp(0.0, 1.0)]
                };
                let r = 1.0 + tw * 0.9;
                self.color_batch.fill_ellipse(
                    w.x + ci as f32 * step,
                    base_y - sh * 0.65,
                    r,
                    r,
                    8,
                    col,
                );
            }
        }
    }

    unsafe fn draw_rain_scene(
        &mut self,
        gl: &GlFns,
        qs: &QuietSnowState,
        atm: &AtmospherePalette,
        w: f32,
        h: f32,
    ) {
        let wind_px = qs.p.wind * 72.0 + qs.gust_val * 115.0;
        let slope_x = wind_px * 1.35;

        // 1. Subtle wet sheen along icon & open window top edges
        if qs.p.accumulate >= 2 && qs.p.adaptive != 0 {
            for s in &qs.surfaces {
                if s.wet < 0.04 {
                    continue;
                }
                self.color_batch.fill_rect_vgrad(
                    s.x + 4.0,
                    s.y,
                    (s.w - 8.0).max(0.0),
                    7.0,
                    rgba_f32(atm.rain_tint, 0.34 * s.wet),
                    rgba_f32(atm.rain_tint, 0.0),
                );
            }
        }
        if qs.p.accumulate >= 1 && qs.p.windows != 0 {
            for win in &qs.win_surfaces {
                if win.wet < 0.04 {
                    continue;
                }
                self.color_batch.fill_rect_vgrad(
                    win.x + 8.0,
                    win.y - 4.0,
                    (win.w - 16.0).max(0.0),
                    5.0,
                    rgba_f32(atm.rain_tint, 0.0),
                    rgba_f32(atm.rain_tint, 0.42 * win.wet),
                );
            }
        }

        // 2. Falling rain streaks
        for r in &qs.rain_streaks {
            let dx = (slope_x * (0.45 + 0.55 * r.z) / r.speed.max(1.0)) * r.len;
            let col = rgba_f32(atm.rain_tint, r.alpha * (0.65 + 0.35 * qs.p.sparkle));
            self.color_batch
                .stroke_round_line(r.x - dx, r.y - r.len, r.x, r.y, r.thick, col);
        }

        // 3. Wet beaded trails left by running drops
        for tr in &qs.rain_trails {
            let col = rgba_f32(atm.rain_tint, tr.a * 0.52);
            self.color_batch.fill_ellipse(tr.x, tr.y, tr.r, tr.r, 8, col);
        }

        // 4. Glass & icon water beads (drop shadow + body + lower caustic crescent + top specular dot)
        for b in &qs.rain_beads {
            let stretch = if b.hanging {
                1.38
            } else if b.vy > 20.0 {
                clamp_f32(1.0 + b.vy / 260.0, 1.05, 1.65)
            } else {
                1.08
            };
            let rx = b.r;
            let ry = b.r * stretch;

            // Soft drop shadow
            self.color_batch.fill_ellipse(
                b.x + 0.6,
                b.y + 1.1,
                rx * 1.02,
                ry * 0.96,
                12,
                [4.0 / 255.0, 10.0 / 255.0, 20.0 / 255.0, 0.24],
            );

            // Translucent bead body
            self.color_batch.fill_ellipse(
                b.x,
                b.y,
                rx,
                ry,
                12,
                rgba_f32(atm.rain_tint, 0.22 + 0.14 * qs.p.sparkle),
            );

            // Lower caustic bright crescent
            self.color_batch.stroke_arc(
                b.x,
                b.y + ry * 0.16,
                rx * 0.72,
                0.25,
                PI - 0.25,
                0.6f32.max(rx * 0.26),
                8,
                [1.0, 1.0, 1.0, (0.42 + 0.36 * qs.p.sparkle).clamp(0.0, 1.0)],
            );

            // Crisp top-left specular dot
            let dot_r = 0.55f32.max(rx * 0.28);
            self.color_batch.fill_ellipse(
                b.x - rx * 0.28,
                b.y - ry * 0.28,
                dot_r,
                dot_r,
                8,
                [1.0, 1.0, 1.0, (0.65 + 0.3 * qs.p.sparkle).clamp(0.0, 1.0)],
            );
        }

        // 5. Splash droplets
        for sp in &qs.splashes {
            let k = clamp_f32(sp.life / sp.max_life.max(1e-4), 0.0, 1.0);
            let col = rgba_f32(atm.rain_tint, 0.68 * k);
            self.color_batch.fill_ellipse(sp.x, sp.y, sp.r, sp.r, 8, col);
        }

        self.flush_color_batch(gl, w, h);
    }

    unsafe fn draw_foliage_scene(
        &mut self,
        gl: &GlFns,
        qs: &QuietSnowState,
        atm: &AtmospherePalette,
        w: f32,
        h: f32,
    ) {
        let is_leaf = qs.p.mode == 2;

        // 1. Far falling leaves / petals (z < 0.7)
        for f in &qs.flakes {
            if f.z >= 0.7 {
                continue;
            }
            self.queue_foliage_item(f, is_leaf, qs, atm);
        }
        self.flush_sprite_batch(gl, w, h);
        self.flush_color_batch(gl, w, h);

        // 2. Settled individual leaf / petal heaps (ground + icon caps + window caps)
        let sp_base_sz = if is_leaf { 44.0 } else { 32.0 };
        for item in &qs.heaps {
            let slot = if is_leaf {
                leaf_slot(item.species % 3, item.color_idx % AUTUMN_COLORS.len())
            } else {
                petal_slot(item.color_idx % BLOSSOM_COLORS.len())
            };
            // Subtle browning/fading tint when wilting (age > 8)
            let wilt = if item.age > 8.0 {
                clamp_f32((item.age - 8.0) / 14.0, 0.0, 0.42)
            } else {
                0.0
            };
            let tint = if wilt > 0.0 {
                let wc = if is_leaf {
                    [62.0 / 255.0, 34.0 / 255.0, 16.0 / 255.0]
                } else {
                    [138.0 / 255.0, 92.0 / 255.0, 80.0 / 255.0]
                };
                [
                    lerp_f32(1.0, wc[0], wilt),
                    lerp_f32(1.0, wc[1], wilt),
                    lerp_f32(1.0, wc[2], wilt),
                    item.alpha,
                ]
            } else {
                [1.0, 1.0, 1.0, item.alpha]
            };
            self.sprite_batch.push_sprite(
                slot,
                item.x,
                item.y,
                sp_base_sz,
                sp_base_sz,
                item.rot,
                item.scale,
                item.scale * item.squash,
                tint,
            );
        }
        self.flush_sprite_batch(gl, w, h);

        // 3. Foreground tumbling leaves / petals (z >= 0.7)
        for f in &qs.flakes {
            if f.z < 0.7 {
                continue;
            }
            self.queue_foliage_item(f, is_leaf, qs, atm);
        }
        self.flush_sprite_batch(gl, w, h);
        self.flush_color_batch(gl, w, h);
    }

    fn queue_foliage_item(
        &mut self,
        f: &Flake,
        is_leaf: bool,
        qs: &QuietSnowState,
        atm: &AtmospherePalette,
    ) {
        let slot = if is_leaf {
            leaf_slot(f.species % 3, f.color_idx % AUTUMN_COLORS.len())
        } else if f.whole_flower {
            FLOWER_SLOT
        } else {
            petal_slot(f.color_idx % BLOSSOM_COLORS.len())
        };
        let tilt = 0.26 + 0.74 * f.pitch.cos().abs();
        let sy = if f.whole_flower {
            0.65 + 0.35 * tilt
        } else {
            tilt
        };
        self.sprite_batch.push_sprite(
            slot,
            f.x,
            f.y,
            f.size,
            f.size,
            f.rot,
            1.0,
            sy,
            [1.0, 1.0, 1.0, f.alpha],
        );

        // Translucent sunlight/lamp glint when petal turns flat toward light
        if !is_leaf && !f.whole_flower && qs.p.sparkle > 0.1 && tilt > 0.86 {
            let ga = (tilt - 0.86)
                * 4.2
                * qs.p.sparkle
                * if atm.lamp { 0.95 } else { 0.65 };
            let col = if atm.lamp {
                [1.0, 243.0 / 255.0, 214.0 / 255.0, ga.clamp(0.0, 1.0)]
            } else {
                [1.0, 1.0, 1.0, ga.clamp(0.0, 1.0)]
            };
            self.color_batch.fill_rotated_ellipse(
                f.x,
                f.y,
                f.size * 0.22,
                f.size * 0.32 * sy,
                f.rot,
                10,
                col,
            );
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

    #[test]
    fn verify_rust_anything_recorded_quietsnow_cases() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root");
        let rec_path = manifest_dir.join("src/quiet_snow/recorded-from.json");
        let cases_path = manifest_dir.join("src/quiet_snow/cases.jsonl");

        // 1. Verify recording fingerprint against prototype/quietsnow-engine.js (rust-anything gate check)
        let rec_json: Value = serde_json::from_str(
            &std::fs::read_to_string(&rec_path).expect("read recorded-from.json"),
        )
        .expect("parse recorded-from.json");
        let unit_rel = rec_json["unit"].as_str().expect("unit path");
        let expected_fp = rec_json["fingerprint"].as_str().expect("fingerprint");
        let js_bytes =
            std::fs::read(repo_root.join(unit_rel)).expect("read prototype/quietsnow-engine.js");
        let actual_fp = fnv1a_fingerprint(unit_rel, &js_bytes);
        assert_eq!(
            actual_fp, expected_fp,
            "rust-anything gate: prototype/quietsnow-engine.js changed since recording"
        );

        // 2. Replay every recorded case in cases.jsonl and verify 100% behavioral match
        let raw_cases = std::fs::read_to_string(&cases_path).expect("read cases.jsonl");
        let mut passed = 0usize;
        for (line_no, line) in raw_cases.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let c: Value = serde_json::from_str(line).expect("parse case JSON");
            let fn_name = c["fn"].as_str().expect("fn");
            let args = c["args"].as_array().expect("args");
            let ok = &c["result"]["ok"];

            match fn_name {
                "targetCount" => {
                    let mode = args[0].as_i64().unwrap() as i32;
                    let quality = args[1].as_i64().unwrap() as i32;
                    let low_mode = args[2].as_bool().unwrap();
                    let w = args[3].as_f64().unwrap() as f32;
                    let h = args[4].as_f64().unwrap() as f32;
                    let intensity = args[5].as_f64().unwrap() as f32;

                    let mut p = QuietSnowParams::default();
                    p.mode = mode;
                    p.quality = quality;
                    p.intensity = intensity;
                    let mut st = QuietSnowState::new(w, h, p, 1001);
                    st.set_low_mode(low_mode);
                    st.resize(w, h);
                    let got = st.target_count() as i64;
                    let exp = ok.as_i64().unwrap();
                    assert_eq!(got, exp, "case #{line_no} targetCount mismatch");
                }
                "windowEdgeEnv" => {
                    let u = args[0].as_f64().unwrap() as f32;
                    let width = args[1].as_f64().unwrap() as f32;
                    let got = QuietSnowState::window_edge_env(u, width);
                    let exp = ok.as_f64().unwrap() as f32;
                    assert!(
                        (got - exp).abs() <= 1e-4,
                        "case #{line_no} windowEdgeEnv({u}, {width}): got {got}, expected {exp}"
                    );
                }
                "seedInitialSnow" => {
                    let w = args[0].as_f64().unwrap() as f32;
                    let h = args[1].as_f64().unwrap() as f32;
                    let depth = args[2].as_f64().unwrap() as f32;
                    let mut p = QuietSnowParams::default();
                    p.depth = depth;
                    let mut st = QuietSnowState::new(w, h, p, 3003);
                    st.resize(w, h);
                    st.seed_initial_snow();

                    assert_eq!(st.cell_w as i64, ok["cellW"].as_i64().unwrap());
                    assert_eq!(st.ground_n as i64, ok["groundN"].as_i64().unwrap());
                    assert_eq!(
                        st.ground_sparkles.len() as i64,
                        ok["sparkleCount"].as_i64().unwrap()
                    );

                    let sample_indices = [
                        0,
                        1,
                        5,
                        15,
                        50,
                        st.ground_n / 2,
                        st.ground_n - 2,
                        st.ground_n - 1,
                    ];
                    let exp_samples = ok["samples"].as_array().unwrap();
                    for ( idx, exp_v) in sample_indices.iter().zip(exp_samples.iter()) {
                        let got_v = st.ground_h[*idx];
                        let ev = exp_v.as_f64().unwrap() as f32;
                        assert!(
                            (got_v - ev).abs() <= 1e-3,
                            "case #{line_no} seedInitialSnow ground_h[{idx}]: got {got_v}, exp {ev}"
                        );
                    }
                    let sp0 = st.ground_sparkles[0];
                    let exp_sp0 = ok["sp0"].as_array().unwrap();
                    for (got_f, exp_f) in [sp0.u, sp0.depth_frac, sp0.phase, sp0.freq, sp0.size]
                        .iter()
                        .zip(exp_sp0.iter())
                    {
                        let ev = exp_f.as_f64().unwrap() as f32;
                        assert!(
                            (got_f - ev).abs() <= 1e-4,
                            "case #{line_no} seedInitialSnow sparkle[0] mismatch: got {got_f}, exp {ev}"
                        );
                    }
                }
                "depositAndRelaxGround" => {
                    let depth = args[0].as_f64().unwrap() as f32;
                    let melt = args[1].as_f64().unwrap() as f32;
                    let mut p = QuietSnowParams::default();
                    p.depth = depth;
                    p.melt = melt;
                    let mut st = QuietSnowState::new(1280.0, 720.0, p, 4004);
                    st.resize(1280.0, 720.0);
                    st.ground_h.fill(0.0);
                    st.deposit_ground(20, 14.0, 50.0);
                    st.deposit_ground(21, 18.0, 50.0);
                    QuietSnowState::relax_array(&mut st.ground_h, 1.35, melt * 0.05, 50.0);
                    let exp_slice = ok.as_array().unwrap();
                    for (k, ev) in exp_slice.iter().enumerate() {
                        let got_v = st.ground_h[17 + k];
                        let exp_v = ev.as_f64().unwrap() as f32;
                        assert!(
                            (got_v - exp_v).abs() <= 1e-3,
                            "case #{line_no} depositAndRelaxGround[{k}]: got {got_v}, exp {exp_v}"
                        );
                    }
                }
                "iconLifecycleAndShedding" => {
                    let mut st =
                        QuietSnowState::new(1280.0, 720.0, QuietSnowParams::default(), 5005);
                    st.resize(1280.0, 720.0);
                    st.set_icons(&[
                        Rect { x: 60.0, y: 80.0, w: 76.0, h: 90.0 },
                        Rect { x: 60.0, y: 200.0, w: 76.0, h: 90.0 },
                    ]);
                    assert_eq!(st.surfaces[0].cap_n as i64, ok["capN"].as_i64().unwrap());
                    for (got_v, ev) in st.surfaces[0]
                        .cap_h
                        .iter()
                        .zip(ok["capBefore"].as_array().unwrap().iter())
                    {
                        assert!((got_v - ev.as_f64().unwrap() as f32).abs() <= 1e-3);
                    }
                    st.set_icons(&[
                        Rect { x: 90.0, y: 95.0, w: 76.0, h: 90.0 },
                        Rect { x: 60.0, y: 200.0, w: 76.0, h: 90.0 },
                    ]);
                    for (got_v, ev) in st.surfaces[0]
                        .cap_h
                        .iter()
                        .zip(ok["capAfterMove"].as_array().unwrap().iter())
                    {
                        assert!((got_v - ev.as_f64().unwrap() as f32).abs() <= 1e-3);
                    }
                    assert_eq!(
                        st.lumps.len() as i64,
                        ok["lumpsAfterMove"].as_i64().unwrap()
                    );
                    st.set_icons(&[Rect { x: 90.0, y: 95.0, w: 76.0, h: 90.0 }]);
                    assert_eq!(
                        st.lumps.len() as i64,
                        ok["lumpsAfterRemove"].as_i64().unwrap()
                    );
                }
                "windowLifecycleAndOcclusion" => {
                    let mut st =
                        QuietSnowState::new(1920.0, 1080.0, QuietSnowParams::default(), 6006);
                    st.resize(1920.0, 1080.0);
                    st.set_windows(&[
                        WindowInput { id: 101, x: 200.0, y: 180.0, w: 600.0, h: 400.0 },
                        WindowInput { id: 202, x: 400.0, y: 260.0, w: 500.0, h: 360.0 },
                    ]);
                    assert_eq!(
                        st.win_surfaces[0].cap_n as i64,
                        ok["win0CapN"].as_i64().unwrap()
                    );
                    assert_eq!(
                        st.is_occluded_by_higher_window(450.0, 260.0, 1),
                        ok["occ1"].as_bool().unwrap()
                    );
                    assert_eq!(
                        st.is_occluded_by_higher_window(850.0, 260.0, 1),
                        ok["occ2"].as_bool().unwrap()
                    );
                    for (got_v, ev) in st.win_surfaces[0].cap_h[0..8]
                        .iter()
                        .zip(ok["win0SampleBefore"].as_array().unwrap().iter())
                    {
                        assert!((got_v - ev.as_f64().unwrap() as f32).abs() <= 1e-3);
                    }
                    st.set_windows(&[
                        WindowInput { id: 101, x: 205.0, y: 205.0, w: 600.0, h: 400.0 },
                        WindowInput { id: 202, x: 400.0, y: 260.0, w: 500.0, h: 360.0 },
                    ]);
                    for (got_v, ev) in st.win_surfaces[0].cap_h[0..8]
                        .iter()
                        .zip(ok["win0SampleAfterShed"].as_array().unwrap().iter())
                    {
                        assert!((got_v - ev.as_f64().unwrap() as f32).abs() <= 1e-3);
                    }
                    assert_eq!(st.lumps.len() as i64, ok["lumpsCount"].as_i64().unwrap());
                }
                "cursorSweepGround" => {
                    let mut st =
                        QuietSnowState::new(1280.0, 720.0, QuietSnowParams::default(), 7007);
                    st.resize(1280.0, 720.0);
                    st.ground_h.fill(20.0);
                    st.sim_time = 1.0;
                    st.on_pointer_move(400.0, 705.0, Some(false));
                    st.sim_time = 1.02;
                    st.on_pointer_move(425.0, 705.0, Some(true));
                    for (got_v, ev) in st.ground_h[95..118]
                        .iter()
                        .zip(ok["sweptSlice"].as_array().unwrap().iter())
                    {
                        assert!(
                            (got_v - ev.as_f64().unwrap() as f32).abs() <= 1e-2,
                            "cursorSweepGround slice mismatch: got {got_v}, exp {}",
                            ev.as_f64().unwrap()
                        );
                    }
                }
                other => panic!("Unknown recorded function: {other}"),
            }
            passed += 1;
        }
        assert_eq!(passed, 267, "All 267 recorded rust-anything cases must run and pass");
    }

    #[test]
    fn seasonal_override_switches_and_restores_user_mode() {
        let mut st = QuietSnowState::new(1280.0, 720.0, QuietSnowParams::default(), 42);
        assert_eq!(st.p.mode, 0);
        st.set_seasonal_override(Some(3)); // Sakura Spring
        assert_eq!(st.p.mode, 3);
        // Updating unrelated params keeps active seasonal override
        let mut np = QuietSnowParams::default();
        np.mode = 1;
        st.sync_params(np);
        assert_eq!(st.p.mode, 3);
        // Clearing override restores user-selected mode
        st.set_seasonal_override(None);
        assert_eq!(st.p.mode, 1);
    }
}
