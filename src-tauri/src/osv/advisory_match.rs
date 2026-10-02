// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The Preemption matcher's verdict for ONE advisory — what the scorer's
//! security lane reads, so Signal / Brief and Preemption answer "is this
//! build exposed?" from one truth.
//!
//! Before this the scorer judged an advisory item with its own route: only
//! the projects that DECLARE the dependency (`DepMatch::project_paths`, from
//! the manifest scan) were checked against `user_dependencies`. A transitive
//! copy is never declared, so the exposed project was never consulted — live
//! 2026-10-02, GHSA-9pj6-vhgr-3mwh (rmcp, HIGH): the scorer checked victauri's
//! direct rmcp 3.1.2 (fixed in 2.0.0), said "not affected" and capped the item
//! at 0.37, while Preemption — reading every installed copy — listed
//! a sibling bridge app's transitive rmcp 1.7.0 as a HIGH.
//!
//! [`exposure_for_advisory`] reads the SAME inventory as
//! [`super::matching::get_matched_advisories`] — the auditable user and scanned
//! dependency readers (identical filters, sliced to the advisory's package),
//! unioned with every multi-version instance — and decides each copy with the
//! SAME primitive, [`super::matching::check_version_affected`].

use std::collections::HashSet;

use crate::db::{Database, StoredDependency};

use super::matching::{check_version_affected, is_npm_security_holding};
use super::types::StoredAdvisory;

/// One installed copy the matcher placed inside the affected range with a
/// parseable version (the only copies a positive verdict may cite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExposedCopy {
    pub project_path: String,
    pub installed_version: String,
    pub is_direct: bool,
    pub is_dev: bool,
}

/// The matcher's verdict for one mirror advisory against the user's
/// auditable inventory.
#[derive(Debug, Clone, Default)]
pub(crate) struct AdvisoryExposure {
    /// Copies CONFIRMED inside the affected range, worst first (runtime
    /// before dev, direct before transitive).
    pub confirmed: Vec<ExposedCopy>,
    /// Copies the range could not decide (no / unparseable version) — the
    /// matcher's conservative match; never a positive verdict on its own.
    pub unconfirmed: usize,
    /// Installed copies of the package that were evaluated. Zero means the
    /// inventory does not hold the package in this ecosystem — no verdict.
    pub copies_checked: usize,
}

fn osv_ecosystem(eco: &str) -> &str {
    crate::ecosystem::Ecosystem::parse(eco).map_or(eco, |e| e.osv_name())
}

fn norm_path(path: &str) -> String {
    path.replace('\\', "/")
        .to_lowercase()
        .trim_end_matches('/')
        .to_string()
}

/// Run the Preemption matcher's per-copy check for ONE advisory.
pub(crate) fn exposure_for_advisory(db: &Database, advisory: &StoredAdvisory) -> AdvisoryExposure {
    let mut out = AdvisoryExposure::default();
    let Ok((user, scanned)) = db.get_auditable_dependencies_for_package(&advisory.package_name)
    else {
        return out;
    };
    let eco = osv_ecosystem(&advisory.ecosystem).to_string();

    // The matcher's merge: user rows win, scanned rows fill (project, package,
    // ecosystem) keys the user table lacks.
    let mut seen: HashSet<String> = HashSet::new();
    let deps: Vec<StoredDependency> = user
        .into_iter()
        .chain(scanned)
        .filter(|d| osv_ecosystem(&d.ecosystem) == eco)
        .filter(|d| seen.insert(norm_path(&d.project_path)))
        .collect();
    if deps.is_empty() {
        return out;
    }
    let instances = db
        .get_dependency_instances_named(&advisory.package_name)
        .unwrap_or_default();

    let mut confirmed: Vec<ExposedCopy> = Vec::new();
    for dep in &deps {
        let project = norm_path(&dep.project_path);
        // Union of every installed instance of this (project, package) with
        // the collapsed survivor, deduped by version — exactly the matcher's
        // candidate set.
        let mut versions: HashSet<String> = HashSet::new();
        let mut candidates: Vec<(Option<String>, bool, bool)> = instances
            .iter()
            .filter(|i| norm_path(&i.project_path) == project && osv_ecosystem(&i.ecosystem) == eco)
            .filter(|i| versions.insert(i.version.clone()))
            .map(|i| (Some(i.version.clone()), i.is_direct, i.is_dev))
            .collect();
        if !dep.version.as_ref().is_some_and(|v| versions.contains(v)) {
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
            out.copies_checked += 1;
            match check_version_affected(version.as_deref(), &advisory.affected_ranges) {
                (true, true) => confirmed.push(ExposedCopy {
                    project_path: project.clone(),
                    installed_version: version.unwrap_or_default(),
                    is_direct,
                    is_dev,
                }),
                (true, false) => out.unconfirmed += 1,
                (false, _) => {}
            }
        }
    }
    confirmed.sort_by(|a, b| {
        a.is_dev
            .cmp(&b.is_dev)
            .then_with(|| b.is_direct.cmp(&a.is_direct))
            .then_with(|| a.project_path.cmp(&b.project_path))
    });
    confirmed.dedup();
    out.confirmed = confirmed;
    out
}

#[cfg(test)]
#[path = "advisory_match_tests.rs"]
mod tests;
