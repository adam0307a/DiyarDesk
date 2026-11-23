// Dedicated OpenGL 3.3 Core GLSL shaders for DiyarDesk Studio Glass 3D
// Ported 1:1 from prototype/studio-glass.html + prototype/studio-glass-engine.js

pub const PBR_VERT: &str = r#"#version 330 core
layout (location = 0) in vec3 a_pos;
layout (location = 1) in vec3 a_norm;
layout (location = 2) in vec2 a_uv;
layout (location = 3) in vec4 a_col; // rgb = linear/srgb base tint, a = material kind (0=solid,1=walnut,2=plasterGreen,3=plasterNiche,4=lampGlass,5=lampUndercap,6=metal)
layout (location = 4) in vec2 a_mat; // x = roughness, y = metalness

uniform mat4 u_vp;
uniform mat4 u_model;

out vec3 v_world;
out vec3 v_norm;
out vec2 v_uv;
out vec4 v_col;
out vec2 v_mat;

void main() {
    vec4 wp = u_model * vec4(a_pos, 1.0);
    v_world = wp.xyz;
    v_norm = normalize(mat3(u_model) * a_norm);
    v_uv = a_uv;
    v_col = a_col;
    v_mat = a_mat;
    gl_Position = u_vp * wp;
}
"#;

pub const PBR_FRAG: &str = r#"#version 330 core
in vec3 v_world;
in vec3 v_norm;
in vec2 v_uv;
in vec4 v_col;
in vec2 v_mat;

out vec4 fragColor;

uniform vec3 u_cam_pos;
uniform vec3 u_sun_dir;
uniform vec3 u_sun_col;
uniform float u_sun_int;
uniform vec3 u_hemi_sky;
uniform vec3 u_hemi_ground;
uniform float u_hemi_int;
uniform vec3 u_amb_col;
uniform float u_amb_int;
uniform vec3 u_lamp_pos;
uniform float u_lamp_int;
uniform float u_lamp_glow;
uniform vec3 u_screen_pos;
uniform float u_screen_int;
uniform vec3 u_fill_col;
uniform float u_fill_int;
uniform vec3 u_wall_tint;
uniform vec3 u_niche_tint;
uniform float u_blinds;
uniform vec2 u_kb_pos;
uniform vec2 u_mouse_pos;
uniform vec2 u_cup_pos;
uniform vec2 u_nb_pos;
uniform float u_books_x;

float hash21(vec2 p) {
    p = fract(p * vec2(127.1, 311.7));
    p += dot(p, p + 34.5);
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
    for (int i = 0; i < 4; i++) {
        v += a * vnoise(p);
        p *= 2.03;
        a *= 0.5;
    }
    return v;
}

vec3 walnutGrain(vec3 wp, vec3 tint) {
    vec2 p = vec2(wp.x * 2.1 + wp.z * 0.85, wp.z * 7.2 + wp.y * 5.2);
    float w = sin(p.y * 2.6 + sin(p.x * 1.8) * 1.4) * 0.5 + 0.5;
    float f = fbm(p * vec2(1.4, 3.8));
    float pore = vnoise(vec2(p.x * 18.0, p.y * 5.0));
    vec3 darkW = vec3(0.11, 0.045, 0.018);
    vec3 midW  = vec3(0.24, 0.105, 0.042);
    vec3 hiW   = vec3(0.36, 0.175, 0.078);
    vec3 wood = mix(darkW, midW, w * 0.65 + f * 0.35);
    wood = mix(wood, hiW, smoothstep(0.62, 0.92, f) * 0.45);
    wood *= (0.86 + 0.22 * pore);
    return wood * (tint * 1.75);
}

vec3 plasterTex(vec3 wp, vec3 baseLin) {
    vec2 p = wp.xy * 1.6 + wp.zy * 0.7;
    float cloud = fbm(p * 1.15);
    float fine = vnoise(p * 16.0);
    return baseLin * (0.84 + 0.28 * cloud + (fine - 0.5) * 0.06);
}

// Soft analytic studio shadow & venetian blind projection
float studioShadow(vec3 wp) {
    float sh = 1.0;

    // 1. Soft desk shadow on floor beneath desk
    if (wp.y < 0.88) {
        vec2 dp = abs(wp.xz - vec2(0.55, 0.08)) - vec2(2.55, 0.96);
        float dDesk = max(dp.x, dp.y);
        float under = 1.0 - smoothstep(-0.25, 0.55, dDesk);
        sh *= mix(1.0, 0.34, under * smoothstep(0.88, 0.0, wp.y));
    }

    // 2. Contact shadows on desk top (monitor stand, keyboard, mouse, pen cup, notebook — dynamically follow dragged props!)
    if (abs(wp.y - 0.92) < 0.08) {
        // Monitor stand + back screen shadow
        float dMon = length((wp.xz - vec2(0.72, -0.34)) / vec2(0.55, 0.32));
        sh *= mix(0.45, 1.0, smoothstep(0.45, 1.35, dMon));
        // Keyboard contact shadow
        vec2 kb = abs(wp.xz - (u_kb_pos + vec2(0.04, 0.02))) - vec2(0.44, 0.16);
        sh *= mix(0.48, 1.0, smoothstep(-0.04, 0.14, max(kb.x, kb.y)));
        // Mouse shadow
        sh *= mix(0.55, 1.0, smoothstep(0.05, 0.16, length(wp.xz - (u_mouse_pos + vec2(0.03, 0.02)))));
        // Pen cup shadow
        sh *= mix(0.52, 1.0, smoothstep(0.05, 0.16, length(wp.xz - (u_cup_pos + vec2(0.03, 0.02)))));
        // Notebook shadow
        vec2 nb = abs(wp.xz - (u_nb_pos + vec2(0.02, 0.02))) - vec2(0.18, 0.22);
        sh *= mix(0.55, 1.0, smoothstep(-0.03, 0.12, max(nb.x, nb.y)));
    }

    // 3. Soft contact shadows on right console table (lamp, books)
    if (abs(wp.y - 1.04) < 0.08) {
        sh *= mix(0.46, 1.0, smoothstep(0.10, 0.28, length(wp.xz - vec2(2.94, -1.16))));
        sh *= mix(0.50, 1.0, smoothstep(0.14, 0.34, length(wp.xz - vec2(u_books_x + 0.02, -1.16))));
    }

    // 4. Corner step wall AO on right niche wall
    if (wp.z < -1.7) {
        float cornerAo = smoothstep(0.0, 0.75, abs(wp.x - 2.42));
        sh *= mix(0.55, 1.0, cornerAo);
        // Monitor shadow cast on back wall
        vec2 ms = abs(wp.xy - vec2(1.25, 1.62)) - vec2(0.95, 0.56);
        sh *= mix(0.62, 1.0, smoothstep(-0.08, 0.45, max(ms.x, ms.y)));
    }

    // 5. Venetian blind sunlit stripe modulation across studio wall & desk
    float windowBeam = smoothstep(-2.8, -1.2, wp.x) * (1.0 - smoothstep(1.4, 3.6, wp.x));
    float yBeam = smoothstep(0.65, 1.15, wp.y) * (1.0 - smoothstep(2.95, 3.9, wp.y));
    float slatCoord = (wp.y + wp.x * 0.28) * 10.5;
    float blindStripe = smoothstep(0.20, 0.78, sin(slatCoord) * 0.5 + 0.5);
    float blindRegion = smoothstep(1.0 - u_blinds * 0.72 - 0.12, 1.0 - u_blinds * 0.72 + 0.12, clamp((wp.y - 0.9) / 2.6, 0.0, 1.0));
    float slatVis = mix(1.0, mix(0.32, 1.0, blindStripe * (0.28 + 0.72 * u_blinds)), blindRegion);
    sh *= mix(1.0, slatVis, windowBeam * yBeam * 0.55);

    return clamp(sh, 0.18, 1.0);
}

