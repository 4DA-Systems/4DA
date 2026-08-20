// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Anti-rollback time floor for licence expiry (audit finding F1/P1b).
//!
//! `verify_license_key` enforces the key's embedded expiry against the wall
//! clock — so once expiry enforcement shipped, winding the SYSTEM CLOCK back
//! before `expires_at` became the next bypass: every time read was a raw
//! `chrono::Utc::now()` with no backward floor. (The trial system has a
//! FORWARD clamp in gating.rs; expiry had no BACKWARD floor.)
//!
//! This module keeps a durable high-water mark of credibly observed wall-clock
//! time. Expiry is then evaluated against `max(now, floor)` — but only when
//! `now` sits so far behind the floor that no legitimate cause explains it.
//!
//! The naive version of this idea is dangerous: one spurious FORWARD reading
//! (dead CMOS battery booting in 2099) would poison the mark and lock a paying
//! user out permanently. Three defences prevent that:
//!
//! 1. **Skew tolerance** — `now` up to [`BACKWARD_SKEW_TOLERANCE_HOURS`] behind
//!    the floor is trusted unchanged (timezone misconfiguration spans 26 h,
//!    DST 1 h, NTP step corrections minutes).
//! 2. **Implausible-jump quarantine** — a forward step beyond
//!    [`MAX_PLAUSIBLE_ADVANCE_DAYS`] does NOT advance the floor. It is parked
//!    as a candidate and adopted only after it persists for
//!    [`ADVANCE_CONFIRM_HOURS`] across [`ADVANCE_CONFIRM_SESSIONS`] distinct
//!    process sessions — a garbage 2099 reading that NTP corrects within
//!    minutes can never confirm; a genuine 2-year shelf gap trivially does.
//! 3. **Self-heal valve** — a stored floor more than
//!    [`IMPLAUSIBLE_FLOOR_LEAD_DAYS`] ahead of the wall clock is treated as
//!    corrupt evidence, not proof of rollback, and resets. Even a
//!    confirmed-but-wrong floor cannot lock anyone out permanently.
//!
//! The mark is mirrored to the OS keychain and a data-dir JSON file; reads
//! max-merge both so deleting one store does not reset protection. Failure
//! anywhere fails OPEN (no floor -> behave exactly as before this module).
//!
//! **Residual limits, stated plainly (honesty-box, like the Keygen offline
//! cache — see revalidation.rs):**
//! - Deleting BOTH durable stores resets the floor for that install. This is a
//!   local-only control, not a cryptographic one.
//! - A first-ever activation performed on an already-rolled-back clock cannot
//!   be detected locally.
//! - The 48 h skew tolerance is a deliberate, bounded, one-shot gift: the
//!   floor never moves backward, so it cannot be milked repeatedly.
//!
//! Deliberately NOT consulted here: the scheduler_state DB table (a free
//! corroborator) — license-path code runs inside init windows where touching
//! the shared Database once-cell can deadlock (the state.rs:371 hazard class).

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use tracing::{info, warn};

use super::keystore;

// ============================================================================
// Constants
// ============================================================================

/// Backward movement we forgive without calling it rollback. Covers the worst
/// legitimate cases — UTC-12..UTC+14 timezone misconfiguration (26 h span), a
/// DST step, an NTP step correction, VM suspend/resume lag — with ~21 h margin.
const BACKWARD_SKEW_TOLERANCE_HOURS: i64 = 48;

/// Largest single forward step the floor adopts unconfirmed. Normal usage
/// advances hours-to-days between observations; 90 days is far beyond any real
/// gap for a live install while a BIOS-default timestamp (1980/2099) always
/// lands outside it.
const MAX_PLAUSIBLE_ADVANCE_DAYS: i64 = 90;

/// How long an implausible forward jump must persist (in its own timeline)
/// before adoption. A dead-CMOS reading is NTP-corrected within minutes of
/// boot and can never survive this; a genuine long absence trivially does.
const ADVANCE_CONFIRM_HOURS: i64 = 24;

/// Distinct process sessions the candidate timeline must be seen in.
const ADVANCE_CONFIRM_SESSIONS: u32 = 2;

