// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Upgrade Plan — the ranked "which dependency upgrade actually matters" brain
//! (Phase 1 of the dependency-intelligence launch;
//! `.claude/plans/upgrade-plan-launch-blueprint.md`, §D-4).
//!
//! Turns the version-confirmed OSV matches (`osv::matching`) into a ranked plan
//! of **per-package** upgrade steps — the actionable unit ("upgrade `lodash` →
//! clears 3 advisories across 4 projects"), not a flat advisory list.
//!
//! Doctrine:
//! - Emits the canonical [`EvidenceItem`] via the direct builder (no bespoke
//!   plan struct — intelligence-doctrine rule 1), schema-validated at the
//!   boundary; an invalid item is dropped (debug builds assert).
//! - **Accuracy-first:** only version-CONFIRMED groups become plan steps. A
//!   conservative (name-only) match is a "maybe", never an "upgrade this" — it
//!   still shows in the Preemption alert feed, just not the plan.
//! - **No fake actions** (rule 5): actions are informational only
//!   (`review_security` / `view_source` / `snooze_7d` / `dismiss`) — 4DA never
//!   claims to perform the upgrade. The removed "Update pkg" button precedent.
//! - `Heuristic` confidence provenance — the *ranking* is a heuristic. This also
//!   correctly excludes plan steps from the free security floor (which admits
//!   only `OsvVerified` provenance), so the plan is Signal-tier by construction.
//! - Cold-start silent: an empty match set yields an empty plan (nothing
//!   renders).
//!
//! Consumers (all shipped): the Preemption "Upgrade Plan" group renders these
//! items (Signal-gated by their `Heuristic` provenance); `persist_upgrade_plan`
//! writes the snapshot to `kv_store` for out-of-process readers; the `4da plan`
//! CLI and the headless engine's per-cycle refresh read/refresh it. This module
//! is the ranking brain behind all of them.

use crate::db::Database;
use crate::osv::types::MatchedAdvisory;

use super::types::{
    Action, Confidence, EvidenceCitation, EvidenceItem, EvidenceKind, LensHints, Urgency,
};

/// Max advisory citations attached to one step (keeps items bounded; the count
/// in the title/explanation still reflects the true total).
const MAX_CITATIONS: usize = 8;

/// `kv_store` key under which the persisted plan snapshot lives (single,
/// latest-wins). The MCP server reads this same key from the shared SQLite file.
const PLAN_KV_KEY: &str = "upgrade_plan_snapshot";

/// Persisted-shape version. Bump on an incompatible change to
/// [`super::types::UpgradePlanSnapshot`] / the item JSON; a reader that sees a
/// higher version treats the snapshot as absent (fail closed).
const PLAN_SCHEMA_VERSION: u32 = 3;

