// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One security truth through the full pipeline (2026-10-02 adversarial
//! audit): the two live advisory items, with their real values, scored end to
//! end — the tier, the verdict and the explanation must agree with
//! Preemption's version-confirmed matcher.

use super::*;
use crate::db::DependencyInstanceInput;

const HONO_TITLE: &str =
    "[CVE-2026-93981] hono/jsx renders plain strings unescaped in boundary components, leading to XSS";
const HONO_URL: &str = "https://github.com/advisories/GHSA-hxh3-vqpv-xpqv";
const HONO_CONTENT: &str = "`hono/jsx` does not HTML-escape a plain string placed directly as a \
child of `Suspense` or `ErrorBoundary`. An attacker who controls a string can exploit this \
cross-site scripting vulnerability to inject arbitrary HTML.\n\n\
Severity: MEDIUM\nAffected: hono (npm)\nAffected range: hono (npm): < 4.13.7\nCVSS: 4.7";

const RMCP_TITLE: &str =
    "[CVE-2026-63128] RMCP: Unauthenticated permanent session-table leak in rmcp \
Streamable HTTP server transport leads to remote denial-of-service";
const RMCP_URL: &str = "https://github.com/advisories/GHSA-9pj6-vhgr-3mwh";
const RMCP_CONTENT: &str = "An unauthenticated remote attacker can exploit this vulnerability in \
rmcp to leak one session per request out of the in-memory session table of \
`LocalSessionManager`, exhausting memory.\n\n\
Severity: HIGH\nAffected: rmcp (rust)\nAffected range: rmcp (rust): < 2.0.0\nCVSS: 7.5";

fn ctx_with_dep(
    package: &str,
    ecosystem: &str,
    version: &str,
    project: &str,
) -> crate::scoring::ScoringContext {
    let mut ace_ctx = ACEContext::default();
    let normalized = dependencies::normalize_package_name(package);
    let info = dependencies::DepInfo {
        package_name: normalized.clone(),
        version: Some(version.to_string()),
        is_dev: false,
        is_direct: true,
        search_terms: dependencies::extract_search_terms(package),
        ecosystem: ecosystem.to_string(),
        project_paths: vec![project.to_string()],
        project_relevance: 1.0,
    };
    for term in &info.search_terms {
        ace_ctx.dependency_names.insert(term.clone());
    }
    ace_ctx.dependency_names.insert(normalized.clone());
    ace_ctx.dependency_info.insert(normalized, info);
    ace_ctx.detected_tech.push(ecosystem.to_string());
    crate::scoring::ScoringContext::builder()
        .ace_ctx(ace_ctx)
        .build()
}

fn signal_options() -> ScoringOptions {
    ScoringOptions {
        apply_freshness: false,
        apply_signals: true,
        trend_topics: vec![],
    }
}

fn input<'a>(
    title: &'a str,
    url: &'a str,
    content: &'a str,
    embedding: &'a [f32],
) -> ScoringInput<'a> {
    ScoringInput {
        id: 136_701,
        title,
        url: Some(url),
        content,
        source_type: "cve",
        embedding,
        created_at: None,
        detected_lang: "",
        source_tags: &[],
        tags_json: None,
        feed_origin: None,
        source_id: None,
    }
}

