import { useEffect, useRef, useCallback, useState } from 'react';

/**
 * GPU-accelerated Turing pattern via WebGPU compute shaders.
 *
 * Gray-Scott reaction-diffusion → gold-on-dark cosine palette.
 * Bilinear sampling, radial vignette, smooth fade-in built into the shader.
 * Properly destroys all GPU resources on unmount.
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

const COMPUTE_WGSL = `
struct Params {
  feed: f32,
  kill: f32,
  da: f32,
  db: f32,
  w: u32,
  h: u32,
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> src: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read_write> dst: array<vec2<f32>>;

fn idx(x: i32, y: i32) -> u32 {
  let wx = u32((x + i32(p.w)) % i32(p.w));
  let wy = u32((y + i32(p.h)) % i32(p.h));
  return wy * p.w + wx;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= p.w || gid.y >= p.h) { return; }
  let x = i32(gid.x);
  let y = i32(gid.y);
  let i = idx(x, y);
  let ab = src[i];
  let a = ab.x;
  let b = ab.y;

  let lap =
    src[idx(x-1,y-1)]*0.05 + src[idx(x,y-1)]*0.2 + src[idx(x+1,y-1)]*0.05 +
    src[idx(x-1,y  )]*0.2  + src[idx(x,y  )]*-1.0 + src[idx(x+1,y  )]*0.2  +
    src[idx(x-1,y+1)]*0.05 + src[idx(x,y+1)]*0.2 + src[idx(x+1,y+1)]*0.05;

  let abb = a * b * b;
  let na = a + p.da * lap.x - abb + p.feed * (1.0 - a);
  let nb = b + p.db * lap.y + abb - (p.feed + p.kill) * b;

  dst[i] = clamp(vec2<f32>(na, nb), vec2<f32>(0.0), vec2<f32>(1.0));
}
`;

const RENDER_WGSL = `
struct Params {
  w: u32,
  h: u32,
  frame: f32,
  _pad: f32,
  resolution: vec2<f32>,
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> field: array<vec2<f32>>;

struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vid: u32) -> VSOut {
  var positions = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(3.0, -1.0),
    vec2<f32>(-1.0, 3.0),
  );
  var out: VSOut;
  out.pos = vec4<f32>(positions[vid], 0.0, 1.0);
  out.uv = positions[vid] * 0.5 + 0.5;
  return out;
}

// Bilinear sample from the field
fn sample_b(uv: vec2<f32>) -> f32 {
  let fx = uv.x * f32(p.w) - 0.5;
  let fy = uv.y * f32(p.h) - 0.5;
  let x0 = i32(floor(fx));
  let y0 = i32(floor(fy));
  let x1 = x0 + 1;
  let y1 = y0 + 1;
  let sx = fx - floor(fx);
  let sy = fy - floor(fy);

  let w = i32(p.w);
  let h = i32(p.h);
  let ix0 = ((x0 % w) + w) % w;
  let iy0 = ((y0 % h) + h) % h;
  let ix1 = ((x1 % w) + w) % w;
  let iy1 = ((y1 % h) + h) % h;

  let b00 = field[u32(iy0 * w + ix0)].y;
  let b10 = field[u32(iy0 * w + ix1)].y;
  let b01 = field[u32(iy1 * w + ix0)].y;
  let b11 = field[u32(iy1 * w + ix1)].y;

  return mix(mix(b00, b10, sx), mix(b01, b11, sx), sy);
}

@fragment
fn fs(input: VSOut) -> @location(0) vec4<f32> {
  // Aspect-correct UV mapping — fill the screen, crop excess
  let aspect_canvas = p.resolution.x / p.resolution.y;
  var uv = input.uv;
  if (aspect_canvas > 1.0) {
    // Wide screen: stretch vertically, crop horizontally
    uv.y = uv.y;
    uv.x = 0.5 + (uv.x - 0.5) / aspect_canvas;
  } else {
    uv.x = uv.x;
    uv.y = 0.5 + (uv.y - 0.5) * aspect_canvas;
  }

  // Out of bounds = black
  if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
    return vec4<f32>(0.02, 0.01, 0.02, 1.0);
  }

  let b = sample_b(vec2<f32>(uv.x, 1.0 - uv.y));

  // Gold-on-dark cosine palette
  // Maps chemical B concentration to a warm gold gradient
  let t = smoothstep(0.05, 0.5, b);

  // Base color: deep warm black → rich gold
  let col_dark = vec3<f32>(0.02, 0.012, 0.008);
  let col_mid  = vec3<f32>(0.45, 0.25, 0.05);
  let col_gold = vec3<f32>(0.85, 0.7, 0.22);
  let col_hot  = vec3<f32>(1.0, 0.92, 0.6);

  var col = col_dark;
  col = mix(col, col_mid,  smoothstep(0.0, 0.25, t));
  col = mix(col, col_gold, smoothstep(0.2, 0.6, t));
  col = mix(col, col_hot,  smoothstep(0.7, 1.0, t));

  // Subtle emission glow at high concentration
  let glow = smoothstep(0.4, 0.8, t) * 0.15;
  col += vec3<f32>(glow, glow * 0.7, glow * 0.15);

  // Radial vignette — smooth darkening toward edges
  let center = (input.uv - 0.5) * 2.0;
  let dist = length(center);
  let vignette = 1.0 - smoothstep(0.3, 1.4, dist);
  col *= vignette;

  // Fade in over first ~120 frames (~2 seconds at 60fps)
  let fade = clamp(p.frame / 120.0, 0.0, 1.0);
  col *= fade * fade; // ease-in

  return vec4<f32>(col, 1.0);
}
`;

interface TuringPatternProps {
  style?: React.CSSProperties;
  className?: string;
  /** Simulation grid (default 384 — good balance of detail vs perf) */
  gridSize?: number;
  /** Compute iterations per frame (default 6) */
  stepsPerFrame?: number;
  /** Feed rate (default 0.037 — coral-like fingers) */
  feed?: number;
  /** Kill rate (default 0.06) */
  kill?: number;
  /** Called when WebGPU is confirmed available and rendering */
  onReady?: () => void;
  /** Called if WebGPU is unavailable */
  onFallback?: () => void;
}

