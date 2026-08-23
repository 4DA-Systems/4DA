// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Registry-subject grounding regression corpus (2026-07-13 live-feed audit).
//!
//! Every negative fixture here is a REAL item that topped the live "Affects
//! You" pool by riding a text mention of a user dependency inside a
//! third-party package's title/description. The doctrine under test:
//! **a registry release item is grounded ONLY by its subject package** —
//! subject ∈ user deps, ecosystem-congruent. Text mentions can never
//! strongly-ground a registry item, never mint "New release in your stack",
//! and never award the fast-path floors.
//!
//! Run: `cargo test scoring::registry_grounding -- --nocapture`

use super::benchmark::{bench_db, no_freshness};
use super::benchmark_scenarios::profile_ctx;
use super::types::ScoringInput;
use super::*;

fn score<'a>(
    ctx: &ScoringContext,
    db: &crate::db::Database,
    source_type: &'a str,
    source_id: Option<&'a str>,
    title: &'a str,
    content: &'a str,
) -> crate::SourceRelevance {
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let opts = no_freshness();
    let input = ScoringInput {
        id: 1,
        title,
        url: Some("https://example.com"),
        content,
        source_type,
        embedding: &zero_emb,
        created_at: None,
        detected_lang: "en",
        source_tags: &[],
        tags_json: None,
        feed_origin: None,
        source_id,
    };
    score_item(&input, ctx, db, &opts, None)
}

fn grounded(r: &crate::SourceRelevance) -> bool {
    r.score_breakdown
        .as_ref()
        .map(|b| b.strongly_grounded)
        .unwrap_or(false)
}

fn necessity_category(r: &crate::SourceRelevance) -> Option<String> {
    r.score_breakdown
        .as_ref()
        .and_then(|b| b.necessity_category.clone())
}

/// The live #1 item: a zero-download placeholder crate whose description says
/// "for Tauri apps". It strongly-grounded to the user's `tauri` dep and was
/// classified "New release in your stack: tauri".
#[test]
fn junk_crate_mentioning_dep_is_not_grounded() {
    let db = bench_db();
    let ctx = profile_ctx("rust_developer"); // has `tauri` (rust, direct)
    let r = score(
        &ctx,
        &db,
        "crates_io",
        Some("crate-capacitor-tauri"),
        "crates.io: capacitor-tauri v0.0.0",
        "Capacitor platform runtime for Tauri apps.\nDownloads: 0",
    );
    assert!(
        !grounded(&r),
        "third-party crate mentioning 'Tauri' must not strongly-ground (breakdown: {:?})",
        r.score_breakdown.as_ref().map(|b| &b.matched_deps)
    );
    assert_ne!(
        necessity_category(&r).as_deref(),
        Some("ecosystem_shift"),
        "junk crate must not be classified as the user's stack update: {:?}",
        r.score_breakdown
            .as_ref()
            .and_then(|b| b.necessity_reason.clone())
    );
}

/// Suffix-compound title: "tauri" inside "crepuscularity-tauri" rode a
/// full-strength title hit pre-fix (the damper only checked term-THEN-hyphen).
#[test]
fn junk_crate_suffix_compound_title_is_not_grounded() {
    let db = bench_db();
    let ctx = profile_ctx("rust_developer");
    let r = score(
        &ctx,
        &db,
        "crates_io",
        Some("crate-crepuscularity-tauri"),
        "crates.io: crepuscularity-tauri v0.1.0",
        "Tauri v1/v2 static-bundle adapter for Crepuscularity.\nDownloads: 0",
    );
    assert!(!grounded(&r), "suffix-compound junk crate must not ground");
    assert_ne!(necessity_category(&r).as_deref(), Some("ecosystem_shift"));
}

/// Cross-ecosystem: a RUST crate whose description says "React-like" grounded
/// to the user's JAVASCRIPT `react` dependency on the live feed.
#[test]
fn cross_ecosystem_mention_is_not_grounded() {
    let db = bench_db();
    let ctx = profile_ctx("fullstack_js"); // has `react` (javascript, direct)
    let r = score(
        &ctx,
        &db,
        "crates_io",
        Some("crate-orbit-md"),
        "crates.io: orbit-md v0.1.0",
        "Fast static site generator with React-like Markdown components\nDownloads: 0",
    );
    assert!(
        !grounded(&r),
        "a Rust crate mentioning React must not ground the JS `react` dep"
    );
}

