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
//! SAME rules, the engine crate's `exposure_for_record` (single-engine C0.2).
//! This module is its database adapter.

use fourda_depengine::matching::Package;

use crate::db::Database;

use super::matching::{advisory_record, resolved_row};
use super::types::StoredAdvisory;

pub(crate) use fourda_depengine::exposure::{AdvisoryExposure, ExposedCopy};

/// Run the Preemption matcher's per-copy check for ONE advisory.
pub(crate) fn exposure_for_advisory(db: &Database, advisory: &StoredAdvisory) -> AdvisoryExposure {
    let Ok((user, scanned)) = db.get_auditable_dependencies_for_package(&advisory.package_name)
    else {
        return AdvisoryExposure::default();
    };
    // Resolved rows in the matcher's precedence (user rows win), then every
    // installed copy of the package.
    let mut inventory: Vec<Package> = user
        .iter()
        .chain(scanned.iter())
        .map(resolved_row)
        .collect();
    if inventory.is_empty() {
        return AdvisoryExposure::default();
    }
    let instances = db
        .get_dependency_instances_named(&advisory.package_name)
        .unwrap_or_default();
    inventory.extend(instances.into_iter().map(|i| Package {
        ecosystem: i.ecosystem,
        project: i.project_path,
        name: i.package_name,
        version: Some(i.version),
        is_direct: i.is_direct,
        is_dev: i.is_dev,
        primary: false,
    }));
    fourda_depengine::exposure::exposure_for_record(&advisory_record(advisory), &inventory)
}

/// The version ONE installed copy must reach to clear every known advisory
/// of its package that has a way out: the walk the matcher runs for each
/// copy's `target_version` (`fourda_depengine::matching`, over every stored
/// advisory of the package and the registry's withdrawn releases), so it is
/// the target the Upgrade Plan's line for that copy names (Y4). One
/// advisory's own fix is not it: rmcp 3.1.2 under RUSTSEC-2026-0461 (fixed
/// 3.6.0) still carries 0457 (3.7.0) and 0462 (3.9.0), so the copy's target
/// is 3.9.0. `None` when no walk lands (no fix with a way out).
pub(crate) fn clearing_target(
    db: &Database,
    registry: &fourda_depengine::registry::RegistryIndex,
    package: &str,
    ecosystem_norm: &str,
    installed: &str,
) -> Option<String> {
    let all = db
        .get_osv_advisories_for_package(package, ecosystem_norm)
        .ok()?;
    let records: Vec<fourda_depengine::osv::AdvisoryRecord> =
        all.iter().map(advisory_record).collect();
    let ranges: Vec<&Option<String>> = records.iter().map(|r| &r.affected_ranges).collect();
    let kinds: Vec<_> = records
        .iter()
        .map(fourda_depengine::osv::AdvisoryRecord::kind)
        .collect();
    let key = (
        super::matching::package_key(package, ecosystem_norm),
        ecosystem_norm.to_string(),
    );
    let none = fourda_depengine::registry::RegistryReleases::default();
    let facts = registry.get(&key).unwrap_or(&none);
    fourda_depengine::fix_target::walk_target_kinds(
        ecosystem_norm,
        installed,
        &ranges,
        &kinds,
        facts,
    )
    .target
    .map(|w| w.version)
}

#[cfg(test)]
#[path = "advisory_match_tests.rs"]
mod tests;
