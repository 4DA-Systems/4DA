use super::*;
use crate::db::DependencyInstanceInput;
use crate::signals::SignalPriority;
use crate::test_utils::test_db;

/// The metadata tail of live item 136701 (cve source, 2026-09-30).
const HONO_TAIL: &str = "This issue affects applications that render untrusted strings.\n\n\
Severity: MEDIUM\nAffected: hono (npm)\nAffected range: hono (npm): < 4.13.7\nCVSS: 4.7";
/// The metadata tail of live item 102612.
const RMCP_TAIL: &str = "A regression test would catch this.\n\n\
Severity: HIGH\nAffected: rmcp (rust)\nAffected range: rmcp (rust): < 2.0.0\nCVSS: 7.5";

#[test]
fn affected_range_is_read_from_the_range_line_not_the_name_line() {
    assert_eq!(
        affected_ranges_for_package(HONO_TAIL, "hono"),
        vec!["< 4.13.7"]
    );
    assert_eq!(
        affected_ranges_for_package(RMCP_TAIL, "rmcp"),
        vec!["< 2.0.0"]
    );
    assert!(affected_ranges_for_package(HONO_TAIL, "react").is_empty());
    let multi =
        "Affected range: @swc/html (npm): < 1.15.47; swc_html_minifier (rust): >= 1.0.0, < 59.0.0";
    assert_eq!(
        affected_ranges_for_package(multi, "swc-html-minifier"),
        vec![">= 1.0.0, < 59.0.0"]
    );
    assert_eq!(
        affected_ranges_for_package(multi, "@swc/html"),
        vec!["< 1.15.47"]
    );
}

#[test]
fn range_verdict_reads_every_release_line() {
    let hono = affected_ranges_for_package(HONO_TAIL, "hono");
    assert_eq!(ranges_verdict(Some("4.13.8"), &hono), Some(false));
    assert_eq!(ranges_verdict(Some("4.13.9"), &hono), Some(false));
    assert_eq!(ranges_verdict(Some("4.13.6"), &hono), Some(true));
    // One entry per branch: 2.0.5 sits in the second window.
    let branches = "Affected range: x (npm): < 1.9.8; x (npm): >= 2.0.0, < 2.1.4";
    let ranges = affected_ranges_for_package(branches, "x");
    assert_eq!(ranges_verdict(Some("2.0.5"), &ranges), Some(true));
    assert_eq!(ranges_verdict(Some("1.9.9"), &ranges), Some(false));
    assert_eq!(ranges_verdict(Some("2.1.4"), &ranges), Some(false));
    assert_eq!(ranges_verdict(None, &ranges), None);
    assert_eq!(
        ranges_verdict(Some("2.0.5"), &["not a range".to_string()]),
        None
    );
}

#[test]
fn severity_comes_from_the_advisory_text() {
    assert_eq!(content_severity_tier(HONO_TAIL), Some("medium"));
    assert_eq!(content_severity_tier(RMCP_TAIL), Some("high"));
    assert_eq!(
        content_severity_tier("Severity: MODERATE\n"),
        Some("medium")
    );
    assert_eq!(
        content_severity_tier("Severity: CVSS_V3: 9.8\n"),
        Some("critical")
    );
    assert_eq!(content_severity_tier("no metadata"), None);
}

