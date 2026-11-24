use crate::meadow::{fbm, Mulberry32};
use std::f64::consts::PI;

pub struct SpriteImage {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            px: vec![0u8; w * h * 4],
        }
    }

    #[inline]
    fn blend_pixel(&mut self, x: i32, y: i32, col: [u8; 4], cov: f64) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let a = ((col[3] as f64 / 255.0) * cov).clamp(0.0, 1.0);
        if a <= 1e-4 {
            return;
        }
        let idx = (y as usize * self.w + x as usize) * 4;
        let da = self.px[idx + 3] as f64 / 255.0;
        let out_a = a + da * (1.0 - a);
        if out_a <= 1e-5 {
            return;
        }
        for c in 0..3 {
            let sc = col[c] as f64 / 255.0;
            let dc = self.px[idx + c] as f64 / 255.0;
            let oc = (sc * a + dc * da * (1.0 - a)) / out_a;
            self.px[idx + c] = (oc * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        self.px[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    }

    fn fill_sdf<F: Fn(f64, f64) -> f64>(
        &mut self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        col: [u8; 4],
        sdf: F,
    ) {
        let x0 = (min_x.floor() as i32 - 2).max(0);
        let y0 = (min_y.floor() as i32 - 2).max(0);
        let x1 = (max_x.ceil() as i32 + 2).min(self.w as i32 - 1);
        let y1 = (max_y.ceil() as i32 + 2).min(self.h as i32 - 1);
        for py in y0..=y1 {
            let fy = py as f64 + 0.5;
            for px in x0..=x1 {
                let fx = px as f64 + 0.5;
                let d = sdf(fx, fy);
                if d < 0.75 {
                    let cov = (0.5 - d).clamp(0.0, 1.0);
                    self.blend_pixel(px, py, col, cov);
                }
            }
        }
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, col: [u8; 4]) {
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let hw = w * 0.5;
        let hh = h * 0.5;
        self.fill_sdf(x, y, x + w, y + h, col, |px, py| {
            ((px - cx).abs() - hw).max((py - cy).abs() - hh)
        });
    }

    fn fill_circle(&mut self, cx: f64, cy: f64, r: f64, col: [u8; 4]) {
        self.fill_sdf(cx - r, cy - r, cx + r, cy + r, col, |x, y| {
            (x - cx).hypot(y - cy) - r
        });
    }

    fn fill_upper_semicircle(&mut self, cx: f64, cy: f64, r: f64, col: [u8; 4]) {
        self.fill_sdf(cx - r, cy - r, cx + r, cy + 1.0, col, |x, y| {
            ((x - cx).hypot(y - cy) - r).max(y - cy)
        });
    }

    fn fill_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, col: [u8; 4]) {
        let m = rx.max(ry) + 2.0;
        let avg_r = (rx + ry) * 0.5;
        self.fill_sdf(cx - m, cy - m, cx + m, cy + m, col, |x, y| {
            (((x - cx) / rx).hypot((y - cy) / ry) - 1.0) * avg_r
        });
    }

    fn draw_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, w: f64, col: [u8; 4]) {
        let r = w * 0.5;
        let min_x = x0.min(x1) - r;
        let min_y = y0.min(y1) - r;
        let max_x = x0.max(x1) + r;
        let max_y = y0.max(y1) + r;
        let vx = x1 - x0;
        let vy = y1 - y0;
        let l2 = (vx * vx + vy * vy).max(1e-6);
        self.fill_sdf(min_x, min_y, max_x, max_y, col, |x, y| {
            let t = (((x - x0) * vx + (y - y0) * vy) / l2).clamp(0.0, 1.0);
            (x - (x0 + t * vx)).hypot(y - (y0 + t * vy)) - r
        });
    }

    fn draw_polyline(&mut self, pts: &[(f64, f64)], w: f64, col: [u8; 4]) {
        for seg in pts.windows(2) {
            self.draw_line(seg[0].0, seg[0].1, seg[1].0, seg[1].1, w, col);
        }
    }

    fn draw_quad_bezier(
        &mut self,
        x0: f64,
        y0: f64,
        cx: f64,
        cy: f64,
        x1: f64,
        y1: f64,
        w: f64,
        col: [u8; 4],
    ) {
        let steps = 14;
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let u = 1.0 - t;
            pts.push((
                u * u * x0 + 2.0 * u * t * cx + t * t * x1,
                u * u * y0 + 2.0 * u * t * cy + t * t * y1,
            ));
        }
        self.draw_polyline(&pts, w, col);
    }

    fn fill_polygon(&mut self, pts: &[(f64, f64)], col: [u8; 4]) {
        if pts.len() < 3 {
            return;
        }
        let mut min_x = 1e9f64;
        let mut min_y = 1e9f64;
        let mut max_x = -1e9f64;
        let mut max_y = -1e9f64;
        for &(x, y) in pts {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        self.fill_sdf(min_x, min_y, max_x, max_y, col, |px, py| {
            let mut inside = false;
            let mut min_d = 1e9f64;
            let n = pts.len();
            for i in 0..n {
                let (x0, y0) = pts[i];
                let (x1, y1) = pts[(i + 1) % n];
                if ((y0 > py) != (y1 > py)) && (px < (x1 - x0) * (py - y0) / (y1 - y0) + x0) {
                    inside = !inside;
                }
                let vx = x1 - x0;
                let vy = y1 - y0;
                let l2 = (vx * vx + vy * vy).max(1e-6);
                let t = (((px - x0) * vx + (py - y0) * vy) / l2).clamp(0.0, 1.0);
                let d = (px - (x0 + t * vx)).hypot(py - (y0 + t * vy));
                if d < min_d {
                    min_d = d;
                }
            }
            if inside {
                -min_d
            } else {
                min_d
            }
        });
    }

    fn fill_radial_gradient(&mut self, cx: f64, cy: f64, r0: f64, r1: f64, stops: &[(f64, [u8; 4])]) {
        let x0 = ((cx - r1).floor() as i32 - 1).max(0);
        let y0 = ((cy - r1).floor() as i32 - 1).max(0);
        let x1 = ((cx + r1).ceil() as i32 + 1).min(self.w as i32 - 1);
        let y1 = ((cy + r1).ceil() as i32 + 1).min(self.h as i32 - 1);
        for py in y0..=y1 {
            let fy = py as f64 + 0.5;
            for px in x0..=x1 {
                let fx = px as f64 + 0.5;
                let d = (fx - cx).hypot(fy - cy);
                if d > r1 + 0.5 {
                    continue;
                }
                let t = ((d - r0) / (r1 - r0).max(1e-4)).clamp(0.0, 1.0);
                let col = eval_stops(t, stops);
                let cov = (r1 + 0.5 - d).clamp(0.0, 1.0);
                self.blend_pixel(px, py, col, cov);
            }
        }
    }

    fn fill_vertical_gradient_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        stops: &[(f64, [u8; 4])],
    ) {
        let x0 = (x.floor() as i32).max(0);
        let y0 = (y.floor() as i32).max(0);
        let x1 = ((x + w).ceil() as i32 - 1).min(self.w as i32 - 1);
        let y1 = ((y + h).ceil() as i32 - 1).min(self.h as i32 - 1);
        for py in y0..=y1 {
            let fy = py as f64 + 0.5;
            let t = ((fy - y) / h.max(1e-4)).clamp(0.0, 1.0);
            let col = eval_stops(t, stops);
            for px in x0..=x1 {
                self.blend_pixel(px, py, col, 1.0);
            }
        }
    }
}

