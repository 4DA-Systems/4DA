// GAME Component: physarum-network — auto-generated, do not edit.
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
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn sdf_circle(p: vec2<f32>, radius: f32) -> f32 {
    return length(p) - radius;
}

fn apply_glow(d: f32, intensity: f32) -> f32 {
    let edge = 0.005;
    let core = smoothstep(edge, -edge, d);
    let halo = intensity / (1.0 + max(d, 0.0) * max(d, 0.0) * intensity * intensity * 16.0);
    return core + halo;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv * 2.0 - 1.0;
    let aspect = u.resolution.x / u.resolution.y;
    let time = fract(u.time / 120.0) * 120.0;

    // ── Layer 0: bg ──
    var p = vec2<f32>(uv.x * aspect, uv.y);
    var sdf_result = sdf_circle(p, 0.500000);
    let glow_pulse = 1.500000 * (0.9 + 0.1 * sin(time * 2.0));
    let glow_result = apply_glow(sdf_result, glow_pulse);
    var color_result = vec4<f32>(vec3<f32>(glow_result), 1.0);
    color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.000000, 0.000000, 0.000000), 1.0);
    return color_result;
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

in vec2 v_uv;
out vec4 fragColor;

float sdf_circle(vec2 p, float radius){
    return length(p) - radius;
}

float apply_glow(float d, float intensity){
    float edge = 0.005;
    float core = smoothstep(edge, -edge, d);
    float halo = intensity / (1.0 + max(d, 0.0) * max(d, 0.0) * intensity * intensity * 16.0);
    return core + halo;
}

void main(){
    vec2 uv = v_uv * 2.0 - 1.0;
    float aspect = u_resolution.x / u_resolution.y;
    float time = fract(u_time / 120.0) * 120.0;

    // ── Layer 0: bg ──
    vec2 p = vec2(uv.x * aspect, uv.y);
    float sdf_result = sdf_circle(p, 0.500000);
    float glow_pulse = 1.500000 * (0.9 + 0.1 * sin(time * 2.0));
    float glow_result = apply_glow(sdf_result, glow_pulse);

    vec4 color_result = vec4(vec3(glow_result), 1.0);
    color_result = vec4(color_result.rgb * vec3(0.000000, 0.000000, 0.000000), 1.0);
    fragColor = color_result;
}
`;
const UNIFORMS = [];
const SWARM_AGENT_WGSL = `struct Agent {
    pos: vec2<f32>,
    angle: f32,
    _pad: f32,
};

struct SwarmParams {
    sensor_angle: f32,
    sensor_dist: f32,
    turn_angle: f32,
    step_size: f32,
    deposit: f32,
    width: u32,
    height: u32,
    count: u32,
    time: f32,
};

@group(0) @binding(0) var<uniform> params: SwarmParams;
@group(0) @binding(1) var<storage, read_write> agents: array<Agent>;
@group(0) @binding(2) var<storage, read_write> trail: array<f32>;

fn hash(seed: u32) -> f32 {
    var x = seed;
    x = x ^ (x >> 16u);
    x = x * 0x45d9f3bu;
    x = x ^ (x >> 16u);
    x = x * 0x45d9f3bu;
    x = x ^ (x >> 16u);
    return f32(x) / 4294967295.0;
}

fn sample_trail(x: f32, y: f32) -> f32 {
    let ix = u32(x + f32(params.width)) % params.width;
    let iy = u32(y + f32(params.height)) % params.height;
    return trail[iy * params.width + ix];
}

