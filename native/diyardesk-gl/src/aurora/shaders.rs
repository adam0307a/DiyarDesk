// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Aurora Lake (Aurora Gölü)
// Ported 1:1 from prototype/aurora.html and prototype/aurora-pines.js

pub const AURORA_VERT: &str = r#"#version 330 core
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const AURORA_FRAG: &str = r#"#version 330 core
uniform sampler2D uNoise, uFA, uFB;
uniform vec2 uRes, uGrid;
uniform float uScale, uSeed, uStep, uMix, uTime, uFlowTime, uDebug;
uniform float uShoal, uAurora;
uniform vec4 uRip[6];
uniform int uRipN;
out vec4 o;

const vec3 ICE_DEEP  = vec3(0.032, 0.075, 0.145);
const vec3 ICE_MID   = vec3(0.055, 0.145, 0.235);
const vec3 ICE_SHOAL = vec3(0.145, 0.340, 0.455);
const vec3 ICE_RIM   = vec3(0.560, 0.840, 0.920);

const vec3 SNOW_DEEP = vec3(0.195, 0.270, 0.395);
const vec3 SNOW_MID  = vec3(0.255, 0.340, 0.475);
const vec3 SNOW_HI   = vec3(0.365, 0.465, 0.610);
const vec3 SNOW_SH   = vec3(0.145, 0.205, 0.320);

const vec3 AUR_GREEN  = vec3(0.180, 0.940, 0.650);
const vec3 AUR_TEAL   = vec3(0.140, 0.780, 0.860);
const vec3 AUR_VIOLET = vec3(0.620, 0.380, 0.940);

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

vec3 evalAurora(vec2 p) {
    if (uAurora <= 0.01) return vec3(0.0);
    vec2 uv = p / max(min(uRes.x, uRes.y), 600.0);
    float t = uFlowTime * 0.22;

    float n1 = N(p + vec2(t * 28.0, -t * 16.0), 1150.0).r;
    float n2 = N(p * 0.75 - vec2(t * 19.0, t * 22.0), 680.0).g;
    float coord1 = uv.x * 0.68 - uv.y * 0.88 + 0.15 + (n1 - 0.5) * 0.62 + sin(uv.x * 2.1 + t * 0.8) * 0.16;
    float band1 = exp(-coord1 * coord1 * 14.0);
    float ray1 = 0.58 + 0.42 * sin(coord1 * 28.0 + n2 * 7.5 - t * 2.3);
    float curtain1 = band1 * ray1 * (0.45 + 0.75 * N(p + vec2(t * 36.0, 0.0), 820.0).b);

    float coord2 = uv.x * 0.45 + uv.y * 0.72 - 0.82 + (n2 - 0.5) * 0.55 + cos(uv.y * 1.9 - t * 0.65) * 0.14;
    float band2 = exp(-coord2 * coord2 * 18.0);
    float ray2 = 0.55 + 0.45 * sin(coord2 * 24.0 - n1 * 6.0 + t * 1.8);
    float curtain2 = band2 * ray2 * (0.4 + 0.7 * N(p + 320.0 - vec2(0.0, t * 30.0), 740.0).r);

    vec3 c1 = mix(AUR_GREEN, AUR_TEAL, ss(-0.18, 0.22, coord1));
    vec3 c2 = mix(AUR_TEAL, AUR_VIOLET, ss(-0.15, 0.18, coord2));
    return (c1 * curtain1 * 0.78 + c2 * curtain2 * 0.52) * uAurora;
}

