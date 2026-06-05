// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Deterministic morning brief — the always-available floor (no LLM required).
//!
//! A morning brief is *facts + packaging*. The facts — which of the user's installed
//! dependencies have a verified vulnerability, which items scored most relevant and why —
//! are COMPUTED, not written: the security section is an OSV ∩ installed-versions lookup,
//! the signals are the ranked scoring output. None of it is synthesized, so none of it
//! can hallucinate.
//!
//! This module renders those facts directly into a clean, grounded brief that:
//!   * works offline and entirely locally (preserves the privacy / local-first moat),
//!   * is available to EVERY user — free, local, or without a capable model,
//!   * cannot fabricate (the Brief-grounding incident: a weak/ungrounded LLM welded a
//!     global CVE onto the wrong stack and invented an outage — impossible here).
//!
//! When the user has a Sonnet-class+ model (`llm_capability::is_brief_capable`), the LLM
//! path in `digest_commands` narrates these same facts on top. Otherwise this floor is
//! served as-is — a genuine brief, never a faked one.

use std::collections::HashMap;

use crate::db::DigestSourceItem;

/// Build the deterministic, grounded brief as Markdown. Pure: reads the preemption feed
/// and the already-ranked items; performs no synthesis and no mutation.
pub(crate) fn build_deterministic_brief(
    items: &[DigestSourceItem],
    explanations: &HashMap<i64, String>,
) -> String {
    let mut out = String::new();

    out.push_str("## Security\n");
    out.push_str(&render_security_section());

    out.push_str("\n\n## Top signals today\n");
    out.push_str(&render_signals_section(items, explanations));

    out.push_str(
        "\n---\n_Grounded brief — computed from your OSV-verified security feed and ranked \
         signals, with no AI synthesis (so it can't hallucinate). Add a Sonnet-class cloud \
         model in Settings → AI Provider for a narrated brief._\n",
    );
    out
}

/// Render the ranked "top signals" list (pure — no feed/DB access).
fn render_signals_section(
    items: &[DigestSourceItem],
    explanations: &HashMap<i64, String>,
) -> String {
    if items.is_empty() {
        return "_No relevant items today. Run an analysis to fetch and score fresh content._\n"
            .to_string();
    }
    let mut out = String::new();
    for (i, item) in items.iter().take(10).enumerate() {
        let pct = (item.relevance_score.unwrap_or(0.0) * 100.0).round() as u32;
        let why = explanations
            .get(&item.id)
            .map(String::as_str)
            .filter(|s| !s.is_empty() && *s != "No context match")
            .unwrap_or("");
        let title = item.title.trim();
        if why.is_empty() {
            out.push_str(&format!("{}. **{title}** ({pct}%)\n", i + 1));
        } else {
            out.push_str(&format!("{}. **{title}** ({pct}%) — {why}\n", i + 1));
        }
    }
    out
}

/// The deterministic security verdict from the preemption feed: confirmed, dep-scoped,
/// OSV-verified advisories, or an explicit "all clear" when there are none. Always present
/// (Preemption appears in every brief), positive or negative.
fn render_security_section() -> String {
    let feed = match crate::preemption::get_preemption_feed() {
        Ok(f) => f,
        Err(_) => {
            return "_Security feed unavailable right now._".to_string();
        }
    };

    let mut alerts: Vec<&crate::preemption::PreemptionAlert> = feed
        .alerts
        .iter()
        .filter(|a| a.osv_verified || a.source_classified)
        .collect();
    if alerts.is_empty() {
        return "✓ No confirmed vulnerabilities affecting your installed dependencies.".to_string();
    }
    // Most urgent first.
    alerts.sort_by_key(|a| urgency_rank(&a.urgency));

    let mut lines = Vec::new();
    for a in alerts.iter().take(8) {
        let (icon, label) = match a.urgency {
            crate::preemption::AlertUrgency::Critical => ("🔴", "CRITICAL"),
            crate::preemption::AlertUrgency::High => ("🟠", "HIGH"),
            crate::preemption::AlertUrgency::Medium => ("🟡", "MEDIUM"),
            crate::preemption::AlertUrgency::Watch => ("⚪", "WATCH"),
        };
        let dep = a
            .affected_dependencies
            .first()
            .map(String::as_str)
            .unwrap_or("");
        let version = match (&a.installed_version, &a.fixed_version) {
            (Some(i), Some(f)) => format!(" {i} → update to ≥ {f}"),
            (Some(i), None) => format!(" (installed {i})"),
            _ => String::new(),
        };
        let scope = if a.affected_projects.is_empty() {
            String::new()
        } else {
            format!(" — affects: {}", a.affected_projects.join(", "))
        };
        lines.push(format!(
            "{icon} **[{label}]** {dep}{version}: {}{scope}",
            a.title.trim()
        ));
    }
    lines.join("\n")
}

fn urgency_rank(u: &crate::preemption::AlertUrgency) -> u8 {
    match u {
        crate::preemption::AlertUrgency::Critical => 0,
        crate::preemption::AlertUrgency::High => 1,
        crate::preemption::AlertUrgency::Medium => 2,
        crate::preemption::AlertUrgency::Watch => 3,
    }
}

#[cfg(test)]
#[path = "briefing_deterministic_tests.rs"]
mod tests;
