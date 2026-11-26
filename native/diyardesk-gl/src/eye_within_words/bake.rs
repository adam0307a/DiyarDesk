//! CPU-side typography baking — the Rust equivalent of main.py's PIL/pygame
//! surface precomputation (`_build_all_layers`).
//!
//! Optimized for real-time mobile & desktop responsiveness:
//! - Two-phase deterministic architecture: Phase 1 computes all `PyRng`
//!   placements & warms the `fontdue` glyph cache; Phase 2 rasterizes all
//!   layers in parallel via `std::thread::scope` over disjoint row strips /
//!   independent layer groups.
//! - Reusable 1-channel `AlphaMask` with a 2 px zero margin + tight OBB
//!   scanline rasterizer (bilinear coverage lookup + integer fast-path blending).
//! - Analytic `erf` radial masks for `cutout` & `pupil_cut`, 2x downsampled
//!   `glow` with O(W*H) 3-pass sliding-window box blur, and vertically cropped
//!   single-render `rim_atlas`.

use crate::eye_within_words::pyrng::PyRng;
use crate::eye_within_words::{
    eval_eyelid_curves, eval_field_streamline_y, generate_meaningful_quotes, scale_geometry,
    EyeCanvasGeometry, EyePalette, PALETTES,
};
use crate::text;
use fontdue::Font;
use std::collections::HashMap;
use std::f64::consts::{PI, SQRT_2, TAU};

// ============================================================================
// Public data structures
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakeKey {
    pub screen_w: i32,
    pub screen_h: i32,
    pub palette: usize,
    pub seed: u32,
    pub bake_scale: u32, // x1000
}

#[derive(Clone, Copy, Debug)]
pub struct RimSpriteRect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub w: i32,
    pub h: i32,
}

/// A single crisp word along the moving upper or lower eyelid rim, with the
/// three pre-rotated angle variants (closed / mid / open) exactly like the
/// original's `EyelidRimWord.surfaces`.
#[derive(Clone, Copy, Debug)]
pub struct RimWord {
    pub u: f64,
    pub v_offset: f64,
    pub is_upper: bool,
    pub variants: [RimSpriteRect; 3],
}

/// Straight-alpha RGBA buffer (PIL `Image.new("RGBA")` equivalent).
pub struct RgbaLayer {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

impl RgbaLayer {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            w: w.max(1),
            h: h.max(1),
            px: vec![0u8; (w.max(1) as usize) * (h.max(1) as usize) * 4],
        }
    }

    pub fn fill(&mut self, color: [u8; 4]) {
        for p in self.px.chunks_exact_mut(4) {
            p.copy_from_slice(&color);
        }
    }

    /// Ensure the outermost 1 px border has alpha = 0 so `GL_CLAMP_TO_EDGE`
    /// never smears radial glow or sprite layers across the quad.
    pub fn clear_outer_border(&mut self) {
        let w = self.w as usize;
        let h = self.h as usize;
        if w == 0 || h == 0 {
            return;
        }
        let row_bytes = w * 4;
        self.px[..row_bytes].fill(0);
        if h > 1 {
            let last = (h - 1) * row_bytes;
            self.px[last..last + row_bytes].fill(0);
        }
        for y in 0..h {
            let base = y * row_bytes;
            self.px[base..base + 4].fill(0);
            if w > 1 {
                let right = base + (w - 1) * 4;
                self.px[right..right + 4].fill(0);
            }
        }
    }

    #[allow(dead_code)]
    pub fn clone_layer(&self) -> RgbaLayer {
        RgbaLayer {
            w: self.w,
            h: self.h,
            px: self.px.clone(),
        }
    }
}

pub struct BakedEyeLayers {
    /// Bake-pixel geometry (screen geometry scaled by `bake_scale`).
    pub geom: EyeCanvasGeometry,
    /// Unscaled screen-space geometry (used for gaze offsets & camera math).
    pub geom_orig: EyeCanvasGeometry,
    pub bake_scale: f32,

    pub field: RgbaLayer,
    pub sclera: RgbaLayer,
    pub cutout: RgbaLayer,
    pub cutout_dim: i32,
    pub glow: RgbaLayer,
    pub rings_a: RgbaLayer,
    pub rings_b: RgbaLayer,
    pub radial_a: RgbaLayer,
    pub radial_b: RgbaLayer,
    pub iris_dim: i32,
    pub pupil: RgbaLayer,
    pub pupil_dim: i32,
    pub highlights: RgbaLayer,
    pub rim_atlas: RgbaLayer,
    pub rim_words: Vec<RimWord>,
    #[allow(dead_code)] // read by the deterministic layout tests
    pub word_count: usize,
}

// ============================================================================
// Glyph rasterization cache & reusable 1-channel AlphaMask
// ============================================================================

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct GlyphKey {
    ch: char,
    size: u16,
    bold: bool,
}

struct GlyphBmp {
    w: i32,
    h: i32,
    left_off: i32, // xmin: pen-relative left edge of the bitmap
    top_off: i32,  // baseline-relative top edge (negative = above baseline)
    advance: f64,
    cov: Vec<u8>,
}

/// Reusable single-channel alpha sprite buffer (1 byte per pixel, zero-padded
/// 2 px margin on all 4 sides). Reuses its `Vec` allocation across words.
struct AlphaMask {
    w: i32,
    h: i32,
    a: Vec<u8>,
}

impl AlphaMask {
    fn new() -> Self {
        Self {
            w: 0,
            h: 0,
            a: Vec::with_capacity(4096),
        }
    }

    #[inline]
    fn resize_clear(&mut self, w: i32, h: i32) {
        self.w = w.max(1);
        self.h = h.max(1);
        let len = (self.w as usize) * (self.h as usize);
        self.a.clear();
        self.a.resize(len, 0);
    }
}

/// Persistent word baker: UI font pair + glyph cache, reused across rebakes.
pub struct EyeBaker {
    fonts: Option<(Font, Font)>,
    glyphs: HashMap<GlyphKey, GlyphBmp>,
}

impl EyeBaker {
    pub fn new() -> Self {
        Self {
            fonts: text::load_baking_font_pair().ok(),
            glyphs: HashMap::new(),
        }
    }

    fn glyph(&mut self, ch: char, size: i32, bold: bool) -> &GlyphBmp {
        let key = GlyphKey {
            ch,
            size: size.clamp(7, 160) as u16,
            bold,
        };
        if !self.glyphs.contains_key(&key) {
            let entry = match &self.fonts {
                Some((reg, bold_font)) => {
                    let primary = if bold { bold_font } else { reg };
                    let (metrics, bitmap) = primary.rasterize(ch, key.size as f32);
                    let gw = metrics.width as i32;
                    let gh = metrics.height as i32;
                    GlyphBmp {
                        w: gw,
                        h: gh,
                        left_off: metrics.xmin as i32,
                        // fontdue: bitmap bottom = baseline - ymin
                        top_off: -metrics.ymin as i32 - gh,
                        advance: metrics.advance_width as f64,
                        cov: bitmap,
                    }
                }
                None => GlyphBmp {
                    w: 0,
                    h: 0,
                    left_off: 0,
                    top_off: 0,
                    advance: 6.0,
                    cov: Vec::new(),
                },
            };
            self.glyphs.insert(key, entry);
        }
        self.glyphs.get(&key).unwrap()
    }

    /// Ensure every glyph in `text` at `(size, bold)` is rasterized and cached
    /// before Phase 2 borrows `self.glyphs` read-only across worker threads.
    #[inline]
    pub fn warm_word(&mut self, text: &str, size: i32, bold: bool) {
        if self.fonts.is_none() {
            return;
        }
        for ch in text.chars() {
            let _ = self.glyph(ch, size, bold);
        }
    }

    /// PIL `font.getbbox` equivalent using already-cached glyphs.
    fn ink_bbox(&mut self, text: &str, size: i32, bold: bool) -> (f64, f64, f64, f64) {
        self.warm_word(text, size, bold);
        ink_bbox_cached(&self.glyphs, text, size, bold)
    }

    /// PIL `FontManager.measure` equivalent: `(max(4, w), max(4, h))`.
    pub fn measure(&mut self, text: &str, size: i32, bold: bool) -> (f64, f64) {
        let (x0, y0, x1, y1) = self.ink_bbox(text, size, bold);
        ((x1 - x0).max(4.0), (y1 - y0).max(4.0))
    }

    /// Render a word into its own tight sprite with a 2px transparent margin.
    #[allow(dead_code)]
    pub fn render_word_sprite(
        &mut self,
        text: &str,
        size: i32,
        bold: bool,
        color: [u8; 4],
    ) -> RgbaLayer {
        self.warm_word(text, size, bold);
        let mut mask = AlphaMask::new();
        render_word_mask(&self.glyphs, text, size, bold, &mut mask);
        let mut spr = RgbaLayer::new(mask.w, mask.h);
        for (i, &cov) in mask.a.iter().enumerate() {
            if cov == 0 {
                continue;
            }
            let idx = i * 4;
            spr.px[idx] = color[0];
            spr.px[idx + 1] = color[1];
            spr.px[idx + 2] = color[2];
            spr.px[idx + 3] = ((color[3] as u32 * cov as u32 + 127) / 255) as u8;
        }
        spr
    }

    /// Anti-aliased word centered at (cx, cy), rotated by `angle_deg`,
    /// alpha-composited into `target`.
    #[allow(dead_code)]
    pub fn draw_word(
        &mut self,
        target: &mut RgbaLayer,
        text: &str,
        cx: f64,
        cy: f64,
        fsize: i32,
        angle_deg: f64,
        color: [u8; 4],
        bold: bool,
    ) {
        if self.fonts.is_none() {
            return;
        }
        self.warm_word(text, fsize, bold);
        let mut mask = AlphaMask::new();
        render_word_mask(&self.glyphs, text, fsize, bold, &mut mask);
        let w = target.w;
        let h = target.h;
        composite_mask_obb(
            &mask,
            target.px.as_mut_slice(),
            w,
            0,
            h,
            None,
            [0, 0, w, h],
            cx,
            cy,
            angle_deg,
            color,
        );
    }
}

