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

fn load_font_candidates(candidates: &[&str]) -> Option<Font> {
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                return Some(font);
            }
        }
    }
    None
}

fn load_system_font(candidates: &[&str]) -> Result<Font, String> {
    if let Some(font) = load_font_candidates(candidates) {
        return Ok(font);
    }
    #[cfg(target_os = "android")]
    {
        if let Some(font) = scan_android_system_font(false) {
            return Ok(font);
        }
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "android"), not(target_os = "macos")))]
    {
        for pattern in ["sans-serif", "DejaVu Sans", "Liberation Sans", "Noto Sans"] {
            if let Some(path) = fc_match_font(pattern) {
                if let Ok(bytes) = std::fs::read(&path) {
                    if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                        return Ok(font);
                    }
                }
            }
        }
    }
    Err("Could not load any system UI font".into())
}

#[cfg(any(target_os = "android", test))]
fn scan_font_dirs(dirs: &[&str], prefer_mono: bool) -> Option<Font> {
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                let is_font = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| {
                        ext.eq_ignore_ascii_case("ttf")
                            || ext.eq_ignore_ascii_case("otf")
                            || ext.eq_ignore_ascii_case("ttc")
                    })
                    .unwrap_or(false);
                if is_font {
                    paths.push(p);
                }
            }
        }
    }
    paths.sort();
    if prefer_mono {
        for p in &paths {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.to_ascii_lowercase().contains("mono") {
                if let Ok(bytes) = std::fs::read(p) {
                    if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                        return Some(font);
                    }
                }
            }
        }
        return None;
    }
    for p in &paths {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let lower = name.to_ascii_lowercase();
        if lower.contains("roboto") || lower.contains("droidsans") || lower.contains("notosans") {
            if let Ok(bytes) = std::fs::read(p) {
                if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                    return Some(font);
                }
            }
        }
    }
    for p in &paths {
        if let Ok(bytes) = std::fs::read(p) {
            if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                return Some(font);
            }
        }
    }
    None
}

#[cfg(target_os = "android")]
const ANDROID_FONT_DIRS: &[&str] = &["/system/fonts", "/product/fonts", "/system_ext/fonts"];

#[cfg(target_os = "android")]
fn scan_android_system_font(prefer_mono: bool) -> Option<Font> {
    scan_font_dirs(ANDROID_FONT_DIRS, prefer_mono)
}

#[cfg(target_os = "android")]
const ANDROID_SANS_REG: &[&str] = &[
    "/system/fonts/Roboto-Regular.ttf",
    "/system/fonts/RobotoStatic-Regular.ttf",
    "/system/fonts/RobotoFlex-Regular.ttf",
    "/system/fonts/DroidSans.ttf",
    "/system/fonts/NotoSans-Regular.ttf",
    "/system/fonts/Roboto-Medium.ttf",
];

#[cfg(target_os = "android")]
const ANDROID_SANS_BOLD: &[&str] = &[
    "/system/fonts/Roboto-Bold.ttf",
    "/system/fonts/RobotoStatic-Bold.ttf",
    "/system/fonts/Roboto-Medium.ttf",
    "/system/fonts/DroidSans-Bold.ttf",
    "/system/fonts/Roboto-Regular.ttf",
    "/system/fonts/RobotoFlex-Regular.ttf",
];

#[cfg(target_os = "android")]
const ANDROID_MONO_REG: &[&str] = &[
    "/system/fonts/DroidSansMono.ttf",
    "/system/fonts/CutiveMono.ttf",
    "/system/fonts/RobotoMono-Regular.ttf",
    "/system/fonts/NotoSansMono-Regular.ttf",
];

#[cfg(target_os = "android")]
const ANDROID_MONO_BOLD: &[&str] = &[
    "/system/fonts/RobotoMono-Bold.ttf",
    "/system/fonts/NotoSansMono-Bold.ttf",
    "/system/fonts/DroidSansMono.ttf",
    "/system/fonts/CutiveMono.ttf",
];

