// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Liquid Glass (Refractive Surface)
// Ported 1:1 with prototype/liquidglass.html (WebGL2 GLSL ES 3.00 <-> GLSL 330 core)
//
// Optical refraction model (crease-free convex dome curvature, high-refraction liquid lip
// bend/bendWidth, 5-band spectral chromatic dispersion, directional sheen & inner glow) inspired by:
//   - @samasante/liquid-glass (MIT License, Copyright (c) Sam Asante)
//     https://github.com/samasante/liquid-glass
//   - iyinchao/liquid-glass-studio (MIT License, Copyright (c) 2024 Charles Yin)
//     https://github.com/iyinchao/liquid-glass-studio

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

    // 1. Floating liquid glass droplets & smooth Apple-style pill lenses
    for (int i = 0; i < MAX_DROPS; i++) {
        if (i >= u_dropCount) break;
        vec4 dr = u_drops[i];
        vec2 hs = dr.zw;
        vec2 bb = abs(cssP - dr.xy) - hs;
        float bound = max(bb.x, bb.y);
        if (bound < d + k * 2.2) {
            float sd = sdSquircleBox(cssP, dr.xy, hs, u_dropRound[i], 2.25);
            d = (d > 1e4) ? sd : smin(d, sd, k);
        }
    }

    // 2. Spring-physics cursor lens
    if (u_cursorOn == 1) {
        vec2 hs = u_cursor.zw;
        vec2 bb = abs(cssP - u_cursor.xy) - hs;
        if (max(bb.x, bb.y) < d + k * 2.5) {
            float sd = sdSquircleBox(cssP, u_cursor.xy, hs, u_cursorRadius, 2.45);
            d = (d > 1e4) ? sd : smin(d, sd, k * 1.25);
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
                sd = sdSquircleBox(cssP, ic.xy, hs * 0.94, rad, 2.45);
            } else if (u_iconStyle == 2) {
                sd = sdRoundBoxFast(cssP, ic.xy, hs, 14.0);
            } else {
                sd = sdSquircleBox(cssP, ic.xy, hs, min(hs.x, hs.y) * 0.44, 3.4);
            }
            d = (d > 1e4) ? sd : smin(d, sd, iconK);
        }
    }

    return d;
}
"#;

// Pass 0: Pure high-contrast Apple macOS Tahoe / Studio silk-wave backdrop (unshadowed behind glass!)
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

