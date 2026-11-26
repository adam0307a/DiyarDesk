//! GLSL 3.30 Core fragment shader for `cosmic-eye`:
//! Live monospace ASCII-art rendering of a photorealistic blinking, gazing
//! human eye whose dark pupil opens seamlessly into deep starry space housing
//! a 3D spinning banded gas giant (Jupiter/Saturn) with tilted rings.

pub const COSMIC_EYE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_res;          // physical screen px
uniform float u_time;        // continuous animation time (s)
uniform float u_zoom;        // exponential camera zoom (1.0 = full eye .. ~16.5 = inside pupil)
uniform float u_openness;    // eyelid openness (0.0 = closed .. 1.0 = open)
uniform vec2 u_gaze;         // normalized gaze (-1..1, -1..1)
uniform float u_planet_rot;  // continuous planet axial rotation (radians)
uniform vec2 u_cell_px;      // ASCII cell width & height in physical px
uniform int u_palette;       // 0 = amberClassic, 1 = emeraldCosmos, 2 = cyanNebula
uniform int u_adaptive;      // 1 = soft dark scrims behind desktop icons
uniform int u_icon_count;
uniform vec4 u_icons[24];    // physical px rects (x, y, w, h)

uniform sampler2D u_atlas;   // 64-glyph monospace ASCII strip (luminance-sorted)

