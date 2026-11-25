pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use crate::text::TextRenderer;
use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shaders::{CAT_SHORE_FRAG, CAT_SHORE_VERT, NOTCH_UI_FRAG, NOTCH_UI_VERT};
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::path::PathBuf;

// ============================================================================
// 1. PARAMS (1:1 with prototype/themes.json)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CatShoreParams {
    pub notch_mode: usize,     // 0=auto, 1=pinned, 2=collapsed, 3=off
    pub cat_scale: f32,        // 0.7..=1.4 (default 1.0)
    pub ripple_strength: f32,  // 0.2..=1.6 (default 1.0)
    pub water_speed: f32,      // 0.2..=2.0 (default 1.0)
    pub shore_tone: usize,     // 0=daylight, 1=golden, 2=twilight
    pub adaptive: bool,        // default true
    pub seed: u32,             // default 42
}

impl Default for CatShoreParams {
    fn default() -> Self {
        Self {
            notch_mode: 0,
            cat_scale: 1.0,
            ripple_strength: 1.0,
            water_speed: 1.0,
            shore_tone: 0,
            adaptive: true,
            seed: 42,
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

impl CatShoreParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        Self {
            notch_mode: parse_enum(raw, "notchMode", &["auto", "pinned", "collapsed", "off"], 0),
            cat_scale: parse_f32(raw, "catScale", 1.0, 0.7, 1.4),
            ripple_strength: parse_f32(raw, "rippleStrength", 1.0, 0.2, 1.6),
            water_speed: parse_f32(raw, "waterSpeed", 1.0, 0.2, 2.0),
            shore_tone: parse_enum(raw, "shoreTone", &["daylight", "golden", "twilight"], 0),
            adaptive: parse_bool(raw, "adaptive", true),
            seed: parse_f32(raw, "seed", 42.0, 0.0, 999999.0) as u32,
        }
    }
}

// ============================================================================
// 2. DETERMINISTIC PRNG & PAINTERLY PEBBLE SHORE TEXTURE BAKER
// ============================================================================

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
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn cpu_hash21(x: f32, y: f32) -> f32 {
    let mut px = (x * 123.34).fract();
    let mut py = (y * 456.21).fract();
    if px < 0.0 {
        px += 1.0;
    }
    if py < 0.0 {
        py += 1.0;
    }
    let d = px * (px + 45.32) + py * (py + 45.32);
    px += d;
    py += d;
    let r = (px * py).fract();
    if r < 0.0 { r + 1.0 } else { r }
}

fn cpu_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = cpu_hash21(ix, iy);
    let b = cpu_hash21(ix + 1.0, iy);
    let c = cpu_hash21(ix, iy + 1.0);
    let d = cpu_hash21(ix + 1.0, iy + 1.0);
    (a + (b - a) * ux) + ((c + (d - c) * ux) - (a + (b - a) * ux)) * uy
}

fn shore_water_depth(u: f32, v: f32) -> f32 {
    // 0.0 = dry sunlit pebble beach (top-right), 1.0 = deep cerulean water (left / bottom-left)
    let diag = (u - 0.34) * 0.96 - (v - 0.04) * 0.92;
    let wob = (cpu_noise(u * 7.0 + 1.3, v * 5.5 + 2.7) - 0.5) * 0.08
        + (cpu_noise(u * 14.0 + 4.2, v * 11.0 + 8.1) - 0.5) * 0.03;
    smoothstep(0.18, -0.18, diag + wob)
}

/// Pre-bakes a rich Ghibli/gouache-style pebble beach + shallow-to-deep cerulean seabed.
pub fn build_pebble_shore_texture(w: usize, h: usize, seed: u32) -> Vec<u8> {
    let mut buf = vec![0.0f32; w * h * 3];

    // 1. Base painterly gradient (warm golden sand beach -> luminous turquoise shallows -> deep Prussian water)
    let dry_sand_hi = [0.91, 0.79, 0.51];
    let dry_sand_lo = [0.80, 0.65, 0.38];
    let shallow_aqua = [0.39, 0.68, 0.79];
    let mid_water = [0.23, 0.50, 0.65];
    let deep_water = [0.13, 0.33, 0.45];

    for y in 0..h {
        let v = (y as f32 + 0.5) / h as f32;
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32;
            let depth = shore_water_depth(u, v);
            let n1 = cpu_noise(u * 16.0 + 2.1, v * 10.0 + 1.4);
            let n2 = cpu_noise(u * 36.0 + 7.7, v * 24.0 + 3.9);
            let brush = (n1 * 0.65 + n2 * 0.35 - 0.5) * 0.14;

            let sand = lerp3(dry_sand_lo, dry_sand_hi, (0.55 + brush * 2.5 + u * 0.35 - v * 0.2).clamp(0.0, 1.0));
            let water_mid = lerp3(shallow_aqua, mid_water, smoothstep(0.15, 0.65, depth));
            let water = lerp3(water_mid, deep_water, smoothstep(0.55, 1.0, depth));
            let mut c = lerp3(sand, water, smoothstep(0.0, 0.42, depth));

            // Subtle gouache diagonal sand strokes on dry shore
            if depth < 0.3 {
                let streak = cpu_noise((u + v) * 28.0, (u - v) * 8.0) - 0.5;
                let s = (1.0 - depth / 0.3) * streak * 0.12;
                c[0] += s;
                c[1] += s * 0.88;
                c[2] += s * 0.55;
            }
            // Subtle sunlit turquoise bloom right around the cat's standing zone
            let dx = u - 0.43;
            let dy = (v - 0.55) * 1.15;
            let glow = (-((dx * dx + dy * dy) / 0.085)).exp() * 0.14;
            c = lerp3(c, [0.46, 0.74, 0.85], glow * smoothstep(0.15, 0.5, depth));

            let idx = (y * w + x) * 3;
            buf[idx] = c[0].clamp(0.0, 1.0);
            buf[idx + 1] = c[1].clamp(0.0, 1.0);
            buf[idx + 2] = c[2].clamp(0.0, 1.0);
        }
    }

    // 2. Layered painterly gouache pebbles (dry beach boulders, shoreline stones, submerged pebbles)
    struct Pebble {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rot: f32,
        tone_idx: usize,
        luma: f32,
        depth: f32,
    }

    let mut rng = Rng::new(seed);
    let mut pebbles: Vec<Pebble> = Vec::with_capacity(820);

    // Dense cluster of warm sunlit pebbles along the upper-right beach & shallow shoreline
    for i in 0..820 {
        let (u, v) = if i < 420 {
            // Concentrated on dry beach & shallow water band (top-right diagonal)
            let t = rng.f32();
            let along = rng.range(-0.08, 1.08);
            let across = rng.range(-0.14, 0.48) * (0.4 + 0.6 * t);
            let u = (0.34 + along * 0.68 + across * 0.72).clamp(0.02, 0.99);
            let v = (-0.02 + along * 0.74 - across * 0.68).clamp(0.01, 0.98);
            (u, v)
        } else if i < 650 {
            // Mid-shore scattered river stones right under the shallow turquoise ripples
            let u = rng.range(0.16, 0.98);
            let v = rng.range(0.02, 0.98);
            (u, v)
        } else {
            // Deep left water faint dark river stones
            let u = rng.range(0.01, 0.45);
            let v = rng.range(0.05, 0.98);
            (u, v)
        };

        let depth = shore_water_depth(u, v);
        // Keep a bit of open clear water immediately under the cat so the cat & ripples pop cleanly
        let d_cat = ((u - 0.43).powi(2) + ((v - 0.55) * 1.1).powi(2)).sqrt();
        if d_cat < 0.10 && rng.f32() < 0.72 {
            continue;
        }

        let size_bias = if depth < 0.25 {
            rng.range(0.75, 2.25)
        } else if depth < 0.65 {
            rng.range(0.60, 1.75)
        } else {
            rng.range(0.85, 2.30)
        };
        let rx = rng.range(4.5, 16.5) * size_bias * (w as f32 / 960.0);
        let ry = rx * rng.range(0.50, 0.88);
        pebbles.push(Pebble {
            cx: u * w as f32,
            cy: v * h as f32,
            rx,
            ry,
            rot: rng.range(-0.9, 0.9) + if depth < 0.2 { 0.45 } else { 0.0 },
            tone_idx: (rng.next_u32() as usize) % 6,
            luma: rng.range(0.84, 1.14),
            depth,
        });
    }

    // Paint larger base stones first, smaller crisp pebbles on top
    pebbles.sort_by(|a, b| b.rx.partial_cmp(&a.rx).unwrap_or(std::cmp::Ordering::Equal));

    const DRY_PALETTE: [[f32; 3]; 6] = [
        [0.86, 0.74, 0.48], // warm golden sand stone
        [0.76, 0.62, 0.39], // ochre river pebble
        [0.91, 0.82, 0.60], // sunlit pale cream stone
        [0.64, 0.53, 0.37], // warm umber stone
        [0.72, 0.64, 0.52], // soft taupe pebble
        [0.54, 0.47, 0.38], // dark river pebble
    ];
    const WET_PALETTE: [[f32; 3]; 6] = [
        [0.44, 0.62, 0.68], // sunlit shallow teal stone
        [0.32, 0.53, 0.63], // cerulean submerged pebble
        [0.54, 0.68, 0.64], // warm-aqua translucent stone
        [0.23, 0.43, 0.55], // deep slate-blue stone
        [0.20, 0.39, 0.50], // dark river cobble
        [0.14, 0.30, 0.41], // deep shadow stone
    ];

    for p in &pebbles {
        let cos_r = p.rot.cos();
        let sin_r = p.rot.sin();
        let bound = p.rx.max(p.ry) + 6.0;
        let x0 = ((p.cx - bound).floor() as i32).max(0) as usize;
        let x1 = ((p.cx + bound).ceil() as i32).min(w as i32 - 1) as usize;
        let y0 = ((p.cy - bound).floor() as i32).max(0) as usize;
        let y1 = ((p.cy + bound).ceil() as i32).min(h as i32 - 1) as usize;

        let dry_col = DRY_PALETTE[p.tone_idx];
        let wet_col = WET_PALETTE[p.tone_idx];
        let base_col = lerp3(dry_col, wet_col, smoothstep(0.02, 0.62, p.depth));
        let base_col = [
            (base_col[0] * p.luma).clamp(0.0, 1.0),
            (base_col[1] * p.luma).clamp(0.0, 1.0),
            (base_col[2] * p.luma).clamp(0.0, 1.0),
        ];
        let hi_col = lerp3(base_col, [0.97, 0.92, 0.76], if p.depth < 0.35 { 0.44 } else { 0.24 });
        let sh_col = lerp3(base_col, [0.10, 0.22, 0.31], if p.depth < 0.25 { 0.48 } else { 0.58 });
        let outline_col = lerp3([0.28, 0.23, 0.17], [0.09, 0.22, 0.31], smoothstep(0.05, 0.5, p.depth));
        let pebble_opacity = if p.depth > 0.72 { 0.24 } else if p.depth > 0.42 { 0.56 } else { 0.90 };

        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 - p.cx;
                let dy = y as f32 - p.cy;

                // Soft cast shadow shifted down-left (sun is top-right!)
                let sdx = dx + p.rx * 0.22;
                let sdy = dy - p.ry * 0.26;
                let slx = (sdx * cos_r + sdy * sin_r) / (p.rx * 1.04);
                let sly = (-sdx * sin_r + sdy * cos_r) / (p.ry * 1.04);
                let sd = (slx * slx + sly * sly).sqrt();

                let lx = (dx * cos_r + dy * sin_r) / p.rx;
                let ly = (-dx * sin_r + dy * cos_r) / p.ry;
                // Slight organic rock irregularity
                let ang = ly.atan2(lx);
                let bump = 1.0 + 0.07 * (ang * 3.0 + p.rot * 2.0).sin() + 0.04 * (ang * 5.0).cos();
                let d = (lx * lx + ly * ly).sqrt() / bump;

                let idx = (y * w + x) * 3;
                if sd < 1.08 && d >= 0.92 {
                    let sha = smoothstep(1.08, 0.75, sd) * 0.32 * pebble_opacity;
                    buf[idx] = buf[idx] * (1.0 - sha) + outline_col[0] * sha;
                    buf[idx + 1] = buf[idx + 1] * (1.0 - sha) + outline_col[1] * sha;
                    buf[idx + 2] = buf[idx + 2] * (1.0 - sha) + outline_col[2] * sha;
                }
                if d < 1.06 {
                    let body_a = smoothstep(1.06, 0.92, d) * pebble_opacity;
                    // Sunlit top-right dome shading
                    let light = (lx * 0.55 - ly * 0.70) * 0.5 + 0.5;
                    let mut c = lerp3(sh_col, base_col, smoothstep(0.15, 0.62, light));
                    c = lerp3(c, hi_col, smoothstep(0.62, 0.95, light) * smoothstep(0.96, 0.45, d));
                    // Stylized thin painter's ink rim
                    let rim = smoothstep(0.76, 0.97, d);
                    c = lerp3(c, outline_col, rim * 0.55);

                    buf[idx] = buf[idx] * (1.0 - body_a) + c[0] * body_a;
                    buf[idx + 1] = buf[idx + 1] * (1.0 - body_a) + c[1] * body_a;
                    buf[idx + 2] = buf[idx + 2] * (1.0 - body_a) + c[2] * body_a;
                }
            }
        }
    }

    let mut rgba = vec![255u8; w * h * 4];
    for i in 0..(w * h) {
        rgba[i * 4] = (buf[i * 3].clamp(0.0, 1.0) * 255.0).round() as u8;
        rgba[i * 4 + 1] = (buf[i * 3 + 1].clamp(0.0, 1.0) * 255.0).round() as u8;
        rgba[i * 4 + 2] = (buf[i * 3 + 2].clamp(0.0, 1.0) * 255.0).round() as u8;
        rgba[i * 4 + 3] = 255;
    }
    rgba
}

