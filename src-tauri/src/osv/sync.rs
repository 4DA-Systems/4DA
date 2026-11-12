// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV sync engine — keeps the local advisory mirror current.
//!
//! Reads which ecosystems the user's dependencies are in, downloads each
//! ecosystem's public advisory archive (`osv-vulnerabilities` bucket,
//! `{ecosystem}/all.zip`) when OSV has published a newer one, and matches it
//! against the dependencies on this machine, storing the advisories that name
//! one of them in `osv_advisories`.
//!
//! Nothing about the user's dependencies is sent: the archive request is the
//! same for every install. Until 2026-11-12 each pass first POSTed every
//! dependency name to `api.osv.dev/v1/querybatch`, which returns ID stubs
//! the mirror cannot store; it stored 0 advisories in 56 of 56 logged runs
//! (audit P2), so it disclosed the dependency list for nothing.

use std::collections::HashMap;
use tracing::{info, warn};

use crate::db::Database;
use crate::error::{FourDaError, Result};

use super::types::{Affected, SyncResult, Vulnerability};

pub(crate) const DEFAULT_SYNC_MAX_AGE_HOURS: i64 = 6;

/// Maximum advisory-mirror age (hours) before a sync is due. Read once from
/// `FOURDA_OSV_MAX_AGE_HOURS`, falling back to [`DEFAULT_SYNC_MAX_AGE_HOURS`].
///
/// The 4DA receipts ledger sets this to `1` so new advisories surface within its hourly
/// cycle instead of waiting up to 6h; the per-ecosystem ETag HEAD check (`is_cache_stale`)
/// keeps a tighter cadence cheap — a re-download only happens when OSV actually publishes a
/// new export, so polling more often costs one HEAD request, not a full mirror pull. With the
/// env var unset, desktop behavior is unchanged (6h). Clamped to a 1h floor so a misconfigured
/// `0` can't force a sync every cycle.
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
    /// Per project: the dependencies (in an ecosystem OSV covers) that no row
    /// gives an installed version. `versioned` is per ECOSYSTEM, so one
    /// pinned project made a sibling's versionless dependencies read as
    /// covered; this keeps them counted (W0-D). Sorted by project.
    pub unversioned: Vec<(String, u32)>,
}

/// A dependency's identity within a project for the coverage count: the
/// project compared case- and separator-blind, the OSV ecosystem, the name
/// as that ecosystem compares names (PEP 503 for PyPI).
fn coverage_key(project: &str, ecosystem: &str, name: &str) -> (String, String, String) {
    let project = project.replace('\\', "/").to_lowercase();
    let name = if ecosystem == "PyPI" {
        name.to_lowercase().replace(['_', '.'], "-")
    } else {
        name.to_lowercase()
    };
    (project, ecosystem.to_string(), name)
}

/// [`AuditableCoverage::unversioned`] of an inventory.
fn unversioned_by_project(deps: &[crate::db::StoredDependency]) -> Vec<(String, u32)> {
    use std::collections::BTreeMap;
    // key -> (project as stored, has a version)
    let mut seen: BTreeMap<(String, String, String), (String, bool)> = BTreeMap::new();
    for d in deps {
        let Some(ecosystem) = normalize_supported_to_osv(&d.ecosystem) else {
            continue;
        };
        let versioned = d.version.as_deref().is_some_and(|v| !v.trim().is_empty());
        let entry = seen
            .entry(coverage_key(&d.project_path, ecosystem, &d.package_name))
            .or_insert_with(|| (d.project_path.clone(), false));
        entry.1 |= versioned;
    }
    let mut by_project: BTreeMap<String, (String, u32)> = BTreeMap::new();
    for ((project, _, _), (stored, versioned)) in seen {
        if !versioned {
            by_project.entry(project).or_insert((stored, 0)).1 += 1;
        }
    }
    by_project.into_values().collect()
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
        unversioned: unversioned_by_project(&deps),
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

/// Run a full sync: read deps → refresh each ecosystem's archive → match it
/// locally → store advisories → update status.
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
    // mirrored too, so a fix target is judged by what it installs. They are
    // matched in the downloaded npm archive like every other name.
    let held_names = with_held_names(
        &mut by_ecosystem,
        super::release_facts::held_names_to_sync(db),
    );

    let mirror = super::cache::Mirror::live();
    for (ecosystem, packages) in &by_ecosystem {
        let outcome = pass::sync_one_ecosystem(db, &mirror, ecosystem, packages).await;
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
