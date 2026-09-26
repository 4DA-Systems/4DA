// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Target resolution and composition for intake enrichment. Fixtures are the
//! shapes stored in the live database on 2026-09-25.

use super::*;

/// A Reddit link post as the RSS adapter stores it (live item 107846).
const REDDIT_LINK_POST: &str = "&#32; submitted by &#32; <a href=\"https://www.reddit.com/user/grishavanika\"> /u/grishavanika </a> <br/> <span><a href=\"https://grishavanika.github.io/async_api.html\">[link]</a></span> &#32; <span><a href=\"https://www.reddit.com/r/programming/comments/1wlpi6i/c_asynchronous_api/\">[comments]</a></span>";

/// A Mastodon share: a mention, a hashtag and the article (live item 96243, abridged).
const MASTODON_SHARE: &str = "<p>Why I Ported Moonshine to JavaScript, by <a href=\"https://mas.to/@petewarden\" class=\"u-url mention\">@petewarden</a>:</p><p><a href=\"https://petewarden.com/2026/08/15/why-i-ported-moonshine-to-javascript/?ref=frontenddogma.com\" rel=\"nofollow noopener\">petewarden.com/2026/08/15/why-</a></p><p><a href=\"https://mas.to/tags/javascript\" class=\"mention hashtag\">#javascript</a></p>";

#[test]
fn a_reddit_link_post_resolves_to_its_article_not_its_thread() {
    assert_eq!(
        resolve_target(
            "reddit",
            "https://www.reddit.com/r/programming/comments/1wlpi6i/c_asynchronous_api/",
            REDDIT_LINK_POST
        ),
        Some(Target::Article(
            "https://grishavanika.github.io/async_api.html".into()
        ))
    );
}

#[test]
fn a_reddit_self_post_has_nothing_to_fetch() {
    let self_post = "<p>I have a question about lifetimes.</p> <a href=\"https://www.reddit.com/r/rust/comments/1/x/\">[comments]</a>";
    assert_eq!(
        resolve_target(
            "reddit",
            "https://www.reddit.com/r/rust/comments/1/x/",
            self_post
        ),
        None
    );
}

#[test]
fn a_mastodon_share_skips_mentions_and_hashtags() {
    assert_eq!(
        resolve_target("mastodon", "https://mas.to/@frontenddogma/117256934561413941", MASTODON_SHARE),
        Some(Target::Article(
            "https://petewarden.com/2026/08/15/why-i-ported-moonshine-to-javascript/?ref=frontenddogma.com".into()
        ))
    );
}

#[test]
fn a_mastodon_post_without_an_outbound_link_has_nothing_to_fetch() {
    let post = "<p>rust channels are pretty interesting <a href=\"https://mastodon.social/tags/rust\">#rust</a></p>";
    assert_eq!(
        resolve_target("mastodon", "https://mastodon.social/@a/1", post),
        None
    );
}

#[test]
fn a_devto_post_is_read_through_its_api() {
    assert_eq!(
        resolve_target(
            "devto",
            "https://dev.to/someone/stop-making-components-reusable-4k2j",
            "teaser"
        ),
        Some(Target::DevTo(
            "https://dev.to/api/articles/someone/stop-making-components-reusable-4k2j".into()
        ))
    );
    assert_eq!(
        resolve_target("devto", "https://dev.to/someone", "teaser"),
        None
    );
}

#[test]
fn a_story_link_is_fetched_but_a_discussion_page_is_not() {
    assert_eq!(
        resolve_target(
            "hackernews",
            "https://minimaxir.com/2026/09/rust-agents/",
            ""
        ),
        Some(Target::Article(
            "https://minimaxir.com/2026/09/rust-agents/".into()
        ))
    );
    assert_eq!(
        resolve_target("hackernews", "https://news.ycombinator.com/item?id=1", ""),
        None
    );
    assert_eq!(
        resolve_target("lobsters", "https://x.com/someone/status/1", ""),
        None
    );
}

#[test]
fn self_contained_sources_are_never_fetched() {
    for source in ["arxiv", "cve", "crates_io", "npm_registry", "stackoverflow"] {
        assert_eq!(
            resolve_target(source, "https://example.com/a", ""),
            None,
            "{source}"
        );
    }
}

#[test]
fn thinness_is_judged_on_visible_text_not_html_length() {
    assert!(
        is_thin(REDDIT_LINK_POST),
        "boilerplate and markup are not content"
    );
    assert!(is_thin(""));
    assert!(!is_thin(&"A sentence with real words in it. ".repeat(12)));
}

#[test]
fn a_social_post_keeps_its_words_ahead_of_the_article() {
    let composed = compose_content("mastodon", MASTODON_SHARE, "The article body.");
    assert!(composed.starts_with("Why I Ported Moonshine to JavaScript"));
    assert!(composed.ends_with("The article body."));
}

