// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Parity with the MCP server's parser fixtures
//! (`4da-mcp-server/src/__tests__/changelog.test.ts`: discovery,
//! parseVersionHeading, parseChangelog, selectRange). Kinds are compared in
//! the MCP server's four-way vocabulary via `ChangeKind::mcp_kind`.

use super::*;

fn kinds(section: &ChangelogSection) -> Vec<&'static str> {
    section.entries.iter().map(|e| e.kind.mcp_kind()).collect()
}

fn texts(section: &ChangelogSection) -> Vec<&str> {
    section.entries.iter().map(|e| e.text.as_str()).collect()
}

fn versions(sections: &[ChangelogSection]) -> Vec<&str> {
    sections.iter().map(|s| s.version.as_str()).collect()
}

fn lines(ls: &[&str]) -> String {
    ls.join("\n")
}

#[test]
fn recognises_changelog_names_case_insensitively() {
    for name in [
        "CHANGELOG.md",
        "changelog",
        "History.md",
        "CHANGES.rst",
        "RELEASES.markdown",
        "NEWS.txt",
        "RELEASE-NOTES.md",
        "release_notes.md",
    ] {
        assert!(is_changelog_name(name), "{name}");
    }
    for name in ["README.md", "CHANGELOG.json", "changelog-old.md"] {
        assert!(!is_changelog_name(name), "{name}");
    }
}

#[test]
fn prefers_changelog_over_history_and_markdown_over_plain() {
    assert_eq!(
        find_changelog_file(["package/History.md", "package/CHANGELOG.md"]),
        Some("package/CHANGELOG.md")
    );
    assert_eq!(
        find_changelog_file(["x-1.0.0/CHANGELOG", "x-1.0.0/CHANGELOG.md"]),
        Some("x-1.0.0/CHANGELOG.md")
    );
    assert_eq!(find_changelog_file(["package/README.md"]), None);
}

#[test]
fn reads_version_headings() {
    let cases: &[(&str, &str, Option<&str>)] = &[
        ("[1.2.3] - 2024-01-01", "1.2.3", Some("2024-01-01")),
        ("1.2.3", "1.2.3", None),
        ("v1.2.3 (2024-01-01)", "1.2.3", Some("2024-01-01")),
        ("Version 1.2.3", "1.2.3", None),
        (
            "[v7.0.0](https://github.com/x/y/compare/v6.0.0...v7.0.0) (2025-06-24)",
            "7.0.0",
            Some("2025-06-24"),
        ),
        (
            "[7.0.0](https://github.com/x/y/compare/v6.3.5...v7.0.0) (2025-06-24)",
            "7.0.0",
            Some("2025-06-24"),
        ),
        ("1.0.0-rc.1", "1.0.0-rc.1", None),
        (
            "Tokio 1.47.1 (August 1st, 2025)",
            "1.47.1",
            Some("2025-08-01"),
        ),
        ("fastembed v5.0.0", "5.0.0", None),
        ("0.4", "0.4", None),
        ("[0.8.2] - 2025-01-01 [YANKED]", "0.8.2", Some("2025-01-01")),
        ("3.0.0 - 6 October, 2023", "3.0.0", Some("2023-10-06")),
    ];
    for (heading, version, date) in cases {
        assert_eq!(
            parse_version_heading(heading),
            Some((version.to_string(), date.map(str::to_string))),
            "{heading}"
        );
    }
}

