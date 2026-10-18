// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Whether a newly announced registry release is itself clear of known
//! advisories, before any surface announces it or names a command toward it.
//!
//! The release grade (`scoring::release_grade`) answers which projects a
//! release concerns and what kind of move it is; it never asked whether the
//! release is safe to move to, because security "reaches the user through the
//! advisory lanes". Those lanes only match advisories against the copies a
//! project already RUNS. A malicious minor (an OSV `MAL-` record, or a GitHub
//! malware advisory) published inside a project's declared range was
//! therefore announced with a ready-to-run `npm update`: the one command that
//! installs it.
//!
//! The check uses the predicate every other "this version is clear" answer
//! uses (`osv::parent_hint`'s `is_clear`): no stored, non-withdrawn advisory
//! for the package places the version inside its affected window, where a
//! window the matcher cannot evaluate counts as affected
//! (`check_version_affected`'s conservative answer). A release the registry
//! withdrew (`osv::release_facts`, FX-H5) is never a target either.
//!
//! "No advisory affects it" is only a claim when the advisory data was read
//! after the release existed: the mirror for the ecosystem must have a
//! successful sync inside the trust horizon (`osv::readiness`, the rule
//! behind Preemption's all-clear). Otherwise the status is unknown, and
//! unknown is never presented as clear.
//!
//! A clean release is also HELD for [`RELEASE_HOLD_DAYS`] after it was
//! published: malware advisories trail a compromised publish by hours to
//! days, so "no advisory yet" minutes after a publish says nothing. The hold
//! is the one this repository applies to itself (`pnpm-workspace.yaml`
//! `minimumReleaseAge`) and the MCP server's `dependency_check` applies
//! (`WAIT_DAYS`). Since the hold is never shorter than the trust horizon, a
//! release past it always predates the ready sync it was checked against. A
//! release the registry did not date cannot be aged, so it is unknown, never
//! clear.

use chrono::NaiveDateTime;

use crate::db::Database;
use fourda_depengine::registry::RegistryIndex;

use super::matching::{check_version_affected, package_key};
use super::readiness::{self, AdvisorySyncState};
use super::types::SyncStatus;

/// Days a release must have been public before 4DA names a command toward
/// it (the hold `pnpm-workspace.yaml`'s `minimumReleaseAge` applies here).
pub(crate) const RELEASE_HOLD_DAYS: i64 = 3;

/// One stored advisory, as the release check reads it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AdvisoryWindow {
    pub advisory_id: String,
    pub summary: String,
    pub aliases: Vec<String>,
    /// The stored OSV ranges JSON (`osv_advisories.affected_ranges`).
    pub affected_ranges: Option<String>,
}

impl AdvisoryWindow {
    /// A malicious-package record: an OSV `MAL-` id (or alias), or a GitHub
    /// malware advisory, whose summary OSV writes as "Malicious code in …".
    pub(crate) fn is_malware(&self) -> bool {
        let mal = |id: &str| id.trim().to_ascii_uppercase().starts_with("MAL-");
        mal(&self.advisory_id)
            || self.aliases.iter().any(|a| mal(a))
            || self
                .summary
                .trim()
                .to_ascii_lowercase()
                .starts_with("malicious")
    }

    /// (inside the window, placed by a readable bound) for `version`.
    fn affects(&self, ecosystem: &str, version: &str) -> (bool, bool) {
        check_version_affected(ecosystem, Some(version), &self.affected_ranges)
    }
}

/// The advisories that affect a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AffectedRelease {
    /// Advisory ids, malware records first, then as stored (worst CVSS first).
    pub ids: Vec<String>,
    /// At least one of them is a malicious-package record.
    pub malware: bool,
    /// At least one places the release inside a window it could read; when
    /// false every match is the conservative "could not place it" answer.
    pub confirmed: bool,
    /// Every one of them also affects (version-confirmed) EVERY copy the
    /// active concerned projects run, so the advisory lane already reports
    /// each of them there, with a target that clears it. False when no
    /// active copy was supplied.
    pub covered_by_installed: bool,
}

