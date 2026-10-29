// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Brief cadence: what counts as a CHANGE in the facts, and the daily cap.
//!
//! Audit 2026-10-07. Local day 2026-10-06 (UTC+10) produced four auto briefs
//! (ids 395-398) against a dogfood target of at most three. Brief 396 said
//! "All facts are UNCHANGED today" and was still regenerated, because the
//! facts fingerprint moved on things that are not news:
//!
//! - an upgrade fact hashed the exact announced version, so @ai-sdk/openai
//!   4.0.84 -> 4.0.85 (a patch inside the breaking line already reported)
//!   rewrote the brief AND re-dated the fact as NEW;
//! - a lower-severity advisory hashed its fix path (`Refresh { to }`, the
//!   parent version), which moves whenever upstream publishes.
//!
//! An upgrade's identity is now its release line (the major at 1.x and
//! above, `0.minor` below 1.0, Cargo's and npm's compatibility unit) plus the
//! yanked flag. A lower-severity advisory's identity is its package key and
//! its sorted advisory ids. On top of that, after [`DAILY_AUTO_BRIEF_CAP`]
//! briefs in the local day an AUTO trigger reuses the latest brief unless a
//! NEW act-now fact (a HIGH or CRITICAL advisory) appears.

use std::collections::BTreeSet;

use crate::brief_facts::{BriefFacts, UpgradeFact};
use crate::scoring::release_version::lenient_semver;

/// Briefs in one local day after which an auto trigger only regenerates for a
/// new act-now fact. AD-050's dogfood target is "at most three a day".
pub(crate) const DAILY_AUTO_BRIEF_CAP: i64 = 3;

/// The compatibility line a release belongs to: "4" for 4.0.85, "0.8" for
/// 0.8.2, "0.0.3" for 0.0.3 (every 0.0.x patch breaks). An unparseable
/// version is its own line.
pub(crate) fn release_line(version: &str) -> String {
    match lenient_semver(version, None) {
        Some(v) if v.major > 0 => v.major.to_string(),
        Some(v) if v.minor > 0 => format!("0.{}", v.minor),
        Some(v) => format!("0.0.{}", v.patch),
        None => version.trim().to_string(),
    }
}

/// What, if it changed, makes an upgrade fact news again: a new breaking line,
/// or the release being yanked. Never a patch inside the same line.
pub(crate) fn upgrade_signature(u: &UpgradeFact) -> String {
    upgrade_signature_of(&u.announced, u.yanked)
}

pub(crate) fn upgrade_signature_of(announced: &str, yanked: bool) -> String {
    let line = release_line(announced);
    if yanked {
        format!("{line}+yanked")
    } else {
        line
    }
}

/// Is a recorded novelty signature the same state as the current one?
///
/// Exact equality, plus the two signature shapes written before 2026-10-07,
/// so that shipping this change does not re-date every open fact as NEW:
/// - an upgrade recorded as its exact version ("4.0.84") is the same state as
///   its line ("4") — never across `|`, which only security signatures use;
/// - a lower-severity advisory recorded as "Medium|ids|site=..->fix" is the
///   same state as "Medium|ids" (exactly one `|`: the current low-tier shape).
pub(crate) fn same_state(recorded: &str, current: &str) -> bool {
    if recorded == current {
        return true;
    }
    let legacy_upgrade = !recorded.contains('|')
        && !current.contains('|')
        && !current.contains("+yanked")
        && recorded.starts_with(&format!("{current}."));
    let legacy_low_tier =
        current.matches('|').count() == 1 && recorded.starts_with(&format!("{current}|"));
    legacy_upgrade || legacy_low_tier
}

/// The act-now identities a brief carries: one per (fact, advisory). A fact
/// with no advisory ids (install drift) is identified by its key alone.
pub(crate) fn act_now_ids(facts: &BriefFacts) -> Vec<String> {
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for f in &facts.security {
        if f.advisory_ids.is_empty() {
            ids.insert(f.key.clone());
        }
        for a in &f.advisory_ids {
            ids.insert(format!("{}#{a}", f.key));
        }
    }
    ids.into_iter().collect()
}

/// Does `current` add an act-now identity the latest brief did not carry?
pub(crate) fn has_new_act_now(recorded: &[String], current: &[String]) -> bool {
    let known: BTreeSet<&str> = recorded.iter().map(String::as_str).collect();
    current.iter().any(|c| !known.contains(c.as_str()))
}

