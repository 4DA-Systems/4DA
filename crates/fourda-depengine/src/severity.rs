// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The severity of an advisory its source never graded, read from its GHSA
//! aliases. One rule for both engines: the app grades from the local mirror,
//! `@4da/mcp-server` (`gradeFromAliases` in `src/live/osv-scanner.ts`) from the
//! records its source returns. `vectors/severity.jsonl` pins it.
//!
//! An unrated record (no `database_specific.severity` label and no scorable
//! CVSS) takes the HIGHEST grade among ALL of its GHSA aliases that can be
//! read. The alias that gave it is named: the lexicographically smallest id on
//! a tie. An alias that cannot be read, or reads ungraded, never lowers the
//! grade. When none can be read the record stays unknown.
//!
//! Umbrella records are why it must be every alias: RUSTSEC-2020-0014
//! (rusqlite) lists 8 GHSA ids, seven of them critical and one high, so taking
//! the first alias made the grade depend on the order OSV lists them in.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

/// The four tiers, most severe first rank highest.
#[must_use]
pub fn tier_rank(tier: &str) -> u8 {
    match tier {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

/// A grade read from an alias, and the alias it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasGrade {
    /// `critical` / `high` / `medium` / `low`.
    pub severity: String,
    /// The GHSA id the grade was read from.
    pub from: String,
}

/// The GHSA ids among `aliases` other than the record's own id, deduplicated
/// (ASCII case-insensitively) and sorted. These are the ids the rule consults.
#[must_use]
pub fn ghsa_aliases(record_id: &str, aliases: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out: Vec<String> = aliases
        .iter()
        .map(|a| a.trim())
        .filter(|a| a.len() > 5 && a[..5].eq_ignore_ascii_case("GHSA-"))
        .filter(|a| !a.eq_ignore_ascii_case(record_id.trim()))
        .filter(|a| seen.insert(a.to_ascii_uppercase()))
        .map(str::to_string)
        .collect();
    out.sort();
    out
}

/// The rule. `read(id)` answers the grade of one GHSA id: `None` when the
/// record cannot be read or carries no grade. `None` overall when no alias
/// gives a grade: the record stays unknown.
pub fn grade_from_aliases<'a>(
    record_id: &str,
    aliases: &[String],
    read: impl Fn(&str) -> Option<&'a str>,
) -> Option<AliasGrade> {
    let mut best: Option<AliasGrade> = None;
    // Ascending ids, and only a strictly higher grade replaces the best one,
    // so a tie names the smallest id.
    for id in ghsa_aliases(record_id, aliases) {
        let Some(grade) = read(&id).filter(|g| tier_rank(g) > 0) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|b| tier_rank(grade) > tier_rank(&b.severity))
        {
            best = Some(AliasGrade {
                severity: grade.to_string(),
                from: id,
            });
        }
    }
    best
}

static MAINTENANCE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(?i)\b(unmaintained|no longer maintained)\b").ok());

/// An unmaintained-package notice, not a vulnerability (the MCP's
/// `isMaintenanceNotice`): such a record is never graded from an alias.
#[must_use]
pub fn is_maintenance_notice(summary: &str) -> bool {
    MAINTENANCE.as_ref().is_some_and(|re| re.is_match(summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn highest_alias_wins_and_a_tie_names_the_smallest_id() {
        let read = |id: &str| match id {
            "GHSA-b" | "GHSA-c" => Some("critical"),
            "GHSA-a" => Some("high"),
            _ => None,
        };
        let got = grade_from_aliases(
            "RUSTSEC-1",
            &ids(&["GHSA-c", "CVE-1", "GHSA-a", "GHSA-b"]),
            read,
        );
        assert_eq!(
            got,
            Some(AliasGrade {
                severity: "critical".into(),
                from: "GHSA-b".into()
            })
        );
    }

    #[test]
    fn unreadable_and_ungraded_aliases_never_lower_the_grade() {
        let read = |id: &str| match id {
            "GHSA-z" => Some("medium"),
            "GHSA-y" => Some("unknown"),
            _ => None,
        };
        let got = grade_from_aliases("GO-1", &ids(&["GHSA-x", "GHSA-y", "GHSA-z"]), read);
        assert_eq!(
            got.map(|g| (g.severity, g.from)),
            Some(("medium".into(), "GHSA-z".into()))
        );
        assert_eq!(
            grade_from_aliases("GO-1", &ids(&["GHSA-x"]), |_| None),
            None
        );
    }

    #[test]
    fn the_record_itself_and_non_ghsa_ids_are_not_aliases() {
        assert_eq!(
            ghsa_aliases(
                "GHSA-self",
                &ids(&["ghsa-SELF", "CVE-2", "GHSA-q", "ghsa-Q", "MAL-1"])
            ),
            ids(&["GHSA-q"])
        );
    }

    #[test]
    fn maintenance_notices_are_recognised() {
        assert!(is_maintenance_notice("`instant` is unmaintained"));
        assert!(is_maintenance_notice("This crate is No Longer Maintained"));
        assert!(!is_maintenance_notice("Use-after-free in unmaintainedness"));
    }
}
