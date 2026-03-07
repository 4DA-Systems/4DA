// GAME Component: feedback-trails — auto-generated, do not edit.
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
    p_orbit_speed: f32,
    p_trail_hue: f32,
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

    let orbit_speed = u.p_orbit_speed;
    let trail_hue = u.p_trail_hue;

    var final_color = vec4<f32>(0.0, 0.0, 0.0, 1.0);

    // ── Layer 0: config ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
    // ── Layer 1: emitter ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        p = p - vec2<f32>(0.000000, 0.000000);
        var sdf_result = sdf_circle(p, 0.040000);
        let glow_pulse = 3.000000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), 1.0);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(1.000000, 0.400000, 0.100000), 1.0);
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.970000);
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb + lc, 1.0);
    }

    // ── Layer 2: satellite ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        var sdf_result = sdf_circle(p, 0.025000);
        let glow_pulse = 2.500000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), 1.0);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.300000, 0.700000, 1.000000), 1.0);
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.920000);
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb + lc, 1.0);
    }

    // ── Layer 3: field ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        var sdf_result = fbm2((p * 3.000000 + vec2<f32>(time * 0.1, time * 0.07)), i32(4.000000), 0.500000, 2.000000);
        let glow_pulse = 0.500000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), 1.0);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.050000, 0.050000, 0.100000), 1.0);
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb + lc, 1.0);
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
uniform float u_p_orbit_speed;
uniform float u_p_trail_hue;
uniform sampler2D u_prev_frame;


in vec2 v_uv;
out vec4 fragColor;

float sdf_circle(vec2 p, float radius){
    return length(p) - radius;
}

float apply_glow(float d, float intensity){
    return exp(-max(d, 0.0) * intensity * 8.0);
}

float hash2(vec2 p){
    vec3 p3 = fract(vec3(p.x, p.y, p.x) * 0.1031);
    p3 += vec3(dot(p3, p3.yzx + 33.33));
    return fract((p3.x + p3.y) * p3.z);
}

float noise2(vec2 p){
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i), hash2(i + vec2(1.0, 0.0)), u.x),
        mix(hash2(i + vec2(0.0, 1.0)), hash2(i + vec2(1.0, 1.0)), u.x),
        u.y
    ) * 2.0 - 1.0;
}

