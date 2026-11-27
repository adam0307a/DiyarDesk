use crate::ai_mascots::AiMascotsRenderer;
use crate::aurora::AuroraRenderer;
use crate::beach::BeachRenderer;
use crate::cat_shore::CatShoreRenderer;
use crate::claymorph::ClaymorphRenderer;
use crate::cosmic_eye::CosmicEyeRenderer;
use crate::custom_glsl::CustomGlslRenderer;
use crate::custom_plugin::CustomPluginRenderer;
use crate::eye_within_words::EyeWithinWordsRenderer;
use crate::neobrutalism::NeobrutalismRenderer;
use crate::gl::*;
use crate::liquidglass::LiquidGlassRenderer;
use crate::glade::GladeRenderer;
use crate::istanbul::IstanbulRenderer;
use crate::meadow::MeadowRenderer;
use crate::neumorph::NeumorphRenderer;
use crate::papercut::PapercutRenderer;
use crate::persephone::PersephoneRenderer;
use crate::desert::DesertRenderer;
use crate::voxelcraft::VoxelcraftRenderer;
use crate::pocket::PocketRenderer;
use crate::polarnight::PolarNightRenderer;
use crate::quiet_snow::QuietSnowRenderer;
use crate::rift::RiftRenderer;
use crate::roach::RoachRenderer;
use crate::rosebrook::RosebrookRenderer;
use crate::shaders::*;
use crate::software_world::SoftwareWorldRenderer;
use crate::solar_system::SolarSystemRenderer;
use crate::state::*;
use crate::strike::StrikeRenderer;
use crate::studio_glass::StudioGlassRenderer;
use crate::text::TextRenderer;
use crate::themes::resolve_shader_and_params;
use crate::thinking_orbs::ThinkingOrbsRenderer;
use crate::winter::WinterRenderer;
use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike};
use std::collections::HashMap;
use std::ffi::c_void;

struct FboTex {
    fbo: GLuint,
    tex: GLuint,
    w: i32,
    h: i32,
}

impl FboTex {
    fn empty() -> Self {
        Self { fbo: 0, tex: 0, w: 0, h: 0 }
    }

    unsafe fn ensure(&mut self, gl: &GlFns, w: i32, h: i32) {
        let w = w.max(16);
        let h = h.max(16);
        if self.fbo != 0 && self.w == w && self.h == h {
            return;
        }
        self.destroy(gl);
        (gl.GenFramebuffers)(1, &mut self.fbo);
        (gl.GenTextures)(1, &mut self.tex);
        (gl.BindTexture)(GL_TEXTURE_2D, self.tex);
        (gl.TexImage2D)(
            GL_TEXTURE_2D, 0, GL_RGBA8 as GLint, w, h, 0,
            GL_RGBA, GL_UNSIGNED_BYTE, std::ptr::null(),
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.fbo);
        (gl.FramebufferTexture2D)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, self.tex, 0);
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        self.w = w;
        self.h = h;
    }

    unsafe fn destroy(&mut self, gl: &GlFns) {
        if self.tex != 0 { (gl.DeleteTextures)(1, &self.tex); self.tex = 0; }
        if self.fbo != 0 { (gl.DeleteFramebuffers)(1, &self.fbo); self.fbo = 0; }
        self.w = 0;
        self.h = 0;
    }
}

pub struct Renderer {
    dummy_vao: GLuint,
    prog_bg: GLuint,
    prog_blur: GLuint,
    prog_blit: GLuint,
    prog_ui: GLuint,
    prog_audio_pulse: GLuint,
    scene_fbo: FboTex,
    blur_a: FboTex,
    blur_b: FboTex,
    art_tex: GLuint,
    text: TextRenderer,
    studio_renderer: Option<StudioGlassRenderer>,
    quiet_snow_renderer: Option<QuietSnowRenderer>,
    papercut_renderer: Option<PapercutRenderer>,
    voxelcraft_renderer: Option<VoxelcraftRenderer>,
    desert_renderer: Option<DesertRenderer>,
    claymorph_renderer: Option<ClaymorphRenderer>,
    neobrutalism_renderer: Option<NeobrutalismRenderer>,
    liquidglass_renderer: Option<LiquidGlassRenderer>,
    zen_archipelago_renderer: Option<crate::zen_archipelago::ZenArchipelagoRenderer>,
    software_world_renderer: Option<SoftwareWorldRenderer>,
    meadow_renderer: Option<MeadowRenderer>,
    beach_renderer: Option<BeachRenderer>,
    aurora_renderer: Option<AuroraRenderer>,
    glade_renderer: Option<GladeRenderer>,
    rift_renderer: Option<RiftRenderer>,
    strike_renderer: Option<StrikeRenderer>,
    pocket_renderer: Option<PocketRenderer>,
    roach_renderer: Option<RoachRenderer>,
    rosebrook_renderer: Option<RosebrookRenderer>,
    polarnight_renderer: Option<PolarNightRenderer>,
    winter_renderer: Option<WinterRenderer>,
    neumorph_renderer: Option<NeumorphRenderer>,
    istanbul_renderer: Option<IstanbulRenderer>,
    solar_system_renderer: Option<SolarSystemRenderer>,
    thinking_orbs_renderer: Option<ThinkingOrbsRenderer>,
    ai_mascots_renderer: Option<AiMascotsRenderer>,
    cat_shore_renderer: Option<CatShoreRenderer>,
    persephone_renderer: Option<PersephoneRenderer>,
    eye_within_words_renderer: Option<EyeWithinWordsRenderer>,
    eye_within_words_failed: bool,
    cosmic_eye_renderer: Option<CosmicEyeRenderer>,
    cosmic_eye_failed: bool,
    custom_glsl_renderer: Option<CustomGlslRenderer>,
    custom_plugin_renderer: Option<CustomPluginRenderer>,
    active_theme_id: String,
    theme_progs: HashMap<String, GLuint>,
}

