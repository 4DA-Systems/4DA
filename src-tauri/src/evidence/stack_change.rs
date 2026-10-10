// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Signal Lane 1, the stack-change stream (AD-054 rule 2).
//!
//! Before this, Lane 1 was a client-side slice of the PASIFA-scored content
//! feed (`signal-lanes.ts`): whichever feed rows the grounding predicate
//! kept, in score order. AD-054 makes Signal the stream of what changed in
//! the user's stack, written from the SAME facts as the Brief:
//!
//! - **security:** the Brief's version-confirmed advisories
//!   (`brief_facts::build_brief_facts`, which reads the Preemption feed's own
//!   alerts), each with the fix path of AD-055 and, where the tool has one,
//!   the exact refresh command;
//! - **releases:** registry releases of packages a project declares, graded
//!   against each project's installed version (`scoring::release_grade`):
//!   yanked, major, breaking (a 0.x minor) or a new minor. Patches and
//!   prereleases are not news (`release_grade`'s rules); a minor that only
//!   dev tooling carries is tooling news, left out.
//!
//! No model touches the item set. Order is actionability: security, then
//! yanked, then breaking, then minors; inside a group, urgency, runtime before
//! dev tooling, newest first. Releases come from the Brief's 30-day window.
//!
//! Item contract (no new type, doctrine rule 1; ADR in
//! `docs/strategy/EVIDENCE-ITEM-SCHEMA.md`):
//! - `id` = `stack-change:<change>:<ecosystem>:<package>@<version>` for a
//!   release, `stack-change:security:<brief fact key>` for an advisory, where
//!   `<change>` is one of [`StackChange::word`]. The frontend reads the
//!   change from the id; nothing else is parsed.
//! - `evidence[0]` is a `version_context` citation ("crates.io · rmcp 1.7.0
//!   -> 2.1.0"); the rest cite the advisories or the registry release.
//! - `affected_deps` = `[package]`, `affected_projects` = the projects it asks
//!   something of.
//! - `suggested_actions` carry `run_command` actions whose label is the exact
//!   command and whose description names where to run it, when one exists.
//!
//! Tier (AD-054 rule 6): the advisories are the free floor; the release
//! stream is Signal's.

use std::time::{Duration, Instant};

use async_trait::async_trait;

use crate::brief_facts::{fix_clause, FixPath, SecurityFact, SecuritySite};
use crate::db::Database;
use crate::error::{FourDaError, Result};

use super::materializer::{EvidenceMaterializer, MaterializeContext};
use super::types::{
    Action, Confidence, EvidenceCitation, EvidenceFeed, EvidenceItem, EvidenceKind, LensHints,
    TierScope, Urgency,
};

#[path = "stack_change_releases.rs"]
mod releases;
use releases::ReleaseChange;

/// Items the command returns at most. The lane shows 20 before "Show all".
const MAX_ITEMS: usize = 200;
/// Commands named on one item at most (one per project tool).
const MAX_COMMANDS: usize = 3;
/// Advisory citations on one item at most.
const MAX_ADVISORY_CITATIONS: usize = 5;
/// How long a computed stream is served before it is recomputed.
const CACHE_TTL: Duration = Duration::from_mins(10);
/// Item titles are at most this many bytes (the schema's limit).
const TITLE_MAX: usize = 120;

/// What changed. The order is the lane's group order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum StackChange {
    /// A version-confirmed advisory against an installed copy.
    Security,
    /// A project runs a version the publisher yanked.
    Yanked,
    /// A new major at 1.0 or above.
    Major,
    /// A new minor below 1.0, where every minor is breaking.
    Breaking,
    /// A new minor at 1.0 or above.
    Minor,
}

impl StackChange {
    /// The id segment and the frontend's badge key.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Security => "security",
            Self::Yanked => "yanked",
            Self::Major => "major",
            Self::Breaking => "breaking",
            Self::Minor => "minor",
        }
    }

    /// Major and 0.x-minor releases are one group: both are breaking.
    fn group(self) -> u8 {
        match self {
            Self::Security => 0,
            Self::Yanked => 1,
            Self::Major | Self::Breaking => 2,
            Self::Minor => 3,
        }
    }
}

/// An item with the keys the lane is ordered by.
#[derive(Debug, Clone)]
pub(crate) struct Ranked {
    pub item: EvidenceItem,
    pub change: StackChange,
    pub dev_only: bool,
    /// `YYYY-MM-DD`; advisories carry their first-seen date.
    pub published: Option<String>,
    pub package: String,
}

