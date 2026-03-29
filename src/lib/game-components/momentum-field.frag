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
uniform float u_mouse_down;
uniform float u_aspect_ratio;
uniform float u_p_trend_warm_r;
uniform float u_p_trend_warm_g;
uniform float u_p_advantage;
uniform float u_p_trend_norm;
uniform float u_p_metabolism;
uniform float u_p_density;
uniform float u_p_urgency;
uniform float u_p_confidence;
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

vec3 cosine_palette(float t, vec3 a, vec3 b, vec3 c, vec3 d){
    return a + b * cos(6.28318 * (c * t + d));
}

vec3 aces_tonemap(vec3 x) {
    vec3 a = x * (2.51 * x + 0.03);
    vec3 b = x * (2.43 * x + 0.59) + 0.14;
    return clamp(a / b, 0.0, 1.0);
}

float dither_noise(vec2 uv) {
    return fract(52.9829189 * fract(dot(uv, vec2(0.06711056, 0.00583715))));
}

void main(){
    vec2 uv = v_uv * 2.0 - 1.0;
    float aspect = u_aspect_ratio;
    float time = fract(u_time / 120.0) * 120.0;
    float mouse_x = u_mouse.x;
    float mouse_y = u_mouse.y;
    float mouse_down = u_mouse_down;

    float trend_warm_r = u_p_trend_warm_r;
    float trend_warm_g = u_p_trend_warm_g;
    float advantage = u_p_advantage;
    float trend_norm = u_p_trend_norm;
    float metabolism = u_p_metabolism;
    float density = u_p_density;
    float urgency = u_p_urgency;
    float confidence = u_p_confidence;

    vec4 final_color = vec4(0.0, 0.0, 0.0, 0.0);

    // ── Layer 0: flow ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        { float warp_x = fbm2(p * 2.500000 + vec2(0.0, 1.3), int(3.000000), 0.040000, 2.000000);
        float warp_y = fbm2(p * 2.500000 + vec2(1.7, 0.0), int(3.000000), 0.040000, 2.000000);
        p = p + vec2(warp_x, warp_y) * 0.040000; }
        float sdf_result = fbm2((p * 4.000000 + vec2(time * 0.1, time * 0.07)), int(3.000000), 0.450000, 2.000000);
        vec3 pal_rgb = cosine_palette(sdf_result, vec3(0.030000, 0.030000, 0.040000), vec3(0.150000, 0.120000, 0.060000), vec3(0.600000, 0.400000, 0.150000), vec3(0.000000, 0.080000, 0.120000));
        vec4 color_result = vec4(pal_rgb, clamp(dot(pal_rgb, vec3(0.299, 0.587, 0.114)) * 2.0, 0.0, 1.0));
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.920000);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 1: band ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        { float warp_x = fbm2(p * 1.800000 + vec2(0.0, 1.3), int(2.000000), 0.050000, 2.000000);
        float warp_y = fbm2(p * 1.800000 + vec2(1.7, 0.0), int(2.000000), 0.050000, 2.000000);
        p = p + vec2(warp_x, warp_y) * 0.050000; }
        float sdf_result = abs(length(p) - 0.500000) - 0.020000;
        float glow_pulse = 1.500000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(trend_warm_r, trend_warm_g, 0.120000), color_result.a);
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.880000);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 2: alert ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        vec4 color_result;
        {
            vec2 p_then = p;
            vec4 then_color;
            vec4 else_color;
            { vec2 p = p_then;
            float sdf_result = noise2(p * 2.500000 + vec2(time * 0.1, time * 0.07));
            float glow_pulse = 0.800000 * (0.9 + 0.1 * sin(time * 2.0));
            float glow_result = apply_glow(sdf_result, glow_pulse);

            vec4 color_result = vec4(vec3(glow_result), glow_result);
            color_result = vec4(color_result.rgb * vec3(0.850000, 0.250000, 0.080000), color_result.a);
            then_color = color_result; }
            { vec2 p = p_then;
            float sdf_result = noise2(p * 2.500000 + vec2(time * 0.1, time * 0.07));
            float glow_pulse = 0.100000 * (0.9 + 0.1 * sin(time * 2.0));
            float glow_result = apply_glow(sdf_result, glow_pulse);

            vec4 color_result = vec4(vec3(glow_result), glow_result);
            color_result = vec4(color_result.rgb * vec3(0.050000, 0.050000, 0.080000), color_result.a);
            else_color = color_result; }
            color_result = (urgency > 0.300000) ? then_color : else_color;
        }
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    final_color = vec4(aces_tonemap(final_color.rgb), final_color.a);
    final_color += (dither_noise(v_uv * u_resolution) - 0.5) / 255.0;
    fragColor = final_color;
}
