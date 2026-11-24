// DiyarDesk Voxel Craft World (voxelcraft) — native Rust + OpenGL 3.3 Core port.
// Ported 1:1 from prototype/voxelcraft.html (pixel block atlas, cached terrain layer,
// day/night sky cycle, blocky clouds, torches, weather, bees, adaptive icon islands).

pub mod shaders;

use crate::gl::*;
use crate::quiet_snow::Mulberry32;
use crate::state::{AppState, Rect};
use shaders::{VC_COLOR_FRAG, VC_COLOR_VERT, VC_SPRITE_FRAG, VC_SPRITE_VERT};
use std::ffi::c_void;
use std::f32::consts::{PI, TAU};

const TILE: i32 = 16;
const TILE_COUNT: i32 = 6;
const ATLAS_W: i32 = TILE * TILE_COUNT;

const T_DIRT: usize = 0;
const T_GRASS: usize = 1;
const T_STONE: usize = 2;
const T_PLANKS: usize = 3;
const T_TRUNK: usize = 4;
const T_LEAVES: usize = 5;

const GLOW_SIZE: i32 = 128;

const fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    ]
}

#[inline]
fn clamp_f(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn lerp_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let ch = |x: f32, y: f32| ((x + (y - x) * t) * 255.0).round() / 255.0;
    [ch(a[0], b[0]), ch(a[1], b[1]), ch(a[2], b[2])]
}

#[inline]
fn rgba(c: [f32; 3], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a]
}

// =========================================================================
// Biome palettes (1:1 with BIOMES in voxelcraft.html)
// =========================================================================
pub struct Biome {
    pub sky_day_top: [f32; 3],
    pub sky_day_bot: [f32; 3],
    pub sky_sun_top: [f32; 3],
    pub sky_sun_bot: [f32; 3],
    pub sky_nit_top: [f32; 3],
    pub sky_nit_bot: [f32; 3],
    pub hill_far: [f32; 3],
    pub hill_far_nit: [f32; 3],
    pub grass_top: [f32; 3],
    pub grass_mid: [f32; 3],
    pub grass_edge: [f32; 3],
    pub dirt: [[f32; 3]; 3],
    pub stone: [[f32; 3]; 3],
    pub planks: [[f32; 3]; 3],
    pub trunk: [[f32; 3]; 3],
    pub leaves: [[f32; 3]; 3],
    pub water: [[f32; 3]; 3],
    pub glow_rgb: [f32; 3],
    pub weather_kind: usize, // 0 firefly, 1 petal, 2 snow, 3 ember
    pub particles: [[f32; 3]; 3],
}

pub const BIOMES: [Biome; 4] = [
    // overworld
    Biome {
        sky_day_top: rgb(0x6AAEFF),
        sky_day_bot: rgb(0xC6E6FF),
        sky_sun_top: rgb(0xC46C60),
        sky_sun_bot: rgb(0xFFCC80),
        sky_nit_top: rgb(0x0B1226),
        sky_nit_bot: rgb(0x1A2848),
        hill_far: rgb(0x5987A8),
        hill_far_nit: rgb(0x1C2B4A),
        grass_top: rgb(0x64B848),
        grass_mid: rgb(0x519E39),
        grass_edge: rgb(0x3E7F2B),
        dirt: [rgb(0x8A5D3B), rgb(0x764E30), rgb(0x966844)],
        stone: [rgb(0x7C8088), rgb(0x686C74), rgb(0x8E929B)],
        planks: [rgb(0xB8844C), rgb(0x9E6D3B), rgb(0xC7935A)],
        trunk: [rgb(0x6C4A2B), rgb(0x54381E), rgb(0x7D5634)],
        leaves: [rgb(0x478D35), rgb(0x5AA844), rgb(0x387229)],
        water: [rgb(0x3882D6), rgb(0x4A98EA), rgb(0x82C8FF)],
        glow_rgb: [255.0 / 255.0, 190.0 / 255.0, 74.0 / 255.0],
        weather_kind: 0,
        particles: [rgb(0x64B848), rgb(0x8A5D3B), rgb(0x478D35)],
    },
    // cherry
    Biome {
        sky_day_top: rgb(0x86C0FF),
        sky_day_bot: rgb(0xFCE0EC),
        sky_sun_top: rgb(0xD46E92),
        sky_sun_bot: rgb(0xFFCEB2),
        sky_nit_top: rgb(0x18122C),
        sky_nit_bot: rgb(0x30224C),
        hill_far: rgb(0x8E88AD),
        hill_far_nit: rgb(0x2C2246),
        grass_top: rgb(0x72BE58),
        grass_mid: rgb(0x5DA645),
        grass_edge: rgb(0x488834),
        dirt: [rgb(0x875C3E), rgb(0x724C31), rgb(0x946747)],
        stone: [rgb(0x807E8A), rgb(0x6B6975), rgb(0x92909C)],
        planks: [rgb(0xD498A6), rgb(0xB87C8C), rgb(0xE0AAB8)],
        trunk: [rgb(0x52343A), rgb(0x3D2429), rgb(0x66434A)],
        leaves: [rgb(0xF299BE), rgb(0xFFB8D6), rgb(0xD87AA2)],
        water: [rgb(0x4B94DC), rgb(0x64AAEF), rgb(0xA6D8FF)],
        glow_rgb: [255.0 / 255.0, 206.0 / 255.0, 132.0 / 255.0],
        weather_kind: 1,
        particles: [rgb(0xFFB8D6), rgb(0xF299BE), rgb(0x72BE58)],
    },
    // snowy
    Biome {
        sky_day_top: rgb(0x88B0D6),
        sky_day_bot: rgb(0xDAECFA),
        sky_sun_top: rgb(0x868EC4),
        sky_sun_bot: rgb(0xF2B89E),
        sky_nit_top: rgb(0x091022),
        sky_nit_bot: rgb(0x16243E),
        hill_far: rgb(0x7B92AD),
        hill_far_nit: rgb(0x18263D),
        grass_top: rgb(0xF2F7FC),
        grass_mid: rgb(0xDBE7F4),
        grass_edge: rgb(0xBFD3EA),
        dirt: [rgb(0x7D5941), rgb(0x684833), rgb(0x8C664C)],
        stone: [rgb(0x77808C), rgb(0x636C78), rgb(0x89929E)],
        planks: [rgb(0x8A6345), rgb(0x735036), rgb(0x9C7252)],
        trunk: [rgb(0x4C3625), rgb(0x382619), rgb(0x5D4430)],
        leaves: [rgb(0x2B5B44), rgb(0x397357), rgb(0x1F4533)],
        water: [rgb(0x457FB5), rgb(0x5E9AD1), rgb(0xB4DCFF)],
        glow_rgb: [255.0 / 255.0, 174.0 / 255.0, 72.0 / 255.0],
        weather_kind: 2,
        particles: [rgb(0xF2F7FC), rgb(0xDBE7F4), rgb(0x7D5941)],
    },
    // nether
    Biome {
        sky_day_top: rgb(0x360C0E),
        sky_day_bot: rgb(0x701A14),
        sky_sun_top: rgb(0x3A0C10),
        sky_sun_bot: rgb(0x801E16),
        sky_nit_top: rgb(0x24080A),
        sky_nit_bot: rgb(0x52120E),
        hill_far: rgb(0x4C1416),
        hill_far_nit: rgb(0x3B0E10),
        grass_top: rgb(0xA63636),
        grass_mid: rgb(0x8A2828),
        grass_edge: rgb(0x6B1C1C),
        dirt: [rgb(0x581C1C), rgb(0x441313), rgb(0x6B2525)],
        stone: [rgb(0x322B38), rgb(0x251F2B), rgb(0x413948)],
        planks: [rgb(0x78283C), rgb(0x5E1C2C), rgb(0x8F344B)],
        trunk: [rgb(0x692232), rgb(0x4F1624), rgb(0x802C40)],
        leaves: [rgb(0xA62C2B), rgb(0xD45A28), rgb(0x7C1C1C)],
        water: [rgb(0xE8581C), rgb(0xFF8C28), rgb(0xFFD85C)],
        glow_rgb: [255.0 / 255.0, 118.0 / 255.0, 38.0 / 255.0],
        weather_kind: 3,
        particles: [rgb(0xA63636), rgb(0xFF8C28), rgb(0x581C1C)],
    },
];

