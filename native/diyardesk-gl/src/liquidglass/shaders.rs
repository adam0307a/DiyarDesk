// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Liquid Glass (Refractive Surface)
// Ported 1:1 from prototype/liquidglass.html (WebGL2 GLSL ES 3.00 -> GLSL 330 core)
//
// Optical shader math & LCH color pipeline adapted from:
//   iyinchao/liquid-glass-studio (MIT License, Copyright (c) 2024 Charles Yin)
//   https://github.com/iyinchao/liquid-glass-studio

// Fullscreen vertex shader (matches FULLSCREEN_VERT, kept local for standalone compile tests)
pub const LG_VERT: &str = r#"#version 330 core
layout (location = 0) in vec2 a_pos;
out vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
"#;

// Shared SDF + polynomial smin library (1:1 SHARED_SDF_GLSL)
pub const LG_SHARED_SDF: &str = r#"
#define MAX_DROPS 10
#define MAX_ICONS 48

uniform vec2  u_resolution;
uniform float u_dpr;
uniform float u_mergePx;
uniform int   u_dropCount;
uniform vec4  u_drops[MAX_DROPS];       // cx, cy, halfW, halfH (CSS px, top-left origin)
uniform float u_dropRound[MAX_DROPS];   // corner radius (CSS px)
uniform int   u_cursorOn;
uniform vec4  u_cursor;                 // cx, cy, halfW, halfH (CSS px)
uniform float u_cursorRadius;
uniform int   u_iconCount;
uniform int   u_iconStyle;              // 0: capsule, 1: droplet, 2: prism
uniform vec4  u_icons[MAX_ICONS];       // cx, cy, halfW, halfH (CSS px)

