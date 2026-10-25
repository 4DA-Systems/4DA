// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! v44: a matcher-CONFIRMED runtime exposure is dependency evidence for the
//! three gates that used to read only the TEXT dependency match (calibration
//! monitor "recall bug misscored=53/62", 2026-10-24).
//!
//! Live rows: twelve Next.js advisories (installed 16.3.6, fixed 16.3.8, each
//! stored once as GHSA and once as CVE) and three `zip` RUSTSEC rows sat at the
//! 0.37 registry-ungrounded ceiling or below, and a transitive rustls 0.23.39
//! at 0.257: the name `next` / `zip` is ambiguous and a transitive copy is
//! never text-matched, so the dependency match was 0 although Preemption's
//! matcher had confirmed the installed version inside the range.

use super::*;
use crate::db::DependencyInstanceInput;
use crate::scoring::security_verdict::{matcher_verdict, SecurityVerdict};

const NEXT_TITLE: &str =
    "[GHSA-cjq9-62q9-8jv4] next: Next.js has Server-Side Request Forgery in Image Optimization";
const NEXT_URL: &str = "https://osv.dev/vulnerability/GHSA-cjq9-62q9-8jv4";
const NEXT_CONTENT: &str = "A Server-Side Request Forgery in the Image Optimization API of \
Next.js lets an attacker make the server fetch arbitrary internal URLs.\n\n\
Severity: MEDIUM\nAffected: next (npm)\nAffected range: next (npm): >= 16.0.0, < 16.3.8";

const RUSTLS_TITLE: &str = "[GHSA-2mjx-qc3c-rqvc] Rustls: TLS 1.3 handshake messages \
incorrectly accepted across encryption boundaries";
const RUSTLS_URL: &str = "https://github.com/advisories/GHSA-2mjx-qc3c-rqvc";
const RUSTLS_CONTENT: &str = "rustls accepted TLS 1.3 handshake messages that straddled a key \
change, letting an attacker inject plaintext handshake data.\n\n\
Severity: MEDIUM\nAffected: rustls (rust)\nAffected range: rustls (rust): >= 0.23.0, < 0.23.45";

const UNDICI_TITLE: &str =
    "[CVE-2026-85024] undici vulnerable to Denial of Service via unhandled error in WebSocket";
const UNDICI_URL: &str = "https://github.com/advisories/GHSA-uuuu-dddd-iiii";
const UNDICI_CONTENT: &str = "An unhandled error in the undici WebSocket client crashes the \
process (denial of service).\n\n\
Severity: MEDIUM\nAffected: undici (npm)\nAffected range: undici (npm): < 8.10.2";

fn options() -> ScoringOptions {
    ScoringOptions {
        apply_freshness: false,
        apply_signals: true,
        trend_topics: vec![],
    }
}

fn registry_input<'a>(
    source_type: &'a str,
    title: &'a str,
    url: &'a str,
    content: &'a str,
    embedding: &'a [f32],
) -> ScoringInput<'a> {
    ScoringInput {
        id: 159_532,
        title,
        url: Some(url),
        content,
        source_type,
        embedding,
        created_at: None,
        detected_lang: "",
        source_tags: &[],
        tags_json: None,
        feed_origin: None,
        source_id: None,
    }
}

/// A web developer's context: the stack declares `next` (direct, 16.3.6) and
/// `react`, exactly as ACE reads the live lockfile.
fn web_ctx() -> crate::scoring::ScoringContext {
    let mut ace_ctx = ACEContext::default();
    for (package, version) in [("next", "16.3.6"), ("react", "19.2.0")] {
        let normalized = dependencies::normalize_package_name(package);
        let info = dependencies::DepInfo {
            package_name: normalized.clone(),
            version: Some(version.to_string()),
            is_dev: false,
            is_direct: true,
            search_terms: dependencies::extract_search_terms(package),
            ecosystem: "javascript".to_string(),
            project_paths: vec!["d:/work/site".to_string()],
            project_relevance: 1.0,
        };
        for term in &info.search_terms {
            ace_ctx.dependency_names.insert(term.clone());
        }
        ace_ctx.dependency_names.insert(normalized.clone());
        ace_ctx.dependency_info.insert(normalized, info);
    }
    ace_ctx.detected_tech.push("javascript".to_string());
    crate::scoring::ScoringContext::builder()
        .ace_ctx(ace_ctx)
        .build()
}

#[allow(clippy::too_many_arguments)]
fn seed_copy(
    db: &Database,
    project: &str,
    package: &str,
    version: &str,
    language: &str,
    registry: &str,
    is_direct: bool,
    is_dev: bool,
) {
    if is_direct {
        db.store_dependency(project, package, Some(version), language, is_dev, None)
            .unwrap();
    } else {
        db.store_transitive_dependency(project, package, Some(version), language, is_dev)
            .unwrap();
    }
    db.store_dependency_instances(
        project,
        registry,
        &[DependencyInstanceInput {
            package_name: package.into(),
            version: version.into(),
            is_direct,
            is_dev,
            scope: "unknown".into(),
        }],
    )
    .unwrap();
}

