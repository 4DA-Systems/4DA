// GAME Component: logo-mark — hand-crafted 4DA brand mark.
// The numeral "4" rendered as a gold wireframe with gentle 3D rotation,
// depth-aware coloring, and vertex glow. Optimized for 48–128px display.
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

fn rot3_y(p: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a); let s = sin(a);
    return vec3<f32>(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}

fn rot3_x(p: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a); let s = sin(a);
    return vec3<f32>(p.x, c * p.y - s * p.z, s * p.y + c * p.z);
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
    let time = fract(u.time / 300.0) * 300.0;
    let spd = u.rotation_speed;

    // Audio reactivity
    let audio_pulse = 1.0 + u.audio_bass * 0.04 + u.audio_beat * 0.03;
    let audio_rot = 1.0 + u.audio_energy * 0.3;

    // "4" numeral vertices in 3D (z varies for depth)
    // Clean geometric "4": diagonal + crossbar + vertical
    //     A
    //    /|
    //   / |
    //  C--E--D
    //     |
    //     B
    var vtx: array<vec3<f32>, 5>;
    vtx[0] = vec3<f32>( 0.08,  0.40, 0.02);  // A: top of vertical
    vtx[1] = vec3<f32>( 0.08, -0.38, -0.02); // B: bottom of vertical
    vtx[2] = vec3<f32>(-0.30, -0.02, 0.03);  // C: left of crossbar
    vtx[3] = vec3<f32>( 0.28, -0.02, -0.01); // D: right of crossbar
    vtx[4] = vec3<f32>( 0.08, -0.02, 0.0);   // E: junction

    // Scale with audio + breathing
    let breath = 1.0 + sin(time * 0.4) * 0.02;
    let sc = 0.85 * audio_pulse * breath;
    for (var i = 0u; i < 5u; i++) {
        vtx[i] = vtx[i] * sc;
    }

    // rotation_speed = accumulated angle from JS physics (inertia + friction)
    let my = (u.mouse.y - 0.5) * -1.5;
    let drift_x = sin(time * 0.3 + 0.7) * 0.10;
    let ry = spd; // JS sets this to the physics-accumulated angle
    let rx = my + drift_x;
    for (var i = 0u; i < 5u; i++) {
        vtx[i] = rot3_y(vtx[i], ry * audio_rot);
        vtx[i] = rot3_x(vtx[i], rx * audio_rot);
    }

    // Project 3D → 2D with perspective
    var p: array<vec2<f32>, 5>;
    var zdepth: array<f32, 5>;
    for (var i = 0u; i < 5u; i++) {
        p[i] = proj3(vtx[i]);
        zdepth[i] = 0.4 + 0.6 * (vtx[i].z + 0.15) / 0.30;
    }

    // 4 edges defining the "4":
    // Edge 0: C → A (diagonal stroke)
    // Edge 1: C → D (horizontal crossbar) via C→E and E→D
    // Edge 2: A → B (vertical stroke)
    // Split crossbar through junction for cleaner intersection
    let d_ca = dist_seg(uv, p[2], p[0]); // diagonal: C→A
    let d_ce = dist_seg(uv, p[2], p[4]); // crossbar left: C→E
    let d_ed = dist_seg(uv, p[4], p[3]); // crossbar right: E→D
    let d_ab = dist_seg(uv, p[0], p[1]); // vertical: A→B

    let min_d = min(min(d_ca, d_ce), min(d_ed, d_ab));

    // Depth-weighted halo per edge
    let hk = 16.0;
    var halo_sum = exp(-d_ca * hk) * (zdepth[2] + zdepth[0]) * 0.5
                 + exp(-d_ce * hk) * (zdepth[2] + zdepth[4]) * 0.5
                 + exp(-d_ed * hk) * (zdepth[4] + zdepth[3]) * 0.5
                 + exp(-d_ab * hk) * (zdepth[0] + zdepth[1]) * 0.5;

    // Nearest vertex
    var min_vd = length(uv - p[0]);
    var min_vz = zdepth[0];
    for (var i = 1u; i < 5u; i++) {
        let vd = length(uv - p[i]);
        if (vd < min_vd) { min_vd = vd; min_vz = zdepth[i]; }
    }

    // Crisp edge core with anti-aliasing
    let edge_w = 0.030 + 0.014 * min_vz;
    let aa = fwidth(min_d);
    let core = (1.0 - smoothstep(edge_w - aa, edge_w + aa, min_d)) * 0.95 * min_vz;
    let halo = halo_sum * 0.25;

    // Vertex dots — prominent at corners, subtle at junction
    let vtx_w = 0.042;
    let vtx_aa = fwidth(min_vd);
    let vtx_g = (1.0 - smoothstep(vtx_w - vtx_aa, vtx_w + vtx_aa, min_vd)) * min_vz
              + exp(-min_vd * 28.0) * 0.55 * min_vz;

    // Junction vertex (E) gets a special glow — the heart of the "4"
    let junc_dist = length(uv - p[4]);
    let junc_glow = exp(-junc_dist * 22.0) * 0.3;

    let total = (core + halo + vtx_g + junc_glow) * u.glow_intensity;

    // Depth-aware gold coloring
    let bright_gold = vec3<f32>(0.92, 0.78, 0.35);
    let deep_gold = vec3<f32>(0.65, 0.50, 0.15);
    let base_color = mix(deep_gold, bright_gold, min_vz);

    // Junction gets warmer tint
    let junc_influence = exp(-junc_dist * 10.0);
    let junc_tint = vec3<f32>(1.0, 0.90, 0.65);
    let final_color = mix(base_color, junc_tint, junc_influence * 0.3);

    let hot = vec3<f32>(1.0, 0.97, 0.90);
    let color = clamp(final_color * total + hot * max(total - 0.5, 0.0) * 0.6, vec3<f32>(0.0), vec3<f32>(1.0));
    let alpha = clamp(total * 1.6, 0.0, 1.0);
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

