use crate::state::Rect;
use std::collections::HashSet;

// =========================================================================
// 1. DETERMINISTIC PRNG & MATH HELPERS
// =========================================================================

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

    #[inline]
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.next_f64() * (hi - lo)
    }
}

#[inline]
pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

// =========================================================================
// 2. ICON INFLUENCE & SPRING-ANIMATED WORKSTATIONS
// =========================================================================

#[derive(Clone, Debug)]
pub struct Workstation {
    pub id: u64,
    pub key: Option<String>,
    // Current smoothly interpolated geometry
    pub cx: f64,
    pub cy: f64,
    pub w: f64,
    pub h: f64,
    // Spring velocities for natural physical transitions
    pub vx: f64,
    pub vy: f64,
    // Target geometry from desktop host
    pub target_cx: f64,
    pub target_cy: f64,
    pub target_w: f64,
    pub target_h: f64,
    // Soft influence radius (safe zone)
    pub pad_r: f64,
    pub influence_r: f64,
    // Lifecycle & reactivity
    pub opacity: f64,
    pub removing: bool,
    pub move_energy: f64,   // spikes when user moves this icon -> alerts nearby mascots
    pub inspect_glow: f64,  // spikes when a mascot interacts with this station
    pub accent_idx: usize,  // subtle station LED tint (0..4)
    pub cluster_id: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct WorkstationLink {
    pub a: usize,
    pub b: usize,
    pub strength: f64,
}

#[derive(Clone, Debug)]
pub struct WorkstationCluster {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
    pub opacity: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InfluenceSample {
    pub strength: f64, // 0 = outside all icons, 1 = icon center
    pub push_x: f64,   // smooth outward displacement vector
    pub push_y: f64,
    pub nearest_idx: Option<usize>,
    pub nearest_dist: f64,
}

pub struct IconInfluence {
    pub stations: Vec<Workstation>,
    pub links: Vec<WorkstationLink>,
    pub clusters: Vec<WorkstationCluster>,
    pub rev: u64,
    next_id: u64,
}

impl Default for IconInfluence {
    fn default() -> Self {
        Self {
            stations: Vec::new(),
            links: Vec::new(),
            clusters: Vec::new(),
            rev: 1,
            next_id: 1,
        }
    }
}

impl IconInfluence {
    pub fn clear_smooth(&mut self) {
        for st in &mut self.stations {
            st.removing = true;
        }
        self.rev = self.rev.wrapping_add(1);
    }

