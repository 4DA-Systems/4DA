// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The narrated Brief's system prompt, the FACTS block it narrates, and the
//! verdict-addressable candidate slate.
//!
//! Split from `digest_commands.rs` (declared there via `#[path]`) for size
//! hygiene and so the MACHINE TRAILER contract — the structured verdict
//! channel that AD-035 binds to the display surfaces — is testable as text.
//!
//! **Facts first (Decision 2, 2026-10-02).** The model used to receive 20
//! titles and a tech-stack string and was asked to work out what mattered.
//! It re-derived fix paths wrongly (rmcp "lockfile refresh", 10 of 10
//! briefs), missed the user's own two-major gap on fastembed, and
//! re-narrated the same items every two hours. It now receives the FACTS
//! `brief_facts` computed — fix path, worst tier, NEW vs UNCHANGED — and
//! only explains them, plus picks the articles worth reading from candidates
//! it can actually read (a body excerpt, not a title).
//!
//! **Both halves of the index contract live here on purpose.** The prompt
//! tells the model "idx is the candidate's `index` attribute";
//! `build_candidate_slate` is what renders those attributes and, in the same
//! pass, produces the ids they resolve to.
//!
//! Regression this structure prevents (2026-08-31, both merged the same day):
//! #560 inserted a titleless-row filter into the prompt slate while #580's
//! `slate_ids` recomputed its own `items.iter().take(20)` WITHOUT that filter.
//! Every verdict index past the first titleless row then addressed the wrong
//! item. There is now no second chain to fall out of sync.

use crate::brief_facts::{
    fix_clause, BriefFacts, FactStatus, SecurityFact, UpgradeFact, WorthKnowingCandidate,
};
use crate::preemption::AlertUrgency;
use crate::prompt_safety::{
    build_briefing_slate, BriefingSlate, SlateItem, UNTRUSTED_CONTENT_DEFENSE_CLAUSE,
};

/// How many candidates the narration prompt shows. Verdict indices are
/// 1-based positions within THIS cut — nowhere else.
const PROMPT_SLATE_TAKE: usize = 12;

/// Build the verdict-addressable slate of WORTH KNOWING candidates.
///
/// This is the ONE definition of the prompt's item cut. Titleless rows carry
/// nothing for the model to read and never take a slot (#560). `send_body`
/// is the `titles_only` privacy gate (`llm_egress`): without it the excerpt
/// is withheld and the model sees titles only.
pub(super) fn build_candidate_slate(
    candidates: &[WorthKnowingCandidate],
    send_body: bool,
) -> BriefingSlate {
    build_briefing_slate(
        candidates
            .iter()
            .filter(|c| !c.title.trim().is_empty())
            .take(PROMPT_SLATE_TAKE)
            .map(|c| SlateItem {
                id: c.id,
                title: &c.title,
                url: c.url.as_deref(),
                source_type: Some(&c.source_type),
                score_percent: None,
                why_matched: None,
                excerpt: if send_body {
                    Some(c.excerpt.as_str())
                } else {
                    None
                },
                published: Some(c.published.as_str()),
            }),
    )
}

fn urgency_label(u: &AlertUrgency) -> &'static str {
    match u {
        AlertUrgency::Critical => "CRITICAL",
        AlertUrgency::High => "HIGH",
        AlertUrgency::Medium => "MEDIUM",
        AlertUrgency::Watch => "LOW",
    }
}

fn status_label(s: &FactStatus) -> String {
    match s {
        FactStatus::New => "NEW".to_string(),
        FactStatus::Unchanged { since } => format!("UNCHANGED since {since} (already reported)"),
    }
}

fn site_notes(dev_only: bool, scratch: bool, dormant_days: Option<i64>) -> String {
    let mut notes: Vec<String> = Vec::new();
    if dev_only {
        notes.push("dev-only".to_string());
    }
    if scratch {
        notes.push("scratch project its repo gitignores".to_string());
    }
    if let Some(days) = dormant_days {
        notes.push(format!("project inactive {days} days"));
    }
    if notes.is_empty() {
        String::new()
    } else {
        format!(" [{}]", notes.join("; "))
    }
}