float smin(float a, float b, float k) {
    if (k <= 0.0001) return min(a, b);
    float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

float sdSuperellipseCorner(vec2 p, float r, float n) {
    p = abs(p);
    float v = pow(pow(p.x, n) + pow(p.y, n), 1.0 / n);
    return v - r;
}

float sdSquircleBox(vec2 p, vec2 c, vec2 halfSize, float cr, float n) {
    vec2 q = p - c;
    cr = min(cr, min(halfSize.x, halfSize.y));
    vec2 d = abs(q) - halfSize;
    if (d.x > -cr && d.y > -cr) {
        vec2 cc = sign(q) * (halfSize - vec2(cr));
        return sdSuperellipseCorner(q - cc, cr, n);
    }
    return min(max(d.x, d.y), 0.0) + length(max(d, 0.0));
}

float sdRoundBoxFast(vec2 p, vec2 c, vec2 halfSize, float cr) {
    vec2 q = abs(p - c) - (halfSize - vec2(cr));
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - cr;
}

// Signed distance in CSS pixels (negative inside glass)
float sceneSDF(vec2 cssP) {
    float k = u_mergePx;
    float d = 1e5;

    // 1. Floating liquid glass droplets & smooth pill capsules
    for (int i = 0; i < MAX_DROPS; i++) {
        if (i >= u_dropCount) break;
        vec4 dr = u_drops[i];
        vec2 hs = dr.zw;
        vec2 bb = abs(cssP - dr.xy) - hs;
        float bound = max(bb.x, bb.y);
        if (bound < d + k * 2.2) {
            float sd = sdSquircleBox(cssP, dr.xy, hs, u_dropRound[i], 2.15);
            d = (d > 1e4) ? sd : smin(d, sd, k);
        }
    }

    // 2. Spring-physics cursor lens
    if (u_cursorOn == 1) {
        vec2 hs = u_cursor.zw;
        vec2 bb = abs(cssP - u_cursor.xy) - hs;
        if (max(bb.x, bb.y) < d + k * 2.5) {
            float sd = sdSquircleBox(cssP, u_cursor.xy, hs, u_cursorRadius, 2.35);
            d = (d > 1e4) ? sd : smin(d, sd, k * 1.2);
        }
    }

    // 3. Adaptive desktop icon glass cradles
    float iconK = k * (u_iconStyle == 2 ? 0.35 : 0.72);
    for (int i = 0; i < MAX_ICONS; i++) {
        if (i >= u_iconCount) break;
        vec4 ic = u_icons[i];
        vec2 hs = ic.zw;
        vec2 bb = abs(cssP - ic.xy) - hs;
        if (max(bb.x, bb.y) < min(d, 120.0) + iconK * 2.2) {
            float sd;
            if (u_iconStyle == 1) {
                float rad = min(hs.x, hs.y) * 0.96;
                sd = sdSquircleBox(cssP, ic.xy, hs * 0.94, rad, 2.4);
            } else if (u_iconStyle == 2) {
                sd = sdRoundBoxFast(cssP, ic.xy, hs, 14.0);
            } else {
                sd = sdSquircleBox(cssP, ic.xy, hs, min(hs.x, hs.y) * 0.44, 4.2);
            }
            d = (d > 1e4) ? sd : smin(d, sd, iconK);
        }
    }

    return d;
}
"#;

// Pass 0: silk optical backdrop + glass cast shadows (1:1 BG_FRAG_SRC)
pub const LG_BG_FRAG: &str = r#"#version 330 core
precision highp float;
in vec2 v_uv;
out vec4 fragColor;

uniform float u_time;
uniform int   u_palette;
uniform float u_shadowFactor;

__SHARED_SDF__

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

vec3 paletteBackdrop(vec2 uv, float t, int pal) {
    float asp = u_resolution.x / max(1.0, u_resolution.y);
    vec2 p = (uv - 0.5) * vec2(asp, 1.0);

    if (pal == 4) {
        vec3 col = vec3(0.043, 0.145, 0.271);
        vec3 lobes[4];
        lobes[0] = vec3(0.220, 0.741, 0.973);
        lobes[1] = vec3(0.176, 0.831, 0.749);
        lobes[2] = vec3(0.231, 0.510, 0.965);
        lobes[3] = vec3(0.729, 0.902, 0.992);
        for (int i = 0; i < 4; i++) {
            float fi = float(i);
            vec2 c = vec2(
                0.18 + 0.22 * fi + sin(t * (0.42 + fi * 0.11) + fi * 1.3) * 0.18,
                0.50 + cos(t * (0.36 + fi * 0.13) + fi * 1.7) * 0.34
            );
            vec2 dv = (uv - c) * vec2(asp, 1.0);
            float rad = (0.48 + 0.08 * fi) * max(asp, 1.0);
            float a = clamp(1.0 - length(dv) / rad, 0.0, 1.0);
            a = a * a * (3.0 - 2.0 * a) * 0.62;
            col = mix(col, lobes[i], a);
        }
        col += (hash21(gl_FragCoord.xy) - 0.5) * (1.2 / 255.0);
        return clamp(col, 0.0, 1.0);
    }

    vec2 q = p;
    q.x += 0.22 * sin(p.y * 2.4 + t * 0.42) + 0.10 * cos(p.x * 3.1 - t * 0.31);
    q.y += 0.20 * cos(p.x * 2.1 - t * 0.36) + 0.09 * sin(p.y * 3.4 + t * 0.27);

    float w1 = 0.5 + 0.5 * sin(q.x * 2.8 + q.y * 2.2 + t * 0.35);
    float w2 = 0.5 + 0.5 * cos(q.x * 3.7 - q.y * 2.9 - t * 0.28);
    float w3 = 0.5 + 0.5 * sin((q.x - q.y) * 4.5 + t * 0.22);

    float ribbonA = exp(-pow(abs(q.y - 0.18 * sin(q.x * 2.6 + t * 0.45) - 0.14), 1.6) * 26.0);
    float ribbonB = exp(-pow(abs(q.y + 0.24 * cos(q.x * 2.1 - t * 0.35) + 0.16), 1.5) * 22.0);
    float ribbonC = exp(-pow(abs((q.x * 0.55 + q.y * 0.85) - 0.22 * sin(q.x * 3.2 - t * 0.3)), 1.7) * 32.0);

    float contourWave = sin((q.x * 0.55 + q.y * 0.85) * 14.0 + 2.2 * sin(q.x * 2.4 - q.y * 1.6 - t * 0.3));
    float contours = smoothstep(0.84, 0.99, contourWave) * 0.11;

    vec2 orb1 = vec2(0.45 * sin(t * 0.24 + 0.5), 0.26 * cos(t * 0.19));
    vec2 orb2 = vec2(-0.52 * cos(t * 0.21 + 1.4), -0.22 * sin(t * 0.27));
    float g1 = exp(-dot(p - orb1, p - orb1) * 4.2);
    float g2 = exp(-dot(p - orb2, p - orb2) * 5.2);

    vec3 c0, c1, c2, c3, hi1, hi2;
    if (pal == 0) {
        c0 = vec3(0.03, 0.11, 0.25);
        c1 = vec3(0.06, 0.28, 0.52);
        c2 = vec3(0.09, 0.56, 0.68);
        c3 = vec3(0.36, 0.82, 0.94);
        hi1 = vec3(0.74, 0.96, 1.00);
        hi2 = vec3(0.32, 0.92, 0.84);
    } else if (pal == 1) {
        c0 = vec3(0.05, 0.04, 0.16);
        c1 = vec3(0.22, 0.10, 0.44);
        c2 = vec3(0.08, 0.52, 0.68);
        c3 = vec3(0.86, 0.28, 0.64);
        hi1 = vec3(0.54, 0.96, 0.98);
        hi2 = vec3(0.98, 0.58, 0.84);
    } else if (pal == 2) {
        c0 = vec3(0.11, 0.06, 0.18);
        c1 = vec3(0.46, 0.15, 0.34);
        c2 = vec3(0.88, 0.38, 0.34);
        c3 = vec3(0.98, 0.74, 0.36);
        hi1 = vec3(1.00, 0.90, 0.68);
        hi2 = vec3(0.42, 0.82, 0.98);
    } else {
        c0 = vec3(0.03, 0.04, 0.07);
        c1 = vec3(0.08, 0.13, 0.21);
        c2 = vec3(0.15, 0.24, 0.34);
        c3 = vec3(0.38, 0.48, 0.58);
        hi1 = vec3(0.78, 0.88, 0.98);
        hi2 = vec3(0.52, 0.66, 0.84);
    }

    vec3 col = mix(c0, c1, smoothstep(0.0, 1.0, uv.y * 0.7 + w1 * 0.45));
    col = mix(col, c2, w2 * 0.55);
    col = mix(col, c3, smoothstep(0.38, 0.95, w3) * 0.42);
    col += hi1 * ribbonA * 0.58;
    col += hi2 * ribbonB * 0.48;
    col += mix(hi1, hi2, 0.5) * ribbonC * 0.34;
    col += hi1 * g1 * 0.30 + hi2 * g2 * 0.26;
    col += mix(hi1, vec3(1.0), 0.4) * contours;

    float vig = 1.0 - dot(p * vec2(0.55, 0.72), p * vec2(0.55, 0.72)) * 0.42;
    col *= clamp(vig, 0.68, 1.05);
    col += (hash21(gl_FragCoord.xy) - 0.5) * (1.2 / 255.0);
    return clamp(col, 0.0, 1.0);
}

void main() {
    vec3 bg = paletteBackdrop(v_uv, u_time, u_palette);

    vec2 cssP = vec2(gl_FragCoord.x, u_resolution.y - gl_FragCoord.y) / u_dpr;
    float dShadow = sceneSDF(cssP - vec2(0.0, 7.0));
    float shadow = exp(-max(dShadow, 0.0) / 22.0) * 0.34 * u_shadowFactor;
    float contact = exp(-max(dShadow, 0.0) / 6.5) * 0.18 * u_shadowFactor;
    bg *= (1.0 - clamp(shadow + contact, 0.0, 0.55));

    fragColor = vec4(bg, 1.0);
}
"#;

// Pass 1 & 2: separable Gaussian blur (1:1 BLUR_FRAG_SRC)
pub const LG_BLUR_FRAG: &str = r#"#version 330 core
precision highp float;
#define MAX_TAP 24
in vec2 v_uv;
out vec4 fragColor;

uniform sampler2D u_tex;
uniform vec2  u_texelDir;
uniform int   u_radius;
uniform float u_weights[MAX_TAP + 1];

void main() {
    vec4 sum = texture(u_tex, v_uv) * u_weights[0];
    for (int i = 1; i <= MAX_TAP; i++) {
        if (i > u_radius) break;
        float w = u_weights[i];
        vec2 off = u_texelDir * float(i);
        sum += texture(u_tex, v_uv + off) * w;
        sum += texture(u_tex, v_uv - off) * w;
    }
    fragColor = sum;
}
"#;

// Pass 3: physical liquid glass composite (1:1 MAIN_FRAG_SRC)
pub const LG_MAIN_FRAG: &str = r#"#version 330 core
precision highp float;
#define PI 3.14159265359

const float N_R = 1.0 - 0.024;
const float N_G = 1.0;
const float N_B = 1.0 + 0.024;

in vec2 v_uv;
out vec4 fragColor;

uniform sampler2D u_bg;
uniform sampler2D u_blurredBg;
uniform float u_refraction;
uniform float u_frost;
uniform float u_dispersion;
uniform vec4  u_tint;
uniform float u_glareAngle;

__SHARED_SDF__

float safeAsin(float x) {
    return asin(clamp(x, -1.0, 1.0));
}

const vec3 D65_WHITE = vec3(0.95045592705, 1.0, 1.08905775076);
const mat3 RGB_TO_XYZ_M = mat3(
    0.4124, 0.3576, 0.1805,
    0.2126, 0.7152, 0.0722,
    0.0193, 0.1192, 0.9505
);
const mat3 XYZ_TO_RGB_M = mat3(
     3.2406255, -1.537208 , -0.4986286,
    -0.9689307,  1.8757561,  0.0415175,
     0.0557101, -0.2040211,  1.0569959
);
float UNCOMPAND_SRGB(float a) { return a > 0.04045 ? pow((a + 0.055) / 1.055, 2.4) : a / 12.92; }
float COMPAND_RGB(float a) { return a <= 0.0031308 ? 12.92 * a : 1.055 * pow(max(a, 0.0), 0.41666666666) - 0.055; }
vec3 SRGB_TO_XYZ(vec3 s) { return vec3(UNCOMPAND_SRGB(s.x), UNCOMPAND_SRGB(s.y), UNCOMPAND_SRGB(s.z)) * RGB_TO_XYZ_M; }
vec3 XYZ_TO_SRGB(vec3 x) {
    vec3 r = x * XYZ_TO_RGB_M;
    return clamp(vec3(COMPAND_RGB(r.x), COMPAND_RGB(r.y), COMPAND_RGB(r.z)), 0.0, 1.0);
}
float XYZ_TO_LAB_F(float x) { return x > 0.00885645167 ? pow(x, 0.333333333) : 7.78703703704 * x + 0.13793103448; }
float LAB_TO_XYZ_F(float x) { return x > 0.206897 ? x * x * x : 0.12841854934 * (x - 0.137931034); }
vec3 SRGB_TO_LCH(vec3 srgb) {
    vec3 xyz = SRGB_TO_XYZ(srgb) / D65_WHITE;
    vec3 f = vec3(XYZ_TO_LAB_F(xyz.x), XYZ_TO_LAB_F(xyz.y), XYZ_TO_LAB_F(xyz.z));
    vec3 lab = vec3(116.0 * f.y - 16.0, 500.0 * (f.x - f.y), 200.0 * (f.y - f.z));
    return vec3(lab.x, sqrt(dot(lab.yz, lab.yz)), atan(lab.z, lab.y));
}
vec3 LCH_TO_SRGB(vec3 lch) {
    vec3 lab = vec3(lch.x, lch.y * cos(lch.z), lch.y * sin(lch.z));
    float w = (lab.x + 16.0) / 116.0;
    vec3 xyz = D65_WHITE * vec3(LAB_TO_XYZ_F(w + lab.y / 500.0), LAB_TO_XYZ_F(w), LAB_TO_XYZ_F(w - lab.z / 200.0));
    return XYZ_TO_SRGB(xyz);
}

vec2 getNormalCSS(vec2 cssP) {
    float e = 0.75;
    vec2 e1 = vec2( 1.0,  1.0);
    vec2 e2 = vec2(-1.0,  1.0);
    vec2 e3 = vec2( 1.0, -1.0);
    vec2 e4 = vec2(-1.0, -1.0);
    vec2 g = e1 * sceneSDF(cssP + e * e1)
           + e2 * sceneSDF(cssP + e * e2)
           + e3 * sceneSDF(cssP + e * e3)
           + e4 * sceneSDF(cssP + e * e4);
    float len = length(g);
    return len > 1e-5 ? g / len : vec2(0.0, 1.0);
}

vec3 sampleDispersion(vec2 uv, vec2 uvOffset, float blurMix, float dispStrength) {
    vec2 oR = uvOffset * (1.0 - (N_R - 1.0) * dispStrength);
    vec2 oG = uvOffset * (1.0 - (N_G - 1.0) * dispStrength);
    vec2 oB = uvOffset * (1.0 - (N_B - 1.0) * dispStrength);

    float bgR = texture(u_bg, clamp(uv + oR, 0.001, 0.999)).r;
    float bgG = texture(u_bg, clamp(uv + oG, 0.001, 0.999)).g;
    float bgB = texture(u_bg, clamp(uv + oB, 0.001, 0.999)).b;

    float blR = texture(u_blurredBg, clamp(uv + oR, 0.001, 0.999)).r;
    float blG = texture(u_blurredBg, clamp(uv + oG, 0.001, 0.999)).g;
    float blB = texture(u_blurredBg, clamp(uv + oB, 0.001, 0.999)).b;

    return vec3(mix(bgR, blR, blurMix), mix(bgG, blG, blurMix), mix(bgB, blB, blurMix));
}

void main() {
    vec2 res1x = u_resolution / u_dpr;
    vec2 cssP = vec2(gl_FragCoord.x, u_resolution.y - gl_FragCoord.y) / u_dpr;
    vec3 bgPixel = texture(u_bg, v_uv).rgb;

    float d = sceneSDF(cssP);
    if (d > 2.0) {
        fragColor = vec4(bgPixel, 1.0);
        return;
    }

    float nmerged = max(0.0, -d);
    float refThickness = 22.0 + 20.0 * u_refraction;
    float ior = 1.24 + 0.52 * u_refraction;
    float frostMix = clamp(u_frost * 0.56, 0.0, 0.92);
    float dispFactor = u_dispersion * (u_iconStyle == 2 ? 18.0 : 13.5);

    float x_R_ratio = clamp(1.0 - nmerged / refThickness, 0.0, 1.0);
    float thetaI = safeAsin(x_R_ratio * x_R_ratio);
    float thetaT = safeAsin(sin(thetaI) / ior);
    float edgeFactor = (nmerged >= refThickness) ? 0.0 : max(0.0, -tan(thetaT - thetaI));

    vec2 normalCSS = getNormalCSS(cssP);
    vec2 normalUV = vec2(normalCSS.x, -normalCSS.y);

    vec3 glassCol;
    if (edgeFactor <= 0.0001) {
        vec2 mildOff = -normalUV * (0.004 * u_refraction);
        glassCol = sampleDispersion(v_uv, mildOff, frostMix, dispFactor * 0.18);
    } else {
        float edgeH = clamp(nmerged / refThickness, 0.0, 1.0);
        float blurRate = mix(frostMix * 0.14, frostMix, smoothstep(0.22, 0.95, edgeH));
        vec2 uvOff = -normalUV * edgeFactor * (34.0 * u_refraction) / res1x;
        glassCol = sampleDispersion(v_uv, uvOff, blurRate, dispFactor);
    }

    glassCol = mix(glassCol, u_tint.rgb, u_tint.a * 0.55);
    glassCol = min(vec3(1.0), glassCol * 1.04);

    float fresnelRange = 20.0;
    float fresnelFactor = clamp(pow(clamp(1.0 - nmerged / fresnelRange + 0.06, 0.0, 1.0), 4.8), 0.0, 1.0);
    float fresnelStrength = (0.36 + 0.24 * u_refraction);
    vec3 fresnelLCH = SRGB_TO_LCH(mix(vec3(1.0), u_tint.rgb, u_tint.a * 0.45));
    fresnelLCH.x = clamp(fresnelLCH.x + 24.0 * fresnelFactor, 0.0, 100.0);
    glassCol = mix(glassCol, LCH_TO_SRGB(fresnelLCH), fresnelFactor * fresnelStrength * 0.58);

    float glareRange = 14.0;
    float glareGeo = clamp(pow(clamp(1.0 - nmerged / glareRange + 0.06, 0.0, 1.0), 5.0), 0.0, 1.0);
    float ang = atan(normalUV.y, normalUV.x);
    if (ang < 0.0) ang += 2.0 * PI;
    float glareAngle = (ang - PI * 0.25 + u_glareAngle) * 2.0;
    bool farSide = (glareAngle > PI * 1.5 && glareAngle < PI * 3.5) || (glareAngle < -PI * 0.5);
    float glareDir = (0.5 + 0.5 * sin(glareAngle)) * (farSide ? 0.80 : 1.24);
    glareDir = clamp(pow(max(glareDir, 0.0), 1.45), 0.0, 1.0);

    vec3 glareLCH = SRGB_TO_LCH(mix(glassCol, u_tint.rgb, u_tint.a * 0.35));
    glareLCH.x = clamp(glareLCH.x + 110.0 * glareDir * glareGeo, 0.0, 100.0);
    glareLCH.y = clamp(glareLCH.y + 22.0 * glareDir * glareGeo, 0.0, 120.0);
    glassCol = mix(glassCol, LCH_TO_SRGB(glareLCH), clamp(glareDir * glareGeo * 0.92, 0.0, 0.95));

    float bevel = smoothstep(2.0, 0.3, nmerged) * (0.24 + 0.68 * glareDir);
    glassCol = mix(glassCol, vec3(1.0), clamp(bevel * 0.68, 0.0, 0.78));

    float aa = max(0.75, 1.1 / max(u_dpr, 0.5));
    float mask = 1.0 - smoothstep(-aa, aa, d);
    fragColor = vec4(mix(bgPixel, glassCol, mask), 1.0);
}
"#;