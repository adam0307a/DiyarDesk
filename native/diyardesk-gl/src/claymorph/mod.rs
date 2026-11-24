pub mod shaders;

use crate::gl::*;
use crate::quiet_snow::Mulberry32;
use crate::state::{AppState, Rect};
use shaders::*;
use std::f32::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// 1. PALETTES, CLAY COLOR SETS & QUALITY PROFILES (1:1 claymorph.html)
// =========================================================================
// Each clay/neutral color set: hi, mid, lo (hex), spec (r,g,b string), rim (r,g,b), drop (r,g,b)
#[derive(Clone, Copy)]
pub struct ClayColor {
    pub hi: [f32; 3],
    pub mid: [f32; 3],
    pub lo: [f32; 3],
    pub spec: [f32; 3],
    pub rim: [f32; 3],
    pub drop: [f32; 3],
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg_top: [f32; 3],
    pub bg_bot: [f32; 3],
    pub shadow_rgb: [f32; 3],
    pub label_pill: [f32; 4],
    pub is_dark: bool,
    pub neutral: ClayColor,
    pub clays: [ClayColor; 5],
}

#[inline]
const fn hex(h: u32) -> [f32; 3] {
    [
        ((h >> 16) & 0xff) as f32 / 255.0,
        ((h >> 8) & 0xff) as f32 / 255.0,
        (h & 0xff) as f32 / 255.0,
    ]
}

#[inline]
const fn rgb_s(r: u32, g: u32, b: u32) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

macro_rules! clay {
    ($hi:expr, $mid:expr, $lo:expr, $spec:expr, $rim:expr, $drop:expr) => {
        ClayColor {
            hi: hex($hi),
            mid: hex($mid),
            lo: hex($lo),
            spec: rgb_s($spec.0, $spec.1, $spec.2),
            rim: rgb_s($rim.0, $rim.1, $rim.2),
            drop: rgb_s($drop.0, $drop.1, $drop.2),
        }
    };
}

pub const PALETTES: [Palette; 4] = [
    // 0: pastel
    Palette {
        bg_top: hex(0xf3f5fc),
        bg_bot: hex(0xe1e7f7),
        shadow_rgb: rgb_s(118, 134, 178),
        label_pill: [38.0 / 255.0, 48.0 / 255.0, 82.0 / 255.0, 0.22],
        is_dark: false,
        neutral: clay!(
            0xffffff, 0xf3f6ff, 0xcfd9f2, (255, 255, 255), (166, 182, 224), (122, 138, 184)
        ),
        clays: [
            clay!(0xb8c8ff, 0x8fa8ff, 0x6580e6, (255, 255, 255), (88, 114, 214), (110, 128, 196)),
            clay!(0xffc2b6, 0xff9b89, 0xe5725f, (255, 248, 245), (214, 96, 78), (188, 116, 112)),
            clay!(0xaaf2da, 0x76e0be, 0x48bd97, (248, 255, 252), (56, 168, 132), (96, 168, 152)),
            clay!(0xffeaa8, 0xffd573, 0xe6b244, (255, 253, 242), (212, 156, 48), (178, 152, 108)),
            clay!(0xdfc8ff, 0xc39eff, 0x9872e3, (255, 250, 255), (134, 94, 212), (142, 122, 192)),
        ],
    },
    // 1: candy
    Palette {
        bg_top: hex(0xfdf2f6),
        bg_bot: hex(0xf5e2ec),
        shadow_rgb: rgb_s(168, 118, 146),
        label_pill: [68.0 / 255.0, 36.0 / 255.0, 56.0 / 255.0, 0.22],
        is_dark: false,
        neutral: clay!(
            0xffffff, 0xfff5f8, 0xefd4df, (255, 255, 255), (216, 174, 194), (176, 128, 152)
        ),
        clays: [
            clay!(0xffadc6, 0xff7ca4, 0xe0517e, (255, 248, 252), (208, 66, 112), (196, 112, 142)),
            clay!(0xa7c2ff, 0x789eff, 0x5278e0, (248, 252, 255), (72, 108, 208), (124, 138, 198)),
            clay!(0xffd4a3, 0xffb469, 0xe48d3c, (255, 250, 242), (210, 124, 46), (188, 138, 108)),
            clay!(0xb4f0d3, 0x82e0b4, 0x52bd8c, (248, 255, 250), (66, 168, 122), (116, 166, 146)),
            clay!(0xe6c2ff, 0xcc94ff, 0xa268df, (255, 248, 255), (148, 88, 208), (156, 118, 186)),
        ],
    },
    // 2: matcha
    Palette {
        bg_top: hex(0xf1f5ed),
        bg_bot: hex(0xe1eadb),
        shadow_rgb: rgb_s(124, 142, 118),
        label_pill: [38.0 / 255.0, 52.0 / 255.0, 40.0 / 255.0, 0.22],
        is_dark: false,
        neutral: clay!(
            0xffffff, 0xf6faf2, 0xd6e2cf, (255, 255, 255), (176, 194, 168), (128, 148, 122)
        ),
        clays: [
            clay!(0xb7e6bf, 0x8ad198, 0x5ead6e, (248, 255, 250), (74, 152, 92), (112, 158, 122)),
            clay!(0xfabaa6, 0xef9278, 0xcf6b50, (255, 248, 245), (192, 92, 66), (176, 122, 108)),
            clay!(0xfae0a2, 0xf2c76b, 0xd4a342, (255, 252, 242), (196, 144, 48), (168, 148, 104)),
            clay!(0xb6ddf5, 0x89c2e8, 0x5b9ac7, (248, 253, 255), (74, 136, 182), (116, 148, 174)),
            clay!(0xe8d7c6, 0xd4bca4, 0xb5997e, (255, 251, 246), (164, 138, 112), (152, 136, 120)),
        ],
    },
    // 3: dusk
    Palette {
        bg_top: hex(0x242136),
        bg_bot: hex(0x181626),
        shadow_rgb: rgb_s(8, 7, 16),
        label_pill: [10.0 / 255.0, 9.0 / 255.0, 18.0 / 255.0, 0.28],
        is_dark: true,
        neutral: clay!(
            0x413c5e, 0x312d4a, 0x211e35, (158, 150, 212), (18, 15, 32), (8, 7, 16)
        ),
        clays: [
            clay!(0xa99bf5, 0x8674e2, 0x5d4bb8, (226, 220, 255), (68, 52, 156), (12, 10, 26)),
            clay!(0xffa399, 0xf2796e, 0xc45046, (255, 224, 220), (164, 56, 48), (18, 10, 16)),
            clay!(0x7be8d3, 0x4bcab2, 0x2b9c87, (218, 255, 248), (32, 132, 114), (10, 16, 20)),
            clay!(0xffd182, 0xf2b150, 0xc4862b, (255, 242, 212), (168, 112, 32), (18, 12, 10)),
            clay!(0xefa8e8, 0xd67dcf, 0xa652a0, (255, 224, 252), (142, 62, 136), (16, 10, 22)),
        ],
    },
];

