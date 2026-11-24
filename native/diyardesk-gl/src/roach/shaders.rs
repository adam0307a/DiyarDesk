pub const ROACH_BG_VS: &str = r#"#version 330 core
out vec2 vUV;
void main() {
  vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2);
  vUV = vec2(p.x, 1.0 - p.y);
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const ROACH_BG_FS: &str = r#"#version 330 core
uniform sampler2D uNoise;
uniform usampler2D uFA;
uniform usampler2D uFB;
uniform vec2 uRes, uGrid;
uniform float uScale, uSeed, uStep, uMix, uTime;
uniform float uGlow, uVibe, uDebug;
in vec2 vUV;
out vec4 O;

float fld(vec2 px) {
  vec2 g = clamp(px / uStep - 0.5, vec2(0.0), uGrid - 1.0);
  vec2 f = fract(g);
  ivec2 i0 = ivec2(floor(g));
  ivec2 i1 = min(i0 + 1, ivec2(uGrid) - 1);
  float a00 = float(texelFetch(uFA, i0, 0).r);
  float a10 = float(texelFetch(uFA, ivec2(i1.x, i0.y), 0).r);
  float a01 = float(texelFetch(uFA, ivec2(i0.x, i1.y), 0).r);
  float a11 = float(texelFetch(uFA, i1, 0).r);
  float va = mix(mix(a00, a10, f.x), mix(a01, a11, f.x), f.y);
  float b00 = float(texelFetch(uFB, i0, 0).r);
  float b10 = float(texelFetch(uFB, ivec2(i1.x, i0.y), 0).r);
  float b01 = float(texelFetch(uFB, ivec2(i0.x, i1.y), 0).r);
  float b11 = float(texelFetch(uFB, i1, 0).r);
  float vb = mix(mix(b00, b10, f.x), mix(b01, b11, f.x), f.y);
  return (mix(va, vb, uMix) / 65535.0) * 2.0 - 1.0;
}

