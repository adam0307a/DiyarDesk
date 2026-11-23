use crate::state::AppState;
use crate::themes::common::PackedThemeParams;

// ================= 1. AURORA (Top-Down Frozen Sapphire Fjord + Glowing Northern Lights + Snow Islands) =================
pub const AURORA_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float warp = clamp(u_p0.y, 0.0, 80.0);
    float shoal = max(12.0, u_p0.z);
    float aur = clamp(u_p0.w, 0.0, 1.2);
    float t = u_time * max(0.0, u_p1.x);

    // Deep dark sapphire frozen lake ice
    vec3 iceDeep = vec3(0.028, 0.075, 0.155);
    vec3 iceMid = vec3(0.055, 0.155, 0.255);
    float iceN = fbm(px * 0.008 + u_seed * 0.01);
    vec3 col = mix(iceDeep, iceMid, iceN);

    // Flowing emerald / cyan / violet aurora curtains reflected in ice
    float band1 = sin(px.x * 0.0042 + fbm(px * 0.003 + vec2(t * 0.12, 0.0)) * 5.0 - t * 0.35);
    float band2 = sin(px.x * 0.0028 - px.y * 0.0025 + fbm(px * 0.004 - vec2(0.0, t * 0.1)) * 4.5 + t * 0.25);
    float a1 = pow(clamp(1.0 - abs(band1), 0.0, 1.0), 2.8);
    float a2 = pow(clamp(1.0 - abs(band2), 0.0, 1.0), 3.0);
    col += vec3(0.16, 0.92, 0.64) * a1 * aur * 0.72;
    col += vec3(0.14, 0.72, 0.88) * a2 * aur * 0.45;
    col += vec3(0.56, 0.36, 0.94) * pow(a1 * a2, 0.7) * aur * 0.55;

    // Snow islands cradling desktop icons + icy turquoise shoal rim
    float id = iconField(px, sigma, warp, 58.0);
    float shoalA = smoothstep(shoal, 0.0, id) * smoothstep(-4.0, 6.0, id);
    col = mix(col, vec3(0.42, 0.78, 0.90), shoalA * 0.45);
    float snowMask = 1.0 - smoothstep(-3.0, 3.0, id);
    vec3 snowCol = mix(vec3(0.20, 0.28, 0.42), vec3(0.32, 0.42, 0.56), fbm(px * 0.02));
    col = mix(col, snowCol, snowMask);
    // Warm lantern glow on snow shoreline
    if (u_adaptive == 1 && u_icon_count > 0) {
        vec2 lc = u_icons[0].xy + vec2(u_icons[0].z + 18.0, u_icons[0].w * 0.5);
        col += vec3(1.0, 0.76, 0.36) * exp(-length(px - lc) / 95.0) * 0.55;
    }

    // Gentle falling snow particles
    float snowAmt = clamp(u_p1.y, 0.0, 1.0);
    if (snowAmt > 0.02) {
        vec2 sc = floor((px - vec2(sin(t) * 18.0, t * 48.0)) / 58.0);
        vec2 sl = fract((px - vec2(sin(t) * 18.0, t * 48.0)) / 58.0);
        float sh = hash21(sc);
        if (sh > 1.0 - snowAmt * 0.45) {
            float sd = length(sl - hash22(sc * 2.3) * 0.6 - 0.2) * 58.0;
            col += vec3(0.92, 0.96, 1.0) * (1.0 - smoothstep(0.8, 2.2, sd)) * 0.75;
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_aurora(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("sigma", 82.0),
            st.param_f32("warp", 32.0),
            st.param_f32("shoal", 90.0),
            st.param_f32("aurora", 0.78),
        ],
        p1: [st.param_f32("flow", 1.0), st.param_f32("snow", 0.45), 0.0, 0.0],
        ..Default::default()
    }
}

