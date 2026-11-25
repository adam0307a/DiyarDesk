//! Dynamic C-ABI loader for user-imported Rust + OpenGL 3.3 `cdylib` theme plugins
//! (`engine: "opengl-rust"`).
//!
//! ABI version 1 exports (implemented by external Rust `cdylib` crates):
//! - `extern "C" fn ddgl_plugin_abi_version() -> u32` (must return `1`)
//! - `extern "C" fn ddgl_plugin_create(gl_loader, width_px, height_px, dpr, assets_dir_utf8) -> *mut c_void`
//! - `extern "C" fn ddgl_plugin_render(ctx, dt)`
//! - `extern "C" fn ddgl_plugin_destroy(ctx)`
//! Optional exports:
//! - `extern "C" fn ddgl_plugin_resize(ctx, width_px, height_px, dpr)`
//! - `extern "C" fn ddgl_plugin_post_json(ctx, json_utf8)`

use crate::custom_glsl::{resolve_inside_dir, CustomGlSpec};
use crate::gl::*;
use crate::shaders::FULLSCREEN_VERT;
use crate::state::AppState;
use std::ffi::{c_char, c_void, CString};
use std::path::Path;

pub const DDGL_PLUGIN_ABI_VERSION: u32 = 1;

pub type GlGetProcFn = unsafe extern "C" fn(*const c_char) -> *const c_void;
type AbiVersionFn = unsafe extern "C" fn() -> u32;
type CreateFn = unsafe extern "C" fn(GlGetProcFn, i32, i32, f32, *const c_char) -> *mut c_void;
type ResizeFn = unsafe extern "C" fn(*mut c_void, i32, i32, f32);
type PostJsonFn = unsafe extern "C" fn(*mut c_void, *const c_char);
type RenderFn = unsafe extern "C" fn(*mut c_void, f32);
type DestroyFn = unsafe extern "C" fn(*mut c_void);

#[cfg(not(target_os = "windows"))]
mod dynlib {
    use std::ffi::{c_char, c_int, c_void, CString};
    use std::path::Path;

    const RTLD_NOW: c_int = 0x0002;
    const RTLD_LOCAL: c_int = 0x0000;

    extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
        fn dlerror() -> *const c_char;
    }

    pub struct Library(*mut c_void);

    impl Library {
        pub unsafe fn open(path: &Path) -> Result<Self, String> {
            let cpath = CString::new(path.to_string_lossy().as_bytes())
                .map_err(|_| "Invalid plugin path".to_string())?;
            let h = dlopen(cpath.as_ptr(), RTLD_NOW | RTLD_LOCAL);
            if h.is_null() {
                let err = dlerror();
                let msg = if err.is_null() {
                    "dlopen failed".to_string()
                } else {
                    std::ffi::CStr::from_ptr(err).to_string_lossy().into_owned()
                };
                return Err(msg);
            }
            Ok(Self(h))
        }

        pub unsafe fn sym(&self, name: &str) -> *mut c_void {
            let Ok(cname) = CString::new(name) else {
                return std::ptr::null_mut();
            };
            dlsym(self.0, cname.as_ptr())
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    dlclose(self.0);
                }
                self.0 = std::ptr::null_mut();
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod dynlib {
    use std::ffi::{c_void, CString};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    pub struct Library(HMODULE);

    impl Library {
        pub unsafe fn open(path: &Path) -> Result<Self, String> {
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let h = LoadLibraryW(wide.as_ptr());
            if h.is_null() {
                return Err(format!("LoadLibraryW failed for {}", path.display()));
            }
            Ok(Self(h))
        }

        pub unsafe fn sym(&self, name: &str) -> *mut c_void {
            let Ok(cname) = CString::new(name) else {
                return std::ptr::null_mut();
            };
            match GetProcAddress(self.0, cname.as_ptr() as *const u8) {
                Some(f) => f as *mut c_void,
                None => std::ptr::null_mut(),
            }
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    FreeLibrary(self.0);
                }
                self.0 = std::ptr::null_mut();
            }
        }
    }
}

struct LoadedPlugin {
    _lib: dynlib::Library,
    ctx: *mut c_void,
    resize_fn: Option<ResizeFn>,
    post_json_fn: Option<PostJsonFn>,
    render_fn: RenderFn,
    destroy_fn: DestroyFn,
    last_w: i32,
    last_h: i32,
    last_dpr: f32,
}