// ============================================================================
// 3. STATE (Cat gaze, blink, ripples, collapsible top notch productivity bar)
// ============================================================================

#[derive(Clone, Copy, Debug)]
struct ClickRipple {
    x: f32,
    y: f32,
    age: f32,
    strength: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotchTask {
    pub title: String,
    pub meta: String,
    pub done: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotchCalEvent {
    pub date: String, // "YYYY-MM-DD"
    pub time: String, // "15:00"
    pub title: String,
    pub meta: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotchClip {
    pub title: String,
    pub body: String,
    pub time_str: String,
    pub color_idx: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedCatShore {
    tasks: Vec<NotchTask>,
    note_title: String,
    note_text: String,
    events: Vec<NotchCalEvent>,
    clips: Vec<NotchClip>,
    timer_preset_idx: usize,
}

fn cat_shore_save_path() -> Option<PathBuf> {
    #[cfg(test)]
    {
        return None;
    }
    #[cfg(not(test))]
    {
        #[cfg(target_os = "windows")]
        {
            let base = std::env::var_os("LOCALAPPDATA")?;
            let mut p = PathBuf::from(base);
            p.push("DesklandPreview");
            p.push("cat-shore-widgets.json");
            return Some(p);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let base = std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|h| {
                        let mut p = PathBuf::from(h);
                        p.push(".config");
                        p
                    })
                })?;
            let mut p = base;
            p.push("deskland");
            p.push("cat-shore-widgets.json");
            Some(p)
        }
    }
}

#[derive(Clone, Copy, Default)]
struct NotchHits {
    collapsed_pill: Rect,
    panel: Rect,
    btn_close: Rect,
    tab_workspace: Rect,
    tab_clips: Rect,
    btn_bell: Rect,
    tab_tasks_todo: Rect,
    tab_tasks_done: Rect,
    task_checks: [Rect; 3],
    task_dels: [Rect; 3],
    btn_add_task: Rect,
    btn_timer_toggle: Rect,
    btn_timer_reset: Rect,
    btn_timer_preset: Rect,
    card_notepad: Rect,
    cal_prev: Rect,
    cal_next: Rect,
    cal_days: [Rect; 7],
    cal_event_dels: [Rect; 2],
    btn_add_event: Rect,
    clip_cards: [Rect; 3],
    clip_dels: [Rect; 3],
    btn_add_clip: Rect,
    toast_rect: Rect,
    btn_toast_snooze: Rect,
    btn_toast_done: Rect,
    cat_hit: Rect,
}

pub struct CatShoreState {
    pub params: CatShoreParams,
    pub w: f32,
    pub h: f32,
    pub dpi: f32,
    pub low_mode: bool,
    pub rev: u64,
    pub shore_seed_built: u32,
    pub shore_dirty: bool,

    pub sim_time: f32,
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub pointer_active: bool,

    // Cat expressive animation state
    pub gaze_x: f32,
    pub gaze_y: f32,
    pub blink: f32,
    blink_timer: f32,
    blink_phase: f32,
    pub happy: f32,
    pub tail_phase: f32,
    ripples: Vec<ClickRipple>,

    // Icons
    pub icons: Vec<Rect>,
    pub icons_visible: bool,

    // Collapsible top notch bar state
    pub notch_open: f32,      // 0.0 = collapsed, 1.0 = expanded
    pub notch_pinned: bool,   // stays open until user clicks X or toggles
    pub notch_hover: bool,
    pub notch_tab: usize,     // 0 = Workspace, 1 = Clips
    pub reminder_open: bool,  // compact reminder toast under notch
    pub reminder_title: String,

    // Workspace interactive widgets inside the notch (all persisted!)
    pub tasks: Vec<NotchTask>,
    pub task_filter: usize, // 0 = To do, 1 = Completed
    pub timer_preset_idx: usize,
    pub timer_left_sec: f32,
    pub timer_running: bool,
    pub note_title: String,
    pub note_text: String,
    pub cal_week_offset: i64, // in days (multiples of 7)
    pub cal_selected_day_off: i64, // offset from today in days
    pub events: Vec<NotchCalEvent>,
    pub clips: Vec<NotchClip>,
    pub clip_copied_idx: Option<usize>,
    pub clip_copied_ttl: f32,
    pub pending_events: VecDeque<String>,

    hits: NotchHits,
}

const TIMER_PRESETS_MIN: [u32; 5] = [45, 25, 15, 5, 60];

impl Default for CatShoreState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0, 1.0)
    }
}

impl CatShoreState {
    pub fn new(w: f32, h: f32, dpi: f32) -> Self {
        let today = Local::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        let mut tasks = vec![
            NotchTask {
                title: "Prep slides for design".into(),
                meta: "Focused · 45 min".into(),
                done: false,
            },
            NotchTask {
                title: "Pay electricity bill".into(),
                meta: "Home · 6:30 PM".into(),
                done: false,
            },
        ];
        let mut note_title = "Notepad".to_string();
        let mut note_text = "Lead the landing page with the notch\nIdea: weekly review every Friday\nClick to edit your notes".to_string();
        let mut events = vec![
            NotchCalEvent {
                date: today_str.clone(),
                time: "15:00".into(),
                title: "Design review".into(),
                meta: "Work · 1 hr".into(),
            },
            NotchCalEvent {
                date: today_str,
                time: "16:30".into(),
                title: "1:1 sync".into(),
                meta: "Team · 30 min".into(),
            },
        ];
        let mut clips = vec![
            NotchClip {
                title: "Quick Note".into(),
                body: "Click any clip to copy it to your clipboard.".into(),
                time_str: "Pinned".into(),
                color_idx: 1,
            },
            NotchClip {
                title: "DiyarDesk".into(),
                body: "Press + Paste Clip to pin your clipboard text here.".into(),
                time_str: "Ready".into(),
                color_idx: 0,
            },
        ];
        let mut timer_preset_idx = 0usize;

        if let Some(path) = cat_shore_save_path() {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Ok(saved) = serde_json::from_str::<PersistedCatShore>(&raw) {
                    tasks = saved.tasks;
                    note_title = saved.note_title;
                    note_text = saved.note_text;
                    events = saved.events;
                    clips = saved.clips;
                    timer_preset_idx = saved.timer_preset_idx.min(TIMER_PRESETS_MIN.len() - 1);
                }
            }
        }

