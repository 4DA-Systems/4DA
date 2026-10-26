// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Named-package ranking (audit L8) on a real corpus.
//!
//! Run on a SNAPSHOT (opening it migrates it; never the live file), with the
//! app's embedding model available:
//!
//! ```text
//! FOURDA_SEARCH_LIVE=1 FOURDA_DB_PATH=<snapshot> FOURDA_DATA_DIR=<scratch> \
//!   cargo test --lib live_named_package_ranking -- --ignored --nocapture --test-threads=1
//! ```

const PACKAGE_QUERIES: &[&str] = &[
    "sharp vulnerability",
    "undici vulnerability",
    "rustls advisory",
    "lodash cve",
    "zod security",
];

const TOPIC_QUERIES: &[&str] = &[
    "rust async runtime",
    "sqlite vector search",
    // Names the installed `react`, but asks no security question.
    "react server components",
];

async fn print_top(q: &str) -> Vec<crate::natural_language_search::QueryResultItem> {
    let (result, _) = crate::natural_language_search::run_search(q.to_string(), true)
        .await
        .expect("search on snapshot");
    println!("\n=== {q} (semantic_pending={})", result.semantic_pending);
    for (i, item) in result.items.iter().take(10).enumerate() {
        println!(
            "{:>2}. {:>3.0}% {:<70.70} | {}",
            i + 1,
            item.relevance * 100.0,
            item.file_name.as_deref().unwrap_or(""),
            item.match_reason
        );
    }
    result.items
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs FOURDA_SEARCH_LIVE=1, FOURDA_DB_PATH=<snapshot> and the embedding model"]
async fn live_named_package_ranking() {
    if std::env::var("FOURDA_SEARCH_LIVE").is_err() || std::env::var("FOURDA_DB_PATH").is_err() {
        return;
    }
    for q in PACKAGE_QUERIES {
        let items = print_top(q).await;
        if items.iter().any(|i| i.affects_package.is_some()) {
            let first = items.iter().find(|i| !i.exact_match);
            assert!(
                first.is_some_and(|i| i.affects_package.is_some()),
                "{q}: the named package's own advisory must lead the non-exact results"
            );
        }
        let foreign_in_top10 = items
            .iter()
            .take(10)
            .filter(|i| i.foreign_ecosystem)
            .count();
        println!("   foreign-ecosystem advisories in top 10: {foreign_in_top10}");
    }
    for q in TOPIC_QUERIES {
        let items = print_top(q).await;
        assert!(
            items
                .iter()
                .all(|i| i.affects_package.is_none() && !i.foreign_ecosystem),
            "{q}: a topic query carries no package marks"
        );
    }
}
