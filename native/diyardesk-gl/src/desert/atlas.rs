// Desert Oasis sprite atlas — software Canvas2D-compatible rasterizer.
// Bakes the same 10 sprites as prototype/desert-props.js (2x supersampled, like the
// prototype's `ctx.scale(2, 2)`), so the instanced GL batch can draw them 1:1.

use std::f32::consts::PI;

pub type Pt = [f32; 2];

pub const SPRITE_COUNT: usize = 10;
pub const SPRITE_SZ: i32 = 128;
pub const ATLAS_SCALE: f32 = 2.0;

#[derive(Clone)]
pub enum Paint {
    Solid([f32; 4]),
    Linear {
        p0: Pt,
        p1: Pt,
        stops: Vec<(f32, [f32; 4])>,
    },
    Radial {
        c0: Pt,
        r0: f32,
        c1: Pt,
        r1: f32,
        stops: Vec<(f32, [f32; 4])>,
    },
}

impl Paint {
    pub fn solid(r: f32, g: f32, b: f32, a: f32) -> Self {
        Paint::Solid([r, g, b, a])
    }

    pub fn hex(hex: u32, a: f32) -> Self {
        Paint::Solid([
            ((hex >> 16) & 0xFF) as f32 / 255.0,
            ((hex >> 8) & 0xFF) as f32 / 255.0,
            (hex & 0xFF) as f32 / 255.0,
            a,
        ])
    }

    fn at(&self, p: Pt) -> [f32; 4] {
        match self {
            Paint::Solid(c) => *c,
            Paint::Linear { p0, p1, stops } => {
                let dx = p1[0] - p0[0];
                let dy = p1[1] - p0[1];
                let len2 = dx * dx + dy * dy;
                let t = if len2 <= 1e-9 {
                    0.0
                } else {
                    ((p[0] - p0[0]) * dx + (p[1] - p0[1]) * dy) / len2
                };
                sample_stops(stops, t)
            }
            Paint::Radial { c0, r0, c1, r1, stops } => {
                let dx = c1[0] - c0[0];
                let dy = c1[1] - c0[1];
                let dr = r1 - r0;
                let qx = p[0] - c0[0];
                let qy = p[1] - c0[1];
                let a = dx * dx + dy * dy - dr * dr;
                let b = -2.0 * (qx * dx + qy * dy) - 2.0 * r0 * dr;
                let c = qx * qx + qy * qy - r0 * r0;
                let t = if a.abs() < 1e-7 {
                    if b.abs() < 1e-9 {
                        0.0
                    } else {
                        -c / b
                    }
                } else {
                    let disc = b * b - 4.0 * a * c;
                    if disc < 0.0 {
                        0.0
                    } else {
                        let sq = disc.sqrt();
                        let t1 = (-b + sq) / (2.0 * a);
                        let t2 = (-b - sq) / (2.0 * a);
                        if t1 >= t2 {
                            t1
                        } else {
                            t2
                        }
                    }
                };
                sample_stops(stops, t)
            }
        }
    }
}

fn sample_stops(stops: &[(f32, [f32; 4])], t: f32) -> [f32; 4] {
    if stops.is_empty() {
        return [0.0, 0.0, 0.0, 0.0];
    }
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if t <= t1 {
            let span = t1 - t0;
            let k = if span.abs() < 1e-9 { 0.0 } else { (t - t0) / span };
            return [
                c0[0] + (c1[0] - c0[0]) * k,
                c0[1] + (c1[1] - c0[1]) * k,
                c0[2] + (c1[2] - c0[2]) * k,
                c0[3] + (c1[3] - c0[3]) * k,
            ];
        }
    }
    stops[stops.len() - 1].1
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub scale: f32,
    pub px: Vec<u8>,
}

impl Canvas {
    pub fn new(w: i32, h: i32, scale: f32) -> Self {
        Self {
            w,
            h,
            scale,
            px: vec![0u8; (w * h * 4) as usize],
        }
    }