/// Persist the ranked plan to `kv_store` (blueprint D-1, DB-as-interface) so it
/// survives restart and is readable out-of-process (the `4da plan` CLI reads
/// this key; the MCP handoff reads it too). Called from BOTH the GUI feed compute
/// and the headless engine cycle — persists EVERY computed plan, including an
/// empty one, so a reader can tell "evaluated, nothing to do" (fresh
/// `generated_at`, 0 items) from "never computed" (no key). Best-effort: a write
/// error is logged, never propagated into the caller.
pub fn persist_upgrade_plan(
    db: &Database,
    items: &[EvidenceItem],
    validation_drop_count: u32,
    engine_run_id: Option<i64>,
) {
    let generated = chrono::Utc::now();
    // Freshness FLOOR of the security data: the oldest ecosystem sync timestamp
    // (lexicographic min of the fixed-width `YYYY-MM-DD HH:MM:SS` = chronological
    // oldest). A reader pairs this with `expires_at` to judge staleness honestly.
    let source_freshness = db
        .get_osv_sync_statuses()
        .ok()
        .and_then(|statuses| statuses.into_iter().filter_map(|s| s.last_synced_at).min());
    // Staleness horizon: the plan is only as fresh as the security data it read,
    // and that data is refreshed on the OSV sync cadence. State that horizon so a
    // reader judges staleness without knowing 4DA's policy.
    let expires = generated + chrono::Duration::hours(crate::osv::sync::osv_sync_max_age_hours());
    // The inventory the plan was computed from (already sorted by the query) —
    // used for both the change-detection hash and the coverage gate.
    let instances = db.get_all_dependency_instances().unwrap_or_default();
    let dependency_inventory_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        // Volatile columns (row id, detected_at) are excluded on purpose — the
        // hash tracks the identity set (project, ecosystem, package, version).
        for r in &instances {
            hasher.update(r.project_path.as_bytes());
            hasher.update([0u8]);
            hasher.update(r.ecosystem.as_bytes());
            hasher.update([0u8]);
            hasher.update(r.package_name.as_bytes());
            hasher.update([0u8]);
            hasher.update(r.version.as_bytes());
            hasher.update(*b"\n");
        }
        hex::encode(hasher.finalize())
    };
    let snapshot = super::types::UpgradePlanSnapshot {
        schema_version: PLAN_SCHEMA_VERSION,
        generated_at: generated.to_rfc3339(),
        expires_at: expires.to_rfc3339(),
        generator_version: env!("CARGO_PKG_VERSION").to_string(),
        entitlement_scope_at_generation: if crate::settings::is_signal() {
            "signal".to_string()
        } else {
            "free".to_string()
        },
        // Green iff the multi-version inventory (Phase 92) is populated — a reader
        // must not trust negative/close verdicts without it.
        multi_version_coverage: !instances.is_empty(),
        dependency_inventory_hash,
        validation_drop_count,
        source_freshness,
        engine_run_id,
        item_count: items.len(),
        items: items.to_vec(),
    };
    match serde_json::to_string(&snapshot) {
        Ok(json) => {
            if let Err(e) = db.set_kv(PLAN_KV_KEY, &json) {
                tracing::warn!(target: "4da::upgrade_plan", error = %e, "failed to persist upgrade plan snapshot");
            }
        }
        Err(e) => {
            tracing::warn!(target: "4da::upgrade_plan", error = %e, "failed to serialize upgrade plan snapshot");
        }
    }
}

/// Read the persisted plan snapshot. Returns `None` when absent, unparseable, or
/// written by an incompatible schema version (fail closed — a reader must never
/// act on a snapshot it cannot fully trust).
///
/// Test-exercised only: the shipped `4da plan` CLI reads the `kv_store` key via
/// raw SQL (it deliberately does not link `fourda_lib`), so it does NOT call this
/// `Database`-based reader. This fn's production caller is the in-app Phase-2a
/// reader (operator-gated), still pending.
#[allow(dead_code)] // REMOVE BY 2026-10-01 — wired by the in-app Phase-2a reader
pub fn read_upgrade_plan_snapshot(db: &Database) -> Option<super::types::UpgradePlanSnapshot> {
    let json = db.get_kv(PLAN_KV_KEY).ok().flatten()?;
    let snapshot: super::types::UpgradePlanSnapshot = serde_json::from_str(&json).ok()?;
    if snapshot.schema_version != PLAN_SCHEMA_VERSION {
        tracing::debug!(
            target: "4da::upgrade_plan",
            found = snapshot.schema_version,
            expected = PLAN_SCHEMA_VERSION,
            "upgrade plan snapshot schema mismatch — treated as absent"
        );
        return None;
    }
    Some(snapshot)
}

