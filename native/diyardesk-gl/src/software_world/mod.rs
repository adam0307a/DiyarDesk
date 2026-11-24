pub mod shaders;

use crate::gl::*;
use crate::state::{AppState, Rect};
use crate::text::TextRenderer;
use shaders::*;
use std::collections::{HashMap, HashSet};
use std::f32::consts::{PI, TAU};
use std::ffi::c_void;

// =========================================================================
// Deterministic Fast PRNG (xorshift32 — bit-for-bit identical to JS)
// =========================================================================
#[derive(Clone, Copy)]
pub struct Xorshift32 {
    state: i32,
}

impl Xorshift32 {
    pub fn from_seed(seed: i32) -> Self {
        let mut s = Self { state: 0x2F6E2B1 };
        s.reseed(seed);
        s
    }

    pub fn reseed(&mut self, seed: i32) {
        let v = seed ^ (0x9E3779B9u32 as i32);
        self.state = if v == 0 { 0x2F6E2B1 } else { v };
    }

    pub fn next_f64(&mut self) -> f64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= ((x as u32) >> 17) as i32;
        x ^= x << 5;
        self.state = x;
        (x as u32) as f64 / 4294967296.0
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }
}

// =========================================================================
// Palette & Software Domain Vocabulary (1:1 prototype/software-world.html)
// =========================================================================
pub const ACCENT_RGB: [[u8; 3]; 4] = [
    [96, 165, 250],  // 0: blue   (application / api)
    [74, 222, 128],  // 1: green  (service / server)
    [217, 160, 75],  // 2: amber  (package / container)
    [155, 135, 215], // 3: purple (module / repository)
];

#[derive(Clone, Copy)]
pub struct NodeRole {
    pub tag: &'static str,
    pub accent_idx: usize,
    pub badge: &'static str,
}

pub const ROLES: [NodeRole; 7] = [
    NodeRole { tag: "APP", accent_idx: 0, badge: "<>" },
    NodeRole { tag: "SVC", accent_idx: 1, badge: "::" },
    NodeRole { tag: "PKG", accent_idx: 2, badge: "{}" },
    NodeRole { tag: "REPO", accent_idx: 3, badge: "git" },
    NodeRole { tag: "SRV", accent_idx: 1, badge: "$" },
    NodeRole { tag: "CTR", accent_idx: 2, badge: "[]" },
    NodeRole { tag: "MOD", accent_idx: 3, badge: "λ" },
];

pub const LINK_PROTOS: [&str; 6] = ["HTTP", "RPC", "SQL", "EVT", "IPC", "TCP"];
pub const CODE_GLYPHS: [&str; 11] = ["{}", "<>", "()", ";", "=>", "::", "./", "~/", "λ", "#", "$"];
pub const LOG_LINES: [&str; 8] = [
    "$ build  compiling modules...",
    "✓ module  linked (14ms)",
    "→ deploy  worker-pool ready",
    "[ok] 200  GET /api/v2/health",
    "git: rebase origin/main clean",
    "cache: hit ratio 98.4%",
    "rpc: stream channel open",
    "✓ tests  42 passed, 0 failed",
];

const MAX_PACKETS: usize = 120;
const MAX_CODE_PARTICLES: usize = 84;
const MAX_AMBIENT_NODES: usize = 60;
const MATCH_RADIUS: f32 = 210.0;

#[inline]
fn rgba_u8(c: [u8; 3], a: f32) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        a.clamp(0.0, 1.0),
    ]
}

// =========================================================================
// Parameters, Lifecycle State Machine & Object Pools
// =========================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct SoftwareWorldParams {
    pub adaptive: i32,
    pub anim_speed: f32,
    pub network_activity: f32,
    pub connection_intensity: f32,
    pub particle_density: f32,
    pub code_particle_density: f32,
    pub env_detail: f32,
    pub quality: usize,
    pub seed: i32,
}

impl Default for SoftwareWorldParams {
    fn default() -> Self {
        Self {
            adaptive: 1,
            anim_speed: 1.0,
            network_activity: 0.65,
            connection_intensity: 0.55,
            particle_density: 0.55,
            code_particle_density: 0.45,
            env_detail: 0.70,
            quality: 1,
            seed: 42,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeLifecycle {
    Boot,
    Initialize,
    Connect,
    Running,
    Disconnect,
    Shutdown,
    Offline,
}

impl NodeLifecycle {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeLifecycle::Boot => "boot",
            NodeLifecycle::Initialize => "initialize",
            NodeLifecycle::Connect => "connect",
            NodeLifecycle::Running => "running",
            NodeLifecycle::Disconnect => "disconnect",
            NodeLifecycle::Shutdown => "shutdown",
            NodeLifecycle::Offline => "offline",
        }
    }
}

#[derive(Clone)]
pub struct SoftwareNode {
    pub id: u32,
    pub role_idx: usize,
    pub x: f32,
    pub y: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub w: f32,
    pub h: f32,
    pub target_w: f32,
    pub target_h: f32,
    pub state: NodeLifecycle,
    pub state_time: f32,
    pub alpha: f32,
    pub scale: f32,
    pub pulse_phase: f32,
    pub build_timer: f32,
    pub build_dur: f32,
}

#[derive(Clone, Copy)]
pub struct NodeLink {
    pub a_id: u32,
    pub b_id: u32,
    pub proto_idx: usize,
    pub curve_dir: f32,
}

#[derive(Clone, Copy)]
pub struct DataPacket {
    pub active: bool,
    pub link_idx: usize,
    pub t: f32,
    pub speed: f32,
    pub dir: f32,
    pub accent_idx: usize,
    pub ambient_y: f32,
    pub is_ambient: bool,
}

#[derive(Clone, Copy)]
pub struct CodeParticle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub glyph_idx: usize,
    pub alpha: f32,
    pub phase: f32,
    pub accent_idx: usize,
}

#[derive(Clone, Copy)]
pub struct AmbientNode {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub r: f32,
    pub phase: f32,
}

#[derive(Clone, Copy)]
pub struct GitCommit {
    pub lane: usize,
    pub x_norm: f32,
    pub accent_idx: usize,
}

pub struct SoftwareWorldState {
    pub p: SoftwareWorldParams,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub rng: Xorshift32,

    pub next_node_id: u32,
    pub nodes: Vec<SoftwareNode>, // preserves insertion order just like JS Map -> nodeList
    pub links: Vec<NodeLink>,
    pub links_dirty: bool,

    pub packet_pool: Vec<DataPacket>,
    pub code_particles: Vec<CodeParticle>,
    pub ambient_nodes: Vec<AmbientNode>,
    pub git_commits: Vec<GitCommit>,
    pub git_scroll: f32,

    pub log_cursor: usize,
    pub log_timer: f32,
    pub build_trigger_timer: f32,
    pub packet_spawn_accum: f32,
}

