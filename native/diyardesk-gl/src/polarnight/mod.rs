pub mod atlas;
pub mod shaders;

use crate::gl::*;
use crate::meadow::{hash2, Mulberry32};
use crate::state::{AppState, Rect};
use atlas::*;
use shaders::*;
use std::f64::consts::PI;
use std::ffi::c_void;

pub const PALETTES: [[[[f32; 3]; 2]; 3]; 3] = [
    // 0: Green / Pink / Cyan
    [
        [[0.353, 1.000, 0.706], [0.851, 1.000, 0.933]],
        [[1.000, 0.624, 0.831], [1.000, 0.851, 0.933]],
        [[0.373, 0.847, 1.000], [0.831, 0.957, 1.000]],
    ],
    // 1: Purple / Cyan / Pink
    [
        [[0.663, 0.482, 1.000], [0.902, 0.847, 1.000]],
        [[0.373, 0.847, 1.000], [0.831, 0.957, 1.000]],
        [[1.000, 0.624, 0.831], [1.000, 0.851, 0.933]],
    ],
    // 2: Gold / Green / Amber
    [
        [[1.000, 0.824, 0.341], [1.000, 0.937, 0.753]],
        [[0.353, 1.000, 0.706], [0.851, 1.000, 0.933]],
        [[1.000, 0.671, 0.420], [1.000, 0.863, 0.745]],
    ],
];

#[derive(Clone, Copy, PartialEq)]
pub struct PolarNightParams {
    pub sigma: f64,
    pub density: f64,
    pub glow: f64,
    pub flow: f64,
    pub palette: usize,
    pub snow: usize,
    pub meteor: bool,
    pub cabin: bool,
    pub seed: i32,
}

