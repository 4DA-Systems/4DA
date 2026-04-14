#version 300 es
precision highp float;

uniform float u_time;
uniform float u_audio_bass;
uniform float u_audio_mid;
uniform float u_audio_treble;
uniform float u_audio_energy;
uniform float u_audio_beat;
uniform vec2 u_resolution;
uniform vec2 u_mouse;
uniform float u_p_progress;
uniform float u_p_urgency;
uniform sampler2D u_prev_frame;


in vec2 v_uv;
out vec4 fragColor;

float apply_glow(float d, float intensity){
    return exp(-max(d, 0.0) * intensity * 8.0);
}

float hash2(vec2 p){
    vec3 p3 = fract(vec3(p.x, p.y, p.x) * 0.1031);
    p3 += vec3(dot(p3, p3.yzx + 33.33));
    return fract((p3.x + p3.y) * p3.z);
}

float noise2(vec2 p){
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i), hash2(i + vec2(1.0, 0.0)), u.x),
        mix(hash2(i + vec2(0.0, 1.0)), hash2(i + vec2(1.0, 1.0)), u.x),
        u.y
    ) * 2.0 - 1.0;
}

float fbm2(vec2 p, int octaves, float persistence, float lacunarity){
    float value = 0.0;
    float amplitude = 1.0;
    float frequency = 1.0;
    float max_val = 0.0;
    for (int i = 0; i < octaves; i++) {
        value += noise2(p * frequency) * amplitude;
        max_val += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    return value / max_val;
}

void main(){
    vec2 uv = v_uv * 2.0 - 1.0;
    float aspect = u_resolution.x / u_resolution.y;
    float time = fract(u_time / 120.0) * 120.0;

    float progress = u_p_progress;
    float urgency = u_p_urgency;

    vec4 final_color = vec4(0.0, 0.0, 0.0, 0.0);

    // ── Layer 1: track ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = abs(length(p) - 0.340000) - 0.025000;
        float glow_pulse = 1.200000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(0.200000, 0.200000, 0.200000), color_result.a);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 2: countdown ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        vec4 color_result;
        {
            vec2 p_then = p;
            vec4 then_color;
            vec4 else_color;
            { vec2 p = p_then;
            { float warp_x = fbm2(p * 5.000000 + vec2(0.0, 1.3), int(2.000000), 0.050000, 2.000000);
            float warp_y = fbm2(p * 5.000000 + vec2(1.7, 0.0), int(2.000000), 0.050000, 2.000000);
            p = p + vec2(warp_x, warp_y) * 0.050000; }
            float sdf_result = abs(length(p) - 0.340000) - 0.050000;
            float arc_theta = atan(p.x, p.y) + 3.14159265359;
            sdf_result = (arc_theta < progress ? sdf_result : 999.0);
            float glow_pulse = 3.000000 * (0.9 + 0.1 * sin(time * 2.0));
            float glow_result = apply_glow(sdf_result, glow_pulse);

            vec4 color_result = vec4(vec3(glow_result), glow_result);
            color_result = vec4(color_result.rgb * vec3(1.000000, 0.150000, 0.080000), color_result.a);
            then_color = color_result; }
            { vec2 p = p_then;
            { float warp_x = fbm2(p * 5.000000 + vec2(0.0, 1.3), int(2.000000), 0.050000, 2.000000);
            float warp_y = fbm2(p * 5.000000 + vec2(1.7, 0.0), int(2.000000), 0.050000, 2.000000);
            p = p + vec2(warp_x, warp_y) * 0.050000; }
            float sdf_result = abs(length(p) - 0.340000) - 0.050000;
            float arc_theta = atan(p.x, p.y) + 3.14159265359;
            sdf_result = (arc_theta < progress ? sdf_result : 999.0);
            float glow_pulse = 2.200000 * (0.9 + 0.1 * sin(time * 2.0));
            float glow_result = apply_glow(sdf_result, glow_pulse);

            vec4 color_result = vec4(vec3(glow_result), glow_result);
            color_result = vec4(color_result.rgb * vec3(0.830000, 0.550000, 0.120000), color_result.a);
            else_color = color_result; }
            color_result = (urgency > 0.700000) ? then_color : else_color;
        }
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.870000);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    fragColor = final_color;
}
