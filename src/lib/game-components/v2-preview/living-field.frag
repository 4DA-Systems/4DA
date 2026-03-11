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
uniform float u_p_life;
uniform float u_p_warmth;
uniform float u_p_density;
uniform sampler2D u_prev_frame;


in vec2 v_uv;
out vec4 fragColor;

float sdf_circle(vec2 p, float radius){
    return length(p) - radius;
}

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

void main(){
    vec2 uv = v_uv * 2.0 - 1.0;
    float aspect = u_resolution.x / u_resolution.y;
    float time = fract(u_time / 120.0) * 120.0;

    float life = u_p_life;
    float warmth = u_p_warmth;
    float density = u_p_density;

    vec4 final_color = vec4(0.0, 0.0, 0.0, 0.0);

    // ── Layer 1: ground ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        { float warp_x = fbm2(p * 1.000000 + vec2(0.0, 1.3), int(5.000000), 0.500000, 0.300000);
        float warp_y = fbm2(p * 1.000000 + vec2(1.7, 0.0), int(5.000000), 0.500000, 0.300000);
        p = p + vec2(warp_x, warp_y) * 0.300000; }
        float sdf_result = fbm2((p * 2.000000 + vec2(time * 0.1, time * 0.07)), int(4.000000), 0.500000, 2.000000);
        vec3 pal_rgb = cosine_palette(sdf_result, vec3(0.500000, 0.400000, 0.300000), vec3(0.300000, 0.300000, 0.200000), vec3(0.800000, 0.600000, 0.300000), vec3(0.100000, 0.150000, 0.050000));
        vec4 color_result = vec4(pal_rgb, clamp(dot(pal_rgb, vec3(0.299, 0.587, 0.114)) * 2.0, 0.0, 1.0));
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.930000);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 2: organisms ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        p = p + vec2(sin(p.y * 4.000000 + time * 0.400000), cos(p.x * 4.000000 + time * 0.400000)) * 0.200000;
        p = vec2(mod(p.x + 0.150000 * 0.5, 0.150000) - 0.150000 * 0.5, mod(p.y + 0.150000 * 0.5, 0.150000) - 0.150000 * 0.5);
        float sdf_result = sdf_circle(p, 0.030000);
        float glow_pulse = 3.000000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(1.000000, 0.700000, 0.300000), color_result.a);
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.870000);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 3: trails ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        p = p + vec2(sin(p.y * 3.000000 + time * 0.300000), cos(p.x * 3.000000 + time * 0.300000)) * 0.150000;
        { float warp_x = fbm2(p * 2.000000 + vec2(0.0, 1.3), int(4.000000), 0.250000, 2.000000);
        float warp_y = fbm2(p * 2.000000 + vec2(1.7, 0.0), int(4.000000), 0.250000, 2.000000);
        p = p + vec2(warp_x, warp_y) * 0.250000; }
        float sdf_result = noise2(p * 6.000000 + vec2(time * 0.1, time * 0.07));
        float glow_pulse = 1.500000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(0.800000, 0.500000, 0.200000), color_result.a);
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.960000);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 4: pollen ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        { float warp_x = fbm2(p * 5.000000 + vec2(0.0, 1.3), int(3.000000), 0.300000, 2.000000);
        float warp_y = fbm2(p * 5.000000 + vec2(1.7, 0.0), int(3.000000), 0.300000, 2.000000);
        p = p + vec2(warp_x, warp_y) * 0.300000; }
        float sdf_result = fbm2((p * 12.000000 + vec2(time * 0.1, time * 0.07)), int(3.000000), 0.500000, 2.000000);
        float glow_pulse = 2.500000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(1.000000, 0.900000, 0.500000), color_result.a);
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.800000);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 5: warmth_glow ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = smoothstep(0.000000, 0.700000, length(p));
        vec3 pal_rgb = cosine_palette(sdf_result, vec3(0.500000, 0.400000, 0.200000), vec3(0.400000, 0.300000, 0.200000), vec3(0.800000, 0.500000, 0.200000), vec3(0.000000, 0.100000, 0.000000));
        vec4 color_result = vec4(pal_rgb, clamp(dot(pal_rgb, vec3(0.299, 0.587, 0.114)) * 2.0, 0.0, 1.0));
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 6: fireflies ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        p = p + vec2(sin(p.y * 7.000000 + time * 0.800000), cos(p.x * 7.000000 + time * 0.800000)) * 0.250000;
        { float warp_x = fbm2(p * 3.000000 + vec2(0.0, 1.3), int(3.000000), 0.350000, 2.000000);
        float warp_y = fbm2(p * 3.000000 + vec2(1.7, 0.0), int(3.000000), 0.350000, 2.000000);
        p = p + vec2(warp_x, warp_y) * 0.350000; }
        float sdf_result = fbm2((p * 18.000000 + vec2(time * 0.1, time * 0.07)), int(2.000000), 0.500000, 2.000000);
        float glow_pulse = 4.000000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(1.000000, 1.000000, 0.700000), color_result.a);
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.720000);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    fragColor = final_color;
}
