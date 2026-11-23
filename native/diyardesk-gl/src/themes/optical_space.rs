use crate::state::AppState;
use crate::themes::common::{str_index, PackedThemeParams};

// ================= 1. LIQUID GLASS (Snell's Law Refraction + Chromatic Dispersion + smin Droplets) =================
pub const LIQUIDGLASS_BODY: &str = r#"
vec3 liquidBackdrop(vec2 uv, float t, int pal) {
    float asp = u_res.x / max(1.0, u_res.y);
    vec2 p = (uv - 0.5) * vec2(asp, 1.0);
    if (pal == 4) {
        vec3 col = vec3(0.043, 0.145, 0.271);
        vec3 lb[4];
        lb[0] = vec3(0.220, 0.741, 0.973);
        lb[1] = vec3(0.176, 0.831, 0.749);
        lb[2] = vec3(0.231, 0.510, 0.965);
        lb[3] = vec3(0.729, 0.902, 0.992);
        for (int i = 0; i < 4; i++) {
            float fi = float(i);
            vec2 c = vec2(0.18 + 0.22 * fi + sin(t * (0.42 + fi * 0.11) + fi * 1.3) * 0.18,
                          0.50 + cos(t * (0.36 + fi * 0.13) + fi * 1.7) * 0.34);
            float a = clamp(1.0 - length((uv - c) * vec2(asp, 1.0)) / (0.48 + 0.08 * fi), 0.0, 1.0);
            col = mix(col, lb[i], a * a * (3.0 - 2.0 * a) * 0.62);
        }
        return col;
    }
    vec2 q = p;
    q.x += sin(p.y * 2.6 + t * 0.45) * 0.22;
    q.y += cos(p.x * 2.2 - t * 0.38) * 0.20;
    float w1 = sin(q.x * 3.1 + q.y * 2.2 + t * 0.32) * 0.5 + 0.5;
    float w2 = sin(q.x * 1.8 - q.y * 3.4 - t * 0.26) * 0.5 + 0.5;
    float rib = pow(clamp(1.0 - abs(sin(q.y * 5.2 + q.x * 2.1 + t * 0.4)), 0.0, 1.0), 3.0);

    vec3 c0, c1, c2, hi;
    if (pal == 1) {
        c0 = vec3(0.03, 0.10, 0.16); c1 = vec3(0.08, 0.52, 0.48); c2 = vec3(0.32, 0.24, 0.72); hi = vec3(0.45, 0.96, 0.80);
    } else if (pal == 2) {
        c0 = vec3(0.11, 0.06, 0.20); c1 = vec3(0.84, 0.34, 0.58); c2 = vec3(0.24, 0.60, 0.96); hi = vec3(1.00, 0.82, 0.68);
    } else if (pal == 3) {
        c0 = vec3(0.04, 0.05, 0.07); c1 = vec3(0.12, 0.15, 0.20); c2 = vec3(0.22, 0.27, 0.34); hi = vec3(0.76, 0.84, 0.96);
    } else {
        c0 = vec3(0.05, 0.17, 0.34); c1 = vec3(0.11, 0.52, 0.82); c2 = vec3(0.16, 0.78, 0.84); hi = vec3(0.82, 0.95, 1.00);
    }
    vec3 col = mix(c0, c1, smoothstep(0.15, 0.85, w1));
    col = mix(col, c2, smoothstep(0.2, 0.9, w2) * 0.65);
    col += hi * rib * 0.24;
    return clamp(col, 0.0, 1.0);
}