impl LoadedPlugin {
    unsafe fn load(
        so_path: &Path,
        assets_dir: &Path,
        w: i32,
        h: i32,
        dpr: f32,
    ) -> Result<Self, String> {
        let lib = dynlib::Library::open(so_path)?;
        let abi_ptr = lib.sym("ddgl_plugin_abi_version");
        if abi_ptr.is_null() {
            return Err("Plugin missing ddgl_plugin_abi_version symbol".into());
        }
        let abi_fn: AbiVersionFn = std::mem::transmute(abi_ptr);
        let ver = abi_fn();
        if ver != DDGL_PLUGIN_ABI_VERSION {
            return Err(format!(
                "Unsupported plugin ABI version {ver} (expected {DDGL_PLUGIN_ABI_VERSION})"
            ));
        }

        let create_ptr = lib.sym("ddgl_plugin_create");
        let render_ptr = lib.sym("ddgl_plugin_render");
        let destroy_ptr = lib.sym("ddgl_plugin_destroy");
        if create_ptr.is_null() || render_ptr.is_null() || destroy_ptr.is_null() {
            return Err("Plugin missing required symbols (create/render/destroy)".into());
        }

        let create_fn: CreateFn = std::mem::transmute(create_ptr);
        let render_fn: RenderFn = std::mem::transmute(render_ptr);
        let destroy_fn: DestroyFn = std::mem::transmute(destroy_ptr);
        let resize_ptr = lib.sym("ddgl_plugin_resize");
        let resize_fn: Option<ResizeFn> = if resize_ptr.is_null() {
            None
        } else {
            Some(std::mem::transmute(resize_ptr))
        };
        let post_ptr = lib.sym("ddgl_plugin_post_json");
        let post_json_fn: Option<PostJsonFn> = if post_ptr.is_null() {
            None
        } else {
            Some(std::mem::transmute(post_ptr))
        };

        let c_assets = CString::new(assets_dir.to_string_lossy().as_bytes())
            .map_err(|_| "Invalid assets directory path".to_string())?;
        let ctx = create_fn(host_gl_get_proc, w.max(16), h.max(16), dpr.max(0.5), c_assets.as_ptr());
        if ctx.is_null() {
            return Err("ddgl_plugin_create returned null context".into());
        }
        Ok(Self {
            _lib: lib,
            ctx,
            resize_fn,
            post_json_fn,
            render_fn,
            destroy_fn,
            last_w: w.max(16),
            last_h: h.max(16),
            last_dpr: dpr.max(0.5),
        })
    }

    unsafe fn post_json(&self, msg: &str) {
        if let Some(f) = self.post_json_fn {
            if let Ok(cstr) = CString::new(msg) {
                f(self.ctx, cstr.as_ptr());
            }
        }
    }

    unsafe fn destroy(mut self) {
        if !self.ctx.is_null() {
            (self.destroy_fn)(self.ctx);
            self.ctx = std::ptr::null_mut();
        }
        // `_lib` drops after `ctx` is destroyed so plugin code is still mapped during destroy.
    }
}

const PLUGIN_FALLBACK_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;
uniform float u_time;
void main() {
    float g = 0.5 + 0.5 * sin(v_uv.x * 5.0 - u_time * 1.1);
    fragColor = vec4(mix(vec3(0.07, 0.08, 0.13), vec3(0.14, 0.11, 0.22), g * 0.6), 1.0);
}
"#;

pub struct CustomPluginRenderer {
    vao: GLuint,
    fallback_prog: GLuint,
    loaded: Option<LoadedPlugin>,
    loaded_rev: u64,
    last_time: f32,
    pub last_error: Option<String>,
}

impl CustomPluginRenderer {
    pub unsafe fn new(gl: &GlFns) -> Result<Self, String> {
        let mut vao = 0;
        (gl.GenVertexArrays)(1, &mut vao);
        let fallback_prog = gl.compile_program(FULLSCREEN_VERT, PLUGIN_FALLBACK_FRAG)?;
        Ok(Self {
            vao,
            fallback_prog,
            loaded: None,
            loaded_rev: 0,
            last_time: 0.0,
            last_error: None,
        })
    }

