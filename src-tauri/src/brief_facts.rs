// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Brief facts: the deterministic layer the Brief is written from.
//!
//! Decision 2 (2026-10-02). The 2026-10-01 audit of ten consecutive briefs
//! found the model doing a database's job, and getting it wrong:
//!
//! - "rmcp: a lockfile refresh should resolve it", in 10 of 10 briefs. rmcp
//!   1.7.0 reaches the atlas bridge through victauri-plugin 0.8.4, whose
//!   0.8.4 line requires rmcp 1.x. No refresh crosses 1.x -> 2.x; the parent
//!   has to move.
//! - fastembed 7.1.0 (the app runs 5.17.4, two majors behind its embedding
//!   engine) was never mentioned, while axum 0.8.9 (April, already
//!   installed) and TypeScript 6.0 (March) were presented as news.
//! - victauri 0.9.0, the operator's OWN release, was presented as news.
//! - 73% of topics repeated the previous brief; rmcp led every one.
//!
//! Every one of those was a fact 4DA already held. This module computes them,
//! so the prompt STATES them and the model only explains them:
//!
//! - [`SecurityFact`]: version-confirmed advisories (the Preemption feed's own
//!   alerts, so the two surfaces cannot disagree), with the fix path worked
//!   out from the lockfile graph, the worst advisory tier, scratch and
//!   dormant projects named as such.
//! - [`UpgradeFact`]: breaking or yanked releases of DIRECT dependencies
//!   (`release_grade`), latest version per package, real publish date, never
//!   the user's own packages and never a version already installed.
//! - [`WorthKnowingCandidate`]: judge-passed editorial items published in the
//!   last week and never featured on an earlier day, with a body excerpt.
//! - [`Novelty`]: what was already reported and since when, so an unchanged
//!   fact is folded into one line instead of re-narrated every brief.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::preemption::AlertUrgency;
use crate::scoring::release_version::lenient_semver;

/// Registry releases older than this are not "news" for the brief: a
/// breaking release from May belongs to the upgrade plan, not today's read.
pub(crate) const UPGRADE_WINDOW_DAYS: i64 = 30;
/// Editorial items older than this are not "worth knowing" today. Measured
/// 2026-10-01: 27% of Worth Knowing bullets cited items over 7 days old.
pub(crate) const WORTH_KNOWING_WINDOW_DAYS: i64 = 7;
/// Candidates handed to the model; it features at most five.
pub(crate) const WORTH_KNOWING_CANDIDATES: usize = 12;
/// The judge relevance a candidate needs when a judgment exists: the feed
/// gate's own bar (#752, measured 2026-09-27; lower bars add items at 8-11%
/// precision).
const WORTH_KNOWING_JUDGE_BAR: f64 = 0.5;
/// Lower-severity security lines the brief lists before "+N more".
pub(crate) const ALSO_OPEN_SHOWN: usize = 6;
/// Characters of article body the model reads per candidate.
const EXCERPT_CHARS: usize = 600;
/// Upgrade facts shown per brief (runtime, then dev tooling).
const MAX_UPGRADES: usize = 8;
/// How far up the lockfile graph the fix-path walk climbs.
const MAX_PARENT_DEPTH: usize = 6;
/// `kv_store` key for the novelty record.
const NOVELTY_KV_KEY: &str = "brief_novelty_v1";
/// Novelty entries older than this are forgotten.
const NOVELTY_RETENTION_DAYS: i64 = 45;

// ============================================================================
// Fact types
// ============================================================================

/// How the fix for one installed copy actually arrives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum FixPath {
    /// The project declares the package: bump it.
    Bump { to: String },
    /// Transitive, and every parent's requirement admits the fix (or the
    /// jump is semver-compatible): refreshing the lockfile reaches it.
    Refresh { to: String },
    /// Transitive, and the parent that pulls it in pins an older line:
    /// that DIRECT dependency has to move. `by_requirement` is true when the
    /// lockfile recorded the requirement and it excludes the fix (npm
    /// package-lock); false when inferred from a semver-incompatible jump
    /// (Cargo.lock and pnpm record resolved versions, not requirements).
    Parent {
        parent: String,
        parent_version: String,
        to: String,
        by_requirement: bool,
    },
    /// Transitive, the jump is semver-incompatible, the parent is unknown.
    ParentUnknown { to: String },
    /// The lockfile already pins the fix; node_modules still runs the old
    /// copy (install drift). Reinstalling is the whole fix.
    Reinstall { to: String },
    /// The advisory lists no fixed version.
    NoFix,
    /// A fix exists, scope unknown.
    Update { to: String },
}

