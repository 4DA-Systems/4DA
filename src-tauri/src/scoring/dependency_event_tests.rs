// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Real titles from the live feed (2026-10-02 .. 2026-10-04) and the cases
//! the dependency-event claim was specified against.

use super::*;
use crate::scoring::dependencies::VersionDelta;

fn event(title: &str, content: &str, package: &str) -> bool {
    has_dependency_event_evidence(title, content, package, &normalize_package_name(package))
}

fn dep(name: &str) -> DepMatch {
    DepMatch {
        package_name: normalize_package_name(name),
        confidence: 0.5,
        version_delta: VersionDelta::Unknown,
        is_dev: false,
        is_direct: true,
        version: None,
        ecosystem: "javascript".to_string(),
        corroborated: true,
        project_paths: vec!["d:/4da".to_string()],
        raw_name: Some(name.to_string()),
    }
}

fn editorial<'a>(title: &'a str, content: &'a str, deps: &'a [DepMatch]) -> EventInputs<'a> {
    EventInputs {
        source_type: "hackernews",
        title,
        content,
        registry_advisory: false,
        security_confirmed: false,
        applicability: None,
        via_registry_subject: false,
        release_class: None,
        already_installed_release: false,
        strongly_grounded: true,
        deps,
    }
}

#[test]
fn release_and_security_headlines_are_events() {
    let cases = [
        ("Announcing Tauri 2.12", "tauri"),
        ("React 20 ships with native signals", "react"),
        ("openai v7.25.0 released", "openai"),
        ("OpenAI Node SDK v5 breaking changes", "openai"),
        ("openai npm package compromised", "openai"),
        ("Announcing TypeScript 7.0", "typescript"),
        ("Critical vulnerability in axios allows SSRF", "axios"),
        (
            "GHSA-1234-abcd-wxyz: tokio broadcast channel use-after-free",
            "tokio",
        ),
        ("Tokio 1.45", "tokio"),
        ("tauri v2.12.0", "tauri"),
        ("Vite 8 is out", "vite"),
        ("TypeScript 7.0 Beta", "typescript"),
        ("Upgrading to React 19: what changed", "react"),
        ("Next.js 16 deprecates the pages router", "next"),
        (
            "TypeScript 7 Is Up to 10x Faster. Should You Upgrade Now?",
            "typescript",
        ),
        ("Tauri 2.0 Stable Release", "tauri"),
        ("Announcing TypeScript 6.0 RC", "typescript"),
    ];
    for (title, pkg) in cases {
        assert!(event(title, "", pkg), "must be a dependency event: {title}");
    }
}

#[test]
fn tutorials_and_essays_that_use_a_dependency_are_not_events() {
    let cases = [
        ("Progressive Hydration in React — Client Islands & Triggers", "react"),
        (
            "The Native TypeScript Compiler Cut Our Typecheck from 13s to 3.5s",
            "typescript",
        ),
        ("React 19.3 ViewTransition: Animate State Without Losing It", "react"),
        ("React.js ~New feature in React 19.3~", "react"),
        (
            "A Function-level Dataset of Vulnerable and Fixed Source Code in JavaScript and TypeScript",
            "typescript",
        ),
        ("tsrs: Rust port of the TypeScript 7 type checker", "typescript"),
        ("We Stopped Letting the Model Write JSX in React", "react"),
        (
            "Building Deterministic Aviation Physics in TypeScript: Why the 120-Ft Rule",
            "typescript",
        ),
        (
            "UUID v4 vs. UUID v7 vs. ULID in 2026: Database Index Performance",
            "uuid",
        ),
        ("Deser: Rethinking Rust Serialization beyond serde", "serde"),
    ];
    for (title, pkg) in cases {
        assert!(
            !event(title, "", pkg),
            "must NOT be a dependency event: {title}"
        );
    }
}

#[test]
fn a_titled_tutorial_cannot_borrow_release_words_from_its_body() {
    // The title names React and decides; the body's history lesson about
    // React's releases is not an event happening to the reader's dependency.
    assert!(!event(
        "Progressive Hydration in React",
        "Since React 18 was released, streaming SSR lets islands hydrate on demand.",
        "react",
    ));
}

#[test]
fn a_body_mention_counts_only_with_event_words_beside_it() {
    assert!(event(
        "Supply-chain attack hits a popular HTTP client",
        "Attackers published malicious axios versions to npm overnight.",
        "axios",
    ));
    // A bare body version literal is usage, not an event.
    assert!(!event(
        "How we built our dashboard",
        "We use react 18.2 with a custom store and a lot of patience.",
        "react",
    ));
    // The event word sits in the title, not beside the body mention.
    assert!(!event(
        "OpenAI announces a new reasoning model",
        "The demo app is built with react and runs in the browser.",
        "react",
    ));
}