/// Pure read-only ink bounding box (x0, y0, x1, y1) from cached glyphs.
fn ink_bbox_cached(
    glyphs: &HashMap<GlyphKey, GlyphBmp>,
    text: &str,
    size: i32,
    bold: bool,
) -> (f64, f64, f64, f64) {
    let clamped = size.clamp(7, 160) as u16;
    let mut pen = 0.0f64;
    let mut x0 = f64::MAX;
    let mut y0 = f64::MAX;
    let mut x1 = f64::MIN;
    let mut y1 = f64::MIN;
    for ch in text.chars() {
        let key = GlyphKey {
            ch,
            size: clamped,
            bold,
        };
        if let Some(g) = glyphs.get(&key) {
            if g.w > 0 && g.h > 0 {
                x0 = x0.min(pen + g.left_off as f64);
                x1 = x1.max(pen + g.left_off as f64 + g.w as f64);
                y0 = y0.min(g.top_off as f64);
                y1 = y1.max(g.top_off as f64 + g.h as f64);
            }
            pen += g.advance;
        } else {
            pen += 6.0;
        }
    }
    if x0 > x1 || y0 > y1 {
        (0.0, 0.0, 4.0, 4.0)
    } else {
        (x0, y0, x1, y1)
    }
}

/// Rasterize `text` into a reusable 1-channel `AlphaMask` with 2 px margin.
fn render_word_mask(
    glyphs: &HashMap<GlyphKey, GlyphBmp>,
    text: &str,
    size: i32,
    bold: bool,
    mask: &mut AlphaMask,
) {
    let clamped = size.clamp(7, 160) as u16;
    let (x0, y0, x1, y1) = ink_bbox_cached(glyphs, text, size, bold);
    let ink_w = ((x1 - x0).ceil() as i32).max(1);
    let ink_h = ((y1 - y0).ceil() as i32).max(1);
    mask.resize_clear(ink_w + 4, ink_h + 4);
    let mw = mask.w;
    let mh = mask.h;

    let mut pen = 0.0f64;
    for ch in text.chars() {
        let key = GlyphKey {
            ch,
            size: clamped,
            bold,
        };
        let Some(g) = glyphs.get(&key) else {
            pen += 6.0;
            continue;
        };
        if g.w > 0 && g.h > 0 {
            let sx = 2 + (pen + g.left_off as f64 - x0).round() as i32;
            let sy = 2 + (g.top_off as f64 - y0).round() as i32;
            for gy in 0..g.h {
                let py = sy + gy;
                if py < 0 || py >= mh {
                    continue;
                }
                let src_row = (gy * g.w) as usize;
                let dst_row = (py * mw) as usize;
                for gx in 0..g.w {
                    let cov = g.cov[src_row + gx as usize];
                    if cov == 0 {
                        continue;
                    }
                    let px = sx + gx;
                    if px < 0 || px >= mw {
                        continue;
                    }
                    let slot = &mut mask.a[dst_row + px as usize];
                    if cov > *slot {
                        *slot = cov;
                    }
                }
            }
        }
        pen += g.advance;
    }
}

// ============================================================================
// Fast integer blending & tight OBB scanline rasterizer
// ============================================================================

#[inline(always)]
fn blend_pixel(dst: &mut [u8], r: u8, g: u8, b: u8, sa: u8) {
    if sa == 0 {
        return;
    }
    let da = dst[3];
    if da == 0 {
        dst[0] = r;
        dst[1] = g;
        dst[2] = b;
        dst[3] = sa;
        return;
    }
    if da == 255 {
        let s = sa as u32;
        let inv = 255u32 - s;
        dst[0] = ((r as u32 * s + dst[0] as u32 * inv + 127) / 255) as u8;
        dst[1] = ((g as u32 * s + dst[1] as u32 * inv + 127) / 255) as u8;
        dst[2] = ((b as u32 * s + dst[2] as u32 * inv + 127) / 255) as u8;
        return;
    }
    let saf = sa as f32 * (1.0 / 255.0);
    let daf = da as f32 * (1.0 / 255.0) * (1.0 - saf);
    let oa = saf + daf;
    if oa <= 0.0 {
        return;
    }
    let inv_oa = 1.0 / oa;
    dst[0] = ((r as f32 * saf + dst[0] as f32 * daf) * inv_oa + 0.5).clamp(0.0, 255.0) as u8;
    dst[1] = ((g as f32 * saf + dst[1] as f32 * daf) * inv_oa + 0.5).clamp(0.0, 255.0) as u8;
    dst[2] = ((b as f32 * saf + dst[2] as f32 * daf) * inv_oa + 0.5).clamp(0.0, 255.0) as u8;
    dst[3] = (oa * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
}

/// Composite `mask` onto `dst_a` (covering global rows `y_offset .. y_offset + strip_h`),
/// and optionally onto a second layer `dst_b` in the exact same loop.
/// `clip_rect` is `[x_min, y_min, x_max, y_max]` (exclusive upper bounds) in global coords.
#[allow(clippy::too_many_arguments)]
fn composite_mask_obb(
    mask: &AlphaMask,
    dst_a: &mut [u8],
    dst_w: i32,
    y_offset: i32,
    strip_h: i32,
    mut dst_b: Option<(&mut [u8], [u8; 4])>,
    clip_rect: [i32; 4],
    cx: f64,
    cy: f64,
    angle_deg: f64,
    col_a: [u8; 4],
) {
    let cx0 = clip_rect[0].max(0);
    let cy0 = clip_rect[1].max(y_offset);
    let cx1 = clip_rect[2].min(dst_w);
    let cy1 = clip_rect[3].min(y_offset + strip_h);
    if cx0 >= cx1 || cy0 >= cy1 {
        return;
    }

    let sw = mask.w;
    let sh = mask.h;
    if sw <= 0 || sh <= 0 {
        return;
    }

    if angle_deg.abs() <= 0.45 {
        let tx = crate::eye_within_words::py_round(cx - sw as f64 * 0.5) as i32;
        let ty = crate::eye_within_words::py_round(cy - sh as f64 * 0.5) as i32;
        let x0 = tx.max(cx0);
        let y0 = ty.max(cy0);
        let x1 = (tx + sw).min(cx1);
        let y1 = (ty + sh).min(cy1);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let a_alpha = col_a[3] as u32;
        for sy in y0..y1 {
            let src_row = ((sy - ty) * sw) as usize;
            let dst_row = ((sy - y_offset) * dst_w) as usize;
            for sx in x0..x1 {
                let cov = mask.a[src_row + (sx - tx) as usize];
                if cov == 0 {
                    continue;
                }
                let sa = ((a_alpha * cov as u32 + 127) / 255) as u8;
                let di = (dst_row + sx as usize) * 4;
                blend_pixel(&mut dst_a[di..di + 4], col_a[0], col_a[1], col_a[2], sa);
                if let Some((ref mut b_px, col_b)) = dst_b {
                    let sb = if col_b[3] == col_a[3] {
                        sa
                    } else {
                        ((col_b[3] as u32 * cov as u32 + 127) / 255) as u8
                    };
                    blend_pixel(&mut b_px[di..di + 4], col_b[0], col_b[1], col_b[2], sb);
                }
            }
        }
        return;
    }

    let theta = angle_deg * PI / 180.0;
    let (c, s) = (theta.cos(), theta.sin());
    let nw = (sw as f64 * c.abs() + sh as f64 * s.abs()).ceil().max(1.0);
    let nh = (sw as f64 * s.abs() + sh as f64 * c.abs()).ceil().max(1.0);
    let px_origin = crate::eye_within_words::py_round(cx - nw * 0.5);
    let py_origin = crate::eye_within_words::py_round(cy - nh * 0.5);
    let cx_center = px_origin + nw * 0.5;
    let cy_center = py_origin + nh * 0.5;

    let hw = sw as f64 * 0.5 + 1.0;
    let hh = sh as f64 * 0.5 + 1.0;
    let corners = [
        (
            cx_center + c * (-hw) + s * (-hh),
            cy_center - s * (-hw) + c * (-hh),
        ),
        (
            cx_center + c * hw + s * (-hh),
            cy_center - s * hw + c * (-hh),
        ),
        (cx_center + c * hw + s * hh, cy_center - s * hw + c * hh),
        (
            cx_center + c * (-hw) + s * hh,
            cy_center - s * (-hw) + c * hh,
        ),
    ];

    let mut y_min_f = f64::MAX;
    let mut y_max_f = f64::MIN;
    for &(_, py) in &corners {
        y_min_f = y_min_f.min(py);
        y_max_f = y_max_f.max(py);
    }
    let y_start = (y_min_f.floor() as i32).max(cy0);
    let y_end = ((y_max_f.ceil() as i32) - 1).min(cy1 - 1);
    if y_start > y_end {
        return;
    }

    let cx_src = sw as f64 * 0.5;
    let cy_src = sh as f64 * 0.5;
    let c32 = c as f32;
    let s32 = s as f32;
    let a_scale = col_a[3] as f32 * (1.0 / 255.0);
    let b_scale = dst_b
        .as_ref()
        .map(|(_, cb)| cb[3] as f32 * (1.0 / 255.0))
        .unwrap_or(0.0);
    let sw_u = sw as usize;
    let sw_m1 = sw - 1;
    let sh_m1 = sh - 1;

    for y in y_start..=y_end {
        let yc = y as f64 + 0.5;
        let mut x_lo = f64::MAX;
        let mut x_hi = f64::MIN;
        let mut hits = 0;
        for i in 0..4 {
            let (ax, ay) = corners[i];
            let (bx, by) = corners[(i + 1) & 3];
            if (ay <= yc && by > yc) || (by <= yc && ay > yc) {
                let t = (yc - ay) / (by - ay);
                let xi = ax + t * (bx - ax);
                x_lo = x_lo.min(xi);
                x_hi = x_hi.max(xi);
                hits += 1;
            }
        }
        if hits < 2 {
            continue;
        }
        let x0 = ((x_lo - 1.0).floor() as i32).max(cx0);
        let x1 = ((x_hi + 1.0).ceil() as i32).min(cx1 - 1);
        if x0 > x1 {
            continue;
        }

        let ox0 = x0 as f64 + 0.5 - cx_center;
        let oy = yc - cy_center;
        let mut fx = (ox0 * c - oy * s + cx_src - 0.5) as f32;
        let mut fy = (ox0 * s + oy * c + cy_src - 0.5) as f32;
        let dst_row = ((y - y_offset) * dst_w) as usize;

        for x in x0..=x1 {
            let ix = fx.floor() as i32;
            let iy = fy.floor() as i32;
            if ix >= 0 && iy >= 0 && ix < sw_m1 && iy < sh_m1 {
                let row0 = iy as usize * sw_u + ix as usize;
                let row1 = row0 + sw_u;
                let a00 = mask.a[row0];
                let a10 = mask.a[row0 + 1];
                let a01 = mask.a[row1];
                let a11 = mask.a[row1 + 1];
                if (a00 | a10 | a01 | a11) != 0 {
                    let tx = fx - ix as f32;
                    let ty = fy - iy as f32;
                    let top = a00 as f32 + (a10 as f32 - a00 as f32) * tx;
                    let bot = a01 as f32 + (a11 as f32 - a01 as f32) * tx;
                    let cov_f = top + (bot - top) * ty;
                    let sa = (cov_f * a_scale + 0.5).clamp(0.0, 255.0) as u8;
                    let di = (dst_row + x as usize) * 4;
                    blend_pixel(&mut dst_a[di..di + 4], col_a[0], col_a[1], col_a[2], sa);
                    if let Some((ref mut b_px, col_b)) = dst_b {
                        let sb = if col_b[3] == col_a[3] {
                            sa
                        } else {
                            (cov_f * b_scale + 0.5).clamp(0.0, 255.0) as u8
                        };
                        blend_pixel(&mut b_px[di..di + 4], col_b[0], col_b[1], col_b[2], sb);
                    }
                }
            }
            fx += c32;
            fy += s32;
        }
    }
}

/// Rotated dimensions matching `rotate_expand` (`angle_deg` degrees CCW).
#[inline]
fn rotated_dims(sw: i32, sh: i32, angle_deg: f64) -> (i32, i32) {
    if angle_deg.abs() <= 0.45 {
        (sw.max(1), sh.max(1))
    } else {
        let theta = angle_deg * PI / 180.0;
        let (c, s) = (theta.cos().abs(), theta.sin().abs());
        let nw = (sw as f64 * c + sh as f64 * s).ceil().max(1.0) as i32;
        let nh = (sw as f64 * s + sh as f64 * c).ceil().max(1.0) as i32;
        (nw, nh)
    }
}

/// Conservative global vertical bounds `[min_y, max_y]` (inclusive) for a placed
/// word so horizontal strips can cull non-overlapping words in 2 comparisons.
#[inline]
fn word_y_bounds(cy: f64, sw: i32, sh: i32, angle_deg: f64) -> (i32, i32) {
    if angle_deg.abs() <= 0.45 {
        let hh = (sh as f64 * 0.5).ceil() as i32 + 2;
        (cy.floor() as i32 - hh, cy.ceil() as i32 + hh)
    } else {
        let theta = angle_deg * PI / 180.0;
        let (c, s) = (theta.cos().abs(), theta.sin().abs());
        let hh = ((sw as f64 + 2.0) * s + (sh as f64 + 2.0) * c) * 0.5 + 2.0;
        ((cy - hh).floor() as i32, (cy + hh).ceil() as i32)
    }
}

// ============================================================================
// Pixel operations (PIL equivalents + fast analytic/box-blur primitives)
// ============================================================================

/// PIL `Image.alpha_composite` (straight-alpha OVER) with clipping.
#[allow(dead_code)]
pub fn composite(dst: &mut RgbaLayer, src: &RgbaLayer, x: i32, y: i32) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + src.w).min(dst.w);
    let y1 = (y + src.h).min(dst.h);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for sy in y0..y1 {
        for sx in x0..x1 {
            let di = ((sy * dst.w + sx) * 4) as usize;
            let si = (((sy - y) * src.w + (sx - x)) * 4) as usize;
            let s = &src.px[si..si + 4];
            blend_pixel(&mut dst.px[di..di + 4], s[0], s[1], s[2], s[3]);
        }
    }
}