void main() {
    vec2 p = vec2(gl_FragCoord.x, uRes.y * uScale - gl_FragCoord.y) / uScale;
    float s = mix(fieldS(uFA, p), fieldS(uFB, p), uMix);

    float grain = (hash2(floor(p * (0.8 * max(uScale, 1.0)))) - 0.5) * 0.032;

    float shoalW = max(uShoal, 8.0);
    float dIce = max(s, 0.0);
    float tMid = ss(0.0, shoalW * 1.5 + 90.0, dIce);
    float tDeep = ss(shoalW * 0.6, shoalW * 2.8 + 210.0, dIce);
    vec3 iceCol = mix(ICE_SHOAL, ICE_MID, tMid);
    iceCol = mix(iceCol, ICE_DEEP, tDeep);

    float fIce = N(p, 950.0).b;
    iceCol *= 0.92 + 0.16 * fIce;
    float crackA = abs(N(p, 520.0).r - 0.5);
    float crackB = abs(N(p + 180.0, 290.0).g - 0.5);
    float crack = (1.0 - ss(0.002, 0.015, crackA)) * 0.45 + (1.0 - ss(0.002, 0.012, crackB)) * 0.28;
    iceCol += vec3(0.18, 0.34, 0.44) * crack * ss(2.0, 55.0, dIce) * 0.7;

    vec2 sc = floor(p / 38.0);
    float sh = hash2(sc + uSeed * 0.11);
    if (sh > 0.87 && dIce > 14.0) {
        vec2 sp = (sc + 0.25 + 0.5 * vec2(hash2(sc + 13.7), hash2(sc + 29.1))) * 38.0;
        float dist = length(p - sp);
        float tw = 0.45 + 0.55 * sin(uTime * (1.4 + 2.2 * fract(sh * 31.0)) + sh * 40.0);
        float star = exp(-dist * dist * 0.55) * tw * ss(14.0, 48.0, dIce);
        iceCol += vec3(0.75, 0.90, 1.0) * star * 0.75;
    }

    vec3 aur = evalAurora(p);
    float aurMask = ss(1.0, 68.0, dIce) * 0.86 + ss(0.0, 20.0, dIce) * 0.14;
    iceCol += aur * aurMask * 0.72;

    for (int i = 0; i < 6; i++) {
        if (i >= uRipN) break;
        vec4 rp = uRip[i];
        float dist = length(p - rp.xy);
        float radius = rp.z * 78.0;
        float ring = exp(-pow(dist - radius, 2.0) * 0.032) * exp(-rp.z * 0.65) * rp.w;
        iceCol += vec3(0.35, 0.88, 0.82) * ring * 0.55;
    }

    float snowN1 = N(p, 1200.0).r;
    float snowN2 = N(p + 230.0, 480.0).g;
    vec3 snowCol = mix(SNOW_DEEP, SNOW_MID, ss(0.32, 0.72, snowN1 * 0.65 + snowN2 * 0.35));

    float ridge = N(p + vec2(-18.0, -24.0), 620.0).g - N(p, 620.0).g;
    snowCol = mix(snowCol, SNOW_HI, clamp(ridge * 3.5 + 0.15, 0.0, 0.55));
    float contour = 0.5 + 0.5 * cos(s * 0.13 + snowN2 * 2.2);
    snowCol *= 0.96 + 0.05 * contour;

    snowCol += aur * ss(-54.0, 0.0, s) * 0.14;
    snowCol = mix(snowCol, SNOW_SH, ss(-18.0, -1.0, s) * 0.38);

    float landMask = ss(1.2, -1.2, s);
    vec3 col = mix(iceCol, snowCol, landMask);
    float rim = exp(-s * s * 0.35);
    col = mix(col, mix(ICE_RIM, vec3(0.82, 0.95, 1.0), 0.35), rim * 0.55);

    col *= 1.0 + grain;
    if (uDebug > 0.5) col = mix(col, vec3(0.25, 0.95, 0.72), 1.0 - ss(0.0, 1.6, abs(s)));
    o = vec4(col, 1.0);
}
"#;

pub const AURORA_SPRITE_VERT: &str = r#"#version 330 core
uniform vec2 uRes;
uniform vec4 uSprite;
uniform float uMirror;
uniform float uTime;
uniform float uSway;
out vec2 spriteUV;
void main() {
    vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
    vec2 local = (corner - 0.5) * vec2(160.0, 160.0);
    local.x *= uMirror;
    float a = sin(uTime * 0.9 + uSprite.w * 1.9 + uSprite.x * 0.01) * 0.035 * uSway;
    local = mat2(cos(a), sin(a), -sin(a), cos(a)) * local;
    vec2 p = uSprite.xy + local * uSprite.z;
    gl_Position = vec4(p.x / uRes.x * 2.0 - 1.0, 1.0 - p.y / uRes.y * 2.0, 0.0, 1.0);
    spriteUV = vec2((corner.x + uSprite.w) / 5.0, corner.y);
}
"#;

pub const AURORA_SPRITE_FRAG: &str = r#"#version 330 core
uniform sampler2D uAtlas;
uniform float uOpacity;
uniform float uPulse;
in vec2 spriteUV;
out vec4 o;
void main() {
    vec4 c = texture(uAtlas, spriteUV);
    if (c.a < 0.004) discard;
    float alpha = c.a * uOpacity * uPulse;
    o = vec4(c.rgb * uPulse, alpha);
}
"#;

pub const AURORA_SNOW_VERT: &str = r#"#version 330 core
uniform vec2 uRes;
uniform float uTime, uScale, uSeed;
out float vAlpha;
float h(float n) { return fract(sin(n * 127.1 + uSeed * 0.13) * 43758.5453); }
void main() {
    float id = float(gl_VertexID);
    float r1 = h(id * 3.1 + 1.0), r2 = h(id * 5.7 + 2.0), r3 = h(id * 7.3 + 3.0), r4 = h(id * 11.9 + 4.0);
    float speedY = 14.0 + r3 * 22.0;
    float driftX = (r4 - 0.35) * 16.0;
    float x = mod(r1 * uRes.x + uTime * driftX + sin(uTime * 0.7 + id) * 18.0, uRes.x);
    float y = mod(r2 * uRes.y + uTime * speedY, uRes.y);
    gl_Position = vec4(x / uRes.x * 2.0 - 1.0, 1.0 - y / uRes.y * 2.0, 0.0, 1.0);
    gl_PointSize = (2.0 + r3 * 2.8) * max(uScale, 0.5);
    vAlpha = 0.25 + 0.45 * r4;
}
"#;

pub const AURORA_SNOW_FRAG: &str = r#"#version 330 core
in float vAlpha;
out vec4 o;
void main() {
    vec2 d = gl_PointCoord - 0.5;
    float r = dot(d, d);
    if (r > 0.25) discard;
    float a = smoothstep(0.25, 0.02, r) * vAlpha;
    o = vec4(0.86, 0.95, 1.0, a);
}
"#;
