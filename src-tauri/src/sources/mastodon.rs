// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Mastodon source — the open-protocol substitute for X/Twitter.
//!
//! Mastodon is ActivityPub/federated, like Lemmy: thousands of independent, self-hostable instances,
//! no single owner that can kill the free API or price it to $42k/mo the way X did. The technical dev
//! audience that fled X largely landed on Mastodon (hachyderm.io, fosstodon.org), so it is the natural
//! X hedge in 4DA's source-resilience model (see [`super::access`]).
//!
//! The FEDERATED public timeline is noise (all topics, all instances, bots), so — exactly like Reddit
//! reads dev subreddits — 4DA reads dev-relevant TAG timelines (`#rust`, `#programming`, …) from a
//! tech-focused instance (`hachyderm.io`). Each tag is reachable two ways, tried in order:
//! `mastodon:api` (`/api/v1/timelines/tag/<tag>`) then `mastodon:rss` (`/tags/<tag>.rss`).

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{debug, info, warn};

use super::access::{resilient_fetch, AccessStrategy};
use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

/// Tech/dev-focused Mastodon instance whose federation reach covers the developer fediverse.
const MASTODON_INSTANCE: &str = "hachyderm.io";
const MASTODON_USER_AGENT: &str = "4DA/1.0 (+https://4da.ai)";

/// Dev-relevant hashtags — the fediverse equivalent of dev subreddits.
const DEV_TAGS: &[&str] = &[
    "rust",
    "programming",
    "javascript",
    "python",
    "golang",
    "webdev",
    "linux",
    "devops",
    "opensource",
    "security",
];

// ============================================================================
// Mastodon API types
// ============================================================================

