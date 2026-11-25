#version 330 core
in vec2 v_uv;
out vec4 fragColor;

uniform vec2 u_res;
uniform float u_time;
uniform vec4 u_mouse;
uniform int u_adaptive;
uniform int u_icon_count;
uniform vec4 u_icons[64];
uniform float u_speed;
uniform float u_glow;
uniform vec4 u_accent;

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

void main() {
    float spd = (u_speed > 0.01) ? u_speed : 1.0;
    float glowAmt = (u_glow > 0.01) ? u_glow : 1.0;
    vec3 accent = (u_accent.a > 0.1) ? u_accent.rgb : vec3(0.196, 0.773, 0.722);
    vec3 warm = vec3(0.96, 0.78, 0.48);

    // Top-down CSS pixel coordinates matching u_icons & u_mouse
    vec2 px = vec2(v_uv.x * u_res.x, (1.0 - v_uv.y) * u_res.y);
    vec2 uv = v_uv;
    float t = u_time * spd;

    // Deep midnight gradient + flowing ribbon waves
    vec3 col = mix(vec3(0.04, 0.05, 0.10), vec3(0.08, 0.11, 0.20), uv.y);
    float wave1 = sin(uv.x * 5.0 + t * 0.7 + sin(uv.y * 3.0 - t * 0.4));
    float wave2 = cos(uv.x * 8.0 - t * 0.5 + uv.y * 4.0);
    col += accent * (0.09 + 0.07 * wave1) * smoothstep(0.1, 0.9, uv.y);
    col += warm * 0.04 * max(0.0, wave2) * smoothstep(0.3, 1.0, 1.0 - uv.y);

    // Subtle geometric grid
    vec2 g = abs(fract(px / 56.0 - 0.5) - 0.5) / 0.025;
    float grid = 1.0 - min(min(g.x, g.y), 1.0);
    col += accent * grid * 0.045;

    // Interactive desktop icon aura rings
    if (u_adaptive != 0 && u_icon_count > 0) {
        int n = min(u_icon_count, 64);
        for (int i = 0; i < n; i++) {
            vec4 ic = u_icons[i];
            vec2 center = ic.xy + ic.zw * 0.5;
            vec2 halfSz = max(ic.zw * 0.5, vec2(12.0));
            float d = sdRoundBox(px - center, halfSz + vec2(6.0), 14.0);
            float halo = exp(-max(d, 0.0) * 0.045) * 0.42 * glowAmt;
            float ring = smoothstep(3.0, 0.0, abs(d - (4.0 + 3.0 * sin(t * 2.2 + float(i))))) * 0.55 * glowAmt;
            col += mix(accent, warm, 0.25) * (halo + ring);
        }
    }

    // Soft cursor spotlight
    if (u_mouse.z > 0.5) {
        float md = length(px - u_mouse.xy);
        col += mix(accent, vec3(1.0), 0.35) * exp(-md * 0.011) * 0.35;
    }

    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
