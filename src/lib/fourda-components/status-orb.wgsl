struct Uniforms {
    time: f32,
    audio_bass: f32,
    audio_mid: f32,
    audio_treble: f32,
    audio_energy: f32,
    audio_beat: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    p_intensity: f32,
    p_green: f32,
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

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv * 2.0 - 1.0;
    let aspect = u.resolution.x / u.resolution.y;
    let time = fract(u.time / 120.0) * 120.0;

    let intensity = u.p_intensity;
    let green = u.p_green;

    // ── Layer 1: orb ──
    var p = vec2<f32>(uv.x * aspect, uv.y);
    var color_result: vec4<f32>;
    {
        var p_then = p;
        var then_color: vec4<f32>;
        var else_color: vec4<f32>;
        { var p = p_then;
        { let warp_x = fbm2(p * 3.000000 + vec2<f32>(0.0, 1.3), i32(2.000000), 0.080000, 2.000000);
        let warp_y = fbm2(p * 3.000000 + vec2<f32>(1.7, 0.0), i32(2.000000), 0.080000, 2.000000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.080000; }
        var sdf_result = sdf_circle(p, 0.220000);
        let glow_pulse = 3.000000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.150000, 0.850000, 0.300000), color_result.a);
        then_color = color_result; }
        { var p = p_then;
        { let warp_x = fbm2(p * 3.000000 + vec2<f32>(0.0, 1.3), i32(2.000000), 0.080000, 2.000000);
        let warp_y = fbm2(p * 3.000000 + vec2<f32>(1.7, 0.0), i32(2.000000), 0.080000, 2.000000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.080000; }
        var sdf_result = sdf_circle(p, 0.220000);
        let glow_pulse = 3.000000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.830000, 0.500000, 0.100000), color_result.a);
        else_color = color_result; }
        color_result = select(else_color, then_color, (green > 0.500000));
    }
    let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
    color_result = mix(color_result, prev_color, 0.850000);
    return color_result;
}
