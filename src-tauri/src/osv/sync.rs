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
/// The batch API's rate-limit refusal. The pass matches this exact text: a
/// bare "429" also appears in other errors (a port number in a URL).
const RATE_LIMITED: &str = "Rate limited by OSV API (429)";

/// The remote endpoints and the local archive store a sync pass uses.
/// Production is [`SyncEndpoints::live`]; tests point both at a local
/// stand-in, so the pass under test is the one that ships.
pub(crate) struct SyncEndpoints {
    pub(crate) batch_url: String,
    pub(crate) mirror: super::cache::Mirror,
}

impl SyncEndpoints {
    fn live() -> Self {
        Self {
            batch_url: OSV_BATCH_URL.to_string(),
            mirror: super::cache::Mirror::live(),
        }
    }
}

/// Maximum advisory-mirror age (hours) before a sync is due. Read once from
/// `FOURDA_OSV_MAX_AGE_HOURS`, falling back to [`DEFAULT_SYNC_MAX_AGE_HOURS`].
///
/// The 4DA receipts ledger sets this to `1` so new advisories surface within its hourly
/// cycle instead of waiting up to 6h; the per-ecosystem ETag HEAD check (`is_cache_stale`)
/// keeps a tighter cadence cheap — a re-download only happens when OSV actually publishes a
/// new export, so polling more often costs one HEAD request, not a full mirror pull. With the
/// env var unset, desktop behavior is unchanged (6h). Clamped to a 1h floor so a misconfigured
/// `0` can't force a sync every cycle against the rate-limited API fallback.
pub(crate) fn osv_sync_max_age_hours() -> i64 {
    static MAX_AGE: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *MAX_AGE.get_or_init(|| parse_osv_max_age(std::env::var("FOURDA_OSV_MAX_AGE_HOURS").ok()))
}

/// Pure parser for [`osv_sync_max_age_hours`]: a positive integer hours value, clamped to a
/// 1h floor; any missing/blank/non-numeric input falls back to [`DEFAULT_SYNC_MAX_AGE_HOURS`].
fn parse_osv_max_age(raw: Option<String>) -> i64 {
    raw.and_then(|v| v.trim().parse::<i64>().ok())
        .map(|h| h.max(1))
        .unwrap_or(DEFAULT_SYNC_MAX_AGE_HOURS)
}

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
    ("kotlin", "Maven"),
    ("ruby", "RubyGems"),
    ("rubygems", "RubyGems"),
    // The ACE scanner tags scanned dependencies with the project LANGUAGE (see
    // project_dependencies.language: "csharp"/"php"/"dart"), not the package-registry name.
    // Java/Ruby above carry BOTH the language key and the registry-alias key, so they resolve;
    // these three previously had ONLY the registry alias ("nuget"/"packagist"/"pub"), so a
    // csharp/php/dart dependency normalized to nothing and its whole ecosystem was silently
    // skipped — no advisory sync, no vulnerabilities surfaced. The cross-stack ledger gate
    // (4da-ledger verify-stack-coverage) caught this: C#/PHP/Dart fixtures grounded against
    // live OSV but the engine surfaced zero. Add the language-name keys to close the gap.
    ("csharp", "NuGet"),
    ("c#", "NuGet"),
    ("dotnet", "NuGet"),
    ("nuget", "NuGet"),
    ("php", "Packagist"),
    ("packagist", "Packagist"),
    ("dart", "Pub"),
    ("flutter", "Pub"),
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

/// Add the held dependency names to the npm set (when the user has npm
/// dependencies at all); returns the names added or already present.
fn with_held_names(
    by_ecosystem: &mut HashMap<String, Vec<String>>,
    held: Vec<String>,
) -> Vec<String> {
    let Some(npm) = by_ecosystem.get_mut("npm") else {
        return Vec::new();
    };
    if held.is_empty() {
        return held;
    }
    npm.extend(held.iter().cloned());
    npm.sort_by_key(|name| name.to_lowercase());
    npm.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    held
}

