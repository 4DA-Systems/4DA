// GAME Component: pentachoron — hand-crafted, Platonic geometry series.
// 5 vertices in 4D, 10 edges. The 4D simplex. 4DA's true form.
// 4D rotation uses golden-ratio-derived speeds for quasi-periodic motion.
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
    w_rotation: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// 4D rotation in the XW plane
fn rot_xw(p: vec4<f32>, a: f32) -> vec4<f32> {
    let c = cos(a); let s = sin(a);
    return vec4<f32>(c * p.x + s * p.w, p.y, p.z, -s * p.x + c * p.w);
}

// 4D rotation in the ZW plane
fn rot_zw(p: vec4<f32>, a: f32) -> vec4<f32> {
    let c = cos(a); let s = sin(a);
    return vec4<f32>(p.x, p.y, c * p.z + s * p.w, -s * p.z + c * p.w);
}

// 4D rotation in the YZ plane
fn rot_yz(p: vec4<f32>, a: f32) -> vec4<f32> {
    let c = cos(a); let s = sin(a);
    return vec4<f32>(p.x, c * p.y - s * p.z, s * p.y + c * p.z, p.w);
}

// 4D -> 3D perspective projection with breathing
fn proj4(p: vec4<f32>, breath: f32) -> vec3<f32> {
    let d = 2.5 + breath;
    let s = d / (d - p.w);
    return p.xyz * s;
}

