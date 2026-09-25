// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! v37 release grading through the full pipeline: the registry-subject domain
//! override and the per-project grade (see `scoring::release_grade`).

use super::*;
use crate::scoring::release_grade;

const OFFSET: f32 = scoring_config::SCORE_OFFSET_NEGATIVE_FLOOR;

fn ctx_with(packages: &[&str], primary: &[&str]) -> crate::scoring::ScoringContext {
    let mut ace_ctx = ACEContext::default();
    for package in packages {
        let normalized = dependencies::normalize_package_name(package);
        let info = dependencies::DepInfo {
            package_name: normalized.clone(),
            version: None,
            is_dev: false,
            is_direct: true,
            search_terms: dependencies::extract_search_terms(package),
            ecosystem: "rust".to_string(),
            project_paths: Vec::new(),
            project_relevance: 1.0,
        };
        for term in &info.search_terms {
            ace_ctx.dependency_names.insert(term.clone());
        }
        ace_ctx.dependency_names.insert(normalized.clone());
        ace_ctx.dependency_info.insert(normalized, info);
    }
    let mut profile = crate::domain_profile::DomainProfile::default();
    for p in primary {
        profile.primary_stack.insert((*p).to_string());
        profile.all_tech.insert((*p).to_string());
    }
    crate::scoring::ScoringContext::builder()
        .ace_ctx(ace_ctx)
        .domain_profile(profile)
        .build()
}

fn opts() -> ScoringOptions {
    ScoringOptions {
        apply_freshness: true,
        apply_signals: true,
        trend_topics: vec![],
    }
}

struct Row {
    title: String,
    content: String,
    source_id: String,
    embedding: Vec<f32>,
    published: chrono::DateTime<chrono::Utc>,
}

impl Row {
    fn crate_release(name: &str, version: &str, content: &str) -> Self {
        Self {
            title: format!("crates.io: {name} v{version}"),
            content: content.to_string(),
            source_id: format!("crate-{name}@{version}"),
            embedding: crate::test_utils::seed_embedding(&format!("{name}-{version}")),
            published: chrono::Utc::now() - chrono::Duration::days(2),
        }
    }

    fn input(&self) -> ScoringInput<'_> {
        ScoringInput {
            id: 1,
            title: &self.title,
            url: Some("https://crates.io/crates/x"),
            content: &self.content,
            source_type: "crates_io",
            embedding: &self.embedding,
            created_at: Some(&self.published),
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
            source_id: Some(&self.source_id),
        }
    }
}

fn breakdown(r: &SourceRelevance) -> &ScoreBreakdown {
    r.score_breakdown
        .as_ref()
        .expect("the pipeline always writes a breakdown")
}

/// Live 2026-09-25: `crates.io: fastembed v7.1.0` (4DA pins 5.13.4) took
/// domain 0.15 from its topic words ("vector embeddings") because the
/// dependency override needed a 0.50 match and a single subject match scores
/// ~0.28. It left the feed at 0.155 with no LLM involved.
#[test]
fn a_release_of_your_dependency_is_in_domain_whatever_its_words() {
    let db = crate::test_utils::test_db();
    db.store_dependency(
        "/proj/app",
        "fastembed",
        Some("5.13.4"),
        "rust",
        false,
        None,
    )
    .unwrap();
    let ctx = ctx_with(&["fastembed"], &["rust"]);
    let row = Row::crate_release(
        "fastembed",
        "7.1.0",
        "Library for generating vector embeddings, reranking locally.\nDownloads: 2682163",
    );
    let r = score_item(&row.input(), &ctx, &db, &opts(), None);
    let b = breakdown(&r);
    assert!(
        (b.domain_relevance - 1.0).abs() < 1e-6,
        "the subject IS the user's dependency (got {})",
        b.domain_relevance
    );
    assert_eq!(b.necessity_category.as_deref(), Some("breaking_change"));
    assert_eq!(
        b.necessity_reason.as_deref(),
        Some("Breaking upgrade: fastembed 7.1.0 for proj/app on 5.13.4")
    );
    assert_eq!(b.installed_version.as_deref(), Some("5.13.4"));
    assert_eq!(b.score_ceiling, None, "a breaking upgrade keeps its score");
    let dep_factor = b
        .explanation_factors
        .iter()
        .find(|f| f.kind == crate::FactorKind::DependencyMatch)
        .expect("the dependency factor is present");
    assert_eq!(
        dep_factor.display,
        "Breaking upgrade of your dependency fastembed"
    );
    assert!(
        dep_factor.evidence.contains("proj/app on 5.13.4"),
        "{}",
        dep_factor.evidence
    );
}