vec3 pbrLight(vec3 N, vec3 V, vec3 L, vec3 lCol, vec3 albedo, float rough, float metal) {
    float NdotL = max(dot(N, L), 0.0);
    if (NdotL <= 0.0001) return vec3(0.0);
    vec3 H = normalize(V + L);
    float NdotH = max(dot(N, H), 0.0);
    float VdotH = max(dot(V, H), 0.0);

    vec3 F0 = mix(vec3(0.04), albedo, metal);
    vec3 F = F0 + (1.0 - F0) * pow(1.0 - VdotH, 5.0);

    float a = max(0.06, rough * rough);
    float a2 = a * a;
    float denom = (NdotH * NdotH * (a2 - 1.0) + 1.0);
    float D = a2 / (3.14159265 * denom * denom + 1e-5);
    float k = (rough + 1.0);
    float G = 1.0 / max(0.25, (NdotL + (1.0 - NdotL) * (k * k * 0.125)));

    vec3 spec = (D * F * G) * 0.25;
    vec3 kd = (vec3(1.0) - F) * (1.0 - metal);
    return (kd * albedo / 3.14159265 + spec) * lCol * NdotL;
}

void main() {
    float matKind = v_col.a;
    vec3 base = pow(max(v_col.rgb, vec3(0.0)), vec3(2.2));
    float rough = clamp(v_mat.x, 0.08, 0.98);
    float metal = clamp(v_mat.y, 0.0, 1.0);

    if (matKind > 0.5 && matKind < 1.5) {
        base = walnutGrain(v_world, base);
    } else if (matKind > 1.5 && matKind < 2.5) {
        base = plasterTex(v_world, pow(max(u_wall_tint, vec3(0.0)), vec3(2.2)));
    } else if (matKind > 2.5 && matKind < 3.5) {
        base = plasterTex(v_world, pow(max(u_niche_tint, vec3(0.0)), vec3(2.2)));
    } else if (matKind > 4.5 && matKind < 5.5) {
        // Lamp under-cap emissive warm core
        vec3 em = vec3(1.0, 0.72, 0.36) * (0.25 + u_lamp_glow * 2.2);
        fragColor = vec4(em, 1.0);
        return;
    }

    vec3 N = normalize(v_norm);
    if (!gl_FrontFacing) N = -N;
    vec3 V = normalize(u_cam_pos - v_world);

    float sh = studioShadow(v_world);

    // 1. Hemisphere + Ambient light
    float hemiMix = N.y * 0.5 + 0.5;
    vec3 hemi = mix(u_hemi_ground, u_hemi_sky, hemiMix) * u_hemi_int;
    vec3 col = base * (hemi * 0.34 + u_amb_col * u_amb_int * 0.30) * (0.62 + 0.38 * sh);

    // 2. Main directional sun/moon light streaming from left window
    vec3 sunL = normalize(u_sun_dir);
    col += pbrLight(N, V, sunL, u_sun_col * u_sun_int * sh, base, rough, metal);

    // 3. Soft front-left window bounce desk fill
    vec3 fillL = normalize(vec3(-2.7, 2.5, 3.6));
    col += pbrLight(N, V, fillL, u_fill_col * u_fill_int * (0.7 + 0.3 * sh), base, rough, metal);

    // 4. Warm frosted mushroom lamp point light (right side table)
    if (u_lamp_int > 0.01) {
        vec3 toLamp = u_lamp_pos - v_world;
        float dist = length(toLamp);
        float att = 1.0 / (1.0 + 0.28 * dist * dist);
        att *= smoothstep(7.5, 0.2, dist);
        col += pbrLight(N, V, toLamp / max(dist, 0.001), vec3(1.0, 0.56, 0.20) * u_lamp_int * 1.45 * att, base, rough, metal);
    }

    // 5. Cool studio monitor screen glow onto keyboard & desk
    if (u_screen_int > 0.01) {
        vec3 toScr = u_screen_pos - v_world;
        float dist = length(toScr);
        float att = 1.0 / (1.0 + 0.45 * dist * dist);
        att *= smoothstep(5.4, 0.15, dist);
        col += pbrLight(N, V, toScr / max(dist, 0.001), vec3(0.68, 0.78, 0.94) * u_screen_int * 0.85 * att, base, rough, metal);
    }

    // 6. Frosted glass mushroom lamp self-illumination (subsurface warm glow!)
    if (matKind > 3.5 && matKind < 4.5) {
        float fres = pow(1.0 - max(dot(N, V), 0.0), 2.2);
        vec3 warmCore = mix(vec3(1.0, 0.62, 0.24), vec3(1.0, 0.88, 0.66), 1.0 - fres * 0.65);
        col += warmCore * u_lamp_glow * 1.15;
    }

    // Subtle studio metallic rim reflection
    if (metal > 0.4) {
        float fres = pow(1.0 - max(dot(N, V), 0.0), 3.0);
        col += mix(u_hemi_sky, u_sun_col, 0.4) * fres * metal * 0.26;
    }

    fragColor = vec4(max(col, vec3(0.0)), 1.0);
}
"#;

pub const WINDOW_VERT: &str = r#"#version 330 core
layout (location = 0) in vec3 a_pos;
layout (location = 1) in vec3 a_norm;
layout (location = 2) in vec2 a_uv;
layout (location = 3) in vec4 a_col;
layout (location = 4) in vec2 a_mat;

uniform mat4 u_vp;
uniform mat4 u_model;
out vec2 v_uv;

void main() {
    v_uv = a_uv;
    gl_Position = u_vp * u_model * vec4(a_pos, 1.0);
}
"#;

pub const WINDOW_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform float u_time;
uniform float u_rain;
uniform float u_weather;
uniform vec3 u_sky_top;
uniform vec3 u_sky_mid;
uniform vec3 u_sky_bot;
uniform float u_stars;
uniform float u_flash;

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash21(i), hash21(i + vec2(1.0, 0.0)), f.x),
               mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), f.x), f.y);
}

