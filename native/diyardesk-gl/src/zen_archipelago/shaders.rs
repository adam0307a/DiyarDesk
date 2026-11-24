pub const ZEN_COLOR_VS: &str = r#"#version 330 core
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

pub const ZEN_COLOR_FS: &str = r#"#version 330 core
in vec4 vCol;
out vec4 O;
void main() {
    if (vCol.a <= 0.001) discard;
    O = vCol;
}
"#;

pub const ZEN_TEX_VS: &str = r#"#version 330 core
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

pub const ZEN_TEX_FS: &str = r#"#version 330 core
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

pub const ZEN_KOI_VS: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
uniform vec2 uRes;
out vec2 vPos;
void main() {
    vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
    vPos = aPos;
}
"#;

pub const ZEN_KOI_FS: &str = r#"#version 330 core
in vec2 vPos;
uniform vec3 uBaseCol;
uniform vec3 uSpotCol0;
uniform vec3 uSpotCol1;
uniform vec3 uSpot0; // (x, y, radius); radius <= 0 disables head spot
uniform vec3 uSpot1; // (x, y, radius)
uniform vec3 uSpot2; // (x, y, radius)
out vec4 O;
void main() {
    vec3 col = uBaseCol;
    if (uSpot0.z > 0.0) {
        float d0 = length(vPos - uSpot0.xy);
        float a0 = 1.0 - smoothstep(uSpot0.z - 0.65, uSpot0.z + 0.65, d0);
        col = mix(col, uSpotCol0, a0);
    }
    if (uSpot1.z > 0.0) {
        float d1 = length(vPos - uSpot1.xy);
        float a1 = 1.0 - smoothstep(uSpot1.z - 0.65, uSpot1.z + 0.65, d1);
        col = mix(col, uSpotCol0, a1);
    }
    if (uSpot2.z > 0.0) {
        float d2 = length(vPos - uSpot2.xy);
        float a2 = 1.0 - smoothstep(uSpot2.z - 0.65, uSpot2.z + 0.65, d2);
        col = mix(col, uSpotCol1, a2);
    }
    O = vec4(col, 1.0);
}
"#;
