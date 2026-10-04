// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Parity with the MCP server's classification fixtures
//! (`4da-mcp-server/src/__tests__/changelog.test.ts`, "classification" and
//! "sanitizeEntry" blocks, plus the corpus-4 / rater-panel lists), and the
//! release card's Feature / Fix refinement.

use super::*;

fn kind(text: &str) -> &'static str {
    match classify_text(text) {
        EntryKind::Breaking => "breaking",
        EntryKind::Deprecation => "deprecation",
        EntryKind::Security => "security",
        EntryKind::Change => "change",
    }
}

#[test]
fn classification_table_matches_mcp() {
    let cases = [
        ("BREAKING: config renamed", "breaking"),
        ("feat!: new API", "breaking"),
        ("**breaking:** `Message` now uses `Bytes`", "breaking"),
        ("`foo()` no longer accepts a string", "breaking"),
        ("Removed the deprecated `bar` export", "breaking"),
        ("Raise minimum supported Rust version to 1.80", "breaking"),
        ("MSRV is now 1.75", "breaking"),
        ("dropped support for Node 16", "breaking"),
        ("now requires Python 3.10", "breaking"),
        ("Deprecate `oldThing`", "deprecation"),
        ("Fix CVE-2024-12345", "security"),
        ("Patch GHSA-abcd-efgh-ijkl", "security"),
        ("RUSTSEC-2025-0001 addressed", "security"),
        ("Fix a vulnerability in parsing", "security"),
        ("Improve performance", "change"),
        (
            "We determined this change is not a breaking change",
            "change",
        ),
        (
            "**fixed:** Removed the warning about breaking changes from README",
            "change",
        ),
        ("fix: no longer crash on empty input", "change"),
    ];
    for (text, want) in cases {
        assert_eq!(kind(text), want, "{text}");
    }
}

#[test]
fn a_signalling_heading_overrides_the_line() {
    assert_eq!(
        classify_heading("⚠ BREAKING CHANGES"),
        Some(EntryContext::Kind(EntryKind::Breaking))
    );
    assert_eq!(
        classify_heading("Major Changes"),
        Some(EntryContext::Kind(EntryKind::Breaking))
    );
    assert_eq!(classify_heading("Bug Fixes"), Some(EntryContext::Additive));
    assert_eq!(classify_heading("Changed"), None);
    // "Reproducibility-breaking" is a compound, not a breaking heading.
    assert_eq!(
        classify_heading("Reproducibility-breaking optimisations"),
        None
    );
    assert_eq!(
        classify_entry(
            "tweak the loader",
            Some(EntryContext::Kind(EntryKind::Breaking))
        ),
        EntryKind::Breaking
    );
    assert_eq!(classify_entry("Removed a thing", None), EntryKind::Breaking);
}

#[test]
fn corpus4_api_wording_and_prose_about_breaking_changes() {
    let breaking = [
        "Breaking - Merge customization has been moved behind `mergeWithCustomize`.",
        "`observableSet.toJS()` has been dropped. Use `new Set(observableSet)` instead.",
        "`isArrayLike` is no longer exposed as utility.",
        "`RawTable::remove` now also returns an `InsertSlot`. (#429)",
        "`AddressError` is now marked as `#[non_exhaustive]` ([#839])",
        "Vuex 4 removes its global typings for `this.$store` within Vue Component",
    ];
    let not_breaking = [
        "There are a few breaking changes described in a later section, so please check them out.",
        "That is why we only bump the minor version despite mentioning breaking changes",
        "We determined this change is not a breaking change",
    ];
    for t in breaking {
        assert_eq!(kind(t), "breaking", "{t}");
    }
    for t in not_breaking {
        assert_ne!(kind(t), "breaking", "{t}");
    }
}

