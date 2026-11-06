// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Auto-trigger briefing reuse: one brief a day, plus one whenever the facts
//! it was written from change.
//!
//! Split from `digest_commands.rs` for size hygiene (declared there via
//! `#[path]`).
//!
//! History. 2026-08-31: opening the window regenerated a perfectly fresh
//! briefing (32 s, 6,290 tokens), so AUTO triggers got a 4-hour reuse
//! window. 2026-10-01: 168 briefs in 6 days (~28/day), 73% of topics
//! repeating the previous one — because no frontend caller ever passed
//! `auto: true`, and because any "immediate" item (including a false hono
//! "Critical") busted the window. Decision 2 replaces the window with the
//! rule a reader expects: the brief is today's brief until something it
//! reports changes. "Changes" is the `brief_facts` fingerprint — confirmed
//! security (urgency, installed versions, fix) and breaking upgrades —
//! not "another article arrived".

use tracing::info;

use crate::brief_cadence::{has_new_act_now, DAILY_AUTO_BRIEF_CAP};

/// How fresh the LATEST briefing's filter verdicts must be to bind the
/// display surfaces (AD-035, `brief_verdict_display`). Independent of reuse.
pub(super) const BRIEFING_REUSE_WINDOW_HOURS: f64 = 4.0;

/// One brief generation at a time. The reuse/cap decision reads the latest
/// brief and today's count, and the write lands up to ~30 s later (facts
/// build + model call); two triggers in that window both saw "regenerate"
/// and both wrote. Live 2026-10-03T19:50:10Z/19:50:13Z two auto triggers
/// overlapped (both reused that time, because the facts held). Held from
/// before the decision until the brief is persisted, the second trigger sees
/// the first one's brief and reuses it, so the daily cap holds under
/// concurrency too.
pub(super) static BRIEF_GENERATION_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// `kv_store` key: the fingerprint the latest briefing was written from.
const FINGERPRINT_KV_KEY: &str = "brief_fingerprint_v1";

/// What a brief could say about the advisory check, which decides whether
/// it may give an all-clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Readiness {
    /// Every ecosystem synced and matched: an all-clear is allowed.
    Checked,
    /// A sync or the match after it is running: minutes from `Checked`.
    InProgress,
    /// Never synced, failed, stale, or nothing to check: no all-clear.
    Unchecked,
}

impl Readiness {
    pub(crate) fn of(state: Option<crate::osv::readiness::AdvisorySyncState>) -> Self {
        use crate::osv::readiness::AdvisorySyncState as S;
        match state {
            None | Some(S::Ready) => Self::Checked,
            Some(S::Syncing | S::Matching) => Self::InProgress,
            Some(S::NoInventory | S::NotStarted | S::Failed | S::Stale) => Self::Unchecked,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Checked => "checked",
            Self::InProgress => "in_progress",
            Self::Unchecked => "unchecked",
        }
    }
}

impl From<bool> for Readiness {
    fn from(ready: bool) -> Self {
        if ready {
            Self::Checked
        } else {
            Self::Unchecked
        }
    }
}

/// Record the facts a persisted briefing was written from: the fingerprint,
/// the act-now identities, whether the advisory check was ready, and the
/// running record of TODAY's briefs the daily cap compares against (every
/// act-now identity and readiness any of them carried).
pub(super) fn remember_fingerprint(
    db: &crate::db::Database,
    briefing_id: i64,
    fingerprint: &str,
    act_now: &[String],
    readiness: impl Into<Readiness>,
) {
    let day = crate::brief_facts::local_today();
    remember_fingerprint_on(
        db,
        &day,
        briefing_id,
        fingerprint,
        act_now,
        readiness.into(),
    );
}

fn remember_fingerprint_on(
    db: &crate::db::Database,
    day: &str,
    briefing_id: i64,
    fingerprint: &str,
    act_now: &[String],
    readiness: Readiness,
) {
    let previous: Option<serde_json::Value> = db
        .get_kv(FINGERPRINT_KV_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok());
    let (mut act_today, mut readiness_today) = previous
        .as_ref()
        .filter(|p| p.get("day").and_then(serde_json::Value::as_str) == Some(day))
        .map(|p| (strings(p, "act_today"), strings(p, "readiness_today")))
        .unwrap_or_default();
    act_today.extend(act_now.iter().cloned());
    act_today.sort();
    act_today.dedup();
    readiness_today.push(readiness.as_str().to_string());
    readiness_today.sort();
    readiness_today.dedup();
    let value = serde_json::json!({
        "id": briefing_id,
        "fp": fingerprint,
        "act": act_now,
        "ready": readiness == Readiness::Checked,
        "day": day,
        "act_today": act_today,
        "readiness_today": readiness_today,
    })
    .to_string();
    if let Err(e) = db.set_kv(FINGERPRINT_KV_KEY, &value) {
        tracing::warn!(target: "4da::briefing", error = %e, "briefing fingerprint not persisted");
    }
}

fn strings(v: &serde_json::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|a| serde_json::from_value(a.clone()).ok())
        .unwrap_or_default()
}

/// The latest briefing, in the same response shape as a fresh generation
/// (plus `"cached": true`), when it was written TODAY (local time) and either
/// the facts are unchanged, or today already holds [`DAILY_AUTO_BRIEF_CAP`]
/// briefs and no NEW act-now fact appeared. `None` means "regenerate",
/// including on any read error, a briefing with no recorded fingerprint, or
/// clock skew: reuse must fail toward regeneration, never toward stale
/// intelligence. A manual Regenerate never calls this.
pub(super) fn try_reuse_recent_briefing(
    db: &crate::db::Database,
    fingerprint: &str,
    act_now: &[String],
    readiness: impl Into<Readiness>,
) -> Option<serde_json::Value> {
    let offset = *chrono::Local::now().offset();
    try_reuse_at(
        db,
        fingerprint,
        act_now,
        readiness,
        chrono::Utc::now(),
        offset,
    )
}

