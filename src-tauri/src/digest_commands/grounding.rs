// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Deterministic security grounding for the AI briefing prompt.
//!
//! Split from `digest_commands.rs` (file-size gate): the CONFIRMED SECURITY
//! section is a self-contained, deterministic prompt input — the briefing's
//! sole authoritative source of security impact.

use tracing::info;

/// Advisory ids (GHSA-/RUSTSEC-/CVE-/PYSEC-) named anywhere in an alert's
/// evidence — urls and titles both carry them.
fn alert_advisory_ids(a: &crate::preemption::PreemptionAlert) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let texts = a
        .evidence
        .iter()
        .flat_map(|e| [Some(e.title.as_str()), e.url.as_deref()])
        .flatten();
    for text in texts {
        for prefix in ["GHSA-", "RUSTSEC-", "CVE-", "PYSEC-"] {
            for (start, _) in text.match_indices(prefix) {
                let rest = &text[start..];
                let end = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                    .unwrap_or(rest.len());
                let id = rest[..end].trim_end_matches('-').to_string();
                if id.len() > prefix.len() && !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    ids
}

/// When 4DA first held any of the alert's advisories: the earliest osv/cve
/// source row naming one of its ids. `(YYYY-MM-DD, days ago)`, or `None`
/// when no such row exists — then the prompt carries no age at all, which
/// the rules turn into "say nothing about age".
fn advisory_first_seen(a: &crate::preemption::PreemptionAlert) -> Option<(String, i64)> {
    let ids = alert_advisory_ids(a);
    if ids.is_empty() {
        return None;
    }
    let conn = crate::open_db_connection().ok()?;
    first_seen_for_ids(&conn, &ids)
}

pub(super) fn first_seen_for_ids(
    conn: &rusqlite::Connection,
    ids: &[String],
) -> Option<(String, i64)> {
    let mut earliest: Option<String> = None;
    let mut stmt = conn
        .prepare(
            "SELECT MIN(created_at) FROM source_items
             WHERE source_type IN ('osv', 'cve') AND title LIKE '%' || ?1 || '%'",
        )
        .ok()?;
    for id in ids {
        let found: Option<String> = stmt
            .query_row(rusqlite::params![id], |r| r.get::<_, Option<String>>(0))
            .ok()
            .flatten();
        if let Some(ts) = found {
            if earliest.as_ref().is_none_or(|e| ts < *e) {
                earliest = Some(ts);
            }
        }
    }
    let ts = earliest?;
    let date = ts.get(..10)?.to_string();
    let seen = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok()?;
    let days = (chrono::Utc::now().date_naive() - seen).num_days().max(0);
    Some((date, days))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{insert_test_item, test_db};

    /// The only honest age is when 4DA first held the advisory; a brief that
    /// cannot find one carries no age and the prompt rule forbids inventing it.
    #[test]
    fn first_seen_is_the_earliest_advisory_row_naming_the_id() {
        let db = test_db();
        insert_test_item(
            &db,
            "osv",
            "a1",
            "[GHSA-h395-gr6q-cpjc] jsonwebtoken: type confusion",
            "body",
        );
        let conn = db.conn.lock();
        let (date, days) = first_seen_for_ids(&conn, &["GHSA-h395-gr6q-cpjc".to_string()])
            .expect("an osv row names the id");
        assert_eq!(date, chrono::Utc::now().date_naive().to_string());
        assert_eq!(days, 0);
        assert!(
            first_seen_for_ids(&conn, &["GHSA-nope-nope-nope".to_string()]).is_none(),
            "no row, no age"
        );
    }
}

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
            (Some(i), None) => format!(" (installed {i}; no fix published)"),
            (None, None) => " (no fix published)".to_string(),
            _ => String::new(),
        };
        // The only honest age: when 4DA first held the advisory. The model
        // otherwise invented one and incremented it every brief (2026-09-07).
        let first_seen = advisory_first_seen(a)
            .map(|(date, days)| format!(" -- first seen by 4DA {date} ({days} days ago)"))
            .unwrap_or_default();
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
            "  - [{sev}] {dep}{version}: {}{scope}{first_seen}",
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
