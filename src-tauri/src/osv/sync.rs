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
pub(crate) const DEFAULT_SYNC_MAX_AGE_HOURS: i64 = 6;

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
    normalize_supported_to_osv(ecosystem).unwrap_or(ecosystem)
}

fn normalize_supported_to_osv(ecosystem: &str) -> Option<&'static str> {
    let lower = ecosystem.to_lowercase();
    ECOSYSTEM_NORMALIZE
        .iter()
        .find(|(key, _)| *key == lower.as_str())
        .map(|(_, osv)| *osv)
}

/// Public-within-module wrapper for ecosystem normalization.
/// Used by the cache module to map user dependency ecosystems to OSV names.
pub(super) fn normalize_to_osv_pub(ecosystem: &str) -> String {
    normalize_to_osv(ecosystem).to_string()
}

fn auditable_packages_by_ecosystem(db: &Database) -> Result<HashMap<String, Vec<String>>> {
    let mut deps = db
        .get_auditable_user_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read dependencies: {e}")))?;
    let scanned = db
        .get_auditable_scanned_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read scanned dependencies: {e}")))?;
    deps.extend(scanned);

    let mut by_ecosystem: HashMap<String, Vec<String>> = HashMap::new();
    for dep in deps {
        let Some(ecosystem) = normalize_supported_to_osv(&dep.ecosystem) else {
            tracing::debug!(
                target: "4da::osv",
                ecosystem = dep.ecosystem,
                package = dep.package_name,
                "Skipping dependency from unsupported OSV ecosystem"
            );
            continue;
        };
        by_ecosystem
            .entry(ecosystem.to_string())
            .or_default()
            .push(dep.package_name);
    }
    for packages in by_ecosystem.values_mut() {
        packages.sort_by_key(|name| name.to_lowercase());
        packages.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    }
    Ok(by_ecosystem)
}

