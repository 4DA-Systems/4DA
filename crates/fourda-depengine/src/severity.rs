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

/// THE maintenance-notice predicate, both engines' rule (the MCP's
/// `osv-ranges.ts` / `maintenance.ts`). Structured data beats wording: when
/// OSV states an `informational` class for the record (`informational`, the
/// class that applies to the package, see [`crate::osv::is_notice_for`]),
/// that class alone decides: `unmaintained` / `notice` is a notice, `unsound`
/// or any other stated class is not, whatever the summary says
/// (RUSTSEC-2025-0068, serde_yml: class `unsound`, summary "serde_yml crate
/// is unsound and unmaintained", a vulnerability). Only when no class is
/// stated does the summary decide: it says `unmaintained` or `no longer
/// maintained` as words (case-insensitive). Nothing else: not "deprecated"
/// (it appears in plenty of real vulnerability summaries), and a published
/// fix does not stop a notice being one (ring's RUSTSEC-2025-0010, "prior to
/// 0.17 are unmaintained", fixed 0.17.0). A notice never blocks a fix target,
/// never is one, and is never graded as a vulnerability. RustSec puts the
/// class in `affected[].database_specific`, so a stored record is classified
/// per package by [`crate::osv::is_notice_for`], which calls this.
pub fn is_maintenance_notice(summary: &str, informational: Option<&str>) -> bool {
    if let Some(class) = informational {
        return matches!(class, "unmaintained" | "notice");
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

/// The tier of a CVSS base score, by the CVSS v3.1 qualitative rating scale
/// (§5; v4.0 keeps the same bands): critical from 9.0, high from 7.0, medium
/// from 4.0, low above 0.0. `None` for 0.0 (the scale's "None": no grade,
/// never "low") and for anything that is not a base score.
#[must_use]
pub fn cvss_tier(score: f64) -> Option<&'static str> {
    if !(score > 0.0 && score <= 10.0) {
        return None;
    }
    Some(if score >= 9.0 {
        "critical"
    } else if score >= 7.0 {
        "high"
    } else if score >= 4.0 {
        "medium"
    } else {
        "low"
    })
}

/// One record's CVSS evidence, both engines' rule (the MCP's
/// `extractCvssScore`): the HIGHEST valid score among every severity entry it
/// carries (`type`, and the score the host's reader computed from the entry,
/// `None` when it could not read it), with the type of the entry that gave
/// it. A 0.0 or an unreadable entry never hides one that scores:
/// PYSEC-2026-1943 (starlette) carries a v3.1 vector with no impact (0.0)
/// beside a v4.0 vector that scores 8.7, and taking the v3 entry first graded
/// it as if it had no score. A tie keeps the CVSS_V3 entry (the score most
/// tools quote), then CVSS_V4, then the order listed. With no valid score the
/// type is the entry that order puts first, and the score is `None`.
pub fn highest_cvss<'a>(
    entries: impl IntoIterator<Item = (&'a str, Option<f64>)>,
) -> (Option<&'a str>, Option<f64>) {
    let mut entries: Vec<(&str, Option<f64>)> = entries.into_iter().collect();
    let preference = |kind: &str| match kind {
        "CVSS_V3" => 0u8,
        "CVSS_V4" => 1,
        _ => 2,
    };
    // Stable: equal preference keeps the order listed.
    entries.sort_by_key(|(kind, _)| preference(kind));
    let mut best: Option<(&str, f64)> = None;
    for (kind, score) in &entries {
        let Some(score) = score.filter(|s| (0.0..=10.0).contains(s)) else {
            continue;
        };
        if best.is_none_or(|(_, b)| score > b) {
            best = Some((kind, score));
        }
    }
    match best {
        Some((kind, score)) => (Some(kind), Some(score)),
        None => (entries.first().map(|(kind, _)| *kind), None),
    }
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

    /// The CVSS v3.1 qualitative rating scale (§5), at every edge: the
    /// `@4da/mcp-server` audit's surviving mutant S05 was `>= 9.0` read as
    /// `> 9.0`. 0.0 is "None": no grade, never low.
    #[test]
    fn cvss_tiers_follow_the_rating_scale_at_every_edge() {
        let cases = [
            (10.0, Some("critical")),
            (9.0, Some("critical")),
            (8.9, Some("high")),
            (7.0, Some("high")),
            (6.9, Some("medium")),
            (4.0, Some("medium")),
            (3.9, Some("low")),
            (0.1, Some("low")),
            (0.0, None),
            (-1.0, None),
            (10.1, None),
            (f64::NAN, None),
        ];
        for (score, want) in cases {
            assert_eq!(cvss_tier(score), want, "{score}");
        }
    }

    /// PYSEC-2026-1943 (starlette) as OSV serves it: a v3.1 vector with no
    /// impact (0.0) beside a v4.0 vector that scores 8.7. The grade comes
    /// from the highest valid score, whichever entry comes first; a 0.0 or an
    /// unreadable entry never hides a readable one.
    #[test]
    fn the_highest_valid_score_grades_whatever_its_place() {
        let v3_none = ("CVSS_V3", Some(0.0));
        let v4_high = ("CVSS_V4", Some(8.7));
        assert_eq!(
            highest_cvss([v3_none, v4_high]),
            (Some("CVSS_V4"), Some(8.7))
        );
        assert_eq!(
            highest_cvss([v4_high, v3_none]),
            (Some("CVSS_V4"), Some(8.7))
        );
        assert_eq!(
            highest_cvss([("CVSS_V3", None), v4_high]),
            (Some("CVSS_V4"), Some(8.7)),
            "an unreadable v3 vector"
        );
        // Both score: the higher one, v3 or v4.
        assert_eq!(
            highest_cvss([("CVSS_V3", Some(7.5)), ("CVSS_V4", Some(9.3))]),
            (Some("CVSS_V4"), Some(9.3))
        );
        assert_eq!(
            highest_cvss([("CVSS_V4", Some(8.7)), ("CVSS_V3", Some(9.8))]),
            (Some("CVSS_V3"), Some(9.8))
        );
        // A tie keeps the v3 entry, the score most tools quote.
        assert_eq!(
            highest_cvss([("CVSS_V4", Some(7.5)), ("CVSS_V3", Some(7.5))]),
            (Some("CVSS_V3"), Some(7.5))
        );
    }

    /// Only 0.0 scores: the score is 0.0 and [`cvss_tier`] grades nothing.
    /// Nothing readable: no score, and the entry the old rule would have
    /// named (v3, else v4, else the first) still names the type.
    #[test]
    fn no_valid_score_is_no_grade() {
        assert_eq!(
            highest_cvss([("CVSS_V3", Some(0.0)), ("CVSS_V4", Some(0.0))]),
            (Some("CVSS_V3"), Some(0.0))
        );
        assert_eq!(
            highest_cvss([("CVSS_V2", None), ("CVSS_V3", None)]),
            (Some("CVSS_V3"), None)
        );
        assert_eq!(
            highest_cvss([("CVSS_V3", Some(11.0)), ("CVSS_V3", Some(f64::NAN))]),
            (Some("CVSS_V3"), None),
            "not a score"
        );
        assert_eq!(highest_cvss(std::iter::empty()), (None, None));
    }

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
        // A stated class beats the summary's wording (RUSTSEC-2025-0068).
        assert!(!is_maintenance_notice(
            "serde_yml crate is unsound and unmaintained",
            Some("unsound")
        ));
        assert!(is_maintenance_notice("crate is fine", Some("unmaintained")));
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
