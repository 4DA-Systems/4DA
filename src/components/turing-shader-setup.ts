/**
 * Turing pattern WebGPU initialization and frame-loop utilities.
 *
 * Shader sources live in turing-shaders.ts.
 * Pure functions — no React dependency.
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

import { COMPUTE_WGSL, RENDER_WGSL } from './turing-shaders';

// ============================================================================
// GPU buffer usage flags (avoid TypeScript global type issues)
// ============================================================================

const UNIFORM = 0x0040;
const STORAGE = 0x0080;
const COPY_DST = 0x0008;

// ============================================================================
// WebGPU Initialization
// ============================================================================

export interface TuringGPUState {
  device: any;
  context: any;
  computePipeline: any;
  renderPipeline: any;
  paramBuf: any;
  renderParamBuf: any;
  bufA: any;
  bufB: any;
  ro: ResizeObserver;
  W: number;
  H: number;
}

/**
 * Initialize WebGPU device, pipelines, buffers, and seed the simulation field.
 * Returns null if WebGPU is unavailable.
 */
export async function initTuringGPU(
  canvas: HTMLCanvasElement,
  gridSize: number,
): Promise<TuringGPUState | null> {
  const gpu = (navigator as any).gpu;
  if (!gpu) return null;

  const adapter = await gpu.requestAdapter();
  if (!adapter) return null;

  const device = await adapter.requestDevice();

  const context = canvas.getContext('webgpu') as any;
  if (!context) {
    device.destroy();
    return null;
  }

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
  const cx = W / 2, cy = H / 2;
  const seedPoint = (sx: number, sy: number, r: number) => {
    for (let dy = -r; dy <= r; dy++) {
      for (let dx = -r; dx <= r; dx++) {
        if (dx * dx + dy * dy > r * r) continue;
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

  return {
    device,
    context,
    computePipeline,
    renderPipeline,
    paramBuf,
    renderParamBuf,
    bufA,
    bufB,
    ro,
    W,
    H,
  };
}

/**
 * Run one animation frame: N compute steps + 1 render pass.
 * Returns the swapped buffer pair [currentA, currentB].
 */
export function turingFrame(
  gpu: TuringGPUState,
  canvas: HTMLCanvasElement,
  feed: number,
  kill: number,
  stepsPerFrame: number,
  frameCount: number,
  currentA: any,
  currentB: any,
): [any, any] {
  const { device, context, computePipeline, renderPipeline, paramBuf, renderParamBuf, W, H } = gpu;

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
  let a = currentA;
  let b = currentB;
  for (let s = 0; s < stepsPerFrame; s++) {
    const bg = device.createBindGroup({
      layout: computePipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: paramBuf } },
        { binding: 1, resource: { buffer: a } },
        { binding: 2, resource: { buffer: b } },
      ],
    });
    const pass = enc.beginComputePass();
    pass.setPipeline(computePipeline);
    pass.setBindGroup(0, bg);
    pass.dispatchWorkgroups(Math.ceil(W / 8), Math.ceil(H / 8));
    pass.end();
    [a, b] = [b, a];
  }

  // Render: visualize field
  const renderBG = device.createBindGroup({
    layout: renderPipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: renderParamBuf } },
      { binding: 1, resource: { buffer: a } },
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

  return [a, b];
}
