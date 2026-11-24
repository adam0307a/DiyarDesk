pub mod shaders;

use crate::gl::*;
use crate::quiet_snow::Mulberry32;
use crate::shaders::FULLSCREEN_VERT;
use crate::state::{AppState, Rect};
use chrono::{Local, Timelike};
use shaders::*;
use std::f32::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// Palettes & Time-of-Day Tints (1:1 prototype/papercut.html)
// =========================================================================
// PAL[palette][0..9]: 0=sky, 1=cloud, 2=city, 3..5=Bosphorus*3, 6=hill, 7=tree, 8=leaf
pub const PAL: [[[u8; 3]; 9]; 4] = [
    [
        [0x9f, 0xd8, 0xe6],
        [0xf4, 0xef, 0xe6],
        [0xe9, 0xa7, 0xa0],
        [0x3d, 0x9f, 0xb5],
        [0x2b, 0x7f, 0x9a],
        [0x1f, 0x65, 0x85],
        [0x9c, 0xc7, 0x7a],
        [0x4f, 0x8f, 0x5b],
        [0x2f, 0x6f, 0x49],
    ],
    [
        [0x8f, 0xe0, 0xd9],
        [0xff, 0xf3, 0xe0],
        [0xf5, 0xb3, 0x93],
        [0x27, 0xb3, 0xb0],
        [0x1b, 0x8f, 0x9a],
        [0x16, 0x70, 0x7f],
        [0x7f, 0xcf, 0x9a],
        [0x2e, 0x9a, 0x7a],
        [0x1c, 0x6e, 0x5c],
    ],
    [
        [0x2f, 0x4b, 0x8f],
        [0xf2, 0xe2, 0xb8],
        [0xd9, 0xa4, 0x41],
        [0x2a, 0x4f, 0x9c],
        [0x1e, 0x3b, 0x7d],
        [0x16, 0x2d, 0x63],
        [0x5a, 0x8f, 0x6a],
        [0x2f, 0x5f, 0x4f],
        [0x1f, 0x45, 0x38],
    ],
    [
        [0xcf, 0xe3, 0xee],
        [0xfd, 0xf6, 0xec],
        [0xf2, 0xc6, 0xc2],
        [0xa9, 0xcf, 0xe0],
        [0x8f, 0xbd, 0xd4],
        [0x74, 0xa7, 0xc4],
        [0xc8, 0xdf, 0xae],
        [0x9d, 0xc2, 0x9e],
        [0x7f, 0xa8, 0x8a],
    ],
];

pub const FLW: [[[u8; 3]; 4]; 4] = [
    [
        [0xe8, 0x57, 0x5f],
        [0xf2, 0xa3, 0x3a],
        [0x7b, 0x5f, 0xc9],
        [0xf7, 0xe7, 0xc4],
    ],
    [
        [0xff, 0x7a, 0x59],
        [0xff, 0xd1, 0x66],
        [0xef, 0x47, 0x6f],
        [0xff, 0xff, 0xff],
    ],
    [
        [0xf2, 0xc1, 0x4e],
        [0xe8, 0x57, 0x5f],
        [0x8a, 0xb4, 0xf8],
        [0xff, 0xf3, 0xc4],
    ],
    [
        [0xf4, 0xa6, 0xb8],
        [0xff, 0xd9, 0xa0],
        [0xb9, 0xa7, 0xe8],
        [0xff, 0xff, 0xff],
    ],
];

#[derive(Clone, Copy)]
pub struct TimeTint {
    pub c: [f32; 3],
    pub k: f32,
    pub l: f32,
}

// 1: Dawn (Seher), 2: Noon (Öğle), 3: Sunset (Akşam), 4: Night (Gece)
pub const TT: [TimeTint; 5] = [
    TimeTint { c: [255.0, 255.0, 255.0], k: 0.0, l: 1.0 },
    TimeTint { c: [0xff as f32, 0xd2 as f32, 0xa8 as f32], k: 0.14, l: 1.0 },
    TimeTint { c: [255.0, 255.0, 255.0], k: 0.0, l: 1.0 },
    TimeTint { c: [0xff as f32, 0x7a as f32, 0x3d as f32], k: 0.24, l: 0.97 },
    TimeTint { c: [0x14 as f32, 0x1c as f32, 0x4d as f32], k: 0.58, l: 0.6 },
];

pub const BX: [f32; 5] = [0.0, 0.18, 0.5, 0.78, 0.76];
pub const BY: [f32; 5] = [0.0, 0.28, 0.14, 0.34, 0.2];
pub const BC: [[u8; 3]; 5] = [
    [0, 0, 0],
    [0xff, 0xd9, 0xa0],
    [0xff, 0xe2, 0x7a],
    [0xff, 0x9a, 0x4d],
    [0xf4, 0xed, 0xd0],
];

#[inline]
pub fn mix3(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * k,
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
    ]
}

#[inline]
pub fn u8_to_f32(c: [u8; 3]) -> [f32; 3] {
    [c[0] as f32, c[1] as f32, c[2] as f32]
}

#[inline]
pub fn shade_color(c: [f32; 3], t_idx: usize) -> [f32; 3] {
    let tt = TT[t_idx.clamp(1, 4)];
    let m = mix3(c, tt.c, tt.k);
    [m[0] * tt.l, m[1] * tt.l, m[2] * tt.l]
}

#[inline]
pub fn palette_col(palette: usize, t_idx: usize, slot: usize) -> [f32; 3] {
    let rgb = PAL[palette.min(3)][slot.min(8)];
    shade_color(u8_to_f32(rgb), t_idx)
}

#[inline]
pub fn rgb255_to_rgba(c: [f32; 3], a: f32) -> [f32; 4] {
    [
        (c[0].round() / 255.0).clamp(0.0, 1.0),
        (c[1].round() / 255.0).clamp(0.0, 1.0),
        (c[2].round() / 255.0).clamp(0.0, 1.0),
        a.clamp(0.0, 1.0),
    ]
}

// =========================================================================
// Papercut Parameters & Live Simulation State (Plazas, Flowers, Critters)
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct PapercutParams {
    pub adaptive: i32,
    pub time: i32,
    pub light: f32,
    pub shadow: f32,
    pub palette: usize,
    pub grain: f32,
    pub gap: f32,
    pub flowers: f32,
    pub trees: i32,
    pub critters: usize,
    pub skyline: i32,
    pub frame: i32,
    pub sway: f32,
    pub fw: f32,
    pub fstyle: i32,
    pub quality: usize,
}

impl Default for PapercutParams {
    fn default() -> Self {
        Self {
            adaptive: 1,
            time: 0,
            light: 225.0,
            shadow: 1.0,
            palette: 0,
            grain: 0.5,
            gap: 1.0,
            flowers: 0.6,
            trees: 1,
            critters: 1,
            skyline: 1,
            frame: 1,
            sway: 1.0,
            fw: 16.0,
            fstyle: 0,
            quality: 1,
        }
    }
}

#[derive(Clone, Copy)]
pub struct IconPlaza {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub tx: f32,
    pub ty: f32,
    pub tw: f32,
    pub th: f32,
    pub k: f32,
    pub used: bool,
}

#[derive(Clone, Copy)]
pub struct FlowerSprite {
    pub x: f32,
    pub y: f32,
    pub kind: usize,      // 0=lale, 1=karanfil, 2=sümbül
    pub color_idx: usize, // 0..4
    pub sz: f32,
    #[allow(dead_code)]
    pub dw: i32,
    pub dh: i32,
    pub f: f32,
    pub ph: f32,
    pub am: f32,
}

#[derive(Clone, Copy)]
pub struct Critter {
    pub b: bool, // true = bird, false = butterfly
    pub x: f32,
    pub y: f32,
    pub s: f32,
    pub v: f32,
    pub ph: f32,
    pub c: [f32; 3],
}

pub struct PapercutState {
    pub p: PapercutParams,
    pub gw: f32,
    pub gh: f32,
    pub t_mode: usize, // resolved time of day 1..4
    pub sim_time: f32,
    pub clock_accum: f32,
    pub plazas: Vec<IconPlaza>,
    pub sprites: Vec<FlowerSprite>,
    pub crit: Vec<Critter>,
    pub cache_ok: bool,
    pub dirty: bool,
}

impl Default for PapercutState {
    fn default() -> Self {
        let mut st = Self {
            p: PapercutParams::default(),
            gw: 1920.0,
            gh: 1080.0,
            t_mode: 3,
            sim_time: 0.0,
            clock_accum: 0.0,
            plazas: Vec::new(),
            sprites: Vec::new(),
            crit: Vec::new(),
            cache_ok: false,
            dirty: true,
        };
        st.rebuild_entities();
        st
    }
}

impl PapercutState {
    pub fn resolve_time_now(&self) -> usize {
        if self.p.time > 0 {
            (self.p.time as usize).clamp(1, 4)
        } else {
            let h = Local::now().hour();
            if (5..10).contains(&h) {
                1
            } else if (10..17).contains(&h) {
                2
            } else if (17..20).contains(&h) {
                3
            } else {
                4
            }
        }
    }

    pub fn light_dir(&self) -> [f32; 2] {
        let a = self.p.light * PI / 180.0;
        [a.cos(), a.sin()]
    }

    pub fn col(&self, slot: usize) -> [f32; 3] {
        palette_col(self.p.palette, self.t_mode, slot)
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = w.max(240.0);
        let nh = h.max(160.0);
        if (self.gw - nw).abs() > 0.5 || (self.gh - nh).abs() > 0.5 {
            self.gw = nw;
            self.gh = nh;
            self.cache_ok = false;
            self.dirty = true;
            self.rebuild_entities();
        }
    }

    pub fn sync_params(&mut self, next: PapercutParams) {
        let reb_changed = self.p.time != next.time
            || (self.p.light - next.light).abs() > 1e-3
            || (self.p.shadow - next.shadow).abs() > 1e-3
            || self.p.palette != next.palette
            || (self.p.grain - next.grain).abs() > 1e-3
            || (self.p.gap - next.gap).abs() > 1e-3
            || (self.p.flowers - next.flowers).abs() > 1e-3
            || self.p.trees != next.trees
            || self.p.critters != next.critters
            || self.p.skyline != next.skyline
            || self.p.frame != next.frame
            || self.p.quality != next.quality;
        let dirty_changed = self.p.adaptive != next.adaptive
            || (self.p.fw - next.fw).abs() > 1e-3
            || self.p.fstyle != next.fstyle;

        self.p = next;
        let new_t = self.resolve_time_now();
        if new_t != self.t_mode || reb_changed {
            self.t_mode = new_t;
            self.cache_ok = false;
            self.dirty = true;
            self.rebuild_entities();
        } else if dirty_changed {
            self.dirty = true;
        }
    }

