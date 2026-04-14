// GAME Component: notif-card-high — auto-generated, do not edit.
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
    mouse_down: f32,
    aspect_ratio: f32,
    p_intensity: f32,
    p_hover: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

@group(1) @binding(0) var prev_frame: texture_2d<f32>;
@group(1) @binding(1) var prev_sampler: sampler;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

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

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = x * (2.51 * x + 0.03);
    let b = x * (2.43 * x + 0.59) + 0.14;
    return clamp(a / b, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn dither_noise(uv: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(uv, vec2<f32>(0.06711056, 0.00583715))));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv * 2.0 - 1.0;
    let aspect = u.aspect_ratio;
    let time = fract(u.time / 120.0) * 120.0;
    let mouse_x = u.mouse.x;
    let mouse_y = u.mouse.y;
    let mouse_down = u.mouse_down;

    let intensity = u.p_intensity;
    let hover = u.p_hover;

    var final_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // ── Layer 1: halo ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        var sdf_result = sdf_box(p, 2.620000, 0.880000);
        sdf_result = sdf_result - 0.080000;
        sdf_result = abs(sdf_result) - 0.050000;
        let glow_pulse = 0.300000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.200000, 0.080000, 0.020000), color_result.a);
        let la = color_result.a * 0.030000;
        let lc = color_result.rgb * 0.030000;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 2: sweep ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        { let ra = time * 0.500000; let rc = cos(ra); let rs = sin(ra);
        p = vec2<f32>(p.x * rc - p.y * rs, p.x * rs + p.y * rc); }
        var sdf_result = sdf_box(p, 2.550000, 0.830000);
        sdf_result = sdf_result - 0.070000;
        sdf_result = abs(sdf_result) - 0.012000;
        let arc_theta = atan2(p.x, p.y) + 3.14159265359;
        sdf_result = select(999.0, sdf_result, arc_theta < 0.350000);
        let glow_pulse = 0.200000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.400000, 0.150000, 0.030000), color_result.a);
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.500000);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 3: track ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        var sdf_result = sdf_box(p, 2.550000, 0.830000);
        sdf_result = sdf_result - 0.070000;
        sdf_result = abs(sdf_result) - 0.003000;
        let glow_pulse = 0.080000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.100000, 0.040000, 0.020000), color_result.a);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 4: card ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        var sdf_result = sdf_box(p, 2.560000, 0.840000);
        sdf_result = sdf_result - 0.065000;
        let shade_fw = fwidth(sdf_result);
        let shade_alpha = 1.0 - smoothstep(-shade_fw, shade_fw, sdf_result);
        var color_result = vec4<f32>(vec3<f32>(0.055000, 0.055000, 0.055000) * shade_alpha, shade_alpha);
        let la = color_result.a;
        let lc = color_result.rgb;
        final_color = vec4<f32>(mix(final_color.rgb, lc, la), final_color.a + la * (1.0 - final_color.a));
    }

    // ── Layer 5: noise ──
    {
        var p = vec2<f32>(uv.x * aspect, uv.y);
        { let warp_x = fbm2(p * 2.000000 + vec2<f32>(0.0, 1.3), i32(2.000000), 0.020000, 2.000000);
        let warp_y = fbm2(p * 2.000000 + vec2<f32>(1.7, 0.0), i32(2.000000), 0.020000, 2.000000);
        p = p + vec2<f32>(warp_x, warp_y) * 0.020000; }
        var sdf_result = fbm2((p * 3.000000 + vec2<f32>(time * 0.1, time * 0.07)), i32(3.000000), 0.500000, 2.000000);
        let glow_pulse = 0.300000 * (0.9 + 0.1 * sin(time * 2.0));
        let glow_result = apply_glow(sdf_result, glow_pulse);
        var color_result = vec4<f32>(vec3<f32>(glow_result), glow_result);
        color_result = vec4<f32>(color_result.rgb * vec3<f32>(0.080000, 0.030000, 0.010000), color_result.a);
        let prev_color = textureSample(prev_frame, prev_sampler, input.uv);
        color_result = mix(color_result, prev_color, 0.940000);
        let la = color_result.a * 0.010000;
        let lc = color_result.rgb * 0.010000;
        final_color = vec4<f32>(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    final_color = vec4<f32>(aces_tonemap(final_color.rgb), final_color.a);
    final_color = final_color + (dither_noise(input.uv * u.resolution) - 0.5) / 255.0;
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
uniform float u_mouse_down;
uniform float u_aspect_ratio;
uniform float u_p_intensity;
uniform float u_p_hover;
uniform sampler2D u_prev_frame;


in vec2 v_uv;
out vec4 fragColor;

float sdf_box(vec2 p, float w, float h){
    vec2 d = abs(p) - vec2(w, h);
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0);
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

vec3 aces_tonemap(vec3 x) {
    vec3 a = x * (2.51 * x + 0.03);
    vec3 b = x * (2.43 * x + 0.59) + 0.14;
    return clamp(a / b, 0.0, 1.0);
}

float dither_noise(vec2 uv) {
    return fract(52.9829189 * fract(dot(uv, vec2(0.06711056, 0.00583715))));
}

void main(){
    vec2 uv = v_uv * 2.0 - 1.0;
    float aspect = u_aspect_ratio;
    float time = fract(u_time / 120.0) * 120.0;
    float mouse_x = u_mouse.x;
    float mouse_y = u_mouse.y;
    float mouse_down = u_mouse_down;

    float intensity = u_p_intensity;
    float hover = u_p_hover;

    vec4 final_color = vec4(0.0, 0.0, 0.0, 0.0);

    // ── Layer 1: halo ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = sdf_box(p, 2.620000, 0.880000);
        sdf_result -= 0.080000;
        sdf_result = abs(sdf_result) - 0.050000;
        float glow_pulse = 0.300000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(0.200000, 0.080000, 0.020000), color_result.a);
        float la = color_result.a * 0.030000;
        vec3 lc = color_result.rgb * 0.030000;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 2: sweep ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        { float ra = time * 0.500000; float rc = cos(ra); float rs = sin(ra);
        p = vec2(p.x * rc - p.y * rs, p.x * rs + p.y * rc); }
        float sdf_result = sdf_box(p, 2.550000, 0.830000);
        sdf_result -= 0.070000;
        sdf_result = abs(sdf_result) - 0.012000;
        float arc_theta = atan(p.x, p.y) + 3.14159265359;
        sdf_result = (arc_theta < 0.350000 ? sdf_result : 999.0);
        float glow_pulse = 0.200000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(0.400000, 0.150000, 0.030000), color_result.a);
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.500000);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 3: track ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = sdf_box(p, 2.550000, 0.830000);
        sdf_result -= 0.070000;
        sdf_result = abs(sdf_result) - 0.003000;
        float glow_pulse = 0.080000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(0.100000, 0.040000, 0.020000), color_result.a);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    // ── Layer 4: card ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        float sdf_result = sdf_box(p, 2.560000, 0.840000);
        sdf_result -= 0.065000;
        float shade_fw = fwidth(sdf_result);
        float shade_alpha = 1.0 - smoothstep(-shade_fw, shade_fw, sdf_result);
        vec4 color_result = vec4(vec3(0.055000, 0.055000, 0.055000) * shade_alpha, shade_alpha);
        float la = color_result.a;
        vec3 lc = color_result.rgb;
        final_color = vec4(mix(final_color.rgb, lc, la), final_color.a + la * (1.0 - final_color.a));
    }

    // ── Layer 5: noise ──
    {
        vec2 p = vec2(uv.x * aspect, uv.y);
        { float warp_x = fbm2(p * 2.000000 + vec2(0.0, 1.3), int(2.000000), 0.020000, 2.000000);
        float warp_y = fbm2(p * 2.000000 + vec2(1.7, 0.0), int(2.000000), 0.020000, 2.000000);
        p = p + vec2(warp_x, warp_y) * 0.020000; }
        float sdf_result = fbm2((p * 3.000000 + vec2(time * 0.1, time * 0.07)), int(3.000000), 0.500000, 2.000000);
        float glow_pulse = 0.300000 * (0.9 + 0.1 * sin(time * 2.0));
        float glow_result = apply_glow(sdf_result, glow_pulse);

        vec4 color_result = vec4(vec3(glow_result), glow_result);
        color_result = vec4(color_result.rgb * vec3(0.080000, 0.030000, 0.010000), color_result.a);
        vec4 prev_color = texture(u_prev_frame, v_uv);
        color_result = mix(color_result, prev_color, 0.940000);
        float la = color_result.a * 0.010000;
        vec3 lc = color_result.rgb * 0.010000;
        final_color = vec4(final_color.rgb * (1.0 - la) + lc, final_color.a * (1.0 - la) + la);
    }

    final_color = vec4(aces_tonemap(final_color.rgb), final_color.a);
    final_color += (dither_noise(v_uv * u_resolution) - 0.5) / 255.0;
    fragColor = final_color;
}
`;
const UNIFORMS = [{name:'intensity',default:1},{name:'hover',default:0}];
const PASS_WGSL_0 = `// Post-processing pass: edge

