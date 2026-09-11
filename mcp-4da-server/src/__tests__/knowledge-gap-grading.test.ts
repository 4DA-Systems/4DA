// SPDX-License-Identifier: Apache-2.0
/**
 * Regression tests for knowledge-gap severity grading.
 *
 * Live incident (2026-08-25 Signal audit): `knowledge_gaps` returned 18 gaps of
 * which one was useful, while the app's own Knowledge Gaps panel showed "No
 * gaps detected — your knowledge is current" over the SAME database. Two
 * implementations of one concept, disagreeing in both directions.
 *
 * The noise came from grading `medium` on mention COUNT, where a mention could
 * match the content body rather than the title. Every fixture below is a real
 * title copied from the live corpus at the ids named in the comments.
 *
 * 2026-09-11: the security tier now comes from the advisory itself
 * (`knowledge_decay::classify_severity` + `advisory_tier_for`, AD-040 rule 2):
 * a still-reaching advisory is `critical` only when the most severe advisory
 * reaching an install is critical or high, and `high` otherwise — ungraded
 * included. Expectations that assumed "any advisory is critical" now pass
 * that tier explicitly.
 */
import { describe, it, expect } from "vitest";
import { gradeGap, type GradableItem } from "../tools/knowledge-gaps.js";

const item = (title: string, source_type = "hackernews", content_type: string | null = null): GradableItem => ({
  title,
  source_type,
  content_type,
});

describe("gradeGap — the signal that must survive", () => {
  it("grades a real advisory about the dependency at the security tier", () => {
    // ids 192/193/194: the Hono CVEs the app reported as "no gaps".
    const items = [
      item("[CVE-2026-71850] Hono: `memo()` retains SSR output across requests", "cve"),
      item("[CVE-2026-71849] Hono: Proxy Helper does not remove response headers", "cve"),
      item("[CVE-2026-71848] Hono: Algorithmic Complexity DoS in Language Middleware", "cve"),
    ];
    // Ungraded it is `high`; `critical` needs the advisory's own critical or high tier.
    expect(gradeGap(items, "hono")).toBe("high");
    expect(gradeGap(items, "hono", true, null, "high")).toBe("critical");
    expect(gradeGap(items, "hono", true, null, "medium")).toBe("high");
  });

  it("grades an OSV advisory the same way", () => {
    const items = [item("[GHSA-gcfj-64vw-6mp9] axios: inherited proxy after config cloning", "osv")];
    expect(gradeGap(items, "axios")).toBe("high");
    expect(gradeGap(items, "axios", true, null, "critical")).toBe("critical");
  });

  it("an editorial security story is a citation, never proof of exposure", () => {
    // AD-040 rule 4 / `knowledge_decay::grounded_security_advisory`: only an
    // osv/cve advisory can carry a gap to the security tiers. This mastodon
    // post used to grade `high` on the word "Security".
    const items = [item("This Week in Security: Stripe Merchants Leak Keys", "mastodon")];
    expect(gradeGap(items, "stripe")).toBe("low");
  });

  it("grades a release that names the dep as medium", () => {
    const items = [item("Announcing axum 0.8.0", "rss")];
    expect(gradeGap(items, "axum")).toBe("medium");
  });

  it("grades a breaking change as high", () => {
    // `knowledge_decay::classify_severity`: a breaking citation is High.
    const items = [
      item("TypeScript 6.0 Strict Function Types: Why Contravariance Breaking Your Callbacks", "devto"),
    ];
    expect(gradeGap(items, "typescript")).toBe("high");
  });
});