export function TuringPattern({
  style,
  className,
  gridSize = 384,
  stepsPerFrame = 6,
  feed = 0.037,
  kill = 0.06,
  onReady,
  onFallback,
}: TuringPatternProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const gpuRef = useRef<any>(null);
  const [ready, setReady] = useState(false);

  const cleanup = useCallback(() => {
    const g = gpuRef.current;
    if (g) {
      cancelAnimationFrame(g.raf);
      try { g.device.destroy(); } catch { /* already destroyed */ }
      gpuRef.current = null;
    }
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    let cancelled = false;

    (async () => {
      try {
        const gpu = (navigator as any).gpu;
        if (!gpu) { onFallback?.(); return; }

        const adapter = await gpu.requestAdapter();
        if (!adapter || cancelled) { if (!cancelled) onFallback?.(); return; }

        const device = await adapter.requestDevice();
        if (cancelled) { device.destroy(); return; }

        const context = canvas.getContext('webgpu') as any;
        if (!context) { device.destroy(); onFallback?.(); return; }

        const format = gpu.getPreferredCanvasFormat();
        context.configure({ device, format, alphaMode: 'opaque' });

        // Size canvas to container
        const resize = () => {
          const rect = canvas.getBoundingClientRect();
          const dpr = Math.min(window.devicePixelRatio || 1, 2);
          canvas.width = Math.round(rect.width * dpr);
          canvas.height = Math.round(rect.height * dpr);
        };
        resize();

        const ro = new ResizeObserver(resize);
        ro.observe(canvas);

        const W = gridSize;
        const H = gridSize;
        const cellCount = W * H;
        const bufSize = cellCount * 8; // vec2<f32> per cell

        // GPU buffer usage flags (avoid TypeScript global type issues)
        const UNIFORM = 0x0040, STORAGE = 0x0080, COPY_DST = 0x0008;

        // --- Compute pipeline ---
        const computeModule = device.createShaderModule({ code: COMPUTE_WGSL });
        const computePipeline = device.createComputePipeline({
          layout: 'auto',
          compute: { module: computeModule, entryPoint: 'main' },
        });

        // --- Render pipeline ---
        const renderModule = device.createShaderModule({ code: RENDER_WGSL });
        const renderPipeline = device.createRenderPipeline({
          layout: 'auto',
          vertex: { module: renderModule, entryPoint: 'vs' },
          fragment: { module: renderModule, entryPoint: 'fs', targets: [{ format }] },
          primitive: { topology: 'triangle-list' },
        });

        // --- Buffers ---
        const paramBuf = device.createBuffer({ size: 24, usage: UNIFORM | COPY_DST });
        // Render params: w, h, frame, _pad, resolution.x, resolution.y = 24 bytes
        const renderParamBuf = device.createBuffer({ size: 24, usage: UNIFORM | COPY_DST });
        const bufA = device.createBuffer({ size: bufSize, usage: STORAGE | COPY_DST });
        const bufB = device.createBuffer({ size: bufSize, usage: STORAGE });

        // --- Initialize field ---
        const init = new Float32Array(cellCount * 2);
        for (let i = 0; i < cellCount; i++) {
          init[i * 2] = 1.0; // chemical A = 1
          // chemical B = 0
        }

        // Seed: ring of points around center + central cluster
        // Creates the dramatic radial emergence pattern
        const cx = W / 2, cy = H / 2;
        const seedPoint = (sx: number, sy: number, r: number) => {
          for (let dy = -r; dy <= r; dy++) {
            for (let dx = -r; dx <= r; dx++) {
              if (dx * dx + dy * dy > r * r) continue; // circular seed
              const px = ((sx + dx) % W + W) % W;
              const py = ((sy + dy) % H + H) % H;
              init[(py * W + px) * 2 + 1] = 1.0;
            }
          }
        };

        // Ring of 30 seeds
        for (let s = 0; s < 30; s++) {
          const angle = (s / 30) * Math.PI * 2 + Math.random() * 0.2;
          const r = W * 0.18 + Math.random() * W * 0.12;
          seedPoint(
            Math.floor(cx + Math.cos(angle) * r),
            Math.floor(cy + Math.sin(angle) * r),
            3,
          );
        }
        // Center cluster
        seedPoint(Math.floor(cx), Math.floor(cy), 5);
        // A few random outliers for variety
        for (let s = 0; s < 8; s++) {
          seedPoint(
            Math.floor(Math.random() * W),
            Math.floor(Math.random() * H),
            2,
          );
        }

        device.queue.writeBuffer(bufA, 0, init);

        const state = { device, raf: 0, ro };
        gpuRef.current = state;

        let currentA = bufA;
        let currentB = bufB;
        let frameCount = 0;

        setReady(true);
        onReady?.();

        const frame = () => {
          if (cancelled) return;

          // Write compute params
          const params = new ArrayBuffer(24);
          const f32 = new Float32Array(params);
          const u32 = new Uint32Array(params);
          f32[0] = feed; f32[1] = kill;
          f32[2] = 1.0; f32[3] = 0.5;
          u32[4] = W; u32[5] = H;
          device.queue.writeBuffer(paramBuf, 0, params);

          // Write render params
          const rParams = new ArrayBuffer(24);
          const rU32 = new Uint32Array(rParams);
          const rF32 = new Float32Array(rParams);
          rU32[0] = W; rU32[1] = H;
          rF32[2] = frameCount; rF32[3] = 0;
          rF32[4] = canvas.width; rF32[5] = canvas.height;
          device.queue.writeBuffer(renderParamBuf, 0, rParams);

          const enc = device.createCommandEncoder();

          // Compute: advance simulation
          for (let s = 0; s < stepsPerFrame; s++) {
            const bg = device.createBindGroup({
              layout: computePipeline.getBindGroupLayout(0),
              entries: [
                { binding: 0, resource: { buffer: paramBuf } },
                { binding: 1, resource: { buffer: currentA } },
                { binding: 2, resource: { buffer: currentB } },
              ],
            });
            const pass = enc.beginComputePass();
            pass.setPipeline(computePipeline);
            pass.setBindGroup(0, bg);
            pass.dispatchWorkgroups(Math.ceil(W / 8), Math.ceil(H / 8));
            pass.end();
            [currentA, currentB] = [currentB, currentA];
          }

          // Render: visualize field
          const renderBG = device.createBindGroup({
            layout: renderPipeline.getBindGroupLayout(0),
            entries: [
              { binding: 0, resource: { buffer: renderParamBuf } },
              { binding: 1, resource: { buffer: currentA } },
            ],
          });

          const rp = enc.beginRenderPass({
            colorAttachments: [{
              view: context.getCurrentTexture().createView(),
              loadOp: 'clear',
              storeOp: 'store',
              clearValue: { r: 0.02, g: 0.012, b: 0.008, a: 1 },
            }],
          });
          rp.setPipeline(renderPipeline);
          rp.setBindGroup(0, renderBG);
          rp.draw(3);
          rp.end();

          device.queue.submit([enc.finish()]);
          frameCount++;
          state.raf = requestAnimationFrame(frame);
        };

        state.raf = requestAnimationFrame(frame);
      } catch (err) {
        console.warn('[TuringPattern] WebGPU init failed:', err);
        if (!cancelled) onFallback?.();
      }
    })();

    return () => {
      cancelled = true;
      const g = gpuRef.current;
      if (g?.ro) g.ro.disconnect();
      cleanup();
    };
  }, [gridSize, stepsPerFrame, feed, kill, cleanup, onReady, onFallback]);

  return (
    <canvas
      ref={canvasRef}
      className={className}
      style={{
        display: 'block',
        width: '100%',
        height: '100%',
        opacity: ready ? 1 : 0,
        transition: 'opacity 500ms ease-in',
        ...style,
      }}
    />
  );
}
