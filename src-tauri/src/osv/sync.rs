// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV sync engine — downloads advisories from OSV API and stores locally.
//!
//! Reads the user's dependencies from the DB, queries the OSV batch API
//! for each ecosystem's packages, and stores the results in osv_advisories.

use std::collections::HashMap;
use tracing::{info, warn};

use crate::db::Database;
use crate::error::{FourDaError, Result};

use super::types::{
    Affected, BatchQuery, BatchRequest, BatchResponse, PackageRef, SyncResult, Vulnerability,
};

const OSV_BATCH_URL: &str = "https://api.osv.dev/v1/querybatch";
const USER_AGENT: &str = "4DA/1.0 (local-osv-mirror)";
const MAX_BATCH_SIZE: usize = 1000;

/// Map from ACE/DB ecosystem names to OSV ecosystem identifiers.
const ECOSYSTEM_NORMALIZE: &[(&str, &str)] = &[
    ("rust", "crates.io"),
    ("javascript", "npm"),
    ("typescript", "npm"),
    ("python", "PyPI"),
    ("pypi", "PyPI"),
    ("pip", "PyPI"),
    ("go", "Go"),
    ("golang", "Go"),
    ("java", "Maven"),
    ("maven", "Maven"),
    ("ruby", "RubyGems"),
    ("rubygems", "RubyGems"),
    ("nuget", "NuGet"),
    ("packagist", "Packagist"),
    ("pub", "Pub"),
    // Already-canonical names pass through
    ("npm", "npm"),
    ("crates.io", "crates.io"),
];

fn normalize_to_osv(ecosystem: &str) -> &str {
    let lower = ecosystem.to_lowercase();
    ECOSYSTEM_NORMALIZE
        .iter()
        .find(|(key, _)| *key == lower.as_str())
        .map(|(_, osv)| *osv)
        .unwrap_or(ecosystem)
}

/// Run a full sync: read deps → query OSV → store advisories → update status.
pub async fn sync(db: &Database) -> Result<SyncResult> {
    let start = std::time::Instant::now();
    let mut result = SyncResult {
        ecosystems_synced: Vec::new(),
        advisories_stored: 0,
        advisories_matched: 0,
        duration_ms: 0,
        errors: Vec::new(),
    };

    let deps = db
        .get_all_user_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read dependencies: {e}")))?;

    if deps.is_empty() {
        info!(target: "4da::osv", "No dependencies found — skipping OSV sync");
        return Ok(result);
    }

    // Group packages by OSV ecosystem
    let mut by_ecosystem: HashMap<String, Vec<String>> = HashMap::new();
    for dep in &deps {
        let osv_eco = normalize_to_osv(&dep.ecosystem).to_string();
        by_ecosystem
            .entry(osv_eco)
            .or_default()
            .push(dep.package_name.clone());
    }

    // Deduplicate package names within each ecosystem
    for packages in by_ecosystem.values_mut() {
        packages.sort();
        packages.dedup();
    }

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    for (ecosystem, packages) in &by_ecosystem {
        let sync_start = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        match sync_ecosystem(&client, db, ecosystem, packages, &sync_start).await {
            Ok(count) => {
                result.ecosystems_synced.push(ecosystem.clone());
                result.advisories_stored += count;
                db.update_osv_sync_status(ecosystem, count as i64, None)
                    .ok();
            }
            Err(e) => {
                let msg = format!("{ecosystem}: {e}");
                warn!(target: "4da::osv", error = %msg, "Ecosystem sync failed");
                result.errors.push(msg);
                db.update_osv_sync_status(ecosystem, 0, Some(&e.to_string()))
                    .ok();
            }
        }
    }

    // Count matched advisories
    result.advisories_matched = super::matching::count_matches(db).unwrap_or(0);
    result.duration_ms = start.elapsed().as_millis() as u64;

    info!(
        target: "4da::osv",
        ecosystems = ?result.ecosystems_synced,
        stored = result.advisories_stored,
        matched = result.advisories_matched,
        duration_ms = result.duration_ms,
        "OSV sync complete"
    );

    Ok(result)
}

