//! THE EYE WITHIN WORDS — 1:1 Rust + OpenGL 3.3 port of
//! `https://github.com/mishratanya2311/eye_python` (main.py).
//!
//! A full-screen generative typography artwork in which 100% of the screen is
//! filled edge-to-edge with readable meaningful quotes that collectively form
//! one enormous, living, blinking human eye, with a continuous cinematic
//! camera zoom between wide and close-up views.
//!
//! Port map (Python -> here):
//!   `generate_meaningful_quotes`  -> `generate_meaningful_quotes` (same lists)
//!   `TypographyPalette` / `PALETTES` -> `EyePalette` / `PALETTES`
//!   `EyeCanvasGeometry` / `calculate_eye_spatial_and_color_patterns` -> same
//!   `eval_eyelid_curves` / `eval_field_streamline_y` -> same (scalar, f64)
//!   `layout_full_canvas_typography` -> `bake::layout_full_canvas_typography`
//!   `create_iris_and_pupil_typography` -> `bake::create_iris_and_pupil_typography`
//!   `create_animated_eyelids_and_masks` -> `bake::create_animated_eyelids_and_masks`
//!   `BlinkController` -> `BlinkController` (natural biological blinking)
//!   `CinematicCamera` -> `CinematicCamera` (zoom + gaze + drift)
//!   `render_fullscreen_composition` -> GLSL `EYE_COMPOSITE_FRAG` + rim sprite pass
//!   `random.Random` -> `pyrng::PyRng` (CPython-exact MT19937)
//!
//! Controls of the original (SPACE/B/Z/C/R keys) map to theme params
//! (`blink` trigger via wallpaper click, `autoBlink`, `autoZoom`, `palette`,
//! `seed`) because wallpaper pages do not receive raw keyboard events.

pub mod bake;
pub mod pyrng;
pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use bake::{BakeKey, BakedEyeLayers, EyeBaker, RgbaLayer, RimWord};
use pyrng::PyRng;
use serde_json::Value;
use shaders::{EYE_COMPOSITE_FRAG, EYE_SPRITE_FRAG, EYE_SPRITE_VERT};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};

// ============================================================================
// 1. GENERATING MEANINGFUL QUOTES & POETIC VOCABULARY (1:1 with main.py)
// ============================================================================

pub const SIGNATURE_QUOTES: &[&str] = &[
    "we are made of stars",
    "look beyond what you see",
    "every thought leaves a trace",
    "the universe lives within us",
    "beauty exists in the details",
    "we become what we believe",
    "silence has its own language",
    "nothing is ever truly invisible",
    "your perception creates your world",
    "the truth hides between the lines",
    "every ending begins somewhere",
    "consciousness observing itself",
    "there is more than meets the eye",
    "even darkness carries meaning",
    "wonder is a form of intelligence",
    "the world is written in patterns",
    "what you seek is seeking you",
    "everything is connected",
    "look closer",
    "stay curious",
];

pub const ADDITIONAL_POETIC_QUOTES: &[&str] = &[
    "light remembers where it has been",
    "every question opens an unseen door",
    "we are the universe becoming aware",
    "in stillness the hidden becomes clear",
    "meaning blooms where attention rests",
    "time is a river of quiet revelations",
    "beneath every surface lies another sky",
    "words are the architecture of thought",
    "to truly see is to be transformed",
    "infinite worlds inside a single glance",
    "memory is starlight traveling through mind",
    "the observer and the observed are one",
    "clarity arrives when the mind grows quiet",
    "every heartbeat echoes across eternity",
    "dreams are letters from the unseen self",
    "curiosity is the compass of the soul",
    "shadows only prove the presence of light",
    "vision begins where certainty ends",
    "all things are woven from the same light",
    "listen to the quiet between your thoughts",
    "awareness is the canvas of existence",
    "every moment contains the whole of time",
    "the eye reads what the heart remembers",
    "patterns of light become living thought",
    "seek the poetry hidden in plain sight",
    "nothing seen is ever separate from you",
    "thought is the brushstroke of reality",
    "within the finite dwells the infinite",
    "each breath is a quiet beginning",
    "truth shines brightest in the dark",
];

pub const MEANINGFUL_SHORT_WORDS: &[&str] = &[
    "existence", "perception", "eternity", "wonder", "look closer",
    "stay curious", "consciousness", "clarity", "horizon", "infinity",
    "presence", "silence", "radiance", "reflection", "insight",
    "empathy", "mystery", "serenity", "awakening", "harmony",
    "cosmos", "memory", "origin", "vision", "truth", "light",
    "dream", "spirit", "essence", "timeless", "intimate", "witness",
    "observer", "vivid", "lucid", "transcend", "illuminate", "connect",
    "breathe", "imagine", "perceive", "discover", "believe", "focus",
];

pub const PUPIL_DARK_WORDS: &[&str] = &[
    "within", "silence", "origin", "eternity", "mystery", "depth",
    "unseen", "essence", "center", "stillness", "cosmos", "infinite",
    "shadow", "secret", "truth", "existence", "source", "soul",
    "awareness", "void", "core", "unknown", "singularity", "intimate",
];

pub const HIGHLIGHT_WORDS: &[&str] = &[
    "light", "clarity", "wonder", "truth", "shine", "vision",
    "lucid", "spark", "pure", "awake", "vivid", "radiance",
    "star", "clear", "see", "glow", "illuminate",
];

/// 1:1 port of `generate_meaningful_quotes(rng, mode)`.
/// Modes: 'quote', 'mixed', 'short', 'pupil', 'highlight'.
pub fn generate_meaningful_quotes(rng: &mut PyRng, mode: &str) -> &'static str {
    match mode {
        "highlight" => return rng.choice(HIGHLIGHT_WORDS),
        "pupil" => return rng.choice(PUPIL_DARK_WORDS),
        "short" => return rng.choice(MEANINGFUL_SHORT_WORDS),
        "quote" => {
            if rng.random() < 0.48 {
                return rng.choice(SIGNATURE_QUOTES);
            }
            return rng.choice(ADDITIONAL_POETIC_QUOTES);
        }
        _ => {}
    }
    // mixed
    let r = rng.random();
    if r < 0.40 {
        rng.choice(SIGNATURE_QUOTES)
    } else if r < 0.76 {
        rng.choice(ADDITIONAL_POETIC_QUOTES)
    } else {
        rng.choice(MEANINGFUL_SHORT_WORDS)
    }
}

// ============================================================================
// 2. COLOR PALETTES & ATMOSPHERIC THEMES (1:1 with main.py)
// ============================================================================

#[derive(Clone, Copy)]
#[allow(dead_code)] // `name`/`accent_tint` kept 1:1 with the Python palette dataclass
pub struct EyePalette {
    pub name: &'static str,
    pub canvas_bg: [u8; 3],
    pub bg_field_dark: &'static [[u8; 3]],
    pub bg_field_orbital: &'static [[u8; 3]],
    pub sclera_colors: &'static [[u8; 3]],
    pub iris_outer: &'static [[u8; 3]],
    pub iris_mid: &'static [[u8; 3]],
    pub iris_inner: &'static [[u8; 3]],
    pub iris_alt: &'static [[u8; 3]],
    pub pupil_words: &'static [[u8; 3]],
    pub highlight_words: &'static [[u8; 3]],
    pub eyelid_rim: &'static [[u8; 3]],
    pub eyelid_sheet: &'static [[u8; 3]],
    pub accent_tint: [u8; 3],
}

