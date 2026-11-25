//! Custom user-imported OpenGL 3.3 Core GLSL shader theme runner (`engine: "opengl-glsl"`).
//!
//! Reads a fragment shader (and optional vertex shader + up to 4 RGBA8 textures)
//! from an imported theme directory and binds DiyarDesk's standard uniforms:
//! - `u_res` (`vec2`, CSS px), `u_res_px` (`vec2`, physical px)
//! - `u_time`, `u_dt`, `u_seed`, `u_solar` (`float`)
//! - `u_mouse` (`vec4`: x, y, active, down), `u_pointer` (`vec2`)
//! - `u_adaptive` (`int`), `u_icon_count` (`int`), `u_icons[64]` (`vec4`: x, y, w, h in CSS px)
//! - `u_param_f[16]` (`float`), `u_param_c[8]` (`vec4`)
//! - `u_tex0..u_tex3` (`sampler2D`)
//! - Plus automatic named uniforms `u_<key>` for every float/bool/color param in `raw_params`.

use crate::gl::*;
use crate::shaders::FULLSCREEN_VERT;
use crate::state::AppState;
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Component, Path, PathBuf};

pub const MAX_CUSTOM_ICONS: usize = 64;
pub const MAX_CUSTOM_TEXTURES: usize = 4;

#[derive(Clone, Debug, Default)]
pub struct CustomGlSpec {
    pub engine: String, // "opengl-glsl" | "opengl-rust"
    pub theme_dir: PathBuf,
    pub entry: String,
    pub vert_entry: Option<String>,
    pub textures: Vec<String>,
    pub param_order: Vec<String>,
    pub color_order: Vec<String>,
    pub rev: u64,
}

impl CustomGlSpec {
    pub fn is_glsl(&self) -> bool {
        self.engine == "opengl-glsl" && !self.entry.is_empty() && !self.theme_dir.as_os_str().is_empty()
    }

    pub fn is_rust_plugin(&self) -> bool {
        self.engine == "opengl-rust" && !self.entry.is_empty() && !self.theme_dir.as_os_str().is_empty()
    }
}

/// Safely resolve a relative path strictly inside `base_dir` (rejects `..` and absolute paths).
pub fn resolve_inside_dir(base_dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let trimmed = rel.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return Err("Empty relative path".into());
    }
    let rel_path = Path::new(trimmed);
    for comp in rel_path.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err(format!("Unsafe relative path: {rel}")),
        }
    }
    let canon_base = base_dir
        .canonicalize()
        .map_err(|e| format!("Invalid theme directory: {e}"))?;
    let joined = canon_base.join(rel_path);
    let canon_target = joined
        .canonicalize()
        .map_err(|e| format!("Asset not found ({rel}): {e}"))?;
    if !canon_target.starts_with(&canon_base) {
        return Err(format!("Path escapes theme directory: {rel}"));
    }
    Ok(canon_target)
}

/// Prepare a user fragment shader for OpenGL 3.3 Core.
/// If the source already declares `#version`, it is compiled verbatim;
/// otherwise a standard `#version 330 core` header with DiyarDesk uniforms is prepended.
pub fn prepare_fragment_source(raw: &str) -> String {
    let trimmed = raw.trim_start_matches('\u{feff}').trim_start();
    if trimmed.starts_with("#version") {
        return raw.trim_start_matches('\u{feff}').to_string();
    }
    let mut out = String::with_capacity(raw.len() + 768);
    out.push_str(
        "#version 330 core\n\
         in vec2 v_uv;\n\
         out vec4 fragColor;\n\
         uniform vec2 u_res;\n\
         uniform vec2 u_res_px;\n\
         uniform float u_time;\n\
         uniform float u_dt;\n\
         uniform float u_seed;\n\
         uniform float u_solar;\n\
         uniform vec4 u_mouse;\n\
         uniform vec2 u_pointer;\n\
         uniform int u_adaptive;\n\
         uniform int u_icon_count;\n\
         uniform vec4 u_icons[64];\n\
         uniform float u_param_f[16];\n\
         uniform vec4 u_param_c[8];\n\
         uniform sampler2D u_tex0;\n\
         uniform sampler2D u_tex1;\n\
         uniform sampler2D u_tex2;\n\
         uniform sampler2D u_tex3;\n\n",
    );
    out.push_str(raw);
    out
}

