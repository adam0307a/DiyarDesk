// Rose Brook 2x sprite atlas — Canvas2D-equivalent CPU painter.
// Port of prototype/rosebrook-blooms.js: makeRoseBrookAtlas()

use super::{RB_ATLAS_COLS, RB_SPRITE_SZ};
use std::f64::consts::{PI, TAU};

type Rgb = [f64; 3];

fn hex(s: &str) -> Rgb {
    let s = s.trim_start_matches('#');
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0) as f64 / 255.0;
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0) as f64 / 255.0;
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0) as f64 / 255.0;
    [r, g, b]
}

fn mix_rgb(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

struct Painter {
    w: usize,
    h: usize,
    scale: f64,
    px: Vec<u8>,
}

impl Painter {
    fn new(logical_w: usize, logical_h: usize, scale: usize) -> Self {
        Self {
            w: logical_w * scale,
            h: logical_h * scale,
            scale: scale as f64,
            px: vec![0u8; logical_w * scale * logical_h * scale * 4],
        }
    }

    #[inline]
    fn blend(&mut self, x: usize, y: usize, rgb: Rgb, a: f64) {
        if a <= 0.001 || x >= self.w || y >= self.h {
            return;
        }
        let i = (y * self.w + x) * 4;
        let sa = a.clamp(0.0, 1.0);
        let da = self.px[i + 3] as f64 / 255.0;
        let oa = sa + da * (1.0 - sa);
        if oa <= 1e-6 {
            return;
        }
        let inv = 1.0 / oa;
        for c in 0..3 {
            let d = self.px[i + c] as f64 / 255.0;
            let v = (rgb[c] * sa + d * da * (1.0 - sa)) * inv;
            self.px[i + c] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        self.px[i + 3] = (oa.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    // ellipse with per-pixel color callback in global (logical) coordinates
    fn ellipse<F>(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, cf: F)
    where
        F: Fn(f64, f64) -> (Rgb, f64),
    {
        let s = self.scale;
        let (scx, scy) = (cx * s, cy * s);
        let (srx, sry) = ((rx * s).max(0.5), (ry * s).max(0.5));
        let mr = srx.max(sry) + 2.0;
        let x0 = ((scx - mr).floor() as isize).max(0) as usize;
        let y0 = ((scy - mr).floor() as isize).max(0) as usize;
        let x1 = ((scx + mr).ceil() as isize).min(self.w as isize) as usize;
        let y1 = ((scy + mr).ceil() as isize).min(self.h as isize) as usize;
        let (cr, sr) = (rot.cos(), rot.sin());
        for py in y0..y1 {
            for px in x0..x1 {
                let dx = px as f64 + 0.5 - scx;
                let dy = py as f64 + 0.5 - scy;
                let lx = dx * cr + dy * sr;
                let ly = -dx * sr + dy * cr;
                let nx = lx / srx;
                let ny = ly / sry;
                let d2 = nx * nx + ny * ny;
                if d2 < 1.35 {
                    let d = d2.sqrt();
                    let g = (nx / srx).hypot(ny / sry).max(1e-6);
                    let cov = (0.5 - (d - 1.0) / g).clamp(0.0, 1.0);
                    if cov > 0.001 {
                        let (rgb, a) = cf(px as f64 / s + 0.5 / s, py as f64 / s + 0.5 / s);
                        self.blend(px, py, rgb, a * cov);
                    }
                }
            }
        }
    }

    fn solid_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, rgb: Rgb, a: f64) {
        self.ellipse(cx, cy, rx, ry, rot, |_, _| (rgb, a));
    }

    fn polygon<F>(&mut self, pts: &[(f64, f64)], cf: F)
    where
        F: Fn(f64, f64) -> (Rgb, f64),
    {
        if pts.len() < 3 {
            return;
        }
        let s = self.scale;
        let sp: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| (x * s, y * s)).collect();
        let (mut mnx, mut mxx, mut mny, mut mxy) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for &(x, y) in &sp {
            mnx = mnx.min(x);
            mxx = mxx.max(x);
            mny = mny.min(y);
            mxy = mxy.max(y);
        }
        let x0 = ((mnx.floor() as isize) - 1).max(0) as usize;
        let x1 = ((mxx.ceil() as isize) + 1).min(self.w as isize) as usize;
        let y0 = ((mny.floor() as isize) - 1).max(0) as usize;
        let y1 = ((mxy.ceil() as isize) + 1).min(self.h as isize) as usize;
        const SUB: usize = 4;
        let n = sp.len();
        let mut xs: Vec<f64> = Vec::with_capacity(16);
        for py in y0..y1 {
            let mut cov_row = vec![0u8; x1.saturating_sub(x0)];
            for sy in 0..SUB {
                let y = py as f64 + (sy as f64 + 0.5) / SUB as f64;
                xs.clear();
                for i in 0..n {
                    let (ax, ay) = sp[i];
                    let (bx, by) = sp[(i + 1) % n];
                    if (ay <= y && by > y) || (by <= y && ay > y) {
                        xs.push(ax + (y - ay) / (by - ay) * (bx - ax));
                    }
                }
                if xs.len() < 2 {
                    continue;
                }
                xs.sort_by(|a, b| a.total_cmp(b));
                for pair in xs.chunks_exact(2) {
                    let px0 = (pair[0].floor() as isize).max(x0 as isize) as usize;
                    let px1 = (pair[1].ceil() as isize).min(x1 as isize) as usize;
                    for px in px0..px1 {
                        let mut hits = 0u8;
                        for sx in 0..SUB {
                            let x = px as f64 + (sx as f64 + 0.5) / SUB as f64;
                            if x >= pair[0] && x <= pair[1] {
                                hits += 1;
                            }
                        }
                        cov_row[px - x0] += hits;
                    }
                }
            }
            for (k, &hits) in cov_row.iter().enumerate() {
                if hits == 0 {
                    continue;
                }
                let px = x0 + k;
                let (rgb, a) = cf(px as f64 / s + 0.5 / s, py as f64 / s + 0.5 / s);
                self.blend(px, py, rgb, a * hits as f64 / (SUB * SUB) as f64);
            }
        }
    }

    fn stroke<F>(&mut self, pts: &[(f64, f64)], width: f64, cf: F)
    where
        F: Fn(f64, f64) -> (Rgb, f64),
    {
        if pts.len() < 2 {
            return;
        }
        let s = self.scale;
        let sp: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| (x * s, y * s)).collect();
        let half = (width * 0.5 * s).max(0.5);
        let pad = half + 1.5;
        let (mut mnx, mut mxx, mut mny, mut mxy) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for &(x, y) in &sp {
            mnx = mnx.min(x);
            mxx = mxx.max(x);
            mny = mny.min(y);
            mxy = mxy.max(y);
        }
        let x0 = ((mnx - pad).floor() as isize).max(0) as usize;
        let x1 = ((mxx + pad).ceil() as isize).min(self.w as isize) as usize;
        let y0 = ((mny - pad).floor() as isize).max(0) as usize;
        let y1 = ((mxy + pad).ceil() as isize).min(self.h as isize) as usize;
        for py in y0..y1 {
            for px in x0..x1 {
                let fx = px as f64 + 0.5;
                let fy = py as f64 + 0.5;
                let mut md = f64::INFINITY;
                for seg in sp.windows(2) {
                    let (ax, ay) = seg[0];
                    let (bx, by) = seg[1];
                    let (bx_, by_) = (bx - ax, by - ay);
                    let den = bx_ * bx_ + by_ * by_;
                    let t = if den > 1e-9 {
                        (((fx - ax) * bx_ + (fy - ay) * by_) / den).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let d = (fx - (ax + bx_ * t)).hypot(fy - (ay + by_ * t));
                    if d < md {
                        md = d;
                    }
                }
                let cov = (half + 0.5 - md).clamp(0.0, 1.0);
                if cov > 0.001 {
                    let (rgb, a) = cf(px as f64 / s, py as f64 / s);
                    self.blend(px, py, rgb, a * cov);
                }
            }
        }
    }

    fn solid_stroke(&mut self, pts: &[(f64, f64)], width: f64, rgb: Rgb, a: f64) {
        self.stroke(pts, width, |_, _| (rgb, a));
    }

}

fn bezier_quad(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), steps: usize) -> Vec<(f64, f64)> {
    (1..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let u = 1.0 - t;
            (u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0,
             u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1)
        })
        .collect()
}