float n2(vec2 p) { return texture(uNoise, p / 256.0).r; }
float h12(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

void main() {
  vec2 px = vUV * uRes;
  float s = fld(px);
  float gx = fld(px + vec2(3.0, 0.0)) - fld(px - vec2(3.0, 0.0));
  float gy = fld(px + vec2(0.0, 3.0)) - fld(px - vec2(0.0, 3.0));
  vec2 grad = vec2(gx, gy);

  // Subtle street horizon perspective gradient
  float skyFade = clamp(1.0 - vUV.y, 0.0, 1.0);
  float texA = n2(px * 0.55 + uSeed * 13.0);
  float texB = n2(px * 2.1 - uSeed * 7.0);

  // --- Cobblestone pattern (running-bond Arnavut kaldirimi) ---
  vec2 cSize = vec2(34.0, 22.0);
  float rowIdx = floor(px.y / cSize.y);
  vec2 cUV = px / cSize;
  cUV.x += mod(rowIdx, 2.0) * 0.5;
  vec2 cId = floor(cUV);
  vec2 cF = fract(cUV);
  float stoneRnd = h12(cId + uSeed);
  vec2 dEdge = min(cF, 1.0 - cF);
  float stoneBevel = smoothstep(0.04, 0.18, min(dEdge.x, dEdge.y));
  float mortar = 1.0 - stoneBevel;

  // Dusk boulevard cobblestone vs Sunny schoolyard asphalt/concrete
  vec3 duskStone = mix(vec3(0.14, 0.16, 0.21), vec3(0.22, 0.23, 0.28), stoneRnd) * (0.85 + 0.3 * texB);
  vec3 sunnyStone = mix(vec3(0.62, 0.59, 0.54), vec3(0.71, 0.67, 0.60), stoneRnd) * (0.9 + 0.18 * texB);
  vec3 streetBase = mix(duskStone, sunnyStone, uVibe);
  vec3 mortarCol = mix(vec3(0.07, 0.08, 0.11), vec3(0.45, 0.43, 0.39), uVibe);
  vec3 streetCol = mix(streetBase, mortarCol, mortar * 0.72);

  // Subtle Beyoglu tramway double steel rails near middle-bottom horizontal band
  float railBand = px.y - uRes.y * 0.68;
  float r1 = smoothstep(3.2, 0.8, abs(railBand - 24.0));
  float r2 = smoothstep(3.2, 0.8, abs(railBand + 24.0));
  float rail = max(r1, r2) * smoothstep(0.02, 0.15, s);
  streetCol = mix(streetCol, mix(vec3(0.42, 0.44, 0.50), vec3(0.55, 0.56, 0.58), uVibe), rail * 0.62);

  // Rain puddles reflecting amber streetlights on open cobblestone lanes (stronger in Dusk mode)
  float puddleNoise = n2(px * 0.22 + uSeed * 19.0) * 0.65 + n2(px * 0.62 - uSeed * 11.0) * 0.35;
  float puddleMask = smoothstep(0.60, 0.68, puddleNoise) * smoothstep(0.08, 0.24, s);
  float lampBand = 0.5 + 0.5 * sin(px.x * 0.011 + 1.2) * cos(px.y * 0.009 - 0.5);
  float lampPulse = 0.90 + 0.10 * sin(uTime * 1.9 + px.x * 0.008);
  vec3 amberReflect = mix(vec3(0.96, 0.64, 0.26), vec3(0.84, 0.91, 0.98), uVibe) * (0.35 + 0.65 * uGlow) * lampPulse;
  float ripple = sin(px.x * 0.14 + px.y * 0.20 - uTime * 2.2) * 0.5 + 0.5;
  streetCol = mix(streetCol, streetCol * 0.35 + amberReflect * (0.28 + 0.32 * lampBand + 0.14 * ripple), puddleMask * (1.0 - 0.60 * uVibe));

  // Subtle warm streetlight glow falloff along open cobblestone
  streetCol += vec3(0.16, 0.09, 0.03) * lampBand * (0.3 + 0.7 * uGlow) * (1.0 - 0.85 * uVibe);

  // --- Sidewalk Cafe / Schoolyard Patio Tile Islands (s < 0) ---
  vec2 pUV = px / 44.0;
  vec2 pId = floor(pUV);
  vec2 pF = fract(pUV);
  float pRnd = h12(pId + uSeed * 3.1);
  float pEdge = smoothstep(0.02, 0.08, min(min(pF.x, 1.0 - pF.x), min(pF.y, 1.0 - pF.y)));
  vec3 duskPatio = mix(vec3(0.56, 0.42, 0.29), vec3(0.68, 0.52, 0.36), pRnd);
  vec3 sunnyPatio = mix(vec3(0.82, 0.74, 0.61), vec3(0.90, 0.82, 0.68), pRnd);
  vec3 patioCol = mix(duskPatio, sunnyPatio, uVibe) * (0.88 + 0.24 * texB);
  patioCol = mix(patioCol * 0.72, patioCol, pEdge);

  // Warm cafe terrace cast on patio centers
  patioCol += vec3(0.14, 0.08, 0.02) * uGlow * (1.0 - uVibe);

  // Curbstone rim transition around s = 0
  float curbDrop = smoothstep(0.08, -0.02, s) * (1.0 - smoothstep(-0.02, -0.10, s));
  float aa = 0.022;
  float isPatio = 1.0 - smoothstep(-aa, aa, s);
  vec3 col = mix(streetCol, patioCol, isPatio);

  // Raised granite curbstone bevel & drop shadow
  float shadowBand = smoothstep(0.0, 0.11, s) * (1.0 - smoothstep(0.0, 0.11, s) * 0.0);
  float outerShadow = smoothstep(0.10, 0.0, s) * step(0.0, s);
  col *= 1.0 - outerShadow * 0.42;

  float curbBevel = smoothstep(-0.06, -0.015, s) * (1.0 - smoothstep(-0.015, 0.02, s));
  vec3 curbCol = mix(vec3(0.52, 0.47, 0.41), vec3(0.76, 0.72, 0.66), uVibe);
  // Subtle directional light on top-left edge of curb
  float sunDot = dot(normalize(grad + vec2(1e-5)), normalize(vec2(-0.6, -0.8)));
  curbCol *= 0.85 + 0.25 * clamp(sunDot, -1.0, 1.0);
  col = mix(col, curbCol, curbBevel * 0.82);

  // Gentle top dusk atmosphere vignette
  col *= 0.88 + 0.14 * skyFade;

  if (uDebug > 0.5) {
    float line = 1.0 - smoothstep(0.0, 0.03, abs(s));
    col = mix(col, vec3(1.0, 0.55, 0.1), line * 0.85);
  }
  O = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

pub const ROACH_INST_VS: &str = r#"#version 330 core
layout(location=0) in vec2 aQuad;
layout(location=1) in vec4 aPosScaleRot; // x, y, size, rot
layout(location=2) in vec3 aTileAlpha;   // col, row, alpha
uniform vec2 uRes;
uniform vec2 uGridAtlas;
out vec2 vUV;
out float vAlpha;
void main() {
  float c = cos(aPosScaleRot.w), s = sin(aPosScaleRot.w);
  vec2 p = vec2(aQuad.x * c - aQuad.y * s, aQuad.x * s + aQuad.y * c) * aPosScaleRot.z + aPosScaleRot.xy;
  vec2 ndc = (p / uRes) * 2.0 - 1.0;
  gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
  vUV = (aQuad + 0.5 + aTileAlpha.xy) / uGridAtlas;
  vAlpha = aTileAlpha.z;
}
"#;

pub const ROACH_INST_FS: &str = r#"#version 330 core
uniform sampler2D uAtlas;
in vec2 vUV;
in float vAlpha;
out vec4 O;
void main() {
  vec4 t = texture(uAtlas, vUV);
  if (t.a * vAlpha < 0.01) discard;
  O = vec4(t.rgb, t.a * vAlpha);
}
"#;