fn eval_stops(t: f64, stops: &[(f64, [u8; 4])]) -> [u8; 4] {
    let mut col = stops.last().unwrap().1;
    for w in stops.windows(2) {
        if t >= w[0].0 && t <= w[1].0 {
            let u = ((t - w[0].0) / (w[1].0 - w[0].0).max(1e-5)).clamp(0.0, 1.0);
            col = [
                (w[0].1[0] as f64 + (w[1].1[0] as f64 - w[0].1[0] as f64) * u).round() as u8,
                (w[0].1[1] as f64 + (w[1].1[1] as f64 - w[0].1[1] as f64) * u).round() as u8,
                (w[0].1[2] as f64 + (w[1].1[2] as f64 - w[0].1[2] as f64) * u).round() as u8,
                (w[0].1[3] as f64 + (w[1].1[3] as f64 - w[0].1[3] as f64) * u).round() as u8,
            ];
            break;
        }
    }
    col
}

pub fn build_pine_sprite(variant: usize) -> SpriteImage {
    let s = 2.0;
    let w = 60.0 * s;
    let h = 96.0 * s;
    let mut cv = Canvas::new(w as usize, h as usize);
    let cx = w * 0.5;
    cv.fill_rect(cx - w * 0.045, h * 0.82, w * 0.09, h * 0.18, [0x1c, 0x14, 0x10, 255]);

    let layers = if variant == 1 { 4 } else { 3 };
    let layer_cols = [
        [0x15, 0x27, 0x1f, 255],
        [0x12, 0x23, 0x1d, 255],
        [0x10, 0x1f, 0x1a, 255],
        [0x0e, 0x1c, 0x17, 255],
    ];
    for i in 0..layers {
        let yb = h * (0.92 - i as f64 * (0.6 / layers as f64));
        let spread = if variant == 2 { 0.58 } else { 0.46 };
        let hw = w * spread * (1.0 - i as f64 * (0.22 / layers as f64 + 0.02));
        let ya = yb - h * if variant == 2 { 0.3 } else { 0.36 };

        let mut poly = Vec::with_capacity(28);
        for st in 0..=12 {
            let t = st as f64 / 12.0;
            let u = 1.0 - t;
            let px = u * u * cx + 2.0 * u * t * (cx - hw * 0.55) + t * t * (cx - hw);
            let py = u * u * ya + 2.0 * u * t * (yb - h * 0.16) + t * t * yb;
            poly.push((px, py));
        }
        for st in 0..=12 {
            let t = st as f64 / 12.0;
            let u = 1.0 - t;
            let px = u * u * (cx + hw) + 2.0 * u * t * (cx + hw * 0.55) + t * t * cx;
            let py = u * u * yb + 2.0 * u * t * (yb - h * 0.16) + t * t * ya;
            poly.push((px, py));
        }
        cv.fill_polygon(&poly, layer_cols[i % 4]);

        let snow_a = ((0.2 + 0.32 * (1.0 - i as f64 / layers as f64)) * 255.0).round() as u8;
        let snow_col = [222, 234, 247, snow_a];
        let lw = (w * 0.035).max(1.2 * s);
        cv.draw_quad_bezier(
            cx,
            ya,
            cx - hw * 0.55,
            yb - h * 0.16,
            cx - hw * 0.8,
            yb - h * 0.045,
            lw,
            snow_col,
        );
        cv.draw_quad_bezier(
            cx,
            ya,
            cx + hw * 0.55,
            yb - h * 0.16,
            cx + hw * 0.8,
            yb - h * 0.045,
            lw,
            snow_col,
        );
    }

    let tip_y = h * (0.92 - layers as f64 * (0.6 / layers as f64)) + h * 0.02;
    cv.fill_circle(cx, tip_y, w * 0.05, [228, 238, 250, 191]);

    SpriteImage {
        w: cv.w,
        h: cv.h,
        rgba: cv.px,
    }
}

