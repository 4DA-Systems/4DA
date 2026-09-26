// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Intake enrichment: fetch readable text for items that arrive thin.
//!
//! Most feeds deliver a title and little else: a Hacker News or Lobsters
//! story is a bare link, a dev.to entry carries a ~100-character teaser, and a
//! Reddit link post or a Mastodon share holds its real article only as an
//! `href`. On 2026-09-25, 9,402 of the items ingested in 14 days had under 100
//! characters of content (Hacker News: ~90% in every one of the last ten
//! weeks). Embeddings and LLM judges were scoring titles, and a person looking
//! at the same items could not tell what most of them were.
//!
//! This pass runs at the end of every background cache fill, which the
//! headless engine, the app's scheduled refresh and the post-analysis refresh
//! all share. It resolves each new thin item to the text worth reading, fetches
//! a bounded number per cycle, and stores the result. The stored item goes back
//! to `pending` embedding, so the next scoring pass re-embeds and re-scores it
//! on the real text. Attempts are recorded so a failing page is retried once,
//! not every cycle.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};
use tokio::sync::Mutex;

use crate::db::{Database, EnrichmentCandidate, EnrichmentOutcome};

/// Fetches started per cycle. The embedding repair loop re-embeds up to 100
/// pending items at the start of each scoring pass, so this stays well inside it.
const MAX_FETCHES_PER_CYCLE: usize = 60;
/// Recent unsettled items read per cycle. Most are settled as not thin without
/// a fetch, so the pool is several times the fetch budget.
const CANDIDATE_POOL: usize = 400;
/// Only items ingested this recently are enriched.
const WINDOW_HOURS: i64 = 48;
/// A failed fetch is retried once, no sooner than this.
const RETRY_AFTER_HOURS: i64 = 6;
/// Attempt bookkeeping older than this is dropped.
const PRUNE_AFTER_DAYS: i64 = 14;
const CONCURRENCY: usize = 4;
/// No new fetch starts after this, so enrichment never holds up scoring for long.
const CYCLE_DEADLINE: Duration = Duration::from_mins(2);
/// An item whose visible text is shorter than this is thin.
const THIN_BELOW_CHARS: usize = 300;
/// Fetched text shorter than this is treated as a failed extraction.
const MIN_FETCHED_CHARS: usize = 200;
const MAX_STORED_CHARS: usize = 5000;
/// Minimum spacing between two requests to the same host.
const SAME_HOST_GAP: Duration = Duration::from_millis(500);
/// dev.to's API answers bursts with 429, so its calls are spaced wider.
const API_HOST_GAP: Duration = Duration::from_millis(1100);

/// Sources whose items already carry their full text, or have no article.
const SELF_CONTAINED_SOURCES: &[&str] = &[
    "arxiv",
    "cve",
    "osv",
    "papers_with_code",
    "crates_io",
    "npm_registry",
    "pypi",
    "go_modules",
    "huggingface",
    "github",
    "youtube",
    "twitter",
    "stackoverflow", // the adapter requests the question body itself
];

/// Domains that block scraping or need a login, discussion pages with no
/// article, and store pages that hold a product listing rather than text
/// (the first live pass spent fetches on Amazon and Steam links shared on Mastodon).
const UNFETCHABLE_DOMAINS: &[&str] = &[
    "twitter.com",
    "x.com",
    "facebook.com",
    "instagram.com",
    "linkedin.com",
    "t.co",
    "youtube.com",
    "youtu.be",
    "reddit.com",
    "redd.it",
    "news.ycombinator.com",
    "amazon.com",
    "amazon.co.uk",
    "amazon.de",
    "store.steampowered.com",
    "apps.apple.com",
    "play.google.com",
    "open.spotify.com",
];

/// Where an item's readable text lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// A web page; text is extracted from its HTML.
    Article(String),
    /// A dev.to post, read through the public article API.
    DevTo(String),
}

/// What one enrichment pass did, for the cycle log.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EnrichmentSummary {
    pub examined: usize,
    pub fetched: usize,
    pub enriched: usize,
    pub failed: usize,
}

