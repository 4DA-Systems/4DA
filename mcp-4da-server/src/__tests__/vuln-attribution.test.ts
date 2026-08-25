// SPDX-License-Identifier: Apache-2.0
/**
 * Regression tests for vulnerability_scan attribution and alias collapsing.
 *
 * Live incident (2026-08-25 Signal audit): `vulnerability_scan` reported
 * `jsonwebtoken 9.3.1` as a DIRECT dependency of 4DA with an authorization
 * bypass. 4DA is on 10.4.0 — already past the 10.3.0 fix — and `cargo audit`
 * on its lockfile was clean (904 crates, exit 0). The vulnerable pin belonged
 * to `relay/`, a separate workspace under the same repo root. Six of the top
 * seven recommendations were against versions the named project did not have.
 *
 * Root cause: `dedupeDependencies` collapsed on (ecosystem, name, version) and
 * kept no record of WHICH manifest directory resolved each version, so the
 * report could not distinguish a finding in the primary crate from one in a
 * sibling workspace.
 *
 * Second defect in the same output: OSV returns GHSA and RUSTSEC records for
 * one vulnerability, each naming the other in `aliases`. Reported separately
 * they doubled the count — `quinn-proto` read as "2 high" for one bug.
 */
import { describe, it, expect } from "vitest";
import { collapseAliases, relativeDir, isMaintenanceNotice } from "../tools/vulnerability-scan.js";
import type { VulnerabilityEntry } from "../live/types.js";

function entry(over: Partial<VulnerabilityEntry> = {}): VulnerabilityEntry {
  return {
    package: "quinn-proto",
    currentVersion: "0.11.14",
    ecosystem: "crates.io",
    isDev: false,
    isDirect: false,
    devScopeKnown: false,
    vulnId: "GHSA-4w2j-m93h-cj5j",
    aliases: [],
    severity: "high",
    cvssScore: 7.5,
    summary: "Remote memory exhaustion",
    fixedVersion: "0.11.15",
    published: "2026-07-24T14:07:54Z",
    references: [],
    target: null,
    platformActive: true,
    sourceDirs: ["d:/4da/victauri-gauntlet"],
    ...over,
  };
}

describe("collapseAliases", () => {
  it("folds a GHSA/RUSTSEC pair for one bug into a single finding", () => {
    const collapsed = collapseAliases([
      entry({ vulnId: "GHSA-4w2j-m93h-cj5j", aliases: ["CVE-2026-25800", "RUSTSEC-2026-0185"] }),
      entry({ vulnId: "RUSTSEC-2026-0185", aliases: ["CVE-2026-25800", "GHSA-4w2j-m93h-cj5j"] }),
    ]);

    expect(collapsed).toHaveLength(1);
    expect(collapsed[0].aliases).toContain("RUSTSEC-2026-0185");
    expect(collapsed[0].aliases).not.toContain(collapsed[0].vulnId);
  });

  it("keeps the record carrying the most usable detail", () => {
    const collapsed = collapseAliases([
      // Un-hydrated: no score, no fix.
      entry({ vulnId: "RUSTSEC-2024-0429", aliases: ["GHSA-wrw7-89jp-8q8g"], cvssScore: null, fixedVersion: null, severity: "unknown" }),
      entry({ vulnId: "GHSA-wrw7-89jp-8q8g", aliases: ["RUSTSEC-2024-0429"], cvssScore: 5.9, fixedVersion: "0.20.0", severity: "medium" }),
    ]);

    expect(collapsed).toHaveLength(1);
    expect(collapsed[0].cvssScore).toBe(5.9);
    expect(collapsed[0].fixedVersion).toBe("0.20.0");
  });

  it("never merges across packages, versions, or ecosystems", () => {
    const collapsed = collapseAliases([
      entry({ vulnId: "GHSA-shared", package: "a" }),
      entry({ vulnId: "GHSA-shared", package: "b" }),
      // Same advisory id, different pinned version = a separate thing to fix.
      entry({ vulnId: "GHSA-v", currentVersion: "1.0.0" }),
      entry({ vulnId: "GHSA-v", currentVersion: "2.0.0" }),
    ]);
    expect(collapsed).toHaveLength(4);
  });

  it("leaves genuinely distinct advisories on one package alone", () => {
    const collapsed = collapseAliases([
      entry({ vulnId: "GHSA-one", aliases: [] }),
      entry({ vulnId: "GHSA-two", aliases: [] }),
    ]);
    expect(collapsed).toHaveLength(2);
  });

  it("preserves provenance through the collapse", () => {
    const collapsed = collapseAliases([
      entry({ vulnId: "GHSA-x", aliases: ["RUSTSEC-x"], sourceDirs: ["d:/4da/relay"] }),
      entry({ vulnId: "RUSTSEC-x", aliases: ["GHSA-x"], sourceDirs: ["d:/4da/relay"] }),
    ]);
    expect(collapsed[0].sourceDirs).toEqual(["d:/4da/relay"]);
  });
});

