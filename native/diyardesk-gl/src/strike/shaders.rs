// Bombsite Dust (Bomba Alanı Tozu / strike) OpenGL 3.3 Core shaders —
// 1:1 port of prototype/strike.html + prototype/strike-props.js

pub const STRIKE_VERT: &str = r#"#version 330 core
void main() {
    gl_Position = vec4(vec2((gl_VertexID & 1) << 2, (gl_VertexID & 2) << 1) - 1.0, 0.0, 1.0);
}
"#;

pub const STRIKE_FRAG: &str = r#"#version 330 core
uniform sampler2D uNoise, uFA, uFB;
uniform vec2 uRes, uGrid;
uniform float uScale, uSeed, uStep, uMix, uTime, uHoles, uSmoke, uWind, uDebug;
out vec4 o;

const vec3 SAND_SUN    = vec3(0.845, 0.720, 0.525);
const vec3 SAND_MID    = vec3(0.745, 0.605, 0.415);
const vec3 SAND_SHADE  = vec3(0.495, 0.395, 0.290);
const vec3 CONC_LIGHT  = vec3(0.555, 0.525, 0.475);
const vec3 CONC_MID    = vec3(0.445, 0.420, 0.382);
const vec3 CONC_DARK   = vec3(0.320, 0.300, 0.275);
const vec3 HAZARD_YEL  = vec3(0.880, 0.650, 0.145);

vec2 hash22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}
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

// Procedural concrete bullet hole craters + radial shatter cracks + gunpowder soot
vec3 applyBulletGrid(vec2 p, vec3 baseCol, float cellSize, float holeProb, float rScale, float mask) {
    if (mask <= 0.01) return baseCol;
    vec2 cell = floor(p / cellSize);
    vec2 f = fract(p / cellSize);
    float rnd = hash2(cell + uSeed * 3.1);
    if (rnd > holeProb) return baseCol;

    vec2 jitter = 0.22 + 0.56 * hash22(cell + 17.3);
    vec2 d = (f - jitter) * cellSize;
    float r = length(d);
    float holeR = (2.6 + 2.4 * hash2(cell + 41.7)) * rScale;

    if (r > holeR * 4.5) return baseCol;

    float ang = atan(d.y, d.x);
    float phase = hash2(cell + 73.1) * 6.283;
    // Jagged concrete radial cracks radiating out from entry hole
    float rayWave = pow(abs(cos(ang * 3.0 + phase + sin(r * 0.35 + phase) * 0.45)), 10.0)
                  + pow(abs(cos(ang * 2.5 - phase * 1.4)), 14.0) * 0.7;
    float crack = clamp(rayWave, 0.0, 1.0)
        * (1.0 - ss(holeR * 0.9, holeR * 3.8, r))
        * ss(holeR * 0.6, holeR * 1.1, r);

    // Pale shattered concrete/plaster spall collar around bore
    float spall = (1.0 - ss(holeR * 0.92, holeR * 1.62, r)) * ss(holeR * 0.58, holeR * 1.02, r);
    // Subtle directional 3D rim highlight on lower-right inner edge (sunlit from top-left)
    float rimLit = clamp(dot(normalize(d + 1e-4), vec2(0.707, 0.707)), 0.0, 1.0) * spall;
    // Tight crisp gunpowder soot collar
    float soot = (1.0 - ss(holeR * 0.85, holeR * 2.05, r)) * 0.30;
    // Deep dark conical center bore
    float bore = 1.0 - ss(holeR * 0.52, holeR * 0.92, r);

    vec3 col = baseCol;
    col = mix(col, vec3(0.10, 0.09, 0.08), clamp(soot + crack * 0.82, 0.0, 1.0) * mask);
    col = mix(col, vec3(0.76, 0.72, 0.66), (spall * 0.62 + rimLit * 0.42) * mask);
    col = mix(col, vec3(0.055, 0.048, 0.042), bore * 0.94 * mask);
    return col;
}

vec3 paintCourtyard(vec2 p, vec2 uv) {
    float n1 = N(p, 1400.0).r;
    float n2 = N(p + 193.0, 460.0).g;
    float n3 = N(p + 611.0, 160.0).b;

    // Classic sandstone paving flagstones
    vec2 tile = floor(p / 64.0);
    vec2 tuv = fract(p / 64.0);
    float tRnd = hash2(tile + uSeed);
    float bevel = min(min(tuv.x, 1.0 - tuv.x), min(tuv.y, 1.0 - tuv.y));
    float seam = 1.0 - ss(0.02, 0.075, bevel);

    vec3 stone = mix(SAND_MID, SAND_SUN, tRnd * 0.62 + n2 * 0.38);
    stone = mix(stone, stone * 0.78, seam * 0.58);

    // Drifting warm windblown sand ripples over paving seams
    float flow = uTime * (0.25 + 0.75 * uWind);
    float sandDrift = ss(0.48, 0.84, n1 * 0.55 + N(p + vec2(-flow * 16.0, flow * 7.0), 320.0).g * 0.45);
    stone = mix(stone, SAND_SUN * 1.04, sandDrift * 0.42);

    // Tactical dusty lane line across diagonal mid courtyard
    float laneDist = abs((uv.x * 0.62 + uv.y * 0.38) - 0.51) * min(uRes.x, uRes.y);
    float lanePaint = (1.0 - ss(4.0, 12.0, abs(laneDist - 12.0))) * ss(0.32, 0.72, n3);
    stone = mix(stone, vec3(0.84, 0.66, 0.34), lanePaint * 0.32);

    // Dramatic Dust/Mirage rooftop diagonal cast shadow across top-right & bottom-left corners
    float diag = uv.x * 0.75 - uv.y * 0.65 + (n1 - 0.5) * 0.08;
    float shadeMask = ss(0.32, 0.56, diag) * 0.52 + ss(0.55, 0.92, length(uv - vec2(0.5)) * 1.1) * 0.18;
    stone = mix(stone, SAND_SHADE, clamp(shadeMask, 0.0, 0.62));

    // Subtle sunlit heat dust shimmer with smoke slider
    float haze = ss(0.35, 0.85, N(p + vec2(flow * 22.0, -flow * 10.0), 700.0).r) * uSmoke;
    stone = mix(stone, vec3(0.88, 0.82, 0.72), haze * 0.16);

    return stone;
}