/// Positive control: a release of the user's OWN dependency must ground and
/// must be classified as a stack update — the registry-subject route must not
/// throw the baby out with the bathwater.
#[test]
fn release_of_user_dep_is_grounded_stack_update() {
    let db = bench_db();
    let ctx = profile_ctx("rust_developer");
    let r = score(
        &ctx,
        &db,
        "crates_io",
        Some("crate-tauri"),
        "crates.io: tauri v2.9.0",
        "Build smaller, faster, and more secure desktop applications with a web frontend.\nDownloads: 4200000",
    );
    assert!(
        grounded(&r),
        "a release of the user's own `tauri` dep MUST strongly-ground"
    );
    assert_eq!(
        necessity_category(&r).as_deref(),
        Some("ecosystem_shift"),
        "own-dep release should be a stack update (reason: {:?})",
        r.score_breakdown
            .as_ref()
            .and_then(|b| b.necessity_reason.clone())
    );
}

/// Fallback path: ad-hoc scoring without a source_id keeps the corroborated
/// text route, so a genuine own-dep release still grounds.
#[test]
fn release_of_user_dep_without_source_id_still_grounds() {
    let db = bench_db();
    let ctx = profile_ctx("rust_developer");
    let r = score(
        &ctx,
        &db,
        "crates_io",
        None,
        "crates.io: tauri v2.9.0",
        "Build smaller, faster, and more secure desktop applications with a web frontend.",
    );
    assert!(
        grounded(&r),
        "own-dep release must still ground on the no-source_id fallback path"
    );
}

/// Ecosystem congruence on the subject route itself: an npm package named
/// `tauri` (a squatter of the Rust crate's name) must not ground the user's
/// RUST `tauri` dependency.
#[test]
fn registry_subject_respects_ecosystem() {
    let db = bench_db();
    let ctx = profile_ctx("rust_developer"); // tauri is a RUST dep here
    let r = score(
        &ctx,
        &db,
        "npm_registry",
        Some("tauri@0.1.0"),
        "npm: tauri v0.1.0",
        "Placeholder package.",
    );
    assert!(
        !grounded(&r),
        "npm squatter of a Rust dep's name must not ground cross-ecosystem"
    );
}

/// Chips honesty: on a junk registry item, no matched dep may keep a
/// `corroborated` flag — "named in the item text" evidence must not render.
#[test]
fn junk_registry_item_has_no_corroborated_deps() {
    let db = bench_db();
    let ctx = profile_ctx("rust_developer");
    let r = score(
        &ctx,
        &db,
        "crates_io",
        Some("crate-capacitor-tauri"),
        "crates.io: capacitor-tauri v0.0.0",
        "Capacitor platform runtime for Tauri apps.\nDownloads: 0",
    );
    // The evidence chain's dependency factor only renders corroborated deps;
    // signal trigger chips filter on the same flag. Verify via the factors:
    // no "Names your dependency" display line may survive.
    if let Some(b) = &r.score_breakdown {
        for f in &b.explanation_factors {
            assert!(
                !f.display.to_lowercase().contains("names your dependency"),
                "junk registry item must not carry name-evidence chips: {}",
                f.display
            );
        }
    }
}

