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
uniform float u_p_intensity;
uniform float u_p_green;
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

void main(){
    vec2 uv = v_uv * 2.0 - 1.0;
    float aspect = u_resolution.x / u_resolution.y;
    float time = fract(u_time / 120.0) * 120.0;

    float intensity = u_p_intensity;
    float green = u_p_green;

    // ── Layer 1: orb ──
    vec2 p = vec2(uv.x * aspect, uv.y);
    vec4 color_result;
    {
        vec2 p_then = p;
        vec4 then_color;
        vec4 else_color;
        { vec2 p = p_then;
        { float warp_x = fbm2(p * 3.000000 + vec2(0.0, 1.3), int(2.000000), 0.080000, 2.000000);
        float warp_y = fbm2(p * 3.000000 + vec2(1.7, 0.0), int(2.000000), 0.080000, 2.000000);
        p = p + vec2(warp_x, warp_y) * 0.080000; }
        float sdf_result = sdf_circle(p, 0.220000);
        float glow_pulse = 3.000000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(0.150000, 0.850000, 0.300000), color_result.a);
        then_color = color_result; }
        { vec2 p = p_then;
        { float warp_x = fbm2(p * 3.000000 + vec2(0.0, 1.3), int(2.000000), 0.080000, 2.000000);
        float warp_y = fbm2(p * 3.000000 + vec2(1.7, 0.0), int(2.000000), 0.080000, 2.000000);
        p = p + vec2(warp_x, warp_y) * 0.080000; }
        float sdf_result = sdf_circle(p, 0.220000);
        float glow_pulse = 3.000000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(0.830000, 0.500000, 0.100000), color_result.a);
        else_color = color_result; }
        color_result = (green > 0.500000) ? then_color : else_color;
    }
    vec4 prev_color = texture(u_prev_frame, v_uv);
    color_result = mix(color_result, prev_color, 0.850000);
    fragColor = color_result;
}