    #[inline]
    fn blend(&mut self, x: i32, y: i32, c: [f32; 4], cov: f32) {
        if cov <= 0.0005 || x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let sa = (c[3] * cov).clamp(0.0, 1.0);
        if sa <= 0.0 {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let da = self.px[i + 3] as f32 / 255.0;
        let oa = sa + da * (1.0 - sa);
        if oa <= 1e-6 {
            return;
        }
        for ch in 0..3 {
            let d = self.px[i + ch] as f32 / 255.0;
            let o = (c[ch] * sa + d * da * (1.0 - sa)) / oa;
            self.px[i + ch] = (o.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        self.px[i + 3] = (oa.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    /// Even-odd scanline fill with 4 sub-scanlines per pixel row and analytic x coverage.
    pub fn fill(&mut self, poly: &[Pt], paint: &Paint, clip: Option<&[Pt]>) {
        if poly.len() < 3 {
            return;
        }
        let s = self.scale;
        let pts: Vec<Pt> = poly.iter().map(|p| [p[0] * s, p[1] * s]).collect();
        let clip_pts: Option<Vec<Pt>> = clip.map(|c| c.iter().map(|p| [p[0] * s, p[1] * s]).collect());

        let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &pts {
            minx = minx.min(p[0]);
            maxx = maxx.max(p[0]);
            miny = miny.min(p[1]);
            maxy = maxy.max(p[1]);
        }
        let y0 = (miny.floor() as i32).max(0);
        let y1 = (maxy.ceil() as i32).min(self.h);
        let x0 = (minx.floor() as i32).max(0);
        let x1 = (maxx.ceil() as i32).min(self.w);
        if y1 <= y0 || x1 <= x0 {
            return;
        }
        let row_w = (x1 - x0) as usize;
        let mut cov = vec![0.0f32; row_w];
        let mut ccov = vec![0.0f32; row_w];

        for y in y0..y1 {
            for v in cov.iter_mut() {
                *v = 0.0;
            }
            if clip_pts.is_some() {
                for v in ccov.iter_mut() {
                    *v = 0.0;
                }
            }
            for sub in 0..4 {
                let yy = y as f32 + (sub as f32 + 0.5) / 4.0;
                span_coverage(&pts, yy, x0, x1, &mut cov, 0.25);
                if let Some(cp) = clip_pts.as_ref() {
                    span_coverage(cp, yy, x0, x1, &mut ccov, 0.25);
                }
            }
            for xi in 0..row_w {
                let mut c = cov[xi];
                if clip_pts.is_some() {
                    c *= ccov[xi].min(1.0);
                }
                if c > 0.0005 {
                    let px = x0 + xi as i32;
                    let col = paint.at([(px as f32 + 0.5) / s, (y as f32 + 0.5) / s]);
                    self.blend(px, y, col, c.min(1.0));
                }
            }
        }
    }

    /// Round-capped polyline stroke rasterized as a distance field (matches Canvas2D look).
    pub fn stroke(&mut self, pts: &[Pt], width: f32, paint: &Paint, clip: Option<&[Pt]>) {
        if pts.len() < 2 {
            return;
        }
        let s = self.scale;
        let sp: Vec<Pt> = pts.iter().map(|p| [p[0] * s, p[1] * s]).collect();
        let hw = (width * s * 0.5).max(0.35);
        let clip_pts: Option<Vec<Pt>> = clip.map(|c| c.iter().map(|p| [p[0] * s, p[1] * s]).collect());

        let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &sp {
            minx = minx.min(p[0]);
            maxx = maxx.max(p[0]);
            miny = miny.min(p[1]);
            maxy = maxy.max(p[1]);
        }
        let x0 = ((minx - hw - 1.0).floor() as i32).max(0);
        let x1 = ((maxx + hw + 1.0).ceil() as i32).min(self.w);
        let y0 = ((miny - hw - 1.0).floor() as i32).max(0);
        let y1 = ((maxy + hw + 1.0).ceil() as i32).min(self.h);
        if y1 <= y0 || x1 <= x0 {
            return;
        }
        for y in y0..y1 {
            for x in x0..x1 {
                let mut acc = 0.0f32;
                for sy in 0..2 {
                    for sx in 0..2 {
                        let px = x as f32 + (sx as f32 + 0.5) * 0.5;
                        let py = y as f32 + (sy as f32 + 0.5) * 0.5;
                        if let Some(cp) = clip_pts.as_ref() {
                            if !inside(cp, [px, py]) {
                                continue;
                            }
                        }
                        let mut best = f32::MAX;
                        for w in sp.windows(2) {
                            let d = seg_dist([px, py], w[0], w[1]);
                            if d < best {
                                best = d;
                            }
                        }
                        if best <= hw {
                            acc += 0.25;
                        }
                    }
                }
                if acc > 0.0005 {
                    let col = paint.at([(x as f32 + 0.5) / s, (y as f32 + 0.5) / s]);
                    self.blend(x, y, col, acc);
                }
            }
        }
    }

    /// Premultiply RGB by A (kept for callers that use SRC_ALPHA blending).
    #[allow(dead_code)]
    pub fn premultiply(&mut self) {
        for i in (0..self.px.len()).step_by(4) {
            let a = self.px[i + 3] as f32 / 255.0;
            for ch in 0..3 {
                self.px[i + ch] = (self.px[i + ch] as f32 * a).round() as u8;
            }
        }
    }
}

fn span_coverage(poly: &[Pt], yy: f32, x0: i32, x1: i32, cov: &mut [f32], weight: f32) {
    let mut xs: Vec<f32> = Vec::with_capacity(16);
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        if (a[1] <= yy) != (b[1] <= yy) {
            let dy = b[1] - a[1];
            if dy.abs() > 1e-9 {
                xs.push(a[0] + (yy - a[1]) * (b[0] - a[0]) / dy);
            }
        }
    }
    if xs.len() < 2 {
        return;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut i = 0;
    while i + 1 < xs.len() {
        let xa = xs[i];
        let xb = xs[i + 1];
        i += 2;
        if xb <= xa {
            continue;
        }
        let pa = xa.floor() as i32;
        let pb = xb.floor() as i32;
        for px in pa..=pb {
            if px < x0 || px >= x1 {
                continue;
            }
            let l = xa.max(px as f32);
            let r = xb.min(px as f32 + 1.0);
            if r > l {
                cov[(px - x0) as usize] += (r - l) * weight;
            }
        }
    }
}

fn inside(poly: &[Pt], p: Pt) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let dy = b[1] - a[1];
            if dy.abs() > 1e-9 {
                let x = a[0] + (p[1] - a[1]) * (b[0] - a[0]) / dy;
                if p[0] < x {
                    c = !c;
                }
            }
        }
    }
    c
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len2 = dx * dx + dy * dy;
    if len2 <= 1e-9 {
        return ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0);
    let qx = a[0] + dx * t;
    let qy = a[1] + dy * t;
    ((p[0] - qx).powi(2) + (p[1] - qy).powi(2)).sqrt()
}

// ---------------------------------------------------------------------------
// Shape helpers (Canvas2D path equivalents)
// ---------------------------------------------------------------------------

pub fn ellipse_poly(cx: f32, cy: f32, rx: f32, ry: f32, rot: f32) -> Vec<Pt> {
    let segs = 40;
    let (sr, cr) = rot.sin_cos();
    (0..segs)
        .map(|i| {
            let a = i as f32 / segs as f32 * 2.0 * PI;
            let (sa, ca) = a.sin_cos();
            let x = ca * rx;
            let y = sa * ry;
            [cx + x * cr - y * sr, cy + x * sr + y * cr]
        })
        .collect()
}

pub fn rect_poly(x: f32, y: f32, w: f32, h: f32) -> Vec<Pt> {
    vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
}

pub fn round_rect_poly(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<Pt> {
    let rr = r.min(w * 0.5).min(h * 0.5).max(0.0);
    if rr <= 0.01 {
        return rect_poly(x, y, w, h);
    }
    let mut out = Vec::with_capacity(48);
    let corners = [
        (x + w - rr, y + rr, -PI * 0.5),
        (x + w - rr, y + h - rr, 0.0),
        (x + rr, y + h - rr, PI * 0.5),
        (x + rr, y + rr, PI),
    ];
    for (ccx, ccy, a0) in corners {
        for k in 0..=6 {
            let a = a0 + k as f32 / 6.0 * PI * 0.5;
            out.push([ccx + a.cos() * rr, ccy + a.sin() * rr]);
        }
    }
    out
}

pub fn arc_pts(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, steps: usize) -> Vec<Pt> {
    (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            [cx + a.cos() * r, cy + a.sin() * r]
        })
        .collect()
}

pub fn quad_pts(p0: Pt, p1: Pt, p2: Pt, steps: usize) -> Vec<Pt> {
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let u = 1.0 - t;
            [
                u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
                u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
            ]
        })
        .collect()
}