/// Build the ranked dependency Upgrade Plan as validated [`EvidenceItem`]s,
/// most-actionable first, and the count of steps dropped because they failed
/// `validate_item`. Returns `(empty, 0)` on cold-start (no matches) or if the
/// matcher errors — never breaks on a data quirk.
///
/// Consumed by `preemption::compute_preemption_evidence_feed` (the Signal-tier
/// feed) which renders the items as the "Upgrade Plan" group and persists the
/// snapshot. The drop count is the snapshot's `validation_drop_count` — the
/// "thin plan" canary (a silent drop in a release build once shipped a truncated
/// citation as a dropped step; a reader that sees a non-zero count knows the
/// plan may under-report).
pub fn build_upgrade_plan_with_drops(db: &Database) -> (Vec<EvidenceItem>, u32) {
    let matches = match crate::osv::matching::get_matched_advisories(db) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(target: "4da::upgrade_plan", error = %e, "matcher failed — empty plan");
            return (Vec::new(), 0);
        }
    };

    let mut groups = aggregate_by_package(&matches);
    // Accuracy-first: the plan is the version-confirmed set only.
    groups.retain(|g| g.any_confirmed);

    // Platform relevance (Phase 85): drop packages that are inactive on every
    // build target the host builds (e.g. a `cfg(not(windows))`-only crate's CVE
    // on Windows). The plan answers "which upgrade matters to YOUR build", so a
    // genuinely-irrelevant advisory is not a plan step — but it is NOT hidden:
    // it still surfaces in Preemption's collapsed "other build targets" group
    // (label-and-de-prioritise, never suppress). Unknown platform = kept.
    let platform_inactive = db.platform_inactive_packages();
    if !platform_inactive.is_empty() {
        groups.retain(|g| !platform_inactive.contains(&g.package.to_lowercase()));
    }

    // Rank: most-urgent → confirmed → fixable-now → widest blast radius → CVSS.
    groups.sort_by_key(PackageGroup::sort_key);

    let now = chrono::Utc::now().timestamp_millis();
    let mut items = Vec::with_capacity(groups.len());
    let mut drops = 0u32;
    for g in groups {
        let item = g.into_evidence_item(now);
        match super::validate::validate_item(&item) {
            Ok(()) => items.push(item),
            Err(e) => {
                debug_assert!(false, "upgrade_plan emitted an invalid item: {e:?}");
                tracing::warn!(
                    target: "4da::evidence::validate",
                    id = %item.id,
                    error = ?e,
                    "dropped invalid upgrade-plan item"
                );
                drops += 1;
            }
        }
    }
    (items, drops)
}

/// One package's aggregated upgrade step.
struct PackageGroup<'a> {
    package: String,
    ecosystem_norm: String,
    advisories: Vec<&'a MatchedAdvisory>,
    /// Union of every affected project across the group's advisories (sorted).
    projects: Vec<String>,
    any_confirmed: bool,
    /// At least one affected instance is a DIRECT dep — the user can bump it now.
    fixable_now: bool,
    /// Every affected instance is a dev dependency (a labelled discount).
    all_dev: bool,
    urgency: Urgency,
    max_cvss: f64,
    /// Highest fixed version across the group's advisories (upgrading to it
    /// clears the most); `None` when no advisory has a fix yet.
    target_version: Option<String>,
}

fn aggregate_by_package(matches: &[MatchedAdvisory]) -> Vec<PackageGroup<'_>> {
    use std::collections::BTreeMap;
    // Key by normalized (ecosystem, package) so npm/crates.io/etc. don't collide
    // and the same package across projects folds into one step.
    let mut by_key: BTreeMap<(String, String), Vec<&MatchedAdvisory>> = BTreeMap::new();
    for adv in matches {
        let key = (
            normalize_ecosystem(&adv.ecosystem),
            adv.package_name.to_lowercase(),
        );
        by_key.entry(key).or_default().push(adv);
    }

    by_key
        .into_iter()
        .map(|((ecosystem_norm, _pkg_lower), advisories)| {
            // Display name from the first advisory (preserves original casing).
            let package = advisories[0].package_name.clone();

            let mut projects: Vec<String> = advisories
                .iter()
                .flat_map(|a| a.project_paths.iter().cloned())
                .collect();
            projects.sort();
            projects.dedup();

            let any_confirmed = advisories.iter().any(|a| a.is_version_confirmed);
            let fixable_now = advisories
                .iter()
                .flat_map(|a| a.dependency_instances.iter())
                .any(|d| d.is_direct);
            let instances_exist = advisories
                .iter()
                .any(|a| !a.dependency_instances.is_empty());
            let all_dev = instances_exist
                && advisories
                    .iter()
                    .flat_map(|a| a.dependency_instances.iter())
                    .all(|d| d.is_dev);

            let max_cvss = advisories
                .iter()
                .filter_map(|a| a.cvss_score)
                .fold(0.0_f64, f64::max);

            // Most-urgent advisory, then a one-level discount if the package is
            // dev-only (labelled, never suppressed).
            let base_urgency = advisories
                .iter()
                .map(|a| advisory_urgency(a))
                .min()
                .unwrap_or(Urgency::Medium);
            let urgency = if all_dev {
                downrank(base_urgency)
            } else {
                base_urgency
            };

            let target_version = highest_fixed_version(&advisories);

            PackageGroup {
                package,
                ecosystem_norm,
                advisories,
                projects,
                any_confirmed,
                fixable_now,
                all_dev,
                urgency,
                max_cvss,
                target_version,
            }
        })
        .collect()
}

