//! CVE/NVD feed source adapter for the Developer Immune System.
//!
//! Fetches security advisories from GitHub Advisory Database and NVD.
//! Cross-references against user's installed dependencies to generate
//! targeted vulnerability alerts.

use anyhow::Result;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use super::{shared_client, SourceItem};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CveAdvisory {
    pub cve_id: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub cvss_score: Option<f32>,
    pub affected_packages: Vec<AffectedPackage>,
    pub published_at: String,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AffectedPackage {
    pub name: String,
    pub ecosystem: String,
    pub affected_versions: String,
    pub patched_version: Option<String>,
}

// ============================================================================
// GitHub Advisory Database fetcher
// ============================================================================

/// Fetch recent advisories from GitHub Advisory Database.
/// This is preferred over NVD because it includes ecosystem-specific package data.
pub(crate) async fn fetch_github_advisories(ecosystem: Option<&str>) -> Result<Vec<CveAdvisory>> {
    let client = shared_client();
    let mut url =
        "https://api.github.com/advisories?per_page=30&sort=published&direction=desc".to_string();
    if let Some(eco) = ecosystem {
        url.push_str(&format!("&ecosystem={eco}"));
    }

    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "4DA-Developer-OS/1.0")
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(vec![]);
    }

    let body: serde_json::Value = response.json().await?;
    let mut advisories = Vec::new();

    if let Some(items) = body.as_array() {
        for item in items {
            if let Some(advisory) = parse_github_advisory(item) {
                advisories.push(advisory);
            }
        }
    }

    Ok(advisories)
}

fn parse_github_advisory(item: &serde_json::Value) -> Option<CveAdvisory> {
    let ghsa_id = item.get("ghsa_id")?.as_str()?;
    let cve_id = item
        .get("cve_id")
        .and_then(|v| v.as_str())
        .unwrap_or(ghsa_id);
    let summary = item.get("summary")?.as_str()?;
    let description = item
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let severity = item
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("MEDIUM");
    let cvss_score = item
        .get("cvss")
        .and_then(|v| v.get("score"))
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let published = item
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let url = item.get("html_url").and_then(|v| v.as_str()).unwrap_or("");

    let mut affected_packages = Vec::new();
    if let Some(vulns) = item.get("vulnerabilities").and_then(|v| v.as_array()) {
        for vuln in vulns {
            if let Some(pkg) = vuln.get("package") {
                let name = pkg.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let ecosystem = pkg.get("ecosystem").and_then(|v| v.as_str()).unwrap_or("");
                let range = vuln
                    .get("vulnerable_version_range")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let patched = vuln
                    .get("patched_versions")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                if !name.is_empty() {
                    affected_packages.push(AffectedPackage {
                        name: name.to_string(),
                        ecosystem: ecosystem.to_lowercase(),
                        affected_versions: range.to_string(),
                        patched_version: patched,
                    });
                }
            }
        }
    }

    Some(CveAdvisory {
        cve_id: cve_id.to_string(),
        title: summary.to_string(),
        description: description.to_string(),
        severity: severity.to_uppercase(),
        cvss_score,
        affected_packages,
        published_at: published.to_string(),
        source_url: url.to_string(),
    })
}

// ============================================================================
// Integration with scoring pipeline
// ============================================================================

/// Convert CVE advisories to SourceItems for the PASIFA scoring pipeline.
pub(crate) fn advisories_to_source_items(advisories: &[CveAdvisory]) -> Vec<SourceItem> {
    advisories
        .iter()
        .map(|a| {
            let packages: Vec<String> = a
                .affected_packages
                .iter()
                .map(|p| format!("{} ({})", p.name, p.ecosystem))
                .collect();

            let content = format!(
                "{}\n\nSeverity: {}\nAffected: {}\n{}",
                a.description,
                a.severity,
                packages.join(", "),
                a.cvss_score
                    .map(|s| format!("CVSS: {s:.1}"))
                    .unwrap_or_default()
            );

            SourceItem {
                source_id: a.cve_id.clone(),
                source_type: "cve".to_string(),
                title: format!("[{}] {}", a.cve_id, a.title),
                url: Some(a.source_url.clone()),
                content,
                metadata: serde_json::to_value(&a.affected_packages).ok(),
            }
        })
        .collect()
}