        let mut st = Self {
            params: CatShoreParams::default(),
            w: w.max(64.0),
            h: h.max(64.0),
            dpi: dpi.max(0.5),
            low_mode: false,
            rev: 1,
            shore_seed_built: u32::MAX,
            shore_dirty: true,
            sim_time: 0.0,
            pointer_x: w * 0.5,
            pointer_y: h * 0.22,
            pointer_active: false,
            gaze_x: 0.0,
            gaze_y: -0.35,
            blink: 0.0,
            blink_timer: 3.4,
            blink_phase: -1.0,
            happy: 0.0,
            tail_phase: 0.0,
            ripples: Vec::with_capacity(8),
            icons: Vec::new(),
            icons_visible: false,
            notch_open: 0.0,
            notch_pinned: false,
            notch_hover: false,
            notch_tab: 0,
            reminder_open: false,
            reminder_title: "Focus break".into(),
            tasks,
            task_filter: 0,
            timer_preset_idx,
            timer_left_sec: (TIMER_PRESETS_MIN[timer_preset_idx] * 60) as f32,
            timer_running: false,
            note_title,
            note_text,
            cal_week_offset: 0,
            cal_selected_day_off: 0,
            events,
            clips,
            clip_copied_idx: None,
            clip_copied_ttl: 0.0,
            pending_events: VecDeque::new(),
            hits: NotchHits::default(),
        };
        st.recompute_hits();
        st
    }

    pub fn save_widgets(&self) {
        if let Some(path) = cat_shore_save_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let payload = PersistedCatShore {
                tasks: self.tasks.clone(),
                note_title: self.note_title.clone(),
                note_text: self.note_text.clone(),
                events: self.events.clone(),
                clips: self.clips.clone(),
                timer_preset_idx: self.timer_preset_idx,
            };
            if let Ok(json) = serde_json::to_string_pretty(&payload) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    pub fn push_event(&mut self, msg: String) {
        if self.pending_events.len() < 32 {
            self.pending_events.push_back(msg);
        }
    }

    pub fn drain_events(&mut self) -> Vec<String> {
        self.pending_events.drain(..).collect()
    }

    pub fn selected_date(&self) -> NaiveDate {
        Local::now().date_naive() + ChronoDuration::days(self.cal_selected_day_off)
    }

    pub fn visible_task_indices(&self) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| if self.task_filter == 1 { t.done } else { !t.done })
            .map(|(i, _)| i)
            .take(3)
            .collect()
    }

    pub fn selected_day_event_indices(&self) -> Vec<usize> {
        let key = self.selected_date().format("%Y-%m-%d").to_string();
        self.events
            .iter()
            .enumerate()
            .filter(|(_, ev)| ev.date == key)
            .map(|(i, _)| i)
            .take(2)
            .collect()
    }

    pub fn apply_action(&mut self, v: &Value) {
        let op = v.get("op").and_then(|x| x.as_str()).unwrap_or("");
        match op {
            "addTask" => {
                let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
                let meta = v.get("meta").and_then(|x| x.as_str()).unwrap_or("").trim();
                if !title.is_empty() {
                    self.tasks.insert(
                        0,
                        NotchTask {
                            title: title.chars().take(48).collect(),
                            meta: if meta.is_empty() {
                                "Today".into()
                            } else {
                                meta.chars().take(40).collect()
                            },
                            done: false,
                        },
                    );
                    if self.tasks.len() > 24 {
                        self.tasks.truncate(24);
                    }
                    self.task_filter = 0;
                    self.notch_pinned = true;
                    self.save_widgets();
                }
            }
            "setNote" => {
                let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
                let text = v.get("text").and_then(|x| x.as_str()).unwrap_or("");
                if !title.is_empty() {
                    self.note_title = title.chars().take(32).collect();
                }
                self.note_text = text.chars().take(1200).collect();
                self.notch_pinned = true;
                self.save_widgets();
            }
            "addEvent" => {
                let date = v
                    .get("date")
                    .and_then(|x| x.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| self.selected_date().format("%Y-%m-%d").to_string());
                let time = v.get("time").and_then(|x| x.as_str()).unwrap_or("12:00").trim();
                let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
                let meta = v.get("meta").and_then(|x| x.as_str()).unwrap_or("").trim();
                if !title.is_empty() {
                    self.events.push(NotchCalEvent {
                        date,
                        time: if time.is_empty() { "12:00".into() } else { time.chars().take(10).collect() },
                        title: title.chars().take(42).collect(),
                        meta: if meta.is_empty() { "Event".into() } else { meta.chars().take(36).collect() },
                    });
                    if self.events.len() > 64 {
                        self.events.remove(0);
                    }
                    self.notch_pinned = true;
                    self.save_widgets();
                }
            }
            "addClip" => {
                let body = v.get("body").and_then(|x| x.as_str()).unwrap_or("").trim();
                if !body.is_empty() {
                    let raw_title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
                    let title: String = if !raw_title.is_empty() {
                        raw_title.chars().take(28).collect()
                    } else {
                        body.lines().next().unwrap_or("Clip").chars().take(24).collect()
                    };
                    let now = Local::now();
                    let time_str = format!("{:02}:{:02}", now.hour(), now.minute());
                    let color_idx = (self.clips.len() % 3) as u8;
                    self.clips.insert(
                        0,
                        NotchClip {
                            title,
                            body: body.chars().take(800).collect(),
                            time_str,
                            color_idx,
                        },
                    );
                    if self.clips.len() > 12 {
                        self.clips.truncate(12);
                    }
                    self.notch_tab = 1;
                    self.notch_pinned = true;
                    self.save_widgets();
                }
            }
            _ => {}
        }
    }

    pub fn cat_pos_and_scale(&self) -> (f32, f32, f32) {
        let cx = self.w * 0.44;
        let cy = self.h * 0.59;
        let base_sc = (self.w.min(self.h) * 0.125).clamp(52.0, 148.0);
        (cx, cy, base_sc * self.params.cat_scale)
    }

    pub fn resize(&mut self, w: f32, h: f32, dpi: f32) {
        let nw = w.max(64.0);
        let nh = h.max(64.0);
        let ndpi = dpi.max(0.5);
        if (nw - self.w).abs() > 0.5 || (nh - self.h).abs() > 0.5 || (ndpi - self.dpi).abs() > 0.01 {
            self.w = nw;
            self.h = nh;
            self.dpi = ndpi;
            self.rev = self.rev.wrapping_add(1);
            self.recompute_hits();
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        self.low_mode = low;
    }

    pub fn sync_params(&mut self, p: CatShoreParams) {
        if self.params.seed != p.seed {
            self.shore_dirty = true;
        }
        if self.params != p {
            self.params = p;
            if p.notch_mode == 1 {
                self.notch_pinned = true;
            } else if p.notch_mode == 2 || p.notch_mode == 3 {
                self.notch_pinned = false;
                self.reminder_open = false;
            }
            self.rev = self.rev.wrapping_add(1);
            self.recompute_hits();
        }
    }

    pub fn set_icons(&mut self, icons: &[Rect], visible: bool) {
        let same = self.icons_visible == visible
            && self.icons.len() == icons.len()
            && self.icons.iter().zip(icons.iter()).all(|(a, b)| {
                (a.x - b.x).abs() < 0.5
                    && (a.y - b.y).abs() < 0.5
                    && (a.w - b.w).abs() < 0.5
                    && (a.h - b.h).abs() < 0.5
            });
        if same {
            return;
        }
        self.icons_visible = visible;
        self.icons.clear();
        if visible {
            self.icons.extend_from_slice(&icons[..icons.len().min(24)]);
        }
        self.rev = self.rev.wrapping_add(1);
    }

    pub fn add_ripple(&mut self, x: f32, y: f32, strength: f32) {
        if self.ripples.len() >= 8 {
            self.ripples.remove(0);
        }
        self.ripples.push(ClickRipple {
            x,
            y,
            age: 0.0,
            strength,
        });
    }

    fn notch_geometry(&self) -> (f32, f32, f32, f32, f32) {
        // Returns (panel_x, panel_w, collapsed_w, collapsed_h, expanded_h)
        let ui_scale = (self.w / 1440.0).clamp(0.72, 1.15);
        let collapsed_w = (206.0 * ui_scale).clamp(150.0, 236.0);
        let collapsed_h = (30.0 * ui_scale).clamp(24.0, 34.0);
        let max_w = (self.w - 32.0).max(260.0);
        let panel_w = (812.0 * ui_scale).clamp(520.0, 880.0).min(max_w);
        let expanded_h = if self.notch_tab == 1 {
            (254.0 * ui_scale).clamp(190.0, 282.0)
        } else {
            (322.0 * ui_scale).clamp(240.0, 360.0)
        }
        .min((self.h * 0.56).max(160.0));
        let panel_x = (self.w - panel_w) * 0.5;
        (panel_x, panel_w, collapsed_w, collapsed_h, expanded_h)
    }

    pub fn recompute_hits(&mut self) {
        let (panel_x, panel_w, cw, ch, eh) = self.notch_geometry();
        let cx = self.w * 0.5;
        let mut h = NotchHits::default();

        h.collapsed_pill = Rect {
            x: cx - cw * 0.5 - 12.0,
            y: 0.0,
            w: cw + 104.0,
            h: ch + 10.0,
        };
        h.panel = Rect {
            x: panel_x,
            y: 0.0,
            w: panel_w,
            h: eh,
        };

        let hdr_h = (eh * 0.125).clamp(32.0, 42.0);
        let pad = 12.0;

        // Header buttons
        let close_sz = 22.0;
        h.btn_close = Rect {
            x: panel_x + panel_w - pad - close_sz,
            y: (hdr_h - close_sz) * 0.5,
            w: close_sz,
            h: close_sz,
        };
        h.tab_clips = Rect {
            x: panel_x + 126.0,
            y: (hdr_h - 22.0) * 0.5,
            w: 62.0,
            h: 22.0,
        };
        h.btn_bell = Rect {
            x: panel_x + 196.0,
            y: (hdr_h - 22.0) * 0.5,
            w: 26.0,
            h: 22.0,
        };
        h.tab_workspace = Rect {
            x: panel_x + panel_w - pad - close_sz - 148.0,
            y: (hdr_h - 22.0) * 0.5,
            w: 78.0,
            h: 22.0,
        };

        // 4 cards inside Workspace
        let cards_y = hdr_h + 6.0;
        let cards_h = (eh - cards_y - pad).max(80.0);
        let gap = 9.0;
        let inner_w = panel_w - pad * 2.0 - gap * 3.0;
        let w0 = inner_w * 0.285;
        let w1 = inner_w * 0.225;
        let w2 = inner_w * 0.240;
        let w3 = inner_w * 0.250;
        let x0 = panel_x + pad;
        let x1 = x0 + w0 + gap;
        let x2 = x1 + w1 + gap;
        let x3 = x2 + w2 + gap;

        // Task filter pills inside Card 0
        h.tab_tasks_todo = Rect {
            x: x0 + 10.0,
            y: cards_y + 32.0,
            w: 62.0,
            h: 22.0,
        };
        h.tab_tasks_done = Rect {
            x: x0 + 74.0,
            y: cards_y + 32.0,
            w: 86.0,
            h: 22.0,
        };

        // Task checkboxes & delete buttons inside Card 0
        for i in 0..3 {
            let ty = cards_y + 68.0 + (i as f32) * ((cards_h - 108.0) / 3.0).clamp(36.0, 52.0);
            h.task_checks[i] = Rect {
                x: x0 + 8.0,
                y: ty - 6.0,
                w: (w0 - 38.0).max(40.0),
                h: 32.0,
            };
            h.task_dels[i] = Rect {
                x: x0 + w0 - 26.0,
                y: ty - 2.0,
                w: 20.0,
                h: 22.0,
            };
        }
        h.btn_add_task = Rect {
            x: x0 + 10.0,
            y: cards_y + cards_h - 32.0,
            w: w0 - 20.0,
            h: 24.0,
        };

        // Timer controls inside Card 1
        let btn_y = cards_y + cards_h * 0.65;
        let toggle_w = (w1 * 0.58).max(66.0);
        let btn_h = 28.0;
        let btn_x = x1 + (w1 - toggle_w - 34.0) * 0.5;
        h.btn_timer_toggle = Rect {
            x: btn_x,
            y: btn_y,
            w: toggle_w,
            h: btn_h,
        };
        h.btn_timer_reset = Rect {
            x: btn_x + toggle_w + 6.0,
            y: btn_y,
            w: btn_h,
            h: btn_h,
        };
        h.btn_timer_preset = Rect {
            x: x1 + 14.0,
            y: btn_y + btn_h + 6.0,
            w: w1 - 28.0,
            h: 22.0,
        };

        // Notepad card (click opens real note editor!)
        h.card_notepad = Rect {
            x: x2,
            y: cards_y,
            w: w2,
            h: cards_h,
        };

        // Calendar interactive strip + week arrows + event delete + add button (Card 3)
        let col_w = (w3 - 34.0) / 7.0;
        h.cal_prev = Rect {
            x: x3 + 2.0,
            y: cards_y + 18.0,
            w: 16.0,
            h: 28.0,
        };
        h.cal_next = Rect {
            x: x3 + w3 - 18.0,
            y: cards_y + 18.0,
            w: 16.0,
            h: 28.0,
        };
        for d in 0..7 {
            h.cal_days[d] = Rect {
                x: x3 + 17.0 + (d as f32) * col_w,
                y: cards_y + 8.0,
                w: col_w,
                h: 38.0,
            };
        }
        let ev1_y = cards_y + 56.0;
        let ev1_h = (cards_h * 0.30).clamp(50.0, 68.0);
        h.cal_event_dels[0] = Rect {
            x: x3 + w3 - 28.0,
            y: ev1_y + 4.0,
            w: 18.0,
            h: 18.0,
        };
        let ev2_y = ev1_y + ev1_h + 8.0;
        h.cal_event_dels[1] = Rect {
            x: x3 + w3 - 28.0,
            y: ev2_y + 4.0,
            w: 18.0,
            h: 18.0,
        };
        h.btn_add_event = Rect {
            x: x3 + w3 - 76.0,
            y: cards_y + cards_h - 28.0,
            w: 66.0,
            h: 21.0,
        };

        // Clips clothesline hits (Tab 1)
        let clip_w = ((panel_w - pad * 2.0 - 44.0) / 3.0).min(210.0);
        let clip_h = (cards_h - 48.0).clamp(86.0, 145.0);
        let clip_y = cards_y + 16.0;
        let start_x = panel_x + pad + 12.0;
        for ci in 0..3 {
            let cx_card = start_x + (ci as f32) * (clip_w + 18.0);
            h.clip_cards[ci] = Rect {
                x: cx_card,
                y: clip_y,
                w: clip_w,
                h: clip_h,
            };
            h.clip_dels[ci] = Rect {
                x: cx_card + clip_w - 22.0,
                y: clip_y + 4.0,
                w: 18.0,
                h: 18.0,
            };
        }
        h.btn_add_clip = Rect {
            x: panel_x + panel_w - pad - 118.0,
            y: cards_y + cards_h - 26.0,
            w: 112.0,
            h: 22.0,
        };

        // Compact reminder toast rect under top center
        let tw = 294.0f32.min(self.w - 24.0);
        let th = 96.0;
        let tx = cx - tw * 0.5;
        h.toast_rect = Rect {
            x: tx,
            y: 0.0,
            w: tw,
            h: th,
        };
        h.btn_toast_snooze = Rect {
            x: tx + 86.0,
            y: th - 34.0,
            w: 76.0,
            h: 24.0,
        };
        h.btn_toast_done = Rect {
            x: tx + 170.0,
            y: th - 34.0,
            w: 84.0,
            h: 24.0,
        };

        // Cat hit region
        let (cat_x, cat_y, sc) = self.cat_pos_and_scale();
        h.cat_hit = Rect {
            x: cat_x - sc * 0.72,
            y: cat_y - sc * 0.95,
            w: sc * 1.65,
            h: sc * 1.75,
        };

        self.hits = h;
    }

    pub fn is_notch_expanded(&self) -> bool {
        self.notch_open > 0.55
    }

    pub fn active_notch_rect(&self) -> Rect {
        if self.params.notch_mode == 3 {
            return Rect::default();
        }
        if self.reminder_open && self.notch_open < 0.2 {
            return self.hits.toast_rect;
        }
        if self.notch_open > 0.25 {
            self.hits.panel
        } else {
            self.hits.collapsed_pill
        }
    }

    pub fn handle_mouse(&mut self, kind: i32, x: f32, y: f32) -> bool {
        if kind == 3 {
            self.pointer_active = false;
            self.notch_hover = false;
            return false;
        }
        self.pointer_x = x;
        self.pointer_y = y;
        self.pointer_active = true;

        // Track hover over collapsed notch OR expanded panel
        if self.params.notch_mode != 3 {
            let over_collapsed = self.hits.collapsed_pill.contains(x, y);
            let over_panel = self.notch_open > 0.20 && self.hits.panel.contains(x, y);
            self.notch_hover = over_collapsed || over_panel;
        } else {
            self.notch_hover = false;
        }

        if kind != 1 {
            return self.notch_hover;
        }

        // ---- Click handling (kind == 1) ----
        // 1. Reminder toast clicks
        if self.params.notch_mode != 3 && self.reminder_open && self.notch_open < 0.3 {
            if self.hits.btn_toast_snooze.contains(x, y) {
                self.reminder_open = false;
                self.timer_left_sec = 5.0 * 60.0;
                self.timer_running = true;
                return true;
            }
            if self.hits.btn_toast_done.contains(x, y) {
                self.reminder_open = false;
                return true;
            }
            if self.hits.toast_rect.contains(x, y) {
                self.reminder_open = false;
                self.notch_pinned = true;
                return true;
            }
        }

        // 2. Expanded notch bar interactive controls
        if self.params.notch_mode != 3 && self.notch_open > 0.45 && self.hits.panel.contains(x, y) {
            if self.hits.btn_close.contains(x, y) {
                self.notch_pinned = false;
                self.notch_hover = false;
                return true;
            }
            if self.hits.tab_workspace.contains(x, y) {
                self.notch_tab = 0;
                self.notch_pinned = true;
                self.recompute_hits();
                return true;
            }
            if self.hits.tab_clips.contains(x, y) {
                self.notch_tab = if self.notch_tab == 1 { 0 } else { 1 };
                self.notch_pinned = true;
                self.recompute_hits();
                return true;
            }
            if self.hits.btn_bell.contains(x, y) {
                self.notch_pinned = false;
                self.notch_hover = false;
                self.reminder_open = true;
                self.reminder_title = "Focus break".into();
                return true;
            }
            if self.notch_tab == 0 {
                // Task filter tabs ("To do" / "Completed")
                if self.hits.tab_tasks_todo.contains(x, y) {
                    self.task_filter = 0;
                    self.notch_pinned = true;
                    return true;
                }
                if self.hits.tab_tasks_done.contains(x, y) {
                    self.task_filter = 1;
                    self.notch_pinned = true;
                    return true;
                }
                // Task delete buttons & checkboxes (mapped to visible filtered tasks)
                let visible = self.visible_task_indices();
                for (slot, &real_idx) in visible.iter().enumerate() {
                    if self.hits.task_dels[slot].contains(x, y) {
                        self.tasks.remove(real_idx);
                        self.save_widgets();
                        self.notch_pinned = true;
                        return true;
                    }
                    if self.hits.task_checks[slot].contains(x, y) {
                        self.tasks[real_idx].done = !self.tasks[real_idx].done;
                        self.save_widgets();
                        self.notch_pinned = true;
                        return true;
                    }
                }
                if self.hits.btn_add_task.contains(x, y) {
                    self.push_event(r#"{"t":"catShorePrompt","kind":"addTask"}"#.into());
                    self.notch_pinned = true;
                    return true;
                }
                if self.hits.btn_timer_toggle.contains(x, y) {
                    self.timer_running = !self.timer_running;
                    if self.timer_left_sec <= 0.5 {
                        self.timer_left_sec = (TIMER_PRESETS_MIN[self.timer_preset_idx] * 60) as f32;
                    }
                    self.notch_pinned = true;
                    return true;
                }
                if self.hits.btn_timer_reset.contains(x, y) {
                    self.timer_running = false;
                    self.timer_left_sec = (TIMER_PRESETS_MIN[self.timer_preset_idx] * 60) as f32;
                    self.notch_pinned = true;
                    return true;
                }
                if self.hits.btn_timer_preset.contains(x, y) {
                    self.timer_preset_idx = (self.timer_preset_idx + 1) % TIMER_PRESETS_MIN.len();
                    self.timer_running = false;
                    self.timer_left_sec = (TIMER_PRESETS_MIN[self.timer_preset_idx] * 60) as f32;
                    self.save_widgets();
                    self.notch_pinned = true;
                    return true;
                }
                if self.hits.card_notepad.contains(x, y) {
                    if let Ok(payload) = serde_json::to_string(&serde_json::json!({
                        "t": "catShorePrompt",
                        "kind": "editNote",
                        "title": &self.note_title,
                        "text": &self.note_text,
                    })) {
                        self.push_event(payload);
                    }
                    self.notch_pinned = true;
                    return true;
                }
                // Calendar week navigation & day selection
                if self.hits.cal_prev.contains(x, y) {
                    self.cal_week_offset -= 7;
                    self.cal_selected_day_off -= 7;
                    self.notch_pinned = true;
                    return true;
                }
                if self.hits.cal_next.contains(x, y) {
                    self.cal_week_offset += 7;
                    self.cal_selected_day_off += 7;
                    self.notch_pinned = true;
                    return true;
                }
                for d in 0..7 {
                    if self.hits.cal_days[d].contains(x, y) {
                        self.cal_selected_day_off = self.cal_week_offset + d as i64;
                        self.notch_pinned = true;
                        return true;
                    }
                }
                let day_ev_indices = self.selected_day_event_indices();
                for (slot, &real_idx) in day_ev_indices.iter().enumerate() {
                    if self.hits.cal_event_dels[slot].contains(x, y) {
                        self.events.remove(real_idx);
                        self.save_widgets();
                        self.notch_pinned = true;
                        return true;
                    }
                }
                if self.hits.btn_add_event.contains(x, y) {
                    let date_str = self.selected_date().format("%Y-%m-%d").to_string();
                    if let Ok(payload) = serde_json::to_string(&serde_json::json!({
                        "t": "catShorePrompt",
                        "kind": "addEvent",
                        "date": date_str,
                    })) {
                        self.push_event(payload);
                    }
                    self.notch_pinned = true;
                    return true;
                }
            } else {
                // Tab 1: Clips clothesline interactions
                if self.hits.btn_add_clip.contains(x, y) {
                    self.push_event(r#"{"t":"catShorePrompt","kind":"pasteClip"}"#.into());
                    self.notch_pinned = true;
                    return true;
                }
                for ci in 0..self.clips.len().min(3) {
                    if self.hits.clip_dels[ci].contains(x, y) {
                        self.clips.remove(ci);
                        self.save_widgets();
                        self.notch_pinned = true;
                        return true;
                    }
                    if self.hits.clip_cards[ci].contains(x, y) {
                        if let Ok(payload) = serde_json::to_string(&serde_json::json!({
                            "t": "catShoreCopy",
                            "text": &self.clips[ci].body,
                        })) {
                            self.push_event(payload);
                        }
                        self.clip_copied_idx = Some(ci);
                        self.clip_copied_ttl = 1.8;
                        self.notch_pinned = true;
                        return true;
                    }
                }
            }
            self.notch_pinned = true;
            return true;
        }

        // 3. Collapsed notch click toggles pin open (unless mode == off/collapsed)
        if self.params.notch_mode != 3
            && self.params.notch_mode != 2
            && self.hits.collapsed_pill.contains(x, y)
        {
            self.reminder_open = false;
            self.notch_pinned = !self.notch_pinned;
            return true;
        }

        // Clicking outside collapses pinned panel cleanly
        if self.notch_pinned && self.params.notch_mode != 1 {
            self.notch_pinned = false;
        }

        // 4. Petting the cat or splashing water
        if self.hits.cat_hit.contains(x, y) {
            self.happy = 1.0;
            let (cx, cy, sc) = self.cat_pos_and_scale();
            self.add_ripple(cx, cy + sc * 0.25, 1.35);
            return true;
        }

        self.add_ripple(x, y, 0.95);
        false
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.sim_time += dt;

        // Tail gentle sway (faster when happy)
        self.tail_phase += dt * (2.3 + self.happy * 4.5) * self.params.water_speed.clamp(0.4, 1.6);
        self.happy = (self.happy - dt * 0.85).max(0.0);

        // Focus timer countdown + desktop alert
        if self.timer_running {
            self.timer_left_sec = (self.timer_left_sec - dt).max(0.0);
            if self.timer_left_sec <= 0.0 {
                self.timer_running = false;
                self.reminder_title = "Focus complete!".into();
                self.reminder_open = true;
                self.notch_pinned = false;
                self.push_event(
                    r#"{"t":"catShoreNotify","title":"DiyarDesk Odak Sayacı","body":"Odak süresi tamamlandı! Kısa bir mola zamanı."}"#
                        .into(),
                );
            }
        }
        if self.clip_copied_ttl > 0.0 {
            self.clip_copied_ttl = (self.clip_copied_ttl - dt).max(0.0);
            if self.clip_copied_ttl <= 0.0 {
                self.clip_copied_idx = None;
            }
        }

        // Smooth cat eye gaze tracking toward mouse cursor!
        let (cx, cy, sc) = self.cat_pos_and_scale();
        let eye_center_x = cx;
        let eye_center_y = cy - sc * 0.25;
        let (tx, ty) = if self.pointer_active {
            let dx = self.pointer_x - eye_center_x;
            let dy = self.pointer_y - eye_center_y;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let mag = (dist / (sc * 2.4)).clamp(0.0, 1.0);
            ((dx / dist) * mag, (dy / dist) * mag)
        } else {
            // Gentle idle curiosity looking up toward notch & around shoreline
            let a = self.sim_time * 0.45;
            (a.sin() * 0.35, -0.35 + (a * 0.7).cos() * 0.20)
        };
        let gaze_k = 1.0 - (-dt * 11.0).exp();
        self.gaze_x += (tx - self.gaze_x) * gaze_k;
        self.gaze_y += (ty - self.gaze_y) * gaze_k;

        // Natural periodic blinking
        if self.blink_phase >= 0.0 {
            self.blink_phase += dt / 0.22;
            if self.blink_phase >= 1.0 {
                self.blink_phase = -1.0;
                self.blink = 0.0;
                // Deterministic pseudo-random next blink interval (3.2..5.8s)
                let h = cpu_hash21(self.sim_time * 1.7, self.params.seed as f32);
                self.blink_timer = 3.2 + h * 2.6;
            } else {
                let s = (self.blink_phase * std::f32::consts::PI).sin();
                self.blink = s * s;
            }
        } else {
            self.blink_timer -= dt;
            if self.blink_timer <= 0.0 {
                self.blink_phase = 0.0;
            }
        }

        // Advance expanding water ripples
        for r in &mut self.ripples {
            r.age += dt;
        }
        self.ripples.retain(|r| r.age < 3.6);

        // Smooth top notch expand/collapse animation
        let want_open = match self.params.notch_mode {
            1 => true,                                  // pinned
            2 | 3 => false,                             // collapsed or off
            _ => self.notch_pinned || self.notch_hover, // auto
        };
        let target = if want_open { 1.0 } else { 0.0 };
        let open_k = 1.0 - (-dt * 5.2).exp();
        self.notch_open += (target - self.notch_open) * open_k;
        if (self.notch_open - target).abs() < 0.002 {
            self.notch_open = target;
        }
    }
}

// ============================================================================
// 4. OPENGL 3.3 CORE RENDERER (Shore + Cat + Collapsible Top Notch UI + Text)
// ============================================================================

// 5x7 crisp dot-matrix glyph bitmaps for '0'..'9' and ':'
const DOT_DIGITS: [[u8; 7]; 11] = [
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111], // 2
    [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110], // 3
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // 5
    [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // 7
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // 9
    [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000], // :
];

pub struct CatShoreRenderer {
    prog_scene: GLuint,
    prog_ui: GLuint,
    dummy_vao: GLuint,
    shore_tex: GLuint,

    // Cached scene uniforms
    u_shore_tex: GLint,
    u_res: GLint,
    u_time: GLint,
    u_water_speed: GLint,
    u_ripple_strength: GLint,
    u_tone: GLint,
    u_adaptive: GLint,
    u_cat_pos: GLint,
    u_cat_scale: GLint,
    u_gaze: GLint,
    u_blink: GLint,
    u_happy: GLint,
    u_tail_phase: GLint,
    u_click_count: GLint,
    u_clicks: GLint,
    u_icon_count: GLint,
    u_icons: GLint,

    // Cached UI uniforms
    ui_res: GLint,
    ui_rect: GLint,
    ui_kind: GLint,
    ui_color: GLint,
    ui_color2: GLint,
    ui_params: GLint,
    ui_params2: GLint,
}

impl CatShoreRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_scene = gl.compile_program(CAT_SHORE_VERT, CAT_SHORE_FRAG)?;
        let prog_ui = gl.compile_program(NOTCH_UI_VERT, NOTCH_UI_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let mut shore_tex = 0;
        (gl.GenTextures)(1, &mut shore_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, shore_tex);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            u_shore_tex: gl.uniform_loc(prog_scene, "u_shore_tex"),
            u_res: gl.uniform_loc(prog_scene, "u_res"),
            u_time: gl.uniform_loc(prog_scene, "u_time"),
            u_water_speed: gl.uniform_loc(prog_scene, "u_water_speed"),
            u_ripple_strength: gl.uniform_loc(prog_scene, "u_ripple_strength"),
            u_tone: gl.uniform_loc(prog_scene, "u_tone"),
            u_adaptive: gl.uniform_loc(prog_scene, "u_adaptive"),
            u_cat_pos: gl.uniform_loc(prog_scene, "u_cat_pos"),
            u_cat_scale: gl.uniform_loc(prog_scene, "u_cat_scale"),
            u_gaze: gl.uniform_loc(prog_scene, "u_gaze"),
            u_blink: gl.uniform_loc(prog_scene, "u_blink"),
            u_happy: gl.uniform_loc(prog_scene, "u_happy"),
            u_tail_phase: gl.uniform_loc(prog_scene, "u_tail_phase"),
            u_click_count: gl.uniform_loc(prog_scene, "u_click_count"),
            u_clicks: gl.uniform_loc(prog_scene, "u_clicks"),
            u_icon_count: gl.uniform_loc(prog_scene, "u_icon_count"),
            u_icons: gl.uniform_loc(prog_scene, "u_icons"),
            ui_res: gl.uniform_loc(prog_ui, "u_res"),
            ui_rect: gl.uniform_loc(prog_ui, "u_rect"),
            ui_kind: gl.uniform_loc(prog_ui, "u_kind"),
            ui_color: gl.uniform_loc(prog_ui, "u_color"),
            ui_color2: gl.uniform_loc(prog_ui, "u_color2"),
            ui_params: gl.uniform_loc(prog_ui, "u_params"),
            ui_params2: gl.uniform_loc(prog_ui, "u_params2"),
            prog_scene,
            prog_ui,
            dummy_vao,
            shore_tex,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.shore_tex != 0 {
            (gl.DeleteTextures)(1, &self.shore_tex);
            self.shore_tex = 0;
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
        if self.prog_scene != 0 {
            (gl.DeleteProgram)(self.prog_scene);
            self.prog_scene = 0;
        }
        if self.prog_ui != 0 {
            (gl.DeleteProgram)(self.prog_ui);
            self.prog_ui = 0;
        }
    }

    unsafe fn ensure_shore_tex(&mut self, gl: &GlFns, cs: &mut CatShoreState) {
        if !cs.shore_dirty && cs.shore_seed_built == cs.params.seed {
            return;
        }
        let tw = 960usize;
        let th = 540usize;
        let rgba = build_pebble_shore_texture(tw, th, cs.params.seed);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.shore_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            tw as GLsizei,
            th as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            rgba.as_ptr() as *const c_void,
        );
        cs.shore_seed_built = cs.params.seed;
        cs.shore_dirty = false;
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_ui_box(
        &self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radii: [f32; 4], // (tr, br, tl, bl)
        fill: [f32; 4],
        border: [f32; 4],
        border_w: f32,
        shadow_pad: f32,
    ) {
        if w <= 0.5 || h <= 0.5 || fill[3] <= 0.002 && border[3] <= 0.002 {
            return;
        }
        (gl.UseProgram)(self.prog_ui);
        (gl.Uniform2f)(self.ui_res, css_w, css_h);
        (gl.Uniform4f)(
            self.ui_rect,
            x - shadow_pad,
            y - shadow_pad,
            w + shadow_pad * 2.0,
            h + shadow_pad * 2.0,
        );
        (gl.Uniform1i)(self.ui_kind, 0);
        (gl.Uniform4f)(self.ui_color, fill[0], fill[1], fill[2], fill[3]);
        (gl.Uniform4f)(self.ui_color2, border[0], border[1], border[2], border[3]);
        (gl.Uniform4f)(self.ui_params, radii[0], radii[1], radii[2], radii[3]);
        (gl.Uniform4f)(self.ui_params2, border_w, shadow_pad, 0.0, 0.0);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_ui_circle(
        &self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        cx: f32,
        cy: f32,
        r: f32,
        fill: [f32; 4],
        stroke: [f32; 4],
        stroke_w: f32,
        checked: bool,
    ) {
        (gl.UseProgram)(self.prog_ui);
        (gl.Uniform2f)(self.ui_res, css_w, css_h);
        (gl.Uniform4f)(self.ui_rect, cx - r - 1.0, cy - r - 1.0, r * 2.0 + 2.0, r * 2.0 + 2.0);
        (gl.Uniform1i)(self.ui_kind, 1);
        (gl.Uniform4f)(self.ui_color, fill[0], fill[1], fill[2], fill[3]);
        (gl.Uniform4f)(self.ui_color2, stroke[0], stroke[1], stroke[2], stroke[3]);
        (gl.Uniform4f)(self.ui_params, stroke_w, if checked { 1.0 } else { 0.0 }, 0.0, 0.0);
        (gl.Uniform4f)(self.ui_params2, 0.0, 0.0, 0.0, 0.0);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);
    }

    unsafe fn draw_ui_cat_badge(
        &self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        x: f32,
        y: f32,
        sz: f32,
        alpha: f32,
    ) {
        (gl.UseProgram)(self.prog_ui);
        (gl.Uniform2f)(self.ui_res, css_w, css_h);
        (gl.Uniform4f)(self.ui_rect, x, y, sz, sz);
        (gl.Uniform1i)(self.ui_kind, 3);
        (gl.Uniform4f)(self.ui_color, 0.96, 0.80, 0.72, alpha);
        (gl.Uniform4f)(self.ui_color2, 0.0, 0.0, 0.0, 0.0);
        (gl.Uniform4f)(self.ui_params, 0.0, 0.0, 0.0, 0.0);
        (gl.Uniform4f)(self.ui_params2, 0.0, 0.0, 0.0, 0.0);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);
    }

    unsafe fn draw_dot_matrix_time(
        &self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        cx: f32,
        cy: f32,
        mins: u32,
        secs: u32,
        cell: f32,
        col: [f32; 4],
    ) {
        let glyphs = [
            ((mins / 10) % 10) as usize,
            (mins % 10) as usize,
            10usize, // ':'
            ((secs / 10) % 10) as usize,
            (secs % 10) as usize,
        ];
        let glyph_w = 5.0 * cell;
        let gap = cell * 1.45;
        let colon_w = 3.0 * cell;
        let total_w = 4.0 * glyph_w + colon_w + 4.0 * gap;
        let total_h = 7.0 * cell;
        let mut pen_x = cx - total_w * 0.5;
        let start_y = cy - total_h * 0.5;
        let dot_r = cell * 0.38;

        for &g in &glyphs {
            let rows = DOT_DIGITS[g];
            let is_colon = g == 10;
            let x_off = if is_colon { -cell } else { 0.0 };
            for (r_idx, &bits) in rows.iter().enumerate() {
                for c_idx in 0..5 {
                    if (bits & (1 << (4 - c_idx))) != 0 {
                        let dx = pen_x + x_off + (c_idx as f32 + 0.5) * cell;
                        let dy = start_y + (r_idx as f32 + 0.5) * cell;
                        self.draw_ui_circle(gl, css_w, css_h, dx, dy, dot_r, col, [0.0; 4], 0.0, false);
                    }
                }
            }
            pen_x += (if is_colon { colon_w } else { glyph_w }) + gap;
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState, text: &mut TextRenderer) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.5);

        let cs = &mut st.cat_shore;
        cs.resize(css_w, css_h, dpi);
        self.ensure_shore_tex(gl, cs);

        // 1. Render full painterly pebble shoreline + ripples + shadow + black cat
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.UseProgram)(self.prog_scene);

        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.shore_tex);
        (gl.Uniform1i)(self.u_shore_tex, 0);
        (gl.Uniform2f)(self.u_res, css_w, css_h);
        (gl.Uniform1f)(self.u_time, cs.sim_time);
        (gl.Uniform1f)(self.u_water_speed, cs.params.water_speed);
        (gl.Uniform1f)(self.u_ripple_strength, cs.params.ripple_strength);
        (gl.Uniform1i)(self.u_tone, cs.params.shore_tone as GLint);
        (gl.Uniform1i)(self.u_adaptive, if cs.params.adaptive { 1 } else { 0 });

        let (cat_x, cat_y, cat_sc) = cs.cat_pos_and_scale();
        (gl.Uniform2f)(self.u_cat_pos, cat_x, cat_y);
        (gl.Uniform1f)(self.u_cat_scale, cat_sc);
        (gl.Uniform2f)(self.u_gaze, cs.gaze_x, cs.gaze_y);
        (gl.Uniform1f)(self.u_blink, cs.blink);
        (gl.Uniform1f)(self.u_happy, cs.happy);
        (gl.Uniform1f)(self.u_tail_phase, cs.tail_phase);

        let click_cnt = cs.ripples.len().min(8);
        (gl.Uniform1i)(self.u_click_count, click_cnt as GLint);
        if click_cnt > 0 {
            let mut cbuf = [0.0f32; 8 * 4];
            for (i, r) in cs.ripples.iter().take(8).enumerate() {
                cbuf[i * 4] = r.x;
                cbuf[i * 4 + 1] = r.y;
                cbuf[i * 4 + 2] = r.age;
                cbuf[i * 4 + 3] = r.strength;
            }
            (gl.Uniform4fv)(self.u_clicks, click_cnt as GLsizei, cbuf.as_ptr());
        }

        let ic_cnt = if cs.icons_visible { cs.icons.len().min(24) } else { 0 };
        (gl.Uniform1i)(self.u_icon_count, ic_cnt as GLint);
        if ic_cnt > 0 {
            let mut ibuf = [0.0f32; 24 * 4];
            for (i, ic) in cs.icons.iter().take(24).enumerate() {
                ibuf[i * 4] = ic.x + ic.w * 0.5;
                ibuf[i * 4 + 1] = ic.y + ic.h * 0.5;
                ibuf[i * 4 + 2] = ic.w * 0.5;
                ibuf[i * 4 + 3] = ic.h * 0.5;
            }
            (gl.Uniform4fv)(self.u_icons, ic_cnt as GLsizei, ibuf.as_ptr());
        }

        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // 2. Render Collapsible Top Notch Bar (strict destination-alpha-preserving blending!)
        if cs.params.notch_mode != 3 {
            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
            self.render_notch_ui(gl, cs, text, css_w, css_h, dpi);
            (gl.Disable)(GL_BLEND);
        }
    }

    unsafe fn render_notch_ui(
        &self,
        gl: &GlFns,
        cs: &CatShoreState,
        text: &mut TextRenderer,
        css_w: f32,
        css_h: f32,
        dpi: f32,
    ) {
        let (panel_x, panel_w, cw, ch, eh) = cs.notch_geometry();
        let cx = css_w * 0.5;
        let raw_open = cs.notch_open.clamp(0.0, 1.0);
        let open = raw_open * raw_open * (3.0 - 2.0 * raw_open);

        let mins = (cs.timer_left_sec.ceil() as u32) / 60;
        let secs = (cs.timer_left_sec.ceil() as u32) % 60;
        let timer_str = format!("{mins:02}:{secs:02}");

        text.begin();

        // Interpolate main black notch shell between collapsed pill and expanded panel
        let cur_w = cw + (panel_w - cw) * open;
        let cur_h = ch + (eh - ch) * open;
        let cur_x = cx - cur_w * 0.5;
        let bot_rad = 13.0 + 11.0 * open;

        // Compact reminder toast mode (when collapsed + reminder_open)
        if cs.reminder_open && open < 0.25 {
            let tr = cs.hits.toast_rect;
            self.draw_ui_box(
                gl,
                css_w,
                css_h,
                tr.x,
                0.0,
                tr.w,
                tr.h,
                [0.0, 20.0, 0.0, 20.0],
                [0.04, 0.045, 0.055, 0.97],
                [1.0, 1.0, 1.0, 0.09],
                1.0,
                14.0,
            );
            // Peach mini cat avatar
            self.draw_ui_cat_badge(gl, css_w, css_h, tr.x + 14.0, 18.0, 56.0, 1.0);
            // Tiny orange bell dot on avatar corner
            self.draw_ui_circle(
                gl,
                css_w,
                css_h,
                tr.x + 68.0,
                70.0,
                9.0,
                [0.95, 0.62, 0.34, 1.0],
                [0.05, 0.05, 0.06, 1.0],
                2.0,
                false,
            );
            text.push(
                gl,
                &cs.reminder_title,
                tr.x + 86.0,
                35.0,
                14.5,
                dpi,
                true,
                [0.97, 0.97, 0.98, 1.0],
                0,
                Some(tr.w - 102.0),
            );
            let now = Local::now();
            let sub = format!("{:02}:{:02}", now.hour(), now.minute());
            text.push(
                gl,
                &sub,
                tr.x + 86.0,
                53.0,
                11.5,
                dpi,
                false,
                [0.64, 0.66, 0.70, 1.0],
                0,
                None,
            );
            // +5 min button
            let sn = cs.hits.btn_toast_snooze;
            self.draw_ui_box(
                gl,
                css_w,
                css_h,
                sn.x,
                sn.y,
                sn.w,
                sn.h,
                [12.0; 4],
                [0.16, 0.17, 0.20, 1.0],
                [1.0, 1.0, 1.0, 0.10],
                1.0,
                0.0,
            );
            text.push(
                gl,
                "+5 min",
                sn.x + sn.w * 0.5,
                sn.y + 16.0,
                11.0,
                dpi,
                true,
                [0.88, 0.89, 0.92, 1.0],
                1,
                None,
            );
            // Orange Done button
            let dn = cs.hits.btn_toast_done;
            self.draw_ui_box(
                gl,
                css_w,
                css_h,
                dn.x,
                dn.y,
                dn.w,
                dn.h,
                [12.0; 4],
                [0.95, 0.60, 0.30, 1.0],
                [0.0; 4],
                0.0,
                0.0,
            );
            text.push(
                gl,
                "Done",
                dn.x + dn.w * 0.5,
                dn.y + 16.0,
                11.5,
                dpi,
                true,
                [0.12, 0.08, 0.04, 1.0],
                1,
                None,
            );
            text.flush(gl, css_w, css_h);
            return;
        }

        // Main black notch container
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            cur_x,
            0.0,
            cur_w,
            cur_h,
            [0.0, bot_rad, 0.0, bot_rad],
            [0.035, 0.038, 0.046, 0.96],
            [1.0, 1.0, 1.0, 0.09],
            1.0,
            8.0 + 10.0 * open,
        );

        // Collapsed state details (camera dot + mini live timer capsule right beside notch!)
        if open < 0.35 {
            let ca = 1.0 - open / 0.35;
            // Subtle lens sensor dot inside notch
            self.draw_ui_circle(
                gl,
                css_w,
                css_h,
                cx,
                ch * 0.48,
                3.6,
                [0.12, 0.14, 0.19, ca],
                [0.24, 0.30, 0.40, 0.45 * ca],
                1.0,
                false,
            );
            // Tiny cat eyes peek inside collapsed notch
            self.draw_ui_cat_badge(gl, css_w, css_h, cur_x + 12.0, (ch - 18.0) * 0.5, 18.0, 0.92 * ca);

            // Sleek side timer pill right beside the notch (just like frames 10, 14, 15!)
            let pill_x = cur_x + cur_w + 10.0;
            let pill_w = 72.0;
            let pill_h = (ch - 6.0).max(20.0);
            let pill_y = 3.0;
            self.draw_ui_box(
                gl,
                css_w,
                css_h,
                pill_x,
                pill_y,
                pill_w,
                pill_h,
                [pill_h * 0.5; 4],
                [0.04, 0.045, 0.055, 0.92 * ca],
                [1.0, 1.0, 1.0, 0.12 * ca],
                1.0,
                4.0,
            );
            let pill_text = if cs.timer_running {
                timer_str.clone()
            } else {
                let now = Local::now();
                format!("{:02}:{:02}", now.hour(), now.minute())
            };
            text.push(
                gl,
                &pill_text,
                pill_x + pill_w * 0.5,
                pill_y + pill_h * 0.70,
                11.5,
                dpi,
                true,
                [0.88, 0.80, 0.62, ca],
                1,
                None,
            );
            text.flush(gl, css_w, css_h);
            return;
        }

        // Expanded panel content (fades in cleanly once open > 0.35)
        let alpha = ((open - 0.35) / 0.65).clamp(0.0, 1.0);
        let hdr_h = (eh * 0.125).clamp(32.0, 42.0);
        let pad = 12.0;

        // ---- Header Row ----
        self.draw_ui_cat_badge(gl, css_w, css_h, panel_x + pad, (hdr_h - 22.0) * 0.5, 22.0, alpha);
        text.push(
            gl,
            "DiyarDesk",
            panel_x + pad + 28.0,
            hdr_h * 0.65,
            13.0,
            dpi,
            true,
            [0.96, 0.97, 0.99, alpha],
            0,
            None,
        );

        // Clips pill button
        let tc = cs.hits.tab_clips;
        let clips_active = cs.notch_tab == 1;
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            tc.x,
            tc.y,
            tc.w,
            tc.h,
            [11.0; 4],
            if clips_active {
                [0.95, 0.96, 0.98, alpha]
            } else {
                [0.14, 0.15, 0.17, alpha]
            },
            [1.0, 1.0, 1.0, 0.10 * alpha],
            1.0,
            0.0,
        );
        text.push(
            gl,
            "Clips",
            tc.x + tc.w * 0.5,
            tc.y + 15.0,
            11.0,
            dpi,
            true,
            if clips_active {
                [0.08, 0.09, 0.11, alpha]
            } else {
                [0.82, 0.84, 0.88, alpha]
            },
            1,
            None,
        );

        // Bell reminder button
        let tb = cs.hits.btn_bell;
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            tb.x,
            tb.y,
            tb.w,
            tb.h,
            [11.0; 4],
            [0.14, 0.15, 0.17, alpha],
            [1.0, 1.0, 1.0, 0.09 * alpha],
            1.0,
            0.0,
        );
        self.draw_ui_circle(
            gl,
            css_w,
            css_h,
            tb.x + tb.w * 0.5,
            tb.y + tb.h * 0.5,
            4.2,
            [0.95, 0.66, 0.34, alpha],
            [0.0; 4],
            0.0,
            false,
        );

        // Right header tabs: Workspace pill + live clock + X close button
        let tws = cs.hits.tab_workspace;
        let ws_active = cs.notch_tab == 0;
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            tws.x,
            tws.y,
            tws.w,
            tws.h,
            [11.0; 4],
            if ws_active {
                [0.20, 0.21, 0.24, alpha]
            } else {
                [0.10, 0.11, 0.13, alpha]
            },
            [1.0, 1.0, 1.0, 0.10 * alpha],
            1.0,
            0.0,
        );
        text.push(
            gl,
            "Workspace",
            tws.x + tws.w * 0.5,
            tws.y + 15.0,
            11.0,
            dpi,
            true,
            [0.92, 0.94, 0.97, alpha],
            1,
            None,
        );

        let now = Local::now();
        let clock_str = format!("{:02}:{:02}", now.hour(), now.minute());
        text.push(
            gl,
            &clock_str,
            cs.hits.btn_close.x - 12.0,
            hdr_h * 0.65,
            11.5,
            dpi,
            false,
            [0.70, 0.72, 0.76, alpha],
            2,
            None,
        );

        let bc = cs.hits.btn_close;
        self.draw_ui_circle(
            gl,
            css_w,
            css_h,
            bc.x + bc.w * 0.5,
            bc.y + bc.h * 0.5,
            bc.w * 0.5,
            [0.15, 0.16, 0.19, alpha],
            [1.0, 1.0, 1.0, 0.10 * alpha],
            1.0,
            false,
        );
        text.push(
            gl,
            "x",
            bc.x + bc.w * 0.5,
            bc.y + 14.5,
            11.5,
            dpi,
            true,
            [0.85, 0.87, 0.90, alpha],
            1,
            None,
        );

        let cards_y = hdr_h + 6.0;
        let cards_h = (eh - cards_y - pad).max(80.0);

        if cs.notch_tab == 1 {
            // ---- TAB 1: REAL INTERACTIVE CLIPS CLOTHESLINE ----
            (gl.UseProgram)(self.prog_ui);
            (gl.Uniform2f)(self.ui_res, css_w, css_h);
            (gl.Uniform4f)(self.ui_rect, panel_x + pad, cards_y, panel_w - pad * 2.0, 26.0);
            (gl.Uniform1i)(self.ui_kind, 2);
            (gl.Uniform4f)(self.ui_color, 0.78, 0.74, 0.62, 0.85 * alpha);
            (gl.Uniform4f)(self.ui_color2, 0.0, 0.0, 0.0, 0.0);
            (gl.Uniform4f)(self.ui_params, 7.0, 1.8, 0.0, 0.0);
            (gl.Uniform4f)(self.ui_params2, 0.0, 0.0, 0.0, 0.0);
            (gl.BindVertexArray)(self.dummy_vao);
            (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);

            let clip_bgs: [[f32; 4]; 3] = [
                [0.95, 0.96, 0.98, alpha],
                [0.93, 0.90, 0.62, alpha],
                [0.86, 0.92, 0.96, alpha],
            ];
            if cs.clips.is_empty() {
                text.push(
                    gl,
                    "No clips yet — click + Paste Clip to pin clipboard text",
                    panel_x + panel_w * 0.5,
                    cards_y + cards_h * 0.48,
                    12.0,
                    dpi,
                    false,
                    [0.72, 0.75, 0.80, 0.90 * alpha],
                    1,
                    None,
                );
            } else {
                for (ci, clip) in cs.clips.iter().take(3).enumerate() {
                    let rc = cs.hits.clip_cards[ci];
                    let sway = (cs.sim_time * 1.4 + ci as f32 * 1.7).sin() * 1.8;
                    let cx_card = rc.x + sway;
                    let bg = clip_bgs[clip.color_idx as usize % 3];
                    let is_copied = cs.clip_copied_idx == Some(ci) && cs.clip_copied_ttl > 0.0;
                    self.draw_ui_box(
                        gl,
                        css_w,
                        css_h,
                        cx_card,
                        rc.y,
                        rc.w,
                        rc.h,
                        [6.0; 4],
                        bg,
                        if is_copied {
                            [0.20, 0.78, 0.56, alpha]
                        } else {
                            [0.0; 4]
                        },
                        if is_copied { 2.0 } else { 0.0 },
                        4.0,
                    );
                    // Wooden pegs
                    self.draw_ui_box(gl, css_w, css_h, cx_card + 14.0, rc.y - 10.0, 5.0, 14.0, [2.0; 4], [0.85, 0.80, 0.68, alpha], [0.0; 4], 0.0, 0.0);
                    self.draw_ui_box(gl, css_w, css_h, cx_card + rc.w - 19.0, rc.y - 10.0, 5.0, 14.0, [2.0; 4], [0.85, 0.80, 0.68, alpha], [0.0; 4], 0.0, 0.0);
                    // Delete x button
                    let bd = cs.hits.clip_dels[ci];
                    text.push(gl, "x", bd.x + sway + 9.0, bd.y + 13.0, 10.0, dpi, true, [0.42, 0.44, 0.48, 0.80 * alpha], 1, None);

                    text.push(gl, &clip.title, cx_card + 11.0, rc.y + 22.0, 11.5, dpi, true, [0.14, 0.15, 0.18, alpha], 0, Some(rc.w - 34.0));
                    for (li, line) in wrap_lines(&clip.body, 24, 4).iter().enumerate() {
                        text.push(gl, line, cx_card + 11.0, rc.y + 42.0 + li as f32 * 15.5, 10.2, dpi, false, [0.24, 0.26, 0.30, alpha], 0, Some(rc.w - 20.0));
                    }
                    let footer = if is_copied { "Copied!" } else { clip.time_str.as_str() };
                    let fcol = if is_copied {
                        [0.35, 0.92, 0.70, alpha]
                    } else {
                        [0.58, 0.61, 0.66, alpha]
                    };
                    text.push(gl, footer, cx_card + rc.w * 0.5, rc.y + rc.h + 15.0, 10.0, dpi, is_copied, fcol, 1, None);
                }
            }

            // Bottom-right "+ Paste Clip" button
            let ba = cs.hits.btn_add_clip;
            self.draw_ui_box(
                gl,
                css_w,
                css_h,
                ba.x,
                ba.y,
                ba.w,
                ba.h,
                [11.0; 4],
                [0.20, 0.24, 0.30, alpha],
                [1.0, 1.0, 1.0, 0.16 * alpha],
                1.0,
                0.0,
            );
            text.push(
                gl,
                "+ Paste Clip",
                ba.x + ba.w * 0.5,
                ba.y + 14.5,
                10.2,
                dpi,
                true,
                [0.92, 0.95, 0.98, alpha],
                1,
                None,
            );

            text.flush(gl, css_w, css_h);
            return;
        }

        // ---- TAB 0: 4 PASTEL WORKSPACE CARDS (exact match to frames 2, 4, 7!) ----
        let gap = 9.0;
        let inner_w = panel_w - pad * 2.0 - gap * 3.0;
        let w0 = inner_w * 0.285;
        let w1 = inner_w * 0.225;
        let w2 = inner_w * 0.240;
        let w3 = inner_w * 0.250;
        let x0 = panel_x + pad;
        let x1 = x0 + w0 + gap;
        let x2 = x1 + w1 + gap;
        let x3 = x2 + w2 + gap;
        let crad = 15.0;

        // === CARD 1: SAGE GREEN — Today's tasks ===
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            x0,
            cards_y,
            w0,
            cards_h,
            [crad; 4],
            [0.62, 0.72, 0.63, alpha],
            [1.0, 1.0, 1.0, 0.14 * alpha],
            1.0,
            0.0,
        );
        text.push(
            gl,
            "Today's tasks",
            x0 + 13.0,
            cards_y + 24.0,
            13.5,
            dpi,
            true,
            [0.10, 0.15, 0.11, alpha],
            0,
            None,
        );
        let date_short = format!("{} {}", now.day(), month_short(now.month()));
        text.push(
            gl,
            &date_short,
            x0 + w0 - 12.0,
            cards_y + 23.0,
            10.5,
            dpi,
            false,
            [0.18, 0.25, 0.19, alpha],
            2,
            None,
        );

        let todo_cnt = cs.tasks.iter().filter(|t| !t.done).count();
        let done_cnt = cs.tasks.iter().filter(|t| t.done).count();
        let todo_active = cs.task_filter == 0;
        // Active filter pill highlight
        let pill_r = if todo_active { cs.hits.tab_tasks_todo } else { cs.hits.tab_tasks_done };
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            pill_r.x,
            pill_r.y + 1.0,
            pill_r.w,
            19.0,
            [9.5; 4],
            [0.74, 0.82, 0.74, alpha],
            [0.0; 4],
            0.0,
            0.0,
        );
        text.push(
            gl,
            &format!("To do {todo_cnt}"),
            cs.hits.tab_tasks_todo.x + cs.hits.tab_tasks_todo.w * 0.5,
            cards_y + 47.0,
            10.0,
            dpi,
            todo_active,
            if todo_active { [0.10, 0.16, 0.11, alpha] } else { [0.20, 0.28, 0.21, 0.85 * alpha] },
            1,
            None,
        );
        text.push(
            gl,
            &format!("Completed {done_cnt}"),
            cs.hits.tab_tasks_done.x + cs.hits.tab_tasks_done.w * 0.5,
            cards_y + 47.0,
            10.0,
            dpi,
            !todo_active,
            if !todo_active { [0.10, 0.16, 0.11, alpha] } else { [0.20, 0.28, 0.21, 0.85 * alpha] },
            1,
            None,
        );

        let vis_tasks = cs.visible_task_indices();
        if vis_tasks.is_empty() {
            text.push(
                gl,
                if todo_active { "All done!" } else { "No completed tasks" },
                x0 + w0 * 0.5,
                cards_y + cards_h * 0.52,
                11.0,
                dpi,
                false,
                [0.20, 0.28, 0.21, 0.78 * alpha],
                1,
                None,
            );
        }
        for (slot, &real_idx) in vis_tasks.iter().enumerate() {
            let task = &cs.tasks[real_idx];
            let r = cs.hits.task_checks[slot];
            let dr = cs.hits.task_dels[slot];
            let cy = r.y + 14.0;
            self.draw_ui_circle(
                gl,
                css_w,
                css_h,
                x0 + 21.0,
                cy,
                7.2,
                if task.done {
                    [0.22, 0.36, 0.25, alpha]
                } else {
                    [0.0; 4]
                },
                if task.done {
                    [0.92, 0.96, 0.92, alpha]
                } else {
                    [0.14, 0.20, 0.15, 0.85 * alpha]
                },
                1.5,
                task.done,
            );
            let txt_col = if task.done {
                [0.24, 0.32, 0.25, 0.65 * alpha]
            } else {
                [0.10, 0.15, 0.11, alpha]
            };
            text.push(
                gl,
                &task.title,
                x0 + 34.0,
                cy + 3.0,
                11.8,
                dpi,
                !task.done,
                txt_col,
                0,
                Some((w0 - 62.0).max(32.0)),
            );
            text.push(
                gl,
                &task.meta,
                x0 + 34.0,
                cy + 17.5,
                9.5,
                dpi,
                false,
                [0.22, 0.30, 0.23, 0.82 * alpha],
                0,
                Some((w0 - 62.0).max(32.0)),
            );
            text.push(
                gl,
                "x",
                dr.x + dr.w * 0.5,
                cy + 4.0,
                10.0,
                dpi,
                true,
                [0.20, 0.28, 0.21, 0.68 * alpha],
                1,
                None,
            );
        }

        // Bottom "+ Add a task..." bar
        let add_r = cs.hits.btn_add_task;
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            add_r.x,
            add_r.y,
            add_r.w,
            add_r.h,
            [9.0; 4],
            [0.54, 0.65, 0.55, alpha],
            [0.0; 4],
            0.0,
            0.0,
        );
        text.push(
            gl,
            "+ Add a task...",
            add_r.x + 10.0,
            add_r.y + 16.0,
            10.5,
            dpi,
            false,
            [0.16, 0.23, 0.17, 0.88 * alpha],
            0,
            None,
        );

        // === CARD 2: SOFT LAVENDER — Dot-Matrix Focus Timer ===
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            x1,
            cards_y,
            w1,
            cards_h,
            [crad; 4],
            [0.70, 0.65, 0.78, alpha],
            [1.0, 1.0, 1.0, 0.14 * alpha],
            1.0,
            0.0,
        );
        let focus_title = cs
            .tasks
            .iter()
            .find(|t| !t.done)
            .map(|t| t.title.as_str())
            .unwrap_or("Focus session");
        text.push(
            gl,
            focus_title,
            x1 + w1 * 0.5,
            cards_y + 24.0,
            11.0,
            dpi,
            false,
            [0.20, 0.16, 0.28, alpha],
            1,
            Some(w1 - 22.0),
        );

        // Authentic dot-matrix LED countdown digits!
        let dot_cell = (w1 / 34.0).clamp(3.6, 5.4);
        self.draw_dot_matrix_time(
            gl,
            css_w,
            css_h,
            x1 + w1 * 0.5,
            cards_y + cards_h * 0.39,
            mins,
            secs,
            dot_cell,
            [0.20, 0.16, 0.27, alpha],
        );
        let status_lbl = if cs.timer_running { "Focusing" } else { "Ready" };
        text.push(
            gl,
            status_lbl,
            x1 + w1 * 0.5,
            cards_y + cards_h * 0.54,
            10.5,
            dpi,
            false,
            [0.28, 0.23, 0.37, 0.88 * alpha],
            1,
            None,
        );

        // Start / Pause pill button
        let bt = cs.hits.btn_timer_toggle;
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            bt.x,
            bt.y,
            bt.w,
            bt.h,
            [bt.h * 0.5; 4],
            [0.48, 0.43, 0.57, alpha],
            [0.0; 4],
            0.0,
            0.0,
        );
        text.push(
            gl,
            if cs.timer_running { "Pause" } else { "Start" },
            bt.x + bt.w * 0.5,
            bt.y + 18.0,
            11.5,
            dpi,
            true,
            [0.97, 0.96, 0.99, alpha],
            1,
            None,
        );

        // Reset checkmark circle button
        let br = cs.hits.btn_timer_reset;
        self.draw_ui_circle(
            gl,
            css_w,
            css_h,
            br.x + br.w * 0.5,
            br.y + br.h * 0.5,
            br.w * 0.5,
            [0.54, 0.49, 0.64, alpha],
            [0.95, 0.94, 0.98, alpha],
            0.0,
            true,
        );

        let bp = cs.hits.btn_timer_preset;
        text.push(
            gl,
            &format!("Set time ({}m)", TIMER_PRESETS_MIN[cs.timer_preset_idx]),
            bp.x + bp.w * 0.5,
            bp.y + 15.0,
            10.0,
            dpi,
            false,
            [0.24, 0.19, 0.33, 0.90 * alpha],
            1,
            None,
        );

        // === CARD 3: WARM OLIVE — Real Editable Notepad ===
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            x2,
            cards_y,
            w2,
            cards_h,
            [crad; 4],
            [0.71, 0.72, 0.47, alpha],
            [1.0, 1.0, 1.0, 0.14 * alpha],
            1.0,
            0.0,
        );
        text.push(
            gl,
            &cs.note_title,
            x2 + 13.0,
            cards_y + 24.0,
            13.0,
            dpi,
            true,
            [0.16, 0.17, 0.08, alpha],
            0,
            Some(w2 - 58.0),
        );
        text.push(
            gl,
            "Edit",
            x2 + w2 - 12.0,
            cards_y + 23.0,
            9.8,
            dpi,
            true,
            [0.24, 0.26, 0.13, 0.88 * alpha],
            2,
            None,
        );
        let full_date = format!("{} {} {}", now.day(), month_short(now.month()), now.year());
        text.push(
            gl,
            &full_date,
            x2 + 13.0,
            cards_y + 44.0,
            10.0,
            dpi,
            false,
            [0.26, 0.28, 0.15, 0.85 * alpha],
            0,
            None,
        );
        let max_chars = ((w2 - 26.0) / 6.2).clamp(16.0, 38.0) as usize;
        let max_lines = (((cards_h - 86.0) / 17.5).floor() as usize).clamp(3, 6);
        for (li, line) in wrap_lines(&cs.note_text, max_chars, max_lines).iter().enumerate() {
            text.push(
                gl,
                line,
                x2 + 13.0,
                cards_y + 70.0 + li as f32 * 17.5,
                11.0,
                dpi,
                false,
                [0.16, 0.17, 0.09, alpha],
                0,
                Some(w2 - 22.0),
            );
        }
        let wcount = cs.note_text.split_whitespace().count();
        text.push(
            gl,
            &format!("{wcount} words"),
            x2 + 13.0,
            cards_y + cards_h - 13.0,
            10.0,
            dpi,
            false,
            [0.25, 0.27, 0.14, 0.85 * alpha],
            0,
            None,
        );

        // === CARD 4: POWDER BLUE — Interactive Calendar & Real Schedule ===
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            x3,
            cards_y,
            w3,
            cards_h,
            [crad; 4],
            [0.61, 0.70, 0.78, alpha],
            [1.0, 1.0, 1.0, 0.14 * alpha],
            1.0,
            0.0,
        );

        // Week nav arrows (< / >) + interactive 7-day strip
        text.push(gl, "<", cs.hits.cal_prev.x + 8.0, cards_y + 36.0, 11.0, dpi, true, [0.16, 0.24, 0.32, 0.88 * alpha], 1, None);
        text.push(gl, ">", cs.hits.cal_next.x + 8.0, cards_y + 36.0, 11.0, dpi, true, [0.16, 0.24, 0.32, 0.88 * alpha], 1, None);
        let today = now.date_naive();
        for d_idx in 0..7usize {
            let d_off = cs.cal_week_offset + d_idx as i64;
            let d = today + ChronoDuration::days(d_off);
            let r_day = cs.hits.cal_days[d_idx];
            let dx = r_day.x + r_day.w * 0.5;
            let is_sel = d_off == cs.cal_selected_day_off;
            let is_today = d_off == 0;
            let wd = weekday_letter(d.weekday());
            text.push(
                gl,
                wd,
                dx,
                cards_y + 18.0,
                9.0,
                dpi,
                false,
                [0.22, 0.30, 0.38, 0.85 * alpha],
                1,
                None,
            );
            if is_sel {
                self.draw_ui_circle(
                    gl,
                    css_w,
                    css_h,
                    dx,
                    cards_y + 33.0,
                    10.5,
                    [0.08, 0.10, 0.13, alpha],
                    [0.0; 4],
                    0.0,
                    false,
                );
            } else if is_today {
                self.draw_ui_circle(
                    gl,
                    css_w,
                    css_h,
                    dx,
                    cards_y + 33.0,
                    10.5,
                    [0.0; 4],
                    [0.10, 0.14, 0.19, 0.70 * alpha],
                    1.2,
                    false,
                );
            }
            text.push(
                gl,
                &format!("{}", d.day()),
                dx,
                cards_y + 37.0,
                10.5,
                dpi,
                is_sel || is_today,
                if is_sel {
                    [0.98, 0.99, 1.0, alpha]
                } else {
                    [0.12, 0.18, 0.24, alpha]
                },
                1,
                None,
            );
        }

        let ev_indices = cs.selected_day_event_indices();
        let ev1_y = cards_y + 56.0;
        let ev1_h = (cards_h * 0.30).clamp(50.0, 68.0);
        let ev_card_x = x3 + 42.0;
        let ev_card_w = (w3 - 52.0).max(40.0);

        if ev_indices.is_empty() {
            text.push(
                gl,
                "No events on this day",
                x3 + w3 * 0.5,
                ev1_y + 34.0,
                10.8,
                dpi,
                false,
                [0.18, 0.26, 0.35, 0.82 * alpha],
                1,
                None,
            );
        } else {
            for (slot, &ev_idx) in ev_indices.iter().enumerate() {
                let ev = &cs.events[ev_idx];
                let ey = if slot == 0 { ev1_y } else { ev1_y + ev1_h + 8.0 };
                let eh_card = if slot == 0 { ev1_h } else { 42.0 };
                if ey + eh_card > cards_y + cards_h - 24.0 {
                    break;
                }
                text.push(gl, &ev.time, x3 + 8.0, ey + 17.0, 10.0, dpi, true, [0.12, 0.17, 0.24, alpha], 0, Some(34.0));
                self.draw_ui_box(
                    gl,
                    css_w,
                    css_h,
                    ev_card_x,
                    ey,
                    ev_card_w,
                    eh_card,
                    [10.0; 4],
                    if slot == 0 {
                        [0.09, 0.11, 0.14, alpha]
                    } else {
                        [0.53, 0.63, 0.72, alpha]
                    },
                    [0.0; 4],
                    0.0,
                    0.0,
                );
                let title_col = if slot == 0 {
                    [0.95, 0.97, 0.99, alpha]
                } else {
                    [0.10, 0.15, 0.22, alpha]
                };
                let meta_col = if slot == 0 {
                    [0.64, 0.68, 0.74, alpha]
                } else {
                    [0.20, 0.28, 0.36, 0.88 * alpha]
                };
                text.push(gl, &ev.title, ev_card_x + 10.0, ey + 18.0, 11.0, dpi, true, title_col, 0, Some(ev_card_w - 28.0));
                text.push(gl, &ev.meta, ev_card_x + 10.0, ey + 33.0, 9.2, dpi, false, meta_col, 0, Some(ev_card_w - 24.0));
                let del_r = cs.hits.cal_event_dels[slot];
                text.push(gl, "x", del_r.x + 9.0, del_r.y + 13.0, 9.8, dpi, true, meta_col, 1, None);
            }
        }

        // Footer: selected date + "+ Event" button
        let sel_d = cs.selected_date();
        text.push(
            gl,
            &format!("{} {}", sel_d.day(), month_short(sel_d.month())),
            x3 + 12.0,
            cards_y + cards_h - 13.0,
            10.0,
            dpi,
            false,
            [0.18, 0.26, 0.35, 0.88 * alpha],
            0,
            None,
        );
        let bae = cs.hits.btn_add_event;
        self.draw_ui_box(
            gl,
            css_w,
            css_h,
            bae.x,
            bae.y,
            bae.w,
            bae.h,
            [10.0; 4],
            [0.50, 0.60, 0.70, alpha],
            [0.0; 4],
            0.0,
            0.0,
        );
        text.push(
            gl,
            "+ Event",
            bae.x + bae.w * 0.5,
            bae.y + 14.0,
            9.8,
            dpi,
            true,
            [0.10, 0.16, 0.22, alpha],
            1,
            None,
        );

        text.flush(gl, css_w, css_h);
    }
}

