use crate::gl::*;
use crate::shaders::{TEXT_FRAG, TEXT_VERT};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use std::ffi::c_void;

const ATLAS_W: i32 = 1024;
const ATLAS_H: i32 = 1024;

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
struct GlyphKey {
    ch: char,
    size_px: u16,
    bold: bool,
}

#[derive(Clone, Copy)]
struct GlyphEntry {
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    w: f32,
    h: f32,
    xmin: f32,
    ymin: f32,
    advance: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TextVert {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

pub struct TextRenderer {
    font_regular: Font,
    font_bold: Font,
    font_fallback: Option<Font>,
    atlas_tex: GLuint,
    prog: GLuint,
    vao: GLuint,
    vbo: GLuint,
    u_res: GLint,
    u_atlas: GLint,
    glyphs: HashMap<GlyphKey, GlyphEntry>,
    cursor_x: i32,
    cursor_y: i32,
    row_h: i32,
    verts: Vec<TextVert>,
}

fn load_system_font(candidates: &[&str]) -> Result<Font, String> {
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                return Ok(font);
            }
        }
    }
    Err("Could not load any Windows UI font from C:\\Windows\\Fonts".into())
}

impl TextRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        Self::new_with_fonts(
            gl,
            &[
                r"C:\Windows\Fonts\segoeui.ttf",
                r"C:\Windows\Fonts\arial.ttf",
            ],
            &[
                r"C:\Windows\Fonts\segoeuib.ttf",
                r"C:\Windows\Fonts\seguisb.ttf",
                r"C:\Windows\Fonts\arialbd.ttf",
                r"C:\Windows\Fonts\segoeui.ttf",
            ],
        )
    }

    pub unsafe fn new_monospace(gl: &GlFns) -> Result<Self, String> {
        Self::new_with_fonts(
            gl,
            &[
                r"C:\Windows\Fonts\consola.ttf",
                r"C:\Windows\Fonts\CascadiaMono.ttf",
                r"C:\Windows\Fonts\lucon.ttf",
                r"C:\Windows\Fonts\cour.ttf",
                r"C:\Windows\Fonts\segoeui.ttf",
            ],
            &[
                r"C:\Windows\Fonts\consolab.ttf",
                r"C:\Windows\Fonts\consola.ttf",
                r"C:\Windows\Fonts\courbd.ttf",
                r"C:\Windows\Fonts\segoeuib.ttf",
            ],
        )
    }

    unsafe fn new_with_fonts(
        gl: &GlFns,
        reg_candidates: &[&str],
        bold_candidates: &[&str],
    ) -> Result<Self, String> {
        let font_regular = load_system_font(reg_candidates)?;
        let font_bold = load_system_font(bold_candidates)?;
        let font_fallback = load_system_font(&[
            r"C:\Windows\Fonts\seguisym.ttf",
            r"C:\Windows\Fonts\DejaVuSansMono.ttf",
            r"C:\Windows\Fonts\segoeui.ttf",
        ])
        .ok();

        let prog = gl.compile_program(TEXT_VERT, TEXT_FRAG)?;
        let u_res = gl.uniform_loc(prog, "u_res");
        let u_atlas = gl.uniform_loc(prog, "u_atlas");

        let mut atlas_tex = 0;
        (gl.GenTextures)(1, &mut atlas_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, atlas_tex);
        (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
        let zeros = vec![0u8; (ATLAS_W * ATLAS_H) as usize];
        (gl.TexImage2D)(
            GL_TEXTURE_2D, 0, GL_R8 as GLint, ATLAS_W, ATLAS_H, 0,
            GL_RED, GL_UNSIGNED_BYTE, zeros.as_ptr() as *const c_void,
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        let mut vao = 0;
        let mut vbo = 0;
        (gl.GenVertexArrays)(1, &mut vao);
        (gl.GenBuffers)(1, &mut vbo);
        (gl.BindVertexArray)(vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, vbo);
        let stride = std::mem::size_of::<TextVert>() as GLsizei;
        (gl.EnableVertexAttribArray)(0);
        (gl.VertexAttribPointer)(0, 2, GL_FLOAT, GL_FALSE, stride, 0 as *const c_void);
        (gl.EnableVertexAttribArray)(1);
        (gl.VertexAttribPointer)(1, 2, GL_FLOAT, GL_FALSE, stride, 8 as *const c_void);
        (gl.EnableVertexAttribArray)(2);
        (gl.VertexAttribPointer)(2, 4, GL_FLOAT, GL_FALSE, stride, 16 as *const c_void);
        (gl.BindVertexArray)(0);

        Ok(Self {
            font_regular,
            font_bold,
            font_fallback,
            atlas_tex,
            prog,
            vao,
            vbo,
            u_res,
            u_atlas,
            glyphs: HashMap::new(),
            cursor_x: 1,
            cursor_y: 1,
            row_h: 0,
            verts: Vec::with_capacity(4096),
        })
    }

    pub fn begin(&mut self) {
        self.verts.clear();
    }

    fn get_or_rasterize(&mut self, gl: &GlFns, ch: char, size_px: u16, bold: bool) -> GlyphEntry {
        let key = GlyphKey { ch, size_px, bold };
        if let Some(&e) = self.glyphs.get(&key) {
            return e;
        }
        let primary = if bold { &self.font_bold } else { &self.font_regular };
        let font = if primary.lookup_glyph_index(ch) != 0 {
            primary
        } else if let Some(fb) = self.font_fallback.as_ref() {
            if fb.lookup_glyph_index(ch) != 0 {
                fb
            } else {
                primary
            }
        } else {
            primary
        };
        let (metrics, bitmap) = font.rasterize(ch, size_px as f32);
        let gw = metrics.width as i32;
        let gh = metrics.height as i32;

        if gw > 0 && gh > 0 {
            if self.cursor_x + gw + 2 >= ATLAS_W {
                self.cursor_x = 1;
                self.cursor_y += self.row_h + 2;
                self.row_h = 0;
            }
            if self.cursor_y + gh + 2 >= ATLAS_H {
                // Atlas full: reset cleanly
                self.glyphs.clear();
                self.cursor_x = 1;
                self.cursor_y = 1;
                self.row_h = 0;
                unsafe {
                    (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
                    let zeros = vec![0u8; (ATLAS_W * ATLAS_H) as usize];
                    (gl.TexSubImage2D)(
                        GL_TEXTURE_2D, 0, 0, 0, ATLAS_W, ATLAS_H,
                        GL_RED, GL_UNSIGNED_BYTE, zeros.as_ptr() as *const c_void,
                    );
                }
            }
            let gx = self.cursor_x;
            let gy = self.cursor_y;
            self.cursor_x += gw + 2;
            self.row_h = self.row_h.max(gh);

            unsafe {
                (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
                (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 1);
                (gl.TexSubImage2D)(
                    GL_TEXTURE_2D, 0, gx, gy, gw, gh,
                    GL_RED, GL_UNSIGNED_BYTE, bitmap.as_ptr() as *const c_void,
                );
            }

            let entry = GlyphEntry {
                u0: gx as f32 / ATLAS_W as f32,
                v0: gy as f32 / ATLAS_H as f32,
                u1: (gx + gw) as f32 / ATLAS_W as f32,
                v1: (gy + gh) as f32 / ATLAS_H as f32,
                w: gw as f32,
                h: gh as f32,
                xmin: metrics.xmin as f32,
                ymin: metrics.ymin as f32,
                advance: metrics.advance_width,
            };
            self.glyphs.insert(key, entry);
            entry
        } else {
            let entry = GlyphEntry {
                u0: 0.0, v0: 0.0, u1: 0.0, v1: 0.0,
                w: 0.0, h: 0.0, xmin: 0.0, ymin: 0.0,
                advance: metrics.advance_width,
            };
            self.glyphs.insert(key, entry);
            entry
        }
    }

    pub fn measure(&mut self, gl: &GlFns, text: &str, css_size: f32, dpi: f32, bold: bool) -> f32 {
        let size_px = (css_size * dpi).round().clamp(8.0, 160.0) as u16;
        let inv = 1.0 / dpi.max(0.5);
        let mut w = 0.0;
        for ch in text.chars() {
            let g = self.get_or_rasterize(gl, ch, size_px, bold);
            w += g.advance * inv;
        }
        w
    }

    // align: 0 = left, 1 = center, 2 = right; y is baseline in CSS px
    pub fn push(
        &mut self,
        gl: &GlFns,
        text: &str,
        x: f32,
        baseline_y: f32,
        css_size: f32,
        dpi: f32,
        bold: bool,
        col: [f32; 4],
        align: i32,
        max_width: Option<f32>,
    ) {
        let mut display = text.to_string();
        if let Some(mw) = max_width {
            if self.measure(gl, &display, css_size, dpi, bold) > mw && display.chars().count() > 2 {
                let mut chars: Vec<char> = display.chars().collect();
                while chars.len() > 1 {
                    chars.pop();
                    let cand: String = chars.iter().collect::<String>() + "...";
                    if self.measure(gl, &cand, css_size, dpi, bold) <= mw {
                        display = cand;
                        break;
                    }
                }
            }
        }

        let total_w = if align != 0 {
            self.measure(gl, &display, css_size, dpi, bold)
        } else {
            0.0
        };
        let mut pen_x = match align {
            1 => x - total_w * 0.5,
            2 => x - total_w,
            _ => x,
        };

        let size_px = (css_size * dpi).round().clamp(8.0, 160.0) as u16;
        let inv = 1.0 / dpi.max(0.5);

        for ch in display.chars() {
            let g = self.get_or_rasterize(gl, ch, size_px, bold);
            if g.w > 0.0 && g.h > 0.0 {
                let x0 = pen_x + g.xmin * inv;
                let y1 = baseline_y - g.ymin * inv;
                let y0 = y1 - g.h * inv;
                let x1 = x0 + g.w * inv;
                let v = |px, py, u, v| TextVert {
                    x: px, y: py, u, v,
                    r: col[0], g: col[1], b: col[2], a: col[3],
                };
                self.verts.push(v(x0, y0, g.u0, g.v0));
                self.verts.push(v(x1, y0, g.u1, g.v0));
                self.verts.push(v(x1, y1, g.u1, g.v1));
                self.verts.push(v(x0, y0, g.u0, g.v0));
                self.verts.push(v(x1, y1, g.u1, g.v1));
                self.verts.push(v(x0, y1, g.u0, g.v1));
            }
            pen_x += g.advance * inv;
        }
    }

    pub unsafe fn flush(&mut self, gl: &GlFns, css_w: f32, css_h: f32) {
        if self.verts.is_empty() {
            return;
        }
        (gl.UseProgram)(self.prog);
        (gl.Uniform2f)(self.u_res, css_w, css_h);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.atlas_tex);
        (gl.Uniform1i)(self.u_atlas, 0);

        (gl.BindVertexArray)(self.vao);
        (gl.BindBuffer)(GL_ARRAY_BUFFER, self.vbo);
        let bytes = (self.verts.len() * std::mem::size_of::<TextVert>()) as GLsizeiptr;
        (gl.BufferData)(GL_ARRAY_BUFFER, bytes, self.verts.as_ptr() as *const c_void, GL_DYNAMIC_DRAW);
        (gl.DrawArrays)(GL_TRIANGLES, 0, self.verts.len() as GLsizei);
        (gl.BindVertexArray)(0);
        self.verts.clear();
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.atlas_tex != 0 { (gl.DeleteTextures)(1, &self.atlas_tex); self.atlas_tex = 0; }
        if self.vbo != 0 { (gl.DeleteBuffers)(1, &self.vbo); self.vbo = 0; }
        if self.vao != 0 { (gl.DeleteVertexArrays)(1, &self.vao); self.vao = 0; }
        if self.prog != 0 { (gl.DeleteProgram)(self.prog); self.prog = 0; }
    }
}
