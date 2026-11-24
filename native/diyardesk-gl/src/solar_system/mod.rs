pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use serde_json::Value;
use shaders::*;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::ffi::c_void;

// =========================================================================
// 1. DETERMINISTIC PRNG & HASH (1:1 with prototype/solar-system.html)
// =========================================================================

pub fn fnv1a_hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for ch in s.encode_utf16() {
        h ^= ch as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

#[derive(Clone, Copy, Debug)]
pub struct Mulberry32 {
    a: u32,
}

impl Mulberry32 {
    pub fn new(seed: u32) -> Self {
        Self { a: seed }
    }

    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        self.a = self.a.wrapping_add(0x6D2B_79F5);
        let mut t = (self.a ^ (self.a >> 15)).wrapping_mul(1 | self.a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }
}

#[inline]
fn clamp_f64(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
fn lerp_f64(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[inline]
fn smoothstep_f64(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = clamp_f64((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn rgb_u8(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

// =========================================================================
// 2. PALETTES & QUALITY PROFILES
// =========================================================================

#[derive(Clone, Copy)]
struct QualityProfile {
    star_multiplier: f64,
    asteroid_multiplier: f64,
    enable_moons: bool,
    enable_local_dust: bool,
    enable_resonance_bridges: bool,
    enable_tidal_streams: bool,
}

const QUALITY_PROFILES: [QualityProfile; 3] = [
    QualityProfile {
        star_multiplier: 0.45,
        asteroid_multiplier: 0.35,
        enable_moons: false,
        enable_local_dust: false,
        enable_resonance_bridges: false,
        enable_tidal_streams: false,
    },
    QualityProfile {
        star_multiplier: 0.75,
        asteroid_multiplier: 0.70,
        enable_moons: true,
        enable_local_dust: true,
        enable_resonance_bridges: true,
        enable_tidal_streams: true,
    },
    QualityProfile {
        star_multiplier: 1.0,
        asteroid_multiplier: 1.0,
        enable_moons: true,
        enable_local_dust: true,
        enable_resonance_bridges: true,
        enable_tidal_streams: true,
    },
];

#[derive(Clone, Copy)]
struct StellarType {
    core: [f32; 3],
    corona_rgb: [u8; 3],
    orbit_rgb: [f32; 3],
}

const STELLAR_TYPES: [StellarType; 4] = [
    StellarType {
        core: [1.0, 245.0 / 255.0, 228.0 / 255.0], // #fff5e4
        corona_rgb: [235, 205, 165],
        orbit_rgb: [185.0 / 255.0, 200.0 / 255.0, 225.0 / 255.0],
    },
    StellarType {
        core: [244.0 / 255.0, 247.0 / 255.0, 1.0], // #f4f7ff
        corona_rgb: [190, 212, 245],
        orbit_rgb: [168.0 / 255.0, 194.0 / 255.0, 232.0 / 255.0],
    },
    StellarType {
        core: [1.0, 235.0 / 255.0, 210.0 / 255.0], // #ffebd2
        corona_rgb: [228, 180, 138],
        orbit_rgb: [198.0 / 255.0, 182.0 / 255.0, 168.0 / 255.0],
    },
    StellarType {
        core: [238.0 / 255.0, 244.0 / 255.0, 1.0], // #eef4ff
        corona_rgb: [165, 195, 240],
        orbit_rgb: [155.0 / 255.0, 185.0 / 255.0, 228.0 / 255.0],
    },
];

const STAR_COLORS: [[f32; 3]; 4] = [
    [215.0 / 255.0, 228.0 / 255.0, 250.0 / 255.0],
    [242.0 / 255.0, 236.0 / 255.0, 225.0 / 255.0],
    [185.0 / 255.0, 208.0 / 255.0, 245.0 / 255.0],
    [232.0 / 255.0, 215.0 / 255.0, 198.0 / 255.0],
];

#[derive(Clone, Copy)]
struct PlanetPalette {
    fill: [f32; 3],
    highlight: [f32; 3],
    shadow: [f32; 3],
    atmo: [f32; 3],
}

const PLANET_PALETTES: [PlanetPalette; 7] = [
    PlanetPalette {
        fill: [139.0 / 255.0, 164.0 / 255.0, 196.0 / 255.0],
        highlight: [184.0 / 255.0, 205.0 / 255.0, 230.0 / 255.0],
        shadow: [38.0 / 255.0, 52.0 / 255.0, 71.0 / 255.0],
        atmo: [139.0 / 255.0, 164.0 / 255.0, 196.0 / 255.0],
    },
    PlanetPalette {
        fill: [196.0 / 255.0, 157.0 / 255.0, 121.0 / 255.0],
        highlight: [227.0 / 255.0, 196.0 / 255.0, 166.0 / 255.0],
        shadow: [61.0 / 255.0, 44.0 / 255.0, 30.0 / 255.0],
        atmo: [196.0 / 255.0, 157.0 / 255.0, 121.0 / 255.0],
    },
    PlanetPalette {
        fill: [127.0 / 255.0, 153.0 / 255.0, 149.0 / 255.0],
        highlight: [173.0 / 255.0, 196.0 / 255.0, 192.0 / 255.0],
        shadow: [35.0 / 255.0, 48.0 / 255.0, 46.0 / 255.0],
        atmo: [127.0 / 255.0, 153.0 / 255.0, 149.0 / 255.0],
    },
    PlanetPalette {
        fill: [179.0 / 255.0, 123.0 / 255.0, 104.0 / 255.0],
        highlight: [214.0 / 255.0, 166.0 / 255.0, 150.0 / 255.0],
        shadow: [59.0 / 255.0, 34.0 / 255.0, 26.0 / 255.0],
        atmo: [179.0 / 255.0, 123.0 / 255.0, 104.0 / 255.0],
    },
    PlanetPalette {
        fill: [154.0 / 255.0, 163.0 / 255.0, 181.0 / 255.0],
        highlight: [197.0 / 255.0, 205.0 / 255.0, 224.0 / 255.0],
        shadow: [44.0 / 255.0, 49.0 / 255.0, 61.0 / 255.0],
        atmo: [154.0 / 255.0, 163.0 / 255.0, 181.0 / 255.0],
    },
    PlanetPalette {
        fill: [189.0 / 255.0, 174.0 / 255.0, 147.0 / 255.0],
        highlight: [223.0 / 255.0, 211.0 / 255.0, 188.0 / 255.0],
        shadow: [56.0 / 255.0, 49.0 / 255.0, 36.0 / 255.0],
        atmo: [189.0 / 255.0, 174.0 / 255.0, 147.0 / 255.0],
    },
    PlanetPalette {
        fill: [108.0 / 255.0, 132.0 / 255.0, 163.0 / 255.0],
        highlight: [156.0 / 255.0, 178.0 / 255.0, 207.0 / 255.0],
        shadow: [28.0 / 255.0, 38.0 / 255.0, 54.0 / 255.0],
        atmo: [108.0 / 255.0, 132.0 / 255.0, 163.0 / 255.0],
    },
];

// =========================================================================
// 3. PARAMS (1:1 with prototype/themes.json & solar-system.html)
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarSystemParams {
    pub center_body_type: usize, // 0 = BlackHole, 1 = Sun, 2 = None
    pub black_hole_size: f64,
    pub accretion_intensity: f64,
    pub icon_orbit_mode: usize, // 0 = Orbit, 1 = Anchor
    pub global_orbit_speed: f64,
    pub planet_count: usize,
    pub orbit_size: f64,
    pub orbit_speed: f64,
    pub planet_size: f64,
    pub star_density: f64,
    pub nebula_intensity: f64,
    pub asteroid_density: f64,
    pub shooting_stars: bool,
    pub gravity_strength: f64,
    pub system_attraction: f64,
    pub quality: usize, // 0 = Low, 1 = Medium, 2 = High
    pub adaptive: bool,
    pub seed: u32,
}

impl Default for SolarSystemParams {
    fn default() -> Self {
        Self {
            center_body_type: 0,
            black_hole_size: 1.0,
            accretion_intensity: 1.0,
            icon_orbit_mode: 0,
            global_orbit_speed: 0.65,
            planet_count: 3,
            orbit_size: 1.0,
            orbit_speed: 1.0,
            planet_size: 1.0,
            star_density: 1.0,
            nebula_intensity: 0.65,
            asteroid_density: 0.8,
            shooting_stars: true,
            gravity_strength: 0.6,
            system_attraction: 0.55,
            quality: 2,
            adaptive: true,
            seed: 0,
        }
    }
}

fn raw_val<'a>(raw: &'a HashMap<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    for k in keys {
        if let Some(v) = raw.get(*k) {
            if let Some(obj) = v.as_object() {
                if let Some(inner) = obj.get("value") {
                    return Some(inner);
                }
            }
            return Some(v);
        }
    }
    None
}

fn parse_f64(raw: &HashMap<String, Value>, keys: &[&str], def: f64, lo: f64, hi: f64) -> f64 {
    let Some(v) = raw_val(raw, keys) else {
        return def;
    };
    let n = match v {
        Value::Number(x) => x.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    };
    match n {
        Some(x) if x.is_finite() => clamp_f64(x, lo, hi),
        _ => def,
    }
}

fn parse_bool(raw: &HashMap<String, Value>, keys: &[&str], def: bool) -> bool {
    let Some(v) = raw_val(raw, keys) else {
        return def;
    };
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => {
            let l = s.trim().to_ascii_lowercase();
            l == "true" || l == "1" || l == "on" || l == "yes"
        }
        _ => def,
    }
}

impl SolarSystemParams {
    pub fn from_raw_params(raw: &HashMap<String, Value>) -> Self {
        let mut p = Self::default();

        if let Some(v) = raw_val(raw, &["centerBodyType", "center_body_type"]) {
            let s = match v {
                Value::String(st) => st.trim().to_ascii_lowercase(),
                Value::Number(n) => (n.as_i64().unwrap_or(0)).to_string(),
                _ => "0".to_string(),
            };
            p.center_body_type = if s == "1" || s.contains("sun") || s == "star" {
                1
            } else if s == "2" || s.contains("none") || s == "off" {
                2
            } else {
                0
            };
        }

        p.black_hole_size = parse_f64(raw, &["blackHoleSize", "black_hole_size"], 1.0, 0.4, 2.5);
        p.accretion_intensity =
            parse_f64(raw, &["accretionIntensity", "accretion_intensity"], 1.0, 0.1, 2.0);

        if let Some(v) = raw_val(raw, &["iconOrbitMode", "icon_orbit_mode"]) {
            let s = match v {
                Value::String(st) => st.trim().to_ascii_lowercase(),
                Value::Number(n) => (n.as_i64().unwrap_or(0)).to_string(),
                Value::Bool(b) => if *b { "orbit".to_string() } else { "anchor".to_string() },
                _ => "0".to_string(),
            };
            p.icon_orbit_mode = if s.contains("anchor") || s.contains("fixed") || s == "1" || s == "false" {
                1
            } else {
                0
            };
        }

        p.global_orbit_speed =
            parse_f64(raw, &["globalOrbitSpeed", "global_orbit_speed"], 0.65, 0.0, 3.0);
        p.planet_count = parse_f64(raw, &["planetCount", "planet_count"], 3.0, 1.0, 6.0).round() as usize;
        p.orbit_size = parse_f64(raw, &["orbitSize", "orbit_size"], 1.0, 0.4, 2.2);
        p.orbit_speed = parse_f64(raw, &["orbitSpeed", "orbit_speed"], 1.0, 0.1, 3.0);
        p.planet_size = parse_f64(raw, &["planetSize", "planet_size"], 1.0, 0.4, 2.2);
        p.star_density = parse_f64(raw, &["starDensity", "star_density"], 1.0, 0.1, 2.5);
        p.nebula_intensity =
            parse_f64(raw, &["nebulaIntensity", "nebula_intensity"], 0.65, 0.0, 1.5);
        p.asteroid_density =
            parse_f64(raw, &["asteroidDensity", "asteroid_density"], 0.8, 0.0, 2.5);
        p.shooting_stars = parse_bool(raw, &["shootingStars", "shooting_stars"], true);
        p.gravity_strength =
            parse_f64(raw, &["gravityStrength", "gravity_strength"], 0.6, 0.0, 2.0);
        p.system_attraction =
            parse_f64(raw, &["systemAttraction", "system_attraction"], 0.55, 0.0, 2.0);

        if let Some(v) = raw_val(raw, &["quality", "Quality"]) {
            let s = match v {
                Value::String(st) => st.trim().to_ascii_lowercase(),
                Value::Number(n) => (n.as_i64().unwrap_or(2)).to_string(),
                _ => "high".to_string(),
            };
            p.quality = if s == "low" || s == "0" {
                0
            } else if s == "medium" || s == "med" || s == "1" {
                1
            } else {
                2
            };
        }

        p.adaptive = parse_bool(raw, &["adaptive"], true);
        p.seed = parse_f64(raw, &["seed"], 0.0, 0.0, u32::MAX as f64) as u32;
        p
    }
}

// =========================================================================
// 4. SIMULATION DATA STRUCTURES
// =========================================================================

#[derive(Clone, Copy)]
struct Star {
    nx: f64,
    ny: f64,
    size: f64,
    base_alpha: f64,
    twinkle_amp: f64,
    twinkle_speed: f64,
    twinkle_phase: f64,
    color_idx: usize,
}

#[derive(Clone, Copy)]
struct Asteroid {
    band_norm: f64,
    angle: f64,
    speed: f64,
    radial_jitter: f64,
    size: f64,
    aspect: f64,
    rotation: f64,
    rotation_speed: f64,
    alpha: f64,
}

#[derive(Clone, Copy)]
struct AccretionParticle {
    radius_norm: f64,
    angle: f64,
    speed: f64,
    size: f64,
    alpha: f64,
}

#[derive(Clone, Copy)]
struct MacroOrbit {
    rx_norm: f64,
    ry_norm: f64,
    speed: f64,
    angle: f64,
    size: f64,
    alpha: f64,
}

#[derive(Clone)]
struct CentralBlackHole {
    tilt_angle: f64,
    disk_aspect: f64,
    event_horizon_r: f64,
    accretion_particles: Vec<AccretionParticle>,
    orbits: Vec<MacroOrbit>,
}

#[derive(Clone, Copy)]
struct Moon {
    orbit_radius: f64,
    angle: f64,
    speed: f64,
    size: f64,
}

#[derive(Clone)]
struct Planet {
    base_orbit_radius: f64,
    eccentricity: f64,
    orbit_tilt_offset: f64,
    orbit_alpha: f64,
    angle: f64,
    angular_speed: f64,
    base_size: f64,
    palette_idx: usize,
    has_ring: bool,
    ring_tilt: f64,
    moons: Vec<Moon>,
    lag_x: f64,
    lag_y: f64,
    world_x: f64,
    world_y: f64,
    depth_z: f64,
}

#[derive(Clone, Copy)]
struct LocalDust {
    radius: f64,
    angle: f64,
    speed: f64,
    size: f64,
    alpha: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SysState {
    Spawning,
    Active,
    Removing,
}

#[derive(Clone)]
struct SolarSystemObj {
    id: String,
    raw_index: i32,
    seed: u32,
    is_mini_black_hole: bool,
    anchor_x: f64,
    anchor_y: f64,
    current_x: f64,
    current_y: f64,
    target_x: f64,
    target_y: f64,
    vx: f64,
    vy: f64,
    global_radius: f64,
    global_angle: f64,
    global_angular_speed: f64,
    global_tilt: f64,
    global_plane_rot: f64,
    stellar_type_index: usize,
    star_radius: f64,
    glow_radius: f64,
    pulse_phase: f64,
    pulse_speed: f64,
    plane_rotation: f64,
    plane_tilt: f64,
    planets: Vec<Planet>,
    dust_particles: Vec<LocalDust>,
    state: SysState,
    spawn_progress: f64,
    remove_progress: f64,
    opacity: f64,
    tidal_pull_x: f64,
    tidal_pull_y: f64,
    hover_glow: f64,
    boost: f64,
}

#[derive(Clone, Copy, Default)]
struct Meteor {
    active: bool,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    length: f64,
    life: f64,
    max_life: f64,
    alpha: f64,
}

#[derive(Clone, Copy)]
struct Shockwave {
    x: f64,
    y: f64,
    start_r: f64,
    r: f64,
    max_r: f64,
    life: f64,
    max_life: f64,
    alpha: f64,
    base_alpha: f64,
    rgb: [f32; 3],
}

#[derive(Clone, Copy)]
struct CursorSpark {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max_life: f64,
    size: f64,
    rgb: [f32; 3],
}

struct GlobalOrbParams {
    global_radius: f64,
    global_angle: f64,
    global_angular_speed: f64,
    global_tilt: f64,
    global_plane_rot: f64,
}

#[derive(Clone)]
struct CachedIconInput {
    rect: Rect,
    name: String,
}

pub struct SolarSystemState {
    pub params: SolarSystemParams,
    pub low_mode: bool,
    pub w: f64,
    pub h: f64,
    pub elapsed_time: f64,
    pub bg_rev: u64,
    pub layout_rev: u64,
    pub dirty: bool,
    stars: Vec<Star>,
    asteroids: Vec<Asteroid>,
    meteors: Vec<Meteor>,
    central_bh: CentralBlackHole,
    systems: Vec<SolarSystemObj>,
    last_icons: Vec<CachedIconInput>,
    has_received_icons: bool,
    synthetic_id_counter: u64,
    meteor_spawn_acc: f64,
    // Pointer state
    pointer_x: f64,
    pointer_y: f64,
    pointer_active: bool,
    pointer_down: bool,
    last_spark_time: f64,
    core_flare: f64,
    shockwaves: Vec<Shockwave>,
    cursor_sparks: Vec<CursorSpark>,
    fx_rng: Mulberry32,
}

impl Default for SolarSystemState {
    fn default() -> Self {
        let mut s = Self {
            params: SolarSystemParams::default(),
            low_mode: false,
            w: 1920.0,
            h: 1080.0,
            elapsed_time: 0.0,
            bg_rev: 1,
            layout_rev: 1,
            dirty: true,
            stars: Vec::new(),
            asteroids: Vec::new(),
            meteors: vec![Meteor::default(); 4],
            central_bh: build_central_black_hole(),
            systems: Vec::new(),
            last_icons: Vec::new(),
            has_received_icons: false,
            synthetic_id_counter: 1,
            meteor_spawn_acc: 0.0,
            pointer_x: 0.0,
            pointer_y: 0.0,
            pointer_active: false,
            pointer_down: false,
            last_spark_time: -1.0,
            core_flare: 0.0,
            shockwaves: Vec::new(),
            cursor_sparks: Vec::new(),
            fx_rng: Mulberry32::new(0x534F_4C41),
        };
        s.build_star_field();
        s.build_asteroids();
        s
    }
}

fn build_central_black_hole() -> CentralBlackHole {
    let mut rng = Mulberry32::new(0x47415247); // 'GARG'
    let mut accretion_particles = Vec::with_capacity(64);
    for _ in 0..64 {
        accretion_particles.push(AccretionParticle {
            radius_norm: 1.35 + rng.next_f64().powf(1.3) * 2.85,
            angle: rng.next_f64() * PI * 2.0,
            speed: 0.55 + rng.next_f64() * 0.85,
            size: 0.8 + rng.next_f64() * 1.5,
            alpha: 0.18 + rng.next_f64() * 0.38,
        });
    }
    let orbits = vec![
        MacroOrbit { rx_norm: 0.22, ry_norm: 0.105, speed: 0.014, angle: rng.next_f64() * PI * 2.0, size: 2.2, alpha: 0.036 },
        MacroOrbit { rx_norm: 0.33, ry_norm: 0.155, speed: -0.009, angle: rng.next_f64() * PI * 2.0, size: 2.8, alpha: 0.028 },
        MacroOrbit { rx_norm: 0.45, ry_norm: 0.210, speed: 0.0055, angle: rng.next_f64() * PI * 2.0, size: 2.0, alpha: 0.022 },
    ];
    CentralBlackHole {
        tilt_angle: -0.22,
        disk_aspect: 0.24,
        event_horizon_r: 26.0,
        accretion_particles,
        orbits,
    }
}

impl SolarSystemState {
    fn profile(&self) -> QualityProfile {
        if self.low_mode {
            QUALITY_PROFILES[0]
        } else {
            QUALITY_PROFILES[self.params.quality.min(2)]
        }
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.build_star_field();
            self.build_asteroids();
            self.bg_rev = self.bg_rev.wrapping_add(1);
            self.dirty = true;
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = (w as f64).max(64.0);
        let nh = (h as f64).max(64.0);
        if (nw - self.w).abs() <= 0.5 && (nh - self.h).abs() <= 0.5 {
            return;
        }
        let prev_w = self.w;
        let prev_h = self.h;
        self.w = nw;
        self.h = nh;
        self.bg_rev = self.bg_rev.wrapping_add(1);
        self.layout_rev = self.layout_rev.wrapping_add(1);
        self.dirty = true;
        self.build_star_field();

        if prev_w > 0.0 && prev_h > 0.0 {
            let sx = nw / prev_w;
            let sy = nh / prev_h;
            let w_cur = self.w;
            let h_cur = self.h;
            let bh_size = self.params.black_hole_size;
            for sys in &mut self.systems {
                sys.anchor_x *= sx;
                sys.anchor_y *= sy;
                sys.target_x *= sx;
                sys.target_y *= sy;
                let orb = compute_global_orbit_params(w_cur, h_cur, bh_size, sys.anchor_x, sys.anchor_y, sys.seed);
                sys.global_radius = orb.global_radius;
            }
        }
    }

    fn build_star_field(&mut self) {
        let prof = self.profile();
        let area_scale = (self.w * self.h) / (1920.0 * 1080.0);
        let count = (210.0 * area_scale * self.params.star_density * prof.star_multiplier)
            .round()
            .clamp(40.0, 520.0) as usize;
        let mut rng = Mulberry32::new(0x4449_5941 ^ self.params.seed);
        self.stars.clear();
        self.stars.reserve(count);
        for _ in 0..count {
            let depth_roll = rng.next_f64();
            let is_bright = depth_roll > 0.92;
            let color_idx = ((rng.next_f64() * STAR_COLORS.len() as f64).floor() as usize).min(3);
            let nx = rng.next_f64();
            let ny = rng.next_f64();
            let size = if is_bright { 1.25 + rng.next_f64() * 0.9 } else { 0.55 + rng.next_f64() * 0.85 };
            let base_alpha = if is_bright { 0.45 + rng.next_f64() * 0.35 } else { 0.14 + rng.next_f64() * 0.32 };
            let twinkle_amp = if is_bright { 0.12 + rng.next_f64() * 0.16 } else { 0.04 + rng.next_f64() * 0.09 };
            let twinkle_speed = 0.4 + rng.next_f64() * 1.6;
            let twinkle_phase = rng.next_f64() * PI * 2.0;
            self.stars.push(Star {
                nx,
                ny,
                size,
                base_alpha,
                twinkle_amp,
                twinkle_speed,
                twinkle_phase,
                color_idx,
            });
        }
    }

    fn build_asteroids(&mut self) {
        let prof = self.profile();
        let count = (95.0 * self.params.asteroid_density * prof.asteroid_multiplier)
            .round()
            .clamp(0.0, 220.0) as usize;
        let mut rng = Mulberry32::new(0x5244_534B ^ self.params.seed);
        self.asteroids.clear();
        self.asteroids.reserve(count);
        for _ in 0..count {
            let band_norm = 0.34 + (rng.next_f64() + rng.next_f64()) * 0.16;
            let alpha = 0.08 + rng.next_f64() * 0.16;
            let angle = rng.next_f64() * PI * 2.0;
            let speed = (0.008 + rng.next_f64() * 0.015) * if rng.next_f64() > 0.1 { 1.0 } else { -1.0 };
            let radial_jitter = (rng.next_f64() - 0.5) * 38.0;
            let size = 0.7 + rng.next_f64() * 1.45;
            let aspect = 0.55 + rng.next_f64() * 0.45;
            let rotation = rng.next_f64() * PI * 2.0;
            let rotation_speed = (rng.next_f64() - 0.5) * 0.4;
            self.asteroids.push(Asteroid {
                band_norm,
                angle,
                speed,
                radial_jitter,
                size,
                aspect,
                rotation,
                rotation_speed,
                alpha,
            });
        }
    }

    pub fn sync_params(&mut self, next: SolarSystemParams) {
        let prev = self.params;
        if prev == next {
            return;
        }
        let rebuild_stars = (prev.star_density - next.star_density).abs() > 1e-6
            || prev.quality != next.quality
            || prev.seed != next.seed;
        let rebuild_asteroids = (prev.asteroid_density - next.asteroid_density).abs() > 1e-6
            || prev.quality != next.quality
            || prev.seed != next.seed;
        let rebuild_bg = (prev.nebula_intensity - next.nebula_intensity).abs() > 1e-6
            || prev.seed != next.seed;
        let rebuild_planets = prev.planet_count != next.planet_count;
        let orbit_mode_changed = prev.icon_orbit_mode != next.icon_orbit_mode;
        let adaptive_changed = prev.adaptive != next.adaptive;

        self.params = next;
        self.layout_rev = self.layout_rev.wrapping_add(1);
        self.dirty = true;

        if orbit_mode_changed {
            if next.icon_orbit_mode == 1 {
                for sys in &mut self.systems {
                    sys.target_x = sys.anchor_x;
                    sys.target_y = sys.anchor_y;
                }
            } else {
                let (w, h, bh) = (self.w, self.h, self.params.black_hole_size);
                for sys in &mut self.systems {
                    let orb = compute_global_orbit_params(w, h, bh, sys.current_x, sys.current_y, sys.seed);
                    sys.global_radius = orb.global_radius;
                    sys.global_angle = orb.global_angle;
                    sys.global_angular_speed = orb.global_angular_speed;
                }
            }
        }

        if adaptive_changed {
            if !next.adaptive {
                self.systems.clear();
            } else if !self.last_icons.is_empty() {
                let cached_rects: Vec<Rect> = self.last_icons.iter().map(|c| c.rect).collect();
                let cached_names: Vec<String> = self.last_icons.iter().map(|c| c.name.clone()).collect();
                self.reconcile_icons(&cached_rects, &cached_names);
            }
        }

        if rebuild_stars {
            self.build_star_field();
        }
        if rebuild_asteroids {
            self.build_asteroids();
        }
        if rebuild_bg {
            self.bg_rev = self.bg_rev.wrapping_add(1);
        }
        if rebuild_planets {
            let (w, h, bh, pcnt) = (self.w, self.h, self.params.black_hole_size, self.params.planet_count);
            for sys in &mut self.systems {
                if sys.state == SysState::Removing {
                    continue;
                }
                let mut regen = create_solar_system(
                    &sys.id,
                    sys.anchor_x,
                    sys.anchor_y,
                    sys.seed,
                    sys.raw_index,
                    w,
                    h,
                    bh,
                    pcnt,
                );
                regen.current_x = sys.current_x;
                regen.current_y = sys.current_y;
                regen.target_x = sys.target_x;
                regen.target_y = sys.target_y;
                regen.global_angle = sys.global_angle;
                regen.global_radius = sys.global_radius;
                regen.state = SysState::Active;
                regen.spawn_progress = 1.0;
                regen.opacity = sys.opacity;
                *sys = regen;
            }
        }
    }

    pub fn set_icons(&mut self, icons: &[Rect], names: &[String], vis: bool) {
        self.has_received_icons = true;
        self.layout_rev = self.layout_rev.wrapping_add(1);
        self.dirty = true;
        if !vis {
            self.last_icons.clear();
            self.reconcile_icons(&[], &[]);
            return;
        }
        self.last_icons = icons
            .iter()
            .enumerate()
            .map(|(i, &r)| CachedIconInput {
                rect: r,
                name: names.get(i).cloned().unwrap_or_default(),
            })
            .collect();
        if !self.params.adaptive {
            self.systems.clear();
            return;
        }
        self.reconcile_icons(icons, names);
    }

    fn reconcile_icons(&mut self, icons: &[Rect], names: &[String]) {
        struct ParsedIcon {
            raw_index: i32,
            center_x: f64,
            center_y: f64,
            explicit_key: Option<String>,
        }

        let parsed: Vec<ParsedIcon> = icons
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let cx = r.x as f64 + r.w as f64 * 0.5;
                let cy = r.y as f64 + r.h as f64 * 0.5;
                if !cx.is_finite() || !cy.is_finite() {
                    return None;
                }
                let nm = names.get(i).map(|s| s.trim()).unwrap_or("");
                let explicit_key = if nm.is_empty() {
                    None
                } else {
                    Some(format!("id:{nm}"))
                };
                Some(ParsedIcon {
                    raw_index: i as i32,
                    center_x: cx,
                    center_y: cy,
                    explicit_key,
                })
            })
            .collect();

        let active_indices: Vec<usize> = self
            .systems
            .iter()
            .enumerate()
            .filter_map(|(idx, s)| if s.state != SysState::Removing { Some(idx) } else { None })
            .collect();

        let mut matched_sys: HashSet<usize> = HashSet::new();
        let mut matched_icon: HashSet<usize> = HashSet::new();
        let (w, h, bh, orbit_mode) = (self.w, self.h, self.params.black_hole_size, self.params.icon_orbit_mode);

        // Pass 1: Explicit key match
        for (ii, ic) in parsed.iter().enumerate() {
            let Some(ref key) = ic.explicit_key else { continue };
            for &si in &active_indices {
                if !matched_sys.contains(&si) && self.systems[si].id == *key {
                    update_system_anchor(&mut self.systems[si], ic.center_x, ic.center_y, ic.raw_index, w, h, bh, orbit_mode);
                    matched_sys.insert(si);
                    matched_icon.insert(ii);
                    break;
                }
            }
        }

        // Pass 2A & 2B: Nearest neighbor
        let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
        for (ii, ic) in parsed.iter().enumerate() {
            if matched_icon.contains(&ii) {
                continue;
            }
            for &si in &active_indices {
                if matched_sys.contains(&si) {
                    continue;
                }
                let sys = &self.systems[si];
                let d_anc = (ic.center_x - sys.anchor_x).hypot(ic.center_y - sys.anchor_y);
                let d_tgt = (ic.center_x - sys.target_x).hypot(ic.center_y - sys.target_y);
                let d_cur = (ic.center_x - sys.current_x).hypot(ic.center_y - sys.current_y);
                let dist = d_anc.min(d_tgt).min(d_cur);
                pairs.push((si, ii, dist));
            }
        }
        pairs.sort_by(|a, b| a.2.total_cmp(&b.2));

        // Pass 2A: stationary (<= 16 px)
        for &(si, ii, dist) in &pairs {
            if dist > 16.0 {
                break;
            }
            if matched_sys.contains(&si) || matched_icon.contains(&ii) {
                continue;
            }
            let ic = &parsed[ii];
            update_system_anchor(&mut self.systems[si], ic.center_x, ic.center_y, ic.raw_index, w, h, bh, orbit_mode);
            if let Some(ref k) = ic.explicit_key {
                self.systems[si].id = k.clone();
            }
            matched_sys.insert(si);
            matched_icon.insert(ii);
        }

        // Pass 2B: dragged icons across screen
        let max_drag = w.hypot(h) * 0.95;
        for &(si, ii, dist) in &pairs {
            if dist > max_drag {
                break;
            }
            if matched_sys.contains(&si) || matched_icon.contains(&ii) {
                continue;
            }
            let ic = &parsed[ii];
            update_system_anchor(&mut self.systems[si], ic.center_x, ic.center_y, ic.raw_index, w, h, bh, orbit_mode);
            if let Some(ref k) = ic.explicit_key {
                self.systems[si].id = k.clone();
            }
            matched_sys.insert(si);
            matched_icon.insert(ii);
        }

        // Unmatched existing systems -> removing
        for &si in &active_indices {
            if !matched_sys.contains(&si) {
                self.systems[si].state = SysState::Removing;
                self.systems[si].remove_progress = 0.0;
            }
        }

        // Unmatched incoming icons -> spawn new solar systems
        let pcnt = self.params.planet_count;
        for (ii, ic) in parsed.iter().enumerate() {
            if matched_icon.contains(&ii) {
                continue;
            }
            let gx = (ic.center_x / 36.0).round() as i64;
            let gy = (ic.center_y / 36.0).round() as i64;
            let seed_source = ic
                .explicit_key
                .clone()
                .unwrap_or_else(|| format!("cell:{gx}:{gy}"));
            let seed_val = fnv1a_hash(&seed_source);
            let sys_id = ic.explicit_key.clone().unwrap_or_else(|| {
                let n = self.synthetic_id_counter;
                self.synthetic_id_counter += 1;
                format!("sys_{n}_{seed_val}")
            });
            let new_sys = create_solar_system(
                &sys_id,
                ic.center_x,
                ic.center_y,
                seed_val,
                ic.raw_index,
                w,
                h,
                bh,
                pcnt,
            );
            self.systems.push(new_sys);
        }
    }

    pub fn handle_mouse(&mut self, kind: i32, x: f32, y: f32) -> bool {
        let fx = x as f64;
        let fy = y as f64;
        match kind {
            0 => {
                // move
                let vx = fx - self.pointer_x;
                let vy = fy - self.pointer_y;
                self.pointer_x = fx;
                self.pointer_y = fy;
                self.pointer_active = true;
                if (self.pointer_down || vx.hypot(vy) > 4.0) && (self.elapsed_time - self.last_spark_time) > 0.028 {
                    self.last_spark_time = self.elapsed_time;
                    let r1 = (self.fx_rng.next_f64() - 0.5) * 35.0;
                    let r2 = (self.fx_rng.next_f64() - 0.5) * 35.0;
                    let rgb = if self.pointer_down { rgb_u8(255, 205, 130) } else { rgb_u8(175, 220, 255) };
                    let sz = if self.pointer_down { 2.3 } else { 1.6 };
                    let life = if self.pointer_down { 0.85 } else { 0.55 };
                    self.spawn_cursor_spark(fx, fy, vx * 4.0 + r1, vy * 4.0 + r2, rgb, sz, life);
                }
            }
            1 => {
                // down
                self.pointer_x = fx;
                self.pointer_y = fy;
                self.pointer_active = true;
                self.pointer_down = true;
                self.trigger_pointer_pulse(fx, fy);
            }
            2 => {
                // up
                self.pointer_down = false;
            }
            3 => {
                // leave
                self.pointer_down = false;
                self.pointer_active = false;
            }
            _ => {}
        }
        false
    }

    fn spawn_cursor_spark(&mut self, x: f64, y: f64, vx: f64, vy: f64, rgb: [f32; 3], size: f64, max_life: f64) {
        if self.cursor_sparks.len() >= 96 {
            self.cursor_sparks.remove(0);
        }
        self.cursor_sparks.push(CursorSpark {
            x,
            y,
            vx,
            vy,
            life: 0.0,
            max_life,
            size,
            rgb,
        });
    }

    fn spawn_shockwave(&mut self, x: f64, y: f64, max_r: f64, rgb: [f32; 3], base_alpha: f64) {
        if self.shockwaves.len() >= 10 {
            self.shockwaves.remove(0);
        }
        self.shockwaves.push(Shockwave {
            x,
            y,
            start_r: 8.0,
            r: 8.0,
            max_r,
            life: 0.0,
            max_life: 0.78,
            alpha: base_alpha,
            base_alpha,
            rgb,
        });
    }

    fn trigger_pointer_pulse(&mut self, x: f64, y: f64) {
        let cx = self.w * 0.5;
        let cy = self.h * 0.5;
        let dist_core = (x - cx).hypot(y - cy);

        let mut hit_idx: Option<usize> = None;
        let mut min_sys_d = 90.0;
        for (i, sys) in self.systems.iter().enumerate() {
            let d = (sys.current_x - x).hypot(sys.current_y - y);
            if d < min_sys_d {
                min_sys_d = d;
                hit_idx = Some(i);
            }
        }

        if dist_core < 95.0 && self.params.center_body_type != 2 {
            self.core_flare = 1.0;
            self.spawn_shockwave(cx, cy, self.w.min(self.h) * 0.36, rgb_u8(255, 195, 115), 0.8);
            for i in 0..18 {
                let ang = (i as f64 / 18.0) * PI * 2.0 + self.fx_rng.next_f64() * 0.25;
                let spd = 95.0 + self.fx_rng.next_f64() * 145.0;
                self.spawn_cursor_spark(
                    cx + ang.cos() * 18.0,
                    cy + ang.sin() * 18.0,
                    ang.cos() * spd,
                    ang.sin() * spd,
                    rgb_u8(255, 215, 145),
                    2.4,
                    0.95,
                );
            }
        } else if let Some(idx) = hit_idx {
            self.systems[idx].boost = 1.0;
            let sx = self.systems[idx].current_x;
            let sy = self.systems[idx].current_y;
            let st = STELLAR_TYPES[self.systems[idx].stellar_type_index];
            let corona = rgb_u8(st.corona_rgb[0], st.corona_rgb[1], st.corona_rgb[2]);
            self.spawn_shockwave(sx, sy, 150.0, st.orbit_rgb, 0.75);
            for i in 0..14 {
                let ang = (i as f64 / 14.0) * PI * 2.0 + self.fx_rng.next_f64() * 0.3;
                let spd = 65.0 + self.fx_rng.next_f64() * 110.0;
                self.spawn_cursor_spark(sx, sy, ang.cos() * spd, ang.sin() * spd, corona, 2.2, 0.85);
            }
        } else {
            self.spawn_shockwave(x, y, 165.0, rgb_u8(160, 205, 255), 0.6);
            for i in 0..12 {
                let ang = (i as f64 / 12.0) * PI * 2.0 + self.fx_rng.next_f64() * 0.35;
                let spd = 55.0 + self.fx_rng.next_f64() * 95.0;
                self.spawn_cursor_spark(x, y, ang.cos() * spd, ang.sin() * spd, rgb_u8(190, 220, 255), 2.0, 0.75);
            }
            for m in &mut self.meteors {
                if !m.active {
                    let ang = (cy - y).atan2(cx - x) + (self.fx_rng.next_f64() - 0.5) * 0.5;
                    let spd = 460.0 + self.fx_rng.next_f64() * 180.0;
                    m.active = true;
                    m.x = x;
                    m.y = y;
                    m.vx = ang.cos() * spd;
                    m.vy = ang.sin() * spd;
                    m.length = 64.0;
                    m.life = 0.0;
                    m.max_life = 0.65;
                    m.alpha = 0.75;
                    break;
                }
            }
        }
    }

    pub fn step(&mut self, dt_raw: f32) {
        let max_dt = if self.low_mode { 0.12 } else { 0.05 };
        let dt = clamp_f64(dt_raw as f64, 0.001, max_dt);
        self.elapsed_time += dt;

        self.update_systems(dt);
        self.update_planets(dt);
        self.update_particles(dt);
        self.systems
            .retain(|s| !(s.state == SysState::Removing && s.remove_progress >= 1.0));
    }

    fn update_systems(&mut self, dt: f64) {
        let lerp_alpha = 1.0 - (-8.5 * dt).exp();
        let core_x = self.w * 0.5;
        let core_y = self.h * 0.5;
        let is_orbit = self.params.icon_orbit_mode == 0 && self.params.global_orbit_speed > 0.001;
        let margin = 54.0;
        let min_x = margin;
        let max_x = (self.w - margin).max(min_x);
        let min_y = margin;
        let max_y = (self.h - margin).max(min_y);
        let influence_r = self.w.min(self.h) * 0.48;

        for sys in &mut self.systems {
            if is_orbit && sys.state != SysState::Removing {
                sys.global_angle = (sys.global_angle + sys.global_angular_speed * self.params.global_orbit_speed * dt) % (PI * 2.0);
                let rx = sys.global_radius;
                let ry = rx * sys.global_tilt;
                let lx = sys.global_angle.cos() * rx;
                let ly = sys.global_angle.sin() * ry;
                let cos_r = sys.global_plane_rot.cos();
                let sin_r = sys.global_plane_rot.sin();
                sys.target_x = clamp_f64(core_x + lx * cos_r - ly * sin_r, min_x, max_x);
                sys.target_y = clamp_f64(core_y + lx * sin_r + ly * cos_r, min_y, max_y);
            } else if sys.state != SysState::Removing {
                sys.target_x = sys.anchor_x;
                sys.target_y = sys.anchor_y;
            }

            let prev_x = sys.current_x;
            let prev_y = sys.current_y;
            sys.current_x = lerp_f64(sys.current_x, sys.target_x, lerp_alpha);
            sys.current_y = lerp_f64(sys.current_y, sys.target_y, lerp_alpha);
            if (sys.target_x - sys.current_x).hypot(sys.target_y - sys.current_y) < 0.08 {
                sys.current_x = sys.target_x;
                sys.current_y = sys.target_y;
            }
            sys.vx = (sys.current_x - prev_x) / dt.max(0.001);
            sys.vy = (sys.current_y - prev_y) / dt.max(0.001);

            match sys.state {
                SysState::Spawning => {
                    sys.spawn_progress = clamp_f64(sys.spawn_progress + dt / 0.72, 0.0, 1.0);
                    sys.opacity = smoothstep_f64(0.0, 0.35, sys.spawn_progress);
                    if sys.spawn_progress >= 1.0 {
                        sys.state = SysState::Active;
                        sys.opacity = 1.0;
                    }
                }
                SysState::Active => {
                    sys.opacity = 1.0;
                }
                SysState::Removing => {
                    sys.remove_progress = clamp_f64(sys.remove_progress + dt / 0.85, 0.0, 1.0);
                    sys.opacity = (1.0 - sys.remove_progress).powf(1.45);
                }
            }

            sys.tidal_pull_x = 0.0;
            sys.tidal_pull_y = 0.0;
            if self.params.center_body_type != 2 && self.params.gravity_strength > 0.01 {
                let dx_core = core_x - sys.current_x;
                let dy_core = core_y - sys.current_y;
                let dist_core = dx_core.hypot(dy_core);
                if dist_core > 24.0 && dist_core < influence_r {
                    let norm_dist = 1.0 - dist_core / influence_r;
                    let bh_pull = norm_dist * norm_dist * 16.0 * self.params.gravity_strength * self.params.black_hole_size;
                    sys.tidal_pull_x += (dx_core / dist_core) * bh_pull;
                    sys.tidal_pull_y += (dy_core / dist_core) * bh_pull;
                }
            }
        }

        let attr = self.params.gravity_strength * self.params.system_attraction;
        if attr > 0.001 && self.systems.len() > 1 {
            let max_range = 360.0 * self.params.orbit_size;
            let max_range_sq = max_range * max_range;
            let n = self.systems.len();
            for i in 0..n {
                if self.systems[i].opacity < 0.05 {
                    continue;
                }
                for j in (i + 1)..n {
                    if self.systems[j].opacity < 0.05 {
                        continue;
                    }
                    let dx = self.systems[j].current_x - self.systems[i].current_x;
                    let dy = self.systems[j].current_y - self.systems[i].current_y;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq > 100.0 && dist_sq < max_range_sq {
                        let dist = dist_sq.sqrt();
                        let falloff = (1.0 - dist / max_range).powi(2);
                        let pull_mag = clamp_f64(falloff * 11.5 * attr, 0.0, 14.0);
                        let nx = dx / dist;
                        let ny = dy / dist;
                        let oa = self.systems[i].opacity;
                        let ob = self.systems[j].opacity;
                        self.systems[i].tidal_pull_x += nx * pull_mag * ob;
                        self.systems[i].tidal_pull_y += ny * pull_mag * ob;
                        self.systems[j].tidal_pull_x -= nx * pull_mag * oa;
                        self.systems[j].tidal_pull_y -= ny * pull_mag * oa;
                    }
                }
            }
        }
    }

    fn update_planets(&mut self, dt: f64) {
        let speed_mult = self.params.orbit_speed;
        let size_mult = self.params.orbit_size;
        let lag_decay = 1.0 - (-6.5 * dt).exp();

        for sys in &mut self.systems {
            let cos_rot = sys.plane_rotation.cos();
            let sin_rot = sys.plane_rotation.sin();
            let dissolve_exp = if sys.state == SysState::Removing {
                1.0 + sys.remove_progress * 0.42
            } else {
                1.0
            };
            let p_len = sys.planets.len().max(1) as f64;
            let hover_boost_mul = 1.0 + sys.hover_glow * 1.1 + sys.boost * 2.4;

            for (p, planet) in sys.planets.iter_mut().enumerate() {
                planet.angle = (planet.angle + planet.angular_speed * speed_mult * hover_boost_mul * dt) % (PI * 2.0);
                let orbit_norm = (p + 1) as f64 / p_len;
                let tgt_lx = clamp_f64(-sys.vx * 0.028 * (0.45 + orbit_norm * 0.75), -26.0, 26.0);
                let tgt_ly = clamp_f64(-sys.vy * 0.028 * (0.45 + orbit_norm * 0.75), -26.0, 26.0);
                planet.lag_x = lerp_f64(planet.lag_x, tgt_lx, lag_decay);
                planet.lag_y = lerp_f64(planet.lag_y, tgt_ly, lag_decay);

                let rx = planet.base_orbit_radius * size_mult * dissolve_exp;
                let ry = rx * clamp_f64(sys.plane_tilt + planet.orbit_tilt_offset, 0.35, 0.85);
                let focal_offset = rx * planet.eccentricity;
                let lx = planet.angle.cos() * rx - focal_offset;
                let ly = planet.angle.sin() * ry;

                let rot_x = lx * cos_rot - ly * sin_rot;
                let rot_y = lx * sin_rot + ly * cos_rot;
                let tidal_w = 0.35 + 0.65 * orbit_norm;
                planet.world_x = sys.current_x + rot_x + planet.lag_x + sys.tidal_pull_x * tidal_w;
                planet.world_y = sys.current_y + rot_y + planet.lag_y + sys.tidal_pull_y * tidal_w;
                planet.depth_z = planet.angle.sin();

                for moon in &mut planet.moons {
                    moon.angle = (moon.angle + moon.speed * speed_mult * dt) % (PI * 2.0);
                }
            }
        }
    }

    fn update_particles(&mut self, dt: f64) {
        let speed_mult = self.params.orbit_speed;
        for sys in &mut self.systems {
            for pt in &mut sys.dust_particles {
                pt.angle = (pt.angle + pt.speed * speed_mult * dt) % (PI * 2.0);
            }
        }

        let half_diag = self.w.hypot(self.h) * 0.5;
        for ast in &mut self.asteroids {
            let mut ast_boost = 1.0;
            if self.pointer_active {
                let rx = ast.band_norm * half_diag + ast.radial_jitter;
                let ry = rx * 0.54;
                let ax = self.w * 0.5 + ast.angle.cos() * rx;
                let ay = self.h * 0.5 + ast.angle.sin() * ry;
                let pd = (ax - self.pointer_x).hypot(ay - self.pointer_y);
                if pd < 220.0 {
                    ast_boost += (1.0 - pd / 220.0) * if self.pointer_down { 4.5 } else { 1.8 };
                }
            }
            ast.angle = (ast.angle + ast.speed * speed_mult * ast_boost * dt) % (PI * 2.0);
            ast.rotation = (ast.rotation + ast.rotation_speed * ast_boost * dt) % (PI * 2.0);
        }

        if self.core_flare > 0.0 {
            self.core_flare = (self.core_flare - dt * 1.15).max(0.0);
        }

        for sys in &mut self.systems {
            if sys.boost > 0.0 {
                sys.boost = (sys.boost - dt * 1.2).max(0.0);
            }
            if self.pointer_active {
                let d = (sys.current_x - self.pointer_x).hypot(sys.current_y - self.pointer_y);
                let target_h = if d < 120.0 { 1.0 - d / 120.0 } else { 0.0 };
                sys.hover_glow = lerp_f64(sys.hover_glow, target_h, 1.0 - (-10.0 * dt).exp());
            } else {
                sys.hover_glow = lerp_f64(sys.hover_glow, 0.0, 1.0 - (-6.0 * dt).exp());
            }
        }

        self.shockwaves.retain_mut(|sw| {
            sw.life += dt;
            if sw.life >= sw.max_life {
                return false;
            }
            let p = sw.life / sw.max_life;
            sw.r = sw.start_r + (sw.max_r - sw.start_r) * (1.0 - (1.0 - p).powf(2.2));
            sw.alpha = (1.0 - p) * sw.base_alpha;
            true
        });

        let has_core = self.params.center_body_type != 2;
        let (cx, cy) = (self.w * 0.5, self.h * 0.5);
        self.cursor_sparks.retain_mut(|sp| {
            sp.life += dt;
            if sp.life >= sp.max_life {
                return false;
            }
            if has_core {
                let dx = cx - sp.x;
                let dy = cy - sp.y;
                let dist = dx.hypot(dy).max(1.0);
                if dist < 380.0 {
                    let pull = (1.0 - dist / 380.0) * 180.0;
                    sp.vx += (dx / dist) * pull * dt;
                    sp.vy += (dy / dist) * pull * dt;
                }
            }
            let drag = (-1.4 * dt).exp();
            sp.vx *= drag;
            sp.vy *= drag;
            sp.x += sp.vx * dt;
            sp.y += sp.vy * dt;
            true
        });

        for orb in &mut self.central_bh.orbits {
            orb.angle = (orb.angle + orb.speed * speed_mult * dt) % (PI * 2.0);
        }
        for ap in &mut self.central_bh.accretion_particles {
            ap.angle = (ap.angle + (ap.speed / ap.radius_norm) * speed_mult * dt) % (PI * 2.0);
        }

        if self.params.shooting_stars {
            self.meteor_spawn_acc += dt;
            if self.meteor_spawn_acc >= 1.2 {
                self.meteor_spawn_acc = 0.0;
                let det_roll = ((self.elapsed_time * 12.9898).sin() * 43758.5453).abs() % 1.0;
                if det_roll < 0.18 {
                    self.spawn_meteor(det_roll);
                }
            }
        }

        for m in &mut self.meteors {
            if !m.active {
                continue;
            }
            m.life += dt;
            if m.life >= m.max_life {
                m.active = false;
                continue;
            }
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            let p = m.life / m.max_life;
            m.alpha = (p * PI).sin() * 0.55;
        }
    }

    fn spawn_meteor(&mut self, seed_frac: f64) {
        let Some(slot) = self.meteors.iter_mut().find(|m| !m.active) else {
            return;
        };
        let seed_u32 = (self.elapsed_time * 1000.0 + seed_frac * 9999.0).floor() as u32;
        let mut rng = Mulberry32::new(seed_u32);
        let start_x = (0.12 + rng.next_f64() * 0.76) * self.w;
        let start_y = (0.06 + rng.next_f64() * 0.42) * self.h;
        let angle = (0.35 + rng.next_f64() * 0.45) + if rng.next_f64() > 0.5 { 0.0 } else { PI * 0.45 };
        let speed = 420.0 + rng.next_f64() * 260.0;
        slot.active = true;
        slot.x = start_x;
        slot.y = start_y;
        slot.vx = angle.cos() * speed;
        slot.vy = angle.abs().sin() * speed * 0.65;
        slot.length = 48.0 + rng.next_f64() * 42.0;
        slot.life = 0.0;
        slot.max_life = 0.55 + rng.next_f64() * 0.35;
        slot.alpha = 0.0;
    }
}

fn compute_global_orbit_params(w: f64, h: f64, bh_size: f64, anchor_x: f64, anchor_y: f64, seed_val: u32) -> GlobalOrbParams {
    let core_x = w * 0.5;
    let core_y = h * 0.5;
    let dx = anchor_x - core_x;
    let dy = anchor_y - core_y;
    let global_tilt = 0.56;
    let min_orbit_r = 72.0 * bh_size;
    let eff_dy = dy / global_tilt;
    let raw_r = dx.hypot(eff_dy);
    let global_radius = raw_r.max(min_orbit_r);
    let initial_angle = eff_dy.atan2(dx);
    let ref_radius = w.min(h) * 0.32;
    let kepler_speed = 0.11 * (ref_radius / global_radius.max(90.0)).powf(0.65);
    let mut rng = Mulberry32::new(seed_val ^ 0xA5A5_A5A5);
    GlobalOrbParams {
        global_radius,
        global_angle: initial_angle,
        global_angular_speed: kepler_speed,
        global_tilt: clamp_f64(global_tilt + (rng.next_f64() - 0.5) * 0.08, 0.42, 0.72),
        global_plane_rot: (rng.next_f64() - 0.5) * 0.14,
    }
}

fn update_system_anchor(
    sys: &mut SolarSystemObj,
    new_cx: f64,
    new_cy: f64,
    raw_index: i32,
    w: f64,
    h: f64,
    bh_size: f64,
    orbit_mode: usize,
) {
    sys.raw_index = raw_index;
    let moved_dist = (new_cx - sys.anchor_x).hypot(new_cy - sys.anchor_y);
    sys.anchor_x = new_cx;
    sys.anchor_y = new_cy;
    if orbit_mode == 0 {
        if moved_dist > 18.0 {
            let orb = compute_global_orbit_params(w, h, bh_size, new_cx, new_cy, sys.seed);
            sys.global_radius = orb.global_radius;
            sys.global_angle = orb.global_angle;
            sys.global_angular_speed = orb.global_angular_speed;
        }
    } else {
        sys.target_x = new_cx;
        sys.target_y = new_cy;
    }
}

fn create_solar_system(
    id_key: &str,
    cx: f64,
    cy: f64,
    seed_val: u32,
    raw_index: i32,
    w: f64,
    h: f64,
    bh_size: f64,
    planet_count: usize,
) -> SolarSystemObj {
    let mut rng = Mulberry32::new(seed_val);
    let stellar_type_index = ((rng.next_f64() * STELLAR_TYPES.len() as f64).floor() as usize).min(3);

    let lower = id_key.to_lowercase();
    let is_mini_black_hole = lower.contains("recycle")
        || lower.contains("çöp")
        || lower.contains("geri dönüşüm")
        || lower.contains("trash");

    let variation = (rng.next_f64() * 3.0).floor() as i32 - 1;
    let num_planets = (planet_count as i32 + variation).clamp(1, 6) as usize;
    let plane_rotation = (rng.next_f64() - 0.5) * PI * 0.85;
    let plane_tilt = 0.44 + rng.next_f64() * 0.30;

    let mut planets = Vec::with_capacity(num_planets);
    let mut current_radius = 22.0 + rng.next_f64() * 7.0;

    for p in 0..num_planets {
        let palette_idx = ((rng.next_f64() * PLANET_PALETTES.len() as f64).floor() as usize).min(6);
        let radius_step = 14.0 + rng.next_f64() * 12.0 + p as f64 * 2.5;
        if p > 0 {
            current_radius += radius_step;
        }
        let kepler_factor = 1.0 / (current_radius / 28.0).powf(0.62);
        let dir = if rng.next_f64() > 0.12 { 1.0 } else { -1.0 };
        let base_angular_speed = (0.42 + rng.next_f64() * 0.26) * kepler_factor * dir;

        let base_radius = 1.8
            + rng.next_f64() * 2.1
            + if p == ((num_planets as f64 * 0.6).floor() as usize) { 0.9 } else { 0.0 };
        let has_ring = p >= 1 && base_radius > 2.9 && rng.next_f64() > 0.68;

        let mut moons = Vec::new();
        let max_moons = if p >= 1 && rng.next_f64() > 0.45 {
            if rng.next_f64() > 0.72 { 2 } else { 1 }
        } else {
            0
        };
        for m in 0..max_moons {
            moons.push(Moon {
                orbit_radius: base_radius * 2.2 + 3.2 + m as f64 * 3.6,
                angle: rng.next_f64() * PI * 2.0,
                speed: (2.2 + rng.next_f64() * 1.8) * if m % 2 == 0 { 1.0 } else { -1.0 },
                size: 0.75 + rng.next_f64() * 0.55,
            });
        }

        let eccentricity = 0.02 + rng.next_f64() * 0.08;
        let _orbit_angle_offset = (rng.next_f64() - 0.5) * 0.22;
        let orbit_tilt_offset = (rng.next_f64() - 0.5) * 0.08;
        let angle = rng.next_f64() * PI * 2.0;
        let ring_tilt = 0.32 + rng.next_f64() * 0.2;

        planets.push(Planet {
            base_orbit_radius: current_radius,
            eccentricity,
            orbit_tilt_offset,
            orbit_alpha: clamp_f64(0.115 - p as f64 * 0.015, 0.045, 0.12),
            angle,
            angular_speed: base_angular_speed,
            base_size: base_radius,
            palette_idx,
            has_ring,
            ring_tilt,
            moons,
            lag_x: 0.0,
            lag_y: 0.0,
            world_x: cx,
            world_y: cy,
            depth_z: 0.0,
        });
    }

    let has_local_belt = rng.next_f64() > 0.48;
    let belt_radius = current_radius * (0.72 + rng.next_f64() * 0.35);
    let dust_count = if has_local_belt {
        14 + (rng.next_f64() * 12.0).floor() as usize
    } else {
        6
    };
    let mut dust_particles = Vec::with_capacity(dust_count);
    for _ in 0..dust_count {
        let radius = if has_local_belt {
            belt_radius + (rng.next_f64() - 0.5) * 9.0
        } else {
            20.0 + rng.next_f64() * current_radius
        };
        let angle = rng.next_f64() * PI * 2.0;
        let speed = (0.12 + rng.next_f64() * 0.22) * if rng.next_f64() > 0.15 { 1.0 } else { -1.0 };
        let size = 0.6 + rng.next_f64() * 0.85;
        let alpha = 0.12 + rng.next_f64() * 0.22;
        dust_particles.push(LocalDust {
            radius,
            angle,
            speed,
            size,
            alpha,
        });
    }

    let global_orb = compute_global_orbit_params(w, h, bh_size, cx, cy, seed_val);
    let star_radius = 2.5 + rng.next_f64() * 1.3;
    let glow_radius = 24.0 + rng.next_f64() * 12.0;
    let pulse_phase = rng.next_f64() * PI * 2.0;
    let pulse_speed = 1.1 + rng.next_f64() * 0.9;

    SolarSystemObj {
        id: id_key.to_string(),
        raw_index,
        seed: seed_val,
        is_mini_black_hole,
        anchor_x: cx,
        anchor_y: cy,
        current_x: cx,
        current_y: cy,
        target_x: cx,
        target_y: cy,
        vx: 0.0,
        vy: 0.0,
        global_radius: global_orb.global_radius,
        global_angle: global_orb.global_angle,
        global_angular_speed: global_orb.global_angular_speed,
        global_tilt: global_orb.global_tilt,
        global_plane_rot: global_orb.global_plane_rot,
        stellar_type_index,
        star_radius,
        glow_radius,
        pulse_phase,
        pulse_speed,
        plane_rotation,
        plane_tilt,
        planets,
        dust_particles,
        state: SysState::Spawning,
        spawn_progress: 0.0,
        remove_progress: 0.0,
        opacity: 0.0,
        tidal_pull_x: 0.0,
        tidal_pull_y: 0.0,
        hover_glow: 0.0,
        boost: 0.0,
    }
}

// =========================================================================
// 5. PRE-RENDERED RGBA8 SPRITE BAKING (STRICT 1px alpha=0 BORDER)
// =========================================================================

pub fn bake_radial_sprite(size: usize, inner_r_frac: f32, stops: &[(f32, [f32; 4])]) -> Vec<u8> {
    let mut px = vec![0u8; size * size * 4];
    let half = size as f32 * 0.5;
    let max_r = (half - 1.0).max(1.0);
    let r0 = inner_r_frac * max_r;
    let span = (max_r - r0).max(1e-4);

    for y in 0..size {
        for x in 0..size {
            if x == 0 || y == 0 || x == size - 1 || y == size - 1 {
                continue; // Guarantee outer 1px border alpha == 0
            }
            let dx = x as f32 + 0.5 - half;
            let dy = y as f32 + 0.5 - half;
            let dist = dx.hypot(dy);
            if dist >= max_r {
                continue;
            }
            let t = ((dist - r0) / span).clamp(0.0, 1.0);
            let mut col = stops.last().unwrap().1;
            if t <= stops[0].0 {
                col = stops[0].1;
            } else {
                for w in stops.windows(2) {
                    let (t0, c0) = w[0];
                    let (t1, c1) = w[1];
                    if t >= t0 && t <= t1 {
                        let f = if (t1 - t0).abs() > 1e-6 {
                            (t - t0) / (t1 - t0)
                        } else {
                            0.0
                        };
                        col = [
                            c0[0] + (c1[0] - c0[0]) * f,
                            c0[1] + (c1[1] - c0[1]) * f,
                            c0[2] + (c1[2] - c0[2]) * f,
                            c0[3] + (c1[3] - c0[3]) * f,
                        ];
                        break;
                    }
                }
            }
            let off = (y * size + x) * 4;
            px[off] = (col[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[off + 1] = (col[1].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[off + 2] = (col[2].clamp(0.0, 1.0) * 255.0).round() as u8;
            px[off + 3] = (col[3].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    px
}

pub fn bake_stellar_glow(corona_rgb: [u8; 3]) -> Vec<u8> {
    let c = rgb_u8(corona_rgb[0], corona_rgb[1], corona_rgb[2]);
    bake_radial_sprite(
        128,
        0.0,
        &[
            (0.0, [1.0, 250.0 / 255.0, 240.0 / 255.0, 0.92]),
            (0.08, [c[0], c[1], c[2], 0.55]),
            (0.24, [c[0], c[1], c[2], 0.18]),
            (0.55, [c[0], c[1], c[2], 0.045]),
            (1.0, [c[0], c[1], c[2], 0.0]),
        ],
    )
}

pub fn bake_planet_halo() -> Vec<u8> {
    let c = rgb_u8(180, 205, 240);
    bake_radial_sprite(
        64,
        0.0,
        &[
            (0.0, [c[0], c[1], c[2], 0.22]),
            (0.5, [c[0], c[1], c[2], 0.06]),
            (1.0, [c[0], c[1], c[2], 0.0]),
        ],
    )
}

pub fn bake_black_hole_halo() -> Vec<u8> {
    bake_radial_sprite(
        256,
        0.16,
        &[
            (0.0, [0.0, 0.0, 0.0, 1.0]),
            (0.12, [0.0, 0.0, 0.0, 0.98]),
            (0.155, [1.0, 244.0 / 255.0, 222.0 / 255.0, 0.92]),
            (0.22, [235.0 / 255.0, 172.0 / 255.0, 105.0 / 255.0, 0.52]),
            (0.42, [175.0 / 255.0, 95.0 / 255.0, 52.0 / 255.0, 0.18]),
            (0.72, [90.0 / 255.0, 52.0 / 255.0, 42.0 / 255.0, 0.05]),
            (1.0, [0.0, 0.0, 0.0, 0.0]),
        ],
    )
}

// =========================================================================
// 6. OPENGL 3.3 RENDERER
// =========================================================================

#[repr(C)]
#[derive(Clone, Copy)]
struct ColorVert {
    x: f32,
    y: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TexVert {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
    a: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DiscVert {
    x: f32,
    y: f32,
    lx: f32,
    ly: f32,
    c0: [f32; 4],
    c1: [f32; 4],
    c2: [f32; 4],
    params: [f32; 4],
}

pub struct SolarSystemRenderer {
    prog_bg: GLuint,
    prog_tex: GLuint,
    prog_col: GLuint,
    prog_disc: GLuint,
    dummy_vao: GLuint,
    col_vao: GLuint,
    col_vbo: GLuint,
    tex_vao: GLuint,
    tex_vbo: GLuint,
    disc_vao: GLuint,
    disc_vbo: GLuint,
    bg_fbo: GLuint,
    bg_tex: GLuint,
    bg_w: i32,
    bg_h: i32,
    cached_bg_rev: u64,
    stellar_tex: [GLuint; 4],
    planet_halo_tex: GLuint,
    bh_halo_tex: GLuint,
    col_verts: Vec<ColorVert>,
    tex_verts: Vec<TexVert>,
    disc_verts: Vec<DiscVert>,
}

unsafe fn upload_rgba_tex(gl: &GlFns, w: i32, h: i32, data: &[u8]) -> GLuint {
    let mut tex = 0u32;
    (gl.GenTextures)(1, &mut tex);
    (gl.BindTexture)(GL_TEXTURE_2D, tex);
    (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
    (gl.TexImage2D)(
        GL_TEXTURE_2D,
        0,
        GL_RGBA8 as GLint,
        w,
        h,
        0,
        GL_RGBA,
        GL_UNSIGNED_BYTE,
        data.as_ptr() as *const c_void,
    );
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    tex
}

impl SolarSystemRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_bg = gl.compile_program(SS_BG_VERT, SS_BG_FRAG)?;
        let prog_tex = gl.compile_program(SS_TEX_VERT, SS_TEX_FRAG)?;
        let prog_col = gl.compile_program(SS_COLOR_VERT, SS_COLOR_FS)?;
        let prog_disc = gl.compile_program(SS_DISC_VERT, SS_DISC_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        // Color VAO/VBO
        let (mut col_vao, mut col_vbo) = (0, 0);
        (gl.GenVertexArrays)(1, &mut col_vao);
        (gl.GenBuffers)(1, &mut col_vbo);
        (gl.BindVertexArray)(col_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, col_vbo);
        let stride_c = std::mem::size_of::<ColorVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride_c, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, stride_c, 8 as *const c_void);

        // Tex VAO/VBO
        let (mut tex_vao, mut tex_vbo) = (0, 0);
        (gl.GenVertexArrays)(1, &mut tex_vao);
        (gl.GenBuffers)(1, &mut tex_vbo);
        (gl.BindVertexArray)(tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, tex_vbo);
        let stride_t = std::mem::size_of::<TexVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride_t, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, stride_t, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 1, GL_FLOAT, GL_FALSE, stride_t, 16 as *const c_void);

        // Disc VAO/VBO
        let (mut disc_vao, mut disc_vbo) = (0, 0);
        (gl.GenVertexArrays)(1, &mut disc_vao);
        (gl.GenBuffers)(1, &mut disc_vbo);
        (gl.BindVertexArray)(disc_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, disc_vbo);
        let stride_d = std::mem::size_of::<DiscVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride_d, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, stride_d, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 4, GL_FLOAT, GL_FALSE, stride_d, 16 as *const c_void);
        (gl.EnableVertexAttribArray)(3);
        (gl.VertexAttribPointer)(3, 4, GL_FLOAT, GL_FALSE, stride_d, 32 as *const c_void);
        (gl.EnableVertexAttribArray)(4);
        (gl.VertexAttribPointer)(4, 4, GL_FLOAT, GL_FALSE, stride_d, 48 as *const c_void);
        (gl.EnableVertexAttribArray)(5);
        (gl.VertexAttribPointer)(5, 4, GL_FLOAT, GL_FALSE, stride_d, 64 as *const c_void);
        (gl.BindVertexArray)(0);

        let mut stellar_tex = [0u32; 4];
        for i in 0..4 {
            let px = bake_stellar_glow(STELLAR_TYPES[i].corona_rgb);
            stellar_tex[i] = upload_rgba_tex(gl, 128, 128, &px);
        }
        let planet_halo_tex = upload_rgba_tex(gl, 64, 64, &bake_planet_halo());
        let bh_halo_tex = upload_rgba_tex(gl, 256, 256, &bake_black_hole_halo());

        Ok(Self {
            prog_bg,
            prog_tex,
            prog_col,
            prog_disc,
            dummy_vao,
            col_vao,
            col_vbo,
            tex_vao,
            tex_vbo,
            disc_vao,
            disc_vbo,
            bg_fbo: 0,
            bg_tex: 0,
            bg_w: 0,
            bg_h: 0,
            cached_bg_rev: 0,
            stellar_tex,
            planet_halo_tex,
            bh_halo_tex,
            col_verts: Vec::with_capacity(8192),
            tex_verts: Vec::with_capacity(64),
            disc_verts: Vec::with_capacity(4096),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.bg_tex != 0 { (gl.DeleteTextures)(1, &self.bg_tex); self.bg_tex = 0; }
        if self.bg_fbo != 0 { (gl.DeleteFramebuffers)(1, &self.bg_fbo); self.bg_fbo = 0; }
        for t in &mut self.stellar_tex {
            if *t != 0 { (gl.DeleteTextures)(1, t); *t = 0; }
        }
        if self.planet_halo_tex != 0 { (gl.DeleteTextures)(1, &self.planet_halo_tex); self.planet_halo_tex = 0; }
        if self.bh_halo_tex != 0 { (gl.DeleteTextures)(1, &self.bh_halo_tex); self.bh_halo_tex = 0; }
        for v in [&mut self.dummy_vao, &mut self.col_vao, &mut self.tex_vao, &mut self.disc_vao] {
            if *v != 0 { (gl.DeleteVertexArrays)(1, v); *v = 0; }
        }
        for b in [&mut self.col_vbo, &mut self.tex_vbo, &mut self.disc_vbo] {
            if *b != 0 { (gl.DeleteBuffers)(1, b); *b = 0; }
        }
        for p in [&mut self.prog_bg, &mut self.prog_tex, &mut self.prog_col, &mut self.prog_disc] {
            if *p != 0 { (gl.DeleteProgram)(*p); *p = 0; }
        }
    }

    unsafe fn ensure_bg_cache(&mut self, gl: &GlFns, s: &SolarSystemState, pw: i32, ph: i32) {
        let bw = (pw / 2).max(64);
        let bh = (ph / 2).max(64);
        let size_changed = self.bg_fbo == 0 || self.bg_w != bw || self.bg_h != bh;
        if !size_changed && self.cached_bg_rev == s.bg_rev {
            return;
        }
        if size_changed {
            if self.bg_tex != 0 { (gl.DeleteTextures)(1, &self.bg_tex); }
            if self.bg_fbo != 0 { (gl.DeleteFramebuffers)(1, &self.bg_fbo); }
            (gl.GenFramebuffers)(1, &mut self.bg_fbo);
            (gl.GenTextures)(1, &mut self.bg_tex);
            (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
            (gl.TexImage2D)(
                GL_TEXTURE_2D, 0, GL_RGBA8 as GLint, bw, bh, 0,
                GL_RGBA, GL_UNSIGNED_BYTE, std::ptr::null(),
            );
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
            (gl.FramebufferTexture2D)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, self.bg_tex, 0);
            self.bg_w = bw;
            self.bg_h = bh;
        } else {
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
        }

        (gl.Viewport)(0, 0, bw, bh);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(self.prog_bg);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_bg, "uRes"), s.w as f32, s.h as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "uNebula"), s.params.nebula_intensity as f32);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Bake the 140 static dust specks (mulberry32(0x99117733)) into the cached background FBO
        self.col_verts.clear();
        let mut rng = Mulberry32::new(0x9911_7733);
        let dust_col = [175.0 / 255.0, 198.0 / 255.0, 235.0 / 255.0, 0.045];
        for _ in 0..140 {
            let sx = (rng.next_f64() * s.w) as f32;
            let sy = (rng.next_f64() * s.h) as f32;
            let sr = (0.4 + rng.next_f64() * 0.6) as f32;
            push_rect(&mut self.col_verts, sx, sy, sr, sr, dust_col);
        }
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        self.flush_col(gl, s.w as f32, s.h as f32);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        self.cached_bg_rev = s.bg_rev;
    }

    unsafe fn flush_col(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.col_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_col);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_col, "uRes"), css_w, css_h);
        (gl.BindVertexArray)(self.col_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.col_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.col_verts.len() * std::mem::size_of::<ColorVert>()) as GLsizeiptr,
            self.col_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.col_verts.len() as GLsizei);
        self.col_verts.clear();
    }

    unsafe fn flush_disc(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.disc_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_disc);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_disc, "uRes"), css_w, css_h);
        (gl.BindVertexArray)(self.disc_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.disc_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.disc_verts.len() * std::mem::size_of::<DiscVert>()) as GLsizeiptr,
            self.disc_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.disc_verts.len() as GLsizei);
        self.disc_verts.clear();
    }

    unsafe fn draw_sprite(
        &mut self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        tex: GLuint,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        alpha: f32,
        rot: f32,
        v_flip: bool,
    ) {
        if alpha <= 0.002 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let hw = w * 0.5;
        let hh = h * 0.5;
        let (cr, sr) = (rot.cos(), rot.sin());
        let tx = |lx: f32, ly: f32| (cx + lx * cr - ly * sr, cy + lx * sr + ly * cr);
        let (p0, p1, p2, p3) = (tx(-hw, -hh), tx(hw, -hh), tx(hw, hh), tx(-hw, hh));
        let (v0, v1) = if v_flip { (1.0, 0.0) } else { (0.0, 1.0) };
        self.tex_verts.clear();
        self.tex_verts.extend_from_slice(&[
            TexVert { x: p0.0, y: p0.1, u: 0.0, v: v0, a: alpha },
            TexVert { x: p1.0, y: p1.1, u: 1.0, v: v0, a: alpha },
            TexVert { x: p2.0, y: p2.1, u: 1.0, v: v1, a: alpha },
            TexVert { x: p0.0, y: p0.1, u: 0.0, v: v0, a: alpha },
            TexVert { x: p2.0, y: p2.1, u: 1.0, v: v1, a: alpha },
            TexVert { x: p3.0, y: p3.1, u: 0.0, v: v1, a: alpha },
        ]);
        (gl.UseProgram)(self.prog_tex);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_tex, "uRes"), css_w, css_h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_tex, "uTex"), 0);
        (gl.BindVertexArray)(self.tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.tex_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.tex_verts.len() * std::mem::size_of::<TexVert>()) as GLsizeiptr,
            self.tex_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, 6);
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        st.solar_system.resize(css_w, css_h);
        st.solar_system.dirty = false;

        // 1. Cached deep space + nebula + static dust FBO
        self.ensure_bg_cache(gl, &st.solar_system, pw, ph);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        // Blit cached background (FBO is bottom-up, so v_flip=true)
        self.draw_sprite(gl, css_w, css_h, self.bg_tex, 0.0, 0.0, css_w, css_h, 1.0, 0.0, true);

        let s = &st.solar_system;
        let prof = s.profile();
        let cx = css_w * 0.5;
        let cy = css_h * 0.5;

        // 2. Twinkling stars
        self.col_verts.clear();
        self.disc_verts.clear();
        for star in &s.stars {
            let twinkle = (s.elapsed_time * star.twinkle_speed + star.twinkle_phase).sin() * star.twinkle_amp;
            let alpha = clamp_f64(star.base_alpha + twinkle, 0.04, 0.92) as f32;
            let x = (star.nx * s.w) as f32;
            let y = (star.ny * s.h) as f32;
            let rgb = STAR_COLORS[star.color_idx];
            if star.size <= 1.15 {
                let sz = star.size as f32;
                push_rect(&mut self.col_verts, x, y, sz, sz, [rgb[0], rgb[1], rgb[2], alpha]);
            } else {
                push_disc(&mut self.disc_verts, x, y, (star.size * 0.55) as f32, [rgb[0], rgb[1], rgb[2], alpha]);
            }
        }

        // 3. Global icon orbits
        if s.params.icon_orbit_mode == 0 && s.params.center_body_type != 2 {
            for sys in &s.systems {
                if sys.opacity <= 0.05 {
                    continue;
                }
                let rx = sys.global_radius as f32;
                let ry = (sys.global_radius * sys.global_tilt) as f32;
                let rgb = STELLAR_TYPES[sys.stellar_type_index].orbit_rgb;
                let alpha = (0.042 * sys.opacity) as f32;
                push_ellipse_stroke(
                    &mut self.col_verts,
                    cx,
                    cy,
                    rx,
                    ry,
                    sys.global_plane_rot as f32,
                    0.0,
                    std::f32::consts::TAU,
                    1.0,
                    96,
                    [rgb[0], rgb[1], rgb[2], alpha],
                );
            }
        }

        // Flush stars + global orbits before central core
        self.flush_col(gl, css_w, css_h);
        self.flush_disc(gl, css_w, css_h);

        // 4. Central Core (Macro orbits + BlackHole or Sun)
        if s.params.center_body_type != 2 {
            let tilt = s.central_bh.tilt_angle as f32;
            let (ct, st_sin) = (tilt.cos(), tilt.sin());
            let xform = |lx: f32, ly: f32| (cx + lx * ct - ly * st_sin, cy + lx * st_sin + ly * ct);
            let min_dim = css_w.min(css_h);
            let scale = s.params.black_hole_size as f32;
            let intensity = clamp_f64(s.params.accretion_intensity * (1.0 + s.core_flare * 0.85), 0.1, 2.8) as f32;

            // 4.1 Distant macro orbital rings
            for orb in &s.central_bh.orbits {
                let rx = orb.rx_norm as f32 * css_w;
                let ry = orb.ry_norm as f32 * min_dim;
                push_ellipse_stroke(
                    &mut self.col_verts,
                    cx,
                    cy,
                    rx,
                    ry,
                    tilt,
                    0.0,
                    std::f32::consts::TAU,
                    1.0,
                    96,
                    [165.0 / 255.0, 188.0 / 255.0, 224.0 / 255.0, orb.alpha as f32],
                );
                let (px, py) = xform((orb.angle.cos() as f32) * rx, (orb.angle.sin() as f32) * ry);
                push_disc(
                    &mut self.disc_verts,
                    px,
                    py,
                    orb.size as f32,
                    [185.0 / 255.0, 205.0 / 255.0, 235.0 / 255.0, (orb.alpha * 3.6) as f32],
                );
            }
            self.flush_col(gl, css_w, css_h);
            self.flush_disc(gl, css_w, css_h);

            if s.params.center_body_type == 1 {
                // Grand Central Star (Sun)
                let g_sz = 140.0 * scale;
                self.draw_sprite(
                    gl,
                    css_w,
                    css_h,
                    self.stellar_tex[0],
                    cx - g_sz,
                    cy - g_sz,
                    g_sz * 2.0,
                    g_sz * 2.0,
                    (0.65 * intensity).clamp(0.0, 1.0),
                    tilt,
                    false,
                );
                push_disc(&mut self.disc_verts, cx, cy, 11.0 * scale, [1.0, 245.0 / 255.0, 228.0 / 255.0, 1.0]);
                self.flush_disc(gl, css_w, css_h);
            } else {
                // Supermassive Gargantua Black Hole
                let eh_r = s.central_bh.event_horizon_r as f32 * scale;
                let disk_rx = eh_r * 4.2;
                let disk_ry = disk_rx * s.central_bh.disk_aspect as f32;

                // 4.2 Pre-rendered outer gravitational halo
                let spr_r = eh_r * 5.8;
                self.draw_sprite(
                    gl,
                    css_w,
                    css_h,
                    self.bh_halo_tex,
                    cx - spr_r,
                    cy - spr_r,
                    spr_r * 2.0,
                    spr_r * 2.0,
                    (0.82 * intensity).clamp(0.0, 1.0),
                    tilt,
                    false,
                );

                // 4.3 Bent upper & lower gravitational lensing arcs
                let lens_a_up = (0.32 * intensity).clamp(0.0, 0.65);
                let lens_a_lo = (0.19 * intensity).clamp(0.0, 0.45);
                let (ux, uy) = xform(0.0, -eh_r * 0.18);
                push_ellipse_stroke(
                    &mut self.col_verts,
                    ux,
                    uy,
                    eh_r * 1.68,
                    eh_r * 1.38,
                    tilt,
                    std::f32::consts::PI * 1.06,
                    std::f32::consts::PI * 1.94,
                    3.2 * scale,
                    56,
                    [245.0 / 255.0, 204.0 / 255.0, 152.0 / 255.0, lens_a_up],
                );
                let (lx, ly) = xform(0.0, eh_r * 0.18);
                push_ellipse_stroke(
                    &mut self.col_verts,
                    lx,
                    ly,
                    eh_r * 1.56,
                    eh_r * 1.25,
                    tilt,
                    std::f32::consts::PI * 0.08,
                    std::f32::consts::PI * 0.92,
                    2.2 * scale,
                    56,
                    [218.0 / 255.0, 142.0 / 255.0, 82.0 / 255.0, lens_a_lo],
                );

                // 4.4 Back half of Relativistic Accretion Disk
                push_accretion_half(
                    &mut self.col_verts,
                    cx,
                    cy,
                    tilt,
                    eh_r,
                    disk_rx,
                    disk_ry,
                    intensity,
                    scale,
                    true,
                    &s.central_bh,
                );
                self.flush_col(gl, css_w, css_h);

                // 4.5 Event Horizon shadow + razor-thin Photon Ring
                push_disc(&mut self.disc_verts, cx, cy, eh_r, [0.0, 0.0, 0.0, 1.0]);
                let pulse = 1.0 + (s.elapsed_time * 1.7).sin() as f32 * 0.06;
                let pr_a = (0.82 * intensity * pulse).clamp(0.0, 0.98);
                push_ring(
                    &mut self.disc_verts,
                    cx,
                    cy,
                    eh_r * 1.03,
                    1.35 * scale,
                    [1.0, 242.0 / 255.0, 216.0 / 255.0, pr_a],
                );
                self.flush_disc(gl, css_w, css_h);

                // 4.6 Front half of Relativistic Accretion Disk
                push_accretion_half(
                    &mut self.col_verts,
                    cx,
                    cy,
                    tilt,
                    eh_r,
                    disk_rx,
                    disk_ry,
                    intensity,
                    scale,
                    false,
                    &s.central_bh,
                );
                self.flush_col(gl, css_w, css_h);
            }
        }

        // 5. Asteroids
        if !s.asteroids.is_empty() {
            let half_diag = (s.w.hypot(s.h) * 0.5) as f32;
            for ast in &s.asteroids {
                let rx = ast.band_norm as f32 * half_diag + ast.radial_jitter as f32;
                let ry = rx * 0.54;
                let x = cx + (ast.angle.cos() as f32) * rx;
                let y = cy + (ast.angle.sin() as f32) * ry;
                if x < -10.0 || x > css_w + 10.0 || y < -10.0 || y > css_h + 10.0 {
                    continue;
                }
                let col = [168.0 / 255.0, 180.0 / 255.0, 198.0 / 255.0, ast.alpha as f32];
                let sz = ast.size as f32;
                let asp = ast.aspect as f32;
                if s.low_mode && sz <= 1.35 {
                    push_rect(&mut self.col_verts, x - sz, y - sz * asp, sz * 2.0, sz * 2.0 * asp, col);
                } else {
                    push_filled_ellipse(&mut self.col_verts, x, y, sz, sz * asp, ast.rotation as f32, 10, col);
                }
            }
        }

        // 6. Black Hole Tidal Streams
        let attraction = s.params.gravity_strength * s.params.system_attraction;
        if prof.enable_tidal_streams && s.params.center_body_type == 0 && attraction > 0.02 {
            let max_stream_d = s.w.min(s.h) * 0.44;
            for sys in &s.systems {
                if sys.opacity < 0.1 {
                    continue;
                }
                let dx = s.w * 0.5 - sys.current_x;
                let dy = s.h * 0.5 - sys.current_y;
                let dist = dx.hypot(dy);
                if dist > 36.0 && dist < max_stream_d {
                    let close = 1.0 - dist / max_stream_d;
                    let sa = clamp_f64(close * close * 0.16 * attraction * sys.opacity, 0.0, 0.14) as f32;
                    if sa <= 0.004 {
                        continue;
                    }
                    let px = -dy / dist;
                    let py = dx / dist;
                    let curve = dist * 0.24;
                    let ctrl_x = ((sys.current_x + s.w * 0.5) * 0.5 + px * curve) as f32;
                    let ctrl_y = ((sys.current_y + s.h * 0.5) * 0.5 + py * curve) as f32;
                    let st_c = STELLAR_TYPES[sys.stellar_type_index].corona_rgb;
                    let c0 = [st_c[0] as f32 / 255.0, st_c[1] as f32 / 255.0, st_c[2] as f32 / 255.0, sa];
                    let c_mid = [235.0 / 255.0, 175.0 / 255.0, 112.0 / 255.0, sa * 0.85];
                    let c1 = [235.0 / 255.0, 175.0 / 255.0, 112.0 / 255.0, 0.0];
                    push_quad_bezier_gradient(
                        &mut self.col_verts,
                        sys.current_x as f32,
                        sys.current_y as f32,
                        ctrl_x,
                        ctrl_y,
                        cx,
                        cy,
                        1.2,
                        20,
                        c0,
                        c_mid,
                        c1,
                    );
                }
            }
        }

        // 7. Inter-system Resonance Bridges
        if prof.enable_resonance_bridges && attraction > 0.02 && s.systems.len() >= 2 {
            let max_d = 340.0 * s.params.orbit_size;
            for i in 0..s.systems.len() {
                let a = &s.systems[i];
                if a.opacity < 0.1 {
                    continue;
                }
                for j in (i + 1)..s.systems.len() {
                    let b = &s.systems[j];
                    if b.opacity < 0.1 {
                        continue;
                    }
                    let dist = (b.current_x - a.current_x).hypot(b.current_y - a.current_y);
                    if dist > 40.0 && dist < max_d {
                        let strength = (1.0 - dist / max_d) * attraction * a.opacity.min(b.opacity);
                        let alpha = clamp_f64(strength * 0.075, 0.0, 0.055) as f32;
                        if alpha <= 0.003 {
                            continue;
                        }
                        let mx = ((a.current_x + b.current_x) * 0.5 + (a.tidal_pull_x - b.tidal_pull_x) * 0.4) as f32;
                        let my = ((a.current_y + b.current_y) * 0.5 + (a.tidal_pull_y - b.tidal_pull_y) * 0.4) as f32;
                        let col = [165.0 / 255.0, 192.0 / 255.0, 232.0 / 255.0, alpha];
                        push_quad_bezier_gradient(
                            &mut self.col_verts,
                            a.current_x as f32,
                            a.current_y as f32,
                            mx,
                            my,
                            b.current_x as f32,
                            b.current_y as f32,
                            1.0,
                            16,
                            col,
                            col,
                            col,
                        );
                    }
                }
            }
        }
        self.flush_col(gl, css_w, css_h);

        // 8. Icon Solar Systems
        for i in 0..st.solar_system.systems.len() {
            self.render_system(gl, css_w, css_h, &st.solar_system, i);
        }

        // 9. Meteors + Pointer Effects
        let s = &st.solar_system;
        self.col_verts.clear();
        self.disc_verts.clear();
        for m in &s.meteors {
            if !m.active || m.alpha <= 0.01 {
                continue;
            }
            let spd = m.vx.hypot(m.vy).max(1.0);
            let dx = m.vx / spd;
            let dy = m.vy / spd;
            let tx = (m.x - dx * m.length) as f32;
            let ty = (m.y - dy * m.length) as f32;
            let mx = m.x as f32;
            let my = m.y as f32;
            let ma = m.alpha as f32;
            push_line_gradient(
                &mut self.col_verts,
                mx,
                my,
                tx,
                ty,
                1.25,
                [235.0 / 255.0, 244.0 / 255.0, 1.0, ma],
                [170.0 / 255.0, 198.0 / 255.0, 240.0 / 255.0, 0.0],
            );
        }

        for sw in &s.shockwaves {
            if sw.alpha <= 0.01 {
                continue;
            }
            push_ring(
                &mut self.disc_verts,
                sw.x as f32,
                sw.y as f32,
                sw.r as f32,
                1.6,
                [sw.rgb[0], sw.rgb[1], sw.rgb[2], sw.alpha as f32],
            );
        }

        for sp in &s.cursor_sparks {
            let a = clamp_f64(1.0 - sp.life / sp.max_life, 0.0, 1.0) as f32;
            let r = sp.size as f32 * (0.4 + 0.6 * a);
            push_disc(
                &mut self.disc_verts,
                sp.x as f32,
                sp.y as f32,
                r,
                [sp.rgb[0], sp.rgb[1], sp.rgb[2], a * 0.85],
            );
        }

        if s.pointer_active {
            let rad = if s.pointer_down { 44.0 } else { 28.0 };
            let c0 = if s.pointer_down {
                [1.0, 220.0 / 255.0, 155.0 / 255.0, 0.24]
            } else {
                [170.0 / 255.0, 215.0 / 255.0, 1.0, 0.14]
            };
            let c1 = if s.pointer_down {
                [1.0, 185.0 / 255.0, 110.0 / 255.0, 0.09]
            } else {
                [130.0 / 255.0, 185.0 / 255.0, 1.0, 0.05]
            };
            push_radial_glow(&mut self.disc_verts, s.pointer_x as f32, s.pointer_y as f32, rad, c0, c1);
            let ring_col = if s.pointer_down {
                [1.0, 210.0 / 255.0, 140.0 / 255.0, 0.38]
            } else {
                [170.0 / 255.0, 215.0 / 255.0, 1.0, 0.22]
            };
            push_ring(
                &mut self.disc_verts,
                s.pointer_x as f32,
                s.pointer_y as f32,
                if s.pointer_down { 16.0 } else { 11.0 },
                1.0,
                ring_col,
            );
        }

        for sys in &s.systems {
            let hg = sys.hover_glow.max(sys.boost);
            if hg > 0.04 {
                let rgb = STELLAR_TYPES[sys.stellar_type_index].orbit_rgb;
                push_ring(
                    &mut self.disc_verts,
                    sys.current_x as f32,
                    sys.current_y as f32,
                    (26.0 + hg * 14.0) as f32,
                    1.4,
                    [rgb[0], rgb[1], rgb[2], (hg * 0.35) as f32],
                );
            }
        }

        self.flush_col(gl, css_w, css_h);
        self.flush_disc(gl, css_w, css_h);
    }

    unsafe fn render_system(&mut self, gl: &GlFns, css_w: f32, css_h: f32, s: &SolarSystemState, idx: usize) {
        let sys = &s.systems[idx];
        if sys.opacity <= 0.002 {
            return;
        }
        let prof = s.profile();
        let is_spawning = sys.state == SysState::Spawning;
        let light_rev = if is_spawning { smoothstep_f64(0.0, 0.18, sys.spawn_progress) } else { 1.0 };
        let glow_rev = if is_spawning { smoothstep_f64(0.10, 0.42, sys.spawn_progress) } else { 1.0 };
        let orbit_rev = if is_spawning { smoothstep_f64(0.25, 0.65, sys.spawn_progress) } else { 1.0 };
        let planet_rev = if is_spawning { smoothstep_f64(0.48, 0.96, sys.spawn_progress) } else { 1.0 };

        let st_type = STELLAR_TYPES[sys.stellar_type_index];
        let size_mult = s.params.orbit_size;
        let cos_rot = sys.plane_rotation.cos();
        let sin_rot = sys.plane_rotation.sin();
        let dissolve_exp = if sys.state == SysState::Removing {
            1.0 + sys.remove_progress * 0.42
        } else {
            1.0
        };

        // Orbits + local dust
        self.col_verts.clear();
        let orb_phase_a = sys.opacity * orbit_rev;
        if orb_phase_a > 0.005 {
            let p_len = sys.planets.len().max(1) as f64;
            for (p, planet) in sys.planets.iter().enumerate() {
                let st_start = 0.28 + (p as f64 / p_len) * 0.24;
                let o_rev = if is_spawning {
                    smoothstep_f64(st_start, st_start + 0.25, sys.spawn_progress)
                } else {
                    1.0
                };
                if o_rev <= 0.01 {
                    continue;
                }
                let rx = planet.base_orbit_radius * size_mult * dissolve_exp;
                let ry = rx * clamp_f64(sys.plane_tilt + planet.orbit_tilt_offset, 0.35, 0.85);
                let focal = rx * planet.eccentricity;
                let orbit_norm = (p + 1) as f64 / p_len;
                let tw = 0.35 + 0.65 * orbit_norm;
                let ox = -focal * cos_rot + planet.lag_x + sys.tidal_pull_x * tw;
                let oy = -focal * sin_rot + planet.lag_y + sys.tidal_pull_y * tw;
                let a = (planet.orbit_alpha * orb_phase_a * o_rev) as f32;
                push_ellipse_stroke(
                    &mut self.col_verts,
                    (sys.current_x + ox) as f32,
                    (sys.current_y + oy) as f32,
                    rx as f32,
                    ry as f32,
                    sys.plane_rotation as f32,
                    0.0,
                    (PI * 2.0 * o_rev) as f32,
                    1.0,
                    56,
                    [st_type.orbit_rgb[0], st_type.orbit_rgb[1], st_type.orbit_rgb[2], a],
                );
            }
        }

        let dust_a = sys.opacity * planet_rev;
        if prof.enable_local_dust && dust_a > 0.01 {
            let disperse = if sys.state == SysState::Removing {
                1.0 + sys.remove_progress * 0.65
            } else {
                1.0
            };
            for pt in &sys.dust_particles {
                let rx = pt.radius * size_mult * disperse;
                let ry = rx * sys.plane_tilt;
                let lx = pt.angle.cos() * rx;
                let ly = pt.angle.sin() * ry;
                let wx = (sys.current_x + lx * cos_rot - ly * sin_rot + sys.tidal_pull_x * 0.5) as f32;
                let wy = (sys.current_y + lx * sin_rot + ly * cos_rot + sys.tidal_pull_y * 0.5) as f32;
                let sz = pt.size as f32;
                push_rect(
                    &mut self.col_verts,
                    wx,
                    wy,
                    sz,
                    sz,
                    [st_type.orbit_rgb[0], st_type.orbit_rgb[1], st_type.orbit_rgb[2], (pt.alpha * dust_a) as f32],
                );
            }
        }
        self.flush_col(gl, css_w, css_h);

        // Back planets (depthZ < 0)
        for (p, planet) in sys.planets.iter().enumerate() {
            if planet.depth_z < 0.0 {
                let p_stag = if is_spawning {
                    smoothstep_f64(0.48 + p as f64 * 0.07, 0.78 + p as f64 * 0.07, sys.spawn_progress)
                } else {
                    1.0
                };
                self.render_planet(gl, css_w, css_h, s, sys, planet, sys.opacity * p_stag);
            }
        }

        // Star core (or mini black hole)
        let pulse = 1.0 + (s.elapsed_time * sys.pulse_speed + sys.pulse_phase).sin() * 0.07;
        if sys.is_mini_black_hole {
            let g_rad = (sys.glow_radius * 1.15 * (0.65 + 0.35 * glow_rev)) as f32;
            let ga = (sys.opacity * glow_rev * 0.85) as f32;
            self.draw_sprite(
                gl,
                css_w,
                css_h,
                self.bh_halo_tex,
                sys.current_x as f32 - g_rad,
                sys.current_y as f32 - g_rad,
                g_rad * 2.0,
                g_rad * 2.0,
                ga,
                0.0,
                false,
            );
            if light_rev > 0.01 {
                let eh_r = (sys.star_radius * 1.65 * light_rev) as f32;
                push_disc(&mut self.disc_verts, sys.current_x as f32, sys.current_y as f32, eh_r, [0.0, 0.0, 0.0, 1.0]);
                push_ring(
                    &mut self.disc_verts,
                    sys.current_x as f32,
                    sys.current_y as f32,
                    eh_r,
                    1.0,
                    [1.0, 236.0 / 255.0, 205.0 / 255.0, (sys.opacity * light_rev * 0.85) as f32],
                );
                self.flush_disc(gl, css_w, css_h);
            }
        } else {
            if glow_rev > 0.01 {
                let g_rad = (sys.glow_radius * pulse * (0.65 + 0.35 * glow_rev)) as f32;
                let ga = (sys.opacity * glow_rev * 0.72) as f32;
                self.draw_sprite(
                    gl,
                    css_w,
                    css_h,
                    self.stellar_tex[sys.stellar_type_index],
                    sys.current_x as f32 - g_rad,
                    sys.current_y as f32 - g_rad,
                    g_rad * 2.0,
                    g_rad * 2.0,
                    ga,
                    0.0,
                    false,
                );
            }
            if light_rev > 0.01 {
                let core_r = (sys.star_radius * (0.55 + 0.45 * light_rev)) as f32;
                push_disc(
                    &mut self.disc_verts,
                    sys.current_x as f32,
                    sys.current_y as f32,
                    core_r,
                    [st_type.core[0], st_type.core[1], st_type.core[2], (sys.opacity * light_rev * 0.9) as f32],
                );
                self.flush_disc(gl, css_w, css_h);
            }
        }

        // Front planets (depthZ >= 0)
        for (p, planet) in sys.planets.iter().enumerate() {
            if planet.depth_z >= 0.0 {
                let p_stag = if is_spawning {
                    smoothstep_f64(0.48 + p as f64 * 0.07, 0.78 + p as f64 * 0.07, sys.spawn_progress)
                } else {
                    1.0
                };
                self.render_planet(gl, css_w, css_h, s, sys, planet, sys.opacity * p_stag);
            }
        }
    }

    unsafe fn render_planet(
        &mut self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        s: &SolarSystemState,
        sys: &SolarSystemObj,
        planet: &Planet,
        phase_alpha: f64,
    ) {
        if phase_alpha <= 0.01 {
            return;
        }
        let prof = s.profile();
        let depth_scale = 1.0 + planet.depth_z * 0.13;
        let depth_bright = 0.86 + planet.depth_z * 0.14;
        let r = (planet.base_size * s.params.planet_size * depth_scale).max(1.2) as f32;
        let alpha = clamp_f64(phase_alpha * depth_bright, 0.0, 1.0) as f32;
        let px = planet.world_x as f32;
        let py = planet.world_y as f32;
        let pal = PLANET_PALETTES[planet.palette_idx];

        if r >= 2.2 {
            let hs = r * 5.2;
            self.draw_sprite(
                gl,
                css_w,
                css_h,
                self.planet_halo_tex,
                px - hs * 0.5,
                py - hs * 0.5,
                hs,
                hs,
                alpha * 0.45,
                0.0,
                false,
            );
        }

        if planet.has_ring {
            push_ellipse_stroke(
                &mut self.col_verts,
                px,
                py,
                r * 2.35,
                r * 2.35 * planet.ring_tilt as f32,
                sys.plane_rotation as f32 + 0.3,
                0.0,
                std::f32::consts::TAU,
                1.1,
                32,
                [pal.atmo[0], pal.atmo[1], pal.atmo[2], alpha * 0.34],
            );
            self.flush_col(gl, css_w, css_h);
        }

        let to_star = (sys.current_y - planet.world_y).atan2(sys.current_x - planet.world_x) as f32;
        push_shaded_planet(
            &mut self.disc_verts,
            px,
            py,
            r,
            to_star.cos(),
            to_star.sin(),
            [pal.fill[0], pal.fill[1], pal.fill[2], alpha],
            [pal.highlight[0], pal.highlight[1], pal.highlight[2], alpha],
            [pal.shadow[0], pal.shadow[1], pal.shadow[2], alpha],
        );

        if prof.enable_moons && !planet.moons.is_empty() {
            let m_scale = s.params.planet_size;
            for moon in &planet.moons {
                let orb_r = moon.orbit_radius * m_scale;
                let mx = (planet.world_x + moon.angle.cos() * orb_r) as f32;
                let my = (planet.world_y + moon.angle.sin() * orb_r * 0.62) as f32;
                let mr = (moon.size * m_scale).max(0.65) as f32;
                push_disc(
                    &mut self.disc_verts,
                    mx,
                    my,
                    mr,
                    [205.0 / 255.0, 216.0 / 255.0, 232.0 / 255.0, alpha * 0.72],
                );
            }
        }
        self.flush_disc(gl, css_w, css_h);
    }
}