/// What the auditable inventory holds, by OSV ecosystem.
pub(crate) struct AuditableCoverage {
    /// The ecosystems the advisory check can speak for: those with at least
    /// one auditable row that carries an installed version, sorted. A
    /// manifest row has none until a lockfile pins it, and a versionless
    /// dependency is never matched (#974). A Maven-only project made "Maven"
    /// required, the sync succeeded, and readiness said Ready while nothing
    /// was checked (W5a). The sync itself still covers every auditable
    /// ecosystem ([`sync`], [`needs_sync`]).
    pub versioned: Vec<String>,
    /// Some auditable row is in an ecosystem OSV covers, versioned or not.
    pub any_checkable: bool,
}

/// The auditable inventory's coverage, read once for readiness: which
/// ecosystems are checkable now, and whether any checkable row exists at all
/// (readiness tells "not read yet" from "nothing checkable" with it).
pub(crate) fn auditable_coverage(db: &Database) -> Result<AuditableCoverage> {
    let mut deps = db
        .get_auditable_user_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read dependencies: {e}")))?;
    deps.extend(
        db.get_auditable_scanned_dependencies().map_err(|e| {
            FourDaError::Internal(format!("Failed to read scanned dependencies: {e}"))
        })?,
    );
    let any_checkable = deps
        .iter()
        .any(|d| normalize_supported_to_osv(&d.ecosystem).is_some());
    let mut versioned: Vec<String> = deps
        .iter()
        .filter(|d| d.version.as_deref().is_some_and(|v| !v.trim().is_empty()))
        .filter_map(|d| normalize_supported_to_osv(&d.ecosystem))
        .map(str::to_string)
        .collect();
    versioned.sort();
    versioned.dedup();
    Ok(AuditableCoverage {
        versioned,
        any_checkable,
    })
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

    let mut by_ecosystem = auditable_packages_by_ecosystem(db)?;
    if by_ecosystem.is_empty() {
        info!(target: "4da::osv", "No dependencies found — skipping OSV sync");
        return Ok(result);
    }
    // D-4: the npm dependencies a vulnerable package's releases exact-pin or
    // narrowly range (read from the registry facts) have their advisories
    // mirrored too, so a fix target is judged by what it installs. Public
    // names from public registry documents of packages already sent here.
    let held_names = with_held_names(
        &mut by_ecosystem,
        super::release_facts::held_names_to_sync(db),
    );

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .redirect(crate::http_client::ssrf_guarded_redirect_policy())
        .build()
        .unwrap_or_default();

    let endpoints = SyncEndpoints::live();
    for (ecosystem, packages) in &by_ecosystem {
        let outcome = pass::sync_one_ecosystem(&client, db, &endpoints, ecosystem, packages).await;
        pass::record_outcome(db, ecosystem, outcome, &mut result);
    }

    // The held names are covered once the npm sync that included them
    // succeeded: "no advisory" for one of them is then a reading. A pass that
    // could not verify npm's data records an error, so it does not count.
    if !held_names.is_empty() && !result.errors.iter().any(|e| e.starts_with("npm")) {
        super::release_facts::record_held_synced(db, &held_names);
    }

    // Prune orphan advisories for ecosystems the user no longer depends on
    // (e.g. NuGet/Maven/PyPI rows left by an earlier unfiltered cache ingestion
    // that can never match the current npm/crates.io dependency set). Keyed on
    // the ecosystems we actually audited this run; no-op when that set is empty.
    let keep: Vec<String> = by_ecosystem.keys().cloned().collect();
    match db.prune_advisories_outside_ecosystems(&keep) {
        Ok(pruned) if pruned > 0 => {
            info!(target: "4da::osv", pruned, "Pruned orphan advisories for unused ecosystems");
            pass::forget_ingested_archives(db);
        }
        Err(e) => warn!(target: "4da::osv", error = %e, "Orphan-advisory prune failed"),
        _ => {}
    }

    // Retire dependency alerts whose package has since been patched out of the
    // affected range, so the dashboard/Preemption counts reflect only live risks
    // right after a manual sync rather than waiting for the 6h health job.
    let resolved_alerts =
        crate::dependency_health::resolve_patched_dependency_alerts(db).unwrap_or(0);
    if resolved_alerts > 0 {
        info!(target: "4da::osv", resolved_alerts, "Retired patched dependency alerts");
    }

    // Which releases the registries withdrew (yanked, deprecated), for the
    // packages that now have a confirmed advisory: no fix target may name
    // one. Best effort; without it targets come from the advisory data.
    super::release_facts::refresh(db).await;

    // Count matched advisories
    result.advisories_matched = super::matching::count_matches(db).unwrap_or(0);
    result.duration_ms = start.elapsed().as_millis() as u64;

    if result.errors.is_empty() {
        info!(
            target: "4da::osv",
            ecosystems = ?result.ecosystems_synced,
            stored = result.advisories_stored,
            matched = result.advisories_matched,
            duration_ms = result.duration_ms,
            "OSV sync complete"
        );
    } else {
        // Not "complete": an ecosystem listed here was not verified, whatever
        // was matched from the archive on disk.
        warn!(
            target: "4da::osv",
            verified = ?result.ecosystems_synced,
            not_verified = ?result.errors,
            stored = result.advisories_stored,
            matched = result.advisories_matched,
            duration_ms = result.duration_ms,
            "OSV sync finished without verifying every ecosystem"
        );
    }

    Ok(result)
}