/// Advisory truth (2026-07-09 OSV backfill flood): a historical advisory whose
/// fix PRE-DATES the user's installed version must read `not_affected`, never
/// page as critical, and carry the honest awareness-only necessity reason.
/// A still-affecting advisory for the same dep must keep its urgency.
#[test]
fn patched_advisory_is_not_affected_and_never_critical() {
    let db = bench_db();
    let opts = ScoringOptions {
        apply_freshness: false,
        apply_signals: true,
        trend_topics: vec![],
    };
    let classifier = crate::signals::SignalClassifier::new();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    let mut ctx = profile_ctx("fullstack_js");
    // axios installed at 1.13.0 — PAST the 1.12.2 fix below.
    let info = super::dependencies::DepInfo {
        package_name: "axios".to_string(),
        version: Some("1.13.0".to_string()),
        is_dev: false,
        is_direct: true,
        search_terms: super::dependencies::extract_search_terms("axios"),
        ecosystem: "javascript".to_string(),
    };
    ctx.ace_ctx.dependency_names.insert("axios".to_string());
    ctx.ace_ctx
        .dependency_info
        .insert("axios".to_string(), info);

    let score_advisory = |id: u64, title: &str, content: &str| {
        let input = ScoringInput {
            id,
            title,
            url: Some("https://example.com"),
            content,
            source_type: "osv",
            embedding: &zero_emb,
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
            source_id: Some("GHSA-test"),
        };
        score_item(&input, &ctx, &db, &opts, Some(&classifier))
    };

    // Historical, long-fixed advisory: fixed in 1.12.2, user runs 1.13.0.
    let patched = score_advisory(
        1,
        "[GHSA-wf5p-g6vw-rhxx] Axios Cross-Site Request Forgery Vulnerability",
        "An issue in axios exposes a CSRF vector.\nSeverity: HIGH\n\
         Affected: axios (npm)\nFixed in: 1.12.2",
    );
    assert_eq!(
        patched.applicability.as_deref(),
        Some("not_affected"),
        "installed 1.13.0 >= fix 1.12.2 must read not_affected"
    );
    assert!(
        !patched.is_critical_alert,
        "patched advisory must not be a critical alert"
    );
    assert_ne!(
        patched.signal_priority.as_deref(),
        Some("critical"),
        "patched advisory must not page critical"
    );
    let bd = patched.score_breakdown.as_ref().expect("breakdown");
    assert!(
        bd.necessity_score <= 0.30,
        "patched advisory necessity must be awareness-only, got {}",
        bd.necessity_score
    );

    // Control: an advisory whose fix is AHEAD of the installed version still
    // carries its full urgency — the version gate must not eat real alerts.
    let affecting = score_advisory(
        2,
        "[GHSA-9999-xxxx-yyyy] Axios: Header Injection via Prototype Pollution",
        "A vulnerability in axios allows header injection via prototype pollution.\n\
         Severity: CRITICAL\nAffected: axios (npm)\nFixed in: 1.14.1",
    );
    assert_ne!(
        affecting.applicability.as_deref(),
        Some("not_affected"),
        "installed 1.13.0 < fix 1.14.1 is still affected"
    );
    assert!(
        affecting
            .score_breakdown
            .as_ref()
            .map(|b| b.necessity_score)
            .unwrap_or(0.0)
            > 0.5,
        "still-affecting advisory keeps real necessity"
    );
}

/// Degraded-input markers (2026-08-23 audit, item 11): a scoring run whose
/// inputs silently collapsed must say so on the persisted breakdown. The
/// persist-skip POLICY lands in a later wave — these tests pin the honest
/// carrier only.
mod degraded_inputs {
    use super::*;

    fn breakdown_markers(r: &crate::SourceRelevance) -> Vec<String> {
        r.score_breakdown
            .as_ref()
            .map(|b| b.degraded_inputs.clone())
            .unwrap_or_default()
    }

    #[test]
    fn knn_failure_marks_results_degraded() {
        let db = bench_db();
        // Break the KNN table so find_similar_contexts errors. bench_db is
        // in-memory, so read_conn() falls back to the single writer conn —
        // the DROP is visible to the scoring query.
        db.read_conn()
            .execute_batch("DROP TABLE IF EXISTS context_vec;")
            .expect("drop context_vec");
        let mut ctx = profile_ctx("rust_developer");
        ctx.cached_context_count = 3; // context axis SHOULD run…
        let emb = vec![0.5_f32; crate::EMBEDDING_DIMS]; // …with a real embedding
        let opts = no_freshness();
        let input = ScoringInput {
            id: 91,
            title: "Async runtime scheduling deep dive",
            url: Some("https://example.com"),
            content: "How work-stealing schedulers balance tasks across threads.",
            source_type: "hackernews",
            embedding: &emb,
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
            source_id: None,
        };
        let r = score_item(&input, &ctx, &db, &opts, None);
        assert!(
            breakdown_markers(&r)
                .iter()
                .any(|m| m == "context_knn_failed"),
            "a KNN failure must mark the breakdown degraded (got {:?})",
            breakdown_markers(&r)
        );
    }

    #[test]
    fn zero_embedding_marks_results_degraded() {
        let db = bench_db();
        let ctx = profile_ctx("rust_developer");
        // The shared score() helper uses an all-zero embedding — exactly the
        // OSV/CVE zero-blob retention case: semantic axes default, not measure.
        let r = score(
            &ctx,
            &db,
            "hackernews",
            None,
            "Async runtime scheduling deep dive",
            "How work-stealing schedulers balance tasks across threads.",
        );
        assert!(
            breakdown_markers(&r)
                .iter()
                .any(|m| m == "embedding_missing"),
            "an absent embedding must mark the breakdown degraded (got {:?})",
            breakdown_markers(&r)
        );
    }