// Signed distance to a 2D capsule (rounded bar) from a to b with radius r
float sdCapsule2D(vec2 p, vec2 a, vec2 b, float r) {
    vec2 pa = p - a;
    vec2 ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

// Composite a glossy 3D studio sphere with drop shadow, Fresnel rim, and crisp edge
vec3 drawStudioSphere(vec3 col, vec2 p, vec2 c, float r, vec3 baseA, vec3 baseB, vec3 rimCol, float pxSize) {
    vec2 q = p - c;
    float d = length(q);

    // Soft studio drop shadow on backdrop
    float sdSh = length(p - (c + vec2(0.014, -0.022))) - r;
    float sh = exp(-max(sdSh, 0.0) * 26.0) * 0.28;
    col *= (1.0 - sh * smoothstep(-pxSize, r * 0.35, d - r));

    if (d < r + pxSize * 2.0) {
        float rn = clamp(d / r, 0.0, 1.0);
        float z = sqrt(max(0.0, 1.0 - rn * rn));
        vec3 n = normalize(vec3(q / r, z));
        vec3 keyL = normalize(vec3(-0.52, 0.62, 0.68));
        vec3 fillL = normalize(vec3(0.65, -0.45, 0.50));
        vec3 viewV = vec3(0.0, 0.0, 1.0);

        float diff = clamp(dot(n, keyL) * 0.68 + 0.34, 0.0, 1.0);
        float fill = clamp(dot(n, fillL), 0.0, 1.0) * 0.32;
        vec3 hVec = normalize(keyL + viewV);
        float spec = pow(max(dot(n, hVec), 0.0), 42.0) * 0.85;
        float specBroad = pow(max(dot(n, hVec), 0.0), 9.0) * 0.22;
        float fres = pow(1.0 - max(n.z, 0.0), 3.0);

        vec3 sph = mix(baseA, baseB, clamp(0.5 + 0.5 * (n.x * 0.55 - n.y * 0.72), 0.0, 1.0));
        sph = sph * (0.48 + 0.68 * diff) + rimCol * fill * 0.45;
        sph += rimCol * fres * 0.58;
        sph += vec3(1.0) * (spec + specBroad);

        // Razor-sharp inner highlight ring right inside silhouette
        float rimLine = smoothstep(0.86, 0.99, rn) * (0.35 + 0.65 * max(dot(normalize(q + 1e-5), vec2(-0.55, 0.65)), 0.0));
        sph = mix(sph, vec3(1.0), rimLine * 0.42);

        float mask = 1.0 - smoothstep(r - pxSize, r + pxSize, d);
        col = mix(col, clamp(sph, 0.0, 1.0), mask);
    }
    return col;
}

// Composite a sleek 3D shaded sculptural capsule ribbon with luminous specular ridge
vec3 drawStudioCapsule(vec3 col, vec2 p, vec2 a, vec2 b, float r, vec3 cA, vec3 cB, vec3 hiCol, float pxSize) {
    vec2 pa = p - a;
    vec2 ba = b - a;
    float ba2 = max(dot(ba, ba), 1e-5);
    float tAlong = clamp(dot(pa, ba) / ba2, 0.0, 1.0);
    vec2 closest = a + ba * tAlong;
    vec2 perp = p - closest;
    float d = length(perp) - r;

    // Soft cast shadow behind capsule
    float dSh = sdCapsule2D(p - vec2(0.012, -0.020), a, b, r);
    float sh = exp(-max(dSh, 0.0) * 28.0) * 0.26;
    col *= (1.0 - sh * smoothstep(-pxSize, r * 0.45, d));

    if (d < pxSize * 2.0) {
        vec2 n2 = perp / max(r, 1e-4);
        float rn = clamp(length(n2), 0.0, 1.0);
        float z = sqrt(max(0.0, 1.0 - rn * rn));
        vec3 n = normalize(vec3(n2, z));
        vec3 keyL = normalize(vec3(-0.48, 0.68, 0.62));
        float diff = clamp(dot(n, keyL) * 0.62 + 0.42, 0.0, 1.0);
        float spec = pow(max(dot(n, normalize(keyL + vec3(0.0, 0.0, 1.0))), 0.0), 26.0) * 0.65;
        float fres = pow(1.0 - max(n.z, 0.0), 2.6);

        vec3 cap = mix(cA, cB, tAlong) * (0.52 + 0.58 * diff);
        cap += hiCol * fres * 0.48 + vec3(1.0) * spec;
        // Crisp 1px bright edge line so refraction bending is immediately legible
        float edgeHi = smoothstep(r - pxSize * 3.0, r - pxSize * 0.4, length(perp));
        cap = mix(cap, mix(hiCol, vec3(1.0), 0.65), edgeHi * 0.48);

        float mask = 1.0 - smoothstep(-pxSize, pxSize, d);
        col = mix(col, clamp(cap, 0.0, 1.0), mask);
    }
    return col;
}

// Composite a crisp floating luminous optical ring (best geometric visual cue for glass lip bending!)
vec3 drawOpticalRing(vec3 col, vec2 p, vec2 c, float radius, float halfThick, vec3 ringA, vec3 ringB, float pxSize) {
    vec2 q = p - c;
    float dist = length(q);
    float ringD = abs(dist - radius) - halfThick;

    // Subtle luminous halo around the ring
    float glow = exp(-max(ringD, 0.0) * 44.0) * 0.24;
    float ang = atan(q.y, q.x);
    vec3 rCol = mix(ringA, ringB, 0.5 + 0.5 * sin(ang - 0.8));
    col += rCol * glow;

    if (ringD < pxSize * 2.0) {
        float core = 1.0 - smoothstep(-pxSize, pxSize, ringD);
        float highlight = 0.72 + 0.28 * cos(ang - 2.2);
        vec3 body = mix(rCol, vec3(1.0), 0.42 * highlight);
        col = mix(col, clamp(body, 0.0, 1.0), core * 0.92);
    }
    return col;
}

// Add tasteful Apple Studio / macOS Tahoe geometric objects behind the glass layer
vec3 addStudioObjects(vec3 col, vec2 p, float t, int pal, float pxSize) {
    vec3 objA, objB, objC, objAccent, ringCol1, ringCol2, dotCol;
    if (pal == 0) {
        // Tahoe Azure
        objA      = vec3(0.06, 0.36, 0.86);
        objB      = vec3(0.10, 0.86, 0.88);
        objC      = vec3(0.96, 0.54, 0.32); // warm coral-peach studio accent
        objAccent = vec3(0.92, 0.98, 1.00);
        ringCol1  = vec3(0.58, 0.96, 1.00);
        ringCol2  = vec3(0.32, 0.68, 1.00);
        dotCol    = vec3(0.82, 0.96, 1.00);
    } else if (pal == 1) {
        // Aurora Prism
        objA      = vec3(0.38, 0.16, 0.88);
        objB      = vec3(0.10, 0.92, 0.86);
        objC      = vec3(0.98, 0.30, 0.72);
        objAccent = vec3(0.92, 0.96, 1.00);
        ringCol1  = vec3(0.42, 0.98, 0.94);
        ringCol2  = vec3(1.00, 0.48, 0.86);
        dotCol    = vec3(0.86, 0.94, 1.00);
    } else if (pal == 2) {
        // Sunset Silk
        objA      = vec3(0.86, 0.24, 0.42);
        objB      = vec3(1.00, 0.72, 0.24);
        objC      = vec3(0.28, 0.78, 0.96);
        objAccent = vec3(1.00, 0.96, 0.86);
        ringCol1  = vec3(1.00, 0.88, 0.58);
        ringCol2  = vec3(1.00, 0.48, 0.58);
        dotCol    = vec3(1.00, 0.94, 0.86);
    } else if (pal == 3) {
        // Obsidian Architectural Glass
        objA      = vec3(0.18, 0.24, 0.34);
        objB      = vec3(0.62, 0.74, 0.88);
        objC      = vec3(0.36, 0.68, 0.96);
        objAccent = vec3(0.94, 0.97, 1.00);
        ringCol1  = vec3(0.86, 0.92, 1.00);
        ringCol2  = vec3(0.45, 0.64, 0.86);
        dotCol    = vec3(0.84, 0.90, 0.98);
    } else {
        // Classic Silk
        objA      = vec3(0.12, 0.46, 0.96);
        objB      = vec3(0.14, 0.90, 0.78);
        objC      = vec3(0.98, 0.66, 0.28);
        objAccent = vec3(0.92, 0.98, 1.00);
        ringCol1  = vec3(0.72, 0.97, 1.00);
        ringCol2  = vec3(0.28, 0.76, 1.00);
        dotCol    = vec3(0.84, 0.96, 1.00);
    }

    // 1. Subtle architectural optical dot matrix (crisp micro-dots that visibly warp through glass!)
    float cell = 0.072;
    vec2 gId = floor(p / cell + 0.5);
    vec2 gLocal = p - gId * cell;
    float dotDist = length(gLocal);
    float dotMask = 1.0 - smoothstep(0.0018, 0.0018 + pxSize * 1.1, dotDist);
    // Subtle cross-hair accent every 4th grid point
    vec2 majorMod = abs(mod(gId + 2.0, 4.0) - 2.0);
    float isMajor = step(1.5, majorMod.x) * step(1.5, majorMod.y);
    float crossD = min(
        max(abs(gLocal.x) - 0.0060, abs(gLocal.y) - 0.0009),
        max(abs(gLocal.y) - 0.0060, abs(gLocal.x) - 0.0009)
    );
    float crossMask = (1.0 - smoothstep(-pxSize * 0.6, pxSize * 0.9, crossD)) * isMajor;
    col = mix(col, dotCol, clamp(dotMask * 0.19 + crossMask * 0.16, 0.0, 0.32));

    // 2. Three floating crisp optical rings spanning left icon zone, center, and right
    vec2 r1c = vec2(-0.56 + 0.024 * sin(t * 0.18),  0.08 + 0.026 * cos(t * 0.15));
    vec2 r2c = vec2( 0.04 + 0.030 * cos(t * 0.14),  0.00 + 0.024 * sin(t * 0.17));
    vec2 r3c = vec2( 0.56 + 0.025 * sin(t * 0.16 + 1.4), -0.06 + 0.028 * cos(t * 0.14));
    col = drawOpticalRing(col, p, r1c, 0.215, 0.0034, ringCol1, ringCol2, pxSize);
    col = drawOpticalRing(col, p, r2c, 0.295, 0.0040, ringCol2, objC,     pxSize);
    col = drawOpticalRing(col, p, r3c, 0.195, 0.0032, ringCol1, objAccent, pxSize);

    // 3. Four sleek tilted 3D sculptural capsules with generous Apple Studio breathing room
    float s1 = 0.016 * sin(t * 0.20);
    float s2 = 0.018 * cos(t * 0.16 + 0.9);
    float s3 = 0.016 * sin(t * 0.18 + 2.1);
    float s4 = 0.015 * cos(t * 0.21 + 3.2);

    // Left-column diagonal ribbon (sits right behind typical desktop icon columns)
    col = drawStudioCapsule(
        col, p,
        vec2(-0.84,  0.34 + s1), vec2(-0.48,  0.05 + s1),
        0.044, objA, objB, objAccent, pxSize
    );
    // Center-upper sweeping warm/prism accent pill
    col = drawStudioCapsule(
        col, p,
        vec2(-0.20,  0.29 + s2), vec2( 0.24,  0.17 + s2),
        0.048, objC, objB, objAccent, pxSize
    );
    // Lower-center sculpted bar
    col = drawStudioCapsule(
        col, p,
        vec2(-0.46, -0.29 + s3), vec2( 0.06, -0.19 + s3),
        0.042, objB, objA, ringCol1, pxSize
    );
    // Right diagonal sculptural bar
    col = drawStudioCapsule(
        col, p,
        vec2( 0.36, -0.31 + s4), vec2( 0.80,  0.14 + s4),
        0.046, objA, objC, objAccent, pxSize
    );

    // 4. Four crisp 3D glossy studio spheres (balanced focal points with clean spacing)
    col = drawStudioSphere(
        col, p,
        vec2(-0.69 + 0.020 * cos(t * 0.22), -0.19 + 0.022 * sin(t * 0.18)),
        0.088, objC, objB, objAccent, pxSize
    );
    col = drawStudioSphere(
        col, p,
        vec2(-0.26 + 0.022 * sin(t * 0.17 + 1.2), -0.02 + 0.020 * cos(t * 0.20)),
        0.106, objA, objB, ringCol1, pxSize
    );
    col = drawStudioSphere(
        col, p,
        vec2( 0.22 + 0.020 * cos(t * 0.15 + 2.4), -0.18 + 0.018 * sin(t * 0.19)),
        0.078, objC, objA, objAccent, pxSize
    );
    col = drawStudioSphere(
        col, p,
        vec2( 0.45 + 0.022 * sin(t * 0.19 + 0.6),  0.25 + 0.022 * cos(t * 0.16)),
        0.098, objB, objC, objAccent, pxSize
    );

    return col;
}

vec3 paletteBackdrop(vec2 uv, float t, int pal) {
    float asp = u_resolution.x / max(1.0, u_resolution.y);
    vec2 p = (uv - 0.5) * vec2(asp, 1.0);
    float pxSize = 1.15 / max(1.0, u_resolution.y);

    // Classic Silk (calm blue/teal waves + studio optical objects)
    if (pal == 4) {
        vec3 col = vec3(0.032, 0.105, 0.220);
        vec3 lobes[4];
        lobes[0] = vec3(0.180, 0.720, 0.980);
        lobes[1] = vec3(0.140, 0.850, 0.760);
        lobes[2] = vec3(0.240, 0.480, 0.980);
        lobes[3] = vec3(0.760, 0.930, 1.000);
        for (int i = 0; i < 4; i++) {
            float fi = float(i);
            vec2 c = vec2(
                0.18 + 0.22 * fi + sin(t * (0.40 + fi * 0.10) + fi * 1.3) * 0.18,
                0.50 + cos(t * (0.34 + fi * 0.12) + fi * 1.7) * 0.34
            );
            vec2 dv = (uv - c) * vec2(asp, 1.0);
            float rad = (0.46 + 0.08 * fi) * max(asp, 1.0);
            float a = clamp(1.0 - length(dv) / rad, 0.0, 1.0);
            a = a * a * (3.0 - 2.0 * a) * 0.68;
            col = mix(col, lobes[i], a);
        }
        float wave = sin((p.x * 0.75 + p.y * 0.66) * 10.5 + 1.4 * sin(p.y * 3.0 - t * 0.32));
        col += vec3(0.84, 0.96, 1.0) * smoothstep(0.65, 0.98, wave) * 0.18;
        col = addStudioObjects(col, p, t, pal, pxSize);
        col += (hash21(gl_FragCoord.xy) - 0.5) * (1.2 / 255.0);
        return clamp(col, 0.0, 1.0);
    }

    // Apple macOS Tahoe / iOS 26 style sculpted sweeping glass silk layers
    // Clean, continuous parallel sculptural waves (no harsh laser X-intersections)
    vec2 w = p;
    w.x += 0.16 * sin(p.y * 2.1 + t * 0.32) + 0.06 * cos(p.x * 2.8 - t * 0.22);
    w.y += 0.14 * cos(p.x * 1.9 - t * 0.27) + 0.05 * sin(p.y * 3.1 + t * 0.19);

    // Three sweeping stacked translucent silk curtains (sharp top edge, soft gradient falloff underneath)
    float curve1 = w.y - (0.16 * sin(w.x * 2.1 + t * 0.34) + 0.06 * cos(w.x * 3.8 - t * 0.22) + 0.20);
    float curve2 = w.y - (0.19 * cos(w.x * 1.7 - t * 0.28 + 1.1) - 0.04 * sin(w.x * 3.2 + t * 0.21) - 0.02);
    float curve3 = w.y - (0.15 * sin(w.x * 2.4 + t * 0.24 + 2.5) - 0.24);

    float layer1 = smoothstep(-0.015, 0.015, -curve1);
    float layer2 = smoothstep(-0.015, 0.015, -curve2);
    float layer3 = smoothstep(-0.015, 0.015, -curve3);

    // Crisp luminous specular edge on each silk wave lip — bends dramatically through Liquid Glass!
    float edge1 = exp(-abs(curve1) * 34.0) * 0.65 + exp(-abs(curve1) * 10.0) * 0.22;
    float edge2 = exp(-abs(curve2) * 32.0) * 0.62 + exp(-abs(curve2) * 9.0) * 0.24;
    float edge3 = exp(-abs(curve3) * 30.0) * 0.56 + exp(-abs(curve3) * 9.0) * 0.20;

    // Fine flowing silk contour highlights along the waves (shows lens curvature cleanly)
    float silkRipple = sin(curve2 * 38.0 + 1.2 * sin(w.x * 3.4 - t * 0.24));
    float silkSheen = pow(0.5 + 0.5 * silkRipple, 8.0) * 0.14;

    // Smooth circular studio light blooms (no polygonal artifacts)
    vec2 orb1 = vec2(0.42 * sin(t * 0.20 + 0.4), 0.24 * cos(t * 0.16));
    vec2 orb2 = vec2(-0.48 * cos(t * 0.17 + 1.6), -0.18 * sin(t * 0.22));
    float d1 = length(p - orb1);
    float d2 = length(p - orb2);
    float bloom1 = exp(-d1 * d1 * 4.5) * 0.36;
    float bloom2 = exp(-d2 * d2 * 5.2) * 0.32;

    vec3 c0, c1, c2, c3, hi1, hi2;
    if (pal == 0) {
        // Tahoe Azure (macOS 26 Tahoe sapphire depths, turquoise waves & luminous ice silk)
        c0  = vec3(0.018, 0.068, 0.185);
        c1  = vec3(0.035, 0.215, 0.485);
        c2  = vec3(0.055, 0.485, 0.695);
        c3  = vec3(0.245, 0.790, 0.935);
        hi1 = vec3(0.840, 0.980, 1.000);
        hi2 = vec3(0.280, 0.955, 0.875);
    } else if (pal == 1) {
        // Aurora Prism (midnight indigo, electric cyan & vivid magenta polar curtains)
        c0  = vec3(0.030, 0.022, 0.125);
        c1  = vec3(0.215, 0.075, 0.445);
        c2  = vec3(0.065, 0.485, 0.690);
        c3  = vec3(0.880, 0.255, 0.665);
        hi1 = vec3(0.620, 0.985, 1.000);
        hi2 = vec3(1.000, 0.630, 0.890);
    } else if (pal == 2) {
        // Sunset Silk (deep velvet plum, rich coral & liquid amber-gold horizons)
        c0  = vec3(0.085, 0.038, 0.155);
        c1  = vec3(0.455, 0.130, 0.335);
        c2  = vec3(0.895, 0.355, 0.290);
        c3  = vec3(0.990, 0.745, 0.345);
        hi1 = vec3(1.000, 0.945, 0.780);
        hi2 = vec3(0.420, 0.855, 1.000);
    } else {
        // Obsidian Architectural Glass (deep graphite, slate waves & silver-chrome rim lights)
        c0  = vec3(0.020, 0.025, 0.042);
        c1  = vec3(0.065, 0.092, 0.148);
        c2  = vec3(0.135, 0.195, 0.285);
        c3  = vec3(0.340, 0.435, 0.560);
        hi1 = vec3(0.900, 0.950, 1.000);
        hi2 = vec3(0.500, 0.720, 0.950);
    }

    // Layered silk composition with smooth vertical depth gradients inside each wave
    vec3 col = mix(c0, c1, clamp(uv.y * 0.9 + 0.12, 0.0, 1.0));
    vec3 waveCol1 = mix(c1, c2, clamp(0.45 + curve1 * 1.4, 0.0, 1.0));
    vec3 waveCol2 = mix(c2, c3, clamp(0.40 + curve2 * 1.4, 0.0, 1.0));
    vec3 waveCol3 = mix(c0, c2, clamp(0.55 + curve3 * 1.6, 0.0, 1.0));

    col = mix(col, waveCol1, layer1 * 0.86);
    col = mix(col, waveCol2, layer2 * 0.82);
    col = mix(col, waveCol3, layer3 * 0.78);

    // Subtle soft shadow just above each overlapping wave crest for true 3D layering
    col *= 1.0 - smoothstep(0.0, 0.12, curve1) * exp(-max(curve1, 0.0) * 14.0) * 0.28;
    col *= 1.0 - smoothstep(0.0, 0.12, curve2) * exp(-max(curve2, 0.0) * 14.0) * 0.28;
    col *= 1.0 - smoothstep(0.0, 0.12, curve3) * exp(-max(curve3, 0.0) * 14.0) * 0.26;

    // Luminous wave crests, silk micro-ripples & ambient studio blooms
    col += hi1 * edge1 * 0.42;
    col += hi2 * edge2 * 0.38;
    col += mix(hi1, hi2, 0.5) * edge3 * 0.34;
    col += mix(hi1, vec3(1.0), 0.5) * silkSheen;
    col += hi1 * bloom1 * 0.75 + hi2 * bloom2 * 0.75;

    // Distinct 3D sculptural objects (spheres, capsules, rings, micro-dot grid) so glass refraction pops!
    col = addStudioObjects(col, p, t, pal, pxSize);

    float vig = 1.0 - dot(p * vec2(0.46, 0.62), p * vec2(0.46, 0.62)) * 0.30;
    col *= clamp(vig, 0.74, 1.04);
    col += (hash21(gl_FragCoord.xy) - 0.5) * (1.2 / 255.0);
    return clamp(col, 0.0, 1.0);
}

void main() {
    // Keep u_bg pure and unshadowed so glass refracts crystal-clear luminous light
    vec3 bg = paletteBackdrop(v_uv, u_time, u_palette);
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

// Pass 3: True Crease-Free Liquid Glass Composite (@samasante/liquid-glass lip/dome optics + 5-band spectral dispersion + LCH sheen/glow)
pub const LG_MAIN_FRAG: &str = r#"#version 330 core
precision highp float;
#define PI 3.14159265359

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

// Smooth SDF gradient: returns unnormalized derivative vector (magnitude -> 0 cleanly at medial axis,
// preventing any faceted "X" crease inside rectangular pills or icon glass cradles!)
vec2 getSmoothGradientCSS(vec2 cssP, float e) {
    vec2 e1 = vec2( 1.0,  1.0);
    vec2 e2 = vec2(-1.0,  1.0);
    vec2 e3 = vec2( 1.0, -1.0);
    vec2 e4 = vec2(-1.0, -1.0);
    return (e1 * sceneSDF(cssP + e * e1)
          + e2 * sceneSDF(cssP + e * e2)
          + e3 * sceneSDF(cssP + e * e3)
          + e4 * sceneSDF(cssP + e * e4)) / (4.0 * e);
}

vec3 sampleLayer(vec2 uv, float blurMix) {
    vec2 suv = clamp(uv, vec2(0.001), vec2(0.999));
    vec3 sharp = texture(u_bg, suv).rgb;
    if (blurMix <= 0.005) return sharp;
    vec3 soft = texture(u_blurredBg, suv).rgb;
    return mix(sharp, soft, blurMix);
}

// 5-band white-preserving spectral chromatic dispersion (Red / Amber / Green / Cyan / Blue)
vec3 sampleSpectralDispersion(vec2 uv, vec2 uvOffset, float blurMix, float spread) {
    if (spread <= 0.001) {
        return sampleLayer(uv + uvOffset, blurMix);
    }
    vec3 cR = sampleLayer(uv + uvOffset * (1.0 - spread * 1.00), blurMix);
    vec3 cY = sampleLayer(uv + uvOffset * (1.0 - spread * 0.45), blurMix);
    vec3 cG = sampleLayer(uv + uvOffset,                        blurMix);
    vec3 cC = sampleLayer(uv + uvOffset * (1.0 + spread * 0.45), blurMix);
    vec3 cB = sampleLayer(uv + uvOffset * (1.0 + spread * 1.00), blurMix);

    float r = cR.r * 0.62 + cY.r * 0.38;
    float g = cY.g * 0.24 + cG.g * 0.52 + cC.g * 0.24;
    float b = cC.b * 0.38 + cB.b * 0.62;
    return vec3(r, g, b);
}

void main() {
    vec2 res1x = u_resolution / u_dpr;
    vec2 cssP = vec2(gl_FragCoord.x, u_resolution.y - gl_FragCoord.y) / u_dpr;
    vec3 bgPixel = texture(u_bg, v_uv).rgb;

    float d = sceneSDF(cssP);

    // Soft glass drop shadow + subtle bright caustic rim ONLY outside the glass footprint
    float dShadow = sceneSDF(cssP - vec2(0.0, 9.0));
    float outerShadow = exp(-max(dShadow, 0.0) / 24.0) * 0.32;
    float contactAO = exp(-max(dShadow, 0.0) / 6.0) * 0.20;
    float shadowMask = smoothstep(-1.5, 2.5, d);
    vec3 shadowedBg = bgPixel * (1.0 - clamp(outerShadow + contactAO, 0.0, 0.52) * shadowMask);
    float causticBand = exp(-pow((dShadow - 4.0) / 8.5, 2.0)) * 0.08 * shadowMask * clamp(u_refraction, 0.0, 1.5);
    shadowedBg = min(vec3(1.0), shadowedBg + mix(vec3(1.0), u_tint.rgb, 0.4) * causticBand);

    if (d > 2.2) {
        fragColor = vec4(shadowedBg, 1.0);
        return;
    }

    float depthPx = max(0.0, -d);

    // Tight gradient for crisp rim sheen, wider smooth gradient for crease-free optical lens displacement
    vec2 gRim = getSmoothGradientCSS(cssP, 1.1);
    float gRimLen = length(gRim);
    vec2 rimNormalCSS = gRimLen > 1e-4 ? (gRim / gRimLen) : vec2(0.0, 1.0);
    vec2 rimNormalUV = vec2(rimNormalCSS.x, -rimNormalCSS.y);

    vec2 gLens = getSmoothGradientCSS(cssP, 2.4);
    float gLensLen = length(gLens);
    // Smoothly attenuate lip displacement to exact 0 along medial axis (prevents any center wiggle or X-crease!)
    vec2 lipNormalUV = vec2(gLens.x, -gLens.y) * smoothstep(0.42, 0.96, gLensLen);

    vec2 gDome = getSmoothGradientCSS(cssP, 7.5);
    float gDomeLen = length(gDome);
    vec2 domeNormalUV = vec2(gDome.x, -gDome.y) * smoothstep(0.50, 0.98, gDomeLen);

    // ---- @samasante/liquid-glass two-zone optical profile ----
    // Zone 1: "Liquid Lip" (tight bendWidth rim band that smoothly wraps background around the border)
    float lipWidth = 9.5 + 5.5 * u_refraction;
    float uLip = clamp(depthPx / lipWidth, 0.0, 1.0);
    float sLip = 1.0 - uLip;
    // Smooth cubic taper guarantees zero slope discontinuity where lip meets crystal interior
    float lipEnvelope = sLip * sLip * (3.0 - 2.0 * sLip);
    float ior = 1.38 + 0.42 * u_refraction;
    float thetaI = safeAsin(pow(sLip, 1.25));
    float thetaT = safeAsin(sin(thetaI) / ior);
    float snellSlope = (depthPx >= lipWidth) ? 0.0 : max(0.0, -tan(thetaT - thetaI));
    float lipBend = snellSlope * lipEnvelope * (1.0 + 1.35 * pow(sLip, 2.5));

    // Zone 2: Silky convex crystal dome (vanishes smoothly toward center via wide-kernel domeNormalUV)
    float domeFalloff = exp(-depthPx / (18.0 + 10.0 * u_refraction));
    vec2 dispVecPx = (lipNormalUV * (lipBend * 48.0) + domeNormalUV * (domeFalloff * 8.5)) * u_refraction;
    vec2 uvOff = -dispVecPx / res1x;

    // Crystal-clear liquid rim (razor-sharp refracted edge), gentle smooth frost inside
    float interiorFrost = clamp(u_frost * 0.24, 0.0, 0.72);
    float blurRate = mix(interiorFrost * 0.05, interiorFrost, smoothstep(0.15, 0.95, uLip));

    // 5-band prismatic chromatic dispersion concentrated cleanly in the bent liquid lip
    float dispScale = (u_iconStyle == 2 ? 0.26 : 0.20) * u_dispersion;
    float spread = dispScale * (0.06 + 1.42 * lipEnvelope);

    vec3 glassCol = sampleSpectralDispersion(v_uv, uvOff, blurRate, spread);

    // Crystal-clear luminosity & subtle glass tint
    glassCol = mix(glassCol, u_tint.rgb, u_tint.a * 0.24);
    vec3 baseLch = SRGB_TO_LCH(glassCol);
    baseLch.x = clamp(baseLch.x + 2.8, 0.0, 100.0);
    baseLch.y = clamp(baseLch.y * 1.07, 0.0, 128.0);
    glassCol = LCH_TO_SRGB(baseLch);

    // Subtle internal total-reflection dark micro-line right behind the outer specular bevel
    float innerDarkBand = exp(-pow((depthPx - 3.6) / 2.2, 2.0)) * 0.11 * clamp(u_refraction, 0.3, 1.5);
    glassCol *= (1.0 - innerDarkBand);

    // ---- Soft inner rim glow (@samasante glow + glowSpread) ----
    float innerGlow = exp(-depthPx / 11.5) * (0.14 + 0.09 * u_refraction);
    vec3 glowCol = mix(vec3(1.0), u_tint.rgb, 0.32);
    glassCol = mix(glassCol, max(glassCol, glowCol), clamp(innerGlow * 0.52, 0.0, 0.30));

    // ---- Directional dual-lobe specular sheen (sheen, sheenWidth, sheenFalloff, sheenAngle) ----
    float ang = atan(rimNormalUV.y, rimNormalUV.x);
    if (ang < 0.0) ang += 2.0 * PI;
    float keyDir = cos(ang - (PI * 0.72 - u_glareAngle));   // upper-left key highlight
    float oppDir = cos(ang - (PI * 1.72 - u_glareAngle));   // opposite internal bounce
    float primarySheen = pow(max(keyDir, 0.0), 2.5);
    float secondarySheen = pow(max(oppDir, 0.0), 3.8) * 0.58;
    float sheenDir = clamp(primarySheen + secondarySheen, 0.0, 1.0);

    float sheenBand = clamp(pow(clamp(1.0 - depthPx / 13.0 + 0.04, 0.0, 1.0), 4.4), 0.0, 1.0);
    vec3 sheenLCH = SRGB_TO_LCH(mix(glassCol, vec3(1.0), 0.45));
    sheenLCH.x = clamp(sheenLCH.x + 88.0 * sheenDir * sheenBand, 0.0, 100.0);
    sheenLCH.y = clamp(sheenLCH.y * (1.0 - 0.45 * sheenDir * sheenBand), 0.0, 120.0);
    glassCol = mix(glassCol, LCH_TO_SRGB(sheenLCH), clamp(sheenDir * sheenBand * 0.86, 0.0, 0.92));

    // Delicate top-arc crystal surface sheen
    float topArc = smoothstep(0.25, 0.95, keyDir) * exp(-depthPx / 20.0) * 0.09 * smoothstep(0.25, 0.95, gRimLen);
    glassCol = min(vec3(1.0), glassCol + vec3(topArc));

    // Razor-sharp 1.5px polished crystal outer bevel rim
    float bevel = smoothstep(2.2, 0.35, depthPx) * (0.28 + 0.72 * sheenDir);
    glassCol = mix(glassCol, vec3(1.0), clamp(bevel * 0.84, 0.0, 0.90));

    float aa = max(0.75, 1.1 / max(u_dpr, 0.5));
    float mask = 1.0 - smoothstep(-aa, aa, d);
    fragColor = vec4(mix(shadowedBg, glassCol, mask), 1.0);
}
"#;
