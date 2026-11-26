//! GLSL programs for the eye-within-words theme.
//!
//! PASS 1 (`EYE_COMPOSITE_FRAG`) reproduces `render_fullscreen_composition`:
//! the full-canvas typography wall, the sclera/iris/pupil/highlight stack
//! clipped to the instantaneous eyelid curves, and the cinematic camera crop.
//! PASS 2 (`EYE_SPRITE_*`) blits the pre-rotated eyelid-rim word sprites.

pub const EYE_SPRITE_VERT: &str = r#"#version 330 core
layout(location = 0) in vec2 a_pos;   // physical screen px, top-left origin
layout(location = 1) in vec2 a_uv;    // rim atlas uv
uniform vec2 u_res;
out vec2 v_uv;
void main() {
    v_uv = a_uv;
    gl_Position = vec4(a_pos.x / u_res.x * 2.0 - 1.0, 1.0 - a_pos.y / u_res.y * 2.0, 0.0, 1.0);
}
"#;

pub const EYE_SPRITE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;
uniform sampler2D u_tex;
void main() {
    fragColor = texture(u_tex, v_uv);
}
"#;

pub const EYE_COMPOSITE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_res;            // physical screen px
uniform vec4 u_cam;            // camera rect (left, top, w, h) in BAKE canvas px
uniform vec2 u_canvas;         // bake canvas dims (px)
uniform vec4 u_eye;            // center_x, center_y, eye_half_w, eye_half_h (bake px)
uniform vec2 u_iris_c;         // iris center (bake canvas px)
uniform vec2 u_hl_pos;         // highlight layer top-left (bake canvas px)
uniform float u_iris_dim;
uniform float u_pupil_dim;
uniform float u_cutout_dim;
uniform float u_openness;
uniform float u_now;
uniform float u_glow_a;
uniform float u_iris_mix;
uniform float u_r_scale;
uniform float u_p_scale;
uniform vec2 u_sclera_off;
uniform float u_lid_gaze_y;

uniform int u_adaptive;
uniform int u_icon_count;
uniform vec4 u_icons[24];      // physical screen px rects (x, y, w, h)

