//! Live Reality Check — Validates scoring pipeline against real HN content
//!
//! Fetches top stories from Hacker News and scores them against 3 personas
//! to verify the pipeline produces sane results with real-world data.
//!
//! Marked `#[ignore]` — run on demand: `cargo test live_reality -- --ignored --nocapture`

use tracing::info;

use super::personas::{fullstack_typescript, python_ml_engineer, rust_systems_dev};
use super::{sim_db, sim_input, sim_no_freshness};
use crate::scoring::pipeline::score_item;

#[derive(serde::Deserialize)]
struct HNItem {
    id: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(rename = "type", default)]
    item_type: Option<String>,
}

struct FetchedStory {
    id: u64,
    title: String,
    url: Option<String>,
}

async fn fetch_top_stories(count: usize) -> Vec<FetchedStory> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("HTTP client");

    let ids: Vec<u64> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await
        .expect("fetch top story IDs")
        .json()
        .await
        .expect("parse top story IDs");

    let mut stories = Vec::with_capacity(count);
    for &id in ids.iter().take(count + 10) {
        if stories.len() >= count {
            break;
        }
        let url = format!("https://hacker-news.firebaseio.com/v0/item/{id}.json");
        let resp = client.get(&url).send().await;
        let item: Option<HNItem> = match resp {
            Ok(r) => r.json().await.ok(),
            Err(_) => continue,
        };
        if let Some(item) = item {
            // Skip non-story items (job posts, polls, etc.)
            if item.item_type.as_deref() == Some("job") {
                continue;
            }
            if item.title.is_empty() {
                continue;
            }
            stories.push(FetchedStory {
                id: item.id,
                title: item.title,
                url: item.url,
            });
        }
    }
    stories
}

#[tokio::test]
#[ignore] // network-dependent — run with: cargo test live_reality -- --ignored --nocapture
async fn live_reality_check_hn_top_stories() {
    let stories = fetch_top_stories(30).await;
    assert!(
        stories.len() >= 10,
        "Expected at least 10 stories from HN, got {}",
        stories.len()
    );

    let db = sim_db();
    let opts = sim_no_freshness();
    let zero_emb = vec![0.0f32; 384];

    let personas = vec![
        ("rust_systems", rust_systems_dev()),
        ("python_ml", python_ml_engineer()),
        ("fullstack_ts", fullstack_typescript()),
    ];

    for (persona_name, ctx) in &personas {
        let mut scores: Vec<(f32, &str, bool)> = Vec::with_capacity(stories.len());

        for story in &stories {
            let content = story.url.as_deref().unwrap_or("");
            let input = sim_input(story.id, &story.title, content, &zero_emb);
            let result = score_item(&input, ctx, &db, &opts, None);
            scores.push((result.top_score, &story.title, result.relevant));
        }

        // Sort descending by score
        scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // --- Print top 5 for human review (visible with --nocapture) ---
        info!("\n=== Persona: {persona_name} ===");
        for (i, (score, title, relevant)) in scores.iter().take(5).enumerate() {
            let tag = if *relevant { "REL" } else { "---" };
            info!("  #{}: {:.4} [{tag}] {title}", i + 1, score);
        }

        // --- Validation: generous thresholds ---
        let top_score = scores[0].0;
        let min_score = scores.last().map(|s| s.0).unwrap_or(0.0);
        let spread = top_score - min_score;
        let relevant_count = scores.iter().filter(|s| s.2).count();

        assert!(
            top_score > 0.1,
            "[{persona_name}] Top score {top_score:.4} <= 0.1 — pipeline may be broken"
        );
        assert!(
            spread > 0.01,
            "[{persona_name}] Score spread {spread:.4} <= 0.01 — pipeline may be short-circuiting"
        );
        assert!(
            relevant_count < stories.len(),
            "[{persona_name}] All {relevant_count} items marked relevant — noise rejection broken"
        );

        info!(
            "  top={top_score:.4} spread={spread:.4} relevant={relevant_count}/{} -- OK",
            stories.len()
        );
    }
}
