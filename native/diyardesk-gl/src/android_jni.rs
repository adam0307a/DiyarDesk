//! Zero-dependency JNI bindings for Android (`com.diyardesk.tablet.NativeGlBridge`).
//!
//! Exports standard JNI entry points wrapping `GlEmbeddedEngine` so Android's
//! `DiyarWallpaperService` can render all native Rust themes directly onto
//! `WallpaperService.SurfaceHolder` via EGL 1.4 + OpenGL ES 3.0 without any
//! WebView or VirtualDisplay overhead.
//!
//! Also compiled on Linux so OpenJDK (`javac`/`java`) integration tests can
//! exercise the exact JNI vtable layout and marshaling against a live JVM.

#![allow(non_snake_case)]

use crate::{
    create_embedded_engine_checked, ddgl_embedded_destroy, ddgl_embedded_mouse,
    ddgl_embedded_poll_event, ddgl_embedded_post_json, ddgl_embedded_render,
    ddgl_embedded_render_png, ddgl_embedded_resize, ddgl_embedded_set_low_mode,
    ddgl_embedded_set_paused, GlEmbeddedEngine, NATIVE_GL_THEMES,
};
use std::ffi::{c_char, c_void, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

pub type JniEnvPtr = *mut *const *const c_void;
pub type JClass = *mut c_void;
pub type JString = *mut c_void;
pub type JChar = u16;
pub type JBoolean = u8;
pub type JInt = i32;
pub type JLong = i64;
pub type JFloat = f32;

const JNI_FALSE: JBoolean = 0;
const JNI_TRUE: JBoolean = 1;

// ABI-frozen JNINativeInterface function table indices (verified against OpenJDK 21 & Android NDK jni.h).
// Using UTF-16 NewString/GetStringChars avoids Android ART CheckJNI aborts on 4-byte UTF-8 (non-Modified UTF-8)
// characters such as emojis or supplementary Unicode code points.
const JNI_NEW_STRING_IDX: usize = 163;
const JNI_GET_STRING_LENGTH_IDX: usize = 164;
const JNI_GET_STRING_CHARS_IDX: usize = 165;
const JNI_RELEASE_STRING_CHARS_IDX: usize = 166;

type NewStringFn = unsafe extern "system" fn(JniEnvPtr, *const JChar, JInt) -> JString;
type GetStringLengthFn = unsafe extern "system" fn(JniEnvPtr, JString) -> JInt;
type GetStringCharsFn =
    unsafe extern "system" fn(JniEnvPtr, JString, *mut JBoolean) -> *const JChar;
type ReleaseStringCharsFn = unsafe extern "system" fn(JniEnvPtr, JString, *const JChar);

static LAST_JNI_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn set_last_error(msg: impl Into<String>) {
    let s = msg.into();
    log_android_error(&s);
    if let Ok(mut guard) = LAST_JNI_ERROR.lock() {
        *guard = Some(s);
    }
}

fn clear_last_error() {
    if let Ok(mut guard) = LAST_JNI_ERROR.lock() {
        *guard = None;
    }
}

fn take_last_error() -> Option<String> {
    LAST_JNI_ERROR.lock().ok().and_then(|g| g.clone())
}

#[cfg(target_os = "android")]
fn log_android_error(msg: &str) {
    use std::ffi::c_int;
    use std::sync::OnceLock;
    type AndroidLogWriteFn = unsafe extern "C" fn(c_int, *const c_char, *const c_char) -> c_int;
    extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    static LOG_FN: OnceLock<Option<AndroidLogWriteFn>> = OnceLock::new();
    let f = *LOG_FN.get_or_init(|| unsafe {
        let h = dlopen(b"liblog.so\0".as_ptr() as *const c_char, 1);
        if h.is_null() {
            return None;
        }
        let sym = dlsym(h, b"__android_log_write\0".as_ptr() as *const c_char);
        if sym.is_null() {
            None
        } else {
            Some(std::mem::transmute(sym))
        }
    });
    if let Some(write_fn) = f {
        let sanitized = msg.replace('\0', " ");
        if let Ok(cmsg) = CString::new(sanitized) {
            unsafe {
                // ANDROID_LOG_ERROR = 6
                write_fn(6, b"DiyarDeskGL\0".as_ptr() as *const c_char, cmsg.as_ptr());
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
fn log_android_error(_msg: &str) {}

unsafe fn jstring_to_string(env: JniEnvPtr, jstr: JString) -> Option<String> {
    if env.is_null() || jstr.is_null() {
        return None;
    }
    let vtable = *env;
    if vtable.is_null() {
        return None;
    }
    let len_fn: GetStringLengthFn = std::mem::transmute(*vtable.add(JNI_GET_STRING_LENGTH_IDX));
    let get_fn: GetStringCharsFn = std::mem::transmute(*vtable.add(JNI_GET_STRING_CHARS_IDX));
    let rel_fn: ReleaseStringCharsFn =
        std::mem::transmute(*vtable.add(JNI_RELEASE_STRING_CHARS_IDX));

    let len = len_fn(env, jstr);
    if len <= 0 {
        return Some(String::new());
    }
    let chars = get_fn(env, jstr, std::ptr::null_mut());
    if chars.is_null() {
        return None;
    }
    let slice = std::slice::from_raw_parts(chars, len as usize);
    let out = String::from_utf16_lossy(slice);
    rel_fn(env, jstr, chars);
    Some(out)
}

unsafe fn string_to_jstring(env: JniEnvPtr, s: &str) -> JString {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    let vtable = *env;
    if vtable.is_null() {
        return std::ptr::null_mut();
    }
    let new_fn: NewStringFn = std::mem::transmute(*vtable.add(JNI_NEW_STRING_IDX));
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let ptr = if utf16.is_empty() {
        // JNI NewString allows non-null pointer with length 0 (some VMs check pointer != NULL).
        static EMPTY_U16: JChar = 0;
        &EMPTY_U16 as *const JChar
    } else {
        utf16.as_ptr()
    };
    new_fn(env, ptr, utf16.len() as JInt)
}

#[inline]
fn handle_to_ptr(handle: JLong) -> *mut GlEmbeddedEngine {
    if handle == 0 {
        std::ptr::null_mut()
    } else {
        handle as usize as *mut GlEmbeddedEngine
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeIsNativeTheme(
    env: JniEnvPtr,
    _clazz: JClass,
    theme_id: JString,
) -> JBoolean {
    let res = catch_unwind(AssertUnwindSafe(|| {
        let Some(id) = jstring_to_string(env, theme_id) else {
            return JNI_FALSE;
        };
        let clean = id.trim();
        if NATIVE_GL_THEMES
            .iter()
            .any(|t| t.eq_ignore_ascii_case(clean))
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }));
    res.unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeThemesCsv(
    env: JniEnvPtr,
    _clazz: JClass,
) -> JString {
    let res = catch_unwind(AssertUnwindSafe(|| {
        let csv = NATIVE_GL_THEMES.join(",");
        string_to_jstring(env, &csv)
    }));
    res.unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeCreate(
    _env: JniEnvPtr,
    _clazz: JClass,
    width: JInt,
    height: JInt,
    dpi_scale: JFloat,
) -> JLong {
    let res = catch_unwind(AssertUnwindSafe(|| {
        clear_last_error();
        match create_embedded_engine_checked(width, height, dpi_scale) {
            Ok(ptr) => ptr as usize as JLong,
            Err(err) => {
                set_last_error(err);
                0
            }
        }
    }));
    match res {
        Ok(h) => h,
        Err(_) => {
            set_last_error("Panic during nativeCreate");
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeDestroy(
    _env: JniEnvPtr,
    _clazz: JClass,
    handle: JLong,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let ptr = handle_to_ptr(handle);
        if !ptr.is_null() {
            ddgl_embedded_destroy(ptr);
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeResize(
    _env: JniEnvPtr,
    _clazz: JClass,
    handle: JLong,
    width: JInt,
    height: JInt,
    dpi_scale: JFloat,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let ptr = handle_to_ptr(handle);
        if !ptr.is_null() {
            ddgl_embedded_resize(ptr, width, height, dpi_scale);
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeSetPaused(
    _env: JniEnvPtr,
    _clazz: JClass,
    handle: JLong,
    paused: JBoolean,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let ptr = handle_to_ptr(handle);
        if !ptr.is_null() {
            ddgl_embedded_set_paused(ptr, if paused != 0 { 1 } else { 0 });
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeSetLowMode(
    _env: JniEnvPtr,
    _clazz: JClass,
    handle: JLong,
    low: JBoolean,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let ptr = handle_to_ptr(handle);
        if !ptr.is_null() {
            ddgl_embedded_set_low_mode(ptr, if low != 0 { 1 } else { 0 });
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativePostJson(
    env: JniEnvPtr,
    _clazz: JClass,
    handle: JLong,
    json: JString,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let ptr = handle_to_ptr(handle);
        if ptr.is_null() {
            return;
        }
        let Some(text) = jstring_to_string(env, json) else {
            return;
        };
        let sanitized = if text.as_bytes().contains(&0) {
            text.replace('\0', " ")
        } else {
            text
        };
        if let Ok(cstr) = CString::new(sanitized) {
            ddgl_embedded_post_json(ptr, cstr.as_ptr());
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativePointer(
    _env: JniEnvPtr,
    _clazz: JClass,
    handle: JLong,
    kind: JInt,
    css_x: JFloat,
    css_y: JFloat,
) -> JBoolean {
    let res = catch_unwind(AssertUnwindSafe(|| {
        let ptr = handle_to_ptr(handle);
        if ptr.is_null() {
            return JNI_FALSE;
        }
        if ddgl_embedded_mouse(ptr, kind, css_x, css_y) != 0 {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }));
    res.unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeRender(
    _env: JniEnvPtr,
    _clazz: JClass,
    handle: JLong,
    dt: JFloat,
    target_fbo: JInt,
) -> JBoolean {
    let res = catch_unwind(AssertUnwindSafe(|| {
        let ptr = handle_to_ptr(handle);
        if ptr.is_null() {
            return JNI_FALSE;
        }
        let fbo = if target_fbo < 0 {
            u32::MAX
        } else {
            target_fbo as u32
        };
        if ddgl_embedded_render(ptr, dt, fbo) != 0 {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }));
    match res {
        Ok(ok) => ok,
        Err(_) => {
            set_last_error("Panic during nativeRender");
            JNI_FALSE
        }
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativePollEvent(
    env: JniEnvPtr,
    _clazz: JClass,
    handle: JLong,
) -> JString {
    let res = catch_unwind(AssertUnwindSafe(|| {
        let ptr = handle_to_ptr(handle);
        if ptr.is_null() {
            return std::ptr::null_mut();
        }
        let mut buf = vec![0u8; 8192];
        let n = ddgl_embedded_poll_event(ptr, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
        if n <= 0 {
            return std::ptr::null_mut();
        }
        let s = String::from_utf8_lossy(&buf[..n as usize]);
        string_to_jstring(env, &s)
    }));
    res.unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeRenderPngToPath(
    env: JniEnvPtr,
    _clazz: JClass,
    handle: JLong,
    target_fbo: JInt,
    out_path: JString,
) -> JBoolean {
    let res = catch_unwind(AssertUnwindSafe(|| {
        let ptr = handle_to_ptr(handle);
        if ptr.is_null() {
            return JNI_FALSE;
        }
        let Some(path) = jstring_to_string(env, out_path) else {
            return JNI_FALSE;
        };
        let Ok(cpath) = CString::new(path) else {
            return JNI_FALSE;
        };
        let fbo = if target_fbo < 0 {
            u32::MAX
        } else {
            target_fbo as u32
        };
        if ddgl_embedded_render_png(ptr, fbo, cpath.as_ptr()) != 0 {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }));
    res.unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_diyardesk_tablet_NativeGlBridge_nativeLastError(
    env: JniEnvPtr,
    _clazz: JClass,
) -> JString {
    let res = catch_unwind(AssertUnwindSafe(|| {
        let Some(msg) = take_last_error() else {
            return std::ptr::null_mut();
        };
        string_to_jstring(env, &msg)
    }));
    res.unwrap_or(std::ptr::null_mut())
}
