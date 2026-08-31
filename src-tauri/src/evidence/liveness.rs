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
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::ace::dormancy::{
    is_dormant_days, last_activity_from_fs, project_dormant_days, DORMANT_AFTER_DAYS,
};

use super::types::{ConfidenceProvenance, EvidenceItem, Urgency};

/// Hard ceiling on how many DISTINCT project paths one `ProjectLiveness` will
/// probe on disk. Memoization already collapses a feed's repeated paths to one
/// probe each (a 149-item feed cites a handful of repos, not 149); this bounds
/// the pathological case where a corpus names hundreds of distinct projects.
/// Past the ceiling the answer is `None` — unknown, which never demotes.
/// The live corpus that motivated the fix names 20 distinct project paths.
pub const MAX_FS_LIVENESS_PROBES: usize = 128;

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

/// Days-dormant lookup for the projects an alert can cite, keyed by
/// normalized path.
///
/// Two tiers, in order:
/// 1. `detected_projects.last_activity` — the recorded answer, free.
/// 2. The filesystem, for any path tier 1 does not hold (2026-08-31 audit:
///    see [`ProjectLiveness::dormant_days`] for why tier 1 alone is not
///    enough), memoized per instance and bounded by
///    [`MAX_FS_LIVENESS_PROBES`].
///
/// A path neither tier can resolve stays `None` — and `None` is unknown, not
/// dormant. Unknown must never demote.
#[derive(Debug, Default)]
pub struct ProjectLiveness {
    dormant_days_by_path: HashMap<String, i64>,
    /// Tier-2 memo, keyed by normalized path. Caches MISSES (`None`) as well
    /// as hits, so an unresolvable path costs one probe per instance, not one
    /// per alert that cites it.
    fs_dormant_days: Mutex<HashMap<String, Option<i64>>>,
    /// Distinct paths actually probed on disk — the budget spent against
    /// [`MAX_FS_LIVENESS_PROBES`].
    fs_probes: AtomicUsize,
}

