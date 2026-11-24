use std::f64::consts::PI;

/// Exact Mulberry32 PRNG matching `IST.rng` in `prototype/istanbul-city.js:4`.
#[derive(Clone, Copy, Debug)]
pub struct Mulberry32 {
    state: i32,
}

impl Mulberry32 {
    pub fn new(seed: i32) -> Self {
        Self { state: seed }
    }

    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6D2B_79F5u32 as i32);
        let a = self.state;
        let mut t = (a ^ ((a as u32) >> 15) as i32).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ ((t as u32) >> 7) as i32).wrapping_mul(61 | t)) ^ t;
        let u = (t as u32) ^ ((t as u32) >> 14);
        u as f64 / 4_294_967_296.0
    }
}

/// Exact deterministic hash matching `IST.hash` in `prototype/istanbul-props.js:19`.
#[inline]
pub fn ist_hash(a: f64, b: f64) -> f64 {
    let s = (a * 12.9898 + b * 78.233).sin() * 43758.5453;
    s - s.floor()
}

#[derive(Clone, Copy, Debug)]
pub struct IstanbulPal {
    pub land: [f64; 4],
    pub road: [f64; 4],
    pub edge: [f64; 4],
    pub sea: [[f64; 4]; 2],
    pub roof: [[f64; 4]; 5],
    pub park: [f64; 4],
    pub tint: [f64; 4],
    pub win: f64,
    pub sh: [f64; 2],
    pub glint: [u8; 3],
    pub ga: f64,
    pub sun: [f64; 2],
    pub sun_col: [u8; 3],
}

const fn hex_rgba(hex: u32, a: f64) -> [f64; 4] {
    [
        ((hex >> 16) & 0xFF) as f64 / 255.0,
        ((hex >> 8) & 0xFF) as f64 / 255.0,
        (hex & 0xFF) as f64 / 255.0,
        a,
    ]
}

const fn rgba255(r: u8, g: u8, b: u8, a: f64) -> [f64; 4] {
    [r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0, a]
}

/// `IST.PAL[1..=4]` (`prototype/istanbul-city.js:8-13`). Index 0 is unused.
pub const PALETTES: [IstanbulPal; 5] = [
    // 0: unused placeholder (mirrors Sunset)
    IstanbulPal {
        land: hex_rgba(0xc9a98a, 1.0),
        road: hex_rgba(0x9a8068, 1.0),
        edge: hex_rgba(0xdcb88f, 1.0),
        sea: [hex_rgba(0xe0814f, 1.0), hex_rgba(0x18507c, 1.0)],
        roof: [
            hex_rgba(0xb8553f, 1.0),
            hex_rgba(0xc97652, 1.0),
            hex_rgba(0xd9a778, 1.0),
            hex_rgba(0x9c4638, 1.0),
            hex_rgba(0xc58c5a, 1.0),
        ],
        park: hex_rgba(0x6f8a4c, 1.0),
        tint: rgba255(255, 128, 48, 0.17),
        win: 0.28,
        sh: [5.0, 2.0],
        glint: [255, 200, 120],
        ga: 0.9,
        sun: [0.08, 0.3],
        sun_col: [255, 150, 70],
    },
    // 1: Morning
    IstanbulPal {
        land: hex_rgba(0xd8c8a6, 1.0),
        road: hex_rgba(0xbdb199, 1.0),
        edge: hex_rgba(0xeadfc4, 1.0),
        sea: [hex_rgba(0x5fa9c7, 1.0), hex_rgba(0xa8d9df, 1.0)],
        roof: [
            hex_rgba(0xd6a88a, 1.0),
            hex_rgba(0xc98f73, 1.0),
            hex_rgba(0xe6cdb0, 1.0),
            hex_rgba(0xb9a08a, 1.0),
            hex_rgba(0xd9b37c, 1.0),
        ],
        park: hex_rgba(0x8fa86b, 1.0),
        tint: rgba255(255, 226, 190, 0.10),
        win: 0.0,
        sh: [-3.0, 3.0],
        glint: [255, 245, 220],
        ga: 0.5,
        sun: [0.85, 0.15],
        sun_col: [255, 214, 160],
    },
    // 2: Noon
    IstanbulPal {
        land: hex_rgba(0xdfd0ae, 1.0),
        road: hex_rgba(0xb9ad95, 1.0),
        edge: hex_rgba(0xece1c6, 1.0),
        sea: [hex_rgba(0x1c7ea8, 1.0), hex_rgba(0x47b1c9, 1.0)],
        roof: [
            hex_rgba(0xc9694a, 1.0),
            hex_rgba(0xd98a62, 1.0),
            hex_rgba(0xe8d2b0, 1.0),
            hex_rgba(0xb7553d, 1.0),
            hex_rgba(0xd8a05a, 1.0),
        ],
        park: hex_rgba(0x7f9f55, 1.0),
        tint: rgba255(255, 255, 255, 0.0),
        win: 0.0,
        sh: [1.0, 3.0],
        glint: [255, 255, 255],
        ga: 0.28,
        sun: [0.5, 0.0],
        sun_col: [255, 255, 235],
    },
    // 3: Sunset (default)
    IstanbulPal {
        land: hex_rgba(0xc9a98a, 1.0),
        road: hex_rgba(0x9a8068, 1.0),
        edge: hex_rgba(0xdcb88f, 1.0),
        sea: [hex_rgba(0xe0814f, 1.0), hex_rgba(0x18507c, 1.0)],
        roof: [
            hex_rgba(0xb8553f, 1.0),
            hex_rgba(0xc97652, 1.0),
            hex_rgba(0xd9a778, 1.0),
            hex_rgba(0x9c4638, 1.0),
            hex_rgba(0xc58c5a, 1.0),
        ],
        park: hex_rgba(0x6f8a4c, 1.0),
        tint: rgba255(255, 128, 48, 0.17),
        win: 0.28,
        sh: [5.0, 2.0],
        glint: [255, 200, 120],
        ga: 0.9,
        sun: [0.08, 0.3],
        sun_col: [255, 150, 70],
    },
    // 4: Night
    IstanbulPal {
        land: hex_rgba(0x2b3047, 1.0),
        road: hex_rgba(0x1d2135, 1.0),
        edge: hex_rgba(0x3a4160, 1.0),
        sea: [hex_rgba(0x071833, 1.0), hex_rgba(0x13335c, 1.0)],
        roof: [
            hex_rgba(0x3a3f5e, 1.0),
            hex_rgba(0x454b6b, 1.0),
            hex_rgba(0x2f3552, 1.0),
            hex_rgba(0x51446a, 1.0),
            hex_rgba(0x3d4a66, 1.0),
        ],
        park: hex_rgba(0x1d3a3c, 1.0),
        tint: rgba255(6, 14, 52, 0.40),
        win: 1.0,
        sh: [2.0, 3.0],
        glint: [200, 220, 255],
        ga: 0.6,
        sun: [0.7, 0.12],
        sun_col: [150, 180, 255],
    },
];

#[inline]
pub fn pal(t_idx: usize) -> &'static IstanbulPal {
    &PALETTES[t_idx.clamp(1, 4)]
}

#[derive(Clone, Debug)]
pub struct WaterGlint {
    pub x: f64,
    pub y: f64,
    pub l: f64,
    pub p: f64,
}