impl Default for PolarNightParams {
    fn default() -> Self {
        Self {
            sigma: 90.0,
            density: 0.6,
            glow: 0.75,
            flow: 1.0,
            palette: 0,
            snow: 110,
            meteor: true,
            cabin: true,
            seed: 424242,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PolarStar {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub big: bool,
    pub ph: f64,
    pub sp: f64,
    pub a: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct PolarRibbon {
    pub fy0: f64,
    pub a1: f64,
    pub f1: f64,
    pub s1: f64,
    pub a2: f64,
    pub f2: f64,
    pub s2: f64,
    pub ph: f64,
    pub ph2: f64,
    pub ph3: f64,
    pub th: f64,
    pub f3: f64,
    pub core: f64,
    pub am: f64,
}

impl PolarRibbon {
    #[inline]
    pub fn edge(&self, x: f64, sky_h: f64, t: f64, fl: f64) -> f64 {
        self.fy0 * sky_h
            + self.a1 * (x * self.f1 + t * self.s1 * fl + self.ph).sin()
            + self.a2 * (x * self.f2 + t * self.s2 * fl + self.ph2).sin()
    }

    #[inline]
    pub fn thickness(&self, x: f64, t: f64, fl: f64) -> f64 {
        self.th * (0.55 + 0.45 * (x * self.f3 + t * 0.35 * fl + self.ph3).sin())
    }
}

#[derive(Clone, Debug)]
pub struct PolarTree {
    pub x: f64,
    pub y: f64,
    pub v: usize,
    pub s: f64,
    pub r: f64,
    pub r2: f64,
    pub ph: f64,
    pub d0: f64,
    pub delay: f64,
    pub grow: f64,
    pub want: i32,
}

#[derive(Clone, Debug)]
pub struct PolarSnowFlake {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub vy: f64,
    pub ph: f64,
    pub sw: f64,
}

#[derive(Clone, Debug)]
pub struct PolarMeteor {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub t0: f64,
}

#[derive(Clone, Debug)]
pub struct PolarRipple {
    pub x: f64,
    pub y: f64,
    pub t0: f64,
}

pub struct PolarNightState {
    pub params: PolarNightParams,
    pub w: f64,
    pub h: f64,
    pub sky_h: f64,
    pub lake_top: f64,
    pub lake_h: f64,
    pub ground_top: f64,
    pub ground_h: f64,
    pub moon_x: f64,
    pub moon_y: f64,
    pub cabin_x: f64,
    pub cabin_on: bool,
    pub stars: Vec<PolarStar>,
    pub ribbons: [PolarRibbon; 3],
    pub ridge_refl: Vec<[f64; 3]>,
    pub trees: Vec<PolarTree>,
    pub snow: Vec<PolarSnowFlake>,
    pub meteors: Vec<PolarMeteor>,
    pub next_meteor: f64,
    pub ripples: Vec<PolarRipple>,
    pub icons: Vec<Rect>,
    pub icons_seen: bool,
    pub first_icons: bool,
    pub shore_dirty: bool,
    pub sim_time: f64,
    pub rng: Mulberry32,
}

impl Default for PolarNightState {
    fn default() -> Self {
        let params = PolarNightParams::default();
        let mut s = Self {
            params,
            w: 1920.0,
            h: 1080.0,
            sky_h: 864.0,
            lake_top: 864.0,
            lake_h: 216.0,
            ground_top: 702.0,
            ground_h: 162.0,
            moon_x: 600.0,
            moon_y: 200.0,
            cabin_x: 960.0,
            cabin_on: true,
            stars: Vec::new(),
            ribbons: [PolarRibbon {
                fy0: 0.3,
                a1: 40.0,
                f1: 0.0025,
                s1: 0.3,
                a2: 12.0,
                f2: 0.001,
                s2: -0.15,
                ph: 0.0,
                ph2: 0.0,
                ph3: 0.0,
                th: 100.0,
                f3: 0.003,
                core: 12.0,
                am: 1.0,
            }; 3],
            ridge_refl: Vec::new(),
            trees: Vec::new(),
            snow: Vec::new(),
            meteors: Vec::new(),
            next_meteor: 4.5,
            ripples: Vec::new(),
            icons: Vec::new(),
            icons_seen: true,
            first_icons: true,
            shore_dirty: true,
            sim_time: 0.0,
            rng: Mulberry32::new(params.seed ^ 0x13579bdf),
        };
        s.build_ribbons();
        s.resize(1920.0, 1080.0);
        s
    }
}

impl PolarNightState {
    pub fn layout(&mut self) {
        self.lake_h = (self.h * 0.2).clamp(120.0, 250.0);
        self.lake_top = self.h - self.lake_h;
        self.sky_h = self.lake_top;
        self.ground_h = (self.h * 0.15).clamp(96.0, 190.0);
        self.ground_top = self.lake_top - self.ground_h;

        let mut m_rng = Mulberry32::new(self.params.seed ^ 0x7eea11);
        self.moon_x = (0.2 + m_rng.next() * 0.6) * self.w;
        self.moon_y = (0.1 + m_rng.next() * 0.18) * self.sky_h;
        self.ridge_refl = compute_ridge(
            self.w,
            self.params.seed,
            3,
            0.0,
            self.ground_h * 0.55,
            1.0,
        );
    }

    pub fn build_stars(&mut self) {
        let mut rng = Mulberry32::new(self.params.seed ^ 0x51ed270b);
        let n = ((self.w * self.sky_h / 9000.0).round() as usize).clamp(60, 170);
        self.stars.clear();
        let max_y = (self.sky_h * 0.9).min(self.ground_top + 30.0);
        for _ in 0..n {
            self.stars.push(PolarStar {
                x: rng.next() * self.w,
                y: rng.next() * max_y,
                r: 0.5 + rng.next() * 1.2,
                big: rng.next() < 0.08,
                ph: rng.next() * 7.0,
                sp: 0.6 + rng.next() * 1.6,
                a: 0.45 + rng.next() * 0.5,
            });
        }
    }

    pub fn build_ribbons(&mut self) {
        let mut rng = Mulberry32::new(self.params.seed ^ 0x2c1b3c6d);
        for i in 0..3 {
            self.ribbons[i] = PolarRibbon {
                fy0: 0.3 + i as f64 * 0.14 + (rng.next() - 0.5) * 0.08,
                a1: 30.0 + rng.next() * 38.0,
                f1: 0.0022 + rng.next() * 0.0017,
                s1: 0.22 + rng.next() * 0.3,
                a2: 10.0 + rng.next() * 14.0,
                f2: 0.0009 + rng.next() * 0.0008,
                s2: -(0.1 + rng.next() * 0.2),
                ph: rng.next() * 7.0,
                ph2: rng.next() * 7.0,
                ph3: rng.next() * 7.0,
                th: 84.0 + rng.next() * 80.0,
                f3: 0.0024 + rng.next() * 0.002,
                core: 9.0 + rng.next() * 7.0,
                am: if i == 0 {
                    1.0
                } else if i == 1 {
                    0.72
                } else {
                    0.55
                },
            };
        }
    }

    pub fn build_trees(&mut self) {
        self.trees.clear();
        let mut rng = Mulberry32::new(self.params.seed ^ 0x11a5f3d);
        let mut y = self.ground_top + 10.0;
        while y < self.lake_top - 4.0 {
            let depth = (y - self.ground_top) / (self.lake_top - self.ground_top).max(1.0);
            let mut x = 10.0;
            while x < self.w {
                let jx = x + (rng.next() - 0.5) * 30.0;
                let jy = y + (rng.next() - 0.5) * 22.0;
                if jy <= self.lake_top - 6.0 {
                    let v = ((rng.next() * 3.0) as usize).min(2);
                    let s = (0.42 + (1.05 - 0.42) * depth) * (0.82 + rng.next() * 0.36);
                    let r = rng.next();
                    let r2 = rng.next();
                    let ph = rng.next() * 7.0;
                    let d0 = rng.next() * 0.5;
                    self.trees.push(PolarTree {
                        x: jx,
                        y: jy,
                        v,
                        s,
                        r,
                        r2,
                        ph,
                        d0,
                        delay: 0.0,
                        grow: 0.0,
                        want: 0,
                    });
                }
                x += 38.0;
            }
            y += 26.0;
        }
        if self.icons_seen {
            self.commit(true);
        }
    }

    pub fn icon_dist(&self, x: f64, y: f64) -> f64 {
        let mut d = f64::INFINITY;
        for ic in &self.icons {
            let ix = ic.x as f64;
            let iy = ic.y as f64;
            let iw = ic.w as f64;
            let ih = ic.h as f64;
            let dx = (ix - x).max(0.0).max(x - (ix + iw));
            let dy = (iy - y).max(0.0).max(y - (iy + ih));
            let q = dx.hypot(dy);
            if q < d {
                d = q;
            }
        }
        d
    }

    pub fn update_wants(&mut self) {
        if !self.icons_seen {
            for t in &mut self.trees {
                t.want = 0;
            }
            return;
        }
        let density = self.params.density;
        let sigma = self.params.sigma;
        for idx in 0..self.trees.len() {
            let plantable = self.trees[idx].r2 <= density;
            let thr = sigma * (0.72 + 0.4 * self.trees[idx].r);
            let d = if plantable {
                self.icon_dist(self.trees[idx].x, self.trees[idx].y)
            } else {
                -1.0
            };
            let hyst = if self.trees[idx].want != 0 { -12.0 } else { 12.0 };
            self.trees[idx].want = if plantable && d > thr + hyst { 1 } else { 0 };
        }
    }

    pub fn place_cabin(&mut self) {
        self.cabin_on = self.params.cabin;
        if !self.cabin_on {
            self.cabin_x = 0.0;
            return;
        }
        let spots = [0.14 * self.w, 0.5 * self.w, 0.86 * self.w];
        let mut best = 0usize;
        let mut bd = -1.0f64;
        for (i, &sx) in spots.iter().enumerate() {
            let d = self.icon_dist(sx, self.lake_top - 40.0);
            if d > bd {
                bd = d;
                best = i;
            }
        }
        self.cabin_x = spots[best];
    }

    pub fn commit(&mut self, from_empty: bool) {
        self.update_wants();
        if from_empty {
            for t in &mut self.trees {
                t.grow = 0.0;
                t.delay = t.d0;
            }
        }
        self.place_cabin();
    }

    pub fn sync_snow(&mut self) {
        let want = self.params.snow;
        while self.snow.len() > want {
            self.snow.pop();
        }
        while self.snow.len() < want {
            let r = 0.8 + self.rng.next() * 1.5;
            let x = self.rng.next() * self.w;
            let y = self.rng.next() * self.h;
            let vy = (13.0 + self.rng.next() * 20.0) * r;
            let ph = self.rng.next() * 7.0;
            let sw = 5.0 + self.rng.next() * 14.0;
            self.snow.push(PolarSnowFlake {
                x,
                y,
                r,
                vy,
                ph,
                sw,
            });
        }
    }

    pub fn resize(&mut self, w_f32: f32, h_f32: f32) {
        let w = (w_f32 as f64).max(64.0);
        let h = (h_f32 as f64).max(64.0);
        if (self.w - w).abs() < 0.5 && (self.h - h).abs() < 0.5 && !self.trees.is_empty() {
            return;
        }
        self.w = w;
        self.h = h;
        self.layout();
        self.build_stars();
        self.build_trees();
        self.snow.clear();
        self.sync_snow();
        self.place_cabin();
        self.shore_dirty = true;
    }

    pub fn reseed(&mut self, new_seed: i32) {
        self.params.seed = new_seed;
        self.rng = Mulberry32::new(new_seed ^ 0x13579bdf);
        self.layout();
        self.build_stars();
        self.build_ribbons();
        self.build_trees();
        self.place_cabin();
        self.shore_dirty = true;
    }

    pub fn sync_params(&mut self, next: PolarNightParams) {
        let seed_changed = next.seed != self.params.seed;
        let forest_changed = (next.sigma - self.params.sigma).abs() > 1e-4
            || (next.density - self.params.density).abs() > 1e-4;
        let snow_changed = next.snow != self.params.snow;
        let cabin_changed = next.cabin != self.params.cabin;
        self.params = next;
        if seed_changed {
            self.reseed(next.seed);
            return;
        }
        if snow_changed {
            self.sync_snow();
        }
        if cabin_changed {
            self.place_cabin();
        }
        if forest_changed && self.icons_seen {
            self.update_wants();
            self.place_cabin();
        }
    }

    pub fn set_icons(&mut self, rects: &[Rect], vis: bool) {
        self.icons.clear();
        if vis {
            self.icons.extend_from_slice(rects);
        }
        self.icons_seen = true;
        let first = self.first_icons;
        self.commit(first);
        self.first_icons = false;
    }

    pub fn add_ripple(&mut self, x: f64, y: f64) {
        if y <= self.lake_top + 8.0 {
            return;
        }
        self.ripples.push(PolarRipple {
            x,
            y,
            t0: self.sim_time,
        });
        if self.ripples.len() > 6 {
            self.ripples.remove(0);
        }
    }

    pub fn step_trees(&mut self, dt: f64) {
        for t in &mut self.trees {
            let target = t.want as f64;
            if (t.grow - target).abs() < 1e-6 {
                continue;
            }
            if t.delay > 0.0 {
                t.delay -= dt;
                continue;
            }
            t.grow += (if t.want != 0 { 1.0 } else { -1.0 }) * dt * 1.5;
            t.grow = t.grow.clamp(0.0, 1.0);
            if (t.grow - target).abs() < 0.01 {
                t.grow = target;
            }
        }
    }

    pub fn step(&mut self, dt_f32: f32) {
        let dt = (dt_f32 as f64).min(0.1);
        self.sim_time += dt;
        let t = self.sim_time;
        let now_ms = t * 1000.0;

        self.step_trees(dt);

        // Snow
        let fs = 0.5 + 0.9 * self.params.flow;
        let w = self.w;
        let h = self.h;
        for f in &mut self.snow {
            f.y += f.vy * fs * dt;
            f.x += (now_ms * 0.0007 + f.ph).sin() * f.sw * fs * dt;
            if f.y > h + 4.0 {
                f.y = -4.0;
                f.x = self.rng.next() * w;
            }
            if f.x > w + 6.0 {
                f.x -= w + 12.0;
            } else if f.x < -6.0 {
                f.x += w + 12.0;
            }
        }

        // Meteors
        if self.params.meteor && t > self.next_meteor && self.meteors.len() < 2 {
            let mx = self.w * (0.15 + self.rng.next() * 0.7);
            let my = self.sky_h * (0.05 + self.rng.next() * 0.25);
            let vx = -(200.0 + self.rng.next() * 160.0);
            let vy = 90.0 + self.rng.next() * 70.0;
            self.meteors.push(PolarMeteor {
                x: mx,
                y: my,
                vx,
                vy,
                t0: t,
            });
            self.next_meteor = t + 7.0 + self.rng.next() * 9.0;
        }
        self.meteors.retain(|m| t - m.t0 < 0.9);
        self.ripples.retain(|r| t - r.t0 < 1.8);
    }
}

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

pub struct PolarNightRenderer {
    col_prog: GLuint,
    u_col_res: GLint,
    u_col_flipy: GLint,
    col_vao: GLuint,
    col_vbo: GLuint,
    col_verts: Vec<ColorVert>,

    tex_prog: GLuint,
    u_tex_sampler: GLint,
    u_tex_res: GLint,
    u_tex_flipy: GLint,
    tex_vao: GLuint,
    tex_vbo: GLuint,
    tex_verts: Vec<TexVert>,

    sky_fbo: GLuint,
    sky_tex: GLuint,
    sky_w: i32,
    sky_h: i32,

    shore_tex: GLuint,
    shore_w: usize,
    shore_h: usize,

    tree_tex: [GLuint; 3],
    moon_tex: GLuint,
    cabin_tex: GLuint,
    glow_tex: GLuint,
    streak_tex: GLuint,
}

unsafe fn upload_sprite_tex(gl: &GlFns, img: &SpriteImage, repeat_x: bool) -> GLuint {
    let mut tex = 0;
    (gl.GenTextures)(1, &mut tex);
    (gl.BindTexture)(GL_TEXTURE_2D, tex);
    (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
    (gl.TexImage2D)(
        GL_TEXTURE_2D,
        0,
        GL_RGBA8 as GLint,
        img.w as GLsizei,
        img.h as GLsizei,
        0,
        GL_RGBA,
        GL_UNSIGNED_BYTE,
        img.rgba.as_ptr() as *const c_void,
    );
    (gl.TexParameteri)(
        GL_TEXTURE_2D,
        GL_TEXTURE_MIN_FILTER,
        GL_LINEAR_MIPMAP_LINEAR as GLint,
    );
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as GLint);
    (gl.TexParameteri)(
        GL_TEXTURE_2D,
        GL_TEXTURE_WRAP_S,
        if repeat_x {
            GL_REPEAT as GLint
        } else {
            GL_CLAMP_TO_EDGE as GLint
        },
    );
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as GLint);
    (gl.GenerateMipmap)(GL_TEXTURE_2D);
    tex
}

impl PolarNightRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let col_prog = gl.compile_program(PN_COLOR_VS, PN_COLOR_FS)?;
        let u_col_res = gl.uniform_loc(col_prog, "uRes");
        let u_col_flipy = gl.uniform_loc(col_prog, "uFlipY");

        let mut col_vao = 0;
        let mut col_vbo = 0;
        (gl.GenVertexArrays)(1, &mut col_vao);
        (gl.GenBuffers)(1, &mut col_vbo);
        (gl.BindVertexArray)(col_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, col_vbo);
        let col_stride = std::mem::size_of::<ColorVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, col_stride, std::ptr::null());
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(
            1,
            4,
            GL_FLOAT,
            GL_FALSE,
            col_stride,
            (2 * std::mem::size_of::<f32>()) as *const c_void,
        );
        (gl.BindVertexArray)(0);

        let tex_prog = gl.compile_program(PN_TEX_VS, PN_TEX_FS)?;
        let u_tex_sampler = gl.uniform_loc(tex_prog, "uTex");
        let u_tex_res = gl.uniform_loc(tex_prog, "uRes");
        let u_tex_flipy = gl.uniform_loc(tex_prog, "uFlipY");

        let mut tex_vao = 0;
        let mut tex_vbo = 0;
        (gl.GenVertexArrays)(1, &mut tex_vao);
        (gl.GenBuffers)(1, &mut tex_vbo);
        (gl.BindVertexArray)(tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, tex_vbo);
        let tex_stride = std::mem::size_of::<TexVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, tex_stride, std::ptr::null());
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(
            1,
            2,
            GL_FLOAT,
            GL_FALSE,
            tex_stride,
            (2 * std::mem::size_of::<f32>()) as *const c_void,
        );
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(
            2,
            1,
            GL_FLOAT,
            GL_FALSE,
            tex_stride,
            (4 * std::mem::size_of::<f32>()) as *const c_void,
        );
        (gl.BindVertexArray)(0);

