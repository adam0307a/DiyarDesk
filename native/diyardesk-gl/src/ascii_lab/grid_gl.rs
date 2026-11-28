//! Shared 96-glyph GPU ASCII cell-grid renderer for `ascii-lab` themes.
//!
//! Renders any CPU-evaluated `(cols × rows)` colored ASCII canvas cleanly at
//! 30–60 FPS on OpenGL 3.3 Core AND OpenGL ES 3.0 minimal-caps GPUs using
//! strictly `GL_RGBA8` textures (zero floating-point FBOs or extensions):
//!   - `tex_atlas`: 96 printable ASCII glyphs (32..=127), rasterized once via
//!     `fontdue`, with strictly 1 px zero-alpha padding on all 4 edges.
//!   - `tex_cells`: `cols × rows` RGBA8 (`RGB` = ink color, `A` = glyph id 0..95).
//!   - `tex_bg`:    `cols × rows` RGBA8 (`RGB` = paper/bg color, `A` = bloom weight).

use crate::gl::*;
use crate::shaders::FULLSCREEN_VERT;
use crate::state::Rect;
use crate::text;
use std::ffi::c_void;

pub const GLYPH_COUNT: usize = 96;
pub const TILE_W: usize = 18; // 1 px padding left/right + 16 px glyph box
pub const TILE_H: usize = 28; // 1 px padding top/bottom + 26 px glyph box

#[inline]
pub fn glyph_id(ch: char) -> u8 {
    let b = ch as u32;
    if (32..=126).contains(&b) {
        (b - 32) as u8
    } else {
        0
    }
}

#[inline]
pub fn ramp_glyph(ramp: &[u8], val: f32) -> u8 {
    if ramp.is_empty() {
        return 0;
    }
    let idx = (val.clamp(0.0, 1.0) * (ramp.len() - 1) as f32).round() as usize;
    let ch = ramp[idx.min(ramp.len() - 1)];
    ch.saturating_sub(32).min(95)
}

#[inline]
pub fn line_glyph_cell(dx_px: f32, dy_px: f32, cw: f32, ch: f32) -> u8 {
    let dx = dx_px / cw.max(1.0);
    let dy = dy_px / ch.max(1.0);
    if dy.abs() > 2.0 * dx.abs() {
        glyph_id('|')
    } else if dx.abs() > 2.0 * dy.abs() {
        glyph_id('-')
    } else if dx * dy < 0.0 {
        glyph_id('/')
    } else {
        glyph_id('\\')
    }
}

#[inline]
pub fn line_glyph_underscore(dx_px: f32, dy_px: f32, cw: f32, ch: f32) -> u8 {
    let dx = dx_px / cw.max(1.0);
    let dy = dy_px / ch.max(1.0);
    if dy.abs() > 2.0 * dx.abs() {
        glyph_id('|')
    } else if dx.abs() > 2.0 * dy.abs() {
        glyph_id('_')
    } else if dx * dy < 0.0 {
        glyph_id('/')
    } else {
        glyph_id('\\')
    }
}

pub struct AsciiCanvas {
    pub cols: usize,
    pub rows: usize,
    /// `cols * rows * 4` RGBA8: `(r, g, b, glyph_id)`
    pub cells: Vec<u8>,
    /// `cols * rows * 4` RGBA8: `(bg_r, bg_g, bg_b, bloom_weight)`
    pub bg: Vec<u8>,
    /// Depth buffer (smaller z = nearer by default, or user convention)
    pub zbuf: Vec<f32>,
    /// Ownership tag per cell (used by outline passes)
    pub owner: Vec<u8>,
    /// Monotonic revision bumped whenever canvas dimensions or contents change
    pub rev: u64,
    last_w: f32,
    last_h: f32,
    last_char_size: f32,
    last_low: bool,
}

impl Default for AsciiCanvas {
    fn default() -> Self {
        let mut c = Self {
            cols: 0,
            rows: 0,
            cells: Vec::new(),
            bg: Vec::new(),
            zbuf: Vec::new(),
            owner: Vec::new(),
            rev: 1,
            last_w: 0.0,
            last_h: 0.0,
            last_char_size: 7.5,
            last_low: false,
        };
        c.ensure_layout(1280.0, 720.0, 7.5, false);
        c
    }
}

