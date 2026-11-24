pub mod shaders;

use crate::gl::*;
use crate::meadow::{
    fbm, make_noise_image, vnoise, IconBox, Mulberry32, MORPH_EMPTY_SEC, MORPH_SEC, STEP, TASKBAR,
};
use crate::state::{AppState, Rect};
use shaders::*;
use std::f64::consts::TAU;
use std::ffi::c_void;
use std::path::PathBuf;

// =========================================================================
// Palm Style Metadata (1:1 prototype/beach-palms.js)
// =========================================================================
#[derive(Clone, Copy)]
pub struct PalmStyle {
    pub file: &'static str,
    pub foot: [f64; 2],
    pub crown: [f64; 2],
    pub radius: f64,
    pub width: f64,
    pub height: f64,
    pub size: f64,
    pub face_sea: bool,
}

pub const BEACH_PALM_STYLES: [PalmStyle; 5] = [
    PalmStyle {
        file: "beach-coconut-palm-v2.png",
        foot: [0.454, 0.98],
        crown: [9.0, -84.0],
        radius: 80.0,
        width: 168.0,
        height: 156.0,
        size: 1.0,
        face_sea: false,
    },
    PalmStyle {
        file: "beach-coconut-palm-curved.png",
        foot: [0.470, 0.98],
        crown: [18.0, -80.0],
        radius: 84.0,
        width: 168.0,
        height: 156.0,
        size: 1.0,
        face_sea: true,
    },
    PalmStyle {
        file: "beach-coconut-palm-airy.png",
        foot: [0.415, 0.98],
        crown: [27.0, -82.0],
        radius: 84.0,
        width: 168.0,
        height: 156.0,
        size: 1.0,
        face_sea: false,
    },
    PalmStyle {
        file: "beach-coconut-palm-windswept.png",
        foot: [0.25, 0.98],
        crown: [40.0, -78.0],
        radius: 78.0,
        width: 148.0,
        height: 138.0,
        size: 1.0,
        face_sea: false,
    },
    PalmStyle {
        file: "beach-coconut-palm-arch.png",
        foot: [0.84, 0.98],
        crown: [-45.0, -65.0],
        radius: 70.0,
        width: 128.0,
        height: 119.0,
        size: 1.35,
        face_sea: true,
    },
];

// Exact Uint8ClampedArray rounding (round half to even in JS; standard .round().clamp(0, 255) matches within 0..1)
#[inline]
fn clamp_u8_js(v: f64) -> u8 {
    if !v.is_finite() || v <= 0.0 {
        return 0;
    }
    if v >= 255.0 {
        return 255;
    }
    let fl = v.floor();
    let frac = v - fl;
    if frac < 0.5 {
        fl as u8
    } else if frac > 0.5 {
        (fl + 1.0) as u8
    } else {
        let i = fl as u8;
        if i % 2 == 0 { i } else { i + 1 }
    }
}

pub fn palm_color_grade_pixel(px: &mut [u8; 4]) {
    if px[3] == 0 {
        return;
    }
    let mut r = px[0] as f64;
    let mut g = px[1] as f64;
    let mut b = px[2] as f64;
    let yellow = ((r / g.max(1.0) - 0.60) / 0.35).clamp(0.0, 1.0) * ((g - b) / 50.0).clamp(0.0, 1.0);
    r *= 1.0 - yellow * 0.22;
    g *= 1.0 - yellow * 0.06;
    b += yellow * 10.0;
    let gray = r * 0.2126 + g * 0.7152 + b * 0.0722;
    px[0] = clamp_u8_js((gray + (r - gray) * 0.70) * 0.97);
    px[1] = clamp_u8_js((gray + (g - gray) * 0.70) * 0.97);
    px[2] = clamp_u8_js((gray + (b - gray) * 0.70) * 0.97);
}

// =========================================================================
// Parameters & Entities (1:1 prototype/flat.html + beach-palms.js)
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct BeachParams {
    pub sigma: f64,
    pub warp: f64,
    pub shallow: f64,
    pub shoal: f64,
    pub speed: f64,
    pub runup: f64,
    pub wave: f64,
    pub spacing: f64,
    pub waves: f64,
    pub grain: f64,
    pub kayaks: usize,
    pub birds: usize,
    pub palms: usize,
    pub seed: i32,
    pub debug: bool,
}

