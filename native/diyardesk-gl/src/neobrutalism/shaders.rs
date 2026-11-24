// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Neo-Brutalism (Kinetic Poster)
// Ported 1:1 from prototype/neobrutalism.html

// Colored geometry in CSS pixel space (0..W, 0..H) with per-vertex RGBA (straight alpha).
pub const NB_COLOR_VERT: &str = r#"#version 330 core
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

pub const NB_COLOR_FRAG: &str = r#"#version 330 core
in vec4 v_col;
out vec4 fragColor;

void main() {
    fragColor = v_col;
}
"#;

// Static drafting-paper backdrop: base fill, architectural grid, comic halftone corner
// dot matrix, and bold poster frame inset. Rendered once into an FBO.
pub const NB_BG_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2  u_res;       // CSS px size
uniform vec3  u_bg;        // paper base color
uniform vec4  u_grid;      // grid line rgba
uniform vec4  u_dot;       // halftone dot rgba
uniform vec3  u_ink;       // frame border color
uniform float u_border;    // frame line width
uniform float u_dots;      // 1.0 = halftone enabled, 0.0 = off

void main() {
    vec2 px = vec2(v_uv.x, 1.0 - v_uv.y) * u_res;
    float W = u_res.x;
    float H = u_res.y;
    vec3 col = u_bg;

    // Architectural drafting grid (1px lines every 64px, offset 0.5)
    vec2 g = abs(mod(px + 0.5, 64.0) - 32.0);
    float lineD = min(32.0 - g.x, 32.0 - g.y);
    float gridA = 1.0 - smoothstep(0.0, 1.0, lineD);
    col = mix(col, u_grid.rgb, gridA * u_grid.a);

    // Comic halftone dot matrix concentrated in top-right & bottom-left corners
    if (u_dots > 0.5) {
        vec2 cell = floor(px / 18.0);
        vec2 c = (cell + 0.5) * 18.0;
        float dTopRight = length(vec2(W, 0.0) - c) / max(W, H);
        float dBotLeft = length(vec2(0.0, H) - c) / max(W, H);
        float band = min(dTopRight, dBotLeft);
        if (band < 0.36) {
            float rad = clamp((0.36 - band) * 7.2, 0.9, 2.6);
            float dd = length(px - c);
            float cov = 1.0 - smoothstep(rad - 0.8, rad + 0.8, dd);
            col = mix(col, u_dot.rgb, cov * u_dot.a);
        }
    }

    // Bold poster frame inset
    float pad = 16.0;
    vec2 halfSize = u_res * 0.5;
    vec2 q = abs(px - halfSize) - (halfSize - pad);
    float sdFrame = min(max(q.x, q.y), 0.0) + length(max(q, 0.0));
    float frameCov = 1.0 - smoothstep(u_border * 0.5, u_border * 0.5 + 1.0, abs(sdFrame));
    col = mix(col, u_ink, frameCov);

    fragColor = vec4(col, 1.0);
}
"#;