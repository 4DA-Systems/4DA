// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Opt-in, READ-ONLY measurements of knowledge-gap detection against a COPY
//! of a real database. Never point these at the live `data/4da.db`.
//!
//!   FOURDA_GAPS_DB=E:/kg-dbcopy/4da.db cargo test --lib \
//!       knowledge_decay::live_tests -- --ignored --nocapture

use super::*;

fn open_copy() -> Option<rusqlite::Connection> {
    let path = std::env::var("FOURDA_GAPS_DB").ok()?;
    rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
}

/// Wall-clock of one full detection pass, twice (cold then warm page cache),
/// plus what each gap names — the audit's chrono / notify shapes are the
/// ones to read.
#[test]
#[ignore = "requires FOURDA_GAPS_DB pointing at a COPY of a real database"]
fn live_detect_knowledge_gaps_timing() {
    let Some(conn) = open_copy() else {
        eprintln!("FOURDA_GAPS_DB not set — nothing to measure");
        return;
    };
    for run in 0..2 {
        let start = std::time::Instant::now();
        let deps = crate::temporal::get_all_dependencies(&conn).expect("deps");
        let t_deps = start.elapsed().as_millis();
        let scan = super::gaps_scan::GapScan::load(&conn).expect("scan loads");
        let t_load = start.elapsed().as_millis();
        let raw = scan.scan(&deps);
        println!(
            "run {run}: phases — deps {t_deps} ms, one-time loads {} ms, per-dep scan {} ms ({} raw gaps)",
            t_load - t_deps,
            start.elapsed().as_millis() - t_load,
            raw.len()
        );
        let start = std::time::Instant::now();
        let gaps = detect_knowledge_gaps(&conn).expect("detection runs");
        println!(
            "run {run}: detect_knowledge_gaps = {} ms, {} gaps",
            start.elapsed().as_millis(),
            gaps.len()
        );
        if run == 1 {
            for g in &gaps {
                let item = g.to_evidence_item();
                println!(
                    "  {:?} {} | {} | conf {:.2} {:?}",
                    g.gap_severity,
                    item.title,
                    item.explanation,
                    item.confidence.value,
                    item.confidence.provenance
                );
            }
        }
    }
}