// =========================================================================
// Parameters
// =========================================================================
#[derive(Clone, Copy)]
pub struct VoxelcraftParams {
    pub adaptive: i32,
    pub biome: usize,
    pub time_of_day: usize, // 0 auto, 1 day, 2 sunset, 3 night
    pub icon_style: usize,  // 0 island, 1 planks, 2 frame
    pub block_size: f32,
    pub cloud_density: f32,
    pub weather_amount: f32,
    pub critter_amount: f32,
    pub torch_glow: f32,
    pub anim_speed: f32,
    pub quality: usize, // 0 Low, 1 Medium, 2 High
    pub seed: u32,
}

impl Default for VoxelcraftParams {
    fn default() -> Self {
        Self {
            adaptive: 1,
            biome: 0,
            time_of_day: 0,
            icon_style: 0,
            block_size: 1.0,
            cloud_density: 1.0,
            weather_amount: 1.0,
            critter_amount: 1.0,
            torch_glow: 1.0,
            anim_speed: 1.0,
            quality: 2,
            seed: 0,
        }
    }
}

impl VoxelcraftParams {
    pub fn effective_seed(&self) -> u32 {
        if self.seed == 0 {
            20261118
        } else {
            self.seed
        }
    }
}

// =========================================================================
// Simulation entities
// =========================================================================
#[derive(Clone, Copy)]
pub struct Cloud {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub speed: f32,
}

#[derive(Clone, Copy)]
pub struct WeatherDrop {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub size: f32,
    pub phase: f32,
}

#[derive(Clone, Copy)]
pub struct Critter {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub phase: f32,
    pub dir: f32,
}

#[derive(Clone, Copy)]
pub struct Torch {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub size: f32,
    pub xp: bool,
    pub col: [f32; 3],
    pub life: f32,
    pub max_life: f32,
}

#[derive(Clone, Copy)]
pub struct Star {
    pub nx: f32,
    pub ny: f32,
    pub size: f32,
    pub phase: f32,
}

#[derive(Clone, Copy)]
pub struct IconState {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub tx: f32,
    pub ty: f32,
    pub tw: f32,
    pub th: f32,
    pub opacity: f32,
    pub bounce: f32,
    pub bounce_vel: f32,
    pub has_torch: bool,
    pub removing: bool,
}

#[derive(Clone, Copy)]
pub struct PondSpan {
    pub start_col: i32,
    pub end_col: i32,
    pub water_y: f32,
}

pub struct VoxelcraftState {
    pub p: VoxelcraftParams,
    pub gw: f32,
    pub gh: f32,
    pub block: f32,
    pub sim_time: f32,
    pub low_mode: bool,
    pub clouds: Vec<Cloud>,
    pub weather: Vec<WeatherDrop>,
    pub critters: Vec<Critter>,
    pub torches: Vec<Torch>,
    pub particles: Vec<Particle>,
    pub stars: Vec<Star>,
    pub pond: Option<PondSpan>,
    pub heights: Vec<f32>,
    pub icon_states: Vec<IconState>,
    pub cache_ok: bool,
    pub atlas_biome: i32,
    rng: Mulberry32,
}

impl Default for VoxelcraftState {
    fn default() -> Self {
        let mut st = Self {
            p: VoxelcraftParams::default(),
            gw: 1920.0,
            gh: 1080.0,
            block: 28.0,
            sim_time: 0.0,
            low_mode: false,
            clouds: Vec::new(),
            weather: Vec::new(),
            critters: Vec::new(),
            torches: Vec::new(),
            particles: Vec::new(),
            stars: Vec::new(),
            pond: None,
            heights: Vec::new(),
            icon_states: Vec::new(),
            cache_ok: false,
            atlas_biome: -1,
            rng: Mulberry32::new(0x57454154),
        };
        st.block = st.compute_block();
        st.build_sky_objects();
        st
    }
}