// ================= 2. POLAR NIGHT (Side-View Lake Horizon, Triple Aurora Ribbons, Snow Clearing & Cabin) =================
pub const POLARNIGHT_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.0, u_p0.w);
    float glow = clamp(u_p0.z, 0.0, 1.4);
    float horizon = u_res.y * 0.62;
    bool isLake = px.y > horizon;
    float sy = isLake ? (horizon - (px.y - horizon) * 0.85) : px.y;

    // Deep arctic sky gradient
    vec3 skyTop = vec3(0.015, 0.032, 0.075);
    vec3 skyHor = vec3(0.045, 0.115, 0.195);
    vec3 col = mix(skyTop, skyHor, clamp(sy / max(1.0, horizon), 0.0, 1.0));

    // Crisp arctic stars above horizon
    if (!isLake) {
        vec2 sc = floor(px / 48.0);
        vec2 sl = fract(px / 48.0);
        float sh = hash21(sc + u_seed * 0.01);
        if (sh > 0.72) {
            float sd = length(sl - hash22(sc * 3.1) * 0.7 - 0.15) * 48.0;
            float tw = 0.4 + 0.6 * sin(t * (1.0 + sh * 3.0) + sh * 20.0);
            col += vec3(0.88, 0.94, 1.0) * (1.0 - smoothstep(0.4, 1.5, sd)) * tw;
        }
    }

    // Triple vertical volumetric aurora curtains
    vec3 c1 = (u_palette == 1) ? vec3(0.75, 0.35, 0.95) : vec3(0.18, 0.94, 0.62);
    vec3 c2 = (u_palette == 1) ? vec3(0.98, 0.46, 0.66) : vec3(0.16, 0.74, 0.92);
    for (int i = 0; i < 3; i++) {
        float fi = float(i);
        float ry = horizon * (0.28 + fi * 0.14)
            + sin(px.x * (0.0032 + fi * 0.001) + t * (0.35 + fi * 0.1) + fi * 2.1) * 42.0;
        float distY = sy - ry;
        float curtain = exp(-max(-distY, 0.0) * 0.011) * exp(-max(distY, 0.0) * 0.065);
        float rays = 0.55 + 0.45 * sin(px.x * 0.045 + fbm(vec2(px.x * 0.01, t * 0.2 + fi)) * 6.0);
        vec3 ac = mix(c1, c2, clamp((horizon - sy) / horizon, 0.0, 1.0));
        col += ac * curtain * rays * glow * (isLake ? 0.45 : 0.72);
    }

    if (isLake) {
        float ripple = sin(px.y * 0.25 + t * 1.4 + fbm(px * 0.02) * 3.0) * 0.06;
        col = mix(col, vec3(0.03, 0.07, 0.14), 0.32) + ripple * 0.25;
    }

    // Mountain silhouette along horizon + warm shore cabin glow
    float mt = horizon - (fbm(vec2(px.x * 0.004 + u_seed * 0.02, 1.0)) * 75.0 + 8.0);
    if (!isLake && px.y > mt) {
        col = mix(vec3(0.03, 0.06, 0.11), vec3(0.08, 0.14, 0.22), clamp((px.y - mt) / 70.0, 0.0, 1.0));
    }
    if (u_p1.x > 0.5) {
        vec2 cab = vec2(u_res.x * 0.78, horizon - 12.0);
        col += vec3(1.0, 0.68, 0.28) * exp(-length(px - cab) / 70.0) * 0.62;
    }

    // Subtle snow frost frame around desktop icons
    float id = iconBoxField(px, 10.0, 16.0, 18.0);
    col = mix(col, vec3(0.76, 0.88, 0.98), (1.0 - smoothstep(-1.0, 1.5, abs(id))) * 0.26);
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_polarnight(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        palette: st.param_f32("palette", 0.0).round() as i32,
        p0: [
            st.param_f32("sigma", 90.0),
            st.param_f32("density", 0.60),
            st.param_f32("glow", 0.75),
            st.param_f32("flow", 1.0),
        ],
        p1: [st.param_f32("cabin", 1.0), 0.0, 0.0, 0.0],
        ..Default::default()
    }
}

