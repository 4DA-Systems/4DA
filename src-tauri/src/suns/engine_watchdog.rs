// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Engine Watchdog Sun — a quiet engine names itself (15-min cadence).
//!
//! #549 added the schema-refusal marker and startup banner, but a scheduler
//! that silently stops for any OTHER reason stayed invisible until someone
//! went looking. This sun runs from the same monitoring tick as every other
//! sun and checks the ground-truth freshness receipts (`engine_runs`,
//! written by BOTH the GUI scheduler and the headless `fourda-engine`): when
//! the newest `completed_at` is older than **3x the configured refresh
//! interval** while monitoring is enabled, it stores an `engine_stale`
//! sun_alert naming the age. The alert rides the 24-hour dedup in
//! [`super::store_sun_alert`], so a stalled engine is one refreshed row, not
//! a new row per tick.
//!
//! False-positive guards, each honest about what absence means:
//! - **Monitoring disabled** → idle; the user turned the engine off.
//! - **No `engine_runs` rows** → idle; a fresh install has no receipts yet,
//!   and "never ran" has no age to name (the first-run experience must not
//!   open on a red alert).
//! - **Watchdog uptime below the threshold** → idle; right after boot or a
//!   sleep/wake the newest receipt is legitimately old — the machine, not
//!   the engine, was down. The engine only has to answer for silence that
//!   accumulated while this process was alive to observe it.

use once_cell::sync::Lazy;

use super::SunResult;

/// The staleness multiple: an engine is "quiet" when the newest completed
/// run is older than this many refresh intervals.
const STALE_INTERVAL_MULTIPLE: u64 = 3;

/// First-observation instant for the uptime grace period (set when the first
/// watchdog tick runs, which the sun registry fires immediately at startup).
static FIRST_TICK: Lazy<std::time::Instant> = Lazy::new(std::time::Instant::now);

pub fn execute() -> SunResult {
    let uptime_secs = FIRST_TICK.elapsed().as_secs();
    let (enabled, interval_minutes) = {
        let sm = crate::get_settings_manager().lock();
        let m = &sm.get().monitoring;
        (m.enabled, m.interval_minutes)
    };
    let age_secs = last_engine_run_age_secs();

    match evaluate(enabled, interval_minutes, age_secs, uptime_secs) {
        Some(alert_msg) => {
            super::store_sun_alert("engine_watchdog", "engine_stale", &alert_msg);
            // success: true — the WATCHDOG worked; returning failure here
            // would make `tick()` file a second, duplicate 'failure' alert.
            SunResult {
                success: true,
                message: alert_msg,
                data: Some(serde_json::json!({
                    "stale": true,
                    "age_secs": age_secs,
                    "interval_minutes": interval_minutes,
                    "threshold_secs": STALE_INTERVAL_MULTIPLE * interval_minutes * 60,
                })),
            }
        }
        None => {
            let message = match (enabled, age_secs) {
                (false, _) => "Monitoring disabled — watchdog idle".to_string(),
                (true, None) => "No engine runs recorded yet".to_string(),
                (true, Some(age)) => {
                    format!("Engine healthy: last completed run {} ago", humanize(age))
                }
            };
            SunResult {
                success: true,
                message,
                data: Some(serde_json::json!({
                    "stale": false,
                    "age_secs": age_secs,
                    "interval_minutes": interval_minutes,
                })),
            }
        }
    }
}

/// Pure staleness decision — `Some(alert message)` when the engine is quiet.
/// See the module doc for what each `None` branch means.
fn evaluate(
    monitoring_enabled: bool,
    interval_minutes: u64,
    last_run_age_secs: Option<u64>,
    uptime_secs: u64,
) -> Option<String> {
    if !monitoring_enabled {
        return None;
    }
    // Settings clamp interval_minutes to 1..=1440; guard anyway so a zero can
    // never make every age "stale".
    let threshold_secs = STALE_INTERVAL_MULTIPLE * interval_minutes.max(1) * 60;
    let age_secs = last_run_age_secs?;
    if age_secs <= threshold_secs || uptime_secs < threshold_secs {
        return None;
    }
    Some(format!(
        "Engine quiet: no completed engine run for {} — more than {}x the \
         {}-minute refresh interval. The background engine may have stalled; \
         check the logs or restart 4DA.",
        humanize(age_secs),
        STALE_INTERVAL_MULTIPLE,
        interval_minutes.max(1),
    ))
}