/// Self-heal valve: a stored floor further ahead of the wall clock than this
/// is corrupt evidence, not rollback proof. No licence lifetime approaches
/// 5 years, so this can never punish a paying user.
const IMPLAUSIBLE_FLOOR_LEAD_DAYS: i64 = 1825;

/// Minimum interval between durable writes on the hot Advance path (the
/// in-memory floor is always current; Rollback/Quarantine/Confirm/Reset
/// persist immediately).
const PERSIST_MIN_INTERVAL_SECS: i64 = 3600;

const FLOOR_KEYCHAIN_NAME: &str = "license_time_floor";
const FLOOR_FILE_NAME: &str = "license_clock.json";

// ============================================================================
// Record + decision (pure — the unit-test seam)
// ============================================================================

/// Durable record of the highest wall-clock time credibly observed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TimeFloor {
    /// Schema version. 1 = this layout.
    #[serde(default)]
    pub(crate) v: u8,
    /// High-water mark, unix seconds. 0 = no floor established.
    #[serde(default)]
    pub(crate) floor_unix: i64,
    /// Latest observation on a quarantined candidate timeline. 0 = none.
    #[serde(default)]
    pub(crate) pending_unix: i64,
    /// First sighting of the candidate timeline, in that timeline's seconds.
    #[serde(default)]
    pub(crate) pending_since_unix: i64,
    /// Distinct process sessions the candidate has been seen in.
    #[serde(default)]
    pub(crate) pending_sessions: u32,
    /// Session id that last bumped `pending_sessions` (so one session is never
    /// counted twice).
    #[serde(default)]
    pub(crate) pending_last_session: u64,
}

/// What one observation of the wall clock implies. Pure data — [`apply`]
/// executes it. Exhaustive so new policy branches are compile-time visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloorDecision {
    /// First-ever observation; seed the floor.
    Seed(i64),
    /// Forward by a plausible amount — advance the floor.
    Advance(i64),
    /// `now` is behind the floor but within tolerance — no change, trust `now`.
    WithinSkew,
    /// Forward jump beyond plausibility — hold the floor, park the candidate.
    Quarantine {
        pending_unix: i64,
        pending_since_unix: i64,
    },
    /// A parked candidate persisted long enough — adopt its timeline.
    ConfirmAdvance(i64),
    /// `now` is behind the floor beyond tolerance — clear rollback evidence.
    Rollback { behind_secs: i64 },
    /// Stored floor is implausibly far ahead of `now` — corrupt; reset to `now`.
    ResetImplausibleFloor(i64),
}

/// Decide what observing `now` implies for `store`. Total: never panics, no I/O.
pub(crate) fn decide(now: DateTime<Utc>, store: &TimeFloor, session_id: u64) -> FloorDecision {
    let n = now.timestamp();
    let f = store.floor_unix;

    if f <= 0 {
        return FloorDecision::Seed(n);
    }
    if f - n > IMPLAUSIBLE_FLOOR_LEAD_DAYS * 86_400 {
        return FloorDecision::ResetImplausibleFloor(n);
    }

    let delta = n - f;
    if delta >= 0 {
        if delta <= MAX_PLAUSIBLE_ADVANCE_DAYS * 86_400 {
            return FloorDecision::Advance(n);
        }
        // Implausible forward jump. Consistent with the parked candidate?
        let consistent = store.pending_unix > 0
            && n >= store.pending_unix
            && n - store.pending_unix <= MAX_PLAUSIBLE_ADVANCE_DAYS * 86_400;
        if consistent {
            let session_is_new = store.pending_last_session != session_id;
            let sessions_seen = store.pending_sessions + u32::from(session_is_new);
            if n - store.pending_since_unix >= ADVANCE_CONFIRM_HOURS * 3600
                && sessions_seen >= ADVANCE_CONFIRM_SESSIONS
            {
                return FloorDecision::ConfirmAdvance(n);
            }
            return FloorDecision::Quarantine {
                pending_unix: n,
                pending_since_unix: store.pending_since_unix,
            };
        }
        return FloorDecision::Quarantine {
            pending_unix: n,
            pending_since_unix: n,
        };
    }

    if -delta <= BACKWARD_SKEW_TOLERANCE_HOURS * 3600 {
        return FloorDecision::WithinSkew;
    }
    FloorDecision::Rollback {
        behind_secs: -delta,
    }
}