fn render_security(n: usize, f: &SecurityFact) -> String {
    let worst = f
        .worst_tier
        .as_deref()
        .map(|t| format!(", worst advisory severity {}", t.to_uppercase()))
        .unwrap_or_default();
    let ids = if f.advisory_ids.is_empty() {
        String::new()
    } else {
        format!(" ({})", f.advisory_ids.join(", "))
    };
    let mut out = format!(
        "{n}. [{}] {} ({}) — {} advisor{}{ids}{worst}. Status: {}.\n   Advisory: {}\n",
        urgency_label(&f.urgency),
        f.package,
        f.ecosystem,
        f.advisory_count,
        if f.advisory_count == 1 { "y" } else { "ies" },
        status_label(&f.status),
        f.title,
    );
    for s in &f.sites {
        let installed = s
            .installed
            .as_deref()
            .map(|v| format!("{} {v}", f.package))
            .unwrap_or_else(|| format!("{} (installed version unknown)", f.package));
        out.push_str(&format!(
            "   - {}: {installed}{} — fix: {}\n",
            s.label,
            site_notes(s.dev_only, s.scratch, s.dormant_days),
            fix_clause(&s.fix_path)
        ));
    }
    if let Some(date) = &f.first_seen {
        out.push_str(&format!("   First seen by 4DA: {date}\n"));
    }
    out
}

fn render_upgrade(n: usize, u: &UpgradeFact) -> String {
    let released = u
        .published
        .as_deref()
        .map(|d| format!(", released {d}"))
        .unwrap_or_default();
    let sites = crate::brief_facts::upgrade_sites_line(u);
    let gap = if u.yanked {
        " — a pinned version was YANKED by the publisher"
    } else {
        ""
    };
    let tooling = if u.dev_only { " [dev tooling]" } else { "" };
    format!(
        "{n}. {} {} ({}{released}){tooling} — {sites}{gap}. Status: {}.\n",
        u.package,
        u.announced,
        u.ecosystem,
        status_label(&u.status)
    )
}

/// The FACTS block of the user message: every number the brief may state.
pub(crate) fn render_facts_for_prompt(facts: &BriefFacts) -> String {
    let mut out = String::from(
        "FACTS (computed by 4DA from the user's lockfiles, OSV advisories and package registries; \
         authoritative and complete for security and upgrades):\n\n",
    );
    out.push_str(
        "ACT NOW — confirmed security, HIGH or CRITICAL (each installed version is inside the \
         advisory's affected range):\n",
    );
    if facts.security.is_empty() {
        out.push_str(
            "none — no confirmed advisory at HIGH or CRITICAL affects an installed dependency.\n",
        );
    }
    for (i, f) in facts.security.iter().enumerate() {
        out.push_str(&render_security(i + 1, f));
    }
    if !facts.also_open.is_empty() {
        out.push_str("\nALSO OPEN — lower severity (at most one line in the brief):\n");
        let rest = facts
            .also_open
            .len()
            .saturating_sub(crate::brief_facts::ALSO_OPEN_SHOWN);
        if rest > 0 {
            out.push_str(&format!(
                "({rest} more lower-severity items are listed on the Preemption tab; say so as a count.)\n"
            ));
        }
        for f in facts
            .also_open
            .iter()
            .take(crate::brief_facts::ALSO_OPEN_SHOWN)
        {
            let labels: Vec<&str> = f.sites.iter().map(|s| s.label.as_str()).collect();
            out.push_str(&format!(
                "- {} ({}, {}): {} — fix: {}. Status: {}.\n",
                f.package,
                f.ecosystem,
                urgency_label(&f.urgency),
                labels.join(", "),
                f.sites
                    .first()
                    .map(|s| fix_clause(&s.fix_path))
                    .unwrap_or_default(),
                status_label(&f.status),
            ));
        }
    }
    out.push_str(&format!(
        "\nUPGRADES — breaking or yanked releases of DIRECT dependencies published in the last {} days \
         (never the user's own packages, never a version already installed):\n",
        crate::brief_facts::UPGRADE_WINDOW_DAYS
    ));
    if facts.upgrades.is_empty() {
        out.push_str("none.\n");
    }
    for (i, u) in facts.upgrades.iter().enumerate() {
        out.push_str(&render_upgrade(i + 1, u));
    }
    out
}