/// Enrich the newest thin items. Never fails: errors are logged and the
/// item is left for a later cycle.
pub(crate) async fn enrich_thin_items(db: &Database) -> EnrichmentSummary {
    let mut summary = EnrichmentSummary::default();
    if let Err(e) = db.prune_enrichment_attempts(PRUNE_AFTER_DAYS) {
        tracing::debug!(target: "4da::enrichment", error = %e, "Attempt pruning failed");
    }
    let candidates = match db.enrichment_candidates(WINDOW_HOURS, RETRY_AFTER_HOURS, CANDIDATE_POOL)
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "4da::enrichment", error = %e, "Failed to read enrichment candidates");
            return summary;
        }
    };

    let mut work = Vec::new();
    for c in candidates {
        summary.examined += 1;
        match (
            is_thin(&c.content),
            resolve_target(&c.source_type, &c.url, &c.content),
        ) {
            (true, Some(target)) if work.len() < MAX_FETCHES_PER_CYCLE => work.push((c, target)),
            // Over budget: leave it unrecorded for the next cycle.
            (true, Some(_)) => {}
            _ => record(db, c.id, EnrichmentOutcome::Skipped),
        }
    }

    let pacer = HostPacer::default();
    let deadline = Instant::now() + CYCLE_DEADLINE;
    let results: Vec<(EnrichmentCandidate, Option<String>)> = stream::iter(work)
        .map(|(c, target)| {
            let pacer = &pacer;
            async move {
                if Instant::now() >= deadline {
                    return None;
                }
                let text = fetch_text(&target, pacer).await;
                Some((c, text))
            }
        })
        .buffer_unordered(CONCURRENCY)
        .filter_map(|r| async move { r })
        .collect()
        .await;

    for (c, text) in results {
        summary.fetched += 1;
        match text {
            Some(article) => match store(db, &c, &article) {
                Ok(()) => {
                    summary.enriched += 1;
                    record(db, c.id, EnrichmentOutcome::Enriched);
                }
                Err(e) => {
                    summary.failed += 1;
                    tracing::warn!(target: "4da::enrichment", item_id = c.id, error = %e, "Failed to store enriched text");
                    record(db, c.id, EnrichmentOutcome::Failed);
                }
            },
            None => {
                summary.failed += 1;
                record(db, c.id, EnrichmentOutcome::Failed);
            }
        }
    }

    if summary.fetched > 0 {
        tracing::info!(
            target: "4da::enrichment",
            examined = summary.examined,
            fetched = summary.fetched,
            enriched = summary.enriched,
            failed = summary.failed,
            "Intake enrichment complete"
        );
    }
    summary
}

fn record(db: &Database, id: i64, outcome: EnrichmentOutcome) {
    if let Err(e) = db.record_enrichment_attempt(id, outcome) {
        tracing::debug!(target: "4da::enrichment", item_id = id, error = %e, "Failed to record attempt");
    }
}

/// Whether an item's visible text is too short to judge it by.
pub(crate) fn is_thin(raw_content: &str) -> bool {
    crate::utils::preprocess_content(raw_content)
        .chars()
        .count()
        < THIN_BELOW_CHARS
}

/// Find where an item's readable text lives, or `None` when there is nothing
/// worth fetching.
pub(crate) fn resolve_target(source_type: &str, url: &str, raw_content: &str) -> Option<Target> {
    if SELF_CONTAINED_SOURCES.contains(&source_type) {
        return None;
    }
    let target = match source_type {
        "devto" => return devto_api_url(url).map(Target::DevTo),
        // A Reddit item's URL is its own thread; a link post's article is the `[link]` anchor.
        "reddit" => reddit_link(raw_content)?,
        // A social post links out from its body; its own URL is the post.
        "mastodon" | "bluesky" => first_external_link(raw_content, host_of(url)?)?,
        _ => url.to_string(),
    };
    is_fetchable(&target).then_some(Target::Article(target))
}

fn host_of(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let host = rest.split(['/', '?', '#']).next()?;
    (!host.is_empty()).then_some(host)
}

fn is_fetchable(url: &str) -> bool {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }
    let Some(host) = host_of(url) else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    !UNFETCHABLE_DOMAINS
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Every `href` value in an HTML fragment, entity-decoded.
fn hrefs(html: &str) -> impl Iterator<Item = String> + '_ {
    html.split("href=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(crate::utils::decode_html_entities)
}

/// The article behind a Reddit link post: the anchor whose text is `[link]`.
fn reddit_link(raw_content: &str) -> Option<String> {
    raw_content
        .split("\">[link]")
        .next()
        .filter(|before| before.len() < raw_content.len())
        .and_then(|before| before.rsplit("href=\"").next())
        .map(crate::utils::decode_html_entities)
        .filter(|u| u.starts_with("http"))
}