/// Briefs written per local day, by trigger. Launch gate G4 ("cap
/// respected") reads the auto count against [`DAILY_AUTO_BRIEF_CAP`]: the
/// `briefings` table cannot tell an auto brief from a manual Regenerate,
/// which is the user's own and never counts toward the cap.
pub(crate) const BRIEF_WRITES_KV_KEY: &str = "brief_writes_by_day_v1";
const BRIEF_WRITES_RETENTION_DAYS: i64 = 60;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct DayWrites {
    #[serde(default)]
    pub auto: u32,
    #[serde(default)]
    pub manual: u32,
}

/// Count one written brief for today (local).
pub(crate) fn note_brief_written(db: &crate::db::Database, auto: bool) {
    note_brief_written_on(db, &crate::brief_facts::local_today(), auto);
}

pub(crate) fn note_brief_written_on(db: &crate::db::Database, day: &str, auto: bool) {
    let mut all = load_brief_writes(db);
    let entry = all.entry(day.to_string()).or_default();
    if auto {
        entry.auto += 1;
    } else {
        entry.manual += 1;
    }
    let cutoff = (chrono::Local::now() - chrono::Duration::days(BRIEF_WRITES_RETENTION_DAYS))
        .format("%Y-%m-%d")
        .to_string();
    all.retain(|d, _| d.as_str() >= cutoff.as_str() || d.as_str() == day);
    match serde_json::to_string(&all) {
        Ok(json) => {
            if let Err(e) = db.set_kv(BRIEF_WRITES_KV_KEY, &json) {
                tracing::warn!(target: "4da::briefing", error = %e, "brief write count not persisted");
            }
        }
        Err(e) => {
            tracing::warn!(target: "4da::briefing", error = %e, "brief write count not serialized");
        }
    }
}

pub(crate) fn load_brief_writes(
    db: &crate::db::Database,
) -> std::collections::BTreeMap<String, DayWrites> {
    db.get_kv(BRIEF_WRITES_KV_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_writes_are_counted_per_day_and_trigger() {
        let db = crate::test_utils::test_db();
        note_brief_written_on(&db, "2026-10-13", true);
        note_brief_written_on(&db, "2026-10-13", true);
        note_brief_written_on(&db, "2026-10-13", false);
        note_brief_written_on(&db, "2026-10-14", true);
        let all = load_brief_writes(&db);
        assert_eq!(all["2026-10-13"], DayWrites { auto: 2, manual: 1 });
        assert_eq!(all["2026-10-14"], DayWrites { auto: 1, manual: 0 });
        let raw = db.get_kv(BRIEF_WRITES_KV_KEY).unwrap().unwrap();
        assert!(
            raw.contains("\"2026-10-13\":{\"auto\":2,\"manual\":1}"),
            "{raw}"
        );
    }

    #[test]
    fn release_lines_follow_the_compatibility_unit() {
        assert_eq!(release_line("4.0.84"), "4");
        assert_eq!(release_line("4.0.85"), "4");
        assert_eq!(release_line("5.0.0"), "5");
        assert_eq!(release_line("0.8.1"), "0.8");
        assert_eq!(release_line("0.8.2"), "0.8");
        assert_eq!(release_line("0.9.0"), "0.9");
        assert_eq!(release_line("0.0.3"), "0.0.3");
    }

    #[test]
    fn yanking_a_release_is_a_new_state() {
        assert_ne!(
            upgrade_signature_of("4.0.85", true),
            upgrade_signature_of("4.0.85", false)
        );
    }

    #[test]
    fn legacy_signatures_are_the_same_state() {
        assert!(
            same_state("4.0.84", "4"),
            "old exact-version upgrade record"
        );
        assert!(same_state("0.8.1", "0.8"));
        assert!(!same_state("0.81.0", "0.8"), "a different 0.x line");
        assert!(!same_state("40.1.0", "4"));
        assert!(!same_state("4.0.84", "4+yanked"), "yanking is news");
        assert!(same_state(
            "Medium|GHSA-a|navcal=1.0.0->Refresh { to: \"1.1.21\" }",
            "Medium|GHSA-a"
        ));
        assert!(!same_state("Medium|GHSA-a,GHSA-b|x", "Medium|GHSA-a"));
        assert!(
            !same_state("High|GHSA-a|s=1->Bump|t=2->Bump", "High|GHSA-a|s=1->Bump"),
            "a High signature never matches by prefix"
        );
    }

    #[test]
    fn a_new_act_now_advisory_is_detected() {
        let before = vec!["crates.io:rmcp:x#GHSA-a".to_string()];
        assert!(!has_new_act_now(&before, &before));
        assert!(!has_new_act_now(&before, &[]), "a fixed fact is not new");
        assert!(has_new_act_now(
            &before,
            &[
                "crates.io:rmcp:x#GHSA-a".to_string(),
                "crates.io:rmcp:x#GHSA-b".to_string()
            ]
        ));
    }
}
