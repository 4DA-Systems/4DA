#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Dependency-engine conformance corpus — regeneration tool.
 *
 * The corpus lives in src-tauri/tests/conformance/ (format: README.md there).
 * CI never runs this script: it reads only the checked-in corpus, offline.
 * This script rebuilds that corpus from the network, in five idempotent steps:
 *
 *   fetch   sparse, blob-filtered clone of each case's repository at its pinned
 *           commit into the cache (only the files the case vendors)
 *   vendor  copy lockfiles + manifests into cases/<id>/ (LF-normalised)
 *   scan    osv-scanner --all-packages over each case dir; `go list -m all`
 *           (Go's own MVS build list) for every go.mod
 *   pin     every OSV record that names a package in the truth or engine
 *           inventory, as served by api.osv.dev/v1/vulns/<id> -> osv/<id>.json
 *   truth   expected/<id>.json = adjudicated inventory x OSV's own evaluator
 *           (/v1/querybatch with exact versions), grouped by alias
 *
 *   node scripts/conformance-corpus.mjs <step|all> [--cache DIR]
 *        [--osv-scanner PATH] [--engine-dump DIR] [--only id,id]
 *
 * --engine-dump points at the directory the Rust harness writes when run with
 * FOURDA_CONFORMANCE_DUMP=<dir>; `truth` uses it to list every disagreement
 * between the engine and the truth (report.json in the cache) and `pin` uses
 * its inventory so engine-only packages also get their advisories pinned.
 * Truth corrections live in adjudications.json (reviewed, checked in).
 */
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, posix, relative, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");
const CORPUS = join(ROOT, "src-tauri", "tests", "conformance");
const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : dflt;
};
const CACHE = resolve(opt("cache", join(tmpdir(), "4da-conformance-cache")));
const OSV_SCANNER = opt("osv-scanner", "osv-scanner");
const ENGINE_DUMP = opt("engine-dump", null);
const ONLY = opt("only", null)?.split(",");

// ---------------------------------------------------------------------------
// Shared vocabulary (mirrored in src-tauri/src/osv/conformance_tests.rs)
// ---------------------------------------------------------------------------

/** Files that hold (or claim to hold) an installed dependency set. */
export function isLockfileLike(relPath) {
  const base = posix.basename(relPath);
  if (/^(package-lock\.json|npm-shrinkwrap\.json|pnpm-lock\.yaml|yarn\.lock|bun\.lockb?|Cargo\.lock|poetry\.lock|uv\.lock|Pipfile\.lock|go\.mod|go\.sum|Gemfile\.lock|composer\.lock|pom\.xml|gradle\.lockfile)$/.test(base)) return true;
  if (/requirements.*\.txt$/i.test(base)) return true;
  return /(^|\/)requirements\/[^/]+\.txt$/i.test(relPath);
}

const ECOSYSTEM_OF = [
  [/^(package-lock\.json|npm-shrinkwrap\.json|pnpm-lock\.yaml|yarn\.lock|bun\.lockb?)$/, "npm"],
  [/^Cargo\.lock$/, "crates.io"],
  [/^(poetry\.lock|uv\.lock|Pipfile\.lock|.*\.txt)$/, "PyPI"],
  [/^go\.(mod|sum)$/, "Go"],
  [/^Gemfile\.lock$/, "RubyGems"],
  [/^composer\.lock$/, "Packagist"],
  [/^(pom\.xml|gradle\.lockfile)$/, "Maven"],
];
export const ecosystemOf = (relPath) => ECOSYSTEM_OF.find(([re]) => re.test(posix.basename(relPath)))?.[1] ?? "unknown";

/** Comparison key normalisation — identical rules in the Rust harness. */
export function normName(eco, name) {
  if (eco === "PyPI") return name.toLowerCase().replace(/[-_.]+/g, "-");
  if (eco === "Go") return name;
  return name.toLowerCase();
}
export function normVersion(eco, version) {
  let v = String(version ?? "").trim();
  if (eco === "Go" || eco === "Packagist") v = v.replace(/^v(?=\d)/, "");
  if (eco === "PyPI") {
    v = v.toLowerCase().replace(/^v/, "");
    const m = v.match(/^(\d+(?:\.\d+)*)(.*)$/);
    if (m) v = m[1].replace(/(\.0+)+$/, "").replace(/^$/, "0") + m[2];
  }
  return v;
}
/**
 * The project directory a lockfile belongs to. A requirements file inside a
 * `requirements/` directory (pip-compile layout) belongs to the project above it.
 */
export function ownerDir(rel) {
  const dir = posix.dirname(rel);
  if (posix.basename(dir).toLowerCase() === "requirements" && rel.endsWith(".txt")) return posix.dirname(dir);
  return dir;
}
const invKey = (eco, dir, name, version) => `${eco}|${dir}|${normName(eco, name)}|${normVersion(eco, version)}`;

