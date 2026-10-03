// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ONE security truth for the scorer (2026-10-02 adversarial audit, item 3).
//!
//! A registry advisory item (cve / osv source) is judged by the SAME
//! version-confirmed matcher Preemption uses
//! ([`crate::osv::advisory_match::exposure_for_advisory`]): every installed
//! copy of the advisory's package — direct, transitive and dev, every project
//! in the auditable inventory — against the mirror's structured ranges.
//!
//! The live failures this replaces:
//! - **False critical** — `[CVE-2026-93981] hono/jsx …` (GHSA-hxh3-vqpv-xpqv,
//!   fixed 4.13.7, MEDIUM 4.7) paged as "Critical: Security issue affects
//!   hono" against an installed 4.13.9. The item was scored 12 minutes after
//!   publication, before the OSV mirror held the row, and the text fallback
//!   read the `Affected: hono (npm)` NAME line as the range (the `Affected
//!   range:` line the cve source writes was never parsed), so the verdict was
//!   unknown and the ungraded default said Critical.
//! - **Missed true positive** — GHSA-9pj6-vhgr-3mwh (rmcp, HIGH 7.5) was
//!   judged only against the projects that DECLARE rmcp (victauri, 3.1.2 —
//!   fixed); a sibling bridge app's transitive 1.7.0 was never read, the verdict said
//!   "not affected" and the item sat at 0.37 / watch while Preemption listed
//!   it as HIGH.

use crate::db::Database;
use crate::osv::advisory_match::{exposure_for_advisory, ExposedCopy};
use crate::osv::types::StoredAdvisory;

/// The matcher's verdict for the advisory an item is about.
#[derive(Debug, Clone, Default)]
pub(crate) struct SecurityVerdict {
    /// `Some(true)` a copy is confirmed inside the range; `Some(false)` every
    /// evaluated copy is confirmed outside it; `None` undecidable copies only.
    pub affected: Option<bool>,
    /// Copies confirmed affected, worst first (runtime before dev, direct
    /// before transitive), across every mirror row for the advisory.
    pub exposed: Vec<ExposedCopy>,
    /// The advisory's own severity tier (`critical` / `high` / `medium` /
    /// `low`) from the deciding mirror row — CVSS band first, curated label
    /// otherwise. `None` when the mirror grades it neither way.
    pub tier: Option<&'static str>,
    /// The fix for the exposed copy's package (highest listed fix).
    pub fixed_version: Option<String>,
}

impl SecurityVerdict {
    /// The worst exposed copy — what the priority and the "installed"
    /// evidence are about.
    pub(crate) fn worst_copy(&self) -> Option<&ExposedCopy> {
        self.exposed.first()
    }

    /// Distinct exposed projects, sorted.
    pub(crate) fn exposed_projects(&self) -> Vec<String> {
        let mut paths: Vec<String> = self
            .exposed
            .iter()
            .map(|c| c.project_path.clone())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    /// The dependency path label of the worst exposed copy.
    pub(crate) fn path_label(&self) -> Option<&'static str> {
        self.worst_copy().map(|c| {
            if c.is_dev {
                "dev-only"
            } else if c.is_direct {
                "direct"
            } else {
                "transitive"
            }
        })
    }
}

/// Everything the Signal classifier needs to set a security_alert's tier from
/// the ONE security truth. Built once per item in `score_item`.
#[derive(Debug, Clone, Default)]
pub(crate) struct SecurityLane<'a> {
    /// The item is a registry advisory row (cve / osv), not editorial
    /// coverage of one.
    pub registry_advisory: bool,
    /// The version verdict (`is_version_affected`).
    pub affected: Option<bool>,
    /// The worst copy the matcher confirmed inside the range.
    pub exposed: Option<&'a ExposedCopy>,
    /// Every project holding a confirmed-affected copy.
    pub exposed_projects: Vec<String>,
    /// The advisory's own severity tier.
    pub tier: Option<&'static str>,
    /// The package the advisory names (for the action line).
    pub package: Option<&'a str>,
}

