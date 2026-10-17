// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The matcher's verdict for ONE advisory row: which installed copies it
//! confirms, how many it cannot decide, how many it checked. The pure half of
//! the app's `osv/advisory_match.rs` (single-engine C0.2), which the scorer's
//! security lane reads so Signal / Brief and Preemption answer "is this build
//! exposed?" from one truth. It decides each copy with the same candidate set
//! and the same primitive ([`check_version_affected`]) as
//! [`crate::matching::match_inventory`].

use std::collections::HashSet;

use crate::ecosystem::normalize as osv_ecosystem;
use crate::matching::{is_npm_security_holding, normalize_project_path, Package};
use crate::osv::AdvisoryRecord;
use crate::range::check_version_affected;

/// One installed copy the matcher placed inside the affected range with a
/// parseable version (the only copies a positive verdict may cite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposedCopy {
    pub project_path: String,
    pub installed_version: String,
    pub is_direct: bool,
    pub is_dev: bool,
}

/// The matcher's verdict for one advisory row against the inventory.
#[derive(Debug, Clone, Default)]
pub struct AdvisoryExposure {
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

/// Run the matcher's per-copy check for ONE advisory row.
///
/// `inventory` is already sliced to the advisory's package name (compared
/// case-insensitively), every ecosystem: resolved rows (`primary`) in
/// precedence order (the first row per project wins), then installed copies.
pub fn exposure_for_record(record: &AdvisoryRecord, inventory: &[Package]) -> AdvisoryExposure {
    let mut out = AdvisoryExposure::default();
    let eco = osv_ecosystem(&record.ecosystem);

    let mut seen: HashSet<String> = HashSet::new();
    let deps: Vec<&Package> = inventory
        .iter()
        .filter(|d| d.primary)
        .filter(|d| osv_ecosystem(&d.ecosystem) == eco)
        .filter(|d| seen.insert(normalize_project_path(&d.project)))
        .collect();
    if deps.is_empty() {
        return out;
    }

    let mut confirmed: Vec<ExposedCopy> = Vec::new();
    for dep in &deps {
        let project = normalize_project_path(&dep.project);
        // Union of every installed copy of this (project, package) with the
        // resolved row, deduped by version — the matcher's candidate set.
        let mut versions: HashSet<String> = HashSet::new();
        let mut candidates: Vec<(Option<String>, bool, bool)> = inventory
            .iter()
            .filter(|i| !i.primary)
            .filter(|i| {
                normalize_project_path(&i.project) == project && osv_ecosystem(&i.ecosystem) == eco
            })
            .filter_map(|i| i.version.as_ref().map(|v| (v, i)))
            .filter(|(v, _)| versions.insert((*v).clone()))
            .map(|(v, i)| (Some(v.clone()), i.is_direct, i.is_dev))
            .collect();
        if !dep.version.as_ref().is_some_and(|v| versions.contains(v)) {
            candidates.push((dep.version.clone(), dep.is_direct, dep.is_dev));
        }
        for (version, is_direct, is_dev) in candidates {
            if is_npm_security_holding(&record.ecosystem, version.as_deref()) {
                continue;
            }
            out.copies_checked += 1;
            match check_version_affected(
                &record.ecosystem,
                version.as_deref(),
                &record.affected_ranges,
            ) {
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
mod tests {
    use super::*;

    fn row(eco: &str, project: &str, version: Option<&str>, primary: bool, dev: bool) -> Package {
        Package {
            ecosystem: eco.to_string(),
            project: project.to_string(),
            name: "ws".to_string(),
            version: version.map(str::to_string),
            is_direct: primary,
            is_dev: dev,
            primary,
        }
    }

    fn ws_below_7_5_10() -> AdvisoryRecord {
        AdvisoryRecord {
            advisory_id: "GHSA-ws".to_string(),
            package_name: "ws".to_string(),
            ecosystem: "npm".to_string(),
            affected_ranges: Some(
                r#"[{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.5.10"}]}]"#
                    .to_string(),
            ),
            fixed_versions: Some(r#"["7.5.10"]"#.to_string()),
        }
    }

    #[test]
    fn confirms_copies_counts_unconfirmed_and_orders_runtime_first() {
        let inventory = [
            // Resolved rows: the first per project wins (user before scan).
            row("javascript", r"C:\App\", Some("7.5.10"), true, false),
            row("npm", "c:/app", Some("7.0.0"), true, false),
            row("npm", "/dev-only", None, true, true),
            // Installed copies of c:/app: one vulnerable, transitive, dev.
            row("npm", "C:/App", Some("7.5.9"), false, true),
            row("npm", "C:/App", Some("7.5.10"), false, false),
            // A copy in another ecosystem is not this advisory's.
            row("PyPI", "c:/app", Some("7.5.9"), false, false),
        ];
        let e = exposure_for_record(&ws_below_7_5_10(), &inventory);
        assert_eq!(
            e.confirmed,
            vec![ExposedCopy {
                project_path: "c:/app".to_string(),
                installed_version: "7.5.9".to_string(),
                is_direct: false,
                is_dev: true,
            }]
        );
        // The version-less resolved row of /dev-only cannot be decided.
        assert_eq!(e.unconfirmed, 1);
        // c:/app: copies 7.5.9 and 7.5.10 (the resolved 7.5.10 is a dup);
        // /dev-only: its resolved row.
        assert_eq!(e.copies_checked, 3);
    }

    #[test]
    fn no_resolved_row_in_the_ecosystem_is_no_verdict() {
        let inventory = [row("PyPI", "/p", Some("7.5.9"), true, false)];
        let e = exposure_for_record(&ws_below_7_5_10(), &inventory);
        assert_eq!(e.copies_checked, 0);
        assert!(e.confirmed.is_empty());
    }
}
