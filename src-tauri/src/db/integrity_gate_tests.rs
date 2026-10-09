// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Marker policy for the start-up `PRAGMA quick_check` (see `integrity_gate.rs`).

use super::*;
use rusqlite::Connection;
use tempfile::tempdir;

const NOW: u64 = 1_800_000_000;
const OWN_PID: u32 = 4242;

fn fresh(at: u64) -> Result<Option<Marker>, ()> {
    Ok(Some(Marker {
        verified_at: at,
        db_created: 0,
    }))
}

fn healthy_db(dir: &Path) -> PathBuf {
    let path = dir.join("4da.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1);")
        .unwrap();
    path
}

fn nobody_alive(_: u32) -> bool {
    false
}

// ---- pure policy -----------------------------------------------------------

#[test]
fn no_marker_runs_the_check() {
    assert_eq!(
        decide(Ok(None), NOW, 0, 0),
        CheckDecision::Run(CheckReason::NoMarker)
    );
}

#[test]
fn stale_marker_runs_the_check() {
    let at = NOW - MAX_CHECK_AGE_SECS;
    assert_eq!(
        decide(fresh(at), NOW, 0, 0),
        CheckDecision::Run(CheckReason::MarkerStale {
            age_secs: MAX_CHECK_AGE_SECS
        })
    );
}

#[test]
fn unclean_previous_exit_runs_the_check_even_with_a_fresh_marker() {
    assert_eq!(
        decide(fresh(NOW - 60), NOW, 0, 1),
        CheckDecision::Run(CheckReason::UncleanExit { sessions: 1 })
    );
}

#[test]
fn fresh_marker_and_clean_exits_skip_the_check() {
    assert_eq!(
        decide(fresh(NOW - 3600), NOW, 0, 0),
        CheckDecision::Skip {
            verified_at: NOW - 3600
        }
    );
}

#[test]
fn unreadable_future_or_replaced_markers_are_never_trusted() {
    assert_eq!(
        decide(Err(()), NOW, 0, 0),
        CheckDecision::Run(CheckReason::MarkerUnreadable)
    );
    assert_eq!(
        decide(fresh(NOW + 3600), NOW, 0, 0),
        CheckDecision::Run(CheckReason::MarkerFromFuture)
    );
    let replaced = Ok(Some(Marker {
        verified_at: NOW - 60,
        db_created: 111,
    }));
    assert_eq!(
        decide(replaced, NOW, 222, 0),
        CheckDecision::Run(CheckReason::DbReplaced)
    );
}

#[test]
fn marker_format_round_trips_and_rejects_garbage() {
    assert_eq!(
        parse_marker("v1 123 456\n"),
        Some(Marker {
            verified_at: 123,
            db_created: 456
        })
    );
    assert_eq!(parse_marker("v2 123 456"), None);
    assert_eq!(parse_marker("v1 abc 456"), None);
    assert_eq!(parse_marker(""), None);
}

// ---- unclean-exit detection -----------------------------------------------

#[test]
fn dead_and_own_pid_sessions_are_unclean_live_ones_are_not() {
    let dir = tempdir().unwrap();
    let db = healthy_db(dir.path());
    for pid in [10_u32, 20, OWN_PID] {
        std::fs::write(dir.path().join(format!("4da.db.session.{pid}")), "0").unwrap();
    }
    // Not session files: other DB's, malformed, backups.
    std::fs::write(dir.path().join("other.db.session.30"), "0").unwrap();
    std::fs::write(dir.path().join("4da.db.session.x"), "0").unwrap();
    std::fs::write(dir.path().join("4da.db.backup.v3"), "0").unwrap();

    let alive = |pid: u32| pid == 20;
    let mut stale: Vec<String> = detect_unclean_exits(&db, OWN_PID, &alive)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    stale.sort();
    assert_eq!(
        stale,
        vec![
            "4da.db.session.10".to_string(),
            format!("4da.db.session.{OWN_PID}")
        ]
    );
}

// ---- pre-flight end to end -------------------------------------------------

#[test]
fn first_start_checks_and_writes_marker_then_next_start_skips() {
    let dir = tempdir().unwrap();
    let db = healthy_db(dir.path());

    assert_eq!(
        preflight_with(&db, OWN_PID, NOW, &nobody_alive),
        CorruptionRecovery::Healthy
    );
    let marker = read_marker(&db)
        .unwrap()
        .expect("ok check writes the marker");
    assert_eq!(marker.verified_at, NOW);

    assert_eq!(
        preflight_with(&db, OWN_PID, NOW + 600, &nobody_alive),
        CorruptionRecovery::CheckSkipped {
            verified_at_unix: NOW
        }
    );
    // A day later it checks again and refreshes the marker.
    let later = NOW + MAX_CHECK_AGE_SECS + 1;
    assert_eq!(
        preflight_with(&db, OWN_PID, later, &nobody_alive),
        CorruptionRecovery::Healthy
    );
    assert_eq!(read_marker(&db).unwrap().unwrap().verified_at, later);
}