@compute @workgroup_size(64)
fn cs_agent(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.count) { return; }

    var agent = agents[idx];
    let rng = hash(idx * 1000u + u32(params.time * 1000.0));

    // Sense: forward, left, right
    let sense_l = sample_trail(agent.pos.x + cos(agent.angle + 0.7853981633974483) * 9, agent.pos.y + sin(agent.angle + 0.7853981633974483) * 9);
    let sense_f = sample_trail(agent.pos.x + cos(agent.angle) * 9, agent.pos.y + sin(agent.angle) * 9);
    let sense_r = sample_trail(agent.pos.x + cos(agent.angle - 0.7853981633974483) * 9, agent.pos.y + sin(agent.angle - 0.7853981633974483) * 9);

    // Turn toward strongest pheromone
    if (sense_f >= sense_l && sense_f >= sense_r) {
        // Keep going forward
    } else if (sense_l > sense_r) {
        agent.angle += 0.7853981633974483;
    } else if (sense_r > sense_l) {
        agent.angle -= 0.7853981633974483;
    } else {
        agent.angle += (rng - 0.5) * 0.7853981633974483 * 2.0;
    }

    agent.pos.x += cos(agent.angle) * 1;
    agent.pos.y += sin(agent.angle) * 1;

    // Wrap boundaries
    agent.pos.x = (agent.pos.x + f32(params.width)) % f32(params.width);
    agent.pos.y = (agent.pos.y + f32(params.height)) % f32(params.height);
    // Deposit pheromone at current position
    let dep_x = u32(agent.pos.x) % params.width;
    let dep_y = u32(agent.pos.y) % params.height;
    trail[dep_y * params.width + dep_x] += 5;

    agents[idx] = agent;
}
`;
const SWARM_TRAIL_WGSL = `struct TrailParams {
    width: u32,
    height: u32,
};

@group(0) @binding(0) var<uniform> params: TrailParams;
@group(0) @binding(1) var<storage, read> trail_in: array<f32>;
@group(0) @binding(2) var<storage, read_write> trail_out: array<f32>;

