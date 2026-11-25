//! Minimal zero-dependency DiyarDesk Rust + OpenGL 3.3 Core `cdylib` theme plugin.
//! Implements `DDGL_PLUGIN_ABI_VERSION = 1`.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};

type GLenum = c_uint;
type GLuint = c_uint;
type GLint = c_int;
type GLsizei = c_int;
type GLbitfield = c_uint;
type GlGetProcFn = unsafe extern "C" fn(*const c_char) -> *const c_void;

const GL_COLOR_BUFFER_BIT: GLbitfield = 0x4000;
const GL_TRIANGLES: GLenum = 0x0004;
const GL_VERTEX_SHADER: GLenum = 0x8B31;
const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
const GL_COMPILE_STATUS: GLenum = 0x8B81;
const GL_LINK_STATUS: GLenum = 0x8B82;

const VERT_SRC: &str = r#"#version 330 core
out vec2 v_uv;
void main() {
    vec2 p = vec2((gl_VertexID == 1) ? 3.0 : -1.0, (gl_VertexID == 2) ? 3.0 : -1.0);
    v_uv = p * 0.5 + 0.5;
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;

const FRAG_SRC: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;
uniform vec2 u_res;
uniform float u_time;
uniform float u_speed;
uniform float u_intensity;
void main() {
    float t = u_time * u_speed;
    vec2 uv = v_uv;
    vec3 sky = mix(vec3(0.03, 0.05, 0.11), vec3(0.06, 0.13, 0.22), uv.y);
    float band = sin(uv.x * 6.0 + t * 0.9) * 0.14 + cos(uv.x * 11.0 - t * 0.6) * 0.06;
    float d = abs(uv.y - (0.58 + band));
    float ribbon = exp(-d * 10.5) * u_intensity;
    vec3 aurora = mix(vec3(0.15, 0.90, 0.68), vec3(0.55, 0.35, 0.95), uv.x);
    vec3 col = sky + aurora * ribbon * 0.72;
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

struct PluginCtx {
    width: i32,
    height: i32,
    time: f32,
    speed: f32,
    intensity: f32,
    vao: GLuint,
    prog: GLuint,
    Viewport: unsafe extern "system" fn(GLint, GLint, GLsizei, GLsizei),
    ClearColor: unsafe extern "system" fn(f32, f32, f32, f32),
    Clear: unsafe extern "system" fn(GLbitfield),
    UseProgram: unsafe extern "system" fn(GLuint),
    BindVertexArray: unsafe extern "system" fn(GLuint),
    DrawArrays: unsafe extern "system" fn(GLenum, GLint, GLsizei),
    Uniform1f: unsafe extern "system" fn(GLint, f32),
    Uniform2f: unsafe extern "system" fn(GLint, f32, f32),
    GetUniformLocation: unsafe extern "system" fn(GLuint, *const c_char) -> GLint,
    DeleteProgram: unsafe extern "system" fn(GLuint),
    DeleteVertexArrays: unsafe extern "system" fn(GLsizei, *const GLuint),
}

unsafe fn load_fn<T>(loader: GlGetProcFn, name: &str) -> Option<T> {
    let cname = CString::new(name).ok()?;
    let ptr = loader(cname.as_ptr());
    if ptr.is_null() {
        return None;
    }
    Some(std::mem::transmute_copy(&ptr))
}

fn extract_json_f32(json: &str, key: &str) -> Option<f32> {
    let needle = format!("\"{key}\"");
    let pos = json.find(&needle)?;
    let rest = &json[pos + needle.len()..];
    let colon = rest.find(':')?;
    let tail = rest[colon + 1..].trim_start();
    let end = tail
        .find(|c: char| !matches!(c, '0'..='9' | '.' | '-' | '+' | 'e' | 'E'))
        .unwrap_or(tail.len());
    tail[..end].parse::<f32>().ok()
}

#[no_mangle]
pub extern "C" fn ddgl_plugin_abi_version() -> u32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_plugin_create(
    gl_loader: GlGetProcFn,
    width_px: i32,
    height_px: i32,
    _dpr: f32,
    _assets_dir_utf8: *const c_char,
) -> *mut c_void {
    let (
        Some(Viewport),
        Some(ClearColor),
        Some(Clear),
        Some(GenVertexArrays),
        Some(BindVertexArray),
        Some(DeleteVertexArrays),
        Some(CreateShader),
        Some(ShaderSource),
        Some(CompileShader),
        Some(GetShaderiv),
        Some(DeleteShader),
        Some(CreateProgram),
        Some(AttachShader),
        Some(LinkProgram),
        Some(GetProgramiv),
        Some(UseProgram),
        Some(DeleteProgram),
        Some(DrawArrays),
        Some(Uniform1f),
        Some(Uniform2f),
        Some(GetUniformLocation),
    ) = (
        load_fn(gl_loader, "glViewport"),
        load_fn(gl_loader, "glClearColor"),
        load_fn(gl_loader, "glClear"),
        load_fn::<unsafe extern "system" fn(GLsizei, *mut GLuint)>(gl_loader, "glGenVertexArrays"),
        load_fn(gl_loader, "glBindVertexArray"),
        load_fn(gl_loader, "glDeleteVertexArrays"),
        load_fn::<unsafe extern "system" fn(GLenum) -> GLuint>(gl_loader, "glCreateShader"),
        load_fn::<unsafe extern "system" fn(GLuint, GLsizei, *const *const c_char, *const GLint)>(
            gl_loader,
            "glShaderSource",
        ),
        load_fn::<unsafe extern "system" fn(GLuint)>(gl_loader, "glCompileShader"),
        load_fn::<unsafe extern "system" fn(GLuint, GLenum, *mut GLint)>(gl_loader, "glGetShaderiv"),
        load_fn::<unsafe extern "system" fn(GLuint)>(gl_loader, "glDeleteShader"),
        load_fn::<unsafe extern "system" fn() -> GLuint>(gl_loader, "glCreateProgram"),
        load_fn::<unsafe extern "system" fn(GLuint, GLuint)>(gl_loader, "glAttachShader"),
        load_fn::<unsafe extern "system" fn(GLuint)>(gl_loader, "glLinkProgram"),
        load_fn::<unsafe extern "system" fn(GLuint, GLenum, *mut GLint)>(gl_loader, "glGetProgramiv"),
        load_fn(gl_loader, "glUseProgram"),
        load_fn::<unsafe extern "system" fn(GLuint)>(gl_loader, "glDeleteProgram"),
        load_fn(gl_loader, "glDrawArrays"),
        load_fn(gl_loader, "glUniform1f"),
        load_fn(gl_loader, "glUniform2f"),
        load_fn(gl_loader, "glGetUniformLocation"),
    ) else {
        return std::ptr::null_mut();
    };

    let compile = |kind: GLenum, src: &str| -> GLuint {
        let sh = CreateShader(kind);
        let csrc = CString::new(src).unwrap();
        let ptr = csrc.as_ptr();
        ShaderSource(sh, 1, &ptr, std::ptr::null());
        CompileShader(sh);
        let mut ok = 0;
        GetShaderiv(sh, GL_COMPILE_STATUS, &mut ok);
        if ok == 0 {
            DeleteShader(sh);
            return 0;
        }
        sh
    };

    let vs = compile(GL_VERTEX_SHADER, VERT_SRC);
    let fs = compile(GL_FRAGMENT_SHADER, FRAG_SRC);
    if vs == 0 || fs == 0 {
        if vs != 0 { DeleteShader(vs); }
        if fs != 0 { DeleteShader(fs); }
        return std::ptr::null_mut();
    }
    let prog = CreateProgram();
    AttachShader(prog, vs);
    AttachShader(prog, fs);
    LinkProgram(prog);
    DeleteShader(vs);
    DeleteShader(fs);
    let mut linked = 0;
    GetProgramiv(prog, GL_LINK_STATUS, &mut linked);
    if linked == 0 {
        DeleteProgram(prog);
        return std::ptr::null_mut();
    }

    let mut vao = 0;
    GenVertexArrays(1, &mut vao);

    Box::into_raw(Box::new(PluginCtx {
        width: width_px.max(16),
        height: height_px.max(16),
        time: 0.0,
        speed: 1.0,
        intensity: 1.0,
        vao,
        prog,
        Viewport,
        ClearColor,
        Clear,
        UseProgram,
        BindVertexArray,
        DrawArrays,
        Uniform1f,
        Uniform2f,
        GetUniformLocation,
        DeleteProgram,
        DeleteVertexArrays,
    })) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_plugin_resize(
    ctx: *mut c_void,
    width_px: i32,
    height_px: i32,
    _dpr: f32,
) {
    if ctx.is_null() {
        return;
    }
    let c = &mut *(ctx as *mut PluginCtx);
    c.width = width_px.max(16);
    c.height = height_px.max(16);
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_plugin_post_json(ctx: *mut c_void, json_utf8: *const c_char) {
    if ctx.is_null() || json_utf8.is_null() {
        return;
    }
    let c = &mut *(ctx as *mut PluginCtx);
    let Ok(s) = CStr::from_ptr(json_utf8).to_str() else {
        return;
    };
    if let Some(v) = extract_json_f32(s, "speed") {
        c.speed = v.clamp(0.1, 4.0);
    }
    if let Some(v) = extract_json_f32(s, "intensity") {
        c.intensity = v.clamp(0.1, 3.0);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_plugin_render(ctx: *mut c_void, dt: f32) {
    if ctx.is_null() {
        return;
    }
    let c = &mut *(ctx as *mut PluginCtx);
    c.time += dt.clamp(0.0, 0.25);

    (c.Viewport)(0, 0, c.width, c.height);
    (c.ClearColor)(0.03, 0.05, 0.11, 1.0);
    (c.Clear)(GL_COLOR_BUFFER_BIT);
    (c.UseProgram)(c.prog);
    let l_res = (c.GetUniformLocation)(c.prog, b"u_res\0".as_ptr() as *const c_char);
    if l_res >= 0 { (c.Uniform2f)(l_res, c.width as f32, c.height as f32); }
    let l_time = (c.GetUniformLocation)(c.prog, b"u_time\0".as_ptr() as *const c_char);
    if l_time >= 0 { (c.Uniform1f)(l_time, c.time); }
    let l_spd = (c.GetUniformLocation)(c.prog, b"u_speed\0".as_ptr() as *const c_char);
    if l_spd >= 0 { (c.Uniform1f)(l_spd, c.speed); }
    let l_int = (c.GetUniformLocation)(c.prog, b"u_intensity\0".as_ptr() as *const c_char);
    if l_int >= 0 { (c.Uniform1f)(l_int, c.intensity); }
    (c.BindVertexArray)(c.vao);
    (c.DrawArrays)(GL_TRIANGLES, 0, 3);
}

#[no_mangle]
pub unsafe extern "C" fn ddgl_plugin_destroy(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let c = Box::from_raw(ctx as *mut PluginCtx);
    if c.prog != 0 {
        (c.DeleteProgram)(c.prog);
    }
    if c.vao != 0 {
        (c.DeleteVertexArrays)(1, &c.vao);
    }
}