float fbm(vec2 p) {
    float v = 0.0, a = 0.5;
    for (int i = 0; i < 4; i++) {
        v += a * vnoise(p);
        p *= 2.02;
        a *= 0.5;
    }
    return v;
}

float rainLayer(vec2 uv, float t, float scale) {
    vec2 a = vec2(scale * 1.15, scale * 0.42);
    vec2 st = uv * a;
    st.y += t * 0.22;
    vec2 id = floor(st);
    vec2 gv = fract(st) - 0.5;
    float n = hash21(id);
    float y = -sin(t * 1.4 + n * 6.2831 + sin(t * 1.4 + n * 6.2831) * 0.45) * 0.38;
    float x = (n - 0.5) * 0.72 + sin(st.y * 2.5 + n * 12.0) * 0.06;
    vec2 p = (gv - vec2(x, y)) / vec2(1.0, 2.3);
    float drop = smoothstep(0.085, 0.02, length(p));
    float trailY = clamp((gv.y - y) * 1.8, 0.0, 1.0);
    float trail = smoothstep(0.045, 0.0, abs(gv.x - x)) * smoothstep(0.0, 0.08, gv.y - y) * (1.0 - trailY) * 0.45;
    return clamp(drop + trail, 0.0, 1.0);
}

// 3 misty woodland foliage layers outside window (matches createOutdoorForestTexture!)
vec4 sampleForest(vec2 uv) {
    vec3 col = vec3(0.0);
    float alpha = 0.0;

    // Far misty ridge
    float h0 = 0.36 + 0.16 * fbm(vec2(uv.x * 5.2 + 1.3, 0.4)) + 0.05 * sin(uv.x * 22.0);
    float m0 = 1.0 - smoothstep(h0 - 0.04, h0 + 0.04, uv.y);
    col = mix(col, vec3(0.032, 0.072, 0.060), m0 * 0.45);
    alpha = max(alpha, m0 * 0.45);

    // Mid canopy crowns + trunks
    float h1 = 0.28 + 0.22 * fbm(vec2(uv.x * 8.5 + 4.1, 1.7)) + 0.06 * sin(uv.x * 34.0 + 1.0);
    float m1 = 1.0 - smoothstep(h1 - 0.03, h1 + 0.03, uv.y);
    col = mix(col, vec3(0.016, 0.040, 0.034), m1 * 0.72);
    alpha = max(alpha, m1 * 0.72);

    // Near dark pine & leafy clusters
    float h2 = 0.18 + 0.24 * fbm(vec2(uv.x * 13.0 + 7.2, 3.1));
    float m2 = 1.0 - smoothstep(h2 - 0.025, h2 + 0.025, uv.y);
    col = mix(col, vec3(0.008, 0.020, 0.016), m2 * 0.90);
    alpha = max(alpha, m2 * 0.90);

    return vec4(col, alpha);
}

void main() {
    vec3 sky = mix(u_sky_bot, u_sky_mid, smoothstep(0.0, 0.52, v_uv.y));
    sky = mix(sky, u_sky_top, smoothstep(0.45, 1.0, v_uv.y));

    // Twinkling night stars
    if (u_stars > 0.02) {
        vec2 sid = floor(v_uv * vec2(44.0, 34.0));
        vec2 sgv = fract(v_uv * vec2(44.0, 34.0)) - 0.5;
        float h = hash21(sid);
        if (h > 0.965 && v_uv.y > 0.34) {
            float tw = 0.55 + 0.45 * sin(u_time * 2.2 + h * 40.0);
            sky += vec3(0.85, 0.92, 1.0) * smoothstep(0.18, 0.02, length(sgv)) * u_stars * tw;
        }
    }

    // Soft lightning flash on storm clouds
    sky += vec3(0.65, 0.80, 1.0) * u_flash * 0.55;

    // Rain distortion & glass droplets
    float rainAmt = u_rain * u_weather;
    float drops = 0.0;
    vec2 fUv = v_uv;
    if (rainAmt > 0.02) {
        float r1 = rainLayer(v_uv, u_time * 1.15, 10.0);
        float r2 = rainLayer(v_uv + vec2(0.37, 0.19), u_time * 0.85, 16.0);
        drops = clamp(r1 + r2 * 0.7, 0.0, 1.0) * rainAmt;
        fUv += vec2(drops * 0.014, -drops * 0.018);
    }

    vec4 forest = sampleForest(clamp(fUv, 0.002, 0.998));
    vec3 skyLin = pow(max(sky, vec3(0.0)), vec3(2.2));
    vec3 botLin = pow(max(u_sky_bot, vec3(0.0)), vec3(2.2));
    vec3 col = mix(skyLin, mix(forest.rgb, botLin * 0.65, 0.42), forest.a * 0.86);

    if (rainAmt > 0.02) {
        col += vec3(0.65, 0.82, 0.96) * drops * 0.28;
        float streak = sin((v_uv.x * 75.0 + v_uv.y * 16.0) + u_time * 18.0);
        streak = smoothstep(0.96, 1.0, streak) * rainAmt * 0.12;
        col += vec3(0.65, 0.82, 0.96) * streak;
    }

    float glare = smoothstep(0.65, 0.0, abs((v_uv.x * 0.7 + v_uv.y * 0.5) - 0.55)) * 0.035;
    col += vec3(1.0) * glare;
    fragColor = vec4(col, 1.0);
}
"#;

// Procedural Alpine Mountain Lake + Matterhorn Peak wallpaper on the 3D Studio Display Monitor!
pub const MONITOR_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform float u_emissive;

float hash11(float p) {
    p = fract(p * 0.1031);
    p *= p + 33.33;
    p *= p + p;
    return fract(p);
}