float liquidSDF(vec2 px, float t) {
    float k = mix(12.0, 54.0, u_p0.w);
    float d = 1e5;
    int drops = int(clamp(round(u_p1.x * 7.0), 1.0, 9.0));
    for (int i = 0; i < 9; i++) {
        if (i >= drops) break;
        float fi = float(i);
        float sp = (0.28 + fi * 0.06) * max(0.05, u_p1.y);
        vec2 c = vec2(
            u_res.x * (0.14 + 0.72 * hash11(fi * 7.3 + 1.1) + sin(t * sp + fi * 1.9) * 0.12),
            u_res.y * (0.16 + 0.68 * hash11(fi * 11.7 + 3.4) + cos(t * sp * 0.82 + fi * 2.4) * 0.14)
        );
        float rw = 44.0 + hash11(fi * 5.1) * 58.0;
        float rh = (mod(fi, 2.0) < 0.5) ? rw : rw * 0.62;
        float sd = sdRoundBox(px - c, vec2(rw, rh), min(rw, rh) * 0.82);
        d = (d > 1e4) ? sd : smin(d, sd, k);
    }
    if (u_pointer_on == 1 && u_p1.z > 0.5) {
        float sd = sdRoundBox(px - u_pointer, vec2(74.0, 48.0), 42.0);
        d = (d > 1e4) ? sd : smin(d, sd, k * 1.25);
    }
    if (u_adaptive == 1 && u_icon_count > 0) {
        float ik = k * (u_mode == 2 ? 0.35 : 0.72);
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 c = ic.xy + ic.zw * 0.5;
            vec2 hs = ic.zw * 0.5 + vec2(10.0, 8.0);
            float rad = (u_mode == 1) ? min(hs.x, hs.y) * 0.92 : (u_mode == 2 ? 12.0 : min(hs.x, hs.y) * 0.45);
            float sd = sdRoundBox(px - c, hs, rad);
            d = (d > 1e4) ? sd : smin(d, sd, ik);
        }
    }
    return d;
}

void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p1.y);
    float d = liquidSDF(px, t);
    vec3 bg = liquidBackdrop(v_uv, t, u_palette);

    // Drop shadow under glass
    float ds = liquidSDF(px - vec2(0.0, 10.0), t);
    float sh = exp(-max(ds, 0.0) * 0.055) * 0.32;
    bg *= (1.0 - sh * smoothstep(-2.0, 4.0, d));

    if (d < 2.0) {
        vec2 eps = vec2(2.0, 0.0);
        vec2 grad = normalize(vec2(
            liquidSDF(px + eps.xy, t) - liquidSDF(px - eps.xy, t),
            liquidSDF(px + eps.yx, t) - liquidSDF(px - eps.yx, t)
        ) + vec2(1e-5));
        float depth = clamp(-d / 34.0, 0.0, 1.0);
        float bezel = pow(1.0 - depth, 2.2);
        float refr = u_p0.x * 0.045 * bezel;
        float disp = u_p0.z * 0.010 * bezel;

        vec2 uvG = clamp(v_uv - grad * refr, 0.002, 0.998);
        vec2 uvR = clamp(v_uv - grad * (refr + disp), 0.002, 0.998);
        vec2 uvB = clamp(v_uv - grad * (refr - disp), 0.002, 0.998);

        // Multi-tap optical frost inside glass
        float frost = u_p0.y * 0.012;
        vec3 rC = (liquidBackdrop(uvR, t, u_palette) + liquidBackdrop(uvR + vec2(frost, frost), t, u_palette) + liquidBackdrop(uvR - vec2(frost, -frost), t, u_palette)) / 3.0;
        vec3 gC = (liquidBackdrop(uvG, t, u_palette) + liquidBackdrop(uvG + vec2(-frost, frost), t, u_palette) + liquidBackdrop(uvG + vec2(frost, -frost), t, u_palette)) / 3.0;
        vec3 bC = (liquidBackdrop(uvB, t, u_palette) + liquidBackdrop(uvB - vec2(frost, frost), t, u_palette) + liquidBackdrop(uvB + vec2(0.0, frost), t, u_palette)) / 3.0;
        vec3 glass = vec3(rC.r, gC.g, bC.b);

        // Dual-angle specular rim glare & Fresnel highlight
        float spec1 = pow(clamp(dot(grad, normalize(vec2(-0.65, -0.76))), 0.0, 1.0), 4.0);
        float spec2 = pow(clamp(dot(grad, normalize(vec2(0.65, 0.76))), 0.0, 1.0), 5.0);
        float rim = smoothstep(14.0, 0.0, -d);
        glass = mix(glass, vec3(1.0), 0.08 + rim * 0.24 + (spec1 * 0.55 + spec2 * 0.30) * rim);
        float mask = 1.0 - smoothstep(-1.2, 1.2, d);
        bg = mix(bg, glass, mask);
    }
    fragColor = vec4(dither(bg), 1.0);
}
"#;

pub fn pack_liquidglass(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        palette: str_index(st, "palette", &["tahoe", "aurora", "prism", "obsidian", "classic"], 0),
        mode: str_index(st, "iconGlassStyle", &["capsule", "droplet", "prism"], 0),
        p0: [
            st.param_f32("refraction", 1.0),
            st.param_f32("frostAmount", 0.65),
            st.param_f32("dispersion", 0.95),
            st.param_f32("mergeRate", 0.55),
        ],
        p1: [
            st.param_f32("dropletCount", 1.0),
            st.param_f32("flowSpeed", 0.85),
            if st.param_bool("cursorLens", true) { 1.0 } else { 0.0 },
            0.0,
        ],
        ..Default::default()
    }
}