describe("gradeGap — the noise that must not survive", () => {
  // Every case below was reported as a `medium` gap by the live tool, and
  // `min_severity` defaults to medium, so every one of them shipped.

  it("drops a `tracing` gap evidenced by unrelated articles", () => {
    // ids 40287 / 36778 / 35743 / 34654 / 32742 — none names the crate.
    const items = [
      item("Spike-Killer: Evidence-Gated LLM Assistance for Safe Performance Diagnosis", "arxiv"),
      item("The Matrix: Writing Code That Doesn't Need Comments", "devto"),
      item("New write-up: Reading JS Files Like an Attacker: Three Sources Still Worth Knowing", "mastodon"),
      item("Show HN: Traccia - Observability, Runtime Control & Audit for agents"),
      item("I built a new thing, an idea I have wanted to try for a long time", "mastodon"),
    ];
    expect(gradeGap(items, "tracing")).toBe("low");
  });

  it("drops a `typescript` gap evidenced by a job posting", () => {
    // id 41234 — names the language, carries no consequence whatsoever.
    const items = [
      item("Databricks is hiring Senior Forward Deployed Engineer (FDE) - Retail javascript python scala typescript unity aws", "mastodon"),
    ];
    expect(gradeGap(items, "typescript")).toBe("low");
  });

  it("drops a `uuid` gap evidenced by Go's standard library", () => {
    // ids 35802 / 29666 / 28727 — about UUIDs, useless to a Rust `uuid` user.
    const items = [
      item("Go 1.27 is out and it comes with a bunch of cool stuff, the one I'm most excited about is the UUID being part of the stdlib", "mastodon"),
      item("Go 1.27 introduces a UUID package to the standard library", "mastodon"),
      item("Manticore Search 28.6.6: UUID document IDs, ordered GROUP_CONCAT(), and 16 fixes"),
    ];
    expect(gradeGap(items, "uuid")).toBe("low");
  });

  it("drops a `vite` gap evidenced by a marketplace build-log", () => {
    // id 34127 names Vite but reports no release, deprecation or break.
    const items = [
      item("How I Built Drs Kart: Building a B2B Medical Equipment Marketplace with React, Vite and Supabase", "devto"),
      item("Gea – A new JavaScript framework for old time's sake"),
      item("crates.io: axum_marko_build v0.1.0", "crates_io"),
    ];
    expect(gradeGap(items, "vite")).toBe("low");
  });

  it("volume alone never reaches medium", () => {
    // The precise rule change: five passing mentions used to be a gap.
    const items = Array.from({ length: 5 }, (_, i) =>
      item(`Someone mentions hono in passing, part ${i}`),
    );
    expect(gradeGap(items, "hono")).toBe("low");
  });

  it("a body-only mention cannot grade the gap", () => {
    // Mentions match the content head too; only the TITLE may grade.
    expect(gradeGap([item("An article about something else entirely")], "hono")).toBe("low");
  });

  it("an advisory that does not name the dep is not that dep's gap", () => {
    // Co-mention in a security roundup must not mint a critical gap.
    expect(gradeGap([item("[CVE-2026-0001] lodash prototype pollution", "cve")], "hono")).toBe("low");
  });

  it("word-boundary holds — `hono` is not `phonograph`", () => {
    expect(gradeGap([item("Announcing a new phonograph release", "rss")], "hono")).toBe("low");
  });

  it("handles an empty item list", () => {
    expect(gradeGap([], "hono")).toBe("low");
  });
});

describe("gradeGap — registry rows are version updates", () => {
  // A registry row carries no consequence keyword in its title ("crates.io:
  // serde v1.0.220"), so it graded `low` and an unread release of a direct
  // dependency — the thing the Rust surface counts as substantive — was
  // invisible at the default `medium` floor.
  it("grades a crates.io release of the dependency as medium", () => {
    expect(gradeGap([item("crates.io: serde v1.0.220", "crates_io")], "serde")).toBe("medium");
  });

  it("grades npm, PyPI and Go registry rows the same way", () => {
    expect(gradeGap([item("npm: react@19.3.0", "npm_registry")], "react")).toBe("medium");
    expect(gradeGap([item("pypi: requests 2.33.0", "pypi")], "requests")).toBe("medium");
    expect(gradeGap([item("go: golang.org/x/net v0.45.0", "go_modules")], "golang.org/x/net")).toBe("medium");
  });

  it("a registry row about a different package is not this dependency's gap", () => {
    expect(gradeGap([item("crates.io: axum_marko_build v0.1.0", "crates_io")], "axum")).toBe("low");
  });

  it("an osv or cve row is an advisory regardless of its title words", () => {
    // The SOURCE makes it an advisory, not a security keyword in the title.
    expect(gradeGap([item("[GHSA-f23p-vx2j-j53r] hono: memo() retains SSR output across requests", "osv")], "hono")).toBe("high");
    expect(gradeGap([item("[CVE-2026-71849] Hono: Proxy Helper does not remove response headers", "cve")], "hono")).toBe("high");
  });

  it("a registry row for the installed version is not an update", () => {
    // Live: "npm: @tauri-apps/api v2.11.1" graded a medium gap on 2.11.1.
    expect(gradeGap([item("npm: @tauri-apps/api v2.11.1", "npm_registry")], "@tauri-apps/api", true, "2.11.1")).toBe("low");
    expect(gradeGap([item("npm: @tauri-apps/api v2.11.1", "npm_registry")], "@tauri-apps/api", true, "2.12.0")).toBe("low");
    expect(gradeGap([item("npm: @tauri-apps/api v2.11.2", "npm_registry")], "@tauri-apps/api", true, "2.11.1")).toBe("medium");
    // Unknown installed version keeps the conservative grade.
    expect(gradeGap([item("npm: @tauri-apps/api v2.11.1", "npm_registry")], "@tauri-apps/api")).toBe("medium");
  });

  it("a release is new while any carrying project runs an older version, by semver precedence", () => {
    // AD-041: new against EVERY install; 0.10.0-rc.19 is newer than rc.18.
    const rc19 = [item("crates.io: rsa v0.10.0-rc.19", "crates_io")];
    expect(gradeGap(rc19, "rsa", true, ["0.10.0-rc.18"])).toBe("medium");
    expect(gradeGap(rc19, "rsa", true, ["0.10.0-rc.19", "0.10.0"])).toBe("low");
    expect(gradeGap(rc19, "rsa", true, ["0.10.0", "0.9.10"])).toBe("medium");
  });
});

