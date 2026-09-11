// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Vulnerability identity (Phase 120): which OSV rows are the SAME bug, and
//! the severity tier every surface shares for one such bug — plus (AD-046)
//! the one scope rule every surface grades an install of it by.
//!
//! Split out of `matching.rs` (file-size gate).

use std::collections::HashMap;

use crate::evidence::Urgency;

use super::types::MatchedAdvisory;

// ============================================================================
// Vulnerability identity (Phase 120)
// ============================================================================

/// Group advisory rows that describe the SAME vulnerability under different
/// ids. The mirror holds one row per id, and OSV publishes the same bug as a
/// GHSA and a RUSTSEC (and a CVE) record: quinn-proto's memory-exhaustion
/// bug read as "2 version-confirmed advisories" on Preemption, the upgrade
/// plan and the MCP signals (2026-09-07).
///
/// Two rows join when any id of one appears in the other's `aliases`
/// (transitively — union-find). Rows synced before the column existed carry
/// no aliases; for those the fallback is the story itself: same package,
/// same fixed version, and summaries that are equal once the source's
/// "Crate:" prefix and whitespace are normalised (RustSec drops the prefix
/// GitHub keeps). Different fixes never merge — three distinct openssl bugs
/// with three different summaries stay three.
///
/// Clusters keep first-seen order; within a cluster the GHSA row (the
/// GitHub-reviewed record) leads so the representative id is stable.
pub fn cluster_by_vulnerability<'a>(
    advisories: &[&'a MatchedAdvisory],
) -> Vec<Vec<&'a MatchedAdvisory>> {
    let n = advisories.len();
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], i: usize) -> usize {
        let mut root = i;
        while parent[root] != root {
            root = parent[root];
        }
        let mut cur = i;
        while parent[cur] != root {
            let next = parent[cur];
            parent[cur] = root;
            cur = next;
        }
        root
    }
    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
        }
    }

    let mut by_id: HashMap<String, usize> = HashMap::new();
    for (i, adv) in advisories.iter().enumerate() {
        for id in std::iter::once(&adv.advisory_id).chain(adv.aliases.iter()) {
            let key = id.trim().to_ascii_uppercase();
            if key.is_empty() {
                continue;
            }
            match by_id.get(&key) {
                Some(&j) => union(&mut parent, i, j),
                None => {
                    by_id.insert(key, i);
                }
            }
        }
    }
    for i in 0..n {
        for j in (i + 1)..n {
            if advisories[i].aliases.is_empty()
                && advisories[j].aliases.is_empty()
                && same_story(advisories[i], advisories[j])
            {
                union(&mut parent, i, j);
            }
        }
    }

    let mut order: Vec<usize> = Vec::new();
    let mut members: HashMap<usize, Vec<&'a MatchedAdvisory>> = HashMap::new();
    for (i, adv) in advisories.iter().enumerate() {
        let root = find(&mut parent, i);
        let entry = members.entry(root).or_insert_with(|| {
            order.push(root);
            Vec::new()
        });
        entry.push(adv);
    }
    order
        .into_iter()
        .filter_map(|root| members.remove(&root))
        .map(|mut cluster| {
            cluster.sort_by_key(|a| !a.advisory_id.to_ascii_uppercase().starts_with("GHSA-"));
            cluster
        })
        .collect()
}

/// Legacy-row identity: the same package, the same fix, and the same story
/// once the source's leading "Name:" prefix and whitespace are normalised.
fn same_story(a: &MatchedAdvisory, b: &MatchedAdvisory) -> bool {
    if !a.package_name.eq_ignore_ascii_case(&b.package_name) || a.fixed_version != b.fixed_version {
        return false;
    }
    let sa = normalize_summary(&a.summary);
    let sb = normalize_summary(&b.summary);
    !sa.is_empty() && !sb.is_empty() && (sa == sb || sa.contains(&sb) || sb.contains(&sa))
}