/// Apply a decision in place. Returns true when the record changed in a way
/// that should be persisted durably.
pub(crate) fn apply(decision: FloorDecision, store: &mut TimeFloor, session_id: u64) -> bool {
    store.v = 1;
    match decision {
        FloorDecision::Seed(n)
        | FloorDecision::Advance(n)
        | FloorDecision::ConfirmAdvance(n)
        | FloorDecision::ResetImplausibleFloor(n) => {
            store.floor_unix = n;
            store.pending_unix = 0;
            store.pending_since_unix = 0;
            store.pending_sessions = 0;
            store.pending_last_session = 0;
            true
        }
        FloorDecision::Quarantine {
            pending_unix,
            pending_since_unix,
        } => {
            let fresh_candidate = store.pending_since_unix != pending_since_unix;
            store.pending_unix = pending_unix;
            store.pending_since_unix = pending_since_unix;
            if fresh_candidate {
                store.pending_sessions = 1;
                store.pending_last_session = session_id;
            } else if store.pending_last_session != session_id {
                store.pending_sessions = store.pending_sessions.saturating_add(1);
                store.pending_last_session = session_id;
            }
            true
        }
        // Rollback deliberately changes nothing: the floor IS the evidence,
        // and moving anything would erode it.
        FloorDecision::Rollback { .. } => false,
        FloorDecision::WithinSkew => false,
    }
}

/// The trusted floor, or `None` when unset, out of range, or distrusted.
pub(crate) fn trusted_floor_of(now: DateTime<Utc>, store: &TimeFloor) -> Option<DateTime<Utc>> {
    if store.floor_unix <= 0 {
        return None;
    }
    if store.floor_unix - now.timestamp() > IMPLAUSIBLE_FLOOR_LEAD_DAYS * 86_400 {
        return None;
    }
    DateTime::<Utc>::from_timestamp(store.floor_unix, 0)
}

/// The instant licence expiry must be evaluated against, given a candidate
/// `now` and a floor record. Monotone: the result is never EARLIER than `now`,
/// so it can only expire keys a correctly-set clock would also have expired.
pub(crate) fn effective_now_with(now: DateTime<Utc>, store: &TimeFloor) -> DateTime<Utc> {
    match trusted_floor_of(now, store) {
        Some(floor) if now < floor - Duration::hours(BACKWARD_SKEW_TOLERANCE_HOURS) => floor,
        _ => now,
    }
}

// ============================================================================
// Process-level facade (I/O, cached)
// ============================================================================

static FLOOR_CACHE: std::sync::LazyLock<parking_lot::Mutex<Option<TimeFloor>>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(None));
static LAST_PERSIST_UNIX: AtomicI64 = AtomicI64::new(0);

/// Random-enough per-process id for the distinct-session confirm rule. Not a
/// security boundary — it only needs to differ between two app launches.
static SESSION_ID: std::sync::LazyLock<u64> = std::sync::LazyLock::new(|| {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    (u64::from(std::process::id()) << 32) ^ nanos ^ 0x4DA0_C10C_4DA0_C10C
});

/// Wall-clock time for licence expiry decisions: `Utc::now()` lifted to the
/// trusted floor only on clear rollback evidence. Never fails, never panics;
/// with no floor available it is byte-identical to `Utc::now()`.
pub fn license_effective_now() -> DateTime<Utc> {
    let now = Utc::now();
    let mut cache = FLOOR_CACHE.lock();
    let store = cache.get_or_insert_with(load_floor_record);
    effective_now_with(now, store)
}

