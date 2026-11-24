// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Meadow (Çayır)
// Ported 1:1 from prototype/meadow.html, prototype/meadow-boy.js, and prototype/meadow-cat.js

pub const MEADOW_VERT: &str = r#"#version 330 core
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const MEADOW_FRAG_BODY: &str = r#"
uniform sampler2D uNoise, uFA, uFB;
uniform vec2 uRes, uGrid;
uniform float uScale, uSeed, uStep, uMix, uTime, uDebug, uEdge;
#ifdef MEADOW_ACTOR
uniform sampler2D uActor, uActorShadow;
uniform vec4 uActorRect;
#endif
#ifdef MEADOW_CAT
uniform sampler2D uCat;
uniform vec4 uCatRect;
uniform vec2 uCatShadow;
uniform float uCatTail;
#endif
uniform sampler2D uCirc;
uniform vec2 uCGrid;
uniform float uCellG;
out vec4 o;

const vec3 G_LIGHT = vec3(0.76, 0.83, 0.52);
const vec3 G_MID = vec3(0.62, 0.75, 0.42);
const vec3 G_DEEP = vec3(0.52, 0.68, 0.36);
const vec3 INK = vec3(0.24, 0.38, 0.18);
const vec3 SHADOW = vec3(0.56, 0.66, 0.52);
const vec3 CLOUD_HI = vec3(1.0, 1.0, 0.99);
const vec3 CLOUD_BODY = vec3(0.90, 0.93, 0.96);
const vec3 CLOUD_SH = vec3(0.80, 0.85, 0.90);
const vec2 LIGHT = vec2(-0.6, -0.8);
const vec2 SHADOW_OFF = vec2(115.0, 156.0);
const int CIRC_K = 36;

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
float smin(float a, float b, float k) {
    float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

float cloudDist(vec2 q, bool front, inout vec4 fc) {
    float sF = mix(fieldS(uFA, q), fieldS(uFB, q), uMix);
    float d = sF - uEdge + 0.6 * 95.0;
    ivec2 cell = ivec2(floor(q / uCellG)) + 4;
    if (cell.x < 0 || cell.y < 0 || float(cell.x) >= uCGrid.x || float(cell.y) >= uCGrid.y) return d;
    for (int k = 0; k < CIRC_K; k++) {
        vec4 c = texelFetch(uCirc, ivec2(cell.x * CIRC_K + k, cell.y), 0);
        if (c.z <= 0.0) break;
        float di = length(q - c.xy) - c.z;
        d = smin(d, di, 6.0);
        if (front && di < 0.0 && c.y > fc.y) fc = c;
    }
    return d;
}
float cloudDist(vec2 q) {
    vec4 fc = vec4(0.0, -1e9, 0.0, 0.0);
    return cloudDist(q, false, fc);
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
        float dens = 0.75 * ss(0.42, 0.72, N(cp, 900.0).r) * ss(0.35, 0.6, N(cp + 50.0, 380.0).g) + 0.02;
        if (h > dens) continue;
        float ang = (N(cp, 1400.0).g - 0.5) * 0.7 + (hash2(c + 7.7) - 0.5) * 0.3;
        float len = 2.5 + 3.0 * hash2(c + 1.9);
        vec2 ctr = cp + (vec2(hash2(c + 4.3), hash2(c + 5.9)) - 0.5) * vec2(2.0, 3.0);
        vec2 dir = vec2(cos(ang), sin(ang)) * len;
        float d = segD(p, ctr - dir, ctr + dir);
        ink = max(ink, (1.0 - ss(0.2, 0.9, d)) * (0.35 + 0.45 * hash2(c + 9.1)));
    }
    return ink;
}

#ifdef MEADOW_CAT
float catTailMask(vec2 uv) {
    return smoothstep(0.635, 0.665, uv.x) * smoothstep(0.715, 0.75, uv.y);
}
vec4 catColor(vec2 uv) {
    vec2 pivot = vec2(0.68, 0.73), delta = uv - pivot;
    float angle = -uCatTail * smoothstep(0.025, 0.16, length(delta));
    float c = cos(angle), s = sin(angle);
    vec2 source = pivot + vec2(c * delta.x - s * delta.y, s * delta.x + c * delta.y);
    vec4 body = texture(uCat, uv), tail = texture(uCat, source);
    body.a *= 1.0 - catTailMask(uv);
    tail.a *= catTailMask(source);
    float alpha = body.a + tail.a;
    return vec4((body.rgb * body.a + tail.rgb * tail.a) / max(alpha, 0.001), min(alpha, 1.0));
}
#endif

