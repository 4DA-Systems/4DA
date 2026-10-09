// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One knowledge-gap detection pass: everything the per-dependency loop
//! needs is loaded ONCE ([`GapScan::load`]), then each dependency is a
//! handful of lookups ([`GapScan::gap_for`]).

// UTF-8 safety gate, as in the parent module.
#![deny(clippy::string_slice)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use tracing::warn;

use crate::error::Result;
use crate::temporal::ProjectDependency;

use super::gaps_evidence::GapBasis;
use super::gaps_match::{CandidateIndex, Engagement};
use super::gaps_truth::{
    live_paths, registry_lang, triage_releases, version_label, PinBook, ReleaseTriage,
};
use super::{
    advisory_tier_for, affected_project_paths, build_tech_domain, classify_severity,
    dep_is_relevant, dep_name_is_matchable, get_active_project_paths, grounded_security_advisory,
    has_security_citation, installs_for, is_advisory_row, linked_to, load_gap_candidates,
    load_primary_stack, misses_among, normalize_project_path, still_vulnerable, GapCandidate,
    GapSeverity, KnowledgeGap, MissedItem, MAX_SCANNED_DEPS, NODE_BUILTINS,
};

type Exposed = Vec<(String, crate::osv::exposure::Install)>;

pub(super) struct GapScan<'a> {
    conn: &'a rusqlite::Connection,
    candidates: Vec<GapCandidate>,
    index: CandidateIndex,
    engagement: Engagement,
    pins: PinBook,
    domain: HashSet<String>,
    anti_deps: HashSet<String>,
    active_projects: Vec<String>,
    /// Stored scoring verdicts read so far, by item id (see `stored_verdict`).
    verdicts: RefCell<HashMap<i64, StoredVerdict>>,
}

/// A candidate's stored scoring-time version verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StoredVerdict {
    /// `Some(affected)`, or `None` when the pipeline recorded no verdict.
    Read(Option<bool>),
    /// A value that is neither NULL nor an integer. The eager load this
    /// replaced failed that ROW and dropped the candidate; the scan drops it
    /// the same way (reported as "not affected", which both consumers skip).
    Unreadable,
}

/// The value the eager candidate load read for `item_id`, via the primary key.
/// No breakdown row reads as no verdict. A query error (a missing table, a
/// malformed breakdown) also reads as no verdict here, where the eager load
/// failed the WHOLE detection — one bad row no longer blanks the panel.
fn read_stored_verdict(conn: &rusqlite::Connection, item_id: i64) -> StoredVerdict {
    let read = conn.query_row(
        "SELECT json_extract(breakdown, '$.breakdown.is_version_affected')
           FROM scoring_explanations WHERE source_item_id = ?1",
        [item_id],
        |row| {
            Ok(match row.get_ref(0)? {
                rusqlite::types::ValueRef::Null => StoredVerdict::Read(None),
                rusqlite::types::ValueRef::Integer(v) => StoredVerdict::Read(Some(v != 0)),
                _ => StoredVerdict::Unreadable,
            })
        },
    );
    read.unwrap_or(StoredVerdict::Read(None))
}

impl<'a> GapScan<'a> {
    pub(super) fn load(conn: &'a rusqlite::Connection) -> Result<Self> {
        let engagement = Engagement::load(conn);
        // "Unread" = no feedback AND no engagement interaction (clicks count).
        let mut candidates = load_gap_candidates(conn)?;
        candidates.retain(|c| !engagement.is_engaged(c.item.item_id));
        let index = CandidateIndex::build(&candidates);
        let primary_stack = load_primary_stack(conn);
        Ok(Self {
            conn,
            index,
            candidates,
            engagement,
            pins: PinBook::load(conn),
            domain: build_tech_domain(conn),
            anti_deps: crate::competing_tech::get_anti_dependencies(&primary_stack),
            // Normalized: git_signals stores OS-native paths, the dependency
            // tables lowercase forward-slash ones.
            active_projects: get_active_project_paths(conn)
                .iter()
                .map(|p| normalize_project_path(p))
                .collect(),
            verdicts: RefCell::new(HashMap::new()),
        })
    }

    /// The stored verdict for `c` — the value `version_affected` would carry
    /// had the load read it — fetched once per item, on first use.
    fn stored_verdict(&self, c: &GapCandidate) -> StoredVerdict {
        if c.version_affected.is_some() {
            return StoredVerdict::Read(c.version_affected);
        }
        let id = c.item.item_id;
        if let Some(v) = self.verdicts.borrow().get(&id) {
            return *v;
        }
        let v = read_stored_verdict(self.conn, id);
        self.verdicts.borrow_mut().insert(id, v);
        v
    }

    /// The verdict both consumers apply: the advisory mirror's LIVE answer
    /// for a linked advisory row (AD-045), else the stored one. An unreadable
    /// stored value excludes the row outright, as its failed load did.
    fn verdict(
        &self,
        c: &GapCandidate,
        name: &str,
        dep_lower: &str,
        installs: &[crate::osv::exposure::Install],
    ) -> Option<bool> {
        let StoredVerdict::Read(stored) = self.stored_verdict(c) else {
            return Some(false);
        };
        let live = if is_advisory_row(c) && linked_to(c, dep_lower) {
            crate::osv::exposure::advisory_row_reaches(self.conn, &c.source_id, name, installs)
        } else {
            None
        };
        live.or(stored)
    }