/// Age of the newest freshness receipt, from `MAX(engine_runs.completed_at)`.
/// `None` when the table is missing (pre-first-cycle), empty, or unparseable
/// — every one of those means "nothing to name an age for", not "stale".
fn last_engine_run_age_secs() -> Option<u64> {
    let conn = crate::open_db_connection().ok()?;
    let newest: Option<String> = conn
        .query_row("SELECT MAX(completed_at) FROM engine_runs", [], |r| {
            r.get(0)
        })
        .ok()?;
    parse_age_secs(&newest?, chrono::Utc::now())
}

/// Parse a receipt timestamp (RFC3339, the only format `engine_runs` writes;
/// tolerate the bare SQLite form defensively) into an age. Future-dated
/// receipts (clock skew) read as age 0, never as stale.
fn parse_age_secs(completed_at: &str, now: chrono::DateTime<chrono::Utc>) -> Option<u64> {
    let completed = chrono::DateTime::parse_from_rfc3339(completed_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(completed_at, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc())
        })
        .ok()?;
    Some((now - completed).num_seconds().max(0) as u64)
}

/// "47 minutes" / "3.2 hours" / "2.5 days" — the alert must NAME the age.
fn humanize(age_secs: u64) -> String {
    const HOUR: u64 = 3600;
    const DAY: u64 = 86400;
    if age_secs >= DAY {
        format!("{:.1} days", age_secs as f64 / DAY as f64)
    } else if age_secs >= HOUR {
        format!("{:.1} hours", age_secs as f64 / HOUR as f64)
    } else {
        format!("{} minutes", age_secs / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEN_MIN_THRESHOLD: u64 = 3 * 10 * 60; // 1800s at the default interval

    /// The alert fires only past 3x the interval, with the process itself up
    /// long enough to have witnessed the silence.
    #[test]
    fn fires_only_past_three_intervals_with_uptime() {
        let up = TEN_MIN_THRESHOLD + 60;
        assert!(evaluate(true, 10, Some(TEN_MIN_THRESHOLD + 1), up).is_some());
        assert!(
            evaluate(true, 10, Some(TEN_MIN_THRESHOLD), up).is_none(),
            "exactly at the threshold is not yet quiet"
        );
        assert!(evaluate(true, 10, Some(120), up).is_none(), "fresh run");
    }

    /// Boot/wake grace: an old receipt observed by a young process means the
    /// MACHINE was down, not the engine.
    #[test]
    fn young_process_never_alerts_on_an_old_receipt() {
        assert!(
            evaluate(true, 10, Some(10 * 86400), 30).is_none(),
            "10-day-old receipt 30s after boot is a wake, not a stall"
        );
        assert!(evaluate(true, 10, Some(10 * 86400), TEN_MIN_THRESHOLD + 1).is_some());
    }

    /// Disabled monitoring and a receipt-less fresh install are idle states,
    /// not alerts — honest empty states, per the intelligence doctrine.
    #[test]
    fn disabled_or_never_ran_is_idle() {
        assert!(evaluate(false, 10, Some(10 * 86400), 10 * 86400).is_none());
        assert!(evaluate(true, 10, None, 10 * 86400).is_none());
    }

    /// The message names the age and the interval it was measured against.
    #[test]
    fn alert_names_the_age() {
        let msg = evaluate(true, 10, Some(2 * 86400 + 43200), 86400).unwrap();
        assert!(msg.contains("2.5 days"), "got: {msg}");
        assert!(msg.contains("10-minute"), "got: {msg}");
    }

    /// A zero interval (impossible via settings validation, cheap to guard)
    /// must not turn every age into "stale".
    #[test]
    fn zero_interval_is_clamped() {
        assert!(evaluate(true, 0, Some(170), 3600).is_none());
        assert!(evaluate(true, 0, Some(181), 3600).is_some());
    }

    /// Timestamp parsing: RFC3339 (what engine_runs writes), the bare SQLite
    /// form, clock skew, and junk.
    #[test]
    fn parse_age_handles_receipt_formats() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-08-31T12:00:00+00:00")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(parse_age_secs("2026-08-31T11:00:00+00:00", now), Some(3600));
        assert_eq!(parse_age_secs("2026-08-31 11:30:00", now), Some(1800));
        assert_eq!(
            parse_age_secs("2026-08-31T13:00:00+00:00", now),
            Some(0),
            "future-dated receipt reads as age 0, never stale"
        );
        assert_eq!(parse_age_secs("not-a-time", now), None);
    }

    #[test]
    fn humanize_names_minutes_hours_days() {
        assert_eq!(humanize(47 * 60), "47 minutes");
        assert_eq!(humanize(11520), "3.2 hours");
        assert_eq!(humanize(216_000), "2.5 days");
    }
}