    /// Reconcile incoming desktop icon rects smoothly (spring targets, never teleporting).
    pub fn sync_icons(&mut self, icons: &[Rect], names: &[String], vis: bool, adaptive: bool, w: f64, h: f64) {
        self.rev = self.rev.wrapping_add(1);
        if !vis || !adaptive || icons.is_empty() {
            for st in &mut self.stations {
                st.removing = true;
            }
            return;
        }

        // Cap at 64 active workstations to keep extreme multi-dozen icon desktops calm and fast
        let count = icons.len().min(64);
        let active_indices: Vec<usize> = self
            .stations
            .iter()
            .enumerate()
            .filter_map(|(i, s)| if !s.removing { Some(i) } else { None })
            .collect();

        let mut matched_st: HashSet<usize> = HashSet::new();
        let mut matched_ic: HashSet<usize> = HashSet::new();

        // Pass 1: Match by explicit non-empty icon name if provided
        for ii in 0..count {
            let nm = names.get(ii).map(|s| s.trim()).unwrap_or("");
            if nm.is_empty() {
                continue;
            }
            for &si in &active_indices {
                if !matched_st.contains(&si) {
                    if self.stations[si].key.as_deref() == Some(nm) {
                        self.update_station_target(si, icons[ii]);
                        matched_st.insert(si);
                        matched_ic.insert(ii);
                        break;
                    }
                }
            }
        }

        // Pass 2: Nearest-neighbor spatial matching
        let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
        for ii in 0..count {
            if matched_ic.contains(&ii) {
                continue;
            }
            let ic = icons[ii];
            let cx = ic.x as f64 + ic.w as f64 * 0.5;
            let cy = ic.y as f64 + ic.h as f64 * 0.5;
            for &si in &active_indices {
                if matched_st.contains(&si) {
                    continue;
                }
                let st = &self.stations[si];
                let d = (cx - st.target_cx)
                    .hypot(cy - st.target_cy)
                    .min((cx - st.cx).hypot(cy - st.cy));
                pairs.push((si, ii, d));
            }
        }
        pairs.sort_by(|a, b| a.2.total_cmp(&b.2));

        let max_drag = w.hypot(h) * 0.85;
        for &(si, ii, dist) in &pairs {
            if dist > max_drag {
                break;
            }
            if matched_st.contains(&si) || matched_ic.contains(&ii) {
                continue;
            }
            self.update_station_target(si, icons[ii]);
            let nm = names.get(ii).map(|s| s.trim()).unwrap_or("");
            if !nm.is_empty() {
                self.stations[si].key = Some(nm.to_string());
            }
            matched_st.insert(si);
            matched_ic.insert(ii);
        }

        // Unmatched existing stations fade out smoothly
        for &si in &active_indices {
            if !matched_st.contains(&si) {
                self.stations[si].removing = true;
            }
        }

        // Unmatched incoming icons spawn new smoothly fading-in workstations
        for ii in 0..count {
            if matched_ic.contains(&ii) {
                continue;
            }
            let ic = icons[ii];
            let cx = ic.x as f64 + ic.w as f64 * 0.5;
            let cy = ic.y as f64 + ic.h as f64 * 0.5;
            let iw = (ic.w as f64).clamp(42.0, 140.0);
            let ih = (ic.h as f64).clamp(42.0, 150.0);
            let pad_r = (iw.hypot(ih) * 0.52).clamp(42.0, 74.0);
            let id = self.next_id;
            self.next_id += 1;
            let nm = names.get(ii).map(|s| s.trim()).unwrap_or("");
            self.stations.push(Workstation {
                id,
                key: if nm.is_empty() { None } else { Some(nm.to_string()) },
                cx,
                cy,
                w: iw,
                h: ih,
                vx: 0.0,
                vy: 0.0,
                target_cx: cx,
                target_cy: cy,
                target_w: iw,
                target_h: ih,
                pad_r,
                influence_r: pad_r * 1.95,
                opacity: 0.0,
                removing: false,
                move_energy: 0.35,
                inspect_glow: 0.0,
                accent_idx: (id as usize) % 5,
                cluster_id: 0,
            });
        }
    }

    fn update_station_target(&mut self, idx: usize, ic: Rect) {
        let st = &mut self.stations[idx];
        let ncx = ic.x as f64 + ic.w as f64 * 0.5;
        let ncy = ic.y as f64 + ic.h as f64 * 0.5;
        let moved = (ncx - st.target_cx).hypot(ncy - st.target_cy);
        st.target_cx = ncx;
        st.target_cy = ncy;
        st.target_w = (ic.w as f64).clamp(42.0, 140.0);
        st.target_h = (ic.h as f64).clamp(42.0, 150.0);
        st.removing = false;
        if moved > 12.0 {
            st.move_energy = (st.move_energy + clamp(moved / 90.0, 0.35, 1.0)).min(1.0);
        }
    }

    /// Critically-damped spring physics step + cluster/link recalculation.
    pub fn step(&mut self, dt: f64) {
        let stiffness = 42.0;
        let damping = 11.5;

        for st in &mut self.stations {
            let ax = (st.target_cx - st.cx) * stiffness - st.vx * damping;
            let ay = (st.target_cy - st.cy) * stiffness - st.vy * damping;
            st.vx += ax * dt;
            st.vy += ay * dt;
            st.cx += st.vx * dt;
            st.cy += st.vy * dt;

            // Clamp velocity spikes on rapid icon jumps
            let spd = st.vx.hypot(st.vy);
            if spd > 1800.0 {
                let s = 1800.0 / spd;
                st.vx *= s;
                st.vy *= s;
            }

            let sz_k = 1.0 - (-8.0 * dt).exp();
            st.w = lerp(st.w, st.target_w, sz_k);
            st.h = lerp(st.h, st.target_h, sz_k);
            st.pad_r = (st.w.hypot(st.h) * 0.52).clamp(42.0, 74.0);
            st.influence_r = st.pad_r * 1.95;

            if st.removing {
                st.opacity = (st.opacity - dt * 1.35).max(0.0);
            } else {
                st.opacity = (st.opacity + dt * 1.85).min(1.0);
            }
            st.move_energy = (st.move_energy - dt * 0.85).max(0.0);
            st.inspect_glow = (st.inspect_glow - dt * 0.9).max(0.0);
        }

        self.stations.retain(|s| !(s.removing && s.opacity <= 0.004));
        self.rebuild_clusters_and_links();
    }