/// Whether any dependency ecosystem is missing a successful recent sync.
///
/// Requiring every active ecosystem avoids the previous false-fresh state where
/// one recently-synced ecosystem masked a stale or failed sibling ecosystem.
pub(crate) fn needs_sync(db: &Database, max_age_hours: i64) -> Result<bool> {
    let required = auditable_packages_by_ecosystem(db)?;
    if required.is_empty() {
        return Ok(false);
    }

    let statuses = db
        .get_osv_sync_statuses()
        .map_err(|e| FourDaError::Internal(format!("Failed to read OSV sync status: {e}")))?;
    let now = chrono::Utc::now().naive_utc();
    for ecosystem in required.keys() {
        let Some(status) = statuses
            .iter()
            .find(|status| status.ecosystem.eq_ignore_ascii_case(ecosystem))
        else {
            return Ok(true);
        };
        if status.error.is_some() {
            return Ok(true);
        }
        let Some(last_synced) = status.last_synced_at.as_deref().and_then(|timestamp| {
            chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S").ok()
        }) else {
            return Ok(true);
        };
        if now.signed_duration_since(last_synced).num_hours() >= max_age_hours {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Populate advisories for one ecosystem from the locally cached OSV ZIP
/// mirror, (re)downloading the ZIP when it is missing or when OSV has published
/// a newer advisory set (cheap ETag HEAD check) so routine syncs pick up
/// newly-disclosed CVEs.
///
/// This exists because the OSV `/v1/querybatch` API returns only vulnerability
/// ID-stubs (`{id, modified}`) — never the `affected` package ranges that
/// `store_vulnerability` needs — so the batch path cannot fill the mirror on
/// its own. The per-ecosystem ZIP carries full advisory JSON and is matched
/// locally, so it is both the path that actually populates `osv_advisories`
/// and the more privacy-preserving one (nothing about the user's dependency
/// set leaves the machine).
async fn populate_from_zip_mirror(
    db: &Database,
    ecosystem: &str,
    packages: &[String],
) -> Result<usize> {
    let pkg_set: std::collections::HashSet<String> =
        packages.iter().map(|p| p.to_lowercase()).collect();

    // Refresh the cached ZIP when OSV has published a new advisory set for this
    // ecosystem (ETag mismatch) or when no ZIP exists yet. The staleness check is
    // a cheap HTTP HEAD, so the heavy full download (npm's ZIP is ~200MB) only
    // runs when the advisory data actually changed — that is what lets a routine
    // sync pick up newly-published CVEs instead of serving a frozen snapshot.
    // Best effort: a failed check (offline) falls back to the ZIP already on disk
    // (`is_cache_stale` Err -> treat as fresh); a failed download likewise leaves
    // the prior ZIP intact (the download writes the destination only on success);
    // a genuinely missing ZIP is still downloaded by the match arm below.
    if super::cache::is_cache_stale(ecosystem)
        .await
        .unwrap_or(false)
    {
        if let Err(e) = super::cache::download_ecosystem_zip(ecosystem).await {
            warn!(
                target: "4da::osv",
                ecosystem = ecosystem,
                error = %e,
                "ZIP refresh failed; falling back to the existing cached ZIP if present"
            );
        }
    }

    match super::cache::sync_from_zip(db, ecosystem, &pkg_set) {
        Ok(count) => Ok(count),
        Err(_) => {
            // No usable ZIP on disk (first run offline, or a refresh that never
            // completed) — make a final attempt to download, then match locally.
            super::cache::download_ecosystem_zip(ecosystem).await?;
            super::cache::sync_from_zip(db, ecosystem, &pkg_set)
        }
    }
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

    let by_ecosystem = auditable_packages_by_ecosystem(db)?;
    if by_ecosystem.is_empty() {
        info!(target: "4da::osv", "No dependencies found — skipping OSV sync");
        return Ok(result);
    }

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    for (ecosystem, packages) in &by_ecosystem {
        let sync_start = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        match sync_ecosystem(&client, db, ecosystem, packages, &sync_start).await {
            Ok(count) if count > 0 => {
                result.ecosystems_synced.push(ecosystem.clone());
                result.advisories_stored += count;
                db.update_osv_sync_status(ecosystem, count as i64, None)
                    .ok();
            }
            // The batch API stored nothing. `/v1/querybatch` returns vulnerability
            // ID-stubs without the `affected` ranges `store_vulnerability` needs,
            // so it can never populate the mirror on its own. Fall back to the
            // cached ZIP mirror (full advisory JSON, matched locally). This — not
            // the batch query — is what actually fills `osv_advisories` for
            // npm/crates.io, and it never sends the user's dependency set anywhere.
            Ok(_) => match populate_from_zip_mirror(db, ecosystem, packages).await {
                Ok(zip_count) if zip_count > 0 => {
                    info!(
                        target: "4da::osv",
                        ecosystem = ecosystem.as_str(),
                        cached_advisories = zip_count,
                        "Batch API returned no storable advisories; populated from ZIP mirror"
                    );
                    result.ecosystems_synced.push(ecosystem.clone());
                    result.advisories_stored += zip_count;
                    db.update_osv_sync_status(ecosystem, zip_count as i64, None)
                        .ok();
                }
                Ok(_) => {
                    // Genuinely no advisories for this ecosystem's packages.
                    result.ecosystems_synced.push(ecosystem.clone());
                    db.update_osv_sync_status(ecosystem, 0, None).ok();
                }
                Err(e) => {
                    let msg = format!("{ecosystem} (zip): {e}");
                    warn!(target: "4da::osv", error = %msg, "ZIP mirror population failed");
                    result.errors.push(msg);
                    db.update_osv_sync_status(ecosystem, 0, Some(&e.to_string()))
                        .ok();
                }
            },
            Err(e) => {
                let err_str = e.to_string();
                // Rate limit errors should not fall back to cache
                let is_rate_limit = err_str.contains("429");

                if !is_rate_limit {
                    // Attempt cache fallback for network/server errors
                    if let Ok(zip_count) = populate_from_zip_mirror(db, ecosystem, packages).await {
                        if zip_count > 0 {
                            info!(
                                target: "4da::osv",
                                ecosystem = ecosystem.as_str(),
                                cached_advisories = zip_count,
                                "API sync failed, fell back to cached ZIP"
                            );
                            result.ecosystems_synced.push(ecosystem.clone());
                            result.advisories_stored += zip_count;
                            db.update_osv_sync_status(ecosystem, zip_count as i64, None)
                                .ok();
                            continue;
                        }
                    }
                }

                let msg = format!("{ecosystem}: {e}");
                warn!(target: "4da::osv", error = %msg, "Ecosystem sync failed");
                result.errors.push(msg);
                db.update_osv_sync_status(ecosystem, 0, Some(&err_str)).ok();
            }
        }
    }

    // Retire dependency alerts whose package has since been patched out of the
    // affected range, so the dashboard/Preemption counts reflect only live risks
    // right after a manual sync rather than waiting for the 6h health job.
    let resolved_alerts =
        crate::dependency_health::resolve_patched_dependency_alerts(db).unwrap_or(0);
    if resolved_alerts > 0 {
        info!(target: "4da::osv", resolved_alerts, "Retired patched dependency alerts");
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

    // Clean up advisories that were withdrawn (no longer returned by OSV) —
    // but ONLY when we actually stored fresh advisories this pass. The OSV
    // `/v1/querybatch` endpoint returns vulnerability *ID stubs* (`{id, modified}`)
    // with no `affected` package data, so `store_vulnerability` can store nothing
    // from the batch path (`total_stored == 0`). Pruning on a zero-store would
    // treat the entire existing mirror as "withdrawn" and WIPE it — exactly the
    // silent failure that left Preemption empty while real CVEs existed. When the
    // batch path stores nothing, `sync()` repopulates from the cached ZIP mirror
    // instead; leave the existing rows intact here.
    if total_stored > 0 {
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
pub(super) fn store_vulnerability(
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
            vuln.withdrawn.as_deref(),
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
            withdrawn: None,
        };

        let stored1 = store_vulnerability(&db, &vuln, &mut seen).unwrap();
        assert_eq!(stored1, 1);

        let stored2 = store_vulnerability(&db, &vuln, &mut seen).unwrap();
        assert_eq!(stored2, 0, "Duplicate should be skipped");
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
}
