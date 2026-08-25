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
 * `cargo metadata --filter-platform <triple>` resolves the graph for one target
 * and is authoritative: no curation, no heuristics, no guessing. Measured on
 * this repo it returns 561 packages for `x86_64-pc-windows-msvc` against 904 in
 * the lockfile, in ~1.1s, fully offline.
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
  const triple = hostTriple();
  if (!triple) return null;
  if (!fs.existsSync(path.join(dir, "Cargo.toml"))) return null;

  let raw: string;
  try {
    raw = execFileSync(
      "cargo",
      [
        "metadata",
        "--format-version",
        "1",
        // Never touch the network or mutate the lockfile from a read-only scan.
        "--offline",
        "--filter-platform",
        triple,
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
    // cargo absent, not a workspace, offline resolution impossible, or timeout.
    return null;
  }

  try {
    const meta = JSON.parse(raw) as { packages?: Array<{ name?: string }> };
    const names = (meta.packages ?? [])
      .map((p) => p.name)
      .filter((n): n is string => typeof n === "string" && n.length > 0);
    // An empty result is not a credible answer for a real workspace; treat it
    // as unknown so nothing gets marked inactive on a parse quirk.
    return names.length > 0 ? new Set(names) : null;
  } catch {
    return null;
  }
}
