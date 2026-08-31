// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Liveness policy for evidence items — intelligence about projects that have
//! stopped moving must not outrank intelligence about the code the user works
//! on today.
//!
//! 2026-08-31 live audit, founder's instance: every critical Preemption
//! upgrade card and the AI briefing's "Action Required" section nagged about
//! graveyard projects (next@5 in a repo dead since February), because nothing
//! between `detected_projects.last_activity` and the ranked feed ever asked
//! "is anyone still working on this?". The policy here is cap-and-annotate,
//! never drop: a real CVE in a dormant repo is still true — it just is not
//! today's emergency.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::ace::dormancy::{is_dormant_days, project_dormant_days, DORMANT_AFTER_DAYS};

use super::types::{ConfidenceProvenance, EvidenceItem, Urgency};

/// The sentence appended to an explanation when the dormancy cap fires.
/// A function (not a const) so the threshold can never drift from
/// [`DORMANT_AFTER_DAYS`].
pub fn dormant_projects_note() -> String {
    format!("All affected projects inactive >{DORMANT_AFTER_DAYS} days.")
}

/// The per-project inactivity label appended to project names on briefing
/// and explanation lines, e.g. "old-app (inactive 142 days)". One format for
/// every surface.
pub fn inactive_label(days: i64) -> String {
    format!("(inactive {days} days)")
}

/// Days-dormant lookup for detected projects, keyed by normalized path.
/// Paths whose `last_activity` is missing or unparseable are absent from the
/// map — and an absent path is never dormant (unknown must not demote).
#[derive(Debug, Default)]
pub struct ProjectLiveness {
    dormant_days_by_path: HashMap<String, i64>,
}

impl ProjectLiveness {
    /// Load from `detected_projects`. Best-effort: a missing table (fresh DB
    /// before the ACE migration) or a failed query yields an empty map, which
    /// caps nothing.
    pub fn load(conn: &Connection) -> Self {
        let mut dormant_days_by_path = HashMap::new();
        let Ok(mut stmt) = conn.prepare(
            "SELECT path, last_activity FROM detected_projects WHERE last_activity IS NOT NULL",
        ) else {
            return Self::default();
        };
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        });
        if let Ok(rows) = rows {
            for (path, last_activity) in rows.filter_map(std::result::Result::ok) {
                if let Some(days) = project_dormant_days(&last_activity) {
                    dormant_days_by_path.insert(normalize_project_path(&path), days);
                }
            }
        }
        Self {
            dormant_days_by_path,
        }
    }

    /// Test constructor: (path, days_since_activity) pairs.
    #[cfg(test)]
    pub fn from_entries(entries: &[(&str, i64)]) -> Self {
        Self {
            dormant_days_by_path: entries
                .iter()
                .map(|(p, d)| (normalize_project_path(p), *d))
                .collect(),
        }
    }

    /// Days since the project's last recorded activity. `None` when the
    /// project is unknown (never scanned, or unparseable `last_activity`).
    pub fn dormant_days(&self, project_path: &str) -> Option<i64> {
        self.dormant_days_by_path
            .get(&normalize_project_path(project_path))
            .copied()
    }

    /// True only when the project is KNOWN and past the dormancy threshold.
    pub fn is_dormant(&self, project_path: &str) -> bool {
        self.dormant_days(project_path).is_some_and(is_dormant_days)
    }

    /// True when `paths` is non-empty and EVERY path is known-dormant. One
    /// active — or merely unknown — project keeps the item fully urgent.
    pub fn all_dormant(&self, paths: &[String]) -> bool {
        !paths.is_empty() && paths.iter().all(|p| self.is_dormant(p))
    }
}

/// Windows/Unix separators, drive-letter case and trailing slashes all vary
/// between the writers of `detected_projects.path` and the `affected_projects`
/// carried on alerts; comparisons happen in one canonical shape.
fn normalize_project_path(path: &str) -> String {
    path.to_lowercase()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

/// Distinct lowercase package names from `user_dependencies` — the registry
/// of dependencies the user actually installs. `None` when the table does not
/// exist or cannot be read (cannot verify is not the same as verified-absent);
/// `Some(empty)` when the table exists and is genuinely empty.
pub fn load_user_dependency_names(conn: &Connection) -> Option<HashSet<String>> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT LOWER(package_name) FROM user_dependencies")
        .ok()?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0)).ok()?;
    Some(rows.filter_map(std::result::Result::ok).collect())
}