/// Sync a single ecosystem: batch-query OSV for all packages, store results.
async fn sync_ecosystem(
    client: &reqwest::Client,
    db: &Database,
    batch_url: &str,
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
            .post(batch_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| FourDaError::Internal(format!("OSV batch request failed: {e}")))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            warn!(target: "4da::osv", ecosystem = ecosystem, "Rate limited by OSV API");
            return Err(FourDaError::Internal(RATE_LIMITED.into()));
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
            pass::forget_ingested_archives(db);
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
pub(crate) fn store_vulnerability(
    db: &Database,
    vuln: &Vulnerability,
    seen: &mut std::collections::HashSet<String>,
) -> Result<usize> {
    let (severity_type, cvss_score) = vuln.best_cvss();
    let source_url = vuln.best_url();
    let summary = vuln.summary.as_deref().unwrap_or("Security advisory");

    let mut stored = 0;
    for (key, affected) in merge_affected(vuln) {
        let dedup_key = format!("{}:{}:{}", vuln.id, key.0, key.1);
        if !seen.insert(dedup_key) {
            continue;
        }
        let ranges_json = serialize_ranges(&affected);
        let fixed_json = extract_fixed_versions(&affected);
        // THE notice rule for this package (`fourda_depengine::osv::
        // is_notice_for`): the summary, the record-level flag, or this
        // package's own `affected[].database_specific.informational`.
        let informational = vuln.is_notice_for(&affected);

        db.upsert_osv_advisory_record(
            &vuln.id,
            summary,
            vuln.details.as_deref(),
            &key.0,
            &key.1,
            ranges_json.as_deref(),
            fixed_json.as_deref(),
            severity_type.as_deref(),
            cvss_score,
            source_url.as_deref(),
            vuln.published.as_deref(),
            vuln.modified.as_deref(),
            vuln.withdrawn.as_deref(),
            vuln.aliases_json().as_deref(),
            vuln.severity_label().as_deref(),
            vuln.cwe_ids_json().as_deref(),
            Some(informational),
        )
        .map_err(|e| FourDaError::Internal(format!("Failed to store advisory: {e}")))?;

        stored += 1;
    }

    Ok(stored)
}

/// One advisory's affected entries grouped by (package, ecosystem), in first
/// appearance order, with their ranges UNIONED and an explicit `versions`
/// list as an `ENUMERATED` range. The rule lives in the
/// pure engine crate (`fourda_depengine::osv::merge_affected`, single-engine
/// C0.2), so the mirror, the conformance harness, the rule vectors and the
/// wasm module store a record identically.
pub(super) fn merge_affected(vuln: &Vulnerability) -> Vec<((String, String), Affected)> {
    fourda_depengine::osv::merge_affected(vuln.affected.as_deref().unwrap_or(&[]))
}

/// Serialize the affected ranges to JSON for storage.
pub(super) fn serialize_ranges(affected: &Affected) -> Option<String> {
    fourda_depengine::osv::serialize_ranges(affected)
}

/// Extract fixed versions from range events.
fn extract_fixed_versions(affected: &Affected) -> Option<String> {
    fourda_depengine::osv::fixed_versions_json(affected)
}

// One ecosystem's pass and what it may record (verified, unverified, failed).
#[path = "sync_pass.rs"]
mod pass;

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
