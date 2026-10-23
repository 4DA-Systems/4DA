// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV records as the matcher reads them: one stored row per (advisory,
//! package), its `affected` entries merged. Moved out of the app's
//! `osv/sync.rs` (single-engine C0.2) so the app's mirror and any other host
//! (the conformance harness, the wasm module) store a record identically.

use serde::{Deserialize, Serialize};

use crate::range::{Range, ENUMERATED_RANGE};

/// `affected[].package` of an OSV record.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageRef {
    pub name: String,
    pub ecosystem: String,
}

/// One `affected[]` entry of an OSV record.
#[derive(Debug, Deserialize, Clone)]
pub struct Affected {
    pub package: Option<PackageRef>,
    pub ranges: Option<Vec<Range>>,
    /// OSV's explicit list of affected versions. Some advisories carry ONLY
    /// this (GHSA-q58r-hwc8-rm9j, bootstrap) or only `GIT` ranges plus this.
    /// It used to be dropped on the belief that the matcher would then "assume
    /// affected" — it did not: the stored ranges were `[]` and an empty
    /// range list reads as "confirmed not affected". Stored as an
    /// [`ENUMERATED_RANGE`] range so every ranges reader sees it.
    #[serde(default)]
    pub versions: Option<Vec<String>>,
}

/// The part of a verbatim OSV record the matcher needs. Every other field
/// (references, ...) is ignored when parsing.
#[derive(Debug, Deserialize, Clone)]
pub struct OsvRecord {
    pub id: String,
    #[serde(default)]
    pub aliases: Option<Vec<String>>,
    /// For the malware and maintenance-notice classifications only.
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    /// OSV `severity[]` (CVSS vectors); present = the record is scored.
    #[serde(default)]
    pub severity: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub database_specific: Option<serde_json::Value>,
    #[serde(default)]
    pub affected: Option<Vec<Affected>>,
    /// Set when the source withdrew the advisory.
    #[serde(default)]
    pub withdrawn: Option<String>,
}

/// One stored advisory row: what `osv_advisories` holds for one (advisory,
/// package, ecosystem), restricted to what matching reads.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AdvisoryRecord {
    pub advisory_id: String,
    pub package_name: String,
    pub ecosystem: String,
    /// The merged ranges as stored JSON ([`serialize_ranges`]).
    #[serde(default)]
    pub affected_ranges: Option<String>,
    /// Every `fixed` event, as a stored JSON array ([`fixed_versions_json`]).
    #[serde(default)]
    pub fixed_versions: Option<String>,
    /// A malicious-package record ([`crate::is_malware`]): a release it
    /// affects is never a fix target.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub malware: bool,
    /// A maintenance notice ([`crate::severity::is_maintenance_notice`]): it
    /// never blocks a fix target.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub informational: bool,
}

impl AdvisoryRecord {
    /// How the fix-target walk treats this record.
    pub fn kind(&self) -> crate::fix_target::AdvisoryKind {
        crate::fix_target::AdvisoryKind {
            malware: self.malware,
            informational: self.informational,
        }
    }
}

/// One advisory's affected entries grouped by (package, ecosystem), in first
/// appearance order, with their ranges UNIONED. OSV can list the SAME package
/// in MULTIPLE affected entries (different version branches, e.g. next
/// [13.4.6,15.5.16) AND [16.0.0,16.2.5)). The mirror holds one row per
/// (advisory, package), so keeping only the first dropped the branch that
/// matched the user's pinned version. An explicit `versions` list becomes an
/// [`ENUMERATED_RANGE`] range. The rule vectors evaluate a record through
/// this too, so they read it exactly as it is stored.
pub fn merge_affected(affected_list: &[Affected]) -> Vec<((String, String), Affected)> {
    let mut by_pkg: std::collections::HashMap<(String, String), Affected> =
        std::collections::HashMap::new();
    let mut order: Vec<(String, String)> = Vec::new();
    for affected in affected_list {
        let Some(pkg) = &affected.package else {
            continue;
        };
        let key = (pkg.name.clone(), pkg.ecosystem.clone());
        let merged = by_pkg.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Affected {
                package: Some(pkg.clone()),
                ranges: Some(Vec::new()),
                versions: None,
            }
        });
        if let Some(ranges) = &affected.ranges {
            merged
                .ranges
                .get_or_insert_with(Vec::new)
                .extend(ranges.iter().cloned());
        }
        if let Some(versions) = affected.versions.as_ref().filter(|v| !v.is_empty()) {
            merged.ranges.get_or_insert_with(Vec::new).push(Range {
                range_type: ENUMERATED_RANGE.to_string(),
                events: Some(
                    versions
                        .iter()
                        .cloned()
                        .map(serde_json::Value::from)
                        .collect(),
                ),
            });
        }
    }
    order
        .into_iter()
        .filter_map(|key| by_pkg.remove(&key).map(|affected| (key, affected)))
        .collect()
}

