use crate::ai_mascots::{AiMascotsParams, AiMascotsState};
use crate::aurora::{AuroraParams, AuroraState};
use crate::beach::{BeachParams, BeachState};
use crate::cat_shore::{CatShoreParams, CatShoreState};
use crate::claymorph::{ClaymorphParams, ClaymorphState};
use crate::desert::{DesertParams, DesertState};
use crate::glade::{GladeParams, GladeState};
use crate::istanbul::{IstanbulParams, IstanbulState};
use crate::liquidglass::{LgParams, LiquidGlassState};
use crate::zen_archipelago::{ZenArchipelagoParams, ZenArchipelagoState};
use crate::meadow::{MeadowParams, MeadowState};
use crate::neobrutalism::{NbParams, NeobrutalismState};
use crate::neumorph::{NeumorphParams, NeumorphState};
use crate::papercut::{PapercutParams, PapercutState};
use crate::persephone::{PersephoneParams, PersephoneState};
use crate::pocket::{PocketParams, PocketState};
use crate::polarnight::{PolarNightParams, PolarNightState};
use crate::quiet_snow::{QuietSnowParams, QuietSnowState, WindowInput};
use crate::rift::{RiftParams, RiftState};
use crate::roach::{RoachParams, RoachState};
use crate::rosebrook::{RosebrookParams, RosebrookState};
use crate::software_world::{SoftwareWorldParams, SoftwareWorldState};
use crate::solar_system::{SolarSystemParams, SolarSystemState};
use crate::strike::{StrikeParams, StrikeState};
use crate::studio_glass::StudioGlassState;
use crate::thinking_orbs::{ThinkingOrbsParams, ThinkingOrbsState};
use crate::voxelcraft::{VoxelcraftParams, VoxelcraftState};
use crate::winter::{WinterParams, WinterState};
use crate::themes::common::str_index;
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
    pub icon_names: Vec<String>,
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
    pub quiet_snow: QuietSnowState,
    pub papercut: PapercutState,
    pub voxelcraft: VoxelcraftState,
    pub desert: DesertState,
    pub claymorph: ClaymorphState,
    pub neobrutalism: NeobrutalismState,
    pub software_world: SoftwareWorldState,
    pub meadow: MeadowState,
    pub beach: BeachState,
    pub aurora: AuroraState,
    pub glade: GladeState,
    pub liquidglass: LiquidGlassState,
    pub zen_archipelago: ZenArchipelagoState,
    pub rift: RiftState,
    pub strike: StrikeState,
    pub pocket: PocketState,
    pub roach: RoachState,
    pub rosebrook: RosebrookState,
    pub polarnight: PolarNightState,
    pub winter: WinterState,
    pub neumorph: NeumorphState,
    pub istanbul: IstanbulState,
    pub solar_system: SolarSystemState,
    pub thinking_orbs: ThinkingOrbsState,
    pub ai_mascots: AiMascotsState,
    pub cat_shore: CatShoreState,
    pub persephone: PersephoneState,
    pub windows: Vec<WindowInput>,
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
            icon_names: Vec::new(),
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
            quiet_snow: QuietSnowState::default(),
            papercut: PapercutState::default(),
            voxelcraft: VoxelcraftState::default(),
            desert: DesertState::default(),
            claymorph: ClaymorphState::default(),
            neobrutalism: NeobrutalismState::default(),
            software_world: SoftwareWorldState::default(),
            meadow: MeadowState::default(),
            beach: BeachState::default(),
            aurora: AuroraState::default(),
            glade: GladeState::default(),
            liquidglass: LiquidGlassState::default(),
            zen_archipelago: ZenArchipelagoState::default(),
            rift: RiftState::default(),
            strike: StrikeState::default(),
            pocket: PocketState::default(),
            roach: RoachState::default(),
            rosebrook: RosebrookState::default(),
            polarnight: PolarNightState::default(),
            winter: WinterState::default(),
            neumorph: NeumorphState::default(),
            istanbul: IstanbulState::default(),
            solar_system: SolarSystemState::default(),
            thinking_orbs: ThinkingOrbsState::default(),
            ai_mascots: AiMascotsState::default(),
            cat_shore: CatShoreState::default(),
            persephone: PersephoneState::default(),
            windows: Vec::new(),
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

    pub fn is_quiet_snow_theme(&self) -> bool {
        self.theme_id == "quietsnow"
    }

    pub fn is_papercut_theme(&self) -> bool {
        self.theme_id == "papercut"
    }

    pub fn is_voxelcraft_theme(&self) -> bool {
        self.theme_id == "voxelcraft"
    }

    pub fn is_desert_theme(&self) -> bool {
        self.theme_id == "desert"
    }

    pub fn is_claymorph_theme(&self) -> bool {
        self.theme_id == "claymorph"
    }

    pub fn is_neobrutalism_theme(&self) -> bool {
        self.theme_id == "neobrutalism"
    }

    pub fn is_liquidglass_theme(&self) -> bool {
        self.theme_id == "liquidglass"
    }

    pub fn is_zen_archipelago_theme(&self) -> bool {
        self.theme_id == "zen-archipelago"
    }

    pub fn is_software_world_theme(&self) -> bool {
        self.theme_id == "software-world"
    }

    pub fn is_meadow_theme(&self) -> bool {
        self.theme_id == "meadow"
    }

    pub fn is_beach_theme(&self) -> bool {
        self.theme_id == "beach"
    }

    pub fn is_aurora_theme(&self) -> bool {
        self.theme_id == "aurora"
    }

    pub fn is_glade_theme(&self) -> bool {
        self.theme_id == "glade"
    }

    pub fn is_rift_theme(&self) -> bool {
        self.theme_id == "rift"
    }

    pub fn is_strike_theme(&self) -> bool {
        self.theme_id == "strike"
    }

    pub fn is_pocket_theme(&self) -> bool {
        self.theme_id == "pocket"
    }

    pub fn is_roach_theme(&self) -> bool {
        self.theme_id == "roach"
    }

    pub fn is_rosebrook_theme(&self) -> bool {
        self.theme_id == "rosebrook"
    }

    pub fn is_polarnight_theme(&self) -> bool {
        self.theme_id == "polarnight"
    }

    pub fn is_winter_theme(&self) -> bool {
        self.theme_id == "winter"
    }

    pub fn is_neumorph_theme(&self) -> bool {
        self.theme_id == "neumorph"
    }

    pub fn is_istanbul_theme(&self) -> bool {
        self.theme_id == "istanbul"
    }

    pub fn is_solar_system_theme(&self) -> bool {
        self.theme_id == "solar-system"
    }

    pub fn is_thinking_orbs_theme(&self) -> bool {
        self.theme_id == "thinking-orbs"
    }

    pub fn is_ai_mascots_theme(&self) -> bool {
        self.theme_id == "ai-mascots"
    }

    pub fn is_cat_shore_theme(&self) -> bool {
        self.theme_id == "cat-shore"
    }

    pub fn is_persephone_theme(&self) -> bool {
        self.theme_id == "persephone"
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
        if self.is_papercut_theme() {
            self.papercut.cache_ok = false;
            self.papercut.dirty = true;
        }
        if self.is_voxelcraft_theme() {
            self.voxelcraft.cache_ok = false;
            self.voxelcraft.atlas_biome = -1;
            self.voxelcraft.resize(self.css_w(), self.css_h());
        }
        if self.is_desert_theme() {
            let (dw, dh) = crate::desert::css_dims(self);
            self.desert.resize(dw, dh);
            self.desert.rebake();
        }
        if self.is_claymorph_theme() {
            self.claymorph.resize(self.css_w(), self.css_h());
            self.claymorph.cache_ok = false;
            self.claymorph.dirty = true;
            if !self.icons.is_empty() {
                self.claymorph.sync_incoming_icons(&self.icons);
            }
        }
        if self.is_neobrutalism_theme() {
            self.neobrutalism.resize(self.css_w(), self.css_h());
            self.neobrutalism.cache_ok = false;
            self.neobrutalism.dirty = true;
            if !self.icons.is_empty() {
                self.neobrutalism.sync_incoming_icons(&self.icons);
            }
        }
        if self.is_liquidglass_theme() {
            self.liquidglass.resize(self.css_w(), self.css_h());
            self.liquidglass.cache_ok = false;
            self.liquidglass.dirty = true;
            if !self.icons.is_empty() {
                self.liquidglass.sync_incoming_icons(&self.icons);
            }
        }
        if self.is_zen_archipelago_theme() {
            self.zen_archipelago.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.zen_archipelago.set_icons(&self.icons, true);
            }
        }
        if self.is_software_world_theme() {
            self.software_world.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.software_world.update_icons(&self.icons, true);
            }
        }
        if self.is_meadow_theme() {
            self.meadow.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.meadow.set_icons(&self.icons, true);
            }
        }
        if self.is_beach_theme() {
            self.beach.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.beach.set_icons(&self.icons, true);
            }
        }
        if self.is_aurora_theme() {
            self.aurora.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.aurora.set_icons(&self.icons, true);
            }
        }
        if self.is_glade_theme() {
            self.glade.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.glade.set_icons(&self.icons, true);
            }
        }
        if self.is_rift_theme() {
            self.rift.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.rift.set_icons(&self.icons, true);
            }
        }
        if self.is_strike_theme() {
            self.strike.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.strike.set_icons(&self.icons, true);
            }
        }
        if self.is_pocket_theme() {
            self.pocket.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.pocket.set_icons(&self.icons, &self.icon_names, true);
            }
        }
        if self.is_roach_theme() {
            self.roach.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.roach.set_icons(&self.icons, true);
            }
        }
        if self.is_rosebrook_theme() {
            self.rosebrook.resize(self.css_w() as f64, self.css_h() as f64);
            if !self.icons.is_empty() {
                self.rosebrook.set_icons(&self.icons, true);
            }
        }
        if self.is_polarnight_theme() {
            self.polarnight.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.polarnight.set_icons(&self.icons, true);
            }
        }
        if self.is_winter_theme() {
            self.winter.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.winter.set_icons(&self.icons, &self.icon_names, true);
            }
        }
        if self.is_neumorph_theme() {
            self.neumorph
                .resize(self.css_w(), self.css_h(), self.dpi_scale);
            if !self.icons.is_empty() {
                self.neumorph.sync_incoming_icons(&self.icons);
            }
        }
        if self.is_istanbul_theme() {
            self.istanbul.set_env_hour(self.env_hour);
            self.istanbul.resize(self.css_w(), self.css_h());
            self.istanbul.set_icons(&self.icons, !self.icons.is_empty());
        }
        if self.is_solar_system_theme() {
            self.solar_system.set_low_mode(self.low_mode);
            self.solar_system.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.solar_system.set_icons(&self.icons, &self.icon_names, true);
            }
        }
        if self.is_thinking_orbs_theme() {
            self.thinking_orbs.set_low_mode(self.low_mode);
            self.thinking_orbs.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.thinking_orbs.set_icons(&self.icons, true);
            }
        }
        if self.is_ai_mascots_theme() {
            self.ai_mascots.set_low_mode(self.low_mode);
            self.ai_mascots.resize(self.css_w(), self.css_h());
            if !self.icons.is_empty() {
                self.ai_mascots.set_icons(&self.icons, &self.icon_names, true);
            }
        }
        if self.is_cat_shore_theme() {
            self.cat_shore.set_low_mode(self.low_mode);
            self.cat_shore.resize(self.css_w(), self.css_h(), self.dpi_scale);
            if !self.icons.is_empty() {
                self.cat_shore.set_icons(&self.icons, true);
            }
        }
        if self.is_persephone_theme() {
            self.persephone.set_low_mode(self.low_mode);
            self.persephone.resize(self.css_w(), self.css_h(), self.dpi_scale);
            if changed {
                self.persephone.restart_animation();
            }
            if !self.icons.is_empty() {
                self.persephone.set_icons(&self.icons, true);
            }
        }
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
        if self.is_quiet_snow_theme() {
            self.quiet_snow.resize(w, h);
        }
        if self.is_papercut_theme() {
            self.papercut.resize(w, h);
        }
        if self.is_voxelcraft_theme() {
            self.voxelcraft.resize(w, h);
        }
        if self.is_desert_theme() {
            let (dw, dh) = crate::desert::css_dims(self);
            self.desert.resize(dw, dh);
        }
        if self.is_claymorph_theme() {
            self.claymorph.resize(w, h);
        }
        if self.is_neobrutalism_theme() {
            self.neobrutalism.resize(w, h);
        }
        if self.is_liquidglass_theme() {
            self.liquidglass.resize(w, h);
        }
        if self.is_zen_archipelago_theme() {
            self.zen_archipelago.resize(w, h);
        }
        if self.is_software_world_theme() {
            self.software_world.resize(w, h);
        }
        if self.is_meadow_theme() {
            self.meadow.resize(w, h);
        }
        if self.is_beach_theme() {
            self.beach.resize(w, h);
        }
        if self.is_aurora_theme() {
            self.aurora.resize(w, h);
        }
        if self.is_glade_theme() {
            self.glade.resize(w, h);
        }
        if self.is_rift_theme() {
            self.rift.resize(w, h);
        }
        if self.is_strike_theme() {
            self.strike.resize(w, h);
        }
        if self.is_pocket_theme() {
            self.pocket.resize(w, h);
        }
        if self.is_roach_theme() {
            self.roach.resize(w, h);
        }
        if self.is_rosebrook_theme() {
            self.rosebrook.resize(w as f64, h as f64);
        }
        if self.is_polarnight_theme() {
            self.polarnight.resize(w, h);
        }
        if self.is_winter_theme() {
            self.winter.resize(self.css_w(), self.css_h());
        }
        if self.is_neumorph_theme() {
            self.neumorph
                .resize(self.css_w(), self.css_h(), self.dpi_scale);
        }
        if self.is_istanbul_theme() {
            self.istanbul.resize(w, h);
        }
        if self.is_solar_system_theme() {
            self.solar_system.resize(w, h);
        }
        if self.is_thinking_orbs_theme() {
            self.thinking_orbs.resize(w, h);
        }
        if self.is_ai_mascots_theme() {
            self.ai_mascots.resize(w, h);
        }
        if self.is_cat_shore_theme() {
            self.cat_shore.resize(w, h, self.dpi_scale);
        }
        if self.is_persephone_theme() {
            self.persephone.resize(w, h, self.dpi_scale);
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
        } else if self.is_cat_shore_theme() {
            let r = self.cat_shore.active_notch_rect();
            if r.w > 4.0 && r.h > 4.0 {
                rects.push([
                    r.x.floor() as i32,
                    r.y.floor() as i32,
                    r.w.ceil() as i32,
                    r.h.ceil() as i32,
                ]);
            }
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
                    self.quiet_snow.set_low_mode(b);
                    self.solar_system.set_low_mode(b);
                    self.thinking_orbs.set_low_mode(b);
                    self.ai_mascots.set_low_mode(b);
                    self.cat_shore.set_low_mode(b);
                    self.persephone.set_low_mode(b);
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
                if self.is_quiet_snow_theme() {
                    self.quiet_snow.sync_params(QuietSnowParams::default());
                }
                if self.is_papercut_theme() {
                    self.papercut.sync_params(PapercutParams::default());
                }
                if self.is_voxelcraft_theme() {
                    self.voxelcraft.sync_params(VoxelcraftParams::default());
                }
                if self.is_desert_theme() {
                    self.desert.sync_params(DesertParams::default());
                }
                if self.is_claymorph_theme() {
                    self.claymorph.sync_params(ClaymorphParams::default());
                }
                if self.is_neobrutalism_theme() {
                    self.neobrutalism.sync_params(NbParams::default());
                }
                if self.is_liquidglass_theme() {
                    self.liquidglass.sync_params(LgParams::default());
                }
                if self.is_zen_archipelago_theme() {
                    self.zen_archipelago.sync_params(ZenArchipelagoParams::default());
                }
                if self.is_software_world_theme() {
                    self.software_world.sync_params(SoftwareWorldParams::default());
                }
                if self.is_meadow_theme() {
                    self.meadow.sync_params(MeadowParams::default());
                }
                if self.is_beach_theme() {
                    self.beach.sync_params(BeachParams::default());
                }
                if self.is_aurora_theme() {
                    self.aurora.sync_params(AuroraParams::default());
                }
                if self.is_glade_theme() {
                    self.glade.sync_params(GladeParams::default());
                }
                if self.is_rift_theme() {
                    self.rift.sync_params(RiftParams::default());
                }
                if self.is_strike_theme() {
                    self.strike.sync_params(StrikeParams::default());
                }
                if self.is_pocket_theme() {
                    self.pocket.sync_params(PocketParams::default());
                }
                if self.is_roach_theme() {
                    self.roach.sync_params(RoachParams::default());
                }
                if self.is_rosebrook_theme() {
                    self.rosebrook.sync_params(RosebrookParams::default());
                }
                if self.is_polarnight_theme() {
                    self.polarnight.sync_params(PolarNightParams::default());
                }
                if self.is_winter_theme() {
                    self.winter.sync_params(WinterParams::default());
                }
                if self.is_neumorph_theme() {
                    self.neumorph.sync_params(NeumorphParams::default());
                }
                if self.is_istanbul_theme() {
                    self.istanbul.set_env_hour(self.env_hour);
                    self.istanbul.sync_params(IstanbulParams::default());
                }
                if self.is_solar_system_theme() {
                    self.solar_system.sync_params(SolarSystemParams::default());
                }
                if self.is_thinking_orbs_theme() {
                    self.thinking_orbs.sync_params(ThinkingOrbsParams::default());
                }
                if self.is_ai_mascots_theme() {
                    self.ai_mascots.sync_params(AiMascotsParams::default());
                }
                if self.is_cat_shore_theme() {
                    self.cat_shore.sync_params(CatShoreParams::default());
                }
                if self.is_persephone_theme() {
                    self.persephone.sync_params(PersephoneParams::default());
                }
                if let Some(obj) = v.get("v").and_then(|x| x.as_object()) {
                    self.apply_params(obj);
                }
                self.save_persisted();
                self.recompute_layout();
            }
            "icons" => {
                let vis = v.get("vis").and_then(|x| x.as_bool()).unwrap_or(false);
                self.icons.clear();
                self.icon_names.clear();
                if vis {
                    let names_arr = v.get("n").and_then(|x| x.as_array());
                    if let Some(arr) = v.get("r").and_then(|x| x.as_array()) {
                        let k = self.dpi_scale.max(0.5);
                        for (idx, item) in arr.iter().enumerate() {
                            if let Some(r) = item.as_array() {
                                if r.len() >= 4 {
                                    let l = r[0].as_f64().unwrap_or(0.0) as f32 / k;
                                    let top = r[1].as_f64().unwrap_or(0.0) as f32 / k;
                                    let ri = r[2].as_f64().unwrap_or(0.0) as f32 / k;
                                    let b = r[3].as_f64().unwrap_or(0.0) as f32 / k;
                                    self.icons.push(Rect { x: l, y: top, w: (ri - l).max(1.0), h: (b - top).max(1.0) });
                                    let nm = names_arr
                                        .and_then(|na| na.get(idx))
                                        .and_then(|nv| nv.as_str())
                                        .unwrap_or("");
                                    self.icon_names.push(nm.to_string());
                                }
                            }
                        }
                    }
                }
                self.quiet_snow.set_icons(&self.icons);
                self.desert.set_icons(&self.icons, vis);
                self.software_world.update_icons(&self.icons, vis);
                self.meadow.set_icons(&self.icons, vis);
                self.beach.set_icons(&self.icons, vis);
                self.aurora.set_icons(&self.icons, vis);
                self.glade.set_icons(&self.icons, vis);
                self.rift.set_icons(&self.icons, vis);
                self.strike.set_icons(&self.icons, vis);
                self.pocket.set_icons(&self.icons, &self.icon_names, vis);
                self.roach.set_icons(&self.icons, vis);
                self.rosebrook.set_icons(&self.icons, vis);
                self.polarnight.set_icons(&self.icons, vis);
                self.winter.set_icons(&self.icons, &self.icon_names, vis);
                if self.is_neumorph_theme() {
                    let rects: Vec<Rect> = if vis {
                        self.icons.clone()
                    } else {
                        Vec::new()
                    };
                    self.neumorph.sync_incoming_icons(&rects);
                }
                self.claymorph.sync_incoming_icons(&self.icons);
                self.neobrutalism.sync_incoming_icons(&self.icons);
                self.liquidglass.sync_incoming_icons(&self.icons);
                self.zen_archipelago.set_icons(&self.icons, vis);
                self.istanbul.set_icons(&self.icons, vis);
                self.solar_system.set_icons(&self.icons, &self.icon_names, vis);
                self.thinking_orbs.set_icons(&self.icons, vis);
                self.ai_mascots.set_icons(&self.icons, &self.icon_names, vis);
                self.cat_shore.set_icons(&self.icons, vis);
                self.persephone.set_icons(&self.icons, vis);
                self.recompute_layout();
            }
            "windows" => {
                self.windows.clear();
                if let Some(arr) = v.get("r").and_then(|x| x.as_array()) {
                    let k = self.dpi_scale.max(0.5);
                    for item in arr {
                        if let Some(r) = item.as_array() {
                            if r.len() >= 5 {
                                let id = r[0].as_i64().unwrap_or(0);
                                let l = r[1].as_f64().unwrap_or(0.0) as f32 / k;
                                let top = r[2].as_f64().unwrap_or(0.0) as f32 / k;
                                let ri = r[3].as_f64().unwrap_or(0.0) as f32 / k;
                                let b = r[4].as_f64().unwrap_or(0.0) as f32 / k;
                                self.windows.push(WindowInput {
                                    id,
                                    x: l,
                                    y: top,
                                    w: (ri - l).max(1.0),
                                    h: (b - top).max(1.0),
                                });
                            }
                        }
                    }
                }
                self.quiet_snow.set_windows(&self.windows);
            }
            "environment" => {
                if let Some(hr) = v.get("hour").and_then(|x| x.as_f64()) {
                    self.env_hour = hr as f32;
                    self.istanbul.set_env_hour(self.env_hour);
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
        if self.is_quiet_snow_theme() {
            let qsp = QuietSnowParams {
                adaptive: if self.param_bool("adaptive", true) { 1 } else { 0 },
                mode: (self.param_f32("mode", 0.0).round() as i32).clamp(0, 3),
                intensity: self.param_f32("intensity", 0.85).clamp(0.1, 1.6),
                wind: self.param_f32("wind", 0.22).clamp(-1.5, 1.5),
                gusts: self.param_f32("gusts", 0.45).clamp(0.0, 1.0),
                accumulate: (self.param_f32("accumulate", 2.0).round() as i32).clamp(0, 2),
                windows: if self.param_bool("windows", true) { 1 } else { 0 },
                melt: self.param_f32("melt", 0.35).clamp(0.0, 1.0),
                cursor: (self.param_f32("cursor", 2.0).round() as i32).clamp(0, 2),
                depth: self.param_f32("depth", 0.85).clamp(0.3, 1.4),
                atmosphere: (self.param_f32("atmosphere", 0.0).round() as i32).clamp(0, 3),
                sparkle: self.param_f32("sparkle", 0.55).clamp(0.0, 1.0),
                backdrop: self.param_f32("backdrop", 0.75).clamp(0.0, 1.0),
                quality: (self.param_f32("quality", 1.0).round() as i32).clamp(0, 2),
            };
            self.quiet_snow.sync_params(qsp);
        }
        if self.is_winter_theme() {
            let q = match self.param_str("quality") {
                Some("Low") => 0,
                Some("Medium") => 1,
                Some("High") => 2,
                _ => (self.param_f32("quality", 2.0).round() as i32).clamp(0, 2),
            };
            let seed = self
                .raw_params
                .get("seed")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(20261117);
            let wp = WinterParams {
                adaptive: if self.param_bool("adaptive", true) { 1 } else { 0 },
                snow_intensity: self.param_f32("snowIntensity", 0.75).clamp(0.0, 1.5),
                snow_speed: self.param_f32("snowSpeed", 0.85).clamp(0.2, 2.0),
                wind_strength: self.param_f32("windStrength", 0.18).clamp(-1.0, 1.0),
                smoke_amount: self.param_f32("smokeAmount", 0.85).clamp(0.0, 1.5),
                atmosphere_density: self.param_f32("atmosphereDensity", 0.75).clamp(0.0, 1.5),
                tree_density: self.param_f32("treeDensity", 0.80).clamp(0.0, 1.5),
                mountain_visibility: self.param_f32("mountainVisibility", 0.85).clamp(0.0, 1.0),
                icon_snow_amount: self.param_f32("iconSnowAmount", 0.90).clamp(0.0, 1.5),
                anim_speed: self.param_f32("animSpeed", 1.0).clamp(0.2, 2.0),
                quality: q,
                seed,
            };
            self.winter.sync_params(wp);
        }
        if self.is_neumorph_theme() {
            let palette = match self.param_str("palette") {
                Some("sand") => 1,
                Some("slate") => 2,
                Some("sage") => 3,
                _ => 0,
            };
            let accent = match self.param_str("accentColor") {
                Some("coral") => 1,
                Some("emerald") => 2,
                Some("amber") => 3,
                _ => 0,
            };
            let icon_style = match self.param_str("iconStyle") {
                Some("well") => 1,
                Some("minimal") => 2,
                _ => 0,
            };
            let quality = match self.param_str("quality") {
                Some("Low") => 0,
                Some("Medium") => 1,
                Some("High") => 2,
                _ => (self.param_f32("quality", 2.0).round() as i32).clamp(0, 2),
            };
            let seed = self
                .raw_params
                .get("seed")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(20261118);
            let np = NeumorphParams {
                palette,
                accent,
                relief_depth: self.param_f32("reliefDepth", 1.0).clamp(0.3, 1.6),
                light_follow_cursor: self.param_bool("lightFollowCursor", false),
                wave_speed: self.param_f32("waveSpeed", 0.75).clamp(0.0, 2.5),
                tile_density: self.param_f32("tileDensity", 1.0).clamp(0.5, 1.6),
                ripple_strength: self.param_f32("rippleStrength", 1.0).clamp(0.0, 2.2),
                icon_style,
                quality,
                adaptive: if self.param_bool("adaptive", true) { 1 } else { 0 },
                seed,
            };
            self.neumorph.sync_params(np);
        }
        if self.is_papercut_theme() {
            let pcp = PapercutParams {
                adaptive: if self.param_bool("adaptive", true) { 1 } else { 0 },
                time: (self.param_f32("time", 0.0).round() as i32).clamp(0, 4),
                light: self.param_f32("light", 225.0).clamp(0.0, 360.0),
                shadow: self.param_f32("shadow", 1.0).clamp(0.0, 2.0),
                palette: (self.param_f32("palette", 0.0).round() as usize).min(3),
                grain: self.param_f32("grain", 0.50).clamp(0.0, 1.0),
                gap: self.param_f32("gap", 1.0).clamp(0.4, 2.0),
                flowers: self.param_f32("flowers", 0.60).clamp(0.0, 1.5),
                trees: if self.param_bool("trees", true) { 1 } else { 0 },
                critters: (self.param_f32("critters", 1.0).round() as usize).min(2),
                skyline: if self.param_bool("skyline", true) { 1 } else { 0 },
                frame: if self.param_bool("frame", true) { 1 } else { 0 },
                sway: self.param_f32("sway", 1.0).clamp(0.0, 2.0),
                fw: self.param_f32("fw", 16.0).clamp(6.0, 40.0),
                fstyle: (self.param_f32("fstyle", 0.0).round() as i32).clamp(0, 1),
                quality: (self.param_f32("quality", 1.0).round() as usize).min(2),
            };
            self.papercut.sync_params(pcp);
        }
        if self.is_voxelcraft_theme() {
            let vp = VoxelcraftParams {
                adaptive: if self.param_bool("adaptive", true) { 1 } else { 0 },
                biome: str_index(self, "biome", &["overworld", "cherry", "snowy", "nether"], 0) as usize,
                time_of_day: str_index(self, "timeOfDay", &["auto", "day", "sunset", "night"], 0) as usize,
                icon_style: str_index(self, "iconBlockStyle", &["island", "planks", "frame"], 0) as usize,
                block_size: self.param_f32("blockSize", 1.0).clamp(0.7, 1.5),
                cloud_density: self.param_f32("cloudDensity", 1.0).clamp(0.0, 1.6),
                weather_amount: self.param_f32("weatherAmount", 1.0).clamp(0.0, 1.8),
                critter_amount: self.param_f32("critterAmount", 1.0).clamp(0.0, 1.8),
                torch_glow: self.param_f32("torchGlow", 1.0).clamp(0.0, 1.8),
                anim_speed: self.param_f32("animSpeed", 1.0).clamp(0.2, 2.5),
                quality: str_index(self, "quality", &["Low", "Medium", "High"], 2) as usize,
                seed: self.param_f32("seed", 0.0).max(0.0) as u32,
            };
            self.voxelcraft.sync_params(vp);
        }
        if self.is_desert_theme() {
            let dp = DesertParams {
                sigma: self.param_f32("sigma", 88.0).clamp(40.0, 200.0),
                warp: self.param_f32("warp", 36.0).clamp(0.0, 80.0),
                oases: (self.param_f32("oases", 2.0).round() as i32).clamp(0, 4) as usize,
                warmth: self.param_f32("warmth", 0.78).clamp(0.0, 1.0),
                wind: self.param_f32("wind", 1.0).clamp(0.0, 2.5),
                camels: (self.param_f32("camels", 6.0).round() as i32).clamp(0, 12) as usize,
                travelers: self.param_bool("travelers", true),
                palms: (self.param_f32("palms", 10.0).round() as i32).clamp(0, 18) as usize,
                adaptive: self.param_bool("adaptive", true),
                debug: self.param_bool("debug", false),
                seed: self.param_f32("seed", 73194.0).max(0.0) as u32,
                quality: str_index(self, "quality", &["Low", "Medium", "High"], 2) as usize,
            };
            self.desert.sync_params(dp);
        }
        if self.is_claymorph_theme() {
            let pal_idx = match self.param_str("palette") {
                Some("candy") => 1,
                Some("matcha") => 2,
                Some("dusk") => 3,
                _ => 0,
            };
            let cradle_idx = match self.param_str("iconCradle") {
                Some("cloud") => 1,
                Some("ring") => 2,
                _ => 0,
            };
            let quality = match self.param_str("quality") {
                Some("Low") => 0,
                Some("Medium") => 1,
                _ => 2,
            };
            let cmp = ClaymorphParams {
                palette: pal_idx,
                puffiness: self.param_f32("puffiness", 1.0).clamp(0.4, 1.6),
                squish_strength: self.param_f32("squishStrength", 1.0).clamp(0.0, 1.8),
                blob_count: self.param_f32("blobCount", 1.0).clamp(0.0, 1.8),
                float_speed: self.param_f32("floatSpeed", 0.80).clamp(0.0, 2.0),
                cursor_repel: self.param_f32("cursorRepel", 1.0).clamp(0.0, 1.8),
                icon_cradle: cradle_idx,
                quality,
                adaptive: if self.param_bool("adaptive", true) { 1 } else { 0 },
                seed: self.param_f32("seed", 20261118.0).round() as u32,
            };
            self.claymorph.sync_params(cmp);
        }
        if self.is_neobrutalism_theme() {
            let pal_idx = match self.param_str("palette") {
                Some("acid") => 1,
                Some("zine") => 2,
                Some("dark") => 3,
                _ => 0,
            };
            let frame_idx = match self.param_str("iconFrame") {
                Some("sticker") => 1,
                Some("blueprint") => 2,
                _ => 0,
            };
            let quality = match self.param_str("quality") {
                Some("Low") => 0,
                Some("Medium") => 1,
                _ => 2,
            };
            let nbp = NbParams {
                palette: pal_idx,
                border_weight: self.param_f32("borderWeight", 3.0).clamp(1.5, 5.5),
                shadow_depth: self.param_f32("shadowDepth", 7.0).clamp(2.0, 14.0),
                shape_count: self.param_f32("shapeCount", 1.0).clamp(0.0, 1.8),
                motion_speed: self.param_f32("motionSpeed", 0.85).clamp(0.0, 2.0),
                cursor_push: self.param_f32("cursorPush", 1.0).clamp(0.0, 1.8),
                halftone: self.param_bool("halftonePattern", true),
                icon_frame: frame_idx,
                quality,
                adaptive: if self.param_bool("adaptive", true) { 1 } else { 0 },
                seed: self.param_f32("seed", 0.0).round() as u32,
            };
            self.neobrutalism.sync_params(nbp);
        }
        if self.is_liquidglass_theme() {
            let pal_idx = match self.param_str("palette") {
                Some("aurora") => 1,
                Some("prism") => 2,
                Some("obsidian") => 3,
                Some("classic") => 4,
                _ => 0,
            };
            let style_idx = match self.param_str("iconGlassStyle") {
                Some("droplet") => 1,
                Some("prism") => 2,
                _ => 0,
            };
            let quality = match self.param_str("quality") {
                Some("Low") => 0,
                Some("Medium") => 1,
                _ => 2,
            };
            let lgp = LgParams {
                palette: pal_idx,
                refraction: self.param_f32("refraction", 1.0).clamp(0.15, 1.6),
                frost_amount: self.param_f32("frostAmount", 0.65).clamp(0.0, 1.5),
                dispersion: self.param_f32("dispersion", 0.95).clamp(0.0, 1.6),
                merge_rate: self.param_f32("mergeRate", 0.55).clamp(0.0, 1.5),
                droplet_count: self.param_f32("dropletCount", 1.0).clamp(0.0, 2.0),
                flow_speed: self.param_f32("flowSpeed", 0.85).clamp(0.0, 2.0),
                cursor_lens: self.param_bool("cursorLens", true),
                icon_style: style_idx,
                quality,
                adaptive: if self.param_bool("adaptive", true) { 1 } else { 0 },
                seed: self.param_f32("seed", 0.0).round() as u32,
            };
            self.liquidglass.sync_params(lgp);
        }
        if self.is_zen_archipelago_theme() {
            // TIME_MODES = ["night", "sunset", "day"] -> 0 = night (default), 1 = sunset, 2 = day
            let tod = match self.param_str("timeOfDay") {
                Some("night") => 0,
                Some("sunset") => 1,
                Some("day") => 2,
                _ => (self.param_f32("timeOfDay", 0.0).round() as i32).clamp(0, 2) as usize,
            };
            let zap = ZenArchipelagoParams {
                adaptive: self.param_bool("adaptive", true),
                time_of_day: tod,
                koi_count: (self.param_f32("koiCount", 14.0).round() as i32).clamp(4, 30) as usize,
                lantern_count: (self.param_f32("lanternCount", 10.0).round() as i32).clamp(0, 25) as usize,
                island_margin: (self.param_f32("islandMargin", 44.0) as f64).clamp(18.0, 72.0),
                bridge_distance: (self.param_f32("bridgeDistance", 230.0) as f64).clamp(100.0, 500.0),
                wave_intensity: (self.param_f32("waveIntensity", 1.0) as f64).clamp(0.2, 2.0),
                sakura_density: (self.param_f32("sakuraDensity", 1.0) as f64).clamp(0.0, 2.0),
                fireflies: (self.param_f32("fireflies", 45.0).round() as i32).clamp(0, 100) as usize,
            };
            self.zen_archipelago.sync_params(zap);
        }
        if self.is_software_world_theme() {
            let swp = SoftwareWorldParams {
                adaptive: if self.param_bool("adaptive", true) { 1 } else { 0 },
                anim_speed: self.param_f32("animSpeed", 1.0).clamp(0.25, 2.5),
                network_activity: self.param_f32("networkActivity", 0.65).clamp(0.0, 1.0),
                connection_intensity: self.param_f32("connectionIntensity", 0.55).clamp(0.0, 1.0),
                particle_density: self.param_f32("particleDensity", 0.55).clamp(0.0, 1.0),
                code_particle_density: self.param_f32("codeParticleDensity", 0.45).clamp(0.0, 1.0),
                env_detail: self.param_f32("envDetail", 0.70).clamp(0.0, 1.0),
                quality: (self.param_f32("quality", 1.0).round() as usize).min(2),
                seed: self.param_f32("seed", 42.0).round() as i32,
            };
            self.software_world.sync_params(swp);
            if swp.adaptive != 0 && self.software_world.nodes.is_empty() && !self.icons.is_empty() {
                self.software_world.update_icons(&self.icons, true);
            }
        }
        if self.is_meadow_theme() {
            let mp = MeadowParams {
                sigma: (self.param_f32("sigma", 72.0) as f64).clamp(40.0, 200.0),
                warp: 40.0,
                edge: 10.0,
                cover: (self.param_f32("cover", 0.3) as f64).clamp(0.0, 1.0),
                flow: (self.param_f32("flow", 1.0) as f64).clamp(0.0, 3.0),
                show_boy: self.param_bool("showBoy", true),
                show_cat: self.param_bool("showCat", true),
                adaptive: self.param_bool("adaptive", true),
                seed: self.param_f32("seed", 42.0).round() as i32,
                debug: self.param_bool("debug", false),
            };
            self.meadow.sync_params(mp);
        }
        if self.is_beach_theme() {
            let bp = BeachParams {
                sigma: (self.param_f32("sigma", 85.0) as f64).clamp(40.0, 200.0),
                warp: (self.param_f32("warp", 34.0) as f64).clamp(0.0, 80.0),
                shallow: (self.param_f32("shallow", 260.0) as f64).clamp(120.0, 700.0),
                shoal: (self.param_f32("shoal", 110.0) as f64).clamp(0.0, 300.0),
                speed: 22.0,
                runup: 120.0,
                wave: 1.6,
                spacing: 70.0,
                waves: 1.0,
                grain: 1.0,
                kayaks: (self.param_f32("kayaks", 2.0).round() as i32).clamp(0, 4) as usize,
                birds: (self.param_f32("birds", 3.0).round() as i32).clamp(0, 8) as usize,
                palms: (self.param_f32("palms", 4.0).round() as i32).clamp(0, 10) as usize,
                seed: self.param_f32("seed", 42.0).round() as i32,
                debug: self.param_bool("debug", false),
            };
            self.beach.sync_params(bp);
        }
        if self.is_aurora_theme() {
            let ap = AuroraParams {
                sigma: (self.param_f32("sigma", 82.0) as f64).clamp(40.0, 200.0),
                warp: (self.param_f32("warp", 32.0) as f64).clamp(0.0, 80.0),
                shoal: (self.param_f32("shoal", 90.0) as f64).clamp(0.0, 240.0),
                aurora: (self.param_f32("aurora", 0.78) as f64).clamp(0.0, 1.0),
                flow: (self.param_f32("flow", 1.0) as f64).clamp(0.0, 3.0),
                snow: (self.param_f32("snow", 0.45) as f64).clamp(0.0, 1.0),
                pines: (self.param_f32("pines", 6.0).round() as i32).clamp(0, 12) as usize,
                lanterns: (self.param_f32("lanterns", 2.0).round() as i32).clamp(0, 6) as usize,
                adaptive: self.param_bool("adaptive", true),
                seed: self.param_f32("seed", 42.0).round() as i32,
                debug: self.param_bool("debug", false),
            };
            self.aurora.sync_params(ap);
        }
        if self.is_glade_theme() {
            let gp = GladeParams {
                sigma: (self.param_f32("sigma", 78.0) as f64).clamp(40.0, 200.0),
                warp: (self.param_f32("warp", 28.0) as f64).clamp(0.0, 80.0),
                density: (self.param_f32("density", 1.0) as f64).clamp(0.2, 1.5),
                wind: (self.param_f32("wind", 1.0) as f64).clamp(0.0, 3.0),
                birds: (self.param_f32("birds", 4.0).round() as i32).clamp(0, 10) as usize,
                adaptive: self.param_bool("adaptive", true),
                seed: self.param_f32("seed", 42.0).round() as i32,
                debug: self.param_bool("debug", false),
            };
            self.glade.sync_params(gp);
        }
        if self.is_rift_theme() {
            let rp = RiftParams {
                sigma: (self.param_f32("sigma", 82.0) as f64).clamp(40.0, 200.0),
                warp: (self.param_f32("warp", 24.0) as f64).clamp(0.0, 80.0),
                glow: (self.param_f32("glow", 0.78) as f64).clamp(0.0, 1.0),
                wind: (self.param_f32("wind", 1.0) as f64).clamp(0.0, 3.0),
                heroes: (self.param_f32("heroes", 5.0).round() as i32).clamp(0, 5) as usize,
                turrets: (self.param_f32("turrets", 4.0).round() as i32).clamp(0, 6) as usize,
                wards: (self.param_f32("wards", 4.0).round() as i32).clamp(0, 8) as usize,
                wisps: (self.param_f32("wisps", 8.0).round() as i32).clamp(0, 16) as usize,
                adaptive: self.param_bool("adaptive", true),
                seed: self.param_f32("seed", 42.0).round() as i32,
                debug: self.param_bool("debug", false),
            };
            self.rift.sync_params(rp);
        }
        if self.is_strike_theme() {
            let sp = StrikeParams {
                sigma: (self.param_f32("sigma", 82.0) as f64).clamp(40.0, 200.0),
                warp: (self.param_f32("warp", 20.0) as f64).clamp(0.0, 80.0),
                holes: (self.param_f32("holes", 1.0) as f64).clamp(0.0, 2.0),
                smoke: (self.param_f32("smoke", 0.45) as f64).clamp(0.0, 1.0),
                wind: (self.param_f32("wind", 1.0) as f64).clamp(0.0, 3.0),
                ball: (self.param_f32("ball", 1.0).round() as i32).clamp(0, 3) as usize,
                chickens: (self.param_f32("chickens", 4.0).round() as i32).clamp(0, 10) as usize,
                crates: (self.param_f32("crates", 1.0) as f64).clamp(0.2, 1.5),
                adaptive: self.param_bool("adaptive", true),
                seed: self.param_f32("seed", 42.0).round() as i32,
                debug: self.param_bool("debug", false),
            };
            self.strike.sync_params(sp);
        }
        if self.is_pocket_theme() {
            let pp = PocketParams {
                sigma: (self.param_f32("sigma", 78.0) as f64).clamp(40.0, 200.0),
                warp: (self.param_f32("warp", 22.0) as f64).clamp(0.0, 80.0),
                arrows: (self.param_f32("arrows", 0.78) as f64).clamp(0.0, 1.0),
                divisions: (self.param_f32("divisions", 12.0).round() as i32).clamp(0, 24) as usize,
                faction: (self.param_f32("faction", 0.0).round() as i32).clamp(0, 3),
                pace: (self.param_f32("pace", 1.0) as f64).clamp(0.0, 3.0),
                skirmish: (self.param_f32("skirmish", 0.65) as f64).clamp(0.0, 1.0),
                supply: (self.param_f32("supply", 0.55) as f64).clamp(0.0, 1.0),
                adaptive: self.param_bool("adaptive", true),
                seed: self.param_f32("seed", 42.0).round() as i32,
                debug: self.param_bool("debug", false),
            };
            self.pocket.sync_params(pp);
        }
        if self.is_roach_theme() {
            let rp = RoachParams {
                sigma: (self.param_f32("sigma", 78.0) as f64).clamp(40.0, 200.0),
                warp: (self.param_f32("warp", 24.0) as f64).clamp(0.0, 80.0),
                glow: (self.param_f32("glow", 0.78) as f64).clamp(0.0, 1.0),
                crowd: (self.param_f32("crowd", 12.0).round() as i32).clamp(0, 22) as usize,
                speed: (self.param_f32("speed", 1.0) as f64).clamp(0.2, 2.2),
                props: (self.param_f32("props", 0.9) as f64).clamp(0.0, 1.0),
                vibe: (self.param_f32("vibe", 0.0) as f64).clamp(0.0, 1.0),
                adaptive: self.param_bool("adaptive", true),
                seed: self.param_f32("seed", 41297.0).round() as i32,
                debug: self.param_bool("debug", false),
            };
            self.roach.sync_params(rp);
        }
        if self.is_rosebrook_theme() {
            let rp = RosebrookParams {
                sigma: (self.param_f32("sigma", 72.0) as f64).clamp(40.0, 180.0),
                stream: (self.param_f32("stream", 1.0) as f64).clamp(0.25, 1.5),
                flow: (self.param_f32("flow", 1.0) as f64).clamp(0.2, 2.2),
                sunlight: (self.param_f32("sunlight", 0.78) as f64).clamp(0.0, 1.0),
                roses: (self.param_f32("roses", 0.85) as f64).clamp(0.0, 1.0),
                petals: (self.param_f32("petals", 12.0).round() as i32).clamp(0, 24) as usize,
                breeze: (self.param_f32("breeze", 1.0) as f64).clamp(0.2, 2.0),
                adaptive: self.param_bool("adaptive", true),
                seed: self.param_f32("seed", 42.0).round() as i32,
                debug: self.param_bool("debug", false),
            };
            self.rosebrook.sync_params(rp);
        }
        if self.is_polarnight_theme() {
            let pnp = PolarNightParams {
                sigma: (self.param_f32("sigma", 90.0) as f64).clamp(40.0, 200.0),
                density: (self.param_f32("density", 0.6) as f64).clamp(0.1, 1.0),
                glow: (self.param_f32("glow", 0.75) as f64).clamp(0.1, 1.0),
                flow: (self.param_f32("flow", 1.0) as f64).clamp(0.2, 2.5),
                palette: (self.param_f32("palette", 0.0).round() as i32).clamp(0, 2) as usize,
                snow: (self.param_f32("snow", 110.0).round() as i32).clamp(0, 250) as usize,
                meteor: self.param_bool("meteor", true),
                cabin: self.param_bool("cabin", true),
                seed: self.param_f32("seed", 424242.0).round() as i32,
            };
            self.polarnight.sync_params(pnp);
        }
        if self.is_istanbul_theme() {
            self.istanbul.set_env_hour(self.env_hour);
            self.istanbul.sync_params(IstanbulParams::from_raw_params(&self.raw_params));
        }
        if self.is_solar_system_theme() {
            self.solar_system.sync_params(SolarSystemParams::from_raw_params(&self.raw_params));
        }
        if self.is_thinking_orbs_theme() {
            self.thinking_orbs.sync_params(ThinkingOrbsParams::from_raw_params(&self.raw_params));
        }
        if self.is_ai_mascots_theme() {
            self.ai_mascots.sync_params(AiMascotsParams::from_raw_params(&self.raw_params));
        }
        if self.is_cat_shore_theme() {
            self.cat_shore.sync_params(CatShoreParams::from_raw_params(&self.raw_params));
        }
        if self.is_persephone_theme() {
            self.persephone.sync_params(PersephoneParams::from_raw_params(&self.raw_params));
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
        if self.is_quiet_snow_theme() {
            self.quiet_snow.set_low_mode(self.low_mode);
            self.quiet_snow.step(dt);
        }
        if self.is_papercut_theme() {
            self.papercut.step(dt, &self.icons);
        }
        if self.is_voxelcraft_theme() {
            let px = self.pointer_x;
            let py = self.pointer_y;
            let pa = self.pointer_active;
            let icons = &self.icons;
            self.voxelcraft.set_icons(icons, true);
            self.voxelcraft.step(dt, (px, py, pa));
        }
        if self.is_desert_theme() {
            self.desert.step(dt);
        }
        if self.is_claymorph_theme() {
            let px = self.pointer_x;
            let py = self.pointer_y;
            let pa = self.pointer_active;
            let pd = self.claymorph.pointer_down;
            self.claymorph.step(dt, &self.icons, (px, py, pa, pd));
        }
        if self.is_neobrutalism_theme() {
            let px = self.pointer_x;
            let py = self.pointer_y;
            let pa = self.pointer_active;
            let pd = self.neobrutalism.pointer_down;
            self.neobrutalism.step(dt, &self.icons, (px, py, pa, pd));
        }
        if self.is_liquidglass_theme() {
            self.liquidglass.step(dt, &self.icons);
        }
        if self.is_zen_archipelago_theme() {
            self.zen_archipelago.step(dt);
        }
        if self.is_software_world_theme() {
            self.software_world.step(dt);
        }
        if self.is_meadow_theme() {
            self.meadow.step(dt);
        }
        if self.is_beach_theme() {
            self.beach.step(dt);
        }
        if self.is_aurora_theme() {
            self.aurora.step(dt);
        }
        if self.is_glade_theme() {
            self.glade.step(dt);
        }
        if self.is_rift_theme() {
            self.rift.step(dt);
        }
        if self.is_strike_theme() {
            self.strike.step(dt);
        }
        if self.is_pocket_theme() {
            self.pocket.step(dt);
        }
        if self.is_roach_theme() {
            self.roach.step(dt);
        }
        if self.is_rosebrook_theme() {
            self.rosebrook.step(dt as f64);
        }
        if self.is_polarnight_theme() {
            self.polarnight.step(dt);
        }
        if self.is_winter_theme() {
            self.winter.set_low_mode(self.low_mode);
            self.winter.step(dt);
        }
        if self.is_neumorph_theme() {
            self.neumorph.set_low_mode(self.low_mode);
            self.neumorph.step(dt);
        }
        if self.is_istanbul_theme() {
            self.istanbul.step(dt);
        }
        if self.is_solar_system_theme() {
            self.solar_system.set_low_mode(self.low_mode);
            self.solar_system.step(dt);
        }
        if self.is_thinking_orbs_theme() {
            self.thinking_orbs.set_low_mode(self.low_mode);
            self.thinking_orbs.step(dt);
        }
        if self.is_ai_mascots_theme() {
            self.ai_mascots.set_low_mode(self.low_mode);
            self.ai_mascots.step(dt);
        }
        if self.is_cat_shore_theme() {
            let was_open = self.cat_shore.is_notch_expanded();
            self.cat_shore.set_low_mode(self.low_mode);
            self.cat_shore.step(dt);
            if self.cat_shore.is_notch_expanded() != was_open {
                self.emit_ui_rects();
            }
        }
        if self.is_persephone_theme() {
            self.persephone.set_low_mode(self.low_mode);
            self.persephone.step(dt);
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
            self.quiet_snow.on_pointer_leave();
            self.claymorph.on_pointer_leave();
            self.neobrutalism.on_pointer_leave();
            self.liquidglass.on_pointer_leave();
            self.solar_system.handle_mouse(3, 0.0, 0.0);
            self.thinking_orbs.handle_mouse(3, 0.0, 0.0);
            self.ai_mascots.handle_mouse(3, 0.0, 0.0);
            self.cat_shore.handle_mouse(3, 0.0, 0.0);
            self.persephone.handle_mouse(3, 0.0, 0.0);
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

        if self.is_quiet_snow_theme() {
            match kind {
                0 => self.quiet_snow.on_pointer_move(x, y, None),
                1 => self.quiet_snow.on_pointer_down(x, y),
                2 => self.quiet_snow.on_pointer_up(),
                _ => {}
            }
            return false;
        }
        if self.is_claymorph_theme() {
            match kind {
                0 => self.claymorph.on_pointer_move(x, y),
                1 => self.claymorph.on_pointer_down(x, y),
                2 => self.claymorph.on_pointer_up(),
                _ => {}
            }
            return false;
        }
        if self.is_neobrutalism_theme() {
            match kind {
                0 => self.neobrutalism.on_pointer_move(x, y),
                1 => self.neobrutalism.on_pointer_down(x, y),
                2 => self.neobrutalism.on_pointer_up(),
                _ => {}
            }
            return false;
        }
        if self.is_liquidglass_theme() {
            match kind {
                0 => self.liquidglass.on_pointer_move(x, y),
                1 => self.liquidglass.on_pointer_down(x, y),
                2 => self.liquidglass.on_pointer_up(),
                _ => {}
            }
            return false;
        }
        if self.is_zen_archipelago_theme() {
            if kind == 1 {
                self.zen_archipelago.on_pointer_down(x as f64, y as f64);
            }
            return false;
        }
        if self.is_beach_theme() {
            if kind == 1 {
                self.beach.add_ripple_if_water(x, y, 1.0);
            }
            return false;
        }
        if self.is_voxelcraft_theme() {
            if kind == 1 {
                self.voxelcraft.pop_block_break(x, y);
            }
            return false;
        }
        if self.is_desert_theme() {
            if kind == 1 {
                let on_water = self.desert.is_on_water(x, y);
                self.desert.tap(x, y, on_water);
            }
            return false;
        }
        if self.is_aurora_theme() {
            if kind == 1 {
                self.aurora.add_ripple_if_ice(x, y, 1.0);
            }
            return false;
        }
        if self.is_strike_theme() {
            if kind == 0 || kind == 1 {
                self.strike.handle_pointer(kind == 1, x, y);
            }
            return false;
        }
        if self.is_roach_theme() {
            return self.roach.handle_mouse(kind, x as f64, y as f64);
        }
        if self.is_rosebrook_theme() {
            if kind == 1 {
                self.rosebrook.on_pointer_down(x as f64, y as f64);
            }
            return false;
        }
        if self.is_polarnight_theme() {
            if kind == 1 {
                self.polarnight.add_ripple(x as f64, y as f64);
            }
            return false;
        }
        if self.is_neumorph_theme() {
            match kind {
                0 => self.neumorph.on_pointer_move(x, y),
                1 => self.neumorph.on_pointer_down(x, y),
                2 => self.neumorph.on_pointer_up(),
                _ => self.neumorph.on_pointer_leave(),
            }
            return false;
        }
        if self.is_istanbul_theme() {
            return self.istanbul.handle_mouse(kind, x, y);
        }
        if self.is_solar_system_theme() {
            return self.solar_system.handle_mouse(kind, x, y);
        }
        if self.is_thinking_orbs_theme() {
            return self.thinking_orbs.handle_mouse(kind, x, y);
        }
        if self.is_ai_mascots_theme() {
            return self.ai_mascots.handle_mouse(kind, x, y);
        }
        if self.is_cat_shore_theme() {
            let handled = self.cat_shore.handle_mouse(kind, x, y);
            if kind == 1 {
                self.emit_ui_rects();
            }
            return handled;
        }
        if self.is_persephone_theme() {
            return self.persephone.handle_mouse(kind, x, y);
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
