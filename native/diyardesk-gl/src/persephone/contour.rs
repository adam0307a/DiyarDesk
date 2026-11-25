//! Pure-Rust Canny-style contour extraction & radial segment slicer matching the
//! exact `code.jpg` choreography (`SEG_LEN = 140`, `TRACE_START = 0.25`,
//! `TRACE_SPREAD = 6.0`, `SEG_MIN_DUR = 2.0`, `SEG_MAX_DUR = 3.2`, `TWINKLES = 45`).

use std::f32::consts::PI;

pub const TRACE_START: f32 = 0.25;
pub const TRACE_SPREAD: f32 = 6.0;
pub const SEG_LEN: usize = 140;
pub const SEG_MIN_DUR: f32 = 2.0;
pub const SEG_MAX_DUR: f32 = 3.2;
pub const TWINKLE_COUNT: usize = 45;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TraceVert {
    /// Portrait normalized coordinates in [0, 1] x [0, 1] (top-left origin)
    pub u: f32,
    pub v: f32,
    /// Perpendicular unit normal (portrait normalized space) * side (-1 or +1)
    pub nx: f32,
    pub ny: f32,
    /// Side in [-1, 1] for crisp anti-aliased edge falloff
    pub side: f32,
    /// Exact timestamp (seconds) when this vertex illuminates
    pub t_reveal: f32,
    /// Edge strength / luminance weight in [0.35, 1.0]
    pub strength: f32,
}

#[derive(Clone, Debug)]
pub struct TraceSegment {
    /// Normalized [0, 1] points along the contour segment
    pub pts: Vec<[f32; 2]>,
    pub t_start: f32,
    pub t_dur: f32,
    pub strength: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct TwinkleAnchor {
    pub u: f32,
    pub v: f32,
    pub phase: f32,
    pub speed: f32,
    pub size: f32,
    pub tint_idx: u8,
}

pub struct ExtractedArtwork {
    pub verts: Vec<TraceVert>,
    pub segments: Vec<TraceSegment>,
    pub twinkles: Vec<TwinkleAnchor>,
    pub focus_uv: [f32; 2],
}

#[derive(Clone, Copy)]
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed.wrapping_add(0x9E3779B9))
    }
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }
    fn f32(&mut self) -> f32 {
        (self.next_u32() as f32) / 4294967296.0
    }
}

