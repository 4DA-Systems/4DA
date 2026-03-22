/**
 * WGSL shader source strings for the Turing pattern simulation.
 *
 * Gray-Scott reaction-diffusion compute shader + gold-on-dark cosine palette renderer.
 */

export const COMPUTE_WGSL = `
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

export const RENDER_WGSL = `
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
  let t = smoothstep(0.05, 0.5, b);

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

  // Radial vignette
  let center = (input.uv - 0.5) * 2.0;
  let dist = length(center);
  let vignette = 1.0 - smoothstep(0.3, 1.4, dist);
  col *= vignette;

  // Fade in over first ~120 frames
  let fade = clamp(p.frame / 120.0, 0.0, 1.0);
  col *= fade * fade;

  return vec4<f32>(col, 1.0);
}
`;
