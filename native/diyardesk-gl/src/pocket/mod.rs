pub mod shaders;

use crate::beach::sample_arr;
use crate::gl::*;
use crate::meadow::{fbm, make_noise_image, IconBox as IconRect, STEP};
use crate::state::{AppState, Rect};
use shaders::{POCKET_FRAG, POCKET_INST_FRAG, POCKET_INST_VERT, POCKET_VERT};
use std::f64::consts::PI;
use std::ffi::c_void;

pub const POCKET_COLS: usize = 8;
pub const POCKET_ROWS: usize = 4;
pub const POCKET_CELL: usize = 256; // 2x supersampled (128 * 2) -> 2048 x 1024 RGBA8

const MORPH_SEC: f64 = 1.3;

#[derive(Clone, Copy, PartialEq)]
pub struct PocketParams {
    pub sigma: f64,
    pub warp: f64,
    pub arrows: f64,
    pub divisions: usize,
    pub faction: i32,
    pub pace: f64,
    pub skirmish: f64,
    pub supply: f64,
    pub adaptive: bool,
    pub seed: i32,
    pub debug: bool,
}

impl Default for PocketParams {
    fn default() -> Self {
        Self {
            sigma: 78.0,
            warp: 22.0,
            arrows: 0.78,
            divisions: 12,
            faction: 0,
            pace: 1.0,
            skirmish: 0.65,
            supply: 0.55,
            adaptive: true,
            seed: 42,
            debug: false,
        }
    }
}

pub fn pocket_hash(a: i32, b: i32, k: i32, seed: i32) -> f64 {
    let mut h = a.wrapping_add(17).wrapping_mul(374761393)
        ^ b.wrapping_add(31).wrapping_mul(668265263)
        ^ seed.wrapping_add(k.wrapping_mul(23)).wrapping_mul(2246822519u32 as i32);
    h = (h ^ ((h as u32 >> 13) as i32)).wrapping_mul(1274126177);
    (((h ^ ((h as u32 >> 16) as i32)) as u32) as f64) / 4294967296.0
}

const TR_WORDS: &[&str] = &[
    "belge", "belgeler", "belgelerim", "indirilen", "indirilenler", "masaustu", "masaüstü",
    "resim", "resimler", "resimlerim", "muzik", "müzik", "video", "videolar", "oyun", "oyunlar",
    "proje", "projeler", "yedek", "yedekler", "yeni", "klasor", "klasör", "not", "notlar",
    "odev", "ödev", "arsiv", "arşiv", "dosya", "dosyalar", "rapor", "raporlar", "fatura",
    "fotograf", "fotoğraf", "calisma", "çalışma", "calismalar", "goruntu", "görüntü", "ekran",
    "kurulum", "tasarim", "tasarım", "gunluk", "günlük", "hesap", "liste", "deneme", "cop",
    "çöp", "sozlesme", "sözleşme", "ozgecmis", "özgeçmiş", "gider", "sunum", "bilgisayar",
    "araclar", "araçlar", "onemli", "önemli", "kitap", "ders", "notlarim", "planlama",
];

const DE_WORDS: &[&str] = &[
    "ordner", "dokument", "dokumente", "bilder", "musik", "arbeitsplatz", "papierkorb",
    "herunterladen", "entwurf", "entwurfe", "entwürfe", "projekt", "projekte", "bericht",
    "berichte", "ubersicht", "übersicht", "buro", "büro", "rechnung", "rechnungen", "vertrag",
    "sicherung", "notizen", "zusammenfassung", "schule", "arbeit", "datei", "dateien",
    "karte", "karten", "spiel", "spiele", "foto", "fotos", "neu", "wichtig", "festung",
    "plane", "pläne", "ablage", "studium", "urlaub", "schrift", "daten", "werkzeug",
];

const EN_WORDS: &[&str] = &[
    "documents", "downloads", "desktop", "pictures", "music", "videos", "projects", "project",
    "screenshots", "screenshot", "backup", "backups", "archive", "archives", "games", "game",
    "notes", "report", "reports", "invoice", "draft", "resume", "work", "study", "tools",
    "folder", "new", "setup", "config", "test", "code", "server", "steam", "home", "trash",
    "bin", "readme", "apps", "media", "assets", "files", "movies", "images", "plot", "misc",
];

fn strip_ext(raw: &str) -> &str {
    if let Some(dot) = raw.rfind('.') {
        let ext = &raw[dot + 1..];
        if (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            return &raw[..dot];
        }
    }
    raw
}

pub fn detect_folder_nation(raw_name: &str, salt: i32, faction_mode: i32) -> usize {
    if faction_mode == 1 {
        return 0;
    }
    if faction_mode == 2 {
        return 1;
    }
    if faction_mode == 3 {
        return 2;
    }
    let raw = raw_name.trim();
    if raw.is_empty() {
        return 2;
    }
    let stem = strip_ext(raw);
    let lower: String = stem
        .chars()
        .flat_map(|c| {
            if c == 'İ' || c == 'I' {
                vec!['i']
            } else {
                c.to_lowercase().collect::<Vec<_>>()
            }
        })
        .collect();

    let mut s_tr = 0.0f64;
    let mut s_de = 0.0f64;
    let mut s_en = 0.0f64;

    if stem.chars().any(|c| matches!(c, 'ğ' | 'Ğ' | 'ş' | 'Ş' | 'ı' | 'İ')) {
        s_tr += 8.0;
    }
    if stem.chars().any(|c| matches!(c, 'ß' | 'ä' | 'Ä')) {
        s_de += 8.0;
    }
    if stem.chars().any(|c| matches!(c, 'ç' | 'Ç' | 'ö' | 'Ö' | 'ü' | 'Ü')) {
        s_tr += 2.5;
        s_de += 2.2;
    }

    let is_sep = |c: char| {
        c.is_whitespace() || matches!(c, '_' | '-' | '.' | '(' | ')' | '[' | ']') || c.is_ascii_digit()
    };
    for tok in lower.split(is_sep).filter(|s| !s.is_empty()) {
        if TR_WORDS.contains(&tok) {
            s_tr += 6.0;
        }
        if DE_WORDS.contains(&tok) {
            s_de += 6.0;
        }
        if EN_WORDS.contains(&tok) {
            s_en += 6.0;
        }
        if ["lar", "ler", "lari", "leri", "lik", "luk", "cim", "sim", "yor", "mis", "mus"]
            .iter()
            .any(|suf| tok.ends_with(suf))
        {
            s_tr += 2.5;
        }
        if ["ung", "heit", "keit", "schaft", "chen", "lein", "isch", "tsch"]
            .iter()
            .any(|suf| tok.ends_with(suf))
            || tok.contains("sch")
        {
            s_de += 2.8;
        }
        if ["tion", "ment", "ness", "ing", "shot", "app"]
            .iter()
            .any(|suf| tok.ends_with(suf))
        {
            s_en += 2.4;
        }
    }

    for &w in TR_WORDS {
        if w.chars().count() >= 4 && lower.contains(w) {
            s_tr += 3.0;
        }
    }
    for &w in DE_WORDS {
        if w.chars().count() >= 4 && lower.contains(w) {
            s_de += 3.0;
        }
    }
    for &w in EN_WORDS {
        if w.chars().count() >= 4 && lower.contains(w) {
            s_en += 3.0;
        }
    }

    if s_tr > s_de && s_tr > s_en && s_tr >= 2.0 {
        return 0;
    }
    if s_de > s_tr && s_de > s_en && s_de >= 2.0 {
        return 1;
    }
    if s_en > 0.0 {
        return 2;
    }

    if !stem.is_ascii() {
        (salt.rem_euclid(3)) as usize
    } else {
        2
    }
}

