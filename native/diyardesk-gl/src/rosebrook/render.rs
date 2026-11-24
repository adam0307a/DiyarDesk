// RosebrookRenderer — OpenGL 3.3 Core 1:1 GPU pipeline
use super::atlas::build_rosebrook_atlas;
use super::shaders::*;
use super::{make_noise_image, RosebrookState, RB_STEP};
use crate::gl::*;
use crate::state::AppState;
use std::ffi::c_void;

pub struct RosebrookRenderer {
    ground_prog: GLuint,
    inst_prog: GLuint,
    bg_vao: GLuint,
    inst_vao: GLuint,
    inst_buf_a: GLuint,
    inst_buf_b: GLuint,
    max_inst: usize,
    noise_tex: GLuint,
    field_tex: [GLuint; 2],
    atlas_tex: GLuint,
    field_gw: usize,
    field_gh: usize,
    uploaded_field_version: u64,
    uploaded_noise_version: u64,
    u_noise: GLint,
    u_fa: GLint,
    u_fb: GLint,
    u_res: GLint,
    u_grid: GLint,
    u_scale: GLint,
    u_seed: GLint,
    u_step: GLint,
    u_mix: GLint,
    u_time: GLint,
    u_stream_w: GLint,
    u_flow: GLint,
    u_sun: GLint,
    u_slope: GLint,
    u_cy: GLint,
    u_debug: GLint,
    ui_atlas: GLint,
    ui_res: GLint,
    ui_time: GLint,
    ui_breeze: GLint,
    buf_a: Vec<f32>,
    buf_b: Vec<f32>,
}