fn bezier_cubic(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), steps: usize) -> Vec<(f64, f64)> {
    (1..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let u = 1.0 - t;
            (u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0,
             u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1)
        })
        .collect()
}

fn draw_shadow(p: &mut Painter, cx: f64, cy: f64, rx: f64, ry: f64) {
    p.solid_ellipse(cx + 5.0, cy + 6.0, rx, ry, 0.15, hex("143016"), 0.28);
}

fn draw_leaf(p: &mut Painter, cx: f64, cy: f64, ang: f64, len: f64, wid: f64, dark: &str, light: &str) {
    let (cr, sr) = (ang.cos(), ang.sin());
    let to_local = |x: f64, y: f64| ((x - cx) * cr + (y - cy) * sr, -(x - cx) * sr + (y - cy) * cr);
    let cd = hex(dark);
    let cl = hex(light);
    let mut pts = vec![(cx, cy)];
    pts.extend(bezier_quad((cx, cy),
        (cx + cr * len * 0.45 - sr * (-wid), cy + sr * len * 0.45 + cr * (-wid)),
        (cx + cr * len, cy + sr * len), 10));
    pts.extend(bezier_quad((cx + cr * len, cy + sr * len),
        (cx + cr * len * 0.45 - sr * wid, cy + sr * len * 0.45 + cr * wid),
        (cx, cy), 10));
    p.polygon(&pts, |x, y| {
        let (lx, _) = to_local(x, y);
        (mix_rgb(cd, cl, lx / len.max(1.0)), 1.0)
    });
    let lines: Vec<(f64, f64)> = (0..=6)
        .map(|i| {
            let t = i as f64 / 6.0;
            (cx + cr * (2.0 + t * (len * 0.78 - 2.0)), cy + sr * (2.0 + t * (len * 0.78 - 2.0)))
        })
        .collect();
    p.solid_stroke(&lines, 1.0, hex("E1FCC8"), 0.28);
}