impl PackageGroup<'_> {
    /// Ascending sort key = rank order (smaller sorts first / higher priority).
    /// `Urgency` is `Ord` with `Critical` smallest, so it sorts first already.
    /// Booleans: we want fixable-now and confirmed FIRST, so invert via `!`.
    fn sort_key(
        &self,
    ) -> (
        Urgency,
        bool,
        bool,
        std::cmp::Reverse<usize>,
        std::cmp::Reverse<i64>,
        String,
    ) {
        (
            self.urgency,
            !self.any_confirmed, // confirmed (true) sorts before unconfirmed
            !self.fixable_now,   // fixable-now sorts before waiting-on-upstream
            std::cmp::Reverse(self.projects.len()), // widest blast radius first
            std::cmp::Reverse((self.max_cvss * 1000.0) as i64), // higher CVSS first
            self.package.to_lowercase(), // stable final tiebreak
        )
    }

    fn advisory_count(&self) -> usize {
        self.advisories.len()
    }

    fn into_evidence_item(self, now_millis: i64) -> EvidenceItem {
        let n = self.advisory_count();
        let m = self.projects.len();
        let target_note = self
            .target_version
            .as_deref()
            .map(|v| format!(" to >= {v}"))
            .unwrap_or_default();

        let title = clamp_title(format!(
            "Upgrade {pkg}{target} — clears {n} {adv} across {m} {proj}",
            pkg = self.package,
            target = target_note,
            n = n,
            adv = plural(n, "advisory", "advisories"),
            m = m,
            proj = plural(m, "project", "projects"),
        ));

        let scope_note = if self.fixable_now {
            "Fixable now via a direct dependency bump"
        } else {
            "Fixed only upstream — awaits a parent-package update or lockfile refresh"
        };
        let dev_note = if self.all_dev {
            " All affected instances are dev-only."
        } else {
            ""
        };
        let ids: Vec<&str> = self
            .advisories
            .iter()
            .take(MAX_CITATIONS)
            .map(|a| a.advisory_id.as_str())
            .collect();
        let more = n.saturating_sub(ids.len());
        let more_note = if more > 0 {
            format!(" (+{more} more)")
        } else {
            String::new()
        };
        let explanation = format!(
            "{pkg} has {n} version-confirmed {adv} matching installed versions across {m} {proj}. \
             {scope}.{dev} Advisories: {ids}{more}.",
            pkg = self.package,
            n = n,
            adv = plural(n, "advisory", "advisories"),
            m = m,
            proj = plural(m, "project", "projects"),
            scope = scope_note,
            dev = dev_note,
            ids = ids.join(", "),
            more = more_note,
        );

        // Citations: one per advisory (capped), plus the affected-projects context.
        let mut evidence: Vec<EvidenceCitation> = self
            .advisories
            .iter()
            .take(MAX_CITATIONS)
            .map(|a| EvidenceCitation {
                source: "osv-advisory".to_string(),
                title: truncate(&a.summary, 160),
                url: a.source_url.clone(),
                freshness_days: 0.0,
                relevance_note: truncate(
                    &format!(
                        "{sev}affects installed {ver}",
                        sev = a
                            .cvss_score
                            .map(|c| format!("CVSS {c:.1}; "))
                            .unwrap_or_default(),
                        ver = a.installed_version.as_deref().unwrap_or("version"),
                    ),
                    200,
                ),
            })
            .collect();
        evidence.push(EvidenceCitation {
            source: "project-scan".to_string(),
            title: format!("Installed in {m} {}", plural(m, "project", "projects")),
            url: None,
            freshness_days: 0.0,
            relevance_note: truncate(
                &format!("Affected projects: {}", self.projects.join(", ")),
                200,
            ),
        });

        // Confidence: heuristic ranking; a shade higher when the fix is a direct
        // bump the user controls. All groups here are version-confirmed.
        let confidence_value = if self.fixable_now { 0.9 } else { 0.8 };

        let suggested_actions = vec![
            Action {
                action_id: "review_security".to_string(),
                label: "Review advisories".to_string(),
                description: format!(
                    "Review the {n} {adv} affecting {pkg}",
                    n = n,
                    adv = plural(n, "advisory", "advisories"),
                    pkg = self.package
                ),
            },
            Action {
                action_id: "view_source".to_string(),
                label: "Open advisory".to_string(),
                description: "Open the advisory source".to_string(),
            },
            Action {
                action_id: "snooze_7d".to_string(),
                label: "Snooze 7 days".to_string(),
                description: "Hide this upgrade step for a week".to_string(),
            },
            Action {
                action_id: "dismiss".to_string(),
                label: "Dismiss".to_string(),
                description: "Dismiss this upgrade step".to_string(),
            },
        ];

        EvidenceItem {
            id: format!(
                "upgrade-plan:{}:{}",
                self.ecosystem_norm,
                self.package.to_lowercase()
            ),
            kind: EvidenceKind::Alert,
            title,
            explanation,
            confidence: Confidence::heuristic(confidence_value),
            urgency: self.urgency,
            reversibility: None,
            evidence,
            affected_projects: self.projects,
            affected_deps: vec![self.package],
            suggested_actions,
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::upgrade_plan(),
            created_at: now_millis,
            expires_at: None,
        }
    }
}

