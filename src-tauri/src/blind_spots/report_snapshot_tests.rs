// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("4da-report-snapshot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join(format!("{name}.json"));
    let _ = std::fs::remove_file(&path);
    path
}

const DAY: Duration = Duration::from_hours(24);

#[test]
fn a_saved_value_comes_back_under_the_same_stamp() {
    let path = scratch("roundtrip");
    let stamp = Stamp::with_schema(124);
    save(&path, &stamp, &vec!["axum".to_string(), "hono".to_string()]).expect("save");
    let restored: Restored<Vec<String>> = load(&path, &stamp, DAY).expect("restores");
    assert_eq!(restored.value, vec!["axum".to_string(), "hono".to_string()]);
    assert!(restored.age < Duration::from_secs(60));
    assert!(
        !path.with_extension("json.tmp").exists(),
        "the temp file was renamed into place"
    );
}

#[test]
fn a_schema_or_pipeline_change_discards_the_snapshot() {
    let path = scratch("stamp");
    save(&path, &Stamp::with_schema(124), &7u32).expect("save");
    assert!(
        load::<u32>(&path, &Stamp::with_schema(125), DAY).is_none(),
        "schema moved"
    );
    let mut other_pipeline = Stamp::with_schema(124);
    other_pipeline.pipeline_version += 1;
    assert!(
        load::<u32>(&path, &other_pipeline, DAY).is_none(),
        "pipeline moved"
    );
    let mut other_app = Stamp::with_schema(124);
    other_app.app_version.push_str("-next");
    assert!(
        load::<u32>(&path, &other_app, DAY).is_none(),
        "binary moved"
    );
    let mut other_format = Stamp::with_schema(124);
    other_format.format += 1;
    assert!(
        load::<u32>(&path, &other_format, DAY).is_none(),
        "format moved"
    );
    assert_eq!(
        load::<u32>(&path, &Stamp::with_schema(124), DAY).map(|r| r.value),
        Some(7)
    );
}

#[test]
fn an_old_snapshot_or_a_future_one_is_not_restored() {
    let path = scratch("age");
    let stamp = Stamp::with_schema(124);
    for (saved_at, restorable) in [
        (now_unix() - 2 * 24 * 3600, false), // older than the one-day limit
        (now_unix() - 3600, true),
        (now_unix() + 3600, false), // the clock went backwards
    ] {
        let envelope = Envelope {
            stamp: stamp.clone(),
            saved_at,
            value: 1u8,
        };
        std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
        assert_eq!(
            load::<u8>(&path, &stamp, DAY).is_some(),
            restorable,
            "saved_at offset {}",
            saved_at - now_unix()
        );
    }
}

#[test]
fn a_missing_or_corrupt_snapshot_is_simply_absent() {
    let path = scratch("corrupt");
    let stamp = Stamp::with_schema(124);
    assert!(load::<u8>(&path, &stamp, DAY).is_none(), "missing");
    std::fs::write(&path, b"{not json").unwrap();
    assert!(load::<u8>(&path, &stamp, DAY).is_none(), "corrupt");
    save(&path, &stamp, &"a string").unwrap();
    assert!(load::<u8>(&path, &stamp, DAY).is_none(), "another shape");
}

#[test]
fn the_stamp_reads_the_databases_schema_version() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    assert!(Stamp::current(&conn).is_none(), "no schema table: no stamp");
    conn.execute_batch(
        "CREATE TABLE schema_version (version INTEGER); INSERT INTO schema_version VALUES (124);",
    )
    .unwrap();
    assert_eq!(Stamp::current(&conn), Some(Stamp::with_schema(124)));
}

#[test]
fn snapshots_live_beside_the_database() {
    let path = snapshot_path("blind_spots_snapshot.json");
    assert_eq!(
        path.parent(),
        crate::state::get_db_path().parent(),
        "next to the database, never the operator's data dir from a test"
    );
}