fn paint_rose(p: &mut Painter, col: usize, outer: &str, mid: &str, inner: &str, hi: &str, radius: f64, layers: usize) {
    let cx = col as f64 * RB_SPRITE_SZ as f64 + 64.0;
    let cy = 64.0;
    draw_shadow(p, cx, cy, radius * 1.05, radius * 0.88);
    let co = hex(outer);
    let cm = hex(mid);
    let ci = hex(inner);
    let ch = hex(hi);
    for i in 0..5 {
        let a = i as f64 * (TAU / 5.0) + 0.35 + col as f64 * 0.4;
        draw_leaf(p, cx, cy, a, radius * 1.48, radius * 0.46, "#23592d", "#4b9249");
    }
    for l in 0..layers {
        let f = l as f64 / (layers - 1).max(1) as f64;
        let count = (7usize).saturating_sub(l).max(4);
        let r_ring = radius * (0.94 - f * 0.62);
        let p_rad = radius * (0.56 - f * 0.24);
        let ca = if f < 0.4 { co } else if f < 0.78 { cm } else { ci };
        for k in 0..count {
            let a = (k as f64 / count as f64) * TAU + l as f64 * 0.72 + col as f64;
            let px = cx + a.cos() * r_ring * 0.46;
            let py = cy + a.sin() * r_ring * 0.46;
            let (cr, sr) = ((a + PI * 0.5).cos(), (a + PI * 0.5).sin());
            let local = |x: f64, y: f64| {
                let dx = x - px;
                let dy = y - py;
                ((dx * cr + dy * sr), (-dx * sr + dy * cr))
            };
            p.ellipse(px, py, p_rad * 0.92, p_rad * 0.74, 0.0, |x, y| {
                let (lx, ly) = local(x, y);
                let d = (lx * lx + ly * ly).sqrt() / (p_rad * 1.15).max(1.0);
                let col = if d < 0.55 { mix_rgb(ch, ca, (d / 0.55).clamp(0.0, 1.0)) }
                          else { mix_rgb(ca, co, ((d - 0.55) / 0.45).clamp(0.0, 1.0)) };
                (col, 1.0)
            });
            let arc_pts: Vec<(f64, f64)> = (0..=8)
                .map(|i| {
                    let aa = PI * 1.15 + (PI * 0.7) * (i as f64 / 8.0);
                    (px + aa.cos() * p_rad * 0.76, py - p_rad * 0.08 + aa.sin() * p_rad * 0.76)
                })
                .collect();
            p.solid_stroke(&arc_pts, 0.9, hex("FFF5F8"), 0.28);
        }
    }
    p.solid_ellipse(cx, cy, radius * 0.23, radius * 0.23, 0.0, ci, 1.0);
    let spiral: Vec<(f64, f64)> = (0..16)
        .map(|t| {
            let a = t as f64 * 0.45;
            let rr = t as f64 * radius * 0.015;
            (cx + a.cos() * rr, cy + a.sin() * rr)
        })
        .collect();
    p.solid_stroke(&spiral, 1.6, ch, 1.0);
    p.solid_ellipse(cx - radius * 0.22, cy - radius * 0.25, 2.2, 2.2, 0.0, [1.0, 1.0, 1.0], 0.65);
}