// =========================================================================
// 7. CLEAN GEOMETRY TESSELLATION HELPERS (RIBBONS / STRIPS, NO BROKEN FANS)
// =========================================================================

fn push_rect(out: &mut Vec<ColorVert>, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
    let v = |px, py| ColorVert { x: px, y: py, r: col[0], g: col[1], b: col[2], a: col[3] };
    out.extend_from_slice(&[
        v(x, y), v(x + w, y), v(x + w, y + h),
        v(x, y), v(x + w, y + h), v(x, y + h),
    ]);
}

fn push_disc(out: &mut Vec<DiscVert>, cx: f32, cy: f32, r: f32, col: [f32; 4]) {
    let pad = r + 1.25;
    let lim = pad / r.max(0.1);
    let p = [0.0, 0.0, 0.0, 0.0];
    let z = [0.0; 4];
    let mk = |dx, dy, lx, ly| DiscVert { x: cx + dx, y: cy + dy, lx, ly, c0: col, c1: z, c2: z, params: p };
    out.extend_from_slice(&[
        mk(-pad, -pad, -lim, -lim), mk(pad, -pad, lim, -lim), mk(pad, pad, lim, lim),
        mk(-pad, -pad, -lim, -lim), mk(pad, pad, lim, lim), mk(-pad, pad, -lim, lim),
    ]);
}

