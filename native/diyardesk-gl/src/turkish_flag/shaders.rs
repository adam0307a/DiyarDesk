//! GLSL 3.30 Core shaders for `turkish-flag`:
//!   1. `SKY_AND_POLE_FRAG`: Daylight / Sunset / Starry-Night sky + drifting
//!      multi-layer volumetric clouds + subtle distant Anatolian ridge +
//!      3D metallic flagpole with golden finial (alem) & halyard rope +
//!      adaptive desktop icon readability scrims.
//!   2. `FLAG_VERT` & `FLAG_FRAG`: 3D waving cloth mesh with exact Law No. 2893
//!      Turkish Flag SDF (white uçkurluk, crimson field, crescent & 5-pointed
//!      star pointing left into the crescent), woven micro-fabric texture,
//!      subsurface sunlight translucency, and alpha-preserving shading.

pub const SKY_AND_POLE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_res;            // physical px
uniform float u_time;          // animation time (s)
uniform int u_sky_mode;        // 0=daylight, 1=sunset, 2=night, 3=auto
uniform float u_env_hour;      // local clock hour 0..24
uniform float u_clouds;        // 0..1.5
uniform float u_atmosphere;    // 0..1.5
uniform float u_wind;          // 0.2..2.0
uniform int u_adaptive;        // 0/1
uniform int u_icon_count;      // 0..24
uniform vec4 u_icons[24];      // physical px (x, y, w, h)
uniform vec4 u_pole_top_bot;   // xy = pole finial top NDC, zw = pole base NDC
uniform vec4 u_flag_hoist_y;   // x = flag top NDC.x, y = flag top NDC.y, z = flag bot NDC.x, w = flag bot NDC.y
uniform float u_pole_rad_px;   // pole half-width in physical px at top

const float PI = 3.14159265359;

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
    float a = 0.5;
    mat2 m = mat2(0.80, -0.60, 0.60, 0.80);
    for (int i = 0; i < 5; i++) {
        v += a * vnoise(p);
        p = m * p * 2.03 + vec2(11.3, 7.7);
        a *= 0.5;
    }
    return v;
}

float fbm3(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    mat2 m = mat2(0.80, -0.60, 0.60, 0.80);
    for (int i = 0; i < 3; i++) {
        v += a * vnoise(p);
        p = m * p * 2.07 + vec2(5.1, 13.4);
        a *= 0.5;
    }
    return v;
}

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

vec3 computeSkyWeights() {
    if (u_sky_mode == 0) return vec3(1.0, 0.0, 0.0);
    if (u_sky_mode == 1) return vec3(0.0, 1.0, 0.0);
    if (u_sky_mode == 2) return vec3(0.0, 0.0, 1.0);

    // Auto mode from local clock hour
    float h = mod(u_env_hour, 24.0);
    float day_w = smoothstep(5.8, 8.2, h) * (1.0 - smoothstep(16.8, 18.8, h));
    float sun_morn = smoothstep(4.8, 6.2, h) * (1.0 - smoothstep(7.2, 8.8, h));
    float sun_eve = smoothstep(16.2, 18.0, h) * (1.0 - smoothstep(19.6, 21.2, h));
    float sun_w = clamp(sun_morn + sun_eve, 0.0, 1.0);
    float night_w = max(0.0, 1.0 - day_w - sun_w);
    float sum_w = max(day_w + sun_w + night_w, 0.001);
    return vec3(day_w, sun_w, night_w) / sum_w;
}

