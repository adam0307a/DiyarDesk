// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Quiet Snow (Sessiz Kar)
// Ported 1:1 from prototype/quietsnow.html + prototype/quietsnow-engine.js

// Exact HTML5 Canvas 2D createRadialGradient(x0, y0, r0, x1, y1, r1) with up to 4 stops
pub const QS_RADIAL_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_res;
uniform vec3 u_c0; // (x0, y0, r0)
uniform vec3 u_c1; // (x1, y1, r1)
uniform int u_stop_count;
uniform vec4 u_stops; // stop positions t0, t1, t2, t3
uniform vec4 u_col0;
uniform vec4 u_col1;
uniform vec4 u_col2;
uniform vec4 u_col3;

void main() {
    vec2 p = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    vec2 d = p - u_c0.xy;
    vec2 dc = u_c1.xy - u_c0.xy;
    float r0 = u_c0.z;
    float dr = u_c1.z - r0;

    float t = 0.0;
    if (dot(dc, dc) < 0.001) {
        t = (length(d) - r0) / max(0.001, dr);
    } else {
        // Solve |d - t*dc|^2 = (r0 + t*dr)^2  =>  A*t^2 - 2*B*t + C = 0
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
            // Pick larger root that corresponds to positive radius r0 + t*dr >= 0
            t = (r0 + t2 * dr >= 0.0) ? t2 : t1;
        }
    }
    t = clamp(t, 0.0, 1.0);

    vec4 col = u_col0;
    if (u_stop_count == 2) {
        float u = clamp((t - u_stops.x) / max(0.0001, u_stops.y - u_stops.x), 0.0, 1.0);
        col = mix(u_col0, u_col1, u);
    } else if (u_stop_count == 3) {
        if (t < u_stops.y) {
            float u = clamp((t - u_stops.x) / max(0.0001, u_stops.y - u_stops.x), 0.0, 1.0);
            col = mix(u_col0, u_col1, u);
        } else {
            float u = clamp((t - u_stops.y) / max(0.0001, u_stops.z - u_stops.y), 0.0, 1.0);
            col = mix(u_col1, u_col2, u);
        }
    } else if (u_stop_count >= 4) {
        if (t < u_stops.y) {
            float u = clamp((t - u_stops.x) / max(0.0001, u_stops.y - u_stops.x), 0.0, 1.0);
            col = mix(u_col0, u_col1, u);
        } else if (t < u_stops.z) {
            float u = clamp((t - u_stops.y) / max(0.0001, u_stops.z - u_stops.y), 0.0, 1.0);
            col = mix(u_col1, u_col2, u);
        } else {
            float u = clamp((t - u_stops.z) / max(0.0001, u_stops.w - u_stops.z), 0.0, 1.0);
            col = mix(u_col2, u_col3, u);
        }
    }
    fragColor = col;
}
"#;

pub const QS_COLOR_VERT: &str = r#"#version 330 core
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

pub const QS_COLOR_FRAG: &str = r#"#version 330 core
in vec4 v_col;
out vec4 fragColor;

void main() {
    fragColor = v_col;
}
"#;

pub const QS_SPRITE_VERT: &str = r#"#version 330 core
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

pub const QS_SPRITE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
in vec4 v_col;
out vec4 fragColor;

uniform sampler2D u_atlas;

void main() {
    vec4 tex = texture(u_atlas, v_uv);
    fragColor = vec4(tex.rgb * v_col.rgb, tex.a * v_col.a);
}
"#;
