// GAME Component: compound-five-tetrahedra — hand-crafted, Platonic geometry series.
// 5 interlocking tetrahedra, 20 vertices, 30 edges. The compound of five tetrahedra.
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

fn rot_axis(p: vec3<f32>, axis: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a); let s = sin(a);
    let oc = 1.0 - c;
    let ax = axis;
    return vec3<f32>(
        p.x * (oc * ax.x * ax.x + c) + p.y * (oc * ax.x * ax.y - s * ax.z) + p.z * (oc * ax.x * ax.z + s * ax.y),
        p.x * (oc * ax.y * ax.x + s * ax.z) + p.y * (oc * ax.y * ax.y + c) + p.z * (oc * ax.y * ax.z - s * ax.x),
        p.x * (oc * ax.z * ax.x - s * ax.y) + p.y * (oc * ax.z * ax.y + s * ax.x) + p.z * (oc * ax.z * ax.z + c)
    );
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

    // Audio reactivity
    let audio_scale = 1.0 + u.audio_bass * 0.08 + u.audio_beat * 0.05;
    let audio_rot = 1.0 + u.audio_energy * 0.5;

    let k = 0.5774; // 1/sqrt(3)
    let sc = 0.45 * audio_scale;
    let pi = 3.14159265;

    // Rotation axis for the compound: normalized (0, 1, phi) where phi = golden ratio
    let phi = 1.6180339887;
    let axis_len = sqrt(1.0 + phi * phi);
    let rot_ax = vec3<f32>(0.0, 1.0 / axis_len, phi / axis_len);

    // Base tetrahedron vertices (inscribed in unit sphere)
    var base: array<vec3<f32>, 4>;
    base[0] = vec3<f32>(k, k, k) * sc;
    base[1] = vec3<f32>(k, -k, -k) * sc;
    base[2] = vec3<f32>(-k, k, -k) * sc;
    base[3] = vec3<f32>(-k, -k, k) * sc;

    // 5 tetrahedra: rotate base by i * 72 degrees around the icosahedral axis
    // Store all 20 vertices (5 tetrahedra x 4 vertices)
    var v: array<vec3<f32>, 20>;
    for (var t_idx = 0u; t_idx < 5u; t_idx++) {
        let angle = f32(t_idx) * 2.0 * pi / 5.0;
        for (var vi = 0u; vi < 4u; vi++) {
            v[t_idx * 4u + vi] = rot_axis(base[vi], rot_ax, angle);
        }
    }

    // Common tumble rotation (time + mouse)
    let mx = (u.mouse.x - 0.5) * 0.5;
    let my = (u.mouse.y - 0.5) * 0.5;
    let aspd = spd * audio_rot;
    for (var i = 0u; i < 20u; i++) {
        v[i] = rot_y(rot_x(v[i], time * aspd * 0.7 + my), time * aspd + mx);
    }

    // Project all 20 vertices to 2D
    var p: array<vec2<f32>, 20>;
    for (var i = 0u; i < 20u; i++) {
        p[i] = proj3(v[i]);
    }

    // Depth factors for all 20 vertices
    var df: array<f32, 20>;
    for (var i = 0u; i < 20u; i++) {
        df[i] = 0.3 + 0.7 * (v[i].z + sc) / (2.0 * sc);
    }

    // 5 tetrahedron tint colors (gold palette)
    var colors: array<vec3<f32>, 5>;
    colors[0] = vec3<f32>(0.831, 0.686, 0.216); // pure gold
    colors[1] = vec3<f32>(0.85, 0.72, 0.28);    // warm gold
    colors[2] = vec3<f32>(0.75, 0.55, 0.18);    // amber
    colors[3] = vec3<f32>(0.72, 0.45, 0.14);    // copper
    colors[4] = vec3<f32>(0.65, 0.50, 0.22);    // bronze

    let hot = vec3<f32>(1.0, 0.95, 0.85);
    var color_acc = vec3<f32>(0.0, 0.0, 0.0);

    // Process each tetrahedron
    for (var t_idx = 0u; t_idx < 5u; t_idx++) {
        let b = t_idx * 4u;
        // 6 edges per tetrahedron: (0,1), (0,2), (0,3), (1,2), (1,3), (2,3)
        let d01 = dist_seg(uv, p[b + 0u], p[b + 1u]);
        let d02 = dist_seg(uv, p[b + 0u], p[b + 2u]);
        let d03 = dist_seg(uv, p[b + 0u], p[b + 3u]);
        let d12 = dist_seg(uv, p[b + 1u], p[b + 2u]);
        let d13 = dist_seg(uv, p[b + 1u], p[b + 3u]);
        let d23 = dist_seg(uv, p[b + 2u], p[b + 3u]);
        let min_d = min(min(min(d01, d02), min(d03, d12)), min(d13, d23));

        // Depth-weighted halo
        var halo_sum = exp(-d01 * 22.0) * (df[b + 0u] + df[b + 1u]) * 0.5
                     + exp(-d02 * 22.0) * (df[b + 0u] + df[b + 2u]) * 0.5
                     + exp(-d03 * 22.0) * (df[b + 0u] + df[b + 3u]) * 0.5
                     + exp(-d12 * 22.0) * (df[b + 1u] + df[b + 2u]) * 0.5
                     + exp(-d13 * 22.0) * (df[b + 1u] + df[b + 3u]) * 0.5
                     + exp(-d23 * 22.0) * (df[b + 2u] + df[b + 3u]) * 0.5;

        // Nearest vertex distance + depth for this tetrahedron
        var min_vd = length(uv - p[b]);
        var min_vdf = df[b];
        for (var vi = 1u; vi < 4u; vi++) {
            let vd = length(uv - p[b + vi]);
            if (vd < min_vd) { min_vd = vd; min_vdf = df[b + vi]; }
        }

        // Anti-aliased edge core
        let edge_w = 0.010 + 0.005 * min_vdf;
        let aa = fwidth(min_d);
        let core = (1.0 - smoothstep(edge_w - aa, edge_w + aa, min_d)) * 0.85 * min_vdf;
        let halo = halo_sum * 0.18;
        // Vertex dots
        let vtx_w = 0.020;
        let vtx_aa = fwidth(min_vd);
        let vtx = (1.0 - smoothstep(vtx_w - vtx_aa, vtx_w + vtx_aa, min_vd)) * min_vdf
                + exp(-min_vd * 45.0) * 0.5 * min_vdf;
        let total = (core + halo + vtx) * u.glow_intensity;

        // Tint by this tetrahedron's color + white-hot bloom
        let tint = colors[t_idx];
        color_acc += tint * total + hot * max(total - 0.5, 0.0) * 0.5;
    }

    return vec4<f32>(clamp(color_acc, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
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

in vec2 v_uv;
out vec4 fragColor;

vec3 rot_x(vec3 p, float a){
    float c = cos(a), s = sin(a);
    return vec3(p.x, c*p.y - s*p.z, s*p.y + c*p.z);
}

vec3 rot_y(vec3 p, float a){
    float c = cos(a), s = sin(a);
    return vec3(c*p.x + s*p.z, p.y, -s*p.x + c*p.z);
}

vec3 rot_z(vec3 p, float a){
    float c = cos(a), s = sin(a);
    return vec3(c*p.x - s*p.y, s*p.x + c*p.y, p.z);
}

vec3 rot_axis(vec3 p, vec3 axis, float a){
    float c = cos(a), s = sin(a), oc = 1.0 - c;
    vec3 ax = axis;
    return vec3(
        p.x*(oc*ax.x*ax.x+c)       + p.y*(oc*ax.x*ax.y-s*ax.z) + p.z*(oc*ax.x*ax.z+s*ax.y),
        p.x*(oc*ax.y*ax.x+s*ax.z)  + p.y*(oc*ax.y*ax.y+c)       + p.z*(oc*ax.y*ax.z-s*ax.x),
        p.x*(oc*ax.z*ax.x-s*ax.y)  + p.y*(oc*ax.z*ax.y+s*ax.x) + p.z*(oc*ax.z*ax.z+c)
    );
}

vec2 proj3(vec3 p){
    float d = 3.5;
    float s = d / (d - p.z);
    return p.xy * s;
}

float dist_seg(vec2 p, vec2 a, vec2 b){
    vec2 pa = p - a, ba = b - a;
    float l2 = dot(ba, ba);
    if (l2 < 0.0001) return length(pa);
    float t = clamp(dot(pa, ba) / l2, 0.0, 1.0);
    return length(pa - ba * t);
}

void main(){
    float aspect = u_resolution.x / u_resolution.y;
    vec2 uv = (v_uv * 2.0 - 1.0) * vec2(aspect, 1.0);
    float time = fract(u_time / 120.0) * 120.0;
    float spd = u_p_rotation_speed;

    float audio_scale = 1.0 + u_audio_bass * 0.08 + u_audio_beat * 0.05;
    float audio_rot = 1.0 + u_audio_energy * 0.5;

    float k = 0.5774;
    float sc = 0.45 * audio_scale;
    float pi = 3.14159265;

    // Rotation axis: normalized (0, 1, phi)
    float phi = 1.6180339887;
    float axis_len = sqrt(1.0 + phi * phi);
    vec3 rot_ax = vec3(0.0, 1.0 / axis_len, phi / axis_len);

    // Base tetrahedron
    vec3 base_v[4];
    base_v[0] = vec3(k, k, k) * sc;
    base_v[1] = vec3(k, -k, -k) * sc;
    base_v[2] = vec3(-k, k, -k) * sc;
    base_v[3] = vec3(-k, -k, k) * sc;

    // 5 tetrahedra x 4 vertices = 20 vertices
    vec3 v[20];
    for (int ti = 0; ti < 5; ti++){
        float angle = float(ti) * 2.0 * pi / 5.0;
        for (int vi = 0; vi < 4; vi++){
            v[ti * 4 + vi] = rot_axis(base_v[vi], rot_ax, angle);
        }
    }

    // Common tumble
    float mx = (u_mouse.x - 0.5) * 0.5;
    float my = (u_mouse.y - 0.5) * 0.5;
    float aspd = spd * audio_rot;
    for (int i = 0; i < 20; i++){
        v[i] = rot_y(rot_x(v[i], time * aspd * 0.7 + my), time * aspd + mx);
    }

    // Project to 2D
    vec2 p[20];
    for (int i = 0; i < 20; i++){
        p[i] = proj3(v[i]);
    }

    // Depth factors
    float df[20];
    for (int i = 0; i < 20; i++){
        df[i] = 0.3 + 0.7 * (v[i].z + sc) / (2.0 * sc);
    }

    // 5 tint colors
    vec3 colors[5];
    colors[0] = vec3(0.831, 0.686, 0.216);
    colors[1] = vec3(0.85, 0.72, 0.28);
    colors[2] = vec3(0.75, 0.55, 0.18);
    colors[3] = vec3(0.72, 0.45, 0.14);
    colors[4] = vec3(0.65, 0.50, 0.22);

    vec3 hot = vec3(1.0, 0.95, 0.85);
    vec3 color_acc = vec3(0.0);

    for (int ti = 0; ti < 5; ti++){
        int b = ti * 4;
        float d01 = dist_seg(uv, p[b+0], p[b+1]);
        float d02 = dist_seg(uv, p[b+0], p[b+2]);
        float d03 = dist_seg(uv, p[b+0], p[b+3]);
        float d12 = dist_seg(uv, p[b+1], p[b+2]);
        float d13 = dist_seg(uv, p[b+1], p[b+3]);
        float d23 = dist_seg(uv, p[b+2], p[b+3]);
        float min_d = min(min(min(d01, d02), min(d03, d12)), min(d13, d23));

        float halo_sum = exp(-d01 * 22.0) * (df[b+0] + df[b+1]) * 0.5
                       + exp(-d02 * 22.0) * (df[b+0] + df[b+2]) * 0.5
                       + exp(-d03 * 22.0) * (df[b+0] + df[b+3]) * 0.5
                       + exp(-d12 * 22.0) * (df[b+1] + df[b+2]) * 0.5
                       + exp(-d13 * 22.0) * (df[b+1] + df[b+3]) * 0.5
                       + exp(-d23 * 22.0) * (df[b+2] + df[b+3]) * 0.5;

        float min_vd = length(uv - p[b]);
        float min_vdf = df[b];
        for (int vi = 1; vi < 4; vi++){
            float vd = length(uv - p[b + vi]);
            if (vd < min_vd) { min_vd = vd; min_vdf = df[b + vi]; }
        }

        float edge_w = 0.010 + 0.005 * min_vdf;
        float aa = fwidth(min_d);
        float core = (1.0 - smoothstep(edge_w - aa, edge_w + aa, min_d)) * 0.85 * min_vdf;
        float halo = halo_sum * 0.18;
        float vtx_w = 0.020;
        float vtx_aa = fwidth(min_vd);
        float vtx_g = (1.0 - smoothstep(vtx_w - vtx_aa, vtx_w + vtx_aa, min_vd)) * min_vdf
                    + exp(-min_vd * 45.0) * 0.5 * min_vdf;
        float total = (core + halo + vtx_g) * u_p_glow_intensity;

        vec3 tint = colors[ti];
        color_acc += tint * total + hot * max(total - 0.5, 0.0) * 0.5;
    }

    fragColor = vec4(clamp(color_acc, 0.0, 1.0), 1.0);
}
`;
const UNIFORMS = [
  { name: 'rotation_speed', default: 0.3 },
  { name: 'glow_intensity', default: 1.0 },
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


class CompoundFiveTetrahedra extends HTMLElement {
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
        console.warn('game-compound-five-tetrahedra: no WebGPU or WebGL2 support');
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

customElements.define('game-compound-five-tetrahedra', CompoundFiveTetrahedra);
})();
