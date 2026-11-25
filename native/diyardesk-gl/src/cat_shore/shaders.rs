pub const CAT_SHORE_VERT: &str = r#"#version 330 core
out vec2 v_uv;
void main() {
    vec2 pos = vec2((gl_VertexID == 1) ? 3.0 : -1.0, (gl_VertexID == 2) ? 3.0 : -1.0);
    // v_uv: (0,0) top-left, (1,1) bottom-right
    v_uv = vec2(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);
    gl_Position = vec4(pos, 0.0, 1.0);
}
"#;

pub const CAT_SHORE_FRAG: &str = r#"#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform sampler2D u_shore_tex;
uniform vec2 u_res;              // CSS width, height
uniform float u_time;
uniform float u_water_speed;
uniform float u_ripple_strength;
uniform int u_tone;              // 0 daylight, 1 golden, 2 twilight
uniform int u_adaptive;

// Cat transform & animation
uniform vec2 u_cat_pos;          // CSS px center of cat body
uniform float u_cat_scale;       // pixel scale radius (~95..140 px)
uniform vec2 u_gaze;             // -1..1 eye look direction
uniform float u_blink;           // 0 open .. 1 closed
uniform float u_happy;           // 0 normal .. 1 happy squint/wag
uniform float u_tail_phase;

// Interactive water click/pet ripples: (x, y, age_sec, strength)
uniform int u_click_count;
uniform vec4 u_clicks[8];

// Desktop icons: (cx, cy, half_w, half_h)
uniform int u_icon_count;
uniform vec4 u_icons[24];

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
    float s = 0.0;
    float a = 0.5;
    mat2 m = mat2(0.80, -0.60, 0.60, 0.80);
    for (int i = 0; i < 4; i++) {
        s += a * vnoise(p);
        p = m * p * 2.02 + vec2(17.1, 31.7);
        a *= 0.5;
    }
    return s;
}