impl RosebrookRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let ground_prog = gl.compile_program(ROSEBROOK_VERT, ROSEBROOK_FRAG)?;
        let inst_prog = gl.compile_program(ROSEBROOK_INST_VERT, ROSEBROOK_INST_FRAG)?;

        let mut bg_vao = 0;
        (gl.GenVertexArrays)(1, &mut bg_vao);
        let mut inst_vao = 0;
        (gl.GenVertexArrays)(1, &mut inst_vao);
        (gl.BindVertexArray)(inst_vao);
        let mut inst_buf_a = 0;
        let mut inst_buf_b = 0;
        (gl.GenBuffers)(1, &mut inst_buf_a);
        (gl.GenBuffers)(1, &mut inst_buf_b);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_buf_a);
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 4, GL_FLOAT, GL_FALSE, 0, std::ptr::null());
        (gl.VertexAttribDivisor)(0, 1);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, inst_buf_b);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 4, GL_FLOAT, GL_FALSE, 0, std::ptr::null());
        (gl.VertexAttribDivisor)(1, 1);
        (gl.BindVertexArray)(0);

        let mut noise_tex = 0;
        (gl.GenTextures)(1, &mut noise_tex);
        let mut field_tex = [0u32; 2];
        (gl.GenTextures)(2, field_tex.as_mut_ptr());

        let (aw, ah, apx) = build_rosebrook_atlas();
        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        (gl.ActiveTexture)(GL_TEXTURE6);
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexImage2D)(GL_TEXTURE_2D, 0, GL_RGBA8 as GLint, aw as GLsizei, ah as GLsizei,
            0, GL_RGBA, GL_UNSIGNED_BYTE, apx.as_ptr() as *const c_void);
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            ground_prog,
            inst_prog,
            bg_vao,
            inst_vao,
            inst_buf_a,
            inst_buf_b,
            max_inst: 0,
            noise_tex,
            field_tex,
            atlas_tex,
            field_gw: 0,
            field_gh: 0,
            uploaded_field_version: u64::MAX,
            uploaded_noise_version: u64::MAX,
            u_noise: gl.uniform_loc(ground_prog, "uNoise"),
            u_fa: gl.uniform_loc(ground_prog, "uFA"),
            u_fb: gl.uniform_loc(ground_prog, "uFB"),
            u_res: gl.uniform_loc(ground_prog, "uRes"),
            u_grid: gl.uniform_loc(ground_prog, "uGrid"),
            u_scale: gl.uniform_loc(ground_prog, "uScale"),
            u_seed: gl.uniform_loc(ground_prog, "uSeed"),
            u_step: gl.uniform_loc(ground_prog, "uStep"),
            u_mix: gl.uniform_loc(ground_prog, "uMix"),
            u_time: gl.uniform_loc(ground_prog, "uTime"),
            u_stream_w: gl.uniform_loc(ground_prog, "uStreamW"),
            u_flow: gl.uniform_loc(ground_prog, "uFlow"),
            u_sun: gl.uniform_loc(ground_prog, "uSun"),
            u_slope: gl.uniform_loc(ground_prog, "uSlope"),
            u_cy: gl.uniform_loc(ground_prog, "uCy"),
            u_debug: gl.uniform_loc(ground_prog, "uDebug"),
            ui_atlas: gl.uniform_loc(inst_prog, "uAtlas"),
            ui_res: gl.uniform_loc(inst_prog, "uRes"),
            ui_time: gl.uniform_loc(inst_prog, "uTime"),
            ui_breeze: gl.uniform_loc(inst_prog, "uBreeze"),
            buf_a: Vec::new(),
            buf_b: Vec::new(),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.ground_prog != 0 { (gl.DeleteProgram)(self.ground_prog); }
        if self.inst_prog != 0 { (gl.DeleteProgram)(self.inst_prog); }
        if self.bg_vao != 0 { (gl.DeleteVertexArrays)(1, &self.bg_vao); }
        if self.inst_vao != 0 { (gl.DeleteVertexArrays)(1, &self.inst_vao); }
        if self.inst_buf_a != 0 { (gl.DeleteBuffers)(1, &self.inst_buf_a); }
        if self.inst_buf_b != 0 { (gl.DeleteBuffers)(1, &self.inst_buf_b); }
        if self.noise_tex != 0 { (gl.DeleteTextures)(1, &self.noise_tex); }
        (gl.DeleteTextures)(2, self.field_tex.as_mut_ptr());
        if self.atlas_tex != 0 { (gl.DeleteTextures)(1, &self.atlas_tex); }
    }

    unsafe fn sync_noise_tex(&mut self, gl: &GlFns, s: &RosebrookState) {
        if self.uploaded_noise_version == s.noise_version { return; }
        let (sz, px) = make_noise_image(s.params.seed);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexImage2D)(GL_TEXTURE_2D, 0, GL_RGBA8 as GLint, sz as GLsizei, sz as GLsizei,
            0, GL_RGBA, GL_UNSIGNED_BYTE, px.as_ptr() as *const c_void);
        (gl.GenerateMipmap)(GL_TEXTURE_2D);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);
        self.uploaded_noise_version = s.noise_version;
    }

    unsafe fn sync_field_textures(&mut self, gl: &GlFns, s: &RosebrookState) {
        if self.uploaded_field_version == s.field_version { return; }
        for (i, data) in [&s.field_a, &s.field_b].iter().enumerate() {
            (gl.ActiveTexture)(GL_TEXTURE1 + i as GLenum);
            (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[i]);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            if self.field_gw_mismatch(s) {
                (gl.TexImage2D)(GL_TEXTURE_2D, 0, GL_R32F as GLint, s.gw as GLsizei, s.gh as GLsizei,
                    0, GL_RED, GL_FLOAT, data.as_ptr() as *const c_void);
            } else {
                (gl.TexSubImage2D)(GL_TEXTURE_2D, 0, 0, 0, s.gw as GLsizei, s.gh as GLsizei,
                    GL_RED, GL_FLOAT, data.as_ptr() as *const c_void);
            }
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        }
        self.field_gw = s.gw;
        self.field_gh = s.gh;
        self.uploaded_field_version = s.field_version;
    }

    fn field_gw_mismatch(&self, s: &RosebrookState) -> bool {
        self.field_gw != s.gw || self.field_gh != s.gh
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let dpi = st.dpi_scale.max(0.5);
        let css_w = st.css_w();
        let css_h = st.css_h();
        st.rosebrook.resize(css_w as f64, css_h as f64);
        let s = &st.rosebrook;
        self.sync_noise_tex(gl, s);
        self.sync_field_textures(gl, s);

        let res_x = pw as f32 / dpi;
        let res_y = ph as f32 / dpi;
        let (bs, slope, cy) = s.brook_params();

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_CULL_FACE);
        (gl.Disable)(GL_BLEND);
        (gl.Disable)(GL_SCISSOR_TEST);

        // Pass 1 — emerald meadow + winding crystal brook
        (gl.UseProgram)(self.ground_prog);
        (gl.BindVertexArray)(self.bg_vao);
        (gl.ActiveTexture)(GL_TEXTURE0); (gl.BindTexture)(GL_TEXTURE_2D, self.noise_tex);
        (gl.ActiveTexture)(GL_TEXTURE1); (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[0]);
        (gl.ActiveTexture)(GL_TEXTURE2); (gl.BindTexture)(GL_TEXTURE_2D, self.field_tex[1]);
        (gl.Uniform1i)(self.u_noise, 0);
        (gl.Uniform1i)(self.u_fa, 1);
        (gl.Uniform1i)(self.u_fb, 2);
        (gl.Uniform2f)(self.u_res, res_x, res_y);
        (gl.Uniform2f)(self.u_grid, s.gw as f32, s.gh as f32);
        (gl.Uniform1f)(self.u_scale, dpi);
        (gl.Uniform1f)(self.u_seed, bs as f32);
        (gl.Uniform1f)(self.u_step, RB_STEP as f32);
        (gl.Uniform1f)(self.u_mix, s.cur_mix as f32);
        (gl.Uniform1f)(self.u_time, s.time_sec as f32);
        (gl.Uniform1f)(self.u_stream_w, s.params.stream as f32);
        (gl.Uniform1f)(self.u_flow, s.params.flow as f32);
        (gl.Uniform1f)(self.u_sun, s.params.sunlight as f32);
        (gl.Uniform1f)(self.u_slope, slope as f32);
        (gl.Uniform1f)(self.u_cy, cy as f32);
        (gl.Uniform1f)(self.u_debug, if s.params.debug { 1.0 } else { 0.0 });
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // Pass 2 — instanced roses, daisies, petals & butterflies
        self.buf_a.clear();
        self.buf_b.clear();
        let mut list: Vec<(f64, f64, f64, usize, f64, f64, f64, f64)> = Vec::new();
        for b in &s.ambient_blooms {
            if b.cur < 0.03 { continue; }
            let sc = b.scale * b.cur.min(1.0);
            list.push((b.x, b.y, sc, b.variant, b.rot, b.phase, b.sway, b.cur.min(1.0) * 1.15));
        }
        for ib in &s.icon_blooms {
            let t = (s.time_sec * 1000.0 - ib.born) / 720.0;
            if t <= 0.0 { continue; }
            let e = super::RosebrookState::back_out_ease(t);
            if e <= 0.01 { continue; }
            let spin = (1.0 - t.min(1.0)) * 0.55;
            list.push((ib.x, ib.y, ib.scale * e, ib.variant, ib.rot + spin, ib.phase, ib.sway, (t * 2.2).min(1.0)));
        }
        for p in &s.drifting_petals {
            let fade: f64 = if p.burst {
                let tail = (p.life - (p.max_life - 0.7f64)).max(0.0f64);
                (1.0f64 - tail / 0.7f64).max(0.0f64)
            } else { 1.0f64 };
            let wing = if p.variant >= 10 {
                0.68 + 0.32 * (s.time_sec * 14.0 + p.phase).sin()
            } else { 1.0 };
            list.push((p.x, p.y, p.scale * wing, p.variant, p.rot, p.phase, 0.35, 0.92 * fade));
        }
        // Back-to-front by Y; drifting petals (variant >= 8) float on top
        list.sort_by(|a, b| {
            let ka = a.1 + if a.3 >= 8 { 5000.0 } else { 0.0 };
            let kb = b.1 + if b.3 >= 8 { 5000.0 } else { 0.0 };
            ka.total_cmp(&kb)
        });
        for it in &list {
            self.buf_a.push(it.0 as f32);
            self.buf_a.push(it.1 as f32);
            self.buf_a.push(it.2 as f32);
            self.buf_a.push(it.3 as f32);
            self.buf_b.push(it.4 as f32);
            self.buf_b.push(it.5 as f32);
            self.buf_b.push(it.6 as f32);
            self.buf_b.push(it.7 as f32);
        }
        let count = list.len();
        if count > 0 {
            (gl.UseProgram)(self.inst_prog);
            (gl.BindVertexArray)(self.inst_vao);
            (gl.Enable)(GL_BLEND);
            (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
            (gl.ActiveTexture)(GL_TEXTURE6);
            (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
            (gl.Uniform1i)(self.ui_atlas, 6);
            (gl.Uniform2f)(self.ui_res, res_x, res_y);
            (gl.Uniform1f)(self.ui_time, s.time_sec as f32);
            (gl.Uniform1f)(self.ui_breeze, s.params.breeze as f32);
            let bytes = (count * 4 * std::mem::size_of::<f32>()) as GLsizeiptr;
            if count > self.max_inst {
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_a);
                (gl.BufferData)(GL_ARRAY_BUFFER, bytes, self.buf_a.as_ptr() as *const c_void, GL_DYNAMIC_DRAW);
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_b);
                (gl.BufferData)(GL_ARRAY_BUFFER, bytes, self.buf_b.as_ptr() as *const c_void, GL_DYNAMIC_DRAW);
                self.max_inst = count;
            } else {
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_a);
                (gl.BufferSubData)(GL_ARRAY_BUFFER, 0, bytes, self.buf_a.as_ptr() as *const c_void);
                (gl.BindBuffer)(GL_ARRAY_BUFFER, self.inst_buf_b);
                (gl.BufferSubData)(GL_ARRAY_BUFFER, 0, bytes, self.buf_b.as_ptr() as *const c_void);
            }
            (gl.DrawArraysInstanced)(GL_TRIANGLE_STRIP, 0, 4, count as GLsizei);
            (gl.Disable)(GL_BLEND);
            (gl.BindVertexArray)(0);
        }
    }
}