impl VoxelcraftState {
    pub fn biome(&self) -> &'static Biome {
        &BIOMES[self.p.biome.min(3)]
    }

    pub fn profile(&self) -> (f32, f32, f32) {
        let q = if self.low_mode { 0 } else { self.p.quality.min(2) };
        match q {
            0 => (0.42, 0.45, 0.55),
            1 => (0.75, 0.80, 0.85),
            _ => (1.0, 1.0, 1.0),
        }
    }

    fn compute_block(&self) -> f32 {
        let v = (self.gw.min(self.gh) / 30.0) * clamp_f(self.p.block_size, 0.7, 1.5);
        clamp_f(v.round(), 16.0, 48.0)
    }

    pub fn set_low_mode(&mut self, low: bool) {
        if self.low_mode != low {
            self.low_mode = low;
            self.build_sky_objects();
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = w.max(240.0);
        let nh = h.max(160.0);
        if (self.gw - nw).abs() > 0.5 || (self.gh - nh).abs() > 0.5 {
            self.gw = nw;
            self.gh = nh;
            self.block = self.compute_block();
            self.cache_ok = false;
            self.build_sky_objects();
        }
    }

    pub fn sync_params(&mut self, next: VoxelcraftParams) {
        let biome_changed = self.p.biome != next.biome;
        let seed_changed = self.p.effective_seed() != next.effective_seed();
        let sky_changed = (self.p.cloud_density - next.cloud_density).abs() > 1e-4
            || (self.p.weather_amount - next.weather_amount).abs() > 1e-4
            || (self.p.critter_amount - next.critter_amount).abs() > 1e-4;
        let block_changed = (self.p.block_size - next.block_size).abs() > 1e-4
            || self.p.quality != next.quality;

        self.p = next;

        if biome_changed {
            self.atlas_biome = -1;
            self.cache_ok = false;
        }
        if seed_changed {
            self.cache_ok = false;
        }
        if block_changed {
            self.block = self.compute_block();
            self.cache_ok = false;
        }
        if seed_changed || sky_changed || block_changed {
            self.build_sky_objects();
        }
    }

    pub fn build_sky_objects(&mut self) {
        let (_, weather_scale, critter_scale, cloud_scale) = {
            let (w, c, cl) = self.profile();
            (0.0, w, c, cl)
        };
        let seed = self.p.effective_seed();
        let mut rng = Mulberry32::new((seed ^ 0x534B_5931) as i32);
        let w = self.gw;
        let h = self.gh;
        let block = self.block;

        self.stars.clear();
        for _ in 0..42 {
            let nx = rng.next_f32();
            let ny = 0.04 + rng.next_f32() * 0.44;
            let size = if rng.next_f32() > 0.7 { 3.0 } else { 2.0 };
            let phase = rng.next_f32() * TAU;
            self.stars.push(Star { nx, ny, size, phase });
        }

        let c_count = clamp_f(
            (7.0 * self.p.cloud_density * cloud_scale).round(),
            0.0,
            14.0,
        ) as usize;
        self.clouds.clear();
        for _ in 0..c_count {
            let x = rng.next_f32() * (w + 200.0) - 100.0;
            let y = ((0.06 + rng.next_f32() * 0.24) * h).round();
            let cw = ((4.0 + (rng.next_f32() * 4.0).floor()) * block).round();
            let ch = (block * 0.7).round();
            let speed = 8.0 + rng.next_f32() * 14.0;
            self.clouds.push(Cloud { x, y, w: cw, h: ch, speed });
        }

        let w_count = clamp_f(
            (28.0 * self.p.weather_amount * weather_scale).round(),
            0.0,
            56.0,
        ) as usize;
        self.weather.clear();
        for _ in 0..w_count {
            let x = rng.next_f32() * w;
            let y = rng.next_f32() * h;
            let vx = (rng.next_f32() - 0.5) * 22.0;
            let vy = 16.0 + rng.next_f32() * 26.0;
            let size = if rng.next_f32() > 0.5 { 4.0 } else { 3.0 };
            let phase = rng.next_f32() * TAU;
            self.weather.push(WeatherDrop { x, y, vx, vy, size, phase });
        }

        let k_count = clamp_f(
            (5.0 * self.p.critter_amount * critter_scale).round(),
            0.0,
            10.0,
        ) as usize;
        self.critters.clear();
        for _ in 0..k_count {
            let x = (0.15 + rng.next_f32() * 0.70) * w;
            let y = (0.25 + rng.next_f32() * 0.45) * h;
            let phase = rng.next_f32() * TAU;
            let dir = if rng.next_f32() > 0.5 { 1.0 } else { -1.0 };
            self.critters.push(Critter { x, y, vx: 0.0, vy: 0.0, phase, dir });
        }
    }

    pub fn set_icons(&mut self, rects: &[Rect], vis: bool) {
        let icons: &[Rect] = if vis { rects } else { &[] };
        let mut matched = vec![false; self.icon_states.len()];
        for (i, ic) in icons.iter().enumerate() {
            let mut best_idx: i32 = -1;
            let mut best_dist = 48.0f32;
            for (s, st) in self.icon_states.iter().enumerate() {
                if matched[s] {
                    continue;
                }
                let d = ((ic.x - st.tx).powi(2) + (ic.y - st.ty).powi(2)).sqrt();
                if d < best_dist {
                    best_dist = d;
                    best_idx = s as i32;
                }
            }
            if best_idx >= 0 {
                let st = &mut self.icon_states[best_idx as usize];
                st.tx = ic.x;
                st.ty = ic.y;
                st.tw = ic.w;
                st.th = ic.h;
                st.removing = false;
                matched[best_idx as usize] = true;
            } else {
                self.icon_states.push(IconState {
                    x: ic.x,
                    y: ic.y,
                    w: ic.w,
                    h: ic.h,
                    tx: ic.x,
                    ty: ic.y,
                    tw: ic.w,
                    th: ic.h,
                    opacity: 0.0,
                    bounce: 0.0,
                    bounce_vel: 0.0,
                    has_torch: i % 3 == 0,
                    removing: false,
                });
                matched.push(true);
            }
        }
        for (s, st) in self.icon_states.iter_mut().enumerate() {
            if !matched.get(s).copied().unwrap_or(false) {
                st.removing = true;
            }
        }
    }

    pub fn pop_block_break(&mut self, x: f32, y: f32) {
        let b = self.biome();
        for i in 0..12 {
            if self.particles.len() >= 96 {
                self.particles.remove(0);
            }
            let is_xp = i < 4;
            let ang = -PI * 0.15 - self.rng.next_f32() * PI * 0.7;
            let spd = 75.0 + self.rng.next_f32() * 135.0;
            let vx = ang.cos() * spd + (self.rng.next_f32() - 0.5) * 45.0;
            let vy = ang.sin() * spd;
            let col = if is_xp {
                if i % 2 == 0 {
                    rgb(0xA8FF56)
                } else {
                    rgb(0xFFE85C)
                }
            } else {
                b.particles[i % 3]
            };
            self.particles.push(Particle {
                x,
                y,
                vx,
                vy,
                size: if is_xp { 6.0 } else { 5.0 },
                xp: is_xp,
                col,
                life: 0.0,
                max_life: if is_xp { 1.15 } else { 0.75 },
            });
        }

        for st in self.icon_states.iter_mut() {
            let d = ((st.x + st.w * 0.5 - x).powi(2) + (st.y + st.h * 0.5 - y).powi(2)).sqrt();
            if d < 160.0 {
                st.bounce_vel += (1.0 - d / 160.0) * 90.0;
            }
        }
    }

    pub fn step(&mut self, dt: f32, pointer: (f32, f32, bool)) {
        let spd = clamp_f(self.p.anim_speed, 0.2, 2.5);
        self.sim_time += dt * spd;
        let kind = self.biome().weather_kind;
        let w = self.gw;
        let h = self.gh;
        let t = self.sim_time;

        for c in self.clouds.iter_mut() {
            c.x += c.speed * spd * dt;
            if c.x > w + 40.0 {
                c.x = -c.w - 40.0;
            }
        }

        for d in self.weather.iter_mut() {
            match kind {
                0 => {
                    d.x += (t * 0.9 + d.phase).sin() * 18.0 * dt;
                    d.y += (t * 1.1 + d.phase).cos() * 14.0 * dt;
                }
                3 => {
                    d.x += (t * 2.2 + d.phase).sin() * 20.0 * dt;
                    d.y -= d.vy * spd * dt;
                    if d.y < -12.0 {
                        d.y = h + 12.0;
                        d.x = self.rng.next_f32() * w;
                    }
                }
                _ => {
                    d.x += (d.vx + (t * 1.3 + d.phase).sin() * 16.0) * dt;
                    d.y += d.vy * spd * dt;
                    if d.y > h + 12.0 {
                        d.y = -12.0;
                        d.x = self.rng.next_f32() * w;
                    }
                    if d.x > w + 12.0 {
                        d.x = -12.0;
                    }
                    if d.x < -12.0 {
                        d.x = w + 12.0;
                    }
                }
            }
        }

        let (px, py, pactive) = pointer;
        for (i, cr) in self.critters.iter_mut().enumerate() {
            let mut tx = cr.x + (t * 0.55 + cr.phase).cos() * 45.0;
            let mut ty = cr.y + (t * 0.85 + cr.phase).sin() * 24.0;
            if pactive && i == 0 {
                tx = px + (t * 1.8).cos() * 32.0;
                ty = py + (t * 2.1).sin() * 22.0;
            }
            cr.vx += ((tx - cr.x) * 2.5 - cr.vx * 3.0) * dt;
            cr.vy += ((ty - cr.y) * 2.5 - cr.vy * 3.0) * dt;
            cr.x = clamp_f(cr.x + cr.vx * dt, 24.0, w - 24.0);
            cr.y = clamp_f(cr.y + cr.vy * dt, 24.0, h - 60.0);
            if cr.vx.abs() > 2.0 {
                cr.dir = if cr.vx >= 0.0 { 1.0 } else { -1.0 };
            }
        }

        let mut i = self.particles.len();
        while i > 0 {
            i -= 1;
            let p = &mut self.particles[i];
            p.life += dt;
            if p.life >= p.max_life {
                self.particles.remove(i);
                continue;
            }
            p.vy += if p.xp { 160.0 } else { 380.0 } * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }

        let pos_alpha = 1.0 - (-10.0 * dt).exp();
        let mut i = self.icon_states.len();
        while i > 0 {
            i -= 1;
            let st = &mut self.icon_states[i];
            st.x = lerp(st.x, st.tx, pos_alpha);
            st.y = lerp(st.y, st.ty, pos_alpha);
            st.w = lerp(st.w, st.tw, pos_alpha);
            st.h = lerp(st.h, st.th, pos_alpha);
            if st.removing {
                st.opacity = (st.opacity - dt * 2.6).max(0.0);
                if st.opacity <= 0.005 {
                    self.icon_states.remove(i);
                    continue;
                }
            } else {
                st.opacity = (st.opacity + dt * 3.2).min(1.0);
            }
            let acc = -st.bounce * 120.0 - st.bounce_vel * 12.0;
            st.bounce_vel += acc * dt;
            st.bounce = clamp_f(st.bounce + st.bounce_vel * dt, -10.0, 12.0);
        }
    }

    pub fn sky_phase(&self) -> f32 {
        match self.p.time_of_day {
            1 => 0.25,
            2 => 0.50,
            3 => 0.82,
            _ => (self.sim_time * 0.012 + 0.18) % 1.0,
        }
    }
}