/// One project's copy of a vulnerable package.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SecuritySite {
    pub label: String,
    pub installed: Option<String>,
    pub dev_only: bool,
    pub scratch: bool,
    pub dormant_days: Option<i64>,
    pub fix_path: FixPath,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SecurityFact {
    /// Stable identity for novelty: `ecosystem:package:labels`.
    pub key: String,
    pub package: String,
    pub ecosystem: String,
    /// The Preemption tab's urgency (scope-adjusted, AD-046).
    pub urgency: AlertUrgency,
    /// Worst advisory tier BEFORE the scope rule ("critical" for vitest's
    /// GHSA-5xrq even though a dev-only install ranks it High).
    pub worst_tier: Option<String>,
    pub advisory_count: usize,
    pub advisory_ids: Vec<String>,
    pub title: String,
    pub sites: Vec<SecuritySite>,
    pub first_seen: Option<String>,
    pub status: FactStatus,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpgradeSite {
    pub label: String,
    pub installed: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpgradeFact {
    pub key: String,
    pub package: String,
    pub ecosystem: String,
    pub announced: String,
    pub published: Option<String>,
    pub yanked: bool,
    /// Every concerned pin is a dev dependency (tooling news).
    pub dev_only: bool,
    /// Breaking releases between the lowest pin and the announcement (a 0.x
    /// minor counts, Cargo's compatibility unit).
    pub majors_behind: u64,
    /// The package is below 1.0, where every minor is a breaking release;
    /// "3 major versions behind" would overstate lopdf 0.42 -> 0.45.
    pub pre_one: bool,
    pub sites: Vec<UpgradeSite>,
    pub item_id: i64,
    pub url: Option<String>,
    pub status: FactStatus,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WorthKnowingCandidate {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub published: String,
    pub excerpt: String,
}

/// Whether the brief has told the user this fact before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum FactStatus {
    New,
    /// Reported before and unchanged since `since` (a date).
    Unchanged {
        since: String,
    },
}

impl FactStatus {
    pub(crate) fn is_new(&self) -> bool {
        matches!(self, Self::New)
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct BriefFacts {
    pub security: Vec<SecurityFact>,
    /// Medium-tier security facts: one line at most in the brief.
    pub also_open: Vec<SecurityFact>,
    pub upgrades: Vec<UpgradeFact>,
    pub worth_knowing: Vec<WorthKnowingCandidate>,
    /// Identity of the act-now + upgrade set. Changes when the facts that
    /// would change the brief change, not when one more article arrives.
    pub fingerprint: String,
}

// ============================================================================
// Pure rules (unit-tested in brief_facts_tests.rs)
// ============================================================================

/// Is `target` reachable from `installed` without crossing a semver
/// compatibility boundary? Caret rule shared by Cargo and npm: same major at
/// 1.x and above, same minor below 1.0. `None` when either is unparseable.
pub(crate) fn semver_compatible(installed: &str, target: &str) -> Option<bool> {
    let i = lenient_semver(installed, None)?;
    let t = lenient_semver(target, None)?;
    Some(if i.major == 0 {
        t.major == 0 && t.minor == i.minor
    } else {
        t.major == i.major
    })
}

/// Does an npm-style requirement admit `version`? `None` when the
/// requirement cannot be read (then the caller falls back to
/// [`semver_compatible`]). npm spells an exact pin as a bare version
/// (`5.28.4`), unions with `||`, ANDs comparators with spaces, allows a space
/// after an operator (`>= 4.21.0`) and writes hyphen ranges (`1 - 3`);
/// Cargo's parser wants `=`, one requirement per branch, and commas.
pub(crate) fn requirement_admits(requirement: &str, version: &str) -> Option<bool> {
    let v = lenient_semver(version, None)?;
    let req = requirement.trim();
    if req.is_empty() || req == "*" || req == "latest" || req == "x" {
        return Some(true);
    }
    let mut any_parsed = false;
    for branch in req.split("||") {
        let Some(normalized) = normalize_npm_branch(branch.trim()) else {
            continue;
        };
        if let Ok(parsed) = semver::VersionReq::parse(&normalized) {
            any_parsed = true;
            if parsed.matches(&v) {
                return Some(true);
            }
        }
    }
    any_parsed.then_some(false)
}

/// One npm range branch in Cargo's requirement syntax, or `None` when the
/// branch is a shape this cannot translate faithfully.
fn normalize_npm_branch(branch: &str) -> Option<String> {
    // Hyphen range: "1.2.3 - 2.3.4" means >=1.2.3 <=2.3.4 (partials widen
    // the upper bound; only full versions are translated).
    if let Some((lo, hi)) = branch.split_once(" - ") {
        let full = |s: &str| s.trim().split('.').count() == 3;
        return (full(lo) && full(hi)).then(|| format!(">={}, <={}", lo.trim(), hi.trim()));
    }
    // Join a bare operator to the version after it: ">= 4.21.0" -> ">=4.21.0".
    let mut tokens: Vec<String> = Vec::new();
    let mut pending: Option<String> = None;
    for raw in branch.split_whitespace() {
        let is_op = raw
            .chars()
            .all(|c| matches!(c, '<' | '>' | '=' | '~' | '^'));
        if is_op {
            pending = Some(raw.to_string());
            continue;
        }
        tokens.push(match pending.take() {
            Some(op) => format!("{op}{raw}"),
            None => raw.to_string(),
        });
    }
    if pending.is_some() {
        return None;
    }
    let parts: Vec<String> = tokens
        .into_iter()
        .map(|p| {
            let p = p.replace(".x", ".*").replace(".X", ".*");
            if !p.starts_with(|c: char| c.is_ascii_digit()) || p.contains('*') {
                return p;
            }
            // npm: a full bare version is exact; "1.2" is 1.2.x; "1" is 1.x
            // (which Cargo's caret already means).
            let core = p.split(['-', '+']).next().unwrap_or(&p);
            match core.split('.').count() {
                3 => format!("={p}"),
                2 => format!("~{p}"),
                _ => p,
            }
        })
        .collect();
    Some(parts.join(", "))
}

/// Work out how the fix reaches one installed copy.
///
/// `parent` is the DIRECT dependency the copy is reached through, with the
/// requirement the lockfile recorded, if any (npm package-lock only).
pub(crate) fn fix_path(
    installed: Option<&str>,
    fix: Option<&str>,
    is_direct: Option<bool>,
    parent: Option<&ParentLink>,
) -> FixPath {
    let Some(to) = fix.map(str::to_string) else {
        return FixPath::NoFix;
    };
    match is_direct {
        Some(true) => FixPath::Bump { to },
        None => FixPath::Update { to },
        Some(false) => {
            // The parent's own requirement is the strongest evidence.
            if let Some(link) = parent {
                if let Some(req) = link.requirement.as_deref() {
                    match requirement_admits(req, &to) {
                        Some(true) => return FixPath::Refresh { to },
                        Some(false) => {
                            return FixPath::Parent {
                                parent: link.direct.clone(),
                                parent_version: link.direct_version.clone(),
                                to,
                                by_requirement: true,
                            }
                        }
                        None => {}
                    }
                }
            }
            match installed.and_then(|i| semver_compatible(i, &to)) {
                Some(true) => FixPath::Refresh { to },
                Some(false) => match parent {
                    Some(link) => FixPath::Parent {
                        parent: link.direct.clone(),
                        parent_version: link.direct_version.clone(),
                        to,
                        by_requirement: false,
                    },
                    None => FixPath::ParentUnknown { to },
                },
                None => FixPath::Refresh { to },
            }
        }
    }
}

/// The direct dependency a transitive copy is reached through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParentLink {
    /// The direct dependency at the top of the chain.
    pub direct: String,
    pub direct_version: String,
    /// The requirement the IMMEDIATE parent places on the vulnerable
    /// package, when the lockfile records one (npm package-lock only).
    pub requirement: Option<String>,
}

/// One line of fix advice, worded for both the prompt and the floor.
pub(crate) fn fix_clause(path: &FixPath) -> String {
    match path {
        FixPath::Bump { to } => format!("bump it to >= {to}"),
        FixPath::Refresh { to } => {
            format!("transitive; refreshing the lockfile reaches >= {to} (no manifest change)")
        }
        FixPath::Parent {
            parent,
            parent_version,
            to,
            by_requirement: true,
        } => format!(
            "transitive via {parent} {parent_version}, whose requirement excludes the fix (>= {to}); \
             a lockfile refresh will NOT fix it: upgrade {parent}"
        ),
        FixPath::Parent {
            parent,
            parent_version,
            to,
            by_requirement: false,
        } => format!(
            "transitive via {parent} {parent_version}; the fix (>= {to}) is a semver-incompatible jump that \
             {parent}'s current line does not take, so a lockfile refresh will NOT fix it: upgrade {parent}"
        ),
        FixPath::ParentUnknown { to } => format!(
            "transitive; the fix (>= {to}) is a semver-incompatible jump, so a lockfile refresh will NOT \
             reach it: upgrade the dependency that pulls it in"
        ),
        FixPath::Reinstall { to } => format!(
            "the lockfile already pins {to} but node_modules still runs the old copy: reinstall"
        ),
        FixPath::NoFix => "no fix published: pin, patch locally, replace, or accept the risk".to_string(),
        FixPath::Update { to } => format!("update to >= {to}"),
    }
}

/// Short project label that names the repository: `atlas/bridge/src-tauri`,
/// not the ambiguous `bridge/src-tauri`. `repo` is the repository root's
/// directory name; `rel` the project's path inside it.
pub(crate) fn label_from(repo: &str, rel: &[&str]) -> String {
    let repo = repo.to_lowercase();
    match rel {
        [] => repo,
        [one] => format!("{repo}/{one}"),
        [.., a, b] => format!("{repo}/{a}/{b}"),
    }
}

/// Is `package` one the user publishes? Their own workspace members are
/// detected projects named after the package (victauri-core, @4da/cli).
pub(crate) fn is_own_package(package: &str, own_names: &HashSet<String>) -> bool {
    own_names.contains(&canonical_package(package))
}

fn canonical_package(name: &str) -> String {
    name.trim().to_lowercase().replace('_', "-")
}

/// Whole majors between two versions (0.x minors count as majors, the
/// semver compatibility unit).
pub(crate) fn majors_behind(installed: &str, announced: &str) -> u64 {
    match (
        lenient_semver(installed, None),
        lenient_semver(announced, None),
    ) {
        (Some(i), Some(a)) if a.major == 0 && i.major == 0 => a.minor.saturating_sub(i.minor),
        (Some(i), Some(a)) => a.major.saturating_sub(i.major),
        _ => 0,
    }
}

/// The lower-severity security facts as one line: the first
/// [`ALSO_OPEN_SHOWN`] by name, the rest as a count. Live 2026-10-02 there
/// were 25; listing them all turned one line into a wall.
pub(crate) fn also_open_line(also: &[SecurityFact]) -> String {
    let shown: Vec<String> = also
        .iter()
        .take(ALSO_OPEN_SHOWN)
        .map(|f| {
            let labels: Vec<&str> = f.sites.iter().map(|s| s.label.as_str()).collect();
            format!("{} ({})", f.package, labels.join(", "))
        })
        .collect();
    let rest = also.len().saturating_sub(ALSO_OPEN_SHOWN);
    if rest == 0 {
        shown.join(", ")
    } else {
        format!(
            "{}, and {rest} more on the Preemption tab",
            shown.join(", ")
        )
    }
}

/// How far behind an upgrade fact is, in words a reader trusts: "2 major
/// versions behind" at 1.x and above, "3 breaking releases behind (0.x)"
/// below 1.0, where every minor breaks.
fn gap_words(pre_one: bool, behind: u64) -> String {
    match (pre_one, behind) {
        (_, 0) => "breaking release".to_string(),
        (true, 1) => "one breaking release behind (0.x)".to_string(),
        (true, n) => format!("{n} breaking releases behind (0.x)"),
        (false, 1) => "one major version behind".to_string(),
        (false, n) => format!("{n} major versions behind"),
    }
}

/// Each project with its own gap: "4da/site on 20.3.1 (3 major versions
/// behind); navcal on 22.3.0 (one major version behind)". One gap for the
/// whole fact (computed from the lowest pin) made the live brief call navcal
/// three majors behind on stripe when it is one (2026-10-02).
pub(crate) fn upgrade_sites_line(u: &UpgradeFact) -> String {
    u.sites
        .iter()
        .map(|s| {
            format!(
                "{} on {} ({})",
                s.label,
                s.installed,
                gap_words(u.pre_one, majors_behind(&s.installed, &u.announced))
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// A body excerpt for the prompt: whitespace collapsed, cut on a char
/// boundary at a word end.
pub(crate) fn excerpt(body: &str, max_chars: usize) -> String {
    let collapsed: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let cut: String = collapsed.chars().take(max_chars).collect();
    let trimmed = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{trimmed} …")
}

// ============================================================================
// Novelty
// ============================================================================

/// One reported fact: when it was first reported in its current state, that
/// state, and when a brief last carried it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct FactRecord {
    pub since: String,
    pub sig: String,
    pub seen: String,
}

/// What the brief has reported, persisted in `kv_store`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct Novelty {
    #[serde(default)]
    pub facts: BTreeMap<String, FactRecord>,
    /// source item id -> local date it was featured
    #[serde(default)]
    pub featured: BTreeMap<i64, String>,
}

impl Novelty {
    /// NEW until a brief on an EARLIER day carried it in this state. A
    /// same-day regeneration (the Regenerate button, or a fingerprint change
    /// from one new advisory) must not move this morning's Critical out of
    /// Act now into Still open — live 2026-10-02 it did, in the second brief
    /// of the same run.
    pub(crate) fn status(&self, key: &str, signature: &str, today: &str) -> FactStatus {
        match self.facts.get(key) {
            Some(r) if r.sig == signature && r.since.as_str() < today => FactStatus::Unchanged {
                since: r.since.clone(),
            },
            _ => FactStatus::New,
        }
    }

    /// Record facts as reported today. An unchanged fact keeps its first date
    /// and refreshes its last-seen date; a changed one restarts both.
    pub(crate) fn record_facts<'a>(
        &mut self,
        facts: impl IntoIterator<Item = (&'a str, &'a str)>,
        today: &str,
    ) {
        for (key, sig) in facts {
            match self.facts.get_mut(key) {
                Some(r) if r.sig == sig => r.seen = today.to_string(),
                _ => {
                    self.facts.insert(
                        key.to_string(),
                        FactRecord {
                            since: today.to_string(),
                            sig: sig.to_string(),
                            seen: today.to_string(),
                        },
                    );
                }
            }
        }
    }

    pub(crate) fn record_featured(&mut self, ids: &[i64], today: &str) {
        for id in ids {
            self.featured
                .entry(*id)
                .or_insert_with(|| today.to_string());
        }
    }

    /// Featured on an EARLIER day (a same-day regeneration may repeat it).
    pub(crate) fn featured_before(&self, id: i64, today: &str) -> bool {
        self.featured.get(&id).is_some_and(|d| d.as_str() < today)
    }

    /// Forget what no brief has carried since `cutoff`. Pruning by FIRST
    /// date would resurrect a fact still open after the retention window as
    /// NEW; last-seen keeps it folded for as long as it stays open.
    pub(crate) fn prune(&mut self, cutoff: &str) {
        self.facts.retain(|_, r| r.seen.as_str() >= cutoff);
        self.featured.retain(|_, d| d.as_str() >= cutoff);
    }

    pub(crate) fn load(db: &Database) -> Self {
        db.get_kv(NOVELTY_KV_KEY)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub(crate) fn save(&self, db: &Database) {
        if let Ok(json) = serde_json::to_string(self) {
            if let Err(e) = db.set_kv(NOVELTY_KV_KEY, &json) {
                tracing::warn!(target: "4da::briefing", error = %e, "brief novelty not persisted");
            }
        }
    }
}

pub(crate) fn local_today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// After a brief is written: every fact it carried is now "reported", and the
/// candidates it actually featured are withheld from later days.
pub(crate) fn record_reported(db: &Database, facts: &BriefFacts, featured_ids: &[i64]) {
    let today = local_today();
    let mut novelty = Novelty::load(db);
    let sigs: Vec<(String, String)> = facts
        .security
        .iter()
        .chain(facts.also_open.iter())
        .map(|f| (f.key.clone(), security_signature(f)))
        .chain(
            facts
                .upgrades
                .iter()
                .map(|u| (u.key.clone(), u.announced.clone())),
        )
        .collect();
    novelty.record_facts(sigs.iter().map(|(k, s)| (k.as_str(), s.as_str())), &today);
    novelty.record_featured(featured_ids, &today);
    let cutoff = (chrono::Local::now() - chrono::Duration::days(NOVELTY_RETENTION_DAYS))
        .format("%Y-%m-%d")
        .to_string();
    novelty.prune(&cutoff);
    novelty.save(db);
}

/// The candidates a written brief actually featured: those whose title it
/// names (the prompt tells the model to refer to articles by title). The
/// trailer cannot answer this — it lists rejects, and a candidate left out
/// for space is neither rejected nor featured.
pub(crate) fn featured_in(content: &str, candidates: &[WorthKnowingCandidate]) -> Vec<i64> {
    let haystack = content.to_lowercase();
    candidates
        .iter()
        .filter(|c| {
            let title: String = c.title.trim().to_lowercase().chars().take(48).collect();
            title.chars().count() >= 8 && haystack.contains(title.as_str())
        })
        .map(|c| c.id)
        .collect()
}

/// What, if it changed, makes a security fact news again: the urgency, the
/// fix, and the installed versions.
pub(crate) fn security_signature(f: &SecurityFact) -> String {
    let mut parts: Vec<String> = vec![format!("{:?}", f.urgency)];
    for s in &f.sites {
        parts.push(format!(
            "{}={}->{}",
            s.label,
            s.installed.as_deref().unwrap_or("?"),
            fix_clause(&s.fix_path)
        ));
    }
    parts.join("|")
}

/// Every version the brief may state for each package it can name: the
/// input to `briefing_groundedness::check_factual_claims`, the deterministic
/// backstop that rejects a narration inventing an upgrade target (#662's
/// "bump hono to >= 4.7.5", below the installed 4.13.3). One fact per
/// package, versions merged: that checker resolves the FIRST fact for a name.
pub(crate) fn package_facts(facts: &BriefFacts) -> Vec<crate::briefing_groundedness::PackageFact> {
    let mut by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let add = |map: &mut BTreeMap<String, BTreeSet<String>>, name: &str, version: Option<&str>| {
        let entry = map.entry(name.to_string()).or_default();
        if let Some(v) = version.map(str::trim).filter(|v| !v.is_empty()) {
            entry.insert(v.trim_start_matches(['v', 'V']).to_string());
        }
    };
    let mut add = |name: &str, version: Option<&str>| add(&mut by_name, name, version);
    for f in facts.security.iter().chain(facts.also_open.iter()) {
        for s in &f.sites {
            add(&f.package, s.installed.as_deref());
            match &s.fix_path {
                FixPath::Bump { to }
                | FixPath::Refresh { to }
                | FixPath::ParentUnknown { to }
                | FixPath::Reinstall { to }
                | FixPath::Update { to } => add(&f.package, Some(to)),
                FixPath::Parent {
                    parent,
                    parent_version,
                    to,
                    ..
                } => {
                    add(&f.package, Some(to));
                    add(parent, Some(parent_version));
                }
                FixPath::NoFix => {}
            }
        }
    }
    for u in &facts.upgrades {
        add(&u.package, Some(&u.announced));
        for s in &u.sites {
            add(&u.package, Some(&s.installed));
        }
    }
    // The prompt lets the model quote a version a candidate article states
    // ("Vite 7.1 adds ..."). For each fact package an article names, every
    // version token in that article is legitimate too, or a correct brief
    // quoting its source would be faulted down to the floor.
    let names: Vec<String> = by_name.keys().cloned().collect();
    for c in &facts.worth_knowing {
        let text = format!("{} {}", c.title, c.excerpt);
        let lower = text.to_lowercase();
        let versions = version_tokens(&text);
        if versions.is_empty() {
            continue;
        }
        for name in names
            .iter()
            .filter(|n| names_word(&lower, &n.to_lowercase()))
        {
            by_name
                .entry(name.clone())
                .or_default()
                .extend(versions.iter().cloned());
        }
    }
    by_name
        .into_iter()
        .map(
            |(name, versions)| crate::briefing_groundedness::PackageFact {
                name,
                versions: versions.into_iter().collect(),
            },
        )
        .collect()
}

/// The retry instruction after a version fault. It says WHICH package each
/// misplaced version belongs to: live 2026-10-03 the brief wrote rmcp's fix
/// (2.1.0) in the sentence telling the user to upgrade victauri-plugin, the
/// checker read it as victauri-plugin's target, and a generic "use only the
/// versions in the FACTS" retry repeated the same sentence — 2.1.0 IS in the
/// facts, just not for that package.
pub(crate) fn correction_note(
    violations: &[String],
    facts: &[crate::briefing_groundedness::PackageFact],
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for v in violations {
        let pkg = v.split(" cited version ").next().unwrap_or("").trim();
        let ver = v
            .split(" cited version ")
            .nth(1)
            .and_then(|rest| rest.split(" as ").next())
            .unwrap_or("")
            .trim();
        let owners: Vec<&str> = facts
            .iter()
            .filter(|f| f.versions.iter().any(|x| x == ver))
            .map(|f| f.name.as_str())
            .collect();
        lines.push(if owners.is_empty() {
            format!("- {ver} is not a version of {pkg} (or of any package) in the FACTS; remove it")
        } else {
            format!(
                "- {ver} belongs to {}, not {pkg}: write it right after \"{}\" (e.g. \"{} >= {ver}\")",
                owners.join(" / "),
                owners[0],
                owners[0]
            )
        });
    }
    format!(
        "\n\nYour previous draft attached versions to the wrong package:\n{}\n\
         Write the brief again. Put every version immediately after the name of the package it \
         belongs to, and never after a different package's name.",
        lines.join("\n")
    )
}

/// Dotted version tokens in free text ("7.1", "v2.12.1").
fn version_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
        .map(|t| t.trim_matches('.').trim_start_matches(['v', 'V']))
        .filter(|t| {
            t.contains('.')
                && t.split('.')
                    .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        })
        .map(str::to_string)
        .collect()
}

/// Does `text` name `word` as a standalone token (not inside another name)?
fn names_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        let boundary =
            |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '-' || c == '_'));
        boundary(before) && boundary(after)
    })
}

/// Identity of every fact the brief reports — act-now, lower-severity and
/// upgrades. Computed over the FULL sets, before any display cut, so that
/// recording a brief (which reorders by novelty) cannot change it.
pub(crate) fn fingerprint(
    security: &[SecurityFact],
    also_open: &[SecurityFact],
    upgrades: &[UpgradeFact],
) -> String {
    use sha2::{Digest, Sha256};
    let mut lines: Vec<String> = security
        .iter()
        .chain(also_open.iter())
        .map(|f| format!("s:{}:{}", f.key, security_signature(f)))
        .chain(
            upgrades
                .iter()
                .map(|u| format!("u:{}:{}", u.key, u.announced)),
        )
        .collect();
    lines.sort();
    let mut hasher = Sha256::new();
    for line in &lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

// Builders (DB-backed) live in `brief_facts_build.rs` (file-size gate).
#[path = "brief_facts_build.rs"]
mod build;
pub(crate) use build::build_brief_facts;
#[cfg(test)]
use build::own_package_names;

#[cfg(test)]
#[path = "brief_facts_tests.rs"]
mod tests;