// ================= 2. THINKING ORBS (3D Fibonacci Volumetric Sphere + Icon Energy Links) =================
pub const THINKING_ORBS_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p0.z);

    vec3 bg0, bg1, dotCol, accent;
    if (u_palette == 1) {
        bg0 = vec3(0.03, 0.06, 0.12); bg1 = vec3(0.05, 0.14, 0.20); dotCol = vec3(0.32, 0.92, 0.78); accent = vec3(0.56, 0.44, 0.98);
    } else if (u_palette == 2) {
        bg0 = vec3(0.07, 0.04, 0.03); bg1 = vec3(0.15, 0.08, 0.04); dotCol = vec3(1.00, 0.76, 0.36); accent = vec3(0.96, 0.46, 0.24);
    } else if (u_palette == 3) {
        bg0 = vec3(0.02, 0.05, 0.12); bg1 = vec3(0.06, 0.14, 0.28); dotCol = vec3(0.42, 0.76, 1.00); accent = vec3(0.24, 0.48, 0.98);
    } else if (u_palette == 4) {
        bg0 = vec3(0.95, 0.96, 0.97); bg1 = vec3(0.88, 0.90, 0.93); dotCol = vec3(0.12, 0.14, 0.18); accent = vec3(0.28, 0.36, 0.52);
    } else {
        bg0 = vec3(0.04, 0.045, 0.06); bg1 = vec3(0.09, 0.10, 0.13); dotCol = vec3(0.92, 0.94, 0.98); accent = vec3(0.54, 0.68, 0.98);
    }
    vec2 ctr = u_res * 0.5;
    vec3 col = mix(bg1, bg0, clamp(length(px - ctr) / (max(u_res.x, u_res.y) * 0.65), 0.0, 1.0));

    float orbR = min(u_res.x, u_res.y) * 0.27 * clamp(u_p0.x, 0.5, 1.6);
    float glow = exp(-length(px - ctr) / (orbR * 1.4)) * 0.26 * u_p1.x;
    col += mix(accent, dotCol, 0.4) * glow * (u_palette == 4 ? -0.35 : 1.0);

    // 3D Fibonacci dot sphere projected onto screen (140 dots)
    int nDots = int(clamp(90.0 + u_p0.y * 70.0, 60.0, 160.0));
    float rotY = t * 0.45 + (u_pointer_on == 1 ? (u_pointer.x - ctr.x) * 0.0018 : 0.0);
    float rotX = 0.28 + sin(t * 0.3) * 0.16 + (u_pointer_on == 1 ? (u_pointer.y - ctr.y) * 0.0014 : 0.0);
    float cy = cos(rotY), sy = sin(rotY), cx = cos(rotX), sx = sin(rotX);

    for (int i = 0; i < 160; i++) {
        if (i >= nDots) break;
        float fi = float(i);
        float y = 1.0 - (fi / float(nDots - 1)) * 2.0;
        float r0 = sqrt(max(0.0, 1.0 - y * y));
        float theta = fi * 2.3999632 + (u_mode == 2 ? t * 0.8 : 0.0);
        float radMul = 1.0;
        if (u_mode == 1) radMul += sin(y * 6.0 - t * 3.0) * 0.14;
        else if (u_mode == 3) radMul += sin(theta * 3.0 + t * 2.0) * 0.16;
        else if (u_mode == 4) radMul += sin(t * 2.4 + fi * 0.08) * 0.12;
        else radMul += sin(t * 1.4 + y * 4.0) * 0.07;

        vec3 p3 = vec3(cos(theta) * r0, y, sin(theta) * r0) * radMul;
        // Rotate Y then X
        vec3 r1 = vec3(p3.x * cy - p3.z * sy, p3.y, p3.x * sy + p3.z * cy);
        vec3 r2 = vec3(r1.x, r1.y * cx - r1.z * sx, r1.y * sx + r1.z * cx);

        float persp = 2.6 / (2.6 - r2.z * 0.65);
        vec2 sp = ctr + r2.xy * orbR * persp;
        if (u_pointer_on == 1) {
            vec2 dv = sp - u_pointer;
            float dl = length(dv);
            if (dl < 140.0 && dl > 0.1) sp += normalize(dv) * (1.0 - dl / 140.0) * 24.0 * u_p0.w;
        }
        float depthA = clamp(0.28 + (r2.z + 1.0) * 0.42, 0.15, 1.0);
        float sz = (2.1 + (r2.z + 1.0) * 1.6) * persp;
        float d = length(px - sp);
        float core = 1.0 - smoothstep(sz * 0.5, sz + 1.2, d);
        float halo = exp(-d / (sz * 3.5)) * 0.25 * u_p1.x;
        vec3 dc = mix(accent, dotCol, clamp(0.4 + r2.z * 0.5, 0.0, 1.0));
        col = mix(col, dc, clamp((core + halo) * depthA, 0.0, 1.0));
    }

    // Glowing neural links from orb perimeter to desktop icons
    if (u_adaptive == 1 && u_p1.y > 0.5 && u_icon_count > 0) {
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 icc = ic.xy + ic.zw * 0.5;
            vec2 dir = normalize(icc - ctr + vec2(1e-4));
            vec2 a = ctr + dir * orbR * 0.92;
            float ld = sdSegment(px, a, icc);
            float lineA = (1.0 - smoothstep(0.4, 1.6, ld)) * 0.20;
            // Traveling pulse dot along link
            float prog = fract(t * 0.35 + float(i) * 0.17);
            vec2 pulseP = mix(a, icc, prog);
            float pulseA = exp(-length(px - pulseP) * 0.16) * 0.65;
            // Subtle icon ring
            float id = sdRoundBox(px - icc, ic.zw * 0.5 + vec2(6.0), 14.0);
            float ringA = (1.0 - smoothstep(0.0, 1.5, abs(id))) * 0.35;
            col = mix(col, mix(dotCol, accent, 0.5), clamp(lineA + pulseA + ringA, 0.0, 1.0));
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_thinking_orbs(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        mode: str_index(st, "orbDesign", &["searching", "wave", "spiral", "nebula", "pulse"], 0),
        palette: str_index(st, "palette", &["monochrome-dark", "aurora", "amber", "cobalt", "monochrome-light"], 0),
        p0: [
            st.param_f32("orbScale", 1.0),
            st.param_f32("dotDensity", 1.0),
            st.param_f32("animSpeed", 1.0),
            match st.param_str("hoverReaction") { Some("off") => 0.0, Some("subtle") => 0.45, _ => 1.0 },
        ],
        p1: [
            st.param_f32("dotGlow", 0.65),
            if st.param_bool("iconLinks", true) { 1.0 } else { 0.0 },
            0.0, 0.0,
        ],
        ..Default::default()
    }
}