impl Default for SoftwareWorldState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0, SoftwareWorldParams::default())
    }
}

impl SoftwareWorldState {
    pub fn new(w: f32, h: f32, p: SoftwareWorldParams) -> Self {
        let mut rng = Xorshift32::from_seed(p.seed);
        let packet_pool = vec![
            DataPacket {
                active: false,
                link_idx: 0,
                t: 0.0,
                speed: 0.5,
                dir: 1.0,
                accent_idx: 0,
                ambient_y: 0.0,
                is_ambient: false,
            };
            MAX_PACKETS
        ];

        let mut code_particles = Vec::with_capacity(MAX_CODE_PARTICLES);
        for ci in 0..MAX_CODE_PARTICLES {
            let x = rng.next_f32();
            let y = rng.next_f32();
            let vx = (rng.next_f32() - 0.5) * 0.008;
            let vy = -0.004 - rng.next_f32() * 0.008;
            let alpha = 0.05 + rng.next_f32() * 0.08;
            let phase = rng.next_f32() * TAU;
            code_particles.push(CodeParticle {
                x,
                y,
                vx,
                vy,
                glyph_idx: ci % CODE_GLYPHS.len(),
                alpha,
                phase,
                accent_idx: ci % 4,
            });
        }

        let mut ambient_nodes = Vec::with_capacity(MAX_AMBIENT_NODES);
        for _ in 0..MAX_AMBIENT_NODES {
            let x = rng.next_f32();
            let y = rng.next_f32();
            let vx = (rng.next_f32() - 0.5) * 0.006;
            let vy = (rng.next_f32() - 0.5) * 0.006;
            let r = 1.1 + rng.next_f32() * 1.1;
            let phase = rng.next_f32() * TAU;
            ambient_nodes.push(AmbientNode {
                x,
                y,
                vx,
                vy,
                r,
                phase,
            });
        }

        let mut git_commits = Vec::with_capacity(14);
        for gi in 0..14 {
            let lane = if gi % 3 == 0 {
                1
            } else if gi % 5 == 0 {
                2
            } else {
                0
            };
            git_commits.push(GitCommit {
                lane,
                x_norm: gi as f32 / 13.0,
                accent_idx: gi % 4,
            });
        }

        let w = w.max(320.0);
        let h = h.max(240.0);
        let scale = (w.min(h) / 1080.0).clamp(0.72, 1.85);

        Self {
            p,
            w,
            h,
            scale,
            rng,
            next_node_id: 1,
            nodes: Vec::new(),
            links: Vec::new(),
            links_dirty: true,
            packet_pool,
            code_particles,
            ambient_nodes,
            git_commits,
            git_scroll: 0.0,
            log_cursor: 0,
            log_timer: 0.0,
            build_trigger_timer: 4.0,
            packet_spawn_accum: 0.0,
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        let nw = w.max(320.0);
        let nh = h.max(240.0);
        if (self.w - nw).abs() > 0.5 || (self.h - nh).abs() > 0.5 {
            self.w = nw;
            self.h = nh;
            self.scale = (nw.min(nh) / 1080.0).clamp(0.72, 1.85);
            self.links_dirty = true;
        }
    }

    pub fn sync_params(&mut self, next: SoftwareWorldParams) {
        if next.seed != self.p.seed {
            self.rng.reseed(next.seed);
        }
        let adaptive_off = self.p.adaptive != 0 && next.adaptive == 0;
        self.p = next;
        if adaptive_off {
            self.update_icons(&[], false);
        }
    }

    pub fn update_icons(&mut self, icons: &[Rect], vis: bool) {
        if !vis || self.p.adaptive == 0 {
            for n in &mut self.nodes {
                if !matches!(
                    n.state,
                    NodeLifecycle::Disconnect | NodeLifecycle::Shutdown | NodeLifecycle::Offline
                ) {
                    n.state = NodeLifecycle::Disconnect;
                    n.state_time = 0.0;
                }
            }
            self.links_dirty = true;
            return;
        }

        let incoming: Vec<(f32, f32, f32, f32)> = icons
            .iter()
            .map(|ic| {
                let w = ic.w.max(24.0);
                let h = ic.h.max(24.0);
                (ic.x + w * 0.5, ic.y + h * 0.5, w, h)
            })
            .collect();

        // Spatial nearest-neighbor matching (independent of array order)
        let existing_indices: Vec<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| !matches!(n.state, NodeLifecycle::Shutdown | NodeLifecycle::Offline))
            .map(|(idx, _)| idx)
            .collect();

        let mut pairs: Vec<(usize, usize, f32)> = Vec::new();
        let rad2 = MATCH_RADIUS * MATCH_RADIUS;
        for &ei in &existing_indices {
            let en = &self.nodes[ei];
            for (ii, &(icx, icy, _, _)) in incoming.iter().enumerate() {
                let dx = en.target_x - icx;
                let dy = en.target_y - icy;
                let d2 = dx * dx + dy * dy;
                if d2 <= rad2 {
                    pairs.push((ei, ii, d2));
                }
            }
        }
        pairs.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        let mut matched_node_ids: HashSet<u32> = HashSet::new();
        let mut matched_incoming: HashSet<usize> = HashSet::new();

        for (ei, ii, _) in pairs {
            let nid = self.nodes[ei].id;
            if matched_node_ids.contains(&nid) || matched_incoming.contains(&ii) {
                continue;
            }
            matched_node_ids.insert(nid);
            matched_incoming.insert(ii);
            let (cx, cy, w, h) = incoming[ii];
            let node = &mut self.nodes[ei];
            node.target_x = cx;
            node.target_y = cy;
            node.target_w = w;
            node.target_h = h;
            if node.state == NodeLifecycle::Disconnect {
                node.state = NodeLifecycle::Connect;
                node.state_time = 0.0;
            }
        }

        // Unmatched existing nodes -> begin disconnect/shutdown lifecycle
        for &ei in &existing_indices {
            let node = &mut self.nodes[ei];
            if !matched_node_ids.contains(&node.id) && node.state != NodeLifecycle::Disconnect {
                node.state = NodeLifecycle::Disconnect;
                node.state_time = 0.0;
            }
        }

        // Unmatched incoming icons -> spawn new software nodes
        for (ii, &(cx, cy, w, h)) in incoming.iter().enumerate() {
            if matched_incoming.contains(&ii) {
                continue;
            }
            let id = self.next_node_id;
            self.next_node_id += 1;
            let role_idx = ((id - 1) as usize) % ROLES.len();
            let pulse_phase = self.rng.next_f32() * TAU;
            self.nodes.push(SoftwareNode {
                id,
                role_idx,
                x: cx,
                y: cy,
                target_x: cx,
                target_y: cy,
                w,
                h,
                target_w: w,
                target_h: h,
                state: NodeLifecycle::Boot,
                state_time: 0.0,
                alpha: 0.01,
                scale: 0.72,
                pulse_phase,
                build_timer: -1.0,
                build_dur: 4.5,
            });
        }

        self.links_dirty = true;
    }

