// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV matching — cross-references stored advisories against user dependencies.
//!
//! Joins osv_advisories with user_dependencies and runs semver version checks
//! to produce verified MatchedAdvisory results (Tier 1 intelligence).

use std::collections::HashMap;

use crate::db::Database;
use crate::error::{FourDaError, Result};

use super::fix_target;
use super::reachability::ReachFilter;
use super::types::{MatchedAdvisory, MatchedDependency, NotCompiledMatch};

// The pure affected-window evaluation lives in the engine crate (single-engine
// C0.1). Re-exported so `crate::osv::matching::check_version_affected` and
// `parse_version` keep their paths for every caller.
pub(crate) use fourda_depengine::range::{check_version_affected, parse_version};

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

    // Merge scanned deps, deduped by (package_name, project_path, ecosystem)
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

    // Index deps by (package_name_lower, ecosystem_normalized) for fast lookup
    let mut dep_index: HashMap<(String, String), Vec<&crate::db::StoredDependency>> =
        HashMap::new();
    for dep in &deps {
        let key = (
            package_key(&dep.package_name, &dep.ecosystem),
            normalize_ecosystem(&dep.ecosystem).to_string(),
        );
        dep_index.entry(key).or_default().push(dep);
    }

    // Multi-version inventory (Phase 92). The collapsed dep tables keep ONE
    // version per (project, package), so a project holding both a vulnerable and
    // a patched copy of a package surfaces only the survivor — a false negative
    // when the survivor is the patched one. `dependency_instances` retains every
    // installed version. Indexed by (project_norm, package_lower, ecosystem_osv);
    // per dep below we check the UNION of the collapsed version and all instance
    // versions. Union (never replacement) is strictly additive — it can only ADD
    // a missing affected version, and degrades to today's behavior wherever
    // instances are not yet populated (pre-Phase-92 scans).
    let mut instance_index: HashMap<(String, String, String), Vec<(String, bool, bool)>> =
        HashMap::new();
    match db.get_all_dependency_instances() {
        Ok(rows) => {
            for r in rows {
                let key = (
                    normalize_project_path(&r.project_path),
                    package_key(&r.package_name, &r.ecosystem),
                    normalize_ecosystem(&r.ecosystem).to_string(),
                );
                instance_index
                    .entry(key)
                    .or_default()
                    .push((r.version, r.is_direct, r.is_dev));
            }
        }
        Err(e) => tracing::warn!(
            target: "4da::osv",
            error = %e,
            "OSV matching: dependency_instances read failed — falling back to collapsed versions"
        ),
    }

    // Every known advisory's ranges per package: a copy's clean version must
    // clear ALL of them, not only the ones it matched today.
    let mut package_ranges: HashMap<(String, String), Vec<&Option<String>>> = HashMap::new();
    for advisory in &advisories {
        package_ranges
            .entry((
                package_key(&advisory.package_name, &advisory.ecosystem),
                normalize_ecosystem(&advisory.ecosystem).to_string(),
            ))
            .or_default()
            .push(&advisory.affected_ranges);
    }
    let mut walk_cache: HashMap<(String, String, String), Option<fix_target::WalkTarget>> =
        HashMap::new();
    let mut reach = ReachFilter::default();

    let mut matches: Vec<MatchedAdvisory> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for advisory in &advisories {
        let key = (
            package_key(&advisory.package_name, &advisory.ecosystem),
            normalize_ecosystem(&advisory.ecosystem).to_string(),
        );

        let dep_entries = match dep_index.get(&key) {
            Some(entries) => entries,
            None => continue,
        };

        // Check each dependency instance (could be in multiple projects, and —
        // via the multi-version inventory — multiple versions per project).
        let mut dependency_instances = Vec::new();
        for dep in dep_entries {
            // Union of every installed instance version for this (project,
            // package) with the collapsed survivor, deduped by version string.
            // Per-instance is_direct/is_dev is used where the inventory has it
            // (a version can be direct in one place, transitive in another).
            let inst_key = (
                normalize_project_path(&dep.project_path),
                package_key(&dep.package_name, &dep.ecosystem),
                normalize_ecosystem(&dep.ecosystem).to_string(),
            );
            let mut candidates: Vec<(Option<String>, bool, bool)> = Vec::new();
            let mut seen_versions: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            if let Some(instances) = instance_index.get(&inst_key) {
                for (ver, is_direct, is_dev) in instances {
                    if seen_versions.insert(ver.clone()) {
                        candidates.push((Some(ver.clone()), *is_direct, *is_dev));
                    }
                }
            }
            // Always include the collapsed survivor: it covers projects without
            // instance coverage yet, and a version-less row the parser could not
            // resolve (conservative match). Skip only if an instance already
            // carried that exact version.
            let survivor_is_dup = dep
                .version
                .as_ref()
                .is_some_and(|v| seen_versions.contains(v));
            if !survivor_is_dup {
                candidates.push((dep.version.clone(), dep.is_direct, dep.is_dev));
            }

            for (version, is_direct, is_dev) in candidates {
                if is_npm_security_holding(
                    &advisory.advisory_id,
                    &advisory.ecosystem,
                    version.as_deref(),
                ) {
                    continue;
                }
                let (is_affected, confirmed) = check_version_affected(
                    &advisory.ecosystem,
                    version.as_deref(),
                    &advisory.affected_ranges,
                );
                if is_affected {
                    // Targets are facts about THIS copy (Phase 1.1): its own
                    // line's fix, and where the walk over every advisory of
                    // the package lands (clean, or clearing every advisory
                    // that has a way out).
                    let (fixed_version, walk) = match version.as_deref() {
                        Some(v) if confirmed => {
                            let walk = walk_cache
                                .entry((key.0.clone(), key.1.clone(), v.to_string()))
                                .or_insert_with(|| {
                                    package_ranges.get(&key).and_then(|ranges| {
                                        fix_target::walk_target(&advisory.ecosystem, v, ranges)
                                    })
                                })
                                .clone();
                            (
                                fix_target::fix_for_version(
                                    &advisory.ecosystem,
                                    v,
                                    &advisory.affected_ranges,
                                ),
                                walk,
                            )
                        }
                        _ => (None, None),
                    };
                    let clean_version = walk
                        .as_ref()
                        .filter(|w| w.clears_all)
                        .map(|w| w.version.clone());
                    let target_version = walk.map(|w| w.version);
                    dependency_instances.push(MatchedDependency {
                        project_path: normalize_project_path(&dep.project_path),
                        installed_version: version,
                        is_direct,
                        is_dev,
                        is_version_confirmed: confirmed,
                        fixed_version,
                        clean_version,
                        target_version,
                    });
                }
            }
        }

        reach.retain_compiled(db, advisory, &mut dependency_instances);
        if dependency_instances.is_empty() {
            continue;
        }

        dependency_instances.sort_by(|a, b| {
            b.is_version_confirmed
                .cmp(&a.is_version_confirmed)
                .then_with(|| b.is_direct.cmp(&a.is_direct))
                .then_with(|| a.is_dev.cmp(&b.is_dev))
                .then_with(|| a.project_path.cmp(&b.project_path))
        });
        dependency_instances.dedup_by(|a, b| {
            a.project_path == b.project_path
                && a.installed_version == b.installed_version
                && a.is_direct == b.is_direct
                && a.is_dev == b.is_dev
        });

        let any_version_confirmed = dependency_instances
            .iter()
            .any(|instance| instance.is_version_confirmed);
        let representative_version = dependency_instances
            .iter()
            .find(|instance| instance.is_version_confirmed)
            .or_else(|| dependency_instances.first())
            .and_then(|instance| instance.installed_version.clone());

        // A confirmed advisory must not claim conservative/unverified projects
        // as affected. If no instance can be confirmed, retain the conservative
        // paths for diagnostics but Preemption will not promote the match.
        let mut project_paths: Vec<String> = dependency_instances
            .iter()
            .filter(|instance| !any_version_confirmed || instance.is_version_confirmed)
            .map(|instance| instance.project_path.clone())
            .collect();
        project_paths.sort();
        project_paths.dedup();

        let dedup_key = format!(
            "{}:{}:{}",
            advisory.advisory_id,
            advisory.package_name,
            normalize_ecosystem(&advisory.ecosystem)
        );
        if !seen.insert(dedup_key) {
            continue;
        }

        // Machine-wide (see `MatchedAdvisory::fixed_version`): the fix for the
        // line each confirmed copy is on (highest across copies), else the
        // fix that clears every listed range.
        let fixed_version = dependency_instances
            .iter()
            .filter(|instance| instance.is_version_confirmed)
            .filter_map(|instance| instance.fixed_version.clone())
            .filter_map(|fix| parse_version(&fix).map(|parsed| (parsed, fix)))
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, fix)| fix)
            .or_else(|| highest_listed_fix(&advisory.fixed_versions));

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
            dependency_instances,
            aliases: advisory.aliases.clone(),
            severity_label: advisory.severity_label.clone(),
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