// ================= 3. SOLAR SYSTEM (Relativistic Black Hole / Sun + Orbiting Icon Planetary Systems) =================
pub const SOLAR_SYSTEM_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p0.w);
    vec2 ctr = u_res * 0.5;
    vec3 col = vec3(0.015, 0.022, 0.048);

    // Nebula clouds
    float n1 = fbm(px * 0.0022 + vec2(t * 0.015, u_seed * 0.01));
    float n2 = fbm(px * 0.0038 - vec2(u_seed * 0.02, t * 0.012));
    col += vec3(0.14, 0.09, 0.34) * smoothstep(0.35, 0.85, n1) * u_p1.z * 0.7;
    col += vec3(0.06, 0.24, 0.38) * smoothstep(0.40, 0.88, n2) * u_p1.z * 0.55;

    // Deep starfield
    vec2 sc = floor(px / 52.0);
    vec2 sl = fract(px / 52.0);
    float sh = hash21(sc + u_seed * 0.01);
    if (sh > 1.0 - clamp(0.32 * u_p1.y, 0.08, 0.72)) {
        vec2 sp = hash22(sc * 3.1) * 0.7 + 0.15;
        float sd = length(sl - sp) * 52.0;
        float tw = 0.45 + 0.55 * sin(t * (1.2 + sh * 3.0) + sh * 19.0);
        col += vec3(0.88, 0.93, 1.0) * (1.0 - smoothstep(0.4, 1.6, sd)) * tw;
    }

    // Central Black Hole with Relativistic Accretion Disk (or Radiant Sun)
    float bhR = min(u_res.x, u_res.y) * 0.068 * clamp(u_p0.x, 0.5, 1.6);
    vec2 dCtr = px - ctr;
    float rCtr = length(dCtr);
    if (u_mode == 0) {
        // Tilted glowing accretion disk
        vec2 dp = vec2(dCtr.x, dCtr.y * 2.55);
        float dr = length(dp);
        float ang = atan(dp.y, dp.x);
        float swirl = fbm(vec2(ang * 2.5 - t * 1.4, dr * 0.04));
        float disk = exp(-abs(dr - bhR * 2.05) / (bhR * 0.72)) * (0.6 + 0.6 * swirl) * u_p0.y;
        col += mix(vec3(1.0, 0.42, 0.14), vec3(1.0, 0.86, 0.52), clamp(disk, 0.0, 1.0)) * disk * 1.15;
        // Photon ring + event horizon
        float ring = exp(-abs(rCtr - bhR * 1.08) / (bhR * 0.16)) * 1.2 * u_p0.y;
        col += vec3(1.0, 0.78, 0.42) * ring;
        col *= smoothstep(bhR * 0.94, bhR * 1.04, rCtr);
    } else {
        float sunGlow = exp(-rCtr / (bhR * 3.0)) * 1.1 * u_p0.y;
        col += vec3(1.0, 0.62, 0.22) * sunGlow;
        float sunCore = 1.0 - smoothstep(bhR * 0.92, bhR * 1.06, rCtr);
        col = mix(col, vec3(1.0, 0.94, 0.76), sunCore);
    }

    // Orbiting mini planetary systems around each desktop icon
    if (u_adaptive == 1 && u_icon_count > 0) {
        int planets = int(clamp(round(u_p1.x), 1.0, 4.0));
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 icc = ic.xy + ic.zw * 0.5;
            float baseR = max(ic.z, ic.w) * 0.62;
            // Subtle stellar glow behind icon
            col += vec3(0.35, 0.65, 1.0) * exp(-length(px - icc) / (baseR * 1.3)) * 0.18;
            for (int p = 0; p < 4; p++) {
                if (p >= planets) break;
                float fp = float(p);
                float orad = baseR + (fp + 1.0) * 14.0 * u_p0.z;
                float ringA = (1.0 - smoothstep(0.3, 1.3, abs(length(px - icc) - orad))) * 0.20;
                col += vec3(0.55, 0.75, 1.0) * ringA;
                float pang = t * (0.9 + fp * 0.45) + float(i) * 1.7 + fp * 2.1;
                vec2 pp = icc + vec2(cos(pang), sin(pang)) * orad;
                float pd = length(px - pp);
                vec3 pcol = (p == 0) ? vec3(0.45, 0.84, 1.0) : (p == 1 ? vec3(1.0, 0.68, 0.42) : vec3(0.76, 0.62, 1.0));
                col = mix(col, pcol, 1.0 - smoothstep(2.6, 4.2, pd));
            }
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_solar_system(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        mode: str_index(st, "centerBodyType", &["BlackHole", "Sun"], 0),
        palette: 0,
        p0: [
            st.param_f32("blackHoleSize", 1.0),
            st.param_f32("accretionIntensity", 1.0),
            st.param_f32("orbitSize", 1.0),
            st.param_f32("globalOrbitSpeed", 0.65),
        ],
        p1: [
            st.param_f32("planetCount", 3.0),
            st.param_f32("starDensity", 1.0),
            st.param_f32("nebulaIntensity", 0.65),
            0.0,
        ],
        ..Default::default()
    }
}

