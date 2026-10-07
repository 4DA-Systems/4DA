// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Per-pass lookups for knowledge-gap detection, built ONCE and reused by
//! every dependency: a token index over candidate titles, and the user's
//! engagement history.
//!
//! Measured live 2026-10-07: 183 dependencies × 71,311 candidate titles were
//! substring-scanned one dependency at a time, and with one `feedback` row
//! every dependency then fell through to an ACE lock plus a `detected_tech`
//! query. Both are now one load per pass.

// UTF-8 safety gate, as in the parent module.
#![deny(clippy::string_slice)]

use std::collections::{HashMap, HashSet};

use super::GapCandidate;

/// Interaction kinds that mean the user actually looked at an item. `scroll`
/// and `ignore` are passive or automatic and do not make an item read.
const ENGAGED_ACTIONS: &[&str] = &[
    "click",
    "save",
    "share",
    "dismiss",
    "bookmark",
    "open",
    "engagement_complete",
    "accuracy_feedback",
];

/// Maximal alphanumeric runs of `s` — exactly the units the word-boundary
/// matcher (`utils::has_word_boundary_match_with_ext`) treats as words.
fn alnum_runs(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
}

/// The original per-candidate predicate: a cheap substring reject, then the
/// word-boundary walk.
pub(super) fn title_names(title_lower: &str, dep_lower: &str) -> bool {
    title_lower.contains(dep_lower)
        && crate::utils::has_word_boundary_match_with_ext(title_lower, dep_lower)
}

/// Title run → indices of the candidates whose title contains that run.
///
/// Why a lookup is exact: a word-boundary match of a name requires a
/// non-alphanumeric char (or the string edge) on both sides, and the name's
/// own punctuation bounds its interior runs. So every alphanumeric run of
/// the name appears as a WHOLE run of any title that names it, and the
/// posting list of the name's rarest run is a superset of the matches. Each
/// posting is then confirmed with [`title_names`], so the result is the old
/// scan's result, in the same (recency) order.
pub(super) struct CandidateIndex {
    by_run: HashMap<String, Vec<usize>>,
}

impl CandidateIndex {
    pub(super) fn build(candidates: &[GapCandidate]) -> Self {
        let mut by_run: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, c) in candidates.iter().enumerate() {
            let mut seen: HashSet<&str> = HashSet::new();
            for run in alnum_runs(&c.title_lower) {
                if seen.insert(run) {
                    by_run.entry(run.to_string()).or_default().push(i);
                }
            }
        }
        Self { by_run }
    }

    /// Candidates (ascending index = the load's recency order) whose title
    /// names `dep_lower`.
    pub(super) fn matching<'c>(
        &self,
        candidates: &'c [GapCandidate],
        dep_lower: &str,
    ) -> Vec<&'c GapCandidate> {
        let runs: Vec<&str> = alnum_runs(dep_lower).collect();
        if runs.is_empty() {
            // A name with no alphanumeric run cannot be indexed: scan.
            return candidates
                .iter()
                .filter(|c| title_names(&c.title_lower, dep_lower))
                .collect();
        }
        // A run absent from every title rules every title out.
        let Some(postings) = runs
            .iter()
            .map(|r| self.by_run.get(*r))
            .min_by_key(|p| p.map_or(0, Vec::len))
            .flatten()
        else {
            return Vec::new();
        };
        postings
            .iter()
            .filter_map(|&i| candidates.get(i))
            .filter(|c| title_names(&c.title_lower, dep_lower))
            .collect()
    }
}

/// What the user has engaged with — `feedback` rows AND engagement
/// `interactions` (clicks, saves, ...). Live 2026-10-07 `feedback` held one
/// row while 17 recorded clicks were ignored, so every item read as unread.
#[derive(Default)]
pub(super) struct Engagement {
    item_ids: HashSet<i64>,
    /// (lowercased title, timestamp) of every engaged item.
    titles: Vec<(String, String)>,
    /// ACE-detected tech names, lowercased — hoisted out of the per-dependency
    /// loop, where it cost one ACE lock and one query per dependency.
    detected_tech: HashSet<String>,
}

impl Engagement {
    pub(super) fn load(conn: &rusqlite::Connection) -> Self {
        let mut out = Self::default();
        let feedback = "SELECT f.source_item_id, LOWER(si.title), f.created_at
             FROM feedback f JOIN source_items si ON si.id = f.source_item_id";
        out.absorb(conn, feedback, &[]);
        let placeholders = vec!["?"; ENGAGED_ACTIONS.len()].join(",");
        let interactions = format!(
            "SELECT si.id, LOWER(si.title), COALESCE(i.timestamp, '')
             FROM interactions i
             JOIN source_items si ON si.id = COALESCE(i.item_id, i.source_item_id)
             WHERE COALESCE(i.action_type, i.action) IN ({placeholders})"
        );
        out.absorb(conn, &interactions, ENGAGED_ACTIONS);
        if let Ok(ace) = crate::get_ace_engine() {
            if let Ok(techs) = ace.get_detected_tech() {
                out.detected_tech = techs.iter().map(|t| t.name.to_lowercase()).collect();
            }
        }
        out
    }

    /// Best-effort: a missing table (fresh DB, test fixture) adds nothing.
    fn absorb(&mut self, conn: &rusqlite::Connection, sql: &str, params: &[&str]) {
        let Ok(mut stmt) = conn.prepare(sql) else {
            return;
        };
        let Ok(rows) = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        }) else {
            return;
        };
        for (id, title, at) in rows.flatten() {
            self.item_ids.insert(id);
            if let Some(title) = title {
                self.titles.push((title, at.unwrap_or_default()));
            }
        }
    }

    /// Test constructor.
    #[cfg(test)]
    pub(super) fn from_parts(ids: &[i64], titles: &[(&str, &str)], tech: &[&str]) -> Self {
        Self {
            item_ids: ids.iter().copied().collect(),
            titles: titles
                .iter()
                .map(|(t, at)| (t.to_lowercase(), (*at).to_string()))
                .collect(),
            detected_tech: tech.iter().map(|t| t.to_lowercase()).collect(),
        }
    }

    pub(super) fn is_engaged(&self, item_id: i64) -> bool {
        self.item_ids.contains(&item_id)
    }

    /// Days since the user last engaged with an item whose title names the
    /// package; 0 when ACE detects the tech in their projects; 999 for never.
    pub(super) fn days_since(&self, package_name: &str) -> u32 {
        let pkg = package_name.to_lowercase();
        let latest = self
            .titles
            .iter()
            .filter(|(title, _)| title.contains(&pkg))
            .map(|(_, at)| at.as_str())
            .max();
        if let Some(at) = latest {
            return match chrono::NaiveDateTime::parse_from_str(at, "%Y-%m-%d %H:%M:%S") {
                Ok(date) => {
                    let now = chrono::Utc::now().naive_utc();
                    (now - date).num_days().max(0) as u32
                }
                Err(_) => 999,
            };
        }
        if self.detected_tech.contains(&pkg) {
            return 0;
        }
        999
    }
}

#[cfg(test)]
#[path = "knowledge_gaps_match_tests.rs"]
mod tests;