#[cfg(target_os = "android")]
const ANDROID_SYMBOL_FALLBACK: &[&str] = &[
    "/system/fonts/NotoSansSymbols-Regular-Subsetted.ttf",
    "/system/fonts/NotoSansSymbols-Regular-Subsetted2.ttf",
    "/system/fonts/NotoSansSymbols-Regular.ttf",
    "/system/fonts/DroidSansFallback.ttf",
];

#[cfg(target_os = "android")]
fn load_android_mono_reg() -> Result<Font, String> {
    if let Some(f) = load_font_candidates(ANDROID_MONO_REG) {
        return Ok(f);
    }
    if let Some(f) = scan_android_system_font(true) {
        return Ok(f);
    }
    load_system_font(ANDROID_SANS_REG)
}

#[cfg(target_os = "android")]
fn load_android_mono_bold() -> Result<Font, String> {
    if let Some(f) = load_font_candidates(ANDROID_MONO_BOLD) {
        return Ok(f);
    }
    load_android_mono_reg()
}

#[cfg(target_os = "macos")]
const MACOS_SANS_REG: &[&str] = &[
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/Geneva.ttf",
    "/System/Library/Fonts/Supplemental/Verdana.ttf",
];

#[cfg(target_os = "macos")]
const MACOS_SANS_BOLD: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Supplemental/Verdana Bold.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
];

#[cfg(target_os = "macos")]
const MACOS_MONO_REG: &[&str] = &[
    "/System/Library/Fonts/SFNSMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Monaco.ttf",
    "/System/Library/Fonts/Supplemental/Courier New.ttf",
    "/System/Library/Fonts/Supplemental/Andale Mono.ttf",
];

#[cfg(target_os = "macos")]
const MACOS_MONO_BOLD: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Courier New Bold.ttf",
    "/System/Library/Fonts/SFNSMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
];

#[cfg(target_os = "macos")]
const MACOS_SYMBOL_FALLBACK: &[&str] = &[
    "/System/Library/Fonts/Apple Symbols.ttf",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/System/Library/Fonts/SFNS.ttf",
];

#[cfg(all(not(target_os = "windows"), not(target_os = "android"), not(target_os = "macos")))]
fn fc_match_font(pattern: &str) -> Option<String> {
    let out = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", pattern])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "android"), not(target_os = "macos")))]
fn load_linux_font_with_fc(candidates: &[&str], fc_pattern: &str) -> Result<Font, String> {
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                return Ok(font);
            }
        }
    }
    if let Some(path) = fc_match_font(fc_pattern) {
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                return Ok(font);
            }
        }
    }
    load_system_font(candidates)
}