#[derive(Clone, Debug)]
pub struct IstanbulGeo {
    pub w: f64,
    pub h: f64,
    pub b0: [f64; 2],
    pub b1: [f64; 2],
    pub c0: [f64; 2],
    pub c1: [f64; 2],
    pub e0: [f64; 2],
    pub e1: [f64; 2],
    pub bx: f64,
    pub by0: f64,
    pub by1: f64,
    pub water_poly: Vec<[f64; 2]>,
    pub horn_poly: Vec<[f64; 2]>,
}

#[inline]
pub fn qb(p: [f64; 2], c: [f64; 2], q: [f64; 2], t: f64) -> [f64; 2] {
    let u = 1.0 - t;
    [
        u * u * p[0] + 2.0 * u * t * c[0] + t * t * q[0],
        u * u * p[1] + 2.0 * u * t * c[1] + t * t * q[1],
    ]
}

impl IstanbulGeo {
    pub fn new(w: f64, h: f64) -> Self {
        let w = w.max(64.0);
        let h = h.max(64.0);
        let cy = |x: f64| h * (0.5 + 0.13 * (x / w * 3.2 + 0.5).sin());
        let hw = |x: f64| h * (0.085 + 0.02 * (x / w * 5.0 + 1.0).sin());
        let bank = |x: f64| cy(x) - hw(x);

        let mut water_poly = Vec::with_capacity(130);
        for i in 0..=64 {
            let x = w * (i as f64 / 64.0 * 1.1 - 0.05);
            water_poly.push([x, bank(x)]);
        }
        for i in (0..=64).rev() {
            let x = w * (i as f64 / 64.0 * 1.1 - 0.05);
            water_poly.push([x, cy(x) + hw(x)]);
        }

        let b0 = [w * 0.21, bank(w * 0.21) + 10.0];
        let b1 = [w * 0.33, bank(w * 0.33) + 10.0];
        let c0 = [w * 0.09, h * 0.27];
        let c1 = [w * 0.20, h * 0.36];
        let e0 = [-w * 0.04, h * 0.12];
        let e1 = [-w * 0.04, h * 0.22];

        let mut horn_poly = Vec::with_capacity(130);
        horn_poly.push(b0);
        for i in 1..=64 {
            horn_poly.push(qb(b0, c0, e0, i as f64 / 64.0));
        }
        horn_poly.push(e1);
        for i in 1..=64 {
            horn_poly.push(qb(e1, c1, b1, i as f64 / 64.0));
        }

        let bx = w * 0.86;
        let by0 = bank(bx) - 14.0;
        let by1 = cy(bx) + hw(bx) + 14.0;

        Self {
            w,
            h,
            b0,
            b1,
            c0,
            c1,
            e0,
            e1,
            bx,
            by0,
            by1,
            water_poly,
            horn_poly,
        }
    }

    #[inline]
    pub fn cy(&self, x: f64) -> f64 {
        self.h * (0.5 + 0.13 * (x / self.w * 3.2 + 0.5).sin())
    }

    #[inline]
    pub fn hw(&self, x: f64) -> f64 {
        self.h * (0.085 + 0.02 * (x / self.w * 5.0 + 1.0).sin())
    }

    #[inline]
    pub fn bank(&self, x: f64) -> f64 {
        self.cy(x) - self.hw(x)
    }

    #[inline]
    pub fn in_water(&self, x: f64, y: f64) -> bool {
        point_in_poly(&self.water_poly, x, y) || point_in_poly(&self.horn_poly, x, y)
    }

    #[inline]
    pub fn on_shoreline_stroke(&self, x: f64, y: f64) -> bool {
        min_dist_to_closed_poly(&self.water_poly, x, y) <= 12.0
            || min_dist_to_closed_poly(&self.horn_poly, x, y) <= 12.0
    }
}

#[inline]
fn point_in_poly(poly: &[[f64; 2]], x: f64, y: f64) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        if (a[1] > y) != (b[1] > y) {
            let dy = b[1] - a[1];
            if dy.abs() > 1e-12 {
                let ix = a[0] + (y - a[1]) * (b[0] - a[0]) / dy;
                if x < ix {
                    inside = !inside;
                }
            }
        }
    }
    inside
}

#[inline]
fn seg_dist_f64(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    if len2 <= 1e-12 {
        return (px - ax).hypot(py - ay);
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0);
    let qx = ax + dx * t;
    let qy = ay + dy * t;
    (px - qx).hypot(py - qy)
}

fn min_dist_to_closed_poly(poly: &[[f64; 2]], x: f64, y: f64) -> f64 {
    let mut best = f64::MAX;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let d = seg_dist_f64(x, y, a[0], a[1], b[0], b[1]);
        if d < best {
            best = d;
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Software 2D Canvas Rasterizer (Canvas2D parity for static city + tile pats)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Paint<'a> {
    Solid([f64; 4]),
    Linear {
        p0: [f64; 2],
        p1: [f64; 2],
        stops: &'a [(f64, [f64; 4])],
    },
    Radial {
        c0: [f64; 2],
        r0: f64,
        c1: [f64; 2],
        r1: f64,
        stops: &'a [(f64, [f64; 4])],
    },
}

impl<'a> Paint<'a> {
    #[inline]
    fn at(&self, x: f64, y: f64) -> [f64; 4] {
        match self {
            Paint::Solid(c) => *c,
            Paint::Linear { p0, p1, stops } => {
                let dx = p1[0] - p0[0];
                let dy = p1[1] - p0[1];
                let len2 = dx * dx + dy * dy;
                let t = if len2 <= 1e-12 {
                    0.0
                } else {
                    ((x - p0[0]) * dx + (y - p0[1]) * dy) / len2
                };
                sample_stops(stops, t)
            }
            Paint::Radial {
                c0,
                r0,
                c1,
                r1,
                stops,
            } => {
                let dx = c1[0] - c0[0];
                let dy = c1[1] - c0[1];
                let dr = r1 - r0;
                let qx = x - c0[0];
                let qy = y - c0[1];
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
                        if t1 >= t2 { t1 } else { t2 }
                    }
                };
                sample_stops(stops, t)
            }
        }
    }
}