/// Why a release's advisory status is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UnknownReason {
    /// The ecosystem's advisories were never downloaded.
    NeverSynced,
    /// The first download is running now.
    Syncing,
    /// The latest download failed.
    Failed,
    /// The stored advisory data could not be read.
    Unreadable,
    /// The registry gave no publish date, so the release cannot be aged.
    Undated,
    /// The last successful download is past the trust horizon.
    Stale { synced_at: Option<String> },
}

impl UnknownReason {
    /// The reason, worded to follow "because".
    pub(crate) fn words(&self) -> String {
        match self {
            Self::NeverSynced => "vulnerability advisories have not been downloaded yet".into(),
            Self::Syncing => "vulnerability advisories are still downloading".into(),
            Self::Failed => "the last vulnerability-advisory download failed".into(),
            Self::Unreadable => "the stored advisory data could not be read".into(),
            Self::Undated => {
                "the registry gave no publish date, so 4DA cannot tell how new it is".into()
            }
            Self::Stale {
                synced_at: Some(at),
            } => {
                format!("the advisory data was last downloaded {at} UTC, more than three days ago")
            }
            Self::Stale { synced_at: None } => {
                "the advisory data is more than three days old".into()
            }
        }
    }
}

/// What the stored advisories say about one announced release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReleaseAdvisories {
    /// Checked against advisory data newer than the release: none affects it.
    Clear,
    /// One or more advisories affect the release.
    Affected(AffectedRelease),
    /// No advisory affects it, but the registry withdrew it (yanked, or
    /// deprecated with a clean later release on its line).
    Withdrawn,
    /// No advisory on record affects it, but the record cannot be trusted.
    Unknown(UnknownReason),
    /// Checked and clean, but published fewer than [`RELEASE_HOLD_DAYS`]
    /// days ago: too new for a malware report to exist yet.
    Held {
        /// The registry's publish date, `YYYY-MM-DD`.
        published: String,
        /// Whole days since it was published.
        days: i64,
    },
}

impl ReleaseAdvisories {
    /// Only a clear release may be the target of a command.
    pub(crate) fn is_clear(&self) -> bool {
        matches!(self, Self::Clear)
    }
}

/// A `YYYY-MM-DD[ HH:MM:SS]` / RFC 3339 timestamp as naive UTC.
fn parse_utc(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(t.naive_utc());
    }
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
    ] {
        if let Ok(t) = NaiveDateTime::parse_from_str(s, format) {
            return Some(t);
        }
    }
    // A bare date is read as its LAST second: a release dated today must not
    // age (or predate a sync) from midnight.
    chrono::NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(23, 59, 59))
}

/// The advisories that affect `release`, as one verdict. Pure.
fn affected(
    ecosystem: &str,
    release: &str,
    installed: &[String],
    advisories: &[AdvisoryWindow],
) -> Option<AffectedRelease> {
    let mut hits: Vec<(&AdvisoryWindow, bool)> = advisories
        .iter()
        .filter_map(|a| {
            let (inside, confirmed) = a.affects(ecosystem, release);
            inside.then_some((a, confirmed))
        })
        .collect();
    if hits.is_empty() {
        return None;
    }
    hits.sort_by_key(|(a, _)| !a.is_malware());
    let mut ids: Vec<String> = Vec::new();
    for (a, _) in &hits {
        if !ids.contains(&a.advisory_id) {
            ids.push(a.advisory_id.clone());
        }
    }
    Some(AffectedRelease {
        malware: hits.iter().any(|(a, _)| a.is_malware()),
        confirmed: hits.iter().any(|(_, c)| *c),
        covered_by_installed: !installed.is_empty()
            && hits.iter().all(|(a, _)| {
                installed
                    .iter()
                    .all(|v| a.affects(ecosystem, v) == (true, true))
            }),
        ids,
    })
}

