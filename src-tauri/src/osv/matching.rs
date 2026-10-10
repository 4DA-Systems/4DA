// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV matching — cross-references stored advisories against user dependencies.
//!
//! Joins osv_advisories with user_dependencies and runs semver version checks
//! to produce verified MatchedAdvisory results (Tier 1 intelligence).

use std::collections::HashMap;

use crate::db::Database;
use crate::error::{FourDaError, Result};
use semver::Version;

use super::fix_target;
use super::reachability::ReachFilter;
use super::types::{MatchedAdvisory, MatchedDependency, NotCompiledMatch, Range};
use super::version_order::{in_window, Lower, OrderedVersion, Upper};

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
    let mut clean_cache: HashMap<(String, String, String), Option<String>> = HashMap::new();
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
                let (is_affected, confirmed) =
                    check_version_affected(version.as_deref(), &advisory.affected_ranges);
                if is_affected {
                    // Targets are facts about THIS copy (Phase 1.1): its own
                    // line's fix, and the version clearing every advisory.
                    let (fixed_version, clean_version) = match version.as_deref() {
                        Some(v) if confirmed => {
                            let clean = clean_cache
                                .entry((key.0.clone(), key.1.clone(), v.to_string()))
                                .or_insert_with(|| {
                                    package_ranges
                                        .get(&key)
                                        .and_then(|ranges| fix_target::clean_version(v, ranges))
                                })
                                .clone();
                            (
                                fix_target::fix_for_version(v, &advisory.affected_ranges),
                                clean,
                            )
                        }
                        _ => (None, None),
                    };
                    dependency_instances.push(MatchedDependency {
                        project_path: normalize_project_path(&dep.project_path),
                        installed_version: version,
                        is_direct,
                        is_dev,
                        is_version_confirmed: confirmed,
                        fixed_version,
                        clean_version,
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

/// Check if a user's version is affected by the advisory's ranges.
/// Returns (is_affected, is_confirmed).
/// - is_affected: true if the version falls within affected ranges (or conservative fallback)
/// - is_confirmed: true only if we could definitively verify via semver
pub(crate) fn check_version_affected(
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

    let Some(user) = OrderedVersion::parse(user_ver_str) else {
        return (true, false); // Can't parse user version → conservative
    };

    // A bound the matcher cannot parse (or compare with this version) leaves
    // its window undecided. Reading that as "not in any window" answered
    // "confirmed NOT affected" about a window it never evaluated
    // (dependency-handoff Phase 1.4).
    let mut undecided = false;
    for range in &ranges {
        if range.range_type == super::types::ENUMERATED_RANGE {
            if enumerated_contains(&user, user_ver_str, range) {
                return (true, true);
            }
            continue;
        }
        if range.range_type != "SEMVER" && range.range_type != "ECOSYSTEM" {
            continue;
        }
        match range_contains(&user, range) {
            Some(true) => return (true, true),
            Some(false) => {}
            None => undecided = true,
        }
    }

    if undecided {
        // Conservative, unconfirmed: the same answer as unparseable ranges JSON.
        return (true, false);
    }
    // Went through all ranges, version not in any affected window
    (false, true)
}

/// Whether an advisory's enumerated `versions` list (stored as an
/// [`super::types::ENUMERATED_RANGE`] range) names this version — exactly, or
/// as the same version under the ecosystem's ordering (`19.9` = `19.9.0`).
fn enumerated_contains(user: &OrderedVersion, raw: &str, range: &Range) -> bool {
    range
        .events
        .iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .any(|listed| {
            listed == raw.trim()
                || OrderedVersion::parse(listed)
                    .and_then(|v| user.compare(&v))
                    .is_some_and(|o| o.is_eq())
        })
}

/// Whether `user` sits in any window of one range: `Some(true)` inside one,
/// `Some(false)` outside all of them, `None` when a window could not be
/// decided (an unreadable bound, or one no ordering compares with `user`).
fn range_contains(user: &OrderedVersion, range: &Range) -> Option<bool> {
    let mut undecided = false;
    let mut lower: Option<Lower> = None;
    let mut note = |verdict: Option<bool>| match verdict {
        Some(true) => true,
        Some(false) => false,
        None => {
            undecided = true;
            false
        }
    };
    for obj in range.events.iter().flatten().filter_map(|e| e.as_object()) {
        if let Some(intro) = obj.get("introduced").and_then(|v| v.as_str()) {
            lower = Lower::parse(intro);
            if lower.is_none() && note(None) {
                return Some(true);
            }
        }
        let upper = obj
            .get("fixed")
            .map(|v| (v, false))
            .or_else(|| obj.get("last_affected").map(|v| (v, true)));
        if let Some((bound, inclusive)) = upper {
            let bound = bound.as_str().unwrap_or("");
            if let (Some(low), false) = (lower.as_ref(), is_unknown_bound(bound)) {
                let verdict = OrderedVersion::parse(bound).and_then(|version| {
                    in_window(
                        user,
                        low,
                        Some(Upper {
                            version: &version,
                            inclusive,
                        }),
                    )
                });
                if note(verdict) {
                    return Some(true);
                }
            }
            lower = None;
        }
    }
    // introduced with no fix → every version from introduced onward
    if let Some(low) = lower.as_ref() {
        if note(in_window(user, low, None)) {
            return Some(true);
        }
    }
    (!undecided).then_some(false)
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

/// Whether an OSV version boundary is an unknown ("not available") sentinel rather than a
/// concrete version. OSV's PYSEC import appends "-NA" when the exact affected boundary is
/// unknown; `semver` parses "2.5.0-NA" as a 2.5.0 prerelease, so a naive comparison would
/// over-match (e.g. torch 2.3.0 <= "2.5.0-NA" => affected) — yet OSV's own `/v1/query`
/// matcher does NOT place a version inside such a range. Treat it as no usable bound so the
/// engine stays conservative and consistent with OSV (verified 2026-06-18 via the ledger's
/// external accuracy audit).
pub(super) fn is_unknown_bound(v: &str) -> bool {
    let t = v.trim();
    t.ends_with("-NA") || t.ends_with("-na")
}

/// Parse a version string, handling common non-semver formats.
pub(super) fn parse_version(ver: &str) -> Option<Version> {
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

    // Leading zeros in the numeric core ("17.06.0-ce", a Docker-style bound a
    // Go advisory carries): semver rejects them, every ecosystem reads them
    // as the number. Left unparsed, the window was undecided and the copy
    // stayed an unconfirmed match forever.
    let split = v.find(['-', '+']).unwrap_or(v.len());
    let (core, rest) = v.split_at(split);
    let numbers: Vec<&str> = core.split('.').collect();
    if numbers.len() == 3
        && numbers
            .iter()
            .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        let trimmed: Vec<&str> = numbers
            .iter()
            .map(|n| match n.trim_start_matches('0') {
                "" => "0",
                t => t,
            })
            .collect();
        return Version::parse(&format!("{}{rest}", trimmed.join("."))).ok();
    }

    None
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