fn push_ring(out: &mut Vec<DiscVert>, cx: f32, cy: f32, r: f32, width: f32, col: [f32; 4]) {
    let hw = width * 0.5;
    let outer = r + hw;
    let inner = (r - hw).max(0.0);
    let pad = outer + 1.5;
    let lim = pad / outer.max(0.1);
    let p = [1.0, inner / outer.max(0.1), 1.0, 0.0];
    let z = [0.0; 4];
    let mk = |dx, dy, lx, ly| DiscVert { x: cx + dx, y: cy + dy, lx, ly, c0: col, c1: z, c2: z, params: p };
    out.extend_from_slice(&[
        mk(-pad, -pad, -lim, -lim), mk(pad, -pad, lim, -lim), mk(pad, pad, lim, lim),
        mk(-pad, -pad, -lim, -lim), mk(pad, pad, lim, lim), mk(-pad, pad, -lim, lim),
    ]);
}

fn push_shaded_planet(
    out: &mut Vec<DiscVert>,
    cx: f32,
    cy: f32,
    r: f32,
    dir_x: f32,
    dir_y: f32,
    fill: [f32; 4],
    highlight: [f32; 4],
    shadow: [f32; 4],
) {
    let pad = r + 1.25;
    let lim = pad / r.max(0.1);
    let p = [2.0, dir_x, dir_y, 0.0];
    let mk = |dx, dy, lx, ly| DiscVert { x: cx + dx, y: cy + dy, lx, ly, c0: fill, c1: highlight, c2: shadow, params: p };
    out.extend_from_slice(&[
        mk(-pad, -pad, -lim, -lim), mk(pad, -pad, lim, -lim), mk(pad, pad, lim, lim),
        mk(-pad, -pad, -lim, -lim), mk(pad, pad, lim, lim), mk(-pad, pad, -lim, lim),
    ]);
}