fn seed_advisory(db: &Database, id: &str, package: &str, ecosystem: &str, fixed: &str) {
    db.upsert_osv_advisory_with_meta(
        id,
        "advisory",
        None,
        package,
        ecosystem,
        Some(&format!(
            r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#
        )),
        Some(&format!(r#"["{fixed}"]"#)),
        Some("CVSS_V3"),
        Some(5.3),
        None,
        None,
        None,
        None,
        None,
        Some("medium"),
    )
    .unwrap();
}

fn score(
    db: &Database,
    ctx: &crate::scoring::ScoringContext,
    input: &ScoringInput,
) -> SourceRelevance {
    let classifier = signals::SignalClassifier::new();
    score_item(input, ctx, db, &options(), Some(&classifier))
}

/// The flagship miss: a confirmed DIRECT exposure on an ambiguous package
/// name. The text match cannot reach the fast-path threshold (that is the
/// bug's precondition), yet the advisory must take the v38 direct floor.
#[test]
fn confirmed_direct_exposure_on_ambiguous_name_reaches_the_direct_floor() {
    let db = crate::test_utils::test_db();
    seed_copy(
        &db,
        "d:/work/site",
        "next",
        "16.3.6",
        "javascript",
        "npm",
        true,
        false,
    );
    seed_advisory(&db, "GHSA-cjq9-62q9-8jv4", "next", "npm", "16.3.8");
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let result = score(
        &db,
        &web_ctx(),
        &registry_input("osv", NEXT_TITLE, NEXT_URL, NEXT_CONTENT, &zero),
    );
    let bd = result.score_breakdown.as_ref().expect("breakdown");
    assert!(
        bd.dep_match_score < scoring_config::CRITICAL_FASTPATH_DEP_MATCH_THRESHOLD,
        "precondition: `next` must not text-match strongly (dep {})",
        bd.dep_match_score
    );
    assert_eq!(bd.is_version_affected, Some(true));
    assert_eq!(bd.dependency_path.as_deref(), Some("direct"));
    assert!(bd.strongly_grounded);
    assert_eq!(bd.score_ceiling, None);
    assert!(
        result.top_score >= scoring_config::CRITICAL_FASTPATH_DIRECT_DEP_FLOOR - 1e-6,
        "a confirmed direct exposure takes the direct floor (score {})",
        result.top_score
    );
    assert!(result.relevant);
    assert_eq!(result.applicability.as_deref(), Some("affected"));
}

/// A confirmed TRANSITIVE exposure (rustls 0.23.39 under a direct dep) is
/// feed-relevant at the generic fast-path floor — one tier below a direct
/// copy, exactly as a text-matched transitive edge.
#[test]
fn confirmed_transitive_exposure_passes_the_line() {
    let db = crate::test_utils::test_db();
    seed_copy(
        &db,
        "d:/work/app/src-tauri",
        "rustls",
        "0.23.39",
        "rust",
        "crates.io",
        false,
        false,
    );
    seed_advisory(&db, "GHSA-2mjx-qc3c-rqvc", "rustls", "crates.io", "0.23.45");
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let result = score(
        &db,
        &web_ctx(),
        &registry_input("cve", RUSTLS_TITLE, RUSTLS_URL, RUSTLS_CONTENT, &zero),
    );
    let bd = result.score_breakdown.as_ref().expect("breakdown");
    assert_eq!(bd.is_version_affected, Some(true));
    assert_eq!(bd.dependency_path.as_deref(), Some("transitive"));
    assert_eq!(bd.score_ceiling, None);
    assert!(
        result.top_score >= scoring_config::CRITICAL_FASTPATH_SCORE_FLOOR - 1e-6,
        "a confirmed transitive exposure takes the fast-path floor (score {})",
        result.top_score
    );
    assert!(result.relevant);
}

/// Dev-only exposures are an open product decision: they keep the pre-v44
/// path — no fast path, the registry-ungrounded ceiling, below the line.
#[test]
fn confirmed_dev_only_exposure_is_unchanged() {
    let db = crate::test_utils::test_db();
    seed_copy(
        &db,
        "d:/work/site",
        "undici",
        "7.28.0",
        "javascript",
        "npm",
        false,
        true,
    );
    seed_advisory(&db, "CVE-2026-85024", "undici", "npm", "8.10.2");
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let result = score(
        &db,
        &web_ctx(),
        &registry_input("cve", UNDICI_TITLE, UNDICI_URL, UNDICI_CONTENT, &zero),
    );
    let bd = result.score_breakdown.as_ref().expect("breakdown");
    assert_eq!(bd.is_version_affected, Some(true), "the advisory is true");
    assert_eq!(bd.dependency_path.as_deref(), Some("dev-only"));
    let ceiling = scoring_config::COMMODITY_CEILING_SECURITY_ADVISORY_REGISTRY_UNGROUNDED
        + scoring_config::SCORE_OFFSET_NEGATIVE_FLOOR;
    assert!(
        result.top_score <= ceiling + 1e-6,
        "dev-only keeps the registry-ungrounded ceiling (score {})",
        result.top_score
    );
    assert!(!result.relevant, "dev-only stays below the line");

    let verdict = matcher_verdict(&db, &["CVE-2026-85024".to_string()]).expect("verdict");
    assert!(verdict.confirmed_runtime_copy(&db).is_none());
}

/// A text-only mention gains nothing: an editorial story naming `next`, and a
/// registry row the mirror does not hold, score bit-identically whether or
/// not a confirmed Next.js exposure exists in the inventory.
#[test]
fn unconfirmed_text_only_mentions_are_unchanged() {
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let story_title = "What's next for Next.js: the App Router after 16.3";
    let story_body = "Next.js 16.3 shipped server actions changes; here is what comes next \
                      for the App Router and image optimization.";
    let unmirrored_title =
        "[GHSA-zzzz-zzzz-zzzz] next: Next.js has a cache confusion in middleware rewrites";
    let score_pair = |seed: bool| -> (f32, f32) {
        let db = crate::test_utils::test_db();
        if seed {
            seed_copy(
                &db,
                "d:/work/site",
                "next",
                "16.3.6",
                "javascript",
                "npm",
                true,
                false,
            );
            seed_advisory(&db, "GHSA-cjq9-62q9-8jv4", "next", "npm", "16.3.8");
        }
        let ctx = web_ctx();
        let mut story = registry_input(
            "hackernews",
            story_title,
            "https://example.com/next",
            story_body,
            &zero,
        );
        story.id = 1;
        let row = registry_input(
            "osv",
            unmirrored_title,
            "https://osv.dev/vulnerability/GHSA-zzzz-zzzz-zzzz",
            NEXT_CONTENT,
            &zero,
        );
        (
            score(&db, &ctx, &story).top_score,
            score(&db, &ctx, &row).top_score,
        )
    };
    let (story_without, row_without) = score_pair(false);
    let (story_with, row_with) = score_pair(true);
    assert_eq!(
        story_with.to_bits(),
        story_without.to_bits(),
        "editorial story"
    );
    assert_eq!(
        row_with.to_bits(),
        row_without.to_bits(),
        "unmirrored registry row"
    );
    assert!(
        story_with < scoring_config::CRITICAL_FASTPATH_DIRECT_DEP_FLOOR,
        "the story never reaches the security floor ({story_with})"
    );
}

/// The predicate itself: runtime copies only, live projects only, never a
/// maintenance notice.
#[test]
fn confirmed_runtime_copy_reads_scope_liveness_and_notices() {
    use crate::osv::advisory_match::ExposedCopy;
    let db = crate::test_utils::test_db();
    let copy = |path: &str, is_direct: bool, is_dev: bool| ExposedCopy {
        project_path: path.to_string(),
        installed_version: "1.0.0".to_string(),
        is_direct,
        is_dev,
    };
    let verdict = |exposed: Vec<ExposedCopy>, maintenance_only: bool| SecurityVerdict {
        affected: Some(!exposed.is_empty()),
        exposed,
        maintenance_only,
        ..SecurityVerdict::default()
    };

    let direct = verdict(vec![copy("d:/work/site", true, false)], false);
    assert!(direct
        .confirmed_runtime_copy(&db)
        .is_some_and(|c| c.is_direct));

    let transitive = verdict(
        vec![
            copy("d:/work/site", false, false),
            copy("d:/work/site", false, true),
        ],
        false,
    );
    assert!(transitive
        .confirmed_runtime_copy(&db)
        .is_some_and(|c| !c.is_direct && !c.is_dev));

    let dev_only = verdict(vec![copy("d:/work/site", true, true)], false);
    assert!(dev_only.confirmed_runtime_copy(&db).is_none(), "dev-only");

    let notice = verdict(vec![copy("d:/work/site", true, false)], true);
    assert!(
        notice.confirmed_runtime_copy(&db).is_none(),
        "maintenance notice"
    );

    assert!(verdict(Vec::new(), false)
        .confirmed_runtime_copy(&db)
        .is_none());

    {
        let conn = db.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS detected_projects (
                path TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
                last_activity TEXT, scratch INTEGER NOT NULL DEFAULT 0);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO detected_projects (path, name, last_activity, scratch)
             VALUES ('d:/work/scratch', 'scratch', '2026-04-28T19:28:43Z', 1)",
            [],
        )
        .unwrap();
    }
    let scratch_only = verdict(vec![copy("d:/work/scratch", true, false)], false);
    assert!(
        scratch_only.confirmed_runtime_copy(&db).is_none(),
        "an exposure only in a scratch project does not ground"
    );
}