/// The ranges JSON the mirror stores for one merged entry.
pub fn serialize_ranges(affected: &Affected) -> Option<String> {
    affected
        .ranges
        .as_ref()
        .and_then(|r| serde_json::to_string(r).ok())
}

/// Every `fixed` event of one merged entry as a JSON array, `None` when the
/// entry has none.
pub fn fixed_versions_json(affected: &Affected) -> Option<String> {
    let fixed: Vec<String> = affected
        .ranges
        .as_ref()
        .map(|ranges| {
            ranges
                .iter()
                .filter_map(|r| r.events.as_ref())
                .flatten()
                .filter_map(|event| {
                    event
                        .as_object()
                        .and_then(|obj| obj.get("fixed"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    if fixed.is_empty() {
        None
    } else {
        serde_json::to_string(&fixed).ok()
    }
}

/// The rows the mirror's active read returns for one verbatim OSV record:
/// one per merged (package, ecosystem), none for a withdrawn record.
pub fn stored_records(record: &OsvRecord) -> Vec<AdvisoryRecord> {
    if record.withdrawn.is_some() {
        return Vec::new();
    }
    merge_affected(record.affected.as_deref().unwrap_or(&[]))
        .into_iter()
        .map(|((package_name, ecosystem), affected)| {
            let fixed_versions = fixed_versions_json(&affected);
            AdvisoryRecord {
                advisory_id: record.id.clone(),
                package_name,
                ecosystem,
                affected_ranges: serialize_ranges(&affected),
                malware: record_is_malware(record),
                informational: crate::severity::is_maintenance_notice(
                    fixed_versions.is_none(),
                    record.severity.as_ref().is_some_and(|s| !s.is_empty()),
                    record.summary.as_deref().unwrap_or(""),
                    record.details.as_deref(),
                ),
                fixed_versions,
            }
        })
        .collect()
}

/// [`crate::is_malware`] over a verbatim record.
fn record_is_malware(record: &OsvRecord) -> bool {
    let cwe_ids: Vec<String> = record
        .database_specific
        .as_ref()
        .and_then(|d| d.get("cwe_ids"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_str().map(str::to_string))
        .collect();
    crate::is_malware(
        &record.id,
        record.aliases.as_deref().unwrap_or(&[]),
        &cwe_ids,
        record.summary.as_deref().unwrap_or(""),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(json: &str) -> OsvRecord {
        serde_json::from_str(json).unwrap_or_else(|e| panic!("{e}: {json}"))
    }

    #[test]
    fn branches_of_one_package_merge_and_versions_become_an_enumerated_range() {
        let r = record(
            r#"{"id":"GHSA-x","summary":"ignored","affected":[
              {"package":{"name":"next","ecosystem":"npm"},
               "ranges":[{"type":"SEMVER","events":[{"introduced":"13.4.6"},{"fixed":"15.5.16"}]}]},
              {"package":{"name":"next","ecosystem":"npm"},
               "ranges":[{"type":"SEMVER","events":[{"introduced":"16.0.0"},{"fixed":"16.2.5"}]}],
               "versions":["16.0.0"]},
              {"package":{"name":"other","ecosystem":"npm"}}]}"#,
        );
        let rows = stored_records(&r);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].package_name, "next");
        let ranges = rows[0].affected_ranges.as_deref().unwrap_or_default();
        assert_eq!(ranges.matches("SEMVER").count(), 2);
        assert!(ranges.contains(ENUMERATED_RANGE));
        assert_eq!(
            rows[0].fixed_versions.as_deref(),
            Some(r#"["15.5.16","16.2.5"]"#)
        );
        assert_eq!(rows[1].affected_ranges.as_deref(), Some("[]"));
        assert_eq!(rows[1].fixed_versions, None);
    }

    #[test]
    fn a_withdrawn_record_stores_nothing() {
        let r = record(
            r#"{"id":"GHSA-w","withdrawn":"2024-01-01T00:00:00Z","affected":[
              {"package":{"name":"a","ecosystem":"npm"}}]}"#,
        );
        assert!(stored_records(&r).is_empty());
    }
}