/// Record an observation of the wall clock. Best-effort, throttled, safe to
/// call from any licence path — losing an observation must never deny access,
/// so failures are logged and swallowed.
pub fn observe_license_clock() {
    let now = Utc::now();
    let mut cache = FLOOR_CACHE.lock();
    let store = cache.get_or_insert_with(load_floor_record);
    let decision = decide(now, store, *SESSION_ID);

    match decision {
        FloorDecision::Rollback { behind_secs } => {
            warn!(
                target: "4da::license",
                behind_hours = behind_secs / 3600,
                "System clock is behind the recorded time floor — licence expiry will be evaluated against the floor"
            );
        }
        FloorDecision::Quarantine { pending_unix, .. } => {
            warn!(
                target: "4da::license",
                candidate_unix = pending_unix,
                "Implausible forward clock jump quarantined — floor unchanged until the new timeline persists"
            );
        }
        FloorDecision::ResetImplausibleFloor(n) => {
            warn!(
                target: "4da::license",
                reset_to_unix = n,
                "Stored time floor is implausibly far ahead of the wall clock — treating as corrupt and resetting"
            );
        }
        FloorDecision::ConfirmAdvance(n) => {
            info!(
                target: "4da::license",
                adopted_unix = n,
                "Quarantined clock timeline persisted across sessions — adopting as the new time floor"
            );
        }
        FloorDecision::Seed(_) | FloorDecision::Advance(_) | FloorDecision::WithinSkew => {}
    }

    if apply(decision, store, *SESSION_ID) {
        // Advance is the hot path — throttle its durable writes. Everything
        // else is rare and evidentiary: persist immediately.
        let unix_now = now.timestamp();
        let throttled = matches!(decision, FloorDecision::Advance(_))
            && unix_now - LAST_PERSIST_UNIX.load(Ordering::Relaxed) < PERSIST_MIN_INTERVAL_SECS;
        if !throttled {
            save_floor_record(store);
            LAST_PERSIST_UNIX.store(unix_now, Ordering::Relaxed);
        }
    }
}

// ============================================================================
// Storage (keychain + file, max-merged)
// ============================================================================

fn floor_path_in(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join(FLOOR_FILE_NAME)
}

/// Data dir derived from the DB path, mirroring keygen.rs::load_validation_cache
/// — post-startup license paths hold the settings lock, so the SettingsManager
/// must not be consulted here (the keygen.rs:51 deadlock note).
fn data_dir() -> std::path::PathBuf {
    let db_path = crate::state::get_db_path();
    db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("data"))
        .to_path_buf()
}

pub(crate) fn load_floor_from(data_dir: &std::path::Path) -> Option<TimeFloor> {
    let content = std::fs::read_to_string(floor_path_in(data_dir)).ok()?;
    match serde_json::from_str(&content) {
        Ok(rec) => Some(rec),
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Failed to parse time-floor file — ignoring it");
            None
        }
    }
}

pub(crate) fn save_floor_to(data_dir: &std::path::Path, rec: &TimeFloor) -> bool {
    let path = floor_path_in(data_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = match serde_json::to_string(rec) {
        Ok(j) => j,
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Failed to serialize time floor");
            return false;
        }
    };
    match std::fs::write(&path, &json) {
        Ok(()) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            true
        }
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Failed to write time-floor file");
            false
        }
    }
}

/// Read both stores and max-merge: the higher floor wins, and its pending
/// state comes along whole. Deleting one store must not reset protection.
fn load_floor_record() -> TimeFloor {
    let from_keychain: Option<TimeFloor> = keystore::get_secret(FLOOR_KEYCHAIN_NAME)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok());
    let from_file = load_floor_from(&data_dir());

    match (from_keychain, from_file) {
        (Some(kc), Some(f)) => {
            if kc.floor_unix >= f.floor_unix {
                kc
            } else {
                f
            }
        }
        (Some(kc), None) => kc,
        (None, Some(f)) => f,
        (None, None) => TimeFloor::default(),
    }
}

/// Write both stores; either surviving is enough. Both failing degrades to
/// in-memory-only protection for this session — logged, fails open.
fn save_floor_record(rec: &TimeFloor) {
    let keychain_ok = match serde_json::to_string(rec) {
        Ok(json) => keystore::store_secret(FLOOR_KEYCHAIN_NAME, &json).unwrap_or(false),
        Err(_) => false,
    };
    let file_ok = save_floor_to(&data_dir(), rec);
    if !keychain_ok && !file_ok {
        warn!(
            target: "4da::license",
            "No durable store accepted the time floor (keychain and file both failed) — anti-rollback is in-memory only this session"
        );
    }
}