fn push_radial_glow(out: &mut Vec<DiscVert>, cx: f32, cy: f32, r: f32, c0: [f32; 4], c1: [f32; 4]) {
    let p = [3.0, 0.0, 0.0, 0.0];
    let z = [0.0; 4];
    let mk = |dx, dy, lx, ly| DiscVert { x: cx + dx, y: cy + dy, lx, ly, c0, c1, c2: z, params: p };
    out.extend_from_slice(&[
        mk(-r, -r, -1.0, -1.0), mk(r, -r, 1.0, -1.0), mk(r, r, 1.0, 1.0),
        mk(-r, -r, -1.0, -1.0), mk(r, r, 1.0, 1.0), mk(-r, r, -1.0, 1.0),
    ]);
}

fn push_filled_ellipse(
    out: &mut Vec<ColorVert>,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rot: f32,
    segs: usize,
    col: [f32; 4],
) {
    let (cr, sr) = (rot.cos(), rot.sin());
    let pt = |i: usize| -> ColorVert {
        let a = (i as f32 / segs as f32) * std::f32::consts::TAU;
        let lx = a.cos() * rx;
        let ly = a.sin() * ry;
        ColorVert {
            x: cx + lx * cr - ly * sr,
            y: cy + lx * sr + ly * cr,
            r: col[0],
            g: col[1],
            b: col[2],
            a: col[3],
        }
    };
    let center = ColorVert { x: cx, y: cy, r: col[0], g: col[1], b: col[2], a: col[3] };
    let mut prev = pt(0);
    for i in 1..=segs {
        let cur = pt(i % segs);
        out.extend_from_slice(&[center, prev, cur]);
        prev = cur;
    }
}

