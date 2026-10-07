// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Machine-scope trial stamp (audit 2026-10-07).
//!
//! The 14-day reverse trial used to live only in `settings.json`
//! (`license.trial_started_at`). Deleting that file restarted the trial, and a
//! hand-written far-future stamp clamped to "just started" forever.
//!
//! The stamp now lives in three places — settings.json, the machine-scope
//! keychain entry [`keystore::TRIAL_STAMP_KEY`] and `license_clock.json` — and
//! every startup merges them to the EARLIEST value. A merged stamp more than
//! [`FUTURE_TOLERANCE_HOURS`] ahead of the (clock-floored) wall clock is
//! clamped to now, so a future stamp can never extend the window.
//!
//! Residual limit, stated plainly (like clock.rs): deleting all three stores
//! restarts the trial for that install. This is a local control, not a
//! cryptographic one.

use chrono::{DateTime, Utc};
use tracing::warn;

use super::clock;
use super::keystore;

/// How far ahead of now a stamp may sit before it is treated as tampered
/// (matches the clock floor's backward skew tolerance).
const FUTURE_TOLERANCE_HOURS: i64 = 48;

/// Merge candidate stamps (unix seconds): earliest non-zero wins; a result more
/// than the tolerance ahead of `now` becomes `now`. `None` = no stamp anywhere.
pub(crate) fn merge_trial_stamps(candidates: &[Option<i64>], now: i64) -> Option<i64> {
    let earliest = candidates
        .iter()
        .flatten()
        .copied()
        .filter(|t| *t > 0)
        .min()?;
    if earliest - now > FUTURE_TOLERANCE_HOURS * 3600 {
        Some(now)
    } else {
        Some(earliest)
    }
}

fn parse_unix(stamp: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(stamp)
        .ok()
        .map(|d| d.timestamp())
}

fn to_rfc3339(unix: i64) -> String {
    DateTime::<Utc>::from_timestamp(unix, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

/// Merge the trial stamp across settings.json, the keychain (when
/// `use_keychain`) and `data_dir/license_clock.json`; write the merged value
/// back to the machine-scope stores; return it as RFC 3339. With no stamp
/// anywhere this starts the trial now.
pub fn reconcile_trial_stamp(
    data_dir: &std::path::Path,
    settings_stamp: Option<&str>,
    use_keychain: bool,
) -> String {
    let now = clock::license_effective_now().timestamp();
    let from_keychain = if use_keychain {
        keystore::get_secret(keystore::TRIAL_STAMP_KEY)
            .ok()
            .flatten()
            .and_then(|s| parse_unix(&s))
    } else {
        None
    };
    let file_rec = clock::load_floor_from(data_dir);
    let from_file = file_rec
        .as_ref()
        .map(|r| r.trial_started_unix)
        .filter(|t| *t > 0);
    let from_settings = settings_stamp.and_then(parse_unix);

    let merged = merge_trial_stamps(&[from_settings, from_keychain, from_file], now).unwrap_or(now);
    let merged_str = to_rfc3339(merged);

    let keychain_current = !use_keychain
        || from_keychain == Some(merged)
        || matches!(
            keystore::store_secret(keystore::TRIAL_STAMP_KEY, &merged_str),
            Ok(true)
        );
    if !keychain_current {
        warn!(target: "4da::license", "Could not record the trial stamp in the keychain — license_clock.json still holds it");
    }
    if from_file != Some(merged) {
        let mut rec = file_rec.unwrap_or_default();
        rec.trial_started_unix = merged;
        clock::save_floor_to(data_dir, &rec);
    }
    clock::note_trial_in_cache(merged);
    merged_str
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::keystore::test_support::IsolatedService;

    const NOW: i64 = 1_790_000_000;

    #[test]
    fn earliest_stamp_wins() {
        let merged = merge_trial_stamps(&[Some(NOW - 100), Some(NOW - 5000), None], NOW);
        assert_eq!(merged, Some(NOW - 5000));
    }

    #[test]
    fn far_future_stamp_is_clamped_to_now() {
        let year_2099 = NOW + 73 * 365 * 86_400;
        assert_eq!(merge_trial_stamps(&[Some(year_2099)], NOW), Some(NOW));
    }

    #[test]
    fn slightly_future_stamp_within_tolerance_is_kept() {
        assert_eq!(
            merge_trial_stamps(&[Some(NOW + 3600)], NOW),
            Some(NOW + 3600)
        );
    }

    #[test]
    fn no_stamp_anywhere_is_none() {
        assert_eq!(merge_trial_stamps(&[None, None, Some(0)], NOW), None);
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("4da_trial_{tag}_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// Deleting settings.json (no stamp there any more) cannot restart the
    /// trial: the machine-scope file copy brings the original stamp back.
    #[test]
    fn deleting_settings_cannot_restart_the_trial_file_store() {
        let dir = temp_dir("file");
        let first = reconcile_trial_stamp(&dir, None, false);
        let earlier = to_rfc3339(parse_unix(&first).unwrap_or(NOW) - 10 * 86_400);
        // Simulate an established 10-day-old trial recorded in the file store.
        let mut rec = clock::load_floor_from(&dir).unwrap_or_default();
        rec.trial_started_unix = parse_unix(&earlier).unwrap_or(0);
        clock::save_floor_to(&dir, &rec);

        let restored = reconcile_trial_stamp(&dir, None, false);
        assert_eq!(parse_unix(&restored), parse_unix(&earlier));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Same, through the keychain store alone (license_clock.json deleted too).
    #[test]
    fn deleting_settings_and_clock_file_cannot_restart_the_trial_keychain_store() {
        let _iso = IsolatedService::new("trial");
        let dir = temp_dir("kc");
        let old = Utc::now().timestamp() - 9 * 86_400;
        let stored =
            keystore::store_secret(keystore::TRIAL_STAMP_KEY, &to_rfc3339(old)).unwrap_or(false);
        if !stored {
            return; // no usable credential store on this host
        }
        let merged = reconcile_trial_stamp(&dir, None, true);
        assert_eq!(parse_unix(&merged), Some(old));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A hand-written 2099 stamp in settings.json loses to the real one.
    #[test]
    fn future_settings_stamp_loses_to_the_recorded_one() {
        let dir = temp_dir("future");
        let real = Utc::now().timestamp() - 3 * 86_400;
        let rec = clock::TimeFloor {
            trial_started_unix: real,
            ..clock::TimeFloor::default()
        };
        clock::save_floor_to(&dir, &rec);
        let merged = reconcile_trial_stamp(&dir, Some("2099-01-01T00:00:00Z"), false);
        assert_eq!(parse_unix(&merged), Some(real));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// With nothing else recorded, a 2099 stamp is clamped to now, not kept.
    #[test]
    fn lone_future_settings_stamp_is_clamped() {
        let dir = temp_dir("lone");
        let merged = reconcile_trial_stamp(&dir, Some("2099-01-01T00:00:00Z"), false);
        let merged_unix = parse_unix(&merged).unwrap_or(i64::MAX);
        assert!(merged_unix <= Utc::now().timestamp() + 60);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
