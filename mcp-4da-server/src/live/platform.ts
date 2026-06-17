// SPDX-License-Identifier: Apache-2.0
/**
 * Target/platform awareness for dependency relevance.
 *
 * Cargo (and other ecosystems) gate dependencies behind a target: e.g.
 * `[target.'cfg(windows)'.dependencies]` or `[target.x86_64-pc-windows-msvc.dependencies]`.
 * A dependency that is only active on a platform the user does not build is NOT
 * relevant to them - surfacing its advisory would be noise (the classic
 * "GTK/webkit vuln shown to a Windows user" problem).
 *
 * This module evaluates a target spec against the HOST platform so the scanner
 * can tag each dependency `platformActive` and let consumers suppress or label
 * the irrelevant ones. It is deliberately conservative: anything it cannot
 * confidently evaluate is treated as ACTIVE, so we never hide a real advisory.
 */

// =============================================================================
// Host facts (from the Node process running the scan)
// =============================================================================

export interface HostTarget {
  /** rustc-style os: "windows" | "macos" | "linux" | ... */
  os: string;
  /** "windows" | "unix" */
  family: string;
  /** rustc-style arch: "x86_64" | "aarch64" | ... */
  arch: string;
}

export function hostTarget(): HostTarget {
  const os =
    process.platform === "win32"
      ? "windows"
      : process.platform === "darwin"
        ? "macos"
        : process.platform === "linux"
          ? "linux"
          : String(process.platform);
  const family = os === "windows" ? "windows" : "unix";
  const arch =
    process.arch === "x64"
      ? "x86_64"
      : process.arch === "arm64"
        ? "aarch64"
        : process.arch === "ia32"
          ? "x86"
          : String(process.arch);
  return { os, family, arch };
}

// =============================================================================
// Target evaluation
// =============================================================================

/**
 * Is a dependency's target spec active on the host?
 * `target` is null/empty for unconditional deps (always active).
 * Returns true when active OR when the spec can't be confidently evaluated.
 */
export function targetActiveOnHost(target: string | null | undefined, host: HostTarget = hostTarget()): boolean {
  if (!target) return true;
  const spec = target.trim();

  // Explicit target triple, e.g. "x86_64-pc-windows-msvc" or "aarch64-apple-darwin".
  if (!spec.startsWith("cfg(")) {
    return tripleActiveOnHost(spec, host);
  }

  // cfg(...) predicate. Strip the outer cfg( ).
  const inner = spec.slice(4, spec.lastIndexOf(")"));
  return evalCfg(inner, host);
}

/** Evaluate a cfg predicate body (already unwrapped from `cfg( ... )`). */
export function evalCfg(expr: string, host: HostTarget = hostTarget()): boolean {
  const e = expr.trim();
  if (!e) return true;

  // not( ... )
  const not = matchCall(e, "not");
  if (not !== null) return !evalCfg(not, host);

  // all( a, b, ... )
  const all = matchCall(e, "all");
  if (all !== null) return splitArgs(all).every((a) => evalCfg(a, host));

  // any( a, b, ... )
  const any = matchCall(e, "any");
  if (any !== null) return splitArgs(any).some((a) => evalCfg(a, host));

  // key = "value"
  const kv = e.match(/^([a-z_]+)\s*=\s*"([^"]*)"$/);
  if (kv) return evalPredicate(kv[1], kv[2], host);

  // bare identifier: windows | unix
  if (e === "windows") return host.family === "windows";
  if (e === "unix") return host.family === "unix";

  // Unknown predicate -> conservatively active (never hide a real advisory).
  return true;
}

function evalPredicate(key: string, value: string, host: HostTarget): boolean {
  switch (key) {
    case "target_os":
      return value === host.os;
    case "target_family":
      return value === host.family;
    case "target_arch":
      return value === host.arch;
    case "windows":
      return host.family === "windows";
    case "unix":
      return host.family === "unix";
    default:
      return true; // unknown key -> conservatively active
  }
}

/** Match `name( ...body... )` and return the body, or null if not that call. */
function matchCall(expr: string, name: string): string | null {
  if (!expr.startsWith(name)) return null;
  const rest = expr.slice(name.length).trimStart();
  if (!rest.startsWith("(") || !rest.endsWith(")")) return null;
  return rest.slice(1, -1);
}

/** Split top-level comma-separated args, respecting nested parentheses. */
function splitArgs(body: string): string[] {
  const args: string[] = [];
  let depth = 0;
  let current = "";
  for (const ch of body) {
    if (ch === "(") depth++;
    if (ch === ")") depth--;
    if (ch === "," && depth === 0) {
      if (current.trim()) args.push(current.trim());
      current = "";
    } else {
      current += ch;
    }
  }
  if (current.trim()) args.push(current.trim());
  return args;
}

/** Evaluate an explicit target triple (e.g. x86_64-pc-windows-msvc) against the host. */
function tripleActiveOnHost(triple: string, host: HostTarget): boolean {
  const t = triple.toLowerCase();
  // OS component
  const tripleOs = t.includes("windows")
    ? "windows"
    : t.includes("darwin") || t.includes("apple")
      ? "macos"
      : t.includes("linux")
        ? "linux"
        : null;
  if (tripleOs && tripleOs !== host.os) return false;
  // Arch component (best-effort)
  const tripleArch = t.startsWith("x86_64")
    ? "x86_64"
    : t.startsWith("aarch64")
      ? "aarch64"
      : t.startsWith("i686") || t.startsWith("i586")
        ? "x86"
        : null;
  if (tripleArch && tripleArch !== host.arch) return false;
  return true;
}