vec3 renderSky(vec2 asp_p, float y01, vec3 w) {
    float t = u_time;

    // 1. Daylight sky
    vec3 day_zen = vec3(0.075, 0.290, 0.640);
    vec3 day_mid = vec3(0.245, 0.560, 0.870);
    vec3 day_hor = vec3(0.680, 0.855, 0.965);
    vec3 col_day = mix(day_hor, day_mid, smoothstep(0.0, 0.42, y01));
    col_day = mix(col_day, day_zen, smoothstep(0.36, 0.96, y01));

    // Sun in upper-right sky
    vec2 sun_pos_day = vec2(0.56, 0.42);
    float d_sun_day = length(asp_p - sun_pos_day);
    float sun_halo_day = exp(-d_sun_day * 3.4) * 0.42 + exp(-d_sun_day * 10.0) * 0.48;
    float sun_disc_day = smoothstep(0.055, 0.025, d_sun_day);
    col_day += vec3(1.00, 0.96, 0.86) * sun_halo_day * u_atmosphere;
    col_day = mix(col_day, vec3(1.00, 0.99, 0.94), sun_disc_day);

    // 2. Golden / crimson sunset sky
    vec3 sun_zen = vec3(0.085, 0.105, 0.275);
    vec3 sun_mid = vec3(0.720, 0.255, 0.245);
    vec3 sun_hor = vec3(1.000, 0.680, 0.310);
    vec3 col_sun = mix(sun_hor, sun_mid, smoothstep(0.02, 0.46, y01));
    col_sun = mix(col_sun, sun_zen, smoothstep(0.42, 0.95, y01));

    vec2 sun_pos_eve = vec2(0.56, -0.14);
    float d_sun_eve = length(asp_p - sun_pos_eve);
    float eve_glow = exp(-d_sun_eve * 2.1) * 0.68 + exp(-d_sun_eve * 6.5) * 0.65;
    float eve_disc = smoothstep(0.064, 0.026, d_sun_eve);
    col_sun += vec3(1.00, 0.72, 0.34) * eve_glow * u_atmosphere;
    col_sun = mix(col_sun, vec3(1.00, 0.96, 0.80), eve_disc);
    // Warm horizon band
    col_sun += vec3(0.95, 0.46, 0.18) * exp(-pow((y01 - 0.08) / 0.14, 2.0)) * 0.30 * u_atmosphere;

    // 3. Starry night sky
    vec3 n_zen = vec3(0.014, 0.028, 0.075);
    vec3 n_mid = vec3(0.028, 0.062, 0.140);
    vec3 n_hor = vec3(0.068, 0.130, 0.235);
    vec3 col_night = mix(n_hor, n_mid, smoothstep(0.0, 0.40, y01));
    col_night = mix(col_night, n_zen, smoothstep(0.36, 0.95, y01));

    if (w.z > 0.01) {
        // Subtle Milky Way diagonal shimmer
        float mw_band = exp(-pow((asp_p.y - asp_p.x * 0.55 - 0.14) / 0.28, 2.0));
        float mw_dust = fbm3(asp_p * 3.8 + vec2(1.7, 3.2));
        col_night += vec3(0.09, 0.14, 0.24) * mw_band * mw_dust * 0.55 * u_atmosphere;

        // Multi-layer twinkling stars
        for (int layer = 0; layer < 3; layer++) {
            float scale = (layer == 0) ? 26.0 : ((layer == 1) ? 48.0 : 82.0);
            vec2 sp = asp_p * scale + vec2(float(layer) * 23.1, float(layer) * 11.7);
            vec2 sid = floor(sp);
            vec2 sf = fract(sp) - 0.5;
            float h = hash21(sid + float(layer) * 41.3);
            float thresh = (layer == 0) ? 0.955 : ((layer == 1) ? 0.945 : 0.935);
            if (h > thresh) {
                vec2 jit = vec2(hash21(sid + 4.3), hash21(sid + 9.1)) - 0.5;
                float d = length(sf - jit * 0.56);
                float tw = 0.58 + 0.42 * sin(t * (1.4 + fract(h * 29.0) * 3.2) + h * 50.0);
                float sz = (layer == 0) ? 0.24 : 0.18;
                float s = smoothstep(sz, 0.01, d) * tw;
                vec3 tint = mix(vec3(0.80, 0.90, 1.00), vec3(1.00, 0.92, 0.78), fract(h * 17.0));
                float fade_hor = smoothstep(0.05, 0.35, y01);
                col_night += tint * s * ((layer == 0) ? 1.05 : 0.72) * fade_hor;
            }
        }

        // Periodic delicate shooting star
        float shoot_cyc = mod(t * 0.22, 1.0);
        if (shoot_cyc < 0.22) {
            float p_s = shoot_cyc / 0.22;
            vec2 start_s = vec2(-0.55 + floor(t * 0.22) * 0.37, 0.38);
            start_s.x = mod(start_s.x + 0.6, 1.1) - 0.45;
            vec2 dir_s = normalize(vec2(0.88, -0.34));
            vec2 head_s = start_s + dir_s * (p_s * 0.55);
            vec2 rel = asp_p - head_s;
            float along = dot(rel, -dir_s);
            float perp = abs(dot(rel, vec2(-dir_s.y, dir_s.x)));
            float tail = smoothstep(0.16, 0.0, along) * step(0.0, along);
            float streak = exp(-perp * 420.0) * tail * sin(p_s * PI);
            col_night += vec3(0.92, 0.96, 1.00) * streak * 1.1;
        }

        // Luminous full/gibbous moon in upper right
        vec2 moon_p = vec2(0.58, 0.36);
        float d_moon = length(asp_p - moon_p);
        float moon_halo = exp(-d_moon * 4.5) * 0.26 + exp(-d_moon * 14.0) * 0.32;
        float moon_disc = smoothstep(0.044, 0.039, d_moon);
        float moon_tex = 0.88 + 0.12 * fbm3((asp_p - moon_p) * 32.0);
        col_night += vec3(0.62, 0.76, 0.96) * moon_halo * u_atmosphere;
        col_night = mix(col_night, vec3(0.95, 0.97, 1.00) * moon_tex, moon_disc);
    }

    vec3 sky = col_day * w.x + col_sun * w.y + col_night * w.z;

    // 4. Drifting layered clouds
    if (u_clouds > 0.01) {
        float c_amt = clamp(u_clouds, 0.0, 1.5);
        float drift = t * (0.018 + 0.020 * u_wind);

        // Layer A: high wispy cirrus streaks
        vec2 uv_cir = vec2(asp_p.x * 1.5 - drift * 0.65, asp_p.y * 4.8);
        float cir_n = fbm(uv_cir + vec2(fbm3(uv_cir * 1.4), 0.0) * 0.45);
        float cir_m = smoothstep(0.52, 0.80, cir_n) * smoothstep(0.18, 0.65, y01) * smoothstep(0.98, 0.72, y01);

        // Layer B: soft mid-altitude cumulus billows
        vec2 uv_cum = vec2(asp_p.x * 2.1 - drift * 1.15, asp_p.y * 3.4 - 0.35);
        float cum_n = fbm(uv_cum);
        float cum_lit_n = fbm(uv_cum - vec2(0.045, 0.045));
        float cum_m = smoothstep(0.49 - 0.08 * (c_amt - 0.85), 0.76, cum_n)
                    * smoothstep(0.08, 0.34, y01) * smoothstep(0.90, 0.52, y01);
        float ridge = clamp((cum_n - cum_lit_n) * 4.0 + 0.55, 0.0, 1.0);

        // Layer C: fast foreground wispy puffs
        vec2 uv_fg = vec2(asp_p.x * 1.4 - drift * 1.75 + 4.2, asp_p.y * 2.8 + 1.1);
        float fg_n = fbm(uv_fg);
        float fg_m = smoothstep(0.56 - 0.06 * (c_amt - 0.85), 0.82, fg_n)
                   * smoothstep(0.04, 0.28, y01) * smoothstep(0.78, 0.42, y01);

        vec3 cir_col = vec3(0.96, 0.98, 1.00) * w.x
                     + vec3(1.00, 0.74, 0.56) * w.y
                     + vec3(0.30, 0.38, 0.56) * w.z;

        vec3 cum_hi = vec3(1.00, 1.00, 1.00) * w.x
                    + vec3(1.00, 0.80, 0.48) * w.y
                    + vec3(0.38, 0.48, 0.66) * w.z;
        vec3 cum_lo = vec3(0.62, 0.74, 0.88) * w.x
                    + vec3(0.46, 0.24, 0.34) * w.y
                    + vec3(0.08, 0.12, 0.22) * w.z;
        vec3 cum_col = mix(cum_lo, cum_hi, ridge);

        // Keep clouds softly lit near the setting sun so they never cast a dark grey smudge over the sun disc
        float sun_clear_eve = exp(-length(asp_p - vec2(0.56, -0.14)) * 4.8) * w.y;
        cum_col = mix(cum_col, vec3(1.00, 0.88, 0.64), sun_clear_eve * 0.85);

        float alpha_cir = clamp(cir_m * 0.36 * c_amt, 0.0, 0.65);
        float alpha_cum = clamp(cum_m * 0.76 * c_amt * (1.0 - sun_clear_eve * 0.55), 0.0, 0.88);
        float alpha_fg  = clamp(fg_m * 0.48 * c_amt * (1.0 - sun_clear_eve * 0.60), 0.0, 0.70);

        sky = mix(sky, cir_col, alpha_cir);
        sky = mix(sky, cum_col, alpha_cum);
        sky = mix(sky, mix(cum_lo, cum_hi, 0.72), alpha_fg);
    }

    // 5. Distant atmospheric Anatolian mountain ridge along very bottom (~9%)
    float h_mtn = 0.042
                + 0.022 * sin(asp_p.x * 2.6 + 0.7)
                + 0.015 * sin(asp_p.x * 6.3 - 1.2)
                + 0.014 * fbm3(vec2(asp_p.x * 5.0, 2.0));
    float mtn_mask = 1.0 - smoothstep(h_mtn - 0.004, h_mtn + 0.004, y01);
    vec3 mtn_col = vec3(0.24, 0.36, 0.52) * w.x
                 + vec3(0.18, 0.10, 0.18) * w.y
                 + vec3(0.03, 0.05, 0.10) * w.z;
    sky = mix(sky, mtn_col, mtn_mask * 0.88);

    // Subtle atmospheric horizon mist
    float mist = exp(-y01 * 9.0) * 0.24 * u_atmosphere;
    vec3 mist_col = vec3(0.82, 0.91, 0.98) * w.x
                  + vec3(0.98, 0.64, 0.38) * w.y
                  + vec3(0.10, 0.16, 0.28) * w.z;
    sky = mix(sky, mist_col, clamp(mist, 0.0, 0.45));

    return sky;
}

