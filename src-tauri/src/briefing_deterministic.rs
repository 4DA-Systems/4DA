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
    /// The morning notification's written summary was not available
    /// (`morning_floor`); the facts go out in its place.
    Morning(MorningWhy),
}

/// Why the morning notification carries the facts view instead of a written
/// summary. Recorded with the day's outcome, and said in the footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MorningWhy {
    /// The synthesis abstained ("low signal") on every attempt.
    Abstained,
    /// The synthesis named no fact from its input (no package, version,
    /// advisory id or item title): content-free prose is an abstention.
    ContentFree,
    /// The synthesis call failed (provider error, every provider down).
    Failed,
    /// The synthesis did not finish inside the morning budget.
    TimedOut,
    /// No synthesis-capable provider is configured.
    Unconfigured,
    /// No overnight item or alert to write from, so no synthesis ran.
    Skipped,
}

impl MorningWhy {
    /// Stable machine word for the outcome record and the log.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Abstained => "abstained",
            Self::ContentFree => "content_free",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Unconfigured => "unconfigured",
            Self::Skipped => "skipped",
        }
    }

    fn sentence(self) -> &'static str {
        match self {
            Self::Abstained => {
                "This morning's written summary found nothing it could stand behind, so the facts are shown instead."
            }
            Self::ContentFree => {
                "This morning's written summary named nothing from your stack or the overnight items, so the facts are shown instead."
            }
            Self::Failed => "This morning's written summary failed, so the facts are shown instead.",
            Self::TimedOut => {
                "This morning's written summary did not finish in time, so the facts are shown instead."
            }
            Self::Unconfigured => {
                "Add a Sonnet-class model in Settings → AI Provider for a written summary."
            }
            Self::Skipped => {
                "Nothing came in overnight for a written summary to work from, so the facts stand on their own."
            }
        }
    }
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
    } else if let Some(line) = advisory_gap_line(facts) {
        // Fresh-profile audit E3: "Nothing new touches your code today" was
        // said against 156 vulnerable packages while the advisory database
        // had never synced. Until the check is ready, say where it stands.
        out.push_str(line);
        out.push_str("\n\n");
    } else if new_security.is_empty() && new_upgrades.is_empty() && facts.worth_knowing.is_empty() {
        out.push_str(all_clear_line(facts));
        out.push_str("\n\n");
    }
    if let Some(line) = not_covered_line(facts) {
        out.push_str(&line);
        out.push_str("\n\n");
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

    if let FloorReason::Morning(why) = reason {
        // Labelled as the facts view, never as a synthesis.
        let source = if facts.no_dependencies_known {
            "Facts view, with no AI narration."
        } else {
            "Facts view: computed from your lockfiles, OSV advisories and package registries, \
             with no AI narration."
        };
        out.push_str(&with_honest_sources(
            &format!("---\n_{source} {}_\n", why.sentence()),
            facts,
        ));
        return out;
    }
    let footer = match (reason, facts.no_dependencies_known) {
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
        // Rendered above with its reason.
        (FloorReason::Morning(_), _) => "",
    };
    out.push_str(&with_honest_sources(footer, facts));
    out
}

/// What the brief says in place of an all-clear while the advisory check is
/// not ready, or `None` when it is (or was not read).
pub(crate) fn advisory_gap_line(facts: &BriefFacts) -> Option<&'static str> {
    use crate::osv::readiness::AdvisorySyncState as S;
    Some(match facts.advisories? {
        S::Ready => return None,
        S::NotStarted => {
            "Vulnerability data has not been downloaded yet, so your dependencies have not been \
             checked against published advisories and nothing here means they are clear. The \
             check starts within a few minutes; Preemption has a Check now button."
        }
        S::Syncing | S::Matching => {
            "4DA is downloading vulnerability advisories and checking your dependencies against \
             them now, so nothing here means they are clear yet. Findings appear in Preemption \
             when the check finishes."
        }
        S::Failed => {
            "The last vulnerability-advisory check failed, so 4DA cannot say your dependencies are \
             clear. It retries on its own; Preemption shows the error."
        }
        S::Stale => {
            "The vulnerability advisories 4DA last checked against are more than three days old, \
             so it does not call your dependencies clear. It re-checks on its own; Preemption \
             shows the age."
        }
        // Every project the walk found sat inside the OS temp folder: that,
        // not advisory coverage, is why nothing was checked (as Preemption
        // says).
        S::NoInventory if facts.advisory_temp_skipped > 0 => {
            "Your projects are inside the system temp folder, which 4DA treats as temporary \
             copies and does not check against vulnerability advisories, so nothing here means \
             they are clear. Move them out of the temp folder and scan again to check them."
        }
        // No installed version was read (#987 review B2): OSV covers Maven, and
        // a package.json with no lockfile is npm, so "no ecosystem it can
        // check" was false for what reaches this arm. The not-covered line
        // that follows names the files.
        S::NoInventory => {
            "4DA found dependencies but read no installed versions for them (a manifest without a \
             lockfile, or a format it does not read such as Maven or Gradle), so nothing was \
             checked against vulnerability advisories and nothing here means they are clear."
        }
    })
}

