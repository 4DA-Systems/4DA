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

#[cfg(test)]
#[path = "advisory_match_tests.rs"]
mod tests;
