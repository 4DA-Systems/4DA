// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// G6 idle-resource sampler for scripts/launch-gates.mjs (Windows only).
//
// Read-only. It never changes settings, never quits or relaunches the app:
//   * CPU: Win32_Process kernel+user time for the GUI process and its whole
//     descendant tree (WebView2 renderers/GPU process), plus any
//     fourda-engine.exe, sampled every `intervalSec`.
//   * GPU: `nvidia-smi pmon -c 1 -s u` SM% for the same pids (if available).
//   * Idle detection: the app animates its mark while a background cycle runs,
//     so an interval counts as IDLE only when `document.getAnimations().length`
//     (via the embedded Victauri bridge, eval_js) has been 0 for >= `holdSec`
//     at the interval start and is still 0 at its end. Everything else is
//     reported as in-cycle.
// The pure classification (`summarise`) is exported for unit tests.

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const HUNDRED_NS_PER_SEC = 1e7;

/** One Win32_Process snapshot: [{pid, ppid, name, path, cpu100ns}]. */
export function snapshotProcesses() {
  const ps =
    "Get-CimInstance Win32_Process | ForEach-Object { '{0}|{1}|{2}|{3}|{4}' -f " +
    '$_.ProcessId,$_.ParentProcessId,$_.Name,($_.KernelModeTime + $_.UserModeTime),$_.ExecutablePath }';
  const out = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', ps], {
    encoding: 'utf8',
    windowsHide: true,
    maxBuffer: 16 * 1024 * 1024,
  });
  return out
    .split(/\r?\n/)
    .filter(Boolean)
    .map((l) => {
      const [pid, ppid, name, cpu, ...rest] = l.split('|');
      return { pid: +pid, ppid: +ppid, name, cpu100ns: Number(cpu) || 0, path: rest.join('|') };
    });
}

/** pid plus every descendant pid, from a process snapshot. */
export function descendants(procs, rootPid) {
  const kids = new Map();
  for (const p of procs) {
    if (!kids.has(p.ppid)) kids.set(p.ppid, []);
    kids.get(p.ppid).push(p.pid);
  }
  const seen = new Set([rootPid]);
  const stack = [rootPid];
  while (stack.length) {
    for (const c of kids.get(stack.pop()) || []) {
      if (!seen.has(c) && c !== 0) {
        seen.add(c);
        stack.push(c);
      }
    }
  }
  return seen;
}

/** Find the GUI pid launched from `<appRoot>\src-tauri\target`. */
export function findGuiPid(procs, appRoot) {
  const want = join(appRoot, 'src-tauri', 'target').toLowerCase();
  const gui = procs.find(
    (p) => p.name.toLowerCase() === 'fourda.exe' && (p.path || '').toLowerCase().startsWith(want),
  );
  return gui ? gui.pid : null;
}

/** Victauri credentials published by the GUI process. */
export function victauriFor(pid) {
  const dir = join(process.env.TEMP || tmpdir(), 'victauri', String(pid));
  if (!existsSync(join(dir, 'token'))) return null;
  return {
    token: readFileSync(join(dir, 'token'), 'utf8').trim(),
    port: Number(readFileSync(join(dir, 'port'), 'utf8').trim()) || 7373,
  };
}