uniform sampler2D u_field;
uniform sampler2D u_sclera;
uniform sampler2D u_cutout;
uniform sampler2D u_glow;
uniform sampler2D u_rings_a;
uniform sampler2D u_rings_b;
uniform sampler2D u_radial_a;
uniform sampler2D u_radial_b;
uniform sampler2D u_pupil;
uniform sampler2D u_highlights;

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    vec2 cv = vec2(
        u_cam.x + px.x / u_res.x * u_cam.z,
        u_cam.y + px.y / u_res.y * u_cam.w
    );

    // 1. Full-canvas edge-to-edge wall of typography
    vec4 col = texture(u_field, cv / u_canvas);

    // 2. Eye interior (sclera + iris + pupil + highlights) clipped by the eyelids
    if (u_openness > 0.012) {
        float u = clamp((cv.x - u_eye.x) / u_eye.z, -1.0, 1.0);
        float env = pow(max(0.0, 1.0 - u * u), 0.76);
        float ua = 1.0 - 0.11 * u - 0.06 * u * u;
        float la = 1.0 + 0.13 * u - 0.05 * u * u;
        float yu_open = -u_eye.w * 1.04 * env * ua;
        float yl_open =  u_eye.w * 0.88 * env * la;
        float medial_dip = 0.09 * u_eye.w * exp(-pow((u + 0.89) / 0.20, 2.0));
        float lateral_sweep = -0.06 * u_eye.w * exp(-pow((u - 0.86) / 0.25, 2.0));
        float corner_line = medial_dip + lateral_sweep;
        yu_open += corner_line;
        yl_open += corner_line;
        float y_closed = u_eye.w * 0.16 * env + corner_line;
        float t = clamp(u_openness, 0.0, 1.0);
        float yu = mix(y_closed, yu_open, t);
        float yl = mix(y_closed, yl_open, t);

        float dy = cv.y - u_eye.y;
        float lo = yu + u_lid_gaze_y;
        float hi = yl + u_lid_gaze_y;

        // Polygon fill with a soft 4px outline band (alpha 140/255 at the seam)
        float band = smoothstep(lo - 2.0, lo + 2.0, dy) * (1.0 - smoothstep(hi - 2.0, hi + 2.0, dy));
        float edge_dist = min(dy - lo, hi - dy);
        float eye_a = band * mix(0.55, 1.0, smoothstep(0.0, 4.0, edge_dist));

        if (eye_a > 0.003) {
            // Sclera follows the gaze with a slight parallax
            vec2 sc = (cv - u_sclera_off) / u_canvas;
            vec4 s = texture(u_sclera, sc);
            vec4 E = vec4(s.rgb * s.a, s.a);   // premultiplied

            // Circular cutout so sclera quotes do not bleed behind the iris
            vec2 cut_p = (cv - u_iris_c + 0.5 * u_cutout_dim) / u_cutout_dim;
            vec4 cut = texture(u_cutout, cut_p);
            E.rgb *= cut.rgb * cut.a;
            E.a *= cut.a;

            // Restrained atmospheric glow (pulsating additive bloom)
            vec2 glow_p = (cv - u_iris_c + 0.5 * u_iris_dim) / u_iris_dim;
            vec4 glow_s = texture(u_glow, glow_p);
            E.rgb += glow_s.rgb * glow_s.a * u_glow_a;
            E.a = min(1.0, E.a + glow_s.a * u_glow_a);

            // Concentric iris rings with slow color transition
            vec2 rp = (cv - u_iris_c + 0.5 * u_iris_dim) / u_iris_dim;
            vec4 ra = texture(u_rings_a, rp);
            vec4 rb = texture(u_rings_b, rp);
            E = E * (1.0 - ra.a) + vec4(ra.rgb * ra.a, ra.a);
            float ab = rb.a * u_iris_mix;
            E = E * (1.0 - ab) + vec4(rb.rgb * ab, ab);

            // Radial iris spokes with subtle breathing
            vec2 rdp = ((cv - u_iris_c) / u_r_scale + 0.5 * u_iris_dim) / u_iris_dim;
            vec4 rda = texture(u_radial_a, rdp);
            vec4 rdb = texture(u_radial_b, rdp);
            float aa = rda.a * (230.0 / 255.0);
            E = E * (1.0 - aa) + vec4(rda.rgb * aa, aa);
            float abb = rdb.a * u_iris_mix * (230.0 / 255.0);
            E = E * (1.0 - abb) + vec4(rdb.rgb * abb, abb);

            // Deep circular pupil with subtle dilation / contraction
            vec2 pp = ((cv - u_iris_c) / u_p_scale + 0.5 * u_pupil_dim) / u_pupil_dim;
            vec4 pu = texture(u_pupil, pp);
            E = E * (1.0 - pu.a) + vec4(pu.rgb * pu.a, pu.a);

            // Crisp corneal highlight words
            vec2 hp = (cv - u_hl_pos) / u_iris_dim;
            vec4 hl = texture(u_highlights, hp);
            E = E * (1.0 - hl.a) + vec4(hl.rgb * hl.a, hl.a);

            vec3 eye_rgb = E.rgb / max(E.a, 1e-5);
            col.rgb = col.rgb * (1.0 - E.a * eye_a) + eye_rgb * E.a * eye_a;
        }
    }

    // 3. Adaptive desktop-icon reading scrims (DiyarDesk addition)
    if (u_adaptive == 1) {
        for (int i = 0; i < 24; i++) {
            if (i >= u_icon_count) { break; }
            vec4 ir = u_icons[i];
            vec2 half_ext = ir.zw * 0.5 + vec2(10.0);
            vec2 c = ir.xy + ir.zw * 0.5;
            float d = sdRoundBox(px - c, half_ext, 10.0);
            float a = 1.0 - smoothstep(-2.0, 3.0, d);
            col.rgb = mix(col.rgb, vec3(0.012, 0.024, 0.051), a * 0.52);
        }
    }

    fragColor = vec4(col.rgb, 1.0);
}
"#;
