//! OpenGL 3.3 Core shaders for the Celestial Engraving (`persephone`) theme.
//! Faithfully implements the `code.jpg` palette, volumetric god-rays, radial eye-bloom fill,
//! iridescent shimmer wave (`IRI`), crisp contour line tracing, and blazing spark tips.

pub const PERS_BG_VERT: &str = r#"#version 330 core
out vec2 v_uv;
void main() {
    vec2 pos = vec2((gl_VertexID == 1) ? 3.0 : -1.0, (gl_VertexID == 2) ? 3.0 : -1.0);
    // v_uv: (0,0) top-left, (1,1) bottom-right
    v_uv = vec2(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);
    gl_Position = vec4(pos, 0.0, 1.0);
}
"#;

pub const PERS_BG_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform sampler2D u_art_tex;
uniform vec2 u_res;            // CSS width, height
uniform vec4 u_art_rect;       // (x, y, w, h) in CSS px (after zoom & parallax)
uniform vec2 u_focus_uv;       // Eye/face focus UV inside portrait [0..1]
uniform vec2 u_halo_uv;        // Moon halo center UV inside portrait [0..1]
uniform float u_time;          // Current timeline seconds [0..DURATION+]
uniform float u_anim_time;     // Continuous ambient seconds (never resets)
uniform float u_fill_prog;     // 0..1 eased fill progress (FILL_START..FILL_END)
uniform float u_shimmer_prog;  // -0.5..1.5 iridescent sweep progress
uniform float u_shimmer_amp;   // 0..1 active shimmer amplitude
uniform float u_fade;          // 0..1 master cycle fade (for smooth loop transitions)
uniform int u_palette;         // 0 moonlight (exact code.jpg), 1 roseGold, 2 iceCyan
uniform int u_adaptive;
uniform int u_icon_count;
uniform vec4 u_icons[24];      // (cx, cy, half_w, half_h) CSS px

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

vec3 paletteTint(vec3 mono, int pal) {
    if (pal == 1) {
        // Rose-quartz & warm pearl gold
        return mono * vec3(1.06, 0.93, 0.96) + vec3(0.03, 0.01, 0.015) * mono.r;
    } else if (pal == 2) {
        // Ethereal frost sapphire
        return mono * vec3(0.86, 0.98, 1.08);
    }
    // Exact code.jpg silver-lavender moonlight palette
    return mono * vec3(0.95, 0.97, 1.04);
}

vec3 iridescentColor(float phase, int pal) {
    // code.jpg IRI = [(255,222,240), (226,214,255), (210,232,255)]
    vec3 c0 = vec3(1.00, 0.87, 0.94);
    vec3 c1 = vec3(0.89, 0.84, 1.00);
    vec3 c2 = vec3(0.82, 0.91, 1.00);
    if (pal == 1) {
        c0 = vec3(1.00, 0.82, 0.72);
        c1 = vec3(1.00, 0.78, 0.90);
        c2 = vec3(0.96, 0.90, 0.78);
    } else if (pal == 2) {
        c0 = vec3(0.70, 0.94, 1.00);
        c1 = vec3(0.78, 0.86, 1.00);
        c2 = vec3(0.88, 1.00, 0.98);
    }
    float p = fract(phase);
    if (p < 0.333) return mix(c0, c1, p * 3.0);
    if (p < 0.666) return mix(c1, c2, (p - 0.333) * 3.0);
    return mix(c2, c0, (p - 0.666) * 3.0);
}