#[test]
fn a_title_naming_one_dependency_decides_for_all_of_them() {
    // Live 2026-10-04 (item 128749): the title names typescript; the body's
    // event vocabulary sits beside typescript-eslint, a second dependency the
    // title never names (live: "native 7.0 ships no importable compiler api";
    // here a release word, which counts in a body).
    let deps = [dep("typescript"), dep("typescript-eslint")];
    let body = "The real work was keeping typescript-eslint alive: its latest \
                release cannot load the native compiler.";
    assert!(!is_dependency_event(&editorial(
        "The Native TypeScript Compiler Cut Our Typecheck from 13s to 3.5s",
        body,
        &deps
    )));
    // Without the titled dependency, the body evidence stands.
    let only_eslint = [dep("typescript-eslint")];
    assert!(is_dependency_event(&editorial(
        "Our typecheck got faster",
        body,
        &only_eslint
    )));
}

#[test]
fn a_version_word_in_a_long_title_needs_the_event_beside_the_name() {
    // Live 2026-10-04 (item 143914): "v4"/"v7" are UUID versions, and
    // "Migrate" closes a long comparison-guide title.
    assert!(!event(
        "UUID v4 vs. UUID v7 vs. ULID in 2026: Database Index Performance, B-Tree \
         Fragmentation & When to Migrate",
        "",
        "uuid",
    ));
}

#[test]
fn compromises_in_prose_are_not_a_compromised_package() {
    // Live 2026-10-04 (item 132770): an essay about replacing serde.
    assert!(!event(
        "Deser: Rethinking Rust Serialization",
        "Actually replacing serde is tricky because of the might that it has in \
         the ecosystem, and making some potentially painful compromises.",
        "serde",
    ));
}

#[test]
fn headline_verbs_count_in_titles_only() {
    // Live 2026-10-04 (item 115334), a bundle-size write-up.
    assert!(!event(
        "Why My Next.js Client Bundle Was Bigger Than I Thought",
        "Everything reachable from a 'use client' module ships to the browser,          plus the react and next.js runtime and the RSC payload.",
        "react",
    ));
}

#[test]
fn a_research_paper_is_not_news_about_a_dependency() {
    let deps = [dep("typescript")];
    let mut paper = editorial(
        "A Function-level Dataset of Vulnerable and Fixed Source Code in JavaScript and TypeScript",
        "",
        &deps,
    );
    paper.source_type = "arxiv";
    assert!(!is_dependency_event(&paper));
    // Even a paper whose title would otherwise read as an event.
    let mut release = editorial("Announcing TypeScript 7.0", "", &deps);
    release.source_type = "arxiv";
    assert!(!is_dependency_event(&release));
}

#[test]
fn bare_http_patch_is_not_a_security_event() {
    assert!(!event(
        "TypeScript Partial and Required for PATCH requests",
        "",
        "typescript",
    ));
}

#[test]
fn editorial_claim_needs_strong_grounding_and_event_evidence() {
    let deps = [dep("react")];
    assert!(is_dependency_event(&editorial(
        "React 20 ships with native signals",
        "",
        &deps
    )));
    assert!(!is_dependency_event(&editorial(
        "Progressive Hydration in React",
        "",
        &deps
    )));
    let mut ungrounded = editorial("React 20 ships with native signals", "", &deps);
    ungrounded.strongly_grounded = false;
    assert!(!is_dependency_event(&ungrounded));
    // A weak (sub-floor) match never carries the claim even when the item is
    // grounded by another dependency.
    let mut weak = dep("react");
    weak.confidence = 0.2;
    let weak = [weak, dep("lodash")];
    assert!(!is_dependency_event(&editorial(
        "React 20 ships with native signals",
        "",
        &weak
    )));
}

#[test]
fn scoped_package_event_is_found_by_its_written_name() {
    let deps = [dep("@tauri-apps/api")];
    assert!(is_dependency_event(&editorial(
        "@tauri-apps/api 2.12 released with a new event API",
        "",
        &deps
    )));
}

#[test]
fn registry_advisory_follows_applicability() {
    let mut inp = editorial("[GHSA-c9xm-49cp-xcr9] rmcp OAuth client", "", &[]);
    inp.source_type = "cve";
    inp.registry_advisory = true;
    inp.strongly_grounded = false;
    for (app, want) in [
        (Some("affected"), true),
        (Some("likely_affected"), true),
        (Some("not_affected"), false),
        (None, false),
    ] {
        inp.applicability = app;
        assert_eq!(is_dependency_event(&inp), want, "{app:?}");
    }
    inp.applicability = None;
    inp.security_confirmed = true;
    assert!(is_dependency_event(&inp));
}

#[test]
fn registry_release_follows_the_grade() {
    let mut inp = editorial("crates.io: tauri v2.12.1", "", &[]);
    inp.source_type = "crates_io";
    inp.via_registry_subject = true;
    for (class, want) in [
        (Some(ReleaseClass::Breaking), true),
        (Some(ReleaseClass::Minor), true),
        (Some(ReleaseClass::Yanked), true),
        (Some(ReleaseClass::Patch), false),
        (Some(ReleaseClass::Prerelease), false),
        (None, true),
    ] {
        inp.release_class = class;
        assert_eq!(is_dependency_event(&inp), want, "{class:?}");
    }
    inp.release_class = Some(ReleaseClass::Minor);
    inp.already_installed_release = true;
    assert!(!is_dependency_event(&inp), "a release every project runs");
    // A registry row whose subject is NOT the user's dependency.
    inp.already_installed_release = false;
    inp.via_registry_subject = false;
    inp.strongly_grounded = false;
    assert!(!is_dependency_event(&inp));
}
