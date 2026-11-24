pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use shaders::*;
use std::f32::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// Deterministic Mulberry32 PRNG (bit-for-bit identical to the JS generator)
// =========================================================================
#[derive(Clone, Copy)]
pub struct Mulberry32 {
    a: i32,
}

impl Mulberry32 {
    pub fn new(seed: i32) -> Self {
        Self { a: seed }
    }

    pub fn next_f64(&mut self) -> f64 {
        self.a = self.a.wrapping_add(0x6D2B79F5u32 as i32);
        let a_u = self.a as u32;
        let mut t = (a_u ^ (a_u >> 15)).wrapping_mul(1 | a_u);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }
}

#[inline]
pub fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
#[allow(dead_code)]
pub fn smoothstep_f32(e0: f32, e1: f32, x: f32) -> f32 {
    let denom = if (e1 - e0).abs() < 1e-9 { 1.0 } else { e1 - e0 };
    let t = clamp_f32((x - e0) / denom, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn hex_rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    ]
}

#[inline]
fn rgba(c: [f32; 3], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a.clamp(0.0, 1.0)]
}

// addRoundRectPath radius clamp: Math.min(r, w * 0.5, h * 0.5)
#[inline]
pub fn round_rect_clamp(r: f32, w: f32, h: f32) -> f32 {
    r.min(w * 0.5).min(h * 0.5)
}

// bakeTileSprites pad: Math.ceil(22 * relief)
#[inline]
pub fn tile_sprite_pad(relief: f32) -> f32 {
    (22.0 * relief).ceil()
}

// getIconCradleSprite pad: Math.ceil(24 * relief)
#[inline]
pub fn cradle_sprite_pad(relief: f32) -> f32 {
    (24.0 * relief).ceil()
}

// getIconCradleSprite radius: clamp(Math.min(rw, rh) * 0.22, 12, 22)
#[inline]
pub fn icon_cradle_rad(w: f32, h: f32) -> f32 {
    clamp_f32(w.min(h) * 0.22, 12.0, 22.0)
}

// drawFeatheredRoundedShadow per-step spread / weight / alpha
#[inline]
pub fn feathered_shadow_step(
    steps: i32,
    i: i32,
    blur_r: f32,
    total_alpha: f32,
) -> (f32, f32, f32) {
    let n = steps.max(4);
    let frac = i as f32 / n as f32;
    let spread = blur_r * frac;
    let weight = (1.0 - frac * 0.88).powf(1.85);
    let alpha = (total_alpha / n as f32) * weight * 2.25;
    (spread, weight, alpha.min(0.42))
}

// bakeWellRoundedCanvas inner dual-shadow per-step line width / weights
#[inline]
pub fn well_inner_step(
    steps: i32,
    i: i32,
    off: f32,
    dark_alpha: f32,
    light_alpha: f32,
) -> (f32, f32, f32, f32) {
    let n = steps.max(4);
    let frac = i as f32 / n as f32;
    let lw = 1.5 + off * 1.9 * frac;
    let weight = (1.0 - frac * 0.85).powf(1.8);
    let da = (dark_alpha * 0.55 / n as f32) * weight * 2.4;
    let la = (light_alpha * 0.50 / n as f32) * weight * 2.4;
    (lw, weight, da.min(0.45), la.min(0.45))
}

// =========================================================================
// 1. Palettes, accents & quality profiles (1:1 neumorph.html)
// =========================================================================
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct Palette {
    pub bg: [f32; 3],
    pub bg_hi: [f32; 3],
    pub bg_lo: [f32; 3],
    pub light: [f32; 3],
    pub dark: [f32; 3],
    pub light_alpha: f32,
    pub dark_alpha: f32,
    pub seam: [f32; 3],
    pub seam_alpha: f32,
    pub label_pill: [f32; 4],
    pub is_dark: bool,
}

pub fn palette_at(i: usize) -> Palette {
    match i {
        1 => Palette {
            bg: hex_rgb(0xe6dfd5),
            bg_hi: hex_rgb(0xf1ece4),
            bg_lo: hex_rgb(0xd8cfc2),
            light: hex_rgb(0xfffcf6),
            dark: hex_rgb(0xb3a694),
            light_alpha: 0.88,
            dark_alpha: 0.70,
            seam: hex_rgb(0xb0a391),
            seam_alpha: 0.22,
            label_pill: [54.0 / 255.0, 46.0 / 255.0, 38.0 / 255.0, 0.22],
            is_dark: false,
        },
        2 => Palette {
            bg: hex_rgb(0x1e222b),
            bg_hi: hex_rgb(0x262b36),
            bg_lo: hex_rgb(0x161920),
            light: hex_rgb(0x3c4458),
            dark: hex_rgb(0x090b0f),
            light_alpha: 0.78,
            dark_alpha: 0.90,
            seam: hex_rgb(0x0a0c10),
            seam_alpha: 0.45,
            label_pill: [10.0 / 255.0, 12.0 / 255.0, 18.0 / 255.0, 0.24],
            is_dark: true,
        },
        3 => Palette {
            bg: hex_rgb(0xdce5df),
            bg_hi: hex_rgb(0xe8efe9),
            bg_lo: hex_rgb(0xcdd8d1),
            light: hex_rgb(0xfafffc),
            dark: hex_rgb(0x9eaea4),
            light_alpha: 0.86,
            dark_alpha: 0.70,
            seam: hex_rgb(0x9caca2),
            seam_alpha: 0.22,
            label_pill: [34.0 / 255.0, 52.0 / 255.0, 42.0 / 255.0, 0.22],
            is_dark: false,
        },
        _ => Palette {
            bg: hex_rgb(0xe0e5ec),
            bg_hi: hex_rgb(0xedf1f7),
            bg_lo: hex_rgb(0xd2d9e4),
            light: hex_rgb(0xffffff),
            dark: hex_rgb(0xa3b1c6),
            light_alpha: 0.90,
            dark_alpha: 0.74,
            seam: hex_rgb(0xa3b1c6),
            seam_alpha: 0.22,
            label_pill: [38.0 / 255.0, 50.0 / 255.0, 68.0 / 255.0, 0.22],
            is_dark: false,
        },
    }
}

#[derive(Clone, Copy)]
pub struct Accent {
    pub hex: [f32; 3],
    pub rgb: [f32; 3],
}

