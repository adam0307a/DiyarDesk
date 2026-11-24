pub mod atlas;
pub mod shaders;

use crate::gl::*;
use crate::meadow::Mulberry32;
use crate::state::{AppState, Rect};
use atlas::{build_roach_atlas, ROACH_CELL, ROACH_COLS, ROACH_ROWS};
use shaders::{ROACH_BG_FS, ROACH_BG_VS, ROACH_INST_FS, ROACH_INST_VS};
use std::f64::consts::PI;
use std::ffi::c_void;

pub const NR: usize = 256;
pub const STEP: f64 = 4.0;
const MORPH_DUR: f64 = 0.28;
const GL_UNSIGNED_SHORT: GLenum = 0x1403;
const GL_R16UI: GLenum = 0x8234;
const GL_RED_INTEGER: GLenum = 0x8D94;

#[derive(Clone, Copy, PartialEq)]
pub struct RoachParams {
    pub sigma: f64,
    pub warp: f64,
    pub glow: f64,
    pub crowd: usize,
    pub speed: f64,
    pub props: f64,
    pub vibe: f64,
    pub adaptive: bool,
    pub seed: i32,
    pub debug: bool,
}

impl Default for RoachParams {
    fn default() -> Self {
        Self {
            sigma: 78.0,
            warp: 24.0,
            glow: 0.78,
            crowd: 12,
            speed: 1.0,
            props: 0.9,
            vibe: 0.0,
            adaptive: true,
            seed: 41297,
            debug: false,
        }
    }
}

#[inline]
pub fn hash2(x: i32, y: i32, s: i32) -> f64 {
    let mut h = x.wrapping_mul(374_761_393)
        ^ y.wrapping_mul(668_265_263)
        ^ s.wrapping_mul(1_442_695_041);
    h = (h ^ ((h as u32 >> 13) as i32)).wrapping_mul(1_274_126_177);
    (((h ^ ((h as u32 >> 16) as i32)) as u32) as f64) / 4_294_967_296.0
}

#[inline]
pub fn r_hash(a: i32, b: i32, k: i32, seed: i32) -> f64 {
    let mut h = a.wrapping_add(19).wrapping_mul(374_761_393)
        ^ b.wrapping_add(41).wrapping_mul(668_265_263)
        ^ seed
            .wrapping_add(k.wrapping_mul(31))
            .wrapping_mul(2_246_822_519u32 as i32);
    h = (h ^ ((h as u32 >> 13) as i32)).wrapping_mul(1_274_126_177);
    (((h ^ ((h as u32 >> 16) as i32)) as u32) as f64) / 4_294_967_296.0
}

pub fn build_roach_noise(seed: i32, noise: &mut [f32], noise_rgba: &mut [u8]) {
    let mut g = vec![0.0f32; NR * NR];
    for i in 0..(NR * NR) {
        g[i] = hash2((i & (NR - 1)) as i32, (i >> 8) as i32, seed) as f32;
    }
    let n1 = |x: f64, y: f64, per: i32| -> f64 {
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let fx = x - ix as f64;
        let fy = y - iy as f64;
        let ux = fx * fx * (3.0 - 2.0 * fx);
        let uy = fy * fy * (3.0 - 2.0 * fy);
        let x0 = ix.rem_euclid(per) as usize;
        let x1 = ((x0 as i32 + 1).rem_euclid(per)) as usize;
        let y0 = iy.rem_euclid(per) as usize;
        let y1 = ((y0 as i32 + 1).rem_euclid(per)) as usize;
        let a = g[(y0 << 8) | x0] as f64;
        let b = g[(y0 << 8) | x1] as f64;
        let c = g[(y1 << 8) | x0] as f64;
        let d = g[(y1 << 8) | x1] as f64;
        (a + (b - a) * ux) * (1.0 - uy) + (c + (d - c) * ux) * uy
    };
    for y in 0..NR {
        for x in 0..NR {
            let mut f = 0.0f64;
            let mut amp = 0.5f64;
            let mut fr = 1i32;
            for _ in 0..4 {
                let per = 8 * fr;
                f += amp * n1(x as f64 * per as f64 / NR as f64, y as f64 * per as f64 / NR as f64, per);
                amp *= 0.5;
                fr *= 2;
            }
            let idx = y * NR + x;
            noise[idx] = f as f32;
            noise_rgba[idx * 4] = (f * 255.0).round().clamp(0.0, 255.0) as u8;
            noise_rgba[idx * 4 + 1] = (g[idx] as f64 * 255.0).round().clamp(0.0, 255.0) as u8;
            noise_rgba[idx * 4 + 2] = 0;
            noise_rgba[idx * 4 + 3] = 255;
        }
    }
}

