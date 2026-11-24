// Runic Rift (Rünik Vadi) OpenGL 3.3 Core shaders — 1:1 port of
// prototype/rift.html + prototype/rift-props.js

pub const RIFT_VERT: &str = r#"#version 330 core
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const RIFT_FRAG: &str = r#"#version 330 core
uniform sampler2D uNoise, uFA, uFB;
uniform vec2 uRes, uGrid;
uniform float uScale, uSeed, uStep, uMix, uTime, uGlow, uWind, uDebug;
out vec4 o;

const vec3 FOREST_DARK = vec3(0.080, 0.165, 0.135);
const vec3 FOREST_MOSS = vec3(0.175, 0.335, 0.235);
const vec3 FOREST_LEAF = vec3(0.365, 0.515, 0.270);
const vec3 VOID_INDIGO = vec3(0.145, 0.105, 0.225);

const vec3 STONE_DARK  = vec3(0.285, 0.335, 0.345);
const vec3 STONE_MID   = vec3(0.395, 0.445, 0.435);
const vec3 STONE_LIGHT = vec3(0.515, 0.545, 0.495);

const vec3 BRASS_GOLD   = vec3(0.860, 0.680, 0.320);
const vec3 HEXTECH_CYAN = vec3(0.22, 0.94, 0.89);

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

vec3 paintWilderness(vec2 p, vec2 uv) {
    float n1 = N(p, 1600.0).r;
    float n2 = N(p + 271.0, 520.0).g;
    float n3 = N(p + 713.0, 210.0).b;

    vec3 col = mix(FOREST_DARK, FOREST_MOSS, ss(0.28, 0.72, n1 * 0.65 + n2 * 0.35));
    col = mix(col, FOREST_LEAF, ss(0.55, 0.85, n2 * 0.6 + n3 * 0.4) * 0.55);

    // Corner Baron-pit void shadow nuance
    float cornerVignette = ss(0.45, 1.15, length(uv - vec2(0.5)) * 1.45) * ss(0.42, 0.78, n1);
    col = mix(col, VOID_INDIGO, cornerVignette * 0.52);

    // Serpentine Rift River meandering diagonally across desktop
    float riverCenterY = 0.56 - 0.18 * (uv.x - 0.5)
        + 0.095 * sin(uv.x * 4.6 + uSeed * 0.7)
        + 0.035 * cos(uv.x * 9.2 - uSeed)
        + (N(p, 900.0).r - 0.5) * 0.036;
    float rDist = abs(uv.y - riverCenterY) * uRes.y;
    float riverHalfW = 42.0 + 12.0 * sin(uv.x * 6.0 + uSeed);

    if (rDist < riverHalfW + 34.0) {
        float bankMask = 1.0 - ss(riverHalfW - 4.0, riverHalfW + 24.0, rDist);
        float deepMask = 1.0 - ss(riverHalfW * 0.18, riverHalfW * 0.90, rDist);
        // Warm wet river slate bank
        col = mix(col, vec3(0.27, 0.33, 0.32), bankMask * 0.86);

        // Smooth translucent aqua water surface + gentle flowing highlights
        float flow = uTime * (0.35 + 0.65 * uWind);
        vec2 wp = vec2(p.x * 0.75 - flow * 34.0, p.y * 1.35 + flow * 14.0);
        float caus1 = N(wp, 260.0).g;
        float caus2 = N(wp * vec2(1.15, 0.85) + vec2(flow * 18.0, -flow * 11.0), 170.0).b;
        float ripple = ss(0.46, 0.84, caus1 * 0.55 + caus2 * 0.45);

        vec3 waterCol = mix(vec3(0.11, 0.42, 0.46), vec3(0.06, 0.24, 0.33), deepMask);
        waterCol += vec3(0.12, 0.44, 0.46) * ripple * (0.36 + 0.44 * uGlow);

        // Soft glowing center current streak
        float centerStreak = (1.0 - ss(0.0, riverHalfW * 0.48, rDist))
            * (0.28 + 0.72 * uGlow)
            * (0.65 + 0.35 * sin(flow * 1.4 + uv.x * 7.0));
        waterCol += HEXTECH_CYAN * centerStreak * 0.14;

        float waterMask = 1.0 - ss(riverHalfW - 9.0, riverHalfW + 3.0, rDist);
        col = mix(col, waterCol, waterMask);
    }

    return col;
}