    #[test]
    fn dep_intel_load_failure_marker_is_carried_and_cleared() {
        let db = bench_db();
        let ctx = profile_ctx("rust_developer");
        super::super::dependencies::set_dep_intel_load_degraded_for_test(true);
        let degraded = score(
            &ctx,
            &db,
            "hackernews",
            None,
            "Async runtime scheduling deep dive",
            "How work-stealing schedulers balance tasks across threads.",
        );
        // Clear immediately — the flag is process-global.
        super::super::dependencies::set_dep_intel_load_degraded_for_test(false);
        assert!(
            breakdown_markers(&degraded)
                .iter()
                .any(|m| m == "dep_intel_load_failed"),
            "a failed dep-intel load must mark the breakdown degraded"
        );
        let healthy = score(
            &ctx,
            &db,
            "hackernews",
            None,
            "Async runtime scheduling deep dive",
            "How work-stealing schedulers balance tasks across threads.",
        );
        assert!(
            !breakdown_markers(&healthy)
                .iter()
                .any(|m| m == "dep_intel_load_failed"),
            "a healthy load must not carry the marker"
        );
    }
}

/// Dev-dep registry releases (2026-08-23 audit, item 16): a release of a
/// package the user's manifests declare grounds regardless of dev status —
/// a vitest major IS the user's stack — while the Critical paging lane stays
/// production-only.
#[test]
fn dev_dep_registry_release_grounds_but_never_pages() {
    let db = bench_db();
    let mut ctx = profile_ctx("fullstack_js");
    let info = super::dependencies::DepInfo {
        package_name: "vitest".to_string(),
        version: Some("2.1.0".to_string()),
        is_dev: true,
        is_direct: true,
        search_terms: super::dependencies::extract_search_terms("vitest"),
        ecosystem: "javascript".to_string(),
    };
    ctx.ace_ctx.dependency_names.insert("vitest".to_string());
    ctx.ace_ctx
        .dependency_info
        .insert("vitest".to_string(), info);

    let r = score(
        &ctx,
        &db,
        "npm_registry",
        Some("vitest@3.0.0"),
        "npm: vitest v3.0.0",
        "Next generation testing framework powered by Vite.",
    );
    assert!(
        grounded(&r),
        "a release of the user's declared devDep must ground (breakdown: {:?})",
        r.score_breakdown.as_ref().map(|b| &b.matched_deps)
    );
    assert_ne!(
        r.signal_priority.as_deref(),
        Some("critical"),
        "dev-dep releases never page critical"
    );
}

/// Family rule end-to-end (2026-08-23 audit, item 15): a RUSTSEC advisory for
/// `serde_derive` — present only via the lockfile — must engage the critical
/// fast-path for a serde user instead of stalling at the ungrounded-advisory
/// cap (measured 0.414 pre-fix).
#[test]
fn serde_derive_advisory_engages_fast_path_for_serde_user() {
    let db = bench_db();
    // rust_developer now carries serde_derive as a lockfile transitive,
    // exactly as a real serde user's Cargo.lock does.
    let ctx = profile_ctx("rust_developer");
    let opts = ScoringOptions {
        apply_freshness: false,
        apply_signals: true,
        trend_topics: vec![],
    };
    let classifier = crate::signals::SignalClassifier::new();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let input = ScoringInput {
        id: 15,
        title: "RUSTSEC-2026-0042: serde_derive unbounded recursion during deserialization",
        url: Some("https://example.com"),
        content: "serde_derive versions before 1.0.205 allow unbounded recursion when \
                  deserializing deeply nested structures, leading to stack overflow. \
                  Severity: CVSS_V3: 5.3. Update serde_derive to >= 1.0.205.",
        source_type: "cve",
        embedding: &zero_emb,
        created_at: None,
        detected_lang: "en",
        source_tags: &[],
        tags_json: None,
        feed_origin: None,
        source_id: Some("RUSTSEC-2026-0042"),
    };
    let r = score_item(&input, &ctx, &db, &opts, Some(&classifier));
    assert!(
        grounded(&r),
        "lockfile family child must strongly ground (deps: {:?})",
        r.score_breakdown.as_ref().map(|b| &b.matched_deps)
    );
    assert!(
        r.top_score >= 0.50,
        "critical fast-path floor must engage (got {})",
        r.top_score
    );
    assert!(r.relevant, "grounded advisory must be feed-relevant");
}

