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

/// THE maintenance-notice predicate, the MCP's rule exactly
/// (`osv-ranges.ts` / `maintenance.ts`, canonical since the #1008 review): a
/// record is a notice when OSV's `database_specific.informational` is
/// `unmaintained` or `notice`, or its summary says `unmaintained` or `no
/// longer maintained` as words (case-insensitive). Nothing else: not
/// "deprecated" (it appears in plenty of real vulnerability summaries), and
/// a published fix does not stop a notice being one (ring's
/// RUSTSEC-2025-0010, "prior to 0.17 are unmaintained", fixed 0.17.0). A
/// notice never blocks a fix target, never is one, and is never graded as a
/// vulnerability. `informational` here is ONE block's flag; RustSec puts it
/// in `affected[].database_specific`, so a stored record is classified per
/// package by [`crate::osv::is_notice_for`], which calls this.
pub fn is_maintenance_notice(summary: &str, informational: Option<&str>) -> bool {
    if matches!(informational, Some("unmaintained" | "notice")) {
        return true;
    }
    let text = summary.to_ascii_lowercase();
    ["unmaintained", "no longer maintained"]
        .iter()
        .any(|word| has_word(&text, word))
}

/// `phrase` occurs in `text` with a word boundary on each side (regex `\b`:
/// the neighbours are not `[A-Za-z0-9_]`).
fn has_word(text: &str, phrase: &str) -> bool {
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(phrase).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + phrase.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

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
        // `get`, never a byte slice: alias text is untrusted, and a multi-byte
        // character across index 5 would end a slice mid-char and panic.
        .filter(|a| a.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("GHSA-")) && a.len() > 5)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notice_is_the_mcps_rule() {
        // ring RUSTSEC-2025-0010: a fix is published and it is still a notice.
        assert!(is_maintenance_notice(
            "Versions of *ring* prior to 0.17 are unmaintained.",
            None
        ));
        assert!(is_maintenance_notice("foo is no longer maintained", None));
        assert!(is_maintenance_notice("anything", Some("unmaintained")));
        assert!(is_maintenance_notice("anything", Some("notice")));
        assert!(!is_maintenance_notice("anything", Some("unsound")));
        // Words only, and never "deprecated" / "abandoned" / "archived".
        assert!(!is_maintenance_notice("UnmaintainedFoo crate", None));
        assert!(!is_maintenance_notice("foo has been deprecated", None));
        assert!(!is_maintenance_notice("Prototype pollution in bar", None));
    }

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
    fn a_multi_byte_alias_never_panics_and_is_not_a_ghsa_id() {
        // A 2-byte char across index 5, a 3-byte char starting at index 4, a
        // short multi-byte id, and one real GHSA id.
        let odd = ids(&["GHS\u{c5}-x", "GHSA\u{20ac}abc", "GH\u{20ac}", "GHSA-ok"]);
        assert_eq!(ghsa_aliases("R-1", &odd), ids(&["GHSA-ok"]));
        assert_eq!(
            grade_from_aliases("R-1", &odd, |id| (id == "GHSA-ok").then_some("high"))
                .map(|g| g.from),
            Some("GHSA-ok".to_string())
        );
    }
}