vec3 paintSanctuary(vec2 p, float s) {
    // Carved weathered Summoner's Rift plaza flagstones
    vec2 rotP = vec2(p.x * 0.707 + p.y * 0.707, -p.x * 0.707 + p.y * 0.707);
    vec2 tileIdx = floor(rotP / 46.0);
    vec2 tileUv = fract(rotP / 46.0);
    float tileRnd = hash2(tileIdx + uSeed);
    float bevel = min(min(tileUv.x, 1.0 - tileUv.x), min(tileUv.y, 1.0 - tileUv.y));
    float mortar = 1.0 - ss(0.025, 0.085, bevel);

    float tex = N(p, 380.0).g;
    vec3 stone = mix(STONE_DARK, STONE_MID, tileRnd * 0.65 + tex * 0.35);
    stone = mix(stone, STONE_LIGHT, ss(0.56, 0.88, tex) * 0.42);
    stone = mix(stone, stone * 0.68, mortar * 0.65);

    // Moss creeping gently through outer seams near the cliff edge
    float mossWeight = ss(-48.0, -4.0, s) * mortar;
    stone = mix(stone, vec3(0.24, 0.40, 0.25), mossWeight * 0.72);

    // Crisp brass & cyan Hextech etched rune conduits inside courtyard
    float pulse = 0.65 + 0.35 * sin(uTime * (0.6 + 1.4 * uWind) - length(p) * 0.015);
    float innerGoldRing = (1.0 - ss(0.0, 2.6, abs(s + 24.0))) * ss(-50.0, -18.0, s);
    float innerCyanVein = (1.0 - ss(0.0, 2.5, abs(s + 14.0))) * (0.45 + 0.55 * pulse) * uGlow;

    float notch = step(0.45, sin(atan(p.y - uRes.y * 0.5, p.x - uRes.x * 0.5) * 28.0 + s * 0.08));
    stone = mix(stone, BRASS_GOLD, innerGoldRing * 0.85);
    stone += HEXTECH_CYAN * innerCyanVein * (0.34 + 0.44 * notch);

    return stone;
}

void main() {
    vec2 p = vec2(gl_FragCoord.x, uRes.y * uScale - gl_FragCoord.y) / uScale;
    vec2 uv = p / uRes;
    float s = mix(fieldS(uFA, p), fieldS(uFB, p), uMix);

    vec3 wild = paintWilderness(p, uv);
    vec3 sanct = paintSanctuary(p, s);

    float inside = 1.0 - ss(-3.0, 4.0, s);
    vec3 col = mix(wild, sanct, inside);

    // Carved stone rim + golden Hextech trim right at s = 0
    float rimStone = 1.0 - ss(0.0, 6.0, abs(s - 1.5));
    col = mix(col, vec3(0.42, 0.45, 0.41), rimStone * 0.72);

    float goldTrim = 1.0 - ss(0.0, 2.2, abs(s + 2.5));
    col = mix(col, BRASS_GOLD, goldTrim * 0.88);

    // Outer drop shadow from stone sanctuary lip into jungle moss
    float lipShadow = (1.0 - ss(2.0, 18.0, s)) * ss(-1.0, 6.0, s);
    col *= 1.0 - lipShadow * 0.34;

    // Glowing cyan arcane sigil dots spaced along threshold
    float glyphWave = sin((p.x + p.y) * 0.14 + uTime * (0.4 + uWind * 1.1));
    float rimCyan = (1.0 - ss(0.0, 2.4, abs(s + 0.2))) * ss(0.35, 0.92, glyphWave) * uGlow;
    col += HEXTECH_CYAN * rimCyan * 0.75;

    // Painterly top-left sunlight gradient + micro grain
    col *= 1.0 + 0.055 * (0.5 - (p.x + p.y) / (uRes.x + uRes.y));
    col *= 1.0 + (hash2(floor(p * (0.8 * max(uScale, 1.0)))) - 0.5) * 0.028;

    if (uDebug > 0.5) col = mix(col, vec3(0.95, 0.28, 0.24), 1.0 - ss(0.0, 1.6, abs(s)));
    o = vec4(col, 1.0);
}
"#;

pub const RIFT_INST_VERT: &str = r#"#version 330 core
layout(location = 0) in vec4 aPosScale; // x, y, sizePx, rot
layout(location = 1) in vec4 aUvInfo;   // u0, v0, alpha, flipX
uniform vec2 uRes;
out vec2 vUv;
out float vAlpha;

void main() {
    int id = gl_VertexID;
    vec2 q = vec2(float(id & 1), float((id >> 1) & 1));
    if (id == 3) q = vec2(1.0, 0.0);
    else if (id == 4) q = vec2(0.0, 1.0);
    else if (id == 5) q = vec2(1.0, 1.0);

    vec2 local = vec2(q.x - 0.5, q.y - 0.86) * aPosScale.z;
    float c = cos(aPosScale.w), s = sin(aPosScale.w);
    vec2 rot = vec2(local.x * c - local.y * s, local.x * s + local.y * c);
    vec2 pos = aPosScale.xy + rot;
    vec2 ndc = vec2(pos.x / uRes.x * 2.0 - 1.0, 1.0 - pos.y / uRes.y * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);

    float fx = aUvInfo.w < 0.0 ? (1.0 - q.x) : q.x;
    vUv = aUvInfo.xy + vec2(fx / 8.0, q.y / 3.0);
    vAlpha = aUvInfo.z;
}
"#;

pub const RIFT_INST_FRAG: &str = r#"#version 330 core
uniform sampler2D uAtlas;
in vec2 vUv;
in float vAlpha;
out vec4 o;

void main() {
    vec4 t = texture(uAtlas, vUv);
    t.a *= vAlpha;
    if (t.a < 0.015) discard;
    o = t;
}
"#;
