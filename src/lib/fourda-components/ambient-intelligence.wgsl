struct Uniforms {
    time: f32,
    audio_bass: f32,
    audio_mid: f32,
    audio_treble: f32,
    audio_energy: f32,
    audio_beat: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    p_pulse: f32,
    p_heat: f32,
    p_burst: f32,
    p_morph: f32,
    p_error_val: f32,
    p_staleness: f32,
    p_opacity_val: f32,
    p_signal_intensity: f32,
    p_color_shift: f32,
    p_critical_count: f32,
    p_metabolism: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

@group(1) @binding(0) var prev_frame: texture_2d<f32>;
@group(1) @binding(1) var prev_sampler: sampler;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn sdf_circle(p: vec2<f32>, radius: f32) -> f32 {
    return length(p) - radius;
}

fn apply_glow(d: f32, intensity: f32) -> f32 {
    return exp(-max(d, 0.0) * intensity * 8.0);
}

fn hash2(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + vec3<f32>(dot(p3, p3.yzx + 33.33));
    return fract((p3.x + p3.y) * p3.z);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u_v = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i), hash2(i + vec2<f32>(1.0, 0.0)), u_v.x),
        mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u_v.x),
        u_v.y
    ) * 2.0 - 1.0;
}

fn fbm2(p: vec2<f32>, octaves: i32, persistence: f32, lacunarity: f32) -> f32 {
    var value: f32 = 0.0;
    var amplitude: f32 = 1.0;
    var frequency: f32 = 1.0;
    var max_val: f32 = 0.0;
    for (var i: i32 = 0; i < octaves; i = i + 1) {
        value = value + noise2(p * frequency) * amplitude;
        max_val = max_val + amplitude;
        amplitude = amplitude * persistence;
        frequency = frequency * lacunarity;
    }
    return value / max_val;
}

fn cosine_palette(t: f32, a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return a + b * cos(6.28318 * (c * t + d));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv * 2.0 - 1.0;
    let aspect = u.resolution.x / u.resolution.y;
    let time = fract(u.time / 120.0) * 120.0;

    let pulse = u.p_pulse;
    let heat = u.p_heat;
    let burst = u.p_burst;
    let morph = u.p_morph;
    let error_val = u.p_error_val;
    let staleness = u.p_staleness;
    let opacity_val = u.p_opacity_val;
    let signal_intensity = u.p_signal_intensity;
    let color_shift = u.p_color_shift;
    let critical_count = u.p_critical_count;
    let metabolism = u.p_metabolism;

    var final_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // ── Layer 1: nebula ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        { let warp_x = fbm2(p * 2.500000 + vec2<f32>(0.0, 1.3), i32(3.000000), 0.120000, 2.000000);
        let warp_y = fbm2(p * 2.500000 + vec2<f32>(1.7, 0.0), i32(3.000000), 0.120000, 2.000000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.120000; }
        var sdf_result = fbm2((p * 5.000000 + vec2<f32>(time * 0.1, time * 0.07)), i32(3.000000), 0.400000, 2.000000);
        let pal_rgb = cosine_palette(sdf_result, vec3<f32>(0.060000, 0.030000, 0.010000), vec3<f32>(0.350000, 0.200000, 0.080000), vec3<f32>(1.000000, 0.700000, 0.500000), vec3<f32>(0.000000, 0.120000, 0.250000));
        var color_result = vec4<f32>(pal_rgb, clamp(dot(pal_rgb, vec3<f32>(0.299, 0.587, 0.114)) * 2.0, 0.0, 1.0));
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.930000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 2: core ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        var color_result: vec4<f32>;
        {
            var p_then = p;
            var then_color: vec4<f32>;
            var else_color: vec4<f32>;
            { var p = p_then;
            p = p + vec2<f32>(sin(p.y * 4.000000 + time * 1.200000), cos(p.x * 4.000000 + time * 1.200000)) * 0.150000;
            var sdf_result = sdf_circle(p, 0.080000);
            let glow_pulse = 3.500000 * (0.9 + 0.1 * sin(time * 2.0));
            let glow_result = apply_glow(sdf_result, glow_pulse);
            var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
            color_result = vec4<f32>(color_result.rgb * vec3<f32>(1.000000, 0.200000, 0.080000), color_result.a);
            then_color = color_result; }
            { var p = p_then;
            var sdf_result = sdf_circle(p, 0.070000);
            let glow_pulse = 2.800000 * (0.9 + 0.1 * sin(time * 2.0));
            let glow_result = apply_glow(sdf_result, glow_pulse);
            var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
            color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.830000, 0.650000, 0.180000), color_result.a);
            else_color = color_result; }
            color_result = select(else_color, then_color, (critical_count > 0.000000));
        }
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.880000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 3: corona ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        p = vec2<f32>(length(p), atan2(p.y, p.x));
        var sdf_result = noise2(p * 8.000000 + vec2<f32>(time * 0.1, time * 0.07));
        let glow_pulse = 0.600000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.250000, 0.150000, 0.060000), color_result.a);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    return final_color;
}