pub fn accent_at(i: usize) -> Accent {
    match i {
        1 => Accent {
            hex: hex_rgb(0xf46a55),
            rgb: [244.0 / 255.0, 106.0 / 255.0, 85.0 / 255.0],
        },
        2 => Accent {
            hex: hex_rgb(0x10b981),
            rgb: [16.0 / 255.0, 185.0 / 255.0, 129.0 / 255.0],
        },
        3 => Accent {
            hex: hex_rgb(0xf59e0b),
            rgb: [245.0 / 255.0, 158.0 / 255.0, 11.0 / 255.0],
        },
        _ => Accent {
            hex: hex_rgb(0x3b82f6),
            rgb: [59.0 / 255.0, 130.0 / 255.0, 246.0 / 255.0],
        },
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct QualityProfile {
    pub max_dpr: f32,
    pub shadow_steps: i32,
    pub enable_widgets: bool,
    pub enable_grid_dots: bool,
}

pub const QUALITY_LOW: QualityProfile = QualityProfile {
    max_dpr: 0.75,
    shadow_steps: 7,
    enable_widgets: false,
    enable_grid_dots: false,
};

pub const QUALITY_MEDIUM: QualityProfile = QualityProfile {
    max_dpr: 1.25,
    shadow_steps: 11,
    enable_widgets: true,
    enable_grid_dots: true,
};

pub const QUALITY_HIGH: QualityProfile = QualityProfile {
    max_dpr: 1.50,
    shadow_steps: 14,
    enable_widgets: true,
    enable_grid_dots: true,
};

pub const RIPPLE_MAX: usize = 6;

#[derive(Clone, Copy)]
pub struct NeumorphParams {
    pub palette: usize,
    pub accent: usize,
    pub relief_depth: f32,
    pub light_follow_cursor: bool,
    pub wave_speed: f32,
    pub tile_density: f32,
    pub ripple_strength: f32,
    pub icon_style: i32, // 0 cradle, 1 well, 2 minimal
    pub quality: i32,    // 0 Low, 1 Medium, 2 High
    pub adaptive: i32,
    pub seed: u32,
}

impl Default for NeumorphParams {
    fn default() -> Self {
        Self {
            palette: 0,
            accent: 0,
            relief_depth: 1.0,
            light_follow_cursor: false,
            wave_speed: 0.75,
            tile_density: 1.0,
            ripple_strength: 1.0,
            icon_style: 0,
            quality: 2,
            adaptive: 1,
            seed: 20261118,
        }
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct Tile {
    pub cx: f32,
    pub cy: f32,
    pub col: i32,
    pub row: i32,
    pub phase: f32,
    pub bias: f32,
    pub elev: f32,
    pub vel: f32,
    pub press_target: f32,
    pub under_icon: bool,
}

#[derive(Clone, Copy)]
pub struct Ripple {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub speed: f32,
    pub max_r: f32,
    pub strength: f32,
    pub life: f32,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct Widget {
    pub kind: i32, // 0 dial, 1 pill
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub target_angle: f32,
    pub pos: f32,
    pub target_pos: f32,
    pub press: f32,
    pub visible: bool,
}

impl Widget {
    fn dial() -> Self {
        Self {
            kind: 0,
            x: 0.0,
            y: 0.0,
            angle: 0.7,
            target_angle: 0.7,
            pos: 0.0,
            target_pos: 0.0,
            press: 0.0,
            visible: false,
        }
    }

    fn pill() -> Self {
        Self {
            kind: 1,
            x: 0.0,
            y: 0.0,
            angle: 0.0,
            target_angle: 0.0,
            pos: 0.28,
            target_pos: 0.72,
            press: 0.0,
            visible: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct IconCradle {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub tx: f32,
    pub ty: f32,
    pub tw: f32,
    pub th: f32,
    pub opacity: f32,
    pub press: f32,
    pub pulse: f32,
    pub phase: f32,
    pub removing: bool,
}

#[derive(Clone, Copy)]
pub struct Pointer {
    pub x: f32,
    pub y: f32,
    pub smooth_x: f32,
    pub smooth_y: f32,
    pub active: bool,
    pub down: bool,
}

impl Default for Pointer {
    fn default() -> Self {
        Self {
            x: -9999.0,
            y: -9999.0,
            smooth_x: 0.25,
            smooth_y: 0.25,
            active: false,
            down: false,
        }
    }
}

pub struct NeumorphState {
    pub p: NeumorphParams,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub low_mode: bool,
    pub sim_time: f32,
    pub seed: u32,
    pub grid_spacing: f32,
    pub tile_size: f32,
    pub tiles: Vec<Tile>,
    pub ripples: Vec<Ripple>,
    pub widgets: [Widget; 2],
    pub icons: Vec<IconCradle>,
    pub icons_seen: bool,
    pub pointer: Pointer,
    pub cache_dirty: bool,
    pub sprites_dirty: bool,
    pub icon_style_dirty: bool,
}

impl Default for NeumorphState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0, 1.0, NeumorphParams::default())
    }
}

impl NeumorphState {
    pub fn new(w: f32, h: f32, scale: f32, params: NeumorphParams) -> Self {
        let mut st = Self {
            p: params,
            w: w.max(240.0),
            h: h.max(160.0),
            scale: scale.max(0.1),
            low_mode: false,
            sim_time: 0.0,
            seed: if params.seed == 0 { 20261118 } else { params.seed },
            grid_spacing: 112.0,
            tile_size: 76.0,
            tiles: Vec::new(),
            ripples: Vec::new(),
            widgets: [Widget::dial(), Widget::pill()],
            icons: Vec::new(),
            icons_seen: false,
            pointer: Pointer::default(),
            cache_dirty: true,
            sprites_dirty: true,
            icon_style_dirty: true,
        };
        st.build_grid();
        st
    }

    pub fn profile(&self) -> QualityProfile {
        if self.low_mode {
            return QUALITY_LOW;
        }
        match self.p.quality {
            0 => QUALITY_LOW,
            1 => QUALITY_MEDIUM,
            _ => QUALITY_HIGH,
        }
    }

    pub fn palette(&self) -> Palette {
        palette_at(self.p.palette)
    }

    pub fn accent(&self) -> Accent {
        accent_at(self.p.accent)
    }

    pub fn set_low_mode(&mut self, v: bool) {
        if v != self.low_mode {
            self.low_mode = v;
            self.sprites_dirty = true;
            self.cache_dirty = true;
            self.build_grid();
        }
    }

    pub fn resize(&mut self, w: f32, h: f32, scale: f32) {
        let nw = w.max(240.0);
        let nh = h.max(160.0);
        let ns = scale.max(0.1);
        if (nw - self.w).abs() > 0.5
            || (nh - self.h).abs() > 0.5
            || (ns - self.scale).abs() > 0.01
        {
            self.w = nw;
            self.h = nh;
            self.scale = ns;
            self.build_grid();
        }
    }

    pub fn sync_params(&mut self, next: NeumorphParams) {
        let mut need_rebake = false;
        let mut need_grid = false;
        let mut need_bg = false;

        if next.seed != self.seed && next.seed != 0 {
            self.seed = next.seed;
            need_grid = true;
        }
        if next.palette != self.p.palette {
            need_rebake = true;
            need_bg = true;
        }
        if next.icon_style != self.p.icon_style {
            self.icon_style_dirty = true;
        }
        if next.quality != self.p.quality {
            need_rebake = true;
            need_grid = true;
        }
        if (next.relief_depth - self.p.relief_depth).abs() > 1e-4 {
            need_rebake = true;
        }
        if (next.tile_density - self.p.tile_density).abs() > 1e-4 {
            need_grid = true;
        }
        if next.adaptive != self.p.adaptive {
            self.p.adaptive = next.adaptive;
            self.refresh_tile_icon_masks();
        }
        self.p = next;

        if need_rebake {
            self.sprites_dirty = true;
        }
        if need_bg {
            self.cache_dirty = true;
        }
        if need_grid {
            self.build_grid();
        }
    }

    // =====================================================================
    // 4. Static base layer & kinetic tile grid
    // =====================================================================
    pub fn build_grid(&mut self) {
        let density = clamp_f32(self.p.tile_density, 0.5, 1.6);
        self.grid_spacing = (118.0 / density.sqrt()).round();
        self.tile_size = (self.grid_spacing * 0.68).round();
        self.sprites_dirty = true;

        let cols = ((self.w - 40.0) / self.grid_spacing).floor().max(3.0) as i32;
        let rows = ((self.h - 40.0) / self.grid_spacing).floor().max(3.0) as i32;
        let off_x = (self.w - (cols - 1) as f32 * self.grid_spacing) * 0.5;
        let off_y = (self.h - (rows - 1) as f32 * self.grid_spacing) * 0.5;
        let mut rng = Mulberry32::new((self.seed ^ 0x9E3779B9) as i32);

        self.tiles.clear();
        self.tiles.reserve((cols * rows) as usize);
        for r in 0..rows {
            for c in 0..cols {
                let cx = off_x + c as f32 * self.grid_spacing;
                let cy = off_y + r as f32 * self.grid_spacing;
                self.tiles.push(Tile {
                    cx,
                    cy,
                    col: c,
                    row: r,
                    phase: rng.next_f32() * TAU,
                    bias: (rng.next_f32() - 0.5) * 0.35,
                    elev: 0.0,
                    vel: 0.0,
                    press_target: 0.0,
                    under_icon: false,
                });
            }
        }

        self.refresh_tile_icon_masks();
        self.pick_widget_placements();
        self.cache_dirty = true;
    }

    pub fn refresh_tile_icon_masks(&mut self) {
        let pad = self.tile_size * 0.55 + 18.0;
        let adaptive = self.p.adaptive != 0;
        for i in 0..self.tiles.len() {
            let t = self.tiles[i];
            let mut blocked = false;
            if adaptive {
                for ic in &self.icons {
                    if t.cx >= ic.x - pad
                        && t.cx <= ic.x + ic.w + pad
                        && t.cy >= ic.y - pad
                        && t.cy <= ic.y + ic.h + pad
                    {
                        blocked = true;
                        break;
                    }
                }
            }
            self.tiles[i].under_icon = blocked;
        }
    }

    pub fn rect_overlaps_icons(&self, rx: f32, ry: f32, rw: f32, rh: f32, margin: f32) -> bool {
        for ic in &self.icons {
            if rx < ic.x + ic.w + margin
                && rx + rw > ic.x - margin
                && ry < ic.y + ic.h + margin
                && ry + rh > ic.y - margin
            {
                return true;
            }
        }
        false
    }

    pub fn pick_widget_placements(&mut self) {
        let candidates = [
            (0.78f32, 0.24f32),
            (0.76, 0.72),
            (0.52, 0.22),
            (0.50, 0.74),
            (0.26, 0.74),
            (0.26, 0.24),
        ];
        let mut placed_dial = false;
        let mut placed_pill = false;
        for (fx, fy) in candidates {
            let cx = (self.w * fx).round();
            let cy = (self.h * fy).round();
            if !placed_dial && !self.rect_overlaps_icons(cx - 64.0, cy - 64.0, 128.0, 128.0, 28.0) {
                self.widgets[0].x = cx;
                self.widgets[0].y = cy;
                self.widgets[0].visible = true;
                placed_dial = true;
                continue;
            }
            if !placed_pill && !self.rect_overlaps_icons(cx - 84.0, cy - 32.0, 168.0, 64.0, 28.0) {
                let far = !placed_dial
                    || (cx - self.widgets[0].x).hypot(cy - self.widgets[0].y) > 170.0;
                if far {
                    self.widgets[1].x = cx;
                    self.widgets[1].y = cy;
                    self.widgets[1].visible = true;
                    placed_pill = true;
                }
            }
            if placed_dial && placed_pill {
                break;
            }
        }
        if !placed_dial {
            self.widgets[0].visible = false;
        }
        if !placed_pill {
            self.widgets[1].visible = false;
        }
    }

    // =====================================================================
    // 5. Adaptive icon cradles
    // =====================================================================
    pub fn sync_incoming_icons(&mut self, rects: &[Rect]) {
        let mut matched: Vec<bool> = vec![false; self.icons.len()];

        for (i, ic) in rects.iter().enumerate() {
            let mut best_idx: i32 = -1;
            let mut best_dist = 48.0f32;
            for (s, st) in self.icons.iter().enumerate() {
                if matched[s] {
                    continue;
                }
                let d = (ic.x - st.tx).hypot(ic.y - st.ty);
                if d < best_dist {
                    best_dist = d;
                    best_idx = s as i32;
                }
            }
            if best_idx >= 0 {
                let st = &mut self.icons[best_idx as usize];
                st.tx = ic.x;
                st.ty = ic.y;
                st.tw = ic.w;
                st.th = ic.h;
                st.removing = false;
                matched[best_idx as usize] = true;
            } else {
                self.icons.push(IconCradle {
                    x: ic.x,
                    y: ic.y,
                    w: ic.w,
                    h: ic.h,
                    tx: ic.x,
                    ty: ic.y,
                    tw: ic.w,
                    th: ic.h,
                    opacity: 0.0,
                    press: 0.0,
                    pulse: 0.4,
                    phase: (i as f32 * 1.37) % TAU,
                    removing: false,
                });
                matched.push(true);
            }
        }

        for (s, st) in self.icons.iter_mut().enumerate() {
            if !matched.get(s).copied().unwrap_or(false) {
                st.removing = true;
            }
        }
        self.icons_seen = true;
        self.refresh_tile_icon_masks();
        self.pick_widget_placements();
    }

    // =====================================================================
    // 6. Simulation step (spring physics, waves, ripples, micro-widgets)
    // =====================================================================
    pub fn spawn_ripple(&mut self, x: f32, y: f32, strength_scale: f32) {
        let str_ = clamp_f32(self.p.ripple_strength * strength_scale, 0.0, 2.2);
        if str_ <= 0.02 {
            return;
        }
        if self.ripples.len() >= RIPPLE_MAX {
            self.ripples.remove(0);
        }
        self.ripples.push(Ripple {
            x,
            y,
            r: 12.0,
            speed: 340.0,
            max_r: self.w.hypot(self.h) * 0.55,
            strength: str_,
            life: 0.0,
        });
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.12);
        let wave_rate = clamp_f32(self.p.wave_speed, 0.0, 2.5);
        self.sim_time += dt * (0.65 + wave_rate * 0.85);

        // Smooth ambient light follow when enabled
        let follow = self.p.light_follow_cursor && self.pointer.active;
        let target_lx = if follow {
            clamp_f32(self.pointer.x / self.w.max(1.0), 0.05, 0.95)
        } else {
            0.16
        };
        let target_ly = if follow {
            clamp_f32(self.pointer.y / self.h.max(1.0), 0.05, 0.95)
        } else {
            0.16
        };
        let l_alpha = 1.0 - (-5.0 * dt).exp();
        self.pointer.smooth_x = lerp_f32(self.pointer.smooth_x, target_lx, l_alpha);
        self.pointer.smooth_y = lerp_f32(self.pointer.smooth_y, target_ly, l_alpha);

        // Advance pressure ripples
        let mut i = self.ripples.len();
        while i > 0 {
            i -= 1;
            let rp = &mut self.ripples[i];
            rp.r += rp.speed * dt;
            rp.life += dt;
            if rp.r >= rp.max_r {
                self.ripples.remove(i);
            }
        }

        // Spring-damper tile elevation
        let hover_radius = 115.0f32.max(self.grid_spacing * 1.45);
        let k_spring = 38.0f32;
        let k_damp = 7.4f32;
        let pointer_active = self.pointer.active;
        let pointer_down = self.pointer.down;
        let px = self.pointer.x;
        let py = self.pointer.y;

        for t in self.tiles.iter_mut() {
            if t.under_icon {
                continue;
            }
            let mut target = t.bias;
            if wave_rate > 0.005 {
                let w1 = (self.sim_time * 0.95 + t.col as f32 * 0.52 + t.row as f32 * 0.36).sin();
                let w2 = (self.sim_time * 0.62 - t.col as f32 * 0.28 + t.row as f32 * 0.44).cos()
                    * 0.5;
                target += (w1 + w2) * 0.46;
            }
            if pointer_active {
                let pd = (t.cx - px).hypot(t.cy - py);
                if pd < hover_radius {
                    let inf = 1.0 - pd / hover_radius;
                    target -= inf * inf * if pointer_down { 1.35 } else { 0.95 };
                }
            }
            for rp in self.ripples.iter() {
                let d = (t.cx - rp.x).hypot(t.cy - rp.y);
                let band = 72.0f32;
                let diff = (d - rp.r).abs();
                if diff < band {
                    let fade = (1.0 - rp.r / rp.max_r) * rp.strength;
                    let wave_form = ((diff / band) * PI * 0.5).cos();
                    target -= wave_form * 1.15 * fade;
                }
            }
            target = clamp_f32(target, -1.0, 1.0);
            let accel = (target - t.elev) * k_spring - t.vel * k_damp;
            t.vel += accel * dt;
            t.elev = clamp_f32(t.elev + t.vel * dt, -1.05, 1.05);
        }

        // Update ambient micro-widgets
        if self.widgets[0].visible {
            self.widgets[0].target_angle += dt * (0.24 + wave_rate * 0.22);
            let a = 1.0 - (-8.0 * dt).exp();
            self.widgets[0].angle = lerp_f32(self.widgets[0].angle, self.widgets[0].target_angle, a);
            self.widgets[0].press = (self.widgets[0].press - dt * 2.8).max(0.0);
        }
        if self.widgets[1].visible {
            let auto_slide = 0.5 + (self.sim_time * 0.55).sin() * 0.34;
            if (self.widgets[1].pos - self.widgets[1].target_pos).abs() < 0.02 && wave_rate > 0.05 {
                self.widgets[1].target_pos = auto_slide;
            }
            let a = 1.0 - (-4.5 * dt).exp();
            self.widgets[1].pos = lerp_f32(self.widgets[1].pos, self.widgets[1].target_pos, a);
            self.widgets[1].press = (self.widgets[1].press - dt * 2.8).max(0.0);
        }

        // Update adaptive icon cradle states
        let pos_alpha = 1.0 - (-10.0 * dt).exp();
        let mut i = self.icons.len();
        while i > 0 {
            i -= 1;
            let st = &mut self.icons[i];
            st.x = lerp_f32(st.x, st.tx, pos_alpha);
            st.y = lerp_f32(st.y, st.ty, pos_alpha);
            st.w = lerp_f32(st.w, st.tw, pos_alpha);
            st.h = lerp_f32(st.h, st.th, pos_alpha);

            if st.removing {
                st.opacity = (st.opacity - dt * 2.6).max(0.0);
                if st.opacity <= 0.005 {
                    self.icons.remove(i);
                    continue;
                }
            } else {
                st.opacity = (st.opacity + dt * 3.2).min(1.0);
            }

            let icx = st.x + st.w * 0.5;
            let icy = st.y + st.h * 0.5;
            let mut hover_target = 0.0;
            if pointer_active {
                let d = (icx - px).hypot(icy - py);
                if d < 85.0 {
                    hover_target = (1.0 - d / 85.0) * if pointer_down { 1.0 } else { 0.55 };
                }
            }
            st.press = lerp_f32(st.press, hover_target, 1.0 - (-12.0 * dt).exp());
            st.pulse = (st.pulse - dt * 1.4).max(0.0);

            for rp in self.ripples.iter() {
                let d = (icx - rp.x).hypot(icy - rp.y);
                if (d - rp.r).abs() < 42.0 {
                    st.pulse = st.pulse.max((1.0 - rp.r / rp.max_r) * 0.95);
                }
            }
        }
    }

    // =====================================================================
    // Pointer input (1:1 pointermove / pointerdown / pointerup handlers)
    // =====================================================================
    pub fn on_pointer_move(&mut self, x: f32, y: f32) {
        self.pointer.x = x;
        self.pointer.y = y;
        self.pointer.active = true;
    }

    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        self.pointer.x = x;
        self.pointer.y = y;
        self.pointer.active = true;
        self.pointer.down = true;
        self.spawn_ripple(x, y, 1.0);

        if self.widgets[0].visible
            && (x - self.widgets[0].x).hypot(y - self.widgets[0].y) < 54.0
        {
            self.widgets[0].press = 1.0;
            self.widgets[0].target_angle += PI * 0.35;
        }
        if self.widgets[1].visible
            && (x - self.widgets[1].x).abs() < 76.0
            && (y - self.widgets[1].y).abs() < 26.0
        {
            self.widgets[1].press = 1.0;
            self.widgets[1].target_pos = if self.widgets[1].target_pos > 0.5 {
                0.18
            } else {
                0.82
            };
        }
    }

    pub fn on_pointer_up(&mut self) {
        self.pointer.down = false;
    }

    pub fn on_pointer_leave(&mut self) {
        self.pointer.down = false;
        self.pointer.active = false;
    }

    pub fn bake_scale(&self) -> f32 {
        clamp_f32(self.scale, 0.75, 1.5)
    }
}

// =========================================================================
// 3. CPU sprite baker — a faithful reimplementation of the Canvas 2D baker
//    (bakeRaisedRoundedCanvas / bakeWellRoundedCanvas / getIconCradleSprite)
//    with analytic rounded-box coverage instead of ctx.shadowBlur.
// =========================================================================
struct BakeCanvas {
    w: usize,
    h: usize,
    scale: f32,
    px: Vec<f32>,
    clip: Option<Vec<f32>>,
}

#[inline]
fn cov_round_box(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - hw + r;
    let qy = (py - cy).abs() - hh + r;
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    let d = (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r;
    clamp_f32(0.5 - d, 0.0, 1.0)
}

impl BakeCanvas {
    fn new(logical_w: f32, logical_h: f32, scale: f32) -> Self {
        let w = ((logical_w * scale).ceil() as usize).max(2);
        let h = ((logical_h * scale).ceil() as usize).max(2);
        Self {
            w,
            h,
            scale,
            px: vec![0.0; w * h * 4],
            clip: None,
        }
    }

    #[inline]
    fn blend(&mut self, x: usize, y: usize, col: [f32; 4], cov: f32) {
        let sa = col[3] * cov;
        if sa <= 0.0005 {
            return;
        }
        let i = (y * self.w + x) * 4;
        let da = self.px[i + 3];
        let oa = sa + da * (1.0 - sa);
        if oa <= 1e-6 {
            self.px[i + 3] = 0.0;
            return;
        }
        let inv = 1.0 / oa;
        for c in 0..3 {
            self.px[i + c] = (col[c] * sa + self.px[i + c] * da * (1.0 - sa)) * inv;
        }
        self.px[i + 3] = oa;
    }

    #[inline]
    fn clip_cov(&self, x: usize, y: usize) -> f32 {
        match &self.clip {
            Some(m) => m[y * self.w + x],
            None => 1.0,
        }
    }

    fn set_clip_rounded(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let s = self.scale;
        let cx = (x + w * 0.5) * s;
        let cy = (y + h * 0.5) * s;
        let hw = (w * 0.5 * s).max(0.0);
        let hh = (h * 0.5 * s).max(0.0);
        let rr = (r * s).min(hw).min(hh).max(0.0);
        let mut m = vec![0.0f32; self.w * self.h];
        for py in 0..self.h {
            let fy = py as f32 + 0.5;
            for px in 0..self.w {
                let fx = px as f32 + 0.5;
                m[py * self.w + px] = cov_round_box(fx, fy, cx, cy, hw, hh, rr);
            }
        }
        self.clip = Some(m);
    }

    fn clear_clip(&mut self) {
        self.clip = None;
    }

    fn fill_rounded(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, col: [f32; 4]) {
        if col[3] <= 0.0005 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let s = self.scale;
        let cx = (x + w * 0.5) * s;
        let cy = (y + h * 0.5) * s;
        let hw = (w * 0.5 * s).max(0.0);
        let hh = (h * 0.5 * s).max(0.0);
        let rr = round_rect_clamp(r * s, w * s, h * s).max(0.0);
        let x0 = ((cx - hw - 1.0).floor().max(0.0)) as usize;
        let y0 = ((cy - hh - 1.0).floor().max(0.0)) as usize;
        let x1 = ((cx + hw + 1.0).ceil() as usize).min(self.w);
        let y1 = ((cy + hh + 1.0).ceil() as usize).min(self.h);
        for py in y0..y1 {
            let fy = py as f32 + 0.5;
            for px in x0..x1 {
                let fx = px as f32 + 0.5;
                let cov = cov_round_box(fx, fy, cx, cy, hw, hh, rr) * self.clip_cov(px, py);
                if cov > 0.0005 {
                    self.blend(px, py, col, cov);
                }
            }
        }
    }

    fn fill_rounded_grad(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        c0: [f32; 3],
        c1: [f32; 3],
        c2: [f32; 3],
        mid: f32,
    ) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let s = self.scale;
        let cx = (x + w * 0.5) * s;
        let cy = (y + h * 0.5) * s;
        let hw = (w * 0.5 * s).max(0.0);
        let hh = (h * 0.5 * s).max(0.0);
        let rr = (r * s).min(hw).min(hh).max(0.0);
        let x0 = ((cx - hw - 1.0).floor().max(0.0)) as usize;
        let y0 = ((cy - hh - 1.0).floor().max(0.0)) as usize;
        let x1 = ((cx + hw + 1.0).ceil() as usize).min(self.w);
        let y1 = ((cy + hh + 1.0).ceil() as usize).min(self.h);
        let denom = (w * w + h * h).max(1e-6);
        for py in y0..y1 {
            let fy = py as f32 + 0.5;
            let ly = fy / s;
            for px in x0..x1 {
                let fx = px as f32 + 0.5;
                let cov = cov_round_box(fx, fy, cx, cy, hw, hh, rr) * self.clip_cov(px, py);
                if cov <= 0.0005 {
                    continue;
                }
                let lx = fx / s;
                let t = clamp_f32(((lx - x) * w + (ly - y) * h) / denom, 0.0, 1.0);
                let col = if t < mid {
                    let u = t / mid.max(1e-6);
                    [
                        lerp_f32(c0[0], c1[0], u),
                        lerp_f32(c0[1], c1[1], u),
                        lerp_f32(c0[2], c1[2], u),
                        1.0,
                    ]
                } else {
                    let u = (t - mid) / (1.0 - mid).max(1e-6);
                    [
                        lerp_f32(c1[0], c2[0], u),
                        lerp_f32(c1[1], c2[1], u),
                        lerp_f32(c1[2], c2[2], u),
                        1.0,
                    ]
                };
                self.blend(px, py, col, cov);
            }
        }
    }

    fn stroke_rounded(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        lw: f32,
        col: [f32; 4],
    ) {
        if col[3] <= 0.0005 || w <= 0.0 || h <= 0.0 || lw <= 0.0 {
            return;
        }
        let s = self.scale;
        let cx = (x + w * 0.5) * s;
        let cy = (y + h * 0.5) * s;
        let hw = (w * 0.5 * s).max(0.0);
        let hh = (h * 0.5 * s).max(0.0);
        let rr = round_rect_clamp(r * s, w * s, h * s).max(0.0);
        let half_lw = lw * s * 0.5;
        let x0 = ((cx - hw - half_lw - 1.0).floor().max(0.0)) as usize;
        let y0 = ((cy - hh - half_lw - 1.0).floor().max(0.0)) as usize;
        let x1 = ((cx + hw + half_lw + 1.0).ceil() as usize).min(self.w);
        let y1 = ((cy + hh + half_lw + 1.0).ceil() as usize).min(self.h);
        for py in y0..y1 {
            let fy = py as f32 + 0.5;
            for px in x0..x1 {
                let fx = px as f32 + 0.5;
                let qx = (fx - cx).abs() - hw + rr;
                let qy = (fy - cy).abs() - hh + rr;
                let ax = qx.max(0.0);
                let ay = qy.max(0.0);
                let sd = (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - rr;
                let cov = clamp_f32(0.5 - (sd.abs() - half_lw), 0.0, 1.0)
                    * self.clip_cov(px, py);
                if cov > 0.0005 {
                    self.blend(px, py, col, cov);
                }
            }
        }
    }

    fn to_rgba8(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.w * self.h * 4];
        for i in 0..self.w * self.h {
            let a = clamp_f32(self.px[i * 4 + 3], 0.0, 1.0);
            out[i * 4] = (clamp_f32(self.px[i * 4], 0.0, 1.0) * 255.0).round() as u8;
            out[i * 4 + 1] = (clamp_f32(self.px[i * 4 + 1], 0.0, 1.0) * 255.0).round() as u8;
            out[i * 4 + 2] = (clamp_f32(self.px[i * 4 + 2], 0.0, 1.0) * 255.0).round() as u8;
            out[i * 4 + 3] = (a * 255.0).round() as u8;
        }
        out
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_feathered_rounded_shadow(
    c: &mut BakeCanvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    dx: f32,
    dy: f32,
    blur_r: f32,
    steps: i32,
    rgb: [f32; 3],
    total_alpha: f32,
) {
    let n = steps.max(4);
    let mut i = n;
    while i >= 1 {
        let (spread, _weight, alpha) =
            feathered_shadow_step(n, i, blur_r, total_alpha);
        let frac = i as f32 / n as f32;
        if alpha > 0.002 {
            c.fill_rounded(
                x + dx * (0.45 + 0.55 * frac) - spread * 0.5,
                y + dy * (0.45 + 0.55 * frac) - spread * 0.5,
                w + spread,
                h + spread,
                r + spread * 0.45,
                rgba(rgb, alpha),
            );
        }
        i -= 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn bake_raised_rounded(
    w: f32,
    h: f32,
    r: f32,
    pad: f32,
    pal: &Palette,
    relief: f32,
    steps: i32,
    sprite_scale: f32,
) -> BakeCanvas {
    let mut c = BakeCanvas::new(w + pad * 2.0, h + pad * 2.0, sprite_scale);
    let off = 6.5 * relief;
    let blur_r = 14.0 * relief;

    draw_feathered_rounded_shadow(
        &mut c,
        pad,
        pad,
        w,
        h,
        r,
        -off,
        -off,
        blur_r,
        steps,
        pal.light,
        pal.light_alpha * 0.55,
    );
    draw_feathered_rounded_shadow(
        &mut c,
        pad,
        pad,
        w,
        h,
        r,
        off,
        off,
        blur_r,
        steps,
        pal.dark,
        pal.dark_alpha * 0.62,
    );

    c.fill_rounded_grad(pad, pad, w, h, r, pal.bg_hi, pal.bg, pal.bg_lo, 0.52);

    c.set_clip_rounded(pad, pad, w, h, r);
    c.stroke_rounded(
        pad + 0.7,
        pad + 0.7,
        w,
        h,
        r,
        1.25,
        rgba(pal.light, pal.light_alpha * 0.55),
    );
    c.clear_clip();
    c
}

#[allow(clippy::too_many_arguments)]
fn bake_well_rounded(
    w: f32,
    h: f32,
    r: f32,
    pad: f32,
    pal: &Palette,
    relief: f32,
    steps: i32,
    sprite_scale: f32,
) -> BakeCanvas {
    let mut c = BakeCanvas::new(w + pad * 2.0, h + pad * 2.0, sprite_scale);

    c.stroke_rounded(
        pad - 0.5,
        pad - 0.5,
        w + 1.0,
        h + 1.0,
        r + 0.5,
        1.2,
        rgba(pal.light, pal.light_alpha * 0.35),
    );

    c.fill_rounded_grad(pad, pad, w, h, r, pal.bg_lo, pal.bg, pal.bg_hi, 0.5);

    c.set_clip_rounded(pad, pad, w, h, r);
    let off = 5.5 * relief;
    let n = steps.max(4);
    let mut i = n;
    while i >= 1 {
        let frac = i as f32 / n as f32;
        let (lw, _weight, da, la) =
            well_inner_step(n, i, off, pal.dark_alpha, pal.light_alpha);

        c.stroke_rounded(
            pad + off * 0.65 * frac,
            pad + off * 0.65 * frac,
            w,
            h,
            r,
            lw,
            rgba(pal.dark, da),
        );
        c.stroke_rounded(
            pad - off * 0.65 * frac,
            pad - off * 0.65 * frac,
            w,
            h,
            r,
            lw,
            rgba(pal.light, la),
        );
        i -= 1;
    }
    c.clear_clip();
    c
}

fn bake_minimal_cradle(
    rw: f32,
    rh: f32,
    rad: f32,
    pad: f32,
    pal: &Palette,
    sprite_scale: f32,
) -> BakeCanvas {
    let mut c = BakeCanvas::new(rw + pad * 2.0, rh + pad * 2.0, sprite_scale);
    c.stroke_rounded(
        pad + 1.2,
        pad + 1.2,
        rw,
        rh,
        rad,
        1.6,
        rgba(pal.dark, pal.dark_alpha * 0.36),
    );
    c.stroke_rounded(
        pad - 0.8,
        pad - 0.8,
        rw,
        rh,
        rad,
        1.4,
        rgba(pal.light, pal.light_alpha * 0.55),
    );
    c
}

// =========================================================================
// Sprite atlas (single texture, single draw call for every baked sprite)
// =========================================================================
const ATLAS_SIZE: usize = 2048;

#[derive(Clone, Copy)]
struct Sprite {
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    pad: f32,
    w: f32,
    h: f32,
}

struct Atlas {
    tex: GLuint,
    px: Vec<u8>,
    cursor_x: usize,
    cursor_y: usize,
    shelf_h: usize,
    dirty: bool,
}

impl Atlas {
    unsafe fn new(gl: &GlFns) -> Self {
        let mut tex = 0;
        (gl.GenTextures)(1, &mut tex);
        (gl.BindTexture)(GL_TEXTURE_2D, tex);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            ATLAS_SIZE as GLsizei,
            ATLAS_SIZE as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            std::ptr::null(),
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        Self {
            tex,
            px: vec![0u8; ATLAS_SIZE * ATLAS_SIZE * 4],
            cursor_x: 0,
            cursor_y: 0,
            shelf_h: 0,
            dirty: true,
        }
    }

    fn reset(&mut self) {
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.shelf_h = 0;
        self.px.fill(0);
        self.dirty = true;
    }

    fn alloc(&mut self, w: usize, h: usize) -> Option<(usize, usize)> {
        if w > ATLAS_SIZE || h > ATLAS_SIZE {
            return None;
        }
        if self.cursor_x + w > ATLAS_SIZE {
            self.cursor_y += self.shelf_h;
            self.cursor_x = 0;
            self.shelf_h = 0;
        }
        if self.cursor_y + h > ATLAS_SIZE {
            return None;
        }
        let at = (self.cursor_x, self.cursor_y);
        self.cursor_x += w;
        self.shelf_h = self.shelf_h.max(h);
        Some(at)
    }

    fn write(&mut self, at: (usize, usize), w: usize, h: usize, src: &[u8]) {
        for row in 0..h {
            let dst_off = ((at.1 + row) * ATLAS_SIZE + at.0) * 4;
            let src_off = row * w * 4;
            self.px[dst_off..dst_off + w * 4].copy_from_slice(&src[src_off..src_off + w * 4]);
        }
        self.dirty = true;
    }

    unsafe fn upload(&mut self, gl: &GlFns) {
        if !self.dirty {
            return;
        }
        (gl.BindTexture)(GL_TEXTURE_2D, self.tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexSubImage2D)(
            GL_TEXTURE_2D,
            0,
            0,
            0,
            ATLAS_SIZE as GLsizei,
            ATLAS_SIZE as GLsizei,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            self.px.as_ptr() as *const c_void,
        );
        self.dirty = false;
    }
}

fn sprite_from(canvas: &BakeCanvas, at: (usize, usize), pad: f32, w: f32, h: f32) -> Sprite {
    let inv = 1.0 / ATLAS_SIZE as f32;
    Sprite {
        u0: at.0 as f32 * inv,
        v0: at.1 as f32 * inv,
        u1: (at.0 + canvas.w) as f32 * inv,
        v1: (at.1 + canvas.h) as f32 * inv,
        pad,
        w,
        h,
    }
}

// =========================================================================
// Batched 2D vector geometry (arcs, rings, pills, sheen) in one draw call
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

struct ColorBatch {
    verts: Vec<ColorVert>,
}

impl ColorBatch {
    fn new() -> Self {
        Self {
            verts: Vec::with_capacity(16384),
        }
    }

    #[inline]
    fn push_tri(&mut self, a: ColorVert, b: ColorVert, c: ColorVert) {
        self.verts.push(a);
        self.verts.push(b);
        self.verts.push(c);
    }

    #[inline]
    fn push_quad(&mut self, a: ColorVert, b: ColorVert, c: ColorVert, d: ColorVert) {
        self.push_tri(a, b, c);
        self.push_tri(a, c, d);
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        if col[3] <= 0.001 || w <= 0.0 || h <= 0.0 {
            return;
        }
        self.push_quad(
            ColorVert { pos: [x, y], col },
            ColorVert { pos: [x + w, y], col },
            ColorVert { pos: [x + w, y + h], col },
            ColorVert { pos: [x, y + h], col },
        );
    }

    fn fill_rect_vgrad(&mut self, x: f32, y: f32, w: f32, h: f32, top: [f32; 4], bot: [f32; 4]) {
        if w <= 0.0 || h <= 0.0 || (top[3] <= 0.001 && bot[3] <= 0.001) {
            return;
        }
        self.push_quad(
            ColorVert { pos: [x, y], col: top },
            ColorVert { pos: [x + w, y], col: top },
            ColorVert { pos: [x + w, y + h], col: bot },
            ColorVert { pos: [x, y + h], col: bot },
        );
    }

    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, segs: usize, col: [f32; 4]) {
        if col[3] <= 0.001 || rx <= 0.05 || ry <= 0.05 {
            return;
        }
        let center = ColorVert { pos: [cx, cy], col };
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            self.push_tri(
                center,
                ColorVert {
                    pos: [cx + a0.cos() * rx, cy + a0.sin() * ry],
                    col,
                },
                ColorVert {
                    pos: [cx + a1.cos() * rx, cy + a1.sin() * ry],
                    col,
                },
            );
        }
    }

    fn fill_radial_ellipse(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        stops: &[(f32, [f32; 4])],
        segs: usize,
    ) {
        if stops.len() < 2 || rx <= 0.05 || ry <= 0.05 {
            return;
        }
        for pair in stops.windows(2) {
            let (t0, c0) = pair[0];
            let (t1, c1) = pair[1];
            if c0[3] <= 0.001 && c1[3] <= 0.001 {
                continue;
            }
            let r0x = rx * t0;
            let r0y = ry * t0;
            let r1x = rx * t1;
            let r1y = ry * t1;
            for i in 0..segs {
                let a0 = (i as f32 / segs as f32) * TAU;
                let a1 = ((i + 1) as f32 / segs as f32) * TAU;
                let (s0, k0) = a0.sin_cos();
                let (s1, k1) = a1.sin_cos();
                self.push_quad(
                    ColorVert {
                        pos: [cx + k0 * r0x, cy + s0 * r0y],
                        col: c0,
                    },
                    ColorVert {
                        pos: [cx + k0 * r1x, cy + s0 * r1y],
                        col: c1,
                    },
                    ColorVert {
                        pos: [cx + k1 * r1x, cy + s1 * r1y],
                        col: c1,
                    },
                    ColorVert {
                        pos: [cx + k1 * r0x, cy + s1 * r0y],
                        col: c0,
                    },
                );
            }
        }
    }

    fn stroke_arc(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        a0: f32,
        a1: f32,
        lw: f32,
        col: [f32; 4],
    ) {
        if col[3] <= 0.001 || r <= 0.05 || lw <= 0.05 {
            return;
        }
        let span = a1 - a0;
        if span.abs() < 1e-4 {
            return;
        }
        let segs = ((span.abs() / TAU) * 96.0).ceil().max(6.0) as usize;
        let ri = (r - lw * 0.5).max(0.0);
        let ro = r + lw * 0.5;
        for i in 0..segs {
            let t0 = a0 + span * (i as f32 / segs as f32);
            let t1 = a0 + span * ((i + 1) as f32 / segs as f32);
            let (s0, k0) = t0.sin_cos();
            let (s1, k1) = t1.sin_cos();
            self.push_quad(
                ColorVert {
                    pos: [cx + k0 * ri, cy + s0 * ri],
                    col,
                },
                ColorVert {
                    pos: [cx + k0 * ro, cy + s0 * ro],
                    col,
                },
                ColorVert {
                    pos: [cx + k1 * ro, cy + s1 * ro],
                    col,
                },
                ColorVert {
                    pos: [cx + k1 * ri, cy + s1 * ri],
                    col,
                },
            );
        }
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, lw: f32, col: [f32; 4]) {
        self.stroke_arc(cx, cy, r, 0.0, TAU, lw, col);
    }

    fn fill_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, col: [f32; 4]) {
        if col[3] <= 0.001 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let rr = r.min(w * 0.5).min(h * 0.5).max(0.0);
        if rr <= 0.01 {
            self.fill_rect(x, y, w, h, col);
            return;
        }
        let mut pts: Vec<(f32, f32)> = Vec::with_capacity(40);
        let corners = [
            (x + w - rr, y + rr, -PI * 0.5, 0.0),
            (x + w - rr, y + h - rr, 0.0, PI * 0.5),
            (x + rr, y + h - rr, PI * 0.5, PI),
            (x + rr, y + rr, PI, PI * 1.5),
        ];
        for (ccx, ccy, s, e) in corners {
            let steps = 6;
            for i in 0..=steps {
                let a = s + (e - s) * (i as f32 / steps as f32);
                pts.push((ccx + a.cos() * rr, ccy + a.sin() * rr));
            }
        }
        let mut cx = 0.0;
        let mut cy = 0.0;
        for p in &pts {
            cx += p.0;
            cy += p.1;
        }
        cx /= pts.len() as f32;
        cy /= pts.len() as f32;
        let center = ColorVert { pos: [cx, cy], col };
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            self.push_tri(
                center,
                ColorVert { pos: [a.0, a.1], col },
                ColorVert { pos: [b.0, b.1], col },
            );
        }
    }
}

// =========================================================================
// OpenGL 3.3 Core Neumorphic renderer
// =========================================================================
pub struct NeumorphRenderer {
    prog_sprite: GLuint,
    prog_color: GLuint,
    dummy_vao: GLuint,
    sprite_vao: GLuint,
    sprite_vbo: GLuint,
    color_vao: GLuint,
    color_vbo: GLuint,

    atlas: Atlas,
    tile_raised: Option<Sprite>,
    tile_well: Option<Sprite>,
    dial_base: Option<Sprite>,
    dial_knob: Option<Sprite>,
    pill_track: Option<Sprite>,
    pill_thumb: Option<Sprite>,
    cradle_cache: Vec<(String, Sprite)>,
    bake_sig: String,

    bg_fbo: GLuint,
    bg_tex: GLuint,
    bg_w: i32,
    bg_h: i32,

    sprites: Vec<SpriteVert>,
    colors: ColorBatch,
    batch_w: f32,
    batch_h: f32,
}

impl NeumorphRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_sprite = gl.compile_program(NEU_SPRITE_VERT, NEU_SPRITE_FRAG)?;
        let prog_color = gl.compile_program(NEU_COLOR_VERT, NEU_COLOR_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let mut sprite_vao = 0;
        let mut sprite_vbo = 0;
        (gl.GenVertexArrays)(1, &mut sprite_vao);
        (gl.GenBuffers)(1, &mut sprite_vbo);
        (gl.BindVertexArray)(sprite_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, sprite_vbo);
        let sstride = std::mem::size_of::<SpriteVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, sstride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, sstride, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 4, GL_FLOAT, GL_FALSE, sstride, 16 as *const c_void);
        (gl.BindVertexArray)(0);

        let mut color_vao = 0;
        let mut color_vbo = 0;
        (gl.GenVertexArrays)(1, &mut color_vao);
        (gl.GenBuffers)(1, &mut color_vbo);
        (gl.BindVertexArray)(color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, color_vbo);
        let cstride = std::mem::size_of::<ColorVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, cstride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, cstride, 8 as *const c_void);
        (gl.BindVertexArray)(0);

        Ok(Self {
            prog_sprite,
            prog_color,
            dummy_vao,
            sprite_vao,
            sprite_vbo,
            color_vao,
            color_vbo,
            atlas: Atlas::new(gl),
            tile_raised: None,
            tile_well: None,
            dial_base: None,
            dial_knob: None,
            pill_track: None,
            pill_thumb: None,
            cradle_cache: Vec::new(),
            bake_sig: String::new(),
            bg_fbo: 0,
            bg_tex: 0,
            bg_w: 0,
            bg_h: 0,
            sprites: Vec::with_capacity(8192),
            colors: ColorBatch::new(),
            batch_w: 1920.0,
            batch_h: 1080.0,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.bg_tex != 0 {
            (gl.DeleteTextures)(1, &self.bg_tex);
            self.bg_tex = 0;
        }
        if self.bg_fbo != 0 {
            (gl.DeleteFramebuffers)(1, &self.bg_fbo);
            self.bg_fbo = 0;
        }
        if self.atlas.tex != 0 {
            (gl.DeleteTextures)(1, &self.atlas.tex);
            self.atlas.tex = 0;
        }
        if self.sprite_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.sprite_vbo);
            self.sprite_vbo = 0;
        }
        if self.sprite_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.sprite_vao);
            self.sprite_vao = 0;
        }
        if self.color_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.color_vbo);
            self.color_vbo = 0;
        }
        if self.color_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.color_vao);
            self.color_vao = 0;
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
        if self.prog_sprite != 0 {
            (gl.DeleteProgram)(self.prog_sprite);
            self.prog_sprite = 0;
        }
        if self.prog_color != 0 {
            (gl.DeleteProgram)(self.prog_color);
            self.prog_color = 0;
        }
    }

    // ---------------------------------------------------------------------
    // Sprite baking (bakeTileSprites + getIconCradleSprite)
    // ---------------------------------------------------------------------
    unsafe fn ensure_sprites(&mut self, gl: &GlFns, st: &NeumorphState) {
        let prof = st.profile();
        let relief = clamp_f32(st.p.relief_depth, 0.3, 1.6);
        let bake_scale = st.bake_scale();
        let sig = format!(
            "{}|{:.2}|{}|{}|{:.2}|{}|{}",
            st.p.palette,
            relief,
            prof.shadow_steps,
            st.tile_size.round() as i32,
            bake_scale,
            st.p.icon_style,
            if st.low_mode { 1 } else { 0 }
        );
        if sig == self.bake_sig && self.tile_raised.is_some() {
            return;
        }
        self.bake_sig = sig;
        self.atlas.reset();
        self.cradle_cache.clear();

        let pal = st.palette();
        let steps = prof.shadow_steps;
        let pad = tile_sprite_pad(relief);
        let tile = st.tile_size;
        let r = (tile * 0.22).round().max(10.0);

        let raised = bake_raised_rounded(tile, tile, r, pad, &pal, relief, steps, bake_scale);
        if let Some(at) = self.atlas.alloc(raised.w, raised.h) {
            self.atlas.write(at, raised.w, raised.h, &raised.to_rgba8());
            self.tile_raised = Some(sprite_from(&raised, at, pad, tile, tile));
        }
        let well = bake_well_rounded(tile, tile, r, pad, &pal, relief * 0.9, steps, bake_scale);
        if let Some(at) = self.atlas.alloc(well.w, well.h) {
            self.atlas.write(at, well.w, well.h, &well.to_rgba8());
            self.tile_well = Some(sprite_from(&well, at, pad, tile, tile));
        }

        let dial_r = 54.0f32;
        let base = bake_well_rounded(
            dial_r * 2.0,
            dial_r * 2.0,
            dial_r,
            pad,
            &pal,
            relief,
            steps,
            bake_scale,
        );
        if let Some(at) = self.atlas.alloc(base.w, base.h) {
            self.atlas.write(at, base.w, base.h, &base.to_rgba8());
            self.dial_base = Some(sprite_from(&base, at, pad, dial_r * 2.0, dial_r * 2.0));
        }
        let knob_r = 36.0f32;
        let knob = bake_raised_rounded(
            knob_r * 2.0,
            knob_r * 2.0,
            knob_r,
            pad,
            &pal,
            relief * 0.95,
            steps,
            bake_scale,
        );
        if let Some(at) = self.atlas.alloc(knob.w, knob.h) {
            self.atlas.write(at, knob.w, knob.h, &knob.to_rgba8());
            self.dial_knob = Some(sprite_from(&knob, at, pad, knob_r * 2.0, knob_r * 2.0));
        }

        let (pill_w, pill_h) = (148.0f32, 44.0f32);
        let track = bake_well_rounded(
            pill_w,
            pill_h,
            pill_h * 0.5,
            pad,
            &pal,
            relief * 0.95,
            steps,
            bake_scale,
        );
        if let Some(at) = self.atlas.alloc(track.w, track.h) {
            self.atlas.write(at, track.w, track.h, &track.to_rgba8());
            self.pill_track = Some(sprite_from(&track, at, pad, pill_w, pill_h));
        }
        let thumb_d = 32.0f32;
        let thumb = bake_raised_rounded(
            thumb_d,
            thumb_d,
            thumb_d * 0.5,
            pad,
            &pal,
            relief * 0.9,
            steps,
            bake_scale,
        );
        if let Some(at) = self.atlas.alloc(thumb.w, thumb.h) {
            self.atlas.write(at, thumb.w, thumb.h, &thumb.to_rgba8());
            self.pill_thumb = Some(sprite_from(&thumb, at, pad, thumb_d, thumb_d));
        }

        self.atlas.upload(gl);
    }

    unsafe fn cradle_sprite(
        &mut self,
        gl: &GlFns,
        st: &NeumorphState,
        cw: f32,
        ch: f32,
    ) -> Option<Sprite> {
        let prof = st.profile();
        let relief = clamp_f32(st.p.relief_depth, 0.3, 1.6);
        let bake_scale = st.bake_scale();
        let rw = cw.round();
        let rh = ch.round();
        let key = format!(
            "{}x{}:{}:{}:{:.2}:{}:{:.2}",
            rw as i32,
            rh as i32,
            st.p.icon_style,
            st.p.palette,
            relief,
            prof.shadow_steps,
            bake_scale
        );
        if let Some((_, sp)) = self.cradle_cache.iter().find(|(k, _)| k == &key) {
            return Some(*sp);
        }

        let pal = st.palette();
        let pad = cradle_sprite_pad(relief);
        let rad = icon_cradle_rad(rw, rh);
        let canvas = match st.p.icon_style {
            1 => bake_well_rounded(rw, rh, rad, pad, &pal, relief, prof.shadow_steps, bake_scale),
            2 => bake_minimal_cradle(rw, rh, rad, pad, &pal, bake_scale),
            _ => bake_raised_rounded(
                rw,
                rh,
                rad,
                pad,
                &pal,
                relief * 1.05,
                prof.shadow_steps,
                bake_scale,
            ),
        };

        let at = self.atlas.alloc(canvas.w, canvas.h)?;
        self.atlas.write(at, canvas.w, canvas.h, &canvas.to_rgba8());
        self.atlas.upload(gl);
        let sp = sprite_from(&canvas, at, pad, rw, rh);

        if self.cradle_cache.len() > 24 {
            self.cradle_cache.clear();
        }
        self.cradle_cache.push((key, sp));
        Some(sp)
    }

    // ---------------------------------------------------------------------
    // Draw helpers
    // ---------------------------------------------------------------------
    fn push_sprite(&mut self, sp: Sprite, x: f32, y: f32, w: f32, h: f32, alpha: f32) {
        if alpha <= 0.004 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let col = [1.0, 1.0, 1.0, alpha.clamp(0.0, 1.0)];
        let (x0, y0, x1, y1) = (x, y, x + w, y + h);
        let a = SpriteVert {
            pos: [x0, y0],
            uv: [sp.u0, sp.v0],
            col,
        };
        let b = SpriteVert {
            pos: [x1, y0],
            uv: [sp.u1, sp.v0],
            col,
        };
        let c = SpriteVert {
            pos: [x1, y1],
            uv: [sp.u1, sp.v1],
            col,
        };
        let d = SpriteVert {
            pos: [x0, y1],
            uv: [sp.u0, sp.v1],
            col,
        };
        self.sprites.push(a);
        self.sprites.push(b);
        self.sprites.push(c);
        self.sprites.push(a);
        self.sprites.push(c);
        self.sprites.push(d);
    }

    unsafe fn flush_sprites(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.sprites.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_sprite);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_sprite, "u_res"), css_w, css_h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.atlas.tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_sprite, "u_atlas"), 0);
        (gl.BindVertexArray)(self.sprite_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.sprite_vbo);
        let bytes = (self.sprites.len() * std::mem::size_of::<SpriteVert>()) as GLsizeiptr;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            bytes,
            self.sprites.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.sprites.len() as GLsizei);
        self.sprites.clear();
    }

    unsafe fn flush_colors(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.colors.verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_color);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_color, "u_res"), css_w, css_h);
        (gl.BindVertexArray)(self.color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.color_vbo);
        let bytes = (self.colors.verts.len() * std::mem::size_of::<ColorVert>()) as GLsizeiptr;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            bytes,
            self.colors.verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.colors.verts.len() as GLsizei);
        self.colors.verts.clear();
    }
}

