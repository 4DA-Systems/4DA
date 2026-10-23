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
use crate::osv::fix_target::{self, LineTarget};
use crate::osv::types::MatchedAdvisory;

use super::types::{
    Action, Confidence, EvidenceCitation, EvidenceItem, EvidenceKind, LensHints, Urgency,
};
use super::upgrade_steps::{self, UpgradeStep};
// The plan tests persist and read the envelope through this module.
#[cfg(test)]
use super::plan_snapshot::{persist_upgrade_plan, read_upgrade_plan_snapshot};

/// Max advisory citations attached to one step (keeps items bounded; the count
/// in the title/explanation still reflects the true total).
const MAX_CITATIONS: usize = 8;

/// The ranked plan, its work order and the validation-drop canary — built in
/// one pass, so a step exists exactly when its item does.
pub struct BuiltPlan {
    pub items: Vec<EvidenceItem>,
    /// The machine-readable work order (AD-049), keyed by item id.
    pub steps: Vec<UpgradeStep>,
    pub drops: u32,
}

/// Items and drop count only — the shape the plan tests assert on.
#[cfg(test)]
pub fn build_upgrade_plan_with_drops(db: &Database) -> (Vec<EvidenceItem>, u32) {
    let plan = build_upgrade_plan(db);
    (plan.items, plan.drops)
}