#[derive(Clone, Debug)]
pub struct PocketFort {
    pub x: f64,
    pub y: f64,
    pub grow: f64,
    pub want: f64,
    pub rot: f64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct PocketHQ {
    pub x: f64,
    pub y: f64,
    pub count: usize,
}

#[derive(Clone, Debug)]
pub struct PocketDivision {
    pub idx: usize,
    pub x: f64,
    pub y: f64,
    pub tx: f64,
    pub ty: f64,
    pub in_x: f64,
    pub in_y: f64,
    pub nation: usize,
    pub unit_col: usize,
    pub phase: f64,
    pub fire_cooldown: f64,
}

#[derive(Clone, Debug)]
pub struct PocketSkirmish {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub age: f64,
    pub dur: f64,
    pub kind: usize,
    pub size: f64,
}

#[derive(Clone, Debug)]
struct PocketSlot {
    x: f64,
    y: f64,
    in_x: f64,
    in_y: f64,
    nation: usize,
    ang: f64,
    dist_s: f64,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct InstSprite {
    x: f32,
    y: f32,
    size: f32,
    rot: f32,
    col: f32,
    row: f32,
    alpha: f32,
}

pub struct PocketState {
    pub params: PocketParams,
    pub w: f64,
    pub h: f64,
    pub gw: usize,
    pub gh: usize,
    pub nz_wx: Vec<f32>,
    pub nz_wy: Vec<f32>,
    pub fa: Vec<f32>,
    pub fb: Vec<f32>,
    pub fa_dirty: bool,
    pub fb_dirty: bool,
    pub noise_tex_dirty: bool,
    pub morph_elapsed: f64,
    pub morph_dur: f64,
    pub cur_mix: f64,
    pub sim_time: f64,
    pub raw_icons: Vec<Rect>,
    pub icon_names: Vec<String>,
    pub icon_rects: Vec<IconRect>,
    pub icons_visible: bool,
    pub has_committed_once: bool,
    pub forts: Vec<PocketFort>,
    pub pocket_hq: Vec<PocketHQ>,
    pub divisions: Vec<PocketDivision>,
    pub skirmishes: Vec<PocketSkirmish>,
}

impl Default for PocketState {
    fn default() -> Self {
        Self::new(1920.0, 1080.0)
    }
}

impl PocketState {
    pub fn new(w: f64, h: f64) -> Self {
        let mut st = Self {
            params: PocketParams::default(),
            w: 0.0,
            h: 0.0,
            gw: 1,
            gh: 1,
            nz_wx: Vec::new(),
            nz_wy: Vec::new(),
            fa: Vec::new(),
            fb: Vec::new(),
            fa_dirty: true,
            fb_dirty: true,
            noise_tex_dirty: true,
            morph_elapsed: 100.0,
            morph_dur: MORPH_SEC,
            cur_mix: 1.0,
            sim_time: 0.0,
            raw_icons: Vec::new(),
            icon_names: Vec::new(),
            icon_rects: Vec::new(),
            icons_visible: true,
            has_committed_once: false,
            forts: Vec::new(),
            pocket_hq: Vec::new(),
            divisions: Vec::new(),
            skirmishes: Vec::new(),
        };
        st.resize(w as f32, h as f32);
        st
    }

    pub fn build_noise(&mut self) {
        let n = self.gw * self.gh;
        self.nz_wx.resize(n, 0.0);
        self.nz_wy.resize(n, 0.0);
        let seed = self.params.seed;
        for gy in 0..self.gh {
            for gx in 0..self.gw {
                let x = (gx as f64) * STEP;
                let y = (gy as f64) * STEP;
                let i = gy * self.gw + gx;
                self.nz_wx[i] = ((fbm(x / 170.0, y / 170.0, 11, seed) - 0.5) * 2.0) as f32;
                self.nz_wy[i] = ((fbm(x / 170.0, y / 170.0, 29, seed) - 0.5) * 2.0) as f32;
            }
        }
    }

    pub fn compute_field_raw(&self, rects: &[IconRect]) -> Vec<f32> {
        let n = self.gw * self.gh;
        let mut s = vec![0.0f32; n];
        let sig = self.params.sigma;
        let k = sig * 0.4;
        let w = self.params.warp;
        let max_d = sig * 1.9 + w + k * 3.0;
        if rects.is_empty() {
            s.fill(999.0);
            return s;
        }
        for gy in 0..self.gh {
            let py = (gy as f64) * STEP;
            for gx in 0..self.gw {
                let px = (gx as f64) * STEP;
                let i = gy * self.gw + gx;
                let sx = px + (self.nz_wx[i] as f64) * w;
                let sy = py + (self.nz_wy[i] as f64) * w;
                let mut sum = 0.0f64;
                let mut dmin = 1e9f64;
                for r in rects {
                    let dx = ((sx - r.cx).abs() - r.hw).max(0.0);
                    let dy = ((sy - r.cy).abs() - r.hh).max(0.0);
                    let d = dx.hypot(dy);
                    if d < dmin {
                        dmin = d;
                    }
                    if d < max_d {
                        sum += (-(d - sig) / k).exp();
                    }
                }
                s[i] = (if sum > 0.0 {
                    -k * sum.ln()
                } else {
                    (dmin - sig).max(60.0)
                }) as f32;
            }
        }
        s
    }

    pub fn land_s(&self, x: f64, y: f64) -> f64 {
        if self.fb.is_empty() {
            999.0
        } else {
            sample_arr(&self.fb, self.gw, self.gh, x, y)
        }
    }

    pub fn pocket_gradient(&self, px: f64, py: f64) -> (f64, f64) {
        let gx = self.land_s(px + 6.0, py) - self.land_s(px - 6.0, py);
        let gy = self.land_s(px, py + 6.0) - self.land_s(px, py - 6.0);
        let len = gx.hypot(gy).max(0.001);
        (-gx / len, -gy / len)
    }

    pub fn resize(&mut self, w_css: f32, h_css: f32) {
        let w = (w_css as f64).max(320.0);
        let h = (h_css as f64).max(200.0);
        if (self.w - w).abs() < 0.5 && (self.h - h).abs() < 0.5 && !self.fb.is_empty() {
            return;
        }
        self.w = w;
        self.h = h;
        self.gw = (w / STEP).ceil() as usize + 2;
        self.gh = (h / STEP).ceil() as usize + 2;
        self.build_noise();
        self.refresh();
        self.rebuild_props();
    }

    pub fn commit(&mut self, from_empty: bool) {
        let active_rects = if self.params.adaptive && self.icons_visible {
            &self.icon_rects[..]
        } else {
            &[]
        };
        let nf = self.compute_field_raw(active_rects);
        if from_empty || self.fb.is_empty() {
            self.fa = self.compute_field_raw(&[]);
        } else {
            self.fa = self.fb.clone();
        }
        self.fb = nf;
        self.fa_dirty = true;
        self.fb_dirty = true;
        self.morph_elapsed = 0.0;
        self.morph_dur = if from_empty { 1.8 } else { MORPH_SEC };
        self.cur_mix = 0.0;
        if self.forts.is_empty() {
            self.rebuild_props();
        }
        self.update_targets(from_empty);
    }

    pub fn refresh(&mut self) {
        let active_rects = if self.params.adaptive && self.icons_visible {
            &self.icon_rects[..]
        } else {
            &[]
        };
        let nf = self.compute_field_raw(active_rects);
        self.fa = nf.clone();
        self.fb = nf;
        self.fa_dirty = true;
        self.fb_dirty = true;
        self.morph_elapsed = 100.0;
        self.cur_mix = 1.0;
        if !self.forts.is_empty() {
            self.update_targets(true);
        }
    }

    pub fn set_icons(&mut self, icons: &[Rect], names: &[String], vis: bool) {
        let new_rects: Vec<IconRect> = icons
            .iter()
            .map(|ic| IconRect {
                cx: (ic.x + ic.w * 0.5) as f64,
                cy: (ic.y + ic.h * 0.5) as f64,
                hw: (ic.w * 0.5 - 4.0) as f64,
                hh: (ic.h * 0.5 - 2.0) as f64,
            })
            .collect();
        let same = self.icons_visible == vis
            && self.icon_rects.len() == new_rects.len()
            && self.icon_names == names
            && self
                .icon_rects
                .iter()
                .zip(new_rects.iter())
                .all(|(a, b)| (a.cx - b.cx).abs() < 0.5 && (a.cy - b.cy).abs() < 0.5);
        if same && self.has_committed_once {
            return;
        }
        self.raw_icons = if vis { icons.to_vec() } else { Vec::new() };
        self.icon_names = if vis { names.to_vec() } else { Vec::new() };
        self.icons_visible = vis;
        self.icon_rects = new_rects;
        let first = !self.has_committed_once;
        self.has_committed_once = true;
        self.commit(first);
    }

    pub fn sync_params(&mut self, p: PocketParams) {
        let seed_changed = self.params.seed != p.seed;
        let adaptive_changed = self.params.adaptive != p.adaptive;
        let field_changed =
            (self.params.sigma - p.sigma).abs() > 1e-4 || (self.params.warp - p.warp).abs() > 1e-4;
        let forces_changed =
            self.params.divisions != p.divisions || self.params.faction != p.faction;
        self.params = p;
        if seed_changed {
            self.noise_tex_dirty = true;
            self.build_noise();
            self.refresh();
            self.rebuild_props();
            return;
        }
        if adaptive_changed {
            self.commit(false);
        } else if field_changed {
            self.refresh();
        }
        if forces_changed {
            self.sync_forces(false);
        }
    }

    pub fn rebuild_props(&mut self) {
        if self.w <= 1.0 || self.h <= 1.0 {
            return;
        }
        self.forts.clear();
        self.divisions.clear();
        self.pocket_hq.clear();
        self.skirmishes.clear();

        let seed = self.params.seed;
        let cell = 175.0;
        let cols = (self.w / cell).ceil() as i32;
        let rows = ((self.h - 48.0) / cell).ceil() as i32;
        for r in 0..rows {
            for c in 0..cols {
                if pocket_hash(c, r, 11, seed) > 0.58 {
                    continue;
                }
                let x = (c as f64 + 0.25 + 0.5 * pocket_hash(c, r, 12, seed)) * cell;
                let y = (r as f64 + 0.25 + 0.5 * pocket_hash(c, r, 13, seed)) * cell;
                if x < 40.0 || x > self.w - 40.0 || y < 40.0 || y > self.h - 76.0 {
                    continue;
                }
                let ok = if self.land_s(x, y) > 48.0 { 1.0 } else { 0.0 };
                self.forts.push(PocketFort {
                    x,
                    y,
                    grow: ok,
                    want: ok,
                    rot: pocket_hash(c, r, 14, seed) * PI * 2.0,
                });
            }
        }
        self.sync_forces(true);
    }

