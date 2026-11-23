use crate::state::AppState;
use crate::themes::common::{str_index, PackedThemeParams};

// ================= 1. NEUMORPH (Tactile Soft Extruded Tiles + Dual Highlight/Shadow + Icon Cradles) =================
pub const NEUMORPH_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p0.y);
    float depth = clamp(u_p0.x, 0.2, 2.0);
    float ripS = clamp(u_p0.w, 0.0, 2.0);

    vec3 base, hi, sh, acc;
    if (u_palette == 1) { // slate
        base = vec3(0.16, 0.19, 0.24); hi = vec3(0.24, 0.28, 0.34); sh = vec3(0.09, 0.11, 0.15); acc = vec3(0.38, 0.68, 0.98);
    } else if (u_palette == 2) { // sage
        base = vec3(0.84, 0.88, 0.85); hi = vec3(0.95, 0.98, 0.96); sh = vec3(0.68, 0.74, 0.70); acc = vec3(0.20, 0.62, 0.48);
    } else if (u_palette == 3) { // obsidian
        base = vec3(0.10, 0.11, 0.13); hi = vec3(0.18, 0.19, 0.23); sh = vec3(0.04, 0.04, 0.05); acc = vec3(0.66, 0.56, 0.98);
    } else if (u_palette == 4) { // sand
        base = vec3(0.91, 0.87, 0.82); hi = vec3(0.99, 0.97, 0.94); sh = vec3(0.76, 0.71, 0.64); acc = vec3(0.84, 0.48, 0.28);
    } else { // alabaster
        base = vec3(0.89, 0.91, 0.94); hi = vec3(0.99, 0.995, 1.0); sh = vec3(0.73, 0.77, 0.83); acc = vec3(0.24, 0.50, 0.96);
    }

    vec2 lightDir = normalize(u_pointer_on == 1 && u_p1.x > 0.5 ? (u_pointer - px) : vec2(-1.0, -1.0));
    float tileSz = mix(115.0, 72.0, clamp(u_p0.z * 0.5, 0.0, 1.0));
    vec2 cell = floor(px / tileSz);
    vec2 loc = mod(px, tileSz) - tileSz * 0.5;
    float wave = sin(length(cell * 0.45) - t * 1.4) * 0.5 + 0.5;
    float pad = tileSz * 0.12;
    float d = sdRoundBox(loc, vec2(tileSz * 0.5 - pad), tileSz * 0.22);
    float dHi = sdRoundBox(loc - lightDir * 5.0 * depth, vec2(tileSz * 0.5 - pad), tileSz * 0.22);
    float dSh = sdRoundBox(loc + lightDir * 6.0 * depth, vec2(tileSz * 0.5 - pad), tileSz * 0.22);

    vec3 col = base;
    float shA = exp(-max(dSh, 0.0) * 0.14) * 0.55 * depth * (0.4 + 0.6 * wave * ripS);
    float hiA = exp(-max(dHi, 0.0) * 0.16) * 0.55 * depth * (0.4 + 0.6 * wave * ripS);
    col = mix(col, sh, shA * smoothstep(-1.0, 3.0, d));
    col = mix(col, hi, hiA * smoothstep(-1.0, 3.0, d));
    col = mix(col, mix(base, hi, 0.22 * wave), 1.0 - smoothstep(-0.8, 0.8, d));

    // Tactile extruded cradles under desktop icons
    if (u_adaptive == 1 && u_icon_count > 0) {
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 c = ic.xy + ic.zw * 0.5;
            vec2 hs = ic.zw * 0.5 + vec2(10.0, 8.0);
            float id = sdRoundBox(px - c, hs, 18.0);
            float ish = sdRoundBox(px - c - vec2(6.0, 6.0) * depth, hs, 18.0);
            float ihi = sdRoundBox(px - c + vec2(5.0, 5.0) * depth, hs, 18.0);
            col = mix(col, sh, exp(-max(ish, 0.0) * 0.12) * 0.65 * smoothstep(-1.0, 3.0, id));
            col = mix(col, hi, exp(-max(ihi, 0.0) * 0.14) * 0.65 * smoothstep(-1.0, 3.0, id));
            col = mix(col, mix(base, acc, 0.08), 1.0 - smoothstep(-0.8, 0.8, id));
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_neumorph(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        palette: str_index(st, "palette", &["alabaster", "slate", "sage", "obsidian", "sand"], 0),
        p0: [
            st.param_f32("reliefDepth", 1.0),
            st.param_f32("waveSpeed", 0.75),
            st.param_f32("tileDensity", 1.0),
            st.param_f32("rippleStrength", 1.0),
        ],
        p1: [if st.param_bool("lightFollowCursor", false) { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        ..Default::default()
    }
}

// ================= 2. CLAYMORPH (Inflated 3D Glossy Clay Blobs + Puffy Icon Cushions) =================
pub const CLAYMORPH_BODY: &str = r#"
vec3 clayShade(vec3 baseCol, float d, vec2 grad, float puff) {
    float rim = clamp(-d / (26.0 * max(0.4, puff)), 0.0, 1.0);
    float dome = sqrt(max(0.0, 1.0 - (1.0 - rim) * (1.0 - rim)));
    vec3 n = normalize(vec3(grad * (1.0 - rim) * 0.85, dome + 0.35));
    vec3 ldir = normalize(vec3(-0.45, -0.55, 0.70));
    float diff = clamp(dot(n, ldir), 0.0, 1.0);
    float spec = pow(clamp(dot(reflect(-ldir, n), vec3(0.0, 0.0, 1.0)), 0.0, 1.0), 18.0);
    vec3 c = baseCol * (0.68 + 0.42 * diff) + vec3(1.0) * spec * 0.38;
    // Inner warm bottom bounce
    c += baseCol * clamp(-n.y, 0.0, 1.0) * 0.16;
    return c;
}

void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p0.w);
    float puff = clamp(u_p0.x, 0.4, 1.8);

    vec3 bg0 = (u_palette == 1) ? vec3(0.11, 0.10, 0.18) : vec3(0.95, 0.93, 0.98);
    vec3 bg1 = (u_palette == 1) ? vec3(0.18, 0.14, 0.26) : vec3(0.88, 0.94, 0.99);
    vec3 col = mix(bg0, bg1, clamp((v_uv.x + 1.0 - v_uv.y) * 0.5, 0.0, 1.0));

    vec3 cols[5];
    cols[0] = vec3(0.99, 0.62, 0.70);
    cols[1] = vec3(0.56, 0.84, 0.99);
    cols[2] = vec3(0.60, 0.91, 0.75);
    cols[3] = vec3(0.99, 0.84, 0.52);
    cols[4] = vec3(0.76, 0.67, 0.99);

    int count = int(clamp(round(u_p0.z * 7.0), 3.0, 9.0));
    for (int i = 0; i < 9; i++) {
        if (i >= count) break;
        float fi = float(i);
        vec2 c = vec2(
            u_res.x * (0.14 + 0.72 * hash11(fi * 5.7 + 1.2) + sin(t * (0.4 + fi * 0.07) + fi) * 0.08),
            u_res.y * (0.16 + 0.68 * hash11(fi * 9.3 + 2.4) + cos(t * (0.35 + fi * 0.06) + fi * 1.4) * 0.09)
        );
        if (u_pointer_on == 1) {
            vec2 dv = c - u_pointer;
            float dl = length(dv);
            if (dl < 180.0 && dl > 0.1) c += normalize(dv) * (1.0 - dl / 180.0) * 42.0 * u_p1.x;
        }
        vec2 hs = vec2(52.0 + hash11(fi * 3.1) * 48.0, 44.0 + hash11(fi * 6.7) * 42.0);
        float d = sdRoundBox(px - c, hs, min(hs.x, hs.y) * 0.75);
        float ds = sdRoundBox(px - c - vec2(8.0, 14.0), hs, min(hs.x, hs.y) * 0.75);
        col = mix(col, bg0 * 0.72, exp(-max(ds, 0.0) * 0.07) * 0.34 * smoothstep(-1.0, 3.0, d));
        if (d < 2.0) {
            vec2 g = normalize(px - c + vec2(1e-4));
            col = mix(col, clayShade(cols[i % 5], d, g, puff), 1.0 - smoothstep(-1.0, 1.0, d));
        }
    }

    // Puffy clay cushions cradling desktop icons
    if (u_adaptive == 1 && u_icon_count > 0) {
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 c = ic.xy + ic.zw * 0.5;
            vec2 hs = ic.zw * 0.5 + vec2(10.0, 8.0);
            float d = sdRoundBox(px - c, hs, 20.0);
            float ds = sdRoundBox(px - c - vec2(6.0, 10.0), hs, 20.0);
            col = mix(col, bg0 * 0.70, exp(-max(ds, 0.0) * 0.09) * 0.36 * smoothstep(-1.0, 2.0, d));
            if (d < 2.0) {
                vec2 g = normalize(px - c + vec2(1e-4));
                col = mix(col, clayShade(cols[i % 5], d, g, puff), 1.0 - smoothstep(-1.0, 1.0, d));
            }
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_claymorph(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        palette: str_index(st, "palette", &["pastel", "midnight", "citrus", "berry"], 0),
        p0: [
            st.param_f32("puffiness", 1.0),
            st.param_f32("squishStrength", 1.0),
            st.param_f32("blobCount", 1.0),
            st.param_f32("floatSpeed", 0.80),
        ],
        p1: [st.param_f32("cursorRepel", 1.0), 0.0, 0.0, 0.0],
        ..Default::default()
    }
}

// ================= 3. NEOBRUTALISM (Bauhaus Kinetic Poster + Bold Black Borders + Hard Shadows + Halftone) =================
pub const NEOBRUTALISM_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p0.w);
    float bw = clamp(u_p0.x, 1.5, 6.0);
    float shOff = clamp(u_p0.y, 2.0, 14.0);

    vec3 paper = (u_palette == 1) ? vec3(0.11, 0.11, 0.13) : vec3(0.97, 0.95, 0.90);
    vec3 ink = (u_palette == 1) ? vec3(0.96, 0.96, 0.94) : vec3(0.06, 0.06, 0.07);
    vec3 col = paper;

    // Classic print halftone dot grid
    if (u_p1.x > 0.5) {
        vec2 hp = mod(px, 16.0) - 8.0;
        float hd = 1.0 - smoothstep(1.2, 2.1, length(hp));
        col = mix(col, ink, hd * 0.08);
    }

    vec3 pal[4];
    pal[0] = vec3(0.99, 0.82, 0.14); // Bauhaus yellow
    pal[1] = vec3(0.95, 0.30, 0.24); // Bold red
    pal[2] = vec3(0.16, 0.45, 0.92); // Cobalt blue
    pal[3] = vec3(0.24, 0.84, 0.56); // Mint pop

    for (int i = 0; i < 6; i++) {
        float fi = float(i);
        vec2 c = vec2(
            u_res.x * (0.16 + 0.68 * hash11(fi * 7.1 + 1.0) + sin(t * 0.45 + fi) * 0.05),
            u_res.y * (0.18 + 0.64 * hash11(fi * 4.3 + 2.0) + cos(t * 0.38 + fi * 1.5) * 0.05)
        );
        float a = sin(t * 0.4 + fi) * 0.22;
        mat2 rot = mat2(cos(a), -sin(a), sin(a), cos(a));
        vec2 hs = vec2(48.0 + fi * 10.0, 38.0 + mod(fi, 3.0) * 14.0);
        float ds = sdRoundBox(rot * (px - c - vec2(shOff)), hs, 4.0);
        float d = sdRoundBox(rot * (px - c), hs, 4.0);
        col = mix(col, ink, 1.0 - smoothstep(-0.7, 0.7, ds));
        col = mix(col, pal[i % 4], 1.0 - smoothstep(-0.7, 0.7, d));
        col = mix(col, ink, 1.0 - smoothstep(bw * 0.5 - 0.6, bw * 0.5 + 0.6, abs(d)));
    }

    // Punchy Neo-Brutalist framed cards beneath desktop icons
    if (u_adaptive == 1 && u_icon_count > 0) {
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 c = ic.xy + ic.zw * 0.5;
            vec2 hs = ic.zw * 0.5 + vec2(8.0, 6.0);
            float ds = sdRoundBox(px - c - vec2(shOff * 0.7), hs, 6.0);
            float d = sdRoundBox(px - c, hs, 6.0);
            col = mix(col, ink, 1.0 - smoothstep(-0.7, 0.7, ds));
            col = mix(col, pal[i % 4], 1.0 - smoothstep(-0.7, 0.7, d));
            col = mix(col, ink, 1.0 - smoothstep(bw * 0.5 - 0.6, bw * 0.5 + 0.6, abs(d)));
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_neobrutalism(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        palette: str_index(st, "palette", &["bauhaus", "dark", "pop", "cyber"], 0),
        p0: [
            st.param_f32("borderWeight", 3.0),
            st.param_f32("shadowDepth", 7.0),
            st.param_f32("shapeCount", 1.0),
            st.param_f32("motionSpeed", 0.85),
        ],
        p1: [if st.param_bool("halftonePattern", true) { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        ..Default::default()
    }
}

// ================= 4. VOXELCRAFT (Blocky Voxel Terrain, Biomes, Torchlight & Raised Icon Voxel Islands) =================
pub const VOXELCRAFT_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p0.w);
    float bsz = max(14.0, 32.0 * clamp(u_p0.x, 0.5, 1.8));
    vec2 cell = floor(px / bsz);
    vec2 loc = fract(px / bsz);
    float h = fbm(cell * 0.14 + u_seed * 0.01);

    // Biome block palette
    vec3 grass, dirt, water, accent;
    if (u_palette == 1) { // nether
        grass = vec3(0.46, 0.13, 0.13); dirt = vec3(0.24, 0.09, 0.10); water = vec3(0.95, 0.36, 0.08); accent = vec3(1.0, 0.78, 0.24);
    } else if (u_palette == 2) { // end
        grass = vec3(0.84, 0.82, 0.62); dirt = vec3(0.14, 0.09, 0.20); water = vec3(0.05, 0.03, 0.09); accent = vec3(0.74, 0.38, 0.96);
    } else if (u_palette == 3) { // cherry
        grass = vec3(0.44, 0.72, 0.36); dirt = vec3(0.96, 0.72, 0.82); water = vec3(0.24, 0.62, 0.84); accent = vec3(1.0, 0.86, 0.92);
    } else { // overworld
        grass = vec3(0.36, 0.66, 0.26); dirt = vec3(0.49, 0.36, 0.24); water = vec3(0.18, 0.48, 0.78); accent = vec3(0.98, 0.78, 0.26);
    }

    vec3 col = (h < 0.38) ? water : (h < 0.52 ? dirt : grass);
    // Crisp 4x4 Minecraft-style pixel grain inside each voxel block
    vec2 subPx = floor(loc * 4.0);
    col *= 0.90 + 0.20 * hash21(cell * 4.0 + subPx);
    // Beveled block edge highlight/shadow
    if (loc.x < 0.07 || loc.y < 0.07) col *= 1.14;
    if (loc.x > 0.93 || loc.y > 0.93) col *= 0.82;

    // Animated blocky clouds drifting overhead
    vec2 clCell = floor((px - vec2(t * 24.0, 0.0)) / (bsz * 1.6));
    if (fbm(clCell * 0.22) > 0.78 - u_p0.y * 0.14) {
        col = mix(col, vec3(0.95, 0.97, 1.0), 0.32);
    }

    // Raised voxel island blocks & flickering torchlight around desktop icons
    if (u_adaptive == 1 && u_icon_count > 0) {
        vec2 snapped = (cell + 0.5) * bsz;
        float id = iconBoxField(snapped, 12.0, 4.0, 0.0);
        if (id < 0.0) {
            col = grass * (0.95 + 0.16 * hash21(cell * 4.0 + subPx));
            if (loc.x < 0.09 || loc.y < 0.09) col *= 1.18;
            if (loc.x > 0.91 || loc.y > 0.91) col *= 0.78;
        }
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec2 tc = u_icons[i].xy + vec2(u_icons[i].z * 0.5, u_icons[i].w * 0.5);
            float flick = 0.88 + 0.12 * sin(t * 9.0 + float(i) * 3.1);
            col += accent * exp(-length(px - tc) / (bsz * 4.2)) * 0.32 * u_p0.z * flick;
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_voxelcraft(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        palette: str_index(st, "biome", &["overworld", "nether", "end", "cherry"], 0),
        p0: [
            st.param_f32("blockSize", 1.0),
            st.param_f32("cloudDensity", 1.0),
            st.param_f32("torchGlow", 1.0),
            st.param_f32("animSpeed", 1.0),
        ],
        ..Default::default()
    }
}

// ================= 5. PAPERCUT (Layered Cut-Paper Lightbox Garden + Directional Shadows + Arched Icon Niches) =================
pub const PAPERCUT_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float ang = radians(u_p0.x);
    vec2 shDir = vec2(-cos(ang), -sin(ang)) * 10.0 * clamp(u_p0.z, 0.4, 2.0);
    float shStr = clamp(u_p0.y, 0.0, 1.5) * 0.42;
    float t = u_time * max(0.0, u_p1.y);

    vec3 sky0, sky1, l1, l2, l3, l4;
    if (u_palette == 1) { // Turquoise Coral
        sky0 = vec3(0.58, 0.85, 0.90); sky1 = vec3(0.95, 0.70, 0.64);
        l1 = vec3(0.24, 0.62, 0.71); l2 = vec3(0.16, 0.49, 0.60); l3 = vec3(0.28, 0.56, 0.38); l4 = vec3(0.16, 0.41, 0.27);
    } else if (u_palette == 2) { // Indigo Gold
        sky0 = vec3(0.12, 0.16, 0.34); sky1 = vec3(0.86, 0.66, 0.38);
        l1 = vec3(0.18, 0.24, 0.46); l2 = vec3(0.14, 0.19, 0.38); l3 = vec3(0.24, 0.28, 0.44); l4 = vec3(0.11, 0.14, 0.28);
    } else if (u_palette == 3) { // Soft Pastel
        sky0 = vec3(0.88, 0.91, 0.98); sky1 = vec3(0.98, 0.84, 0.88);
        l1 = vec3(0.72, 0.84, 0.88); l2 = vec3(0.62, 0.76, 0.82); l3 = vec3(0.56, 0.76, 0.64); l4 = vec3(0.42, 0.64, 0.52);
    } else { // Rose Garden
        sky0 = vec3(0.62, 0.85, 0.90); sky1 = vec3(0.92, 0.66, 0.63);
        l1 = vec3(0.24, 0.62, 0.71); l2 = vec3(0.14, 0.45, 0.56); l3 = vec3(0.31, 0.56, 0.36); l4 = vec3(0.18, 0.43, 0.28);
    }

    vec3 col = mix(sky0, sky1, clamp(1.0 - v_uv.y, 0.0, 1.0));
    float grain = (hash21(px * 0.5) - 0.5) * 0.045 * u_p0.w;

    // 4 layered cut-paper hills & waves with crisp cast drop shadows
    vec3 lcols[4]; lcols[0] = l1; lcols[1] = l2; lcols[2] = l3; lcols[3] = l4;
    for (int i = 0; i < 4; i++) {
        float fi = float(i);
        float baseY = u_res.y * (0.54 + fi * 0.11);
        float curve = sin(px.x * (0.004 + fi * 0.0014) + fi * 1.7 + sin(t * 0.6) * 0.15) * (32.0 - fi * 3.0)
            + cos(px.x * 0.009 - fi) * 14.0;
        float h = px.y - (baseY + curve);
        float hSh = (px.y - shDir.y) - (baseY + sin((px.x - shDir.x) * (0.004 + fi * 0.0014) + fi * 1.7) * (32.0 - fi * 3.0));
        col = mix(col, col * (1.0 - shStr), smoothstep(-2.0, 14.0, hSh) * (1.0 - smoothstep(-1.0, 1.0, h)));
        col = mix(col, lcols[i] + grain, smoothstep(-1.0, 1.0, h));
    }

    // 3-ring layered paper-cut niche windows framing every desktop icon!
    if (u_adaptive == 1 && u_icon_count > 0) {
        float fw = clamp(u_p1.z, 6.0, 40.0);
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 c = ic.xy + ic.zw * 0.5;
            for (int ring = 2; ring >= 0; ring--) {
                float fr = float(ring);
                vec2 hs = ic.zw * 0.5 + vec2(fw * (0.35 + fr * 0.35));
                float d = sdRoundBox(px - c, hs, 18.0 + fr * 4.0);
                float dIn = sdRoundBox(px - c, hs - vec2(fw * 0.33), 14.0 + fr * 3.0);
                if (d < 0.0 && dIn > 0.0) {
                    vec3 rc = (ring == 2) ? l1 : (ring == 1 ? sky1 : vec3(0.97, 0.94, 0.89));
                    col = rc + grain;
                }
            }
        }
    }
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_papercut(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        palette: st.param_f32("palette", 0.0).round() as i32,
        p0: [
            st.param_f32("light", 225.0),
            st.param_f32("shadow", 1.0),
            st.param_f32("gap", 1.0),
            st.param_f32("grain", 0.50),
        ],
        p1: [
            st.param_f32("flowers", 0.60),
            st.param_f32("sway", 1.0),
            st.param_f32("fw", 16.0),
            0.0,
        ],
        ..Default::default()
    }
}