pub struct SkyColors {
    pub top: [f32; 3],
    pub bot: [f32; 3],
    pub night_amt: f32,
}

pub fn compute_sky_colors(b: &Biome, phase: f32) -> SkyColors {    if phase < 0.40 {
        return SkyColors { top: b.sky_day_top, bot: b.sky_day_bot, night_amt: 0.0 };
    }
    if phase < 0.50 {
        let u = (phase - 0.40) / 0.10;
        return SkyColors {
            top: lerp_rgb(b.sky_day_top, b.sky_sun_top, u),
            bot: lerp_rgb(b.sky_day_bot, b.sky_sun_bot, u),
            night_amt: u * 0.3,
        };
    }
    if phase < 0.62 {
        let u = (phase - 0.50) / 0.12;
        return SkyColors {
            top: lerp_rgb(b.sky_sun_top, b.sky_nit_top, u),
            bot: lerp_rgb(b.sky_sun_bot, b.sky_nit_bot, u),
            night_amt: 0.3 + u * 0.7,
        };
    }
    if phase < 0.90 {
        return SkyColors { top: b.sky_nit_top, bot: b.sky_nit_bot, night_amt: 1.0 };
    }
    let u = (phase - 0.90) / 0.10;
    SkyColors {
        top: lerp_rgb(b.sky_nit_top, b.sky_day_top, u),
        bot: lerp_rgb(b.sky_nit_bot, b.sky_day_bot, u),
        night_amt: 1.0 - u,
    }
}

