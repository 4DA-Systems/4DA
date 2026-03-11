struct Uniforms {
    time: f32,
    audio_bass: f32,
    audio_mid: f32,
    audio_treble: f32,
    audio_energy: f32,
    audio_beat: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    p_power: f32,
    p_breath: f32,
    p_sanctity: f32,
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

fn sdf_star(p: vec2<f32>, n: f32, r: f32, ir: f32) -> f32 {
    let an = 3.14159265 / n;
    let a = atan2(p.y, p.x);
    let period = 2.0 * an;
    let sa = (a + an) - floor((a + an) / period) * period - an;
    let q = length(p) * vec2<f32>(cos(sa), abs(sin(sa)));
    let tip = vec2<f32>(r, 0.0);
    let valley = vec2<f32>(ir * cos(an), ir * sin(an));
    let e = tip - valley;
    let d = q - valley;
    let t = clamp(dot(d, e) / dot(e, e), 0.0, 1.0);
    let closest = valley + e * t;
    let dist = length(q - closest);
    let cross_val = d.x * e.y - d.y * e.x;
    return select(dist, -dist, cross_val > 0.0);
}

fn sdf_box(p: vec2<f32>, w: f32, h: f32) -> f32 {
    let d = abs(p) - vec2<f32>(w, h);
    return length(max(d, vec2<f32>(0.0, 0.0))) + min(max(d.x, d.y), 0.0);
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

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv * 2.0 - 1.0;
    let aspect = u.resolution.x / u.resolution.y;
    let time = fract(u.time / 120.0) * 120.0;

    let power = u.p_power;
    let breath = u.p_breath;
    let sanctity = u.p_sanctity;

    var final_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // ── Layer 1: field ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        { let warp_x = fbm2(p * 1.500000 + vec2<f32>(0.0, 1.3), i32(5.000000), 0.500000, 0.300000);
        let warp_y = fbm2(p * 1.500000 + vec2<f32>(1.7, 0.0), i32(5.000000), 0.500000, 0.300000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.300000; }
        var sdf_result = noise2(p * 3.000000 + vec2<f32>(time * 0.1, time * 0.07));
        let pal_rgb = cosine_palette(sdf_result, vec3<f32>(0.200000, 0.100000, 0.300000), vec3<f32>(0.200000, 0.100000, 0.300000), vec3<f32>(0.800000, 0.400000, 1.000000), vec3<f32>(0.000000, 0.100000, 0.200000));
        var color_result = vec4<f32>(pal_rgb, clamp(dot(pal_rgb, vec3<f32>(0.299, 0.587, 0.114)) * 2.0, 0.0, 1.0));
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.920000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 2: cross_core ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        let sdf_a = length(p) - 0.2; // fallback
        let sdf_b = sdf_star(p, 4.000000, 0.200000, 0.080000);
        var sdf_result = smin(sdf_a, sdf_b, 0.050000);
        let glow_pulse = 3.500000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(1.000000, 0.300000, 0.300000), color_result.a);
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.880000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 3: inner_halo ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        let sdf_a = abs(length(p) - 0.220000) - 0.025000;
        let sdf_b = length(p) - 0.2; // fallback
        var sdf_result = max(sdf_a, -sdf_b);
        let glow_pulse = 2.500000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(1.000000, 0.500000, 0.200000), color_result.a);
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.900000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 4: rings ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        var sdf_result = sdf_circle(p, 0.350000);
        for (var onion_i: i32 = 0; onion_i < i32(4.000000); onion_i = onion_i + 1) { sdf_result = abs(sdf_result) - 0.008000; }
        let glow_pulse = 2.000000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.800000, 0.200000, 0.400000), color_result.a);
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.850000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 5: rays ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        p = vec2<f32>(length(p), atan2(p.y, p.x));
        { let warp_x = fbm2(p * 2.000000 + vec2<f32>(0.0, 1.3), i32(3.000000), 0.150000, 2.000000);
        let warp_y = fbm2(p * 2.000000 + vec2<f32>(1.7, 0.0), i32(3.000000), 0.150000, 2.000000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.150000; }
        var sdf_result = noise2(p * 12.000000 + vec2<f32>(time * 0.1, time * 0.07));
        let glow_pulse = 1.500000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(1.000000, 0.600000, 0.300000), color_result.a);
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.860000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 6: dust ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        p = p + vec2<f32>(sin(p.y * 6.000000 + time * 0.500000), cos(p.x * 6.000000 + time * 0.500000)) * 0.200000;
        { let warp_x = fbm2(p * 4.000000 + vec2<f32>(0.0, 1.3), i32(3.000000), 0.300000, 2.000000);
        let warp_y = fbm2(p * 4.000000 + vec2<f32>(1.7, 0.0), i32(3.000000), 0.300000, 2.000000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.300000; }
        var sdf_result = fbm2((p * 14.000000 + vec2<f32>(time * 0.1, time * 0.07)), i32(2.000000), 0.500000, 2.000000);
        let glow_pulse = 3.000000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(1.000000, 0.800000, 0.500000), color_result.a);
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.750000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    return final_color;
}