/// True for the provenance tiers that carry no external verification —
/// keyword/pattern (`Checklist`) and weighted-formula (`Heuristic`) numbers.
pub fn provenance_is_unverified(provenance: ConfidenceProvenance) -> bool {
    matches!(
        provenance,
        ConfidenceProvenance::Heuristic | ConfidenceProvenance::Checklist
    )
}

/// Materializer invariant: an item whose confidence is a heuristic guess can
/// never rank Critical or High next to OSV-verified intelligence. Returns
/// true when the cap fired. (2026-08-31 audit: a CRITICAL alert built from an
/// arXiv paper title, confidence 0.57/heuristic, ranked beside 0.95
/// OSV-verified items.)
pub fn cap_unverified_item_urgency(item: &mut EvidenceItem) -> bool {
    if provenance_is_unverified(item.confidence.provenance)
        && matches!(item.urgency, Urgency::Critical | Urgency::High)
    {
        item.urgency = Urgency::Medium;
        return true;
    }
    false
}

/// Dormancy cap: an item whose EVERY affected project is dormant informs, it
/// does not alarm — Critical/High collapse to Medium and the explanation says
/// why. Items are never dropped. Idempotent (the note is appended once).
/// Returns how many items were capped.
pub fn cap_dormant_items(items: &mut [EvidenceItem], liveness: &ProjectLiveness) -> usize {
    let note = dormant_projects_note();
    let mut capped = 0usize;
    for item in items.iter_mut() {
        if !matches!(item.urgency, Urgency::Critical | Urgency::High) {
            continue;
        }
        if !liveness.all_dormant(&item.affected_projects) {
            continue;
        }
        item.urgency = Urgency::Medium;
        if !item.explanation.contains(&note) {
            if !item.explanation.is_empty() && !item.explanation.ends_with(' ') {
                item.explanation.push(' ');
            }
            item.explanation.push_str(&note);
        }
        capped += 1;
    }
    capped
}

#[cfg(test)]
mod tests {
    use super::super::types::{Confidence, EvidenceKind, LensHints};
    use super::*;

    fn item(urgency: Urgency, confidence: Confidence, affected_projects: &[&str]) -> EvidenceItem {
        EvidenceItem {
            id: "t".to_string(),
            kind: EvidenceKind::Alert,
            title: "test".to_string(),
            explanation: "Because a test requires it.".to_string(),
            confidence,
            urgency,
            reversibility: None,
            evidence: vec![],
            affected_projects: affected_projects.iter().map(|s| s.to_string()).collect(),
            affected_deps: vec![],
            suggested_actions: vec![],
            precedents: vec![],
            refutation_condition: None,
            lens_hints: LensHints::preemption_only(),
            created_at: 0,
            expires_at: None,
        }
    }

    // ── path normalization / lookup ─────────────────────────────────

    #[test]
    fn lookup_survives_separator_and_case_differences() {
        let l = ProjectLiveness::from_entries(&[(r"C:\Users\Dev\Documents\old-app", 200)]);
        assert_eq!(l.dormant_days("c:/users/dev/documents/old-app/"), Some(200));
        assert!(l.is_dormant(r"C:\USERS\DEV\Documents\OLD-APP"));
    }

    #[test]
    fn unknown_project_is_never_dormant() {
        let l = ProjectLiveness::from_entries(&[("/proj/a", 500)]);
        assert_eq!(l.dormant_days("/proj/other"), None);
        assert!(!l.is_dormant("/proj/other"));
    }

