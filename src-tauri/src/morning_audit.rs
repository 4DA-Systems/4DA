// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! What a delivered morning showed, audited when it is shown (AD-054 gate G4).
//!
//! G4 asks for 14 consecutive mornings with zero shown version faults. Until
//! now that was only measurable by replaying a database copy
//! (`live_snapshot_morning_replay`), so the gate could not be read from
//! stored data. Each delivered morning now records, next to its outcome in
//! `morning_brief_outcomes_v1`:
//!
//! - `versions_shown`: every `package version` the window's own text showed:
//!   the facts view, the written summary, and the fresh alert cards;
//! - `version_faults`: the shown versions that are not real, by the replay's
//!   check ([`version_is_true`]: installed in some project, a fixed version
//!   OSV lists for the package, or a release the registry feed carries);
//! - `items_shown` / `cap`: act-now facts the facts view named as new, against
//!   [`crate::morning_floor::MORNING_ACT_NOW_SHOWN`].
//!
//! Feed item titles are third-party text, not 4DA's claims, so they are not
//! audited; a summary quoting a version from its own input is grounded.

use rusqlite::Connection;

use crate::brief_facts::{BriefFacts, FixPath};
use crate::briefing_groundedness::looks_like_version_token;
use crate::monitoring_briefing::BriefingNotification;
use crate::morning_floor::OutcomeRecord;

/// The shown text of an alert card, as `public/briefing.js` truncates it.
const CARD_TITLE_CHARS: usize = 70;
const CARD_EXPLANATION_CHARS: usize = 160;

/// The audit of one delivered morning.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MorningAudit {
    /// `package version`, deduplicated, in the order first shown.
    pub versions_shown: Vec<String>,
    /// `<where>: package version`, one per shown version that is not real.
    pub version_faults: Vec<String>,
    /// New act-now facts the facts view named (0 when no facts view).
    pub items_shown: usize,
    pub cap: usize,
}

impl MorningAudit {
    fn shown(&mut self, package: &str, version: &str, what: &str, real: bool) {
        let pv = format!("{package} {version}");
        if !self.versions_shown.contains(&pv) {
            self.versions_shown.push(pv.clone());
        }
        let fault = format!("{what}: {pv}");
        if !real && !self.version_faults.contains(&fault) {
            self.version_faults.push(fault);
        }
    }
}

/// Is `version` a real version of `package`: installed in some project, a
/// fixed version OSV lists for it, a release the registry feed carries, or a
/// release the stored registry facts list (`osv::release_facts`)?
/// The G4 replay's check, shared so the gate and the replay agree.
pub(crate) fn version_is_true(conn: &Connection, package: &str, version: &str) -> bool {
    let q = |sql: &str| -> bool {
        conn.query_row(sql, rusqlite::params![package, version], |_| Ok(()))
            .is_ok()
    };
    q("SELECT 1 FROM dependency_instances WHERE LOWER(package_name) = LOWER(?1) AND version = ?2 LIMIT 1")
        || q("SELECT 1 FROM osv_advisories WHERE LOWER(package_name) = LOWER(?1)
               AND fixed_versions LIKE '%\"' || ?2 || '\"%' LIMIT 1")
        || q("SELECT 1 FROM source_items WHERE title LIKE '%' || ?1 || ' v' || ?2 || '%' LIMIT 1")
        // A release the registry lists (FX-H5: a target past a withdrawn fix).
        || crate::osv::release_facts::registry_lists(conn, package, version)
}

/// Every `(package, version, what)` the facts carry, in render order.
pub(crate) fn fact_versions(facts: &BriefFacts) -> Vec<(String, String, &'static str)> {
    let mut out = Vec::new();
    let mut push = |p: &str, v: &str, what: &'static str| {
        out.push((p.to_string(), v.to_string(), what));
    };
    for f in facts.security.iter().chain(facts.also_open.iter()) {
        for s in &f.sites {
            if let Some(i) = &s.installed {
                push(&f.package, i, "installed");
            }
            match &s.fix_path {
                FixPath::Bump { to }
                | FixPath::Refresh { to, .. }
                | FixPath::ParentUnknown { to }
                | FixPath::Reinstall { to }
                | FixPath::Update { to } => push(&f.package, to, "fix"),
                FixPath::Parent {
                    parent,
                    parent_version,
                    to,
                    proven,
                    ..
                } => {
                    push(&f.package, to, "fix");
                    push(parent, parent_version, "parent installed");
                    if let Some(p) = proven {
                        push(parent, &p.parent_version, "proven parent");
                        push(&f.package, &p.child_version, "proven child");
                    }
                }
                FixPath::PerLine { lines } => {
                    for l in lines {
                        push(&f.package, &l.to, "line fix");
                    }
                }
                FixPath::NoFix => {}
            }
        }
    }
    for u in &facts.upgrades {
        push(&u.package, &u.announced, "announced");
        for s in &u.sites {
            push(&u.package, &s.installed, "upgrade installed");
        }
    }
    out
}