void main() {
    vec2 p = vec2(gl_FragCoord.x, uRes.y * uScale - gl_FragCoord.y) / uScale;

    float w1 = N(p, 1900.0).b, w2 = N(p + 311.0, 650.0).g;
    vec3 col = mix(G_MID, G_LIGHT, ss(0.3, 0.75, w1 * 0.7 + w2 * 0.3));
    col = mix(col, G_DEEP, ss(0.45, 0.85, N(p + 977.0, 1300.0).r) * 0.4);
    col *= 1.0 + 0.05 * (0.5 - (p.x + p.y) / (uRes.x + uRes.y));
    col *= 1.0 + (hash2(floor(p * (0.8 * max(uScale, 1.0)))) - 0.5) * 0.035;
    col = mix(col, INK, grassInk(p) * 0.75);

#ifdef MEADOW_ACTOR
    vec2 actorUV = (p - uActorRect.xy) / uActorRect.zw;
    actorUV.y = 1.0 - actorUV.y;
    float inActor = step(0.0, actorUV.x) * step(0.0, actorUV.y) * step(actorUV.x, 1.0) * step(actorUV.y, 1.0);
    vec4 actor = texture(uActor, actorUV);
    float actorShadow = texture(uActorShadow, actorUV).r * inActor;
    col = mix(col, col * SHADOW, actorShadow * 0.85);
    col = mix(col, actor.rgb, actor.a * inActor);
#endif

#ifdef MEADOW_CAT
    vec2 catUV = (p - uCatRect.xy) / uCatRect.zw;
    vec2 catShadowUV = (p - uCatShadow - uCatRect.xy) / uCatRect.zw;
    vec4 cat = catColor(catUV);
    float catShadow = catColor(catShadowUV).a;
    float inCat = step(0.0, catUV.x) * step(0.0, catUV.y) * step(catUV.x, 1.0) * step(catUV.y, 1.0);
    float inShadow = step(0.0, catShadowUV.x) * step(0.0, catShadowUV.y) * step(catShadowUV.x, 1.0) * step(catShadowUV.y, 1.0);
    col = mix(col, col * SHADOW, catShadow * inShadow * 0.78);
    col = mix(col, cat.rgb, cat.a * inCat);
#endif

    float sd = cloudDist(p - SHADOW_OFF);
    col = mix(col, col * SHADOW, ss(3.0, -3.0, sd) * 0.85);

    vec4 fc = vec4(0.0, -1e9, 0.0, 0.0);
    float d = cloudDist(p, true, fc);
    if (d < 1.5) {
        float hi = d > -21.0 ? ss(-1.0, 1.0, cloudDist(p + LIGHT * 20.0)) : 0.0;
        vec3 cc = fc.y > -1e8 ? mix(CLOUD_BODY, CLOUD_SH, fc.w * 0.6) : mix(CLOUD_BODY, CLOUD_SH, 0.45);
        cc = mix(cc, CLOUD_HI, hi);
        col = mix(col, cc, ss(1.0, -1.0, d));
    }
    if (uDebug > 0.5) col = mix(col, vec3(0.9, 0.15, 0.15), 1.0 - ss(0.0, 1.5, abs(d)));
    o = vec4(col, 1.0);
}
"#;

pub fn meadow_frag_source(with_actor: bool, with_cat: bool) -> String {
    let mut s = String::from("#version 330 core\n");
    if with_actor {
        s.push_str("#define MEADOW_ACTOR\n");
    }
    if with_cat {
        s.push_str("#define MEADOW_CAT\n");
    }
    s.push_str(MEADOW_FRAG_BODY);
    s
}

pub const MEADOW_BOY_FRAG: &str = r#"#version 330 core
uniform sampler2D uCharacter, uRunSprite;
uniform vec2 uRes, uOffset, uKite, uBody, uLegs, uHand, uReelHand, uTug;
uniform vec4 uPose, uMotion;
uniform vec4 uLeftHipKnee, uRightHipKnee, uRig;
uniform vec2 uLeftAnkle, uRightAnkle, uFootAngles;
uniform vec2 uGround;
uniform vec3 uContactL, uContactR;
uniform vec3 uPlace;
uniform float uScale, uTime, uLook, uShadow, uRunWind, uTension, uKiteRock;
out vec4 o;