// ================= 3. QUIETSNOW (4 Seasonal Modes: Snow, Raindrops, Autumn Leaves, Sakura + Icon Snow Caps) =================
pub const QUIETSNOW_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time;
    int mode = u_mode;    // 0 Snow, 1 Rain, 2 Autumn, 3 Sakura
    int atm = u_palette;  // 0 Quiet Night, 1 Blue Hour, 2 Misty Dawn, 3 Warm Streetlamp
    float inten = clamp(u_p0.x, 0.15, 1.5);
    float wind = u_p0.y + sin(t * 0.6) * u_p0.z * 0.6;

    vec3 topC, botC;
    if (atm == 1) { topC = vec3(0.05, 0.11, 0.24); botC = vec3(0.16, 0.28, 0.45); }
    else if (atm == 2) { topC = vec3(0.16, 0.22, 0.30); botC = vec3(0.48, 0.56, 0.64); }
    else if (atm == 3) { topC = vec3(0.08, 0.07, 0.10); botC = vec3(0.24, 0.17, 0.12); }
    else { topC = vec3(0.03, 0.06, 0.11); botC = vec3(0.11, 0.20, 0.31); }

    vec3 col = mix(topC, botC, clamp(1.0 - v_uv.y, 0.0, 1.0));
    if (atm == 3) {
        vec2 lamp = vec2(u_res.x * 0.82, u_res.y * 0.18);
        col += vec3(1.0, 0.78, 0.45) * exp(-length(px - lamp) / (u_res.y * 0.45)) * 0.45;
    }

    // Distant misty mountains & treeline backdrop
    float mt = u_res.y * 0.74 - fbm(vec2(px.x * 0.003 + 2.0, 0.5)) * 110.0;
    if (px.y > mt) {
        col = mix(col, topC * 1.18, u_p1.z * 0.55);
    }

    // Ground snow/leaf drift bank
    float depth = clamp(u_p1.x, 0.3, 1.4);
    float groundY = u_res.y - (26.0 + fbm(vec2(px.x * 0.004, 1.0)) * 34.0) * depth;
    if (u_p0.w > 0.5 && px.y > groundY) {
        vec3 bankCol = (mode == 2) ? vec3(0.76, 0.36, 0.16) : (mode == 3 ? vec3(0.96, 0.82, 0.88) : vec3(0.90, 0.94, 0.99));
        col = mix(col, bankCol, smoothstep(groundY, groundY + 12.0, px.y));
    }

    // Snow/petal caps resting neatly on top of every desktop icon!
    if (u_adaptive == 1 && u_p0.w > 1.5 && u_icon_count > 0) {
        vec3 capCol = (mode == 2) ? vec3(0.84, 0.42, 0.18) : (mode == 3 ? vec3(0.98, 0.84, 0.90) : vec3(0.94, 0.97, 1.0));
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            float capH = 9.0 * depth;
            vec2 capC = vec2(ic.x + ic.z * 0.5, ic.y - capH * 0.35);
            float cd = sdRoundBox(px - capC, vec2(ic.z * 0.46, capH * 0.6), capH * 0.55);
            col = mix(col, capCol, 1.0 - smoothstep(-1.0, 1.5, cd));
        }
    }

    // 3 parallax layers of falling snow / rain / autumn leaves / sakura petals
    for (int layer = 0; layer < 3; layer++) {
        float fl = float(layer);
        float cellSz = mix(34.0, 74.0, fl / 2.0);
        float fallSp = (mode == 1 ? 420.0 : (55.0 + fl * 32.0));
        vec2 vel = vec2(wind * (45.0 + fl * 25.0), fallSp);
        vec2 spx = px - vel * t;
        vec2 cell = floor(spx / cellSz);
        vec2 loc = fract(spx / cellSz);
        float h = hash21(cell + fl * 19.7);
        if (h > 1.0 - clamp(0.32 * inten, 0.08, 0.82)) {
            vec2 pt = hash22(cell * 2.1 + fl) * 0.65 + 0.17;
            pt.x += sin(t * (1.2 + h * 2.0) + h * 9.0) * (mode == 1 ? 0.0 : 0.12);
            vec2 dvec = (loc - pt) * cellSz;
            float a;
            vec3 pcol;
            if (mode == 1) {
                a = (1.0 - smoothstep(0.6, 1.8, abs(dvec.x))) * (1.0 - smoothstep(4.0, 14.0, abs(dvec.y)));
                pcol = vec3(0.76, 0.88, 1.0);
            } else if (mode == 2) {
                a = 1.0 - smoothstep(2.5, 4.8, length(dvec * vec2(1.0, 1.5)));
                pcol = (h > 0.85) ? vec3(0.95, 0.58, 0.18) : vec3(0.84, 0.28, 0.14);
            } else if (mode == 3) {
                a = 1.0 - smoothstep(2.0, 4.4, length(dvec * vec2(1.2, 0.85)));
                pcol = vec3(1.0, 0.78, 0.86);
            } else {
                float r = 1.5 + fl * 1.1;
                a = 1.0 - smoothstep(r * 0.4, r + 1.0, length(dvec));
                pcol = vec3(0.96, 0.98, 1.0);
            }
            col = mix(col, pcol, clamp(a * (0.5 + 0.25 * fl), 0.0, 1.0));
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_quietsnow(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        mode: st.param_f32("mode", 0.0).round() as i32,
        palette: st.param_f32("atmosphere", 0.0).round() as i32,
        p0: [
            st.param_f32("intensity", 0.85),
            st.param_f32("wind", 0.22),
            st.param_f32("gusts", 0.45),
            st.param_f32("accumulate", 2.0),
        ],
        p1: [
            st.param_f32("depth", 0.85),
            st.param_f32("sparkle", 0.55),
            st.param_f32("backdrop", 0.75),
            0.0,
        ],
        ..Default::default()
    }
}

// ================= 4. WINTER (Cozy Mountain Cabin + Warm Window Glow + Chimney Smoke + Live Snow) =================
pub const WINTER_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p1.y);
    vec3 col = mix(vec3(0.04, 0.08, 0.16), vec3(0.16, 0.25, 0.38), clamp(1.0 - v_uv.y, 0.0, 1.0));

    // Distant snowy Alpine mountain peaks
    float m1 = u_res.y * 0.62 - abs(sin(px.x * 0.0028 + 0.5)) * 135.0 * u_p0.w - fbm(vec2(px.x * 0.007, 1.0)) * 45.0;
    if (px.y > m1) col = mix(vec3(0.14, 0.21, 0.32), vec3(0.28, 0.36, 0.48), smoothstep(m1, m1 + 35.0, px.y));

    // Snowy foreground hill
    float hill = u_res.y * 0.76 + sin(px.x * 0.003) * 28.0;
    if (px.y > hill) {
        col = mix(vec3(0.76, 0.84, 0.94), vec3(0.58, 0.68, 0.82), clamp((px.y - hill) / (u_res.y * 0.25), 0.0, 1.0));
    }

    // Cozy wooden cabin with glowing golden windows on the snow hill
    vec2 cab = vec2(u_res.x * 0.72, hill - 22.0);
    float bodyD = sdRoundBox(px - cab, vec2(46.0, 26.0), 3.0);
    if (bodyD < 0.0) col = vec3(0.18, 0.11, 0.08);
    // Warm window glow spilling across the snow
    float wGlow = exp(-length(px - (cab + vec2(0.0, 8.0))) / 110.0);
    col += vec3(1.0, 0.68, 0.28) * wGlow * 0.55;
    float win1 = sdRoundBox(px - (cab + vec2(-16.0, 2.0)), vec2(9.0, 9.0), 1.5);
    float win2 = sdRoundBox(px - (cab + vec2(16.0, 2.0)), vec2(9.0, 9.0), 1.5);
    col = mix(col, vec3(1.0, 0.88, 0.55), 1.0 - smoothstep(-0.6, 0.8, min(win1, win2)));

    // Rising chimney smoke puff ribbon
    vec2 smk = px - (cab + vec2(26.0, -34.0));
    if (smk.y < 0.0 && smk.y > -200.0) {
        float driftX = sin(smk.y * 0.04 + t * 1.4) * 18.0 - smk.y * 0.25;
        float sd = abs(smk.x - driftX) / (12.0 - smk.y * 0.16);
        float sa = exp(-sd * sd) * (1.0 + smk.y / 200.0) * 0.35 * u_p1.x;
        col = mix(col, vec3(0.84, 0.88, 0.94), sa);
    }

    // Snow ledges resting on desktop icons
    if (u_adaptive == 1 && u_icon_count > 0) {
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            float cd = sdRoundBox(px - vec2(ic.x + ic.z * 0.5, ic.y - 3.0), vec2(ic.z * 0.45, 5.0), 4.5);
            col = mix(col, vec3(0.95, 0.98, 1.0), 1.0 - smoothstep(-0.8, 1.2, cd));
        }
    }

    // Layered parallax snowfall
    for (int l = 0; l < 2; l++) {
        float fl = float(l);
        float csz = mix(42.0, 68.0, fl);
        vec2 spx = px - vec2(u_p0.z * 110.0 * t, (65.0 + fl * 40.0) * u_p0.y * t);
        vec2 cell = floor(spx / csz);
        vec2 loc = fract(spx / csz);
        float h = hash21(cell + fl * 13.0);
        if (h > 1.0 - clamp(0.36 * u_p0.x, 0.08, 0.8)) {
            float sd = length((loc - hash22(cell * 2.4) * 0.65 - 0.17) * csz);
            col = mix(col, vec3(0.96, 0.98, 1.0), (1.0 - smoothstep(1.0, 2.8 + fl, sd)) * 0.85);
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_winter(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("snowIntensity", 0.75),
            st.param_f32("snowSpeed", 0.85),
            st.param_f32("windStrength", 0.18),
            st.param_f32("mountainVisibility", 0.85),
        ],
        p1: [
            st.param_f32("smokeAmount", 0.85),
            st.param_f32("animSpeed", 1.0),
            0.0, 0.0,
        ],
        ..Default::default()
    }
}