/// Normalize ecosystem names to canonical forms for consistent cross-referencing.
/// GitHub Advisory Database, NVD, and user lockfiles may use different names
/// for the same ecosystem (e.g., "javascript" vs "npm" vs "typescript").
fn normalize_ecosystem(eco: &str) -> &str {
    match eco.to_lowercase().as_str() {
        "javascript" | "typescript" | "npm" => "npm",
        "rust" | "crates.io" => "crates.io",
        "python" | "pip" | "pypi" => "pip",
        "go" | "golang" => "go",
        "ruby" | "rubygems" => "rubygems",
        "java" | "maven" => "maven",
        _ => eco,
    }
}

/// Normalize a version range string from GitHub Advisory / npm format into
/// something the `semver` crate's `VersionReq` can parse. GitHub advisories
/// use ranges like `"< 4.17.21"` or `">= 2.0.0, < 2.1.5"` which mostly
/// align with Cargo's semver syntax, but a few transformations are needed:
///
/// - Strip leading `= ` (exact pin) → bare version for `VersionReq`
/// - Collapse double-spaces around operators
/// - Return `None` for empty/unparseable ranges (caller falls back to name match)
fn parse_version_range(range: &str) -> Option<VersionReq> {
    let trimmed = range.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try parsing directly first — works for most GitHub Advisory ranges
    if let Ok(req) = VersionReq::parse(trimmed) {
        return Some(req);
    }

    // Normalize: some advisories use `= 1.2.3` (exact pin) which VersionReq
    // doesn't accept — it wants `=1.2.3` (no space after `=` when alone)
    let normalized = trimmed
        .replace("= ", "=")
        .replace(">  ", "> ")
        .replace("<  ", "< ");

    VersionReq::parse(&normalized).ok()
}

/// Try to parse a user-supplied version string as a semver `Version`.
/// Handles common non-semver formats:
/// - Two-part versions like `"1.2"` → `"1.2.0"`
/// - Leading `v` prefix → stripped
/// - Anything else returns `None` (caller falls back to name match)
fn parse_user_version(ver: &str) -> Option<Version> {
    let v = ver.trim().trim_start_matches('v');
    if v.is_empty() {
        return None;
    }

    // Try direct parse
    if let Ok(version) = Version::parse(v) {
        return Some(version);
    }

    // Handle two-part versions (e.g., "1.2" → "1.2.0")
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() == 2 {
        if let Ok(version) = Version::parse(&format!("{v}.0")) {
            return Some(version);
        }
    }

    None
}

/// Check whether a user's version falls within an advisory's affected range.
/// Returns `true` (conservatively match) when:
/// - User has no version info (`None`)
/// - Version can't be parsed as semver
/// - Range can't be parsed as semver
///
/// Returns `false` only when both version and range parse successfully and
/// the version is outside the affected range.
fn version_is_affected(user_version: Option<&str>, affected_range: &str) -> bool {
    let user_ver = match user_version {
        Some(v) => v,
        // No version info — conservatively assume affected
        None => return true,
    };

    let parsed_version = match parse_user_version(user_ver) {
        Some(v) => v,
        // Can't parse user version — fall back to name-only match (conservative)
        None => return true,
    };

    let req = match parse_version_range(affected_range) {
        Some(r) => r,
        // Can't parse range — fall back to name-only match (conservative)
        None => return true,
    };

    req.matches(&parsed_version)
}