        let tree_tex = [
            upload_sprite_tex(gl, &build_pine_sprite(0), false),
            upload_sprite_tex(gl, &build_pine_sprite(1), false),
            upload_sprite_tex(gl, &build_pine_sprite(2), false),
        ];
        let moon_tex = upload_sprite_tex(gl, &build_moon_sprite(), false);
        let cabin_tex = upload_sprite_tex(gl, &build_cabin_sprite(), false);
        let glow_tex = upload_sprite_tex(gl, &build_glow_sprite(), false);
        let streak_tex = upload_sprite_tex(gl, &build_streak_sprite(), true);

        let mut shore_tex = 0;
        (gl.GenTextures)(1, &mut shore_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, shore_tex);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as GLint);

        Ok(Self {
            col_prog,
            u_col_res,
            u_col_flipy,
            col_vao,
            col_vbo,
            col_verts: Vec::with_capacity(4096),
            tex_prog,
            u_tex_sampler,
            u_tex_res,
            u_tex_flipy,
            tex_vao,
            tex_vbo,
            tex_verts: Vec::with_capacity(1024),
            sky_fbo: 0,
            sky_tex: 0,
            sky_w: 0,
            sky_h: 0,
            shore_tex,
            shore_w: 0,
            shore_h: 0,
            tree_tex,
            moon_tex,
            cabin_tex,
            glow_tex,
            streak_tex,
        })
    }

    unsafe fn ensure_sky_fbo(&mut self, gl: &GlFns, w: i32, h: i32) {
        let w = w.max(16);
        let h = h.max(16);
        if self.sky_fbo != 0 && self.sky_w == w && self.sky_h == h {
            return;
        }
        if self.sky_tex != 0 {
            (gl.DeleteTextures)(1, &self.sky_tex);
            self.sky_tex = 0;
        }
        if self.sky_fbo != 0 {
            (gl.DeleteFramebuffers)(1, &self.sky_fbo);
            self.sky_fbo = 0;
        }
        (gl.GenFramebuffers)(1, &mut self.sky_fbo);
        (gl.GenTextures)(1, &mut self.sky_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, self.sky_tex);
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
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as GLint);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as GLint);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.sky_fbo);
        (gl.FramebufferTexture2D)(
            GL_FRAMEBUFFER,
            GL_COLOR_ATTACHMENT0,
            GL_TEXTURE_2D,
            self.sky_tex,
            0,
        );
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        self.sky_w = w;
        self.sky_h = h;
    }

    fn push_col_quad(
        verts: &mut Vec<ColorVert>,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        c_top: [f32; 4],
        c_bot: [f32; 4],
    ) {
        let v00 = ColorVert {
            x: x0,
            y: y0,
            r: c_top[0],
            g: c_top[1],
            b: c_top[2],
            a: c_top[3],
        };
        let v10 = ColorVert {
            x: x1,
            y: y0,
            r: c_top[0],
            g: c_top[1],
            b: c_top[2],
            a: c_top[3],
        };
        let v11 = ColorVert {
            x: x1,
            y: y1,
            r: c_bot[0],
            g: c_bot[1],
            b: c_bot[2],
            a: c_bot[3],
        };
        let v01 = ColorVert {
            x: x0,
            y: y1,
            r: c_bot[0],
            g: c_bot[1],
            b: c_bot[2],
            a: c_bot[3],
        };
        verts.extend_from_slice(&[v00, v10, v11, v00, v11, v01]);
    }

    fn push_col_disc(
        verts: &mut Vec<ColorVert>,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        col: [f32; 4],
        segs: usize,
    ) {
        let center = ColorVert {
            x: cx,
            y: cy,
            r: col[0],
            g: col[1],
            b: col[2],
            a: col[3],
        };
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * (PI as f32 * 2.0);
            let a1 = ((i + 1) as f32 / segs as f32) * (PI as f32 * 2.0);
            let p0 = ColorVert {
                x: cx + a0.cos() * rx,
                y: cy + a0.sin() * ry,
                r: col[0],
                g: col[1],
                b: col[2],
                a: col[3],
            };
            let p1 = ColorVert {
                x: cx + a1.cos() * rx,
                y: cy + a1.sin() * ry,
                r: col[0],
                g: col[1],
                b: col[2],
                a: col[3],
            };
            verts.extend_from_slice(&[center, p0, p1]);
        }
    }

    fn push_col_ring(
        verts: &mut Vec<ColorVert>,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        lw: f32,
        col: [f32; 4],
        segs: usize,
    ) {
        let hw = lw * 0.5;
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * (PI as f32 * 2.0);
            let a1 = ((i + 1) as f32 / segs as f32) * (PI as f32 * 2.0);
            let c0 = a0.cos();
            let s0 = a0.sin();
            let c1 = a1.cos();
            let s1 = a1.sin();
            let i0 = ColorVert {
                x: cx + c0 * (rx - hw),
                y: cy + s0 * (ry - hw),
                r: col[0],
                g: col[1],
                b: col[2],
                a: col[3],
            };
            let o0 = ColorVert {
                x: cx + c0 * (rx + hw),
                y: cy + s0 * (ry + hw),
                r: col[0],
                g: col[1],
                b: col[2],
                a: col[3],
            };
            let o1 = ColorVert {
                x: cx + c1 * (rx + hw),
                y: cy + s1 * (ry + hw),
                r: col[0],
                g: col[1],
                b: col[2],
                a: col[3],
            };
            let i1 = ColorVert {
                x: cx + c1 * (rx - hw),
                y: cy + s1 * (ry - hw),
                r: col[0],
                g: col[1],
                b: col[2],
                a: col[3],
            };
            verts.extend_from_slice(&[i0, o0, o1, i0, o1, i1]);
        }
    }

    fn push_col_line(
        verts: &mut Vec<ColorVert>,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        w: f32,
        c0: [f32; 4],
        c1: [f32; 4],
    ) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = dx.hypot(dy).max(1e-4);
        let nx = (-dy / len) * (w * 0.5);
        let ny = (dx / len) * (w * 0.5);
        let v0 = ColorVert {
            x: x0 - nx,
            y: y0 - ny,
            r: c0[0],
            g: c0[1],
            b: c0[2],
            a: c0[3],
        };
        let v1 = ColorVert {
            x: x0 + nx,
            y: y0 + ny,
            r: c0[0],
            g: c0[1],
            b: c0[2],
            a: c0[3],
        };
        let v2 = ColorVert {
            x: x1 + nx,
            y: y1 + ny,
            r: c1[0],
            g: c1[1],
            b: c1[2],
            a: c1[3],
        };
        let v3 = ColorVert {
            x: x1 - nx,
            y: y1 - ny,
            r: c1[0],
            g: c1[1],
            b: c1[2],
            a: c1[3],
        };
        verts.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
    }

    fn push_ribbon(
        verts: &mut Vec<ColorVert>,
        rib: &PolarRibbon,
        colors: &[[f32; 3]; 2],
        w: f64,
        sky_h: f64,
        t: f64,
        fl: f64,
        glow: f64,
        core_k: f64,
    ) {
        let step = 30.0;
        let mut pts = Vec::new();
        let mut x = -40.0;
        while x <= w + 40.0 {
            let yb = rib.edge(x, sky_h, t, fl);
            let yt = yb - rib.thickness(x, t, fl);
            pts.push((x as f32, yt as f32, yb as f32));
            x += step;
        }
        let c0 = colors[0];
        let g = glow as f32;
        let stops: [(f32, f32); 4] = [(0.0, 0.0), (0.55, 0.10 * g), (0.85, 0.30 * g), (1.0, 0.42 * g)];
        for seg in pts.windows(2) {
            let (x0, yt0, yb0) = seg[0];
            let (x1, yt1, yb1) = seg[1];
            for s in stops.windows(2) {
                let (u0, a0) = s[0];
                let (u1, a1) = s[1];
                let y00 = yt0 + (yb0 - yt0) * u0;
                let y01 = yt0 + (yb0 - yt0) * u1;
                let y10 = yt1 + (yb1 - yt1) * u0;
                let y11 = yt1 + (yb1 - yt1) * u1;
                let v00 = ColorVert {
                    x: x0,
                    y: y00,
                    r: c0[0],
                    g: c0[1],
                    b: c0[2],
                    a: a0,
                };
                let v10 = ColorVert {
                    x: x1,
                    y: y10,
                    r: c0[0],
                    g: c0[1],
                    b: c0[2],
                    a: a0,
                };
                let v11 = ColorVert {
                    x: x1,
                    y: y11,
                    r: c0[0],
                    g: c0[1],
                    b: c0[2],
                    a: a1,
                };
                let v01 = ColorVert {
                    x: x0,
                    y: y01,
                    r: c0[0],
                    g: c0[1],
                    b: c0[2],
                    a: a1,
                };
                verts.extend_from_slice(&[v00, v10, v11, v00, v11, v01]);
            }
        }

        if core_k > 0.01 {
            let c1 = colors[1];
            let ck = (glow * core_k) as f32;
            let col_outer = [c1[0], c1[1], c1[2], 0.26 * ck];
            let col_inner = [c1[0], c1[1], c1[2], 0.50 * ck];
            let w_outer = (rib.core * 2.8) as f32;
            let w_inner = (rib.core * 0.9) as f32;
            for seg in pts.windows(2) {
                let (x0, _, yb0) = seg[0];
                let (x1, _, yb1) = seg[1];
                Self::push_col_line(verts, x0, yb0, x1, yb1, w_outer, col_outer, col_outer);
                Self::push_col_line(verts, x0, yb0, x1, yb1, w_inner, col_inner, col_inner);
            }
        }
    }

    unsafe fn flush_col(&mut self, gl: &GlFns, res_w: f32, res_h: f32, flip_y: f32) {
        if self.col_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.col_prog);
        (gl.Uniform2f)(self.u_col_res, res_w, res_h);
        (gl.Uniform1f)(self.u_col_flipy, flip_y);
        (gl.BindVertexArray)(self.col_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.col_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.col_verts.len() * std::mem::size_of::<ColorVert>()) as isize,
            self.col_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.col_verts.len() as GLsizei);
        (gl.BindVertexArray)(0);
        self.col_verts.clear();
    }

    #[allow(clippy::too_many_arguments)]
    fn push_tex_quad_rot(
        verts: &mut Vec<TexVert>,
        cx: f32,
        cy: f32,
        ox0: f32,
        oy0: f32,
        ox1: f32,
        oy1: f32,
        rot: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        alpha: f32,
    ) {
        let c = rot.cos();
        let s = rot.sin();
        let tr = |lx: f32, ly: f32| (cx + lx * c - ly * s, cy + lx * s + ly * c);
        let (x00, y00) = tr(ox0, oy0);
        let (x10, y10) = tr(ox1, oy0);
        let (x11, y11) = tr(ox1, oy1);
        let (x01, y01) = tr(ox0, oy1);
        verts.extend_from_slice(&[
            TexVert {
                x: x00,
                y: y00,
                u: u0,
                v: v0,
                a: alpha,
            },
            TexVert {
                x: x10,
                y: y10,
                u: u1,
                v: v0,
                a: alpha,
            },
            TexVert {
                x: x11,
                y: y11,
                u: u1,
                v: v1,
                a: alpha,
            },
            TexVert {
                x: x00,
                y: y00,
                u: u0,
                v: v0,
                a: alpha,
            },
            TexVert {
                x: x11,
                y: y11,
                u: u1,
                v: v1,
                a: alpha,
            },
            TexVert {
                x: x01,
                y: y01,
                u: u0,
                v: v1,
                a: alpha,
            },
        ]);
    }

    unsafe fn flush_tex(&mut self, gl: &GlFns, tex: GLuint, res_w: f32, res_h: f32, flip_y: f32) {
        if self.tex_verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.tex_prog);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, tex);
        (gl.Uniform1i)(self.u_tex_sampler, 0);
        (gl.Uniform2f)(self.u_tex_res, res_w, res_h);
        (gl.Uniform1f)(self.u_tex_flipy, flip_y);
        (gl.BindVertexArray)(self.tex_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.tex_vbo);
        (gl.BufferData)(
            GL_ARRAY_BUFFER,
            (self.tex_verts.len() * std::mem::size_of::<TexVert>()) as isize,
            self.tex_verts.as_ptr() as *const c_void,
            GL_DYNAMIC_DRAW,
        );
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.tex_verts.len() as GLsizei);
        (gl.BindVertexArray)(0);
        self.tex_verts.clear();
    }

    pub unsafe fn render(&mut self, gl: &GlFns, state: &mut AppState) {
        let pn = &mut state.polarnight;
        let w = pn.w as f32;
        let h = pn.h as f32;
        let sky_h = pn.sky_h as f32;
        let lake_top = pn.lake_top as f32;
        let t = pn.sim_time;
        let now_ms = t * 1000.0;

        // Rebuild static shore layer texture if dirty
        let want_sw = (pn.w.round() as usize).max(16);
        let want_sh = (pn.h.round() as usize).max(16);
        if pn.shore_dirty || self.shore_w != want_sw || self.shore_h != want_sh {
            let img = build_shore_static_image(
                pn.w,
                pn.h,
                pn.params.seed,
                pn.ground_top,
                pn.ground_h,
                pn.lake_top,
            );
            (gl.BindTexture)(GL_TEXTURE_2D, self.shore_tex);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                img.w as GLsizei,
                img.h as GLsizei,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                img.rgba.as_ptr() as *const c_void,
            );
            self.shore_w = img.w;
            self.shore_h = img.h;
            pn.shore_dirty = false;
        }

        let dpr = state.dpi_scale.max(0.5);
        let fbo_w = ((pn.w * dpr as f64).round() as i32).max(16);
        let fbo_h = ((pn.sky_h * dpr as f64).round() as i32).max(16);
        self.ensure_sky_fbo(gl, fbo_w, fbo_h);

        // =====================================================================
        // Pass 1: Render Sky into sky_fbo (uFlipY = 1.0 so texture y=0 is top)
        // =====================================================================
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.sky_fbo);
        (gl.Viewport)(0, 0, self.sky_w, self.sky_h);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ONE_MINUS_SRC_ALPHA);

        // Sky gradient: 0 -> 0.4 -> 0.78 -> 1.0
        self.col_verts.clear();
        let c_s0 = [0x05 as f32 / 255.0, 0x0a as f32 / 255.0, 0x1c as f32 / 255.0, 1.0];
        let c_s1 = [0x0b as f32 / 255.0, 0x17 as f32 / 255.0, 0x33 as f32 / 255.0, 1.0];
        let c_s2 = [0x15 as f32 / 255.0, 0x2c as f32 / 255.0, 0x4c as f32 / 255.0, 1.0];
        let c_s3 = [0x24 as f32 / 255.0, 0x43 as f32 / 255.0, 0x5f as f32 / 255.0, 1.0];
        Self::push_col_quad(&mut self.col_verts, 0.0, 0.0, w, sky_h * 0.4, c_s0, c_s1);
        Self::push_col_quad(&mut self.col_verts, 0.0, sky_h * 0.4, w, sky_h * 0.78, c_s1, c_s2);
        Self::push_col_quad(&mut self.col_verts, 0.0, sky_h * 0.78, w, sky_h, c_s2, c_s3);

        // Stars
        for s in &pn.stars {
            let tw = 0.55 + 0.45 * (t * s.sp + s.ph).sin();
            let alpha = (s.a * tw) as f32;
            let sc = [0.933, 0.957, 1.0, alpha];
            Self::push_col_disc(&mut self.col_verts, s.x as f32, s.y as f32, s.r as f32, s.r as f32, sc, 8);
            if s.big && tw > 0.75 {
                let ca = (((tw - 0.75) * 2.4 * s.a) as f32).clamp(0.0, 1.0);
                let cc = [0.933, 0.957, 1.0, ca];
                let arm = (s.r * 5.0) as f32;
                let sx = s.x as f32;
                let sy = s.y as f32;
                Self::push_col_line(&mut self.col_verts, sx - arm, sy, sx + arm, sy, 1.0, cc, cc);
                Self::push_col_line(&mut self.col_verts, sx, sy - arm, sx, sy + arm, 1.0, cc, cc);
            }
        }
        self.flush_col(gl, w, sky_h, 1.0);

        // Moon
        self.tex_verts.clear();
        Self::push_tex_quad_rot(
            &mut self.tex_verts,
            pn.moon_x as f32,
            pn.moon_y as f32,
            -55.0,
            -55.0,
            55.0,
            55.0,
            0.0,
            0.0,
            0.0,
            1.0,
            1.0,
            1.0,
        );
        self.flush_tex(gl, self.moon_tex, w, sky_h, 1.0);

        // Aurora curtain + 3 ribbons
        let pal = &PALETTES[pn.params.palette.min(2)];
        let fl = 0.4 + 0.6 * pn.params.flow;
        let glow = pn.params.glow;
        let r0 = pn.ribbons[0];
        let curtain = PolarRibbon {
            fy0: r0.fy0 * 1.06,
            a1: r0.a1 * 1.5,
            f1: r0.f1 * 0.6,
            s1: r0.s1 * 0.6,
            a2: r0.a2,
            f2: r0.f2,
            s2: r0.s2,
            ph: r0.ph + 2.2,
            ph2: r0.ph2,
            ph3: r0.ph3,
            th: pn.sky_h * 0.55,
            f3: r0.f3,
            core: 0.0,
            am: 1.0,
        };
        Self::push_ribbon(&mut self.col_verts, &curtain, &pal[0], pn.w, pn.sky_h, t, fl, glow * 0.35, 0.0);
        for i in 0..3 {
            let rib = &pn.ribbons[i];
            Self::push_ribbon(
                &mut self.col_verts,
                rib,
                &pal[i],
                pn.w,
                pn.sky_h,
                t,
                fl,
                glow * rib.am,
                1.0,
            );
        }
        self.flush_col(gl, w, sky_h, 1.0);

        // Aurora vertical curtain streaks (additive 'lighter')
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE, GL_ZERO, GL_ONE);
        self.tex_verts.clear();
        let streak_a = (0.10 * glow) as f32;
        let off = ((t * 26.0 * fl) % 256.0) as f32;
        let off2 = ((-t * 14.0 * fl) % 256.0) as f32;
        Self::push_tex_quad_rot(
            &mut self.tex_verts,
            0.0,
            0.0,
            0.0,
            sky_h * 0.12,
            w,
            sky_h * 0.62,
            0.0,
            off / 256.0,
            0.0,
            (w + off) / 256.0,
            1.0,
            streak_a,
        );
        Self::push_tex_quad_rot(
            &mut self.tex_verts,
            0.0,
            0.0,
            0.0,
            sky_h * 0.20,
            w,
            sky_h * 0.62,
            0.0,
            (256.0 + off2) / 256.0,
            0.0,
            (w + 256.0 + off2) / 256.0,
            1.0,
            streak_a,
        );
        self.flush_tex(gl, self.streak_tex, w, sky_h, 1.0);

        // Meteors
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
        for m in &pn.meteors {
            let age = (t - m.t0).clamp(0.0, 0.9);
            let k = age / 0.9;
            let mx = (m.x + m.vx * age) as f32;
            let my = (m.y + m.vy * age) as f32;
            let tx = (m.x + m.vx * age - m.vx * 0.28) as f32;
            let ty = (m.y + m.vy * age - m.vy * 0.28) as f32;
            let fade = if k < 0.15 {
                k / 0.15
            } else {
                1.0 - (k - 0.15) / 0.85
            } as f32;
            Self::push_col_line(
                &mut self.col_verts,
                mx,
                my,
                tx,
                ty,
                2.0,
                [0.941, 0.973, 1.0, 0.9 * fade],
                [0.941, 0.973, 1.0, 0.0],
            );
        }
        self.flush_col(gl, w, sky_h, 1.0);

        // =====================================================================
        // Pass 2: Main Framebuffer 0 (uFlipY = -1.0)
        // =====================================================================
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, state.width_px.max(64), state.height_px.max(64));
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ONE_MINUS_SRC_ALPHA);

        // 1. Blit Sky (0..skyH)
        Self::push_tex_quad_rot(
            &mut self.tex_verts,
            0.0,
            0.0,
            0.0,
            0.0,
            w,
            sky_h,
            0.0,
            0.0,
            0.0,
            1.0,
            1.0,
            1.0,
        );
        self.flush_tex(gl, self.sky_tex, w, h, -1.0);

        // 2. Lake base gradient + inverted mountain reflection (k = 3)
        let c_l0 = [0x1b as f32 / 255.0, 0x3c as f32 / 255.0, 0x5e as f32 / 255.0, 1.0];
        let c_l1 = [0x13 as f32 / 255.0, 0x2c as f32 / 255.0, 0x49 as f32 / 255.0, 1.0];
        let c_l2 = [0x0c as f32 / 255.0, 0x1f as f32 / 255.0, 0x36 as f32 / 255.0, 1.0];
        let lake_mid = lake_top + (h - lake_top) * 0.4;
        Self::push_col_quad(&mut self.col_verts, 0.0, lake_top, w, lake_mid, c_l0, c_l1);
        Self::push_col_quad(&mut self.col_verts, 0.0, lake_mid, w, h, c_l1, c_l2);

        let c_refl = [9.0 / 255.0, 20.0 / 255.0, 38.0 / 255.0, 0.55];
        for seg in pn.ridge_refl.windows(2) {
            let x0 = seg[0][0] as f32;
            let y0 = (pn.lake_top + (-seg[0][1]) * 0.5 + 2.0) as f32;
            let x1 = seg[1][0] as f32;
            let y1 = (pn.lake_top + (-seg[1][1]) * 0.5 + 2.0) as f32;
            let v00 = ColorVert {
                x: x0,
                y: lake_top,
                r: c_refl[0],
                g: c_refl[1],
                b: c_refl[2],
                a: c_refl[3],
            };
            let v10 = ColorVert {
                x: x1,
                y: lake_top,
                r: c_refl[0],
                g: c_refl[1],
                b: c_refl[2],
                a: c_refl[3],
            };
            let v11 = ColorVert {
                x: x1,
                y: y1,
                r: c_refl[0],
                g: c_refl[1],
                b: c_refl[2],
                a: c_refl[3],
            };
            let v01 = ColorVert {
                x: x0,
                y: y0,
                r: c_refl[0],
                g: c_refl[1],
                b: c_refl[2],
                a: c_refl[3],
            };
            self.col_verts.extend_from_slice(&[v00, v10, v11, v00, v11, v01]);
        }
        self.flush_col(gl, w, h, -1.0);

        // 3. Lake reflected sky slices (10 slices, squash = 0.3, alpha = 0.55)
        let squash = 0.3f32;
        let slices = 10usize;
        let sh = (h - lake_top) / slices as f32;
        for j in 0..slices {
            let jf = j as f32;
            let dy = lake_top + jf * sh;
            let sy = sky_h - (jf * sh + sh) / squash;
            if sy < -sh / squash {
                continue;
            }
            let sx = ((t * 1.4 + j as f64 * 0.9).sin()
                * (2.0 + j as f64 * 0.5)
                * (0.3 + 0.7 * pn.params.flow)) as f32;
            let v_top = (sy.clamp(0.0, sky_h)) / sky_h;
            let v_bot = ((sy + (sh / squash).min(sky_h)).clamp(0.0, sky_h)) / sky_h;
            Self::push_tex_quad_rot(
                &mut self.tex_verts,
                0.0,
                0.0,
                sx,
                dy,
                sx + w,
                dy + sh + 0.6,
                0.0,
                0.0,
                v_bot,
                1.0,
                v_top,
                0.55,
            );
        }
        self.flush_tex(gl, self.sky_tex, w, h, -1.0);

        // 4. Deep water darkening + shimmer bars + moonlight water bars + ripples
        let c_deep = [10.0 / 255.0, 26.0 / 255.0, 46.0 / 255.0, 0.22];
        Self::push_col_quad(&mut self.col_verts, 0.0, lake_top, w, h, c_deep, c_deep);

        for i in 0..6 {
            let yy = pn.lake_top
                + (i as f64 + 0.4) * (pn.h - pn.lake_top) / 6.4
                + (t * (0.8 + i as f64 * 0.13) + i as f64 * 1.7).sin() * 2.0;
            let xw = 60.0 + hash2(i, 3, 0, pn.params.seed) * 150.0;
            let xx = hash2(i, 7, 0, pn.params.seed) * (pn.w - xw);
            let a = (0.05 + 0.05 * (t * 1.6 + i as f64 * 1.3).sin() + 0.03) as f32;
            let sc = [205.0 / 255.0, 228.0 / 255.0, 250.0 / 255.0, a];
            Self::push_col_quad(
                &mut self.col_verts,
                xx as f32,
                yy as f32,
                (xx + xw) as f32,
                (yy + 1.6) as f32,
                sc,
                sc,
            );
        }

        for i in 0..5 {
            let yy = pn.lake_top + 8.0 + i as f64 * (pn.h - pn.lake_top) / 9.0;
            let w2 = 16.0 + hash2(i, 9, 0, pn.params.seed) * 30.0;
            let xx = pn.moon_x - w2 * 0.5 + (t * 1.1 + i as f64 * 2.2).sin() * 12.0;
            let a = ((0.3 - i as f64 * 0.05) * (0.6 + 0.4 * (t * 2.0 + i as f64).sin())) as f32;
            let mc = [244.0 / 255.0, 238.0 / 255.0, 216.0 / 255.0, a];
            Self::push_col_quad(
                &mut self.col_verts,
                xx as f32,
                yy as f32,
                (xx + w2) as f32,
                (yy + 2.0) as f32,
                mc,
                mc,
            );
        }

        for r in &pn.ripples {
            let age = ((t - r.t0) / 1.8).clamp(0.0, 1.0) as f32;
            if age >= 1.0 {
                continue;
            }
            let c_o = [214.0 / 255.0, 232.0 / 255.0, 250.0 / 255.0, (1.0 - age) * 0.4];
            let c_i = [214.0 / 255.0, 232.0 / 255.0, 250.0 / 255.0, (1.0 - age) * 0.2];
            Self::push_col_ring(
                &mut self.col_verts,
                r.x as f32,
                r.y as f32,
                age * 60.0,
                age * 22.0,
                1.6,
                c_o,
                24,
            );
            Self::push_col_ring(
                &mut self.col_verts,
                r.x as f32,
                r.y as f32,
                age * 34.0,
                age * 12.0,
                1.6,
                c_i,
                20,
            );
        }
        self.flush_col(gl, w, h, -1.0);

        // 5. Shore static layer (mountains + snow plain + distant forest + snow mounds)
        Self::push_tex_quad_rot(
            &mut self.tex_verts,
            0.0,
            0.0,
            0.0,
            0.0,
            w,
            h,
            0.0,
            0.0,
            0.0,
            1.0,
            1.0,
            1.0,
        );
        self.flush_tex(gl, self.shore_tex, w, h, -1.0);

        // 6. Shore Cabin + warm window glow + chimney smoke
        if pn.cabin_on {
            let bx = pn.cabin_x as f32;
            let by = (pn.lake_top - 34.0) as f32;
            Self::push_tex_quad_rot(
                &mut self.tex_verts,
                bx,
                by,
                -70.0,
                -88.0,
                70.0,
                22.0,
                0.0,
                0.0,
                0.0,
                1.0,
                1.0,
                1.0,
            );
            self.flush_tex(gl, self.cabin_tex, w, h, -1.0);

            let fl_glow = (0.72
                + 0.16 * (now_ms * 0.007).sin()
                + 0.08 * (now_ms * 0.0131 + 1.7).sin()) as f32;
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE, GL_ZERO, GL_ONE);
            Self::push_tex_quad_rot(
                &mut self.tex_verts,
                bx,
                by,
                -40.0,
                -60.0,
                56.0,
                36.0,
                0.0,
                0.0,
                0.0,
                1.0,
                1.0,
                fl_glow,
            );
            self.flush_tex(gl, self.glow_tex, w, h, -1.0);

            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            for i in 0..3 {
                let age = ((now_ms * 0.001 * (0.5 + 0.12 * pn.params.flow) + i as f64 / 3.0) % 1.0)
                    as f32;
                let sx = bx + 23.0 + age * 26.0 + (age * 9.0 + i as f32).sin() * 4.0;
                let sy = by - 58.0 - age * 74.0;
                let sc = [215.0 / 255.0, 224.0 / 255.0, 238.0 / 255.0, (1.0 - age) * 0.26];
                Self::push_col_disc(
                    &mut self.col_verts,
                    sx,
                    sy,
                    3.5 + age * 11.0,
                    4.0 + age * 9.0,
                    sc,
                    14,
                );
            }
            self.flush_col(gl, w, h, -1.0);
        }

        // 7. Adaptive pine trees (3 variants)
        let fl_tree = 0.4 + 0.6 * pn.params.flow;
        for variant in 0..3 {
            self.tex_verts.clear();
            for tr in &pn.trees {
                if tr.v != variant || tr.grow <= 0.01 {
                    continue;
                }
                let k = if tr.grow < 1.0 {
                    1.0 - (1.0 - tr.grow).powf(2.2)
                } else {
                    1.0
                };
                let sw = ((now_ms * 0.0011 * fl_tree + tr.ph).sin() * 0.022 * fl_tree) as f32;
                let tw = (60.0 * tr.s * (0.6 + 0.4 * k)) as f32;
                let th = (96.0 * tr.s * k) as f32;
                let alpha = (0.25 + 0.75 * k) as f32;
                Self::push_tex_quad_rot(
                    &mut self.tex_verts,
                    tr.x as f32,
                    tr.y as f32,
                    -tw * 0.5,
                    -th,
                    tw * 0.5,
                    0.0,
                    sw,
                    0.0,
                    0.0,
                    1.0,
                    1.0,
                    alpha,
                );
            }
            self.flush_tex(gl, self.tree_tex[variant], w, h, -1.0);
        }

        // 8. Falling snow on top
        for f in &pn.snow {
            let a = (0.35 + 0.4 * f.r / 2.3) as f32;
            let sc = [238.0 / 255.0, 246.0 / 255.0, 1.0, a];
            Self::push_col_disc(
                &mut self.col_verts,
                f.x as f32,
                f.y as f32,
                f.r as f32,
                f.r as f32,
                sc,
                8,
            );
        }
        self.flush_col(gl, w, h, -1.0);
        (gl.Disable)(GL_BLEND);
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.col_prog != 0 {
            (gl.DeleteProgram)(self.col_prog);
            self.col_prog = 0;
        }
        if self.tex_prog != 0 {
            (gl.DeleteProgram)(self.tex_prog);
            self.tex_prog = 0;
        }
        if self.col_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.col_vao);
            self.col_vao = 0;
        }
        if self.col_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.col_vbo);
            self.col_vbo = 0;
        }
        if self.tex_vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.tex_vao);
            self.tex_vao = 0;
        }
        if self.tex_vbo != 0 {
            (gl.DeleteBuffers)(1, &self.tex_vbo);
            self.tex_vbo = 0;
        }
        if self.sky_tex != 0 {
            (gl.DeleteTextures)(1, &self.sky_tex);
            self.sky_tex = 0;
        }
        if self.sky_fbo != 0 {
            (gl.DeleteFramebuffers)(1, &self.sky_fbo);
            self.sky_fbo = 0;
        }
        if self.shore_tex != 0 {
            (gl.DeleteTextures)(1, &self.shore_tex);
            self.shore_tex = 0;
        }
        if self.tree_tex[0] != 0 {
            (gl.DeleteTextures)(3, self.tree_tex.as_ptr());
            self.tree_tex = [0; 3];
        }
        for t in [
            &mut self.moon_tex,
            &mut self.cabin_tex,
            &mut self.glow_tex,
            &mut self.streak_tex,
        ] {
            if *t != 0 {
                (gl.DeleteTextures)(1, t);
                *t = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fnv1a_fingerprint(rel_path: &str, bytes: &[u8]) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        let prime: u64 = 0x00000100000001b3;
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
        format!("{:016x}", h)
    }

    #[test]
    fn verify_rust_anything_recorded_polarnight_cases() {
        let rec_str = include_str!("recorded-from.json");
        let rec: Value = serde_json::from_str(rec_str).expect("recorded-from.json parse");
        let sources = rec["sources"].as_object().expect("sources object");

        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        for (rel, exp_hash_val) in sources {
            let exp_hash = exp_hash_val.as_str().unwrap();
            let bytes = std::fs::read(repo_root.join(rel)).expect("read prototype source");
            let actual_hash = fnv1a_fingerprint(rel, &bytes);
            assert_eq!(
                actual_hash, exp_hash,
                "Source fingerprint mismatch for {}",
                rel
            );
        }

        let cases_str = include_str!("cases.jsonl");
        let mut count = 0usize;
        for line in cases_str.lines().filter(|l| !l.trim().is_empty()) {
            let c: Value = serde_json::from_str(line).expect("parse case line");
            let name = c["name"].as_str().unwrap();
            let inp = &c["input"];
            let exp = &c["expected"];

            let seed = inp["seed"].as_i64().unwrap() as i32;
            let w = inp["W"].as_f64().unwrap();
            let h = inp["H"].as_f64().unwrap();
            let p = &inp["P"];

            let mut st = PolarNightState::default();
            st.params = PolarNightParams {
                sigma: p["sigma"].as_f64().unwrap(),
                density: p["density"].as_f64().unwrap(),
                glow: p["glow"].as_f64().unwrap(),
                flow: p["flow"].as_f64().unwrap(),
                palette: p["palette"].as_u64().unwrap() as usize,
                snow: p["snow"].as_u64().unwrap() as usize,
                meteor: p["meteor"].as_i64().unwrap() != 0,
                cabin: p["cabin"].as_i64().unwrap() != 0,
                seed,
            };
            st.w = w;
            st.h = h;
            st.icons.clear();
            for ic in inp["icons"].as_array().unwrap() {
                st.icons.push(Rect {
                    x: ic["x"].as_f64().unwrap() as f32,
                    y: ic["y"].as_f64().unwrap() as f32,
                    w: ic["w"].as_f64().unwrap() as f32,
                    h: ic["h"].as_f64().unwrap() as f32,
                });
            }
            st.icons_seen = true;
            st.layout();
            st.build_stars();
            st.build_ribbons();
            st.build_trees();
            st.place_cabin();

            let steps = inp["steps"].as_u64().unwrap() as usize;
            let dt = inp["dt"].as_f64().unwrap();
            for _ in 0..steps {
                st.step_trees(dt);
            }

            assert!((st.lake_h - exp["lakeH"].as_f64().unwrap()).abs() < 1e-3, "{}", name);
            assert!((st.lake_top - exp["lakeTop"].as_f64().unwrap()).abs() < 1e-3, "{}", name);
            assert!((st.ground_top - exp["groundTop"].as_f64().unwrap()).abs() < 1e-3, "{}", name);
            assert!((st.moon_x - exp["moonX"].as_f64().unwrap()).abs() < 1e-2, "{}", name);
            assert!((st.moon_y - exp["moonY"].as_f64().unwrap()).abs() < 1e-2, "{}", name);
            assert_eq!(st.cabin_on, exp["cabinOn"].as_bool().unwrap(), "{}", name);
            assert!((st.cabin_x - exp["cabinX"].as_f64().unwrap()).abs() < 1e-2, "{}", name);
            assert_eq!(st.stars.len(), exp["starCount"].as_u64().unwrap() as usize, "{}", name);

            for (i, es) in exp["firstStars"].as_array().unwrap().iter().enumerate() {
                let as_ = &st.stars[i];
                assert!((as_.x - es["x"].as_f64().unwrap()).abs() < 1e-2);
                assert!((as_.y - es["y"].as_f64().unwrap()).abs() < 1e-2);
                assert!((as_.r - es["r"].as_f64().unwrap()).abs() < 1e-3);
            }

            for (i, er) in exp["ribbons"].as_array().unwrap().iter().enumerate() {
                let ar = &st.ribbons[i];
                assert!((ar.fy0 - er["fy0"].as_f64().unwrap()).abs() < 1e-4);
                assert!((ar.a1 - er["a1"].as_f64().unwrap()).abs() < 1e-2);
                assert!((ar.th - er["th"].as_f64().unwrap()).abs() < 1e-2);
            }

            let ridge_far = compute_ridge(
                st.w,
                seed,
                1,
                st.ground_top - st.ground_h * 0.28,
                st.ground_h * 0.9,
                0.4,
            );
            for (k_idx, &sample_idx) in [0usize, 5, 10, 15].iter().enumerate() {
                let exp_y = exp["ridgeFarSamples"][k_idx].as_f64().unwrap();
                assert!((ridge_far[sample_idx][1] - exp_y).abs() < 1e-2);
            }

            assert_eq!(st.trees.len(), exp["treeCount"].as_u64().unwrap() as usize, "{}", name);
            let wanted = st.trees.iter().filter(|t| t.want == 1).count();
            let grown = st.trees.iter().filter(|t| t.grow > 0.5).count();
            assert_eq!(wanted, exp["wantedTrees"].as_u64().unwrap() as usize, "{}", name);
            assert_eq!(grown, exp["grownTrees"].as_u64().unwrap() as usize, "{}", name);

            for (i, et) in exp["firstTrees"].as_array().unwrap().iter().enumerate() {
                let at = &st.trees[i];
                assert_eq!(at.v, et["v"].as_u64().unwrap() as usize);
                assert_eq!(at.want, et["want"].as_i64().unwrap() as i32);
                assert!((at.x - et["x"].as_f64().unwrap()).abs() < 1e-2);
                assert!((at.y - et["y"].as_f64().unwrap()).abs() < 1e-2);
                assert!((at.grow - et["grow"].as_f64().unwrap()).abs() < 1e-2);
            }

            count += 1;
        }
        assert_eq!(count, 3);
    }
}