/// Control: the override is the registry SUBJECT, never a mention. A crate
/// that is not the user's dependency keeps its topic-derived domain.
#[test]
fn a_crate_you_do_not_use_gets_no_domain_override() {
    let db = crate::test_utils::test_db();
    let ctx = ctx_with(&["tokio"], &["rust"]);
    let row = Row::crate_release(
        "fastembed",
        "7.1.0",
        "Library for generating vector embeddings, reranking locally.",
    );
    let r = score_item(&row.input(), &ctx, &db, &opts(), None);
    assert!(breakdown(&r).domain_relevance < 1.0);
    assert!(
        !r.relevant,
        "an ungrounded registry release never reaches the feed"
    );
}

#[test]
fn a_patch_release_leaves_the_feed() {
    let db = crate::test_utils::test_db();
    db.store_dependency(
        "/proj/app",
        "thiserror",
        Some("2.0.18"),
        "rust",
        false,
        None,
    )
    .unwrap();
    let ctx = ctx_with(&["thiserror"], &["rust"]);
    let row = Row::crate_release("thiserror", "2.0.21", "derive(Error) for Rust.");
    let r = score_item(&row.input(), &ctx, &db, &opts(), None);
    let b = breakdown(&r);
    assert!(!r.relevant, "a patch is not news on its own");
    assert!(
        b.score_ceiling
            .is_some_and(
                |c| (c - (scoring_config::RELEASE_GRADE_PATCH_CEILING + OFFSET)).abs() < 1e-4
            ),
        "held at the patch ceiling (got {:?})",
        b.score_ceiling
    );
    assert_eq!(
        b.necessity_reason.as_deref(),
        Some("Patch release thiserror 2.0.21 (proj/app on 2.0.18)")
    );
}

#[test]
fn a_new_minor_ranks_below_a_breaking_upgrade() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/proj/app", "tokio", Some("1.52.3"), "rust", false, None)
        .unwrap();
    let ctx = ctx_with(&["tokio"], &["rust"]);
    let row = Row::crate_release(
        "tokio",
        "1.53.1",
        "An event-driven, non-blocking I/O platform.",
    );
    let r = score_item(&row.input(), &ctx, &db, &opts(), None);
    let b = breakdown(&r);
    assert!(b
        .score_ceiling
        .is_some_and(
            |c| (c - (scoring_config::RELEASE_GRADE_MINOR_CEILING + OFFSET)).abs() < 1e-4
        ));
    assert!(r.top_score <= scoring_config::RELEASE_GRADE_MINOR_CEILING + OFFSET + 1e-4);
    assert_eq!(b.necessity_category.as_deref(), Some("ecosystem_shift"));
    assert_eq!(
        b.necessity_reason.as_deref(),
        Some("New tokio 1.53.1 for proj/app on 1.52.3")
    );
}

/// Live 2026-09-24: seven `tauri 3.0.0-alpha` rows at 0.90 above every
/// stable release of the user's dependencies.
#[test]
fn a_prerelease_leaves_the_feed() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/proj/app", "tauri", Some("2.11.6"), "rust", false, None)
        .unwrap();
    let ctx = ctx_with(&["tauri"], &["rust", "tauri"]);
    let row = Row::crate_release(
        "tauri",
        "3.0.0-alpha.2",
        "Make tiny, secure apps for all desktop platforms.",
    );
    let r = score_item(&row.input(), &ctx, &db, &opts(), None);
    let b = breakdown(&r);
    assert!(!r.relevant, "one registry row per plugin alpha is not news");
    assert!(b
        .score_ceiling
        .is_some_and(
            |c| (c - (scoring_config::RELEASE_GRADE_PRERELEASE_CEILING + OFFSET)).abs() < 1e-4
        ));
    assert!(r.top_score <= scoring_config::RELEASE_GRADE_PRERELEASE_CEILING + OFFSET + 1e-4);
    const {
        assert!(
            scoring_config::RELEASE_GRADE_PRERELEASE_CEILING
                < scoring_config::RELEASE_GRADE_MINOR_CEILING
        );
    }
}

/// Live 2026-09-25: `crates.io: sha2 v0.11.0` read "installed v0.11.0" (the
/// 4DA copy) while two sibling projects, on 0.10.9, were the projects it
/// concerned.
#[test]
fn the_projects_behind_are_named_not_the_current_copy() {
    let db = crate::test_utils::test_db();
    for (path, v) in [
        ("/proj/app", "0.11.0"),
        ("/proj/tools", "0.10.9"),
        ("/proj/web", "0.10.9"),
    ] {
        db.store_dependency(path, "sha2", Some(v), "rust", false, None)
            .unwrap();
    }
    let ctx = ctx_with(&["sha2"], &["rust"]);
    let row = Row::crate_release(
        "sha2",
        "0.11.0",
        "Pure Rust implementation of the SHA-2 hash functions.",
    );
    let r = score_item(&row.input(), &ctx, &db, &opts(), None);
    let b = breakdown(&r);
    assert_eq!(b.installed_version.as_deref(), Some("0.10.9"));
    assert_eq!(b.necessity_category.as_deref(), Some("breaking_change"));
    let reason = b.necessity_reason.as_deref().unwrap_or_default();
    assert!(
        reason.contains("proj/tools (+1 more) on 0.10.9"),
        "{reason}"
    );
    let dep_factor = b
        .explanation_factors
        .iter()
        .find(|f| f.kind == crate::FactorKind::DependencyMatch)
        .unwrap();
    assert!(
        dep_factor.evidence.contains("already current: proj/app"),
        "{}",
        dep_factor.evidence
    );
}

