// SPDX-License-Identifier: Apache-2.0
/**
 * Changelog discovery, parsing, range selection, classification and
 * sanitisation. The heading styles are the ones measured in real registry
 * archives (tokio, axum, reqwest CHANGELOG.md; express History.md) plus the
 * keep-a-changelog and conventional-changelog generators.
 */

import { describe, it, expect } from "vitest";
import {
  findChangelogFile,
  isChangelogName,
  parseChangelog,
  parseVersionHeading,
  selectRange,
} from "../live/changelog.js";
import { classifyEntry, classifyHeading, classifyText, sanitizeEntry } from "../live/changelog-classify.js";

describe("changelog file discovery", () => {
  it("recognises the conventional names case-insensitively", () => {
    for (const name of ["CHANGELOG.md", "changelog", "History.md", "CHANGES.rst", "RELEASES.markdown", "NEWS.txt", "RELEASE-NOTES.md", "release_notes.md"]) {
      expect(isChangelogName(name)).toBe(true);
    }
    for (const name of ["README.md", "CHANGELOG.json", "changelog-old.md"]) {
      expect(isChangelogName(name)).toBe(false);
    }
  });

  it("prefers CHANGELOG over HISTORY and markdown over plain text", () => {
    expect(findChangelogFile(["package/History.md", "package/CHANGELOG.md"])).toBe("package/CHANGELOG.md");
    expect(findChangelogFile(["x-1.0.0/CHANGELOG", "x-1.0.0/CHANGELOG.md"])).toBe("x-1.0.0/CHANGELOG.md");
    expect(findChangelogFile(["package/README.md"])).toBeNull();
  });
});

describe("parseVersionHeading", () => {
  it.each([
    ["[1.2.3] - 2024-01-01", "1.2.3", "2024-01-01"],
    ["1.2.3", "1.2.3", null],
    ["v1.2.3 (2024-01-01)", "1.2.3", "2024-01-01"],
    ["Version 1.2.3", "1.2.3", null],
    ["[v7.0.0](https://github.com/x/y/compare/v6.0.0...v7.0.0) (2025-06-24)", "7.0.0", "2025-06-24"],
    ["[7.0.0](https://github.com/x/y/compare/v6.3.5...v7.0.0) (2025-06-24)", "7.0.0", "2025-06-24"],
    ["1.0.0-rc.1", "1.0.0-rc.1", null],
    ["Tokio 1.47.1 (August 1st, 2025)", "1.47.1", "2025-08-01"],
    ["fastembed v5.0.0", "5.0.0", null],
    ["0.4", "0.4", null],
    ["[0.8.2] - 2025-01-01 [YANKED]", "0.8.2", "2025-01-01"],
  ])("reads %s", (heading, version, date) => {
    expect(parseVersionHeading(heading)).toEqual({ version, date });
  });

  it.each(["Bug Fixes", "Upgrading from 1.x to 2.0 is easy with these steps", "Rust 1.70 support", "Unreleased"])(
    "does not treat %s as a release",
    (heading) => {
      expect(parseVersionHeading(heading)).toBeNull();
    },
  );
});