// 3D -> 2D perspective projection
fn proj3(p: vec3<f32>) -> vec2<f32> {
    let d = 4.0;
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
    let wspd = u.w_rotation;

    // Audio reactivity
    let audio_scale = 1.0 + u.audio_bass * 0.06 + u.audio_beat * 0.04;
    let audio_rot = 1.0 + u.audio_energy * 0.4;

    // Regular pentachoron vertices on 4D unit sphere
    // v0-v3: regular tetrahedron with w = -1/4
    // v4: apex at w = 1
    let q = 0.559; // sqrt(5)/4 — normalized coordinate
    let sc = 0.5 * audio_scale;  // visual scale + audio pulse
    var v: array<vec4<f32>, 5>;
    v[0] = vec4<f32>( q,  q,  q, -0.25) * sc;
    v[1] = vec4<f32>( q, -q, -q, -0.25) * sc;
    v[2] = vec4<f32>(-q,  q, -q, -0.25) * sc;
    v[3] = vec4<f32>(-q, -q,  q, -0.25) * sc;
    v[4] = vec4<f32>(0.0, 0.0, 0.0, 1.0) * sc;

    // 4D rotation — golden ratio speeds + mouse + audio boost
    let mx = (u.mouse.x - 0.5) * 0.4;
    let my = (u.mouse.y - 0.5) * 0.4;
    let awspd = wspd * audio_rot;
    let aspd = spd * audio_rot;
    for (var i = 0u; i < 5u; i++) {
        v[i] = rot_xw(v[i], time * awspd + mx);
        v[i] = rot_zw(v[i], time * awspd * 0.618 + my);
        v[i] = rot_yz(v[i], time * aspd * 0.382);
    }

    // Breathing projection — subtle 4D depth variation (reduced for professional feel)
    let breath = sin(time * 0.4) * 0.04;

    // Double projection: 4D -> 3D -> 2D
    var p: array<vec2<f32>, 5>;
    var wdepth: array<f32, 5>; // w-depth for brightness cues
    for (var i = 0u; i < 5u; i++) {
        let p3 = proj4(v[i], breath);
        p[i] = proj3(p3);
        wdepth[i] = 0.35 + 0.65 * (v[i].w + sc) / (2.0 * sc);
    }

    // 10 edges (complete graph K5) — all pairs of 5 vertices
    let d01 = dist_seg(uv, p[0], p[1]);
    let d02 = dist_seg(uv, p[0], p[2]);
    let d03 = dist_seg(uv, p[0], p[3]);
    let d04 = dist_seg(uv, p[0], p[4]);
    let d12 = dist_seg(uv, p[1], p[2]);
    let d13 = dist_seg(uv, p[1], p[3]);
    let d14 = dist_seg(uv, p[1], p[4]);
    let d23 = dist_seg(uv, p[2], p[3]);
    let d24 = dist_seg(uv, p[2], p[4]);
    let d34 = dist_seg(uv, p[3], p[4]);
    var min_d = min(min(min(d01, d02), min(d03, d04)), min(min(d12, d13), min(d14, d23)));
    min_d = min(min_d, min(d24, d34));

    // Depth-weighted halo — tightened falloff for crisp technical appearance
    let hk = 28.0;
    var halo_sum = exp(-d01 * hk) * (wdepth[0] + wdepth[1]) * 0.5
                 + exp(-d02 * hk) * (wdepth[0] + wdepth[2]) * 0.5
                 + exp(-d03 * hk) * (wdepth[0] + wdepth[3]) * 0.5
                 + exp(-d04 * hk) * (wdepth[0] + wdepth[4]) * 0.5
                 + exp(-d12 * hk) * (wdepth[1] + wdepth[2]) * 0.5
                 + exp(-d13 * hk) * (wdepth[1] + wdepth[3]) * 0.5
                 + exp(-d14 * hk) * (wdepth[1] + wdepth[4]) * 0.5
                 + exp(-d23 * hk) * (wdepth[2] + wdepth[3]) * 0.5
                 + exp(-d24 * hk) * (wdepth[2] + wdepth[4]) * 0.5
                 + exp(-d34 * hk) * (wdepth[3] + wdepth[4]) * 0.5;

    // Nearest vertex distance with depth
    var min_vd = length(uv - p[0]);
    var min_vw = wdepth[0];
    for (var i = 1u; i < 5u; i++) {
        let vd = length(uv - p[i]);
        if (vd < min_vd) { min_vd = vd; min_vw = wdepth[i]; }
    }

    // Anti-aliased edge core + tightened halo + precise vertex dots
    let edge_w = 0.022 + 0.012 * min_vw;
    let aa = fwidth(min_d);
    let core = (1.0 - smoothstep(edge_w - aa, edge_w + aa, min_d)) * 0.8 * min_vw;
    let halo = halo_sum * 0.12;
    let vtx_w = 0.025;
    let vtx_aa = fwidth(min_vd);
    let vtx = (1.0 - smoothstep(vtx_w - vtx_aa, vtx_w + vtx_aa, min_vd)) * min_vw
            + exp(-min_vd * 50.0) * 0.35 * min_vw;
    let total = (core + halo + vtx) * u.glow_intensity;

    // 4DA platinum-gold palette — technical precision, not mystical warmth
    let gold = vec3<f32>(0.78, 0.72, 0.45);
    let hot = vec3<f32>(1.0, 0.97, 0.9);
    let color = clamp(gold * total + hot * max(total - 0.6, 0.0) * 0.15, vec3<f32>(0.0), vec3<f32>(1.0));
    let alpha = clamp(total * 1.2, 0.0, 1.0);
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
uniform float u_p_w_rotation;

in vec2 v_uv;
out vec4 fragColor;

vec4 rot_xw(vec4 p, float a){
    float c = cos(a), s = sin(a);
    return vec4(c*p.x + s*p.w, p.y, p.z, -s*p.x + c*p.w);
}

vec4 rot_zw(vec4 p, float a){
    float c = cos(a), s = sin(a);
    return vec4(p.x, p.y, c*p.z + s*p.w, -s*p.z + c*p.w);
}

vec4 rot_yz(vec4 p, float a){
    float c = cos(a), s = sin(a);
    return vec4(p.x, c*p.y - s*p.z, s*p.y + c*p.z, p.w);
}

vec3 proj4b(vec4 p, float breath){
    float d = 2.5 + breath;
    float s = d / (d - p.w);
    return p.xyz * s;
}

vec2 proj3(vec3 p){
    float d = 4.0;
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
    float wspd = u_p_w_rotation;

    float q = 0.559;
    float sc = 0.5;
    vec4 v[5];
    v[0] = vec4( q,  q,  q, -0.25) * sc;
    v[1] = vec4( q, -q, -q, -0.25) * sc;
    v[2] = vec4(-q,  q, -q, -0.25) * sc;
    v[3] = vec4(-q, -q,  q, -0.25) * sc;
    v[4] = vec4(0.0, 0.0, 0.0, 1.0) * sc;

    float mx = (u_mouse.x - 0.5) * 0.4;
    float my = (u_mouse.y - 0.5) * 0.4;
    for (int i = 0; i < 5; i++){
        v[i] = rot_xw(v[i], time * wspd + mx);
        v[i] = rot_zw(v[i], time * wspd * 0.618 + my);
        v[i] = rot_yz(v[i], time * spd * 0.382);
    }

    float breath = sin(time * 0.4) * 0.04;

    vec2 p[5];
    float wdepth[5];
    for (int i = 0; i < 5; i++){
        vec3 p3 = proj4b(v[i], breath);
        p[i] = proj3(p3);
        wdepth[i] = 0.35 + 0.65 * (v[i].w + sc) / (2.0 * sc);
    }

    float d01 = dist_seg(uv, p[0], p[1]);
    float d02 = dist_seg(uv, p[0], p[2]);
    float d03 = dist_seg(uv, p[0], p[3]);
    float d04 = dist_seg(uv, p[0], p[4]);
    float d12 = dist_seg(uv, p[1], p[2]);
    float d13 = dist_seg(uv, p[1], p[3]);
    float d14 = dist_seg(uv, p[1], p[4]);
    float d23 = dist_seg(uv, p[2], p[3]);
    float d24 = dist_seg(uv, p[2], p[4]);
    float d34 = dist_seg(uv, p[3], p[4]);
    float min_d = min(min(min(d01, d02), min(d03, d04)), min(min(d12, d13), min(d14, d23)));
    min_d = min(min_d, min(d24, d34));

    float hk = 28.0;
    float halo_sum = exp(-d01*hk) * (wdepth[0]+wdepth[1])*0.5
                   + exp(-d02*hk) * (wdepth[0]+wdepth[2])*0.5
                   + exp(-d03*hk) * (wdepth[0]+wdepth[3])*0.5
                   + exp(-d04*hk) * (wdepth[0]+wdepth[4])*0.5
                   + exp(-d12*hk) * (wdepth[1]+wdepth[2])*0.5
                   + exp(-d13*hk) * (wdepth[1]+wdepth[3])*0.5
                   + exp(-d14*hk) * (wdepth[1]+wdepth[4])*0.5
                   + exp(-d23*hk) * (wdepth[2]+wdepth[3])*0.5
                   + exp(-d24*hk) * (wdepth[2]+wdepth[4])*0.5
                   + exp(-d34*hk) * (wdepth[3]+wdepth[4])*0.5;

    float min_vd = length(uv - p[0]);
    float min_vw = wdepth[0];
    for (int i = 1; i < 5; i++){
        float vd = length(uv - p[i]);
        if (vd < min_vd) { min_vd = vd; min_vw = wdepth[i]; }
    }

    float edge_w = 0.022 + 0.012 * min_vw;
    float aa = fwidth(min_d);
    float core = (1.0 - smoothstep(edge_w - aa, edge_w + aa, min_d)) * 0.8 * min_vw;
    float halo = halo_sum * 0.12;
    float vtx_w = 0.025;
    float vtx_aa = fwidth(min_vd);
    float vtx = (1.0 - smoothstep(vtx_w - vtx_aa, vtx_w + vtx_aa, min_vd)) * min_vw
              + exp(-min_vd * 50.0) * 0.35 * min_vw;
    float total = (core + halo + vtx) * u_p_glow_intensity;

    vec3 gold = vec3(0.78, 0.72, 0.45);
    vec3 hot = vec3(1.0, 0.97, 0.9);
    vec3 color = clamp(gold * total + hot * max(total - 0.6, 0.0) * 0.15, 0.0, 1.0);
    float alpha = clamp(total * 1.2, 0.0, 1.0);
    fragColor = vec4(color * alpha, alpha);
}
`;
const UNIFORMS = [
  { name: 'rotation_speed', default: 0.3 },
  { name: 'glow_intensity', default: 1.0 },
  { name: 'w_rotation', default: 0.2 },
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


class Pentachoron extends HTMLElement {
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
        console.warn('fourda-pentachoron: no WebGPU or WebGL2 support');
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

customElements.define('fourda-pentachoron', Pentachoron);
})();
