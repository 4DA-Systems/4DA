// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Install drift on the Preemption feed (AD-046) — a child module of
//! `preemption.rs`, split out because that file is past the size ceiling.
//!
//! The canonical row is `evidence::install_drift`'s. Two hand-offs live here:
//! - [`append_to_feed`] — the lens feeds (full, fast, free floor). The free
//!   floor admits only rows whose urgency rests on an OSV range match: the
//!   floor's own provenance rule, the one `free_floor_view` applies to a
//!   cached full feed, so a cache hit and a miss agree.
//! - [`legacy_alerts`] — `get_preemption_feed`, which the AI brief and the
//!   deterministic brief read. Only a row whose install clears an advisory
//!   AND whose pin is clean is projected: those surfaces print every security
//!   line as "(installed -> update to >= fix)" or "(no fix published)", and
//!   only that row has a fix that is true to print.

use crate::evidence::install_drift::{self, ProjectDrift};
use crate::evidence::{ConfidenceProvenance, EvidenceItem, TierScope};

use super::{AlertEvidence, AlertUrgency, PreemptionAlert, PreemptionType, SuggestedAction};

/// True for an install-drift row's id — the canonical item's and the
/// legacy alert's alike.
pub(super) fn is_install_drift(id: &str) -> bool {
    id.starts_with(install_drift::ID_PREFIX)
}

/// Append the live rows this feed's tier admits. Returns how many.
pub(super) fn append_to_feed(
    items: &mut Vec<EvidenceItem>,
    conn: &rusqlite::Connection,
    scope: TierScope,
) -> usize {
    let before = items.len();
    items.extend(
        install_drift::detect(conn)
            .into_iter()
            .filter(|row| admitted(row, scope)),
    );
    items.len() - before
}

fn admitted(row: &EvidenceItem, scope: TierScope) -> bool {
    scope != TierScope::FreeFloor || row.confidence.provenance == ConfidenceProvenance::OsvVerified
}

/// The brief's projection: one alert per row with a true fix to state.
pub(super) fn legacy_alerts(conn: &rusqlite::Connection) -> Vec<PreemptionAlert> {
    install_drift::projects(conn)
        .iter()
        .filter_map(legacy_alert)
        .collect()
}

fn legacy_alert(drift: &ProjectDrift) -> Option<PreemptionAlert> {
    let (package, installed, pinned) = drift.lead_fix()?;
    let item = drift.to_evidence_item();
    // The brief names `affected_dependencies[0]`: the package with the fix.
    let mut dependencies = vec![package.to_string()];
    dependencies.extend(
        item.affected_deps
            .iter()
            .filter(|d| d.as_str() != package)
            .cloned(),
    );
    Some(PreemptionAlert {
        id: item.id,
        alert_type: PreemptionType::SecurityAdvisory,
        title: item.title,
        explanation: item.explanation,
        // No advisory citations: the brief merges alerts that share an
        // advisory id found in their evidence, and this row must not fold
        // into — and vanish under — the OSV alert for the same package in
        // another project.
        evidence: item
            .evidence
            .iter()
            .filter(|c| c.source != "osv-advisory")
            .map(|c| AlertEvidence {
                source: c.source.clone(),
                title: c.title.clone(),
                url: c.url.clone(),
                freshness_days: c.freshness_days,
                relevance_score: 1.0,
            })
            .collect(),
        affected_projects: item.affected_projects,
        affected_dependencies: dependencies,
        urgency: AlertUrgency::High,
        confidence: item.confidence.value,
        predicted_window: None,
        suggested_actions: item
            .suggested_actions
            .iter()
            .take(1)
            .map(|a| SuggestedAction {
                action_type: "acknowledge".to_string(),
                label: a.label.clone(),
                description: a.description.clone(),
            })
            .collect(),
        created_at: chrono::Utc::now().to_rfc3339(),
        osv_verified: true,
        source_classified: false,
        installed_version: Some(installed.to_string()),
        fixed_version: Some(pinned.to_string()),
        is_direct: Some(true),
        is_dev: Some(drift.all_dev_only()),
        platform_inactive: false,
        lockfile_only: false,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::*;
    use crate::evidence::install_drift::{AdvisoryRef, InstalledCopy, Lockfile, PackageDrift};
    use crate::evidence::Confidence;

    fn hono(cleared: bool, pin_exposed: bool) -> ProjectDrift {
        let advisory = AdvisoryRef {
            id: "GHSA-hono-smuggle".to_string(),
            summary: "hono: request smuggling".to_string(),
            url: Some("https://osv.dev/GHSA-hono-smuggle".to_string()),
        };
        ProjectDrift {
            project: "d:/4da/mcp-4da-server".to_string(),
            lockfile: Lockfile::Pnpm,
            packages: vec![PackageDrift {
                package: "hono".to_string(),
                pinned: BTreeSet::from(["4.13.5".to_string()]),
                installed: Some(InstalledCopy {
                    version: "4.13.1".to_string(),
                    manifest: PathBuf::from("d:/4da/mcp-4da-server/node_modules/hono/package.json"),
                }),
                cleared_by_install: if cleared {
                    vec![advisory.clone()]
                } else {
                    vec![]
                },
                still_exposed: if cleared { vec![] } else { vec![advisory] },
                pin_exposed,
                dev_only: false,
            }],
        }
    }

    #[test]
    fn the_brief_gets_the_high_row_with_a_true_fix_and_no_advisory_to_merge_on() {
        let alert = legacy_alert(&hono(true, false)).expect("a clean fix is projected");
        assert!(matches!(alert.urgency, AlertUrgency::High));
        assert!(
            alert.osv_verified,
            "survives the brief's verified-only filter"
        );
        assert_eq!(alert.installed_version.as_deref(), Some("4.13.1"));
        assert_eq!(alert.fixed_version.as_deref(), Some("4.13.5"));
        assert_eq!(alert.affected_dependencies, vec!["hono".to_string()]);
        assert!(
            alert.evidence.iter().all(|e| !e.title.contains("GHSA-")
                && e.url.as_deref().is_none_or(|u| !u.contains("GHSA-"))),
            "{:?}",
            alert.evidence
        );
        assert!(is_install_drift(&alert.id));
        assert_eq!(
            super::super::cross_tier_dedup_key(&alert),
            alert.id,
            "keyed by its own id, never by the advisory its text cites"
        );
    }

    #[test]
    fn a_row_without_a_true_fix_to_state_stays_off_the_brief() {
        assert!(
            legacy_alert(&hono(false, true)).is_none(),
            "exposed either way: Medium"
        );
        assert!(
            legacy_alert(&hono(true, true)).is_none(),
            "the pin carries its own advisory"
        );
    }

    #[test]
    fn the_free_floor_admits_only_osv_verified_drift() {
        let mut row = hono(true, false).to_evidence_item();
        assert!(admitted(&row, TierScope::FreeFloor));
        row.confidence = Confidence::heuristic(0.95);
        assert!(!admitted(&row, TierScope::FreeFloor));
        assert!(admitted(&row, TierScope::Full));
    }
}
