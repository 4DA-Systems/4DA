import { useRef, useEffect, useMemo } from 'react';
import type { VoidSignal } from '../../types';
import { computeCoreColor } from './void-colors';

interface VoidHeartbeatProps {
  signal: VoidSignal;
  size?: number;
}

/**
 * Ambient heartbeat indicator for 4DA.
 *
 * Renders as a CSS-animated luminous circle that communicates system state
 * through visual change: pulse speed, color warmth, glow intensity.
 *
 * Optionally upgrades to a WebGL2 fragment shader for richer visuals.
 * Falls back to CSS silently if WebGL2 is unavailable (RDP, VMs, etc).
 */
export function VoidHeartbeat({ signal, size = 200 }: VoidHeartbeatProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const glRef = useRef<WebGL2RenderingContext | null>(null);
  const programRef = useRef<WebGLProgram | null>(null);
  // eslint-disable-next-line no-undef
  const uniformLocationsRef = useRef<Record<string, WebGLUniformLocation | null>>({});
  const startTimeRef = useRef(Date.now());
  const rafRef = useRef<number>(0);
  const webglAvailable = useRef<boolean | null>(null);

  // Derive visual parameters from signal
  const pulseSpeed = useMemo(() => {
    if (signal.error > 0.5) return 0.3; // Fast flicker
    if (signal.staleness > 0.8) return 6.0; // Very slow
    // Critical signals make pulse faster
    if (signal.critical_count > 0) return 1.0;
    // Map pulse: idle=4s, active=0.8s
    return 4.0 - signal.pulse * 3.2;
  }, [signal.pulse, signal.error, signal.staleness, signal.critical_count]);

  const coreColor = useMemo(
    () => computeCoreColor(signal.heat, signal.error, signal.staleness, signal.signal_color_shift),
    [signal.heat, signal.error, signal.staleness, signal.signal_color_shift],
  );

  const glowRadius = useMemo(() => {
    const base = 8;
    const burstBoost = signal.burst * 16;
    return base + burstBoost;
  }, [signal.burst]);

  const opacity = useMemo(() => {
    if (signal.item_count === 0 && signal.staleness > 0.9) return 0.15; // Dormant
    if (signal.staleness > 0.8) return 0.3; // Stale
    return 0.5 + signal.heat * 0.5; // Active range: 0.5 - 1.0
  }, [signal.item_count, signal.staleness, signal.heat]);

  // Attempt WebGL2 initialization
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || webglAvailable.current !== null) return;

    const gl = canvas.getContext('webgl2', {
      alpha: true,
      premultipliedAlpha: false,
      antialias: false,
      powerPreference: 'low-power',
    });

    if (!gl) {
      webglAvailable.current = false;
      return;
    }

    // Compile shader
    const program = createShaderProgram(gl);
    if (!program) {
      webglAvailable.current = false;
      return;
    }

    glRef.current = gl;
    programRef.current = program;
    webglAvailable.current = true;

    // Cache uniform locations
    uniformLocationsRef.current = {
      u_time: gl.getUniformLocation(program, 'u_time'),
      u_resolution: gl.getUniformLocation(program, 'u_resolution'),
      u_pulse: gl.getUniformLocation(program, 'u_pulse'),
      u_heat: gl.getUniformLocation(program, 'u_heat'),
      u_burst: gl.getUniformLocation(program, 'u_burst'),
      u_error: gl.getUniformLocation(program, 'u_error'),
      u_staleness: gl.getUniformLocation(program, 'u_staleness'),
      u_opacity: gl.getUniformLocation(program, 'u_opacity'),
      u_signal_intensity: gl.getUniformLocation(program, 'u_signal_intensity'),
      u_signal_color_shift: gl.getUniformLocation(program, 'u_signal_color_shift'),
      u_critical_count: gl.getUniformLocation(program, 'u_critical_count'),
    };

    // Set up fullscreen quad
    const vertices = new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
    const posLoc = gl.getAttribLocation(program, 'a_position');
    gl.enableVertexAttribArray(posLoc);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

    return () => {
      cancelAnimationFrame(rafRef.current);
      gl.deleteProgram(program);
      gl.deleteBuffer(buffer);
    };
  }, []);

  // WebGL2 render loop
  useEffect(() => {
    if (!webglAvailable.current) return;

    const gl = glRef.current;
    const program = programRef.current;
    if (!gl || !program) return;

    gl.useProgram(program);

    const render = () => {
      rafRef.current = requestAnimationFrame(render);

      const elapsed = (Date.now() - startTimeRef.current) / 1000.0;
      const canvas = canvasRef.current;
      if (!canvas) return;

      canvas.width = size * window.devicePixelRatio;
      canvas.height = size * window.devicePixelRatio;
      gl.viewport(0, 0, canvas.width, canvas.height);

      const locs = uniformLocationsRef.current;
      gl.uniform1f(locs.u_time, elapsed);
      gl.uniform2f(locs.u_resolution, canvas.width, canvas.height);
      gl.uniform1f(locs.u_pulse, signal.pulse);
      gl.uniform1f(locs.u_heat, signal.heat);
      gl.uniform1f(locs.u_burst, signal.burst);
      gl.uniform1f(locs.u_error, signal.error);
      gl.uniform1f(locs.u_staleness, signal.staleness);
      gl.uniform1f(locs.u_opacity, opacity);
      gl.uniform1f(locs.u_signal_intensity, signal.signal_intensity);
      gl.uniform1f(locs.u_signal_color_shift, signal.signal_color_shift);
      gl.uniform1i(locs.u_critical_count, signal.critical_count);

      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    };

    rafRef.current = requestAnimationFrame(render);
    return () => cancelAnimationFrame(rafRef.current);
  }, [signal, opacity, size]);

  // State label: signal-aware labels take priority over legacy labels
  const stateLabel = useMemo(() => {
    // Signal-aware labels (highest priority)
    if (signal.critical_count > 0 && signal.signal_intensity > 0.75) {
      return signal.critical_count > 1 ? `${signal.critical_count} Alerts` : 'Alert';
    }
    if (signal.signal_color_shift > 0.5) return 'Breaking';
    if (signal.signal_color_shift > 0.2) return 'Discovery';
    if (signal.signal_color_shift < -0.3) return 'Learning';

    // Legacy labels (fallback)
    if (signal.item_count === 0 && signal.heat === 0) {
      return signal.staleness > 0.9 ? 'Dormant' : 'Awakening';
    }
    if (signal.error > 0.5) return 'Error';
    if (signal.staleness > 0.8) return 'Stale';
    if (signal.pulse > 0.5) return 'Scanning';
    if (signal.heat > 0.5) return 'Discoveries';
    if (signal.item_count > 0) return 'Active';
    return 'Idle';
  }, [signal]);

  return (
    <div
      className="void-heartbeat-container"
      style={{
        width: size,
        height: size,
        position: 'relative',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      {/* CSS fallback (always rendered, hidden when WebGL active) */}
      <div
        className="void-heartbeat-css"
        style={{
          width: size * 0.6,
          height: size * 0.6,
          borderRadius: '50%',
          background: `radial-gradient(circle, ${coreColor} 0%, transparent 70%)`,
          animation: `void-pulse ${pulseSpeed}s ease-in-out infinite`,
          filter: `blur(${glowRadius}px)`,
          opacity: opacity,
          transition: 'opacity 0.5s ease',
          display: webglAvailable.current ? 'none' : 'block',
        }}
      />

      {/* WebGL2 canvas (overlays CSS when available) */}
      <canvas
        ref={canvasRef}
        style={{
          width: size,
          height: size,
          position: 'absolute',
          top: 0,
          left: 0,
          display: webglAvailable.current === false ? 'none' : 'block',
          pointerEvents: 'none',
        }}
      />

      {/* State label (subtle, below the glow) - hidden at small sizes */}
      {size >= 100 && (
        <span
          className="void-heartbeat-label"
          style={{
            position: 'absolute',
            bottom: 8,
            fontSize: 10,
            color: signal.error > 0.5 || signal.critical_count > 0 ? '#EF4444'
              : signal.signal_color_shift > 0.5 ? '#D4AF37'
              : signal.signal_color_shift < -0.3 ? '#4A90D9'
              : '#666666',
            letterSpacing: '0.1em',
            textTransform: 'uppercase',
            fontFamily: 'JetBrains Mono, monospace',
            opacity: 0.6,
            transition: 'color 0.3s ease',
          }}
        >
          {stateLabel}
        </span>
      )}
    </div>
  );
}

// ============================================================================
// WebGL2 Shader
// ============================================================================

const VERTEX_SHADER = `#version 300 es
in vec2 a_position;
out vec2 v_uv;
void main() {
  v_uv = a_position * 0.5 + 0.5;
  gl_Position = vec4(a_position, 0.0, 1.0);
}`;

// SDF sphere with simplex noise displacement
const FRAGMENT_SHADER = `#version 300 es
precision mediump float;

in vec2 v_uv;
out vec4 fragColor;

uniform float u_time;
uniform vec2 u_resolution;
uniform float u_pulse;
uniform float u_heat;
uniform float u_burst;
uniform float u_error;
uniform float u_staleness;
uniform float u_opacity;
uniform float u_signal_intensity;
uniform float u_signal_color_shift;
uniform int u_critical_count;

// Simplex-like noise (2D)
vec3 mod289(vec3 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec2 mod289(vec2 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec3 permute(vec3 x) { return mod289(((x*34.0)+1.0)*x); }

float snoise(vec2 v) {
  const vec4 C = vec4(0.211324865405187, 0.366025403784439,
                      -0.577350269189626, 0.024390243902439);
  vec2 i = floor(v + dot(v, C.yy));
  vec2 x0 = v - i + dot(i, C.xx);
  vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
  vec4 x12 = x0.xyxy + C.xxzz;
  x12.xy -= i1;
  i = mod289(i);
  vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
  vec3 m = max(0.5 - vec3(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), 0.0);
  m = m * m;
  m = m * m;
  vec3 x = 2.0 * fract(p * C.www) - 1.0;
  vec3 h = abs(x) - 0.5;
  vec3 ox = floor(x + 0.5);
  vec3 a0 = x - ox;
  m *= 1.79284291400159 - 0.85373472095314 * (a0*a0 + h*h);
  vec3 g;
  g.x = a0.x * x0.x + h.x * x0.y;
  g.yz = a0.yz * x12.xz + h.yz * x12.yw;
  return 130.0 * dot(m, g);
}

void main() {
  vec2 uv = v_uv * 2.0 - 1.0;
  float aspect = u_resolution.x / u_resolution.y;
  uv.x *= aspect;

  // Pulse speed: idle=slow, active=fast
  float speed = mix(0.25, 1.2, u_pulse);
  float t = u_time * speed;

  // SDF circle with noise displacement
  float dist = length(uv);
  float noise = snoise(uv * 2.0 + t * 0.3) * 0.15;
  float morph_noise = snoise(uv * 3.0 - t * 0.2) * 0.08;
  dist += noise + morph_noise;

  // Breathing: pulse the radius
  float breathe = sin(t * 3.14159) * 0.05 + 0.05;
  float radius = 0.3 + breathe;

  // Core glow
  float glow = smoothstep(radius + 0.3, radius - 0.1, dist);

  // Color: idle blue -> active gold, error red
  vec3 idle_color = vec3(0.102, 0.102, 0.243);   // #1a1a3e
  vec3 active_color = vec3(0.831, 0.686, 0.216);  // #D4AF37
  vec3 error_color = vec3(0.937, 0.267, 0.267);   // #EF4444

  vec3 color = mix(idle_color, active_color, u_heat);
  color = mix(color, error_color, u_error);

  // Signal color shift: blend toward cool blue or warm gold/red
  vec3 cool_color = vec3(0.102, 0.227, 0.431);    // #1a3a6e deep blue
  vec3 warm_color = vec3(0.831, 0.686, 0.216);     // #D4AF37 gold
  vec3 alert_color = vec3(0.937, 0.267, 0.267);    // #EF4444 red

  if (u_signal_color_shift < 0.0) {
    // Cool shift: blend toward deep blue
    color = mix(color, cool_color, -u_signal_color_shift * u_signal_intensity);
  } else if (u_signal_color_shift > 0.8) {
    // Hot shift: blend toward red for high alert
    float alert_t = (u_signal_color_shift - 0.8) / 0.2 * u_signal_intensity;
    color = mix(color, alert_color, alert_t);
  } else if (u_signal_color_shift > 0.0) {
    // Warm shift: blend toward gold
    color = mix(color, warm_color, u_signal_color_shift * u_signal_intensity);
  }

  // Stale: desaturate and dim
  float stale_dim = 1.0 - u_staleness * 0.7;
  color *= stale_dim;

  // Burst: additive flash
  float burst_flash = u_burst * exp(-dist * 4.0) * (0.5 + 0.5 * sin(u_time * 8.0));
  color += vec3(1.0, 0.9, 0.6) * burst_flash;

  // Critical double-pulse: add second pulse at 2x frequency
  if (u_critical_count > 0) {
    float double_pulse = sin(t * 6.28318 * 2.0) * 0.3;
    float critical_glow = smoothstep(radius + 0.2, radius - 0.05, dist);
    color += alert_color * max(double_pulse, 0.0) * critical_glow * u_signal_intensity;
  }

  // Final alpha
  float alpha = glow * u_opacity;

  fragColor = vec4(color * alpha, alpha);
}`;

function createShaderProgram(gl: WebGL2RenderingContext): WebGLProgram | null {
  const vs = gl.createShader(gl.VERTEX_SHADER);
  const fs = gl.createShader(gl.FRAGMENT_SHADER);
  if (!vs || !fs) return null;

  gl.shaderSource(vs, VERTEX_SHADER);
  gl.compileShader(vs);
  if (!gl.getShaderParameter(vs, gl.COMPILE_STATUS)) {
    console.warn('VoidHeartbeat: vertex shader error', gl.getShaderInfoLog(vs));
    return null;
  }

  gl.shaderSource(fs, FRAGMENT_SHADER);
  gl.compileShader(fs);
  if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
    console.warn('VoidHeartbeat: fragment shader error', gl.getShaderInfoLog(fs));
    return null;
  }

  const program = gl.createProgram();
  if (!program) return null;

  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);

  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.warn('VoidHeartbeat: link error', gl.getProgramInfoLog(program));
    return null;
  }

  gl.deleteShader(vs);
  gl.deleteShader(fs);

  // Enable blending for transparency
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

  return program;
}