// ================= 6. ISTANBUL (Bosphorus Sunset/Night Panorama, Skyline, Shimmering Water & Ferry Lights) =================
pub const ISTANBUL_BODY: &str = r#"
void main() {
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float t = u_time * max(0.05, u_p0.y);
    int tod = u_mode; // 0 Auto, 1 Dawn, 2 Noon, 3 Sunset, 4 Night
    float hor = u_res.y * 0.56;
    bool water = px.y > hor;

    vec3 skyT, skyB, sunC, silC, watC;
    if (tod == 4) {
        skyT = vec3(0.03, 0.06, 0.14); skyB = vec3(0.10, 0.16, 0.28); sunC = vec3(0.95, 0.88, 0.72); silC = vec3(0.04, 0.06, 0.12); watC = vec3(0.03, 0.08, 0.18);
    } else if (tod == 2) {
        skyT = vec3(0.24, 0.56, 0.86); skyB = vec3(0.68, 0.84, 0.94); sunC = vec3(1.0, 0.96, 0.82); silC = vec3(0.16, 0.24, 0.34); watC = vec3(0.12, 0.42, 0.66);
    } else { // Golden Bosphorus Sunset
        skyT = vec3(0.15, 0.14, 0.32); skyB = vec3(0.94, 0.48, 0.28); sunC = vec3(1.0, 0.82, 0.46); silC = vec3(0.11, 0.08, 0.16); watC = vec3(0.14, 0.16, 0.30);
    }

    vec2 sunP = vec2(u_res.x * 0.52, hor - u_res.y * 0.10);
    vec3 col;
    if (!water) {
        col = mix(skyT, skyB, pow(clamp(px.y / max(1.0, hor), 0.0, 1.0), 1.35));
        float sunD = length(px - sunP);
        col += sunC * exp(-sunD / (u_res.y * 0.24)) * 0.65 * u_p0.x;
        col = mix(col, vec3(1.0, 0.95, 0.82), 1.0 - smoothstep(26.0, 32.0, sunD));

        // Iconic Istanbul skyline silhouette: Hagia Sophia dome, minarets & Galata Tower
        float ridge = hor - 24.0 - fbm(vec2(px.x * 0.008, 1.0)) * 22.0;
        // Central grand dome
        float domeX = px.x - u_res.x * 0.36;
        float dome = hor - 24.0 - sqrt(max(0.0, 42.0 * 42.0 - domeX * domeX));
        ridge = min(ridge, dome);
        // Minarets
        if (abs(px.x - u_res.x * 0.31) < 3.5 || abs(px.x - u_res.x * 0.41) < 3.5) ridge = min(ridge, hor - 105.0);
        // Galata Tower on the right
        float galX = abs(px.x - u_res.x * 0.69);
        if (galX < 13.0) ridge = min(ridge, hor - 96.0 + galX * 1.4);
        if (px.y > ridge) col = silC;
    } else {
        float dy = (px.y - hor) / max(1.0, u_res.y - hor);
        col = mix(mix(watC, skyB, 0.32), watC * 0.65, dy);
        // Golden shimmering sun reflection column on the Bosphorus
        float rip = sin(px.y * 0.22 - t * 2.4 + fbm(vec2(px.x * 0.02, px.y * 0.05)) * 4.0);
        float pathW = 35.0 + dy * 170.0;
        float shimmer = exp(-abs(px.x - sunP.x + rip * 14.0) / pathW) * (0.55 + 0.45 * rip) * u_p0.x;
        col += sunC * max(0.0, shimmer) * 0.68;

        // Glowing Bosphorus ferries cruising across the strait
        for (int f = 0; f < 3; f++) {
            float ff = float(f);
            float fx = mod(t * (22.0 + ff * 9.0) * (mod(ff, 2.0) < 0.5 ? 1.0 : -1.0) + u_res.x * (0.2 + ff * 0.3), u_res.x + 120.0) - 60.0;
            float fy = hor + (u_res.y - hor) * (0.22 + ff * 0.24);
            float hull = sdRoundBox(px - vec2(fx, fy), vec2(28.0, 6.0), 4.0);
            col = mix(col, vec3(0.95, 0.94, 0.92), 1.0 - smoothstep(-0.6, 0.8, hull));
            col += vec3(1.0, 0.84, 0.46) * exp(-length(px - vec2(fx, fy - 3.0)) / 24.0) * 0.45;
        }
    }

    // Subtle warm golden tile frame around desktop icons
    float id = iconBoxField(px, clamp(u_p0.z, 6.0, 28.0), 14.0, 14.0);
    col = mix(col, sunC, (1.0 - smoothstep(0.0, 1.6, abs(id))) * 0.36);
    fragColor = vec4(dither(col), 1.0);
}
"#;

pub fn pack_istanbul(st: &AppState) -> PackedThemeParams {
    PackedThemeParams {
        mode: st.param_f32("time", 3.0).round() as i32,
        p0: [
            st.param_f32("glow", 1.0),
            st.param_f32("flow", 1.0),
            st.param_f32("pad", 14.0),
            st.param_f32("ferries", 3.0),
        ],
        ..Default::default()
    }
}