fn normalize_project_path(path: &str) -> String {
    path.replace('\\', "/")
        .to_lowercase()
        .trim_end_matches('/')
        .to_string()
}

/// Count matched advisories without building the full result.
pub fn count_matches(db: &Database) -> Result<usize> {
    get_matched_advisories(db).map(|m| m.len())
}

/// npm's "Security holding package": after a malicious package is taken
/// down, npm republishes the name as an empty `0.0.1-security` placeholder.
/// A malicious-package advisory (`MAL-`) covers every version ("introduced
/// 0"), so the placeholder read as installed malware (4da-ledger's
/// transitive `fs@0.0.1-security`, 2026-09-26). The placeholder contains no
/// code, so no MAL advisory applies to it.
pub(crate) fn is_npm_security_holding(
    advisory_id: &str,
    ecosystem: &str,
    version: Option<&str>,
) -> bool {
    advisory_id.starts_with("MAL-")
        && normalize_ecosystem(ecosystem) == "npm"
        && version.is_some_and(|v| v.trim().ends_with("-security"))
}

/// Highest semver among an advisory's listed fixes: the version that clears
/// every range. Used when no installed copy could be placed in a window.
fn highest_listed_fix(fixed_versions_json: &Option<String>) -> Option<String> {
    let versions: Vec<String> = serde_json::from_str(fixed_versions_json.as_deref()?).ok()?;
    versions
        .into_iter()
        .filter_map(|v| parse_version(&v).map(|parsed| (parsed, v)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, v)| v)
}