    /// Gaps for every dependency name, one per name, in first-seen order.
    pub(super) fn scan(&self, deps: &[ProjectDependency]) -> Vec<KnowledgeGap> {
        let mut order: Vec<&str> = Vec::new();
        let mut groups: std::collections::HashMap<&str, Vec<&ProjectDependency>> =
            std::collections::HashMap::new();
        for dep in deps {
            let entry = groups.entry(dep.package_name.as_str()).or_default();
            if entry.is_empty() {
                order.push(dep.package_name.as_str());
            }
            entry.push(dep);
        }
        let mut gaps = Vec::new();
        for (scanned, name) in order.iter().enumerate() {
            if scanned >= MAX_SCANNED_DEPS {
                // Runaway guard only, never a silent truncation.
                warn!(
                    target: "4da::knowledge_decay",
                    scanned = MAX_SCANNED_DEPS,
                    remaining = order.len() - scanned,
                    "Knowledge-gap scan hit its dependency ceiling — coverage is incomplete"
                );
                break;
            }
            let Some(group) = groups.get(name) else {
                continue;
            };
            if let Some(gap) = self.gap_for(group) {
                gaps.push(gap);
            }
        }
        gaps
    }

    /// The name/domain/stack/activity filters, unchanged from the original
    /// loop. `first` is the name's first declaring row.
    fn admits(&self, first: &ProjectDependency, paths: &[String]) -> bool {
        let name = first.package_name.as_str();
        if !dep_name_is_matchable(name)
            || name.starts_with("node:")
            || NODE_BUILTINS.contains(&name)
            || name.starts_with("content_")
            || name == "fourda-macros"
            || name == "nlp"
        {
            return false;
        }
        // Direct runtime deps ARE the user's stack; transitive/dev deps must
        // also sit in the onboarding domain.
        if !dep_is_relevant(first.is_direct, first.is_dev, name, &self.domain) {
            return false;
        }
        if self.anti_deps.contains(&name.to_lowercase()) {
            return false;
        }
        self.active_projects.is_empty()
            || self.active_projects.iter().any(|ap| {
                paths.iter().any(|dp| {
                    let dp = normalize_project_path(dp);
                    dp.contains(ap) || ap.contains(&dp)
                })
            })
    }

    /// The candidate rows naming this dependency, judged LIVE against its
    /// installs where the advisory mirror can (AD-045).
    fn misses(&self, name: &str, paths: &[String]) -> Vec<MissedItem> {
        let installs = installs_for(self.conn, name, paths);
        let dep_lower = name.to_lowercase();
        let live = |c: &GapCandidate| self.verdict(c, name, &dep_lower, &installs);
        let hits = self.index.matching(&self.candidates, &dep_lower);
        misses_among(hits.into_iter(), name, &live)
    }

    fn gap_for(&self, group: &[&ProjectDependency]) -> Option<KnowledgeGap> {
        let first = *group.first()?;
        let name = first.package_name.as_str();
        let paths: Vec<String> = group.iter().map(|d| d.project_path.clone()).collect();
        if !self.admits(first, &paths) {
            return None;
        }
        let mut langs: Vec<String> = group.iter().map(|d| registry_lang(&d.language)).collect();
        langs.sort();
        langs.dedup();
        let triage = triage_releases(
            self.conn,
            &self.pins,
            name,
            &langs,
            self.misses(name, &paths),
        );
        if triage.kept.is_empty() {
            return None;
        }
        let days_since = self.engagement.days_since(name);
        let (severity, exposed) = self.severity(first, &paths, &triage, days_since);
        if severity == GapSeverity::Low && days_since < 14 {
            return None;
        }
        Some(self.attribute(name, &paths, triage, exposed, severity, days_since))
    }

    /// Severity plus the projects an advisory still reaches.
    fn severity(
        &self,
        first: &ProjectDependency,
        paths: &[String],
        triage: &ReleaseTriage,
        days_since: u32,
    ) -> (GapSeverity, Exposed) {
        let name = first.package_name.as_str();
        let installs = installs_for(self.conn, name, paths);
        let dep_lower = name.to_lowercase();
        let live = |c: &GapCandidate| self.verdict(c, name, &dep_lower, &installs);
        let vulnerable = still_vulnerable(self.conn, name, first.version.as_deref(), paths)
            && grounded_security_advisory(&self.candidates, name, &live);
        let exposed = if vulnerable {
            affected_project_paths(self.conn, name, paths)
        } else {
            Vec::new()
        };
        let tier_installs: Vec<crate::osv::exposure::Install> = if exposed.is_empty() {
            first
                .version
                .iter()
                .map(|v| {
                    crate::osv::exposure::Install::new(Some(first.language.as_str()), v.clone())
                })
                .collect()
        } else {
            exposed.iter().map(|(_, install)| install.clone()).collect()
        };
        let tier = advisory_tier_for(self.conn, name, &tier_installs);
        let base = classify_severity(&triage.kept, days_since, name, vulnerable, tier);
        (release_adjusted(base, triage, name), exposed)
    }

