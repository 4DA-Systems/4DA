// GAME Component: icosahedron — hand-crafted, Platonic geometry series.
// 12 vertices, 30 edges, 20 faces. Golden ratio geometry.
// The network topology — 5-connected, diameter 3, triangulated consensus.
(function(){
const WGSL_V = `struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VertexOutput;
    out.pos = vec4<f32>(positions[vid], 0.0, 1.0);
    out.uv = positions[vid] * 0.5 + 0.5;
    return out;
}
`;
const WGSL_F = `struct Uniforms {
    time: f32,
    audio_bass: f32,
    audio_mid: f32,
    audio_treble: f32,
    audio_energy: f32,
    audio_beat: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
    rotation_speed: f32,
    glow_intensity: f32,
    pulse: f32,
    fill_opacity: f32,
    highlight_vertex: f32,
    highlight_color: f32,
    geodesic: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn rot_x(p: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a); let s = sin(a);
    return vec3<f32>(p.x, c * p.y - s * p.z, s * p.y + c * p.z);
}

fn rot_y(p: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a); let s = sin(a);
    return vec3<f32>(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}

fn rot_z(p: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a); let s = sin(a);
    return vec3<f32>(c * p.x - s * p.y, s * p.x + c * p.y, p.z);
}

fn proj3(p: vec3<f32>) -> vec2<f32> {
    let d = 3.5;
    let s = d / (d - p.z);
    return p.xy * s;
}

fn dist_seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let l2 = dot(ba, ba);
    if (l2 < 0.0001) { return length(pa); }
    let t = clamp(dot(pa, ba) / l2, 0.0, 1.0);
    return length(pa - ba * t);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = u.resolution.x / u.resolution.y;
    let uv = (input.uv * 2.0 - 1.0) * vec2<f32>(aspect, 1.0);
    let time = fract(u.time / 120.0) * 120.0;
    let spd = u.rotation_speed;

    // Audio reactivity (must be declared before use)
    let audio_scale = 1.0 + u.audio_bass * 0.05 + u.audio_beat * 0.03;
    let audio_rot = 1.0 + u.audio_energy * 0.3;

    // Golden ratio
    let phi = 1.6180339887;
    // Normalize to unit sphere: divide by sqrt(1 + phi*phi)
    let norm = 1.0 / sqrt(1.0 + phi * phi); // ~0.5257
    let sc = 0.38 * audio_scale; // visual scale + audio pulse

    // 12 icosahedral vertices: (0, +/-1, +/-phi), (+-1, +/-phi, 0), (+/-phi, 0, +/-1)
    var v: array<vec3<f32>, 12>;
    v[0]  = vec3<f32>(0.0,  1.0,  phi) * norm * sc;
    v[1]  = vec3<f32>(0.0,  1.0, -phi) * norm * sc;
    v[2]  = vec3<f32>(0.0, -1.0,  phi) * norm * sc;
    v[3]  = vec3<f32>(0.0, -1.0, -phi) * norm * sc;
    v[4]  = vec3<f32>( 1.0,  phi, 0.0) * norm * sc;
    v[5]  = vec3<f32>( 1.0, -phi, 0.0) * norm * sc;
    v[6]  = vec3<f32>(-1.0,  phi, 0.0) * norm * sc;
    v[7]  = vec3<f32>(-1.0, -phi, 0.0) * norm * sc;
    v[8]  = vec3<f32>( phi, 0.0,  1.0) * norm * sc;
    v[9]  = vec3<f32>( phi, 0.0, -1.0) * norm * sc;
    v[10] = vec3<f32>(-phi, 0.0,  1.0) * norm * sc;
    v[11] = vec3<f32>(-phi, 0.0, -1.0) * norm * sc;

    // 3D rotation — slow tumble + mouse + audio boost
    let mx = (u.mouse.x - 0.5) * 0.5;
    let my = (u.mouse.y - 0.5) * 0.5;
    let aspd = spd * audio_rot;
    for (var i = 0u; i < 12u; i++) {
        v[i] = rot_y(rot_x(rot_z(v[i], time * aspd * 0.3), time * aspd * 0.5 + my), time * aspd + mx);
    }

    // Perspective projection to 2D + depth factors
    var p: array<vec2<f32>, 12>;
    var df: array<f32, 12>;
    let r = norm * sc; // vertex radius for depth normalization
    for (var i = 0u; i < 12u; i++) {
        p[i] = proj3(v[i]);
        df[i] = 0.3 + 0.7 * (v[i].z + r) / (2.0 * r);
    }

    // 30 edges — distance to nearest edge
    var min_d = dist_seg(uv, p[0], p[2]);
    min_d = min(min_d, dist_seg(uv, p[0], p[4]));
    min_d = min(min_d, dist_seg(uv, p[0], p[6]));
    min_d = min(min_d, dist_seg(uv, p[0], p[8]));
    min_d = min(min_d, dist_seg(uv, p[0], p[10]));
    min_d = min(min_d, dist_seg(uv, p[1], p[3]));
    min_d = min(min_d, dist_seg(uv, p[1], p[4]));
    min_d = min(min_d, dist_seg(uv, p[1], p[6]));
    min_d = min(min_d, dist_seg(uv, p[1], p[9]));
    min_d = min(min_d, dist_seg(uv, p[1], p[11]));
    min_d = min(min_d, dist_seg(uv, p[2], p[5]));
    min_d = min(min_d, dist_seg(uv, p[2], p[7]));
    min_d = min(min_d, dist_seg(uv, p[2], p[8]));
    min_d = min(min_d, dist_seg(uv, p[2], p[10]));
    min_d = min(min_d, dist_seg(uv, p[3], p[5]));
    min_d = min(min_d, dist_seg(uv, p[3], p[7]));
    min_d = min(min_d, dist_seg(uv, p[3], p[9]));
    min_d = min(min_d, dist_seg(uv, p[3], p[11]));
    min_d = min(min_d, dist_seg(uv, p[4], p[6]));
    min_d = min(min_d, dist_seg(uv, p[4], p[8]));
    min_d = min(min_d, dist_seg(uv, p[4], p[9]));
    min_d = min(min_d, dist_seg(uv, p[5], p[7]));
    min_d = min(min_d, dist_seg(uv, p[5], p[8]));
    min_d = min(min_d, dist_seg(uv, p[5], p[9]));
    min_d = min(min_d, dist_seg(uv, p[6], p[10]));
    min_d = min(min_d, dist_seg(uv, p[6], p[11]));
    min_d = min(min_d, dist_seg(uv, p[7], p[10]));
    min_d = min(min_d, dist_seg(uv, p[7], p[11]));
    min_d = min(min_d, dist_seg(uv, p[8], p[9]));
    min_d = min(min_d, dist_seg(uv, p[10], p[11]));

    // Geodesic mode: add midpoint vertices on each edge (frequency 2 preview)
    var geo_glow = 0.0;
    if (u.geodesic > 0.5) {
        // Compute midpoints of all 30 edges, project, and add vertex dots
        // Edge midpoint = normalize((v[a] + v[b]) / 2) * radius, then project
        // For efficiency, compute distance to nearest midpoint only
        var mid_min = 999.0;
        // Midpoint distances unrolled inline (cannot define helper functions mid-shader)
        // Sample 12 edge midpoints (every other edge) for visual density
        let m0 = proj3((v[0] + v[2]) * 0.5);
        let m1 = proj3((v[0] + v[8]) * 0.5);
        let m2 = proj3((v[1] + v[3]) * 0.5);
        let m3 = proj3((v[1] + v[9]) * 0.5);
        let m4 = proj3((v[2] + v[8]) * 0.5);
        let m5 = proj3((v[3] + v[9]) * 0.5);
        let m6 = proj3((v[4] + v[8]) * 0.5);
        let m7 = proj3((v[5] + v[9]) * 0.5);
        let m8 = proj3((v[6] + v[10]) * 0.5);
        let m9 = proj3((v[7] + v[11]) * 0.5);
        let m10 = proj3((v[4] + v[6]) * 0.5);
        let m11 = proj3((v[8] + v[9]) * 0.5);
        mid_min = min(mid_min, length(uv - m0));
        mid_min = min(mid_min, length(uv - m1));
        mid_min = min(mid_min, length(uv - m2));
        mid_min = min(mid_min, length(uv - m3));
        mid_min = min(mid_min, length(uv - m4));
        mid_min = min(mid_min, length(uv - m5));
        mid_min = min(mid_min, length(uv - m6));
        mid_min = min(mid_min, length(uv - m7));
        mid_min = min(mid_min, length(uv - m8));
        mid_min = min(mid_min, length(uv - m9));
        mid_min = min(mid_min, length(uv - m10));
        mid_min = min(mid_min, length(uv - m11));
        let geo_aa = fwidth(mid_min);
        geo_glow = (1.0 - smoothstep(0.006 - geo_aa, 0.006 + geo_aa, mid_min)) * 0.5
                 + exp(-mid_min * 25.0) * 0.2;
    }

    // Nearest vertex distance + depth + pulse + vertex highlight
    var min_vd = 999.0;
    var min_vdf = 0.5;
    var pulse_glow = 0.0;
    var highlight_glow = vec3<f32>(0.0);
    let hv = i32(round(u.highlight_vertex)); // -1 = none, 0-11 = vertex index
    // highlight_color: 0=green(synced), 1=red(error), 0.5=blue(syncing)
    let hc_r = select(0.2, 1.0, u.highlight_color > 0.7);
    let hc_g = select(0.9, 0.3, u.highlight_color > 0.7);
    let hc_b = select(0.3, 0.3, u.highlight_color < 0.3);
    let hc = vec3<f32>(hc_r, hc_g, hc_b);
    for (var i = 0u; i < 12u; i++) {
        let vd = length(uv - p[i]);
        if (vd < min_vd) { min_vd = vd; min_vdf = df[i]; }
        // Pulse: sequentially highlight vertices
        let phase = fract(time * u.pulse * 0.1 - f32(i) / 12.0);
        let pw = exp(-phase * 8.0) * u.pulse;
        pulse_glow += exp(-vd * 50.0) * pw * df[i];
        // Vertex highlight: specific vertex glows in highlight color
        if (hv >= 0 && i32(i) == hv) {
            let hw = exp(-vd * 40.0) * (0.8 + 0.2 * sin(time * 3.0));
            highlight_glow += hc * hw;
        }
    }

    // Anti-aliased edge core + halo + vertex glow
    let edge_w = 0.012 + 0.008 * min_vdf;
    let aa = fwidth(min_d);
    let core = (1.0 - smoothstep(edge_w - aa, edge_w + aa, min_d)) * 0.75 * min_vdf;
    let halo = exp(-min_d * 10.0) * 0.25;
    let vtx_w = 0.028;
    let vtx_aa = fwidth(min_vd);
    let vtx = (1.0 - smoothstep(vtx_w - vtx_aa, vtx_w + vtx_aa, min_vd)) * min_vdf
            + exp(-min_vd * 30.0) * 0.4 * min_vdf;
    let total = (core + halo + vtx + geo_glow) * u.glow_intensity + pulse_glow;

    // Translucent face fill — faint amber glow inside the projected hull
    var face_fill = 0.0;
    if (u.fill_opacity > 0.001) {
        // Approximate: if pixel is close to multiple vertices, it's likely inside a face
        var near_count = 0.0;
        for (var fi = 0u; fi < 12u; fi++) {
            near_count += smoothstep(0.5, 0.1, length(uv - p[fi]));
        }
        // Inside when near_count is high (surrounded by projected vertices)
        face_fill = smoothstep(1.5, 3.0, near_count) * u.fill_opacity;
    }

    // Gold color with white-hot bloom
    let gold = vec3<f32>(0.831, 0.686, 0.216);
    let amber = vec3<f32>(0.6, 0.45, 0.15); // deeper amber for face fill
    let hot = vec3<f32>(1.0, 0.95, 0.85);
    let pulse_tint = vec3<f32>(0.9, 0.95, 1.0) * pulse_glow * 0.5;
    let color = clamp(gold * total + hot * max(total - 0.5, 0.0) * 0.5 + pulse_tint + amber * face_fill + highlight_glow, vec3<f32>(0.0), vec3<f32>(1.0));
    let alpha = clamp(total * 1.5 + face_fill + length(highlight_glow), 0.0, 1.0);
    return vec4<f32>(color * alpha, alpha);
}
`;
const GLSL_V = `#version 300 es
precision highp float;
out vec2 v_uv;
void main(){
    vec2 pos[3] = vec2[3](
        vec2(-1.0, -1.0),
        vec2(3.0, -1.0),
        vec2(-1.0, 3.0)
    );
    gl_Position = vec4(pos[gl_VertexID], 0.0, 1.0);
    v_uv = pos[gl_VertexID] * 0.5 + 0.5;
}
`;
const GLSL_F = `#version 300 es
precision highp float;

uniform float u_time;
uniform float u_audio_bass;
uniform float u_audio_mid;
uniform float u_audio_treble;
uniform float u_audio_energy;
uniform float u_audio_beat;
uniform vec2 u_resolution;
uniform vec2 u_mouse;
uniform float u_p_rotation_speed;
uniform float u_p_glow_intensity;
uniform float u_p_pulse;
uniform float u_p_fill_opacity;
uniform float u_p_highlight_vertex;
uniform float u_p_highlight_color;
uniform float u_p_geodesic;

in vec2 v_uv;
out vec4 fragColor;

vec3 rot_x(vec3 p, float a){ float c=cos(a),s=sin(a); return vec3(p.x,c*p.y-s*p.z,s*p.y+c*p.z); }
vec3 rot_y(vec3 p, float a){ float c=cos(a),s=sin(a); return vec3(c*p.x+s*p.z,p.y,-s*p.x+c*p.z); }
vec3 rot_z(vec3 p, float a){ float c=cos(a),s=sin(a); return vec3(c*p.x-s*p.y,s*p.x+c*p.y,p.z); }

vec2 proj3(vec3 p){ float d=3.5; float s=d/(d-p.z); return p.xy*s; }

float dist_seg(vec2 p, vec2 a, vec2 b){
    vec2 pa=p-a, ba=b-a;
    float l2=dot(ba,ba);
    if (l2 < 0.0001) return length(pa);
    float t=clamp(dot(pa,ba)/l2, 0.0, 1.0);
    return length(pa-ba*t);
}

void main(){
    float aspect = u_resolution.x / u_resolution.y;
    vec2 uv = (v_uv * 2.0 - 1.0) * vec2(aspect, 1.0);
    float time = fract(u_time / 120.0) * 120.0;
    float spd = u_p_rotation_speed;

    float phi = 1.6180339887;
    float norm = 1.0 / sqrt(1.0 + phi * phi);
    float sc = 0.38;

    vec3 v[12];
    v[0]  = vec3(0.0,  1.0,  phi) * norm * sc;
    v[1]  = vec3(0.0,  1.0, -phi) * norm * sc;
    v[2]  = vec3(0.0, -1.0,  phi) * norm * sc;
    v[3]  = vec3(0.0, -1.0, -phi) * norm * sc;
    v[4]  = vec3( 1.0,  phi, 0.0) * norm * sc;
    v[5]  = vec3( 1.0, -phi, 0.0) * norm * sc;
    v[6]  = vec3(-1.0,  phi, 0.0) * norm * sc;
    v[7]  = vec3(-1.0, -phi, 0.0) * norm * sc;
    v[8]  = vec3( phi, 0.0,  1.0) * norm * sc;
    v[9]  = vec3( phi, 0.0, -1.0) * norm * sc;
    v[10] = vec3(-phi, 0.0,  1.0) * norm * sc;
    v[11] = vec3(-phi, 0.0, -1.0) * norm * sc;

    float mx = (u_mouse.x - 0.5) * 0.5;
    float my = (u_mouse.y - 0.5) * 0.5;
    for (int i = 0; i < 12; i++){
        v[i] = rot_y(rot_x(rot_z(v[i], time*spd*0.3), time*spd*0.5 + my), time*spd + mx);
    }

    vec2 p[12];
    float df[12];
    float r = norm * sc;
    for (int i = 0; i < 12; i++){ p[i] = proj3(v[i]); df[i] = 0.3 + 0.7 * (v[i].z + r) / (2.0 * r); }

    float min_d = dist_seg(uv, p[0], p[2]);
    min_d = min(min_d, dist_seg(uv, p[0], p[4]));
    min_d = min(min_d, dist_seg(uv, p[0], p[6]));
    min_d = min(min_d, dist_seg(uv, p[0], p[8]));
    min_d = min(min_d, dist_seg(uv, p[0], p[10]));
    min_d = min(min_d, dist_seg(uv, p[1], p[3]));
    min_d = min(min_d, dist_seg(uv, p[1], p[4]));
    min_d = min(min_d, dist_seg(uv, p[1], p[6]));
    min_d = min(min_d, dist_seg(uv, p[1], p[9]));
    min_d = min(min_d, dist_seg(uv, p[1], p[11]));
    min_d = min(min_d, dist_seg(uv, p[2], p[5]));
    min_d = min(min_d, dist_seg(uv, p[2], p[7]));
    min_d = min(min_d, dist_seg(uv, p[2], p[8]));
    min_d = min(min_d, dist_seg(uv, p[2], p[10]));
    min_d = min(min_d, dist_seg(uv, p[3], p[5]));
    min_d = min(min_d, dist_seg(uv, p[3], p[7]));
    min_d = min(min_d, dist_seg(uv, p[3], p[9]));
    min_d = min(min_d, dist_seg(uv, p[3], p[11]));
    min_d = min(min_d, dist_seg(uv, p[4], p[6]));
    min_d = min(min_d, dist_seg(uv, p[4], p[8]));
    min_d = min(min_d, dist_seg(uv, p[4], p[9]));
    min_d = min(min_d, dist_seg(uv, p[5], p[7]));
    min_d = min(min_d, dist_seg(uv, p[5], p[8]));
    min_d = min(min_d, dist_seg(uv, p[5], p[9]));
    min_d = min(min_d, dist_seg(uv, p[6], p[10]));
    min_d = min(min_d, dist_seg(uv, p[6], p[11]));
    min_d = min(min_d, dist_seg(uv, p[7], p[10]));
    min_d = min(min_d, dist_seg(uv, p[7], p[11]));
    min_d = min(min_d, dist_seg(uv, p[8], p[9]));
    min_d = min(min_d, dist_seg(uv, p[10], p[11]));

    // Geodesic midpoints (GLSL version)
    float geo_glow = 0.0;
    if (u_p_geodesic > 0.5) {
        float mid_min = 999.0;
        mid_min = min(mid_min, length(uv - proj3((v[0]+v[2])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[0]+v[8])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[1]+v[3])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[1]+v[9])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[2]+v[8])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[3]+v[9])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[4]+v[8])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[5]+v[9])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[6]+v[10])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[7]+v[11])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[4]+v[6])*0.5)));
        mid_min = min(mid_min, length(uv - proj3((v[8]+v[9])*0.5)));
        float geo_aa = fwidth(mid_min);
        geo_glow = (1.0 - smoothstep(0.006 - geo_aa, 0.006 + geo_aa, mid_min)) * 0.5
                 + exp(-mid_min * 25.0) * 0.2;
    }

    float min_vd = 999.0;
    float min_vdf = 0.5;
    float pulse_glow = 0.0;
    vec3 highlight_glow = vec3(0.0);
    int hv = int(round(u_p_highlight_vertex));
    float hc_r = u_p_highlight_color > 0.7 ? 1.0 : 0.2;
    float hc_g = u_p_highlight_color > 0.7 ? 0.3 : 0.9;
    float hc_b = 0.3;
    vec3 hc = vec3(hc_r, hc_g, hc_b);
    for (int i = 0; i < 12; i++){
        float vd = length(uv - p[i]);
        if (vd < min_vd) { min_vd = vd; min_vdf = df[i]; }
        float phase = fract(time * u_p_pulse * 0.1 - float(i) / 12.0);
        float pw = exp(-phase * 8.0) * u_p_pulse;
        pulse_glow += exp(-vd * 50.0) * pw * df[i];
        if (hv >= 0 && i == hv) {
            float hw = exp(-vd * 40.0) * (0.8 + 0.2 * sin(time * 3.0));
            highlight_glow += hc * hw;
        }
    }

    float edge_w = 0.012 + 0.008 * min_vdf;
    float aa = fwidth(min_d);
    float core = (1.0 - smoothstep(edge_w - aa, edge_w + aa, min_d)) * 0.75 * min_vdf;
    float halo = exp(-min_d * 10.0) * 0.25;
    float vtx_w = 0.028;
    float vtx_aa = fwidth(min_vd);
    float vtx = (1.0 - smoothstep(vtx_w - vtx_aa, vtx_w + vtx_aa, min_vd)) * min_vdf
              + exp(-min_vd * 30.0) * 0.4 * min_vdf;
    float total = (core + halo + vtx + geo_glow) * u_p_glow_intensity + pulse_glow;

    // Face fill
    float face_fill = 0.0;
    if (u_p_fill_opacity > 0.001){
        float near_count = 0.0;
        for (int fi = 0; fi < 12; fi++){
            near_count += smoothstep(0.5, 0.1, length(uv - p[fi]));
        }
        face_fill = smoothstep(1.5, 3.0, near_count) * u_p_fill_opacity;
    }

    vec3 gold = vec3(0.831, 0.686, 0.216);
    vec3 amber = vec3(0.6, 0.45, 0.15);
    vec3 hot = vec3(1.0, 0.95, 0.85);
    vec3 pulse_tint = vec3(0.9, 0.95, 1.0) * pulse_glow * 0.5;
    vec3 color = clamp(gold * total + hot * max(total - 0.5, 0.0) * 0.5 + pulse_tint + amber * face_fill + highlight_glow, 0.0, 1.0);
    float alpha = clamp(total * 1.5 + face_fill + length(highlight_glow), 0.0, 1.0);
    fragColor = vec4(color * alpha, alpha);
}
`;
const UNIFORMS = [
  { name: 'rotation_speed', default: 0.25 },
  { name: 'glow_intensity', default: 1.0 },
  { name: 'pulse', default: 0.0 },
  { name: 'fill_opacity', default: 0.0 },
  { name: 'highlight_vertex', default: -1.0 },
  { name: 'highlight_color', default: 0.0 },
  { name: 'geodesic', default: 0.0 },
];

class GameRenderer {
  constructor(canvas, wgslVertex, wgslFragment, uniformDefs) {
    this.canvas = canvas;
    this.wgslVertex = wgslVertex;
    this.wgslFragment = wgslFragment;
    this.uniformDefs = uniformDefs;
    this.device = null;
    this.pipeline = null;
    this.uniformBuffer = null;
    this.bindGroup = null;
    this.running = false;
    this.startTime = performance.now() / 1000;
    this.audioData = { bass: 0, mid: 0, treble: 0, energy: 0, beat: 0 };
    this.mouseX = 0; this.mouseY = 0;
    this.userParams = {};
    for (const u of uniformDefs) this.userParams[u.name] = u.default;
    this._onMouseMove = (e) => {
      const r = this.canvas.getBoundingClientRect();
      this.mouseX = (e.clientX - r.left) / r.width;
      this.mouseY = 1.0 - (e.clientY - r.top) / r.height;
    };
    this.canvas.addEventListener('mousemove', this._onMouseMove);
  }

  async init() {
    if (!navigator.gpu) return false;
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) return false;
    this.device = await adapter.requestDevice();
    const ctx = this.canvas.getContext('webgpu');
    const format = navigator.gpu.getPreferredCanvasFormat();
    ctx.configure({ device: this.device, format, alphaMode: 'premultiplied' });
    this.ctx = ctx;
    this.format = format;

    const vMod = this.device.createShaderModule({ code: this.wgslVertex });
    const fMod = this.device.createShaderModule({ code: this.wgslFragment });

    const floatCount = 8 + 2 + 2 + this.uniformDefs.length;
    const bufSize = Math.ceil(floatCount * 4 / 16) * 16;
    this.uniformBuffer = this.device.createBuffer({
      size: bufSize, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST
    });
    this.floatCount = floatCount;

    const bindGroupLayout = this.device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } }]
    });
    this.bindGroup = this.device.createBindGroup({
      layout: bindGroupLayout,
      entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }]
    });

    const pipelineLayout = this.device.createPipelineLayout({ bindGroupLayouts: [bindGroupLayout] });

    this.pipeline = this.device.createRenderPipeline({
      layout: pipelineLayout,
      vertex: { module: vMod, entryPoint: 'vs_main' },
      fragment: { module: fMod, entryPoint: 'fs_main', targets: [{ format }] },
      primitive: { topology: 'triangle-list' }
    });
    return true;
  }

  start() {
    if (this.running) return;
    this.running = true;
    const loop = () => {
      if (!this.running) return;
      this.render();
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }

  stop() { this.running = false; }

  render() {
    if (this._preRender) this._preRender();
    const t = performance.now() / 1000 - this.startTime;
    const w = this.canvas.width;
    const h = this.canvas.height;
    const data = new Float32Array(this.floatCount);
    data[0] = t;
    data[1] = this.audioData.bass;
    data[2] = this.audioData.mid;
    data[3] = this.audioData.treble;
    data[4] = this.audioData.energy;
    data[5] = this.audioData.beat;
    data[6] = w; data[7] = h;
    this._smx = (this._smx ?? 0.5) + (this.mouseX - (this._smx ?? 0.5)) * 0.07;
    this._smy = (this._smy ?? 0.5) + (this.mouseY - (this._smy ?? 0.5)) * 0.07;
    data[8] = this._smx; data[9] = this._smy;
    let i = 10;
    for (const u of this.uniformDefs) data[i++] = this.userParams[u.name] ?? u.default;
    this.device.queue.writeBuffer(this.uniformBuffer, 0, data);

    const encoder = this.device.createCommandEncoder();

    const mainPass = encoder.beginRenderPass({
      colorAttachments: [{
        view: this.ctx.getCurrentTexture().createView(),
        loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 0 }
      }]
    });
    mainPass.setPipeline(this.pipeline);
    mainPass.setBindGroup(0, this.bindGroup);
    mainPass.draw(3);
    mainPass.end();
    this.device.queue.submit([encoder.finish()]);
  }

  setParam(name, value) { this.userParams[name] = value; }
  setAudioData(d) { Object.assign(this.audioData, d); }
  destroy() { this.stop(); this.canvas.removeEventListener('mousemove', this._onMouseMove); this.device?.destroy(); }
}