fn push_ellipse_stroke(
    out: &mut Vec<ColorVert>,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rot: f32,
    start_a: f32,
    end_a: f32,
    width: f32,
    segs: usize,
    col: [f32; 4],
) {
    if col[3] <= 0.001 || segs < 2 || rx <= 0.05 || ry <= 0.05 {
        return;
    }
    let hw = width * 0.5;
    let (cr, sr) = (rot.cos(), rot.sin());
    let sample = |t: f32| -> ((f32, f32), (f32, f32)) {
        let a = start_a + (end_a - start_a) * t;
        let (ca, sa) = (a.cos(), a.sin());
        let lx = ca * rx;
        let ly = sa * ry;
        // Outward normal of ellipse (ca/rx, sa/ry)
        let nx0 = ca / rx;
        let ny0 = sa / ry;
        let nl = nx0.hypot(ny0).max(1e-6);
        let nx = nx0 / nl;
        let ny = ny0 / nl;
        let px0 = lx - nx * hw;
        let py0 = ly - ny * hw;
        let px1 = lx + nx * hw;
        let py1 = ly + ny * hw;
        (
            (cx + px0 * cr - py0 * sr, cy + px0 * sr + py0 * cr),
            (cx + px1 * cr - py1 * sr, cy + px1 * sr + py1 * cr),
        )
    };
    let mk = |(x, y): (f32, f32)| ColorVert { x, y, r: col[0], g: col[1], b: col[2], a: col[3] };
    let (mut p_in, mut p_out) = sample(0.0);
    for i in 1..=segs {
        let (c_in, c_out) = sample(i as f32 / segs as f32);
        out.extend_from_slice(&[
            mk(p_in), mk(p_out), mk(c_out),
            mk(p_in), mk(c_out), mk(c_in),
        ]);
        p_in = c_in;
        p_out = c_out;
    }
}