impl SecurityLane<'_> {
    /// The tier of a dependency-grounded security_alert. `best` is the
    /// strongest grounded edge — the reach of a text-route-confirmed advisory
    /// when the mirror did not hold it.
    pub(crate) fn priority(
        &self,
        best: Option<&super::dependencies::DepMatch>,
    ) -> crate::signals::SignalPriority {
        use crate::signals::{confirmed_advisory_priority, SignalPriority};
        if !self.registry_advisory {
            return SignalPriority::Advisory;
        }
        match (self.affected, self.exposed) {
            (Some(false), _) => SignalPriority::Watch,
            (_, Some(copy)) => confirmed_advisory_priority(self.tier, copy.is_direct, copy.is_dev),
            (Some(true), None) => match best {
                Some(dep) => confirmed_advisory_priority(self.tier, dep.is_direct, dep.is_dev),
                None => SignalPriority::Advisory,
            },
            (None, None) => SignalPriority::Advisory,
        }
    }

    /// Upper bound for ANY security_alert's final tier: Alert/Critical need a
    /// version-confirmed affected registry advisory; a confirmed
    /// not-affected one is Watch; everything else is Advisory at most.
    pub(crate) fn cap(&self) -> crate::signals::SignalPriority {
        use crate::signals::SignalPriority;
        match (self.registry_advisory, self.affected) {
            (true, Some(true)) => SignalPriority::Critical,
            (true, Some(false)) => SignalPriority::Watch,
            _ => SignalPriority::Advisory,
        }
    }
}

/// Parse a persisted priority label back into its tier.
pub(crate) fn priority_from_label(label: &str) -> Option<crate::signals::SignalPriority> {
    use crate::signals::SignalPriority;
    [
        SignalPriority::Watch,
        SignalPriority::Advisory,
        SignalPriority::Alert,
        SignalPriority::Critical,
    ]
    .into_iter()
    .find(|p| p.label() == label)
}

/// Action line for a matcher-confirmed advisory: names the package and the
/// EXPOSED project(s), never a declaring project that runs the fix.
pub(crate) fn exposed_security_action(
    package: &str,
    exposed_projects: &[String],
    prefix: &str,
) -> String {
    match super::dependencies::project_label(exposed_projects) {
        Some(location) => format!("{prefix}: Security issue affects {package} in {location}"),
        None => format!("{prefix}: Security issue affects your dependency {package}"),
    }
}

/// The severity tier of one mirror row.
pub(crate) fn advisory_tier(advisory: &StoredAdvisory) -> Option<&'static str> {
    if let Some(score) = advisory.cvss_score.filter(|s| *s > 0.0) {
        return Some(crate::osv::types::cvss_band(score));
    }
    normalize_tier(advisory.severity_label.as_deref()?)
}

fn normalize_tier(label: &str) -> Option<&'static str> {
    match label.trim().to_ascii_lowercase().as_str() {
        "critical" => Some("critical"),
        "high" => Some("high"),
        "medium" | "moderate" => Some("medium"),
        "low" => Some("low"),
        _ => None,
    }
}

fn tier_rank(tier: Option<&str>) -> u8 {
    match tier {
        Some("critical") => 4,
        Some("high") => 3,
        Some("medium") => 2,
        Some("low") => 1,
        _ => 0,
    }
}

/// The severity tier an advisory item carries in its OWN text: the numeric
/// `CVSS:` line (banded), else the curated `Severity:` label (GitHub writes
/// `MODERATE` for medium). The cve source writes both; this is the fallback
/// when the mirror does not hold the advisory yet — severity must come from
/// the advisory, never a default.
pub(crate) fn content_severity_tier(content: &str) -> Option<&'static str> {
    let mut label: Option<&'static str> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("CVSS:") {
            if let Ok(score) = rest.trim().parse::<f64>() {
                if score > 0.0 && score <= 10.0 {
                    return Some(crate::osv::types::cvss_band(score));
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("Severity:") {
            // `Severity: CVSS_V3: 9.8` (osv source) carries a score, not a label.
            let rest = rest.trim();
            if let Some((_, score)) = rest.split_once(':') {
                if let Some(score) = super::cvss::parse_cvss_score(score.trim()) {
                    if score > 0.0 && score <= 10.0 {
                        return Some(crate::osv::types::cvss_band(score));
                    }
                }
            } else if label.is_none() {
                label = normalize_tier(rest);
            }
        }
    }
    label
}

/// The `Affected range:` entries the cve source writes for ONE package —
/// `Affected range: hono (npm): < 4.13.7; other (rust): >= 1.0.0, < 1.2.0`.
/// GitHub lists one entry per release line, so a package can carry several.
/// Package names compare registry-equal (`-`/`_`, case). Empty when the item
/// carries no range for this package.
pub(crate) fn affected_ranges_for_package(content: &str, package: &str) -> Vec<String> {
    let Some(line) = content
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("Affected range:"))
    else {
        return Vec::new();
    };
    line.split(';')
        .filter_map(|entry| {
            let (head, range) = entry.split_once("):")?;
            let name = head.rsplit_once('(').map_or(head, |(n, _)| n).trim();
            let range = range.trim();
            (!range.is_empty() && crate::dep_linker::registry_names_equal(name, package))
                .then(|| range.to_string())
        })
        .collect()
}