struct Uniforms {
    time: f32,
    audio_bass: f32,
    audio_mid: f32,
    audio_treble: f32,
    audio_energy: f32,
    audio_beat: f32,
    resolution: vec2<f32>,
    mouse: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(3) var pass_tex: texture_2d<f32>;
@group(0) @binding(4) var pass_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let pixel = textureSample(pass_tex, pass_sampler, uv);
    var color_result = pixel;

    let vign = 1.0 - 0.040000 * length(uv - 0.5);
    color_result = vec4<f32>(color_result.rgb * vign, color_result.a * vign);
    return color_result;
}
`;
const PASS_SHADERS = [PASS_WGSL_0];

class GameResonanceNetwork {
  constructor() {
    this._couplings = [
      { source: 'intensity', target: 'sweep', field: 'brightness', weight: 0.25 },
      { source: 'hover', target: 'sweep', field: 'brightness', weight: 0.08 },
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


class NotifCardHigh extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this._renderer = null;
    this._resizeObserver = null;
    this._pendingParams = {};
  }

  connectedCallback() {
    const style = document.createElement('style');
    style.textContent = ':host{display:block;width:100%;height:100%;position:relative}canvas{width:100%;height:100%;display:block}';
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
    const gpu = new GameRenderer(this._canvas, WGSL_V, WGSL_F, UNIFORMS, PASS_SHADERS);
    if (await gpu.init()) {
      this._renderer = gpu;
    } else {
      const gl = new GameRendererGL(this._canvas, GLSL_V, GLSL_F, UNIFORMS);
      if (gl.init()) {
        this._renderer = gl;
      } else {
        console.warn('fourda-notif-card-high: no WebGPU or WebGL2 support');
        return;
      }
    }
    this._resize();
    for (const [k, v] of Object.entries(this._pendingParams)) {
      this._renderer.setParam(k, v);
    }
    this._renderer.start();
  }

  _resize() {
    const rect = this.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    this._canvas.width = Math.round(rect.width * dpr);
    this._canvas.height = Math.round(rect.height * dpr);
    if (this._renderer?._resizeMemory) this._renderer._resizeMemory();
    if (this._renderer?._resizePassFBOs) this._renderer._resizePassFBOs();
  }

  setParam(name, value) {
    this._pendingParams[name] = value;
    this._renderer?.setParam(name, value);
  }
  setAudioData(data) { this._renderer?.setAudioData(data); }
  setAudioSource(bridge) { bridge?.subscribe(d => this._renderer?.setAudioData(d)); }

  getFrame() {
    if (!this._canvas) return null;
    const w = this._canvas.width, h = this._canvas.height;
    const offscreen = document.createElement('canvas');
    offscreen.width = w;
    offscreen.height = h;
    const ctx = offscreen.getContext('2d');
    ctx.drawImage(this._canvas, 0, 0);
    return ctx.getImageData(0, 0, w, h);
  }

  getFrameDataURL(type) {
    if (!this._canvas) return null;
    return this._canvas.toDataURL(type || 'image/png');
  }

  // Property accessors for each uniform
  get intensity() { return this._renderer?.userParams['intensity'] ?? this._pendingParams['intensity'] ?? 1; }
  set intensity(v) { this.setParam('intensity', v); }
  get hover() { return this._renderer?.userParams['hover'] ?? this._pendingParams['hover'] ?? 0; }
  set hover(v) { this.setParam('hover', v); }
  get health() { return this.intensity; }
  set health(v) { this.intensity = v; }

  static get observedAttributes() { return UNIFORMS.map(u => u.name); }
  attributeChangedCallback(name, _, val) {
    if (val !== null) this.setParam(name, parseFloat(val));
  }
}

customElements.define('fourda-notif-card-high', NotifCardHigh);
})();
