use crate::state::AppState;
use crate::themes::common::PackedThemeParams;

// ================= 1. BEACH (Adaptive Sand Coastline + Shoal + Rolling Foam Waves + Deep Ocean) =================
pub const BEACH_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float warp = clamp(u_p0.y, 0.0, 80.0);
    float shoalW = max(10.0, u_p0.z);
    float shallowW = max(60.0, u_p0.w);

    // Distance from sand shoreline (negative = sand, positive = ocean)
    float d = iconField(px, sigma, warp, 64.0);
    // Gentle diagonal base coastline when no icons exist
    if (d > 1e4) {
        d = (px.x * 0.55 + px.y * 0.45) - (u_res.x * 0.28 + u_res.y * 0.16)
            + (fbm(px * 0.004 + u_seed * 0.01) - 0.5) * warp * 2.4;
    }

    // Warm golden sand texture
    float sandGrain = fbm(px * 0.045) * 0.07;
    vec3 sandDry = vec3(0.955, 0.915, 0.855) - sandGrain;
    vec3 sandWet = vec3(0.84, 0.77, 0.67) - sandGrain * 0.6;

    // Animated ocean waves surging up the beach
    float wavePhase = d * 0.045 - u_time * 1.35 + fbm(px * 0.012 + vec2(u_time * 0.12, 0.0)) * 2.6;
    float surge = sin(u_time * 0.9 + fbm(px * 0.006) * 3.0) * 14.0;
    float dWave = d - surge;

    vec3 col;
    if (dWave < 0.0) {
        float wet = smoothstep(-28.0, 0.0, dWave);
        col = mix(sandDry, sandWet, wet * 0.65);
    } else {
        float tShoal = clamp(dWave / shoalW, 0.0, 1.0);
        float tDeep = clamp((dWave - shoalW) / shallowW, 0.0, 1.0);
        vec3 cShoal = vec3(0.74, 0.93, 0.88);
        vec3 cTurq = vec3(0.32, 0.81, 0.82);
        vec3 cMid = vec3(0.06, 0.62, 0.77);
        vec3 cDeep = vec3(0.03, 0.44, 0.63);
        col = mix(cShoal, cTurq, smoothstep(0.0, 1.0, tShoal));
        col = mix(col, cMid, smoothstep(0.0, 0.65, tDeep));
        col = mix(col, cDeep, smoothstep(0.45, 1.0, tDeep));

        // Sunlit water caustics
        float caust = pow(fbm(px * 0.018 + vec2(u_time * 0.25, -u_time * 0.18)), 2.4);
        col += vec3(0.28, 0.42, 0.44) * caust * (1.0 - tDeep * 0.65) * 0.45;

        // Multi-band curling white surf foam
        float foam1 = smoothstep(0.76, 0.94, sin(wavePhase)) * exp(-dWave * 0.014);
        float foamEdge = (1.0 - smoothstep(0.0, 9.0, dWave)) * (0.72 + 0.28 * fbm(px * 0.05));
        float foam = clamp(foam1 * 0.78 + foamEdge * 0.92, 0.0, 1.0);
        col = mix(col, vec3(0.97, 0.99, 1.0), foam);
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_beach(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("sigma", 85.0),
            st.param_f32("warp", 34.0),
            st.param_f32("shoal", 110.0),
            st.param_f32("shallow", 260.0),
        ],
        ..Default::default()
    }
}