class GameRendererGL {
  constructor(canvas, glslVertex, glslFragment, uniformDefs) {
    this.canvas = canvas;
    this.glslVertex = glslVertex;
    this.glslFragment = glslFragment;
    this.uniformDefs = uniformDefs;
    this.gl = null;
    this.program = null;
    this.running = false;
    this.startTime = performance.now() / 1000;
    this.audioData = { bass: 0, mid: 0, treble: 0, energy: 0, beat: 0 };
    this.mouseX = 0; this.mouseY = 0;
    this.userParams = {};
    for (const u of uniformDefs) this.userParams[u.name] = u.default;
    this._onMouseMove = (e) => {
      const r = this.canvas.getBoundingClientRect();
      this.mouseX = (e.clientX - r.left) / r.width;
      this.mouseY = 1.0 - (e.clientY - r.top) / r.height;
    };
    this.canvas.addEventListener('mousemove', this._onMouseMove);
  }

  init() {
    const gl = this.canvas.getContext('webgl2');
    if (!gl) return false;
    this.gl = gl;

    const vs = this._compile(gl.VERTEX_SHADER, this.glslVertex);
    const fs = this._compile(gl.FRAGMENT_SHADER, this.glslFragment);
    if (!vs || !fs) return false;

    this.program = gl.createProgram();
    gl.attachShader(this.program, vs);
    gl.attachShader(this.program, fs);
    gl.linkProgram(this.program);
    if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) {
      console.error('GAME link error:', gl.getProgramInfoLog(this.program));
      return false;
    }
    gl.useProgram(this.program);

