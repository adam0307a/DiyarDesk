use std::f64::consts::PI;

pub const ROACH_COLS: usize = 8;
pub const ROACH_ROWS: usize = 5;
pub const ROACH_CELL: usize = 256; // 2x supersampled (128 * 2) -> 2048 x 1280 RGBA8

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

    fn fill_circle(&mut self, cx: f64, cy: f64, r: f64, col: [u8; 4]) {
        self.fill_sdf(cx - r, cy - r, cx + r, cy + r, col, |x, y| {
            (x - cx).hypot(y - cy) - r
        });
    }

    fn stroke_circle(&mut self, cx: f64, cy: f64, r: f64, w: f64, col: [u8; 4]) {
        let hw = w * 0.5;
        self.fill_sdf(
            cx - r - hw,
            cy - r - hw,
            cx + r + hw,
            cy + r + hw,
            col,
            |x, y| ((x - cx).hypot(y - cy) - r).abs() - hw,
        );
    }

    fn fill_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, rot: f64, col: [u8; 4]) {
        let m = rx.max(ry) + 2.0;
        let c = rot.cos();
        let s = rot.sin();
        let avg_r = (rx + ry) * 0.5;
        self.fill_sdf(cx - m, cy - m, cx + m, cy + m, col, |x, y| {
            let dx = x - cx;
            let dy = y - cy;
            let lx = dx * c + dy * s;
            let ly = -dx * s + dy * c;
            ((lx / rx).hypot(ly / ry) - 1.0) * avg_r
        });
    }

    fn stroke_ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        rot: f64,
        lw: f64,
        col: [u8; 4],
    ) {
        let m = rx.max(ry) + lw + 2.0;
        let c = rot.cos();
        let s = rot.sin();
        let avg_r = (rx + ry) * 0.5;
        let hlw = lw * 0.5;
        self.fill_sdf(cx - m, cy - m, cx + m, cy + m, col, |x, y| {
            let dx = x - cx;
            let dy = y - cy;
            let lx = dx * c + dy * s;
            let ly = -dx * s + dy * c;
            let d = ((lx / rx).hypot(ly / ry) - 1.0) * avg_r;
            d.abs() - hlw
        });
    }

    fn fill_ellipse_radial_grad(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        gx: f64,
        gy: f64,
        gr: f64,
        stops: &[(f64, [u8; 4])],
    ) {
        let m = rx.max(ry) + 2.0;
        let avg_r = (rx + ry) * 0.5;
        let x0 = ((cx - m).floor() as i32).max(0);
        let y0 = ((cy - m).floor() as i32).max(0);
        let x1 = ((cx + m).ceil() as i32).min(self.w as i32 - 1);
        let y1 = ((cy + m).ceil() as i32).min(self.h as i32 - 1);
        for py in y0..=y1 {
            let fy = py as f64 + 0.5;
            for px in x0..=x1 {
                let fx = px as f64 + 0.5;
                let ed = (((fx - cx) / rx).hypot((fy - cy) / ry) - 1.0) * avg_r;
                if ed >= 0.75 {
                    continue;
                }
                let cov = (0.5 - ed).clamp(0.0, 1.0);
                let t = ((fx - gx).hypot(fy - gy) / gr.max(1e-4)).clamp(0.0, 1.0);
                let col = eval_stops(t, stops);
                self.blend_pixel(px, py, col, cov);
            }
        }
    }

    fn fill_round_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64, col: [u8; 4]) {
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let hw = w * 0.5;
        let hh = h * 0.5;
        self.fill_sdf(x, y, x + w, y + h, col, |px, py| {
            let qx = (px - cx).abs() - (hw - r);
            let qy = (py - cy).abs() - (hh - r);
            qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
        });
    }

    fn stroke_round_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: f64,
        lw: f64,
        col: [u8; 4],
    ) {
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let hw = w * 0.5;
        let hh = h * 0.5;
        let hlw = lw * 0.5;
        self.fill_sdf(x - hlw, y - hlw, x + w + hlw, y + h + hlw, col, |px, py| {
            let qx = (px - cx).abs() - (hw - r);
            let qy = (py - cy).abs() - (hh - r);
            let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
            d.abs() - hlw
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
        let steps = 16;
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

    fn stroke_arc(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64, w: f64, col: [u8; 4]) {
        let steps = 18;
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let a = a0 + (a1 - a0) * t;
            pts.push((cx + a.cos() * r, cy + a.sin() * r));
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

    fn stroke_polygon(&mut self, pts: &[(f64, f64)], w: f64, col: [u8; 4]) {
        if pts.len() < 2 {
            return;
        }
        for i in 0..pts.len() {
            let (x0, y0) = pts[i];
            let (x1, y1) = pts[(i + 1) % pts.len()];
            self.draw_line(x0, y0, x1, y1, w, col);
        }
    }

    fn fill_radial_gradient(&mut self, cx: f64, cy: f64, r: f64, stops: &[(f64, [u8; 4])]) {
        let x0 = ((cx - r).floor() as i32 - 1).max(0);
        let y0 = ((cy - r).floor() as i32 - 1).max(0);
        let x1 = ((cx + r).ceil() as i32 + 1).min(self.w as i32 - 1);
        let y1 = ((cy + r).ceil() as i32 + 1).min(self.h as i32 - 1);
        for py in y0..=y1 {
            let fy = py as f64 + 0.5;
            for px in x0..=x1 {
                let fx = px as f64 + 0.5;
                let d = (fx - cx).hypot(fy - cy);
                if d > r + 0.5 {
                    continue;
                }
                let t = (d / r).clamp(0.0, 1.0);
                let col = eval_stops(t, stops);
                let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                self.blend_pixel(px, py, col, cov);
            }
        }
    }

    fn apply_dark_noir_filter(&mut self, ox: usize, oy: usize, size: usize) {
        // Matches: tctx.filter = 'grayscale(92%) contrast(155%) brightness(64%)'
        for y in oy..(oy + size).min(self.h) {
            for x in ox..(ox + size).min(self.w) {
                let idx = (y * self.w + x) * 4;
                if self.px[idx + 3] == 0 {
                    continue;
                }
                let r = self.px[idx] as f64 / 255.0;
                let g = self.px[idx + 1] as f64 / 255.0;
                let b = self.px[idx + 2] as f64 / 255.0;
                let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                for c in 0..3 {
                    let orig = self.px[idx + c] as f64 / 255.0;
                    let gray = orig * 0.08 + luma * 0.92;
                    let cont = (gray - 0.5) * 1.55 + 0.5;
                    let out = (cont * 0.64).clamp(0.0, 1.0);
                    self.px[idx + c] = (out * 255.0).round() as u8;
                }
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

fn paint_carapace_roach(cv: &mut Canvas, ox: f64, oy: f64, kind: usize) {
    let s = if kind == 10 { 2.0 * 0.84 } else { 2.0 };
    let cx = ox + 128.0;
    let cy = oy + 136.0;
    let arm_raise = kind == 5;

    // Back wing shell peeking behind shoulders
    let wing_fill = if kind == 9 {
        [0x8e, 0x45, 0x20, 255]
    } else {
        [0x73, 0x35, 0x19, 255]
    };
    let wing_stroke = [0x3f, 0x1b, 0x0b, 255];
    cv.fill_ellipse(cx - 10.0 * s, cy + 4.0 * s, 13.0 * s, 24.0 * s, 0.24, wing_fill);
    cv.stroke_ellipse(
        cx - 10.0 * s,
        cy + 4.0 * s,
        13.0 * s,
        24.0 * s,
        0.24,
        1.6 * s,
        wing_stroke,
    );
    cv.fill_ellipse(cx + 10.0 * s, cy + 4.0 * s, 13.0 * s, 24.0 * s, -0.24, wing_fill);
    cv.stroke_ellipse(
        cx + 10.0 * s,
        cy + 4.0 * s,
        13.0 * s,
        24.0 * s,
        -0.24,
        1.6 * s,
        wing_stroke,
    );

    // Articulated legs
    let leg_col = [0x5e, 0x29, 0x12, 255];
    cv.draw_polyline(
        &[
            (cx - 7.0 * s, cy + 20.0 * s),
            (cx - 10.0 * s, cy + 34.0 * s),
            (cx - 13.0 * s, cy + 47.0 * s),
            (cx - 18.0 * s, cy + 49.0 * s),
        ],
        3.8 * s,
        leg_col,
    );
    cv.draw_polyline(
        &[
            (cx + 7.0 * s, cy + 20.0 * s),
            (cx + 10.0 * s, cy + 34.0 * s),
            (cx + 13.0 * s, cy + 47.0 * s),
            (cx + 18.0 * s, cy + 49.0 * s),
        ],
        3.8 * s,
        leg_col,
    );

    if kind == 3 {
        // Yellow factory boots
        cv.fill_round_rect(
            cx - 20.0 * s,
            cy + 42.0 * s,
            11.0 * s,
            8.0 * s,
            2.0 * s,
            [0xe7, 0xbd, 0x2a, 255],
        );
        cv.stroke_round_rect(
            cx - 20.0 * s,
            cy + 42.0 * s,
            11.0 * s,
            8.0 * s,
            2.0 * s,
            1.3 * s,
            [0x6d, 0x54, 0x09, 255],
        );
        cv.fill_round_rect(
            cx + 9.0 * s,
            cy + 42.0 * s,
            11.0 * s,
            8.0 * s,
            2.0 * s,
            [0xe7, 0xbd, 0x2a, 255],
        );
        cv.stroke_round_rect(
            cx + 9.0 * s,
            cy + 42.0 * s,
            11.0 * s,
            8.0 * s,
            2.0 * s,
            1.3 * s,
            [0x6d, 0x54, 0x09, 255],
        );
    } else if kind == 9 {
        // Red heels for lady
        cv.fill_polygon(
            &[
                (cx - 19.0 * s, cy + 49.0 * s),
                (cx - 9.0 * s, cy + 45.0 * s),
                (cx - 9.0 * s, cy + 50.0 * s),
            ],
            [0xc8, 0x1c, 0x28, 255],
        );
        cv.fill_polygon(
            &[
                (cx + 19.0 * s, cy + 49.0 * s),
                (cx + 9.0 * s, cy + 45.0 * s),
                (cx + 9.0 * s, cy + 50.0 * s),
            ],
            [0xc8, 0x1c, 0x28, 255],
        );
    }

    // Segmented thorax/abdomen torso
    cv.fill_ellipse_radial_grad(
        cx,
        cy + 7.0 * s,
        15.5 * s,
        22.0 * s,
        cx - 4.0 * s,
        cy + 2.0 * s,
        26.0 * s,
        &[
            (0.0, [0xdf, 0x92, 0x56, 255]),
            (0.55, [0xa8, 0x5a, 0x2c, 255]),
            (1.0, [0x5d, 0x2b, 0x12, 255]),
        ],
    );
    cv.stroke_ellipse(
        cx,
        cy + 7.0 * s,
        15.5 * s,
        22.0 * s,
        0.0,
        1.6 * s,
        [0x3c, 0x1a, 0x09, 255],
    );

    // Carapace belly segments
    for seg in 0..4 {
        let sy = 5.0 + seg as f64 * 5.6;
        cv.stroke_arc(
            cx,
            cy + (sy - 5.0) * s,
            (13.5 - seg as f64 * 0.9) * s,
            0.28,
            PI - 0.28,
            1.4 * s,
            [62, 24, 8, 140],
        );
    }

    // Outfits over torso
    match kind {
        0 => {
            // White tee + open dark biker leather vest
            cv.fill_ellipse(
                cx,
                cy + 4.0 * s,
                13.0 * s,
                15.0 * s,
                0.0,
                [0xf2, 0xf3, 0xf5, 255],
            );
            cv.fill_polygon(
                &[
                    (cx - 14.0 * s, cy - 7.0 * s),
                    (cx - 4.0 * s, cy - 8.0 * s),
                    (cx - 3.0 * s, cy + 19.0 * s),
                    (cx - 15.0 * s, cy + 17.0 * s),
                ],
                [0x25, 0x29, 0x30, 255],
            );
            cv.fill_polygon(
                &[
                    (cx + 14.0 * s, cy - 7.0 * s),
                    (cx + 4.0 * s, cy - 8.0 * s),
                    (cx + 3.0 * s, cy + 19.0 * s),
                    (cx + 15.0 * s, cy + 17.0 * s),
                ],
                [0x25, 0x29, 0x30, 255],
            );
            cv.fill_round_rect(
                cx - 10.0 * s,
                cy,
                2.0 * s,
                2.0 * s,
                0.2,
                [0x9a, 0xa4, 0xb2, 255],
            );
            cv.fill_round_rect(
                cx + 8.0 * s,
                cy,
                2.0 * s,
                2.0 * s,
                0.2,
                [0x9a, 0xa4, 0xb2, 255],
            );
        }
        1 => {
            // Crisp white school shirt + navy tie + tiny emblem
            cv.fill_ellipse(
                cx,
                cy + 3.0 * s,
                14.5 * s,
                15.5 * s,
                0.0,
                [0xf7, 0xf8, 0xfa, 255],
            );
            cv.stroke_ellipse(
                cx,
                cy + 3.0 * s,
                14.5 * s,
                15.5 * s,
                0.0,
                1.2 * s,
                [0xca, 0xd0, 0xdb, 255],
            );
            cv.fill_polygon(
                &[
                    (cx - 2.5 * s, cy - 7.0 * s),
                    (cx + 2.5 * s, cy - 7.0 * s),
                    (cx + 3.5 * s, cy + 11.0 * s),
                    (cx, cy + 15.0 * s),
                    (cx - 3.5 * s, cy + 11.0 * s),
                ],
                [0x24, 0x3c, 0x6d, 255],
            );
            cv.fill_round_rect(
                cx + 5.5 * s,
                cy - 1.0 * s,
                4.5 * s,
                5.0 * s,
                0.5,
                [0xd4, 0xa6, 0x3a, 255],
            );
        }
        2 => {
            // Hi-vis neon yellow safety vest with silver reflective stripes
            cv.fill_ellipse(
                cx,
                cy + 4.0 * s,
                15.0 * s,
                16.0 * s,
                0.0,
                [0xc5, 0xe2, 0x2e, 255],
            );
            cv.stroke_ellipse(
                cx,
                cy + 4.0 * s,
                15.0 * s,
                16.0 * s,
                0.0,
                1.2 * s,
                [0x6c, 0x7c, 0x10, 255],
            );
            cv.fill_round_rect(
                cx - 13.0 * s,
                cy + 3.0 * s,
                26.0 * s,
                3.2 * s,
                0.5,
                [0xe7, 0xed, 0xf2, 255],
            );
            cv.fill_round_rect(
                cx - 11.0 * s,
                cy + 10.0 * s,
                22.0 * s,
                3.0 * s,
                0.5,
                [0xe7, 0xed, 0xf2, 255],
            );
        }
        4 => {
            // Green-white plaid shirt
            cv.fill_ellipse(
                cx,
                cy + 3.0 * s,
                15.0 * s,
                15.0 * s,
                0.0,
                [0xdd, 0xe5, 0xdd, 255],
            );
            let mut gx = -12;
            while gx <= 12 {
                cv.fill_round_rect(
                    cx + gx as f64 * s,
                    cy - 10.0 * s,
                    2.5 * s,
                    26.0 * s,
                    0.5,
                    [42, 92, 66, 184],
                );
                gx += 6;
            }
            let mut gy = -6;
            while gy <= 12 {
                cv.fill_round_rect(
                    cx - 13.0 * s,
                    cy + gy as f64 * s,
                    26.0 * s,
                    2.5 * s,
                    0.5,
                    [42, 92, 66, 184],
                );
                gy += 6;
            }
        }
        6 => {
            // Tattered grey street t-shirt
            cv.fill_ellipse(
                cx,
                cy + 3.0 * s,
                14.8 * s,
                16.0 * s,
                0.0,
                [0x6f, 0x7d, 0x8c, 255],
            );
            cv.fill_circle(
                cx - 6.0 * s,
                cy + 7.0 * s,
                2.6 * s,
                [0xa8, 0x5a, 0x2c, 255],
            );
            cv.fill_circle(
                cx + 5.0 * s,
                cy + 11.0 * s,
                2.2 * s,
                [0xa8, 0x5a, 0x2c, 255],
            );
        }
        7 => {
            // Warm taupe polo + dark shorts
            cv.fill_ellipse(
                cx,
                cy + 2.0 * s,
                15.0 * s,
                15.0 * s,
                0.0,
                [0x8b, 0x71, 0x5a, 255],
            );
            cv.fill_round_rect(
                cx - 12.0 * s,
                cy + 13.0 * s,
                24.0 * s,
                9.0 * s,
                1.0,
                [0x2d, 0x31, 0x38, 255],
            );
        }
        8 => {
            // Dark Beyoglu street jacket
            cv.fill_ellipse(
                cx,
                cy + 3.0 * s,
                15.5 * s,
                16.5 * s,
                0.0,
                [0x38, 0x3d, 0x46, 255],
            );
            cv.fill_round_rect(
                cx - 2.0 * s,
                cy - 9.0 * s,
                4.0 * s,
                26.0 * s,
                0.5,
                [0x20, 0x24, 0x2a, 255],
            );
        }
        9 => {
            // Red evening slit dress
            let pts = [
                (cx - 11.0 * s, cy - 10.0 * s),
                (cx + 11.0 * s, cy - 10.0 * s),
                (cx + 14.0 * s, cy + 30.0 * s),
                (cx + 3.0 * s, cy + 30.0 * s),
                (cx + 1.0 * s, cy + 18.0 * s),
                (cx - 2.0 * s, cy + 30.0 * s),
                (cx - 14.0 * s, cy + 30.0 * s),
            ];
            cv.fill_polygon(&pts, [0xcf, 0x24, 0x28, 255]);
            cv.stroke_polygon(&pts, 1.2 * s, [0x7b, 0x11, 0x14, 255]);
        }
        10 => {
            // White tee with tiny cute graphic squiggle
            cv.fill_ellipse(
                cx,
                cy + 3.0 * s,
                14.5 * s,
                14.5 * s,
                0.0,
                [0xee, 0xf1, 0xf5, 255],
            );
            cv.fill_round_rect(
                cx - 7.0 * s,
                cy,
                14.0 * s,
                2.0 * s,
                0.5,
                [0x2d, 0x35, 0x40, 255],
            );
            cv.fill_round_rect(
                cx - 5.0 * s,
                cy + 4.0 * s,
                10.0 * s,
                2.0 * s,
                0.5,
                [0x2d, 0x35, 0x40, 255],
            );
        }
        11 => {
            // Doctor white coat over grey collared shirt + pens + stethoscope
            cv.fill_ellipse(
                cx,
                cy + 3.0 * s,
                15.0 * s,
                16.0 * s,
                0.0,
                [0x73, 0x7d, 0x87, 255],
            );
            let left_lapel = [
                (cx - 4.0 * s, cy - 9.0 * s),
                (cx - 16.0 * s, cy - 6.0 * s),
                (cx - 17.0 * s, cy + 25.0 * s),
                (cx - 3.0 * s, cy + 25.0 * s),
            ];
            let right_lapel = [
                (cx + 4.0 * s, cy - 9.0 * s),
                (cx + 16.0 * s, cy - 6.0 * s),
                (cx + 17.0 * s, cy + 25.0 * s),
                (cx + 3.0 * s, cy + 25.0 * s),
            ];
            cv.fill_polygon(&left_lapel, [0xf8, 0xfa, 0xfc, 255]);
            cv.stroke_polygon(&left_lapel, 1.2 * s, [0xc0, 0xca, 0xd6, 255]);
            cv.fill_polygon(&right_lapel, [0xf8, 0xfa, 0xfc, 255]);
            cv.stroke_polygon(&right_lapel, 1.2 * s, [0xc0, 0xca, 0xd6, 255]);
            cv.fill_round_rect(
                cx + 8.0 * s,
                cy + 3.0 * s,
                2.0 * s,
                7.0 * s,
                0.3,
                [0x24, 0x4b, 0x96, 255],
            );
            cv.fill_round_rect(
                cx + 11.0 * s,
                cy + 4.0 * s,
                2.0 * s,
                6.0 * s,
                0.3,
                [0xc8, 0x32, 0x32, 255],
            );
            cv.stroke_arc(
                cx,
                cy - 4.0 * s,
                9.5 * s,
                0.15,
                PI - 0.15,
                2.4 * s,
                [0x25, 0x2a, 0x32, 255],
            );
            cv.fill_circle(cx, cy + 8.0 * s, 3.2 * s, [0xb9, 0xc6, 0xd4, 255]);
        }
        12 => {
            // White strapped straitjacket (deli gomlegi)
            cv.fill_ellipse(
                cx,
                cy + 4.0 * s,
                16.5 * s,
                19.0 * s,
                0.0,
                [0xf3, 0xf4, 0xf6, 255],
            );
            cv.stroke_ellipse(
                cx,
                cy + 4.0 * s,
                16.5 * s,
                19.0 * s,
                0.0,
                1.4 * s,
                [0xb8, 0xbf, 0xc9, 255],
            );
            cv.draw_line(
                cx - 14.0 * s,
                cy - 2.0 * s,
                cx + 13.0 * s,
                cy + 10.0 * s,
                4.2 * s,
                [0xcd, 0xd3, 0xdc, 255],
            );
            cv.draw_line(
                cx + 14.0 * s,
                cy - 2.0 * s,
                cx - 13.0 * s,
                cy + 10.0 * s,
                4.2 * s,
                [0xcd, 0xd3, 0xdc, 255],
            );
            cv.fill_round_rect(
                cx - 3.0 * s,
                cy + 2.0 * s,
                6.0 * s,
                4.0 * s,
                0.5,
                [0x9a, 0xa3, 0xb0, 255],
            );
        }
        13 => {
            // Pale mint dotted hospital gown
            cv.fill_ellipse(
                cx,
                cy + 7.0 * s,
                16.5 * s,
                22.0 * s,
                0.0,
                [0xdc, 0xee, 0xf0, 255],
            );
            let mut gy = -10;
            while gy < 28 {
                let mut gx = -12;
                while gx <= 12 {
                    let dx = (gx + if gy % 14 == 0 { 3 } else { 0 }) as f64;
                    let dy = gy as f64;
                    if (dx / 15.0).hypot((dy - 7.0) / 20.0) <= 1.0 {
                        cv.fill_circle(
                            cx + dx * s,
                            cy + dy * s,
                            1.3 * s,
                            [0x7b, 0xb4, 0xc2, 255],
                        );
                    }
                    gx += 7;
                }
                gy += 7;
            }
            cv.stroke_ellipse(
                cx,
                cy + 7.0 * s,
                16.5 * s,
                22.0 * s,
                0.0,
                1.2 * s,
                [0x9a, 0xbd, 0xc5, 255],
            );
        }
        15 => {
            // Electric blue aura + plug cord held out for charging
            cv.fill_ellipse(
                cx,
                cy + 3.0 * s,
                14.5 * s,
                15.5 * s,
                0.0,
                [0x33, 0x41, 0x55, 255],
            );
            cv.draw_quad_bezier(
                cx + 12.0 * s,
                cy + 8.0 * s,
                cx + 25.0 * s,
                cy - 4.0 * s,
                cx + 34.0 * s,
                cy - 12.0 * s,
                2.2 * s,
                [0x5e, 0xea, 0xd4, 255],
            );
            cv.fill_round_rect(
                cx + 32.0 * s,
                cy - 16.0 * s,
                7.0 * s,
                8.0 * s,
                1.5 * s,
                [0xe2, 0xe8, 0xf0, 255],
            );
            cv.fill_round_rect(
                cx + 39.0 * s,
                cy - 15.0 * s,
                5.0 * s,
                1.8 * s,
                0.3,
                [0x94, 0xa3, 0xb8, 255],
            );
            cv.fill_round_rect(
                cx + 39.0 * s,
                cy - 11.0 * s,
                5.0 * s,
                1.8 * s,
                0.3,
                [0x94, 0xa3, 0xb8, 255],
            );
        }
        16 | 17 => {
            // Crisp white dress shirt + black waistcoat/suit + bow tie + silver name tag
            cv.fill_ellipse(
                cx,
                cy + 3.0 * s,
                14.5 * s,
                15.5 * s,
                0.0,
                [0xf8, 0xfa, 0xfc, 255],
            );
            cv.fill_polygon(
                &[
                    (cx - 4.0 * s, cy - 9.0 * s),
                    (cx - 15.5 * s, cy - 5.0 * s),
                    (cx - 15.0 * s, cy + 22.0 * s),
                    (cx - 1.5 * s, cy + 22.0 * s),
                ],
                [0x18, 0x1a, 0x1f, 255],
            );
            cv.fill_polygon(
                &[
                    (cx + 4.0 * s, cy - 9.0 * s),
                    (cx + 15.5 * s, cy - 5.0 * s),
                    (cx + 15.0 * s, cy + 22.0 * s),
                    (cx + 1.5 * s, cy + 22.0 * s),
                ],
                [0x18, 0x1a, 0x1f, 255],
            );
            // Black bow tie
            cv.fill_polygon(
                &[
                    (cx, cy - 6.0 * s),
                    (cx - 6.5 * s, cy - 9.5 * s),
                    (cx - 6.5 * s, cy - 2.5 * s),
                ],
                [0x0f, 0x17, 0x2a, 255],
            );
            cv.fill_polygon(
                &[
                    (cx, cy - 6.0 * s),
                    (cx + 6.5 * s, cy - 9.5 * s),
                    (cx + 6.5 * s, cy - 2.5 * s),
                ],
                [0x0f, 0x17, 0x2a, 255],
            );
            // Silver name tag
            cv.fill_round_rect(
                cx + 4.5 * s,
                cy + 1.0 * s,
                7.5 * s,
                3.2 * s,
                0.5,
                [0xcb, 0xd5, 0xe1, 255],
            );
        }
        _ => {}
    }

    // Arms
    let arm_col = [0x6c, 0x31, 0x16, 255];
    let left_arm = if arm_raise {
        [
            (cx - 13.0 * s, cy - 3.0 * s),
            (cx - 23.0 * s, cy - 18.0 * s),
            (cx - 30.0 * s, cy - 34.0 * s),
        ]
    } else if kind == 12 {
        [
            (cx - 13.0 * s, cy - 3.0 * s),
            (cx - 18.0 * s, cy + 5.0 * s),
            (cx - 4.0 * s, cy + 12.0 * s),
        ]
    } else if kind == 17 {
        [
            (cx - 13.0 * s, cy - 3.0 * s),
            (cx - 24.0 * s, cy - 4.0 * s),
            (cx - 15.0 * s, cy - 18.0 * s),
        ]
    } else if kind == 16 {
        [
            (cx - 13.0 * s, cy - 3.0 * s),
            (cx - 21.0 * s, cy + 7.0 * s),
            (cx - 17.0 * s, cy + 17.0 * s),
        ]
    } else if kind == 0 {
        [
            (cx - 13.0 * s, cy - 3.0 * s),
            (cx - 20.0 * s, cy + 6.0 * s),
            (cx - 16.0 * s, cy + 18.0 * s),
        ]
    } else {
        [
            (cx - 13.0 * s, cy - 3.0 * s),
            (cx - 21.0 * s, cy + 7.0 * s),
            (cx - 18.0 * s, cy + 19.0 * s),
        ]
    };
    cv.draw_polyline(&left_arm, 3.2 * s, arm_col);

    let right_arm = if kind == 12 {
        [
            (cx + 13.0 * s, cy - 3.0 * s),
            (cx + 18.0 * s, cy + 5.0 * s),
            (cx + 4.0 * s, cy + 12.0 * s),
        ]
    } else if kind == 15 {
        [
            (cx + 13.0 * s, cy - 3.0 * s),
            (cx + 24.0 * s, cy + 1.0 * s),
            (cx + 31.0 * s, cy - 8.0 * s),
        ]
    } else {
        [
            (cx + 13.0 * s, cy - 3.0 * s),
            (cx + 22.0 * s, cy + 8.0 * s),
            (cx + 18.0 * s, cy + 20.0 * s),
        ]
    };
    cv.draw_polyline(&right_arm, 3.2 * s, arm_col);

    // Doctor clipboard / Straitjacket fishing rod / Walkie-talkie
    if kind == 11 {
        let bx = cx + 18.0 * s;
        let by = cy + 14.0 * s;
        cv.fill_round_rect(
            bx - 7.0 * s,
            by - 9.0 * s,
            14.0 * s,
            18.0 * s,
            2.0 * s,
            [0xa4, 0x72, 0x44, 255],
        );
        cv.fill_round_rect(
            bx - 5.0 * s,
            by - 6.0 * s,
            10.0 * s,
            13.0 * s,
            0.5,
            [0xf8, 0xfa, 0xfc, 255],
        );
        cv.fill_round_rect(
            bx - 3.5 * s,
            by - 3.0 * s,
            7.0 * s,
            1.5 * s,
            0.3,
            [0x64, 0x74, 0x8b, 255],
        );
        cv.fill_round_rect(
            bx - 3.5 * s,
            by + 1.0 * s,
            5.5 * s,
            1.5 * s,
            0.3,
            [0x64, 0x74, 0x8b, 255],
        );
    } else if kind == 12 {
        cv.draw_line(
            cx + 4.0 * s,
            cy + 12.0 * s,
            cx + 28.0 * s,
            cy - 14.0 * s,
            2.6 * s,
            [0xc8, 0x26, 0x30, 255],
        );
        cv.draw_line(
            cx + 28.0 * s,
            cy - 14.0 * s,
            cx + 28.0 * s,
            cy + 12.0 * s,
            1.0 * s,
            [235, 244, 255, 204],
        );
    } else if kind == 16 || kind == 17 {
        let wx = cx + if kind == 17 { -15.0 } else { -17.0 } * s;
        let wy = cy + if kind == 17 { -18.0 } else { 17.0 } * s;
        cv.fill_round_rect(
            wx - 4.5 * s,
            wy - 7.0 * s,
            9.0 * s,
            14.0 * s,
            2.0 * s,
            [0x11, 0x18, 0x27, 255],
        );
        cv.stroke_round_rect(
            wx - 4.5 * s,
            wy - 7.0 * s,
            9.0 * s,
            14.0 * s,
            2.0 * s,
            1.1 * s,
            [0x47, 0x55, 0x69, 255],
        );
        cv.fill_round_rect(
            wx - 3.0 * s,
            wy - 4.0 * s,
            6.0 * s,
            4.0 * s,
            0.5,
            [0x64, 0x74, 0x8b, 255],
        );
        cv.draw_line(
            wx - 2.0 * s,
            wy - 7.0 * s,
            wx - 3.0 * s,
            wy - 20.0 * s,
            1.8 * s,
            [0x0f, 0x17, 0x2a, 255],
        );
        let led = if kind == 17 {
            [0xef, 0x44, 0x44, 255]
        } else {
            [0x22, 0xc5, 0x5e, 255]
        };
        cv.fill_circle(wx + 2.5 * s, wy - 5.2 * s, 1.1 * s, led);
    }

    // Extra insect middle arms
    cv.draw_line(
        cx - 13.0 * s,
        cy + 6.0 * s,
        cx - 20.0 * s,
        cy + 15.0 * s,
        2.1 * s,
        [0x53, 0x23, 0x0f, 255],
    );
    cv.draw_line(
        cx + 13.0 * s,
        cy + 6.0 * s,
        cx + 20.0 * s,
        cy + 15.0 * s,
        2.1 * s,
        [0x53, 0x23, 0x0f, 255],
    );

    // Diva pink feather boa
    if kind == 5 {
        let pinks = [
            [0xf6, 0x9a, 0xcb, 255],
            [0xec, 0x72, 0xb6, 255],
            [0xff, 0xbb, 0xe0, 255],
            [0xe5, 0x5a, 0xa8, 255],
        ];
        for i in 0..12 {
            let t = i as f64 / 11.0;
            let fx = -10.0 + t * 24.0;
            let fy = 4.0 + t * 14.0 + if t > 0.65 { (t - 0.65) * 18.0 } else { 0.0 };
            cv.fill_circle(
                cx + (fx + (i as f64 * 2.1).sin() * 2.5) * s,
                cy + (fy + (i as f64 * 1.7).cos() * 2.0) * s,
                5.2 * s,
                pinks[i % pinks.len()],
            );
        }
    }

    // Orange juice box in front of vest_juice
    if kind == 0 {
        let jx = cx - 17.0 * s;
        let jy = cy + 25.0 * s;
        cv.fill_round_rect(
            jx - 6.0 * s,
            jy - 8.0 * s,
            12.0 * s,
            16.0 * s,
            1.5 * s,
            [0xe7, 0xed, 0xe0, 255],
        );
        cv.stroke_round_rect(
            jx - 6.0 * s,
            jy - 8.0 * s,
            12.0 * s,
            16.0 * s,
            1.5 * s,
            1.1 * s,
            [0x47, 0x68, 0x32, 255],
        );
        cv.fill_round_rect(
            jx - 5.5 * s,
            jy - 7.5 * s,
            11.0 * s,
            4.5 * s,
            0.5,
            [0x5d, 0xa4, 0x3e, 255],
        );
        cv.fill_circle(jx, jy + 3.0 * s, 3.6 * s, [0xf5, 0x9c, 0x24, 255]);
        cv.draw_polyline(
            &[
                (jx + 1.0 * s, jy - 8.0 * s),
                (jx + 2.0 * s, jy - 14.0 * s),
                (jx + 7.0 * s, jy - 17.0 * s),
            ],
            1.6 * s,
            [255, 255, 255, 255],
        );
    }

    // Head
    let head_y = cy - 20.0 * s;
    cv.fill_ellipse_radial_grad(
        cx,
        head_y,
        15.0 * s,
        14.0 * s,
        cx - 4.0 * s,
        head_y - 4.0 * s,
        17.0 * s,
        &[
            (0.0, [0xea, 0xa2, 0x67, 255]),
            (0.65, [0xb6, 0x61, 0x2f, 255]),
            (1.0, [0x5c, 0x29, 0x11, 255]),
        ],
    );
    cv.stroke_ellipse(
        cx,
        head_y,
        15.0 * s,
        14.0 * s,
        0.0,
        1.6 * s,
        [0x3b, 0x19, 0x08, 255],
    );

    // Antennae
    if kind == 15 {
        cv.draw_polyline(
            &[
                (cx - 5.0 * s, head_y - 12.0 * s),
                (cx - 14.0 * s, head_y - 22.0 * s),
                (cx - 9.0 * s, head_y - 28.0 * s),
                (cx - 22.0 * s, head_y - 36.0 * s),
            ],
            2.2 * s,
            [0x7d, 0xd3, 0xfc, 255],
        );
        cv.draw_polyline(
            &[
                (cx + 5.0 * s, head_y - 12.0 * s),
                (cx + 14.0 * s, head_y - 22.0 * s),
                (cx + 9.0 * s, head_y - 28.0 * s),
                (cx + 22.0 * s, head_y - 36.0 * s),
            ],
            2.2 * s,
            [0x7d, 0xd3, 0xfc, 255],
        );
    } else {
        cv.draw_quad_bezier(
            cx - 5.0 * s,
            head_y - 12.0 * s,
            cx - 18.0 * s,
            head_y - 34.0 * s,
            cx - 30.0 * s,
            head_y - 26.0 * s,
            1.7 * s,
            [0x4c, 0x20, 0x0b, 255],
        );
        cv.draw_quad_bezier(
            cx + 5.0 * s,
            head_y - 12.0 * s,
            cx + 18.0 * s,
            head_y - 34.0 * s,
            cx + 30.0 * s,
            head_y - 26.0 * s,
            1.7 * s,
            [0x4c, 0x20, 0x0b, 255],
        );
    }

    // Eyes
    if kind == 4 {
        cv.fill_ellipse(
            cx - 6.2 * s,
            head_y - 2.0 * s,
            5.2 * s,
            6.0 * s,
            0.0,
            [0x12, 0x13, 0x16, 255],
        );
        cv.fill_ellipse(
            cx + 6.2 * s,
            head_y - 2.0 * s,
            5.2 * s,
            6.0 * s,
            0.0,
            [0x12, 0x13, 0x16, 255],
        );
        cv.fill_circle(
            cx - 7.6 * s,
            head_y - 4.0 * s,
            1.8 * s,
            [255, 255, 255, 255],
        );
        cv.fill_circle(
            cx + 4.8 * s,
            head_y - 4.0 * s,
            1.8 * s,
            [255, 255, 255, 255],
        );
    } else if kind == 9 {
        cv.fill_round_rect(
            cx - 12.0 * s,
            head_y - 5.0 * s,
            10.0 * s,
            6.5 * s,
            2.5 * s,
            [0x18, 0x1a, 0x20, 255],
        );
        cv.fill_round_rect(
            cx + 2.0 * s,
            head_y - 5.0 * s,
            10.0 * s,
            6.5 * s,
            2.5 * s,
            [0x18, 0x1a, 0x20, 255],
        );
        cv.draw_line(
            cx - 2.0 * s,
            head_y - 2.0 * s,
            cx + 2.0 * s,
            head_y - 2.0 * s,
            1.2 * s,
            [0xd4, 0xaa, 0x52, 255],
        );
        cv.fill_circle(cx, head_y - 17.0 * s, 7.5 * s, [0xdf, 0xbe, 0x74, 255]);
        cv.stroke_circle(
            cx,
            head_y - 17.0 * s,
            7.5 * s,
            1.2 * s,
            [0x7f, 0x62, 0x24, 255],
        );
        cv.fill_ellipse(
            cx,
            head_y - 10.0 * s,
            14.5 * s,
            3.5 * s,
            0.0,
            [0xdf, 0xbe, 0x74, 255],
        );
    } else {
        cv.fill_ellipse(
            cx - 5.6 * s,
            head_y - 2.0 * s,
            4.7 * s,
            5.2 * s,
            0.0,
            [0xfb, 0xfa, 0xf5, 255],
        );
        cv.stroke_ellipse(
            cx - 5.6 * s,
            head_y - 2.0 * s,
            4.7 * s,
            5.2 * s,
            0.0,
            1.1 * s,
            [0x3c, 0x1b, 0x0c, 255],
        );
        cv.fill_ellipse(
            cx + 5.6 * s,
            head_y - 2.0 * s,
            4.7 * s,
            5.2 * s,
            0.0,
            [0xfb, 0xfa, 0xf5, 255],
        );
        cv.stroke_ellipse(
            cx + 5.6 * s,
            head_y - 2.0 * s,
            4.7 * s,
            5.2 * s,
            0.0,
            1.1 * s,
            [0x3c, 0x1b, 0x0c, 255],
        );
        cv.fill_circle(
            cx - 5.0 * s,
            head_y - 1.5 * s,
            2.4 * s,
            [0x1d, 0x12, 0x0c, 255],
        );
        cv.fill_circle(
            cx + 5.0 * s,
            head_y - 1.5 * s,
            2.4 * s,
            [0x1d, 0x12, 0x0c, 255],
        );
        cv.fill_circle(
            cx - 6.0 * s,
            head_y - 2.8 * s,
            0.9 * s,
            [255, 255, 255, 255],
        );
        cv.fill_circle(
            cx + 4.0 * s,
            head_y - 2.8 * s,
            0.9 * s,
            [255, 255, 255, 255],
        );

        if kind == 5 {
            for i in -2..=2 {
                let fi = i as f64;
                cv.draw_line(
                    cx + (-5.6 + fi * 2.0) * s,
                    head_y - 6.8 * s,
                    cx + (-6.2 + fi * 2.5) * s,
                    head_y - 10.2 * s,
                    1.3 * s,
                    [0x18, 0x10, 0x10, 255],
                );
                cv.draw_line(
                    cx + (5.6 + fi * 2.0) * s,
                    head_y - 6.8 * s,
                    cx + (6.2 + fi * 2.5) * s,
                    head_y - 10.2 * s,
                    1.3 * s,
                    [0x18, 0x10, 0x10, 255],
                );
            }
        } else if kind == 12 || kind == 13 {
            cv.fill_ellipse(
                cx - 6.0 * s,
                head_y - 2.0 * s,
                5.4 * s,
                6.2 * s,
                0.0,
                [0xfb, 0xfa, 0xf5, 255],
            );
            cv.fill_ellipse(
                cx + 6.0 * s,
                head_y - 1.0 * s,
                5.0 * s,
                5.8 * s,
                0.0,
                [0xfb, 0xfa, 0xf5, 255],
            );
            cv.fill_circle(
                cx - 7.4 * s,
                head_y - 3.6 * s,
                1.7 * s,
                [0x14, 0x0c, 0x08, 255],
            );
            cv.fill_circle(
                cx + 4.3 * s,
                head_y + 0.6 * s,
                1.6 * s,
                [0x14, 0x0c, 0x08, 255],
            );
            cv.draw_line(
                cx - 10.0 * s,
                head_y - 9.0 * s,
                cx - 2.0 * s,
                head_y - 7.0 * s,
                1.6 * s,
                [0x1f, 0x0d, 0x06, 255],
            );
            cv.draw_line(
                cx + 2.0 * s,
                head_y - 8.0 * s,
                cx + 10.0 * s,
                head_y - 10.0 * s,
                1.6 * s,
                [0x1f, 0x0d, 0x06, 255],
            );
        } else if kind == 15 {
            cv.fill_ellipse(
                cx - 5.8 * s,
                head_y - 2.0 * s,
                5.2 * s,
                5.8 * s,
                0.0,
                [0xe0, 0xf8, 0xff, 255],
            );
            cv.fill_ellipse(
                cx + 5.8 * s,
                head_y - 2.0 * s,
                5.2 * s,
                5.8 * s,
                0.0,
                [0xe0, 0xf8, 0xff, 255],
            );
            cv.fill_circle(
                cx - 5.6 * s,
                head_y - 2.0 * s,
                2.3 * s,
                [0x0e, 0xa5, 0xe9, 255],
            );
            cv.fill_circle(
                cx + 5.6 * s,
                head_y - 2.0 * s,
                2.3 * s,
                [0x0e, 0xa5, 0xe9, 255],
            );
            cv.fill_circle(
                cx - 6.4 * s,
                head_y - 3.0 * s,
                0.9 * s,
                [255, 255, 255, 255],
            );
            cv.fill_circle(
                cx + 4.8 * s,
                head_y - 3.0 * s,
                0.9 * s,
                [255, 255, 255, 255],
            );
        } else if kind == 11 {
            cv.stroke_circle(
                cx - 5.6 * s,
                head_y - 2.0 * s,
                5.4 * s,
                1.25 * s,
                [0x33, 0x41, 0x55, 255],
            );
            cv.stroke_circle(
                cx + 5.6 * s,
                head_y - 2.0 * s,
                5.4 * s,
                1.25 * s,
                [0x33, 0x41, 0x55, 255],
            );
            cv.draw_line(
                cx - 0.3 * s,
                head_y - 2.0 * s,
                cx + 0.3 * s,
                head_y - 2.0 * s,
                1.25 * s,
                [0x33, 0x41, 0x55, 255],
            );
        } else if kind == 16 {
            cv.fill_ellipse(
                cx - 5.6 * s,
                head_y - 5.2 * s,
                4.6 * s,
                1.8 * s,
                0.0,
                [0x92, 0x52, 0x2d, 255],
            );
            cv.fill_ellipse(
                cx + 5.6 * s,
                head_y - 5.2 * s,
                4.6 * s,
                1.8 * s,
                0.0,
                [0x92, 0x52, 0x2d, 255],
            );
        } else if kind == 17 {
            cv.fill_polygon(
                &[
                    (cx - 11.0 * s, head_y - 7.5 * s),
                    (cx - 1.0 * s, head_y - 3.0 * s),
                    (cx - 10.0 * s, head_y - 2.2 * s),
                ],
                [0x26, 0x14, 0x0c, 255],
            );
            cv.fill_polygon(
                &[
                    (cx + 11.0 * s, head_y - 7.5 * s),
                    (cx + 1.0 * s, head_y - 3.0 * s),
                    (cx + 10.0 * s, head_y - 2.2 * s),
                ],
                [0x26, 0x14, 0x0c, 255],
            );
            cv.draw_line(
                cx - 11.0 * s,
                head_y - 8.5 * s,
                cx - 1.2 * s,
                head_y - 3.5 * s,
                2.0 * s,
                [0x11, 0x08, 0x04, 255],
            );
            cv.draw_line(
                cx + 11.0 * s,
                head_y - 8.5 * s,
                cx + 1.2 * s,
                head_y - 3.5 * s,
                2.0 * s,
                [0x11, 0x08, 0x04, 255],
            );
        }
    }

    // Mouth expression
    if kind == 17 {
        cv.fill_round_rect(
            cx - 6.2 * s,
            head_y + 4.4 * s,
            12.4 * s,
            4.4 * s,
            1.8 * s,
            [0x1e, 0x0b, 0x08, 255],
        );
        cv.fill_round_rect(
            cx - 4.6 * s,
            head_y + 5.1 * s,
            9.2 * s,
            1.4 * s,
            0.3,
            [0xe2, 0xe8, 0xf0, 255],
        );
        cv.fill_round_rect(
            cx - 4.6 * s,
            head_y + 6.9 * s,
            9.2 * s,
            1.2 * s,
            0.3,
            [0xe2, 0xe8, 0xf0, 255],
        );
    } else if kind == 12 || kind == 13 {
        cv.fill_ellipse(
            cx,
            head_y + 6.8 * s,
            6.2 * s,
            4.6 * s,
            0.0,
            [0x2b, 0x0d, 0x0a, 255],
        );
        cv.fill_ellipse(
            cx,
            head_y + 8.8 * s,
            3.8 * s,
            2.1 * s,
            0.0,
            [0xef, 0x44, 0x44, 255],
        );
        cv.fill_round_rect(
            cx - 3.6 * s,
            head_y + 3.0 * s,
            7.2 * s,
            1.8 * s,
            0.3,
            [255, 255, 255, 255],
        );
    } else if kind == 15 {
        cv.draw_polyline(
            &[
                (cx - 5.0 * s, head_y + 6.5 * s),
                (cx - 2.5 * s, head_y + 4.8 * s),
                (cx, head_y + 7.5 * s),
                (cx + 2.5 * s, head_y + 4.8 * s),
                (cx + 5.0 * s, head_y + 6.5 * s),
            ],
            1.6 * s,
            [0x38, 0x16, 0x0b, 255],
        );
    } else if kind == 5 {
        cv.fill_ellipse(
            cx,
            head_y + 7.2 * s,
            5.8 * s,
            3.2 * s,
            0.0,
            [0xc8, 0x20, 0x34, 255],
        );
        cv.fill_round_rect(
            cx - 3.6 * s,
            head_y + 6.2 * s,
            7.2 * s,
            1.8 * s,
            0.3,
            [255, 255, 255, 255],
        );
    } else if kind == 1 || kind == 4 || kind == 7 || kind == 11 {
        cv.fill_ellipse(
            cx,
            head_y + 7.2 * s,
            5.8 * s,
            3.0 * s,
            0.0,
            [0x38, 0x16, 0x0b, 255],
        );
        cv.fill_round_rect(
            cx - 4.2 * s,
            head_y + 5.6 * s,
            8.4 * s,
            2.0 * s,
            0.3,
            [255, 255, 255, 255],
        );
    } else if kind == 6 {
        cv.draw_quad_bezier(
            cx - 4.5 * s,
            head_y + 7.5 * s,
            cx,
            head_y + 5.5 * s,
            cx + 4.5 * s,
            head_y + 7.5 * s,
            1.6 * s,
            [0x3d, 0x19, 0x0a, 255],
        );
    } else {
        cv.stroke_arc(
            cx,
            head_y + 4.5 * s,
            5.5 * s,
            0.25,
            PI - 0.25,
            1.6 * s,
            [0x3d, 0x19, 0x0a, 255],
        );
    }

    // Headgear
    if kind == 2 {
        // White construction hard hat
        cv.fill_ellipse(
            cx,
            head_y - 10.5 * s,
            13.5 * s,
            8.0 * s,
            0.0,
            [0xee, 0xf2, 0xf5, 255],
        );
        cv.stroke_ellipse(
            cx,
            head_y - 10.5 * s,
            13.5 * s,
            8.0 * s,
            0.0,
            1.4 * s,
            [0x73, 0x82, 0x91, 255],
        );
        cv.fill_round_rect(
            cx - 16.0 * s,
            head_y - 8.5 * s,
            32.0 * s,
            3.6 * s,
            1.5 * s,
            [0xee, 0xf2, 0xf5, 255],
        );
        cv.stroke_round_rect(
            cx - 16.0 * s,
            head_y - 8.5 * s,
            32.0 * s,
            3.6 * s,
            1.5 * s,
            1.4 * s,
            [0x73, 0x82, 0x91, 255],
        );
    } else if kind == 3 {
        // Factory hygiene hairnet
        cv.fill_ellipse(
            cx,
            head_y - 8.5 * s,
            14.5 * s,
            8.5 * s,
            0.0,
            [235, 245, 255, 216],
        );
        cv.stroke_ellipse(
            cx,
            head_y - 8.5 * s,
            14.5 * s,
            8.5 * s,
            0.0,
            1.3 * s,
            [0x9b, 0xb5, 0xca, 255],
        );
    }
}

fn paint_street_lamp(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_radial_gradient(
        cx,
        cy - 34.0 * s,
        44.0 * s,
        &[
            (0.0, [255, 224, 146, 235]),
            (0.35, [255, 178, 72, 107]),
            (1.0, [255, 160, 48, 0]),
        ],
    );
    cv.fill_round_rect(
        cx - 3.0 * s,
        cy - 24.0 * s,
        6.0 * s,
        76.0 * s,
        0.5,
        [0x25, 0x2a, 0x32, 255],
    );
    cv.fill_round_rect(
        cx - 9.0 * s,
        cy + 44.0 * s,
        18.0 * s,
        9.0 * s,
        2.0 * s,
        [0x25, 0x2a, 0x32, 255],
    );
    cv.fill_round_rect(
        cx - 8.0 * s,
        cy - 42.0 * s,
        16.0 * s,
        18.0 * s,
        3.0 * s,
        [0xff, 0xdf, 0x93, 255],
    );
    cv.stroke_round_rect(
        cx - 8.0 * s,
        cy - 42.0 * s,
        16.0 * s,
        18.0 * s,
        3.0 * s,
        2.0 * s,
        [0x23, 0x28, 0x30, 255],
    );
    cv.fill_polygon(
        &[
            (cx - 12.0 * s, cy - 42.0 * s),
            (cx, cy - 52.0 * s),
            (cx + 12.0 * s, cy - 42.0 * s),
        ],
        [0x1f, 0x24, 0x2c, 255],
    );
}

fn paint_juice_prop(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 144.0;
    cv.fill_ellipse(
        cx + 2.0 * s,
        cy + 22.0 * s,
        18.0 * s,
        6.0 * s,
        0.0,
        [0, 0, 0, 71],
    );
    cv.fill_round_rect(
        cx - 16.0 * s,
        cy - 20.0 * s,
        32.0 * s,
        42.0 * s,
        4.0 * s,
        [0xef, 0xf4, 0xe2, 255],
    );
    cv.stroke_round_rect(
        cx - 16.0 * s,
        cy - 20.0 * s,
        32.0 * s,
        42.0 * s,
        4.0 * s,
        2.2 * s,
        [0x47, 0x6b, 0x32, 255],
    );
    cv.fill_round_rect(
        cx - 15.0 * s,
        cy - 19.0 * s,
        30.0 * s,
        11.0 * s,
        1.0,
        [0x5b, 0xa3, 0x3d, 255],
    );
    cv.fill_circle(cx, cy + 7.0 * s, 9.0 * s, [0xf8, 0xa2, 0x24, 255]);
    cv.draw_polyline(
        &[
            (cx + 3.0 * s, cy - 20.0 * s),
            (cx + 4.0 * s, cy - 34.0 * s),
            (cx + 17.0 * s, cy - 41.0 * s),
        ],
        3.5 * s,
        [255, 255, 255, 255],
    );
}

fn paint_tea_glass(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 144.0;
    cv.fill_ellipse(
        cx,
        cy + 18.0 * s,
        22.0 * s,
        6.0 * s,
        0.0,
        [0xec, 0xef, 0xf5, 255],
    );
    cv.stroke_ellipse(
        cx,
        cy + 18.0 * s,
        22.0 * s,
        6.0 * s,
        0.0,
        1.6 * s,
        [0x8b, 0x95, 0xa5, 255],
    );
    let pts = [
        (cx - 11.0 * s, cy - 14.0 * s),
        (cx - 7.5 * s, cy + 2.0 * s),
        (cx - 10.0 * s, cy + 16.0 * s),
        (cx + 10.0 * s, cy + 16.0 * s),
        (cx + 7.5 * s, cy + 2.0 * s),
        (cx + 11.0 * s, cy - 14.0 * s),
    ];
    cv.fill_polygon(&pts, [0x8c, 0x1f, 0x16, 255]);
    cv.stroke_polygon(&pts, 2.0 * s, [255, 255, 255, 166]);
}

fn paint_bubble(cv: &mut Canvas, ox: f64, oy: f64, sym: usize) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_circle(cx, cy - 4.0 * s, 24.0 * s, [255, 250, 240, 240]);
    cv.stroke_circle(
        cx,
        cy - 4.0 * s,
        24.0 * s,
        2.4 * s,
        [92, 54, 28, 140],
    );
    if sym == 0 {
        // Red heart
        cv.fill_circle(
            cx - 5.5 * s,
            cy - 7.5 * s,
            5.8 * s,
            [0xe5, 0x35, 0x48, 255],
        );
        cv.fill_circle(
            cx + 5.5 * s,
            cy - 7.5 * s,
            5.8 * s,
            [0xe5, 0x35, 0x48, 255],
        );
        cv.fill_polygon(
            &[
                (cx - 11.0 * s, cy - 5.0 * s),
                (cx + 11.0 * s, cy - 5.0 * s),
                (cx, cy + 8.0 * s),
            ],
            [0xe5, 0x35, 0x48, 255],
        );
    } else if sym == 1 {
        // Music note
        cv.fill_ellipse(
            cx - 6.0 * s,
            cy + 5.0 * s,
            5.5 * s,
            4.0 * s,
            -0.3,
            [0xd9, 0x48, 0x98, 255],
        );
        cv.fill_ellipse(
            cx + 6.0 * s,
            cy + 2.0 * s,
            5.5 * s,
            4.0 * s,
            -0.3,
            [0xd9, 0x48, 0x98, 255],
        );
        cv.fill_round_rect(
            cx - 1.5 * s,
            cy - 13.0 * s,
            3.0 * s,
            18.0 * s,
            0.5,
            [0xd9, 0x48, 0x98, 255],
        );
        cv.fill_round_rect(
            cx + 10.5 * s,
            cy - 15.0 * s,
            3.0 * s,
            17.0 * s,
            0.5,
            [0xd9, 0x48, 0x98, 255],
        );
        cv.fill_round_rect(
            cx - 1.5 * s,
            cy - 15.0 * s,
            15.0 * s,
            4.5 * s,
            0.5,
            [0xd9, 0x48, 0x98, 255],
        );
    } else {
        // 8-point sparkle star
        let mut pts = Vec::with_capacity(8);
        for i in 0..8 {
            let r = if i % 2 == 0 { 13.0 * s } else { 5.0 * s };
            let a = (i as f64 / 8.0) * PI * 2.0;
            pts.push((cx + a.cos() * r, cy - 4.0 * s + a.sin() * r));
        }
        cv.fill_polygon(&pts, [0xf2, 0xa8, 0x29, 255]);
    }
}

fn paint_bed_roach(cv: &mut Canvas, ox: f64, oy: f64, occupied: bool) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;

    // Headboard & footboard bars
    cv.draw_line(
        cx - 48.0 * s,
        cy - 22.0 * s,
        cx - 48.0 * s,
        cy + 28.0 * s,
        3.2 * s,
        [0xcb, 0xd5, 0xe1, 255],
    );
    cv.draw_line(
        cx - 48.0 * s,
        cy - 14.0 * s,
        cx - 34.0 * s,
        cy - 14.0 * s,
        3.2 * s,
        [0xcb, 0xd5, 0xe1, 255],
    );
    cv.draw_line(
        cx + 48.0 * s,
        cy - 10.0 * s,
        cx + 48.0 * s,
        cy + 28.0 * s,
        3.2 * s,
        [0xcb, 0xd5, 0xe1, 255],
    );

    // Mattress & white hospital sheets
    cv.fill_round_rect(
        cx - 47.0 * s,
        cy + 2.0 * s,
        94.0 * s,
        16.0 * s,
        4.0 * s,
        [0xe2, 0xe8, 0xf0, 255],
    );
    cv.stroke_round_rect(
        cx - 47.0 * s,
        cy + 2.0 * s,
        94.0 * s,
        16.0 * s,
        4.0 * s,
        1.4 * s,
        [0x94, 0xa3, 0xb8, 255],
    );

    // Fluffy white pillow
    cv.fill_ellipse(
        cx - 30.0 * s,
        cy - 1.0 * s,
        14.0 * s,
        7.5 * s,
        -0.1,
        [0xf8, 0xfa, 0xfc, 255],
    );
    cv.stroke_ellipse(
        cx - 30.0 * s,
        cy - 1.0 * s,
        14.0 * s,
        7.5 * s,
        -0.1,
        1.2 * s,
        [0xcb, 0xd5, 0xe1, 255],
    );

    if !occupied {
        cv.fill_round_rect(
            cx + 12.0 * s,
            cy - 2.0 * s,
            32.0 * s,
            12.0 * s,
            4.0 * s,
            [0xba, 0xe6, 0xfd, 255],
        );
        cv.stroke_round_rect(
            cx + 12.0 * s,
            cy - 2.0 * s,
            32.0 * s,
            12.0 * s,
            4.0 * s,
            1.2 * s,
            [0x7d, 0xd3, 0xfc, 255],
        );
        return;
    }

    // Roach head resting on pillow
    cv.fill_ellipse(
        cx - 26.0 * s,
        cy - 7.0 * s,
        13.0 * s,
        10.5 * s,
        -0.15,
        [0x7b, 0x3e, 0x1a, 255],
    );
    cv.draw_quad_bezier(
        cx - 30.0 * s,
        cy - 16.0 * s,
        cx - 36.0 * s,
        cy - 34.0 * s,
        cx - 22.0 * s,
        cy - 40.0 * s,
        1.7 * s,
        [0x5a, 0x29, 0x10, 255],
    );
    cv.draw_quad_bezier(
        cx - 24.0 * s,
        cy - 16.0 * s,
        cx - 20.0 * s,
        cy - 34.0 * s,
        cx - 8.0 * s,
        cy - 36.0 * s,
        1.7 * s,
        [0x5a, 0x29, 0x10, 255],
    );

    // Crazy eyes + shouting open mouth
    cv.fill_circle(cx - 28.0 * s, cy - 11.0 * s, 3.8 * s, [255, 255, 255, 255]);
    cv.fill_circle(cx - 20.0 * s, cy - 10.0 * s, 3.5 * s, [255, 255, 255, 255]);
    cv.fill_circle(
        cx - 28.8 * s,
        cy - 11.8 * s,
        1.4 * s,
        [0x11, 0x18, 0x27, 255],
    );
    cv.fill_circle(
        cx - 19.2 * s,
        cy - 9.2 * s,
        1.3 * s,
        [0x11, 0x18, 0x27, 255],
    );
    cv.fill_ellipse(
        cx - 21.0 * s,
        cy - 4.0 * s,
        4.5 * s,
        3.2 * s,
        0.2,
        [0x2b, 0x0d, 0x0a, 255],
    );

    // Patient body in tied straitjacket under light cyan blanket
    cv.fill_round_rect(
        cx - 16.0 * s,
        cy - 9.0 * s,
        36.0 * s,
        15.0 * s,
        6.0 * s,
        [0xf1, 0xf5, 0xf9, 255],
    );
    cv.stroke_round_rect(
        cx - 16.0 * s,
        cy - 9.0 * s,
        36.0 * s,
        15.0 * s,
        6.0 * s,
        1.2 * s,
        [0x94, 0xa3, 0xb8, 255],
    );
    cv.draw_line(
        cx - 6.0 * s,
        cy - 8.0 * s,
        cx + 4.0 * s,
        cy + 5.0 * s,
        2.0 * s,
        [0x64, 0x74, 0x8b, 255],
    );
    cv.draw_line(
        cx + 4.0 * s,
        cy - 8.0 * s,
        cx - 6.0 * s,
        cy + 5.0 * s,
        2.0 * s,
        [0x64, 0x74, 0x8b, 255],
    );
    cv.fill_round_rect(
        cx + 8.0 * s,
        cy - 7.0 * s,
        36.0 * s,
        14.0 * s,
        5.0 * s,
        [0xba, 0xe6, 0xfd, 255],
    );
    cv.stroke_round_rect(
        cx + 8.0 * s,
        cy - 7.0 * s,
        36.0 * s,
        14.0 * s,
        5.0 * s,
        1.2 * s,
        [0x7d, 0xd3, 0xfc, 255],
    );
    cv.draw_line(
        cx + 41.0 * s,
        cy - 3.0 * s,
        cx + 48.0 * s,
        cy - 11.0 * s,
        2.2 * s,
        [0x5a, 0x29, 0x10, 255],
    );
    cv.draw_line(
        cx + 41.0 * s,
        cy + 2.0 * s,
        cx + 47.0 * s,
        cy - 3.0 * s,
        2.2 * s,
        [0x5a, 0x29, 0x10, 255],
    );
}

fn paint_wall_outlet(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_round_rect(
        cx - 28.0 * s,
        cy - 26.0 * s,
        56.0 * s,
        62.0 * s,
        6.0 * s,
        [0xcb, 0xd5, 0xe1, 255],
    );
    cv.stroke_round_rect(
        cx - 28.0 * s,
        cy - 26.0 * s,
        56.0 * s,
        62.0 * s,
        6.0 * s,
        2.0 * s,
        [0x64, 0x74, 0x8b, 255],
    );
    cv.fill_round_rect(
        cx - 20.0 * s,
        cy - 18.0 * s,
        40.0 * s,
        46.0 * s,
        5.0 * s,
        [0xf8, 0xfa, 0xfc, 255],
    );
    cv.stroke_round_rect(
        cx - 20.0 * s,
        cy - 18.0 * s,
        40.0 * s,
        46.0 * s,
        5.0 * s,
        1.5 * s,
        [0x94, 0xa3, 0xb8, 255],
    );
    cv.fill_circle(cx, cy + 5.0 * s, 13.0 * s, [0xe2, 0xe8, 0xf0, 255]);
    cv.stroke_circle(
        cx,
        cy + 5.0 * s,
        13.0 * s,
        1.3 * s,
        [0x64, 0x74, 0x8b, 255],
    );
    cv.fill_circle(
        cx - 5.2 * s,
        cy + 5.0 * s,
        2.5 * s,
        [0x0f, 0x17, 0x2a, 255],
    );
    cv.fill_circle(
        cx + 5.2 * s,
        cy + 5.0 * s,
        2.5 * s,
        [0x0f, 0x17, 0x2a, 255],
    );
    cv.fill_round_rect(
        cx - 1.2 * s,
        cy - 7.5 * s,
        2.4 * s,
        3.2 * s,
        0.3,
        [0xf5, 0x9e, 0x0b, 255],
    );
    cv.fill_round_rect(
        cx - 1.2 * s,
        cy + 14.3 * s,
        2.4 * s,
        3.2 * s,
        0.3,
        [0xf5, 0x9e, 0x0b, 255],
    );
    cv.fill_circle(cx, cy - 12.0 * s, 2.2 * s, [0x22, 0xc5, 0x5e, 255]);
}

fn paint_zap_spark(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_radial_gradient(
        cx,
        cy,
        44.0 * s,
        &[
            (0.0, [56, 189, 248, 184]),
            (0.55, [56, 189, 248, 64]),
            (1.0, [56, 189, 248, 0]),
        ],
    );
    let rays = [
        [(-26.0, -18.0), (-10.0, -6.0), (0.0, 0.0)],
        [(28.0, -14.0), (12.0, -2.0), (0.0, 0.0)],
        [(-20.0, 22.0), (-6.0, 10.0), (0.0, 0.0)],
        [(22.0, 20.0), (8.0, 8.0), (0.0, 0.0)],
    ];
    for r in &rays {
        cv.draw_polyline(
            &[
                (cx + r[0].0 * s, cy + r[0].1 * s),
                (cx + r[1].0 * s, cy + r[1].1 * s),
                (cx + r[2].0 * s, cy + r[2].1 * s),
            ],
            3.2 * s,
            [0x7d, 0xd3, 0xfc, 255],
        );
    }
    let bolt = [
        (cx + 3.0 * s, cy - 18.0 * s),
        (cx - 9.0 * s, cy + 2.0 * s),
        (cx - 1.0 * s, cy + 2.0 * s),
        (cx - 4.0 * s, cy + 18.0 * s),
        (cx + 10.0 * s, cy - 3.0 * s),
        (cx + 1.0 * s, cy - 3.0 * s),
    ];
    cv.fill_polygon(&bolt, [0xfe, 0xf0, 0x8a, 255]);
    cv.stroke_polygon(&bolt, 1.6 * s, [0x02, 0x84, 0xc7, 255]);
}

fn glyph_5x7(c: char) -> [u8; 7] {
    match c {
        'A' | 'a' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' | 'b' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'D' | 'd' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' | 'e' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'Ğ' | 'ğ' => [0b01110, 0b00000, 0b01111, 0b10000, 0b10011, 0b10001, 0b01111],
        'H' | 'h' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' | 'ı' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'İ' | 'i' => [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
        'K' | 'k' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' | 'l' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' | 'm' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' | 'n' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' | 'o' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' | 'p' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'R' | 'r' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'Ş' | 'ş' => [0b01111, 0b10000, 0b01110, 0b00001, 0b11110, 0b00100, 0b01100],
        'T' | 't' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' | 'u' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'Ü' | 'ü' => [0b01010, 0b00000, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' | 'v' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100],
        'Y' | 'y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' | 'z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        '%' => [0b11001, 0b11010, 0b00100, 0b01000, 0b10110, 0b00110, 0b00000],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        '⚡' => [0b00010, 0b00100, 0b01111, 0b00110, 0b11110, 0b00100, 0b01000],
        _ => [0; 7],
    }
}

fn draw_centered_text(cv: &mut Canvas, cx: f64, cy: f64, text: &str, scale: f64, col: [u8; 4]) {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return;
    }
    let cw = 6.0 * scale;
    let total_w = chars.len() as f64 * cw - scale;
    let start_x = cx - total_w * 0.5;
    let start_y = cy - 3.5 * scale;
    for (idx, &ch) in chars.iter().enumerate() {
        let g = glyph_5x7(ch);
        let ox = start_x + idx as f64 * cw;
        for (ry, &row_bits) in g.iter().enumerate() {
            for rx in 0..5 {
                if (row_bits >> (4 - rx)) & 1 != 0 {
                    cv.fill_round_rect(
                        ox + rx as f64 * scale,
                        start_y + ry as f64 * scale,
                        scale * 1.15,
                        scale * 1.15,
                        0.25 * scale,
                        col,
                    );
                }
            }
        }
    }
}

fn paint_text_bubble(
    cv: &mut Canvas,
    ox: f64,
    oy: f64,
    text: &str,
    bg: [u8; 4],
    border: [u8; 4],
    fg: [u8; 4],
) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    let lines: Vec<&str> = text.split('\n').collect();
    let multi = lines.len() > 1;
    let top_y = if multi { -32.0 } else { -24.0 };
    let box_h = if multi { 44.0 } else { 36.0 };

    cv.fill_round_rect(
        cx - 61.0 * s,
        cy + top_y * s,
        122.0 * s,
        box_h * s,
        12.0 * s,
        bg,
    );
    cv.stroke_round_rect(
        cx - 61.0 * s,
        cy + top_y * s,
        122.0 * s,
        box_h * s,
        12.0 * s,
        2.4 * s,
        border,
    );

    // Speech tail
    let tail = [
        (cx - 10.0 * s, cy + 11.0 * s),
        (cx - 16.0 * s, cy + 25.0 * s),
        (cx + 3.0 * s, cy + 11.0 * s),
    ];
    cv.fill_polygon(&tail, bg);
    cv.stroke_polygon(&tail, 2.4 * s, border);
    cv.fill_round_rect(cx - 11.0 * s, cy + 8.0 * s, 15.0 * s, 5.0 * s, 0.5, bg);

    if multi {
        let mid_y = cy + (top_y + box_h * 0.5) * s;
        let sc0 = (110.0 * s / (lines[0].chars().count().max(1) as f64 * 6.0)).clamp(1.7, 3.0);
        let sc1 = (110.0 * s / (lines[1].chars().count().max(1) as f64 * 6.0)).clamp(1.7, 3.0);
        draw_centered_text(cv, cx, mid_y - 7.5 * s, lines[0], sc0, fg);
        draw_centered_text(cv, cx, mid_y + 7.5 * s, lines[1], sc1, fg);
    } else {
        let sc = (106.0 * s / (text.chars().count().max(1) as f64 * 6.0)).clamp(2.2, 3.6);
        draw_centered_text(cv, cx, cy - 6.0 * s, text, sc, fg);
    }
}

fn paint_soft_shadow(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_ellipse_radial_grad(
        cx,
        cy,
        36.0 * s,
        15.0 * s,
        cx,
        cy,
        38.0 * s,
        &[
            (0.0, [12, 8, 6, 133]),
            (0.6, [12, 8, 6, 56]),
            (1.0, [12, 8, 6, 0]),
        ],
    );
}

fn paint_dark_spotlight(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_radial_gradient(
        cx,
        cy,
        58.0 * s,
        &[
            (0.0, [2, 4, 8, 224]),
            (0.55, [4, 6, 12, 143]),
            (1.0, [4, 6, 12, 0]),
        ],
    );
}

fn paint_fighter_jet(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;

    // Twin afterburner exhaust plumes
    cv.fill_polygon(
        &[
            (cx - 26.0 * s, cy - 6.0 * s),
            (cx - 58.0 * s, cy),
            (cx - 26.0 * s, cy + 6.0 * s),
        ],
        [249, 115, 22, 224],
    );
    cv.fill_polygon(
        &[
            (cx - 26.0 * s, cy - 3.5 * s),
            (cx - 42.0 * s, cy),
            (cx - 26.0 * s, cy + 3.5 * s),
        ],
        [254, 240, 138, 250],
    );

    // Delta main wings + horizontal stabilizers
    let wings = [
        (cx + 14.0 * s, cy - 4.0 * s),
        (cx - 16.0 * s, cy - 42.0 * s),
        (cx - 24.0 * s, cy - 42.0 * s),
        (cx - 14.0 * s, cy - 7.0 * s),
        (cx - 26.0 * s, cy - 20.0 * s),
        (cx - 32.0 * s, cy - 20.0 * s),
        (cx - 28.0 * s, cy - 5.0 * s),
        (cx - 28.0 * s, cy + 5.0 * s),
        (cx - 32.0 * s, cy + 20.0 * s),
        (cx - 26.0 * s, cy + 20.0 * s),
        (cx - 14.0 * s, cy + 7.0 * s),
        (cx - 24.0 * s, cy + 42.0 * s),
        (cx - 16.0 * s, cy + 42.0 * s),
        (cx + 14.0 * s, cy + 4.0 * s),
    ];
    cv.fill_polygon(&wings, [0x47, 0x55, 0x69, 255]);
    cv.stroke_polygon(&wings, 1.8 * s, [0x0f, 0x17, 0x2a, 255]);

    // Underwing missiles
    cv.fill_round_rect(
        cx - 10.0 * s,
        cy - 25.0 * s,
        14.0 * s,
        3.0 * s,
        0.5,
        [0xe2, 0xe8, 0xf0, 255],
    );
    cv.fill_round_rect(
        cx - 10.0 * s,
        cy + 22.0 * s,
        14.0 * s,
        3.0 * s,
        0.5,
        [0xe2, 0xe8, 0xf0, 255],
    );

    // Fuselage + nose cone
    let fuselage = [
        (cx + 46.0 * s, cy),
        (cx + 22.0 * s, cy - 7.5 * s),
        (cx - 28.0 * s, cy - 6.0 * s),
        (cx - 28.0 * s, cy + 6.0 * s),
        (cx + 22.0 * s, cy + 7.5 * s),
    ];
    cv.fill_polygon(&fuselage, [0xcb, 0xd5, 0xe1, 255]);
    cv.stroke_polygon(&fuselage, 1.8 * s, [0x0f, 0x17, 0x2a, 255]);

    // Bubble canopy cockpit
    cv.fill_ellipse(
        cx + 16.0 * s,
        cy,
        11.0 * s,
        4.2 * s,
        0.0,
        [0x38, 0xbd, 0xf8, 255],
    );
    cv.fill_ellipse(
        cx + 18.0 * s,
        cy - 1.0 * s,
        5.5 * s,
        1.5 * s,
        0.0,
        [0xba, 0xe6, 0xfd, 255],
    );

    // Wing roundels
    cv.fill_circle(
        cx - 9.0 * s,
        cy - 24.0 * s,
        4.0 * s,
        [0xdc, 0x26, 0x26, 255],
    );
    cv.fill_circle(
        cx - 9.0 * s,
        cy + 24.0 * s,
        4.0 * s,
        [0xdc, 0x26, 0x26, 255],
    );
    cv.fill_circle(cx - 9.0 * s, cy - 24.0 * s, 1.8 * s, [255, 255, 255, 255]);
    cv.fill_circle(cx - 9.0 * s, cy + 24.0 * s, 1.8 * s, [255, 255, 255, 255]);
}

fn paint_bomb(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    let rot = 0.55f64;
    let c = rot.cos();
    let sn = rot.sin();
    let rot_pt = |lx: f64, ly: f64| (cx + (lx * c - ly * sn) * s, cy + (lx * sn + ly * c) * s);

    let fins = [
        rot_pt(-18.0, 0.0),
        rot_pt(-30.0, -13.0),
        rot_pt(-22.0, -3.0),
        rot_pt(-22.0, 3.0),
        rot_pt(-30.0, 13.0),
    ];
    cv.fill_polygon(&fins, [0xdc, 0x26, 0x26, 255]);
    cv.stroke_polygon(&fins, 1.6 * s, [0x1e, 0x29, 0x3b, 255]);

    let (bcx, bcy) = rot_pt(4.0, 0.0);
    cv.fill_ellipse(bcx, bcy, 22.0 * s, 12.0 * s, -rot, [0x1e, 0x29, 0x3b, 255]);
    cv.stroke_ellipse(
        bcx,
        bcy,
        22.0 * s,
        12.0 * s,
        -rot,
        1.6 * s,
        [0x09, 0x0d, 0x16, 255],
    );

    let band = [
        rot_pt(10.0, -10.0),
        rot_pt(15.0, -10.0),
        rot_pt(15.0, 10.0),
        rot_pt(10.0, 10.0),
    ];
    cv.fill_polygon(&band, [0xfa, 0xcc, 0x15, 255]);

    let (hcx, hcy) = rot_pt(2.0, -5.0);
    cv.fill_ellipse(hcx, hcy, 11.0 * s, 3.0 * s, -rot, [255, 255, 255, 115]);
}

fn paint_explosion(cv: &mut Canvas, ox: f64, oy: f64) {
    let s = 2.0;
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_radial_gradient(
        cx,
        cy,
        56.0 * s,
        &[
            (0.0, [255, 250, 210, 250]),
            (0.35, [251, 146, 60, 235]),
            (0.75, [220, 38, 38, 166]),
            (1.0, [120, 20, 10, 0]),
        ],
    );
    let mut outer_pts = Vec::with_capacity(16);
    for i in 0..16 {
        let a = (i as f64 / 16.0) * PI * 2.0;
        let r = if i % 2 == 0 { 48.0 * s } else { 26.0 * s };
        outer_pts.push((cx + a.cos() * r, cy + a.sin() * r));
    }
    cv.fill_polygon(&outer_pts, [0xf9, 0x73, 0x16, 255]);
    cv.stroke_polygon(&outer_pts, 2.0 * s, [0x7c, 0x2d, 0x12, 255]);

    let mut core_pts = Vec::with_capacity(12);
    for i in 0..12 {
        let a = (i as f64 / 12.0) * PI * 2.0;
        let r = if i % 2 == 0 { 28.0 * s } else { 14.0 * s };
        core_pts.push((cx + a.cos() * r, cy + a.sin() * r));
    }
    cv.fill_polygon(&core_pts, [0xfe, 0xf0, 0x8a, 255]);
}

pub fn build_roach_atlas() -> Vec<u8> {
    let mut cv = Canvas::new(ROACH_COLS * ROACH_CELL, ROACH_ROWS * ROACH_CELL);
    let cell = ROACH_CELL as f64;

    // Row 0: 8 cockroach meme leads (0..8)
    for i in 0..8 {
        paint_carapace_roach(&mut cv, i as f64 * cell, 0.0, i);
    }

    // Row 1: couple (8, 9), buddy (10), doctor (11), crazy jacket/gown (12, 13), bed patient, wall charger (15)
    paint_carapace_roach(&mut cv, 0.0 * cell, 1.0 * cell, 8);
    paint_carapace_roach(&mut cv, 1.0 * cell, 1.0 * cell, 9);
    paint_carapace_roach(&mut cv, 2.0 * cell, 1.0 * cell, 10);
    paint_carapace_roach(&mut cv, 3.0 * cell, 1.0 * cell, 11);
    paint_carapace_roach(&mut cv, 4.0 * cell, 1.0 * cell, 12);
    paint_carapace_roach(&mut cv, 5.0 * cell, 1.0 * cell, 13);
    paint_bed_roach(&mut cv, 6.0 * cell, 1.0 * cell, true);
    paint_carapace_roach(&mut cv, 7.0 * cell, 1.0 * cell, 15);

    // Row 2: streetlamp, juice box, tea glass, emoji bubbles (0,1,2), wall outlet, electric zap
    paint_street_lamp(&mut cv, 0.0 * cell, 2.0 * cell);
    paint_juice_prop(&mut cv, 1.0 * cell, 2.0 * cell);
    paint_tea_glass(&mut cv, 2.0 * cell, 2.0 * cell);
    paint_bubble(&mut cv, 3.0 * cell, 2.0 * cell, 0);
    paint_bubble(&mut cv, 4.0 * cell, 2.0 * cell, 1);
    paint_bubble(&mut cv, 5.0 * cell, 2.0 * cell, 2);
    paint_wall_outlet(&mut cv, 6.0 * cell, 2.0 * cell);
    paint_zap_spark(&mut cv, 7.0 * cell, 2.0 * cell);

    // Row 3: shadow, Turkish bubbles, walkie boss normal (4), call bubble (5), dark-noir angry (6), spotlight (7)
    paint_soft_shadow(&mut cv, 0.0 * cell, 3.0 * cell);
    paint_text_bubble(
        &mut cv,
        1.0 * cell,
        3.0 * cell,
        "LÜLÜLÜLÜ!",
        [0xff, 0xf7, 0xed, 255],
        [0x9a, 0x34, 0x12, 255],
        [0xb9, 0x1c, 0x1c, 255],
    );
    paint_text_bubble(
        &mut cv,
        2.0 * cell,
        3.0 * cell,
        "DUKTUR DUKTUR!",
        [0xec, 0xfe, 0xff, 255],
        [0x0e, 0x74, 0x90, 255],
        [0x0f, 0x17, 0x2a, 255],
    );
    paint_text_bubble(
        &mut cv,
        3.0 * cell,
        3.0 * cell,
        "⚡ ŞARJ %99",
        [0xfe, 0xfc, 0xe8, 255],
        [0x02, 0x84, 0xc7, 255],
        [0x03, 0x69, 0xa1, 255],
    );
    paint_carapace_roach(&mut cv, 4.0 * cell, 3.0 * cell, 16);
    paint_text_bubble(
        &mut cv,
        5.0 * cell,
        3.0 * cell,
        "Kurban olduğum\nşuraları bi aydınlat hele",
        [0xff, 0xfb, 0xeb, 255],
        [0xb4, 0x53, 0x09, 255],
        [0x1e, 0x29, 0x3b, 255],
    );
    paint_carapace_roach(&mut cv, 6.0 * cell, 3.0 * cell, 17);
    cv.apply_dark_noir_filter(6 * ROACH_CELL, 3 * ROACH_CELL, ROACH_CELL);
    paint_dark_spotlight(&mut cv, 7.0 * cell, 3.0 * cell);

    // Row 4: fighter jet (0), falling bomb (1), comic explosion fireball (2), Diva quote bubble (3), empty bed (4)
    paint_fighter_jet(&mut cv, 0.0 * cell, 4.0 * cell);
    paint_bomb(&mut cv, 1.0 * cell, 4.0 * cell);
    paint_explosion(&mut cv, 2.0 * cell, 4.0 * cell);
    paint_text_bubble(
        &mut cv,
        3.0 * cell,
        4.0 * cell,
        "Divalar yemek\nyapmaz!",
        [0xfd, 0xf2, 0xf8, 255],
        [0xbe, 0x18, 0x5d, 255],
        [0x83, 0x18, 0x43, 255],
    );
    paint_bed_roach(&mut cv, 4.0 * cell, 4.0 * cell, false);

    cv.px
}