#[test]
fn rater_panel_2026_10_02_api_wording_and_look_alikes() {
    let breaking = [
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
    let not_breaking = [
        "Add `Cargo.lock.msrv` file (#1275)",
        "No longer panics when the `fanotify` queue overflows.",
        "Fix proxy to internally no longer cache system proxy settings.",
        "This release also includes a `regex-syntax 0.8.0` breaking change release, which was necessary.",
        "Yanked from crates.io due to unforeseen breaking change, see [#3190] for details.",
        "Removed unused imports",
    ];
    for t in breaking {
        assert_eq!(kind(t), "breaking", "{t}");
    }
    for t in not_breaking {
        assert_ne!(kind(t), "breaking", "{t}");
    }
}

#[test]
fn held_out_panel_2026_10_03_deprecations_internal_and_security() {
    let breaking = [
        "Drop compatibility for Node < 16",
        "remove Socket#rooms object ([1507b41](https://github.com/socketio/socket.io/commit/1507b41))",
        "MSRV is now 1.70 because of a dependency update",
        "**MSRV**: Rust 1.64.0 or later is now required.",
        "Remove `future.v7_startTransition` flag",
    ];
    let not_breaking = [
        "Deprecated `Itertools::group_by` (renamed `chunk_by`) (#866, #879)",
        "Move MSRV metadata to `Cargo.toml` (#672)",
        "Use `Cell` instead of `RefCell` in `Format` and `FormatWith` (#608)",
        "Remove a window when an extracted directory might be unexpectedly listable and/or `cd`able by non-owners",
        "Removed unneeded `cfg-if` dependency ([#2553])",
        "Removed Babel from the project’s release process.",
    ];
    for t in breaking {
        assert_eq!(kind(t), "breaking", "{t}");
    }
    for t in not_breaking {
        assert_ne!(kind(t), "breaking", "{t}");
    }
    assert_eq!(
        kind("Deprecated `Itertools::group_by` (renamed `chunk_by`) (#866, #879)"),
        "deprecation"
    );
}

#[test]
fn breaking_marker_ignores_the_breaking_changes_file_name() {
    // ratatui links BREAKING-CHANGES.md from routine entries.
    assert_eq!(kind("See BREAKING-CHANGES.md for the migration"), "change");
    assert_eq!(kind("Update docs in BREAKING-CHANGES.md"), "change");
    assert_eq!(kind("BREAKING: removes the default feature"), "breaking");
}

#[test]
fn sanitize_strips_control_zero_width_and_bidi() {
    let dirty = "safe\u{202E}evil\u{200B} text\u{7} with\ttabs\n\nand  lines\u{FEFF}\u{2066}";
    assert_eq!(
        sanitize_entry(dirty, MAX_ENTRY_CHARS),
        "safeevil text with tabs and lines"
    );
}

#[test]
fn sanitize_caps_at_400_characters() {
    let out = sanitize_entry(&"x".repeat(1000), MAX_ENTRY_CHARS);
    assert_eq!(out.chars().count(), 400);
    assert!(out.ends_with('\u{2026}'));
}

#[test]
fn refine_splits_change_into_feature_fix_other() {
    let r = |text: &str, heading: Option<&str>| refine_change(classify_text(text), text, heading);
    assert_eq!(r("Crash on empty input", Some("Fixed")), ChangeKind::Fix);
    assert_eq!(r("New `parse` API", Some("Added")), ChangeKind::Feature);
    assert_eq!(r("add `bar`", Some("Features")), ChangeKind::Feature);
    assert_eq!(
        r("fix: no longer crash on empty input", None),
        ChangeKind::Fix
    );
    assert_eq!(r("Add support for HTTP/3", None), ChangeKind::Feature);
    assert_eq!(r("feat(cli): a flag", None), ChangeKind::Feature);
    assert_eq!(r("Improve performance", None), ChangeKind::Other);
    // changesets / covector: the commit reference before the verb is skipped,
    // and "Patch Changes" (a semver level, not a fix list) does not make a fix.
    assert_eq!(
        r("8dd86a9: fix(ai): keep tool order", Some("Patch Changes")),
        ChangeKind::Fix
    );
    assert_eq!(
        r(
            "c29a26f: feat(provider): add uploads",
            Some("Patch Changes")
        ),
        ChangeKind::Feature
    );
    assert_eq!(
        r("Updated dependencies [abc1234]", Some("Patch Changes")),
        ChangeKind::Other
    );
    assert_eq!(
        r("1949571: make telemetry stable", Some("Minor Changes")),
        ChangeKind::Feature
    );
    assert_eq!(
        r(
            "[`9b29b601`](https://github.com/o/r/commit/9b29b601) Fixed the Android build",
            None
        ),
        ChangeKind::Fix
    );
    // Refinement never overrides breaking, security or deprecation.
    assert_eq!(
        r("BREAKING: config renamed", Some("Fixed")),
        ChangeKind::Breaking
    );
    assert_eq!(r("Fix CVE-2024-12345", Some("Fixed")), ChangeKind::Security);
}