/// The verdict for `release` of one package. Pure: the caller supplies the
/// package's stored advisories, whether its registry withdrew the release,
/// the ecosystem's sync rows, `now`, whether a sync is running, and the
/// registry's publish date for the release.
///
/// An affecting advisory is reported whatever the mirror's age (stale data
/// that names an advisory is still evidence against the release); only the
/// all-clear needs fresh data.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assess(
    ecosystem: &str,
    release: &str,
    installed: &[String],
    advisories: &[AdvisoryWindow],
    withdrawn: bool,
    statuses: &[SyncStatus],
    now: NaiveDateTime,
    in_flight: bool,
    published: Option<&str>,
) -> ReleaseAdvisories {
    if let Some(hit) = affected(ecosystem, release, installed, advisories) {
        return ReleaseAdvisories::Affected(hit);
    }
    if withdrawn {
        return ReleaseAdvisories::Withdrawn;
    }
    let sync = readiness::classify(&[ecosystem.to_string()], statuses, now, in_flight);
    let reason = match sync.state {
        AdvisorySyncState::Ready | AdvisorySyncState::Matching => None,
        AdvisorySyncState::Syncing => Some(UnknownReason::Syncing),
        AdvisorySyncState::Failed => Some(UnknownReason::Failed),
        AdvisorySyncState::Stale => Some(UnknownReason::Stale {
            synced_at: sync.synced_at.clone(),
        }),
        AdvisorySyncState::NotStarted | AdvisorySyncState::NoInventory => {
            Some(UnknownReason::NeverSynced)
        }
    };
    if let Some(reason) = reason {
        return ReleaseAdvisories::Unknown(reason);
    }
    // A ready state always carries a sync time; without one, trust nothing.
    if sync.synced_at.as_deref().and_then(parse_utc).is_none() {
        return ReleaseAdvisories::Unknown(UnknownReason::NeverSynced);
    }
    let Some(released) = published.and_then(parse_utc) else {
        return ReleaseAdvisories::Unknown(UnknownReason::Undated);
    };
    let age = now.signed_duration_since(released);
    if age < chrono::Duration::days(RELEASE_HOLD_DAYS) {
        return ReleaseAdvisories::Held {
            published: released.format("%Y-%m-%d").to_string(),
            days: age.num_days().max(0),
        };
    }
    // Past the hold, a ready mirror's sync (younger than the trust horizon,
    // which the hold is never shorter than; pinned by a test) is newer than
    // the release, so the data checked was read after the release existed.
    ReleaseAdvisories::Clear
}

/// The name equality the matcher uses (`package_key`), plus crates.io's
/// `-`/`_` equivalence (the registry treats them as one crate).
fn same_package(ecosystem: &str, a: &str, b: &str) -> bool {
    let key = |n: &str| {
        let k = package_key(n, ecosystem);
        if ecosystem == "crates.io" {
            k.replace('_', "-")
        } else {
            k
        }
    };
    key(a) == key(b)
}

/// The once-per-pass reads behind [`ReleaseChecker::check`]: the sync rows
/// and the registry facts.
pub(crate) struct ReleaseChecker {
    statuses: Vec<SyncStatus>,
    registry: RegistryIndex,
    in_flight: bool,
}

impl ReleaseChecker {
    /// Read the sync rows and registry facts. An unreadable sync table
    /// leaves no rows, which reads as never synced, never as clear.
    pub(crate) fn load(db: &Database) -> Self {
        Self {
            statuses: db.get_osv_sync_statuses().unwrap_or_default(),
            registry: super::release_facts::load(db),
            in_flight: readiness::refresh::sync_in_flight(),
        }
    }