// Quality profile: max_dpr, shadow_steps, blob_scale, enable_sparkles
#[derive(Clone, Copy)]
pub struct QualityProfile {
    #[allow(dead_code)]
    pub max_dpr: f32,
    #[allow(dead_code)]
    pub shadow_steps: usize,
    pub blob_scale: f32,
    #[allow(dead_code)]
    pub enable_sparkles: bool,
}

pub const QUALITY_PROFILES: [QualityProfile; 3] = [
    QualityProfile { max_dpr: 0.75, shadow_steps: 7, blob_scale: 0.55, enable_sparkles: false },
    QualityProfile { max_dpr: 1.25, shadow_steps: 11, blob_scale: 0.85, enable_sparkles: true },
    QualityProfile { max_dpr: 1.50, shadow_steps: 14, blob_scale: 1.00, enable_sparkles: true },
];

// =========================================================================
// 2. PARAMETERS & LIVE SIMULATION STATE
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct ClaymorphParams {
    pub palette: usize,
    pub puffiness: f32,
    pub squish_strength: f32,
    pub blob_count: f32,
    pub float_speed: f32,
    pub cursor_repel: f32,
    pub icon_cradle: usize, // 0=puffy, 1=cloud, 2=ring
    pub quality: usize,     // 0=Low, 1=Medium, 2=High
    pub adaptive: i32,
    pub seed: u32,
}

