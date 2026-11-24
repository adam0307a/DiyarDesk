// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Voxel Craft World (voxelcraft)
// Ported 1:1 from prototype/voxelcraft.html

// Renders 2D geometry in CSS pixel space (0..W, 0..H) with per-vertex RGBA color
pub const VC_COLOR_VERT: &str = r#"#version 330 core
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

pub const VC_COLOR_FRAG: &str = r#"#version 330 core
in vec4 v_col;
out vec4 fragColor;

void main() {
    fragColor = v_col;
}
"#;

// Textured quads (16x16 block atlas, 128x128 radial glow sprite, cached terrain layer)
pub const VC_SPRITE_VERT: &str = r#"#version 330 core
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

pub const VC_SPRITE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
in vec4 v_col;
out vec4 fragColor;

uniform sampler2D u_atlas;

void main() {
    vec4 tex = texture(u_atlas, v_uv);
    fragColor = vec4(tex.rgb * v_col.rgb, tex.a * v_col.a);
}
"#;
