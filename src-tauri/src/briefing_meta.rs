// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Strip the model's commentary about the brief's own structure.
//!
//! Live 2026-10-18 (brief #421) the narrated brief said: "ammonia
//! (4da/src-tauri) also newly flagged: bump to >= 4.2.4 (…) — medium
//! severity, listed under Also Open rather than critical, noting it here
//! since it's NEW." The last two clauses are the model explaining where it put
//! a line, in the FACTS block's internal vocabulary ("Also Open", "NEW").
//! The prompt forbids it; this filter makes it impossible: a clause that
//! talks about the brief, its sections or its input is removed, and the rest
//! of the sentence stays.

/// Phrases that only ever describe the brief itself (lowercase). Chosen so a
/// real finding cannot contain them: section names of the INPUT ("also open",
/// "facts block") are never user vocabulary, and the rest name the act of
/// placing a line ("listed under", "noting it here").
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

fn is_meta(clause: &str) -> bool {
    let lower = clause.to_lowercase();
    META_MARKERS.iter().any(|m| lower.contains(m))
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

/// One line with its meta clauses removed. `None` when nothing but meta
/// was left (the line is dropped).
fn strip_line(line: &str) -> Option<String> {
    let parts = clauses(line);
    if !parts.iter().any(|(clause, _)| is_meta(clause)) {
        return Some(line.to_string());
    }
    let kept: Vec<&(String, String)> = parts.iter().filter(|(c, _)| !is_meta(c)).collect();
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
    let bare = text.trim_start_matches(['-', '*', ' ']).trim();
    if bare.is_empty() {
        return None;
    }
    let ends = text.ends_with(['.', '!', '?', ':']);
    Some(if ends { text } else { format!("{text}.") })
}

/// The brief with every meta clause removed. Headings and blank lines are
/// untouched; a line that was nothing but meta is dropped.
pub(crate) fn strip_meta_commentary(brief: &str) -> String {
    let mut removed = 0usize;
    let lines: Vec<String> = brief
        .lines()
        .filter_map(|line| {
            if line.trim_start().starts_with('#') || line.trim().is_empty() {
                return Some(line.to_string());
            }
            let stripped = strip_line(line);
            if stripped.as_deref() != Some(line) {
                removed += 1;
            }
            stripped
        })
        .collect();
    if removed > 0 {
        tracing::info!(target: "4da::briefing", lines = removed, "Removed meta-commentary about the brief's own structure");
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

    /// The exact live sentence (brief #421, 2026-10-18).
    #[test]
    fn the_live_also_open_leak_is_removed_and_the_finding_kept() {
        let line = "- ammonia (4da/src-tauri) also newly flagged: bump to >= 4.2.4 \
                    (`cargo add ammonia@4.2.4`) \u{2014} medium severity, listed under Also Open \
                    rather than critical, noting it here since it's NEW.";
        let out = strip_meta_commentary(line);
        assert_eq!(
            out,
            "- ammonia (4da/src-tauri) also newly flagged: bump to >= 4.2.4 \
             (`cargo add ammonia@4.2.4`) \u{2014} medium severity."
        );
        assert!(!out.to_lowercase().contains("also open"));
        assert!(!out.contains("noting it here"));
    }

    #[test]
    fn real_findings_and_headings_are_untouched() {
        let brief = "## Act now\n- Upgrade zip (4da/src-tauri): fix to >= 9.0.0 (e.g. `cargo add zip@9.0.0`) \u{2014} critical advisory, symlink handling lets extraction write outside the target.\n\n## Still open\nsharp (navcal, since 2026-10-06), uuid (4da-ledger, since 2026-10-13)\n";
        assert_eq!(strip_meta_commentary(brief), brief);
    }

    #[test]
    fn a_line_that_is_only_meta_is_dropped() {
        let brief =
            "## Act now\n- Upgrade zip: fix to >= 9.0.0.\n- Noting it here since it's NEW.\n";
        assert_eq!(
            strip_meta_commentary(brief),
            "## Act now\n- Upgrade zip: fix to >= 9.0.0.\n"
        );
    }

    #[test]
    fn other_structure_talk_is_removed_too() {
        let line = "Lower severity: braces (navcal), as listed in the FACTS block.";
        assert_eq!(
            strip_meta_commentary(line),
            "Lower severity: braces (navcal)."
        );
    }
}
