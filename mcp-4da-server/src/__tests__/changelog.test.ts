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
    for (const name of ["CHANGELOG.md", "changelog", "History.md", "CHANGES.rst", "RELEASES.markdown", "NEWS.txt"]) {
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
    expect(sections[0].entries).toEqual([
      { kind: "breaking", text: "Breaking:" },
      { kind: "breaking", text: "config loader rewritten" },
      { kind: "change", text: "docs tweak" },
    ]);
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
    expect(classifyHeading("Bug Fixes")).toBeNull();
    expect(classifyEntry("tweak the loader", "breaking")).toBe("breaking");
    expect(classifyEntry("Removed a thing", null)).toBe("breaking");
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