    pub fn sync_forces(&mut self, reset_all: bool) {
        if reset_all {
            self.divisions.clear();
        }
        let want_divs = self.params.divisions.min(24);
        for i in 0..self.forts.len() {
            let ok = if self.land_s(self.forts[i].x, self.forts[i].y) > 48.0 {
                1.0
            } else {
                0.0
            };
            self.forts[i].want = ok;
        }

        self.pocket_hq.clear();
        let n_icons = self.raw_icons.len();
        let mut icon_cluster_id = vec![0usize; n_icons];
        let mut clusters: Vec<PocketHQ> = Vec::new();
        if n_icons > 0 && self.params.adaptive {
            let mut visited = vec![false; n_icons];
            let link_dist = (self.params.sigma * 2.25).max(120.0);
            for i in 0..n_icons {
                if visited[i] {
                    continue;
                }
                let mut q = std::collections::VecDeque::new();
                q.push_back(i);
                visited[i] = true;
                let cid = clusters.len();
                let mut cluster_indices = Vec::new();
                while let Some(cur) = q.pop_front() {
                    icon_cluster_id[cur] = cid;
                    cluster_indices.push(cur);
                    let cx = (self.raw_icons[cur].x + self.raw_icons[cur].w * 0.5) as f64;
                    let cy = (self.raw_icons[cur].y + self.raw_icons[cur].h * 0.5) as f64;
                    for j in 0..n_icons {
                        if visited[j] {
                            continue;
                        }
                        let jx = (self.raw_icons[j].x + self.raw_icons[j].w * 0.5) as f64;
                        let jy = (self.raw_icons[j].y + self.raw_icons[j].h * 0.5) as f64;
                        if (cx - jx).hypot(cy - jy) < link_dist {
                            visited[j] = true;
                            q.push_back(j);
                        }
                    }
                }
                let mut sx = 0.0;
                let mut sy = 0.0;
                for &idx in &cluster_indices {
                    sx += (self.raw_icons[idx].x + self.raw_icons[idx].w * 0.5) as f64;
                    sy += (self.raw_icons[idx].y + self.raw_icons[idx].h * 0.5) as f64;
                }
                let cnt = cluster_indices.len();
                let c_obj = PocketHQ {
                    x: sx / cnt as f64,
                    y: sy / cnt as f64,
                    count: cnt,
                };
                clusters.push(c_obj.clone());
                if cnt >= 2 {
                    self.pocket_hq.push(c_obj);
                }
            }
        }

        let mut cluster_buckets: Vec<Vec<PocketSlot>> = vec![Vec::new(); clusters.len()];
        let step_scan = 22.0;
        let mut py = 36.0;
        while py < self.h - 72.0 {
            let mut px = 44.0;
            while px < self.w - 44.0 {
                let s = self.land_s(px, py);
                if (18.0..=48.0).contains(&s) && n_icons > 0 && !clusters.is_empty() {
                    let mut best_idx = 0usize;
                    let mut best_d = 1e9f64;
                    for i in 0..n_icons {
                        let ix = (self.raw_icons[i].x + self.raw_icons[i].w * 0.5) as f64;
                        let iy = (self.raw_icons[i].y + self.raw_icons[i].h * 0.5) as f64;
                        let d = (px - ix).hypot(py - iy);
                        if d < best_d {
                            best_d = d;
                            best_idx = i;
                        }
                    }
                    let cid = icon_cluster_id[best_idx];
                    let cl = &clusters[cid];
                    let name = self
                        .icon_names
                        .get(best_idx)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    let nation =
                        detect_folder_nation(name, (px + py).round() as i32, self.params.faction);
                    let (in_x, in_y) = self.pocket_gradient(px, py);
                    cluster_buckets[cid].push(PocketSlot {
                        x: px,
                        y: py,
                        in_x,
                        in_y,
                        nation,
                        ang: (py - cl.y).atan2(px - cl.x),
                        dist_s: (s - 30.0).abs(),
                    });
                }
                px += step_scan;
            }
            py += step_scan;
        }

        let mut chosen_slots: Vec<PocketSlot> = Vec::new();
        let active_buckets: Vec<&Vec<PocketSlot>> =
            cluster_buckets.iter().filter(|b| !b.is_empty()).collect();
        if active_buckets.is_empty() {
            for i in 0..want_divs {
                let a = (i as f64 / want_divs.max(1) as f64) * PI * 2.0;
                let px = self.w * 0.5 + a.cos() * self.w.min(self.h) * 0.28;
                let py = self.h * 0.48 + a.sin() * self.w.min(self.h) * 0.24;
                let nat = if self.params.faction > 0 {
                    (self.params.faction - 1).clamp(0, 2) as usize
                } else {
                    i % 3
                };
                chosen_slots.push(PocketSlot {
                    x: px,
                    y: py,
                    in_x: -a.cos(),
                    in_y: -a.sin(),
                    nation: nat,
                    ang: a,
                    dist_s: 0.0,
                });
            }
        } else {
            const SECTOR_COUNT: usize = 16;
            let mut per_cluster_ordered: Vec<Vec<PocketSlot>> = Vec::new();
            for bucket in active_buckets {
                let mut best_per_sector: Vec<Option<PocketSlot>> = vec![None; SECTOR_COUNT];
                for slot in bucket {
                    let sec = ((((slot.ang + PI) / (PI * 2.0)) * SECTOR_COUNT as f64).floor()
                        as isize)
                        .rem_euclid(SECTOR_COUNT as isize) as usize;
                    let replace = match &best_per_sector[sec] {
                        None => true,
                        Some(prev) => slot.dist_s < prev.dist_s,
                    };
                    if replace {
                        best_per_sector[sec] = Some(slot.clone());
                    }
                }
                let clean: Vec<PocketSlot> = best_per_sector.into_iter().flatten().collect();
                if !clean.is_empty() {
                    per_cluster_ordered.push(clean);
                } else {
                    per_cluster_ordered.push(bucket.iter().take(SECTOR_COUNT).cloned().collect());
                }
            }

            let n_clusters = per_cluster_ordered.len();
            for (c, list) in per_cluster_ordered.iter().enumerate() {
                let base = want_divs / n_clusters;
                let want_here = base + if c < (want_divs % n_clusters) { 1 } else { 0 };
                for k in 0..want_here {
                    let pos_idx = ((k * list.len()) / want_here.max(1)) % list.len();
                    chosen_slots.push(list[pos_idx].clone());
                }
            }
        }

        if self.divisions.len() > want_divs {
            self.divisions.truncate(want_divs);
        }
        let seed = self.params.seed;
        for i in 0..want_divs {
            let slot = &chosen_slots[i];
            let unit_type = (pocket_hash(i as i32, 71, 51, seed) * 4.0).floor() as usize;
            let elite = if pocket_hash(i as i32, 72, 52, seed) < 0.35 {
                4
            } else {
                0
            };
            if i < self.divisions.len() {
                let d = &mut self.divisions[i];
                d.tx = slot.x;
                d.ty = slot.y;
                d.in_x = slot.in_x;
                d.in_y = slot.in_y;
                d.nation = slot.nation;
            } else {
                self.divisions.push(PocketDivision {
                    idx: i,
                    x: slot.x,
                    y: slot.y,
                    tx: slot.x,
                    ty: slot.y,
                    in_x: slot.in_x,
                    in_y: slot.in_y,
                    nation: slot.nation,
                    unit_col: (unit_type.min(3)) + elite,
                    phase: pocket_hash(i as i32, 73, 53, seed) * PI * 2.0,
                    fire_cooldown: pocket_hash(i as i32, 74, 54, seed) * 3.5,
                });
            }
        }
    }

    pub fn update_targets(&mut self, snap: bool) {
        for i in 0..self.forts.len() {
            let ok = if self.land_s(self.forts[i].x, self.forts[i].y) > 48.0 {
                1.0
            } else {
                0.0
            };
            self.forts[i].want = ok;
            if snap {
                self.forts[i].grow = ok;
            }
        }
        self.sync_forces(snap);
    }