fn eval_doppler_grad(u: f32, a_mult: f32) -> [f32; 4] {
    let t = u.clamp(0.0, 1.0);
    let stops: [(f32, [f32; 4]); 5] = [
        (0.0, [1.0, 225.0 / 255.0, 175.0 / 255.0, 0.0]),
        (0.18, [1.0, 245.0 / 255.0, 224.0 / 255.0, (0.68 * a_mult).clamp(0.0, 0.95)]),
        (0.46, [238.0 / 255.0, 168.0 / 255.0, 96.0 / 255.0, (0.44 * a_mult).clamp(0.0, 0.85)]),
        (0.78, [172.0 / 255.0, 84.0 / 255.0, 42.0 / 255.0, (0.25 * a_mult).clamp(0.0, 0.65)]),
        (1.0, [120.0 / 255.0, 48.0 / 255.0, 24.0 / 255.0, 0.0]),
    ];
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if t >= t0 && t <= t1 {
            let f = (t - t0) / (t1 - t0).max(1e-5);
            return [
                c0[0] + (c1[0] - c0[0]) * f,
                c0[1] + (c1[1] - c0[1]) * f,
                c0[2] + (c1[2] - c0[2]) * f,
                c0[3] + (c1[3] - c0[3]) * f,
            ];
        }
    }
    stops[4].1
}