describe("gradeGap — an advisory names the dependency by its SUBJECT package", () => {
  // Live 2026-09-07: `url` graded critical on a SurrealDB advisory whose
  // title said "via URL path"; `hmac` on a Phalcon advisory about HMAC
  // verification. Both real advisories, neither about the dependency.
  it("does not mint a security gap from an advisory about another package", () => {
    expect(
      gradeGap([item("[CVE-2026-63735] SurrealDB: Custom API route lets authenticated callers override namespace/database scope via URL path", "cve")], "url"),
    ).toBe("low");
    expect(
      gradeGap([item("[CVE-2026-54736] Phalcon: Non-constant-time HMAC verification in `Encryption\\Crypt::decrypt` (timing side-channel)", "cve")], "hmac"),
    ).toBe("low");
  });

  it("still grades an advisory whose subject IS the dependency at the security tier", () => {
    expect(gradeGap([item("[CVE-2026-71850] Hono: `memo()` retains SSR output across requests", "cve")], "hono")).toBe("high");
    expect(gradeGap([item("[RUSTSEC-2023-0071] rsa: Marvin Attack: potential key recovery through timing sidechannels", "osv")], "rsa")).toBe("high");
    // crates.io `-`/`_` are one namespace.
    expect(gradeGap([item("[GHSA-x] http-body-util: unbounded buffering", "osv")], "http_body_util", true, null, "critical")).toBe("critical");
  });

  it("never falls back to a title word for an advisory row", () => {
    // Measured 2026-09-11: "[CVE-2026-63642] MagicMirror newsfeed Socket.IO
    // notification ..." has no subject package, and the word fallback minted
    // a critical socket.io gap from it. An advisory row with no readable
    // subject cites a dependency only through the linker's structured proof.
    expect(gradeGap([item("CVE-2024-1234: Critical security vulnerability in express", "cve")], "express")).toBe("low");
    expect(
      gradeGap([item("[CVE-2026-63642] MagicMirror newsfeed Socket.IO notification allows blind server-side request forgery", "cve")], "socket.io"),
    ).toBe("low");
    expect(gradeGap([item("hono 4.12.34 memo() retains SSR output", "osv")], "hono")).toBe("low");
  });
});

describe("gradeGap — release announcements (aligned with content_dna_classifiers)", () => {
  it("keeps an announcement that carries a version", () => {
    expect(gradeGap([item("Announcing axum 0.8.0", "rss")], "axum")).toBe("medium");
    expect(gradeGap([item("Announcing TypeScript 6.0 Beta", "rss")], "typescript")).toBe("medium");
  });

  it("rejects an announcement with no version — a launch, not a release", () => {
    // The exact carve-out content_dna settled on: project launches and company
    // news use the same verb and are not a version you are behind on.
    expect(
      gradeGap([item("Announcing Toasty, an async ORM for Rust, is now on crates.io", "rss")], "toasty"),
    ).toBe("low");
  });

  it("does not let a bare version number in any title imply consequence", () => {
    // Version literals are everywhere; only the announcement phrasing counts.
    expect(
      gradeGap([item("Manticore Search 28.6.6: UUID document IDs and 16 fixes")], "uuid"),
    ).toBe("low");
  });
});