/// Security, then yanked, then breaking, then minors; inside a group the
/// urgency, runtime before dev tooling, newest first, then the name.
pub(crate) fn order(items: &mut [Ranked]) {
    items.sort_by(|a, b| {
        a.change
            .group()
            .cmp(&b.change.group())
            .then(a.item.urgency.cmp(&b.item.urgency))
            .then(a.dev_only.cmp(&b.dev_only))
            .then(b.published.cmp(&a.published))
            .then(a.package.cmp(&b.package))
    });
}

// ============================================================================
// Item builders (pure; unit-tested in stack_change_tests.rs)
// ============================================================================

/// Cut a title on a char boundary under the schema's byte limit.
fn fit_title(title: String) -> String {
    let title = title.trim_end_matches('.').to_string();
    if title.len() <= TITLE_MAX {
        return title;
    }
    let mut cut = TITLE_MAX - '…'.len_utf8();
    while !title.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", title[..cut].trim_end())
}

fn sentence(text: &str) -> String {
    let t = text.trim().trim_end_matches('.');
    format!("{t}.")
}

/// Whole days between a `YYYY-MM-DD…` date and today, never negative.
fn days_since(date: Option<&str>) -> f32 {
    date.and_then(|d| chrono::NaiveDate::parse_from_str(d.get(..10)?, "%Y-%m-%d").ok())
        .map(|d| (chrono::Utc::now().date_naive() - d).num_days().max(0) as f32)
        .unwrap_or(0.0)
}

fn run_command(command: &str, label: &str) -> Action {
    Action {
        action_id: "run_command".to_string(),
        label: command.to_string(),
        description: format!("Run in {label}"),
    }
}

fn version_context(
    ecosystem: &str,
    package: &str,
    from: Option<&str>,
    to: Option<&str>,
    change: StackChange,
) -> EvidenceCitation {
    let span = match (from, to) {
        (Some(f), Some(t)) => format!("{package} {f} \u{2192} {t}"),
        (Some(f), None) => format!("{package} {f}"),
        (None, Some(t)) => format!("{package} \u{2192} {t}"),
        (None, None) => package.to_string(),
    };
    EvidenceCitation {
        source: "version_context".to_string(),
        title: format!("{ecosystem} \u{b7} {span}"),
        url: None,
        freshness_days: 0.0,
        relevance_note: change.word().to_string(),
    }
}

/// The version a fix path moves the package to, when it names one.
fn fix_target(path: &FixPath) -> Option<&str> {
    match path {
        FixPath::Bump { to }
        | FixPath::Refresh { to, .. }
        | FixPath::Parent { to, .. }
        | FixPath::ParentUnknown { to }
        | FixPath::Reinstall { to }
        | FixPath::Update { to } => Some(to),
        FixPath::NoFix => None,
    }
}

/// "1.7.0", or "1.7.0 (+1 other version)" when projects run different copies.
fn installed_span<'a>(versions: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut v: Vec<&str> = versions.collect();
    v.sort_unstable();
    v.dedup();
    match v.as_slice() {
        [] => None,
        [one] => Some((*one).to_string()),
        [first, rest @ ..] => Some(format!(
            "{first} (+{} other version{})",
            rest.len(),
            if rest.len() == 1 { "" } else { "s" }
        )),
    }
}

fn site_qualifiers(s: &SecuritySite) -> String {
    let mut q: Vec<String> = Vec::new();
    if s.dev_only {
        q.push("dev only".to_string());
    }
    if s.scratch {
        q.push("scratch tree".to_string());
    }
    if let Some(d) = s.dormant_days {
        q.push(format!("untouched for {d} days"));
    }
    if q.is_empty() {
        String::new()
    } else {
        format!(" ({})", q.join(", "))
    }
}