void main() {
    // v_uv.y: 0=bottom, 1=top
    vec2 uv = v_uv;

    // 1. Alpine sky gradient (matches createMonitorWallpaperTexture!)
    vec3 sky = mix(vec3(0.90, 0.84, 0.76), vec3(0.84, 0.81, 0.78), smoothstep(0.38, 0.54, uv.y));
    sky = mix(sky, vec3(0.62, 0.70, 0.75), smoothstep(0.54, 0.76, uv.y));
    sky = mix(sky, vec3(0.42, 0.51, 0.60), smoothstep(0.76, 1.0, uv.y));
    // Soft clouds behind peak
    float cloud = smoothstep(0.25, 0.85, sin(uv.x * 7.5 + 0.6) * cos(uv.y * 9.0 - 4.0) * 0.5 + 0.5);
    sky = mix(sky, vec3(0.98, 0.95, 0.91), cloud * 0.22 * smoothstep(0.40, 0.82, uv.y));

    vec3 col = sky;

    // 2. Distant blue-grey mountain range
    float bgM = 0.44 + 0.12 * sin(uv.x * 9.5 + 0.4) + 0.05 * cos(uv.x * 23.0);
    if (uv.y < bgM && uv.y > 0.33) {
        col = vec3(0.36, 0.44, 0.51);
    }

    // 3. Hero sunlit Matterhorn peak centered around x=0.535, apex y=0.86
    float dx = uv.x - 0.535;
    float peakH = 0.86 - abs(dx) * 1.92 + sin(uv.x * 38.0) * 0.012;
    if (uv.y < peakH && uv.y >= 0.34) {
        vec3 shadowFace = vec3(0.31, 0.36, 0.42);
        vec3 sunlitFace = mix(vec3(0.36, 0.29, 0.27), vec3(0.79, 0.53, 0.39), smoothstep(0.34, 0.84, uv.y));
        float ridge = smoothstep(-0.015, 0.015, dx + (0.86 - uv.y) * 0.05);
        col = mix(shadowFace, sunlitFace, ridge);
        // Snow streaks along upper ridges
        float snow = smoothstep(0.56, 0.82, uv.y) * smoothstep(0.52, 0.85, sin(uv.x * 65.0 + uv.y * 18.0) * 0.5 + 0.5);
        col = mix(col, vec3(0.96, 0.94, 0.92), snow * 0.48);
    }

    // 4. Misty valley band
    float mist = exp(-pow((uv.y - 0.37) / 0.06, 2.0)) * 0.48;
    col = mix(col, vec3(0.87, 0.89, 0.90), mist);

    // 5. Alpine lake & peak reflection (y < 0.34)
    if (uv.y < 0.34) {
        float lt = clamp(uv.y / 0.34, 0.0, 1.0);
        vec3 lake = mix(vec3(0.15, 0.20, 0.24), vec3(0.55, 0.63, 0.68), lt);
        // Alpenglow peak reflection
        float reflW = (0.34 - uv.y) * 0.42;
        float refl = 1.0 - smoothstep(reflW, reflW + 0.06, abs(uv.x - 0.54));
        lake = mix(lake, vec3(0.69, 0.49, 0.38), refl * 0.32 * lt);
        // Gentle lake ripples
        lake += vec3(0.03) * sin(uv.y * 180.0 + uv.x * 16.0);
        col = lake;
    }

    // 6. Left & right dark pine forest banks framing the lake
    float leftEdge = 0.38 - uv.y * 0.14;
    float rightEdge = 0.62 + uv.y * 0.14;
    float pineSpireL = 0.36 + 0.06 * sin(uv.x * 95.0) * (1.0 - uv.x * 1.8);
    float pineSpireR = 0.36 + 0.06 * sin(uv.x * 92.0 + 1.4) * (uv.x * 1.8 - 0.8);
    if ((uv.x < leftEdge && uv.y < pineSpireL) || (uv.x > rightEdge && uv.y < pineSpireR)) {
        float tone = hash11(floor(uv.x * 90.0));
        col = mix(vec3(0.065, 0.105, 0.105), vec3(0.11, 0.17, 0.15), tone);
    }

    // Subtle glass screen reflection
    float sheen = smoothstep(0.35, 0.0, abs(uv.x * 0.7 - uv.y * 0.5 - 0.1)) * 0.04;
    vec3 lin = pow(max(col + vec3(sheen), vec3(0.0)), vec3(2.2));
    fragColor = vec4(lin * u_emissive, 1.0);
}
"#;

pub const SHAFT_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec3 u_color;
uniform float u_alpha;
uniform float u_blinds;
uniform float u_time;

void main() {
    float along = smoothstep(0.0, 0.14, v_uv.x) * (1.0 - smoothstep(0.18, 1.0, v_uv.x));
    float edgeY = smoothstep(0.0, 0.16, v_uv.y) * (1.0 - smoothstep(0.84, 1.0, v_uv.y));
    float blindRegion = smoothstep(1.0 - u_blinds * 0.72 - 0.05, 1.0 - u_blinds * 0.72 + 0.05, v_uv.y);
    float stripes = 0.5 + 0.5 * sin(v_uv.y * 54.0);
    float slatMask = mix(1.0, mix(0.18, 1.0, smoothstep(0.25, 0.75, stripes) * (0.28 + 0.72 * u_blinds)), blindRegion);
    float shimmer = 0.92 + 0.08 * sin(u_time * 0.6 + v_uv.y * 5.0);
    float a = along * edgeY * slatMask * u_alpha * shimmer;
    if (a < 0.002) discard;
    fragColor = vec4(u_color, a);
}
"#;

pub const DUST_VERT: &str = r#"#version 330 core
layout (location = 0) in vec3 a_pos;
uniform mat4 u_vp;
uniform float u_pt_size;
void main() {
    vec4 clip = u_vp * vec4(a_pos, 1.0);
    gl_Position = clip;
    gl_PointSize = clamp(u_pt_size / max(0.5, clip.w), 1.5, 6.0);
}
"#;

pub const DUST_FRAG: &str = r#"#version 330 core
out vec4 fragColor;
uniform vec3 u_color;
uniform float u_alpha;
void main() {
    vec2 p = gl_PointCoord - vec2(0.5);
    float d = length(p);
    if (d > 0.5) discard;
    float a = smoothstep(0.5, 0.08, d) * u_alpha;
    fragColor = vec4(u_color, a);
}
"#;

pub const POST_BLUR_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform sampler2D u_tex;
uniform vec2 u_texel;
uniform vec2 u_dir;
uniform float u_radius;
uniform int u_taps;

void main() {
    float sigma = max(0.8, u_radius * 0.48);
    vec3 sum = texture(u_tex, v_uv).rgb;
    float wSum = 1.0;
    for (int i = 1; i <= 14; i++) {
        if (i > u_taps) break;
        float fi = float(i);
        float w = exp(-0.5 * (fi * fi) / (sigma * sigma));
        vec2 off = u_dir * u_texel * (fi * u_radius / max(1.0, float(u_taps)));
        sum += texture(u_tex, clamp(v_uv + off, vec2(0.001), vec2(0.999))).rgb * w;
        sum += texture(u_tex, clamp(v_uv - off, vec2(0.001), vec2(0.999))).rgb * w;
        wSum += 2.0 * w;
    }
    fragColor = vec4(sum / wSum, 1.0);
}
"#;

// Exact 1:1 port of Studio Glass Multi-Pass Optical Refractive Glass Shader!
pub const POST_GLASS_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

#define MAX_GLASS 56

uniform sampler2D u_scene;
uniform sampler2D u_blur;
uniform vec2 u_res;
uniform int u_count;
uniform vec4 u_rects[MAX_GLASS];
uniform vec4 u_meta[MAX_GLASS];
uniform float u_refraction;
uniform float u_frost;
uniform vec3 u_rim_tint;
uniform vec2 u_light_pos;
uniform int u_pulse_idx;
uniform float u_pulse_age;
uniform vec3 u_pulse_col_a;
uniform vec3 u_pulse_col_b;
uniform vec3 u_pulse_col_c;
uniform float u_time;
uniform float u_exposure;