void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float min_dim = max(min(u_res.x, u_res.y), 1.0);
    // Aspect-corrected centered coordinates: y up (+1 at top, -1 at bottom)
    vec2 ndc = v_uv * 2.0 - 1.0;
    float aspect = u_res.x / max(u_res.y, 1.0);
    vec2 asp_p = vec2(ndc.x * aspect * 0.5, ndc.y * 0.5);
    float y01 = v_uv.y;

    vec3 w = computeSkyWeights();
    vec3 col = renderSky(asp_p, y01, w);

    // Light direction matching sky mode for pole specular highlights
    vec3 sun_dir = normalize(
        vec3(0.65, 0.55, 0.52) * w.x +
        vec3(0.75, 0.16, 0.64) * w.y +
        vec3(0.60, 0.48, 0.64) * w.z
    );

    // -----------------------------------------------------------------------
    // 3D Cylindrical Flagpole + Golden Alem (Finial) + Halyard Rope
    // Projected in exact pixel space from the same camera matrix as the flag!
    // -----------------------------------------------------------------------
    vec2 p_top_px = vec2((u_pole_top_bot.x * 0.5 + 0.5) * u_res.x,
                         (0.5 - u_pole_top_bot.y * 0.5) * u_res.y);
    vec2 p_bot_px = vec2((u_pole_top_bot.z * 0.5 + 0.5) * u_res.x,
                         (0.5 - u_pole_top_bot.w * 0.5) * u_res.y);
    vec2 f_top_px = vec2((u_flag_hoist_y.x * 0.5 + 0.5) * u_res.x,
                         (0.5 - u_flag_hoist_y.y * 0.5) * u_res.y);
    vec2 f_bot_px = vec2((u_flag_hoist_y.z * 0.5 + 0.5) * u_res.x,
                         (0.5 - u_flag_hoist_y.w * 0.5) * u_res.y);

    vec2 seg = p_bot_px - p_top_px;
    float seg_len2 = max(dot(seg, seg), 1.0);
    float seg_len = sqrt(seg_len2);
    vec2 axis_dir = seg / seg_len;
    vec2 perp_dir = vec2(-axis_dir.y, axis_dir.x);

    float t_proj = dot(px - p_top_px, axis_dir);
    float x_perp = dot(px - p_top_px, perp_dir);
    float t_norm = clamp(t_proj / seg_len, 0.0, 1.0);

    // Slight natural taper from base to finial
    float r_pole = max(u_pole_rad_px, 2.5) * mix(0.95, 1.38, t_norm);

    if (t_proj >= 0.0 && abs(x_perp) <= r_pole + 1.5) {
        float edge_a = 1.0 - smoothstep(r_pole - 0.8, r_pole + 0.8, abs(x_perp));
        float nx = clamp(x_perp / max(r_pole, 0.5), -1.0, 1.0);
        float nz = sqrt(max(0.0, 1.0 - nx * nx));
        vec3 n_pole = normalize(vec3(nx, 0.06, nz));

        float ndl = max(0.0, dot(n_pole, sun_dir));
        vec3 view_dir = vec3(0.0, 0.0, 1.0);
        vec3 half_v = normalize(sun_dir + view_dir);
        float spec = pow(max(0.0, dot(n_pole, half_v)), 36.0);

        vec3 metal_base = vec3(0.46, 0.50, 0.56) * w.x
                        + vec3(0.52, 0.42, 0.40) * w.y
                        + vec3(0.24, 0.28, 0.36) * w.z;
        vec3 sun_tint = vec3(1.00, 0.98, 0.94) * w.x
                      + vec3(1.00, 0.72, 0.42) * w.y
                      + vec3(0.76, 0.86, 1.00) * w.z;

        // Subtle crimson bounce light along the flag hoist section
        float flag_span = smoothstep(f_top_px.y - 12.0, f_top_px.y + 16.0, px.y)
                        * (1.0 - smoothstep(f_bot_px.y - 16.0, f_bot_px.y + 12.0, px.y));
        vec3 metal_col = metal_base * (0.36 + 0.72 * ndl)
                       + sun_tint * spec * 0.85
                       + vec3(0.24, 0.02, 0.03) * flag_span * max(0.0, nx) * 0.55;

        // Decorative collar rings near top and hoist attachments
        float collar = exp(-pow((px.y - (p_top_px.y + r_pole * 1.4)) / 2.6, 2.0))
                     + exp(-pow((px.y - f_top_px.y) / 3.0, 2.0))
                     + exp(-pow((px.y - f_bot_px.y) / 3.0, 2.0));
        metal_col = mix(metal_col, vec3(0.84, 0.68, 0.28) * (0.45 + 0.65 * ndl), clamp(collar, 0.0, 1.0) * 0.75);

        col = mix(col, metal_col, edge_a);
    }

    // Golden spherical finial (Alem) + neck spire at top of pole
    float r_alem = max(u_pole_rad_px, 2.5) * 2.35;
    vec2 alem_c = p_top_px - axis_dir * (r_alem * 0.88);
    float d_alem = length(px - alem_c);
    if (d_alem <= r_alem + 1.5) {
        float a_alem = 1.0 - smoothstep(r_alem - 0.8, r_alem + 0.8, d_alem);
        vec2 n2 = (px - alem_c) / max(r_alem, 1.0);
        float nz = sqrt(max(0.0, 1.0 - dot(n2, n2)));
        vec3 n_sphere = normalize(vec3(n2.x, -n2.y, nz));
        float ndl = max(0.0, dot(n_sphere, sun_dir));
        vec3 half_v = normalize(sun_dir + vec3(0.0, 0.0, 1.0));
        float spec = pow(max(0.0, dot(n_sphere, half_v)), 44.0);
        float fres = pow(1.0 - nz, 2.5);

        vec3 gold_dark = vec3(0.42, 0.27, 0.06);
        vec3 gold_mid  = vec3(0.95, 0.75, 0.24);
        vec3 gold_hi   = vec3(1.00, 0.96, 0.78);
        vec3 gold_col = mix(gold_dark, gold_mid, 0.28 + 0.72 * ndl)
                      + gold_hi * spec * 0.95
                      + vec3(0.88, 0.68, 0.30) * fres * (w.z * 0.3 + 0.25);
        col = mix(col, gold_col, a_alem);
    }

    // Subtle halyard rope running along the left side of the pole & swaying slightly in wind
    if (px.y > p_top_px.y + 4.0) {
        float sway = sin(u_time * 3.1 + px.y * 0.03) * (1.2 + u_wind * 1.4);
        float rope_x = p_top_px.x - r_pole * 1.35 + sway * smoothstep(f_top_px.y, f_bot_px.y + 180.0, px.y);
        float d_rope = abs(px.x - rope_x);
        float a_rope = 1.0 - smoothstep(0.6, 1.6, d_rope);
        vec3 rope_col = vec3(0.84, 0.86, 0.88) * w.x
                      + vec3(0.76, 0.56, 0.44) * w.y
                      + vec3(0.32, 0.38, 0.48) * w.z;
        col = mix(col, rope_col, a_rope * 0.82);
    }

    // -----------------------------------------------------------------------
    // Adaptive desktop icon contrast scrims (keeps labels legible on bright clouds)
    // -----------------------------------------------------------------------
    if (u_adaptive == 1) {
        for (int i = 0; i < 24; i++) {
            if (i >= u_icon_count) { break; }
            vec4 ir = u_icons[i];
            vec2 half_ext = ir.zw * 0.5 + vec2(10.0);
            vec2 center = ir.xy + ir.zw * 0.5;
            float d = sdRoundBox(px - center, half_ext, 10.0);
            float scrim = 1.0 - smoothstep(-2.0, 10.0, d);
            vec3 tint = mix(vec3(0.06, 0.14, 0.26), vec3(0.02, 0.04, 0.09), w.z);
            float strength = mix(0.14, 0.24, w.z);
            col = mix(col, tint, scrim * strength);
        }
    }

    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

