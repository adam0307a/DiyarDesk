//! **ASCII Cat Knight (`ascii-cat-knight`)** — High-density photorealistic
//! Native Rust + OpenGL 3.3 wallpaper.
//!
//! Features a 288×168 max high-density ASCII grid (`charSize = 4.5 px`), soft
//! photographic `u_bg` underlay behind crisp woven ASCII letters, 2×2 area
//! supersampling, silhouette border glyphs (`/`, `\`, `|`, `-`), blinking and
//! mouse-tracking kitten eyes, sweeping sword glints, live 24×16 Verlet cloth
//! cape with continuous fold shading, and desktop-icon wind deflection.

pub mod cloth;
pub mod portrait;

use crate::ascii_lab::grid_gl::{AsciiCanvas, AsciiGridRenderer, glyph_id, ramp_glyph};
use crate::gl::GlFns;
use crate::state::{AppState, Rect};
use cloth::{CapeCloth, cape_wind_scalar};
use portrait::{
    HEAD_CX, HEAD_CY, REF_H, REF_W, SWORD_GUARD_Y, SWORD_TOP_Y, SWORD_X, portrait,
};
use serde_json::Value;
use std::collections::HashMap;

/// Delicate woven character progression matching the `@jin_22ee` photo-ASCII style:
/// smooth dot/punctuation shadows -> rich woven lowercase mid-tones (`crvtxr...`) ->
/// bold metallic specular highlights ONLY on brightest armor & sword glints.
const WOVEN_RAMP: &[u8] =
    b" .'`^,:;il!><~+=-?][}{)(|/tfjrxnuvczoae0O8%$#@";

const LOOP_SECONDS: f32 = 20.0;
const BLINK_STARTS: [f32; 3] = [7.0, 12.5, 17.0];

const MAX_COLS: usize = 288;
const MAX_ROWS: usize = 168;
const CELL_ASPECT: f32 = 1.48;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AsciiCatKnightParams {
    pub palette: usize,     // 0: original silver, 1: amber paladin, 2: moonlight sentinel
    pub camera_mode: usize, // 0: full knight, 1: cinematic zoom-out, 2: close portrait
    pub char_size: f32,
    pub glow: f32,
    pub wind: f32,
    pub speed: f32,
    pub mouse_interact: bool,
    pub adaptive: bool,
}

impl Default for AsciiCatKnightParams {
    fn default() -> Self {
        Self {
            palette: 0,
            camera_mode: 0,
            char_size: 4.5,
            glow: 0.48,
            wind: 1.0,
            speed: 1.0,
            mouse_interact: true,
            adaptive: true,
        }
    }
}

fn parse_enum(raw: &HashMap<String, Value>, key: &str, names: &[&str], def: usize) -> usize {
    let Some(v) = raw.get(key) else {
        return def;
    };
    match v {
        Value::String(s) => {
            let l = s.trim().to_ascii_lowercase();
            for (i, n) in names.iter().enumerate() {
                if n.eq_ignore_ascii_case(&l) {
                    return i;
                }
            }
            if let Ok(idx) = l.parse::<usize>() {
                return idx.min(names.len() - 1);
            }
            def
        }
        Value::Number(n) => (n.as_i64().unwrap_or(def as i64).max(0) as usize).min(names.len() - 1),
        _ => def,
    }
}

fn parse_f32(raw: &HashMap<String, Value>, key: &str, def: f32, lo: f32, hi: f32) -> f32 {
    let Some(v) = raw.get(key) else {
        return def;
    };
    let n = match v {
        Value::Number(x) => x.as_f64().map(|f| f as f32),
        Value::String(s) => s.trim().parse::<f32>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    };
    match n {
        Some(x) if x.is_finite() => x.clamp(lo, hi),
        _ => def,
    }
}

fn parse_bool(raw: &HashMap<String, Value>, key: &str, def: bool) -> bool {
    let Some(v) = raw.get(key) else {
        return def;
    };
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => {
            let l = s.trim().to_ascii_lowercase();
            l == "true" || l == "1"
        }
        _ => def,
    }
}

impl AsciiCatKnightParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        Self {
            palette: parse_enum(raw, "palette", &["original", "amber", "moonlight"], 0) % 3,
            camera_mode: parse_enum(raw, "cameraMode", &["full", "cinematic", "portrait"], 0) % 3,
            char_size: parse_f32(raw, "charSize", 4.5, 3.5, 12.0),
            glow: parse_f32(raw, "glow", 0.48, 0.0, 1.5),
            wind: parse_f32(raw, "wind", 1.0, 0.2, 2.0),
            speed: parse_f32(raw, "speed", 1.0, 0.3, 2.0),
            mouse_interact: parse_bool(raw, "mouseInteract", true),
            adaptive: parse_bool(raw, "adaptive", true),
        }
    }
}