impl ProjectLiveness {
    /// Load from `detected_projects`. Best-effort: a missing table (fresh DB
    /// before the ACE migration) or a failed query yields an empty map — from
    /// which the filesystem fallback can still answer.
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
            ..Self::default()
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
            ..Self::default()
        }
    }

    /// Days since the project's last activity, or `None` when neither the
    /// recorded row nor the filesystem can say.
    ///
    /// **Why the filesystem tier exists.** `detected_projects` rows are
    /// written by `ace::upsert_detected_project`, gated behind
    /// `relevance >= 0.15` — so a project scoring below that threshold is
    /// never recorded, while `user_dependencies` (a different writer, a
    /// different gate) still carries its packages and therefore still puts
    /// its path on alerts. Measured on the founder's instance 2026-08-31:
    /// `detected_projects` held 15 rows against 20 distinct project paths in
    /// `user_dependencies`; an `ace_full_scan` of one of the missing five
    /// returned `manifest_scan.confidence = 0.0737` and wrote no row. The cap
    /// added in #560 was therefore structurally inert for exactly the repos
    /// it was built for — a project too irrelevant to be recorded was also
    /// too unrecorded to be capped, so `next`/`turbo` upgrades kept ranking
    /// Critical against repos idle since February. Reading the same
    /// filesystem evidence the scanner would have written closes that gap
    /// without depending on the relevance gate at all.
    pub fn dormant_days(&self, project_path: &str) -> Option<i64> {
        let key = normalize_project_path(project_path);
        if let Some(days) = self.dormant_days_by_path.get(&key) {
            return Some(*days);
        }
        self.dormant_days_from_fs(&key, project_path)
    }

    /// Tier 2: memoized, budgeted filesystem probe. The lock is held across
    /// the probe so concurrent callers asking for the same path wait for the
    /// one answer rather than each paying for it.
    fn dormant_days_from_fs(&self, key: &str, raw_path: &str) -> Option<i64> {
        // A poisoned memo is still a valid memo — a panic in another thread
        // must not turn every subsequent lookup into a fresh disk probe.
        let mut memo = match self.fs_dormant_days.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(cached) = memo.get(key) {
            return *cached;
        }
        let resolved = if self.fs_probes.load(Ordering::Relaxed) >= MAX_FS_LIVENESS_PROBES {
            None
        } else {
            self.fs_probes.fetch_add(1, Ordering::Relaxed);
            probe_fs_dormant_days(raw_path)
        };
        memo.insert(key.to_string(), resolved);
        resolved
    }

    /// Distinct paths this instance has probed on disk. Test-only: the bound
    /// is the point of the design, so it is asserted, not assumed.
    #[cfg(test)]
    pub fn fs_probe_count(&self) -> usize {
        self.fs_probes.load(Ordering::Relaxed)
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

/// One filesystem probe: the same git-marker and manifest-mtime evidence
/// `ace::dormancy` writes into `detected_projects.last_activity`, read live.
///
/// Probes the RAW path, not the normalized one — normalization lowercases for
/// comparison, and case matters to a case-sensitive filesystem. Storage paths
/// are already lowercased on Windows only (`project_inclusion::
/// canonical_storage_path`), so the raw string resolves on both platforms.
///
/// A path that is not a readable directory returns `None`. A project whose
/// directory is gone is unreadable, not proven dead: never guess dormant.
fn probe_fs_dormant_days(project_path: &str) -> Option<i64> {
    let trimmed = project_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let dir = Path::new(trimmed);
    if !dir.is_dir() {
        return None;
    }
    last_activity_from_fs(dir)
        .as_deref()
        .and_then(project_dormant_days)
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

    // ── filesystem fallback (the data #560's cap was missing) ────────

    /// A project directory whose every activity marker is backdated `days`.
    /// It gets its OWN `.git` so `last_activity_from_fs`' upward search stops
    /// here and never inherits the mtime of some ancestor repository the test
    /// machine happens to have.
    fn project_last_touched(root: &std::path::Path, name: &str, days: u64) -> std::path::PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join(".git")).expect("create project + .git");
        let when = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
        for marker in ["Cargo.toml", ".git/HEAD"] {
            let path = dir.join(marker);
            std::fs::write(&path, "x").expect("write marker");
            std::fs::File::options()
                .write(true)
                .open(&path)
                .expect("open marker")
                .set_modified(when)
                .expect("backdate marker");
        }
        dir
    }

    fn path_str(p: &std::path::Path) -> String {
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn filesystem_answers_for_a_project_absent_from_detected_projects() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dead = path_str(&project_last_touched(tmp.path(), "graveyard", 200));
        // Exactly the live shape: `detected_projects` holds no row for it,
        // because its relevance never cleared ace's 0.15 persistence gate.
        let l = ProjectLiveness::from_entries(&[]);
        let days = l.dormant_days(&dead).expect("the filesystem can still say");
        assert!((199..=201).contains(&days), "got {days}");
        assert!(l.is_dormant(&dead));
    }

    #[test]
    fn recorded_row_wins_and_costs_no_probe() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // On disk this project was touched today; the recorded row says 300.
        let active = path_str(&project_last_touched(tmp.path(), "recorded", 0));
        let l = ProjectLiveness::from_entries(&[(active.as_str(), 300)]);
        assert_eq!(
            l.dormant_days(&active),
            Some(300),
            "tier 1 is authoritative"
        );
        assert_eq!(l.fs_probe_count(), 0, "a recorded row must not touch disk");
    }

    #[test]
    fn nonexistent_path_stays_unknown_never_dormant() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ghost = path_str(&tmp.path().join("never-existed"));
        let l = ProjectLiveness::from_entries(&[]);
        assert_eq!(l.dormant_days(&ghost), None);
        assert!(
            !l.is_dormant(&ghost),
            "a path we cannot see is not proven dead"
        );
        assert!(!l.all_dormant(&[ghost]));
    }

    #[test]
    fn hits_are_memoized_one_probe_serves_every_lookup() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dead = path_str(&project_last_touched(tmp.path(), "graveyard", 200));
        let l = ProjectLiveness::from_entries(&[]);
        for _ in 0..50 {
            assert!(l.is_dormant(&dead));
        }
        assert_eq!(l.fs_probe_count(), 1, "50 lookups, one stat");

        // Strongest proof the answer came from the memo and not the disk:
        // delete the project and ask again.
        std::fs::remove_dir_all(tmp.path().join("graveyard")).expect("remove project");
        assert!(
            l.is_dormant(&dead),
            "served from the memo, not a fresh probe"
        );
        assert_eq!(l.fs_probe_count(), 1);
    }

    #[test]
    fn misses_are_memoized_too() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let later = path_str(&tmp.path().join("later"));
        let l = ProjectLiveness::from_entries(&[]);
        assert_eq!(l.dormant_days(&later), None);
        // Create the project AFTER the miss was cached. A cached miss must
        // cost nothing — an unresolvable path is one probe per instance, not
        // one per alert that cites it.
        project_last_touched(tmp.path(), "later", 200);
        assert_eq!(l.dormant_days(&later), None);
        assert_eq!(l.fs_probe_count(), 1);
    }

    #[test]
    fn probes_are_bounded_and_the_budget_fails_safe() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let l = ProjectLiveness::from_entries(&[]);
        for i in 0..(MAX_FS_LIVENESS_PROBES + 25) {
            assert_eq!(l.dormant_days(&format!("/no/such/project/{i}")), None);
        }
        assert_eq!(
            l.fs_probe_count(),
            MAX_FS_LIVENESS_PROBES,
            "distinct paths past the ceiling spend no further budget"
        );
        // Past the ceiling the answer is unknown — which never demotes.
        let dead = path_str(&project_last_touched(tmp.path(), "past-budget", 200));
        assert_eq!(l.dormant_days(&dead), None);
        assert!(!l.is_dormant(&dead));
    }

    // ── dormancy cap ─────────────────────────────────────────────────

    /// The 2026-08-31 live case, end to end: `next` / `turbo` upgrade steps
    /// ranked Critical citing repos idle since February. Their relevance
    /// (0.0737 measured) never cleared ace's 0.15 persistence gate, so
    /// `detected_projects` held nothing to cap them with and #560's cap was
    /// structurally inert. With the filesystem tier, it fires.
    #[test]
    fn dormant_by_filesystem_but_unrecorded_still_caps() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let a = path_str(&project_last_touched(tmp.path(), "4da-platform", 190));
        let b = path_str(&project_last_touched(tmp.path(), "vercel-workflow", 210));
        let l = ProjectLiveness::from_entries(&[]);
        let mut items = vec![item(
            Urgency::Critical,
            Confidence::osv_verified(0.95),
            &[a.as_str(), b.as_str()],
        )];
        assert_eq!(cap_dormant_items(&mut items, &l), 1);
        assert_eq!(items[0].urgency, Urgency::Medium);
        assert!(
            items[0].explanation.contains(&dormant_projects_note()),
            "explanation must say why: {}",
            items[0].explanation
        );
    }

    /// The other half of the same mechanism: one project the user still works
    /// in keeps the whole alert urgent, even when the filesystem — not a
    /// recorded row — is what proves it alive.
    #[test]
    fn one_filesystem_active_project_defeats_the_cap() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dead = path_str(&project_last_touched(tmp.path(), "graveyard", 200));
        let live = path_str(&project_last_touched(tmp.path(), "today", 0));
        let l = ProjectLiveness::from_entries(&[]);
        let mut items = vec![item(
            Urgency::Critical,
            Confidence::osv_verified(0.95),
            &[dead.as_str(), live.as_str()],
        )];
        assert_eq!(cap_dormant_items(&mut items, &l), 0);
        assert_eq!(items[0].urgency, Urgency::Critical);
    }

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