vec4 over(vec4 base, vec4 top) {
    float alpha = top.a + base.a * (1.0 - top.a);
    return vec4((top.rgb * top.a + base.rgb * base.a * (1.0 - top.a)) / max(alpha, 0.001), alpha);
}
float segment(vec2 p, vec2 a, vec2 b) {
    vec2 v = b - a;
    return length(p - a - v * clamp(dot(p - a, v) / max(dot(v, v), 0.001), 0.0, 1.0));
}
vec2 turn(vec2 p, float angle) {
    float c = cos(angle), s = sin(angle);
    return vec2(c * p.x - s * p.y, s * p.x + c * p.y);
}
vec2 bodySpace(vec2 p) {
    return p - uBody * clamp((78.0 - p.y) / 170.0, 0.0, 1.0);
}
float armMask(vec2 p) {
    float boundary = p.y < -30.0 ? -38.0 + (-p.y - 30.0) * 0.10 : min(-17.0, -38.0 + (p.y + 30.0) * 0.78);
    return (1.0 - smoothstep(boundary - 0.45, boundary + 0.45, p.x)) * (1.0 - smoothstep(0.0, 3.0, p.y));
}
vec4 spriteColor(vec4 color, vec2 p) {
    color.a *= step(-76.0, p.x) * step(-95.0, p.y) * step(p.x, 76.0) * step(p.y, 85.0);
    return color;
}
vec2 spriteUV(vec2 p) {
    return clamp((p + vec2(76.0, 95.0)) / vec2(152.0, 180.0), 0.002, 0.998);
}
vec2 personSource(vec2 p) {
    vec2 boy = bodySpace(p);
    vec2 rest = boy;
    float lower = smoothstep(24.0, 48.0, rest.y);
    float right = smoothstep(-4.0, 9.0, rest.x);
    float lift = mix(uLegs.x, uLegs.y, right);
    boy.y += lift * lower;
    boy.x -= (right * 2.0 - 1.0) * lift * 0.24 * lower;
    return boy;
}
float ribbon(vec2 p, float offset, float phase) {
    float t = clamp(p.y / 108.0, 0.0, 1.0);
    float gust = sin(t * 5.6 - uTime * 4.8 + phase) * uRunWind * 9.0
               + sin(t * 6.0 - uTime * 7.0 + phase) * abs(uKiteRock) * 12.0;
    float center = offset + 50.0 * t + (12.0 * sin(t * 4.8 - uTime * 1.15 + phase) + gust) * t;
    float width = mix(3.4, 1.6, t);
    return (1.0 - smoothstep(width, width + 1.0, abs(p.x - center))) * step(0.0, p.y) * step(p.y, 108.0);
}
float kiteAngle() {
    return -0.23 + sin(uTime * 0.7) * 0.09 + sin(uTime * 4.8 + 0.7) * uRunWind * 0.24 + uKiteRock;
}
vec4 kiteColor(vec2 p, vec2 screen) {
    vec4 result = vec4(0.0);
    float angle = kiteAngle();
    vec2 delta = p - uKite;
    vec2 q = vec2(cos(angle) * delta.x + sin(angle) * delta.y, -sin(angle) * delta.x + cos(angle) * delta.y);
    vec2 tail = q - vec2(2.0, 65.0);
    result = over(result, vec4(0.88, 0.65, 0.37, ribbon(tail, 0.0, 0.0)));
    result = over(result, vec4(0.81, 0.39, 0.30, ribbon(tail, 8.0, 0.45)));
    result = over(result, vec4(0.32, 0.55, 0.62, ribbon(tail, 16.0, 0.9)));
    float diamond = abs(q.x) / 53.0 + abs(q.y) / 70.0;
    float coverage = 1.0 - smoothstep(mix(0.97, 0.94, uShadow), mix(1.025, 1.07, uShadow), diamond);
    vec3 fabric = q.y < 0.0
        ? (q.x < 0.0 ? vec3(0.95, 0.77, 0.42) : vec3(0.88, 0.43, 0.33))
        : (q.x < 0.0 ? vec3(0.94, 0.77, 0.44) : vec3(0.35, 0.57, 0.64));
    float grain = fract(sin(dot(floor(screen * 1.7), vec2(12.9898, 78.233))) * 43758.5453);
    fabric *= 0.97 + grain * 0.06;
    fabric = mix(fabric, vec3(0.46, 0.40, 0.25), smoothstep(0.93, 1.0, diamond) * 0.78);
    float seams = 1.0 - smoothstep(0.35, 0.9, min(abs(q.x), abs(q.y)));
    fabric = mix(fabric, vec3(0.98, 0.92, 0.71), seams * 0.65);
    return over(result, vec4(fabric, coverage));
}
vec4 standingBoy(vec2 p, float lod) {
    return spriteColor(textureLod(uRunSprite, (spriteUV(p) + vec2(1.0)) * 0.5, lod), p);
}
vec2 boneSource(vec2 p, vec2 sourceA, vec2 sourceB, vec2 posedA, vec2 posedB) {
    vec2 v = posedB - posedA, s = sourceB - sourceA;
    float lengthV = max(length(v), 0.001), lengthS = max(length(s), 0.001);
    vec2 axis = v / lengthV, normal = vec2(-axis.y, axis.x), sourceAxis = s / lengthS;
    return sourceA + s * (dot(p - posedA, axis) / lengthV) + vec2(-sourceAxis.y, sourceAxis.x) * dot(p - posedA, normal);
}
float rigMask(vec2 p, float side, float y0, float y1) {
    float xMask = mix(1.0 - smoothstep(-2.0, 1.0, p.x), smoothstep(2.0, 5.0, p.x), step(0.0, side));
    return xMask * smoothstep(y0 - 0.6, y0 + 0.6, p.y) * (1.0 - smoothstep(y1 - 0.6, y1 + 0.6, p.y));
}
vec4 rigPiece(vec2 source, float side, float y0, float y1, float lod) {
    vec4 color = standingBoy(source, lod);
    color.a *= rigMask(source, side, y0, y1);
    return color;
}
vec4 rigJoint(vec2 p, vec2 center, vec3 color, float radius) {
    float d = length(p - center);
    vec4 edge = vec4(0.26, 0.18, 0.12, 1.0 - smoothstep(radius + 0.2, radius + 0.8, d));
    return over(edge, vec4(color, 1.0 - smoothstep(radius - 0.6, radius, d)));
}
float runnerArmMask(vec2 p) {
    return (1.0 - smoothstep(-25.0, -21.0, p.x)) * smoothstep(-8.0, -3.0, p.y) * (1.0 - smoothstep(29.0, 31.0, p.y));
}
float runnerReelArmMask(vec2 p) {
    float boundary = p.y < -50.0 ? 32.0
        : p.y < -43.0 ? mix(31.0, 34.0, (p.y + 50.0) / 7.0)
        : p.y < -34.0 ? mix(34.0, 28.0, (p.y + 43.0) / 9.0)
        : mix(28.0, 23.0, clamp((p.y + 34.0) / 14.0, 0.0, 1.0));
    return smoothstep(boundary - 0.5, boundary + 0.5, p.x) * (1.0 - smoothstep(2.0, 6.0, p.y));
}
vec4 runningBoy(vec2 p, vec4 trunk, float lod) {
    vec2 lh = vec2(-12.0, 37.0), lk = vec2(-14.0, 47.0), la = vec2(-15.0, 57.0);
    vec2 rh = vec2(14.0, 35.0), rk = vec2(18.0, 44.0), ra = vec2(21.0, 51.0);
    vec2 leftThigh = boneSource(p, lh, lk, uLeftHipKnee.xy, uLeftHipKnee.zw);
    vec2 leftShin = boneSource(p, lk, la, uLeftHipKnee.zw, uLeftAnkle);
    vec2 leftFoot = la + turn(p - uLeftAnkle, -uFootAngles.x);
    vec2 rightThigh = boneSource(p, rh, rk, uRightHipKnee.xy, uRightHipKnee.zw);
    vec2 rightShin = boneSource(p, rk, ra, uRightHipKnee.zw, uRightAnkle);
    vec2 rightFoot = ra + turn(p - uRightAnkle, -uFootAngles.y);
    vec4 left = rigJoint(p, uLeftHipKnee.zw, vec3(0.98, 0.76, 0.47), 3.5);
    left = over(left, rigJoint(p, uLeftAnkle, vec3(0.97, 0.96, 0.88), 2.8));
    left = over(left, rigPiece(leftThigh, -1.0, 35.5, 49.0, lod));
    left = over(left, rigPiece(leftShin, -1.0, 47.0, 58.5, lod));
    left = over(left, rigPiece(leftFoot, -1.0, 56.5, 74.0, lod));
    vec4 right = rigJoint(p, uRightHipKnee.zw, vec3(0.98, 0.76, 0.47), 3.5);
    right = over(right, rigJoint(p, uRightAnkle, vec3(0.97, 0.96, 0.88), 2.8));
    right = over(right, rigPiece(rightThigh, 1.0, 34.5, 46.0, lod));
    right = over(right, rigPiece(rightShin, 1.0, 44.0, 53.0, lod));
    right = over(right, rigPiece(rightFoot, 1.0, 51.5, 68.0, lod));
    float cut = max(rigMask(p, -1.0, 37.0, 86.0), rigMask(p, 1.0, 36.0, 86.0));
    trunk.a *= (1.0 - cut) * (1.0 - runnerArmMask(p)) * (1.0 - runnerReelArmMask(p));
    vec2 armSource = vec2(-23.0, -5.0) + turn(p - vec2(-23.0, -5.0), -uRig.y);
    vec4 arm = standingBoy(armSource, lod);
    arm.a *= runnerArmMask(armSource);
    vec2 reelPivot = vec2(22.0, -7.0);
    vec2 reelSource = reelPivot + turn(p - reelPivot, -uRig.z);
    vec4 reelArm = standingBoy(reelSource, lod);
    reelArm.a *= runnerReelArmMask(reelSource);
    vec4 legs = mix(over(left, right), over(right, left), step(uRightAnkle.y, uLeftAnkle.y));
    legs = over(legs, rigJoint(p, vec2(-23.0, -5.0), vec3(0.87, 0.24, 0.18), 4.5));
    legs = over(legs, rigJoint(p, reelPivot, vec3(0.87, 0.24, 0.18), 5.0));
    return over(over(over(legs, trunk), arm), reelArm);
}
vec4 characterColor(vec2 p) {
    p.x /= uMotion.z;
    vec2 boy = personSource(p), space = bodySpace(p);
    vec2 pivot = vec2(-21.0, -9.0);
    vec2 armSource = pivot + turn(space + uTug * 0.35 - pivot, -(uPose.z + 0.18));
    float lod = uShadow * 2.8;
    vec4 person = spriteColor(textureLod(uCharacter, spriteUV(boy), lod), boy);
    vec4 arm = spriteColor(textureLod(uCharacter, spriteUV(armSource), lod), armSource);
    person.a *= 1.0 - armMask(boy);
    arm.a *= armMask(armSource);
    vec4 shoulder = rigJoint(space, pivot, vec3(0.90, 0.22, 0.16), 5.0);
    vec4 front = over(over(person, shoulder), arm);
    vec2 runPoint = p - vec2(0.0, uMotion.w - 10.0);
    vec2 cell = vec2(mod(uMotion.x, 2.0), floor(uMotion.x * 0.5));
    vec4 back = spriteColor(textureLod(uRunSprite, (spriteUV(runPoint) + cell) * 0.5, lod), runPoint);
    back = mix(back, runningBoy(runPoint, back, lod), uRig.x);
    return mix(front, back, uMotion.y);
}
void main() {
    vec2 screen = uOffset + vec2(gl_FragCoord.x / uScale, uRes.y - gl_FragCoord.y / uScale);
    vec2 p = (screen - uPlace.xy) / uPlace.z;
    float sy = (p.y - uGround.x * p.x - uGround.y) / (uGround.x * 0.46 - 0.64);
    vec2 projected = vec2(p.x + 0.46 * sy, uGround.x * (p.x + 0.46 * sy) + uGround.y + sy);
    vec2 personPoint = mix(p, projected, uShadow);
    vec4 person = characterColor(personPoint);
    vec4 kite = kiteColor(p - uShadow * vec2(98.0, 134.0), screen);
    vec2 hand = uHand, attach = uKite + turn(vec2(-41.0, -16.0), kiteAngle());
    vec2 control = mix(hand, attach, 0.5) + vec2(-13.0 + sin(uTime * 0.8) * 6.0, 18.0) * (1.0 - uTension * 0.85);
    float line = 1e5;
    vec2 previous = hand;
    for (int i = 1; i <= 6; i++) {
        float t = float(i) / 6.0;
        vec2 point = mix(mix(hand, control, t), mix(control, attach, t), t);
        line = min(line, segment(p, previous, point));
        previous = point;
    }
    vec4 result = over(kite, person);
    result = over(result, vec4(0.94, 0.92, 0.77, (1.0 - smoothstep(0.4, 1.1, line)) * 0.94));
    float contactL = (1.0 - smoothstep(0.65, 1.15, length((p - uContactL.xy) / vec2(7.0, 2.6)))) * uContactL.z;
    float contactR = (1.0 - smoothstep(0.65, 1.15, length((p - uContactR.xy) / vec2(7.0, 2.6)))) * uContactR.z;
    float silhouette = max(max(person.a, kite.a), max(contactL, contactR));
    o = mix(result, vec4(vec3(silhouette), 1.0), uShadow);
}
"#;