@compute @workgroup_size(8, 8)
fn cs_diffuse(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.width || gid.y >= params.height) { return; }

    // 3x3 box blur
    var sum = 0.0;
    for (var dy: i32 = -1; dy <= 1; dy = dy + 1) {
        for (var dx: i32 = -1; dx <= 1; dx = dx + 1) {
            let nx = u32((i32(gid.x) + dx + i32(params.width)) % i32(params.width));
            let ny = u32((i32(gid.y) + dy + i32(params.height)) % i32(params.height));
            sum += trail_in[ny * params.width + nx];
        }
    }

    trail_out[gid.y * params.width + gid.x] = (sum / 9.0) * 0.95;
}
`;

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
    data[8] = this.mouseX; data[9] = this.mouseY;
    let i = 10;
    for (const u of this.uniformDefs) data[i++] = this.userParams[u.name] ?? u.default;
    this.device.queue.writeBuffer(this.uniformBuffer, 0, data);

    const encoder = this.device.createCommandEncoder();

    const mainPass = encoder.beginRenderPass({
      colorAttachments: [{
        view: this.ctx.getCurrentTexture().createView(),
        loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 }
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
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(this.program);

    gl.uniform1f(this.locs.time, t);
    gl.uniform1f(this.locs.bass, this.audioData.bass);
    gl.uniform1f(this.locs.mid, this.audioData.mid);
    gl.uniform1f(this.locs.treble, this.audioData.treble);
    gl.uniform1f(this.locs.energy, this.audioData.energy);
    gl.uniform1f(this.locs.beat, this.audioData.beat);
    gl.uniform2f(this.locs.resolution, this.canvas.width, this.canvas.height);
    gl.uniform2f(this.locs.mouse, this.mouseX, this.mouseY);
    for (const u of this.uniformDefs) {
      gl.uniform1f(this.paramLocs[u.name], this.userParams[u.name] ?? u.default);
    }
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  setParam(name, value) { this.userParams[name] = value; }
  setAudioData(d) { Object.assign(this.audioData, d); }
  destroy() { this.stop(); this.canvas.removeEventListener('mousemove', this._onMouseMove); }
}


class GameSwarmSim {
  constructor(device, agentCode, trailCode) { this._count = 100000; this._w = 512; this._h = 512; this._device = device; this._agentCode = agentCode; this._trailCode = trailCode; }

  async init() {
    const device = this._device;
    const agentModule = device.createShaderModule({ code: this._agentCode });
    this._agentPipeline = device.createComputePipeline({
      layout: 'auto',
      compute: { module: agentModule, entryPoint: 'cs_agent' },
    });

    const trailModule = device.createShaderModule({ code: this._trailCode });
    this._trailPipeline = device.createComputePipeline({
      layout: 'auto',
      compute: { module: trailModule, entryPoint: 'cs_diffuse' },
    });

    const agentSize = 16; // vec2 + f32 + pad
    this._agentBuf = device.createBuffer({ size: this._count * agentSize, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    const trailSize = this._w * this._h * 4;
    this._trailA = device.createBuffer({ size: trailSize, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    this._trailB = device.createBuffer({ size: trailSize, usage: GPUBufferUsage.STORAGE });
    this._paramBuf = device.createBuffer({ size: 36, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this._trailParamBuf = device.createBuffer({ size: 8, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });

    const initAgents = new Float32Array(this._count * 4);
    for (let i = 0; i < this._count; i++) {
      initAgents[i*4] = Math.random() * this._w;
      initAgents[i*4+1] = Math.random() * this._h;
      initAgents[i*4+2] = Math.random() * Math.PI * 2;
      initAgents[i*4+3] = 0;
    }
    device.queue.writeBuffer(this._agentBuf, 0, initAgents);

    const tp = new Uint32Array([this._w, this._h]);
    device.queue.writeBuffer(this._trailParamBuf, 0, tp);
    this._time = 0;
  }

  dispatch(dt) {
    this._time += dt;
    const device = this._device;

    const p = new ArrayBuffer(36);
    const f = new Float32Array(p); const u = new Uint32Array(p);
    f[0] = 0.7853981633974483; f[1] = 9; f[2] = 0.7853981633974483; f[3] = 1; f[4] = 5;
    u[5] = this._w; u[6] = this._h; u[7] = this._count;
    f[8] = this._time;
    device.queue.writeBuffer(this._paramBuf, 0, p);

    const enc = device.createCommandEncoder();

    const agentBG = device.createBindGroup({
      layout: this._agentPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this._paramBuf } },
        { binding: 1, resource: { buffer: this._agentBuf } },
        { binding: 2, resource: { buffer: this._trailA } },
      ],
    });
    const ap = enc.beginComputePass();
    ap.setPipeline(this._agentPipeline);
    ap.setBindGroup(0, agentBG);
    ap.dispatchWorkgroups(Math.ceil(this._count / 64));
    ap.end();

    const trailBG = device.createBindGroup({
      layout: this._trailPipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this._trailParamBuf } },
        { binding: 1, resource: { buffer: this._trailA } },
        { binding: 2, resource: { buffer: this._trailB } },
      ],
    });
    const tp = enc.beginComputePass();
    tp.setPipeline(this._trailPipeline);
    tp.setBindGroup(0, trailBG);
    tp.dispatchWorkgroups(Math.ceil(this._w / 8), Math.ceil(this._h / 8));
    tp.end();

    device.queue.submit([enc.finish()]);
    [this._trailA, this._trailB] = [this._trailB, this._trailA];
  }

  get trailBuffer() { return this._trailA; }
  get agentBuffer() { return this._agentBuf; }
  get agentCount() { return 100000; }
}


class PhysarumNetwork extends HTMLElement {
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
        console.warn('game-physarum-network: no WebGPU or WebGL2 support');
        return;
      }
    }
    this._resize();
    if (this._renderer.device) {
      const dev = this._renderer.device;
      if (typeof SWARM_AGENT_WGSL !== 'undefined') {
        const sim = new GameSwarmSim(dev, SWARM_AGENT_WGSL, SWARM_TRAIL_WGSL);
        await sim.init();
        this._swarmSim = sim;
      }
      this._renderer._preRender = () => {
        const dt = 1/60;
        if (this._swarmSim) this._swarmSim.dispatch(dt);
      };
    }
    this._renderer.start();
  }

  _resize() {
    const rect = this.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    this._canvas.width = Math.round(rect.width * dpr);
    this._canvas.height = Math.round(rect.height * dpr);
    if (this._renderer?._resizeMemory) this._renderer._resizeMemory();
  }

  setParam(name, value) { this._renderer?.setParam(name, value); }
  setAudioData(data) { this._renderer?.setAudioData(data); }
  setAudioSource(bridge) { bridge?.subscribe(d => this._renderer?.setAudioData(d)); }

  // Property accessors for each uniform

  static get observedAttributes() { return UNIFORMS.map(u => u.name); }
  attributeChangedCallback(name, _, val) {
    if (val !== null) this.setParam(name, parseFloat(val));
  }
}

customElements.define('game-physarum-network', PhysarumNetwork);
})();