#[derive(Debug, Deserialize)]
struct MastodonStatus {
    uri: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    content: String,
    #[serde(default)]
    account: Option<MastodonAccount>,
    #[serde(default)]
    favourites_count: i64,
    #[serde(default)]
    reblogs_count: i64,
    #[serde(default)]
    replies_count: i64,
    #[serde(default)]
    tags: Vec<MastodonTag>,
    /// Present (non-null) when this status is a pure boost of another — skipped to avoid duplicates.
    #[serde(default)]
    reblog: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct MastodonAccount {
    #[serde(default)]
    acct: String,
}

#[derive(Debug, Deserialize)]
struct MastodonTag {
    name: String,
}

/// Mastodon posts have no title (microblog) — derive a clean, short one from the HTML body: strip
/// tags, decode the common entities, collapse whitespace, truncate at a word boundary.
///
/// Titles are the app's densest display surface, so three microblog artifacts
/// are scrubbed (live corpus 2026-07-17: 3,208/20,938 items had a URL glued
/// into the title, 7,818 carried hashtags):
/// - tag boundaries become word boundaries — anchor/br removal used to weld
///   adjacent runs together ("editionshttps://…", "clienthttps://…")
/// - URL text is dropped — the link already lives in `item.url`; inside a
///   title it is pure noise. This covers scheme tokens ("https://…"),
///   scheme-less displayed link text ("news.ycombinator.com/item?id=4" — a
///   dot-TLD host followed by '/'; "node.js" and "TCP/IP" survive), the
///   tail fragments Mastodon's ellipsis/invisible spans sever off a long URL
///   ("8895199", "ns/"), and a now-orphaned trailing label ("Comments:")
///   whose URL was just dropped
/// - a trailing run of 2+ hashtags (poster metadata: "… #HackerNews #Tech
///   #Rust") is dropped; an inline/grammatical hashtag keeps its word with the
///   '#' stripped ("#Microsoft releases…" → "Microsoft releases…")
///
/// When the body mentions a CVE/GHSA id, that id is hoisted to the front so a
/// rambling toot ("Saturday, but self hosting, so here we go. …CVE-2026-49975…")
/// leads with the newsworthy token instead of conversational preamble. This also
/// lets the downstream content classifier see the id and tag the item as a
/// security advisory (it classifies off the title).
///
/// `pub(crate)`: the Phase 93 migration re-derives stored titles with this
/// same function so historical rows heal identically to fresh ingest.
pub(crate) fn derive_title(html: &str) -> String {
    // Strip tags, capturing each tag's text so `<span class="invisible">`
    // content is skipped entirely. Mastodon wraps the parts of a long URL it
    // does NOT display (the "https://" prefix and the severed tail after the
    // ellipsis span) in invisible spans; keeping that text planted mid-word
    // fragments no downstream heuristic can tell from prose — live corpus
    // 2026-07-27: "…tip jar are ay Code for", where "ay" is the invisible
    // tail of "sketch-a-day". What the reader can't see, the title must not
    // contain.
    let mut text = String::with_capacity(html.len());
    let mut tag = String::new();
    let mut in_tag = false;
    let mut invisible = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
                // A tag boundary is a word boundary — never weld adjacent runs.
                text.push(' ');
            }
            '>' => {
                in_tag = false;
                let t = tag.to_ascii_lowercase();
                if t.starts_with("span") && t.contains("invisible") {
                    invisible = true;
                } else if invisible && t.starts_with("/span") {
                    invisible = false;
                }
            }
            _ if in_tag => tag.push(c),
            _ if !invisible => text.push(c),
            _ => {}
        }
    }
    let decoded = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ");

    // Mastodon renders hashtags and mentions as `#<span>tag</span>` /
    // `@<span>user</span>` — the tag-boundary space above splits those into a
    // lone sigil followed by the word. Re-join them so the hashtag rules see
    // "#tag" and mentions read "@user" instead of "( @ user )".
    let raw_words: Vec<&str> = decoded.split_whitespace().collect();
    let mut joined: Vec<String> = Vec::with_capacity(raw_words.len());
    let mut i = 0;
    while i < raw_words.len() {
        if (raw_words[i] == "#" || raw_words[i] == "@") && i + 1 < raw_words.len() {
            joined.push(format!("{}{}", raw_words[i], raw_words[i + 1]));
            i += 2;
        } else {
            joined.push(raw_words[i].to_string());
            i += 1;
        }
    }

    // URL scrub with drop-tracking. Mastodon splits a long displayed URL
    // across ellipsis/invisible spans ("https://" · "news.ycombinator.com/
    // item?id=4" · "8895199"), so the scheme token, the scheme-less visible
    // part, and the severed tail must all go — and a label the URL hung off
    // ("Comments:") is dropped when its URL is.
    let mut kept: Vec<&str> = Vec::with_capacity(joined.len());
    let mut prev_dropped = false;
    for w in &joined {
        let dropped = w.contains("://") || is_bare_url(w) || (prev_dropped && is_url_tail(w));
        if dropped {
            // First drop of a run orphans a preceding "Label:" — take it too.
            if !prev_dropped {
                if let Some(last) = kept.last() {
                    if last.ends_with(':') && last.len() > 1 {
                        kept.pop();
                    }
                }
            }
            prev_dropped = true;
            continue;
        }
        prev_dropped = false;
        kept.push(w.as_str());
    }
    let mut words = kept;

    // Drop a trailing hashtag RUN (2+ = poster metadata). A single trailing
    // hashtag is usually grammatical ("… released as #opensource") and is
    // handled by the inline rule below instead.
    let trailing = words.iter().rev().take_while(|w| is_hashtag(w)).count();
    if trailing >= 2 {
        words.truncate(words.len() - trailing);
    }

    // Inline hashtags keep their word, minus the '#'.
    let collapsed = words
        .iter()
        .map(|w| if is_hashtag(w) { &w[1..] } else { w })
        .collect::<Vec<_>>()
        .join(" ");

    // Hoist a security id to the front when it isn't already near the start.
    if let Some(id) = first_security_id(&collapsed) {
        let head: String = collapsed.chars().take(id.chars().count() + 4).collect();
        if !head.contains(&id) {
            return format!("{id} — {}", truncate_at_word(&collapsed, 96));
        }
    }

    truncate_at_word(&collapsed, 120)
}

/// Scheme-less displayed link text: a host with a dot + alphabetic TLD
/// followed by a '/' path ("mort.coffee/home/x", "news.ycombinator.com/item?id=4").
/// "node.js" / "socket.io" (no slash) and "TCP/IP" (no dot-TLD host) survive.
fn is_bare_url(w: &str) -> bool {
    let Some(slash) = w.find('/') else {
        return false;
    };
    let host = &w[..slash];
    let Some(dot) = host.rfind('.') else {
        return false;
    };
    let tld = &host[dot + 1..];
    dot > 0 && !tld.is_empty() && tld.len() <= 24 && tld.chars().all(|c| c.is_ascii_alphabetic())
}

