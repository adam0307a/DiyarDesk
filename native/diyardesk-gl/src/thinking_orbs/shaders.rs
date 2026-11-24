pub const TO_BG_VERT: &str = r#"#version 330 core
out vec2 vUV;
void main() {
    vec2 p = vec2((gl_VertexID == 1) ? 3.0 : -1.0, (gl_VertexID == 2) ? 3.0 : -1.0);
    vUV = p * 0.5 + 0.5;
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;

pub const TO_BG_FRAG: &str = r#"#version 330 core
in vec2 vUV;
uniform vec2 uRes;
uniform vec3 uBgInner;
uniform vec3 uBgMid;
uniform vec3 uBgOuter;
out vec4 O;

void main() {
    vec2 pos = vec2(vUV.x * uRes.x, (1.0 - vUV.y) * uRes.y);
    vec2 c = uRes * 0.5;
    float rad = max(length(c), 1.0);
    float d = length(pos - c);
    float r0 = rad * 0.04;
    float t = clamp((d - r0) / max(rad - r0, 1.0), 0.0, 1.0);
    vec3 col = (t < 0.48)
        ? mix(uBgInner, uBgMid, t / 0.48)
        : mix(uBgMid, uBgOuter, (t - 0.48) / 0.52);
    O = vec4(col, 1.0);
}
"#;

pub const TO_TEX_VERT: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
layout(location=1) in vec2 aUV;
layout(location=2) in float aAlpha;
uniform vec2 uRes;
out vec2 vUV;
out float vAlpha;
void main() {
    vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    vUV = aUV;
    vAlpha = aAlpha;
}
"#;

pub const TO_TEX_FRAG: &str = r#"#version 330 core
uniform sampler2D uTex;
in vec2 vUV;
in float vAlpha;
out vec4 O;
void main() {
    vec4 t = texture(uTex, vUV);
    float a = t.a * vAlpha;
    if (a <= 0.001) discard;
    O = vec4(t.rgb, a);
}
"#;

pub const TO_PRIM_VERT: &str = r#"#version 330 core
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

pub const TO_PRIM_FRAG: &str = r#"#version 330 core
in vec2 vLocal;
in vec4 vCol;
in vec4 vStroke;
in vec4 vParams;
out vec4 O;

void main() {
    float kind = vParams.x;
    if (kind < 0.5) {
        // 0: Smooth anti-aliased disc (orb dots, glow dots, telemetry dots)
        float d = length(vLocal);
        float aa = max(fwidth(d), 0.02);
        float cov = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, d);
        float a = vCol.a * cov;
        if (a <= 0.001) discard;
        O = vec4(vCol.rgb, a);
    } else if (kind < 1.5) {
        // 1: Rounded-cap capsule line segment
        // vLocal in pixel coords relative to segment center: x along segment [-halfLen-pad, halfLen+pad], y perp
        // vParams.y = halfLen, vParams.z = halfWidth
        float halfLen = vParams.y;
        float halfW = vParams.z;
        float dx = max(abs(vLocal.x) - halfLen, 0.0);
        float d = length(vec2(dx, vLocal.y)) - halfW;
        float aa = max(fwidth(d), 0.65);
        float cov = 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, d);
        float a = vCol.a * cov;
        if (a <= 0.001) discard;
        O = vec4(vCol.rgb, a);
    } else {
        // 2: Adaptive icon rounded-rect cradle (fill + 1px stroke)
        // vLocal in pixel coords relative to rect center
        // vParams.y = halfW, vParams.z = halfH, vParams.w = cornerRadius
        vec2 halfSize = vec2(vParams.y, vParams.z);
        float r = min(vParams.w, min(halfSize.x, halfSize.y));
        vec2 q = abs(vLocal) - (halfSize - vec2(r));
        float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
        float aa = max(fwidth(d), 0.75);
        float fillCov = 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, d);
        float strokeCov = 1.0 - smoothstep(0.5 - 0.5 * aa, 0.5 + 0.5 * aa, abs(d));

        float fa = vCol.a * fillCov;
        float sa = vStroke.a * strokeCov;
        float outA = sa + fa * (1.0 - sa);
        if (outA <= 0.001) discard;
        vec3 outPm = vStroke.rgb * sa + vCol.rgb * fa * (1.0 - sa);
        O = vec4(outPm / outA, outA);
    }
}
"#;