/// Dep-gate bypass integration (2026-08-23 audit, item 22b): post-bootstrap
/// (feedback_interaction_count >= 10 arms the 2-signal quality floor), a
/// strong direct-dep release whose ONLY confirmed axis is the dependency must
/// carry the LIFTED confirmation multiplier on its breakdown. The unit tests
/// on `apply_gate_effect` pin the score arithmetic (0.72 ceiling reachable);
/// this pins the wiring end-to-end.
#[test]
fn post_bootstrap_single_axis_dep_release_carries_lifted_conf_mult() {
    let db = bench_db();
    let mut ctx = profile_ctx("rust_developer");
    ctx.feedback_interaction_count = 25; // post-bootstrap
    let r = score(
        &ctx,
        &db,
        "crates_io",
        Some("crate-tokio"),
        "crates.io: tokio v1.40.0",
        "A runtime for writing reliable asynchronous applications. tokio 1.40.0 \
         brings scheduler and IO driver improvements.\nDownloads: 4200000",
    );
    let bd = r.score_breakdown.as_ref().expect("breakdown");
    assert!(
        bd.dep_match_score >= crate::scoring_config::DEPENDENCY_GATE_BYPASS_DIRECT_DEP_MIN_SCORE,
        "premise: the dep axis is bypass-strength (got {})",
        bd.dep_match_score
    );
    assert!(
        bd.signal_count <= 1,
        "premise: dependency is the only confirmed axis (got {} — {:?})",
        bd.signal_count,
        bd.confirmed_signals
    );
    assert!(
        (bd.confirmation_mult - crate::scoring_config::CONFIRMATION_GATE[2].0).abs() < f32::EPSILON,
        "bypass must lift conf_mult to the 2-signal tier (got {})",
        bd.confirmation_mult
    );
    // Reachable-band justification (measured 2026-08-23): this harness item
    // carries a ZERO embedding, so its relevance core is semantic-only
    // (~0.18 boosted) and it lands at ~0.22 = boosted x conf 1.00 x domain
    // 1.10 + 0.02 offset — 2.2x the ~0.10 the pre-fix 0.45 multiplier
    // produced for the IDENTICAL item, but honestly below the feed
    // threshold: a release with no semantic evidence at all should not
    // surface on the dep name alone. The [0.70, 0.72+offset] band the
    // audit's intent comment names requires boosted >= ~0.64 (real
    // embedding similarity + quality composite), and its reachability under
    // conf 1.00 is pinned by `dep_gate_bypass_lifts_conf_mult_to_two_signal_
    // tier` (0.90 boosted -> exactly the 0.72 ceiling). Pre-fix, NO boosted
    // value could exceed 0.45 x 1.10 + 0.02 = 0.515 — the band was
    // arithmetically empty.
    assert!(
        (0.19..0.30).contains(&r.top_score),
        "measured band pin for the minimal harness item (got {}) — a shift \
         means the gate arithmetic changed; re-derive the band",
        r.top_score
    );
    assert!(
        r.top_score < scoring_config_bypass_ceiling_plus_offset(),
        "bypass ceiling must still hold the top band out of reach"
    );
}

fn scoring_config_bypass_ceiling_plus_offset() -> f32 {
    crate::scoring_config::DEPENDENCY_GATE_BYPASS_DIRECT_DEP_CEILING
        + crate::scoring_config::SCORE_OFFSET_NEGATIVE_FLOOR
        + f32::EPSILON
}

/// Stale published-content wiring (2026-08-23 audit, item 19): the discount
/// must flow through `score_item`'s quality composite, not just exist as a
/// helper. `created_at` carries published_at on the analysis paths, so a
/// years-old PUBLISH date discounts regardless of fetch date. The ramp,
/// security exemption, and grounded softening are pinned at helper level
/// (`stale_multiplier_*` in pipeline_v2); this pins the end-to-end effect.
#[test]
fn stale_published_content_scores_below_fresh_twin() {
    let db = bench_db();
    let ctx = profile_ctx("rust_developer");
    let opts = ScoringOptions {
        apply_freshness: true,
        apply_signals: false,
        trend_topics: vec![],
    };
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let score_published_at = |id: u64, published: chrono::DateTime<chrono::Utc>| {
        let input = ScoringInput {
            id,
            title: "TypeScript 5.1 Beta is OUT! Deep dive into the new features",
            url: Some("https://example.com"),
            content: "A tour of the new TypeScript beta: decorators, const type \
                      parameters, and improved inference for Rust-interop tooling.",
            source_type: "rss",
            embedding: &zero_emb,
            created_at: Some(&published),
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
            source_id: None,
        };
        score_item(&input, &ctx, &db, &opts, None).top_score
    };
    let fresh = score_published_at(191, chrono::Utc::now() - chrono::Duration::days(2));
    let stale = score_published_at(192, chrono::Utc::now() - chrono::Duration::days(1200));
    assert!(
        stale < fresh,
        "a 40-month-old publish date must score below its fresh twin \
         (fresh {fresh}, stale {stale})"
    );
    // The drop must exceed what the freshness tiers alone explain (their
    // floor is 0.80 vs 1.00 at 2 days): the stale multiplier (0.55) stacks.
    assert!(
        stale < fresh * 0.60,
        "stale discount must stack on the freshness floor (fresh {fresh}, stale {stale})"
    );
}