#[inline]
fn smoothstep01(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn clamp_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[inline]
fn cell_hash(c: usize, r: usize) -> f32 {
    let n = ((c as u32).wrapping_mul(374761393) ^ (r as u32).wrapping_mul(668265263))
        .wrapping_mul(1274126177);
    let m = (n ^ (n >> 13)) & 0xFFFF;
    (m as f32 / 65535.0) - 0.5
}

#[inline]
fn sample_bilinear_rgb(buf: &[[f32; 3]], x: f32, y: f32) -> [f32; 3] {
    if x < 0.0 || y < 0.0 || x >= (REF_W - 1) as f32 || y >= (REF_H - 1) as f32 {
        return [0.0, 0.0, 0.0];
    }
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(REF_W - 1);
    let y1 = (y0 + 1).min(REF_H - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let c00 = buf[y0 * REF_W + x0];
    let c10 = buf[y0 * REF_W + x1];
    let c01 = buf[y1 * REF_W + x0];
    let c11 = buf[y1 * REF_W + x1];
    let mut out = [0.0f32; 3];
    for ch in 0..3 {
        let top = c00[ch] + (c10[ch] - c00[ch]) * fx;
        let bot = c01[ch] + (c11[ch] - c01[ch]) * fx;
        out[ch] = top + (bot - top) * fy;
    }
    out
}

#[inline]
fn sample_bilinear_f32(buf: &[f32], x: f32, y: f32) -> f32 {
    if x < 0.0 || y < 0.0 || x >= (REF_W - 1) as f32 || y >= (REF_H - 1) as f32 {
        return 0.0;
    }
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(REF_W - 1);
    let y1 = (y0 + 1).min(REF_H - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let v00 = buf[y0 * REF_W + x0];
    let v10 = buf[y0 * REF_W + x1];
    let v01 = buf[y1 * REF_W + x0];
    let v11 = buf[y1 * REF_W + x1];
    let top = v00 + (v10 - v00) * fx;
    let bot = v01 + (v11 - v01) * fx;
    top + (bot - top) * fy
}

#[inline]
fn apply_palette_rgb(rgb: [f32; 3], palette: usize, is_sword_glint: f32) -> [f32; 3] {
    let gl = is_sword_glint.clamp(0.0, 1.0);
    let tinted = match palette {
        1 => [
            (rgb[0] * 1.12).clamp(0.0, 1.0),
            (rgb[1] * 0.96).clamp(0.0, 1.0),
            (rgb[2] * 0.72).clamp(0.0, 1.0),
        ],
        2 => [
            (rgb[0] * 0.84).clamp(0.0, 1.0),
            (rgb[1] * 0.94).clamp(0.0, 1.0),
            (rgb[2] * 1.14).clamp(0.0, 1.0),
        ],
        _ => rgb,
    };
    if gl > 0.01 {
        [
            (tinted[0] + gl * 0.65).clamp(0.0, 1.0),
            (tinted[1] + gl * 0.68).clamp(0.0, 1.0),
            (tinted[2] + gl * 0.72).clamp(0.0, 1.0),
        ]
    } else {
        tinted
    }
}

pub struct AsciiCatKnightState {
    pub canvas: AsciiCanvas,
    pub params: AsciiCatKnightParams,
    pub icons: Vec<Rect>,
    pub icons_vis: bool,
    pub anim_time: f32,
    pub cloth: CapeCloth,
    cape_lum_buf: Vec<f32>,
    cape_alpha_buf: Vec<f32>,
    icon_obstacles_ref: Vec<[f32; 4]>,
    low_mode: bool,
    css_w: f32,
    css_h: f32,
    dpi: f32,
    mouse_active: bool,
    mouse_x: f32,
    mouse_y: f32,
    gust_vx: f32,
    gust_vy: f32,
    gaze_x: f32,
    gaze_y: f32,
    click_glint_timer: f32,
    click_blink_timer: f32,
}

impl Default for AsciiCatKnightState {
    fn default() -> Self {
        Self::new(1280.0, 720.0, 1.0)
    }
}

impl AsciiCatKnightState {
    pub fn new(css_w: f32, css_h: f32, dpi: f32) -> Self {
        let mut canvas = AsciiCanvas::default();
        canvas.invalidate_layout();
        let mut s = Self {
            canvas,
            params: AsciiCatKnightParams::default(),
            icons: Vec::new(),
            icons_vis: false,
            anim_time: 0.0,
            cloth: CapeCloth::default(),
            cape_lum_buf: Vec::new(),
            cape_alpha_buf: Vec::new(),
            icon_obstacles_ref: Vec::new(),
            low_mode: false,
            css_w: css_w.max(120.0),
            css_h: css_h.max(90.0),
            dpi: dpi.max(0.5),
            mouse_active: false,
            mouse_x: css_w * 0.5,
            mouse_y: css_h * 0.5,
            gust_vx: 0.0,
            gust_vy: 0.0,
            gaze_x: 0.0,
            gaze_y: 0.0,
            click_glint_timer: 0.0,
            click_blink_timer: 0.0,
        };
        s.canvas.ensure_layout_custom(
            s.css_w,
            s.css_h,
            s.params.char_size,
            s.low_mode,
            MAX_COLS,
            MAX_ROWS,
            CELL_ASPECT,
        );
        s.rebuild_grid();
        s
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.canvas.ensure_layout_custom(
                self.css_w,
                self.css_h,
                self.params.char_size,
                self.low_mode,
                MAX_COLS,
                MAX_ROWS,
                CELL_ASPECT,
            );
            self.rebuild_grid();
        }
    }

    pub fn resize(&mut self, css_w: f32, css_h: f32, dpi: f32) {
        let w = css_w.max(120.0);
        let h = css_h.max(90.0);
        let d = dpi.max(0.5);
        if (w - self.css_w).abs() <= 0.5
            && (h - self.css_h).abs() <= 0.5
            && (d - self.dpi).abs() <= 0.01
        {
            return;
        }
        self.css_w = w;
        self.css_h = h;
        self.dpi = d;
        if self.canvas.ensure_layout_custom(
            self.css_w,
            self.css_h,
            self.params.char_size,
            self.low_mode,
            MAX_COLS,
            MAX_ROWS,
            CELL_ASPECT,
        ) {
            self.rebuild_grid();
        }
    }

    pub fn sync_params(&mut self, next: AsciiCatKnightParams) {
        if self.params != next {
            self.params = next;
            self.canvas.ensure_layout_custom(
                self.css_w,
                self.css_h,
                self.params.char_size,
                self.low_mode,
                MAX_COLS,
                MAX_ROWS,
                CELL_ASPECT,
            );
            self.rebuild_grid();
        }
    }

    pub fn set_icons(&mut self, icons: &[Rect], vis: bool) {
        self.icons_vis = vis;
        self.icons.clear();
        if vis {
            self.icons.extend(icons.iter().take(24).copied());
        }
        self.rebuild_grid();
    }

    fn camera_window(&self, t: f32) -> (f32, f32, f32, f32) {
        match self.params.camera_mode {
            1 => {
                // Cinematic intro zoom-out: kitten face -> full knight
                let lt = t.rem_euclid(LOOP_SECONDS);
                let k = smoothstep01((lt - 1.8) / 11.0);
                let k_pos = k * k * (3.0 - 2.0 * k);
                let cx = HEAD_CX * (1.0 - k_pos) + 365.0 * k_pos;
                let cy = HEAD_CY * (1.0 - k_pos) + 422.0 * k_pos;
                let view_h = 285.0 * (1.0 - k) + 740.0 * k;
                (cx, cy, view_h, k)
            }
            2 => {
                // Close portrait matching screenshot framing
                (276.0, 392.0, 326.0, 0.15)
            }
            _ => {
                // Full knight + upright sword + billowing cape centered cleanly
                (365.0, 422.0, 736.0, 1.0)
            }
        }
    }

    fn map_css_to_ref(&self, css_x: f32, css_y: f32) -> (f32, f32) {
        let cols = self.canvas.cols.max(1) as f32;
        let rows = self.canvas.rows.max(1) as f32;
        let (cx, cy, view_h, _) = self.camera_window(self.anim_time);
        let cell_ratio = (self.css_h / rows) / (self.css_w / cols);
        let view_w = view_h * (cols / rows) / cell_ratio.max(0.5);
        let u = (css_x / self.css_w.max(1.0)).clamp(0.0, 1.0);
        let v = (css_y / self.css_h.max(1.0)).clamp(0.0, 1.0);
        (cx + (u - 0.5) * view_w, cy + (v - 0.5) * view_h)
    }

    pub fn handle_mouse(&mut self, kind: i32, css_x: f32, css_y: f32) -> bool {
        if kind == 3 {
            self.mouse_active = false;
            return false;
        }
        if !self.params.mouse_interact {
            self.mouse_active = false;
            return false;
        }
        let prev_x = self.mouse_x;
        let prev_y = self.mouse_y;
        self.mouse_x = css_x;
        self.mouse_y = css_y;

        if kind == 0 {
            if self.mouse_active {
                let dx = (css_x - prev_x).clamp(-120.0, 120.0);
                let dy = (css_y - prev_y).clamp(-120.0, 120.0);
                self.gust_vx = (self.gust_vx + dx * 4.8).clamp(-380.0, 380.0);
                self.gust_vy = (self.gust_vy + dy * 4.0).clamp(-280.0, 280.0);
            }
            self.mouse_active = true;
            let (rx, ry) = self.map_css_to_ref(css_x, css_y);
            if (rx - SWORD_X).abs() < 30.0
                && ry >= SWORD_TOP_Y - 14.0
                && ry <= SWORD_GUARD_Y + 14.0
                && self.click_glint_timer <= 0.05
            {
                self.click_glint_timer = 0.88;
            }
            false
        } else if kind == 1 {
            self.mouse_active = true;
            let nx = ((css_x / self.css_w.max(1.0)) - 0.5) * 2.0;
            self.gust_vx = (self.gust_vx + 250.0 + nx * 95.0).clamp(-400.0, 440.0);
            self.gust_vy = (self.gust_vy - 125.0).clamp(-300.0, 300.0);
            self.click_glint_timer = 0.92;
            self.click_blink_timer = 0.24;
            true
        } else {
            false
        }
    }

    pub fn step(&mut self, dt: f32) {
        let clamped_dt = dt.clamp(0.0, 0.06);
        self.anim_time += clamped_dt * self.params.speed;
        if self.click_glint_timer > 0.0 {
            self.click_glint_timer = (self.click_glint_timer - clamped_dt).max(0.0);
        }
        if self.click_blink_timer > 0.0 {
            self.click_blink_timer = (self.click_blink_timer - clamped_dt).max(0.0);
        }

        let (target_gx, target_gy) = if self.mouse_active && self.params.mouse_interact {
            let (rx, ry) = self.map_css_to_ref(self.mouse_x, self.mouse_y);
            let dx = ((rx - HEAD_CX) / 150.0).clamp(-1.0, 1.0) * 3.8;
            let dy = ((ry - HEAD_CY) / 150.0).clamp(-1.0, 1.0) * 2.6;
            (dx, dy)
        } else {
            (0.0, 0.0)
        };
        let lerp_k = (clamped_dt * 8.0).min(1.0);
        self.gaze_x += (target_gx - self.gaze_x) * lerp_k;
        self.gaze_y += (target_gy - self.gaze_y) * lerp_k;

        let damp = (-clamped_dt * 4.2).exp();
        self.gust_vx *= damp;
        self.gust_vy *= damp;

        self.icon_obstacles_ref.clear();
        if self.params.adaptive && self.icons_vis {
            let cols = self.canvas.cols.max(1) as f32;
            let rows = self.canvas.rows.max(1) as f32;
            let (cx, cy, view_h, _) = self.camera_window(self.anim_time);
            let cell_ratio = (self.css_h / rows) / (self.css_w / cols);
            let view_w = view_h * (cols / rows) / cell_ratio.max(0.5);
            for r in &self.icons {
                let ix = r.x + r.w * 0.5;
                let iy = r.y + r.h * 0.5;
                let ref_x = cx + ((ix / self.css_w.max(1.0)) - 0.5) * view_w;
                let ref_y = cy + ((iy / self.css_h.max(1.0)) - 0.5) * view_h;
                let rad_ref = (r.w.max(r.h) * 0.5 / self.css_h.max(1.0)) * view_h + 22.0;
                self.icon_obstacles_ref.push([ref_x, ref_y, rad_ref, 0.0]);
            }
        }

        self.cloth.step(
            clamped_dt * self.params.speed,
            self.anim_time,
            self.params.wind,
            self.gust_vx,
            self.gust_vy,
            &self.icon_obstacles_ref,
        );

        self.rebuild_grid();
    }

    pub fn rebuild_grid(&mut self) {
        let cols = self.canvas.cols;
        let rows = self.canvas.rows;
        if cols == 0 || rows == 0 {
            return;
        }
        let len = cols * rows;
        if self.cape_lum_buf.len() != len {
            self.cape_lum_buf.resize(len, 0.0);
            self.cape_alpha_buf.resize(len, 0.0);
        }

        let p = portrait();
        let t = self.anim_time;
        let lt = t.rem_euclid(LOOP_SECONDS);
        let (cx, cy, view_h, zoom_k) = self.camera_window(t);

        let cw = self.css_w / cols as f32;
        let ch = self.css_h / rows as f32;
        let sy = view_h / rows as f32;
        let sx = sy * (cw / ch.max(0.1));
        let ox = cx - (cols as f32 * 0.5) * sx;
        let oy = cy - (rows as f32 * 0.5) * sy;

        let (iris_r2, grass_born) = if self.params.camera_mode == 1 {
            let born_circle = smoothstep01(lt / 2.2);
            let r_max = 104.0 + 420.0 * zoom_k;
            let r = if lt < 2.2 { 28.0 + (r_max - 28.0) * born_circle } else { -1.0 };
            let g_born = smoothstep01((lt - 4.5) / 1.4);
            (if r > 0.0 { r * r } else { -1.0 }, g_born)
        } else {
            (-1.0, 1.0)
        };

        let mut auto_blink = 0.0f32;
        for b0 in BLINK_STARTS {
            let bq = (lt - b0) / 0.24;
            if (0.0..=1.0).contains(&bq) {
                auto_blink = auto_blink.max((bq * std::f32::consts::PI).sin());
            }
        }
        let click_blink = if self.click_blink_timer > 0.0 {
            ((1.0 - self.click_blink_timer / 0.24).clamp(0.0, 1.0) * std::f32::consts::PI).sin()
        } else {
            0.0
        };
        let blink = auto_blink.max(click_blink).clamp(0.0, 1.0);

        let mut glint_q: Option<f32> = None;
        for b0 in BLINK_STARTS {
            let gq = (lt - b0) / 0.90;
            if (0.0..=1.0).contains(&gq) {
                glint_q = Some(gq);
            }
        }
        if self.click_glint_timer > 0.0 {
            let gq = 1.0 - (self.click_glint_timer / 0.92).clamp(0.0, 1.0);
            glint_q = Some(glint_q.map_or(gq, |cur| cur.min(gq)));
        }
        let glint_y = glint_q.map(|q| 536.0 - 460.0 * q);
        let w_now = cape_wind_scalar(t, self.params.wind) + (self.gust_vx / 220.0);

        let bg_rgb = match self.params.palette {
            1 => [3, 2, 1],
            2 => [1, 2, 5],
            _ => [0, 0, 0],
        };
        self.canvas.clear(bg_rgb, 100.0);

        self.cloth.rasterize_to_cells(
            cols,
            rows,
            ox,
            oy,
            sx,
            sy,
            t,
            HEAD_CX,
            HEAD_CY,
            iris_r2,
            &mut self.cape_lum_buf,
            &mut self.cape_alpha_buf,
        );

        let ramp_len_m1 = (WOVEN_RAMP.len() - 1) as f32;
        let qx_off = 0.25 * sx;
        let qy_off = 0.25 * sy;

        for r in 0..rows {
            let ref_y = oy + (r as f32 + 0.5) * sy;
            let outside_y = ref_y < 52.0 || ref_y > REF_H as f32;
            for c in 0..cols {
                let ref_x = ox + (c as f32 + 0.5) * sx;
                if iris_r2 > 0.0 {
                    let dx_i = ref_x - HEAD_CX;
                    let dy_i = ref_y - HEAD_CY;
                    if dx_i * dx_i + dy_i * dy_i > iris_r2 {
                        continue;
                    }
                }

                let idx = r * cols + c;
                let c_a = self.cape_alpha_buf[idx];
                if outside_y && c_a <= 0.01 {
                    continue;
                }

                let mut rgb_sum = [0.0f32; 3];
                let mut cov_sum = 0.0f32;
                let mut glint_max = 0.0f32;
                // Subsample quadrant alphas for crisp edge glyph orientation (`/`, `\`, `|`, `-`)
                let mut q_cov = [0.0f32; 4];

                let may_have_body = ref_x >= -8.0 && ref_x <= REF_W as f32 + 8.0 && ref_y >= 64.0 && ref_y <= 780.0;
                let may_have_grass = grass_born > 0.01 && ref_y >= 638.0 && ref_y <= 776.0 && ref_x >= 100.0 && ref_x <= 550.0;

                // Quick spatial rejection for completely empty pure-black columns
                if c_a <= 0.01 && !may_have_grass && (ref_x < 130.0 || ref_x > 460.0) {
                    continue;
                }

                // 2x2 area supersampling inside the cell
                for q in 0..4 {
                    let sub_x = ref_x + if q & 1 == 0 { -qx_off } else { qx_off };
                    let sub_y = ref_y + if q & 2 == 0 { -qy_off } else { qy_off };

                    let mut sub_rgb = [0.0f32; 3];
                    let mut sub_cov = 0.0f32;

                    if c_a > 0.01 {
                        let c_lum = self.cape_lum_buf[idx];
                        sub_rgb = [c_lum * 0.94, c_lum * 0.89, c_lum * 0.82];
                        sub_cov = c_a;
                    }

                    if may_have_grass {
                        let sway_x = 2.8 * w_now * (t * 3.0 + sub_x * 0.042 + sub_y * 0.075).sin();
                        let gx = sub_x - sway_x;
                        let g_rad = sample_bilinear_f32(&p.grass_radius, gx, sub_y);
                        if g_rad <= grass_born {
                            let g_a = sample_bilinear_f32(&p.grass_mask, gx, sub_y);
                            if g_a > 0.02 {
                                let g_col = sample_bilinear_rgb(&p.grass_rgb, gx, sub_y);
                                for ch in 0..3 {
                                    sub_rgb[ch] = sub_rgb[ch] * (1.0 - g_a) + g_col[ch] * g_a;
                                }
                                sub_cov = sub_cov.max(g_a);
                            }
                        }
                    }

                    if may_have_body {
                        let fur_w = sample_bilinear_f32(&p.fur_weight, sub_x, sub_y);
                        let fx = sub_x - (2.1 * w_now * (t * 5.2 + sub_y * 0.11).sin()) * fur_w;
                        let fy = sub_y - (1.1 * w_now * (t * 4.1 + sub_x * 0.10).sin()) * fur_w;

                        let b_a = sample_bilinear_f32(&p.body_mask, fx, fy);
                        if b_a > 0.02 {
                            let eye_w = sample_bilinear_f32(&p.eye_mask, fx, fy);
                            let (sx_eye, sy_eye) = if eye_w > 0.25 && blink < 0.85 {
                                (fx - self.gaze_x * eye_w, fy - self.gaze_y * eye_w)
                            } else {
                                (fx, fy)
                            };
                            let c_open = sample_bilinear_rgb(&p.rgb_open, sx_eye, sy_eye);
                            let c_closed = sample_bilinear_rgb(&p.rgb_closed, fx, fy);
                            let mut k_col = [0.0f32; 3];
                            let eff_blink = blink * eye_w;
                            for ch in 0..3 {
                                k_col[ch] = c_open[ch] * (1.0 - eff_blink) + c_closed[ch] * eff_blink;
                            }

                            let sw_w = sample_bilinear_f32(&p.sword_mask, fx, fy);
                            if sw_w > 0.05 {
                                if let Some(gy_band) = glint_y {
                                    let dy_g = sub_y - gy_band;
                                    let pulse = (-((dy_g * dy_g) / 165.0)).exp() * sw_w;
                                    if pulse > 0.01 {
                                        glint_max = glint_max.max(pulse);
                                        for ch in 0..3 {
                                            k_col[ch] = (k_col[ch] + pulse * 0.72).clamp(0.0, 1.0);
                                        }
                                    }
                                }
                            }

                            for ch in 0..3 {
                                sub_rgb[ch] = sub_rgb[ch] * (1.0 - b_a) + k_col[ch] * b_a;
                            }
                            sub_cov = sub_cov.max(b_a);
                        }
                    }

                    rgb_sum[0] += sub_rgb[0] * sub_cov;
                    rgb_sum[1] += sub_rgb[1] * sub_cov;
                    rgb_sum[2] += sub_rgb[2] * sub_cov;
                    cov_sum += sub_cov;
                    q_cov[q] = sub_cov;
                }

                let cov = cov_sum * 0.25;
                if cov > 0.03 {
                    let avg_rgb = [
                        (rgb_sum[0] / cov_sum.max(1e-4)).clamp(0.0, 1.0),
                        (rgb_sum[1] / cov_sum.max(1e-4)).clamp(0.0, 1.0),
                        (rgb_sum[2] / cov_sum.max(1e-4)).clamp(0.0, 1.0),
                    ];
                    let lum = (0.299 * avg_rgb[0] + 0.587 * avg_rgb[1] + 0.114 * avg_rgb[2]).clamp(0.0, 1.0);

                    // Soft continuous photographic underlay into `canvas.bg` (keeps dark eyes dark,
                    // fur silky, and outer black background untouched `[0, 0, 0]`)
                    let bg_photo_scale = 0.44 * cov;
                    let underlay = apply_palette_rgb(
                        [
                            avg_rgb[0] * bg_photo_scale,
                            avg_rgb[1] * bg_photo_scale,
                            avg_rgb[2] * bg_photo_scale,
                        ],
                        self.params.palette,
                        glint_max * 0.25,
                    );
                    self.canvas.set_bg_rgb(
                        c as i32,
                        r as i32,
                        [clamp_u8(underlay[0]), clamp_u8(underlay[1]), clamp_u8(underlay[2])],
                    );

                    // Choose glyph: silhouette edge slash/bar on outer partial contours,
                    // otherwise woven character with subtle organic hash dither.
                    let gx = (q_cov[1] + q_cov[3]) - (q_cov[0] + q_cov[2]);
                    let gy = (q_cov[2] + q_cov[3]) - (q_cov[0] + q_cov[1]);
                    let grad = (gx * gx + gy * gy).sqrt();

                    let glyph = if cov < 0.72 && grad > 0.26 {
                        if gx.abs() > 2.1 * gy.abs() {
                            glyph_id('|')
                        } else if gy.abs() > 2.1 * gx.abs() {
                            glyph_id(if gy > 0.0 { '-' } else { '_' })
                        } else if gx * gy < 0.0 {
                            glyph_id('/')
                        } else {
                            glyph_id('\\')
                        }
                    } else {
                        let base_t = lum.powf(0.84) * (0.45 + 0.55 * cov);
                        let dither = cell_hash(c, r) * (2.2 / ramp_len_m1);
                        let ramp_val = (base_t + dither).clamp(0.02, 1.0);
                        ramp_glyph(WOVEN_RAMP, ramp_val)
                    };

                    // Crisp luminous ASCII ink (`photo * 1.18 + gentle floor lift`)
                    let ink_boost = 1.16 + 0.14 * glint_max;
                    let ink_raw = apply_palette_rgb(
                        [
                            (avg_rgb[0] * ink_boost + 0.03 * cov).clamp(0.0, 1.0),
                            (avg_rgb[1] * ink_boost + 0.03 * cov).clamp(0.0, 1.0),
                            (avg_rgb[2] * ink_boost + 0.025 * cov).clamp(0.0, 1.0),
                        ],
                        self.params.palette,
                        glint_max,
                    );

                    let bloom = if glint_max > 0.1 {
                        245u8
                    } else if lum > 0.78 {
                        115u8
                    } else {
                        22u8
                    };

                    self.canvas.set_cell(
                        c as i32,
                        r as i32,
                        glyph,
                        [clamp_u8(ink_raw[0]), clamp_u8(ink_raw[1]), clamp_u8(ink_raw[2])],
                        bloom,
                        1,
                    );
                }
            }
        }

        // Trailing cape-hem wind wisps (`.`, `'`, `-`, `~`)
        for w in &self.cloth.wisps {
            let wc = ((w.x - ox) / sx).round() as i32;
            let wr = ((w.y - oy) / sy).round() as i32;
            if let Some(cell_idx) = self.canvas.idx(wc, wr) {
                if self.canvas.owner[cell_idx] == 0 {
                    let life_t = (w.life / w.max_life).clamp(0.0, 1.0);
                    let fade = (life_t * std::f32::consts::PI).sin() * 0.38;
                    if fade > 0.06 {
                        let out_c = apply_palette_rgb([fade, fade * 0.96, fade * 0.88], self.params.palette, 0.0);
                        let ch = if fade > 0.28 { '~' } else if fade > 0.17 { '-' } else { '.' };
                        self.canvas.set_cell(
                            wc,
                            wr,
                            glyph_id(ch),
                            [clamp_u8(out_c[0]), clamp_u8(out_c[1]), clamp_u8(out_c[2])],
                            20,
                            2,
                        );
                    }
                }
            }
        }

        self.canvas.mark_dirty();
    }
}

pub struct AsciiCatKnightRenderer {
    grid: AsciiGridRenderer,
}

impl AsciiCatKnightRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        Ok(Self {
            grid: AsciiGridRenderer::new(gl)?,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        self.grid.destroy(gl);
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let dpi = st.dpi_scale.max(0.5);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let s = &mut st.ascii_cat_knight;
        s.resize(css_w, css_h, dpi);
        self.grid.render(
            gl,
            &s.canvas,
            5,
            pw,
            ph,
            dpi,
            s.params.glow * 0.62,
            0.12,
            s.params.adaptive,
            s.icons_vis,
            &s.icons,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gl::*;

    #[test]
    fn params_parsing_and_clamping() {
        let mut raw = HashMap::new();
        raw.insert("palette".into(), Value::String("amber".into()));
        raw.insert("cameraMode".into(), Value::String("cinematic".into()));
        raw.insert("charSize".into(), Value::from(24.0));
        raw.insert("glow".into(), Value::from(2.2));
        raw.insert("wind".into(), Value::from(1.75));
        raw.insert("speed".into(), Value::from(0.1));
        raw.insert("mouseInteract".into(), Value::from(0));
        raw.insert("adaptive".into(), Value::Bool(false));

        let p = AsciiCatKnightParams::from_raw_params(&raw);
        assert_eq!(p.palette, 1);
        assert_eq!(p.camera_mode, 1);
        assert!((p.char_size - 12.0).abs() < 1e-5);
        assert!((p.glow - 1.5).abs() < 1e-5);
        assert!((p.wind - 1.75).abs() < 1e-5);
        assert!((p.speed - 0.3).abs() < 1e-5);
        assert!(!p.mouse_interact);
        assert!(!p.adaptive);
    }

    #[test]
    fn deterministic_simulation_and_interactive_response() {
        let icons = vec![
            Rect { x: 48.0, y: 48.0, w: 68.0, h: 88.0 },
            Rect { x: 920.0, y: 380.0, w: 72.0, h: 90.0 },
        ];
        let mut a = AsciiCatKnightState::new(1280.0, 720.0, 1.0);
        let mut b = AsciiCatKnightState::new(1280.0, 720.0, 1.0);
        a.step(0.033);
        b.step(0.033);
        assert_eq!(a.canvas.cells, b.canvas.cells);

        assert!(a.handle_mouse(1, 780.0, 360.0));
        a.set_icons(&icons, true);
        a.step(0.05);
        assert_ne!(a.canvas.cells, b.canvas.cells);
    }

    #[test]
    fn cloth_verlet_pins_and_bounds_stay_finite() {
        let mut cloth = CapeCloth::default();
        let rest_copy = cloth.rest.clone();
        for k in 0..120 {
            cloth.step(
                0.016,
                k as f32 * 0.016,
                1.8,
                320.0,
                -180.0,
                &[[460.0, 560.0, 52.0, 0.0]],
            );
        }
        for (i, &is_pin) in cloth.pin.iter().enumerate() {
            let p = cloth.pos[i];
            assert!(p[0].is_finite() && p[1].is_finite(), "non-finite vertex {i}");
            if is_pin {
                assert!((p[0] - rest_copy[i][0]).abs() < 1e-4);
                assert!((p[1] - rest_copy[i][1]).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn resize_edge_cases_and_sub_half_px_guard() {
        let mut st = AsciiCatKnightState::new(1280.0, 720.0, 1.0);
        let rev0 = st.canvas.rev;
        st.resize(1280.3, 720.3, 1.0);
        assert_eq!(st.canvas.rev, rev0);
        assert!(st.canvas.cols >= 260 && st.canvas.rows >= 95);
        for (w, h) in [(64.0, 64.0), (1920.0, 1080.0), (3440.0, 1440.0)] {
            st.resize(w, h, 1.25);
            st.step(0.03);
            assert!(st.canvas.cols >= 58 && st.canvas.rows >= 38);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn gpu_render_ascii_cat_knight_pngs() {
        use crate::gl::WglContext;
        use crate::renderer::Renderer;
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExA, DefWindowProcA, DestroyWindow, RegisterClassA, WNDCLASSA, WS_POPUP,
        };

        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarCatKnightGpuTest\0";
            let wc = WNDCLASSA {
                style: 0x0020,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassA(&wc);
            let (pw, ph) = (1280i32, 720i32);
            let hwnd = CreateWindowExA(
                0,
                cls.as_ptr(),
                b"catknighttest\0".as_ptr(),
                WS_POPUP,
                0,
                0,
                64,
                64,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinst,
                std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let ctx = WglContext::create(hwnd).expect("WglContext failed");
            assert!(ctx.make_current());
            let gl = GlFns::load().expect("GlFns::load failed");

            let mut fbo = 0u32;
            let mut tex = 0u32;
            (gl.GenFramebuffers)(1, &mut fbo);
            (gl.GenTextures)(1, &mut tex);
            (gl.BindTexture)(GL_TEXTURE_2D, tex);
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
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as GLint);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as GLint);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, fbo);
            (gl.FramebufferTexture2D)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, tex, 0);
            assert_eq!((gl.CheckFramebufferStatus)(GL_FRAMEBUFFER), GL_FRAMEBUFFER_COMPLETE);

            let mut renderer = Renderer::new(&gl).expect("Renderer::new failed");
            let mut st = AppState::new(pw, ph, 1.0);
            st.set_theme("ascii-cat-knight");
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[48,48,116,136],[48,160,116,248],[1080,460,1148,548]]}"#);

            let out_dir = std::env::var_os("DIYARDESK_ASCII_VERIFY_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let _ = std::fs::create_dir_all(&out_dir);

            let mut raw = vec![0u8; (pw * ph * 4) as usize];
            let mut flipped = vec![0u8; (pw * ph * 4) as usize];

            let mut render_shot = |name: &str, st: &mut AppState| -> Vec<u8> {
                while (gl.GetError)() != GL_NO_ERROR {}
                let _guard = DefaultFboGuard::new(fbo);
                (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
                renderer.render(&gl, st);
                let err = (gl.GetError)();
                assert_eq!(err, GL_NO_ERROR, "GL error 0x{:X} on {name}", err);
                (gl.Finish)();
                (gl.ReadPixels)(
                    0,
                    0,
                    pw,
                    ph,
                    GL_RGBA,
                    GL_UNSIGNED_BYTE,
                    raw.as_mut_ptr() as *mut std::ffi::c_void,
                );
                drop(_guard);
                assert!(raw.chunks_exact(4).all(|px| px[3] == 255), "{name} leaked non-opaque alpha");
                let lit_px = raw.chunks_exact(4).filter(|px| px[0] > 28 || px[1] > 28 || px[2] > 28).count();
                assert!(lit_px > 6000, "{name} too few lit pixels ({lit_px})");
                crate::flip_rgba_vertical(&raw, &mut flipped, pw as usize, ph as usize);
                if std::env::var_os("DIYARDESK_ASCII_VERIFY_DIR").is_some() {
                    let png_path = out_dir.join(name);
                    image::save_buffer(&png_path, &flipped, pw as u32, ph as u32, image::ColorType::Rgba8)
                        .expect("save cat knight png");
                }
                raw.clone()
            };

            // 1. Full Knight shot
            st.step(0.45);
            let full_buf = render_shot("verify_ascii_cat_knight_full.png", &mut st);

            // 2. Portrait / Face close-up shot (without mid-cape icon scrim)
            st.handle_json(r#"{"t":"icons","vis":false,"r":[]}"#);
            st.handle_json(r#"{"t":"settings","v":{"cameraMode":"portrait"}}"#);
            st.step(0.05);
            let face_buf = render_shot("verify_ascii_cat_knight_face.png", &mut st);
            assert_ne!(full_buf, face_buf);

            // 3. Full Knight + Sword Glint shot
            st.handle_json(r#"{"t":"icons","vis":true,"r":[[48,48,116,136],[48,160,116,248],[1080,460,1148,548]]}"#);
            st.handle_json(r#"{"t":"settings","v":{"cameraMode":"full"}}"#);
            st.ascii_cat_knight.anim_time = 7.35;
            st.step(0.04);
            let glint_buf = render_shot("verify_ascii_cat_knight_glint.png", &mut st);
            assert_ne!(full_buf, glint_buf);

            renderer.destroy(&gl);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
            (gl.DeleteTextures)(1, &tex);
            (gl.DeleteFramebuffers)(1, &fbo);
            drop(ctx);
            DestroyWindow(hwnd);
        }
    }
}