impl Default for BeachParams {
    fn default() -> Self {
        Self {
            sigma: 85.0,
            warp: 34.0,
            shallow: 260.0,
            shoal: 110.0,
            speed: 22.0,
            runup: 120.0,
            wave: 1.6,
            spacing: 70.0,
            waves: 1.0,
            grain: 1.0,
            kayaks: 2,
            birds: 3,
            palms: 4,
            seed: 42,
            debug: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Kayak {
    pub x: f64,
    pub y: f64,
    pub h: f64,
    pub v: f64,
    pub ph: f64,
    pub wander: f64,
}

#[derive(Clone, Copy)]
pub struct Bird {
    pub x: f64,
    pub y: f64,
    pub h: f64,
    pub v: f64,
    pub mode: i32,
    pub mode_t: f64,
    pub ph: f64,
    pub flap: bool,
    pub flap_t: f64,
    pub wander: f64,
}

#[derive(Clone, Copy)]
pub struct Ripple {
    pub x: f64,
    pub y: f64,
    pub t0: f64,
    pub strength: f64,
}

#[derive(Clone, Copy)]
pub struct PalmPlacement {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub variant: usize,
    pub mirror: f64,
    pub opacity: f64,
    pub retiring: bool,
}

#[inline]
fn js_sign(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

#[inline]
pub fn turn_toward(h: f64, target: f64, rate: f64) -> f64 {
    let mut d = target - h;
    d = d.sin().atan2(d.cos());
    h + js_sign(d) * d.abs().min(rate)
}

pub fn beach_palm_clear(icons: &[Rect], x: f64, y: f64, radius: f64) -> bool {
    icons.iter().all(|ic| {
        let dx = (ic.x as f64 - 7.0 - x)
            .max(0.0)
            .max(x - (ic.x + ic.w) as f64 - 7.0);
        let dy = (ic.y as f64 - 7.0 - y)
            .max(0.0)
            .max(y - (ic.y + ic.h) as f64 - 7.0);
        dx.hypot(dy) > radius
    })
}

pub fn choose_maldives_palms(
    fb: &[f32],
    gw: usize,
    gh: usize,
    w: f64,
    h: f64,
    seed: i32,
    icons: &[Rect],
) -> Vec<PalmPlacement> {
    let mut random = Mulberry32::new(seed ^ 0x0041_FC72);
    let mut chosen: Vec<PalmPlacement> = Vec::new();
    let mut order: Vec<usize> = (0..BEACH_PALM_STYLES.len()).collect();
    for i in (1..order.len()).rev() {
        let j = (random.next() * (i + 1) as f64).floor() as usize;
        order.swap(i, j);
    }
    let mut style_random = Mulberry32::new(seed ^ 0x006A_C309);
    while order.len() < 10 {
        order.push((style_random.next() * BEACH_PALM_STYLES.len() as f64).floor() as usize);
    }

    for &variant in &order {
        let style = BEACH_PALM_STYLES[variant];
        let spacing = 60.0 + random.next() * 35.0;
        let mut selected: Option<PalmPlacement> = None;
        for &base_scale in &[0.62f64] {
            let scale = base_scale * 0.80 * style.size;
            let radius = style.radius * scale;
            let offset_x = random.next() * 13.0;
            let offset_y = random.next() * 13.0;
            let mut best: Option<PalmPlacement> = None;
            let mut candidates = 0usize;
            let mut y = 100.0 * scale + offset_y;
            while y < h - TASKBAR - 55.0 * scale {
                let mut x = radius + 5.0 + offset_x;
                while x < w - radius - 5.0 {
                    let shore = sample_arr(fb, gw, gh, x, y);
                    if (-58.0..=-25.0).contains(&shore)
                        && !chosen.iter().any(|q| (q.x - x).hypot(q.y - y) < spacing)
                    {
                        let sea_x = if style.face_sea {
                            sample_arr(fb, gw, gh, x + 8.0, y) - sample_arr(fb, gw, gh, x - 8.0, y)
                        } else {
                            0.0
                        };
                        let mirror = if style.face_sea && sea_x * style.crown[0] < 0.0 {
                            -1.0
                        } else {
                            1.0
                        };
                        let cx = x + style.crown[0] * scale * mirror;
                        let cy = y + style.crown[1] * scale;
                        if cx >= radius + 3.0
                            && cx <= w - radius - 3.0
                            && cy >= radius + 3.0
                            && beach_palm_clear(icons, x, y - 16.0 * scale, 10.0 * scale)
                            && beach_palm_clear(icons, cx, cy, radius)
                        {
                            candidates += 1;
                            if random.next() < 1.0 / (candidates as f64) {
                                best = Some(PalmPlacement {
                                    x,
                                    y,
                                    scale,
                                    variant,
                                    mirror,
                                    opacity: 0.0,
                                    retiring: false,
                                });
                            }
                        }
                    }
                    x += 13.0;
                }
                y += 13.0;
            }
            if best.is_some() {
                selected = best;
                break;
            }
        }
        if let Some(p) = selected {
            chosen.push(p);
        }
    }
    chosen
}

pub fn sample_arr(a: &[f32], gw: usize, gh: usize, x: f64, y: f64) -> f64 {
    if a.is_empty() || gw < 2 || gh < 2 {
        return 999.0;
    }
    let fx = (x / STEP).clamp(0.0, gw as f64 - 1.001);
    let fy = (y / STEP).clamp(0.0, gh as f64 - 1.001);
    let ix = fx.floor() as usize;
    let iy = fy.floor() as usize;
    let tx = fx - ix as f64;
    let ty = fy - iy as f64;
    let i = iy * gw + ix;
    let v00 = a[i] as f64;
    let v10 = a[i + 1] as f64;
    let v01 = a[i + gw] as f64;
    let v11 = a[i + gw + 1] as f64;
    (v00 * (1.0 - tx) + v10 * tx) * (1.0 - ty) + (v01 * (1.0 - tx) + v11 * tx) * ty
}

// =========================================================================
// Beach State Machine
// =========================================================================
pub struct BeachState {
    pub p: BeachParams,
    pub w: f64,
    pub h: f64,
    pub gw: usize,
    pub gh: usize,
    pub wx: Vec<f32>,
    pub wy: Vec<f32>,
    pub fa: Vec<f32>,
    pub fb: Vec<f32>,
    pub field_gen: u64,
    pub field_dirty_a: bool,
    pub field_dirty_b: bool,
    pub noise_dirty: bool,
    pub morph_elapsed: f64,
    pub morph_dur: f64,
    pub cur_mix: f64,
    pub time: f64,
    pub icons: Vec<Rect>,
    pub kayaks: Vec<Kayak>,
    pub birds: Vec<Bird>,
    pub ripples: Vec<Ripple>,
    pub palms: Vec<PalmPlacement>,
    palm_placements: Vec<PalmPlacement>,
    palm_sig: (u64, i32, i32, i32, usize),
    pub rng: Mulberry32,
    pub icons_seen: bool,
    pub first_icons: bool,
}

impl Default for BeachState {
    fn default() -> Self {
        let mut st = Self {
            p: BeachParams::default(),
            w: 1280.0,
            h: 720.0,
            gw: 1,
            gh: 1,
            wx: Vec::new(),
            wy: Vec::new(),
            fa: Vec::new(),
            fb: Vec::new(),
            field_gen: 0,
            field_dirty_a: true,
            field_dirty_b: true,
            noise_dirty: true,
            morph_elapsed: 10.0,
            morph_dur: MORPH_SEC,
            cur_mix: 1.0,
            time: 0.0,
            icons: Vec::new(),
            kayaks: Vec::new(),
            birds: Vec::new(),
            ripples: Vec::new(),
            palms: Vec::new(),
            palm_placements: Vec::new(),
            palm_sig: (0, 0, 0, 0, usize::MAX),
            rng: Mulberry32::new(42 ^ 0x4245_4143),
            icons_seen: false,
            first_icons: true,
        };
        st.resize(1280.0, 720.0);
        st.sync_birds(false);
        st.sync_kayaks();
        st
    }
}

impl BeachState {
    pub fn build_noise(&mut self) {
        let n = self.gw * self.gh;
        self.wx.resize(n, 0.0);
        self.wy.resize(n, 0.0);
        let seed = self.p.seed;
        for gy in 0..self.gh {
            for gx in 0..self.gw {
                let i = gy * self.gw + gx;
                let x = gx as f64 * STEP;
                let y = gy as f64 * STEP;
                self.wx[i] = (fbm(x * 0.0028, y * 0.0028, 1, seed) - 0.47) as f32;
                self.wy[i] = (fbm(x * 0.0028, y * 0.0028, 2, seed) - 0.47) as f32;
            }
        }
    }

    pub fn compute_field_raw(&self, rects: &[IconBox]) -> Vec<f32> {
        let n = self.gw * self.gh;
        let mut f1 = vec![0.0f32; n];
        let sig = self.p.sigma;
        let is1 = 1.0 / (sig * sig);
        let wa = self.p.warp * 2.2;
        let wmax = wa * 0.5;
        for r in rects {
            let rad = 5.5 * sig + r.hw.max(r.hh) * 0.5 + wmax;
            let gx0 = ((r.cx - rad) / STEP).floor().max(0.0) as usize;
            let gx1 = (((r.cx + rad) / STEP).ceil() as isize)
                .clamp(0, self.gw as isize - 1) as usize;
            let gy0 = ((r.cy - rad) / STEP).floor().max(0.0) as usize;
            let gy1 = (((r.cy + rad) / STEP).ceil() as isize)
                .clamp(0, self.gh as isize - 1) as usize;
            let chw = r.hw * 0.5;
            let chh = r.hh * 0.5;
            for gy in gy0..=gy1 {
                for gx in gx0..=gx1 {
                    let i = gy * self.gw + gx;
                    let qx = gx as f64 * STEP + self.wx[i] as f64 * wa;
                    let qy = gy as f64 * STEP + self.wy[i] as f64 * wa;
                    let dx = ((qx - r.cx).abs() - chw).max(0.0);
                    let dy = ((qy - r.cy).abs() - chh).max(0.0);
                    f1[i] += (-(dx * dx + dy * dy) * is1).exp() as f32;
                }
            }
        }
        let c0 = (-0.35f64.ln()).sqrt();
        let f_min = (-30.0f64).exp();
        let mut s = vec![0.0f32; n];
        for i in 0..n {
            let g = (f1[i] as f64).max(f_min).ln();
            s[i] = (sig * ((-g).max(0.0).sqrt() - c0) - sig * 0.6 * g.max(0.0)) as f32;
        }
        s
    }

    #[inline]
    pub fn cur_s(&self, x: f64, y: f64) -> f64 {
        if self.fb.is_empty() {
            return 999.0;
        }
        sample_arr(&self.fa, self.gw, self.gh, x, y) * (1.0 - self.cur_mix)
            + sample_arr(&self.fb, self.gw, self.gh, x, y) * self.cur_mix
    }

    fn icon_boxes(&self) -> Vec<IconBox> {
        self.icons.iter().map(IconBox::from_rect).collect()
    }

    pub fn commit(&mut self, from_empty: bool) {
        let boxes = self.icon_boxes();
        let nf = self.compute_field_raw(&boxes);
        if from_empty {
            self.fa = self.compute_field_raw(&[]);
        } else {
            self.fa = self.fb.clone();
        }
        self.fb = nf;
        self.field_gen = self.field_gen.wrapping_add(1);
        self.field_dirty_a = true;
        self.field_dirty_b = true;
        self.morph_elapsed = 0.0;
        self.morph_dur = if from_empty { MORPH_EMPTY_SEC } else { MORPH_SEC };
        self.cur_mix = 0.0;
    }

    pub fn refresh(&mut self) {
        let boxes = self.icon_boxes();
        let nf = self.compute_field_raw(&boxes);
        self.fa = nf.clone();
        self.fb = nf;
        self.field_gen = self.field_gen.wrapping_add(1);
        self.field_dirty_a = true;
        self.field_dirty_b = true;
        self.morph_elapsed = self.morph_dur;
        self.cur_mix = 1.0;
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = (w as f64).max(320.0);
        let nh = (h as f64).max(200.0);
        let ngw = (nw / STEP).ceil() as usize + 2;
        let ngh = (nh / STEP).ceil() as usize + 2;
        if (self.w - nw).abs() < 0.5 && (self.h - nh).abs() < 0.5 && self.gw == ngw && self.gh == ngh
        {
            return;
        }
        self.w = nw;
        self.h = nh;
        self.gw = ngw;
        self.gh = ngh;
        self.build_noise();
        self.refresh();
        self.kayaks.clear();
        self.sync_kayaks();
    }

    pub fn set_icons(&mut self, icons: &[Rect], vis: bool) {
        let next: Vec<Rect> = if vis { icons.to_vec() } else { Vec::new() };
        let same = next.len() == self.icons.len()
            && next.iter().zip(self.icons.iter()).all(|(a, b)| {
                (a.x - b.x).abs() < 0.5
                    && (a.y - b.y).abs() < 0.5
                    && (a.w - b.w).abs() < 0.5
                    && (a.h - b.h).abs() < 0.5
            });
        if same && self.icons_seen {
            return;
        }
        self.icons = next;
        self.icons_seen = true;
        let first = self.first_icons;
        self.first_icons = false;
        self.commit(first);
        if first {
            let prev_mix = self.cur_mix;
            self.cur_mix = 1.0;
            self.kayaks.clear();
            self.sync_kayaks();
            self.cur_mix = prev_mix;
        }
    }

    pub fn add_ripple_if_water(&mut self, x: f32, y: f32, strength: f64) {
        let xf = x as f64;
        let yf = y as f64;
        if self.cur_s(xf, yf) > 4.0 {
            self.ripples.push(Ripple {
                x: xf,
                y: yf,
                t0: self.time,
                strength,
            });
            if self.ripples.len() > 6 {
                self.ripples.remove(0);
            }
        }
    }

    pub fn sync_params(&mut self, next: BeachParams) {
        let seed_changed = next.seed != self.p.seed;
        let field_changed =
            (next.sigma - self.p.sigma).abs() > 1e-4 || (next.warp - self.p.warp).abs() > 1e-4;
        let kay_changed = next.kayaks != self.p.kayaks;
        let bird_changed = next.birds != self.p.birds;
        self.p = next;
        if seed_changed {
            self.rng = Mulberry32::new(self.p.seed ^ 0x4245_4143);
            self.build_noise();
            self.noise_dirty = true;
            if self.icons_seen {
                self.commit(true);
                let prev_mix = self.cur_mix;
                self.cur_mix = 1.0;
                self.kayaks.clear();
                self.sync_kayaks();
                self.cur_mix = prev_mix;
            } else {
                self.refresh();
            }
            return;
        }
        if field_changed {
            self.refresh();
        }
        if kay_changed {
            self.sync_kayaks();
        }
        if bird_changed {
            self.sync_birds(true);
        }
    }

    fn spawn_kayak(&mut self) -> Option<Kayak> {
        let span_x = (self.w - 120.0).max(10.0);
        let span_y = (self.h - TASKBAR - 120.0).max(10.0);
        for _ in 0..400 {
            let x = 60.0 + self.rng.next() * span_x;
            let y = 60.0 + self.rng.next() * span_y;
            if self.cur_s(x, y) > 170.0
                && self.kayaks.iter().all(|k| (k.x - x).hypot(k.y - y) > 160.0)
            {
                return Some(Kayak {
                    x,
                    y,
                    h: self.rng.next() * TAU,
                    v: 7.0 + self.rng.next() * 4.0,
                    ph: self.rng.next() * 6.0,
                    wander: self.rng.next() * 100.0,
                });
            }
        }
        None
    }

    pub fn sync_kayaks(&mut self) {
        let want = self.p.kayaks.min(4);
        while self.kayaks.len() > want {
            self.kayaks.pop();
        }
        while self.kayaks.len() < want {
            let Some(k) = self.spawn_kayak() else { break };
            self.kayaks.push(k);
        }
    }

    pub fn step_kayaks(&mut self, dt: f64) {
        let clearance = 64.0;
        let n = self.kayaks.len();
        if n == 0 {
            return;
        }
        struct Plan {
            h: f64,
            dx: f64,
            dy: f64,
        }
        let mut plans = Vec::with_capacity(n);
        let seed = self.p.seed;
        for i in 0..n {
            let k = self.kayaks[i];
            let mut heading = k.h;
            let ax = k.x + k.h.cos() * 70.0;
            let ay = k.y + k.h.sin() * 70.0;
            let margin = ax < 40.0 || ay < 40.0 || ax > self.w - 40.0 || ay > self.h - TASKBAR - 40.0;
            let mut wander = k.wander;
            if self.cur_s(ax, ay) < 150.0 || margin {
                let mut gx = self.cur_s(k.x + 10.0, k.y) - self.cur_s(k.x - 10.0, k.y);
                let mut gy = self.cur_s(k.x, k.y + 10.0) - self.cur_s(k.x, k.y - 10.0);
                if margin {
                    gx += (self.w * 0.5 - k.x) * 0.02;
                    gy += ((self.h - TASKBAR) * 0.5 - k.y) * 0.02;
                }
                let mut d = gy.atan2(gx) - k.h;
                d = d.sin().atan2(d.cos());
                heading += js_sign(d) * d.abs().min(0.7 * dt);
            } else {
                wander += dt * 0.3;
                heading += (vnoise(wander, (i as f64) * 10.0, 0, seed) - 0.5) * 0.25 * dt;
            }
            self.kayaks[i].wander = wander;

            let (fx, fy) = (k.h.cos(), k.h.sin());
            let mut steer_x = heading.cos();
            let mut steer_y = heading.sin();
            let mut speed = 1.0f64;
            let mut avoiding = false;
            for j in 0..n {
                if i == j {
                    continue;
                }
                let other = self.kayaks[j];
                let dx = other.x - k.x;
                let dy = other.y - k.y;
                let distance = dx.hypot(dy);
                if distance > 210.0 {
                    continue;
                }
                let rvx = other.h.cos() * other.v - fx * k.v;
                let rvy = other.h.sin() * other.v - fy * k.v;
                let closing = dx * rvx + dy * rvy;
                let ahead = (-closing / (rvx * rvx + rvy * rvy).max(0.01)).clamp(0.0, 7.0);
                let closest = (dx + rvx * ahead).hypot(dy + rvy * ahead);
                let close_risk = if closing < 0.0 {
                    (1.0 - (closest / 90.0).min(1.0)) * (1.0 - ahead / 8.0)
                } else {
                    0.0
                };
                let risk = ((110.0 - distance) / 46.0).clamp(0.0, 1.0).max(close_risk);
                if risk <= 0.0 {
                    continue;
                }
                avoiding = true;
                let (away_x, away_y) = if distance > 0.001 {
                    (-dx / distance, -dy / distance)
                } else {
                    (-fy, fx)
                };
                steer_x += (away_x * 2.0 - fy * 1.6) * risk;
                steer_y += (away_y * 2.0 + fx * 1.6) * risk;
                if dx * fx + dy * fy > 0.0 && closing < 0.0 {
                    speed = speed.min(0.25 + 0.75 * ((distance - clearance) / 50.0).clamp(0.0, 1.0));
                }
            }
            if avoiding {
                let target_a = steer_y.atan2(steer_x) - k.h;
                let d = target_a.sin().atan2(target_a.cos());
                heading = k.h + js_sign(d) * d.abs().min(dt);
            }
            let ph = k.ph + dt * 2.4;
            self.kayaks[i].ph = ph;
            let v = k.v * (0.75 + 0.25 * ph.sin().abs()) * speed;
            plans.push(Plan {
                h: heading,
                dx: heading.cos() * v * dt,
                dy: heading.sin() * v * dt,
            });
        }

        for _ in 0..n {
            for i in 0..n {
                for j in (i + 1)..n {
                    let dx = self.kayaks[j].x - self.kayaks[i].x;
                    let dy = self.kayaks[j].y - self.kayaks[i].y;
                    let vx = plans[j].dx - plans[i].dx;
                    let vy = plans[j].dy - plans[i].dy;
                    let vv = vx * vx + vy * vy;
                    let dot = dx * vx + dy * vy;
                    if dot >= 0.0 || vv < 1e-12 {
                        continue;
                    }
                    let closest_at = (-dot / vv).clamp(0.0, 1.0);
                    if (dx + vx * closest_at).hypot(dy + vy * closest_at) >= clearance {
                        continue;
                    }
                    let gap = dx * dx + dy * dy - clearance * clearance;
                    let fraction = if gap <= 0.0 {
                        0.0
                    } else {
                        (gap / (-dot + (dot * dot - vv * gap).max(0.0).sqrt()) - 0.001)
                            .clamp(0.0, 1.0)
                    };
                    plans[i].dx *= fraction;
                    plans[i].dy *= fraction;
                    plans[j].dx *= fraction;
                    plans[j].dy *= fraction;
                }
            }
        }

        let mut i = n;
        while i > 0 {
            i -= 1;
            self.kayaks[i].h = plans[i].h;
            self.kayaks[i].x += plans[i].dx;
            self.kayaks[i].y += plans[i].dy;
            if self.cur_s(self.kayaks[i].x, self.kayaks[i].y) < 40.0 {
                if let Some(nk) = self.spawn_kayak() {
                    self.kayaks[i] = nk;
                } else {
                    self.kayaks.remove(i);
                }
            }
        }
    }

    fn spawn_bird(&mut self, from_edge: bool) -> Bird {
        let h2 = (self.h - TASKBAR).max(100.0);
        let mut x = self.rng.next() * self.w;
        let mut y = self.rng.next() * h2;
        if from_edge {
            match (self.rng.next() * 4.0).floor() as usize {
                0 => x = -40.0,
                1 => x = self.w + 40.0,
                2 => y = -40.0,
                _ => y = h2 + 40.0,
            }
        }
        let h = (h2 * 0.5 - y).atan2(self.w * 0.5 - x) + (self.rng.next() - 0.5) * 1.2;
        Bird {
            x,
            y,
            h,
            v: 36.0 + self.rng.next() * 16.0,
            mode: 0,
            mode_t: 2.0 + self.rng.next() * 4.0,
            ph: 0.0,
            flap: false,
            flap_t: self.rng.next() * 3.0,
            wander: self.rng.next() * 100.0,
        }
    }

    pub fn sync_birds(&mut self, from_edge: bool) {
        let want = self.p.birds.min(8);
        while self.birds.len() > want {
            self.birds.pop();
        }
        while self.birds.len() < want {
            let b = self.spawn_bird(from_edge);
            self.birds.push(b);
        }
    }

    pub fn step_birds(&mut self, dt: f64) {
        let h2 = self.h - TASKBAR;
        let m = 90.0;
        let w = self.w;
        let seed = self.p.seed;
        for i in 0..self.birds.len() {
            let mut b = self.birds[i];
            b.mode_t -= dt;
            if b.mode_t <= 0.0 {
                b.mode = if self.rng.next() < 0.4 {
                    if self.rng.next() < 0.5 { -1 } else { 1 }
                } else {
                    0
                };
                b.mode_t = if b.mode != 0 {
                    4.0 + self.rng.next() * 6.0
                } else {
                    3.0 + self.rng.next() * 5.0
                };
            }
            b.wander += dt * 0.4;
            if b.x < -m || b.x > w + m || b.y < -m || b.y > h2 + m {
                b.h = turn_toward(b.h, (h2 * 0.5 - b.y).atan2(w * 0.5 - b.x), 1.3 * dt);
            } else {
                b.h += (b.mode as f64 * 0.55
                    + (vnoise(b.wander, i as f64 * 7.3, 0, seed) - 0.5) * 0.8)
                    * dt;
            }
            b.flap_t -= dt;
            if b.flap_t <= 0.0 {
                b.flap = !b.flap;
                b.flap_t = if b.flap {
                    0.8 + self.rng.next() * 1.2
                } else {
                    2.5 + self.rng.next() * 4.0
                };
            }
            if b.flap || b.ph.cos() < 0.97 {
                b.ph = (b.ph + dt * 9.0) % TAU;
            }
            let v = b.v * if b.flap { 1.15 } else { 1.0 };
            b.x += b.h.cos() * v * dt;
            b.y += b.h.sin() * v * dt;
            self.birds[i] = b;
        }
    }

    fn step_palms(&mut self, dt: f64) {
        let count = self.p.palms.min(10);
        let cur_sig = (
            self.field_gen,
            self.p.seed,
            self.w.round() as i32,
            self.h.round() as i32,
            count,
        );
        let layout_changed = self.palm_sig.0 != cur_sig.0
            || self.palm_sig.1 != cur_sig.1
            || self.palm_sig.2 != cur_sig.2
            || self.palm_sig.3 != cur_sig.3;
        if layout_changed || self.palm_sig.4 != cur_sig.4 {
            if layout_changed || (count > 0 && self.palm_placements.is_empty()) {
                self.palm_placements = if count > 0 {
                    choose_maldives_palms(
                        &self.fb,
                        self.gw,
                        self.gh,
                        self.w,
                        self.h,
                        self.p.seed,
                        &self.icons,
                    )
                } else {
                    Vec::new()
                };
            }
            let next: Vec<PalmPlacement> = self.palm_placements.iter().take(count).copied().collect();
            for old in &mut self.palms {
                old.retiring = true;
            }
            for p in next {
                if let Some(old) = self.palms.iter_mut().find(|q| {
                    (q.x - p.x).hypot(q.y - p.y) < 26.0
                        && (q.scale - p.scale).abs() < 0.02
                        && (q.mirror - p.mirror).abs() < 0.01
                }) {
                    old.retiring = false;
                    old.variant = p.variant;
                } else {
                    self.palms.push(PalmPlacement {
                        opacity: 0.0,
                        retiring: false,
                        ..p
                    });
                }
            }
            self.palm_sig = cur_sig;
        }

        let k = 1.0 - (-dt * 4.0).exp();
        for i in 0..self.palms.len() {
            let (px, py, retiring) = (self.palms[i].x, self.palms[i].y, self.palms[i].retiring);
            let on_sand = (-self.cur_s(px, py) / 4.0).clamp(0.0, 1.0);
            let target = if retiring { 0.0 } else { on_sand };
            self.palms[i].opacity += (target - self.palms[i].opacity) * k;
        }
        self.palms.retain(|p| !p.retiring || p.opacity > 0.003);
    }

    pub fn step(&mut self, dt_f32: f32) {
        let dt = (dt_f32 as f64).clamp(0.001, 0.1);
        self.time += dt;

        if self.morph_elapsed < self.morph_dur {
            self.morph_elapsed = (self.morph_elapsed + dt).min(self.morph_dur);
        }
        let e = (self.morph_elapsed / self.morph_dur.max(0.001)).clamp(0.0, 1.0);
        self.cur_mix = if e < 0.5 {
            2.0 * e * e
        } else {
            1.0 - (-2.0 * e + 2.0).powi(2) * 0.5
        };

        self.step_kayaks(dt);
        self.step_birds(dt);
        self.step_palms(dt);
        self.ripples.retain(|r| (self.time - r.t0) < 3.0);
    }
}

// =========================================================================
// CPU Palm Atlas Synthesizer (1:1 makeMaldivesPalmAtlas in beach-palms.js)
// =========================================================================
fn find_prototype_asset(filename: &str) -> Option<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            bases.push(dir.to_path_buf());
            if let Some(p1) = dir.parent() {
                bases.push(p1.to_path_buf());
                if let Some(p2) = p1.parent() {
                    bases.push(p2.to_path_buf());
                }
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd.clone());
        if let Some(p1) = cwd.parent() {
            bases.push(p1.to_path_buf());
            if let Some(p2) = p1.parent() {
                bases.push(p2.to_path_buf());
            }
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(p1) = manifest.parent() {
        if let Some(p2) = p1.parent() {
            bases.push(p2.to_path_buf());
        }
    }
    for base in bases {
        let p = base.join("prototype").join("assets").join(filename);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn blend_over(dst: &mut [u8; 4], src: [f64; 4]) {
    let sa = src[3].clamp(0.0, 1.0);
    if sa <= 1e-4 {
        return;
    }
    let da = dst[3] as f64 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-5 {
        *dst = [0, 0, 0, 0];
        return;
    }
    for c in 0..3 {
        let dc = dst[c] as f64 / 255.0;
        let oc = (src[c] * sa + dc * da * (1.0 - sa)) / out_a;
        dst[c] = (oc * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn sample_rgba_bilinear(img: &image::RgbaImage, u: f64, v: f64) -> [f64; 4] {
    let w = img.width() as f64;
    let h = img.height() as f64;
    if u < 0.0 || v < 0.0 || u >= w || v >= h {
        return [0.0; 4];
    }
    let x0 = (u.floor() as u32).min(img.width() - 1);
    let y0 = (v.floor() as u32).min(img.height() - 1);
    let x1 = (x0 + 1).min(img.width() - 1);
    let y1 = (y0 + 1).min(img.height() - 1);
    let fx = u - x0 as f64;
    let fy = v - y0 as f64;
    let p00 = img.get_pixel(x0, y0).0;
    let p10 = img.get_pixel(x1, y0).0;
    let p01 = img.get_pixel(x0, y1).0;
    let p11 = img.get_pixel(x1, y1).0;
    let mut out = [0.0; 4];
    for c in 0..4 {
        let top = p00[c] as f64 * (1.0 - fx) + p10[c] as f64 * fx;
        let bot = p01[c] as f64 * (1.0 - fx) + p11[c] as f64 * fx;
        out[c] = (top * (1.0 - fy) + bot * fy) / 255.0;
    }
    out
}

fn box_blur_alpha(buf: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    let mut tmp = vec![0.0f32; w * h];
    let mut out = vec![0.0f32; w * h];
    let r = radius as isize;
    let inv = 1.0 / (2 * radius + 1) as f32;
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0f32;
            for k in -r..=r {
                let sx = (x as isize + k).clamp(0, w as isize - 1) as usize;
                sum += buf[y * w + sx];
            }
            tmp[y * w + x] = sum * inv;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0f32;
            for k in -r..=r {
                let sy = (y as isize + k).clamp(0, h as isize - 1) as usize;
                sum += tmp[sy * w + x];
            }
            out[y * w + x] = sum * inv;
        }
    }
    out
}

fn build_palm_atlas_rgba() -> Option<(u32, u32, Vec<u8>)> {
    let cell_w = 448usize; // 224 * 2
    let cell_h = 512usize; // 256 * 2
    let atlas_w = cell_w * BEACH_PALM_STYLES.len();
    let atlas_h = cell_h;
    let mut atlas = vec![0u8; atlas_w * atlas_h * 4];

    for (variant, style) in BEACH_PALM_STYLES.iter().enumerate() {
        let path = find_prototype_asset(style.file)?;
        let raw_img = image::open(&path).ok()?;
        let tw = (style.width * 2.0).round() as u32;
        let th = (style.height * 2.0).round() as u32;
        let resized = raw_img
            .resize_exact(tw, th, image::imageops::FilterType::Triangle)
            .to_rgba8();

        let foot_x = style.foot[0] * style.width;
        let foot_y = style.foot[1] * style.height;

        // 1) Affine-sheared shadow mask: transform(0.85, 0.08, 0.0, -0.50, 112, 162)
        // Forward: lx = 112 + 0.85 * sx, ly = 162 + 0.08 * sx - 0.50 * sy
        // Inverse: sx = (lx - 112) / 0.85, sy = (162 + 0.08 * sx - ly) / 0.50
        let mut sh_alpha = vec![0.0f32; cell_w * cell_h];
        for py in 0..cell_h {
            let ly = (py as f64 + 0.5) * 0.5;
            for px in 0..cell_w {
                let lx = (px as f64 + 0.5) * 0.5;
                let sx = (lx - 112.0) / 0.85;
                let sy = (162.0 + 0.08 * sx - ly) / 0.50;
                let u = (sx + foot_x) * 2.0;
                let v = (sy + foot_y) * 2.0;
                if u >= 0.0 && v >= 0.0 && u < tw as f64 && v < th as f64 {
                    let s = sample_rgba_bilinear(&resized, u, v);
                    sh_alpha[py * cell_w + px] = s[3] as f32;
                }
            }
        }
        // Soft 2-pass box blur approximating CSS blur(1.2px) at 2x scale
        let sh_blur = box_blur_alpha(&box_blur_alpha(&sh_alpha, cell_w, cell_h, 2), cell_w, cell_h, 2);

        let x_off = variant * cell_w;
        // Composite shadow (#3c6257 @ 0.20) + root ellipse (#516051 @ 0.17) + palm image (sat 0.94, bri 0.98)
        for py in 0..cell_h {
            let ly = (py as f64 + 0.5) * 0.5;
            for px in 0..cell_w {
                let lx = (px as f64 + 0.5) * 0.5;
                let mut out_px = [0u8; 4];

                let sa = sh_blur[py * cell_w + px] as f64 * 0.20;
                if sa > 0.001 {
                    blend_over(&mut out_px, [0x3c as f64 / 255.0, 0x62 as f64 / 255.0, 0x57 as f64 / 255.0, sa]);
                }

                // Root ellipse at (112, 162) radii (4.4, 1.6)
                let ex = (lx - 112.0) / 4.4;
                let ey = (ly - 162.0) / 1.6;
                let ed = (ex * ex + ey * ey).sqrt();
                if ed < 1.15 {
                    let ea = (1.0 - ((ed - 0.85) / 0.30).clamp(0.0, 1.0)) * 0.17;
                    blend_over(&mut out_px, [0x51 as f64 / 255.0, 0x60 as f64 / 255.0, 0x51 as f64 / 255.0, ea]);
                }

                // Palm image at (112 - foot_x, 162 - foot_y)
                let u = (lx - 112.0 + foot_x) * 2.0;
                let v = (ly - 162.0 + foot_y) * 2.0;
                if u >= 0.0 && v >= 0.0 && u < tw as f64 && v < th as f64 {
                    let mut p = sample_rgba_bilinear(&resized, u, v);
                    if p[3] > 0.001 {
                        // saturate(0.94) brightness(0.98)
                        let luma = p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722;
                        for c in 0..3 {
                            p[c] = ((luma + (p[c] - luma) * 0.94) * 0.98).clamp(0.0, 1.0);
                        }
                        blend_over(&mut out_px, p);
                    }
                }

                // Exact desaturation & yellow suppression pass
                palm_color_grade_pixel(&mut out_px);

                let dst_i = (py * atlas_w + x_off + px) * 4;
                atlas[dst_i..dst_i + 4].copy_from_slice(&out_px);
            }
        }
    }

    Some((atlas_w as u32, atlas_h as u32, atlas))
}

// =========================================================================
// OpenGL 3.3 Core Beach Renderer
// =========================================================================
pub struct BeachRenderer {
    dummy_vao: GLuint,
    prog_main: GLuint,
    prog_palm: GLuint,
    noise_tex: GLuint,
    ftex: [GLuint; 2],
    field_w: usize,
    field_h: usize,
    palm_atlas_tex: Option<GLuint>,
}

impl BeachRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let prog_main = gl.compile_program(BEACH_VERT, &beach_frag_source())?;
        let prog_palm = gl.compile_program(BEACH_PALM_VERT, &beach_palm_frag_source())?;

        let mut noise_tex = 0;
        (gl.GenTextures)(1, &mut noise_tex);
        let mut ftex = [0u32; 2];
        (gl.GenTextures)(2, ftex.as_mut_ptr());

        let palm_atlas_tex = build_palm_atlas_rgba().map(|(w, h, rgba)| {
            let mut tex = 0u32;
            (gl.GenTextures)(1, &mut tex);
            (gl.ActiveTexture)(GL_TEXTURE6);
            (gl.BindTexture)(GL_TEXTURE_2D, tex);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                w as GLsizei,
                h as GLsizei,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                rgba.as_ptr() as *const c_void,
            );
            (gl.GenerateMipmap)(GL_TEXTURE_2D);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            tex
        });

        Ok(Self {
            dummy_vao,
            prog_main,
            prog_palm,
            noise_tex,
            ftex,
            field_w: 0,
            field_h: 0,
            palm_atlas_tex,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if let Some(t) = self.palm_atlas_tex.take() {
            (gl.DeleteTextures)(1, &t);
        }
        (gl.DeleteTextures)(2, self.ftex.as_ptr());
        if self.noise_tex != 0 {
            (gl.DeleteTextures)(1, &self.noise_tex);
            self.noise_tex = 0;
        }
        if self.prog_palm != 0 {
            (gl.DeleteProgram)(self.prog_palm);
            self.prog_palm = 0;
        }
        if self.prog_main != 0 {
            (gl.DeleteProgram)(self.prog_main);
            self.prog_main = 0;
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
    }

    unsafe fn sync_noise_tex(&mut self, gl: &GlFns, b: &mut BeachState) {
        if !b.noise_dirty {
            return;
        }
        b.noise_dirty = false;
        let (s, data) = make_noise_image(b.p.seed);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            s as GLsizei,
            s as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            data.as_ptr() as *const c_void,
        );
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);
    }

    unsafe fn sync_field_textures(&mut self, gl: &GlFns, b: &mut BeachState) {
        if self.field_w != b.gw || self.field_h != b.gh {
            self.field_w = b.gw;
            self.field_h = b.gh;
            for &tex in &self.ftex {
                (gl.BindTexture)(GL_TEXTURE_2D, tex);
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_R32F as GLint,
                    b.gw as GLsizei,
                    b.gh as GLsizei,
                    0,
                    GL_RED,
                    GL_FLOAT,
                    std::ptr::null(),
                );
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            }
            b.field_dirty_a = true;
            b.field_dirty_b = true;
        }
        if b.field_dirty_a && b.fa.len() == b.gw * b.gh {
            b.field_dirty_a = false;
            (gl.ActiveTexture)(GL_TEXTURE1);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[0]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            (gl.TexSubImage2D)(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                b.gw as GLsizei,
                b.gh as GLsizei,
                GL_RED,
                GL_FLOAT,
                b.fa.as_ptr() as *const c_void,
            );
        }
        if b.field_dirty_b && b.fb.len() == b.gw * b.gh {
            b.field_dirty_b = false;
            (gl.ActiveTexture)(GL_TEXTURE2);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[1]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            (gl.TexSubImage2D)(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                b.gw as GLsizei,
                b.gh as GLsizei,
                GL_RED,
                GL_FLOAT,
                b.fb.as_ptr() as *const c_void,
            );
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.5);

        st.beach.resize(css_w, css_h);
        self.sync_noise_tex(gl, &mut st.beach);
        self.sync_field_textures(gl, &mut st.beach);

        let b = &st.beach;
        let mut rip_buf = [0.0f32; 24];
        for (i, r) in b.ripples.iter().take(6).enumerate() {
            let age = b.time - r.t0;
            if age < 3.0 {
                rip_buf[i * 4] = r.x as f32;
                rip_buf[i * 4 + 1] = r.y as f32;
                rip_buf[i * 4 + 2] = age as f32;
                rip_buf[i * 4 + 3] = r.strength as f32;
            }
        }
        let mut kay_buf = [0.0f32; 16];
        for (i, k) in b.kayaks.iter().take(4).enumerate() {
            kay_buf[i * 4] = k.x as f32;
            kay_buf[i * 4 + 1] = k.y as f32;
            kay_buf[i * 4 + 2] = k.h as f32;
            kay_buf[i * 4 + 3] = k.ph as f32;
        }
        let mut bird_buf = [0.0f32; 32];
        for (i, bd) in b.birds.iter().take(8).enumerate() {
            bird_buf[i * 4] = bd.x as f32;
            bird_buf[i * 4 + 1] = bd.y as f32;
            bird_buf[i * 4 + 2] = bd.h as f32;
            bird_buf[i * 4 + 3] = (0.68 + 0.32 * bd.ph.cos()) as f32;
        }

        let seed_u = ((b.p.seed.rem_euclid(997)) as f32) * 0.37;
        let configure_pass = |prog: GLuint| {
            (gl.UseProgram)(prog);
            (gl.Viewport)(0, 0, pw, ph);
            (gl.ActiveTexture)(GL_TEXTURE0);
            (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
            (gl.Uniform1i)(gl.uniform_loc(prog, "uNoise"), 0);

            (gl.ActiveTexture)(GL_TEXTURE1);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[0]);
            (gl.Uniform1i)(gl.uniform_loc(prog, "uFA"), 1);

            (gl.ActiveTexture)(GL_TEXTURE2);
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[1]);
            (gl.Uniform1i)(gl.uniform_loc(prog, "uFB"), 2);

            (gl.Uniform2f)(gl.uniform_loc(prog, "uRes"), pw as f32 / dpi, ph as f32 / dpi);
            (gl.Uniform2f)(gl.uniform_loc(prog, "uGrid"), b.gw as f32, b.gh as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uScale"), dpi);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uSeed"), seed_u);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uStep"), STEP as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uMix"), b.cur_mix as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uTime"), b.time as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uSpeed"), b.p.speed as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uRunup"), b.p.runup as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uShallow"), b.p.shallow as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uShoal"), b.p.shoal as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uGrain"), b.p.grain as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uWave"), b.p.wave as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uSigma"), b.p.sigma as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uSpacing"), b.p.spacing as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uWaves"), b.p.waves as f32);
            (gl.Uniform1f)(gl.uniform_loc(prog, "uDebug"), if b.p.debug { 1.0 } else { 0.0 });
            (gl.Uniform1f)(gl.uniform_loc(prog, "uKayN"), b.kayaks.len().min(4) as f32);
            (gl.Uniform4fv)(gl.uniform_loc(prog, "uKay"), 4, kay_buf.as_ptr());
            (gl.Uniform4fv)(gl.uniform_loc(prog, "uRip"), 6, rip_buf.as_ptr());
            (gl.Uniform1f)(gl.uniform_loc(prog, "uBirdN"), b.birds.len().min(8) as f32);
            (gl.Uniform4fv)(gl.uniform_loc(prog, "uBird"), 8, bird_buf.as_ptr());
        };

        // Pass 1: Ocean, waves, wet sand, kayaks & seagulls
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_SCISSOR_TEST);
        configure_pass(self.prog_main);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Pass 2: Coconut palms along shoreline (premultiplied blend, birds fly over crowns)
        if let Some(atlas_tex) = self.palm_atlas_tex {
            if !b.palms.is_empty() {
                configure_pass(self.prog_palm);
                (gl.ActiveTexture)(GL_TEXTURE6);
                (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
                (gl.Uniform1i)(gl.uniform_loc(self.prog_palm, "uPalmAtlas"), 6);

                (gl.Enable)(GL_BLEND);
                (gl.BlendFuncSeparate)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
                (gl.BindVertexArray)(self.dummy_vao);
                for palm in &b.palms {
                    if palm.opacity < 0.003 {
                        continue;
                    }
                    (gl.Uniform4f)(
                        gl.uniform_loc(self.prog_palm, "uPalm"),
                        palm.x as f32,
                        palm.y as f32,
                        palm.scale as f32,
                        palm.variant as f32,
                    );
                    (gl.Uniform1f)(gl.uniform_loc(self.prog_palm, "uPalmOpacity"), palm.opacity as f32);
                    (gl.Uniform1f)(gl.uniform_loc(self.prog_palm, "uPalmMirror"), palm.mirror as f32);
                    (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);
                }
                (gl.Disable)(GL_BLEND);
            }
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

    fn assert_near(got: f64, exp: f64, eps: f64, label: &str) {
        assert!(
            (got - exp).abs() <= eps,
            "{label}: got {got}, expected {exp} (diff {})",
            (got - exp).abs()
        );
    }

    #[test]
    fn verify_rust_anything_recorded_beach_cases() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root");
        let rec: Value = serde_json::from_str(
            &std::fs::read_to_string(manifest_dir.join("src/beach/recorded-from.json"))
                .expect("read recorded-from"),
        )
        .expect("parse recorded-from");
        for src in rec["sources"].as_array().unwrap() {
            let rel = src["path"].as_str().unwrap();
            let exp = src["fnv1a"].as_str().unwrap();
            let bytes = std::fs::read(repo_root.join(rel)).expect("read source");
            assert_eq!(fnv1a_fingerprint(rel, &bytes), exp, "Drift in {rel}");
        }

        let raw = std::fs::read_to_string(manifest_dir.join("src/beach/cases.jsonl")).unwrap();
        let mut checked = 0usize;
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            let c: Value = serde_json::from_str(line).unwrap();
            let fname = c["fn"].as_str().unwrap();
            let inp = &c["input"];
            let out = &c["output"];

            let parse_icons = |arr: &Value| -> Vec<Rect> {
                arr.as_array()
                    .unwrap()
                    .iter()
                    .map(|r| Rect {
                        x: r["x"].as_f64().unwrap() as f32,
                        y: r["y"].as_f64().unwrap() as f32,
                        w: r["w"].as_f64().unwrap() as f32,
                        h: r["h"].as_f64().unwrap() as f32,
                    })
                    .collect()
            };

            match fname {
                "computeFieldBeach" => {
                    let mut st = BeachState::default();
                    st.p.seed = inp["seed"].as_i64().unwrap() as i32;
                    st.p.sigma = inp["sigma"].as_f64().unwrap();
                    st.p.warp = inp["warp"].as_f64().unwrap();
                    st.resize(inp["W"].as_f64().unwrap() as f32, inp["H"].as_f64().unwrap() as f32);
                    st.build_noise();
                    st.icons = parse_icons(&inp["icons"]);
                    st.refresh();
                    for (i, pr) in inp["probes"].as_array().unwrap().iter().enumerate() {
                        let got = sample_arr(&st.fb, st.gw, st.gh, pr[0].as_f64().unwrap(), pr[1].as_f64().unwrap());
                        assert_near(got, out["sampled"][i].as_f64().unwrap(), 1e-4, "beach field");
                    }
                }
                "chooseMaldivesPalms" => {
                    let icons = parse_icons(&inp["icons"]);
                    for (seed_val, key) in [(42, "palms42"), (1337, "palms1337")] {
                        let mut st = BeachState::default();
                        st.p.seed = seed_val;
                        st.p.sigma = inp["sigma"].as_f64().unwrap();
                        st.p.warp = inp["warp"].as_f64().unwrap();
                        st.resize(inp["W"].as_f64().unwrap() as f32, inp["H"].as_f64().unwrap() as f32);
                        st.build_noise();
                        st.icons = icons.clone();
                        st.refresh();
                        let got = choose_maldives_palms(&st.fb, st.gw, st.gh, st.w, st.h, seed_val, &icons);
                        let exp = out[key].as_array().unwrap();
                        assert_eq!(got.len(), exp.len(), "{key} len");
                        for (i, e) in exp.iter().enumerate() {
                            assert_eq!(got[i].variant, e["variant"].as_u64().unwrap() as usize);
                            assert_near(got[i].x, e["x"].as_f64().unwrap(), 1e-6, "palm.x");
                            assert_near(got[i].y, e["y"].as_f64().unwrap(), 1e-6, "palm.y");
                            assert_near(got[i].scale, e["scale"].as_f64().unwrap(), 1e-6, "palm.scale");
                            assert_near(got[i].mirror, e["mirror"].as_f64().unwrap(), 1e-6, "palm.mirror");
                        }
                    }
                }
                "kayakStepAndAvoidance" => {
                    let mut st = BeachState::default();
                    st.p.seed = inp["seed"].as_i64().unwrap() as i32;
                    st.resize(inp["W"].as_f64().unwrap() as f32, inp["H"].as_f64().unwrap() as f32);
                    st.build_noise();
                    st.icons = parse_icons(&inp["icons"]);
                    st.refresh();
                    st.kayaks = vec![
                        Kayak { x: 560.0, y: 420.0, h: 0.05, v: 9.0, ph: 0.3, wander: 1.2 },
                        Kayak { x: 690.0, y: 428.0, h: std::f64::consts::PI - 0.05, v: 8.5, ph: 1.9, wander: 4.7 },
                    ];
                    for _ in 0..30 {
                        st.step_kayaks(0.05);
                    }
                    let exp = out["kayaks"].as_array().unwrap();
                    assert_eq!(st.kayaks.len(), exp.len());
                    for (i, e) in exp.iter().enumerate() {
                        assert_near(st.kayaks[i].x, e["x"].as_f64().unwrap(), 1e-4, "kay.x");
                        assert_near(st.kayaks[i].y, e["y"].as_f64().unwrap(), 1e-4, "kay.y");
                        assert_near(st.kayaks[i].h, e["h"].as_f64().unwrap(), 1e-4, "kay.h");
                        assert_near(st.kayaks[i].ph, e["ph"].as_f64().unwrap(), 1e-9, "kay.ph");
                    }
                }
                "birdStepAndFlap" => {
                    let mut st = BeachState::default();
                    st.p.seed = inp["seed"].as_i64().unwrap() as i32;
                    st.resize(inp["W"].as_f64().unwrap() as f32, inp["H"].as_f64().unwrap() as f32);
                    st.birds = vec![
                        Bird { x: 400.0, y: 260.0, h: 0.4, v: 42.0, mode: 1, mode_t: 5.0, ph: 0.2, flap: true, flap_t: 5.0, wander: 2.0 },
                        Bird { x: -110.0, y: 300.0, h: std::f64::consts::PI, v: 45.0, mode: 0, mode_t: 5.0, ph: 0.0, flap: false, flap_t: 5.0, wander: 8.0 },
                    ];
                    for _ in 0..20 {
                        st.step_birds(0.04);
                    }
                    let exp = out["birds"].as_array().unwrap();
                    for (i, e) in exp.iter().enumerate() {
                        assert_near(st.birds[i].x, e["x"].as_f64().unwrap(), 1e-6, "bird.x");
                        assert_near(st.birds[i].y, e["y"].as_f64().unwrap(), 1e-6, "bird.y");
                        assert_near(st.birds[i].h, e["h"].as_f64().unwrap(), 1e-6, "bird.h");
                        assert_near(st.birds[i].ph, e["ph"].as_f64().unwrap(), 1e-6, "bird.ph");
                    }
                }
                "palmColorGrade" => {
                    let ins = inp["pixels"].as_array().unwrap();
                    let outs = out["pixels"].as_array().unwrap();
                    for (i, p) in ins.iter().enumerate() {
                        let mut px = [
                            p[0].as_u64().unwrap() as u8,
                            p[1].as_u64().unwrap() as u8,
                            p[2].as_u64().unwrap() as u8,
                            p[3].as_u64().unwrap() as u8,
                        ];
                        palm_color_grade_pixel(&mut px);
                        let e = outs[i].as_array().unwrap();
                        for ch in 0..4 {
                            assert_eq!(px[ch] as u64, e[ch].as_u64().unwrap(), "grade[{i}][{ch}]");
                        }
                    }
                }
                other => panic!("Unexpected case fn: {other}"),
            }
            checked += 1;
        }
        assert_eq!(checked, 5);
    }
}