pub const FLAG_VERT: &str = r#"#version 330 core
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_norm;
layout(location = 2) in vec2 a_uv;

uniform mat4 u_proj_view;

out vec3 v_world_pos;
out vec3 v_norm;
out vec2 v_uv;

void main() {
    v_world_pos = a_pos;
    v_norm = a_norm;
    v_uv = a_uv;
    gl_Position = u_proj_view * vec4(a_pos, 1.0);
}
"#;

pub const FLAG_FRAG: &str = r#"#version 330 core
in vec3 v_world_pos;
in vec3 v_norm;
in vec2 v_uv;

out vec4 fragColor;

uniform float u_time;
uniform int u_sky_mode;     // 0=daylight, 1=sunset, 2=night, 3=auto
uniform float u_env_hour;
uniform float u_atmosphere;
uniform int u_fabric;       // 0=silk, 1=cotton, 2=ceremonial
uniform vec3 u_cam_pos;

// Law No. 2893 Turkish Flag total aspect: (1/30 + 1.5) : 1.0
const float UCKURLUK_W = 0.03333333;
const float TOTAL_LEN  = 1.53333333;

float hash21(vec2 p) {
    p = fract(p * vec2(127.1, 311.7));
    p += dot(p, p + 34.5);
    return fract(p.x * p.y);
}