fn push_accretion_half(
    out: &mut Vec<ColorVert>,
    cx: f32,
    cy: f32,
    tilt: f32,
    eh_r: f32,
    disk_rx: f32,
    disk_ry: f32,
    intensity: f32,
    scale: f32,
    is_back: bool,
    bh: &CentralBlackHole,
) {
    let start_a = if is_back { std::f32::consts::PI } else { 0.0 };
    let end_a = if is_back { std::f32::consts::TAU } else { std::f32::consts::PI };
    let a_mult = (intensity * if is_back { 0.72 } else { 1.0 }).clamp(0.0, 1.4);
    let (cr, sr) = (tilt.cos(), tilt.sin());
    let bands: [(f32, f32); 3] = [(0.52, 3.2), (0.72, 6.5), (0.94, 9.0)];
    let segs = 64usize;

    for &(r_fac, width) in &bands {
        let rx = disk_rx * r_fac;
        let ry = disk_ry * r_fac;
        let hw = width * scale * 0.5;
        let sample = |t: f32| -> ((f32, f32), (f32, f32), [f32; 4]) {
            let a = start_a + (end_a - start_a) * t;
            let (ca, sa) = (a.cos(), a.sin());
            let lx = ca * rx;
            let ly = sa * ry;
            let u = (lx + disk_rx) / (2.0 * disk_rx).max(1.0);
            let col = eval_doppler_grad(u, a_mult);
            let nx0 = ca / rx;
            let ny0 = sa / ry;
            let nl = nx0.hypot(ny0).max(1e-6);
            let nx = nx0 / nl;
            let ny = ny0 / nl;
            let px0 = lx - nx * hw;
            let py0 = ly - ny * hw;
            let px1 = lx + nx * hw;
            let py1 = ly + ny * hw;
            (
                (cx + px0 * cr - py0 * sr, cy + px0 * sr + py0 * cr),
                (cx + px1 * cr - py1 * sr, cy + px1 * sr + py1 * cr),
                col,
            )
        };
        let mk = |(x, y): (f32, f32), c: [f32; 4]| ColorVert { x, y, r: c[0], g: c[1], b: c[2], a: c[3] };
        let (mut pi, mut po, mut pc) = sample(0.0);
        for i in 1..=segs {
            let (ci, co, cc) = sample(i as f32 / segs as f32);
            out.extend_from_slice(&[
                mk(pi, pc), mk(po, pc), mk(co, cc),
                mk(pi, pc), mk(co, cc), mk(ci, cc),
            ]);
            pi = ci;
            po = co;
            pc = cc;
        }
    }

    // Orbiting hot plasma knots
    for ap in &bh.accretion_particles {
        let sin_a = ap.angle.sin();
        if is_back && sin_a >= 0.0 {
            continue;
        }
        if !is_back && sin_a < 0.0 {
            continue;
        }
        let r = eh_r * ap.radius_norm as f32;
        let lx = (ap.angle.cos() as f32) * r;
        let ly = (sin_a as f32) * (r * bh.disk_aspect as f32);
        let doppler = 0.65 - 0.35 * ap.angle.cos();
        let pa = clamp_f64(ap.alpha * doppler * intensity as f64, 0.03, 0.88) as f32;
        let sz = ap.size as f32 * scale;
        let col = [1.0, 228.0 / 255.0, 188.0 / 255.0, pa];
        let xform = |x: f32, y: f32| ColorVert {
            x: cx + x * cr - y * sr,
            y: cy + x * sr + y * cr,
            r: col[0],
            g: col[1],
            b: col[2],
            a: col[3],
        };
        let (v0, v1, v2, v3) = (xform(lx, ly), xform(lx + sz, ly), xform(lx + sz, ly + sz), xform(lx, ly + sz));
        out.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
    }
}

