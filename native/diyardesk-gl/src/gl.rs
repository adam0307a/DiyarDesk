#![allow(non_snake_case, dead_code)]

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_uint, c_void, CString};
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{HMODULE, HWND};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC, HDC};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::OpenGL::{
    ChoosePixelFormat, SetPixelFormat, SwapBuffers, wglCreateContext, wglDeleteContext,
    wglGetProcAddress, wglMakeCurrent, HGLRC, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW,
    PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

pub type GLenum = c_uint;
pub type GLuint = c_uint;
pub type GLint = c_int;
pub type GLsizei = c_int;
pub type GLfloat = f32;
pub type GLboolean = u8;
pub type GLbitfield = c_uint;
pub type GLsizeiptr = isize;
pub type GLintptr = isize;

pub const GL_DEPTH_BUFFER_BIT: GLbitfield = 0x0000_0100;
pub const GL_COLOR_BUFFER_BIT: GLbitfield = 0x0000_4000;
pub const GL_POINTS: GLenum = 0x0000;
pub const GL_TRIANGLES: GLenum = 0x0004;
pub const GL_TRIANGLE_STRIP: GLenum = 0x0005;
pub const GL_UNSIGNED_BYTE: GLenum = 0x1401;
pub const GL_FLOAT: GLenum = 0x1406;
pub const GL_FALSE: GLboolean = 0;
pub const GL_TRUE: GLboolean = 1;
pub const GL_DEPTH_TEST: GLenum = 0x0B71;
pub const GL_CULL_FACE: GLenum = 0x0B44;
pub const GL_BLEND: GLenum = 0x0BE2;
pub const GL_PROGRAM_POINT_SIZE: GLenum = 0x8642;
pub const GL_ZERO: GLenum = 0;
pub const GL_ONE: GLenum = 1;
pub const GL_SRC_ALPHA: GLenum = 0x0302;
pub const GL_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
pub const GL_SCISSOR_TEST: GLenum = 0x0C11;
pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
pub const GL_TEXTURE0: GLenum = 0x84C0;
pub const GL_TEXTURE1: GLenum = 0x84C1;
pub const GL_TEXTURE2: GLenum = 0x84C2;
pub const GL_TEXTURE3: GLenum = 0x84C3;
pub const GL_TEXTURE4: GLenum = 0x84C4;
pub const GL_TEXTURE5: GLenum = 0x84C5;
pub const GL_TEXTURE6: GLenum = 0x84C6;
pub const GL_TEXTURE7: GLenum = 0x84C7;
pub const GL_TEXTURE8: GLenum = 0x84C8;
pub const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
pub const GL_TEXTURE_WRAP_T: GLenum = 0x2803;
pub const GL_NEAREST: GLint = 0x2600;
pub const GL_LINEAR: GLint = 0x2601;
pub const GL_LINEAR_MIPMAP_LINEAR: GLint = 0x2703;
pub const GL_REPEAT: GLint = 0x2901;
pub const GL_CLAMP_TO_EDGE: GLint = 0x812F;
pub const GL_RED: GLenum = 0x1903;
pub const GL_R8: GLenum = 0x8229;
pub const GL_R16F: GLenum = 0x822D;
pub const GL_R32F: GLenum = 0x822E;
pub const GL_HALF_FLOAT: GLenum = 0x140B;
pub const GL_RGBA: GLenum = 0x1908;
pub const GL_RGBA8: GLenum = 0x8058;
pub const GL_RGBA16F: GLenum = 0x881A;
pub const GL_RGBA32F: GLenum = 0x8814;
pub const GL_UNPACK_ALIGNMENT: GLenum = 0x0CF5;
pub const GL_ARRAY_BUFFER: GLenum = 0x8892;
pub const GL_STATIC_DRAW: GLenum = 0x88E4;
pub const GL_DYNAMIC_DRAW: GLenum = 0x88E8;
pub const GL_VERTEX_SHADER: GLenum = 0x8B31;
pub const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
pub const GL_COMPILE_STATUS: GLenum = 0x8B81;
pub const GL_LINK_STATUS: GLenum = 0x8B82;
pub const GL_FRAMEBUFFER: GLenum = 0x8D40;
pub const GL_RENDERBUFFER: GLenum = 0x8D41;
pub const GL_COLOR_ATTACHMENT0: GLenum = 0x8CE0;
pub const GL_DEPTH_ATTACHMENT: GLenum = 0x8D00;
pub const GL_DEPTH_COMPONENT24: GLenum = 0x81A6;
pub const GL_FRAMEBUFFER_BINDING: GLenum = 0x8CA6;
pub const GL_FRAMEBUFFER_COMPLETE: GLenum = 0x8CD5;
pub const GL_NO_ERROR: GLenum = 0;
pub const GL_VERSION: GLenum = 0x1F02;
pub const GL_EXTENSIONS: GLenum = 0x1F03;
pub const GL_NUM_EXTENSIONS: GLenum = 0x821D;

#[cfg(target_os = "windows")]
const WGL_CONTEXT_MAJOR_VERSION_ARB: c_int = 0x2091;
#[cfg(target_os = "windows")]
const WGL_CONTEXT_MINOR_VERSION_ARB: c_int = 0x2092;
#[cfg(target_os = "windows")]
const WGL_CONTEXT_PROFILE_MASK_ARB: c_int = 0x9126;
#[cfg(target_os = "windows")]
const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: c_int = 0x0000_0001;

#[cfg(target_os = "windows")]
type WglCreateContextAttribsArb =
    unsafe extern "system" fn(HDC, HGLRC, *const c_int) -> HGLRC;
#[cfg(target_os = "windows")]
type WglSwapIntervalExt = unsafe extern "system" fn(c_int) -> c_int;

// Redirect FBO 0 bindings to a host-supplied target FBO (GtkGLArea, Qt6 FBO, or headless EGL FBO)
// without modifying any theme renderer. When DEFAULT_FRAMEBUFFER == 0, binds FBO 0 directly.
static REAL_BIND_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static REAL_ENABLE: AtomicUsize = AtomicUsize::new(0);
static REAL_DISABLE: AtomicUsize = AtomicUsize::new(0);
static REAL_TEX_IMAGE_2D: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static DEFAULT_FRAMEBUFFER: Cell<GLuint> = const { Cell::new(0) };
}

unsafe extern "system" fn bind_framebuffer_redirect(target: GLenum, fb: GLuint) {
    let actual = if fb == 0 {
        DEFAULT_FRAMEBUFFER.with(|c| c.get())
    } else {
        fb
    };
    let addr = REAL_BIND_FRAMEBUFFER.load(Ordering::Relaxed);
    if addr != 0 {
        let real_fn: unsafe extern "system" fn(GLenum, GLuint) = std::mem::transmute(addr);
        real_fn(target, actual);
    }
}