    #[test]
    fn all_dormant_requires_every_path_known_and_past_threshold() {
        let l = ProjectLiveness::from_entries(&[("/proj/dead", 200), ("/proj/live", 3)]);
        let dead = vec!["/proj/dead".to_string()];
        let mixed = vec!["/proj/dead".to_string(), "/proj/live".to_string()];
        let with_unknown = vec!["/proj/dead".to_string(), "/proj/unknown".to_string()];
        assert!(l.all_dormant(&dead));
        assert!(!l.all_dormant(&mixed), "one live project keeps urgency");
        assert!(
            !l.all_dormant(&with_unknown),
            "an unknown project must not read as dormant"
        );
        assert!(!l.all_dormant(&[]), "no projects = nothing proven dormant");
    }

    // ── dormancy cap ─────────────────────────────────────────────────

    #[test]
    fn dormant_critical_caps_to_medium_and_annotates() {
        let l = ProjectLiveness::from_entries(&[("/proj/dead", 180)]);
        let mut items = vec![item(
            Urgency::Critical,
            Confidence::osv_verified(0.95),
            &["/proj/dead"],
        )];
        assert_eq!(cap_dormant_items(&mut items, &l), 1);
        assert_eq!(items[0].urgency, Urgency::Medium);
        assert!(
            items[0].explanation.contains(&dormant_projects_note()),
            "explanation must say why: {}",
            items[0].explanation
        );
    }

    #[test]
    fn dormancy_cap_is_idempotent() {
        let l = ProjectLiveness::from_entries(&[("/proj/dead", 180)]);
        let mut items = vec![item(
            Urgency::High,
            Confidence::osv_verified(0.9),
            &["/proj/dead"],
        )];
        cap_dormant_items(&mut items, &l);
        let after_once = items[0].explanation.clone();
        // A second pass (e.g. an item flowing through two feed assemblies)
        // must not duplicate the note.
        items[0].urgency = Urgency::High;
        cap_dormant_items(&mut items, &l);
        assert_eq!(items[0].explanation, after_once);
    }

    #[test]
    fn active_or_unknown_projects_are_untouched() {
        let l = ProjectLiveness::from_entries(&[("/proj/dead", 180), ("/proj/live", 1)]);
        let mut items = vec![
            item(
                Urgency::Critical,
                Confidence::osv_verified(0.95),
                &["/proj/dead", "/proj/live"],
            ),
            item(Urgency::Critical, Confidence::osv_verified(0.95), &[]),
        ];
        assert_eq!(cap_dormant_items(&mut items, &l), 0);
        assert_eq!(items[0].urgency, Urgency::Critical);
        assert_eq!(items[1].urgency, Urgency::Critical);
    }

    #[test]
    fn medium_and_watch_items_gain_no_note() {
        let l = ProjectLiveness::from_entries(&[("/proj/dead", 180)]);
        let mut items = vec![item(
            Urgency::Watch,
            Confidence::heuristic(0.4),
            &["/proj/dead"],
        )];
        assert_eq!(cap_dormant_items(&mut items, &l), 0);
        assert!(!items[0].explanation.contains(&dormant_projects_note()));
    }

    // ── provenance cap ───────────────────────────────────────────────

    #[test]
    fn heuristic_critical_and_high_cap_to_medium() {
        for urgency in [Urgency::Critical, Urgency::High] {
            let mut it = item(urgency, Confidence::heuristic(0.57), &[]);
            assert!(cap_unverified_item_urgency(&mut it));
            assert_eq!(it.urgency, Urgency::Medium);
        }
    }

    #[test]
    fn verified_and_llm_items_keep_their_urgency() {
        let mut osv = item(Urgency::Critical, Confidence::osv_verified(0.95), &[]);
        assert!(!cap_unverified_item_urgency(&mut osv));
        assert_eq!(osv.urgency, Urgency::Critical);

        let mut llm = item(Urgency::High, Confidence::llm_assessed(0.8), &[]);
        assert!(!cap_unverified_item_urgency(&mut llm));
        assert_eq!(llm.urgency, Urgency::High);
    }

    #[test]
    fn heuristic_medium_needs_no_cap() {
        let mut it = item(Urgency::Medium, Confidence::heuristic(0.6), &[]);
        assert!(!cap_unverified_item_urgency(&mut it));
        assert_eq!(it.urgency, Urgency::Medium);
    }
}