impl Renderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let mut dummy_vao = 0;
        (gl.GenVertexArrays)(1, &mut dummy_vao);

        let prog_bg = gl.compile_program(FULLSCREEN_VERT, BACKDROP_FRAG)?;
        let prog_blur = gl.compile_program(FULLSCREEN_VERT, BLUR_FRAG)?;
        let prog_blit = gl.compile_program(FULLSCREEN_VERT, BLIT_FRAG)?;
        let prog_ui = gl.compile_program(SDF_UI_VERT, SDF_UI_FRAG)?;
        let prog_audio_pulse = gl.compile_program(FULLSCREEN_VERT, AUDIO_PULSE_FRAG)?;
        let text = TextRenderer::new(gl)?;

        let mut art_tex = 0;
        (gl.GenTextures)(1, &mut art_tex);
        (gl.BindTexture)(GL_TEXTURE_2D, art_tex);
        let px: [u8; 4] = [20, 24, 30, 255];
        (gl.TexImage2D)(
            GL_TEXTURE_2D, 0, GL_RGBA8 as GLint, 1, 1, 0,
            GL_RGBA, GL_UNSIGNED_BYTE, px.as_ptr() as *const c_void,
        );
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        (gl.TexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

        Ok(Self {
            dummy_vao,
            prog_bg,
            prog_blur,
            prog_blit,
            prog_ui,
            prog_audio_pulse,
            scene_fbo: FboTex::empty(),
            blur_a: FboTex::empty(),
            blur_b: FboTex::empty(),
            art_tex,
            text,
            studio_renderer: None,
            quiet_snow_renderer: None,
            papercut_renderer: None,
            voxelcraft_renderer: None,
            desert_renderer: None,
            claymorph_renderer: None,
            neobrutalism_renderer: None,
            liquidglass_renderer: None,
            zen_archipelago_renderer: None,
            software_world_renderer: None,
            meadow_renderer: None,
            beach_renderer: None,
            aurora_renderer: None,
            glade_renderer: None,
            rift_renderer: None,
            strike_renderer: None,
            pocket_renderer: None,
            roach_renderer: None,
            rosebrook_renderer: None,
            polarnight_renderer: None,
            winter_renderer: None,
            neumorph_renderer: None,
            istanbul_renderer: None,
            solar_system_renderer: None,
            thinking_orbs_renderer: None,
            ai_mascots_renderer: None,
            cat_shore_renderer: None,
            persephone_renderer: None,
            eye_within_words_renderer: None,
            eye_within_words_failed: false,
            cosmic_eye_renderer: None,
            cosmic_eye_failed: false,
            custom_glsl_renderer: None,
            custom_plugin_renderer: None,
            active_theme_id: String::new(),
            theme_progs: HashMap::new(),
        })
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        for (_, prog) in self.theme_progs.drain() {
            if prog != 0 { (gl.DeleteProgram)(prog); }
        }
        if let Some(mut sr) = self.studio_renderer.take() {
            sr.destroy(gl);
        }
        if let Some(mut qr) = self.quiet_snow_renderer.take() {
            qr.destroy(gl);
        }
        if let Some(mut pr) = self.papercut_renderer.take() {
            pr.destroy(gl);
        }
        if let Some(mut vr) = self.voxelcraft_renderer.take() {
            vr.destroy(gl);
        }
        if let Some(mut dr) = self.desert_renderer.take() {
            dr.destroy(gl);
        }
        if let Some(mut cr) = self.claymorph_renderer.take() {
            cr.destroy(gl);
        }
        if let Some(mut nr) = self.neobrutalism_renderer.take() {
            nr.destroy(gl);
        }
        if let Some(mut lg) = self.liquidglass_renderer.take() {
            lg.destroy(gl);
        }
        if let Some(mut za) = self.zen_archipelago_renderer.take() {
            za.destroy(gl);
        }
        if let Some(mut swr) = self.software_world_renderer.take() {
            swr.destroy(gl);
        }
        if let Some(mut mr) = self.meadow_renderer.take() {
            mr.destroy(gl);
        }
        if let Some(mut br) = self.beach_renderer.take() {
            br.destroy(gl);
        }
        if let Some(mut ar) = self.aurora_renderer.take() {
            ar.destroy(gl);
        }
        if let Some(mut gr) = self.glade_renderer.take() {
            gr.destroy(gl);
        }
        if let Some(mut rr) = self.rift_renderer.take() {
            rr.destroy(gl);
        }
        if let Some(mut sr) = self.strike_renderer.take() {
            sr.destroy(gl);
        }
        if let Some(mut pr) = self.pocket_renderer.take() {
            pr.destroy(gl);
        }
        if let Some(mut rr) = self.roach_renderer.take() {
            rr.destroy(gl);
        }
        if let Some(mut rb) = self.rosebrook_renderer.take() {
            rb.destroy(gl);
        }
        if let Some(mut pnr) = self.polarnight_renderer.take() {
            pnr.destroy(gl);
        }
        if let Some(mut wr) = self.winter_renderer.take() {
            wr.destroy(gl);
        }
        if let Some(mut nr) = self.neumorph_renderer.take() {
            nr.destroy(gl);
        }
        if let Some(mut ir) = self.istanbul_renderer.take() {
            ir.destroy(gl);
        }
        if let Some(mut ssr) = self.solar_system_renderer.take() {
            ssr.destroy(gl);
        }
        if let Some(mut tor) = self.thinking_orbs_renderer.take() {
            tor.destroy(gl);
        }
        if let Some(mut amr) = self.ai_mascots_renderer.take() {
            amr.destroy(gl);
        }
        if let Some(mut csr) = self.cat_shore_renderer.take() {
            csr.destroy(gl);
        }
        if let Some(mut pr) = self.persephone_renderer.take() {
            pr.destroy(gl);
        }
        if let Some(mut er) = self.eye_within_words_renderer.take() {
            er.destroy(gl);
        }
        if let Some(mut cr) = self.cosmic_eye_renderer.take() {
            cr.destroy(gl);
        }
        if let Some(mut cgr) = self.custom_glsl_renderer.take() {
            cgr.destroy(gl);
        }
        if let Some(mut cpr) = self.custom_plugin_renderer.take() {
            cpr.destroy(gl);
        }
        self.scene_fbo.destroy(gl);
        self.blur_a.destroy(gl);
        self.blur_b.destroy(gl);
        self.text.destroy(gl);
        if self.art_tex != 0 { (gl.DeleteTextures)(1, &self.art_tex); self.art_tex = 0; }
        if self.dummy_vao != 0 { (gl.DeleteVertexArrays)(1, &self.dummy_vao); self.dummy_vao = 0; }
        if self.prog_bg != 0 { (gl.DeleteProgram)(self.prog_bg); self.prog_bg = 0; }
        if self.prog_blur != 0 { (gl.DeleteProgram)(self.prog_blur); self.prog_blur = 0; }
        if self.prog_blit != 0 { (gl.DeleteProgram)(self.prog_blit); self.prog_blit = 0; }
        if self.prog_ui != 0 { (gl.DeleteProgram)(self.prog_ui); self.prog_ui = 0; }
        if self.prog_audio_pulse != 0 { (gl.DeleteProgram)(self.prog_audio_pulse); self.prog_audio_pulse = 0; }
    }

    unsafe fn upload_pending_art(&mut self, gl: &GlFns, st: &mut AppState) {
        if let Some(art) = st.media.pending_art.take() {
            (gl.BindTexture)(GL_TEXTURE_2D, self.art_tex);
            (gl.PixelStorei)(GL_UNPACK_ALIGNMENT, 4);
            (gl.TexImage2D)(
                GL_TEXTURE_2D, 0, GL_RGBA8 as GLint,
                art.width as GLsizei, art.height as GLsizei, 0,
                GL_RGBA, GL_UNSIGNED_BYTE, art.rgba.as_ptr() as *const c_void,
            );
            st.media.has_art_tex = true;
        }
    }

    unsafe fn draw_ui_prim(
        &self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        kind: i32,
        r: Rect,
        col: [f32; 4],
        col2: [f32; 4],
        col3: [f32; 4],
        params: [f32; 4],
        blur_tex: GLuint,
    ) {
        (gl.UseProgram)(self.prog_ui);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_ui, "u_res"), css_w, css_h);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_ui, "u_kind"), kind);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_ui, "u_rect"), r.x, r.y, r.w, r.h);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_ui, "u_color"), col[0], col[1], col[2], col[3]);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_ui, "u_color2"), col2[0], col2[1], col2[2], col2[3]);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_ui, "u_color3"), col3[0], col3[1], col3[2], col3[3]);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_ui, "u_params"), params[0], params[1], params[2], params[3]);

        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, blur_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_ui, "u_blur_tex"), 0);

        (gl.ActiveTexture)(GL_TEXTURE1);
        (gl.BindTexture)(GL_TEXTURE_2D, self.art_tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_ui, "u_art_tex"), 1);

        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLE_STRIP, 0, 4);
    }

    // Crisp analytical SDF vector icon (u_kind == 10)
    // 0=Play, 1=Pause, 2=Prev, 3=Next, 4=Search, 5=Check, 6=Star(y>0.5 filled),
    // 7=Plus, 8=Close, 9=ChevronL, 10=ChevronR, 11=Swap, 12=Reset
    unsafe fn draw_icon(
        &self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        icon_id: f32,
        r: Rect,
        col: [f32; 4],
        extra: f32,
        blur_tex: GLuint,
    ) {
        if col[3] <= 0.005 { return; }
        self.draw_ui_prim(
            gl, css_w, css_h, 10, r, col,
            [0.0; 4], [0.0; 4], [icon_id, extra, 0.0, 0.0], blur_tex,
        );
    }

    unsafe fn draw_glass_card(
        &self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        r: Rect,
        rad: f32,
        pal: &PaletteColors,
        opacity: f32,
        hovered: bool,
        dragged: bool,
        blur_tex: GLuint,
    ) {
        let pad = 22.0;
        let expanded = Rect {
            x: r.x - pad,
            y: r.y - pad,
            w: r.w + pad * 2.0,
            h: r.h + pad * 2.0,
        };
        let sh_a = if dragged {
            if pal.is_light { 0.22 } else { 0.48 }
        } else if pal.is_light {
            0.14
        } else {
            0.34
        };
        let border = if dragged {
            [pal.primary[0], pal.primary[1], pal.primary[2], 0.72]
        } else if hovered {
            pal.outline_hi
        } else {
            pal.outline
        };
        self.draw_ui_prim(
            gl, css_w, css_h, 0, expanded,
            [pal.surface[0], pal.surface[1], pal.surface[2], opacity],
            border,
            [0.0; 4],
            [rad, pad, sh_a, 0.0],
            blur_tex,
        );
    }

    unsafe fn draw_diyar_dots(&self, gl: &GlFns, css_w: f32, css_h: f32, x: f32, y: f32, s: f32, light: bool, blur_tex: GLuint) {
        let cols = if light { &DIYAR_BRAND_LIGHT } else { &DIYAR_BRAND_DARK };
        let d = 6.0 * s;
        for (i, c) in cols.iter().enumerate() {
            self.draw_ui_prim(
                gl, css_w, css_h, 1,
                Rect { x: x + (i as f32) * 9.0 * s, y, w: d, h: d },
                [c[0], c[1], c[2], 1.0],
                [0.0; 4], [0.0; 4], [d * 0.5, 0.0, 0.0, 0.0], blur_tex,
            );
        }
    }

    unsafe fn draw_caret(
        &self,
        gl: &GlFns,
        css_w: f32,
        css_h: f32,
        x: f32,
        y: f32,
        h: f32,
        col: [f32; 4],
        blur_tex: GLuint,
    ) {
        self.draw_ui_prim(
            gl, css_w, css_h, 1,
            Rect { x, y, w: 1.6, h },
            col, [0.0; 4], [0.0; 4], [0.8, 0.0, 0.0, 0.0], blur_tex,
        );
    }

    unsafe fn render_shader_theme(&mut self, gl: &GlFns, st: &AppState) {
        let (frag_src, packed) = resolve_shader_and_params(st);
        let prog = if let Some(&p) = self.theme_progs.get(&st.theme_id) {
            p
        } else {
            match gl.compile_program(FULLSCREEN_VERT, &frag_src) {
                Ok(p) => {
                    self.theme_progs.insert(st.theme_id.clone(), p);
                    p
                }
                Err(_) => self.prog_bg,
            }
        };

        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(prog);
        (gl.Uniform2f)(gl.uniform_loc(prog, "u_res"), css_w, css_h);
        (gl.Uniform1f)(gl.uniform_loc(prog, "u_time"), st.sim_time);
        (gl.Uniform1f)(gl.uniform_loc(prog, "u_seed"), st.params.seed as f32);
        (gl.Uniform2f)(gl.uniform_loc(prog, "u_pointer"), st.pointer_x, st.pointer_y);
        (gl.Uniform1i)(gl.uniform_loc(prog, "u_pointer_on"), if st.pointer_active && !st.low_mode { 1 } else { 0 });
        (gl.Uniform1i)(gl.uniform_loc(prog, "u_adaptive"), if st.params.adaptive { 1 } else { 0 });
        (gl.Uniform1i)(gl.uniform_loc(prog, "u_mode"), packed.mode);
        (gl.Uniform1i)(gl.uniform_loc(prog, "u_palette"), packed.palette);
        (gl.Uniform4f)(gl.uniform_loc(prog, "u_p0"), packed.p0[0], packed.p0[1], packed.p0[2], packed.p0[3]);
        (gl.Uniform4f)(gl.uniform_loc(prog, "u_p1"), packed.p1[0], packed.p1[1], packed.p1[2], packed.p1[3]);
        (gl.Uniform4f)(gl.uniform_loc(prog, "u_p2"), packed.p2[0], packed.p2[1], packed.p2[2], packed.p2[3]);
        (gl.Uniform4f)(gl.uniform_loc(prog, "u_p3"), packed.p3[0], packed.p3[1], packed.p3[2], packed.p3[3]);

        let ic_cnt = st.effective_icons.len().min(48);
        (gl.Uniform1i)(gl.uniform_loc(prog, "u_icon_count"), ic_cnt as i32);
        if ic_cnt > 0 {
            let mut flat = [0.0f32; 48 * 4];
            for (i, ic) in st.effective_icons.iter().take(48).enumerate() {
                flat[i * 4] = ic.x;
                flat[i * 4 + 1] = ic.y;
                flat[i * 4 + 2] = ic.w;
                flat[i * 4 + 3] = ic.h;
            }
            (gl.Uniform4fv)(gl.uniform_loc(prog, "u_icons"), ic_cnt as GLsizei, flat.as_ptr());
        }

        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
    }

    /// Returns the number of currently instantiated per-theme sub-renderers.
    #[allow(dead_code)]
    pub fn loaded_renderer_count(&self) -> usize {
        let flags = [
            self.studio_renderer.is_some(),
            self.quiet_snow_renderer.is_some(),
            self.papercut_renderer.is_some(),
            self.voxelcraft_renderer.is_some(),
            self.desert_renderer.is_some(),
            self.claymorph_renderer.is_some(),
            self.neobrutalism_renderer.is_some(),
            self.liquidglass_renderer.is_some(),
            self.zen_archipelago_renderer.is_some(),
            self.software_world_renderer.is_some(),
            self.meadow_renderer.is_some(),
            self.beach_renderer.is_some(),
            self.aurora_renderer.is_some(),
            self.glade_renderer.is_some(),
            self.rift_renderer.is_some(),
            self.strike_renderer.is_some(),
            self.pocket_renderer.is_some(),
            self.roach_renderer.is_some(),
            self.rosebrook_renderer.is_some(),
            self.polarnight_renderer.is_some(),
            self.winter_renderer.is_some(),
            self.neumorph_renderer.is_some(),
            self.istanbul_renderer.is_some(),
            self.solar_system_renderer.is_some(),
            self.thinking_orbs_renderer.is_some(),
            self.ai_mascots_renderer.is_some(),
            self.cat_shore_renderer.is_some(),
            self.persephone_renderer.is_some(),
            self.eye_within_words_renderer.is_some(),
            self.cosmic_eye_renderer.is_some(),
            self.custom_glsl_renderer.is_some(),
            self.custom_plugin_renderer.is_some(),
        ];
        flags.iter().filter(|&&b| b).count()
    }

    /// Destroys GPU resources (FBOs, textures, VAOs, programs) owned by inactive themes
    /// whenever `st.theme_id` changes, keeping resident VRAM/RAM bounded across theme switches.
    unsafe fn evict_inactive_renderers(&mut self, gl: &GlFns, st: &AppState) {
        if self.active_theme_id == st.theme_id {
            return;
        }
        if !self.active_theme_id.is_empty() {
            macro_rules! evict_slot {
                ($field:ident, $keep:expr) => {
                    if !$keep {
                        if let Some(mut sub) = self.$field.take() {
                            sub.destroy(gl);
                        }
                    }
                };
            }
            evict_slot!(custom_plugin_renderer, st.is_custom_plugin_theme());
            evict_slot!(custom_glsl_renderer, st.is_custom_glsl_theme());
            evict_slot!(studio_renderer, st.is_studio_glass_theme());
            evict_slot!(quiet_snow_renderer, st.is_quiet_snow_theme());
            evict_slot!(papercut_renderer, st.is_papercut_theme());
            evict_slot!(voxelcraft_renderer, st.is_voxelcraft_theme());
            evict_slot!(desert_renderer, st.is_desert_theme());
            evict_slot!(claymorph_renderer, st.is_claymorph_theme());
            evict_slot!(neobrutalism_renderer, st.is_neobrutalism_theme());
            evict_slot!(liquidglass_renderer, st.is_liquidglass_theme());
            evict_slot!(zen_archipelago_renderer, st.is_zen_archipelago_theme());
            evict_slot!(software_world_renderer, st.is_software_world_theme());
            evict_slot!(meadow_renderer, st.is_meadow_theme());
            evict_slot!(beach_renderer, st.is_beach_theme());
            evict_slot!(aurora_renderer, st.is_aurora_theme());
            evict_slot!(glade_renderer, st.is_glade_theme());
            evict_slot!(rift_renderer, st.is_rift_theme());
            evict_slot!(strike_renderer, st.is_strike_theme());
            evict_slot!(pocket_renderer, st.is_pocket_theme());
            evict_slot!(roach_renderer, st.is_roach_theme());
            evict_slot!(rosebrook_renderer, st.is_rosebrook_theme());
            evict_slot!(polarnight_renderer, st.is_polarnight_theme());
            evict_slot!(winter_renderer, st.is_winter_theme());
            evict_slot!(neumorph_renderer, st.is_neumorph_theme());
            evict_slot!(istanbul_renderer, st.is_istanbul_theme());
            evict_slot!(solar_system_renderer, st.is_solar_system_theme());
            evict_slot!(thinking_orbs_renderer, st.is_thinking_orbs_theme());
            evict_slot!(ai_mascots_renderer, st.is_ai_mascots_theme());
            evict_slot!(cat_shore_renderer, st.is_cat_shore_theme());
            evict_slot!(persephone_renderer, st.is_persephone_theme());
            evict_slot!(eye_within_words_renderer, st.is_eye_within_words_theme());
            evict_slot!(cosmic_eye_renderer, st.is_cosmic_eye_theme());

            if !st.is_impulse_theme() {
                self.scene_fbo.destroy(gl);
                self.blur_a.destroy(gl);
                self.blur_b.destroy(gl);
            }

            let keep_id = st.theme_id.as_str();
            self.theme_progs.retain(|k, prog| {
                if k == keep_id {
                    true
                } else {
                    if *prog != 0 {
                        (gl.DeleteProgram)(*prog);
                    }
                    false
                }
            });
        }
        self.active_theme_id = st.theme_id.clone();
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        self.render_active_theme(gl, st);
        self.draw_audio_pulse_overlay(gl, st);
    }

    unsafe fn draw_audio_pulse_overlay(&self, gl: &GlFns, st: &AppState) {
        if !st.audio.is_active() || self.prog_audio_pulse == 0 {
            return;
        }
        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let pal = st.palette_colors();
        let accent = pal.primary;

        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Enable)(GL_BLEND);
        (gl.BlendFuncSeparate)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ZERO, GL_ONE);
        (gl.UseProgram)(self.prog_audio_pulse);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_audio_pulse, "u_res"), st.css_w(), st.css_h());
        (gl.Uniform1f)(gl.uniform_loc(self.prog_audio_pulse, "u_time"), st.sim_time);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_audio_pulse, "u_level"), st.audio.level());
        (gl.Uniform1f)(gl.uniform_loc(self.prog_audio_pulse, "u_beat"), st.audio.beat());
        (gl.Uniform3f)(
            gl.uniform_loc(self.prog_audio_pulse, "u_accent"),
            accent[0],
            accent[1],
            accent[2],
        );
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
        (gl.Disable)(GL_BLEND);
    }

    unsafe fn render_active_theme(&mut self, gl: &GlFns, st: &mut AppState) {
        self.evict_inactive_renderers(gl, st);
        if st.is_custom_plugin_theme() {
            if self.custom_plugin_renderer.is_none() {
                if let Ok(cpr) = CustomPluginRenderer::new(gl) {
                    self.custom_plugin_renderer = Some(cpr);
                }
            }
            if let Some(cpr) = self.custom_plugin_renderer.as_mut() {
                cpr.render(gl, st);
                return;
            }
        }
        if st.is_custom_glsl_theme() {
            if self.custom_glsl_renderer.is_none() {
                if let Ok(cgr) = CustomGlslRenderer::new(gl) {
                    self.custom_glsl_renderer = Some(cgr);
                }
            }
            if let Some(cgr) = self.custom_glsl_renderer.as_mut() {
                cgr.render(gl, st);
                return;
            }
        }
        if st.is_studio_glass_theme() {
            if self.studio_renderer.is_none() {
                if let Ok(sr) = StudioGlassRenderer::new(gl) {
                    self.studio_renderer = Some(sr);
                }
            }
            if let Some(sr) = self.studio_renderer.as_mut() {
                sr.render(gl, st, &mut self.text);
                return;
            }
        }
        if st.is_quiet_snow_theme() {
            if self.quiet_snow_renderer.is_none() {
                if let Ok(qr) = QuietSnowRenderer::new(gl) {
                    self.quiet_snow_renderer = Some(qr);
                }
            }
            if let Some(qr) = self.quiet_snow_renderer.as_mut() {
                qr.render(gl, st, self.prog_blit);
                return;
            }
        }
        if st.is_voxelcraft_theme() {
            if self.voxelcraft_renderer.is_none() {
                if let Ok(vr) = VoxelcraftRenderer::new(gl) {
                    self.voxelcraft_renderer = Some(vr);
                }
            }
            if let Some(vr) = self.voxelcraft_renderer.as_mut() {
                vr.render(gl, st);
                return;
            }
        }
        if st.is_desert_theme() {
            if self.desert_renderer.is_none() {
                if let Ok(dr) = DesertRenderer::new(gl) {
                    self.desert_renderer = Some(dr);
                }
            }
            if let Some(dr) = self.desert_renderer.as_mut() {
                dr.render(gl, st);
                return;
            }
        }
        if st.is_papercut_theme() {
            if self.papercut_renderer.is_none() {
                if let Ok(pr) = PapercutRenderer::new(gl) {
                    self.papercut_renderer = Some(pr);
                }
            }
            if let Some(pr) = self.papercut_renderer.as_mut() {
                pr.render(gl, st);
                return;
            }
        }
        if st.is_claymorph_theme() {
            if self.claymorph_renderer.is_none() {
                if let Ok(cr) = ClaymorphRenderer::new(gl) {
                    self.claymorph_renderer = Some(cr);
                }
            }
            if let Some(cr) = self.claymorph_renderer.as_mut() {
                cr.render(gl, st);
                return;
            }
        }
        if st.is_neobrutalism_theme() {
            if self.neobrutalism_renderer.is_none() {
                if let Ok(nr) = NeobrutalismRenderer::new(gl) {
                    self.neobrutalism_renderer = Some(nr);
                }
            }
            if let Some(nr) = self.neobrutalism_renderer.as_mut() {
                nr.render(gl, st);
                return;
            }
        }
        if st.is_liquidglass_theme() {
            if self.liquidglass_renderer.is_none() {
                if let Ok(lg) = LiquidGlassRenderer::new(gl) {
                    self.liquidglass_renderer = Some(lg);
                }
            }
            if let Some(lg) = self.liquidglass_renderer.as_mut() {
                lg.render(gl, st);
                return;
            }
        }
        if st.is_zen_archipelago_theme() {
            if self.zen_archipelago_renderer.is_none() {
                if let Ok(za) = crate::zen_archipelago::ZenArchipelagoRenderer::new(gl) {
                    self.zen_archipelago_renderer = Some(za);
                }
            }
            if let Some(za) = self.zen_archipelago_renderer.as_mut() {
                za.render(gl, st);
                return;
            }
        }
        if st.is_software_world_theme() {
            if self.software_world_renderer.is_none() {
                if let Ok(swr) = SoftwareWorldRenderer::new(gl) {
                    self.software_world_renderer = Some(swr);
                }
            }
            if let Some(swr) = self.software_world_renderer.as_mut() {
                swr.render(gl, st, self.prog_blit);
                return;
            }
        }
        if st.is_meadow_theme() {
            if self.meadow_renderer.is_none() {
                if let Ok(mr) = MeadowRenderer::new(gl) {
                    self.meadow_renderer = Some(mr);
                }
            }
            if let Some(mr) = self.meadow_renderer.as_mut() {
                mr.render(gl, st);
                return;
            }
        }
        if st.is_beach_theme() {
            if self.beach_renderer.is_none() {
                if let Ok(br) = BeachRenderer::new(gl) {
                    self.beach_renderer = Some(br);
                }
            }
            if let Some(br) = self.beach_renderer.as_mut() {
                br.render(gl, st);
                return;
            }
        }
        if st.is_aurora_theme() {
            if self.aurora_renderer.is_none() {
                if let Ok(ar) = AuroraRenderer::new(gl) {
                    self.aurora_renderer = Some(ar);
                }
            }
            if let Some(ar) = self.aurora_renderer.as_mut() {
                ar.render(gl, st);
                return;
            }
        }
        if st.is_glade_theme() {
            if self.glade_renderer.is_none() {
                if let Ok(gr) = GladeRenderer::new(gl) {
                    self.glade_renderer = Some(gr);
                }
            }
            if let Some(gr) = self.glade_renderer.as_mut() {
                gr.render(gl, st);
                return;
            }
        }
        if st.is_rift_theme() {
            if self.rift_renderer.is_none() {
                if let Ok(rr) = RiftRenderer::new(gl) {
                    self.rift_renderer = Some(rr);
                }
            }
            if let Some(rr) = self.rift_renderer.as_mut() {
                rr.render(gl, st);
                return;
            }
        }
        if st.is_strike_theme() {
            if self.strike_renderer.is_none() {
                if let Ok(sr) = StrikeRenderer::new(gl) {
                    self.strike_renderer = Some(sr);
                }
            }
            if let Some(sr) = self.strike_renderer.as_mut() {
                sr.render(gl, st);
                return;
            }
        }
        if st.is_pocket_theme() {
            if self.pocket_renderer.is_none() {
                if let Ok(pr) = PocketRenderer::new(gl) {
                    self.pocket_renderer = Some(pr);
                }
            }
            if let Some(pr) = self.pocket_renderer.as_mut() {
                pr.render(gl, st);
                return;
            }
        }
        if st.is_roach_theme() {
            if self.roach_renderer.is_none() {
                if let Ok(rr) = RoachRenderer::new(gl) {
                    self.roach_renderer = Some(rr);
                }
            }
            if let Some(rr) = self.roach_renderer.as_mut() {
                rr.render(gl, st);
                return;
            }
        }
        if st.is_polarnight_theme() {
            if self.polarnight_renderer.is_none() {
                if let Ok(pnr) = PolarNightRenderer::new(gl) {
                    self.polarnight_renderer = Some(pnr);
                }
            }
            if let Some(pnr) = self.polarnight_renderer.as_mut() {
                pnr.render(gl, st);
                return;
            }
        }
        if st.is_rosebrook_theme() {
            if self.rosebrook_renderer.is_none() {
                if let Ok(rr) = RosebrookRenderer::new(gl) {
                    self.rosebrook_renderer = Some(rr);
                }
            }
            if let Some(rr) = self.rosebrook_renderer.as_mut() {
                rr.render(gl, st);
                return;
            }
        }
        if st.is_neumorph_theme() {
            if self.neumorph_renderer.is_none() {
                if let Ok(nr) = NeumorphRenderer::new(gl) {
                    self.neumorph_renderer = Some(nr);
                }
            }
            if let Some(nr) = self.neumorph_renderer.as_mut() {
                nr.render(gl, st, self.prog_blit);
                return;
            }
        }
        if st.is_winter_theme() {
            if self.winter_renderer.is_none() {
                if let Ok(wr) = WinterRenderer::new(gl) {
                    self.winter_renderer = Some(wr);
                }
            }
            if let Some(wr) = self.winter_renderer.as_mut() {
                wr.render(gl, st, self.prog_blit);
                return;
            }
        }
        if st.is_istanbul_theme() {
            if self.istanbul_renderer.is_none() {
                if let Ok(ir) = IstanbulRenderer::new(gl) {
                    self.istanbul_renderer = Some(ir);
                }
            }
            if let Some(ir) = self.istanbul_renderer.as_mut() {
                ir.render(gl, st);
                return;
            }
        }
        if st.is_solar_system_theme() {
            if self.solar_system_renderer.is_none() {
                if let Ok(ssr) = SolarSystemRenderer::new(gl) {
                    self.solar_system_renderer = Some(ssr);
                }
            }
            if let Some(ssr) = self.solar_system_renderer.as_mut() {
                ssr.render(gl, st);
                return;
            }
        }
        if st.is_thinking_orbs_theme() {
            if self.thinking_orbs_renderer.is_none() {
                if let Ok(tor) = ThinkingOrbsRenderer::new(gl) {
                    self.thinking_orbs_renderer = Some(tor);
                }
            }
            if let Some(tor) = self.thinking_orbs_renderer.as_mut() {
                tor.render(gl, st);
                return;
            }
        }
        if st.is_ai_mascots_theme() {
            if self.ai_mascots_renderer.is_none() {
                if let Ok(amr) = AiMascotsRenderer::new(gl) {
                    self.ai_mascots_renderer = Some(amr);
                }
            }
            if let Some(amr) = self.ai_mascots_renderer.as_mut() {
                amr.render(gl, st, &mut self.text);
                return;
            }
        }
        if st.is_cat_shore_theme() {
            if self.cat_shore_renderer.is_none() {
                if let Ok(csr) = CatShoreRenderer::new(gl) {
                    self.cat_shore_renderer = Some(csr);
                }
            }
            if let Some(csr) = self.cat_shore_renderer.as_mut() {
                csr.render(gl, st, &mut self.text);
                return;
            }
        }
        if st.is_persephone_theme() {
            if self.persephone_renderer.is_none() {
                if let Ok(pr) = PersephoneRenderer::new(gl) {
                    self.persephone_renderer = Some(pr);
                }
            }
            if let Some(pr) = self.persephone_renderer.as_mut() {
                pr.render(gl, st, &mut self.text);
                return;
            }
        }
        if st.is_eye_within_words_theme() {
            if self.eye_within_words_renderer.is_none() && !self.eye_within_words_failed {
                match EyeWithinWordsRenderer::new(gl) {
                    Ok(er) => self.eye_within_words_renderer = Some(er),
                    Err(err) => {
                        eprintln!("DiyarDeskGl: eye-within-words init failed: {err}");
                        self.eye_within_words_failed = true;
                    }
                }
            }
            if let Some(er) = self.eye_within_words_renderer.as_mut() {
                er.render(gl, st);
                return;
            }
        }
        if st.is_cosmic_eye_theme() {
            if self.cosmic_eye_renderer.is_none() && !self.cosmic_eye_failed {
                match CosmicEyeRenderer::new(gl) {
                    Ok(cr) => self.cosmic_eye_renderer = Some(cr),
                    Err(err) => {
                        eprintln!("DiyarDeskGl: cosmic-eye init failed: {err}");
                        self.cosmic_eye_failed = true;
                    }
                }
            }
            if let Some(cr) = self.cosmic_eye_renderer.as_mut() {
                cr.render(gl, st);
                return;
            }
        }
        if !st.is_impulse_theme() {
            self.render_shader_theme(gl, st);
            return;
        }
        self.upload_pending_art(gl, st);

        let pw = st.width_px.max(64);
        let ph = st.height_px.max(64);
        let css_w = st.css_w();
        let css_h = st.css_h();
        let dpi = st.dpi_scale;
        let pal = st.palette_colors();
        let s = st.params.widget_scale.clamp(0.75, 1.25);
        let brand = if pal.is_light { &DIYAR_BRAND_LIGHT } else { &DIYAR_BRAND_DARK };
        let soft_bg = if pal.is_light { [0.08, 0.07, 0.13, 0.06] } else { [1.0, 1.0, 1.0, 0.07] };
        let soft_hov = if pal.is_light { [0.08, 0.07, 0.13, 0.12] } else { [1.0, 1.0, 1.0, 0.14] };
        let input_bg = if pal.is_light { [0.06, 0.05, 0.12, 0.05] } else { [0.0, 0.0, 0.0, 0.22] };

        let q_scale = if st.low_mode { 0.75 } else { match st.params.quality { 0 => 0.75, 2 => 1.0, _ => 0.9 } };
        let sw = ((pw as f32 * q_scale).round() as i32).max(64);
        let sh = ((ph as f32 * q_scale).round() as i32).max(64);
        self.scene_fbo.ensure(gl, sw, sh);

        let bw = (sw / 2).max(64);
        let bh = (sh / 2).max(64);
        let do_blur = !st.low_mode && st.params.glass_blur > 0.8;
        if do_blur {
            self.blur_a.ensure(gl, bw, bh);
            self.blur_b.ensure(gl, bw, bh);
        }

        // ============ PASS 1: Backdrop + Adaptive Desktop Icon Cradles ============
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.scene_fbo.fbo);
        (gl.Viewport)(0, 0, sw, sh);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(self.prog_bg);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_bg, "u_res"), css_w, css_h);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "u_time"), st.sim_time);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "u_seed"), st.params.seed as f32);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_bg, "u_light"), if pal.is_light { 1 } else { 0 });
        (gl.Uniform1i)(gl.uniform_loc(self.prog_bg, "u_backdrop"), st.params.backdrop as i32);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_bg0"), pal.bg0[0], pal.bg0[1], pal.bg0[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_bg1"), pal.bg1[0], pal.bg1[1], pal.bg1[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_bg2"), pal.bg2[0], pal.bg2[1], pal.bg2[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_blob0"), pal.blobs[0][0], pal.blobs[0][1], pal.blobs[0][2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_blob1"), pal.blobs[1][0], pal.blobs[1][1], pal.blobs[1][2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_blob2"), pal.blobs[2][0], pal.blobs[2][1], pal.blobs[2][2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_blob3"), pal.blobs[3][0], pal.blobs[3][1], pal.blobs[3][2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_primary"), pal.primary[0], pal.primary[1], pal.primary[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_secondary"), pal.secondary[0], pal.secondary[1], pal.secondary[2]);
        (gl.Uniform3f)(gl.uniform_loc(self.prog_bg, "u_on_surface"), pal.on_surface[0], pal.on_surface[1], pal.on_surface[2]);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_bg, "u_outline"), pal.outline[0], pal.outline[1], pal.outline[2], pal.outline[3]);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_bg, "u_primary_cont"), pal.primary_cont[0], pal.primary_cont[1], pal.primary_cont[2], pal.primary_cont[3]);
        (gl.Uniform2f)(gl.uniform_loc(self.prog_bg, "u_pointer"), st.pointer_x, st.pointer_y);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_bg, "u_pointer_on"), if st.pointer_active && !st.low_mode { 1 } else { 0 });
        let ic_style = if st.params.adaptive { st.params.icon_style as i32 } else { 3 };
        (gl.Uniform1i)(gl.uniform_loc(self.prog_bg, "u_icon_style"), ic_style);
        (gl.Uniform1f)(gl.uniform_loc(self.prog_bg, "u_corner_r"), st.params.corner_radius);
        let ic_cnt = st.effective_icons.len().min(48);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_bg, "u_icon_count"), ic_cnt as i32);
        if ic_cnt > 0 {
            let mut flat = [0.0f32; 48 * 4];
            for (i, ic) in st.effective_icons.iter().take(48).enumerate() {
                flat[i * 4] = ic.x;
                flat[i * 4 + 1] = ic.y;
                flat[i * 4 + 2] = ic.w;
                flat[i * 4 + 3] = ic.h;
            }
            (gl.Uniform4fv)(gl.uniform_loc(self.prog_bg, "u_icons"), ic_cnt as GLsizei, flat.as_ptr());
        }
        (gl.Uniform4f)(gl.uniform_loc(self.prog_bg, "u_cradle"), pal.cradle[0], pal.cradle[1], pal.cradle[2], pal.cradle[3]);
        (gl.Uniform4f)(gl.uniform_loc(self.prog_bg, "u_cradle_stroke"), pal.cradle_stroke[0], pal.cradle_stroke[1], pal.cradle_stroke[2], pal.cradle_stroke[3]);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        // ============ PASS 2: Separable Frosted Glass Blur ============
        let blur_tex = if do_blur {
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.blur_a.fbo);
            (gl.Viewport)(0, 0, bw, bh);
            (gl.UseProgram)(self.prog_blur);
            (gl.ActiveTexture)(GL_TEXTURE0);
            (gl.BindTexture)(GL_TEXTURE_2D, self.scene_fbo.tex);
            (gl.Uniform1i)(gl.uniform_loc(self.prog_blur, "u_tex"), 0);
            (gl.Uniform2f)(gl.uniform_loc(self.prog_blur, "u_dir"), 1.0 / bw as f32, 0.0);
            (gl.Uniform1f)(gl.uniform_loc(self.prog_blur, "u_radius"), st.params.glass_blur);
            (gl.BindVertexArray)(self.dummy_vao);
            (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

            (gl.BindFramebuffer)(GL_FRAMEBUFFER, self.blur_b.fbo);
            (gl.BindTexture)(GL_TEXTURE_2D, self.blur_a.tex);
            (gl.Uniform2f)(gl.uniform_loc(self.prog_blur, "u_dir"), 0.0, 1.0 / bh as f32);
            (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
            self.blur_b.tex
        } else {
            self.scene_fbo.tex
        };

        // ============ PASS 3: Blit Backdrop + Render Material You SDF Widgets ============
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(self.prog_blit);
        (gl.ActiveTexture)(GL_TEXTURE0);
        (gl.BindTexture)(GL_TEXTURE_2D, self.scene_fbo.tex);
        (gl.Uniform1i)(gl.uniform_loc(self.prog_blit, "u_tex"), 0);
        (gl.BindVertexArray)(self.dummy_vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);

        (gl.Enable)(GL_BLEND);
        (gl.BlendFunc)(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        let now = Local::now();
        let h24 = now.hour();
        let min = now.minute();
        let sec = now.second();
        let rad = st.params.corner_radius * s;
        let op = st.params.card_opacity;
        let on_surf = [pal.on_surface[0], pal.on_surface[1], pal.on_surface[2], 1.0];
        let muted = [pal.muted[0], pal.muted[1], pal.muted[2], 1.0];
        let prim = [pal.primary[0], pal.primary[1], pal.primary[2], 1.0];
        let on_prim = [pal.on_primary[0], pal.on_primary[1], pal.on_primary[2], 1.0];
        let show_caret = st.caret_blink < 0.54;
        let dragged_wid = st.drag.map(|d| d.wid);

        // ---------- 1. CLOCK WIDGET (Cookie Hybrid OR DiyarDesk Glance) ----------
        if st.widget_visible(WidgetId::Clock) {
            let rc = st.rect_clock;
            let (hh_disp, suffix) = if st.params.clock_24 {
                (h24, "")
            } else {
                let h12 = if h24 % 12 == 0 { 12 } else { h24 % 12 };
                (h12, if h24 >= 12 { " PM" } else { " AM" })
            };
            let hh_str = format!("{hh_disp:02}");
            let mm_str = format!("{min:02}");

            if st.params.clock_style == 0 {
                // 12-lobed Scalloped Cookie Face + 60 ticks
                let pad = 18.0 * s;
                let expanded = Rect { x: rc.x - pad, y: rc.y - pad, w: rc.w + pad * 2.0, h: rc.h + pad * 2.0 };
                self.draw_ui_prim(
                    gl, css_w, css_h, 2, expanded,
                    [pal.cookie_face[0], pal.cookie_face[1], pal.cookie_face[2], 1.0],
                    [0.0; 4], [0.0; 4], [0.0; 4], blur_tex,
                );

                // Stacked oversized digital HH / MM behind analog hands
                self.text.begin();
                let digit_sz = 62.0 * s;
                let cx = rc.x + rc.w * 0.5;
                let cy = rc.y + rc.h * 0.5;
                self.text.push(gl, &hh_str, cx, cy - 8.0 * s, digit_sz, dpi, true, pal.cookie_digit, 1, None);
                self.text.push(gl, &mm_str, cx, cy + 49.0 * s, digit_sz, dpi, true, pal.cookie_digit, 1, None);
                self.text.flush(gl, css_w, css_h);

                // Expressive analog hands ON TOP of stacked digits
                let h_ang = (((h24 % 12) as f32 + min as f32 / 60.0) / 12.0) * std::f32::consts::TAU;
                let m_ang = ((min as f32 + sec as f32 / 60.0) / 60.0) * std::f32::consts::TAU;
                self.draw_ui_prim(
                    gl, css_w, css_h, 3, rc,
                    [pal.cookie_hour[0], pal.cookie_hour[1], pal.cookie_hour[2], 1.0],
                    [pal.cookie_min[0], pal.cookie_min[1], pal.cookie_min[2], 1.0],
                    [pal.cookie_face[0], pal.cookie_face[1], pal.cookie_face[2], 1.0],
                    [h_ang, m_ang, 0.0, 0.0], blur_tex,
                );

                // Top-left Pentagon Month Badge
                let m_badge = Rect { x: rc.x + 2.0 * s, y: rc.y + 2.0 * s, w: 58.0 * s, h: 58.0 * s };
                self.draw_ui_prim(
                    gl, css_w, css_h, 4, m_badge,
                    [pal.cookie_month[0], pal.cookie_month[1], pal.cookie_month[2], 1.0],
                    [0.0; 4], [0.0; 4], [0.0; 4], blur_tex,
                );

                // Bottom-right Teardrop Day Badge
                let d_badge = Rect { x: rc.x + rc.w - 60.0 * s, y: rc.y + rc.h - 60.0 * s, w: 58.0 * s, h: 58.0 * s };
                self.draw_ui_prim(
                    gl, css_w, css_h, 5, d_badge,
                    [pal.cookie_day[0], pal.cookie_day[1], pal.cookie_day[2], 1.0],
                    [0.0; 4], [0.0; 4], [0.0; 4], blur_tex,
                );

                // Subtle hover-revealed style switch (top-right) & optional layout reset (bottom-left)
                let tog = Rect { x: rc.x + rc.w - 28.0 * s, y: rc.y + 6.0 * s, w: 22.0 * s, h: 22.0 * s };
                st.hit_clock_toggle = tog;
                let tog_a = (st.clock_hover_alpha * 0.85).max(if is_hov(st, tog) { 0.95 } else { 0.14 });
                let tog_bg = if is_hov(st, tog) {
                    [pal.primary[0], pal.primary[1], pal.primary[2], 0.28 * tog_a]
                } else {
                    [pal.surface[0], pal.surface[1], pal.surface[2], 0.72 * tog_a]
                };
                self.draw_ui_prim(
                    gl, css_w, css_h, 1, tog, tog_bg,
                    [pal.outline[0], pal.outline[1], pal.outline[2], pal.outline[3] * tog_a],
                    [0.0; 4], [11.0 * s, 1.0, 0.0, 0.0], blur_tex,
                );
                self.draw_icon(
                    gl, css_w, css_h, 11.0, inset(tog, 5.0 * s),
                    [on_surf[0], on_surf[1], on_surf[2], tog_a], 0.0, blur_tex,
                );

                let has_custom = st.custom_pos.iter().any(|p| p.is_some());
                if has_custom {
                    let rst = Rect { x: rc.x + 6.0 * s, y: rc.y + rc.h - 28.0 * s, w: 22.0 * s, h: 22.0 * s };
                    st.hit_clock_reset = rst;
                    let ra = st.clock_hover_alpha.max(if is_hov(st, rst) { 0.95 } else { 0.0 });
                    if ra > 0.02 {
                        self.draw_ui_prim(
                            gl, css_w, css_h, 1, rst,
                            [pal.surface[0], pal.surface[1], pal.surface[2], 0.74 * ra],
                            [pal.outline[0], pal.outline[1], pal.outline[2], pal.outline[3] * ra],
                            [0.0; 4], [11.0 * s, 1.0, 0.0, 0.0], blur_tex,
                        );
                        self.draw_icon(
                            gl, css_w, css_h, 12.0, inset(rst, 5.0 * s),
                            [on_surf[0], on_surf[1], on_surf[2], ra], 0.0, blur_tex,
                        );
                    }
                } else {
                    st.hit_clock_reset = Rect::default();
                }

                self.text.begin();
                let mon_str = format!("{:02}", now.month());
                let day_str = format!("{:02}", now.day());
                self.text.push(gl, &mon_str, m_badge.x + m_badge.w * 0.5, m_badge.y + m_badge.h * 0.63, 21.0 * s, dpi, true, [0.96, 0.95, 0.99, 1.0], 1, None);
                self.text.push(gl, &day_str, d_badge.x + d_badge.w * 0.5, d_badge.y + d_badge.h * 0.63, 21.0 * s, dpi, true, [0.95, 0.96, 0.98, 1.0], 1, None);
                self.text.flush(gl, css_w, css_h);
            } else {
                // DiyarDesk Glance Card Clock (1:1 Material 3 polish)
                self.draw_glass_card(gl, css_w, css_h, rc, rad, &pal, op, is_hov(st, rc), dragged_wid == Some(WidgetId::Clock), blur_tex);
                self.draw_diyar_dots(gl, css_w, css_h, rc.x + 15.0 * s, rc.y + 16.0 * s, s, pal.is_light, blur_tex);

                let rst = Rect { x: rc.x + rc.w - 35.0 * s, y: rc.y + 10.0 * s, w: 22.0 * s, h: 22.0 * s };
                let tog = Rect { x: rst.x - 26.0 * s, y: rc.y + 10.0 * s, w: 22.0 * s, h: 22.0 * s };
                st.hit_clock_toggle = tog;
                st.hit_clock_reset = rst;
                let tog_bg = if is_hov(st, tog) { soft_hov } else { soft_bg };
                let rst_bg = if is_hov(st, rst) { soft_hov } else { soft_bg };
                self.draw_ui_prim(gl, css_w, css_h, 1, tog, tog_bg, pal.outline, [0.0; 4], [11.0 * s, 1.0, 0.0, 0.0], blur_tex);
                self.draw_ui_prim(gl, css_w, css_h, 1, rst, rst_bg, pal.outline, [0.0; 4], [11.0 * s, 1.0, 0.0, 0.0], blur_tex);
                self.draw_icon(gl, css_w, css_h, 11.0, inset(tog, 5.2 * s), on_surf, 0.0, blur_tex);
                self.draw_icon(gl, css_w, css_h, 12.0, inset(rst, 5.2 * s), muted, 0.0, blur_tex);

                // Seconds pill + Solar phase below it
                let sec_pill = Rect { x: rc.x + rc.w - 64.0 * s, y: rc.y + 44.0 * s, w: 48.0 * s, h: 22.0 * s };
                self.draw_ui_prim(gl, css_w, css_h, 1, sec_pill, pal.primary_cont, [0.0; 4], [0.0; 4], [11.0 * s, 0.0, 0.0, 0.0], blur_tex);

                // Next event pill (two-line Glance pill)
                let gp = Rect { x: rc.x + 14.0 * s, y: rc.y + 114.0 * s, w: rc.w - 28.0 * s, h: 42.0 * s };
                self.draw_ui_prim(gl, css_w, css_h, 1, gp, soft_bg, pal.outline, [0.0; 4], [14.0 * s, 1.0, 0.0, 0.0], blur_tex);
                let (ev_title, ev_sub, ev_col_idx) = next_event_summary(&st.events, &now);
                let dot_c = brand[ev_col_idx % 4];
                self.draw_ui_prim(
                    gl, css_w, css_h, 1,
                    Rect { x: gp.x + 11.0 * s, y: gp.y + (gp.h - 9.0 * s) * 0.5, w: 9.0 * s, h: 9.0 * s },
                    [dot_c[0], dot_c[1], dot_c[2], 1.0], [0.0; 4], [0.0; 4], [4.5 * s, 0.0, 0.0, 0.0], blur_tex,
                );

                // Day progress bar
                let day_frac = ((h24 * 60 + min) as f32 / 1440.0).clamp(0.0, 1.0);
                let bar_bg = Rect { x: rc.x + 44.0 * s, y: rc.y + 171.0 * s, w: rc.w - 92.0 * s, h: 5.0 * s };
                self.draw_ui_prim(gl, css_w, css_h, 1, bar_bg, soft_bg, [0.0; 4], [0.0; 4], [2.5 * s, 0.0, 0.0, 0.0], blur_tex);
                self.draw_ui_prim(
                    gl, css_w, css_h, 1,
                    Rect { x: bar_bg.x, y: bar_bg.y, w: (bar_bg.w * day_frac).max(4.0), h: bar_bg.h },
                    prim, [0.0; 4], [0.0; 4], [2.5 * s, 0.0, 0.0, 0.0], blur_tex,
                );

                let greet = if (5..12).contains(&h24) { "Günaydın" }
                    else if (12..18).contains(&h24) { "İyi günler" }
                    else if (18..22).contains(&h24) { "İyi akşamlar" }
                    else { "İyi geceler" };
                let date_str = format!("{}, {} {}", weekday_tr(now.weekday().num_days_from_sunday()), now.day(), month_tr(now.month()));

                self.text.begin();
                self.text.push(gl, "DiyarDesk Bakış", rc.x + 56.0 * s, rc.y + 23.0 * s, 12.0 * s, dpi, true, on_surf, 0, None);
                self.text.push(gl, greet, tog.x - 8.0 * s, rc.y + 23.0 * s, 10.5 * s, dpi, false, muted, 2, None);
                self.text.push(gl, &format!("{hh_str}:{mm_str}"), rc.x + 15.0 * s, rc.y + 82.0 * s, 46.0 * s, dpi, true, on_surf, 0, None);
                self.text.push(gl, &format!(":{sec:02}{suffix}"), sec_pill.x + sec_pill.w * 0.5, sec_pill.y + 15.0 * s, 11.0 * s, dpi, true, prim, 1, None);
                self.text.push(gl, solar_phase_label(h24), sec_pill.x + sec_pill.w * 0.5, sec_pill.y + 36.0 * s, 10.0 * s, dpi, false, muted, 1, None);
                self.text.push(gl, &date_str, rc.x + 16.0 * s, rc.y + 104.0 * s, 12.5 * s, dpi, true, on_surf, 0, None);
                if ev_sub.is_empty() {
                    self.text.push(gl, &ev_title, gp.x + 28.0 * s, gp.y + 25.0 * s, 11.5 * s, dpi, true, on_surf, 0, Some(gp.w - 36.0 * s));
                } else {
                    self.text.push(gl, &ev_title, gp.x + 28.0 * s, gp.y + 18.0 * s, 11.5 * s, dpi, true, on_surf, 0, Some(gp.w - 36.0 * s));
                    self.text.push(gl, &ev_sub, gp.x + 28.0 * s, gp.y + 33.0 * s, 10.0 * s, dpi, false, muted, 0, Some(gp.w - 36.0 * s));
                }
                self.text.push(gl, "Gün", rc.x + 16.0 * s, rc.y + 177.0 * s, 10.5 * s, dpi, false, muted, 0, None);
                self.text.push(gl, &format!("{}%", (day_frac * 100.0).round() as i32), rc.x + rc.w - 16.0 * s, rc.y + 177.0 * s, 10.5 * s, dpi, false, muted, 2, None);
                self.text.flush(gl, css_w, css_h);
            }
        }

        // ---------- 2. LIVE WAVY MUSIC PLAYER WIDGET ----------
        if st.widget_visible(WidgetId::Media) {
            let rm = st.rect_media;
            self.draw_glass_card(gl, css_w, css_h, rm, rad, &pal, op, is_hov(st, rm), dragged_wid == Some(WidgetId::Media), blur_tex);

            let art_sz = 86.0 * s;
            let art_r = Rect {
                x: rm.x + 13.0 * s,
                y: rm.y + (rm.h - art_sz) * 0.5,
                w: art_sz,
                h: art_sz,
            };
            if st.media.has_art_tex {
                self.draw_ui_prim(
                    gl, css_w, css_h, 9, art_r,
                    [1.0; 4], [0.0; 4], [0.0; 4], [16.0 * s, 0.0, 0.0, 0.0], blur_tex,
                );
            } else {
                self.draw_ui_prim(
                    gl, css_w, css_h, 8, art_r,
                    [pal.primary[0], pal.primary[1], pal.primary[2], 1.0],
                    [pal.tertiary[0], pal.tertiary[1], pal.tertiary[2], 1.0],
                    [brand[0][0], brand[0][1], brand[0][2], 1.0],
                    [16.0 * s, st.media.art_angle, 0.0, 0.0], blur_tex,
                );
            }

            // Smoothly morphing Circle <-> Squircle Play/Pause Button with crisp SDF icon
            let play_sz = 44.0 * s;
            let play_r = Rect {
                x: rm.x + rm.w - 14.0 * s - play_sz,
                y: rm.y + 16.0 * s,
                w: play_sz,
                h: play_sz,
            };
            st.hit_media_play = play_r;
            let is_playing = st.media.active && st.media.playing;
            let morph = st.media.play_morph.clamp(0.0, 1.0);
            let play_rad = play_sz * 0.5 + (14.5 * s - play_sz * 0.5) * morph;
            let mut play_bg = mix4(pal.primary_cont, prim, morph);
            if is_hov(st, play_r) {
                play_bg = lift_col(play_bg, pal.is_light);
            }
            let play_fg = mix4(on_surf, on_prim, morph);
            self.draw_ui_prim(gl, css_w, css_h, 1, play_r, play_bg, [0.0; 4], [0.0; 4], [play_rad, 0.0, 0.0, 0.0], blur_tex);
            self.draw_icon(
                gl, css_w, css_h,
                if is_playing { 1.0 } else { 0.0 },
                inset(play_r, 12.5 * s),
                play_fg, 0.0, blur_tex,
            );

            // Prev / Wavy Seekbar / Next row
            let ctrl_y = rm.y + rm.h - 32.0 * s;
            let left_x = art_r.x + art_r.w + 12.0 * s;
            let prev_r = Rect { x: left_x, y: ctrl_y, w: 22.0 * s, h: 22.0 * s };
            let next_r = Rect { x: rm.x + rm.w - 36.0 * s, y: ctrl_y, w: 22.0 * s, h: 22.0 * s };
            let wave_r = Rect {
                x: prev_r.x + prev_r.w + 6.0 * s,
                y: ctrl_y,
                w: (next_r.x - (prev_r.x + prev_r.w + 12.0 * s)).max(40.0),
                h: 22.0 * s,
            };
            st.hit_media_prev = prev_r;
            st.hit_media_next = next_r;
            st.hit_media_wave = wave_r;

            if is_hov(st, prev_r) {
                self.draw_ui_prim(gl, css_w, css_h, 1, prev_r, soft_hov, [0.0; 4], [0.0; 4], [11.0 * s, 0.0, 0.0, 0.0], blur_tex);
            }
            if is_hov(st, next_r) {
                self.draw_ui_prim(gl, css_w, css_h, 1, next_r, soft_hov, [0.0; 4], [0.0; 4], [11.0 * s, 0.0, 0.0, 0.0], blur_tex);
            }
            self.draw_icon(gl, css_w, css_h, 2.0, inset(prev_r, 4.5 * s), on_surf, 0.0, blur_tex);
            self.draw_icon(gl, css_w, css_h, 3.0, inset(next_r, 4.5 * s), on_surf, 0.0, blur_tex);

            let pos = st.media.current_pos();
            let dur = st.media.dur;
            let frac = if dur > 0.0 { (pos / dur).clamp(0.0, 1.0) as f32 } else { 0.0 };
            self.draw_ui_prim(
                gl, css_w, css_h, 6, wave_r,
                prim, pal.outline, on_surf,
                [frac, st.media.wave_phase, st.media.wave_amp, 0.0],
                blur_tex,
            );

            let max_meta_w = (play_r.x - left_x - 10.0 * s).max(60.0);
            let time_str = format!("{} / {}", fmt_mmss(pos), fmt_mmss(dur));
            self.text.begin();
            self.text.push(gl, &st.media.title, left_x, rm.y + 30.0 * s, 14.0 * s, dpi, true, on_surf, 0, Some(max_meta_w));
            self.text.push(gl, &st.media.artist, left_x, rm.y + 47.0 * s, 11.5 * s, dpi, false, muted, 0, Some(max_meta_w));
            self.text.push(gl, &time_str, left_x, rm.y + 64.0 * s, 10.5 * s, dpi, false, muted, 0, None);
            self.text.flush(gl, css_w, css_h);
        }

        // ---------- 3. DIYARDESK CALENDAR WIDGET ----------
        if st.widget_visible(WidgetId::Calendar) {
            let rcal = st.rect_cal;
            self.draw_glass_card(gl, css_w, css_h, rcal, rad, &pal, op, is_hov(st, rcal), dragged_wid == Some(WidgetId::Calendar), blur_tex);
            self.draw_diyar_dots(gl, css_w, css_h, rcal.x + 15.0 * s, rcal.y + 16.0 * s, s, pal.is_light, blur_tex);

            // Header "+" Add Event Toggle Button
            let add_btn = Rect { x: rcal.x + rcal.w - 36.0 * s, y: rcal.y + 10.0 * s, w: 22.0 * s, h: 22.0 * s };
            st.hit_cal_add_toggle = add_btn;
            let add_bg = if st.ev_form_open {
                prim
            } else if is_hov(st, add_btn) {
                soft_hov
            } else {
                soft_bg
            };
            let add_fg = if st.ev_form_open { on_prim } else { on_surf };
            self.draw_ui_prim(gl, css_w, css_h, 1, add_btn, add_bg, pal.outline, [0.0; 4], [11.0 * s, if st.ev_form_open { 0.0 } else { 1.0 }, 0.0, 0.0], blur_tex);
            self.draw_icon(gl, css_w, css_h, if st.ev_form_open { 8.0 } else { 7.0 }, inset(add_btn, 5.5 * s), add_fg, 0.0, blur_tex);

            // Prev / Next month buttons with crisp SDF chevrons
            let prev_r = Rect { x: rcal.x + 14.0 * s, y: rcal.y + 38.0 * s, w: 22.0 * s, h: 22.0 * s };
            let next_r = Rect { x: rcal.x + rcal.w - 36.0 * s, y: rcal.y + 38.0 * s, w: 22.0 * s, h: 22.0 * s };
            st.hit_cal_prev = prev_r;
            st.hit_cal_next = next_r;
            self.draw_ui_prim(gl, css_w, css_h, 1, prev_r, if is_hov(st, prev_r) { soft_hov } else { soft_bg }, pal.outline, [0.0; 4], [11.0 * s, 1.0, 0.0, 0.0], blur_tex);
            self.draw_ui_prim(gl, css_w, css_h, 1, next_r, if is_hov(st, next_r) { soft_hov } else { soft_bg }, pal.outline, [0.0; 4], [11.0 * s, 1.0, 0.0, 0.0], blur_tex);
            self.draw_icon(gl, css_w, css_h, 9.0, inset(prev_r, 5.5 * s), on_surf, 0.0, blur_tex);
            self.draw_icon(gl, css_w, css_h, 10.0, inset(next_r, 5.5 * s), on_surf, 0.0, blur_tex);

            let first = NaiveDate::from_ymd_opt(st.view_year, st.view_month, 1).unwrap_or_else(|| now.date_naive());
            let start_dow = first.weekday().num_days_from_monday() as usize;
            let days_in_month = days_in_ym(st.view_year, st.view_month);
            let cell_w = (rcal.w - 28.0 * s) / 7.0;
            let cell_h = 24.0 * s;
            let grid_y0 = rcal.y + 84.0 * s;

            st.hit_cal_days.clear();
            let mut day_labels: Vec<(u32, f32, f32, bool, bool)> = Vec::new();
            for d in 1..=days_in_month {
                let slot = start_dow + (d as usize - 1);
                let col = slot % 7;
                let row = slot / 7;
                if row >= 6 { break; }
                let cx = rcal.x + 14.0 * s + col as f32 * cell_w;
                let cy = grid_y0 + row as f32 * cell_h;
                let cell_r = Rect { x: cx, y: cy, w: cell_w, h: cell_h };
                st.hit_cal_days.push((d, cell_r));

                let is_today = st.view_year == now.year() && st.view_month == now.month() && d == now.day();
                let is_sel = d == st.selected_day;
                let pill_r = Rect { x: cx + 3.0 * s, y: cy + 1.0 * s, w: cell_w - 6.0 * s, h: cell_h - 2.0 * s };
                if is_today {
                    self.draw_ui_prim(gl, css_w, css_h, 1, pill_r, prim, [0.0; 4], [0.0; 4], [9.0 * s, 0.0, 0.0, 0.0], blur_tex);
                } else if is_sel {
                    self.draw_ui_prim(gl, css_w, css_h, 1, pill_r, pal.primary_cont, prim, [0.0; 4], [9.0 * s, 1.0, 0.0, 0.0], blur_tex);
                } else if is_hov(st, cell_r) {
                    self.draw_ui_prim(gl, css_w, css_h, 1, pill_r, soft_bg, [0.0; 4], [0.0; 4], [9.0 * s, 0.0, 0.0, 0.0], blur_tex);
                }

                // Small event indicator pip on days with events
                let has_ev = st.events.iter().any(|e| e.year == st.view_year && e.month == st.view_month && e.day == d);
                if has_ev {
                    let pip_c = if is_today { on_prim } else { [brand[0][0], brand[0][1], brand[0][2], 1.0] };
                    let ps = 3.4 * s;
                    self.draw_ui_prim(
                        gl, css_w, css_h, 1,
                        Rect { x: cx + (cell_w - ps) * 0.5, y: cy + cell_h - 4.6 * s, w: ps, h: ps },
                        pip_c, [0.0; 4], [0.0; 4], [ps * 0.5, 0.0, 0.0, 0.0], blur_tex,
                    );
                }

                day_labels.push((d, cx + cell_w * 0.5, cy + 15.0 * s, is_today, is_sel));
            }

            // Divider
            let mut ev_top = rcal.y + 234.0 * s;
            self.draw_ui_prim(
                gl, css_w, css_h, 1,
                Rect { x: rcal.x + 14.0 * s, y: ev_top - 6.0 * s, w: rcal.w - 28.0 * s, h: 1.0 },
                pal.outline, [0.0; 4], [0.0; 4], [0.0; 4], blur_tex,
            );

            // Optional inline Add Event form
            let mut ev_form_rects: Option<(Rect, Rect, Rect)> = None;
            if st.ev_form_open {
                let fh = 28.0 * s;
                let time_r = Rect { x: rcal.x + 14.0 * s, y: ev_top, w: 54.0 * s, h: fh };
                let sub_r = Rect { x: rcal.x + rcal.w - 14.0 * s - fh, y: ev_top, w: fh, h: fh };
                let title_r = Rect {
                    x: time_r.x + time_r.w + 6.0 * s,
                    y: ev_top,
                    w: (sub_r.x - (time_r.x + time_r.w + 12.0 * s)).max(60.0),
                    h: fh,
                };
                st.hit_cal_ev_time = time_r;
                st.hit_cal_ev_title = title_r;
                st.hit_cal_ev_submit = sub_r;

                let time_foc = st.active_field == TextField::EventTime;
                let title_foc = st.active_field == TextField::EventTitle;
                self.draw_ui_prim(gl, css_w, css_h, 1, time_r, input_bg, if time_foc { prim } else { pal.outline }, [0.0; 4], [10.0 * s, if time_foc { 1.5 } else { 1.0 }, 0.0, 0.0], blur_tex);
                self.draw_ui_prim(gl, css_w, css_h, 1, title_r, input_bg, if title_foc { prim } else { pal.outline }, [0.0; 4], [10.0 * s, if title_foc { 1.5 } else { 1.0 }, 0.0, 0.0], blur_tex);
                let sub_bg = if is_hov(st, sub_r) { lift_col(prim, pal.is_light) } else { prim };
                self.draw_ui_prim(gl, css_w, css_h, 1, sub_r, sub_bg, [0.0; 4], [0.0; 4], [fh * 0.5, 0.0, 0.0, 0.0], blur_tex);
                self.draw_icon(gl, css_w, css_h, 5.0, inset(sub_r, 7.0 * s), on_prim, 0.0, blur_tex);

                ev_form_rects = Some((time_r, title_r, sub_r));
                ev_top += 36.0 * s;
            } else {
                st.hit_cal_ev_time = Rect::default();
                st.hit_cal_ev_title = Rect::default();
                st.hit_cal_ev_submit = Rect::default();
            }

            // Events for selected day (with hover delete x button)
            st.hit_cal_ev_dels.clear();
            let day_evs: Vec<(usize, String, String, usize)> = st.events.iter().enumerate()
                .filter(|(_, e)| e.year == st.view_year && e.month == st.view_month && e.day == st.selected_day)
                .take(2)
                .map(|(i, e)| (i, e.title.clone(), e.time.clone(), e.color_idx))
                .collect();

            for (row_i, (orig_idx, _, _, color_idx)) in day_evs.iter().enumerate() {
                let ey = ev_top + (row_i as f32) * 44.0 * s;
                let er = Rect { x: rcal.x + 14.0 * s, y: ey, w: rcal.w - 28.0 * s, h: 38.0 * s };
                let row_hov = is_hov(st, er);
                self.draw_ui_prim(gl, css_w, css_h, 1, er, if row_hov { soft_hov } else { soft_bg }, [0.0; 4], [0.0; 4], [12.0 * s, 0.0, 0.0, 0.0], blur_tex);
                let bc = brand[*color_idx % 4];
                self.draw_ui_prim(
                    gl, css_w, css_h, 1,
                    Rect { x: er.x + 8.0 * s, y: er.y + 7.0 * s, w: 3.5 * s, h: 24.0 * s },
                    [bc[0], bc[1], bc[2], 1.0], [0.0; 4], [0.0; 4], [1.7 * s, 0.0, 0.0, 0.0], blur_tex,
                );
                let del_r = Rect { x: er.x + er.w - 24.0 * s, y: er.y + (er.h - 18.0 * s) * 0.5, w: 18.0 * s, h: 18.0 * s };
                st.hit_cal_ev_dels.push((*orig_idx, del_r));
                let del_col = if is_hov(st, del_r) { on_surf } else { [muted[0], muted[1], muted[2], if row_hov { 0.85 } else { 0.45 }] };
                self.draw_icon(gl, css_w, css_h, 8.0, inset(del_r, 4.5 * s), del_col, 0.0, blur_tex);
            }

            // Caret for calendar event form inputs
            if show_caret {
                if let Some((time_r, title_r, _)) = ev_form_rects {
                    if st.active_field == TextField::EventTime {
                        let tw = self.text.measure(gl, &st.ev_time_input, 10.5 * s, dpi, false);
                        let cx = (time_r.x + 8.0 * s + tw + 1.0).min(time_r.x + time_r.w - 5.0 * s);
                        self.draw_caret(gl, css_w, css_h, cx, time_r.y + 6.0 * s, time_r.h - 12.0 * s, prim, blur_tex);
                    } else if st.active_field == TextField::EventTitle {
                        let tw = self.text.measure(gl, &st.ev_title_input, 10.5 * s, dpi, false);
                        let cx = (title_r.x + 8.0 * s + tw + 1.0).min(title_r.x + title_r.w - 5.0 * s);
                        self.draw_caret(gl, css_w, css_h, cx, title_r.y + 6.0 * s, title_r.h - 12.0 * s, prim, blur_tex);
                    }
                }
            }

            self.text.begin();
            self.text.push(gl, "DiyarDesk Takvim", rcal.x + 56.0 * s, rcal.y + 23.0 * s, 12.0 * s, dpi, true, on_surf, 0, None);
            let m_title = format!("{} {}", month_tr(st.view_month), st.view_year);
            self.text.push(gl, &m_title, rcal.x + rcal.w * 0.5, rcal.y + 54.0 * s, 13.0 * s, dpi, true, on_surf, 1, None);

            let dows = ["Pt", "Sa", "Ça", "Pe", "Cu", "Ct", "Pz"];
            for (i, dw) in dows.iter().enumerate() {
                self.text.push(gl, dw, rcal.x + 14.0 * s + (i as f32 + 0.5) * cell_w, rcal.y + 75.0 * s, 10.0 * s, dpi, true, muted, 1, None);
            }
            for (d, cx, cy, is_today, is_sel) in day_labels {
                let col = if is_today { on_prim } else { on_surf };
                self.text.push(gl, &d.to_string(), cx, cy, 11.0 * s, dpi, is_today || is_sel, col, 1, None);
            }
            if let Some((time_r, title_r, _)) = ev_form_rects {
                let (t_txt, t_col) = if st.ev_time_input.is_empty() { ("14:00", muted) } else { (st.ev_time_input.as_str(), on_surf) };
                let (n_txt, n_col) = if st.ev_title_input.is_empty() { ("Etkinlik adı...", muted) } else { (st.ev_title_input.as_str(), on_surf) };
                self.text.push(gl, t_txt, time_r.x + 8.0 * s, time_r.y + 18.0 * s, 10.5 * s, dpi, false, t_col, 0, Some(time_r.w - 14.0 * s));
                self.text.push(gl, n_txt, title_r.x + 8.0 * s, title_r.y + 18.0 * s, 10.5 * s, dpi, false, n_col, 0, Some(title_r.w - 14.0 * s));
            }
            if day_evs.is_empty() {
                self.text.push(gl, "Seçili gün için etkinlik yok", rcal.x + rcal.w * 0.5, ev_top + 38.0 * s, 11.0 * s, dpi, false, muted, 1, None);
            } else {
                for (row_i, (_, title, time, _)) in day_evs.iter().enumerate() {
                    let ey = ev_top + (row_i as f32) * 44.0 * s;
                    self.text.push(gl, title, rcal.x + 32.0 * s, ey + 16.5 * s, 11.5 * s, dpi, true, on_surf, 0, Some(rcal.w - 76.0 * s));
                    self.text.push(gl, time, rcal.x + 32.0 * s, ey + 30.5 * s, 10.0 * s, dpi, false, muted, 0, None);
                }
            }
            self.text.flush(gl, css_w, css_h);
        }

        // ---------- 4. DIYARDESK TASKS WIDGET ----------
        if st.widget_visible(WidgetId::Tasks) {
            let rtk = st.rect_tasks;
            self.draw_glass_card(gl, css_w, css_h, rtk, rad, &pal, op, is_hov(st, rtk), dragged_wid == Some(WidgetId::Tasks), blur_tex);
            self.draw_diyar_dots(gl, css_w, css_h, rtk.x + 15.0 * s, rtk.y + 16.0 * s, s, pal.is_light, blur_tex);

            let done_cnt = st.tasks.iter().filter(|t| t.done).count();
            let badge_r = Rect { x: rtk.x + rtk.w - 48.0 * s, y: rtk.y + 10.0 * s, w: 34.0 * s, h: 20.0 * s };
            self.draw_ui_prim(gl, css_w, css_h, 1, badge_r, soft_bg, pal.outline, [0.0; 4], [10.0 * s, 1.0, 0.0, 0.0], blur_tex);

            // Filter tab pills: Tümü / Açık / Bitti
            let tab_labels = ["Tümü", "Açık", "Bitti"];
            let tab_w = [48.0 * s, 44.0 * s, 42.0 * s];
            let mut tx = rtk.x + 14.0 * s;
            let ty = rtk.y + 38.0 * s;
            let th = 22.0 * s;
            for i in 0..3 {
                let tr = Rect { x: tx, y: ty, w: tab_w[i], h: th };
                st.hit_task_tabs[i] = tr;
                let active = st.task_filter == i;
                let bg = if active { prim } else if is_hov(st, tr) { soft_hov } else { soft_bg };
                self.draw_ui_prim(gl, css_w, css_h, 1, tr, bg, if active { [0.0; 4] } else { pal.outline }, [0.0; 4], [11.0 * s, if active { 0.0 } else { 1.0 }, 0.0, 0.0], blur_tex);
                tx += tab_w[i] + 6.0 * s;
            }

            // Filtered task rows (max 3 visible)
            st.hit_task_toggles.clear();
            st.hit_task_stars.clear();
            st.hit_task_dels.clear();
            let filtered: Vec<(usize, String, bool, bool)> = st.tasks.iter().enumerate()
                .filter(|(_, tk)| match st.task_filter {
                    1 => !tk.done,
                    2 => tk.done,
                    _ => true,
                })
                .take(3)
                .map(|(i, tk)| (i, tk.text.clone(), tk.done, tk.star))
                .collect();

            let row_y0 = rtk.y + 68.0 * s;
            let row_h = 34.0 * s;
            let mut strike_lines: Vec<(f32, f32, f32)> = Vec::new();
            for (row_i, (orig_idx, text, done, star)) in filtered.iter().enumerate() {
                let ry = row_y0 + (row_i as f32) * (row_h + 6.0 * s);
                let rr = Rect { x: rtk.x + 14.0 * s, y: ry, w: rtk.w - 28.0 * s, h: row_h };
                let row_hov = is_hov(st, rr);
                let base_bg = [soft_bg[0], soft_bg[1], soft_bg[2], if *done { soft_bg[3] * 0.65 } else { soft_bg[3] }];
                self.draw_ui_prim(gl, css_w, css_h, 1, rr, if row_hov { soft_hov } else { base_bg }, [0.0; 4], [0.0; 4], [12.0 * s, 0.0, 0.0, 0.0], blur_tex);

                let del_r = Rect { x: rr.x + rr.w - 22.0 * s, y: rr.y + (rr.h - 18.0 * s) * 0.5, w: 18.0 * s, h: 18.0 * s };
                let star_r = Rect { x: del_r.x - 20.0 * s, y: rr.y + (rr.h - 18.0 * s) * 0.5, w: 18.0 * s, h: 18.0 * s };
                let tog_r = Rect { x: rr.x, y: rr.y, w: (star_r.x - rr.x - 2.0 * s).max(40.0), h: rr.h };
                st.hit_task_toggles.push((*orig_idx, tog_r));
                st.hit_task_stars.push((*orig_idx, star_r));
                st.hit_task_dels.push((*orig_idx, del_r));

                let chk_r = Rect { x: rr.x + 9.0 * s, y: rr.y + (rr.h - 17.0 * s) * 0.5, w: 17.0 * s, h: 17.0 * s };
                let chk_fill = if *done { prim } else { [0.0; 4] };
                self.draw_ui_prim(gl, css_w, css_h, 1, chk_r, chk_fill, prim, [0.0; 4], [8.5 * s, 1.5, 0.0, 0.0], blur_tex);
                if *done {
                    self.draw_icon(gl, css_w, css_h, 5.0, inset(chk_r, 3.6 * s), on_prim, 0.0, blur_tex);
                    let max_tw = (star_r.x - (rr.x + 34.0 * s) - 6.0 * s).max(30.0);
                    let tw = self.text.measure(gl, text, 11.5 * s, dpi, false).min(max_tw);
                    strike_lines.push((rr.x + 34.0 * s, ry + 17.0 * s, tw));
                }

                let star_col = if *star {
                    [brand[1][0], brand[1][1], brand[1][2], 1.0]
                } else if is_hov(st, star_r) {
                    on_surf
                } else {
                    [muted[0], muted[1], muted[2], 0.55]
                };
                self.draw_icon(gl, css_w, css_h, 6.0, inset(star_r, 2.5 * s), star_col, if *star { 1.0 } else { 0.0 }, blur_tex);

                let del_col = if is_hov(st, del_r) { on_surf } else { [muted[0], muted[1], muted[2], if row_hov { 0.85 } else { 0.42 }] };
                self.draw_icon(gl, css_w, css_h, 8.0, inset(del_r, 4.5 * s), del_col, 0.0, blur_tex);
            }

            // Bottom Inline "+ Görev ekle..." input + "+" submit circle
            let inp_h = 28.0 * s;
            let inp_y = rtk.y + rtk.h - inp_h - 12.0 * s;
            let sub_r = Rect { x: rtk.x + rtk.w - 14.0 * s - inp_h, y: inp_y, w: inp_h, h: inp_h };
            let inp_r = Rect { x: rtk.x + 14.0 * s, y: inp_y, w: (sub_r.x - (rtk.x + 14.0 * s) - 7.0 * s).max(80.0), h: inp_h };
            st.hit_task_input = inp_r;
            st.hit_task_submit = sub_r;

            let tk_foc = st.active_field == TextField::TaskNew;
            self.draw_ui_prim(gl, css_w, css_h, 1, inp_r, input_bg, if tk_foc { prim } else { pal.outline }, [0.0; 4], [14.0 * s, if tk_foc { 1.5 } else { 1.0 }, 0.0, 0.0], blur_tex);
            let sub_bg = if is_hov(st, sub_r) { lift_col(prim, pal.is_light) } else { prim };
            self.draw_ui_prim(gl, css_w, css_h, 1, sub_r, sub_bg, [0.0; 4], [0.0; 4], [inp_h * 0.5, 0.0, 0.0, 0.0], blur_tex);
            self.draw_icon(gl, css_w, css_h, 7.0, inset(sub_r, 7.2 * s), on_prim, 0.0, blur_tex);

            if show_caret && tk_foc {
                let tw = self.text.measure(gl, &st.task_input, 11.0 * s, dpi, false);
                let cx = (inp_r.x + 12.0 * s + tw + 1.0).min(inp_r.x + inp_r.w - 8.0 * s);
                self.draw_caret(gl, css_w, css_h, cx, inp_r.y + 6.0 * s, inp_r.h - 12.0 * s, prim, blur_tex);
            }

            self.text.begin();
            self.text.push(gl, "DiyarDesk Görevler", rtk.x + 56.0 * s, rtk.y + 23.0 * s, 12.0 * s, dpi, true, on_surf, 0, None);
            self.text.push(gl, &format!("{done_cnt}/{}", st.tasks.len()), badge_r.x + badge_r.w * 0.5, badge_r.y + 14.0 * s, 10.5 * s, dpi, true, on_surf, 1, None);
            for i in 0..3 {
                let tr = st.hit_task_tabs[i];
                let tc = if st.task_filter == i { on_prim } else { muted };
                self.text.push(gl, tab_labels[i], tr.x + tr.w * 0.5, tr.y + 14.5 * s, 10.5 * s, dpi, st.task_filter == i, tc, 1, None);
            }
            if filtered.is_empty() {
                self.text.push(gl, "Bu listede görev yok", rtk.x + rtk.w * 0.5, row_y0 + 48.0 * s, 11.0 * s, dpi, false, muted, 1, None);
            } else {
                for (row_i, (_, text, done, _)) in filtered.iter().enumerate() {
                    let ry = row_y0 + (row_i as f32) * (row_h + 6.0 * s);
                    let tc = if *done { muted } else { on_surf };
                    self.text.push(gl, text, rtk.x + 48.0 * s, ry + 21.0 * s, 11.5 * s, dpi, false, tc, 0, Some(rtk.w - 110.0 * s));
                }
            }
            let (in_txt, in_col) = if st.task_input.is_empty() { ("+ Görev ekle...", muted) } else { (st.task_input.as_str(), on_surf) };
            self.text.push(gl, in_txt, inp_r.x + 12.0 * s, inp_r.y + 18.0 * s, 11.0 * s, dpi, false, in_col, 0, Some(inp_r.w - 20.0 * s));
            self.text.flush(gl, css_w, css_h);

            // Subtle strikethrough lines on completed tasks
            for (sx, sy, sw) in strike_lines {
                self.draw_ui_prim(
                    gl, css_w, css_h, 1,
                    Rect { x: sx, y: sy, w: sw, h: 1.0 },
                    [muted[0], muted[1], muted[2], 0.72], [0.0; 4], [0.0; 4], [0.0; 4], blur_tex,
                );
            }
        }

        // ---------- 5. DIYARDESK TOOLS (3 TABS: Komut / Notlar / Odak) ----------
        if st.widget_visible(WidgetId::Tools) {
            let rtl = st.rect_tools;
            self.draw_glass_card(gl, css_w, css_h, rtl, rad, &pal, op, is_hov(st, rtl), dragged_wid == Some(WidgetId::Tools), blur_tex);
            self.draw_diyar_dots(gl, css_w, css_h, rtl.x + 15.0 * s, rtl.y + 16.0 * s, s, pal.is_light, blur_tex);

            // Header 3 Tab Pills: Komut / Notlar / Odak (aligned right, 1:1 WebView2)
            let tool_labels = ["Komut", "Notlar", "Odak"];
            let tws = [52.0 * s, 52.0 * s, 46.0 * s];
            let total_tw = tws[0] + tws[1] + tws[2] + 10.0 * s;
            let mut tx = rtl.x + rtl.w - 14.0 * s - total_tw;
            let ty = rtl.y + 10.0 * s;
            let th = 22.0 * s;
            for i in 0..3 {
                let tr = Rect { x: tx, y: ty, w: tws[i], h: th };
                st.hit_tool_tabs[i] = tr;
                let active = st.tools_tab == i;
                let bg = if active { prim } else if is_hov(st, tr) { soft_hov } else { soft_bg };
                self.draw_ui_prim(gl, css_w, css_h, 1, tr, bg, if active { [0.0; 4] } else { pal.outline }, [0.0; 4], [11.0 * s, if active { 0.0 } else { 1.0 }, 0.0, 0.0], blur_tex);
                tx += tws[i] + 5.0 * s;
            }

            // Reset inactive tab hit rects
            st.hit_cmd_input = Rect::default();
            st.hit_cmd_apply = Rect::default();
            st.hit_cmd_hints = [Rect::default(); 4];
            st.hit_note_area = Rect::default();
            st.hit_note_to_task = Rect::default();
            st.hit_pomo_toggle = Rect::default();
            st.hit_pomo_reset = Rect::default();
            st.hit_pomo_mode = Rect::default();

            let body_y = rtl.y + 40.0 * s;

            if st.tools_tab == 0 {
                // ---- TAB 0: KOMUT (Calculator, Unit Converter, /todo, Offline Search) ----
                let cmd_r = Rect { x: rtl.x + 14.0 * s, y: body_y, w: rtl.w - 28.0 * s, h: 32.0 * s };
                st.hit_cmd_input = cmd_r;
                let cmd_foc = st.active_field == TextField::Cmd;
                self.draw_ui_prim(gl, css_w, css_h, 1, cmd_r, input_bg, if cmd_foc { prim } else { pal.outline }, [0.0; 4], [16.0 * s, if cmd_foc { 1.5 } else { 1.0 }, 0.0, 0.0], blur_tex);
                self.draw_icon(
                    gl, css_w, css_h, 4.0,
                    Rect { x: cmd_r.x + 10.0 * s, y: cmd_r.y + 8.5 * s, w: 15.0 * s, h: 15.0 * s },
                    if cmd_foc { prim } else { muted }, 0.0, blur_tex,
                );

                if show_caret && cmd_foc {
                    let tw = self.text.measure(gl, &st.cmd_input, 11.5 * s, dpi, false);
                    let cx = (cmd_r.x + 31.0 * s + tw + 1.0).min(cmd_r.x + cmd_r.w - 10.0 * s);
                    self.draw_caret(gl, css_w, css_h, cx, cmd_r.y + 7.5 * s, cmd_r.h - 15.0 * s, prim, blur_tex);
                }

                let mut next_y = cmd_r.y + cmd_r.h + 8.0 * s;
                let has_res = !st.cmd_result.is_empty();
                let mut res_r = Rect::default();
                let mut app_r = Rect::default();
                if has_res {
                    res_r = Rect { x: rtl.x + 14.0 * s, y: next_y, w: rtl.w - 28.0 * s, h: 24.0 * s };
                    self.draw_ui_prim(gl, css_w, css_h, 1, res_r, pal.primary_cont, [0.0; 4], [0.0; 4], [12.0 * s, 0.0, 0.0, 0.0], blur_tex);
                    if st.cmd_has_apply {
                        app_r = Rect { x: res_r.x + res_r.w - 56.0 * s, y: res_r.y + 2.5 * s, w: 52.0 * s, h: 19.0 * s };
                        st.hit_cmd_apply = app_r;
                        let abg = if is_hov(st, app_r) { lift_col(prim, pal.is_light) } else { prim };
                        self.draw_ui_prim(gl, css_w, css_h, 1, app_r, abg, [0.0; 4], [0.0; 4], [9.5 * s, 0.0, 0.0, 0.0], blur_tex);
                    }
                    next_y += 30.0 * s;
                }

                // 4 Quick hint chips
                let hint_labels = ["/todo Görev", "1920/16*9", "22°C -> °F", "10 km -> mi"];
                let chip_gap = 6.0 * s;
                let chip_w = (rtl.w - 28.0 * s - chip_gap * 3.0) / 4.0;
                for i in 0..4 {
                    let hr = Rect { x: rtl.x + 14.0 * s + (i as f32) * (chip_w + chip_gap), y: next_y, w: chip_w, h: 24.0 * s };
                    st.hit_cmd_hints[i] = hr;
                    self.draw_ui_prim(gl, css_w, css_h, 1, hr, if is_hov(st, hr) { soft_hov } else { soft_bg }, pal.outline, [0.0; 4], [12.0 * s, 1.0, 0.0, 0.0], blur_tex);
                }

                self.text.begin();
                self.text.push(gl, "DiyarDesk Araçlar", rtl.x + 56.0 * s, rtl.y + 23.0 * s, 12.0 * s, dpi, true, on_surf, 0, None);
                for i in 0..3 {
                    let tr = st.hit_tool_tabs[i];
                    self.text.push(gl, tool_labels[i], tr.x + tr.w * 0.5, tr.y + 14.5 * s, 10.5 * s, dpi, st.tools_tab == i, if st.tools_tab == i { on_prim } else { muted }, 1, None);
                }
                let (ct, cc) = if st.cmd_input.is_empty() {
                    ("Hesapla, birim çevir veya /todo yaz...", muted)
                } else {
                    (st.cmd_input.as_str(), on_surf)
                };
                self.text.push(gl, ct, cmd_r.x + 31.0 * s, cmd_r.y + 20.0 * s, 11.5 * s, dpi, false, cc, 0, Some(cmd_r.w - 42.0 * s));
                if has_res {
                    let max_rw = if st.cmd_has_apply { res_r.w - 76.0 * s } else { res_r.w - 24.0 * s };
                    self.text.push(gl, &st.cmd_result, res_r.x + 12.0 * s, res_r.y + 15.5 * s, 11.0 * s, dpi, true, on_surf, 0, Some(max_rw));
                    if st.cmd_has_apply {
                        self.text.push(gl, "Uygula", app_r.x + app_r.w * 0.5, app_r.y + 13.0 * s, 9.5 * s, dpi, true, on_prim, 1, None);
                    }
                }
                for i in 0..4 {
                    let hr = st.hit_cmd_hints[i];
                    self.text.push(gl, hint_labels[i], hr.x + hr.w * 0.5, hr.y + 15.5 * s, 9.8 * s, dpi, false, muted, 1, Some(hr.w - 8.0 * s));
                }
                self.text.flush(gl, css_w, css_h);
            } else if st.tools_tab == 1 {
                // ---- TAB 1: NOTLAR (Keep-style offline scratchpad + Göreve aktar) ----
                let note_r = Rect { x: rtl.x + 14.0 * s, y: body_y, w: rtl.w - 28.0 * s, h: 68.0 * s };
                st.hit_note_area = note_r;
                let note_foc = st.active_field == TextField::Note;
                self.draw_ui_prim(gl, css_w, css_h, 1, note_r, input_bg, if note_foc { prim } else { pal.outline }, [0.0; 4], [14.0 * s, if note_foc { 1.5 } else { 1.0 }, 0.0, 0.0], blur_tex);

                let to_tk = Rect { x: rtl.x + rtl.w - 14.0 * s - 92.0 * s, y: note_r.y + note_r.h + 7.0 * s, w: 92.0 * s, h: 22.0 * s };
                st.hit_note_to_task = to_tk;
                self.draw_ui_prim(gl, css_w, css_h, 1, to_tk, if is_hov(st, to_tk) { soft_hov } else { soft_bg }, pal.outline, [0.0; 4], [11.0 * s, 1.0, 0.0, 0.0], blur_tex);

                let lines = wrap_note_lines(&mut self.text, gl, &st.note_text, note_r.w - 22.0 * s, 11.0 * s, dpi, 4);
                if show_caret && note_foc {
                    let line_idx = lines.len().saturating_sub(1);
                    let last_line = lines.last().map(|l| l.as_str()).unwrap_or("");
                    let tw = self.text.measure(gl, last_line, 11.0 * s, dpi, false);
                    let cx = (note_r.x + 11.0 * s + tw + 1.0).min(note_r.x + note_r.w - 8.0 * s);
                    let cy = note_r.y + 8.0 * s + (line_idx as f32) * 14.5 * s;
                    self.draw_caret(gl, css_w, css_h, cx, cy, 13.0 * s, prim, blur_tex);
                }

                self.text.begin();
                self.text.push(gl, "DiyarDesk Araçlar", rtl.x + 56.0 * s, rtl.y + 23.0 * s, 12.0 * s, dpi, true, on_surf, 0, None);
                for i in 0..3 {
                    let tr = st.hit_tool_tabs[i];
                    self.text.push(gl, tool_labels[i], tr.x + tr.w * 0.5, tr.y + 14.5 * s, 10.5 * s, dpi, st.tools_tab == i, if st.tools_tab == i { on_prim } else { muted }, 1, None);
                }
                if st.note_text.is_empty() {
                    self.text.push(gl, "Hızlı not al... (otomatik yerel kayıt)", note_r.x + 11.0 * s, note_r.y + 20.0 * s, 11.0 * s, dpi, false, muted, 0, Some(note_r.w - 22.0 * s));
                } else {
                    for (li, line) in lines.iter().enumerate() {
                        self.text.push(gl, line, note_r.x + 11.0 * s, note_r.y + 18.5 * s + (li as f32) * 14.5 * s, 11.0 * s, dpi, false, on_surf, 0, Some(note_r.w - 22.0 * s));
                    }
                }
                let char_cnt = st.note_text.chars().count();
                self.text.push(gl, &format!("{char_cnt} karakter · çevrimdışı"), rtl.x + 16.0 * s, to_tk.y + 14.5 * s, 10.0 * s, dpi, false, muted, 0, None);
                self.text.push(gl, "Göreve aktar", to_tk.x + to_tk.w * 0.5, to_tk.y + 14.5 * s, 10.0 * s, dpi, true, on_surf, 1, None);
                self.text.flush(gl, css_w, css_h);
            } else {
                // ---- TAB 2: ODAK (Pomodoro + CPU / RAM / PİL DiyarDesk Brand Rings) ----
                let pomo_r = Rect { x: rtl.x + 14.0 * s, y: body_y, w: 170.0 * s, h: 72.0 * s };
                self.draw_ui_prim(gl, css_w, css_h, 1, pomo_r, soft_bg, pal.outline, [0.0; 4], [16.0 * s, 1.0, 0.0, 0.0], blur_tex);

                let btn_y = pomo_r.y + 43.0 * s;
                let btn_h = 20.0 * s;
                let b_tog = Rect { x: pomo_r.x + 8.0 * s, y: btn_y, w: 54.0 * s, h: btn_h };
                let b_rst = Rect { x: b_tog.x + b_tog.w + 5.0 * s, y: btn_y, w: 46.0 * s, h: btn_h };
                let b_mod = Rect { x: b_rst.x + b_rst.w + 5.0 * s, y: btn_y, w: 44.0 * s, h: btn_h };
                st.hit_pomo_toggle = b_tog;
                st.hit_pomo_reset = b_rst;
                st.hit_pomo_mode = b_mod;
                self.draw_ui_prim(gl, css_w, css_h, 1, b_tog, if is_hov(st, b_tog) { lift_col(prim, pal.is_light) } else { prim }, [0.0; 4], [0.0; 4], [10.0 * s, 0.0, 0.0, 0.0], blur_tex);
                self.draw_ui_prim(gl, css_w, css_h, 1, b_rst, if is_hov(st, b_rst) { soft_hov } else { soft_bg }, pal.outline, [0.0; 4], [10.0 * s, 1.0, 0.0, 0.0], blur_tex);
                self.draw_ui_prim(gl, css_w, css_h, 1, b_mod, if is_hov(st, b_mod) { soft_hov } else { soft_bg }, pal.outline, [0.0; 4], [10.0 * s, 1.0, 0.0, 0.0], blur_tex);

                let rings_x0 = pomo_r.x + pomo_r.w + 10.0 * s;
                let cell_w = ((rtl.x + rtl.w - 14.0 * s) - rings_x0 - 12.0 * s) / 3.0;
                let metrics = [
                    ("CPU", st.cpu, brand[0]),
                    ("RAM", st.mem, brand[1]),
                    ("PİL", st.battery, brand[2]),
                ];
                for (i, &(_, pct, col)) in metrics.iter().enumerate() {
                    let cx = rings_x0 + (i as f32) * (cell_w + 6.0 * s);
                    let cr = Rect { x: cx, y: pomo_r.y, w: cell_w, h: pomo_r.h };
                    self.draw_ui_prim(gl, css_w, css_h, 1, cr, soft_bg, [0.0; 4], [0.0; 4], [14.0 * s, 0.0, 0.0, 0.0], blur_tex);
                    let ring_sz = 42.0 * s;
                    let rr = Rect { x: cr.x + (cr.w - ring_sz) * 0.5, y: cr.y + 6.0 * s, w: ring_sz, h: ring_sz };
                    self.draw_ui_prim(
                        gl, css_w, css_h, 7, rr,
                        [col[0], col[1], col[2], 1.0],
                        pal.outline, [0.0; 4], [pct / 100.0, 0.0, 0.0, 0.0], blur_tex,
                    );
                }

                let pm = (st.pomo_left_sec.max(0.0) as i32) / 60;
                let ps = (st.pomo_left_sec.max(0.0) as i32) % 60;
                self.text.begin();
                self.text.push(gl, "DiyarDesk Araçlar", rtl.x + 56.0 * s, rtl.y + 23.0 * s, 12.0 * s, dpi, true, on_surf, 0, None);
                for i in 0..3 {
                    let tr = st.hit_tool_tabs[i];
                    self.text.push(gl, tool_labels[i], tr.x + tr.w * 0.5, tr.y + 14.5 * s, 10.5 * s, dpi, st.tools_tab == i, if st.tools_tab == i { on_prim } else { muted }, 1, None);
                }
                self.text.push(gl, if st.pomo_break { "Mola · 5 dk" } else { "Odak · 25 dk" }, pomo_r.x + 10.0 * s, pomo_r.y + 27.0 * s, 10.5 * s, dpi, false, muted, 0, None);
                self.text.push(gl, &format!("{pm:02}:{ps:02}"), pomo_r.x + pomo_r.w - 10.0 * s, pomo_r.y + 30.0 * s, 22.0 * s, dpi, true, on_surf, 2, None);
                self.text.push(gl, if st.pomo_running { "Durdur" } else { "Başlat" }, b_tog.x + b_tog.w * 0.5, b_tog.y + 13.5 * s, 10.0 * s, dpi, true, on_prim, 1, None);
                self.text.push(gl, "Sıfırla", b_rst.x + b_rst.w * 0.5, b_rst.y + 13.5 * s, 10.0 * s, dpi, true, on_surf, 1, None);
                self.text.push(gl, if st.pomo_break { "Odak" } else { "Mola" }, b_mod.x + b_mod.w * 0.5, b_mod.y + 13.5 * s, 10.0 * s, dpi, true, on_surf, 1, None);

                for (i, &(lbl, pct, _)) in metrics.iter().enumerate() {
                    let cx = rings_x0 + (i as f32) * (cell_w + 6.0 * s);
                    self.text.push(gl, &format!("{}%", pct.round() as i32), cx + cell_w * 0.5, pomo_r.y + 31.0 * s, 10.0 * s, dpi, true, on_surf, 1, None);
                    self.text.push(gl, lbl, cx + cell_w * 0.5, pomo_r.y + 62.0 * s, 9.5 * s, dpi, true, muted, 1, None);
                }
                self.text.flush(gl, css_w, css_h);
            }
        }
    }
}