    pub fn rebuild_entities(&mut self) {
        self.mk_sprites(self.gw, self.gh);
        self.mk_crit(self.gw, self.gh);
    }

    pub fn mk_sprites(&mut self, w: f32, h: f32) {
        let mut r = Mulberry32::new(5);
        let u = h / 720.0;
        let q_mul = if self.p.quality == 0 { 0.6 } else { 1.0 };
        let n = ((w / 16.0) * self.p.flowers * q_mul).round().max(0.0) as usize;
        self.sprites.clear();
        for i in 0..n {
            let r_y = r.next_f64() as f32;
            let y = h * (0.8 + r_y * 0.18);
            let sc = 0.7 + 0.7 * (y - h * 0.78) / (h * 0.22);
            let color_idx = ((r.next_f64() * 4.0) as usize).min(3);
            let sz = (46.0 + (r.next_f64() as f32) * 24.0) * u * sc;
            let dw = (sz * 1.1).ceil() as i32;
            let dh = (sz * 1.5).ceil() as i32;
            let x = w * (0.1 + (r.next_f64() as f32) * 0.8);
            let f = 0.7 + (r.next_f64() as f32) * 0.9;
            let ph = (r.next_f64() as f32) * 6.28;
            let am = 0.03 + (r.next_f64() as f32) * 0.05;
            self.sprites.push(FlowerSprite {
                x,
                y,
                kind: i % 3,
                color_idx,
                sz,
                dw,
                dh,
                f,
                ph,
                am,
            });
        }
        self.sprites
            .sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn mk_crit(&mut self, w: f32, h: f32) {
        let mut r = Mulberry32::new(9);
        let counts = [0usize, 4, 9];
        let n = counts.get(self.p.critters).copied().unwrap_or(0);
        let fc: Vec<[f32; 3]> = FLW[self.p.palette.min(3)]
            .iter()
            .map(|&rgb| shade_color(u8_to_f32(rgb), self.t_mode))
            .collect();
        let bird_col = mix3(self.col(7), [0.0, 0.0, 30.0], 0.35);

        self.crit.clear();
        for i in 0..n {
            // In JS: const b = i < n/2 (floating point division in JS, so for n=9, i < 4.5 -> i=0,1,2,3,4!)
            let b = (i as f32) < (n as f32 / 2.0);
            let x = (r.next_f64() as f32) * w;
            let y = h
                * if b {
                    0.08 + (r.next_f64() as f32) * 0.3
                } else {
                    0.7 + (r.next_f64() as f32) * 0.22
                };
            let s = if b {
                12.0 + (r.next_f64() as f32) * 8.0
            } else {
                9.0 + (r.next_f64() as f32) * 5.0
            } * h
                / 720.0;
            let spd = if b {
                8.0 + (r.next_f64() as f32) * 14.0
            } else {
                5.0 + (r.next_f64() as f32) * 8.0
            };
            let dir = if r.next_f64() < 0.5 { -1.0 } else { 1.0 };
            let v = spd * dir;
            let ph = (r.next_f64() as f32) * 6.0;
            let c = if b { bird_col } else { fc[i % 4] };
            self.crit.push(Critter {
                b,
                x,
                y,
                s,
                v,
                ph,
                c,
            });
        }
    }

    pub fn step(&mut self, dt: f32, icons: &[Rect]) {
        self.sim_time += dt;
        if self.p.time == 0 {
            self.clock_accum += dt;
            if self.clock_accum >= 60.0 {
                self.clock_accum = 0.0;
                if self.resolve_time_now() != self.t_mode {
                    self.cache_ok = false;
                }
            }
        }
        let f = (dt * 9.0).min(1.0);
        let mut ch = false;
        for p in &mut self.plazas {
            p.used = false;
        }
        if self.p.adaptive != 0 {
            for ic in icons {
                let mut best_idx: Option<usize> = None;
                let mut bd = 8100.0f32;
                for (pi, p) in self.plazas.iter().enumerate() {
                    if p.used {
                        continue;
                    }
                    let d = (p.tx - ic.x).powi(2) + (p.ty - ic.y).powi(2);
                    if d < bd {
                        bd = d;
                        best_idx = Some(pi);
                    }
                }
                let idx = if let Some(bi) = best_idx {
                    bi
                } else {
                    self.plazas.push(IconPlaza {
                        x: ic.x,
                        y: ic.y,
                        w: ic.w,
                        h: ic.h,
                        tx: ic.x,
                        ty: ic.y,
                        tw: ic.w,
                        th: ic.h,
                        k: 0.0,
                        used: false,
                    });
                    self.plazas.len() - 1
                };
                let best = &mut self.plazas[idx];
                best.used = true;
                best.tx = ic.x;
                best.ty = ic.y;
                best.tw = ic.w;
                best.th = ic.h;
            }
        }
        let kf = (dt * 4.0).min(1.0);
        for p in &mut self.plazas {
            let o = p.x + p.y + p.w + p.h;
            let ok = p.k;
            p.x += (p.tx - p.x) * f;
            p.y += (p.ty - p.y) * f;
            p.w += (p.tw - p.w) * f;
            p.h += (p.th - p.h) * f;
            if (p.tx - p.x).abs() + (p.ty - p.y).abs() < 0.2 {
                p.x = p.tx;
                p.y = p.ty;
            }
            let target_k = if p.used { 1.0 } else { 0.0 };
            p.k += (target_k - p.k) * kf;
            if (p.x + p.y + p.w + p.h - o).abs() > 0.04 || (p.k - ok).abs() > 0.002 {
                ch = true;
            }
        }
        let n = self.plazas.len();
        self.plazas.retain(|p| p.used || p.k > 0.02);
        if self.plazas.len() != n {
            ch = true;
        }
        if ch {
            self.dirty = true;
        }
        for q in &mut self.crit {
            q.x += q.v * dt;
            if q.x > self.gw + 40.0 {
                q.x = -40.0;
            }
            if q.x < -40.0 {
                q.x = self.gw + 40.0;
            }
        }
    }

    pub fn is_near_plaza(&self, sx: f32, sy: f32, dh: f32) -> bool {
        let m = self.p.fw + 14.0;
        for p in &self.plazas {
            if p.k > 0.15
                && sx > p.x - m - 20.0
                && sx < p.x + p.w + m + 20.0
                && sy > p.y - m
                && sy - dh < p.y + p.h + m
            {
                return true;
            }
        }
        false
    }
}

// =========================================================================
// Procedural 128x128 Paper Fiber Grain Texture (1:1 mkGrain with rng(3))
// =========================================================================
pub fn build_grain_texture() -> Vec<u8> {
    let mut r = Mulberry32::new(3);
    let mut rgba = vec![0u8; 128 * 128 * 4];
    for i in (0..rgba.len()).step_by(4) {
        let v = (215.0 + r.next_f32() * 40.0).clamp(0.0, 255.0) as u8;
        rgba[i] = v;
        rgba[i + 1] = v;
        rgba[i + 2] = v;
        rgba[i + 3] = 255;
    }
    // 70 paper fiber strokes (lineWidth = 0.6)
    for _ in 0..70 {
        let is_light = r.next_f32() < 0.5;
        let (sr, sg, sb, sa) = if is_light {
            (255.0f32, 255.0, 255.0, 0.35)
        } else {
            (120.0f32, 100.0, 80.0, 0.25)
        };
        let x0 = r.next_f32() * 128.0;
        let y0 = r.next_f32() * 128.0;
        let a = r.next_f32() * 6.28;
        let l = 4.0 + r.next_f32() * 10.0;
        let x1 = x0 + a.cos() * l;
        let y1 = y0 + a.sin() * l;

        let min_x = (x0.min(x1) - 1.0).floor().max(0.0) as usize;
        let max_x = (x0.max(x1) + 1.0).ceil().min(127.0) as usize;
        let min_y = (y0.min(y1) - 1.0).floor().max(0.0) as usize;
        let max_y = (y0.max(y1) + 1.0).ceil().min(127.0) as usize;
        let dx = x1 - x0;
        let dy = y1 - y0;
        let l2 = (dx * dx + dy * dy).max(1e-5);
        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let fx = px as f32 + 0.5;
                let fy = py as f32 + 0.5;
                let t = (((fx - x0) * dx + (fy - y0) * dy) / l2).clamp(0.0, 1.0);
                let dist = ((fx - (x0 + t * dx)).powi(2) + (fy - (y0 + t * dy)).powi(2)).sqrt();
                let cov = (0.8 - dist).clamp(0.0, 1.0) * sa;
                if cov > 0.0 {
                    let idx = (py * 128 + px) * 4;
                    rgba[idx] = (rgba[idx] as f32 * (1.0 - cov) + sr * cov).round() as u8;
                    rgba[idx + 1] = (rgba[idx + 1] as f32 * (1.0 - cov) + sg * cov).round() as u8;
                    rgba[idx + 2] = (rgba[idx + 2] as f32 * (1.0 - cov) + sb * cov).round() as u8;
                }
            }
        }
    }
    rgba
}

// =========================================================================
// OpenGL 3.3 Core Renderer for Kâğıt Bahçe (Shadow-Box FBO Layer Compositor)
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

struct ShapeBatch {
    verts: Vec<ColorVert>,
}

impl ShapeBatch {
    fn new() -> Self {
        Self {
            verts: Vec::with_capacity(16384),
        }
    }

    fn clear(&mut self) {
        self.verts.clear();
    }

