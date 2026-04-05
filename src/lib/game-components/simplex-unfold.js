// GAME Component: simplex-unfold — hand-crafted, Platonic geometry series.
// Animated dimensional progression: point -> line -> triangle -> tetrahedron -> pentachoron.
// Each dimension emerges from the last. 4D rotation appears only at the final phase.
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
    phase_offset: f32,
    auto_speed: f32,
    glow_intensity: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn dist_seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let l2 = dot(ba, ba);
    if (l2 < 0.0001) { return length(pa); }
    let t = clamp(dot(pa, ba) / l2, 0.0, 1.0);
    return length(pa - ba * t);
}

// Target vertex positions for each dimensional phase (5 vertices x 5 phases)
fn target(v: u32, ph: u32) -> vec2<f32> {
    // Phase 0: point — all at origin
    if (ph == 0u) { return vec2<f32>(0.0, 0.0); }

    // Phase 1: line segment
    if (ph == 1u) {
        if (v == 0u) { return vec2<f32>(-0.28, 0.0); }
        if (v == 1u) { return vec2<f32>( 0.28, 0.0); }
        return vec2<f32>(0.0, 0.0);
    }

    // Phase 2: equilateral triangle
    if (ph == 2u) {
        if (v == 0u) { return vec2<f32>( 0.0,   0.32); }
        if (v == 1u) { return vec2<f32>(-0.277, -0.16); }
        if (v == 2u) { return vec2<f32>( 0.277, -0.16); }
        return vec2<f32>(0.0, 0.0);
    }

    // Phase 3: tetrahedron (front projection)
    if (ph == 3u) {
        if (v == 0u) { return vec2<f32>( 0.0,   0.38); }
        if (v == 1u) { return vec2<f32>(-0.33, -0.19); }
        if (v == 2u) { return vec2<f32>( 0.33, -0.19); }
        if (v == 3u) { return vec2<f32>( 0.0,   0.02); }
        return vec2<f32>(0.0, 0.0);
    }

    // Phase 4: pentachoron (Schlegel diagram)
    if (v == 0u) { return vec2<f32>( 0.0,   0.42); }
    if (v == 1u) { return vec2<f32>(-0.36, -0.21); }
    if (v == 2u) { return vec2<f32>( 0.36, -0.21); }
    if (v == 3u) { return vec2<f32>( 0.0,  -0.42); }
    return vec2<f32>(0.0, 0.0); // v4 = center
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = u.resolution.x / u.resolution.y;
    let uv = (input.uv * 2.0 - 1.0) * vec2<f32>(aspect, 1.0);
    let time = fract(u.time / 120.0) * 120.0;

    // Compute phase: auto-cycle with eased transitions
    var phase: f32;
    if (u.auto_speed > 0.001) {
        let raw = fract(time * u.auto_speed / 6.0) * 6.0; // 0-6 cycle (5 phases + hold)
        let clamped = min(raw, 5.0); // clamp to 0-5
        let sub = fract(clamped);
        // Ease: fast morph in first 60%, hold last 40%
        let eased = smoothstep(0.0, 0.6, sub);
        phase = min(floor(clamped) + eased, 4.0);
    } else {
        phase = clamp(u.phase_offset, 0.0, 4.0);
    }

    // Interpolate between floor/ceil phase states
    let ph_lo = u32(floor(phase));
    let ph_hi = min(ph_lo + 1u, 4u);
    let blend = fract(phase);

    // Compute vertex positions via lerp between phase states
    var pos: array<vec2<f32>, 5>;
    for (var i = 0u; i < 5u; i++) {
        pos[i] = mix(target(i, ph_lo), target(i, ph_hi), blend);
    }

    // 4D rotation perturbation at phase >= 3.5 (pentachoron comes alive)
    let rot_amount = smoothstep(3.5, 4.2, phase);
    if (rot_amount > 0.001) {
        let t = time * 0.4;
        // Outer vertices wobble
        for (var i = 0u; i < 4u; i++) {
            let angle = f32(i) * 1.5708 + 0.3;
            let wobble = sin(t * 0.618 + angle) * 0.025 * rot_amount;
            pos[i] = pos[i] + vec2<f32>(cos(angle + t * 0.2) * wobble, sin(angle + t * 0.3) * wobble);
        }
        // Center vertex Lissajous
        pos[4] = pos[4] + vec2<f32>(sin(t * 1.618) * 0.018, cos(t) * 0.018) * rot_amount;
    }

    // Vertex visibility: vertex i born at phase i (vertex 0 always visible)
    var v_alpha: array<f32, 5>;
    for (var i = 0u; i < 5u; i++) {
        v_alpha[i] = smoothstep(f32(i) - 0.3, f32(i) + 0.2, phase);
    }
    // vertex 0 always alive
    v_alpha[0] = 1.0;

    // Edge visibility: edge (i,j) born when max(i,j) vertex is born
    // 10 possible edges of K5
    // Edge alpha = min(v_alpha[i], v_alpha[j])

    // Compute edge distances and accumulate glow
    var min_d = 999.0;
    var edge_halo = 0.0;

    // Macro: process one edge
    // Edge 0-1 (born at phase 1)
    let ea01 = min(v_alpha[0], v_alpha[1]);
    if (ea01 > 0.01) {
        let d = dist_seg(uv, pos[0], pos[1]);
        min_d = min(min_d, d / ea01);
        edge_halo += exp(-d * 12.0) * ea01;
    }
    // Edge 0-2 (born at phase 2)
    let ea02 = min(v_alpha[0], v_alpha[2]);
    if (ea02 > 0.01) {
        let d = dist_seg(uv, pos[0], pos[2]);
        min_d = min(min_d, d / ea02);
        edge_halo += exp(-d * 12.0) * ea02;
    }
    // Edge 1-2 (born at phase 2)
    let ea12 = min(v_alpha[1], v_alpha[2]);
    if (ea12 > 0.01) {
        let d = dist_seg(uv, pos[1], pos[2]);
        min_d = min(min_d, d / ea12);
        edge_halo += exp(-d * 12.0) * ea12;
    }
    // Edge 0-3 (born at phase 3)
    let ea03 = min(v_alpha[0], v_alpha[3]);
    if (ea03 > 0.01) {
        let d = dist_seg(uv, pos[0], pos[3]);
        min_d = min(min_d, d / ea03);
        edge_halo += exp(-d * 12.0) * ea03;
    }
    // Edge 1-3 (born at phase 3)
    let ea13 = min(v_alpha[1], v_alpha[3]);
    if (ea13 > 0.01) {
        let d = dist_seg(uv, pos[1], pos[3]);
        min_d = min(min_d, d / ea13);
        edge_halo += exp(-d * 12.0) * ea13;
    }
    // Edge 2-3 (born at phase 3)
    let ea23 = min(v_alpha[2], v_alpha[3]);
    if (ea23 > 0.01) {
        let d = dist_seg(uv, pos[2], pos[3]);
        min_d = min(min_d, d / ea23);
        edge_halo += exp(-d * 12.0) * ea23;
    }
    // Edge 0-4 (born at phase 4)
    let ea04 = min(v_alpha[0], v_alpha[4]);
    if (ea04 > 0.01) {
        let d = dist_seg(uv, pos[0], pos[4]);
        min_d = min(min_d, d / ea04);
        edge_halo += exp(-d * 12.0) * ea04;
    }
    // Edge 1-4 (born at phase 4)
    let ea14 = min(v_alpha[1], v_alpha[4]);
    if (ea14 > 0.01) {
        let d = dist_seg(uv, pos[1], pos[4]);
        min_d = min(min_d, d / ea14);
        edge_halo += exp(-d * 12.0) * ea14;
    }
    // Edge 2-4 (born at phase 4)
    let ea24 = min(v_alpha[2], v_alpha[4]);
    if (ea24 > 0.01) {
        let d = dist_seg(uv, pos[2], pos[4]);
        min_d = min(min_d, d / ea24);
        edge_halo += exp(-d * 12.0) * ea24;
    }
    // Edge 3-4 (born at phase 4)
    let ea34 = min(v_alpha[3], v_alpha[4]);
    if (ea34 > 0.01) {
        let d = dist_seg(uv, pos[3], pos[4]);
        min_d = min(min_d, d / ea34);
        edge_halo += exp(-d * 12.0) * ea34;
    }

    // Vertex glow (weighted by visibility)
    var vtx_glow = 0.0;
    for (var i = 0u; i < 5u; i++) {
        let vd = length(uv - pos[i]);
        vtx_glow += exp(-vd * 40.0) * v_alpha[i] * 1.6;
    }

    // Composite glow
    let core = exp(-min_d * 60.0) * 0.95;
    let halo = edge_halo * 0.16;
    let total = (core + halo + vtx_glow) * u.glow_intensity;

    // Dimension-aware color: subtle shift as dimensions increase
    let dim_t = phase / 4.0;
    let gold = vec3<f32>(0.831, 0.686, 0.216);
    let silver = vec3<f32>(0.85, 0.88, 0.92);
    let base_color = mix(silver, gold, smoothstep(0.0, 0.5, dim_t));
    let hot = vec3<f32>(1.0, 0.95, 0.85);
    let color = base_color * total + hot * max(total - 0.5, 0.0) * 0.6;

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
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
uniform float u_p_phase_offset;
uniform float u_p_auto_speed;
uniform float u_p_glow_intensity;

in vec2 v_uv;
out vec4 fragColor;

float dist_seg(vec2 p, vec2 a, vec2 b){
    vec2 pa = p - a, ba = b - a;
    float l2 = dot(ba, ba);
    if (l2 < 0.0001) return length(pa);
    float t = clamp(dot(pa, ba) / l2, 0.0, 1.0);
    return length(pa - ba * t);
}

vec2 target_pos(int v, int ph){
    if (ph == 0) return vec2(0.0);
    if (ph == 1){
        if (v == 0) return vec2(-0.28, 0.0);
        if (v == 1) return vec2( 0.28, 0.0);
        return vec2(0.0);
    }
    if (ph == 2){
        if (v == 0) return vec2( 0.0,   0.32);
        if (v == 1) return vec2(-0.277, -0.16);
        if (v == 2) return vec2( 0.277, -0.16);
        return vec2(0.0);
    }
    if (ph == 3){
        if (v == 0) return vec2( 0.0,   0.38);
        if (v == 1) return vec2(-0.33, -0.19);
        if (v == 2) return vec2( 0.33, -0.19);
        if (v == 3) return vec2( 0.0,   0.02);
        return vec2(0.0);
    }
    // ph == 4: pentachoron Schlegel
    if (v == 0) return vec2( 0.0,   0.42);
    if (v == 1) return vec2(-0.36, -0.21);
    if (v == 2) return vec2( 0.36, -0.21);
    if (v == 3) return vec2( 0.0,  -0.42);
    return vec2(0.0); // v4 center
}

void main(){
    float aspect = u_resolution.x / u_resolution.y;
    vec2 uv = (v_uv * 2.0 - 1.0) * vec2(aspect, 1.0);
    float time = fract(u_time / 120.0) * 120.0;

    float phase;
    if (u_p_auto_speed > 0.001){
        float raw = fract(time * u_p_auto_speed / 6.0) * 6.0;
        float clamped = min(raw, 5.0);
        float sub = fract(clamped);
        float eased = smoothstep(0.0, 0.6, sub);
        phase = min(floor(clamped) + eased, 4.0);
    } else {
        phase = clamp(u_p_phase_offset, 0.0, 4.0);
    }

    int ph_lo = int(floor(phase));
    int ph_hi = min(ph_lo + 1, 4);
    float blend = fract(phase);

    vec2 pos[5];
    for (int i = 0; i < 5; i++){
        pos[i] = mix(target_pos(i, ph_lo), target_pos(i, ph_hi), blend);
    }

    float rot_amount = smoothstep(3.5, 4.2, phase);
    if (rot_amount > 0.001){
        float t = time * 0.4;
        for (int i = 0; i < 4; i++){
            float angle = float(i) * 1.5708 + 0.3;
            float wobble = sin(t * 0.618 + angle) * 0.025 * rot_amount;
            pos[i] += vec2(cos(angle + t * 0.2) * wobble, sin(angle + t * 0.3) * wobble);
        }
        pos[4] += vec2(sin(t * 1.618) * 0.018, cos(t) * 0.018) * rot_amount;
    }

    float v_alpha[5];
    v_alpha[0] = 1.0;
    for (int i = 1; i < 5; i++){
        v_alpha[i] = smoothstep(float(i) - 0.3, float(i) + 0.2, phase);
    }

    float min_d = 999.0;
    float edge_halo = 0.0;

    // Process edges with visibility
    float ea; float d;
    // 0-1
    ea = min(v_alpha[0], v_alpha[1]);
    if (ea > 0.01){ d = dist_seg(uv, pos[0], pos[1]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }
    // 0-2
    ea = min(v_alpha[0], v_alpha[2]);
    if (ea > 0.01){ d = dist_seg(uv, pos[0], pos[2]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }
    // 1-2
    ea = min(v_alpha[1], v_alpha[2]);
    if (ea > 0.01){ d = dist_seg(uv, pos[1], pos[2]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }
    // 0-3
    ea = min(v_alpha[0], v_alpha[3]);
    if (ea > 0.01){ d = dist_seg(uv, pos[0], pos[3]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }
    // 1-3
    ea = min(v_alpha[1], v_alpha[3]);
    if (ea > 0.01){ d = dist_seg(uv, pos[1], pos[3]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }
    // 2-3
    ea = min(v_alpha[2], v_alpha[3]);
    if (ea > 0.01){ d = dist_seg(uv, pos[2], pos[3]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }
    // 0-4
    ea = min(v_alpha[0], v_alpha[4]);
    if (ea > 0.01){ d = dist_seg(uv, pos[0], pos[4]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }
    // 1-4
    ea = min(v_alpha[1], v_alpha[4]);
    if (ea > 0.01){ d = dist_seg(uv, pos[1], pos[4]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }
    // 2-4
    ea = min(v_alpha[2], v_alpha[4]);
    if (ea > 0.01){ d = dist_seg(uv, pos[2], pos[4]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }
    // 3-4
    ea = min(v_alpha[3], v_alpha[4]);
    if (ea > 0.01){ d = dist_seg(uv, pos[3], pos[4]); min_d = min(min_d, d/ea); edge_halo += exp(-d*18.0)*ea; }

    float vtx_glow = 0.0;
    for (int i = 0; i < 5; i++){
        float vd = length(uv - pos[i]);
        vtx_glow += exp(-vd * 40.0) * v_alpha[i] * 1.6;
    }

    float core = exp(-min_d * 60.0) * 0.95;
    float halo = edge_halo * 0.16;
    float total = (core + halo + vtx_glow) * u_p_glow_intensity;

    float dim_t = phase / 4.0;
    vec3 gold = vec3(0.831, 0.686, 0.216);
    vec3 silver = vec3(0.85, 0.88, 0.92);
    vec3 base_color = mix(silver, gold, smoothstep(0.0, 0.5, dim_t));
    vec3 hot = vec3(1.0, 0.95, 0.85);
    vec3 color = base_color * total + hot * max(total - 0.5, 0.0) * 0.6;

    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
`;
const UNIFORMS = [
  { name: 'phase_offset', default: 0.0 },
  { name: 'auto_speed', default: 0.12 },
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


class SimplexUnfold extends HTMLElement {
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
        console.warn('game-simplex-unfold: no WebGPU or WebGL2 support');
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

customElements.define('game-simplex-unfold', SimplexUnfold);
})();