    pub fn rebuild_links(&mut self) {
        self.links.clear();
        let active: Vec<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                matches!(
                    n.state,
                    NodeLifecycle::Connect | NodeLifecycle::Running | NodeLifecycle::Disconnect
                )
            })
            .map(|(i, _)| i)
            .collect();
        if active.len() < 2 {
            self.links_dirty = false;
            return;
        }

        let max_dist = self.w.max(self.h) * 0.42;
        let max_dist2 = max_dist * max_dist;
        let mut edge_keys: HashSet<(u32, u32)> = HashSet::new();

        for &ai in &active {
            let na = &self.nodes[ai];
            let mut neighbors: Vec<(u32, f32)> = Vec::new();
            for &bi in &active {
                if ai == bi {
                    continue;
                }
                let nb = &self.nodes[bi];
                let dx = na.x - nb.x;
                let dy = na.y - nb.y;
                let d2 = dx * dx + dy * dy;
                if d2 <= max_dist2 {
                    neighbors.push((nb.id, d2));
                }
            }
            neighbors.sort_by(|u, v| u.1.partial_cmp(&v.1).unwrap_or(std::cmp::Ordering::Equal));
            for &(target_id, _) in neighbors.iter().take(3) {
                let lo = na.id.min(target_id);
                let hi = na.id.max(target_id);
                if !edge_keys.insert((lo, hi)) {
                    continue;
                }
                let sum = (lo + hi) as usize;
                self.links.push(NodeLink {
                    a_id: na.id,
                    b_id: target_id,
                    proto_idx: sum % LINK_PROTOS.len(),
                    curve_dir: if sum % 2 == 0 { 1.0 } else { -1.0 },
                });
            }
        }
        self.links_dirty = false;
    }

    pub fn step(&mut self, dt: f32) {
        let speed = self.p.anim_speed;
        let s_dt = dt * speed;
        let interp_alpha = 1.0 - (-11.0 * dt).exp();
        let mut needs_cleanup = false;

        for n in &mut self.nodes {
            n.x += (n.target_x - n.x) * interp_alpha;
            n.y += (n.target_y - n.y) * interp_alpha;
            n.w += (n.target_w - n.w) * interp_alpha;
            n.h += (n.target_h - n.h) * interp_alpha;
            n.pulse_phase += s_dt * 2.1;
            n.state_time += dt;

            match n.state {
                NodeLifecycle::Boot => {
                    let bt = (n.state_time / 0.4).clamp(0.0, 1.0);
                    n.alpha = bt * 0.7;
                    n.scale = 0.72 + bt * 0.22;
                    if n.state_time >= 0.4 {
                        n.state = NodeLifecycle::Initialize;
                        n.state_time = 0.0;
                    }
                }
                NodeLifecycle::Initialize => {
                    let it = (n.state_time / 0.5).clamp(0.0, 1.0);
                    n.alpha = 0.7 + it * 0.2;
                    n.scale = 0.94 + it * 0.06;
                    if n.state_time >= 0.5 {
                        n.state = NodeLifecycle::Connect;
                        n.state_time = 0.0;
                        self.links_dirty = true;
                    }
                }
                NodeLifecycle::Connect => {
                    let ct = (n.state_time / 0.5).clamp(0.0, 1.0);
                    n.alpha = 0.9 + ct * 0.1;
                    n.scale = 1.0;
                    if n.state_time >= 0.5 {
                        n.state = NodeLifecycle::Running;
                        n.state_time = 0.0;
                    }
                }
                NodeLifecycle::Running => {
                    n.alpha = 1.0;
                    n.scale = 1.0;
                    if n.build_timer >= 0.0 {
                        n.build_timer += s_dt;
                        if n.build_timer >= n.build_dur {
                            n.build_timer = -1.0;
                        }
                    }
                }
                NodeLifecycle::Disconnect => {
                    let ddt = (n.state_time / 0.35).clamp(0.0, 1.0);
                    n.alpha = 1.0 - ddt * 0.35;
                    if n.state_time >= 0.35 {
                        n.state = NodeLifecycle::Shutdown;
                        n.state_time = 0.0;
                        self.links_dirty = true;
                    }
                }
                NodeLifecycle::Shutdown => {
                    let st = (n.state_time / 0.5).clamp(0.0, 1.0);
                    n.alpha = (1.0 - st) * 0.65;
                    n.scale = 1.0 - st * 0.3;
                    if n.state_time >= 0.5 {
                        n.state = NodeLifecycle::Offline;
                        needs_cleanup = true;
                    }
                }
                NodeLifecycle::Offline => {}
            }
        }
        if needs_cleanup {
            self.nodes.retain(|n| n.state != NodeLifecycle::Offline);
            self.links_dirty = true;
        }
        if self.links_dirty {
            self.rebuild_links();
        }

        // Occasional calm build/compile cycle on one running node
        if self.p.env_detail > 0.25 && !self.nodes.is_empty() {
            self.build_trigger_timer -= s_dt;
            if self.build_trigger_timer <= 0.0 {
                self.build_trigger_timer = 7.5 + self.rng.next_f32() * 7.0;
                let pick_idx =
                    ((self.rng.next_f32() * self.nodes.len() as f32) as usize).min(self.nodes.len() - 1);
                let cand = &mut self.nodes[pick_idx];
                if cand.state == NodeLifecycle::Running && cand.build_timer < 0.0 {
                    cand.build_timer = 0.0;
                }
            }
        }

        // Spawn & advance data packets (object-pooled)
        let net_rate = self.p.network_activity * if self.p.quality == 0 { 0.5 } else { 1.0 };
        if net_rate > 0.02 {
            let link_len = self.links.len();
            let target_rate = (if link_len > 0 {
                2.2 + link_len as f32 * 1.1
            } else {
                1.6
            }) * net_rate
                * speed;
            self.packet_spawn_accum += dt * target_rate;
            while self.packet_spawn_accum >= 1.0 {
                self.packet_spawn_accum -= 1.0;
                if let Some(p_idx) = self.packet_pool.iter().position(|pk| !pk.active) {
                    let pkt_speed = (0.45 + self.rng.next_f32() * 0.65) * speed;
                    let dir = if self.rng.next_f32() > 0.5 { 1.0 } else { -1.0 };
                    let (is_ambient, link_idx, accent_idx, ambient_y, final_speed) =
                        if link_len > 0 && self.rng.next_f32() > 0.18 {
                            let l_idx = ((self.rng.next_f32() * link_len as f32) as usize)
                                .min(link_len - 1);
                            let a_id = self.links[l_idx].a_id;
                            let acc = self
                                .nodes
                                .iter()
                                .find(|n| n.id == a_id)
                                .map(|n| ROLES[n.role_idx].accent_idx)
                                .unwrap_or(0);
                            (false, l_idx, acc, 0.0, pkt_speed)
                        } else {
                            let ay = 0.14 + self.rng.next_f32() * 0.72;
                            let acc = ((self.rng.next_f32() * 4.0) as usize).min(3);
                            (true, 0, acc, ay, pkt_speed * 0.45)
                        };
                    self.packet_pool[p_idx] = DataPacket {
                        active: true,
                        link_idx,
                        t: 0.0,
                        speed: final_speed,
                        dir,
                        accent_idx,
                        ambient_y,
                        is_ambient,
                    };
                }
            }
        }

        for pk in &mut self.packet_pool {
            if !pk.active {
                continue;
            }
            pk.t += dt * pk.speed;
            if pk.t >= 1.0 {
                pk.active = false;
            }
        }

        // Drift code particles & ambient nodes
        let c_count = ((MAX_CODE_PARTICLES as f32)
            * self.p.code_particle_density
            * if self.p.quality == 0 { 0.5 } else { 1.0 })
        .round() as usize;
        for c in 0..c_count.min(MAX_CODE_PARTICLES) {
            let mut cp = self.code_particles[c];
            cp.x += cp.vx * s_dt;
            cp.y += cp.vy * s_dt;
            cp.phase += s_dt * 0.8;
            if cp.y < -0.04 {
                cp.y = 1.04;
                cp.x = self.rng.next_f32();
            }
            if cp.x < -0.04 {
                cp.x = 1.04;
            } else if cp.x > 1.04 {
                cp.x = -0.04;
            }
            self.code_particles[c] = cp;
        }

        let a_count = ((MAX_AMBIENT_NODES as f32)
            * self.p.particle_density
            * if self.p.quality == 0 { 0.5 } else { 1.0 })
        .round() as usize;
        for a in 0..a_count.min(MAX_AMBIENT_NODES) {
            let an = &mut self.ambient_nodes[a];
            an.x += an.vx * s_dt;
            an.y += an.vy * s_dt;
            an.phase += s_dt * 1.4;
            if an.x < 0.0 {
                an.x += 1.0;
            } else if an.x > 1.0 {
                an.x -= 1.0;
            }
            if an.y < 0.0 {
                an.y += 1.0;
            } else if an.y > 1.0 {
                an.y -= 1.0;
            }
        }

        // Git branch drift & terminal log cycle
        self.git_scroll = (self.git_scroll + s_dt * 0.018) % 1.0;
        self.log_timer += s_dt;
        if self.log_timer >= 4.2 {
            self.log_timer = 0.0;
            self.log_cursor = (self.log_cursor + 1) % LOG_LINES.len();
        }
    }
}