// Exact signed distance to a regular 5-pointed star with radius `r`,
// inner-to-outer ratio `rf`, and one tip pointing along -X (into the crescent).
float sdStar5Left(vec2 q, float r, float rf) {
    // Rotate +Y tip to -X: (x, y) -> (y, -x)
    vec2 p = vec2(q.y, -q.x);
    const vec2 k1 = vec2(0.809016994375, -0.587785252292);
    const vec2 k2 = vec2(-k1.x, k1.y);
    p.x = abs(p.x);
    p -= 2.0 * max(dot(k1, p), 0.0) * k1;
    p -= 2.0 * max(dot(k2, p), 0.0) * k2;
    p.x = abs(p.x);
    p.y -= r;
    vec2 ba = rf * vec2(-k1.y, k1.x) - vec2(0.0, 1.0);
    float h = clamp(dot(p, ba) / dot(ba, ba), 0.0, r);
    return length(p - ba * h) * sign(p.y * ba.x - p.x * ba.y);
}

vec3 computeSkyWeights() {
    if (u_sky_mode == 0) return vec3(1.0, 0.0, 0.0);
    if (u_sky_mode == 1) return vec3(0.0, 1.0, 0.0);
    if (u_sky_mode == 2) return vec3(0.0, 0.0, 1.0);
    float h = mod(u_env_hour, 24.0);
    float day_w = smoothstep(5.8, 8.2, h) * (1.0 - smoothstep(16.8, 18.8, h));
    float sun_morn = smoothstep(4.8, 6.2, h) * (1.0 - smoothstep(7.2, 8.8, h));
    float sun_eve = smoothstep(16.2, 18.0, h) * (1.0 - smoothstep(19.6, 21.2, h));
    float sun_w = clamp(sun_morn + sun_eve, 0.0, 1.0);
    float night_w = max(0.0, 1.0 - day_w - sun_w);
    float sum_w = max(day_w + sun_w + night_w, 0.001);
    return vec3(day_w, sun_w, night_w) / sum_w;
}

