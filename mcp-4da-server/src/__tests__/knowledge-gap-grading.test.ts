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
 */
import { describe, it, expect } from "vitest";
import { gradeGap, type GradableItem } from "../tools/knowledge-gaps.js";

const item = (title: string, source_type = "hackernews", content_type: string | null = null): GradableItem => ({
  title,
  source_type,
  content_type,
});

describe("gradeGap — the signal that must survive", () => {
  it("grades a real advisory about the dependency as critical", () => {
    // ids 192/193/194: the Hono CVEs the app reported as "no gaps".
    const items = [
      item("[CVE-2026-71850] Hono: `memo()` retains SSR output across requests", "cve"),
      item("[CVE-2026-71849] Hono: Proxy Helper does not remove response headers", "cve"),
      item("[CVE-2026-71848] Hono: Algorithmic Complexity DoS in Language Middleware", "cve"),
    ];
    expect(gradeGap(items, "hono")).toBe("critical");
  });

  it("grades an OSV advisory as critical too", () => {
    const items = [item("[GHSA-gcfj-64vw-6mp9] axios: inherited proxy after config cloning", "osv")];
    expect(gradeGap(items, "axios")).toBe("critical");
  });

  it("grades a security-keyword item naming the dep as high", () => {
    const items = [item("This Week in Security: Stripe Merchants Leak Keys", "mastodon")];
    expect(gradeGap(items, "stripe")).toBe("high");
  });

  it("grades a release that names the dep as medium", () => {
    const items = [item("Announcing axum 0.8.0", "rss")];
    expect(gradeGap(items, "axum")).toBe("medium");
  });

  it("grades a breaking change as medium", () => {
    const items = [
      item("TypeScript 6.0 Strict Function Types: Why Contravariance Breaking Your Callbacks", "devto"),
    ];
    expect(gradeGap(items, "typescript")).toBe("medium");
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