// ---- helpers ----

fn normalize_ecosystem(eco: &str) -> String {
    crate::ecosystem::Ecosystem::parse(eco)
        .map_or_else(|| eco.to_string(), |e| e.osv_name().to_string())
}

/// CVSS-driven urgency. OSV `severity_type` is the scoring *system*
/// ("CVSS_V3"), NOT the level — so the level comes from `cvss_score`. A
/// confirmed match with no score defaults to `Medium` (a real advisory, but no
/// evidence to call it Critical or dismiss it to Watch).
fn advisory_urgency(a: &MatchedAdvisory) -> Urgency {
    match a.cvss_score {
        Some(c) if c >= 9.0 => Urgency::Critical,
        Some(c) if c >= 7.0 => Urgency::High,
        Some(c) if c >= 4.0 => Urgency::Medium,
        Some(c) if c > 0.0 => Urgency::Watch,
        _ => Urgency::Medium,
    }
}

/// One-level urgency discount (labelled dev-only scope). Never below `Watch`.
fn downrank(u: Urgency) -> Urgency {
    match u {
        Urgency::Critical => Urgency::High,
        Urgency::High => Urgency::Medium,
        Urgency::Medium => Urgency::Watch,
        Urgency::Watch => Urgency::Watch,
    }
}

/// Highest fixed version across the advisories (semver-aware; falls back to a
/// lexicographic max for non-semver strings, then to the first fix seen).
fn highest_fixed_version(advisories: &[&MatchedAdvisory]) -> Option<String> {
    let fixes: Vec<String> = advisories
        .iter()
        .filter_map(|a| a.fixed_version.clone())
        .filter(|v| !v.is_empty())
        .collect();
    if fixes.is_empty() {
        return None;
    }
    let mut best = fixes[0].clone();
    for v in fixes.iter().skip(1) {
        if version_gt(v, &best) {
            best = v.clone();
        }
    }
    Some(best)
}

fn version_gt(a: &str, b: &str) -> bool {
    match (
        semver::Version::parse(a.trim_start_matches('v')),
        semver::Version::parse(b.trim_start_matches('v')),
    ) {
        (Ok(va), Ok(vb)) => va > vb,
        _ => a > b, // both non-semver → lexicographic
    }
}

fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// Trim trailing period and clamp to 120 BYTES on a char boundary (the schema
/// limit is byte length).
fn clamp_title(s: String) -> String {
    let s = s.trim_end().trim_end_matches('.').to_string();
    if s.len() <= 120 {
        return s;
    }
    let mut end = 120;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].trim_end().trim_end_matches('.').to_string()
}

/// Truncate to at most `max` bytes on a char boundary (adds an ellipsis if cut).
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // The schema limit is BYTE length and the ellipsis is 3 bytes in UTF-8 —
    // budgeting 1 byte for it emitted up to max+2 bytes, which hard-panicked
    // the validator in dev (CitationNoteTooLong { len: 202 }, live 2026-07-17).
    let budget = max.saturating_sub('\u{2026}'.len_utf8());
    let mut end = budget.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\u{2026}", &s[..end])
}

#[cfg(test)]
#[path = "upgrade_plan_tests.rs"]
mod tests;