/// Does `text` show `version` as a whole token (so "1.2" is not found
/// inside "11.2.0")?
fn shows_version(text: &str, version: &str) -> bool {
    let v = version.trim();
    if v.is_empty() {
        return false;
    }
    text.match_indices(v).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + v.len()..].chars().next();
        let edge = |c: Option<char>| c.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '.'));
        // A trailing sentence dot is an edge, a dot followed by a digit is not.
        let after_ok = edge(after)
            || (after == Some('.')
                && !text[i + v.len() + 1..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit()));
        edge(before) && after_ok
    })
}

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// A version field (`">= 4.13.5"`, `"4.13.5, 5.0.1"`) as its version tokens.
fn version_tokens(field: &str) -> Vec<String> {
    field
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|t| t.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
        .map(|t| t.trim_start_matches('v'))
        .filter(|t| looks_like_version_token(t))
        .map(str::to_string)
        .collect()
}

/// Package-attributed version claims in prose: a version token after a
/// package mention, in the same sentence, before the next package mention
/// (the attribution `check_factual_claims` uses, without its upgrade-intent
/// filter: every shown version is audited, not only upgrade targets).
pub(crate) fn prose_version_claims(prose: &str, packages: &[String]) -> Vec<(String, String)> {
    let norm = |t: &str| -> String {
        t.trim_matches(|c: char| !c.is_alphanumeric() && !matches!(c, '@' | '/' | '-' | '_' | '.'))
            .trim_matches(|c: char| matches!(c, '.' | '-' | '_'))
            .to_lowercase()
    };
    let names: Vec<(String, Vec<String>)> = packages
        .iter()
        .filter(|p| p.trim().chars().count() >= 2)
        .map(|p| {
            let l = p.trim().to_lowercase();
            let mut aliases = vec![l.clone()];
            if let Some(i) = l.rfind('/') {
                aliases.push(l[i + 1..].to_string());
            }
            (p.trim().to_string(), aliases)
        })
        .collect();
    let tokens: Vec<&str> = prose.split_whitespace().collect();
    let pkg_at: Vec<Option<usize>> = tokens
        .iter()
        .map(|t| {
            let n = norm(t);
            names.iter().position(|(_, a)| a.contains(&n))
        })
        .collect();
    let ends = |t: &str| t.ends_with(['.', '!', '?']);
    let mut claims = Vec::new();
    for (i, at) in pkg_at.iter().enumerate() {
        let Some(idx) = *at else { continue };
        if ends(tokens[i]) {
            continue;
        }
        for (pos, raw) in tokens.iter().enumerate().skip(i + 1) {
            if pkg_at[pos].is_some() {
                break;
            }
            let v = raw
                .trim_matches(|c: char| !c.is_ascii_digit() && c != '.')
                .trim_matches('.');
            if looks_like_version_token(v) {
                let claim = (names[idx].0.clone(), v.to_string());
                if !claims.contains(&claim) {
                    claims.push(claim);
                }
            }
            if ends(raw) {
                break;
            }
        }
    }
    claims
}