/// PIL `Image.rotate(angle, resample=BICUBIC, expand=True)` — counterclockwise
/// for positive angles, expanded canvas, transparent fill.
#[allow(dead_code)]
pub fn rotate_expand(src: &RgbaLayer, angle_deg: f64) -> RgbaLayer {
    let theta = angle_deg * PI / 180.0;
    let (c, s) = (theta.cos(), theta.sin());
    let w = src.w as f64;
    let h = src.h as f64;
    let nw = (w * c.abs() + h * s.abs()).ceil().max(1.0) as i32;
    let nh = (w * s.abs() + h * c.abs()).ceil().max(1.0) as i32;
    let mut out = RgbaLayer::new(nw, nh);

    let cx_src = w / 2.0;
    let cy_src = h / 2.0;
    let cx_dst = nw as f64 / 2.0;
    let cy_dst = nh as f64 / 2.0;

    for dy in 0..nh {
        for dx in 0..nw {
            let ox = dx as f64 + 0.5 - cx_dst;
            let oy = dy as f64 + 0.5 - cy_dst;
            let fx = ox * c - oy * s + cx_src - 0.5;
            let fy = ox * s + oy * c + cy_src - 0.5;
            let px = bicubic_sample(src, fx, fy);
            let idx = ((dy * nw + dx) * 4) as usize;
            out.px[idx] = px[0].round().clamp(0.0, 255.0) as u8;
            out.px[idx + 1] = px[1].round().clamp(0.0, 255.0) as u8;
            out.px[idx + 2] = px[2].round().clamp(0.0, 255.0) as u8;
            out.px[idx + 3] = px[3].round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

#[allow(dead_code)]
fn bicubic_sample(src: &RgbaLayer, x: f64, y: f64) -> [f64; 4] {
    let x0f = x.floor();
    let y0f = y.floor();
    let tx = x - x0f;
    let ty = y - y0f;
    let x0 = x0f as i32;
    let y0 = y0f as i32;

    let mut acc = [0.0f64; 4];
    let mut wsum = 0.0;
    for j in -1..=2i32 {
        let wy = cubic_weight(j, ty);
        if wy == 0.0 {
            continue;
        }
        for i in -1..=2i32 {
            let wx = cubic_weight(i, tx);
            if wx == 0.0 {
                continue;
            }
            let sx = x0 + i;
            let sy = y0 + j;
            if sx < 0 || sy < 0 || sx >= src.w || sy >= src.h {
                continue;
            }
            let w = wx * wy;
            let idx = ((sy * src.w + sx) * 4) as usize;
            for k in 0..4 {
                acc[k] += src.px[idx + k] as f64 * w;
            }
            wsum += w;
        }
    }
    if wsum > 0.0001 {
        for k in 0..4 {
            acc[k] /= wsum;
        }
    }
    acc
}

#[allow(dead_code)]
fn cubic_weight(i: i32, t: f64) -> f64 {
    match i {
        -1 => -0.5 * t * t * t + t * t - 0.5 * t,
        0 => 1.5 * t * t * t - 2.5 * t * t + 1.0,
        1 => -1.5 * t * t * t + 2.0 * t * t + 0.5 * t,
        _ => 0.5 * t * t * t - 0.5 * t * t,
    }
}

/// Fast O(W*H) Kovesi 3-pass sliding-window separable box blur approximating
/// Gaussian blur with `sigma = radius.max(0.5)`.
pub fn gaussian_blur(layer: &mut RgbaLayer, radius: f64) {
    if radius < 0.3 || layer.w < 2 || layer.h < 2 {
        return;
    }
    let sigma = radius.max(0.5);
    let w_ideal = ((12.0 * sigma * sigma / 3.0) + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    wl = wl.max(1);
    let wu = wl + 2;
    let m_ideal = (12.0 * sigma * sigma - 3.0 * (wl * wl) as f64 - 12.0 * wl as f64 - 9.0)
        / (-4.0 * wl as f64 - 4.0);
    let m = m_ideal.round().clamp(0.0, 3.0) as usize;

    let w = layer.w as usize;
    let h = layer.h as usize;
    let mut tmp = vec![0u8; layer.px.len()];

    for pass in 0..3 {
        let box_w = if pass < m { wl } else { wu };
        if box_w <= 1 {
            continue;
        }
        let r = (box_w - 1) / 2;
        box_blur_h(&layer.px, &mut tmp, w, h, r);
        box_blur_v(&tmp, &mut layer.px, w, h, r);
    }
}

fn box_blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: i32) {
    let denom = r * 2 + 1;
    let half = denom / 2;
    let wm1 = w as i32 - 1;
    for y in 0..h {
        let row_off = y * w * 4;
        let mut acc = [0i32; 4];
        for k in -r..=r {
            let sx = k.clamp(0, wm1) as usize;
            let idx = row_off + sx * 4;
            acc[0] += src[idx] as i32;
            acc[1] += src[idx + 1] as i32;
            acc[2] += src[idx + 2] as i32;
            acc[3] += src[idx + 3] as i32;
        }
        for x in 0..w {
            let di = row_off + x * 4;
            dst[di] = ((acc[0] + half) / denom) as u8;
            dst[di + 1] = ((acc[1] + half) / denom) as u8;
            dst[di + 2] = ((acc[2] + half) / denom) as u8;
            dst[di + 3] = ((acc[3] + half) / denom) as u8;

            let sub_x = (x as i32 - r).clamp(0, wm1) as usize;
            let add_x = (x as i32 + r + 1).clamp(0, wm1) as usize;
            let si = row_off + sub_x * 4;
            let ai = row_off + add_x * 4;
            acc[0] += src[ai] as i32 - src[si] as i32;
            acc[1] += src[ai + 1] as i32 - src[si + 1] as i32;
            acc[2] += src[ai + 2] as i32 - src[si + 2] as i32;
            acc[3] += src[ai + 3] as i32 - src[si + 3] as i32;
        }
    }
}

fn box_blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: i32) {
    let denom = r * 2 + 1;
    let half = denom / 2;
    let hm1 = h as i32 - 1;
    let row_stride = w * 4;
    for x in 0..w {
        let col_off = x * 4;
        let mut acc = [0i32; 4];
        for k in -r..=r {
            let sy = k.clamp(0, hm1) as usize;
            let idx = sy * row_stride + col_off;
            acc[0] += src[idx] as i32;
            acc[1] += src[idx + 1] as i32;
            acc[2] += src[idx + 2] as i32;
            acc[3] += src[idx + 3] as i32;
        }
        for y in 0..h {
            let di = y * row_stride + col_off;
            dst[di] = ((acc[0] + half) / denom) as u8;
            dst[di + 1] = ((acc[1] + half) / denom) as u8;
            dst[di + 2] = ((acc[2] + half) / denom) as u8;
            dst[di + 3] = ((acc[3] + half) / denom) as u8;

            let sub_y = (y as i32 - r).clamp(0, hm1) as usize;
            let add_y = (y as i32 + r + 1).clamp(0, hm1) as usize;
            let si = sub_y * row_stride + col_off;
            let ai = add_y * row_stride + col_off;
            acc[0] += src[ai] as i32 - src[si] as i32;
            acc[1] += src[ai + 1] as i32 - src[si + 1] as i32;
            acc[2] += src[ai + 2] as i32 - src[si + 2] as i32;
            acc[3] += src[ai + 3] as i32 - src[si + 3] as i32;
        }
    }
}

/// Error function approximation (Abramowitz & Stegun 7.1.26, max err < 1.5e-7).
#[inline]
fn erf_approx(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let ax = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * ax);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-ax * ax).exp();
    sign * y
}