impl Default for ClaymorphParams {
    fn default() -> Self {
        Self {
            palette: 0,
            puffiness: 1.0,
            squish_strength: 1.0,
            blob_count: 1.0,
            float_speed: 0.8,
            cursor_repel: 1.0,
            icon_cradle: 0,
            quality: 2,
            adaptive: 1,
            seed: 20261118,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ClayBlob {
    pub kind: usize, // 0=orb, 1=pill
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub color_idx: usize,
    pub home_x: f32,
    pub home_y: f32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub bob_amp_x: f32,
    pub bob_amp_y: f32,
    pub bob_freq_x: f32,
    pub bob_freq_y: f32,
    pub phase: f32,
    pub rot: f32,
    pub rot_speed: f32,
    pub squash: f32,
    pub squash_vel: f32,
    pub depth_alpha: f32,
}

#[derive(Clone, Copy)]
pub struct ClayIconState {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub tx: f32,
    pub ty: f32,
    pub tw: f32,
    pub th: f32,
    pub opacity: f32,
    pub color_idx: usize,
    pub squash: f32,
    pub squash_vel: f32,
    pub phase: f32,
    pub removing: bool,
}

#[derive(Clone, Copy)]
pub struct Ripple {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub speed: f32,
    pub max_r: f32,
    #[allow(dead_code)]
    pub life: f32,
}

pub struct ClaymorphState {
    pub p: ClaymorphParams,
    pub gw: f32,
    pub gh: f32,
    pub sim_time: f32,
    pub blobs: Vec<ClayBlob>,
    pub icon_states: Vec<ClayIconState>,
    pub ripples: Vec<Ripple>,
    pub icons_seen: bool,
    pub cache_ok: bool,
    pub dirty: bool,
    pub pointer_down: bool,
}

impl Default for ClaymorphState {
    fn default() -> Self {
        let mut st = Self {
            p: ClaymorphParams::default(),
            gw: 1920.0,
            gh: 1080.0,
            sim_time: 0.0,
            blobs: Vec::new(),
            icon_states: Vec::new(),
            ripples: Vec::new(),
            icons_seen: false,
            cache_ok: false,
            dirty: true,
            pointer_down: false,
        };
        st.build_blobs();
        st
    }
}

impl ClaymorphState {
    #[inline]
    fn profile(&self) -> QualityProfile {
        QUALITY_PROFILES[self.p.quality.min(2)]
    }

    pub fn palette(&self) -> &'static Palette {
        &PALETTES[self.p.palette.min(3)]
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = w.max(240.0);
        let nh = h.max(160.0);
        if (self.gw - nw).abs() > 0.5 || (self.gh - nh).abs() > 0.5 {
            self.gw = nw;
            self.gh = nh;
            self.cache_ok = false;
            self.dirty = true;
            self.build_blobs();
        }
    }

    pub fn sync_params(&mut self, next: ClaymorphParams) {
        let need_blobs = next.seed != self.p.seed || (next.blob_count - self.p.blob_count).abs() > 1e-3;
        let need_bg = next.palette != self.p.palette;
        let rebake = next.puffiness != self.p.puffiness || next.palette != self.p.palette || next.icon_cradle != self.p.icon_cradle;
        self.p = next;
        if rebake {
            self.cache_ok = false;
            self.dirty = true;
        }
        if need_blobs {
            self.build_blobs();
        }
        if need_bg {
            self.cache_ok = false;
            self.dirty = true;
        }
    }

    pub fn set_low_mode(&mut self, _low: bool) {}

    pub fn on_pointer_move(&mut self, _x: f32, _y: f32) {}

    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        self.pointer_down = true;
        self.apply_click_squish(x, y);
    }

    pub fn on_pointer_up(&mut self) {
        self.pointer_down = false;
    }

    pub fn on_pointer_leave(&mut self) {
        self.pointer_down = false;
    }

    // 1:1 buildBlobs()
    pub fn build_blobs(&mut self) {
        let prof = self.profile();
        let count = (14.0 * self.p.blob_count * prof.blob_scale).round().clamp(0.0, 26.0) as usize;
        let mut rng = Mulberry32::new((self.p.seed ^ 0x434C4159) as i32);
        self.blobs.clear();

        let orb_buckets = [24.0f32, 32.0, 42.0, 54.0, 68.0];
        let pill_buckets: [[f32; 3]; 4] = [
            [108.0, 48.0, 24.0],
            [136.0, 56.0, 28.0],
            [92.0, 92.0, 30.0],
            [124.0, 64.0, 32.0],
        ];

        for _ in 0..count {
            let is_pill = rng.next_f64() > 0.55;
            let color_idx = (rng.next_f64() * 5.0) as usize % 5;
            let home_x = (0.08 + rng.next_f64() as f32 * 0.84) * self.gw;
            let home_y = (0.08 + rng.next_f64() as f32 * 0.84) * self.gh;
            let (w, h, r, kind) = if is_pill {
                let pb = pill_buckets[(rng.next_f64() * pill_buckets.len() as f64) as usize % pill_buckets.len()];
                (pb[0], pb[1], pb[2], 1usize)
            } else {
                let rad = orb_buckets[(rng.next_f64() * orb_buckets.len() as f64) as usize % orb_buckets.len()];
                (rad * 2.0, rad * 2.0, rad, 0usize)
            };
            self.blobs.push(ClayBlob {
                kind,
                w,
                h,
                r,
                color_idx,
                home_x,
                home_y,
                x: home_x,
                y: home_y,
                vx: 0.0,
                vy: 0.0,
                bob_amp_x: 8.0 + rng.next_f64() as f32 * 16.0,
                bob_amp_y: 10.0 + rng.next_f64() as f32 * 18.0,
                bob_freq_x: 0.35 + rng.next_f64() as f32 * 0.45,
                bob_freq_y: 0.42 + rng.next_f64() as f32 * 0.48,
                phase: rng.next_f64() as f32 * PI * 2.0,
                rot: (rng.next_f64() as f32 - 0.5) * 0.45,
                rot_speed: (rng.next_f64() as f32 - 0.5) * 0.14,
                squash: 0.0,
                squash_vel: 0.0,
                depth_alpha: 0.72 + rng.next_f64() as f32 * 0.28,
            });
        }
        self.dirty = true;
    }

    // 1:1 syncIncomingIcons(rects)
    pub fn sync_incoming_icons(&mut self, rects: &[Rect]) {
        self.icons_seen = true;
        let mut matched = vec![false; self.icon_states.len()];

        for (i, ic) in rects.iter().enumerate() {
            let mut best_idx: Option<usize> = None;
            let mut best_dist = 48.0f32;
            for s in 0..self.icon_states.len() {
                if matched[s] {
                    continue;
                }
                let st = self.icon_states[s];
                let d = ((ic.x - st.tx).powi(2) + (ic.y - st.ty).powi(2)).sqrt();
                if d < best_dist {
                    best_dist = d;
                    best_idx = Some(s);
                }
            }
            if let Some(bi) = best_idx {
                let st = &mut self.icon_states[bi];
                st.tx = ic.x;
                st.ty = ic.y;
                st.tw = ic.w;
                st.th = ic.h;
                st.removing = false;
                matched[bi] = true;
            } else {
                let color_seed = ((ic.x / 40.0).round() as i64 * 7 + (ic.y / 40.0).round() as i64 * 13 + i as i64).unsigned_abs() as usize % 5;
                self.icon_states.push(ClayIconState {
                    x: ic.x,
                    y: ic.y,
                    w: ic.w,
                    h: ic.h,
                    tx: ic.x,
                    ty: ic.y,
                    tw: ic.w,
                    th: ic.h,
                    opacity: 0.0,
                    color_idx: color_seed,
                    squash: 0.0,
                    squash_vel: 0.0,
                    phase: (i as f32 * 1.29) % (PI * 2.0),
                    removing: false,
                });
                matched.push(true);
            }
        }

        for s in 0..self.icon_states.len() {
            if !matched[s] {
                self.icon_states[s].removing = true;
            }
        }
        self.dirty = true;
    }

    // 1:1 applyClickSquish(x, y)
    pub fn apply_click_squish(&mut self, x: f32, y: f32) {
        let sq_str = self.p.squish_strength.clamp(0.0, 2.0);
        if sq_str <= 0.01 {
            return;
        }
        self.spawn_ripple(x, y);

        for b in &mut self.blobs {
            let dx = b.x - x;
            let dy = b.y - y;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            if dist < 280.0 {
                let inf = 1.0 - dist / 280.0;
                b.squash_vel += inf * 1.45 * sq_str;
                b.vx += (dx / dist) * inf * 130.0 * sq_str;
                b.vy += (dy / dist) * inf * 130.0 * sq_str;
            }
        }
        for st in &mut self.icon_states {
            let icx = st.x + st.w * 0.5;
            let icy = st.y + st.h * 0.5;
            let dist = ((icx - x).powi(2) + (icy - y).powi(2)).sqrt();
            if dist < 220.0 {
                let inf = 1.0 - dist / 220.0;
                st.squash_vel += inf * 1.25 * sq_str;
            }
        }
        self.dirty = true;
    }

    // 1:1 spawnRipple(x, y)
    fn spawn_ripple(&mut self, x: f32, y: f32) {
        if self.ripples.len() >= 5 {
            self.ripples.remove(0);
        }
        self.ripples.push(Ripple {
            x,
            y,
            r: 14.0,
            speed: 380.0,
            max_r: (self.gw * self.gw + self.gh * self.gh).sqrt() * 0.5,
            life: 0.0,
        });
    }

    // 1:1 stepSimulation(dt)
    pub fn step(&mut self, dt: f32, icons: &[Rect], pointer: (f32, f32, bool, bool)) {
        let (px, py, p_active, p_down) = pointer;
        let f_speed = self.p.float_speed.clamp(0.0, 2.5);
        self.sim_time += dt * (0.25 + f_speed * 0.9);

        // Update ripples
        for i in (0..self.ripples.len()).rev() {
            let rp = &mut self.ripples[i];
            rp.r += rp.speed * dt;
            rp.life += dt;
            if rp.r >= rp.max_r {
                self.ripples.remove(i);
            }
        }

        // Floating clay blobs
        let repel_radius = 155.0;
        let repel_strength = self.p.cursor_repel.clamp(0.0, 2.0);
        let k_sq = 82.0f32;
        let d_sq = 7.2f32;

        for b in &mut self.blobs {
            let target_x = b.home_x + (self.sim_time * b.bob_freq_x + b.phase).sin() * b.bob_amp_x;
            let target_y = b.home_y + (self.sim_time * b.bob_freq_y + b.phase * 1.3).cos() * b.bob_amp_y;

            let mut push_x = 0.0f32;
            let mut push_y = 0.0f32;
            if p_active && repel_strength > 0.01 {
                let dx = b.x - px;
                let dy = b.y - py;
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                if d < repel_radius + b.r {
                    let inf = 1.0 - d / (repel_radius + b.r);
                    let force = inf * inf * 420.0 * repel_strength * if p_down { 1.45 } else { 1.0 };
                    push_x = (dx / d) * force;
                    push_y = (dy / d) * force;
                    b.squash = (b.squash + inf * 0.35 * dt * 10.0 * self.p.squish_strength.clamp(0.2, 2.0)).clamp(-0.22, 0.24);
                }
            }

            // Gentle avoidance from desktop icon rects
            if self.p.adaptive != 0 {
                for ic in icons {
                    let icx = ic.x + ic.w * 0.5;
                    let icy = ic.y + ic.h * 0.5;
                    let dx = b.x - icx;
                    let dy = b.y - icy;
                    let min_sep = ic.w.max(ic.h) * 0.65 + b.r + 14.0;
                    let d = (dx * dx + dy * dy).sqrt().max(1.0);
                    if d < min_sep {
                        let inf = 1.0 - d / min_sep;
                        push_x += (dx / d) * inf * 260.0;
                        push_y += (dy / d) * inf * 260.0;
                    }
                }
            }

            let ax = (target_x - b.x) * 14.0 + push_x - b.vx * 4.5;
            let ay = (target_y - b.y) * 14.0 + push_y - b.vy * 4.5;
            b.vx += ax * dt;
            b.vy += ay * dt;
            b.x += b.vx * dt;
            b.y += b.vy * dt;
            b.rot += b.rot_speed * dt * (0.3 + f_speed);

            let sq_accel = -b.squash * k_sq - b.squash_vel * d_sq;
            b.squash_vel += sq_accel * dt;
            b.squash = (b.squash + b.squash_vel * dt).clamp(-0.25, 0.28);
        }

        // Adaptive icon cradles
        let pos_alpha = 1.0 - (-10.0 * dt).exp();
        for i in (0..self.icon_states.len()).rev() {
            let st = &mut self.icon_states[i];
            st.x += (st.tx - st.x) * pos_alpha;
            st.y += (st.ty - st.y) * pos_alpha;
            st.w += (st.tw - st.w) * pos_alpha;
            st.h += (st.th - st.h) * pos_alpha;

            if st.removing {
                st.opacity = (st.opacity - dt * 2.5).max(0.0);
                if st.opacity <= 0.005 {
                    self.icon_states.remove(i);
                    continue;
                }
            } else {
                st.opacity = (st.opacity + dt * 3.0).min(1.0);
            }

            if p_active && self.p.squish_strength > 0.01 {
                let icx = st.x + st.w * 0.5;
                let icy = st.y + st.h * 0.5;
                let d = ((icx - px).powi(2) + (icy - py).powi(2)).sqrt();
                if d < 68.0 {
                    let inf = (1.0 - d / 68.0) * if p_down { 0.14 } else { 0.06 } * self.p.squish_strength.clamp(0.0, 1.8);
                    st.squash_vel += (inf - st.squash) * 32.0 * dt;
                }
            }

            let sq_accel = -st.squash * k_sq - st.squash_vel * d_sq;
            st.squash_vel += sq_accel * dt;
            st.squash = (st.squash + st.squash_vel * dt).clamp(-0.18, 0.22);
        }
    }
}

// =========================================================================
// =========================================================================
// 3. PER-FRAME SDF CLAY RENDERER (1:1 claymorph.html look, no atlas baking)
// =========================================================================
#[repr(C)]
#[derive(Clone, Copy)]
struct ColorVert {
    pos: [f32; 2],
    col: [f32; 4],
}

struct ColorBatch {
    verts: Vec<ColorVert>,
}

impl ColorBatch {
    fn new() -> Self {
        Self { verts: Vec::with_capacity(32768) }
    }
    #[inline]
    fn clear(&mut self) {
        self.verts.clear();
    }
    #[inline]
    fn push_tri(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], col: [f32; 4]) {
        self.verts.push(ColorVert { pos: a, col });
        self.verts.push(ColorVert { pos: b, col });
        self.verts.push(ColorVert { pos: c, col });
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
    fn add_rect_radial(&mut self, cx: f32, cy: f32, r: f32, c_in: [f32; 4], segs: usize) {
        if r <= 0.05 {
            return;
        }
        let c_out = [c_in[0], c_in[1], c_in[2], 0.0];
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            let p0 = [cx + a0.cos() * r, cy + a0.sin() * r];
            let p1 = [cx + a1.cos() * r, cy + a1.sin() * r];
            self.verts.push(ColorVert { pos: [cx, cy], col: c_in });
            self.verts.push(ColorVert { pos: p0, col: c_out });
            self.verts.push(ColorVert { pos: p1, col: c_out });
        }
    }
    fn add_circle(&mut self, cx: f32, cy: f32, r: f32, segs: usize, col: [f32; 4]) {
        if r <= 0.05 {
            return;
        }
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            self.push_tri([cx, cy], [cx + a0.cos() * r, cy + a0.sin() * r], [cx + a1.cos() * r, cy + a1.sin() * r], col);
        }
    }
    #[allow(dead_code)]
    fn add_rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        self.push_tri([x, y], [x + w, y], [x + w, y + h], col);
        self.push_tri([x, y], [x + w, y + h], [x, y + h], col);
    }
    #[allow(dead_code)]
    fn add_round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, col: [f32; 4]) {
        let rr = r.clamp(0.0, (w * 0.5).min(h * 0.5));
        let segs = 10usize;
        let mut poly: Vec<[f32; 2]> = Vec::with_capacity((segs + 1) * 4);
        for i in 0..=segs {
            let a = -PI * 0.5 + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + w - rr + a.cos() * rr, y + rr + a.sin() * rr]);
        }
        for i in 0..=segs {
            let a = (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + w - rr + a.cos() * rr, y + h - rr + a.sin() * rr]);
        }
        for i in 0..=segs {
            let a = PI * 0.5 + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + rr + a.cos() * rr, y + h - rr + a.sin() * rr]);
        }
        for i in 0..=segs {
            let a = PI + (i as f32 / segs as f32) * (PI * 0.5);
            poly.push([x + rr + a.cos() * rr, y + rr + a.sin() * rr]);
        }
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        for i in 0..poly.len() {
            let p0 = poly[i];
            let p1 = poly[(i + 1) % poly.len()];
            self.push_tri([cx, cy], p0, p1, col);
        }
    }
    #[allow(dead_code)]
    fn add_ring(&mut self, cx: f32, cy: f32, r: f32, thickness: f32, col: [f32; 4], segs: usize) {
        let inner = (r - thickness).max(0.5);
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            let o0 = [cx + a0.cos() * r, cy + a0.sin() * r];
            let o1 = [cx + a1.cos() * r, cy + a1.sin() * r];
            let i0 = [cx + a0.cos() * inner, cy + a0.sin() * inner];
            let i1 = [cx + a1.cos() * inner, cy + a1.sin() * inner];
            self.push_tri(o0, o1, i1, col);
            self.push_tri(o0, i1, i0, col);
        }
    }
    #[allow(dead_code)]
    fn add_rect_outline(&mut self, x: f32, y: f32, w: f32, h: f32, thick: f32, col: [f32; 4]) {
        self.add_rect(x, y, w, thick, col);
        self.add_rect(x, y + h - thick, w, thick, col);
        self.add_rect(x, y, thick, h, col);
        self.add_rect(x + w - thick, y, thick, h, col);
    }
}