/// The first link in a social post that leaves the post's own instance and is
/// not a mention or a hashtag.
fn first_external_link(raw_content: &str, post_host: &str) -> Option<String> {
    hrefs(raw_content).find(|h| {
        let Some(host) = host_of(h) else {
            return false;
        };
        let path = h.split_once(host).map_or("", |(_, p)| p);
        host != post_host
            && !host.ends_with("bsky.app")
            && !path.contains("/tags/")
            && !path.starts_with("/@")
            && !path.starts_with("/u/")
            && !path.starts_with("/users/")
            && !path.starts_with("/profile/")
    })
}

/// `https://dev.to/{user}/{slug}` → the article API URL for that post.
fn devto_api_url(url: &str) -> Option<String> {
    if host_of(url)? != "dev.to" {
        return None;
    }
    let path = url.split_once("dev.to/")?.1;
    let mut parts = path
        .split(['?', '#'])
        .next()?
        .split('/')
        .filter(|p| !p.is_empty());
    let (user, slug) = (parts.next()?, parts.next()?);
    parts
        .next()
        .is_none()
        .then(|| format!("https://dev.to/api/articles/{user}/{slug}"))
}

/// Spaces requests to the same host so one busy site is not hammered.
#[derive(Default)]
struct HostPacer {
    last: Mutex<HashMap<String, Instant>>,
}

impl HostPacer {
    async fn wait(&self, host: &str, gap: Duration) {
        let sleep_for = {
            let mut last = self.last.lock().await;
            let now = Instant::now();
            let next = last.get(host).map_or(now, |t| (*t + gap).max(now));
            last.insert(host.to_string(), next);
            next - now
        };
        if !sleep_for.is_zero() {
            tokio::time::sleep(sleep_for).await;
        }
    }
}

async fn fetch_text(target: &Target, pacer: &HostPacer) -> Option<String> {
    let text = match target {
        Target::Article(url) => {
            pacer.wait(host_of(url)?, SAME_HOST_GAP).await;
            crate::utils::scrape_article_content(url).await?
        }
        Target::DevTo(api_url) => {
            pacer.wait("dev.to", API_HOST_GAP).await;
            fetch_devto_body(api_url).await?
        }
    };
    let text = crate::utils::html_to_text(&text, MAX_STORED_CHARS);
    (text.chars().count() >= MIN_FETCHED_CHARS).then_some(text)
}

async fn fetch_devto_body(api_url: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Article {
        body_markdown: Option<String>,
    }
    let resp = crate::sources::shared_client()
        .get(api_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        tracing::debug!(target: "4da::enrichment", url = api_url, status = %resp.status(), "dev.to article API refused");
        return None;
    }
    let body = resp.json::<Article>().await.ok()?.body_markdown?;
    Some(strip_front_matter(&body).to_string())
}

/// dev.to returns posts written with a YAML header (`---\ntitle: ...\n---`)
/// verbatim; the header is metadata, not text.
pub(crate) fn strip_front_matter(markdown: &str) -> &str {
    markdown
        .trim_start()
        .strip_prefix("---")
        .and_then(|rest| rest.split_once("\n---"))
        .map_or(markdown, |(_, body)| {
            body.trim_start_matches('-').trim_start()
        })
}

/// The text to store: a social post keeps its own words ahead of the article
/// it shares; any other item's thin content is replaced by the article.
pub(crate) fn compose_content(source_type: &str, raw_content: &str, article: &str) -> String {
    let post = crate::utils::html_to_text(raw_content, 600);
    let keep_post =
        matches!(source_type, "mastodon" | "bluesky" | "lemmy") && post.chars().count() >= 40;
    let combined = if keep_post {
        format!("{post}\n\n{article}")
    } else {
        article.to_string()
    };
    crate::utils::truncate_utf8(&combined, MAX_STORED_CHARS)
}

fn store(db: &Database, c: &EnrichmentCandidate, article: &str) -> rusqlite::Result<()> {
    let content = compose_content(&c.source_type, &c.content, article);
    let content_hash = crate::db::hash_content_parts(&[&c.title, &content]);
    let embed_text = crate::utils::build_embedding_text(&c.title, &content);
    db.update_enriched_content(c.id, &content, &content_hash, &embed_text)
}

#[cfg(test)]
#[path = "content_enrichment_tests.rs"]
mod tests;