// ================= 2. MEADOW (Ink-and-Wash Spring Grass + Icon-Hugging Fluffy Clouds & Cloud Shadows) =================
pub const MEADOW_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float cover = clamp(u_p0.y, 0.0, 1.0);
    float t = u_time * max(0.0, u_p0.z);

    // Warm botanical sunlit meadow grass base
    float gNoise = fbm(px * 0.008 + u_seed * 0.01);
    float blades = fbm(vec2(px.x * 0.06 + sin(px.y * 0.02 + t * 0.8) * 0.4, px.y * 0.02));
    vec3 grassLight = vec3(0.76, 0.84, 0.51);
    vec3 grassMid = vec3(0.62, 0.75, 0.40);
    vec3 grassShadow = vec3(0.36, 0.54, 0.24);
    vec3 col = mix(grassMid, grassLight, smoothstep(0.25, 0.80, gNoise));
    col += (blades - 0.5) * 0.055;

    // Tiny wildflower speckles
    vec2 fc = floor(px / 34.0);
    vec2 fl = fract(px / 34.0);
    float fh = hash21(fc + u_seed * 0.02);
    if (fh > 0.86) {
        vec2 fp = hash22(fc * 2.3) * 0.6 + 0.2;
        float fd = length(fl - fp) * 34.0;
        vec3 fcol = (fh > 0.94) ? vec3(0.99, 0.96, 0.76) : vec3(0.98, 0.98, 0.99);
        col = mix(col, fcol, 1.0 - smoothstep(1.2, 2.4, fd));
    }

    // Ambient drifting cloud field + icon-hugging cloud puffs
    vec2 drift = vec2(t * 14.0, t * 4.0);
    float ambCloud = fbm((px - drift) * 0.0032 + u_seed * 0.03);
    float ambMask = smoothstep(0.84 - cover * 0.36, 0.92 - cover * 0.30, ambCloud);
    float ambSh = fbm((px - vec2(44.0, 56.0) - drift) * 0.0032 + u_seed * 0.03);
    float ambShMask = smoothstep(0.84 - cover * 0.36, 0.92 - cover * 0.30, ambSh);

    float iconC = iconField(px, sigma, 28.0 + sin(t * 0.4) * 4.0, 56.0);
    float iconSh = iconField(px - vec2(44.0, 56.0), sigma, 28.0 + sin(t * 0.4) * 4.0, 56.0);
    float icMask = 1.0 - smoothstep(-10.0, 12.0, iconC);
    float icShMask = 1.0 - smoothstep(-14.0, 18.0, iconSh);

    // Deep green cloud shadows cast onto meadow
    float shMask = max(ambShMask * 0.75, icShMask * 0.88);
    col = mix(col, grassShadow, shMask * 0.78);

    // Crisp ink-and-wash fluffy white clouds on top
    float cMask = max(ambMask, icMask);
    float rimShade = smoothstep(-28.0, 4.0, iconC) * icMask;
    vec3 cloudCol = mix(vec3(0.99, 0.995, 1.0), vec3(0.86, 0.90, 0.94), rimShade * 0.75);
    col = mix(col, cloudCol, cMask);

    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_meadow(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("sigma", 72.0),
            st.param_f32("cover", 0.30),
            st.param_f32("flow", 1.0),
            0.0,
        ],
        ..Default::default()
    }
}

// ================= 3. DESERT (Sahara Sunlit Sand Dunes + Raised Icon Hills + Turquoise Oases) =================
pub const DESERT_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float warp = clamp(u_p0.y, 0.0, 80.0);
    float warmth = clamp(u_p0.z, 0.0, 1.0);
    float t = u_time * max(0.0, u_p0.w);

    // Sculpted overhead Sahara dune ridges
    float ridge = sin(px.x * 0.0065 + px.y * 0.0042 + fbm(px * 0.003 + u_seed * 0.01) * 3.8);
    float ripple = sin((px.x * 0.14 - px.y * 0.08) + fbm(px * 0.02) * 3.0) * 0.5 + 0.5;
    vec3 sandLit = mix(vec3(0.93, 0.78, 0.54), vec3(0.98, 0.80, 0.48), warmth);
    vec3 sandSh = mix(vec3(0.71, 0.48, 0.28), vec3(0.76, 0.44, 0.22), warmth);
    vec3 col = mix(sandSh, sandLit, smoothstep(-0.6, 0.65, ridge));
    col += (ripple - 0.5) * 0.032;

    // Turquoise desert oasis pool nestled away from icons
    vec2 oasisC = vec2(u_res.x * 0.68, u_res.y * 0.54);
    float od = length((px - oasisC) * vec2(0.85, 1.25)) + (fbm(px * 0.015) - 0.5) * 44.0;
    float oWater = 1.0 - smoothstep(42.0, 62.0, od);
    float oGrass = (1.0 - smoothstep(60.0, 96.0, od)) * (1.0 - oWater);
    col = mix(col, vec3(0.32, 0.56, 0.30), oGrass * 0.82);
    vec3 waterC = mix(vec3(0.11, 0.65, 0.62), vec3(0.34, 0.84, 0.78), fbm(px * 0.03 + t * 0.25));
    col = mix(col, waterC, oWater);

    // Raised sunlit sand mounds under desktop icons
    float id = iconField(px, sigma, warp, 64.0);
    float idSh = iconField(px - vec2(18.0, 14.0), sigma, warp, 64.0);
    float mound = 1.0 - smoothstep(-24.0, 26.0, id);
    float moundSh = (1.0 - smoothstep(-10.0, 32.0, idSh)) * (1.0 - mound * 0.85);
    col = mix(col, sandSh * 0.78, moundSh * 0.55);
    col = mix(col, sandLit * 1.06, mound * 0.55);

    // Drifting windblown sand haze
    float haze = fbm(vec2(px.x * 0.004 - t * 0.35, px.y * 0.008)) * 0.14;
    col = mix(col, vec3(0.98, 0.87, 0.68), haze);
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_desert(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("sigma", 88.0),
            st.param_f32("warp", 36.0),
            st.param_f32("warmth", 0.78),
            st.param_f32("wind", 1.0),
        ],
        ..Default::default()
    }
}

