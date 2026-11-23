use crate::state::AppState;
use crate::themes::common::PackedThemeParams;

// ================= 1. SOFTWARE WORLD (Developer Node Ecosystem, Data Packets, Git Rails & Code Particles) =================
pub const SOFTWARE_WORLD_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p0.x);
    float netAct = clamp(u_p0.y, 0.0, 1.0);
    float conn = clamp(u_p0.z, 0.0, 1.0);

    // Dark IDE / terminal gradient backdrop + subtle engineering dot grid
    vec3 col = mix(vec3(0.038, 0.050, 0.075), vec3(0.058, 0.088, 0.145), clamp(v_uv.x * 0.6 + (1.0 - v_uv.y) * 0.4, 0.0, 1.0));
    vec2 g = mod(px, 36.0) - 18.0;
    float gridDot = 1.0 - smoothstep(0.8, 1.6, length(g));
    col += vec3(0.22, 0.36, 0.56) * gridDot * 0.22 * u_p1.x;

    // Animated Git branch rails sweeping across bottom
    for (int b = 0; b < 3; b++) {
        float fb = float(b);
        float by = u_res.y * (0.78 + fb * 0.06) + sin(px.x * 0.005 + fb * 1.4) * 14.0;
        float lineA = (1.0 - smoothstep(0.5, 1.8, abs(px.y - by))) * 0.35 * u_p1.x;
        vec3 bc = (b == 0) ? vec3(0.20, 0.78, 0.72) : (b == 1 ? vec3(0.54, 0.46, 0.98) : vec3(0.96, 0.76, 0.36));
        col += bc * lineA;
        float commitX = mod(px.x + t * (28.0 + fb * 10.0), 160.0) - 80.0;
        float nd = length(vec2(commitX, px.y - by));
        col = mix(col, bc, (1.0 - smoothstep(3.0, 4.5, nd)) * 0.75 * u_p1.x);
    }

    // Desktop icons as interactive microservice nodes connected by glowing data-packet links!
    if (u_adaptive == 1 && u_icon_count > 0) {
        int n = min(u_icon_count, 24);
        for (int i = 0; i < 24; i++) {
            if (i >= n) break;
            vec4 ic = u_icons[i];
            vec2 a = ic.xy + ic.zw * 0.5;
            // Node frame + pulsing status ring
            float d = sdRoundBox(px - a, ic.zw * 0.5 + vec2(8.0, 6.0), 14.0);
            float rim = (1.0 - smoothstep(0.0, 1.6, abs(d))) * 0.55;
            float fill = (1.0 - smoothstep(-0.8, 0.8, d)) * 0.14;
            vec3 nCol = (mod(float(i), 2.0) < 0.5) ? vec3(0.22, 0.82, 0.76) : vec3(0.42, 0.66, 0.98);
            col = mix(col, nCol, fill + rim);

            // Connect to next node in ring
            if (conn > 0.05 && n > 1) {
                vec4 ic2 = u_icons[(i + 1) % n];
                vec2 b = ic2.xy + ic2.zw * 0.5;
                if (length(b - a) < 520.0) {
                    float ld = sdSegment(px, a, b);
                    col += nCol * (1.0 - smoothstep(0.4, 1.5, ld)) * 0.25 * conn;
                    float prog = fract(t * (0.35 + netAct * 0.45) + float(i) * 0.23);
                    vec2 pkt = mix(a, b, prog);
                    col += vec3(0.92, 0.98, 1.0) * exp(-length(px - pkt) * 0.24) * 0.9 * netAct;
                }
            }
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_software_world(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("animSpeed", 1.0),
            st.param_f32("networkActivity", 0.65),
            st.param_f32("connectionIntensity", 0.55),
            st.param_f32("particleDensity", 0.55),
        ],
        p1: [st.param_f32("envDetail", 0.70), 0.0, 0.0, 0.0],
        ..Default::default()
    }
}