float smin(float a, float b, float k) {
    float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

float sdCircle(vec2 p, float r) {
    return length(p) - r;
}

float sdEllipse(vec2 p, vec2 r) {
    float k0 = length(p / r);
    float k1 = length(p / (r * r));
    return k0 * (k0 - 1.0) / max(k1, 1e-4);
}

float sdCapsule(vec2 p, vec2 a, vec2 b, float r) {
    vec2 pa = p - a, ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

float sdTriangle(vec2 p, vec2 p0, vec2 p1, vec2 p2) {
    vec2 e0 = p1 - p0, e1 = p2 - p1, e2 = p0 - p2;
    vec2 v0 = p - p0, v1 = p - p1, v2 = p - p2;
    vec2 pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    vec2 pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    vec2 pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    float s = sign(e0.x * e2.y - e0.y * e2.x);
    vec2 d = min(min(
        vec2(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
        vec2(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))),
        vec2(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
}

// Water depth factor matching the CPU pre-baked shoreline diagonal:
// 0.0 = dry golden pebble beach (top-right), 1.0 = deep cerulean water (left & bottom-left)
float shoreWaterMask(vec2 uv) {
    // Diagonal shoreline running from top-center-left (~0.33, 0) down to right (~1.0, 0.72)
    float diag = (uv.x - 0.34) * 0.96 - (uv.y - 0.04) * 0.92;
    float wobble = (fbm(uv * vec2(7.0, 5.5) + vec2(1.3, 2.7)) - 0.5) * 0.09;
    float edge = diag + wobble;
    return smoothstep(0.18, -0.14, edge);
}

void main() {
    vec2 px = v_uv * u_res;
    float t = u_time * u_water_speed;

    float waterMask = shoreWaterMask(v_uv);
    float shallowMask = smoothstep(0.02, 0.45, waterMask);

    // Concentric painterly ripples radiating outward from the black cat (dense Ghibli gouache rings!)
    vec2 catDelta = px - (u_cat_pos + vec2(0.0, u_cat_scale * 0.20));
    vec2 rVec = vec2(catDelta.x, catDelta.y * 1.14);
    float catDist = length(rVec);
    float ang = atan(rVec.y, rVec.x);

    // Multi-scale painterly brushstroke perturbation so concentric arcs look hand-painted
    float angWarp = fbm(vec2(cos(ang) * 2.6, sin(ang) * 2.6) + rVec * 0.005 + vec2(t * 0.07, -t * 0.05));
    float strokeMod = fbm(px * 0.015 + vec2(t * 0.11, t * 0.08));
    float arcBreak = fbm(vec2(cos(ang) * 5.2, sin(ang) * 5.2) + vec2(catDist * 0.014, t * 0.09));

    float waveFreq = 0.215;
    float rWarped = catDist + (angWarp - 0.5) * 18.0 + (strokeMod - 0.5) * 8.0;
    float ringEnv = exp(-max(0.0, catDist - u_cat_scale * 0.50) / (u_cat_scale * 4.4))
                  * smoothstep(u_cat_scale * 0.38, u_cat_scale * 0.78, catDist);

    float w1 = sin(rWarped * waveFreq - t * 2.35);
    float w2 = sin(rWarped * waveFreq * 1.62 - t * 3.15 + angWarp * 3.4);
    float w3 = sin(rWarped * waveFreq * 0.52 - t * 1.45);
    float waveSum = (w1 * 0.58 + w2 * 0.27 + w3 * 0.15) * ringEnv * u_ripple_strength;

    // Gentle ambient water shimmer waves across the left/bottom water
    float ambWave = sin(px.x * 0.042 + px.y * 0.032 - t * 1.4 + strokeMod * 5.0) * 0.20 * waterMask;

    // Interactive click / pet expanding ripple rings
    float clickWave = 0.0;
    vec2 clickGrad = vec2(0.0);
    for (int i = 0; i < 8; i++) {
        if (i >= u_click_count) break;
        vec4 cl = u_clicks[i];
        vec2 dp = px - cl.xy;
        float d = length(dp);
        float rad = cl.z * 165.0;
        float env = exp(-abs(d - rad) * 0.052) * exp(-cl.z * 0.85) * cl.w;
        float cw = sin((d - rad) * 0.28) * env;
        clickWave += cw;
        clickGrad += normalize(dp + vec2(1e-4)) * cw * 3.0;
    }

    // Desktop icon adaptive ripples (when adaptive is enabled)
    float iconWave = 0.0;
    vec2 iconGrad = vec2(0.0);
    if (u_adaptive != 0 && u_icon_count > 0) {
        for (int i = 0; i < 24; i++) {
            if (i >= u_icon_count) break;
            vec4 ic = u_icons[i];
            vec2 dp = px - ic.xy;
            float baseR = max(ic.z, ic.w) * 0.92;
            float d = length(dp);
            if (d < baseR + 125.0) {
                float env = smoothstep(baseR + 125.0, baseR * 0.65, d) * smoothstep(baseR * 0.3, baseR * 0.85, d);
                float iw = sin((d - baseR) * 0.20 - t * 2.0 + float(i) * 1.7) * env * 0.48 * u_ripple_strength;
                iconWave += iw;
                iconGrad += normalize(dp + vec2(1e-4)) * iw * 1.6;
            }
        }
    }

    float totalWave = (waveSum + ambWave + clickWave + iconWave) * shallowMask;

    // Subtle refraction UV offset on the pre-baked pebble bed (zero distortion on dry beach!)
    vec2 radialDir = normalize(rVec + vec2(1e-4));
    vec2 refractPx = (radialDir * waveSum * 4.4 + clickGrad * 2.4 + iconGrad * 1.8
                   + vec2(cos(strokeMod * 6.28), sin(strokeMod * 6.28)) * 1.5 * waterMask) * shallowMask;
    vec2 sampleUv = clamp((px + refractPx) / u_res, vec2(0.001), vec2(0.999));

    vec3 col = texture(u_shore_tex, sampleUv).rgb;

    // Luminous sunlit turquoise halo around the rippled shallow pool where the cat stands
    float poolGlow = exp(-catDist * catDist / (u_cat_scale * u_cat_scale * 14.0)) * shallowMask;
    col = mix(col, vec3(0.36, 0.66, 0.79), poolGlow * 0.32);

    // Crisp painterly cel-shaded concentric ripple highlights & shadow troughs
    float arcMask = smoothstep(0.24, 0.72, arcBreak);
    float crestBand = smoothstep(0.26, 0.62, totalWave) * (0.55 + 0.45 * arcMask) * shallowMask;
    float crestCrisp = smoothstep(0.64, 0.88, totalWave + (arcBreak - 0.5) * 0.32) * shallowMask;
    float troughDark = smoothstep(-0.18, -0.68, totalWave) * (0.65 + 0.35 * arcMask) * shallowMask;

    // Color palette adjustments per tone
    vec3 glintCol = vec3(0.86, 0.95, 0.98);
    vec3 crestCol = vec3(0.50, 0.77, 0.87);
    vec3 troughCol = vec3(0.11, 0.31, 0.44);

    if (u_tone == 1) {
        // Golden afternoon
        col *= vec3(1.06, 0.98, 0.87);
        glintCol = vec3(0.99, 0.93, 0.78);
        crestCol = vec3(0.66, 0.78, 0.76);
        troughCol = vec3(0.16, 0.31, 0.40);
    } else if (u_tone == 2) {
        // Twilight dusk
        col = mix(col * vec3(0.64, 0.74, 0.92), vec3(0.08, 0.16, 0.28), 0.22);
        glintCol = vec3(0.70, 0.86, 0.98);
        crestCol = vec3(0.32, 0.55, 0.74);
        troughCol = vec3(0.05, 0.13, 0.24);
    }

    // Apply trough shadows and painted concentric turquoise/white ripple strokes
    col = mix(col, troughCol, troughDark * 0.46);
    col = mix(col, crestCol, crestBand * 0.48);
    col = mix(col, glintCol, crestCrisp * 0.62);

    // Subtle sunlit shore foam sparkle right along the water's edge
    float edgeBand = smoothstep(0.04, 0.16, waterMask) * (1.0 - smoothstep(0.22, 0.42, waterMask));
    float foamStroke = smoothstep(0.52, 0.78, fbm(px * 0.025 + vec2(t * 0.2, -t * 0.15)));
    col = mix(col, glintCol, edgeBand * foamStroke * 0.30);

    // =========================================================================
    // LONG DIAGONAL CAST SHADOW OF THE BLACK CAT (sun coming from top-right!)
    // =========================================================================
    float sc = u_cat_scale;
    float bob = sin(t * 1.9) * sc * 0.012;
    vec2 catOrigin = u_cat_pos + vec2(0.0, bob);

    vec2 sp = (px - (catOrigin + vec2(-sc * 0.06, sc * 0.38))) / sc;
    vec2 sDir = normalize(vec2(-0.54, 0.84));
    vec2 sPerp = vec2(-sDir.y, sDir.x);
    float sAlong = dot(sp, sDir);
    float sAcross = dot(sp, sPerp);
    float sWobble = sin(sAlong * 13.0 - t * 2.3 + angWarp * 3.0) * 0.045 * clamp(sAlong, 0.0, 2.8);
    float sWidth = mix(0.45, 0.68, clamp(sAlong / 2.8, 0.0, 1.0));
    float shadowBody = smoothstep(sWidth, sWidth - 0.13, abs(sAcross + sWobble))
                     * smoothstep(-0.10, 0.18, sAlong)
                     * (1.0 - smoothstep(2.2, 3.8, sAlong));
    vec3 shadowTint = (u_tone == 2) ? vec3(0.03, 0.07, 0.14) : vec3(0.07, 0.18, 0.29);
    col = mix(col, shadowTint, shadowBody * 0.64 * shallowMask);

    // =========================================================================
    // PAINTERLY BLACK CAT (Analytical SDF + Fur Tufts + Gaze Tracking Eyes!)
    // =========================================================================
    vec2 cp = (px - catOrigin) / sc;
    float headTilt = u_gaze.x * 0.055;
    float ct = cos(headTilt), st = sin(headTilt);
    vec2 hp = mat2(ct, -st, st, ct) * (cp - vec2(0.0, -0.22));

    // Fur tuft micro-displacement for hand-painted organic fluffy silhouette
    float furAngle = atan(cp.y, cp.x);
    float furNoise = (vnoise(vec2(furAngle * 13.0, 3.7)) - 0.5) * 0.022
                   + (vnoise(vec2(furAngle * 25.0 + 4.1, 7.2)) - 0.5) * 0.012;

    // 1. Compact chubby seated body + cute lower paws resting in water
    float dBody = sdEllipse(cp - vec2(0.0, 0.27), vec2(0.49, 0.45));
    float dPawL = sdEllipse(cp - vec2(-0.22, 0.62), vec2(0.16, 0.13));
    float dPawR = sdEllipse(cp - vec2(0.20, 0.63), vec2(0.16, 0.13));
    dBody = smin(dBody, min(dPawL, dPawR), 0.10);

    // 2. Round fluffy head with soft wide cheeks
    float dHead = sdEllipse(hp, vec2(0.51, 0.43));
    float dCheekL = sdCircle(hp - vec2(-0.30, 0.10), 0.23);
    float dCheekR = sdCircle(hp - vec2(0.30, 0.10), 0.23);
    dHead = smin(dHead, min(dCheekL, dCheekR), 0.12);

    // 3. Cute slightly wider kitten ears (matching frames 1, 4, 14, 15!)
    float earShift = u_gaze.x * 0.022;
    float dEarL = sdTriangle(hp, vec2(-0.44, -0.10), vec2(-0.35 + earShift, -0.66), vec2(-0.07, -0.32)) - 0.036;
    float dEarR = sdTriangle(hp, vec2(0.44, -0.10), vec2(0.36 + earShift, -0.64), vec2(0.07, -0.32)) - 0.036;
    float dHeadEars = smin(dHead, min(dEarL, dEarR), 0.065);

    // 4. Playful curved S-shaped fluffy tail sticking out to the right
    float wag = sin(u_tail_phase) * (0.06 + u_happy * 0.09);
    vec2 t0 = vec2(0.38, 0.36);
    vec2 t1 = vec2(0.56, 0.20 + wag * 0.35);
    vec2 t2 = vec2(0.72, 0.02 + wag * 0.75);
    vec2 t3 = vec2(0.90, 0.07 + wag * 1.05);
    vec2 t4 = vec2(0.97, 0.16 + wag * 1.20);
    float dTail = sdCapsule(cp, t0, t1, 0.088);
    dTail = smin(dTail, sdCapsule(cp, t1, t2, 0.080), 0.045);
    dTail = smin(dTail, sdCapsule(cp, t2, t3, 0.075), 0.045);
    dTail = smin(dTail, sdCapsule(cp, t3, t4, 0.072), 0.045);

    // Combine full cat silhouette with soft fur tuft bump
    float dCat = smin(dBody, dHeadEars, 0.10);
    dCat = smin(dCat, dTail, 0.06);
    float dCatFluffy = dCat + furNoise;

    float pxAA = 1.6 / max(sc, 20.0);

    // Delicate painted pale-blue/white water meniscus rim right where lower body meets water
    float lowerMask = smoothstep(0.08, 0.56, cp.y);
    float meniscus = smoothstep(0.028, 0.005, abs(dCatFluffy - 0.015)) * lowerMask;
    col = mix(col, glintCol, meniscus * 0.58);

    // Cat fur base (velvet ink-black with subtle warm/cool brush shading)
    float catAlpha = 1.0 - smoothstep(-pxAA, pxAA, dCatFluffy);
    if (catAlpha > 0.001) {
        float furGrain = fbm(cp * 18.0 + vec2(2.3, 5.1));
        vec3 furCol = vec3(0.028, 0.030, 0.036);
        float sunRim = smoothstep(-0.16, 0.0, dCatFluffy) * clamp(dot(normalize(cp + vec2(1e-4)), normalize(vec2(0.55, -0.82))), 0.0, 1.0);
        furCol += vec3(0.050, 0.060, 0.078) * sunRim;
        furCol += vec3(0.020, 0.022, 0.028) * furGrain;

        // Warm dark-rose inner ear shading
        float dInEarL = sdTriangle(hp, vec2(-0.35, -0.19), vec2(-0.31 + earShift, -0.52), vec2(-0.15, -0.31));
        float dInEarR = sdTriangle(hp, vec2(0.35, -0.19), vec2(0.32 + earShift, -0.50), vec2(0.15, -0.31));
        float inEar = 1.0 - smoothstep(-0.015, 0.035, min(dInEarL, dInEarR));
        vec3 innerEarCol = mix(vec3(0.22, 0.13, 0.13), vec3(0.33, 0.20, 0.18), furGrain);
        furCol = mix(furCol, innerEarCol, inEar * 0.72);

        // =====================================================================
        // BIG EXPRESSIVE YELLOW EYES + MOUSE-TRACKING PUPILS + BLINK
        // =====================================================================
        vec2 eyeLC = vec2(-0.182, -0.025);
        vec2 eyeRC = vec2(0.182, -0.015);
        float eyeRadL = 0.170;
        float eyeRadR = 0.165;

        float openH = max(0.04, 1.0 - u_blink * 0.94);
        vec2 eLP = hp - eyeLC;
        vec2 eRP = hp - eyeRC;

        float happyArchL = u_happy * 0.095 * (1.0 - clamp((eLP.x / eyeRadL) * (eLP.x / eyeRadL), 0.0, 1.0));
        float happyArchR = u_happy * 0.095 * (1.0 - clamp((eRP.x / eyeRadR) * (eRP.x / eyeRadR), 0.0, 1.0));

        float dEyeL = sdEllipse(vec2(eLP.x, (eLP.y + happyArchL) / openH), vec2(eyeRadL, eyeRadL * 1.02));
        float dEyeR = sdEllipse(vec2(eRP.x, (eRP.y + happyArchR) / openH), vec2(eyeRadR, eyeRadR * 1.02));

        for (int ei = 0; ei < 2; ei++) {
            vec2 ep = (ei == 0) ? eLP : eRP;
            float dEye = (ei == 0) ? dEyeL : dEyeR;
            float eRad = (ei == 0) ? eyeRadL : eyeRadR;
            float eyeMask = 1.0 - smoothstep(-pxAA * 1.2, pxAA * 1.2, dEye);
            if (eyeMask > 0.001 && u_blink < 0.92) {
                float grad = clamp(length(ep) / eRad, 0.0, 1.0);
                vec3 iris = mix(vec3(0.98, 0.93, 0.68), vec3(0.89, 0.76, 0.42), grad * grad);
                float lidShade = smoothstep(-eRad * 0.25, -eRad * 0.95, ep.y);
                iris *= (1.0 - lidShade * 0.14);

                vec2 pupOff = u_gaze * vec2(0.068, 0.064);
                vec2 pp = ep - pupOff;
                float pupRad = eRad * (0.56 + u_happy * 0.06);
                float dPup = sdCircle(pp, pupRad);
                float pupMask = 1.0 - smoothstep(-pxAA * 1.2, pxAA * 1.2, dPup);
                vec3 eyeCol = mix(iris, vec3(0.025, 0.028, 0.036), pupMask);

                float dSpec1 = sdCircle(pp - vec2(-0.028, -0.032), 0.025);
                float dSpec2 = sdCircle(pp - vec2(0.026, 0.028), 0.012);
                float spec = max(1.0 - smoothstep(-pxAA, pxAA, dSpec1), (1.0 - smoothstep(-pxAA, pxAA, dSpec2)) * 0.75);
                eyeCol = mix(eyeCol, vec3(1.0, 0.99, 0.96), spec * pupMask);

                float eyeRim = smoothstep(0.018, 0.0, abs(dEye));
                eyeCol = mix(eyeCol, vec3(0.05, 0.04, 0.04), eyeRim * 0.55);

                furCol = mix(furCol, eyeCol, eyeMask * (1.0 - smoothstep(0.82, 0.95, u_blink)));
            }
        }

        // Cute soft coral-pink little triangular nose
        float dNose = sdTriangle(hp, vec2(-0.048, 0.145), vec2(0.048, 0.145), vec2(0.0, 0.195)) - 0.014;
        float noseMask = 1.0 - smoothstep(-pxAA, pxAA, dNose);
        vec3 noseCol = mix(vec3(0.95, 0.66, 0.60), vec3(0.86, 0.52, 0.46), smoothstep(0.13, 0.20, hp.y));
        furCol = mix(furCol, noseCol, noseMask);

        col = mix(col, furCol, catAlpha);
    }

    // 5. Delicate curved dark whiskers extending slightly outside cheeks
    float wDist = 10.0;
    wDist = min(wDist, sdCapsule(hp, vec2(-0.34, 0.09), vec2(-0.68, 0.03), 0.008));
    wDist = min(wDist, sdCapsule(hp, vec2(-0.35, 0.14), vec2(-0.72, 0.14), 0.008));
    wDist = min(wDist, sdCapsule(hp, vec2(-0.33, 0.19), vec2(-0.66, 0.25), 0.008));
    wDist = min(wDist, sdCapsule(hp, vec2(0.34, 0.09), vec2(0.68, 0.03), 0.008));
    wDist = min(wDist, sdCapsule(hp, vec2(0.35, 0.14), vec2(0.72, 0.14), 0.008));
    wDist = min(wDist, sdCapsule(hp, vec2(0.33, 0.19), vec2(0.66, 0.25), 0.008));
    float whiskerAlpha = (1.0 - smoothstep(-pxAA, pxAA, wDist)) * 0.78;
    col = mix(col, vec3(0.04, 0.05, 0.07), whiskerAlpha);

    // Subtle 8-bit dithering
    col += (hash21(gl_FragCoord.xy) - 0.5) * (1.2 / 255.0);
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

// Crisp analytical SDF UI shader for the collapsible top notch productivity bar
pub const NOTCH_UI_VERT: &str = r#"#version 330 core
uniform vec2 u_res;    // CSS w, h
uniform vec4 u_rect;   // x, y, w, h (CSS px, top-left origin)
out vec2 v_local;      // 0..w, 0..h inside u_rect

void main() {
    vec2 c = vec2((gl_VertexID & 1) != 0 ? 1.0 : 0.0, (gl_VertexID & 2) != 0 ? 1.0 : 0.0);
    vec2 px = u_rect.xy + c * u_rect.zw;
    v_local = c * u_rect.zw;
    vec2 ndc = vec2((px.x / u_res.x) * 2.0 - 1.0, 1.0 - (px.y / u_res.y) * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

pub const NOTCH_UI_FRAG: &str = r#"#version 330 core
in vec2 v_local;
out vec4 fragColor;

uniform int u_kind;      // 0 roundBox4, 1 circle/checkbox, 2 clothesline bezier, 3 mini cat badge
uniform vec4 u_rect;     // x, y, w, h
uniform vec4 u_color;    // fill rgba
uniform vec4 u_color2;   // border / accent rgba
uniform vec4 u_params;   // kind 0: radii (tr, br, tl, bl); kind 1: (strokeW, checked, 0, 0); kind 2: (sag, thick, 0, 0)
uniform vec4 u_params2;  // kind 0: (borderW, shadowPad, 0, 0)

float sdRoundBox4(vec2 p, vec2 b, vec4 r) {
    // r = (tr, br, tl, bl)
    vec2 rr = (p.x > 0.0) ? r.xy : r.zw;
    float rad = (p.y > 0.0) ? rr.y : rr.x;
    rad = min(rad, min(b.x, b.y));
    vec2 q = abs(p) - b + vec2(rad);
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - rad;
}

float sdSegment(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a, ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h);
}

void main() {
    vec2 sz = u_rect.zw;
    vec2 p = v_local - sz * 0.5;

    if (u_kind == 0) {
        float pad = u_params2.y;
        vec2 halfB = max(vec2(1.0), sz * 0.5 - vec2(pad));
        float d = sdRoundBox4(p, halfB, u_params);

        vec4 outCol = vec4(0.0);
        if (pad > 0.5) {
            float sh = smoothstep(pad, 0.0, d) * 0.36;
            outCol = vec4(0.0, 0.0, 0.0, sh * u_color.a);
        }

        float fillA = (1.0 - smoothstep(-0.7, 0.7, d)) * u_color.a;
        outCol.rgb = mix(outCol.rgb, u_color.rgb, fillA);
        outCol.a = max(outCol.a, fillA);

        float bw = u_params2.x;
        if (bw > 0.1 && u_color2.a > 0.001) {
            float strokeA = (1.0 - smoothstep(bw * 0.5 - 0.6, bw * 0.5 + 0.6, abs(d))) * u_color2.a;
            outCol.rgb = mix(outCol.rgb, u_color2.rgb, strokeA);
            outCol.a = max(outCol.a, strokeA);
        }
        if (outCol.a < 0.002) discard;
        fragColor = outCol;
        return;
    }

    if (u_kind == 1) {
        float rad = min(sz.x, sz.y) * 0.5 - 1.0;
        float d = length(p) - rad;
        float sw = u_params.x;
        float checked = u_params.y;
        vec4 outCol = vec4(0.0);
        if (u_color.a > 0.001) {
            float fillA = (1.0 - smoothstep(-0.7, 0.7, d)) * u_color.a;
            outCol = vec4(u_color.rgb, fillA);
        }
        if (sw > 0.1 && u_color2.a > 0.001) {
            float sA = (1.0 - smoothstep(sw * 0.5 - 0.6, sw * 0.5 + 0.6, abs(d))) * u_color2.a;
            outCol.rgb = mix(outCol.rgb, u_color2.rgb, sA);
            outCol.a = max(outCol.a, sA);
        }
        if (checked > 0.5) {
            float r = rad * 0.55;
            float ck = min(
                sdSegment(p, vec2(-r * 0.65, r * 0.05), vec2(-r * 0.10, r * 0.60)),
                sdSegment(p, vec2(-r * 0.10, r * 0.60), vec2(r * 0.75, -r * 0.55))
            );
            float ckA = 1.0 - smoothstep(0.9, 1.8, ck);
            outCol.rgb = mix(outCol.rgb, u_color2.rgb, ckA);
            outCol.a = max(outCol.a, ckA);
        }
        if (outCol.a < 0.002) discard;
        fragColor = outCol;
        return;
    }

    if (u_kind == 2) {
        // Gentle sagging clothesline cord across u_rect
        float xNorm = clamp(v_local.x / max(sz.x, 1.0), 0.0, 1.0);
        float sag = u_params.x * 4.0 * xNorm * (1.0 - xNorm);
        float lineY = sz.y * 0.35 + sag;
        float dist = abs(v_local.y - lineY);
        float a = (1.0 - smoothstep(u_params.y * 0.5 - 0.6, u_params.y * 0.5 + 0.7, dist)) * u_color.a;
        if (a < 0.002) discard;
        fragColor = vec4(u_color.rgb, a);
        return;
    }

    if (u_kind == 3) {
        // Cute mini cat avatar inside rounded peach-cream square badge
        float rad = min(sz.x, sz.y) * 0.28;
        float dBox = sdRoundBox4(p, sz * 0.5 - vec2(0.5), vec4(rad));
        float boxA = 1.0 - smoothstep(-0.7, 0.7, dBox);
        if (boxA < 0.002) discard;
        vec3 col = u_color.rgb; // warm peach bg

        vec2 q = p / (min(sz.x, sz.y) * 0.5);
        // Dark cute cat head + ears
        float dHead = length(q - vec2(0.0, 0.08)) - 0.58;
        float dEarL = max(abs(q.x + 0.35) + (q.y + 0.32) * 0.55 - 0.25, q.y - 0.05);
        float dEarR = max(abs(q.x - 0.35) + (q.y + 0.32) * 0.55 - 0.25, q.y - 0.05);
        float dCat = min(dHead, min(dEarL, dEarR));
        float catM = 1.0 - smoothstep(-0.05, 0.05, dCat);
        col = mix(col, vec3(0.11, 0.11, 0.15), catM);

        // Two big bright yellow eyes + tiny pupils
        float eL = length(q - vec2(-0.20, 0.06)) - 0.17;
        float eR = length(q - vec2(0.20, 0.06)) - 0.17;
        float eyes = 1.0 - smoothstep(-0.04, 0.04, min(eL, eR));
        col = mix(col, vec3(0.98, 0.92, 0.64), eyes);

        float pL = length(q - vec2(-0.18, 0.05)) - 0.09;
        float pR = length(q - vec2(0.22, 0.05)) - 0.09;
        float pups = 1.0 - smoothstep(-0.03, 0.03, min(pL, pR));
        col = mix(col, vec3(0.05, 0.05, 0.08), pups);

        // Tiny pink nose
        float nose = 1.0 - smoothstep(-0.03, 0.03, length(q - vec2(0.0, 0.22)) - 0.06);
        col = mix(col, vec3(0.96, 0.64, 0.58), nose);

        fragColor = vec4(col, boxA * u_color.a);
        return;
    }

    discard;
}
"#;
