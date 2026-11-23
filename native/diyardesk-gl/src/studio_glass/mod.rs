pub mod shaders;

use crate::gl::*;
use crate::shaders::FULLSCREEN_VERT;
use crate::state::{AppState, Rect};
use crate::text::TextRenderer;
use chrono::{Local, Timelike};
use shaders::*;
use std::ffi::c_void;

const MAX_GLASS: usize = 56;
const SLAT_COUNT: usize = 26;
const DUST_COUNT: usize = 90;

// =========================================================================
// Lightweight 3D Vector & Column-Major Mat4 Math
// =========================================================================
#[derive(Clone, Copy)]
struct Vec3(f32, f32, f32);

impl Vec3 {
    fn sub(self, o: Self) -> Self { Self(self.0 - o.0, self.1 - o.1, self.2 - o.2) }
    fn mul(self, s: f32) -> Self { Self(self.0 * s, self.1 * s, self.2 * s) }
    fn dot(self, o: Self) -> f32 { self.0 * o.0 + self.1 * o.1 + self.2 * o.2 }
    fn cross(self, o: Self) -> Self {
        Self(
            self.1 * o.2 - self.2 * o.1,
            self.2 * o.0 - self.0 * o.2,
            self.0 * o.1 - self.1 * o.0,
        )
    }
    fn norm(self) -> Self {
        let l = (self.0 * self.0 + self.1 * self.1 + self.2 * self.2).sqrt().max(1e-6);
        Self(self.0 / l, self.1 / l, self.2 / l)
    }
}

#[derive(Clone, Copy)]
struct Mat4([f32; 16]); // column-major for OpenGL