// ================= 2. RIFT (Arcane Runic Chasm Terrain + Glowing Mana Ley-Lines + Spirit Wisps) =================
pub const RIFT_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float warp = clamp(u_p0.y, 0.0, 80.0);
    float glow = clamp(u_p0.z, 0.0, 1.4);
    float t = u_time * max(0.05, u_p0.w);

    // Ancient stone canyon floor
    float n = fbm(px * 0.01 + u_seed * 0.01);
    vec3 stone = mix(vec3(0.08, 0.11, 0.14), vec3(0.16, 0.21, 0.24), n);
    vec3 col = stone;

    // Glowing arcane chasm & runic ley-lines carving around desktop icons
    float id = iconField(px, sigma, warp, 54.0);
    float chasm = exp(-abs(id - 14.0) * 0.09) * glow;
    float pulse = 0.72 + 0.28 * sin(t * 2.2 + id * 0.08);
    col += vec3(0.14, 0.86, 0.82) * chasm * pulse * 0.92;
    col += vec3(0.95, 0.78, 0.34) * exp(-abs(id) * 0.22) * glow * 0.65;

    // Floating arcane spirit wisps
    for (int i = 0; i < 7; i++) {
        float fi = float(i);
        vec2 wp = vec2(
            u_res.x * (0.14 + 0.72 * hash11(fi * 4.7) + sin(t * (0.5 + fi * 0.09) + fi) * 0.10),
            u_res.y * (0.15 + 0.70 * hash11(fi * 8.3) + cos(t * (0.45 + fi * 0.08) + fi * 1.3) * 0.10)
        );
        col += vec3(0.32, 0.94, 0.88) * exp(-length(px - wp) * 0.08) * 0.75 * glow;
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_rift(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("sigma", 82.0),
            st.param_f32("warp", 24.0),
            st.param_f32("glow", 0.78),
            st.param_f32("wind", 1.0),
        ],
        ..Default::default()
    }
}

// ================= 3. STRIKE (Tactical Sandstone Dust Arena + Contour Lines + Volumetric Smoke Plumes) =================
pub const STRIKE_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float warp = clamp(u_p0.y, 0.0, 80.0);
    float smoke = clamp(u_p0.z, 0.0, 1.0);
    float t = u_time * max(0.05, u_p0.w);

    float n = fbm(px * 0.012 + u_seed * 0.01);
    vec3 dust = mix(vec3(0.76, 0.65, 0.49), vec3(0.88, 0.79, 0.63), n);
    vec3 col = dust;

    // Tactical sandstone site zones & crisp perimeter stripes around desktop icons
    float id = iconField(px, sigma, warp, 44.0);
    float site = 1.0 - smoothstep(-3.0, 3.0, id);
    col = mix(col, vec3(0.62, 0.53, 0.42), site * 0.55);
    float stripe = (1.0 - smoothstep(0.0, 2.0, abs(id))) * 0.85;
    col = mix(col, vec3(0.86, 0.30, 0.16), stripe);

    // Drifting tactical smoke grenade plumes
    if (smoke > 0.02) {
        vec2 smP = vec2(u_res.x * 0.66 + sin(t * 0.25) * 40.0, u_res.y * 0.48 + cos(t * 0.2) * 30.0);
        float smN = fbm((px - smP) * 0.008 - vec2(t * 0.18, 0.0));
        float smA = exp(-length(px - smP) / 210.0) * smoothstep(0.28, 0.78, smN) * smoke * 0.85;
        col = mix(col, vec3(0.86, 0.87, 0.89), smA);
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_strike(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("sigma", 82.0),
            st.param_f32("warp", 20.0),
            st.param_f32("smoke", 0.45),
            st.param_f32("wind", 1.0),
        ],
        ..Default::default()
    }
}

