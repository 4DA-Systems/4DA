// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Strip the model's commentary about the brief's own structure.
//!
//! Live 2026-10-18 (brief #421) the narrated brief said: "ammonia
//! (4da/src-tauri) also newly flagged: bump to >= 4.2.4 (…) — medium
//! severity, listed under Also Open rather than critical, noting it here
//! since it's NEW." The last two clauses are the model explaining where it put
//! a line, in the FACTS block's internal vocabulary ("Also Open", "NEW").
//! The prompt forbids it; this filter removes what slips through.
//!
//! It is conservative, because a false positive deletes a finding:
//! - a leading section label ("Also open: braces (navcal), …") loses only the
//!   label, never the bullet;
//! - a clause is removed only when it carries a marker AND names no package
//!   from the brief's facts ("ammonia 4.2.3 (listed under RUSTSEC-…): bump to
//!   >= 4.2.4" is kept whole);
//! - markers match whole words only ("also open" never matches inside
//!   "also openssl").

/// Phrases that only ever describe the brief itself (lowercase).
const META_MARKERS: &[&str] = &[
    "listed under",
    "noting it here",
    "noted here",
    "mentioning it here",
    "including it here",
    "also open",
    "act now section",
    "facts block",
    "in the facts",
    "per the facts",
    "this brief",
    "this section",
    "this bullet",
    "since it's new",
    "since it is new",
    "marked new",
    "status is new",
];