    this.locs = {
      time: gl.getUniformLocation(this.program, 'u_time'),
      bass: gl.getUniformLocation(this.program, 'u_audio_bass'),
      mid: gl.getUniformLocation(this.program, 'u_audio_mid'),
      treble: gl.getUniformLocation(this.program, 'u_audio_treble'),
      energy: gl.getUniformLocation(this.program, 'u_audio_energy'),
      beat: gl.getUniformLocation(this.program, 'u_audio_beat'),
      resolution: gl.getUniformLocation(this.program, 'u_resolution'),
      mouse: gl.getUniformLocation(this.program, 'u_mouse'),
    };
    this.paramLocs = {};
    for (const u of this.uniformDefs) {
      this.paramLocs[u.name] = gl.getUniformLocation(this.program, 'u_p_' + u.name);
    }
    return true;
  }

  _compile(type, src) {
    const gl = this.gl;
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
      console.error('GAME shader error:', gl.getShaderInfoLog(s));
      return null;
    }
    return s;
  }

  start() {
    if (this.running) return;
    this.running = true;
    const loop = () => {
      if (!this.running) return;
      this.render();
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }

  stop() { this.running = false; }

  render() {
    const gl = this.gl;
    const t = performance.now() / 1000 - this.startTime;
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(this.program);

    gl.uniform1f(this.locs.time, t);
    gl.uniform1f(this.locs.bass, this.audioData.bass);
    gl.uniform1f(this.locs.mid, this.audioData.mid);
    gl.uniform1f(this.locs.treble, this.audioData.treble);
    gl.uniform1f(this.locs.energy, this.audioData.energy);
    gl.uniform1f(this.locs.beat, this.audioData.beat);
    gl.uniform2f(this.locs.resolution, this.canvas.width, this.canvas.height);
    this._smx = (this._smx ?? 0.5) + (this.mouseX - (this._smx ?? 0.5)) * 0.07;
    this._smy = (this._smy ?? 0.5) + (this.mouseY - (this._smy ?? 0.5)) * 0.07;
    gl.uniform2f(this.locs.mouse, this._smx, this._smy);
    for (const u of this.uniformDefs) {
      gl.uniform1f(this.paramLocs[u.name], this.userParams[u.name] ?? u.default);
    }
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  setParam(name, value) { this.userParams[name] = value; }
  setAudioData(d) { Object.assign(this.audioData, d); }
  destroy() { this.stop(); this.canvas.removeEventListener('mousemove', this._onMouseMove); }
}


class Icosahedron extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this._renderer = null;
    this._resizeObserver = null;
  }

  connectedCallback() {
    const style = document.createElement('style');
    style.textContent = ':host{display:block;width:100%;height:100%}canvas{width:100%;height:100%;display:block}';
    const canvas = document.createElement('canvas');
    this.shadowRoot.appendChild(style);
    this.shadowRoot.appendChild(canvas);
    this._canvas = canvas;
    this._initRenderer();
    this._resizeObserver = new ResizeObserver(() => this._resize());
    this._resizeObserver.observe(this);
  }

  disconnectedCallback() {
    this._renderer?.destroy();
    this._renderer = null;
    this._resizeObserver?.disconnect();
  }

  async _initRenderer() {
    const gpu = new GameRenderer(this._canvas, WGSL_V, WGSL_F, UNIFORMS);
    if (await gpu.init()) {
      this._renderer = gpu;
    } else {
      const gl = new GameRendererGL(this._canvas, GLSL_V, GLSL_F, UNIFORMS);
      if (gl.init()) {
        this._renderer = gl;
      } else {
        console.warn('fourda-icosahedron: no WebGPU or WebGL2 support');
        return;
      }
    }
    this._resize();
    this._renderer.start();
  }

  _resize() {
    const rect = this.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    this._canvas.width = Math.round(rect.width * dpr);
    this._canvas.height = Math.round(rect.height * dpr);
  }

  setParam(name, value) { this._renderer?.setParam(name, value); }
  setAudioData(data) { this._renderer?.setAudioData(data); }
  setAudioSource(bridge) { bridge?.subscribe(d => this._renderer?.setAudioData(d)); }

  static get observedAttributes() { return UNIFORMS.map(u => u.name); }
  attributeChangedCallback(name, _, val) {
    if (val !== null) this.setParam(name, parseFloat(val));
  }
}

customElements.define('fourda-icosahedron', Icosahedron);
})();