pub fn parse_hex_color(s: &str) -> Option<[f32; 4]> {
    let hex = s.trim().strip_prefix('#')?;
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
        Some([r, g, b, 1.0])
    } else if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).ok()? as f32 / 15.0;
        let g = u8::from_str_radix(&hex[1..2], 16).ok()? as f32 / 15.0;
        let b = u8::from_str_radix(&hex[2..3], 16).ok()? as f32 / 15.0;
        Some([r, g, b, 1.0])
    } else if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
        let a = u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0;
        Some([r, g, b, a])
    } else {
        None
    }
}

const FALLBACK_ERROR_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;
uniform float u_time;
void main() {
    float wave = 0.5 + 0.5 * sin(v_uv.x * 6.0 + v_uv.y * 4.0 + u_time * 1.2);
    vec3 c0 = vec3(0.08, 0.09, 0.14);
    vec3 c1 = vec3(0.18, 0.11, 0.16);
    fragColor = vec4(mix(c0, c1, wave * 0.65), 1.0);
}
"#;

pub struct CustomGlslRenderer {
    vao: GLuint,
    prog: GLuint,
    fallback_prog: GLuint,
    textures: [GLuint; MAX_CUSTOM_TEXTURES],
    loaded_rev: u64,
    last_time: f32,
    uniform_cache: HashMap<String, GLint>,
    pub last_error: Option<String>,
}