// =========================================================================
// OpenGL 3.3 Core Batched Vector & Monospace Text Renderer for Software World
// =========================================================================
#[repr(C)]
#[derive(Clone, Copy)]
struct ColorVert {
    pos: [f32; 2],
    col: [f32; 4],
}

struct VecBatch {
    verts: Vec<ColorVert>,
}

impl VecBatch {
    fn new() -> Self {
        Self {
            verts: Vec::with_capacity(16384),
        }
    }

    fn clear(&mut self) {
        self.verts.clear();
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

    fn fill_rect_vgrad(&mut self, x: f32, y: f32, w: f32, h: f32, top_col: [f32; 4], bot_col: [f32; 4]) {
        self.push_quad(
            ColorVert { pos: [x, y], col: top_col },
            ColorVert { pos: [x + w, y], col: top_col },
            ColorVert { pos: [x + w, y + h], col: bot_col },
            ColorVert { pos: [x, y + h], col: bot_col },
        );
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, segs: usize, col: [f32; 4]) {
        if col[3] <= 0.001 || r <= 0.05 {
            return;
        }
        let center = ColorVert { pos: [cx, cy], col };
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * TAU;
            self.push_tri(
                center,
                ColorVert { pos: [cx + a0.cos() * r, cy + a0.sin() * r], col },
                ColorVert { pos: [cx + a1.cos() * r, cy + a1.sin() * r], col },
            );
        }
    }

    fn stroke_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, col: [f32; 4]) {
        if col[3] <= 0.001 || width <= 0.05 {
            return;
        }
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = dx.hypot(dy);
        if len < 1e-4 {
            return;
        }
        let hw = width * 0.5;
        let nx = -dy / len * hw;
        let ny = dx / len * hw;
        self.push_quad(
            ColorVert { pos: [x0 + nx, y0 + ny], col },
            ColorVert { pos: [x0 - nx, y0 - ny], col },
            ColorVert { pos: [x1 - nx, y1 - ny], col },
            ColorVert { pos: [x1 + nx, y1 + ny], col },
        );
    }

    fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, col: [f32; 4]) {
        self.stroke_line(x, y, x + w, y, width, col);
        self.stroke_line(x + w, y, x + w, y + h, width, col);
        self.stroke_line(x + w, y + h, x, y + h, width, col);
        self.stroke_line(x, y + h, x, y, width, col);
    }

    fn stroke_quad_bezier(
        &mut self,
        x0: f32,
        y0: f32,
        cx: f32,
        cy: f32,
        x1: f32,
        y1: f32,
        width: f32,
        steps: usize,
        col: [f32; 4],
    ) {
        let mut px = x0;
        let mut py = y0;
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let it = 1.0 - t;
            let nx = it * it * x0 + 2.0 * it * t * cx + t * t * x1;
            let ny = it * it * y0 + 2.0 * it * t * cy + t * t * y1;
            self.stroke_line(px, py, nx, ny, width, col);
            px = nx;
            py = ny;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn stroke_cubic_bezier(
        &mut self,
        x0: f32,
        y0: f32,
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x1: f32,
        y1: f32,
        width: f32,
        steps: usize,
        col: [f32; 4],
    ) {
        let mut px = x0;
        let mut py = y0;
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let it = 1.0 - t;
            let nx = it * it * it * x0
                + 3.0 * it * it * t * c1x
                + 3.0 * it * t * t * c2x
                + t * t * t * x1;
            let ny = it * it * it * y0
                + 3.0 * it * it * t * c1y
                + 3.0 * it * t * t * c2y
                + t * t * t * y1;
            self.stroke_line(px, py, nx, ny, width, col);
            px = nx;
            py = ny;
        }
    }
}

pub struct SoftwareWorldRenderer {
    prog_color: GLuint,
    dummy_vao: GLuint,
    vao: GLuint,
    vbo: GLuint,
    batch: VecBatch,
    mono_text: TextRenderer,

    bg_fbo: GLuint,
    bg_tex: GLuint,
    bg_w: i32,
    bg_h: i32,
    bg_sig: (i32, i32, i32, i32, i32),
}

