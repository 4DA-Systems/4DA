// Manifest parsers — package.json, Cargo.toml, requirements.txt, go.mod.
// Pure functions, no DOM, no network: unit-testable in Node, imported by the page.
// Output contract: { ecosystem, label, deps: [{ name, version, dev, indirect }] }
//   ecosystem: "npm" | "crates.io" | "PyPI" | "Go"  (OSV ecosystem names, used verbatim)
//   version: best-effort concrete version ("1.2.3") or null when only a range is known.

/** Detect manifest type from content (and optional filename hint). */
export function detectManifest(text, filename = "") {
  const t = text.trim();
  const f = filename.toLowerCase();
  if (f.endsWith("package.json")) return "npm";
  if (f.endsWith("cargo.toml")) return "crates.io";
  if (f.endsWith("requirements.txt")) return "PyPI";
  if (f.endsWith("go.mod")) return "Go";
  if (t.startsWith("{")) {
    try {
      const j = JSON.parse(t);
      if (j.dependencies || j.devDependencies || (j.name && j.version)) return "npm";
    } catch { /* not JSON */ }
  }
  if (/^\s*module\s+\S+/m.test(t) && /^\s*(go\s+\d|require[\s(])/m.test(t)) return "Go";
  if (/^\s*\[(dependencies|package|workspace)/m.test(t)) return "crates.io";
  if (/^[A-Za-z0-9_.-]+\s*[=<>~!]=/m.test(t)) return "PyPI";
  return null;
}

/** Strip a semver range prefix down to a concrete version, or null if not concrete enough. */
function concreteSemver(range) {
  if (!range || typeof range !== "string") return null;
  const r = range.trim();
  // ^1.2.3 / ~1.2.3 / =1.2.3 / 1.2.3 — usable. "*", ">=1.0", "1.x", workspace:, git/url — not.
  const m = r.match(/^[\^~=v]?(\d+\.\d+\.\d+(?:[-+][\w.-]+)?)$/);
  if (m) return m[1];
  const short = r.match(/^[\^~=v]?(\d+(?:\.\d+)?)$/); // "1" / "0.7" — pad for matching
  if (short) return short[1] + (short[1].includes(".") ? ".0" : ".0.0");
  return null;
}

export function parsePackageJson(text) {
  const j = JSON.parse(text);
  const deps = [];
  for (const [section, dev] of [["dependencies", false], ["devDependencies", true]]) {
    for (const [name, range] of Object.entries(j[section] || {})) {
      if (typeof range !== "string" || /^(workspace:|file:|link:|git|https?:)/.test(range)) continue;
      deps.push({ name, version: concreteSemver(range), range, dev, indirect: false });
    }
  }
  return { ecosystem: "npm", label: j.name || "package.json", deps };
}

export function parseCargoToml(text) {
  // Minimal TOML walk: track [section]; collect entries under *dependencies sections.
  // Handles `name = "1.2"` and `name = { version = "1.2", ... }` and `[dependencies.name]`.
  const deps = [];
  let section = "";
  let pkgName = null;
  for (const rawLine of text.split("\n")) {
    const line = rawLine.replace(/#.*$/, "").trim();
    if (!line) continue;
    const sec = line.match(/^\[([^\]]+)\]$/);
    if (sec) {
      section = sec[1].trim();
      const dotted = section.match(/^(?:workspace\.)?(dev-|build-)?dependencies\.(.+)$/);
      if (dotted) {
        deps.push({ name: dotted[2].trim().replace(/^"|"$/g, ""), version: null, range: null, dev: dotted[1] === "dev-", indirect: false, _dotted: true });
      }
      continue;
    }
    const inPkg = section === "package";
    const depSec = /^(?:workspace\.)?(dev-|build-)?dependencies$/.exec(section);
    const kv = line.match(/^([A-Za-z0-9_."'-]+)\s*=\s*(.+)$/);
    if (!kv) continue;
    const key = kv[1].replace(/^"|"$/g, "").replace(/^'|'$/g, "");
    const val = kv[2].trim();
    if (inPkg && key === "name") pkgName = val.replace(/^"|"$/g, "");
    if (depSec) {
      const dev = depSec[1] === "dev-";
      let version = null;
      const plain = val.match(/^"([^"]+)"$/);
      if (plain) version = concreteSemver(plain[1]);
      else {
        const inline = val.match(/version\s*=\s*"([^"]+)"/);
        if (inline) version = concreteSemver(inline[1]);
        else if (/^\{/.test(val) && !/version/.test(val)) continue; // path/git-only dep — skip
      }
      deps.push({ name: key, version, range: plain ? plain[1] : null, dev, indirect: false });
    }
    // version line inside a dotted [dependencies.name] section
    if (/^(?:workspace\.)?(dev-|build-)?dependencies\..+$/.test(section) && key === "version") {
      const last = deps[deps.length - 1];
      if (last && last._dotted) last.version = concreteSemver(val.replace(/^"|"$/g, ""));
    }
  }
  for (const d of deps) delete d._dotted;
  return { ecosystem: "crates.io", label: pkgName || "Cargo.toml", deps };
}

export function parseRequirementsTxt(text) {
  const deps = [];
  for (const rawLine of text.split("\n")) {
    const line = rawLine.replace(/(^|\s)#.*$/, "").trim();
    if (!line || line.startsWith("-")) continue; // skip -r includes, flags
    // name[extras]==1.2.3 ; markers   (PyPI names are case/sep-insensitive; normalize lightly)
    const m = line.match(/^([A-Za-z0-9_.-]+)(\[[^\]]*\])?\s*((?:[=<>~!]=?|===)\s*[^;,\s]+)?/);
    if (!m || !m[1]) continue;
    const spec = (m[3] || "").trim();
    const exact = spec.match(/^==\s*v?(\d[\w.+-]*)$/);
    deps.push({
      name: m[1].toLowerCase().replace(/[_.]+/g, "-"),
      version: exact ? exact[1] : null,
      range: spec || null,
      dev: false,
      indirect: false,
    });
  }
  return { ecosystem: "PyPI", label: "requirements.txt", deps };
}

export function parseGoMod(text) {
  const deps = [];
  let moduleName = null;
  let inRequire = false;
  for (const rawLine of text.split("\n")) {
    const line = rawLine.trim();
    const mod = line.match(/^module\s+(\S+)/);
    if (mod) { moduleName = mod[1]; continue; }
    if (/^require\s*\($/.test(line)) { inRequire = true; continue; }
    if (inRequire && line === ")") { inRequire = false; continue; }
    const one = line.match(/^require\s+(\S+)\s+(v\S+)/);
    const inBlock = inRequire ? line.match(/^(\S+)\s+(v\S+)(\s*\/\/\s*indirect)?/) : null;
    const m = one ? [null, one[1], one[2], /\/\/\s*indirect/.test(line) ? "i" : null] : inBlock;
    if (m) {
      deps.push({
        name: m[1],
        version: m[2].replace(/^v/, "").replace(/\+incompatible$/, ""),
        range: m[2],
        dev: false,
        indirect: Boolean(m[3]),
      });
    }
  }
  return { ecosystem: "Go", label: moduleName || "go.mod", deps };
}

export function parseManifest(text, filename = "") {
  const kind = detectManifest(text, filename);
  if (!kind) return null;
  try {
    if (kind === "npm") return parsePackageJson(text);
    if (kind === "crates.io") return parseCargoToml(text);
    if (kind === "PyPI") return parseRequirementsTxt(text);
    if (kind === "Go") return parseGoMod(text);
  } catch {
    return null;
  }
  return null;
}