    pub fn step_props(&mut self, dt: f64, time_sec: f64) {
        let rate = dt * 2.8;
        for f in &mut self.forts {
            if f.grow < f.want {
                f.grow = (f.grow + rate).min(f.want);
            } else if f.grow > f.want {
                f.grow = (f.grow - rate * 1.5).max(f.want);
            }
        }

        let pace = self.params.pace.max(0.1);
        let skirmish = self.params.skirmish;
        let seed = self.params.seed;
        for d in &mut self.divisions {
            let dx = d.tx - d.x;
            let dy = d.ty - d.y;
            let dist = dx.hypot(dy);
            if dist > 1.0 {
                let spd = (dist * 4.5).min((52.0 + dist * 0.85) * pace) * dt;
                d.x += (dx / dist) * dist.min(spd);
                d.y += (dy / dist) * dist.min(spd);
            }

            if skirmish > 0.04 {
                d.fire_cooldown -= dt * (0.5 + pace * 0.9);
                if d.fire_cooldown <= 0.0 {
                    d.fire_cooldown = 1.8
                        + pocket_hash(d.idx as i32, (time_sec * 2.0).floor() as i32, 81, seed)
                            * 3.6;
                    let muzzle_x = d.x + d.in_x * 26.0;
                    let muzzle_y = d.y + d.in_y * 26.0;
                    let kind = if pocket_hash(d.idx as i32, muzzle_x.round() as i32, 82, seed)
                        < 0.55
                    {
                        5
                    } else {
                        6
                    };
                    let size = 28.0 + pocket_hash(d.idx as i32, 3, 83, seed) * 18.0;
                    self.skirmishes.push(PocketSkirmish {
                        x: muzzle_x,
                        y: muzzle_y,
                        vx: d.in_x * 14.0,
                        vy: d.in_y * 14.0,
                        age: 0.0,
                        dur: 0.42,
                        kind,
                        size,
                    });
                }
            }
        }

        let mut i = self.skirmishes.len();
        while i > 0 {
            i -= 1;
            self.skirmishes[i].age += dt;
            self.skirmishes[i].x += self.skirmishes[i].vx * dt;
            self.skirmishes[i].y += self.skirmishes[i].vy * dt;
            if self.skirmishes[i].age >= self.skirmishes[i].dur {
                self.skirmishes.remove(i);
            }
        }
    }

    pub fn step(&mut self, dt_f32: f32) {
        let dt = (dt_f32 as f64).min(0.1);
        self.sim_time += dt;
        self.morph_elapsed += dt;
        let e = (self.morph_elapsed / self.morph_dur).clamp(0.0, 1.0);
        self.cur_mix = if e < 0.5 {
            2.0 * e * e
        } else {
            1.0 - (-2.0 * e + 2.0).powi(2) * 0.5
        };
        let t = self.sim_time;
        self.step_props(dt, t);
    }
}

// ======================================================================
// 2D Software Painter for the 32-cell (8x4) Pocket Division & Tactical Atlas
// ======================================================================
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            px: vec![0u8; w * h * 4],
        }
    }

    #[inline]
    fn blend_pixel(&mut self, x: i32, y: i32, col: [u8; 4], cov: f64) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let a = ((col[3] as f64 / 255.0) * cov).clamp(0.0, 1.0);
        if a <= 1e-4 {
            return;
        }
        let idx = (y as usize * self.w + x as usize) * 4;
        let da = self.px[idx + 3] as f64 / 255.0;
        let out_a = a + da * (1.0 - a);
        if out_a <= 1e-5 {
            return;
        }
        for c in 0..3 {
            let sc = col[c] as f64 / 255.0;
            let dc = self.px[idx + c] as f64 / 255.0;
            let oc = (sc * a + dc * da * (1.0 - a)) / out_a;
            self.px[idx + c] = (oc * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        self.px[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    }

    fn fill_sdf<F: Fn(f64, f64) -> f64>(
        &mut self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        col: [u8; 4],
        sdf: F,
    ) {
        let x0 = (min_x.floor() as i32 - 2).max(0);
        let y0 = (min_y.floor() as i32 - 2).max(0);
        let x1 = (max_x.ceil() as i32 + 2).min(self.w as i32 - 1);
        let y1 = (max_y.ceil() as i32 + 2).min(self.h as i32 - 1);
        for py in y0..=y1 {
            let fy = py as f64 + 0.5;
            for px in x0..=x1 {
                let fx = px as f64 + 0.5;
                let d = sdf(fx, fy);
                if d < 0.75 {
                    let cov = (0.5 - d).clamp(0.0, 1.0);
                    self.blend_pixel(px, py, col, cov);
                }
            }
        }
    }

    fn fill_circle(&mut self, cx: f64, cy: f64, r: f64, col: [u8; 4]) {
        self.fill_sdf(cx - r, cy - r, cx + r, cy + r, col, |x, y| {
            (x - cx).hypot(y - cy) - r
        });
    }

    fn stroke_circle(&mut self, cx: f64, cy: f64, r: f64, w: f64, col: [u8; 4]) {
        let hw = w * 0.5;
        self.fill_sdf(
            cx - r - hw,
            cy - r - hw,
            cx + r + hw,
            cy + r + hw,
            col,
            |x, y| ((x - cx).hypot(y - cy) - r).abs() - hw,
        );
    }

    fn fill_round_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64, col: [u8; 4]) {
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let hw = w * 0.5;
        let hh = h * 0.5;
        self.fill_sdf(x, y, x + w, y + h, col, |px, py| {
            let qx = (px - cx).abs() - (hw - r);
            let qy = (py - cy).abs() - (hh - r);
            qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
        });
    }

    fn stroke_round_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: f64,
        lw: f64,
        col: [u8; 4],
    ) {
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let hw = w * 0.5;
        let hh = h * 0.5;
        let hlw = lw * 0.5;
        self.fill_sdf(x - hlw, y - hlw, x + w + hlw, y + h + hlw, col, |px, py| {
            let qx = (px - cx).abs() - (hw - r);
            let qy = (py - cy).abs() - (hh - r);
            let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
            d.abs() - hlw
        });
    }

    fn draw_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, w: f64, col: [u8; 4]) {
        let r = w * 0.5;
        let min_x = x0.min(x1) - r;
        let min_y = y0.min(y1) - r;
        let max_x = x0.max(x1) + r;
        let max_y = y0.max(y1) + r;
        let vx = x1 - x0;
        let vy = y1 - y0;
        let l2 = (vx * vx + vy * vy).max(1e-6);
        self.fill_sdf(min_x, min_y, max_x, max_y, col, |x, y| {
            let t = (((x - x0) * vx + (y - y0) * vy) / l2).clamp(0.0, 1.0);
            (x - (x0 + t * vx)).hypot(y - (y0 + t * vy)) - r
        });
    }

    fn fill_polygon(&mut self, pts: &[(f64, f64)], col: [u8; 4]) {
        if pts.len() < 3 {
            return;
        }
        let mut min_x = 1e9f64;
        let mut min_y = 1e9f64;
        let mut max_x = -1e9f64;
        let mut max_y = -1e9f64;
        for &(x, y) in pts {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        self.fill_sdf(min_x, min_y, max_x, max_y, col, |px, py| {
            let mut inside = false;
            let mut min_d = 1e9f64;
            let n = pts.len();
            for i in 0..n {
                let (x0, y0) = pts[i];
                let (x1, y1) = pts[(i + 1) % n];
                if ((y0 > py) != (y1 > py)) && (px < (x1 - x0) * (py - y0) / (y1 - y0) + x0) {
                    inside = !inside;
                }
                let vx = x1 - x0;
                let vy = y1 - y0;
                let l2 = (vx * vx + vy * vy).max(1e-6);
                let t = (((px - x0) * vx + (py - y0) * vy) / l2).clamp(0.0, 1.0);
                let d = (px - (x0 + t * vx)).hypot(py - (y0 + t * vy));
                if d < min_d {
                    min_d = d;
                }
            }
            if inside {
                -min_d
            } else {
                min_d
            }
        });
    }

    fn fill_radial_gradient(&mut self, cx: f64, cy: f64, r: f64, stops: &[(f64, [u8; 4])]) {
        let x0 = ((cx - r).floor() as i32 - 1).max(0);
        let y0 = ((cy - r).floor() as i32 - 1).max(0);
        let x1 = ((cx + r).ceil() as i32 + 1).min(self.w as i32 - 1);
        let y1 = ((cy + r).ceil() as i32 + 1).min(self.h as i32 - 1);
        for py in y0..=y1 {
            let fy = py as f64 + 0.5;
            for px in x0..=x1 {
                let fx = px as f64 + 0.5;
                let d = (fx - cx).hypot(fy - cy);
                if d > r + 0.5 {
                    continue;
                }
                let t = (d / r).clamp(0.0, 1.0);
                let mut col = stops.last().unwrap().1;
                for w in stops.windows(2) {
                    if t >= w[0].0 && t <= w[1].0 {
                        let u = ((t - w[0].0) / (w[1].0 - w[0].0).max(1e-5)).clamp(0.0, 1.0);
                        col = [
                            (w[0].1[0] as f64 + (w[1].1[0] as f64 - w[0].1[0] as f64) * u).round()
                                as u8,
                            (w[0].1[1] as f64 + (w[1].1[1] as f64 - w[0].1[1] as f64) * u).round()
                                as u8,
                            (w[0].1[2] as f64 + (w[1].1[2] as f64 - w[0].1[2] as f64) * u).round()
                                as u8,
                            (w[0].1[3] as f64 + (w[1].1[3] as f64 - w[0].1[3] as f64) * u).round()
                                as u8,
                        ];
                        break;
                    }
                }
                let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                self.blend_pixel(px, py, col, cov);
            }
        }
    }
}

