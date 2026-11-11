// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;

#[test]
fn test_parse_osv_max_age() {
    // Unset / blank / non-numeric -> conservative 6h default (desktop unchanged).
    assert_eq!(parse_osv_max_age(None), DEFAULT_SYNC_MAX_AGE_HOURS);
    assert_eq!(
        parse_osv_max_age(Some("".into())),
        DEFAULT_SYNC_MAX_AGE_HOURS
    );
    assert_eq!(
        parse_osv_max_age(Some("soon".into())),
        DEFAULT_SYNC_MAX_AGE_HOURS
    );
    // Explicit value (the ledger sets "1") is honored.
    assert_eq!(parse_osv_max_age(Some("1".into())), 1);
    assert_eq!(parse_osv_max_age(Some(" 2 ".into())), 2);
    assert_eq!(parse_osv_max_age(Some("24".into())), 24);
    // A misconfigured 0/negative is clamped to a 1h floor (never sync-every-cycle).
    assert_eq!(parse_osv_max_age(Some("0".into())), 1);
    assert_eq!(parse_osv_max_age(Some("-5".into())), 1);
}

#[test]
fn test_normalize_to_osv() {
    assert_eq!(normalize_to_osv("rust"), "crates.io");
    assert_eq!(normalize_to_osv("Rust"), "crates.io");
    assert_eq!(normalize_to_osv("javascript"), "npm");
    assert_eq!(normalize_to_osv("typescript"), "npm");
    assert_eq!(normalize_to_osv("npm"), "npm");
    assert_eq!(normalize_to_osv("python"), "PyPI");
    assert_eq!(normalize_to_osv("pypi"), "PyPI");
    assert_eq!(normalize_to_osv("go"), "Go");
    assert_eq!(normalize_to_osv("golang"), "Go");
    assert_eq!(normalize_to_osv("unknown"), "unknown");
    // Language-name keys must resolve, not just registry aliases — the ACE scanner tags
    // scanned deps with the language ("csharp"/"php"/"dart"), so without these a whole
    // ecosystem's advisories were silently skipped (regression guard for the langmap gap).
    assert_eq!(normalize_to_osv("java"), "Maven");
    assert_eq!(normalize_to_osv("kotlin"), "Maven");
    assert_eq!(normalize_to_osv("ruby"), "RubyGems");
    assert_eq!(normalize_to_osv("csharp"), "NuGet");
    assert_eq!(normalize_to_osv("C#"), "NuGet"); // case-insensitive
    assert_eq!(normalize_to_osv("dotnet"), "NuGet");
    assert_eq!(normalize_to_osv("nuget"), "NuGet");
    assert_eq!(normalize_to_osv("php"), "Packagist");
    assert_eq!(normalize_to_osv("packagist"), "Packagist");
    assert_eq!(normalize_to_osv("dart"), "Pub");
    assert_eq!(normalize_to_osv("flutter"), "Pub");
    assert_eq!(normalize_to_osv("pub"), "Pub");
}

#[test]
fn test_extract_fixed_versions() {
    use super::super::types::{Affected, Range};

    let affected = Affected {
        package: None,
        ranges: Some(vec![Range {
            range_type: "SEMVER".to_string(),
            events: Some(vec![
                serde_json::json!({"introduced": "0"}),
                serde_json::json!({"fixed": "1.2.3"}),
            ]),
        }]),
        versions: None,
        database_specific: None,
    };

    let fixed = extract_fixed_versions(&affected).unwrap();
    assert!(fixed.contains("1.2.3"));
}

#[test]
fn test_extract_fixed_versions_multiple() {
    use super::super::types::{Affected, Range};

    let affected = Affected {
        package: None,
        ranges: Some(vec![Range {
            range_type: "SEMVER".to_string(),
            events: Some(vec![
                serde_json::json!({"introduced": "1.0.0"}),
                serde_json::json!({"fixed": "1.0.5"}),
                serde_json::json!({"introduced": "2.0.0"}),
                serde_json::json!({"fixed": "2.1.0"}),
            ]),
        }]),
        versions: None,
        database_specific: None,
    };

    let fixed = extract_fixed_versions(&affected).unwrap();
    let versions: Vec<String> = serde_json::from_str(&fixed).unwrap();
    assert_eq!(versions, vec!["1.0.5", "2.1.0"]);
}

#[test]
fn test_extract_fixed_versions_none() {
    use super::super::types::{Affected, Range};

    let affected = Affected {
        package: None,
        ranges: Some(vec![Range {
            range_type: "SEMVER".to_string(),
            events: Some(vec![serde_json::json!({"introduced": "0"})]),
        }]),
        versions: None,
        database_specific: None,
    };

    assert!(extract_fixed_versions(&affected).is_none());
}

#[test]
fn test_store_vulnerability_dedup() {
    use crate::test_utils::test_db;

    let db = test_db();
    let mut seen = std::collections::HashSet::new();

    let vuln = Vulnerability {
        id: "GHSA-test-001".to_string(),
        summary: Some("Test vuln".to_string()),
        details: None,
        severity: Some(vec![super::super::types::Severity {
            severity_type: "CVSS_V3".to_string(),
            score: "7.5".to_string(),
        }]),
        affected: Some(vec![super::super::types::Affected {
            package: Some(PackageRef {
                name: "react".to_string(),
                ecosystem: "npm".to_string(),
            }),
            ranges: None,
            versions: None,
            database_specific: None,
        }]),
        references: None,
        published: None,
        modified: None,
        withdrawn: None,
        aliases: None,
        database_specific: None,
    };

    let stored1 = store_vulnerability(&db, &vuln, &mut seen).unwrap();
    assert_eq!(stored1, 1);

    let stored2 = store_vulnerability(&db, &vuln, &mut seen).unwrap();
    assert_eq!(stored2, 0, "Duplicate should be skipped");
}