/// Apply analytic Gaussian-softened circular pupil alpha cutout in-place over
/// the tight bounding box around `(icx, icy)` instead of blurring a full buffer.
fn apply_radial_pupil_cut(
    layers: &mut [&mut RgbaLayer],
    icx: i32,
    icy: i32,
    pr_clear: f64,
    sigma: f64,
) {
    let r_max = (pr_clear + sigma * 3.5).ceil() as i32;
    let inv_sig = 1.0 / (sigma.max(0.5) * SQRT_2);
    for layer in layers.iter_mut() {
        let w = layer.w;
        let h = layer.h;
        let y0 = (icy - r_max).max(0);
        let y1 = (icy + r_max).min(h - 1);
        let x0 = (icx - r_max).max(0);
        let x1 = (icx + r_max).min(w - 1);
        for y in y0..=y1 {
            let dy = (y - icy) as f64;
            let row = (y * w) as usize;
            for x in x0..=x1 {
                let dx = (x - icx) as f64;
                let dist = dx.hypot(dy);
                let t = (0.5 * (1.0 + erf_approx((dist - pr_clear) * inv_sig))).clamp(0.0, 1.0);
                if t >= 0.9999 {
                    continue;
                }
                let ai = (row + x as usize) * 4 + 3;
                layer.px[ai] = ((layer.px[ai] as f64 * t + 0.5) as u32).min(255) as u8;
            }
        }
    }
}

/// Build the analytic `cutout` texture at capped resolution (`128..=512`),
/// matching Gaussian blur of `fill_ellipse(cr, [0,0,0,10])` on white `[255,255,255,255]`.
fn build_analytic_cutout(cutout_dim: i32, geom: &EyeCanvasGeometry) -> RgbaLayer {
    let tex_dim = (cutout_dim / 2).clamp(128, 512);
    let mut cutout = RgbaLayer::new(tex_dim, tex_dim);
    let ratio = tex_dim as f64 / cutout_dim.max(1) as f64;
    let cr_tex = (geom.iris_radius * 0.97) * ratio;
    let sigma_tex = (((7.0 * geom.scale) as i32).max(3) as f64) * ratio;
    let inv_sig = 1.0 / (sigma_tex.max(0.5) * SQRT_2);
    let cc = tex_dim as f64 * 0.5;
    let w = tex_dim as usize;
    for y in 0..tex_dim {
        let dy = y as f64 + 0.5 - cc;
        let row = y as usize * w;
        for x in 0..tex_dim {
            let dx = x as f64 + 0.5 - cc;
            let dist = dx.hypot(dy);
            let t = (0.5 * (1.0 + erf_approx((dist - cr_tex) * inv_sig))).clamp(0.0, 1.0);
            let rgb = (255.0 * t + 0.5).clamp(0.0, 255.0) as u8;
            let a = (10.0 + 245.0 * t + 0.5).clamp(0.0, 255.0) as u8;
            let idx = (row + x as usize) * 4;
            cutout.px[idx] = rgb;
            cutout.px[idx + 1] = rgb;
            cutout.px[idx + 2] = rgb;
            cutout.px[idx + 3] = a;
        }
    }
    cutout
}

/// Downsample `rings_a` by 2x (box average) into half-res `glow`, apply fast
/// O(W*H) blur, and clear outer 1 px border.
fn build_half_res_glow(rings_a: &RgbaLayer, scale: f64) -> RgbaLayer {
    let gw = (rings_a.w / 2).max(2);
    let gh = (rings_a.h / 2).max(2);
    let mut glow = RgbaLayer::new(gw, gh);
    let sw = rings_a.w as usize;
    for y in 0..gh as usize {
        let sy = y * 2;
        for x in 0..gw as usize {
            let sx = x * 2;
            let i00 = (sy * sw + sx) * 4;
            let i10 = i00 + 4;
            let i01 = i00 + sw * 4;
            let i11 = i01 + 4;
            let di = (y * gw as usize + x) * 4;
            for c in 0..4 {
                let sum = rings_a.px[i00 + c] as u32
                    + rings_a.px[i10 + c] as u32
                    + rings_a.px[i01 + c] as u32
                    + rings_a.px[i11 + c] as u32;
                glow.px[di + c] = ((sum + 2) / 4) as u8;
            }
        }
    }
    let sigma = ((10.0 * scale) as i32).max(4) as f64 * 0.5;
    gaussian_blur(&mut glow, sigma);
    glow.clear_outer_border();
    glow
}