/// Build the briefing system prompt: analyst persona, section structure,
/// grounding rules, and the MACHINE TRAILER contract (the fenced `rejects`
/// block parsed by `crate::brief_rejections` and stripped before render).
pub(super) fn briefing_system_prompt() -> String {
    format!(
        r#"{defense}

You are the user's intelligence analyst: a senior colleague who read everything overnight and tells them, in under a minute, what changed that touches their code.

The FACTS block in the user message was computed by software from the user's lockfiles, the OSV advisory database and the package registries. It is correct and complete for security and upgrades. Your job is to explain it, keep it short, and pick the few articles worth the user's time from the candidates — never to restate a fact differently or add one.

Write these sections in this order. Omit a section that would be empty.

## Act now
One bullet per ACT NOW fact whose status is NEW. Lead with the package and the project label, give the fix exactly as the fact's fix clause says (same parent package, same versions), and add one short clause on why it matters: what the advisory is about, its severity, dev-only or not. A fact marked UNCHANGED does not go here.

## Upgrades to plan
One bullet per UPGRADES fact whose status is NEW: the package, the new version, each project with its installed version, and how far behind it is. Say it is a breaking release and that the changelog should be read before bumping. Do not describe what changed in the release: you have not read its changelog. A [dev tooling] upgrade gets half a sentence.

## Worth knowing
At most 5 of the candidates under "Today's N items", and only those whose excerpt shows a concrete connection to one of the user's projects or dependencies in "My projects". One or two sentences each: the concrete point from the excerpt, then which project it matters to and why. Refer to the article by its title. If none qualify, write: "Nothing else worth your time today."

## Still open
One line naming every UNCHANGED fact as "package (project, since DATE)" (no such line when none is UNCHANGED). If ALSO OPEN has entries, add one line: "Lower severity: package (project), ...". Each project's gap is stated per project in the FACTS; never apply one project's gap to another.

Rules:
- Every version number, advisory id, date and project label you write must appear in the FACTS or in a candidate's excerpt. Never compute, round or guess a version or a fix.
- Copy project labels exactly as the FACTS write them (`atlas/bridge/src-tauri`, not `atlas` and never the name of a package). A package that pulls in another is a dependency, not a project.
- Severity words follow the fact: "critical" only for CRITICAL, "high" only for HIGH. Never call anything urgent that the FACTS do not.
- Security comes ONLY from ACT NOW and ALSO OPEN. A security article among the candidates is general awareness; never tell the user they are affected by it.
- Never claim an article affects a project unless its excerpt names that project's dependency or what the project does. Otherwise write "if you use X, ...".
- A transitive dependency is never fixed by bumping it in a manifest. Use the fact's fix clause.
- An item published more than 7 days ago is not news. Do not present a release the user already runs as news.
- If no fact is NEW and no candidate qualifies, the whole brief is one sentence, "Nothing new touches your code today.", followed by Still open.
- Refer to items by title, never by index number. Never write meta-commentary about 4DA, its data, freshness or pipeline.
- Plain sentences, no hype, at most 350 words.

MACHINE TRAILER (required — parsed by software and stripped before the user sees the briefing):
End your response with exactly one fenced block listing the candidates you judged NOT worth the user's time (off-stack, self-promotional, beginner material, a release they already run). Do NOT list a good candidate you left out only because five is the limit:
```rejects
[{{"idx": 3, "reason": "self-promotional"}}, {{"idx": 7, "reason": "no stack relevance"}}]
```
- "idx" is the candidate's `index` attribute from its <source_item> tag; "reason" is a short slug or phrase.
- ONLY the numbered items under "Today's N items" carry an `index` and can be filtered. Any other <source_item> in this message has NO `index` attribute and is context only — never emit an idx for one.
- Output `[]` inside the block if you filtered nothing.
- The block must be the LAST thing in your response. Never reference it, or any idx, in the briefing prose."#,
        defense = UNTRUSTED_CONTENT_DEFENSE_CLAUSE
    )
}

#[cfg(test)]
#[path = "briefing_prompt_tests.rs"]
mod tests;