impl AsciiCanvas {
    pub fn ensure_layout(&mut self, css_w: f32, css_h: f32, char_size: f32, low_mode: bool) -> bool {
        let w = css_w.max(120.0);
        let h = css_h.max(90.0);
        let cs = char_size.clamp(4.2, 14.0);
        if self.cols > 0
            && (w - self.last_w).abs() <= 0.5
            && (h - self.last_h).abs() <= 0.5
            && (cs - self.last_char_size).abs() <= 0.05
            && low_mode == self.last_low
        {
            return false;
        }
        self.last_w = w;
        self.last_h = h;
        self.last_char_size = cs;
        self.last_low = low_mode;

        let scale = if low_mode { 1.32 } else { 1.0 };
        let eff_cw = (cs * scale).max(4.2);
        let eff_ch = eff_cw * 1.56;
        // Target crisp widescreen/portrait ASCII density up to 224x136
        let cols = ((w / eff_cw).round() as usize).clamp(58, if low_mode { 136 } else { 224 });
        let rows = ((h / eff_ch).round() as usize).clamp(38, if low_mode { 82 } else { 136 });
        if cols != self.cols || rows != self.rows {
            self.cols = cols;
            self.rows = rows;
            let len = cols * rows;
            self.cells.resize(len * 4, 0);
            self.bg.resize(len * 4, 0);
            self.zbuf.resize(len, 9999.0);
            self.owner.resize(len, 0);
            self.rev = self.rev.wrapping_add(1);
            return true;
        }
        false
    }

    /// Forces next `ensure_layout*` call to recompute grid dimensions cleanly.
    pub fn invalidate_layout(&mut self) {
        self.last_w = 0.0;
    }

    /// Custom high-density grid layout for high-detail photographic ASCII scenes.
    pub fn ensure_layout_custom(
        &mut self,
        css_w: f32,
        css_h: f32,
        char_size: f32,
        low_mode: bool,
        max_cols: usize,
        max_rows: usize,
        cell_aspect: f32,
    ) -> bool {
        let w = css_w.max(120.0);
        let h = css_h.max(90.0);
        let cs = char_size.clamp(3.2, 14.0);
        if self.cols > 0
            && (w - self.last_w).abs() <= 0.5
            && (h - self.last_h).abs() <= 0.5
            && (cs - self.last_char_size).abs() <= 0.05
            && low_mode == self.last_low
        {
            return false;
        }
        self.last_w = w;
        self.last_h = h;
        self.last_char_size = cs;
        self.last_low = low_mode;

        let scale = if low_mode { 1.48 } else { 1.0 };
        let eff_cw = (cs * scale).max(3.2);
        let eff_ch = eff_cw * cell_aspect.clamp(1.2, 1.8);
        let hi_c = if low_mode { (max_cols * 3) / 5 } else { max_cols }.max(58);
        let hi_r = if low_mode { (max_rows * 3) / 5 } else { max_rows }.max(38);
        let cols = ((w / eff_cw).round() as usize).clamp(58, hi_c);
        let rows = ((h / eff_ch).round() as usize).clamp(38, hi_r);
        if cols != self.cols || rows != self.rows {
            self.cols = cols;
            self.rows = rows;
            let len = cols * rows;
            self.cells.resize(len * 4, 0);
            self.bg.resize(len * 4, 0);
            self.zbuf.resize(len, 9999.0);
            self.owner.resize(len, 0);
            self.rev = self.rev.wrapping_add(1);
            return true;
        }
        false
    }

    pub fn clear(&mut self, bg_rgb: [u8; 3], default_z: f32) {
        for px in self.cells.chunks_exact_mut(4) {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            px[3] = 0;
        }
        for px in self.bg.chunks_exact_mut(4) {
            px[0] = bg_rgb[0];
            px[1] = bg_rgb[1];
            px[2] = bg_rgb[2];
            px[3] = 0;
        }
        self.zbuf.fill(default_z);
        self.owner.fill(0);
    }

    #[inline]
    pub fn idx(&self, c: i32, r: i32) -> Option<usize> {
        if c >= 0 && r >= 0 && (c as usize) < self.cols && (r as usize) < self.rows {
            Some((r as usize) * self.cols + (c as usize))
        } else {
            None
        }
    }

    #[inline]
    pub fn set_cell(&mut self, c: i32, r: i32, glyph: u8, rgb: [u8; 3], bloom: u8, own: u8) {
        if let Some(i) = self.idx(c, r) {
            let p = i * 4;
            self.cells[p] = rgb[0];
            self.cells[p + 1] = rgb[1];
            self.cells[p + 2] = rgb[2];
            self.cells[p + 3] = glyph.min(95);
            self.bg[p + 3] = bloom;
            self.owner[i] = own;
        }
    }

