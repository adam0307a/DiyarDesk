pub const PN_COLOR_VS: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
layout(location=1) in vec4 aCol;
uniform vec2 uRes;
uniform float uFlipY;
out vec4 vCol;
void main() {
  vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
  gl_Position = vec4(ndc.x, ndc.y * uFlipY, 0.0, 1.0);
  vCol = aCol;
}
"#;

pub const PN_COLOR_FS: &str = r#"#version 330 core
in vec4 vCol;
out vec4 O;
void main() {
  if (vCol.a <= 0.001) discard;
  O = vCol;
}
"#;

pub const PN_TEX_VS: &str = r#"#version 330 core
layout(location=0) in vec2 aPos;
layout(location=1) in vec2 aUV;
layout(location=2) in float aAlpha;
uniform vec2 uRes;
uniform float uFlipY;
out vec2 vUV;
out float vAlpha;
void main() {
  vec2 ndc = (aPos / uRes) * 2.0 - 1.0;
  gl_Position = vec4(ndc.x, ndc.y * uFlipY, 0.0, 1.0);
  vUV = aUV;
  vAlpha = aAlpha;
}
"#;

pub const PN_TEX_FS: &str = r#"#version 330 core
uniform sampler2D uTex;
in vec2 vUV;
in float vAlpha;
out vec4 O;
void main() {
  vec4 t = texture(uTex, vUV);
  if (t.a * vAlpha <= 0.002) discard;
  O = vec4(t.rgb, t.a * vAlpha);
}
"#;