fn normalize_summary(summary: &str) -> String {
    let trimmed = summary.trim();
    // "Quinn: Remote memory exhaustion…" → "Remote memory exhaustion…": a
    // single leading word followed by a colon is the source's crate prefix.
    let body = match trimmed.split_once(':') {
        Some((head, rest)) if !head.trim().contains(' ') && head.len() <= 40 => rest,
        _ => trimmed,
    };
    body.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The four-tier severity every surface shares, for one vulnerability
/// cluster: the highest CVSS band across the cluster's rows when any row is
/// scored, else the most severe curated label. `None` when the source gives
/// neither (RustSec maintenance notices) — the caller decides the fallback.
pub fn cluster_severity_tier(cluster: &[&MatchedAdvisory]) -> Option<&'static str> {
    let max_cvss = cluster
        .iter()
        .filter_map(|m| m.cvss_score)
        .fold(None, |acc: Option<f64>, s| {
            Some(acc.map_or(s, |a| a.max(s)))
        });
    if let Some(score) = max_cvss {
        return Some(crate::osv::types::cvss_band(score));
    }
    cluster
        .iter()
        .filter_map(|m| m.severity_label.as_deref())
        .filter_map(canonical_tier)
        .min_by_key(|t| tier_rank(t))
}

// ============================================================================
// Exposure identity (AD-044)
// ============================================================================

/// Split one package's matched advisories into groups that are each true on
/// their own — the projects sharing a group all carry exactly its advisories.
///
/// A package is installed at whatever version each project pinned, so an
/// advisory applies per PROJECT. Folding all of a package's advisories into one
/// row and grading it by the most urgent lends that severity to every project
/// the row names. Live 2026-09-09, Preemption's #1 row: "Upgrade vitest to
/// >= 4.1.11 — clears 2 advisories across 2 projects", **critical**, naming
/// `navcal` (3.2.4) and `D:\4DA` (3.2.6) — GHSA-5xrq-8626-4rwp is fixed in
/// exactly 3.2.6, so D:\4DA was never exposed to the critical. The AI brief then
/// told the reader to "bump to a clean version of `vitest@2` or above": 2.x is
/// affected by BOTH advisories.
///
/// Both aggregators over `MatchedAdvisory` use this — the upgrade plan
/// (`evidence::upgrade_plan`) and the alert path (`preemption`) — so the
/// Signal-tier feed, the free floor and the Brief cannot disagree about who is
/// exposed to what.
///
/// One group — one version, or several versions the same advisories cover — is
/// the un-split behavior. Groups and their project lists are sorted, so the
/// output is deterministic.
pub fn split_by_exposure<'a>(
    advisories: &[&'a MatchedAdvisory],
) -> Vec<(Vec<String>, Vec<&'a MatchedAdvisory>)> {
    use std::collections::{BTreeMap, BTreeSet};

    // A match always names at least one affected project — the matcher emits one
    // only when an instance matched. If that ever stops holding, keep the single
    // group so no advisory is dropped on the floor.
    if advisories.iter().any(|a| a.project_paths.is_empty()) {
        let mut projects: Vec<String> = advisories
            .iter()
            .flat_map(|a| a.project_paths.iter().cloned())
            .collect();
        projects.sort();
        projects.dedup();
        return vec![(projects, advisories.to_vec())];
    }

    let mut per_project: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for (idx, adv) in advisories.iter().enumerate() {
        for project in &adv.project_paths {
            per_project.entry(project.as_str()).or_default().insert(idx);
        }
    }

    let mut by_advisory_set: BTreeMap<BTreeSet<usize>, Vec<String>> = BTreeMap::new();
    for (project, applicable) in per_project {
        by_advisory_set
            .entry(applicable)
            .or_default()
            .push(project.to_string());
    }

    by_advisory_set
        .into_iter()
        .map(|(applicable, projects)| {
            (
                projects,
                applicable.iter().map(|&idx| advisories[idx]).collect(),
            )
        })
        .collect()
}

