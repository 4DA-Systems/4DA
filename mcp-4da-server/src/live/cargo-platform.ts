// SPDX-License-Identifier: Apache-2.0
/**
 * Which crates in a Cargo workspace are actually built on THIS host.
 *
 * Cargo.lock is target-agnostic: it lists every crate for every platform the
 * tree can compile for. So a Windows machine's lockfile still contains the
 * whole Linux GTK3 stack that Tauri pulls in — `gtk`, `gdk`, `atk`, `glib`,
 * `gdkx11`, `gdkwayland-sys`, `gtk3-macros` — none of which are ever compiled
 * there. Advisories against them are unreachable noise on that host.
 *
 * `vulnerability_scan` claimed to filter for exactly this ("Advisories are
 * filtered to the host platform") while reporting `platform_inactive_packages:
 * 0` and listing nine Linux-only crates on a Windows box. The gate it relied on
 * only ever saw `[target.'cfg(...)'.dependencies]` entries from a manifest,
 * which covers DIRECT platform-gated deps and cannot see transitives — and the
 * GTK3 cluster is entirely transitive.
 *
 * Cargo.lock is also FEATURE-agnostic: it lists every optional dependency,
 * including the ones no enabled feature turns on. `cargo tree` resolves both
 * axes — target AND features — and is what the build itself does.
 *
 * This used `cargo metadata --filter-platform <triple>`, which resolves only
 * the target axis. Measured on 4DA's own workspace, 2026-09-08, host
 * `x86_64-pc-windows-msvc`:
 *
 * | mechanism                                   | distinct crates | `quinn-proto` |
 * |---------------------------------------------|-----------------|---------------|
 * | `Cargo.lock`                                | 788             | present       |
 * | `cargo metadata --filter-platform <triple>` | 560             | **present**   |
 * | `cargo tree` (host, default features)       | 538             | absent        |
 *
 * `cargo metadata` keeps the `reqwest -> quinn` edge even though `reqwest`'s
 * enabled feature set contains no `http3` — so `quinn-proto`, a crate that has
 * never been compiled on this machine, counted as built-on-host. It was
 * Preemption's #1 item on 2026-09-07 (HIGH, "version-confirmed"). `cargo tree`
 * excludes it. Same predicate, same answer, as the app's
 * `src-tauri/src/ace/cargo_resolve.rs`.
 *
 * When cargo is unavailable the answer is `null` — *unknown*, never "nothing is
 * inactive". Callers surface that distinction rather than quietly asserting a
 * filter they did not run.
 */
import { execFileSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";

/** Rust target triple for the current host, or null for a platform we do not map. */
export function hostTriple(): string | null {
  const arch =
    process.arch === "x64" ? "x86_64" : process.arch === "arm64" ? "aarch64" : null;
  if (!arch) return null;

  switch (process.platform) {
    case "win32":
      return `${arch}-pc-windows-msvc`;
    case "darwin":
      return `${arch}-apple-darwin`;
    case "linux":
      return `${arch}-unknown-linux-gnu`;
    default:
      return null;
  }
}

/** Per-directory memo. A scan touches the same workspace once per group. */
const cache = new Map<string, Set<string> | null>();

/** Reset the memo. Tests only — the process is short-lived in production. */
export function _resetCargoPlatformCache(): void {
  cache.clear();
}

/**
 * Crate names cargo resolves for this host in the workspace at `dir`.
 *
 * Returns `null` — meaning *unknown*, never "nothing" — when there is no
 * Cargo.toml, no host triple mapping, or cargo is missing/fails. A `null` must
 * never be treated as an empty set: that would mark every crate inactive and
 * silently hide real advisories.
 */
export function activeCratesForHost(dir: string): Set<string> | null {
  const key = path.resolve(dir);
  const memo = cache.get(key);
  if (memo !== undefined) return memo;

  const result = computeActiveCrates(key);
  cache.set(key, result);
  return result;
}

function computeActiveCrates(dir: string): Set<string> | null {
  if (!hostTriple()) return null;
  if (!fs.existsSync(path.join(dir, "Cargo.toml"))) return null;

  let raw: string;
  try {
    raw = execFileSync(
      "cargo",
      [
        "tree",
        // Never touch the network or mutate the lockfile from a read-only
        // scan. `--locked` also means a lockfile out of step with the manifest
        // fails loudly here rather than being silently re-resolved.
        "--offline",
        "--locked",
        // One package per line, no tree glyphs: `name vX.Y.Z [(...)]`.
        "--prefix",
        "none",
        // dev-dependencies are compiled by `cargo test`, so an advisory
        // against one is reachable. proc-macro deps ride along with normal.
        "--edges",
        "normal,build,dev",
      ],
      {
        cwd: dir,
        encoding: "utf8",
        maxBuffer: 64 * 1024 * 1024,
        stdio: ["ignore", "pipe", "ignore"],
        timeout: 30_000,
        windowsHide: true,
      },
    );
  } catch {
    // cargo absent, not a workspace, offline resolution impossible, a stale
    // lockfile, a held package-cache lock, or a timeout.
    return null;
  }

  const names = parseTreeNames(raw);
  // An empty result is not a credible answer for a real workspace; treat it
  // as unknown so nothing gets marked inactive on a parse quirk.
  return names.size > 0 ? names : null;
}

/**
 * Crate names from `cargo tree --prefix none` output.
 *
 * Exported for tests: the parser is the part that can silently degrade to an
 * empty set, and an empty set is the one answer that must never be believed.
 */
export function parseTreeNames(stdout: string): Set<string> {
  const names = new Set<string>();
  for (const line of stdout.split(/\r?\n/)) {
    const token = line.split(/\s+/).find((t) => t.length > 0);
    // A package line starts with a crate name. Anything else — a blank line,
    // a `[dev-dependencies]` header from a future cargo, a warning — is not.
    if (token && /^[A-Za-z0-9._+-]+$/.test(token)) names.add(token);
  }
  return names;
}