    fn attribute(
        &self,
        name: &str,
        paths: &[String],
        triage: ReleaseTriage,
        exposed: Exposed,
        severity: GapSeverity,
        days_since: u32,
    ) -> KnowledgeGap {
        let (basis, projects, version) = if !exposed.is_empty() {
            let version = version_label(exposed.iter().map(|(_, i)| i.version.as_str()));
            let projects = exposed.into_iter().map(|(p, _)| p).collect();
            (GapBasis::Advisory, projects, version)
        } else if !triage.behind.is_empty() {
            let installed: Vec<String> = triage
                .behind
                .iter()
                .map(|b| b.installed.to_string())
                .collect();
            let version = version_label(installed.iter().map(String::as_str));
            let projects = triage.behind.iter().map(|b| b.path.clone()).collect();
            (GapBasis::Release, projects, version)
        } else {
            let projects = live_paths(&self.pins, paths);
            let installs = installs_for(self.conn, name, &projects);
            let version = version_label(installs.iter().map(|i| i.version.as_str()));
            (GapBasis::Editorial, projects, version)
        };
        KnowledgeGap {
            dependency: name.to_string(),
            version,
            project_path: display_label(&projects),
            latest_release: (basis == GapBasis::Release)
                .then(|| triage.latest.as_ref().map(ToString::to_string))
                .flatten(),
            projects,
            basis,
            missed_items: triage.kept,
            gap_severity: severity,
            days_since_last_engagement: days_since,
        }
    }
}

/// Release evidence moves the tier: a yanked pin is at least High; a gap
/// whose only consequence is a patch release is not news on its own (Low).
fn release_adjusted(base: GapSeverity, triage: &ReleaseTriage, name: &str) -> GapSeverity {
    use crate::scoring::release_grade::ReleaseClass;
    match triage.worst {
        Some(ReleaseClass::Yanked) if matches!(base, GapSeverity::Medium | GapSeverity::Low) => {
            GapSeverity::High
        }
        Some(ReleaseClass::Patch)
            if base == GapSeverity::Medium && !has_security_citation(&triage.kept, name) =>
        {
            GapSeverity::Low
        }
        _ => base,
    }
}

/// "d:/4da/relay" or "d:/4da/relay (+2 more)" — the legacy display string.
fn display_label(projects: &[String]) -> String {
    match projects {
        [] => String::new(),
        [only] => only.clone(),
        [first, rest @ ..] => format!("{first} (+{} more)", rest.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The deferred read returns exactly what the eager candidate load read:
    /// an integer verdict, NULL (no verdict, or no breakdown row), and — for
    /// any other value, which failed the eager row read — an exclusion.
    #[test]
    fn the_deferred_verdict_reads_what_the_eager_load_read() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        assert_eq!(
            read_stored_verdict(&conn, 1),
            StoredVerdict::Read(None),
            "no table: no verdict"
        );
        conn.execute_batch(
            r#"CREATE TABLE scoring_explanations (source_item_id INTEGER PRIMARY KEY, breakdown TEXT NOT NULL);
               INSERT INTO scoring_explanations VALUES
                 (1, '{"breakdown":{"is_version_affected":false}}'),
                 (2, '{"breakdown":{"is_version_affected":true}}'),
                 (3, '{"breakdown":{"strongly_grounded":true}}'),
                 (4, '{"breakdown":{"is_version_affected":"yes"}}'),
                 (5, '{"breakdown":{"is_version_affected":0.5}}');"#,
        )
        .unwrap();
        assert_eq!(
            read_stored_verdict(&conn, 1),
            StoredVerdict::Read(Some(false))
        );
        assert_eq!(
            read_stored_verdict(&conn, 2),
            StoredVerdict::Read(Some(true))
        );
        assert_eq!(read_stored_verdict(&conn, 3), StoredVerdict::Read(None));
        assert_eq!(read_stored_verdict(&conn, 4), StoredVerdict::Unreadable);
        assert_eq!(read_stored_verdict(&conn, 5), StoredVerdict::Unreadable);
        assert_eq!(read_stored_verdict(&conn, 99), StoredVerdict::Read(None));
        // The eager load's own read of the same rows: a value it could not
        // read as an integer failed the row (here: `None`, the row dropped).
        for (id, want) in [
            (1, Some(Some(false))),
            (2, Some(Some(true))),
            (3, Some(None)),
            (4, None),
            (5, None),
        ] {
            let eager: Option<Option<bool>> = conn
                .query_row(
                    "SELECT json_extract(breakdown, '$.breakdown.is_version_affected')
                       FROM scoring_explanations WHERE source_item_id = ?1",
                    [id],
                    |r| r.get::<_, Option<i64>>(0),
                )
                .ok()
                .map(|v| v.map(|v| v != 0));
            assert_eq!(eager, want, "row {id}");
        }
    }
}