pub const PALETTES: [EyePalette; 3] = [
    // 0: Midnight Poetry — Cyan, Electric Blue & Violet
    EyePalette {
        name: "Midnight Poetry — Cyan, Electric Blue & Violet",
        canvas_bg: [3, 6, 13],
        bg_field_dark: &[
            [36, 58, 96], [42, 66, 108], [46, 50, 92],
            [32, 52, 86], [50, 74, 118],
        ],
        bg_field_orbital: &[
            [62, 94, 148], [74, 82, 144], [56, 86, 138], [80, 110, 164],
        ],
        sclera_colors: &[
            [244, 248, 255], [242, 236, 224], [208, 224, 244],
            [184, 212, 242], [228, 236, 248],
        ],
        iris_outer: &[
            [32, 78, 175], [68, 52, 168], [24, 108, 168], [82, 58, 182],
        ],
        iris_mid: &[
            [0, 222, 252], [22, 238, 218], [45, 155, 255],
            [118, 82, 248], [0, 198, 210], [68, 195, 255],
        ],
        iris_inner: &[
            [135, 250, 255], [85, 235, 255], [185, 245, 255], [162, 138, 255],
        ],
        iris_alt: &[
            [142, 78, 252], [0, 242, 218], [48, 178, 255], [178, 105, 255],
        ],
        pupil_words: &[
            [28, 44, 78], [34, 52, 90], [42, 38, 82], [24, 38, 68],
        ],
        highlight_words: &[
            [255, 255, 255], [222, 248, 255], [190, 240, 255],
        ],
        eyelid_rim: &[
            [232, 244, 255], [196, 222, 250], [172, 206, 245], [218, 232, 252],
        ],
        eyelid_sheet: &[
            [98, 136, 192], [108, 98, 172], [84, 118, 172], [124, 156, 208],
        ],
        accent_tint: [0, 195, 245],
    },
    // 1: Bioluminescent Verse — Turquoise, Emerald & Deep Navy
    EyePalette {
        name: "Bioluminescent Verse — Turquoise, Emerald & Deep Navy",
        canvas_bg: [3, 9, 12],
        bg_field_dark: &[
            [30, 66, 76], [36, 74, 86], [28, 58, 72], [42, 82, 92],
        ],
        bg_field_orbital: &[
            [54, 108, 118], [62, 118, 128], [48, 96, 112],
        ],
        sclera_colors: &[
            [242, 250, 248], [238, 235, 224], [202, 228, 226], [178, 216, 220],
        ],
        iris_outer: &[
            [16, 96, 102], [24, 76, 128], [14, 112, 92], [38, 64, 134],
        ],
        iris_mid: &[
            [0, 230, 184], [20, 240, 212], [38, 188, 238], [16, 210, 152],
        ],
        iris_inner: &[
            [142, 255, 228], [98, 248, 242], [182, 255, 238], [82, 238, 198],
        ],
        iris_alt: &[
            [0, 198, 245], [52, 248, 168], [26, 224, 218], [112, 252, 204],
        ],
        pupil_words: &[
            [22, 46, 54], [28, 54, 66], [26, 42, 62],
        ],
        highlight_words: &[
            [255, 255, 255], [222, 255, 248], [188, 248, 240],
        ],
        eyelid_rim: &[
            [226, 248, 244], [188, 228, 222], [164, 212, 208],
        ],
        eyelid_sheet: &[
            [84, 150, 156], [74, 126, 144], [102, 166, 170],
        ],
        accent_tint: [0, 215, 175],
    },
    // 2: Astral Manuscript — Amethyst, Starlight & Indigo
    EyePalette {
        name: "Astral Manuscript — Amethyst, Starlight & Indigo",
        canvas_bg: [6, 5, 14],
        bg_field_dark: &[
            [48, 44, 88], [56, 50, 98], [42, 52, 92], [62, 54, 108],
        ],
        bg_field_orbital: &[
            [82, 74, 138], [74, 86, 142], [94, 82, 152],
        ],
        sclera_colors: &[
            [248, 245, 254], [242, 236, 228], [216, 212, 238], [192, 206, 242],
        ],
        iris_outer: &[
            [78, 42, 152], [106, 36, 142], [48, 56, 158], [92, 34, 128],
        ],
        iris_mid: &[
            [176, 88, 252], [124, 112, 255], [218, 84, 228], [72, 174, 255],
        ],
        iris_inner: &[
            [236, 188, 255], [196, 222, 255], [248, 204, 255], [162, 236, 255],
        ],
        iris_alt: &[
            [92, 182, 255], [232, 108, 250], [154, 124, 255], [244, 144, 236],
        ],
        pupil_words: &[
            [36, 28, 68], [44, 34, 80], [30, 38, 74],
        ],
        highlight_words: &[
            [255, 255, 255], [245, 238, 255], [218, 242, 255],
        ],
        eyelid_rim: &[
            [240, 230, 255], [208, 196, 242], [186, 204, 248],
        ],
        eyelid_sheet: &[
            [128, 110, 186], [104, 120, 178], [144, 124, 198],
        ],
        accent_tint: [168, 92, 248],
    },
];

// ============================================================================
// 3. CALCULATING THE EYE'S SPATIAL AND COLOR PATTERNS (1:1 with main.py)
// ============================================================================

/// Defines both the physical screen resolution and the larger high-resolution
/// virtual typography canvas so zooming in/out is always razor-sharp and never
/// exposes blank edges. (All units: physical screen pixels, like pygame.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EyeCanvasGeometry {
    pub screen_w: i32,
    pub screen_h: i32,
    pub canvas_w: i32,
    pub canvas_h: i32,
    pub center_x: i32,
    pub center_y: i32,
    pub eye_half_w: f64,
    pub eye_half_h: f64,
    pub iris_radius: f64,
    pub pupil_radius: f64,
    pub scale: f64,
}

/// 1:1 port of `calculate_eye_spatial_and_color_patterns`.
pub fn calculate_eye_spatial_and_color_patterns(
    screen_w: i32,
    screen_h: i32,
) -> EyeCanvasGeometry {
    let canvas_mult = 1.55;
    let mut canvas_w = (screen_w as f64 * canvas_mult).round() as i32;
    let mut canvas_h = (screen_h as f64 * canvas_mult).round() as i32;
    if canvas_w % 2 == 1 {
        canvas_w += 1;
    }
    if canvas_h % 2 == 1 {
        canvas_h += 1;
    }

    let center_x = canvas_w / 2;
    let center_y = canvas_h / 2;

    let scale = (screen_w as f64 / 1512.0).min(screen_h as f64 / 982.0) * 1.18;

    let wide_view_w = canvas_w as f64 * 0.82;
    let wide_view_h = canvas_h as f64 * 0.82;
    let eye_half_w = wide_view_w * 0.455;
    let eye_half_h = wide_view_h * 0.285;

    let iris_radius = eye_half_h * 1.02;
    let pupil_radius = iris_radius * 0.34;

    EyeCanvasGeometry {
        screen_w,
        screen_h,
        canvas_w,
        canvas_h,
        center_x,
        center_y,
        eye_half_w,
        eye_half_h,
        iris_radius,
        pupil_radius,
        scale,
    }
}

/// Scale a geometry into bake-pixel space (used when quality < 1.0).
pub fn scale_geometry(geom: &EyeCanvasGeometry, s: f64) -> EyeCanvasGeometry {
    EyeCanvasGeometry {
        screen_w: ((geom.screen_w as f64 * s).round() as i32).max(64),
        screen_h: ((geom.screen_h as f64 * s).round() as i32).max(64),
        canvas_w: ((geom.canvas_w as f64 * s).round() as i32).max(128),
        canvas_h: ((geom.canvas_h as f64 * s).round() as i32).max(80),
        center_x: (geom.center_x as f64 * s).round() as i32,
        center_y: (geom.center_y as f64 * s).round() as i32,
        eye_half_w: geom.eye_half_w * s,
        eye_half_h: geom.eye_half_h * s,
        iris_radius: geom.iris_radius * s,
        pupil_radius: geom.pupil_radius * s,
        scale: geom.scale * s,
    }
}

/// 1:1 port of `eval_eyelid_curves` (scalar): upper and lower eyelid vertical
/// offsets y_upper(u), y_lower(u) relative to `geom.center_y` (+y is downward).
/// `u` in [-1.0, +1.0] spans inner medial (-1) to outer lateral (+1) corner.
pub fn eval_eyelid_curves(u: f64, openness: f64, geom: &EyeCanvasGeometry) -> (f64, f64) {
    let u_clamped = u.clamp(-1.0, 1.0);
    let env = (1.0 - u_clamped * u_clamped).max(0.0).powf(0.76);

    let upper_arch = 1.0 - 0.11 * u_clamped - 0.06 * (u_clamped * u_clamped);
    let lower_arch = 1.0 + 0.13 * u_clamped - 0.05 * (u_clamped * u_clamped);

    let mut y_upper_open = -geom.eye_half_h * 1.04 * env * upper_arch;
    let mut y_lower_open = geom.eye_half_h * 0.88 * env * lower_arch;

    let medial_dip = 0.09 * geom.eye_half_h * (-(((u_clamped + 0.89) / 0.20).powi(2))).exp();
    let lateral_sweep = -0.06 * geom.eye_half_h * (-(((u_clamped - 0.86) / 0.25).powi(2))).exp();
    let corner_line = medial_dip + lateral_sweep;

    y_upper_open += corner_line;
    y_lower_open += corner_line;

    // Closed eye curve: gentle downward-sloping sleeping curve
    let y_closed = geom.eye_half_h * 0.16 * env + corner_line;

    let t = openness.clamp(0.0, 1.0);
    let y_upper = y_closed * (1.0 - t) + y_upper_open * t;
    let y_lower = y_closed * (1.0 - t) + y_lower_open * t;
    (y_upper, y_lower)
}

/// 1:1 port of `eval_field_streamline_y` (scalar): smooth, continuous
/// streamline function across the entire virtual canvas.
pub fn eval_field_streamline_y(x: f64, base_y_offset: f64, geom: &EyeCanvasGeometry) -> f64 {
    let u = (x - geom.center_x as f64) / geom.eye_half_w;
    let gaussian_bell = (-0.5 * (u / 0.76).powi(2)).exp();
    let norm_y = base_y_offset.abs() / geom.eye_half_h.max(1.0);
    let dist_atten = (-norm_y / 1.85).exp();
    let arch_push = base_y_offset * 0.42 * gaussian_bell * dist_atten;
    geom.center_y as f64 + base_y_offset + arch_push
}

/// CPython `round()` (banker's rounding, half to even).
pub fn py_round(x: f64) -> f64 {
    let frac = (x - x.trunc()).abs();
    if frac == 0.5 {
        let f = x.floor();
        if (f as i64) % 2 == 0 {
            f
        } else {
            f + 1.0
        }
    } else {
        x.round()
    }
}

/// CPython `int()` (truncation toward zero).
pub fn py_int(x: f64) -> i64 {
    x.trunc() as i64
}