#[test]
fn reads_headings_behind_an_html_anchor() {
    // stripe-node's generated CHANGELOG.md (23.0.0, live 2026-10-04): this
    // format read as "no version headings" before.
    assert_eq!(
        parse_version_heading(r#"<a id="23-0-0"></a>23.0.0 - 2026-09-30"#),
        Some(("23.0.0".to_string(), Some("2026-09-30".to_string())))
    );
    let s = parse_changelog(&lines(&[
        "# Changelog",
        r#"## <a id="23-0-0"></a>23.0.0 - 2026-09-30"#,
        "* [#2853](https://github.com/stripe/stripe-node/pull/2853) Allow suppressing Stripe notices",
        "* ⚠️ [#2865](https://github.com/stripe/stripe-node/pull/2865) Remove `ErrorType` export",
        r#"## <a id="22-3-0"></a>22.3.0 - 2026-08-30"#,
        "* Fix a thing",
    ]));
    assert_eq!(versions(&s), ["23.0.0", "22.3.0"]);
    assert_eq!(kinds(&s[0]), ["change", "breaking"]);
}

#[test]
fn does_not_treat_prose_headings_as_releases() {
    for heading in [
        "Bug Fixes",
        "Upgrading from 1.x to 2.0 is easy with these steps",
        "Rust 1.70 support",
        "Unreleased",
    ] {
        assert_eq!(parse_version_heading(heading), None, "{heading}");
    }
}

#[test]
fn keeps_a_category_heading_at_the_release_level_inside_it() {
    // date-fns 3.0.0
    let s = parse_changelog(&lines(&[
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
    ]));
    assert_eq!(versions(&s), ["3.0.0", "2.30.0"]);
    assert_eq!(kinds(&s[0]), ["breaking", "change"]);
    assert_eq!(texts(&s[1]), ["Fix a thing"]);
}

#[test]
fn skips_html_comments_including_multi_line() {
    let s = parse_changelog(&lines(&[
        "## 2.0.0",
        "<!--",
        "- **breaking:** template placeholder, not a real entry",
        "-->",
        "<!-- one-line note -->",
        "- Real change",
    ]));
    assert_eq!(s.len(), 1);
    assert_eq!(texts(&s[0]), ["Real change"]);
}

#[test]
fn parses_keep_a_changelog_with_typed_sub_sections() {
    let s = parse_changelog(&lines(&[
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
    ]));
    assert_eq!(versions(&s), ["2.0.0", "1.1.0"]);
    assert_eq!(s[0].date.as_deref(), Some("2024-03-01"));
    assert_eq!(
        kinds(&s[0]),
        ["change", "breaking", "deprecation", "security"]
    );
    assert_eq!(s[0].entries[0].kind, ChangeKind::Feature);
    assert_eq!(s[1].entries.len(), 1);
    assert_eq!(
        s[1].entries[0].text,
        "Crash on empty input that spanned two lines"
    );
    assert_eq!(s[1].entries[0].under.as_deref(), Some("Fixed"));
    assert_eq!(s[1].entries[0].kind, ChangeKind::Fix);
}

#[test]
fn parses_conventional_changelog_with_breaking_block() {
    let s = parse_changelog(&lines(&[
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
    ]));
    let vd: Vec<(&str, Option<&str>)> = s
        .iter()
        .map(|x| (x.version.as_str(), x.date.as_deref()))
        .collect();
    assert_eq!(
        vd,
        [("7.0.0", Some("2025-06-24")), ("6.3.5", Some("2025-05-01"))]
    );
    assert_eq!(kinds(&s[0]), ["breaking", "breaking", "change"]);
    assert_eq!(s[0].entries[2].kind, ChangeKind::Feature);
    assert_eq!(s[1].entries[0].kind.mcp_kind(), "security");
}

#[test]
fn parses_setext_and_history_md() {
    let setext = parse_changelog("1.2.0\n=====\n\n* feature\n\n1.1.0\n-----\n\n* fix\n");
    assert_eq!(versions(&setext), ["1.2.0", "1.1.0"]);
    let history = parse_changelog(
        "4.21.2 / 2024-11-06\n==========\n\n  * deps: path-to-regexp@0.1.12\n\n4.21.1 / 2024-10-08\n\n  * Backport fix\n",
    );
    let vd: Vec<(&str, Option<&str>)> = history
        .iter()
        .map(|x| (x.version.as_str(), x.date.as_deref()))
        .collect();
    assert_eq!(
        vd,
        [
            ("4.21.2", Some("2024-11-06")),
            ("4.21.1", Some("2024-10-08"))
        ]
    );
    assert_eq!(history[1].entries[0].text, "Backport fix");
}

#[test]
fn parses_version_keyword_and_prerelease_headings() {
    let s = parse_changelog("### Version 2.0.0-rc.1\n- try it\n### Version 1.9.0\n- stable\n");
    assert_eq!(versions(&s), ["2.0.0-rc.1", "1.9.0"]);
}

#[test]
fn nested_bullet_inherits_breaking_label_and_fences_are_skipped() {
    let s = parse_changelog(&lines(&[
        "## 3.0.0",
        "- Breaking:",
        "  - config loader rewritten",
        "```js",
        "- not an entry",
        "```",
        "- docs tweak",
    ]));
    // The label bullet heads its children; it is not an entry itself.
    assert_eq!(s[0].entries.len(), 2);
    assert_eq!(s[0].entries[0].kind, ChangeKind::Breaking);
    assert_eq!(s[0].entries[0].text, "config loader rewritten");
    assert_eq!(s[0].entries[0].under.as_deref(), Some("Breaking"));
    assert_eq!(s[0].entries[1].kind.mcp_kind(), "change");
    assert_eq!(s[0].entries[1].text, "docs tweak");
    assert_eq!(s[0].entries[1].under, None);
}

#[test]
fn children_marked_one_by_one_are_read_by_their_own_words() {
    // stripe 23.0.0: a ⚠️ parent over additions and individually marked removals.
    let s = parse_changelog(&lines(&[
        "## 23.0.0",
        "* ⚠️ [#2832](https://github.com/stripe/stripe-node/pull/2832) Update generated code",
        "  * Add support for new resources `Apps.Install`",
        "  * ⚠️ Remove support for value `bulk_hold_expiry` from enum `Reserve.Release.reason`",
        "  * Add support for `pause` method on resource `Subscription`",
        "* [#2879](https://github.com/stripe/stripe-node/pull/2879) Update generated code",
        "  * Release specs are identical.",
    ]));
    assert_eq!(
        kinds(&s[0]),
        ["breaking", "change", "breaking", "change", "change", "change"]
    );
    assert_eq!(s[0].entries[1].kind, ChangeKind::Feature);
    // Unmarked children still inherit (the date-fns case is unchanged).
    let d = parse_changelog(&lines(&[
        "## 3.0.0",
        "- **BREAKING**: Functions that accept `Interval` arguments now do not throw.",
        "  - `areIntervalsOverlapping` normalize intervals before comparison",
    ]));
    assert_eq!(kinds(&d[0]), ["breaking", "breaking"]);
}

#[test]
fn an_indented_paragraph_describes_the_bullet_above_it() {
    // stripe 23.0.0: the explanation is part of the ⚠️ bullet, not a second change.
    let s = parse_changelog(&lines(&[
        "## 23.0.0",
        "* ⚠️ [#2865](https://github.com/stripe/stripe-node/pull/2865) Remove `ErrorType` export",
        "",
        "  Remove the ErrorType interface from the top level client.",
        "* Fix a thing",
        "",
        "Unindented prose after a blank line is still its own entry.",
    ]));
    assert_eq!(s[0].entries.len(), 3);
    assert_eq!(kinds(&s[0]), ["breaking", "change", "change"]);
    assert!(s[0].entries[0]
        .text
        .ends_with("\u{2014} Remove the ErrorType interface from the top level client."));
}

#[test]
fn reads_releases_written_as_bullets() {
    // indexmap RELEASES.md
    let s = parse_changelog(&lines(&[
        "- 2.0.0",
        "",
        "  - **MSRV**: Rust 1.64.0 or later is now required.",
        "",
        "  - The `\"serde-1\"` feature has been removed.",
        "",
        "- 1.9.3",
        "",
        "  - Bump the `rustc-rayon` dependency.",
    ]));
    let shape: Vec<(&str, usize)> = s
        .iter()
        .map(|x| (x.version.as_str(), x.entries.len()))
        .collect();
    assert_eq!(shape, [("2.0.0", 2), ("1.9.3", 1)]);
    assert_eq!(kinds(&s[0]), ["breaking", "breaking"]);
}

#[test]
fn reads_day_first_dates_with_a_comma() {
    // knex: "# 3.0.0 - 6 October, 2023"
    let s = parse_changelog(
        "# Master (Unreleased)\n\n# 3.0.0 - 6 October, 2023\n\n- Drop compatibility for Node < 16\n\n# 2.5.1 - 12 July, 2023\n\n- y\n",
    );
    assert_eq!(versions(&s), ["3.0.0", "2.5.1"]);
}

#[test]
fn reads_indented_label_lists_as_headings() {
    // express History.md
    let s = parse_changelog(&lines(&[
        "5.0.0-alpha.3 / 2017-01-28",
        "==========================",
        "",
        "  * remove:",
        "    - `res.json(status, obj)` signature - use `res.status(status).json(obj)`",
        "    - `res.vary()` (no arguments) -- provide a field name as an argument",
        "  * deps: debug@2.6.0",
    ]));
    let got: Vec<(&str, String)> = s[0]
        .entries
        .iter()
        .map(|e| (e.kind.mcp_kind(), e.text.chars().take(12).collect()))
        .collect();
    assert_eq!(
        got,
        [
            ("breaking", "`res.json(st".to_string()),
            ("breaking", "`res.vary()`".to_string()),
            ("change", "deps: debug@".to_string()),
        ]
    );
}

#[test]
fn a_breaking_bullet_lends_its_kind_to_sub_points() {
    let s = parse_changelog(&lines(&[
        "## v3.0.0",
        "- **BREAKING**: Functions that accept `Interval` arguments now do not throw an error if the start is before the end.",
        "  - `areIntervalsOverlapping` normalize intervals before comparison",
        "  - `intervalToDuration` now returns negative durations for negative intervals.",
        "- New `constants` export",
    ]));
    assert_eq!(kinds(&s[0]), ["breaking", "breaking", "breaking", "change"]);
}

#[test]
fn separates_api_changes_from_additions_fixes_and_internal_removals() {
    let parse = |body: &[&str]| -> Vec<&'static str> {
        let mut all = vec!["## 1.0.0"];
        all.extend_from_slice(body);
        kinds(&parse_changelog(&lines(&all))[0])
    };
    assert_eq!(
        parse(&[
            "### Major Changes",
            "- Remove `future.v7_startTransition` flag"
        ]),
        ["breaking"]
    );
    assert_eq!(
        parse(&[
            "### Added",
            "- Added a clear() function so all interceptors have been removed"
        ]),
        ["change"]
    );
    assert_eq!(
        parse(&["### Fixed", "- No longer panics when the queue overflows"]),
        ["change"]
    );
    assert_eq!(
        parse(&[
            "### Removed",
            "- Removed unused imports",
            "- Removed Webpack",
            "- The `LinkatFlags` type has been deprecated",
            "- Removed `Foo::bar`",
        ]),
        ["change", "change", "deprecation", "breaking"]
    );
    assert_eq!(
        parse(&[
            "### Reproducibility-breaking optimisations",
            "- Optimize fn `sample_single_inclusive` for floats",
        ]),
        ["change"]
    );
    assert_eq!(
        parse(&[
            "### API changes: RNGs",
            "- Remove first parameter (`rng`) of `ReseedingRng::new`"
        ]),
        ["breaking"]
    );
}