fn paint_daisy(p: &mut Painter, col: usize, petal_a: &str, petal_b: &str, core: &str, petals: usize, rad: f64) {
    let cx = col as f64 * RB_SPRITE_SZ as f64 + 64.0;
    let cy = 64.0;
    draw_shadow(p, cx, cy, rad * 0.92, rad * 0.8);
    for i in 0..4 {
        draw_leaf(p, cx, cy, i as f64 * 1.57 + 0.4, rad * 1.32, rad * 0.34, "#2d6a36", "#5cab55");
    }
    let pa = hex(petal_a);
    let pb = hex(petal_b);
    for i in 0..petals {
        let a = (i as f64 / petals as f64) * TAU;
        let (cr, sr) = (a.cos(), a.sin());
        let local = |x: f64, y: f64| (x - cx) * cr + (y - cy) * sr;
        p.ellipse(cx + cr * rad * 0.56, cy + sr * rad * 0.56, rad * 0.46, rad * 0.21, a, |x, y| {
            (mix_rgb(pb, pa, (local(x, y) / rad).clamp(0.0, 1.0)), 1.0)
        });
    }
    let cc = hex(core);
    p.ellipse(cx, cy, rad * 0.32, rad * 0.32, 0.0, |x, y| {
        let d = ((x - cx + 2.0).hypot(y - cy + 2.0) / (rad * 0.34)).clamp(0.0, 1.0);
        let c = if d < 0.7 { mix_rgb(hex("fff3a8"), cc, d / 0.7) } else { mix_rgb(cc, hex("b87616"), (d - 0.7) / 0.3) };
        (c, 1.0)
    });
}

fn paint_cluster(p: &mut Painter, col: usize, c1: &str, c2: &str, c3: &str) {
    let cx = col as f64 * RB_SPRITE_SZ as f64 + 64.0;
    let cy = 64.0;
    draw_shadow(p, cx, cy, 26.0, 22.0);
    for i in 0..5 {
        draw_leaf(p, cx, cy, i as f64 * 1.25 + 0.2, 34.0, 10.0, "#235c2f", "#52a34b");
    }
    let (a, b, c) = (hex(c1), hex(c2), hex(c3));
    let pts = [(-10.0, -8.0, 11.0), (10.0, -6.0, 10.0), (-7.0, 9.0, 10.0), (9.0, 8.0, 9.0), (0.0, 0.0, 12.0)];
    for &(dx, dy, rr) in &pts {
        for k in 0..5 {
            let ang = k as f64 * 1.256 + dx;
            let ax = cx + dx + ang.cos() * rr * 0.45;
            let ay = cy + dy + ang.sin() * rr * 0.45;
            p.solid_ellipse(ax, ay, rr * 0.44, rr * 0.44, 0.0, if k % 2 == 0 { a } else { b }, 1.0);
        }
        p.solid_ellipse(cx + dx, cy + dy, rr * 0.25, rr * 0.25, 0.0, c, 1.0);
    }
}