vec3 acesFilm(vec3 x) {
    x = max(x, vec3(0.0));
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

vec3 gradeScene(vec3 lin) {
    return pow(acesFilm(lin * u_exposure), vec3(1.0 / 2.2));
}

float gHash(vec2 p) {
    p = fract(p * vec2(234.34, 435.345));
    p += dot(p, p + 34.23);
    return fract(p.x * p.y);
}

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

float sdOctagon(vec2 p, vec2 b, float c) {
    vec2 q = abs(p) - b;
    float dBox = max(q.x, q.y);
    float dDiag = (abs(p.x) + abs(p.y) - (b.x + b.y - c)) * 0.70710678;
    return max(dBox, dDiag);
}

float evalOne(vec2 px, vec4 rc, vec4 mt) {
    if (mt.y > 0.5 && mt.y < 1.5) {
        return sdOctagon(px - rc.xy, rc.zw, min(rc.z, rc.w) * 0.36);
    }
    return sdRoundBox(px - rc.xy, rc.zw, mt.x);
}

float evalField(vec2 px, out vec4 bestRc, out vec4 bestMt, out int bestIdx) {
    float d = 1e6;
    bestRc = vec4(0.0, 0.0, 32.0, 32.0);
    bestMt = vec4(14.0, 0.0, 0.0, 0.0);
    bestIdx = -1;
    for (int i = 0; i < MAX_GLASS; i++) {
        if (i >= u_count) break;
        float di = evalOne(px, u_rects[i], u_meta[i]);
        if (di < d) {
            d = di;
            bestRc = u_rects[i];
            bestMt = u_meta[i];
            bestIdx = i;
        }
    }
    return d;
}

float evalOnly(vec2 px) {
    vec4 a, b;
    int idx;
    return evalField(px, a, b, idx);
}

void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    vec3 baseCol = gradeScene(texture(u_scene, v_uv).rgb);

    if (u_count == 0) {
        fragColor = vec4(baseCol, 1.0);
        return;
    }

    vec4 rc, mt;
    int bestIdx;
    float d = evalField(px, rc, mt, bestIdx);

    // Soft ambient drop shadow behind glass cards
    float shD = evalOnly(px - vec2(0.0, 8.0));
    float shadow = exp(-max(shD, 0.0) * 0.055) * 0.36 * smoothstep(-2.0, 4.0, d);
    vec3 outCol = baseCol * (1.0 - shadow);

    if (d < 2.2) {
        float eps = 1.6;
        vec2 grad = vec2(
            evalOnly(px + vec2(eps, 0.0)) - evalOnly(px - vec2(eps, 0.0)),
            evalOnly(px + vec2(0.0, eps)) - evalOnly(px - vec2(0.0, eps))
        );
        float gLen = length(grad);
        vec2 n2 = gLen > 1e-4 ? (grad / gLen) : vec2(0.0, -1.0);

        vec2 localN = (px - rc.xy) / max(vec2(12.0), rc.zw);

        // Prismatic click burst shockwave inside clicked card
        float pulseOn = (bestIdx == u_pulse_idx && u_pulse_age >= 0.0 && u_pulse_age < 1.75) ? 1.0 : 0.0;
        float pNorm = clamp(u_pulse_age / 1.65, 0.0, 1.0);
        float pRise = smoothstep(0.0, 0.16, pNorm);
        float pFall = 1.0 - smoothstep(0.45, 1.0, pNorm);
        float pEnv = pulseOn * pRise * pFall;
        vec2 ringCenter = vec2(0.02, 0.16);
        vec2 ovalP = (localN - ringCenter) / vec2(0.86, 0.56);
        float ovalR = length(ovalP);
        float waveR = mix(0.28, 0.94, pow(pNorm, 0.52));
        float ringWave = exp(-pow((ovalR - waveR) / 0.14, 2.0)) * pEnv;
        float ringHalo = exp(-pow((ovalR - waveR) / 0.42, 2.0)) * pEnv;

        // Curved optical glass bezel near card borders + click wave bend
        float bevelW = min(22.0, min(rc.z, rc.w) * 0.55);
        float edgeNorm = clamp(-d / max(6.0, bevelW), 0.0, 1.0);
        float rimCurve = pow(1.0 - edgeNorm, 2.0);

        vec2 waveDir = ovalR > 1e-3 ? normalize(ovalP) : vec2(0.0, 1.0);
        float refrPx = (7.0 + 18.0 * u_refraction) * rimCurve;
        vec2 refrUV = vec2(-n2.x * refrPx / u_res.x, n2.y * refrPx / u_res.y)
                    + vec2(waveDir.x, -waveDir.y) * (ringWave * 16.0 / u_res);
        float dispPx = (1.4 + 4.2 * u_refraction) * rimCurve + ringWave * 5.5;
        vec2 dispUV = vec2(-n2.x * dispPx / u_res.x, n2.y * dispPx / u_res.y);

        vec2 uvR = clamp(v_uv + refrUV + dispUV, vec2(0.002), vec2(0.998));
        vec2 uvG = clamp(v_uv + refrUV,          vec2(0.002), vec2(0.998));
        vec2 uvB = clamp(v_uv + refrUV - dispUV, vec2(0.002), vec2(0.998));

        vec3 sharpCol = gradeScene(vec3(
            texture(u_scene, uvR).r,
            texture(u_scene, uvG).g,
            texture(u_scene, uvB).b
        ));
        vec3 blurCol = gradeScene(vec3(
            texture(u_blur, uvR).r,
            texture(u_blur, uvG).g,
            texture(u_blur, uvB).b
        ));

        float frostAmt = clamp(u_frost * (0.62 + 0.34 * edgeNorm), 0.0, 0.90);
        vec3 glassCol = mix(sharpCol, blurCol, frostAmt);

        // Dark smoked glass body (Apple/Studio Glass aesthetic)
        float smoke = mix(0.24, 0.14, mt.z);
        glassCol = mix(glassCol, vec3(0.07, 0.09, 0.11), smoke);

        // Brightness pill fill (mt.y > 1.5)
        if (mt.y > 1.5) {
            float localY = clamp(((rc.y + rc.w) - px.y) / max(1.0, rc.w * 2.0), 0.0, 1.0);
            float filled = 1.0 - smoothstep(mt.w - 0.015, mt.w + 0.015, localY);
            vec3 ivory = vec3(0.93, 0.92, 0.88);
            glassCol = mix(glassCol, ivory, filled * 0.92);
        }

        // Studio window directional Fresnel & crisp caustic edge highlight
        vec2 lDir = normalize(u_light_pos * u_res - px);
        vec3 N3 = normalize(vec3(n2 * rimCurve * 0.9, 0.52));
        vec3 L3 = normalize(vec3(lDir, 0.65));
        vec3 V3 = vec3(0.0, 0.0, 1.0);
        vec3 H3 = normalize(L3 + V3);

        float fresnel = pow(1.0 - max(dot(N3, V3), 0.0), 2.5);
        float spec = pow(max(dot(N3, H3), 0.0), 28.0);
        float rimLine = exp(-abs(d + 1.0) * 1.15);
        float topLeftGlow = smoothstep(0.85, -0.95, localN.x * 0.65 + localN.y * 0.75);

        float activeBoost = 1.0 + mt.z * 0.75;
        glassCol += u_rim_tint * (fresnel * (0.24 + 0.24 * u_refraction) * activeBoost + rimLine * (0.26 + 0.34 * mt.z) * (0.55 + 0.65 * topLeftGlow));
        glassCol += vec3(1.0) * (spec * 0.34 + topLeftGlow * (0.045 + 0.07 * mt.z));

        // Iridescent prismatic luminous ring + sparkles + chromatic edge bloom on click!
        if (pEnv > 0.004) {
            float ang = atan(ovalP.y, ovalP.x);
            float s1 = 0.5 + 0.5 * sin(ang * 1.0 + pNorm * 5.2);
            float s2 = 0.5 + 0.5 * cos(ang * 1.0 - pNorm * 3.8 + 1.2);
            vec3 prism = mix(u_pulse_col_a, u_pulse_col_b, s1);
            prism = mix(prism, u_pulse_col_c, s2 * 0.65);

            float cometAng = -2.2 + pNorm * 6.8;
            float dAng = atan(sin(ang - cometAng), cos(ang - cometAng));
            float comet = exp(-pow(dAng / 0.65, 2.0)) * ringWave;

            vec2 spGrid = floor((localN + vec2(0.0, -pNorm * 0.25)) * 22.0);
            vec2 spF = fract((localN + vec2(0.0, -pNorm * 0.25)) * 22.0) - 0.5;
            float spH = gHash(spGrid);
            float sparkle = step(0.84, spH)
                          * smoothstep(0.26, 0.04, length(spF))
                          * (0.45 + 0.55 * sin(u_time * 14.0 + spH * 50.0))
                          * ringHalo * 1.65;

            float cornerWash = smoothstep(0.05, 1.05, localN.y) * pEnv * 0.42;
            vec3 washCol = mix(u_pulse_col_c, u_pulse_col_b, clamp(localN.x * 0.5 + 0.5, 0.0, 1.0));

            glassCol += prism * (ringHalo * 0.58 + ringWave * 0.72);
            glassCol += vec3(1.0, 0.96, 0.92) * (ringWave * 0.48 + comet * 0.95 + sparkle);
            glassCol += washCol * (cornerWash + rimLine * pEnv * 0.65);
        }

        float mask = 1.0 - smoothstep(-1.1, 1.1, d);
        outCol = mix(outCol, glassCol, mask);
    }

    fragColor = vec4(clamp(outCol, 0.0, 1.0), 1.0);
}
"#;