void main() {
    vec2 px = v_uv * u_res;

    // 1. Exact obsidian violet-midnight BG = (8, 5, 15) from code.jpg!
    vec3 bgBase = vec3(8.0 / 255.0, 5.0 / 255.0, 15.0 / 255.0);
    if (u_palette == 1) {
        bgBase = vec3(14.0 / 255.0, 7.0 / 255.0, 14.0 / 255.0);
    } else if (u_palette == 2) {
        bgBase = vec3(5.0 / 255.0, 10.0 / 255.0, 19.0 / 255.0);
    }

    // Subtle deep-space obsidian nebula depth gradient
    vec2 haloPx = u_art_rect.xy + u_halo_uv * u_art_rect.zw;
    vec2 focusPx = u_art_rect.xy + u_focus_uv * u_art_rect.zw;
    float dHalo = length((px - haloPx) / max(u_art_rect.w, 1.0));
    vec3 nebulaGlow = (u_palette == 1)
        ? vec3(0.10, 0.05, 0.09)
        : (u_palette == 2 ? vec3(0.04, 0.09, 0.16) : vec3(0.07, 0.06, 0.14));
    vec3 col = bgBase + nebulaGlow * exp(-dHalo * dHalo * 3.2) * (0.28 + 0.72 * u_fill_prog);

    // 2. Volumetric celestial god-rays fanning upward & outward from the moon halo (frames 22, 25, 33!)
    if (u_fill_prog > 0.01) {
        vec2 dp = px - haloPx;
        float dist = length(dp) / max(u_art_rect.w, 1.0);
        float ang = atan(dp.y, dp.x);
        // Stronger rays radiating into the upper obsidian sky
        float upBias = smoothstep(0.55, -0.85, sin(ang));
        float r1 = 0.5 + 0.5 * sin(ang * 11.0 + u_anim_time * 0.14);
        float r2 = 0.5 + 0.5 * sin(ang * 19.0 - u_anim_time * 0.11 + 1.4);
        float r3 = 0.5 + 0.5 * sin(ang * 7.0 + u_anim_time * 0.07 + 2.8);
        float rays = pow(r1 * 0.5 + r2 * 0.3 + r3 * 0.2, 2.2);
        float rayFade = exp(-dist * 1.45) * smoothstep(0.04, 0.24, dist);
        vec3 rayCol = paletteTint(vec3(0.68, 0.74, 0.90), u_palette);
        col += rayCol * rays * rayFade * (0.14 + 0.24 * upBias) * u_fill_prog * u_fade;

        // Soft radiant silver aura right around the full-moon halo
        float moonBloom = exp(-dist * dist * 16.0) * 0.22 * u_fill_prog * u_fade;
        col += paletteTint(vec3(0.86, 0.90, 1.00), u_palette) * moonBloom;
    }

    // 3. Sculpted Silver-Engraving Portrait Fill Reveal (radially blooming from eyes!)
    vec2 artUv = (px - u_art_rect.xy) / max(u_art_rect.zw, vec2(1.0));
    if (artUv.x >= 0.0 && artUv.x <= 1.0 && artUv.y >= 0.0 && artUv.y <= 1.0) {
        vec3 artRaw = texture(u_art_tex, artUv).rgb;
        float artLuma = dot(artRaw, vec3(0.299, 0.587, 0.114));

        // Soft seamless vignette near portrait edges so it merges invisibly into obsidian desktop
        float edgeX = smoothstep(0.0, 0.06, artUv.x) * smoothstep(1.0, 0.94, artUv.x);
        float edgeY = smoothstep(0.0, 0.05, artUv.y) * smoothstep(1.0, 0.90, artUv.y);
        float edgeMask = edgeX * edgeY;

        // Distance from glowing eyes focus point (normalized to portrait diagonal)
        float aspect = u_art_rect.z / max(u_art_rect.w, 1.0);
        float dFocus = length(vec2((artUv.x - u_focus_uv.x) * aspect, artUv.y - u_focus_uv.y));

        // Expanding radial fill mask (matches frame 18 -> 22!)
        float waveRad = u_fill_prog * 1.18;
        float fillMask = smoothstep(waveRad + 0.14, waveRad - 0.18, dFocus) * smoothstep(0.0, 0.08, u_fill_prog);

        // Luminous white-silver bloom ring riding the expanding edge of the fill reveal (frame 18!)
        float ringEnv = exp(-pow((dFocus - waveRad) / 0.11, 2.0))
                      * sin(clamp(u_fill_prog, 0.0, 1.0) * 3.14159265);
        float eyeCore = exp(-dFocus * dFocus * 28.0) * sin(clamp(u_fill_prog * 1.25, 0.0, 1.0) * 3.14159265);

        // Map artwork luminance cleanly onto the obsidian-to-silver-lavender palette
        vec3 artTinted = paletteTint(artRaw, u_palette);
        // Lift deep background of artwork cleanly into transparent blend over obsidian sky
        float fgMask = smoothstep(0.05, 0.18, artLuma) * edgeMask;

        // Subtle iridescent mother-of-pearl shimmer wave (SHIMMER_START..13.4 + gentle living breath!)
        float diag = artUv.x * 0.65 + artUv.y * 0.75;
        float band = exp(-pow((diag - u_shimmer_prog) / 0.14, 2.0)) * u_shimmer_amp;
        // Subtle perpetual living moonlight shimmer after completion
        float breathDiag = fract(u_anim_time * 0.065);
        float breathBand = exp(-pow((diag - (breathDiag * 1.6 - 0.3)) / 0.18, 2.0)) * 0.16 * u_fill_prog;
        vec3 iriCol = iridescentColor(diag * 1.4 + u_anim_time * 0.08, u_palette);
        float highlightMask = smoothstep(0.32, 0.85, artLuma);

        vec3 artFinal = artTinted + iriCol * (band * 0.58 + breathBand) * highlightMask;
        // Gentle pulse on the brightest pure-white highlights (celestial eyes + moon rim)
        float eyeGlowMask = smoothstep(0.84, 0.98, artLuma) * exp(-dFocus * dFocus * 14.0);
        float eyePulse = 0.85 + 0.15 * sin(u_anim_time * 2.2);
        artFinal += paletteTint(vec3(0.95, 0.97, 1.00), u_palette) * eyeGlowMask * 0.28 * eyePulse;

        col = mix(col, artFinal, fgMask * fillMask * u_fade);

        // Add the blazing radial silver light wave during Fill Reveal (frame 18 magic!)
        vec3 bloomCol = paletteTint(vec3(0.92, 0.95, 1.00), u_palette);
        col += bloomCol * (ringEnv * (0.28 + 0.55 * artLuma) + eyeCore * 0.65) * edgeMask * u_fade;
    }

    // 4. Adaptive Desktop Icon Celestial Halos (gentle silver-violet cradles around icons)
    if (u_adaptive != 0 && u_icon_count > 0) {
        vec3 haloTint = paletteTint(vec3(0.74, 0.78, 0.96), u_palette);
        for (int i = 0; i < 24; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 dp = px - ic.xy;
            float r = max(ic.z, ic.w) * 1.02;
            float d = length(dp);
            if (d < r * 2.2) {
                float glow = exp(-d * d / (r * r * 1.35)) * 0.14;
                float ring = smoothstep(2.2, 0.0, abs(d - r * 1.05)) * 0.22;
                col += haloTint * (glow + ring);
            }
        }
    }

    // Subtle film-grade dithering to prevent dark-obsidian 8-bit banding
    col += (hash21(gl_FragCoord.xy) - 0.5) * (1.2 / 255.0);
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

// Progressive Silver Contour Line-Ribbon Shader (draws tens of thousands of razor-sharp strokes in 1 call!)
pub const PERS_TRACE_VERT: &str = r#"#version 330 core
layout(location = 0) in vec2 a_uv;
layout(location = 1) in vec2 a_norm;
layout(location = 2) in float a_side;
layout(location = 3) in float a_t_reveal;
layout(location = 4) in float a_strength;

uniform vec2 u_res;
uniform vec4 u_art_rect;
uniform float u_line_half_w;   // CSS px half-width (~1.15 px)
uniform float u_time;

out float v_side;
out float v_age;
out float v_strength;
out vec2 v_art_uv;

void main() {
    float age = u_time - a_t_reveal;
    // Slight width swell right at the freshly glowing tip!
    float tipBoost = 1.0 + 0.55 * exp(-max(0.0, age) * 4.5);
    vec2 centerPx = u_art_rect.xy + a_uv * u_art_rect.zw;
    vec2 normPx = normalize(a_norm * u_art_rect.zw + vec2(1e-6));
    vec2 px = centerPx + normPx * (u_line_half_w * tipBoost);

    vec2 ndc = vec2((px.x / u_res.x) * 2.0 - 1.0, 1.0 - (px.y / u_res.y) * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_side = a_side;
    v_age = age;
    v_strength = a_strength;
    v_art_uv = a_uv;
}
"#;

pub const PERS_TRACE_FRAG: &str = r#"#version 330 core
in float v_side;
in float v_age;
in float v_strength;
in vec2 v_art_uv;

out vec4 fragColor;

uniform float u_fill_prog;
uniform float u_shimmer_prog;
uniform float u_shimmer_amp;
uniform float u_fade;
uniform int u_palette;

void main() {
    if (v_age < 0.0) discard;

    // Smooth anti-aliased stroke cross-section
    float aa = 1.0 - smoothstep(0.18, 0.96, abs(v_side));
    float revealFade = smoothstep(0.0, 0.14, v_age);
    float hotTip = exp(-v_age * 4.2);

    // Exact code.jpg LINE = (215, 222, 235) & SILVER = (235, 242, 255)
    vec3 lineCol = vec3(215.0 / 255.0, 222.0 / 255.0, 235.0 / 255.0);
    vec3 hotCol = vec3(0.97, 0.98, 1.00);
    if (u_palette == 1) {
        lineCol = vec3(0.95, 0.82, 0.88);
        hotCol = vec3(1.00, 0.94, 0.92);
    } else if (u_palette == 2) {
        lineCol = vec3(0.76, 0.91, 1.00);
        hotCol = vec3(0.94, 0.99, 1.00);
    }

    // Iridescent shimmer highlight along contour strokes
    float diag = v_art_uv.x * 0.65 + v_art_uv.y * 0.75;
    float band = exp(-pow((diag - u_shimmer_prog) / 0.13, 2.0)) * u_shimmer_amp;
    vec3 iri = (u_palette == 1) ? vec3(1.0, 0.84, 0.76) : vec3(0.89, 0.84, 1.00);

    vec3 col = mix(lineCol, hotCol, clamp(hotTip * 0.85, 0.0, 1.0));
    col = mix(col, iri, band * 0.65);

    // During Fill Reveal, keep crisp subtle silver engraving edge definition without overpowering the shading
    float settledAlpha = mix(0.88, 0.24, u_fill_prog);
    float alpha = aa * revealFade * (settledAlpha * v_strength + hotTip * 0.45 + band * 0.35) * u_fade;
    if (alpha < 0.004) discard;

    fragColor = vec4(col, clamp(alpha, 0.0, 1.0));
}
"#;

// Instanced Blazing Star Spark, 4-Point Twinkle & Stardust Sprite Shader
pub const PERS_SPARK_VERT: &str = r#"#version 330 core
layout(location = 0) in vec4 a_pos_rad_kind; // x, y (CSS px), radius (CSS px), kind (0=spark, 1=twinkle, 2=dust)
layout(location = 1) in vec4 a_col;          // rgba

uniform vec2 u_res;

out vec2 v_local;
out float v_kind;
out vec4 v_col;

void main() {
    vec2 corner = vec2((gl_VertexID & 1) != 0 ? 1.0 : -1.0, (gl_VertexID & 2) != 0 ? 1.0 : -1.0);
    vec2 px = a_pos_rad_kind.xy + corner * a_pos_rad_kind.z;
    vec2 ndc = vec2((px.x / u_res.x) * 2.0 - 1.0, 1.0 - (px.y / u_res.y) * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_local = corner;
    v_kind = a_pos_rad_kind.w;
    v_col = a_col;
}
"#;

pub const PERS_SPARK_FRAG: &str = r#"#version 330 core
in vec2 v_local;
in float v_kind;
in vec4 v_col;

out vec4 fragColor;

void main() {
    float r = length(v_local);
    if (r >= 1.0) discard;

    float shape = 0.0;
    if (v_kind < 0.5) {
        // Blazing contour-tracing tip spark: hot pinpoint core + silky radial halo + tiny cross flare!
        float core = exp(-r * r * 18.0);
        float halo = exp(-r * r * 4.2) * 0.58;
        float flareH = exp(-abs(v_local.y) * 14.0) * exp(-abs(v_local.x) * 3.8) * 0.55;
        float flareV = exp(-abs(v_local.x) * 14.0) * exp(-abs(v_local.y) * 3.8) * 0.55;
        shape = clamp(core + halo + flareH + flareV, 0.0, 1.0) * (1.0 - smoothstep(0.75, 1.0, r));
    } else if (v_kind < 1.5) {
        // Crisp 4-point celestial diamond twinkle star!
        float core = exp(-r * r * 24.0);
        float spikeX = exp(-abs(v_local.y) * 18.0) * exp(-abs(v_local.x) * 2.6);
        float spikeY = exp(-abs(v_local.x) * 18.0) * exp(-abs(v_local.y) * 2.6);
        float glow = exp(-r * r * 5.0) * 0.35;
        shape = clamp(core + (spikeX + spikeY) * 0.85 + glow, 0.0, 1.0) * (1.0 - smoothstep(0.80, 1.0, r));
    } else {
        // Soft floating iridescent stardust mote
        shape = exp(-r * r * 5.5) * (1.0 - smoothstep(0.70, 1.0, r));
    }

    float a = shape * v_col.a;
    if (a < 0.003) discard;
    fragColor = vec4(v_col.rgb, a);
}
"#;