fn seed_hono_mirror(db: &Database) {
    db.upsert_osv_advisory_with_meta(
        "GHSA-hxh3-vqpv-xpqv",
        "hono/jsx renders plain strings unescaped",
        None,
        "hono",
        "npm",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.13.7"}]}]"#),
        Some(r#"["4.13.7"]"#),
        Some("CVSS_V3"),
        Some(4.7),
        None,
        None,
        None,
        None,
        Some(r#"["CVE-2026-93981"]"#),
        Some("medium"),
    )
    .unwrap();
}

fn assert_never_interrupts(result: &SourceRelevance) {
    assert!(
        !matches!(
            result.signal_priority.as_deref(),
            Some("critical" | "alert")
        ),
        "priority {:?} / action {:?}",
        result.signal_priority,
        result.signal_action
    );
    assert!(!result.is_critical_alert);
    assert!(
        !result
            .signal_action
            .as_deref()
            .is_some_and(|a| a.starts_with("Critical")),
        "action {:?}",
        result.signal_action
    );
}

/// The live timing: the item was scored 12 minutes after publication, BEFORE
/// the OSV mirror held GHSA-hxh3-vqpv-xpqv. The text route must still read
/// the `Affected range:` line and confirm 4.13.8 is past the fix.
#[test]
fn hono_before_the_mirror_syncs_is_not_affected_by_the_text_range() {
    let db = crate::test_utils::test_db();
    let ctx = ctx_with_dep("hono", "javascript", "4.13.8", "d:/4da/mcp-4da-server");
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let classifier = signals::SignalClassifier::new();
    let result = score_item(
        &input(HONO_TITLE, HONO_URL, HONO_CONTENT, &zero),
        &ctx,
        &db,
        &signal_options(),
        Some(&classifier),
    );
    let bd = result.score_breakdown.as_ref().expect("breakdown");
    assert_eq!(bd.affected_versions.as_deref(), Some("< 4.13.7"));
    assert_eq!(bd.is_version_affected, Some(false));
    assert!(!result.relevant, "a fixed advisory leaves Signal");
    assert_never_interrupts(&result);
}

/// With the mirror row and the inventory (4.13.9, the live lockfile): the
/// matcher verdict is final.
#[test]
fn hono_with_the_mirror_is_not_affected_by_the_matcher() {
    let db = crate::test_utils::test_db();
    seed_hono_mirror(&db);
    db.store_dependency(
        "d:/4da/mcp-4da-server",
        "hono",
        Some("4.13.9"),
        "javascript",
        false,
        None,
    )
    .unwrap();
    let ctx = ctx_with_dep("hono", "javascript", "4.13.8", "d:/4da/mcp-4da-server");
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let classifier = signals::SignalClassifier::new();
    let result = score_item(
        &input(HONO_TITLE, HONO_URL, HONO_CONTENT, &zero),
        &ctx,
        &db,
        &signal_options(),
        Some(&classifier),
    );
    let bd = result.score_breakdown.as_ref().expect("breakdown");
    assert_eq!(bd.is_version_affected, Some(false));
    assert!(!result.relevant);
    assert_never_interrupts(&result);
    if result.signal_type.as_deref() == Some("security_alert") {
        assert_eq!(result.signal_priority.as_deref(), Some("watch"));
        assert_eq!(result.applicability.as_deref(), Some("not_affected"));
    }
}

/// A grounded advisory whose applicability cannot be decided (no range, no
/// fix, no mirror row) is Advisory at most — the old ungraded default was
/// Critical, then Alert.
#[test]
fn unverified_grounded_advisory_caps_at_advisory() {
    let db = crate::test_utils::test_db();
    let ctx = ctx_with_dep("hono", "javascript", "4.13.8", "d:/4da/mcp-4da-server");
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let classifier = signals::SignalClassifier::new();
    let content = "`hono/jsx` does not HTML-escape a plain string; an attacker can exploit this \
                   cross-site scripting vulnerability.\n\nSeverity: MEDIUM\nAffected: hono (npm)";
    let result = score_item(
        &input(HONO_TITLE, HONO_URL, content, &zero),
        &ctx,
        &db,
        &signal_options(),
        Some(&classifier),
    );
    let bd = result.score_breakdown.as_ref().expect("breakdown");
    assert_eq!(bd.is_version_affected, None);
    assert_never_interrupts(&result);
}

/// Live 2026-10-02: rmcp is declared (direct, 3.1.2 — fixed) by victauri, and
/// installed TRANSITIVELY at 1.7.0 in a sibling bridge app. Preemption lists the
/// HIGH; the Signal lane must agree: affected, relevant, Alert (HIGH, one
/// tier down for transitive reach), naming the exposed project and version.
#[test]
fn rmcp_transitive_exposure_surfaces_as_an_alert() {
    let db = crate::test_utils::test_db();
    db.store_dependency(
        "d:/runyourempire/victauri",
        "rmcp",
        Some("3.1.2"),
        "rust",
        false,
        None,
    )
    .unwrap();
    db.store_transitive_dependency(
        "d:/work/agent-bridge/src-tauri",
        "rmcp",
        Some("1.7.0"),
        "rust",
        false,
    )
    .unwrap();
    db.store_dependency_instances(
        "d:/work/agent-bridge/src-tauri",
        "crates.io",
        &[DependencyInstanceInput {
            package_name: "rmcp".into(),
            version: "1.7.0".into(),
            is_direct: false,
            is_dev: false,
            scope: "unknown".into(),
        }],
    )
    .unwrap();
    db.upsert_osv_advisory_with_meta(
        "GHSA-9pj6-vhgr-3mwh",
        "RMCP: Unauthenticated permanent session-table leak",
        None,
        "rmcp",
        "crates.io",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#),
        Some(r#"["2.0.0"]"#),
        Some("CVSS_V3"),
        Some(7.5),
        None,
        None,
        None,
        None,
        Some(r#"["CVE-2026-63128"]"#),
        Some("high"),
    )
    .unwrap();
    let ctx = ctx_with_dep("rmcp", "rust", "3.1.2", "d:/runyourempire/victauri");
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let classifier = signals::SignalClassifier::new();
    let result = score_item(
        &input(RMCP_TITLE, RMCP_URL, RMCP_CONTENT, &zero),
        &ctx,
        &db,
        &signal_options(),
        Some(&classifier),
    );
    let bd = result.score_breakdown.as_ref().expect("breakdown");
    assert_eq!(
        bd.is_version_affected,
        Some(true),
        "a sibling bridge app runs 1.7.0 < 2.0.0"
    );
    assert_eq!(bd.installed_version.as_deref(), Some("1.7.0"));
    assert_eq!(bd.dependency_path.as_deref(), Some("transitive"));
    assert_eq!(bd.affected_project_count, Some(1));
    assert_eq!(bd.score_ceiling, None, "no not-affected cap");
    assert!(
        bd.strongly_grounded,
        "a matcher-confirmed exposure grounds the advisory"
    );
    assert!(
        result.relevant && result.top_score >= get_relevance_threshold(),
        "a confirmed HIGH must surface (score {})",
        result.top_score
    );
    assert_eq!(result.signal_type.as_deref(), Some("security_alert"));
    assert_eq!(result.signal_priority.as_deref(), Some("alert"));
    assert_eq!(result.applicability.as_deref(), Some("affected"));
    assert!(
        !result.is_critical_alert,
        "transitive HIGH is an Alert, not a Critical page"
    );
    let action = result.signal_action.as_deref().unwrap_or_default();
    assert!(
        action.starts_with("Security:")
            && action.contains("rmcp")
            && action.contains("bridge/src-tauri")
            && !action.contains("victauri"),
        "action names the exposed project: {action}"
    );
    let security_factor = bd
        .explanation_factors
        .iter()
        .find(|f| f.kind == crate::types::FactorKind::SecurityAdvisory)
        .expect("security factor");
    assert!(
        security_factor.evidence.contains("installed v1.7.0"),
        "evidence cites the exposed copy: {}",
        security_factor.evidence
    );
    assert!(
        !security_factor.display.contains("victauri"),
        "the declaring project runs the fix: {}",
        security_factor.display
    );
}

/// An editorial security story (no registry row) never interrupts, however
/// it is grounded — the arXiv dataset paper and the OpenAI news items were
/// security_alert ALERTs live.
#[test]
fn editorial_security_story_never_interrupts() {
    let db = crate::test_utils::test_db();
    let ctx = ctx_with_dep("hono", "javascript", "4.13.8", "d:/4da/mcp-4da-server");
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let classifier = signals::SignalClassifier::new();
    let mut story = input(
        "Critical hono vulnerability: XSS exploit in hono/jsx, patch now",
        "https://example.com/hono-xss",
        "A critical cross-site scripting vulnerability in hono lets an attacker exploit SSR. \
         Patch released; upgrade hono urgently.",
        &zero,
    );
    story.source_type = "hackernews";
    let result = score_item(&story, &ctx, &db, &signal_options(), Some(&classifier));
    assert_never_interrupts(&result);
}