/// The latest briefing row: id, content, model, item_count, created_at.
type LatestBriefing = (i64, String, Option<String>, i64, String);

fn latest_briefing(db: &crate::db::Database) -> Option<LatestBriefing> {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT id, content, model, item_count, created_at
         FROM briefings ORDER BY created_at DESC, id DESC LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )
    .ok()
}

/// Midnight of `now`'s local day (at `offset`), in UTC: the boundary the
/// `briefings.created_at` column (UTC, SQLite `datetime('now')`) compares to.
fn local_day_start_utc(
    now: chrono::DateTime<chrono::Utc>,
    offset: chrono::FixedOffset,
) -> chrono::NaiveDateTime {
    let local_midnight = now
        .with_timezone(&offset)
        .date_naive()
        .and_time(chrono::NaiveTime::MIN);
    local_midnight - chrono::Duration::seconds(i64::from(offset.local_minus_utc()))
}

fn briefs_since(db: &crate::db::Database, since: &str) -> i64 {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT COUNT(*) FROM briefings WHERE created_at >= ?1",
        [since],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

/// Past the daily cap, what still earns a new brief: `None` holds the cap.
///
/// Measured against everything TODAY's briefs carried, not only the latest
/// one (local 2026-10-25 wrote four auto briefs against a cap of three). Two
/// holes let a brief through on a day that had already said it all:
/// - an act-now advisory that dropped out of one brief and came back counted
///   as "new" against the latest brief alone;
/// - any change in the advisory check's readiness, in either direction,
///   regenerated. The check passes through `Syncing`/`Matching` on every
///   six-hourly sync and every start, so each sync could buy a brief.
///
/// What still breaks through: an act-now identity no brief today carried,
/// and a readiness no brief today was written in, except the in-progress
/// states (a sync minutes from done says nothing new). A record from before
/// the cap existed has no act-now list: it cannot prove nothing new arrived.
fn breaks_the_cap(
    stored: &serde_json::Value,
    act_now: &[String],
    readiness: Readiness,
    today: &str,
) -> Option<&'static str> {
    let same_day = stored.get("day").and_then(serde_json::Value::as_str) == Some(today);
    let (act_seen, readiness_seen) = if same_day {
        (
            Some(strings(stored, "act_today")),
            strings(stored, "readiness_today"),
        )
    } else {
        // An older record: the latest brief is all that is known of today.
        let act: Option<Vec<String>> = stored
            .get("act")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let ready = stored.get("ready").and_then(serde_json::Value::as_bool);
        (
            act,
            ready
                .map(|r| Readiness::from(r).as_str().to_string())
                .into_iter()
                .collect(),
        )
    };
    let Some(act_seen) = act_seen else {
        return Some("no act-now record to compare");
    };
    if has_new_act_now(&act_seen, act_now) {
        return Some("a new act-now advisory");
    }
    if readiness != Readiness::InProgress && !readiness_seen.iter().any(|r| r == readiness.as_str())
    {
        return Some(match readiness {
            Readiness::Checked => "the advisory check completed",
            _ => "the advisory check is no longer complete",
        });
    }
    None
}

fn try_reuse_at(
    db: &crate::db::Database,
    fingerprint: &str,
    act_now: &[String],
    readiness: impl Into<Readiness>,
    now: chrono::DateTime<chrono::Utc>,
    offset: chrono::FixedOffset,
) -> Option<serde_json::Value> {
    let readiness = readiness.into();
    let (id, content, model, item_count, created_at) = latest_briefing(db)?;
    let created = chrono::NaiveDateTime::parse_from_str(&created_at, "%Y-%m-%d %H:%M:%S").ok()?;
    let day_start = local_day_start_utc(now, offset);
    if created > now.naive_utc() || created < day_start {
        return None;
    }
    let stored: serde_json::Value = db
        .get_kv(FINGERPRINT_KV_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())?;
    let same_brief = stored.get("id").and_then(serde_json::Value::as_i64) == Some(id);
    let same_facts = stored.get("fp").and_then(serde_json::Value::as_str) == Some(fingerprint);
    let today = now
        .with_timezone(&offset)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let day_start = day_start.format("%Y-%m-%d %H:%M:%S").to_string();
    let at_cap = || briefs_since(db, &day_start) >= DAILY_AUTO_BRIEF_CAP;
    let reason = if same_brief && same_facts {
        "facts unchanged"
    } else if same_brief && at_cap() {
        if let Some(why) = breaks_the_cap(&stored, act_now, readiness, &today) {
            info!(
                target: "4da::briefing",
                why,
                cap = DAILY_AUTO_BRIEF_CAP,
                "Auto-trigger regenerating past the daily cap"
            );
            return None;
        }
        "daily cap reached, nothing new to act on"
    } else {
        info!(
            target: "4da::briefing",
            same_brief,
            same_facts,
            "Auto-trigger regenerating — the facts changed since today's briefing"
        );
        return None;
    };
    info!(
        target: "4da::briefing",
        item_count,
        reason,
        "Auto-trigger reusing today's briefing"
    );
    // Keep the in-memory cache (TTS / handoff readers) aligned with what the
    // UI is about to show.
    *crate::digest_config::LATEST_BRIEFING.lock() = Some(content.clone());
    Some(serde_json::json!({
        "success": true,
        "briefing": content,
        "item_count": item_count,
        "model": model,
        "latency_ms": 0,
        "cached": true,
        "briefing_created_at": created_at,
        "auto_triggered": true,
    }))
}

#[cfg(test)]
#[path = "briefing_reuse_tests.rs"]
mod tests;