impl Mat4 {
    fn ident() -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    fn mul(self, b: Self) -> Self {
        let a = self.0;
        let b = b.0;
        let mut r = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                r[col * 4 + row] =
                    a[0 * 4 + row] * b[col * 4 + 0]
                    + a[1 * 4 + row] * b[col * 4 + 1]
                    + a[2 * 4 + row] * b[col * 4 + 2]
                    + a[3 * 4 + row] * b[col * 4 + 3];
            }
        }
        Self(r)
    }

    fn translate(x: f32, y: f32, z: f32) -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            x,   y,   z,   1.0,
        ])
    }

    fn scale(x: f32, y: f32, z: f32) -> Self {
        Self([
            x,   0.0, 0.0, 0.0,
            0.0, y,   0.0, 0.0,
            0.0, 0.0, z,   0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    fn rot_x(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self([
            1.0, 0.0, 0.0, 0.0,
            0.0, c,   s,   0.0,
            0.0, -s,  c,   0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    fn rot_y(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self([
            c,   0.0, -s,  0.0,
            0.0, 1.0, 0.0, 0.0,
            s,   0.0, c,   0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    fn rot_z(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self([
            c,   s,   0.0, 0.0,
            -s,  c,   0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    fn perspective(fov_deg: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_deg.to_radians() * 0.5).tan();
        let nf = 1.0 / (near - far);
        Self([
            f / aspect.max(0.1), 0.0, 0.0, 0.0,
            0.0, f, 0.0, 0.0,
            0.0, 0.0, (far + near) * nf, -1.0,
            0.0, 0.0, 2.0 * far * near * nf, 0.0,
        ])
    }

    fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = target.sub(eye).norm();
        let s = f.cross(up).norm();
        let u = s.cross(f);
        Self([
            s.0, u.0, -f.0, 0.0,
            s.1, u.1, -f.1, 0.0,
            s.2, u.2, -f.2, 0.0,
            -s.dot(eye), -u.dot(eye), f.dot(eye), 1.0,
        ])
    }

    fn transform_point(self, p: Vec3) -> Vec3 {
        let m = self.0;
        Vec3(
            m[0] * p.0 + m[4] * p.1 + m[8] * p.2 + m[12],
            m[1] * p.0 + m[5] * p.1 + m[9] * p.2 + m[13],
            m[2] * p.0 + m[6] * p.1 + m[10] * p.2 + m[14],
        )
    }

    fn transform_normal(self, n: Vec3) -> Vec3 {
        let m = self.0;
        Vec3(
            m[0] * n.0 + m[4] * n.1 + m[8] * n.2,
            m[1] * n.0 + m[5] * n.1 + m[9] * n.2,
            m[2] * n.0 + m[6] * n.1 + m[10] * n.2,
        )
        .norm()
    }
}

// =========================================================================
// Procedural 3D Vertex & Mesh Builder
// =========================================================================
#[repr(C)]
#[derive(Clone, Copy)]
struct Vert3D {
    pos: [f32; 3],
    norm: [f32; 3],
    uv: [f32; 2],
    col: [f32; 4], // rgb + matKind
    mat: [f32; 2], // roughness, metalness
}

struct MeshBuilder {
    verts: Vec<Vert3D>,
}

impl MeshBuilder {
    fn new() -> Self {
        Self { verts: Vec::with_capacity(2048) }
    }

    fn push_tri(&mut self, a: Vert3D, b: Vert3D, c: Vert3D) {
        self.verts.push(a);
        self.verts.push(b);
        self.verts.push(c);
    }

    fn push_quad(&mut self, a: Vert3D, b: Vert3D, c: Vert3D, d: Vert3D) {
        self.push_tri(a, b, c);
        self.push_tri(a, c, d);
    }

    fn add_plane(&mut self, xform: Mat4, w: f32, h: f32, col: [f32; 4], mat: [f32; 2]) {
        let hw = w * 0.5;
        let hh = h * 0.5;
        let n = xform.transform_normal(Vec3(0.0, 0.0, 1.0));
        let mk = |px: f32, py: f32, u: f32, v: f32| {
            let p = xform.transform_point(Vec3(px, py, 0.0));
            Vert3D { pos: [p.0, p.1, p.2], norm: [n.0, n.1, n.2], uv: [u, v], col, mat }
        };
        self.push_quad(
            mk(-hw, -hh, 0.0, 0.0),
            mk( hw, -hh, 1.0, 0.0),
            mk( hw,  hh, 1.0, 1.0),
            mk(-hw,  hh, 0.0, 1.0),
        );
    }

    fn add_box(&mut self, xform: Mat4, w: f32, h: f32, d: f32, col: [f32; 4], mat: [f32; 2]) {
        let (hw, hh, hd) = (w * 0.5, h * 0.5, d * 0.5);
        let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
            ([ 0.0,  0.0,  1.0], [[-hw, -hh,  hd], [ hw, -hh,  hd], [ hw,  hh,  hd], [-hw,  hh,  hd]]),
            ([ 0.0,  0.0, -1.0], [[ hw, -hh, -hd], [-hw, -hh, -hd], [-hw,  hh, -hd], [ hw,  hh, -hd]]),
            ([ 1.0,  0.0,  0.0], [[ hw, -hh,  hd], [ hw, -hh, -hd], [ hw,  hh, -hd], [ hw,  hh,  hd]]),
            ([-1.0,  0.0,  0.0], [[-hw, -hh, -hd], [-hw, -hh,  hd], [-hw,  hh,  hd], [-hw,  hh, -hd]]),
            ([ 0.0,  1.0,  0.0], [[-hw,  hh,  hd], [ hw,  hh,  hd], [ hw,  hh, -hd], [-hw,  hh, -hd]]),
            ([ 0.0, -1.0,  0.0], [[-hw, -hh, -hd], [ hw, -hh, -hd], [ hw, -hh,  hd], [-hw, -hh,  hd]]),
        ];
        for (fnorm, pts) in faces {
            let n = xform.transform_normal(Vec3(fnorm[0], fnorm[1], fnorm[2]));
            let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
            let mut v = [Vert3D { pos: [0.0; 3], norm: [n.0, n.1, n.2], uv: [0.0; 2], col, mat }; 4];
            for i in 0..4 {
                let p = xform.transform_point(Vec3(pts[i][0], pts[i][1], pts[i][2]));
                v[i].pos = [p.0, p.1, p.2];
                v[i].uv = uvs[i];
            }
            self.push_quad(v[0], v[1], v[2], v[3]);
        }
    }

    fn add_cylinder(&mut self, xform: Mat4, r_top: f32, r_bot: f32, h: f32, segs: usize, col: [f32; 4], mat: [f32; 2]) {
        let hh = h * 0.5;
        let slope = (r_bot - r_top) / h.max(0.001);
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();

            let n0 = xform.transform_normal(Vec3(c0, slope, s0).norm());
            let n1 = xform.transform_normal(Vec3(c1, slope, s1).norm());
            let pb0 = xform.transform_point(Vec3(c0 * r_bot, -hh, s0 * r_bot));
            let pb1 = xform.transform_point(Vec3(c1 * r_bot, -hh, s1 * r_bot));
            let pt1 = xform.transform_point(Vec3(c1 * r_top,  hh, s1 * r_top));
            let pt0 = xform.transform_point(Vec3(c0 * r_top,  hh, s0 * r_top));

            self.push_quad(
                Vert3D { pos: [pb0.0, pb0.1, pb0.2], norm: [n0.0, n0.1, n0.2], uv: [0.0, 0.0], col, mat },
                Vert3D { pos: [pb1.0, pb1.1, pb1.2], norm: [n1.0, n1.1, n1.2], uv: [1.0, 0.0], col, mat },
                Vert3D { pos: [pt1.0, pt1.1, pt1.2], norm: [n1.0, n1.1, n1.2], uv: [1.0, 1.0], col, mat },
                Vert3D { pos: [pt0.0, pt0.1, pt0.2], norm: [n0.0, n0.1, n0.2], uv: [0.0, 1.0], col, mat },
            );

            // Top cap
            let nt = xform.transform_normal(Vec3(0.0, 1.0, 0.0));
            let ct = xform.transform_point(Vec3(0.0, hh, 0.0));
            self.push_tri(
                Vert3D { pos: [ct.0, ct.1, ct.2], norm: [nt.0, nt.1, nt.2], uv: [0.5, 0.5], col, mat },
                Vert3D { pos: [pt0.0, pt0.1, pt0.2], norm: [nt.0, nt.1, nt.2], uv: [0.0, 0.0], col, mat },
                Vert3D { pos: [pt1.0, pt1.1, pt1.2], norm: [nt.0, nt.1, nt.2], uv: [1.0, 0.0], col, mat },
            );
        }
    }

    fn add_sphere_part(&mut self, xform: Mat4, r: f32, segs: usize, rings: usize, max_theta: f32, col: [f32; 4], mat: [f32; 2]) {
        for ir in 0..rings {
            let t0 = (ir as f32 / rings as f32) * max_theta;
            let t1 = ((ir + 1) as f32 / rings as f32) * max_theta;
            for is in 0..segs {
                let p0 = (is as f32 / segs as f32) * std::f32::consts::TAU;
                let p1 = ((is + 1) as f32 / segs as f32) * std::f32::consts::TAU;
                let mk = |th: f32, ph: f32| {
                    let (st, ct) = th.sin_cos();
                    let (sp, cp) = ph.sin_cos();
                    let dir = Vec3(st * cp, ct, st * sp);
                    let pos = xform.transform_point(dir.mul(r));
                    let n = xform.transform_normal(dir);
                    Vert3D { pos: [pos.0, pos.1, pos.2], norm: [n.0, n.1, n.2], uv: [0.5, 0.5], col, mat }
                };
                self.push_quad(mk(t0, p0), mk(t1, p0), mk(t1, p1), mk(t0, p1));
            }
        }
    }

    unsafe fn upload(self, gl: &GlFns) -> GlMesh {
        let mut vao = 0;
        let mut vbo = 0;
        (gl.GenVertexArrays)(1, &mut vao);
        (gl.GenBuffers)(1, &mut vbo);
        (gl.BindVertexArray)(vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, vbo);
        let bytes = (self.verts.len() * std::mem::size_of::<Vert3D>()) as GLsizeiptr;
        (gl.BufferData)(GL_ARRAY_BUFFER, bytes, self.verts.as_ptr() as *const c_void, GL_STATIC_DRAW);
        let stride = std::mem::size_of::<Vert3D>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 3, GL_FLOAT, GL_FALSE, stride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 3, GL_FLOAT, GL_FALSE, stride, 12 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 2, GL_FLOAT, GL_FALSE, stride, 24 as *const c_void);
        (gl.EnableVertexAttribArray)(3);
        (gl.VertexAttribPointer)(3, 4, GL_FLOAT, GL_FALSE, stride, 32 as *const c_void);
        (gl.EnableVertexAttribArray)(4);
        (gl.VertexAttribPointer)(4, 2, GL_FLOAT, GL_FALSE, stride, 48 as *const c_void);
        (gl.BindVertexArray)(0);
        GlMesh { vao, vbo, count: self.verts.len() as GLsizei }
    }
}

struct GlMesh {
    vao: GLuint,
    vbo: GLuint,
    count: GLsizei,
}

impl GlMesh {
    unsafe fn draw(&self, gl: &GlFns) {
        if self.count <= 0 { return; }
        (gl.BindVertexArray)(self.vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.count);
    }

    unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.vbo != 0 { (gl.DeleteBuffers)(1, &self.vbo); self.vbo = 0; }
        if self.vao != 0 { (gl.DeleteVertexArrays)(1, &self.vao); self.vao = 0; }
        self.count = 0;
    }
}

struct DepthFbo {
    fbo: GLuint,
    tex: GLuint,
    rbo: GLuint,
    w: i32,
    h: i32,
}

impl DepthFbo {
    fn empty() -> Self { Self { fbo: 0, tex: 0, rbo: 0, w: 0, h: 0 } }

    unsafe fn ensure(&mut self, gl: &GlFns, w: i32, h: i32, with_depth: bool) {
        let w = w.max(16);
        let h = h.max(16);
        if self.fbo != 0 && self.w == w && self.h == h { return; }
        self.destroy(gl);
        (gl.GenFramebuffers)(1, &mut self.fbo);
        (gl.GenTextures)(1, &mut self.tex);
        (gl.BindTexture)(GL_TEXTURE_2D, self.tex);
        (gl.TexImage2D)(
            GL_TEXTURE_2D, 0, GL_RGBA16F as GLint, w, h, 0,
            GL_RGBA, GL_FLOAT, std::ptr::null(),
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.fbo);
        (gl.FramebufferTexture2D)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, self.tex, 0);
        if with_depth {
            (gl.GenRenderbuffers)(1, &mut self.rbo);
            (gl.BindRenderbuffer)(GL_RENDERBUFFER, self.rbo);
            (gl.RenderbufferStorage)(GL_RENDERBUFFER, GL_DEPTH_COMPONENT24, w, h);
            (gl.FramebufferRenderbuffer)(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER, self.rbo);
        }
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        self.w = w;
        self.h = h;
    }

    unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.rbo != 0 { (gl.DeleteRenderbuffers)(1, &self.rbo); self.rbo = 0; }
        if self.tex != 0 { (gl.DeleteTextures)(1, &self.tex); self.tex = 0; }
        if self.fbo != 0 { (gl.DeleteFramebuffers)(1, &self.fbo); self.fbo = 0; }
        self.w = 0;
        self.h = 0;
    }
}

// =========================================================================
// Exact 5 Studio Glass Presets & Mood Interpolation
// =========================================================================
const fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    ]
}

#[derive(Clone, Copy)]
struct PresetSpec {
    pct: f32,
    sun_col: [f32; 3],
    sun_int: f32,
    sun_pos: [f32; 3],
    hemi_sky: [f32; 3],
    hemi_ground: [f32; 3],
    hemi_int: f32,
    amb_col: [f32; 3],
    amb_int: f32,
    lamp_int: f32,
    lamp_glow: f32,
    screen_int: f32,
    sky_top: [f32; 3],
    sky_mid: [f32; 3],
    sky_bot: [f32; 3],
    shaft_col: [f32; 3],
    shaft_alpha: f32,
    rain: f32,
    thunder: f32,
    dust: f32,
    stars: f32,
    rim_tint: [f32; 3],
    exposure: f32,
    wall_tint: [f32; 3],
    niche_tint: [f32; 3],
}

const PRESETS: [PresetSpec; 5] = [
    // 0: Morning
    PresetSpec {
        pct: 0.58, sun_col: rgb(0xFFE4BE), sun_int: 3.4, sun_pos: [-7.5, 5.2, -1.6],
        hemi_sky: rgb(0xAED4CC), hemi_ground: rgb(0x524234), hemi_int: 1.15,
        amb_col: rgb(0x5D7269), amb_int: 0.56, lamp_int: 1.55, lamp_glow: 0.92, screen_int: 1.05,
        sky_top: [76.0/255.0, 118.0/255.0, 118.0/255.0], sky_mid: [130.0/255.0, 166.0/255.0, 152.0/255.0], sky_bot: [224.0/255.0, 218.0/255.0, 190.0/255.0],
        shaft_col: [1.0, 0.90, 0.72], shaft_alpha: 0.22, rain: 0.0, thunder: 0.0, dust: 0.72, stars: 0.0,
        rim_tint: [1.0, 0.93, 0.82], exposure: 1.08, wall_tint: rgb(0x1D342E), niche_tint: rgb(0x4B421F),
    },
    // 1: Thunderstorm
    PresetSpec {
        pct: 0.36, sun_col: rgb(0xA8CDE4), sun_int: 1.85, sun_pos: [-7.0, 5.6, -1.2],
        hemi_sky: rgb(0x7A9EB6), hemi_ground: rgb(0x363D45), hemi_int: 0.96,
        amb_col: rgb(0x425866), amb_int: 0.50, lamp_int: 2.65, lamp_glow: 1.18, screen_int: 1.30,
        sky_top: [30.0/255.0, 46.0/255.0, 56.0/255.0], sky_mid: [50.0/255.0, 74.0/255.0, 86.0/255.0], sky_bot: [86.0/255.0, 112.0/255.0, 122.0/255.0],
        shaft_col: [0.68, 0.84, 0.96], shaft_alpha: 0.14, rain: 1.0, thunder: 1.0, dust: 0.10, stars: 0.0,
        rim_tint: [0.74, 0.89, 0.98], exposure: 1.02, wall_tint: rgb(0x17292A), niche_tint: rgb(0x3A321B),
    },
    // 2: Afternoon
    PresetSpec {
        pct: 1.00, sun_col: rgb(0xFFF8EC), sun_int: 4.2, sun_pos: [-7.0, 6.8, -0.6],
        hemi_sky: rgb(0xC8E2F2), hemi_ground: rgb(0x4A3E33), hemi_int: 1.32,
        amb_col: rgb(0x5C6A70), amb_int: 0.62, lamp_int: 0.32, lamp_glow: 0.28, screen_int: 0.75,
        sky_top: [106.0/255.0, 162.0/255.0, 214.0/255.0], sky_mid: [168.0/255.0, 206.0/255.0, 232.0/255.0], sky_bot: [236.0/255.0, 244.0/255.0, 248.0/255.0],
        shaft_col: [1.0, 0.98, 0.93], shaft_alpha: 0.26, rain: 0.0, thunder: 0.0, dust: 0.55, stars: 0.0,
        rim_tint: [0.95, 0.98, 1.0], exposure: 1.18, wall_tint: rgb(0x223B35), niche_tint: rgb(0x5C4F24),
    },
    // 3: Sunset
    PresetSpec {
        pct: 0.64, sun_col: rgb(0xFF9F4E), sun_int: 3.7, sun_pos: [-8.2, 3.3, -2.0],
        hemi_sky: rgb(0xC69484), hemi_ground: rgb(0x4D342B), hemi_int: 1.08,
        amb_col: rgb(0x644336), amb_int: 0.52, lamp_int: 2.25, lamp_glow: 1.08, screen_int: 1.15,
        sky_top: [82.0/255.0, 58.0/255.0, 96.0/255.0], sky_mid: [196.0/255.0, 108.0/255.0, 76.0/255.0], sky_bot: [254.0/255.0, 192.0/255.0, 118.0/255.0],
        shaft_col: [1.0, 0.68, 0.36], shaft_alpha: 0.30, rain: 0.0, thunder: 0.0, dust: 0.90, stars: 0.08,
        rim_tint: [1.0, 0.78, 0.54], exposure: 1.08, wall_tint: rgb(0x1E2B27), niche_tint: rgb(0x54381D),
    },
    // 4: Night
    PresetSpec {
        pct: 0.22, sun_col: rgb(0x86B8FF), sun_int: 1.65, sun_pos: [-7.6, 5.8, -1.8],
        hemi_sky: rgb(0x5272AA), hemi_ground: rgb(0x2C3040), hemi_int: 0.82,
        amb_col: rgb(0x334466), amb_int: 0.44, lamp_int: 3.4, lamp_glow: 1.38, screen_int: 1.45,
        sky_top: [14.0/255.0, 24.0/255.0, 48.0/255.0], sky_mid: [24.0/255.0, 44.0/255.0, 82.0/255.0], sky_bot: [52.0/255.0, 82.0/255.0, 126.0/255.0],
        shaft_col: [0.56, 0.76, 1.0], shaft_alpha: 0.18, rain: 0.0, thunder: 0.0, dust: 0.32, stars: 1.0,
        rim_tint: [0.68, 0.84, 1.0], exposure: 1.00, wall_tint: rgb(0x122122), niche_tint: rgb(0x352C18),
    },
];

const PULSE_PALETTES: [[[f32; 3]; 3]; 5] = [
    [[1.00, 0.80, 0.42], [1.00, 0.48, 0.76], [0.42, 0.88, 1.00]],
    [[0.34, 0.78, 1.00], [0.66, 0.52, 1.00], [0.90, 0.97, 1.00]],
    [[1.00, 0.95, 0.76], [0.46, 0.84, 1.00], [0.40, 0.92, 0.82]],
    [[1.00, 0.54, 0.24], [0.98, 0.42, 0.68], [1.00, 0.86, 0.36]],
    [[0.50, 0.56, 1.00], [0.76, 0.50, 1.00], [0.30, 0.82, 0.96]],
];

// HUD card slot order matches studio-glass.html:
// slot 0 = Morning (preset 0), slot 1 = Thunderstorm (preset 1), slot 2 = Sunset (preset 3), slot 3 = Afternoon (preset 2), slot 4 = Night (preset 4)
const CARD_PRESETS: [usize; 5] = [0, 1, 3, 2, 4];
const CARD_NAMES: [&str; 5] = ["Morning", "Thunderstorm", "Sunset", "Afternoon", "Night"];
const CARD_SUBS: [&str; 5] = ["Soft sun through the window", "Rain on glass", "Low amber light", "", ""];

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn lerpf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn auto_preset_index() -> usize {
    let h = Local::now().hour();
    if (6..11).contains(&h) { 0 }
    else if (11..17).contains(&h) { 2 }
    else if (17..20).contains(&h) { 3 }
    else { 4 }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragTarget {
    None,
    Blinds,
    Monitor,
    Lamp,
    Keyboard,
    Mouse,
    PenCup,
    Notebook,
    Chair,
    SideBooks,
    PlantL,
    PlantR,
}

fn ray_aabb(ro: Vec3, rd: Vec3, mn: Vec3, mx: Vec3) -> Option<f32> {
    let mut tmin = 0.0f32;
    let mut tmax = 1e6f32;
    for (o, d, lo, hi) in [
        (ro.0, rd.0, mn.0, mx.0),
        (ro.1, rd.1, mn.1, mx.1),
        (ro.2, rd.2, mn.2, mx.2),
    ] {
        if d.abs() < 1e-6 {
            if o < lo || o > hi { return None; }
        } else {
            let inv = 1.0 / d;
            let mut t1 = (lo - o) * inv;
            let mut t2 = (hi - o) * inv;
            if t1 > t2 { std::mem::swap(&mut t1, &mut t2); }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax { return None; }
        }
    }
    if tmax >= 0.0 { Some(tmin) } else { None }
}

fn ray_plane_y(ro: Vec3, rd: Vec3, plane_y: f32) -> Option<Vec3> {
    if rd.1.abs() < 1e-5 { return None; }
    let t = (plane_y - ro.1) / rd.1;
    if t <= 0.0 || t > 40.0 { return None; }
    Some(Vec3(ro.0 + rd.0 * t, plane_y, ro.2 + rd.2 * t))
}

// =========================================================================
// Studio Glass Interactive State (embedded cleanly inside AppState)
// =========================================================================
pub struct StudioGlassState {
    pub param_preset: i32, // 0=Auto, 1..5=presets
    pub param_blinds: f32,
    pub param_shafts: f32,
    pub param_weather: f32,
    pub param_monitor_angle: f32,
    pub param_desk_lamp: f32,
    pub param_parallax: f32,
    pub param_refraction: f32,
    pub param_frost: f32,
    pub param_icon_glass: i32,
    pub param_show_hud: bool,

    pub interactive_preset: Option<usize>,
    pub preset_brightness: [f32; 5],
    pub interactive_brightness: f32,
    pub interactive_blinds: Option<f32>,
    pub interactive_monitor: Option<f32>,
    pub lamp_manual: Option<bool>,

    // Live draggable 3D Studio object positions (1:1 studio-glass-engine.js defaults!)
    pub kb_pos: (f32, f32),
    pub mouse_pos: (f32, f32),
    pub pen_cup_pos: (f32, f32),
    pub notebook_pos: (f32, f32),
    pub chair_x: f32,
    pub chair_rot: f32,
    pub side_books_x: f32,
    pub plant_l_x: f32,
    pub plant_r_x: f32,

    pub cur_blinds: f32,
    pub cur_monitor: f32,
    pub ptr_smooth_x: f32,
    pub ptr_smooth_y: f32,
    pub plant_sway: [(f32, f32); 2],
    pub flash: f32,
    pub next_flash: f32,
    pub time: f32,

    pub pulse_card_idx: i32,
    pub pulse_age: f32,
    pub pulse_preset: usize,

    mood: PresetSpec,

    // Movable HUD state & hit rectangles
    pub hud_collapsed: bool,
    pub hud_pos: Option<(f32, f32)>,
    pub hud_bounds: Rect,
    pub hit_header: Rect,
    pub hit_collapse: Rect,
    pub hit_cards: [Rect; 5],
    pub hit_slider: Rect,

    dragging_hud: bool,
    drag_hud_off: (f32, f32),
    sliding_bright: bool,
    drag_target: DragTarget,
    drag_start_pt: (f32, f32),
    drag_start_val: f32,
    drag_off_xz: (f32, f32),
    drag_moved: bool,
}

impl Default for StudioGlassState {
    fn default() -> Self {
        Self {
            param_preset: 1,
            param_blinds: 0.70,
            param_shafts: 0.80,
            param_weather: 0.78,
            param_monitor_angle: -10.0,
            param_desk_lamp: 0.70,
            param_parallax: 0.55,
            param_refraction: 0.85,
            param_frost: 0.50,
            param_icon_glass: 0,
            param_show_hud: true,

            interactive_preset: None,
            preset_brightness: [PRESETS[0].pct, PRESETS[1].pct, PRESETS[2].pct, PRESETS[3].pct, PRESETS[4].pct],
            interactive_brightness: PRESETS[0].pct,
            interactive_blinds: None,
            interactive_monitor: None,
            lamp_manual: None,

            kb_pos: (0.32, 0.40),
            mouse_pos: (1.06, 0.48),
            pen_cup_pos: (-0.36, 0.10),
            notebook_pos: (-0.84, 0.28),
            chair_x: -0.56,
            chair_rot: 0.42,
            side_books_x: 2.42,
            plant_l_x: -0.66,
            plant_r_x: 0.68,

            cur_blinds: 0.70,
            cur_monitor: -10.0,
            ptr_smooth_x: 0.5,
            ptr_smooth_y: 0.5,
            plant_sway: [(0.0, 0.0); 2],
            flash: 0.0,
            next_flash: 2.8,
            time: 0.0,

            pulse_card_idx: -1,
            pulse_age: 99.0,
            pulse_preset: 0,

            mood: PRESETS[0],

            hud_collapsed: false,
            hud_pos: None,
            hud_bounds: Rect::default(),
            hit_header: Rect::default(),
            hit_collapse: Rect::default(),
            hit_cards: [Rect::default(); 5],
            hit_slider: Rect::default(),

            dragging_hud: false,
            drag_hud_off: (0.0, 0.0),
            sliding_bright: false,
            drag_target: DragTarget::None,
            drag_start_pt: (0.0, 0.0),
            drag_start_val: 0.0,
            drag_off_xz: (0.0, 0.0),
            drag_moved: false,
        }
    }
}

impl StudioGlassState {
    pub fn reset_interaction(&mut self) {
        self.dragging_hud = false;
        self.sliding_bright = false;
        self.drag_target = DragTarget::None;
        self.drag_moved = false;
    }

    fn camera_pos(&self) -> Vec3 {
        let par = self.param_parallax;
        Vec3(
            0.45 + (self.ptr_smooth_x - 0.5) * 0.42 * par,
            2.02 - (self.ptr_smooth_y - 0.5) * 0.22 * par,
            5.75,
        )
    }

    fn screen_ray(&self, x: f32, y: f32, css_w: f32, css_h: f32) -> (Vec3, Vec3) {
        let eye = self.camera_pos();
        let target = Vec3(-0.08, 1.48, -0.55);
        let fwd = target.sub(eye).norm();
        let right = fwd.cross(Vec3(0.0, 1.0, 0.0)).norm();
        let up = right.cross(fwd);

        let aspect = css_w / css_h.max(1.0);
        let tan_half = (36.0f32.to_radians() * 0.5).tan();
        let ndc_x = (x / css_w.max(1.0)) * 2.0 - 1.0;
        let ndc_y = 1.0 - (y / css_h.max(1.0)) * 2.0;

        let dir = Vec3(
            fwd.0 + right.0 * (ndc_x * aspect * tan_half) + up.0 * (ndc_y * tan_half),
            fwd.1 + right.1 * (ndc_x * aspect * tan_half) + up.1 * (ndc_y * tan_half),
            fwd.2 + right.2 * (ndc_x * aspect * tan_half) + up.2 * (ndc_y * tan_half),
        )
        .norm();
        (eye, dir)
    }

    fn pick_3d(&self, x: f32, y: f32, css_w: f32, css_h: f32) -> DragTarget {
        let (ro, rd) = self.screen_ray(x, y, css_w, css_h);
        let mut best_t = 1e6f32;
        let mut best = DragTarget::None;

        let mut test_world = |target: DragTarget, mn: Vec3, mx: Vec3| {
            if let Some(t) = ray_aabb(ro, rd, mn, mx) {
                if t < best_t {
                    best_t = t;
                    best = target;
                }
            }
        };

        let dy = 0.92;
        // 1. Ergonomic Task Chair (foreground-left)
        test_world(
            DragTarget::Chair,
            Vec3(self.chair_x - 0.46, 0.05, 1.38 - 0.46),
            Vec3(self.chair_x + 0.46, 1.42, 1.38 + 0.46),
        );
        // 2. Mechanical Keyboard
        test_world(
            DragTarget::Keyboard,
            Vec3(self.kb_pos.0 - 0.46, dy - 0.02, self.kb_pos.1 - 0.19),
            Vec3(self.kb_pos.0 + 0.46, dy + 0.10, self.kb_pos.1 + 0.19),
        );
        // 3. Wireless Studio Mouse
        test_world(
            DragTarget::Mouse,
            Vec3(self.mouse_pos.0 - 0.12, dy - 0.02, self.mouse_pos.1 - 0.16),
            Vec3(self.mouse_pos.0 + 0.12, dy + 0.10, self.mouse_pos.1 + 0.16),
        );
        // 4. Ceramic Pen Cup + Pencils
        test_world(
            DragTarget::PenCup,
            Vec3(self.pen_cup_pos.0 - 0.11, dy, self.pen_cup_pos.1 - 0.11),
            Vec3(self.pen_cup_pos.0 + 0.11, dy + 0.30, self.pen_cup_pos.1 + 0.11),
        );
        // 5. Black Leather Notebook
        test_world(
            DragTarget::Notebook,
            Vec3(self.notebook_pos.0 - 0.21, dy - 0.02, self.notebook_pos.1 - 0.25),
            Vec3(self.notebook_pos.0 + 0.21, dy + 0.07, self.notebook_pos.1 + 0.25),
        );
        // 6. Swiveling Studio Display Monitor
        test_world(
            DragTarget::Monitor,
            Vec3(0.62 - 0.92, dy + 0.28, -0.39 - 0.22),
            Vec3(0.62 + 0.92, dy + 1.38, -0.39 + 0.22),
        );
        // 7. Frosted Glass Mushroom Lamp (right side table)
        let sy = 1.04;
        test_world(
            DragTarget::Lamp,
            Vec3(2.92 - 0.32, sy, -1.16 - 0.32),
            Vec3(2.92 + 0.32, sy + 0.60, -1.16 + 0.32),
        );
        // 8. Stacked Art Books + Ceramic Vase on Side Table
        test_world(
            DragTarget::SideBooks,
            Vec3(self.side_books_x - 0.26, sy, -1.16 - 0.22),
            Vec3(self.side_books_x + 0.26, sy + 0.54, -1.16 + 0.22),
        );

        // 9. Angled Left Window Local Space (Plants & 26-Slat Venetian Blinds!)
        let inv_win = Mat4::rot_y(-0.72).mul(Mat4::translate(3.12, 0.0, 0.32));
        let lro = inv_win.transform_point(ro);
        let lrd = inv_win.transform_normal(rd);
        let mut test_win = |target: DragTarget, mn: Vec3, mx: Vec3| {
            if let Some(t) = ray_aabb(lro, lrd, mn, mx) {
                if t < best_t {
                    best_t = t;
                    best = target;
                }
            }
        };
        test_win(
            DragTarget::PlantL,
            Vec3(self.plant_l_x - 0.24, 1.06, -0.06),
            Vec3(self.plant_l_x + 0.24, 1.72, 0.46),
        );
        test_win(
            DragTarget::PlantR,
            Vec3(self.plant_r_x - 0.20, 1.06, -0.04),
            Vec3(self.plant_r_x + 0.20, 1.58, 0.44),
        );
        test_win(
            DragTarget::Blinds,
            Vec3(-1.38, 1.10, -0.05),
            Vec3(1.38, 3.60, 0.30),
        );

        best
    }

    pub fn active_preset_idx(&self) -> usize {
        if let Some(ip) = self.interactive_preset {
            return ip.min(4);
        }
        if self.param_preset <= 0 {
            auto_preset_index()
        } else {
            ((self.param_preset - 1) as usize).min(4)
        }
    }

    pub fn sync_from_raw_params(&mut self, st_preset: i32, blinds: f32, shafts: f32, weather: f32, mon_ang: f32, lamp: f32, par: f32, refr: f32, frost: f32, ic_glass: i32, show_hud: bool) {
        if (blinds - self.param_blinds).abs() > 0.001 {
            self.interactive_blinds = None;
        }
        if (mon_ang - self.param_monitor_angle).abs() > 0.05 {
            self.interactive_monitor = None;
        }
        if st_preset != self.param_preset {
            self.param_preset = st_preset;
            self.interactive_preset = None;
            self.lamp_manual = None;
            let idx = self.active_preset_idx();
            self.interactive_brightness = self.preset_brightness[idx];
            if let Some(slot) = CARD_PRESETS.iter().position(|&p| p == idx) {
                self.pulse_card_idx = slot as i32;
                self.pulse_age = 0.0;
                self.pulse_preset = idx;
            }
        }
        self.param_blinds = blinds.clamp(0.0, 1.0);
        self.param_shafts = shafts.clamp(0.0, 1.2);
        self.param_weather = weather.clamp(0.0, 1.0);
        self.param_monitor_angle = mon_ang.clamp(-35.0, 35.0);
        self.param_desk_lamp = lamp.clamp(0.0, 1.2);
        self.param_parallax = par.clamp(0.0, 1.0);
        self.param_refraction = refr.clamp(0.0, 1.4);
        self.param_frost = frost.clamp(0.0, 1.0);
        self.param_icon_glass = ic_glass.clamp(0, 2);
        self.param_show_hud = show_hud;
    }

    pub fn recompute_hud(&mut self, css_w: f32, css_h: f32) {
        let hud_w = (css_w * 0.84).min(760.0).max(300.0).round();
        let header_h = 44.0;
        let grid_h = (css_h * 0.215).clamp(174.0, 228.0).round();
        let total_h = if self.hud_collapsed { header_h } else { header_h + grid_h };

        let (hx, hy) = if let Some((rx, ry)) = self.hud_pos {
            (
                (rx * css_w).clamp(8.0, (css_w - hud_w - 8.0).max(8.0)).round(),
                (ry * css_h).clamp(8.0, (css_h - total_h - 8.0).max(8.0)).round(),
            )
        } else {
            (((css_w - hud_w) * 0.5).round(), (css_h * 0.045).round())
        };

        self.hud_bounds = Rect { x: hx, y: hy, w: hud_w, h: total_h };
        self.hit_collapse = Rect { x: hx + hud_w - 38.0, y: hy + 2.0, w: 34.0, h: 34.0 };
        self.hit_header = Rect { x: hx, y: hy, w: hud_w - 44.0, h: 38.0 };

        if !self.param_show_hud || self.hud_collapsed {
            self.hit_cards = [Rect::default(); 5];
            self.hit_slider = Rect::default();
            return;
        }

        // Exact 4-column grid: 1.55fr 1.5fr 0.78fr 0.78fr (total = 4.61)
        let gap = (css_w * 0.0075).clamp(7.0, 11.0).round();
        let avail_w = hud_w - gap * 3.0;
        let fr = avail_w / 4.61;
        let w0 = (1.55 * fr).round();
        let w1 = (1.50 * fr).round();
        let w2 = (0.78 * fr).round();
        let w3 = (avail_w - w0 - w1 - w2).max(48.0);
        let row_h = ((grid_h - gap) * 0.5).round();

        let gx0 = hx;
        let gx1 = gx0 + w0 + gap;
        let gx2 = gx1 + w1 + gap;
        let gx3 = gx2 + w2 + gap;
        let gy0 = hy + header_h;
        let gy1 = gy0 + row_h + gap;

        // Slot 0: Morning (hero, spans 2 rows)
        self.hit_cards[0] = Rect { x: gx0, y: gy0, w: w0, h: grid_h };
        // Slot 1: Thunderstorm (col 2, row 1)
        self.hit_cards[1] = Rect { x: gx1, y: gy0, w: w1, h: row_h };
        // Slot 2: Sunset (col 2, row 2)
        self.hit_cards[2] = Rect { x: gx1, y: gy1, w: w1, h: grid_h - row_h - gap };
        // Slot 3: Afternoon (col 3, row 1)
        self.hit_cards[3] = Rect { x: gx2, y: gy0, w: w2, h: row_h };
        // Slot 4: Night (col 3, row 2)
        self.hit_cards[4] = Rect { x: gx2, y: gy1, w: w2, h: grid_h - row_h - gap };
        // Brightness vertical pill slider (col 4, spans 2 rows)
        self.hit_slider = Rect { x: gx3, y: gy0, w: w3, h: grid_h };
    }

    pub fn ui_rects(&self) -> Vec<[i32; 4]> {
        if !self.param_show_hud {
            return Vec::new();
        }
        let r = self.hud_bounds;
        if r.w > 4.0 && r.h > 4.0 {
            vec![[r.x.floor() as i32, r.y.floor() as i32, r.w.ceil() as i32, r.h.ceil() as i32]]
        } else {
            Vec::new()
        }
    }

    pub fn step(&mut self, dt: f32, ptr_x: f32, ptr_y: f32, ptr_active: bool, css_w: f32, css_h: f32) {
        self.time += dt;
        let target = PRESETS[self.active_preset_idx()];
        let k = 1.0 - (-dt * 3.4).exp();
        let m = &mut self.mood;
        m.sun_col = lerp3(m.sun_col, target.sun_col, k);
        m.sun_int = lerpf(m.sun_int, target.sun_int, k);
        m.sun_pos = lerp3(m.sun_pos, target.sun_pos, k);
        m.hemi_sky = lerp3(m.hemi_sky, target.hemi_sky, k);
        m.hemi_ground = lerp3(m.hemi_ground, target.hemi_ground, k);
        m.hemi_int = lerpf(m.hemi_int, target.hemi_int, k);
        m.amb_col = lerp3(m.amb_col, target.amb_col, k);
        m.amb_int = lerpf(m.amb_int, target.amb_int, k);
        m.lamp_int = lerpf(m.lamp_int, target.lamp_int, k);
        m.lamp_glow = lerpf(m.lamp_glow, target.lamp_glow, k);
        m.screen_int = lerpf(m.screen_int, target.screen_int, k);
        m.sky_top = lerp3(m.sky_top, target.sky_top, k);
        m.sky_mid = lerp3(m.sky_mid, target.sky_mid, k);
        m.sky_bot = lerp3(m.sky_bot, target.sky_bot, k);
        m.shaft_col = lerp3(m.shaft_col, target.shaft_col, k);
        m.shaft_alpha = lerpf(m.shaft_alpha, target.shaft_alpha, k);
        m.rain = lerpf(m.rain, target.rain, k);
        m.thunder = lerpf(m.thunder, target.thunder, k);
        m.dust = lerpf(m.dust, target.dust, k);
        m.stars = lerpf(m.stars, target.stars, k);
        m.rim_tint = lerp3(m.rim_tint, target.rim_tint, k);
        m.exposure = lerpf(m.exposure, target.exposure, k);
        m.wall_tint = lerp3(m.wall_tint, target.wall_tint, k);
        m.niche_tint = lerp3(m.niche_tint, target.niche_tint, k);

        if self.pulse_age < 10.0 {
            self.pulse_age += dt;
        }

        if m.thunder > 0.35 && self.param_weather > 0.1 {
            self.next_flash -= dt;
            if self.next_flash <= 0.0 {
                let pseudo = (self.time * 13.7).sin().abs() * 0.35;
                self.flash = 0.85 + pseudo;
                self.next_flash = 4.2 + ((self.time * 7.3).cos().abs() * 6.0);
            }
        }
        self.flash = (self.flash - dt * 3.8).max(0.0);

        let sk = 1.0 - (-dt * 8.0).exp();
        let goal_blinds = self.interactive_blinds.unwrap_or(self.param_blinds);
        let goal_mon = self.interactive_monitor.unwrap_or(self.param_monitor_angle);
        self.cur_blinds = lerpf(self.cur_blinds, goal_blinds, sk);
        self.cur_monitor = lerpf(self.cur_monitor, goal_mon, sk);

        // Spring leaf sway on windowsill plants
        for i in 0..2 {
            let (mut ang, mut vel) = self.plant_sway[i];
            let breeze = (self.time * (1.2 + i as f32 * 0.35) + i as f32 * 2.0).sin() * 0.022 * (0.4 + 0.6 * self.cur_blinds);
            let force = -(ang - breeze) * 24.0 - vel * 4.5;
            vel += force * dt;
            ang += vel * dt;
            self.plant_sway[i] = (ang.clamp(-0.35, 0.35), vel);
        }

        // Smooth camera parallax
        let tx = if ptr_active { (ptr_x / css_w.max(1.0)).clamp(0.0, 1.0) } else { 0.5 };
        let ty = if ptr_active { (ptr_y / css_h.max(1.0)).clamp(0.0, 1.0) } else { 0.5 };
        let pk = 1.0 - (-dt * 4.8).exp();
        self.ptr_smooth_x = lerpf(self.ptr_smooth_x, tx, pk);
        self.ptr_smooth_y = lerpf(self.ptr_smooth_y, ty, pk);
    }

    // kind: 0=move, 1=down, 2=up, 3=leave. Returns (handled, ui_rects_changed)
    pub fn handle_mouse(&mut self, kind: i32, x: f32, y: f32, css_w: f32, css_h: f32) -> (bool, bool) {
        if kind == 3 {
            self.reset_interaction();
            return (false, false);
        }

        if kind == 0 {
            if self.sliding_bright {
                self.update_bright_from_y(y);
                return (true, false);
            }
            if self.dragging_hud {
                let nx = (x - self.drag_hud_off.0).clamp(8.0, (css_w - self.hud_bounds.w - 8.0).max(8.0));
                let ny = (y - self.drag_hud_off.1).clamp(8.0, (css_h - self.hud_bounds.h - 8.0).max(8.0));
                self.hud_pos = Some((nx / css_w.max(1.0), ny / css_h.max(1.0)));
                self.recompute_hud(css_w, css_h);
                return (true, true);
            }
            if self.drag_target != DragTarget::None {
                let dx = x - self.drag_start_pt.0;
                let dy = y - self.drag_start_pt.1;
                if dx.hypot(dy) > 4.0 { self.drag_moved = true; }
                let (ro, rd) = self.screen_ray(x, y, css_w, css_h);
                match self.drag_target {
                    DragTarget::Blinds => {
                        self.interactive_blinds = Some((self.drag_start_val - dy / (css_h * 0.45).max(180.0)).clamp(0.0, 1.0));
                    }
                    DragTarget::Monitor => {
                        self.interactive_monitor = Some((self.drag_start_val + (dx / (css_w * 0.25).max(160.0)) * 45.0).clamp(-35.0, 35.0));
                    }
                    DragTarget::Keyboard => {
                        if let Some(hit) = ray_plane_y(ro, rd, 0.92) {
                            self.kb_pos.0 = (hit.0 + self.drag_off_xz.0).clamp(-1.25, 2.1);
                            self.kb_pos.1 = (hit.2 + self.drag_off_xz.1).clamp(-0.05, 0.82);
                        }
                    }
                    DragTarget::Mouse => {
                        if let Some(hit) = ray_plane_y(ro, rd, 0.92) {
                            self.mouse_pos.0 = (hit.0 + self.drag_off_xz.0).clamp(-1.1, 2.35);
                            self.mouse_pos.1 = (hit.2 + self.drag_off_xz.1).clamp(-0.05, 0.86);
                        }
                    }
                    DragTarget::PenCup => {
                        if let Some(hit) = ray_plane_y(ro, rd, 0.92) {
                            self.pen_cup_pos.0 = (hit.0 + self.drag_off_xz.0).clamp(-1.5, 2.3);
                            self.pen_cup_pos.1 = (hit.2 + self.drag_off_xz.1).clamp(-0.52, 0.80);
                        }
                    }
                    DragTarget::Notebook => {
                        if let Some(hit) = ray_plane_y(ro, rd, 0.92) {
                            self.notebook_pos.0 = (hit.0 + self.drag_off_xz.0).clamp(-1.55, 2.2);
                            self.notebook_pos.1 = (hit.2 + self.drag_off_xz.1).clamp(-0.45, 0.76);
                        }
                    }
                    DragTarget::Chair => {
                        if let Some(hit) = ray_plane_y(ro, rd, 0.75) {
                            let next_x = (hit.0 + self.drag_off_xz.0).clamp(-1.55, 1.55);
                            let vel_x = next_x - self.chair_x;
                            self.chair_x = next_x;
                            self.chair_rot = (self.chair_rot + vel_x * 0.35).clamp(-0.55, 0.95);
                        }
                    }
                    DragTarget::SideBooks => {
                        self.side_books_x = (self.drag_start_val + dx * 0.004).clamp(2.18, 2.62);
                    }
                    DragTarget::PlantL => {
                        let next_x = (self.drag_start_val + dx * 0.005).clamp(-1.12, -0.05);
                        let delta = next_x - self.plant_l_x;
                        self.plant_l_x = next_x;
                        self.plant_sway[0].1 -= delta * 18.0;
                    }
                    DragTarget::PlantR => {
                        let next_x = (self.drag_start_val + dx * 0.005).clamp(0.15, 1.12);
                        let delta = next_x - self.plant_r_x;
                        self.plant_r_x = next_x;
                        self.plant_sway[1].1 -= delta * 18.0;
                    }
                    DragTarget::Lamp | DragTarget::None => {}
                }
                return (true, false);
            }

            // Gentle leaf rustle when hovering over windowsill plants!
            if !(self.param_show_hud && self.hud_bounds.contains(x, y)) {
                match self.pick_3d(x, y, css_w, css_h) {
                    DragTarget::PlantL => { self.plant_sway[0].1 += 0.25; }
                    DragTarget::PlantR => { self.plant_sway[1].1 += 0.25; }
                    _ => {}
                }
            }
            return (false, false);
        }

        if kind == 2 {
            let was = self.dragging_hud || self.sliding_bright || self.drag_target != DragTarget::None;
            if self.drag_target == DragTarget::Lamp && !self.drag_moved {
                let auto_on = self.mood.lamp_int > 0.6;
                self.lamp_manual = Some(match self.lamp_manual {
                    None => !auto_on,
                    Some(v) => !v,
                });
            }
            let changed = self.dragging_hud;
            self.reset_interaction();
            return (was, changed);
        }

        // kind == 1 (mouse down)
        self.drag_start_pt = (x, y);
        self.drag_moved = false;

        if self.param_show_hud {
            if self.hit_collapse.contains(x, y) {
                self.hud_collapsed = !self.hud_collapsed;
                self.recompute_hud(css_w, css_h);
                return (true, true);
            }
            if self.hit_header.contains(x, y) {
                self.dragging_hud = true;
                self.drag_hud_off = (x - self.hud_bounds.x, y - self.hud_bounds.y);
                return (true, false);
            }
            if !self.hud_collapsed {
                if self.hit_slider.contains(x, y) {
                    self.sliding_bright = true;
                    self.update_bright_from_y(y);
                    return (true, false);
                }
                for slot in 0..5 {
                    if self.hit_cards[slot].contains(x, y) {
                        let preset_idx = CARD_PRESETS[slot];
                        self.interactive_preset = Some(preset_idx);
                        self.interactive_brightness = self.preset_brightness[preset_idx];
                        self.lamp_manual = None;
                        self.pulse_card_idx = slot as i32;
                        self.pulse_age = 0.0;
                        self.pulse_preset = preset_idx;
                        if preset_idx == 1 {
                            self.flash = 0.92;
                            self.next_flash = 4.2;
                        }
                        return (true, false);
                    }
                }
            }
        }

        // Real 3D Raycast Pick across ALL 10 Studio Glass objects!
        let picked = self.pick_3d(x, y, css_w, css_h);
        if picked == DragTarget::None {
            return (false, false);
        }
        self.drag_target = picked;
        let (ro, rd) = self.screen_ray(x, y, css_w, css_h);
        match picked {
            DragTarget::Blinds => {
                self.drag_start_val = self.interactive_blinds.unwrap_or(self.param_blinds);
            }
            DragTarget::Monitor => {
                self.drag_start_val = self.interactive_monitor.unwrap_or(self.param_monitor_angle);
            }
            DragTarget::Keyboard => {
                if let Some(hit) = ray_plane_y(ro, rd, 0.92) {
                    self.drag_off_xz = (self.kb_pos.0 - hit.0, self.kb_pos.1 - hit.2);
                }
            }
            DragTarget::Mouse => {
                if let Some(hit) = ray_plane_y(ro, rd, 0.92) {
                    self.drag_off_xz = (self.mouse_pos.0 - hit.0, self.mouse_pos.1 - hit.2);
                }
            }
            DragTarget::PenCup => {
                if let Some(hit) = ray_plane_y(ro, rd, 0.92) {
                    self.drag_off_xz = (self.pen_cup_pos.0 - hit.0, self.pen_cup_pos.1 - hit.2);
                }
            }
            DragTarget::Notebook => {
                if let Some(hit) = ray_plane_y(ro, rd, 0.92) {
                    self.drag_off_xz = (self.notebook_pos.0 - hit.0, self.notebook_pos.1 - hit.2);
                }
            }
            DragTarget::Chair => {
                if let Some(hit) = ray_plane_y(ro, rd, 0.75) {
                    self.drag_off_xz = (self.chair_x - hit.0, 0.0);
                }
            }
            DragTarget::SideBooks => {
                self.drag_start_val = self.side_books_x;
            }
            DragTarget::PlantL => {
                self.drag_start_val = self.plant_l_x;
                self.plant_sway[0].1 += 2.6;
            }
            DragTarget::PlantR => {
                self.drag_start_val = self.plant_r_x;
                self.plant_sway[1].1 += 2.6;
            }
            DragTarget::Lamp | DragTarget::None => {}
        }
        (true, false)
    }

    fn update_bright_from_y(&mut self, y: f32) {
        let r = self.hit_slider;
        let frac = (1.0 - (y - r.y) / r.h.max(1.0)).clamp(0.08, 1.0);
        self.interactive_brightness = frac;
        let idx = self.active_preset_idx();
        self.preset_brightness[idx] = frac;
    }
}

// =========================================================================
// Studio Glass 3D Multi-Pass Renderer
// =========================================================================
pub struct StudioGlassRenderer {
    prog_pbr: GLuint,
    prog_window: GLuint,
    prog_monitor: GLuint,
    prog_shaft: GLuint,
    prog_dust: GLuint,
    prog_blur: GLuint,
    prog_glass: GLuint,
    prog_icon: GLuint,
    dummy_vao: GLuint,

    scene_fbo: DepthFbo,
    blur_a: DepthFbo,
    blur_b: DepthFbo,

    // Static & articulated 3D studio meshes
    mesh_static: GlMesh,
    mesh_keyboard: GlMesh,
    mesh_mouse: GlMesh,
    mesh_pen_cup: GlMesh,
    mesh_notebook: GlMesh,
    mesh_chair: GlMesh,
    mesh_side_books: GlMesh,
    mesh_plant_l_pot: GlMesh,
    mesh_plant_r_pot: GlMesh,
    mesh_lamp_glass: GlMesh,
    mesh_lamp_cap: GlMesh,
    mesh_window_pane: GlMesh,
    mesh_slat: GlMesh,
    mesh_blind_rail: GlMesh,
    mesh_plant_l_crown: GlMesh,
    mesh_plant_r_crown: GlMesh,
    mesh_monitor_head: GlMesh,
    mesh_monitor_screen: GlMesh,
    mesh_shafts: GlMesh,

    dust_vao: GLuint,
    dust_vbo: GLuint,
    dust_base: Vec<[f32; 5]>, // x, y, z, phase, speed
}

impl StudioGlassRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog_pbr = gl.compile_program(PBR_VERT, PBR_FRAG)?;
        let prog_window = gl.compile_program(WINDOW_VERT, WINDOW_FRAG)?;
        let prog_monitor = gl.compile_program(WINDOW_VERT, MONITOR_FRAG)?;
        let prog_shaft = gl.compile_program(WINDOW_VERT, SHAFT_FRAG)?;
        let prog_dust = gl.compile_program(DUST_VERT, DUST_FRAG)?;
        let prog_blur = gl.compile_program(FULLSCREEN_VERT, POST_BLUR_FRAG)?;
        let prog_glass = gl.compile_program(FULLSCREEN_VERT, POST_GLASS_FRAG)?;
        let prog_icon = gl.compile_program(HUD_ICON_VERT, HUD_ICON_FRAG)?;

        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let meshes = build_studio_meshes(gl);

        // 90 floating 3D sunlit dust motes
        let mut dust_base = Vec::with_capacity(DUST_COUNT);
        let mut seed: u32 = 7123;
        let mut next_r = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed as f32) / 4294967296.0
        };
        for _ in 0..DUST_COUNT {
            dust_base.push([
                -2.4 + next_r() * 5.2,
                1.05 + next_r() * 2.6,
                -1.3 + next_r() * 2.2,
                next_r() * std::f32::consts::TAU,
                0.35 + next_r() * 0.65,
            ]);
        }
        let mut dust_vao = 0;
        let mut dust_vbo = 0;
        (gl.GenVertexArrays)(1, &mut dust_vao);
        (gl.GenBuffers)(1, &mut dust_vbo);
        (gl.BindVertexArray)(dust_vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, dust_vbo);
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 3, GL_FLOAT, GL_FALSE, 12, 0 as *const c_void);
        (gl.BindVertexArray)(0);

        Ok(Self {
            prog_pbr,
            prog_window,
            prog_monitor,
            prog_shaft,
            prog_dust,
            prog_blur,
            prog_glass,
            prog_icon,
            dummy_vao,
            scene_fbo: DepthFbo::empty(),
            blur_a: DepthFbo::empty(),
            blur_b: DepthFbo::empty(),
            mesh_static: meshes.static_room,
            mesh_keyboard: meshes.keyboard,
            mesh_mouse: meshes.mouse,
            mesh_pen_cup: meshes.pen_cup,
            mesh_notebook: meshes.notebook,
            mesh_chair: meshes.chair,
            mesh_side_books: meshes.side_books,
            mesh_plant_l_pot: meshes.plant_l_pot,
            mesh_plant_r_pot: meshes.plant_r_pot,
            mesh_lamp_glass: meshes.lamp_glass,
            mesh_lamp_cap: meshes.lamp_cap,
            mesh_window_pane: meshes.window_pane,
            mesh_slat: meshes.slat,
            mesh_blind_rail: meshes.blind_rail,
            mesh_plant_l_crown: meshes.plant_l_crown,
            mesh_plant_r_crown: meshes.plant_r_crown,
            mesh_monitor_head: meshes.monitor_head,
            mesh_monitor_screen: meshes.monitor_screen,
            mesh_shafts: meshes.shafts,
            dust_vao,
            dust_vbo,
            dust_base,
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        self.scene_fbo.destroy(gl);
        self.blur_a.destroy(gl);
        self.blur_b.destroy(gl);
        self.mesh_static.destroy(gl);
        self.mesh_keyboard.destroy(gl);
        self.mesh_mouse.destroy(gl);
        self.mesh_pen_cup.destroy(gl);
        self.mesh_notebook.destroy(gl);
        self.mesh_chair.destroy(gl);
        self.mesh_side_books.destroy(gl);
        self.mesh_plant_l_pot.destroy(gl);
        self.mesh_plant_r_pot.destroy(gl);
        self.mesh_lamp_glass.destroy(gl);
        self.mesh_lamp_cap.destroy(gl);
        self.mesh_window_pane.destroy(gl);
        self.mesh_slat.destroy(gl);
        self.mesh_blind_rail.destroy(gl);
        self.mesh_plant_l_crown.destroy(gl);
        self.mesh_plant_r_crown.destroy(gl);
        self.mesh_monitor_head.destroy(gl);
        self.mesh_monitor_screen.destroy(gl);
        self.mesh_shafts.destroy(gl);
        if self.dust_vbo != 0 { (gl.DeleteBuffers)(1, &self.dust_vbo); }
        if self.dust_vao != 0 { (gl.DeleteVertexArrays)(1, &self.dust_vao); }
        if self.dummy_vao != 0 { (gl.DeleteVertexArrays)(1, &self.dummy_vao); }
        for p in [
            self.prog_pbr, self.prog_window, self.prog_monitor, self.prog_shaft,
            self.prog_dust, self.prog_blur, self.prog_glass, self.prog_icon,
        ] {
            if p != 0 { (gl.DeleteProgram)(p); }
        }
    }

    unsafe fn draw_icon(&self, gl: &GlFns, css_w: f32, css_h: f32, icon: i32, r: Rect, params: [f32; 4]) {
        (gl.UseProgram)(self.prog_icon);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_icon, "u_res"), css_w, css_h);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_icon, "u_rect"), r.x, r.y, r.w, r.h);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_icon, "u_icon"), icon);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_icon, "u_params"), params[0], params[1], params[2], params[3]);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState, text: &mut TextRenderer) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale;

        let q_scale = if st.low_mode { 0.72 } else { match st.params.quality { 0 => 0.78, 2 => 1.0, _ => 0.92 } };
        let sw = ((pw as f32 * q_scale).round() as i32).max(64);
        let sh = ((ph as f32 * q_scale).round() as i32).max(64);
        let div = if st.low_mode { 3 } else { 2 };
        let bw = (sw / div).max(32);
        let bh = (sh / div).max(32);

        self.scene_fbo.ensure(gl, sw, sh, true);
        self.blur_a.ensure(gl, bw, bh, false);
        self.blur_b.ensure(gl, bw, bh, false);

        st.studio.recompute_hud(css_w, css_h);

        let sg = &st.studio;
        let m = sg.mood;
        let b_norm = sg.interactive_brightness;
        let bright = 0.46 + b_norm * 1.18;
        let sun_mul = (0.34 + 0.86 * sg.cur_blinds) * bright;
        let lamp_on = sg.lamp_manual.unwrap_or(true);
        let lamp_mul = if lamp_on { sg.param_desk_lamp * (0.58 + 0.62 * bright) } else { 0.0 };
        let exposure = m.exposure * (0.76 + 0.62 * b_norm);

        // Camera with gentle mouse parallax (exact studio-glass-engine.js vectors!)
        let par = sg.param_parallax * if st.low_mode { 0.35 } else { 1.0 };
        let cam_pos = Vec3(
            0.45 + (sg.ptr_smooth_x - 0.5) * 0.42 * par,
            2.02 - (sg.ptr_smooth_y - 0.5) * 0.22 * par,
            5.75,
        );
        let cam_target = Vec3(-0.08, 1.48, -0.55);
        let proj = Mat4::perspective(36.0, css_w / css_h.max(1.0), 0.1, 45.0);
        let view = Mat4::look_at(cam_pos, cam_target, Vec3(0.0, 1.0, 0.0));
        let vp = proj.mul(view);

        // ==================== PASS 1: 3D PBR Studio Scene ====================
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.scene_fbo.fbo);
        (gl.Viewport)(0, 0, sw, sh);
        (gl.Enable)(GL_DEPTH_TEST);
        (gl.DepthMask)(GL_TRUE);
        (gl.Disable)(GL_BLEND);
        (gl.ClearColor)(0.015, 0.028, 0.024, 1.0);
        (gl.Clear)(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

        // 1A. Static & articulated PBR studio meshes
        (gl.UseProgram)(self.prog_pbr);
        (gl.UniformMatrix4fv)(gl.uniform_loc(self.prog_pbr, "u_vp"), 1, GL_FALSE, vp.0.as_ptr());
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_cam_pos"), cam_pos.0, cam_pos.1, cam_pos.2);
        let sun_dir = Vec3(m.sun_pos[0] - 1.2, m.sun_pos[1] - 0.8, m.sun_pos[2] - 0.4).norm();
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_sun_dir"), sun_dir.0, sun_dir.1, sun_dir.2);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_sun_col"), m.sun_col[0], m.sun_col[1], m.sun_col[2]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_pbr, "u_sun_int"), m.sun_int * sun_mul + sg.flash * 4.5);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_hemi_sky"), m.hemi_sky[0], m.hemi_sky[1], m.hemi_sky[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_hemi_ground"), m.hemi_ground[0], m.hemi_ground[1], m.hemi_ground[2]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_pbr, "u_hemi_int"), m.hemi_int * bright);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_amb_col"), m.amb_col[0], m.amb_col[1], m.amb_col[2]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_pbr, "u_amb_int"), m.amb_int * bright);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_lamp_pos"), 2.92, 1.68, -1.15);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_pbr, "u_lamp_int"), m.lamp_int * lamp_mul);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_pbr, "u_lamp_glow"), m.lamp_glow * lamp_mul);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_screen_pos"), 0.45, 1.72, 0.25);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_pbr, "u_screen_int"), m.screen_int * (0.65 + 0.35 * bright));
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_fill_col"), m.hemi_sky[0], m.hemi_sky[1], m.hemi_sky[2]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_pbr, "u_fill_int"), 0.52 * bright * (0.50 + 0.50 * sg.cur_blinds));
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_wall_tint"), m.wall_tint[0], m.wall_tint[1], m.wall_tint[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_pbr, "u_niche_tint"), m.niche_tint[0], m.niche_tint[1], m.niche_tint[2]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_pbr, "u_blinds"), sg.cur_blinds);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_pbr, "u_kb_pos"), sg.kb_pos.0, sg.kb_pos.1);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_pbr, "u_mouse_pos"), sg.mouse_pos.0, sg.mouse_pos.1);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_pbr, "u_cup_pos"), sg.pen_cup_pos.0, sg.pen_cup_pos.1);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_pbr, "u_nb_pos"), sg.notebook_pos.0, sg.notebook_pos.1);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_pbr, "u_books_x"), sg.side_books_x);

        let u_model = gl.uniform_loc(self.prog_pbr, "u_model");
        let ident = Mat4::ident();
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, ident.0.as_ptr());
        self.mesh_static.draw(gl);
        self.mesh_lamp_glass.draw(gl);
        if lamp_mul > 0.04 {
            self.mesh_lamp_cap.draw(gl);
        }

        // Dynamic draggable 3D desk & studio props!
        let kb_m = Mat4::translate(sg.kb_pos.0, 0.92, sg.kb_pos.1).mul(Mat4::rot_y(0.04));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, kb_m.0.as_ptr());
        self.mesh_keyboard.draw(gl);

        let ms_m = Mat4::translate(sg.mouse_pos.0, 0.92, sg.mouse_pos.1).mul(Mat4::rot_y(-0.14));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, ms_m.0.as_ptr());
        self.mesh_mouse.draw(gl);

        let cup_m = Mat4::translate(sg.pen_cup_pos.0, 0.92, sg.pen_cup_pos.1);
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, cup_m.0.as_ptr());
        self.mesh_pen_cup.draw(gl);

        let nb_m = Mat4::translate(sg.notebook_pos.0, 0.92, sg.notebook_pos.1).mul(Mat4::rot_y(0.16));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, nb_m.0.as_ptr());
        self.mesh_notebook.draw(gl);

        let ch_m = Mat4::translate(sg.chair_x, 0.0, 1.38).mul(Mat4::rot_y(sg.chair_rot));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, ch_m.0.as_ptr());
        self.mesh_chair.draw(gl);

        let sb_m = Mat4::translate(sg.side_books_x, 1.04, -1.16);
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, sb_m.0.as_ptr());
        self.mesh_side_books.draw(gl);

        // Dynamic swiveling Studio Display monitor head
        let mon_xform = Mat4::translate(0.62, 0.92 + 0.82, -0.34 - 0.05)
            .mul(Mat4::rot_y(sg.cur_monitor.to_radians()));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, mon_xform.0.as_ptr());
        self.mesh_monitor_head.draw(gl);

        // Dynamic 26-slat 3D Wooden Venetian Blinds inside angled window group
        let win_xform = Mat4::translate(-3.12, 0.0, -0.32).mul(Mat4::rot_y(0.72));
        let drop_h = 2.28 * (1.0 - sg.cur_blinds * 0.72);
        let slat_tilt = lerpf(1.32, 0.24, sg.cur_blinds);
        for i in 0..SLAT_COUNT {
            let frac = (i + 1) as f32 / SLAT_COUNT as f32;
            let sm = win_xform
                .mul(Mat4::translate(0.0, 3.55 - frac * drop_h, 0.12))
                .mul(Mat4::rot_x(slat_tilt));
            (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, sm.0.as_ptr());
            self.mesh_slat.draw(gl);
        }
        let rail_m = win_xform.mul(Mat4::translate(0.0, 3.55 - drop_h - 0.03, 0.12));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, rail_m.0.as_ptr());
        self.mesh_blind_rail.draw(gl);

        // Dynamic sliding windowsill ceramic pots + spring-swaying foliage crowns
        let pl_pot_m = win_xform.mul(Mat4::translate(sg.plant_l_x, 1.09, 0.20));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, pl_pot_m.0.as_ptr());
        self.mesh_plant_l_pot.draw(gl);
        let pl_m = pl_pot_m
            .mul(Mat4::translate(0.0, 0.22 * 0.92, 0.0))
            .mul(Mat4::rot_z(sg.plant_sway[0].0))
            .mul(Mat4::rot_x(sg.plant_sway[0].0 * 0.45));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, pl_m.0.as_ptr());
        self.mesh_plant_l_crown.draw(gl);

        let pr_pot_m = win_xform.mul(Mat4::translate(sg.plant_r_x, 1.09, 0.20));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, pr_pot_m.0.as_ptr());
        self.mesh_plant_r_pot.draw(gl);
        let pr_m = pr_pot_m
            .mul(Mat4::translate(0.0, 0.17 * 0.92, 0.0))
            .mul(Mat4::rot_z(sg.plant_sway[1].0))
            .mul(Mat4::rot_x(sg.plant_sway[1].0 * 0.45));
        (gl.UniformMatrix4fv)(u_model, 1, GL_FALSE, pr_m.0.as_ptr());
        self.mesh_plant_r_crown.draw(gl);

        // 1B. Outdoor Sky + Misty Forest + Animated Rain-on-Glass Window Pane
        (gl.UseProgram)(self.prog_window);
        (gl.UniformMatrix4fv)(gl.uniform_loc(self.prog_window, "u_vp"), 1, GL_FALSE, vp.0.as_ptr());
        let pane_m = win_xform.mul(Mat4::translate(0.0, 2.32, -0.06));
        (gl.UniformMatrix4fv)(gl.uniform_loc(self.prog_window, "u_model"), 1, GL_FALSE, pane_m.0.as_ptr());
        (gl.Uniform1f)(gl.uniform_loc(self.prog_window, "u_time"), sg.time);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_window, "u_rain"), m.rain);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_window, "u_weather"), sg.param_weather);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_window, "u_sky_top"), m.sky_top[0], m.sky_top[1], m.sky_top[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_window, "u_sky_mid"), m.sky_mid[0], m.sky_mid[1], m.sky_mid[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_window, "u_sky_bot"), m.sky_bot[0], m.sky_bot[1], m.sky_bot[2]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_window, "u_stars"), m.stars);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_window, "u_flash"), sg.flash);
        self.mesh_window_pane.draw(gl);

        // 1C. Procedural Alpine Mountain Lake Studio Display Screen
        (gl.UseProgram)(self.prog_monitor);
        (gl.UniformMatrix4fv)(gl.uniform_loc(self.prog_monitor, "u_vp"), 1, GL_FALSE, vp.0.as_ptr());
        (gl.UniformMatrix4fv)(gl.uniform_loc(self.prog_monitor, "u_model"), 1, GL_FALSE, mon_xform.0.as_ptr());
        (gl.Uniform1f)(gl.uniform_loc(self.prog_monitor, "u_emissive"), 0.55 + 0.40 * bright);
        self.mesh_monitor_screen.draw(gl);

        // 1D. Additive Volumetric Sun/Moon Light Shafts & 90 Floating Dust Motes
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE);
        (gl.DepthMask)(GL_FALSE);

        let shaft_a = m.shaft_alpha * sg.param_shafts * (0.16 + 0.96 * sg.cur_blinds) * bright;
        if shaft_a > 0.004 {
            (gl.UseProgram)(self.prog_shaft);
            (gl.UniformMatrix4fv)(gl.uniform_loc(self.prog_shaft, "u_vp"), 1, GL_FALSE, vp.0.as_ptr());
            let sh_m = Mat4::translate(-1.35, 2.25, -0.35)
                .mul(Mat4::rot_z(-0.36))
                .mul(Mat4::rot_y(0.24))
                .mul(Mat4::rot_x(0.05));
            (gl.UniformMatrix4fv)(gl.uniform_loc(self.prog_shaft, "u_model"), 1, GL_FALSE, sh_m.0.as_ptr());
            (gl.Uniform3f)(gl.uniform_loc(self.prog_shaft, "u_color"), m.shaft_col[0], m.shaft_col[1], m.shaft_col[2]);
            (gl.Uniform1f)(gl.uniform_loc(self.prog_shaft, "u_alpha"), shaft_a);
            (gl.Uniform1f)(gl.uniform_loc(self.prog_shaft, "u_blinds"), sg.cur_blinds);
            (gl.Uniform1f)(gl.uniform_loc(self.prog_shaft, "u_time"), sg.time);
            self.mesh_shafts.draw(gl);
        }

        let dust_a = (m.dust * sg.param_weather * sg.param_shafts * (0.2 + 0.8 * sg.cur_blinds) * 0.55).clamp(0.0, 0.72);
        if dust_a > 0.01 && !st.low_mode {
            let mut dpos = [0.0f32; DUST_COUNT * 3];
            for (i, d) in self.dust_base.iter().enumerate() {
                dpos[i * 3] = d[0] + (sg.time * 0.32 * d[4] + d[3]).sin() * 0.16;
                dpos[i * 3 + 1] = d[1] + (sg.time * 0.25 * d[4] + d[3]).cos() * 0.12;
                dpos[i * 3 + 2] = d[2];
            }
            (gl.Enable)(GL_PROGRAM_POINT_SIZE);
            (gl.UseProgram)(self.prog_dust);
            (gl.UniformMatrix4fv)(gl.uniform_loc(self.prog_dust, "u_vp"), 1, GL_FALSE, vp.0.as_ptr());
            (gl.Uniform1f)(gl.uniform_loc(self.prog_dust, "u_pt_size"), 18.0 * dpi);
            (gl.Uniform3f)(gl.uniform_loc(self.prog_dust, "u_color"), m.shaft_col[0], m.shaft_col[1], m.shaft_col[2]);
            (gl.Uniform1f)(gl.uniform_loc(self.prog_dust, "u_alpha"), dust_a);
            (gl.BindVertexArray)(self.dust_vao);
            (gl.BindBuffer)(GL_ARRAY_BUFFER, self.dust_vbo);
            (gl.BufferData)(GL_ARRAY_BUFFER, (dpos.len() * 4) as GLsizeiptr, dpos.as_ptr() as *const c_void, GL_DYNAMIC_DRAW);
            (gl.DrawArrays)(GL_POINTS, 0, DUST_COUNT as GLsizei);
            (gl.Disable)(GL_PROGRAM_POINT_SIZE);
        }

        (gl.DepthMask)(GL_TRUE);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_BLEND);

        // ==================== PASS 2 & 3: Separable Frost Blur ====================
        let taps = if st.low_mode { 5 } else { match st.params.quality { 0 => 6, 2 => 13, _ => 9 } };
        let radius = 1.6 + sg.param_frost * if st.low_mode { 6.5 } else { 13.5 };

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.blur_a.fbo);
        (gl.Viewport)(0, 0, bw, bh);
        (gl.UseProgram)(self.prog_blur);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.scene_fbo.tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_blur, "u_tex"), 0);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_blur, "u_texel"), 1.0 / bw as f32, 1.0 / bh as f32);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_blur, "u_dir"), 1.0, 0.0);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_blur, "u_radius"), radius);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_blur, "u_taps"), taps);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.blur_b.fbo);
        (gl.BindTexture)(GL_TEXTURE_2D, self.blur_a.tex);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_blur, "u_dir"), 0.0, 1.0);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // ==================== PASS 4: Optical Refractive Glass Composite ====================
        let mut g_rects = [0.0f32; MAX_GLASS * 4];
        let mut g_meta = [0.0f32; MAX_GLASS * 4];
        let mut g_cnt: usize = 0;
        let active_preset = sg.active_preset_idx();

        // 4A. Studio Lighting HUD glass cards + vertical Brightness pill
        if sg.param_show_hud && !sg.hud_collapsed {
            for slot in 0..5 {
                let r = sg.hit_cards[slot];
                if r.w > 2.0 && r.h > 2.0 && g_cnt < MAX_GLASS {
                    let rad = if slot == 0 { 22.0 } else { 18.0 };
                    let is_act = if CARD_PRESETS[slot] == active_preset { 1.0 } else { 0.0 };
                    g_rects[g_cnt * 4] = r.x + r.w * 0.5;
                    g_rects[g_cnt * 4 + 1] = r.y + r.h * 0.5;
                    g_rects[g_cnt * 4 + 2] = r.w * 0.5;
                    g_rects[g_cnt * 4 + 3] = r.h * 0.5;
                    g_meta[g_cnt * 4] = rad;
                    g_meta[g_cnt * 4 + 1] = 0.0;
                    g_meta[g_cnt * 4 + 2] = is_act;
                    g_meta[g_cnt * 4 + 3] = 0.0;
                    g_cnt += 1;
                }
            }
            let sr = sg.hit_slider;
            if sr.w > 2.0 && sr.h > 2.0 && g_cnt < MAX_GLASS {
                g_rects[g_cnt * 4] = sr.x + sr.w * 0.5;
                g_rects[g_cnt * 4 + 1] = sr.y + sr.h * 0.5;
                g_rects[g_cnt * 4 + 2] = sr.w * 0.5;
                g_rects[g_cnt * 4 + 3] = sr.h * 0.5;
                g_meta[g_cnt * 4] = (sr.w * 0.45).min(28.0);
                g_meta[g_cnt * 4 + 1] = 2.0; // vertical brightness pill
                g_meta[g_cnt * 4 + 2] = 0.0;
                g_meta[g_cnt * 4 + 3] = sg.interactive_brightness;
                g_cnt += 1;
            }
        }

        // 4B. Adaptive desktop icon refractive glass plaques
        if st.params.adaptive && sg.param_icon_glass != 2 {
            for ic in &st.icons {
                if g_cnt >= MAX_GLASS { break; }
                let hw = (ic.w * 0.5).max(22.0) + 6.0;
                let hh = (ic.h * 0.5).max(26.0) + 4.0;
                let rad = if sg.param_icon_glass == 0 { hw.min(hh) * 0.42 } else { 12.0 };
                g_rects[g_cnt * 4] = ic.x + ic.w * 0.5;
                g_rects[g_cnt * 4 + 1] = ic.y + ic.h * 0.5;
                g_rects[g_cnt * 4 + 2] = hw;
                g_rects[g_cnt * 4 + 3] = hh;
                g_meta[g_cnt * 4] = rad;
                g_meta[g_cnt * 4 + 1] = if sg.param_icon_glass == 1 { 1.0 } else { 0.0 };
                g_meta[g_cnt * 4 + 2] = 0.15;
                g_meta[g_cnt * 4 + 3] = 0.0;
                g_cnt += 1;
            }
        }

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.UseProgram)(self.prog_glass);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.scene_fbo.tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_glass, "u_scene"), 0);
        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.blur_b.tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_glass, "u_blur"), 1);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_glass, "u_res"), css_w, css_h);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_glass, "u_count"), g_cnt as i32);
        if g_cnt > 0 {
            (gl.Uniform4fv)(gl.uniform_loc(self.prog_glass, "u_rects"), g_cnt as GLsizei, g_rects.as_ptr());
            (gl.Uniform4fv)(gl.uniform_loc(self.prog_glass, "u_meta"), g_cnt as GLsizei, g_meta.as_ptr());
        }
        (gl.Uniform1f)(gl.uniform_loc(self.prog_glass, "u_refraction"), sg.param_refraction);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_glass, "u_frost"), sg.param_frost);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_glass, "u_rim_tint"), m.rim_tint[0], m.rim_tint[1], m.rim_tint[2]);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_glass, "u_light_pos"), 0.14, 0.25);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_glass, "u_pulse_idx"), sg.pulse_card_idx);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_glass, "u_pulse_age"), sg.pulse_age);
        let pp = PULSE_PALETTES[sg.pulse_preset.min(4)];
        (gl.Uniform3f)(gl.uniform_loc(self.prog_glass, "u_pulse_col_a"), pp[0][0], pp[0][1], pp[0][2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_glass, "u_pulse_col_b"), pp[1][0], pp[1][1], pp[1][2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_glass, "u_pulse_col_c"), pp[2][0], pp[2][1], pp[2][2]);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_glass, "u_time"), sg.time);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_glass, "u_exposure"), exposure);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // ==================== PASS 5: Studio Lighting HUD Icons & Crisp Typography ====================
        if !sg.param_show_hud {
            return;
        }
        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        // Header: "Studio lighting" + 6-dot drag grip + circular sliders toggle button
        let hdr = sg.hit_header;
        let col_btn = sg.hit_collapse;
        let col_hov = st.pointer_active && col_btn.contains(st.pointer_x, st.pointer_y);
        self.draw_icon(gl, css_w, css_h, 6, col_btn, [0.0, if col_hov { 1.0 } else { 0.0 }, 0.0, col_btn.w]);

        let title_sz = (css_w * 0.014).clamp(14.0, 20.0);
        let title_w = text.measure(gl, "Studio lighting", title_sz, dpi, true);
        self.draw_icon(
            gl, css_w, css_h, 8,
            Rect { x: hdr.x + 12.0 + title_w, y: hdr.y + 11.0, w: 12.0, h: 16.0 },
            [0.0, 0.0, 0.0, 14.0],
        );

        text.begin();
        // Subtle shadow behind header text
        text.push(gl, "Studio lighting", hdr.x + 5.0, hdr.y + 25.0, title_sz, dpi, true, [0.0, 0.0, 0.0, 0.45], 0, None);
        text.push(gl, "Studio lighting", hdr.x + 4.0, hdr.y + 24.0, title_sz, dpi, true, [1.0, 1.0, 1.0, 0.94], 0, None);
        text.flush(gl, css_w, css_h);

        if sg.hud_collapsed {
            return;
        }

        // Draw 5 Studio Glass Preset Cards' Icons & Check Badges
        for slot in 0..5 {
            let r = sg.hit_cards[slot];
            if r.w <= 4.0 || r.h <= 4.0 { continue; }
            let preset_idx = CARD_PRESETS[slot];
            let is_act = preset_idx == active_preset;
            let burst_p = if sg.pulse_card_idx == slot as i32 && sg.pulse_age < 1.45 {
                (sg.pulse_age / 1.45).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let ico_rect = if slot == 0 {
                // Hero Morning Fountain Pen centered nicely in upper half
                let sz = (css_w * 0.054).clamp(56.0, 78.0);
                Rect { x: r.x + r.w * 0.46 - sz * 0.5, y: r.y + r.h * 0.34 - sz * 0.5, w: sz, h: sz }
            } else {
                let sz = (css_w * 0.022).clamp(24.0, 32.0);
                Rect { x: r.x + 11.0, y: r.y + 9.0, w: sz, h: sz }
            };
            self.draw_icon(gl, css_w, css_h, slot as i32, ico_rect, [burst_p, 0.0, 0.0, ico_rect.w]);

            if is_act {
                let ck = 16.0;
                self.draw_icon(
                    gl, css_w, css_h, 7,
                    Rect { x: r.x + r.w - 12.0 - ck, y: r.y + r.h - 11.0 - ck, w: ck, h: ck },
                    [0.0, 0.0, 0.0, ck],
                );
            }
        }

        // Brightness Pill Sun Icon
        let sr = sg.hit_slider;
        let dark_lbl = sg.interactive_brightness < 0.28;
        let sun_sz = 19.0;
        self.draw_icon(
            gl, css_w, css_h, 5,
            Rect { x: sr.x + (sr.w - sun_sz) * 0.5, y: sr.y + sr.h - 46.0, w: sun_sz, h: sun_sz },
            [0.0, if dark_lbl { 1.0 } else { 0.0 }, 0.0, sun_sz],
        );

        // Crisp Card Labels, Subtitles, Percentages & Brightness Text
        text.begin();
        for slot in 0..5 {
            let r = sg.hit_cards[slot];
            if r.w <= 4.0 || r.h <= 4.0 { continue; }
            let preset_idx = CARD_PRESETS[slot];
            let pct_str = format!("{}%", (sg.preset_brightness[preset_idx] * 100.0).round() as i32);
            let is_hero = slot == 0;
            let pad_x = if is_hero { 15.0 } else { 12.0 };
            let pct_sz = if is_hero { (css_w * 0.017).clamp(17.0, 24.0) } else { (css_w * 0.010).clamp(11.0, 14.0) };
            let name_sz = if is_hero { (css_w * 0.020).clamp(19.0, 27.0) } else { (css_w * 0.0098).clamp(11.5, 14.5) };
            let sub_sz = if is_hero { (css_w * 0.0082).clamp(10.0, 12.0) } else { (css_w * 0.0074).clamp(9.5, 11.0) };

            let pct_y = r.y + if is_hero { 32.0 } else { 22.0 };
            text.push(gl, &pct_str, r.x + r.w - pad_x + 1.0, pct_y + 1.0, pct_sz, dpi, true, [0.0, 0.0, 0.0, 0.45], 2, None);
            text.push(gl, &pct_str, r.x + r.w - pad_x, pct_y, pct_sz, dpi, true, [1.0, 1.0, 1.0, 0.92], 2, None);

            let sub = CARD_SUBS[slot];
            let max_tw = (r.w - pad_x * 2.0 - 20.0).max(40.0);
            if !sub.is_empty() {
                let ny = r.y + r.h - if is_hero { 32.0 } else { 25.0 };
                let sy = r.y + r.h - if is_hero { 14.0 } else { 11.0 };
                text.push(gl, CARD_NAMES[slot], r.x + pad_x + 1.0, ny + 1.0, name_sz, dpi, true, [0.0, 0.0, 0.0, 0.50], 0, Some(max_tw));
                text.push(gl, CARD_NAMES[slot], r.x + pad_x, ny, name_sz, dpi, true, [1.0, 1.0, 1.0, 0.96], 0, Some(max_tw));
                text.push(gl, sub, r.x + pad_x, sy, sub_sz, dpi, false, [1.0, 1.0, 1.0, 0.78], 0, Some(max_tw));
            } else {
                let ny = r.y + r.h - 13.0;
                text.push(gl, CARD_NAMES[slot], r.x + pad_x + 1.0, ny + 1.0, name_sz, dpi, true, [0.0, 0.0, 0.0, 0.50], 0, Some(max_tw));
                text.push(gl, CARD_NAMES[slot], r.x + pad_x, ny, name_sz, dpi, true, [1.0, 1.0, 1.0, 0.96], 0, Some(max_tw));
            }
        }

        let br_col = if dark_lbl { [0.96, 0.97, 0.99, 0.92] } else { [0.09, 0.10, 0.12, 0.94] };
        let br_sz = (css_w * 0.0082).clamp(10.0, 12.0);
        text.push(gl, "Brightness", sr.x + sr.w * 0.5, sr.y + sr.h - 13.0, br_sz, dpi, true, br_col, 1, Some(sr.w - 8.0));
        text.flush(gl, css_w, css_h);
    }
}

