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
    /// `affected[].database_specific`. RustSec keeps `informational`
    /// (`unmaintained` / `unsound` / `notice`) HERE, never at the record
    /// level: 90 per-affected entries across the pinned conformance corpus,
    /// 0 record-level ones. [`merge_affected`] keeps a notice class any of
    /// the merged entries carries; [`is_notice_for`] reads it.
    #[serde(default)]
    pub database_specific: Option<serde_json::Value>,
}

/// `informational` of a `database_specific` block: the class OSV states,
/// `None` when the block states none (absent, not a string, or empty).
fn informational(database_specific: Option<&serde_json::Value>) -> Option<&str> {
    database_specific
        .and_then(|d| d.get("informational"))
        .and_then(serde_json::Value::as_str)
        .filter(|class| !class.is_empty())
}

/// The informational classes that make a record a maintenance notice.
/// `unsound` is NOT one: unsound records carry real fixes, and a notice's fix
/// is dropped (`matching`).
fn is_notice_class(value: Option<&str>) -> bool {
    matches!(value, Some("unmaintained" | "notice"))
}

/// Fold one more entry's `database_specific` into the merged entry's: the
/// first entry's block is kept, a class a later entry states fills an
/// `informational` the block lacks, and a notice class replaces one that is
/// not. A merged entry is therefore a notice when ANY of its source entries
/// is, and states a class when any of them does.
fn merge_database_specific(
    merged: &mut Option<serde_json::Value>,
    incoming: Option<&serde_json::Value>,
) {
    let Some(incoming) = incoming else {
        return;
    };
    let Some(current) = merged.as_mut() else {
        *merged = Some(incoming.clone());
        return;
    };
    let class = informational(Some(incoming));
    let held = informational(Some(current));
    let adopt =
        class.is_some() && (held.is_none() || (is_notice_class(class) && !is_notice_class(held)));
    if !adopt {
        return;
    }
    match current.as_object_mut() {
        Some(obj) => {
            obj.insert(
                "informational".to_string(),
                serde_json::Value::from(class.unwrap_or_default()),
            );
        }
        None => *current = incoming.clone(),
    }
}