// ================= 4. ROSEBROOK (Lush Grass + Shimmering Brook + Blooming Icon Roses + Petals) =================
pub const ROSEBROOK_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float t = u_time * max(0.0, u_p0.y);
    float sun = clamp(u_p0.z, 0.0, 1.0);
    float roses = clamp(u_p0.w, 0.0, 1.0);

    float gn = fbm(px * 0.009 + u_seed * 0.01);
    vec3 col = mix(vec3(0.16, 0.42, 0.22), vec3(0.36, 0.64, 0.30), smoothstep(0.2, 0.85, gn));

    // Shimmering winding crystal brook
    float brookX = u_res.x * 0.64 + sin(px.y * 0.0045 + u_seed * 0.02) * 95.0;
    float bd = abs(px.x - brookX) + (fbm(px * 0.014) - 0.5) * 26.0;
    float bMask = 1.0 - smoothstep(42.0, 62.0, bd);
    float shimmer = pow(fbm(vec2(px.x * 0.03, px.y * 0.02 - t * 0.75)), 2.2);
    vec3 brookCol = mix(vec3(0.14, 0.56, 0.66), vec3(0.54, 0.88, 0.92), shimmer);
    col = mix(col, brookCol, bMask * u_p1.x);

    // Lush floral rosebed halo framing desktop icons
    float id = iconField(px, sigma, 24.0, 48.0);
    float bed = 1.0 - smoothstep(-10.0, 20.0, id);
    float ring = exp(-abs(id + 6.0) * 0.08) * roses;
    col = mix(col, vec3(0.14, 0.34, 0.18), bed * 0.65);

    // Layered procedural blooming roses along icon perimeter
    vec2 rc = floor(px / 28.0);
    vec2 rl = fract(px / 28.0);
    float rh = hash21(rc + 4.1);
    if (ring > 0.22 && rh > 1.0 - roses * 0.65) {
        vec2 rp = hash22(rc * 1.9) * 0.5 + 0.25;
        float rd = length(rl - rp) * 28.0;
        float petal = 1.0 - smoothstep(3.5, 8.5, rd);
        vec3 rcol = (rh > 0.78) ? vec3(0.95, 0.36, 0.48) : (rh > 0.55 ? vec3(0.98, 0.64, 0.72) : vec3(0.86, 0.22, 0.34));
        col = mix(col, rcol, petal * clamp(ring * 1.3, 0.0, 1.0));
    }

    // Warm golden diagonal sunbeams
    float beam = pow(max(0.0, sin((px.x * 0.4 - px.y * 0.6) * 0.008 + t * 0.15)), 4.0);
    col += vec3(1.0, 0.92, 0.72) * beam * sun * 0.16;
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_rosebrook(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("sigma", 72.0),
            st.param_f32("flow", 1.0),
            st.param_f32("sunlight", 0.78),
            st.param_f32("roses", 0.85),
        ],
        p1: [st.param_f32("stream", 1.0), 0.0, 0.0, 0.0],
        ..Default::default()
    }
}

// ================= 5. GLADE (Sun-Dappled Forest Floor + Layered Canopy Parting Around Icons) =================
pub const GLADE_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float warp = clamp(u_p0.y, 0.0, 80.0);
    float dens = clamp(u_p0.z, 0.2, 1.8);
    float t = u_time * max(0.0, u_p0.w);

    // Sunlit mossy forest glade clearing
    float fn = fbm(px * 0.01 + u_seed * 0.01);
    vec3 moss = mix(vec3(0.32, 0.52, 0.24), vec3(0.56, 0.72, 0.34), smoothstep(0.25, 0.8, fn));
    float dapple = pow(fbm(px * 0.014 + vec2(sin(t * 0.4) * 0.3, cos(t * 0.3) * 0.2)), 2.0);
    moss += vec3(0.28, 0.30, 0.14) * dapple;

    // Dense layered forest canopy parting around desktop icons!
    float id = iconField(px, sigma, warp, 64.0);
    vec2 sway = vec2(sin(t * 0.9 + px.y * 0.01), cos(t * 0.7 + px.x * 0.01)) * 6.0;
    float leafN = fbm((px + sway) * 0.022 * dens);
    float canopy1 = smoothstep(-6.0, 46.0, id + (leafN - 0.5) * 44.0);
    float canopy2 = smoothstep(22.0, 86.0, id + (fbm((px - sway) * 0.03) - 0.5) * 38.0);

    vec3 col = moss;
    col = mix(col, vec3(0.11, 0.31, 0.18), canopy1 * 0.88);
    col = mix(col, vec3(0.06, 0.20, 0.12), canopy2 * 0.85);
    col += vec3(0.18, 0.36, 0.16) * smoothstep(0.55, 0.85, leafN) * canopy1 * 0.45;
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_glade(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("sigma", 78.0),
            st.param_f32("warp", 28.0),
            st.param_f32("density", 1.0),
            st.param_f32("wind", 1.0),
        ],
        ..Default::default()
    }
}

