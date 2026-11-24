// Woodland Glade (Orman Açıklığı) OpenGL 3.3 Core shaders — 1:1 port of
// prototype/glade.html + prototype/glade-trees.js

pub const GLADE_VERT: &str = r#"#version 330 core
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const GLADE_FRAG: &str = r#"#version 330 core
uniform sampler2D uNoise, uFA, uFB;
uniform vec2 uRes, uGrid;
uniform float uScale, uSeed, uStep, uMix, uDebug;
out vec4 o;

const vec3 G_LIGHT = vec3(0.77, 0.85, 0.53);
const vec3 G_MID   = vec3(0.65, 0.78, 0.44);
const vec3 G_DEEP  = vec3(0.51, 0.68, 0.36);
const vec3 INK     = vec3(0.24, 0.39, 0.19);

float hash2(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}
float ss(float a, float b, float x) {
    float t = clamp((x - a) / (b - a), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}
vec4 N(vec2 p, float scale) {
    return texture(uNoise, p / scale + uSeed * 0.013);
}
float fieldS(sampler2D t, vec2 p) {
    return texture(t, (p / uStep + 0.5) / uGrid).r;
}
float segD(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a, ba = b - a;
    return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}

float grassInk(vec2 p) {
    const vec2 G = vec2(12.0, 6.0);
    vec2 c0 = floor(p / G);
    float ink = 0.0;
    for (int j = -1; j <= 1; j++) {
        vec2 c = c0 + vec2(0.0, float(j));
        vec2 cp = (c + 0.5) * G;
        float h = hash2(c + 3.1);
        float dens = 0.72 * ss(0.42, 0.72, N(cp, 900.0).r) * ss(0.35, 0.6, N(cp + 50.0, 380.0).g) + 0.02;
        if (h > dens) continue;
        float ang = (N(cp, 1400.0).g - 0.5) * 0.7 + (hash2(c + 7.7) - 0.5) * 0.3;
        float len = 2.4 + 2.9 * hash2(c + 1.9);
        vec2 ctr = cp + (vec2(hash2(c + 4.3), hash2(c + 5.9)) - 0.5) * vec2(2.0, 3.0);
        vec2 dir = vec2(cos(ang), sin(ang)) * len;
        float d = segD(p, ctr - dir, ctr + dir);
        ink = max(ink, (1.0 - ss(0.2, 0.9, d)) * (0.35 + 0.42 * hash2(c + 9.1)));
    }
    return ink;
}

void main() {
    vec2 p = vec2(gl_FragCoord.x, uRes.y * uScale - gl_FragCoord.y) / uScale;
    float s = mix(fieldS(uFA, p), fieldS(uFB, p), uMix);

    float w1 = N(p, 1900.0).b, w2 = N(p + 311.0, 650.0).g;
    vec3 col = mix(G_MID, G_LIGHT, ss(0.3, 0.75, w1 * 0.7 + w2 * 0.3));
    col = mix(col, G_DEEP, ss(0.45, 0.85, N(p + 977.0, 1300.0).r) * 0.38);

    // Subtle shaded mossy forest floor outside clearing edge
    col = mix(col, col * vec3(0.84, 0.90, 0.82), ss(-4.0, 36.0, s) * 0.72);
    col *= 1.0 + 0.045 * (0.5 - (p.x + p.y) / (uRes.x + uRes.y));
    col *= 1.0 + (hash2(floor(p * (0.8 * max(uScale, 1.0)))) - 0.5) * 0.032;
    col = mix(col, INK, grassInk(p) * 0.72);

    if (uDebug > 0.5) col = mix(col, vec3(0.92, 0.25, 0.2), 1.0 - ss(0.0, 1.6, abs(s)));
    o = vec4(col, 1.0);
}
"#;

pub const GLADE_INST_VERT: &str = r#"#version 330 core
layout(location = 0) in vec4 aInstA; // x, y, scale, variant
layout(location = 1) in vec4 aInstB; // mirror, phase, swayAmt, alpha
uniform vec2 uRes;
uniform float uTime, uWind;
out vec2 vUV;
out float vAlpha;

void main() {
    vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
    // Anchor horizontal center, trunk base at GLADE_BASE_Y (146) / GLADE_SPRITE_H (168)
    vec2 local = vec2(
        (corner.x - 0.5) * 128.0,
        corner.y * 168.0 - 146.0
    );
    local.x *= aInstB.x;
    // Sway more at the treetop, trunk base stays planted
    float heightFrac = clamp((146.0 - corner.y * 168.0) / 146.0, 0.0, 1.0);
    float gust = sin(uTime * (0.95 + 0.55 * uWind) + aInstB.y + aInstA.x * 0.004 + aInstA.y * 0.002);
    local.x += gust * heightFrac * heightFrac * 6.5 * aInstB.z * (0.25 + 0.75 * min(uWind, 2.5));
    vec2 p = aInstA.xy + local * aInstA.z;
    gl_Position = vec4(p.x / uRes.x * 2.0 - 1.0, 1.0 - p.y / uRes.y * 2.0, 0.0, 1.0);
    vUV = vec2((corner.x + aInstA.w) / 11.0, corner.y);
    vAlpha = aInstB.w;
}
"#;

pub const GLADE_INST_FRAG: &str = r#"#version 330 core
uniform sampler2D uAtlas;
in vec2 vUV;
in float vAlpha;
out vec4 o;

void main() {
    vec4 c = texture(uAtlas, vUV);
    float a = c.a * vAlpha;
    if (a < 0.01) discard;
    o = vec4(c.rgb, a);
}
"#;
