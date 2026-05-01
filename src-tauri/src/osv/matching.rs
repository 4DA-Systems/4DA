// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV matching — cross-references stored advisories against user dependencies.
//!
//! Joins osv_advisories with user_dependencies and runs semver version checks
//! to produce verified MatchedAdvisory results (Tier 1 intelligence).

use std::collections::HashMap;

use crate::db::Database;
use crate::error::{FourDaError, Result};
use semver::Version;

use super::types::{MatchedAdvisory, Range};

/// Get all advisories that match the user's installed dependencies.
/// Merges deps from both `user_dependencies` (user-curated) and
/// `project_dependencies` (ACE-scanned) to ensure coverage.
/// Version matching is attempted for SEMVER ranges; conservative (assume affected)
/// fallback for non-semver or unparseable versions.
pub fn get_matched_advisories(db: &Database) -> Result<Vec<MatchedAdvisory>> {
    let advisories = db
        .get_all_osv_advisories()
        .map_err(|e| FourDaError::Internal(format!("Failed to read OSV advisories: {e}")))?;

    let mut deps = db
        .get_all_user_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read user dependencies: {e}")))?;

    let scanned = db
        .get_all_scanned_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read scanned dependencies: {e}")))?;

    // Merge scanned deps, deduped by (package_name, project_path, ecosystem)
    let mut seen_deps: std::collections::HashSet<(String, String, String)> =
        deps.iter()
            .map(|d| (d.package_name.to_lowercase(), d.project_path.clone(), normalize_ecosystem(&d.ecosystem).to_string()))
            .collect();

    for dep in scanned {
        let key = (dep.package_name.to_lowercase(), dep.project_path.clone(), normalize_ecosystem(&dep.ecosystem).to_string());
        if seen_deps.insert(key) {
            deps.push(dep);
        }
    }

    if advisories.is_empty() || deps.is_empty() {
        return Ok(Vec::new());
    }

    // Index deps by (package_name_lower, ecosystem_normalized) for fast lookup
    let mut dep_index: HashMap<(String, String), Vec<(Option<String>, String)>> = HashMap::new();
    for dep in &deps {
        let key = (
            dep.package_name.to_lowercase(),
            normalize_ecosystem(&dep.ecosystem).to_string(),
        );
        dep_index
            .entry(key)
            .or_default()
            .push((dep.version.clone(), dep.project_path.clone()));
    }

    let mut matches: Vec<MatchedAdvisory> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for advisory in &advisories {
        let key = (
            advisory.package_name.to_lowercase(),
            normalize_ecosystem(&advisory.ecosystem).to_string(),
        );

        let dep_entries = match dep_index.get(&key) {
            Some(entries) => entries,
            None => continue,
        };

        // Check each dependency instance (could be in multiple projects)
        let mut project_paths = Vec::new();
        let mut any_version_confirmed = false;
        let mut representative_version: Option<String> = None;

        for (dep_version, project_path) in dep_entries {
            let (is_affected, confirmed) =
                check_version_affected(dep_version.as_deref(), &advisory.affected_ranges);

            if is_affected {
                project_paths.push(project_path.clone());
                if confirmed {
                    any_version_confirmed = true;
                }
                if representative_version.is_none() {
                    representative_version = dep_version.clone();
                }
            }
        }

        if project_paths.is_empty() {
            continue;
        }

        project_paths.sort();
        project_paths.dedup();

        let dedup_key = format!("{}:{}", advisory.advisory_id, advisory.package_name);
        if !seen.insert(dedup_key) {
            continue;
        }

        let fixed_version = advisory
            .fixed_versions
            .as_ref()
            .and_then(|fv| serde_json::from_str::<Vec<String>>(fv).ok())
            .and_then(|versions| versions.into_iter().next());

        matches.push(MatchedAdvisory {
            advisory_id: advisory.advisory_id.clone(),
            summary: advisory.summary.clone(),
            details: advisory.details.clone(),
            package_name: advisory.package_name.clone(),
            ecosystem: advisory.ecosystem.clone(),
            installed_version: representative_version,
            fixed_version,
            severity_type: advisory.severity_type.clone(),
            cvss_score: advisory.cvss_score,
            source_url: advisory.source_url.clone(),
            is_version_confirmed: any_version_confirmed,
            project_paths,
            published_at: advisory.published_at.clone(),
        });
    }

    // Sort: confirmed first, then by CVSS score descending
    matches.sort_by(|a, b| {
        b.is_version_confirmed
            .cmp(&a.is_version_confirmed)
            .then_with(|| {
                b.cvss_score
                    .unwrap_or(0.0)
                    .partial_cmp(&a.cvss_score.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    Ok(matches)
}

/// Count matched advisories without building the full result.
pub fn count_matches(db: &Database) -> Result<usize> {
    get_matched_advisories(db).map(|m| m.len())
}

/// Check if a user's version is affected by the advisory's ranges.
/// Returns (is_affected, is_confirmed).
/// - is_affected: true if the version falls within affected ranges (or conservative fallback)
/// - is_confirmed: true only if we could definitively verify via semver
fn check_version_affected(
    user_version: Option<&str>,
    affected_ranges_json: &Option<String>,
) -> (bool, bool) {
    let ranges_json = match affected_ranges_json {
        Some(json) if !json.is_empty() => json,
        _ => return (true, false), // No range info → conservative match
    };

    let ranges: Vec<Range> = match serde_json::from_str(ranges_json) {
        Ok(r) => r,
        Err(_) => return (true, false), // Can't parse → conservative
    };

    let user_ver_str = match user_version {
        Some(v) if !v.is_empty() => v,
        _ => return (true, false), // No version → conservative
    };

    let parsed_user = match parse_version(user_ver_str) {
        Some(v) => v,
        None => return (true, false), // Can't parse user version → conservative
    };

    for range in &ranges {
        if range.range_type != "SEMVER" && range.range_type != "ECOSYSTEM" {
            continue;
        }

        let events = match &range.events {
            Some(e) if !e.is_empty() => e,
            _ => continue,
        };

        // Process events as (introduced, fixed/last_affected) pairs
        let mut introduced: Option<Version> = None;

        for event in events {
            let obj = match event.as_object() {
                Some(o) => o,
                None => continue,
            };

            if let Some(intro_str) = obj.get("introduced").and_then(|v| v.as_str()) {
                introduced = if intro_str == "0" {
                    Some(Version::new(0, 0, 0))
                } else {
                    parse_version(intro_str)
                };
            }

            if let Some(fixed_str) = obj.get("fixed").and_then(|v| v.as_str()) {
                if let Some(ref intro_ver) = introduced {
                    if let Some(fix_ver) = parse_version(fixed_str) {
                        if parsed_user >= *intro_ver && parsed_user < fix_ver {
                            return (true, true);
                        }
                    }
                }
                introduced = None;
            }

            if let Some(la_str) = obj.get("last_affected").and_then(|v| v.as_str()) {
                if let Some(ref intro_ver) = introduced {
                    if let Some(la_ver) = parse_version(la_str) {
                        if parsed_user >= *intro_ver && parsed_user <= la_ver {
                            return (true, true);
                        }
                    }
                }
                introduced = None;
            }
        }

        // introduced with no fixed → all versions from introduced onward
        if let Some(ref intro_ver) = introduced {
            if parsed_user >= *intro_ver {
                return (true, true);
            }
        }
    }

    // Went through all ranges, version not in any affected window
    (false, true)
}

/// Parse a version string, handling common non-semver formats.
fn parse_version(ver: &str) -> Option<Version> {
    let v = ver.trim().trim_start_matches('v');
    if v.is_empty() {
        return None;
    }

    if let Ok(version) = Version::parse(v) {
        return Some(version);
    }

    // Two-part version: "1.2" → "1.2.0"
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() == 2 {
        if let Ok(version) = Version::parse(&format!("{v}.0")) {
            return Some(version);
        }
    }

    None
}

/// Normalize ecosystem names to canonical forms for matching.
fn normalize_ecosystem(eco: &str) -> &str {
    match eco.to_lowercase().as_str() {
        "javascript" | "typescript" | "npm" => "npm",
        "rust" | "crates.io" => "crates.io",
        "python" | "pip" | "pypi" => "PyPI",
        "go" | "golang" => "Go",
        "ruby" | "rubygems" => "RubyGems",
        "java" | "maven" => "Maven",
        "nuget" => "NuGet",
        "packagist" => "Packagist",
        _ => eco,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_in_simple_range() {
        let ranges = Some(
            r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.2.3"}]}]"#.to_string(),
        );

        let (affected, confirmed) = check_version_affected(Some("1.2.2"), &ranges);
        assert!(affected, "1.2.2 < 1.2.3 should be affected");
        assert!(confirmed);

        let (affected, confirmed) = check_version_affected(Some("1.2.3"), &ranges);
        assert!(!affected, "1.2.3 == fixed, should NOT be affected");
        assert!(confirmed);

        let (affected, confirmed) = check_version_affected(Some("2.0.0"), &ranges);
        assert!(!affected, "2.0.0 > 1.2.3 should NOT be affected");
        assert!(confirmed);
    }

    #[test]
    fn test_version_in_compound_range() {
        let ranges = Some(
            r#"[{"type":"SEMVER","events":[
                {"introduced":"1.0.0"},{"fixed":"1.0.5"},
                {"introduced":"2.0.0"},{"fixed":"2.1.0"}
            ]}]"#
                .to_string(),
        );

        // In first range
        let (affected, _) = check_version_affected(Some("1.0.3"), &ranges);
        assert!(affected);

        // Between ranges (not affected)
        let (affected, confirmed) = check_version_affected(Some("1.5.0"), &ranges);
        assert!(!affected);
        assert!(confirmed);

        // In second range
        let (affected, _) = check_version_affected(Some("2.0.5"), &ranges);
        assert!(affected);

        // After all ranges
        let (affected, _) = check_version_affected(Some("2.1.0"), &ranges);
        assert!(!affected);
    }

    #[test]
    fn test_no_version_conservative() {
        let ranges = Some(
            r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.0"}]}]"#.to_string(),
        );

        let (affected, confirmed) = check_version_affected(None, &ranges);
        assert!(affected, "No version → conservative match");
        assert!(!confirmed, "No version → not confirmed");
    }

    #[test]
    fn test_no_ranges_conservative() {
        let (affected, confirmed) = check_version_affected(Some("1.0.0"), &None);
        assert!(affected, "No ranges → conservative match");
        assert!(!confirmed);
    }

    #[test]
    fn test_introduced_no_fixed() {
        let ranges = Some(r#"[{"type":"SEMVER","events":[{"introduced":"2.0.0"}]}]"#.to_string());

        let (affected, confirmed) = check_version_affected(Some("2.5.0"), &ranges);
        assert!(affected, "After introduced with no fix → affected");
        assert!(confirmed);

        let (affected, confirmed) = check_version_affected(Some("1.9.0"), &ranges);
        assert!(!affected, "Before introduced → not affected");
        assert!(confirmed);
    }

    #[test]
    fn test_last_affected() {
        let ranges = Some(
            r#"[{"type":"SEMVER","events":[{"introduced":"1.0.0"},{"last_affected":"1.5.0"}]}]"#
                .to_string(),
        );

        let (affected, _) = check_version_affected(Some("1.3.0"), &ranges);
        assert!(affected, "1.3.0 <= 1.5.0 (last_affected)");

        let (affected, _) = check_version_affected(Some("1.5.0"), &ranges);
        assert!(affected, "1.5.0 == last_affected → still affected");

        let (affected, _) = check_version_affected(Some("1.5.1"), &ranges);
        assert!(!affected, "1.5.1 > last_affected → not affected");
    }

    #[test]
    fn test_v_prefix_handled() {
        let ranges = Some(
            r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#.to_string(),
        );

        let (affected, confirmed) = check_version_affected(Some("v1.5.0"), &ranges);
        assert!(affected);
        assert!(confirmed);
    }

    #[test]
    fn test_two_part_version() {
        let ranges = Some(
            r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#.to_string(),
        );

        let (affected, confirmed) = check_version_affected(Some("1.5"), &ranges);
        assert!(affected, "1.5 → 1.5.0 < 2.0.0");
        assert!(confirmed);
    }

    #[test]
    fn test_unparseable_version_conservative() {
        let ranges = Some(
            r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.0"}]}]"#.to_string(),
        );

        let (affected, confirmed) = check_version_affected(Some("banana"), &ranges);
        assert!(affected, "Unparseable → conservative");
        assert!(!confirmed);
    }

    #[test]
    fn test_non_semver_range_type_skipped() {
        let ranges = Some(
            r#"[{"type":"GIT","events":[{"introduced":"abc123"},{"fixed":"def456"}]}]"#.to_string(),
        );

        // GIT ranges are skipped, no SEMVER ranges found → conservative false
        // (because we went through all ranges and found none applicable)
        let (affected, confirmed) = check_version_affected(Some("1.0.0"), &ranges);
        // No SEMVER range matched → not affected (we only skip non-SEMVER ranges)
        assert!(!affected);
        assert!(confirmed);
    }

    #[test]
    fn test_normalize_ecosystem() {
        assert_eq!(normalize_ecosystem("rust"), "crates.io");
        assert_eq!(normalize_ecosystem("javascript"), "npm");
        assert_eq!(normalize_ecosystem("python"), "PyPI");
        assert_eq!(normalize_ecosystem("pip"), "PyPI");
        assert_eq!(normalize_ecosystem("go"), "Go");
        assert_eq!(normalize_ecosystem("golang"), "Go");
        assert_eq!(normalize_ecosystem("unknown"), "unknown");
    }

    #[test]
    fn test_parse_version_formats() {
        assert!(parse_version("1.2.3").is_some());
        assert!(parse_version("v1.2.3").is_some());
        assert!(parse_version("1.2").is_some());
        assert!(parse_version("0.0.0").is_some());
        assert!(parse_version("banana").is_none());
        assert!(parse_version("").is_none());
    }

    #[test]
    fn test_matched_advisories_integration() {
        use crate::test_utils::test_db;

        let db = test_db();

        // Store a dependency
        db.store_dependency("/project/a", "lodash", Some("4.17.20"), "npm", false, None)
            .unwrap();

        // Store an advisory that affects lodash < 4.17.21
        db.upsert_osv_advisory(
            "GHSA-test-001",
            "Prototype pollution in lodash",
            Some("Details here"),
            "lodash",
            "npm",
            Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]"#),
            Some(r#"["4.17.21"]"#),
            Some("CVSS_V3"),
            Some(7.5),
            Some("https://github.com/advisories/GHSA-test-001"),
            Some("2026-01-01T00:00:00Z"),
            None,
        )
        .unwrap();

        let matches = get_matched_advisories(&db).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].advisory_id, "GHSA-test-001");
        assert_eq!(matches[0].installed_version.as_deref(), Some("4.17.20"));
        assert_eq!(matches[0].fixed_version.as_deref(), Some("4.17.21"));
        assert!(matches[0].is_version_confirmed);
        assert_eq!(matches[0].project_paths, vec!["/project/a"]);
    }

    #[test]
    fn test_no_match_when_version_patched() {
        use crate::test_utils::test_db;

        let db = test_db();

        db.store_dependency("/project/a", "lodash", Some("4.17.21"), "npm", false, None)
            .unwrap();

        db.upsert_osv_advisory(
            "GHSA-test-002",
            "Vuln in lodash",
            None,
            "lodash",
            "npm",
            Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]"#),
            Some(r#"["4.17.21"]"#),
            Some("CVSS_V3"),
            Some(7.5),
            None,
            None,
            None,
        )
        .unwrap();

        let matches = get_matched_advisories(&db).unwrap();
        assert!(matches.is_empty(), "Patched version should not match");
    }

    #[test]
    fn test_multiple_projects_same_dep() {
        use crate::test_utils::test_db;

        let db = test_db();

        db.store_dependency("/project/a", "serde", Some("1.0.100"), "rust", false, None)
            .unwrap();
        db.store_dependency("/project/b", "serde", Some("1.0.100"), "rust", false, None)
            .unwrap();

        db.upsert_osv_advisory(
            "GHSA-test-003",
            "Vuln in serde",
            None,
            "serde",
            "crates.io",
            Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.200"}]}]"#),
            Some(r#"["1.0.200"]"#),
            None,
            Some(5.0),
            None,
            None,
            None,
        )
        .unwrap();

        let matches = get_matched_advisories(&db).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].project_paths.len(), 2);
    }
}