/// OpenGL ES 3.0 always enables `gl_PointSize` in vertex shaders and rejects
/// `GL_PROGRAM_POINT_SIZE` with `GL_INVALID_ENUM`. Skip it transparently on GLES.
unsafe extern "system" fn enable_gles_safe(cap: GLenum) {
    if cap == GL_PROGRAM_POINT_SIZE {
        return;
    }
    let addr = REAL_ENABLE.load(Ordering::Relaxed);
    if addr != 0 {
        let real_fn: unsafe extern "system" fn(GLenum) = std::mem::transmute(addr);
        real_fn(cap);
    }
}

unsafe extern "system" fn disable_gles_safe(cap: GLenum) {
    if cap == GL_PROGRAM_POINT_SIZE {
        return;
    }
    let addr = REAL_DISABLE.load(Ordering::Relaxed);
    if addr != 0 {
        let real_fn: unsafe extern "system" fn(GLenum) = std::mem::transmute(addr);
        real_fn(cap);
    }
}

/// On OpenGL ES 3.0 without `OES_texture_float_linear`, `GL_R32F` textures with
/// `GL_LINEAR` filtering are texture-incomplete and sample as 0. Promoting
/// `GL_R32F` (`GL_RED` + `GL_FLOAT`) to `GL_R16F` keeps full GLES 3.0 core
/// hardware linear filtering and valid float uploads (GLES 3.0 Table 3.2 & 3.12).
unsafe extern "system" fn tex_image_2d_gles_safe(
    target: GLenum,
    level: GLint,
    internalformat: GLint,
    width: GLsizei,
    height: GLsizei,
    border: GLint,
    format: GLenum,
    ty: GLenum,
    pixels: *const c_void,
) {
    let resolved_internal = if internalformat == GL_R32F as GLint
        && format == GL_RED
        && ty == GL_FLOAT
    {
        GL_R16F as GLint
    } else {
        internalformat
    };
    let addr = REAL_TEX_IMAGE_2D.load(Ordering::Relaxed);
    if addr != 0 {
        let real_fn: unsafe extern "system" fn(
            GLenum, GLint, GLint, GLsizei, GLsizei, GLint, GLenum, GLenum, *const c_void,
        ) = std::mem::transmute(addr);
        real_fn(target, level, resolved_internal, width, height, border, format, ty, pixels);
    }
}

pub struct DefaultFboGuard {
    prev: GLuint,
}

impl DefaultFboGuard {
    pub fn new(fbo: GLuint) -> Self {
        let prev = DEFAULT_FRAMEBUFFER.with(|c| {
            let p = c.get();
            c.set(fbo);
            p
        });
        Self { prev }
    }
}

impl Drop for DefaultFboGuard {
    fn drop(&mut self) {
        DEFAULT_FRAMEBUFFER.with(|c| c.set(self.prev));
    }
}

pub struct GlFns {
    pub Viewport: unsafe extern "system" fn(GLint, GLint, GLsizei, GLsizei),
    pub Scissor: unsafe extern "system" fn(GLint, GLint, GLsizei, GLsizei),
    pub Enable: unsafe extern "system" fn(GLenum),
    pub Disable: unsafe extern "system" fn(GLenum),
    pub BlendFunc: unsafe extern "system" fn(GLenum, GLenum),
    pub BlendFuncSeparate: unsafe extern "system" fn(GLenum, GLenum, GLenum, GLenum),
    pub DepthMask: unsafe extern "system" fn(GLboolean),
    pub ClearColor: unsafe extern "system" fn(GLfloat, GLfloat, GLfloat, GLfloat),
    pub Clear: unsafe extern "system" fn(GLbitfield),
    pub DrawArrays: unsafe extern "system" fn(GLenum, GLint, GLsizei),
    pub DrawArraysInstanced: unsafe extern "system" fn(GLenum, GLint, GLsizei, GLsizei),
    pub GenTextures: unsafe extern "system" fn(GLsizei, *mut GLuint),
    pub DeleteTextures: unsafe extern "system" fn(GLsizei, *const GLuint),
    pub BindTexture: unsafe extern "system" fn(GLenum, GLuint),
    pub TexParameteri: unsafe extern "system" fn(GLenum, GLenum, GLint),
    pub TexImage2D: unsafe extern "system" fn(
        GLenum, GLint, GLint, GLsizei, GLsizei, GLint, GLenum, GLenum, *const c_void,
    ),
    pub TexSubImage2D: unsafe extern "system" fn(
        GLenum, GLint, GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, *const c_void,
    ),
    pub PixelStorei: unsafe extern "system" fn(GLenum, GLint),
    pub GenerateMipmap: unsafe extern "system" fn(GLenum),
    pub ActiveTexture: unsafe extern "system" fn(GLenum),
    pub GenBuffers: unsafe extern "system" fn(GLsizei, *mut GLuint),
    pub DeleteBuffers: unsafe extern "system" fn(GLsizei, *const GLuint),
    pub BindBuffer: unsafe extern "system" fn(GLenum, GLuint),
    pub BufferData: unsafe extern "system" fn(GLenum, GLsizeiptr, *const c_void, GLenum),
    pub BufferSubData: unsafe extern "system" fn(GLenum, GLintptr, GLsizeiptr, *const c_void),
    pub GenVertexArrays: unsafe extern "system" fn(GLsizei, *mut GLuint),
    pub DeleteVertexArrays: unsafe extern "system" fn(GLsizei, *const GLuint),
    pub BindVertexArray: unsafe extern "system" fn(GLuint),
    pub EnableVertexAttribArray: unsafe extern "system" fn(GLuint),
    pub VertexAttribPointer:
        unsafe extern "system" fn(GLuint, GLint, GLenum, GLboolean, GLsizei, *const c_void),
    pub VertexAttribDivisor: unsafe extern "system" fn(GLuint, GLuint),
    pub CreateShader: unsafe extern "system" fn(GLenum) -> GLuint,
    pub DeleteShader: unsafe extern "system" fn(GLuint),
    pub ShaderSource: unsafe extern "system" fn(GLuint, GLsizei, *const *const c_char, *const GLint),
    pub CompileShader: unsafe extern "system" fn(GLuint),
    pub GetShaderiv: unsafe extern "system" fn(GLuint, GLenum, *mut GLint),
    pub GetShaderInfoLog: unsafe extern "system" fn(GLuint, GLsizei, *mut GLsizei, *mut c_char),
    pub CreateProgram: unsafe extern "system" fn() -> GLuint,
    pub DeleteProgram: unsafe extern "system" fn(GLuint),
    pub AttachShader: unsafe extern "system" fn(GLuint, GLuint),
    pub LinkProgram: unsafe extern "system" fn(GLuint),
    pub GetProgramiv: unsafe extern "system" fn(GLuint, GLenum, *mut GLint),
    pub GetProgramInfoLog: unsafe extern "system" fn(GLuint, GLsizei, *mut GLsizei, *mut c_char),
    pub UseProgram: unsafe extern "system" fn(GLuint),
    pub GetUniformLocation: unsafe extern "system" fn(GLuint, *const c_char) -> GLint,
    pub Uniform1i: unsafe extern "system" fn(GLint, GLint),
    pub Uniform1f: unsafe extern "system" fn(GLint, GLfloat),
    pub Uniform2f: unsafe extern "system" fn(GLint, GLfloat, GLfloat),
    pub Uniform3f: unsafe extern "system" fn(GLint, GLfloat, GLfloat, GLfloat),
    pub Uniform4f: unsafe extern "system" fn(GLint, GLfloat, GLfloat, GLfloat, GLfloat),
    pub Uniform4fv: unsafe extern "system" fn(GLint, GLsizei, *const GLfloat),
    pub Uniform1fv: unsafe extern "system" fn(GLint, GLsizei, *const GLfloat),
    pub UniformMatrix4fv: unsafe extern "system" fn(GLint, GLsizei, GLboolean, *const GLfloat),
    pub GenFramebuffers: unsafe extern "system" fn(GLsizei, *mut GLuint),
    pub DeleteFramebuffers: unsafe extern "system" fn(GLsizei, *const GLuint),
    pub BindFramebuffer: unsafe extern "system" fn(GLenum, GLuint),
    pub FramebufferTexture2D: unsafe extern "system" fn(GLenum, GLenum, GLenum, GLuint, GLint),
    pub GenRenderbuffers: unsafe extern "system" fn(GLsizei, *mut GLuint),
    pub DeleteRenderbuffers: unsafe extern "system" fn(GLsizei, *const GLuint),
    pub BindRenderbuffer: unsafe extern "system" fn(GLenum, GLuint),
    pub RenderbufferStorage: unsafe extern "system" fn(GLenum, GLenum, GLsizei, GLsizei),
    pub FramebufferRenderbuffer: unsafe extern "system" fn(GLenum, GLenum, GLenum, GLuint),
    pub CheckFramebufferStatus: unsafe extern "system" fn(GLenum) -> GLenum,
    pub GetIntegerv: unsafe extern "system" fn(GLenum, *mut GLint),
    pub GetError: unsafe extern "system" fn() -> GLenum,
    pub GetString: unsafe extern "system" fn(GLenum) -> *const u8,
    pub GetStringi: unsafe extern "system" fn(GLenum, GLuint) -> *const u8,
    pub ReadPixels: unsafe extern "system" fn(GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, *mut c_void),
    pub Flush: unsafe extern "system" fn(),
    pub Finish: unsafe extern "system" fn(),
    pub is_gles: bool,
    pub has_float_linear: bool,
    pub has_color_buffer_float: bool,
}