/// Sync a single ecosystem: batch-query OSV for all packages, store results.
async fn sync_ecosystem(
    client: &reqwest::Client,
    db: &Database,
    ecosystem: &str,
    packages: &[String],
    sync_start: &str,
) -> Result<usize> {
    info!(
        target: "4da::osv",
        ecosystem = ecosystem,
        packages = packages.len(),
        "Syncing ecosystem"
    );

    let mut total_stored = 0usize;
    let mut seen_ids = std::collections::HashSet::new();

    // Process in batches of MAX_BATCH_SIZE
    for chunk in packages.chunks(MAX_BATCH_SIZE) {
        let queries: Vec<BatchQuery> = chunk
            .iter()
            .map(|name| BatchQuery {
                package: PackageRef {
                    name: name.clone(),
                    ecosystem: ecosystem.to_string(),
                },
                version: None,
            })
            .collect();

        let body = BatchRequest { queries };

        let response = client
            .post(OSV_BATCH_URL)
            .json(&body)
            .send()
            .await
            .map_err(|e| FourDaError::Internal(format!("OSV batch request failed: {e}")))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            warn!(target: "4da::osv", ecosystem = ecosystem, "Rate limited by OSV API");
            return Err(FourDaError::Internal(
                "Rate limited by OSV API (429)".into(),
            ));
        }
        if !status.is_success() {
            return Err(FourDaError::Internal(format!(
                "OSV API error: HTTP {}",
                status.as_u16()
            )));
        }

        let batch: BatchResponse = response
            .json()
            .await
            .map_err(|e| FourDaError::Internal(format!("Failed to parse OSV response: {e}")))?;

        if let Some(results) = batch.results {
            for query_result in results {
                if let Some(vulns) = query_result.vulns {
                    for vuln in vulns {
                        total_stored += store_vulnerability(db, &vuln, &mut seen_ids)?;
                    }
                }
            }
        }

        // Brief pause between batches to be respectful
        if packages.len() > MAX_BATCH_SIZE {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    // Clean up advisories that were withdrawn (no longer returned by OSV)
    let deleted = db
        .delete_stale_osv_advisories(ecosystem, sync_start)
        .unwrap_or(0);
    if deleted > 0 {
        info!(
            target: "4da::osv",
            ecosystem = ecosystem,
            deleted = deleted,
            "Removed withdrawn advisories"
        );
    }

    info!(
        target: "4da::osv",
        ecosystem = ecosystem,
        stored = total_stored,
        "Ecosystem sync complete"
    );

    Ok(total_stored)
}

/// Store a single vulnerability's affected packages into the DB.
/// Returns the number of rows stored.
fn store_vulnerability(
    db: &Database,
    vuln: &Vulnerability,
    seen: &mut std::collections::HashSet<String>,
) -> Result<usize> {
    let (severity_type, cvss_score) = vuln.best_cvss();
    let source_url = vuln.best_url();
    let summary = vuln.summary.as_deref().unwrap_or("Security advisory");

    let affected_list = vuln.affected.as_deref().unwrap_or(&[]);
    let mut stored = 0;

    for affected in affected_list {
        let pkg = match &affected.package {
            Some(p) => p,
            None => continue,
        };

        let dedup_key = format!("{}:{}:{}", vuln.id, pkg.name, pkg.ecosystem);
        if !seen.insert(dedup_key) {
            continue;
        }

        let ranges_json = serialize_ranges(affected);
        let fixed_json = extract_fixed_versions(affected);

        db.upsert_osv_advisory(
            &vuln.id,
            summary,
            vuln.details.as_deref(),
            &pkg.name,
            &pkg.ecosystem,
            ranges_json.as_deref(),
            fixed_json.as_deref(),
            severity_type.as_deref(),
            cvss_score,
            source_url.as_deref(),
            vuln.published.as_deref(),
            vuln.modified.as_deref(),
        )
        .map_err(|e| FourDaError::Internal(format!("Failed to store advisory: {e}")))?;

        stored += 1;
    }

    Ok(stored)
}

/// Serialize the affected ranges to JSON for storage.
fn serialize_ranges(affected: &Affected) -> Option<String> {
    affected
        .ranges
        .as_ref()
        .and_then(|r| serde_json::to_string(r).ok())
}

/// Extract fixed versions from range events.
fn extract_fixed_versions(affected: &Affected) -> Option<String> {
    let fixed: Vec<String> = affected
        .ranges
        .as_ref()
        .map(|ranges| {
            ranges
                .iter()
                .filter_map(|r| r.events.as_ref())
                .flatten()
                .filter_map(|event| {
                    event
                        .as_object()
                        .and_then(|obj| obj.get("fixed"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    if fixed.is_empty() {
        None
    } else {
        serde_json::to_string(&fixed).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            }]),
            references: None,
            published: None,
            modified: None,
        };

        let stored1 = store_vulnerability(&db, &vuln, &mut seen).unwrap();
        assert_eq!(stored1, 1);

        let stored2 = store_vulnerability(&db, &vuln, &mut seen).unwrap();
        assert_eq!(stored2, 0, "Duplicate should be skipped");
    }
}
