// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Inventory matching: which installed copies each stored advisory affects,
//! confirmed or conservative, with each copy's own fix targets. The pure half
//! of the app's `osv/matching.rs` (single-engine C0.2): the app reads
//! `osv_advisories`, `user_dependencies`, `project_dependencies` and
//! `dependency_instances`, turns them into [`Package`]s and
//! [`AdvisoryRecord`]s, and calls [`match_inventory_with`].
//!
//! The one step that needs the host is reachability (AD-051: is the code an
//! advisory names compiled into this build?). It reads cargo's resolved
//! features and the crate's source, so it is a host hook: the `retain`
//! callback of [`match_inventory_with`] runs on each advisory's affected
//! copies before they are grouped. [`match_inventory`] keeps every copy.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::ecosystem::normalize as normalize_ecosystem;
use crate::fix_target::{installable_fix, installable_line_fix, walk_target_in, WalkOutcome};
use crate::osv::AdvisoryRecord;
use crate::range::{check_version_affected, parse_version};
use crate::registry::{RegistryIndex, RegistryReleases};

/// Installed copies by (project, package key, OSV ecosystem): each copy's
/// (version, is_direct, is_dev).
type CopyIndex = HashMap<(String, String, String), Vec<(String, bool, bool)>>;

/// One row of the installed inventory.
///
/// The app's inventory has two layers, and the matcher reads both:
/// - `primary: true`: the project's resolved row for a package (one per
///   project and package; `user_dependencies` / `project_dependencies`).
///   These decide WHICH (project, package) pairs are audited.
/// - `primary: false`: one installed copy from the multi-version inventory
///   (`dependency_instances`). Every copy of an audited (project, package) is
///   checked, so a project holding a vulnerable and a patched copy surfaces
///   the vulnerable one even when the resolved row names the patched one.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Package {
    /// Any alias the app stores (`rust`, `javascript`, `crates.io`, ...).
    pub ecosystem: String,
    /// The project directory the package belongs to.
    pub project: String,
    pub name: String,
    /// `None` only on a resolved row whose version could not be read (a
    /// conservative match); an installed copy always has one.
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub is_direct: bool,
    #[serde(default)]
    pub is_dev: bool,
    /// Required (no default): a caller that forgets it would otherwise mark
    /// every row an installed copy, audit nothing, and read as all-clear.
    pub primary: bool,
}

/// One installed dependency instance affected by an advisory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatchedDependency {
    pub project_path: String,
    pub installed_version: Option<String>,
    pub is_direct: bool,
    pub is_dev: bool,
    pub is_version_confirmed: bool,
    /// This advisory's fix on the release line THIS copy is on (`None` when
    /// the window has no fix, or the copy could not be placed in a window).
    #[serde(default)]
    pub fixed_version: Option<String>,
    /// The lowest version reachable from this copy by successive fixes that
    /// no known advisory of the package affects — the version that clears
    /// everything, on this copy's own line where one exists. `None` when the
    /// path runs into a window no known release leaves (open-ended, or a
    /// `last_affected` bound with nothing published above it in the data), or
    /// an advisory the matcher cannot evaluate (no clean version can then be
    /// claimed).
    #[serde(default)]
    pub clean_version: Option<String>,
    /// Where that walk lands when it cannot clear everything: the lowest
    /// version reachable by successive fixes that no advisory WITH a way out
    /// affects (equal to `clean_version` when that is set). It is never
    /// inside a window a later release leaves; only advisories no release
    /// fixes stay open there. `None` when nothing has a way out, or the walk
    /// could not evaluate an advisory.
    #[serde(default)]
    pub target_version: Option<String>,
    /// Releases the registry withdrew (yanked, deprecated) that this copy's
    /// line fix or walk would otherwise have named; none of the fields above
    /// ever holds one. With no target, a non-empty list means the fix exists
    /// but was withdrawn ("no installable fix", not "no fix published").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub withheld: Vec<String>,
    /// Parity decision 2: the advisories every fixing release of this copy
    /// brings in (no way out, and the installed version does not have them).
    /// Sorted and deduped. Non-empty means no target is named: neither `target_version` nor the
    /// line fix (`fourda_depengine::fix_target::WalkOutcome::introduces`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub introduces: Vec<String>,
}

/// One advisory row that affects at least one installed copy.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Finding {
    /// Index of the advisory in the `records` slice the matcher was given.
    pub record: usize,
    /// A version-confirmed copy's version, else the first copy's.
    pub installed_version: Option<String>,
    /// Machine-wide: the highest line fix across every confirmed copy, else
    /// the highest listed fix (the one that clears every range).
    pub fixed_version: Option<String>,
    /// At least one copy is confirmed inside the affected range.
    pub is_version_confirmed: bool,
    /// The projects the advisory applies to: only the confirmed ones when any
    /// copy is confirmed.
    pub project_paths: Vec<String>,
    /// Every affected copy, confirmed first, then direct, then runtime.
    pub dependency_instances: Vec<MatchedDependency>,
}