/** document.getAnimations().length in the main webview, or null if unreachable. */
export async function animationCount(vic) {
  if (!vic) return null;
  try {
    const res = await fetch(`http://127.0.0.1:${vic.port}/api/tools/eval_js`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${vic.token}` },
      body: JSON.stringify({ code: 'document.getAnimations().length' }),
      signal: AbortSignal.timeout(15000),
    });
    const j = await res.json();
    return typeof j.result === 'number' ? j.result : Number(j.result);
  } catch {
    return null;
  }
}

/** SM% per pid from nvidia-smi pmon; null when nvidia-smi is absent. */
export function gpuSmByPid() {
  try {
    const out = execFileSync('nvidia-smi', ['pmon', '-c', '1', '-s', 'u'], {
      encoding: 'utf8',
      windowsHide: true,
      timeout: 15000,
    });
    const m = new Map();
    for (const line of out.split(/\r?\n/)) {
      if (line.startsWith('#') || !line.trim()) continue;
      const cols = line.trim().split(/\s+/);
      const sm = Number(cols[3]);
      m.set(Number(cols[1]), Number.isFinite(sm) ? sm : 0);
    }
    return m;
  } catch {
    return null;
  }
}

/**
 * Classify samples into idle / in-cycle windows and compute % of one core.
 * samples: [{t (ms), app100ns, engine100ns, ollama100ns, anim (number|null), gpuSm}]
 */
export function summarise(samples, holdSec = 120) {
  const acc = {
    idle: { wallSec: 0, app: 0, engine: 0, ollama: 0, gpu: [] },
    cycle: { wallSec: 0, app: 0, engine: 0, ollama: 0, gpu: [] },
    unknown: { wallSec: 0, app: 0, engine: 0, ollama: 0, gpu: [] },
  };
  annotateQuiet(samples);
  for (let i = 1; i < samples.length; i++) {
    const s = samples[i];
    const prev = samples[i - 1];
    const prevQuietSince = prev.quietSince;
    let bucket;
    if (prev.anim == null || s.anim == null) bucket = 'unknown';
    else if (prev.anim === 0 && s.anim === 0 && prevQuietSince != null && prev.t - prevQuietSince >= holdSec * 1000)
      bucket = 'idle';
    else bucket = 'cycle';
    const b = acc[bucket];
    const dt = (s.t - prev.t) / 1000;
    b.wallSec += dt;
    b.app += Math.max(0, s.app100ns - prev.app100ns) / HUNDRED_NS_PER_SEC;
    b.engine += Math.max(0, s.engine100ns - prev.engine100ns) / HUNDRED_NS_PER_SEC;
    b.ollama += Math.max(0, s.ollama100ns - prev.ollama100ns) / HUNDRED_NS_PER_SEC;
    if (s.gpuSm != null) b.gpu.push(s.gpuSm);
  }
  const pct = (cpuSec, wall) => (wall > 0 ? (100 * cpuSec) / wall : null);
  const out = {};
  for (const [k, b] of Object.entries(acc)) {
    out[k] = {
      wallSec: Math.round(b.wallSec),
      appPctCore: pct(b.app, b.wallSec),
      enginePctCore: pct(b.engine, b.wallSec),
      ollamaPctCore: pct(b.ollama, b.wallSec),
      gpuSmMean: b.gpu.length ? b.gpu.reduce((a, x) => a + x, 0) / b.gpu.length : null,
    };
  }
  return out;
}

/** Pre-pass so `summarise` can read quietSince of the previous sample. */
export function annotateQuiet(samples) {
  let quietSince = null;
  for (const s of samples) {
    if (s.anim === 0) quietSince ??= s.t;
    else quietSince = null;
    s.quietSince = quietSince;
  }
  return samples;
}

/** Run the live sampler. Returns { guiPid, samples, summary }. */
export async function measureIdle({ appRoot, minutes = 12, intervalSec = 10, holdSec = 120, log = () => {} }) {
  if (process.platform !== 'win32') throw new Error('idle sampler is Windows-only (Win32_Process)');
  let procs = snapshotProcesses();
  const guiPid = findGuiPid(procs, appRoot);
  if (!guiPid) throw new Error(`no fourda.exe running from ${appRoot}\\src-tauri\\target`);
  const vic = victauriFor(guiPid);
  const samples = [];
  const end = Date.now() + minutes * 60_000;
  while (true) {
    const t = Date.now();
    procs = snapshotProcesses();
    if (!procs.some((p) => p.pid === guiPid)) throw new Error(`GUI pid ${guiPid} exited during sampling`);
    const tree = descendants(procs, guiPid);
    const sum = (pred) => procs.filter(pred).reduce((a, p) => a + p.cpu100ns, 0);
    const app100ns = sum((p) => tree.has(p.pid));
    const enginePids = procs.filter((p) => /^fourda-engine/i.test(p.name)).map((p) => p.pid);
    const engine100ns = sum((p) => enginePids.includes(p.pid));
    const ollama100ns = sum((p) => /^(ollama|llama-server)/i.test(p.name) && !/app/i.test(p.name));
    const gpu = gpuSmByPid();
    const gpuSm = gpu ? [...tree, ...enginePids].reduce((a, pid) => a + (gpu.get(pid) || 0), 0) : null;
    const anim = await animationCount(vic);
    samples.push({ t, app100ns, engine100ns, ollama100ns, anim, gpuSm, engines: enginePids.length });
    log(`${new Date(t).toISOString()} anim=${anim} engines=${enginePids.length} gpuSm=${gpuSm}`);
    if (t >= end) break;
    await new Promise((r) => setTimeout(r, Math.max(0, intervalSec * 1000 - (Date.now() - t))));
  }
  return { guiPid, samples, summary: summarise(samples, holdSec) };
}

/** Latest `victauri/<pid>` dirs are not cleaned; list live ones (diagnostic helper). */
export function victauriPids() {
  const root = join(process.env.TEMP || tmpdir(), 'victauri');
  return existsSync(root) ? readdirSync(root).map(Number).filter(Boolean) : [];
}