/// One advisory fact from the Brief as a stream item.
pub(crate) fn security_item(fact: &SecurityFact, now_ms: i64) -> Ranked {
    let pkg = &fact.package;
    let target = fact.sites.iter().find_map(|s| fix_target(&s.fix_path));
    let installed = installed_span(fact.sites.iter().filter_map(|s| s.installed.as_deref()));
    let urgency = crate::preemption::alert_urgency_to_canonical(&fact.urgency);
    let tier = fact
        .worst_tier
        .clone()
        .unwrap_or_else(|| format!("{urgency:?}").to_lowercase());
    let title = match (installed.as_deref(), target) {
        (Some(i), Some(t)) => format!("{pkg} {i} \u{2192} {t}: security fix ({tier})"),
        (Some(i), None) => format!("{pkg} {i}: {tier} advisory, no fix published"),
        (None, Some(t)) => format!("{pkg} \u{2192} {t}: security fix ({tier})"),
        (None, None) => format!("{pkg}: {tier} advisory, no fix published"),
    };

    let ids = &fact.advisory_ids;
    let shown: Vec<&str> = ids.iter().take(4).map(String::as_str).collect();
    let more = ids.len().saturating_sub(shown.len());
    let mut explanation = vec![
        sentence(&fact.title),
        format!(
            "Version-confirmed, because the installed copy is inside the affected range of {} {}{}.",
            if fact.advisory_count == 1 { "advisory" } else { "advisories" },
            shown.join(", "),
            if more > 0 { format!(" and {more} more") } else { String::new() }
        ),
    ];
    for s in &fact.sites {
        explanation.push(sentence(&format!(
            "{}{}: {}",
            s.label,
            site_qualifiers(s),
            fix_clause(pkg, &s.fix_path)
        )));
    }
    if !fact.not_compiled.is_empty() {
        let nc: Vec<&str> = fact
            .not_compiled
            .iter()
            .map(|n| n.advisory_id.as_str())
            .collect();
        explanation.push(format!(
            "Not counted, because these builds do not compile the code they name: {}.",
            nc.join(", ")
        ));
    }

    let mut evidence = vec![version_context(
        &fact.ecosystem,
        pkg,
        installed.as_deref(),
        target,
        StackChange::Security,
    )];
    let freshness = days_since(fact.first_seen.as_deref());
    evidence.extend(
        ids.iter()
            .take(MAX_ADVISORY_CITATIONS)
            .map(|id| EvidenceCitation {
                source: "osv".to_string(),
                title: id.clone(),
                url: Some(format!("https://osv.dev/vulnerability/{id}")),
                freshness_days: freshness,
                relevance_note: String::new(),
            }),
    );

    let mut actions = vec![Action {
        action_id: "review_security".to_string(),
        label: "Read the advisory".to_string(),
        description: ids.first().map_or_else(
            || "Open the advisory".to_string(),
            |id| format!("Open {id} on osv.dev"),
        ),
    }];
    let mut commands: Vec<(&str, &str)> = Vec::new();
    for s in &fact.sites {
        if let FixPath::Refresh {
            command: Some(c), ..
        } = &s.fix_path
        {
            if !commands.iter().any(|(seen, _)| seen == c) && commands.len() < MAX_COMMANDS {
                commands.push((c, &s.label));
            }
        }
    }
    actions.extend(commands.iter().map(|(c, label)| run_command(c, label)));

    let mut projects: Vec<String> = fact.sites.iter().map(|s| s.label.clone()).collect();
    projects.dedup();
    Ranked {
        item: EvidenceItem {
            id: format!("stack-change:security:{}", fact.key),
            kind: EvidenceKind::Alert,
            title: fit_title(title),
            explanation: explanation.join(" "),
            confidence: Confidence::osv_verified(0.95),
            urgency,
            reversibility: None,
            evidence,
            evidence_total: None,
            affected_projects: projects,
            affected_deps: vec![pkg.clone()],
            suggested_actions: actions,
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::default(),
            created_at: now_ms,
            expires_at: None,
        },
        change: StackChange::Security,
        dev_only: fact.sites.iter().all(|s| s.dev_only),
        published: fact
            .first_seen
            .as_ref()
            .map(|d| d.chars().take(10).collect()),
        package: pkg.clone(),
    }
}

fn manifest_name(ecosystem: &str) -> &'static str {
    match ecosystem {
        "crates.io" => "Cargo.toml",
        "npm" => "package.json",
        "PyPI" => "the project's requirements",
        "Go" => "go.mod",
        _ => "the manifest",
    }
}

/// What to do about a release, in one sentence.
fn release_action(r: &ReleaseChange) -> String {
    let pkg = &r.package;
    let new = &r.announced;
    let manifest = manifest_name(&r.ecosystem);
    match r.change {
        StackChange::Yanked => format!(
            "The publisher withdrew the version you pin: move {pkg} to {new} (or the newest release on your line that is not yanked)."
        ),
        StackChange::Major | StackChange::Breaking => format!(
            "A breaking release: read the release notes, then raise the {pkg} requirement in {manifest} to {new} and fix what changed."
        ),
        StackChange::Minor | StackChange::Security => {
            let inside: Vec<&str> = r
                .sites
                .iter()
                .filter(|s| s.admitted == Some(true))
                .map(|s| s.label.as_str())
                .collect();
            let outside: Vec<&str> = r
                .sites
                .iter()
                .filter(|s| s.admitted == Some(false))
                .map(|s| s.label.as_str())
                .collect();
            let mut parts: Vec<String> = Vec::new();
            if !inside.is_empty() {
                parts.push(format!(
                    "{pkg} {new} is inside the requirement {} declares, so refreshing the lockfile picks it up",
                    inside.join(", ")
                ));
            }
            if !outside.is_empty() {
                parts.push(format!(
                    "{} declares a requirement that excludes {new}: raise it in {manifest}",
                    outside.join(", ")
                ));
            }
            if parts.is_empty() {
                format!("Bump {pkg} to {new} where it is declared.")
            } else {
                sentence(&parts.join("; "))
            }
        }
    }
}