// ============================================================================
// 4. PARAMS (mirrors prototype/themes.json)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EyeWithinWordsParams {
    pub palette: usize,   // 0=midnightPoetry, 1=bioluminescentVerse, 2=astralManuscript
    pub auto_blink: bool, // original B key
    pub auto_zoom: bool,  // original Z key
    pub adaptive: bool,   // desktop-icon reading scrims (DiyarDesk addition)
    pub quality: usize,   // 0=low, 1=balanced, 2=high
    pub seed: u32,        // original R key (regenerate arrangement)
}

impl Default for EyeWithinWordsParams {
    fn default() -> Self {
        Self {
            palette: 0,
            auto_blink: true,
            auto_zoom: true,
            adaptive: true,
            quality: 2,
            seed: 424242,
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

impl EyeWithinWordsParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        Self {
            palette: parse_enum(
                raw,
                "palette",
                &["midnightpoetry", "bioluminescentverse", "astralmanuscript"],
                0,
            ),
            auto_blink: parse_bool(raw, "autoBlink", true),
            auto_zoom: parse_bool(raw, "autoZoom", true),
            adaptive: parse_bool(raw, "adaptive", true),
            quality: parse_enum(raw, "quality", &["low", "balanced", "high"], 2),
            seed: parse_f32(raw, "seed", 424242.0, 0.0, 999999.0).round() as u32,
        }
    }
}

// ============================================================================
// 5. ANIMATED EYELIDS: BlinkController (1:1 with main.py)
// ============================================================================

/// Natural biological blink controller (every 3-6 seconds with occasional
/// double blinks).
pub struct BlinkController {
    pub auto_blink: bool,
    pub is_blinking: bool,
    pub blink_start: f64,
    pub close_dur: f64,
    pub hold_dur: f64,
    pub open_dur: f64,
    pub queued_double: bool,
    pub next_blink_time: f64,
}

impl BlinkController {
    pub fn new(rng: &mut PyRng) -> Self {
        Self {
            auto_blink: true,
            is_blinking: false,
            blink_start: 0.0,
            close_dur: 0.125,
            hold_dur: 0.045,
            open_dur: 0.225,
            queued_double: false,
            next_blink_time: rng.uniform(2.5, 4.5),
        }
    }

    pub fn trigger_blink(&mut self, now: f64, allow_double: bool, rng: &mut PyRng) {
        self.is_blinking = true;
        self.blink_start = now;
        self.close_dur = rng.uniform(0.115, 0.140);
        self.hold_dur = rng.uniform(0.038, 0.058);
        self.open_dur = rng.uniform(0.200, 0.260);
        // Python short-circuits: `allow_double and (random.random() < 0.24)`
        self.queued_double = allow_double && (rng.random() < 0.24);
        self.next_blink_time = now + rng.uniform(3.0, 6.0);
    }

    pub fn update(&mut self, now: f64, rng: &mut PyRng) -> f64 {
        if self.auto_blink && !self.is_blinking && now >= self.next_blink_time {
            self.trigger_blink(now, true, rng);
        }

        if !self.is_blinking {
            return 1.0;
        }

        let elapsed = now - self.blink_start;
        let total = self.close_dur + self.hold_dur + self.open_dur;

        if elapsed < 0.0 {
            return 1.0;
        }
        if elapsed < self.close_dur {
            let u = elapsed / self.close_dur;
            let ease = 3.0 * (u * u) - 2.0 * (u * u * u);
            return (1.0 - ease).max(0.0);
        } else if elapsed < self.close_dur + self.hold_dur {
            return 0.0;
        } else if elapsed < total {
            let u = (elapsed - self.close_dur - self.hold_dur) / self.open_dur;
            let ease = 1.0 - (1.0 - u).powf(2.6);
            return ease.min(1.0);
        } else {
            if self.queued_double {
                self.queued_double = false;
                self.trigger_blink(now + 0.045, false, rng);
            } else {
                self.is_blinking = false;
            }
            return 1.0;
        }
    }
}

// ============================================================================
// 6. CINEMATIC CAMERA (1:1 with main.py)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CamRect {
    pub left: i32,
    pub top: i32,
    pub w: i32,
    pub h: i32,
}

/// Manages continuous, smooth camera zoom and gentle panning between the wide
/// view (zoom = 1.0, whole eye in the wall of typography) and close-up views
/// (zoom up to 2.55x, deep into readable quotes without blank edges).
pub struct CinematicCamera {
    pub auto_zoom: bool,
    pub zoom: f64,
    pub phase: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub gaze_x: f64,
    pub gaze_y: f64,
    pub last_mouse_x: i32,
    pub last_mouse_y: i32,
    pub last_mouse_time: f64,
}

impl Default for CinematicCamera {
    fn default() -> Self {
        Self {
            auto_zoom: true,
            zoom: 1.0,
            phase: 0.0,
            pan_x: 0.0,
            pan_y: 0.0,
            gaze_x: 0.0,
            gaze_y: 0.0,
            last_mouse_x: 0,
            last_mouse_y: 0,
            last_mouse_time: 0.0,
        }
    }
}

impl CinematicCamera {
    /// 1:1 port of `manage_zoom_and_camera`. `now`/`dt` in seconds; `mouse`
    /// replaces `pygame.mouse.get_pos()` in physical screen pixels.
    pub fn manage_zoom_and_camera(
        &mut self,
        geom: &EyeCanvasGeometry,
        now: f64,
        dt: f64,
        mouse: (i32, i32),
    ) -> (CamRect, f64, f64) {
        if mouse != (self.last_mouse_x, self.last_mouse_y) {
            self.last_mouse_x = mouse.0;
            self.last_mouse_y = mouse.1;
            self.last_mouse_time = now;
        }

        let max_gx = geom.screen_w as f64 * 0.022;
        let max_gy = geom.screen_h as f64 * 0.018;
        if now - self.last_mouse_time < 4.5 {
            let nx = (mouse.0 as f64 - geom.screen_w as f64 * 0.5)
                / (geom.screen_w as f64 * 0.5).max(1.0);
            let ny = (mouse.1 as f64 - geom.screen_h as f64 * 0.5)
                / (geom.screen_h as f64 * 0.5).max(1.0);
            let target_gx = nx.clamp(-1.0, 1.0) * max_gx;
            let target_gy = ny.clamp(-1.0, 1.0) * max_gy;
            self.gaze_x += (target_gx - self.gaze_x) * (dt * 5.5).min(1.0);
            self.gaze_y += (target_gy - self.gaze_y) * (dt * 5.5).min(1.0);
        } else {
            let target_gx = (now * 0.45).sin() * (now * 0.21).cos() * (max_gx * 0.45);
            let target_gy = (now * 0.32 + 1.1).sin() * (max_gy * 0.40);
            self.gaze_x += (target_gx - self.gaze_x) * (dt * 5.5).min(1.0);
            self.gaze_y += (target_gy - self.gaze_y) * (dt * 5.5).min(1.0);
        }

        if self.auto_zoom {
            let speed_mod = 0.34 + 0.08 * (now * 0.17).sin();
            self.phase += dt * speed_mod;
            let raw_wave = 0.5 - 0.5 * self.phase.cos();
            let eased = raw_wave * raw_wave * (3.0 - 2.0 * raw_wave);
            self.zoom = 1.0 + eased * 1.55;
        } else {
            self.zoom += (1.0 - self.zoom) * (dt * 4.0).min(1.0);
        }

        let base_vw = geom.canvas_w as f64 * 0.82;
        let base_vh = geom.canvas_h as f64 * 0.82;

        let mut view_w = py_int(py_round(base_vw / self.zoom)) as i32;
        let mut view_h = py_int(py_round(base_vh / self.zoom)) as i32;
        view_w = view_w.clamp(320, geom.canvas_w);
        view_h = view_h.clamp(200, geom.canvas_h);

        let zoom_factor = (self.zoom - 1.0) / 1.55;
        let drift_radius_x = geom.eye_half_w * 0.30 * zoom_factor;
        let drift_radius_y = geom.eye_half_h * 0.25 * zoom_factor;

        let target_px = (now * 0.23).sin() * drift_radius_x + self.gaze_x * 1.4;
        let target_py = (now * 0.19).cos() * drift_radius_y + self.gaze_y * 1.4;
        self.pan_x += (target_px - self.pan_x) * (dt * 3.5).min(1.0);
        self.pan_y += (target_py - self.pan_y) * (dt * 3.5).min(1.0);

        let cam_cx = py_int(py_round(geom.center_x as f64 + self.pan_x)) as i32;
        let cam_cy = py_int(py_round(geom.center_y as f64 + self.pan_y)) as i32;

        let left = (cam_cx - view_w / 2).clamp(0, geom.canvas_w - view_w);
        let top = (cam_cy - view_h / 2).clamp(0, geom.canvas_h - view_h);

        (
            CamRect {
                left,
                top,
                w: view_w,
                h: view_h,
            },
            self.gaze_x,
            self.gaze_y,
        )
    }
}

// ============================================================================
// 7. THEME STATE (animation only; typography layers are baked by the renderer)
// ============================================================================

pub struct EyeWithinWordsState {
    pub params: EyeWithinWordsParams,
    pub w_css: f32,
    pub h_css: f32,
    pub dpr: f32,
    pub low_mode: bool,
    pub rev: u64,