fn round_rect_sdf(px: f64, py: f64, x: f64, y: f64, w: f64, h: f64, r: f64) -> f64 {
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let qx = (px - cx).abs() - (w * 0.5 - r);
    let qy = (py - cy).abs() - (h * 0.5 - r);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

fn paint_crescent_emblem(cv: &mut Canvas, cx: f64, cy: f64, r: f64) {
    let w = r * 2.0;
    let h = r * 1.7;
    let x0 = cx - r;
    let y0 = cy - r * 0.85;
    cv.fill_round_rect(x0, y0, w, h, 6.0, [0xc8, 0x1d, 0x25, 255]);
    cv.stroke_round_rect(x0, y0, w, h, 6.0, 2.8, [0xf8, 0xe6, 0xb0, 255]);

    // White crescent
    cv.fill_circle(cx - r * 0.14, cy, r * 0.46, [0xff, 0xff, 0xff, 255]);
    cv.fill_circle(cx + r * 0.04, cy, r * 0.36, [0xc8, 0x1d, 0x25, 255]);

    // Five-point star
    let sx = cx + r * 0.44;
    let sy = cy;
    let sr = r * 0.22;
    let mut pts = Vec::with_capacity(10);
    for i in 0..10 {
        let rad = if i % 2 == 0 { sr } else { sr * 0.42 };
        let a = (i as f64 / 10.0) * PI * 2.0 - PI * 0.5;
        pts.push((sx + a.cos() * rad, sy + a.sin() * rad));
    }
    cv.fill_polygon(&pts, [0xff, 0xff, 0xff, 255]);
}

fn paint_german_cross_emblem(cv: &mut Canvas, cx: f64, cy: f64, r: f64) {
    let w = r * 2.0;
    let h = r * 1.7;
    let x0 = cx - r;
    let y0 = cy - r * 0.85;
    cv.fill_round_rect(x0, y0, w, h, 6.0, [0x32, 0x38, 0x3e, 255]);
    cv.stroke_round_rect(x0, y0, w, h, 6.0, 2.8, [0xd4, 0xdb, 0xe0, 255]);

    let w_outer = r * 0.42;
    let len_outer = r * 0.68;
    cv.fill_round_rect(
        cx - len_outer,
        cy - w_outer * 0.5,
        len_outer * 2.0,
        w_outer,
        0.5,
        [0xf2, 0xf5, 0xf8, 255],
    );
    cv.fill_round_rect(
        cx - w_outer * 0.5,
        cy - len_outer * 0.85,
        w_outer,
        len_outer * 1.7,
        0.5,
        [0xf2, 0xf5, 0xf8, 255],
    );

    let w_inner = r * 0.24;
    let len_inner = r * 0.52;
    cv.fill_round_rect(
        cx - len_inner,
        cy - w_inner * 0.5,
        len_inner * 2.0,
        w_inner,
        0.5,
        [0x18, 0x1b, 0x1e, 255],
    );
    cv.fill_round_rect(
        cx - w_inner * 0.5,
        cy - len_inner * 0.85,
        w_inner,
        len_inner * 1.7,
        0.5,
        [0x18, 0x1b, 0x1e, 255],
    );
}

fn paint_british_crown_emblem(cv: &mut Canvas, cx: f64, cy: f64, r: f64) {
    let w = r * 2.0;
    let h = r * 1.7;
    let x0 = cx - r;
    let y0 = cy - r * 0.85;
    cv.fill_round_rect(x0, y0, w, h, 6.0, [0x19, 0x3b, 0x78, 255]);

    // Clipped diagonal white saltire & crimson cross
    cv.fill_sdf(x0, y0, x0 + w, y0 + h, [0xff, 0xff, 0xff, 255], |px, py| {
        let clip = round_rect_sdf(px, py, x0, y0, w, h, 6.0);
        let dx = px - cx;
        let dy = py - cy;
        let len = r.hypot(r * 0.85);
        let d1 = ((dx * (r * 0.85) - dy * r).abs() / len) - 4.2;
        let d2 = ((dx * (r * 0.85) + dy * r).abs() / len) - 4.2;
        clip.max(d1.min(d2))
    });
    cv.fill_sdf(x0, y0, x0 + w, y0 + h, [0xcf, 0x24, 0x2b, 255], |px, py| {
        let clip = round_rect_sdf(px, py, x0, y0, w, h, 6.0);
        let d1 = (py - cy).abs() - 2.4;
        let d2 = (px - cx).abs() - 2.4;
        clip.max(d1.min(d2))
    });

    // Gold heraldic crown centerpiece
    let crown = [
        (cx - r * 0.45, cy + r * 0.25),
        (cx - r * 0.52, cy - r * 0.25),
        (cx - r * 0.18, cy + r * 0.02),
        (cx, cy - r * 0.36),
        (cx + r * 0.18, cy + r * 0.02),
        (cx + r * 0.52, cy - r * 0.25),
        (cx + r * 0.45, cy + r * 0.25),
    ];
    cv.fill_polygon(&crown, [0xf2, 0xc1, 0x44, 255]);
    cv.stroke_round_rect(x0, y0, w, h, 6.0, 2.8, [0xf2, 0xd3, 0x7a, 255]);
}

fn paint_division_card(
    cv: &mut Canvas,
    ox: f64,
    oy: f64,
    faction: usize,
    unit_type: usize,
    card_style: usize,
) {
    let cx = ox + 128.0;
    let cy = oy + 128.0;

    let (top, body_a, body_b, border, ink, nat_accent): ([u8; 4], [u8; 4], [u8; 4], [u8; 4], [u8; 4], [u8; 4]) =
        match faction {
            0 => (
                [0xb8, 0x21, 0x28, 255],
                [0xe8, 0xd8, 0xb6, 255],
                [0xcb, 0xb4, 0x86, 255],
                [0x6b, 0x18, 0x1b, 255],
                [0x26, 0x18, 0x12, 255],
                [0xe2, 0x3a, 0x3f, 255],
            ),
            1 => (
                [0x3a, 0x42, 0x49, 255],
                [0xcf, 0xd6, 0xdc, 255],
                [0x9a, 0xa4, 0xad, 255],
                [0x20, 0x26, 0x2b, 255],
                [0x14, 0x18, 0x1c, 255],
                [0x6c, 0x7a, 0x86, 255],
            ),
            _ => (
                [0x1d, 0x44, 0x86, 255],
                [0xeb, 0xe2, 0xcc, 255],
                [0xcb, 0xbd, 0x9c, 255],
                [0x14, 0x2a, 0x52, 255],
                [0x19, 0x22, 0x34, 255],
                [0x32, 0x68, 0xc2, 255],
            ),
        };

    // Beveled drop shadow
    cv.fill_round_rect(
        cx - 88.0 + 8.0,
        cy - 58.0 + 12.0,
        176.0,
        116.0,
        14.0,
        [14, 16, 18, 122],
    );

    // Main card body with vertical linear gradient + clipped top banner
    let x0 = (cx - 90.0).floor() as i32;
    let y0 = (cy - 60.0).floor() as i32;
    let x1 = (cx + 90.0).ceil() as i32;
    let y1 = (cy + 60.0).ceil() as i32;
    for py in y0..=y1 {
        let fy = py as f64 + 0.5;
        let t = ((fy - (cy - 58.0)) / 116.0).clamp(0.0, 1.0);
        let grad_col = [
            (body_a[0] as f64 + (body_b[0] as f64 - body_a[0] as f64) * t).round() as u8,
            (body_a[1] as f64 + (body_b[1] as f64 - body_a[1] as f64) * t).round() as u8,
            (body_a[2] as f64 + (body_b[2] as f64 - body_a[2] as f64) * t).round() as u8,
            255,
        ];
        for px in x0..=x1 {
            let fx = px as f64 + 0.5;
            let d = round_rect_sdf(fx, fy, cx - 88.0, cy - 58.0, 176.0, 116.0, 14.0);
            if d < 0.75 {
                let cov = (0.5 - d).clamp(0.0, 1.0);
                let col = if fy < cy - 58.0 + 8.0 {
                    [
                        ((top[0] as f64) * 0.84 + 255.0 * 0.16).round() as u8,
                        ((top[1] as f64) * 0.84 + 255.0 * 0.16).round() as u8,
                        ((top[2] as f64) * 0.84 + 255.0 * 0.16).round() as u8,
                        255,
                    ]
                } else if fy < cy - 58.0 + 34.0 {
                    top
                } else {
                    grad_col
                };
                cv.blend_pixel(px, py, col, cov);
            }
        }
    }

    // Outer metallic border
    cv.stroke_round_rect(cx - 88.0, cy - 58.0, 176.0, 116.0, 14.0, 5.2, border);

    // Nation emblem
    match faction {
        0 => paint_crescent_emblem(cv, cx - 54.0, cy - 28.0, 24.0),
        1 => paint_german_cross_emblem(cv, cx - 54.0, cy - 28.0, 24.0),
        _ => paint_british_crown_emblem(cv, cx - 54.0, cy - 28.0, 24.0),
    }

    // Division size marker ("XX" or "XXX") in top header bar at (cx + 28, cy - 40)
    let count_x = if card_style != 0 { 3 } else { 2 };
    let spacing = 13.0;
    let start_x = cx + 28.0 - ((count_x - 1) as f64 * spacing) * 0.5;
    let gy = cy - 41.0;
    for i in 0..count_x {
        let gx = start_x + (i as f64) * spacing;
        cv.draw_line(gx - 4.2, gy - 5.5, gx + 4.2, gy + 5.5, 2.8, [0xf8, 0xf6, 0xee, 255]);
        cv.draw_line(gx + 4.2, gy - 5.5, gx - 4.2, gy + 5.5, 2.8, [0xf8, 0xf6, 0xee, 255]);
    }

    // Inner NATO tactical box at (cx + 28, cy + 8), 84x50
    let bx = cx + 28.0;
    let by = cy + 8.0;
    let bw = 84.0;
    let bh = 50.0;
    cv.fill_round_rect(
        bx - bw * 0.5,
        by - bh * 0.5,
        bw,
        bh,
        5.0,
        [255, 252, 244, 184],
    );
    cv.stroke_round_rect(bx - bw * 0.5, by - bh * 0.5, bw, bh, 5.0, 4.0, ink);

    // NATO unit symbol geometry
    match unit_type {
        0 => {
            cv.draw_line(
                bx - bw * 0.45,
                by - bh * 0.45,
                bx + bw * 0.45,
                by + bh * 0.45,
                4.2,
                ink,
            );
            cv.draw_line(
                bx + bw * 0.45,
                by - bh * 0.45,
                bx - bw * 0.45,
                by + bh * 0.45,
                4.2,
                ink,
            );
        }
        1 => {
            cv.stroke_round_rect(bx - 26.0, by - 13.0, 52.0, 26.0, 13.0, 4.2, ink);
        }
        2 => {
            cv.fill_circle(bx, by, 10.0, ink);
        }
        _ => {
            cv.draw_line(
                bx - bw * 0.45,
                by - bh * 0.45,
                bx + bw * 0.45,
                by + bh * 0.45,
                4.2,
                ink,
            );
            cv.draw_line(
                bx + bw * 0.45,
                by - bh * 0.45,
                bx - bw * 0.45,
                by + bh * 0.45,
                4.2,
                ink,
            );
            cv.stroke_round_rect(bx - 22.0, by - 11.0, 44.0, 22.0, 11.0, 4.2, ink);
        }
    }

    // Strength & Organization status bars along bottom edge
    cv.fill_round_rect(cx - 78.0, cy + 40.0, 156.0, 9.0, 1.0, [0x22, 0x26, 0x29, 255]);
    cv.fill_round_rect(cx - 77.0, cy + 41.0, 84.0, 7.0, 1.0, [0x4a, 0xd1, 0x64, 255]);
    cv.fill_round_rect(cx + 11.0, cy + 41.0, 64.0, 7.0, 1.0, [0xe5, 0xa9, 0x3b, 255]);

    // Veteran regiment pip beside emblem
    cv.fill_circle(cx - 54.0, cy + 16.0, 8.4, nat_accent);
    cv.stroke_circle(cx - 54.0, cy + 16.0, 8.4, 2.2, [0xff, 0xf5, 0xd4, 255]);
}

fn paint_enemy_hq_marker(cv: &mut Canvas, ox: f64, oy: f64) {
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_circle(cx + 6.0, cy + 8.0, 48.0, [20, 10, 10, 107]);
    cv.fill_radial_gradient(
        cx,
        cy,
        42.0,
        &[
            (0.0, [0xdc, 0x3b, 0x32, 255]),
            (1.0, [0x7a, 0x19, 0x16, 255]),
        ],
    );
    cv.stroke_circle(cx, cy, 42.0, 4.8, [0xff, 0xd7, 0xa8, 255]);
    cv.draw_line(
        cx - 18.0,
        cy - 18.0,
        cx + 18.0,
        cy + 18.0,
        5.2,
        [0xff, 0xf6, 0xe8, 255],
    );
    cv.draw_line(
        cx + 18.0,
        cy - 18.0,
        cx - 18.0,
        cy + 18.0,
        5.2,
        [0xff, 0xf6, 0xe8, 255],
    );
}

fn paint_attack_arrow(cv: &mut Canvas, ox: f64, oy: f64, col_a: [u8; 4], col_b: [u8; 4]) {
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    let sh_pts = [
        (cx - 52.0, cy - 12.0 + 6.0),
        (cx + 12.0, cy - 12.0 + 6.0),
        (cx + 12.0, cy - 30.0 + 6.0),
        (cx + 60.0, cy + 6.0),
        (cx + 12.0, cy + 30.0 + 6.0),
        (cx + 12.0, cy + 12.0 + 6.0),
        (cx - 52.0, cy + 12.0 + 6.0),
    ];
    cv.fill_polygon(&sh_pts, [12, 16, 18, 90]);

    let pts = [
        (cx - 52.0, cy - 12.0),
        (cx + 12.0, cy - 12.0),
        (cx + 12.0, cy - 30.0),
        (cx + 60.0, cy),
        (cx + 12.0, cy + 30.0),
        (cx + 12.0, cy + 12.0),
        (cx - 52.0, cy + 12.0),
    ];
    let c_mid = [
        ((col_a[0] as u16 + col_b[0] as u16) / 2) as u8,
        ((col_a[1] as u16 + col_b[1] as u16) / 2) as u8,
        ((col_a[2] as u16 + col_b[2] as u16) / 2) as u8,
        255,
    ];
    cv.fill_polygon(&pts, c_mid);
    for i in 0..pts.len() {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[(i + 1) % pts.len()];
        cv.draw_line(x0, y0, x1, y1, 3.2, [0xff, 0xf5, 0xdc, 255]);
    }
}

fn paint_fort_depot(cv: &mut Canvas, ox: f64, oy: f64) {
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_circle(cx + 6.0, cy + 8.0, 42.0, [18, 20, 18, 97]);

    let mut pts = Vec::with_capacity(12);
    for i in 0..12 {
        let a = (i as f64 / 12.0) * PI * 2.0;
        let rad = if i % 2 == 0 { 42.0 } else { 26.0 };
        pts.push((cx + a.cos() * rad, cy + a.sin() * rad));
    }
    cv.fill_polygon(&pts, [0x5e, 0x6b, 0x58, 255]);
    for i in 0..pts.len() {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[(i + 1) % pts.len()];
        cv.draw_line(x0, y0, x1, y1, 4.0, [0x25, 0x2c, 0x22, 255]);
    }
    cv.fill_circle(cx, cy, 13.0, [0xe7, 0xd8, 0xb0, 255]);
    cv.fill_circle(cx, cy, 5.6, [0x33, 0x40, 0x2e, 255]);
}

fn paint_artillery_flash(cv: &mut Canvas, ox: f64, oy: f64) {
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    cv.fill_radial_gradient(
        cx,
        cy,
        68.0,
        &[
            (0.0, [255, 250, 210, 245]),
            (0.35, [255, 166, 48, 184]),
            (0.7, [220, 74, 24, 64]),
            (1.0, [220, 74, 24, 0]),
        ],
    );
}

fn paint_battle_smoke(cv: &mut Canvas, ox: f64, oy: f64) {
    let cx = ox + 128.0;
    let cy = oy + 128.0;
    for &(bx, by, r, col) in &[
        (0.0, 0.0, 60.0, [72, 68, 64, 87]),
        (-24.0, -16.0, 44.0, [130, 122, 114, 66]),
        (26.0, 12.0, 40.0, [92, 88, 84, 71]),
    ] {
        cv.fill_radial_gradient(
            cx + bx,
            cy + by,
            r,
            &[(0.0, col), (1.0, [80, 80, 80, 0])],
        );
    }
}

pub fn build_pocket_atlas() -> (i32, i32, Vec<u8>) {
    let w = POCKET_COLS * POCKET_CELL;
    let h = POCKET_ROWS * POCKET_CELL;
    let mut cv = Canvas::new(w, h);
    let cell = POCKET_CELL as f64;

    for faction in 0..3 {
        for u in 0..4 {
            paint_division_card(&mut cv, (u as f64) * cell, (faction as f64) * cell, faction, u, 0);
            paint_division_card(
                &mut cv,
                ((u + 4) as f64) * cell,
                (faction as f64) * cell,
                faction,
                u,
                1,
            );
        }
    }

    paint_enemy_hq_marker(&mut cv, 0.0 * cell, 3.0 * cell);
    paint_attack_arrow(
        &mut cv,
        1.0 * cell,
        3.0 * cell,
        [0xc8, 0x23, 0x2c, 255],
        [0xff, 0x64, 0x54, 255],
    );
    paint_attack_arrow(
        &mut cv,
        2.0 * cell,
        3.0 * cell,
        [0x3f, 0x49, 0x52, 255],
        [0xa8, 0xba, 0xc8, 255],
    );
    paint_attack_arrow(
        &mut cv,
        3.0 * cell,
        3.0 * cell,
        [0x1e, 0x4c, 0x9a, 255],
        [0x5c, 0xa6, 0xff, 255],
    );
    paint_fort_depot(&mut cv, 4.0 * cell, 3.0 * cell);
    paint_artillery_flash(&mut cv, 5.0 * cell, 3.0 * cell);
    paint_battle_smoke(&mut cv, 6.0 * cell, 3.0 * cell);

    (w as i32, h as i32, cv.px)
}

pub struct PocketRenderer {
    bg_vao: GLuint,
    prog_bg: GLuint,
    prog_inst: GLuint,
    inst_vao: GLuint,
    inst_vbo_quad: GLuint,
    inst_vbo_data: GLuint,
    noise_tex: GLuint,
    fa_tex: GLuint,
    fb_tex: GLuint,
    atlas_tex: GLuint,
    tex_gw: usize,
    tex_gh: usize,
}

impl PocketRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let mut bg_vao = 0;
        (gl.GenVertexArrays)(1, &mut bg_vao);

        let prog_bg = gl.compile_program(POCKET_VERT, POCKET_FRAG)?;
        let prog_inst = gl.compile_program(POCKET_INST_VERT, POCKET_INST_FRAG)?;

        let mut inst_vao = 0;
        let mut inst_vbo_quad = 0;
        let mut inst_vbo_data = 0;
        (gl.GenVertexArrays)(1, &mut inst_vao);
        (gl.GenBuffers)(1, &mut inst_vbo_quad);
        (gl.GenBuffers)(1, &mut inst_vbo_data);

        (gl.BindVertexArray)(inst_vao);
        let quad: [f32; 12] = [
            -0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, 0.5, -0.5, 0.5,
        ];
        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_vbo_quad);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            std::mem::size_of_val(&quad) as isize,
            quad.as_ptr() as *const c_void,
            GL_STATIC_DRAW,
        );
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, 8, std::ptr::null());

        let stride = std::mem::size_of::<InstSprite>() as i32;
        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_vbo_data);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, stride, std::ptr::null());
        (gl.VertexAttribDivisor)(1, 1);

        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 1, GL_FLOAT, GL_FALSE, stride, 8 as *const c_void);
        (gl.VertexAttribDivisor)(2, 1);

        (gl.EnableVertexAttribArray)(3);
        (gl.VertexAttribPointer)(3, 1, GL_FLOAT, GL_FALSE, stride, 12 as *const c_void);
        (gl.VertexAttribDivisor)(3, 1);

        (gl.EnableVertexAttribArray)(4);
        (gl.VertexAttribPointer)(4, 2, GL_FLOAT, GL_FALSE, stride, 16 as *const c_void);
        (gl.VertexAttribDivisor)(4, 1);

        (gl.EnableVertexAttribArray)(5);
        (gl.VertexAttribPointer)(5, 1, GL_FLOAT, GL_FALSE, stride, 24 as *const c_void);
        (gl.VertexAttribDivisor)(5, 1);
        (gl.BindVertexArray)(0);

        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        let (aw, ah, apx) = build_pocket_atlas();
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as i32,
            aw,
            ah,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            apx.as_ptr() as *const c_void,
        );
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            bg_vao,
            prog_bg,
            prog_inst,
            inst_vao,
            inst_vbo_quad,
            inst_vbo_data,
            noise_tex: 0,
            fa_tex: 0,
            fb_tex: 0,
            atlas_tex,
            tex_gw: 0,
            tex_gh: 0,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        for t in [self.noise_tex, self.fa_tex, self.fb_tex, self.atlas_tex] {
            if t != 0 {
                (gl.DeleteTextures)(1, &t);
            }
        }
        if self.prog_bg != 0 {
            (gl.DeleteProgram)(self.prog_bg);
        }
        if self.prog_inst != 0 {
            (gl.DeleteProgram)(self.prog_inst);
        }
        for b in [self.inst_vbo_quad, self.inst_vbo_data] {
            if b != 0 {
                (gl.DeleteBuffers)(1, &b);
            }
        }
        for v in [self.bg_vao, self.inst_vao] {
            if v != 0 {
                (gl.DeleteVertexArrays)(1, &v);
            }
        }
    }

    unsafe fn sync_textures(&mut self, gl: &GlFns, st: &mut PocketState) {
        if st.noise_tex_dirty || self.noise_tex == 0 {
            if self.noise_tex == 0 {
                (gl.GenTextures)(1, &mut self.noise_tex);
            }
            let (s, d) = make_noise_image(st.params.seed);
            (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as i32,
                s as i32,
                s as i32,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                d.as_ptr() as *const c_void,
            );
            (gl.GenerateMipmap)(GL_TEXTURE_2D);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);
            st.noise_tex_dirty = false;
        }

        if self.tex_gw != st.gw || self.tex_gh != st.gh || self.fa_tex == 0 || self.fb_tex == 0 {
            for t in [&mut self.fa_tex, &mut self.fb_tex] {
                if *t != 0 {
                    (gl.DeleteTextures)(1, t);
                }
                (gl.GenTextures)(1, t);
                (gl.BindTexture)(GL_TEXTURE_2D, *t);
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_R32F as i32,
                    st.gw as i32,
                    st.gh as i32,
                    0,
                    GL_RED,
                    GL_FLOAT,
                    std::ptr::null(),
                );
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
                (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            }
            self.tex_gw = st.gw;
            self.tex_gh = st.gh;
            st.fa_dirty = true;
            st.fb_dirty = true;
        }

        if st.fa_dirty && st.fa.len() == st.gw * st.gh {
            (gl.BindTexture)(GL_TEXTURE_2D, self.fa_tex);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
            (gl.TexSubImage2D)(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                st.gw as i32,
                st.gh as i32,
                GL_RED,
                GL_FLOAT,
                st.fa.as_ptr() as *const c_void,
            );
            st.fa_dirty = false;
        }

        if st.fb_dirty && st.fb.len() == st.gw * st.gh {
            (gl.BindTexture)(GL_TEXTURE_2D, self.fb_tex);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
            (gl.TexSubImage2D)(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                st.gw as i32,
                st.gh as i32,
                GL_RED,
                GL_FLOAT,
                st.fb.as_ptr() as *const c_void,
            );
            st.fb_dirty = false;
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, app: &mut AppState) {
        let pw = app.width_px.max(64);
        let ph = app.height_px.max(64);
        let css_w = app.css_w();
        let css_h = app.css_h();
        let scale = app.dpi_scale.max(0.5);

        app.pocket.resize(css_w, css_h);
        self.sync_textures(gl, &mut app.pocket);

        let st = &app.pocket;
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);

        // Pass 1: HOI4 / AoH Cartographic Strategy Map, Crimson Pocket Cross-Hatch, Frontline & Chevrons
        (gl.UseProgram)(self.prog_bg);
        (gl.BindVertexArray)(self.bg_vao);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_bg, "uNoise"), 0);

        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.fa_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_bg, "uFA"), 1);

        (gl.ActiveTexture)(GL_TEXTURE2);
        (gl.BindTexture)(GL_TEXTURE_2D, self.fb_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_bg, "uFB"), 2);

        (gl.Uniform2f)(gl.uniform_loc(self.prog_bg, "uRes"), css_w, css_h);
        (gl.Uniform2f)(
            gl.uniform_loc(self.prog_bg, "uGrid"),
            st.gw as f32,
            st.gh as f32,
        );
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "uScale"), scale);
        (gl.Uniform1f)(
            gl.uniform_loc(self.prog_bg, "uSeed"),
            ((st.params.seed.rem_euclid(997)) as f32) * 0.37,
        );
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "uStep"), STEP as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "uMix"), st.cur_mix as f32);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "uTime"), st.sim_time as f32);
        (gl.Uniform1f)(
            gl.uniform_loc(self.prog_bg, "uArrows"),
            st.params.arrows as f32,
        );
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "uPace"), st.params.pace as f32);
        (gl.Uniform1f)(
            gl.uniform_loc(self.prog_bg, "uSupply"),
            st.params.supply as f32,
        );
        (gl.Uniform1f)(
            gl.uniform_loc(self.prog_bg, "uDebug"),
            if st.params.debug { 1.0 } else { 0.0 },
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Pass 2: Instanced Star-Fort Depots, Pocket HQ Badges, Attack Arrows, TR/DE/EN Division Cards & Artillery Flashes
        let mut items: Vec<InstSprite> = Vec::with_capacity(256);
        let time_sec = st.sim_time;
        let pace = st.params.pace;

        // 1. Friendly star-fort depots in outer map
        if st.params.supply > 0.05 {
            for f in &st.forts {
                if f.grow <= 0.01 {
                    continue;
                }
                let ease = f.grow * f.grow * (3.0 - 2.0 * f.grow);
                items.push(InstSprite {
                    x: f.x as f32,
                    y: f.y as f32,
                    size: (36.0 * (0.3 + 0.7 * ease)) as f32,
                    rot: (f.rot * 0.2) as f32,
                    col: 4.0,
                    row: 3.0,
                    alpha: (ease.min(1.0) * (0.45 + 0.55 * st.params.supply)) as f32,
                });
            }
        }

        // 2. Pocket center garrison HQ warning badges on multi-icon pockets
        for hq in &st.pocket_hq {
            let pulse = 1.0 + 0.08 * (time_sec * 3.2).sin();
            items.push(InstSprite {
                x: hq.x as f32,
                y: hq.y as f32,
                size: (38.0 * pulse) as f32,
                rot: 0.0,
                col: 0.0,
                row: 3.0,
                alpha: 0.78,
            });
        }

        // 3. Attack thrust arrows + Division counters sorted along Y
        let mut ordered = st.divisions.clone();
        ordered.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
        for d in &ordered {
            let ang = d.in_y.atan2(d.in_x);
            let march_step = (time_sec * (1.8 + 1.4 * pace) + d.phase).sin() * 3.5;

            if st.params.arrows > 0.05 {
                let arrow_x = d.x + d.in_x * (27.0 + march_step);
                let arrow_y = d.y + d.in_y * (27.0 + march_step);
                items.push(InstSprite {
                    x: arrow_x as f32,
                    y: arrow_y as f32,
                    size: 30.0,
                    rot: ang as f32,
                    col: (1 + d.nation) as f32,
                    row: 3.0,
                    alpha: (0.3 + 0.7 * st.params.arrows).min(1.0) as f32,
                });
            }

            let sway = (time_sec * (1.2 + pace) + d.phase).sin() * 0.03;
            items.push(InstSprite {
                x: (d.x + d.in_x * (march_step * 0.45)) as f32,
                y: (d.y + d.in_y * (march_step * 0.45)) as f32,
                size: 58.0,
                rot: sway as f32,
                col: d.unit_col as f32,
                row: d.nation as f32,
                alpha: 0.98,
            });
        }

        // 4. Artillery flashes & tactical battle smoke
        if st.params.skirmish > 0.04 {
            for sk in &st.skirmishes {
                let t = sk.age / sk.dur;
                let sz = sk.size
                    * if sk.kind == 5 {
                        0.65 + t * 0.6
                    } else {
                        0.7 + t * 1.1
                    };
                items.push(InstSprite {
                    x: sk.x as f32,
                    y: sk.y as f32,
                    size: sz as f32,
                    rot: (sk.age * 1.8) as f32,
                    col: sk.kind as f32,
                    row: 3.0,
                    alpha: ((1.0 - t) * (0.4 + 0.6 * st.params.skirmish)) as f32,
                });
            }
        }

        if !items.is_empty() {
            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
            (gl.UseProgram)(self.prog_inst);
            (gl.BindVertexArray)(self.inst_vao);
            (gl.ActiveTexture)(GL_TEXTURE0);
            (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
            (gl.Uniform1i)(gl.uniform_loc(self.prog_inst, "uAtlas"), 0);
            (gl.Uniform2f)(gl.uniform_loc(self.prog_inst, "uRes"), css_w, css_h);
            (gl.Uniform2f)(
                gl.uniform_loc(self.prog_inst, "uGridAtlas"),
                POCKET_COLS as f32,
                POCKET_ROWS as f32,
            );

            (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_vbo_data);
            (gl.BufferData)(
                GL_ARRAY_BUFFER,
                (items.len() * std::mem::size_of::<InstSprite>()) as isize,
                items.as_ptr() as *const c_void,
                GL_DYNAMIC_DRAW,
            );
            (gl.DrawArraysInstanced)(GL_TRIANGLES, 0, 6, items.len() as i32);
            (gl.BindVertexArray)(0);
            (gl.Disable)(GL_BLEND);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fnv1a_fingerprint(rel_path: &str, bytes: &[u8]) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        let prime: u64 = 0x100000001b3;
        for &b in rel_path.as_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(prime);
        }
        h ^= 0;
        h = h.wrapping_mul(prime);
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(prime);
        }
        h ^= 0;
        h = h.wrapping_mul(prime);
        format!("{h:016x}")
    }

    #[test]
    fn verify_rust_anything_recorded_pocket_cases() {
        let recorded_meta: Value =
            serde_json::from_str(include_str!("recorded-from.json")).expect("recorded-from.json");
        let html_bytes = include_bytes!("../../../../prototype/pocket.html");
        let props_bytes = include_bytes!("../../../../prototype/pocket-props.js");
        assert_eq!(
            fnv1a_fingerprint("prototype/pocket.html", html_bytes),
            recorded_meta["sources"]["prototype/pocket.html"]
                .as_str()
                .unwrap()
        );
        assert_eq!(
            fnv1a_fingerprint("prototype/pocket-props.js", props_bytes),
            recorded_meta["sources"]["prototype/pocket-props.js"]
                .as_str()
                .unwrap()
        );

        let sample_icons = vec![
            Rect { x: 52.0, y: 44.0, w: 76.0, h: 90.0 },
            Rect { x: 52.0, y: 154.0, w: 76.0, h: 90.0 },
            Rect { x: 168.0, y: 44.0, w: 76.0, h: 90.0 },
            Rect { x: 460.0, y: 160.0, w: 76.0, h: 90.0 },
            Rect { x: 460.0, y: 270.0, w: 76.0, h: 90.0 },
            Rect { x: 880.0, y: 440.0, w: 76.0, h: 90.0 },
        ];
        let sample_names: Vec<String> = vec![
            "Belgelerim".into(),
            "Projeler".into(),
            "İndirilenler".into(),
            "Büro_Entwürfe".into(),
            "Übersicht.docx".into(),
            "Screenshots".into(),
        ];

        for line in include_str!("cases.jsonl").lines() {
            if line.trim().is_empty() {
                continue;
            }
            let c: Value = serde_json::from_str(line).expect("case json");
            let name = c["name"].as_str().unwrap();
            match name {
                "computeFieldPocket" => {
                    let mut st = PocketState::new(1280.0, 720.0);
                    st.set_icons(&sample_icons, &sample_names, true);
                    for s in c["expected"]["samples"].as_array().unwrap() {
                        let x = s["x"].as_f64().unwrap();
                        let y = s["y"].as_f64().unwrap();
                        let exp_s = s["s"].as_f64().unwrap();
                        let got_s = st.land_s(x, y);
                        assert!(
                            (got_s - exp_s).abs() < 1e-3,
                            "Pocket SDF mismatch at ({x},{y}): got {got_s}, expected {exp_s}"
                        );
                    }
                }
                "detectFolderNation" => {
                    for r in c["expected"]["results"].as_array().unwrap() {
                        let folder = r["name"].as_str().unwrap();
                        let salt = r["salt"].as_i64().unwrap() as i32;
                        let fac = r["faction"].as_i64().unwrap() as i32;
                        let exp_nat = r["nation"].as_u64().unwrap() as usize;
                        let got_nat = detect_folder_nation(folder, salt, fac);
                        assert_eq!(
                            got_nat, exp_nat,
                            "detect_folder_nation({folder:?}, {salt}, {fac}) mismatch"
                        );
                    }
                }
                "pocketForcesAndStep" => {
                    for run in c["expected"]["runs"].as_array().unwrap() {
                        let seed_val = run["seed"].as_i64().unwrap() as i32;
                        let mut st = PocketState::new(1280.0, 720.0);
                        st.params.seed = seed_val;
                        st.build_noise();
                        st.set_icons(&sample_icons, &sample_names, true);
                        st.rebuild_props();
                        for step in 0..30 {
                            st.step_props(0.05, (step as f64 + 1.0) * 0.05);
                        }
                        assert_eq!(st.forts.len(), run["fortCount"].as_u64().unwrap() as usize);
                        assert_eq!(
                            st.pocket_hq.len(),
                            run["hqCount"].as_u64().unwrap() as usize
                        );
                        assert_eq!(
                            st.divisions.len(),
                            run["divCount"].as_u64().unwrap() as usize
                        );
                        assert_eq!(
                            st.skirmishes.len(),
                            run["skirmishCount"].as_u64().unwrap() as usize
                        );
                        for (i, d_exp) in run["sampleDivs"].as_array().unwrap().iter().enumerate() {
                            let d = &st.divisions[i];
                            assert!(
                                (d.x - d_exp["x"].as_f64().unwrap()).abs() < 1e-2
                                    && (d.y - d_exp["y"].as_f64().unwrap()).abs() < 1e-2,
                                "Division {i} pos mismatch for seed={seed_val}"
                            );
                            assert_eq!(d.nation, d_exp["nation"].as_u64().unwrap() as usize);
                            assert_eq!(d.unit_col, d_exp["unitCol"].as_u64().unwrap() as usize);
                        }
                    }
                }
                other => panic!("Unknown pocket test case: {other}"),
            }
        }
    }
}