#[cfg(target_os = "windows")]
struct GlProcLoader {
    opengl32: HMODULE,
}

#[cfg(target_os = "windows")]
impl GlProcLoader {
    unsafe fn open() -> Result<Self, String> {
        let lib_name = b"opengl32.dll\0";
        let opengl32 = LoadLibraryA(lib_name.as_ptr());
        if opengl32.is_null() {
            return Err("LoadLibraryA(opengl32.dll) failed".into());
        }
        Ok(Self { opengl32 })
    }

    unsafe fn load_proc(&self, name: &str) -> Result<*const c_void, String> {
        let cname = CString::new(name).unwrap();
        let ptr = wglGetProcAddress(cname.as_ptr() as *const u8);
        if let Some(f) = ptr {
            let addr = f as usize;
            if addr != 0 && addr != 1 && addr != 2 && addr != 3 && addr != usize::MAX {
                return Ok(f as *const c_void);
            }
        }
        let ptr2 = GetProcAddress(self.opengl32, cname.as_ptr() as *const u8);
        if let Some(f) = ptr2 {
            return Ok(f as *const c_void);
        }
        Err(format!("Failed to load OpenGL function: {name}"))
    }
}

#[cfg(not(target_os = "windows"))]
mod unix_dl {
    use std::ffi::{c_char, c_int, c_void};

    pub const RTLD_LAZY: c_int = 0x0001;
    #[cfg(target_os = "macos")]
    pub const RTLD_LOCAL: c_int = 0x0004;
    #[cfg(not(target_os = "macos"))]
    pub const RTLD_LOCAL: c_int = 0x0000;
    #[cfg(target_os = "macos")]
    pub const RTLD_NOLOAD: c_int = 0x0010;
    #[cfg(not(target_os = "macos"))]
    pub const RTLD_NOLOAD: c_int = 0x0004;

    extern "C" {
        pub fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        pub fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        pub fn dlclose(handle: *mut c_void) -> c_int;
    }

    pub unsafe fn open_any(names: &[&[u8]]) -> *mut c_void {
        for name in names {
            let h = dlopen(name.as_ptr() as *const c_char, RTLD_LAZY | RTLD_LOCAL);
            if !h.is_null() {
                return h;
            }
        }
        std::ptr::null_mut()
    }
}

#[cfg(not(target_os = "windows"))]
struct GlProcLoader {
    egl_get_proc: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
    glx_get_proc: Option<unsafe extern "C" fn(*const u8) -> *const c_void>,
    prefer_egl: bool,
    lib_gles: *mut c_void,
    lib_gl: *mut c_void,
    lib_opengl: *mut c_void,
}