impl TextRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        {
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
        #[cfg(target_os = "android")]
        {
            let reg = load_system_font(ANDROID_SANS_REG)?;
            let bold = load_system_font(ANDROID_SANS_BOLD)
                .or_else(|_| load_system_font(ANDROID_SANS_REG))?;
            let fallback = load_font_candidates(ANDROID_SYMBOL_FALLBACK);
            Self::new_from_loaded_fonts(gl, reg, bold, fallback)
        }
        #[cfg(target_os = "macos")]
        {
            let reg = load_system_font(MACOS_SANS_REG)?;
            let bold = load_system_font(MACOS_SANS_BOLD)
                .or_else(|_| load_system_font(MACOS_SANS_REG))?;
            let fallback = load_font_candidates(MACOS_SYMBOL_FALLBACK);
            Self::new_from_loaded_fonts(gl, reg, bold, fallback)
        }
        #[cfg(all(not(target_os = "windows"), not(target_os = "android"), not(target_os = "macos")))]
        {
            let reg = load_linux_font_with_fc(
                &[
                    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                    "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
                    "/usr/share/fonts/TTF/Inter-Regular.ttf",
                    "/usr/share/fonts/TTF/DejaVuSans.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
                    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                    "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
                    "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
                    "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf",
                    "/usr/share/fonts/cantarell/Cantarell-Regular.otf",
                ],
                "sans-serif:weight=regular",
            )?;
            let bold = load_linux_font_with_fc(
                &[
                    "/usr/share/fonts/noto/NotoSans-Bold.ttf",
                    "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
                    "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
                    "/usr/share/fonts/TTF/Inter-Bold.ttf",
                    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
                    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
                    "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
                    "/usr/share/fonts/truetype/ubuntu/Ubuntu-B.ttf",
                    "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Bold.ttf",
                    "/usr/share/fonts/cantarell/Cantarell-Bold.otf",
                ],
                "sans-serif:weight=bold",
            )
            .or_else(|_| load_linux_font_with_fc(&[], "sans-serif"))?;
            let fallback = load_linux_font_with_fc(
                &[
                    "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
                    "/usr/share/fonts/noto/NotoSansSymbols-Regular.ttf",
                    "/usr/share/fonts/TTF/DejaVuSans.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
                ],
                "DejaVu Sans",
            )
            .ok();
            Self::new_from_loaded_fonts(gl, reg, bold, fallback)
        }
    }

    pub unsafe fn new_monospace(gl: &GlFns) -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        {
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
        #[cfg(target_os = "android")]
        {
            let reg = load_android_mono_reg()?;
            let bold = load_android_mono_bold()?;
            let fallback = load_font_candidates(ANDROID_SYMBOL_FALLBACK);
            Self::new_from_loaded_fonts(gl, reg, bold, fallback)
        }
        #[cfg(target_os = "macos")]
        {
            let reg = load_system_font(MACOS_MONO_REG)
                .or_else(|_| load_system_font(MACOS_SANS_REG))?;
            let bold = load_system_font(MACOS_MONO_BOLD)
                .or_else(|_| load_system_font(MACOS_MONO_REG))
                .or_else(|_| load_system_font(MACOS_SANS_REG))?;
            let fallback = load_font_candidates(MACOS_SYMBOL_FALLBACK);
            Self::new_from_loaded_fonts(gl, reg, bold, fallback)
        }
        #[cfg(all(not(target_os = "windows"), not(target_os = "android"), not(target_os = "macos")))]
        {
            let reg = load_linux_font_with_fc(
                &[
                    "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
                    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                    "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
                    "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
                    "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
                    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
                    "/usr/share/fonts/liberation-mono/LiberationMono-Regular.ttf",
                ],
                "monospace:weight=regular",
            )?;
            let bold = load_linux_font_with_fc(
                &[
                    "/usr/share/fonts/TTF/JetBrainsMono-Bold.ttf",
                    "/usr/share/fonts/TTF/DejaVuSansMono-Bold.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
                    "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono-Bold.ttf",
                    "/usr/share/fonts/noto/NotoSansMono-Bold.ttf",
                    "/usr/share/fonts/truetype/noto/NotoSansMono-Bold.ttf",
                    "/usr/share/fonts/truetype/liberation/LiberationMono-Bold.ttf",
                    "/usr/share/fonts/liberation-mono/LiberationMono-Bold.ttf",
                ],
                "monospace:weight=bold",
            )
            .or_else(|_| load_linux_font_with_fc(&[], "monospace"))?;
            let fallback = load_linux_font_with_fc(
                &[
                    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                    "/usr/share/fonts/TTF/DejaVuSans.ttf",
                ],
                "DejaVu Sans Mono",
            )
            .ok();
            Self::new_from_loaded_fonts(gl, reg, bold, fallback)
        }
    }

    #[cfg(target_os = "windows")]
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
        Self::new_from_loaded_fonts(gl, font_regular, font_bold, font_fallback)
    }

    unsafe fn new_from_loaded_fonts(
        gl: &GlFns,
        font_regular: Font,
        font_bold: Font,
        font_fallback: Option<Font>,
    ) -> Result<Self, String> {

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

/// CPU-side UI font pair (regular + bold) for themes that bake word
/// typography offscreen without a GL context (eye-within-words). Mirrors the
/// `TextRenderer::new` platform candidate lists.
pub fn load_baking_font_pair() -> Result<(Font, Font), String> {
    #[cfg(target_os = "windows")]
    {
        let reg = load_system_font(&[
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\arial.ttf",
        ])?;
        let bold = load_system_font(&[
            r"C:\Windows\Fonts\segoeuib.ttf",
            r"C:\Windows\Fonts\seguisb.ttf",
            r"C:\Windows\Fonts\arialbd.ttf",
            r"C:\Windows\Fonts\segoeui.ttf",
        ])?;
        Ok((reg, bold))
    }
    #[cfg(target_os = "android")]
    {
        let reg = load_system_font(ANDROID_SANS_REG)?;
        let bold = load_system_font(ANDROID_SANS_BOLD)
            .or_else(|_| load_system_font(ANDROID_SANS_REG))?;
        Ok((reg, bold))
    }
    #[cfg(target_os = "macos")]
    {
        let reg = load_system_font(MACOS_SANS_REG)?;
        let bold = load_system_font(MACOS_SANS_BOLD)
            .or_else(|_| load_system_font(MACOS_SANS_REG))?;
        Ok((reg, bold))
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "android"), not(target_os = "macos")))]
    {
        let reg = load_linux_font_with_fc(
            &[
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/TTF/Inter-Regular.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
                "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
                "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf",
                "/usr/share/fonts/cantarell/Cantarell-Regular.otf",
            ],
            "sans-serif:weight=regular",
        )?;
        let bold = load_linux_font_with_fc(
            &[
                "/usr/share/fonts/noto/NotoSans-Bold.ttf",
                "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
                "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
                "/usr/share/fonts/TTF/Inter-Bold.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
                "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
                "/usr/share/fonts/truetype/ubuntu/Ubuntu-B.ttf",
                "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Bold.ttf",
                "/usr/share/fonts/cantarell/Cantarell-Bold.otf",
            ],
            "sans-serif:weight=bold",
        )
        .or_else(|_| load_linux_font_with_fc(&[], "sans-serif"))?;
        Ok((reg, bold))
    }
}