#[test]
fn a_yanked_pin_is_not_hidden_as_already_installed() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/proj/app", "rusqlite", Some("0.26.1"), "rust", false, None)
        .unwrap();
    let ctx = ctx_with(&["rusqlite"], &["rust"]);
    let row = Row::crate_release(
        "rusqlite",
        "0.26.1",
        "Ergonomic wrapper for SQLite\nDownloads: 101385377\nYanked versions: 0.26.1, 0.26.0",
    );
    let r = score_item(&row.input(), &ctx, &db, &opts(), None);
    let b = breakdown(&r);
    assert_eq!(b.necessity_category.as_deref(), Some("breaking_change"));
    assert_eq!(
        b.score_ceiling, None,
        "not held at the already-installed ceiling"
    );
    assert!(b
        .necessity_reason
        .as_deref()
        .is_some_and(|r| r.contains("was yanked")));
}

#[test]
fn every_class_is_reachable_from_the_pipeline_inputs() {
    // A structural guard: the four ceilings the pipeline can apply come from
    // the DSL, and the grade's patch ceiling keeps the verdict-gate margin
    // (a patch must never reach the 0.40 line with the post-ceiling offset).
    const {
        assert!(scoring_config::RELEASE_GRADE_PATCH_CEILING <= 0.37);
        assert!(scoring_config::RELEASE_GRADE_PRERELEASE_CEILING <= 0.37);
        assert!(
            scoring_config::RELEASE_GRADE_MINOR_CEILING
                > scoring_config::RELEASE_GRADE_PRERELEASE_CEILING
        );
    }
    let g = release_grade::ReleaseGrade::from_pins(
        "x",
        semver::Version::parse("2.0.0").unwrap(),
        vec![("/p".to_string(), "1.0.0".to_string(), true, false)],
        &[],
    );
    assert_eq!(g.class(), Some(release_grade::ReleaseClass::Breaking));
}

/// Live E2E 2026-09-25: `npm: stripe v22.6.2` for the site (pinned 20.3.1,
/// two majors behind) scored 0.396 — under the line. A breaking upgrade of a
/// RUNTIME dependency always reaches the feed; a dev-only pin does not get
/// the floor.
#[test]
fn a_breaking_upgrade_of_a_runtime_dependency_always_reaches_the_feed() {
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let published = chrono::Utc::now() - chrono::Duration::days(2);
    let input = |source_id: &'static str| ScoringInput {
        id: 1,
        title: "crates.io: paycrate v22.6.2",
        url: Some("https://crates.io/crates/paycrate"),
        content: "Client library.",
        source_type: "crates_io",
        embedding: &zero,
        created_at: Some(&published),
        detected_lang: "en",
        source_tags: &[],
        tags_json: None,
        feed_origin: None,
        source_id: Some(source_id),
    };
    let ctx = ctx_with(&["paycrate"], &["rust"]);
    let floor = get_relevance_threshold() + scoring_config::RELEASE_GRADE_BREAKING_FLOOR_MARGIN;

    let db = crate::test_utils::test_db();
    db.store_dependency(
        "/proj/site",
        "paycrate",
        Some("20.3.1"),
        "rust",
        false,
        None,
    )
    .unwrap();
    let r = score_item(&input("crate-paycrate@22.6.2"), &ctx, &db, &opts(), None);
    assert!(r.relevant, "a runtime breaking upgrade is feed-relevant");
    assert!(
        r.top_score >= floor - 1e-4,
        "floored above the line (got {})",
        r.top_score
    );

    let dev_db = crate::test_utils::test_db();
    dev_db
        .store_dependency("/proj/site", "paycrate", Some("20.3.1"), "rust", true, None)
        .unwrap();
    let dev = score_item(
        &input("crate-paycrate@22.6.2"),
        &ctx,
        &dev_db,
        &opts(),
        None,
    );
    assert_eq!(
        breakdown(&dev).necessity_category.as_deref(),
        Some("breaking_change"),
        "still graded breaking"
    );
    assert!(
        dev.top_score < floor - 1e-4,
        "a dev-only pin is left to its own score, never lifted by the floor (got {})",
        dev.top_score
    );
    assert!(!dev.relevant);
}
