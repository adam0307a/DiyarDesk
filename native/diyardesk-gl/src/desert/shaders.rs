// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Desert Oasis (desert)
// Ported 1:1 from prototype/desert.html (WebGL2 background) and
// prototype/desert-props.js (instanced sprite batch).

// Fullscreen triangle strip generated from gl_VertexID, exactly like the prototype VS.
pub const DS_BG_VERT: &str = r#"#version 330 core
void main() {
    vec2 p = vec2(float(gl_VertexID & 1) * 2.0, float(gl_VertexID >> 1) * 2.0) - 1.0;
    gl_Position = vec4(p.x, -p.y, 0.0, 1.0);
}
"#;

// Golden Sahara sand sea + icon-driven dune mounds + oasis water + tap ripples.
pub const DS_BG_FRAG: &str = r#"#version 330 core
precision highp float;

uniform vec2 uRes;
uniform vec2 uGrid;
uniform float uTime;
uniform float uScale;
uniform float uMorph;
uniform float uWarmth;
uniform float uWind;
uniform float uDebug;
uniform float uSeed;
uniform float uAdaptive;
uniform int uOasesCount;
uniform vec4 uOases[4];
uniform vec4 uPulse;
uniform sampler2D uFieldA;
uniform sampler2D uFieldB;
uniform sampler2D uNoise;

out vec4 o;