/// Why `inventory` cannot be matched meaningfully, if it cannot: installed
/// copies with no resolved (`primary`) row. Only primary rows decide which
/// (project, package) pairs are audited, so such an inventory matches
/// nothing and would read as a silent all-clear. Hosts that build their own
/// inventory (the JSON API) must refuse it; the app always supplies both
/// layers.
pub fn inventory_problem(inventory: &[Package]) -> Option<String> {
    let copies = inventory.iter().filter(|p| !p.primary).count();
    let primary = inventory.len() - copies;
    (copies > 0 && primary == 0).then(|| {
        format!(
            "inventory has {copies} installed cop{} and no primary (resolved) rows:              nothing would be audited; mark each project's resolved row primary:true",
            if copies == 1 { "y" } else { "ies" }
        )
    })
}

/// [`match_inventory_with`] without registry facts or a host hook: every
/// affected copy counts.
pub fn match_inventory(inventory: &[Package], records: &[AdvisoryRecord]) -> Vec<Finding> {
    match_inventory_with(inventory, records, &RegistryIndex::new(), |_, _, _| {})
}

/// Match `records` against `inventory`. Findings come in `records` order (one
/// per advisory row that affects a copy; a repeated (id, package, ecosystem)
/// row is reported once). Version matching is confirmed for parseable
/// versions inside parseable windows; a copy with no version, or an advisory
/// whose window cannot be read, is a conservative (unconfirmed) match.
///
/// `registry` holds what each package's registry says about its releases
/// (keyed by [`package_key`] and OSV ecosystem); no copy's target, line fix
/// or finding fix is ever a release it withdrew. A package with no entry is
/// matched on the advisory data alone.
///
/// `retain(index, record, copies)` sees each advisory's affected copies
/// (`index` into `records`) before they are grouped and may drop some (the
/// app's reachability filter).
pub fn match_inventory_with<F>(
    inventory: &[Package],
    records: &[AdvisoryRecord],
    registry: &RegistryIndex,
    mut retain: F,
) -> Vec<Finding>
where
    F: FnMut(usize, &AdvisoryRecord, &mut Vec<MatchedDependency>),
{
    let deps: Vec<&Package> = inventory.iter().filter(|p| p.primary).collect();
    if records.is_empty() || deps.is_empty() {
        return Vec::new();
    }

    // Resolved rows by (package key, OSV ecosystem).
    let mut dep_index: HashMap<(String, String), Vec<&Package>> = HashMap::new();
    for dep in &deps {
        let key = (
            package_key(&dep.name, &dep.ecosystem),
            normalize_ecosystem(&dep.ecosystem).to_string(),
        );
        dep_index.entry(key).or_default().push(dep);
    }

    // Installed copies by (project, package key, OSV ecosystem). Per resolved
    // row the matcher checks the UNION of its version and every copy's:
    // strictly additive, so it degrades to the resolved rows where the
    // multi-version inventory is empty.
    let mut instance_index: CopyIndex = HashMap::new();
    for copy in inventory.iter().filter(|p| !p.primary) {
        let Some(version) = &copy.version else {
            continue;
        };
        instance_index
            .entry((
                normalize_project_path(&copy.project),
                package_key(&copy.name, &copy.ecosystem),
                normalize_ecosystem(&copy.ecosystem).to_string(),
            ))
            .or_default()
            .push((version.clone(), copy.is_direct, copy.is_dev));
    }

    // Every known advisory's ranges per package: a copy's clean version must
    // clear ALL of them, not only the ones it matched today.
    let mut package_ranges: HashMap<(String, String), Vec<&Option<String>>> = HashMap::new();
    let mut package_ids: HashMap<(String, String), Vec<&str>> = HashMap::new();
    for record in records {
        package_ids
            .entry((
                package_key(&record.package_name, &record.ecosystem),
                normalize_ecosystem(&record.ecosystem).to_string(),
            ))
            .or_default()
            .push(&record.advisory_id);
        package_ranges
            .entry((
                package_key(&record.package_name, &record.ecosystem),
                normalize_ecosystem(&record.ecosystem).to_string(),
            ))
            .or_default()
            .push(&record.affected_ranges);
    }
    let mut walk_cache: HashMap<(String, String, String), WalkOutcome> = HashMap::new();
    let no_facts = RegistryReleases::default();

    let mut findings: Vec<Finding> = Vec::new();
    let mut seen = HashSet::new();

    for (index, record) in records.iter().enumerate() {
        let key = (
            package_key(&record.package_name, &record.ecosystem),
            normalize_ecosystem(&record.ecosystem).to_string(),
        );
        let Some(dep_entries) = dep_index.get(&key) else {
            continue;
        };
        let facts = registry.get(&key).unwrap_or(&no_facts);

        let mut copies = Vec::new();
        for dep in dep_entries {
            for (version, is_direct, is_dev) in candidates(dep, &instance_index) {
                if is_npm_security_holding(&record.ecosystem, version.as_deref()) {
                    continue;
                }
                let (is_affected, confirmed) = check_version_affected(
                    &record.ecosystem,
                    version.as_deref(),
                    &record.affected_ranges,
                );
                if !is_affected {
                    continue;
                }
                // Targets are facts about THIS copy: its own line's fix, and
                // where the walk over every advisory of the package lands
                // (clean, or clearing every advisory that has a way out).
                let targets = match version.as_deref() {
                    Some(v) if confirmed => {
                        let walk = walk_cache
                            .entry((key.0.clone(), key.1.clone(), v.to_string()))
                            .or_insert_with(|| {
                                package_ranges
                                    .get(&key)
                                    .map(|ranges| {
                                        walk_target_in(&record.ecosystem, v, ranges, facts)
                                    })
                                    .unwrap_or_default()
                            })
                            .clone();
                        let ids = package_ids.get(&key).map_or(&[][..], Vec::as_slice);
                        CopyTargets::new(record, v, walk, facts, ids)
                    }
                    _ => CopyTargets::default(),
                };
                copies.push(MatchedDependency {
                    project_path: normalize_project_path(&dep.project),
                    installed_version: version,
                    is_direct,
                    is_dev,
                    is_version_confirmed: confirmed,
                    fixed_version: targets.fixed,
                    clean_version: targets.clean,
                    target_version: targets.target,
                    withheld: targets.withheld,
                    introduces: targets.introduces,
                });
            }
        }

        retain(index, record, &mut copies);
        if copies.is_empty() {
            continue;
        }

        copies.sort_by(|a, b| {
            b.is_version_confirmed
                .cmp(&a.is_version_confirmed)
                .then_with(|| b.is_direct.cmp(&a.is_direct))
                .then_with(|| a.is_dev.cmp(&b.is_dev))
                .then_with(|| a.project_path.cmp(&b.project_path))
        });
        copies.dedup_by(|a, b| {
            a.project_path == b.project_path
                && a.installed_version == b.installed_version
                && a.is_direct == b.is_direct
                && a.is_dev == b.is_dev
        });

        let any_version_confirmed = copies.iter().any(|c| c.is_version_confirmed);
        let installed_version = copies
            .iter()
            .find(|c| c.is_version_confirmed)
            .or_else(|| copies.first())
            .and_then(|c| c.installed_version.clone());

        // A confirmed advisory must not claim conservative/unverified projects
        // as affected. If no copy can be confirmed, the conservative paths are
        // kept for diagnostics.
        let mut project_paths: Vec<String> = copies
            .iter()
            .filter(|c| !any_version_confirmed || c.is_version_confirmed)
            .map(|c| c.project_path.clone())
            .collect();
        project_paths.sort();
        project_paths.dedup();

        let dedup_key = format!(
            "{}:{}:{}",
            record.advisory_id,
            record.package_name,
            normalize_ecosystem(&record.ecosystem)
        );
        if !seen.insert(dedup_key) {
            continue;
        }

        let fixed_version = copies
            .iter()
            .filter(|c| c.is_version_confirmed)
            .filter_map(|c| c.fixed_version.clone())
            .filter_map(|fix| parse_version(&fix).map(|parsed| (parsed, fix)))
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, fix)| fix)
            .or_else(|| {
                highest_listed_fix(&record.fixed_versions).and_then(|fix| {
                    installable_fix(&record.ecosystem, &fix, &record.affected_ranges, facts).fix
                })
            });

        findings.push(Finding {
            record: index,
            installed_version,
            fixed_version,
            is_version_confirmed: any_version_confirmed,
            project_paths,
            dependency_instances: copies,
        });
    }
    findings
}