    unsafe fn unload(&mut self) {
        if let Some(plug) = self.loaded.take() {
            plug.destroy();
        }
    }

    unsafe fn reload_if_needed(&mut self, st: &mut AppState) {
        let spec: &CustomGlSpec = &st.custom_gl;
        if self.loaded_rev == spec.rev && (self.loaded.is_some() || self.last_error.is_some()) {
            return;
        }
        self.unload();
        self.loaded_rev = spec.rev;
        self.last_error = None;

        let so_path = match resolve_inside_dir(&spec.theme_dir, &spec.entry) {
            Ok(p) => p,
            Err(e) => {
                self.last_error = Some(e);
                return;
            }
        };

        match LoadedPlugin::load(&so_path, &spec.theme_dir, st.width_px, st.height_px, st.dpi_scale) {
            Ok(plug) => {
                // Push current state snapshot (settings, icons, solar, lowMode) right after load.
                if !st.raw_params.is_empty() {
                    if let Ok(s) = serde_json::to_string(&serde_json::json!({
                        "t": "settings",
                        "v": &st.raw_params,
                    })) {
                        plug.post_json(&s);
                    }
                }
                if !st.icons.is_empty() {
                    let rects: Vec<[f32; 4]> = st
                        .icons
                        .iter()
                        .map(|r| [r.x, r.y, r.x + r.w, r.y + r.h])
                        .collect();
                    if let Ok(s) = serde_json::to_string(&serde_json::json!({
                        "t": "icons",
                        "vis": true,
                        "r": rects,
                        "n": &st.icon_names,
                    })) {
                        plug.post_json(&s);
                    }
                }
                self.loaded = Some(plug);
            }
            Err(e) => {
                self.last_error = Some(e);
            }
        }
    }

    pub unsafe fn render(&mut self, gl: &GlFns, st: &mut AppState) {
        self.reload_if_needed(st);

        let pw = st.width_px.max(16);
        let ph = st.height_px.max(16);
        let dt = (st.sim_time - self.last_time).clamp(0.0, 0.25);
        self.last_time = st.sim_time;

        if let Some(plug) = self.loaded.as_mut() {
            if plug.last_w != pw
                || plug.last_h != ph
                || (plug.last_dpr - st.dpi_scale).abs() > 0.01
            {
                plug.last_w = pw;
                plug.last_h = ph;
                plug.last_dpr = st.dpi_scale;
                if let Some(rf) = plug.resize_fn {
                    rf(plug.ctx, pw, ph, st.dpi_scale);
                }
            }
            while let Some(msg) = st.custom_plugin_inbox.pop_front() {
                plug.post_json(&msg);
            }
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
            (gl.Viewport)(0, 0, pw, ph);
            (plug.render_fn)(plug.ctx, dt);
            // Restore safe default GL state after external plugin draw
            (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
            (gl.Disable)(GL_SCISSOR_TEST);
            (gl.Disable)(GL_DEPTH_TEST);
            (gl.Disable)(GL_CULL_FACE);
            (gl.Disable)(GL_BLEND);
            (gl.UseProgram)(0);
            (gl.BindVertexArray)(0);
            (gl.ActiveTexture)(GL_TEXTURE0);
            return;
        }

        st.custom_plugin_inbox.clear();
        (gl.BindFramebuffer)(GL_FRAMEBUFFER, 0);
        (gl.Viewport)(0, 0, pw, ph);
        (gl.Disable)(GL_BLEND);
        (gl.UseProgram)(self.fallback_prog);
        (gl.Uniform1f)(gl.uniform_loc(self.fallback_prog, "u_time"), st.sim_time);
        (gl.BindVertexArray)(self.vao);
        (gl.DrawArrays)(GL_TRIANGLES, 0, 3);
    }

    pub unsafe fn destroy(&mut self, gl: &GlFns) {
        self.unload();
        if self.fallback_prog != 0 {
            (gl.DeleteProgram)(self.fallback_prog);
            self.fallback_prog = 0;
        }
        if self.vao != 0 {
            (gl.DeleteVertexArrays)(1, &self.vao);
            self.vao = 0;
        }
    }
}