const float PI = 3.14159265359;
const float GLYPH_COUNT = 64.0;
const float TILE_W = 18.0;
const float TILE_H = 28.0;

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = hash21(i);
    float b = hash21(i + vec2(1.0, 0.0));
    float c = hash21(i + vec2(0.0, 1.0));
    float d = hash21(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

float fbm(vec2 p) {
    float v = 0.0;
    float amp = 0.5;
    mat2 rot = mat2(0.80, -0.60, 0.60, 0.80);
    for (int i = 0; i < 4; i++) {
        v += amp * vnoise(p);
        p = rot * p * 2.05 + vec2(13.7, 9.2);
        amp *= 0.5;
    }
    return v;
}

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

// ---------------------------------------------------------------------------
// Palette helpers
// ---------------------------------------------------------------------------
vec3 palSkinLight() {
    if (u_palette == 1) return vec3(0.42, 0.50, 0.46);
    if (u_palette == 2) return vec3(0.38, 0.46, 0.58);
    return vec3(0.58, 0.41, 0.31);
}

vec3 palSkinDark() {
    if (u_palette == 1) return vec3(0.11, 0.16, 0.14);
    if (u_palette == 2) return vec3(0.09, 0.12, 0.20);
    return vec3(0.16, 0.10, 0.07);
}

vec3 palIrisInner() {
    if (u_palette == 1) return vec3(0.45, 0.96, 0.68);
    if (u_palette == 2) return vec3(0.32, 0.92, 1.00);
    return vec3(0.96, 0.74, 0.36);
}

vec3 palIrisMid() {
    if (u_palette == 1) return vec3(0.16, 0.62, 0.40);
    if (u_palette == 2) return vec3(0.12, 0.48, 0.84);
    return vec3(0.68, 0.42, 0.16);
}

vec3 palIrisOuter() {
    if (u_palette == 1) return vec3(0.04, 0.16, 0.12);
    if (u_palette == 2) return vec3(0.04, 0.10, 0.24);
    return vec3(0.14, 0.08, 0.04);
}

vec3 palPlanetBand(float lat, float lon, float turb) {
    float b1 = sin(lat * 13.5 + turb * 2.8);
    float b2 = cos(lat * 25.0 - turb * 3.6);
    float eq = exp(-lat * lat * 7.5);
    float poles = smoothstep(0.55, 0.96, abs(lat));

    vec3 cream;
    vec3 ochre;
    vec3 rust;
    vec3 umber;
    if (u_palette == 1) {
        cream = vec3(0.84, 0.96, 0.88);
        ochre = vec3(0.32, 0.78, 0.58);
        rust  = vec3(0.16, 0.50, 0.44);
        umber = vec3(0.08, 0.24, 0.22);
    } else if (u_palette == 2) {
        cream = vec3(0.84, 0.94, 1.00);
        ochre = vec3(0.34, 0.74, 0.96);
        rust  = vec3(0.42, 0.34, 0.82);
        umber = vec3(0.12, 0.16, 0.36);
    } else {
        // 1:1 warm Jupiter / Saturn gas-giant bands from the reference frames
        cream = vec3(0.95, 0.85, 0.74);
        ochre = vec3(0.82, 0.54, 0.32);
        rust  = vec3(0.72, 0.30, 0.18);
        umber = vec3(0.34, 0.17, 0.11);
    }

    vec3 c = mix(ochre, cream, smoothstep(-0.25, 0.65, b1));
    c = mix(c, rust, smoothstep(0.15, 0.85, b2) * 0.58);
    c = mix(c, cream, eq * 0.35);
    c = mix(c, umber, poles * 0.62);

    // Great Red Spot / swirling storm vortex rotating with longitude
    float dlon = atan(sin(lon - 0.8), cos(lon - 0.8));
    float dlat = lat + 0.26;
    vec2 sp = vec2(dlon / 0.34, dlat / 0.17);
    float sdist = length(sp);
    if (sdist < 1.35) {
        float swirl = sin(atan(sp.y, sp.x) * 3.0 + sdist * 6.0);
        float spot_m = smoothstep(1.15, 0.25, sdist);
        vec3 spot_col = (u_palette == 0)
            ? vec3(0.88, 0.28, 0.16)
            : mix(rust * 1.4, cream, 0.25);
        c = mix(c, spot_col * (0.90 + 0.15 * swirl), spot_m * 0.82);
    }
    return c;
}

// ---------------------------------------------------------------------------
// Cosmos inside the pupil: starfield + 3D spinning ringed gas giant
// p_cos is in pupil-normalized coordinates (pupil edge at |p_cos| = 1.0)
// ---------------------------------------------------------------------------
vec3 renderCosmos(vec2 p_cos, vec2 cell_id) {
    // 1. Deep starry cosmos backdrop
    vec3 col = vec3(0.0);

    // Two parallax star layers (anchored to pupil space so zooming glides smoothly)
    for (int layer = 0; layer < 2; layer++) {
        float grid_s = (layer == 0) ? 14.0 : 28.0;
        vec2 sp = p_cos * grid_s + vec2(float(layer) * 19.3, float(layer) * 7.1);
        vec2 sid = floor(sp);
        vec2 sf = fract(sp) - 0.5;
        float h = hash21(sid + float(layer) * 91.7);
        float thresh = (layer == 0) ? 0.875 : 0.915;
        if (h > thresh) {
            vec2 jitter = vec2(hash21(sid + 3.1), hash21(sid + 8.7)) - 0.5;
            float d = length(sf - jitter * 0.52);
            float twinkle = 0.62 + 0.38 * sin(u_time * (1.8 + fract(h * 37.0) * 3.4) + h * 62.0);
            float star = smoothstep(0.28, 0.02, d) * twinkle;
            vec3 star_tint = (fract(h * 19.0) > 0.50)
                ? vec3(1.00, 0.91, 0.78)
                : vec3(0.84, 0.91, 1.00);
            col += star_tint * star * ((layer == 0) ? 0.95 : 0.68);
        }
    }

    // Subtle cosmic dust shimmer around the planet
    float nebula = fbm(p_cos * 2.4 + vec2(u_time * 0.02, -u_time * 0.015));
    col += palIrisMid() * pow(nebula, 3.0) * 0.18 * exp(-dot(p_cos, p_cos) * 1.2);

    // 2. 3D spinning banded gas giant + tilted rings (matching frames 26..38)
    float R_planet = 0.33;
    // Slight roll tilt (-17 deg) matching crop_26 & crop_33
    float roll = -0.30;
    float cr = cos(roll);
    float sr = sin(roll);
    vec2 pr = vec2(p_cos.x * cr - p_cos.y * sr, p_cos.x * sr + p_cos.y * cr);

    // Pitch tilt (~20 deg) so we see the rings wrapping cleanly around front & back
    float pitch = 0.34;
    float sin_p = sin(pitch);
    float cos_p = cos(pitch);

    // Ring intersection in tilted equatorial plane:
    // Orthographic ray from (pr.x, pr.y, +inf) toward -z hits equatorial plane
    // where y_world * cos_p + z_world * sin_p = 0 => z_ring = -pr.y * cos_p / sin_p
    float ring_z = -pr.y * (cos_p / max(sin_p, 0.08));
    float ring_y_plane = pr.y / max(sin_p, 0.08);
    float ring_r = length(vec2(pr.x, ring_y_plane));
    float r_norm = ring_r / R_planet;

    float ring_alpha = 0.0;
    vec3 ring_col = vec3(0.0);
    if (r_norm > 1.24 && r_norm < 2.08) {
        float outer_fade = smoothstep(2.08, 1.88, r_norm);
        float inner_fade = smoothstep(1.24, 1.38, r_norm);
        float cassini = 1.0 - 0.72 * exp(-pow((r_norm - 1.67) / 0.045, 2.0));
        float r_ang = atan(ring_y_plane, pr.x) - u_planet_rot * 0.55;
        float striations = 0.68 + 0.32 * sin(r_norm * 46.0)
                         + 0.14 * sin(r_ang * 6.0 + r_norm * 14.0);
        // Subtle planet shadow cast across the back-right arc of the ring
        float in_shadow = (ring_z < 0.0)
            ? smoothstep(R_planet * 0.65, R_planet * 1.15, abs(pr.x - 0.10))
            : 1.0;
        ring_alpha = clamp(inner_fade * outer_fade * cassini * striations * 0.66, 0.0, 0.78);
        vec3 base_ring = (u_palette == 0)
            ? mix(vec3(0.76, 0.54, 0.36), vec3(0.95, 0.84, 0.70), smoothstep(1.3, 1.85, r_norm))
            : mix(palIrisMid(), palIrisInner(), 0.55);
        ring_col = base_ring * (0.28 + 0.72 * in_shadow);
    }

    // Back half of ring (behind sphere, ring_z < 0)
    if (ring_alpha > 0.005 && ring_z < 0.0) {
        col = mix(col, ring_col, ring_alpha);
    }

    // Planet sphere intersection
    float r2 = dot(pr, pr);
    float R2 = R_planet * R_planet;
    if (r2 < R2) {
        float pz = sqrt(max(0.0, R2 - r2));
        vec3 n = normalize(vec3(pr.x, pr.y, pz));

        // Rotate normal by pitch into planet-local coordinates so bands tilt with the ring
        vec3 n_loc = vec3(
            n.x,
            n.y * cos_p + n.z * sin_p,
            -n.y * sin_p + n.z * cos_p
        );
        float lat = asin(clamp(n_loc.y, -1.0, 1.0));
        float lon = atan(n_loc.x, n_loc.z) + u_planet_rot;

        // Turbulent gas flow along latitude bands
        float turb = fbm(vec2(lon * 2.2, lat * 6.5)) - 0.5;
        vec3 albedo = palPlanetBand(lat, lon, turb);

        // Directional sunlight (upper-left-front) + warm atmospheric limb darkening
        vec3 light_dir = normalize(vec3(-0.36, -0.26, 0.90));
        float ndl = max(0.0, dot(n, light_dir));
        float limb = pow(clamp(n.z, 0.0, 1.0), 0.36);
        float shade = (0.14 + 0.94 * pow(ndl, 0.78)) * limb;

        // Subtle atmospheric rim glow
        float rim = pow(1.0 - clamp(n.z, 0.0, 1.0), 2.8) * max(0.25, ndl);
        vec3 planet_col = albedo * shade + palIrisInner() * rim * 0.22;

        // Anti-aliased sphere limb
        float edge_a = smoothstep(R_planet, R_planet * 0.94, sqrt(r2));
        col = mix(col, planet_col, edge_a);
    }

    // Front half of ring (in front of sphere, ring_z >= 0)
    if (ring_alpha > 0.005 && ring_z >= 0.0) {
        col = mix(col, ring_col, ring_alpha);
    }

    // Subtle cell twinkle glint
    float micro = hash21(cell_id + floor(u_time * 4.0) * 0.13);
    if (micro > 0.996 && length(p_cos) > R_planet * 1.15) {
        col += vec3(0.35);
    }

    return col;
}

// ---------------------------------------------------------------------------
// Complete unified eye + pupil-cosmos scene evaluator
// ---------------------------------------------------------------------------
vec3 evalScene(vec2 p, vec2 cell_id) {
    // Bold, commanding eye proportions matching the reference sequence
    float eye_hw = 1.12;
    float eye_hh = 0.46;
    float R_iris = 0.41;
    float R_pupil = 0.172;

    // Gaze offset (iris & pupil slide smoothly inside the almond opening)
    vec2 gaze = vec2(u_gaze.x * 0.33, u_gaze.y * 0.12);

    // Fast path when zoomed deep inside the pupil cosmos
    vec2 dp = p - gaze;
    float dist_pupil_c = length(dp);
    float inner_zoom_bypass = step(6.5, u_zoom) * step(dist_pupil_c, R_pupil * 0.96);
    if (inner_zoom_bypass > 0.5) {
        return renderCosmos(dp / R_pupil, cell_id);
    }

    // Eyelid almond curves (y down: upper lid is negative y, lower lid is positive y)
    float u = clamp(p.x / eye_hw, -1.0, 1.0);
    float env = pow(max(0.0, 1.0 - u * u), 0.72);
    float ua = 1.0 - 0.08 * u - 0.04 * u * u;
    float la = 1.0 + 0.07 * u - 0.04 * u * u;
    float medial_dip = 0.06 * eye_hh * exp(-pow((u + 0.90) / 0.18, 2.0));
    float lateral_lift = -0.04 * eye_hh * exp(-pow((u - 0.88) / 0.22, 2.0));
    float corner_line = medial_dip + lateral_lift;

    float yu_open = -eye_hh * 1.06 * env * ua + corner_line;
    float yl_open =  eye_hh * 0.86 * env * la + corner_line;
    float y_closed = eye_hh * 0.18 * env + corner_line;

    float lid_follow_y = gaze.y * 0.22 * u_openness;
    float yu = mix(y_closed, yu_open, u_openness) + lid_follow_y;
    float yl = mix(y_closed, yl_open, u_openness) + lid_follow_y;

    // Inside almond eye opening mask
    float x_in = 1.0 - smoothstep(eye_hw * 0.96, eye_hw * 1.01, abs(p.x));
    float top_in = smoothstep(yu - 0.016, yu + 0.016, p.y);
    float bot_in = 1.0 - smoothstep(yl - 0.016, yl + 0.016, p.y);
    float eye_mask = x_in * top_in * bot_in * smoothstep(0.02, 0.08, u_openness);

    // -----------------------------------------------------------------------
    // A. Skin, eyelids, crease & eyebrow (outer face surrounding the eye)
    // -----------------------------------------------------------------------
    float skin_grain = fbm(p * 4.4 + vec2(3.1, 1.7));
    float socket_d = length(vec2(p.x * 0.56, (p.y + 0.02) * 1.05));
    float skin_lit = clamp(0.64 - socket_d * 0.24 + (skin_grain - 0.5) * 0.20, 0.14, 0.88);

    // Subtle cheek/brow bone lighting
    skin_lit += 0.15 * exp(-pow((p.y - 0.56) / 0.36, 2.0)) * exp(-pow(p.x / 1.45, 2.0));
    vec3 skin_col = mix(palSkinDark(), palSkinLight(), skin_lit);

    // Upper eyelid crease fold above the opening
    float crease_y = yu - (0.13 + 0.05 * (1.0 - u_openness)) * env - 0.025;
    float crease = exp(-pow((p.y - crease_y) / 0.048, 2.0)) * smoothstep(eye_hw * 1.16, eye_hw * 0.74, abs(p.x));
    skin_col *= (1.0 - 0.50 * crease);

    // Lower lid soft fold shadow
    float lower_fold_y = yl + 0.13 * env + 0.025;
    float lower_fold = exp(-pow((p.y - lower_fold_y) / 0.062, 2.0)) * smoothstep(eye_hw * 1.12, eye_hw * 0.65, abs(p.x));
    skin_col *= (1.0 - 0.26 * lower_fold);

    // Dark textured eyebrow arch across the upper region (matching crop_08 & crop_13)
    float brow_u = clamp(p.x / 1.32, -1.0, 1.0);
    float brow_y = -0.76 - 0.11 * cos(brow_u * PI * 0.60) + 0.06 * brow_u;
    float brow_thick = 0.105 * (1.0 - 0.32 * brow_u * brow_u) * smoothstep(1.02, 0.74, abs(brow_u));
    float brow_d = abs(p.y - brow_y) / max(brow_thick, 0.001);
    float brow_hair = fbm(vec2(p.x * 22.0, p.y * 12.0));
    float brow_m = smoothstep(1.25, 0.32, brow_d) * smoothstep(1.02, 0.72, abs(brow_u)) * (0.68 + 0.32 * brow_hair);
    skin_col *= (1.0 - 0.80 * brow_m);

    // Dark upper lash line & closed-lid seam
    float lash_d = abs(p.y - yu);
    float lash_band = exp(-pow(lash_d / 0.044, 2.0)) * smoothstep(eye_hw * 1.07, eye_hw * 0.82, abs(p.x));
    skin_col *= (1.0 - 0.78 * lash_band);

    // Lower lid rim darkening
    float low_rim = exp(-pow(abs(p.y - yl) / 0.028, 2.0)) * smoothstep(eye_hw * 1.04, eye_hw * 0.80, abs(p.x));
    skin_col *= (1.0 - 0.45 * low_rim);

    // -----------------------------------------------------------------------
    // B. Eyeball interior: sclera + golden radial iris + black starry pupil
    // -----------------------------------------------------------------------
    vec3 eye_col = vec3(0.0);
    if (eye_mask > 0.002) {
        // Sclera with spherical 3D shading + upper lid cast shadow
        float s_rad = length(vec2(p.x / (eye_hw * 1.08), p.y / (eye_hh * 1.45)));
        float s_shade = clamp(1.04 - s_rad * s_rad * 0.52, 0.38, 1.0);
        // Upper eyelid casts a soft dark shadow onto the top of the eyeball
        float lid_shadow = smoothstep(0.0, 0.16, p.y - yu);
        float bot_shadow = smoothstep(0.0, 0.07, yl - p.y);
        vec3 sclera = vec3(0.96, 0.97, 0.99) * s_shade * (0.40 + 0.60 * lid_shadow) * (0.68 + 0.32 * bot_shadow);

        // Subtle warm inner caruncle at medial corner
        float caruncle = smoothstep(0.24, 0.02, length(p - vec2(-eye_hw * 0.86, corner_line)));
        sclera = mix(sclera, palSkinLight() * 0.82, caruncle * 0.52);

        eye_col = sclera;

        // Iris (centered at gaze)
        if (dist_pupil_c < R_iris * 1.06) {
            float r_frac = dist_pupil_c / R_iris;
            float ang = atan(dp.y, dp.x);

            // Multi-frequency radial iris stroma fibers (matching crop_05 & crop_22)
            float f1 = sin(ang * 28.0 + fbm(vec2(ang * 3.0, r_frac * 5.0)) * 5.5);
            float f2 = sin(ang * 64.0 - r_frac * 9.0 + fbm(dp * 16.0) * 3.0);
            float fibers = 0.50 + 0.30 * f1 + 0.20 * f2;

            // Golden inner collarette ring around the pupil
            float collarette = exp(-pow((r_frac - 0.52) / 0.18, 2.0));
            // Dark outer limbal ring
            float limbal = smoothstep(0.75, 0.98, r_frac);

            vec3 iris_c = mix(palIrisMid(), palIrisInner(), clamp(collarette * 0.90 + fibers * 0.44, 0.0, 1.0));
            iris_c = mix(iris_c, palIrisOuter(), limbal * 0.90);

            // Bright radial spokes radiating out from the pupil
            float spokes = smoothstep(0.40, 0.92, fibers) * (1.0 - limbal * 0.72);
            iris_c += palIrisInner() * spokes * 0.28;

            // Upper lid shadow softens across the iris (fades out as we zoom deep in)
            float iris_lid_sh = mix(0.48 + 0.52 * lid_shadow, 1.0, smoothstep(1.6, 3.2, u_zoom));
            iris_c *= iris_lid_sh;

            // Corneal specular glint when viewing the eye at wide/mid zoom
            float glint_fade = 1.0 - smoothstep(1.6, 3.0, u_zoom);
            vec2 glint_pos = gaze + vec2(-R_iris * 0.36, -R_iris * 0.34);
            float glint = exp(-dot(p - glint_pos, p - glint_pos) / (0.0034)) * glint_fade;
            iris_c += vec3(0.98, 0.99, 1.0) * glint * 0.88;

            float iris_edge = 1.0 - smoothstep(0.95, 1.04, r_frac);
            eye_col = mix(eye_col, iris_c, iris_edge);

            // Pupil + cosmic universe inside!
            if (dist_pupil_c < R_pupil * 1.08) {
                vec3 cosmos = renderCosmos(dp / R_pupil, cell_id);
                float pupil_m = 1.0 - smoothstep(R_pupil * 0.92, R_pupil * 1.05, dist_pupil_c);
                eye_col = mix(eye_col, cosmos, pupil_m);
            }
        }
    }

    return mix(skin_col, eye_col, eye_mask);
}

void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);

    // Quantize to monospace ASCII character cell
    vec2 cell_id = floor(px / u_cell_px);
    vec2 cell_uv = fract(px / u_cell_px);
    vec2 cell_center = (cell_id + 0.5) * u_cell_px;

    float scale_ref = min(u_res.x, u_res.y) * 0.50;
    vec2 p_screen = (cell_center - 0.5 * u_res) / max(scale_ref, 1.0);
    vec2 p_world = p_screen / max(u_zoom, 0.25);

    vec3 scene_rgb = evalScene(p_world, cell_id);

    // Adaptive subtle dark backing behind desktop icons so icons stay crisp
    if (u_adaptive == 1) {
        for (int i = 0; i < 24; i++) {
            if (i >= u_icon_count) { break; }
            vec4 ir = u_icons[i];
            vec2 half_ext = ir.zw * 0.5 + vec2(10.0);
            vec2 center = ir.xy + ir.zw * 0.5;
            float d = sdRoundBox(cell_center - center, half_ext, 10.0);
            float scrim = 1.0 - smoothstep(-2.0, 4.0, d);
            scene_rgb *= (1.0 - scrim * 0.58);
        }
    }

    // Perceptual luminance -> 64-glyph monospace ASCII index
    float lum = dot(scene_rgb, vec3(0.299, 0.587, 0.114));
    // Slight deterministic per-cell variation for organic terminal texture
    float cell_jit = (hash21(cell_id) - 0.5) * 0.035;
    float mapped = clamp(pow(max(0.0, lum), 0.86) + cell_jit, 0.0, 1.0);

    // Keep pure black void as empty space (index 0)
    float glyph_idx = (lum < 0.018) ? 0.0 : clamp(floor(mapped * (GLYPH_COUNT - 1.0) + 0.5), 1.0, GLYPH_COUNT - 1.0);

    // Sample padded glyph tile cleanly without bleeding into neighboring glyphs
    float u_tile = (glyph_idx * TILE_W + 1.0 + cell_uv.x * (TILE_W - 2.0)) / (GLYPH_COUNT * TILE_W);
    float v_tile = (1.0 + cell_uv.y * (TILE_H - 2.0)) / TILE_H;
    float glyph_cov = texture(u_atlas, vec2(u_tile, v_tile)).r;

    // Normalize color intensity so dim ASCII characters remain rich in hue
    vec3 char_col = scene_rgb * 1.28;
    vec3 out_rgb = char_col * glyph_cov;

    // Subtle CRT/terminal vignette at far screen corners
    vec2 vig_p = v_uv - 0.5;
    float vig = 1.0 - dot(vig_p, vig_p) * 0.35;
    out_rgb *= clamp(vig, 0.65, 1.0);

    fragColor = vec4(clamp(out_rgb, 0.0, 1.0), 1.0);
}
"#;