    #[inline]
    fn push_tri(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], col: [f32; 4]) {
        self.verts.push(ColorVert { pos: a, col });
        self.verts.push(ColorVert { pos: b, col });
        self.verts.push(ColorVert { pos: c, col });
    }

    #[inline]
    fn push_quad(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2], col: [f32; 4]) {
        self.push_tri(a, b, c, col);
        self.push_tri(a, c, d, col);
    }

    fn add_rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        self.push_quad([x, y], [x + w, y], [x + w, y + h], [x, y + h], col);
    }

    fn add_rect_vgrad(&mut self, x: f32, y: f32, w: f32, h: f32, c_top: [f32; 4], c_bot: [f32; 4]) {
        let a = ColorVert { pos: [x, y], col: c_top };
        let b = ColorVert { pos: [x + w, y], col: c_top };
        let c = ColorVert { pos: [x + w, y + h], col: c_bot };
        let d = ColorVert { pos: [x, y + h], col: c_bot };
        self.verts.push(a);
        self.verts.push(b);
        self.verts.push(c);
        self.verts.push(a);
        self.verts.push(c);
        self.verts.push(d);
    }

    fn add_circle(&mut self, cx: f32, cy: f32, r: f32, segs: usize, col: [f32; 4]) {
        if r <= 0.05 {
            return;
        }
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            self.push_tri(
                [cx, cy],
                [cx + a0.cos() * r, cy + a0.sin() * r],
                [cx + a1.cos() * r, cy + a1.sin() * r],
                col,
            );
        }
    }

    // Upper semicircle dome: arc(x, y, r, PI, 2*PI)
    fn add_dome(&mut self, cx: f32, cy: f32, r: f32, segs: usize, col: [f32; 4]) {
        if r <= 0.05 {
            return;
        }
        for i in 0..segs {
            let a0 = PI + (i as f32 / segs as f32) * PI;
            let a1 = PI + ((i + 1) as f32 / segs as f32) * PI;
            self.push_tri(
                [cx, cy],
                [cx + a0.cos() * r, cy + a0.sin() * r],
                [cx + a1.cos() * r, cy + a1.sin() * r],
                col,
            );
        }
    }

    fn add_rotated_ellipse(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rot: f32,
        segs: usize,
        col: [f32; 4],
    ) {
        if rx <= 0.05 || ry <= 0.05 {
            return;
        }
        let (sr, cr) = rot.sin_cos();
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            let lx0 = a0.cos() * rx;
            let ly0 = a0.sin() * ry;
            let lx1 = a1.cos() * rx;
            let ly1 = a1.sin() * ry;
            self.push_tri(
                [cx, cy],
                [cx + lx0 * cr - ly0 * sr, cy + lx0 * sr + ly0 * cr],
                [cx + lx1 * cr - ly1 * sr, cy + lx1 * sr + ly1 * cr],
                col,
            );
        }
    }

    // 16-point Ottoman star: star(c, x, y, r)
    fn add_star(&mut self, cx: f32, cy: f32, r: f32, col: [f32; 4]) {
        for i in 0..16 {
            let a0 = (i as f32) * PI / 8.0 - PI * 0.5;
            let a1 = ((i + 1) as f32) * PI / 8.0 - PI * 0.5;
            let q0 = if i % 2 == 1 { r * 0.45 } else { r };
            let q1 = if (i + 1) % 2 == 1 { r * 0.45 } else { r };
            self.push_tri(
                [cx, cy],
                [cx + a0.cos() * q0, cy + a0.sin() * q0],
                [cx + a1.cos() * q1, cy + a1.sin() * q1],
                col,
            );
        }
    }

    // mina(c, x, y, hh, w)
    fn add_minaret(&mut self, x: f32, y: f32, hh: f32, w: f32, col: [f32; 4]) {
        self.add_rect(x - w * 0.5, y - hh, w, hh, col);
        self.add_rect(x - w * 1.3, y - hh * 0.7, w * 2.6, w * 0.55, col);
        self.push_tri(
            [x - w * 0.75, y - hh],
            [x, y - hh - w * 3.4],
            [x + w * 0.75, y - hh],
            col,
        );
    }

    // mosque(c, x, y, r)
    fn add_mosque(&mut self, x: f32, y: f32, r: f32, col: [f32; 4]) {
        let bh = r * 0.95;
        self.add_rect(x - r * 1.7, y - bh, r * 3.4, bh, col);
        self.add_dome(x, y - bh, r, 24, col);
        self.add_dome(x - r * 1.15, y - bh, r * 0.62, 18, col);
        self.add_dome(x + r * 1.15, y - bh, r * 0.62, 18, col);
        self.add_minaret(x - r * 2.1, y, r * 3.3, r * 0.2, col);
        self.add_minaret(x + r * 2.1, y, r * 3.3, r * 0.2, col);
    }

    // Rounded arch rectangle rr(c, x, y, w, h, a_top, b_bot)
    fn add_arch_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r_top: f32,
        r_bot: f32,
        col: [f32; 4],
    ) {
        if w <= 0.1 || h <= 0.1 {
            return;
        }
        let rt = r_top.min(w * 0.5).min(h * 0.5).max(0.0);
        let rb = r_bot.min(w * 0.5).min(h * 0.5).max(0.0);
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;

        let mut poly = Vec::with_capacity(56);
        let segs = 12usize;
        // Top-right corner: (x + w - rt, y + rt), angle -PI/2 .. 0
        for i in 0..=segs {
            let ang = -PI * 0.5 + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + w - rt + ang.cos() * rt, y + rt + ang.sin() * rt]);
        }
        // Bottom-right corner: (x + w - rb, y + h - rb), angle 0 .. PI/2
        for i in 0..=segs {
            let ang = (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + w - rb + ang.cos() * rb, y + h - rb + ang.sin() * rb]);
        }
        // Bottom-left corner: (x + rb, y + h - rb), angle PI/2 .. PI
        for i in 0..=segs {
            let ang = PI * 0.5 + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + rb + ang.cos() * rb, y + h - rb + ang.sin() * rb]);
        }
        // Top-left corner: (x + rt, y + rt), angle PI .. 3*PI/2
        for i in 0..=segs {
            let ang = PI + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + rt + ang.cos() * rt, y + rt + ang.sin() * rt]);
        }

        for i in 0..poly.len() {
            let p0 = poly[i];
            let p1 = poly[(i + 1) % poly.len()];
            self.push_tri([cx, cy], p0, p1, col);
        }
    }

    // Two-sided symmetric or asymmetric quadratic leaf/cypress/reed blade from base to tip
    #[allow(clippy::too_many_arguments)]
    fn add_double_quad_blade(
        &mut self,
        p0_left: [f32; 2],
        ctrl_left: [f32; 2],
        tip: [f32; 2],
        ctrl_right: [f32; 2],
        p0_right: [f32; 2],
        steps: usize,
        col: [f32; 4],
    ) {
        let eval_q = |a: [f32; 2], c: [f32; 2], b: [f32; 2], t: f32| -> [f32; 2] {
            let it = 1.0 - t;
            [
                it * it * a[0] + 2.0 * it * t * c[0] + t * t * b[0],
                it * it * a[1] + 2.0 * it * t * c[1] + t * t * b[1],
            ]
        };
        for i in 0..steps {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            let l0 = eval_q(p0_left, ctrl_left, tip, t0);
            let l1 = eval_q(p0_left, ctrl_left, tip, t1);
            let r0 = eval_q(p0_right, ctrl_right, tip, t0);
            let r1 = eval_q(p0_right, ctrl_right, tip, t1);
            self.push_quad(l0, r0, r1, l1, col);
        }
    }
}

struct FboLayer {
    fbo: GLuint,
    tex: GLuint,
    w: i32,
    h: i32,
}

impl FboLayer {
    fn empty() -> Self {
        Self {
            fbo: 0,
            tex: 0,
            w: 0,
            h: 0,
        }
    }

    unsafe fn ensure(&mut self, gl: &GlFns, w: i32, h: i32) {
        let w = w.max(16);
        let h = h.max(16);
        if self.fbo != 0 && self.w == w && self.h == h {
            return;
        }
        self.destroy(gl);
        (gl.GenFramebuffers)(1, &mut self.fbo);
        (gl.GenTextures)(1, &mut self.tex);
        (gl.BindTexture)(GL_TEXTURE_2D, self.tex);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            w,
            h,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            std::ptr::null(),
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.fbo);
        (gl.FramebufferTexture2D)(
            GL_FRAMEBUFFER,
            GL_COLOR_ATTACHMENT0,
            GL_TEXTURE_2D,
            self.tex,
            0,
        );
        self.w = w;
        self.h = h;
    }

    unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.tex != 0 {
            (gl.DeleteTextures)(1, &self.tex);
            self.tex = 0;
        }
        if self.fbo != 0 {
            (gl.DeleteFramebuffers)(1, &self.fbo);
            self.fbo = 0;
        }
        self.w = 0;
        self.h = 0;
    }
}

const FLW_CELL_W: i32 = 128;
const FLW_CELL_H: i32 = 192;
const FLW_ATLAS_W: i32 = FLW_CELL_W * 4; // 4 colors
const FLW_ATLAS_H: i32 = FLW_CELL_H * 3; // 3 species (lale, karanfil, sümbül)
const FLW_REF_SZ: f32 = 72.0;

pub struct PapercutRenderer {
    prog_color: GLuint,
    prog_paper: GLuint,
    prog_niche_hole: GLuint,
    prog_post: GLuint,
    prog_sprite: GLuint,
    dummy_vao: GLuint,

    grain_tex: GLuint,

    back_fbo: FboLayer,
    fg_fbo: FboLayer,
    tree_fbo: FboLayer,
    mask_fbo: FboLayer,
    raw_comp_fbo: FboLayer,
    comp_fbo: FboLayer,
    flower_atlas: FboLayer,
    flower_mask: FboLayer,

    color_vao: GLuint,
    color_vbo: GLuint,
    batch: ShapeBatch,

    sprite_vao: GLuint,
    sprite_vbo: GLuint,
    sprite_verts: Vec<SpriteVert>,

    last_w: i32,
    last_h: i32,
}