/// Monospace UI font loader for ASCII-art themes (`cosmic-eye`).
pub fn load_mono_font() -> Result<Font, String> {
    #[cfg(target_os = "windows")]
    {
        load_system_font(&[
            r"C:\Windows\Fonts\consola.ttf",
            r"C:\Windows\Fonts\CascadiaMono.ttf",
            r"C:\Windows\Fonts\cour.ttf",
            r"C:\Windows\Fonts\lucon.ttf",
        ])
        .or_else(|_| load_baking_font_pair().map(|(r, _)| r))
    }
    #[cfg(target_os = "android")]
    {
        load_android_mono_reg()
    }
    #[cfg(target_os = "macos")]
    {
        load_system_font(MACOS_MONO_REG)
            .or_else(|_| load_baking_font_pair().map(|(r, _)| r))
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "android"), not(target_os = "macos")))]
    {
        load_linux_font_with_fc(
            &[
                "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
                "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
                "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
                "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
                "/usr/share/fonts/liberation-mono/LiberationMono-Regular.ttf",
                "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
            ],
            "monospace:weight=regular",
        )
        .or_else(|_| load_baking_font_pair().map(|(r, _)| r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_font_dir_scanner_prefers_mono_and_handles_missing_dirs() {
        assert!(scan_font_dirs(&["/nonexistent/android/fonts/dir"], false).is_none());
        assert!(scan_font_dirs(&["/nonexistent/android/fonts/dir"], true).is_none());
        // Verify against real system font dirs on Linux host when present.
        let linux_dirs = [
            "/usr/share/fonts/TTF",
            "/usr/share/fonts/noto",
            "/usr/share/fonts/truetype/dejavu",
            "/usr/share/fonts/dejavu-sans-fonts",
            "/usr/share/fonts/liberation",
        ];
        if linux_dirs.iter().any(|d| std::path::Path::new(d).is_dir()) {
            assert!(
                scan_font_dirs(&linux_dirs, false).is_some(),
                "Expected scan_font_dirs(prefer_mono=false) to load a font"
            );
        }
    }
}
