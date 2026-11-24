// Rose Brook (Gül Deresi) OpenGL 3.3 Core shaders — 1:1 port of
// prototype/rosebrook.html + prototype/rosebrook-blooms.js

pub const ROSEBROOK_VERT: &str = r#"#version 330 core
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const ROSEBROOK_FRAG: &str = r#"#version 330 core
uniform sampler2D uNoise, uFA, uFB;
uniform vec2 uRes, uGrid;
uniform float uScale, uSeed, uStep, uMix, uTime, uStreamW, uFlow, uSun, uSlope, uCy, uDebug;
out vec4 o;

float hash2(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}
float ss(float a, float b, float x) {
    float t = clamp((x - a) / (b - a), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}
vec4 N(vec2 p, float sc) { return texture(uNoise, p / sc + uSeed * 0.013); }
float fieldS(sampler2D t, vec2 p) { return texture(t, (p / uStep + 0.5) / uGrid).r; }

void main() {
    vec2 p = vec2(gl_FragCoord.x, uRes.y * uScale - gl_FragCoord.y) / uScale;
    float s = mix(fieldS(uFA, p), fieldS(uFB, p), uMix);

    vec4 n1 = N(p, 1500.0);
    vec4 n2 = N(p + 271.0, 520.0);
    vec4 n3 = N(p + 613.0, 180.0);
    vec3 grassDeep = vec3(0.18, 0.43, 0.22);
    vec3 grassMid  = vec3(0.31, 0.62, 0.29);
    vec3 grassSun  = vec3(0.56, 0.80, 0.37);
    vec3 grassWarm = vec3(0.66, 0.84, 0.44);

    vec3 col = mix(grassDeep, grassMid, ss(0.24, 0.72, n1.r * 0.65 + n2.g * 0.35));
    col = mix(col, grassSun, ss(0.45, 0.82, n2.b * 0.6 + n3.r * 0.4) * (0.45 + 0.45 * uSun));
    float lawn = 1.0 - ss(-18.0, 26.0, s);
    col = mix(col, mix(col, grassWarm, 0.42), lawn);
    col *= 0.96 + 0.08 * n3.g;

    float nx = p.x / max(uRes.x, 1.0);
    float cy = uRes.y * (uCy + uSlope * (nx - 0.5) + 0.115 * sin(nx * 5.2 + uSeed * 1.1) + 0.042 * sin(nx * 11.4 + uSeed * 2.3));
    float hw = (34.0 + 9.0 * sin(nx * 8.0 + uSeed)) * uStreamW;
    float edgeNoise = (N(p, 260.0).r - 0.5) * 12.0 + (N(p, 95.0).g - 0.5) * 4.5;
    float dist = abs(p.y - cy) - hw + edgeNoise;

    float bankBand = 1.0 - ss(-2.0, 26.0, dist);
    vec3 mossCol = vec3(0.20, 0.45, 0.26);
    col = mix(col, mossCol, bankBand * 0.55);

    if (dist < 10.0) {
        float pebble = N(p * vec2(1.0, 1.35), 48.0).b;
        vec3 bedCol = mix(vec3(0.28, 0.38, 0.35), vec3(0.55, 0.61, 0.53), ss(0.32, 0.72, pebble));
        float depth = clamp(-dist / max(hw, 1.0), 0.0, 1.0);
        vec3 shallowW = vec3(0.24, 0.72, 0.68);
        vec3 deepW    = vec3(0.10, 0.47, 0.53);
        vec3 waterCol = mix(shallowW, deepW, pow(depth, 0.65));

        float t = uTime * uFlow;
        vec2 flowUV = vec2(p.x - t * 44.0, p.y - (p.y - cy) * 0.12);
        float c1 = N(flowUV * vec2(0.55, 1.8), 160.0).r;
        float c2 = N((flowUV + vec2(-t * 28.0, 40.0)) * vec2(0.8, 2.2), 110.0).g;
        float caustic = ss(0.56, 0.88, c1 * 0.55 + c2 * 0.45);
        float sparkle = pow(ss(0.68, 0.95, c1 * c2 * 1.45), 2.0) * (0.35 + 0.65 * uSun);

        vec3 streamFinal = mix(bedCol, waterCol, 0.42 + 0.48 * depth);
        streamFinal += vec3(0.18, 0.40, 0.38) * caustic * (0.4 + 0.4 * uSun);
        streamFinal += vec3(0.75, 0.96, 0.98) * sparkle * 0.65;

        float foam = (1.0 - ss(-6.0, 3.0, dist)) * ss(0.45, 0.75, N(flowUV, 72.0).b);
        streamFinal = mix(streamFinal, vec3(0.88, 0.98, 0.96), foam * 0.55);

        float waterMask = 1.0 - ss(-3.0, 6.0, dist);
        col = mix(col, streamFinal, waterMask);
    }

    float sunGrad = clamp(1.0 - (p.x * 0.4 + p.y * 0.6) / (uRes.x * 0.4 + uRes.y * 0.6), 0.0, 1.0);
    col += vec3(0.08, 0.07, 0.01) * sunGrad * uSun;
    col *= 1.0 + (hash2(floor(p * (0.8 * max(uScale, 1.0)))) - 0.5) * 0.025;

    if (uDebug > 0.5) col = mix(col, vec3(0.95, 0.28, 0.35), 1.0 - ss(0.0, 1.6, abs(s)));
    o = vec4(col, 1.0);
}
"#;

pub const ROSEBROOK_INST_VERT: &str = r#"#version 330 core
layout(location = 0) in vec4 aInstA; // x, y, scale, variant
layout(location = 1) in vec4 aInstB; // rot, phase, sway, alpha
uniform vec2 uRes;
uniform float uTime, uBreeze;
out vec2 vUV;
out float vAlpha;

void main() {
    vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
    vec2 local = (corner - 0.5) * 128.0 * aInstA.z;
    float c = cos(aInstB.x), s = sin(aInstB.x);
    vec2 rotP = vec2(local.x * c - local.y * s, local.x * s + local.y * c);
    float gust = sin(uTime * (1.25 + 0.75 * uBreeze) + aInstB.y + aInstA.x * 0.006);
    rotP.x += gust * 3.8 * aInstB.z * (0.25 + 0.75 * min(uBreeze, 2.2));
    vec2 p = aInstA.xy + rotP;
    gl_Position = vec4(p.x / uRes.x * 2.0 - 1.0, 1.0 - p.y / uRes.y * 2.0, 0.0, 1.0);
    vUV = vec2((corner.x + aInstA.w) / 12.0, corner.y);
    vAlpha = aInstB.w;
}
"#;

pub const ROSEBROOK_INST_FRAG: &str = r#"#version 330 core
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
