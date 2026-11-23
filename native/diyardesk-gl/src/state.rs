use crate::studio_glass::StudioGlassState;
use base64::Engine;
use chrono::{Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        self.w > 0.0 && self.h > 0.0 && px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[derive(Clone, Copy)]
pub struct ThemeParams {
    pub adaptive: bool,
    pub palette: usize,
    pub color_mode: usize, // 0 Auto, 1 Dark, 2 Light
    pub backdrop: usize,   // 0 Blobs, 1 Topo Waves, 2 Starlit
    pub layout: usize,     // 0 Impulse Split, 1 Right Sidebar, 2 Bento Center, 3 Minimal
    pub show_clock: bool,
    pub clock_style: usize, // 0 Cookie Hybrid, 1 DiyarDesk Glance Card
    pub show_media: bool,
    pub show_calendar: bool,
    pub show_tasks: bool,
    pub show_tools: bool,
    pub clock_24: bool,
    pub widget_scale: f32,
    pub card_opacity: f32,
    pub glass_blur: f32,
    pub corner_radius: f32,
    pub icon_style: usize, // 0 Soft Pill, 1 Outlined, 2 Glow Aura, 3 Off
    pub motion: f32,
    pub quality: usize,
    pub seed: u32,
}

impl Default for ThemeParams {
    fn default() -> Self {
        Self {
            adaptive: true,
            palette: 0,
            color_mode: 1,
            backdrop: 0,
            layout: 0,
            show_clock: true,
            clock_style: 0,
            show_media: true,
            show_calendar: true,
            show_tasks: true,
            show_tools: true,
            clock_24: true,
            widget_scale: 1.0,
            card_opacity: 0.78,
            glass_blur: 18.0,
            corner_radius: 26.0,
            icon_style: 0,
            motion: 1.0,
            quality: 1,
            seed: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct PaletteColors {
    pub bg0: [f32; 3],
    pub bg1: [f32; 3],
    pub bg2: [f32; 3],
    pub blobs: [[f32; 3]; 4],
    pub surface: [f32; 3],
    pub on_surface: [f32; 3],
    pub muted: [f32; 3],
    pub outline: [f32; 4],
    pub outline_hi: [f32; 4],
    pub primary: [f32; 3],
    pub on_primary: [f32; 3],
    pub primary_cont: [f32; 4],
    pub secondary: [f32; 3],
    pub tertiary: [f32; 3],
    pub cradle: [f32; 4],
    pub cradle_stroke: [f32; 4],
    pub cookie_face: [f32; 3],
    pub cookie_digit: [f32; 4],
    pub cookie_hour: [f32; 3],
    pub cookie_min: [f32; 3],
    pub cookie_month: [f32; 3],
    pub cookie_day: [f32; 3],
    pub is_light: bool,
}

const fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    ]
}

const fn rgba(hex: u32, a: f32) -> [f32; 4] {
    let c = rgb(hex);
    [c[0], c[1], c[2], a]
}

// DiyarDesk Brand Accents (from deskland.svg): Oasis Turquoise, Golden Sun, Dune Terracotta, Twilight Sky
pub const DIYAR_BRAND_DARK: [[f32; 3]; 4] = [
    rgb(0x32C5B8),
    rgb(0xF5C97A),
    rgb(0xE08B49),
    rgb(0x5C9BD6),
];
pub const DIYAR_BRAND_LIGHT: [[f32; 3]; 4] = [
    rgb(0x148E84),
    rgb(0xC9862A),
    rgb(0xBE6228),
    rgb(0x255885),
];

pub fn resolve_palette(idx: usize, light: bool) -> PaletteColors {
    let (cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day) = match idx {
        1 => (rgb(0x1A4C3E), rgba(0x9EE0C6, 0.54), rgb(0xB3F0D5), rgb(0xECE2A8), rgb(0x46584B), rgb(0x2E3B36)),
        2 => (rgb(0x5C2A32), rgba(0xFFC0B2, 0.54), rgb(0xFFD0C2), rgb(0xF6D59C), rgb(0x634157), rgb(0x443238)),
        3 => (rgb(0x154463), rgba(0x94D2F5, 0.56), rgb(0xA8E0FF), rgb(0xE4C2EA), rgb(0x4B4464), rgb(0x313D48)),
        4 => (rgb(0x2B2F38), rgba(0xD7DEEC, 0.50), rgb(0xE2E6F0), rgb(0xD4C9E6), rgb(0x4B4E5A), rgb(0x3A3E47)),
        _ => (rgb(0x19456B), rgba(0x9BC8EE, 0.56), rgb(0xB4DCFF), rgb(0xE4C2EA), rgb(0x564765), rgb(0x383E48)),
    };

    if !light {
        match idx {
            1 => PaletteColors {
                bg0: rgb(0x0B1411), bg1: rgb(0x11211B), bg2: rgb(0x121D20),
                blobs: [rgb(0x2F8F6B), rgb(0x4BA38B), rgb(0x80A85B), rgb(0x226554)],
                surface: rgb(0x121C18), on_surface: rgb(0xE6F2EC), muted: rgb(0xA4BBB0),
                outline: rgba(0xA2E6C6, 0.19), outline_hi: rgba(0xA2E6C6, 0.36), primary: rgb(0x97E2BF), on_primary: rgb(0x083623),
                primary_cont: rgba(0x369E73, 0.36), secondary: rgb(0xA3D9C9), tertiary: rgb(0xD8D294),
                cradle: rgba(0x97E2BF, 0.15), cradle_stroke: rgba(0x97E2BF, 0.36),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: false,
            },
            2 => PaletteColors {
                bg0: rgb(0x181014), bg1: rgb(0x26161C), bg2: rgb(0x1E1624),
                blobs: [rgb(0xE06A5A), rgb(0xD98A4E), rgb(0xA94F86), rgb(0x9B404D)],
                surface: rgb(0x21161A), on_surface: rgb(0xF8EBE9), muted: rgb(0xC7AEB2),
                outline: rgba(0xFFC0AF, 0.19), outline_hi: rgba(0xFFC0AF, 0.36), primary: rgb(0xFFB5A0), on_primary: rgb(0x521B0F),
                primary_cont: rgba(0xDA6C52, 0.36), secondary: rgb(0xF6CF94), tertiary: rgb(0xE5A8D4),
                cradle: rgba(0xFFB5A0, 0.15), cradle_stroke: rgba(0xFFB5A0, 0.36),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: false,
            },
            3 => PaletteColors {
                bg0: rgb(0x09131C), bg1: rgb(0x0E2030), bg2: rgb(0x0C1B29),
                blobs: [rgb(0x2485C7), rgb(0x21A6A6), rgb(0x4664CC), rgb(0x186496)],
                surface: rgb(0x101B26), on_surface: rgb(0xE6F3FB), muted: rgb(0x9DB7CA),
                outline: rgba(0x96D7FF, 0.19), outline_hi: rgba(0x96D7FF, 0.36), primary: rgb(0x8CD6FF), on_primary: rgb(0x06334F),
                primary_cont: rgba(0x2A90D4, 0.36), secondary: rgb(0x7EE6DA), tertiary: rgb(0xB5C2FF),
                cradle: rgba(0x8CD6FF, 0.15), cradle_stroke: rgba(0x8CD6FF, 0.36),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: false,
            },
            4 => PaletteColors {
                bg0: rgb(0x0E0F12), bg1: rgb(0x17191F), bg2: rgb(0x121318),
                blobs: [rgb(0x525866), rgb(0x3D424E), rgb(0x697080), rgb(0x313640)],
                surface: rgb(0x181A1F), on_surface: rgb(0xF0F2F7), muted: rgb(0xA4AAB6),
                outline: rgba(0xDCE2EE, 0.18), outline_hi: rgba(0xDCE2EE, 0.34), primary: rgb(0xDCE0EA), on_primary: rgb(0x1A1D24),
                primary_cont: rgba(0xAFB6C6, 0.24), secondary: rgb(0xB8C0CF), tertiary: rgb(0xC9C4D4),
                cradle: rgba(0xDCE0EA, 0.13), cradle_stroke: rgba(0xDCE0EA, 0.32),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: false,
            },
            _ => PaletteColors {
                bg0: rgb(0x0E0D18), bg1: rgb(0x17142B), bg2: rgb(0x12192C),
                blobs: [rgb(0x6F52E6), rgb(0x3D8AD4), rgb(0xB85AA4), rgb(0x4D3BB8)],
                surface: rgb(0x171622), on_surface: rgb(0xEFEAF8), muted: rgb(0xB5AECB),
                outline: rgba(0xCDC0FF, 0.19), outline_hi: rgba(0xCDC0FF, 0.38), primary: rgb(0xC9B9FF), on_primary: rgb(0x241459),
                primary_cont: rgba(0x745ADA, 0.38), secondary: rgb(0x8FD5EB), tertiary: rgb(0xF2B6CC),
                cradle: rgba(0xC9B9FF, 0.16), cradle_stroke: rgba(0xC9B9FF, 0.38),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: false,
            },
        }
    } else {
        match idx {
            1 => PaletteColors {
                bg0: rgb(0xEEF7F2), bg1: rgb(0xE1F2E8), bg2: rgb(0xE6F1EE),
                blobs: [rgb(0x8FDCB8), rgb(0xA6D8C8), rgb(0xCDE29E), rgb(0x9ED9BE)],
                surface: rgb(0xF9FEFB), on_surface: rgb(0x14241C), muted: rgb(0x4E665A),
                outline: rgba(0x1F7454, 0.16), outline_hi: rgba(0x1F7454, 0.34), primary: rgb(0x1F7454), on_primary: rgb(0xFFFFFF),
                primary_cont: rgba(0x1F7454, 0.15), secondary: rgb(0x2D7068), tertiary: rgb(0x786E20),
                cradle: rgba(0x1F7454, 0.10), cradle_stroke: rgba(0x1F7454, 0.28),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: true,
            },
            2 => PaletteColors {
                bg0: rgb(0xFDF2EF), bg1: rgb(0xFAE8E3), bg2: rgb(0xF6EAF2),
                blobs: [rgb(0xFFB8A4), rgb(0xFCD49D), rgb(0xEAB4D7), rgb(0xF7A99B)],
                surface: rgb(0xFFFAF8), on_surface: rgb(0x281817), muted: rgb(0x6D5555),
                outline: rgba(0xB94832, 0.16), outline_hi: rgba(0xB94832, 0.34), primary: rgb(0xB94832), on_primary: rgb(0xFFFFFF),
                primary_cont: rgba(0xB94832, 0.15), secondary: rgb(0x96611B), tertiary: rgb(0x8F3F74),
                cradle: rgba(0xB94832, 0.10), cradle_stroke: rgba(0xB94832, 0.28),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: true,
            },
            3 => PaletteColors {
                bg0: rgb(0xEDF6FC), bg1: rgb(0xDFF0FA), bg2: rgb(0xE5F6F5),
                blobs: [rgb(0x8FD0F7), rgb(0x8CE0D8), rgb(0xB0C5FA), rgb(0x99D5F5)],
                surface: rgb(0xF8FDFF), on_surface: rgb(0x12222D), muted: rgb(0x4F6778),
                outline: rgba(0x0E699C, 0.16), outline_hi: rgba(0x0E699C, 0.34), primary: rgb(0x0E699C), on_primary: rgb(0xFFFFFF),
                primary_cont: rgba(0x0E699C, 0.15), secondary: rgb(0x0F7A72), tertiary: rgb(0x4955A8),
                cradle: rgba(0x0E699C, 0.10), cradle_stroke: rgba(0x0E699C, 0.28),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: true,
            },
            4 => PaletteColors {
                bg0: rgb(0xF3F4F7), bg1: rgb(0xE8EAEF), bg2: rgb(0xEFF0F3),
                blobs: [rgb(0xCFD4DF), rgb(0xBFC6D4), rgb(0xDADDE6), rgb(0xC5CBD8)],
                surface: rgb(0xFCFCFE), on_surface: rgb(0x181A20), muted: rgb(0x5D6270),
                outline: rgba(0x2C313D, 0.15), outline_hi: rgba(0x2C313D, 0.32), primary: rgb(0x2C313D), on_primary: rgb(0xFFFFFF),
                primary_cont: rgba(0x2C313D, 0.13), secondary: rgb(0x4B5364), tertiary: rgb(0x5A5666),
                cradle: rgba(0x2C313D, 0.09), cradle_stroke: rgba(0x2C313D, 0.25),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: true,
            },
            _ => PaletteColors {
                bg0: rgb(0xF3EFFD), bg1: rgb(0xE8E4FB), bg2: rgb(0xE2EEF9),
                blobs: [rgb(0xB8A5FF), rgb(0x9CD4F5), rgb(0xF3B7D4), rgb(0xCBBCFF)],
                surface: rgb(0xFCFAFF), on_surface: rgb(0x1D192B), muted: rgb(0x5C5570),
                outline: rgba(0x483880, 0.16), outline_hi: rgba(0x483880, 0.34), primary: rgb(0x5B42C4), on_primary: rgb(0xFFFFFF),
                primary_cont: rgba(0x5B42C4, 0.16), secondary: rgb(0x127692), tertiary: rgb(0xB2416D),
                cradle: rgba(0x5B42C4, 0.11), cradle_stroke: rgba(0x5B42C4, 0.30),
                cookie_face, cookie_digit, cookie_hour, cookie_min, cookie_month, cookie_day, is_light: true,
            },
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CalEvent {
    pub day: u32,
    pub month: u32,
    pub year: i32,
    pub time: String,
    pub title: String,
    pub color_idx: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub text: String,
    pub done: bool,
    pub star: bool,
}

#[derive(Serialize, Deserialize)]
struct PersistedImpulse {
    events: Vec<CalEvent>,
    tasks: Vec<TaskItem>,
    note: String,
    custom_pos: [Option<(f32, f32)>; 5],
}

pub struct DecodedArt {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct MediaInfo {
    pub active: bool,
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub pos: f64,
    pub dur: f64,
    pub sync_at: Instant,
    pub wave_phase: f32,
    pub wave_amp: f32,
    pub play_morph: f32, // 0 = circle (paused), 1 = squircle (playing)
    pub art_angle: f32,
    pub last_art_hash: u64,
    pub pending_art: Option<DecodedArt>,
    pub has_art_tex: bool,
}

impl Default for MediaInfo {
    fn default() -> Self {
        Self {
            active: false,
            playing: false,
            title: "Medya bekleniyor...".into(),
            artist: "Spotify / YouTube oynatın".into(),
            pos: 0.0,
            dur: 0.0,
            sync_at: Instant::now(),
            wave_phase: 0.0,
            wave_amp: 0.0,
            play_morph: 0.0,
            art_angle: 0.0,
            last_art_hash: 0,
            pending_art: None,
            has_art_tex: false,
        }
    }
}

impl MediaInfo {
    pub fn current_pos(&self) -> f64 {
        if !self.active {
            return 0.0;
        }
        let mut p = self.pos;
        if self.playing {
            p += self.sync_at.elapsed().as_secs_f64();
        }
        if self.dur > 0.0 {
            p = p.min(self.dur);
        }
        p.max(0.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WidgetId {
    Clock,
    Media,
    Calendar,
    Tasks,
    Tools,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TextField {
    None,
    Cmd,
    Note,
    TaskNew,
    EventTime,
    EventTitle,
}

#[derive(Clone, Copy)]
pub struct DragInfo {
    pub wid: WidgetId,
    pub offset_x: f32,
    pub offset_y: f32,
}

pub struct AppState {
    pub theme_id: String,
    pub raw_params: HashMap<String, Value>,
    pub width_px: i32,
    pub height_px: i32,
    pub dpi_scale: f32,
    pub paused: bool,
    pub low_mode: bool,
    pub params: ThemeParams,
    pub icons: Vec<Rect>,
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub pointer_active: bool,
    pub env_hour: f32,
    pub cpu: f32,
    pub mem: f32,
    pub battery: f32,
    pub media: MediaInfo,
    // Calendar state
    pub view_year: i32,
    pub view_month: u32,
    pub selected_day: u32,
    pub events: Vec<CalEvent>,
    pub ev_form_open: bool,
    pub ev_time_input: String,
    pub ev_title_input: String,
    // Tasks state
    pub tasks: Vec<TaskItem>,
    pub task_filter: usize, // 0 All, 1 Open, 2 Done
    pub task_input: String,
    // Tools state (3 tabs: 0=Komut, 1=Notlar, 2=Odak)
    pub tools_tab: usize,
    pub cmd_input: String,
    pub cmd_result: String,
    pub cmd_has_apply: bool,
    pub note_text: String,
    pub pomo_running: bool,
    pub pomo_break: bool,
    pub pomo_left_sec: f32,
    // Focus & animation
    pub active_field: TextField,
    pub caret_blink: f32,
    pub clock_hover_alpha: f32,
    // Widget rects (CSS px) & custom user drag positions
    pub rect_clock: Rect,
    pub rect_media: Rect,
    pub rect_cal: Rect,
    pub rect_tasks: Rect,
    pub rect_tools: Rect,
    pub custom_pos: [Option<(f32, f32)>; 5],
    pub drag: Option<DragInfo>,
    pub seeking_media: bool,
    // Interactive sub-rects updated every render
    pub hit_clock_toggle: Rect,
    pub hit_clock_reset: Rect,
    pub hit_media_play: Rect,
    pub hit_media_prev: Rect,
    pub hit_media_next: Rect,
    pub hit_media_wave: Rect,
    pub hit_cal_prev: Rect,
    pub hit_cal_next: Rect,
    pub hit_cal_add_toggle: Rect,
    pub hit_cal_ev_time: Rect,
    pub hit_cal_ev_title: Rect,
    pub hit_cal_ev_submit: Rect,
    pub hit_cal_days: Vec<(u32, Rect)>,
    pub hit_cal_ev_dels: Vec<(usize, Rect)>,
    pub hit_task_tabs: [Rect; 3],
    pub hit_task_toggles: Vec<(usize, Rect)>,
    pub hit_task_stars: Vec<(usize, Rect)>,
    pub hit_task_dels: Vec<(usize, Rect)>,
    pub hit_task_input: Rect,
    pub hit_task_submit: Rect,
    pub hit_tool_tabs: [Rect; 3],
    pub hit_cmd_input: Rect,
    pub hit_cmd_apply: Rect,
    pub hit_cmd_hints: [Rect; 4],
    pub hit_note_area: Rect,
    pub hit_note_to_task: Rect,
    pub hit_pomo_toggle: Rect,
    pub hit_pomo_reset: Rect,
    pub hit_pomo_mode: Rect,
    pub studio: StudioGlassState,
    pub outbox: VecDeque<String>,
    pub last_ui_rects_sig: String,
    pub sim_time: f32,
    pub fps_smooth: f32,
}

fn state_save_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let mut p = PathBuf::from(base);
    p.push("DesklandPreview");
    p.push("impulse-gl-state.json");
    Some(p)
}

impl AppState {
    pub fn new(w: i32, h: i32, dpi: f32) -> Self {
        let now = Local::now();
        let y = now.year();
        let m = now.month();
        let d = now.day();

        let mut events = vec![
            CalEvent { day: d, month: m, year: y, time: "10:30".into(), title: "Tasarım & Sprint Kontrolü".into(), color_idx: 0 },
            CalEvent { day: d, month: m, year: y, time: "13:00".into(), title: "Öğle Yürüyüşü".into(), color_idx: 1 },
            CalEvent { day: d, month: m, year: y, time: "16:30".into(), title: "DiyarDesk Sürüm Planı".into(), color_idx: 2 },
        ];
        let mut tasks = vec![
            TaskItem { text: "Takvim .ics dosyasını içeri aktar".into(), done: false, star: true },
            TaskItem { text: "Masaüstü widget düzenini özelleştir".into(), done: false, star: false },
            TaskItem { text: "25 dk odak oturumunu tamamla".into(), done: true, star: false },
        ];
        let mut note_text = String::new();
        let mut custom_pos = [None; 5];

        if let Some(path) = state_save_path() {
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Ok(saved) = serde_json::from_str::<PersistedImpulse>(&raw) {
                    events = saved.events;
                    tasks = saved.tasks;
                    note_text = saved.note;
                    custom_pos = saved.custom_pos;
                }
            }
        }

        let mut st = Self {
            theme_id: "material-impulse-gl".into(),
            raw_params: HashMap::new(),
            width_px: w.max(64),
            height_px: h.max(64),
            dpi_scale: dpi.max(0.5),
            paused: false,
            low_mode: false,
            params: ThemeParams::default(),
            icons: Vec::new(),
            pointer_x: -999.0,
            pointer_y: -999.0,
            pointer_active: false,
            env_hour: now.hour() as f32 + now.minute() as f32 / 60.0,
            cpu: 12.0,
            mem: 38.0,
            battery: 88.0,
            media: MediaInfo::default(),
            view_year: y,
            view_month: m,
            selected_day: d,
            events,
            ev_form_open: false,
            ev_time_input: "14:00".into(),
            ev_title_input: String::new(),
            tasks,
            task_filter: 0,
            task_input: String::new(),
            tools_tab: 0,
            cmd_input: String::new(),
            cmd_result: String::new(),
            cmd_has_apply: false,
            note_text,
            pomo_running: false,
            pomo_break: false,
            pomo_left_sec: 25.0 * 60.0,
            active_field: TextField::None,
            caret_blink: 0.0,
            clock_hover_alpha: 0.0,
            rect_clock: Rect::default(),
            rect_media: Rect::default(),
            rect_cal: Rect::default(),
            rect_tasks: Rect::default(),
            rect_tools: Rect::default(),
            custom_pos,
            drag: None,
            seeking_media: false,
            hit_clock_toggle: Rect::default(),
            hit_clock_reset: Rect::default(),
            hit_media_play: Rect::default(),
            hit_media_prev: Rect::default(),
            hit_media_next: Rect::default(),
            hit_media_wave: Rect::default(),
            hit_cal_prev: Rect::default(),
            hit_cal_next: Rect::default(),
            hit_cal_add_toggle: Rect::default(),
            hit_cal_ev_time: Rect::default(),
            hit_cal_ev_title: Rect::default(),
            hit_cal_ev_submit: Rect::default(),
            hit_cal_days: Vec::new(),
            hit_cal_ev_dels: Vec::new(),
            hit_task_tabs: [Rect::default(); 3],
            hit_task_toggles: Vec::new(),
            hit_task_stars: Vec::new(),
            hit_task_dels: Vec::new(),
            hit_task_input: Rect::default(),
            hit_task_submit: Rect::default(),
            hit_tool_tabs: [Rect::default(); 3],
            hit_cmd_input: Rect::default(),
            hit_cmd_apply: Rect::default(),
            hit_cmd_hints: [Rect::default(); 4],
            hit_note_area: Rect::default(),
            hit_note_to_task: Rect::default(),
            hit_pomo_toggle: Rect::default(),
            hit_pomo_reset: Rect::default(),
            hit_pomo_mode: Rect::default(),
            studio: StudioGlassState::default(),
            outbox: VecDeque::new(),
            last_ui_rects_sig: String::new(),
            sim_time: 0.0,
            fps_smooth: 60.0,
        };
        st.recompute_layout();
        st
    }

    pub fn save_persisted(&self) {
        if let Some(path) = state_save_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let payload = PersistedImpulse {
                events: self.events.clone(),
                tasks: self.tasks.clone(),
                note: self.note_text.clone(),
                custom_pos: self.custom_pos,
            };
            if let Ok(json) = serde_json::to_string_pretty(&payload) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    pub fn set_active_field(&mut self, next: TextField) {
        let was_focused = self.active_field != TextField::None;
        let is_focused = next != TextField::None;
        self.active_field = next;
        self.caret_blink = 0.0;
        if is_focused || was_focused != is_focused {
            self.outbox.push_back(
                serde_json::json!({ "t": "textFocus", "v": is_focused }).to_string(),
            );
        }
    }

    pub fn css_w(&self) -> f32 {
        (self.width_px as f32 / self.dpi_scale).max(320.0)
    }

    pub fn css_h(&self) -> f32 {
        (self.height_px as f32 / self.dpi_scale).max(200.0)
    }

    pub fn is_light(&self) -> bool {
        match self.params.color_mode {
            1 => false,
            2 => true,
            _ => self.env_hour >= 7.0 && self.env_hour < 18.5,
        }
    }

    pub fn palette_colors(&self) -> PaletteColors {
        resolve_palette(self.params.palette, self.is_light())
    }

    pub fn is_impulse_theme(&self) -> bool {
        self.theme_id == "material-impulse" || self.theme_id == "material-impulse-gl"
    }

    pub fn is_studio_glass_theme(&self) -> bool {
        self.theme_id == "studio-glass"
    }

    pub fn set_theme(&mut self, id: &str) {
        let clean = id.trim();
        if clean.is_empty() {
            return;
        }
        let changed = self.theme_id != clean;
        self.theme_id = clean.to_string();
        if changed {
            self.raw_params.clear();
        }
        self.drag = None;
        self.seeking_media = false;
        self.studio.reset_interaction();
        self.set_active_field(TextField::None);
        // Always clear cached uiRects signature because host clears uiRectGroups on every LoadPage/switch!
        self.last_ui_rects_sig.clear();
        self.recompute_layout();
    }

    pub fn param_f32(&self, key: &str, default: f32) -> f32 {
        match self.raw_params.get(key) {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(default as f64) as f32,
            Some(Value::Bool(b)) => if *b { 1.0 } else { 0.0 },
            _ => default,
        }
    }

    pub fn param_bool(&self, key: &str, default: bool) -> bool {
        match self.raw_params.get(key) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
            _ => default,
        }
    }

    pub fn param_str(&self, key: &str) -> Option<&str> {
        self.raw_params.get(key).and_then(|v| v.as_str())
    }

    pub fn widget_visible(&self, w: WidgetId) -> bool {
        if self.params.layout == 3 {
            return matches!(w, WidgetId::Clock | WidgetId::Media | WidgetId::Tools);
        }
        match w {
            WidgetId::Clock => self.params.show_clock,
            WidgetId::Media => self.params.show_media,
            WidgetId::Calendar => self.params.show_calendar,
            WidgetId::Tasks => self.params.show_tasks,
            WidgetId::Tools => self.params.show_tools,
        }
    }

    pub fn widget_index(w: WidgetId) -> usize {
        match w {
            WidgetId::Clock => 0,
            WidgetId::Media => 1,
            WidgetId::Calendar => 2,
            WidgetId::Tasks => 3,
            WidgetId::Tools => 4,
        }
    }

    pub fn recompute_layout(&mut self) {
        let w = self.css_w();
        let h = self.css_h();
        let s = self.params.widget_scale.clamp(0.75, 1.25);
        let margin = (w.min(h) * 0.032).max(20.0).round();
        let gap = (16.0 * s).round();

        let (cw, ch) = if self.params.clock_style == 0 {
            (236.0 * s, 236.0 * s)
        } else {
            (316.0 * s, 194.0 * s)
        };
        let (mw, mh) = (376.0 * s, 114.0 * s);
        let cal_extra = if self.ev_form_open { 36.0 * s } else { 0.0 };
        let (calw, calh) = (316.0 * s, 344.0 * s + cal_extra);
        let (tkw, tkh) = (316.0 * s, 236.0 * s);
        let tools_h = match self.tools_tab {
            0 => if !self.cmd_result.is_empty() { 154.0 * s } else { 126.0 * s },
            1 => 148.0 * s,
            _ => 126.0 * s,
        };
        let (tlw, tlh) = (396.0 * s, tools_h);

        let mut left_icons = 0usize;
        let mut right_icons = 0usize;
        if self.params.adaptive {
            for ic in &self.icons {
                let cx = ic.x + ic.w * 0.5;
                if cx < w * 0.45 { left_icons += 1; }
                if cx > w * 0.55 { right_icons += 1; }
            }
        }
        let prefer_right = left_icons >= right_icons;

        let (d_clock, d_media, d_cal, d_tasks, d_tools) = match self.params.layout {
            1 => {
                let col_x = (w - calw - margin).max(margin);
                let mut cy = margin;
                let r_clk = (col_x + ((calw - cw) * 0.5).max(0.0), cy);
                if self.widget_visible(WidgetId::Clock) { cy += ch + gap; }
                let r_cal = (col_x, cy);
                if self.widget_visible(WidgetId::Calendar) { cy += calh + gap; }
                let r_tk = (col_x, cy);
                let r_med = ((col_x - mw - gap).max(margin), margin);
                let r_tl = ((col_x - tlw - gap).max(margin), (h - tlh - margin - 46.0).max(margin));
                (r_clk, r_med, r_cal, r_tk, r_tl)
            }
            2 => {
                let lw = cw.max(mw).max(tlw);
                let total = lw + gap + calw;
                let sx = ((w - total) * 0.5).max(margin);
                let ty = (h * 0.07).max(margin);
                let r_clk = (sx + (lw - cw) * 0.5, ty);
                let my = ty + if self.widget_visible(WidgetId::Clock) { ch + gap } else { 0.0 };
                let r_med = (sx, my);
                let r_tl = (sx, my + if self.widget_visible(WidgetId::Media) { mh + gap } else { 0.0 });
                let r_cal = (sx + lw + gap, ty);
                let r_tk = (sx + lw + gap, ty + calh + gap);
                (r_clk, r_med, r_cal, r_tk, r_tl)
            }
            3 => {
                let ty = (h * 0.11).max(margin);
                let r_clk = ((w - cw) * 0.5, ty);
                let r_med = ((w - mw) * 0.5, ty + if self.widget_visible(WidgetId::Clock) { ch + gap } else { 0.0 });
                let r_tl = ((w - tlw) * 0.5, (h - tlh - margin - 48.0).max(margin));
                let r_cal = (w - calw - margin, margin);
                let r_tk = (w - tkw - margin, margin + calh + gap);
                (r_clk, r_med, r_cal, r_tk, r_tl)
            }
            _ => {
                let clock_x = if prefer_right {
                    (margin + if left_icons > 0 { 118.0 } else { 0.0 }).max(w * 0.14)
                } else {
                    (w - cw.max(mw) - margin - if right_icons > 0 { 118.0 } else { 0.0 }).max(margin)
                };
                let stack_x = if prefer_right {
                    (w - calw - margin).max(margin)
                } else {
                    margin
                };
                let ty = (h * 0.07).max(margin);
                let r_clk = (clock_x, ty);
                let r_med = (clock_x, ty + if self.widget_visible(WidgetId::Clock) { ch + gap } else { 0.0 });
                let r_cal = (stack_x, margin);
                let r_tk = (stack_x, margin + if self.widget_visible(WidgetId::Calendar) { calh + gap } else { 0.0 });
                let r_tl = (clock_x.min(w - tlw - margin).max(margin), (h - tlh - margin - 48.0).max(margin));
                (r_clk, r_med, r_cal, r_tk, r_tl)
            }
        };

        let place = |idx: usize, def: (f32, f32), rw: f32, rh: f32| -> Rect {
            let (px, py) = if let Some((rx, ry)) = self.custom_pos[idx] {
                (rx * w, ry * h)
            } else {
                def
            };
            Rect {
                x: px.clamp(8.0, (w - rw - 8.0).max(8.0)).round(),
                y: py.clamp(8.0, (h - rh - 8.0).max(8.0)).round(),
                w: rw.round(),
                h: rh.round(),
            }
        };

        self.rect_clock = place(0, d_clock, cw, ch);
        self.rect_media = place(1, d_media, mw, mh);
        self.rect_cal = place(2, d_cal, calw, calh);
        self.rect_tasks = place(3, d_tasks, tkw, tkh);
        self.rect_tools = place(4, d_tools, tlw, tlh);

        if self.is_studio_glass_theme() {
            self.studio.recompute_hud(w, h);
        }

        self.emit_ui_rects();
    }

    pub fn emit_ui_rects(&mut self) {
        let mut rects: Vec<[i32; 4]> = Vec::new();
        if self.is_impulse_theme() {
            for (wid, r) in [
                (WidgetId::Clock, self.rect_clock),
                (WidgetId::Media, self.rect_media),
                (WidgetId::Calendar, self.rect_cal),
                (WidgetId::Tasks, self.rect_tasks),
                (WidgetId::Tools, self.rect_tools),
            ] {
                if self.widget_visible(wid) && r.w > 4.0 && r.h > 4.0 {
                    rects.push([
                        r.x.floor() as i32,
                        r.y.floor() as i32,
                        r.w.ceil() as i32,
                        r.h.ceil() as i32,
                    ]);
                }
            }
        } else if self.is_studio_glass_theme() {
            rects.extend(self.studio.ui_rects());
        }
        let json = serde_json::json!({
            "t": "uiRects",
            "src": "gl-widgets",
            "r": rects,
        })
        .to_string();
        if json != self.last_ui_rects_sig {
            self.last_ui_rects_sig = json.clone();
            self.outbox.push_back(json);
        }
    }

    pub fn handle_json(&mut self, text: &str) {
        let Ok(v) = serde_json::from_str::<Value>(text) else { return };
        let Some(t) = v.get("t").and_then(|x| x.as_str()) else { return };

        match t {
            "theme" => {
                if let Some(id) = v.get("id").or_else(|| v.get("v")).and_then(|x| x.as_str()) {
                    self.set_theme(id);
                }
            }
            "blurInput" => {
                self.set_active_field(TextField::None);
            }
            "pause" => {
                if let Some(b) = v.get("v").and_then(|x| x.as_bool()) {
                    self.paused = b;
                }
            }
            "lowMode" => {
                if let Some(b) = v.get("v").and_then(|x| x.as_bool()) {
                    self.low_mode = b;
                }
            }
            "settings" => {
                if let Some(obj) = v.get("v").or_else(|| v.get("params")).and_then(|x| x.as_object()) {
                    self.apply_params(obj);
                }
            }
            "reset" => {
                self.params = ThemeParams::default();
                self.raw_params.clear();
                self.custom_pos = [None; 5];
                if let Some(obj) = v.get("v").and_then(|x| x.as_object()) {
                    self.apply_params(obj);
                }
                self.save_persisted();
                self.recompute_layout();
            }
            "icons" => {
                let vis = v.get("vis").and_then(|x| x.as_bool()).unwrap_or(false);
                self.icons.clear();
                if vis {
                    if let Some(arr) = v.get("r").and_then(|x| x.as_array()) {
                        let k = self.dpi_scale.max(0.5);
                        for item in arr {
                            if let Some(r) = item.as_array() {
                                if r.len() >= 4 {
                                    let l = r[0].as_f64().unwrap_or(0.0) as f32 / k;
                                    let top = r[1].as_f64().unwrap_or(0.0) as f32 / k;
                                    let ri = r[2].as_f64().unwrap_or(0.0) as f32 / k;
                                    let b = r[3].as_f64().unwrap_or(0.0) as f32 / k;
                                    self.icons.push(Rect { x: l, y: top, w: (ri - l).max(1.0), h: (b - top).max(1.0) });
                                }
                            }
                        }
                    }
                }
                self.recompute_layout();
            }
            "environment" => {
                if let Some(hr) = v.get("hour").and_then(|x| x.as_f64()) {
                    self.env_hour = hr as f32;
                }
            }
            "systemStats" => {
                if let Some(c) = v.get("cpu").and_then(|x| x.as_f64()) { self.cpu = c as f32; }
                if let Some(m) = v.get("mem").and_then(|x| x.as_f64()) { self.mem = m as f32; }
                if let Some(b) = v.get("battery").and_then(|x| x.as_f64()) { self.battery = b as f32; }
            }
            "mediaState" => {
                self.media.active = v.get("active").and_then(|x| x.as_bool()).unwrap_or(false);
                self.media.playing = v.get("playing").and_then(|x| x.as_bool()).unwrap_or(false);
                let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
                let artist = v.get("artist").and_then(|x| x.as_str()).unwrap_or("").trim();
                let album = v.get("album").and_then(|x| x.as_str()).unwrap_or("").trim();
                self.media.title = if title.is_empty() { "Medya bekleniyor...".into() } else { title.into() };
                self.media.artist = if !artist.is_empty() {
                    artist.into()
                } else if !album.is_empty() {
                    album.into()
                } else {
                    "Spotify / YouTube oynatın".into()
                };
                self.media.pos = v.get("pos").and_then(|x| x.as_f64()).unwrap_or(0.0);
                self.media.dur = v.get("dur").and_then(|x| x.as_f64()).unwrap_or(0.0);
                self.media.sync_at = Instant::now();

                let art = v.get("art").and_then(|x| x.as_str()).unwrap_or("");
                if art.is_empty() {
                    self.media.last_art_hash = 0;
                    self.media.has_art_tex = false;
                } else {
                    let h = simple_hash(art.as_bytes());
                    if h != self.media.last_art_hash {
                        self.media.last_art_hash = h;
                        if let Some(decoded) = decode_data_url_image(art) {
                            self.media.pending_art = Some(decoded);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_params(&mut self, obj: &serde_json::Map<String, Value>) {
        for (k, val) in obj {
            self.raw_params.insert(k.clone(), val.clone());
        }
        let num = |k: &str| obj.get(k).and_then(|x| x.as_f64());
        if let Some(v) = num("adaptive") { self.params.adaptive = v != 0.0; }
        if let Some(v) = num("palette") { self.params.palette = (v as usize).min(4); }
        if let Some(v) = num("colorMode") { self.params.color_mode = (v as usize).min(2); }
        if let Some(v) = num("backdrop") { self.params.backdrop = (v as usize).min(2); }
        if let Some(v) = num("layout") { self.params.layout = (v as usize).min(3); }
        if let Some(v) = num("showClock") { self.params.show_clock = v != 0.0; }
        if let Some(v) = num("clockStyle") { self.params.clock_style = (v as usize).min(1); }
        if let Some(v) = num("showMedia") { self.params.show_media = v != 0.0; }
        if let Some(v) = num("showCalendar") { self.params.show_calendar = v != 0.0; }
        if let Some(v) = num("showTasks") { self.params.show_tasks = v != 0.0; }
        if let Some(v) = num("showTools") { self.params.show_tools = v != 0.0; }
        if let Some(v) = num("clock24") { self.params.clock_24 = v != 0.0; }
        if let Some(v) = num("widgetScale") { self.params.widget_scale = (v as f32).clamp(0.75, 1.25); }
        if let Some(v) = num("cardOpacity") { self.params.card_opacity = (v as f32).clamp(0.35, 0.96); }
        if let Some(v) = num("glassBlur") { self.params.glass_blur = (v as f32).clamp(0.0, 32.0); }
        if let Some(v) = num("cornerRadius") { self.params.corner_radius = (v as f32).clamp(12.0, 36.0); }
        if let Some(v) = num("iconStyle") { self.params.icon_style = (v as usize).min(3); }
        if let Some(v) = num("motion") { self.params.motion = (v as f32).clamp(0.0, 2.0); }
        if let Some(v) = num("quality") { self.params.quality = (v as usize).min(2); }
        if let Some(v) = num("seed") { self.params.seed = v as u32; }

        if self.is_studio_glass_theme() {
            let st_preset = self.param_f32("preset", 1.0).round() as i32;
            let blinds = self.param_f32("blinds", 0.70);
            let shafts = self.param_f32("shafts", 0.80);
            let weather = self.param_f32("weather", 0.78);
            let mon_ang = self.param_f32("monitorAngle", -10.0);
            let lamp = self.param_f32("deskLamp", 0.70);
            let par = self.param_f32("parallax", 0.55);
            let refr = self.param_f32("refraction", 0.85);
            let frost = self.param_f32("frost", 0.50);
            let ic_glass = self.param_f32("iconGlass", 0.0).round() as i32;
            let show_hud = self.param_bool("showHud", true);
            self.studio.sync_from_raw_params(st_preset, blinds, shafts, weather, mon_ang, lamp, par, refr, frost, ic_glass, show_hud);
        }
        self.recompute_layout();
    }

    pub fn step(&mut self, dt: f32) {
        self.sim_time += dt * self.params.motion;
        self.caret_blink = (self.caret_blink + dt) % 1.06;
        if dt > 0.001 {
            let inst_fps = (1.0 / dt).clamp(1.0, 240.0);
            self.fps_smooth = self.fps_smooth * 0.9 + inst_fps * 0.1;
        }
        let playing = self.media.active && self.media.playing;
        let target_amp = if playing { 3.2 } else { 0.0 };
        self.media.wave_amp += (target_amp - self.media.wave_amp) * (dt * 8.0).min(1.0);
        let target_morph = if playing { 1.0 } else { 0.0 };
        self.media.play_morph += (target_morph - self.media.play_morph) * (dt * 11.0).min(1.0);
        if playing {
            self.media.wave_phase += dt * 4.2;
            if !self.media.has_art_tex {
                self.media.art_angle += dt * 0.35;
            }
        }
        let clock_hov = self.pointer_active && self.rect_clock.contains(self.pointer_x, self.pointer_y);
        let target_ch = if clock_hov { 1.0 } else { 0.0 };
        self.clock_hover_alpha += (target_ch - self.clock_hover_alpha) * (dt * 10.0).min(1.0);

        if self.pomo_running && self.pomo_left_sec > 0.0 {
            self.pomo_left_sec -= dt;
            if self.pomo_left_sec <= 0.0 {
                self.pomo_running = false;
                self.pomo_break = !self.pomo_break;
                self.pomo_left_sec = if self.pomo_break { 5.0 * 60.0 } else { 25.0 * 60.0 };
            }
        }

        if self.is_studio_glass_theme() {
            let cw = self.css_w();
            let ch = self.css_h();
            let px = self.pointer_x;
            let py = self.pointer_y;
            let pa = self.pointer_active;
            self.studio.step(dt, px, py, pa, cw, ch);
        }
    }

    pub fn eval_cmd(&mut self) {
        let prev_empty = self.cmd_result.is_empty();
        let q = self.cmd_input.trim();
        self.cmd_has_apply = false;
        if q.is_empty() {
            self.cmd_result.clear();
            if !prev_empty { self.recompute_layout(); }
            return;
        }
        let ql = q.to_lowercase();
        if ql.starts_with("/todo ") {
            let body = q[6..].trim();
            if !body.is_empty() {
                self.cmd_result = format!("Yeni görev: {body}");
                self.cmd_has_apply = true;
                if prev_empty { self.recompute_layout(); }
                return;
            }
        }
        if let Some(conv) = eval_unit_conversion(q) {
            self.cmd_result = conv;
            if prev_empty { self.recompute_layout(); }
            return;
        }
        if let Some(val) = eval_math_expr(q) {
            let s = format!("{val:.6}");
            let trimmed = s.trim_end_matches('0').trim_end_matches('.');
            self.cmd_result = format!("= {trimmed}");
            if prev_empty { self.recompute_layout(); }
            return;
        }
        // Offline search in events & tasks
        if let Some(ev) = self.events.iter().find(|e| e.title.to_lowercase().contains(&ql)) {
            self.cmd_result = format!("{} · {}", ev.time, ev.title);
        } else if let Some(tk) = self.tasks.iter().find(|t| t.text.to_lowercase().contains(&ql)) {
            self.cmd_result = format!("{} {}", if tk.done { "✓" } else { "○" }, tk.text);
        } else {
            self.cmd_result.clear();
        }
        if prev_empty != self.cmd_result.is_empty() {
            self.recompute_layout();
        }
    }

    pub fn apply_cmd_todo(&mut self) {
        let q = self.cmd_input.trim().to_string();
        if q.to_lowercase().starts_with("/todo ") {
            let body = q[6..].trim().to_string();
            if !body.is_empty() {
                self.add_task(body);
                self.cmd_input.clear();
                self.cmd_has_apply = false;
                self.cmd_result = "Görevlere eklendi ✓".into();
                self.recompute_layout();
            }
        }
    }

    pub fn add_task(&mut self, text: String) {
        let clean = text.trim();
        if clean.is_empty() { return; }
        self.tasks.insert(0, TaskItem {
            text: clean.chars().take(72).collect(),
            done: false,
            star: false,
        });
        self.save_persisted();
    }

    pub fn submit_new_event(&mut self) {
        let title = self.ev_title_input.trim().to_string();
        if title.is_empty() { return; }
        let raw_time = self.ev_time_input.trim().to_string();
        let time = if raw_time.is_empty() { "Tüm gün".into() } else { raw_time };
        self.events.push(CalEvent {
            day: self.selected_day,
            month: self.view_month,
            year: self.view_year,
            time,
            title: title.chars().take(48).collect(),
            color_idx: self.events.len() % 4,
        });
        self.ev_title_input.clear();
        self.ev_form_open = false;
        self.set_active_field(TextField::None);
        self.save_persisted();
        self.recompute_layout();
    }

    // kind: 0 = text insert, 1 = special key (vk: 0x08 Backspace, 0x0D Enter, 0x1B Escape, 0x01 Ctrl+A clear)
    pub fn handle_key(&mut self, kind: i32, vk: i32, text: &str) {
        if self.active_field == TextField::None {
            return;
        }
        self.caret_blink = 0.0;
        if kind == 1 {
            match vk {
                0x1B => {
                    self.set_active_field(TextField::None);
                }
                0x08 | 0x2E => {
                    match self.active_field {
                        TextField::Cmd => { self.cmd_input.pop(); self.eval_cmd(); }
                        TextField::Note => { self.note_text.pop(); self.save_persisted(); }
                        TextField::TaskNew => { self.task_input.pop(); }
                        TextField::EventTime => { self.ev_time_input.pop(); }
                        TextField::EventTitle => { self.ev_title_input.pop(); }
                        TextField::None => {}
                    }
                }
                0x41 => {
                    // Ctrl+A: clear active field for quick replacement
                    match self.active_field {
                        TextField::Cmd => { self.cmd_input.clear(); self.eval_cmd(); }
                        TextField::Note => { self.note_text.clear(); self.save_persisted(); }
                        TextField::TaskNew => { self.task_input.clear(); }
                        TextField::EventTime => { self.ev_time_input.clear(); }
                        TextField::EventTitle => { self.ev_title_input.clear(); }
                        TextField::None => {}
                    }
                }
                0x0D => {
                    match self.active_field {
                        TextField::Cmd => {
                            if self.cmd_has_apply { self.apply_cmd_todo(); }
                        }
                        TextField::Note => {
                            if self.note_text.chars().count() < 260 {
                                self.note_text.push('\n');
                                self.save_persisted();
                            }
                        }
                        TextField::TaskNew => {
                            let s = std::mem::take(&mut self.task_input);
                            self.add_task(s);
                        }
                        TextField::EventTime => {
                            self.set_active_field(TextField::EventTitle);
                        }
                        TextField::EventTitle => {
                            self.submit_new_event();
                        }
                        TextField::None => {}
                    }
                }
                _ => {}
            }
            return;
        }

        if kind == 0 && !text.is_empty() {
            match self.active_field {
                TextField::Cmd => {
                    for ch in text.chars() {
                        if !ch.is_control() && self.cmd_input.chars().count() < 64 {
                            self.cmd_input.push(ch);
                        }
                    }
                    self.eval_cmd();
                }
                TextField::Note => {
                    for ch in text.chars() {
                        if (!ch.is_control() || ch == '\n') && self.note_text.chars().count() < 260 {
                            self.note_text.push(ch);
                        }
                    }
                    self.save_persisted();
                }
                TextField::TaskNew => {
                    for ch in text.chars() {
                        if !ch.is_control() && self.task_input.chars().count() < 64 {
                            self.task_input.push(ch);
                        }
                    }
                }
                TextField::EventTime => {
                    for ch in text.chars() {
                        if (ch.is_ascii_digit() || ch == ':') && self.ev_time_input.chars().count() < 5 {
                            self.ev_time_input.push(ch);
                        }
                    }
                }
                TextField::EventTitle => {
                    for ch in text.chars() {
                        if !ch.is_control() && self.ev_title_input.chars().count() < 48 {
                            self.ev_title_input.push(ch);
                        }
                    }
                }
                TextField::None => {}
            }
        }
    }

    // kind: 0=move, 1=down, 2=up, 3=leave
    pub fn handle_mouse(&mut self, kind: i32, x: f32, y: f32) -> bool {
        if kind == 3 {
            self.pointer_active = false;
            self.drag = None;
            self.seeking_media = false;
            self.studio.handle_mouse(3, 0.0, 0.0, 1.0, 1.0);
            return false;
        }
        self.pointer_x = x;
        self.pointer_y = y;
        self.pointer_active = true;

        if self.is_studio_glass_theme() {
            let cw = self.css_w();
            let ch = self.css_h();
            let (handled, rects_changed) = self.studio.handle_mouse(kind, x, y, cw, ch);
            if rects_changed {
                self.emit_ui_rects();
            }
            return handled;
        }

        if kind == 0 {
            if self.seeking_media {
                self.seek_media_at(x);
                return true;
            }
            if let Some(d) = self.drag {
                let w = self.css_w();
                let h = self.css_h();
                let idx = Self::widget_index(d.wid);
                let rect = self.rect_mut(d.wid);
                let nx = (x - d.offset_x).clamp(8.0, (w - rect.w - 8.0).max(8.0)).round();
                let ny = (y - d.offset_y).clamp(8.0, (h - rect.h - 8.0).max(8.0)).round();
                rect.x = nx;
                rect.y = ny;
                self.custom_pos[idx] = Some((nx / w.max(1.0), ny / h.max(1.0)));
                return true;
            }
            return false;
        }

        if kind == 2 {
            let was_drag = self.drag.is_some();
            let was_active = was_drag || self.seeking_media;
            self.drag = None;
            self.seeking_media = false;
            if was_drag {
                self.save_persisted();
                self.emit_ui_rects();
            }
            return was_active;
        }

        if kind == 1 && !self.is_impulse_theme() {
            return false;
        }

        // ---- kind == 1 (mouse down) ----
        // 1. Check text input fields first!
        if self.widget_visible(WidgetId::Tools) && self.tools_tab == 0 && self.hit_cmd_input.contains(x, y) {
            self.set_active_field(TextField::Cmd);
            return true;
        }
        if self.widget_visible(WidgetId::Tools) && self.tools_tab == 1 && self.hit_note_area.contains(x, y) {
            self.set_active_field(TextField::Note);
            return true;
        }
        if self.widget_visible(WidgetId::Tasks) && self.hit_task_input.contains(x, y) {
            self.set_active_field(TextField::TaskNew);
            return true;
        }
        if self.widget_visible(WidgetId::Calendar) && self.ev_form_open {
            if self.hit_cal_ev_time.contains(x, y) {
                self.set_active_field(TextField::EventTime);
                return true;
            }
            if self.hit_cal_ev_title.contains(x, y) {
                self.set_active_field(TextField::EventTitle);
                return true;
            }
            if self.hit_cal_ev_submit.contains(x, y) {
                self.submit_new_event();
                return true;
            }
        }

        // Clicking anywhere else clears active text focus (except submit buttons handled below)
        let keep_focus_btn = (self.widget_visible(WidgetId::Tasks) && self.hit_task_submit.contains(x, y))
            || (self.widget_visible(WidgetId::Tools) && self.tools_tab == 0 && self.hit_cmd_apply.contains(x, y));
        if !keep_focus_btn && self.active_field != TextField::None {
            self.set_active_field(TextField::None);
        }

        // 2. Clock buttons
        if self.widget_visible(WidgetId::Clock) {
            if self.hit_clock_toggle.contains(x, y) {
                self.params.clock_style = if self.params.clock_style == 0 { 1 } else { 0 };
                self.recompute_layout();
                return true;
            }
            if self.hit_clock_reset.contains(x, y) {
                self.custom_pos = [None; 5];
                self.save_persisted();
                self.recompute_layout();
                return true;
            }
        }

        // 3. Music player controls
        if self.widget_visible(WidgetId::Media) {
            if self.hit_media_play.contains(x, y) {
                self.outbox.push_back(r#"{"t":"mediaCmd","action":"toggle","pos":0}"#.into());
                return true;
            }
            if self.hit_media_prev.contains(x, y) {
                self.outbox.push_back(r#"{"t":"mediaCmd","action":"prev","pos":0}"#.into());
                return true;
            }
            if self.hit_media_next.contains(x, y) {
                self.outbox.push_back(r#"{"t":"mediaCmd","action":"next","pos":0}"#.into());
                return true;
            }
            if self.hit_media_wave.contains(x, y) {
                self.seeking_media = true;
                self.seek_media_at(x);
                return true;
            }
        }

        // 4. Calendar controls
        if self.widget_visible(WidgetId::Calendar) {
            if self.hit_cal_add_toggle.contains(x, y) {
                self.ev_form_open = !self.ev_form_open;
                if self.ev_form_open {
                    self.set_active_field(TextField::EventTitle);
                } else {
                    self.set_active_field(TextField::None);
                }
                self.recompute_layout();
                return true;
            }
            if self.hit_cal_prev.contains(x, y) {
                if self.view_month == 1 { self.view_month = 12; self.view_year -= 1; }
                else { self.view_month -= 1; }
                return true;
            }
            if self.hit_cal_next.contains(x, y) {
                if self.view_month == 12 { self.view_month = 1; self.view_year += 1; }
                else { self.view_month += 1; }
                return true;
            }
            for &(day, r) in &self.hit_cal_days {
                if r.contains(x, y) {
                    self.selected_day = day;
                    return true;
                }
            }
            let del_hit = self.hit_cal_ev_dels.iter().find(|(_, r)| r.contains(x, y)).map(|(i, _)| *i);
            if let Some(idx) = del_hit {
                if idx < self.events.len() {
                    self.events.remove(idx);
                    self.save_persisted();
                }
                return true;
            }
        }

        // 5. Tasks controls
        if self.widget_visible(WidgetId::Tasks) {
            for (ti, tr) in self.hit_task_tabs.iter().enumerate() {
                if tr.contains(x, y) {
                    self.task_filter = ti;
                    return true;
                }
            }
            if self.hit_task_submit.contains(x, y) {
                let s = std::mem::take(&mut self.task_input);
                self.add_task(s);
                return true;
            }
            let del_hit = self.hit_task_dels.iter().find(|(_, r)| r.contains(x, y)).map(|(i, _)| *i);
            if let Some(idx) = del_hit {
                if idx < self.tasks.len() {
                    self.tasks.remove(idx);
                    self.save_persisted();
                }
                return true;
            }
            let star_hit = self.hit_task_stars.iter().find(|(_, r)| r.contains(x, y)).map(|(i, _)| *i);
            if let Some(idx) = star_hit {
                if let Some(tk) = self.tasks.get_mut(idx) {
                    tk.star = !tk.star;
                    self.save_persisted();
                }
                return true;
            }
            let tog_hit = self.hit_task_toggles.iter().find(|(_, r)| r.contains(x, y)).map(|(i, _)| *i);
            if let Some(idx) = tog_hit {
                if let Some(tk) = self.tasks.get_mut(idx) {
                    tk.done = !tk.done;
                    self.save_persisted();
                }
                return true;
            }
        }

        // 6. Tools tabs & buttons
        if self.widget_visible(WidgetId::Tools) {
            for (ti, tr) in self.hit_tool_tabs.iter().enumerate() {
                if tr.contains(x, y) {
                    self.tools_tab = ti;
                    self.recompute_layout();
                    return true;
                }
            }
            if self.tools_tab == 0 {
                if self.cmd_has_apply && self.hit_cmd_apply.contains(x, y) {
                    self.apply_cmd_todo();
                    return true;
                }
                const HINTS: [&str; 4] = ["/todo ", "1920 / 16 * 9", "22 c to f", "10 km to mi"];
                for (i, hr) in self.hit_cmd_hints.iter().enumerate() {
                    if hr.contains(x, y) {
                        self.cmd_input = HINTS[i].into();
                        self.eval_cmd();
                        self.set_active_field(TextField::Cmd);
                        return true;
                    }
                }
            } else if self.tools_tab == 1 {
                if self.hit_note_to_task.contains(x, y) {
                    if let Some(line) = self.note_text.lines().map(|l| l.trim()).find(|l| !l.is_empty()) {
                        let s = line.to_string();
                        self.add_task(s);
                    }
                    return true;
                }
            } else if self.tools_tab == 2 {
                if self.hit_pomo_toggle.contains(x, y) {
                    self.pomo_running = !self.pomo_running;
                    return true;
                }
                if self.hit_pomo_reset.contains(x, y) {
                    self.pomo_running = false;
                    self.pomo_left_sec = if self.pomo_break { 5.0 * 60.0 } else { 25.0 * 60.0 };
                    return true;
                }
                if self.hit_pomo_mode.contains(x, y) {
                    self.pomo_running = false;
                    self.pomo_break = !self.pomo_break;
                    self.pomo_left_sec = if self.pomo_break { 5.0 * 60.0 } else { 25.0 * 60.0 };
                    return true;
                }
            }
        }

        // 7. Drag whichever visible widget card was clicked
        for (wid, r) in [
            (WidgetId::Tools, self.rect_tools),
            (WidgetId::Tasks, self.rect_tasks),
            (WidgetId::Calendar, self.rect_cal),
            (WidgetId::Media, self.rect_media),
            (WidgetId::Clock, self.rect_clock),
        ] {
            if self.widget_visible(wid) && r.contains(x, y) {
                self.drag = Some(DragInfo {
                    wid,
                    offset_x: x - r.x,
                    offset_y: y - r.y,
                });
                return true;
            }
        }
        false
    }

    fn seek_media_at(&mut self, x: f32) {
        if self.media.dur <= 0.0 || self.hit_media_wave.w <= 1.0 {
            return;
        }
        let frac = ((x - self.hit_media_wave.x) / self.hit_media_wave.w).clamp(0.0, 1.0);
        let pos = frac as f64 * self.media.dur;
        self.media.pos = pos;
        self.media.sync_at = Instant::now();
        self.outbox.push_back(
            serde_json::json!({ "t": "mediaCmd", "action": "seek", "pos": (pos * 10.0).round() / 10.0 })
                .to_string(),
        );
    }

    fn rect_mut(&mut self, w: WidgetId) -> &mut Rect {
        match w {
            WidgetId::Clock => &mut self.rect_clock,
            WidgetId::Media => &mut self.rect_media,
            WidgetId::Calendar => &mut self.rect_cal,
            WidgetId::Tasks => &mut self.rect_tasks,
            WidgetId::Tools => &mut self.rect_tools,
        }
    }
}

fn eval_unit_conversion(q: &str) -> Option<String> {
    let parts: Vec<&str> = q.split_whitespace().collect();
    if parts.len() != 4 {
        return None;
    }
    let sep = parts[2].to_lowercase();
    if sep != "to" && sep != "->" && sep != "→" {
        return None;
    }
    let n: f64 = parts[0].replace(',', ".").parse().ok()?;
    let a = parts[1].to_lowercase();
    let b = parts[3].to_lowercase();
    let out = match (a.as_str(), b.as_str()) {
        ("km", "mi") => format!("{:.2} mi", n * 0.621371),
        ("mi", "km") => format!("{:.2} km", n / 0.621371),
        ("m", "ft") => format!("{:.2} ft", n * 3.28084),
        ("ft", "m") => format!("{:.2} m", n / 3.28084),
        ("kg", "lb") => format!("{:.2} lb", n * 2.20462),
        ("lb", "kg") => format!("{:.2} kg", n / 2.20462),
        ("c", "f") => format!("{:.1} °F", n * 9.0 / 5.0 + 32.0),
        ("f", "c") => format!("{:.1} °C", (n - 32.0) * 5.0 / 9.0),
        _ => return None,
    };
    Some(format!("{n} {} = {out}", a.to_uppercase()))
}

fn eval_math_expr(expr: &str) -> Option<f64> {
    let cleaned: String = expr.replace(',', ".").chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() || !cleaned.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if !cleaned.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | '*' | '/' | '(' | ')' | '%' | '^')) {
        return None;
    }
    let bytes = cleaned.as_bytes();
    let mut pos = 0usize;

    fn parse_expr(b: &[u8], i: &mut usize) -> Option<f64> {
        let mut v = parse_term(b, i)?;
        while *i < b.len() && (b[*i] == b'+' || b[*i] == b'-') {
            let op = b[*i];
            *i += 1;
            let r = parse_term(b, i)?;
            v = if op == b'+' { v + r } else { v - r };
        }
        Some(v)
    }
    fn parse_term(b: &[u8], i: &mut usize) -> Option<f64> {
        let mut v = parse_pow(b, i)?;
        while *i < b.len() && matches!(b[*i], b'*' | b'/' | b'%') {
            let op = b[*i];
            *i += 1;
            let r = parse_pow(b, i)?;
            v = match op {
                b'*' => v * r,
                b'/' => v / r,
                _ => v % r,
            };
        }
        Some(v)
    }
    fn parse_pow(b: &[u8], i: &mut usize) -> Option<f64> {
        let mut v = parse_unary(b, i)?;
        if *i < b.len() && b[*i] == b'^' {
            *i += 1;
            v = v.powf(parse_pow(b, i)?);
        }
        Some(v)
    }
    fn parse_unary(b: &[u8], i: &mut usize) -> Option<f64> {
        if *i < b.len() && b[*i] == b'-' { *i += 1; return Some(-parse_unary(b, i)?); }
        if *i < b.len() && b[*i] == b'+' { *i += 1; return parse_unary(b, i); }
        if *i < b.len() && b[*i] == b'(' {
            *i += 1;
            let v = parse_expr(b, i)?;
            if *i < b.len() && b[*i] == b')' { *i += 1; } else { return None; }
            return Some(v);
        }
        let start = *i;
        while *i < b.len() && (b[*i].is_ascii_digit() || b[*i] == b'.') { *i += 1; }
        if *i == start { return None; }
        std::str::from_utf8(&b[start..*i]).ok()?.parse::<f64>().ok()
    }

    let val = parse_expr(bytes, &mut pos)?;
    if pos == bytes.len() && val.is_finite() { Some(val) } else { None }
}

fn simple_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

fn decode_data_url_image(data_url: &str) -> Option<DecodedArt> {
    let comma = data_url.find(',')?;
    let b64 = &data_url[comma + 1..];
    let raw = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let img = image::load_from_memory(&raw).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(DecodedArt {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}