/// The id discriminator for a package that split into several groups: the
/// alphabetically-first advisory id of THIS group. `None` keeps the id a
/// package's row has always had, so triage state on every unsplit row survives.
pub fn exposure_key(split: bool, advisories: &[&MatchedAdvisory]) -> Option<String> {
    split.then(|| {
        advisories
            .iter()
            .map(|a| a.advisory_id.to_lowercase())
            .min()
            .unwrap_or_default()
    })
}

// ============================================================================
// Exposure scope (AD-046)
// ============================================================================

/// The ONE scope rule every dependency surface grades an advisory by (AD-046).
///
/// Two copies existed and disagreed, although AD-040 said they matched:
/// `preemption::rank_osv_urgency` (the free floor, and through
/// `get_preemption_feed` the AI brief) dropped a dev-only Critical or High
/// straight to Medium, while `evidence::upgrade_plan` dropped a dev-only row
/// ONE level (Critical -> High). A dev-only direct Critical read Medium in the
/// brief and High on the tab. Nobody saw it only because the lockfile walk
/// recorded every install `is_dev = 0` — fixing that alone would have made
/// `sandbox` Medium in the brief and High on the tab.
///
/// Applied in order:
/// 1. a transitive-only install clamps Critical to High (AD-040: reachability
///    through a parent is unproven);
/// 2. a dev-only install then drops ONE level — Critical -> High, High ->
///    Medium, Medium -> Watch; Watch stays.
///
/// `None` is unknown and discounts nothing. The MCP server's TypeScript twin
/// carries the same name: change both or neither.
pub fn scope_adjusted_urgency(
    base: Urgency,
    all_transitive: Option<bool>,
    all_dev: Option<bool>,
) -> Urgency {
    let reachable = if all_transitive == Some(true) && base == Urgency::Critical {
        Urgency::High
    } else {
        base
    };
    if all_dev != Some(true) {
        return reachable;
    }
    match reachable {
        Urgency::Critical => Urgency::High,
        Urgency::High => Urgency::Medium,
        Urgency::Medium | Urgency::Watch => Urgency::Watch,
    }
}

/// Which kinds of install an aggregate row speaks for: the version-CONFIRMED
/// instances in the projects it names (AD-044). Both aggregators — the
/// upgrade plan and the alert path — read the inputs of
/// [`scope_adjusted_urgency`] from here, so one rule is never again applied
/// to two different instance sets (the plan counted unconfirmed instances;
/// the alert path did not).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExposureScope {
    pub direct_runtime: bool,
    pub transitive_runtime: bool,
    pub direct_dev: bool,
    pub transitive_dev: bool,
}

impl ExposureScope {
    pub fn of(advisories: &[&MatchedAdvisory], projects: &[String]) -> Self {
        let mut scope = Self::default();
        let instances = advisories
            .iter()
            .flat_map(|advisory| advisory.dependency_instances.iter())
            .filter(|instance| instance.is_version_confirmed)
            .filter(|instance| projects.iter().any(|p| p == &instance.project_path));
        for instance in instances {
            match (instance.is_direct, instance.is_dev) {
                (true, false) => scope.direct_runtime = true,
                (false, false) => scope.transitive_runtime = true,
                (true, true) => scope.direct_dev = true,
                (false, true) => scope.transitive_dev = true,
            }
        }
        scope
    }

    fn is_known(&self) -> bool {
        self.direct_runtime || self.transitive_runtime || self.direct_dev || self.transitive_dev
    }

    /// Every install is transitive; `None` when there is no install to judge.
    pub fn all_transitive(&self) -> Option<bool> {
        self.is_known()
            .then_some(!self.direct_runtime && !self.direct_dev)
    }

    /// Every install is dev-only; `None` when there is no install to judge.
    pub fn all_dev(&self) -> Option<bool> {
        self.is_known()
            .then_some(!self.direct_runtime && !self.transitive_runtime)
    }
}

