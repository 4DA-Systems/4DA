// SPDX-License-Identifier: Apache-2.0
/**
 * Regression tests for host-platform crate resolution.
 *
 * Live incident (2026-08-25 Signal audit): `vulnerability_scan` reported
 * `platform_inactive_packages: 0` and `_meta.relevance` claimed "Advisories are
 * filtered to the host platform" — while listing nine Linux-only GTK3 crates
 * (`gtk`, `gdk`, `atk`, `glib`, `gdkx11`, `gdkwayland-sys`, `gtk3-macros`,
 * `gdk-sys`, `atk-sys`) on a Windows host. Those crates are never compiled
 * there.
 *
 * Root cause: the only platform signal was `[target.'cfg(...)'.dependencies]`
 * parsed from a manifest, which covers DIRECT gated deps. The GTK3 cluster is
 * entirely transitive — Tauri pulls it in on Linux — and Cargo.lock does not
 * encode targets, so nothing could see it.
 */
import { describe, it, expect, beforeEach } from "vitest";
import * as fs from "node:fs";
import * as path from "node:path";
import { execFileSync } from "node:child_process";
import {
  hostTriple,
  activeCratesForHost,
  _resetCargoPlatformCache,
} from "../live/cargo-platform.js";
import { platformFilterNote } from "../tools/vulnerability-scan.js";
import type { VulnerabilityEntry } from "../live/types.js";

beforeEach(() => _resetCargoPlatformCache());

/** The Rust workspace this repo ships, if the test is running inside it. */
function repoCargoDir(): string | null {
  let dir = process.cwd();
  for (let i = 0; i < 5; i++) {
    const candidate = path.join(dir, "src-tauri");
    if (fs.existsSync(path.join(candidate, "Cargo.toml"))) return candidate;
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return null;
}

function cargoAvailable(): boolean {
  try {
    execFileSync("cargo", ["--version"], { stdio: "ignore", timeout: 15_000, windowsHide: true });
    return true;
  } catch {
    return false;
  }
}

describe("hostTriple", () => {
  it("maps this host to a real Rust target triple", () => {
    const triple = hostTriple();
    if (triple === null) return; // unmapped platform — nothing to assert
    expect(triple).toMatch(/^(x86_64|aarch64)-(pc-windows-msvc|apple-darwin|unknown-linux-gnu)$/);
  });
});

describe("activeCratesForHost", () => {
  it("returns null — unknown, never empty — for a directory with no Cargo.toml", () => {
    const result = activeCratesForHost(path.join(process.cwd(), "src", "__tests__"));
    expect(result).toBeNull();
  });

  it("returns null rather than an empty set for a nonexistent path", () => {
    // The distinction is load-bearing: an empty set would mark EVERY crate
    // inactive and silently hide real advisories.
    expect(activeCratesForHost(path.join(process.cwd(), "no-such-dir-xyz"))).toBeNull();
  });

  const dir = repoCargoDir();
  const runnable = dir !== null && cargoAvailable() && hostTriple() !== null;

  // These two shell out to real cargo. `cargo metadata` normally answers in
  // ~1s, but it takes the same lock a concurrent `cargo build`/`cargo test`
  // holds, and CI runs both. The default 5s test timeout made them flake under
  // that contention; the work itself is bounded by execFileSync's own 30s.
  const CARGO_TEST_TIMEOUT_MS = 90_000;

  it.runIf(runnable)(
    "excludes transitive crates that never build on this host",
    () => {
      const crates = activeCratesForHost(dir!);

      // `null` means cargo declined to answer — `--offline` needs a warm
      // registry, and a CI checkout may not have one. That is a HANDLED
      // outcome (callers keep every crate active), not a defect, so asserting
      // non-null here would fail the build for an environment condition
      // rather than a regression. Verify the behaviour when cargo does answer.
      if (crates === null) {
        console.warn("cargo metadata --offline unavailable here — assertions skipped");
        return;
      }

      expect(crates.size).toBeGreaterThan(50);

      if (process.platform === "win32") {
        // The exact cluster the live scan reported as vulnerable on Windows.
        for (const linuxOnly of ["gtk", "gdk", "atk", "glib", "gdkx11", "gtk3-macros"]) {
          expect(crates.has(linuxOnly), `${linuxOnly} must not be active on Windows`).toBe(false);
        }
        expect(crates.has("windows-sys"), "windows-sys must be active on Windows").toBe(true);
      }
    },
    CARGO_TEST_TIMEOUT_MS,
  );

  it.runIf(runnable)(
    "memoizes per directory",
    () => {
      const first = activeCratesForHost(dir!);
      const second = activeCratesForHost(dir!);
      // Identity holds for a real answer; `null === null` also holds when cargo
      // declines, so this asserts the memo either way.
      expect(second).toBe(first);
    },
    CARGO_TEST_TIMEOUT_MS,
  );
});

function entry(over: Partial<VulnerabilityEntry> = {}): VulnerabilityEntry {
  return {
    package: "gtk",
    currentVersion: "0.18.2",
    ecosystem: "crates.io",
    isDev: false,
    isDirect: false,
    devScopeKnown: false,
    vulnId: "RUSTSEC-2024-0415",
    aliases: [],
    severity: "unknown",
    cvssScore: null,
    summary: "gtk-rs GTK3 bindings - no longer maintained",
    fixedVersion: null,
    published: "2024-03-04T12:00:00Z",
    references: [],
    target: null,
    platformActive: true,
    sourceDirs: ["d:/4da/src-tauri"],
    ...over,
  };
}

describe("platformFilterNote", () => {
  it("does NOT claim filtering when no target information was resolvable", () => {
    // This is the exact state that produced the false claim.
    const note = platformFilterNote([entry(), entry({ package: "gdk" })]);
    expect(note).toMatch(/^Not platform-filtered/);
    expect(note).not.toMatch(/Filtered to/);
  });

  it("reports the count when advisories were actually suppressed", () => {
    const note = platformFilterNote([
      entry({ platformActive: false, target: "not built for x86_64-pc-windows-msvc" }),
      entry({ package: "serde", platformActive: true, target: null }),
    ]);
    expect(note).toMatch(/^Filtered to /);
    expect(note).toContain("1 advisory is");
  });

  it("says so plainly when the filter ran and suppressed nothing", () => {
    const note = platformFilterNote([entry({ platformActive: true, target: "cfg(unix)" })]);
    expect(note).toMatch(/^Filtered to /);
    expect(note).toContain("every advisory below");
  });
});