fn seed_rmcp(db: &Database) {
    db.store_dependency(
        "d:/runyourempire/victauri",
        "rmcp",
        Some("3.1.2"),
        "rust",
        false,
        None,
    )
    .unwrap();
    db.store_dependency(
        "d:/runyourempire/victauri/crates/victauri-plugin",
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
}

/// Live 2026-10-02: the CVE item names `CVE-2026-63128`; the mirror row is
/// keyed by its GHSA id with the CVE as an alias. The exposed copy is the
/// transitive one in a sibling bridge app — not the declaring victauri projects.
#[test]
fn rmcp_verdict_finds_the_transitive_exposure() {
    let db = test_db();
    seed_rmcp(&db);
    let v = matcher_verdict(&db, &["CVE-2026-63128".to_string()]).expect("verdict");
    assert_eq!(v.affected, Some(true));
    assert_eq!(v.tier, Some("high"));
    assert_eq!(v.fixed_version.as_deref(), Some("2.0.0"));
    assert_eq!(
        v.exposed_projects(),
        vec!["d:/work/agent-bridge/src-tauri".to_string()]
    );
    assert_eq!(v.path_label(), Some("transitive"));
    assert_eq!(v.worst_copy().unwrap().installed_version, "1.7.0");

    let lane = SecurityLane {
        registry_advisory: true,
        affected: v.affected,
        exposed: v.worst_copy(),
        exposed_projects: v.exposed_projects(),
        tier: v.tier,
        package: Some("rmcp"),
    };
    // HIGH, transitive-only -> Alert (not Watch, not Critical).
    assert_eq!(lane.priority(None), SignalPriority::Alert);
    assert_eq!(lane.cap(), SignalPriority::Critical);
}

/// Live 2026-10-02: hono 4.13.9 is past the 4.13.7 fix.
#[test]
fn hono_verdict_is_not_affected() {
    let db = test_db();
    db.store_dependency(
        "d:/4da/mcp-4da-server",
        "hono",
        Some("4.13.9"),
        "javascript",
        false,
        None,
    )
    .unwrap();
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
    let v = matcher_verdict(&db, &["CVE-2026-93981".to_string()]).expect("verdict");
    assert_eq!(v.affected, Some(false));
    assert!(v.exposed.is_empty());
    assert_eq!(v.tier, Some("medium"));
    let lane = SecurityLane {
        registry_advisory: true,
        affected: v.affected,
        tier: v.tier,
        ..SecurityLane::default()
    };
    assert_eq!(lane.priority(None), SignalPriority::Watch);
    assert_eq!(lane.cap(), SignalPriority::Watch);
}

#[test]
fn mirror_without_the_advisory_gives_no_verdict() {
    let db = test_db();
    seed_rmcp(&db);
    assert!(matcher_verdict(&db, &["CVE-2099-0001".to_string()]).is_none());
}

#[test]
fn unknown_applicability_and_editorial_security_cap_at_advisory() {
    let unknown = SecurityLane {
        registry_advisory: true,
        tier: Some("critical"),
        ..SecurityLane::default()
    };
    assert_eq!(unknown.priority(None), SignalPriority::Advisory);
    assert_eq!(unknown.cap(), SignalPriority::Advisory);
    let editorial = SecurityLane {
        registry_advisory: false,
        affected: Some(true),
        tier: Some("critical"),
        ..SecurityLane::default()
    };
    assert_eq!(editorial.priority(None), SignalPriority::Advisory);
    assert_eq!(editorial.cap(), SignalPriority::Advisory);
}

#[test]
fn priority_labels_round_trip() {
    for p in [
        SignalPriority::Watch,
        SignalPriority::Advisory,
        SignalPriority::Alert,
        SignalPriority::Critical,
    ] {
        assert_eq!(priority_from_label(p.label()), Some(p));
    }
    assert_eq!(priority_from_label("bogus"), None);
}

// ── advisory_signal_type ────────────────────────────────────────────────

fn trend_classification() -> crate::signals::SignalClassification {
    crate::signals::SignalClassification {
        signal_type: crate::signals::SignalType::TechTrend,
        priority: crate::signals::SignalPriority::Advisory,
        confidence: 0.6,
        action: "Emerging trend".to_string(),
        triggers: vec!["oauth".to_string(), "client".to_string()],
        horizon: crate::signals::SignalHorizon::Strategic,
        dependency_confirmed: false,
        corroboration_sources: 0,
    }
}

#[test]
fn registry_advisory_typed_by_other_words_becomes_security() {
    // Live 2026-10-03: GHSA-c9xm-49cp-xcr9 read as an "Emerging trend".
    let c = advisory_signal_type(
        true,
        "[GHSA-c9xm-49cp-xcr9] rmcp OAuth client fetches server-controlled resource_metadata URLs",
        Some(trend_classification()),
    )
    .expect("an advisory row always classifies");
    assert_eq!(c.signal_type, crate::signals::SignalType::SecurityAlert);
    assert_eq!(c.horizon, crate::signals::SignalHorizon::Tactical);
    assert_eq!(c.triggers, vec!["oauth".to_string(), "client".to_string()]);
}

#[test]
fn registry_advisory_the_classifier_skipped_still_classifies() {
    // Live 2026-10-03: RUSTSEC-2026-0190 (anyhow) carried no signal at all.
    let title = "[RUSTSEC-2026-0190] anyhow: Unsoundness in `Error::downcast_mut()`";
    let c = advisory_signal_type(true, title, None).expect("advisory row classifies");
    assert_eq!(c.signal_type, crate::signals::SignalType::SecurityAlert);
    assert_eq!(c.priority, crate::signals::SignalPriority::Advisory);
    assert!(c.action.contains("RUSTSEC-2026-0190"));
}

#[test]
fn editorial_items_keep_the_classifier_verdict() {
    let c = advisory_signal_type(false, "Some HN story", Some(trend_classification()));
    assert_eq!(
        c.map(|c| c.signal_type),
        Some(crate::signals::SignalType::TechTrend)
    );
    assert!(advisory_signal_type(false, "Some HN story", None).is_none());
}