/// Text-route verdict from a package's own range entries: affected when the
/// installed version is inside ANY of them. `None` when the version or any
/// range does not parse — a half-read list would silently drop the branch
/// that decides.
pub(crate) fn ranges_verdict(installed: Option<&str>, ranges: &[String]) -> Option<bool> {
    if ranges.is_empty() {
        return None;
    }
    let version = semver::Version::parse(installed?.trim().trim_start_matches('v')).ok()?;
    let reqs: Option<Vec<semver::VersionReq>> = ranges
        .iter()
        .map(|r| semver::VersionReq::parse(r).ok())
        .collect();
    Some(reqs?.iter().any(|req| req.matches(&version)))
}

fn highest_fix(advisory: &StoredAdvisory) -> Option<String> {
    let versions: Vec<String> = serde_json::from_str(advisory.fixed_versions.as_deref()?).ok()?;
    versions
        .into_iter()
        .filter_map(|v| super::release_version::lenient_semver(&v, None).map(|parsed| (parsed, v)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, v)| v)
}

/// Run the Preemption matcher for every mirror row of the advisory the item
/// is about (`ids`: its title id and URL GHSA id; aliases count). `None` when
/// the mirror holds no row, or no row's package is in the user's inventory —
/// the caller's fallback routes then decide.
pub(crate) fn matcher_verdict(db: &Database, ids: &[String]) -> Option<SecurityVerdict> {
    let mut rows: Vec<StoredAdvisory> = Vec::new();
    for id in ids {
        for row in db.get_osv_advisories_by_id(id).unwrap_or_default() {
            if !rows.iter().any(|r| r.id == row.id) {
                rows.push(row);
            }
        }
    }
    let mut checked = 0usize;
    let mut undecided = 0usize;
    let mut verdict = SecurityVerdict::default();
    let mut deciding_tier: Option<&'static str> = None;
    let mut any_tier: Option<&'static str> = None;
    for row in &rows {
        let exposure = exposure_for_advisory(db, row);
        if exposure.copies_checked == 0 {
            continue;
        }
        checked += exposure.copies_checked;
        undecided += exposure.unconfirmed;
        let tier = advisory_tier(row);
        if tier_rank(tier) > tier_rank(any_tier) {
            any_tier = tier;
        }
        if !exposure.confirmed.is_empty() {
            if tier_rank(tier) > tier_rank(deciding_tier) {
                deciding_tier = tier;
            }
            if verdict.fixed_version.is_none() {
                verdict.fixed_version = highest_fix(row);
            }
            verdict.exposed.extend(exposure.confirmed);
        }
    }
    if checked == 0 {
        return None;
    }
    verdict.exposed.sort_by(|a, b| {
        a.is_dev
            .cmp(&b.is_dev)
            .then_with(|| b.is_direct.cmp(&a.is_direct))
            .then_with(|| a.project_path.cmp(&b.project_path))
    });
    verdict.exposed.dedup();
    verdict.affected = if !verdict.exposed.is_empty() {
        Some(true)
    } else if undecided == 0 {
        Some(false)
    } else {
        None
    };
    verdict.tier = if verdict.exposed.is_empty() {
        any_tier
    } else {
        deciding_tier.or(any_tier)
    };
    Some(verdict)
}

/// An advisory-registry row (cve / osv) IS a security alert, whatever words
/// its title happens to use. The keyword classifier reads vocabulary, so an
/// advisory without security keywords was typed by its other words or not at
/// all (live 2026-10-03: "[GHSA-c9xm-49cp-xcr9] rmcp OAuth client fetches
/// server-controlled resource_metadata URLs" read as an "Emerging trend";
/// "[RUSTSEC-2026-0190] anyhow: Unsoundness in `Error::downcast_mut()`" got no
/// signal at all) and sat outside the Security lane. The tier is still the
/// caller's: the version-confirmed priority path runs on `SecurityAlert`.
/// Editorial items pass through untouched.
pub(crate) fn advisory_signal_type(
    is_registry_advisory: bool,
    title: &str,
    classified: Option<crate::signals::SignalClassification>,
) -> Option<crate::signals::SignalClassification> {
    use crate::signals::{SignalClassification, SignalHorizon, SignalPriority, SignalType};
    if !is_registry_advisory {
        return classified;
    }
    Some(match classified {
        Some(mut c) => {
            c.signal_type = SignalType::SecurityAlert;
            c.horizon = SignalHorizon::Tactical;
            c
        }
        None => SignalClassification {
            signal_type: SignalType::SecurityAlert,
            priority: SignalPriority::Advisory,
            confidence: 0.5,
            action: format!("Security advisory: {title}"),
            triggers: Vec::new(),
            horizon: SignalHorizon::Tactical,
            dependency_confirmed: false,
            corroboration_sources: 0,
        },
    })
}

#[cfg(test)]
#[path = "security_verdict_tests.rs"]
mod tests;