fn wrap_lines(text: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let mut out = Vec::new();
    let lim = max_chars.max(8);
    for raw_line in text.lines() {
        let trimmed = raw_line.trim_end();
        if trimmed.is_empty() {
            if out.len() < max_lines {
                out.push(String::new());
            }
            continue;
        }
        let mut cur = String::new();
        for word in trimmed.split_whitespace() {
            let wlen = word.chars().count();
            let clen = cur.chars().count();
            if clen == 0 {
                if wlen > lim {
                    cur = word.chars().take(lim).collect();
                } else {
                    cur.push_str(word);
                }
            } else if clen + 1 + wlen <= lim {
                cur.push(' ');
                cur.push_str(word);
            } else {
                out.push(cur);
                if out.len() >= max_lines {
                    return out;
                }
                cur = if wlen > lim {
                    word.chars().take(lim).collect()
                } else {
                    word.to_string()
                };
            }
        }
        if !cur.is_empty() && out.len() < max_lines {
            out.push(cur);
        }
        if out.len() >= max_lines {
            break;
        }
    }
    out
}

fn month_short(m: u32) -> &'static str {
    match m {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    }
}

fn weekday_letter(w: chrono::Weekday) -> &'static str {
    match w {
        chrono::Weekday::Mon => "M",
        chrono::Weekday::Tue => "T",
        chrono::Weekday::Wed => "W",
        chrono::Weekday::Thu => "T",
        chrono::Weekday::Fri => "F",
        chrono::Weekday::Sat => "S",
        chrono::Weekday::Sun => "S",
    }
}