void main() {
    // 1. Official Law No. 2893 Vector SDF Flag Pattern
    float x_tot = v_uv.x * TOTAL_LEN;
    float y = v_uv.y;

    float aa_edge = max(fwidth(x_tot), 0.0015);
    float hoist_mask = 1.0 - smoothstep(UCKURLUK_W - aa_edge, UCKURLUK_W + aa_edge, x_tot);

    // Coordinates inside the 1.5 x 1.0 crimson field
    vec2 p = vec2(x_tot - UCKURLUK_W, y);

    // Crescent: outer circle A=(0.5, 0.5) r=0.25, inner cutout B=(0.5625, 0.5) r=0.20
    float d_outer = length(p - vec2(0.5000, 0.5)) - 0.2500;
    float d_inner = length(p - vec2(0.5625, 0.5)) - 0.2000;
    float d_crescent = max(d_outer, -d_inner);

    // 5-pointed star: center S=(0.8208333, 0.5), outer R=0.125, pentagram rf=0.38196601
    float d_star = sdStar5Left(p - vec2(0.82083333, 0.5), 0.1250, 0.381966011);

    float d_emblem = min(d_crescent, d_star);
    float aa_sym = clamp(max(fwidth(d_emblem), 0.0010), 0.0008, 0.008);
    float emblem_mask = 1.0 - smoothstep(-aa_sym, aa_sym, d_emblem);

    float white_mask = clamp(max(hoist_mask, emblem_mask), 0.0, 1.0);

    // Official Pantone-calibrated Turkish Flag Crimson (#E30A17) & clean cloth white
    vec3 crimson = vec3(0.890, 0.039, 0.090);
    vec3 white_col = vec3(0.965, 0.968, 0.978);
    vec3 albedo = mix(crimson, white_col, white_mask);

    // Subtle hoist sleeve stitch seam & edge hem detail
    float seam = exp(-pow((x_tot - (UCKURLUK_W - 0.006)) / 0.0022, 2.0));
    float hem = exp(-pow(min(y, 1.0 - y) / 0.0055, 2.0))
              + exp(-pow((1.5 - p.x) / 0.006, 2.0));
    albedo *= (1.0 - seam * 0.12 - clamp(hem, 0.0, 1.0) * 0.07);

    // 2. Procedural woven cloth micro-texture & normal perturbation
    float weave_freq = (u_fabric == 0) ? 740.0 : ((u_fabric == 1) ? 520.0 : 620.0);
    float wx = sin(x_tot * weave_freq);
    float wy = sin(y * weave_freq);
    float weave = wx * wy;
    float grain = (hash21(floor(vec2(x_tot, y) * weave_freq * 0.45)) - 0.5) * 0.018;

    vec3 n = normalize(v_norm);
    if (!gl_FrontFacing) {
        n = -n;
    }

    // Tangent-approximate weave normal ripple
    float bump_amp = (u_fabric == 0) ? 0.016 : ((u_fabric == 1) ? 0.042 : 0.028);
    n = normalize(n + vec3(wx * bump_amp, wy * bump_amp, 0.0));

    // Slightly modulate albedo by micro-weave
    float tex_contrast = (u_fabric == 0) ? 0.005 : ((u_fabric == 1) ? 0.014 : 0.009);
    albedo *= (1.0 + weave * tex_contrast + grain * (tex_contrast * 10.0));

    // 3. Physical cloth lighting synced with selected sky mode
    vec3 w = computeSkyWeights();
    vec3 sun_dir = normalize(
        vec3(0.55, 0.52, 0.65) * w.x +
        vec3(0.68, 0.14, 0.72) * w.y +
        vec3(0.52, 0.45, 0.72) * w.z
    );
    vec3 view_dir = normalize(u_cam_pos - v_world_pos);
    vec3 half_v = normalize(sun_dir + view_dir);

    float ndl_raw = dot(n, sun_dir);
    // Soft wrap diffuse keeps folds rich without crushing shadows to black
    float wrap = clamp((ndl_raw + 0.38) / 1.38, 0.0, 1.0);
    float fold_ao = 0.74 + 0.26 * clamp(n.z * 0.5 + 0.5, 0.0, 1.0);

    vec3 light_col = vec3(1.00, 0.98, 0.94) * w.x
                   + vec3(1.00, 0.68, 0.36) * w.y
                   + vec3(0.54, 0.64, 0.86) * w.z;

    vec3 sky_amb = vec3(0.32, 0.46, 0.66) * w.x
                 + vec3(0.32, 0.18, 0.24) * w.y
                 + vec3(0.14, 0.18, 0.30) * w.z;
    vec3 gnd_amb = vec3(0.18, 0.22, 0.28) * w.x
                 + vec3(0.18, 0.10, 0.12) * w.y
                 + vec3(0.16, 0.12, 0.14) * w.z;
    vec3 hemi = mix(gnd_amb, sky_amb, n.y * 0.5 + 0.5);

    // Monument uplight at night (gentle warm ground spotlight so crimson & crescent glow proudly)
    float uplight = clamp((-n.y * 0.45 + 0.65) * (0.65 + 0.35 * n.z), 0.0, 1.0) * w.z;
    vec3 monument_col = vec3(0.38, 0.26, 0.22) * uplight;

    // Subsurface fabric translucency when light strikes across/behind folds
    float back_lit = pow(clamp(dot(view_dir, -sun_dir) * 0.5 + 0.5, 0.0, 1.0), 2.2)
                   * (0.35 + 0.65 * (1.0 - abs(ndl_raw)));
    float trans_str = (u_fabric == 0) ? 0.36 : ((u_fabric == 1) ? 0.20 : 0.14);
    vec3 trans_col = mix(vec3(0.95, 0.08, 0.10), vec3(1.0, 0.88, 0.80), white_mask);

    // Silk / velvet Fresnel sheen
    float ndv = clamp(dot(n, view_dir), 0.0, 1.0);
    float fres = pow(1.0 - ndv, 3.2);
    float spec_pow = (u_fabric == 0) ? 28.0 : ((u_fabric == 1) ? 8.0 : 14.0);
    float spec_str = (u_fabric == 0) ? 0.26 : ((u_fabric == 1) ? 0.06 : 0.14);
    float spec = pow(max(0.0, dot(n, half_v)), spec_pow) * spec_str;
    float sheen_str = (u_fabric == 2) ? 0.28 : ((u_fabric == 0) ? 0.18 : 0.08);

    vec3 lit = albedo * (hemi * 0.72 + light_col * wrap * 0.92 + monument_col) * fold_ao
             + trans_col * light_col * back_lit * trans_str * u_atmosphere
             + light_col * spec
             + mix(albedo, light_col, 0.45) * fres * sheen_str;

    // Subtle hoist shadow near flagpole
    float pole_sh = smoothstep(0.0, 0.055, x_tot);
    lit *= (0.76 + 0.24 * pole_sh);

    // Clean 1.5px anti-aliased edge on top/bottom/fly borders
    float edge_x = 1.0 - smoothstep(0.994, 1.000, v_uv.x);
    float edge_y = smoothstep(0.000, 0.006, v_uv.y) * (1.0 - smoothstep(0.994, 1.000, v_uv.y));
    float alpha = clamp(edge_x * edge_y, 0.0, 1.0);

    fragColor = vec4(clamp(lit, 0.0, 1.0), alpha);
}
"#;