float ss(float a, float b, float x) {
    float t = clamp((x - a) / (b - a), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

vec3 N(vec2 p, float s) {
    return texture(uNoise, p / (s * uScale)).rgb;
}

float hash2(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float sampleField(vec2 px) {
    vec2 uv = (px / uScale) / uGrid;
    float a = texture(uFieldA, uv).r;
    float b = texture(uFieldB, uv).r;
    return mix(a, b, uMorph);
}

void main() {
    vec2 p = vec2(gl_FragCoord.x, uRes.y - gl_FragCoord.y);
    vec2 cp = p / uScale;
    float nA = N(p + vec2(uSeed * 3.1, uSeed * 1.7), 520.0).r;
    float nB = N(p * 1.65 - vec2(uSeed * 2.3, 41.0), 190.0).g;
    float nFine = N(p * 3.1, 64.0).b;

    // Macro rolling Sahara dunes (SW -> NE crest orientation)
    vec2 rotP = vec2(cp.x * 0.86 + cp.y * 0.51, -cp.x * 0.51 + cp.y * 0.86);
    float macroWave = sin(rotP.y * 0.017 + nA * 4.4);
    float microRipple = sin(rotP.y * 0.24 + nB * 5.2 + sin(rotP.x * 0.04) * 2.1);

    // Height of icon-driven sand dune mound + wide smoothed gradient
    float fC = sampleField(p);
    float eps = 24.0 * uScale;
    float fL = sampleField(p - vec2(eps, 0.0));
    float fR = sampleField(p + vec2(eps, 0.0));
    float fU = sampleField(p - vec2(0.0, eps));
    float fD = sampleField(p + vec2(0.0, eps));
    vec2 grad = vec2(fR - fL, fD - fU) / (2.0 * eps);

    float duneZone = ss(-110.0, -5.0, fC) * uAdaptive;
    float duneEl = ss(-34.0, 48.0, fC) * uAdaptive;
    float lightDot = dot(normalize(vec2(-0.72, -0.68)), grad) * 14.0 * duneZone;
    float sunFace = lightDot / (1.0 + abs(lightDot));

    // Base warm Sahara sand palette (smooth silky rolling dune bands)
    vec3 sandDeep = vec3(0.79, 0.57, 0.31);
    vec3 sandMid  = vec3(0.90, 0.72, 0.44);
    vec3 sandLit  = vec3(0.97, 0.85, 0.59);
    vec3 col = mix(sandDeep, sandMid, 0.50 + 0.42 * macroWave);
    col = mix(col, sandLit, ss(0.28, 0.92, macroWave + (nB - 0.5) * 0.25) * 0.60);

    // Crisp wind-blown sand ripples modulated along dune crests
    float rip = 0.5 + 0.5 * microRipple;
    col += (rip - 0.5) * mix(0.045, 0.018, duneEl * 0.75) * vec3(0.95, 0.74, 0.42);
    col += (nFine - 0.5) * 0.018;

    // Sculpted sand dune mound under icons (sunlit NW ridge, warm amber SE slope)
    vec3 moundLit = mix(sandLit, vec3(0.99, 0.90, 0.68), 0.35);
    vec3 moundShad = vec3(0.73, 0.50, 0.25);
    col = mix(col, moundLit, duneEl * 0.48);
    col = mix(col, moundLit + vec3(0.04, 0.03, 0.0), ss(0.04, 0.75, sunFace) * 0.46 * duneZone);
    col = mix(col, moundShad, ss(-0.04, -0.78, sunFace) * 0.42 * duneZone);

    // Soft sunlit dune ridge highlight around icon sand mound
    float rim = exp(-pow(fC / 18.0, 2.0)) * duneZone;
    col += vec3(0.11, 0.08, 0.02) * rim * (0.45 + 0.55 * clamp(sunFace + 0.5, 0.0, 1.0));

    // Gentle warm contrast base under icon labels so white text stays crisp
    float iconCore = ss(18.0, 74.0, fC) * uAdaptive;
    col = mix(col, col * vec3(0.80, 0.72, 0.60), iconCore * 0.48);

    // Ambient drifting sand haze bands across open dunes
    float drift = sin((rotP.x - uTime * 36.0 * uWind) * 0.014 + nA * 3.0) * 0.5 + 0.5;
    col += vec3(0.05, 0.035, 0.01) * drift * (0.35 + 0.65 * uWarmth);

    // Oasis turquoise lakes nestled in low sand basins
    for (int i = 0; i < 4; i++) {
        if (i >= uOasesCount) break;
        vec4 oa = uOases[i];
        vec2 dp = (cp - oa.xy) / oa.zw;
        float ang = atan(dp.y, dp.x);
        float lobe = 1.0 + 0.14 * sin(ang * 3.0 + float(i) * 1.9 + uSeed) + 0.07 * cos(ang * 5.0 - float(i));
        float dOa = length(dp) / lobe;

        // Damp golden-olive shoreline & lush moss grass halo around oasis
        float wetSand = 1.0 - ss(0.95, 1.24, dOa);
        float greenRing = (1.0 - ss(0.88, 1.16, dOa)) * ss(0.76, 1.02, dOa);
        vec3 shoreCol = vec3(0.62, 0.48, 0.27);
        vec3 mossCol = vec3(0.28, 0.50, 0.25);
        col = mix(col, shoreCol, wetSand * 0.62);
        col = mix(col, mossCol, greenRing * 0.72 * (0.65 + 0.35 * nB));

        // Turquoise-emerald oasis water with shallow sandy shelf & caustic sparkle
        if (dOa < 1.04) {
            float depth = clamp(1.0 - dOa, 0.0, 1.0);
            vec3 shallowWater = vec3(0.27, 0.76, 0.72);
            vec3 deepWater    = vec3(0.06, 0.44, 0.50);
            vec3 wCol = mix(shallowWater, deepWater, pow(depth, 0.62));

            float t = uTime * max(0.2, uWind);
            vec2 wuv = p * vec2(0.8, 1.3) + vec2(t * 14.0, -t * 9.0);
            float c1 = N(wuv, 88.0).r;
            float c2 = N(wuv * 1.3 - vec2(t * 18.0, t * 7.0), 64.0).g;
            float shimmer = pow(ss(0.55, 0.94, c1 * 0.55 + c2 * 0.45), 2.0);
            wCol += vec3(0.55, 0.92, 0.88) * shimmer * (0.35 + 0.65 * uWarmth);

            // Interactive tap ripple on oasis water
            float dtTap = uTime - uPulse.z;
            if (uPulse.w > 0.5 && dtTap >= 0.0 && dtTap < 2.2) {
                float rd = length(cp - uPulse.xy);
                float waveR = dtTap * 68.0;
                float ring = exp(-pow((rd - waveR) / 5.5, 2.0)) * (1.0 - dtTap / 2.2);
                wCol += vec3(0.45, 0.80, 0.80) * ring * 0.7;
            }

            float wMask = 1.0 - ss(0.92, 1.0, dOa);
            col = mix(col, wCol, wMask);
        }
    }

    // Sand tap shockwave ring if clicked on open sand
    float dtSand = uTime - uPulse.z;
    if (uPulse.w < 0.5 && dtSand >= 0.0 && dtSand < 1.5) {
        float rd = length(cp - uPulse.xy);
        float ring = exp(-pow((rd - dtSand * 82.0) / 7.5, 2.0)) * (1.0 - dtSand / 1.5);
        col += vec3(0.18, 0.13, 0.04) * ring;
    }

    // Warm sunlight golden wash from upper-left + film grain
    float sunGrad = clamp(1.0 - (p.x * 0.45 + p.y * 0.55) / (uRes.x * 0.45 + uRes.y * 0.55), 0.0, 1.0);
    col += vec3(0.10, 0.065, 0.01) * sunGrad * uWarmth;
    col *= 1.0 + (hash2(floor(p * (0.85 * max(uScale, 1.0)))) - 0.5) * 0.024;

    if (uDebug > 0.5) col = mix(col, vec3(0.95, 0.25, 0.15), 1.0 - ss(0.0, 1.8, abs(fC)));
    o = vec4(col, 1.0);
}
"#;

// Instanced sprite batch: aQuad (unit quad), aBox (x, y, size, rot), aInfo (col, alpha, 0, 0)
pub const DS_SPRITE_VERT: &str = r#"#version 330 core
layout (location = 0) in vec2 aQuad;
layout (location = 1) in vec4 aBox;
layout (location = 2) in vec4 aInfo;

uniform vec2 uRes;

out vec2 vUV;
out float vAlpha;

void main() {
    float c = cos(aBox.w);
    float s = sin(aBox.w);
    vec2 p = aQuad * aBox.z;
    vec2 wp = aBox.xy + vec2(p.x * c - p.y * s, p.x * s + p.y * c);
    vec2 clip = (wp / uRes) * 2.0 - 1.0;
    gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
    float col = aInfo.x;
    vUV = vec2((col + aQuad.x + 0.5) / 10.0, aQuad.y + 0.5);
    vAlpha = aInfo.y;
}
"#;

pub const DS_SPRITE_FRAG: &str = r#"#version 330 core
in vec2 vUV;
in float vAlpha;
out vec4 o;

uniform sampler2D uAtlas;

void main() {
    vec4 t = texture(uAtlas, vUV);
    o = vec4(t.rgb * vAlpha, t.a * vAlpha);
}
"#;