    #[inline]
    pub fn plot_z_less(&mut self, c: i32, r: i32, z: f32, glyph: u8, rgb: [u8; 3], bloom: u8, own: u8) {
        if let Some(i) = self.idx(c, r) {
            if z < self.zbuf[i] {
                self.zbuf[i] = z;
                let p = i * 4;
                self.cells[p] = rgb[0];
                self.cells[p + 1] = rgb[1];
                self.cells[p + 2] = rgb[2];
                self.cells[p + 3] = glyph.min(95);
                self.bg[p + 3] = bloom;
                self.owner[i] = own;
            }
        }
    }

    #[inline]
    pub fn set_bg_rgb(&mut self, c: i32, r: i32, rgb: [u8; 3]) {
        if let Some(i) = self.idx(c, r) {
            let p = i * 4;
            self.bg[p] = rgb[0];
            self.bg[p + 1] = rgb[1];
            self.bg[p + 2] = rgb[2];
        }
    }

    #[inline]
    pub fn mark_dirty(&mut self) {
        self.rev = self.rev.wrapping_add(1);
    }
}

pub fn build_ascii_96_atlas() -> Vec<u8> {
    let font_opt = text::load_mono_font().ok();
    let font_px = 21.0f32;
    let atlas_w = GLYPH_COUNT * TILE_W;
    let atlas_h = TILE_H;
    let mut rgba = vec![0u8; atlas_w * atlas_h * 4];

    for gi in 0..GLYPH_COUNT {
        let ch = if gi == 95 { ' ' } else { (32 + gi as u8) as char };
        let x_off = gi * TILE_W;
        if ch == ' ' {
            continue;
        }
        match &font_opt {
            Some(font) => {
                let (metrics, bitmap) = font.rasterize(ch, font_px);
                let gw = metrics.width as i32;
                let gh = metrics.height as i32;
                let baseline_y = if ch == '_' { 22i32 } else { 20i32 };
                let left_x = ((TILE_W as i32 - gw) / 2).max(1);
                let top_y = baseline_y - gh - metrics.ymin;
                for gy in 0..gh {
                    let dy = top_y + gy;
                    if dy < 1 || dy >= (TILE_H as i32 - 1) {
                        continue;
                    }
                    for gx in 0..gw {
                        let dx = left_x + gx;
                        if dx < 1 || dx >= (TILE_W as i32 - 1) {
                            continue;
                        }
                        let raw = bitmap[(gy as usize) * (gw as usize) + (gx as usize)];
                        let boosted = ((raw as f32 / 255.0).powf(0.84) * 255.0)
                            .round()
                            .clamp(0.0, 255.0) as u8;
                        let dst = (dy as usize * atlas_w + x_off + dx as usize) * 4;
                        rgba[dst] = boosted;
                        rgba[dst + 1] = boosted;
                        rgba[dst + 2] = boosted;
                        rgba[dst + 3] = boosted;
                    }
                }
            }
            None => {
                // Crisp fallback glyph shapes when running without system fonts
                let cx = (TILE_W / 2) as i32;
                let cy = (TILE_H / 2) as i32;
                let mut draw_px = |px: i32, py: i32, v: u8| {
                    if px >= 2 && px < (TILE_W as i32 - 2) && py >= 2 && py < (TILE_H as i32 - 2) {
                        let dst = (py as usize * atlas_w + x_off + px as usize) * 4;
                        rgba[dst] = v;
                        rgba[dst + 1] = v;
                        rgba[dst + 2] = v;
                        rgba[dst + 3] = v;
                    }
                };
                match ch {
                    '|' => {
                        for y in 4..24 {
                            draw_px(cx, y, 255);
                            draw_px(cx + 1, y, 210);
                        }
                    }
                    '-' => {
                        for x in 3..15 {
                            draw_px(x, cy, 255);
                            draw_px(x, cy + 1, 200);
                        }
                    }
                    '_' => {
                        for x in 2..16 {
                            draw_px(x, 22, 255);
                            draw_px(x, 23, 210);
                        }
                    }
                    '/' => {
                        for k in 0..18 {
                            draw_px(13 - (k * 9) / 18, 5 + k, 255);
                        }
                    }
                    '\\' => {
                        for k in 0..18 {
                            draw_px(4 + (k * 9) / 18, 5 + k, 255);
                        }
                    }
                    '.' | ',' | '\'' | ':' | ';' => {
                        draw_px(cx, 19, 220);
                        draw_px(cx + 1, 19, 220);
                        if ch == ':' || ch == ';' || ch == '\'' {
                            draw_px(cx, 10, 220);
                            draw_px(cx + 1, 10, 220);
                        }
                    }
                    _ => {
                        let frac = ((gi % 32) as f32 / 31.0).clamp(0.18, 1.0);
                        let hw = (2.0 + frac * 4.5) as i32;
                        let hh = (3.0 + frac * 7.5) as i32;
                        for y in (cy - hh)..=(cy + hh) {
                            for x in (cx - hw)..=(cx + hw) {
                                let v = (frac * 245.0) as u8;
                                draw_px(x, y, v);
                            }
                        }
                    }
                }
            }
        }
    }
    rgba
}