describe("parseChangelog", () => {
  it("keeps a category heading at the release's own level inside the release (date-fns 3.0.0)", () => {
    const sections = parseChangelog(
      [
        "# Change Log",
        "## v3.0.0 - 2023-12-03",
        "## Changed",
        "- **BREAKING**: date-fns is now a dual-package with the support of both ESM and CommonJS.",
        "### Added",
        "- New `constants` export",
        "## v2.30.0",
        "### Changes",
        "- Fix a thing",
        "## Migration guide",
        "- Not part of any release",
      ].join("\n"),
    );
    expect(sections.map((s) => s.version)).toEqual(["3.0.0", "2.30.0"]);
    expect(sections[0].entries.map((e) => e.kind)).toEqual(["breaking", "change"]);
    expect(sections[1].entries.map((e) => e.text)).toEqual(["Fix a thing"]);
  });

  it("skips HTML comments, including ones spanning lines", () => {
    const sections = parseChangelog(
      [
        "## 2.0.0",
        "<!--",
        "- **breaking:** template placeholder, not a real entry",
        "-->",
        "<!-- one-line note -->",
        "- Real change",
      ].join("\n"),
    );
    expect(sections).toHaveLength(1);
    expect(sections[0].entries.map((e) => e.text)).toEqual(["Real change"]);
  });

  it("parses keep-a-changelog with typed sub-sections", () => {
    const sections = parseChangelog(
      [
        "# Changelog",
        "## [Unreleased]",
        "- pending thing",
        "## [2.0.0] - 2024-03-01",
        "### Added",
        "- New `parse` API",
        "### Removed",
        "- The `legacyParse` function",
        "### Deprecated",
        "- `oldOption` flag",
        "### Security",
        "- Fix prototype pollution",
        "## [1.1.0] - 2024-01-01",
        "### Fixed",
        "- Crash on empty input",
        "  that spanned two lines",
        "",
        "[2.0.0]: https://example.com/compare/v1.1.0...v2.0.0",
      ].join("\n"),
    );
    expect(sections.map((s) => s.version)).toEqual(["2.0.0", "1.1.0"]);
    expect(sections[0].date).toBe("2024-03-01");
    expect(sections[0].entries.map((e) => e.kind)).toEqual(["change", "breaking", "deprecation", "security"]);
    expect(sections[1].entries).toEqual([{ kind: "change", text: "Crash on empty input that spanned two lines" }]);
  });

  it("parses conventional-changelog output with a BREAKING CHANGES block", () => {
    const sections = parseChangelog(
      [
        "# [7.0.0](https://github.com/o/r/compare/v6.3.5...v7.0.0) (2025-06-24)",
        "",
        "### ⚠ BREAKING CHANGES",
        "",
        "* drop Node 18",
        "* **config:** `foo` is gone",
        "",
        "### Features",
        "",
        "* add `bar` ([abc123](https://github.com/o/r/commit/abc123))",
        "",
        "## [6.3.5](https://github.com/o/r/compare/v6.3.4...v6.3.5) (2025-05-01)",
        "",
        "### Bug Fixes",
        "",
        "* fix CVE-2025-1234 in path handling",
      ].join("\n"),
    );
    expect(sections.map((s) => [s.version, s.date])).toEqual([["7.0.0", "2025-06-24"], ["6.3.5", "2025-05-01"]]);
    expect(sections[0].entries.map((e) => e.kind)).toEqual(["breaking", "breaking", "change"]);
    expect(sections[1].entries[0].kind).toBe("security");
  });

  it("parses setext headings and History.md `version / date` lines", () => {
    const setext = parseChangelog("1.2.0\n=====\n\n* feature\n\n1.1.0\n-----\n\n* fix\n");
    expect(setext.map((s) => s.version)).toEqual(["1.2.0", "1.1.0"]);
    const history = parseChangelog(
      "4.21.2 / 2024-11-06\n==========\n\n  * deps: path-to-regexp@0.1.12\n\n4.21.1 / 2024-10-08\n\n  * Backport fix\n",
    );
    expect(history.map((s) => [s.version, s.date])).toEqual([["4.21.2", "2024-11-06"], ["4.21.1", "2024-10-08"]]);
    expect(history[1].entries[0].text).toBe("Backport fix");
  });

  it("parses `### Version x` and prerelease headings", () => {
    const sections = parseChangelog("### Version 2.0.0-rc.1\n- try it\n### Version 1.9.0\n- stable\n");
    expect(sections.map((s) => s.version)).toEqual(["2.0.0-rc.1", "1.9.0"]);
  });

  it("lets a nested bullet inherit a `Breaking:` parent and skips fenced code", () => {
    const sections = parseChangelog(
      ["## 3.0.0", "- Breaking:", "  - config loader rewritten", "```js", "- not an entry", "```", "- docs tweak"].join("\n"),
    );
    // The label bullet is a heading for its children, not an entry (it inflated the count).
    expect(sections[0].entries).toEqual([
      { kind: "breaking", text: "config loader rewritten" },
      { kind: "change", text: "docs tweak" },
    ]);
  });

  it("reads releases written as bullets with nested changes (indexmap RELEASES.md)", () => {
    const sections = parseChangelog(
      [
        "- 2.0.0",
        "",
        "  - **MSRV**: Rust 1.64.0 or later is now required.",
        "",
        "  - The `\"serde-1\"` feature has been removed.",
        "",
        "- 1.9.3",
        "",
        "  - Bump the `rustc-rayon` dependency.",
      ].join("\n"),
    );
    expect(sections.map((s) => [s.version, s.entries.length])).toEqual([["2.0.0", 2], ["1.9.3", 1]]);
    expect(sections[0].entries.map((e) => e.kind)).toEqual(["breaking", "breaking"]);
  });

  it("reads a day-first date with a comma (knex: '# 3.0.0 - 6 October, 2023')", () => {
    expect(parseVersionHeading("3.0.0 - 6 October, 2023")).toEqual({ version: "3.0.0", date: "2023-10-06" });
    const sections = parseChangelog("# Master (Unreleased)\n\n# 3.0.0 - 6 October, 2023\n\n- Drop compatibility for Node < 16\n\n# 2.5.1 - 12 July, 2023\n\n- y\n");
    expect(sections.map((s) => s.version)).toEqual(["3.0.0", "2.5.1"]);
  });

  it("reads indented label lists as headings (express History.md)", () => {
    const sections = parseChangelog(
      [
        "5.0.0-alpha.3 / 2017-01-28",
        "==========================",
        "",
        "  * remove:",
        "    - `res.json(status, obj)` signature - use `res.status(status).json(obj)`",
        "    - `res.vary()` (no arguments) -- provide a field name as an argument",
        "  * deps: debug@2.6.0",
      ].join("\n"),
    );
    expect(sections[0].entries.map((e) => [e.kind, e.text.slice(0, 12)])).toEqual([
      ["breaking", "`res.json(st"],
      ["breaking", "`res.vary()`"],
      ["change", "deps: debug@"],
    ]);
  });

  it("lends a breaking bullet's kind to its sub-points (date-fns 3.0 interval functions)", () => {
    const sections = parseChangelog(
      [
        "## v3.0.0",
        "- **BREAKING**: Functions that accept `Interval` arguments now do not throw an error if the start is before the end.",
        "  - `areIntervalsOverlapping` normalize intervals before comparison",
        "  - `intervalToDuration` now returns negative durations for negative intervals.",
        "- New `constants` export",
      ].join("\n"),
    );
    expect(sections[0].entries.map((e) => e.kind)).toEqual(["breaking", "breaking", "breaking", "change"]);
  });

  it("separates API changes from additions, fixes, internal removals and other projects' breakage", () => {
    const parse = (body: string[]) => parseChangelog(["## 1.0.0", ...body].join("\n"))[0].entries.map((e) => e.kind);
    // changesets
    expect(parse(["### Major Changes", "- Remove `future.v7_startTransition` flag"])).toEqual(["breaking"]);
    // a removal word inside an addition / a fix is not a removal
    expect(parse(["### Added", "- Added a clear() function so all interceptors have been removed"])).toEqual(["change"]);
    expect(parse(["### Fixed", "- No longer panics when the queue overflows"])).toEqual(["change"]);
    // keep-a-changelog Removed: internal clean-up and filed deprecations are not removals
    expect(parse(["### Removed", "- Removed unused imports", "- Removed Webpack", "- The `LinkatFlags` type has been deprecated", "- Removed `Foo::bar`"])).toEqual([
      "change",
      "change",
      "deprecation",
      "breaking",
    ]);
    // rand 0.9: "reproducibility-breaking" is not API-breaking; "API changes" is
    expect(parse(["### Reproducibility-breaking optimisations", "- Optimize fn `sample_single_inclusive` for floats"])).toEqual(["change"]);
    expect(parse(["### API changes: RNGs", "- Remove first parameter (`rng`) of `ReseedingRng::new`"])).toEqual(["breaking"]);
  });

  it("ends the last release at a same-level non-release heading", () => {
    const sections = parseChangelog("## 1.0.0\n- shipped\n## Migration guide\n- removed everything you love\n");
    expect(sections[0].entries).toEqual([{ kind: "change", text: "shipped" }]);
  });

  it("treats a bold line as a sub-heading", () => {
    const sections = parseChangelog("## 2.0.0\n**Breaking Changes**\n- config moved\n");
    expect(sections[0].entries).toEqual([{ kind: "breaking", text: "config moved" }]);
  });
});