#[cfg(not(target_os = "windows"))]
impl GlProcLoader {
    unsafe fn open() -> Result<Self, String> {
        if cfg!(target_os = "macos") {
            let lib_opengl = unix_dl::open_any(&[
                b"/System/Library/Frameworks/OpenGL.framework/Versions/Current/OpenGL\0",
                b"/System/Library/Frameworks/OpenGL.framework/OpenGL\0",
            ]);
            if lib_opengl.is_null() {
                return Err("Could not open macOS OpenGL.framework".into());
            }
            return Ok(Self {
                egl_get_proc: None,
                glx_get_proc: None,
                prefer_egl: false,
                lib_gles: std::ptr::null_mut(),
                lib_gl: std::ptr::null_mut(),
                lib_opengl,
            });
        }

        let lib_egl = unix_dl::open_any(&[b"libEGL.so\0", b"libEGL.so.1\0"]);
        let lib_gles = unix_dl::open_any(&[
            b"libGLESv3.so\0",
            b"libGLESv3.so.3\0",
            b"libGLESv2.so.2\0",
            b"libGLESv2.so\0",
        ]);
        let lib_glx = if cfg!(target_os = "android") {
            std::ptr::null_mut()
        } else {
            unix_dl::open_any(&[b"libGLX.so.0\0", b"libGL.so.1\0", b"libGL.so\0"])
        };
        let lib_gl = if cfg!(target_os = "android") {
            std::ptr::null_mut()
        } else {
            unix_dl::open_any(&[b"libGL.so.1\0", b"libGL.so\0"])
        };
        let lib_opengl = if cfg!(target_os = "android") {
            std::ptr::null_mut()
        } else {
            unix_dl::open_any(&[b"libOpenGL.so.0\0", b"libOpenGL.so\0"])
        };

        let mut egl_get_proc = None;
        let mut prefer_egl = cfg!(target_os = "android");
        if !lib_egl.is_null() {
            let p = unix_dl::dlsym(lib_egl, b"eglGetProcAddress\0".as_ptr() as *const c_char);
            if !p.is_null() {
                egl_get_proc = Some(std::mem::transmute(p));
            }
            let cur_p = unix_dl::dlsym(lib_egl, b"eglGetCurrentContext\0".as_ptr() as *const c_char);
            if !cur_p.is_null() {
                let get_cur: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(cur_p);
                if !get_cur().is_null() {
                    prefer_egl = true;
                }
            }
        }

        let mut glx_get_proc = None;
        if !lib_glx.is_null() {
            let p = unix_dl::dlsym(lib_glx, b"glXGetProcAddressARB\0".as_ptr() as *const c_char);
            let p = if !p.is_null() {
                p
            } else {
                unix_dl::dlsym(lib_glx, b"glXGetProcAddress\0".as_ptr() as *const c_char)
            };
            if !p.is_null() {
                glx_get_proc = Some(std::mem::transmute(p));
            }
        }

        if egl_get_proc.is_none()
            && glx_get_proc.is_none()
            && lib_gles.is_null()
            && lib_gl.is_null()
            && lib_opengl.is_null()
        {
            return Err(
                "Could not open libEGL.so, libGLESv3.so, libGLX.so.0, libGL.so.1 or libOpenGL.so.0"
                    .into(),
            );
        }

        Ok(Self {
            egl_get_proc,
            glx_get_proc,
            prefer_egl,
            lib_gles,
            lib_gl,
            lib_opengl,
        })
    }

    unsafe fn load_proc(&self, name: &str) -> Result<*const c_void, String> {
        let cname = CString::new(name).unwrap();
        let try_egl = || -> *const c_void {
            if let Some(f) = self.egl_get_proc {
                f(cname.as_ptr())
            } else {
                std::ptr::null()
            }
        };
        let try_glx = || -> *const c_void {
            if let Some(f) = self.glx_get_proc {
                f(cname.as_ptr() as *const u8)
            } else {
                std::ptr::null()
            }
        };

        // On Android, dlsym on libGLESv3.so is the fastest and most reliable path for core ES 3.0
        // entry points (older drivers may return null from eglGetProcAddress for core symbols).
        if cfg!(target_os = "android") && !self.lib_gles.is_null() {
            let p = unix_dl::dlsym(self.lib_gles, cname.as_ptr());
            if !p.is_null() {
                return Ok(p as *const c_void);
            }
        }

        // On macOS, core OpenGL 3.3 symbols live directly inside OpenGL.framework.
        if cfg!(target_os = "macos") && !self.lib_opengl.is_null() {
            let p = unix_dl::dlsym(self.lib_opengl, cname.as_ptr());
            if !p.is_null() {
                return Ok(p as *const c_void);
            }
        }

        let first = if self.prefer_egl { try_egl() } else { try_glx() };
        if !first.is_null() {
            return Ok(first);
        }
        let second = if self.prefer_egl { try_glx() } else { try_egl() };
        if !second.is_null() {
            return Ok(second);
        }
        for lib in [self.lib_gles, self.lib_opengl, self.lib_gl, std::ptr::null_mut()] {
            if lib.is_null() && !self.lib_gles.is_null() {
                continue;
            }
            let p = unix_dl::dlsym(lib, cname.as_ptr());
            if !p.is_null() {
                return Ok(p as *const c_void);
            }
        }
        Err(format!("Failed to load OpenGL function: {name}"))
    }
}

/// C-ABI OpenGL procedure loader passed to external Rust `cdylib` theme plugins.
/// Redirects `glBindFramebuffer` through `bind_framebuffer_redirect` so plugins
/// binding FBO 0 automatically render into GtkGLArea / Qt6 / headless EGL FBOs.
pub unsafe extern "C" fn host_gl_get_proc(name: *const c_char) -> *const c_void {
    if name.is_null() {
        return std::ptr::null();
    }
    let Ok(s) = std::ffi::CStr::from_ptr(name).to_str() else {
        return std::ptr::null();
    };
    if s == "glBindFramebuffer" {
        return bind_framebuffer_redirect as *const c_void;
    }
    match GlProcLoader::open() {
        Ok(loader) => loader.load_proc(s).unwrap_or(std::ptr::null()),
        Err(_) => std::ptr::null(),
    }
}

const GLES3_DEFAULT_PRECISION: &str =
    "precision highp float;\nprecision highp int;\nprecision highp sampler2D;\nprecision highp isampler2D;\nprecision highp usampler2D;\n";