vec3 paintImpactZone(vec2 p, float s) {
    float n2 = N(p + 317.0, 340.0).g;
    float n3 = N(p + 823.0, 110.0).b;

    // Weathered tactical concrete slab beneath icon clearings
    vec2 slab = floor(p / 72.0);
    vec2 suv = fract(p / 72.0);
    float slabRnd = hash2(slab + uSeed * 2.0);
    float bevel = min(min(suv.x, 1.0 - suv.x), min(suv.y, 1.0 - suv.y));
    float joint = 1.0 - ss(0.018, 0.06, bevel);

    vec3 conc = mix(CONC_DARK, CONC_MID, slabRnd * 0.5 + n2 * 0.5);
    conc = mix(conc, CONC_LIGHT, ss(0.52, 0.88, n3) * 0.45);
    conc = mix(conc, conc * 0.74, joint * 0.55);

    // Bullet hole craters clustered cleanly inside/around icon pads
    float innerMask = (1.0 - ss(-16.0, -2.0, s)) * clamp(uHoles, 0.0, 2.0);
    float pDense = min(0.78, 0.28 * uHoles);
    float pHeavy = min(0.68, 0.20 * uHoles);
    conc = applyBulletGrid(p + vec2(19.0, 31.0), conc, 32.0, pDense, 0.86, innerMask * 0.85);
    conc = applyBulletGrid(p * 1.02 + vec2(83.0, 17.0), conc, 48.0, pHeavy, 1.24, innerMask * 0.95);

    return conc;
}

void main() {
    vec2 p = vec2(gl_FragCoord.x, uRes.y * uScale - gl_FragCoord.y) / uScale;
    vec2 uv = p / uRes;
    float s = mix(fieldS(uFA, p), fieldS(uFB, p), uMix);

    vec3 court = paintCourtyard(p, uv);
    vec3 zone  = paintImpactZone(p, s);

    float inside = 1.0 - ss(-3.5, 3.5, s);
    vec3 col = mix(court, zone, inside);

    // Diagonal yellow-and-charcoal tactical hazard curb right along concrete perimeter
    float curbBand = 1.0 - ss(0.0, 5.5, abs(s + 2.0));
    float stripe = step(0.5, fract((p.x + p.y) * 0.055));
    vec3 hazardCol = mix(vec3(0.16, 0.14, 0.12), HAZARD_YEL, stripe);
    col = mix(col, hazardCol, curbBand * 0.72);

    // Chipped concrete lip cast shadow onto warm courtyard sand
    float lipShadow = (1.0 - ss(1.5, 16.0, s)) * ss(-1.0, 5.0, s);
    col *= 1.0 - lipShadow * 0.32;

    // Painterly warm desert sunlight gradient + crisp stone micro-grain
    col *= 1.0 + 0.06 * (0.5 - (p.x + p.y) / (uRes.x + uRes.y));
    col *= 1.0 + (hash2(floor(p * (0.85 * max(uScale, 1.0)))) - 0.5) * 0.026;

    if (uDebug > 0.5) col = mix(col, vec3(0.96, 0.26, 0.20), 1.0 - ss(0.0, 1.6, abs(s)));
    o = vec4(col, 1.0);
}
"#;

pub const STRIKE_INST_VERT: &str = r#"#version 330 core
layout(location = 0) in vec2 aQuad;
layout(location = 1) in vec2 aPos;
layout(location = 2) in float aSize;
layout(location = 3) in float aRot;
layout(location = 4) in vec2 aCell;
layout(location = 5) in float aAlpha;
layout(location = 6) in float aFlip;

uniform vec2 uRes;
uniform vec2 uGridAtlas;

out vec2 vUv;
out float vAlpha;

void main() {
    float c = cos(aRot);
    float s = sin(aRot);
    vec2 q = vec2(aQuad.x * aFlip, aQuad.y);
    vec2 rot = vec2(q.x * c - q.y * s, q.x * s + q.y * c) * aSize;
    vec2 wp = aPos + rot;
    vec2 ndc = vec2(wp.x / uRes.x * 2.0 - 1.0, 1.0 - wp.y / uRes.y * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
    vUv = (aCell + (aQuad + 0.5)) / uGridAtlas;
    vAlpha = aAlpha;
}
"#;

pub const STRIKE_INST_FRAG: &str = r#"#version 330 core
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