/// The all-clear, scoped to what was checked: with files 4DA did not read,
/// "your code" would claim the parts it never looked at (W5a).
pub(crate) fn all_clear_line(facts: &BriefFacts) -> &'static str {
    if facts.not_covered.is_some() {
        "Nothing new touches the code 4DA checked today."
    } else {
        "Nothing new touches your code today."
    }
}

/// Most dependency files named in the not-covered line.
const NOT_COVERED_SHOWN: usize = 3;

/// The project-relative tail of a path: `spring-petclinic/pom.xml`.
fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    parts[parts.len().saturating_sub(2)..].join("/")
}

/// `a (x), b (y), c (z), and N more` for `total` entries of which `named` are
/// shown (at most [`NOT_COVERED_SHOWN`]).
fn name_list<'a>(named: impl Iterator<Item = (&'a str, &'a str)>, total: u32) -> String {
    let shown: Vec<String> = named
        .take(NOT_COVERED_SHOWN)
        .map(|(path, what)| format!("{} ({what})", short_path(path)))
        .collect();
    let more = (total as usize).saturating_sub(shown.len());
    let tail = if more > 0 {
        format!(", and {more} more")
    } else {
        String::new()
    };
    format!("{}{tail}", shown.join(", "))
}

/// What 4DA did not check, by name (#974's not-covered report): the files it
/// does not read installed versions from, and the lockfiles it could not
/// read. `None` when every file was read.
pub(crate) fn not_covered_line(facts: &BriefFacts) -> Option<String> {
    let nc = facts.not_covered.as_ref()?;
    let mut parts: Vec<String> = Vec::new();
    if nc.count > 0 {
        let named = name_list(
            nc.files
                .iter()
                .map(|f| (f.path.as_str(), f.ecosystem.as_str())),
            nc.count,
        );
        parts.push(format!(
            "Not checked: {} dependency file{} 4DA does not read installed versions from: {named}.",
            nc.count,
            if nc.count == 1 { "" } else { "s" },
        ));
    }
    if nc.unreadable_count > 0 {
        let named = name_list(
            nc.unreadable
                .iter()
                .map(|f| (f.path.as_str(), f.reason.as_str())),
            nc.unreadable_count,
        );
        parts.push(format!(
            "{} lockfile{} could not be read, so what {} pinned was not checked: {named}.",
            nc.unreadable_count,
            if nc.unreadable_count == 1 { "" } else { "s" },
            if nc.unreadable_count == 1 {
                "it"
            } else {
                "they"
            },
        ));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// A footer that names "OSV advisories" as a source only when the advisory
/// check actually completed.
fn with_honest_sources(footer: &str, facts: &BriefFacts) -> String {
    if advisory_gap_line(facts).is_none() {
        return footer.to_string();
    }
    footer.replace(
        "lockfiles, OSV advisories and package registries",
        "lockfiles and package registries (the advisory check is not complete)",
    )
}

/// What the brief says when no lockfile has been read: the honest state and
/// the action that fixes it, never an all-clear.
pub(crate) const NO_LOCKFILES_LINE: &str = "4DA has not read any of your projects' lockfiles yet, \
     so nothing here was checked against your dependencies. Scan your projects (Settings → \
     Projects) and the brief will flag advisories and breaking releases that touch your code.";

/// A narrated brief written while nothing was checked — no lockfile known, or
/// the advisory check not ready: drop any all-clear sentence the model wrote
/// anyway and end on the line that says where things stand
/// ([`NO_LOCKFILES_LINE`] or [`advisory_gap_line`]). The prompt already says
/// so; this makes it hold regardless of what the model does with that.
pub(crate) fn honest_without_lockfiles(content: &str, facts: &BriefFacts) -> String {
    let line = if facts.no_dependencies_known {
        NO_LOCKFILES_LINE
    } else {
        match advisory_gap_line(facts) {
            Some(line) => line,
            None => return with_not_covered(content, facts),
        }
    };
    let kept: Vec<&str> = content
        .lines()
        .filter(|l| !l.contains("Nothing new touches your code"))
        .collect();
    let mut out = kept.join("\n").trim_end().to_string();
    if !out.contains(line) {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(line);
    }
    // A Maven-only project is NoInventory: the gap line says nothing was
    // checked, and this names the files (#987 review B1).
    if let Some(files) = not_covered_line(facts).filter(|f| !out.contains(f.as_str())) {
        out.push_str("\n\n");
        out.push_str(&files);
    }
    out.push('\n');
    out
}

/// A narrated brief while some files were not read (W5a): an all-clear the
/// model wrote is scoped to what was checked, and the brief ends on the line
/// that names what was not. Idempotent.
fn with_not_covered(content: &str, facts: &BriefFacts) -> String {
    let Some(line) = not_covered_line(facts) else {
        return content.to_string();
    };
    let kept: Vec<String> = content
        .lines()
        .map(|l| {
            l.replace(
                "Nothing new touches your code",
                "Nothing new touches the code 4DA checked",
            )
        })
        .collect();
    let mut out = kept.join("\n").trim_end().to_string();
    if !out.contains(&line) {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&line);
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
            let tier = crate::osv::severity::labelled(tier, f.worst_tier_from.as_deref());
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
    // A project with copies on two lines has two sites; name it once.
    let mut labels: Vec<&str> = f.sites.iter().map(|s| s.label.as_str()).collect();
    labels.dedup();
    labels.join(", ")
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