// ================= 6. ZEN ARCHIPELAGO (Japanese Pond, Stone Islands, Ripples, Koi & Fireflies) =================
pub const ZEN_ARCHIPELAGO_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time;
    float waveI = clamp(u_p0.x, 0.0, 2.0);
    float margin = clamp(u_p0.y, 18.0, 120.0);
    int tod = u_mode; // 0 Dawn/Twilight, 1 Day, 2 Sunset, 3 Night

    vec3 wDeep, wLit, mossC, stoneC;
    if (tod == 1) {
        wDeep = vec3(0.06, 0.24, 0.30); wLit = vec3(0.14, 0.44, 0.48); mossC = vec3(0.24, 0.46, 0.26); stoneC = vec3(0.45, 0.47, 0.46);
    } else if (tod == 2) {
        wDeep = vec3(0.12, 0.10, 0.18); wLit = vec3(0.46, 0.26, 0.24); mossC = vec3(0.22, 0.32, 0.20); stoneC = vec3(0.36, 0.32, 0.34);
    } else if (tod == 3) {
        wDeep = vec3(0.03, 0.07, 0.12); wLit = vec3(0.08, 0.18, 0.26); mossC = vec3(0.12, 0.26, 0.18); stoneC = vec3(0.26, 0.30, 0.34);
    } else {
        wDeep = vec3(0.04, 0.12, 0.18); wLit = vec3(0.12, 0.28, 0.36); mossC = vec3(0.18, 0.38, 0.24); stoneC = vec3(0.34, 0.38, 0.40);
    }

    float wn = fbm(px * 0.012 + vec2(t * 0.15, -t * 0.10));
    vec3 col = mix(wDeep, wLit, smoothstep(0.25, 0.85, wn));

    // Gentle concentric water ripples around desktop icon islands
    float id = iconField(px, margin * 1.4, 18.0, 40.0);
    if (id > 0.0 && id < 180.0) {
        float rip = sin(id * 0.18 - t * 2.1) * exp(-id * 0.015) * waveI;
        col += vec3(0.16, 0.28, 0.32) * max(0.0, rip) * 0.55;
    }

    // Swimming orange/white Koi fish gliding in the pond
    for (int k = 0; k < 6; k++) {
        float fk = float(k);
        float ang = t * (0.22 + fk * 0.04) + fk * 1.2;
        vec2 kp = vec2(
            u_res.x * (0.22 + 0.56 * hash11(fk * 5.3) + cos(ang) * 0.16),
            u_res.y * (0.20 + 0.60 * hash11(fk * 8.7) + sin(ang * 0.8) * 0.16)
        );
        float kd = length(px - kp);
        vec3 kcol = (mod(fk, 2.0) < 0.5) ? vec3(0.98, 0.46, 0.20) : vec3(0.96, 0.94, 0.88);
        col = mix(col, kcol, (1.0 - smoothstep(3.0, 9.0, kd)) * smoothstep(-2.0, 14.0, id));
    }

    // Zen moss & stone islands hugging desktop icons
    float isl = 1.0 - smoothstep(-2.0, 3.0, id);
    float rim = (1.0 - smoothstep(0.0, 6.0, abs(id))) * 0.85;
    col = mix(col, mix(stoneC, mossC, fbm(px * 0.04)), isl);
    col = mix(col, stoneC * 1.15, rim * 0.55);

    // Floating warm fireflies & sakura petals
    vec2 fc = floor(px / 95.0);
    vec2 fl = fract(px / 95.0);
    float fh = hash21(fc + u_seed * 0.01);
    if (fh > 0.65) {
        vec2 fp = hash22(fc * 2.7) * 0.6 + 0.2 + vec2(sin(t * 0.8 + fh * 6.0), cos(t * 0.6 + fh * 9.0)) * 0.14;
        float fd = length(fl - fp) * 95.0;
        float glow = exp(-fd * 0.22) * (0.5 + 0.5 * sin(t * 2.5 + fh * 12.0));
        col += vec3(1.0, 0.86, 0.45) * glow * 0.85;
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_zen_archipelago(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        mode: st.param_f32("timeOfDay", 0.0).round() as i32,
        p0: [
            st.param_f32("waveIntensity", 1.0),
            st.param_f32("islandMargin", 44.0),
            st.param_f32("bridgeDistance", 230.0),
            st.param_f32("sakuraDensity", 1.0),
        ],
        ..Default::default()
    }
}