#[inline]
fn sample_stops(stops: &[(f64, [f64; 4])], t: f64) -> [f64; 4] {
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
            let k = if span.abs() < 1e-12 {
                0.0
            } else {
                (t - t0) / span
            };
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

struct PolyField {
    cov: Vec<f32>,
    edge_dist: Vec<f32>,
}

impl PolyField {
    fn build(poly: &[[f64; 2]], w: i32, h: i32, scale: f64, max_stroke_pad: f64) -> Self {
        let len = (w * h) as usize;
        let mut cov = vec![0.0f32; len];
        let mut edge_dist = vec![f32::MAX; len];

        let pts: Vec<[f64; 2]> = poly.iter().map(|p| [p[0] * scale, p[1] * scale]).collect();
        let n = pts.len();
        if n >= 3 {
            let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for p in &pts {
                minx = minx.min(p[0]);
                maxx = maxx.max(p[0]);
                miny = miny.min(p[1]);
                maxy = maxy.max(p[1]);
            }
            let y0 = (miny.floor() as i32).max(0);
            let y1 = (maxy.ceil() as i32).min(h);
            let x0 = (minx.floor() as i32).max(0);
            let x1 = (maxx.ceil() as i32).min(w);
            if y1 > y0 && x1 > x0 {
                let row_w = (x1 - x0) as usize;
                let mut row_cov = vec![0.0f32; row_w];
                let mut xs = Vec::with_capacity(16);
                for y in y0..y1 {
                    row_cov.fill(0.0);
                    for sub in 0..4 {
                        let yy = y as f64 + (sub as f64 + 0.5) * 0.25;
                        xs.clear();
                        for i in 0..n {
                            let a = pts[i];
                            let b = pts[(i + 1) % n];
                            if (a[1] <= yy) != (b[1] <= yy) {
                                let dy = b[1] - a[1];
                                if dy.abs() > 1e-12 {
                                    xs.push(a[0] + (yy - a[1]) * (b[0] - a[0]) / dy);
                                }
                            }
                        }
                        if xs.len() >= 2 {
                            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                            let mut i = 0;
                            while i + 1 < xs.len() {
                                let xa = xs[i];
                                let xb = xs[i + 1];
                                i += 2;
                                if xb <= xa {
                                    continue;
                                }
                                let pa = (xa.floor() as i32).max(x0);
                                let pb = (xb.floor() as i32).min(x1 - 1);
                                for px in pa..=pb {
                                    let l = xa.max(px as f64);
                                    let r = xb.min(px as f64 + 1.0);
                                    if r > l {
                                        row_cov[(px - x0) as usize] += ((r - l) * 0.25) as f32;
                                    }
                                }
                            }
                        }
                    }
                    let row_off = (y * w + x0) as usize;
                    for xi in 0..row_w {
                        cov[row_off + xi] = row_cov[xi].min(1.0);
                    }
                }
            }

            // Segment-AABB distance buffer for O(W*H) single-pass round-joined strokes
            let pad = max_stroke_pad * scale;
            for i in 0..n {
                let a = pts[i];
                let b = pts[(i + 1) % n];
                let sx0 = ((a[0].min(b[0]) - pad).floor() as i32).max(0);
                let sx1 = ((a[0].max(b[0]) + pad).ceil() as i32).min(w - 1);
                let sy0 = ((a[1].min(b[1]) - pad).floor() as i32).max(0);
                let sy1 = ((a[1].max(b[1]) + pad).ceil() as i32).min(h - 1);
                let dx = b[0] - a[0];
                let dy = b[1] - a[1];
                let len2 = dx * dx + dy * dy;
                for y in sy0..=sy1 {
                    let py = y as f64 + 0.5;
                    let row_off = (y * w) as usize;
                    for x in sx0..=sx1 {
                        let px = x as f64 + 0.5;
                        let d = if len2 <= 1e-12 {
                            (px - a[0]).hypot(py - a[1])
                        } else {
                            let t = (((px - a[0]) * dx + (py - a[1]) * dy) / len2).clamp(0.0, 1.0);
                            let qx = a[0] + dx * t;
                            let qy = a[1] + dy * t;
                            (px - qx).hypot(py - qy)
                        } as f32;
                        let idx = row_off + x as usize;
                        if d < edge_dist[idx] {
                            edge_dist[idx] = d;
                        }
                    }
                }
            }
        }

        Self {
            cov,
            edge_dist,
        }
    }
}

pub struct AtlasCanvas {
    pub w: i32,
    pub h: i32,
    pub scale: f64,
    pub px: Vec<u8>,
}

impl AtlasCanvas {
    pub fn new_rgba(w: i32, h: i32, scale: f64, init: [u8; 4]) -> Self {
        let len = (w * h) as usize;
        let mut px = Vec::with_capacity(len * 4);
        for _ in 0..len {
            px.extend_from_slice(&init);
        }
        Self { w, h, scale, px }
    }

    #[inline]
    fn blend_px(&mut self, x: i32, y: i32, col: [f64; 4], cov: f64) {
        if cov <= 0.0005 || x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let sa = (col[3] * cov).clamp(0.0, 1.0);
        if sa <= 0.0005 {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let da = self.px[i + 3] as f64 / 255.0;
        let oa = sa + da * (1.0 - sa);
        if oa <= 1e-6 {
            return;
        }
        if da <= 1e-6 || sa >= 0.9995 {
            self.px[i] = (col[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            self.px[i + 1] = (col[1].clamp(0.0, 1.0) * 255.0).round() as u8;
            self.px[i + 2] = (col[2].clamp(0.0, 1.0) * 255.0).round() as u8;
            self.px[i + 3] = (oa.clamp(0.0, 1.0) * 255.0).round() as u8;
            return;
        }
        for ch in 0..3 {
            let d = self.px[i + ch] as f64 / 255.0;
            let o = (col[ch] * sa + d * da * (1.0 - sa)) / oa;
            self.px[i + ch] = (o.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        self.px[i + 3] = (oa.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    pub fn fill_solid_bg(&mut self, col: [f64; 4]) {
        let r = (col[0].clamp(0.0, 1.0) * 255.0).round() as u8;
        let g = (col[1].clamp(0.0, 1.0) * 255.0).round() as u8;
        let b = (col[2].clamp(0.0, 1.0) * 255.0).round() as u8;
        let a = (col[3].clamp(0.0, 1.0) * 255.0).round() as u8;
        for chunk in self.px.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = a;
        }
    }

    fn stroke_poly_field(&mut self, pf: &PolyField, width: f64, col: [f64; 4]) {
        let hw = (width * self.scale * 0.5) as f32;
        for y in 0..self.h {
            let row = (y * self.w) as usize;
            for x in 0..self.w {
                let d = pf.edge_dist[row + x as usize];
                if d <= hw + 0.5 {
                    let cov = (hw + 0.5 - d).clamp(0.0, 1.0) as f64;
                    self.blend_px(x, y, col, cov);
                }
            }
        }
    }

    fn fill_poly_field_paint(&mut self, pf: &PolyField, paint: &Paint) {
        let inv_s = 1.0 / self.scale;
        for y in 0..self.h {
            let ly = (y as f64 + 0.5) * inv_s;
            let row = (y * self.w) as usize;
            for x in 0..self.w {
                let cov = pf.cov[row + x as usize] as f64;
                if cov > 0.0005 {
                    let lx = (x as f64 + 0.5) * inv_s;
                    let col = paint.at(lx, ly);
                    self.blend_px(x, y, col, cov);
                }
            }
        }
    }

    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, col: [f64; 4]) {
        let s = self.scale;
        let sx0 = x * s;
        let sy0 = y * s;
        let sx1 = (x + w) * s;
        let sy1 = (y + h) * s;
        let x0 = (sx0.floor() as i32).max(0);
        let x1 = (sx1.ceil() as i32).min(self.w);
        let y0 = (sy0.floor() as i32).max(0);
        let y1 = (sy1.ceil() as i32).min(self.h);
        for py in y0..y1 {
            let cy = (sy1.min(py as f64 + 1.0) - sy0.max(py as f64)).clamp(0.0, 1.0);
            if cy <= 0.0 {
                continue;
            }
            for px in x0..x1 {
                let cx = (sx1.min(px as f64 + 1.0) - sx0.max(px as f64)).clamp(0.0, 1.0);
                if cx > 0.0 {
                    self.blend_px(px, py, col, cx * cy);
                }
            }
        }
    }

    pub fn fill_round_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64, col: [f64; 4]) {
        let s = self.scale;
        let hw = w * s * 0.5;
        let hh = h * s * 0.5;
        let cx = (x + w * 0.5) * s;
        let cy = (y + h * 0.5) * s;
        let rr = (r * s).min(hw).min(hh).max(0.0);
        let x0 = ((cx - hw - 1.0).floor() as i32).max(0);
        let x1 = ((cx + hw + 1.0).ceil() as i32).min(self.w);
        let y0 = ((cy - hh - 1.0).floor() as i32).max(0);
        let y1 = ((cy + hh + 1.0).ceil() as i32).min(self.h);
        let bx = hw - rr;
        let by = hh - rr;
        for py in y0..y1 {
            let dy = (py as f64 + 0.5 - cy).abs() - by;
            for px in x0..x1 {
                let dx = (px as f64 + 0.5 - cx).abs() - bx;
                let sd = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - rr;
                let cov = (0.5 - sd).clamp(0.0, 1.0);
                if cov > 0.0005 {
                    self.blend_px(px, py, col, cov);
                }
            }
        }
    }

    pub fn fill_rotated_rect(
        &mut self,
        cx: f64,
        cy: f64,
        w: f64,
        d: f64,
        angle: f64,
        col: [f64; 4],
    ) {
        self.fill_rotated_subrect(cx, cy, angle, -w * 0.5, -d * 0.5, w, d, col);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_rotated_subrect(
        &mut self,
        origin_x: f64,
        origin_y: f64,
        angle: f64,
        lx0: f64,
        ly0: f64,
        lw: f64,
        lh: f64,
        col: [f64; 4],
    ) {
        let s = self.scale;
        let ox = origin_x * s;
        let oy = origin_y * s;
        let lcx = (lx0 + lw * 0.5) * s;
        let lcy = (ly0 + lh * 0.5) * s;
        let lhw = lw * s * 0.5;
        let lhh = lh * s * 0.5;
        let (sa, ca) = angle.sin_cos();
        let gcx = ox + lcx * ca - lcy * sa;
        let gcy = oy + lcx * sa + lcy * ca;
        let rad = lhw.hypot(lhh) + 1.5;
        let x0 = ((gcx - rad).floor() as i32).max(0);
        let x1 = ((gcx + rad).ceil() as i32).min(self.w);
        let y0 = ((gcy - rad).floor() as i32).max(0);
        let y1 = ((gcy + rad).ceil() as i32).min(self.h);
        for py in y0..y1 {
            let dy = py as f64 + 0.5 - oy;
            for px in x0..x1 {
                let dx = px as f64 + 0.5 - ox;
                let lx = dx * ca + dy * sa;
                let ly = -dx * sa + dy * ca;
                let sd = ((lx - lcx).abs() - lhw).max((ly - lcy).abs() - lhh);
                let cov = (0.5 - sd).clamp(0.0, 1.0);
                if cov > 0.0005 {
                    self.blend_px(px, py, col, cov);
                }
            }
        }
    }

    pub fn fill_circle_paint(&mut self, cx: f64, cy: f64, r: f64, paint: &Paint) {
        let s = self.scale;
        let scx = cx * s;
        let scy = cy * s;
        let sr = r * s;
        let x0 = ((scx - sr - 1.5).floor() as i32).max(0);
        let x1 = ((scx + sr + 1.5).ceil() as i32).min(self.w);
        let y0 = ((scy - sr - 1.5).floor() as i32).max(0);
        let y1 = ((scy + sr + 1.5).ceil() as i32).min(self.h);
        let inv_s = 1.0 / s;
        for py in y0..y1 {
            let dy = py as f64 + 0.5 - scy;
            let ly = (py as f64 + 0.5) * inv_s;
            for px in x0..x1 {
                let dx = px as f64 + 0.5 - scx;
                let d = dx.hypot(dy) - sr;
                let cov = (0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0005 {
                    let lx = (px as f64 + 0.5) * inv_s;
                    let col = paint.at(lx, ly);
                    self.blend_px(px, py, col, cov);
                }
            }
        }
    }

    pub fn fill_circles_union(&mut self, circles: &[(f64, f64, f64)], col: [f64; 4]) {
        if circles.is_empty() {
            return;
        }
        let s = self.scale;
        let mut minx = f64::MAX;
        let mut miny = f64::MAX;
        let mut maxx = f64::MIN;
        let mut maxy = f64::MIN;
        for &(cx, cy, r) in circles {
            minx = minx.min((cx - r) * s);
            maxx = maxx.max((cx + r) * s);
            miny = miny.min((cy - r) * s);
            maxy = maxy.max((cy + r) * s);
        }
        let x0 = ((minx - 1.5).floor() as i32).max(0);
        let x1 = ((maxx + 1.5).ceil() as i32).min(self.w);
        let y0 = ((miny - 1.5).floor() as i32).max(0);
        let y1 = ((maxy + 1.5).ceil() as i32).min(self.h);
        for py in y0..y1 {
            let y = py as f64 + 0.5;
            for px in x0..x1 {
                let x = px as f64 + 0.5;
                let mut best = f64::MAX;
                for &(cx, cy, r) in circles {
                    let d = (x - cx * s).hypot(y - cy * s) - r * s;
                    if d < best {
                        best = d;
                    }
                }
                let cov = (0.5 - best).clamp(0.0, 1.0);
                if cov > 0.0005 {
                    self.blend_px(px, py, col, cov);
                }
            }
        }
    }

    pub fn fill_rotated_ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        rot: f64,
        col: [f64; 4],
    ) {
        let s = self.scale;
        let scx = cx * s;
        let scy = cy * s;
        let srx = (rx * s).max(0.5);
        let sry = (ry * s).max(0.5);
        let (sr, cr) = rot.sin_cos();
        let rad = srx.max(sry) + 2.0;
        let x0 = ((scx - rad).floor() as i32).max(0);
        let x1 = ((scx + rad).ceil() as i32).min(self.w);
        let y0 = ((scy - rad).floor() as i32).max(0);
        let y1 = ((scy + rad).ceil() as i32).min(self.h);
        for py in y0..y1 {
            let dy = py as f64 + 0.5 - scy;
            for px in x0..x1 {
                let dx = px as f64 + 0.5 - scx;
                let u = (dx * cr + dy * sr) / srx;
                let v = (-dx * sr + dy * cr) / sry;
                let d2 = u * u + v * v;
                let d = d2.sqrt();
                let gx = 2.0 * u / srx;
                let gy = 2.0 * v / sry;
                let g = gx.hypot(gy).max(1e-6);
                let sd = (d2 - 1.0) / g;
                let _ = d;
                let cov = (0.5 - sd).clamp(0.0, 1.0);
                if cov > 0.0005 {
                    self.blend_px(px, py, col, cov);
                }
            }
        }
    }

    pub fn draw_line_round(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, width: f64, col: [f64; 4]) {
        let s = self.scale;
        let ax = x0 * s;
        let ay = y0 * s;
        let bx = x1 * s;
        let by = y1 * s;
        let hw = width * s * 0.5;
        let pad = hw + 1.5;
        let px0 = ((ax.min(bx) - pad).floor() as i32).max(0);
        let px1 = ((ax.max(bx) + pad).ceil() as i32).min(self.w);
        let py0 = ((ay.min(by) - pad).floor() as i32).max(0);
        let py1 = ((ay.max(by) + pad).ceil() as i32).min(self.h);
        for py in py0..py1 {
            let y = py as f64 + 0.5;
            for px in px0..px1 {
                let x = px as f64 + 0.5;
                let d = seg_dist_f64(x, y, ax, ay, bx, by) - hw;
                let cov = (0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0005 {
                    self.blend_px(px, py, col, cov);
                }
            }
        }
    }

    pub fn draw_line_butt(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, width: f64, col: [f64; 4]) {
        self.stroke_lines_union_butt(&[(x0, y0, x1, y1)], width, col);
    }

    pub fn stroke_lines_union_butt(
        &mut self,
        lines: &[(f64, f64, f64, f64)],
        width: f64,
        col: [f64; 4],
    ) {
        if lines.is_empty() {
            return;
        }
        let s = self.scale;
        let hw = width * s * 0.5;
        let pad = hw + 1.5;
        struct Seg {
            cx: f64,
            cy: f64,
            ux: f64,
            uy: f64,
            hl: f64,
        }
        let mut segs = Vec::with_capacity(lines.len());
        let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &(x0, y0, x1, y1) in lines {
            let ax = x0 * s;
            let ay = y0 * s;
            let bx = x1 * s;
            let by = y1 * s;
            minx = minx.min(ax).min(bx);
            maxx = maxx.max(ax).max(bx);
            miny = miny.min(ay).min(by);
            maxy = maxy.max(ay).max(by);
            let dx = bx - ax;
            let dy = by - ay;
            let len = dx.hypot(dy);
            if len > 1e-6 {
                segs.push(Seg {
                    cx: (ax + bx) * 0.5,
                    cy: (ay + by) * 0.5,
                    ux: dx / len,
                    uy: dy / len,
                    hl: len * 0.5,
                });
            }
        }
        if segs.is_empty() {
            return;
        }
        let px0 = ((minx - pad).floor() as i32).max(0);
        let px1 = ((maxx + pad).ceil() as i32).min(self.w);
        let py0 = ((miny - pad).floor() as i32).max(0);
        let py1 = ((maxy + pad).ceil() as i32).min(self.h);
        for py in py0..py1 {
            let y = py as f64 + 0.5;
            for px in px0..px1 {
                let x = px as f64 + 0.5;
                let mut best = f64::MAX;
                for seg in &segs {
                    let rx = x - seg.cx;
                    let ry = y - seg.cy;
                    let along = (rx * seg.ux + ry * seg.uy).abs() - seg.hl;
                    let perp = (-rx * seg.uy + ry * seg.ux).abs() - hw;
                    let d = along.max(0.0).hypot(perp.max(0.0)) + along.max(perp).min(0.0);
                    if d < best {
                        best = d;
                    }
                }
                let cov = (0.5 - best).clamp(0.0, 1.0);
                if cov > 0.0005 {
                    self.blend_px(px, py, col, cov);
                }
            }
        }
    }

    pub fn draw_quad_bezier_round(
        &mut self,
        p0: [f64; 2],
        p1: [f64; 2],
        p2: [f64; 2],
        width: f64,
        col: [f64; 4],
    ) {
        let s = self.scale;
        let steps = 16usize;
        let mut pts = [[0.0f64; 2]; 17];
        let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for i in 0..=steps {
            let pt = qb(p0, p1, p2, i as f64 / steps as f64);
            let sp = [pt[0] * s, pt[1] * s];
            pts[i] = sp;
            minx = minx.min(sp[0]);
            maxx = maxx.max(sp[0]);
            miny = miny.min(sp[1]);
            maxy = maxy.max(sp[1]);
        }
        let hw = width * s * 0.5;
        let pad = hw + 1.5;
        let px0 = ((minx - pad).floor() as i32).max(0);
        let px1 = ((maxx + pad).ceil() as i32).min(self.w);
        let py0 = ((miny - pad).floor() as i32).max(0);
        let py1 = ((maxy + pad).ceil() as i32).min(self.h);
        if px1 <= px0 || py1 <= py0 {
            return;
        }
        for py in py0..py1 {
            let y = py as f64 + 0.5;
            for px in px0..px1 {
                let x = px as f64 + 0.5;
                let mut best = f64::MAX;
                for i in 0..steps {
                    let d = seg_dist_f64(x, y, pts[i][0], pts[i][1], pts[i + 1][0], pts[i + 1][1]);
                    if d < best {
                        best = d;
                    }
                }
                let cov = (0.5 - (best - hw)).clamp(0.0, 1.0);
                if cov > 0.0005 {
                    self.blend_px(px, py, col, cov);
                }
            }
        }
    }

    pub fn stroke_closed_polygon(&mut self, poly: &[[f64; 2]], width: f64, col: [f64; 4]) {
        let n = poly.len();
        if n < 2 {
            return;
        }
        let s = self.scale;
        let pts: Vec<[f64; 2]> = poly.iter().map(|p| [p[0] * s, p[1] * s]).collect();
        let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in &pts {
            minx = minx.min(p[0]);
            maxx = maxx.max(p[0]);
            miny = miny.min(p[1]);
            maxy = maxy.max(p[1]);
        }
        let hw = width * s * 0.5;
        let pad = hw + 1.5;
        let px0 = ((minx - pad).floor() as i32).max(0);
        let px1 = ((maxx + pad).ceil() as i32).min(self.w);
        let py0 = ((miny - pad).floor() as i32).max(0);
        let py1 = ((maxy + pad).ceil() as i32).min(self.h);
        for py in py0..py1 {
            let y = py as f64 + 0.5;
            for px in px0..px1 {
                let x = px as f64 + 0.5;
                let mut best = f64::MAX;
                for i in 0..n {
                    let a = pts[i];
                    let b = pts[(i + 1) % n];
                    let d = seg_dist_f64(x, y, a[0], a[1], b[0], b[1]);
                    if d < best {
                        best = d;
                    }
                }
                let cov = (0.5 - (best - hw)).clamp(0.0, 1.0);
                if cov > 0.0005 {
                    self.blend_px(px, py, col, cov);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Landmark Painters (`IST.dome`, `IST.minaret`, `IST.hagia`, `IST.blue`,
// `IST.galata`, `IST.maiden`, `IST.hornBridge` in `prototype/istanbul-city.js`)
// ---------------------------------------------------------------------------

fn draw_dome(c: &mut AtlasCanvas, x: f64, y: f64, r: f64, lite: [f64; 4], dark: [f64; 4]) {
    let stops = [(0.0, lite), (1.0, dark)];
    let paint = Paint::Radial {
        c0: [x - r * 0.35, y - r * 0.35],
        r0: r * 0.1,
        c1: [x, y],
        r1: r,
        stops: &stops,
    };
    c.fill_circle_paint(x, y, r, &paint);
    let mut ribs = [(0.0f64, 0.0f64, 0.0f64, 0.0f64); 12];
    for (i, slot) in ribs.iter_mut().enumerate() {
        let a = i as f64 * PI / 6.0;
        *slot = (x, y, x + a.cos() * r, y + a.sin() * r);
    }
    c.stroke_lines_union_butt(&ribs, 0.8, [0.0, 0.0, 0.0, 0.14]);
}

fn draw_minaret(c: &mut AtlasCanvas, x: f64, y: f64, s: f64, sh: [f64; 2]) {
    let hypot = sh[0].hypot(sh[1]);
    let n = if hypot == 0.0 { 1.0 } else { hypot };
    c.draw_line_round(
        x,
        y,
        x + sh[0] / n * s * 2.4,
        y + sh[1] / n * s * 2.4,
        s * 0.2,
        [0.0, 0.0, 0.0, 0.26],
    );
    c.fill_circle_paint(x, y, s * 0.3, &Paint::Solid(hex_rgba(0xefe6d6, 1.0)));
    c.fill_circle_paint(x, y, s * 0.14, &Paint::Solid(hex_rgba(0x9aa3ab, 1.0)));
}

fn draw_hagia(c: &mut AtlasCanvas, x: f64, y: f64, s: f64, p: &IstanbulPal, col: [f64; 4]) {
    c.fill_rect(
        x - s * 1.7 + p.sh[0] * 1.5,
        y - s * 1.4 + p.sh[1] * 1.5,
        s * 3.4,
        s * 2.8,
        [0.0, 0.0, 0.0, 0.25],
    );
    c.fill_rect(x - s * 1.7, y - s * 1.4, s * 3.4, s * 2.8, col);
    c.fill_rect(
        x - s * 1.7,
        y - s * 1.4,
        s * 3.4,
        s * 0.5,
        [1.0, 1.0, 1.0, 0.2],
    );
    for k in [-1.0, 1.0] {
        draw_dome(
            c,
            x + k * s * 1.15,
            y,
            s * 0.8,
            hex_rgba(0xb4bcc2, 1.0),
            hex_rgba(0x7e8993, 1.0),
        );
    }
    draw_dome(
        c,
        x,
        y,
        s * 1.05,
        hex_rgba(0xc8cfd3, 1.0),
        hex_rgba(0x7f8b95, 1.0),
    );
    for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        draw_minaret(c, x + a * s * 2.0, y + b * s * 1.6, s * 0.7, p.sh);
    }
}

fn draw_blue_mosque(c: &mut AtlasCanvas, x: f64, y: f64, s: f64, p: &IstanbulPal) {
    c.fill_rect(
        x - s * 1.5 + 3.0,
        y - s * 1.5 + 4.0,
        s * 3.0,
        s * 3.0,
        [0.0, 0.0, 0.0, 0.25],
    );
    c.fill_rect(
        x - s * 1.5,
        y - s * 1.5,
        s * 3.0,
        s * 3.0,
        hex_rgba(0xb9b2a4, 1.0),
    );
    c.fill_rect(
        x - s * 3.6,
        y - s * 1.2,
        s * 2.1,
        s * 2.4,
        hex_rgba(0xcdbf9e, 1.0),
    );
    let dome_l = hex_rgba(0xaeb9c4, 1.0);
    let dome_d = hex_rgba(0x6f7f90, 1.0);
    for i in 0..3 {
        let dx = x - s * 3.1 + i as f64 * s * 0.72;
        draw_dome(c, dx, y - s * 0.75, s * 0.3, dome_l, dome_d);
        draw_dome(c, dx, y + s * 0.75, s * 0.3, dome_l, dome_d);
    }
    for (a, b) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        draw_dome(c, x + a * s * 1.05, y + b * s * 1.05, s * 0.6, dome_l, dome_d);
    }
    for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        draw_dome(c, x + a * s * 1.1, y + b * s * 1.1, s * 0.4, dome_l, dome_d);
    }
    draw_dome(
        c,
        x,
        y,
        s * 0.95,
        hex_rgba(0xbfcad4, 1.0),
        hex_rgba(0x6b7c8f, 1.0),
    );
    for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        draw_minaret(c, x + a * s * 1.9, y + b * s * 1.9, s * 0.7, p.sh);
    }
    draw_minaret(c, x - s * 3.6, y - s * 1.2, s * 0.7, p.sh);
    draw_minaret(c, x - s * 3.6, y + s * 1.2, s * 0.7, p.sh);
}

fn draw_galata(c: &mut AtlasCanvas, x: f64, y: f64, s: f64, p: &IstanbulPal) {
    let hypot = p.sh[0].hypot(p.sh[1]);
    let n = if hypot == 0.0 { 1.0 } else { hypot };
    c.draw_line_round(
        x,
        y,
        x + p.sh[0] / n * s * 2.6,
        y + p.sh[1] / n * s * 2.6,
        s * 0.55,
        [0.0, 0.0, 0.0, 0.28],
    );
    c.fill_circle_paint(x, y, s * 0.95, &Paint::Solid(hex_rgba(0xd8c7a8, 1.0)));
    let stops = [
        (0.0, hex_rgba(0xc48c63, 1.0)),
        (0.5, hex_rgba(0xa06a45, 1.0)),
        (1.0, hex_rgba(0x5e3a24, 1.0)),
    ];
    let grad = Paint::Linear {
        p0: [x - s, y - s],
        p1: [x + s, y + s],
        stops: &stops,
    };
    c.fill_circle_paint(x, y, s * 0.64, &grad);
    c.fill_circle_paint(x, y, s * 0.08, &Paint::Solid(hex_rgba(0xe8dcc0, 1.0)));
}

fn draw_maiden(c: &mut AtlasCanvas, x: f64, y: f64, s: f64) {
    c.fill_rotated_ellipse(x, y, s * 1.5, s * 1.0, 0.0, [1.0, 1.0, 1.0, 0.35]);
    c.fill_rotated_ellipse(x, y, s * 1.15, s * 0.72, 0.3, hex_rgba(0x8f8678, 1.0));
    c.fill_circle_paint(x, y, s * 0.38, &Paint::Solid(hex_rgba(0xefe9de, 1.0)));
    c.fill_circle_paint(x, y, s * 0.24, &Paint::Solid(hex_rgba(0xc4553a, 1.0)));
}

fn draw_horn_bridge(c: &mut AtlasCanvas, g: &IstanbulGeo, p: &IstanbulPal) {
    let a = qb(g.b0, g.c0, g.e0, 0.2);
    let b = qb(g.b1, g.c1, g.e1, 0.2);
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let l = dx.hypot(dy);
    if l <= 1e-6 {
        return;
    }
    let ux = dx / l;
    let uy = dy / l;
    c.draw_line_butt(
        a[0] - ux * 10.0,
        a[1] - uy * 10.0,
        b[0] + ux * 10.0,
        b[1] + uy * 10.0,
        8.0,
        hex_rgba(0x7d838c, 1.0),
    );
    let mut d = 4.0;
    while d < l {
        let px = a[0] + ux * d;
        let py = a[1] + uy * d;
        let side = if (d % 20.0) < 10.0 { 1.0 } else { -1.0 };
        c.fill_circle_paint(
            px,
            py,
            1.2,
            &Paint::Solid(rgba255(255, 240, 190, 0.5 + p.win * 0.5)),
        );
        let ax = px - uy * 6.5 * side;
        let ay = py + ux * 6.5 * side;
        c.fill_circle_paint(ax, ay, 1.4, &Paint::Solid(hex_rgba(0x2b2b2b, 1.0)));
        c.draw_line_butt(
            ax,
            ay,
            px - uy * 15.0 * side,
            py + ux * 15.0 * side,
            0.6,
            [1.0, 1.0, 1.0, 0.55],
        );
        d += 10.0;
    }
}

// ---------------------------------------------------------------------------
// Public Bakers: Static City + Water Mask + Glints + Patterns + Glow Sprites
// ---------------------------------------------------------------------------

pub struct StaticCityOutput {
    pub w: i32,
    pub h: i32,
    pub static_rgba: Vec<u8>,
    pub water_mask_r8: Vec<u8>,
    pub glints: Vec<WaterGlint>,
}

pub fn build_static_city_and_mask(
    geo: &IstanbulGeo,
    t_idx: usize,
    landmarks: bool,
) -> StaticCityOutput {
    let w_css = geo.w;
    let h_css = geo.h;
    let cw = w_css.round().max(64.0) as i32;
    let ch = h_css.round().max(64.0) as i32;
    let p = pal(t_idx);

    let mut c = AtlasCanvas::new_rgba(cw, ch, 1.0, [0, 0, 0, 255]);
    let cs = (w_css.min(h_css) / 38.0).max(15.0);
    let s = w_css.min(h_css) * 0.042;

    // Precompute exact PolyFields for water and horn
    let water_pf = PolyField::build(&geo.water_poly, cw, ch, 1.0, 15.0);
    let horn_pf = PolyField::build(&geo.horn_poly, cw, ch, 1.0, 15.0);

    // 1. Land background
    c.fill_solid_bg(p.land);

    // 2. Coastal road (26px) + shoreline edge (14px)
    for (width, col) in [(26.0, p.road), (14.0, p.edge)] {
        c.stroke_poly_field(&water_pf, width, col);
        c.stroke_poly_field(&horn_pf, width, col);
    }

    // 3. Sea linear gradient fill
    let sea_stops = [(0.0, p.sea[0]), (1.0, p.sea[1])];
    let sea_paint = Paint::Linear {
        p0: [0.0, 0.0],
        p1: [w_css, h_css * 0.3],
        stops: &sea_stops,
    };
    c.fill_poly_field_paint(&water_pf, &sea_paint);
    c.fill_poly_field_paint(&horn_pf, &sea_paint);

    // 4. Deterministic city grid (`Mulberry32(1453)`)
    let mut rng = Mulberry32::new(1453);
    let mut y = 0.0;
    while y < h_css {
        let mut x = 0.0;
        while x < w_css {
            let px = x + cs * 0.5;
            let py = y + cs * 0.5;
            if geo.in_water(px, py) || geo.on_shoreline_stroke(px, py) {
                x += cs;
                continue;
            }
            let north = py < geo.cy(px);
            let r = rng.next_f64();
            let a = if north {
                0.32 + 0.15 * (px / w_css * 4.0).sin()
            } else {
                -0.22 + 0.1 * (py / h_css * 5.0).cos()
            };
            if r < if north { 0.06 } else { 0.2 } {
                c.fill_circle_paint(px, py, cs * 0.62, &Paint::Solid(p.park));
                let mut trees = [(0.0f64, 0.0f64, 0.0f64); 3];
                for tree in &mut trees {
                    let ox = (rng.next_f64() - 0.5) * cs * 0.7;
                    let oy = (rng.next_f64() - 0.5) * cs * 0.7;
                    *tree = (px + ox, py + oy, cs * 0.2);
                }
                c.fill_circles_union(&trees, rgba255(15, 55, 30, 0.55));
            } else if r < 0.985 {
                let bw = cs * (0.62 + rng.next_f64() * 0.3);
                let bd = cs * (0.5 + rng.next_f64() * 0.38);
                let roof_idx = ((rng.next_f64() * 5.0).floor() as usize).min(4);
                c.fill_rotated_rect(
                    px + p.sh[0],
                    py + p.sh[1],
                    bw,
                    bd,
                    a,
                    [0.0, 0.0, 0.0, 0.22],
                );
                c.fill_rotated_rect(px, py, bw, bd, a, p.roof[roof_idx]);
                c.fill_rotated_subrect(
                    px,
                    py,
                    a,
                    -bw * 0.5,
                    -bd * 0.5,
                    bw,
                    bd * 0.5,
                    [1.0, 1.0, 1.0, 0.16],
                );
                c.fill_rotated_subrect(
                    px,
                    py,
                    a,
                    -bw * 0.5,
                    -0.5,
                    bw,
                    1.0,
                    [0.0, 0.0, 0.0, 0.18],
                );
                if p.win != 0.0 && rng.next_f64() < 0.7 {
                    for _ in 0..3 {
                        let wa = ((0.35 + 0.5 * rng.next_f64()) * p.win * 100.0).round() / 100.0;
                        let wx = (rng.next_f64() - 0.5) * bw;
                        let wy = (rng.next_f64() - 0.5) * bd;
                        c.fill_rotated_subrect(
                            px,
                            py,
                            a,
                            wx,
                            wy,
                            1.8,
                            1.8,
                            rgba255(255, 205, 110, wa),
                        );
                    }
                }
            } else {
                draw_dome(
                    &mut c,
                    px,
                    py,
                    cs * 0.3,
                    hex_rgba(0xb6bfc6, 1.0),
                    hex_rgba(0x76838f, 1.0),
                );
                draw_minaret(&mut c, px + cs * 0.38, py - cs * 0.28, cs * 0.22, p.sh);
            }
            x += cs;
        }
        y += cs;
    }

    // 5. Emirgan Tulip Garden
    let tc = [
        hex_rgba(0xd8322d, 1.0),
        hex_rgba(0xf2c230, 1.0),
        hex_rgba(0xc43a8f, 1.0),
        hex_rgba(0xf6f1e6, 1.0),
        hex_rgba(0xf08a2c, 1.0),
    ];
    let fx = w_css * 0.52;
    let fy = geo.cy(w_css * 0.6) + geo.hw(w_css * 0.6) + h_css * 0.05;
    let fw = w_css * 0.16;
    let fh = h_css * 0.1;
    c.fill_round_rect(
        fx - 6.0,
        fy - 6.0,
        fw + 12.0,
        fh + 12.0,
        8.0,
        hex_rgba(0x5d8a46, 1.0),
    );
    let mut row_idx = 0usize;
    let mut ty = fy;
    let mut row_circles = Vec::with_capacity(64);
    while ty < fy + fh {
        row_circles.clear();
        let mut tx = fx;
        while tx < fx + fw {
            row_circles.push((tx, ty, 2.2));
            tx += 6.0;
        }
        c.fill_circles_union(&row_circles, tc[row_idx % 5]);
        ty += 7.0;
        row_idx += 1;
    }

    // 6. Historic Landmarks (Süleymaniye, Hagia Sophia, Blue Mosque, Galata Tower)
    if landmarks {
        draw_hagia(
            &mut c,
            w_css * 0.47,
            geo.bank(w_css * 0.47) - h_css * 0.2,
            s * 0.8,
            p,
            hex_rgba(0xb4a690, 1.0),
        );
        draw_hagia(
            &mut c,
            w_css * 0.62,
            geo.bank(w_css * 0.62) - h_css * 0.14,
            s * 1.05,
            p,
            hex_rgba(0xc98b6d, 1.0),
        );
        draw_blue_mosque(
            &mut c,
            w_css * 0.76,
            geo.bank(w_css * 0.76) - h_css * 0.15,
            s * 0.95,
            p,
        );
        draw_galata(
            &mut c,
            w_css * 0.37,
            geo.bank(w_css * 0.37) - h_css * 0.075,
            s * 0.9,
            p,
        );
    }

    // 7. Galata Bridge + Anglers
    draw_horn_bridge(&mut c, geo, p);

    // 8. Bosphorus Bridge
    let bx = geo.bx;
    let y0 = geo.by0;
    let y1 = geo.by1;
    c.draw_line_butt(
        bx + p.sh[0] * 2.0,
        y0 + p.sh[1] * 2.0,
        bx + p.sh[0] * 2.0,
        y1 + p.sh[1] * 2.0,
        9.0,
        [0.0, 0.0, 0.0, 0.2],
    );
    c.draw_line_butt(bx, y0, bx, y1, 9.0, hex_rgba(0x8b9199, 1.0));
    let mut dy = y0;
    while dy < y1 {
        let seg_h = (y1 - dy).min(1.0);
        c.fill_rect(bx - 7.5, dy, 15.0, seg_h, [1.0, 1.0, 1.0, 0.6]);
        dy += 6.0;
    }
    c.stroke_lines_union_butt(
        &[(bx - 7.0, y0, bx - 7.0, y1), (bx + 7.0, y0, bx + 7.0, y1)],
        1.2,
        rgba255(235, 238, 242, 0.85),
    );
    for f in [0.2, 0.8] {
        let py = y0 + (y1 - y0) * f;
        c.fill_rect(bx - 6.0, py - 6.0, 12.0, 12.0, hex_rgba(0xdfe2e6, 1.0));
        c.fill_rect(bx - 6.0, py - 6.0, 12.0, 3.0, hex_rgba(0x9ea5ad, 1.0));
    }

    // 9. Maiden's Tower
    if landmarks {
        draw_maiden(
            &mut c,
            w_css * 0.52,
            geo.cy(w_css * 0.52) + h_css * 0.025,
            s,
        );
    }

    // 10. Water mask R8 (union of water and horn scanline coverages)
    let mut water_mask_r8 = vec![0u8; (cw * ch) as usize];
    for i in 0..water_mask_r8.len() {
        let cov = water_pf.cov[i].max(horn_pf.cov[i]).clamp(0.0, 1.0);
        water_mask_r8[i] = (cov * 255.0).round() as u8;
    }

    // 11. Glints (`Mulberry32(99)` after consuming 46*5 = 230 wave arc calls)
    let glints = build_glints(geo, t_idx);

    StaticCityOutput {
        w: cw,
        h: ch,
        static_rgba: c.px,
        water_mask_r8,
        glints,
    }
}

pub fn build_glints(geo: &IstanbulGeo, t_idx: usize) -> Vec<WaterGlint> {
    let p = pal(t_idx);
    let mut rng = Mulberry32::new(99);
    for _ in 0..(46 * 5) {
        rng.next_f64();
    }
    let mut glints = Vec::with_capacity(90);
    let gx = geo.w * (p.sun[0] + 0.12).min(0.9);
    for _ in 0..500 {
        if glints.len() >= 90 {
            break;
        }
        let x = gx + (rng.next_f64() - 0.5) * geo.w * 0.28;
        let y = rng.next_f64() * geo.h;
        if geo.in_water(x, y) {
            glints.push(WaterGlint {
                x,
                y,
                l: 4.0 + rng.next_f64() * 9.0,
                p: rng.next_f64() * 6.3,
            });
        }
    }
    glints
}

/// Bakes `512x512` RGBA8 seamless scrolling wave pattern (`IST.prep`, `Mulberry32(99)`).
/// Transparent texels store `P.glint` RGB with `alpha = 0` so `GL_LINEAR` produces zero dark halos.
pub fn build_wave_pattern(t_idx: usize) -> (i32, i32, Vec<u8>) {
    let p = pal(t_idx);
    let mut rng = Mulberry32::new(99);
    let mut c = AtlasCanvas::new_rgba(512, 512, 2.0, [p.glint[0], p.glint[1], p.glint[2], 0]);
    for _ in 0..46 {
        let x = rng.next_f64() * 256.0;
        let y = rng.next_f64() * 256.0;
        let l = 8.0 + rng.next_f64() * 18.0;
        let alpha = ((0.1 + rng.next_f64() * 0.16) * 100.0).round() / 100.0;
        let lw = 1.0 + rng.next_f64() * 1.4;
        let col = rgba255(p.glint[0], p.glint[1], p.glint[2], alpha);
        let dys: &[f64] = if y < 4.0 { &[0.0, -256.0, 256.0] } else { &[0.0, -256.0] };
        for &dx in &[0.0, -256.0] {
            for &dy in dys {
                c.draw_quad_bezier_round(
                    [x + dx, y + dy],
                    [x + dx + l * 0.5, y + dy - 2.5],
                    [x + dx + l, y + dy],
                    lw,
                    col,
                );
            }
        }
    }
    (512, 512, c.px)
}

/// Bakes `28x28` RGBA8 Iznik tile pattern (`14x14` CSS px at 2x).
pub fn build_tile_pattern() -> (i32, i32, Vec<u8>) {
    let mut c = AtlasCanvas::new_rgba(28, 28, 2.0, [29, 79, 147, 255]);
    c.fill_solid_bg(hex_rgba(0x1d4f93, 1.0));
    let turq = hex_rgba(0x2aa6b8, 1.0);
    c.fill_rect(0.0, 0.0, 7.0, 7.0, turq);
    c.fill_rect(7.0, 7.0, 7.0, 7.0, turq);
    let ivory = Paint::Solid(hex_rgba(0xefe5d0, 1.0));
    c.fill_circle_paint(3.5, 10.5, 1.6, &ivory);
    c.fill_circle_paint(10.5, 3.5, 1.6, &ivory);
    let coral = Paint::Solid(hex_rgba(0xe2674a, 1.0));
    c.fill_circle_paint(3.5, 3.5, 1.0, &coral);
    c.fill_circle_paint(10.5, 10.5, 1.0, &coral);
    (28, 28, c.px)
}

/// Bakes `48x48` RGBA8 Seljuk marble star pattern (`24x24` CSS px at 2x).
pub fn build_marble_pattern() -> (i32, i32, Vec<u8>) {
    let mut c = AtlasCanvas::new_rgba(48, 48, 2.0, [239, 229, 208, 255]);
    c.fill_solid_bg(hex_rgba(0xefe5d0, 1.0));
    let mut star = Vec::with_capacity(16);
    for i in 0..16 {
        let a = i as f64 * PI / 8.0;
        let r = if i % 2 != 0 { 4.0 } else { 8.5 };
        star.push([12.0 + a.cos() * r, 12.0 + a.sin() * r]);
    }
    c.stroke_closed_polygon(&star, 0.8, rgba255(150, 104, 64, 0.28));
    (48, 48, c.px)
}

/// Bakes `28x14` RGBA8 wooden pier plank pattern (`14x7` CSS px at 2x).
pub fn build_plank_pattern() -> (i32, i32, Vec<u8>) {
    let mut c = AtlasCanvas::new_rgba(28, 14, 2.0, [140, 91, 54, 255]);
    c.fill_solid_bg(hex_rgba(0x8c5b36, 1.0));
    c.fill_rect(0.0, 6.0, 14.0, 1.0, hex_rgba(0x7a4c2c, 1.0));
    c.fill_rect(0.0, 0.0, 14.0, 2.0, rgba255(255, 220, 170, 0.10));
    c.fill_rect(6.0, 0.0, 1.0, 6.0, rgba255(0, 0, 0, 0.18));
    (28, 14, c.px)
}

/// Generates an RGBA8 radial glow sprite with a strict 1px transparent outer border (`alpha == 0`).
pub fn build_radial_glow_rgba(size: usize, rgb: [u8; 3]) -> Vec<u8> {
    let mut buf = vec![0u8; size * size * 4];
    let center = (size as f64 - 1.0) * 0.5;
    let max_r = (size as f64 * 0.5 - 1.5).max(1.0);
    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            buf[idx] = rgb[0];
            buf[idx + 1] = rgb[1];
            buf[idx + 2] = rgb[2];
            if x == 0 || y == 0 || x + 1 == size || y + 1 == size {
                buf[idx + 3] = 0;
                continue;
            }
            let dist = ((x as f64 - center).hypot(y as f64 - center) / max_r).clamp(0.0, 1.0);
            let alpha = 1.0 - dist;
            buf[idx + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    buf
}