float fbm2(vec2 p, int octaves, float persistence, float lacunarity){
    float value = 0.0;
    float amplitude = 1.0;
    float frequency = 1.0;
    float max_val = 0.0;
    for (int i = 0; i < octaves; i++) {
        value += noise2(p * frequency) * amplitude;
        max_val += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    return value / max_val;
}

void main(){
    vec2 uv = v_uv * 2.0 - 1.0;
    float aspect = u_resolution.x / u_resolution.y;
    float time = fract(u_time / 120.0) * 120.0;

    float orbit_speed = u_p_orbit_speed;
    float trail_hue = u_p_trail_hue;

    vec4 final_color = vec4(0.0, 0.0, 0.0, 1.0);

    // ── Layer 0: config ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
    // ── Layer 1: emitter ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        p = p - vec2(0.000000, 0.000000);
        float sdf_result = sdf_circle(p, 0.040000);
        float glow_pulse = 3.000000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), 1.0);
        color_result = vec4(color_result.rgb * vec3(1.000000, 0.400000, 0.100000), 1.0);
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.970000);
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb + lc, 1.0);
    }

    // ── Layer 2: satellite ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = sdf_circle(p, 0.025000);
        float glow_pulse = 2.500000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), 1.0);
        color_result = vec4(color_result.rgb * vec3(0.300000, 0.700000, 1.000000), 1.0);
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.920000);
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb + lc, 1.0);
    }

    // ── Layer 3: field ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = fbm2((p * 3.000000 + vec2(time * 0.1, time * 0.07)), int(4.000000), 0.500000, 2.000000);
        float glow_pulse = 0.500000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), 1.0);
        color_result = vec4(color_result.rgb * vec3(0.050000, 0.050000, 0.100000), 1.0);
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb + lc, 1.0);
    }

    fragColor = final_color;
}
`;
const UNIFORMS = [{name:'orbit_speed',default:0.4},{name:'trail_hue',default:0}];

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

    // Memory/feedback: ping-pong textures (Group 1)
    this._initMemory();
    const pipelineLayout = this.device.createPipelineLayout({
      bindGroupLayouts: [bindGroupLayout, this._memBindGroupLayout]
    });

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
    mainPass.setBindGroup(1, this._memBindGroup);
    mainPass.draw(3);
    mainPass.end();

    // Capture frame for memory/feedback
    this._swapMemory(encoder, this.ctx.getCurrentTexture());
    this.device.queue.submit([encoder.finish()]);
  }

  _initMemory() {
    const w = this.canvas.width || 1;
    const h = this.canvas.height || 1;
    const desc = {
      size: { width: w, height: h },
      format: 'rgba8unorm',
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC | GPUTextureUsage.COPY_DST
    };
    this._memTex = [this.device.createTexture(desc), this.device.createTexture(desc)];
    this._memIdx = 0;
    this._memSampler = this.device.createSampler({ magFilter: 'linear', minFilter: 'linear' });
    this._memBindGroupLayout = this.device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float' } },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'filtering' } }
      ]
    });
    this._updateMemBindGroup();
  }

  _updateMemBindGroup() {
    const readTex = this._memTex[this._memIdx];
    this._memBindGroup = this.device.createBindGroup({
      layout: this._memBindGroupLayout,
      entries: [
        { binding: 0, resource: readTex.createView() },
        { binding: 1, resource: this._memSampler }
      ]
    });
  }

  _swapMemory(encoder, sourceTex) {
    const writeTex = this._memTex[1 - this._memIdx];
    encoder.copyTextureToTexture(
      { texture: sourceTex },
      { texture: writeTex },
      { width: this.canvas.width, height: this.canvas.height }
    );
    this._memIdx = 1 - this._memIdx;
    this._updateMemBindGroup();
  }

  _resizeMemory() {
    if (this._memTex) {
      this._memTex[0].destroy();
      this._memTex[1].destroy();
      this._initMemory();
    }
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
    this._initMemoryGL();
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

    // Bind previous frame texture
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this._memTex[this._memIdx]);
    gl.uniform1i(this._memLoc, 1);

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

    // Capture frame for memory/feedback
    this._swapMemoryGL();
  }

  _initMemoryGL() {
    const gl = this.gl;
    const w = this.canvas.width || 1;
    const h = this.canvas.height || 1;
    this._memFbo = [gl.createFramebuffer(), gl.createFramebuffer()];
    this._memTex = [gl.createTexture(), gl.createTexture()];
    for (let i = 0; i < 2; i++) {
      gl.bindTexture(gl.TEXTURE_2D, this._memTex[i]);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.bindFramebuffer(gl.FRAMEBUFFER, this._memFbo[i]);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, this._memTex[i], 0);
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.bindTexture(gl.TEXTURE_2D, null);
    this._memIdx = 0;
    this._memLoc = gl.getUniformLocation(this.program, 'u_prev_frame');
  }

  _swapMemoryGL() {
    const gl = this.gl;
    const w = this.canvas.width;
    const h = this.canvas.height;
    const writeIdx = 1 - this._memIdx;
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, this._memFbo[writeIdx]);
    gl.blitFramebuffer(0, 0, w, h, 0, 0, w, h, gl.COLOR_BUFFER_BIT, gl.NEAREST);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    this._memIdx = writeIdx;
  }

  _resizeMemory() {
    if (this._memTex) {
      const gl = this.gl;
      const w = this.canvas.width || 1;
      const h = this.canvas.height || 1;
      for (let i = 0; i < 2; i++) {
        gl.bindTexture(gl.TEXTURE_2D, this._memTex[i]);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
      }
      gl.bindTexture(gl.TEXTURE_2D, null);
    }
  }

  setParam(name, value) { this.userParams[name] = value; }
  setAudioData(d) { Object.assign(this.audioData, d); }
  destroy() { this.stop(); this.canvas.removeEventListener('mousemove', this._onMouseMove); }
}


class GameResonanceNetwork {
  constructor() {
    this._couplings = [
      { source: 'orbit_speed', target: 'satellite', field: 'scale', weight: 0.3 },
      { source: 'orbit_speed', target: 'emitter', field: 'brightness', weight: 0.5 },
    ];
    this._damping = 0.95;
    this._maxDepth = 4;
    this._state = new Map();
    this._deltas = new Map();
  }

  propagate(uniforms) {
    // Snapshot current values
    const prev = new Map(this._state);
    for (const [k, v] of Object.entries(uniforms)) {
      this._state.set(k, v);
    }

    // Compute deltas from source changes
    this._deltas.clear();
    for (const c of this._couplings) {
      const srcKey = c.source;
      const curVal = this._state.get(srcKey) ?? 0;
      const prevVal = prev.get(srcKey) ?? curVal;
      const delta = (curVal - prevVal) * c.weight;
      if (Math.abs(delta) > 0.0001) {
        const tgtKey = `${c.target}.${c.field}`;
        this._deltas.set(tgtKey, (this._deltas.get(tgtKey) ?? 0) + delta);
      }
    }

    // Apply damped deltas to uniforms
    const result = { ...uniforms };
    for (const [key, delta] of this._deltas) {
      const parts = key.split('.');
      const paramName = parts.length > 1 ? parts[1] : parts[0];
      if (paramName in result) {
        result[paramName] += delta * this._damping;
      }
    }

    // Multi-hop cascade (depth-limited)
    for (let depth = 1; depth < this._maxDepth; depth++) {
      let anyChange = false;
      for (const c of this._couplings) {
        const tgtKey = `${c.target}.${c.field}`;
        const srcDelta = this._deltas.get(c.source) ?? 0;
        if (Math.abs(srcDelta) > 0.0001) {
          const cascadeDelta = srcDelta * c.weight * Math.pow(this._damping, depth);
          this._deltas.set(tgtKey, (this._deltas.get(tgtKey) ?? 0) + cascadeDelta);
          const parts = tgtKey.split('.');
          const pn = parts.length > 1 ? parts[1] : parts[0];
          if (pn in result) { result[pn] += cascadeDelta; anyChange = true; }
        }
      }
      if (!anyChange) break;
    }

    // Update state for next frame
    for (const [k, v] of Object.entries(result)) {
      this._state.set(k, v);
    }

    return result;
  }

  get couplings() { return this._couplings; }
  get activeDeltas() { return Object.fromEntries(this._deltas); }
}


const _gameEasings = {
  linear: t => t,
  ease_in_out: t => t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t,
};

class GameArcTimeline {
  constructor() {
    this._startTime = null;
    this._entries = [
      { target: 'orbit_speed', from: 0, to: 1, duration: 5, easing: 'ease_in_out' },
    ];
  }

  evaluate(elapsedSec) {
    if (this._startTime === null) this._startTime = elapsedSec;
    const t = elapsedSec - this._startTime;
    const values = {};

    for (const e of this._entries) {
      const progress = Math.min(t / e.duration, 1.0);
      const easeFn = _gameEasings[e.easing] || _gameEasings.linear;
      const eased = easeFn(progress);
      values[e.target] = e.from + (e.to - e.from) * eased;
    }

    return values;
  }

  isComplete(elapsedSec) {
    if (this._startTime === null) return false;
    const t = elapsedSec - this._startTime;
    return this._entries.every(e => t >= e.duration);
  }

  reset() { this._startTime = null; }

  progress(elapsedSec) {
    if (this._startTime === null) return 0;
    const t = elapsedSec - this._startTime;
    const maxDur = Math.max(...this._entries.map(e => e.duration));
    return Math.min(t / maxDur, 1.0);
  }
}


class FeedbackTrails extends HTMLElement {
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
        console.warn('game-feedback-trails: no WebGPU or WebGL2 support');
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
    if (this._renderer?._resizeMemory) this._renderer._resizeMemory();
  }

  setParam(name, value) { this._renderer?.setParam(name, value); }
  setAudioData(data) { this._renderer?.setAudioData(data); }
  setAudioSource(bridge) { bridge?.subscribe(d => this._renderer?.setAudioData(d)); }

  // Property accessors for each uniform
  get orbit_speed() { return this._renderer?.userParams['orbit_speed'] ?? 0; }
  set orbit_speed(v) { this.setParam('orbit_speed', v); }
  get trail_hue() { return this._renderer?.userParams['trail_hue'] ?? 0; }
  set trail_hue(v) { this.setParam('trail_hue', v); }

  static get observedAttributes() { return UNIFORMS.map(u => u.name); }
  attributeChangedCallback(name, _, val) {
    if (val !== null) this.setParam(name, parseFloat(val));
  }
}

customElements.define('game-feedback-trails', FeedbackTrails);
})();
