// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Claymorph (Q Harfi 3D Claymorphism)
// Ported 1:1 from prototype/claymorph.html

// Renders 2D geometry in CSS pixel space (0..W, 0..H) with per-vertex RGBA color
pub const CM_COLOR_VERT: &str = r#"#version 330 core
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

pub const CM_COLOR_FRAG: &str = r#"#version 330 core
in vec4 v_col;
out vec4 fragColor;

void main() {
    fragColor = v_col;
}
"#;

// Textured sprite blitter: draws a baked clay sprite (atlas region) with per-vertex RGBA tint.
// Uses premultiplied output so the caller can blend with glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA).


// Simple full-screen blit of an FBO color texture onto the default framebuffer



// Per-frame clay body vertex shader: passes CSS-pixel position to the fragment stage.
pub const CM_CLAY_VERT: &str = r#"#version 330 core
layout (location = 0) in vec2 a_pos;

uniform vec2 u_res;

out vec2 v_pos;

void main() {
    v_pos = a_pos;
    vec2 ndc = vec2(
        (a_pos.x / max(1.0, u_res.x)) * 2.0 - 1.0,
        1.0 - (a_pos.y / max(1.0, u_res.y)) * 2.0
    );
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

// Per-frame analytic SDF clay shading, 1:1 with claymorph.html's offscreen bake:
//   soft tinted drop shadow + directional volume gradient + inner cushion highlight
//   + bottom-right bounce rim + soft specular bean highlight.
pub const CM_CLAY_FRAG: &str = r#"#version 330 core
in vec2 v_pos;
out vec4 fragColor;

uniform vec2  u_res;
uniform vec2  u_center;
uniform vec2  u_half;
uniform float u_radius;
uniform vec4  u_xform;   // columns of the 2x2 inverse transform: (m00, m10, m01, m11)
uniform int   u_is_orb;
uniform int   u_is_ring;
uniform float u_ringT;
uniform float u_alpha;
uniform float u_puff;
uniform vec3  u_hi;
uniform vec3  u_mid;
uniform vec3  u_lo;
uniform vec3  u_spec;
uniform vec3  u_rim;
uniform vec3  u_drop;

float sdRoundBox(vec2 p, vec2 hs, float r) {
    vec2 q = abs(p) - hs + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

void main() {
    vec2 lp = vec2(u_xform.x * (v_pos.x - u_center.x) + u_xform.z * (v_pos.y - u_center.y),
                   u_xform.y * (v_pos.x - u_center.x) + u_xform.w * (v_pos.y - u_center.y));
    float d = sdRoundBox(lp, u_half, u_radius);

    float ringMask = 1.0;
    if (u_is_ring == 1) {
        ringMask = step(abs(d + u_ringT * 0.5), u_ringT * 0.5);
    }

    // Soft tinted drop shadow, lifted down
    vec2 shHs = u_half + vec2(9.0 * u_puff);
    float shD = sdRoundBox(lp - vec2(0.0, 11.0 * u_puff * 0.45), shHs, u_radius + 9.0 * u_puff);
    float blur = max(2.0, 10.0 * u_puff);
    float sh = exp(-max(shD, 0.0) / blur) * 0.34 * ringMask;

    // Body volume gradient
    vec3 body;
    if (u_is_orb == 1) {
        float gd = clamp(length(lp - vec2(-u_radius * 0.34, -u_radius * 0.34)) / max(1.0, u_radius * 1.15), 0.0, 1.0);
        body = (gd < 0.55) ? mix(u_hi, u_mid, gd / 0.55)
                           : mix(u_mid, u_lo, (gd - 0.55) / 0.45);
    } else {
        float t = clamp((lp.x / max(1.0, u_half.x) + lp.y / max(1.0, u_half.y)) * 0.25 + 0.5, 0.0, 1.0);
        body = (t < 0.48) ? mix(u_hi, u_mid, t / 0.48)
                          : mix(u_mid, u_lo, (t - 0.48) / 0.52);
    }

    // Inner cushion highlight (top-left) + bounce rim (bottom-right)
    float innerDepth = max(0.0, -d);
    float band = clamp(1.0 - innerDepth / max(1.0, 5.5 * u_puff), 0.0, 1.0);
    vec2 gdir = normalize(lp + vec2(1e-4));
    float tl = clamp(-(gdir.x + gdir.y) * 0.5 + 0.5, 0.0, 1.0);
    body += u_spec * band * tl * 0.55 * u_puff;
    body += u_rim * band * (1.0 - tl) * 0.45 * u_puff;

    // Soft specular bean highlight at upper-left shoulder
    vec2 bc = (u_is_orb == 1) ? vec2(-u_radius * 0.36, -u_radius * 0.38)
                              : vec2(-u_half.x * 0.52, -u_half.y * 0.60);
    vec2 br = (u_is_orb == 1) ? vec2(max(2.0, u_radius * 0.24), max(2.0, u_radius * 0.12))
                              : vec2(max(3.0, u_half.x * 0.48), max(3.0, u_half.y * 0.40));
    vec2 bd = (lp - bc) / br;
    float gloss = exp(-dot(bd, bd) * 2.0) * 0.42 * u_puff * (u_is_ring == 1 ? 0.0 : 1.0);
    body += u_spec * gloss;

    float alpha = (1.0 - smoothstep(-1.0, 1.0, d)) * ringMask;
    float aOut = alpha + sh * (1.0 - alpha);
    vec3 rgb = (body * alpha + u_drop * sh * (1.0 - alpha)) / max(aOut, 1e-4);
    float outA = clamp(aOut * u_alpha, 0.0, 1.0);
    fragColor = vec4(clamp(rgb, 0.0, 1.0) * outA, outA);
}
"#;
