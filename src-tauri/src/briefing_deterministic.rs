// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Deterministic brief — the always-available floor (no LLM required).
//!
//! A brief is *facts + packaging*. The facts — which installed dependencies
//! carry a confirmed vulnerability and how the fix actually arrives, which
//! direct dependencies shipped a breaking release, which fresh articles the
//! judges kept — are COMPUTED by `brief_facts`. This module renders them in
//! the same sections the narrated brief uses, so the floor and the narration
//! can never disagree on a fact, and the tab renders both the same way.
//!
//! Served when the user has no Sonnet-class model
//! (`llm_capability::is_brief_capable`), and as the fallback when a narration
//! states a version the facts do not hold. Works offline, stays private,
//! cannot fabricate.

use crate::brief_facts::{fix_clause, BriefFacts, FactStatus, SecurityFact, UpgradeFact};
use crate::preemption::AlertUrgency;

/// Articles the floor lists (its caller records them as featured).
pub(crate) const FLOOR_ARTICLES: usize = 5;

/// Why the floor was served — the footer says so honestly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloorReason {
    /// No Sonnet-class model is configured.
    NoCapableModel,
    /// A narration stated a version the facts do not hold, twice.
    NarrationRejected,
}

/// The facts brief as Markdown. Pure: performs no synthesis and no I/O.
pub(crate) fn build_deterministic_brief(facts: &BriefFacts, reason: FloorReason) -> String {
    let mut out = String::new();
    let new_security: Vec<&SecurityFact> = facts
        .security
        .iter()
        .filter(|f| f.status.is_new())
        .collect();
    let new_upgrades: Vec<&UpgradeFact> = facts
        .upgrades
        .iter()
        .filter(|u| u.status.is_new())
        .collect();

    if !new_security.is_empty() {
        out.push_str("## Act now\n");
        for f in &new_security {
            out.push_str(&security_line(f));
        }
        out.push('\n');
    }
    if !new_upgrades.is_empty() {
        out.push_str("## Upgrades to plan\n");
        for u in &new_upgrades {
            out.push_str(&upgrade_line(u));
        }
        out.push('\n');
    }
    if !facts.worth_knowing.is_empty() {
        out.push_str("## Worth knowing\n");
        for c in facts.worth_knowing.iter().take(FLOOR_ARTICLES) {
            let title = escape_markdown(&c.title);
            match &c.url {
                Some(url) => out.push_str(&format!("- [{title}]({url}) ({})\n", c.source_type)),
                None => out.push_str(&format!("- {title} ({})\n", c.source_type)),
            }
        }
        out.push('\n');
    }
    if facts.no_dependencies_known {
        // Fresh-profile E2E 2026-10-09: a user who skipped the project scan
        // was told "Nothing new touches your code today", computed "from your
        // lockfiles", when no lockfile had been read. Nothing was checked, so
        // nothing may be called clear.
        out.push_str(NO_LOCKFILES_LINE);
        out.push_str("\n\n");
    } else if new_security.is_empty() && new_upgrades.is_empty() && facts.worth_knowing.is_empty() {
        out.push_str("Nothing new touches your code today.\n\n");
    }

    let still: Vec<String> = facts
        .security
        .iter()
        .filter_map(|f| unchanged_label(&f.package, &site_labels_sec(f), &f.status))
        .chain(
            facts
                .upgrades
                .iter()
                .filter_map(|u| unchanged_label(&u.package, &site_labels_up(u), &u.status)),
        )
        .collect();
    if !still.is_empty() || !facts.also_open.is_empty() {
        out.push_str("## Still open\n");
        if !still.is_empty() {
            out.push_str(&format!("{}\n", still.join("; ")));
        }
        if !facts.also_open.is_empty() {
            out.push_str(&format!(
                "Lower severity: {}\n",
                crate::brief_facts::also_open_line(&facts.also_open)
            ));
        }
        out.push('\n');
    }

    out.push_str(match (reason, facts.no_dependencies_known) {
        (FloorReason::NoCapableModel, false) => {
            "---\n_Computed from your lockfiles, OSV advisories and package registries, with no AI \
             narration. Add a Sonnet-class model in Settings → AI Provider for a written brief._\n"
        }
        (FloorReason::NoCapableModel, true) => {
            "---\n_Computed with no AI narration. Add a Sonnet-class model in Settings → AI \
             Provider for a written brief._\n"
        }
        (FloorReason::NarrationRejected, false) => {
            "---\n_Computed from your lockfiles, OSV advisories and package registries. The written \
             brief stated a version these facts do not hold, so the facts are shown instead._\n"
        }
        (FloorReason::NarrationRejected, true) => {
            "---\n_The written brief stated a version these facts do not hold, so the facts are \
             shown instead._\n"
        }
    });
    out
}

