pub const SS_BG_VERT: &str = r#"#version 330 core
out vec2 vUV;
void main() {
    vec2 p = vec2((gl_VertexID == 1) ? 3.0 : -1.0, (gl_VertexID == 2) ? 3.0 : -1.0);
    vUV = p * 0.5 + 0.5;
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;

pub const SS_BG_FRAG: &str = r#"#version 330 core
in vec2 vUV;
uniform vec2 uRes;
uniform float uNebula;
out vec4 O;

void main() {
    vec2 pos = vec2(vUV.x * uRes.x, (1.0 - vUV.y) * uRes.y);
    float diag = length(uRes);
    vec2 center = uRes * 0.5;
    float d = length(pos - center);
    float r0 = diag * 0.04;
    float r1 = max(diag * 0.75, r0 + 1.0);
    float t = clamp((d - r0) / (r1 - r0), 0.0, 1.0);

    vec3 c0 = vec3(10.0, 15.0, 29.0) / 255.0;  // #0a0f1d
    vec3 c1 = vec3(6.0, 9.0, 19.0) / 255.0;    // #060913
    vec3 c2 = vec3(4.0, 6.0, 13.0) / 255.0;    // #04060d
    vec3 c3 = vec3(2.0, 3.0, 7.0) / 255.0;     // #020307

    vec3 col;
    if (t < 0.45) {
        col = mix(c0, c1, t / 0.45);
    } else if (t < 0.80) {
        col = mix(c1, c2, (t - 0.45) / 0.35);
    } else {
        col = mix(c2, c3, (t - 0.80) / 0.20);
    }

    float intensity = clamp(uNebula, 0.0, 1.5);
    if (intensity > 0.01) {
        float maxDim = max(uRes.x, uRes.y);
        vec4 clouds[4];
        clouds[0] = vec4(0.26, 0.32, 0.42, 0.14);
        clouds[1] = vec4(0.74, 0.66, 0.40, 0.12);
        clouds[2] = vec4(0.58, 0.25, 0.33, 0.09);
        clouds[3] = vec4(0.35, 0.75, 0.36, 0.10);

        vec3 cloudRgb[4];
        cloudRgb[0] = vec3(24.0, 42.0, 78.0) / 255.0;
        cloudRgb[1] = vec3(38.0, 26.0, 58.0) / 255.0;
        cloudRgb[2] = vec3(18.0, 46.0, 68.0) / 255.0;
        cloudRgb[3] = vec3(28.0, 34.0, 64.0) / 255.0;

        for (int i = 0; i < 4; i++) {
            vec2 cc = vec2(clouds[i].x * uRes.x, clouds[i].y * uRes.y);
            float rad = max(clouds[i].z * maxDim, 1.0);
            float ct = clamp(length(pos - cc) / rad, 0.0, 1.0);
            float baseA = clamp(clouds[i].w * intensity, 0.0, 0.28);
            float a = (ct < 0.5)
                ? mix(baseA, baseA * 0.42, ct / 0.5)
                : mix(baseA * 0.42, 0.0, (ct - 0.5) / 0.5);
            col = mix(col, cloudRgb[i], a);
        }
    }

    O = vec4(col, 1.0);
}
"#;

pub const SS_TEX_VERT: &str = r#"#version 330 core
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

pub const SS_TEX_FRAG: &str = r#"#version 330 core
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

pub const SS_COLOR_VERT: &str = r#"#version 330 core
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

pub const SS_COLOR_FS: &str = r#"#version 330 core
in vec4 vCol;
out vec4 O;
void main() {
    if (vCol.a <= 0.001) discard;
    O = vCol;
}
"#;

pub const SS_DISC_VERT: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
layout(location=1) in vec2 aLocal;
layout(location=2) in vec4 aCol;
layout(location=3) in vec4 aCol2;
layout(location=4) in vec4 aCol3;
layout(location=5) in vec4 aParams;
uniform vec2 uRes;
out vec2 vLocal;
out vec4 vCol;
out vec4 vCol2;
out vec4 vCol3;
out vec4 vParams;
void main() {
    vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    vLocal = aLocal;
    vCol = aCol;
    vCol2 = aCol2;
    vCol3 = aCol3;
    vParams = aParams;
}
"#;

pub const SS_DISC_FRAG: &str = r#"#version 330 core
in vec2 vLocal;
in vec4 vCol;
in vec4 vCol2;
in vec4 vCol3;
in vec4 vParams;
out vec4 O;

void main() {
    float kind = vParams.x;
    float d = length(vLocal);
    float aa = max(fwidth(d), 0.015);

    if (kind < 0.5) {
        // 0: Smooth anti-aliased disc
        float cov = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, d);
        float a = vCol.a * cov;
        if (a <= 0.001) discard;
        O = vec4(vCol.rgb, a);
    } else if (kind < 1.5) {
        // 1: Smooth ring stroke; vParams.y = inner radius norm, vParams.z = outer radius norm
        float rIn = vParams.y;
        float rOut = vParams.z;
        float cov = smoothstep(rIn - aa, rIn + aa, d) * (1.0 - smoothstep(rOut - aa, rOut + aa, d));
        float a = vCol.a * cov;
        if (a <= 0.001) discard;
        O = vec4(vCol.rgb, a);
    } else if (kind < 2.5) {
        // 2: Shaded planet (fill + star-facing highlight + opposite shadow, clipped to disc)
        // vParams.yz = unit direction toward star
        float discCov = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, d);
        if (discCov <= 0.001) discard;
        vec2 dir = vParams.yz;
        vec3 col = vCol.rgb;
        float dHi = length(vLocal - dir * 0.32);
        float hiCov = 1.0 - smoothstep(0.62 - aa, 0.62 + aa, dHi);
        col = mix(col, vCol2.rgb, hiCov);
        float dSh = length(vLocal + dir * 0.36);
        float shCov = (1.0 - smoothstep(0.72 - aa, 0.72 + aa, dSh)) * 0.58;
        col = mix(col, vCol3.rgb, shCov);
        float a = vCol.a * discCov;
        if (a <= 0.001) discard;
        O = vec4(col, a);
    } else {
        // 3: Radial 3-stop glow (pointer halo)
        if (d >= 1.0) discard;
        vec4 c;
        if (d < 0.55) {
            c = mix(vCol, vCol2, d / 0.55);
        } else {
            c = mix(vCol2, vec4(vCol2.rgb, 0.0), (d - 0.55) / 0.45);
        }
        if (c.a <= 0.001) discard;
        O = c;
    }
}
"#;