struct StudioMeshes {
    static_room: GlMesh,
    keyboard: GlMesh,
    mouse: GlMesh,
    pen_cup: GlMesh,
    notebook: GlMesh,
    chair: GlMesh,
    side_books: GlMesh,
    plant_l_pot: GlMesh,
    plant_r_pot: GlMesh,
    lamp_glass: GlMesh,
    lamp_cap: GlMesh,
    window_pane: GlMesh,
    slat: GlMesh,
    blind_rail: GlMesh,
    plant_l_crown: GlMesh,
    plant_r_crown: GlMesh,
    monitor_head: GlMesh,
    monitor_screen: GlMesh,
    shafts: GlMesh,
}

// =========================================================================
// Build Complete 1:1 3D Designer Studio Geometry (Exact studio-glass-engine.js coordinates!)
// =========================================================================
unsafe fn build_studio_meshes(gl: &GlFns) -> StudioMeshes {
    let mut b = MeshBuilder::new();
    let solid = |hex: u32| -> [f32; 4] { let c = rgb(hex); [c[0], c[1], c[2], 0.0] };
    let walnut = |hex: u32| -> [f32; 4] { let c = rgb(hex); [c[0], c[1], c[2], 1.0] };
    let wall_green = [1.0, 1.0, 1.0, 2.0];
    let wall_niche = [1.0, 1.0, 1.0, 3.0];
    let lamp_glass_col = [1.0, 0.94, 0.85, 4.0];
    let lamp_cap_col = [1.0, 0.84, 0.62, 5.0];

    // 1. Studio Floor, Ceiling, Main Green Plaster Back Wall & Warm Ochre Recess Niche
    b.add_plane(Mat4::rot_x(-std::f32::consts::FRAC_PI_2), 16.0, 14.0, solid(0x161412), [0.86, 0.05]);
    b.add_plane(Mat4::translate(0.0, 4.9, 0.0).mul(Mat4::rot_x(std::f32::consts::FRAC_PI_2)), 16.0, 14.0, solid(0x141A18), [0.95, 0.0]);
    b.add_box(Mat4::translate(-0.2, 2.5, -1.85), 5.6, 5.4, 0.28, wall_green, [0.90, 0.02]);
    b.add_box(Mat4::translate(4.3, 2.5, -2.35), 4.4, 5.4, 0.25, wall_niche, [0.88, 0.02]);
    b.add_box(Mat4::translate(2.42, 2.5, -1.98), 0.42, 5.4, 0.62, wall_niche, [0.88, 0.02]);

    // 2. Angled Left Architectural Window Surround, Walnut Frame, Mullions & Brass Handle
    let win = Mat4::translate(-3.12, 0.0, -0.32).mul(Mat4::rot_y(0.72));
    b.add_box(win.mul(Mat4::translate(0.0, 0.51, 0.0)), 4.4, 1.02, 0.34, wall_green, [0.90, 0.02]);
    b.add_box(win.mul(Mat4::translate(0.0, 4.25, 0.0)), 4.4, 1.35, 0.34, wall_green, [0.90, 0.02]);
    b.add_box(win.mul(Mat4::translate(-1.78, 2.32, 0.0)), 0.85, 2.75, 0.34, wall_green, [0.90, 0.02]);
    b.add_box(win.mul(Mat4::translate( 1.78, 2.32, 0.0)), 0.85, 2.75, 0.34, wall_green, [0.90, 0.02]);

    let w_frame = walnut(0x6B452C);
    b.add_box(win.mul(Mat4::translate(0.0, 1.04, 0.14)), 2.96, 0.09, 0.52, w_frame, [0.58, 0.04]);
    b.add_box(win.mul(Mat4::translate(0.0, 3.60, 0.02)), 2.84, 0.10, 0.24, w_frame, [0.58, 0.04]);
    b.add_box(win.mul(Mat4::translate(-1.36, 2.32, 0.02)), 0.10, 2.58, 0.24, w_frame, [0.58, 0.04]);
    b.add_box(win.mul(Mat4::translate( 1.36, 2.32, 0.02)), 0.10, 2.58, 0.24, w_frame, [0.58, 0.04]);
    b.add_box(win.mul(Mat4::translate(0.08, 2.32, 0.02)), 0.085, 2.52, 0.18, w_frame, [0.58, 0.04]);
    b.add_box(win.mul(Mat4::translate(0.0, 2.78, 0.02)), 2.68, 0.065, 0.15, w_frame, [0.58, 0.04]);
    b.add_box(win.mul(Mat4::translate(0.08, 2.15, 0.12)), 0.025, 0.14, 0.05, solid(0xC9A462), [0.32, 0.82]);
    b.add_box(win.mul(Mat4::translate(0.0, 3.57, 0.14)), 2.74, 0.12, 0.14, walnut(0x8A5B39), [0.55, 0.05]);

    // 3. Walnut Designer Desk, Legs, Monitor Stand & Right Side Console Table
    let desk_y = 0.92;
    b.add_box(Mat4::translate(0.55, desk_y - 0.0475, 0.08), 5.1, 0.095, 1.92, walnut(0xA26C44), [0.42, 0.04]);
    b.add_box(Mat4::translate(-1.82, (desk_y - 0.08) * 0.5, 0.08), 0.08, desk_y - 0.08, 1.56, solid(0x1B1C1F), [0.55, 0.50]);
    b.add_box(Mat4::translate( 2.92, (desk_y - 0.08) * 0.5, 0.08), 0.08, desk_y - 0.08, 1.56, solid(0x1B1C1F), [0.55, 0.50]);

    let alu = solid(0xC2C7CE);
    b.add_box(Mat4::translate(0.62, desk_y + 0.009, -0.32), 0.56, 0.018, 0.42, alu, [0.28, 0.85]);
    b.add_box(Mat4::translate(0.62, desk_y + 0.22, -0.44).mul(Mat4::rot_x(0.06)), 0.28, 0.42, 0.024, alu, [0.28, 0.85]);

    let side_y = 1.04;
    b.add_box(Mat4::translate(2.96, side_y - 0.0375, -1.25), 1.75, 0.075, 0.85, walnut(0x8D5935), [0.48, 0.04]);
    for i in 0..2 {
        let fi = i as f32;
        b.add_box(Mat4::translate(3.38, side_y + 0.021 + fi * 0.042, -1.18), 0.36 - fi * 0.03, 0.042, 0.28, solid(if i == 0 { 0x2B333A } else { 0x7B563A }), [0.78, 0.02]);
    }
    let static_room = b.upload(gl);

    // 4. Separate Movable 3D Mechanical Keyboard (local origin at base center)
    let mut bkb = MeshBuilder::new();
    bkb.add_box(Mat4::translate(0.0, 0.018, 0.0), 0.84, 0.036, 0.30, solid(0x26292E), [0.42, 0.62]);
    for r in 0..5 {
        for c in 0..14 {
            let kx = -0.365 + c as f32 * 0.056;
            let kz = -0.104 + r as f32 * 0.052;
            let ky = 0.042 + (4 - r) as f32 * 0.0025;
            let is_space = r == 4 && (4..=8).contains(&c);
            let is_accent = (r == 0 && c == 0) || (r == 2 && c == 13);
            let is_edge = c == 0 || c == 13 || (r == 4 && (c <= 2 || c >= 10));
            let kcol = if is_accent { solid(0xB8844C) } else if is_edge { solid(0x5A574F) } else { solid(0x3A3D42) };
            bkb.add_box(Mat4::translate(kx, ky, kz), if is_space { 0.053 } else { 0.046 }, 0.022, 0.044, kcol, [0.55, 0.08]);
        }
    }
    let keyboard = bkb.upload(gl);

    // 5. Separate Movable 3D Wireless Studio Mouse
    let mut bms = MeshBuilder::new();
    bms.add_sphere_part(Mat4::translate(0.0, 0.028, 0.0).mul(Mat4::scale(0.86, 0.44, 1.38)), 0.085, 14, 10, std::f32::consts::PI, solid(0x2C2F34), [0.36, 0.18]);
    bms.add_cylinder(Mat4::translate(0.0, 0.055, -0.036).mul(Mat4::rot_z(std::f32::consts::FRAC_PI_2)), 0.012, 0.012, 0.022, 8, solid(0x7A808A), [0.35, 0.70]);
    let mouse = bms.upload(gl);

    // 6. Separate Movable Ceramic Pen Cup + 5 Angled Pencils
    let mut bcup = MeshBuilder::new();
    bcup.add_cylinder(Mat4::translate(0.0, 0.09, 0.0), 0.072, 0.064, 0.18, 14, solid(0x3B3530), [0.85, 0.04]);
    let pen_cols = [0x1C1F24u32, 0xC89A56, 0x3B6C6A, 0x8F4338, 0xD9D2C5];
    for (i, &pc) in pen_cols.iter().enumerate() {
        let a = (i as f32 / 5.0) * std::f32::consts::TAU;
        let pm = Mat4::translate(a.cos() * 0.024, 0.145, a.sin() * 0.024)
            .mul(Mat4::rot_z((i as f32 - 2.0) * 0.09))
            .mul(Mat4::rot_x(a.sin() * 0.08));
        bcup.add_cylinder(pm, 0.007, 0.007, 0.24, 6, solid(pc), [0.45, 0.05]);
    }
    let pen_cup = bcup.upload(gl);

    // 7. Separate Movable Black Leather Sketchbook / Notebook
    let mut bnb = MeshBuilder::new();
    bnb.add_box(Mat4::translate(0.0, 0.011, 0.0), 0.32, 0.022, 0.42, solid(0x1E1F22), [0.72, 0.04]);
    bnb.add_box(Mat4::translate(0.006, 0.011, 0.0), 0.305, 0.016, 0.402, solid(0xE2DACB), [0.85, 0.0]);
    let notebook = bnb.upload(gl);

    // 8. Separate Movable Ergonomic Studio Task Chair
    let mut bch = MeshBuilder::new();
    bch.add_box(Mat4::translate(0.0, 0.64, 0.0), 0.76, 0.095, 0.72, solid(0x1C2123), [0.86, 0.04]);
    bch.add_box(Mat4::translate(0.0, 1.02, 0.34).mul(Mat4::rot_x(0.14)), 0.74, 0.68, 0.08, solid(0x1C2123), [0.86, 0.04]);
    bch.add_box(Mat4::translate(0.0, 1.02, 0.375).mul(Mat4::rot_x(0.14)), 0.77, 0.71, 0.04, solid(0x14171A), [0.48, 0.35]);
    for side in [-1.0f32, 1.0] {
        bch.add_box(Mat4::translate(side * 0.39, 0.78, 0.05), 0.04, 0.24, 0.05, solid(0x14171A), [0.48, 0.35]);
        bch.add_box(Mat4::translate(side * 0.39, 0.90, 0.04), 0.07, 0.032, 0.38, solid(0x14171A), [0.48, 0.35]);
    }
    bch.add_cylinder(Mat4::translate(0.0, 0.34, 0.0), 0.042, 0.05, 0.56, 10, solid(0x626973), [0.32, 0.82]);
    for i in 0..5 {
        let a = (i as f32 / 5.0) * std::f32::consts::TAU;
        bch.add_box(Mat4::translate(a.sin() * 0.18, 0.07, a.cos() * 0.18).mul(Mat4::rot_y(a)), 0.04, 0.035, 0.38, solid(0x14171A), [0.48, 0.35]);
    }
    let chair = bch.upload(gl);

    // 9. Separate Movable Stacked Art Books + Ceramic Vase + Botanical Sprig
    let mut bsb = MeshBuilder::new();
    let bk_cols = [0x6E3729u32, 0x2C3B36, 0xC7B99E];
    for (i, &bc) in bk_cols.iter().enumerate() {
        let fi = i as f32;
        bsb.add_box(
            Mat4::translate(0.0, 0.022 + fi * 0.044, 0.0).mul(Mat4::rot_y((fi - 1.0) * 0.05)),
            0.46 - fi * 0.04, 0.044, 0.34 - fi * 0.02, solid(bc), [0.75, 0.02],
        );
    }
    bsb.add_sphere_part(Mat4::translate(0.02, 0.21, 0.0).mul(Mat4::scale(1.0, 1.25, 1.0)), 0.068, 12, 10, std::f32::consts::PI, solid(0x9C8974), [0.68, 0.04]);
    bsb.add_cylinder(Mat4::translate(-0.04, 0.38, 0.0).mul(Mat4::rot_z(0.42)), 0.004, 0.006, 0.34, 5, solid(0x3B3127), [0.90, 0.0]);
    let side_books = bsb.upload(gl);

    // 10. Separate Sliding Windowsill Ceramic Pots
    let mut bpl = MeshBuilder::new();
    bpl.add_cylinder(Mat4::translate(0.0, 0.11, 0.0), 0.18, 0.18 * 0.74, 0.22, 16, solid(0x26292A), [0.78, 0.05]);
    bpl.add_cylinder(Mat4::translate(0.0, 0.22 * 0.94, 0.0), 0.18 * 0.88, 0.18 * 0.88, 0.02, 12, solid(0x221914), [0.96, 0.0]);
    let plant_l_pot = bpl.upload(gl);

    let mut bpr = MeshBuilder::new();
    bpr.add_cylinder(Mat4::translate(0.0, 0.085, 0.0), 0.14, 0.14 * 0.74, 0.17, 16, solid(0x26292A), [0.78, 0.05]);
    bpr.add_cylinder(Mat4::translate(0.0, 0.17 * 0.94, 0.0), 0.14 * 0.88, 0.14 * 0.88, 0.02, 12, solid(0x221914), [0.96, 0.0]);
    let plant_r_pot = bpr.upload(gl);

    // 11. Iconic Frosted Glass Mushroom Lamp
    let mut bl = MeshBuilder::new();
    let lm = Mat4::translate(2.92, side_y, -1.16);
    bl.add_cylinder(lm.mul(Mat4::translate(0.0, 0.17, 0.0)), 0.085, 0.135, 0.34, 22, lamp_glass_col, [0.34, 0.04]);
    bl.add_sphere_part(lm.mul(Mat4::translate(0.0, 0.31, 0.0)).mul(Mat4::scale(1.0, 0.78, 1.0)), 0.285, 26, 14, std::f32::consts::PI * 0.56, lamp_glass_col, [0.34, 0.04]);
    let lamp_glass = bl.upload(gl);

    let mut bc = MeshBuilder::new();
    bc.add_cylinder(lm.mul(Mat4::translate(0.0, 0.295, 0.0)), 0.27, 0.27, 0.012, 20, lamp_cap_col, [0.5, 0.0]);
    let lamp_cap = bc.upload(gl);

    let mut bw = MeshBuilder::new();
    bw.add_plane(Mat4::ident(), 2.66, 2.52, [1.0; 4], [0.5, 0.0]);
    let window_pane = bw.upload(gl);

    let mut bs = MeshBuilder::new();
    bs.add_box(Mat4::ident(), 2.62, 0.014, 0.088, walnut(0x8A5B39), [0.55, 0.05]);
    let slat = bs.upload(gl);

    let mut br = MeshBuilder::new();
    br.add_box(Mat4::ident(), 2.64, 0.042, 0.095, walnut(0x8A5B39), [0.55, 0.05]);
    let blind_rail = br.upload(gl);

    let plant_l_crown = build_plant_crown(gl, 0.18, 0.22, 16, 101);
    let plant_r_crown = build_plant_crown(gl, 0.14, 0.17, 11, 202);

    let mut bm = MeshBuilder::new();
    bm.add_box(Mat4::ident(), 1.78, 1.06, 0.048, alu, [0.28, 0.85]);
    bm.add_box(Mat4::translate(0.0, 0.0, 0.025), 1.75, 1.03, 0.014, solid(0x111317), [0.25, 0.35]);
    let monitor_head = bm.upload(gl);

    let mut bsc = MeshBuilder::new();
    bsc.add_plane(Mat4::translate(0.0, 0.0, 0.034), 1.66, 0.95, [1.0; 4], [0.14, 0.05]);
    let monitor_screen = bsc.upload(gl);

    let mut bsh = MeshBuilder::new();
    for i in 0..3 {
        let fi = i as f32;
        bsh.add_plane(Mat4::translate(fi * 0.16, -fi * 0.12, (fi - 1.0) * 0.42), 5.6, 2.45, [1.0; 4], [0.5, 0.0]);
    }
    let shafts = bsh.upload(gl);

    StudioMeshes {
        static_room,
        keyboard,
        mouse,
        pen_cup,
        notebook,
        chair,
        side_books,
        plant_l_pot,
        plant_r_pot,
        lamp_glass,
        lamp_cap,
        window_pane,
        slat,
        blind_rail,
        plant_l_crown,
        plant_r_crown,
        monitor_head,
        monitor_screen,
        shafts,
    }
}