/// What the brief says when no lockfile has been read: the honest state and
/// the action that fixes it, never an all-clear.
pub(crate) const NO_LOCKFILES_LINE: &str = "4DA has not read any of your projects' lockfiles yet, \
     so nothing here was checked against your dependencies. Scan your projects (Settings → \
     Projects) and the brief will flag advisories and breaking releases that touch your code.";

/// A narrated brief written while no lockfile is known: drop any all-clear
/// sentence the model wrote anyway and end on [`NO_LOCKFILES_LINE`]. The
/// prompt already says nothing was checked; this makes it hold regardless of
/// what the model does with that.
pub(crate) fn honest_without_lockfiles(content: &str, facts: &BriefFacts) -> String {
    if !facts.no_dependencies_known {
        return content.to_string();
    }
    let kept: Vec<&str> = content
        .lines()
        .filter(|l| !l.contains("Nothing new touches your code"))
        .collect();
    let mut out = kept.join("\n").trim_end().to_string();
    if !out.contains(NO_LOCKFILES_LINE) {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(NO_LOCKFILES_LINE);
    }
    out.push('\n');
    out
}

/// Escape the characters that would change how an interpolated title renders
/// as Markdown. LWN titles start "[$] ..." (subscriber-only), and inside a
/// link label the bracket closed the label early, so the brief showed the raw
/// Markdown (fresh-profile E2E 2026-10-09). The Brief tab's parser
/// (`briefing-parser.tsx`) reads these escapes back to the plain characters.
pub(crate) fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '\\' | '`' | '*' | '_' | '[' | ']') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn urgency_word(u: &AlertUrgency) -> &'static str {
    match u {
        AlertUrgency::Critical => "Critical",
        AlertUrgency::High => "High",
        AlertUrgency::Medium => "Medium",
        AlertUrgency::Watch => "Low",
    }
}

fn security_line(f: &SecurityFact) -> String {
    let mut line = format!(
        "- **{}** ({}, {} — {} advisor{})",
        f.package,
        f.ecosystem,
        urgency_word(&f.urgency),
        f.advisory_count,
        if f.advisory_count == 1 { "y" } else { "ies" }
    );
    if let Some(tier) = &f.worst_tier {
        if !tier.eq_ignore_ascii_case(urgency_word(&f.urgency)) {
            line.push_str(&format!("; worst advisory {tier}"));
        }
    }
    line.push_str(&format!(": {}\n", escape_markdown(&f.title)));
    for s in &f.sites {
        let installed = s.installed.as_deref().unwrap_or("version unknown");
        let dev = if s.dev_only { ", dev-only" } else { "" };
        line.push_str(&format!(
            "  - {} on {installed}{dev}: {}\n",
            s.label,
            fix_clause(&f.package, &s.fix_path)
        ));
    }
    if !f.not_compiled.is_empty() {
        let ids: Vec<&str> = f
            .not_compiled
            .iter()
            .map(|n| n.advisory_id.as_str())
            .collect();
        line.push_str(&format!(
            "  - Not counted: {} — the code {} feature-gated out of this build.\n",
            ids.join(", "),
            if ids.len() == 1 {
                "it names is"
            } else {
                "they name is"
            }
        ));
    }
    line
}

fn upgrade_line(u: &UpgradeFact) -> String {
    let sites = crate::brief_facts::upgrade_sites_line(u);
    let when = u
        .published
        .as_deref()
        .map(|d| format!(", released {d}"))
        .unwrap_or_default();
    let what = if u.yanked {
        "a pinned version was yanked by the publisher"
    } else {
        "read the changelog before bumping"
    };
    let tooling = if u.dev_only { " (dev tooling)" } else { "" };
    let title = match &u.url {
        Some(url) => format!("[{} {}]({url})", u.package, u.announced),
        None => format!("{} {}", u.package, u.announced),
    };
    format!("- **{title}**{tooling}{when}: {sites} — {what}\n")
}

fn site_labels_sec(f: &SecurityFact) -> String {
    f.sites
        .iter()
        .map(|s| s.label.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn site_labels_up(u: &UpgradeFact) -> String {
    u.sites
        .iter()
        .map(|s| format!("{} on {}", s.label, s.installed))
        .collect::<Vec<_>>()
        .join(", ")
}

fn unchanged_label(package: &str, sites: &str, status: &FactStatus) -> Option<String> {
    match status {
        FactStatus::Unchanged { since } => Some(format!("{package} ({sites}, since {since})")),
        FactStatus::New => None,
    }
}

#[cfg(test)]
#[path = "briefing_deterministic_tests.rs"]
mod tests;