fn is_hov(st: &AppState, r: Rect) -> bool {
    st.pointer_active && r.contains(st.pointer_x, st.pointer_y)
}

fn inset(r: Rect, p: f32) -> Rect {
    Rect {
        x: r.x + p,
        y: r.y + p,
        w: (r.w - p * 2.0).max(2.0),
        h: (r.h - p * 2.0).max(2.0),
    }
}

fn mix4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn lift_col(c: [f32; 4], light: bool) -> [f32; 4] {
    let target = if light { [0.0, 0.0, 0.0, c[3]] } else { [1.0, 1.0, 1.0, c[3]] };
    mix4(c, target, 0.12)
}

fn wrap_note_lines(
    tr: &mut TextRenderer,
    gl: &GlFns,
    text: &str,
    max_w: f32,
    css_size: f32,
    dpi: f32,
    max_lines: usize,
) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines: Vec<String> = Vec::new();
    for raw_line in text.split('\n') {
        let mut cur = String::new();
        for ch in raw_line.chars() {
            let mut cand = cur.clone();
            cand.push(ch);
            if !cur.is_empty() && tr.measure(gl, &cand, css_size, dpi, false) > max_w {
                lines.push(cur);
                cur = ch.to_string();
                if lines.len() >= max_lines {
                    break;
                }
            } else {
                cur = cand;
            }
        }
        if lines.len() < max_lines {
            lines.push(cur);
        }
        if lines.len() >= max_lines {
            break;
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn solar_phase_label(h24: u32) -> &'static str {
    if (5..=6).contains(&h24) { "Şafak" }
    else if (7..=17).contains(&h24) { "Gündüz" }
    else if (18..=19).contains(&h24) { "Gün batımı" }
    else { "Gece" }
}

fn parse_hhmm(s: &str) -> Option<i32> {
    let mut it = s.trim().split(':');
    let h: i32 = it.next()?.parse().ok()?;
    let m: i32 = it.next()?.parse().ok()?;
    if (0..24).contains(&h) && (0..60).contains(&m) {
        Some(h * 60 + m)
    } else {
        None
    }
}

fn next_event_summary(events: &[CalEvent], now: &DateTime<Local>) -> (String, String, usize) {
    let y = now.year();
    let m = now.month();
    let d = now.day();
    let now_min = (now.hour() * 60 + now.minute()) as i32;

    let mut best_upcoming: Option<(i32, &CalEvent)> = None;
    let mut first_today: Option<&CalEvent> = None;
    for ev in events {
        if ev.year == y && ev.month == m && ev.day == d {
            if first_today.is_none() {
                first_today = Some(ev);
            }
            if let Some(em) = parse_hhmm(&ev.time) {
                let diff = em - now_min;
                if diff >= 0 {
                    if best_upcoming.map(|(bd, _)| diff < bd).unwrap_or(true) {
                        best_upcoming = Some((diff, ev));
                    }
                }
            }
        }
    }

    if let Some((diff, ev)) = best_upcoming {
        let sub = if diff == 0 {
            format!("{} · Şimdi", ev.time)
        } else if diff < 60 {
            format!("{} · {} dk sonra", ev.time, diff)
        } else {
            let h = diff / 60;
            let rm = diff % 60;
            if rm == 0 {
                format!("{} · {} sa sonra", ev.time, h)
            } else {
                format!("{} · {} sa {} dk sonra", ev.time, h, rm)
            }
        };
        return (ev.title.clone(), sub, ev.color_idx);
    }
    if let Some(ev) = first_today {
        return (ev.title.clone(), format!("Bugün · {}", ev.time), ev.color_idx);
    }
    if let Some(ev) = events.first() {
        return (ev.title.clone(), format!("{} {} · {}", ev.day, month_tr(ev.month), ev.time), ev.color_idx);
    }
    ("Yaklaşan etkinlik yok".into(), String::new(), 0)
}

fn fmt_mmss(sec: f64) -> String {
    let s = sec.max(0.0).floor() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn days_in_ym(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let start = NaiveDate::from_ymd_opt(year, month, 1);
    let end = NaiveDate::from_ymd_opt(ny, nm, 1);
    match (start, end) {
        (Some(a), Some(b)) => (b - a).num_days().clamp(28, 31) as u32,
        _ => 30,
    }
}

fn month_tr(m: u32) -> &'static str {
    match m {
        1 => "Ocak", 2 => "Şubat", 3 => "Mart", 4 => "Nisan",
        5 => "Mayıs", 6 => "Haziran", 7 => "Temmuz", 8 => "Ağustos",
        9 => "Eylül", 10 => "Ekim", 11 => "Kasım", 12 => "Aralık",
        _ => "",
    }
}

fn weekday_tr(sun0: u32) -> &'static str {
    match sun0 {
        0 => "Pazar", 1 => "Pazartesi", 2 => "Salı", 3 => "Çarşamba",
        4 => "Perşembe", 5 => "Cuma", 6 => "Cumartesi",
        _ => "",
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::gl::WglContext;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExA, DefWindowProcA, DestroyWindow, RegisterClassA, WNDCLASSA, WS_POPUP,
    };

    #[test]
    fn evicts_inactive_theme_renderers_on_theme_switch() {
        unsafe {
            let hinst = GetModuleHandleA(std::ptr::null());
            let cls = b"DiyarGlEvictionTest\0";
            let wc = WNDCLASSA {
                style: 0x0020,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassA(&wc);
            let hwnd = CreateWindowExA(
                0, cls.as_ptr(), b"evict\0".as_ptr(), WS_POPUP,
                0, 0, 128, 128, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null(),
            );
            assert!(!hwnd.is_null(), "Failed to create dummy test HWND");
            let ctx = WglContext::create(hwnd).expect("WGL context failed");
            assert!(ctx.make_current());
            let gl = GlFns::load().expect("OpenGL load failed");

            let mut renderer = Renderer::new(&gl).expect("Renderer::new failed");
            let mut st = AppState::new(128, 128, 1.0);

            // 1. Render Papercut
            st.theme_id = "papercut".to_string();
            renderer.render(&gl, &mut st);
            assert_eq!(renderer.loaded_renderer_count(), 1);
            assert!(renderer.papercut_renderer.is_some());

            // 2. Switch to Desert -> Papercut must be evicted
            st.theme_id = "desert".to_string();
            renderer.render(&gl, &mut st);
            assert_eq!(renderer.loaded_renderer_count(), 1);
            assert!(renderer.papercut_renderer.is_none());
            assert!(renderer.desert_renderer.is_some());

            // 3. Switch to Impulse -> Desert evicted, scene_fbo allocated
            st.theme_id = "material-impulse-gl".to_string();
            renderer.render(&gl, &mut st);
            assert_eq!(renderer.loaded_renderer_count(), 0);
            assert!(renderer.desert_renderer.is_none());
            assert_ne!(renderer.scene_fbo.fbo, 0);

            // 4. Switch back to Papercut -> Impulse FBOs destroyed
            st.theme_id = "papercut".to_string();
            renderer.render(&gl, &mut st);
            assert_eq!(renderer.loaded_renderer_count(), 1);
            assert!(renderer.papercut_renderer.is_some());
            assert_eq!(renderer.scene_fbo.fbo, 0);
            assert_eq!(renderer.blur_a.fbo, 0);
            assert_eq!(renderer.blur_b.fbo, 0);

            // 5. Audio pulse shader compiled & renders cleanly when audio is active
            assert_ne!(renderer.prog_audio_pulse, 0);
            st.audio.push_peak(0.9);
            st.audio.step(0.033);
            assert!(st.audio.is_active());
            renderer.render(&gl, &mut st);
            assert_eq!((gl.GetError)(), 0);

            renderer.destroy(&gl);
            drop(ctx);
            DestroyWindow(hwnd);
        }
    }
}
