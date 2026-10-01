// SPDX-License-Identifier: Apache-2.0
/**
 * Python and Go lockfile parsers (split out of lockfile-parsers.ts).
 *
 * Measured 2026-10-02 against osv-scanner and pip-audit, every miss below was
 * a parsing loss before OSV was ever asked:
 * - requirements.txt: `urllib3[secure]==1.25.0` and dotted names were skipped,
 *   and an inline comment rode along as part of the version — OSV was asked
 *   about `"8.0.0  # imaging"` and answered with 63 advisories for the real 45.
 * - poetry.lock: the reader required `name` and `version` on consecutive
 *   lines. Poetry before 1.0 wrote its keys alphabetically (`category`,
 *   `description`, `name`, ..., `version`), so a whole lockfile read as empty.
 * - go.sum: `(v[\d.]+)` cut `v0.0.0-20200622213623-75b288015ac9` to `v0.0.0`
 *   and dropped `+incompatible`; `v0.0.0` then sorted AFTER the real
 *   pseudo-version, so older advisories were missed and later ones invented.
 *   go.sum also lists versions the build never selected.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { emptySource, found, InstanceSet, type VersionSource } from "./lockfile-types.js";

// =============================================================================
// Python: poetry.lock > uv.lock > Pipfile.lock > requirements.txt
// =============================================================================

/** PEP 503 normalised name: lowercase, runs of `-`, `_`, `.` become one `-`. */
export function normalizePythonName(name: string): string {
  return name.trim().toLowerCase().replace(/[-_.]+/g, "-");
}

export function resolvePython(cwd: string): VersionSource {
  // poetry.lock and uv.lock are TOML arrays of [[package]] tables.
  for (const file of ["poetry.lock", "uv.lock"]) {
    const lockPath = path.join(cwd, file);
    if (!fs.existsSync(lockPath)) continue;
    try {
      const read = readTomlPackageLock(fs.readFileSync(lockPath, "utf-8"));
      if (read.instances.size > 0) return found(read.versions, read.instances, lockPath, "lockfile");
    } catch { /* fall through */ }
  }

  const pipfileLock = path.join(cwd, "Pipfile.lock");
  if (fs.existsSync(pipfileLock)) {
    try {
      const lock = JSON.parse(fs.readFileSync(pipfileLock, "utf-8"));
      const versions = new Map<string, string>();
      const instances = new InstanceSet();
      for (const [section, dev] of [["default", false], ["develop", true]] as const) {
        for (const [name, info] of Object.entries(lock[section] ?? {})) {
          const version = (info as { version?: string }).version?.replace(/^===?/, "");
          if (!version) continue;
          const normalized = normalizePythonName(name);
          instances.add(normalized, version, dev);
          if (!versions.has(normalized)) versions.set(normalized, version);
        }
      }
      if (instances.size > 0) return found(versions, instances, pipfileLock, "lockfile");
    } catch { /* fall through */ }
  }

  const reqPath = path.join(cwd, "requirements.txt");
  if (fs.existsSync(reqPath)) {
    const versions = new Map<string, string>();
    const instances = new InstanceSet();
    try {
      for (const { name, version } of parseRequirements(fs.readFileSync(reqPath, "utf-8"))) {
        instances.add(name, version);
        if (!versions.has(name)) versions.set(name, version);
      }
    } catch { /* skip */ }
    return found(versions, instances, reqPath, "manifest");
  }

  return emptySource();
}

/**
 * [[package]] tables from poetry.lock (any version) or uv.lock. A pre-1.2
 * poetry.lock records `category = "dev"` for dev-only packages; newer files
 * and uv.lock do not record scope here, so it stays unknown.
 */