/// A URL tail severed by Mastodon's ellipsis/invisible spans ("8895199",
/// "ns/", "8iK3NByz1pVVba4kGmycDs"): URL-safe characters with at least one
/// digit or path/query character. Only meaningful right after a dropped URL
/// token — plain words ("rocks") never match, so grammar is safe.
fn is_url_tail(w: &str) -> bool {
    !w.is_empty()
        && w.chars()
            .all(|c| c.is_ascii_alphanumeric() || "/?=&._%-".contains(c))
        && w.chars().any(|c| c.is_ascii_digit() || "/?=&%".contains(c))
}

/// A '#'-led token whose body is word-like (letters/digits/_/-), e.g. a
/// Mastodon hashtag. "C#" or a lone "#" are not hashtags.
fn is_hashtag(w: &str) -> bool {
    w.len() > 1
        && w.starts_with('#')
        && w[1..]
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Truncate `text` to at most `max` chars at a word boundary, appending an
/// ellipsis when shortened.
fn truncate_at_word(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max).collect();
    match truncated.rfind(' ') {
        Some(i) => format!("{}…", &truncated[..i]),
        None => format!("{truncated}…"),
    }
}

/// Find the first CVE-YYYY-NNNN(+) or GHSA-xxxx-xxxx-xxxx id in `text`
/// (uppercase form, the canonical convention). Returns the id verbatim.
fn first_security_id(text: &str) -> Option<String> {
    for key in ["CVE-", "GHSA-"] {
        if let Some(pos) = text.find(key) {
            // `text.find` returns a byte offset valid in `text`, so this slice is safe.
            let token: String = text[pos..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            let token = token.trim_end_matches('-');
            let has_digit = token.chars().any(|c| c.is_ascii_digit());
            let hyphens = token.matches('-').count();
            // CVE-2026-49975 (has digits) or GHSA-xxxx-xxxx-xxxx (3 hyphens).
            if token.len() >= 8 && (has_digit || hyphens >= 3) {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn status_to_item(status: MastodonStatus) -> Option<SourceItem> {
    if status.reblog.is_some() {
        return None; // pure boost — the original will appear on its own
    }
    let title = derive_title(&status.content);
    if title.is_empty() {
        return None;
    }
    let link = status.url.clone().unwrap_or_else(|| status.uri.clone());
    let author = status.account.as_ref().map(|a| a.acct.as_str());
    let tags: Vec<&str> = status.tags.iter().map(|t| t.name.as_str()).collect();
    Some(
        SourceItem::new("mastodon", &status.uri, &title)
            .with_url(Some(link))
            .with_content(status.content.clone())
            .with_metadata(serde_json::json!({
                "author": author,
                "tags": tags,
                "score": status.favourites_count + status.reblogs_count,
                "comments": status.replies_count,
                "is_self": true,
                "via": "api",
            })),
    )
}

// ============================================================================
// Per-tag fetchers (one per access path)
// ============================================================================

async fn fetch_tag_api(
    client: &reqwest::Client,
    instance: &str,
    tag: &str,
    limit: usize,
) -> SourceResult<Vec<SourceItem>> {
    let url = format!(
        "https://{instance}/api/v1/timelines/tag/{tag}?limit={}",
        limit.clamp(1, 40)
    );
    let response = client
        .get(&url)
        .header("User-Agent", MASTODON_USER_AGENT)
        .send()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SourceError::RateLimited(
            "Mastodon rate limited (HTTP 429)".to_string(),
        ));
    }
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(SourceError::Forbidden(
            "Mastodon requires auth (HTTP 401/403)".to_string(),
        ));
    }
    super::check_http_status(status, "Mastodon API")?;

    let statuses: Vec<MastodonStatus> = response
        .json()
        .await
        .map_err(|e| SourceError::Parse(e.to_string()))?;
    Ok(statuses.into_iter().filter_map(status_to_item).collect())
}

async fn fetch_tag_rss(
    client: &reqwest::Client,
    instance: &str,
    tag: &str,
    limit: usize,
) -> SourceResult<Vec<SourceItem>> {
    let url = format!("https://{instance}/tags/{tag}.rss");
    let response = client
        .get(&url)
        .header("User-Agent", MASTODON_USER_AGENT)
        .send()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SourceError::RateLimited(
            "Mastodon RSS rate limited (HTTP 429)".to_string(),
        ));
    }
    super::check_http_status(status, "Mastodon RSS")?;

    let body = response
        .text()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;
    Ok(parse_mastodon_rss(&body, tag, limit))
}