/// Extracts thin engraving contour polylines from an RGBA portrait image, chops them
/// into `SEG_LEN`-point segments ordered radially from the focus point, and builds
/// both the GPU line-ribbon mesh and spark-tip segment metadata.
pub fn extract_contours_from_rgba(
    rgba: &[u8],
    w: usize,
    h: usize,
    focus_uv: [f32; 2],
    seed: u32,
) -> ExtractedArtwork {
    let n = w * h;
    let mut luma = vec![0.0f32; n];
    for i in 0..n {
        let r = rgba[i * 4] as f32 / 255.0;
        let g = rgba[i * 4 + 1] as f32 / 255.0;
        let b = rgba[i * 4 + 2] as f32 / 255.0;
        luma[i] = 0.299 * r + 0.587 * g + 0.114 * b;
    }

    // 1. Mild 3x3 Gaussian blur to suppress JPEG/compression speckle while preserving fine linework
    let mut blur = vec![0.0f32; n];
    for y in 1..h.saturating_sub(1) {
        let row = y * w;
        for x in 1..w.saturating_sub(1) {
            let s = luma[row - w + x - 1]
                + 2.0 * luma[row - w + x]
                + luma[row - w + x + 1]
                + 2.0 * luma[row + x - 1]
                + 4.0 * luma[row + x]
                + 2.0 * luma[row + x + 1]
                + luma[row + w + x - 1]
                + 2.0 * luma[row + w + x]
                + luma[row + w + x + 1];
            blur[row + x] = s * (1.0 / 16.0);
        }
    }

    // 2. Sobel gradient magnitude & quantized orientation (0, 45, 90, 135 deg)
    let mut mag = vec![0.0f32; n];
    let mut dir = vec![0u8; n];
    let mut max_mag = 0.001f32;

    for y in 2..h.saturating_sub(2) {
        let row = y * w;
        for x in 2..w.saturating_sub(2) {
            let tl = blur[row - w + x - 1];
            let tc = blur[row - w + x];
            let tr = blur[row - w + x + 1];
            let ml = blur[row + x - 1];
            let mr = blur[row + x + 1];
            let bl = blur[row + w + x - 1];
            let bc = blur[row + w + x];
            let br = blur[row + w + x + 1];

            let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
            let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;
            let m = (gx * gx + gy * gy).sqrt();
            mag[row + x] = m;
            if m > max_mag {
                max_mag = m;
            }
            let mut ang = gy.atan2(gx) * (180.0 / PI);
            if ang < 0.0 {
                ang += 180.0;
            }
            dir[row + x] = if !(22.5..157.5).contains(&ang) {
                0
            } else if ang < 67.5 {
                1
            } else if ang < 112.5 {
                2
            } else {
                3
            };
        }
    }

    // 3. Non-maximum suppression + hysteresis thresholding (Canny-style crisp 1px contours!)
    let hi_thr = (max_mag * 0.145).max(0.16);
    let lo_thr = hi_thr * 0.42;
    let mut nms = vec![0.0f32; n];
    let mut state = vec![0u8; n]; // 0=none, 1=weak, 2=strong
    let mut stack: Vec<usize> = Vec::with_capacity(n / 12);

    for y in 2..h.saturating_sub(2) {
        let row = y * w;
        for x in 2..w.saturating_sub(2) {
            let idx = row + x;
            let m = mag[idx];
            if m < lo_thr {
                continue;
            }
            // Skip pure dark background noise
            if blur[idx] < 0.055 && m < hi_thr * 1.15 {
                continue;
            }
            let (m1, m2) = match dir[idx] {
                0 => (mag[idx - 1], mag[idx + 1]),
                1 => (mag[idx - w - 1], mag[idx + w + 1]),
                2 => (mag[idx - w], mag[idx + w]),
                _ => (mag[idx - w + 1], mag[idx + w - 1]),
            };
            if m >= m1 && m >= m2 {
                nms[idx] = m;
                if m >= hi_thr {
                    state[idx] = 2;
                    stack.push(idx);
                } else {
                    state[idx] = 1;
                }
            }
        }
    }

    // Promote connected weak edges to strong
    while let Some(idx) = stack.pop() {
        let x = idx % w;
        let y = idx / w;
        if x <= 1 || x + 1 >= w || y <= 1 || y + 1 >= h {
            continue;
        }
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let ni = ((y as i32 + dy) as usize) * w + (x as i32 + dx) as usize;
                if state[ni] == 1 {
                    state[ni] = 2;
                    stack.push(ni);
                }
            }
        }
    }

    // 4. Trace connected edge pixels into continuous smooth polylines
    let mut visited = vec![false; n];
    let mut raw_chains: Vec<Vec<(usize, usize, f32)>> = Vec::with_capacity(1200);
    const OFF8: [(i32, i32); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];

    for y in 2..h.saturating_sub(2) {
        for x in 2..w.saturating_sub(2) {
            let start = y * w + x;
            if state[start] != 2 || visited[start] {
                continue;
            }
            let mut chain = Vec::with_capacity(64);
            let mut cx = x as i32;
            let mut cy = y as i32;
            let mut prev_dx = 0.0f32;
            let mut prev_dy = 0.0f32;
            visited[start] = true;
            chain.push((x, y, (nms[start] / max_mag).clamp(0.35, 1.0)));

            loop {
                let mut best: Option<(i32, i32, f32)> = None;
                for &(dx, dy) in &OFF8 {
                    let nx = cx + dx;
                    let ny = cy + dy;
                    if nx < 2 || ny < 2 || nx >= (w as i32 - 2) || ny >= (h as i32 - 2) {
                        continue;
                    }
                    let ni = (ny as usize) * w + (nx as usize);
                    if state[ni] != 2 || visited[ni] {
                        continue;
                    }
                    // Prefer smooth continuation direction + strong edge magnitude
                    let len = ((dx * dx + dy * dy) as f32).sqrt();
                    let align = if prev_dx != 0.0 || prev_dy != 0.0 {
                        (dx as f32 / len) * prev_dx + (dy as f32 / len) * prev_dy
                    } else {
                        0.0
                    };
                    let score = align * 1.4 + (nms[ni] / max_mag) * 0.8;
                    if best.is_none_or(|b| score > b.2) {
                        best = Some((nx, ny, score));
                    }
                }
                if let Some((nx, ny, _)) = best {
                    let dx = (nx - cx) as f32;
                    let dy = (ny - cy) as f32;
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    prev_dx = dx / len;
                    prev_dy = dy / len;
                    cx = nx;
                    cy = ny;
                    let ni = (ny as usize) * w + (nx as usize);
                    visited[ni] = true;
                    chain.push((nx as usize, ny as usize, (nms[ni] / max_mag).clamp(0.35, 1.0)));
                } else {
                    break;
                }
            }

            if chain.len() >= 6 {
                raw_chains.push(chain);
            }
        }
    }

    // 5. Chop chains into SEG_LEN segments, smooth slightly, and order radially from focus_uv!
    let mut rng = Rng::new(seed);
    let inv_w = 1.0 / (w.max(1) as f32);
    let inv_h = 1.0 / (h.max(1) as f32);
    let aspect = (w as f32) / (h.max(1) as f32);

    let mut seg_candidates: Vec<(f32, Vec<[f32; 2]>, f32)> = Vec::with_capacity(raw_chains.len() * 2);
    let mut max_dist = 0.001f32;

    for chain in &raw_chains {
        // Subsample every 2nd pixel + keep endpoints for crisp smooth vector curves
        let mut pts: Vec<[f32; 2]> = Vec::with_capacity(chain.len() / 2 + 2);
        let mut str_sum = 0.0f32;
        for (i, &(px, py, st)) in chain.iter().enumerate() {
            if i == 0 || i + 1 == chain.len() || i % 2 == 0 {
                pts.push([(px as f32 + 0.5) * inv_w, (py as f32 + 0.5) * inv_h]);
                str_sum += st;
            }
        }
        if pts.len() < 4 {
            continue;
        }
        // Light 1-pass Laplacian smoothing on interior points to eliminate pixel stair-stepping
        if pts.len() >= 5 {
            let orig = pts.clone();
            for i in 1..(pts.len() - 1) {
                pts[i][0] = orig[i - 1][0] * 0.22 + orig[i][0] * 0.56 + orig[i + 1][0] * 0.22;
                pts[i][1] = orig[i - 1][1] * 0.22 + orig[i][1] * 0.56 + orig[i + 1][1] * 0.22;
            }
        }
        let avg_str = (str_sum / pts.len() as f32).clamp(0.40, 1.0);

        // Chop into chunks of up to SEG_LEN / 2 subsampled points (~140 px)
        let chunk_pts = (SEG_LEN / 2).max(16);
        let mut idx = 0usize;
        while idx + 3 < pts.len() {
            let end = (idx + chunk_pts).min(pts.len());
            let mut slice = pts[idx..end].to_vec();
            if slice.len() >= 4 {
                // Orient segment so it grows outward away from focus_uv (eyes)!
                let d0 = ((slice[0][0] - focus_uv[0]) * aspect).hypot(slice[0][1] - focus_uv[1]);
                let d1 = ((slice[slice.len() - 1][0] - focus_uv[0]) * aspect)
                    .hypot(slice[slice.len() - 1][1] - focus_uv[1]);
                if d1 < d0 {
                    slice.reverse();
                }
                let mid = slice[slice.len() / 2];
                let dm = ((mid[0] - focus_uv[0]) * aspect).hypot(mid[1] - focus_uv[1]);
                if dm > max_dist {
                    max_dist = dm;
                }
                seg_candidates.push((dm, slice, avg_str));
            }
            if end == pts.len() {
                break;
            }
            idx = end.saturating_sub(1);
        }
    }

    // Sort radially outward from eyes so the center blooms first (exact frame 3 -> 8 -> 12!)
    seg_candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Cap total segment count gently if an image is ultra-noisy, keeping the strongest/longest ones
    if seg_candidates.len() > 1600 {
        seg_candidates.retain(|(_, pts, st)| pts.len() >= 7 || *st > 0.55);
    }

    let mut segments = Vec::with_capacity(seg_candidates.len());
    let mut verts = Vec::with_capacity(seg_candidates.len() * 64);

    for (dist, pts, strength) in seg_candidates {
        let d_norm = (dist / max_dist).clamp(0.0, 1.0);
        // Smooth radial wave with tiny natural organic spread jitter
        let jitter = (rng.f32() - 0.5) * 0.55;
        let t_start = (TRACE_START + d_norm.powf(0.88) * TRACE_SPREAD + jitter).max(TRACE_START);
        let t_dur = SEG_MIN_DUR + rng.f32() * (SEG_MAX_DUR - SEG_MIN_DUR);

        // Emit anti-aliased quad ribbon triangles for this segment
        let m = pts.len();
        let inv_m = 1.0 / ((m - 1).max(1) as f32);
        for i in 0..(m - 1) {
            let p0 = pts[i];
            let p1 = pts[i + 1];
            let dx = (p1[0] - p0[0]) * aspect;
            let dy = p1[1] - p0[1];
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-6 {
                continue;
            }
            // Unit perpendicular normal in isotropic space, scaled back to portrait UV
            let nx = (-dy / len) / aspect;
            let ny = dx / len;

            let t0 = t_start + (i as f32 * inv_m) * t_dur;
            let t1 = t_start + ((i + 1) as f32 * inv_m) * t_dur;

            let v0l = TraceVert { u: p0[0], v: p0[1], nx: -nx, ny: -ny, side: -1.0, t_reveal: t0, strength };
            let v0r = TraceVert { u: p0[0], v: p0[1], nx, ny, side: 1.0, t_reveal: t0, strength };
            let v1l = TraceVert { u: p1[0], v: p1[1], nx: -nx, ny: -ny, side: -1.0, t_reveal: t1, strength };
            let v1r = TraceVert { u: p1[0], v: p1[1], nx, ny, side: 1.0, t_reveal: t1, strength };

            verts.push(v0l);
            verts.push(v0r);
            verts.push(v1r);
            verts.push(v0l);
            verts.push(v1r);
            verts.push(v1l);
        }

        segments.push(TraceSegment {
            pts,
            t_start,
            t_dur,
            strength,
        });
    }

    // 6. Pick 45 well-spaced twinkling star anchors along luminous portrait highlights
    let mut candidates: Vec<(f32, f32, f32)> = Vec::new();
    let stride = 8usize;
    for y in (4..h.saturating_sub(4)).step_by(stride) {
        for x in (4..w.saturating_sub(4)).step_by(stride) {
            let idx = y * w + x;
            let l = luma[idx];
            if l > 0.62 {
                candidates.push((l + (nms[idx] / max_mag) * 0.4, (x as f32 + 0.5) * inv_w, (y as f32 + 0.5) * inv_h));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut twinkles: Vec<TwinkleAnchor> = Vec::with_capacity(TWINKLE_COUNT);
    for &(_score, u, v) in &candidates {
        if twinkles.len() >= TWINKLE_COUNT {
            break;
        }
        let too_close = twinkles.iter().any(|t| ((t.u - u) * aspect).hypot(t.v - v) < 0.055);
        if !too_close {
            twinkles.push(TwinkleAnchor {
                u,
                v,
                phase: rng.f32() * 6.2831853,
                speed: 1.4 + rng.f32() * 2.2,
                size: 0.7 + rng.f32() * 0.9,
                tint_idx: (rng.next_u32() % 3) as u8,
            });
        }
    }

    ExtractedArtwork {
        verts,
        segments,
        twinkles,
        focus_uv,
    }
}