/// Cross-reference CVE advisories against user's installed dependencies.
/// Returns advisories that affect the user, with the matching packages.
///
/// Version-aware: if the user provides version info, the advisory's
/// `affected_versions` range is checked against it. Only dependencies
/// whose version falls within the affected range are matched.
///
/// Conservative fallback: if version info is missing or unparseable,
/// the dependency is still matched (alert rather than miss).
///
/// Ecosystem names are normalized before comparison to handle mismatches
/// between advisory sources and lockfile formats.
pub(crate) fn cross_reference_advisories(
    advisories: &[CveAdvisory],
    user_deps: &[(String, String, Option<String>)], // (name, ecosystem, version)
) -> Vec<(CveAdvisory, Vec<AffectedPackage>)> {
    let mut matches = Vec::new();

    for advisory in advisories {
        let matched: Vec<AffectedPackage> = advisory
            .affected_packages
            .iter()
            .filter(|ap| {
                user_deps.iter().any(|(name, eco, version)| {
                    let name_match = name.eq_ignore_ascii_case(&ap.name);
                    let eco_match = normalize_ecosystem(eco) == normalize_ecosystem(&ap.ecosystem);
                    let ver_match = version_is_affected(version.as_deref(), &ap.affected_versions);

                    name_match && eco_match && ver_match
                })
            })
            .cloned()
            .collect();

        if !matched.is_empty() {
            matches.push((advisory.clone(), matched));
        }
    }

    matches
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_advisory() -> CveAdvisory {
        CveAdvisory {
            cve_id: "CVE-2026-0001".to_string(),
            title: "Prototype pollution in lodash".to_string(),
            description: "A prototype pollution vulnerability exists in lodash".to_string(),
            severity: "HIGH".to_string(),
            cvss_score: Some(7.5),
            affected_packages: vec![AffectedPackage {
                name: "lodash".to_string(),
                ecosystem: "npm".to_string(),
                affected_versions: "< 4.17.21".to_string(),
                patched_version: Some("4.17.21".to_string()),
            }],
            published_at: "2026-03-19T00:00:00Z".to_string(),
            source_url: "https://github.com/advisories/GHSA-test".to_string(),
        }
    }

    #[test]
    fn test_advisory_to_source_item() {
        let items = advisories_to_source_items(&[sample_advisory()]);
        assert_eq!(items.len(), 1);
        assert!(items[0].title.contains("CVE-2026-0001"));
        assert_eq!(items[0].source_type, "cve");
        assert!(items[0].content.contains("HIGH"));
    }

    #[test]
    fn test_cross_reference_match_no_version() {
        let advisories = vec![sample_advisory()];
        let user_deps = vec![("lodash".to_string(), "npm".to_string(), None)];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1[0].name, "lodash");
    }

    #[test]
    fn test_cross_reference_no_match() {
        let advisories = vec![sample_advisory()];
        let user_deps = vec![("express".to_string(), "npm".to_string(), None)];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_cross_reference_case_insensitive() {
        let advisories = vec![sample_advisory()];
        let user_deps = vec![("Lodash".to_string(), "NPM".to_string(), None)];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_parse_github_advisory_valid() {
        let json = serde_json::json!({
            "ghsa_id": "GHSA-test-1234",
            "cve_id": "CVE-2026-9999",
            "summary": "Test vulnerability",
            "description": "A test vulnerability",
            "severity": "high",
            "cvss": { "score": 7.5 },
            "published_at": "2026-03-19T00:00:00Z",
            "html_url": "https://github.com/advisories/GHSA-test-1234",
            "vulnerabilities": [{
                "package": {
                    "name": "test-pkg",
                    "ecosystem": "npm"
                },
                "vulnerable_version_range": "< 2.0.0",
                "patched_versions": "2.0.0"
            }]
        });

        let advisory = parse_github_advisory(&json);
        assert!(advisory.is_some());
        let a = advisory.unwrap();
        assert_eq!(a.cve_id, "CVE-2026-9999");
        assert_eq!(a.affected_packages.len(), 1);
        assert_eq!(a.affected_packages[0].name, "test-pkg");
    }

    #[test]
    fn test_parse_github_advisory_minimal() {
        let json = serde_json::json!({
            "ghsa_id": "GHSA-minimal",
            "summary": "Minimal advisory"
        });

        let advisory = parse_github_advisory(&json);
        assert!(advisory.is_some());
        let a = advisory.unwrap();
        assert_eq!(a.cve_id, "GHSA-minimal");
        assert!(a.affected_packages.is_empty());
    }

    #[test]
    fn test_normalize_ecosystem() {
        assert_eq!(normalize_ecosystem("javascript"), "npm");
        assert_eq!(normalize_ecosystem("typescript"), "npm");
        assert_eq!(normalize_ecosystem("npm"), "npm");
        assert_eq!(normalize_ecosystem("rust"), "crates.io");
        assert_eq!(normalize_ecosystem("crates.io"), "crates.io");
        assert_eq!(normalize_ecosystem("python"), "pip");
        assert_eq!(normalize_ecosystem("pypi"), "pip");
        assert_eq!(normalize_ecosystem("go"), "go");
        assert_eq!(normalize_ecosystem("golang"), "go");
        assert_eq!(normalize_ecosystem("ruby"), "rubygems");
        assert_eq!(normalize_ecosystem("java"), "maven");
        assert_eq!(normalize_ecosystem("unknown_eco"), "unknown_eco");
    }

    #[test]
    fn test_cross_reference_ecosystem_normalization() {
        let advisories = vec![sample_advisory()]; // ecosystem: "npm"
                                                  // User has "javascript" ecosystem — should still match via normalization
        let user_deps = vec![("lodash".to_string(), "javascript".to_string(), None)];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert_eq!(matches.len(), 1, "Should match via ecosystem normalization");
    }

    // ========================================================================
    // Version range matching tests
    // ========================================================================

    #[test]
    fn test_version_in_affected_range_matches() {
        // Advisory: lodash affected < 4.17.21
        let advisories = vec![sample_advisory()];
        let user_deps = vec![(
            "lodash".to_string(),
            "npm".to_string(),
            Some("4.17.20".to_string()),
        )];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert_eq!(
            matches.len(),
            1,
            "Version 4.17.20 is < 4.17.21, should match"
        );
    }

    #[test]
    fn test_version_outside_affected_range_no_match() {
        // Advisory: lodash affected < 4.17.21
        let advisories = vec![sample_advisory()];
        let user_deps = vec![(
            "lodash".to_string(),
            "npm".to_string(),
            Some("4.17.21".to_string()),
        )];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert!(
            matches.is_empty(),
            "Version 4.17.21 is NOT < 4.17.21, should not match"
        );
    }

    #[test]
    fn test_version_well_above_range_no_match() {
        let advisories = vec![sample_advisory()];
        let user_deps = vec![(
            "lodash".to_string(),
            "npm".to_string(),
            Some("5.0.0".to_string()),
        )];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert!(matches.is_empty(), "Version 5.0.0 is NOT < 4.17.21");
    }

    #[test]
    fn test_no_version_info_conservative_match() {
        // No version → conservative: assume affected
        let advisories = vec![sample_advisory()];
        let user_deps = vec![("lodash".to_string(), "npm".to_string(), None)];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert_eq!(
            matches.len(),
            1,
            "No version info should conservatively match"
        );
    }

    #[test]
    fn test_unparseable_version_falls_back_to_name_match() {
        // Unparseable version (e.g., Python's "2024.1.post1") → conservative match
        let advisories = vec![sample_advisory()];
        let user_deps = vec![(
            "lodash".to_string(),
            "npm".to_string(),
            Some("not-a-version".to_string()),
        )];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert_eq!(
            matches.len(),
            1,
            "Unparseable version should fall back to name-only match"
        );
    }

    #[test]
    fn test_compound_version_range() {
        // Advisory with compound range: >= 2.0.0, < 2.1.5
        let advisory = CveAdvisory {
            cve_id: "CVE-2026-0002".to_string(),
            title: "Vuln in some-pkg".to_string(),
            description: "Test".to_string(),
            severity: "CRITICAL".to_string(),
            cvss_score: Some(9.1),
            affected_packages: vec![AffectedPackage {
                name: "some-pkg".to_string(),
                ecosystem: "npm".to_string(),
                affected_versions: ">= 2.0.0, < 2.1.5".to_string(),
                patched_version: Some("2.1.5".to_string()),
            }],
            published_at: "2026-03-19T00:00:00Z".to_string(),
            source_url: "https://example.com".to_string(),
        };

        // Version 2.1.0 is in range [2.0.0, 2.1.5)
        let in_range = vec![(
            "some-pkg".to_string(),
            "npm".to_string(),
            Some("2.1.0".to_string()),
        )];
        let matches = cross_reference_advisories(&[advisory.clone()], &in_range);
        assert_eq!(matches.len(), 1, "2.1.0 is in [2.0.0, 2.1.5)");

        // Version 1.9.0 is below range
        let below_range = vec![(
            "some-pkg".to_string(),
            "npm".to_string(),
            Some("1.9.0".to_string()),
        )];
        let matches = cross_reference_advisories(&[advisory.clone()], &below_range);
        assert!(matches.is_empty(), "1.9.0 is below >= 2.0.0");

        // Version 2.1.5 is at the boundary (not affected)
        let at_boundary = vec![(
            "some-pkg".to_string(),
            "npm".to_string(),
            Some("2.1.5".to_string()),
        )];
        let matches = cross_reference_advisories(&[advisory], &at_boundary);
        assert!(matches.is_empty(), "2.1.5 is NOT < 2.1.5");
    }

    #[test]
    fn test_version_with_v_prefix() {
        let advisories = vec![sample_advisory()];
        let user_deps = vec![(
            "lodash".to_string(),
            "npm".to_string(),
            Some("v4.17.20".to_string()),
        )];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        assert_eq!(
            matches.len(),
            1,
            "v-prefixed version should parse correctly"
        );
    }

    #[test]
    fn test_two_part_version() {
        let advisories = vec![sample_advisory()]; // affected < 4.17.21
        let user_deps = vec![(
            "lodash".to_string(),
            "npm".to_string(),
            Some("4.17".to_string()),
        )];

        let matches = cross_reference_advisories(&advisories, &user_deps);
        // 4.17.0 < 4.17.21, so should match
        assert_eq!(
            matches.len(),
            1,
            "Two-part version '4.17' -> '4.17.0' should match"
        );
    }

    #[test]
    fn test_parse_version_range_empty() {
        assert!(parse_version_range("").is_none());
        assert!(parse_version_range("   ").is_none());
    }

    #[test]
    fn test_parse_version_range_valid() {
        assert!(parse_version_range("< 4.17.21").is_some());
        assert!(parse_version_range(">= 2.0.0, < 2.1.5").is_some());
        assert!(parse_version_range("^1.0.0").is_some());
        assert!(parse_version_range("~1.2.3").is_some());
    }

    #[test]
    fn test_version_is_affected_direct() {
        // Affected: < 4.17.21
        assert!(version_is_affected(Some("4.17.20"), "< 4.17.21"));
        assert!(!version_is_affected(Some("4.17.21"), "< 4.17.21"));
        assert!(!version_is_affected(Some("5.0.0"), "< 4.17.21"));

        // No version → conservative true
        assert!(version_is_affected(None, "< 4.17.21"));

        // Unparseable version → conservative true
        assert!(version_is_affected(Some("banana"), "< 4.17.21"));

        // Empty range → conservative true
        assert!(version_is_affected(Some("1.0.0"), ""));
    }
}