describe("relativeDir", () => {
  it("names a sibling workspace relative to the scan root", () => {
    expect(relativeDir("d:/4da/relay", "d:/4da")).toBe("relay");
    expect(relativeDir("d:/4da/victauri-gauntlet", "d:/4da")).toBe("victauri-gauntlet");
    expect(relativeDir("d:/4da/editors/vscode/4da", "d:/4da")).toBe("editors/vscode/4da");
  });

  it("marks the root itself", () => {
    expect(relativeDir("d:/4da", "d:/4da")).toBe(".");
    expect(relativeDir("d:/4da/", "d:/4da")).toBe(".");
  });

  it("tolerates Windows separators and case", () => {
    expect(relativeDir("D:\\4DA\\src-tauri", "d:/4da")).toBe("src-tauri");
  });

  it("falls back to the absolute path when outside the root", () => {
    expect(relativeDir("c:/users/x/other", "d:/4da")).toBe("c:/users/x/other");
  });

  /**
   * The incident in one assertion: two workspaces pin different versions of
   * one crate, and the report must be able to tell them apart.
   */
  it("distinguishes the patched primary from the lagging sibling", () => {
    const root = "d:/4da";
    const vulnerable = entry({ currentVersion: "0.11.14", sourceDirs: ["d:/4da/victauri-gauntlet"] });
    expect(relativeDir(vulnerable.sourceDirs[0], root)).toBe("victauri-gauntlet");
    expect(relativeDir(vulnerable.sourceDirs[0], root)).not.toBe("src-tauri");
  });
});

describe("isMaintenanceNotice", () => {
  // Live scan: 23 of 41 findings were unmaintained-dependency notices, which
  // generated 27 "Review X" recommendations around six real upgrades.
  it("classifies RustSec unmaintained advisories", () => {
    for (const summary of [
      "gtk-rs GTK3 bindings - no longer maintained",
      "paste - no longer maintained",
      "proc-macro-error is unmaintained",
      "`ttf-parser` is unmaintained",
      "`unic-char-property` is unmaintained",
    ]) {
      expect(isMaintenanceNotice(entry({ summary })), summary).toBe(true);
    }
  });

  it("never reclassifies a real vulnerability as maintenance", () => {
    for (const summary of [
      "Quinn: Remote memory exhaustion in quinn-proto from unbounded reassembly",
      "rust-openssl has undefined behavior in X509Ref::ocsp_responders",
      "Marvin Attack: potential key recovery through timing sidechannels",
      "jsonwebtoken has Type Confusion that leads to potential authorization bypass",
      // "deprecated" is deliberately NOT a trigger — it appears in real CVEs.
      "Use of a deprecated cipher allows plaintext recovery",
    ]) {
      expect(isMaintenanceNotice(entry({ summary })), summary).toBe(false);
    }
  });

  it("tolerates a missing summary", () => {
    expect(isMaintenanceNotice(entry({ summary: "" }))).toBe(false);
  });
});