/// Parse a Mastodon tag RSS 2.0 feed. Mastodon already derives a `<title>` from the post body.
fn parse_mastodon_rss(xml: &str, tag: &str, limit: usize) -> Vec<SourceItem> {
    xml.split("<item>")
        .skip(1)
        .take(limit)
        .filter_map(|block| {
            let item = &block[..block.find("</item>").unwrap_or(block.len())];
            let link =
                super::extract_tag(item, "link").or_else(|| super::extract_tag(item, "guid"))?;
            let title = super::extract_tag(item, "title")
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| {
                    derive_title(&super::extract_tag(item, "description").unwrap_or_default())
                });
            if title.is_empty() {
                return None;
            }
            let content = super::extract_tag(item, "description").unwrap_or_default();
            let id = super::extract_tag(item, "guid").unwrap_or_else(|| link.clone());
            Some(
                SourceItem::new("mastodon", &id, &title)
                    .with_url(Some(link))
                    .with_content(content)
                    .with_metadata(serde_json::json!({
                        "tags": [tag],
                        "is_self": true,
                        "via": "rss",
                    })),
            )
        })
        .collect()
}

// ============================================================================
// Access strategies
// ============================================================================

#[derive(Clone, Copy)]
enum AccessPath {
    Api,
    Rss,
}

/// Walk the dev tag set via one access path, aggregating into a ranked batch. Early-bails on the
/// first `Forbidden`/`RateLimited` — a 401/403/429 is a whole-instance condition, not tag-specific,
/// so hammering the remaining tags only burns the instance's budget (the lesson the live Reddit test
/// taught us). A reachable-but-empty set returns `Ok(empty)`.
async fn fetch_via(
    client: &reqwest::Client,
    instance: &str,
    tags: &[&str],
    max_items: usize,
    path: AccessPath,
) -> SourceResult<Vec<SourceItem>> {
    let per_tag = (max_items / tags.len().max(1)).max(3);
    let mut all = Vec::new();
    let mut errors = Vec::new();
    let mut any_ok = false;

    for tag in tags {
        let result = match path {
            AccessPath::Api => fetch_tag_api(client, instance, tag, per_tag).await,
            AccessPath::Rss => fetch_tag_rss(client, instance, tag, per_tag).await,
        };
        match result {
            Ok(items) => {
                any_ok = true;
                all.extend(items);
            }
            Err(e) => {
                let whole_instance_block =
                    matches!(e, SourceError::Forbidden(_) | SourceError::RateLimited(_));
                match &e {
                    SourceError::Forbidden(_) | SourceError::RateLimited(_) => {
                        debug!(tag, error = %e, "Skipped Mastodon tag (auth/rate-limit)");
                    }
                    _ => warn!(tag, error = %e, "Failed to fetch Mastodon tag"),
                }
                errors.push(e);
                if whole_instance_block {
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    if all.is_empty() {
        if errors.is_empty() && any_ok {
            return Ok(Vec::new());
        }
        return Err(errors
            .into_iter()
            .max_by_key(super::access::actionability)
            .unwrap_or_else(|| SourceError::Other("mastodon: no tags configured".to_string())));
    }

    all.sort_by_key(|b| std::cmp::Reverse(item_score(b)));
    all.truncate(max_items);
    Ok(all)
}

fn item_score(item: &SourceItem) -> i64 {
    item.metadata
        .as_ref()
        .and_then(|m| m.get("score"))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0)
}

struct MastodonStrategy {
    client: reqwest::Client,
    instance: &'static str,
    tags: Vec<&'static str>,
    max_items: usize,
    path: AccessPath,
}

#[async_trait]
impl AccessStrategy for MastodonStrategy {
    fn label(&self) -> &str {
        match self.path {
            AccessPath::Api => "mastodon:api",
            AccessPath::Rss => "mastodon:rss",
        }
    }

    async fn fetch(&self) -> SourceResult<Vec<SourceItem>> {
        fetch_via(
            &self.client,
            self.instance,
            &self.tags,
            self.max_items,
            self.path,
        )
        .await
    }
}

// ============================================================================
// Mastodon Source
// ============================================================================

/// Mastodon source — federated open-protocol developer microblog (X substitute).
pub struct MastodonSource {
    config: SourceConfig,
    client: reqwest::Client,
    instance: &'static str,
    tags: Vec<&'static str>,
}

impl MastodonSource {
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 40,
                fetch_interval_secs: 600,
                custom: None,
            },
            client: super::shared_client(),
            instance: MASTODON_INSTANCE,
            tags: DEV_TAGS.to_vec(),
        }
    }

    fn strategies(&self) -> Vec<Box<dyn AccessStrategy>> {
        vec![
            Box::new(MastodonStrategy {
                client: self.client.clone(),
                instance: self.instance,
                tags: self.tags.clone(),
                max_items: self.config.max_items,
                path: AccessPath::Api,
            }),
            Box::new(MastodonStrategy {
                client: self.client.clone(),
                instance: self.instance,
                tags: self.tags.clone(),
                max_items: self.config.max_items,
                path: AccessPath::Rss,
            }),
        ]
    }
}