// ================= 4. POCKET (Strategic War-Room Topographic Map + Front-Line Encirclement Arcs) =================
pub const POCKET_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float warp = clamp(u_p0.y, 0.0, 80.0);
    float arrows = clamp(u_p0.z, 0.0, 1.0);
    float t = u_time * max(0.05, u_p0.w);

    // Parchment topographic strategy map with crisp elevation contour rings
    float elev = fbm(px * 0.0045 + u_seed * 0.01);
    vec3 col = mix(vec3(0.86, 0.82, 0.72), vec3(0.74, 0.72, 0.60), elev);
    float contour = 1.0 - smoothstep(0.0, 0.06, abs(fract(elev * 12.0) - 0.5));
    col = mix(col, vec3(0.46, 0.42, 0.34), contour * 0.28);
    // Subtle lat/lon tactical grid
    vec2 gr = abs(fract(px / 90.0) - 0.5);
    float gLine = 1.0 - smoothstep(0.485, 0.5, max(gr.x, gr.y));
    col = mix(col, vec3(0.38, 0.36, 0.30), gLine * 0.16);

    // Animated front-line encirclement perimeter wrapping desktop icons
    float id = iconField(px, sigma, warp, 64.0);
    float pocketIn = 1.0 - smoothstep(-3.0, 3.0, id);
    col = mix(col, vec3(0.24, 0.48, 0.74), pocketIn * 0.24);
    float dash = smoothstep(-0.2, 0.2, sin((px.x + px.y) * 0.08 - t * 2.5));
    float frontLine = (1.0 - smoothstep(0.0, 2.4, abs(id))) * (0.45 + 0.55 * dash);
    col = mix(col, vec3(0.82, 0.20, 0.18), frontLine * (0.35 + 0.65 * arrows));
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_pocket(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        p0: [
            st.param_f32("sigma", 78.0),
            st.param_f32("warp", 22.0),
            st.param_f32("arrows", 0.78),
            st.param_f32("pace", 1.0),
        ],
        ..Default::default()
    }
}

// ================= 5. ROACH (Neon Retro Tabletop + Spotlight Vibes + Lively Scurrying Critters) =================
pub const ROACH_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float sigma = clamp(u_p0.x, 40.0, 200.0);
    float warp = clamp(u_p0.y, 0.0, 80.0);
    float glow = clamp(u_p0.z, 0.0, 1.4);
    float t = u_time * max(0.05, u_p0.w);

    vec3 bg = (u_palette == 1) ? vec3(0.11, 0.05, 0.16) : vec3(0.07, 0.08, 0.11);
    vec3 spot = (u_palette == 1) ? vec3(0.98, 0.34, 0.72) : vec3(0.96, 0.66, 0.22);
    vec3 col = bg;

    // Retro tabletop checkerboard tiling
    vec2 tile = floor(px / 54.0);
    float chk = mod(tile.x + tile.y, 2.0);
    col += vec3(0.03) * chk;

    // Sweeping disco/diner spotlight
    vec2 sp = u_res * vec2(0.5 + sin(t * 0.45) * 0.25, 0.5 + cos(t * 0.35) * 0.22);
    col += spot * exp(-length(px - sp) / (min(u_res.x, u_res.y) * 0.42)) * 0.35 * glow;

    // Safe tabletop coasters under desktop icons
    float id = iconField(px, sigma, warp, 48.0);
    float coaster = 1.0 - smoothstep(-2.0, 2.0, id);
    col = mix(col, vec3(0.18, 0.14, 0.12), coaster * 0.75);
    col += spot * (1.0 - smoothstep(0.0, 2.0, abs(id))) * 0.65 * glow;

    // Lively animated critters scurrying around the floor
    for (int i = 0; i < 10; i++) {
        float fi = float(i);
        float a = t * (0.9 + fi * 0.15) + fi * 2.1;
        vec2 cp = vec2(
            u_res.x * (0.12 + 0.76 * hash11(fi * 6.3) + sin(a) * 0.14),
            u_res.y * (0.14 + 0.72 * hash11(fi * 9.1) + cos(a * 0.85) * 0.14)
        );
        vec2 vel = vec2(cos(a) * 0.14, -sin(a * 0.85) * 0.14 * 0.85);
        float ang = atan(vel.y, vel.x);
        mat2 rot = mat2(cos(ang), sin(ang), -sin(ang), cos(ang));
        vec2 lp = rot * (px - cp);
        float body = length(lp * vec2(0.65, 1.45)) - 6.0;
        col = mix(col, vec3(0.42, 0.22, 0.10), 1.0 - smoothstep(-0.8, 0.8, body));
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_roach(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        palette: st.param_f32("vibe", 0.0).round() as i32,
        p0: [
            st.param_f32("sigma", 78.0),
            st.param_f32("warp", 24.0),
            st.param_f32("glow", 0.78),
            st.param_f32("speed", 1.0),
        ],
        ..Default::default()
    }
}