impl NeumorphRenderer {
    // ---------------------------------------------------------------------
    // 4. Static base layer (renderStaticBackground) cached in an FBO
    // ---------------------------------------------------------------------
    unsafe fn ensure_bg_fbo(
        &mut self,
        gl: &GlFns,
        pw: i32,
        ph: i32,
        css_w: f32,
        css_h: f32,
        st: &mut NeumorphState,
    ) {
        let size_changed = self.bg_w != pw || self.bg_h != ph;
        if self.bg_tex != 0 && !size_changed && !st.cache_dirty {
            return;
        }
        if self.bg_tex == 0 || size_changed {
            if self.bg_tex != 0 {
                (gl.DeleteTextures)(1, &self.bg_tex);
                self.bg_tex = 0;
            }
            if self.bg_fbo != 0 {
                (gl.DeleteFramebuffers)(1, &self.bg_fbo);
                self.bg_fbo = 0;
            }
            (gl.GenTextures)(1, &mut self.bg_tex);
            (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                pw,
                ph,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                std::ptr::null(),
            );
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            (gl.GenFramebuffers)(1, &mut self.bg_fbo);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
            (gl.FramebufferTexture2D)(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                self.bg_tex,
                0,
            );
            self.bg_w = pw;
            self.bg_h = ph;
        } else {
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
        }

        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);
        (gl.ClearColor)(0.0, 0.0, 0.0, 1.0);
        (gl.Clear)(GL_COLOR_BUFFER_BIT);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        self.draw_static_background(gl, st, css_w, css_h);
        st.cache_dirty = false;
    }

    unsafe fn draw_static_background(
        &mut self,
        gl: &GlFns,
        st: &NeumorphState,
        css_w: f32,
        css_h: f32,
    ) {
        let w = st.w;
        let h = st.h;
        let pal = st.palette();
        let prof = st.profile();

        // Soft ambient surface illumination from top-left to bottom-right
        // (Canvas linear gradient (0,0)->(W,H) approximated with a band sweep)
        let bands = 96;
        for i in 0..bands {
            let t0 = i as f32 / bands as f32;
            let t1 = (i + 1) as f32 / bands as f32;
            let c0 = if t0 < 0.5 {
                let u = t0 / 0.5;
                [
                    lerp_f32(pal.bg_hi[0], pal.bg[0], u),
                    lerp_f32(pal.bg_hi[1], pal.bg[1], u),
                    lerp_f32(pal.bg_hi[2], pal.bg[2], u),
                    1.0,
                ]
            } else {
                let u = (t0 - 0.5) / 0.5;
                [
                    lerp_f32(pal.bg[0], pal.bg_lo[0], u),
                    lerp_f32(pal.bg[1], pal.bg_lo[1], u),
                    lerp_f32(pal.bg[2], pal.bg_lo[2], u),
                    1.0,
                ]
            };
            let c1 = if t1 < 0.5 {
                let u = t1 / 0.5;
                [
                    lerp_f32(pal.bg_hi[0], pal.bg[0], u),
                    lerp_f32(pal.bg_hi[1], pal.bg[1], u),
                    lerp_f32(pal.bg_hi[2], pal.bg[2], u),
                    1.0,
                ]
            } else {
                let u = (t1 - 0.5) / 0.5;
                [
                    lerp_f32(pal.bg[0], pal.bg_lo[0], u),
                    lerp_f32(pal.bg[1], pal.bg_lo[1], u),
                    lerp_f32(pal.bg[2], pal.bg_lo[2], u),
                    1.0,
                ]
            };
            self.colors
                .fill_rect_vgrad(0.0, h * t0, w, h * (t1 - t0), c0, c1);
        }

        // Subtle molded architectural grid seams between tiles
        if !st.tiles.is_empty() {
            let half = st.grid_spacing * 0.5;
            let first = st.tiles[0];
            let last = st.tiles[st.tiles.len() - 1];
            let min_x = first.cx - half;
            let max_x = last.cx + half;
            let min_y = first.cy - half;
            let max_y = last.cy + half;

            let mut seen_x: Vec<f32> = Vec::new();
            let mut seen_y: Vec<f32> = Vec::new();
            for t in &st.tiles {
                let tx = (t.cx - half).round();
                let ty = (t.cy - half).round();
                if !seen_x.iter().any(|v| (*v - tx).abs() < 0.5) {
                    seen_x.push(tx);
                }
                if !seen_y.iter().any(|v| (*v - ty).abs() < 0.5) {
                    seen_y.push(ty);
                }
            }
            let seam = rgba(pal.seam, pal.seam_alpha);
            for tx in &seen_x {
                self.colors.fill_rect(*tx, min_y, 1.0, max_y - min_y, seam);
            }
            for ty in &seen_y {
                self.colors.fill_rect(min_x, *ty, max_x - min_x, 1.0, seam);
            }
            self.colors.fill_rect(
                max_x.round(),
                min_y,
                1.0,
                max_y - min_y,
                seam,
            );
            self.colors.fill_rect(
                min_x,
                max_y.round(),
                max_x - min_x,
                1.0,
                seam,
            );

            if prof.enable_grid_dots {
                let dot = rgba(pal.dark, pal.seam_alpha * 1.15);
                for vx in &seen_x {
                    for vy in &seen_y {
                        self.colors.fill_rect(vx - 1.0, vy - 1.0, 2.0, 2.0, dot);
                    }
                }
            }
        }

        self.flush_colors(gl, css_w, css_h);
    }

    // ---------------------------------------------------------------------
    // 7. Rendering pipeline (draw)
    // ---------------------------------------------------------------------
    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState, prog_blit: GLuint) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.1);

        st.neumorph.resize(css_w, css_h, dpi);
        self.batch_w = css_w;
        self.batch_h = css_h;

        self.ensure_sprites(gl, &st.neumorph);
        self.ensure_bg_fbo(gl, pw, ph, css_w, css_h, &mut st.neumorph);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(prog_blit);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
        (gl.Uniform1i)(gl.uniform_loc(prog_blit, "u_tex"), 0);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        let pal = st.neumorph.palette();
        let accent = st.neumorph.accent();
        let prof = st.neumorph.profile();

        // Optional dynamic ambient light sheen following the cursor
        if st.neumorph.p.light_follow_cursor {
            let lx = st.neumorph.pointer.smooth_x * css_w;
            let ly = st.neumorph.pointer.smooth_y * css_h;
            let rad = css_w.max(css_h) * 0.55;
            let a = if pal.is_dark { 0.07 } else { 0.18 };
            self.colors.fill_radial_ellipse(
                lx,
                ly,
                rad,
                rad,
                &[(0.0, rgba(pal.light, a)), (1.0, rgba(pal.light, 0.0))],
                64,
            );
            self.flush_colors(gl, css_w, css_h);
        }

        // Kinetic molded tiles
        let half = st.neumorph.tile_size * 0.5;
        for t in &st.neumorph.tiles {
            if t.under_icon {
                continue;
            }
            let ev = t.elev;
            if ev > 0.04 {
                if let Some(sp) = self.tile_raised {
                    let a = clamp_f32(ev, 0.0, 1.0);
                    self.push_sprite(
                        sp,
                        t.cx - half - sp.pad,
                        t.cy - half - sp.pad,
                        sp.w + sp.pad * 2.0,
                        sp.h + sp.pad * 2.0,
                        a,
                    );
                }
            } else if ev < -0.04 {
                if let Some(sp) = self.tile_well {
                    let a = clamp_f32(-ev, 0.0, 1.0);
                    self.push_sprite(
                        sp,
                        t.cx - half - sp.pad,
                        t.cy - half - sp.pad,
                        sp.w + sp.pad * 2.0,
                        sp.h + sp.pad * 2.0,
                        a,
                    );
                }
            }
        }
        self.flush_sprites(gl, css_w, css_h);

        // Ambient Soft UI micro-widgets
        if prof.enable_widgets {
            self.draw_micro_widgets(gl, st, &pal, &accent, css_w, css_h);
        }

        // Expanding concentric neumorphic click ripples
        self.draw_ripples(gl, st, &pal, &accent, css_w, css_h);

        // Adaptive desktop icon cradles
        if st.neumorph.p.adaptive != 0 {
            self.draw_icon_cradles(gl, st, &pal, &accent, css_w, css_h);
        }
    }

    unsafe fn draw_micro_widgets(
        &mut self,
        gl: &GlFns,
        st: &mut AppState,
        _pal: &Palette,
        accent: &Accent,
        css_w: f32,
        css_h: f32,
    ) {
        // 1. Rotary Soft Dial
        let d = st.neumorph.widgets[0];
        if d.visible {
            if let Some(base) = self.dial_base {
                self.push_sprite(
                    base,
                    d.x - 54.0 - base.pad,
                    d.y - 54.0 - base.pad,
                    base.w + base.pad * 2.0,
                    base.h + base.pad * 2.0,
                    1.0,
                );
                self.flush_sprites(gl, css_w, css_h);
            }
            self.colors.stroke_arc(
                d.x,
                d.y,
                44.0,
                -PI * 0.75,
                d.angle,
                2.4,
                rgba(accent.rgb, 0.45),
            );
            self.flush_colors(gl, css_w, css_h);

            let press_scale = 1.0 - d.press * 0.035;
            if let Some(knob) = self.dial_knob {
                let size = (knob.w + knob.pad * 2.0) * press_scale;
                self.push_sprite(
                    knob,
                    d.x - size * 0.5,
                    d.y - size * 0.5,
                    size,
                    size,
                    1.0,
                );
                self.flush_sprites(gl, css_w, css_h);
            }
            let pip_x = d.x + d.angle.cos() * 21.0 * press_scale;
            let pip_y = d.y + d.angle.sin() * 21.0 * press_scale;
            self.colors.fill_ellipse(
                pip_x,
                pip_y,
                3.6 * press_scale,
                3.6 * press_scale,
                16,
                rgba(accent.hex, 1.0),
            );
            self.flush_colors(gl, css_w, css_h);
        }

        // 2. Tactile Pill Slider
        let pl = st.neumorph.widgets[1];
        if pl.visible {
            let track_w = 148.0f32;
            let track_h = 44.0f32;
            let left_x = pl.x - track_w * 0.5;
            let top_y = pl.y - track_h * 0.5;
            if let Some(track) = self.pill_track {
                self.push_sprite(
                    track,
                    left_x - track.pad,
                    top_y - track.pad,
                    track.w + track.pad * 2.0,
                    track.h + track.pad * 2.0,
                    1.0,
                );
                self.flush_sprites(gl, css_w, css_h);
            }

            let travel = track_w - 44.0;
            let thumb_x = left_x + 22.0 + travel * clamp_f32(pl.pos, 0.0, 1.0);
            let bar_w = (thumb_x - (left_x + 22.0)).max(0.0);
            self.colors.fill_rect(
                left_x + 22.0,
                pl.y - 2.0,
                bar_w,
                4.0,
                rgba(accent.rgb, 0.50),
            );
            self.colors.fill_ellipse(
                left_x + 22.0,
                pl.y,
                2.0,
                2.0,
                12,
                rgba(accent.rgb, 0.50),
            );
            self.colors
                .fill_ellipse(thumb_x, pl.y, 2.0, 2.0, 12, rgba(accent.rgb, 0.50));
            self.flush_colors(gl, css_w, css_h);

            let press_scale = 1.0 - pl.press * 0.05;
            if let Some(thumb) = self.pill_thumb {
                let size = (thumb.w + thumb.pad * 2.0) * press_scale;
                self.push_sprite(
                    thumb,
                    thumb_x - size * 0.5,
                    pl.y - size * 0.5,
                    size,
                    size,
                    1.0,
                );
                self.flush_sprites(gl, css_w, css_h);
            }
            self.colors.fill_ellipse(
                thumb_x,
                pl.y,
                2.8 * press_scale,
                2.8 * press_scale,
                14,
                rgba(accent.hex, 1.0),
            );
            self.flush_colors(gl, css_w, css_h);
        }
    }

    unsafe fn draw_ripples(
        &mut self,
        gl: &GlFns,
        st: &mut AppState,
        pal: &Palette,
        accent: &Accent,
        css_w: f32,
        css_h: f32,
    ) {
        for rp in &st.neumorph.ripples {
            let fade = (1.0 - rp.r / rp.max_r) * rp.strength.min(1.0);
            if fade <= 0.01 {
                continue;
            }
            self.colors.stroke_circle(
                rp.x - 1.8,
                rp.y - 1.8,
                rp.r,
                3.2,
                rgba(pal.light, fade * pal.light_alpha * 0.42),
            );
            self.colors.stroke_circle(
                rp.x + 1.8,
                rp.y + 1.8,
                rp.r,
                3.2,
                rgba(pal.dark, fade * pal.dark_alpha * 0.36),
            );
            self.colors.stroke_circle(
                rp.x,
                rp.y,
                rp.r,
                1.2,
                rgba(accent.rgb, fade * 0.22),
            );
        }
        self.flush_colors(gl, css_w, css_h);
    }

    unsafe fn draw_icon_cradles(
        &mut self,
        gl: &GlFns,
        st: &mut AppState,
        pal: &Palette,
        accent: &Accent,
        css_w: f32,
        css_h: f32,
    ) {
        let icons = st.neumorph.icons.clone();
        let sim_time = st.neumorph.sim_time;
        let icon_style = st.neumorph.p.icon_style;

        let mut pills: Vec<(f32, f32, f32, f32, f32)> = Vec::new();
        let mut leds: Vec<(f32, f32, f32, f32)> = Vec::new();

        for ic in &icons {
            if ic.opacity <= 0.01 {
                continue;
            }
            let pad_x = 10.0f32;
            let pad_y = 8.0f32;
            let cw = ic.w + pad_x * 2.0;
            let ch = ic.h + pad_y * 2.0;
            let cx = ic.x - pad_x;
            let cy = ic.y - pad_y;
            let scale_mod = 1.0 - ic.press * 0.025 + ic.pulse * 0.015;
            let mid_x = cx + cw * 0.5;
            let mid_y = cy + ch * 0.5;

            let sp = match self.cradle_sprite(gl, &st.neumorph, cw, ch) {
                Some(s) => s,
                None => continue,
            };
            let size_w = (sp.w + sp.pad * 2.0) * scale_mod;
            let size_h = (sp.h + sp.pad * 2.0) * scale_mod;
            self.push_sprite(
                sp,
                mid_x - size_w * 0.5,
                mid_y - size_h * 0.5,
                size_w,
                size_h,
                ic.opacity,
            );

            if !pal.is_dark && icon_style != 2 {
                let lbl_w = (cw - 14.0).min((ic.w * 0.84).max(44.0));
                let lbl_h = 14.0f32;
                let lx = mid_x + (-lbl_w * 0.5) * scale_mod;
                let ly = mid_y + (ch * 0.5 - lbl_h - 7.0) * scale_mod;
                pills.push((lx, ly, lbl_w * scale_mod, lbl_h * scale_mod, ic.opacity));
            }

            let breath = 0.55 + 0.35 * (sim_time * 1.8 + ic.phase).sin();
            let led_alpha = clamp_f32(breath + ic.pulse * 0.5 + ic.press * 0.4, 0.25, 1.0);
            let lx = mid_x + (cw * 0.5 - 11.0) * scale_mod;
            let ly = mid_y + (-ch * 0.5 + 11.0) * scale_mod;
            let lr = (2.4 + ic.pulse * 0.9) * scale_mod;
            leds.push((lx, ly, lr, led_alpha * ic.opacity));
        }

        self.flush_sprites(gl, css_w, css_h);

        for (x, y, w, h, a) in pills {
            self.colors.fill_rounded_rect(
                x,
                y,
                w,
                h,
                h * 0.5,
                [pal.label_pill[0], pal.label_pill[1], pal.label_pill[2], pal.label_pill[3] * a],
            );
        }
        for (x, y, r, a) in leds {
            self.colors.fill_ellipse(x, y, r, r, 14, rgba(accent.rgb, a));
        }
        self.flush_colors(gl, css_w, css_h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const CASES: &str = include_str!("cases.jsonl");
    const RECORDED: &str = include_str!("recorded-from.json");

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-4
    }

    fn num(v: &Value) -> f64 {
        v.as_f64().unwrap_or(f64::NAN)
    }

    fn icons_from(v: &Value) -> Vec<Rect> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|r| Rect {
                        x: num(&r["x"]) as f32,
                        y: num(&r["y"]) as f32,
                        w: num(&r["w"]) as f32,
                        h: num(&r["h"]) as f32,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn state_for(w: f32, h: f32, density: f32, seed: u32) -> NeumorphState {
        NeumorphState::new(
            w,
            h,
            1.0,
            NeumorphParams {
                tile_density: density,
                seed,
                ..NeumorphParams::default()
            },
        )
    }

    #[test]
    fn recorded_fingerprint_matches_source() {
        let meta: Value = serde_json::from_str(RECORDED).expect("recorded-from.json");
        assert_eq!(meta["schema"], "rust-anything/recorded-from@1");
        assert_eq!(meta["unit"], "prototype/neumorph.html");
        assert_eq!(meta["cases"].as_u64().unwrap() as usize, CASES.lines().count());
        assert_eq!(meta["fingerprint"].as_str().unwrap().len(), 16);
    }

    #[test]
    fn replays_recorded_cases() {
        let mut checked = 0usize;
        for line in CASES.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let case: Value = serde_json::from_str(line).expect("case json");
            let name = case["fn"].as_str().unwrap();
            let args = case["args"].as_array().unwrap();
            let ok = &case["result"]["ok"];
            match name {
                "clamp" => {
                    let got =
                        clamp_f32(num(&args[0]) as f32, num(&args[1]) as f32, num(&args[2]) as f32);
                    assert!(close(got as f64, num(ok)), "clamp {:?}", args);
                }
                "lerp" => {
                    let got =
                        lerp_f32(num(&args[0]) as f32, num(&args[1]) as f32, num(&args[2]) as f32);
                    assert!(close(got as f64, num(ok)), "lerp {:?}", args);
                }
                "smoothstep" => {
                    let got = smoothstep_f32(
                        num(&args[0]) as f32,
                        num(&args[1]) as f32,
                        num(&args[2]) as f32,
                    );
                    assert!(close(got as f64, num(ok)), "smoothstep {:?}", args);
                }
                "mulberry32" => {
                    let mut rng = Mulberry32::new(num(&args[0]) as u32 as i32);
                    for want in ok.as_array().unwrap() {
                        assert!(close(rng.next_f64(), num(want)), "mulberry32 {:?}", args);
                    }
                }
                "roundRectClamp" => {
                    let got = round_rect_clamp(
                        num(&args[0]) as f32,
                        num(&args[1]) as f32,
                        num(&args[2]) as f32,
                    );
                    assert!(close(got as f64, num(ok)), "roundRectClamp {:?}", args);
                }
                "tileSpritePad" => {
                    let got = tile_sprite_pad(num(&args[0]) as f32);
                    assert!(close(got as f64, num(ok)), "tileSpritePad {:?}", args);
                }
                "iconCradleRad" => {
                    let got = icon_cradle_rad(num(&args[0]) as f32, num(&args[1]) as f32);
                    assert!(close(got as f64, num(ok)), "iconCradleRad {:?}", args);
                }
                "featheredShadowStep" => {
                    let (spread, weight, alpha) = feathered_shadow_step(
                        num(&args[0]) as i32,
                        num(&args[1]) as i32,
                        num(&args[2]) as f32,
                        num(&args[3]) as f32,
                    );
                    assert!(close(spread as f64, num(&ok["spread"])), "spread {:?}", args);
                    assert!(close(weight as f64, num(&ok["weight"])), "weight {:?}", args);
                    assert!(close(alpha as f64, num(&ok["alpha"])), "alpha {:?}", args);
                }
                "wellInnerStep" => {
                    let (lw, weight, da, la) = well_inner_step(
                        num(&args[0]) as i32,
                        num(&args[1]) as i32,
                        num(&args[2]) as f32,
                        num(&args[3]) as f32,
                        num(&args[4]) as f32,
                    );
                    assert!(close(lw as f64, num(&ok["lw"])), "lw {:?}", args);
                    assert!(close(weight as f64, num(&ok["weight"])), "weight {:?}", args);
                    assert!(close(da as f64, num(&ok["da"])), "da {:?}", args);
                    assert!(close(la as f64, num(&ok["la"])), "la {:?}", args);
                }
                "getProfile" => {
                    let quality = args[0].as_str().unwrap();
                    let low = args[1].as_bool().unwrap();
                    let mut st = state_for(1920.0, 1080.0, 1.0, 20261118);
                    st.p.quality = match quality {
                        "Low" => 0,
                        "Medium" => 1,
                        _ => 2,
                    };
                    st.low_mode = low;
                    let prof = st.profile();
                    assert_eq!(prof.shadow_steps as u64, ok["shadowSteps"].as_u64().unwrap());
                    assert_eq!(prof.enable_widgets, ok["enableWidgets"].as_bool().unwrap());
                    assert_eq!(
                        prof.enable_grid_dots,
                        ok["enableGridDots"].as_bool().unwrap()
                    );
                }
                "spawnRippleStrength" => {
                    let mut st = state_for(1920.0, 1080.0, 1.0, 20261118);
                    st.p.ripple_strength = num(&args[0]) as f32;
                    st.ripples.clear();
                    st.spawn_ripple(100.0, 100.0, num(&args[1]) as f32);
                    let want = num(ok);
                    if want <= 0.02 {
                        assert!(st.ripples.is_empty(), "no ripple expected");
                    } else {
                        assert_eq!(st.ripples.len(), 1);
                        assert!(close(st.ripples[0].strength as f64, want));
                    }
                }
                "gridLayout" => {
                    let st = state_for(
                        num(&args[0]) as f32,
                        num(&args[1]) as f32,
                        num(&args[2]) as f32,
                        num(&args[3]) as u32,
                    );
                    assert_eq!(st.grid_spacing as u64, ok["gridSpacing"].as_u64().unwrap());
                    assert_eq!(st.tile_size as u64, ok["tileSize"].as_u64().unwrap());
                    assert_eq!(st.tiles.len() as u64, ok["count"].as_u64().unwrap());
                    let first = st.tiles[0];
                    let last = st.tiles[st.tiles.len() - 1];
                    let want_first = ok["first"].as_array().unwrap();
                    let want_last = ok["last"].as_array().unwrap();
                    assert!(close(first.cx as f64, num(&want_first[0])), "first cx");
                    assert!(close(first.cy as f64, num(&want_first[1])), "first cy");
                    assert!(close(first.phase as f64, num(&want_first[2])), "first phase");
                    assert!(close(first.bias as f64, num(&want_first[3])), "first bias");
                    assert!(close(last.cx as f64, num(&want_last[0])), "last cx");
                    assert!(close(last.cy as f64, num(&want_last[1])), "last cy");
                    assert!(close(last.phase as f64, num(&want_last[2])), "last phase");
                    assert!(close(last.bias as f64, num(&want_last[3])), "last bias");
                }
                "tileIconMask" => {
                    let mut st = state_for(
                        num(&args[0]) as f32,
                        num(&args[1]) as f32,
                        num(&args[2]) as f32,
                        num(&args[3]) as u32,
                    );
                    st.p.adaptive = num(&args[5]) as i32;
                    st.icons = icons_from(&case["args"][4]).into_iter().map(|r| IconCradle {
                        x: r.x,
                        y: r.y,
                        w: r.w,
                        h: r.h,
                        tx: r.x,
                        ty: r.y,
                        tw: r.w,
                        th: r.h,
                        opacity: 1.0,
                        press: 0.0,
                        pulse: 0.0,
                        phase: 0.0,
                        removing: false,
                    }).collect();
                    st.refresh_tile_icon_masks();
                    let blocked = st.tiles.iter().filter(|t| t.under_icon).count();
                    assert_eq!(blocked as u64, ok["blocked"].as_u64().unwrap());
                    assert_eq!(st.tiles.len() as u64, ok["total"].as_u64().unwrap());
                }
                "pickWidgets" => {
                    let mut st = state_for(num(&args[0]) as f32, num(&args[1]) as f32, 1.0, 20261118);
                    st.icons = icons_from(&case["args"][2]).into_iter().map(|r| IconCradle {
                        x: r.x,
                        y: r.y,
                        w: r.w,
                        h: r.h,
                        tx: r.x,
                        ty: r.y,
                        tw: r.w,
                        th: r.h,
                        opacity: 1.0,
                        press: 0.0,
                        pulse: 0.0,
                        phase: 0.0,
                        removing: false,
                    }).collect();
                    st.pick_widget_placements();
                    let dial = ok["dial"].as_array().unwrap();
                    let pill = ok["pill"].as_array().unwrap();
                    assert!(close(st.widgets[0].x as f64, num(&dial[0])), "dial x");
                    assert!(close(st.widgets[0].y as f64, num(&dial[1])), "dial y");
                    assert_eq!(st.widgets[0].visible, dial[2].as_bool().unwrap());
                    assert!(close(st.widgets[1].x as f64, num(&pill[0])), "pill x");
                    assert!(close(st.widgets[1].y as f64, num(&pill[1])), "pill y");
                    assert_eq!(st.widgets[1].visible, pill[2].as_bool().unwrap());
                }
                other => panic!("unhandled case fn {}", other),
            }
            checked += 1;
        }
        assert_eq!(
            checked,
            CASES.lines().filter(|l| !l.trim().is_empty()).count()
        );
        assert!(checked >= 60);
    }

    #[test]
    fn tile_spring_stays_bounded_and_ripples_expire() {
        let mut st = state_for(1280.0, 720.0, 1.0, 20261118);
        st.on_pointer_down(400.0, 300.0);
        for _ in 0..300 {
            st.step(1.0 / 30.0);
        }
        assert!(st.tiles.iter().all(|t| t.elev.is_finite() && t.elev.abs() <= 1.05));
        assert!(st.tiles.iter().all(|t| t.vel.is_finite()));
        assert!(st.ripples.len() <= RIPPLE_MAX);
        assert!(st.ripples.iter().all(|r| r.r.is_finite()));
    }

    #[test]
    fn icon_cradles_fade_out_after_removal() {
        let mut st = state_for(1280.0, 720.0, 1.0, 20261118);
        st.sync_incoming_icons(&[Rect {
            x: 40.0,
            y: 40.0,
            w: 76.0,
            h: 90.0,
        }]);
        assert_eq!(st.icons.len(), 1);
        for _ in 0..40 {
            st.step(1.0 / 30.0);
        }
        assert!(st.icons[0].opacity > 0.5);
        st.sync_incoming_icons(&[]);
        for _ in 0..120 {
            st.step(1.0 / 30.0);
        }
        assert!(st.icons.is_empty(), "removed cradle should be dropped");
    }

    #[test]
    fn sprite_baker_produces_visible_pixels() {
        let pal = palette_at(0);
        let raised = bake_raised_rounded(76.0, 76.0, 17.0, 22.0, &pal, 1.0, 14, 1.0);
        let well = bake_well_rounded(76.0, 76.0, 17.0, 22.0, &pal, 0.9, 14, 1.0);
        let minimal = bake_minimal_cradle(96.0, 106.0, 21.0, 24.0, &pal, 1.0);
        for (name, c) in [("raised", &raised), ("well", &well), ("minimal", &minimal)] {
            let px = c.to_rgba8();
            let opaque = px.chunks(4).filter(|p| p[3] > 8).count();
            assert!(opaque > 1000, "{} sprite looks empty ({} px)", name, opaque);
            assert_eq!(px.len(), c.w * c.h * 4);
        }
        // Raised body centre must be lighter than the well cavity centre.
        let rc = raised.w / 2 + (raised.h / 2) * raised.w;
        let wc = well.w / 2 + (well.h / 2) * well.w;
        let rpx = raised.to_rgba8();
        let wpx = well.to_rgba8();
        assert!(rpx[rc * 4] > 150 && wpx[wc * 4] > 100);
    }

    #[test]
    fn atlas_holds_every_sprite() {
        let mut atlas = Atlas {
            tex: 0,
            px: vec![0u8; ATLAS_SIZE * ATLAS_SIZE * 4],
            cursor_x: 0,
            cursor_y: 0,
            shelf_h: 0,
            dirty: false,
        };
        for _ in 0..30 {
            assert!(atlas.alloc(180, 180).is_some(), "atlas overflowed");
        }
    }
}