/// A `versions`-only advisory (GHSA-q58r-hwc8-rm9j's shape) keeps its list,
/// so the matcher can confirm the listed versions.
#[test]
fn a_versions_only_advisory_is_stored_matchable() {
    let db = crate::test_utils::test_db();
    let vuln: Vulnerability = serde_json::from_value(serde_json::json!({
        "id": "GHSA-q58r-hwc8-rm9j",
        "affected": [{
            "package": {"name": "bootstrap", "ecosystem": "npm"},
            "versions": ["3.4.0", "3.4.1"]
        }]
    }))
    .unwrap();
    store_vulnerability(&db, &vuln, &mut std::collections::HashSet::new()).unwrap();
    let stored = db.get_all_osv_advisories().unwrap();
    let ranges = &stored[0].affected_ranges;
    assert_eq!(
        super::super::matching::check_version_affected("npm", Some("3.4.1"), ranges),
        (true, true)
    );
}

#[test]
fn test_needs_sync_requires_every_active_ecosystem_to_be_fresh() {
    use crate::test_utils::test_db;

    let db = test_db();
    assert!(!needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).unwrap());

    db.store_dependency(
        "/project",
        "internal-package",
        Some("1.0.0"),
        "custom",
        false,
        None,
    )
    .unwrap();
    assert!(
        !needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).unwrap(),
        "unsupported ecosystems must not keep OSV freshness permanently stale"
    );

    db.store_dependency("/project", "react", Some("19.0.0"), "npm", false, None)
        .unwrap();
    db.store_dependency("/project", "tokio", Some("1.0.0"), "rust", false, None)
        .unwrap();
    assert!(needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).unwrap());

    db.update_osv_sync_status("npm", 0, None).unwrap();
    assert!(
        needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).unwrap(),
        "fresh npm status must not mask missing crates.io status"
    );

    db.update_osv_sync_status("crates.io", 0, None).unwrap();
    assert!(!needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).unwrap());

    db.update_osv_sync_status("npm", 0, Some("network failed"))
        .unwrap();
    assert!(
        needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).unwrap(),
        "a recorded ecosystem failure must remain stale"
    );

    db.update_osv_sync_status("npm", 0, None).unwrap();
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE osv_sync_status SET last_synced_at = datetime('now', '-7 hours') WHERE ecosystem = 'npm'",
            [],
        )
        .unwrap();
    }
    assert!(
        needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).unwrap(),
        "an old ecosystem status must make the dependency mirror stale"
    );
}

/// Phase 128 backfill: a row stored before `cwe_ids` existed is re-synced
/// with it. The migration makes every ecosystem due (a fresh status would
/// otherwise skip the sync for up to 6 h), and the next sync's upsert of
/// the same record writes its CWE ids, so crossenv GHSA-c2m4-w5hm-vqjw
/// (CWE-506, no `MAL-` alias) reads as malware.
#[test]
fn a_pre_migration_row_gets_its_cwe_ids_on_the_next_sync() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/project", "crossenv", Some("1.0.0"), "npm", false, None)
        .unwrap();
    let record = serde_json::json!({
        "id": "GHSA-c2m4-w5hm-vqjw",
        "summary": "crossenv is malware",
        "aliases": ["CVE-2017-16074"],
        "affected": [{"package": {"ecosystem": "npm", "name": "crossenv"},
                      "ranges": [{"type": "SEMVER", "events": [{"introduced": "0"}]}]}],
        "database_specific": {"cwe_ids": ["CWE-506"], "severity": "CRITICAL"}
    });
    let vuln: Vulnerability = serde_json::from_value(record).unwrap();
    // As stored before Phase 128: the row exists, its CWE ids do not.
    let mut seen = std::collections::HashSet::new();
    store_vulnerability(&db, &vuln, &mut seen).unwrap();
    db.conn
        .lock()
        .execute("UPDATE osv_advisories SET cwe_ids = NULL", [])
        .unwrap();
    db.update_osv_sync_status("npm", 1, None).unwrap();
    let stored = || {
        db.get_osv_advisories_for_package("crossenv", "npm")
            .unwrap()
    };
    assert!(!stored()[0].is_malware());
    assert!(!needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).unwrap());

    // The migration's step makes the sync due ...
    {
        let conn = db.conn.lock();
        crate::db::Database::make_osv_sync_due(&conn).unwrap();
    }
    assert!(needs_sync(&db, DEFAULT_SYNC_MAX_AGE_HOURS).unwrap());
    // ... and the sync's re-upsert fills the column.
    let mut seen = std::collections::HashSet::new();
    store_vulnerability(&db, &vuln, &mut seen).unwrap();
    assert_eq!(stored()[0].cwe_ids, ["CWE-506"]);
    assert!(stored()[0].is_malware());
}