// =========================================================================
// Procedural 16x16 block atlas + 128x128 radial glow sprite
// =========================================================================
fn tile_fill(buf: &mut [u8], x: i32, y: i32, w: i32, h: i32, c: [f32; 3]) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(TILE);
    let y1 = (y + h).min(TILE);
    let r = (c[0] * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (c[1] * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (c[2] * 255.0).round().clamp(0.0, 255.0) as u8;
    for py in y0..y1 {
        for px in x0..x1 {
            let i = ((py * TILE + px) * 4) as usize;
            buf[i] = r;
            buf[i + 1] = g;
            buf[i + 2] = b;
            buf[i + 3] = 255;
        }
    }
}

fn tile_copy(dst: &mut [u8], src: &[u8]) {
    dst.copy_from_slice(src);
}

pub fn bake_atlas(biome_idx: usize) -> Vec<u8> {
    let b = &BIOMES[biome_idx.min(3)];
    let mut rng = Mulberry32::new(0x564F_584C);
    let mut atlas = vec![0u8; (ATLAS_W * TILE * 4) as usize];

    // dirt
    let mut dirt = vec![0u8; (TILE * TILE * 4) as usize];
    tile_fill(&mut dirt, 0, 0, 16, 16, b.dirt[0]);
    let mut y = 0;
    while y < 16 {
        let mut x = 0;
        while x < 16 {
            let r = rng.next_f32();
            if r < 0.25 {
                tile_fill(&mut dirt, x, y, 2, 2, b.dirt[1]);
            } else if r > 0.82 {
                tile_fill(&mut dirt, x, y, 2, 2, b.dirt[2]);
            }
            x += 2;
        }
        y += 2;
    }

    // grass
    let mut grass = vec![0u8; (TILE * TILE * 4) as usize];
    tile_copy(&mut grass, &dirt);
    tile_fill(&mut grass, 0, 0, 16, 5, b.grass_mid);
    tile_fill(&mut grass, 0, 0, 16, 3, b.grass_top);
    let drops = [2, 3, 1, 4, 2, 3, 1, 2];
    for i in 0..8 {
        let c = if i % 2 == 0 { b.grass_mid } else { b.grass_edge };
        tile_fill(&mut grass, i * 2, 5, 2, drops[i as usize], c);
    }

    // stone
    let mut stone = vec![0u8; (TILE * TILE * 4) as usize];
    tile_fill(&mut stone, 0, 0, 16, 16, b.stone[0]);
    let mut y = 0;
    while y < 16 {
        let mut x = 0;
        while x < 16 {
            let r = rng.next_f32();
            if r < 0.28 {
                tile_fill(&mut stone, x, y, 2, 2, b.stone[1]);
            } else if r > 0.78 {
                tile_fill(&mut stone, x, y, 2, 2, b.stone[2]);
            }
            x += 2;
        }
        y += 2;
    }

    // planks
    let mut planks = vec![0u8; (TILE * TILE * 4) as usize];
    tile_fill(&mut planks, 0, 0, 16, 16, b.planks[0]);
    tile_fill(&mut planks, 0, 1, 16, 2, b.planks[2]);
    tile_fill(&mut planks, 0, 9, 16, 2, b.planks[2]);
    tile_fill(&mut planks, 0, 7, 16, 1, b.planks[1]);
    tile_fill(&mut planks, 0, 15, 16, 1, b.planks[1]);
    tile_fill(&mut planks, 7, 0, 1, 8, b.planks[1]);
    tile_fill(&mut planks, 14, 8, 1, 8, b.planks[1]);

    // trunk
    let mut trunk = vec![0u8; (TILE * TILE * 4) as usize];
    tile_fill(&mut trunk, 0, 0, 16, 16, b.trunk[0]);
    tile_fill(&mut trunk, 2, 0, 2, 16, b.trunk[1]);
    tile_fill(&mut trunk, 10, 0, 2, 16, b.trunk[1]);
    tile_fill(&mut trunk, 6, 2, 2, 10, b.trunk[2]);
    tile_fill(&mut trunk, 13, 4, 2, 9, b.trunk[2]);

    // leaves
    let mut leaves = vec![0u8; (TILE * TILE * 4) as usize];
    tile_fill(&mut leaves, 0, 0, 16, 16, b.leaves[0]);
    let mut y = 0;
    while y < 16 {
        let mut x = 0;
        while x < 16 {
            let r = rng.next_f32();
            if r < 0.30 {
                tile_fill(&mut leaves, x, y, 2, 2, b.leaves[1]);
            } else if r > 0.74 {
                tile_fill(&mut leaves, x, y, 2, 2, b.leaves[2]);
            }
            x += 2;
        }
        y += 2;
    }

    for (idx, tile) in [dirt, grass, stone, planks, trunk, leaves].iter().enumerate() {
        let ox = idx as i32 * TILE;
        for row in 0..TILE {
            let src = (row * TILE * 4) as usize;
            let dst = ((row * ATLAS_W + ox) * 4) as usize;
            atlas[dst..dst + (TILE * 4) as usize]
                .copy_from_slice(&tile[src..src + (TILE * 4) as usize]);
        }
    }
    atlas
}

pub fn build_glow_texture(biome_idx: usize) -> Vec<u8> {
    let b = &BIOMES[biome_idx.min(3)];
    let mut buf = vec![0u8; (GLOW_SIZE * GLOW_SIZE * 4) as usize];
    let c = GLOW_SIZE as f32 * 0.5;
    let stops = [(0.0f32, 0.65f32), (0.35, 0.25), (1.0, 0.0)];
    for y in 0..GLOW_SIZE {
        for x in 0..GLOW_SIZE {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            let d = (dx * dx + dy * dy).sqrt() / c;
            let a = if d <= stops[0].0 {
                stops[0].1
            } else if d >= stops[2].0 {
                0.0
            } else if d <= stops[1].0 {
                lerp(stops[0].1, stops[1].1, (d - stops[0].0) / (stops[1].0 - stops[0].0))
            } else {
                lerp(stops[1].1, stops[2].1, (d - stops[1].0) / (stops[2].0 - stops[1].0))
            };
            let i = ((y * GLOW_SIZE + x) * 4) as usize;
            buf[i] = (b.glow_rgb[0] * 255.0).round() as u8;
            buf[i + 1] = (b.glow_rgb[1] * 255.0).round() as u8;
            buf[i + 2] = (b.glow_rgb[2] * 255.0).round() as u8;
            buf[i + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    buf
}

// =========================================================================
// OpenGL 3.3 Core renderer
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

struct VcFbo {
    fbo: GLuint,
    tex: GLuint,
    w: i32,
    h: i32,
}

impl VcFbo {
    fn empty() -> Self {
        Self { fbo: 0, tex: 0, w: 0, h: 0 }
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
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
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

pub struct VoxelcraftRenderer {
    prog_color: GLuint,
    prog_sprite: GLuint,
    dummy_vao: GLuint,
    color_vao: GLuint,
    color_vbo: GLuint,
    sprite_vao: GLuint,
    sprite_vbo: GLuint,
    atlas_tex: GLuint,
    glow_tex: GLuint,
    terrain: VcFbo,
    batch: Vec<ColorVert>,
    sprites: Vec<SpriteVert>,
    last_w: i32,
    last_h: i32,
    atlas_biome: i32,
}

impl VoxelcraftRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_color = gl.compile_program(VC_COLOR_VERT, VC_COLOR_FRAG)?;
        let prog_sprite = gl.compile_program(VC_SPRITE_VERT, VC_SPRITE_FRAG)?;

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

        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        let mut glow_tex = 0;
        (gl.GenTextures)(1, &mut glow_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, glow_tex);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            prog_color,
            prog_sprite,
            dummy_vao,
            color_vao,
            color_vbo,
            sprite_vao,
            sprite_vbo,
            atlas_tex,
            glow_tex,
            terrain: VcFbo::empty(),
            batch: Vec::with_capacity(8192),
            sprites: Vec::with_capacity(8192),
            last_w: 0,
            last_h: 0,
            atlas_biome: -1,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        self.terrain.destroy(gl);
        if self.atlas_tex != 0 {
            (gl.DeleteTextures)(1, &self.atlas_tex);
            self.atlas_tex = 0;
        }
        if self.glow_tex != 0 {
            (gl.DeleteTextures)(1, &self.glow_tex);
            self.glow_tex = 0;
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
        for p in [self.prog_color, self.prog_sprite] {
            if p != 0 {
                (gl.DeleteProgram)(p);
            }
        }
    }

    // ---- batching helpers ----
    fn push_tri(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], col: [f32; 4]) {
        self.batch.push(ColorVert { pos: a, col });
        self.batch.push(ColorVert { pos: b, col });
        self.batch.push(ColorVert { pos: c, col });
    }

    fn add_rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        self.push_tri([x, y], [x + w, y], [x + w, y + h], col);
        self.push_tri([x, y], [x + w, y + h], [x, y + h], col);
    }

    fn add_rect_vgrad(&mut self, x: f32, y: f32, w: f32, h: f32, top: [f32; 4], bot: [f32; 4]) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let a = ColorVert { pos: [x, y], col: top };
        let b = ColorVert { pos: [x + w, y], col: top };
        let c = ColorVert { pos: [x + w, y + h], col: bot };
        let d = ColorVert { pos: [x, y + h], col: bot };
        self.batch.push(a);
        self.batch.push(b);
        self.batch.push(c);
        self.batch.push(a);
        self.batch.push(c);
        self.batch.push(d);
    }

    fn push_tile(&mut self, x: f32, y: f32, size: f32, tile: usize, col: [f32; 4]) {
        let u0 = (tile as f32 * TILE as f32) / ATLAS_W as f32;
        let u1 = ((tile as f32 + 1.0) * TILE as f32) / ATLAS_W as f32;
        let x1 = x + size;
        let y1 = y + size;
        let p0 = SpriteVert { pos: [x, y], uv: [u0, 1.0], col };
        let p1 = SpriteVert { pos: [x1, y], uv: [u1, 1.0], col };
        let p2 = SpriteVert { pos: [x1, y1], uv: [u1, 0.0], col };
        let p3 = SpriteVert { pos: [x, y1], uv: [u0, 0.0], col };
        self.sprites.extend_from_slice(&[p0, p1, p2, p0, p2, p3]);
    }

    fn push_sprite_rect(&mut self, x: f32, y: f32, w: f32, h: f32, col: [f32; 4]) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let x1 = x + w;
        let y1 = y + h;
        let p0 = SpriteVert { pos: [x, y], uv: [0.0, 1.0], col };
        let p1 = SpriteVert { pos: [x1, y], uv: [1.0, 1.0], col };
        let p2 = SpriteVert { pos: [x1, y1], uv: [1.0, 0.0], col };
        let p3 = SpriteVert { pos: [x, y1], uv: [0.0, 0.0], col };
        self.sprites.extend_from_slice(&[p0, p1, p2, p0, p2, p3]);
    }

    unsafe fn flush_color(&mut self, gl: &GlFns, w: f32, h: f32) {
        if self.batch.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_color);
        let loc_res = gl.uniform_loc(self.prog_color, "u_res");
        (gl.Uniform2f)(loc_res, w, h);
        (gl.BindVertexArray)(self.color_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.color_vbo);
        let bytes = (self.batch.len() * std::mem::size_of::<ColorVert>()) as GLsizeiptr;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            bytes,
            self.batch.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.batch.len() as GLsizei);
        self.batch.clear();
    }

    unsafe fn flush_sprites(&mut self, gl: &GlFns, tex: GLuint, w: f32, h: f32) {
        if self.sprites.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_sprite);
        let loc_res = gl.uniform_loc(self.prog_sprite, "u_res");
        let loc_atlas = gl.uniform_loc(self.prog_sprite, "u_atlas");
        (gl.Uniform2f)(loc_res, w, h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, tex);
        (gl.Uniform1i)(loc_atlas, 0);
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

    unsafe fn flush_atlas(&mut self, gl: &GlFns, w: f32, h: f32) {
        let tex = self.atlas_tex;
        self.flush_sprites(gl, tex, w, h);
    }

    unsafe fn flush_glow(&mut self, gl: &GlFns, w: f32, h: f32) {
        let tex = self.glow_tex;
        self.flush_sprites(gl, tex, w, h);
    }

    unsafe fn flush_terrain(&mut self, gl: &GlFns, w: f32, h: f32) {
        let tex = self.terrain.tex;
        self.flush_sprites(gl, tex, w, h);
    }

    unsafe fn upload_atlas(&mut self, gl: &GlFns, biome_idx: usize) {
        if self.atlas_biome == biome_idx as i32 {
            return;
        }
        let atlas = bake_atlas(biome_idx);
        (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            ATLAS_W,
            TILE,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            atlas.as_ptr() as *const c_void,
        );
        let glow = build_glow_texture(biome_idx);
        (gl.BindTexture)(GL_TEXTURE_2D, self.glow_tex);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            GLOW_SIZE,
            GLOW_SIZE,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            glow.as_ptr() as *const c_void,
        );
        self.atlas_biome = biome_idx as i32;
    }

    fn draw_block_tree(
        &mut self,
        col_x: f32,
        ground_y: f32,
        is_pine: bool,
        block: f32,
        rng: &mut Mulberry32,
    ) {
        let trunk_h = if is_pine {
            4.0 + (rng.next_f32() * 2.0).floor()
        } else {
            3.0 + (rng.next_f32() * 2.0).floor()
        };
        let mut i = 1.0;
        while i <= trunk_h {
            self.push_tile(col_x, ground_y - i * block, block, T_TRUNK, [1.0; 4]);
            i += 1.0;
        }
        let top_y = ground_y - trunk_h * block;
        if is_pine {
            let layers = [(0.0f32, 2i32), (-1.0, 2), (-2.0, 1), (-3.0, 1), (-4.0, 0)];
            for (dy, w) in layers {
                let ly = top_y + dy * block;
                let mut dx = -w;
                while dx <= w {
                    self.push_tile(
                        col_x + dx as f32 * block,
                        ly,
                        block,
                        T_LEAVES,
                        [1.0; 4],
                    );
                    dx += 1;
                }
            }
        } else {
            let mut dy = -2;
            while dy <= 0 {
                let span = if dy == -2 { 1 } else { 2 };
                let mut dx: i32 = -span;
                while dx <= span {
                    if dy == -2 && dx.abs() == 1 && rng.next_f32() > 0.6 {
                        dx += 1;
                        continue;
                    }
                    self.push_tile(
                        col_x + dx as f32 * block,
                        top_y + dy as f32 * block,
                        block,
                        T_LEAVES,
                        [1.0; 4],
                    );
                    dx += 1;
                }
                dy += 1;
            }
            self.push_tile(col_x, top_y - 3.0 * block, block, T_LEAVES, [1.0; 4]);
        }
    }

    unsafe fn build_terrain(&mut self, gl: &GlFns, st: &mut AppState, pw: i32, ph: i32) {
        let vc = &mut st.voxelcraft;
        let w = vc.gw;
        let h = vc.gh;
        let block = vc.block;
        let seed = vc.p.effective_seed();
        let biome_idx = vc.p.biome.min(3);
        let is_pine = biome_idx == 2;

        self.upload_atlas(gl, biome_idx);

        self.terrain.ensure(gl, pw, ph);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.terrain.fbo);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.ClearColor)(0.0, 0.0, 0.0, 0.0);
        (gl.Clear)(GL_COLOR_BUFFER_BIT);

        self.sprites.clear();

        let cols = (w / block).ceil() as i32 + 1;
        let base_row = ((h - block * 2.2) / block).floor() as i32;
        let mut rng = Mulberry32::new((seed ^ 0x5445_5252) as i32);

        let p_start = (cols as f32 * 0.48).floor() as i32;
        let p_end = (cols - 3).min(p_start + (cols as f32 * 0.14).floor().max(4.0) as i32);
        vc.pond = Some(PondSpan {
            start_col: p_start,
            end_col: p_end,
            water_y: (base_row + 1) as f32 * block,
        });

        vc.heights.clear();
        vc.heights.resize(cols as usize, 0.0);
        vc.torches.clear();

        for c in 0..cols {
            let h_offset = if c >= p_start && c <= p_end {
                -1
            } else {
                let wave = ((c as f32 * 0.35 + (seed % 7) as f32).sin() * 1.4
                    + (c as f32 * 0.18).cos() * 1.1)
                    .round();
                clamp_f(wave, 0.0, 3.0) as i32
            };
            let top_row = base_row - h_offset;
            vc.heights[c as usize] = top_row as f32 * block;

            let mut r = top_row;
            while (r as f32) * block < h {
                let px = c as f32 * block;
                let py = r as f32 * block;
                let tile = if c >= p_start && c <= p_end && r == top_row {
                    T_STONE
                } else if r == top_row {
                    T_GRASS
                } else if r <= top_row + 1 {
                    T_DIRT
                } else {
                    T_STONE
                };
                self.push_tile(px, py, block, tile, [1.0; 4]);
                r += 1;
            }
        }

        let mut last_tree_col = -5;
        let mut c = 2;
        while c < cols - 3 {
            if c >= p_start - 2 && c <= p_end + 2 {
                c += 1;
                continue;
            }
            if c - last_tree_col >= 5 && rng.next_f32() > 0.62 {
                let ground = vc.heights[c as usize];
                self.draw_block_tree(c as f32 * block, ground, is_pine, block, &mut rng);
                last_tree_col = c;
                if rng.next_f32() > 0.45
                    && c + 2 < cols
                    && (c + 2 < p_start - 1 || c + 2 > p_end + 1)
                {
                    vc.torches.push(Torch {
                        x: (c + 2) as f32 * block + block * 0.42,
                        y: vc.heights[(c + 2) as usize],
                    });
                }
            }
            c += 1;
        }
        if vc.torches.is_empty() && cols > 5 {
            vc.torches.push(Torch {
                x: 3.0 * block + block * 0.42,
                y: vc.heights[3],
            });
        }

        self.flush_atlas(gl, w, h);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        vc.cache_ok = true;
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_torch(
        &mut self,
        tx: f32,
        ground_y: f32,
        glow_strength: f32,
        night_amt: f32,
        block: f32,
        sim_time: f32,
    ) {
        let tw = (block * 0.18).round().max(4.0);
        let th = (block * 0.55).round();
        self.add_rect(tx.round(), (ground_y - th).round(), tw, th, rgba(rgb(0x6B4728), 1.0));
        let flicker = 0.85 + 0.15 * (sim_time * 9.0 + tx).sin();
        self.add_rect(
            (tx - 1.0).round(),
            (ground_y - th - tw).round(),
            tw + 2.0,
            tw + 1.0,
            rgba(rgb(0xFFD659), 1.0),
        );
        self.add_rect(
            tx.round(),
            (ground_y - th - tw + 1.0).round(),
            tw,
            (tw - 1.0).max(2.0),
            rgba(rgb(0xFFF8C4), 1.0),
        );

        if glow_strength > 0.02 {
            let g_rad = block * (2.6 + night_amt * 1.4) * flicker;
            let a = clamp_f((0.35 + night_amt * 0.55) * glow_strength, 0.0, 0.95);
            self.push_sprite_rect(
                tx + tw * 0.5 - g_rad,
                ground_y - th - g_rad,
                g_rad * 2.0,
                g_rad * 2.0,
                [1.0, 1.0, 1.0, a],
            );
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let w = st.css_w();
        let h = st.css_h();

        st.voxelcraft.resize(w, h);
        let low = st.low_mode;
        st.voxelcraft.set_low_mode(low);

        if self.last_w != pw || self.last_h != ph {
            self.last_w = pw;
            self.last_h = ph;
            st.voxelcraft.cache_ok = false;
        }

        if !st.voxelcraft.cache_ok {
            self.build_terrain(gl, st, pw, ph);
        }

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        let vc = &st.voxelcraft;
        let b = vc.biome();
        let block = vc.block;
        let sim_time = vc.sim_time;
        let phase = vc.sky_phase();
        let sky = compute_sky_colors(b, phase);
        let night = sky.night_amt;
        let biome_idx = vc.p.biome.min(3);
        let icon_style = vc.p.icon_style;
        let t_glow = clamp_f(vc.p.torch_glow, 0.0, 1.8);
        let adaptive = vc.p.adaptive != 0;

        // 1. Sky backdrop
        self.add_rect_vgrad(
            0.0,
            0.0,
            w,
            h * 0.85,
            rgba(sky.top, 1.0),
            rgba(sky.bot, 1.0),
        );
        self.add_rect(0.0, h * 0.85, w, h - h * 0.85, rgba(sky.bot, 1.0));

        // 2. Twinkling square stars
        if night > 0.15 && biome_idx != 3 {
            for s in &vc.stars {
                let tw = 0.5 + 0.5 * (sim_time * 2.1 + s.phase).sin();
                let a = night * tw * 0.85;
                self.add_rect(
                    (s.nx * w).round(),
                    (s.ny * h).round(),
                    s.size,
                    s.size,
                    [1.0, 1.0, 1.0, a],
                );
            }
        }

        // 3. Square voxel Sun / Moon
        if biome_idx != 3 {
            let sun_size = (block * 1.45).round();
            let arc = phase * TAU;
            let sun_x = (w * 0.5 + (arc - PI * 0.85).cos() * w * 0.36).round();
            let sun_y = (h * 0.42 - (arc - PI * 0.85).sin() * h * 0.32).round();
            let is_moon = night > 0.55;
            let col = if is_moon { rgb(0xE8F1FF) } else { rgb(0xFFF3A1) };
            self.add_rect(sun_x, sun_y, sun_size, sun_size, rgba(col, 1.0));
            if is_moon {
                self.add_rect(sun_x + 6.0, sun_y + 6.0, 8.0, 8.0, rgba(rgb(0xC5D6F0), 1.0));
            }
        }

        // 4. Blocky drifting clouds
        let cloud_col = if biome_idx == 3 {
            [1.0, 92.0 / 255.0, 42.0 / 255.0, 0.14]
        } else {
            [1.0, 1.0, 1.0, 0.78 - night * 0.55]
        };
        for c in &vc.clouds {
            let cx = c.x.round();
            self.add_rect(cx, c.y, c.w, c.h, cloud_col);
            self.add_rect(
                cx + (c.w * 0.18).round(),
                c.y - (c.h * 0.55).round(),
                (c.w * 0.64).round(),
                (c.h * 0.55).round(),
                cloud_col,
            );
        }

        // 5. Distant voxel silhouette hills
        let hill_col = if night > 0.5 { b.hill_far_nit } else { b.hill_far };
        let far_step = (block * 1.6).round();
        let hill_seed = vc.p.effective_seed() as f64;
        let mut x = 0.0;
        while x < w {
            let wave = ((x as f64 * 0.008 + hill_seed).sin() * 0.5 + 0.5) as f32;
            let hh = (wave * block * 3.5 + block * 3.5).round();
            self.add_rect(
                x,
                h - hh - block * 2.0,
                far_step + 1.0,
                hh + block * 2.0,
                rgba(hill_col, 1.0),
            );
            x += far_step;
        }

        self.flush_color(gl, w, h);

        // 6. Cached voxel terrain & forest
        self.push_sprite_rect(0.0, 0.0, w, h, [1.0, 1.0, 1.0, 1.0]);
        self.flush_terrain(gl, w, h);

        // 7. Animated pond water / Nether lava pool
        if let Some(pond) = vc.pond {
            let wx = pond.start_col as f32 * block;
            let ww = (pond.end_col - pond.start_col + 1) as f32 * block;
            let wy = pond.water_y - (block * 0.22).round();
            let wh = (block * 1.22).round();
            self.add_rect(wx, wy, ww, wh, rgba(b.water[0], 1.0));
            self.add_rect(
                wx,
                wy,
                ww,
                (block * 0.25).round().max(4.0),
                rgba(b.water[1], 1.0),
            );
            let glint_x = wx + ((sim_time * 28.0) % (ww - 18.0).max(20.0)).round();
            self.add_rect(glint_x, wy + 2.0, 14.0, 3.0, rgba(b.water[2], 1.0));
        }

        // 8. Ground torches & glow
        for t in &vc.torches {
            self.draw_torch(t.x, t.y, t_glow, night, block, sim_time);
        }
        self.flush_color(gl, w, h);
        self.flush_glow(gl, w, h);

        // 9. Weather particles (fireflies / petals / snow / embers)
        for d in &vc.weather {
            let col = match b.weather_kind {
                0 => {
                    let pulse = (0.35 + 0.65 * (sim_time * 3.2 + d.phase).sin())
                        * (0.25 + night * 0.75);
                    if pulse <= 0.05 {
                        continue;
                    }
                    [204.0 / 255.0, 1.0, 118.0 / 255.0, pulse]
                }
                1 => [1.0, 184.0 / 255.0, 214.0 / 255.0, 0.85],
                3 => [1.0, 138.0 / 255.0, 48.0 / 255.0, 0.85],
                _ => [244.0 / 255.0, 249.0 / 255.0, 1.0, 0.88],
            };
            self.add_rect(d.x.round(), d.y.round(), d.size, d.size, col);
        }

        // 10. Cute pixel bees / critters
        for cr in &vc.critters {
            let bx = cr.x.round();
            let by = (cr.y + (sim_time * 6.0 + cr.phase).sin() * 3.0).round();
            if biome_idx == 3 {
                self.add_rect(bx - 5.0, by - 4.0, 10.0, 8.0, rgba(rgb(0xFF9C47), 1.0));
                self.add_rect(
                    bx + if cr.dir > 0.0 { 1.0 } else { -4.0 },
                    by - 2.0,
                    3.0,
                    2.0,
                    rgba(rgb(0xFFF5B8), 1.0),
                );
            } else {
                self.add_rect(bx - 6.0, by - 4.0, 12.0, 8.0, rgba(rgb(0xFFD644), 1.0));
                self.add_rect(bx - 2.0, by - 4.0, 3.0, 8.0, rgba(rgb(0x382A1E), 1.0));
                self.add_rect(bx + 3.0, by - 4.0, 3.0, 8.0, rgba(rgb(0x382A1E), 1.0));
                let wing_lift = if (sim_time * 24.0 + cr.phase).sin() > 0.0 {
                    -3.0
                } else {
                    -1.0
                };
                self.add_rect(
                    bx - 3.0,
                    by - 4.0 + wing_lift,
                    6.0,
                    3.0,
                    [240.0 / 255.0, 250.0 / 255.0, 1.0, 0.85],
                );
            }
        }
        self.flush_color(gl, w, h);

        // 11. Adaptive desktop icon voxel platforms
        if adaptive {
            let tile_s = (block * 0.82).round().max(18.0);
            if icon_style == 2 {
                for s in &vc.icon_states {
                    if s.opacity <= 0.01 {
                        continue;
                    }
                    let fw = (s.w + 14.0).round();
                    let fh = (s.h + 10.0).round();
                    let fx = (s.x - 7.0).round();
                    let fy = (s.y - 5.0 + s.bounce).round();
                    let a = s.opacity;
                    self.add_rect(fx, fy, fw, fh, [24.0 / 255.0, 30.0 / 255.0, 44.0 / 255.0, 0.36 * a]);
                    let sc = rgba(b.grass_top, a);
                    self.add_rect(fx - 1.5, fy - 1.5, fw + 3.0, 3.0, sc);
                    self.add_rect(fx - 1.5, fy + fh - 1.5, fw + 3.0, 3.0, sc);
                    self.add_rect(fx - 1.5, fy - 1.5, 3.0, fh + 3.0, sc);
                    self.add_rect(fx + fw - 1.5, fy - 1.5, 3.0, fh + 3.0, sc);
                }
                self.flush_color(gl, w, h);
            } else {
                let tex = if icon_style == 1 { T_PLANKS } else { T_GRASS };
                for s in &vc.icon_states {
                    if s.opacity <= 0.01 {
                        continue;
                    }
                    let base_y = (s.y + s.h + 4.0 + s.bounce).round();
                    let count = (((s.w + 16.0) / tile_s).ceil()).max(3.0);
                    let total_w = count * tile_s;
                    let start_x = (s.x + s.w * 0.5 - total_w * 0.5).round();
                    let mut k = 0.0;
                    while k < count {
                        self.push_tile(
                            start_x + k * tile_s,
                            base_y - (tile_s * 0.55).round(),
                            tile_s,
                            tex,
                            [1.0, 1.0, 1.0, s.opacity],
                        );
                        k += 1.0;
                    }
                }
                self.flush_atlas(gl, w, h);
                if icon_style == 0 {
                    for s in &vc.icon_states {
                        if s.opacity <= 0.01 || !s.has_torch {
                            continue;
                        }
                        let base_y = (s.y + s.h + 4.0 + s.bounce).round();
                        let count = (((s.w + 16.0) / tile_s).ceil()).max(3.0);
                        let total_w = count * tile_s;
                        let start_x = (s.x + s.w * 0.5 - total_w * 0.5).round();
                        self.draw_torch(
                            start_x + total_w - 8.0,
                            base_y - (tile_s * 0.55).round(),
                            t_glow * 0.75 * s.opacity,
                            night,
                            block,
                            sim_time,
                        );
                    }
                    self.flush_color(gl, w, h);
                    self.flush_glow(gl, w, h);
                }
            }
        }

        // 12. Cursor lantern glow at night
        if st.pointer_active && (night > 0.25 || biome_idx == 3) && t_glow > 0.05 {
            let pr = block * 3.0;
            let a = clamp_f((0.25 + night * 0.45) * t_glow, 0.0, 0.75);
            self.push_sprite_rect(
                st.pointer_x - pr,
                st.pointer_y - pr,
                pr * 2.0,
                pr * 2.0,
                [1.0, 1.0, 1.0, a],
            );
            self.flush_glow(gl, w, h);
        }

        // 13. Block-break debris & bouncy XP orbs
        for p in &vc.particles {
            let a = clamp_f(1.0 - p.life / p.max_life, 0.0, 1.0);
            self.add_rect(p.x.round(), p.y.round(), p.size, p.size, rgba(p.col, a));
        }
        self.flush_color(gl, w, h);
    }
}
