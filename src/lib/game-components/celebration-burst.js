// GAME Component: celebration — auto-generated, do not edit.
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
    p_ring_r: f32,
    p_glow_str: f32,
    p_gold: f32,
    p_inner_r: f32,
    p_inner_glow: f32,
    p_flash_str: f32,
    p_white: f32,
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
    return exp(-max(d, 0.0) * intensity * 8.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv * 2.0 - 1.0;
    let aspect = u.resolution.x / u.resolution.y;
    let time = fract(u.time / 120.0) * 120.0;

    let ring_r = u.p_ring_r;
    let glow_str = u.p_glow_str;
    let gold = u.p_gold;
    let inner_r = u.p_inner_r;
    let inner_glow = u.p_inner_glow;
    let flash_str = u.p_flash_str;
    let white = u.p_white;

    var final_color = vec4<f32>(0.0, 0.0, 0.0, 1.0);

    // ── Layer 0: ring_outer ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        let sdf_result = abs(length(p) - ring_r) - 0.020000;
        let glow_result = apply_glow(sdf_result, glow_str);
        var color_result = vec4<f32>(vec3<f32>(glow_result), 1.0);
        let pp_lum = dot(color_result.rgb, vec3<f32>(0.299, 0.587, 0.114));
        color_result = vec4<f32>(color_result.rgb + max(pp_lum - 0.300000, 0.0) * 2.000000, 1.0);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(gold, 1.000000, 1.000000), 1.0);
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb + lc * 1.0, 1.0);
    }

    // ── Layer 1: ring_inner ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        let sdf_result = abs(length(p) - inner_r) - 0.015000;
        let glow_result = apply_glow(sdf_result, inner_glow);
        var color_result = vec4<f32>(vec3<f32>(glow_result), 1.0);
        let pp_lum = dot(color_result.rgb, vec3<f32>(0.299, 0.587, 0.114));
        color_result = vec4<f32>(color_result.rgb + max(pp_lum - 0.300000, 0.0) * 2.000000, 1.0);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(gold, 1.000000, 1.000000), 1.0);
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb + lc * 1.0, 1.0);
    }

    // ── Layer 2: center_flash ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        let sdf_result = sdf_circle(p, 0.080000);
        let glow_result = apply_glow(sdf_result, flash_str);
        var color_result = vec4<f32>(vec3<f32>(glow_result), 1.0);
        let pp_lum = dot(color_result.rgb, vec3<f32>(0.299, 0.587, 0.114));
        color_result = vec4<f32>(color_result.rgb + max(pp_lum - 0.400000, 0.0) * 3.000000, 1.0);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(white, 1.000000, 1.000000), 1.0);
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb + lc * 1.0, 1.0);
    }

    return final_color;
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
uniform float u_p_ring_r;
uniform float u_p_glow_str;
uniform float u_p_gold;
uniform float u_p_inner_r;
uniform float u_p_inner_glow;
uniform float u_p_flash_str;
uniform float u_p_white;

in vec2 v_uv;
out vec4 fragColor;

float sdf_circle(vec2 p, float radius){
    return length(p) - radius;
}

float apply_glow(float d, float intensity){
    return exp(-max(d, 0.0) * intensity * 8.0);
}

void main(){
    vec2 uv = v_uv * 2.0 - 1.0;
    float aspect = u_resolution.x / u_resolution.y;
    float time = fract(u_time / 120.0) * 120.0;

    float ring_r = u_p_ring_r;
    float glow_str = u_p_glow_str;
    float gold = u_p_gold;
    float inner_r = u_p_inner_r;
    float inner_glow = u_p_inner_glow;
    float flash_str = u_p_flash_str;
    float white = u_p_white;

    vec4 final_color = vec4(0.0, 0.0, 0.0, 1.0);

    // ── Layer 0: ring_outer ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = abs(length(p) - ring_r) - 0.020000;
        float glow_result = apply_glow(sdf_result, glow_str);

        vec4 color_result = vec4(vec3(glow_result), 1.0);
        float pp_lum = dot(color_result.rgb, vec3(0.299, 0.587, 0.114));
        color_result = vec4(color_result.rgb + max(pp_lum - 0.300000, 0.0) * 2.000000, 1.0);
        color_result = vec4(color_result.rgb * vec3(gold, 1.000000, 1.000000), 1.0);
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb + lc * 1.000, 1.0);
    }

    // ── Layer 1: ring_inner ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = abs(length(p) - inner_r) - 0.015000;
        float glow_result = apply_glow(sdf_result, inner_glow);

        vec4 color_result = vec4(vec3(glow_result), 1.0);
        float pp_lum = dot(color_result.rgb, vec3(0.299, 0.587, 0.114));
        color_result = vec4(color_result.rgb + max(pp_lum - 0.300000, 0.0) * 2.000000, 1.0);
        color_result = vec4(color_result.rgb * vec3(gold, 1.000000, 1.000000), 1.0);
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb + lc * 1.000, 1.0);
    }

    // ── Layer 2: center_flash ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = sdf_circle(p, 0.080000);
        float glow_result = apply_glow(sdf_result, flash_str);

        vec4 color_result = vec4(vec3(glow_result), 1.0);
        float pp_lum = dot(color_result.rgb, vec3(0.299, 0.587, 0.114));
        color_result = vec4(color_result.rgb + max(pp_lum - 0.400000, 0.0) * 3.000000, 1.0);
        color_result = vec4(color_result.rgb * vec3(white, 1.000000, 1.000000), 1.0);
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb + lc * 1.000, 1.0);
    }

    fragColor = final_color;
}
`;
const UNIFORMS = [{name:'ring_r',default:0.1},{name:'glow_str',default:4},{name:'gold',default:0},{name:'inner_r',default:0.05},{name:'inner_glow',default:3},{name:'flash_str',default:2},{name:'white',default:0}];
const USES_MEMORY = false;

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
    this.userParams = {};
    for (const u of uniformDefs) this.userParams[u.name] = u.default;
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

    // 8 base floats + user params, padded to 16-byte alignment
    const floatCount = 8 + 2 + 2 + this.uniformDefs.length; // time,bass,mid,treble,energy,beat + res(2) + mouse(2) + user
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
    data[8] = 0; data[9] = 0; // mouse
    let i = 10;
    for (const u of this.uniformDefs) data[i++] = this.userParams[u.name] ?? u.default;
    this.device.queue.writeBuffer(this.uniformBuffer, 0, data);

    const encoder = this.device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [{
        view: this.ctx.getCurrentTexture().createView(),
        loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 }
      }]
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.bindGroup);
    pass.draw(3);
    pass.end();
    this.device.queue.submit([encoder.finish()]);
  }

  setParam(name, value) { this.userParams[name] = value; }
  setAudioData(d) { Object.assign(this.audioData, d); }
  destroy() { this.stop(); this.device?.destroy(); }
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
    this.userParams = {};
    for (const u of uniformDefs) this.userParams[u.name] = u.default;
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

    // Cache uniform locations
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
    gl.uniform2f(this.locs.mouse, 0, 0);
    for (const u of this.uniformDefs) {
      gl.uniform1f(this.paramLocs[u.name], this.userParams[u.name] ?? u.default);
    }
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  setParam(name, value) { this.userParams[name] = value; }
  setAudioData(d) { Object.assign(this.audioData, d); }
  destroy() { this.stop(); }
}

class Celebration extends HTMLElement {
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
        console.warn('game-celebration: no WebGPU or WebGL2 support');
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

customElements.define('game-celebration', Celebration);
})();
