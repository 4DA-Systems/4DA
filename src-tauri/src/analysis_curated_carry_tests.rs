// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `analysis_curated_carry` — the cold full-window candidate set.

use rusqlite::params;

use super::*;
use crate::test_utils::{insert_test_item, test_db};
use crate::types::SourceRelevance;

fn days_ago(days: f64) -> String {
    let at = chrono::Utc::now() - chrono::Duration::seconds((days * 86_400.0) as i64);
    at.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// One row first ingested and published `age` days ago, with `verdict` as its
/// durable feed verdict (`None` = never judged).
fn row(db: &Database, source: &str, key: &str, age: f64, verdict: Option<i64>) -> i64 {
    let id = insert_test_item(db, source, key, &format!("{source} item {key}"), "body");
    let at = days_ago(age);
    db.conn
        .lock()
        .execute(
            "UPDATE source_items SET published_at = ?1, created_at = ?1, feed_relevant = ?2
             WHERE id = ?3",
            params![at, verdict, id],
        )
        .expect("pin row");
    id
}

/// `n` fresh, unjudged devto rows — enough to fill a small newest-N window.
fn fill_window(db: &Database, n: usize) -> Vec<i64> {
    (0..n)
        .map(|i| {
            row(
                db,
                "devto",
                &format!("fresh{i}"),
                0.01 * (i + 1) as f64,
                None,
            )
        })
        .collect()
}

fn ids(items: &[StoredSourceItem]) -> Vec<i64> {
    items.iter().map(|i| i.id).collect()
}

fn shown(item: &StoredSourceItem) -> SourceRelevance {
    serde_json::from_value(serde_json::json!({
        "id": item.id,
        "title": item.title,
        "url": null,
        "top_score": 0.8,
        "matches": [],
        "relevant": true,
        "source_type": item.source_type,
    }))
    .expect("minimal SourceRelevance deserializes")
}

/// (1) The measured gap: a curated registry release older than the newest-N
/// window is in the cold candidate set, with its body loaded for scoring.
#[test]
fn cold_full_window_carries_curated_registry_row_outside_newest_n() {
    let db = test_db();
    let window = fill_window(&db, 5);
    let release = row(&db, "crates_io", "tokio-1.48", 2.0, Some(1));
    let old_release = row(&db, "npm_registry", "vite-7", 45.0, Some(1));

    // The window as `get_items_tiered` alone selects it: the release is out.
    let newest_n = db.get_items_balanced_by_source(168, 50, 5).expect("window");
    assert_eq!(ids(&newest_n), window);

    let signal = full_window_candidates_with(&db, true, 5, CURATED_CARRY_CAP)
        .expect("select")
        .items;
    let got = ids(&signal);
    assert!(got.contains(&release), "curated crates.io row carried");
    assert!(got.contains(&old_release), "Signal has no recency bound");
    assert!(window.iter().all(|id| got.contains(id)), "window kept");
    assert_eq!(got.len(), window.len() + 2, "union, no duplicates");
    let carried = signal.iter().find(|i| i.id == release).expect("carried");
    assert_eq!(carried.source_type, "crates_io");
    assert!(!carried.embedding.is_empty(), "scorable body loaded");

    // Free tier: the same 30-day history gate as every other selection.
    let free = full_window_candidates_with(&db, false, 5, CURATED_CARRY_CAP)
        .expect("select")
        .items;
    assert!(ids(&free).contains(&release));
    assert!(
        !ids(&free).contains(&old_release),
        "past the free history gate"
    );
}

/// A curated row already inside the window is not loaded twice.
#[test]
fn a_curated_row_inside_the_window_is_not_duplicated() {
    let db = test_db();
    let curated_fresh = row(&db, "hackernews", "in-window", 0.001, Some(1));
    fill_window(&db, 3);
    let got = ids(
        &full_window_candidates_with(&db, true, 10, CURATED_CARRY_CAP)
            .expect("select")
            .items,
    );
    assert_eq!(got.iter().filter(|id| **id == curated_fresh).count(), 1);
    assert_eq!(got.len(), 4);
}

/// (2) Only the standing positive verdict is carried: a rejected or unjudged
/// row stays out, and so does a source the feed no longer admits.
#[test]
fn rejected_unjudged_and_feed_excluded_rows_are_never_carried() {
    let db = test_db();
    fill_window(&db, 5);
    let curated = row(&db, "crates_io", "kept", 3.0, Some(1));
    let rejected = row(&db, "crates_io", "rejected", 3.0, Some(0));
    let unjudged = row(&db, "osv", "unjudged", 3.0, None);
    let excluded_source = row(&db, "reddit", "excluded", 1.0, Some(1));

    let got = ids(
        &full_window_candidates_with(&db, true, 5, CURATED_CARRY_CAP)
            .expect("select")
            .items,
    );
    assert!(got.contains(&curated));
    assert!(
        !got.contains(&rejected),
        "feed_relevant = 0 is never resurrected"
    );
    assert!(!got.contains(&unjudged), "no verdict, no carry");
    assert!(
        !got.contains(&excluded_source),
        "source_not_in_feed stays out"
    );
}

/// (3) The #821 news window still binds: a curated editorial row older than
/// 21 days is not carried (it would only be aged straight back out), and
/// every row the carry DOES bring survives `age_out_display_news` — while
/// the same rule still removes the aged row if anything surfaces it.
#[test]
fn curated_editorial_row_past_21_days_stays_out_of_the_display() {
    let db = test_db();
    fill_window(&db, 5);
    let aged_news = row(&db, "devto", "aged", 25.0, Some(1));
    let young_news = row(&db, "devto", "young", 20.0, Some(1));
    let aged_lobsters = row(&db, "lobsters", "aged", 15.0, Some(1));
    let old_release = row(&db, "crates_io", "old", 90.0, Some(1));
    // An old news row with CVE ids may be backend-confirmed security truth,
    // which the window exempts; only the scorer can say, so it is carried.
    let security = row(&db, "hackernews", "cve", 40.0, Some(1));
    db.conn
        .lock()
        .execute(
            "UPDATE source_items SET cve_ids = 'CVE-2026-0001' WHERE id = ?1",
            params![security],
        )
        .expect("cve");

    let items = full_window_candidates_with(&db, true, 5, CURATED_CARRY_CAP)
        .expect("select")
        .items;
    let got = ids(&items);
    assert!(!got.contains(&aged_news) && !got.contains(&aged_lobsters));
    assert!(got.contains(&young_news) && got.contains(&old_release));
    assert!(got.contains(&security));

    // Run the merged set through the display window exactly as
    // `analyze_cached_content_inner` does after scoring.
    let mut display: Vec<SourceRelevance> = items.iter().map(shown).collect();
    let security_pos = display.iter().position(|r| r.id == security as u64);
    if let Some(pos) = security_pos {
        display[pos].is_critical_alert = true;
    }
    assert_eq!(
        crate::analysis_display_window::age_out_display_news(&db, &mut display),
        0,
        "the carry never brings in a row the news window removes"
    );
    // And the rule itself is untouched: the aged row, if surfaced, leaves.
    let aged_item = load_items(&db, &[aged_news]).expect("load");
    let mut surfaced = vec![shown(&aged_item[0])];
    assert_eq!(
        crate::analysis_display_window::age_out_display_news(&db, &mut surfaced),
        1
    );
    assert_eq!(surfaced[0].excluded_by.as_deref(), Some("aged:21d"));
}

/// Rows that never age out take the cap first; news fills the rest newest
/// first.
#[test]
fn the_cap_keeps_non_aging_rows_first() {
    let db = test_db();
    fill_window(&db, 5);
    let news_new = row(&db, "hackernews", "n1", 1.0, Some(1));
    let news_old = row(&db, "hackernews", "n2", 2.0, Some(1));
    let release_old = row(&db, "crates_io", "r1", 60.0, Some(1));

    let got = ids(&full_window_candidates_with(&db, true, 5, 2)
        .expect("select")
        .items);
    assert!(
        got.contains(&release_old),
        "registry row takes the first slot"
    );
    assert!(got.contains(&news_new), "then the newest news row");
    assert!(!got.contains(&news_old), "cap reached");
}

/// Carried rows are display carry-over: they are excluded from the cycle's
/// scored set, so rank and verdict persistence (and "new this cycle"
/// receipts and notifications) cover only the window rows, while every
/// evaluated row still reaches the evidence write.
#[test]
fn carried_rows_are_not_this_cycles_verdicts_or_news() {
    let db = test_db();
    let window_ids = fill_window(&db, 2);
    let release = row(&db, "crates_io", "carry", 3.0, Some(1));
    let window = full_window_candidates_with(&db, true, 2, CURATED_CARRY_CAP).expect("select");
    assert_eq!(window.carried, [release as u64].into_iter().collect());

    let results: Vec<SourceRelevance> = window.items.iter().map(shown).collect();
    let evaluated = results
        .iter()
        .map(crate::analysis::analysis_cycle::EvaluatedItem::from)
        .collect();
    let cycle = window.cycle(crate::analysis::analysis_cycle::ScoredBatch { results, evaluated });
    let scored = cycle.scored_ids.as_ref().expect("partial scored set");
    assert!(window_ids.iter().all(|id| scored.contains(&(*id as u64))));
    assert!(!scored.contains(&(release as u64)));
    assert_eq!(cycle.scored_count(), 2);
    assert!(cycle.full_display, "the set still replaces the display");
    assert_eq!(cycle.results.len(), 3, "the carried row is displayed");
    assert_eq!(cycle.evaluated.len(), 3, "and its evidence persists");

    // With nothing carried the cycle is an ordinary full pass.
    let plain = FullWindow {
        items: Vec::new(),
        carried: HashSet::new(),
    };
    let batch = crate::analysis::analysis_cycle::ScoredBatch {
        results: Vec::new(),
        evaluated: Vec::new(),
    };
    assert!(plain.cycle(batch).scored_ids.is_none());
}

/// An empty window still reads as "cache stale, fetch" — the carry does not
/// mask it.
#[test]
fn an_empty_window_stays_empty() {
    let db = test_db();
    row(&db, "crates_io", "old", 30.0, Some(1));
    let got = full_window_candidates_with(&db, true, 5, CURATED_CARRY_CAP)
        .expect("select")
        .items;
    assert!(got.is_empty());
}

// ============================================================================
// Live measurement on a corpus snapshot (`#[ignore]`d)
// ============================================================================

/// Score `items` through `score_item` exactly as `score_items_full` does,
/// returning the results and the wall-clock milliseconds the loop took.
fn score_live(
    items: &[StoredSourceItem],
    ctx: &crate::scoring::ScoringContext,
    db: &Database,
) -> (Vec<SourceRelevance>, f64) {
    let options = crate::scoring::ScoringOptions {
        apply_freshness: true,
        apply_signals: true,
        trend_topics: vec![],
    };
    let classifier = crate::analysis::signal_classifier();
    let started = std::time::Instant::now();
    let results = items
        .iter()
        .map(|item| {
            let tags = crate::scoring::parse_tags_topics(item.tags.as_deref());
            crate::scoring::score_item(
                &crate::scoring::ScoringInput {
                    id: item.id as u64,
                    title: &item.title,
                    url: item.url.as_deref(),
                    content: &item.content,
                    source_type: &item.source_type,
                    embedding: &item.embedding,
                    created_at: Some(item.published_at.as_ref().unwrap_or(&item.created_at)),
                    detected_lang: &item.detected_lang,
                    source_tags: &tags,
                    tags_json: item.tags.as_deref(),
                    feed_origin: item.feed_origin.as_deref(),
                    source_id: Some(&item.source_id),
                },
                ctx,
                db,
                &options,
                Some(classifier),
            )
        })
        .collect();
    (results, started.elapsed().as_secs_f64() * 1000.0)
}

/// Shown rows by source after the cycle's two display passes (durable-verdict
/// convergence, then the #821 news window), as `analyze_cached_content_inner`
/// applies them. The batch layer (dedup, percentile, rerank) is NOT applied.
fn shown_by_source(db: &Database, results: &mut [SourceRelevance]) -> Vec<(String, usize)> {
    crate::analysis_verdicts::converge_display_on_durable_verdicts(db, results);
    crate::analysis_display_window::age_out_display_news(db, results);
    let mut by: std::collections::BTreeMap<String, usize> = Default::default();
    for r in results.iter().filter(|r| r.relevant && !r.excluded) {
        *by.entry(r.source_type.clone()).or_default() += 1;
    }
    let mut v: Vec<(String, usize)> = by.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v
}

/// Before/after cold-start display estimate and the added scoring cost, on a
/// SNAPSHOT (the open migrates it; never point it at the live file):
///
/// ```text
/// FOURDA_DB_PATH=<snapshot> FOURDA_DATA_DIR=<scratch> cargo test --lib \
///     live_cold_window_carry_on_snapshot -- --ignored --nocapture
/// ```
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_cold_window_carry_on_snapshot() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        return;
    };
    let signal = std::env::var("FOURDA_CARRY_FREE").is_err();
    let db = Database::new(std::path::Path::new(&path)).expect("open snapshot");
    let before = db
        .get_items_balanced_by_source(FULL_WINDOW_HOURS, FULL_WINDOW_LIMIT / 5, FULL_WINDOW_LIMIT)
        .expect("window");
    let after = full_window_candidates_with(&db, signal, FULL_WINDOW_LIMIT, CURATED_CARRY_CAP)
        .expect("candidates")
        .items;
    let added: Vec<StoredSourceItem> = after[before.len()..].to_vec();
    println!(
        "tier={} window={} carried={} total={}",
        if signal { "signal" } else { "free" },
        before.len(),
        added.len(),
        after.len()
    );
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let ctx = rt
        .block_on(crate::scoring::build_scoring_context(&db))
        .expect("scoring context");
    let (mut window_results, window_ms) = score_live(&before, &ctx, &db);
    let (added_results, added_ms) = score_live(&added, &ctx, &db);
    println!(
        "score_item: window {:.0} ms ({:.1} ms/item), carried {:.0} ms ({:.1} ms/item)",
        window_ms,
        window_ms / before.len().max(1) as f64,
        added_ms,
        added_ms / added.len().max(1) as f64
    );
    let carried_relevant = added_results.iter().filter(|r| r.relevant).count();
    println!(
        "carried rows the scorer still judges relevant: {carried_relevant}/{}",
        added.len()
    );
    let mut all_results = window_results.clone();
    all_results.extend(added_results);
    let before_shown = shown_by_source(&db, &mut window_results);
    let after_shown = shown_by_source(&db, &mut all_results);
    let total = |v: &[(String, usize)]| v.iter().map(|(_, n)| n).sum::<usize>();
    println!("BEFORE shown={} {before_shown:?}", total(&before_shown));
    println!("AFTER  shown={} {after_shown:?}", total(&after_shown));
}
