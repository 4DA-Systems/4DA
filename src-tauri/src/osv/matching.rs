// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV matching — the database adapter around the engine crate's matcher.
//!
//! The rules (which installed copies an advisory affects, confirmed or
//! conservative, and each copy's fix targets) are the pure
//! `fourda_depengine::matching::match_inventory` (single-engine C0.2). This
//! module reads `osv_advisories`, `user_dependencies`, `project_dependencies`
//! and `dependency_instances`, hands them over as an inventory and advisory
//! records, supplies the one host step (AD-051 reachability: cargo's resolved
//! features and the crate's source), and turns the findings back into
//! `MatchedAdvisory` rows (Tier 1 intelligence).

use fourda_depengine::matching::{match_inventory_with, Package};
use fourda_depengine::osv::AdvisoryRecord;

use crate::db::{Database, StoredDependency};
use crate::error::{FourDaError, Result};

use super::reachability::ReachFilter;
use super::types::{MatchedAdvisory, NotCompiledMatch, StoredAdvisory};

// The pure affected-window evaluation lives in the engine crate (single-engine
// C0.1). Re-exported so `crate::osv::matching::check_version_affected` and
// `parse_version` keep their paths for every caller.
pub(crate) use fourda_depengine::range::{check_version_affected, parse_version};
// The matcher's name and placeholder rules (C0.2), under their old paths.
#[cfg(test)]
use super::fix_target;
#[cfg(test)]
use fourda_depengine::matching::{highest_listed_fix, is_npm_security_holding};
pub(crate) use fourda_depengine::matching::{package_key, pep503_name};

/// Get all advisories that match the user's installed dependencies.
/// Merges deps from both `user_dependencies` (user-curated) and
/// `project_dependencies` (ACE-scanned) to ensure coverage.
/// Version matching is attempted for SEMVER ranges; conservative (assume affected)
/// fallback for non-semver or unparseable versions. A crates.io copy whose
/// build gates off every file the advisory names is not matched (AD-051).
pub fn get_matched_advisories(db: &Database) -> Result<Vec<MatchedAdvisory>> {
    get_matched_advisories_with_not_compiled(db).map(|(matches, _)| matches)
}