    pub anim_time: f64,
    /// Sole consumer of this stream is the BlinkController (the original uses
    /// the global `random` module; layouts use their own seeded instances).
    pub rng: PyRng,
    pub blink: BlinkController,
    pub camera: CinematicCamera,

    pub pointer_x: f32,
    pub pointer_y: f32,
    pub pointer_active: bool,

    pub icons: Vec<Rect>,
    pub icons_visible: bool,

    // Cached per-frame outputs (filled by `step`, read by the renderer)
    pub openness: f64,
    pub cam: CamRect,
    pub gaze_x: f64,
    pub gaze_y: f64,
    needs_first_frame: bool,
}

impl Default for EyeWithinWordsState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0, 1.0)
    }
}

impl EyeWithinWordsState {
    pub fn new(css_w: f32, css_h: f32, dpi: f32) -> Self {
        let params = EyeWithinWordsParams::default();
        let mut rng = PyRng::new(params.seed as u64);
        let blink = BlinkController::new(&mut rng);
        let mut st = Self {
            params,
            w_css: css_w.max(64.0),
            h_css: css_h.max(64.0),
            dpr: dpi.max(0.5),
            low_mode: false,
            rev: 1,
            anim_time: 0.0,
            rng,
            blink,
            camera: CinematicCamera::default(),
            pointer_x: css_w * 0.5,
            pointer_y: css_h * 0.5,
            pointer_active: false,
            icons: Vec::new(),
            icons_visible: false,
            openness: 1.0,
            cam: CamRect {
                left: 0,
                top: 0,
                w: 320,
                h: 200,
            },
            gaze_x: 0.0,
            gaze_y: 0.0,
            needs_first_frame: true,
        };
        st.advance(0.001);
        st.needs_first_frame = true;
        st
    }

    pub fn geom(&self) -> EyeCanvasGeometry {
        calculate_eye_spatial_and_color_patterns(
            (self.w_css * self.dpr).round() as i32,
            (self.h_css * self.dpr).round() as i32,
        )
    }

    pub fn resize(&mut self, w: f32, h: f32, dpi: f32) {
        let nw = w.max(64.0);
        let nh = h.max(64.0);
        let ndpi = dpi.max(0.5);
        if (nw - self.w_css).abs() > 0.5
            || (nh - self.h_css).abs() > 0.5
            || (ndpi - self.dpr).abs() > 0.01
        {
            self.w_css = nw;
            self.h_css = nh;
            self.dpr = ndpi;
            self.rev = self.rev.wrapping_add(1);
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.rev = self.rev.wrapping_add(1);
        }
    }

    pub fn sync_params(&mut self, p: EyeWithinWordsParams) {
        if self.params.seed != p.seed {
            // Original R key: regenerate the arrangement of quotes
            self.rng = PyRng::new(p.seed as u64);
        }
        if self.params != p {
            self.params = p;
            self.rev = self.rev.wrapping_add(1);
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

    /// Original SPACE key: trigger an immediate blink (allow_double=false).
    pub fn trigger_blink_now(&mut self) {
        self.blink
            .trigger_blink(self.anim_time, false, &mut self.rng);
    }

    pub fn handle_mouse(&mut self, kind: i32, x: f32, y: f32) -> bool {
        if kind == 3 {
            self.pointer_active = false;
            return false;
        }
        self.pointer_x = x;
        self.pointer_y = y;
        self.pointer_active = true;
        if kind == 1 {
            // Click = SPACE: an immediate blink
            self.trigger_blink_now();
            return true;
        }
        false
    }

    fn mouse_phys(&self) -> (i32, i32) {
        if self.pointer_active {
            (
                (self.pointer_x * self.dpr) as i32,
                (self.pointer_y * self.dpr) as i32,
            )
        } else {
            (0, 0)
        }
    }

    fn advance(&mut self, dt: f32) {
        // Python run loop: dt = min(0.05, max(0.001, now - prev))
        let dt = dt.clamp(0.001, 0.05) as f64;
        self.anim_time += dt;
        self.blink.auto_blink = self.params.auto_blink;
        self.camera.auto_zoom = self.params.auto_zoom;

        self.openness = self.blink.update(self.anim_time, &mut self.rng);
        let geom = self.geom();
        let (cam, gx, gy) = self.camera.manage_zoom_and_camera(
            &geom,
            self.anim_time,
            dt,
            self.mouse_phys(),
        );
        self.cam = cam;
        self.gaze_x = gx;
        self.gaze_y = gy;
        self.needs_first_frame = false;
    }

    pub fn step(&mut self, dt: f32) {
        self.advance(dt);
    }

    pub fn bake_scale(&self) -> f64 {
        let geom = self.geom();
        let mut bs = match self.params.quality {
            0 => 0.65,
            1 => 0.85,
            _ => 1.0,
        };
        if self.low_mode {
            bs *= 0.75;
        }
        // Cap bake textures (2560px on Android to cut RAM & CPU bake work; 4096px on desktop)
        #[cfg(target_os = "android")]
        let max_tex_dim = 2560.0;
        #[cfg(not(target_os = "android"))]
        let max_tex_dim = 4096.0;
        let max_dim = (geom.canvas_w as f64).max(geom.canvas_h as f64) * bs;
        if max_dim > max_tex_dim {
            bs *= max_tex_dim / max_dim;
        }
        bs
    }

    pub fn bake_key(&self) -> BakeKey {
        let geom = self.geom();
        BakeKey {
            screen_w: geom.screen_w,
            screen_h: geom.screen_h,
            palette: self.params.palette,
            seed: self.params.seed,
            bake_scale: (self.bake_scale() * 1000.0).round() as u32,
        }
    }
}

// ============================================================================
// 8. OPENGL 3.3 CORE RENDERER
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy)]
struct SpriteVert {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
}

struct BakeJob {
    key: BakeKey,
    geom_orig: EyeCanvasGeometry,
    bake_scale: f64,
    palette_idx: usize,
}

struct BakeMailbox {
    pending: Option<BakeJob>,
    completed: Option<(BakeKey, BakedEyeLayers)>,
    shutdown: bool,
}

/// Dedicated background worker thread for CPU typography baking so neither the
/// GTK3 settings preview nor KDE Plasma's render thread ever freezes.
/// Rapid resizes or parameter changes overwrite `pending` and coalesce cleanly.
struct AsyncBakeWorker {
    shared: Arc<(Mutex<BakeMailbox>, Condvar)>,
}