/// THE maintenance-notice rule for one package of a record, both engines'.
/// Structured data beats wording: the class OSV states for this package
/// decides on its own, the package's affected entry first (`merged`, from
/// [`merge_affected`], so ANY of its entries), else the record-level
/// `database_specific.informational`. `unmaintained` / `notice` is a notice;
/// `unsound` or any other stated class is not, whatever the summary says
/// (RUSTSEC-2025-0068, serde_yml: `unsound`, summary "unsound and
/// unmaintained"). Another package's entry is never read. Only when no class
/// is stated does the summary decide ([`crate::severity::is_maintenance_notice`]).
pub fn is_notice_for(
    summary: &str,
    record_database_specific: Option<&serde_json::Value>,
    merged: &Affected,
) -> bool {
    let class = informational(merged.database_specific.as_ref())
        .or_else(|| informational(record_database_specific));
    crate::severity::is_maintenance_notice(summary, class)
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
    /// A maintenance notice ([`is_notice_for`]): it never blocks a fix
    /// target.
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
                database_specific: None,
            }
        });
        merge_database_specific(
            &mut merged.database_specific,
            affected.database_specific.as_ref(),
        );
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
                informational: is_notice_for(
                    record.summary.as_deref().unwrap_or(""),
                    record.database_specific.as_ref(),
                    &affected,
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

    /// RUSTSEC-2018-0017 as pinned: `informational: unmaintained` sits in
    /// `affected[].database_specific` and the summary never says the word.
    #[test]
    fn a_per_affected_unmaintained_flag_makes_a_notice() {
        let r = record(
            r#"{"id":"RUSTSEC-2018-0017",
              "summary":"`tempdir` crate has been deprecated; use `tempfile` instead",
              "database_specific":{"license":"CC0-1.0"},
              "affected":[{"package":{"name":"tempdir","ecosystem":"crates.io"},
                "ranges":[{"type":"SEMVER","events":[{"introduced":"0.0.0-0"}]}],
                "database_specific":{"categories":[],"cvss":null,"informational":"unmaintained"}}]}"#,
        );
        let rows = stored_records(&r);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].informational);
        assert!(rows[0].kind().informational);
    }

    /// RUSTSEC-2018-0018 (smallvec) as pinned: `informational: unsound`
    /// with a fix is a vulnerability, not a notice.
    #[test]
    fn a_per_affected_unsound_flag_stays_a_vulnerability() {
        let r = record(
            r#"{"id":"RUSTSEC-2018-0018",
              "summary":"smallvec creates uninitialized value of any type",
              "affected":[{"package":{"name":"smallvec","ecosystem":"crates.io"},
                "ranges":[{"type":"SEMVER","events":[{"introduced":"0.0.0-0"},{"fixed":"0.6.13"}]}],
                "database_specific":{"informational":"unsound"}}]}"#,
        );
        let rows = stored_records(&r);
        assert!(!rows[0].informational);
        assert_eq!(rows[0].fixed_versions.as_deref(), Some(r#"["0.6.13"]"#));
    }

    /// The flag is per package: a notice entry for one package does not make
    /// the record's other package a notice, and merging two entries of one
    /// package keeps the notice class whichever entry carries it.
    #[test]
    fn the_notice_flag_is_per_package_and_survives_the_merge() {
        let r = record(
            r#"{"id":"RUSTSEC-2099-0100","summary":"something happened",
              "affected":[
                {"package":{"name":"a","ecosystem":"crates.io"},
                 "database_specific":{"informational":"unsound","source":"x"}},
                {"package":{"name":"a","ecosystem":"crates.io"},
                 "database_specific":{"informational":"unmaintained"}},
                {"package":{"name":"b","ecosystem":"crates.io"},
                 "database_specific":{"informational":"unsound"}},
                {"package":{"name":"c","ecosystem":"crates.io"}}]}"#,
        );
        let rows = stored_records(&r);
        let flag = |name: &str| {
            rows.iter()
                .find(|row| row.package_name == name)
                .map(|row| row.informational)
        };
        assert_eq!(flag("a"), Some(true));
        assert_eq!(flag("b"), Some(false));
        assert_eq!(flag("c"), Some(false));
        let merged = merge_affected(r.affected.as_deref().unwrap_or_default());
        let a = &merged[0].1;
        assert_eq!(
            a.database_specific.as_ref().and_then(|d| d.get("source")),
            Some(&serde_json::Value::from("x")),
            "the first entry's block is kept"
        );
    }

    /// The record-level flag and the summary still make a notice on their
    /// own; a `notice` class counts like `unmaintained`.
    #[test]
    fn record_level_flag_summary_and_notice_class_each_suffice() {
        let at_record = record(
            r#"{"id":"X-1","summary":"s","database_specific":{"informational":"unmaintained"},
              "affected":[{"package":{"name":"a","ecosystem":"crates.io"}}]}"#,
        );
        let by_summary = record(
            r#"{"id":"X-2","summary":"a is no longer maintained",
              "affected":[{"package":{"name":"a","ecosystem":"crates.io"}}]}"#,
        );
        let notice = record(
            r#"{"id":"X-3","summary":"s",
              "affected":[{"package":{"name":"a","ecosystem":"crates.io"},
                "database_specific":{"informational":"notice"}}]}"#,
        );
        for r in [at_record, by_summary, notice] {
            assert!(stored_records(&r)[0].informational, "{}", r.id);
        }
    }

    fn flags(r: &OsvRecord) -> Vec<(String, bool)> {
        stored_records(r)
            .into_iter()
            .map(|row| (row.package_name, row.informational))
            .collect()
    }

    /// RUSTSEC-2025-0068 (serde_yml) as OSV serves it: the per-affected
    /// class `unsound` decides, though the summary says "unmaintained".
    #[test]
    fn a_stated_unsound_class_beats_unmaintained_wording() {
        let r = record(
            r#"{"id":"RUSTSEC-2025-0068","summary":"serde_yml crate is unsound and unmaintained",
              "database_specific":{"license":"CC0-1.0"},
              "affected":[{"package":{"name":"serde_yml","ecosystem":"crates.io"},
                "ranges":[{"type":"SEMVER","events":[{"introduced":"0.0.0-0"}]}],
                "database_specific":{"categories":[],"cvss":null,"informational":"unsound"}}]}"#,
        );
        assert!(!stored_records(&r)[0].informational);
        // The record-level class decides the same way when no entry states one.
        let at_record = record(
            r#"{"id":"X-4","summary":"a is unmaintained","database_specific":{"informational":"unsound"},
              "affected":[{"package":{"name":"a","ecosystem":"crates.io"}}]}"#,
        );
        assert!(!stored_records(&at_record)[0].informational);
        // The package's own class comes before the record-level one.
        let both = record(
            r#"{"id":"X-5","summary":"s","database_specific":{"informational":"unsound"},
              "affected":[{"package":{"name":"a","ecosystem":"crates.io"},
                "database_specific":{"informational":"unmaintained"}}]}"#,
        );
        assert!(stored_records(&both)[0].informational);
    }

    /// Another package's per-affected class is never borrowed, in either
    /// direction: a package that states no class is decided by the summary.
    #[test]
    fn another_packages_class_is_not_borrowed() {
        let r = record(
            r#"{"id":"X-6","summary":"memory corruption in a",
              "affected":[
                {"package":{"name":"a","ecosystem":"crates.io"},
                 "database_specific":{"informational":"unmaintained"}},
                {"package":{"name":"b","ecosystem":"crates.io"}}]}"#,
        );
        assert_eq!(
            flags(&r),
            vec![("a".to_string(), true), ("b".to_string(), false)]
        );
        let worded = record(
            r#"{"id":"X-7","summary":"b is unmaintained",
              "affected":[
                {"package":{"name":"a","ecosystem":"crates.io"},
                 "database_specific":{"informational":"unsound"}},
                {"package":{"name":"b","ecosystem":"crates.io"}}]}"#,
        );
        assert_eq!(
            flags(&worded),
            vec![("a".to_string(), false), ("b".to_string(), true)],
            "a's own unsound class decides; b states none, so its wording does"
        );
    }

    /// A class a later entry of the same package states fills a first entry
    /// that states none.
    #[test]
    fn a_later_entrys_class_fills_the_merge() {
        let r = record(
            r#"{"id":"X-8","summary":"a is unmaintained",
              "affected":[
                {"package":{"name":"a","ecosystem":"crates.io"},"database_specific":{"source":"x"}},
                {"package":{"name":"a","ecosystem":"crates.io"},
                 "database_specific":{"informational":"unsound"}}]}"#,
        );
        assert!(!stored_records(&r)[0].informational);
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