    fn rebuild_clusters_and_links(&mut self) {
        self.links.clear();
        self.clusters.clear();
        let n = self.stations.len();
        if n == 0 {
            return;
        }

        // Build glowing path links between nearby workstations (nearest 2 neighbors each to prevent clutter)
        let link_max = 210.0;
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for i in 0..n {
            if self.stations[i].opacity < 0.08 {
                continue;
            }
            let mut cands: Vec<(usize, f64)> = Vec::new();
            for j in 0..n {
                if i == j || self.stations[j].opacity < 0.08 {
                    continue;
                }
                let d = (self.stations[j].cx - self.stations[i].cx)
                    .hypot(self.stations[j].cy - self.stations[i].cy);
                if d > 24.0 && d < link_max {
                    cands.push((j, d));
                }
            }
            cands.sort_by(|a, b| a.1.total_cmp(&b.1));
            for &(j, d) in cands.iter().take(2) {
                adj[i].push(j);
                adj[j].push(i);
                if i < j {
                    let s = (1.0 - d / link_max) * self.stations[i].opacity.min(self.stations[j].opacity);
                    self.links.push(WorkstationLink { a: i, b: j, strength: s });
                }
            }
        }

        // Connected components for clustered workstations (2+ close icons share a unified terrace deck)
        let mut visited = vec![false; n];
        let mut cid = 1usize;
        for i in 0..n {
            if visited[i] || self.stations[i].opacity < 0.08 {
                continue;
            }
            let mut stack = vec![i];
            let mut comp = Vec::new();
            visited[i] = true;
            while let Some(u) = stack.pop() {
                comp.push(u);
                for &v in &adj[u] {
                    if !visited[v] {
                        visited[v] = true;
                        stack.push(v);
                    }
                }
            }
            for &u in &comp {
                self.stations[u].cluster_id = cid;
            }
            if comp.len() >= 2 {
                let mut min_x = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                let mut op_sum = 0.0;
                for &u in &comp {
                    let st = &self.stations[u];
                    min_x = min_x.min(st.cx - st.pad_r);
                    max_x = max_x.max(st.cx + st.pad_r);
                    min_y = min_y.min(st.cy - st.pad_r * 0.78);
                    max_y = max_y.max(st.cy + st.pad_r * 0.78);
                    op_sum += st.opacity;
                }
                self.clusters.push(WorkstationCluster {
                    cx: (min_x + max_x) * 0.5,
                    cy: (min_y + max_y) * 0.5,
                    rx: ((max_x - min_x) * 0.5 + 14.0).min(420.0),
                    ry: ((max_y - min_y) * 0.5 + 12.0).min(320.0),
                    opacity: (op_sum / comp.len() as f64).min(1.0),
                });
            }
            cid += 1;
        }
    }

    /// Reusable soft circular influence query with smooth cubic falloff (0 far -> 1 center).
    pub fn sample(&self, x: f64, y: f64) -> InfluenceSample {
        let mut out = InfluenceSample {
            strength: 0.0,
            push_x: 0.0,
            push_y: 0.0,
            nearest_idx: None,
            nearest_dist: f64::INFINITY,
        };
        for (i, st) in self.stations.iter().enumerate() {
            if st.opacity <= 0.01 {
                continue;
            }
            let dx = x - st.cx;
            let dy = y - st.cy;
            let dist = dx.hypot(dy);
            if dist < out.nearest_dist {
                out.nearest_dist = dist;
                out.nearest_idx = Some(i);
            }
            if dist < st.influence_r {
                let u = 1.0 - dist / st.influence_r;
                let fall = u * u * (3.0 - 2.0 * u) * st.opacity;
                out.strength = (out.strength + fall).min(1.0);
                let inv = if dist > 0.5 { 1.0 / dist } else { 1.0 };
                let nx = if dist > 0.5 { dx * inv } else { 1.0 };
                let ny = if dist > 0.5 { dy * inv } else { 0.0 };
                let mag = fall * st.pad_r * 0.85;
                out.push_x += nx * mag;
                out.push_y += ny * mag;
            }
        }
        out
    }