export function readTomlPackageLock(content: string): { versions: Map<string, string>; instances: InstanceSet } {
  const versions = new Map<string, string>();
  const instances = new InstanceSet();
  for (const block of content.split(/^\[\[package\]\]\s*$/m).slice(1)) {
    // Stop at the next table header (e.g. [package.dependencies], [metadata]).
    const body = block.split(/^\[/m)[0];
    const field = (key: string) => new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, "m").exec(body)?.[1] ?? null;
    const name = field("name");
    const version = field("version");
    if (!name || !version) continue;
    // uv.lock lists the project itself (source = { editable/virtual = "." }); it is not a release.
    if (/^source\s*=\s*\{\s*(?:editable|virtual)\s*=/m.test(body)) continue;
    const category = field("category");
    const normalized = normalizePythonName(name);
    instances.add(normalized, version, category === null ? undefined : category === "dev");
    if (!versions.has(normalized)) versions.set(normalized, version);
  }
  return { versions, instances };
}

/**
 * Exact pins from a requirements file: `name==1.2`, `name===1.2`,
 * `name[extra1,extra2]==1.2`, with environment markers and inline comments.
 * Ranges (`>=`, `~=`) do not name an installed version and are skipped, as are
 * options (`-r`, `-e`, `--hash`) and URL requirements.
 */
export function parseRequirements(content: string): Array<{ name: string; version: string }> {
  const out: Array<{ name: string; version: string }> = [];
  // Backslash continuations join physical lines into one requirement.
  const logical = content.replace(/\\\r?\n/g, " ").split(/\r?\n/);
  for (const raw of logical) {
    // A `#` starts a comment at line start or after whitespace (PEP 508 URLs may contain `#`).
    const line = raw.replace(/(^|\s)#.*$/, "").trim();
    if (!line || line.startsWith("-")) continue;
    // Per-requirement options (pip-compile writes `--hash=sha256:...` on
    // continuation lines) follow the specifier; they are not part of it.
    const spec = line.split(";")[0].replace(/\s--?[A-Za-z][\w-]*(?:[=\s]\S+)?/g, "").trim();
    const match = /^([A-Za-z0-9][A-Za-z0-9._-]*)\s*(?:\[[^\]]*\])?\s*===?\s*([^\s,;]+)\s*$/.exec(spec);
    if (!match) continue;
    out.push({ name: normalizePythonName(match[1]), version: match[2] });
  }
  return out;
}

// =============================================================================
// Go: go.mod (the selected module set) completed from go.sum for go < 1.17
// =============================================================================

/** A full Go module version: release, prerelease, pseudo-version, `+incompatible`. */
const GO_VERSION = /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

export function resolveGo(cwd: string): VersionSource {
  const goModPath = path.join(cwd, "go.mod");
  if (!fs.existsSync(goModPath)) return emptySource();

  let mod: ReturnType<typeof parseGoMod>;
  try {
    mod = parseGoMod(fs.readFileSync(goModPath, "utf-8"));
  } catch {
    return emptySource();
  }
  const versions = new Map<string, string>();
  const instances = new InstanceSet();
  for (const { module, version } of mod.requires) {
    versions.set(module, version);
    instances.add(module, version);
  }

  // From go 1.17 the go.mod `require` list names every module the build needs
  // (module-graph pruning), so it IS the selected set and go.sum (which keeps
  // hashes for versions the build never selects) would only over-report.
  // Before 1.17 go.mod lists direct requirements only; transitives come from
  // go.sum, at the highest version whose module zip is hashed there — what
  // minimal version selection picked.
  if (!mod.goVersionAtLeast117) {
    const goSumPath = path.join(cwd, "go.sum");
    if (fs.existsSync(goSumPath)) {
      try {
        for (const [module, version] of selectedFromGoSum(fs.readFileSync(goSumPath, "utf-8"))) {
          if (versions.has(module)) continue;
          versions.set(module, version);
          instances.add(module, version);
        }
      } catch { /* skip */ }
    }
  }
  return found(versions, instances, goModPath, "lockfile");
}

/** `require` lines (block and single form) with `replace` applied, and whether the go directive is >= 1.17. */
export function parseGoMod(content: string): {
  requires: Array<{ module: string; version: string; indirect: boolean }>;
  goVersionAtLeast117: boolean;
} {
  const requires = new Map<string, { module: string; version: string; indirect: boolean }>();
  const replaces = new Map<string, { module: string; version: string } | null>();
  let block: "require" | "replace" | null = null;

  for (const rawLine of content.split(/\r?\n/)) {
    const indirect = /\/\/\s*indirect\b/.test(rawLine);
    const line = rawLine.replace(/\/\/.*$/, "").trim();
    if (!line) continue;
    if (block && line === ")") {
      block = null;
      continue;
    }
    const opener = /^(require|replace)\s*\($/.exec(line);
    if (opener) {
      block = opener[1] as "require" | "replace";
      continue;
    }
    const single = /^(require|replace)\s+(.+)$/.exec(line);
    const kind = single ? (single[1] as "require" | "replace") : block;
    const rest = single ? single[2] : line;
    if (kind === "require") {
      const [module, version] = rest.split(/\s+/);
      if (module && version && GO_VERSION.test(version)) requires.set(module, { module, version, indirect });
    } else if (kind === "replace") {
      // `old [v] => new v` replaces with another module version; `=> ./path` is local code.
      const [left, right] = rest.split("=>").map((s) => s.trim().split(/\s+/));
      if (!left?.[0] || !right?.[0]) continue;
      replaces.set(left[0], right[1] && GO_VERSION.test(right[1]) ? { module: right[0], version: right[1] } : null);
    }
  }

  const result: Array<{ module: string; version: string; indirect: boolean }> = [];
  for (const req of requires.values()) {
    if (!replaces.has(req.module)) {
      result.push(req);
      continue;
    }
    const target = replaces.get(req.module);
    if (target) result.push({ module: target.module, version: target.version, indirect: req.indirect });
  }

  const go = /^go\s+(\d+)\.(\d+)/m.exec(content);
  const goVersionAtLeast117 = go ? Number(go[1]) > 1 || Number(go[2]) >= 17 : false;
  return { requires: result, goVersionAtLeast117 };
}

/** Per module, the highest version whose module zip (not just its go.mod) is hashed in go.sum. */
export function selectedFromGoSum(content: string): Map<string, string> {
  const selected = new Map<string, string>();
  for (const line of content.split(/\r?\n/)) {
    const [module, version] = line.trim().split(/\s+/);
    if (!module || !version || version.endsWith("/go.mod") || !GO_VERSION.test(version)) continue;
    const current = selected.get(module);
    if (!current || compareGoVersions(version, current) > 0) selected.set(module, version);
  }
  return selected;
}

/** Semver precedence over Go versions; build metadata (`+incompatible`) is ignored. */
export function compareGoVersions(a: string, b: string): number {
  const parse = (v: string) => {
    const m = /^v(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?/.exec(v);
    return m ? { nums: [Number(m[1]), Number(m[2]), Number(m[3])], pre: m[4] ? m[4].split(".") : [] } : null;
  };
  const pa = parse(a);
  const pb = parse(b);
  if (!pa || !pb) return 0;
  for (let i = 0; i < 3; i++) if (pa.nums[i] !== pb.nums[i]) return pa.nums[i] > pb.nums[i] ? 1 : -1;
  if (pa.pre.length === 0 || pb.pre.length === 0) return Math.sign(pb.pre.length - pa.pre.length);
  for (let i = 0; i < Math.min(pa.pre.length, pb.pre.length); i++) {
    const x = pa.pre[i];
    const y = pb.pre[i];
    if (x === y) continue;
    const xn = /^\d+$/.test(x);
    const yn = /^\d+$/.test(y);
    if (xn && yn) return BigInt(x) > BigInt(y) ? 1 : -1;
    if (xn !== yn) return xn ? -1 : 1;
    return x > y ? 1 : -1;
  }
  return Math.sign(pa.pre.length - pb.pre.length);
}