in vec2 v_uv;
out vec4 fragColor;

vec3 rot3_y(vec3 p, float a){
    float c = cos(a), s = sin(a);
    return vec3(c*p.x + s*p.z, p.y, -s*p.x + c*p.z);
}

vec3 rot3_x(vec3 p, float a){
    float c = cos(a), s = sin(a);
    return vec3(p.x, c*p.y - s*p.z, s*p.y + c*p.z);
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
    float time = fract(u_time / 300.0) * 300.0;
    float spd = u_p_rotation_speed;

    float audio_pulse = 1.0 + u_audio_bass * 0.04 + u_audio_beat * 0.03;
    float audio_rot = 1.0 + u_audio_energy * 0.3;

    vec3 vtx[5];
    vtx[0] = vec3( 0.08,  0.40, 0.02);
    vtx[1] = vec3( 0.08, -0.38, -0.02);
    vtx[2] = vec3(-0.30, -0.02, 0.03);
    vtx[3] = vec3( 0.28, -0.02, -0.01);
    vtx[4] = vec3( 0.08, -0.02, 0.0);

    float breath = 1.0 + sin(time * 0.4) * 0.02;
    float sc = 0.85 * audio_pulse * breath;
    for (int i = 0; i < 5; i++) vtx[i] *= sc;

    float my = (u_mouse.y - 0.5) * -1.5;
    float drift_x = sin(time * 0.3 + 0.7) * 0.10;
    float ry = spd;
    float rx = my + drift_x;
    for (int i = 0; i < 5; i++){
        vtx[i] = rot3_y(vtx[i], ry * audio_rot);
        vtx[i] = rot3_x(vtx[i], rx * audio_rot);
    }

    vec2 p[5]; float zdepth[5];
    for (int i = 0; i < 5; i++){
        p[i] = proj3(vtx[i]);
        zdepth[i] = 0.4 + 0.6 * (vtx[i].z + 0.15) / 0.30;
    }

    float d_ca = dist_seg(uv, p[2], p[0]);
    float d_ce = dist_seg(uv, p[2], p[4]);
    float d_ed = dist_seg(uv, p[4], p[3]);
    float d_ab = dist_seg(uv, p[0], p[1]);
    float min_d = min(min(d_ca, d_ce), min(d_ed, d_ab));

    float hk = 16.0;
    float halo_sum = exp(-d_ca*hk) * (zdepth[2]+zdepth[0])*0.5
                   + exp(-d_ce*hk) * (zdepth[2]+zdepth[4])*0.5
                   + exp(-d_ed*hk) * (zdepth[4]+zdepth[3])*0.5
                   + exp(-d_ab*hk) * (zdepth[0]+zdepth[1])*0.5;

    float min_vd = length(uv - p[0]);
    float min_vz = zdepth[0];
    for (int i = 1; i < 5; i++){
        float vd = length(uv - p[i]);
        if (vd < min_vd) { min_vd = vd; min_vz = zdepth[i]; }
    }

    float edge_w = 0.030 + 0.014 * min_vz;
    float aa = fwidth(min_d);
    float core = (1.0 - smoothstep(edge_w - aa, edge_w + aa, min_d)) * 0.95 * min_vz;
    float halo = halo_sum * 0.25;
    float vtx_w = 0.042;
    float vtx_aa = fwidth(min_vd);
    float vtx_g = (1.0 - smoothstep(vtx_w - vtx_aa, vtx_w + vtx_aa, min_vd)) * min_vz
                + exp(-min_vd * 28.0) * 0.55 * min_vz;

    float junc_dist = length(uv - p[4]);
    float junc_glow = exp(-junc_dist * 22.0) * 0.3;

    float total = (core + halo + vtx_g + junc_glow) * u_p_glow_intensity;

    vec3 bright_gold = vec3(0.92, 0.78, 0.35);
    vec3 deep_gold = vec3(0.65, 0.50, 0.15);
    vec3 base_color = mix(deep_gold, bright_gold, min_vz);

    float junc_influence = exp(-junc_dist * 10.0);
    vec3 junc_tint = vec3(1.0, 0.90, 0.65);
    vec3 final_color = mix(base_color, junc_tint, junc_influence * 0.3);

    vec3 hot = vec3(1.0, 0.97, 0.90);
    vec3 color = clamp(final_color * total + hot * max(total - 0.5, 0.0) * 0.6, 0.0, 1.0);
    float alpha = clamp(total * 1.6, 0.0, 1.0);
    fragColor = vec4(color * alpha, alpha);
}
`;
const UNIFORMS = [
  { name: 'rotation_speed', default: 0.10 },
  { name: 'glow_intensity', default: 1.2 },
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
    try {
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

      const [vInfo, fInfo] = await Promise.all([vMod.getCompilationInfo(), fMod.getCompilationInfo()]);
      const hasError = [...vInfo.messages, ...fInfo.messages].some(m => m.type === 'error');
      if (hasError) {
        console.error('game-logo-mark: shader compilation failed');
        this.device.destroy();
        this.device = null;
        return false;
      }

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
    } catch (err) {
      console.warn('game-logo-mark: WebGPU init failed:', err);
      if (this.device) { this.device.destroy(); this.device = null; }
      return false;
    }
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
    this._smx = (this._smx ?? 0.5) + (this.mouseX - (this._smx ?? 0.5)) * 0.14;
    this._smy = (this._smy ?? 0.5) + (this.mouseY - (this._smy ?? 0.5)) * 0.14;
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
    this._smx = (this._smx ?? 0.5) + (this.mouseX - (this._smx ?? 0.5)) * 0.14;
    this._smy = (this._smy ?? 0.5) + (this.mouseY - (this._smy ?? 0.5)) * 0.14;
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


class LogoMark extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this._renderer = null;
    this._resizeObserver = null;
    // Physics state — office chair inertia
    this._angle = 0;
    this._angularVel = 0.04; // gentle initial spin
    this._prevMouseX = 0.5;
    this._friction = 0.975; // stronger damping — settles in ~2s
    this._minSpin = 0.002; // very slow ambient drift
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
        console.warn('game-logo-mark: no WebGPU or WebGL2 support');
        return;
      }
    }

    // Hook physics into render loop
    this._renderer._preRender = () => {
      // Mouse velocity → angular impulse
      const dx = this._renderer.mouseX - this._prevMouseX;
      this._prevMouseX = this._renderer.mouseX;
      if (Math.abs(dx) > 0.001) {
        this._angularVel += dx * 2.0;
      }

      // Friction decay
      this._angularVel *= this._friction;

      // When nearly stopped, snap back to nearest "home" angle (0°)
      // Uses a spring force toward the nearest multiple of 2π
      if (Math.abs(this._angularVel) < 0.01) {
        const nearest = Math.round(this._angle / (Math.PI * 2)) * Math.PI * 2;
        const spring = (nearest - this._angle) * 0.03; // gentle spring
        this._angularVel += spring;
      }

      // Accumulate angle and pass to shader
      this._angle += this._angularVel;
      this._renderer.setParam('rotation_speed', this._angle);
    };

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

customElements.define('game-logo-mark', LogoMark);
})();