fn rot_pt(p: Pt, rot: f32) -> Pt {
    let (s, c) = rot.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

// ---------------------------------------------------------------------------
// Sprite painters (1:1 with prototype/desert-props.js)
// ---------------------------------------------------------------------------

fn shadow(c: &mut Canvas, cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, alpha: f32) {
    let p = Paint::solid(92.0 / 255.0, 54.0 / 255.0, 18.0 / 255.0, alpha);
    c.fill(&ellipse_poly(cx + 6.0, cy + 7.0, rx, ry, rot), &p, None);
}

fn paint_camel(c: &mut Canvas, idx: usize, dark: u32, light: u32, saddle_a: u32, saddle_b: u32, stride: f32) {
    let cx = idx as f32 * SPRITE_SZ as f32 + 64.0;
    let cy = 64.0;
    shadow(c, cx, cy, 34.0, 14.0, 0.12, 0.30);

    let dark_p = Paint::hex(dark, 1.0);
    let light_p = Paint::hex(light, 1.0);

    // 4 legs stepping out beneath torso
    for (lx, ly, st) in [(14.0f32, -12.0f32, stride), (14.0, 12.0, -stride), (-15.0, -11.0, -stride), (-15.0, 11.0, stride)] {
        c.fill(&ellipse_poly(cx + lx + st * 4.5, cy + ly, 5.5, 3.2, 0.0), &dark_p, None);
    }

    // Swishing tail
    let tail = quad_pts([cx - 25.0, cy], [cx - 34.0, cy + stride * 4.0], [cx - 39.0, cy + stride * 6.0], 10);
    c.stroke(&tail, 2.4, &dark_p, None);

    // Muscular torso
    let body = Paint::Radial {
        c0: [cx - 2.0, cy - 3.0],
        r0: 3.0,
        c1: [cx - 2.0, cy],
        r1: 27.0,
        stops: vec![
            (0.0, hex4(light, 1.0)),
            (0.75, hex4(dark, 1.0)),
            (1.0, hex4(0x7a5228, 1.0)),
        ],
    };
    c.fill(&ellipse_poly(cx - 2.0, cy, 24.0, 13.5, 0.0), &body, None);

    // Woven Bedouin saddle blanket & packs
    c.fill(&round_rect_poly(cx - 14.0, cy - 11.5, 22.0, 23.0, 5.0), &Paint::hex(saddle_a, 1.0), None);
    c.fill(&rect_poly(cx - 10.0, cy - 11.0, 4.5, 22.0), &Paint::hex(saddle_b, 1.0), None);
    c.fill(&rect_poly(cx + 0.5, cy - 11.0, 4.5, 22.0), &Paint::hex(saddle_b, 1.0), None);
    let rim = rect_poly(cx - 12.0, cy - 9.5, 18.0, 19.0);
    let mut rim_closed = rim.clone();
    rim_closed.push(rim[0]);
    c.stroke(&rim_closed, 1.1, &Paint::solid(255.0 / 255.0, 235.0 / 255.0, 190.0 / 255.0, 0.65), None);

    // Single hump highlight protruding through saddle
    let hump = Paint::Radial {
        c0: [cx - 4.0, cy - 2.0],
        r0: 1.0,
        c1: [cx - 3.0, cy],
        r1: 10.0,
        stops: vec![(0.0, hex4(light, 1.0)), (1.0, hex4(dark, 1.0))],
    };
    c.fill(&ellipse_poly(cx - 3.0, cy, 9.0, 7.0, 0.0), &hump, None);

    // Curved neck stretching forward to +X
    let neck = Paint::Linear {
        p0: [cx + 14.0, cy],
        p1: [cx + 36.0, cy],
        stops: vec![(0.0, hex4(dark, 1.0)), (1.0, hex4(light, 1.0))],
    };
    let mut neck_poly = vec![[cx + 15.0, cy - 5.5]];
    neck_poly.extend(quad_pts([cx + 15.0, cy - 5.5], [cx + 28.0, cy - 4.5], [cx + 36.0, cy - 3.8], 8));
    neck_poly.push([cx + 36.0, cy + 3.8]);
    neck_poly.extend(quad_pts([cx + 36.0, cy + 3.8], [cx + 28.0, cy + 4.5], [cx + 15.0, cy + 5.5], 8));
    c.fill(&neck_poly, &neck, None);

    // Camel head + ears + halter
    c.fill(&ellipse_poly(cx + 41.0, cy, 7.2, 4.6, 0.0), &light_p, None);
    c.fill(&ellipse_poly(cx + 37.0, cy - 5.2, 2.5, 1.5, -0.4), &dark_p, None);
    c.fill(&ellipse_poly(cx + 37.0, cy + 5.2, 2.5, 1.5, 0.4), &dark_p, None);
    c.stroke(&[[cx + 43.5, cy - 4.0], [cx + 43.5, cy + 4.0]], 1.6, &Paint::hex(0xc83e2b, 1.0), None);
}

fn paint_traveler(
    c: &mut Canvas,
    idx: usize,
    robe_main: u32,
    robe_trim: u32,
    keffiyeh_base: u32,
    keffiyeh_pattern: Option<[f32; 4]>,
    agal_dark: u32,
    has_staff: bool,
    step: f32,
) {
    let cx = idx as f32 * SPRITE_SZ as f32 + 64.0;
    let cy = 64.0;
    shadow(c, cx, cy, 16.0, 15.0, 0.2, 0.30);

    // Stepping sandals peeking beneath flowing robe
    c.fill(&ellipse_poly(cx + step * 4.0, cy - 5.5, 5.0, 2.8, 0.0), &Paint::hex(0x5c3b1e, 1.0), None);
    c.fill(&ellipse_poly(cx - step * 4.0, cy + 5.5, 5.0, 2.8, 0.0), &Paint::hex(0x5c3b1e, 1.0), None);

    // Wide flowing kandura / bisht shoulders
    let robe = Paint::Radial {
        c0: [cx - 2.0, cy - 3.0],
        r0: 2.0,
        c1: [cx, cy],
        r1: 18.0,
        stops: vec![(0.0, hex4(robe_main, 1.0)), (1.0, hex4(robe_trim, 1.0))],
    };
    c.fill(&ellipse_poly(cx - 1.0, cy, 12.5, 15.5, 0.0), &robe, None);

    // Subtle shoulder folds / cloak edge
    let folds = arc_pts(cx - 1.0, cy, 13.5, PI * 0.55, PI * 1.45, 18);
    c.stroke(&folds, 1.2, &Paint::solid(220.0 / 255.0, 185.0 / 255.0, 115.0 / 255.0, 0.55), None);

    // Walking wooden staff held slightly forward on the right side
    if has_staff {
        c.stroke(&[[cx - 4.0, cy + 15.0], [cx + 19.0, cy + 13.0]], 2.4, &Paint::hex(0x6f4722, 1.0), None);
        c.fill(&ellipse_poly(cx + 19.0, cy + 13.0, 2.0, 2.0, 0.0), &Paint::hex(0x4e3116, 1.0), None);
    }

    // Draping Keffiyeh (Ghutra) headscarf trailing slightly back (-X)
    c.fill(&ellipse_poly(cx - 3.0, cy, 10.5, 9.2, 0.0), &Paint::hex(keffiyeh_base, 1.0), None);

    if let Some(pat) = keffiyeh_pattern {
        let mut d = -6.0f32;
        while d <= 4.0 {
            c.stroke(&[[cx - 10.0, cy + d - 2.0], [cx + 4.0, cy + d + 2.0]], 1.1, &Paint::Solid(pat), None);
            d += 3.5;
        }
    }

    // Black Agal double cord ring on crown + crown highlight
    let ring = arc_pts(cx, cy, 6.2, 0.0, 2.0 * PI, 28);
    c.stroke(&ring, 2.6, &Paint::hex(agal_dark, 1.0), None);
    c.fill(&ellipse_poly(cx, cy, 4.4, 4.4, 0.0), &Paint::hex(keffiyeh_base, 1.0), None);

    // Tiny hint of sunlit nose/brow facing +X
    c.fill(&ellipse_poly(cx + 6.5, cy, 2.2, 3.2, 0.0), &Paint::hex(0xb97e52, 1.0), None);
}

fn paint_bedouin_tent(c: &mut Canvas, idx: usize) {
    let cx = idx as f32 * SPRITE_SZ as f32 + 64.0;
    let cy = 64.0;
    shadow(c, cx, cy, 42.0, 30.0, 0.1, 0.34);

    // Woven desert rug in front (+Y side)
    c.fill(&rect_poly(cx - 18.0, cy + 12.0, 36.0, 22.0), &Paint::hex(0xa83428, 1.0), None);
    c.fill(&rect_poly(cx - 14.0, cy + 15.0, 28.0, 3.0), &Paint::hex(0xe5b869, 1.0), None);
    c.fill(&rect_poly(cx - 14.0, cy + 27.0, 28.0, 3.0), &Paint::hex(0xe5b869, 1.0), None);
    c.fill(&rect_poly(cx - 10.0, cy + 20.0, 20.0, 5.0), &Paint::hex(0x2d5c62, 1.0), None);

    // Guy ropes to wooden stakes
    let rope = Paint::solid(130.0 / 255.0, 92.0 / 255.0, 48.0 / 255.0, 0.72);
    for (rx, ry, tx, ty) in [
        (-28.0f32, -20.0f32, -44.0f32, -30.0f32),
        (28.0, -20.0, 44.0, -30.0),
        (-28.0, 14.0, -44.0, 24.0),
        (28.0, 14.0, 44.0, 24.0),
    ] {
        c.stroke(&[[cx + rx, cy + ry], [cx + tx, cy + ty]], 1.2, &rope, None);
        c.fill(&ellipse_poly(cx + tx, cy + ty, 2.0, 2.0, 0.0), &Paint::hex(0x5e3c1e, 1.0), None);
    }

    // Striped Bedouin canopy (clipped roundRect)
    let clip = round_rect_poly(cx - 30.0, cy - 24.0, 60.0, 40.0, 6.0);
    let stripes = [0x4a2218u32, 0xe6cfad, 0x963122, 0xe6cfad, 0x382018, 0xd7b278, 0x963122, 0xe6cfad, 0x4a2218];
    let sw = 60.0 / stripes.len() as f32;
    for (i, col) in stripes.iter().enumerate() {
        c.fill(
            &rect_poly(cx - 30.0 + i as f32 * sw, cy - 24.0, sw + 0.5, 40.0),
            &Paint::hex(*col, 1.0),
            Some(&clip),
        );
    }
    // Sun ridge shading
    let shade = Paint::Linear {
        p0: [cx, cy - 24.0],
        p1: [cx, cy + 16.0],
        stops: vec![
            (0.0, [255.0 / 255.0, 245.0 / 255.0, 220.0 / 255.0, 0.22]),
            (0.45, [1.0, 1.0, 1.0, 0.0]),
            (1.0, [38.0 / 255.0, 16.0 / 255.0, 8.0 / 255.0, 0.28]),
        ],
    };
    c.fill(&rect_poly(cx - 30.0, cy - 24.0, 60.0, 40.0), &shade, Some(&clip));

    // Ridge pole + wooden finials
    c.stroke(&[[cx - 31.0, cy - 4.0], [cx + 31.0, cy - 4.0]], 2.6, &Paint::hex(0x6c4220, 1.0), None);
}

fn paint_palm(c: &mut Canvas, idx: usize, fronds: usize, frond_len: f32, c_dark: u32, c_mid: u32, c_bright: u32) {
    let cx = idx as f32 * SPRITE_SZ as f32 + 64.0;
    let cy = 64.0;
    shadow(c, cx + 4.0, cy + 4.0, frond_len * 0.78, frond_len * 0.68, 0.15, 0.25);

    // Date fruit clusters near crown center
    for i in 0..4 {
        let a = i as f32 * 1.57 + 0.4;
        c.fill(
            &ellipse_poly(cx + a.cos() * 9.0, cy + a.sin() * 9.0, 4.5, 3.0, a),
            &Paint::hex(0xc46c1c, 1.0),
            None,
        );
    }

    for i in 0..fronds {
        let a = (i as f32 / fronds as f32) * 2.0 * PI + (i % 2) as f32 * 0.18 + idx as f32 * 0.4;
        let len = frond_len * (0.86 + (i % 3) as f32 * 0.08);
        let leaflets = 11;
        for k in 1..=leaflets {
            let t = k as f32 / (leaflets + 1) as f32;
            let px = 4.0 + t * (len - 6.0);
            let w = (t * PI).sin() * 11.5;
            let col = if k % 2 == 0 {
                c_bright
            } else if i % 2 == 0 {
                c_mid
            } else {
                c_dark
            };
            for side in [-1.0f32, 1.0] {
                let mut poly = vec![rot_pt([px - 1.5, 0.0], a)];
                for p in quad_pts([px - 1.5, 0.0], [px + 3.0, side * w * 0.6], [px + 6.5, side * w], 5) {
                    poly.push(rot_pt(p, a));
                }
                for p in quad_pts([px + 6.5, side * w], [px + 2.0, side * w * 0.3], [px + 1.5, 0.0], 5) {
                    poly.push(rot_pt(p, a));
                }
                let poly: Vec<Pt> = poly.into_iter().map(|p| [cx + p[0], cy + p[1]]).collect();
                c.fill(&poly, &Paint::hex(col, 1.0), None);
            }
        }
        // Midrib spine
        let spine = [rot_pt([0.0, 0.0], a), rot_pt([len * 0.94, 0.0], a)];
        c.stroke(
            &[[cx + spine[0][0], cy + spine[0][1]], [cx + spine[1][0], cy + spine[1][1]]],
            1.6,
            &Paint::hex(0x9bb556, 1.0),
            None,
        );
    }

    // Palm heart
    c.fill(&ellipse_poly(cx, cy, 4.8, 4.8, 0.0), &Paint::hex(0x524024, 1.0), None);
}

fn paint_rock_scrub(c: &mut Canvas, idx: usize) {
    let cx = idx as f32 * SPRITE_SZ as f32 + 64.0;
    let cy = 64.0;
    shadow(c, cx, cy, 24.0, 18.0, -0.2, 0.28);

    // Warm layered sandstone rocks
    for (rx, ry, rw, rh, c1, c2) in [
        (-4.0f32, -2.0f32, 18.0f32, 13.0f32, 0xb8824eu32, 0xdfb47eu32),
        (12.0, 6.0, 11.0, 8.0, 0x9f6c3d, 0xcfa06b),
        (-14.0, 9.0, 9.0, 7.0, 0xa87544, 0xd4a673),
    ] {
        let g = Paint::Linear {
            p0: [cx + rx - rw, cy + ry - rh],
            p1: [cx + rx + rw, cy + ry + rh],
            stops: vec![(0.0, hex4(c2, 1.0)), (1.0, hex4(c1, 1.0))],
        };
        c.fill(&ellipse_poly(cx + rx, cy + ry, rw, rh, 0.35), &g, None);
    }
    // Small olive desert scrub tucked by the rocks
    for i in 0..8 {
        let a = i as f32 * 0.78 + 0.2;
        let col = if i % 2 == 1 { 0x6f8545 } else { 0x8b9b54 };
        c.fill(
            &ellipse_poly(cx - 10.0 + a.cos() * 6.0, cy - 9.0 + a.sin() * 5.0, 5.5, 3.0, a),
            &Paint::hex(col, 1.0),
            None,
        );
    }
}

fn paint_oasis_bush(c: &mut Canvas, idx: usize) {
    let cx = idx as f32 * SPRITE_SZ as f32 + 64.0;
    let cy = 64.0;
    shadow(c, cx, cy, 20.0, 17.0, 0.1, 0.25);
    for i in 0..12 {
        let a = (i as f32 / 12.0) * 2.0 * PI;
        let r = 6.0 + (i % 3) as f32 * 4.0;
        let col = if i % 3 == 0 {
            0x2b6f38
        } else if i % 3 == 1 {
            0x44914c
        } else {
            0x6cb35c
        };
        c.fill(
            &ellipse_poly(cx + a.cos() * r, cy + a.sin() * r, 7.5, 4.5, a),
            &Paint::hex(col, 1.0),
            None,
        );
    }
    // Tiny warm desert blossoms on the bush
    for (bx, by, col) in [(5.0f32, -6.0f32, 0xf6d860u32), (-7.0, 4.0, 0xf08a5d), (2.0, 8.0, 0xf6d860)] {
        c.fill(&ellipse_poly(cx + bx, cy + by, 2.3, 2.3, 0.0), &Paint::hex(col, 1.0), None);
    }
}

fn paint_sand_dust(c: &mut Canvas, idx: usize) {
    let cx = idx as f32 * SPRITE_SZ as f32 + 64.0;
    let cy = 64.0;
    let g = Paint::Linear {
        p0: [cx - 28.0, cy],
        p1: [cx + 28.0, cy],
        stops: vec![
            (0.0, [255.0 / 255.0, 236.0 / 255.0, 188.0 / 255.0, 0.0]),
            (0.5, [255.0 / 255.0, 236.0 / 255.0, 188.0 / 255.0, 0.72]),
            (1.0, [255.0 / 255.0, 236.0 / 255.0, 188.0 / 255.0, 0.0]),
        ],
    };
    c.fill(&ellipse_poly(cx, cy, 28.0, 3.8, 0.0), &g, None);
}

fn hex4(hex: u32, a: f32) -> [f32; 4] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        a,
    ]
}