unsafe fn build_plant_crown(gl: &GlFns, pot_r: f32, pot_h: f32, count: usize, mut seed: u32) -> GlMesh {
    let mut next_r = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed as f32) / 4294967296.0
    };
    let mut b = MeshBuilder::new();
    let leaf_a = { let c = rgb(0x243B2A); [c[0], c[1], c[2], 0.0] };
    let leaf_b = { let c = rgb(0x304D34); [c[0], c[1], c[2], 0.0] };
    let stem_c = { let c = rgb(0x2A3B26); [c[0], c[1], c[2], 0.0] };

    for i in 0..count {
        let ang = (i as f32 / count as f32) * std::f32::consts::TAU + (next_r() - 0.5) * 0.45;
        let tilt = 0.25 + next_r() * 0.68;
        let len = pot_h * (0.95 + next_r() * 1.15);
        let lr = pot_r * (0.34 + next_r() * 0.24);
        let br = Mat4::rot_y(ang).mul(Mat4::rot_z(tilt));
        b.add_cylinder(br.mul(Mat4::translate(0.0, len * 0.5, 0.0)), 0.006, 0.009, len, 5, stem_c, [0.82, 0.0]);
        b.add_sphere_part(
            br.mul(Mat4::translate(0.0, len, 0.0)).mul(Mat4::rot_z(0.35)).mul(Mat4::scale(1.35, 0.28, 0.72)),
            lr, 7, 5, std::f32::consts::PI, if i % 2 == 0 { leaf_a } else { leaf_b }, [0.58, 0.02],
        );
    }
    b.upload(gl)
}
