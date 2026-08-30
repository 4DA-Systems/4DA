// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Deterministic security grounding for the AI briefing prompt.
//!
//! Split from `digest_commands.rs` (file-size gate): the CONFIRMED SECURITY
//! section is a self-contained, deterministic prompt input — the briefing's
//! sole authoritative source of security impact.

use tracing::info;

/// Build a deterministic, dependency-scoped security section from the OSV-verified
/// Preemption feed. This is the AUTHORITATIVE security input for the briefing: every
/// entry is matched against the user's actually-installed dependency versions and
/// already carries its exact project scope, so the LLM can no longer weld a global
/// CVE onto the wrong project or ecosystem (e.g. attributing an axios/npm advisory to
/// a Rust/Axum backend). Always returns a section (Preemption is in EVERY brief): the
/// confirmed dep-scoped advisories, or an explicit "none" all-clear when there are no
/// confirmed issues — in which case the briefing must NOT manufacture a security
/// emergency. See the brief-grounding fix (PENDING-DECISION 2026-06-06, lever 2).
pub(super) fn build_grounded_security_section() -> String {
    let feed = match crate::preemption::get_preemption_feed() {
        Ok(f) => f,
        Err(e) => {
            info!(target: "4da::briefing", error = %e, "preemption feed unavailable for briefing grounding");
            return String::new();
        }
    };

    // Dormancy lookup (2026-08-31 live audit): "Action Required" nagged about
    // graveyard projects because these lines named the affected repos with no
    // hint that nobody had touched them since February. Each dormant project
    // is labelled "(inactive N days)" so the model can weigh it honestly.
    let liveness = crate::open_db_connection()
        .map(|conn| crate::evidence::ProjectLiveness::load(&conn))
        .unwrap_or_default();

    // Only deterministic (OSV) or source-classified alerts are trustworthy enough to
    // anchor "Action Required". Heuristic signal-chain predictions are excluded.
    let mut lines: Vec<String> = Vec::new();
    for a in feed
        .alerts
        .iter()
        .filter(|a| a.osv_verified || a.source_classified)
        .take(8)
    {
        let sev = match a.urgency {
            crate::preemption::AlertUrgency::Critical => "CRITICAL",
            crate::preemption::AlertUrgency::High => "HIGH",
            crate::preemption::AlertUrgency::Medium => "MEDIUM",
            crate::preemption::AlertUrgency::Watch => "WATCH",
        };
        let version = match (&a.installed_version, &a.fixed_version) {
            (Some(i), Some(f)) => format!(" ({i} -> update to >= {f})"),
            (Some(i), None) => format!(" (installed {i})"),
            _ => String::new(),
        };
        let scope = if a.affected_projects.is_empty() {
            String::new()
        } else {
            let named: Vec<String> = a
                .affected_projects
                .iter()
                .map(|p| match liveness.dormant_days(p) {
                    Some(days) if crate::ace::dormancy::is_dormant_days(days) => {
                        format!("{p} {}", crate::evidence::inactive_label(days))
                    }
                    _ => p.clone(),
                })
                .collect();
            format!(" -- affects: {}", named.join(", "))
        };
        let dep = a
            .affected_dependencies
            .first()
            .map(String::as_str)
            .unwrap_or("");
        lines.push(format!(
            "  - [{sev}] {dep}{version}: {}{scope}",
            a.title.trim()
        ));
    }

    if lines.is_empty() {
        // Preemption appears in EVERY brief: an explicit all-clear (not silence) confirms
        // the check actually ran and forecloses the LLM inventing a vulnerability from
        // un-scoped CVE news in the day's items.
        return "\n\nCONFIRMED SECURITY: none — no OSV-verified advisory affects the user's \
                actually-installed dependencies. There are NO confirmed vulnerabilities for \
                them today; do NOT report a security action item or infer one from CVE news."
            .to_string();
    }

    format!(
        "\n\nCONFIRMED SECURITY (OSV-verified, matched to your ACTUAL installed dependency \
         versions -- the ONLY authoritative source of security impact for this briefing; each line \
         already names the exact affected project(s), so never reassign an advisory to a different \
         project or ecosystem):\n{}",
        lines.join("\n")
    )
}