/// Bake the 10-sprite atlas (2560x256 logical, 2x supersampled -> 5120x512 RGBA8).
pub fn bake_atlas() -> Canvas {
    let mut c = Canvas::new(SPRITE_COUNT as i32 * SPRITE_SZ * 2, SPRITE_SZ * 2, ATLAS_SCALE);

    // 0: Golden cream camel (stride A)
    paint_camel(&mut c, 0, 0xc28d4f, 0xe7be82, 0xa83428, 0xe8b85c, 0.85);
    // 1: Cinnamon brown camel (stride B)
    paint_camel(&mut c, 1, 0x9c6633, 0xc89258, 0x235a63, 0xe2c27c, -0.85);
    // 2: Arab caravan guide (white thobe, red-checkered keffiyeh + black agal, walking staff)
    paint_traveler(
        &mut c,
        2,
        0xf8f5ee,
        0xd6cdb8,
        0xfbf8f2,
        Some([182.0 / 255.0, 42.0 / 255.0, 42.0 / 255.0, 0.55]),
        0x1e1b18,
        true,
        0.8,
    );
    // 3: Desert traveler (sandstone/indigo bisht over white robe, cream keffiyeh + gold trim)
    paint_traveler(&mut c, 3, 0x2d4b63, 0x1c3144, 0xf7edd8, None, 0x241e18, false, -0.7);
    // 4: Bedouin striped tent + woven rug
    paint_bedouin_tent(&mut c, 4);
    // 5: Mature lush date palm
    paint_palm(&mut c, 5, 9, 44.0, 0x245d31, 0x3b803f, 0x60a64b);
    // 6: Young desert palm
    paint_palm(&mut c, 6, 7, 34.0, 0x2d6836, 0x4a8e46, 0x72b456);
    // 7: Sandstone outcrop & desert scrub
    paint_rock_scrub(&mut c, 7);
    // 8: Lush oasis shore bush with desert blossoms
    paint_oasis_bush(&mut c, 8);
    // 9: Wind-blown sand wisp
    paint_sand_dust(&mut c, 9);

    c
}