impl Default for MastodonSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for MastodonSource {
    fn source_type(&self) -> &'static str {
        "mastodon"
    }

    fn name(&self) -> &'static str {
        "Mastodon"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::Community,
            default_content_type: "discussion",
            default_multiplier: 1.0,
            label: "Mastodon",
            color_hint: "purple",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }
        info!(
            instance = self.instance,
            tags = self.tags.len(),
            "Fetching Mastodon dev tags"
        );
        resilient_fetch("mastodon", &self.strategies()).await
    }

    async fn fetch_items_deep(&self, items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }
        let mut deep = MastodonSource::new();
        deep.config.max_items = (items_per_category * self.tags.len()).clamp(1, 400);
        resilient_fetch("mastodon", &deep.strategies()).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_clean_title_from_html() {
        assert_eq!(
            derive_title("<p>Could this <em>kill</em> the project?</p>"),
            "Could this kill the project?"
        );
        assert_eq!(derive_title("<p>a &amp; b &lt; c</p>"), "a & b < c");
        let long = format!("<p>{}</p>", "word ".repeat(60));
        assert!(
            derive_title(&long).ends_with('…'),
            "long titles truncate at a word boundary"
        );
        assert_eq!(derive_title("<p></p>"), "");
    }

    #[test]
    fn tag_boundaries_are_word_boundaries() {
        // Live-corpus regression (2026-07-17): anchor/br removal welded runs
        // together — "editionshttps://mort.coffee/…", "clienthttps://github…".
        assert_eq!(
            derive_title(
                "<p>SQLite should have (Rust-style) editions<br><a href=\"https://mort.coffee/x\">https://mort.coffee/home/sqlite-editions/</a></p>"
            ),
            "SQLite should have (Rust-style) editions"
        );
    }

    #[test]
    fn drops_url_tokens_from_title() {
        // The link already lives in item.url — inside a title it is noise.
        // "RE:" is an orphaned label once its URL goes, so it goes too.
        assert_eq!(
            derive_title("<p>RE: <a href=\"x\">https://mastodon.social/@arstechnica/1169</a>It feels great to see this</p>"),
            "It feels great to see this"
        );
        // A URL-only toot derives an empty title (caller drops the item).
        assert_eq!(
            derive_title("<p><a href=\"x\">https://example.com/post</a></p>"),
            ""
        );
    }

    #[test]
    fn drops_schemeless_urls_severed_tails_and_orphaned_labels() {
        // Live-corpus residue class (2026-07-17): Mastodon splits a long URL
        // across invisible/ellipsis spans — scheme token, visible scheme-less
        // part, and severed tail all become separate words. All must go, and
        // the "Comments:" label that hung off the URL goes with it.
        assert_eq!(
            derive_title(
                "<p>SQLite should have (Rust-style) editions <a>mort.coffee/home/sqlite-editio</a><span>ns/</span> Comments: <a><span>https://</span>news.ycombinator.com/item?id=4</a><span>8895199</span></p>"
            ),
            "SQLite should have (Rust-style) editions"
        );
        assert_eq!(
            derive_title("<p>Parsing SGF files for fun <a>video.infosec.exchange/w/8iK3NByz1pVVba4kGmycDs</a></p>"),
            "Parsing SGF files for fun"
        );
        // Dev vocabulary is NOT a URL: no slash after the dot-TLD…
        assert_eq!(
            derive_title("<p>why node.js beats socket.io here</p>"),
            "why node.js beats socket.io here"
        );
        // …and no dot-TLD host before the slash.
        assert_eq!(
            derive_title("<p>TCP/IP stack rewrite, 1.80/1.81 diff</p>"),
            "TCP/IP stack rewrite, 1.80/1.81 diff"
        );
        // A plain word after a dropped URL is grammar, not a tail.
        assert_eq!(
            derive_title("<p>see <a>https://example.com/docs</a> rocks anyway</p>"),
            "see rocks anyway"
        );
    }

    #[test]
    fn invisible_span_content_never_reaches_the_title() {
        // Exact live-corpus HTML (2026-07-27, pynews.com.br/@villares): the
        // invisible tail of "sketch-a-day" is purely alphabetic ("ay"), so the
        // is_url_tail digit/path heuristic can NOT catch it — it must die at
        // the span level. Old output: "The sketch-a-day archives and tip jar
        // are ay Code for".
        let live = "<p>The sketch-a-day archives and tip jar are at: <a href=\"https://abav.lugaralgum.com/sketch-a-day\" rel=\"nofollow noopener\" translate=\"no\" target=\"_blank\"><span class=\"invisible\">https://</span><span class=\"ellipsis\">abav.lugaralgum.com/sketch-a-d</span><span class=\"invisible\">ay</span></a> Code for this: <a href=\"https://github.com/villares/sketch-a-day/tree/main/2026/sketch_2026_06_10\" rel=\"nofollow noopener\" translate=\"no\" target=\"_blank\"><span class=\"invisible\">https://</span><span class=\"ellipsis\">github.com/villares/sketch-a-d</span><span class=\"invisible\">ay/tree/main/2026/sketch_2026_06_10</span></a> <a href=\"https://pynews.com.br/tags/Processing\" class=\"mention hashtag\" rel=\"nofollow noopener\" target=\"_blank\">#<span>Processing</span></a> <a href=\"https://pynews.com.br/tags/py5\" class=\"mention hashtag\" rel=\"nofollow noopener\" target=\"_blank\">#<span>py5</span></a></p>";
        let title = derive_title(live);
        assert!(
            !title.contains(" ay ") && !title.ends_with(" ay"),
            "invisible URL tail leaked into title: {title}"
        );
        assert!(
            title.starts_with("The sketch-a-day archives and tip jar"),
            "prose head lost: {title}"
        );
        assert!(!title.contains("https"), "scheme leaked: {title}");

        // The invisible flag must reset at the span close — prose after a
        // link stays visible.
        assert_eq!(
            derive_title(
                "<p>Ship it <a><span class=\"invisible\">https://</span><span class=\"ellipsis\">example.com/x</span><span class=\"invisible\">yz</span></a> today</p>"
            ),
            "Ship it today"
        );
    }

    #[test]
    fn hashtags_trailing_run_dropped_inline_kept() {
        // Trailing run of 2+ = poster metadata — dropped.
        assert_eq!(
            derive_title("<p>Here's how I host my own AIM server <a>#SelfHosting</a> <a>#Networking</a> <a>#Retro</a></p>"),
            "Here's how I host my own AIM server"
        );
        // Inline hashtags are grammar — keep the word, strip the '#'. A SINGLE
        // trailing hashtag is treated as grammar too ("released as #opensource").
        assert_eq!(
            derive_title("<p><a>#Microsoft</a> releases its weird '90s <a>#IRC</a> client as <a>#opensource</a></p>"),
            "Microsoft releases its weird '90s IRC client as opensource"
        );
        // "C#" is not a hashtag.
        assert_eq!(derive_title("<p>why C# wins here</p>"), "why C# wins here");
        // Mastodon's real hashtag markup splits '#' and the word into separate
        // text nodes: `#<span>tag</span>` — they must re-join before the rules.
        assert_eq!(
            derive_title("<p>Learn Linux with David <a>#<span>Linux</span></a> <a>#<span>Ubuntu</span></a> <a>#<span>KaliLinux</span></a></p>"),
            "Learn Linux with David"
        );
        // Mentions split the same way and must re-join (and stay in the title).
        // Surrounding punctuation stays space-separated (tag boundaries are
        // word boundaries) — "( @amnesty )" is readable; "( @ amnesty )" wasn't.
        assert_eq!(
            derive_title("<p>Amnesty (<a>@<span>amnesty</span></a>) released a breakdown</p>"),
            "Amnesty ( @amnesty ) released a breakdown"
        );
    }

    #[test]
    fn hoists_security_id_to_title_front() {
        // A rambling toot that buries the CVE — the id must lead so the headline
        // is useful and the downstream classifier (which reads the title) can tag
        // it as a security advisory.
        let html = "<p>Saturday, but self hosting, so here we go. Earlier this month the HTTP/2 Bomb CVE-2026-49975 dropped, worth a look.</p>";
        let title = derive_title(html);
        assert!(
            title.starts_with("CVE-2026-49975"),
            "CVE should lead the title, got: {title}"
        );

        // GHSA ids too.
        let g = derive_title("<p>heads up, GHSA-w24r-5266-9c3c affects clerk, patch soon</p>");
        assert!(g.starts_with("GHSA-w24r-5266-9c3c"), "got: {g}");

        // Already at the front — don't double-prefix.
        assert_eq!(
            derive_title("<p>CVE-2026-1111 is a nasty one</p>"),
            "CVE-2026-1111 is a nasty one"
        );

        // No id — unchanged behavior.
        assert_eq!(
            derive_title("<p>just a normal post about rust</p>"),
            "just a normal post about rust"
        );
    }

    #[test]
    fn parses_mastodon_api_json() {
        let json = r#"[
            {
                "uri": "https://hachyderm.io/users/x/statuses/1",
                "url": "https://hachyderm.io/@x/1",
                "content": "<p>A neat Rust crate just dropped</p>",
                "account": { "acct": "x@hachyderm.io" },
                "favourites_count": 12, "reblogs_count": 3, "replies_count": 2,
                "tags": [{ "name": "rust" }]
            },
            { "uri": "https://hachyderm.io/users/y/statuses/2", "content": "<p>boost</p>", "reblog": { "id": "9" } }
        ]"#;
        let statuses: Vec<MastodonStatus> = serde_json::from_str(json).unwrap();
        let items: Vec<_> = statuses.into_iter().filter_map(status_to_item).collect();
        assert_eq!(items.len(), 1, "the pure boost is skipped");
        assert_eq!(
            items[0].source_id,
            "https://hachyderm.io/users/x/statuses/1"
        );
        assert_eq!(items[0].title, "A neat Rust crate just dropped");
        let md = items[0].metadata.as_ref().unwrap();
        assert_eq!(md.get("score").and_then(|v| v.as_i64()), Some(15)); // favs + reblogs
    }

    #[test]
    fn parses_mastodon_rss() {
        let xml = r#"<rss><channel>
            <item><title>A tagged post</title><link>https://hachyderm.io/@x/1</link>
              <description>&lt;p&gt;body&lt;/p&gt;</description><guid>https://hachyderm.io/@x/1</guid></item>
        </channel></rss>"#;
        let items = parse_mastodon_rss(xml, "rust", 40);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "A tagged post");
        assert_eq!(items[0].source_id, "https://hachyderm.io/@x/1");
    }

    #[test]
    fn mastodon_source_defaults() {
        let s = MastodonSource::new();
        assert_eq!(s.source_type(), "mastodon");
        assert_eq!(s.name(), "Mastodon");
        assert!(s.config().enabled);
        assert_eq!(s.strategies().len(), 2, "api + rss");
        assert!(s.tags.contains(&"rust"));
    }

    /// LIVE: federated open-protocol dev-tag fetch produces items credential-free.
    /// Run: `cargo test --lib sources::mastodon::tests::live -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "network: verifies live Mastodon dev-tag fetch"]
    async fn live_mastodon_produces_items() {
        match MastodonSource::new().fetch_items().await {
            Ok(items) => {
                assert!(!items.is_empty(), "expected dev-tag items from Mastodon");
                let via = items
                    .iter()
                    .filter_map(|i| i.metadata.as_ref()?.get("via")?.as_str())
                    .next()
                    .unwrap_or("unknown");
                println!("LIVE mastodon: {} items via {via}", items.len());
            }
            Err(e) => {
                assert!(
                    matches!(e, SourceError::Forbidden(_) | SourceError::RateLimited(_)),
                    "credential-free paths must fail with an ACTIONABLE error, got {e:?}"
                );
                println!("LIVE mastodon: walled -> surfaced {e:?}");
            }
        }
    }
}