function formatOf(abs, relPath) {
  const base = posix.basename(relPath);
  const head = () => readFileSync(abs, "utf8").slice(0, 4000);
  switch (base) {
    case "package-lock.json":
    case "npm-shrinkwrap.json":
      return `npm-v${JSON.parse(readFileSync(abs, "utf8")).lockfileVersion ?? 1}`;
    case "pnpm-lock.yaml":
      return `pnpm-v${(head().match(/lockfileVersion:\s*'?([\d.]+)/) || [])[1]}`;
    case "yarn.lock":
      return head().includes("__metadata:") ? "yarn-berry" : "yarn-v1";
    case "bun.lock": return "bun-text";
    case "bun.lockb": return "bun-binary";
    case "go.mod": return `go.mod-go${(readFileSync(abs, "utf8").match(/^go\s+([\d.]+)/m) || [])[1]}`;
    default: return base.endsWith(".txt") ? "requirements" : base;
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const readJson = (p, dflt) => (existsSync(p) ? JSON.parse(readFileSync(p, "utf8")) : dflt);
const writeJson = (p, v, pretty = true) => {
  mkdirSync(dirname(p), { recursive: true });
  writeFileSync(p, (pretty ? JSON.stringify(v, null, 1) : JSON.stringify(v)) + "\n");
};
const run = (cmd, argv, cwd, extra = {}) => spawnSync(cmd, argv, { cwd, encoding: "utf8", maxBuffer: 1 << 30, ...extra });
const git = (cwd, ...a) => run("git", ["-c", "core.longpaths=true", ...a], cwd);

function loadCases() {
  const ids = readdirSync(join(CORPUS, "cases")).filter((d) => existsSync(join(CORPUS, "cases", d, "case.json")));
  return ids.filter((id) => !ONLY || ONLY.includes(id)).map((id) => readJson(join(CORPUS, "cases", id, "case.json")));
}

function walkFiles(dir, base = dir, out = []) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walkFiles(p, base, out);
    else out.push(relative(base, p).replace(/\\/g, "/"));
  }
  return out;
}

/** Simple glob for workspace patterns: `a/*`, `a/**`, exact paths. */
function globToRegex(pattern) {
  const esc = pattern.replace(/\/+$/, "").replace(/[.+^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`^${esc.replace(/\*\*/g, "\u0000").replace(/\*/g, "[^/]+").replace(/\u0000/g, ".*")}$`);
}

async function pool(items, n, fn) {
  const out = new Array(items.length);
  let i = 0;
  await Promise.all(Array.from({ length: n }, async () => {
    while (i < items.length) {
      const k = i++;
      out[k] = await fn(items[k], k);
    }
  }));
  return out;
}

async function fetchJson(url, body, cacheFile) {
  if (cacheFile && existsSync(cacheFile)) return JSON.parse(readFileSync(cacheFile, "utf8"));
  for (let attempt = 0; attempt < 5; attempt++) {
    try {
      const r = await fetch(url, body ? { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) } : {});
      if (r.status === 404) return null;
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const j = await r.json();
      if (cacheFile) writeJson(cacheFile, j, false);
      return j;
    } catch (e) {
      if (attempt === 4) throw new Error(`${url}: ${e.message}`);
      await new Promise((s) => setTimeout(s, 1500 * (attempt + 1)));
    }
  }
}

// ---------------------------------------------------------------------------
// fetch + vendor
// ---------------------------------------------------------------------------