/// Section labels of the FACTS input the model sometimes copies as a line
/// prefix ("Also open: …"). Only the prefix is removed.
const META_LABELS: &[&str] = &["also open", "act now", "unchanged", "new"];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// `needle` occurs in `hay` (both lowercase) as whole words.
fn contains_words(hay: &str, needle: &str) -> bool {
    hay.match_indices(needle).any(|(pos, _)| {
        let before = hay[..pos].chars().next_back();
        let after = hay[pos + needle.len()..].chars().next();
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

fn has_marker(clause: &str) -> bool {
    let lower = clause.to_lowercase();
    META_MARKERS.iter().any(|m| contains_words(&lower, m))
}

fn names_a_package(clause: &str, packages: &[String]) -> bool {
    let lower = clause.to_lowercase();
    packages
        .iter()
        .any(|p| !p.is_empty() && contains_words(&lower, &p.to_lowercase()))
}

/// Split a line into clauses, keeping each clause's trailing separator
/// (", ", "; ", " — ", ". ") so the kept ones rejoin unchanged.
fn clauses(line: &str) -> Vec<(String, String)> {
    const SEPARATORS: &[&str] = &[" \u{2014} ", "; ", ", ", ". "];
    let mut out = Vec::new();
    let mut rest = line;
    loop {
        let next = SEPARATORS
            .iter()
            .filter_map(|sep| rest.find(sep).map(|pos| (pos, *sep)))
            .min_by_key(|(pos, _)| *pos);
        match next {
            Some((pos, sep)) => {
                out.push((rest[..pos].to_string(), sep.to_string()));
                rest = &rest[pos + sep.len()..];
            }
            None => {
                out.push((rest.to_string(), String::new()));
                return out;
            }
        }
    }
}

/// Remove a leading FACTS section label ("Also open: …") after the bullet
/// marker, keeping the bullet and everything after the label.
fn strip_label(line: &str) -> String {
    let body_start = line.len() - line.trim_start_matches(['-', '*', ' ']).len();
    let (bullet, body) = line.split_at(body_start);
    if let Some(colon) = body.find(':') {
        let label = body[..colon].trim().to_lowercase();
        if META_LABELS.contains(&label.as_str()) {
            return format!("{bullet}{}", body[colon + 1..].trim_start());
        }
    }
    line.to_string()
}

/// One line with its meta removed. `None` when nothing but meta was left.
fn strip_line(line: &str, packages: &[String]) -> Option<String> {
    let line = strip_label(line);
    let parts = clauses(&line);
    let drop = |clause: &str| has_marker(clause) && !names_a_package(clause, packages);
    if !parts.iter().any(|(clause, _)| drop(clause)) {
        return Some(line);
    }
    let kept: Vec<&(String, String)> = parts.iter().filter(|(c, _)| !drop(c)).collect();
    let mut text = String::new();
    for (i, (clause, sep)) in kept.iter().enumerate() {
        text.push_str(clause);
        if i + 1 < kept.len() {
            text.push_str(sep);
        }
    }
    let text = text
        .trim_end_matches([',', ';', ' ', '\u{2014}'])
        .to_string();
    if text.trim_start_matches(['-', '*', ' ']).trim().is_empty() {
        return None;
    }
    let ends = text.ends_with(['.', '!', '?', ':']);
    Some(if ends { text } else { format!("{text}.") })
}

/// The brief with meta-commentary removed (see the module doc). `packages`
/// are the names the brief's facts carry; a clause naming one is never
/// removed. Headings and blank lines are untouched.
pub(crate) fn strip_meta_commentary(brief: &str, packages: &[String]) -> String {
    let mut changed = 0usize;
    let lines: Vec<String> = brief
        .lines()
        .filter_map(|line| {
            if line.trim_start().starts_with('#') || line.trim().is_empty() {
                return Some(line.to_string());
            }
            let stripped = strip_line(line, packages);
            if stripped.as_deref() != Some(line) {
                changed += 1;
            }
            stripped
        })
        .collect();
    if changed > 0 {
        tracing::info!(target: "4da::briefing", lines = changed, "Removed meta-commentary about the brief's own structure");
    }
    let mut out = lines.join("\n");
    if brief.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkgs(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_string()).collect()
    }

    /// The exact live sentence (brief #421, 2026-10-18).
    #[test]
    fn the_live_also_open_leak_is_removed_and_the_finding_kept() {
        let line = "- ammonia (4da/src-tauri) also newly flagged: bump to >= 4.2.4 \
                    (`cargo add ammonia@4.2.4`) \u{2014} medium severity, listed under Also Open \
                    rather than critical, noting it here since it's NEW.";
        let out = strip_meta_commentary(line, &pkgs(&["ammonia", "zip"]));
        assert_eq!(
            out,
            "- ammonia (4da/src-tauri) also newly flagged: bump to >= 4.2.4 \
             (`cargo add ammonia@4.2.4`) \u{2014} medium severity."
        );
    }

    #[test]
    fn real_findings_and_headings_are_untouched() {
        let brief = "## Act now\n- Upgrade zip (4da/src-tauri): fix to >= 9.0.0 (e.g. `cargo add zip@9.0.0`) \u{2014} critical advisory, symlink handling lets extraction write outside the target.\n\n## Still open\nsharp (navcal, since 2026-10-06), uuid (4da-ledger, since 2026-10-13)\n";
        assert_eq!(
            strip_meta_commentary(brief, &pkgs(&["zip", "sharp", "uuid"])),
            brief
        );
    }

    #[test]
    fn a_line_that_is_only_meta_is_dropped() {
        let brief =
            "## Act now\n- Upgrade zip: fix to >= 9.0.0.\n- Noting it here since it's NEW.\n";
        assert_eq!(
            strip_meta_commentary(brief, &pkgs(&["zip"])),
            "## Act now\n- Upgrade zip: fix to >= 9.0.0.\n"
        );
    }

    #[test]
    fn other_structure_talk_is_removed_too() {
        let line = "Lower severity: braces (navcal), as listed in the FACTS block.";
        assert_eq!(
            strip_meta_commentary(line, &pkgs(&["braces"])),
            "Lower severity: braces (navcal)."
        );
    }

    // ---- False positives (review B2): a finding is never deleted ----

    #[test]
    fn a_section_label_prefix_loses_only_the_label() {
        let line = "- Also open: braces (navcal), uuid (4da-ledger)";
        assert_eq!(
            strip_meta_commentary(line, &pkgs(&["braces", "uuid"])),
            "- braces (navcal), uuid (4da-ledger)"
        );
    }

    #[test]
    fn a_clause_naming_a_package_is_never_dropped() {
        let line = "- ammonia 4.2.3 (listed under RUSTSEC-2026-0373): bump to >= 4.2.4";
        assert_eq!(strip_meta_commentary(line, &pkgs(&["ammonia"])), line);
    }

    #[test]
    fn new_and_open_inside_real_words_are_not_markers() {
        for line in [
            "- New minor release of tokio: 1.47.0 is out (4da/src-tauri runs 1.46.1).",
            "- openssl (navcal) also opens a new advisory: upgrade to >= 3.0.16.",
            "- newrelic (api) has a new release; also openapi-types 12.1.3 is current.",
        ] {
            assert_eq!(
                strip_meta_commentary(
                    line,
                    &pkgs(&["tokio", "openssl", "newrelic", "openapi-types"])
                ),
                line
            );
        }
    }
}
