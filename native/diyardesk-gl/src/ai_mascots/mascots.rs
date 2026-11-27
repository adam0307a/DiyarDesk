use super::field::{clamp, lerp, CampusEnvironment, IconInfluence, Mulberry32};
use crate::quiet_snow::WindowInput;
use std::f64::consts::TAU;

// =========================================================================
// 1. 8 ORIGINAL AI MASCOT ARCHETYPES (No Brand Logos, Coherent Robot Family)
// =========================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MascotKind {
    General,    // Friendly white bot, mint-green accent, near terminal
    Coding,     // Dark compact bot, cyan eyes, floating code sparks
    Reasoning,  // Taller slate-indigo bot, purple accent, rotating floating geometry
    Vision,     // Articulated camera-head bot, amber-orange optical lens + scan cone
    Research,   // Floating capsule bot, blue-white glow + floating mini data panels
    Local,      // Sturdy compact bot, emerald accent, server-stack charging pulses
    Creative,   // Expressive plum-ceramic bot, rose-violet accent, morphing abstract shapes
    OpenSource, // Rugged mechanical maker bot, copper-orange & green accents, turning gear
}

impl MascotKind {
    pub const ALL: [MascotKind; 8] = [
        MascotKind::General,
        MascotKind::Coding,
        MascotKind::Reasoning,
        MascotKind::Vision,
        MascotKind::Research,
        MascotKind::Local,
        MascotKind::Creative,
        MascotKind::OpenSource,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MascotKind::General => "GENERAL AI",
            MascotKind::Coding => "CODING AI",
            MascotKind::Reasoning => "REASONING AI",
            MascotKind::Vision => "VISION AI",
            MascotKind::Research => "RESEARCH AI",
            MascotKind::Local => "LOCAL AI",
            MascotKind::Creative => "CREATIVE AI",
            MascotKind::OpenSource => "OPEN SOURCE",
        }
    }

    pub fn accent(self) -> [f32; 3] {
        match self {
            MascotKind::General => [0.43, 0.91, 0.72],    // soft mint-green
            MascotKind::Coding => [0.34, 0.80, 0.95],     // soft cyan
            MascotKind::Reasoning => [0.66, 0.55, 0.98],  // muted lavender-purple
            MascotKind::Vision => [0.96, 0.65, 0.30],     // warm amber-orange
            MascotKind::Research => [0.58, 0.78, 0.99],   // blue-white
            MascotKind::Local => [0.29, 0.87, 0.52],      // calm emerald
            MascotKind::Creative => [0.91, 0.55, 0.77],   // rose-violet
            MascotKind::OpenSource => [0.90, 0.58, 0.30], // copper-orange
        }
    }

    pub fn body_color(self) -> [f32; 3] {
        match self {
            MascotKind::General => [0.92, 0.94, 0.97],    // warm pearlescent white
            MascotKind::Coding => [0.16, 0.21, 0.29],     // dark compact charcoal-blue
            MascotKind::Reasoning => [0.21, 0.24, 0.35],  // slate-indigo
            MascotKind::Vision => [0.23, 0.26, 0.33],     // matte graphite
            MascotKind::Research => [0.85, 0.90, 0.96],   // silver-white
            MascotKind::Local => [0.16, 0.22, 0.25],      // deep steel teal
            MascotKind::Creative => [0.25, 0.21, 0.31],   // ceramic dusk plum
            MascotKind::OpenSource => [0.25, 0.24, 0.22], // brushed workshop iron
        }
    }

    pub fn home_building_idx(self) -> usize {
        match self {
            MascotKind::General | MascotKind::Research => 0,     // Central Atrium
            MascotKind::Coding => 1,                             // Code Forge
            MascotKind::Reasoning | MascotKind::Vision => 2,     // Observatory
            MascotKind::Local => 3,                              // Server Vault
            MascotKind::Creative | MascotKind::OpenSource => 4,  // Workshop Annex
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MascotBehavior {
    Idle,
    Walk,
    Work,
    LookAround,
    Interact, // Inspecting a desktop icon workstation or greeting another mascot
    Sleep,
    Charge,
}

#[derive(Clone, Debug)]
pub struct Mascot {
    pub kind: MascotKind,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub target_x: f64,
    pub target_y: f64,
    pub home_x: f64,
    pub home_y: f64,
    pub state: MascotBehavior,
    pub state_timer: f64,
    pub anim_phase: f64,
    pub facing: f64,        // -1.0 left .. +1.0 right
    pub look_angle: f64,    // head/eye aim angle
    pub wave_amount: f64,   // 0..1 greeting wave
    pub hover_dwell: f64,   // seconds cursor has lingered near this mascot
    pub label_alpha: f64,   // smooth 0..1 fade for hover-only category pill
    pub cursor_focus: f64,  // 0..1 attention toward mouse cursor
    pub target_station_id: Option<u64>,
    pub target_window_id: Option<i64>,
    pub speed: f64,
    pub is_floating: bool,
    pub scan_phase: f64,
    pub carry_orb: bool,
}

pub struct MascotManager {
    pub mascots: Vec<Mascot>,
    rng: Mulberry32,
    ai_tick_acc: f64,
}

impl Default for MascotManager {
    fn default() -> Self {
        Self {
            mascots: Vec::new(),
            rng: Mulberry32::new(0x4D41_5343),
            ai_tick_acc: 0.0,
        }
    }
}

impl MascotManager {
    pub fn spawn_all(&mut self, env: &CampusEnvironment, field: &IconInfluence, seed: u32) {
        self.rng = Mulberry32::new(seed ^ 0x4D41_5343);
        self.mascots.clear();
        let s = (env.w.min(env.h) / 1080.0).clamp(0.6, 1.6);

        // Initial curated positions across the research campus, pushed out of any icon safe zones
        let home_offsets: [(f64, f64); 8] = [
            (-52.0, 84.0),  // General: near Atrium desk
            (48.0, 68.0),   // Coding: near Code Forge holo desk
            (-46.0, 74.0),  // Reasoning: near Observatory pedestal
            (32.0, 92.0),   // Vision: Observatory courtyard lookout
            (58.0, 88.0),   // Research: Atrium data hub
            (66.0, 26.0),   // Local: beside Server Vault
            (-64.0, 36.0),  // Creative: Creative Annex garden
            (-26.0, 56.0),  // OpenSource: Workshop hardware bench
        ];

        for (i, &kind) in MascotKind::ALL.iter().enumerate() {
            let b = &env.buildings[kind.home_building_idx()];
            let (ox, oy) = home_offsets[i];
            let mut hx = clamp(b.x + ox * s, 68.0, env.w - 68.0);
            let mut hy = clamp(b.y + oy * s, 140.0, env.h - 56.0);

            // Never spawn on top of a desktop icon
            let safe = push_out_of_icons(hx, hy, field, 24.0, env.w, env.h);
            hx = safe.0;
            hy = safe.1;

            let initial_state = match i % 4 {
                0 => MascotBehavior::Work,
                1 => MascotBehavior::Idle,
                2 => MascotBehavior::LookAround,
                _ => if kind == MascotKind::Local { MascotBehavior::Charge } else { MascotBehavior::Idle },
            };

            self.mascots.push(Mascot {
                kind,
                x: hx,
                y: hy,
                vx: 0.0,
                vy: 0.0,
                target_x: hx,
                target_y: hy,
                home_x: hx,
                home_y: hy,
                state: initial_state,
                state_timer: self.rng.range(2.2, 6.8),
                anim_phase: self.rng.range(0.0, TAU),
                facing: if i % 2 == 0 { 1.0 } else { -1.0 },
                look_angle: 0.0,
                wave_amount: 0.0,
                hover_dwell: 0.0,
                label_alpha: 0.0,
                cursor_focus: 0.0,
                target_station_id: None,
                target_window_id: None,
                speed: match kind {
                    MascotKind::Research => 46.0 * s,
                    MascotKind::Coding | MascotKind::Vision => 38.0 * s,
                    MascotKind::Reasoning | MascotKind::Local => 28.0 * s,
                    _ => 34.0 * s,
                },
                is_floating: kind == MascotKind::Research,
                scan_phase: self.rng.range(0.0, TAU),
                carry_orb: false,
            });
        }
    }

    pub fn rescale_homes(&mut self, env: &CampusEnvironment, field: &IconInfluence) {
        if self.mascots.is_empty() {
            self.spawn_all(env, field, 2026);
            return;
        }
        let s = (env.w.min(env.h) / 1080.0).clamp(0.6, 1.6);
        let home_offsets: [(f64, f64); 8] = [
            (-52.0, 84.0),
            (48.0, 68.0),
            (-46.0, 74.0),
            (32.0, 92.0),
            (58.0, 88.0),
            (66.0, 26.0),
            (-64.0, 36.0),
            (-26.0, 56.0),
        ];
        for (i, m) in self.mascots.iter_mut().enumerate() {
            let b = &env.buildings[m.kind.home_building_idx()];
            let (ox, oy) = home_offsets[i % 8];
            let safe = push_out_of_icons(
                clamp(b.x + ox * s, 68.0, env.w - 68.0),
                clamp(b.y + oy * s, 140.0, env.h - 56.0),
                field,
                24.0,
                env.w,
                env.h,
            );
            m.home_x = safe.0;
            m.home_y = safe.1;
            let cur = push_out_of_icons(
                clamp(m.x, 48.0, env.w - 48.0),
                clamp(m.y, 120.0, env.h - 48.0),
                field,
                22.0,
                env.w,
                env.h,
            );
            m.x = cur.0;
            m.y = cur.1;
            m.target_x = clamp(m.target_x, 48.0, env.w - 48.0);
            m.target_y = clamp(m.target_y, 120.0, env.h - 48.0);
        }
    }

    pub fn step(
        &mut self,
        dt: f64,
        activity: f64,
        env: &CampusEnvironment,
        field: &mut IconInfluence,
        pointer_x: f64,
        pointer_y: f64,
        pointer_active: bool,
        windows: &[WindowInput],
    ) {
        let act = clamp(activity, 0.2, 2.0);
        self.ai_tick_acc += dt;
        let run_decisions = self.ai_tick_acc >= 0.20;
        let dec_dt = if run_decisions {
            let d = self.ai_tick_acc;
            self.ai_tick_acc = 0.0;
            d
        } else {
            0.0
        };

        let usable_windows: Vec<&WindowInput> = windows
            .iter()
            .filter(|w| {
                (w.w as f64) >= 140.0
                    && (w.h as f64) >= 80.0
                    && (w.y as f64) >= 44.0
                    && (w.y as f64) <= env.h - 72.0
                    && (w.w as f64) * (w.h as f64) <= env.w * env.h * 0.88
            })
            .collect();

        // Detect recently moved desktop icons -> 1-2 nearest mascots look toward the moved icon
        let mut moved_icon_pos: Option<(f64, f64)> = None;
        for st in &field.stations {
            if !st.removing && st.move_energy > 0.35 {
                moved_icon_pos = Some((st.cx, st.cy));
                break;
            }
        }
        let mut notified_count = 0usize;

        // Count currently walking mascots & window-ledge mascots so campus stays calm
        let mut walking_count = self
            .mascots
            .iter()
            .filter(|m| m.state == MascotBehavior::Walk)
            .count();
        let mut window_target_count = self
            .mascots
            .iter()
            .filter(|m| m.target_window_id.is_some())
            .count();

        // Snapshot positions for gentle mascot-to-mascot separation & social interaction
        let positions: Vec<(f64, f64)> = self.mascots.iter().map(|m| (m.x, m.y)).collect();

        for i in 0..self.mascots.len() {
            let m = &mut self.mascots[i];
            m.anim_phase = (m.anim_phase + dt * (1.8 + 0.6 * act)) % (TAU * 100.0);
            m.scan_phase = (m.scan_phase + dt * 1.15) % (TAU * 100.0);

            // Sync or clear active desktop window ledge target before steering
            if let Some(wid) = m.target_window_id {
                if let Some(win) = usable_windows.iter().copied().find(|w| w.id == wid) {
                    let ledge_y = clamp(win.y as f64 - 12.0, 44.0, env.h - 48.0);
                    let ledge_min_x = clamp(win.x as f64 + 28.0, 52.0, env.w - 52.0);
                    let ledge_max_x = clamp((win.x + win.w) as f64 - 28.0, ledge_min_x, env.w - 52.0);
                    m.target_x = clamp(m.target_x, ledge_min_x, ledge_max_x);
                    m.target_y = ledge_y;
                    if m.state != MascotBehavior::Walk
                        && ((m.y - ledge_y).abs() > 14.0 || m.x < ledge_min_x - 8.0 || m.x > ledge_max_x + 8.0)
                    {
                        m.state = MascotBehavior::Walk;
                        m.state_timer = 5.0;
                        walking_count += 1;
                    }
                } else {
                    m.target_window_id = None;
                    window_target_count = window_target_count.saturating_sub(1);
                    m.target_x = m.home_x;
                    m.target_y = m.home_y;
                    if m.state != MascotBehavior::Walk {
                        walking_count += 1;
                    }
                    m.state = MascotBehavior::Walk;
                    m.state_timer = 4.0;
                }
            }

            // Mouse interaction: notice cursor, wave gently, fade in minimal hover label
            let dist_cursor = if pointer_active {
                (pointer_x - m.x).hypot(pointer_y - m.y)
            } else {
                1e9
            };
            let cursor_near = pointer_active && dist_cursor < 105.0;
            let cursor_hover = pointer_active && dist_cursor < 56.0;

            let tgt_focus = if cursor_near { 1.0 - dist_cursor / 105.0 } else { 0.0 };
            m.cursor_focus = lerp(m.cursor_focus, tgt_focus, 1.0 - (-8.0 * dt).exp());

            if cursor_hover {
                m.hover_dwell = (m.hover_dwell + dt).min(2.0);
            } else {
                m.hover_dwell = (m.hover_dwell - dt * 2.5).max(0.0);
            }
            let tgt_label = if m.hover_dwell > 0.14 { 1.0 } else { 0.0 };
            m.label_alpha = lerp(m.label_alpha, tgt_label, 1.0 - (-9.0 * dt).exp());

            if cursor_near {
                let ang = (pointer_y - m.y).atan2(pointer_x - m.x);
                m.look_angle = lerp_angle(m.look_angle, ang, 1.0 - (-9.0 * dt).exp());
                if (pointer_x - m.x).abs() > 6.0 {
                    m.facing = lerp(m.facing, if pointer_x >= m.x { 1.0 } else { -1.0 }, 1.0 - (-8.0 * dt).exp());
                }
                let tgt_wave = if dist_cursor < 76.0 { 1.0 } else { 0.35 };
                m.wave_amount = lerp(m.wave_amount, tgt_wave, 1.0 - (-7.0 * dt).exp());
            } else {
                let base_wave = if m.state == MascotBehavior::Interact { 0.45 } else { 0.0 };
                m.wave_amount = lerp(m.wave_amount, base_wave, 1.0 - (-5.0 * dt).exp());
            }

            // Notice moved desktop icon (1-2 nearby mascots turn their head toward it)
            if !cursor_near {
                if let Some((mx, my)) = moved_icon_pos {
                    if notified_count < 2 && (mx - m.x).hypot(my - m.y) < 320.0 {
                        let ang = (my - m.y).atan2(mx - m.x);
                        m.look_angle = lerp_angle(m.look_angle, ang, 1.0 - (-7.0 * dt).exp());
                        m.facing = lerp(m.facing, if mx >= m.x { 1.0 } else { -1.0 }, 1.0 - (-6.0 * dt).exp());
                        notified_count += 1;
                    }
                }
            }

            // Low-frequency autonomous AI state decisions (~5 Hz)
            if run_decisions {
                m.state_timer -= dec_dt * act;
                if m.state_timer <= 0.0 {
                    let was_walking = m.state == MascotBehavior::Walk;
                    let roll = self.rng.next_f64();
                    let can_walk = walking_count < 4 || was_walking;

                    if can_walk && roll < 0.36 {
                        // Pick a calm destination: open window ledge (if visible), icon workstation rim, campus terminal, or home wing
                        let mut picked = false;
                        if !usable_windows.is_empty() && window_target_count < 2 && self.rng.next_f64() < 0.42 {
                            let wi = (self.rng.next_f64() * usable_windows.len() as f64).floor() as usize % usable_windows.len();
                            let win = usable_windows[wi];
                            let ledge_min_x = clamp(win.x as f64 + 28.0, 52.0, env.w - 52.0);
                            let ledge_max_x = clamp((win.x + win.w) as f64 - 28.0, ledge_min_x, env.w - 52.0);
                            let tx = lerp(ledge_min_x, ledge_max_x, self.rng.next_f64());
                            let ty = clamp(win.y as f64 - 12.0, 44.0, env.h - 48.0);
                            if m.target_window_id.is_none() {
                                window_target_count += 1;
                            }
                            m.target_x = tx;
                            m.target_y = ty;
                            m.target_station_id = None;
                            m.target_window_id = Some(win.id);
                            m.carry_orb = self.rng.next_f64() < 0.30;
                            picked = true;
                        }
                        if !picked && !field.stations.is_empty() && self.rng.next_f64() < 0.48 {
                            let active_sts: Vec<usize> = field
                                .stations
                                .iter()
                                .enumerate()
                                .filter_map(|(si, st)| if !st.removing && st.opacity > 0.4 { Some(si) } else { None })
                                .collect();
                            if !active_sts.is_empty() {
                                let pick_idx = active_sts[(self.rng.next_f64() * active_sts.len() as f64).floor() as usize % active_sts.len()];
                                let st = &field.stations[pick_idx];
                                let ang = (m.y - st.cy).atan2(m.x - st.cx) + self.rng.range(-0.65, 0.65);
                                let safe_r = st.pad_r + 22.0;
                                let tx = clamp(st.cx + ang.cos() * safe_r, 52.0, env.w - 52.0);
                                let ty = clamp(st.cy + ang.sin() * safe_r * 0.85, 130.0, env.h - 48.0);
                                if !field.is_inside_safe_zone(tx, ty, 12.0) {
                                    if m.target_window_id.take().is_some() {
                                        window_target_count = window_target_count.saturating_sub(1);
                                    }
                                    m.target_x = tx;
                                    m.target_y = ty;
                                    m.target_station_id = Some(st.id);
                                    m.carry_orb = self.rng.next_f64() < 0.35;
                                    picked = true;
                                }
                            }
                        }

                        if !picked {
                            let use_home = self.rng.next_f64() < 0.45 || env.terminals.is_empty();
                            let (raw_tx, raw_ty) = if use_home {
                                (
                                    m.home_x + self.rng.range(-72.0, 72.0),
                                    m.home_y + self.rng.range(-48.0, 48.0),
                                )
                            } else {
                                let ti = (self.rng.next_f64() * env.terminals.len() as f64).floor() as usize % env.terminals.len();
                                let tm = env.terminals[ti];
                                (
                                    tm.x + self.rng.range(-42.0, 42.0),
                                    tm.y + self.rng.range(16.0, 44.0),
                                )
                            };
                            let safe = push_out_of_icons(
                                clamp(raw_tx, 54.0, env.w - 54.0),
                                clamp(raw_ty, 135.0, env.h - 48.0),
                                field,
                                20.0,
                                env.w,
                                env.h,
                            );
                            if m.target_window_id.take().is_some() {
                                window_target_count = window_target_count.saturating_sub(1);
                            }
                            m.target_x = safe.0;
                            m.target_y = safe.1;
                            m.target_station_id = None;
                            m.carry_orb = self.rng.next_f64() < 0.25;
                        }

                        if !was_walking {
                            walking_count += 1;
                        }
                        m.state = MascotBehavior::Walk;
                        m.state_timer = self.rng.range(4.0, 9.0);
                    } else if roll < 0.60 {
                        if was_walking {
                            walking_count = walking_count.saturating_sub(1);
                        }
                        m.state = MascotBehavior::Work;
                        m.state_timer = self.rng.range(4.0, 8.5);
                        m.carry_orb = false;
                    } else if roll < 0.78 {
                        if was_walking {
                            walking_count = walking_count.saturating_sub(1);
                        }
                        m.state = MascotBehavior::LookAround;
                        m.state_timer = self.rng.range(2.8, 5.5);
                        m.carry_orb = false;
                    } else if roll < 0.90 && m.kind == MascotKind::Local {
                        if was_walking {
                            walking_count = walking_count.saturating_sub(1);
                        }
                        m.state = MascotBehavior::Charge;
                        m.state_timer = self.rng.range(4.5, 8.0);
                        m.carry_orb = false;
                    } else if roll < 0.94 {
                        if was_walking {
                            walking_count = walking_count.saturating_sub(1);
                        }
                        m.state = MascotBehavior::Idle;
                        m.state_timer = self.rng.range(3.0, 6.5);
                        m.carry_orb = false;
                    } else {
                        if was_walking {
                            walking_count = walking_count.saturating_sub(1);
                        }
                        m.state = MascotBehavior::Sleep;
                        m.state_timer = self.rng.range(4.5, 8.0);
                        m.carry_orb = false;
                    }
                }
            }

            // Continuous smooth movement + strict icon avoidance steering
            let mut desired_vx = 0.0;
            let mut desired_vy = 0.0;
            if m.state == MascotBehavior::Walk {
                let dx = m.target_x - m.x;
                let dy = m.target_y - m.y;
                let dist = dx.hypot(dy);
                if dist < 8.0 {
                    // Arrived! If at a desktop icon workstation, inspect it gently!
                    if let Some(sid) = m.target_station_id {
                        if let Some(st) = field.stations.iter_mut().find(|s| s.id == sid && !s.removing) {
                            m.state = MascotBehavior::Interact;
                            m.state_timer = self.rng.range(3.2, 5.5);
                            m.look_angle = (st.cy - m.y).atan2(st.cx - m.x);
                            m.facing = if st.cx >= m.x { 1.0 } else { -1.0 };
                            st.inspect_glow = 1.0;
                        } else {
                            m.state = MascotBehavior::Idle;
                            m.state_timer = self.rng.range(2.5, 5.0);
                        }
                    } else {
                        m.state = MascotBehavior::Work;
                        m.state_timer = self.rng.range(3.5, 7.0);
                    }
                    m.carry_orb = false;
                } else {
                    let spd = m.speed * act * clamp(dist / 36.0, 0.35, 1.0);
                    desired_vx = (dx / dist) * spd;
                    desired_vy = (dy / dist) * spd;
                }
            }

            // Soft icon repulsion force so mascots ALWAYS skirt around desktop icons
            for st in &field.stations {
                if st.opacity < 0.1 {
                    continue;
                }
                let dx = m.x - st.cx;
                let dy = m.y - st.cy;
                let dist = dx.hypot(dy);
                let safe_r = st.pad_r + 16.0;
                if dist < safe_r {
                    let nx = if dist > 0.5 { dx / dist } else { 1.0 };
                    let ny = if dist > 0.5 { dy / dist } else { 0.0 };
                    let push = (1.0 - dist / safe_r) * 145.0;
                    desired_vx += nx * push;
                    desired_vy += ny * push;
                }
            }

            // Gentle separation between mascots so they never overlap
            for (j, &(ox, oy)) in positions.iter().enumerate() {
                if j == i {
                    continue;
                }
                let dx = m.x - ox;
                let dy = m.y - oy;
                let dist = dx.hypot(dy);
                if dist < 44.0 && dist > 0.1 {
                    let push = (1.0 - dist / 44.0) * 42.0;
                    desired_vx += (dx / dist) * push;
                    desired_vy += (dy / dist) * push;
                }
            }

            let vel_k = 1.0 - (-6.5 * dt).exp();
            m.vx = lerp(m.vx, desired_vx, vel_k);
            m.vy = lerp(m.vy, desired_vy, vel_k);
            let min_y = if m.target_window_id.is_some() { 40.0 } else { env.h * 0.22 };
            m.x = clamp(m.x + m.vx * dt, env.w * 0.08, env.w * 0.92);
            m.y = clamp(m.y + m.vy * dt, min_y, env.h * 0.88);

            // Hard safety clamp: never end up directly on top of an icon center
            if field.is_inside_safe_zone(m.x, m.y, 4.0) {
                let safe = push_out_of_icons(m.x, m.y, field, 14.0, env.w, env.h);
                m.x = lerp(m.x, safe.0, 0.35);
                m.y = lerp(m.y, safe.1, 0.35);
            }

            if !cursor_near {
                if m.vx.hypot(m.vy) > 6.0 {
                    m.facing = lerp(m.facing, if m.vx >= 0.0 { 1.0 } else { -1.0 }, 1.0 - (-7.0 * dt).exp());
                    m.look_angle = lerp_angle(m.look_angle, m.vy.atan2(m.vx), 1.0 - (-5.0 * dt).exp());
                } else if m.state == MascotBehavior::LookAround {
                    let target_ang = (m.anim_phase * 0.45).sin() * 1.1;
                    m.look_angle = lerp_angle(m.look_angle, target_ang, 1.0 - (-3.5 * dt).exp());
                    m.facing = lerp(m.facing, if target_ang.cos() >= 0.0 { 1.0 } else { -1.0 }, 1.0 - (-3.0 * dt).exp());
                } else if m.state == MascotBehavior::Interact {
                    if let Some(sid) = m.target_station_id {
                        if let Some(st) = field.stations.iter_mut().find(|s| s.id == sid) {
                            st.inspect_glow = (st.inspect_glow + dt * 1.2).min(1.0);
                        }
                    }
                }
            }
        }
    }
}

fn push_out_of_icons(mut x: f64, mut y: f64, field: &IconInfluence, margin: f64, w: f64, h: f64) -> (f64, f64) {
    for _ in 0..3 {
        let mut pushed = false;
        for st in &field.stations {
            if st.opacity < 0.15 {
                continue;
            }
            let dx = x - st.cx;
            let dy = y - st.cy;
            let dist = dx.hypot(dy);
            let min_r = st.pad_r + margin;
            if dist < min_r {
                let nx = if dist > 0.5 { dx / dist } else { 1.0 };
                let ny = if dist > 0.5 { dy / dist } else { 0.0 };
                x = clamp(st.cx + nx * (min_r + 2.0), 42.0, w - 42.0);
                y = clamp(st.cy + ny * (min_r + 2.0), 120.0, h - 42.0);
                pushed = true;
            }
        }
        if !pushed {
            break;
        }
    }
    (x, y)
}

fn lerp_angle(a: f64, b: f64, t: f64) -> f64 {
    let mut diff = (b - a) % TAU;
    if diff > std::f64::consts::PI {
        diff -= TAU;
    } else if diff < -std::f64::consts::PI {
        diff += TAU;
    }
    a + diff * t
}