/// One confirmed copy's targets, none of them a release the registry
/// withdrew.
#[derive(Default)]
struct CopyTargets {
    fixed: Option<String>,
    clean: Option<String>,
    target: Option<String>,
    withheld: Vec<String>,
    introduces: Vec<String>,
}

impl CopyTargets {
    /// `ids`: the package's advisory ids, parallel to the walk's ranges.
    fn new(
        record: &AdvisoryRecord,
        installed: &str,
        walk: WalkOutcome,
        facts: &RegistryReleases,
        ids: &[&str],
    ) -> Self {
        let line =
            installable_line_fix(&record.ecosystem, installed, &record.affected_ranges, facts);
        let mut withheld = walk.withheld;
        for w in line.withheld {
            if !withheld.contains(&w) {
                withheld.push(w);
            }
        }
        CopyTargets {
            fixed: line.fix,
            clean: walk
                .target
                .as_ref()
                .filter(|w| w.clears_all)
                .map(|w| w.version.clone()),
            target: walk.target.map(|w| w.version),
            withheld,
            introduces: introduced_ids(&walk.introduces, ids),
        }
    }
}

/// The ids at `indices` of `ids`, sorted and deduped (the order both
/// engines name them in).
fn introduced_ids(indices: &[usize], ids: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = indices
        .iter()
        .filter_map(|&i| ids.get(i))
        .map(|id| (*id).to_string())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The versions checked for one resolved row: every installed copy of that
/// (project, package), deduped by version (first wins, with its own
/// direct/dev labels), then the resolved row's own version unless a copy
/// already carried it. The resolved row covers projects without copy
/// coverage yet and a version-less row (conservative match).
fn candidates(dep: &Package, instance_index: &CopyIndex) -> Vec<(Option<String>, bool, bool)> {
    let inst_key = (
        normalize_project_path(&dep.project),
        package_key(&dep.name, &dep.ecosystem),
        normalize_ecosystem(&dep.ecosystem).to_string(),
    );
    let mut out: Vec<(Option<String>, bool, bool)> = Vec::new();
    let mut seen_versions: HashSet<String> = HashSet::new();
    if let Some(instances) = instance_index.get(&inst_key) {
        for (ver, is_direct, is_dev) in instances {
            if seen_versions.insert(ver.clone()) {
                out.push((Some(ver.clone()), *is_direct, *is_dev));
            }
        }
    }
    let survivor_is_dup = dep
        .version
        .as_ref()
        .is_some_and(|v| seen_versions.contains(v));
    if !survivor_is_dup {
        out.push((dep.version.clone(), dep.is_direct, dep.is_dev));
    }
    out
}

/// A project path as findings carry it: `/` separators, lowercase, no
/// trailing slash.
pub fn normalize_project_path(path: &str) -> String {
    path.replace('\\', "/")
        .to_lowercase()
        .trim_end_matches('/')
        .to_string()
}

/// npm's "Security holding package": after a malicious package is taken
/// down, npm republishes the name as an empty `0.0.1-security` placeholder
/// (the registry describes it as "security holding package"). The advisory
/// that caused the takedown covers every version ("introduced 0"), so the
/// placeholder read as installed malware: 4da-ledger's transitive
/// `fs@0.0.1-security` (MAL-, 2026-09-26), and crossenv `0.0.2-security`
/// under GHSA-c2m4-w5hm-vqjw ("crossenv is malware", `last_affected 1.0.1`,
/// 2026-10-16). The placeholder contains no code, so NO advisory applies to
/// it, whichever database's id it carries. Only npm's own placeholder shape,
/// `0.0.<n>-security`, counts: a publisher's `1.0.0-security` prerelease is
/// real code and stays affected.
pub fn is_npm_security_holding(ecosystem: &str, version: Option<&str>) -> bool {
    normalize_ecosystem(ecosystem) == "npm"
        && version
            .and_then(|v| v.trim().strip_prefix("0.0."))
            .and_then(|v| v.strip_suffix("-security"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Highest semver among an advisory's listed fixes: the version that clears
/// every range. Used when no installed copy could be placed in a window.
pub fn highest_listed_fix(fixed_versions_json: &Option<String>) -> Option<String> {
    let versions: Vec<String> = serde_json::from_str(fixed_versions_json.as_deref()?).ok()?;
    versions
        .into_iter()
        .filter_map(|v| parse_version(&v).map(|parsed| (parsed, v)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, v)| v)
}

/// The name a registry treats as one package: PyPI compares PEP 503
/// normalized names (`typing_extensions` = `Typing-Extensions` =
/// `typing.extensions`), every other ecosystem here case-insensitively.
pub fn package_key(name: &str, ecosystem: &str) -> String {
    if normalize_ecosystem(ecosystem) == "PyPI" {
        pep503_name(name)
    } else {
        name.to_lowercase()
    }
}

/// PEP 503: lowercase, every run of `-`, `_` and `.` collapsed to one `-`.
pub fn pep503_name(name: &str) -> String {
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

#[cfg(test)]
#[path = "matching_tests.rs"]
mod tests;