fn paint_fern(p: &mut Painter, col: usize) {
    let cx = col as f64 * RB_SPRITE_SZ as f64 + 64.0;
    let cy = 64.0;
    draw_shadow(p, cx, cy, 28.0, 22.0);
    for i in 0..7 {
        let a = (i as f64 / 7.0) * TAU + 0.25;
        draw_leaf(p, cx, cy, a, 24.0 + (i % 2) as f64 * 10.0, 9.5, "#1e572b", "#62b356");
    }
    for i in 0..4 {
        draw_leaf(p, cx, cy, i as f64 * 1.57 + 0.7, 16.0, 6.5, "#2d763a", "#87d16a");
    }
}

fn paint_petal(p: &mut Painter, col: usize, top: &str, bot: &str) {
    let cx = col as f64 * RB_SPRITE_SZ as f64 + 64.0;
    let cy = 64.0;
    let ct = hex(top);
    let cb = hex(bot);
    let mut pts = vec![(cx, cy - 11.0)];
    pts.extend(bezier_cubic((cx, cy - 11.0), (cx + 12.0, cy - 10.0), (cx + 11.0, cy + 7.0), (cx, cy + 11.0), 10));
    pts.extend(bezier_cubic((cx, cy + 11.0), (cx - 11.0, cy + 7.0), (cx - 12.0, cy - 10.0), (cx, cy - 11.0), 10));
    p.polygon(&pts, |x, y| {
        let d = ((x - cx + 3.0).hypot(y - cy + 4.0) / 14.0).clamp(0.0, 1.0);
        (mix_rgb(ct, cb, d), 1.0)
    });
}

fn paint_butterfly(p: &mut Painter, col: usize, outer: &str, inner: &str) {
    let cx = col as f64 * RB_SPRITE_SZ as f64 + 64.0;
    let cy = 64.0;
    let co = hex(outer);
    let ci = hex(inner);
    for s in [-1.0, 1.0] {
        p.ellipse(cx + s * 9.0, cy - 5.0, 9.0, 6.5, -0.4, |x, y| {
            let d = ((x - (cx + s * 6.0)).hypot(y - (cy - 4.0)) / 18.0).clamp(0.0, 1.0);
            (mix_rgb(ci, co, d), 1.0)
        });
        p.ellipse(cx + s * 7.0, cy + 5.0, 6.5, 4.8, 0.35, |x, y| {
            let d = ((x - (cx + s * 6.0)).hypot(y - (cy - 4.0)) / 18.0).clamp(0.0, 1.0);
            (mix_rgb(ci, co, d), 1.0)
        });
    }
    p.solid_ellipse(cx, cy, 1.8, 7.5, 0.0, hex("2d2420"), 1.0);
}

pub fn build_rosebrook_atlas() -> (u32, u32, Vec<u8>) {
    const SCALE: usize = 2;
    let mut p = Painter::new(RB_ATLAS_COLS * RB_SPRITE_SZ, RB_SPRITE_SZ, SCALE);
    paint_rose(&mut p, 0, "#961231", "#d9234b", "#ff6484", "#ffc0ce", 29.0, 4);
    paint_rose(&mut p, 1, "#c93c62", "#f26b90", "#ffa4be", "#ffe5ee", 28.0, 4);
    paint_rose(&mut p, 2, "#cf892f", "#f4bd58", "#ffe38e", "#fff9df", 26.0, 4);
    paint_rose(&mut p, 3, "#82102c", "#bd1c42", "#eb4e72", "#ffa1b8", 21.0, 3);
    paint_daisy(&mut p, 4, "#ffffff", "#e6ecd8", "#f2ae28", 11, 27.0);
    paint_cluster(&mut p, 5, "#9d7ae6", "#c2a6ff", "#fff0a6");
    paint_cluster(&mut p, 6, "#f6c83e", "#ffe477", "#d97a16");
    paint_fern(&mut p, 7);
    paint_petal(&mut p, 8, "#ff7d9b", "#c61b41");
    paint_petal(&mut p, 9, "#ffc4d4", "#ec5b82");
    paint_butterfly(&mut p, 10, "#f5a631", "#ffe885");
    paint_butterfly(&mut p, 11, "#58b8eb", "#c8f0ff");
    (p.w as u32, p.h as u32, p.px)
}