// ---------------------------------------------------------------------------
// Already-patched suppression
//
// The audit's own headline claim was WRONG on impact, and this is the code that
// makes it wrong. `knowledge_gaps` graded the three Hono CVEs `critical` on
// hono 4.13.2 — every one of them is fixed in 4.12.34, and this repo had
// already pinned past it via `pnpm.overrides` ("hono": ">=4.12.34 <5"). The
// tool never compared the installed version against the fix.
// ---------------------------------------------------------------------------

import { versionInAnyRange, type AdvisoryRangeEvent } from "../tools/knowledge-gaps.js";

/** The real OSV ranges for the three Hono advisories, verbatim. */
const HONO_RANGES: AdvisoryRangeEvent[][] = [
  [{ introduced: "3.8.0" }, { fixed: "4.12.34" }], // GHSA-f23p-vx2j-j53r
  [{ introduced: "4.7.0" }, { fixed: "4.12.34" }], // GHSA-79qm-7rj5-m7r9
  [{ introduced: "4.12.0" }, { fixed: "4.12.34" }], // GHSA-54fx-42gc-7vw4
];

describe("versionInAnyRange", () => {
  it("reports the installed version as SAFE when it is past every fix", () => {
    // 4.13.2 — the version actually installed when the audit called this critical.
    expect(versionInAnyRange(HONO_RANGES, "4.13.2")).toBe(false);
    expect(versionInAnyRange(HONO_RANGES, "4.12.34")).toBe(false);
  });

  it("reports a genuinely behind version as affected", () => {
    expect(versionInAnyRange(HONO_RANGES, "4.12.33")).toBe(true);
    expect(versionInAnyRange(HONO_RANGES, "4.8.0")).toBe(true);
  });

  it("treats a version below every `introduced` as unaffected", () => {
    expect(versionInAnyRange(HONO_RANGES, "3.0.0")).toBe(false);
  });

  it("handles an open-ended range with no fix", () => {
    const unfixed: AdvisoryRangeEvent[][] = [[{ introduced: "1.0.0" }]];
    expect(versionInAnyRange(unfixed, "2.0.0")).toBe(true);
    expect(versionInAnyRange(unfixed, "0.9.0")).toBe(false);
  });

  it("treats `introduced: 0` as affecting everything below the fix", () => {
    const fromZero: AdvisoryRangeEvent[][] = [[{ introduced: "0" }, { fixed: "4.12.34" }]];
    expect(versionInAnyRange(fromZero, "1.0.0")).toBe(true);
    expect(versionInAnyRange(fromZero, "4.13.2")).toBe(false);
  });

  it("never claims safety on missing or unreadable version data", () => {
    expect(versionInAnyRange(HONO_RANGES, null)).toBe(true);
    expect(versionInAnyRange(HONO_RANGES, undefined)).toBe(true);
    expect(versionInAnyRange(HONO_RANGES, "not-a-version")).toBe(true);
    expect(versionInAnyRange([], "4.13.2")).toBe(false);
  });
});

describe("gradeGap — already-patched dependencies", () => {
  const honoCves = [
    item("[CVE-2026-71850] Hono: `memo()` retains SSR output across requests", "cve"),
    item("[CVE-2026-71849] Hono: Proxy Helper does not remove response headers", "cve"),
    item("[CVE-2026-71848] Hono: Algorithmic Complexity DoS in Language Middleware", "cve"),
  ];

  it("does not raise a security grade for a dependency already past the fix", () => {
    // The exact live case. Was `critical`; must not be.
    expect(gradeGap(honoCves, "hono", versionInAnyRange(HONO_RANGES, "4.13.2"))).toBe("low");
  });

  it("still raises the security tier when the install really is behind", () => {
    expect(gradeGap(honoCves, "hono", versionInAnyRange(HONO_RANGES, "4.12.0"))).toBe("high");
    expect(gradeGap(honoCves, "hono", versionInAnyRange(HONO_RANGES, "4.12.0"), "4.12.0", "critical")).toBe("critical");
  });

  it("defaults to the conservative grade when no version data is passed", () => {
    // Callers without version information must not silently lose the alert.
    expect(gradeGap(honoCves, "hono")).toBe("high");
  });
});