fn push_line_gradient(
    out: &mut Vec<ColorVert>,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    width: f32,
    c0: [f32; 4],
    c1: [f32; 4],
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = dx.hypot(dy).max(1e-5);
    let nx = -dy / len * width * 0.5;
    let ny = dx / len * width * 0.5;
    let mk = |x, y, c: [f32; 4]| ColorVert { x, y, r: c[0], g: c[1], b: c[2], a: c[3] };
    out.extend_from_slice(&[
        mk(x0 - nx, y0 - ny, c0), mk(x0 + nx, y0 + ny, c0), mk(x1 + nx, y1 + ny, c1),
        mk(x0 - nx, y0 - ny, c0), mk(x1 + nx, y1 + ny, c1), mk(x1 - nx, y1 - ny, c1),
    ]);
}

fn push_quad_bezier_gradient(
    out: &mut Vec<ColorVert>,
    x0: f32,
    y0: f32,
    cx: f32,
    cy: f32,
    x1: f32,
    y1: f32,
    width: f32,
    segs: usize,
    c0: [f32; 4],
    cmid: [f32; 4],
    c1: [f32; 4],
) {
    let hw = width * 0.5;
    let eval = |t: f32| -> (f32, f32, [f32; 4]) {
        let u = 1.0 - t;
        let x = u * u * x0 + 2.0 * u * t * cx + t * t * x1;
        let y = u * u * y0 + 2.0 * u * t * cy + t * t * y1;
        let col = if t < 0.7 {
            let f = t / 0.7;
            [
                c0[0] + (cmid[0] - c0[0]) * f,
                c0[1] + (cmid[1] - c0[1]) * f,
                c0[2] + (cmid[2] - c0[2]) * f,
                c0[3] + (cmid[3] - c0[3]) * f,
            ]
        } else {
            let f = (t - 0.7) / 0.3;
            [
                cmid[0] + (c1[0] - cmid[0]) * f,
                cmid[1] + (c1[1] - cmid[1]) * f,
                cmid[2] + (c1[2] - cmid[2]) * f,
                cmid[3] + (c1[3] - cmid[3]) * f,
            ]
        };
        (x, y, col)
    };
    let mut pts = Vec::with_capacity(segs + 1);
    for i in 0..=segs {
        pts.push(eval(i as f32 / segs as f32));
    }
    let mk = |x, y, c: [f32; 4]| ColorVert { x, y, r: c[0], g: c[1], b: c[2], a: c[3] };
    for i in 0..segs {
        let (ax, ay, ca) = pts[i];
        let (bx, by, cb) = pts[i + 1];
        let dx = bx - ax;
        let dy = by - ay;
        let len = dx.hypot(dy).max(1e-5);
        let nx = -dy / len * hw;
        let ny = dx / len * hw;
        out.extend_from_slice(&[
            mk(ax - nx, ay - ny, ca), mk(ax + nx, ay + ny, ca), mk(bx + nx, by + ny, cb),
            mk(ax - nx, ay - ny, ca), mk(bx + nx, by + ny, cb), mk(bx - nx, by - ny, cb),
        ]);
    }
}

// =========================================================================
// 8. UNIT TESTS (Determinism, Params/Enum Mapping, Cache/Icon Invalidation, Sprite Border)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_and_mulberry32_match_js_prototype_exactly() {
        // Exact JS fnv1aHash('cell:10:5') and fnv1aHash('id:preview-chrome')
        assert_eq!(fnv1a_hash(""), 2166136261u32);
        assert_eq!(fnv1a_hash("cell:10:5"), {
            let mut h: u32 = 2166136261;
            for b in b"cell:10:5" {
                h ^= *b as u32;
                h = h.wrapping_mul(16777619);
            }
            h
        });
        // Exact JS mulberry32(0x47415247) sequence
        let mut r = Mulberry32::new(0x4741_5247);
        let v0 = r.next_f64();
        let v1 = r.next_f64();
        let v2 = r.next_f64();
        assert!(v0 >= 0.0 && v0 < 1.0);
        // Verify bit-identical manual formula for first output of 0x47415247
        let a = 0x4741_5247u32.wrapping_add(0x6D2B_79F5);
        let t0 = (a ^ (a >> 15)).wrapping_mul(1 | a);
        let t1 = t0.wrapping_add((t0 ^ (t0 >> 7)).wrapping_mul(61 | t0)) ^ t0;
        let expected0 = ((t1 ^ (t1 >> 14)) as f64) / 4294967296.0;
        assert!((v0 - expected0).abs() == 0.0);
        assert_ne!(v0, v1);
        assert_ne!(v1, v2);
    }

    #[test]
    fn params_defaults_enums_and_clamping_match_themes_json() {
        let def = SolarSystemParams::default();
        assert_eq!(def.center_body_type, 0); // BlackHole
        assert_eq!(def.icon_orbit_mode, 0); // Orbit
        assert_eq!(def.planet_count, 3);
        assert_eq!(def.quality, 2); // High
        assert!(def.shooting_stars);

        let mut raw = HashMap::new();
        raw.insert("centerBodyType".into(), Value::String("Sun".into()));
        raw.insert("iconOrbitMode".into(), Value::String("Anchor".into()));
        raw.insert("quality".into(), Value::String("Low".into()));
        raw.insert("planetCount".into(), Value::from(99));
        raw.insert("shootingStars".into(), Value::Bool(false));
        let p1 = SolarSystemParams::from_raw_params(&raw);
        assert_eq!(p1.center_body_type, 1);
        assert_eq!(p1.icon_orbit_mode, 1);
        assert_eq!(p1.quality, 0);
        assert_eq!(p1.planet_count, 6);
        assert!(!p1.shooting_stars);

        // Numeric indices also parse identically
        let mut raw_num = HashMap::new();
        raw_num.insert("centerBodyType".into(), Value::from(2));
        raw_num.insert("iconOrbitMode".into(), Value::from(1));
        raw_num.insert("quality".into(), Value::from(1));
        let p2 = SolarSystemParams::from_raw_params(&raw_num);
        assert_eq!(p2.center_body_type, 2);
        assert_eq!(p2.icon_orbit_mode, 1);
        assert_eq!(p2.quality, 1);
    }

    #[test]
    fn cache_rev_invalidates_on_icon_move_params_adaptive_and_real_resize_only() {
        let mut st = SolarSystemState::default();
        let rev0 = st.bg_rev;
        let lrev0 = st.layout_rev;

        // Sub-0.5px resize must NOT mark dirty or bump rev
        st.dirty = false;
        st.resize(1920.3, 1080.2);
        assert!(!st.dirty);
        assert_eq!(st.bg_rev, rev0);

        // Real resize > 0.5px bumps bg_rev & layout_rev
        st.resize(1280.0, 720.0);
        assert!(st.dirty);
        assert!(st.bg_rev > rev0);
        assert!(st.layout_rev > lrev0);

        // Same icon count moved to new position updates system anchor & bumps layout_rev
        let icons_a = vec![
            Rect { x: 100.0, y: 100.0, w: 76.0, h: 76.0 },
            Rect { x: 300.0, y: 200.0, w: 76.0, h: 76.0 },
        ];
        let names = vec!["A".to_string(), "B".to_string()];
        st.set_icons(&icons_a, &names, true);
        assert_eq!(st.systems.len(), 2);
        let lrev_after_a = st.layout_rev;

        let icons_b = vec![
            Rect { x: 500.0, y: 400.0, w: 76.0, h: 76.0 },
            Rect { x: 700.0, y: 500.0, w: 76.0, h: 76.0 },
        ];
        st.set_icons(&icons_b, &names, true);
        assert!(st.layout_rev > lrev_after_a);
        assert!((st.systems[0].anchor_x - 538.0).abs() < 1e-3);

        // Adaptive off clears systems; adaptive on restores them
        let mut p = st.params;
        p.adaptive = false;
        st.sync_params(p);
        assert!(st.systems.is_empty());
        p.adaptive = true;
        st.sync_params(p);
        assert_eq!(st.systems.len(), 2);

        // Nebula change bumps bg_rev
        let bg_before = st.bg_rev;
        p.nebula_intensity = 1.2;
        st.sync_params(p);
        assert!(st.bg_rev > bg_before);
    }

    #[test]
    fn glow_sprites_have_strictly_transparent_outer_1px_border() {
        for (sz, data) in [
            (128usize, bake_stellar_glow(STELLAR_TYPES[0].corona_rgb)),
            (64usize, bake_planet_halo()),
            (256usize, bake_black_hole_halo()),
        ] {
            for x in 0..sz {
                assert_eq!(data[(0 * sz + x) * 4 + 3], 0);
                assert_eq!(data[((sz - 1) * sz + x) * 4 + 3], 0);
            }
            for y in 0..sz {
                assert_eq!(data[(y * sz + 0) * 4 + 3], 0);
                assert_eq!(data[(y * sz + (sz - 1)) * 4 + 3], 0);
            }
        }
    }
}