/// Normalize an ecosystem name to its OSV identifier for matching. Alias
/// recognition (registry name, ACE language name, csharp/php/dart, etc.) is
/// centralized in [`crate::ecosystem::Ecosystem`]; unknown ecosystems pass
/// through unchanged.
fn normalize_ecosystem(eco: &str) -> &str {
    crate::ecosystem::Ecosystem::parse(eco).map_or(eco, |e| e.osv_name())
}

/// The name a registry treats as one package: PyPI compares PEP 503
/// normalized names (`typing_extensions` = `Typing-Extensions` =
/// `typing.extensions`), every other ecosystem here case-insensitively. A
/// lockfile and an advisory routinely spell a Python package differently;
/// keying by the lowercase spelling alone missed the match.
pub(crate) fn package_key(name: &str, ecosystem: &str) -> String {
    if normalize_ecosystem(ecosystem) == "PyPI" {
        pep503_name(name)
    } else {
        name.to_lowercase()
    }
}

/// PEP 503: lowercase, every run of `-`, `_` and `.` collapsed to one `-`.
pub(crate) fn pep503_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut in_separator = false;
    for c in name.trim().chars() {
        if matches!(c, '-' | '_' | '.') {
            if !in_separator {
                out.push('-');
            }
            in_separator = true;
        } else {
            out.extend(c.to_lowercase());
            in_separator = false;
        }
    }
    out
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