// ================= 4. STUDIO GLASS (Designer Studio, Wooden Blinds Light Shafts, Lamp & Refractive Icon Plaques) =================
pub const STUDIO_GLASS_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time;
    int pr = u_mode; // 0 auto, 1 morning, 2 storm, 3 afternoon, 4 sunset, 5 night

    vec3 wallTop, wallBot, deskCol, sunCol;
    if (pr == 2) { // Thunderstorm
        wallTop = vec3(0.06, 0.11, 0.14); wallBot = vec3(0.04, 0.08, 0.11); deskCol = vec3(0.18, 0.11, 0.08); sunCol = vec3(0.52, 0.72, 0.88);
    } else if (pr == 3) { // Afternoon
        wallTop = vec3(0.16, 0.25, 0.22); wallBot = vec3(0.10, 0.18, 0.16); deskCol = vec3(0.38, 0.23, 0.13); sunCol = vec3(1.00, 0.88, 0.68);
    } else if (pr == 4) { // Sunset
        wallTop = vec3(0.22, 0.13, 0.16); wallBot = vec3(0.12, 0.08, 0.12); deskCol = vec3(0.32, 0.16, 0.10); sunCol = vec3(1.00, 0.58, 0.34);
    } else if (pr == 5) { // Night
        wallTop = vec3(0.05, 0.07, 0.10); wallBot = vec3(0.03, 0.05, 0.08); deskCol = vec3(0.15, 0.10, 0.07); sunCol = vec3(0.28, 0.42, 0.68);
    } else { // Morning
        wallTop = vec3(0.11, 0.20, 0.17); wallBot = vec3(0.08, 0.14, 0.13); deskCol = vec3(0.34, 0.21, 0.12); sunCol = vec3(0.98, 0.86, 0.66);
    }

    // Parallax studio wall & warm walnut desk horizon
    vec2 par = (u_pointer_on == 1) ? ((u_pointer / u_res - 0.5) * 24.0 * u_p1.y) : vec2(0.0);
    float deskY = u_res.y * 0.70 + par.y * 0.35;
    vec3 col = mix(wallTop, wallBot, clamp(px.y / max(1.0, deskY), 0.0, 1.0));
    if (px.y > deskY) {
        float grain = fbm(vec2(px.x * 0.008, px.y * 0.06)) * 0.16;
        col = deskCol * (0.86 + grain) * mix(1.0, 0.55, clamp((px.y - deskY) / (u_res.y * 0.32), 0.0, 1.0));
    }

    // Left window + wooden venetian blind volumetric light shafts
    float blindOpen = clamp(u_p0.x, 0.0, 1.0);
    float beamAngle = (px.x - par.x) * 0.55 - (px.y - u_res.y * 0.18) * 0.85;
    float slats = smoothstep(0.25, 0.75, sin(beamAngle * 0.14) * 0.5 + 0.5 + (blindOpen - 0.5) * 0.6);
    float beamMask = smoothstep(0.0, 0.18, v_uv.x) * (1.0 - smoothstep(0.25, 0.85, v_uv.x)) * (1.0 - smoothstep(0.1, 0.95, 1.0 - v_uv.y));
    col += sunCol * slats * beamMask * u_p0.y * 0.46;

    // Storm lightning flash or rain streaks
    if (pr == 2 && u_p0.z > 0.05) {
        float flash = pow(max(0.0, sin(t * 0.7) * sin(t * 2.9) * sin(t * 7.3)), 18.0);
        col += vec3(0.65, 0.80, 1.0) * flash * 0.55 * u_p0.z;
        float rain = smoothstep(0.92, 0.99, sin(px.x * 0.16 + px.y * 0.035 - t * 22.0));
        col += vec3(0.65, 0.82, 0.95) * rain * 0.08 * u_p0.z;
    }

    // Right warm mushroom frosted glass desk lamp glow
    vec2 lampP = vec2(u_res.x * 0.84 - par.x * 0.4, deskY - u_res.y * 0.08);
    float lampGlow = exp(-length(px - lampP) / (min(u_res.x, u_res.y) * 0.30)) * u_p1.x;
    col += vec3(1.0, 0.72, 0.38) * lampGlow * 0.62;
    float dome = sdRoundBox(px - lampP, vec2(44.0, 26.0), 24.0);
    col = mix(col, vec3(1.0, 0.90, 0.72), (1.0 - smoothstep(-1.0, 1.5, dome)) * (0.3 + 0.7 * u_p1.x));

    // Optical refractive glass icon plaques
    if (u_adaptive == 1 && u_p1.w < 1.5 && u_icon_count > 0) {
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 c = ic.xy + ic.zw * 0.5;
            vec2 hs = ic.zw * 0.5 + vec2(9.0, 7.0);
            float rad = (u_p1.w > 0.5) ? 8.0 : 16.0;
            float d = sdRoundBox(px - c, hs, rad);
            float mask = 1.0 - smoothstep(-0.8, 0.8, d);
            float rim = (1.0 - smoothstep(0.0, 1.6, abs(d))) * 0.48;
            col = mix(col, mix(col * 1.18, vec3(0.95, 0.97, 1.0), 0.14), mask);
            col += mix(sunCol, vec3(0.85, 0.94, 1.0), 0.5) * rim;
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_studio_glass(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        mode: st.param_f32("preset", 1.0).round() as i32,
        palette: 0,
        p0: [
            st.param_f32("blinds", 0.70),
            st.param_f32("shafts", 0.80),
            st.param_f32("weather", 0.78),
            st.param_f32("refraction", 0.85),
        ],
        p1: [
            st.param_f32("deskLamp", 0.70),
            st.param_f32("parallax", 0.55),
            st.param_f32("frost", 0.50),
            st.param_f32("iconGlass", 0.0),
        ],
        ..Default::default()
    }
}