pub struct ClaymorphRenderer {
    prog_color: GLuint,
    prog_clay: GLuint,
    dummy_vao: GLuint,
    color_vao: GLuint,
    color_vbo: GLuint,
    batch: ColorBatch,
    clay_vao: GLuint,
    clay_vbo: GLuint,
}

impl ClaymorphRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_color = gl.compile_program(CM_COLOR_VERT, CM_COLOR_FRAG)?;
        let prog_clay = gl.compile_program(CM_CLAY_VERT, CM_CLAY_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

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

        let mut clay_vao = 0;
        let mut clay_vbo = 0;
        (gl.GenVertexArrays)(1, &mut clay_vao);
        (gl.GenBuffers)(1, &mut clay_vbo);
        (gl.BindVertexArray)(clay_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, clay_vbo);
        let stride = 8;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride, 0 as *const c_void);
        (gl.BindVertexArray)(0);

        Ok(Self {
            prog_color,
            prog_clay,
            dummy_vao,
            color_vao,
            color_vbo,
            batch: ColorBatch::new(),
            clay_vao,
            clay_vbo,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.color_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.color_vbo);
            self.color_vbo = 0;
        }
        if self.color_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.color_vao);
            self.color_vao = 0;
        }
        if self.clay_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.clay_vbo);
            self.clay_vbo = 0;
        }
        if self.clay_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.clay_vao);
            self.clay_vao = 0;
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
        for p in [self.prog_color, self.prog_clay] {
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
        (gl.BufferData)(GL_ARRAY_BUFFER, bytes, self.batch.verts.as_ptr() as *const c_void, GL_DYNAMIC_DRAW);
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.batch.verts.len() as GLsizei);
        self.batch.clear();
    }

    // Draw one clay body (orb / pill / ring) with per-frame SDF shading.
    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_clay(
        &mut self,
        gl: &GlFns,
        cx: f32,
        cy: f32,
        hw: f32,
        hh: f32,
        radius: f32,
        rot: f32,
        sx: f32,
        sy: f32,
        is_orb: bool,
        is_ring: bool,
        ring_t: f32,
        alpha: f32,
        color: &ClayColor,
        puff: f32,
        res_w: f32,
        res_h: f32,
    ) {
        // Bounding extent in screen space (covers rotated/scaled shape + shadow pad).
        let base = hw.max(hh);
        let ext = (base * sx.max(sy) + 30.0 * puff + 18.0).min(res_w.max(res_h));
        let x0 = cx - ext;
        let y0 = cy - ext;
        let x1 = cx + ext;
        let y1 = cy + ext;
        let verts = [
            x0, y0, x1, y0, x1, y1, x0, y0, x1, y1, x0, y1,
        ];
        (gl.UseProgram)(self.prog_clay);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_clay, "u_res"), res_w, res_h);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_clay, "u_center"), cx, cy);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_clay, "u_half"), hw, hh);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_clay, "u_radius"), radius);
        let (sc, ss) = rot.sin_cos();
        // local = S(1/sx,1/sy) * R(-rot) * delta ; columns (m00,m10,m01,m11)
        (gl.Uniform4f)(gl.uniform_loc(self.prog_clay, "u_xform"), sc / sx, -ss / sy, ss / sx, sc / sy);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_clay, "u_is_orb"), if is_orb { 1 } else { 0 });
        (gl.Uniform1i)(gl.uniform_loc(self.prog_clay, "u_is_ring"), if is_ring { 1 } else { 0 });
        (gl.Uniform1f)(gl.uniform_loc(self.prog_clay, "u_ringT"), ring_t);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_clay, "u_alpha"), alpha);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_clay, "u_puff"), puff);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_clay, "u_hi"), color.hi[0], color.hi[1], color.hi[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_clay, "u_mid"), color.mid[0], color.mid[1], color.mid[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_clay, "u_lo"), color.lo[0], color.lo[1], color.lo[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_clay, "u_spec"), color.spec[0], color.spec[1], color.spec[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_clay, "u_rim"), color.rim[0], color.rim[1], color.rim[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_clay, "u_drop"), color.drop[0], color.drop[1], color.drop[2]);
        (gl.BindVertexArray)(self.clay_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.clay_vbo);
        (gl.BufferData)(GL_ARRAY_BUFFER, (verts.len() * 4) as GLsizeiptr, verts.as_ptr() as *const c_void, GL_DYNAMIC_DRAW);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 6);
    }

    // 1:1 renderStaticBackground()
    unsafe fn render_background(&mut self, gl: &GlFns, w: f32, h: f32, st: &ClaymorphState) {
        let pal = st.palette();
        self.batch.clear();
        // Opaque base vertical gradient
        self.batch.add_rect_vgrad(
            0.0,
            0.0,
            w,
            h,
            [pal.bg_top[0], pal.bg_top[1], pal.bg_top[2], 1.0],
            [pal.bg_bot[0], pal.bg_bot[1], pal.bg_bot[2], 1.0],
        );
        self.flush_batch(gl, w, h);

        // Translucent corner washes (must alpha-blend, not overwrite)
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        self.batch.clear();
        let c0 = &pal.clays[0];
        let wash1_a = if pal.is_dark { 0.14 } else { 0.10 };
        self.batch.add_rect_radial(
            w * 0.18,
            h * 0.22,
            w.max(h) * 0.46,
            [c0.drop[0], c0.drop[1], c0.drop[2], wash1_a],
            28,
        );
        let c1 = &pal.clays[1];
        let wash2_a = if pal.is_dark { 0.12 } else { 0.09 };
        self.batch.add_rect_radial(
            w * 0.82,
            h * 0.78,
            w.max(h) * 0.44,
            [c1.drop[0], c1.drop[1], c1.drop[2], wash2_a],
            28,
        );
        self.flush_batch(gl, w, h);
        (gl.Disable)(GL_BLEND);
    }

    pub unsafe fn render(&mut self, gl: &GlFns, app: &mut AppState) {
        let pw = app.width_px.max(64) as f32;
        let ph = app.height_px.max(64) as f32;
        let css_w = app.css_w();
        let css_h = app.css_h();

        let st = &mut app.claymorph;
        st.resize(css_w, css_h);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw as i32, ph as i32);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);

        // Static gradient + corner washes
        self.render_background(gl, css_w, css_h, st);

        // Floating clay blobs (premultiplied)
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
        let puff = st.p.puffiness.clamp(0.4, 1.6);
        for b in &st.blobs {
            let color = &st.palette().clays[b.color_idx % 5];
            let sx = 1.0 + b.squash;
            let sy = 1.0 / (1.0 + b.squash).max(0.74);
            let rot = if b.kind == 1 { b.rot } else { 0.0 };
            let hw = b.w * 0.5;
            let hh = b.h * 0.5;
            self.draw_clay(
                gl, b.x, b.y, hw, hh, b.r, rot, sx, sy,
                b.kind == 0, false, 0.0, b.depth_alpha, color, puff, css_w, css_h,
            );
        }

        // Soft expanding click ripples (straight alpha)
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        self.batch.clear();
        let pal = st.palette();
        for rp in &st.ripples {
            let fade = (1.0 - rp.r / rp.max_r) * st.p.squish_strength.clamp(0.0, 1.4);
            if fade <= 0.01 {
                continue;
            }
            let a = fade * if pal.is_dark { 0.14 } else { 0.34 };
            let segs = 48usize;
            for i in 0..segs {
                let a0 = (i as f32 / segs as f32) * TAU;
                let a1 = ((i + 1) as f32 / segs as f32) * TAU;
                let p0 = [rp.x - 1.5 + a0.cos() * rp.r, rp.y - 1.5 + a0.sin() * rp.r];
                let p1 = [rp.x - 1.5 + a1.cos() * rp.r, rp.y - 1.5 + a1.sin() * rp.r];
                self.batch.push_tri([rp.x - 1.5, rp.y - 1.5], p0, p1, [1.0, 1.0, 1.0, a]);
            }
            let sa = fade * 0.24;
            for i in 0..segs {
                let a0 = (i as f32 / segs as f32) * TAU;
                let a1 = ((i + 1) as f32 / segs as f32) * TAU;
                let p0 = [rp.x + 1.5 + a0.cos() * rp.r, rp.y + 1.5 + a0.sin() * rp.r];
                let p1 = [rp.x + 1.5 + a1.cos() * rp.r, rp.y + 1.5 + a1.sin() * rp.r];
                self.batch.push_tri([rp.x + 1.5, rp.y + 1.5], p0, p1, [pal.shadow_rgb[0], pal.shadow_rgb[1], pal.shadow_rgb[2], sa]);
            }
        }
        self.flush_batch(gl, css_w, css_h);

        // Adaptive desktop icon clay cradles
        if st.p.adaptive != 0 {
            let is_ring = st.p.icon_cradle == 2;
            let use_neutral = st.p.icon_cradle == 1;
            (gl.Enable)(GL_BLEND);
            (gl.BlendFunc)(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            for ic in &st.icon_states {
                if ic.opacity <= 0.01 {
                    continue;
                }
                let cw = (ic.w + 20.0).round();
                let ch = (ic.h + 16.0).round();
                let cr = ((cw.min(ch)) * 0.28).round().clamp(16.0, 28.0);
                let ring_t = (cw.min(ch) * 0.14).max(8.0);
                let color = if use_neutral { &pal.neutral } else { &pal.clays[ic.color_idx % 5] };
                let sx = 1.0 + ic.squash;
                let sy = 1.0 / (1.0 + ic.squash).max(0.80);
                self.draw_clay(
                    gl,
                    ic.x + ic.w * 0.5,
                    ic.y + ic.h * 0.5,
                    cw * 0.5,
                    ch * 0.5,
                    cr,
                    0.0,
                    sx,
                    sy,
                    false,
                    is_ring,
                    ring_t,
                    ic.opacity,
                    color,
                    puff,
                    css_w,
                    css_h,
                );
            }

            // Soft contrast label pill beneath white OS icon labels on light backdrops
            if !pal.is_dark && !is_ring {
                (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
                self.batch.clear();
                for ic in &st.icon_states {
                    if ic.opacity <= 0.01 {
                        continue;
                    }
                    let cw = ic.w + 20.0;
                    let ch = ic.h + 16.0;
                    let lbl_w = (cw - 14.0).min((ic.w * 0.84).max(44.0));
                    let lbl_h = 14.0;
                    let lbl_x = ic.x + ic.w * 0.5 - lbl_w * 0.5;
                    let lbl_y = ic.y + ic.h * 0.5 + ch * 0.5 - lbl_h - 7.0;
                    let a = pal.label_pill[3] * ic.opacity;
                    self.batch.add_round_rect(lbl_x, lbl_y, lbl_w, lbl_h, lbl_h * 0.5, [pal.label_pill[0], pal.label_pill[1], pal.label_pill[2], a]);
                }
                self.flush_batch(gl, css_w, css_h);
            }

            // Pastel clay bead accent on 'cloud' pillow mode
            if use_neutral {
                (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
                self.batch.clear();
                for ic in &st.icon_states {
                    if ic.opacity <= 0.01 {
                        continue;
                    }
                    let cw = ic.w + 20.0;
                    let ch = ic.h + 16.0;
                    let bead = &pal.clays[ic.color_idx % 5];
                    let breath = 0.72 + 0.25 * (st.sim_time * 1.4 + ic.phase).sin();
                    let bx = ic.x + ic.w * 0.5 + cw * 0.5 - 12.0;
                    let by = ic.y + ic.h * 0.5 - ch * 0.5 + 12.0;
                    self.batch.add_circle(bx, by, 3.8, 12, [bead.mid[0], bead.mid[1], bead.mid[2], ic.opacity * breath]);
                }
                self.flush_batch(gl, css_w, css_h);
            }
        }
        (gl.Disable)(GL_BLEND);
    }
}

// =========================================================================
// 6. rust-anything Recorded Ground-Truth Cases (1:1 claymorph.html pure logic)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
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

    fn blobs_snapshot(blobs: &[ClayBlob]) -> Value {
        let arr: Vec<Value> = blobs
            .iter()
            .map(|b| {
                json!({
                    "kind": b.kind,
                    "w": b.w,
                    "h": b.h,
                    "r": b.r,
                    "color": b.color_idx,
                    "hx": b.home_x,
                    "hy": b.home_y,
                    "amp_x": b.bob_amp_x,
                    "amp_y": b.bob_amp_y,
                    "freq_x": b.bob_freq_x,
                    "freq_y": b.bob_freq_y,
                    "phase": b.phase,
                    "rot": b.rot,
                    "rot_speed": b.rot_speed,
                    "depth": b.depth_alpha,
                })
            })
            .collect();
        json!({ "count": blobs.len(), "blobs": arr })
    }

    fn build_blob_case(blob_count: f32, quality: usize, seed: u32, w: f32, h: f32) -> Value {
        let mut st = ClaymorphState::default();
        st.p.blob_count = blob_count;
        st.p.quality = quality;
        st.p.seed = seed;
        st.gw = w;
        st.gh = h;
        st.build_blobs();
        blobs_snapshot(&st.blobs)
    }

    fn icon_sync_case(rects: &[Rect]) -> Value {
        let mut st = ClaymorphState::default();
        st.p.adaptive = 1;
        st.gw = 1920.0;
        st.gh = 1080.0;
        st.icon_states.clear();
        st.sync_incoming_icons(rects);
        let states: Vec<Value> = st
            .icon_states
            .iter()
            .map(|s| {
                json!({
                    "x": s.x, "y": s.y, "w": s.w, "h": s.h,
                    "tx": s.tx, "ty": s.ty, "tw": s.tw, "th": s.th,
                    "color": s.color_idx, "opacity": s.opacity, "phase": s.phase,
                })
            })
            .collect();
        json!({ "count": states.len(), "states": states })
    }

    fn step_case(steps: usize) -> Value {
        let mut st = ClaymorphState::default();
        st.p.adaptive = 1;
        st.p.seed = 20261118;
        st.gw = 1920.0;
        st.gh = 1080.0;
        st.build_blobs();
        let icons = [
            Rect { x: 80.0, y: 100.0, w: 76.0, h: 90.0 },
            Rect { x: 200.0, y: 100.0, w: 76.0, h: 90.0 },
        ];
        st.sync_incoming_icons(&icons);
        let mut snaps = Vec::new();
        for i in 0..steps {
            st.step(0.033, &icons, (300.0, 300.0, i % 2 == 0, i % 3 == 0));
            if i == 1 || i == steps - 1 {
                let blob_arr: Vec<Value> = st
                    .blobs
                    .iter()
                    .take(4)
                    .map(|b| json!([b.x, b.y, b.vx, b.vy, b.squash]))
                    .collect();
                let icon_arr: Vec<Value> = st
                    .icon_states
                    .iter()
                    .map(|s| json!([s.x, s.y, s.w, s.h, s.squash, s.opacity]))
                    .collect();
                snaps.push(json!({ "step": i, "blobs": blob_arr, "icons": icon_arr }));
            }
        }
        json!({ "blobs_total": st.blobs.len(), "snaps": snaps })
    }

    // Records cases.jsonl when DIYARDESK_RECORD_CLAYMORPH=1; otherwise verifies.
    #[test]
    fn record_or_verify_claymorph_cases() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir.parent().and_then(|p| p.parent()).expect("repo root");
        let rec_path = manifest_dir.join("src/claymorph/recorded-from.json");
        let cases_path = manifest_dir.join("src/claymorph/cases.jsonl");

        let rec_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&rec_path).expect("read recorded-from.json"))
                .expect("parse recorded-from.json");
        let unit_rel = rec_json["unit"].as_str().expect("unit path");
        let expected_fp = rec_json["fingerprint"].as_str().expect("fingerprint");
        let html_bytes = std::fs::read(repo_root.join(unit_rel)).expect("read prototype/claymorph.html");
        let actual_fp = fnv1a_fingerprint(unit_rel, &html_bytes);
        assert_eq!(actual_fp, expected_fp, "rust-anything gate: prototype/claymorph.html changed since recording");

        let record = std::env::var("DIYARDESK_RECORD_CLAYMORPH").is_ok();
        if record {
            let mut out = String::new();
            let configs = [
                (1.0f32, 2usize, 20261118u32),
                (1.8, 0, 42),
                (0.0, 1, 7),
                (0.5, 2, 123456),
            ];
            for &(bc, q, sd) in &configs {
                let snap = build_blob_case(bc, q, sd, 1920.0, 1080.0);
                out.push_str(&json!({ "fn": "buildBlobsLayout", "args": [bc, q, sd, 1920.0, 1080.0], "result": { "ok": snap } }).to_string());
                out.push('\n');
            }

            let icon_sets: [&[Rect]; 3] = [
                &[Rect { x: 80.0, y: 100.0, w: 76.0, h: 90.0 }, Rect { x: 200.0, y: 100.0, w: 76.0, h: 90.0 }],
                &[Rect { x: 110.0, y: 120.0, w: 80.0, h: 86.0 }],
                &[],
            ];
            for set in &icon_sets {
                let snap = icon_sync_case(set);
                let rects: Vec<Value> = set.iter().map(|r| json!([r.x, r.y, r.w, r.h])).collect();
                out.push_str(&json!({ "fn": "syncIncomingIcons", "args": [rects], "result": { "ok": snap } }).to_string());
                out.push('\n');
            }

            for &steps in &[2usize, 20, 60] {
                let snap = step_case(steps);
                out.push_str(&json!({ "fn": "stepSimulation", "args": [steps], "result": { "ok": snap } }).to_string());
                out.push('\n');
            }

            std::fs::write(&cases_path, out).expect("write cases.jsonl");
            let count = std::fs::read_to_string(&cases_path).unwrap().lines().filter(|l| !l.trim().is_empty()).count();
            println!("Recorded {count} claymorph cases to cases.jsonl");
            return;
        }

        // Verify mode: replay recorded cases against the Rust implementation.
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
                "buildBlobsLayout" => {
                    let bc = args[0].as_f64().unwrap() as f32;
                    let q = args[1].as_u64().unwrap() as usize;
                    let sd = args[2].as_u64().unwrap() as u32;
                    let w = args[3].as_f64().unwrap() as f32;
                    let h = args[4].as_f64().unwrap() as f32;
                    let snap = build_blob_case(bc, q, sd, w, h);
                    assert_eq!(snap["count"].as_u64().unwrap() as i64, ok["count"].as_i64().unwrap(), "case #{line_no} blob count");
                    for (i, ev) in ok["blobs"].as_array().unwrap().iter().enumerate() {
                        let gv = &snap["blobs"][i];
                        assert_eq!(gv["kind"].as_i64().unwrap(), ev["kind"].as_i64().unwrap(), "case #{line_no} blob {i} kind");
                        assert_eq!(gv["color"].as_i64().unwrap(), ev["color"].as_i64().unwrap(), "case #{line_no} blob {i} color");
                        for k in ["w", "h", "r", "hx", "hy", "amp_x", "amp_y", "freq_x", "freq_y", "phase", "rot", "rot_speed", "depth"] {
                            let g = gv[k].as_f64().unwrap() as f32;
                            let e = ev[k].as_f64().unwrap() as f32;
                            assert!((g - e).abs() <= 1e-3, "case #{line_no} blob {i} {k}: got {g}, exp {e}");
                        }
                    }
                }
                "syncIncomingIcons" => {
                    let rects_v = args[0].as_array().unwrap();
                    let rects: Vec<Rect> = rects_v
                        .iter()
                        .map(|r| {
                            let a = r.as_array().unwrap();
                            Rect { x: a[0].as_f64().unwrap() as f32, y: a[1].as_f64().unwrap() as f32, w: a[2].as_f64().unwrap() as f32, h: a[3].as_f64().unwrap() as f32 }
                        })
                        .collect();
                    let snap = icon_sync_case(&rects);
                    assert_eq!(snap["count"].as_u64().unwrap() as i64, ok["count"].as_i64().unwrap(), "case #{line_no} icon count");
                    for (i, ev) in ok["states"].as_array().unwrap().iter().enumerate() {
                        let gv = &snap["states"][i];
                        for k in ["x", "y", "w", "h", "tx", "ty", "tw", "th", "phase"] {
                            let g = gv[k].as_f64().unwrap() as f32;
                            let e = ev[k].as_f64().unwrap() as f32;
                            assert!((g - e).abs() <= 1e-3, "case #{line_no} icon {i} {k}: got {g}, exp {e}");
                        }
                    }
                }
                "stepSimulation" => {
                    let steps = args[0].as_u64().unwrap() as usize;
                    let snap = step_case(steps);
                    assert_eq!(snap["blobs_total"].as_u64().unwrap() as i64, ok["blobs_total"].as_i64().unwrap());
                    for (si, ev) in ok["snaps"].as_array().unwrap().iter().enumerate() {
                        let gv = &snap["snaps"][si];
                        for (bi, bev) in ev["blobs"].as_array().unwrap().iter().enumerate() {
                            let gba = gv["blobs"][bi].as_array().unwrap();
                            let eba = bev.as_array().unwrap();
                            for k in 0..5 {
                                let g = gba[k].as_f64().unwrap() as f32;
                                let e = eba[k].as_f64().unwrap() as f32;
                                assert!((g - e).abs() <= 1e-2, "case #{line_no} step snap {si} blob {bi}[{k}]: got {g}, exp {e}");
                            }
                        }
                    }
                }
                other => panic!("Unknown recorded function: {other}"),
            }
            passed += 1;
        }
        assert!(passed > 0, "No claymorph recorded cases found; run with DIYARDESK_RECORD_CLAYMORPH=1");
    }
}