// ============================================================================
// Tests — pure seam, zero I/O
// ============================================================================

#[cfg(test)]
mod anti_rollback_tests {
    use super::*;

    fn at(unix: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(unix, 0).expect("timestamp in range")
    }

    /// A floor anchored mid-2026 (2026-07-01T00:00:00Z).
    const FLOOR: i64 = 1_782_864_000;

    fn floored() -> TimeFloor {
        TimeFloor {
            v: 1,
            floor_unix: FLOOR,
            ..TimeFloor::default()
        }
    }

    #[test]
    fn no_floor_uses_wall_clock() {
        let store = TimeFloor::default();
        let now = at(FLOOR);
        assert_eq!(effective_now_with(now, &store), now);
        assert_eq!(decide(now, &store, 1), FloorDecision::Seed(FLOOR));
    }

    #[test]
    fn ntp_correction_of_three_hours_is_forgiven() {
        let store = floored();
        let now = at(FLOOR - 3 * 3600);
        assert_eq!(decide(now, &store, 1), FloorDecision::WithinSkew);
        assert_eq!(effective_now_with(now, &store), now, "now stays trusted");
    }

    #[test]
    fn timezone_misconfiguration_of_26h_is_forgiven() {
        let store = floored();
        let now = at(FLOOR - 26 * 3600);
        assert_eq!(decide(now, &store, 1), FloorDecision::WithinSkew);
        assert_eq!(effective_now_with(now, &store), now);
    }

    #[test]
    fn rollback_of_six_months_returns_the_floor() {
        let store = floored();
        let now = at(FLOOR - 180 * 86_400);
        assert!(matches!(
            decide(now, &store, 1),
            FloorDecision::Rollback { .. }
        ));
        assert_eq!(
            effective_now_with(now, &store),
            at(FLOOR),
            "expiry is evaluated against the floor, not the rolled-back clock"
        );
    }

    #[test]
    fn plausible_advance_moves_the_floor() {
        let mut store = floored();
        let now = at(FLOOR + 5 * 86_400);
        let d = decide(now, &store, 1);
        assert_eq!(d, FloorDecision::Advance(FLOOR + 5 * 86_400));
        assert!(apply(d, &mut store, 1));
        assert_eq!(store.floor_unix, FLOOR + 5 * 86_400);
    }

    #[test]
    fn implausible_forward_jump_is_quarantined_not_adopted() {
        let mut store = floored();
        let year_2099 = FLOOR + 73 * 365 * 86_400;
        let d = decide(at(year_2099), &store, 1);
        assert!(matches!(d, FloorDecision::Quarantine { .. }));
        apply(d, &mut store, 1);
        assert_eq!(store.floor_unix, FLOOR, "floor is untouched by the jump");
        assert_eq!(store.pending_unix, year_2099);
        assert_eq!(store.pending_sessions, 1);
    }

    #[test]
    fn quarantined_jump_is_dropped_when_the_clock_corrects() {
        // Dead CMOS boots in 2099; NTP corrects minutes later. The corrected
        // reading is a plausible advance, which clears the parked candidate.
        let mut store = floored();
        let year_2099 = FLOOR + 73 * 365 * 86_400;
        apply(decide(at(year_2099), &store, 1), &mut store, 1);
        let corrected = at(FLOOR + 600);
        let d = decide(corrected, &store, 1);
        assert_eq!(d, FloorDecision::Advance(FLOOR + 600));
        apply(d, &mut store, 1);
        assert_eq!(store.pending_unix, 0, "candidate cleared — no poisoning");
    }