// ============================================================================
// 5. UNIT TESTS (Determinism, params, gaze, notch interactions, icons)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_parsing_and_clamping() {
        let mut raw = HashMap::new();
        raw.insert("notchMode".into(), Value::String("pinned".into()));
        raw.insert("catScale".into(), Value::from(9.9));
        raw.insert("rippleStrength".into(), Value::from(0.05));
        raw.insert("waterSpeed".into(), Value::from(1.6));
        raw.insert("shoreTone".into(), Value::String("twilight".into()));
        raw.insert("adaptive".into(), Value::Bool(false));
        raw.insert("seed".into(), Value::from(1337));

        let p = CatShoreParams::from_raw_params(&raw);
        assert_eq!(p.notch_mode, 1);
        assert!((p.cat_scale - 1.4).abs() < 1e-6);
        assert!((p.ripple_strength - 0.2).abs() < 1e-6);
        assert!((p.water_speed - 1.6).abs() < 1e-6);
        assert_eq!(p.shore_tone, 2);
        assert!(!p.adaptive);
        assert_eq!(p.seed, 1337);
    }

    #[test]
    fn shore_texture_is_deterministic_and_opaque() {
        let a = build_pebble_shore_texture(128, 72, 42);
        let b = build_pebble_shore_texture(128, 72, 42);
        assert_eq!(a, b);
        assert_eq!(a.len(), 128 * 72 * 4);
        assert!(a.chunks_exact(4).all(|px| px[3] == 255));
        // Top-right corner should be warmer (sand) than bottom-left (cerulean water)
        let tr = (2 * 128 + 125) * 4;
        let bl = (69 * 128 + 2) * 4;
        assert!(a[tr] > a[tr + 2], "Top-right beach should be warm golden sand");
        assert!(a[bl + 2] > a[bl], "Bottom-left should be cool cerulean water");
    }

    #[test]
    fn cat_gaze_tracks_mouse_and_notch_expands_interactively() {
        let mut st = CatShoreState::new(1920.0, 1080.0, 1.0);
        // Move pointer to top-right
        st.handle_mouse(0, 1700.0, 100.0);
        for _ in 0..30 {
            st.step(0.033);
        }
        assert!(st.gaze_x > 0.3, "Cat should look right toward cursor, got {}", st.gaze_x);
        assert!(st.gaze_y < -0.2, "Cat should look up toward cursor, got {}", st.gaze_y);

        // Hover collapsed top notch -> expands smoothly
        st.handle_mouse(0, 960.0, 14.0);
        assert!(st.notch_hover);
        for _ in 0..45 {
            st.step(0.033);
        }
        assert!(st.notch_open > 0.9, "Notch should expand on hover, got {}", st.notch_open);

        // Click task 0 checkbox -> toggles done
        let chk = st.hits.task_checks[0];
        assert!(!st.tasks[0].done);
        assert!(st.handle_mouse(1, chk.x + 12.0, chk.y + 12.0));
        assert!(st.tasks[0].done);

        // Click timer start -> timer begins counting down
        let bt = st.hits.btn_timer_toggle;
        assert!(!st.timer_running);
        assert!(st.handle_mouse(1, bt.x + 10.0, bt.y + 10.0));
        assert!(st.timer_running);
        let before = st.timer_left_sec;
        st.step(0.1);
        assert!(st.timer_left_sec < before);

        // Switch to Clips tab and back
        let tc = st.hits.tab_clips;
        assert!(st.handle_mouse(1, tc.x + 10.0, tc.y + 10.0));
        assert_eq!(st.notch_tab, 1);

        // Resize threshold (> 0.5px only bumps rev)
        let rev0 = st.rev;
        st.resize(1920.2, 1080.2, 1.0);
        assert_eq!(st.rev, rev0);
        st.resize(1280.0, 720.0, 1.0);
        assert_ne!(st.rev, rev0);

        // Icons update rev
        let rev1 = st.rev;
        st.set_icons(
            &[Rect {
                x: 100.0,
                y: 120.0,
                w: 76.0,
                h: 86.0,
            }],
            true,
        );
        assert_ne!(st.rev, rev1);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn gpu_render_cat_shore_collapsed_and_expanded() {
        use std::ffi::CString;
        let _guard = crate::LINUX_EGL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let eng = crate::ddgl_headless_create(1280, 720, 1.0);
            assert!(!eng.is_null());
            let theme_msg = CString::new(r#"{"t":"theme","id":"cat-shore"}"#).unwrap();
            crate::ddgl_headless_post_json(eng, theme_msg.as_ptr());

            // 1. Collapsed notch + cat looking up-right toward cursor
            crate::ddgl_headless_mouse(eng, 0, 960.0, 260.0);
            let out1 = std::env::temp_dir().join("ddgl_cat_shore_collapsed.png");
            let c1 = CString::new(out1.to_str().unwrap()).unwrap();
            for _ in 0..8 {
                assert_eq!(crate::ddgl_headless_render_png(eng, 0.05, c1.as_ptr()), 1);
            }
            assert!(std::fs::metadata(&out1).map(|m| m.len() > 20_000).unwrap_or(false));

            // 2. Expanded 4-card top notch bar + cat looking straight up at notch
            let pin_msg = CString::new(r#"{"t":"settings","v":{"notchMode":"pinned"}}"#).unwrap();
            crate::ddgl_headless_post_json(eng, pin_msg.as_ptr());
            crate::ddgl_headless_mouse(eng, 0, 640.0, 210.0);
            let out2 = std::env::temp_dir().join("ddgl_cat_shore_expanded.png");
            let c2 = CString::new(out2.to_str().unwrap()).unwrap();
            for _ in 0..28 {
                assert_eq!(crate::ddgl_headless_render_png(eng, 0.05, c2.as_ptr()), 1);
            }
            assert!(std::fs::metadata(&out2).map(|m| m.len() > 20_000).unwrap_or(false));

            crate::ddgl_headless_destroy(eng);
        }
    }
}
