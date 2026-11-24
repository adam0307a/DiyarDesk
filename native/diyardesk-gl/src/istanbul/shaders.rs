pub const IST_TEX_VS: &str = r#"#version 330 core
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

pub const IST_TEX_FS: &str = r#"#version 330 core
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

pub const IST_WATER_VS: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
layout(location=1) in vec2 aUV;
uniform vec2 uRes;
out vec2 vPos;
out vec2 vMaskUV;
void main() {
    vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    vPos = aPos;
    vMaskUV = aUV;
}
"#;

pub const IST_WATER_FS: &str = r#"#version 330 core
uniform sampler2D uMaskTex;
uniform sampler2D uWaveTex;
uniform vec2 uOffset0;
uniform vec2 uOffset1;
uniform float uAlpha0;
uniform float uAlpha1;
in vec2 vPos;
in vec2 vMaskUV;
out vec4 O;
void main() {
    float mask = texture(uMaskTex, vMaskUV).r;
    if (mask <= 0.001) discard;
    vec4 w0 = texture(uWaveTex, (vPos - uOffset0) / 256.0);
    vec4 w1 = texture(uWaveTex, (vPos - uOffset1) / 256.0);
    float a0 = w0.a * uAlpha0;
    float a1 = w1.a * uAlpha1;
    float outA = a0 * (1.0 - a1) + a1;
    if (outA <= 0.001) discard;
    vec3 outPm = w0.rgb * a0 * (1.0 - a1) + w1.rgb * a1;
    O = vec4(outPm / outA, outA * mask);
}
"#;

pub const IST_COLOR_VS: &str = r#"#version 330 core
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

pub const IST_COLOR_FS: &str = r#"#version 330 core
in vec4 vCol;
out vec4 O;
void main() {
    if (vCol.a <= 0.001) discard;
    O = vCol;
}
"#;

pub const IST_PATTERN_RECT_VS: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
uniform vec2 uRes;
out vec2 vPos;
void main() {
    vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    vPos = aPos;
}
"#;

pub const IST_PATTERN_RECT_FS: &str = r#"#version 330 core
uniform sampler2D uPatTex;
uniform int uUseTex;
uniform vec2 uTileSize;
uniform vec4 uRect;
uniform float uRadius;
uniform float uAlpha;
uniform vec4 uFillCol;
uniform float uStrokeWidth;
uniform vec4 uStrokeCol;
in vec2 vPos;
out vec4 O;
void main() {
    vec2 halfSize = max(uRect.zw * 0.5, vec2(0.001));
    vec2 center = uRect.xy + halfSize;
    float r = min(uRadius, min(halfSize.x, halfSize.y));
    vec2 q = abs(vPos - center) - (halfSize - vec2(r));
    float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
    float aa = max(fwidth(d), 0.75);
    float fillCov = 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, d);
    vec4 src = (uUseTex != 0) ? texture(uPatTex, vPos / uTileSize) : uFillCol;
    vec3 col = src.rgb;
    float cov = fillCov * src.a;
    if (uStrokeWidth > 0.0 && uStrokeCol.a > 0.001) {
        float hw = uStrokeWidth * 0.5;
        float sCov = 1.0 - smoothstep(hw - 0.5 * aa, hw + 0.5 * aa, abs(d));
        float sa = uStrokeCol.a * sCov;
        float newCov = sa + cov * (1.0 - sa);
        if (newCov > 0.0001) {
            col = (uStrokeCol.rgb * sa + col * cov * (1.0 - sa)) / newCov;
        }
        cov = newCov;
    }
    float outA = cov * uAlpha;
    if (outA <= 0.001) discard;
    O = vec4(col, outA);
}
"#;

pub const IST_POST_VS: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
uniform vec2 uRes;
out vec2 vPos;
void main() {
    vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    vPos = aPos;
}
"#;

pub const IST_POST_FS: &str = r#"#version 330 core
uniform int uMode; // 0 = tint, 1 = screen radial sun/moon, 2 = radial vignette
uniform vec4 uColor;
uniform vec2 uCenter;
uniform vec2 uRadii; // (innerR, outerR)
in vec2 vPos;
out vec4 O;
void main() {
    if (uMode == 0) {
        if (uColor.a <= 0.001) discard;
        O = uColor;
        return;
    }
    float d = length(vPos - uCenter);
    float t = clamp((d - uRadii.x) / max(uRadii.y - uRadii.x, 1e-4), 0.0, 1.0);
    if (uMode == 1) {
        float a = uColor.a * (1.0 - t);
        O = vec4(uColor.rgb * a, 1.0);
        return;
    }
    float a = uColor.a * t;
    if (a <= 0.001) discard;
    O = vec4(uColor.rgb, a);
}
"#;