    /// The verdict for `release` of `package` (OSV-canonical `ecosystem`),
    /// given the versions the concerned projects run and the registry's
    /// publish date. Blocking; takes the database lock briefly.
    pub(crate) fn check(
        &self,
        db: &Database,
        ecosystem: &str,
        package: &str,
        release: &str,
        installed: &[String],
        published: Option<&str>,
    ) -> ReleaseAdvisories {
        let withdrawn = self
            .registry
            .get(&(package_key(package, ecosystem), ecosystem.to_string()))
            .is_some_and(|r| r.excludes(ecosystem, release));
        let advisories = match stored_advisories(db, ecosystem, package) {
            Some(a) => a,
            None => return ReleaseAdvisories::Unknown(UnknownReason::Unreadable),
        };
        assess(
            ecosystem,
            release,
            installed,
            &advisories,
            withdrawn,
            &self.statuses,
            chrono::Utc::now().naive_utc(),
            self.in_flight,
            published,
        )
    }

    /// The verdict for a graded registry release (`source_type` is the
    /// registry row's), against `installed`: the copies the ACTIVE concerned
    /// projects run (coverage by the advisory lane is judged on them alone).
    pub(crate) fn check_grade(
        &self,
        db: &Database,
        source_type: &str,
        grade: &crate::scoring::release_grade::ReleaseGrade,
        installed: &[String],
        published: Option<&str>,
    ) -> ReleaseAdvisories {
        let Some(ecosystem) = super::exposure::registry_source_ecosystem(source_type) else {
            return ReleaseAdvisories::Unknown(UnknownReason::NeverSynced);
        };
        self.check(
            db,
            ecosystem,
            &grade.package,
            &grade.announced.to_string(),
            installed,
            published,
        )
    }

    /// The Brief's rule: an upgrade fact never points at a release an
    /// advisory affects or its registry withdrew. The Brief names no command
    /// for an upgrade, and says itself when advisories are not checked, so an
    /// unchecked release is still reported there.
    pub(crate) fn advises_against(
        &self,
        db: &Database,
        source_type: &str,
        grade: &crate::scoring::release_grade::ReleaseGrade,
        published: Option<&str>,
    ) -> bool {
        matches!(
            self.check_grade(db, source_type, grade, &[], published),
            ReleaseAdvisories::Affected(_) | ReleaseAdvisories::Withdrawn
        )
    }
}

/// Every active stored advisory for `package` in `ecosystem`, matched by
/// [`same_package`]. `None` when the mirror cannot be read.
fn stored_advisories(db: &Database, ecosystem: &str, package: &str) -> Option<Vec<AdvisoryWindow>> {
    let folded = package.trim().to_lowercase().replace(['_', '.'], "-");
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare_cached(
            "SELECT advisory_id, summary, aliases, affected_ranges, package_name
             FROM osv_advisories
             WHERE ecosystem = ?1 AND withdrawn_at IS NULL
               AND REPLACE(REPLACE(LOWER(package_name), '_', '-'), '.', '-') = ?2
             ORDER BY cvss_score DESC NULLS LAST, advisory_id",
        )
        .ok()?;
    let rows: Vec<(AdvisoryWindow, String)> = stmt
        .query_map(rusqlite::params![ecosystem, folded], |r| {
            Ok((
                AdvisoryWindow {
                    advisory_id: r.get(0)?,
                    summary: r.get(1)?,
                    aliases: r
                        .get::<_, Option<String>>(2)?
                        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
                        .unwrap_or_default(),
                    affected_ranges: r.get(3)?,
                },
                r.get::<_, String>(4)?,
            ))
        })
        .ok()?
        // One undecodable row would otherwise vanish and could leave a false
        // all-clear: any row error makes the whole read unknown.
        .collect::<rusqlite::Result<_>>()
        .ok()?;
    Some(
        rows.into_iter()
            .filter(|(_, name)| same_package(ecosystem, name, package))
            .map(|(a, _)| a)
            .collect(),
    )
}

#[cfg(test)]
#[path = "release_advisories_tests.rs"]
mod tests;
