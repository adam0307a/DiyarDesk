pub const AM_BG_VERT: &str = r#"#version 330 core
out vec2 vUV;
void main() {
    vec2 p = vec2((gl_VertexID == 1) ? 3.0 : -1.0, (gl_VertexID == 2) ? 3.0 : -1.0);
    vUV = p * 0.5 + 0.5;
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;

// Night / Evening / Day atmospheric isometric campus ground + subtle architectural tile grid + drifting fog & light rays
pub const AM_BG_FRAG: &str = r#"#version 330 core
in vec2 vUV;
uniform vec2 uRes;
uniform float uTime;
uniform vec2 uParallax;
uniform vec3 uSkyTop;
uniform vec3 uSkyMid;
uniform vec3 uGround;
uniform vec3 uGridCol;
uniform vec3 uRayCol;
uniform float uFogAlpha;
uniform float uStarAlpha;
out vec4 O;

float hash21(vec2 p) {
    p = fract(p * vec2(234.34, 435.345));
    p += dot(p, p + 34.23);
    return fract(p.x * p.y);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = hash21(i);
    float b = hash21(i + vec2(1.0, 0.0));
    float c = hash21(i + vec2(0.0, 1.0));
    float d = hash21(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

void main() {
    vec2 px = vec2(vUV.x * uRes.x, (1.0 - vUV.y) * uRes.y);
    vec2 uv = px / max(uRes, vec2(1.0));

    // Soft elevated campus gradient (horizon band at top -> campus courtyard below)
    float yBand = smoothstep(0.0, 0.30, uv.y);
    vec3 col = mix(uSkyTop, uSkyMid, smoothstep(0.0, 0.22, uv.y));
    col = mix(col, uGround, yBand);

    // Subtle radial courtyard focus vignette (keeps desktop icons super readable)
    vec2 centered = (uv - vec2(0.5, 0.54)) * vec2(1.0, 1.15);
    float rad = length(centered);
    col *= (1.06 - smoothstep(0.15, 0.92, rad) * 0.22);

    // Subtle starry upper sky atmosphere (crisp tiny circular stars, never blocky squares)
    if (uStarAlpha > 0.01 && uv.y < 0.30) {
        vec2 cellCoord = (px + uParallax * 0.25) / 14.0;
        vec2 sc = floor(cellCoord);
        vec2 f = fract(cellCoord);
        float h = hash21(sc);
        if (h > 0.92) {
            vec2 starPos = vec2(0.2 + fract(h * 17.3) * 0.6, 0.2 + fract(h * 31.7) * 0.6);
            float sd = length((f - starPos) * 14.0);
            float rad = 0.7 + fract(h * 53.1) * 0.85;
            float star = 1.0 - smoothstep(rad * 0.4, rad, sd);
            float tw = 0.55 + 0.45 * sin(uTime * (1.1 + fract(h * 73.1) * 1.6) + h * 40.0);
            float fade = (1.0 - smoothstep(0.05, 0.28, uv.y)) * uStarAlpha;
            col += vec3(0.76, 0.87, 1.0) * star * tw * fade * 0.72;
        }
    }

    // Isometric architectural campus courtyard paver grid (below horizon)
    float groundMask = smoothstep(0.14, 0.24, uv.y);
    if (groundMask > 0.001) {
        vec2 gp = (px - uRes * 0.5 - uParallax) / max(min(uRes.x, uRes.y) * 0.068, 28.0);
        // Isometric transform: diamond tiles (2:1 aspect)
        vec2 iso = vec2(gp.x * 0.5 + gp.y, -gp.x * 0.5 + gp.y);
        vec2 cell = fract(iso);
        vec2 distEdge = min(cell, 1.0 - cell);
        float line = 1.0 - smoothstep(0.0, 0.032, min(distEdge.x, distEdge.y));
        // Subtle paver tone variation per tile
        float tileTone = (hash21(floor(iso)) - 0.5) * 0.022;
        // Larger architectural courtyard sector seams (every 4 tiles)
        vec2 majorCell = fract(iso * 0.25);
        vec2 majorDist = min(majorCell, 1.0 - majorCell);
        float majorLine = 1.0 - smoothstep(0.0, 0.014, min(majorDist.x, majorDist.y));

        col += vec3(tileTone) * groundMask;
        col = mix(col, uGridCol, (line * 0.22 + majorLine * 0.35) * groundMask);
    }

    // Gentle diagonal atmospheric light rays from upper-left
    float rayCoord = (uv.x * 0.85 - uv.y * 0.55) * 6.5 + sin(uTime * 0.06) * 0.25;
    float ray = pow(max(0.0, sin(rayCoord) * 0.5 + 0.5), 4.0) * (1.0 - smoothstep(0.0, 0.78, uv.y));
    col += uRayCol * ray * 0.048;

    // Slowly drifting soft atmospheric fog / cloud wisps
    if (uFogAlpha > 0.005) {
        vec2 fuv1 = uv * vec2(3.2, 2.1) + vec2(uTime * 0.0075, -uTime * 0.002);
        vec2 fuv2 = uv * vec2(5.4, 3.4) + vec2(-uTime * 0.005, uTime * 0.0015);
        float n = vnoise(fuv1) * 0.65 + vnoise(fuv2) * 0.35;
        float fog = smoothstep(0.46, 0.86, n) * uFogAlpha * (0.45 + 0.55 * (1.0 - uv.y));
        col = mix(col, uSkyMid * 1.35 + vec3(0.04, 0.06, 0.09), fog);
    }

    O = vec4(col, 1.0);
}
"#;

pub const AM_COL_VERT: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
layout(location=1) in vec4 aCol;
uniform vec2 uRes;
out vec4 vCol;
void main() {
    vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    vCol = aCol;
}
"#;

pub const AM_COL_FRAG: &str = r#"#version 330 core
in vec4 vCol;
out vec4 O;
void main() {
    if (vCol.a <= 0.001) discard;
    O = vCol;
}
"#;

// Batched tinted glow sprite shader (RGBA8 radial glow with strict 1px alpha=0 border)
pub const AM_TEX_VERT: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
layout(location=1) in vec2 aUV;
layout(location=2) in vec4 aCol;
uniform vec2 uRes;
out vec2 vUV;
out vec4 vCol;
void main() {
    vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    vUV = aUV;
    vCol = aCol;
}
"#;

pub const AM_TEX_FRAG: &str = r#"#version 330 core
uniform sampler2D uTex;
in vec2 vUV;
in vec4 vCol;
out vec4 O;
void main() {
    vec4 t = texture(uTex, vUV);
    float a = t.a * vCol.a;
    if (a <= 0.001) discard;
    O = vec4(t.rgb * vCol.rgb, a);
}
"#;

pub const AM_SDF_VERT: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
layout(location=1) in vec2 aLocal;
layout(location=2) in vec4 aCol;
layout(location=3) in vec4 aStroke;
layout(location=4) in vec4 aParams;
uniform vec2 uRes;
out vec2 vLocal;
out vec4 vCol;
out vec4 vStroke;
out vec4 vParams;
void main() {
    vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    vLocal = aLocal;
    vCol = aCol;
    vStroke = aStroke;
    vParams = aParams;
}
"#;

pub const AM_SDF_FRAG: &str = r#"#version 330 core
in vec2 vLocal;
in vec4 vCol;
in vec4 vStroke;
in vec4 vParams;
out vec4 O;

void main() {
    float kind = vParams.x;
    if (kind < 0.5) {
        // 0: Smooth anti-aliased disc
        float d = length(vLocal);
        float aa = max(fwidth(d), 0.02);
        float cov = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, d);
        float a = vCol.a * cov;
        if (a <= 0.001) discard;
        O = vec4(vCol.rgb, a);
    } else if (kind < 1.5) {
        // 1: Smooth anti-aliased ring (vParams.y = inner norm, vParams.z = outer norm)
        float d = length(vLocal);
        float aa = max(fwidth(d), 0.02);
        float cov = smoothstep(vParams.y - aa, vParams.y + aa, d)
                  * (1.0 - smoothstep(vParams.z - aa, vParams.z + aa, d));
        float a = vCol.a * cov;
        if (a <= 0.001) discard;
        O = vec4(vCol.rgb, a);
    } else if (kind < 2.5) {
        // 2: Rounded rect / isometric workstation pad or pill label
        // vLocal in px relative to center; vParams.y = hw, vParams.z = hh, vParams.w = radius
        vec2 hs = vec2(vParams.y, vParams.z);
        float r = min(vParams.w, min(hs.x, hs.y));
        vec2 q = abs(vLocal) - (hs - vec2(r));
        float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
        float aa = max(fwidth(d), 0.75);
        float fillCov = 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, d);
        float strokeCov = 1.0 - smoothstep(0.65 - 0.5 * aa, 0.65 + 0.5 * aa, abs(d));
        float fa = vCol.a * fillCov;
        float sa = vStroke.a * strokeCov;
        float outA = sa + fa * (1.0 - sa);
        if (outA <= 0.001) discard;
        vec3 pm = vStroke.rgb * sa + vCol.rgb * fa * (1.0 - sa);
        O = vec4(pm / outA, outA);
    } else {
        // 3: Soft circular workstation platform (dark readable matte center + beveled rim)
        // vLocal in [-1.1, 1.1] normalized to outer radius; vParams.y = isoYScale (aspect)
        vec2 p = vec2(vLocal.x, vLocal.y);
        float d = length(p);
        float aa = max(fwidth(d), 0.018);
        float outerCov = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, d);
        if (outerCov <= 0.001) discard;
        // Subtle bevel shading + inner matte recess for icon clarity
        float rimBand = smoothstep(0.84, 0.94, d) * (1.0 - smoothstep(0.95, 1.01, d));
        float innerRing = smoothstep(0.70, 0.73, d) * (1.0 - smoothstep(0.73, 0.76, d));
        vec3 baseCol = mix(vCol.rgb * 0.88, vCol.rgb * 1.12, smoothstep(0.0, 0.9, d));
        vec3 col = mix(baseCol, vStroke.rgb, rimBand * vStroke.a + innerRing * vStroke.a * 0.38);
        float a = vCol.a * outerCov;
        O = vec4(col, a);
    }
}
"#;