#[test]
fn other_items_are_replaced_by_the_article() {
    assert_eq!(
        compose_content("reddit", REDDIT_LINK_POST, "The article body."),
        "The article body."
    );
}

#[test]
fn stored_text_is_capped() {
    let long = "x".repeat(MAX_STORED_CHARS * 2);
    assert_eq!(
        compose_content("hackernews", "", &long).chars().count(),
        MAX_STORED_CHARS
    );
}

#[tokio::test]
async fn the_pacer_spaces_requests_to_the_same_host_only() {
    let pacer = HostPacer::default();
    let start = Instant::now();
    pacer.wait("a.example", Duration::from_millis(80)).await;
    pacer.wait("b.example", Duration::from_millis(80)).await;
    assert!(
        start.elapsed() < Duration::from_millis(60),
        "different hosts do not wait"
    );
    pacer.wait("a.example", Duration::from_millis(80)).await;
    assert!(
        start.elapsed() >= Duration::from_millis(75),
        "the same host waits its gap"
    );
}

#[tokio::test]
async fn a_pass_settles_items_that_need_no_fetch_without_touching_the_network() {
    let db = crate::test_utils::test_db();
    let long = "A sentence with real words in it. ".repeat(12);
    let rich = crate::test_utils::insert_test_item_with_url(
        &db,
        "hackernews",
        "1",
        "https://example.com/a",
        "T",
        &long,
    );
    let paper = crate::test_utils::insert_test_item_with_url(
        &db,
        "arxiv",
        "2",
        "https://arxiv.org/abs/1",
        "T",
        "",
    );

    let summary = enrich_thin_items(&db).await;
    assert_eq!(
        summary,
        EnrichmentSummary {
            examined: 2,
            ..Default::default()
        }
    );
    assert!(
        db.enrichment_candidates(48, 0, 10).unwrap().is_empty(),
        "both items are settled as skipped: {rich} {paper}"
    );
}

/// Runs one real enrichment pass against a snapshot of a live database, with
/// real network fetches. Never point it at the live file.
/// `FOURDA_VERIFY_DB=<snapshot> cargo test --lib live_snapshot_enrichment -- --ignored --nocapture`
#[tokio::test]
#[ignore = "requires FOURDA_VERIFY_DB pointing at a database snapshot, and network"]
async fn live_snapshot_enrichment_pass() {
    let Ok(path) = std::env::var("FOURDA_VERIFY_DB") else {
        return;
    };
    crate::register_sqlite_vec_extension();
    let db = Database::new(std::path::Path::new(&path)).expect("open snapshot");
    let started = Instant::now();
    let summary = enrich_thin_items(&db).await;
    println!("pass: {summary:?} in {:?}", started.elapsed());

    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT s.source_type, a.outcome, COUNT(*), CAST(AVG(LENGTH(s.content)) AS INTEGER)
             FROM enrichment_attempts a JOIN source_items s ON s.id = a.item_id
             GROUP BY 1, 2 ORDER BY 1, 2",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok(format!(
                "{:<16} {:<9} n={:<4} avg_content={}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?
            ))
        })
        .unwrap();
    for row in rows {
        println!("{}", row.unwrap());
    }
    let mut sample = conn
        .prepare(
            "SELECT s.source_type, s.title, SUBSTR(s.content, 1, 160), s.embedding_status
             FROM enrichment_attempts a JOIN source_items s ON s.id = a.item_id
             WHERE a.outcome = 'enriched' ORDER BY RANDOM() LIMIT 8",
        )
        .unwrap();
    let rows = sample
        .query_map([], |r| {
            Ok(format!(
                "[{}] {} ({})\n    {}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(2)?
            ))
        })
        .unwrap();
    for row in rows {
        println!("{}", row.unwrap());
    }
    assert!(summary.enriched > 0, "a live pass must enrich something");
}

#[test]
fn devto_front_matter_is_not_stored_as_text() {
    let md = "---\ntitle: Should You Still Learn to Code\npublished: true\n---\n\nEveryone says coding is dead.";
    assert_eq!(strip_front_matter(md), "Everyone says coding is dead.");
    assert_eq!(strip_front_matter("No header here."), "No header here.");
    assert_eq!(strip_front_matter("--- not closed"), "--- not closed");
}

#[test]
fn store_pages_shared_on_social_are_not_fetched() {
    let post =
        "<p>Get it now <a href=\"https://www.amazon.com/dp/B00ZBM34D2\">amazon.com/dp</a></p>";
    assert_eq!(
        resolve_target("mastodon", "https://m.example/@a/1", post),
        None
    );
}