fn change_label(r: &ReleaseChange) -> &'static str {
    match r.change {
        StackChange::Yanked => "your pinned version was yanked",
        StackChange::Major => "major release",
        StackChange::Breaking => "breaking release (0.x)",
        StackChange::Minor | StackChange::Security => "new minor",
    }
}

/// One graded registry release as a stream item.
pub(crate) fn release_item(r: &ReleaseChange, now_ms: i64) -> Ranked {
    let pkg = &r.package;
    let installed = installed_span(r.sites.iter().map(|s| s.installed.as_str()));
    let title = match installed.as_deref() {
        Some(i) => format!("{pkg} {i} \u{2192} {}: {}", r.announced, change_label(r)),
        None => format!("{pkg} {}: {}", r.announced, change_label(r)),
    };
    let who: Vec<String> = r
        .sites
        .iter()
        .map(|s| {
            format!(
                "{} on {}{}",
                s.label,
                s.installed,
                if s.is_dev { " (dev)" } else { "" }
            )
        })
        .collect();
    let mut explanation = vec![format!(
        "{pkg} {} was published{}.",
        r.announced,
        r.published
            .as_deref()
            .map(|d| format!(" on {d}"))
            .unwrap_or_default()
    )];
    explanation.push(format!(
        "It concerns {}, because they declare {pkg} directly.",
        who.join("; ")
    ));
    if let Some(current) = &r.current {
        explanation.push(format!("Already current: {current}."));
    }
    explanation.push(release_action(r));

    let mut evidence = vec![version_context(
        &r.ecosystem,
        pkg,
        installed.as_deref(),
        Some(&r.announced),
        r.change,
    )];
    evidence.push(EvidenceCitation {
        source: r.source_type.clone(),
        title: r.row_title.clone(),
        url: r.url.clone(),
        freshness_days: days_since(r.published.as_deref()),
        relevance_note: String::new(),
    });

    let (action_id, label) = match r.change {
        StackChange::Minor | StackChange::Security => ("review_updates", "Review the release"),
        StackChange::Yanked | StackChange::Major | StackChange::Breaking => {
            ("check_breaking", "Read the release notes")
        }
    };
    let mut actions = vec![Action {
        action_id: action_id.to_string(),
        label: label.to_string(),
        description: format!("Open {pkg} {} on its registry", r.announced),
    }];
    let mut seen: Vec<&str> = Vec::new();
    for s in &r.sites {
        if let Some(c) = s.command.as_deref() {
            if !seen.contains(&c) && seen.len() < MAX_COMMANDS {
                seen.push(c);
                actions.push(run_command(c, &s.label));
            }
        }
    }

    let urgency = match r.change {
        StackChange::Yanked => Urgency::High,
        StackChange::Major | StackChange::Breaking if !r.dev_only => Urgency::Medium,
        _ => Urgency::Watch,
    };
    Ranked {
        item: EvidenceItem {
            id: format!(
                "stack-change:{}:{}:{pkg}@{}",
                r.change.word(),
                r.ecosystem.to_lowercase(),
                r.announced
            ),
            kind: EvidenceKind::Alert,
            title: fit_title(title),
            explanation: explanation.join(" "),
            confidence: Confidence::checklist(0.9),
            urgency,
            reversibility: None,
            evidence,
            evidence_total: None,
            affected_projects: r.sites.iter().map(|s| s.project_path.clone()).collect(),
            affected_deps: vec![pkg.clone()],
            suggested_actions: actions,
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::default(),
            created_at: now_ms,
            expires_at: None,
        },
        change: r.change,
        dev_only: r.dev_only,
        published: r.published.clone(),
        package: pkg.clone(),
    }
}