/// Build the ranked dependency Upgrade Plan as validated [`EvidenceItem`]s,
/// most-actionable first, with each item's [`UpgradeStep`]s and the count of
/// items dropped because they failed `validate_item` (their steps go with
/// them). Empty on cold-start (no matches) or if the matcher errors — never
/// breaks on a data quirk.
///
/// Consumed by `preemption::compute_preemption_evidence_feed` (the Signal-tier
/// feed) which renders the items as the "Upgrade Plan" group and persists the
/// snapshot. The drop count is the snapshot's `validation_drop_count` — the
/// "thin plan" canary (a silent drop in a release build once shipped a truncated
/// citation as a dropped step; a reader that sees a non-zero count knows the
/// plan may under-report).
pub fn build_upgrade_plan(db: &Database) -> BuiltPlan {
    let matches = match crate::osv::matching::get_matched_advisories(db) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(target: "4da::upgrade_plan", error = %e, "matcher failed — empty plan");
            return BuiltPlan {
                items: Vec::new(),
                steps: Vec::new(),
                drops: 0,
            };
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
    // Maintenance notices (RustSec INFO unmaintained / deprecated: no fix, no
    // CVSS) are not upgrade steps — live 2026-09-06, 14 of 30 plan items were
    // "Upgrade gtk — clears 1 advisory" with nothing to upgrade to. They fold
    // into ONE Watch item at the end of the plan.
    let (informational, steps): (Vec<PackageGroup<'_>>, Vec<PackageGroup<'_>>) =
        groups.into_iter().partition(|g| g.informational);
    let mut candidates: Vec<(EvidenceItem, Vec<UpgradeStep>)> = steps
        .into_iter()
        .map(|mut g| {
            // One bump per dependency (audit H1): a declared package's target
            // rises to the release that also clears the copies it pins.
            g.raise_note = crate::osv::raised_lines::raise_declared_lines(
                db,
                &g.ecosystem_norm,
                &g.package,
                &mut g.lines,
            );
            if !g.raise_note.is_empty() {
                g.targets = fix_target::distinct_targets(&g.lines);
            }
            // How the fix reaches each transitive copy: its installed parent,
            // whether a lockfile refresh is enough and the command that does
            // it (`osv::fix_path`; empty for a direct-only row).
            g.routes =
                crate::osv::parent_hint::load_routes(db, &g.ecosystem_norm, &g.package, &g.lines);
            // How each DIRECT copy moves where it is declared (audit E8).
            g.direct_fixes =
                crate::osv::direct_upgrade::direct_fixes(&g.ecosystem_norm, &g.package, &g.lines);
            g.into_evidence_item(now)
        })
        .collect();
    if !informational.is_empty() {
        candidates.push(informational_item(informational, now));
    }
    let mut items = Vec::with_capacity(candidates.len());
    let mut work_order = Vec::with_capacity(candidates.len());
    let mut drops = 0u32;
    for (item, item_steps) in candidates {
        match super::validate::validate_item(&item) {
            Ok(()) => {
                items.push(item);
                work_order.extend(item_steps);
            }
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
    BuiltPlan {
        items,
        steps: work_order,
        drops,
    }
}

/// One package's aggregated upgrade step.
struct PackageGroup<'a> {
    package: String,
    ecosystem_norm: String,
    advisories: Vec<&'a MatchedAdvisory>,
    /// The projects this row speaks for — every one of them carries exactly the
    /// row's advisories (sorted). See [`split_by_applicable_advisories`].
    projects: Vec<String>,
    any_confirmed: bool,
    /// At least one affected instance is a DIRECT dep — the user can bump it now.
    fixable_now: bool,
    /// Per declaring project and target: the command or manifest edit that
    /// moves the direct copy (`osv::direct_upgrade`); empty when none is
    /// certain.
    direct_fixes: Vec<(String, String, crate::osv::direct_upgrade::DirectFix)>,
    /// Every affected instance is a dev dependency (a labelled discount).
    all_dev: bool,
    urgency: Urgency,
    max_cvss: f64,
    /// The target of each installed version in THIS row's projects (Phase
    /// 1.1). Never the machine-wide maximum: a copy in another project can
    /// neither set nor raise it.
    lines: Vec<LineTarget>,
    /// The distinct targets of `lines`, ascending. Usually one; a project
    /// holding two copies on two lines has two.
    targets: Vec<String>,
    /// Some line has a target — the only case in which the step may say
    /// "Upgrade" (2026-09-06: 14 of 30 live plan items said "Upgrade gtk —
    /// clears 1 advisory" with nothing to upgrade to).
    has_fix: bool,
    /// With no fix: the releases that fixed it but that the registry withdrew
    /// (yanked, deprecated; FX-H5). Non-empty means "no installable fix",
    /// never "no fix published".
    withheld: Vec<String>,
    /// With no fix: every fixing release brings in these (parity decision 2).
    introduces: Vec<String>,
    /// Every advisory is a maintenance notice (unmaintained / deprecated: no
    /// fix, no CVSS). Folded into ONE Watch item, never an upgrade step.
    informational: bool,
    /// Set only when this package produced MORE than one row, to keep the item
    /// ids distinct: the alphabetically-first advisory id of this row's set.
    /// `None` — the overwhelmingly common case — leaves the id untouched.
    cohort_key: Option<String>,
    /// The route of each transitive copy (`osv::parent_hint`,
    /// `osv::fix_path`). Filled after ranking; it never changes the rank.
    routes: crate::osv::parent_hint::TransitiveRoutes,
    /// Why a declared line's target was raised above its own advisories' fix
    /// (`osv::raised_lines`); empty in the common case.
    raise_note: String,
}

fn aggregate_by_package(matches: &[MatchedAdvisory]) -> Vec<PackageGroup<'_>> {
    use std::collections::BTreeMap;
    // Key by normalized (ecosystem, package) so npm/crates.io/etc. don't collide
    // and the same package across projects folds into one step — then split that
    // step per EXPOSURE, so the row is true for every project it names
    // (`osv::identity::split_by_exposure`, AD-044).
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
        .flat_map(|((ecosystem_norm, _pkg_lower), advisories)| {
            let cohorts = crate::osv::identity::split_by_exposure(&advisories);
            let keys = crate::osv::identity::exposure_keys(&cohorts);
            cohorts
                .into_iter()
                .zip(keys)
                .map(|(cohort, key)| package_group(&ecosystem_norm, cohort, key))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Build one row from one cohort. Every instance-derived judgement is scoped to
/// the projects the row names, so an install in a project this row does not
/// speak for can neither discount it nor withhold a discount from it.
fn package_group<'a>(
    ecosystem_norm: &str,
    (projects, advisories): (Vec<String>, Vec<&'a MatchedAdvisory>),
    cohort_key: Option<String>,
) -> PackageGroup<'a> {
    // Display name from the first advisory (preserves original casing).
    let package = advisories[0].package_name.clone();

    let instances = || {
        advisories
            .iter()
            .flat_map(|a| a.dependency_instances.iter())
            .filter(|d| projects.iter().any(|p| p == &d.project_path))
    };

    let any_confirmed = advisories.iter().any(|a| a.is_version_confirmed);
    let lines = fix_target::line_targets(&advisories, &projects);
    let targets = fix_target::distinct_targets(&lines);
    let has_fix = !targets.is_empty();
    // "Fixable now" needs something to bump TO: rsa's Marvin attack
    // (no fix exists) read "Fixable now via a direct dependency bump"
    // on the live plan (2026-09-06).
    let fixable_now = has_fix && instances().any(|d| d.is_direct);
    let mut withheld: Vec<String> = Vec::new();
    if !has_fix {
        for w in instances().flat_map(|d| d.withheld.iter()) {
            if !withheld.contains(w) {
                withheld.push(w.clone());
            }
        }
    }
    let introduces = fix_target::introduced_by(&lines);
    let informational = !has_fix
        && withheld.is_empty()
        && introduces.is_empty()
        && fix_target::malicious_of(&lines).is_empty()
        && advisories
            .iter()
            .all(|a| crate::osv::severity::is_informational(a));
    // The inputs of the ONE scope rule (AD-046): version-confirmed installs in
    // the projects this row names — the set the alert path reads too.
    let scope = crate::osv::identity::ExposureScope::of(&advisories, &projects);
    let all_dev = scope.all_dev() == Some(true);

    let max_cvss = advisories
        .iter()
        .filter_map(|a| a.cvss_score)
        .fold(0.0_f64, f64::max);

    // Most-urgent vulnerability (alias clusters, Phase 120), then the ONE
    // scope rule every surface grades by (`osv::identity::
    // scope_adjusted_urgency`, AD-046). Two copies of it drifted: the alert
    // path dropped a dev-only Critical to Medium and this one to High, and
    // before AD-040 a transitive-only Critical was High on one surface and
    // Critical on the other. One function, one input set.
    let base_urgency = crate::osv::identity::cluster_by_vulnerability(&advisories)
        .iter()
        .map(|cluster| crate::osv::severity::cluster_urgency(cluster))
        .min()
        .unwrap_or(Urgency::Medium);
    let urgency = if crate::osv::severity::contains_malware(&advisories) {
        Urgency::Critical
    } else {
        crate::osv::identity::scope_adjusted_urgency(
            base_urgency,
            scope.all_transitive(),
            scope.all_dev(),
        )
    };

    PackageGroup {
        package,
        ecosystem_norm: ecosystem_norm.to_string(),
        advisories,
        projects,
        any_confirmed,
        fixable_now,
        all_dev,
        urgency,
        max_cvss,
        lines,
        targets,
        has_fix,
        withheld,
        introduces,
        informational,
        cohort_key,
        routes: crate::osv::parent_hint::TransitiveRoutes::default(),
        direct_fixes: Vec::new(),
        raise_note: String::new(),
    }
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
        bool,
        std::cmp::Reverse<usize>,
        std::cmp::Reverse<i64>,
        String,
    ) {
        (
            self.urgency,
            // At equal (scope-adjusted) urgency a runtime exposure outranks a
            // dev-only one. Live 2026-10-02 both HIGH rows tied here and the
            // dev-only vitest (a Critical discounted for being dev-only) led
            // the runtime rmcp session leak on fixable-now alone.
            self.all_dev,
            !self.any_confirmed, // confirmed (true) sorts before unconfirmed
            !self.fixable_now,   // fixable-now sorts before waiting-on-upstream
            std::cmp::Reverse(self.projects.len()), // widest blast radius first
            std::cmp::Reverse((self.max_cvss * 1000.0) as i64), // higher CVSS first
            // Stable final tiebreak. The cohort key is part of it so a package
            // that produced several rows still orders deterministically.
            format!(
                "{}|{}",
                self.package.to_lowercase(),
                self.cohort_key.as_deref().unwrap_or("")
            ),
        )
    }

    /// One row per VULNERABILITY (Phase 120): the mirror stores a GHSA and
    /// its RUSTSEC twin as two rows; the plan counts, cites and grades the
    /// bug once ("clears 2 advisories" for one quinn-proto bug, 2026-09-07).
    fn representatives(&self) -> Vec<&'_ MatchedAdvisory> {
        crate::osv::identity::cluster_by_vulnerability(&self.advisories)
            .into_iter()
            .map(|cluster| cluster[0])
            .collect()
    }

    fn advisory_count(&self) -> usize {
        self.representatives().len()
    }

    /// The row's item and its one work-order step, from the same lines.
    fn into_evidence_item(self, now_millis: i64) -> (EvidenceItem, Vec<UpgradeStep>) {
        let n = self.advisory_count();
        let m = self.projects.len();
        let major = self.lines.iter().any(LineTarget::is_major);
        // One target per line the row's copies are on; two lines in one
        // project read "to >= 1.1.21 / 5.0.12", never the higher of the two.
        let target_note = if self.targets.is_empty() {
            String::new()
        } else {
            format!(
                " to >= {}{}",
                self.targets.join(" / "),
                if major { " (major)" } else { "" }
            )
        };

        // No fixed version anywhere in the group: the step must not say
        // "Upgrade" — there is nothing to upgrade to. It stays an Alert at its
        // CVSS urgency; the reader's move is to pin, patch or replace.
        // A transitive-only step names the one parent that is the move.
        let via = if self.fixable_now {
            String::new()
        } else {
            super::upgrade_parent::single_parent(&self.routes.links)
                .map(|p| format!(" via {p}"))
                .unwrap_or_default()
        };
        let title = clamp_title(if self.has_fix {
            format!(
                "Upgrade {pkg}{target}{via} — clears {n} {adv} across {m} {proj}",
                pkg = self.package,
                target = target_note,
                n = n,
                adv = plural(n, "advisory", "advisories"),
                m = m,
                proj = plural(m, "project", "projects"),
            )
        } else {
            format!(
                "{none} for {pkg} — {n} {adv} across {m} {proj}",
                none = if !self.introduces.is_empty()
                    || !fix_target::malicious_of(&self.lines).is_empty()
                {
                    "No clean fix"
                } else if self.withheld.is_empty() {
                    "No fix published"
                } else {
                    "No installable fix"
                },
                pkg = self.package,
                n = n,
                adv = plural(n, "advisory", "advisories"),
                m = m,
                proj = plural(m, "project", "projects"),
            )
        });

        let malicious = fix_target::malicious_of(&self.lines);
        let scope_note = if !self.has_fix && !malicious.is_empty() {
            // The MCP's order: the malware first, then what else blocks.
            let also = if self.introduces.is_empty() {
                String::new()
            } else {
                format!(
                    ". It also cannot move: {}",
                    fix_target::introduces_text(&self.introduces)
                )
            };
            format!(
                "No release is named: {}{also}",
                fix_target::malicious_text(&self.package, &malicious)
            )
        } else if !self.has_fix && !self.introduces.is_empty() {
            let why = fix_target::introduces_text(&self.introduces);
            format!("No release is named: {why} — pin, patch, or replace")
        } else if !self.has_fix && !self.withheld.is_empty() {
            format!(
                "The release that fixes it ({}) was withdrawn by its registry (yanked or \
                 deprecated), and no later release is known to clear it — pin, patch, or replace",
                self.withheld.join(", ")
            )
        } else if !self.has_fix {
            "No fix has been published — pin, patch, or replace; there is nothing to upgrade to yet"
                .to_string()
        } else if self.fixable_now {
            // A row can hold direct and transitive copies: a bump only fixes
            // the declared ones (bumping where it is not declared would add a
            // second, direct copy and leave the vulnerable one in place).
            let transitive = self
                .lines
                .iter()
                .any(|l| l.target_version.is_some() && l.sites.iter().any(|s| !s.is_direct));
            let how = crate::osv::direct_upgrade::direct_fix_note(&self.direct_fixes, m > 1);
            // The declared range already admits the fix: a refresh, not a bump.
            let lead = if crate::osv::direct_upgrade::all_refresh(&self.direct_fixes, &self.lines) {
                "Fixable now with a lockfile refresh (the declared range already admits the fix)"
            } else {
                "Fixable now via a direct dependency bump"
            };
            if transitive {
                format!(
                    "{lead} where it is declared{how}. Elsewhere it is transitive — {}",
                    super::upgrade_parent::upstream_note(&self.package, &self.routes, m > 1)
                )
            } else {
                format!("{lead}{how}")
            }
        } else {
            super::upgrade_parent::upstream_note(&self.package, &self.routes, m > 1)
        };
        let dev_note = if self.all_dev {
            " All affected instances are dev-only."
        } else {
            ""
        };
        let target_detail = format!(
            "{}{}",
            target_detail(&self.package, &self.lines, major),
            self.raise_note
        );
        let representatives = self.representatives();
        let ids: Vec<&str> = representatives
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
        // An inherited grade is never presented as the record's own: the
        // sentence names the GHSA alias it came from (`osv::severity`).
        let graded = crate::osv::severity::group_severity(&self.advisories);
        let severity_note = crate::osv::severity::inherited_note(graded.as_ref(), &self.advisories)
            .map(|note| format!(" {note}"))
            .unwrap_or_default();
        let explanation = format!(
            "{pkg} has {n} version-confirmed {adv} matching installed versions across {m} {proj}.{sev} \
             {scope}.{dev}{targets} Advisories: {ids}{more}.",
            sev = severity_note,
            pkg = self.package,
            n = n,
            adv = plural(n, "advisory", "advisories"),
            m = m,
            proj = plural(m, "project", "projects"),
            scope = scope_note,
            dev = dev_note,
            targets = target_detail,
            ids = ids.join(", "),
            more = more_note,
        );

        // Citations: one per vulnerability (capped), plus the affected-projects
        // context. The installed version cited is the one in THIS row's
        // projects, not the advisory's machine-wide representative.
        let mut evidence: Vec<EvidenceCitation> = representatives
            .iter()
            .take(MAX_CITATIONS)
            .map(|a| EvidenceCitation {
                source: "osv-advisory".to_string(),
                title: truncate(&a.summary, 160),
                url: a.source_url.clone(),
                freshness_days: advisory_age_days(a.published_at.as_deref()),
                relevance_note: truncate(
                    &format!(
                        "{sev}affects installed {ver}",
                        sev = citation_severity(a),
                        ver = installed_in(a, &self.projects),
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
        evidence.extend(line_citations(&self.lines));
        evidence.extend(super::upgrade_parent::path_citations(
            &self.package,
            &self.routes.links,
        ));

        // Confidence: heuristic ranking; a shade higher when the fix is a direct
        // bump the user controls, a shade lower when there is no fix to point
        // at. All groups here are version-confirmed.
        let confidence_value = if !self.has_fix {
            0.7
        } else if self.fixable_now {
            0.9
        } else {
            0.8
        };

        let suggested_actions = plan_actions(format!(
            "Review the {n} {adv} affecting {pkg}",
            n = n,
            adv = plural(n, "advisory", "advisories"),
            pkg = self.package
        ));

        // A package that speaks for one situation keeps the id it always had;
        // only a package that split needs a discriminator, so triage state on
        // every other row survives this change untouched.
        let id = match &self.cohort_key {
            Some(key) => format!(
                "upgrade-plan:{}:{}:{}",
                self.ecosystem_norm,
                self.package.to_lowercase(),
                key
            ),
            None => format!(
                "upgrade-plan:{}:{}",
                self.ecosystem_norm,
                self.package.to_lowercase()
            ),
        };

        let step = upgrade_steps::step_for(
            &id,
            &self.ecosystem_norm,
            &self.package,
            &self.lines,
            &self.advisories,
        );
        let item = EvidenceItem {
            id,
            kind: EvidenceKind::Alert,
            title,
            explanation,
            confidence: Confidence::heuristic(confidence_value),
            urgency: self.urgency,
            reversibility: None,
            evidence,
            evidence_total: None,
            affected_projects: self.projects,
            affected_deps: vec![self.package],
            suggested_actions,
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::upgrade_plan(),
            created_at: now_millis,
            expires_at: None,
        };
        (item, vec![step])
    }
}

/// The severity a citation's note leads with: the CVSS score, or a grade the
/// record inherited from its GHSA alias, named as such. Empty otherwise.
fn citation_severity(a: &MatchedAdvisory) -> String {
    match (
        a.cvss_score,
        a.severity_from.as_deref(),
        a.severity_label.as_deref(),
    ) {
        (Some(c), _, _) => format!("CVSS {c:.1}; "),
        (None, Some(from), Some(tier)) => {
            format!("{}; ", crate::osv::severity::labelled(tier, Some(from)))
        }
        _ => String::new(),
    }
}

/// One `fix-target` citation per installed version (Phase 1.2), carried in
/// the canonical item rather than a new type: title `"1.1.12 -> 1.1.21"`, and
/// a note with the upgrade type, whether it clears every known advisory, and
/// each install site as `project (direct|transitive[, dev])`.
fn line_citations(lines: &[LineTarget]) -> Vec<EvidenceCitation> {
    lines
        .iter()
        .map(|l| {
            let none = if l.introduces.is_empty() {
                "no fix published"
            } else {
                "no clean fix"
            };
            let target = l.target_version.as_deref().unwrap_or(none);
            let kind = match l.upgrade_type {
                Some(fix_target::UpgradeType::Patch) => "patch",
                Some(fix_target::UpgradeType::Minor) => "minor",
                Some(fix_target::UpgradeType::Major) => "major",
                None => "unknown",
            };
            let clears = if l.target_version.is_none() {
                ""
            } else if l.clears_all_known {
                "; clears every known advisory"
            } else {
                "; clears only the advisories with a fix"
            };
            let sites: Vec<String> = l
                .sites
                .iter()
                .map(|s| {
                    format!(
                        "{} ({}{})",
                        s.project_path,
                        if s.is_direct { "direct" } else { "transitive" },
                        if s.is_dev { ", dev" } else { "" }
                    )
                })
                .collect();
            EvidenceCitation {
                source: "fix-target".to_string(),
                title: truncate(&format!("{} -> {target}", l.installed_version), 160),
                url: None,
                freshness_days: 0.0,
                relevance_note: truncate(
                    &format!("{kind} upgrade{clears}. Installed in: {}", sites.join(", ")),
                    200,
                ),
            }
        })
        .collect()
}

/// The per-line sentence of a step's explanation: where each copy goes when
/// there is more than one line, that the move is a major one, and that the
/// target leaves an unfixed advisory open. Empty in the common case (one line,
/// in-major, clears everything) — the title already says it.
fn target_detail(package: &str, lines: &[LineTarget], major: bool) -> String {
    let mut out = String::new();
    if lines.iter().filter(|l| l.target_version.is_some()).count() > 1 {
        out.push_str(&format!(
            " Each installed version has its own target: {}.",
            fix_target::describe_lines(lines)
        ));
    }
    if major {
        out.push_str(
            " The target crosses a major version — breaking changes are possible; \
             check the changelog before upgrading.",
        );
    }
    if lines
        .iter()
        .any(|l| l.target_version.is_some() && !l.clears_all_known)
    {
        out.push_str(&format!(
            " No published version of {package} clears every known advisory; \
             the target clears the ones that have a fix."
        ));
    }
    out
}

/// The installed versions of `advisory`'s confirmed copies in `projects`
/// ("1.1.18, 5.0.9"); "version" when none is known.
fn installed_in(advisory: &MatchedAdvisory, projects: &[String]) -> String {
    let mut versions: Vec<&str> = advisory
        .dependency_instances
        .iter()
        .filter(|d| d.is_version_confirmed)
        .filter(|d| projects.iter().any(|p| p == &d.project_path))
        .filter_map(|d| d.installed_version.as_deref())
        .collect();
    versions.sort_unstable();
    versions.dedup();
    if versions.is_empty() {
        "version".to_string()
    } else {
        versions.join(", ")
    }
}

/// ONE Watch-urgency item for every package whose advisories are all
/// maintenance notices. The reader learns which of their dependencies are
/// unmaintained and where, without fourteen "Upgrade X" steps that have no
/// version to upgrade to (live 2026-09-06: gtk ×10, paste, proc-macro-error
/// ×2, ttf-parser — Tauri's Linux GTK3 bindings and their transitive tail).
/// Its work order is one `no_fix` step per package named.
fn informational_item(
    groups: Vec<PackageGroup<'_>>,
    now_millis: i64,
) -> (EvidenceItem, Vec<UpgradeStep>) {
    const ID: &str = "upgrade-plan:informational";
    let steps = upgrade_steps::merge_same_package(
        groups
            .iter()
            .map(|g| {
                upgrade_steps::step_for(ID, &g.ecosystem_norm, &g.package, &g.lines, &g.advisories)
            })
            .collect(),
    );
    // Count DEPENDENCIES, not rows: a package that split into several rows
    // (`split_by_applicable_advisories`) must still be named once and counted
    // once — "7 unmaintained dependencies" is a claim about dependencies. The
    // rows are ranked, not grouped by package, so dedup by first sighting.
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let names: Vec<String> = groups
        .iter()
        .filter(|g| seen.insert(g.package.as_str()))
        .map(|g| g.package.clone())
        .collect();
    let k = names.len();
    let mut projects: Vec<String> = groups
        .iter()
        .flat_map(|g| g.projects.iter().cloned())
        .collect();
    projects.sort();
    projects.dedup();
    let m = projects.len();

    let listed: Vec<&str> = names.iter().take(3).map(String::as_str).collect();
    let more = k.saturating_sub(listed.len());
    let more_note = if more > 0 {
        format!(" +{more} more")
    } else {
        String::new()
    };
    let title = clamp_title(format!(
        "{k} unmaintained {dep} — no fix to apply ({list}{more_note})",
        dep = plural(k, "dependency", "dependencies"),
        list = listed.join(", "),
    ));
    // Each named as what it is (#986): `unmaintained: <crate> (<id>), no
    // exploitable vulnerability` (`osv::severity::notice_wording`).
    let notices: Vec<String> = names
        .iter()
        .map(|name| {
            let mut ids: Vec<&str> = groups
                .iter()
                .filter(|g| &g.package == name)
                .flat_map(|g| g.advisories.iter().map(|a| a.advisory_id.as_str()))
                .collect();
            ids.sort_unstable();
            ids.dedup();
            crate::osv::severity::notice_wording(name, &ids)
        })
        .collect();
    let explanation = format!(
        "{list}. There is nothing to upgrade to; replace {them} when convenient. \
         Installed across {m} {proj}.",
        list = notices.join("; "),
        them = if k == 1 { "it" } else { "them" },
        proj = plural(m, "project", "projects"),
    );

    let mut evidence: Vec<EvidenceCitation> = groups
        .iter()
        .take(MAX_CITATIONS)
        .filter_map(|g| g.advisories.first().map(|a| (g, a)))
        .map(|(g, a)| EvidenceCitation {
            source: "osv-advisory".to_string(),
            title: truncate(&a.summary, 160),
            url: a.source_url.clone(),
            freshness_days: advisory_age_days(a.published_at.as_deref()),
            relevance_note: truncate(
                &format!(
                    "{}: no fixed version; installed {}",
                    g.package,
                    installed_in(a, &g.projects)
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
        relevance_note: truncate(&format!("Affected projects: {}", projects.join(", ")), 200),
    });

    let item = EvidenceItem {
        id: ID.to_string(),
        kind: EvidenceKind::Alert,
        title,
        explanation,
        confidence: Confidence::heuristic(0.8),
        urgency: Urgency::Watch,
        reversibility: None,
        evidence,
        evidence_total: None,
        affected_projects: projects,
        affected_deps: names,
        suggested_actions: plan_actions(format!(
            "Review the maintenance notices on {k} {dep}",
            dep = plural(k, "dependency", "dependencies")
        )),
        precedents: Vec::new(),
        refutation_condition: None,
        lens_hints: LensHints::upgrade_plan(),
        created_at: now_millis,
        expires_at: None,
    };
    (item, steps)
}

/// The plan's four informational actions (doctrine rule 5: none executes an
/// upgrade). `review_description` names what "Review advisories" opens.
fn plan_actions(review_description: String) -> Vec<Action> {
    vec![
        Action {
            action_id: "review_security".to_string(),
            label: "Review advisories".to_string(),
            description: review_description,
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
    ]
}

// ---- helpers ----

fn normalize_ecosystem(eco: &str) -> String {
    crate::ecosystem::Ecosystem::parse(eco)
        .map_or_else(|| eco.to_string(), |e| e.osv_name().to_string())
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
/// Shared with `install_drift`, whose citations obey the same 200-byte bound.
pub(super) fn truncate(s: &str, max: usize) -> String {
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

/// A citation age the UI shows as unknown (no label) — never "today".
pub(crate) const UNKNOWN_AGE_DAYS: f32 = -1.0;

/// Days since an advisory was published (OSV `published`: RFC 3339, SQLite
/// `YYYY-MM-DD HH:MM:SS`, or a bare date), or [`UNKNOWN_AGE_DAYS`] without a
/// parseable date. The citations carried a 0.0 placeholder, so 2023 CVEs read
/// "today" (fresh-profile audit E9).
pub(crate) fn advisory_age_days(published: Option<&str>) -> f32 {
    let Some(p) = published.map(str::trim).filter(|p| !p.is_empty()) else {
        return UNKNOWN_AGE_DAYS;
    };
    chrono::DateTime::parse_from_rfc3339(p)
        .map(|d| d.naive_utc())
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(p, "%Y-%m-%d %H:%M:%S"))
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(p.get(..10).unwrap_or(p), "%Y-%m-%d")
                .map(|d| d.and_time(chrono::NaiveTime::MIN))
        })
        .map(|at| ((chrono::Utc::now().naive_utc() - at).num_hours() as f32 / 24.0).max(0.0))
        .unwrap_or(UNKNOWN_AGE_DAYS)
}

#[cfg(test)]
#[path = "upgrade_targets_tests.rs"]
mod target_tests;

#[cfg(test)]
#[path = "upgrade_plan_tests.rs"]
mod tests;