pub const ASCII_GRID_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_res;
uniform vec2 u_grid;         // (cols, rows)
uniform float u_glow;        // bloom multiplier (0.0 .. 1.5)
uniform float u_crt;         // subtle CRT/paper lift
uniform int u_adaptive;
uniform int u_icon_count;
uniform vec4 u_icons[24];    // physical px rects (x, y, w, h)

uniform sampler2D u_atlas;   // 96-glyph strip (18 * 96 x 28)
uniform sampler2D u_cells;   // (cols x rows): rgb = ink, a = glyph_id / 255
uniform sampler2D u_bg;      // (cols x rows): rgb = paper bg, a = bloom weight

const float GLYPH_COUNT = 96.0;
const float TILE_W = 18.0;
const float TILE_H = 28.0;

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    vec2 gcoord = vec2(v_uv.x, 1.0 - v_uv.y) * u_grid;
    vec2 cell_id = floor(gcoord);
    vec2 cell_uv = fract(gcoord);
    vec2 cell_uv_tex = (cell_id + 0.5) / u_grid;

    vec4 cell_px = texture(u_cells, cell_uv_tex);
    vec4 bg_px = texture(u_bg, vec2(v_uv.x, 1.0 - v_uv.y));

    float glyph_idx = clamp(floor(cell_px.a * 255.0 + 0.5), 0.0, GLYPH_COUNT - 1.0);
    float cov = 0.0;
    if (glyph_idx > 0.5) {
        float u_tile = (glyph_idx * TILE_W + 1.0 + cell_uv.x * (TILE_W - 2.0)) / (GLYPH_COUNT * TILE_W);
        float v_tile = (1.0 + cell_uv.y * (TILE_H - 2.0)) / TILE_H;
        cov = texture(u_atlas, vec2(u_tile, v_tile)).r;
    }

    vec3 out_rgb = mix(bg_px.rgb, cell_px.rgb, cov);

    // Multi-tap phosphor glow matching ASCII Lab paint() near+far bloom
    if (u_glow > 0.01) {
        vec2 step_uv = 1.0 / u_grid;
        vec3 near_sum = vec3(0.0);
        near_sum += texture(u_cells, cell_uv_tex + vec2( step_uv.x, 0.0)).rgb * step(0.5 / 255.0, texture(u_cells, cell_uv_tex + vec2( step_uv.x, 0.0)).a);
        near_sum += texture(u_cells, cell_uv_tex + vec2(-step_uv.x, 0.0)).rgb * step(0.5 / 255.0, texture(u_cells, cell_uv_tex + vec2(-step_uv.x, 0.0)).a);
        near_sum += texture(u_cells, cell_uv_tex + vec2(0.0,  step_uv.y)).rgb * step(0.5 / 255.0, texture(u_cells, cell_uv_tex + vec2(0.0,  step_uv.y)).a);
        near_sum += texture(u_cells, cell_uv_tex + vec2(0.0, -step_uv.y)).rgb * step(0.5 / 255.0, texture(u_cells, cell_uv_tex + vec2(0.0, -step_uv.y)).a);

        vec2 step2 = step_uv * 2.1;
        vec3 far_sum = vec3(0.0);
        far_sum += texture(u_cells, cell_uv_tex + vec2( step2.x,  step2.y)).rgb;
        far_sum += texture(u_cells, cell_uv_tex + vec2(-step2.x,  step2.y)).rgb;
        far_sum += texture(u_cells, cell_uv_tex + vec2( step2.x, -step2.y)).rgb;
        far_sum += texture(u_cells, cell_uv_tex + vec2(-step2.x, -step2.y)).rgb;

        float bloom_w = 0.25 + 0.75 * bg_px.a;
        vec3 glow_col = near_sum * 0.075 + far_sum * 0.036;
        out_rgb += glow_col * bloom_w * u_glow;
    }

    // Subtle scanline/grid micro-contrast
    if (u_crt > 0.001) {
        float scan = 0.96 + 0.04 * sin(cell_uv.y * 3.14159265);
        out_rgb *= mix(1.0, scan, u_crt);
    }

    // Adaptive soft rounded scrim behind desktop icons
    if (u_adaptive == 1) {
        for (int i = 0; i < 24; i++) {
            if (i >= u_icon_count) { break; }
            vec4 ir = u_icons[i];
            vec2 half_ext = ir.zw * 0.5 + vec2(10.0);
            vec2 center = ir.xy + ir.zw * 0.5;
            float d = sdRoundBox(px - center, half_ext, 10.0);
            float scrim = 1.0 - smoothstep(-2.0, 5.0, d);
            out_rgb *= (1.0 - scrim * 0.48);
        }
    }

    // Vignette at far corners
    vec2 vig_p = v_uv - 0.5;
    float vig = 1.0 - dot(vig_p, vig_p) * 0.26;
    out_rgb *= clamp(vig, 0.72, 1.0);

    fragColor = vec4(clamp(out_rgb, 0.0, 1.0), 1.0);
}
"#;

