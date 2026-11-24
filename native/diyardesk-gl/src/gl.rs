#![allow(non_snake_case, dead_code)]

use std::ffi::{c_char, c_int, c_uint, c_void, CString};
use windows_sys::Win32::Foundation::{HMODULE, HWND};
use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC, HDC};
use windows_sys::Win32::Graphics::OpenGL::{
    ChoosePixelFormat, SetPixelFormat, SwapBuffers, wglCreateContext, wglDeleteContext,
    wglGetProcAddress, wglMakeCurrent, HGLRC, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW,
    PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
};
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

const WGL_CONTEXT_MAJOR_VERSION_ARB: c_int = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: c_int = 0x2092;
const WGL_CONTEXT_PROFILE_MASK_ARB: c_int = 0x9126;
const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: c_int = 0x0000_0001;

type WglCreateContextAttribsArb =
    unsafe extern "system" fn(HDC, HGLRC, *const c_int) -> HGLRC;
type WglSwapIntervalExt = unsafe extern "system" fn(c_int) -> c_int;

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
    pub ReadPixels: unsafe extern "system" fn(GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, *mut c_void),
    pub Finish: unsafe extern "system" fn(),
}

unsafe fn load_proc(opengl32: HMODULE, name: &str) -> Result<*const c_void, String> {
    let cname = CString::new(name).unwrap();
    let ptr = wglGetProcAddress(cname.as_ptr() as *const u8);
    if let Some(f) = ptr {
        let addr = f as usize;
        if addr != 0 && addr != 1 && addr != 2 && addr != 3 && addr != usize::MAX {
            return Ok(f as *const c_void);
        }
    }
    let ptr2 = GetProcAddress(opengl32, cname.as_ptr() as *const u8);
    if let Some(f) = ptr2 {
        return Ok(f as *const c_void);
    }
    Err(format!("Failed to load OpenGL function: {name}"))
}

impl GlFns {
    pub unsafe fn load() -> Result<Self, String> {
        let lib_name = b"opengl32.dll\0";
        let opengl32 = LoadLibraryA(lib_name.as_ptr());
        if opengl32.is_null() {
            return Err("LoadLibraryA(opengl32.dll) failed".into());
        }
        macro_rules! ld {
            ($name:literal) => {
                std::mem::transmute(load_proc(opengl32, $name)?)
            };
        }
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
            BindFramebuffer: ld!("glBindFramebuffer"),
            FramebufferTexture2D: ld!("glFramebufferTexture2D"),
            GenRenderbuffers: ld!("glGenRenderbuffers"),
            DeleteRenderbuffers: ld!("glDeleteRenderbuffers"),
            BindRenderbuffer: ld!("glBindRenderbuffer"),
            RenderbufferStorage: ld!("glRenderbufferStorage"),
            FramebufferRenderbuffer: ld!("glFramebufferRenderbuffer"),
            ReadPixels: ld!("glReadPixels"),
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

pub struct WglContext {
    pub hwnd: HWND,
    pub hdc: HDC,
    pub hglrc: HGLRC,
}

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