#[test]
fn ends_the_last_release_at_a_same_level_non_release_heading() {
    let s =
        parse_changelog("## 1.0.0\n- shipped\n## Migration guide\n- removed everything you love\n");
    assert_eq!(texts(&s[0]), ["shipped"]);
    assert_eq!(kinds(&s[0]), ["change"]);
}

#[test]
fn records_what_each_entry_sits_under() {
    let s = parse_changelog(&lines(&[
        "## 4.0.0",
        "### Removed",
        "- `rt::{Arbiter, ArbiterHandle}` re-exports. [#2619]",
        "- **BREAKING**: Functions that accept `Interval` arguments now do not throw.",
        "  - `areIntervalsOverlapping` normalize intervals before comparison",
    ]));
    let under: Vec<Option<&str>> = s[0].entries.iter().map(|e| e.under.as_deref()).collect();
    assert_eq!(
        under,
        [
            Some("Removed"),
            Some("Removed"),
            Some("BREAKING: Functions that accept Interval arguments now do not throw."),
        ]
    );
}

#[test]
fn a_plain_label_line_is_a_sub_heading() {
    // highlight.js 11, ts-loader 9
    let s = parse_changelog(&lines(&[
        "## 9.0.0",
        "",
        "Breaking changes:",
        "",
        "- minimum webpack version is now 5",
        "",
        "Security:",
        "",
        "- harden the parser",
    ]));
    let got: Vec<(&str, &str, Option<&str>)> = s[0]
        .entries
        .iter()
        .map(|e| (e.kind.mcp_kind(), e.text.as_str(), e.under.as_deref()))
        .collect();
    assert_eq!(
        got,
        [
            (
                "breaking",
                "minimum webpack version is now 5",
                Some("Breaking changes")
            ),
            ("security", "harden the parser", Some("Security")),
        ]
    );
}