/// Order, validate and cap. An invalid item is a bug: debug builds stop on
/// it, release builds drop it with a structured log (doctrine rule 9).
pub(crate) fn finish(mut ranked: Vec<Ranked>, max_items: usize) -> Vec<EvidenceItem> {
    order(&mut ranked);
    let mut out = Vec::with_capacity(ranked.len().min(max_items));
    for r in ranked {
        match super::validate::validate_item(&r.item) {
            Ok(()) => out.push(r.item),
            Err(e) => {
                debug_assert!(false, "stack_change emitted an invalid item: {e:?}");
                tracing::warn!(target: "4da::evidence::validate", id = %r.item.id, error = ?e, "dropped invalid stack-change item");
            }
        }
        if out.len() >= max_items {
            break;
        }
    }
    out
}

// ============================================================================
// Materializer + command
// ============================================================================

/// Every stack-change item, computed from the database. Blocking.
pub(crate) fn collect(
    db: &Database,
    include_releases: bool,
    max_items: usize,
) -> Vec<EvidenceItem> {
    let now = chrono::Utc::now().timestamp_millis();
    let facts = crate::brief_facts::build_brief_facts(db);
    let mut ranked: Vec<Ranked> = facts
        .security
        .iter()
        .chain(facts.also_open.iter())
        .map(|f| security_item(f, now))
        .collect();
    if include_releases {
        match crate::open_db_connection() {
            Ok(conn) => ranked.extend(
                releases::collect_release_changes(db, &conn)
                    .iter()
                    .map(|r| release_item(r, now)),
            ),
            Err(e) => {
                tracing::warn!(target: "4da::stack_change", error = %e, "release pass skipped")
            }
        }
    }
    finish(ranked, max_items)
}

/// The stack-change materializer. Advisories always; registry releases only
/// for Signal (AD-054 rule 6).
pub struct StackChangeMaterializer {
    db: &'static Database,
    include_releases: bool,
}

#[async_trait]
impl EvidenceMaterializer for StackChangeMaterializer {
    fn name(&self) -> &'static str {
        "stack_change"
    }

    async fn materialize(&self, ctx: &MaterializeContext) -> Result<Vec<EvidenceItem>> {
        let (db, include, max) = (self.db, self.include_releases, ctx.max_items);
        tokio::task::spawn_blocking(move || collect(db, include, max))
            .await
            .map_err(|e| FourDaError::Internal(format!("stack-change task failed: {e}")))
    }
}

static CACHE: parking_lot::Mutex<Option<(Instant, bool, EvidenceFeed)>> =
    parking_lot::Mutex::new(None);

fn cached(entitled: bool) -> Option<EvidenceFeed> {
    let guard = CACHE.lock();
    guard
        .as_ref()
        .filter(|(at, scope, _)| *scope == entitled && at.elapsed() < CACHE_TTL)
        .map(|(.., feed)| feed.clone())
}

async fn compute_feed(entitled: bool) -> Result<EvidenceFeed> {
    let db: &'static Database = crate::get_database()?;
    let tracked = tokio::task::spawn_blocking(|| {
        crate::open_db_connection()
            .map(|conn| crate::knowledge_decay::known_dependency_count(&conn))
            .ok()
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("stack-change task failed: {e}")))?;
    // No lockfile read yet: there is nothing to compare, so the lane shows
    // onboarding (doctrine rule 6), not "nothing changed".
    let items = if tracked == Some(0) {
        Vec::new()
    } else {
        let mut ctx = MaterializeContext::new();
        ctx.max_items = MAX_ITEMS;
        StackChangeMaterializer {
            db,
            include_releases: entitled,
        }
        .materialize(&ctx)
        .await?
    };
    let mut feed = EvidenceFeed::from_items(items);
    feed.total_tracked = tracked.map(|n| usize::try_from(n).unwrap_or(0));
    feed.tier_scope = Some(if entitled {
        TierScope::Full
    } else {
        TierScope::FreeFloor
    });
    feed.computed_at = Some(chrono::Utc::now().to_rfc3339());
    Ok(feed)
}

/// Signal Lane 1: what changed in the user's stack, as an `EvidenceFeed`.
/// `total_tracked` is the number of dependencies known (0 = nothing scanned
/// yet); `tier_scope` says whether releases are included. Served from a
/// ten-minute cache unless `force`.
#[tauri::command]
pub async fn get_stack_changes(force: Option<bool>) -> std::result::Result<EvidenceFeed, String> {
    let entitled = crate::settings::is_signal();
    if !force.unwrap_or(false) {
        if let Some(feed) = cached(entitled) {
            return Ok(feed);
        }
    }
    let feed = compute_feed(entitled).await.map_err(|e| e.to_string())?;
    *CACHE.lock() = Some((Instant::now(), entitled, feed.clone()));
    Ok(feed)
}

#[cfg(test)]
#[path = "stack_change_tests.rs"]
mod tests;
