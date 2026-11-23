use crate::state::AppState;

// Shared GLSL 330 Core library prepended to every theme fragment shader.
// Provides: hash, value noise, fbm, smin, sdRoundBox, sdSquircle, sdSegment,
// and organic icon-field SDF helpers for real-time desktop icon adaptation.
pub const GLSL_COMMON: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_res;          // CSS px width, height
uniform float u_time;
uniform float u_seed;
uniform vec2 u_pointer;      // CSS px top-left origin
uniform int u_pointer_on;
uniform int u_adaptive;
uniform int u_mode;
uniform int u_palette;
uniform vec4 u_p0;
uniform vec4 u_p1;
uniform vec4 u_p2;
uniform vec4 u_p3;
uniform int u_icon_count;
uniform vec4 u_icons[48];    // x, y, w, h (CSS px, top-left origin)

const float PI = 3.14159265359;
const float TAU = 6.28318530718;

float hash11(float p) {
    p = fract(p * 0.1031);
    p *= p + 33.33;
    p *= p + p;
    return fract(p);
}

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

vec2 hash22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = hash21(i);
    float b = hash21(i + vec2(1.0, 0.0));
    float c = hash21(i + vec2(0.0, 1.0));
    float d = hash21(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    mat2 rot = mat2(0.8, 0.6, -0.6, 0.8);
    for (int i = 0; i < 4; i++) {
        v += a * vnoise(p);
        p = rot * p * 2.03 + vec2(13.7, 7.1);
        a *= 0.5;
    }
    return v;
}

float smin(float a, float b, float k) {
    if (k <= 0.0001) return min(a, b);
    float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

float sdRoundBox(vec2 p, vec2 b, float r) {
    float rr = min(r, min(b.x, b.y));
    vec2 q = abs(p) - b + vec2(rr);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - rr;
}

float sdSegment(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a, ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h);
}

// Smooth organic signed distance to merged desktop icon islands (negative = inside island)
float iconField(vec2 px, float sigma, float warp, float mergeK) {
    if (u_adaptive == 0 || u_icon_count <= 0) return 1e5;
    float d = 1e5;
    for (int i = 0; i < 48; i++) {
        if (i >= u_icon_count) break;
        vec4 ic = u_icons[i];
        vec2 c = ic.xy + ic.zw * 0.5;
        vec2 hs = ic.zw * 0.5 + vec2(sigma * 0.42);
        float sd = sdRoundBox(px - c, hs, min(hs.x, hs.y) * 0.72);
        d = (d > 1e4) ? sd : smin(d, sd, mergeK);
    }
    if (warp > 0.5 && d < 320.0) {
        float n = fbm(px * 0.0085 + vec2(u_seed * 0.013, u_seed * 0.021)) - 0.5;
        d += n * warp * 2.1;
    }
    return d;
}

// Raw unwarped SDF to nearest icon box
float iconBoxField(vec2 px, float pad, float rad, float mergeK) {
    if (u_adaptive == 0 || u_icon_count <= 0) return 1e5;
    float d = 1e5;
    for (int i = 0; i < 48; i++) {
        if (i >= u_icon_count) break;
        vec4 ic = u_icons[i];
        vec2 c = ic.xy + ic.zw * 0.5;
        vec2 hs = ic.zw * 0.5 + vec2(pad);
        float sd = sdRoundBox(px - c, hs, rad);
        d = (d > 1e4) ? sd : smin(d, sd, mergeK);
    }
    return d;
}

vec3 dither(vec3 col) {
    return clamp(col + (hash21(gl_FragCoord.xy) - 0.5) * (1.2 / 255.0), 0.0, 1.0);
}
"#;

pub struct PackedThemeParams {
    pub mode: i32,
    pub palette: i32,
    pub p0: [f32; 4],
    pub p1: [f32; 4],
    pub p2: [f32; 4],
    pub p3: [f32; 4],
}

impl Default for PackedThemeParams {
    fn default() -> Self {
        Self {
            mode: 0,
            palette: 0,
            p0: [1.0, 0.5, 0.5, 0.5],
            p1: [0.5, 0.5, 0.5, 0.5],
            p2: [0.5, 0.5, 0.5, 0.5],
            p3: [0.5, 0.5, 0.5, 0.5],
        }
    }
}

pub fn str_index(st: &AppState, key: &str, opts: &[&str], default_idx: i32) -> i32 {
    if let Some(s) = st.param_str(key) {
        for (i, &o) in opts.iter().enumerate() {
            if s.eq_ignore_ascii_case(o) {
                return i as i32;
            }
        }
    }
    let n = st.param_f32(key, default_idx as f32).round() as i32;
    n.clamp(0, (opts.len() as i32 - 1).max(0))
}
