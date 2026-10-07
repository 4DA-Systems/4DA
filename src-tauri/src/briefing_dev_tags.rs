// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! "Dev tooling" is a fact, not a word the model may choose.
//!
//! Audit 2026-10-07: brief 401 tagged dotenv "[dev tooling]" although its
//! fact was runtime (navcal declares dotenv in `dependencies`). The prompt
//! printed the tag as an input marker and told the model "a [dev tooling]
//! upgrade gets half a sentence", and the only output check
//! (`check_factual_claims`) verifies version numbers. Calling a runtime
//! dependency dev tooling tells the reader it cannot reach production, which
//! is exactly the wrong way to be wrong.
//!
//! This check runs on every narrated brief: a line that calls something dev
//! tooling / dev-only and names a package whose fact is NOT dev-only loses
//! the tag. Stripping is deterministic and logged; a brief is never sent back
//! to the model (or down to the floor) for a wrong adjective.

use tracing::warn;

use crate::briefing_groundedness::PackageFact;

/// The tag spellings, lowercase.
const TAGS: &[&str] = &["dev tooling", "dev-only", "dev only"];

/// Remove the dev tag from every line that names a runtime fact package.
/// Lines that name no fact package, or only dev-only ones, are untouched.
pub(crate) fn strip_unfounded_dev_tags(brief: &str, facts: &[PackageFact]) -> String {
    let mut stripped: Vec<String> = Vec::new();
    let lines: Vec<String> = brief
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if !TAGS.iter().any(|t| lower.contains(t)) {
                return line.to_string();
            }
            let runtime: Vec<&str> = facts
                .iter()
                .filter(|f| !f.dev_only && names_package(&lower, &f.name))
                .map(|f| f.name.as_str())
                .collect();
            if runtime.is_empty() {
                return line.to_string();
            }
            stripped.extend(runtime.iter().map(|s| (*s).to_string()));
            remove_tags(line)
        })
        .collect();
    if stripped.is_empty() {
        return brief.to_string();
    }
    warn!(
        target: "4da::briefing",
        packages = ?stripped,
        "Brief called a runtime dependency dev tooling; tag removed"
    );
    let mut out = lines.join("\n");
    if brief.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Does the lowercase `text` name the package (or a scoped package's bare
/// name) as a standalone token?
fn names_package(text: &str, name: &str) -> bool {
    let full = name.to_lowercase();
    let bare = full.rsplit('/').next().unwrap_or(&full).to_string();
    [full.as_str(), bare.as_str()]
        .iter()
        .any(|alias| has_token(text, alias))
}

fn has_token(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let inner = |c: char| c.is_alphanumeric() || matches!(c, '-' | '_' | '/');
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        before.is_none_or(|c| !(inner(c) || c == '@')) && after.is_none_or(|c| !inner(c))
    })
}

/// Remove every tag spelling, with the punctuation that framed it, then tidy
/// the spacing the removal leaves.
fn remove_tags(line: &str) -> String {
    let mut out = line.to_string();
    for tag in TAGS {
        let forms = [
            format!(" [{tag}]"),
            format!("[{tag}] "),
            format!("[{tag}]"),
            format!(" ({tag})"),
            format!("({tag}) "),
            format!("({tag})"),
            format!(", {tag}"),
            format!(" — {tag}"),
            format!("; {tag}"),
            format!("{tag} "),
            (*tag).to_string(),
        ];
        for form in &forms {
            out = remove_ascii_ci(&out, form);
        }
    }
    tidy(&out)
}

/// Remove every ASCII-case-insensitive occurrence of `pat` (lowercase).
/// `to_ascii_lowercase` keeps byte offsets, so indices map back exactly.
fn remove_ascii_ci(text: &str, pat: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (i, _) in lower.match_indices(pat) {
        out.push_str(&text[last..i]);
        last = i + pat.len();
    }
    out.push_str(&text[last..]);
    out
}

fn tidy(line: &str) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, body) = line.split_at(indent_len);
    let mut body = body
        .split(' ')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    for (from, to) in [
        (" .", "."),
        (" ,", ","),
        (" ;", ";"),
        (" :", ":"),
        ("()", ""),
        ("[]", ""),
        ("****", ""),
    ] {
        body = body.replace(from, to);
    }
    format!("{indent}{}", body.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(name: &str, dev_only: bool) -> PackageFact {
        PackageFact {
            name: name.to_string(),
            versions: vec![],
            dev_only,
        }
    }

    /// Brief 401 (2026-10-07): dotenv is a runtime dependency of navcal.
    #[test]
    fn a_runtime_fact_loses_an_invented_dev_tag() {
        let facts = [fact("dotenv", false), fact("vitest", true)];
        let brief = "## Upgrades to plan\n\
                     - dotenv 17.2.0 [dev tooling] — navcal on 16.4.5 (one major version behind).\n\
                     - [dev tooling] dotenv 17.2.0 for navcal.\n\
                     - dotenv 17.2.0 for navcal, dev-only.\n";
        let out = strip_unfounded_dev_tags(brief, &facts);
        assert!(!out.to_lowercase().contains("dev tooling"), "{out}");
        assert!(!out.to_lowercase().contains("dev-only"), "{out}");
        assert!(
            out.contains("- dotenv 17.2.0 — navcal on 16.4.5 (one major version behind)."),
            "{out}"
        );
        assert!(out.contains("- dotenv 17.2.0 for navcal.\n"), "{out}");
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn a_real_dev_fact_keeps_its_tag() {
        let facts = [fact("dotenv", false), fact("vitest", true)];
        let brief = "- vitest 4.0.0 (dev tooling) — navcal on 3.2.4.";
        assert_eq!(strip_unfounded_dev_tags(brief, &facts), brief);
    }

    #[test]
    fn a_line_naming_no_fact_package_is_untouched() {
        let facts = [fact("dotenv", false)];
        let brief = "- Vite 7.1 speeds up dev tooling for everyone.";
        assert_eq!(strip_unfounded_dev_tags(brief, &facts), brief);
    }

    #[test]
    fn package_names_match_as_whole_tokens_only() {
        assert!(names_package(
            "upgrade @ai-sdk/openai now",
            "@ai-sdk/openai"
        ));
        assert!(names_package("the openai sdk", "@ai-sdk/openai"));
        assert!(!names_package("upgrade @ai-sdk/openai now", "openai"));
        assert!(!names_package("dotenv-expand 12", "dotenv"));
    }

    #[test]
    fn package_facts_carry_dev_only_from_the_facts() {
        use crate::brief_facts::{BriefFacts, FactStatus, UpgradeFact};
        let up = |pkg: &str, dev: bool| UpgradeFact {
            key: format!("npm:{pkg}"),
            package: pkg.into(),
            ecosystem: "npm".into(),
            announced: "17.0.0".into(),
            published: None,
            yanked: false,
            dev_only: dev,
            majors_behind: 1,
            pre_one: false,
            sites: vec![],
            item_id: 1,
            url: None,
            status: FactStatus::New,
        };
        let facts = BriefFacts {
            upgrades: vec![up("dotenv", false), up("vitest", true)],
            ..BriefFacts::default()
        };
        let pf = crate::brief_facts::package_facts(&facts);
        let dev = |n: &str| pf.iter().find(|p| p.name == n).map(|p| p.dev_only);
        assert_eq!(dev("dotenv"), Some(false));
        assert_eq!(dev("vitest"), Some(true));
    }
}