impl SoftwareWorldRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_color = gl.compile_program(SW_COLOR_VERT, SW_COLOR_FRAG)?;
        let mono_text = TextRenderer::new_monospace(gl)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let mut vao = 0;
        let mut vbo = 0;
        (gl.GenVertexArrays)(1, &mut vao);
        (gl.GenBuffers)(1, &mut vbo);
        (gl.BindVertexArray)(vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, vbo);
        let stride = std::mem::size_of::<ColorVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, stride, 8 as *const c_void);
        (gl.BindVertexArray)(0);

        Ok(Self {
            prog_color,
            dummy_vao,
            vao,
            vbo,
            batch: VecBatch::new(),
            mono_text,
            bg_fbo: 0,
            bg_tex: 0,
            bg_w: 0,
            bg_h: 0,
            bg_sig: (0, 0, 0, 0, -1),
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
        self.mono_text.destroy(gl);
        if self.vbo != 0 {
            (gl.DeleteBuffers)(1, &self.vbo);
            self.vbo = 0;
        }
        if self.vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.vao);
            self.vao = 0;
        }
        if self.dummy_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.dummy_vao);
            self.dummy_vao = 0;
        }
        if self.prog_color != 0 {
            (gl.DeleteProgram)(self.prog_color);
            self.prog_color = 0;
        }
    }

    unsafe fn flush_batch(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.batch.verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog_color);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_color, "u_res"), css_w, css_h);
        (gl.BindVertexArray)(self.vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.vbo);
        let bytes = (self.batch.verts.len() * std::mem::size_of::<ColorVert>()) as GLsizeiptr;
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            bytes,
            self.batch.verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.batch.verts.len() as GLsizei);
        self.batch.clear();
    }

    unsafe fn ensure_bg_fbo(&mut self, gl: &GlFns, pw: i32, ph: i32, sw: &SoftwareWorldState) {
        let pw = pw.max(64);
        let ph = ph.max(64);
        if self.bg_fbo == 0 || self.bg_w != pw || self.bg_h != ph {
            if self.bg_tex != 0 {
                (gl.DeleteTextures)(1, &self.bg_tex);
            }
            if self.bg_fbo != 0 {
                (gl.DeleteFramebuffers)(1, &self.bg_fbo);
            }
            (gl.GenFramebuffers)(1, &mut self.bg_fbo);
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
            self.bg_sig = (0, 0, 0, 0, -1);
        }

        let sig = (
            pw,
            ph,
            (sw.w * 10.0) as i32,
            (sw.h * 10.0) as i32,
            (sw.p.env_detail * 1000.0).round() as i32,
        );
        if sig == self.bg_sig {
            return;
        }
        self.bg_sig = sig;

        let w = sw.w;
        let h = sw.h;
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.bg_fbo);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(
            GL_SRC_ALPHA,
            GL_ONE_MINUS_SRC_ALPHA,
            GL_ONE,
            GL_ONE_MINUS_SRC_ALPHA,
        );

        self.batch.clear();
        // #0a0d13 -> #0f141d vertical gradient
        self.batch.fill_rect_vgrad(
            0.0,
            0.0,
            w,
            h,
            [10.0 / 255.0, 13.0 / 255.0, 19.0 / 255.0, 1.0],
            [15.0 / 255.0, 20.0 / 255.0, 29.0 / 255.0, 1.0],
        );

        if sw.p.env_detail >= 0.05 {
            let step = (48.0 * sw.scale).round().max(28.0);
            let grid_col = [130.0 / 255.0, 160.0 / 255.0, 200.0 / 255.0, 0.088];
            let mut x = step * 0.5;
            while x < w {
                let px = x.floor() + 0.5;
                self.batch.stroke_line(px, 0.0, px, h, 1.0, grid_col);
                x += step;
            }
            let mut y = step * 0.5;
            while y < h {
                let py = y.floor() + 0.5;
                self.batch.stroke_line(0.0, py, w, py, 1.0, grid_col);
                y += step;
            }

            if sw.p.env_detail >= 0.35 {
                let cross_col = [140.0 / 255.0, 178.0 / 255.0, 226.0 / 255.0, 0.22];
                let major = step * 4.0;
                let mut gx = step * 0.5;
                while gx < w {
                    let mut gy = step * 0.5;
                    while gy < h {
                        let cx = gx.floor() + 0.5;
                        let cy = gy.floor() + 0.5;
                        self.batch.stroke_line(cx - 3.0, cy, cx + 3.0, cy, 1.0, cross_col);
                        self.batch.stroke_line(cx, cy - 3.0, cx, cy + 3.0, 1.0, cross_col);
                        gy += major;
                    }
                    gx += major;
                }
            }
        }
        self.flush_batch(gl, w, h);
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState, prog_blit: GLuint) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale.max(0.5);

        st.software_world.resize(css_w, css_h);
        self.ensure_bg_fbo(gl, pw, ph, &st.software_world);

        // 10a. Blit cached static background
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
        (gl.BlendFuncSeparate)(
            GL_SRC_ALPHA,
            GL_ONE_MINUS_SRC_ALPHA,
            GL_ONE,
            GL_ONE_MINUS_SRC_ALPHA,
        );

        let sw = &st.software_world;
        let w = sw.w;
        let h = sw.h;
        let scale = sw.scale;

        // Build fast id -> index lookup for links & packets
        let node_map: HashMap<u32, usize> = sw
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id, i))
            .collect();

        self.batch.clear();
        self.mono_text.begin();

        // 10b. Ambient background mesh particles
        let a_count = ((MAX_AMBIENT_NODES as f32)
            * sw.p.particle_density
            * if sw.p.quality == 0 { 0.5 } else { 1.0 })
        .round() as usize;
        let amb_col = [148.0 / 255.0, 182.0 / 255.0, 224.0 / 255.0, 0.36];
        for an in sw.ambient_nodes.iter().take(a_count.min(MAX_AMBIENT_NODES)) {
            self.batch.fill_circle(an.x * w, an.y * h, an.r, 10, amb_col);
        }

        // 10c. Subtle Git Branch Graph (lower-left)
        if sw.p.env_detail >= 0.2 && sw.p.quality > 0 {
            let base_y = h * 0.86;
            let lane_h = 16.0 * scale;
            let start_x = w * 0.04;
            let span_w = w * 0.28;
            let alpha = 0.36 * sw.p.env_detail;

            self.batch.stroke_line(
                start_x,
                base_y,
                start_x + span_w,
                base_y,
                1.0,
                [139.0 / 255.0, 148.0 / 255.0, 158.0 / 255.0, alpha],
            );
            self.batch.stroke_cubic_bezier(
                start_x + span_w * 0.18,
                base_y,
                start_x + span_w * 0.25,
                base_y - lane_h,
                start_x + span_w * 0.55,
                base_y - lane_h,
                start_x + span_w * 0.65,
                base_y,
                1.0,
                16,
                [155.0 / 255.0, 135.0 / 255.0, 215.0 / 255.0, alpha * 0.9],
            );
            for gc in &sw.git_commits {
                let nx = (gc.x_norm + sw.git_scroll) % 1.0;
                let cx = start_x + nx * span_w;
                let cy = if gc.lane == 1 && nx > 0.22 && nx < 0.60 {
                    base_y - lane_h
                } else {
                    base_y
                };
                let edge_fade = (nx * PI).sin();
                let col = rgba_u8(ACCENT_RGB[gc.accent_idx], alpha * 1.4 * edge_fade);
                self.batch.fill_circle(cx, cy, 2.2 * scale, 10, col);
            }
        }

        // 10d. Subtle Terminal / Log Atmosphere
        if sw.p.env_detail >= 0.3 {
            let alpha = 0.34 * sw.p.env_detail;
            let x = w * 0.04;
            let y = h * 0.93;
            let fsz = (10.0 * scale).round().max(8.0);
            let cur_line = LOG_LINES[sw.log_cursor % LOG_LINES.len()];
            let prev_line = LOG_LINES[(sw.log_cursor + LOG_LINES.len() - 1) % LOG_LINES.len()];
            self.mono_text.push(
                gl,
                cur_line,
                x,
                y + fsz * 0.32,
                fsz,
                dpi,
                false,
                [148.0 / 255.0, 163.0 / 255.0, 184.0 / 255.0, alpha],
                0,
                None,
            );
            self.mono_text.push(
                gl,
                prev_line,
                x,
                y - 14.0 * scale + fsz * 0.32,
                fsz,
                dpi,
                false,
                [148.0 / 255.0, 163.0 / 255.0, 184.0 / 255.0, alpha * 0.55],
                0,
                None,
            );
        }

        // 10e. Floating Code Particles (avoiding icon rects for crisp legibility)
        let c_count = ((MAX_CODE_PARTICLES as f32)
            * sw.p.code_particle_density
            * if sw.p.quality == 0 { 0.5 } else { 1.0 })
        .round() as usize;
        if c_count > 0 {
            let fsz = (10.0 * scale).round().max(8.0);
            let near_d2 = 5200.0 * scale;
            for cp in sw.code_particles.iter().take(c_count.min(MAX_CODE_PARTICLES)) {
                let px = cp.x * w;
                let py = cp.y * h;
                let mut fade = 1.0f32;
                for nd in &sw.nodes {
                    let dx = px - nd.x;
                    let dy = py - nd.y;
                    if dx * dx + dy * dy < near_d2 {
                        fade = 0.25;
                        break;
                    }
                }
                let alpha = cp.alpha * fade * (0.75 + 0.25 * cp.phase.sin());
                let col = rgba_u8(ACCENT_RGB[cp.accent_idx], alpha);
                self.mono_text.push(
                    gl,
                    CODE_GLYPHS[cp.glyph_idx],
                    px,
                    py + fsz * 0.32,
                    fsz,
                    dpi,
                    false,
                    col,
                    1,
                    None,
                );
            }
        }

        // 10f. Inter-Node Connections (quadratic bezier architectural traces + protocol tags)
        let conn_alpha_base = sw.p.connection_intensity * 0.58;
        if conn_alpha_base > 0.005 && !sw.links.is_empty() {
            let proto_fsz = (8.5 * scale).round().max(8.0);
            for lnk in &sw.links {
                let (Some(&ai), Some(&bi)) = (node_map.get(&lnk.a_id), node_map.get(&lnk.b_id)) else {
                    continue;
                };
                let na = &sw.nodes[ai];
                let nb = &sw.nodes[bi];
                let la = na.alpha.min(nb.alpha);
                if la <= 0.02 {
                    continue;
                }
                let (x1, y1, x2, y2) = (na.x, na.y, nb.x, nb.y);
                let mx = (x1 + x2) * 0.5;
                let my = (y1 + y2) * 0.5;
                let dx = x2 - x1;
                let dy = y2 - y1;
                let len = dx.hypot(dy).max(1.0);
                let bend = (28.0 * scale).min(len * 0.14) * lnk.curve_dir;
                let cx = mx - (dy / len) * bend;
                let cy = my + (dx / len) * bend;

                self.batch.stroke_quad_bezier(
                    x1,
                    y1,
                    cx,
                    cy,
                    x2,
                    y2,
                    1.45,
                    18,
                    [132.0 / 255.0, 178.0 / 255.0, 236.0 / 255.0, conn_alpha_base * la],
                );

                if sw.p.env_detail >= 0.55 && len > 130.0 * scale && sw.p.quality > 0 {
                    let lx = 0.25 * x1 + 0.5 * cx + 0.25 * x2;
                    let ly = 0.25 * y1 + 0.5 * cy + 0.25 * y2;
                    self.mono_text.push(
                        gl,
                        LINK_PROTOS[lnk.proto_idx],
                        lx,
                        ly - 6.0 * scale + proto_fsz * 0.32,
                        proto_fsz,
                        dpi,
                        false,
                        [
                            186.0 / 255.0,
                            208.0 / 255.0,
                            232.0 / 255.0,
                            (0.52 * sw.p.connection_intensity * la).clamp(0.0, 1.0),
                        ],
                        1,
                        None,
                    );
                }
            }
        }

        // 10g. Data Packets traveling along links & ambient bus lines
        if sw.p.network_activity > 0.02 {
            for pk in &sw.packet_pool {
                if !pk.active {
                    continue;
                }
                let mut p_alpha =
                    (pk.t * PI).sin() * 0.94 * (0.45 + 0.55 * sw.p.network_activity);
                if p_alpha <= 0.02 {
                    continue;
                }
                let (px_pos, py_pos) = if !pk.is_ambient && pk.link_idx < sw.links.len() {
                    let ln = &sw.links[pk.link_idx];
                    if let (Some(&ai), Some(&bi)) = (node_map.get(&ln.a_id), node_map.get(&ln.b_id))
                    {
                        let na = &sw.nodes[ai];
                        let nb = &sw.nodes[bi];
                        let u = if pk.dir > 0.0 { pk.t } else { 1.0 - pk.t };
                        let (ax1, ay1, ax2, ay2) = (na.x, na.y, nb.x, nb.y);
                        let amx = (ax1 + ax2) * 0.5;
                        let amy = (ay1 + ay2) * 0.5;
                        let adx = ax2 - ax1;
                        let ady = ay2 - ay1;
                        let alen = adx.hypot(ady).max(1.0);
                        let abend = (28.0 * scale).min(alen * 0.14) * ln.curve_dir;
                        let acx = amx - (ady / alen) * abend;
                        let acy = amy + (adx / alen) * abend;
                        let inv = 1.0 - u;
                        p_alpha *= na.alpha.min(nb.alpha);
                        (
                            inv * inv * ax1 + 2.0 * inv * u * acx + u * u * ax2,
                            inv * inv * ay1 + 2.0 * inv * u * acy + u * u * ay2,
                        )
                    } else {
                        continue;
                    }
                } else {
                    p_alpha *= 0.35;
                    (
                        (if pk.dir > 0.0 { pk.t } else { 1.0 - pk.t }) * w,
                        pk.ambient_y * h,
                    )
                };
                self.batch.fill_circle(
                    px_pos,
                    py_pos,
                    2.7 * scale,
                    12,
                    rgba_u8(ACCENT_RGB[pk.accent_idx], p_alpha),
                );
            }
        }

        // 10h. Software Nodes anchored smoothly to desktop icons
        let tag_fsz = (8.5 * scale).round().max(8.0);
        let stage_fsz = (8.0 * scale).round().max(8.0);
        for n in &sw.nodes {
            if n.alpha <= 0.01 {
                continue;
            }
            let role = ROLES[n.role_idx];
            let rgb = ACCENT_RGB[role.accent_idx];
            let pad = 12.0 * scale;
            let bw = (54.0 * scale).max(n.w + pad * 2.0) * n.scale;
            let bh = (54.0 * scale).max(n.h + pad * 2.0) * n.scale;
            let left = n.x - bw * 0.5;
            let top = n.y - bh * 0.5;
            let right = left + bw;
            let bottom = top + bh;
            let corner = 8.0 * scale;

            // Subtle dark node backing
            self.batch.fill_rect(
                left + 2.0,
                top + 2.0,
                (bw - 4.0).max(0.0),
                (bh - 4.0).max(0.0),
                [16.0 / 255.0, 22.0 / 255.0, 34.0 / 255.0, 0.64 * n.alpha],
            );

            // 4 Technical corner brackets
            let bracket_alpha = ((0.76 + 0.14 * n.pulse_phase.sin()) * n.alpha).min(1.0);
            let br_col = rgba_u8(rgb, bracket_alpha);
            // Top-left
            self.batch.stroke_line(left, top + corner, left, top, 1.9, br_col);
            self.batch.stroke_line(left, top, left + corner, top, 1.9, br_col);
            // Top-right
            self.batch.stroke_line(right - corner, top, right, top, 1.9, br_col);
            self.batch.stroke_line(right, top, right, top + corner, 1.9, br_col);
            // Bottom-right
            self.batch.stroke_line(right, bottom - corner, right, bottom, 1.9, br_col);
            self.batch.stroke_line(right, bottom, right - corner, bottom, 1.9, br_col);
            // Bottom-left
            self.batch.stroke_line(left + corner, bottom, left, bottom, 1.9, br_col);
            self.batch.stroke_line(left, bottom, left, bottom - corner, 1.9, br_col);

            // Subtle inner hairline frame
            self.batch.stroke_rect(
                left + 0.5,
                top + 0.5,
                (bw - 1.0).max(0.0),
                (bh - 1.0).max(0.0),
                1.25,
                rgba_u8(rgb, 0.38 * n.alpha),
            );

            // Status dot & role tag above node
            if sw.p.env_detail >= 0.25 {
                let dot_alpha = if n.state == NodeLifecycle::Running {
                    0.55 + 0.25 * n.pulse_phase.sin()
                } else {
                    0.4
                } * n.alpha;
                self.batch.fill_circle(
                    left + 7.0 * scale,
                    top - 6.0 * scale,
                    2.0 * scale,
                    10,
                    rgba_u8(rgb, dot_alpha),
                );
                let label = match n.state {
                    NodeLifecycle::Boot => "BOOT",
                    NodeLifecycle::Initialize => "INIT",
                    NodeLifecycle::Disconnect | NodeLifecycle::Shutdown => "STOP",
                    _ => role.tag,
                };
                let full_label = format!("{label} {}", role.badge);
                self.mono_text.push(
                    gl,
                    &full_label,
                    left + 13.0 * scale,
                    top - 6.0 * scale + tag_fsz * 0.32,
                    tag_fsz,
                    dpi,
                    false,
                    [
                        205.0 / 255.0,
                        222.0 / 255.0,
                        242.0 / 255.0,
                        (0.78 * n.alpha * sw.p.env_detail.max(0.35)).clamp(0.0, 1.0),
                    ],
                    0,
                    None,
                );
            }

            // Occasional calm SOURCE -> COMPILE -> BUILD -> RUN progress bar
            if n.build_timer >= 0.0 && n.state == NodeLifecycle::Running {
                let prog = (n.build_timer / n.build_dur).clamp(0.0, 1.0);
                let stage = if prog < 0.25 {
                    "SRC"
                } else if prog < 0.55 {
                    "CMP"
                } else if prog < 0.85 {
                    "BLD"
                } else {
                    "RUN"
                };
                let bar_w = bw * prog;
                let bar_col = if prog > 0.85 {
                    [74.0 / 255.0, 222.0 / 255.0, 128.0 / 255.0, 0.45 * n.alpha]
                } else {
                    [217.0 / 255.0, 160.0 / 255.0, 75.0 / 255.0, 0.40 * n.alpha]
                };
                self.batch.fill_rect(left, bottom + 4.0 * scale, bar_w, 1.5, bar_col);
                self.mono_text.push(
                    gl,
                    stage,
                    right,
                    bottom + 11.0 * scale + stage_fsz * 0.32,
                    stage_fsz,
                    dpi,
                    false,
                    [217.0 / 255.0, 160.0 / 255.0, 75.0 / 255.0, 0.45 * n.alpha],
                    2,
                    None,
                );
            }
        }

        // Flush batched vector geometry, then Consolas monospace text labels
        self.flush_batch(gl, css_w, css_h);
        self.mono_text.flush(gl, css_w, css_h);
    }
}

