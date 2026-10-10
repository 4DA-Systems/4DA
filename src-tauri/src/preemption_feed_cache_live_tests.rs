// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The first Preemption read after a restart, before and after the restart
//! snapshot, on a SNAPSHOT of a real database (the opens migrate it and the
//! upgrade plan is persisted into it; never point this at the live file):
//!
//! ```text
//! FOURDA_DB_PATH=<snapshot>/4da.db cargo test --lib \
//!     live_preemption_restart -- --ignored --nocapture --test-threads=1
//! ```
//!
//! BEFORE is what that read paid: the cold compute on the request path (the
//! entitled fast feed, or the free floor). AFTER is a new process's first
//! read: restore the persisted feed for the caller's scope, serve it from the
//! cache and map it through the list presenter. No LLM, no settings, no
//! keychain: the scope is passed in, never read from the licence.

use std::time::Instant;

use super::*;

fn compute_cold(scope: TierScope) -> EvidenceFeed {
    match scope {
        TierScope::Full => compute_preemption_fast_full_feed(),
        TierScope::FreeFloor => compute_preemption_free_floor_feed(),
    }
    .expect("feed on snapshot")
}

fn noop_rebuild() -> FeedFuture {
    Box::pin(async { Err("not rebuilt in the harness".to_string()) })
}

#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_preemption_restart_serves_the_persisted_feed() {
    if std::env::var("FOURDA_DB_PATH").is_err() {
        return;
    }
    // The app holds the database singleton open long before a tab opens;
    // its first open (migration + integrity check) is not part of either path.
    let t = Instant::now();
    crate::get_database().expect("open snapshot");
    println!(
        "database singleton open {} ms (excluded)",
        t.elapsed().as_millis()
    );

    for scope in [TierScope::FreeFloor, TierScope::Full] {
        let file = snapshot_for(scope).path();
        let _ = std::fs::remove_file(&file);

        // Page-cache warm-up pass, then the measured cold compute.
        let _ = compute_cold(scope);
        let t = Instant::now();
        let built = compute_cold(scope);
        let cold_ms = t.elapsed().as_millis();
        let t = Instant::now();
        let _ = crate::evidence::present_preemption_list(built.clone(), &[], false);
        let present_ms = t.elapsed().as_millis();

        persist(&built);
        let bytes = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);

        let t = Instant::now();
        let conn = crate::open_db_connection().expect("conn");
        let restored = snapshot_for(scope)
            .restore::<EvidenceFeed>(&conn)
            .expect("restores");
        assert!(restored_feed_fits(&restored.value, scope));
        let cache: &'static FeedCache = Box::leak(Box::new(FeedCache::new()));
        cache.serve_restored(restored.value.clone(), noop_rebuild);
        let served = cache.serve(scope).expect("served while the rebuild runs");
        let shown = crate::evidence::present_preemption_list(served, &[], false);
        let restart_ms = t.elapsed().as_millis();

        assert_eq!(
            restored.value, built,
            "the restored feed is the persisted one"
        );
        // The other scope's caller is never served this snapshot.
        let other = match scope {
            TierScope::Full => TierScope::FreeFloor,
            TierScope::FreeFloor => TierScope::Full,
        };
        assert!(cache.serve(other).is_none(), "never across tiers");
        // A dismissal made after the snapshot was written is honoured.
        if let Some(first) = shown.items.first() {
            let dismissed = crate::evidence::present_preemption_list(
                restored.value.clone(),
                std::slice::from_ref(&first.id),
                false,
            );
            // (Dismissing a plan step may resurface the alerts it grouped, so
            // only the dismissed id's absence is asserted.)
            assert!(dismissed.items.iter().all(|i| i.id != first.id));
        }
        println!(
            "{scope:?}: BEFORE cold compute {cold_ms} ms (+ present {present_ms} ms); \
             persisted {bytes} bytes; AFTER restore+serve+present {restart_ms} ms; \
             {} items listed of {} computed",
            shown.items.len(),
            built.items.len()
        );
        let _ = std::fs::remove_file(&file);
    }
}