pub fn build_moon_sprite() -> SpriteImage {
    let s = 2.0;
    let w = (110.0 * s) as usize;
    let h = (110.0 * s) as usize;
    let mut cv = Canvas::new(w, h);
    cv.fill_radial_gradient(
        55.0 * s,
        55.0 * s,
        8.0 * s,
        52.0 * s,
        &[
            (0.0, [244, 238, 216, 128]),
            (0.4, [244, 238, 216, 36]),
            (1.0, [244, 238, 216, 0]),
        ],
    );
    cv.fill_circle(55.0 * s, 55.0 * s, 17.0 * s, [0xf2, 0xee, 0xdd, 255]);
    cv.fill_circle(49.0 * s, 50.0 * s, 4.0 * s, [214, 208, 184, 178]);
    cv.fill_circle(60.0 * s, 60.0 * s, 3.0 * s, [214, 208, 184, 178]);
    cv.fill_circle(58.0 * s, 47.0 * s, 2.2 * s, [214, 208, 184, 178]);
    SpriteImage {
        w: cv.w,
        h: cv.h,
        rgba: cv.px,
    }
}

pub fn build_cabin_sprite() -> SpriteImage {
    let s = 2.0;
    let w = (140.0 * s) as usize;
    let h = (110.0 * s) as usize;
    let mut cv = Canvas::new(w, h);
    // Drop shadow
    cv.fill_ellipse(72.0 * s, 104.0 * s, 56.0 * s, 6.0 * s, [0x17, 0x13, 0x10, 255]);
    // Log wall
    cv.fill_rect(30.0 * s, 58.0 * s, 80.0 * s, 46.0 * s, [0x3a, 0x2c, 0x22, 255]);
    let mut y = 66;
    while y < 104 {
        cv.draw_line(
            30.0 * s,
            y as f64 * s,
            110.0 * s,
            y as f64 * s,
            1.0 * s,
            [0, 0, 0, 77],
        );
        y += 8;
    }
    // Snow roof
    let mut roof = Vec::with_capacity(20);
    roof.push((20.0 * s, 60.0 * s));
    roof.push((70.0 * s, 26.0 * s));
    roof.push((120.0 * s, 60.0 * s));
    for i in 1..=14 {
        let t = i as f64 / 14.0;
        let u = 1.0 - t;
        let px = u * u * 120.0 + 2.0 * u * t * 70.0 + t * t * 20.0;
        let py = u * u * 60.0 + 2.0 * u * t * 68.0 + t * t * 60.0;
        roof.push((px * s, py * s));
    }
    cv.fill_polygon(&roof, [0xe9, 0xef, 0xf8, 255]);
    cv.draw_polyline(
        &[(20.0 * s, 60.0 * s), (70.0 * s, 26.0 * s), (120.0 * s, 60.0 * s)],
        1.4 * s,
        [180, 196, 216, 128],
    );
    // Chimney
    cv.fill_rect(88.0 * s, 32.0 * s, 10.0 * s, 22.0 * s, [0x2a, 0x20, 0x1a, 255]);
    cv.fill_rect(86.0 * s, 30.0 * s, 14.0 * s, 5.0 * s, [0xe9, 0xef, 0xf8, 255]);
    // Door + warm slit
    cv.fill_rect(64.0 * s, 74.0 * s, 16.0 * s, 30.0 * s, [0x24, 0x1a, 0x12, 255]);
    cv.fill_rect(65.0 * s, 76.0 * s, 14.0 * s, 28.0 * s, [255, 205, 130, 41]);
    // Warm left window pane
    cv.fill_rect(42.0 * s, 74.0 * s, 14.0 * s, 13.0 * s, [255, 210, 140, 220]);

    SpriteImage {
        w: cv.w,
        h: cv.h,
        rgba: cv.px,
    }
}