#[test]
fn unclean_exit_forces_the_check_and_consumes_the_session_file() {
    let dir = tempdir().unwrap();
    let db = healthy_db(dir.path());
    assert_eq!(
        preflight_with(&db, OWN_PID, NOW, &nobody_alive),
        CorruptionRecovery::Healthy
    );
    let crashed = dir.path().join("4da.db.session.77");
    std::fs::write(&crashed, "0").unwrap();

    assert_eq!(
        preflight_with(&db, OWN_PID, NOW + 60, &nobody_alive),
        CorruptionRecovery::Healthy,
        "a dead opener's session file must force the scan"
    );
    assert!(!crashed.exists(), "a completed check consumes the evidence");
    assert!(matches!(
        preflight_with(&db, OWN_PID, NOW + 120, &nobody_alive),
        CorruptionRecovery::CheckSkipped { .. }
    ));
}

#[test]
fn a_live_concurrent_opener_does_not_force_the_check() {
    let dir = tempdir().unwrap();
    let db = healthy_db(dir.path());
    preflight_with(&db, OWN_PID, NOW, &nobody_alive);
    let gui = dir.path().join("4da.db.session.900");
    std::fs::write(&gui, "0").unwrap();

    let gui_alive = |pid: u32| pid == 900;
    assert!(matches!(
        preflight_with(&db, OWN_PID, NOW + 60, &gui_alive),
        CorruptionRecovery::CheckSkipped { .. }
    ));
    assert!(
        gui.exists(),
        "a live opener's session file is never removed"
    );
}

#[test]
fn failed_check_never_writes_the_marker() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("4da.db");
    std::fs::write(&db, b"this is not a sqlite database").unwrap();

    let verdict = preflight_with(&db, OWN_PID, NOW, &nobody_alive);
    assert!(
        matches!(verdict, CorruptionRecovery::QuarantinedNoBackup { .. }),
        "got {verdict:?}"
    );
    assert!(
        !marker_path(&db).exists(),
        "a failed check wrote the marker"
    );
}

#[test]
fn failed_check_removes_a_previously_good_marker() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("4da.db");
    std::fs::write(&db, b"corrupt").unwrap();
    write_marker(&db, NOW - 60);
    // Fresh marker, but a crashed opener forces the scan.
    std::fs::write(dir.path().join("4da.db.session.5"), "0").unwrap();

    let verdict = preflight_with(&db, OWN_PID, NOW, &nobody_alive);
    assert!(!matches!(
        verdict,
        CorruptionRecovery::Healthy | CorruptionRecovery::CheckSkipped { .. }
    ));
    assert!(!marker_path(&db).exists());
}

#[test]
fn late_verification_of_a_corrupt_file_never_vouches_for_it() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("4da.db");
    std::fs::write(&db, b"corrupt").unwrap();
    write_marker(&db, NOW - 60);

    let verdict = verify_after_open_failure(&db);
    assert!(!matches!(verdict, CorruptionRecovery::Healthy));
    assert!(!marker_path(&db).exists());
}

#[test]
fn missing_database_clears_any_marker() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("4da.db");
    std::fs::write(marker_path(&db), "v1 1 0").unwrap();
    assert_eq!(
        preflight_with(&db, OWN_PID, NOW, &nobody_alive),
        CorruptionRecovery::NoExistingDb
    );
    assert!(!marker_path(&db).exists());
}

#[test]
fn lock_contention_writes_no_marker_and_keeps_unclean_evidence() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("4da.db");
    let locker = Connection::open(&db).unwrap();
    locker
        .execute_batch(
            "PRAGMA journal_mode = DELETE;
             CREATE TABLE t (x INTEGER);
             INSERT INTO t VALUES (1);
             BEGIN EXCLUSIVE;
             INSERT INTO t VALUES (2);",
        )
        .unwrap();
    let crashed = dir.path().join("4da.db.session.8");
    std::fs::write(&crashed, "0").unwrap();

    let verdict = preflight_with(&db, OWN_PID, NOW, &nobody_alive);
    assert!(
        matches!(verdict, CorruptionRecovery::RecoveryFailed { .. }),
        "got {verdict:?}"
    );
    assert!(!marker_path(&db).exists());
    assert!(crashed.exists(), "no check ran — the next start must retry");
    let _ = locker.execute_batch("ROLLBACK;");
}