impl CustomGlslRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let mut vao = 0;
        (gl.GenVertexArrays)(1, &mut vao);
        let fallback_prog = gl.compile_program(FULLSCREEN_VERT, FALLBACK_ERROR_FRAG)?;
        let mut textures = [0u32; MAX_CUSTOM_TEXTURES];
        (gl.GenTextures)(MAX_CUSTOM_TEXTURES as GLsizei, textures.as_mut_ptr());
        let blank: [u8; 4] = [0, 0, 0, 255];
        for &tex in &textures {
            (gl.BindTexture)(GL_TEXTURE_2D, tex);
            (gl.TexImage2D)(
                GL_TEXTURE_2D,
                0,
                GL_RGBA8 as GLint,
                1,
                1,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                blank.as_ptr() as *const c_void,
            );
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        }
        Ok(Self {
            vao,
            prog: 0,
            fallback_prog,
            textures,
            loaded_rev: 0,
            last_time: 0.0,
            uniform_cache: HashMap::new(),
            last_error: None,
        })
    }

    unsafe fn reload_if_needed(&mut self, gl: &GlFns, spec: &CustomGlSpec) {
        if self.loaded_rev == spec.rev && (self.prog != 0 || self.last_error.is_some()) {
            return;
        }
        self.loaded_rev = spec.rev;
        self.uniform_cache.clear();
        self.last_error = None;
        if self.prog != 0 {
            (gl.DeleteProgram)(self.prog);
            self.prog = 0;
        }

        let frag_path = match resolve_inside_dir(&spec.theme_dir, &spec.entry) {
            Ok(p) => p,
            Err(e) => {
                self.last_error = Some(e);
                return;
            }
        };
        let raw_frag = match std::fs::read_to_string(&frag_path) {
            Ok(s) => s,
            Err(e) => {
                self.last_error = Some(format!("Failed to read fragment shader: {e}"));
                return;
            }
        };
        let frag_src = prepare_fragment_source(&raw_frag);

        let vert_src = if let Some(ref ve) = spec.vert_entry {
            match resolve_inside_dir(&spec.theme_dir, ve).and_then(|p| {
                std::fs::read_to_string(p).map_err(|e| format!("Failed to read vertex shader: {e}"))
            }) {
                Ok(s) => s,
                Err(e) => {
                    self.last_error = Some(e);
                    return;
                }
            }
        } else {
            FULLSCREEN_VERT.to_string()
        };

        match gl.compile_program(&vert_src, &frag_src) {
            Ok(prog) => {
                self.prog = prog;
            }
            Err(e) => {
                self.last_error = Some(e);
                return;
            }
        }

        // Upload up to 4 optional textures
        for (slot, rel_tex) in spec.textures.iter().take(MAX_CUSTOM_TEXTURES).enumerate() {
            if let Ok(tp) = resolve_inside_dir(&spec.theme_dir, rel_tex) {
                if let Ok(img) = image::open(&tp) {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    if w > 0 && h > 0 {
                        (gl.BindTexture)(GL_TEXTURE_2D, self.textures[slot]);
                        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
                        (gl.TexImage2D)(
                            GL_TEXTURE_2D,
                            0,
                            GL_RGBA8 as GLint,
                            w as GLsizei,
                            h as GLsizei,
                            0,
                            GL_RGBA,
                            GL_UNSIGNED_BYTE,
                            rgba.as_raw().as_ptr() as *const c_void,
                        );
                        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
                        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
                        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
                        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
                    }
                }
            }
        }
    }

    unsafe fn cached_loc(&mut self, gl: &GlFns, prog: GLuint, name: &str) -> GLint {
        if let Some(&loc) = self.uniform_cache.get(name) {
            return loc;
        }
        let loc = gl.uniform_loc(prog, name);
        self.uniform_cache.insert(name.to_string(), loc);
        loc
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &AppState) {
        self.reload_if_needed(gl, &st.custom_gl);
        let prog = if self.prog != 0 { self.prog } else { self.fallback_prog };
        let is_fallback = self.prog == 0;

        let pw = st.width_px.max(16);
        let ph = st.height_px.max(16);
        let css_w = st.css_w().max(16.0);
        let css_h = st.css_h().max(16.0);
        let dt = (st.sim_time - self.last_time).clamp(0.0, 0.25);
        self.last_time = st.sim_time;

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_DEPTH_TEST);
        (gl.Disable)(GL_SCISSOR_TEST);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
        (gl.ClearColor)(0.04, 0.05, 0.08, 1.0);
        (gl.Clear)(GL_COLOR_BUFFER_BIT);

        (gl.UseProgram)(prog);

        if is_fallback {
            (gl.Uniform1f)(gl.uniform_loc(prog, "u_time"), st.sim_time);
            (gl.BindVertexArray)(self.vao);
            (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
            (gl.Disable)(GL_BLEND);
            return;
        }

        let loc_res = self.cached_loc(gl, prog, "u_res");
        if loc_res >= 0 { (gl.Uniform2f)(loc_res, css_w, css_h); }
        let loc_res_px = self.cached_loc(gl, prog, "u_res_px");
        if loc_res_px >= 0 { (gl.Uniform2f)(loc_res_px, pw as f32, ph as f32); }
        let loc_time = self.cached_loc(gl, prog, "u_time");
        if loc_time >= 0 { (gl.Uniform1f)(loc_time, st.sim_time); }
        let loc_dt = self.cached_loc(gl, prog, "u_dt");
        if loc_dt >= 0 { (gl.Uniform1f)(loc_dt, dt); }
        let loc_seed = self.cached_loc(gl, prog, "u_seed");
        if loc_seed >= 0 { (gl.Uniform1f)(loc_seed, st.params.seed as f32); }
        let loc_solar = self.cached_loc(gl, prog, "u_solar");
        if loc_solar >= 0 { (gl.Uniform1f)(loc_solar, (st.env_hour / 24.0).clamp(0.0, 1.0)); }
        let loc_mouse = self.cached_loc(gl, prog, "u_mouse");
        if loc_mouse >= 0 {
            let active = if st.pointer_active && !st.low_mode { 1.0 } else { 0.0 };
            (gl.Uniform4f)(loc_mouse, st.pointer_x, st.pointer_y, active, 0.0);
        }
        let loc_ptr = self.cached_loc(gl, prog, "u_pointer");
        if loc_ptr >= 0 { (gl.Uniform2f)(loc_ptr, st.pointer_x, st.pointer_y); }
        let loc_adapt = self.cached_loc(gl, prog, "u_adaptive");
        if loc_adapt >= 0 {
            let adapt = st.param_bool("adaptive", st.params.adaptive);
            (gl.Uniform1i)(loc_adapt, if adapt { 1 } else { 0 });
        }

        // Bind desktop icon bounding boxes (CSS px)
        let ic_cnt = st.icons.len().min(MAX_CUSTOM_ICONS);
        let loc_ic_cnt = self.cached_loc(gl, prog, "u_icon_count");
        if loc_ic_cnt >= 0 { (gl.Uniform1i)(loc_ic_cnt, ic_cnt as i32); }
        let loc_icons = self.cached_loc(gl, prog, "u_icons");
        if loc_icons >= 0 && ic_cnt > 0 {
            let mut flat = [0.0f32; MAX_CUSTOM_ICONS * 4];
            for (i, ic) in st.icons.iter().take(MAX_CUSTOM_ICONS).enumerate() {
                flat[i * 4] = ic.x;
                flat[i * 4 + 1] = ic.y;
                flat[i * 4 + 2] = ic.w;
                flat[i * 4 + 3] = ic.h;
            }
            (gl.Uniform4fv)(loc_icons, ic_cnt as GLsizei, flat.as_ptr());
        }

        // Bind indexed u_param_f[16] & u_param_c[8]
        let mut param_f = [0.0f32; 16];
        for (i, key) in st.custom_gl.param_order.iter().take(16).enumerate() {
            param_f[i] = st.param_f32(key, 0.0);
        }
        let loc_pf = self.cached_loc(gl, prog, "u_param_f");
        if loc_pf >= 0 {
            for (i, &val) in param_f.iter().enumerate() {
                let el_loc = self.cached_loc(gl, prog, &format!("u_param_f[{i}]"));
                if el_loc >= 0 {
                    (gl.Uniform1f)(el_loc, val);
                }
            }
        }
        let mut param_c = [[0.0f32; 4]; 8];
        for (i, key) in st.custom_gl.color_order.iter().take(8).enumerate() {
            if let Some(hex) = st.param_str(key) {
                if let Some(c) = parse_hex_color(hex) {
                    param_c[i] = c;
                }
            }
        }
        let loc_pc = self.cached_loc(gl, prog, "u_param_c");
        if loc_pc >= 0 {
            (gl.Uniform4fv)(loc_pc, 8, param_c.as_ptr() as *const f32);
        }

        // Bind named uniforms `u_<key>` for all raw_params!
        for (k, v) in &st.raw_params {
            let uname = format!("u_{k}");
            let loc = self.cached_loc(gl, prog, &uname);
            if loc < 0 {
                continue;
            }
            match v {
                Value::Number(n) => {
                    (gl.Uniform1f)(loc, n.as_f64().unwrap_or(0.0) as f32);
                }
                Value::Bool(b) => {
                    (gl.Uniform1f)(loc, if *b { 1.0 } else { 0.0 });
                }
                Value::String(s) => {
                    if let Some(c) = parse_hex_color(s) {
                        (gl.Uniform4f)(loc, c[0], c[1], c[2], c[3]);
                    }
                }
                _ => {}
            }
        }

        // Bind texture units 0..3
        for slot in 0..MAX_CUSTOM_TEXTURES {
            (gl.ActiveTexture)(GL_TEXTURE0 + slot as GLenum);
            (gl.BindTexture)(GL_TEXTURE_2D, self.textures[slot]);
            let tname = format!("u_tex{slot}");
            let loc = self.cached_loc(gl, prog, &tname);
            if loc >= 0 {
                (gl.Uniform1i)(loc, slot as GLint);
            }
        }

        (gl.BindVertexArray)(self.vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.Disable)(GL_BLEND);
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.prog != 0 {
            (gl.DeleteProgram)(self.prog);
            self.prog = 0;
        }
        if self.fallback_prog != 0 {
            (gl.DeleteProgram)(self.fallback_prog);
            self.fallback_prog = 0;
        }
        (gl.DeleteTextures)(MAX_CUSTOM_TEXTURES as GLsizei, self.textures.as_ptr());
        self.textures = [0; MAX_CUSTOM_TEXTURES];
        if self.vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.vao);
            self.vao = 0;
        }
        self.uniform_cache.clear();
    }
}