pub fn build_glow_sprite() -> SpriteImage {
    let s = 2.0;
    let w = (120.0 * s) as usize;
    let h = (120.0 * s) as usize;
    let mut cv = Canvas::new(w, h);
    cv.fill_radial_gradient(
        60.0 * s,
        60.0 * s,
        2.0 * s,
        58.0 * s,
        &[
            (0.0, [255, 186, 110, 128]),
            (0.35, [255, 170, 90, 41]),
            (1.0, [255, 170, 90, 0]),
        ],
    );
    SpriteImage {
        w: cv.w,
        h: cv.h,
        rgba: cv.px,
    }
}

pub fn build_streak_sprite() -> SpriteImage {
    let s = 2.0;
    let w = 256.0 * s;
    let h = 160.0 * s;
    let mut cv = Canvas::new(w as usize, h as usize);
    let mut rng = Mulberry32::new(7331);
    for _ in 0..26 {
        let x = rng.next() * w;
        let bw = (3.0 + rng.next() * 11.0) * s;
        let a = ((0.12 + rng.next() * 0.3) * 255.0).round() as u8;
        cv.fill_vertical_gradient_rect(
            x,
            0.0,
            bw,
            h,
            &[
                (0.0, [255, 255, 255, 0]),
                (0.55, [255, 255, 255, a]),
                (1.0, [255, 255, 255, 0]),
            ],
        );
    }
    SpriteImage {
        w: cv.w,
        h: cv.h,
        rgba: cv.px,
    }
}

pub fn compute_ridge(w: f64, seed: i32, k: i32, base_y: f64, amp: f64, sharp: f64) -> Vec<[f64; 3]> {
    let mut pts = Vec::new();
    let mut x = -40.0;
    while x <= w + 40.0 {
        let n = fbm(x * 0.0035, k as f64 * 13.7, k, seed) * 0.62
            + (1.0 - (x * 0.0042 + k as f64 * 2.1).sin().abs()).powf(2.4) * 0.5 * sharp;
        pts.push([x, base_y - amp * n, n]);
        x += 34.0;
    }
    pts
}