pub struct AsciiGridRenderer {
    prog: GLuint,
    vao: GLuint,
    atlas_tex: GLuint,
    cells_tex: GLuint,
    bg_tex: GLuint,
    tex_cols: usize,
    tex_rows: usize,
    uploaded_rev: u64,
    uploaded_tag: u8,
    u_res: GLint,
    u_grid: GLint,
    u_glow: GLint,
    u_crt: GLint,
    u_adaptive: GLint,
    u_icon_count: GLint,
    u_icons: GLint,
    u_atlas: GLint,
    u_cells: GLint,
    u_bg: GLint,
}

unsafe fn make_texture(gl: &GlFns, filter: GLint) -> GLuint {
    let mut tex = 0;
    (gl.GenTextures)(1, &mut tex);
    (gl.BindTexture)(GL_TEXTURE_2D, tex);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, filter);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, filter);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as GLint);
    (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as GLint);
    tex
}

impl AsciiGridRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let prog = gl.compile_program(FULLSCREEN_VERT, ASCII_GRID_FRAG)?;
        let mut vao = 0;
        (gl.GenVertexArrays)(1, &mut vao);

        let atlas_rgba = build_ascii_96_atlas();
        let atlas_tex = make_texture(gl, GL_LINEAR as GLint);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        (gl.TexImage2D)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as GLint,
            (GLYPH_COUNT * TILE_W) as GLsizei,
            TILE_H as GLsizei,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            atlas_rgba.as_ptr() as *const c_void,
        );
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
        (gl.BindTexture)(GL_TEXTURE_2D, 0);

        let cells_tex = make_texture(gl, GL_NEAREST as GLint);
        let bg_tex = make_texture(gl, GL_LINEAR as GLint);
        (gl.BindTexture)(GL_TEXTURE_2D, 0);

        let u_res = gl.uniform_loc(prog, "u_res");
        let u_grid = gl.uniform_loc(prog, "u_grid");
        let u_glow = gl.uniform_loc(prog, "u_glow");
        let u_crt = gl.uniform_loc(prog, "u_crt");
        let u_adaptive = gl.uniform_loc(prog, "u_adaptive");
        let u_icon_count = gl.uniform_loc(prog, "u_icon_count");
        let u_icons = gl.uniform_loc(prog, "u_icons");
        let u_atlas = gl.uniform_loc(prog, "u_atlas");
        let u_cells = gl.uniform_loc(prog, "u_cells");
        let u_bg = gl.uniform_loc(prog, "u_bg");

        Ok(Self {
            prog,
            vao,
            atlas_tex,
            cells_tex,
            bg_tex,
            tex_cols: 0,
            tex_rows: 0,
            uploaded_rev: 0,
            uploaded_tag: 0,
            u_res,
            u_grid,
            u_glow,
            u_crt,
            u_adaptive,
            u_icon_count,
            u_icons,
            u_atlas,
            u_cells,
            u_bg,
        })
    }

    pub unsafe fn render(
        &mut self,
        gl: &GlFns,
        canvas: &AsciiCanvas,
        scene_tag: u8,
        pw: i32,
        ph: i32,
        dpi: f32,
        glow: f32,
        crt: f32,
        adaptive: bool,
        icons_vis: bool,
        icons: &[Rect],
    ) {
        if canvas.cols == 0 || canvas.rows == 0 {
            return;
        }
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);

        // Upload grid textures whenever theme tag, grid size, or revision changed
        if self.uploaded_tag != scene_tag
            || self.uploaded_rev != canvas.rev
            || self.tex_cols != canvas.cols
            || self.tex_rows != canvas.rows
        {
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
            let realloc = self.tex_cols != canvas.cols || self.tex_rows != canvas.rows;
            (gl.BindTexture)(GL_TEXTURE_2D, self.cells_tex);
            if realloc {
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_RGBA8 as GLint,
                    canvas.cols as GLsizei,
                    canvas.rows as GLsizei,
                    0,
                    GL_RGBA,
                    GL_UNSIGNED_BYTE,
                    canvas.cells.as_ptr() as *const c_void,
                );
            } else {
                (gl.TexSubImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    0,
                    0,
                    canvas.cols as GLsizei,
                    canvas.rows as GLsizei,
                    GL_RGBA,
                    GL_UNSIGNED_BYTE,
                    canvas.cells.as_ptr() as *const c_void,
                );
            }

            (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
            if realloc {
                (gl.TexImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    GL_RGBA8 as GLint,
                    canvas.cols as GLsizei,
                    canvas.rows as GLsizei,
                    0,
                    GL_RGBA,
                    GL_UNSIGNED_BYTE,
                    canvas.bg.as_ptr() as *const c_void,
                );
            } else {
                (gl.TexSubImage2D)(
                    GL_TEXTURE_2D,
                    0,
                    0,
                    0,
                    canvas.cols as GLsizei,
                    canvas.rows as GLsizei,
                    GL_RGBA,
                    GL_UNSIGNED_BYTE,
                    canvas.bg.as_ptr() as *const c_void,
                );
            }
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
            self.tex_cols = canvas.cols;
            self.tex_rows = canvas.rows;
            self.uploaded_rev = canvas.rev;
            self.uploaded_tag = scene_tag;
        }

        (gl.UseProgram)(self.prog);
        (gl.Uniform2f)(self.u_res, pw as f32, ph as f32);
        (gl.Uniform2f)(self.u_grid, canvas.cols as f32, canvas.rows as f32);
        (gl.Uniform1f)(self.u_glow, glow);
        (gl.Uniform1f)(self.u_crt, crt);
        (gl.Uniform1i)(self.u_adaptive, if adaptive && icons_vis { 1 } else { 0 });

        let mut packed = [0.0f32; 24 * 4];
        let count = if icons_vis { icons.len().min(24) } else { 0 };
        for (i, r) in icons.iter().take(count).enumerate() {
            packed[i * 4] = r.x * dpi;
            packed[i * 4 + 1] = r.y * dpi;
            packed[i * 4 + 2] = r.w * dpi;
            packed[i * 4 + 3] = r.h * dpi;
        }
        (gl.Uniform1i)(self.u_icon_count, count as GLint);
        (gl.Uniform4fv)(self.u_icons, 24, packed.as_ptr());

        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
        (gl.Uniform1i)(self.u_atlas, 0);

        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.cells_tex);
        (gl.Uniform1i)(self.u_cells, 1);

        (gl.ActiveTexture)(GL_TEXTURE2);
        (gl.BindTexture)(GL_TEXTURE_2D, self.bg_tex);
        (gl.Uniform1i)(self.u_bg, 2);

        (gl.BindVertexArray)(self.vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
        (gl.BindVertexArray)(0);
        (gl.ActiveTexture)(GL_TEXTURE2);
        (gl.BindTexture)(GL_TEXTURE_2D, 0);
        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, 0);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, 0);
        (gl.UseProgram)(0);
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        for tex in [&mut self.atlas_tex, &mut self.cells_tex, &mut self.bg_tex] {
            if *tex != 0 {
                (gl.DeleteTextures)(1, tex);
                *tex = 0;
            }
        }
        if self.vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.vao);
            self.vao = 0;
        }
        if self.prog != 0 {
            (gl.DeleteProgram)(self.prog);
            self.prog = 0;
        }
    }
}