/// [`get_matched_advisories`], plus the copies it left out because their
/// build does not compile the code the advisory names.
pub(crate) fn get_matched_advisories_with_not_compiled(
    db: &Database,
) -> Result<(Vec<MatchedAdvisory>, Vec<NotCompiledMatch>)> {
    let advisories = db
        .get_all_osv_advisories()
        .map_err(|e| FourDaError::Internal(format!("Failed to read OSV advisories: {e}")))?;

    let mut deps = db
        .get_auditable_user_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read user dependencies: {e}")))?;

    let scanned = db
        .get_auditable_scanned_dependencies()
        .map_err(|e| FourDaError::Internal(format!("Failed to read scanned dependencies: {e}")))?;

    tracing::debug!(
        target: "4da::osv",
        user_deps = deps.len(),
        scanned_deps = scanned.len(),
        advisories = advisories.len(),
        "OSV matching: auditable dep counts"
    );

    // Merge scanned deps, deduped by (package_name, project_path, ecosystem):
    // a user row wins over the scan's.
    let mut seen_deps: std::collections::HashSet<(String, String, String)> = deps
        .iter()
        .map(|d| {
            (
                package_key(&d.package_name, &d.ecosystem),
                d.project_path.replace('\\', "/").to_lowercase(),
                normalize_ecosystem(&d.ecosystem).to_string(),
            )
        })
        .collect();

    for dep in scanned {
        let key = (
            package_key(&dep.package_name, &dep.ecosystem),
            dep.project_path.replace('\\', "/").to_lowercase(),
            normalize_ecosystem(&dep.ecosystem).to_string(),
        );
        if seen_deps.insert(key) {
            deps.push(dep);
        }
    }

    if advisories.is_empty() || deps.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    // The inventory: the resolved rows, then every installed copy of the
    // multi-version inventory (Phase 92). A failed copy read degrades to the
    // resolved rows alone.
    let mut inventory: Vec<Package> = deps.iter().map(resolved_row).collect();
    match db.get_all_dependency_instances() {
        Ok(rows) => inventory.extend(rows.into_iter().map(|r| Package {
            ecosystem: r.ecosystem,
            project: r.project_path,
            name: r.package_name,
            version: Some(r.version),
            is_direct: r.is_direct,
            is_dev: r.is_dev,
            primary: false,
        })),
        Err(e) => tracing::warn!(
            target: "4da::osv",
            error = %e,
            "OSV matching: dependency_instances read failed — falling back to collapsed versions"
        ),
    }

    let records: Vec<AdvisoryRecord> = advisories.iter().map(advisory_record).collect();
    let mut reach = ReachFilter::default();
    // Releases each package's registry withdrew (FX-H5): never a target.
    let registry = super::release_facts::load(db);
    let findings = match_inventory_with(&inventory, &records, &registry, |index, _, copies| {
        reach.retain_compiled(db, &advisories[index], copies);
    });

    let mut matches: Vec<MatchedAdvisory> = findings
        .into_iter()
        .map(|f| {
            let advisory = &advisories[f.record];
            MatchedAdvisory {
                advisory_id: advisory.advisory_id.clone(),
                summary: advisory.summary.clone(),
                details: advisory.details.clone(),
                package_name: advisory.package_name.clone(),
                ecosystem: advisory.ecosystem.clone(),
                installed_version: f.installed_version,
                fixed_version: f.fixed_version,
                severity_type: advisory.severity_type.clone(),
                cvss_score: advisory.cvss_score,
                source_url: advisory.source_url.clone(),
                is_version_confirmed: f.is_version_confirmed,
                project_paths: f.project_paths,
                published_at: advisory.published_at.clone(),
                dependency_instances: f.dependency_instances,
                aliases: advisory.aliases.clone(),
                severity_label: advisory.severity_label.clone(),
            }
        })
        .collect();

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

    let confirmed = matches.iter().filter(|m| m.is_version_confirmed).count();
    tracing::debug!(
        target: "4da::osv",
        total = matches.len(),
        confirmed = confirmed,
        conservative = matches.len() - confirmed,
        not_compiled = reach.excluded.len(),
        "OSV matching: final results"
    );

    let mut excluded = reach.excluded;
    excluded.sort_by(|a, b| {
        (&a.advisory_id, &a.project_path, &a.installed_version).cmp(&(
            &b.advisory_id,
            &b.project_path,
            &b.installed_version,
        ))
    });
    excluded.dedup();
    Ok((matches, excluded))
}

/// A resolved `user_dependencies` / `project_dependencies` row as the
/// engine's inventory row.
pub(crate) fn resolved_row(dep: &StoredDependency) -> Package {
    Package {
        ecosystem: dep.ecosystem.clone(),
        project: dep.project_path.clone(),
        name: dep.package_name.clone(),
        version: dep.version.clone(),
        is_direct: dep.is_direct,
        is_dev: dep.is_dev,
        primary: true,
    }
}

/// A stored mirror row as the engine's advisory record.
pub(crate) fn advisory_record(advisory: &StoredAdvisory) -> AdvisoryRecord {
    AdvisoryRecord {
        advisory_id: advisory.advisory_id.clone(),
        package_name: advisory.package_name.clone(),
        ecosystem: advisory.ecosystem.clone(),
        affected_ranges: advisory.affected_ranges.clone(),
        fixed_versions: advisory.fixed_versions.clone(),
    }
}

/// Count matched advisories without building the full result.
pub fn count_matches(db: &Database) -> Result<usize> {
    get_matched_advisories(db).map(|m| m.len())
}

/// Normalize an ecosystem name to its OSV identifier for matching. Alias
/// recognition is centralized in [`crate::ecosystem::Ecosystem`] (the engine
/// crate's copy is held to it by `depengine_parity_tests`); unknown
/// ecosystems pass through unchanged.
fn normalize_ecosystem(eco: &str) -> &str {
    crate::ecosystem::Ecosystem::parse(eco).map_or(eco, |e| e.osv_name())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "matching_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "matching_audit_tests.rs"]
mod audit_tests;
