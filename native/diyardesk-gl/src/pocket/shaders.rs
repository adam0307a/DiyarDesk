pub const POCKET_VERT: &str = r#"#version 330 core
void main() {
    vec2 p = vec2(float((gl_VertexID & 1) << 2), float((gl_VertexID & 2) << 1)) - 1.0;
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;

pub const POCKET_FRAG: &str = r#"#version 330 core
precision highp float;
uniform sampler2D uNoise;
uniform sampler2D uFA;
uniform sampler2D uFB;
uniform vec2 uRes;
uniform vec2 uGrid;
uniform float uScale;
uniform float uSeed;
uniform float uStep;
uniform float uMix;
uniform float uTime;
uniform float uArrows;
uniform float uPace;
uniform float uSupply;
uniform float uDebug;
out vec4 o;

const vec3 MAP_LOW     = vec3(0.450, 0.495, 0.365);
const vec3 MAP_MID     = vec3(0.595, 0.585, 0.435);
const vec3 MAP_HIGH    = vec3(0.710, 0.655, 0.505);
const vec3 POCKET_DARK = vec3(0.320, 0.215, 0.195);
const vec3 FRONTLINE_R = vec3(0.860, 0.210, 0.180);
const vec3 ALLY_GLOW   = vec3(0.240, 0.720, 0.920);
const vec3 GOLD_ARROW  = vec3(0.950, 0.760, 0.320);

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
float curS(vec2 p) {
    return mix(fieldS(uFA, p), fieldS(uFB, p), uMix);
}

vec3 paintStrategyMap(vec2 p, vec2 uv) {
    float h1 = N(p, 1500.0).r;
    float h2 = N(p + 231.0, 560.0).g;
    float elev = h1 * 0.68 + h2 * 0.32;

    vec3 col = mix(MAP_LOW, MAP_MID, ss(0.26, 0.62, elev));
    col = mix(col, MAP_HIGH, ss(0.58, 0.88, elev));

    // Vintage cartographic elevation contour lines (topographic map feel)
    float iso = abs(fract(elev * 9.0) - 0.5);
    float contour = 1.0 - ss(0.02, 0.09, iso);
    col = mix(col, col * 0.78, contour * (0.28 + 0.42 * uSupply));

    // Province border grid + tactical coordinate lines
    float provNoise = N(p * 0.85 + 91.0, 420.0).b;
    float provEdge = 1.0 - ss(0.0, 0.028, abs(provNoise - 0.5));
    col = mix(col, vec3(0.24, 0.22, 0.18), provEdge * 0.55);

    vec2 gridUv = fract(p / 120.0);
    float gLine = 1.0 - ss(0.0, 0.016, min(gridUv.x, gridUv.y));
    col = mix(col, vec3(0.35, 0.34, 0.28), gLine * 0.26 * uSupply);

    // Serpentine tactical supply river crossing the theater map
    float riverY = 0.52 + 0.14 * sin(uv.x * 5.2 + uSeed * 0.8) - 0.08 * (uv.x - 0.5);
    float rDist = abs(uv.y - riverY) * uRes.y;
    float rMask = 1.0 - ss(8.0, 14.0, rDist);
    col = mix(col, vec3(0.22, 0.42, 0.52), rMask * 0.72);

    return col;
}

void main() {
    vec2 p = vec2(gl_FragCoord.x, uRes.y * uScale - gl_FragCoord.y) / uScale;
    vec2 uv = p / uRes;
    float s = curS(p);

    vec3 mapCol = paintStrategyMap(p, uv);

    // Encircled pocket interior (s < 0): crimson occupation cross-hatch shading
    float inside = 1.0 - ss(-2.5, 2.5, s);
    float hatchCoord = (p.x + p.y) * 0.16;
    float hatch = step(0.55, fract(hatchCoord));
    vec3 pocketCol = mix(mapCol * 0.62, POCKET_DARK, 0.68);
    pocketCol = mix(pocketCol, vec3(0.62, 0.16, 0.14), hatch * 0.36);

    vec3 col = mix(mapCol, pocketCol, inside);

    // Dual HOI4/AoH Frontline perimeter right at s = 0
    float enemyDefense = 1.0 - ss(0.0, 3.2, abs(s + 2.2));
    float allyFront = 1.0 - ss(0.0, 3.2, abs(s - 2.6));
    col = mix(col, FRONTLINE_R, enemyDefense * 0.92);
    col = mix(col, ALLY_GLOW, allyFront * (0.45 + 0.55 * uArrows));

    // Animated inward offensive thrust chevrons pulsing along pocket belt s in [6..26]
    float arrowBelt = ss(4.0, 9.0, s) * (1.0 - ss(20.0, 28.0, s));
    if (arrowBelt > 0.01 && uArrows > 0.02) {
        vec2 grad = vec2(curS(p + vec2(5.0, 0.0)) - curS(p - vec2(5.0, 0.0)),
                         curS(p + vec2(0.0, 5.0)) - curS(p - vec2(0.0, 5.0)));
        float flow = sin(s * 0.42 + uTime * (1.2 + uPace * 2.4));
        float tang = sin((p.x - p.y + length(grad) * 2.0) * 0.11);
        float chevron = ss(0.35, 0.88, flow) * ss(-0.1, 0.6, tang);
        col = mix(col, GOLD_ARROW, chevron * arrowBelt * uArrows * 0.72);
    }

    // Subtle vintage parchment edge vignette & paper grain
    float vig = ss(0.45, 1.05, length(uv - vec2(0.5)) * 1.25);
    col *= 1.0 - vig * 0.24;
    col *= 1.0 + (hash2(floor(p * (0.85 * max(uScale, 1.0)))) - 0.5) * 0.024;

    if (uDebug > 0.5) col = mix(col, vec3(1.0, 0.3, 0.2), 1.0 - ss(0.0, 1.6, abs(s)));
    o = vec4(col, 1.0);
}
"#;

pub const POCKET_INST_VERT: &str = r#"#version 330 core
layout(location = 0) in vec2 aQuad;
layout(location = 1) in vec2 aPos;
layout(location = 2) in float aSize;
layout(location = 3) in float aRot;
layout(location = 4) in vec2 aCell;
layout(location = 5) in float aAlpha;

uniform vec2 uRes;
uniform vec2 uGridAtlas;

out vec2 vUv;
out float vAlpha;

void main() {
    float c = cos(aRot);
    float s = sin(aRot);
    vec2 rot = vec2(aQuad.x * c - aQuad.y * s, aQuad.x * s + aQuad.y * c) * aSize;
    vec2 wp = aPos + rot;
    vec2 ndc = vec2(wp.x / uRes.x * 2.0 - 1.0, 1.0 - wp.y / uRes.y * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
    vUv = (aCell + (aQuad + 0.5)) / uGridAtlas;
    vAlpha = aAlpha;
}
"#;

pub const POCKET_INST_FRAG: &str = r#"#version 330 core
precision highp float;
in vec2 vUv;
in float vAlpha;
uniform sampler2D uAtlas;
out vec4 o;
void main() {
    vec4 t = texture(uAtlas, vUv);
    if (t.a * vAlpha < 0.01) discard;
    o = vec4(t.rgb, t.a * vAlpha);
}
"#;
