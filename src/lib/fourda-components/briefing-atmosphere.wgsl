struct Uniforms {
    time: f32,
    audio_bass: f32,
    audio_mid: f32,
    audio_treble: f32,
    audio_energy: f32,
    audio_beat: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    p_signal_heat: f32,
    p_decision_pressure: f32,
    p_quality: f32,
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

fn apply_color_matrix(color: vec3f) -> vec3f {
    let m = mat3x3f(
        vec3f(1.1, 0.05, -0.02),
        vec3f(-0.05, 1.05, 0),
        vec3f(0, -0.02, 0.95)
    );
    return clamp(m * color, vec3f(0.0), vec3f(1.0));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv * 2.0 - 1.0;
    let aspect = u.resolution.x / u.resolution.y;
    let time = fract(u.time / 120.0) * 120.0;

    let signal_heat = u.p_signal_heat;
    let decision_pressure = u.p_decision_pressure;
    let quality = u.p_quality;

    var final_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // ── Layer 0: fog ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        { let warp_x = fbm2(p * 2.000000 + vec2<f32>(0.0, 1.3), i32(3.000000), 0.100000, 2.000000);
        let warp_y = fbm2(p * 2.000000 + vec2<f32>(1.7, 0.0), i32(3.000000), 0.100000, 2.000000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.100000; }
        var sdf_result = fbm2((p * 3.000000 + vec2<f32>(time * 0.1, time * 0.07)), i32(4.000000), 0.600000, 2.000000);
        let pal_rgb = cosine_palette(sdf_result, vec3<f32>(0.040000, 0.040000, 0.060000), vec3<f32>(0.120000, 0.100000, 0.150000), vec3<f32>(0.600000, 0.500000, 0.700000), vec3<f32>(0.000000, 0.050000, 0.150000));
        var color_result = vec4<f32>(pal_rgb, clamp(dot(pal_rgb, vec3<f32>(0.299, 0.587, 0.114)) * 2.0, 0.0, 1.0));
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.920000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 1: heat ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        { let warp_x = fbm2(p * 3.000000 + vec2<f32>(0.0, 1.3), i32(2.000000), 0.150000, 2.000000);
        let warp_y = fbm2(p * 3.000000 + vec2<f32>(1.7, 0.0), i32(2.000000), 0.150000, 2.000000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.150000; }
        var sdf_result = noise2(p * 2.000000 + vec2<f32>(time * 0.1, time * 0.07));
        let pal_rgb = cosine_palette(sdf_result, vec3<f32>(0.000000, 0.000000, 0.000000), vec3<f32>(signal_heat, 0.200000, 0.050000), vec3<f32>(1.000000, 0.700000, 0.300000), vec3<f32>(0.000000, 0.100000, 0.200000));
        var color_result = vec4<f32>(pal_rgb, clamp(dot(pal_rgb, vec3<f32>(0.299, 0.587, 0.114)) * 2.0, 0.0, 1.0));
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 2: pressure ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        { let warp_x = fbm2(p * 1.500000 + vec2<f32>(0.0, 1.3), i32(2.000000), 0.200000, 2.000000);
        let warp_y = fbm2(p * 1.500000 + vec2<f32>(1.7, 0.0), i32(2.000000), 0.200000, 2.000000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.200000; }
        var sdf_result = fbm2((p * 2.000000 + vec2<f32>(time * 0.1, time * 0.07)), i32(2.000000), 0.300000, 2.000000);
        let pal_rgb = cosine_palette(sdf_result, vec3<f32>(decision_pressure, 0.020000, 0.020000), vec3<f32>(0.300000, 0.050000, 0.050000), vec3<f32>(0.800000, 0.150000, 0.100000), vec3<f32>(0.000000, 0.000000, 0.000000));
        var color_result = vec4<f32>(pal_rgb, clamp(dot(pal_rgb, vec3<f32>(0.299, 0.587, 0.114)) * 2.0, 0.0, 1.0));
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 3: quality_glow ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        var sdf_result = sdf_circle(p, 0.500000);
        let glow_pulse = quality * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.900000, 0.850000, 0.750000), color_result.a);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    final_color = vec4<f32>(apply_color_matrix(final_color.rgb), final_color.a);
    return final_color;
}