    pub fn is_inside_safe_zone(&self, x: f64, y: f64, extra_margin: f64) -> bool {
        for st in &self.stations {
            if st.opacity < 0.15 {
                continue;
            }
            let r = st.pad_r + extra_margin;
            if (x - st.cx).hypot(y - st.cy) < r {
                return true;
            }
        }
        false
    }
}

// =========================================================================
// 3. CAMPUS ENVIRONMENT (Labs, Server Vaults, Curved Paths, Trees, Lamps)
// =========================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildingKind {
    Atrium,      // Central Research Atrium
    CodeForge,   // Coding AI Lab
    Observatory, // Reasoning Observatory
    ServerVault, // Local AI Server Room
    Workshop,    // Creative & Open-Source Annex
}

#[derive(Clone, Debug)]
pub struct CampusBuilding {
    pub kind: BuildingKind,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub height: f64,
    pub accent: [f32; 3],
    pub dock_x: f64, // where mascots work/charge near this station
    pub dock_y: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct CampusTree {
    pub base_x: f64,
    pub base_y: f64,
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub scale: f64,
    pub variant: usize,
    pub sway_phase: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct CampusLamp {
    pub base_x: f64,
    pub base_y: f64,
    pub x: f64,
    pub y: f64,
    pub warm: bool,
    pub opacity: f64,
}

#[derive(Clone, Debug)]
pub struct CampusPath {
    pub pts: Vec<[f64; 2]>,
    pub base_pts: Vec<[f64; 2]>,
}

#[derive(Clone, Copy, Debug)]
pub struct CampusTerminal {
    pub x: f64,
    pub y: f64,
    pub accent: [f32; 3],
    pub kind: usize, // 0=terminal desk, 1=server rack, 2=holo pedestal
}

pub struct CampusEnvironment {
    pub w: f64,
    pub h: f64,
    pub buildings: Vec<CampusBuilding>,
    pub trees: Vec<CampusTree>,
    pub lamps: Vec<CampusLamp>,
    pub paths: Vec<CampusPath>,
    pub terminals: Vec<CampusTerminal>,
}

impl Default for CampusEnvironment {
    fn default() -> Self {
        let mut env = Self {
            w: 1920.0,
            h: 1080.0,
            buildings: Vec::new(),
            trees: Vec::new(),
            lamps: Vec::new(),
            paths: Vec::new(),
            terminals: Vec::new(),
        };
        env.rebuild(1920.0, 1080.0, 2026);
        env
    }
}

impl CampusEnvironment {
    pub fn rebuild(&mut self, w: f64, h: f64, seed: u32) {
        self.w = w;
        self.h = h;
        let s = (w.min(h) / 1080.0).clamp(0.58, 1.8);
        let mut rng = Mulberry32::new(seed ^ 0x4C41_4253);

        // 5 balanced isometric campus buildings positioned in calm safe architectural zones
        let specs: [(BuildingKind, f64, f64, f64, f64, f64, [f32; 3], f64, f64); 5] = [
            (
                BuildingKind::Atrium,
                0.50, 0.34,
                164.0 * s, 88.0 * s, 56.0 * s,
                [0.44, 0.84, 0.94], // soft cyan
                0.0, 44.0 * s,
            ),
            (
                BuildingKind::CodeForge,
                0.26, 0.32,
                122.0 * s, 68.0 * s, 44.0 * s,
                [0.34, 0.78, 0.96], // coding blue-cyan
                34.0 * s, 32.0 * s,
            ),
            (
                BuildingKind::Observatory,
                0.74, 0.32,
                124.0 * s, 70.0 * s, 54.0 * s,
                [0.66, 0.56, 0.96], // reasoning purple
                -34.0 * s, 34.0 * s,
            ),
            (
                BuildingKind::ServerVault,
                0.29, 0.72,
                118.0 * s, 66.0 * s, 40.0 * s,
                [0.34, 0.86, 0.56], // local emerald
                38.0 * s, 12.0 * s,
            ),
            (
                BuildingKind::Workshop,
                0.72, 0.71,
                128.0 * s, 68.0 * s, 42.0 * s,
                [0.92, 0.62, 0.34], // warm amber-orange
                -38.0 * s, 14.0 * s,
            ),
        ];

        self.buildings.clear();
        for &(kind, nx, ny, bw, bh, bht, accent, dx, dy) in &specs {
            let x = nx * w;
            let y = ny * h;
            self.buildings.push(CampusBuilding {
                kind,
                x,
                y,
                w: bw,
                h: bh,
                height: bht,
                accent,
                dock_x: x + dx,
                dock_y: y + dy,
            });
        }

        // Outdoor research terminals, mini server stacks & holographic pedestals
        self.terminals.clear();
        let term_specs: [(f64, f64, [f32; 3], usize); 6] = [
            (0.42, 0.52, [0.43, 0.91, 0.72], 0), // General desk
            (0.35, 0.43, [0.34, 0.80, 0.95], 0), // Coding holo desk
            (0.61, 0.45, [0.66, 0.55, 0.96], 2), // Reasoning pedestal
            (0.51, 0.61, [0.58, 0.77, 0.99], 2), // Research data hub
            (0.34, 0.75, [0.32, 0.86, 0.54], 1), // Local server rack
            (0.65, 0.74, [0.92, 0.58, 0.34], 1), // Open-source hardware rack
        ];
        for &(nx, ny, acc, kind) in &term_specs {
            self.terminals.push(CampusTerminal {
                x: nx * w,
                y: ny * h,
                accent: acc,
                kind,
            });
        }

        // Curved scenic walkways linking central courtyard to all 4 wings + perimeter loop
        self.paths.clear();
        let hub = [0.50 * w, 0.52 * h];
        let path_pairs: [([f64; 2], [f64; 2]); 6] = [
            (hub, [self.buildings[0].dock_x, self.buildings[0].dock_y]),
            (hub, [self.buildings[1].dock_x, self.buildings[1].dock_y]),
            (hub, [self.buildings[2].dock_x, self.buildings[2].dock_y]),
            (hub, [self.buildings[3].dock_x, self.buildings[3].dock_y]),
            (hub, [self.buildings[4].dock_x, self.buildings[4].dock_y]),
            (
                [self.buildings[3].dock_x, self.buildings[3].dock_y],
                [self.buildings[4].dock_x, self.buildings[4].dock_y],
            ),
        ];
        for (p0, p1) in path_pairs {
            let segs = 18usize;
            let mx = (p0[0] + p1[0]) * 0.5;
            let my = (p0[1] + p1[1]) * 0.5;
            let dx = p1[0] - p0[0];
            let dy = p1[1] - p0[1];
            let bend = 0.08;
            let cx = mx - dy * bend;
            let cy = my + dx * bend * 0.6;
            let mut base_pts = Vec::with_capacity(segs + 1);
            for i in 0..=segs {
                let t = i as f64 / segs as f64;
                let u = 1.0 - t;
                let x = u * u * p0[0] + 2.0 * u * t * cx + t * t * p1[0];
                let y = u * u * p0[1] + 2.0 * u * t * cy + t * t * p1[1];
                base_pts.push([x, y]);
            }
            self.paths.push(CampusPath {
                pts: base_pts.clone(),
                base_pts,
            });
        }

        // Courtyard trees & plants positioned outside building footprints
        self.trees.clear();
        let mut tries = 0usize;
        while self.trees.len() < 30 && tries < 260 {
            tries += 1;
            let x = rng.range(0.06, 0.94) * w;
            let y = rng.range(0.20, 0.92) * h;
            let mut blocked = false;
            for b in &self.buildings {
                if (x - b.x).abs() < b.w * 0.72 && (y - b.y).abs() < b.h * 0.85 {
                    blocked = true;
                    break;
                }
            }
            if !blocked {
                for t in &self.trees {
                    if (x - t.base_x).hypot(y - t.base_y) < 58.0 * s {
                        blocked = true;
                        break;
                    }
                }
            }
            if !blocked && (x - hub[0]).hypot(y - hub[1]) < 90.0 * s {
                blocked = true;
            }
            if blocked {
                continue;
            }
            let size = rng.range(11.0, 19.0) * s;
            let variant = (rng.next_f64() * 3.0).floor() as usize;
            let sway_phase = rng.range(0.0, std::f64::consts::TAU);
            self.trees.push(CampusTree {
                base_x: x,
                base_y: y,
                x,
                y,
                size,
                scale: 1.0,
                variant,
                sway_phase,
            });
        }

        // Subtle courtyard bollard lamps along paths and near buildings
        self.lamps.clear();
        for (pi, path) in self.paths.iter().enumerate() {
            for &idx in &[5usize, 12usize] {
                if let Some(&pt) = path.base_pts.get(idx) {
                    let off = if (pi + idx) % 2 == 0 { 20.0 * s } else { -20.0 * s };
                    let lx = clamp(pt[0] + off, 24.0, w - 24.0);
                    let ly = clamp(pt[1] + 10.0 * s, 90.0, h - 24.0);
                    self.lamps.push(CampusLamp {
                        base_x: lx,
                        base_y: ly,
                        x: lx,
                        y: ly,
                        warm: (pi + idx) % 3 != 0,
                        opacity: 1.0,
                    });
                }
            }
        }
    }

    /// Smoothly adapt paths, trees, and lamps around desktop icon safe zones every frame.
    pub fn adapt_to_icons(&mut self, field: &IconInfluence, dt: f64) {
        let k = 1.0 - (-7.5 * dt).exp();

        // Curve walking paths smoothly around icon influence areas
        for path in &mut self.paths {
            let len = path.base_pts.len();
            for i in 0..len {
                let bp = path.base_pts[i];
                // Keep endpoints anchored softly, bend interior points smoothly
                let end_weight = if i == 0 || i + 1 == len { 0.25 } else { 1.0 };
                let inf = field.sample(bp[0], bp[1]);
                let tx = bp[0] + inf.push_x * 1.15 * end_weight;
                let ty = bp[1] + inf.push_y * 1.15 * end_weight;
                path.pts[i][0] = lerp(path.pts[i][0], tx, k);
                path.pts[i][1] = lerp(path.pts[i][1], ty, k);
            }
        }

        // Smoothly displace trees radially away from icons; gently tuck down if deeply covered
        for tr in &mut self.trees {
            let inf = field.sample(tr.base_x, tr.base_y);
            let tx = clamp(tr.base_x + inf.push_x * 1.35, 18.0, self.w - 18.0);
            let ty = clamp(tr.base_y + inf.push_y * 1.35, 64.0, self.h - 18.0);
            let tgt_scale = if inf.strength > 0.72 {
                lerp(1.0, 0.28, (inf.strength - 0.72) / 0.28)
            } else {
                1.0
            };
            tr.x = lerp(tr.x, tx, k);
            tr.y = lerp(tr.y, ty, k);
            tr.scale = lerp(tr.scale, tgt_scale, k);
        }

        // Smoothly displace courtyard lamps away from icons
        for lm in &mut self.lamps {
            let inf = field.sample(lm.base_x, lm.base_y);
            let tx = clamp(lm.base_x + inf.push_x * 1.25, 18.0, self.w - 18.0);
            let ty = clamp(lm.base_y + inf.push_y * 1.25, 64.0, self.h - 18.0);
            let tgt_op = if inf.strength > 0.78 { 0.25 } else { 1.0 };
            lm.x = lerp(lm.x, tx, k);
            lm.y = lerp(lm.y, ty, k);
            lm.opacity = lerp(lm.opacity, tgt_op, k);
        }
    }
}
