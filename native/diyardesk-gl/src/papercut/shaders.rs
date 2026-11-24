// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Kâğıt Bahçe — Katı' Işığında (papercut)
// Ported 1:1 from prototype/papercut.html

// Renders 2D geometry in CSS pixel space (0..W, 0..H) with per-vertex RGBA color
pub const PC_COLOR_VERT: &str = r#"#version 330 core
layout (location = 0) in vec2 a_pos;
layout (location = 1) in vec4 a_col;

uniform vec2 u_res;

out vec4 v_col;

void main() {
    vec2 ndc = vec2(
        (a_pos.x / max(1.0, u_res.x)) * 2.0 - 1.0,
        1.0 - (a_pos.y / max(1.0, u_res.y)) * 2.0
    );
    v_col = a_col;
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

pub const PC_COLOR_FRAG: &str = r#"#version 330 core
in vec4 v_col;
out vec4 fragColor;

void main() {
    fragColor = v_col;
}
"#;

// Applies a cut-paper layer from u_mask onto the current framebuffer (outputs PREMULTIPLIED RGBA
// for exact Porter-Duff source-over compositing via glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA)):
// 1. Contact drop shadow (offset1, blur1, alpha1)
// 2. Soft ambient drop shadow (offset2, blur2, alpha2)
// 3. Crisp paper fill (u_color)
// 4. Delicate 1px white bevel edge stroke (rgba(255,255,255, u_rim_alpha))
pub const PC_PAPER_APPLY_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform sampler2D u_mask;
uniform vec2 u_res;
uniform vec4 u_color;
uniform vec2 u_sh_off1;   // in CSS pixels
uniform float u_sh_blur1; // in CSS pixels
uniform float u_sh_a1;
uniform vec2 u_sh_off2;   // in CSS pixels
uniform float u_sh_blur2; // in CSS pixels
uniform float u_sh_a2;
uniform float u_rim_a;

// 25-tap Vogel spiral disk for smooth, stipple-free Gaussian shadow blur matching Canvas2D shadowBlur
float sampleBlurredMask(vec2 uvCenter, float radiusPx) {
    if (radiusPx <= 0.35) {
        return texture(u_mask, uvCenter).a;
    }
    vec2 radUv = vec2(radiusPx * 1.15 / max(1.0, u_res.x), -(radiusPx * 1.15 / max(1.0, u_res.y)));
    float sum = texture(u_mask, uvCenter).a * 1.6;
    float wSum = 1.6;
    const float GOLDEN = 2.39996323;
    for (int i = 1; i <= 24; i++) {
        float fi = float(i);
        float r = sqrt(fi / 24.5);
        float theta = fi * GOLDEN;
        vec2 o = vec2(cos(theta), sin(theta)) * r;
        float w = exp(-r * r * 2.2);
        sum += texture(u_mask, uvCenter + o * radUv).a * w;
        wSum += w;
    }
    return sum / wSum;
}

void main() {
    vec2 pxStep = vec2(1.0 / max(1.0, u_res.x), 1.0 / max(1.0, u_res.y));
    vec2 uv1 = v_uv + vec2(-u_sh_off1.x * pxStep.x, u_sh_off1.y * pxStep.y);
    vec2 uv2 = v_uv + vec2(-u_sh_off2.x * pxStep.x, u_sh_off2.y * pxStep.y);

    // 5-tap supersampled body alpha + 1px bevel edge detection
    float bodyA = texture(u_mask, v_uv).a;
    float nL = texture(u_mask, v_uv - vec2(pxStep.x, 0.0)).a;
    float nR = texture(u_mask, v_uv + vec2(pxStep.x, 0.0)).a;
    float nU = texture(u_mask, v_uv + vec2(0.0, pxStep.y)).a;
    float nD = texture(u_mask, v_uv - vec2(0.0, pxStep.y)).a;
    float smoothA = clamp((bodyA * 2.0 + nL + nR + nU + nD) / 6.0, 0.0, 1.0);

    // Shadow 1 (contact) + Shadow 2 (soft ambient)
    float s1 = (u_sh_a1 > 0.001) ? sampleBlurredMask(uv1, u_sh_blur1) * u_sh_a1 : 0.0;
    float s2 = (u_sh_a2 > 0.001) ? sampleBlurredMask(uv2, u_sh_blur2) * u_sh_a2 : 0.0;

    // Composite shadow1 then shadow2 in premultiplied space (matching Canvas 2D sequential fill)
    vec3 shRgb = vec3(12.0 / 255.0, 10.0 / 255.0, 36.0 / 255.0);
    float shA = s1 + s2 * (1.0 - s1);
    vec4 res = vec4(shRgb * shA, shA);

    // Composite crisp paper fill over shadow (premultiplied source-over)
    float fillA = smoothA * u_color.a;
    vec3 fillPremul = u_color.rgb * fillA;
    res = vec4(fillPremul + res.rgb * (1.0 - fillA), fillA + res.a * (1.0 - fillA));

    // Composite 1px white cut-paper bevel rim around shape boundary
    if (u_rim_a > 0.001) {
        float grad = clamp(abs(nR - nL) + abs(nU - nD), 0.0, 1.0);
        float rim = grad * u_rim_a;
        res = vec4(vec3(rim) + res.rgb * (1.0 - rim), rim + res.a * (1.0 - rim));
    }

    fragColor = res;
}
"#;

// Punches the inner arch niche holes through the foreground to reveal u_back,
// and casts the inner even-odd cavity shadow (sh(c, 5, 10, 0.55)) inside the hole
pub const PC_NICHE_HOLE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform sampler2D u_hole_mask;
uniform sampler2D u_back;
uniform vec2 u_res;
uniform vec2 u_sh_off;   // in CSS pixels: -L * 5 * gap
uniform float u_sh_blur; // in CSS pixels: 10 * gap
uniform float u_sh_a;    // min(0.9, 0.55 * shadow)

void main() {
    float holeA = texture(u_hole_mask, v_uv).a;
    if (holeA <= 0.001) {
        discard;
    }
    vec3 backCol = texture(u_back, v_uv).rgb;

    // Outside of the hole (1.0 - hole_mask) casts an inner cavity shadow into the hole
    vec2 pxStep = vec2(1.0 / max(1.0, u_res.x), 1.0 / max(1.0, u_res.y));
    vec2 uvSh = v_uv + vec2(-u_sh_off.x * pxStep.x, u_sh_off.y * pxStep.y);
    vec2 radUv = vec2(u_sh_blur * 1.15 * pxStep.x, -u_sh_blur * 1.15 * pxStep.y);

    float sum = (1.0 - texture(u_hole_mask, uvSh).a) * 1.6;
    float wSum = 1.6;
    const float GOLDEN = 2.39996323;
    for (int i = 1; i <= 24; i++) {
        float fi = float(i);
        float r = sqrt(fi / 24.5);
        float theta = fi * GOLDEN;
        vec2 o = vec2(cos(theta), sin(theta)) * r;
        float w = exp(-r * r * 2.2);
        sum += (1.0 - texture(u_hole_mask, uvSh + o * radUv).a) * w;
        wSum += w;
    }
    float cavSh = clamp((sum / wSum) * u_sh_a, 0.0, 0.9);
    vec3 shRgb = vec3(12.0 / 255.0, 10.0 / 255.0, 36.0 / 255.0);
    vec3 outCol = mix(backCol, shRgb, cavSh);

    // Output premultiplied RGBA
    fragColor = vec4(outCol * holeA, holeA);
}
"#;

// Applies the 128x128 paper fiber grain texture (multiply blend) and directional radial vignette
pub const PC_POST_GRAIN_VIG_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform sampler2D u_comp;
uniform sampler2D u_grain;
uniform vec2 u_res;
uniform vec2 u_phys_res;
uniform float u_grain_alpha; // 0.08 + P.grain * 0.32
uniform vec3 u_vig_c0;       // (x0, y0, r0) in CSS px
uniform vec3 u_vig_c1;       // (x1, y1, r1) in CSS px

void main() {
    vec3 col = texture(u_comp, v_uv).rgb;

    // 1. Paper grain repeat pattern in physical pixels (c.setTransform(1,0,0,1,0,0); multiply)
    vec2 physPx = vec2(v_uv.x * u_phys_res.x, (1.0 - v_uv.y) * u_phys_res.y);
    vec3 grain = texture(u_grain, physPx / 128.0).rgb;
    col = mix(col, col * grain, clamp(u_grain_alpha, 0.0, 1.0));

    // 2. Two-circle directional radial light vignette
    vec2 p = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    vec2 d = p - u_vig_c0.xy;
    vec2 dc = u_vig_c1.xy - u_vig_c0.xy;
    float r0 = u_vig_c0.z;
    float dr = u_vig_c1.z - r0;
    float t = 0.0;
    if (dot(dc, dc) < 0.001) {
        t = (length(d) - r0) / max(0.001, dr);
    } else {
        float A = dot(dc, dc) - dr * dr;
        float B = dot(d, dc) + r0 * dr;
        float C = dot(d, d) - r0 * r0;
        if (abs(A) < 1e-5) {
            t = C / max(1e-5, 2.0 * B);
        } else {
            float disc = max(0.0, B * B - A * C);
            float s = sqrt(disc);
            float t1 = (B + s) / A;
            float t2 = (B - s) / A;
            t = (r0 + t2 * dr >= 0.0) ? t2 : t1;
        }
    }
    t = clamp(t, 0.0, 1.0);
    // Stop 0: rgba(255,255,255,0.06), Stop 1: rgba(10,8,30,0.30)
    vec4 v0 = vec4(1.0, 1.0, 1.0, 0.06);
    vec4 v1 = vec4(10.0 / 255.0, 8.0 / 255.0, 30.0 / 255.0, 0.30);
    vec4 vg = mix(v0, v1, t);
    col = mix(col, vg.rgb, vg.a);

    fragColor = vec4(col, 1.0);
}
"#;

pub const PC_SPRITE_VERT: &str = r#"#version 330 core
layout (location = 0) in vec2 a_pos;
layout (location = 1) in vec2 a_uv;
layout (location = 2) in vec4 a_col;

uniform vec2 u_res;

out vec2 v_uv;
out vec4 v_col;

void main() {
    vec2 ndc = vec2(
        (a_pos.x / max(1.0, u_res.x)) * 2.0 - 1.0,
        1.0 - (a_pos.y / max(1.0, u_res.y)) * 2.0
    );
    v_uv = a_uv;
    v_col = a_col;
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

pub const PC_SPRITE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
in vec4 v_col;
out vec4 fragColor;

uniform sampler2D u_atlas;

void main() {
    vec4 tex = texture(u_atlas, v_uv);
    fragColor = vec4(tex.rgb * v_col.rgb, tex.a * v_col.a);
}
"#;