/// Translates a desktop GLSL `#version 330 core` shader into OpenGL ES 3.00 (`#version 300 es`).
/// Preserves `#extension` directives immediately after `#version 300 es` before injecting
/// default high-precision qualifiers for `float`, `int`, and 2D sampler types (`sampler2D`,
/// `isampler2D`, `usampler2D`).
pub fn translate_glsl_330_to_300_es(src: &str) -> String {
    let clean = src.trim_start_matches('\u{feff}');
    let mut lines = clean.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let first_trimmed = first.trim();
    if first_trimmed.starts_with("#version 300 es") {
        return clean.to_string();
    }

    let mut out = String::with_capacity(clean.len() + GLES3_DEFAULT_PRECISION.len() + 32);
    if first_trimmed.starts_with("#version") {
        out.push_str("#version 300 es\n");
    } else {
        out.push_str("#version 300 es\n");
        out.push_str(GLES3_DEFAULT_PRECISION);
        out.push_str(first);
        out.push('\n');
        for line in lines {
            out.push_str(line);
            out.push('\n');
        }
        return out;
    }

    let mut precision_emitted = false;
    for line in lines {
        let trimmed = line.trim_start();
        if !precision_emitted {
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#extension") {
                out.push_str(line);
                out.push('\n');
                continue;
            }
            out.push_str(GLES3_DEFAULT_PRECISION);
            precision_emitted = true;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !precision_emitted {
        out.push_str(GLES3_DEFAULT_PRECISION);
    }
    out
}

impl GlFns {
    pub unsafe fn load() -> Result<Self, String> {
        let loader = GlProcLoader::open()?;
        macro_rules! ld {
            ($name:literal) => {
                std::mem::transmute(loader.load_proc($name)?)
            };
        }
        let raw_bind_fb = loader.load_proc("glBindFramebuffer")?;
        let raw_enable = loader.load_proc("glEnable")?;
        let raw_disable = loader.load_proc("glDisable")?;
        let raw_tex_image_2d = loader.load_proc("glTexImage2D")?;
        REAL_BIND_FRAMEBUFFER.store(raw_bind_fb as usize, Ordering::Relaxed);
        REAL_ENABLE.store(raw_enable as usize, Ordering::Relaxed);
        REAL_DISABLE.store(raw_disable as usize, Ordering::Relaxed);
        REAL_TEX_IMAGE_2D.store(raw_tex_image_2d as usize, Ordering::Relaxed);

        let get_integerv: unsafe extern "system" fn(GLenum, *mut GLint) = ld!("glGetIntegerv");
        let get_string: unsafe extern "system" fn(GLenum) -> *const u8 = ld!("glGetString");
        let get_stringi: unsafe extern "system" fn(GLenum, GLuint) -> *const u8 = ld!("glGetStringi");

        let ver_ptr = get_string(GL_VERSION);
        let ver_str = if ver_ptr.is_null() {
            ""
        } else {
            std::ffi::CStr::from_ptr(ver_ptr as *const c_char)
                .to_str()
                .unwrap_or("")
        };
        let is_gles = cfg!(target_os = "android") || ver_str.contains("OpenGL ES");

        let mut has_float_linear = !is_gles;
        let mut has_color_buffer_float = !is_gles;
        if is_gles {
            let mut num_ext: GLint = 0;
            get_integerv(GL_NUM_EXTENSIONS, &mut num_ext);
            for i in 0..num_ext.max(0) as GLuint {
                let p = get_stringi(GL_EXTENSIONS, i);
                if p.is_null() {
                    continue;
                }
                if let Ok(ext) = std::ffi::CStr::from_ptr(p as *const c_char).to_str() {
                    if ext == "GL_OES_texture_float_linear" {
                        has_float_linear = true;
                    } else if ext == "GL_EXT_color_buffer_float"
                        || ext == "GL_EXT_color_buffer_half_float"
                    {
                        has_color_buffer_float = true;
                    }
                }
            }
        }

        let enable_fn: unsafe extern "system" fn(GLenum) = if is_gles {
            enable_gles_safe
        } else {
            std::mem::transmute(raw_enable)
        };
        let disable_fn: unsafe extern "system" fn(GLenum) = if is_gles {
            disable_gles_safe
        } else {
            std::mem::transmute(raw_disable)
        };
        let tex_image_2d_fn: unsafe extern "system" fn(
            GLenum, GLint, GLint, GLsizei, GLsizei, GLint, GLenum, GLenum, *const c_void,
        ) = if is_gles && !has_float_linear {
            tex_image_2d_gles_safe
        } else {
            std::mem::transmute(raw_tex_image_2d)
        };

        Ok(Self {
            Viewport: ld!("glViewport"),
            Scissor: ld!("glScissor"),
            Enable: enable_fn,
            Disable: disable_fn,
            BlendFunc: ld!("glBlendFunc"),
            BlendFuncSeparate: ld!("glBlendFuncSeparate"),
            DepthMask: ld!("glDepthMask"),
            ClearColor: ld!("glClearColor"),
            Clear: ld!("glClear"),
            DrawArrays: ld!("glDrawArrays"),
            DrawArraysInstanced: ld!("glDrawArraysInstanced"),
            GenTextures: ld!("glGenTextures"),
            DeleteTextures: ld!("glDeleteTextures"),
            BindTexture: ld!("glBindTexture"),
            TexParameteri: ld!("glTexParameteri"),
            TexImage2D: tex_image_2d_fn,
            TexSubImage2D: ld!("glTexSubImage2D"),
            PixelStorei: ld!("glPixelStorei"),
            GenerateMipmap: ld!("glGenerateMipmap"),
            ActiveTexture: ld!("glActiveTexture"),
            GenBuffers: ld!("glGenBuffers"),
            DeleteBuffers: ld!("glDeleteBuffers"),
            BindBuffer: ld!("glBindBuffer"),
            BufferData: ld!("glBufferData"),
            BufferSubData: ld!("glBufferSubData"),
            GenVertexArrays: ld!("glGenVertexArrays"),
            DeleteVertexArrays: ld!("glDeleteVertexArrays"),
            BindVertexArray: ld!("glBindVertexArray"),
            EnableVertexAttribArray: ld!("glEnableVertexAttribArray"),
            VertexAttribPointer: ld!("glVertexAttribPointer"),
            VertexAttribDivisor: ld!("glVertexAttribDivisor"),
            CreateShader: ld!("glCreateShader"),
            DeleteShader: ld!("glDeleteShader"),
            ShaderSource: ld!("glShaderSource"),
            CompileShader: ld!("glCompileShader"),
            GetShaderiv: ld!("glGetShaderiv"),
            GetShaderInfoLog: ld!("glGetShaderInfoLog"),
            CreateProgram: ld!("glCreateProgram"),
            DeleteProgram: ld!("glDeleteProgram"),
            AttachShader: ld!("glAttachShader"),
            LinkProgram: ld!("glLinkProgram"),
            GetProgramiv: ld!("glGetProgramiv"),
            GetProgramInfoLog: ld!("glGetProgramInfoLog"),
            UseProgram: ld!("glUseProgram"),
            GetUniformLocation: ld!("glGetUniformLocation"),
            Uniform1i: ld!("glUniform1i"),
            Uniform1f: ld!("glUniform1f"),
            Uniform2f: ld!("glUniform2f"),
            Uniform3f: ld!("glUniform3f"),
            Uniform4f: ld!("glUniform4f"),
            Uniform4fv: ld!("glUniform4fv"),
            Uniform1fv: ld!("glUniform1fv"),
            UniformMatrix4fv: ld!("glUniformMatrix4fv"),
            GenFramebuffers: ld!("glGenFramebuffers"),
            DeleteFramebuffers: ld!("glDeleteFramebuffers"),
            BindFramebuffer: bind_framebuffer_redirect,
            FramebufferTexture2D: ld!("glFramebufferTexture2D"),
            GenRenderbuffers: ld!("glGenRenderbuffers"),
            DeleteRenderbuffers: ld!("glDeleteRenderbuffers"),
            BindRenderbuffer: ld!("glBindRenderbuffer"),
            RenderbufferStorage: ld!("glRenderbufferStorage"),
            FramebufferRenderbuffer: ld!("glFramebufferRenderbuffer"),
            CheckFramebufferStatus: ld!("glCheckFramebufferStatus"),
            GetIntegerv: get_integerv,
            GetError: ld!("glGetError"),
            GetString: get_string,
            GetStringi: get_stringi,
            ReadPixels: ld!("glReadPixels"),
            Flush: ld!("glFlush"),
            Finish: ld!("glFinish"),
            is_gles,
            has_float_linear,
            has_color_buffer_float,
        })
    }

    pub unsafe fn compile_program(&self, vert: &str, frag: &str) -> Result<GLuint, String> {
        let vs = self.compile_shader(GL_VERTEX_SHADER, vert)?;
        let fs = match self.compile_shader(GL_FRAGMENT_SHADER, frag) {
            Ok(id) => id,
            Err(e) => {
                (self.DeleteShader)(vs);
                return Err(e);
            }
        };
        let prog = (self.CreateProgram)();
        (self.AttachShader)(prog, vs);
        (self.AttachShader)(prog, fs);
        (self.LinkProgram)(prog);
        (self.DeleteShader)(vs);
        (self.DeleteShader)(fs);

        let mut ok: GLint = 0;
        (self.GetProgramiv)(prog, GL_LINK_STATUS, &mut ok);
        if ok == 0 {
            let mut buf = vec![0u8; 1024];
            let mut len: GLsizei = 0;
            (self.GetProgramInfoLog)(prog, buf.len() as GLsizei, &mut len, buf.as_mut_ptr() as *mut c_char);
            (self.DeleteProgram)(prog);
            let msg = String::from_utf8_lossy(&buf[..len.max(0) as usize]).to_string();
            return Err(format!("Program link error: {msg}"));
        }
        Ok(prog)
    }

    unsafe fn compile_shader(&self, kind: GLenum, src: &str) -> Result<GLuint, String> {
        let translated;
        let effective_src = if self.is_gles {
            translated = translate_glsl_330_to_300_es(src);
            #[cfg(test)]
            record_gles_shader_if_enabled(kind, &translated);
            translated.as_str()
        } else {
            src
        };
        let sh = (self.CreateShader)(kind);
        let csrc = CString::new(effective_src).unwrap();
        let ptr = csrc.as_ptr();
        (self.ShaderSource)(sh, 1, &ptr, std::ptr::null());
        (self.CompileShader)(sh);
        let mut ok: GLint = 0;
        (self.GetShaderiv)(sh, GL_COMPILE_STATUS, &mut ok);
        if ok == 0 {
            let mut buf = vec![0u8; 2048];
            let mut len: GLsizei = 0;
            (self.GetShaderInfoLog)(sh, buf.len() as GLsizei, &mut len, buf.as_mut_ptr() as *mut c_char);
            (self.DeleteShader)(sh);
            let msg = String::from_utf8_lossy(&buf[..len.max(0) as usize]).to_string();
            return Err(format!("Shader compile error: {msg}"));
        }
        Ok(sh)
    }

    pub unsafe fn uniform_loc(&self, prog: GLuint, name: &str) -> GLint {
        let c = CString::new(name).unwrap();
        (self.GetUniformLocation)(prog, c.as_ptr())
    }

    #[cfg(test)]
    pub(crate) fn force_gles_minimal_caps(&mut self) {
        self.has_float_linear = false;
        self.has_color_buffer_float = false;
        self.TexImage2D = tex_image_2d_gles_safe;
    }
}

#[cfg(test)]
static RECORDED_GLES_SHADERS: std::sync::Mutex<Option<Vec<(GLenum, String)>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
fn record_gles_shader_if_enabled(kind: GLenum, src: &str) {
    if let Ok(mut guard) = RECORDED_GLES_SHADERS.lock() {
        if let Some(vec) = guard.as_mut() {
            vec.push((kind, src.to_string()));
        }
    }
}

#[cfg(test)]
pub(crate) fn start_gles_shader_recording() {
    if let Ok(mut guard) = RECORDED_GLES_SHADERS.lock() {
        *guard = Some(Vec::new());
    }
}

#[cfg(test)]
pub(crate) fn take_recorded_gles_shaders() -> Vec<(GLenum, String)> {
    if let Ok(mut guard) = RECORDED_GLES_SHADERS.lock() {
        guard.take().unwrap_or_default()
    } else {
        Vec::new()
    }
}

#[cfg(target_os = "windows")]
pub struct WglContext {
    pub hwnd: HWND,
    pub hdc: HDC,
    pub hglrc: HGLRC,
}

#[cfg(target_os = "windows")]
impl WglContext {
    pub unsafe fn create(hwnd: HWND) -> Result<Self, String> {
        let hdc = GetDC(hwnd);
        if hdc.is_null() {
            return Err("GetDC failed".into());
        }
        let mut pfd: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
        pfd.nSize = std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16;
        pfd.nVersion = 1;
        pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
        pfd.iPixelType = PFD_TYPE_RGBA;
        pfd.cColorBits = 32;
        pfd.cAlphaBits = 8;
        pfd.cDepthBits = 0;
        pfd.iLayerType = PFD_MAIN_PLANE as u8;

        let pf = ChoosePixelFormat(hdc, &pfd);
        if pf == 0 {
            ReleaseDC(hwnd, hdc);
            return Err("ChoosePixelFormat failed".into());
        }
        if SetPixelFormat(hdc, pf, &pfd) == 0 {
            ReleaseDC(hwnd, hdc);
            return Err("SetPixelFormat failed".into());
        }

        let legacy = wglCreateContext(hdc);
        if legacy.is_null() {
            ReleaseDC(hwnd, hdc);
            return Err("wglCreateContext failed".into());
        }
        if wglMakeCurrent(hdc, legacy) == 0 {
            wglDeleteContext(legacy);
            ReleaseDC(hwnd, hdc);
            return Err("wglMakeCurrent(legacy) failed".into());
        }

        // Upgrade to OpenGL 3.3 Core Profile when supported
        let mut hglrc = legacy;
        let create_arb_name = b"wglCreateContextAttribsARB\0";
        if let Some(proc_ptr) = wglGetProcAddress(create_arb_name.as_ptr()) {
            let create_arb: WglCreateContextAttribsArb = std::mem::transmute(proc_ptr);
            let attribs = [
                WGL_CONTEXT_MAJOR_VERSION_ARB, 3,
                WGL_CONTEXT_MINOR_VERSION_ARB, 3,
                WGL_CONTEXT_PROFILE_MASK_ARB, WGL_CONTEXT_CORE_PROFILE_BIT_ARB,
                0,
            ];
            let core = create_arb(hdc, std::ptr::null_mut(), attribs.as_ptr());
            if !core.is_null() && wglMakeCurrent(hdc, core) != 0 {
                wglDeleteContext(legacy);
                hglrc = core;
            } else if !core.is_null() {
                wglDeleteContext(core);
                wglMakeCurrent(hdc, legacy);
            }
        }

        // Enable VSync if available
        let swap_name = b"wglSwapIntervalEXT\0";
        if let Some(proc_ptr) = wglGetProcAddress(swap_name.as_ptr()) {
            let swap_interval: WglSwapIntervalExt = std::mem::transmute(proc_ptr);
            swap_interval(1);
        }

        Ok(Self { hwnd, hdc, hglrc })
    }

    pub unsafe fn make_current(&self) -> bool {
        wglMakeCurrent(self.hdc, self.hglrc) != 0
    }

    pub unsafe fn swap(&self) {
        SwapBuffers(self.hdc);
    }
}

#[cfg(target_os = "windows")]
impl Drop for WglContext {
    fn drop(&mut self) {
        unsafe {
            wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
            if !self.hglrc.is_null() {
                wglDeleteContext(self.hglrc);
            }
            if !self.hdc.is_null() && !self.hwnd.is_null() {
                ReleaseDC(self.hwnd, self.hdc);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub struct EglHeadlessContext {
    display: *mut c_void,
    surface: *mut c_void,
    context: *mut c_void,
    prev_display: Cell<*mut c_void>,
    prev_draw: Cell<*mut c_void>,
    prev_read: Cell<*mut c_void>,
    prev_context: Cell<*mut c_void>,
    egl_get_current_display: unsafe extern "C" fn() -> *mut c_void,
    egl_get_current_surface: unsafe extern "C" fn(c_int) -> *mut c_void,
    egl_get_current_context: unsafe extern "C" fn() -> *mut c_void,
    egl_make_current: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> c_uint,
    egl_destroy_context: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_uint,
    egl_destroy_surface: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_uint,
    egl_terminate: unsafe extern "C" fn(*mut c_void) -> c_uint,
}

#[cfg(not(target_os = "windows"))]
unsafe impl Send for EglHeadlessContext {}

#[cfg(not(target_os = "windows"))]
impl EglHeadlessContext {
    pub unsafe fn create() -> Result<Self, String> {
        Self::create_with_api(cfg!(target_os = "android"))
    }

    pub unsafe fn create_gles3() -> Result<Self, String> {
        Self::create_with_api(true)
    }

    unsafe fn create_with_api(use_gles3: bool) -> Result<Self, String> {
        const EGL_NONE: c_int = 0x3038;
        const EGL_RED_SIZE: c_int = 0x3024;
        const EGL_GREEN_SIZE: c_int = 0x3023;
        const EGL_BLUE_SIZE: c_int = 0x3022;
        const EGL_ALPHA_SIZE: c_int = 0x3021;
        const EGL_DEPTH_SIZE: c_int = 0x3025;
        const EGL_SURFACE_TYPE: c_int = 0x3033;
        const EGL_PBUFFER_BIT: c_int = 0x0001;
        const EGL_RENDERABLE_TYPE: c_int = 0x3040;
        const EGL_OPENGL_ES2_BIT: c_int = 0x0004;
        const EGL_OPENGL_ES3_BIT: c_int = 0x0040;
        const EGL_OPENGL_BIT: c_int = 0x0008;
        const EGL_OPENGL_ES_API: c_uint = 0x30A0;
        const EGL_OPENGL_API: c_uint = 0x30A2;
        const EGL_WIDTH: c_int = 0x3057;
        const EGL_HEIGHT: c_int = 0x3056;
        const EGL_CONTEXT_MAJOR_VERSION: c_int = 0x3098;
        const EGL_CONTEXT_MINOR_VERSION: c_int = 0x30FB;
        const EGL_CONTEXT_OPENGL_PROFILE_MASK: c_int = 0x30FD;
        const EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT: c_int = 0x0000_0001;
        const EGL_PLATFORM_SURFACELESS_MESA: c_uint = 0x31DD;

        let lib = unix_dl::open_any(&[b"libEGL.so\0", b"libEGL.so.1\0"]);
        if lib.is_null() {
            return Err("Failed to dlopen libEGL.so / libEGL.so.1".into());
        }

        macro_rules! egl_sym {
            ($name:literal) => {{
                let p = unix_dl::dlsym(lib, concat!($name, "\0").as_ptr() as *const c_char);
                if p.is_null() {
                    return Err(format!("Missing EGL symbol: {}", $name));
                }
                std::mem::transmute(p)
            }};
        }

        let egl_get_display: unsafe extern "C" fn(*mut c_void) -> *mut c_void =
            egl_sym!("eglGetDisplay");
        let egl_get_proc: unsafe extern "C" fn(*const c_char) -> *const c_void =
            egl_sym!("eglGetProcAddress");
        let egl_initialize: unsafe extern "C" fn(*mut c_void, *mut c_int, *mut c_int) -> c_uint =
            egl_sym!("eglInitialize");
        let egl_bind_api: unsafe extern "C" fn(c_uint) -> c_uint = egl_sym!("eglBindAPI");
        let egl_choose_config: unsafe extern "C" fn(
            *mut c_void,
            *const c_int,
            *mut *mut c_void,
            c_int,
            *mut c_int,
        ) -> c_uint = egl_sym!("eglChooseConfig");
        let egl_create_pbuffer: unsafe extern "C" fn(
            *mut c_void,
            *mut c_void,
            *const c_int,
        ) -> *mut c_void = egl_sym!("eglCreatePbufferSurface");
        let egl_create_context: unsafe extern "C" fn(
            *mut c_void,
            *mut c_void,
            *mut c_void,
            *const c_int,
        ) -> *mut c_void = egl_sym!("eglCreateContext");
        let egl_get_current_display: unsafe extern "C" fn() -> *mut c_void =
            egl_sym!("eglGetCurrentDisplay");
        let egl_get_current_surface: unsafe extern "C" fn(c_int) -> *mut c_void =
            egl_sym!("eglGetCurrentSurface");
        let egl_get_current_context: unsafe extern "C" fn() -> *mut c_void =
            egl_sym!("eglGetCurrentContext");
        let egl_make_current: unsafe extern "C" fn(
            *mut c_void,
            *mut c_void,
            *mut c_void,
            *mut c_void,
        ) -> c_uint = egl_sym!("eglMakeCurrent");
        let egl_destroy_context: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_uint =
            egl_sym!("eglDestroyContext");
        let egl_destroy_surface: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_uint =
            egl_sym!("eglDestroySurface");
        let egl_terminate: unsafe extern "C" fn(*mut c_void) -> c_uint =
            egl_sym!("eglTerminate");

        // Prefer surfaceless Mesa platform on Linux so headless rendering works without X11/Wayland
        let mut display: *mut c_void = std::ptr::null_mut();
        if !cfg!(target_os = "android") {
            let get_platform_ext_ptr = egl_get_proc(b"eglGetPlatformDisplayEXT\0".as_ptr() as *const c_char);
            if !get_platform_ext_ptr.is_null() {
                let get_platform_ext: unsafe extern "C" fn(
                    c_uint,
                    *mut c_void,
                    *const c_int,
                ) -> *mut c_void = std::mem::transmute(get_platform_ext_ptr);
                display = get_platform_ext(
                    EGL_PLATFORM_SURFACELESS_MESA,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                );
            }
            if display.is_null() {
                let get_platform_ptr = unix_dl::dlsym(lib, b"eglGetPlatformDisplay\0".as_ptr() as *const c_char);
                if !get_platform_ptr.is_null() {
                    let get_platform: unsafe extern "C" fn(
                        c_uint,
                        *mut c_void,
                        *const isize,
                    ) -> *mut c_void = std::mem::transmute(get_platform_ptr);
                    display = get_platform(
                        EGL_PLATFORM_SURFACELESS_MESA,
                        std::ptr::null_mut(),
                        std::ptr::null(),
                    );
                }
            }
        }
        if display.is_null() {
            display = egl_get_display(std::ptr::null_mut());
        }
        if display.is_null() {
            return Err("eglGetDisplay failed".into());
        }

        let mut major = 0;
        let mut minor = 0;
        if egl_initialize(display, &mut major, &mut minor) == 0 {
            return Err("eglInitialize failed".into());
        }
        let target_api = if use_gles3 { EGL_OPENGL_ES_API } else { EGL_OPENGL_API };
        if egl_bind_api(target_api) == 0 {
            egl_terminate(display);
            return Err(format!("eglBindAPI(0x{:X}) failed", target_api));
        }

        let renderable = if use_gles3 { EGL_OPENGL_ES3_BIT } else { EGL_OPENGL_BIT };
        let cfg_attribs = [
            EGL_SURFACE_TYPE, EGL_PBUFFER_BIT,
            EGL_RED_SIZE, 8,
            EGL_GREEN_SIZE, 8,
            EGL_BLUE_SIZE, 8,
            EGL_ALPHA_SIZE, 8,
            EGL_DEPTH_SIZE, 24,
            EGL_RENDERABLE_TYPE, renderable,
            EGL_NONE,
        ];
        let mut config: *mut c_void = std::ptr::null_mut();
        let mut num_cfg: c_int = 0;
        if egl_choose_config(display, cfg_attribs.as_ptr(), &mut config, 1, &mut num_cfg) == 0
            || num_cfg < 1
        {
            // Retry without pbuffer requirement (or ES2 bit for older ES3 drivers)
            let fallback_renderable = if use_gles3 { EGL_OPENGL_ES2_BIT } else { EGL_OPENGL_BIT };
            let cfg_attribs2 = [
                EGL_RED_SIZE, 8,
                EGL_GREEN_SIZE, 8,
                EGL_BLUE_SIZE, 8,
                EGL_ALPHA_SIZE, 8,
                EGL_RENDERABLE_TYPE, fallback_renderable,
                EGL_NONE,
            ];
            if egl_choose_config(display, cfg_attribs2.as_ptr(), &mut config, 1, &mut num_cfg) == 0
                || num_cfg < 1
            {
                egl_terminate(display);
                return Err("eglChooseConfig failed".into());
            }
        }

        let context = if use_gles3 {
            let ctx_attribs = [
                EGL_CONTEXT_MAJOR_VERSION, 3,
                EGL_NONE,
            ];
            egl_create_context(display, config, std::ptr::null_mut(), ctx_attribs.as_ptr())
        } else {
            let ctx_attribs = [
                EGL_CONTEXT_MAJOR_VERSION, 3,
                EGL_CONTEXT_MINOR_VERSION, 3,
                EGL_CONTEXT_OPENGL_PROFILE_MASK, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT,
                EGL_NONE,
            ];
            egl_create_context(display, config, std::ptr::null_mut(), ctx_attribs.as_ptr())
        };
        if context.is_null() {
            egl_terminate(display);
            return Err(if use_gles3 {
                "eglCreateContext(OpenGL ES 3.0) failed".into()
            } else {
                "eglCreateContext(OpenGL 3.3 Core) failed".into()
            });
        }

        const EGL_DRAW: c_int = 0x3059;
        const EGL_READ: c_int = 0x305A;
        let saved_display = egl_get_current_display();
        let saved_draw = egl_get_current_surface(EGL_DRAW);
        let saved_read = egl_get_current_surface(EGL_READ);
        let saved_context = egl_get_current_context();

        // Try surfaceless first (EGL_KHR_surfaceless_context), else create a 1x1 pbuffer.
        let mut surface: *mut c_void = std::ptr::null_mut();
        if egl_make_current(display, std::ptr::null_mut(), std::ptr::null_mut(), context) == 0 {
            let pb_attribs = [EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE];
            surface = egl_create_pbuffer(display, config, pb_attribs.as_ptr());
            if surface.is_null() || egl_make_current(display, surface, surface, context) == 0 {
                if !surface.is_null() {
                    egl_destroy_surface(display, surface);
                }
                egl_destroy_context(display, context);
                egl_terminate(display);
                return Err("eglMakeCurrent failed (surfaceless & pbuffer)".into());
            }
        }

        Ok(Self {
            display,
            surface,
            context,
            prev_display: Cell::new(saved_display),
            prev_draw: Cell::new(saved_draw),
            prev_read: Cell::new(saved_read),
            prev_context: Cell::new(saved_context),
            egl_get_current_display,
            egl_get_current_surface,
            egl_get_current_context,
            egl_make_current,
            egl_destroy_context,
            egl_destroy_surface,
            egl_terminate,
        })
    }

    pub unsafe fn make_current(&self) -> bool {
        const EGL_DRAW: c_int = 0x3059;
        const EGL_READ: c_int = 0x305A;
        let cur_ctx = (self.egl_get_current_context)();
        if cur_ctx != self.context {
            self.prev_display.set((self.egl_get_current_display)());
            self.prev_draw.set((self.egl_get_current_surface)(EGL_DRAW));
            self.prev_read.set((self.egl_get_current_surface)(EGL_READ));
            self.prev_context.set(cur_ctx);
        }
        (self.egl_make_current)(self.display, self.surface, self.surface, self.context) != 0
    }

    pub unsafe fn release_current(&self) {
        let prev_dpy = self.prev_display.replace(std::ptr::null_mut());
        let prev_draw = self.prev_draw.replace(std::ptr::null_mut());
        let prev_read = self.prev_read.replace(std::ptr::null_mut());
        let prev_ctx = self.prev_context.replace(std::ptr::null_mut());
        if !prev_dpy.is_null() && !prev_ctx.is_null() {
            (self.egl_make_current)(prev_dpy, prev_draw, prev_read, prev_ctx);
        } else {
            (self.egl_make_current)(
                self.display,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
impl Drop for EglHeadlessContext {
    fn drop(&mut self) {
        unsafe {
            self.release_current();
            if !self.context.is_null() {
                (self.egl_destroy_context)(self.display, self.context);
            }
            if !self.surface.is_null() {
                (self.egl_destroy_surface)(self.display, self.surface);
            }
            if !self.display.is_null() {
                (self.egl_terminate)(self.display);
            }
        }
    }
}