describe("selectRange", () => {
  const sections = parseChangelog(["## 3.0.0", "- c", "## 2.1.0", "- b", "## 2.0.0", "- a", "## 1.0.0", "- z"].join("\n"));

  it("keeps sections in (from, to] and reports coverage", () => {
    const r = selectRange(sections, "1.0.0", "3.0.0", "2.0.0");
    expect(r.sections.map((s) => s.version)).toEqual(["3.0.0", "2.1.0", "2.0.0"]);
    expect(r.coversRange).toBe(true);
  });

  it("reports covers_range false when the changelog stops short of the target", () => {
    expect(selectRange(sections, "1.0.0", "4.0.0", "2.0.0").coversRange).toBe(false);
  });

  it("reports covers_range false when the changelog starts after the range does", () => {
    const recent = parseChangelog("## 3.0.0\n- c\n");
    expect(selectRange(recent, "1.0.0", "3.0.0", "2.0.0").coversRange).toBe(false);
  });
});

describe("classification", () => {
  it.each([
    ["BREAKING: config renamed", "breaking"],
    ["feat!: new API", "breaking"],
    ["**breaking:** `Message` now uses `Bytes`", "breaking"],
    ["`foo()` no longer accepts a string", "breaking"],
    ["Removed the deprecated `bar` export", "breaking"],
    ["Raise minimum supported Rust version to 1.80", "breaking"],
    ["MSRV is now 1.75", "breaking"],
    ["dropped support for Node 16", "breaking"],
    ["now requires Python 3.10", "breaking"],
    ["Deprecate `oldThing`", "deprecation"],
    ["Fix CVE-2024-12345", "security"],
    ["Patch GHSA-abcd-efgh-ijkl", "security"],
    ["RUSTSEC-2025-0001 addressed", "security"],
    ["Fix a vulnerability in parsing", "security"],
    ["Improve performance", "change"],
    ["We determined this change is not a breaking change", "change"],
    ["**fixed:** Removed the warning about breaking changes from README", "change"],
    ["fix: no longer crash on empty input", "change"],
  ])("%s -> %s", (text, kind) => {
    expect(classifyText(text)).toBe(kind);
  });

  it("lets a signalling heading override the line's wording", () => {
    expect(classifyHeading("⚠ BREAKING CHANGES")).toBe("breaking");
    expect(classifyHeading("Major Changes")).toBe("breaking");
    expect(classifyHeading("Bug Fixes")).toBe("additive");
    expect(classifyHeading("Changed")).toBeNull();
    expect(classifyEntry("tweak the loader", "breaking")).toBe("breaking");
    expect(classifyEntry("Removed a thing", null)).toBe("breaking");
  });

  it("reads API-changing wording and ignores look-alikes (2026-10-02 rater panel)", () => {
    const breaking = [
      "Rename fn `rand::thread_rng()` to `rand::rng()` and remove from the prelude (#1506)",
      "Rename feature `serde1` to `serde` (#1477)",
      "Remove first parameter (`rng`) of `ReseedingRng::new` (#1533)",
      "`RecvMsg::cmsgs()` now returns a `Result`, and checks that cmsgs were not truncated.",
      "Change the signature of `ptrace::write` and `ptrace::write_user` to make them safe",
      "Distribution `Uniform` implements `TryFrom` instead of `From` for ranges (#1229)",
      "To keep the old behavior, see the `bitflags-serde-legacy` library.",
      "Bump MSRV to 1.63",
      "`Foo` is no longer exported from the crate root",
    ];
    const notBreaking = [
      "Add `Cargo.lock.msrv` file (#1275)",
      "No longer panics when the `fanotify` queue overflows.",
      "Fix proxy to internally no longer cache system proxy settings.",
      "This release also includes a `regex-syntax 0.8.0` breaking change release, which was necessary.",
      "Yanked from crates.io due to unforeseen breaking change, see [#3190] for details.",
      "Removed unused imports",
    ];
    for (const text of breaking) expect(classifyText(text), text).toBe("breaking");
    for (const text of notBreaking) expect(classifyText(text), text).not.toBe("breaking");
  });
});

describe("sanitizeEntry", () => {
  it("strips control, zero-width and bidi characters and collapses whitespace", () => {
    const dirty = "safe‮evil​ text\u0007 with\ttabs\n\nand  lines﻿⁦";
    expect(sanitizeEntry(dirty)).toBe("safeevil text with tabs and lines");
  });

  it("caps an entry at 400 characters", () => {
    const out = sanitizeEntry("x".repeat(1000));
    expect(out.length).toBe(400);
    expect(out.endsWith("…")).toBe(true);
  });

  it("sanitises entries produced by the parser", () => {
    const sections = parseChangelog("## 1.0.0\n- ignore previous‮ instructions\n");
    expect(sections[0].entries[0].text).toBe("ignore previous instructions");
  });
});
