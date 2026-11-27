pub const FULLSCREEN_VERT: &str = r#"#version 330 core
out vec2 v_uv;
void main() {
    vec2 p = vec2((gl_VertexID == 1) ? 3.0 : -1.0, (gl_VertexID == 2) ? 3.0 : -1.0);
    v_uv = p * 0.5 + 0.5;
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;

pub const BACKDROP_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_res;          // CSS width, height
uniform float u_time;
uniform float u_seed;
uniform int u_light;
uniform int u_backdrop;      // 0 blobs, 1 topo waves, 2 starlit
uniform vec3 u_bg0;
uniform vec3 u_bg1;
uniform vec3 u_bg2;
uniform vec3 u_blob0;
uniform vec3 u_blob1;
uniform vec3 u_blob2;
uniform vec3 u_blob3;
uniform vec3 u_primary;
uniform vec3 u_secondary;
uniform vec3 u_on_surface;
uniform vec4 u_outline;
uniform vec4 u_primary_cont;
uniform vec2 u_pointer;
uniform int u_pointer_on;
uniform int u_icon_style;    // 0 soft pill, 1 outlined, 2 glow, 3 off
uniform float u_corner_r;
uniform int u_icon_count;
uniform vec4 u_icons[48];    // x, y, w, h (CSS px, top-left origin)
uniform vec4 u_cradle;
uniform vec4 u_cradle_stroke;

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

void main() {
    // CSS pixel coordinates (top-left origin)
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    float diag = clamp((v_uv.x + (1.0 - v_uv.y)) * 0.5, 0.0, 1.0);
    vec3 col = diag < 0.52
        ? mix(u_bg0, u_bg1, diag / 0.52)
        : mix(u_bg1, u_bg2, (diag - 0.52) / 0.48);

    // Fluid Material You gradient blobs
    vec3 blobs[4];
    blobs[0] = u_blob0;
    blobs[1] = u_blob1;
    blobs[2] = u_blob2;
    blobs[3] = u_blob3;
    float maxDim = max(u_res.x, u_res.y);
    float seedShift = mod(u_seed, 997.0) * 0.017;
    for (int i = 0; i < 4; i++) {
        float fi = float(i);
        float ang = u_time * (0.18 + fi * 0.05) + fi * 1.68 + seedShift;
        vec2 bc = vec2(
            u_res.x * (0.20 + 0.21 * fi + sin(ang) * 0.14),
            u_res.y * (0.26 + mod(fi, 2.0) * 0.44 + cos(ang * 0.85) * 0.15)
        );
        float rad = maxDim * (0.34 + 0.06 * fi);
        float d = clamp(1.0 - length(px - bc) / rad, 0.0, 1.0);
        float w = d * d * (u_light == 1 ? 0.34 : 0.36);
        if (u_light == 1) {
            col = mix(col, col * blobs[i], w);
        } else {
            col = 1.0 - (1.0 - col) * (1.0 - blobs[i] * w);
        }
    }

    // Backdrop 1: Topographic M3 Ribbon Waves
    if (u_backdrop == 1) {
        for (int b = 0; b < 6; b++) {
            float fb = float(b);
            float baseY = u_res.y * (0.22 + fb * 0.14);
            float waveY = baseY
                + sin(px.x * 0.0032 + u_time * 0.35 + fb * 1.1 + seedShift) * (28.0 + fb * 5.0)
                + cos(px.x * 0.0068 - u_time * 0.22 + fb * 0.7) * 14.0;
            float diff = px.y - waveY;
            if (diff > 0.0) {
                col = mix(col, u_light == 1 ? vec3(0.25, 0.22, 0.45) : vec3(1.0), u_light == 1 ? 0.04 : 0.03);
            }
            float lineA = 1.0 - smoothstep(0.0, 1.4, abs(diff));
            col = mix(col, u_outline.rgb, lineA * u_outline.a * 0.8);
        }
    } else if (u_backdrop == 2) {
        // Backdrop 2: Starlit Bokeh Mesh (grid-hashed deterministic stars)
        vec2 cell = floor(px / 110.0);
        vec2 loc = fract(px / 110.0);
        for (int ox = -1; ox <= 1; ox++) {
            for (int oy = -1; oy <= 1; oy++) {
                vec2 c = cell + vec2(float(ox), float(oy));
                float h1 = hash21(c + seedShift);
                float h2 = hash21(c * 1.73 + 9.2);
                vec2 starP = vec2(float(ox) + 0.15 + h1 * 0.7, float(oy) + 0.15 + h2 * 0.7);
                starP += vec2(sin(u_time * (0.3 + h1) + h2 * 6.28), cos(u_time * (0.25 + h2) + h1 * 6.28)) * 0.08;
                float dist = length(loc - starP) * 110.0;
                float r = 1.0 + h1 * 2.1;
                float tw = 0.25 + 0.75 * (0.5 + 0.5 * sin(u_time * (0.8 + h2 * 1.8) + h1 * 6.28));
                float a = (1.0 - smoothstep(r * 0.3, r + 1.1, dist)) * tw * (u_light == 1 ? 0.35 : 0.68);
                vec3 scol = (h1 < 0.33) ? u_primary : (h1 < 0.66 ? u_secondary : u_on_surface);
                col = mix(col, scol, clamp(a, 0.0, 1.0));
            }
        }
    }

    // Subtle pointer ambient aura
    if (u_pointer_on == 1) {
        float pd = clamp(1.0 - length(px - u_pointer) / 190.0, 0.0, 1.0);
        float pa = pd * pd * u_primary_cont.a * (u_light == 1 ? 0.35 : 0.48);
        col = mix(col, u_primary_cont.rgb, pa);
    }

    // Adaptive Desktop Icon Cradles
    if (u_icon_style < 3) {
        float rad = clamp(u_corner_r * 0.65, 12.0, 20.0);
        for (int i = 0; i < 48; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 center = ic.xy + ic.zw * 0.5;
            vec2 halfSz = ic.zw * 0.5 + vec2(7.0, 6.0);
            float pDist = (u_pointer_on == 1) ? length(u_pointer - center) : 999.0;
            float boost = clamp(1.0 - pDist / 150.0, 0.0, 1.0);

            if (u_icon_style == 2) {
                float gr = max(halfSz.x, halfSz.y) * (1.35 + boost * 0.2);
                float g = clamp(1.0 - length(px - center) / gr, 0.0, 1.0);
                col = mix(col, u_cradle_stroke.rgb, g * g * u_cradle_stroke.a * (0.72 + boost * 0.25));
            } else {
                float d = sdRoundBox(px - center, halfSz, rad);
                if (u_icon_style == 0) {
                    float fillA = (1.0 - smoothstep(-0.7, 0.7, d)) * u_cradle.a * (0.85 + boost * 0.15);
                    col = mix(col, u_cradle.rgb, fillA);
                }
                float sw = (u_icon_style == 1) ? (1.5 + boost * 0.6) : (1.0 + boost * 0.5);
                float strokeA = (1.0 - smoothstep(sw * 0.5 - 0.6, sw * 0.5 + 0.7, abs(d)))
                    * u_cradle_stroke.a * ((u_icon_style == 1) ? 0.85 : (0.55 + boost * 0.35));
                col = mix(col, u_cradle_stroke.rgb, strokeA);
            }
        }
    }

    // Fine subtle dithering to eliminate 8-bit gradient banding
    col += (hash21(gl_FragCoord.xy) - 0.5) * (1.2 / 255.0);
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

pub const BLUR_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform sampler2D u_tex;
uniform vec2 u_dir;      // (1/w, 0) or (0, 1/h)
uniform float u_radius;

void main() {
    if (u_radius <= 0.5) {
        fragColor = texture(u_tex, v_uv);
        return;
    }
    vec4 sum = texture(u_tex, v_uv) * 0.227027;
    float w[4];
    w[0] = 0.1945946;
    w[1] = 0.1216216;
    w[2] = 0.054054;
    w[3] = 0.016216;
    float stepMul = max(0.75, u_radius * 0.14);
    for (int i = 1; i <= 4; i++) {
        vec2 off = u_dir * (float(i) * stepMul);
        sum += texture(u_tex, clamp(v_uv + off, 0.001, 0.999)) * w[i - 1];
        sum += texture(u_tex, clamp(v_uv - off, 0.001, 0.999)) * w[i - 1];
    }
    fragColor = sum;
}
"#;

pub const BLIT_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;
uniform sampler2D u_tex;
void main() {
    fragColor = texture(u_tex, v_uv);
}
"#;

// Unified Analytical SDF UI Shader: renders frosted glass cards, Cookie 12-lobed clock,
// clock hands, Pentagon & Teardrop badges, Wavy Music Seekbar, System Rings, Kaleidoscope art & Cover art!
pub const SDF_UI_VERT: &str = r#"#version 330 core
uniform vec2 u_res;    // CSS w, h
uniform vec4 u_rect;   // x, y, w, h (CSS px, top-left origin)
out vec2 v_local;      // 0..w, 0..h inside u_rect
out vec2 v_screen_uv;  // 0..1 screen UV (OpenGL bottom-left origin)

void main() {
    vec2 c = vec2((gl_VertexID & 1) != 0 ? 1.0 : 0.0, (gl_VertexID & 2) != 0 ? 1.0 : 0.0);
    vec2 px = u_rect.xy + c * u_rect.zw;
    v_local = c * u_rect.zw;
    vec2 ndc = vec2((px.x / u_res.x) * 2.0 - 1.0, 1.0 - (px.y / u_res.y) * 2.0);
    v_screen_uv = vec2(px.x / u_res.x, 1.0 - px.y / u_res.y);
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

pub const SDF_UI_FRAG: &str = r#"#version 330 core
in vec2 v_local;
in vec2 v_screen_uv;
out vec4 fragColor;

uniform int u_kind;
uniform vec4 u_rect;      // x, y, w, h
uniform vec4 u_color;     // main rgba
uniform vec4 u_color2;    // border / secondary rgba
uniform vec4 u_color3;    // extra accent rgba
uniform vec4 u_params;    // kind-specific params
uniform vec4 u_params2;   // extra kind-specific params
uniform sampler2D u_blur_tex;
uniform sampler2D u_art_tex;

float sdRoundBox(vec2 p, vec2 b, float r) {
    float rr = min(r, min(b.x, b.y));
    vec2 q = abs(p) - b + vec2(rr);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - rr;
}

float sdRoundBox4(vec2 p, vec2 b, vec4 r) {
    // r = (topRight, bottomRight, topLeft, bottomLeft)
    r.xy = (p.x > 0.0) ? r.xy : r.zw;
    r.x  = (p.y > 0.0) ? r.y  : r.x;
    vec2 q = abs(p) - b + r.x;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r.x;
}

float sdSegment(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a, ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h);
}

// Regular 5-sided pentagon SDF (top vertex pointing up)
float sdPentagon(vec2 p, float r) {
    const vec3 k = vec3(0.809016994, 0.587785252, 0.726542528);
    p.x = abs(p.x);
    p -= 2.0 * min(dot(vec2(-k.x, k.y), p), 0.0) * vec2(-k.x, k.y);
    p -= 2.0 * min(dot(vec2( k.x, k.y), p), 0.0) * vec2( k.x, k.y);
    p -= vec2(clamp(p.x, -r * k.z, r * k.z), r);
    return length(p) * sign(p.y);
}

void main() {
    vec2 sz = u_rect.zw;
    vec2 p = v_local - sz * 0.5;

    // 0: Frosted Glass Material You Card (with analytical drop shadow + blur sample + hairline outline)
    if (u_kind == 0) {
        float pad = u_params.y; // shadow padding margin around card
        vec2 cardHalf = sz * 0.5 - vec2(pad);
        float rad = u_params.x;
        float d = sdRoundBox(p, cardHalf, rad);

        // Soft analytical drop shadow
        float ds = sdRoundBox(p - vec2(0.0, 8.0), cardHalf, rad);
        float shA = exp(-max(ds, 0.0) * 0.11) * u_params.z;

        float mask = 1.0 - smoothstep(-0.7, 0.7, d);
        if (mask < 0.002 && shA < 0.002) discard;

        vec3 blurCol = texture(u_blur_tex, clamp(v_screen_uv, 0.001, 0.999)).rgb;
        // Subtle saturation lift like CSS backdrop-filter: saturate(1.25)
        float lum = dot(blurCol, vec3(0.299, 0.587, 0.114));
        blurCol = clamp(mix(vec3(lum), blurCol, 1.22), 0.0, 1.0);

        vec3 cardCol = mix(blurCol, u_color.rgb, u_color.a);
        // Subtle top-left Material 3 Expressive glass rim sheen
        float sheen = clamp((1.0 - v_local.y / max(sz.y, 1.0)) * 0.65 + (1.0 - v_local.x / max(sz.x, 1.0)) * 0.35, 0.0, 1.0);
        float topRim = (1.0 - smoothstep(0.0, 1.8, abs(d + 0.9))) * sheen;
        cardCol = mix(cardCol, vec3(1.0), topRim * 0.14);
        // Crisp 1.1px inner outline
        float borderA = (1.0 - smoothstep(0.0, 1.35, abs(d + 0.5))) * u_color2.a;
        cardCol = mix(cardCol, u_color2.rgb, borderA);

        vec3 outRgb = mix(vec3(0.0), cardCol, mask);
        float outA = max(mask, shA * (1.0 - mask));
        fragColor = vec4(outRgb, outA);
        return;
    }

    // 1: Solid / Outlined Rounded Box or Pill
    if (u_kind == 1) {
        float rad = u_params.x;
        float strokeW = u_params.y;
        float d = sdRoundBox(p, sz * 0.5 - vec2(0.5), rad);
        float fillA = (1.0 - smoothstep(-0.6, 0.6, d)) * u_color.a;
        vec3 col = u_color.rgb;
        float outA = fillA;
        if (strokeW > 0.01) {
            float stA = (1.0 - smoothstep(strokeW * 0.5 - 0.6, strokeW * 0.5 + 0.6, abs(d + strokeW * 0.5))) * u_color2.a;
            col = mix(col, u_color2.rgb, stA);
            outA = max(fillA, stA);
        }
        if (outA < 0.003) discard;
        fragColor = vec4(col, outA);
        return;
    }

    // 2: 12-Lobed Scalloped Material 3 Cookie Clock Face + 60 Radial Ticks
    if (u_kind == 2) {
        float scale = min(sz.x, sz.y) / 236.0;
        vec2 q = p / max(scale, 0.01);
        float ang = atan(q.y, q.x);
        float r = length(q);
        float cookieR = 96.0 + 7.2 * cos(12.0 * ang);
        float d = r - cookieR;

        // Soft drop shadow
        vec2 qs = (p - vec2(0.0, 9.0 * scale)) / max(scale, 0.01);
        float rs = length(qs);
        float angS = atan(qs.y, qs.x);
        float ds = rs - (96.0 + 7.2 * cos(12.0 * angS));
        float shA = exp(-max(ds, 0.0) * 0.085) * 0.36;

        float mask = 1.0 - smoothstep(-0.9, 0.9, d);
        if (mask < 0.002 && shA < 0.002) discard;

        vec3 col = u_color.rgb;
        // Subtle radial sheen
        col *= 1.0 + 0.06 * clamp(1.0 - r / 110.0, 0.0, 1.0);

        // 60 radial tick marks inside rim
        float sector = 6.2831853 / 60.0;
        float idx = floor((ang + 3.14159265 + sector * 0.5) / sector);
        float aCenter = idx * sector - 3.14159265;
        float isHour = (mod(idx + 60.0, 5.0) < 0.5) ? 1.0 : 0.0;
        vec2 dir = vec2(cos(aCenter), sin(aCenter));
        float rOut = 83.0;
        float rIn = mix(78.0, 72.5, isHour);
        float td = sdSegment(q, dir * rIn, dir * rOut) - mix(0.75, 1.15, isHour);
        float tickA = (1.0 - smoothstep(-0.5, 0.6, td)) * mix(0.16, 0.34, isHour);
        col = mix(col, vec3(0.86, 0.93, 1.0), tickA);

        vec3 outRgb = mix(vec3(0.0), col, mask);
        float outA = max(mask, shA * (1.0 - mask));
        fragColor = vec4(outRgb, outA);
        return;
    }

    // 3: Cookie Hybrid Clock Hands (Hollow Capsule Hour Hand + Solid Pastel Capsule Minute Hand)
    if (u_kind == 3) {
        float scale = min(sz.x, sz.y) / 236.0;
        vec2 q = p / max(scale, 0.01);
        float hAng = u_params.x; // radians, 0 = up (-Y)
        float mAng = u_params.y;

        // Hour hand: hollow rounded capsule from center toward hDir
        vec2 hDir = vec2(sin(hAng), -cos(hAng));
        float hd = sdSegment(q, -hDir * 2.0, hDir * 52.0) - 6.2;
        float hStroke = 1.0 - smoothstep(1.2, 2.2, abs(hd));

        // Minute hand: solid rounded capsule from center toward mDir
        vec2 mDir = vec2(sin(mAng), -cos(mAng));
        float md = sdSegment(q, vec2(0.0), mDir * 71.0) - 6.0;
        float mFill = 1.0 - smoothstep(-0.7, 0.7, md);

        // Center pivot ring & inner dot
        float pivotOut = 1.0 - smoothstep(-0.7, 0.7, length(q) - 6.0);
        float pivotHole = 1.0 - smoothstep(-0.6, 0.6, length(q) - 2.4);

        vec4 outCol = vec4(0.0);
        // Layer 1: Hour hand stroke
        outCol = vec4(u_color.rgb, hStroke * u_color.a);
        // Layer 2: Minute hand fill on top
        float ma = max(mFill, pivotOut);
        outCol.rgb = mix(outCol.rgb, u_color2.rgb, ma);
        outCol.a = max(outCol.a, ma);
        // Layer 3: Center pivot hole
        outCol.rgb = mix(outCol.rgb, u_color3.rgb, pivotHole);
        outCol.a = max(outCol.a, pivotHole);

        if (outCol.a < 0.003) discard;
        fragColor = outCol;
        return;
    }

    // 4: Rounded 5-Sided Pentagon Badge (Month "11")
    if (u_kind == 4) {
        float r = min(sz.x, sz.y) * 0.34;
        float roundR = min(sz.x, sz.y) * 0.11;
        float d = sdPentagon(p + vec2(0.0, 1.5), r) - roundR;
        float ds = sdPentagon(p - vec2(0.0, 3.5), r) - roundR;
        float shA = exp(-max(ds, 0.0) * 0.16) * 0.30;
        float mask = 1.0 - smoothstep(-0.7, 0.7, d);
        if (mask < 0.002 && shA < 0.002) discard;
        fragColor = vec4(mix(vec3(0.0), u_color.rgb, mask), max(mask, shA * (1.0 - mask)));
        return;
    }

    // 5: Teardrop / Asymmetric Squircle Badge (Day "23")
    if (u_kind == 5) {
        vec2 hb = sz * 0.42;
        float rBig = min(hb.x, hb.y);
        float rCorner = rBig * 0.46;
        // topRight, bottomRight(tighter!), topLeft, bottomLeft
        float d = sdRoundBox4(p, hb, vec4(rBig, rCorner, rBig, rBig));
        float ds = sdRoundBox4(p - vec2(0.0, 4.0), hb, vec4(rBig, rCorner, rBig, rBig));
        float shA = exp(-max(ds, 0.0) * 0.16) * 0.32;
        float mask = 1.0 - smoothstep(-0.7, 0.7, d);
        if (mask < 0.002 && shA < 0.002) discard;
        fragColor = vec4(mix(vec3(0.0), u_color.rgb, mask), max(mask, shA * (1.0 - mask)));
        return;
    }

    // 6: Material 3 Animated Sinusoidal Wavy Music Seekbar!
    if (u_kind == 6) {
        float frac = clamp(u_params.x, 0.0, 1.0);
        float phase = u_params.y;
        float amp = u_params.z;
        float pad = 4.0;
        float usable = max(10.0, sz.x - pad * 2.0);
        float thumbX = pad + frac * usable;
        float midY = sz.y * 0.5;
        float lx = v_local.x;
        float ly = v_local.y;

        float alpha = 0.0;
        vec3 col = u_color.rgb;

        // Played left wavy segment
        float leftEnd = max(pad, thumbX - 5.5);
        if (lx >= pad - 2.0 && lx <= leftEnd + 2.0 && thumbX > pad + 3.0) {
            float edgeFade = clamp(min((lx - pad) / 8.0, (leftEnd - lx) / 8.0), 0.0, 1.0);
            float wy = midY + sin(lx * 0.23 - phase) * amp * edgeFade;
            float slope = cos(lx * 0.23 - phase) * 0.23 * amp * edgeFade;
            float dist = abs(ly - wy) / sqrt(1.0 + slope * slope);
            float capMask = smoothstep(pad - 1.5, pad + 0.5, lx) * (1.0 - smoothstep(leftEnd - 0.5, leftEnd + 1.5, lx));
            float wA = (1.0 - smoothstep(1.0, 2.0, dist)) * capMask;
            if (wA > alpha) { alpha = wA; col = u_color.rgb; }
        }

        // Remaining right straight track + bright end dot
        float rightStart = min(sz.x - pad, thumbX + 6.0);
        float rightEnd = sz.x - pad - 2.0;
        if (rightStart < rightEnd - 2.0) {
            float td = sdSegment(v_local, vec2(rightStart, midY), vec2(rightEnd, midY)) - 1.25;
            float tA = (1.0 - smoothstep(-0.6, 0.6, td)) * u_color2.a;
            if (tA > alpha) { alpha = tA; col = u_color2.rgb; }

            float dotD = length(v_local - vec2(rightEnd, midY)) - 2.0;
            float dotA = (1.0 - smoothstep(-0.6, 0.6, dotD)) * 0.78;
            if (dotA > alpha) { alpha = dotA; col = u_color3.rgb; }
        }

        // Vertical rounded playhead thumb "|"
        float thD = sdRoundBox(v_local - vec2(thumbX, midY), vec2(1.7, 8.5), 1.6);
        float thA = 1.0 - smoothstep(-0.6, 0.6, thD);
        if (thA > alpha) { alpha = thA; col = u_color.rgb; }

        if (alpha < 0.003) discard;
        fragColor = vec4(col, alpha);
        return;
    }

    // 7: Circular Progress Arc Gauge (CPU / RAM / Battery)
    if (u_kind == 7) {
        float pct = clamp(u_params.x, 0.0, 1.0);
        float rad = min(sz.x, sz.y) * 0.40;
        float thick = max(3.6, min(sz.x, sz.y) * 0.11);
        float r = length(p);
        float ringMask = 1.0 - smoothstep(thick * 0.5 - 0.7, thick * 0.5 + 0.7, abs(r - rad));
        if (ringMask < 0.003) discard;

        // Angle starting from 12 o'clock clockwise (0..1)
        float ang = atan(p.x, -p.y);
        if (ang < 0.0) ang += 6.2831853;
        float frac = ang / 6.2831853;
        bool inArc = frac <= pct;
        vec3 col = inArc ? u_color.rgb : u_color2.rgb;
        float a = ringMask * (inArc ? u_color.a : u_color2.a);
        fragColor = vec4(col, a);
        return;
    }

    // 8: Real-Time GPU Kaleidoscope Mandala Album Art (rounded square)
    if (u_kind == 8) {
        float rad = u_params.x;
        float d = sdRoundBox(p, sz * 0.5 - vec2(0.5), rad);
        float mask = 1.0 - smoothstep(-0.7, 0.7, d);
        if (mask < 0.003) discard;

        vec2 uv = p / (min(sz.x, sz.y) * 0.5);
        float r = length(uv);
        float a = atan(uv.y, uv.x) + u_params.y;
        // 12-fold kaleidoscope symmetry
        float fold = 6.2831853 / 12.0;
        float fa = abs(mod(a, fold) - fold * 0.5);
        vec2 kp = vec2(cos(fa), sin(fa)) * r;

        float pat1 = sin(kp.x * 16.0 + kp.y * 8.0) * cos(r * 11.0 - u_params.y * 1.5);
        float pat2 = sin(length(kp - vec2(0.45, 0.15)) * 18.0);
        float petals = smoothstep(-0.15, 0.65, pat1 * 0.6 + pat2 * 0.4);
        vec3 base = vec3(0.06, 0.08, 0.10);
        vec3 c = mix(base, u_color.rgb, petals * 0.75);
        c = mix(c, u_color2.rgb, smoothstep(0.35, 0.9, pat2) * 0.65);
        c = mix(c, u_color3.rgb, exp(-r * 3.8) * 0.95);
        // Subtle crisp white center starburst
        c += vec3(1.0) * exp(-r * 7.5) * 0.65;
        // Hairline border
        float br = (1.0 - smoothstep(0.0, 1.3, abs(d + 0.5))) * 0.25;
        c = mix(c, vec3(1.0), br);
        fragColor = vec4(clamp(c, 0.0, 1.0), mask);
        return;
    }

    // 9: Rounded Real Album Art Texture (SMTC Cover)
    if (u_kind == 9) {
        float rad = u_params.x;
        float d = sdRoundBox(p, sz * 0.5 - vec2(0.5), rad);
        float mask = 1.0 - smoothstep(-0.7, 0.7, d);
        if (mask < 0.003) discard;
        vec2 uv = v_local / max(sz, vec2(1.0));
        vec3 texCol = texture(u_art_tex, uv).rgb;
        float br = (1.0 - smoothstep(0.0, 1.3, abs(d + 0.5))) * 0.22;
        texCol = mix(texCol, vec3(1.0), br);
        fragColor = vec4(texCol, mask);
        return;
    }

    // 10: Crisp Analytical SDF Vector Icons (normalized -1..1 space!)
    // u_params.x: 0=Play, 1=Pause, 2=Prev, 3=Next, 4=Search, 5=Check, 6=Star, 7=Plus, 8=Close, 9=ChevronL, 10=ChevronR, 11=Swap, 12=Reset
    if (u_kind == 10) {
        float icon = u_params.x;
        float sc = min(sz.x, sz.y) * 0.5;
        vec2 q = p / max(sc, 1.0); // -1..1
        float aa = 1.4 / max(sc, 1.0);
        float d = 1e5;

        if (icon < 0.5) {
            // 0: Rounded Play Triangle pointing right
            vec2 a = vec2(-0.34, -0.50);
            vec2 b = vec2(-0.34,  0.50);
            vec2 c = vec2( 0.52,  0.00);
            float e0 = sdSegment(q, a, b);
            float e1 = sdSegment(q, b, c);
            float e2 = sdSegment(q, c, a);
            float edge = min(e0, min(e1, e2));
            // Sign test (CW in screen Y-down coords)
            float s0 = (b.x - a.x) * (q.y - a.y) - (b.y - a.y) * (q.x - a.x);
            float s1 = (c.x - b.x) * (q.y - b.y) - (c.y - b.y) * (q.x - b.x);
            float s2 = (a.x - c.x) * (q.y - c.y) - (a.y - c.y) * (q.x - c.x);
            float inside = (s0 <= 0.0 && s1 <= 0.0 && s2 <= 0.0) ? -1.0 : 1.0;
            d = inside * edge - 0.10;
        } else if (icon < 1.5) {
            // 1: Pause (two crisp rounded capsules)
            float d1 = sdRoundBox(q - vec2(-0.28, 0.0), vec2(0.15, 0.48), 0.12);
            float d2 = sdRoundBox(q - vec2( 0.28, 0.0), vec2(0.15, 0.48), 0.12);
            d = min(d1, d2);
        } else if (icon < 2.5) {
            // 2: Skip Previous (left bar + left-pointing triangle)
            float bar = sdRoundBox(q - vec2(-0.46, 0.0), vec2(0.10, 0.48), 0.06);
            vec2 a = vec2( 0.50, -0.44);
            vec2 b = vec2( 0.50,  0.44);
            vec2 c = vec2(-0.22,  0.00);
            float edge = min(sdSegment(q, a, b), min(sdSegment(q, b, c), sdSegment(q, c, a)));
            float s0 = (b.x - a.x) * (q.y - a.y) - (b.y - a.y) * (q.x - a.x);
            float s1 = (c.x - b.x) * (q.y - b.y) - (c.y - b.y) * (q.x - b.x);
            float s2 = (a.x - c.x) * (q.y - c.y) - (a.y - c.y) * (q.x - c.x);
            float tri = ((s0 >= 0.0 && s1 >= 0.0 && s2 >= 0.0) ? -1.0 : 1.0) * edge - 0.06;
            d = min(bar, tri);
        } else if (icon < 3.5) {
            // 3: Skip Next (right-pointing triangle + right bar)
            float bar = sdRoundBox(q - vec2(0.46, 0.0), vec2(0.10, 0.48), 0.06);
            vec2 a = vec2(-0.50, -0.44);
            vec2 b = vec2(-0.50,  0.44);
            vec2 c = vec2( 0.22,  0.00);
            float edge = min(sdSegment(q, a, b), min(sdSegment(q, b, c), sdSegment(q, c, a)));
            float s0 = (b.x - a.x) * (q.y - a.y) - (b.y - a.y) * (q.x - a.x);
            float s1 = (c.x - b.x) * (q.y - b.y) - (c.y - b.y) * (q.x - b.x);
            float s2 = (a.x - c.x) * (q.y - c.y) - (a.y - c.y) * (q.x - c.x);
            float tri = ((s0 <= 0.0 && s1 <= 0.0 && s2 <= 0.0) ? -1.0 : 1.0) * edge - 0.06;
            d = min(bar, tri);
        } else if (icon < 4.5) {
            // 4: Search Magnifier
            float ring = abs(length(q - vec2(-0.14, -0.14)) - 0.44) - 0.11;
            float handle = sdSegment(q, vec2(0.18, 0.18), vec2(0.62, 0.62)) - 0.11;
            d = min(ring, handle);
        } else if (icon < 5.5) {
            // 5: Crisp Checkmark
            float d1 = sdSegment(q, vec2(-0.48, 0.02), vec2(-0.12, 0.40)) - 0.14;
            float d2 = sdSegment(q, vec2(-0.12, 0.40), vec2( 0.52, -0.36)) - 0.14;
            d = min(d1, d2);
        } else if (icon < 6.5) {
            // 6: 5-Pointed Star (filled if u_params.y > 0.5, outlined otherwise)
            const vec2 k1 = vec2(0.809016994, -0.587785252);
            const vec2 k2 = vec2(-k1.x, k1.y);
            vec2 sp = vec2(q.x, -q.y);
            sp.x = abs(sp.x);
            sp -= 2.0 * max(dot(k1, sp), 0.0) * k1;
            sp -= 2.0 * max(dot(k2, sp), 0.0) * k2;
            sp.x = abs(sp.x);
            sp.y -= 0.72;
            vec2 ba = 0.46 * vec2(-k1.y, k1.x) - vec2(0.0, 1.0);
            float h = clamp(dot(sp, ba) / dot(ba, ba), 0.0, 0.72);
            float sd = length(sp - ba * h) * sign(sp.y * ba.x - sp.x * ba.y);
            d = (u_params.y > 0.5) ? sd : (abs(sd) - 0.08);
        } else if (icon < 7.5) {
            // 7: Plus (+)
            d = min(sdSegment(q, vec2(-0.48, 0.0), vec2(0.48, 0.0)), sdSegment(q, vec2(0.0, -0.48), vec2(0.0, 0.48))) - 0.12;
        } else if (icon < 8.5) {
            // 8: Close (x)
            d = min(sdSegment(q, vec2(-0.42, -0.42), vec2(0.42, 0.42)), sdSegment(q, vec2(0.42, -0.42), vec2(-0.42, 0.42))) - 0.12;
        } else if (icon < 9.5) {
            // 9: Chevron Left (<)
            d = min(sdSegment(q, vec2(0.22, -0.44), vec2(-0.26, 0.0)), sdSegment(q, vec2(-0.26, 0.0), vec2(0.22, 0.44))) - 0.12;
        } else if (icon < 10.5) {
            // 10: Chevron Right (>)
            d = min(sdSegment(q, vec2(-0.22, -0.44), vec2(0.26, 0.0)), sdSegment(q, vec2(0.26, 0.0), vec2(-0.22, 0.44))) - 0.12;
        } else if (icon < 11.5) {
            // 11: Clock Style Swap (two opposing arrows)
            float l1 = sdSegment(q, vec2(-0.48, -0.22), vec2(0.44, -0.22)) - 0.09;
            float a1 = min(sdSegment(q, vec2(0.20, -0.46), vec2(0.46, -0.22)), sdSegment(q, vec2(0.20, 0.02), vec2(0.46, -0.22))) - 0.09;
            float l2 = sdSegment(q, vec2(-0.44, 0.24), vec2(0.48, 0.24)) - 0.09;
            float a2 = min(sdSegment(q, vec2(-0.20, 0.00), vec2(-0.46, 0.24)), sdSegment(q, vec2(-0.20, 0.48), vec2(-0.46, 0.24))) - 0.09;
            d = min(min(l1, a1), min(l2, a2));
        } else {
            // 12: Reset Loop Arrow
            float ring = abs(length(q) - 0.46) - 0.10;
            if (q.x < 0.0 && q.y < -0.15) ring = 1.0;
            float tip = min(sdSegment(q, vec2(-0.46, -0.10), vec2(-0.46, -0.52)), sdSegment(q, vec2(-0.46, -0.10), vec2(-0.08, -0.10))) - 0.10;
            d = min(ring, tip);
        }

        float alpha = (1.0 - smoothstep(-aa * 0.5, aa * 0.5, d)) * u_color.a;
        if (alpha < 0.003) discard;
        fragColor = vec4(u_color.rgb, alpha);
        return;
    }

    discard;
}
"#;

pub const TEXT_VERT: &str = r#"#version 330 core
layout (location = 0) in vec2 a_pos;
layout (location = 1) in vec2 a_uv;
layout (location = 2) in vec4 a_col;

uniform vec2 u_res;
out vec2 v_uv;
out vec4 v_col;

void main() {
    v_uv = a_uv;
    v_col = a_col;
    vec2 ndc = vec2((a_pos.x / u_res.x) * 2.0 - 1.0, 1.0 - (a_pos.y / u_res.y) * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

pub const TEXT_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
in vec4 v_col;
out vec4 fragColor;

uniform sampler2D u_atlas;

void main() {
    float a = texture(u_atlas, v_uv).r * v_col.a;
    if (a < 0.004) discard;
    fragColor = vec4(v_col.rgb, a);
}
"#;

pub const AUDIO_PULSE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_res;
uniform float u_time;
uniform float u_level;
uniform float u_beat;
uniform vec3 u_accent;

void main() {
    float energy = clamp(u_level * 0.72 + u_beat * 0.85, 0.0, 1.0);
    if (energy < 0.002) discard;

    vec2 uv = v_uv;
    vec2 center = abs(uv - vec2(0.5)) * 2.0;
    float edge = smoothstep(0.62, 1.0, max(center.x, center.y));
    float vigA = edge * edge * (0.045 + u_beat * 0.085) * energy;

    // Gentle bottom harmonic wave ribbon
    float waveH = 0.032 + 0.038 * u_level + 0.045 * u_beat;
    float waveY = waveH
        + 0.012 * sin(uv.x * 11.0 + u_time * 3.2) * energy
        + 0.007 * cos(uv.x * 23.0 - u_time * 4.7) * u_beat;
    float bandA = (1.0 - smoothstep(0.0, max(0.018, waveY), uv.y)) * (0.065 + u_beat * 0.095) * energy;

    float alpha = clamp(vigA + bandA, 0.0, 0.24);
    if (alpha < 0.002) discard;
    fragColor = vec4(u_accent, alpha);
}
"#;