// =========================================================================
// rust-anything Recorded Ground-Truth Diff & Gate Verification Suite
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::path::Path;

    fn fnv1a_fingerprint(rel_path: &str, bytes: &[u8]) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |buf: &[u8]| {
            for &b in buf {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
        };
        eat(rel_path.replace('\\', "/").as_bytes());
        eat(&[0]);
        eat(bytes);
        eat(&[0]);
        format!("{hash:016x}")
    }

    #[test]
    fn verify_rust_anything_recorded_software_world_cases() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root");
        let rec_path = manifest_dir.join("src/software_world/recorded-from.json");
        let cases_path = manifest_dir.join("src/software_world/cases.jsonl");

        let rec_json: Value = serde_json::from_str(
            &std::fs::read_to_string(&rec_path).expect("read recorded-from.json"),
        )
        .expect("parse recorded-from.json");
        let unit_rel = rec_json["unit"].as_str().expect("unit path");
        let expected_fp = rec_json["fingerprint"].as_str().expect("fingerprint");
        let html_bytes =
            std::fs::read(repo_root.join(unit_rel)).expect("read prototype/software-world.html");
        let actual_fp = fnv1a_fingerprint(unit_rel, &html_bytes);
        assert_eq!(
            actual_fp, expected_fp,
            "rust-anything gate: prototype/software-world.html changed since recording"
        );

        let raw_cases = std::fs::read_to_string(&cases_path).expect("read cases.jsonl");
        let mut passed = 0usize;
        for (line_no, line) in raw_cases.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let c: Value = serde_json::from_str(line).expect("parse case JSON");
            let fn_name = c["fn"].as_str().expect("fn");
            let args = c["args"].as_array().expect("args");
            let ok = &c["result"]["ok"];

            match fn_name {
                "xorshift32" => {
                    let seed = args[0].as_i64().unwrap() as i32;
                    let mut rng = Xorshift32::from_seed(seed);
                    let exp = ok.as_array().unwrap();
                    for (i, ev) in exp.iter().enumerate() {
                        let got = rng.next_f32();
                        let e = ev.as_f64().unwrap() as f32;
                        assert!(
                            (got - e).abs() <= 1e-4,
                            "case #{line_no} xorshift32({seed})[{i}]: got {got}, exp {e}"
                        );
                    }
                }
                "initialPools" => {
                    let st = SoftwareWorldState::default();
                    let exp_c = ok["code0"].as_array().unwrap();
                    for (i, ev) in exp_c.iter().enumerate() {
                        let arr = ev.as_array().unwrap();
                        let cp = st.code_particles[i];
                        assert!((cp.x - arr[0].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert!((cp.y - arr[1].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert!((cp.vx - arr[2].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert!((cp.vy - arr[3].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert_eq!(CODE_GLYPHS[cp.glyph_idx], arr[4].as_str().unwrap());
                        assert!((cp.alpha - arr[5].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert_eq!(cp.accent_idx as i64, arr[6].as_i64().unwrap());
                    }
                    let exp_a = ok["amb0"].as_array().unwrap();
                    for (i, ev) in exp_a.iter().enumerate() {
                        let arr = ev.as_array().unwrap();
                        let an = st.ambient_nodes[i];
                        assert!((an.x - arr[0].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert!((an.y - arr[1].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert!((an.vx - arr[2].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert!((an.vy - arr[3].as_f64().unwrap() as f32).abs() <= 1e-4);
                        assert!((an.r - arr[4].as_f64().unwrap() as f32).abs() <= 1e-4);
                    }
                }
                "nodeLifecycleAndLinks" => {
                    let mut st = SoftwareWorldState::new(1920.0, 1080.0, SoftwareWorldParams::default());
                    st.rng.reseed(42);
                    let to_rects = |raw: &[[f32; 4]]| -> Vec<Rect> {
                        raw.iter()
                            .map(|r| Rect { x: r[0], y: r[1], w: r[2] - r[0], h: r[3] - r[1] })
                            .collect()
                    };
                    let r1 = to_rects(&[
                        [100.0, 100.0, 176.0, 190.0],
                        [320.0, 140.0, 396.0, 230.0],
                        [540.0, 260.0, 616.0, 350.0],
                        [180.0, 420.0, 256.0, 510.0],
                        [460.0, 520.0, 536.0, 610.0],
                        [1600.0, 900.0, 1676.0, 990.0],
                    ]);
                    st.update_icons(&r1, true);
                    for _ in 0..48 {
                        st.step(0.033);
                    }
                    let exp_run = ok["runningNodes"].as_array().unwrap();
                    assert_eq!(st.nodes.len(), exp_run.len());
                    for (got, ev) in st.nodes.iter().zip(exp_run.iter()) {
                        assert_eq!(got.id as i64, ev["id"].as_i64().unwrap());
                        assert_eq!(ROLES[got.role_idx].tag, ev["roleTag"].as_str().unwrap());
                        assert_eq!(got.state.as_str(), ev["state"].as_str().unwrap());
                        assert!((got.alpha - ev["alpha"].as_f64().unwrap() as f32).abs() <= 1e-3);
                    }
                    let exp_links = ok["runningLinks"].as_array().unwrap();
                    assert_eq!(st.links.len(), exp_links.len());
                    for (got, ev) in st.links.iter().zip(exp_links.iter()) {
                        assert_eq!(got.a_id as i64, ev["a"].as_i64().unwrap());
                        assert_eq!(got.b_id as i64, ev["b"].as_i64().unwrap());
                        assert_eq!(LINK_PROTOS[got.proto_idx], ev["proto"].as_str().unwrap());
                    }

                    let r2 = to_rects(&[
                        [140.0, 130.0, 216.0, 220.0],
                        [320.0, 140.0, 396.0, 230.0],
                        [540.0, 260.0, 616.0, 350.0],
                        [180.0, 420.0, 256.0, 510.0],
                        [780.0, 200.0, 856.0, 290.0],
                        [1600.0, 900.0, 1676.0, 990.0],
                    ]);
                    st.update_icons(&r2, true);
                    for _ in 0..15 {
                        st.step(0.033);
                    }
                    let exp_mov = ok["movedNodes"].as_array().unwrap();
                    assert_eq!(st.nodes.len(), exp_mov.len());
                    for (got, ev) in st.nodes.iter().zip(exp_mov.iter()) {
                        assert_eq!(got.id as i64, ev["id"].as_i64().unwrap());
                        assert_eq!(got.state.as_str(), ev["state"].as_str().unwrap());
                        assert!((got.x - ev["x"].as_f64().unwrap() as f32).abs() <= 0.2);
                        assert!((got.y - ev["y"].as_f64().unwrap() as f32).abs() <= 0.2);
                    }

                    for _ in 0..30 {
                        st.step(0.033);
                    }
                    let exp_ids: Vec<i64> = ok["afterShutdownIds"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_i64().unwrap())
                        .collect();
                    let got_ids: Vec<i64> = st.nodes.iter().map(|n| n.id as i64).collect();
                    assert_eq!(got_ids, exp_ids);
                    assert!((st.git_scroll - ok["gitScroll"].as_f64().unwrap() as f32).abs() <= 1e-3);
                    assert_eq!(st.log_cursor as i64, ok["logCursor"].as_i64().unwrap());
                }
                "settingsClamp" => {
                    assert_eq!(9.0f32.clamp(0.25, 2.5), ok["animSpeed"].as_f64().unwrap() as f32);
                    assert_eq!((-0.5f32).clamp(0.0, 1.0), ok["networkActivity"].as_f64().unwrap() as f32);
                    assert_eq!(2.0f32.clamp(0.0, 1.0), ok["connectionIntensity"].as_f64().unwrap() as f32);
                }
                other => panic!("Unknown recorded function: {other}"),
            }
            passed += 1;
        }
        assert_eq!(passed, 8, "All 8 recorded rust-anything software-world cases must pass");
    }
}