impl AsyncBakeWorker {
    fn new() -> Self {
        let shared = Arc::new((
            Mutex::new(BakeMailbox {
                pending: None,
                completed: None,
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let worker_shared = Arc::clone(&shared);
        let _ = std::thread::Builder::new()
            .name("eye-within-words-baker".into())
            .spawn(move || {
                let mut baker = EyeBaker::new();
                let (lock, cvar) = &*worker_shared;
                loop {
                    let job = {
                        let mut guard = match lock.lock() {
                            Ok(g) => g,
                            Err(e) => e.into_inner(),
                        };
                        while guard.pending.is_none() && !guard.shutdown {
                            guard = match cvar.wait(guard) {
                                Ok(g) => g,
                                Err(e) => e.into_inner(),
                            };
                        }
                        if guard.shutdown {
                            return;
                        }
                        guard.pending.take()
                    };
                    let Some(job) = job else {
                        continue;
                    };
                    let layers = baker.bake(
                        &job.key,
                        &job.geom_orig,
                        job.bake_scale,
                        job.palette_idx,
                    );
                    let mut guard = match lock.lock() {
                        Ok(g) => g,
                        Err(e) => e.into_inner(),
                    };
                    // Drop stale result if an even newer job superseded it.
                    if guard.pending.is_none() {
                        guard.completed = Some((job.key, layers));
                    }
                    cvar.notify_all();
                }
            });
        Self { shared }
    }

    fn request(&self, job: BakeJob) {
        let (lock, cvar) = &*self.shared;
        let mut guard = match lock.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        guard.pending = Some(job);
        cvar.notify_all();
    }

    fn poll_completed(&self) -> Option<(BakeKey, BakedEyeLayers)> {
        let (lock, _) = &*self.shared;
        let mut guard = match lock.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        guard.completed.take()
    }

    fn request_and_wait(&self, job: BakeJob) -> Option<(BakeKey, BakedEyeLayers)> {
        let target_key = job.key;
        let (lock, cvar) = &*self.shared;
        let mut guard = match lock.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if let Some((ref k, _)) = guard.completed {
            if *k == target_key {
                return guard.completed.take();
            }
        }
        guard.completed = None;
        guard.pending = Some(job);
        cvar.notify_all();
        while !guard.shutdown {
            if let Some((ref k, _)) = guard.completed {
                if *k == target_key {
                    return guard.completed.take();
                }
            }
            guard = match cvar.wait(guard) {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
        }
        None
    }

    fn shutdown(&self) {
        let (lock, cvar) = &*self.shared;
        let mut guard = match lock.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        guard.shutdown = true;
        guard.pending = None;
        cvar.notify_all();
    }
}

impl Drop for AsyncBakeWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct UploadedEyeMeta {
    geom: EyeCanvasGeometry,
    geom_orig: EyeCanvasGeometry,
    bake_scale: f32,
    iris_dim: i32,
    pupil_dim: i32,
    cutout_dim: i32,
    rim_words: Vec<RimWord>,
}

pub struct EyeWithinWordsRenderer {
    prog_composite: GLuint,
    prog_sprite: GLuint,
    dummy_vao: GLuint,
    sprite_vao: GLuint,
    sprite_vbo: GLuint,
    sprite_buf: Vec<SpriteVert>,

    worker: AsyncBakeWorker,
    requested_key: Option<BakeKey>,
    baked: Option<UploadedEyeMeta>,
    baked_key: Option<BakeKey>,

    tex_layers: [GLuint; 10], // field, sclera, cutout, glow, rings_a, rings_b, radial_a, radial_b, pupil, highlights
    tex_atlas: GLuint,

    // Composite uniforms
    u_res: GLint,
    u_cam: GLint,
    u_canvas: GLint,
    u_eye: GLint,
    u_iris_c: GLint,
    u_hl_pos: GLint,
    u_iris_dim: GLint,
    u_pupil_dim: GLint,
    u_cutout_dim: GLint,
    u_openness: GLint,
    u_now: GLint,
    u_glow_a: GLint,
    u_iris_mix: GLint,
    u_r_scale: GLint,
    u_p_scale: GLint,
    u_sclera_off: GLint,
    u_lid_gaze_y: GLint,
    u_adaptive: GLint,
    u_icon_count: GLint,
    u_icons: GLint,
    u_samplers: [GLint; 10],

    // Sprite uniforms
    u_sp_res: GLint,
    u_sp_tex: GLint,
}

impl EyeWithinWordsRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_composite = gl.compile_program(crate::shaders::FULLSCREEN_VERT, EYE_COMPOSITE_FRAG)?;
        let prog_sprite = gl.compile_program(EYE_SPRITE_VERT, EYE_SPRITE_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let mut sprite_vao = 0;
        let mut sprite_vbo = 0;
        (gl.GenVertexArrays)(1, &mut sprite_vao);
        (gl.GenBuffers)(1, &mut sprite_vbo);
        (gl.BindVertexArray)(sprite_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, sprite_vbo);
        let stride = std::mem::size_of::<SpriteVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, stride, 8 as *const c_void);
        (gl.BindVertexArray)(0);

        let samplers = [
            "u_field", "u_sclera", "u_cutout", "u_glow", "u_rings_a", "u_rings_b",
            "u_radial_a", "u_radial_b", "u_pupil", "u_highlights",
        ];
        let mut u_samplers = [0 as GLint; 10];
        for (i, name) in samplers.iter().enumerate() {
            u_samplers[i] = gl.uniform_loc(prog_composite, name);
        }

        Ok(Self {
            u_res: gl.uniform_loc(prog_composite, "u_res"),
            u_cam: gl.uniform_loc(prog_composite, "u_cam"),
            u_canvas: gl.uniform_loc(prog_composite, "u_canvas"),
            u_eye: gl.uniform_loc(prog_composite, "u_eye"),
            u_iris_c: gl.uniform_loc(prog_composite, "u_iris_c"),
            u_hl_pos: gl.uniform_loc(prog_composite, "u_hl_pos"),
            u_iris_dim: gl.uniform_loc(prog_composite, "u_iris_dim"),
            u_pupil_dim: gl.uniform_loc(prog_composite, "u_pupil_dim"),
            u_cutout_dim: gl.uniform_loc(prog_composite, "u_cutout_dim"),
            u_openness: gl.uniform_loc(prog_composite, "u_openness"),
            u_now: gl.uniform_loc(prog_composite, "u_now"),
            u_glow_a: gl.uniform_loc(prog_composite, "u_glow_a"),
            u_iris_mix: gl.uniform_loc(prog_composite, "u_iris_mix"),
            u_r_scale: gl.uniform_loc(prog_composite, "u_r_scale"),
            u_p_scale: gl.uniform_loc(prog_composite, "u_p_scale"),
            u_sclera_off: gl.uniform_loc(prog_composite, "u_sclera_off"),
            u_lid_gaze_y: gl.uniform_loc(prog_composite, "u_lid_gaze_y"),
            u_adaptive: gl.uniform_loc(prog_composite, "u_adaptive"),
            u_icon_count: gl.uniform_loc(prog_composite, "u_icon_count"),
            u_icons: gl.uniform_loc(prog_composite, "u_icons"),
            u_sp_res: gl.uniform_loc(prog_sprite, "u_res"),
            u_sp_tex: gl.uniform_loc(prog_sprite, "u_tex"),
            u_samplers,
            prog_composite,
            prog_sprite,
            dummy_vao,
            sprite_vao,
            sprite_vbo,
            sprite_buf: Vec::with_capacity(4096),
            worker: AsyncBakeWorker::new(),
            requested_key: None,
            baked: None,
            baked_key: None,
            tex_layers: [0; 10],
            tex_atlas: 0,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        self.worker.shutdown();
        for t in self.tex_layers.iter_mut() {
            if *t != 0 {
                (gl.DeleteTextures)(1, t);
                *t = 0;
            }
        }
        if self.tex_atlas != 0 {
            (gl.DeleteTextures)(1, &self.tex_atlas);
            self.tex_atlas = 0;
        }
        if self.sprite_vbo != 0 { (gl.DeleteBuffers)(1, &self.sprite_vbo); self.sprite_vbo = 0; }
        if self.sprite_vao != 0 { (gl.DeleteVertexArrays)(1, &self.sprite_vao); self.sprite_vao = 0; }
        if self.dummy_vao != 0 { (gl.DeleteVertexArrays)(1, &self.dummy_vao); self.dummy_vao = 0; }
        if self.prog_composite != 0 { (gl.DeleteProgram)(self.prog_composite); self.prog_composite = 0; }
        if self.prog_sprite != 0 { (gl.DeleteProgram)(self.prog_sprite); self.prog_sprite = 0; }
    }

    unsafe fn upload_layer(gl: &GlFns, tex: &mut GLuint, layer: &RgbaLayer) {
        if *tex == 0 {
            (gl.GenTextures)(1, tex);
        }
        (gl.BindTexture)(GL_TEXTURE_2D, *tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            layer.w as GLsizei,
            layer.h as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            layer.px.as_ptr() as *const c_void,
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    }

    unsafe fn upload_baked(&mut self, gl: &GlFns, key: BakeKey, baked: BakedEyeLayers) {
        Self::upload_layer(gl, &mut self.tex_layers[0], &baked.field);
        Self::upload_layer(gl, &mut self.tex_layers[1], &baked.sclera);
        Self::upload_layer(gl, &mut self.tex_layers[2], &baked.cutout);
        Self::upload_layer(gl, &mut self.tex_layers[3], &baked.glow);
        Self::upload_layer(gl, &mut self.tex_layers[4], &baked.rings_a);
        Self::upload_layer(gl, &mut self.tex_layers[5], &baked.rings_b);
        Self::upload_layer(gl, &mut self.tex_layers[6], &baked.radial_a);
        Self::upload_layer(gl, &mut self.tex_layers[7], &baked.radial_b);
        Self::upload_layer(gl, &mut self.tex_layers[8], &baked.pupil);
        Self::upload_layer(gl, &mut self.tex_layers[9], &baked.highlights);
        Self::upload_layer(gl, &mut self.tex_atlas, &baked.rim_atlas);
        self.baked = Some(UploadedEyeMeta {
            geom: baked.geom,
            geom_orig: baked.geom_orig,
            bake_scale: baked.bake_scale,
            iris_dim: baked.iris_dim,
            pupil_dim: baked.pupil_dim,
            cutout_dim: baked.cutout_dim,
            rim_words: baked.rim_words,
        });
        self.baked_key = Some(key);
    }

    unsafe fn ensure_baked(&mut self, gl: &GlFns, ps: &EyeWithinWordsState, sync_bake: bool) {
        if let Some((done_key, layers)) = self.worker.poll_completed() {
            self.upload_baked(gl, done_key, layers);
        }

        let key = ps.bake_key();
        if self.baked_key.as_ref() == Some(&key) {
            return;
        }

        let job = BakeJob {
            key,
            geom_orig: ps.geom(),
            bake_scale: ps.bake_scale(),
            palette_idx: ps.params.palette,
        };

        if sync_bake {
            self.requested_key = Some(key);
            if let Some((done_key, layers)) = self.worker.request_and_wait(job) {
                self.upload_baked(gl, done_key, layers);
            }
        } else if self.requested_key.as_ref() != Some(&key) {
            self.requested_key = Some(key);
            self.worker.request(job);
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);

        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpr = st.dpi_scale;
        let sync_bake = st.sync_bake;
        let ps = &mut st.eye_within_words;
        ps.resize(css_w, css_h, dpr);
        self.ensure_baked(gl, ps, sync_bake);

        let baked = match self.baked.as_ref() {
            Some(b) => b,
            None => {
                // While the first async typography bake completes in the
                // background, clear cleanly to the active palette's midnight
                // canvas background (alpha = 1.0) so the compositor never
                // shows uninitialized memory or freezes.
                let pal = &PALETTES[ps.params.palette.min(PALETTES.len() - 1)];
                (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
                (gl.Viewport)(0, 0, pw, ph);
                (gl.ClearColor)(
                    pal.canvas_bg[0] as f32 / 255.0,
                    pal.canvas_bg[1] as f32 / 255.0,
                    pal.canvas_bg[2] as f32 / 255.0,
                    1.0,
                );
                (gl.Clear)(GL_COLOR_BUFFER_BIT);
                return;
            }
        };
        let geom = &baked.geom; // bake-space geometry
        let bs = baked.bake_scale as f32;
        let orig = &baked.geom_orig;
        let cur_geom = ps.geom();
        let rx = orig.canvas_w as f32 / (cur_geom.canvas_w.max(1) as f32);
        let ry = orig.canvas_h as f32 / (cur_geom.canvas_h.max(1) as f32);

        let openness = ps.openness as f32;
        let now = ps.anim_time as f32;

        // Per-frame scalars from render_fullscreen_composition
        let iris_mix = 0.5 + 0.5 * (now * 0.60).sin();
        let r_scale = 1.0 + 0.025 * (now * 1.3).sin();
        let p_scale = 1.0 + 0.055 * (now * 0.92).sin() + 0.035 * (1.0 - openness);
        let glow_a = (95.0 + 30.0 * (now * 1.2).sin()) / 255.0;
        let sclera_off = (
            py_int(ps.gaze_x * 0.25) as f32 * bs,
            py_int(ps.gaze_y * 0.22) as f32 * bs,
        );
        let iris_c = (
            (orig.center_x as f64 + py_int(ps.gaze_x) as f64) as f32 * bs,
            (orig.center_y as f64 + py_int(ps.gaze_y) as f64
                - py_int(orig.eye_half_h * 0.04) as f64) as f32
                * bs,
        );
        let hl_w = baked.iris_dim as f32;
        let hl_h = baked.iris_dim as f32;
        let bs_d = bs as f64;
        let hl_pos = (
            ((orig.center_x as f64 + py_int(ps.gaze_x * 1.16) as f64 - hl_w as f64 / bs_d / 2.0)
                * bs_d) as f32,
            ((orig.center_y as f64 + py_int(ps.gaze_y * 1.16) as f64
                - py_int(orig.eye_half_h * 0.04) as f64
                - hl_h as f64 / bs_d / 2.0)
                * bs_d) as f32,
        );
        let lid_gaze_y = (ps.gaze_y * 0.20 * ps.openness) as f32 * bs;

        let cam = ps.cam;
        let cam_bake = [
            cam.left as f32 * rx * bs,
            cam.top as f32 * ry * bs,
            cam.w as f32 * rx * bs,
            cam.h as f32 * ry * bs,
        ];

        // ------------------------------------------------------------------
        // PASS 1: Full-screen composition (field + eye interior + camera crop)
        // ------------------------------------------------------------------
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.UseProgram)(self.prog_composite);

        for (i, _) in self.tex_layers.iter().enumerate() {
            (gl.ActiveTexture)(GL_TEXTURE0 + i as GLenum);
            (gl.BindTexture)(GL_TEXTURE_2D, self.tex_layers[i]);
            (gl.Uniform1i)(self.u_samplers[i], i as GLint);
        }

        (gl.Uniform2f)(self.u_res, pw as f32, ph as f32);
        (gl.Uniform4f)(self.u_cam, cam_bake[0], cam_bake[1], cam_bake[2], cam_bake[3]);
        (gl.Uniform2f)(
            self.u_canvas,
            geom.canvas_w as f32,
            geom.canvas_h as f32,
        );
        (gl.Uniform4f)(
            self.u_eye,
            geom.center_x as f32,
            geom.center_y as f32,
            geom.eye_half_w as f32,
            geom.eye_half_h as f32,
        );
        (gl.Uniform2f)(self.u_iris_c, iris_c.0, iris_c.1);
        (gl.Uniform2f)(self.u_hl_pos, hl_pos.0, hl_pos.1);
        (gl.Uniform1f)(self.u_iris_dim, baked.iris_dim as f32);
        (gl.Uniform1f)(self.u_pupil_dim, baked.pupil_dim as f32);
        (gl.Uniform1f)(self.u_cutout_dim, baked.cutout_dim as f32);
        (gl.Uniform1f)(self.u_openness, openness);
        (gl.Uniform1f)(self.u_now, now);
        (gl.Uniform1f)(self.u_glow_a, glow_a);
        (gl.Uniform1f)(self.u_iris_mix, iris_mix);
        (gl.Uniform1f)(self.u_r_scale, r_scale);
        (gl.Uniform1f)(self.u_p_scale, p_scale);
        (gl.Uniform2f)(self.u_sclera_off, sclera_off.0, sclera_off.1);
        (gl.Uniform1f)(self.u_lid_gaze_y, lid_gaze_y);
        (gl.Uniform1i)(self.u_adaptive, if ps.params.adaptive { 1 } else { 0 });

        let ic_cnt = if ps.icons_visible { ps.icons.len().min(24) } else { 0 };
        (gl.Uniform1i)(self.u_icon_count, ic_cnt as GLint);
        if ic_cnt > 0 {
            let dpr = st.dpi_scale.max(0.5);
            let mut ibuf = [0.0f32; 24 * 4];
            for (i, ic) in ps.icons.iter().take(24).enumerate() {
                ibuf[i * 4] = ic.x * dpr;
                ibuf[i * 4 + 1] = ic.y * dpr;
                ibuf[i * 4 + 2] = ic.w * dpr;
                ibuf[i * 4 + 3] = ic.h * dpr;
            }
            (gl.Uniform4fv)(self.u_icons, ic_cnt as GLsizei, ibuf.as_ptr());
        }

        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
        (gl.BindVertexArray)(0);

        // ------------------------------------------------------------------
        // PASS 2: Moving upper & lower eyelid-rim word streams
        // ------------------------------------------------------------------
        let angle_idx = if openness < 0.22 {
            0
        } else if openness < 0.68 {
            1
        } else {
            2
        };

        self.sprite_buf.clear();
        let inv_w = pw as f32 / cam_bake[2].max(1.0);
        let inv_h = ph as f32 / cam_bake[3].max(1.0);
        for w in &baked.rim_words {
            let spr = &w.variants[angle_idx];
            let wx = geom.center_x as f64 + w.u * geom.eye_half_w;
            let (yu, yl) = eval_eyelid_curves(w.u, ps.openness, geom);
            let base_y = if w.is_upper { yu } else { yl };
            let wy = geom.center_y as f64 + base_y + (ps.gaze_y * 0.20 * ps.openness) + w.v_offset;

            let sx = (wx as f32 - cam_bake[0]) * inv_w;
            let sy = (wy as f32 - cam_bake[1]) * inv_h;
            // Sprite pixel size maps 1:1 to bake canvas px; convert through cam.
            let px_per_bake_x = pw as f32 / cam_bake[2];
            let px_per_bake_y = ph as f32 / cam_bake[3];
            let qw = spr.w as f32 * px_per_bake_x;
            let qh = spr.h as f32 * px_per_bake_y;
            let x0 = sx - qw * 0.5;
            let y0 = sy - qh * 0.5;
            let x1 = x0 + qw;
            let y1 = y0 + qh;

            let v = |x: f32, y: f32, u: f32, vv: f32| SpriteVert { x, y, u, v: vv };
            self.sprite_buf.push(v(x0, y0, spr.u0, spr.v0));
            self.sprite_buf.push(v(x1, y0, spr.u1, spr.v0));
            self.sprite_buf.push(v(x1, y1, spr.u1, spr.v1));
            self.sprite_buf.push(v(x0, y0, spr.u0, spr.v0));
            self.sprite_buf.push(v(x1, y1, spr.u1, spr.v1));
            self.sprite_buf.push(v(x0, y1, spr.u0, spr.v1));
        }

        if !self.sprite_buf.is_empty() {
            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
            (gl.UseProgram)(self.prog_sprite);
            (gl.Uniform2f)(self.u_sp_res, pw as f32, ph as f32);
            (gl.ActiveTexture)(GL_TEXTURE0 + 10);
            (gl.BindTexture)(GL_TEXTURE_2D, self.tex_atlas);
            (gl.Uniform1i)(self.u_sp_tex, 10);
            (gl.BindVertexArray)(self.sprite_vao);
            (gl.BindBuffer)(GL_ARRAY_BUFFER, self.sprite_vbo);
            let bytes = (self.sprite_buf.len() * std::mem::size_of::<SpriteVert>()) as GLsizeiptr;
            (gl.BufferData)(
                GL_ARRAY_BUFFER,
                bytes,
                self.sprite_buf.as_ptr() as *const c_void,
                GL_DYNAMIC_DRAW,
            );
            (gl.DrawArrays)(GL_TRIANGLES, 0, self.sprite_buf.len() as GLsizei);
            (gl.BindVertexArray)(0);
            (gl.Disable)(GL_BLEND);
        }
    }
}

// ============================================================================
// 9. UNIT & GPU RENDER TESTS (rust-anything style recorded cases)
// ============================================================================


#[cfg(test)]
mod tests {
    use super::*;
    use crate::eye_within_words::bake::RimSpriteRect;

    fn cases_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/eye_within_words")
            .join(name)
    }

    fn load_cases() -> Vec<Value> {
        std::fs::read_to_string(cases_path("cases.jsonl"))
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn assert_close(actual: f64, expected: f64, tol: f64, what: &str) {
        assert!(
            (actual - expected).abs() <= tol,
            "{what}: got {actual}, expected {expected} (tol {tol})"
        );
    }

    #[test]
    fn verify_rust_anything_recorded_eye_cases() {
        let cases = load_cases();
        assert_eq!(cases.len(), 7);

        for c in &cases {
            let fname = c["fn"].as_str().unwrap();
            match fname {
                "rng" => {
                    let mut rng = PyRng::new(12345);
                    for (k, v) in c["output"]["random"].as_array().unwrap().iter().enumerate() {
                        assert_close(rng.random(), v.as_f64().unwrap(), 1e-12, &format!("random[{k}]"));
                    }
                    for (k, v) in c["output"]["uniform_2_5_4_5"].as_array().unwrap().iter().enumerate() {
                        assert_close(rng.uniform(2.5, 4.5), v.as_f64().unwrap(), 1e-12, &format!("uniform[{k}]"));
                    }
                    for (k, v) in c["output"]["uniform_0_115_0_140"].as_array().unwrap().iter().enumerate() {
                        assert_close(rng.uniform(0.115, 0.140), v.as_f64().unwrap(), 1e-12, &format!("uniform2[{k}]"));
                    }
                    for (k, v) in c["output"]["randint_200_238"].as_array().unwrap().iter().enumerate() {
                        assert_eq!(rng.randint(200, 238), v.as_i64().unwrap(), "randint[{k}]");
                    }
                    for (k, v) in c["output"]["choice"].as_array().unwrap().iter().enumerate() {
                        let items = ["a", "b", "c", "d", "e"];
                        assert_eq!(*rng.choice(&items), v.as_str().unwrap(), "choice[{k}]");
                    }
                }
                "geometry" => {
                    for item in c["output"]["items"].as_array().unwrap() {
                        let sw = item["screen_w"].as_i64().unwrap() as i32;
                        let sh = item["screen_h"].as_i64().unwrap() as i32;
                        let g = calculate_eye_spatial_and_color_patterns(sw, sh);
                        assert_eq!(g.canvas_w, item["canvas_w"].as_i64().unwrap() as i32, "canvas_w {sw}x{sh}");
                        assert_eq!(g.canvas_h, item["canvas_h"].as_i64().unwrap() as i32, "canvas_h {sw}x{sh}");
                        assert_eq!(g.center_x, item["center_x"].as_i64().unwrap() as i32);
                        assert_eq!(g.center_y, item["center_y"].as_i64().unwrap() as i32);
                        assert_close(g.eye_half_w, item["eye_half_w"].as_f64().unwrap(), 1e-9, "eye_half_w");
                        assert_close(g.eye_half_h, item["eye_half_h"].as_f64().unwrap(), 1e-9, "eye_half_h");
                        assert_close(g.iris_radius, item["iris_radius"].as_f64().unwrap(), 1e-9, "iris_radius");
                        assert_close(g.pupil_radius, item["pupil_radius"].as_f64().unwrap(), 1e-9, "pupil_radius");
                        assert_close(g.scale, item["scale"].as_f64().unwrap(), 1e-12, "scale");
                    }
                }
                "eyelid" => {
                    let geom = calculate_eye_spatial_and_color_patterns(1920, 1080);
                    let us: Vec<f64> = c["input"]["u"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
                    let opens: Vec<f64> = c["input"]["openness"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
                    for (oi, o) in opens.iter().enumerate() {
                        for (ui, u) in us.iter().enumerate() {
                            let (yu, yl) = eval_eyelid_curves(*u, *o, &geom);
                            let eyu = c["output"]["y_upper"][oi][ui].as_f64().unwrap();
                            let eyl = c["output"]["y_lower"][oi][ui].as_f64().unwrap();
                            assert_close(yu, eyu, 1e-9, &format!("y_upper o={o} u={u}"));
                            assert_close(yl, eyl, 1e-9, &format!("y_lower o={o} u={u}"));
                        }
                    }
                }
                "streamline" => {
                    let geom = calculate_eye_spatial_and_color_patterns(1920, 1080);
                    let xs: Vec<f64> = c["input"]["x"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
                    let offs: Vec<f64> = c["input"]["base_y_offset"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
                    for (oi, off) in offs.iter().enumerate() {
                        for (xi, x) in xs.iter().enumerate() {
                            let y = eval_field_streamline_y(*x, *off, &geom);
                            let ey = c["output"]["y"][oi][xi].as_f64().unwrap();
                            assert_close(y, ey, 1e-9, &format!("streamline off={off} x={x}"));
                        }
                    }
                }
                "quotes" => {
                    for mode in ["quote", "mixed", "short", "pupil", "highlight"] {
                        let mut rng = PyRng::new(777);
                        for (k, v) in c["output"][mode].as_array().unwrap().iter().enumerate() {
                            let q = generate_meaningful_quotes(&mut rng, mode);
                            assert_eq!(q, v.as_str().unwrap(), "quote {mode}[{k}]");
                        }
                    }
                }
                "blink" => {
                    let mut rng = PyRng::new(1000);
                    let mut bc = BlinkController::new(&mut rng);
                    let mut blink_starts = 0i32;
                    let mut prev_blinking = false;
                    let mut t = 0.0f64;
                    let dt = 0.02f64;
                    let mut sample_idx = 0usize;
                    let samples = c["output"]["samples"].as_array().unwrap();
                    while t <= 12.0 {
                        let o = bc.update(t, &mut rng);
                        if bc.is_blinking && !prev_blinking {
                            blink_starts += 1;
                        }
                        prev_blinking = bc.is_blinking;
                        if sample_idx < samples.len()
                            && (t - samples[sample_idx]["t"].as_f64().unwrap()).abs() < 1e-9
                        {
                            assert_close(
                                o,
                                samples[sample_idx]["openness"].as_f64().unwrap(),
                                1e-9,
                                &format!("blink sample t={t}"),
                            );
                            assert_eq!(
                                bc.is_blinking,
                                samples[sample_idx]["is_blinking"].as_bool().unwrap(),
                                "blink is_blinking t={t}"
                            );
                            sample_idx += 1;
                        }
                        t += dt;
                    }
                    assert_eq!(blink_starts, c["output"]["blink_starts"].as_i64().unwrap() as i32);
                }
                "camera" => {
                    let geom = calculate_eye_spatial_and_color_patterns(1920, 1080);
                    let mut cam = CinematicCamera::default();
                    let dt = c["input"]["dt"].as_f64().unwrap();
                    let frames = c["output"]["samples"].as_array().unwrap();
                    let mut fi = 0usize;
                    for k in 0..900i64 {
                        let now = k as f64 * dt;
                        let (rect, gx, gy) = cam.manage_zoom_and_camera(&geom, now, dt, (400, 300));
                        if fi < frames.len() && frames[fi]["k"].as_i64().unwrap() == k {
                            assert_eq!(rect.left, frames[fi]["left"].as_i64().unwrap() as i32, "cam left k={k}");
                            assert_eq!(rect.top, frames[fi]["top"].as_i64().unwrap() as i32, "cam top k={k}");
                            assert_eq!(rect.w, frames[fi]["w"].as_i64().unwrap() as i32, "cam w k={k}");
                            assert_eq!(rect.h, frames[fi]["h"].as_i64().unwrap() as i32, "cam h k={k}");
                            assert_close(gx, frames[fi]["gaze_x"].as_f64().unwrap(), 1e-9, &format!("gaze_x k={k}"));
                            assert_close(gy, frames[fi]["gaze_y"].as_f64().unwrap(), 1e-9, &format!("gaze_y k={k}"));
                            assert_close(cam.zoom, frames[fi]["zoom"].as_f64().unwrap(), 1e-9, &format!("zoom k={k}"));
                            fi += 1;
                        }
                    }
                    assert_eq!(fi, frames.len(), "all camera frames verified");
                }
                other => panic!("unknown recorded case fn: {other}"),
            }
        }
    }

    #[test]
    fn params_parse_and_bake_key() {
        let mut raw = HashMap::new();
        raw.insert("palette".into(), Value::String("bioluminescentVerse".into()));
        raw.insert("autoBlink".into(), Value::Bool(false));
        raw.insert("autoZoom".into(), Value::Bool(false));
        raw.insert("adaptive".into(), Value::Bool(false));
        raw.insert("quality".into(), Value::String("balanced".into()));
        raw.insert("seed".into(), Value::from(31337));
        let p = EyeWithinWordsParams::from_raw_params(&raw);
        assert_eq!(p.palette, 1);
        assert!(!p.auto_blink);
        assert!(!p.auto_zoom);
        assert!(!p.adaptive);
        assert_eq!(p.quality, 1);
        assert_eq!(p.seed, 31337);

        let mut st = EyeWithinWordsState::new(1280.0, 720.0, 1.0);
        let k0 = st.bake_key();
        st.sync_params(p);
        let k1 = st.bake_key();
        assert_ne!(k0, k1);
        assert_eq!(k1.screen_w, 1280);
        assert_eq!(k1.screen_h, 720);

        // Resize threshold > 0.5 px
        let r0 = st.rev;
        st.resize(1280.2, 720.1, 1.0);
        assert_eq!(st.rev, r0);
        st.resize(1920.0, 1080.0, 1.0);
        assert_ne!(st.rev, r0);

        // Quality scales the bake
        let scale_high = {
            let mut s2 = EyeWithinWordsState::new(1920.0, 1080.0, 1.0);
            s2.sync_params(EyeWithinWordsParams {
                quality: 2,
                ..EyeWithinWordsParams::default()
            });
            s2.bake_scale()
        };
        st.sync_params(EyeWithinWordsParams {
            quality: 0,
            ..EyeWithinWordsParams::default()
        });
        assert!(st.bake_scale() < scale_high, "low quality reduces bake scale");
    }

    #[test]
    fn blink_click_and_pointer_state() {
        let mut st = EyeWithinWordsState::new(1280.0, 720.0, 1.0);
        assert!(!st.blink.is_blinking);
        assert!(st.handle_mouse(1, 100.0, 100.0), "click triggers blink");
        assert!(st.blink.is_blinking);
        st.step(0.02);
        assert!(st.openness < 1.0, "eye should be closing right after click");

        // Pointer updates reach the camera gaze
        st.handle_mouse(0, 640.0, 360.0);
        st.step(0.1);
        assert!(st.pointer_active);

        // Leave disables pointer influence
        st.handle_mouse(3, 0.0, 0.0);
        assert!(!st.pointer_active);
    }

    #[test]
    fn rim_sprite_rect_sizes() {
        let r = RimSpriteRect {
            u0: 0.0,
            v0: 0.0,
            u1: 0.5,
            v1: 0.5,
            w: 120,
            h: 24,
        };
        assert_eq!(r.w, 120);
        assert_eq!(r.h, 24);
    }

    #[test]
    fn layout_is_deterministic_and_rich() {
        let geom = calculate_eye_spatial_and_color_patterns(1280, 720);
        let mut baker = EyeBaker::new();
        let a = baker.bake(
            &BakeKey {
                screen_w: 1280,
                screen_h: 720,
                palette: 0,
                seed: 424242,
                bake_scale: 1000,
            },
            &geom,
            1.0,
            0,
        );
        let b = baker.bake(
            &BakeKey {
                screen_w: 1280,
                screen_h: 720,
                palette: 0,
                seed: 424242,
                bake_scale: 1000,
            },
            &geom,
            1.0,
            0,
        );
        assert_eq!(a.field.px, b.field.px, "field bake must be deterministic");
        assert_eq!(a.sclera.px, b.sclera.px);
        assert_eq!(a.rings_a.px, b.rings_a.px);
        assert_eq!(a.pupil.px, b.pupil.px);
        assert_eq!(a.rim_words.len(), b.rim_words.len());
        assert!(
            a.word_count > 300,
            "expected a rich typography field, got {} words",
            a.word_count
        );
        assert!(!a.rim_words.is_empty(), "eyelid rim words must exist");
        assert!(
            a.rim_words.len() >= 10,
            "expected both lid rim streams, got {}",
            a.rim_words.len()
        );
        assert!(a.rim_words.iter().any(|w| w.is_upper));
        assert!(a.rim_words.iter().any(|w| !w.is_upper));

        // Different palette changes the colors
        let c = baker.bake(
            &BakeKey {
                screen_w: 1280,
                screen_h: 720,
                palette: 1,
                seed: 424242,
                bake_scale: 1000,
            },
            &geom,
            1.0,
            1,
        );
        assert_ne!(a.field.px, c.field.px, "palette must recolor the field");

        // Field layer is opaque and covers the whole canvas
        assert_eq!(a.field.w, a.geom.canvas_w);
        assert_eq!(a.field.h, a.geom.canvas_h);
        let opaque = a
            .field
            .px
            .chunks_exact(4)
            .filter(|p| p[3] == 255)
            .count();
        assert!(
            opaque > (a.field.px.len() / 4) * 95 / 100,
            "field must stay ~opaque edge-to-edge"
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn gpu_render_eye_wide_closeup_and_blink() {
        use std::ffi::CString;
        let _guard = crate::LINUX_EGL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let eng = crate::ddgl_headless_create(1280, 720, 1.0);
            assert!(!eng.is_null());
            let theme_msg = CString::new(r#"{"t":"theme","id":"eye-within-words"}"#).unwrap();
            crate::ddgl_headless_post_json(eng, theme_msg.as_ptr());
            let icons_msg = CString::new(
                r#"{"t":"icons","vis":true,"r":[[60,80,136,170],[60,210,136,300],[180,80,256,170]]}"#,
            )
            .unwrap();
            crate::ddgl_headless_post_json(eng, icons_msg.as_ptr());

            let mut scratch = vec![0u8; 1280 * 720 * 4];

            // 1. Wide open view early in the zoom cycle
            for _ in 0..24 {
                let n = crate::ddgl_headless_render_rgba(
                    eng,
                    0.08,
                    scratch.as_mut_ptr(),
                    scratch.len() as i32,
                );
                assert_eq!(n, scratch.len() as i32);
            }
            let out1 = std::env::temp_dir().join("ddgl_eye_wide.png");
            let c1 = CString::new(out1.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.08, c1.as_ptr()), 1);
            assert!(std::fs::metadata(&out1).map(|m| m.len() > 20_000).unwrap_or(false));

            // Verify real eye typography rendered (not fallback Aurora shader):
            // 1. Alpha channel preserved opaque (255) everywhere
            // 2. Outer typography field has dark midnight background pixels (< 20 luma)
            // 3. Central eye iris/sclera band contains bright crisp cyan/white words (> 180)
            let mut dark_bg = 0usize;
            let mut bright_eye = 0usize;
            let mut cyan_iris = 0usize;
            for (idx, px) in scratch.chunks_exact(4).enumerate() {
                assert_eq!(px[3], 255, "alpha must remain opaque");
                let x = idx % 1280;
                let y = idx / 1280;
                if x < 160 && y < 120 && px[0] <= 18 && px[1] <= 24 && px[2] <= 36 {
                    dark_bg += 1;
                }
                if (360..920).contains(&x) && (220..500).contains(&y) {
                    if px[1] >= 180 || px[2] >= 200 {
                        bright_eye += 1;
                    }
                    if px[1] >= 140 && px[2] >= 160 && px[0] <= 90 {
                        cyan_iris += 1;
                    }
                }
            }
            assert!(
                dark_bg > 2_000,
                "expected midnight typography backdrop in top-left corner, got {dark_bg}"
            );
            assert!(
                bright_eye > 500 && cyan_iris > 400,
                "expected bright cyan/white iris & sclera typography in center eye (bright={bright_eye}, cyan={cyan_iris})"
            );

            // 2. Deep close-up zoom (~8s into the cycle)
            for _ in 0..99 {
                let n = crate::ddgl_headless_render_rgba(
                    eng,
                    0.08,
                    scratch.as_mut_ptr(),
                    scratch.len() as i32,
                );
                assert_eq!(n, scratch.len() as i32);
            }
            let out2 = std::env::temp_dir().join("ddgl_eye_closeup.png");
            let c2 = CString::new(out2.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.08, c2.as_ptr()), 1);
            assert!(std::fs::metadata(&out2).map(|m| m.len() > 20_000).unwrap_or(false));

            // 3. Mid-blink frame via a click (SPACE equivalent)
            let _ = crate::ddgl_headless_mouse(eng, 1, 400.0, 300.0);
            for _ in 0..1 {
                let n = crate::ddgl_headless_render_rgba(
                    eng,
                    0.09,
                    scratch.as_mut_ptr(),
                    scratch.len() as i32,
                );
                assert_eq!(n, scratch.len() as i32);
            }
            let out3 = std::env::temp_dir().join("ddgl_eye_blink.png");
            let c3 = CString::new(out3.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.02, c3.as_ptr()), 1);
            assert!(std::fs::metadata(&out3).map(|m| m.len() > 20_000).unwrap_or(false));

            // Frames must differ (camera motion + blink)
            let f1 = std::fs::read(&out1).unwrap();
            let f2 = std::fs::read(&out2).unwrap();
            let f3 = std::fs::read(&out3).unwrap();
            assert_ne!(f1, f2, "wide and close-up frames must differ");
            assert_ne!(f2, f3, "close-up and blink frames must differ");

            // 4. Palette + quality change triggers a deterministic rebake
            let settings = CString::new(
                r#"{"t":"settings","v":{"palette":"bioluminescentVerse","quality":"low"}}"#,
            )
            .unwrap();
            crate::ddgl_headless_post_json(eng, settings.as_ptr());
            for _ in 0..3 {
                let n = crate::ddgl_headless_render_rgba(
                    eng,
                    0.033,
                    scratch.as_mut_ptr(),
                    scratch.len() as i32,
                );
                assert_eq!(n, scratch.len() as i32);
            }
            let out4 = std::env::temp_dir().join("ddgl_eye_palette.png");
            let c4 = CString::new(out4.to_str().unwrap()).unwrap();
            assert_eq!(crate::ddgl_headless_render_png(eng, 0.033, c4.as_ptr()), 1);
            assert!(std::fs::metadata(&out4).map(|m| m.len() > 20_000).unwrap_or(false));
            let f4 = std::fs::read(&out4).unwrap();
            assert_ne!(f3, f4, "palette change must recolor the typography");

            crate::ddgl_headless_destroy(eng);
        }
    }
}