#[inline]
pub fn sample_roach_noise(noise: &[f32], x: f64, y: f64) -> f64 {
    let nr = NR as f64;
    let fx = ((x % nr) + nr) % nr;
    let fy = ((y % nr) + nr) % nr;
    let ix = (fx as i32).clamp(0, NR as i32 - 1) as usize;
    let iy = (fy as i32).clamp(0, NR as i32 - 1) as usize;
    noise[(iy << 8) | ix] as f64
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallPhase {
    Idle,
    DivaProvoke,
    Raising,
    Calling,
    Lowering,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct RoachMember {
    pub id: usize,
    pub kind: usize,
    pub col: usize,
    pub row: usize,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub base_speed: f64,
    pub phase: f64,
    pub stop_timer: f64,
    pub charge_timer: f64,
    pub charge_cooldown: f64,
    pub target_outlet: Option<usize>,
    pub z: f64,
    pub vz: f64,
    pub air_rot: f64,
    pub spin_vel: f64,
    pub dragged: bool,
    pub in_bed: Option<usize>,
    pub bed_timer: f64,
    pub bed_cooldown: f64,
    pub call_phase: CallPhase,
    pub call_timer: f64,
    pub call_mood: f64,
    pub move_timer: f64,
    pub facing: i32,
    pub size: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StreetPropType {
    Bed,
    Outlet,
    Prop,
}

#[derive(Clone, Debug)]
pub struct StreetProp {
    pub prop_type: StreetPropType,
    pub x: f64,
    pub y: f64,
    pub col: usize,
    pub row: usize,
    pub size: f64,
    pub occupant: Option<usize>,
    pub phase: f64,
    pub shout_timer: f64,
}

#[derive(Clone, Debug)]
pub struct RoachBubble {
    pub x: f64,
    pub y: f64,
    pub age: f64,
    pub dur: f64,
    pub col: usize,
    pub row: usize,
}

#[derive(Clone, Debug)]
pub struct JetTarget {
    pub tx: f64,
    pub ty: f64,
    pub dropped: bool,
}

#[derive(Clone, Debug)]
pub struct JetStrike {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub rot: f64,
    pub targets: Vec<JetTarget>,
}

#[derive(Clone, Debug)]
pub struct RoachBomb {
    pub sx: f64,
    pub sy: f64,
    pub tx: f64,
    pub ty: f64,
    pub age: f64,
    pub dur: f64,
}

#[derive(Clone, Debug)]
pub struct RoachExplosion {
    pub x: f64,
    pub y: f64,
    pub age: f64,
    pub dur: f64,
    pub size: f64,
}

#[derive(Clone, Debug)]
pub struct RoachDragState {
    pub roach_idx: usize,
    pub off_x: f64,
    pub off_y: f64,
    pub last_x: f64,
    pub last_y: f64,
    pub last_t: f64,
    pub vx: f64,
    pub vy: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InstSprite {
    x: f32,
    y: f32,
    size: f32,
    rot: f32,
    col: f32,
    row: f32,
    alpha: f32,
}

pub struct RoachState {
    pub params: RoachParams,
    pub w: f64,
    pub h: f64,
    pub gw: usize,
    pub gh: usize,
    pub noise: Vec<f32>,
    pub noise_rgba: Vec<u8>,
    pub fa: Vec<u16>,
    pub fb: Vec<u16>,
    pub fa_dirty: bool,
    pub fb_dirty: bool,
    pub noise_tex_dirty: bool,
    pub morph_elapsed: f64,
    pub cur_mix: f64,
    pub sim_time: f64,
    pub raw_icons: Vec<Rect>,
    pub icon_boxes: Vec<[f64; 4]>,
    pub icons_visible: bool,
    pub has_committed_once: bool,
    pub roaches: Vec<RoachMember>,
    pub street_props: Vec<StreetProp>,
    pub bubbles: Vec<RoachBubble>,
    pub jet_strikes: Vec<JetStrike>,
    pub bombs: Vec<RoachBomb>,
    pub explosions: Vec<RoachExplosion>,
    pub bubble_timer: f64,
    pub bubble_cooldown: f64,
    pub drag_state: Option<RoachDragState>,
    pub rng: Mulberry32,
}

impl Default for RoachState {
    fn default() -> Self {
        let params = RoachParams::default();
        let mut s = Self {
            params,
            w: 1920.0,
            h: 1080.0,
            gw: 480,
            gh: 270,
            noise: vec![0.0; NR * NR],
            noise_rgba: vec![0u8; NR * NR * 4],
            fa: Vec::new(),
            fb: Vec::new(),
            fa_dirty: true,
            fb_dirty: true,
            noise_tex_dirty: true,
            morph_elapsed: MORPH_DUR,
            cur_mix: 1.0,
            sim_time: 0.0,
            raw_icons: Vec::new(),
            icon_boxes: Vec::new(),
            icons_visible: true,
            has_committed_once: false,
            roaches: Vec::new(),
            street_props: Vec::new(),
            bubbles: Vec::new(),
            jet_strikes: Vec::new(),
            bombs: Vec::new(),
            explosions: Vec::new(),
            bubble_timer: 0.0,
            bubble_cooldown: 0.0,
            drag_state: None,
            rng: Mulberry32::new(params.seed),
        };
        build_roach_noise(s.params.seed, &mut s.noise, &mut s.noise_rgba);
        s.resize(1920.0, 1080.0);
        s.commit(true);
        s
    }
}

impl RoachState {
    pub fn resize(&mut self, w_f32: f32, h_f32: f32) {
        let w = (w_f32 as f64).max(32.0);
        let h = (h_f32 as f64).max(32.0);
        let gw = ((w / STEP).ceil() as usize).max(8);
        let gh = ((h / STEP).ceil() as usize).max(8);
        let changed = (self.w - w).abs() > 0.5 || (self.h - h).abs() > 0.5 || self.gw != gw || self.gh != gh;
        self.w = w;
        self.h = h;
        self.gw = gw;
        self.gh = gh;
        let len = self.gw * self.gh;
        self.fa.resize(len, 65535);
        self.fb.resize(len, 65535);
        if changed && self.has_committed_once {
            self.refresh();
        }
    }

    pub fn sync_params(&mut self, next: RoachParams) {
        let seed_changed = next.seed != self.params.seed;
        let adaptive_changed = next.adaptive != self.params.adaptive;
        let field_changed =
            (next.sigma - self.params.sigma).abs() > 1e-4 || (next.warp - self.params.warp).abs() > 1e-4;
        let crowd_changed =
            next.crowd != self.params.crowd || (next.props - self.params.props).abs() > 1e-4;
        self.params = next;
        if seed_changed {
            self.reseed(next.seed);
            return;
        }
        if adaptive_changed {
            self.commit(false);
        } else if field_changed {
            self.refresh();
        } else if crowd_changed {
            self.sync_roach_props();
        }
    }

    pub fn reseed(&mut self, new_seed: i32) {
        self.params.seed = new_seed;
        self.rng = Mulberry32::new(new_seed);
        build_roach_noise(new_seed, &mut self.noise, &mut self.noise_rgba);
        self.noise_tex_dirty = true;
        self.commit(true);
    }

    pub fn compute_field_into(&self, out: &mut [u16]) {
        let s = self.params.sigma;
        let w = self.params.warp;
        let boxes: &[[f64; 4]] = if self.params.adaptive {
            &self.icon_boxes
        } else {
            &[]
        };
        for gy in 0..self.gh {
            let py = gy as f64 * STEP + STEP * 0.5;
            for gx in 0..self.gw {
                let px = gx as f64 * STEP + STEP * 0.5;
                let mut md2 = 1e9f64;
                for b in boxes {
                    let dx = if px < b[0] {
                        b[0] - px
                    } else if px > b[2] {
                        px - b[2]
                    } else {
                        0.0
                    };
                    let dy = if py < b[1] {
                        b[1] - py
                    } else if py > b[3] {
                        py - b[3]
                    } else {
                        0.0
                    };
                    let d2 = dx * dx + dy * dy;
                    if d2 < md2 {
                        md2 = d2;
                    }
                }
                let d = if boxes.is_empty() {
                    120.0
                } else if md2 == 0.0 {
                    let mut inside = 1e9f64;
                    for b in boxes {
                        if px >= b[0] && px <= b[2] && py >= b[1] && py <= b[3] {
                            inside = inside
                                .min(px - b[0])
                                .min(b[2] - px)
                                .min(py - b[1])
                                .min(b[3] - py);
                        }
                    }
                    -inside
                } else {
                    md2.sqrt()
                };
                let n = (sample_roach_noise(&self.noise, px * 0.24, py * 0.24) - 0.5) * 2.0 * w;
                let val = ((d + n - s * 0.72) / (s * 0.72)).clamp(-1.0, 1.0);
                out[gy * self.gw + gx] = ((val * 0.5 + 0.5) * 65535.0).round() as u16;
            }
        }
    }

    #[inline]
    pub fn sample_field(&self, px: f64, py: f64) -> f64 {
        if self.fb.is_empty() || self.gw == 0 || self.gh == 0 {
            return 1.0;
        }
        let gx = ((px / STEP).round() as i32).clamp(0, self.gw as i32 - 1) as usize;
        let gy = ((py / STEP).round() as i32).clamp(0, self.gh as i32 - 1) as usize;
        (self.fb[gy * self.gw + gx] as f64 / 65535.0) * 2.0 - 1.0
    }

    #[inline]
    pub fn walkable_gradient(&self, px: f64, py: f64) -> (f64, f64) {
        let e = 8.0;
        let gx = self.sample_field(px + e, py) - self.sample_field(px - e, py);
        let gy = self.sample_field(px, py + e) - self.sample_field(px, py - e);
        let len = gx.hypot(gy).max(0.001);
        (gx / len, gy / len)
    }

    pub fn commit(&mut self, snap: bool) {
        if self.gw == 0 || self.gh == 0 {
            return;
        }
        let mut next_fb = vec![0u16; self.gw * self.gh];
        self.compute_field_into(&mut next_fb);
        if snap || !self.has_committed_once {
            self.fb = next_fb;
            self.fa.copy_from_slice(&self.fb);
            self.fa_dirty = true;
            self.fb_dirty = true;
            self.cur_mix = 1.0;
            self.morph_elapsed = MORPH_DUR;
            self.has_committed_once = true;
            self.rebuild_roach_props();
            self.sync_roach_props();
        } else {
            let mix = self.cur_mix;
            for i in 0..self.fa.len() {
                let v = self.fa[i] as f64 * (1.0 - mix) + self.fb[i] as f64 * mix;
                self.fa[i] = v.round().clamp(0.0, 65535.0) as u16;
            }
            self.fb = next_fb;
            self.fa_dirty = true;
            self.fb_dirty = true;
            self.cur_mix = 0.0;
            self.morph_elapsed = 0.0;
            self.sync_roach_props();
        }
    }

    pub fn refresh(&mut self) {
        if self.gw == 0 || self.gh == 0 {
            return;
        }
        let mut next_fb = vec![0u16; self.gw * self.gh];
        self.compute_field_into(&mut next_fb);
        self.fb = next_fb;
        self.fa.copy_from_slice(&self.fb);
        self.fa_dirty = true;
        self.fb_dirty = true;
        self.cur_mix = 1.0;
        self.morph_elapsed = MORPH_DUR;
        self.sync_roach_props();
    }

    pub fn set_icons(&mut self, rects: &[Rect], visible: bool) {
        self.raw_icons = rects.to_vec();
        self.icons_visible = visible;
        self.icon_boxes.clear();
        if visible {
            for r in rects {
                let x = r.x as f64;
                let y = r.y as f64;
                let w = r.w as f64;
                let h = r.h as f64;
                self.icon_boxes.push([x - 4.0, y - 4.0, x + w + 4.0, y + h + 4.0]);
            }
        }
        let first = !self.has_committed_once;
        self.commit(first);
    }

    pub fn spawn_roach_member(&self, i: usize) -> RoachMember {
        const CAST_ORDER: [usize; 19] = [
            16, 11, 12, 13, 15, 0, 5, 8, 9, 1, 2, 4, 6, 12, 15, 3, 7, 10, 11,
        ];
        let raw = if i < CAST_ORDER.len() {
            CAST_ORDER[i]
        } else {
            i % 17
        };
        let kind = if raw == 14 { 12 } else { raw };
        let col = if kind == 16 {
            4
        } else if kind < 8 {
            kind
        } else {
            kind - 8
        };
        let row = if kind == 16 {
            3
        } else if kind < 8 {
            0
        } else {
            1
        };

        let seed = self.params.seed;
        let mut sx = self.w * (0.1 + 0.8 * r_hash(i as i32, 11, 1, seed));
        let mut sy = self.h * (0.12 + 0.76 * r_hash(i as i32, 23, 2, seed));
        if !self.fb.is_empty() {
            let max_x_frac = if i < 6 { 0.68 } else { 0.88 };
            for t in 0..50 {
                let tx = self.w * (0.06 + (max_x_frac - 0.06) * r_hash(i as i32, t * 7 + 3, 3, seed));
                let ty = self.h * (0.08 + 0.84 * r_hash(i as i32, t * 13 + 5, 4, seed));
                if self.sample_field(tx, ty) > 0.06 {
                    sx = tx;
                    sy = ty;
                    break;
                }
            }
        }
        let ang = r_hash(i as i32, 37, 5, seed) * PI * 2.0;
        let speed = (if kind == 12 || kind == 13 { 34.0 } else { 26.0 })
            + r_hash(i as i32, 43, 6, seed) * 20.0;
        let size = if kind == 10 {
            74.0
        } else if kind == 9 {
            94.0
        } else {
            90.0
        };
        RoachMember {
            id: i,
            kind,
            col,
            row,
            x: sx,
            y: sy,
            vx: ang.cos() * speed,
            vy: ang.sin() * speed,
            base_speed: speed,
            phase: r_hash(i as i32, 59, 7, seed) * 6.28,
            stop_timer: 0.0,
            charge_timer: 0.0,
            charge_cooldown: if i < 4 { 0.1 } else { 1.5 },
            target_outlet: None,
            z: 0.0,
            vz: 0.0,
            air_rot: 0.0,
            spin_vel: 0.0,
            dragged: false,
            in_bed: None,
            bed_timer: 0.0,
            bed_cooldown: if kind == 12 || kind == 13 {
                2.5 + (i % 3) as f64 * 2.5
            } else {
                0.0
            },
            call_phase: CallPhase::Idle,
            call_timer: 2.2,
            call_mood: 0.0,
            move_timer: 0.8 + r_hash(i as i32, 67, 8, seed) * 2.2,
            facing: if ang.cos() >= 0.0 { 1 } else { -1 },
            size,
        }
    }

    pub fn rebuild_roach_props(&mut self) {
        self.drag_state = None;
        self.roaches.clear();
        self.bubbles.clear();
        self.jet_strikes.clear();
        self.bombs.clear();
        self.explosions.clear();
        self.bubble_cooldown = 0.0;
        let count = self.params.crowd;
        for i in 0..count {
            self.roaches.push(self.spawn_roach_member(i));
        }

        // Keep couple (kind 8 & 9) walking right next to each other at spawn
        let him_pos = self
            .roaches
            .iter()
            .find(|r| r.kind == 8)
            .map(|r| (r.x, r.y, r.vx, r.vy));
        if let Some((hx, hy, hvx, hvy)) = him_pos {
            if let Some(her) = self.roaches.iter_mut().find(|r| r.kind == 9) {
                her.x = (hx + 28.0).min(self.w - 40.0);
                her.y = hy + 4.0;
                her.vx = hvx;
                her.vy = hvy;
            }
        }
    }

    pub fn sync_street_props(&mut self) {
        self.street_props.clear();
        for r in &mut self.roaches {
            r.in_bed = None;
            r.target_outlet = None;
        }
        if self.fb.is_empty() || self.params.props <= 0.04 {
            return;
        }
        let seed = self.params.seed;
        let is_too_close = |props: &[StreetProp], px: f64, py: f64, min_d: f64| -> bool {
            props.iter().any(|p| (p.x - px).hypot(p.y - py) < min_d)
        };

        // 1. Hospital beds
        let target_beds = if self.params.props > 0.55 { 2 } else { 1 };
        let mut placed_beds = 0usize;
        for attempt in 0..220 {
            if placed_beds >= target_beds {
                break;
            }
            let px = self.w * (0.14 + 0.72 * r_hash(attempt, 401, 21, seed));
            let py = self.h * (0.18 + 0.64 * r_hash(attempt, 503, 22, seed));
            if self.sample_field(px, py) < 0.16 {
                continue;
            }
            if is_too_close(&self.street_props, px, py, 200.0) {
                continue;
            }
            self.street_props.push(StreetProp {
                prop_type: StreetPropType::Bed,
                x: px,
                y: py,
                col: 4,
                row: 4,
                size: 110.0,
                occupant: None,
                phase: r_hash(attempt, 607, 23, seed) * 6.28,
                shout_timer: 4.5 + placed_beds as f64 * 3.0,
            });
            placed_beds += 1;
        }

        // 2. Wall power outlets
        let target_outlets = 3usize;
        let mut placed_outlets = 0usize;
        for attempt in 0..320 {
            if placed_outlets >= target_outlets {
                break;
            }
            let px = self.w * (0.06 + 0.88 * r_hash(attempt, 701, 31, seed));
            let py = self.h * (0.09 + 0.80 * r_hash(attempt, 809, 32, seed));
            let s = self.sample_field(px, py);
            if !(0.025..=0.12).contains(&s) {
                continue;
            }
            if is_too_close(&self.street_props, px, py, 160.0) {
                continue;
            }
            self.street_props.push(StreetProp {
                prop_type: StreetPropType::Outlet,
                x: px,
                y: py,
                col: 6,
                row: 2,
                size: 48.0,
                occupant: None,
                phase: r_hash(attempt, 907, 33, seed) * 6.28,
                shout_timer: 0.0,
            });
            placed_outlets += 1;
        }
        let wall_anchors = [
            (54.0, self.h * 0.34),
            (self.w - 54.0, self.h * 0.42),
            (54.0, self.h * 0.68),
            (self.w - 54.0, self.h * 0.72),
        ];
        for (wx, wy) in wall_anchors {
            if placed_outlets >= target_outlets {
                break;
            }
            if self.sample_field(wx, wy) < 0.04 || is_too_close(&self.street_props, wx, wy, 120.0) {
                continue;
            }
            self.street_props.push(StreetProp {
                prop_type: StreetPropType::Outlet,
                x: wx,
                y: wy,
                col: 6,
                row: 2,
                size: 48.0,
                occupant: None,
                phase: placed_outlets as f64 * 1.7,
                shout_timer: 0.0,
            });
            placed_outlets += 1;
        }

        // 3. Streetlamps, juice boxes & Turkish tea glasses
        let target_props =
            (5.0 + 10.0 * self.params.props).round() as usize + placed_beds + placed_outlets;
        for attempt in 0..260 {
            if self.street_props.len() >= target_props {
                break;
            }
            let px = self.w * (0.05 + 0.90 * r_hash(attempt, 101, 11, seed));
            let py = self.h * (0.07 + 0.84 * r_hash(attempt, 211, 12, seed));
            if self.sample_field(px, py) < 0.14 {
                continue;
            }
            if is_too_close(&self.street_props, px, py, 135.0) {
                continue;
            }
            let r = r_hash(attempt, 307, 13, seed);
            let col = if r < 0.56 {
                0
            } else if r < 0.82 {
                1
            } else {
                2
            };
            self.street_props.push(StreetProp {
                prop_type: StreetPropType::Prop,
                x: px,
                y: py,
                col,
                row: 2,
                size: if col == 0 { 112.0 } else { 42.0 },
                occupant: None,
                phase: r * 6.28,
                shout_timer: 0.0,
            });
        }
    }

    pub fn sync_roach_props(&mut self) {
        self.sync_street_props();
        let target = self.params.crowd;
        if self.roaches.len() < target {
            for i in self.roaches.len()..target {
                self.roaches.push(self.spawn_roach_member(i));
            }
        } else if self.roaches.len() > target {
            self.roaches.truncate(target);
            if let Some(ds) = &self.drag_state {
                if ds.roach_idx >= self.roaches.len() {
                    self.drag_state = None;
                }
            }
        }
    }

    fn push_bubble(&mut self, x: f64, y: f64, col: usize, row: usize, dur: f64, force: bool) {
        if !force && (self.bubble_cooldown > 0.0 || self.bubbles.len() >= 4) {
            return;
        }
        if !force {
            self.bubble_cooldown = 2.4 + self.rng.next() * 2.2;
        }
        self.bubbles.push(RoachBubble {
            x,
            y,
            age: 0.0,
            dur,
            col,
            row,
        });
    }

    fn spawn_jet_strike(&mut self, caller_idx: usize) {
        let mut targets: Vec<JetTarget> = self
            .roaches
            .iter()
            .enumerate()
            .filter(|&(idx, r)| idx != caller_idx && r.kind != 16)
            .take(5)
            .map(|(_, r)| JetTarget {
                tx: r.x,
                ty: r.y,
                dropped: false,
            })
            .collect();
        if targets.is_empty() {
            targets.push(JetTarget {
                tx: self.w * 0.35,
                ty: self.h * 0.45,
                dropped: false,
            });
            targets.push(JetTarget {
                tx: self.w * 0.62,
                ty: self.h * 0.55,
                dropped: false,
            });
        }
        targets.sort_by(|a, b| a.tx.partial_cmp(&b.tx).unwrap_or(std::cmp::Ordering::Equal));
        let avg_y = targets.iter().map(|t| t.ty).sum::<f64>() / targets.len() as f64;
        let start_y = (avg_y - 125.0).max(84.0);
        let end_y = (avg_y - 75.0).max(84.0);
        let travel_x = self.w + 260.0;
        let spd_x = 430.0;
        let vx = spd_x;
        let vy = ((end_y - start_y) / travel_x) * spd_x;
        self.jet_strikes.push(JetStrike {
            x: -130.0,
            y: start_y,
            vx,
            vy,
            rot: vy.atan2(vx),
            targets,
        });
    }

    pub fn step_props(&mut self, dt: f64) {
        if self.fb.is_empty() {
            return;
        }
        let spd_mul = self.params.speed.max(0.15);
        if self.bubble_cooldown > 0.0 {
            self.bubble_cooldown -= dt;
        }

        // Occupied bed patients shout "LÜLÜLÜLÜ!"
        let mut bed_bubbles = Vec::new();
        for b in &mut self.street_props {
            if b.prop_type == StreetPropType::Bed && b.occupant.is_some() {
                b.shout_timer -= dt * spd_mul;
                if b.shout_timer <= 0.0 {
                    b.shout_timer = 3.2 + self.rng.next() * 3.2;
                    bed_bubbles.push((b.x - 14.0, b.y - 46.0));
                }
            }
        }
        for (bx, by) in bed_bubbles {
            self.push_bubble(bx, by, 1, 3, 2.3, false);
        }

        let him_pos = self
            .roaches
            .iter()
            .find(|r| r.kind == 8)
            .map(|r| (r.x, r.y));
        let her_idx = self.roaches.iter().position(|r| r.kind == 9);
        let outlet_indices: Vec<usize> = self
            .street_props
            .iter()
            .enumerate()
            .filter(|(_, p)| p.prop_type == StreetPropType::Outlet)
            .map(|(i, _)| i)
            .collect();
        let bed_indices: Vec<usize> = self
            .street_props
            .iter()
            .enumerate()
            .filter(|(_, p)| p.prop_type == StreetPropType::Bed)
            .map(|(i, _)| i)
            .collect();
        let crazy_positions: Vec<(f64, f64)> = self
            .roaches
            .iter()
            .filter(|r| r.kind == 12 || r.kind == 13)
            .map(|r| (r.x, r.y))
            .collect();

        let roach_len = self.roaches.len();
        for idx in 0..roach_len {
            if self.roaches[idx].dragged {
                let z = self.roaches[idx].z;
                self.roaches[idx].z = z + (34.0 - z) * (dt * 14.0).min(1.0);
                continue;
            }

            // Airborne launch physics from jet bombing
            if self.roaches[idx].z > 0.0 || self.roaches[idx].vz > 0.0 {
                let mut r = self.roaches[idx].clone();
                r.x += r.vx * dt;
                r.y += r.vy * dt;
                if r.x < 34.0 {
                    r.x = 34.0;
                    r.vx = r.vx.abs();
                }
                if r.x > self.w - 34.0 {
                    r.x = self.w - 34.0;
                    r.vx = -r.vx.abs();
                }
                if r.y < 46.0 {
                    r.y = 46.0;
                    r.vy = r.vy.abs();
                }
                if r.y > self.h - 64.0 {
                    r.y = self.h - 64.0;
                    r.vy = -r.vy.abs();
                }
                r.vz -= 720.0 * dt;
                r.z += r.vz * dt;
                r.air_rot += r.spin_vel * dt;
                if r.z <= 0.0 {
                    r.z = 0.0;
                    r.vz = 0.0;
                    r.air_rot = 0.0;
                    r.spin_vel = 0.0;
                    let cur_spd = r.vx.hypot(r.vy);
                    if cur_spd > 0.01 {
                        r.vx = (r.vx / cur_spd) * r.base_speed;
                        r.vy = (r.vy / cur_spd) * r.base_speed;
                    }
                }
                self.roaches[idx] = r;
                continue;
            }

            // Walkie-talkie suit boss (kind 16)
            if self.roaches[idx].kind == 16 {
                match self.roaches[idx].call_phase {
                    CallPhase::Idle => {
                        self.roaches[idx].call_timer -= dt;
                        if self.roaches[idx].call_timer <= 0.0 && self.jet_strikes.is_empty() {
                            if let Some(diva_idx) = self
                                .roaches
                                .iter()
                                .position(|x| x.kind == 5 && x.z <= 0.0)
                            {
                                self.roaches[diva_idx].stop_timer = 4.0;
                                let dx = self.roaches[diva_idx].x;
                                let dy =
                                    self.roaches[diva_idx].y - self.roaches[diva_idx].size * 0.56;
                                self.push_bubble(dx, dy, 3, 4, 6.4, true);
                                self.roaches[idx].call_phase = CallPhase::DivaProvoke;
                                self.roaches[idx].call_timer = 3.0;
                            } else {
                                self.roaches[idx].call_phase = CallPhase::Raising;
                                self.roaches[idx].call_mood = 0.0;
                                self.roaches[idx].stop_timer = 0.0;
                            }
                            continue;
                        }
                    }
                    CallPhase::DivaProvoke => {
                        self.roaches[idx].call_timer -= dt;
                        if self.roaches[idx].call_timer <= 0.0 {
                            self.roaches[idx].call_phase = CallPhase::Raising;
                            self.roaches[idx].call_mood = 0.0;
                            self.roaches[idx].stop_timer = 0.0;
                        }
                        continue;
                    }
                    CallPhase::Raising => {
                        self.roaches[idx].call_mood =
                            (self.roaches[idx].call_mood + dt / 1.35).min(1.0);
                        if self.roaches[idx].call_mood >= 1.0 {
                            self.roaches[idx].call_phase = CallPhase::Calling;
                            self.roaches[idx].call_timer = 4.6;
                            let rx = self.roaches[idx].x;
                            let ry = self.roaches[idx].y - self.roaches[idx].size * 0.58;
                            self.push_bubble(rx, ry, 5, 3, 4.8, true);
                        }
                        continue;
                    }
                    CallPhase::Calling => {
                        self.roaches[idx].call_timer -= dt;
                        if self.roaches[idx].call_timer <= 0.0 {
                            self.spawn_jet_strike(idx);
                            self.roaches[idx].call_phase = CallPhase::Lowering;
                        }
                        continue;
                    }
                    CallPhase::Lowering => {
                        self.roaches[idx].call_mood =
                            (self.roaches[idx].call_mood - dt / 0.85).max(0.0);
                        if self.roaches[idx].call_mood <= 0.0 {
                            self.roaches[idx].call_phase = CallPhase::Idle;
                            self.roaches[idx].call_timer = 45.0 + self.rng.next() * 20.0;
                        }
                        continue;
                    }
                }
            }

            if self.roaches[idx].charge_cooldown > 0.0 {
                self.roaches[idx].charge_cooldown -= dt;
            }
            if self.roaches[idx].bed_cooldown > 0.0 {
                self.roaches[idx].bed_cooldown -= dt;
            }

            // Crazy roaches resting inside a bed
            if (self.roaches[idx].kind == 12 || self.roaches[idx].kind == 13)
                && self.roaches[idx].in_bed.is_some()
            {
                let b_idx = self.roaches[idx].in_bed.unwrap();
                let bx = self.street_props[b_idx].x;
                let by = self.street_props[b_idx].y;
                self.roaches[idx].x = bx;
                self.roaches[idx].y = by;
                self.roaches[idx].bed_timer -= dt;
                if self.roaches[idx].bed_timer <= 0.0 {
                    self.roaches[idx].x = bx;
                    self.roaches[idx].y = (by + 18.0).min(self.h - 64.0);
                    self.street_props[b_idx].occupant = None;
                    self.roaches[idx].in_bed = None;
                    self.roaches[idx].bed_cooldown = 9.0 + self.rng.next() * 8.0;
                    self.roaches[idx].move_timer = 2.0 + self.rng.next() * 2.0;
                }
                continue;
            }

            // Wall-outlet self-charging behavior for kind 15
            if self.roaches[idx].kind == 15 && self.roaches[idx].charge_timer > 0.0 {
                self.roaches[idx].charge_timer -= dt;
                if self.rng.next() < dt * 0.35 {
                    let rx = self.roaches[idx].x;
                    let ry = self.roaches[idx].y - self.roaches[idx].size * 0.54;
                    self.push_bubble(rx, ry, 3, 3, 1.9, false);
                }
                if self.roaches[idx].charge_timer <= 0.0 {
                    self.roaches[idx].charge_cooldown = 4.5 + self.rng.next() * 3.5;
                    self.roaches[idx].target_outlet = None;
                    self.roaches[idx].move_timer = 2.0;
                }
                continue;
            }

            if self.roaches[idx].stop_timer > 0.0 {
                self.roaches[idx].stop_timer -= dt;
                continue;
            }

            // Charging roach steers toward nearest wall outlet
            if self.roaches[idx].kind == 15
                && self.roaches[idx].charge_cooldown <= 0.0
                && !outlet_indices.is_empty()
            {
                if self.roaches[idx].target_outlet.is_none() {
                    let rx = self.roaches[idx].x;
                    let ry = self.roaches[idx].y;
                    let mut best = outlet_indices[0];
                    let mut best_d = 1e9f64;
                    for &oi in &outlet_indices {
                        let d = (self.street_props[oi].x - rx).hypot(self.street_props[oi].y - ry);
                        if d < best_d {
                            best_d = d;
                            best = oi;
                        }
                    }
                    self.roaches[idx].target_outlet = Some(best);
                }
                if let Some(oi) = self.roaches[idx].target_outlet {
                    let tx = self.street_props[oi].x + 22.0;
                    let ty = self.street_props[oi].y + 8.0;
                    let dx = tx - self.roaches[idx].x;
                    let dy = ty - self.roaches[idx].y;
                    let dist = dx.hypot(dy);
                    if dist < 28.0 {
                        self.roaches[idx].charge_timer = 3.6 + self.rng.next() * 2.0;
                        let rx = self.roaches[idx].x;
                        let ry = self.roaches[idx].y - self.roaches[idx].size * 0.54;
                        self.push_bubble(rx, ry, 3, 3, 2.1, false);
                        continue;
                    }
                    self.roaches[idx].vx += (dx / dist) * 150.0 * dt;
                    self.roaches[idx].vy += (dy / dist) * 150.0 * dt;
                }
            }

            // Crazy roaches hop into a vacant hospital bed
            if (self.roaches[idx].kind == 12 || self.roaches[idx].kind == 13)
                && self.roaches[idx].in_bed.is_none()
                && self.roaches[idx].bed_cooldown <= 0.0
                && !bed_indices.is_empty()
            {
                let rx = self.roaches[idx].x;
                let ry = self.roaches[idx].y;
                let vacant = bed_indices.iter().copied().find(|&bi| {
                    self.street_props[bi].occupant.is_none()
                        && (self.street_props[bi].x - rx).hypot(self.street_props[bi].y - ry) < 34.0
                });
                if let Some(bi) = vacant {
                    self.street_props[bi].occupant = Some(idx);
                    self.street_props[bi].shout_timer = 0.8 + self.rng.next() * 2.5;
                    self.roaches[idx].in_bed = Some(bi);
                    self.roaches[idx].bed_timer = 4.5 + self.rng.next() * 4.5;
                    self.roaches[idx].stop_timer = 0.0;
                    continue;
                }
            }

            // Doctor (kind 11) walks rounds
            if self.roaches[idx].kind == 11 && self.roaches[idx].move_timer < 0.8 {
                let target_pt = if !bed_indices.is_empty()
                    && (crazy_positions.is_empty() || self.rng.next() < 0.55)
                {
                    let bi = bed_indices[self.roaches[idx].id % bed_indices.len()];
                    Some((self.street_props[bi].x, self.street_props[bi].y))
                } else if !crazy_positions.is_empty() {
                    Some(crazy_positions[self.roaches[idx].id % crazy_positions.len()])
                } else {
                    None
                };
                if let Some((tx, ty)) = target_pt {
                    let dx = tx - self.roaches[idx].x;
                    let dy = ty - self.roaches[idx].y;
                    let dist = dx.hypot(dy);
                    if dist > 38.0 && dist < 420.0 {
                        self.roaches[idx].vx += (dx / dist) * 65.0 * dt;
                        self.roaches[idx].vy += (dy / dist) * 65.0 * dt;
                    }
                }
            }

            self.roaches[idx].move_timer -= dt;
            if self.roaches[idx].move_timer <= 0.0 {
                let kind = self.roaches[idx].kind;
                if self.rng.next() < 0.44 {
                    self.roaches[idx].stop_timer = if kind == 12 || kind == 13 {
                        1.1 + self.rng.next() * 1.4
                    } else {
                        1.5 + self.rng.next() * 2.1
                    };
                    self.roaches[idx].move_timer = 3.0 + self.rng.next() * 4.0;
                    let shout_prob = if kind == 12 || kind == 13 { 0.72 } else { 0.45 };
                    if kind != 16 && self.rng.next() < shout_prob {
                        let rx = self.roaches[idx].x;
                        let ry = self.roaches[idx].y;
                        let rsize = self.roaches[idx].size;
                        if kind == 11 {
                            self.push_bubble(rx, ry - rsize * 0.52, 2, 3, 2.2, false);
                        } else if kind == 12 || kind == 13 {
                            let text_col = if self.rng.next() < 0.88 { 1 } else { 2 };
                            self.push_bubble(rx, ry - rsize * 0.54, text_col, 3, 2.2, false);
                        } else {
                            let sym = if kind == 5 {
                                4
                            } else if kind == 9 || kind == 8 {
                                3
                            } else {
                                5
                            };
                            self.push_bubble(rx, ry - rsize * 0.48, sym, 2, 1.7, false);
                        }
                    }
                    continue;
                } else {
                    let turn = (self.rng.next() - 0.5)
                        * if kind == 12 || kind == 13 { 2.4 } else { 1.4 };
                    let c = turn.cos();
                    let s = turn.sin();
                    let vx = self.roaches[idx].vx;
                    let vy = self.roaches[idx].vy;
                    self.roaches[idx].vx = vx * c - vy * s;
                    self.roaches[idx].vy = vx * s + vy * c;
                    self.roaches[idx].move_timer = 2.0 + self.rng.next() * 3.0;
                }
            }

            // Gentle couple cohesion
            if Some(idx) == her_idx {
                if let Some((hx, hy)) = him_pos {
                    let tx = hx + 26.0;
                    let ty = hy + 3.0;
                    self.roaches[idx].vx += (tx - self.roaches[idx].x) * 1.6 * dt;
                    self.roaches[idx].vy += (ty - self.roaches[idx].y) * 1.6 * dt;
                }
            }

            let mut nx = self.roaches[idx].x + self.roaches[idx].vx * spd_mul * dt;
            let mut ny = self.roaches[idx].y + self.roaches[idx].vy * spd_mul * dt;

            if nx < 34.0 {
                nx = 34.0;
                self.roaches[idx].vx = self.roaches[idx].vx.abs();
            }
            if nx > self.w - 34.0 {
                nx = self.w - 34.0;
                self.roaches[idx].vx = -self.roaches[idx].vx.abs();
            }
            if ny < 46.0 {
                ny = 46.0;
                self.roaches[idx].vy = self.roaches[idx].vy.abs();
            }
            if ny > self.h - 64.0 {
                ny = self.h - 64.0;
                self.roaches[idx].vy = -self.roaches[idx].vy.abs();
            }

            // Steer away from icon curb islands (stay on cobblestone lanes s > 0.04)
            let sd = self.sample_field(nx, ny);
            if sd < 0.04 {
                let (gx, gy) = self.walkable_gradient(nx, ny);
                let dot = self.roaches[idx].vx * gx + self.roaches[idx].vy * gy;
                if dot < 0.0 {
                    self.roaches[idx].vx -= 1.85 * dot * gx;
                    self.roaches[idx].vy -= 1.85 * dot * gy;
                } else {
                    self.roaches[idx].vx += gx * 32.0 * dt;
                    self.roaches[idx].vy += gy * 32.0 * dt;
                }
                nx = self.roaches[idx].x + gx * 18.0 * dt;
                ny = self.roaches[idx].y + gy * 18.0 * dt;
            }

            let cur_spd = self.roaches[idx].vx.hypot(self.roaches[idx].vy);
            if cur_spd > 0.01 {
                let bs = self.roaches[idx].base_speed;
                self.roaches[idx].vx = (self.roaches[idx].vx / cur_spd) * bs;
                self.roaches[idx].vy = (self.roaches[idx].vy / cur_spd) * bs;
            }
            self.roaches[idx].x = nx;
            self.roaches[idx].y = ny;
        }

        // Fighter jets flying across and dropping non-lethal bombs
        let mut ji = self.jet_strikes.len();
        while ji > 0 {
            ji -= 1;
            self.jet_strikes[ji].x += self.jet_strikes[ji].vx * dt;
            self.jet_strikes[ji].y += self.jet_strikes[ji].vy * dt;
            let jx = self.jet_strikes[ji].x;
            let jy = self.jet_strikes[ji].y;
            for t in &mut self.jet_strikes[ji].targets {
                if !t.dropped && jx >= t.tx - 36.0 {
                    t.dropped = true;
                    self.bombs.push(RoachBomb {
                        sx: jx,
                        sy: jy + 8.0,
                        tx: t.tx,
                        ty: t.ty,
                        age: 0.0,
                        dur: 0.50,
                    });
                }
            }
            if jx > self.w + 160.0 {
                self.jet_strikes.remove(ji);
            }
        }

        // Falling bombs -> explosion fireball -> launch nearby cockroaches sky-high!
        let mut bi = self.bombs.len();
        while bi > 0 {
            bi -= 1;
            self.bombs[bi].age += dt;
            if self.bombs[bi].age >= self.bombs[bi].dur {
                let btx = self.bombs[bi].tx;
                let bty = self.bombs[bi].ty;
                self.explosions.push(RoachExplosion {
                    x: btx,
                    y: bty,
                    age: 0.0,
                    dur: 0.96,
                    size: 176.0,
                });
                for r_idx in 0..self.roaches.len() {
                    if self.roaches[r_idx].kind == 16 || self.roaches[r_idx].dragged {
                        continue;
                    }
                    let dx = self.roaches[r_idx].x - btx;
                    let dy = self.roaches[r_idx].y - bty;
                    let dist = dx.hypot(dy);
                    if dist < 210.0 {
                        if let Some(bed_i) = self.roaches[r_idx].in_bed.take() {
                            self.street_props[bed_i].occupant = None;
                            self.roaches[r_idx].x = self.street_props[bed_i].x;
                            self.roaches[r_idx].y =
                                (self.street_props[bed_i].y + 18.0).min(self.h - 64.0);
                            self.roaches[r_idx].bed_cooldown = 9.0 + self.rng.next() * 8.0;
                        }
                        let fall = 1.0 - dist / 210.0;
                        let ang = if dist > 4.0 {
                            dy.atan2(dx)
                        } else {
                            self.rng.next() * PI * 2.0
                        };
                        let kick = 220.0 + 260.0 * fall;
                        self.roaches[r_idx].vx = ang.cos() * kick;
                        self.roaches[r_idx].vy = ang.sin() * kick;
                        self.roaches[r_idx].vz = self.roaches[r_idx].vz.max(320.0 + 280.0 * fall);
                        self.roaches[r_idx].spin_vel =
                            (if self.rng.next() < 0.5 { -1.0 } else { 1.0 })
                                * (10.0 + 9.0 * self.rng.next());
                        self.roaches[r_idx].charge_timer = 0.0;
                        self.roaches[r_idx].stop_timer = 0.0;
                    }
                }
                self.bombs.remove(bi);
            }
        }

        let mut ei = self.explosions.len();
        while ei > 0 {
            ei -= 1;
            self.explosions[ei].age += dt;
            if self.explosions[ei].age >= self.explosions[ei].dur {
                self.explosions.remove(ei);
            }
        }

        // Ambient speech & reaction bubbles
        self.bubble_timer += dt * spd_mul;
        if self.bubble_timer > 1.35 && !self.roaches.is_empty() {
            self.bubble_timer = 0.0;
            let crazy_indices: Vec<usize> = self
                .roaches
                .iter()
                .enumerate()
                .filter(|(_, r)| r.kind == 12 || r.kind == 13)
                .map(|(i, _)| i)
                .collect();
            let star_idx = if !crazy_indices.is_empty() && self.rng.next() < 0.42 {
                crazy_indices[(self.rng.next() * crazy_indices.len() as f64) as usize
                    % crazy_indices.len()]
            } else {
                (self.rng.next() * self.roaches.len() as f64) as usize % self.roaches.len()
            };
            let star_kind = self.roaches[star_idx].kind;
            let sx = self.roaches[star_idx].x;
            let sy = self.roaches[star_idx].y;
            let ssize = self.roaches[star_idx].size;
            let scharge = self.roaches[star_idx].charge_timer;
            if star_kind == 11 {
                self.push_bubble(sx, sy - ssize * 0.52, 2, 3, 2.3, false);
            } else if star_kind == 12 || star_kind == 13 {
                let col = if self.rng.next() < 0.88 { 1 } else { 2 };
                self.push_bubble(sx, sy - ssize * 0.52, col, 3, 2.3, false);
            } else if star_kind == 15 && scharge > 0.0 {
                self.push_bubble(sx, sy - ssize * 0.52, 3, 3, 2.1, false);
            } else if star_kind != 16 {
                let col = if star_kind == 5 {
                    4
                } else if star_kind == 8 || star_kind == 9 {
                    3
                } else {
                    5
                };
                let jitter = (self.rng.next() - 0.5) * 14.0;
                self.push_bubble(sx + jitter, sy - ssize * 0.45, col, 2, 1.8, false);
            }
        }

        let mut bub_i = self.bubbles.len();
        while bub_i > 0 {
            bub_i -= 1;
            self.bubbles[bub_i].age += dt;
            self.bubbles[bub_i].y -= 18.0 * dt;
            if self.bubbles[bub_i].age >= self.bubbles[bub_i].dur {
                self.bubbles.remove(bub_i);
            }
        }
    }

    pub fn step(&mut self, dt_f32: f32) {
        let dt = (dt_f32 as f64).min(0.1);
        self.sim_time += dt;
        self.morph_elapsed += dt;
        let e = (self.morph_elapsed / MORPH_DUR).clamp(0.0, 1.0);
        self.cur_mix = if e < 0.5 {
            2.0 * e * e
        } else {
            1.0 - (-2.0 * e + 2.0).powi(2) * 0.5
        };
        self.step_props(dt);
    }

    pub fn pick_roach_at(&self, px: f64, py: f64) -> Option<usize> {
        let mut best: Option<usize> = None;
        let mut best_score = -1e9f64;
        for (idx, r) in self.roaches.iter().enumerate() {
            let (rx, ry, radius) = if let Some(bi) = r.in_bed {
                (
                    self.street_props[bi].x,
                    self.street_props[bi].y - 4.0,
                    r.size * 0.56,
                )
            } else {
                (r.x, r.y - r.z, r.size * 0.48)
            };
            let dist = (rx - px).hypot(ry - py);
            if dist <= radius {
                let sort_score = r.y + if r.z > 0.0 { 120.0 } else { 0.0 } - dist * 0.05;
                if sort_score > best_score {
                    best_score = sort_score;
                    best = Some(idx);
                }
            }
        }
        best
    }

    pub fn handle_mouse(&mut self, kind: i32, px: f64, py: f64) -> bool {
        match kind {
            1 => {
                // Down
                let Some(idx) = self.pick_roach_at(px, py) else {
                    return false;
                };
                if let Some(bi) = self.roaches[idx].in_bed.take() {
                    self.roaches[idx].x = self.street_props[bi].x;
                    self.roaches[idx].y = self.street_props[bi].y;
                    self.street_props[bi].occupant = None;
                    self.roaches[idx].bed_cooldown = 9.0 + self.rng.next() * 8.0;
                }
                self.roaches[idx].dragged = true;
                self.roaches[idx].charge_timer = 0.0;
                self.roaches[idx].target_outlet = None;
                self.roaches[idx].stop_timer = 0.0;
                self.roaches[idx].vz = 0.0;
                self.roaches[idx].spin_vel = 0.0;
                self.roaches[idx].air_rot = 0.0;
                self.roaches[idx].z = self.roaches[idx].z.max(28.0);
                let rx = self.roaches[idx].x;
                let ry = self.roaches[idx].y;
                let rz = self.roaches[idx].z;
                let rsize = self.roaches[idx].size;
                let rkind = self.roaches[idx].kind;
                self.drag_state = Some(RoachDragState {
                    roach_idx: idx,
                    off_x: rx - px,
                    off_y: (ry - rz) - py,
                    last_x: px,
                    last_y: py,
                    last_t: self.sim_time,
                    vx: 0.0,
                    vy: 0.0,
                });
                if rkind == 12 || rkind == 13 {
                    self.push_bubble(rx, ry - rsize * 0.56 - rz, 1, 3, 2.0, true);
                } else if rkind == 11 {
                    self.push_bubble(rx, ry - rsize * 0.56 - rz, 2, 3, 2.0, true);
                } else {
                    self.push_bubble(rx, ry - rsize * 0.52 - rz, 1, 2, 1.4, true);
                }
                true
            }
            0 => {
                // Move
                let Some(ds) = &mut self.drag_state else {
                    return false;
                };
                let idx = ds.roach_idx;
                let dt = (self.sim_time - ds.last_t).max(0.008);
                let inst_vx = (px - ds.last_x) / dt;
                let inst_vy = (py - ds.last_y) / dt;
                let blend = (dt * 18.0).min(1.0);
                ds.vx += (inst_vx - ds.vx) * blend;
                ds.vy += (inst_vy - ds.vy) * blend;
                ds.last_x = px;
                ds.last_y = py;
                ds.last_t = self.sim_time;
                let off_x = ds.off_x;
                let off_y = ds.off_y;
                let dvx = ds.vx;
                if idx < self.roaches.len() {
                    let rz = self.roaches[idx].z;
                    self.roaches[idx].x = (px + off_x).clamp(28.0, (self.w - 28.0).max(28.0));
                    self.roaches[idx].y =
                        (py + off_y + rz).clamp(42.0, (self.h - 58.0).max(42.0));
                    self.roaches[idx].air_rot = (dvx * 0.0012).clamp(-0.55, 0.55);
                }
                true
            }
            2 => {
                // Up
                let Some(ds) = self.drag_state.take() else {
                    return false;
                };
                let idx = ds.roach_idx;
                if idx < self.roaches.len() {
                    self.roaches[idx].dragged = false;
                    let throw_vx = (ds.vx * 0.65).clamp(-620.0, 620.0);
                    let throw_vy = (ds.vy * 0.65).clamp(-620.0, 620.0);
                    let fling_spd = throw_vx.hypot(throw_vy);
                    if fling_spd > 35.0 {
                        self.roaches[idx].vx = throw_vx;
                        self.roaches[idx].vy = throw_vy;
                        self.roaches[idx].vz = (fling_spd * 0.42).clamp(65.0, 280.0);
                        self.roaches[idx].spin_vel = (throw_vx * 0.012).clamp(-8.0, 8.0);
                    } else {
                        let cur_spd = self.roaches[idx].vx.hypot(self.roaches[idx].vy).max(1.0);
                        let bs = self.roaches[idx].base_speed;
                        self.roaches[idx].vx = (self.roaches[idx].vx / cur_spd) * bs;
                        self.roaches[idx].vy = (self.roaches[idx].vy / cur_spd) * bs;
                        self.roaches[idx].vz = 55.0;
                        self.roaches[idx].spin_vel =
                            (if self.roaches[idx].phase % 2.0 > 1.0 { 1.0 } else { -1.0 }) * 1.4;
                    }
                    self.roaches[idx].move_timer = 1.4 + self.rng.next() * 1.6;
                }
                true
            }
            _ => false,
        }
    }

    file_private_build_instances!();
}

macro_rules! file_private_build_instances {
    () => {
        fn build_draw_instances(&self, out: &mut Vec<InstSprite>) {
            out.clear();
            let time_sec = self.sim_time;
            let spd_mul = self.params.speed.max(0.15);

            struct ActorItem {
                sort_y: f64,
                x: f64,
                y: f64,
                ground_y: f64,
                size: f64,
                rot: f64,
                col: usize,
                row: usize,
                alpha: f64,
                shadow_size: f64,
                shadow_off_y: f64,
                dark_aura: f64,
                angry_overlay: f64,
            }

            let mut actors = Vec::with_capacity(self.street_props.len() + self.roaches.len());
            let mut zaps = Vec::new();

            if self.params.props > 0.04 {
                for p in &self.street_props {
                    let mut sway = 0.0;
                    let mut bounce_y = 0.0;
                    let mut p_col = p.col;
                    let mut p_row = p.row;
                    if p.prop_type == StreetPropType::Bed {
                        let occupied = p.occupant.is_some();
                        p_col = if occupied { 6 } else { 4 };
                        p_row = if occupied { 1 } else { 4 };
                        if occupied {
                            sway = (time_sec * 8.5 + p.phase).sin() * 0.035;
                            bounce_y = (time_sec * 6.0 + p.phase).sin().abs() * 2.2;
                        }
                    } else if p.prop_type == StreetPropType::Prop && p.col != 0 {
                        sway = (time_sec * 1.4 + p.phase).sin() * 0.04;
                    }
                    actors.push(ActorItem {
                        sort_y: p.y + p.size * 0.28,
                        x: p.x,
                        y: p.y - bounce_y,
                        ground_y: p.y,
                        size: p.size,
                        rot: sway,
                        col: p_col,
                        row: p_row,
                        alpha: 0.98,
                        shadow_size: if p.prop_type == StreetPropType::Bed {
                            92.0
                        } else if p.col == 0 && p.prop_type == StreetPropType::Prop {
                            54.0
                        } else {
                            30.0
                        },
                        shadow_off_y: if p.prop_type == StreetPropType::Bed {
                            28.0 + bounce_y
                        } else if p.col == 0 && p.prop_type == StreetPropType::Prop {
                            44.0
                        } else {
                            18.0
                        },
                        dark_aura: 0.0,
                        angry_overlay: 0.0,
                    });
                }
            }

            for r in &self.roaches {
                if r.in_bed.is_some() {
                    continue;
                }
                let held = r.dragged;
                let charging = !held && r.kind == 15 && r.charge_timer > 0.0 && r.z <= 0.0;
                let crazy = r.kind == 12 || r.kind == 13;
                let calling = !held && r.kind == 16 && r.call_mood > 0.0;
                let moving = !held && r.stop_timer <= 0.0 && !charging && !calling && r.z <= 0.0;
                let jitter_x = if charging {
                    (time_sec * 48.0 + r.phase).sin() * 2.2
                } else {
                    0.0
                };
                let bounce = if r.z > 0.0 || held {
                    0.0
                } else if charging {
                    (time_sec * 28.0 + r.phase).sin().abs() * 3.0
                } else if moving {
                    (time_sec * ((if crazy { 8.0 } else { 5.5 }) * spd_mul) + r.phase)
                        .sin()
                        .abs()
                        * if crazy { 6.5 } else { 4.5 }
                } else {
                    (time_sec * 2.8 + r.phase).sin() * if crazy { 3.0 } else { 1.5 }
                };
                let tilt = if held {
                    r.air_rot + (time_sec * 30.0 + r.phase).sin() * 0.22
                } else if r.z > 0.0 {
                    r.air_rot
                } else if charging {
                    (time_sec * 42.0 + r.phase).sin() * 0.14
                } else if moving {
                    (time_sec * ((if crazy { 8.0 } else { 5.5 }) * spd_mul) + r.phase).sin()
                        * if crazy { 0.17 } else { 0.09 }
                } else if r.kind == 5 || crazy {
                    (time_sec * 4.5 + r.phase).sin() * 0.16
                } else {
                    (time_sec * 1.8 + r.phase).sin() * 0.04
                };

                let mood = if calling {
                    r.call_mood * r.call_mood * (3.0 - 2.0 * r.call_mood)
                } else {
                    0.0
                };
                let height_scale = 1.0 + r.z / 420.0;
                let shadow_scale = (1.0 - r.z / 280.0).max(0.45);
                actors.push(ActorItem {
                    sort_y: r.y
                        + r.size * 0.34
                        + if held {
                            140.0
                        } else if r.z > 0.0 {
                            120.0
                        } else {
                            0.0
                        },
                    x: r.x + jitter_x,
                    y: r.y - bounce - r.z,
                    ground_y: r.y,
                    size: r.size * height_scale,
                    rot: tilt,
                    col: r.col,
                    row: r.row,
                    alpha: 1.0,
                    shadow_size: r.size * 0.72 * shadow_scale,
                    shadow_off_y: r.size * 0.35,
                    dark_aura: mood,
                    angry_overlay: mood,
                });

                if charging {
                    if let Some(oi) = r.target_outlet {
                        if oi < self.street_props.len() {
                            let ox = self.street_props[oi].x;
                            let oy = self.street_props[oi].y;
                            zaps.push(InstSprite {
                                x: ((r.x + ox) * 0.5) as f32,
                                y: ((r.y + oy) * 0.5 - 6.0) as f32,
                                size: (46.0 + (time_sec * 36.0).sin() * 8.0) as f32,
                                rot: (time_sec * 7.0 + r.phase) as f32,
                                col: 7.0,
                                row: 2.0,
                                alpha: (0.72 + 0.26 * (time_sec * 40.0).sin()) as f32,
                            });
                        }
                    }
                }
            }

            actors.sort_by(|a, b| {
                a.sort_y
                    .partial_cmp(&b.sort_y)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Soft shadows first
            for a in &actors {
                out.push(InstSprite {
                    x: a.x as f32,
                    y: (a.ground_y + a.shadow_off_y) as f32,
                    size: a.shadow_size as f32,
                    rot: 0.0,
                    col: 0.0,
                    row: 3.0,
                    alpha: (0.85 * a.alpha) as f32,
                });
            }

            // Dramatic dark noir filter spotlight around calling roach, then actor sprites
            for a in &actors {
                if a.dark_aura > 0.01 {
                    out.push(InstSprite {
                        x: a.x as f32,
                        y: a.y as f32,
                        size: (a.size * 1.95) as f32,
                        rot: 0.0,
                        col: 7.0,
                        row: 3.0,
                        alpha: (0.78 * a.dark_aura) as f32,
                    });
                }
                out.push(InstSprite {
                    x: a.x as f32,
                    y: a.y as f32,
                    size: a.size as f32,
                    rot: a.rot as f32,
                    col: a.col as f32,
                    row: a.row as f32,
                    alpha: a.alpha as f32,
                });
                if a.angry_overlay > 0.01 {
                    out.push(InstSprite {
                        x: a.x as f32,
                        y: a.y as f32,
                        size: a.size as f32,
                        rot: a.rot as f32,
                        col: 6.0,
                        row: 3.0,
                        alpha: a.angry_overlay as f32,
                    });
                }
            }

            // Electric discharge flashes
            out.extend_from_slice(&zaps);

            // Falling bombs + ground target shadows
            for b in &self.bombs {
                let t = (b.age / b.dur).clamp(0.0, 1.0);
                let bx = b.sx + (b.tx - b.sx) * t;
                let by = b.sy + (b.ty - b.sy) * (t * t);
                out.push(InstSprite {
                    x: b.tx as f32,
                    y: b.ty as f32,
                    size: (22.0 + 34.0 * t) as f32,
                    rot: 0.0,
                    col: 0.0,
                    row: 3.0,
                    alpha: (0.55 * t) as f32,
                });
                out.push(InstSprite {
                    x: bx as f32,
                    y: by as f32,
                    size: (54.0 - 14.0 * t) as f32,
                    rot: 0.3,
                    col: 1.0,
                    row: 4.0,
                    alpha: 0.98,
                });
            }

            // Comic explosions
            for e in &self.explosions {
                let t = (e.age / e.dur).clamp(0.0, 1.0);
                let grow = (t * 4.5).min(1.0);
                let alpha = if t < 0.55 {
                    0.98
                } else {
                    ((1.0 - t) / 0.45).max(0.0)
                };
                out.push(InstSprite {
                    x: e.x as f32,
                    y: (e.y - 12.0) as f32,
                    size: (e.size * (0.45 + 0.75 * grow)) as f32,
                    rot: (t * 1.6) as f32,
                    col: 2.0,
                    row: 4.0,
                    alpha: alpha as f32,
                });
            }

            // Fighter jet flying overhead with ground shadow
            for j in &self.jet_strikes {
                out.push(InstSprite {
                    x: (j.x + 28.0) as f32,
                    y: (j.y + 96.0) as f32,
                    size: 96.0,
                    rot: j.rot as f32,
                    col: 0.0,
                    row: 3.0,
                    alpha: 0.44,
                });
                out.push(InstSprite {
                    x: j.x as f32,
                    y: j.y as f32,
                    size: 156.0,
                    rot: j.rot as f32,
                    col: 0.0,
                    row: 4.0,
                    alpha: 0.99,
                });
            }

            // Reaction & Turkish speech bubbles on top
            for b in &self.bubbles {
                let t = (b.age / b.dur).clamp(0.0, 1.0);
                let pop = (t * 4.0).min(1.0);
                let is_text = b.row == 3 || (b.row == 4 && b.col == 3);
                let base_size = if is_text {
                    if (b.row == 3 && b.col == 5) || b.row == 4 {
                        112.0
                    } else {
                        96.0
                    }
                } else {
                    26.0
                };
                let alpha = if t < 0.72 {
                    0.98
                } else {
                    ((1.0 - t) / 0.28).max(0.0)
                };
                out.push(InstSprite {
                    x: (b.x + (t * 6.0).sin() * if is_text { 2.5 } else { 5.0 }) as f32,
                    y: b.y as f32,
                    size: (base_size * (0.74 + 0.30 * pop)) as f32,
                    rot: ((t * 5.0).sin() * if is_text { 0.04 } else { 0.12 }) as f32,
                    col: b.col as f32,
                    row: b.row as f32,
                    alpha: alpha as f32,
                });
            }
        }
    };
}
use file_private_build_instances;

pub struct RoachRenderer {
    bg_prog: GLuint,
    u_noise: GLint,
    u_fa: GLint,
    u_fb: GLint,
    u_res: GLint,
    u_grid: GLint,
    u_scale: GLint,
    u_seed: GLint,
    u_step: GLint,
    u_mix: GLint,
    u_time: GLint,
    u_glow: GLint,
    u_vibe: GLint,
    u_debug: GLint,
    bg_vao: GLuint,
    noise_tex: GLuint,
    ftex: [GLuint; 2],
    tex_gw: usize,
    tex_gh: usize,

    inst_prog: GLuint,
    u_inst_atlas: GLint,
    u_inst_res: GLint,
    u_inst_grid: GLint,
    inst_vao: GLuint,
    inst_vbo_quad: GLuint,
    inst_vbo_data: GLuint,
    inst_cap: usize,
    atlas_tex: GLuint,
    instances: Vec<InstSprite>,
}

impl RoachRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let bg_prog = gl.compile_program(ROACH_BG_VS, ROACH_BG_FS)?;
        let u_noise = gl.uniform_loc(bg_prog, "uNoise");
        let u_fa = gl.uniform_loc(bg_prog, "uFA");
        let u_fb = gl.uniform_loc(bg_prog, "uFB");
        let u_res = gl.uniform_loc(bg_prog, "uRes");
        let u_grid = gl.uniform_loc(bg_prog, "uGrid");
        let u_scale = gl.uniform_loc(bg_prog, "uScale");
        let u_seed = gl.uniform_loc(bg_prog, "uSeed");
        let u_step = gl.uniform_loc(bg_prog, "uStep");
        let u_mix = gl.uniform_loc(bg_prog, "uMix");
        let u_time = gl.uniform_loc(bg_prog, "uTime");
        let u_glow = gl.uniform_loc(bg_prog, "uGlow");
        let u_vibe = gl.uniform_loc(bg_prog, "uVibe");
        let u_debug = gl.uniform_loc(bg_prog, "uDebug");

        let mut bg_vao = 0;
        (gl.GenVertexArrays)(1, &mut bg_vao);

        let mut noise_tex = 0;
        (gl.GenTextures)(1, &mut noise_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, noise_tex);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT as GLint);

        let mut ftex = [0u32; 2];
        (gl.GenTextures)(2, ftex.as_mut_ptr());
        for &t in &ftex {
            (gl.BindTexture)(GL_TEXTURE_2D, t);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST as GLint);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST as GLint);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as GLint);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as GLint);
        }

        let inst_prog = gl.compile_program(ROACH_INST_VS, ROACH_INST_FS)?;
        let u_inst_atlas = gl.uniform_loc(inst_prog, "uAtlas");
        let u_inst_res = gl.uniform_loc(inst_prog, "uRes");
        let u_inst_grid = gl.uniform_loc(inst_prog, "uGridAtlas");

        let mut inst_vao = 0;
        (gl.GenVertexArrays)(1, &mut inst_vao);
        (gl.BindVertexArray)(inst_vao);

        let quad: [f32; 12] = [
            -0.5, -0.5, 0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5,
        ];
        let mut inst_vbo_quad = 0;
        (gl.GenBuffers)(1, &mut inst_vbo_quad);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_vbo_quad);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            std::mem::size_of_val(&quad) as isize,
            quad.as_ptr() as *const c_void,
            GL_STATIC_DRAW,
        );
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, 8, std::ptr::null());

        let mut inst_vbo_data = 0;
        let inst_cap = 256usize;
        (gl.GenBuffers)(1, &mut inst_vbo_data);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_vbo_data);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (inst_cap * std::mem::size_of::<InstSprite>()) as isize,
            std::ptr::null(),
            GL_DYNAMIC_DRAW,
        );
        let stride = std::mem::size_of::<InstSprite>() as GLsizei;
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, stride, std::ptr::null());
        (gl.VertexAttribDivisor)(1, 1);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(
            2,
            3,
            GL_FLOAT,
            GL_FALSE,
            stride,
            (4 * std::mem::size_of::<f32>()) as *const c_void,
        );
        (gl.VertexAttribDivisor)(2, 1);
        (gl.BindVertexArray)(0);

        let atlas_rgba = build_roach_atlas();
        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            (ROACH_COLS * ROACH_CELL) as GLsizei,
            (ROACH_ROWS * ROACH_CELL) as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            atlas_rgba.as_ptr() as *const c_void,
        );
        (gl.TexParameteri)(
            GL_TEXTURE_2D,
            GL_TEXTURE_MIN_FILTER,
            GL_LINEAR_MIPMAP_LINEAR as GLint,
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as GLint);
        (gl.GenerateMipmap)(GL_TEXTURE_2D);

        Ok(Self {
            bg_prog,
            u_noise,
            u_fa,
            u_fb,
            u_res,
            u_grid,
            u_scale,
            u_seed,
            u_step,
            u_mix,
            u_time,
            u_glow,
            u_vibe,
            u_debug,
            bg_vao,
            noise_tex,
            ftex,
            tex_gw: 0,
            tex_gh: 0,
            inst_prog,
            u_inst_atlas,
            u_inst_res,
            u_inst_grid,
            inst_vao,
            inst_vbo_quad,
            inst_vbo_data,
            inst_cap,
            atlas_tex,
            instances: Vec::with_capacity(256),
        })
    }

    pub unsafe fn render(&mut self, gl: &GlFns, state: &mut AppState) {
        let rs = &mut state.roach;
        if rs.noise_tex_dirty {
            (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                NR as GLsizei,
                NR as GLsizei,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                rs.noise_rgba.as_ptr() as *const c_void,
            );
            rs.noise_tex_dirty = false;
        }

        let size_changed = self.tex_gw != rs.gw || self.tex_gh != rs.gh;
        if size_changed || rs.fa_dirty {
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[0]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 2);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_R16UI as GLint,
                rs.gw as GLsizei,
                rs.gh as GLsizei,
                0,
                GL_RED_INTEGER,
                GL_UNSIGNED_SHORT,
                rs.fa.as_ptr() as *const c_void,
            );
            rs.fa_dirty = false;
        }
        if size_changed || rs.fb_dirty {
            (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[1]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 2);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_R16UI as GLint,
                rs.gw as GLsizei,
                rs.gh as GLsizei,
                0,
                GL_RED_INTEGER,
                GL_UNSIGNED_SHORT,
                rs.fb.as_ptr() as *const c_void,
            );
            rs.fb_dirty = false;
        }
        self.tex_gw = rs.gw;
        self.tex_gh = rs.gh;

        // 1. Background cobblestone street & patio tile island shader pass
        (gl.Disable)(GL_BLEND);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, state.width_px.max(64), state.height_px.max(64));
        (gl.UseProgram)(self.bg_prog);
        (gl.BindVertexArray)(self.bg_vao);

        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.Uniform1i)(self.u_noise, 0);

        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[0]);
        (gl.Uniform1i)(self.u_fa, 1);

        (gl.ActiveTexture)(GL_TEXTURE2);
        (gl.BindTexture)(GL_TEXTURE_2D, self.ftex[1]);
        (gl.Uniform1i)(self.u_fb, 2);

        (gl.Uniform2f)(self.u_res, rs.w as f32, rs.h as f32);
        (gl.Uniform2f)(self.u_grid, rs.gw as f32, rs.gh as f32);
        (gl.Uniform1f)(self.u_scale, state.dpi_scale.max(0.5));
        (gl.Uniform1f)(self.u_seed, (rs.params.seed.rem_euclid(997)) as f32 * 0.37);
        (gl.Uniform1f)(self.u_step, STEP as f32);
        (gl.Uniform1f)(self.u_mix, rs.cur_mix as f32);
        (gl.Uniform1f)(self.u_time, rs.sim_time as f32);
        (gl.Uniform1f)(self.u_glow, rs.params.glow as f32);
        (gl.Uniform1f)(self.u_vibe, rs.params.vibe as f32);
        (gl.Uniform1f)(self.u_debug, if rs.params.debug { 1.0 } else { 0.0 });
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // 2. Instanced roaches, beds, streetlamps, outlets, jets, bombs, explosions & Turkish bubbles
        rs.build_draw_instances(&mut self.instances);
        if !self.instances.is_empty() {
            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
            (gl.UseProgram)(self.inst_prog);
            (gl.BindVertexArray)(self.inst_vao);

            (gl.ActiveTexture)(GL_TEXTURE0);
            (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
            (gl.Uniform1i)(self.u_inst_atlas, 0);
            (gl.Uniform2f)(self.u_inst_res, rs.w as f32, rs.h as f32);
            (gl.Uniform2f)(self.u_inst_grid, ROACH_COLS as f32, ROACH_ROWS as f32);

            (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_vbo_data);
            if self.instances.len() > self.inst_cap {
                self.inst_cap = self.instances.len().next_power_of_two().max(64);
                (gl.BufferData)(
                    GL_ARRAY_BUFFER,
                    (self.inst_cap * std::mem::size_of::<InstSprite>()) as isize,
                    std::ptr::null(),
                    GL_DYNAMIC_DRAW,
                );
            }
            (gl.BufferSubData)(
                GL_ARRAY_BUFFER,
                0,
                (self.instances.len() * std::mem::size_of::<InstSprite>()) as isize,
                self.instances.as_ptr() as *const c_void,
            );
            (gl.DrawArraysInstanced)(GL_TRIANGLES, 0, 6, self.instances.len() as GLsizei);
            (gl.BindVertexArray)(0);
            (gl.Disable)(GL_BLEND);
        }
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.bg_prog != 0 {
            (gl.DeleteProgram)(self.bg_prog);
            self.bg_prog = 0;
        }
        if self.inst_prog != 0 {
            (gl.DeleteProgram)(self.inst_prog);
            self.inst_prog = 0;
        }
        if self.bg_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.bg_vao);
            self.bg_vao = 0;
        }
        if self.inst_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.inst_vao);
            self.inst_vao = 0;
        }
        if self.inst_vbo_quad != 0 {
            (gl.DeleteBuffers)(1, &self.inst_vbo_quad);
            self.inst_vbo_quad = 0;
        }
        if self.inst_vbo_data != 0 {
            (gl.DeleteBuffers)(1, &self.inst_vbo_data);
            self.inst_vbo_data = 0;
        }
        if self.noise_tex != 0 {
            (gl.DeleteTextures)(1, &self.noise_tex);
            self.noise_tex = 0;
        }
        if self.atlas_tex != 0 {
            (gl.DeleteTextures)(1, &self.atlas_tex);
            self.atlas_tex = 0;
        }
        if self.ftex[0] != 0 {
            (gl.DeleteTextures)(2, self.ftex.as_ptr());
            self.ftex = [0; 2];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fnv1a_fingerprint(rel_path: &str, bytes: &[u8]) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        let prime: u64 = 0x00000100000001b3;
        for &b in rel_path.as_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(prime);
        }
        h ^= 0;
        h = h.wrapping_mul(prime);
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(prime);
        }
        h ^= 0;
        h = h.wrapping_mul(prime);
        format!("{:016x}", h)
    }

    #[test]
    fn verify_rust_anything_recorded_roach_cases() {
        let rec_str = include_str!("recorded-from.json");
        let rec: Value = serde_json::from_str(rec_str).expect("recorded-from.json parse");
        let sources = rec["sources"].as_object().expect("sources object");

        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        for (rel, exp_hash_val) in sources {
            let exp_hash = exp_hash_val.as_str().unwrap();
            let bytes = std::fs::read(repo_root.join(rel)).expect("read prototype source");
            let actual_hash = fnv1a_fingerprint(rel, &bytes);
            assert_eq!(
                actual_hash, exp_hash,
                "Source fingerprint mismatch for {}",
                rel
            );
        }

        let cases_str = include_str!("cases.jsonl");
        let mut count = 0usize;
        for line in cases_str.lines().filter(|l| !l.trim().is_empty()) {
            let c: Value = serde_json::from_str(line).expect("parse case line");
            let name = c["name"].as_str().unwrap();
            let inp = &c["input"];
            let exp = &c["expected"];

            let seed = inp["seed"].as_i64().unwrap() as i32;
            let w = inp["W"].as_f64().unwrap();
            let h = inp["H"].as_f64().unwrap();
            let p = &inp["P"];

            let mut st = RoachState::default();
            st.params.seed = seed;
            st.params.sigma = p["sigma"].as_f64().unwrap();
            st.params.warp = p["warp"].as_f64().unwrap();
            st.params.glow = p["glow"].as_f64().unwrap();
            st.params.crowd = p["crowd"].as_u64().unwrap() as usize;
            st.params.speed = p["speed"].as_f64().unwrap();
            st.params.props = p["props"].as_f64().unwrap();
            st.params.vibe = p["vibe"].as_f64().unwrap();
            st.params.adaptive = p["adaptive"].as_i64().unwrap() != 0;

            build_roach_noise(seed, &mut st.noise, &mut st.noise_rgba);
            st.resize(w as f32, h as f32);
            st.icon_boxes.clear();
            for ic in inp["icons"].as_array().unwrap() {
                let x = ic["x"].as_f64().unwrap();
                let y = ic["y"].as_f64().unwrap();
                let iw = ic["w"].as_f64().unwrap();
                let ih = ic["h"].as_f64().unwrap();
                st.icon_boxes
                    .push([x - 4.0, y - 4.0, x + iw + 4.0, y + ih + 4.0]);
            }
            st.has_committed_once = false;
            st.commit(true);

            assert_eq!(st.gw, exp["Gw"].as_u64().unwrap() as usize, "{}", name);
            assert_eq!(st.gh, exp["Gh"].as_u64().unwrap() as usize, "{}", name);

            let field_indices = [
                0,
                (st.gw / 4) + (st.gh / 4) * st.gw,
                (st.gw / 2) + (st.gh / 2) * st.gw,
                (st.gw * 3 / 4) + (st.gh * 3 / 4) * st.gw,
                st.gw * st.gh - 1,
            ];
            let exp_samples = exp["fieldSamples"].as_array().unwrap();
            for (i, &idx) in field_indices.iter().enumerate() {
                let actual = st.fb[idx] as i64;
                let expected = exp_samples[i].as_i64().unwrap();
                assert!(
                    (actual - expected).abs() <= 1,
                    "case {} sample {}: actual {} vs expected {}",
                    name,
                    i,
                    actual,
                    expected
                );
            }

            assert_eq!(
                st.roaches.len(),
                exp["roachCount"].as_u64().unwrap() as usize,
                "case {} roachCount",
                name
            );
            assert_eq!(
                st.street_props.len(),
                exp["propCount"].as_u64().unwrap() as usize,
                "case {} propCount",
                name
            );

            for (i, er) in exp["firstRoaches"].as_array().unwrap().iter().enumerate() {
                let ar = &st.roaches[i];
                assert_eq!(ar.kind, er["kind"].as_u64().unwrap() as usize);
                assert_eq!(ar.col, er["col"].as_u64().unwrap() as usize);
                assert_eq!(ar.row, er["row"].as_u64().unwrap() as usize);
                assert!((ar.x - er["x"].as_f64().unwrap()).abs() < 1e-2);
                assert!((ar.y - er["y"].as_f64().unwrap()).abs() < 1e-2);
                assert!((ar.vx - er["vx"].as_f64().unwrap()).abs() < 1e-2);
                assert!((ar.vy - er["vy"].as_f64().unwrap()).abs() < 1e-2);
            }

            for (i, ep) in exp["firstProps"].as_array().unwrap().iter().enumerate() {
                let ap = &st.street_props[i];
                assert_eq!(ap.col, ep["col"].as_u64().unwrap() as usize);
                assert_eq!(ap.row, ep["row"].as_u64().unwrap() as usize);
                assert!((ap.x - ep["x"].as_f64().unwrap()).abs() < 1e-2);
                assert!((ap.y - ep["y"].as_f64().unwrap()).abs() < 1e-2);
            }

            count += 1;
        }
        assert_eq!(count, 3);
    }
}