fn paint_ridge(
    cv: &mut Canvas,
    w: f64,
    pts: &[[f64; 3]],
    close_y: f64,
    fill: [u8; 4],
    rim: Option<[u8; 4]>,
) {
    let mut poly = Vec::with_capacity(pts.len() + 2);
    for p in pts {
        poly.push((p[0], p[1]));
    }
    poly.push((w + 40.0, close_y));
    poly.push((-40.0, close_y));
    cv.fill_polygon(&poly, fill);

    if let Some(rim_col) = rim {
        let line_pts: Vec<(f64, f64)> = pts.iter().map(|p| (p[0], p[1])).collect();
        cv.draw_polyline(&line_pts, 2.5, rim_col);
        for i in 1..(pts.len().saturating_sub(1)) {
            if pts[i][2] > 0.72 && pts[i][2] >= pts[i - 1][2] && pts[i][2] >= pts[i + 1][2] {
                cv.fill_ellipse(
                    pts[i][0],
                    pts[i][1] + 2.0,
                    10.0 + pts[i][2] * 10.0,
                    4.0,
                    [226, 238, 250, 102],
                );
            }
        }
    }
}

pub fn build_shore_static_image(
    w: f64,
    h: f64,
    seed: i32,
    ground_top: f64,
    ground_h: f64,
    lake_top: f64,
) -> SpriteImage {
    let iw = (w.round() as usize).max(16);
    let ih = (h.round() as usize).max(16);
    let mut cv = Canvas::new(iw, ih);
    let mut rng = Mulberry32::new(seed ^ (0x9e3779b9u32 as i32));

    // Far mountains (k = 1)
    let far_pts = compute_ridge(
        w,
        seed,
        1,
        ground_top - ground_h * 0.28,
        ground_h * 0.9,
        0.4,
    );
    paint_ridge(&mut cv, w, &far_pts, ground_top + 20.0, [38, 60, 88, 217], None);

    // Near mountains (k = 2) + snow ridge rim & snow caps
    let near_pts = compute_ridge(w, seed, 2, ground_top + 6.0, ground_h * 0.72, 1.0);
    paint_ridge(
        &mut cv,
        w,
        &near_pts,
        ground_top + 40.0,
        [0x1d, 0x30, 0x49, 255],
        Some([214, 230, 246, 77]),
    );

    // Snow plain gradient
    cv.fill_vertical_gradient_rect(
        0.0,
        ground_top,
        w,
        ground_h,
        &[
            (0.0, [0xae, 0xbc, 0xd8, 255]),
            (0.7, [0x9c, 0xae, 0xcc, 255]),
            (1.0, [0x8e, 0xa2, 0xc6, 255]),
        ],
    );

    // Distant forest silhouette at foot of mountains
    let mut x = -20.0;
    while x < w + 20.0 {
        let w2 = 10.0 + rng.next() * 16.0;
        let h2 = 10.0 + rng.next() * 20.0;
        let y0 = ground_top + 14.0 + rng.next() * 8.0;
        cv.fill_polygon(
            &[
                (x, y0 + h2 * 0.3),
                (x + w2 * 0.5, y0 - h2),
                (x + w2, y0 + h2 * 0.3),
            ],
            [31, 49, 68, 230],
        );
        if rng.next() < 0.5 {
            cv.fill_circle(x + w2 * 0.5, y0 - h2 + 2.0, 1.6, [222, 234, 247, 77]);
        }
        x += w2 * 0.6 + 2.0;
    }

    // Static snow plain sparkles
    let sparkle_cnt = ((ground_h * w) / 1400.0).ceil() as usize;
    for _ in 0..sparkle_cnt {
        let sx = rng.next() * w;
        let sy = ground_top + rng.next() * ground_h;
        let sa = ((0.08 + rng.next() * 0.3) * 255.0).round() as u8;
        cv.fill_rect(sx, sy, 1.4, 1.4, [240, 248, 255, sa]);
    }

    // Shoreline highlight + snow mounds
    cv.draw_line(0.0, lake_top, w, lake_top, 1.5, [226, 238, 250, 89]);
    let mound_cnt = (w / 90.0).ceil() as usize;
    for _ in 0..mound_cnt {
        let mx = rng.next() * w;
        let mr = 5.0 + rng.next() * 9.0;
        let my = lake_top - 2.0 - rng.next() * 5.0;
        cv.fill_upper_semicircle(mx, my, mr, [214, 228, 244, 230]);
    }

    let _ = PI;
    SpriteImage {
        w: cv.w,
        h: cv.h,
        rgba: cv.px,
    }
}