fn canonical_tier(label: &str) -> Option<&'static str> {
    match label {
        "critical" => Some("critical"),
        "high" => Some("high"),
        "medium" => Some("medium"),
        "low" => Some("low"),
        _ => None,
    }
}

fn tier_rank(tier: &str) -> u8 {
    match tier {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adv(
        id: &str,
        pkg: &str,
        fix: Option<&str>,
        summary: &str,
        aliases: &[&str],
        cvss: Option<f64>,
        label: Option<&str>,
    ) -> MatchedAdvisory {
        MatchedAdvisory {
            advisory_id: id.to_string(),
            summary: summary.to_string(),
            details: None,
            package_name: pkg.to_string(),
            ecosystem: "crates.io".to_string(),
            installed_version: Some("0.11.14".to_string()),
            fixed_version: fix.map(str::to_string),
            severity_type: None,
            cvss_score: cvss,
            source_url: None,
            is_version_confirmed: true,
            project_paths: vec!["/p".to_string()],
            published_at: None,
            dependency_instances: vec![],
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            severity_label: label.map(str::to_string),
        }
    }

    /// AD-044: `vitest` live 2026-09-09 — the critical advisory reaches only
    /// the project on 3.2.4; the medium reaches both. Two exposures, and the
    /// project on the critical's own fix version is not in the critical's group.
    #[test]
    fn split_by_exposure_separates_projects_with_different_advisory_sets() {
        let mut crit = adv(
            "GHSA-crit",
            "vitest",
            Some("3.2.6"),
            "crit",
            &[],
            Some(9.8),
            None,
        );
        crit.project_paths = vec!["/behind".to_string()];
        let mut med = adv(
            "GHSA-med",
            "vitest",
            Some("4.1.11"),
            "med",
            &[],
            Some(5.9),
            None,
        );
        med.project_paths = vec!["/behind".to_string(), "/current".to_string()];

        let groups = split_by_exposure(&[&crit, &med]);
        assert_eq!(groups.len(), 2, "two exposures");

        let (behind_projects, behind_advs) = groups
            .iter()
            .find(|(p, _)| p == &vec!["/behind".to_string()])
            .expect("the exposed project has its own group");
        assert_eq!(behind_projects.len(), 1);
        assert_eq!(behind_advs.len(), 2, "it carries both advisories");

        let (_, current_advs) = groups
            .iter()
            .find(|(p, _)| p == &vec!["/current".to_string()])
            .expect("the project on the fix version has its own group");
        assert_eq!(current_advs.len(), 1, "only the advisory it is exposed to");
        assert_eq!(current_advs[0].advisory_id, "GHSA-med");

        // Split -> a discriminator; unsplit -> the id the row always had.
        assert_eq!(
            exposure_key(true, current_advs),
            Some("ghsa-med".to_string())
        );
        assert_eq!(exposure_key(false, current_advs), None);
    }

    /// The common case: every project carries the same advisories, so nothing
    /// splits and the caller keeps its single row and its stable id.
    #[test]
    fn split_by_exposure_keeps_one_group_when_every_project_shares_the_set() {
        let mut a = adv(
            "GHSA-a",
            "lodash",
            Some("4.17.21"),
            "a",
            &[],
            Some(7.5),
            None,
        );
        a.project_paths = vec!["/x".to_string(), "/y".to_string()];
        let mut b = adv(
            "GHSA-b",
            "lodash",
            Some("4.17.21"),
            "b",
            &[],
            Some(5.0),
            None,
        );
        b.project_paths = vec!["/y".to_string(), "/x".to_string()];

        let groups = split_by_exposure(&[&a, &b]);
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].0,
            vec!["/x".to_string(), "/y".to_string()],
            "projects come back sorted"
        );
        assert_eq!(groups[0].1.len(), 2);
    }

    /// A match with no project attribution must never disappear: fall back to
    /// one group holding every advisory.
    #[test]
    fn split_by_exposure_never_drops_an_unattributed_advisory() {
        let mut a = adv("GHSA-a", "pkg", Some("2.0.0"), "a", &[], Some(7.5), None);
        a.project_paths = vec![];
        let b = adv("GHSA-b", "pkg", Some("2.0.0"), "b", &[], Some(5.0), None);

        let groups = split_by_exposure(&[&a, &b]);
        assert_eq!(groups.len(), 1, "one group, nothing lost");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[0].0, vec!["/p".to_string()]);
    }

    /// Phase 120: the GHSA row lists the RUSTSEC id as an alias; the RUSTSEC
    /// row (synced first, no aliases yet) joins it — ONE vulnerability, with
    /// the GHSA record leading. Before this the live plan said "clears 2
    /// advisories" for one quinn-proto bug.
    #[test]
    fn alias_twins_cluster_as_one_vulnerability() {
        let rustsec = adv(
            "RUSTSEC-2026-0185",
            "quinn-proto",
            Some("0.11.15"),
            " Remote memory exhaustion in quinn-proto",
            &[],
            Some(7.5),
            None,
        );
        let ghsa = adv(
            "GHSA-4w2j-m93h-cj5j",
            "quinn-proto",
            Some("0.11.15"),
            "Quinn: Remote memory exhaustion in quinn-proto",
            &["CVE-2026-25800", "RUSTSEC-2026-0185"],
            Some(7.5),
            Some("high"),
        );
        let clusters = cluster_by_vulnerability(&[&rustsec, &ghsa]);
        assert_eq!(clusters.len(), 1, "alias twins are one vulnerability");
        assert_eq!(clusters[0][0].advisory_id, "GHSA-4w2j-m93h-cj5j");
        assert_eq!(cluster_severity_tier(&clusters[0]), Some("high"));
    }

    /// Rows synced before the aliases column existed carry none; the same
    /// package + fix + story still joins them, a different fix never does.
    #[test]
    fn legacy_rows_cluster_by_story_not_by_fix_alone() {
        let a = adv(
            "GHSA-a",
            "glib",
            Some("0.20.0"),
            "Unsoundness in VariantStrIter",
            &[],
            None,
            Some("medium"),
        );
        let b = adv(
            "RUSTSEC-b",
            "glib",
            Some("0.20.0"),
            "Unsoundness in  VariantStrIter",
            &[],
            None,
            None,
        );
        let c = adv(
            "GHSA-c",
            "glib",
            Some("0.21.0"),
            "Unsoundness in VariantStrIter",
            &[],
            None,
            None,
        );
        let clusters = cluster_by_vulnerability(&[&a, &b, &c]);
        assert_eq!(
            clusters.len(),
            2,
            "same story+fix merges; a different fix stays apart"
        );
        assert_eq!(clusters[0].len(), 2);
        assert_eq!(cluster_severity_tier(&clusters[0]), Some("medium"));
        assert_eq!(cluster_severity_tier(&clusters[1]), None);
    }

    /// Three distinct openssl bugs with three summaries and two fix versions
    /// stay three — clustering must never collapse real breadth.
    #[test]
    fn distinct_vulnerabilities_stay_separate() {
        let a = adv(
            "GHSA-1",
            "openssl",
            Some("0.10.80"),
            "OOB write in cipher_update_inplace",
            &["CVE-1"],
            None,
            Some("medium"),
        );
        let b = adv(
            "GHSA-2",
            "openssl",
            Some("0.10.79"),
            "UB in ocsp_responders",
            &["CVE-2"],
            None,
            Some("high"),
        );
        let c = adv(
            "GHSA-3",
            "openssl",
            Some("0.10.79"),
            "Heap overflow in key wrap",
            &["CVE-3"],
            None,
            Some("medium"),
        );
        assert_eq!(cluster_by_vulnerability(&[&a, &b, &c]).len(), 3);
    }

    /// CVSS decides the tier when present; the curated label decides when the
    /// CVSS block is unscorable (GHSA-h395-gr6q-cpjc: a v4 vector the mirror
    /// cannot score, labelled MODERATE by GitHub → medium, not critical).
    #[test]
    fn severity_tier_prefers_cvss_then_curated_label() {
        let scored = adv("GHSA-s", "x", None, "s", &[], Some(9.1), Some("low"));
        assert_eq!(cluster_severity_tier(&[&scored]), Some("critical"));
        let labelled = adv(
            "GHSA-l",
            "jsonwebtoken",
            Some("10.3.0"),
            "Type confusion",
            &[],
            None,
            Some("medium"),
        );
        assert_eq!(cluster_severity_tier(&[&labelled]), Some("medium"));
        let bare = adv(
            "RUSTSEC-m",
            "atk",
            None,
            "no longer maintained",
            &[],
            None,
            None,
        );
        assert_eq!(cluster_severity_tier(&[&bare]), None);
    }

    /// AD-046: the specified table, row for row.
    #[test]
    fn scope_adjusted_urgency_matches_the_table() {
        use crate::evidence::Urgency::{Critical, High, Medium, Watch};
        let cases = [
            // (base, all_transitive, all_dev) -> expected
            (Critical, Some(false), Some(false), Critical), // direct runtime
            (Critical, Some(true), Some(false), High),      // transitive runtime (AD-040)
            (Critical, Some(false), Some(true), High),      // direct dev
            (Critical, Some(true), Some(true), Medium),     // transitive dev: sandbox
            (High, Some(false), Some(true), Medium),        // direct dev
            (High, Some(true), Some(true), Medium),         // transitive dev
            (High, Some(true), Some(false), High),          // the clamp is Critical-only
            (Medium, Some(false), Some(false), Medium),     // jsonwebtoken
            (Medium, Some(true), Some(true), Watch),
            (Watch, Some(true), Some(true), Watch), // never below Watch
            (Critical, None, None, Critical),       // unknown discounts nothing
        ];
        for (base, transitive, dev, expected) in cases {
            assert_eq!(
                scope_adjusted_urgency(base, transitive, dev),
                expected,
                "{base:?} all_transitive={transitive:?} all_dev={dev:?}"
            );
        }
    }

    /// The inputs come from version-confirmed installs in the NAMED projects
    /// only — an unconfirmed copy, or a copy in a project the row does not
    /// speak for, can neither grant nor withhold a discount (AD-044).
    #[test]
    fn exposure_scope_reads_only_confirmed_installs_in_the_named_projects() {
        use crate::osv::types::MatchedDependency;
        let install = |project: &str, direct: bool, dev: bool, confirmed: bool| MatchedDependency {
            project_path: project.to_string(),
            installed_version: Some("3.1.2".to_string()),
            is_direct: direct,
            is_dev: dev,
            is_version_confirmed: confirmed,
        };
        let mut sandbox = adv("GHSA-s", "sandbox", None, "s", &[], Some(9.8), None);
        sandbox.dependency_instances = vec![
            install("/webhook", false, true, true),
            install("/webhook", true, false, false),
            install("/other", true, false, true),
        ];

        let webhook = ExposureScope::of(&[&sandbox], &["/webhook".to_string()]);
        assert_eq!(webhook.all_transitive(), Some(true));
        assert_eq!(webhook.all_dev(), Some(true));

        let both = ExposureScope::of(&[&sandbox], &["/webhook".to_string(), "/other".to_string()]);
        assert_eq!(both.all_transitive(), Some(false));
        assert_eq!(both.all_dev(), Some(false));

        let nowhere = ExposureScope::of(&[&sandbox], &["/nowhere".to_string()]);
        assert_eq!((nowhere.all_transitive(), nowhere.all_dev()), (None, None));
    }
}
