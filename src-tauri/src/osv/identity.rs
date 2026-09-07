// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Vulnerability identity (Phase 120): which OSV rows are the SAME bug, and
//! the severity tier every surface shares for one such bug.
//!
//! Split out of `matching.rs` (file-size gate).

use std::collections::HashMap;

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
}