/** Workspace-member manifests a lockfile (or package.json) refers to. */
function workspaceManifests(repo, lockRel, tree) {
  const dir = posix.dirname(lockRel) === "." ? "" : `${posix.dirname(lockRel)}/`;
  const abs = join(repo, lockRel);
  if (!existsSync(abs)) return [];
  const text = readFileSync(abs, "utf8");
  const base = posix.basename(lockRel);
  let dirs = [];
  if (base === "package-lock.json") {
    dirs = Object.keys(JSON.parse(text).packages ?? {}).filter((k) => k && !k.includes("node_modules/"));
  } else if (base === "pnpm-lock.yaml") {
    const sec = text.split(/^importers:\s*$/m)[1]?.split(/^\S/m)[0] ?? "";
    dirs = [...sec.matchAll(/^ {2}'?([^\s':]+)'?:\s*$/gm)].map((m) => m[1]).filter((d) => d !== ".");
  } else if (base === "yarn.lock") {
    dirs = [...text.matchAll(/@workspace:([^",\s]+)/g)].map((m) => m[1]).filter((d) => d !== ".");
  } else if (base === "bun.lock") {
    const ws = text.split(/"workspaces":\s*\{/)[1] ?? "";
    dirs = [...ws.matchAll(/^ {4}"([^"]+)":\s*\{/gm)].map((m) => m[1]).filter(Boolean);
  } else if (base === "package.json") {
    const pj = JSON.parse(text);
    const pats = Array.isArray(pj.workspaces) ? pj.workspaces : pj.workspaces?.packages ?? [];
    const res = pats.map((p) => globToRegex(dir + p.replace(/^\.\//, "")));
    return tree.filter((t) => t.endsWith("/package.json") && res.some((re) => re.test(posix.dirname(t))));
  }
  return [...new Set(dirs)].map((d) => posix.normalize(dir + d + "/package.json")).filter((p) => tree.includes(p));
}

function resolveIncludes(c, repo, tree) {
  const plain = c.include.filter((p) => !p.startsWith("@"));
  const out = new Set(plain.filter((p) => tree.includes(p)));
  if (c.include.includes("@cargo-manifests")) {
    for (const t of tree) if (posix.basename(t) === "Cargo.toml" && !/(^|\/)target\//.test(t)) out.add(t);
  }
  if (c.include.includes("@workspaces")) {
    for (const p of plain) for (const m of workspaceManifests(repo, p, tree)) out.add(m);
  }
  return [...out].sort();
}

function stepFetch(c) {
  const repo = join(CACHE, "repos", c.id);
  if (!existsSync(join(repo, ".git"))) {
    mkdirSync(repo, { recursive: true });
    git(repo, "init", "-q");
    git(repo, "remote", "add", "origin", c.repo);
    git(repo, "config", "core.autocrlf", "false");
    const f = git(repo, "fetch", "-q", "--depth", "1", "--filter=blob:none", "origin", c.commit);
    if (f.status !== 0) throw new Error(`${c.id}: fetch ${c.commit}: ${f.stderr}`);
    git(repo, "sparse-checkout", "set", "--no-cone", "/package.json");
    git(repo, "-c", "advice.detachedHead=false", "checkout", "-q", "FETCH_HEAD");
  }
  const tree = git(repo, "ls-tree", "-r", "--name-only", "HEAD").stdout.split("\n").filter(Boolean);
  const first = c.include.filter((p) => !p.startsWith("@"));
  git(repo, "sparse-checkout", "set", "--no-cone", ...first.map((p) => `/${p}`));
  const all = resolveIncludes(c, repo, tree);
  git(repo, "sparse-checkout", "set", "--no-cone", ...all.map((p) => `/${p}`));
  const missing = first.filter((p) => !tree.includes(p));
  if (missing.length) throw new Error(`${c.id}: include paths not in ${c.commit}: ${missing.join(", ")}`);
  return { repo, files: resolveIncludes(c, repo, tree) };
}

function stepVendor(c, fetched) {
  const caseDir = join(CORPUS, "cases", c.id);
  for (const e of readdirSync(caseDir)) if (e !== "case.json") rmSync(join(caseDir, e), { recursive: true, force: true });
  for (const rel of fetched.files) {
    const buf = readFileSync(join(fetched.repo, rel));
    const isText = !buf.includes(0);
    mkdirSync(dirname(join(caseDir, rel)), { recursive: true });
    writeFileSync(join(caseDir, rel), isText ? buf.toString("utf8").replace(/\r\n/g, "\n") : buf);
  }
  console.log(`vendor ${c.id}: ${fetched.files.length} files`);
}

// ---------------------------------------------------------------------------
// scan: osv-scanner inventory + Go build lists
// ---------------------------------------------------------------------------

// --no-resolve: osv-scanner would otherwise RESOLVE manifests (requirements.txt,
// pom.xml) into a transitive set via deps.dev — a network guess, not a lockfile fact.
const SCANNER_FLAGS = ["scan", "source", "--no-ignore", "--no-resolve", "--allow-no-lockfiles", "--all-packages", "--format", "json"];

function stepScan(c) {
  const caseDir = join(CORPUS, "cases", c.id);
  const out = join(CACHE, "scan", `${c.id}.json`);
  mkdirSync(dirname(out), { recursive: true });
  rmSync(join(CACHE, "scan-extra", c.id), { recursive: true, force: true });
  const r = run(OSV_SCANNER, [...SCANNER_FLAGS, "-r", "--output-file", out, caseDir]);
  if (!existsSync(out)) throw new Error(`${c.id}: osv-scanner rc=${r.status}: ${r.stderr.slice(-800)}`);
  // Lockfiles the directory walk did not extract: requirements files with other
  // names get an explicit parser; a file osv-scanner cannot parse at all falls
  // back to this script's own minimal extractor (recorded per file in truth).
  const seen = new Set((readJson(out).results ?? []).map((x) => relative(caseDir, x.source.path).replace(/\\/g, "/")));
  const unsupported = new Set((c.unsupported ?? []).map((u) => u.path));
  for (const rel of walkFiles(caseDir).filter((f) => isLockfileLike(f) && !seen.has(f) && !unsupported.has(f))) {
    const base = posix.basename(rel);
    if (base === "go.sum" || base === "go.mod") continue; // Go truth comes from goBuildList
    const extra = join(CACHE, "scan-extra", c.id, `${rel.replace(/\//g, "__")}.json`);
    mkdirSync(dirname(extra), { recursive: true });
    const parser = base.endsWith(".txt") ? "requirements.txt" : base;
    const rr = run(OSV_SCANNER, [...SCANNER_FLAGS, "--output-file", extra, "-L", `${parser}:${join(caseDir, rel)}`]);
    const ok = existsSync(extra) && (readJson(extra).results ?? []).length > 0;
    if (!ok && base === "pnpm-lock.yaml") writeJson(extra, { fallback: `osv-scanner could not parse (${(rr.stderr.match(/could not extract: [^\n]*/) || [""])[0]})`, results: [{ source: { path: join(caseDir, rel), fallback: "own pnpm packages: reader" }, packages: pnpmPackages(readFileSync(join(caseDir, rel), "utf8")) }] });
    else if (!ok) console.warn(`  ${c.id}: ${rel} extracted by nothing (rc=${rr.status})`);
  }
  for (const rel of walkFiles(caseDir).filter((f) => posix.basename(f) === "go.mod")) goBuildList(c, caseDir, rel);
  console.log(`scan ${c.id}: rc=${r.status}`);
}

/** Minimal pnpm `packages:` key reader (v5 `/n/1.0.0_peer`, v6 `/n@1.0.0(peer)`, v9 `n@1.0.0(peer)`). */
function pnpmPackages(text) {
  const out = new Map();
  const put = (name, version, key) => out.set(`${name}@${version}`, { package: { ecosystem: "npm", name, version }, key });
  let inPackages = false;
  let pending = null; // a tarball/URL key: identity comes from its name:/version: fields
  for (const line of text.split("\n")) {
    if (/^\S/.test(line)) { inPackages = line.startsWith("packages:"); continue; }
    if (!inPackages) continue;
    const key = line.match(/^ {2}'?([^\s':][^']*?)'?:\s*$/)?.[1];
    if (key) {
      const m = key.match(/^\/?((?:@[^/@]+\/)?[^/@(]+)[/@](\d[^_(/]*)/);
      pending = null;
      if (m && !/\.tgz|https?:|registry\./.test(key)) put(m[1], m[2], key);
      else if (/\.tgz|https?:|registry\./.test(key)) pending = { key }; // file:/link: keys are local code
      continue;
    }
    if (pending) {
      const f = line.match(/^ {4}(name|version):\s*'?([^'\s]+)'?\s*$/);
      if (f) pending[f[1]] = f[2];
      if (pending.name && pending.version) { put(pending.name, pending.version, pending.key); pending = null; }
    }
  }
  return [...out.values()];
}

/**
 * The modules a Go build installs, with `replace` applied (a replacement by a
 * local directory installs nothing from the registry; a replacement by another
 * module installs THAT module at THAT version). go >= 1.17 go.mod files are
 * complete by contract (module-graph pruning), so their require lines are the
 * set; older go.mod files omit indirect modules, so the set is Go's own MVS
 * build list (`go list -m all`) — never the go.sum universe.
 */
function goBuildList(c, caseDir, goModRel) {
  const text = readFileSync(join(caseDir, goModRel), "utf8");
  const [maj, min] = ((text.match(/^go\s+(\d+)\.(\d+)/m) || []).slice(1).map(Number));
  const out = join(CACHE, "golist", c.id, `${goModRel.replace(/\//g, "__")}.json`);
  if (maj > 1 || (maj === 1 && min >= 17)) return writeJson(out, goModRequires(text));
  const work = join(CACHE, "gowork", c.id, posix.dirname(goModRel));
  rmSync(work, { recursive: true, force: true });
  mkdirSync(work, { recursive: true });
  for (const f of ["go.mod", "go.sum"]) {
    const src = join(caseDir, posix.dirname(goModRel), f);
    if (existsSync(src)) cpSync(src, join(work, f));
  }
  const env = { ...process.env, GOFLAGS: "-mod=mod", GOTOOLCHAIN: "local", GOMODCACHE: join(CACHE, "gomod"), GOCACHE: join(CACHE, "gobuild"), GONOSUMDB: "*", GOSUMDB: "off" };
  const r = run("go", ["list", "-m", "-json", "all"], work, { env });
  if (r.status !== 0) throw new Error(`${c.id}/${goModRel}: go list failed: ${r.stderr.slice(-1500)}`);
  const mods = [];
  for (const chunk of r.stdout.split(/\n}\n?/)) {
    if (!chunk.trim()) continue;
    const m = JSON.parse(`${chunk}}`);
    if (m.Main) continue;
    const eff = m.Replace?.Version ? m.Replace : m;
    if (m.Replace && !m.Replace.Version) continue; // replaced by a local directory: not a registry install
    if (!eff.Version) continue;
    mods.push({ name: eff.Path, version: eff.Version.replace(/^v/, ""), replaces: m.Replace ? m.Path : undefined });
  }
  // The MVS list also holds modules consulted only for their go.mod (graph
  // nodes whose source is never downloaded). Keep the selected modules whose
  // SOURCE hash is in go.sum: the ones the build actually fetched.
  const sumPath = join(caseDir, posix.dirname(goModRel), "go.sum");
  const sum = existsSync(sumPath) ? readFileSync(sumPath, "utf8").split("\n").map((l) => l.trim().split(/\s+/)) : [];
  const zips = new Set(sum.filter(([, v]) => v && !v.endsWith("/go.mod")).map(([n, v]) => `${n}@${v.replace(/^v/, "")}`));
  writeJson(out, mods.filter((m) => zips.has(`${m.name}@${m.version}`)));
}

/** go.mod `require` set with `replace` directives applied. */
function goModRequires(text) {
  const body = text.replace(/\/\/[^\n]*/g, "");
  const lines = [];
  for (const [, kw, block, single] of body.matchAll(/^(require|replace)\s*(?:\(([\s\S]*?)\)|([^\n]+))/gm)) {
    for (const l of (block ?? single).split("\n").map((s) => s.trim()).filter(Boolean)) lines.push([kw, l]);
  }
  const reqs = lines.filter(([k]) => k === "require").map(([, l]) => l.split(/\s+/));
  const reps = lines.filter(([k]) => k === "replace").map(([, l]) => l.split(/\s*=>\s*/).map((s) => s.split(/\s+/)));
  const mods = [];
  for (const [name, version] of reqs) {
    const rep = reps.find(([[from, fromVer]]) => from === name && (!fromVer || fromVer === version));
    if (!rep) { mods.push({ name, version: version.replace(/^v/, "") }); continue; }
    const [to, toVer] = rep[1];
    if (!toVer) continue; // local directory replacement
    mods.push({ name: to, version: toVer.replace(/^v/, ""), replaces: name });
  }
  return mods;
}

/** Raw inventory per lockfile from osv-scanner (+ go list for go.mod). */
function scannerInventory(c) {
  const caseDir = join(CORPUS, "cases", c.id);
  const scan = readJson(join(CACHE, "scan", `${c.id}.json`), { results: [] });
  const extraDir = join(CACHE, "scan-extra", c.id);
  const extras = existsSync(extraDir) ? readdirSync(extraDir).flatMap((f) => readJson(join(extraDir, f)).results ?? []) : [];
  const byLock = new Map();
  for (const res of [...(scan.results ?? []), ...extras]) {
    const rel = relative(caseDir, res.source.path).replace(/\\/g, "/");
    const pkgs = (res.packages ?? []).map((p) => ({ eco: normEco(p.package.ecosystem), name: p.package.name, version: p.package.version }));
    const kept = pkgs.filter((p) => !(p.eco === "Go" && ["stdlib", "toolchain"].includes(p.name)));
    kept.fallback = res.source.fallback;
    byLock.set(rel, kept);
  }
  for (const rel of walkFiles(caseDir).filter((f) => posix.basename(f) === "go.mod")) {
    const list = readJson(join(CACHE, "golist", c.id, `${rel.replace(/\//g, "__")}.json`), null);
    if (list) byLock.set(`${rel}#golist`, list.map((m) => ({ eco: "Go", name: m.name, version: m.version, replacedBy: m.replacedBy })));
  }
  return byLock;
}
const normEco = (e) => ({ pypi: "PyPI", golang: "Go", "crates.io": "crates.io" })[String(e).toLowerCase()] ?? String(e).replace(/:.*$/, "");

// ---------------------------------------------------------------------------
// truth inventory = scanner inventory + adjudications
// ---------------------------------------------------------------------------

/**
 * Mechanical corrections to osv-scanner's extraction, each a documented
 * scanner error (README "Adjudication rules"). Returns {keys, applied[]}.
 */
function truthInventory(c, adjud) {
  const inv = new Map(); // key -> {eco,dir,name,version}
  const applied = [];
  const note = (rule, key, evidence) => applied.push({ case: c.id, rule, key, evidence });
  const unsupported = new Set((c.unsupported ?? []).map((u) => u.path));
  const caseDir = join(CORPUS, "cases", c.id);
  const pins = requirementPins(caseDir);
  for (const [lock, pkgs] of scannerInventory(c)) {
    const isGolist = lock.endsWith("#golist");
    const rel = lock.replace(/#golist$/, "");
    const dir = ownerDir(rel);
    if (unsupported.has(rel)) continue; // declared unsupported: outside the scored inventory
    if (posix.basename(rel) === "go.mod" && !isGolist) continue; // superseded by the build list
    if (posix.basename(rel) === "go.sum") continue; // go.sum is a checksum DB, not an install set
    if (pkgs.fallback) note("extractor-fallback", rel, `osv-scanner could not parse ${rel}; inventory from ${pkgs.fallback}`);
    const local = isGolist ? new Map() : localPackages(join(caseDir, rel));
    for (const p of pkgs) {
      let version = p.version ?? "";
      if (!version) continue; // unpinned requirement: nothing is installed at a known version
      if (/^0\.0\.0-use\.local$/.test(version) || /^(link|file|workspace|portal):/.test(version)) {
        note("local-workspace-package", invKey(p.eco, dir, p.name, version), `${rel}: ${p.name}@${version}`);
        continue;
      }
      const why = local.get(`${normName(p.eco, p.name)}@${normVersion(p.eco, version)}`);
      if (why) {
        note("local-workspace-package", invKey(p.eco, dir, p.name, version), `${rel}: ${why}`);
        continue;
      }
      if (rel.endsWith(".txt") && !pins.has(`${normName("PyPI", p.name)}@${normVersion("PyPI", version)}`)) {
        note("unpinned-requirement", invKey(p.eco, dir, p.name, version), `${rel}: ${p.name} is not pinned with == / === (osv-scanner reported ${version})`);
        continue;
      }
      const peer = version.match(/^([^(_]+?)(?:\(|_)/);
      if (p.eco === "npm" && peer) {
        note("pnpm-peer-suffix", invKey(p.eco, dir, p.name, version), `${rel}: ${version} -> ${peer[1]}`);
        version = peer[1];
      }
      inv.set(invKey(p.eco, dir, p.name, version), { eco: p.eco, dir, name: p.name, version, lock: rel });
    }
    if (posix.basename(rel) === "pnpm-lock.yaml" && !pkgs.fallback) {
      // osv-scanner 2.6.0 silently drops pnpm keys it cannot split (v6
      // `/a@1.0.0(peer@x)(peer@y)` with several peers, tarball keys). The
      // lockfile's own `packages:` keys are the installed set.
      for (const { package: p, key } of pnpmPackages(readFileSync(join(caseDir, rel), "utf8"))) {
        const k = invKey("npm", dir, p.name, p.version);
        if (inv.has(k)) continue;
        inv.set(k, { eco: "npm", dir, name: p.name, version: p.version, lock: rel });
        note("pnpm-key-dropped-by-scanner", k, `${rel}: packages key ${key}`);
      }
    }
  }
  for (const a of adjud.truth_overrides.filter((x) => x.case === c.id)) {
    for (const k of a.remove ?? []) if (inv.delete(k)) note(`override:${a.reason_code}`, k, a.reason);
    for (const k of a.add ?? []) {
      const [eco, dir, name, version] = k.split("|");
      inv.set(k, { eco, dir, name, version, lock: a.lockfile });
      note(`override:${a.reason_code}`, k, a.reason);
    }
  }
  return { inv, applied };
}

/** `name@version` for every `==`/`===` pin in any requirements file of a case. */
function requirementPins(caseDir) {
  const pins = new Set();
  for (const rel of walkFiles(caseDir).filter((f) => f.endsWith(".txt") && isLockfileLike(f))) {
    const text = readFileSync(join(caseDir, rel), "utf8").replace(/\\\n/g, " ");
    for (const line of text.split("\n")) {
      const m = line.replace(/#.*/, "").match(/^\s*([A-Za-z0-9][A-Za-z0-9._-]*)\s*(?:\[[^\]]*\])?\s*(===|==)\s*([^\s;,\\]+)/);
      if (m) pins.add(`${normName("PyPI", m[1])}@${normVersion("PyPI", m[3])}`);
    }
  }
  return pins;
}

/**
 * Packages a lockfile resolves from the project itself (workspace members,
 * path/editable/VCS sources) rather than from the registry: `name@version` ->
 * evidence. Registry advisories describe the registry artifact, not local code
 * that happens to share its name, so these are never in the truth.
 */
function localPackages(abs) {
  const out = new Map();
  if (!existsSync(abs)) return out;
  const base = posix.basename(abs.replace(/\\/g, "/"));
  const text = readFileSync(abs, "utf8");
  const add = (eco, name, version, why) => version && out.set(`${normName(eco, name)}@${normVersion(eco, version)}`, why);
  if (base === "package-lock.json" || base === "npm-shrinkwrap.json") {
    const pk = JSON.parse(text).packages ?? {};
    for (const [key, e] of Object.entries(pk)) {
      if (!key || key.includes("node_modules/")) continue;
      add("npm", e.name ?? posix.basename(key), e.version, `workspace member ${key} (no node_modules path)`);
      add("npm", key, e.version, `workspace member ${key} (no node_modules path)`);
    }
  } else if (base === "Cargo.lock" || base === "uv.lock" || base === "poetry.lock") {
    const eco = base === "Cargo.lock" ? "crates.io" : "PyPI";
    for (const block of text.split(/^\[\[package\]\]\s*$/m).slice(1)) {
      const name = block.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
      const version = block.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
      if (!name) continue;
      if (base === "Cargo.lock" && !/^source\s*=/m.test(block)) add(eco, name, version, `${name} ${version} has no source: a workspace/path crate`);
      const uvSrc = block.match(/^source\s*=\s*\{\s*(editable|virtual|directory|path|git|url)\s*=/m)?.[1];
      if (base === "uv.lock" && uvSrc) add(eco, name, version, `${name} ${version} source = ${uvSrc}`);
      const poSrc = block.match(/^\[package\.source\][\s\S]*?^type\s*=\s*"([^"]+)"/m)?.[1];
      if (base === "poetry.lock" && poSrc && poSrc !== "legacy") add(eco, name, version, `${name} ${version} source type = ${poSrc}`);
    }
  } else if (base === "Gemfile.lock") {
    for (const block of text.split(/\n[ \t]*\n/)) {
      const kind = block.trimStart().split("\n")[0].trim();
      if (kind !== "PATH" && kind !== "GIT") continue;
      for (const m of block.matchAll(/^ {4}(\S+) \(([^)]+)\)$/gm)) add("RubyGems", m[1], m[2], `${m[1]} ${m[2]} is a ${kind} gem (not from rubygems.org)`);
    }
  } else if (base === "bun.lock") {
    for (const m of text.matchAll(/"([^"]+)":\s*\["\1@(?:workspace|link|file):[^"]*"/g)) add("npm", m[1], "0.0.0", `${m[1]} is a bun workspace/link package`);
  }
  return out;
}

// ---------------------------------------------------------------------------
// pin: OSV records
// ---------------------------------------------------------------------------

async function querybatch(queries, tag) {
  const out = [];
  for (let i = 0; i < queries.length; i += 1000) {
    const chunk = queries.slice(i, i + 1000);
    const h = createHash("sha1").update(JSON.stringify(chunk)).digest("hex").slice(0, 16);
    let res = await fetchJson("https://api.osv.dev/v1/querybatch", { queries: chunk }, join(CACHE, "osvq", `${tag}-${h}.json`));
    const results = res.results.map((r) => ({ ids: (r.vulns ?? []).map((v) => v.id), token: r.next_page_token }));
    for (let j = 0; j < chunk.length; j++) {
      let token = results[j].token;
      while (token) {
        const page = await fetchJson("https://api.osv.dev/v1/query", { ...chunk[j], page_token: token }, null);
        results[j].ids.push(...(page.vulns ?? []).map((v) => v.id));
        token = page.next_page_token;
      }
    }
    out.push(...results.map((r) => r.ids));
  }
  return out;
}

function engineInventory(id) {
  if (!ENGINE_DUMP) return [];
  return readJson(join(ENGINE_DUMP, `${id}.json`), { inventory: [] }).inventory;
}

async function stepPin(cases, adjud) {
  const names = new Map();
  for (const c of cases) {
    for (const p of truthInventory(c, adjud).inv.values()) names.set(`${p.eco}|${p.name}`, { eco: p.eco, name: p.name });
    for (const k of engineInventory(c.id)) {
      const [eco, , name] = k.split("|");
      names.set(`${eco}|${name}`, { eco, name });
    }
  }
  const list = [...names.values()].sort((a, b) => `${a.eco}|${a.name}`.localeCompare(`${b.eco}|${b.name}`));
  const idLists = await querybatch(list.map((p) => ({ package: { ecosystem: p.eco, name: p.name } })), "names");
  const ids = [...new Set(idLists.flat())].sort();
  console.log(`pin: ${list.length} package names -> ${ids.length} advisories`);
  const osvDir = join(CORPUS, "osv");
  rmSync(osvDir, { recursive: true, force: true });
  mkdirSync(osvDir, { recursive: true });
  await pool(ids, 16, async (id) => {
    const rec = await fetchJson(`https://api.osv.dev/v1/vulns/${encodeURIComponent(id)}`, null, join(CACHE, "vulns", `${id}.json`));
    if (rec) writeFileSync(join(osvDir, `${id}.json`), `${JSON.stringify(rec)}\n`);
  });
  writeJson(join(CORPUS, "osv-snapshot.json"), { fetched_at: new Date().toISOString().slice(0, 10), source: "https://api.osv.dev/v1/vulns/<id>", package_names: list.length, records: ids.length });
}

// ---------------------------------------------------------------------------
// truth: expected/<id>.json
// ---------------------------------------------------------------------------

function aliasGroups() {
  const parent = new Map();
  const find = (x) => {
    if (!parent.has(x)) parent.set(x, x);
    while (parent.get(x) !== x) x = parent.get(x);
    return x;
  };
  const union = (a, b) => parent.set(find(a), find(b));
  for (const f of readdirSync(join(CORPUS, "osv"))) {
    const r = readJson(join(CORPUS, "osv", f));
    for (const a of r.aliases ?? []) union(r.id, a);
  }
  return find;
}

function lockfileTable(c, inv) {
  const caseDir = join(CORPUS, "cases", c.id);
  const unsupported = new Map((c.unsupported ?? []).map((u) => [u.path, u.reason]));
  return walkFiles(caseDir).filter(isLockfileLike).sort().map((rel) => {
    const eco = ecosystemOf(rel);
    const dir = ownerDir(rel);
    const truthPackages = [...inv.values()].filter((p) => p.eco === eco && p.dir === dir).length;
    const row = { path: rel, dir, format: formatOf(join(caseDir, rel), rel), ecosystem: eco };
    return unsupported.has(rel) ? { ...row, status: "unsupported", reason: unsupported.get(rel) } : { ...row, status: "supported", truth_packages: truthPackages };
  });
}

async function stepTruth(cases, adjud) {
  const find = aliasGroups();
  const corrections = [];
  const disagreements = [];
  for (const c of cases) {
    const { inv, applied } = truthInventory(c, adjud);
    const pkgs = [...inv.values()].sort((a, b) => invKey(a.eco, a.dir, a.name, a.version).localeCompare(invKey(b.eco, b.dir, b.name, b.version)));
    const idLists = await querybatch(pkgs.map((p) => ({ package: { ecosystem: p.eco, name: p.name }, version: p.eco === "Go" ? `v${p.version}` : p.version })), "versions");
    const findings = [];
    pkgs.forEach((p, i) => {
      const groups = new Map();
      for (const id of idLists[i]) {
        if (!existsSync(join(CORPUS, "osv", `${id}.json`))) throw new Error(`${c.id}: ${id} affects ${p.name}@${p.version} but is not pinned; re-run pin`);
        const g = find(id);
        groups.set(g, [...(groups.get(g) ?? []), id]);
      }
      for (const ids of groups.values()) findings.push({ ecosystem: p.eco, dir: p.dir, package: p.name, version: p.version, ids: ids.sort() });
    });
    const expected = {
      schema: 1,
      case: c.id,
      lockfiles: lockfileTable(c, inv),
      inventory: { count: pkgs.length, packages: pkgs.map((p) => invKey(p.eco, p.dir, p.name, p.version)) },
      findings,
    };
    writeJson(join(CORPUS, "expected", `${c.id}.json`), expected);
    corrections.push(...groupBy(applied, (a) => `${a.case}\u0000${a.rule}`).map(([, xs]) => ({
      case: c.id, rule: xs[0].rule, count: xs.length, keys: xs.map((x) => x.key).sort(), evidence: xs.slice(0, 3).map((x) => x.evidence),
    })));
    if (ENGINE_DUMP) disagreements.push(...classifyDisagreements(c, expected, find));
    console.log(`truth ${c.id}: ${pkgs.length} packages, ${findings.length} findings, ${applied.length} truth corrections`);
  }
  writeJson(join(CORPUS, "adjudications.json"), {
    schema: 1,
    about: "Every correction applied to osv-scanner's extraction (truth_corrections: mechanical rules, see README), every manual truth override (truth_overrides: hand-reviewed, preserved across regeneration), and every disagreement between the Rust engine and the truth at the time of generation, each with a verdict (engine_disagreements: regenerated with --engine-dump).",
    generated: new Date().toISOString().slice(0, 10),
    truth_rules: TRUTH_RULES,
    truth_corrections: ONLY ? [...(adjud.truth_corrections ?? []).filter((x) => !ONLY.includes(x.case)), ...corrections] : corrections,
    truth_overrides: adjud.truth_overrides,
    engine_disagreements: ENGINE_DUMP ? (ONLY ? [...(adjud.engine_disagreements ?? []).filter((x) => !ONLY.includes(x.case)), ...disagreements] : disagreements) : adjud.engine_disagreements ?? [],
  });
}

const TRUTH_RULES = {
  "local-workspace-package": "A package the lockfile resolves from the project itself (workspace member, `0.0.0-use.local`, `link:`/`file:`/`portal:`, a Cargo crate without `source`, a uv/poetry editable/directory/git/url source, a Gemfile PATH/GIT gem). Registry advisories describe the registry artifact, not local code sharing its name; osv-scanner lists some of these as registry packages.",
  "pnpm-peer-suffix": "A pnpm key carries peer-dependency context (`1.0.0_react@18.2.0`, `1.0.0(react@18.2.0)`); the installed version is the part before it.",
  "pnpm-key-dropped-by-scanner": "osv-scanner 2.6.0 silently drops pnpm `packages:` keys it cannot split (v6 keys carrying several `(peer@x)` suffixes, tarball-URL keys with name/version fields). The lockfile's own keys are the installed set, so the missing entries are added.",
  "extractor-fallback": "osv-scanner could not parse the lockfile at all; its inventory comes from this script's own minimal reader (pnpm `packages:` keys).",
  "go-build-list": "Go truth is never go.sum (a checksum database that also lists modules outside the build) nor, before go 1.17, go.mod (which omits indirect modules): go >= 1.17 uses the complete go.mod require set; older modules use Go's own MVS build list (`go list -m all`) restricted to modules whose source hash go.sum records. `replace` is applied; a local-directory replacement installs nothing from the registry.",
  "unpinned-requirement": "A requirements line with a range (or no version) installs no known version and is not in the inventory.",
};

const groupBy = (xs, keyOf) => {
  const m = new Map();
  for (const x of xs) m.set(keyOf(x), [...(m.get(keyOf(x)) ?? []), x]);
  return [...m.entries()];
};

/**
 * Every disagreement between the engine dump and the truth, with a verdict.
 * Inventory first (a finding disagreement usually follows from one), then
 * findings. `UNRESOLVED` verdicts need a manual truth_override or a reason.
 */
function classifyDisagreements(c, expected, find) {
  const caseDir = join(CORPUS, "cases", c.id);
  const dump = readJson(join(ENGINE_DUMP, `${c.id}.json`), { inventory: [], findings: [] });
  const truth = new Set(expected.inventory.packages);
  const eng = new Set(dump.inventory);
  const nameKey = (k) => k.split("|").slice(0, 3).join("|");
  const truthNames = new Set([...truth].map(nameKey));
  const engNames = new Set([...eng].map(nameKey));
  const engDirs = new Set([...eng].map((k) => k.split("|").slice(0, 2).join("|")));
  const sharedNames = new Set([...eng].filter((e) => truth.has(e)).map(nameKey)); // a version both sides hold
  const lockText = (eco, dir) => expected.lockfiles.filter((l) => l.dir === dir && l.ecosystem === eco).map((l) => readFileSync(join(caseDir, l.path), "utf8")).join("\n");
  const verdicts = [];
  const push = (kind, side, verdict, key) => verdicts.push({ kind, side, verdict, key });
  for (const k of [...eng].filter((x) => !truth.has(x))) {
    const [eco, dir, name, version] = k.split("|");
    const locals = expected.lockfiles.filter((l) => l.dir === dir).map((l) => localPackages(join(caseDir, l.path)));
    const text = lockText(eco, dir);
    if (locals.some((m) => m.has(`${name}@${version}`))) push("inventory", "engine-only", "engine-error: local/workspace package read as a registry install", k);
    else if (/\.tgz|registry\.|https?:/.test(name)) push("inventory", "engine-only", "engine-error: a tarball/URL lockfile key read as a package name", k);
    else if (eco === "RubyGems" && /-(java|x86|x64|universal|mingw|mswin|darwin|linux|arm)/.test(version)) push("inventory", "engine-only", "engine-error: gem platform suffix kept in the version", k);
    else if (eco === "Go" && goModOnly(text, name, version)) push("inventory", "engine-only", "engine-error: graph-only module — go.sum holds only its go.mod hash, so its source is never downloaded or built", k);
    else if (eco === "Go" && new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")} v${version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}[ /]`, "m").test(text)) push("inventory", "engine-only", "engine-error: go.sum universe — module listed in go.sum but not installed by the build", k);
    else if (truthNames.has(nameKey(k))) push("inventory", "engine-only", "version disagreement: truth holds this package at another version", k);
    else if (!rawPresent(text, eco, name, version)) push("inventory", "engine-only", "engine-error: package@version does not occur in the lockfile text", k);
    else push("inventory", "engine-only", "UNRESOLVED: present in the lockfile text but not in the truth", k);
  }
  for (const k of [...truth].filter((x) => !eng.has(x))) {
    const [eco, dir] = k.split("|");
    if (!engDirs.has(`${eco}|${dir}`)) push("inventory", "truth-only", "engine-gap: the engine read nothing for this lockfile directory/ecosystem", k);
    else if (sharedNames.has(nameKey(k))) push("inventory", "truth-only", "engine-error: one of several installed versions of this package is missing (nested duplicates collapsed)", k);
    else if (eco === "RubyGems" && [...eng].some((e) => e.startsWith(`${k}-`))) push("inventory", "truth-only", "engine-error: gem platform suffix kept in the version", k);
    else if (engNames.has(nameKey(k))) push("inventory", "truth-only", "version disagreement: engine holds this package at another version", k);
    else push("inventory", "truth-only", "engine-error: package in the lockfile but not in the engine inventory", k);
  }
  const fkey = (f, id) => `${invKey(f.ecosystem, f.dir, f.package, f.version)}|${find(id)}`;
  const truthF = new Set(expected.findings.map((f) => fkey(f, f.ids[0])));
  const engF = new Set(dump.findings.map((f) => fkey(f, f.id)));
  const pkgOf = (fk) => fk.split("|").slice(0, 4).join("|");
  for (const k of [...engF].filter((x) => !truthF.has(x))) {
    push("finding", "engine-only", truth.has(pkgOf(k)) ? "engine-error: matched an advisory OSV's own evaluator says does not affect this version" : "follows from an engine-only inventory entry", k);
  }
  for (const k of [...truthF].filter((x) => !engF.has(x))) {
    push("finding", "truth-only", eng.has(pkgOf(k)) ? "engine-error: missed an advisory that affects an inventoried version" : "follows from a truth-only inventory entry", k);
  }
  return groupBy(verdicts, (v) => `${v.kind}\u0000${v.side}\u0000${v.verdict}`).map(([, xs]) => ({
    case: c.id, kind: xs[0].kind, side: xs[0].side, verdict: xs[0].verdict, count: xs.length,
    keys: xs.map((x) => x.key).sort().slice(0, 40), ...(xs.length > 40 ? { keys_truncated: xs.length - 40 } : {}),
  }));
}

function goModOnly(text, name, version) {
  const lines = text.split("\n").filter((l) => l.startsWith(`${name} v${version} `) || l.startsWith(`${name} v${version}/`));
  return lines.length > 0 && lines.every((l) => l.split(/\s+/)[1].endsWith("/go.mod"));
}

function rawPresent(text, eco, name, version) {
  const esc = (x) => x.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
  const n = eco === "PyPI" ? name.split("-").map(esc).join("[-_.]") : esc(name);
  return new RegExp(`${n}[\\s\\S]{0,400}?${esc(version)}`, "i").test(text);
}

// ---------------------------------------------------------------------------

async function main() {
  const step = args[0];
  const cases = loadCases();
  const adjud = readJson(join(CORPUS, "adjudications.json"), { truth_overrides: [] });
  const repoCases = cases.filter((c) => c.source === "repository");
  if (step === "fetch" || step === "vendor" || step === "all") {
    for (const c of repoCases) {
      const fetched = stepFetch(c);
      if (step !== "fetch") stepVendor(c, fetched);
    }
  }
  if (step === "scan" || step === "all") for (const c of cases) stepScan(c);
  if (step === "pin" || step === "all") await stepPin(cases, adjud);
  if (step === "truth" || step === "all") await stepTruth(cases, adjud);
  if (!["fetch", "vendor", "scan", "pin", "truth", "all"].includes(step)) {
    console.error("usage: node scripts/conformance-corpus.mjs <fetch|vendor|scan|pin|truth|all> [--cache DIR] [--osv-scanner PATH] [--engine-dump DIR] [--only a,b]");
    process.exit(2);
  }
}

main().catch((e) => {
  console.error(e.stack || e.message);
  process.exit(1);
});