/// Audit what the morning window showed. `floor_facts`: the facts the facts
/// view was rendered from (after the act-now cap), when it went out.
pub(crate) fn audit_morning(
    conn: &Connection,
    briefing: &BriefingNotification,
    floor_facts: Option<&BriefFacts>,
    cap: usize,
) -> MorningAudit {
    let mut audit = MorningAudit {
        cap,
        ..MorningAudit::default()
    };

    // The facts view: structured facts, kept to the versions its text shows.
    if let (Some(facts), Some(markdown)) = (floor_facts, briefing.facts_brief.as_deref()) {
        audit.items_shown = facts.security.iter().filter(|f| f.status.is_new()).count();
        for (p, v, what) in fact_versions(facts) {
            if shows_version(markdown, &v) {
                let real = version_is_true(conn, &p, &v);
                audit.shown(&p, &v, what, real);
            }
        }
    }

    // Fresh alert cards (title + explanation, as the window truncates them).
    // Long-unchanged alerts collapse into a strip of names: no versions.
    for a in briefing
        .preemption_alerts
        .iter()
        .filter(|a| !a.persistent_unchanged)
    {
        let shown = format!(
            "{} {}",
            truncate_chars(&a.title, CARD_TITLE_CHARS),
            truncate_chars(&a.explanation, CARD_EXPLANATION_CHARS)
        );
        let mut fields: Vec<(String, Option<String>, Option<String>)> = Vec::new();
        if let Some(p) = &a.package_name {
            fields.push((
                p.clone(),
                a.installed_version.clone(),
                a.fixed_version.clone(),
            ));
        }
        for m in &a.merged_package_versions {
            fields.push((
                m.package_name.clone(),
                m.installed_version.clone(),
                m.fixed_version.clone(),
            ));
        }
        for (pkg, installed, fixed) in fields {
            for (field, what) in [(installed, "card installed"), (fixed, "card fix")] {
                for v in field.as_deref().map(version_tokens).unwrap_or_default() {
                    if shows_version(&shown, &v) {
                        let real = version_is_true(conn, &pkg, &v);
                        audit.shown(&pkg, &v, what, real);
                    }
                }
            }
        }
    }

    // The written summary, when it was the one shown.
    if briefing.facts_brief.is_none() {
        if let Some(prose) = briefing.synthesis.as_deref().filter(|s| {
            !crate::monitoring_briefing::is_abstention_synthesis(s)
                && !crate::morning_floor::window_folds(s)
        }) {
            let mut packages: Vec<String> = Vec::new();
            let mut input = String::new();
            for i in &briefing.items {
                packages.extend(i.matched_deps.iter().cloned());
                input.push_str(&i.title);
                input.push('\n');
                input.push_str(i.description.as_deref().unwrap_or(""));
                input.push('\n');
            }
            for a in &briefing.preemption_alerts {
                if let Some(p) = &a.package_name {
                    packages.extend(p.split(", ").map(str::to_string));
                }
                for m in &a.merged_package_versions {
                    packages.push(m.package_name.clone());
                }
                for s in [&a.title, &a.explanation] {
                    input.push_str(s);
                    input.push('\n');
                }
                for v in [&a.installed_version, &a.fixed_version]
                    .into_iter()
                    .flatten()
                {
                    input.push_str(v);
                    input.push('\n');
                }
            }
            for (p, v) in prose_version_claims(prose, &packages) {
                // A version quoted from the summary's own input is grounded.
                let real = shows_version(&input, &v) || version_is_true(conn, &p, &v);
                audit.shown(&p, &v, "summary", real);
            }
        }
    }
    audit
}

impl OutcomeRecord {
    /// Put the audit on the day's record: the fields, or why it is missing.
    pub(crate) fn set_audit(&mut self, audit: Option<&Result<MorningAudit, String>>) {
        match audit {
            Some(Ok(a)) => {
                self.versions_shown = Some(a.versions_shown.clone());
                self.version_faults = Some(a.version_faults.clone());
                self.items_shown = Some(a.items_shown);
                self.cap = Some(a.cap);
                self.audit_error = None;
            }
            Some(Err(e)) => self.audit_error = Some(e.clone()),
            None => {}
        }
    }

    /// Fold an earlier delivery of the same day into this one: a fault shown
    /// on any delivery stays on the day, a day with an unaudited delivery
    /// stays unknown, and a Brief is not demoted by a later items-only window.
    pub(crate) fn merge_earlier(&mut self, earlier: &OutcomeRecord) {
        let union = |a: &Option<Vec<String>>, b: &Option<Vec<String>>| match (a, b) {
            (Some(a), Some(b)) => {
                let mut out = b.clone();
                out.extend(a.iter().filter(|x| !b.contains(x)).cloned());
                Some(out)
            }
            _ => None,
        };
        self.versions_shown = union(&self.versions_shown, &earlier.versions_shown);
        self.version_faults = union(&self.version_faults, &earlier.version_faults);
        self.items_shown = match (self.items_shown, earlier.items_shown) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => None,
        };
        if self.audit_error.is_none() {
            self.audit_error.clone_from(&earlier.audit_error);
        }
        if earlier.brief_delivered() && !self.brief_delivered() {
            self.outcome.clone_from(&earlier.outcome);
            self.reason.clone_from(&earlier.reason);
            self.path.clone_from(&earlier.path);
        }
    }
}

#[cfg(test)]
#[path = "morning_audit_tests.rs"]
mod tests;