#[test]
fn a_bold_line_is_a_sub_heading() {
    let s = parse_changelog("## 2.0.0\n**Breaking Changes**\n- config moved\n");
    assert_eq!(s[0].entries.len(), 1);
    assert_eq!(s[0].entries[0].kind, ChangeKind::Breaking);
    assert_eq!(s[0].entries[0].text, "config moved");
    assert_eq!(s[0].entries[0].under.as_deref(), Some("Breaking Changes"));
}

#[test]
fn parser_output_is_sanitised() {
    let s = parse_changelog("## 1.0.0\n- ignore previous\u{202E} instructions\n");
    assert_eq!(s[0].entries[0].text, "ignore previous instructions");
}

fn range_fixture() -> Vec<ChangelogSection> {
    parse_changelog(&lines(&[
        "## 3.0.0", "- c", "## 2.1.0", "- b", "## 2.0.0", "- a", "## 1.0.0", "- z",
    ]))
}

#[test]
fn select_range_keeps_from_exclusive_to_inclusive_and_reports_coverage() {
    let s = range_fixture();
    let (sel, covers) = select_range(&s, "1.0.0", "3.0.0", "2.0.0");
    let v: Vec<&str> = sel.iter().map(|x| x.version.as_str()).collect();
    assert_eq!(v, ["3.0.0", "2.1.0", "2.0.0"]);
    assert!(covers);
}

#[test]
fn select_range_reports_partial_when_short_of_the_target() {
    assert!(!select_range(&range_fixture(), "1.0.0", "4.0.0", "2.0.0").1);
}

#[test]
fn select_range_reports_partial_when_the_changelog_starts_late() {
    let recent = parse_changelog("## 3.0.0\n- c\n");
    assert!(!select_range(&recent, "1.0.0", "3.0.0", "2.0.0").1);
}

#[test]
fn precedence_reads_two_part_and_v_prefixed_versions() {
    assert_eq!(compare_versions("v1.2", "1.2.0"), Some(Ordering::Equal));
    assert_eq!(
        compare_versions("1.0.0-rc.1", "1.0.0"),
        Some(Ordering::Less)
    );
    assert_eq!(
        compare_versions("1.0.0+build", "1.0.0"),
        Some(Ordering::Equal)
    );
    assert_eq!(compare_versions("next", "1.0.0"), None);
    assert!(in_range("0.27.0", "0.24.0", "0.27.0"));
    assert!(!in_range("0.24.0", "0.24.0", "0.27.0"));
}