impl PapercutRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_color = gl.compile_program(PC_COLOR_VERT, PC_COLOR_FRAG)?;
        let prog_paper = gl.compile_program(FULLSCREEN_VERT, PC_PAPER_APPLY_FRAG)?;
        let prog_niche_hole = gl.compile_program(FULLSCREEN_VERT, PC_NICHE_HOLE_FRAG)?;
        let prog_post = gl.compile_program(FULLSCREEN_VERT, PC_POST_GRAIN_VIG_FRAG)?;
        let prog_sprite = gl.compile_program(PC_SPRITE_VERT, PC_SPRITE_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let grain_bytes = build_grain_texture();
        let mut grain_tex = 0;
        (gl.GenTextures)(1, &mut grain_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, grain_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            128,
            128,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            grain_bytes.as_ptr() as *const c_void,
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        // GL_REPEAT = 0x2901 for seamless 128x128 paper fiber grain tiling
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, 0x2901);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, 0x2901);

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
            prog_color,
            prog_paper,
            prog_niche_hole,
            prog_post,
            prog_sprite,
            dummy_vao,
            grain_tex,
            back_fbo: FboLayer::empty(),
            fg_fbo: FboLayer::empty(),
            tree_fbo: FboLayer::empty(),
            mask_fbo: FboLayer::empty(),
            raw_comp_fbo: FboLayer::empty(),
            comp_fbo: FboLayer::empty(),
            flower_atlas: FboLayer::empty(),
            flower_mask: FboLayer::empty(),
            color_vao,
            color_vbo,
            batch: ShapeBatch::new(),
            sprite_vao,
            sprite_vbo,
            sprite_verts: Vec::with_capacity(8192),
            last_w: 0,
            last_h: 0,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        self.back_fbo.destroy(gl);
        self.fg_fbo.destroy(gl);
        self.tree_fbo.destroy(gl);
        self.mask_fbo.destroy(gl);
        self.raw_comp_fbo.destroy(gl);
        self.comp_fbo.destroy(gl);
        self.flower_atlas.destroy(gl);
        self.flower_mask.destroy(gl);
        if self.grain_tex != 0 {
            (gl.DeleteTextures)(1, &self.grain_tex);
            self.grain_tex = 0;
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
        for p in [
            self.prog_color,
            self.prog_paper,
            self.prog_niche_hole,
            self.prog_post,
            self.prog_sprite,
        ] {
            if p != 0 {
                (gl.DeleteProgram)(p);
            }
        }
    }

    unsafe fn flush_batch(&mut self, gl: &GlFns, res_w: f32, res_h: f32) {
        if self.batch.verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_color);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_color, "u_res"), res_w, res_h);
        (gl.BindVertexArray)(self.color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.color_vbo);
        let bytes = (self.batch.verts.len() * std::mem::size_of::<ColorVert>()) as GLsizeiptr;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            bytes,
            self.batch.verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.batch.verts.len() as GLsizei);
        self.batch.clear();
    }

    // Renders the geometry currently in self.batch into mask_fbo, then composites it onto target_fbo
    // with 1:1 pp(c, color, dep, f) 2-layer shadows + white cut-paper bevel rim!
    #[allow(clippy::too_many_arguments)]
    unsafe fn apply_paper_piece(
        &mut self,
        gl: &GlFns,
        target_fbo: GLuint,
        mask_fbo: GLuint,
        mask_tex: GLuint,
        vp_w: i32,
        vp_h: i32,
        css_w: f32,
        css_h: f32,
        color_255: [f32; 3],
        dep: f32,
        pc: &PapercutState,
        rim_alpha: f32,
    ) {
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, mask_fbo);
        (gl.Viewport)(0, 0, vp_w, vp_h);
        (gl.Disable)(GL_BLEND);
        (gl.ClearColor)(0.0, 0.0, 0.0, 0.0);
        (gl.Clear)(GL_COLOR_BUFFER_BIT);
        self.flush_batch(gl, css_w, css_h);

        let l = pc.light_dir();
        let gap = pc.p.gap;
        let sh_mul = pc.p.shadow;
        let off1 = [-l[0] * dep * 0.45 * gap, -l[1] * dep * 0.45 * gap];
        let blur1 = 1.5 * gap;
        let a1 = 0.9f32.min(0.32 * sh_mul);

        let off2 = [-l[0] * dep * 1.0 * gap, -l[1] * dep * 1.0 * gap];
        let blur2 = (dep * 1.2 + 2.0) * gap;
        let a2 = 0.9f32.min(0.20 * sh_mul);

        self.composite_mask_to_target(
            gl,
            target_fbo,
            mask_tex,
            vp_w,
            vp_h,
            css_w,
            css_h,
            rgb255_to_rgba(color_255, 1.0),
            off1,
            blur1,
            a1,
            off2,
            blur2,
            a2,
            rim_alpha,
        );
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn composite_mask_to_target(
        &self,
        gl: &GlFns,
        target_fbo: GLuint,
        mask_tex: GLuint,
        vp_w: i32,
        vp_h: i32,
        css_w: f32,
        css_h: f32,
        col: [f32; 4],
        off1: [f32; 2],
        blur1: f32,
        a1: f32,
        off2: [f32; 2],
        blur2: f32,
        a2: f32,
        rim_a: f32,
    ) {
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, target_fbo);
        (gl.Viewport)(0, 0, vp_w, vp_h);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
        (gl.UseProgram)(self.prog_paper);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, mask_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_paper, "u_mask"), 0);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_paper, "u_res"), css_w, css_h);
        (gl.Uniform4f)(
            gl.uniform_loc(self.prog_paper, "u_color"),
            col[0],
            col[1],
            col[2],
            col[3],
        );
        (gl.Uniform2f)(gl.uniform_loc(self.prog_paper, "u_sh_off1"), off1[0], off1[1]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_paper, "u_sh_blur1"), blur1);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_paper, "u_sh_a1"), a1);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_paper, "u_sh_off2"), off2[0], off2[1]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_paper, "u_sh_blur2"), blur2);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_paper, "u_sh_a2"), a2);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_paper, "u_rim_a"), rim_a);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
    }

    // 1:1 buildBack(c, w, h)
    unsafe fn build_back(&mut self, gl: &GlFns, pw: i32, ph: i32, pc: &PapercutState) {
        let w = pc.gw;
        let h = pc.gh;
        let u = h / 720.0;
        let t_idx = pc.t_mode.clamp(1, 4);
        let mut r = Mulberry32::new(11);
        let white = [1.0, 1.0, 1.0, 1.0];

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.back_fbo.fbo);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        // 1. Sky base color + vertical linear gradient (0..h*0.62)
        let sk = pc.col(0);
        let sk_rgba = rgb255_to_rgba(sk, 1.0);
        self.batch.clear();
        self.batch.add_rect(0.0, 0.0, w, h, sk_rgba);
        self.batch.add_rect_vgrad(
            0.0,
            0.0,
            w,
            h * 0.62,
            [0.0, 0.0, 30.0 / 255.0, 0.12],
            [1.0, 1.0, 1.0, 0.16],
        );
        self.batch.add_rect(0.0, h * 0.62, w, h * 0.38, [1.0, 1.0, 1.0, 0.16]);

        // 2. 16-pointed Ottoman stars (ns = T===4 ? 70 : T===1 ? 10 : 0)
        let ns = if t_idx == 4 {
            70
        } else if t_idx == 1 {
            10
        } else {
            0
        };
        let star_col = [1.0, 244.0 / 255.0, 200.0 / 255.0, 0.95];
        for _ in 0..ns {
            let sx = (r.next_f64() as f32) * w;
            let sy = (r.next_f64() as f32) * h * 0.5;
            let sr = (1.5 + (r.next_f64() as f32) * 2.8) * u;
            self.batch.add_star(sx, sy, sr, star_col);
        }
        self.flush_batch(gl, w, h);

        // 3. 3 concentric cut-paper sun/moon discs: [[2.1, 0.2], [1.5, 0.5], [1.0, 1.0]]
        let bc = u8_to_f32(BC[t_idx]);
        let sun_r = 34.0 * u;
        let sun_x = w * BX[t_idx];
        let sun_y = h * BY[t_idx];
        for (i, &(m, k)) in [(2.1f32, 0.2f32), (1.5, 0.5), (1.0, 1.0)].iter().enumerate() {
            self.batch.clear();
            self.batch.add_circle(sun_x, sun_y, sun_r * m, 36, white);
            self.apply_paper_piece(
                gl,
                self.back_fbo.fbo,
                self.mask_fbo.fbo,
                self.mask_fbo.tex,
                pw,
                ph,
                w,
                h,
                mix3(sk, bc, k),
                2.0 + i as f32,
                pc,
                0.18,
            );
        }

        // 4. 7 multi-puff cut-paper clouds
        let c1 = pc.col(1);
        for i in 0..7 {
            let far = (i % 2) == 1;
            let cx = (0.08 + i as f32 * 0.14 + (r.next_f64() as f32) * 0.05) * w;
            let cy = h * (0.13 + (if far { 0.12 } else { 0.0 }) + (r.next_f64() as f32) * 0.1);
            let s = (24.0 + (r.next_f64() as f32) * 20.0) * u * if far { 0.7 } else { 1.0 };
            let cloud_col = mix3(c1, sk, if far { 0.35 } else { 0.0 });
            let dep = if far { 3.0 } else { 5.0 };

            self.batch.clear();
            for j in 0..5 {
                let px = cx + (j as f32 - 2.0) * s * 0.8;
                let pr = s * (0.55 + 0.35 * (j as f32 * 1.9 + i as f32).sin());
                self.batch.add_circle(px, cy, pr, 24, white);
            }
            self.apply_paper_piece(
                gl,
                self.back_fbo.fbo,
                self.mask_fbo.fbo,
                self.mask_fbo.tex,
                pw,
                ph,
                w,
                h,
                cloud_col,
                dep,
                pc,
                0.18,
            );
        }

        // 5. Cut-paper Istanbul skyline: houses, 2 mosques, Galata Tower & glowing night windows
        if pc.p.skyline != 0 {
            let y0 = h * 0.63;
            let mut hs: Vec<(f32, f32, f32, bool)> = Vec::new();
            let mut x = -20.0f32;
            while x < w + 20.0 {
                let bw = (14.0 + (r.next_f64() as f32) * 26.0) * u;
                let bh = (10.0 + (r.next_f64() as f32) * 34.0) * u;
                let rf = r.next_f64() < 0.45;
                hs.push((x, bw, bh, rf));
                x += bw * (0.7 + (r.next_f64() as f32) * 0.5);
            }
            self.batch.clear();
            for &(hx, bw, bh, rf) in &hs {
                self.batch.add_rect(hx, y0 - bh, bw, bh, white);
                if rf {
                    self.batch.push_tri(
                        [hx - 2.0 * u, y0 - bh],
                        [hx + bw * 0.5, y0 - bh - bw * 0.35],
                        [hx + bw + 2.0 * u, y0 - bh],
                        white,
                    );
                }
            }
            self.batch.add_mosque(w * 0.27, y0, 21.0 * u, white);
            self.batch.add_mosque(w * 0.70, y0, 17.0 * u, white);
            // Galata Tower at w * 0.5
            let gx = w * 0.5;
            self.batch
                .add_rect(gx - 7.0 * u, y0 - 118.0 * u, 14.0 * u, 118.0 * u, white);
            self.batch
                .add_rect(gx - 10.0 * u, y0 - 124.0 * u, 20.0 * u, 7.0 * u, white);
            self.batch.push_tri(
                [gx - 10.0 * u, y0 - 124.0 * u],
                [gx, y0 - 152.0 * u],
                [gx + 10.0 * u, y0 - 124.0 * u],
                white,
            );
            self.apply_paper_piece(
                gl,
                self.back_fbo.fbo,
                self.mask_fbo.fbo,
                self.mask_fbo.tex,
                pw,
                ph,
                w,
                h,
                pc.col(2),
                5.0,
                pc,
                0.18,
            );

            // Glowing golden windows at Sunset & Night (T >= 3)
            if t_idx >= 3 {
                let mut r2 = Mulberry32::new(4);
                let win_col = [1.0, 208.0 / 255.0, 110.0 / 255.0, 0.92];
                self.batch.clear();
                for &(hx, bw, bh, _) in &hs {
                    let cnt = 1 + ((r2.next_f64() * 3.0) as usize);
                    for _ in 0..cnt {
                        let wx = hx + (r2.next_f64() as f32) * (bw - 3.0 * u);
                        let wy = y0 - bh * (0.2 + (r2.next_f64() as f32) * 0.7);
                        self.batch.add_rect(wx, wy, 2.4 * u, 3.2 * u, win_col);
                    }
                }
                (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.back_fbo.fbo);
                (gl.Viewport)(0, 0, pw, ph);
                (gl.Enable)(GL_BLEND);
                (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
                self.flush_batch(gl, w, h);
            }
        }

        // 6. 3 layered Bosphorus water wave strips
        for i in 0..3 {
            let fi = i as f32;
            let y0 = h * (0.6 + fi * 0.055);
            let a = (5.0 + fi * 3.0) * u;
            let f = 0.006 - fi * 0.0011;
            let ph_w = fi * 1.7;
            self.batch.clear();
            let eval_y = |x: f32| -> f32 {
                y0 + a * (x * f + ph_w).sin() + a * 0.5 * (x * f * 2.3 + ph_w * 2.0).sin()
            };
            let mut prev_x = -10.0f32;
            let mut prev_y = eval_y(prev_x);
            let mut x = 0.0f32;
            while x <= w + 10.0 {
                let cur_y = eval_y(x);
                self.batch.push_quad(
                    [prev_x, prev_y],
                    [x, cur_y],
                    [x, h],
                    [prev_x, h],
                    white,
                );
                prev_x = x;
                prev_y = cur_y;
                x += 10.0;
            }
            self.apply_paper_piece(
                gl,
                self.back_fbo.fbo,
                self.mask_fbo.fbo,
                self.mask_fbo.tex,
                pw,
                ph,
                w,
                h,
                pc.col(3 + i),
                4.0 + fi * 1.5,
                pc,
                0.18,
            );
        }
    }

    // 1:1 buildFg(c, w, h)
    unsafe fn build_fg(&mut self, gl: &GlFns, pw: i32, ph: i32, pc: &PapercutState) {
        let w = pc.gw;
        let h = pc.gh;
        let u = h / 720.0;
        let mut r = Mulberry32::new(21);
        let white = [1.0, 1.0, 1.0, 1.0];

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.fg_fbo.fbo);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.ClearColor)(0.0, 0.0, 0.0, 0.0);
        (gl.Clear)(GL_COLOR_BUFFER_BIT);

        let add_hill = |batch: &mut ShapeBatch, y0: f32, a: f32, ph_h: f32| {
            batch.clear();
            let eval_y = |x: f32| -> f32 {
                y0 + a * (x * 0.004 + ph_h).sin() + a * 0.4 * (x * 0.011 + ph_h * 2.0).sin()
            };
            let mut prev_x = -10.0f32;
            let mut prev_y = eval_y(prev_x);
            let mut x = 0.0f32;
            while x <= w + 10.0 {
                let cur_y = eval_y(x);
                batch.push_quad(
                    [prev_x, prev_y],
                    [x, cur_y],
                    [x, h + 10.0],
                    [prev_x, h + 10.0],
                    white,
                );
                prev_x = x;
                prev_y = cur_y;
                x += 10.0;
            }
        };

        // 1. Midground green hill (h * 0.76, 8*u, 0.5, col(6), dep=6)
        add_hill(&mut self.batch, h * 0.76, 8.0 * u, 0.5);
        self.apply_paper_piece(
            gl,
            self.fg_fbo.fbo,
            self.mask_fbo.fbo,
            self.mask_fbo.tex,
            pw,
            ph,
            w,
            h,
            pc.col(6),
            6.0,
            pc,
            0.18,
        );

        // 2. Cypress trees (selvi) & plane tree (çınar) with punched leaf holes (destination-out)
        if pc.p.trees != 0 {
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.tree_fbo.fbo);
            (gl.Viewport)(0, 0, pw, ph);
            (gl.ClearColor)(0.0, 0.0, 0.0, 0.0);
            (gl.Clear)(GL_COLOR_BUFFER_BIT);

            let yb = h * 0.84;
            let tcol = pc.col(7);
            let mut holes: Vec<(f32, f32, f32, f32, f32)> = Vec::with_capacity(120);

            for &(xr, ht_r, wd_r) in &[
                (0.045f32, 0.52f32, 20.0f32),
                (0.10, 0.40, 16.0),
                (0.935, 0.48, 19.0),
                (0.975, 0.36, 15.0),
            ] {
                let cx = w * xr;
                let ht = h * ht_r;
                let wd = wd_r * u;
                self.batch.clear();
                self.batch.add_double_quad_blade(
                    [cx - wd, yb],
                    [cx - wd * 1.15, yb - ht * 0.55],
                    [cx, yb - ht],
                    [cx + wd * 1.15, yb - ht * 0.55],
                    [cx + wd, yb],
                    24,
                    white,
                );
                self.apply_paper_piece(
                    gl,
                    self.tree_fbo.fbo,
                    self.mask_fbo.fbo,
                    self.mask_fbo.tex,
                    pw,
                    ph,
                    w,
                    h,
                    tcol,
                    6.0,
                    pc,
                    0.18,
                );
                for _ in 0..22 {
                    let py = yb - ht * (0.12 + 0.78 * (r.next_f64() as f32));
                    let hw = wd * 0.8 * (1.0 - ((yb - py) / ht).powi(2));
                    let hx = cx + ((r.next_f64() as f32) * 2.0 - 1.0) * hw * 0.65;
                    let ro = (r.next_f64() as f32) * 3.0;
                    holes.push((hx, py, 2.2 * u, 5.0 * u, ro));
                }
            }

            // Plane tree (çınar) at cx = w * 0.84, cy = h * 0.46, cr = h * 0.1
            let cx = w * 0.84;
            let cy = h * 0.46;
            let cr = h * 0.1;
            self.batch.clear();
            self.batch.add_rect(cx - 8.0 * u, cy, 16.0 * u, yb - cy, white);
            for j in 0..7 {
                let a = (j as f32) * 0.9;
                let px = cx + a.cos() * cr * 0.55;
                let py = cy - cr * 0.1 + a.sin() * cr * 0.45;
                let pr = cr * (0.55 + 0.15 * ((j % 3) as f32) / 2.0);
                self.batch.add_circle(px, py, pr, 28, white);
            }
            self.apply_paper_piece(
                gl,
                self.tree_fbo.fbo,
                self.mask_fbo.fbo,
                self.mask_fbo.tex,
                pw,
                ph,
                w,
                h,
                mix3(tcol, pc.col(6), 0.25),
                7.0,
                pc,
                0.18,
            );
            for _ in 0..30 {
                let a = (r.next_f64() as f32) * 6.28;
                let d = (r.next_f64() as f32).sqrt() * cr * 0.85;
                let hx = cx + a.cos() * d;
                let hy = cy - cr * 0.1 + a.sin() * d * 0.8;
                let ro = (r.next_f64() as f32) * 3.0;
                holes.push((hx, hy, 3.0 * u, 6.0 * u, ro));
            }

            // Punch leaf holes out of tree_fbo using destination-out (clear alpha & color to 0)
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.tree_fbo.fbo);
            (gl.Viewport)(0, 0, pw, ph);
            (gl.Disable)(GL_BLEND);
            self.batch.clear();
            for &(hx, hy, rx, ry, ro) in &holes {
                self.batch
                    .add_rotated_ellipse(hx, hy, rx, ry, ro, 12, [0.0, 0.0, 0.0, 0.0]);
            }
            self.flush_batch(gl, w, h);

            // Blit premultiplied tree_fbo over fg_fbo with exact Porter-Duff source-over
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.fg_fbo.fbo);
            (gl.Viewport)(0, 0, pw, ph);
            (gl.Enable)(GL_BLEND);
            (gl.BlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            self.sprite_verts.clear();
            let c_w = [1.0, 1.0, 1.0, 1.0];
            self.sprite_verts.extend_from_slice(&[
                SpriteVert { pos: [0.0, 0.0], uv: [0.0, 1.0], col: c_w },
                SpriteVert { pos: [w, 0.0], uv: [1.0, 1.0], col: c_w },
                SpriteVert { pos: [w, h], uv: [1.0, 0.0], col: c_w },
                SpriteVert { pos: [0.0, 0.0], uv: [0.0, 1.0], col: c_w },
                SpriteVert { pos: [w, h], uv: [1.0, 0.0], col: c_w },
                SpriteVert { pos: [0.0, h], uv: [0.0, 0.0], col: c_w },
            ]);
            self.flush_sprites(gl, self.tree_fbo.tex, w, h);
        }

        // 3. Foreground dark hill (h * 0.88, 6*u, 2.1, col(8), dep=8)
        add_hill(&mut self.batch, h * 0.88, 6.0 * u, 2.1);
        self.apply_paper_piece(
            gl,
            self.fg_fbo.fbo,
            self.mask_fbo.fbo,
            self.mask_fbo.tex,
            pw,
            ph,
            w,
            h,
            pc.col(8),
            8.0,
            pc,
            0.18,
        );

        // 4. 10 curved reed blades (saz yaprakları, side = 0..2, i = 0..5)
        for side in 0..2 {
            for i in 0..5 {
                let fi = i as f32;
                let bx = if side == 1 {
                    w - (10.0 + fi * 20.0) * u
                } else {
                    (10.0 + fi * 20.0) * u
                };
                let by = h + 8.0;
                let len = (90.0 + fi * 28.0 + (r.next_f64() as f32) * 30.0) * u;
                let dir = (if side == 1 { -1.0 } else { 1.0 }) * (0.15 + fi * 0.14);
                let tx = bx + dir * len;
                let ty = by - len * 0.9;
                let wd = 13.0 * u;
                let bend = (if side == 1 { 1.0 } else { -1.0 }) * 20.0 * u;
                let blade_col = mix3(pc.col(8), pc.col(6), ((i % 2) as f32) * 0.4);

                self.batch.clear();
                self.batch.add_double_quad_blade(
                    [bx - wd, by],
                    [bx - wd + (tx - bx) * 0.2 + bend, by + (ty - by) * 0.55],
                    [tx, ty],
                    [
                        bx + wd + (tx - bx) * 0.2 + bend * 0.3,
                        by + (ty - by) * 0.55,
                    ],
                    [bx + wd, by],
                    20,
                    white,
                );
                self.apply_paper_piece(
                    gl,
                    self.fg_fbo.fbo,
                    self.mask_fbo.fbo,
                    self.mask_fbo.tex,
                    pw,
                    ph,
                    w,
                    h,
                    blade_col,
                    9.0,
                    pc,
                    0.18,
                );
            }
        }

        // 5. Outer shadow-box mat frame (if P.frame, even-odd cutout)
        if pc.p.frame != 0 {
            let fb = 10.0f32.max(w.min(h) * 0.022);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.mask_fbo.fbo);
            (gl.Viewport)(0, 0, pw, ph);
            (gl.Disable)(GL_BLEND);
            (gl.ClearColor)(1.0, 1.0, 1.0, 1.0);
            (gl.Clear)(GL_COLOR_BUFFER_BIT);

            // Cut out the inner rounded rectangle with alpha = 0
            self.batch.clear();
            self.batch.add_arch_rect(
                fb,
                fb,
                w - 2.0 * fb,
                h - 2.0 * fb,
                fb * 0.9,
                fb * 0.9,
                [0.0, 0.0, 0.0, 0.0],
            );
            self.flush_batch(gl, w, h);

            let l = pc.light_dir();
            let gap = pc.p.gap;
            let sh_mul = pc.p.shadow;
            let dep = 7.0f32;
            let frame_col = mix3(pc.col(1), [255.0, 255.0, 255.0], 0.2);
            self.composite_mask_to_target(
                gl,
                self.fg_fbo.fbo,
                self.mask_fbo.tex,
                pw,
                ph,
                w,
                h,
                rgb255_to_rgba(frame_col, 1.0),
                [-l[0] * dep * 0.45 * gap, -l[1] * dep * 0.45 * gap],
                1.5 * gap,
                0.9f32.min(0.32 * sh_mul),
                [-l[0] * dep * 1.0 * gap, -l[1] * dep * 1.0 * gap],
                (dep * 1.2 + 2.0) * gap,
                0.9f32.min(0.20 * sh_mul),
                0.18,
            );
        }
    }

    // Pre-bakes all 12 flower sprite variations (3 types x 4 colors) into self.flower_atlas
    // with authentic 2-layer directional cut-paper drop shadows & white bevel rims!
    unsafe fn build_flower_atlas(&mut self, gl: &GlFns, pc: &PapercutState) {
        self.flower_atlas.ensure(gl, FLW_ATLAS_W, FLW_ATLAS_H);
        self.flower_mask.ensure(gl, FLW_CELL_W, FLW_CELL_H);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.flower_atlas.fbo);
        (gl.Viewport)(0, 0, FLW_ATLAS_W, FLW_ATLAS_H);
        (gl.ClearColor)(0.0, 0.0, 0.0, 0.0);
        (gl.Clear)(GL_COLOR_BUFFER_BIT);

        let fc_list: Vec<[f32; 3]> = FLW[pc.p.palette.min(3)]
            .iter()
            .map(|&rgb| shade_color(u8_to_f32(rgb), pc.t_mode))
            .collect();
        let stem_col = pc.col(8);
        let leaf_col = pc.col(6);
        let sz = FLW_REF_SZ;
        let cw = FLW_CELL_W as f32;
        let ch = FLW_CELL_H as f32;
        let gx = cw * 0.5;
        let by = ch - 22.0; // leave 22px bottom margin for drop shadows
        let hy = by - sz * 1.05;
        let a = sz * 0.2;
        let white = [1.0, 1.0, 1.0, 1.0];

        for kind in 0..3 {
            for (ci, &fc) in fc_list.iter().enumerate() {
                let cell_x = (ci as i32) * FLW_CELL_W;
                let cell_y = (kind as i32) * FLW_CELL_H;

                let apply_cell_piece = |this: &mut Self, col_255: [f32; 3], dep: f32| {
                    (gl.BindFramebuffer)(GL_FRAMEBUFFER, this.flower_mask.fbo);
                    (gl.Viewport)(0, 0, FLW_CELL_W, FLW_CELL_H);
                    (gl.Disable)(GL_BLEND);
                    (gl.ClearColor)(0.0, 0.0, 0.0, 0.0);
                    (gl.Clear)(GL_COLOR_BUFFER_BIT);
                    this.flush_batch(gl, cw, ch);

                    // Composite into the cell sub-viewport of flower_atlas
                    // Notice OpenGL viewport Y is bottom-up, whereas our row index is top-down
                    let gl_y = FLW_ATLAS_H - cell_y - FLW_CELL_H;
                    let l = pc.light_dir();
                    let gap = pc.p.gap;
                    let sh_mul = pc.p.shadow;
                    (gl.BindFramebuffer)(GL_FRAMEBUFFER, this.flower_atlas.fbo);
                    (gl.Viewport)(cell_x, gl_y, FLW_CELL_W, FLW_CELL_H);
                    (gl.Enable)(GL_BLEND);
                    (gl.BlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
                    (gl.UseProgram)(this.prog_paper);
                    (gl.ActiveTexture)(GL_TEXTURE0);
                    (gl.BindTexture)(GL_TEXTURE_2D, this.flower_mask.tex);
                    (gl.Uniform1i)(gl.uniform_loc(this.prog_paper, "u_mask"), 0);
                    (gl.Uniform2f)(gl.uniform_loc(this.prog_paper, "u_res"), cw, ch);
                    let rgba = rgb255_to_rgba(col_255, 1.0);
                    (gl.Uniform4f)(
                        gl.uniform_loc(this.prog_paper, "u_color"),
                        rgba[0],
                        rgba[1],
                        rgba[2],
                        rgba[3],
                    );
                    (gl.Uniform2f)(
                        gl.uniform_loc(this.prog_paper, "u_sh_off1"),
                        -l[0] * dep * 0.45 * gap,
                        -l[1] * dep * 0.45 * gap,
                    );
                    (gl.Uniform1f)(gl.uniform_loc(this.prog_paper, "u_sh_blur1"), 1.5 * gap);
                    (gl.Uniform1f)(
                        gl.uniform_loc(this.prog_paper, "u_sh_a1"),
                        0.9f32.min(0.32 * sh_mul),
                    );
                    (gl.Uniform2f)(
                        gl.uniform_loc(this.prog_paper, "u_sh_off2"),
                        -l[0] * dep * 1.0 * gap,
                        -l[1] * dep * 1.0 * gap,
                    );
                    (gl.Uniform1f)(
                        gl.uniform_loc(this.prog_paper, "u_sh_blur2"),
                        (dep * 1.2 + 2.0) * gap,
                    );
                    (gl.Uniform1f)(
                        gl.uniform_loc(this.prog_paper, "u_sh_a2"),
                        0.9f32.min(0.20 * sh_mul),
                    );
                    (gl.Uniform1f)(gl.uniform_loc(this.prog_paper, "u_rim_a"), 0.18);
                    (gl.BindVertexArray)(this.dummy_vao);
                    (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
                };

                // 1. Stem: rect(gx - sz*0.03, hy, sz*0.06, by - hy)
                self.batch.clear();
                self.batch
                    .add_rect(gx - sz * 0.03, hy, sz * 0.06, by - hy, white);
                apply_cell_piece(self, stem_col, 1.5);

                // 2. Left & right leaves
                for d in [-1.0f32, 1.0f32] {
                    self.batch.clear();
                    self.batch.add_double_quad_blade(
                        [gx, by],
                        [gx + d * sz * 0.35, by - sz * 0.25],
                        [gx + d * sz * 0.30, by - sz * 0.55],
                        [gx + d * sz * 0.10, by - sz * 0.30],
                        [gx, by],
                        14,
                        white,
                    );
                    apply_cell_piece(self, leaf_col, 1.5);
                }

                // 3. Flower head (0=Lale/Tulip, 1=Karanfil/Carnation, 2=Sümbül/Hyacinth)
                if kind == 0 {
                    // Lale outer crown
                    self.batch.clear();
                    let top_pts = [
                        [gx - a, hy],
                        [gx - a * 0.85, hy - a * 1.5],
                        [gx - a * 0.3, hy - a * 0.8],
                        [gx, hy - a * 1.7],
                        [gx + a * 0.3, hy - a * 0.8],
                        [gx + a * 0.85, hy - a * 1.5],
                        [gx + a, hy],
                    ];
                    let center = [gx, hy];
                    for w_pt in top_pts.windows(2) {
                        self.batch.push_tri(center, w_pt[0], w_pt[1], white);
                    }
                    // Bottom quadratic cup: moveTo(gx+a, hy) -> quadraticCurveTo(gx, hy+a*1.3, gx-a, hy)
                    let steps = 12usize;
                    for s in 0..steps {
                        let t0 = s as f32 / steps as f32;
                        let t1 = (s + 1) as f32 / steps as f32;
                        let eval_cup = |t: f32| -> [f32; 2] {
                            let it = 1.0 - t;
                            [
                                it * it * (gx + a) + 2.0 * it * t * gx + t * t * (gx - a),
                                it * it * hy + 2.0 * it * t * (hy + a * 1.3) + t * t * hy,
                            ]
                        };
                        self.batch
                            .push_tri(center, eval_cup(t0), eval_cup(t1), white);
                    }
                    apply_cell_piece(self, fc, 2.0);

                    // Lale inner petal
                    self.batch.clear();
                    let ic = [gx, hy + a * 0.1];
                    self.batch
                        .push_tri(ic, [gx - a * 0.45, hy + a * 0.1], [gx, hy - a * 1.1], white);
                    self.batch
                        .push_tri(ic, [gx, hy - a * 1.1], [gx + a * 0.45, hy + a * 0.1], white);
                    for s in 0..10 {
                        let t0 = s as f32 / 10.0;
                        let t1 = (s + 1) as f32 / 10.0;
                        let eval_in = |t: f32| -> [f32; 2] {
                            let it = 1.0 - t;
                            [
                                it * it * (gx + a * 0.45)
                                    + 2.0 * it * t * gx
                                    + t * t * (gx - a * 0.45),
                                it * it * (hy + a * 0.1)
                                    + 2.0 * it * t * (hy + a * 0.7)
                                    + t * t * (hy + a * 0.1),
                            ]
                        };
                        self.batch.push_tri(ic, eval_in(t0), eval_in(t1), white);
                    }
                    apply_cell_piece(self, mix3(fc, [255.0, 255.0, 255.0], 0.3), 1.0);
                } else if kind == 1 {
                    // Karanfil 9 outer scalloped circles + center circle
                    self.batch.clear();
                    for i in 0..9 {
                        let ang = (i as f32) * 0.698;
                        self.batch.add_circle(
                            gx + ang.cos() * a * 0.75,
                            hy - a * 0.6 + ang.sin() * a * 0.75,
                            a * 0.55,
                            18,
                            white,
                        );
                    }
                    self.batch.add_circle(gx, hy - a * 0.6, a * 0.8, 22, white);
                    apply_cell_piece(self, fc, 2.0);

                    // Karanfil dark center core
                    self.batch.clear();
                    self.batch.add_circle(gx, hy - a * 0.6, a * 0.35, 16, white);
                    apply_cell_piece(self, mix3(fc, [40.0, 10.0, 30.0], 0.28), 1.0);
                } else {
                    // Sümbül 8 alternating bell circles
                    self.batch.clear();
                    for i in 0..8 {
                        let fi = i as f32;
                        let side = if i % 2 == 1 { 1.0 } else { -1.0 };
                        let bx = gx + side * a * 0.35 * (1.0 - fi / 10.0);
                        let by_c = hy - fi * a * 0.38;
                        let br = a * 0.38 * (1.0 - fi * 0.06);
                        self.batch.add_circle(bx, by_c, br, 16, white);
                    }
                    apply_cell_piece(self, fc, 2.0);
                }
            }
        }
    }

    // 1:1 recomp() — combines back + fg + 3-ring arched icon niches + hole + cavity shadow + paper grain + vignette
    unsafe fn recomp(&mut self, gl: &GlFns, pw: i32, ph: i32, pc: &PapercutState) {
        let w = pc.gw;
        let h = pc.gh;
        let white = [1.0, 1.0, 1.0, 1.0];

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.raw_comp_fbo.fbo);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);

        // 1. Blit back_fbo then blend premultiplied fg_fbo
        (gl.Disable)(GL_BLEND);
        self.blit_full_tex(gl, self.back_fbo.tex, w, h);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
        self.blit_full_tex(gl, self.fg_fbo.tex, w, h);

        // 2. Arched icon niche windows (3 rings + hole + inner cavity shadow)
        let has_active = pc.plazas.iter().any(|p| p.k > 0.02);
        if has_active {
            let e = pc.p.fw;
            let rc = [
                pc.col(7),
                pc.col(6),
                mix3(pc.col(1), [255.0, 255.0, 255.0], 0.3),
            ];
            let l = pc.light_dir();
            let gap = pc.p.gap;
            let sh_mul = pc.p.shadow;

            let add_plaza_arches = |batch: &mut ShapeBatch, plazas: &[IconPlaza], ex: f32, fstyle: i32| {
                batch.clear();
                for p in plazas {
                    if p.k <= 0.02 {
                        continue;
                    }
                    let k = p.k.max(0.02);
                    let hw = (p.w * 0.5 + ex) * k;
                    let hh = (p.h * 0.5 + ex) * k;
                    let a = if fstyle != 0 {
                        hw.min(hh)
                    } else {
                        hw.min(hh * 0.9)
                    };
                    let b = if fstyle != 0 { a } else { 8.0 * k };
                    batch.add_arch_rect(
                        p.x + p.w * 0.5 - hw,
                        p.y + p.h * 0.5 - hh,
                        hw * 2.0,
                        hh * 2.0,
                        a,
                        b,
                        white,
                    );
                }
            };

            // 3 concentric cut-paper arch rings (k = 3, 2, 1)
            for k in (1..=3).rev() {
                let fk = k as f32;
                add_plaza_arches(&mut self.batch, &pc.plazas, 8.0 + e * fk / 3.0, pc.p.fstyle);
                (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.mask_fbo.fbo);
                (gl.Viewport)(0, 0, pw, ph);
                (gl.Disable)(GL_BLEND);
                (gl.ClearColor)(0.0, 0.0, 0.0, 0.0);
                (gl.Clear)(GL_COLOR_BUFFER_BIT);
                self.flush_batch(gl, w, h);

                let sh_o = (2.0 + fk * 1.5) * gap;
                let sh_b = (3.0 + fk * 2.0) * gap;
                let sh_a = 0.9f32.min(0.32 * sh_mul);
                self.composite_mask_to_target(
                    gl,
                    self.raw_comp_fbo.fbo,
                    self.mask_fbo.tex,
                    pw,
                    ph,
                    w,
                    h,
                    rgb255_to_rgba(rc[3 - k], 1.0),
                    [-l[0] * sh_o, -l[1] * sh_o],
                    sh_b,
                    sh_a,
                    [0.0, 0.0],
                    0.0,
                    0.0,
                    0.0,
                );
            }

            // Inner arch hole (ex = 8.0) revealing back_fbo + inner cavity shadow (sh(c, 5, 10, 0.55))
            add_plaza_arches(&mut self.batch, &pc.plazas, 8.0, pc.p.fstyle);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.mask_fbo.fbo);
            (gl.Viewport)(0, 0, pw, ph);
            (gl.Disable)(GL_BLEND);
            (gl.ClearColor)(0.0, 0.0, 0.0, 0.0);
            (gl.Clear)(GL_COLOR_BUFFER_BIT);
            self.flush_batch(gl, w, h);

            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.raw_comp_fbo.fbo);
            (gl.Viewport)(0, 0, pw, ph);
            (gl.Enable)(GL_BLEND);
            (gl.BlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            (gl.UseProgram)(self.prog_niche_hole);
            (gl.ActiveTexture)(GL_TEXTURE0);
            (gl.BindTexture)(GL_TEXTURE_2D, self.mask_fbo.tex);
            (gl.Uniform1i)(gl.uniform_loc(self.prog_niche_hole, "u_hole_mask"), 0);
            (gl.ActiveTexture)(GL_TEXTURE1);
            (gl.BindTexture)(GL_TEXTURE_2D, self.back_fbo.tex);
            (gl.Uniform1i)(gl.uniform_loc(self.prog_niche_hole, "u_back"), 1);
            (gl.Uniform2f)(gl.uniform_loc(self.prog_niche_hole, "u_res"), w, h);
            (gl.Uniform2f)(
                gl.uniform_loc(self.prog_niche_hole, "u_sh_off"),
                -l[0] * 5.0 * gap,
                -l[1] * 5.0 * gap,
            );
            (gl.Uniform1f)(gl.uniform_loc(self.prog_niche_hole, "u_sh_blur"), 10.0 * gap);
            (gl.Uniform1f)(
                gl.uniform_loc(self.prog_niche_hole, "u_sh_a"),
                0.9f32.min(0.55 * sh_mul),
            );
            (gl.BindVertexArray)(self.dummy_vao);
            (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
        }

        // 3. Apply 128x128 paper fiber grain (multiply) & directional radial light vignette into comp_fbo
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.comp_fbo.fbo);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(self.prog_post);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.raw_comp_fbo.tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_post, "u_comp"), 0);
        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.grain_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_post, "u_grain"), 1);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_post, "u_res"), w, h);
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_post, "u_phys_res"),
            pw as f32,
            ph as f32,
        );
        (gl.Uniform1f)(
            gl.uniform_loc(self.prog_post, "u_grain_alpha"),
            0.08 + pc.p.grain * 0.32,
        );
        let l = pc.light_dir();
        (gl.Uniform3f)(
            gl.uniform_loc(self.prog_post, "u_vig_c0"),
            w * (0.5 + l[0] * 0.3),
            h * (0.5 + l[1] * 0.3),
            w.min(h) * 0.3,
        );
        (gl.Uniform3f)(
            gl.uniform_loc(self.prog_post, "u_vig_c1"),
            w * 0.5,
            h * 0.5,
            w.max(h) * 0.8,
        );
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
    }

    unsafe fn blit_full_tex(&mut self, gl: &GlFns, tex: GLuint, w: f32, h: f32) {
        self.sprite_verts.clear();
        let c = [1.0, 1.0, 1.0, 1.0];
        self.sprite_verts.extend_from_slice(&[
            SpriteVert { pos: [0.0, 0.0], uv: [0.0, 1.0], col: c },
            SpriteVert { pos: [w, 0.0], uv: [1.0, 1.0], col: c },
            SpriteVert { pos: [w, h], uv: [1.0, 0.0], col: c },
            SpriteVert { pos: [0.0, 0.0], uv: [0.0, 1.0], col: c },
            SpriteVert { pos: [w, h], uv: [1.0, 0.0], col: c },
            SpriteVert { pos: [0.0, h], uv: [0.0, 0.0], col: c },
        ]);
        self.flush_sprites(gl, tex, w, h);
    }

    unsafe fn flush_sprites(&mut self, gl: &GlFns, tex: GLuint, w: f32, h: f32) {
        if self.sprite_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_sprite);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_sprite, "u_res"), w, h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_sprite, "u_atlas"), 0);
        (gl.BindVertexArray)(self.sprite_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.sprite_vbo);
        let bytes = (self.sprite_verts.len() * std::mem::size_of::<SpriteVert>()) as GLsizeiptr;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            bytes,
            self.sprite_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.sprite_verts.len() as GLsizei);
        self.sprite_verts.clear();
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();

        st.papercut.resize(css_w, css_h);
        if self.last_w != pw || self.last_h != ph {
            self.last_w = pw;
            self.last_h = ph;
            st.papercut.cache_ok = false;
            st.papercut.dirty = true;
        }

        self.back_fbo.ensure(gl, pw, ph);
        self.fg_fbo.ensure(gl, pw, ph);
        self.tree_fbo.ensure(gl, pw, ph);
        self.mask_fbo.ensure(gl, pw, ph);
        self.raw_comp_fbo.ensure(gl, pw, ph);
        self.comp_fbo.ensure(gl, pw, ph);

        if !st.papercut.cache_ok {
            let new_t = st.papercut.resolve_time_now();
            if new_t != st.papercut.t_mode {
                st.papercut.t_mode = new_t;
                st.papercut.rebuild_entities();
            }
            self.build_back(gl, pw, ph, &st.papercut);
            self.build_fg(gl, pw, ph, &st.papercut);
            self.build_flower_atlas(gl, &st.papercut);
            st.papercut.cache_ok = true;
            st.papercut.dirty = true;
        }

        if st.papercut.dirty {
            self.recomp(gl, pw, ph, &st.papercut);
            st.papercut.dirty = false;
        }

        // Main Frame Draw (draw(now) in papercut.html)
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);
        self.blit_full_tex(gl, self.comp_fbo.tex, css_w, css_h);

        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);

        let pc = &st.papercut;
        let t = pc.sim_time;

        // 1. Swaying Ottoman garden flowers (lale, karanfil, sümbül — premultiplied FBO atlas)
        self.sprite_verts.clear();
        let cw = FLW_CELL_W as f32;
        let ch = FLW_CELL_H as f32;
        // Root pivot inside each 128x192 cell is (gx = cw*0.5, by = ch - 22.0)
        let piv_x = cw * 0.5;
        let piv_y = ch - 22.0;
        let white = [1.0, 1.0, 1.0, 1.0];

        for s in &pc.sprites {
            if pc.is_near_plaza(s.x, s.y, s.dh as f32) {
                continue;
            }
            let rot =
                ((t * s.f + s.ph).sin() * s.am + (t * 0.35 + s.x * 0.004).sin() * 0.04) * pc.p.sway;
            let (sr, cr) = rot.sin_cos();
            let scale = s.sz / FLW_REF_SZ;

            // Cell UV bounds in flower_atlas (note OpenGL bottom-up V coordinates)
            let u0 = (s.color_idx as f32 * cw) / FLW_ATLAS_W as f32;
            let u1 = ((s.color_idx + 1) as f32 * cw) / FLW_ATLAS_W as f32;
            let v_top = 1.0 - (s.kind as f32 * ch) / FLW_ATLAS_H as f32;
            let v_bot = 1.0 - ((s.kind + 1) as f32 * ch) / FLW_ATLAS_H as f32;

            let x0 = -piv_x * scale;
            let x1 = (cw - piv_x) * scale;
            let y0 = -piv_y * scale;
            let y1 = (ch - piv_y) * scale;
            let xform = |lx: f32, ly: f32| -> [f32; 2] {
                [s.x + lx * cr - ly * sr, s.y + lx * sr + ly * cr]
            };

            let p0 = SpriteVert { pos: xform(x0, y0), uv: [u0, v_top], col: white };
            let p1 = SpriteVert { pos: xform(x1, y0), uv: [u1, v_top], col: white };
            let p2 = SpriteVert { pos: xform(x1, y1), uv: [u1, v_bot], col: white };
            let p3 = SpriteVert { pos: xform(x0, y1), uv: [u0, v_bot], col: white };
            self.sprite_verts.extend_from_slice(&[p0, p1, p2, p0, p2, p3]);
        }
        self.flush_sprites(gl, self.flower_atlas.tex, css_w, css_h);

        // 2. Flying/flapping paper birds & butterflies (crit — straight alpha geometry)
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        self.batch.clear();
        for q in &pc.crit {
            let cy = q.y
                + if q.b {
                    (t * 0.9 + q.ph).sin() * 8.0
                } else {
                    (t * 1.7 + q.ph).sin() * 10.0
                };
            let flip = if q.v < 0.0 { -1.0 } else { 1.0 };
            let fl = (t * if q.b { 3.0 } else { 9.0 } + q.ph).sin().abs();
            let s = q.s;
            let col = rgb255_to_rgba(q.c, 1.0);

            if q.b {
                // Bird wings: moveTo(0,0); lineTo(-s, -s*(0.2+0.8*fl)); lineTo(-s*0.5, s*0.15)
                // and moveTo(0,0); lineTo(s*0.2, -s*(0.2+0.8*fl)); lineTo(s, -s*0.1); lineTo(s*0.4, s*0.18)
                let wy = -s * (0.2 + 0.8 * fl);
                self.batch.push_tri(
                    [q.x, cy],
                    [q.x - s * flip, cy + wy],
                    [q.x - s * 0.5 * flip, cy + s * 0.15],
                    col,
                );
                self.batch.push_tri(
                    [q.x, cy],
                    [q.x + s * 0.2 * flip, cy + wy],
                    [q.x + s * flip, cy - s * 0.1],
                    col,
                );
                self.batch.push_tri(
                    [q.x, cy],
                    [q.x + s * flip, cy - s * 0.1],
                    [q.x + s * 0.4 * flip, cy + s * 0.18],
                    col,
                );
            } else {
                // Butterfly wings: scale(0.2 + 0.8*fl, 1), two tilted ellipses at (-s*0.45, 0) rot -0.4 and (+s*0.45, 0) rot +0.4
                let sx_scale = (0.2 + 0.8 * fl) * flip;
                for (ox, rot_w) in [(-s * 0.45, -0.4f32), (s * 0.45, 0.4f32)] {
                    let (sr, cr) = rot_w.sin_cos();
                    let rx = s * 0.45;
                    let ry = s * 0.7;
                    let segs = 12usize;
                    let c_pt = [q.x + ox * sx_scale, cy];
                    for i in 0..segs {
                        let a0 = (i as f32 / segs as f32) * TAU;
                        let a1 = ((i + 1) as f32 / segs as f32) * TAU;
                        let lx0 = a0.cos() * rx;
                        let ly0 = a0.sin() * ry;
                        let lx1 = a1.cos() * rx;
                        let ly1 = a1.sin() * ry;
                        let p0 = [
                            q.x + (ox + lx0 * cr - ly0 * sr) * sx_scale,
                            cy + lx0 * sr + ly0 * cr,
                        ];
                        let p1 = [
                            q.x + (ox + lx1 * cr - ly1 * sr) * sx_scale,
                            cy + lx1 * sr + ly1 * cr,
                        ];
                        self.batch.push_tri(c_pt, p0, p1, col);
                    }
                }
            }
        }
        self.flush_batch(gl, css_w, css_h);
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
    fn verify_rust_anything_recorded_papercut_cases() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root");
        let rec_path = manifest_dir.join("src/papercut/recorded-from.json");
        let cases_path = manifest_dir.join("src/papercut/cases.jsonl");

        // 1. Verify recording fingerprint against prototype/papercut.html
        let rec_json: Value = serde_json::from_str(
            &std::fs::read_to_string(&rec_path).expect("read recorded-from.json"),
        )
        .expect("parse recorded-from.json");
        let unit_rel = rec_json["unit"].as_str().expect("unit path");
        let expected_fp = rec_json["fingerprint"].as_str().expect("fingerprint");
        let html_bytes =
            std::fs::read(repo_root.join(unit_rel)).expect("read prototype/papercut.html");
        let actual_fp = fnv1a_fingerprint(unit_rel, &html_bytes);
        assert_eq!(
            actual_fp, expected_fp,
            "rust-anything gate: prototype/papercut.html changed since recording"
        );

        // 2. Replay all 59 recorded cases in cases.jsonl
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
                "paletteShadedColors" => {
                    let palette = args[0].as_u64().unwrap() as usize;
                    let t_mode = args[1].as_u64().unwrap() as usize;
                    let exp_cols = ok.as_array().unwrap();
                    for slot in 0..9 {
                        let got = palette_col(palette, t_mode, slot);
                        let exp = exp_cols[slot].as_array().unwrap();
                        for k in 0..3 {
                            let ev = exp[k].as_f64().unwrap() as f32;
                            assert!(
                                (got[k] - ev).abs() <= 1e-3,
                                "case #{line_no} paletteShadedColors({palette}, {t_mode}) slot {slot}: got {}, exp {ev}",
                                got[k]
                            );
                        }
                    }
                }
                "mkSpritesLayout" => {
                    let w = args[0].as_f64().unwrap() as f32;
                    let h = args[1].as_f64().unwrap() as f32;
                    let flowers = args[2].as_f64().unwrap() as f32;
                    let quality = args[3].as_u64().unwrap() as usize;
                    let palette = args[4].as_u64().unwrap() as usize;

                    let mut st = PapercutState::default();
                    st.p.flowers = flowers;
                    st.p.quality = quality;
                    st.p.palette = palette;
                    st.t_mode = 2;
                    st.mk_sprites(w, h);

                    assert_eq!(st.sprites.len() as i64, ok["count"].as_i64().unwrap());
                    let check_sp = |sp: &FlowerSprite, arr: &[Value]| {
                        assert!((sp.x - arr[0].as_f64().unwrap() as f32).abs() <= 1e-2);
                        assert!((sp.y - arr[1].as_f64().unwrap() as f32).abs() <= 1e-2);
                        assert_eq!(sp.dw as i64, arr[2].as_i64().unwrap());
                        assert_eq!(sp.dh as i64, arr[3].as_i64().unwrap());
                        assert!((sp.f - arr[4].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert!((sp.ph - arr[5].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert!((sp.am - arr[6].as_f64().unwrap() as f32).abs() <= 1e-4);
                    };
                    check_sp(&st.sprites[0], ok["first"].as_array().unwrap());
                    check_sp(
                        st.sprites.last().unwrap(),
                        ok["last"].as_array().unwrap(),
                    );
                }
                "mkCritLayout" => {
                    let w = args[0].as_f64().unwrap() as f32;
                    let h = args[1].as_f64().unwrap() as f32;
                    let critters = args[2].as_u64().unwrap() as usize;

                    let mut st = PapercutState::default();
                    st.p.critters = critters;
                    st.p.palette = 1;
                    st.t_mode = 3;
                    st.mk_crit(w, h);

                    let exp_list = ok.as_array().unwrap();
                    assert_eq!(st.crit.len(), exp_list.len());
                    for (got, ev) in st.crit.iter().zip(exp_list.iter()) {
                        assert_eq!(got.b, ev["b"].as_bool().unwrap());
                        assert!((got.x - ev["x"].as_f64().unwrap() as f32).abs() <= 1e-2);
                        assert!((got.y - ev["y"].as_f64().unwrap() as f32).abs() <= 1e-2);
                        assert!((got.s - ev["s"].as_f64().unwrap() as f32).abs() <= 1e-3);
                        assert!((got.v - ev["v"].as_f64().unwrap() as f32).abs() <= 1e-3);
                        assert!((got.ph - ev["ph"].as_f64().unwrap() as f32).abs() <= 1e-3);
                    }
                }
                "plazaStepAndNear" => {
                    let mut st = PapercutState::default();
                    st.p.adaptive = 1;
                    st.p.fw = 16.0;
                    st.gw = 1920.0;
                    st.gh = 1080.0;
                    st.plazas.clear();
                    let icons1 = [
                        Rect { x: 80.0, y: 100.0, w: 76.0, h: 90.0 },
                        Rect { x: 200.0, y: 100.0, w: 76.0, h: 90.0 },
                    ];
                    st.step(0.033, &icons1);
                    st.step(0.033, &icons1);
                    for (p, ev) in st
                        .plazas
                        .iter()
                        .zip(ok["snap1"].as_array().unwrap().iter())
                    {
                        let arr = ev.as_array().unwrap();
                        assert!((p.x - arr[0].as_f64().unwrap() as f32).abs() <= 1e-3);
                        assert!((p.y - arr[1].as_f64().unwrap() as f32).abs() <= 1e-3);
                        assert!((p.k - arr[4].as_f64().unwrap() as f32).abs() <= 1e-3);
                    }

                    let icons2 = [Rect { x: 110.0, y: 120.0, w: 76.0, h: 90.0 }];
                    st.step(0.033, &icons2);
                    st.step(0.033, &icons2);
                    for (p, ev) in st
                        .plazas
                        .iter()
                        .zip(ok["snap2"].as_array().unwrap().iter())
                    {
                        let arr = ev.as_array().unwrap();
                        assert!((p.x - arr[0].as_f64().unwrap() as f32).abs() <= 1e-3);
                        assert!((p.y - arr[1].as_f64().unwrap() as f32).abs() <= 1e-3);
                        assert!((p.k - arr[4].as_f64().unwrap() as f32).abs() <= 1e-3);
                    }
                    assert_eq!(
                        st.is_near_plaza(130.0, 160.0, 60.0),
                        ok["nearHit"].as_bool().unwrap()
                    );
                    assert_eq!(
                        st.is_near_plaza(800.0, 900.0, 60.0),
                        ok["nearMiss"].as_bool().unwrap()
                    );
                }
                other => panic!("Unknown recorded function: {other}"),
            }
            passed += 1;
        }
        assert_eq!(passed, 59, "All 59 recorded rust-anything papercut cases must pass");
    }
}