    #[test]
    fn quarantined_jump_confirms_after_24h_and_two_sessions() {
        // A machine genuinely shelved for two years: the new timeline persists,
        // so after 24 h (its own clock) and a second session it is adopted.
        let mut store = floored();
        let future = FLOOR + 2 * 365 * 86_400;
        apply(decide(at(future), &store, 1), &mut store, 1);
        // Same session, 25 h later on the candidate timeline: time bar met,
        // session bar not.
        let d = decide(at(future + 25 * 3600), &store, 1);
        assert!(matches!(d, FloorDecision::Quarantine { .. }));
        apply(d, &mut store, 1);
        // New session, later still: both bars met.
        let d = decide(at(future + 26 * 3600), &store, 2);
        assert_eq!(d, FloorDecision::ConfirmAdvance(future + 26 * 3600));
        apply(d, &mut store, 2);
        assert_eq!(store.floor_unix, future + 26 * 3600);
        assert_eq!(store.pending_unix, 0);
    }

    #[test]
    fn two_quick_sessions_alone_do_not_confirm() {
        // Two app restarts inside the same wrong hour must not adopt the jump —
        // the 24 h persistence bar also has to pass.
        let mut store = floored();
        let future = FLOOR + 2 * 365 * 86_400;
        apply(decide(at(future), &store, 1), &mut store, 1);
        let d = decide(at(future + 600), &store, 2);
        assert!(
            matches!(d, FloorDecision::Quarantine { .. }),
            "sessions bar met, time bar not — still quarantined"
        );
    }

    #[test]
    fn implausible_floor_lead_self_heals() {
        // Worst case realized: a wrong far-future floor was confirmed. When the
        // clock is corrected, the floor reads as corrupt and resets — no
        // permanent lockout.
        let mut store = TimeFloor {
            v: 1,
            floor_unix: FLOOR + 73 * 365 * 86_400,
            ..TimeFloor::default()
        };
        let now = at(FLOOR);
        assert_eq!(
            effective_now_with(now, &store),
            now,
            "distrusted floor is not applied"
        );
        let d = decide(now, &store, 1);
        assert_eq!(d, FloorDecision::ResetImplausibleFloor(FLOOR));
        apply(d, &mut store, 1);
        assert_eq!(store.floor_unix, FLOOR);
    }

    #[test]
    fn floor_never_moves_backward() {
        // Property over a mixed observation sequence: only ResetImplausibleFloor
        // may lower the floor, and it never fires while the floor is plausible.
        let mut store = floored();
        let observations = [
            FLOOR + 86_400,             // advance
            FLOOR - 30 * 86_400,        // rollback attempt
            FLOOR + 2 * 86_400,         // advance
            FLOOR - 3600,               // small skew
            FLOOR + 100 * 365 * 86_400, // absurd jump (quarantined)
            FLOOR + 3 * 86_400,         // advance (clears candidate)
        ];
        let mut high_water = store.floor_unix;
        for (i, unix) in observations.into_iter().enumerate() {
            let d = decide(at(unix), &store, i as u64);
            apply(d, &mut store, i as u64);
            assert!(
                store.floor_unix >= high_water,
                "floor regressed at step {i}: {} < {high_water}",
                store.floor_unix
            );
            high_water = store.floor_unix;
        }
    }

    #[test]
    fn effective_now_is_never_earlier_than_now() {
        // Monotone guarantee: the floor can only expire keys a correct clock
        // would also expire; it can never resurrect one.
        let store = floored();
        for unix in [FLOOR - 200 * 86_400, FLOOR - 1, FLOOR, FLOOR + 86_400] {
            let now = at(unix);
            assert!(effective_now_with(now, &store) >= now);
        }
    }

    #[test]
    fn record_round_trips_through_json_and_missing_fields_default() {
        let rec = TimeFloor {
            v: 1,
            floor_unix: FLOOR,
            pending_unix: FLOOR + 1,
            pending_since_unix: FLOOR + 1,
            pending_sessions: 1,
            pending_last_session: 42,
        };
        let json = serde_json::to_string(&rec).expect("serialize");
        let back: TimeFloor = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, rec);

        // Forward compatibility: an older/partial record parses with defaults.
        let sparse: TimeFloor = serde_json::from_str("{\"floor_unix\": 123}").expect("parse");
        assert_eq!(sparse.floor_unix, 123);
        assert_eq!(sparse.pending_sessions, 0);
    }
}
