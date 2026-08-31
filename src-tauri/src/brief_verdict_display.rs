// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Display binding for the Brief's structured verdicts (AD-035).
//!
//! Split from `digest_commands.rs` for size hygiene (declared there via
//! `#[path]`). Live audit 2026-08-31: the AI briefing's prose filtered the
//! "DocSend alternative" article and the "Typebase" Show HN as noise while
//! the Brief hero cards and the Signal tab's "Affects You" pool promoted the
//! SAME two items as top ALERTs — on one screen. The verdicts were already
//! structured and stored (`brief_rejections`), but nothing display-side ever
//! read them. This module serves the LATEST briefing's filter verdicts to
//! the frontend, bounded by the briefing reuse window, so the promoted
//! surfaces can demote what the briefing explicitly filtered.
//!
//! Contract (one item, one verdict — demote-only):
//! - Only the LATEST briefing binds, and only while younger than
//!   [`super::briefing_reuse::BRIEFING_REUSE_WINDOW_HOURS`]. A stale
//!   briefing binds nothing; a newer verdict-less briefing (deterministic
//!   floor) unbinds everything.
//! - Fail-open everywhere: read errors, no briefing, clock skew (negative
//!   age) all serve an EMPTY verdict set — suppression can only ever lose a
//!   verdict, never invent one.
//! - Never promotes: only `filtered` verdicts are served; the trailer's
//!   judgment that an item was KEPT has no display consumer by design.

use tracing::info;

/// Build the `get_brief_display_verdicts` response from the latest
/// briefing's stored verdicts.
///
/// `latest` is `Database::get_latest_brief_verdicts` output: the newest
/// briefing's age in hours plus its recorded verdicts (empty when it
/// recorded none). `window_hours` is the freshness window the verdicts live
/// in — outside `[0, window)` nothing binds.
pub(super) fn display_verdicts_response(
    latest: Option<(f64, Vec<(i64, String)>)>,
    window_hours: f64,
) -> serde_json::Value {
    let empty = serde_json::json!({ "filtered": [], "expires_in_seconds": 0 });
    let Some((age_hours, verdicts)) = latest else {
        return empty;
    };
    // Negative age = clock skew: mirror #565's reuse gate and fail toward
    // "binds nothing" rather than serving a verdict from the future.
    if !(0.0..window_hours).contains(&age_hours) || verdicts.is_empty() {
        return empty;
    }
    let expires_in_seconds = ((window_hours - age_hours) * 3600.0).round().max(0.0) as u64;
    info!(
        target: "4da::briefing",
        count = verdicts.len(),
        age_hours = format!("{age_hours:.2}"),
        expires_in_seconds,
        "Brief display verdicts served — latest briefing binds these items (demote-only)"
    );
    let filtered: Vec<serde_json::Value> = verdicts
        .into_iter()
        .map(|(id, reason)| serde_json::json!({ "id": id, "reason": reason }))
        .collect();
    serde_json::json!({
        "filtered": filtered,
        "expires_in_seconds": expires_in_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::super::briefing_reuse::BRIEFING_REUSE_WINDOW_HOURS;
    use super::*;

    fn verdicts(v: &[(i64, &str)]) -> Vec<(i64, String)> {
        v.iter().map(|(id, r)| (*id, (*r).to_string())).collect()
    }

    #[test]
    fn fresh_briefing_verdicts_are_served_with_expiry() {
        let resp = display_verdicts_response(
            Some((1.0, verdicts(&[(42, "self-promotional"), (7, "listicle")]))),
            BRIEFING_REUSE_WINDOW_HOURS,
        );
        let filtered = resp["filtered"].as_array().expect("filtered array");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0]["id"], 42);
        assert_eq!(filtered[0]["reason"], "self-promotional");
        // 3h of the 4h window remain.
        let expires = resp["expires_in_seconds"].as_u64().expect("expiry");
        assert!((10_700..=10_900).contains(&expires), "got {expires}");
    }

    #[test]
    fn stale_briefing_binds_nothing() {
        let resp = display_verdicts_response(
            Some((BRIEFING_REUSE_WINDOW_HOURS + 0.1, verdicts(&[(42, "spam")]))),
            BRIEFING_REUSE_WINDOW_HOURS,
        );
        assert_eq!(resp["filtered"].as_array().map(Vec::len), Some(0));
        assert_eq!(resp["expires_in_seconds"], 0);
    }

    #[test]
    fn clock_skew_binds_nothing() {
        // A briefing "from the future" must not bind (mirror of #565's gate).
        let resp = display_verdicts_response(
            Some((-2.0, verdicts(&[(42, "spam")]))),
            BRIEFING_REUSE_WINDOW_HOURS,
        );
        assert_eq!(resp["filtered"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn no_briefing_and_verdictless_briefing_bind_nothing() {
        let resp = display_verdicts_response(None, BRIEFING_REUSE_WINDOW_HOURS);
        assert_eq!(resp["filtered"].as_array().map(Vec::len), Some(0));
        assert_eq!(resp["expires_in_seconds"], 0);

        // Latest briefing recorded no verdicts (deterministic floor / trailer
        // parse failed open): nothing binds, exactly as today.
        let resp = display_verdicts_response(Some((0.5, vec![])), BRIEFING_REUSE_WINDOW_HOURS);
        assert_eq!(resp["filtered"].as_array().map(Vec::len), Some(0));
        assert_eq!(resp["expires_in_seconds"], 0);
    }
}