/// PIL `ImageDraw.polygon` (even-odd scanline fill, no AA — like PIL).
pub fn fill_polygon(layer: &mut RgbaLayer, pts: &[(f64, f64)], color: [u8; 4]) {
    if pts.len() < 3 {
        return;
    }
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    for p in pts {
        min_y = min_y.min(p.1);
        max_y = max_y.max(p.1);
    }
    let y_start = min_y.ceil().max(0.0) as i32;
    let y_end = max_y.floor().min(layer.h as f64 - 1.0) as i32;
    let mut xs: Vec<f64> = Vec::with_capacity(16);
    for y in y_start..=y_end {
        let yc = y as f64 + 0.5;
        xs.clear();
        let n = pts.len();
        for i in 0..n {
            let (x1, y1) = pts[i];
            let (x2, y2) = pts[(i + 1) % n];
            if (y1 <= yc && y2 > yc) || (y2 <= yc && y1 > yc) {
                let t = (yc - y1) / (y2 - y1);
                xs.push(x1 + t * (x2 - x1));
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut i = 0;
        while i + 1 < xs.len() {
            let xa = xs[i].ceil().max(0.0) as i32;
            let xb = xs[i + 1].floor().min(layer.w as f64 - 1.0) as i32;
            for x in xa..=xb {
                if x >= 0 && x < layer.w && y >= 0 && y < layer.h {
                    let idx = ((y * layer.w + x) * 4) as usize;
                    layer.px[idx..idx + 4].copy_from_slice(&color);
                }
            }
            i += 2;
        }
    }
}

/// PIL `ImageDraw.ellipse` (hard edges, like PIL).
pub fn fill_ellipse(layer: &mut RgbaLayer, cx: i32, cy: i32, r: i32, color: [u8; 4]) {
    if r <= 0 {
        return;
    }
    let rf = r as f64;
    for dy in -r..=r {
        let y = cy + dy;
        if y < 0 || y >= layer.h {
            continue;
        }
        let t = 1.0 - (dy * dy) as f64 / (rf * rf);
        if t <= 0.0 {
            continue;
        }
        let dx = (t.sqrt() * rf).floor() as i32;
        let xa = (cx - dx).max(0);
        let xb = (cx + dx).min(layer.w - 1);
        for x in xa..=xb {
            let idx = ((y * layer.w + x) * 4) as usize;
            layer.px[idx..idx + 4].copy_from_slice(&color);
        }
    }
}

fn linspace(a: f64, b: f64, n: usize) -> Vec<f64> {
    if n == 1 {
        return vec![a];
    }
    let step = (b - a) / (n - 1) as f64;
    (0..n).map(|i| a + i as f64 * step).collect()
}

/// `np.gradient` with a scalar sample distance (edge_order=1).
fn gradient(y: &[f64], dx: f64) -> Vec<f64> {
    let n = y.len();
    let mut g = vec![0.0; n];
    if n == 1 {
        return g;
    }
    g[0] = (y[1] - y[0]) / dx;
    g[n - 1] = (y[n - 1] - y[n - 2]) / dx;
    for i in 1..n - 1 {
        g[i] = (y[i + 1] - y[i - 1]) / (2.0 * dx);
    }
    g
}

// ============================================================================
// Rim atlas (shelf packer with direct OBB rasterization & vertical crop)
// ============================================================================

struct PixelSlot {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

struct RimAtlasBuilder {
    layer: RgbaLayer,
    cx: i32,
    cy: i32,
    row_h: i32,
}

impl RimAtlasBuilder {
    fn new(w: i32, h: i32) -> Self {
        Self {
            layer: RgbaLayer::new(w, h),
            cx: 1,
            cy: 1,
            row_h: 0,
        }
    }

    fn alloc_slot(&mut self, sw: i32, sh: i32) -> Option<PixelSlot> {
        if sw + 2 > self.layer.w || sh + 2 > self.layer.h {
            return None;
        }
        if self.cx + sw + 1 >= self.layer.w {
            self.cx = 1;
            self.cy += self.row_h + 1;
            self.row_h = 0;
        }
        if self.cy + sh + 1 >= self.layer.h {
            return None;
        }
        let slot = PixelSlot {
            x: self.cx,
            y: self.cy,
            w: sw,
            h: sh,
        };
        self.cx += sw + 1;
        self.row_h = self.row_h.max(sh);
        Some(slot)
    }

    fn finish_cropped(mut self) -> (RgbaLayer, i32) {
        let used_h = (self.cy + self.row_h + 2).min(self.layer.h).max(4);
        let row_bytes = self.layer.w as usize * 4;
        self.layer.px.truncate(used_h as usize * row_bytes);
        self.layer.h = used_h;
        self.layer.clear_outer_border();
        (self.layer, used_h)
    }
}

// ============================================================================
// Two-phase deterministic layout plans
// ============================================================================

struct SingleWord {
    text: &'static str,
    cx: f64,
    cy: f64,
    fsize: i32,
    angle: f64,
    color: [u8; 4],
    bold: bool,
    min_y: i32,
    max_y: i32,
}

struct DualWord {
    text: &'static str,
    cx: f64,
    cy: f64,
    fsize: i32,
    angle: f64,
    col_a: [u8; 4],
    col_b: [u8; 4],
    bold: bool,
}

struct RimPlan {
    phrase: &'static str,
    u: f64,
    v_offset: f64,
    is_upper: bool,
    fsize: i32,
    color: [u8; 4],
    angles: [f64; 3], // closed, mid, open
}

/// Phase 1 helper: measure word sprite dims cleanly & warm its glyph cache.
fn plan_word_dims(
    baker: &mut EyeBaker,
    text: &str,
    fsize: i32,
    bold: bool,
) -> (i32, i32, f64) {
    baker.warm_word(text, fsize, bold);
    let (x0, y0, x1, y1) = ink_bbox_cached(&baker.glyphs, text, fsize, bold);
    let sw = ((x1 - x0).ceil() as i32).max(1) + 4;
    let sh = ((y1 - y0).ceil() as i32).max(1) + 4;
    let w_px = (x1 - x0).max(4.0);
    (sw, sh, w_px)
}

struct BakePlan {
    geom: EyeCanvasGeometry,
    cutout_dim: i32,
    iris_dim: i32,
    pupil_dim: i32,
    palette: &'static EyePalette,

    field_words: Vec<SingleWord>,
    sclera_poly: Vec<(f64, f64)>,
    sclera_words: Vec<SingleWord>,
    ring_words: Vec<DualWord>,
    radial_words: Vec<DualWord>,
    pupil_words: Vec<SingleWord>,
    highlight_words: Vec<SingleWord>,
    rim_plans: Vec<RimPlan>,
    word_count: usize,
}

impl EyeBaker {
    /// Phase 1 (single-threaded): execute every `PyRng` draw and font measurement
    /// in canonical order, warming the glyph cache and producing placement plans.
    fn plan_layout(
        &mut self,
        geom_orig: &EyeCanvasGeometry,
        bake_scale: f64,
        palette_idx: usize,
        seed: u32,
    ) -> BakePlan {
        let palette = &PALETTES[palette_idx.min(PALETTES.len() - 1)];
        let geom = scale_geometry(geom_orig, bake_scale);
        let mut rng = PyRng::new(if seed == 0 { 0 } else { seed } as u64);
        let mut word_count = 0usize;

        // ------------------------------------------------------------------
        // A. FULL-CANVAS WALL OF POETRY
        // ------------------------------------------------------------------
        let mut field_words = Vec::with_capacity(900);
        let row_spacing = (18.0 * geom.scale).max(16.0);
        let max_y_extent = geom.canvas_h as f64 * 0.54;
        let stop = max_y_extent + row_spacing;
        let mut y_offsets: Vec<f64> = Vec::new();
        let mut ri = 0;
        loop {
            let yo = -max_y_extent + ri as f64 * row_spacing;
            if yo >= stop {
                break;
            }
            y_offsets.push(yo);
            ri += 1;
        }

        for (r_idx, &base_y_off) in y_offsets.iter().enumerate() {
            let fsize = if r_idx % 4 == 0 {
                ((12.8 * geom.scale) as i32).max(10)
            } else if r_idx % 2 == 0 {
                ((11.4 * geom.scale) as i32).max(9)
            } else {
                ((10.2 * geom.scale) as i32).max(8)
            };

            let mut x_cursor = 6.0 + rng.uniform(0.0, 18.0);
            while x_cursor < geom.canvas_w as f64 - 8.0 {
                let text = generate_meaningful_quotes(&mut rng, "mixed");
                let mut is_bold = r_idx % 5 == 0;
                let (w_px, _) = self.measure(text, fsize, is_bold);
                let gap_px = (12.0 * geom.scale).max(10.0);
                let wx = x_cursor + w_px * 0.5;

                if wx - w_px * 0.5 > geom.canvas_w as f64 {
                    break;
                }

                let ys = [
                    eval_field_streamline_y(wx - 12.0, base_y_off, &geom),
                    eval_field_streamline_y(wx, base_y_off, &geom),
                    eval_field_streamline_y(wx + 12.0, base_y_off, &geom),
                ];
                let wy = ys[1];

                if wy >= -20.0 && wy <= geom.canvas_h as f64 + 20.0 {
                    let slope = (ys[2] - ys[0]) / 24.0;
                    let angle = -slope.atan().to_degrees();

                    let u_rel = (wx - geom.center_x as f64) / geom.eye_half_w;
                    let v_rel = (wy - geom.center_y as f64) / geom.eye_half_h;
                    let elliptical_dist = (u_rel * 0.88).hypot(v_rel * 0.78);

                    let (yu_open, yl_open) = eval_eyelid_curves(u_rel, 1.0, &geom);
                    let (y_closed, _) = eval_eyelid_curves(u_rel, 0.0, &geom);
                    let dy_from_center = wy - geom.center_y as f64;

                    let col: [u8; 3];
                    let shade: f64;
                    let alpha: i64;
                    if u_rel.abs() <= 1.03
                        && dy_from_center >= yu_open - 46.0 * geom.scale
                        && dy_from_center <= yl_open + 42.0 * geom.scale
                    {
                        let dist_to_closed_seam = (dy_from_center - y_closed).abs();
                        let dist_to_upper_open = (dy_from_center - yu_open).abs();
                        let dist_to_lower_open = (dy_from_center - yl_open).abs();

                        if dist_to_closed_seam < 15.0 * geom.scale {
                            col = *rng.choice(palette.eyelid_rim);
                            shade = 1.0;
                            alpha = 252;
                            is_bold = true;
                        } else if dist_to_upper_open < 24.0 * geom.scale
                            || dist_to_lower_open < 22.0 * geom.scale
                        {
                            col = *rng.choice(palette.eyelid_rim);
                            shade = 0.92;
                            alpha = rng.randint(225, 250);
                            is_bold = true;
                        } else {
                            col = *rng.choice(palette.eyelid_sheet);
                            shade = (1.0 - 0.25 * u_rel.abs()).max(0.68);
                            alpha = rng.randint(205, 240);
                        }
                    } else if elliptical_dist < 1.58 {
                        col = *rng.choice(palette.bg_field_orbital);
                        let t_halo = ((elliptical_dist - 1.0) / 0.58).max(0.0);
                        shade = 1.0 - 0.24 * t_halo;
                        alpha = rng.randint(205, 240);
                    } else {
                        col = *rng.choice(palette.bg_field_dark);
                        let wave = 0.88 + 0.12 * (u_rel * 2.0 + v_rel * 1.6).sin();
                        shade = wave;
                        alpha = rng.randint(200, 238);
                    }

                    // Re-warm AFTER any `is_bold = true` override so bold glyphs exist in Phase 2!
                    let (sw, sh, _) = plan_word_dims(self, text, fsize, is_bold);
                    let (min_y, max_y) = word_y_bounds(wy, sw, sh, angle);
                    let r = ((col[0] as f64 * shade).min(255.0)) as u8;
                    let g = ((col[1] as f64 * shade).min(255.0)) as u8;
                    let b = ((col[2] as f64 * shade).min(255.0)) as u8;
                    field_words.push(SingleWord {
                        text,
                        cx: wx,
                        cy: wy,
                        fsize,
                        angle,
                        color: [r, g, b, alpha.clamp(0, 255) as u8],
                        bold: is_bold,
                        min_y,
                        max_y,
                    });
                    word_count += 1;
                }

                x_cursor += w_px + gap_px;
            }
        }

        // ------------------------------------------------------------------
        // B. SCLERA TYPOGRAPHY
        // ------------------------------------------------------------------
        let u_poly = linspace(-1.0, 1.0, 96);
        let mut sclera_poly: Vec<(f64, f64)> = Vec::with_capacity(192);
        for &u in &u_poly {
            let (yu, _) = eval_eyelid_curves(u, 1.0, &geom);
            sclera_poly.push((
                geom.center_x as f64 + u * geom.eye_half_w,
                geom.center_y as f64 + yu,
            ));
        }
        for &u in u_poly.iter().rev() {
            let (_, yl) = eval_eyelid_curves(u, 1.0, &geom);
            sclera_poly.push((
                geom.center_x as f64 + u * geom.eye_half_w,
                geom.center_y as f64 + yl,
            ));
        }

        let mut sclera_words = Vec::with_capacity(220);
        let n_sclera_rows = 28;
        for row_idx in 0..n_sclera_rows {
            let v_frac = (row_idx as f64 + 0.5) / n_sclera_rows as f64;
            let fsize = (((10.2 + 3.6 * (v_frac * PI).sin()) * geom.scale) as i32).max(9);
            let mut u_cursor = -0.97 + rng.uniform(0.0, 0.025);

            while u_cursor < 0.97 {
                let mode = if u_cursor.abs() > 0.74 { "short" } else { "mixed" };
                let quote = generate_meaningful_quotes(&mut rng, mode);
                let is_bold = row_idx % 4 == 0;
                let (sw, sh, w_px) = plan_word_dims(self, quote, fsize, is_bold);
                let du = (w_px + 8.0 * geom.scale) / geom.eye_half_w;
                let u_mid = u_cursor + du * 0.5;
                if u_mid > 0.97 {
                    break;
                }

                let um = [u_mid - 0.025, u_mid, u_mid + 0.025];
                let mut yu3 = [0.0; 3];
                let mut yl3 = [0.0; 3];
                for k in 0..3 {
                    let (a, b) = eval_eyelid_curves(um[k], 1.0, &geom);
                    yu3[k] = a;
                    yl3[k] = b;
                }
                let (yu, yl) = (yu3[1], yl3[1]);
                if yl - yu < fsize as f64 * 0.75 {
                    u_cursor += du * 0.65;
                    continue;
                }

                let wy_rel = yu + v_frac * (yl - yu);
                let wx_rel = u_mid * geom.eye_half_w;
                let y0 = yu3[0] + v_frac * (yl3[0] - yu3[0]);
                let y2 = yu3[2] + v_frac * (yl3[2] - yu3[2]);
                let slope = (y2 - y0) / (0.05 * geom.eye_half_w);
                let angle = -slope.atan().to_degrees();

                let dist_x = u_mid.abs();
                let dist_y = (v_frac - 0.52).abs() * 1.7;
                let sphere_shade =
                    (1.16 - 0.62 * dist_x.powf(1.55) - 0.22 * dist_y.powf(1.5)).clamp(0.38, 1.0);
                let lid_shadow = (0.56 + 0.70 * v_frac).min(1.0);
                let brightness = sphere_shade * lid_shadow;

                let base_col = *rng.choice(palette.sclera_colors);
                let r = ((base_col[0] as f64 * brightness).min(255.0)) as u8;
                let g = ((base_col[1] as f64 * brightness).min(255.0)) as u8;
                let b = ((base_col[2] as f64 * brightness).min(255.0)) as u8;
                let alpha = ((205.0 + 50.0 * brightness) as i32).min(255) as u8;

                let cx = geom.center_x as f64 + wx_rel;
                let cy = geom.center_y as f64 + wy_rel;
                let (min_y, max_y) = word_y_bounds(cy, sw, sh, angle);
                sclera_words.push(SingleWord {
                    text: quote,
                    cx,
                    cy,
                    fsize,
                    angle,
                    color: [r, g, b, alpha],
                    bold: is_bold,
                    min_y,
                    max_y,
                });
                word_count += 1;
                u_cursor += du;
            }
        }

        let cutout_dim = (geom.iris_radius * 2.25) as i32;

        // ------------------------------------------------------------------
        // C. IRIS CONCENTRIC RINGS + RADIAL SPOKES + PUPIL + HIGHLIGHTS
        // ------------------------------------------------------------------
        let mut iris_dim = (geom.iris_radius * 2.38) as i32;
        if iris_dim % 2 == 1 {
            iris_dim += 1;
        }
        let icx = iris_dim / 2;
        let icy = iris_dim / 2;

        let mut ring_words = Vec::with_capacity(240);
        let ring_fracs = linspace(0.96, 0.37, 15);
        for (r_idx, &r_frac) in ring_fracs.iter().enumerate() {
            let rad = geom.iris_radius * r_frac;
            let fsize = (((8.8 + (1.0 - r_frac) * 4.2) * geom.scale) as i32).max(8);
            let is_bold = r_idx % 3 == 0;
            let mut theta_cur = rng.uniform(0.0, TAU);
            let theta_end = theta_cur + TAU - 0.03;

            while theta_cur < theta_end {
                let mut word = generate_meaningful_quotes(
                    &mut rng,
                    if r_frac < 0.58 { "short" } else { "mixed" },
                );
                if word.len() > 22 {
                    word = generate_meaningful_quotes(&mut rng, "short");
                }
                let (_, _, w_px) = plan_word_dims(self, word, fsize, is_bold);
                let d_theta = (w_px + 6.0 * geom.scale) / rad.max(24.0);
                let theta_mid = theta_cur + d_theta * 0.5;
                if theta_mid > theta_end {
                    break;
                }

                let wx = icx as f64 + rad * theta_mid.cos();
                let wy = icy as f64 + rad * theta_mid.sin();
                let mut deg_tangent = -theta_mid.to_degrees() - 90.0;
                let m = deg_tangent.rem_euclid(360.0);
                if m > 90.0 && m < 270.0 {
                    deg_tangent += 180.0;
                }

                let col_a: [u8; 3];
                let col_b: [u8; 3];
                let alpha: u8;
                if r_frac > 0.84 {
                    col_a = *rng.choice(palette.iris_outer);
                    col_b = *rng.choice(palette.iris_mid);
                    alpha = 248;
                } else if r_frac > 0.48 {
                    col_a = *rng.choice(palette.iris_mid);
                    col_b = *rng.choice(palette.iris_alt);
                    alpha = 252;
                } else {
                    col_a = *rng.choice(palette.iris_inner);
                    col_b = *rng.choice(palette.iris_alt);
                    alpha = 255;
                }

                ring_words.push(DualWord {
                    text: word,
                    cx: wx,
                    cy: wy,
                    fsize,
                    angle: deg_tangent,
                    col_a: [col_a[0], col_a[1], col_a[2], alpha],
                    col_b: [col_b[0], col_b[1], col_b[2], alpha],
                    bold: is_bold,
                });
                word_count += 1;
                theta_cur += d_theta;
            }
        }

        let mut radial_words = Vec::with_capacity(84);
        let n_spokes = 84;
        for s_idx in 0..n_spokes {
            let theta = (s_idx as f64 / n_spokes as f64) * TAU + rng.uniform(-0.015, 0.015);
            let r_band = rng.uniform(0.46, 0.84);
            let rad = geom.iris_radius * r_band;
            let wx = icx as f64 + rad * theta.cos();
            let wy = icy as f64 + rad * theta.sin();

            let mut deg_radial = -theta.to_degrees();
            let m = deg_radial.rem_euclid(360.0);
            if m > 90.0 && m < 270.0 {
                deg_radial += 180.0;
            }

            let word = generate_meaningful_quotes(&mut rng, "short");
            let fsize = ((rng.uniform(8.2, 11.0) * geom.scale) as i32).max(8);
            let col_a = if r_band < 0.60 {
                *rng.choice(palette.iris_inner)
            } else {
                *rng.choice(palette.iris_mid)
            };
            let col_b = *rng.choice(palette.iris_alt);
            let alpha = rng.randint(165, 225) as u8;
            self.warm_word(word, fsize, false);

            radial_words.push(DualWord {
                text: word,
                cx: wx,
                cy: wy,
                fsize,
                angle: deg_radial,
                col_a: [col_a[0], col_a[1], col_a[2], alpha],
                col_b: [col_b[0], col_b[1], col_b[2], alpha],
                bold: false,
            });
            word_count += 1;
        }

        let mut pupil_dim = (geom.pupil_radius * 2.45) as i32;
        if pupil_dim % 2 == 1 {
            pupil_dim += 1;
        }
        let pcx = pupil_dim / 2;
        let pcy = pupil_dim / 2;
        let mut pupil_words = Vec::with_capacity(64);
        for &p_frac in &[0.88, 0.72, 0.56, 0.40, 0.24] {
            let rad_p = geom.pupil_radius * p_frac;
            let fsize = (((8.2 + (1.0 - p_frac) * 1.8) * geom.scale) as i32).max(7);
            let mut th_cur = rng.uniform(0.0, TAU);
            let th_end = th_cur + TAU - 0.06;
            while th_cur < th_end {
                let word = generate_meaningful_quotes(&mut rng, "pupil");
                let (w_px, _) = self.measure(word, fsize, false);
                let d_th = ((w_px + 4.5 * geom.scale) / rad_p.max(12.0)).max(0.32);
                let th_mid = th_cur + d_th * 0.5;
                if th_mid > th_end {
                    break;
                }
                let wx = pcx as f64 + rad_p * th_mid.cos();
                let wy = pcy as f64 + rad_p * th_mid.sin();
                let mut deg = -th_mid.to_degrees() - 90.0;
                let m = deg.rem_euclid(360.0);
                if m > 90.0 && m < 270.0 {
                    deg += 180.0;
                }
                let col = *rng.choice(palette.pupil_words);
                let vis = [
                    (col[0] as i32 + 26).min(255) as u8,
                    (col[1] as i32 + 40).min(255) as u8,
                    (col[2] as i32 + 68).min(255) as u8,
                ];
                pupil_words.push(SingleWord {
                    text: word,
                    cx: wx,
                    cy: wy,
                    fsize,
                    angle: deg,
                    color: [vis[0], vis[1], vis[2], 255],
                    bold: false,
                    min_y: 0,
                    max_y: pupil_dim,
                });
                word_count += 1;
                th_cur += d_th;
            }
        }
        let center_word = *rng.choice(&["consciousness", "look within", "existence", "eternity"]);
        let cw_fsize = ((9.2 * geom.scale) as i32).max(8);
        self.warm_word(center_word, cw_fsize, true);
        pupil_words.push(SingleWord {
            text: center_word,
            cx: pcx as f64,
            cy: pcy as f64,
            fsize: cw_fsize,
            angle: 0.0,
            color: [62, 92, 148, 255],
            bold: true,
            min_y: 0,
            max_y: pupil_dim,
        });
        word_count += 1;

        let mut highlight_words = Vec::with_capacity(20);
        let hl1_cx = icx as f64 - geom.iris_radius * 0.34;
        let hl1_cy = icy as f64 - geom.iris_radius * 0.32;
        for row_h in -2i32..=2 {
            for col_h in -1i32..=1 {
                let word = generate_meaningful_quotes(&mut rng, "highlight");
                let fsize = (((10.5 - (row_h as f64).abs() * 0.5) * geom.scale) as i32).max(8);
                let col = *rng.choice(palette.highlight_words);
                let dx_h = col_h as f64 * 34.0 * geom.scale;
                let dy_h = row_h as f64 * 14.0 * geom.scale;
                let ang_rad = (-24.0f64).to_radians();
                let rx = dx_h * ang_rad.cos() - dy_h * ang_rad.sin();
                let ry = dx_h * ang_rad.sin() + dy_h * ang_rad.cos();
                let alpha =
                    (255 - ((row_h as i32).abs() * 18 + (col_h as i32).abs() * 16)).max(195) as u8;
                self.warm_word(word, fsize, true);
                highlight_words.push(SingleWord {
                    text: word,
                    cx: hl1_cx + rx,
                    cy: hl1_cy + ry,
                    fsize,
                    angle: -24.0,
                    color: [col[0], col[1], col[2], alpha],
                    bold: true,
                    min_y: 0,
                    max_y: iris_dim,
                });
                word_count += 1;
            }
        }
        let hl2_cx = icx as f64 + geom.iris_radius * 0.36;
        let hl2_cy = icy as f64 + geom.iris_radius * 0.35;
        for k_h in -2i32..=2 {
            let word = generate_meaningful_quotes(&mut rng, "highlight");
            let fsize = ((9.0 * geom.scale) as i32).max(7);
            let col = *rng.choice(palette.highlight_words);
            let dx_h = k_h as f64 * 28.0 * geom.scale;
            let dy_h = -(k_h as f64).abs() * 3.5 * geom.scale;
            let ang_rad = (-28.0f64).to_radians();
            let rx = dx_h * ang_rad.cos() - dy_h * ang_rad.sin();
            let ry = dx_h * ang_rad.sin() + dy_h * ang_rad.cos();
            self.warm_word(word, fsize, true);
            highlight_words.push(SingleWord {
                text: word,
                cx: hl2_cx + rx,
                cy: hl2_cy + ry,
                fsize,
                angle: -28.0,
                color: [col[0], col[1], col[2], 225],
                bold: true,
                min_y: 0,
                max_y: iris_dim,
            });
            word_count += 1;
        }

        // ------------------------------------------------------------------
        // D. EYELID-RIM WORD STREAM
        // ------------------------------------------------------------------
        let mut rim_plans = Vec::with_capacity(48);
        let u_grid = linspace(-1.02, 1.02, 160);
        let dx_grid = (u_grid[1] - u_grid[0]) * geom.eye_half_w;
        let n_grid = u_grid.len();
        let mut yu_open = Vec::with_capacity(n_grid);
        let mut yl_open = Vec::with_capacity(n_grid);
        let mut yu_mid = Vec::with_capacity(n_grid);
        let mut yl_mid = Vec::with_capacity(n_grid);
        let mut yu_closed = Vec::with_capacity(n_grid);
        let mut yl_closed = Vec::with_capacity(n_grid);
        for &u in &u_grid {
            let (a, b) = eval_eyelid_curves(u, 1.0, &geom);
            yu_open.push(a);
            yl_open.push(b);
            let (a, b) = eval_eyelid_curves(u, 0.45, &geom);
            yu_mid.push(a);
            yl_mid.push(b);
            let (a, b) = eval_eyelid_curves(u, 0.0, &geom);
            yu_closed.push(a);
            yl_closed.push(b);
        }
        let slope_u_open = gradient(&yu_open, dx_grid);
        let slope_u_mid = gradient(&yu_mid, dx_grid);
        let slope_u_closed = gradient(&yu_closed, dx_grid);
        let slope_l_open = gradient(&yl_open, dx_grid);
        let slope_l_mid = gradient(&yl_mid, dx_grid);
        let slope_l_closed = gradient(&yl_closed, dx_grid);
        let idx_of = |u: f64| -> usize {
            ((((u + 1.02) / 2.04) * (n_grid as f64 - 1.0)).clamp(0.0, n_grid as f64 - 1.0)) as usize
        };

        let mut plan_rim_stream = |is_upper: bool, v_off: f64, fsize: i32| {
            let mut u_cur = -0.98 + rng.uniform(0.0, 0.02);
            while u_cur < 0.98 {
                let mut phrase = generate_meaningful_quotes(
                    &mut rng,
                    if u_cur.abs() > 0.65 { "short" } else { "mixed" },
                );
                if phrase.len() > 22 {
                    phrase = generate_meaningful_quotes(&mut rng, "short");
                }
                let (w_px, _) = self.measure(phrase, fsize, true);

                let idx_est = idx_of(u_cur);
                let local_slope = if is_upper {
                    slope_u_open[idx_est]
                } else {
                    slope_l_open[idx_est]
                };
                let cos_theta = 1.0 / (1.0 + local_slope * local_slope).sqrt();
                let du = ((w_px + 12.0 * geom.scale) * cos_theta.max(0.65)) / geom.eye_half_w;
                let u_mid = u_cur + du * 0.5;
                if u_mid > 0.98 {
                    break;
                }

                let idx = idx_of(u_mid);
                let (ang_open, ang_mid, ang_closed) = if is_upper {
                    (
                        -slope_u_open[idx].atan().to_degrees(),
                        -slope_u_mid[idx].atan().to_degrees(),
                        -slope_u_closed[idx].atan().to_degrees(),
                    )
                } else {
                    (
                        -slope_l_open[idx].atan().to_degrees(),
                        -slope_l_mid[idx].atan().to_degrees(),
                        -slope_l_closed[idx].atan().to_degrees(),
                    )
                };

                let col = *rng.choice(palette.eyelid_rim);
                let edge_atten = (1.0 - 0.50 * u_mid.abs().powf(2.4)).max(0.45);
                let alpha = (250.0 * edge_atten).clamp(0.0, 255.0) as u8;
                self.warm_word(phrase, fsize, true);

                rim_plans.push(RimPlan {
                    phrase,
                    u: u_mid,
                    v_offset: v_off,
                    is_upper,
                    fsize,
                    color: [col[0], col[1], col[2], alpha],
                    angles: [ang_closed, ang_mid, ang_open],
                });
                word_count += 1;
                u_cur += du;
            }
        };

        plan_rim_stream(true, -9.0 * geom.scale, ((12.2 * geom.scale) as i32).max(10));
        plan_rim_stream(false, 9.0 * geom.scale, ((11.6 * geom.scale) as i32).max(10));

        BakePlan {
            geom,
            cutout_dim,
            iris_dim,
            pupil_dim,
            palette,
            field_words,
            sclera_poly,
            sclera_words,
            ring_words,
            radial_words,
            pupil_words,
            highlight_words,
            rim_plans,
            word_count,
        }
    }

    pub fn bake(
        &mut self,
        key: &BakeKey,
        geom_orig: &EyeCanvasGeometry,
        bake_scale: f64,
        palette_idx: usize,
    ) -> BakedEyeLayers {
        // Phase 1: Single-threaded deterministic RNG placement & glyph cache warm
        let plan = self.plan_layout(geom_orig, bake_scale, palette_idx, key.seed);
        let glyphs: &HashMap<GlyphKey, GlyphBmp> = &self.glyphs;
        let has_fonts = self.fonts.is_some();

        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .clamp(1, 6);

        let geom = plan.geom;
        let pal = plan.palette;

        // ------------------------------------------------------------------
        // Phase 2 Wave 1: Full-canvas poetry wall parallelized across strips
        // ------------------------------------------------------------------
        let mut field = RgbaLayer::new(geom.canvas_w, geom.canvas_h);
        field.fill([pal.canvas_bg[0], pal.canvas_bg[1], pal.canvas_bg[2], 255]);

        if has_fonts && !plan.field_words.is_empty() {
            let fw = field.w;
            let fh = field.h;
            let row_bytes = fw as usize * 4;
            let strip_rows = ((fh + n_threads as i32 - 1) / n_threads as i32).max(1);
            let field_words = &plan.field_words;

            std::thread::scope(|scope| {
                for (strip_idx, slice) in
                    field.px.chunks_mut(strip_rows as usize * row_bytes).enumerate()
                {
                    let y0 = strip_idx as i32 * strip_rows;
                    let sh = (slice.len() / row_bytes) as i32;
                    let y1 = y0 + sh;
                    scope.spawn(move || {
                        let mut mask = AlphaMask::new();
                        for w in field_words {
                            if w.max_y < y0 || w.min_y >= y1 {
                                continue;
                            }
                            render_word_mask(glyphs, w.text, w.fsize, w.bold, &mut mask);
                            composite_mask_obb(
                                &mask,
                                slice,
                                fw,
                                y0,
                                sh,
                                None,
                                [0, y0, fw, y1],
                                w.cx,
                                w.cy,
                                w.angle,
                                w.color,
                            );
                        }
                    });
                }
            });
        }

        // ------------------------------------------------------------------
        // Phase 2 Wave 2: Sclera, Rings+Glow, Radial, Pupil+Highlights+Cutout+Rim
        // ------------------------------------------------------------------
        let (sclera, cutout, rings_a, rings_b, glow, radial_a, radial_b, pupil, highlights, rim_atlas, rim_words) =
            std::thread::scope(|scope| {
                // Task A: Sclera
                let sclera_handle = scope.spawn(|| {
                    let mut sclera = RgbaLayer::new(geom.canvas_w, geom.canvas_h);
                    fill_polygon(&mut sclera, &plan.sclera_poly, [6, 11, 22, 250]);
                    if has_fonts && !plan.sclera_words.is_empty() {
                        let sw_px = sclera.w;
                        let sh_px = sclera.h;
                        let mut min_sy = i32::MAX;
                        let mut max_sy = i32::MIN;
                        for w in &plan.sclera_words {
                            min_sy = min_sy.min(w.min_y);
                            max_sy = max_sy.max(w.max_y);
                        }
                        min_sy = min_sy.max(0);
                        max_sy = max_sy.min(sh_px - 1);
                        if min_sy <= max_sy {
                            let row_bytes = sw_px as usize * 4;
                            let span = max_sy - min_sy + 1;
                            let sub_threads = if n_threads >= 4 && span >= 64 { 2 } else { 1 };
                            if sub_threads == 2 {
                                let mid_y = min_sy + span / 2;
                                let split_byte = mid_y as usize * row_bytes;
                                let (top_all, bot_all) = sclera.px.split_at_mut(split_byte);
                                let top_slice = &mut top_all[min_sy as usize * row_bytes..];
                                let bot_len = (max_sy - mid_y + 1) as usize * row_bytes;
                                let bot_slice = &mut bot_all[..bot_len];
                                let sclera_words = &plan.sclera_words;
                                std::thread::scope(|inner| {
                                    inner.spawn(|| {
                                        let mut mask = AlphaMask::new();
                                        let h0 = mid_y - min_sy;
                                        for w in sclera_words {
                                            if w.max_y < min_sy || w.min_y >= mid_y {
                                                continue;
                                            }
                                            render_word_mask(
                                                glyphs,
                                                w.text,
                                                w.fsize,
                                                w.bold,
                                                &mut mask,
                                            );
                                            composite_mask_obb(
                                                &mask,
                                                top_slice,
                                                sw_px,
                                                min_sy,
                                                h0,
                                                None,
                                                [0, min_sy, sw_px, mid_y],
                                                w.cx,
                                                w.cy,
                                                w.angle,
                                                w.color,
                                            );
                                        }
                                    });
                                    inner.spawn(|| {
                                        let mut mask = AlphaMask::new();
                                        let h1 = max_sy - mid_y + 1;
                                        for w in sclera_words {
                                            if w.max_y < mid_y || w.min_y > max_sy {
                                                continue;
                                            }
                                            render_word_mask(
                                                glyphs,
                                                w.text,
                                                w.fsize,
                                                w.bold,
                                                &mut mask,
                                            );
                                            composite_mask_obb(
                                                &mask,
                                                bot_slice,
                                                sw_px,
                                                mid_y,
                                                h1,
                                                None,
                                                [0, mid_y, sw_px, max_sy + 1],
                                                w.cx,
                                                w.cy,
                                                w.angle,
                                                w.color,
                                            );
                                        }
                                    });
                                });
                            } else {
                                let mut mask = AlphaMask::new();
                                for w in &plan.sclera_words {
                                    render_word_mask(glyphs, w.text, w.fsize, w.bold, &mut mask);
                                    composite_mask_obb(
                                        &mask,
                                        &mut sclera.px,
                                        sw_px,
                                        0,
                                        sh_px,
                                        None,
                                        [0, 0, sw_px, sh_px],
                                        w.cx,
                                        w.cy,
                                        w.angle,
                                        w.color,
                                    );
                                }
                            }
                        }
                    }
                    sclera
                });

                // Task B: Iris concentric rings + pupil cut + half-res glow
                let rings_handle = scope.spawn(|| {
                    let dim = plan.iris_dim;
                    let icx = dim / 2;
                    let icy = dim / 2;
                    let mut rings_a = RgbaLayer::new(dim, dim);
                    let mut rings_b = RgbaLayer::new(dim, dim);
                    let ir_px = (geom.iris_radius * 0.98) as i32;
                    fill_ellipse(&mut rings_a, icx, icy, ir_px, [5, 10, 22, 244]);
                    fill_ellipse(&mut rings_b, icx, icy, ir_px, [5, 10, 22, 244]);

                    if has_fonts {
                        let mut mask = AlphaMask::new();
                        for w in &plan.ring_words {
                            render_word_mask(glyphs, w.text, w.fsize, w.bold, &mut mask);
                            composite_mask_obb(
                                &mask,
                                &mut rings_a.px,
                                dim,
                                0,
                                dim,
                                Some((&mut rings_b.px, w.col_b)),
                                [0, 0, dim, dim],
                                w.cx,
                                w.cy,
                                w.angle,
                                w.col_a,
                            );
                        }
                    }

                    let pr_clear = (geom.pupil_radius * 0.98) as i32 as f64;
                    let cut_sigma = ((3.0 * geom.scale) as i32).max(2) as f64;
                    apply_radial_pupil_cut(
                        &mut [&mut rings_a, &mut rings_b],
                        icx,
                        icy,
                        pr_clear,
                        cut_sigma,
                    );
                    rings_a.clear_outer_border();
                    rings_b.clear_outer_border();
                    let glow = build_half_res_glow(&rings_a, geom.scale);
                    (rings_a, rings_b, glow)
                });

                // Task C: Iris radial spokes + pupil cut
                let radial_handle = scope.spawn(|| {
                    let dim = plan.iris_dim;
                    let icx = dim / 2;
                    let icy = dim / 2;
                    let mut radial_a = RgbaLayer::new(dim, dim);
                    let mut radial_b = RgbaLayer::new(dim, dim);
                    if has_fonts {
                        let mut mask = AlphaMask::new();
                        for w in &plan.radial_words {
                            render_word_mask(glyphs, w.text, w.fsize, w.bold, &mut mask);
                            composite_mask_obb(
                                &mask,
                                &mut radial_a.px,
                                dim,
                                0,
                                dim,
                                Some((&mut radial_b.px, w.col_b)),
                                [0, 0, dim, dim],
                                w.cx,
                                w.cy,
                                w.angle,
                                w.col_a,
                            );
                        }
                    }
                    let pr_clear = (geom.pupil_radius * 0.98) as i32 as f64;
                    let cut_sigma = ((3.0 * geom.scale) as i32).max(2) as f64;
                    apply_radial_pupil_cut(
                        &mut [&mut radial_a, &mut radial_b],
                        icx,
                        icy,
                        pr_clear,
                        cut_sigma,
                    );
                    radial_a.clear_outer_border();
                    radial_b.clear_outer_border();
                    (radial_a, radial_b)
                });

                // Task D: Pupil + Highlights + Analytic Cutout + Rim Atlas
                let misc_handle = scope.spawn(|| {
                    let pdim = plan.pupil_dim;
                    let pcx = pdim / 2;
                    let pcy = pcy_of(pdim);
                    let mut pupil = RgbaLayer::new(pdim, pdim);
                    let pr_disc = (geom.pupil_radius * 0.97) as i32;
                    fill_ellipse(&mut pupil, pcx, pcy, pr_disc, [3, 5, 11, 248]);
                    let mut mask = AlphaMask::new();
                    if has_fonts {
                        for w in &plan.pupil_words {
                            render_word_mask(glyphs, w.text, w.fsize, w.bold, &mut mask);
                            composite_mask_obb(
                                &mask,
                                &mut pupil.px,
                                pdim,
                                0,
                                pdim,
                                None,
                                [0, 0, pdim, pdim],
                                w.cx,
                                w.cy,
                                w.angle,
                                w.color,
                            );
                        }
                    }
                    pupil.clear_outer_border();

                    let idim = plan.iris_dim;
                    let mut highlights = RgbaLayer::new(idim, idim);
                    if has_fonts {
                        for w in &plan.highlight_words {
                            render_word_mask(glyphs, w.text, w.fsize, w.bold, &mut mask);
                            composite_mask_obb(
                                &mask,
                                &mut highlights.px,
                                idim,
                                0,
                                idim,
                                None,
                                [0, 0, idim, idim],
                                w.cx,
                                w.cy,
                                w.angle,
                                w.color,
                            );
                        }
                    }
                    highlights.clear_outer_border();

                    let cutout = build_analytic_cutout(plan.cutout_dim, &geom);

                    // Pack & vertically crop Rim Atlas (1 mask rasterization per phrase!)
                    let mut atlas = RimAtlasBuilder::new(2048, 2048);
                    struct PendingRim {
                        u: f64,
                        v_offset: f64,
                        is_upper: bool,
                        slots: [PixelSlot; 3],
                    }
                    let mut pending_rims: Vec<PendingRim> = Vec::with_capacity(plan.rim_plans.len());
                    if has_fonts {
                        for rp in &plan.rim_plans {
                            render_word_mask(glyphs, rp.phrase, rp.fsize, true, &mut mask);
                            let mut slots: [Option<PixelSlot>; 3] = [None, None, None];
                            let mut ok = true;
                            for (vi, &ang) in rp.angles.iter().enumerate() {
                                let (nw, nh) = rotated_dims(mask.w, mask.h, ang);
                                if let Some(slot) = atlas.alloc_slot(nw, nh) {
                                    let cx_slot = slot.x as f64 + nw as f64 * 0.5;
                                    let cy_slot = slot.y as f64 + nh as f64 * 0.5;
                                    let aw = atlas.layer.w;
                                    let ah = atlas.layer.h;
                                    composite_mask_obb(
                                        &mask,
                                        &mut atlas.layer.px,
                                        aw,
                                        0,
                                        ah,
                                        None,
                                        [slot.x, slot.y, slot.x + nw, slot.y + nh],
                                        cx_slot,
                                        cy_slot,
                                        ang,
                                        rp.color,
                                    );
                                    slots[vi] = Some(slot);
                                } else {
                                    ok = false;
                                }
                            }
                            if ok {
                                pending_rims.push(PendingRim {
                                    u: rp.u,
                                    v_offset: rp.v_offset,
                                    is_upper: rp.is_upper,
                                    slots: [
                                        slots[0].take().unwrap(),
                                        slots[1].take().unwrap(),
                                        slots[2].take().unwrap(),
                                    ],
                                });
                            }
                        }
                    }
                    let (rim_atlas, used_h) = atlas.finish_cropped();
                    let aw_f = rim_atlas.w as f32;
                    let ah_f = used_h as f32;
                    let rim_words: Vec<RimWord> = pending_rims
                        .into_iter()
                        .map(|pr| {
                            let to_rect = |s: &PixelSlot| RimSpriteRect {
                                u0: s.x as f32 / aw_f,
                                v0: s.y as f32 / ah_f,
                                u1: (s.x + s.w) as f32 / aw_f,
                                v1: (s.y + s.h) as f32 / ah_f,
                                w: s.w,
                                h: s.h,
                            };
                            RimWord {
                                u: pr.u,
                                v_offset: pr.v_offset,
                                is_upper: pr.is_upper,
                                variants: [
                                    to_rect(&pr.slots[0]),
                                    to_rect(&pr.slots[1]),
                                    to_rect(&pr.slots[2]),
                                ],
                            }
                        })
                        .collect();

                    (pupil, highlights, cutout, rim_atlas, rim_words)
                });

                let sclera = sclera_handle.join().unwrap();
                let (rings_a, rings_b, glow) = rings_handle.join().unwrap();
                let (radial_a, radial_b) = radial_handle.join().unwrap();
                let (pupil, highlights, cutout, rim_atlas, rim_words) = misc_handle.join().unwrap();
                (
                    sclera, cutout, rings_a, rings_b, glow, radial_a, radial_b, pupil, highlights,
                    rim_atlas, rim_words,
                )
            });

        BakedEyeLayers {
            geom,
            geom_orig: *geom_orig,
            bake_scale: bake_scale as f32,
            field,
            sclera,
            cutout_dim: plan.cutout_dim,
            cutout,
            glow,
            rings_a,
            rings_b,
            radial_a,
            radial_b,
            iris_dim: plan.iris_dim,
            pupil,
            pupil_dim: plan.pupil_dim,
            highlights,
            rim_atlas,
            rim_words,
            word_count: plan.word_count,
        }
    }
}

#[inline]
fn pcy_of(pdim: i32) -> i32 {
    pdim / 2
}