// Rich multi-color 3D-styled Studio Glass HUD icons (Fountain Pen, Terminal, Hourglass, Studio Mic, Headphones, Sun, Sliders, CheckBadge)
pub const HUD_ICON_VERT: &str = r#"#version 330 core
uniform vec2 u_res;
uniform vec4 u_rect; // x, y, w, h in CSS px (top-left origin)
out vec2 v_local;

void main() {
    vec2 c = vec2(float(gl_VertexID & 1), float((gl_VertexID >> 1) & 1));
    v_local = c;
    vec2 p = u_rect.xy + c * u_rect.zw;
    vec2 ndc = vec2((p.x / u_res.x) * 2.0 - 1.0, 1.0 - (p.y / u_res.y) * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

pub const HUD_ICON_FRAG: &str = r#"#version 330 core
in vec2 v_local;
out vec4 fragColor;

uniform int u_icon;     // 0=Pen, 1=Terminal, 2=Hourglass, 3=Mic, 4=Headphones, 5=Sun, 6=SlidersBtn, 7=CheckBadge, 8=GripDots
uniform vec4 u_params;  // x = burstProgress (0..1, 0 when idle), y = hover/darkFlag, z = extra, w = sizePx

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

float sdSegment(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a, ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

vec2 rot2(vec2 p, float a) {
    float c = cos(a), s = sin(a);
    return vec2(c * p.x - s * p.y, s * p.x + c * p.y);
}

void over(inout vec4 dst, vec3 col, float a) {
    a = clamp(a, 0.0, 1.0);
    dst.rgb = mix(dst.rgb, col, a);
    dst.a = a + dst.a * (1.0 - a);
}

void main() {
    vec2 q = v_local * 2.0 - 1.0; // -1..1 (y down!)
    float aa = 2.4 / max(14.0, u_params.w);
    float burst = clamp(u_params.x, 0.0, 1.0);
    vec4 outC = vec4(0.0);

    // 0: 3D Teal & Gold Fountain Pen (Morning Hero Card)
    if (u_icon == 0) {
        float sweep = (burst > 0.001 && burst < 0.999) ? sin(burst * 3.14159 * 2.0) * 0.22 : 0.0;
        vec2 p = rot2(q, -0.54 + sweep); // tilted 30deg like reference

        // Drop shadow
        float shD = sdRoundBox(p - vec2(0.04, 0.06), vec2(0.14, 0.62), 0.08);
        over(outC, vec3(0.0), exp(-max(shD, 0.0) * 7.5) * 0.42);

        // Gold finial cap
        float capD = sdRoundBox(p - vec2(0.0, -0.74), vec2(0.13, 0.065), 0.05);
        vec3 gold = mix(vec3(1.0, 0.91, 0.65), vec3(0.55, 0.38, 0.14), clamp((p.x + 0.14) / 0.28, 0.0, 1.0));
        over(outC, gold, 1.0 - smoothstep(-aa, aa, capD));

        // Teal anodized barrel with cylindrical specular highlight
        float barD = sdRoundBox(p - vec2(0.0, -0.22), vec2(0.155, 0.46), 0.03);
        vec3 teal = mix(vec3(0.54, 0.78, 0.82), vec3(0.11, 0.22, 0.25), clamp((p.x + 0.16) / 0.32, 0.0, 1.0));
        float hiBar = exp(-pow((p.x + 0.06) / 0.045, 2.0)) * 0.45;
        over(outC, teal + vec3(hiBar), 1.0 - smoothstep(-aa, aa, barD));

        // Gold pocket clip on right side
        float clipD = sdRoundBox(p - vec2(0.175, -0.40), vec2(0.038, 0.24), 0.025);
        over(outC, gold, 1.0 - smoothstep(-aa, aa, clipD));

        // Brushed steel grip collar & tapered section
        float ringD = sdRoundBox(p - vec2(0.0, 0.24), vec2(0.15, 0.055), 0.02);
        vec3 steel = mix(vec3(0.95, 0.97, 0.99), vec3(0.30, 0.34, 0.39), clamp((p.x + 0.15) / 0.30, 0.0, 1.0));
        over(outC, steel, 1.0 - smoothstep(-aa, aa, ringD));

        float secW = mix(0.13, 0.085, clamp((p.y - 0.28) / 0.22, 0.0, 1.0));
        float secD = max(abs(p.x) - secW, abs(p.y - 0.38) - 0.11);
        over(outC, steel * 0.92, 1.0 - smoothstep(-aa, aa, secD));

        // Golden fountain nib triangle
        float nibW = max(0.0, 0.092 * (1.0 - clamp((p.y - 0.48) / 0.34, 0.0, 1.0)));
        float nibD = max(abs(p.x) - nibW, abs(p.y - 0.65) - 0.17);
        over(outC, gold, 1.0 - smoothstep(-aa, aa, nibD));
        // Nib slit + breather hole
        float slit = sdSegment(p, vec2(0.0, 0.56), vec2(0.0, 0.78)) - 0.014;
        over(outC, vec3(0.22, 0.14, 0.05), 1.0 - smoothstep(-aa, aa, slit));

        // Fiery star flare ignited at nib tip during click burst!
        if (burst > 0.01 && burst < 0.98) {
            float env = sin(burst * 3.14159);
            vec2 tip = p - vec2(0.0, 0.80);
            float r = length(tip);
            float glow = exp(-r * 5.2) * env;
            float star = exp(-abs(tip.x) * 28.0) * exp(-abs(tip.y) * 5.0) + exp(-abs(tip.y) * 28.0) * exp(-abs(tip.x) * 5.0);
            over(outC, vec3(1.0, 0.45, 0.68), glow * 0.72);
            over(outC, vec3(1.0, 0.94, 0.76), clamp(star * env * 0.9 + exp(-r * 12.0) * env, 0.0, 1.0));
        }
    }
    // 1: Tilted 3D Terminal Window (Thunderstorm)
    else if (u_icon == 1) {
        float zap = (burst > 0.01 && burst < 0.98) ? sin(burst * 3.14159 * 3.0) * 0.12 : 0.0;
        vec2 p = rot2(q, 0.16 + zap);
        p.x += p.y * 0.08; // subtle perspective skew

        float boxD = sdRoundBox(p, vec2(0.74, 0.54), 0.16);
        // Electric cyan glow during burst
        if (burst > 0.01 && burst < 0.98) {
            float env = sin(burst * 3.14159);
            over(outC, vec3(0.38, 0.86, 1.0), exp(-max(boxD, 0.0) * 5.0) * env * 0.75);
        }
        vec3 bg = mix(vec3(0.24, 0.29, 0.34), vec3(0.06, 0.08, 0.10), clamp((p.x + p.y + 1.0) * 0.5, 0.0, 1.0));
        over(outC, bg, 1.0 - smoothstep(-aa, aa, boxD));
        // Metallic border
        over(outC, vec3(0.62, 0.72, 0.78), (1.0 - smoothstep(0.0, aa * 1.8, abs(boxD))) * 0.85);
        // Header divider
        float divD = max(abs(p.y + 0.28) - 0.02, abs(p.x) - 0.68);
        over(outC, vec3(1.0), (1.0 - smoothstep(-aa, aa, divD)) * 0.16);
        // Traffic light dots
        over(outC, vec3(1.0, 0.45, 0.41), 1.0 - smoothstep(-aa, aa, length(p - vec2(-0.52, -0.40)) - 0.06));
        over(outC, vec3(1.0, 0.82, 0.37), 1.0 - smoothstep(-aa, aa, length(p - vec2(-0.34, -0.40)) - 0.06));
        over(outC, vec3(0.36, 0.90, 0.54), 1.0 - smoothstep(-aa, aa, length(p - vec2(-0.16, -0.40)) - 0.06));
        // Prompt ">_"
        float gt = min(sdSegment(p, vec2(-0.46, -0.06), vec2(-0.22, 0.10)), sdSegment(p, vec2(-0.22, 0.10), vec2(-0.46, 0.26))) - 0.055;
        over(outC, vec3(0.85, 0.96, 1.0), 1.0 - smoothstep(-aa, aa, gt));
        float cur = sdRoundBox(p - vec2(0.04, 0.22), vec2(0.16, 0.05), 0.02);
        over(outC, vec3(0.50, 0.90, 1.0), 1.0 - smoothstep(-aa, aa, cur));
        // Lightning bolt flash on click!
        if (burst > 0.01 && burst < 0.85) {
            float fl = 0.5 + 0.5 * sin(burst * 25.0);
            float b1 = sdSegment(p, vec2(0.42, -0.22), vec2(0.24, 0.04)) - 0.055;
            float b2 = sdSegment(p, vec2(0.24, 0.04), vec2(0.46, 0.04)) - 0.05;
            float b3 = sdSegment(p, vec2(0.46, 0.04), vec2(0.26, 0.36)) - 0.055;
            over(outC, vec3(0.78, 0.96, 1.0), (1.0 - smoothstep(-aa, aa, min(b1, min(b2, b3)))) * fl);
        }
    }
    // 2: Brass & Amber Sand Hourglass (Sunset)
    else if (u_icon == 2) {
        float flip = (burst > 0.01 && burst < 0.99) ? smoothstep(0.0, 0.85, burst) * 3.14159265 : 0.0;
        vec2 p = rot2(q, flip);
        vec3 brass = mix(vec3(0.97, 0.85, 0.61), vec3(0.45, 0.29, 0.11), clamp((p.x + 0.55) / 1.1, 0.0, 1.0));

        // Top & bottom brass plates
        float capT = sdRoundBox(p - vec2(0.0, -0.72), vec2(0.48, 0.08), 0.04);
        float capB = sdRoundBox(p - vec2(0.0,  0.72), vec2(0.48, 0.08), 0.04);
        // Twin brass side rods
        float rodL = sdRoundBox(p - vec2(-0.37, 0.0), vec2(0.045, 0.68), 0.02);
        float rodR = sdRoundBox(p - vec2( 0.37, 0.0), vec2(0.045, 0.68), 0.02);

        // Glass bulbs (pinched waist at y=0)
        float bulbW = 0.05 + 0.28 * pow(clamp(abs(p.y) / 0.64, 0.0, 1.0), 0.72);
        float bulbD = max(abs(p.x) - bulbW, abs(p.y) - 0.64);
        over(outC, vec3(0.86, 0.92, 0.96), (1.0 - smoothstep(-aa, aa, bulbD)) * 0.26);
        over(outC, vec3(1.0, 0.95, 0.88), (1.0 - smoothstep(0.0, aa * 1.4, abs(bulbD))) * 0.55);

        // Glowing amber sand (top cone + bottom mound + thin falling stream)
        float topSand = max(bulbD + 0.04, max(-p.y - 0.42, p.y - 0.02));
        float botSand = max(bulbD + 0.04, 0.32 - p.y + abs(p.x) * 0.85);
        float stream = sdSegment(p, vec2(0.0, 0.0), vec2(0.0, 0.42)) - 0.025;
        vec3 sandCol = mix(vec3(1.0, 0.89, 0.58), vec3(0.90, 0.52, 0.20), clamp((p.y + 0.4) * 0.8, 0.0, 1.0));
        over(outC, sandCol, 1.0 - smoothstep(-aa, aa, min(topSand, min(botSand, stream))));

        over(outC, brass, 1.0 - smoothstep(-aa, aa, min(min(capT, capB), min(rodL, rodR))));
    }
    // 3: Chrome Studio Condenser Mic (Afternoon)
    else if (u_icon == 3) {
        float bounce = (burst > 0.01 && burst < 0.98) ? -sin(burst * 3.14159) * 0.14 : 0.0;
        vec2 p = q + vec2(0.0, -bounce);
        vec3 chrome = mix(vec3(1.0), vec3(0.28, 0.33, 0.38), clamp((p.x + 0.4) / 0.8, 0.0, 1.0));

        // Expanding cyan acoustic ring wave during burst
        if (burst > 0.01 && burst < 0.98) {
            float r = mix(0.32, 0.88, burst);
            float wD = abs(length(p - vec2(0.0, -0.24)) - r) - 0.035;
            over(outC, vec3(0.62, 0.94, 1.0), (1.0 - smoothstep(-aa, aa, wD)) * (1.0 - burst));
        }

        // Condenser capsule
        float capD = sdRoundBox(p - vec2(0.0, -0.26), vec2(0.32, 0.44), 0.30);
        over(outC, chrome, 1.0 - smoothstep(-aa, aa, capD));
        // Horizontal grille slats
        if (capD < 0.0) {
            float slats = abs(sin((p.y + 0.26) * 24.0));
            float slatMask = (1.0 - smoothstep(0.0, 0.26, slats)) * step(abs(p.x), 0.23) * step(abs(p.y + 0.28), 0.25);
            over(outC, vec3(0.18, 0.22, 0.26), slatMask * 0.72);
        }
        // U-yoke
        float yokeRing = abs(length(p - vec2(0.0, -0.10)) - 0.44) - 0.055;
        float yokeMask = step(-0.10, p.y);
        over(outC, chrome, (1.0 - smoothstep(-aa, aa, yokeRing)) * yokeMask);
        // Stand stem + oval base
        float stemD = sdSegment(p, vec2(0.0, 0.34), vec2(0.0, 0.66)) - 0.055;
        float baseD = length(vec2((p.x) / 0.38, (p.y - 0.70) / 0.11)) - 1.0;
        over(outC, chrome, 1.0 - smoothstep(-aa, aa, min(stemD, baseD * 0.2)));
    }
    // 4: Metallic Studio Headphones (Night)
    else if (u_icon == 4) {
        float sway = (burst > 0.01 && burst < 0.98) ? sin(burst * 3.14159 * 2.5) * 0.22 : 0.0;
        vec2 p = rot2(q, sway);
        vec3 metal = mix(vec3(0.90, 0.93, 0.96), vec3(0.24, 0.29, 0.35), clamp((p.x + p.y + 0.9) * 0.55, 0.0, 1.0));

        // Headband arch
        float archR = abs(length(p - vec2(0.0, 0.05)) - 0.58) - 0.085;
        float archM = step(p.y, 0.12);
        over(outC, metal, (1.0 - smoothstep(-aa, aa, archR)) * archM);
        // Left & right studio earcups
        float cupL = sdRoundBox(p - vec2(-0.56, 0.30), vec2(0.16, 0.31), 0.14);
        float cupR = sdRoundBox(p - vec2( 0.56, 0.30), vec2(0.16, 0.31), 0.14);
        over(outC, metal * 0.85, 1.0 - smoothstep(-aa, aa, min(cupL, cupR)));
        // Inner dark leather earpads
        float padL = sdRoundBox(p - vec2(-0.42, 0.30), vec2(0.075, 0.24), 0.06);
        float padR = sdRoundBox(p - vec2( 0.42, 0.30), vec2(0.075, 0.24), 0.06);
        over(outC, vec3(0.10, 0.12, 0.15), 1.0 - smoothstep(-aa, aa, min(padL, padR)));
    }
    // 5: Brightness Sun Icon
    else if (u_icon == 5) {
        vec3 col = (u_params.y > 0.5) ? vec3(0.96, 0.97, 0.99) : vec3(0.09, 0.10, 0.12);
        float core = abs(length(q) - 0.32) - 0.085;
        float d = core;
        for (int i = 0; i < 8; i++) {
            float a = float(i) * 0.78539816;
            vec2 dir = vec2(cos(a), sin(a));
            d = min(d, sdSegment(q, dir * 0.56, dir * 0.78) - 0.08);
        }
        over(outC, col, 1.0 - smoothstep(-aa, aa, d));
    }
    // 6: HUD Collapse Sliders Button (circular translucent glass button + sliders glyph)
    else if (u_icon == 6) {
        float cD = length(q) - 0.92;
        float bgA = (u_params.y > 0.5) ? 0.65 : 0.42;
        over(outC, vec3(0.10, 0.13, 0.16), (1.0 - smoothstep(-aa, aa, cD)) * bgA);
        over(outC, vec3(1.0), (1.0 - smoothstep(0.0, aa * 1.4, abs(cD))) * ((u_params.y > 0.5) ? 0.48 : 0.26));
        float l1 = sdSegment(q, vec2(-0.46, -0.22), vec2(0.46, -0.22)) - 0.055;
        float l2 = sdSegment(q, vec2(-0.46,  0.22), vec2(0.46,  0.22)) - 0.055;
        float k1 = length(q - vec2(-0.18, -0.22)) - 0.13;
        float k2 = length(q - vec2( 0.18,  0.22)) - 0.13;
        over(outC, vec3(0.95, 0.97, 1.0), 1.0 - smoothstep(-aa, aa, min(min(l1, l2), min(k1, k2))));
    }
    // 7: Active Checkmark White Circle Badge
    else if (u_icon == 7) {
        float cD = length(q) - 0.88;
        over(outC, vec3(0.96, 0.97, 0.98), (1.0 - smoothstep(-aa, aa, cD)) * 0.95);
        float ck = min(sdSegment(q, vec2(-0.36, 0.02), vec2(-0.08, 0.30)), sdSegment(q, vec2(-0.08, 0.30), vec2(0.40, -0.26))) - 0.11;
        over(outC, vec3(0.08, 0.10, 0.12), 1.0 - smoothstep(-aa, aa, ck));
    }
    // 8: 6-Dot Drag Grip Handle
    else if (u_icon == 8) {
        float d = 1e5;
        for (int ix = 0; ix < 2; ix++) {
            for (int iy = 0; iy < 3; iy++) {
                vec2 dp = vec2(-0.36 + float(ix) * 0.72, -0.58 + float(iy) * 0.58);
                d = min(d, length(q - dp) - 0.18);
            }
        }
        over(outC, vec3(1.0), (1.0 - smoothstep(-aa, aa, d)) * 0.62);
    }

    if (outC.a < 0.004) discard;
    fragColor = outC;
}
"#;
