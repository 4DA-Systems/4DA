// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One knowledge-gap detection pass: everything the per-dependency loop
//! needs is loaded ONCE ([`GapScan::load`]), then each dependency is a
//! handful of lookups ([`GapScan::gap_for`]).

// UTF-8 safety gate, as in the parent module.
#![deny(clippy::string_slice)]

use std::collections::HashSet;

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
        })
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
        let conn = self.conn;
        let live = |c: &GapCandidate| -> Option<bool> {
            if is_advisory_row(c) && linked_to(c, &dep_lower) {
                crate::osv::exposure::advisory_row_reaches(conn, &c.source_id, name, &installs)
            } else {
                None
            }
        };
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
        let conn = self.conn;
        let live = |c: &GapCandidate| -> Option<bool> {
            if is_advisory_row(c) && linked_to(c, &dep_lower) {
                crate::osv::exposure::advisory_row_reaches(conn, &c.source_id, name, &installs)
            } else {
                None
            }
        };
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
