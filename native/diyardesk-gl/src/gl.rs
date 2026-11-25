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
    pub ReadPixels: unsafe extern "system" fn(GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, *mut c_void),
    pub Flush: unsafe extern "system" fn(),
    pub Finish: unsafe extern "system" fn(),
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
    pub const RTLD_LOCAL: c_int = 0x0000;
    pub const RTLD_NOLOAD: c_int = 0x0004;

    extern "C" {
        pub fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        pub fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
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
    lib_gl: *mut c_void,
    lib_opengl: *mut c_void,
}

#[cfg(not(target_os = "windows"))]
impl GlProcLoader {
    unsafe fn open() -> Result<Self, String> {
        let lib_egl = unix_dl::open_any(&[b"libEGL.so.1\0", b"libEGL.so\0"]);
        let lib_glx = unix_dl::open_any(&[b"libGLX.so.0\0", b"libGL.so.1\0", b"libGL.so\0"]);
        let lib_gl = unix_dl::open_any(&[b"libGL.so.1\0", b"libGL.so\0"]);
        let lib_opengl = unix_dl::open_any(&[b"libOpenGL.so.0\0", b"libOpenGL.so\0"]);

        let mut egl_get_proc = None;
        let mut prefer_egl = false;
        if !lib_egl.is_null() {
            let p = unix_dl::dlsym(lib_egl, b"eglGetProcAddress\0".as_ptr() as *const c_char);
            if !p.is_null() {
                egl_get_proc = Some(std::mem::transmute(p));
            }
            let cur_p = unix_dl::dlsym(lib_egl, b"eglGetCurrentContext\0".as_ptr() as *const c_char);
            if !cur_p.is_null() {
                let get_cur: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(cur_p);
                prefer_egl = !get_cur().is_null();
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

        if egl_get_proc.is_none() && glx_get_proc.is_none() && lib_gl.is_null() && lib_opengl.is_null() {
            return Err("Could not open libEGL.so.1, libGLX.so.0, libGL.so.1 or libOpenGL.so.0".into());
        }

        Ok(Self {
            egl_get_proc,
            glx_get_proc,
            prefer_egl,
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

        let first = if self.prefer_egl { try_egl() } else { try_glx() };
        if !first.is_null() {
            return Ok(first);
        }
        let second = if self.prefer_egl { try_glx() } else { try_egl() };
        if !second.is_null() {
            return Ok(second);
        }
        for lib in [self.lib_opengl, self.lib_gl, std::ptr::null_mut()] {
            let p = unix_dl::dlsym(lib, cname.as_ptr());
            if !p.is_null() {
                return Ok(p as *const c_void);
            }
        }
        Err(format!("Failed to load OpenGL function: {name}"))
    }
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
        REAL_BIND_FRAMEBUFFER.store(raw_bind_fb as usize, Ordering::Relaxed);
        Ok(Self {
            Viewport: ld!("glViewport"),
            Scissor: ld!("glScissor"),
            Enable: ld!("glEnable"),
            Disable: ld!("glDisable"),
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
            TexImage2D: ld!("glTexImage2D"),
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
            GetIntegerv: ld!("glGetIntegerv"),
            ReadPixels: ld!("glReadPixels"),
            Flush: ld!("glFlush"),
            Finish: ld!("glFinish"),
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
        let sh = (self.CreateShader)(kind);
        let csrc = CString::new(src).unwrap();
        let ptr = csrc.as_ptr();
        (self.ShaderSource)(sh, 1, &ptr, std::ptr::null());
        (self.CompileShader)(sh);
        let mut ok: GLint = 0;
        (self.GetShaderiv)(sh, GL_COMPILE_STATUS, &mut ok);
        if ok == 0 {
            let mut buf = vec![0u8; 1024];
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
        const EGL_NONE: c_int = 0x3038;
        const EGL_RED_SIZE: c_int = 0x3024;
        const EGL_GREEN_SIZE: c_int = 0x3023;
        const EGL_BLUE_SIZE: c_int = 0x3022;
        const EGL_ALPHA_SIZE: c_int = 0x3021;
        const EGL_DEPTH_SIZE: c_int = 0x3025;
        const EGL_SURFACE_TYPE: c_int = 0x3033;
        const EGL_PBUFFER_BIT: c_int = 0x0001;
        const EGL_RENDERABLE_TYPE: c_int = 0x3040;
        const EGL_OPENGL_BIT: c_int = 0x0008;
        const EGL_OPENGL_API: c_uint = 0x30A2;
        const EGL_WIDTH: c_int = 0x3057;
        const EGL_HEIGHT: c_int = 0x3056;
        const EGL_CONTEXT_MAJOR_VERSION: c_int = 0x3098;
        const EGL_CONTEXT_MINOR_VERSION: c_int = 0x30FB;
        const EGL_CONTEXT_OPENGL_PROFILE_MASK: c_int = 0x30FD;
        const EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT: c_int = 0x0000_0001;
        const EGL_PLATFORM_SURFACELESS_MESA: c_uint = 0x31DD;

        let lib = unix_dl::open_any(&[b"libEGL.so.1\0", b"libEGL.so\0"]);
        if lib.is_null() {
            return Err("Failed to dlopen libEGL.so.1".into());
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

        // Prefer surfaceless Mesa platform so headless rendering works without X11/Wayland
        let mut display: *mut c_void = std::ptr::null_mut();
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
        if egl_bind_api(EGL_OPENGL_API) == 0 {
            egl_terminate(display);
            return Err("eglBindAPI(EGL_OPENGL_API) failed".into());
        }

        let cfg_attribs = [
            EGL_SURFACE_TYPE, EGL_PBUFFER_BIT,
            EGL_RED_SIZE, 8,
            EGL_GREEN_SIZE, 8,
            EGL_BLUE_SIZE, 8,
            EGL_ALPHA_SIZE, 8,
            EGL_DEPTH_SIZE, 24,
            EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT,
            EGL_NONE,
        ];
        let mut config: *mut c_void = std::ptr::null_mut();
        let mut num_cfg: c_int = 0;
        if egl_choose_config(display, cfg_attribs.as_ptr(), &mut config, 1, &mut num_cfg) == 0
            || num_cfg < 1
        {
            // Retry without pbuffer requirement (surfaceless-only drivers)
            let cfg_attribs2 = [
                EGL_RED_SIZE, 8,
                EGL_GREEN_SIZE, 8,
                EGL_BLUE_SIZE, 8,
                EGL_ALPHA_SIZE, 8,
                EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT,
                EGL_NONE,
            ];
            if egl_choose_config(display, cfg_attribs2.as_ptr(), &mut config, 1, &mut num_cfg) == 0
                || num_cfg < 1
            {
                egl_terminate(display);
                return Err("eglChooseConfig failed for OpenGL".into());
            }
        }

        let ctx_attribs = [
            EGL_CONTEXT_MAJOR_VERSION, 3,
            EGL_CONTEXT_MINOR_VERSION, 3,
            EGL_CONTEXT_OPENGL_PROFILE_MASK, EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT,
            EGL_NONE,
        ];
        let context = egl_create_context(display, config, std::ptr::null_mut(), ctx_attribs.as_ptr());
        if context.is_null() {
            egl_terminate(display);
            return Err("eglCreateContext(OpenGL 3.3 Core) failed".into());
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
