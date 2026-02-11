//! Twitter/X source implementation
//!
//! Fetches tweets from X API v2 using Bearer Token auth (BYOK).
//! Requires user to provide their own X API Bearer Token.

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{debug, info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// X API v2 Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct XApiResponse {
    data: Option<Vec<XTweet>>,
    includes: Option<XIncludes>,
    #[allow(dead_code)]
    meta: Option<XMeta>,
}

#[derive(Debug, Deserialize)]
struct XTweet {
    id: String,
    text: String,
    author_id: Option<String>,
    created_at: Option<String>,
    public_metrics: Option<XPublicMetrics>,
}

#[derive(Debug, Deserialize)]
struct XIncludes {
    users: Option<Vec<XUser>>,
}

#[derive(Debug, Deserialize)]
struct XUser {
    id: String,
    username: String,
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct XPublicMetrics {
    retweet_count: u64,
    reply_count: u64,
    like_count: u64,
    #[serde(default)]
    impression_count: u64,
}

#[derive(Debug, Deserialize)]
struct XMeta {
    #[allow(dead_code)]
    result_count: Option<u32>,
    #[allow(dead_code)]
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XUserLookupResponse {
    data: Option<XUserLookupData>,
}

#[derive(Debug, Deserialize)]
struct XUserLookupData {
    id: String,
}

// ============================================================================
// Twitter/X Source
// ============================================================================

/// Twitter/X source - fetches tweets via X API v2 (Bearer Token)
pub struct TwitterSource {
    config: SourceConfig,
    client: reqwest::Client,
    handles: Vec<String>,
    api_key: String,
}

impl TwitterSource {
    /// Create with default tech influencer handles (requires API key to function)
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 50,
                fetch_interval_secs: 900, // 15 minutes
                custom: None,
            },
            client: super::shared_client(),
            handles: vec![
                // Core developer advocates
                "ThePrimeagen".into(),
                "dan_abramov".into(),
                "kentcdodds".into(),
                "addyosmani".into(),
                "traversymedia".into(),
                // Rust / systems
                "jonhoo".into(),
                "m_ou_se".into(),
                "fasterthanlime".into(),
                // AI/ML
                "karpathy".into(),
                "ylecun".into(),
                "svpino".into(),
                "AndrewYNg".into(),
                // Web platform
                "leeerob".into(),
                "rauchg".into(),
                "Rich_Harris".into(),
                "ryanflorence".into(),
                "wesbos".into(),
                // DevOps / Cloud
                "kelseyhightower".into(),
                "jessfraz".into(),
                // Security
                "taviso".into(),
                "SwiftOnSecurity".into(),
                // Tech journalists / commentators
                "benthompson".into(),
                "dhh".into(),
                "paulg".into(),
                // Open source
                "mitchellh".into(),
                "antirez".into(),
                "matklad".into(),
                // Indie hackers
                "levelsio".into(),
                "marc_louvion".into(),
                "t3dotgg".into(),
            ],
            api_key: String::new(),
        }
    }

    /// Create with custom handles
    pub fn with_handles(handles: Vec<String>) -> Self {
        let mut source = Self::new();
        if !handles.is_empty() {
            source.handles = handles;
        }
        source
    }

    /// Set the X API Bearer Token
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = api_key;
        self
    }

    /// Look up a user ID from a username
    async fn lookup_user_id(&self, username: &str) -> Result<String, String> {
        let url = format!("https://api.x.com/2/users/by/username/{}", username);

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| format!("Network error looking up @{}: {}", username, e))?;

        if resp.status() == 429 {
            return Err(format!("Rate limited looking up @{}", username));
        }

        if !resp.status().is_success() {
            return Err(format!(
                "X API error for @{}: HTTP {}",
                username,
                resp.status()
            ));
        }

        let body: XUserLookupResponse = resp
            .json()
            .await
            .map_err(|e| format!("Parse error for @{}: {}", username, e))?;

        body.data
            .map(|d| d.id)
            .ok_or_else(|| format!("User @{} not found", username))
    }

    /// Fetch recent tweets for a user by their ID
    async fn fetch_user_tweets(
        &self,
        user_id: &str,
        username: &str,
    ) -> Result<Vec<SourceItem>, String> {
        let url = format!(
            "https://api.x.com/2/users/{}/tweets?max_results=10&tweet.fields=created_at,public_metrics,author_id&expansions=author_id&user.fields=username,name",
            user_id
        );

        debug!(username, user_id, "Fetching tweets from X API v2");

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| format!("Network error fetching tweets for @{}: {}", username, e))?;

        if resp.status() == 429 {
            return Err(format!("Rate limited fetching tweets for @{}", username));
        }

        if !resp.status().is_success() {
            return Err(format!(
                "X API error for @{}: HTTP {}",
                username,
                resp.status()
            ));
        }

        let body: XApiResponse = resp
            .json()
            .await
            .map_err(|e| format!("Parse error for @{}: {}", username, e))?;

        let tweets = body.data.unwrap_or_default();
        let users = body.includes.and_then(|i| i.users).unwrap_or_default();

        let items: Vec<SourceItem> = tweets
            .into_iter()
            .map(|tweet| {
                // Find the author username from includes
                let author = tweet
                    .author_id
                    .as_ref()
                    .and_then(|aid| users.iter().find(|u| &u.id == aid))
                    .map(|u| format!("@{}", u.username))
                    .unwrap_or_else(|| format!("@{}", username));

                let tweet_url = format!("https://x.com/{}/status/{}", username, tweet.id);

                // Build a readable title from the tweet text (first ~100 chars)
                let title = if tweet.text.len() > 120 {
                    format!(
                        "{}...",
                        &tweet.text[..tweet
                            .text
                            .char_indices()
                            .nth(117)
                            .map(|(i, _)| i)
                            .unwrap_or(tweet.text.len())]
                    )
                } else {
                    tweet.text.clone()
                };

                let metrics = tweet.public_metrics.as_ref();
                let metadata = serde_json::json!({
                    "author": author,
                    "created_at": tweet.created_at,
                    "likes": metrics.map(|m| m.like_count).unwrap_or(0),
                    "retweets": metrics.map(|m| m.retweet_count).unwrap_or(0),
                    "replies": metrics.map(|m| m.reply_count).unwrap_or(0),
                    "impressions": metrics.map(|m| m.impression_count).unwrap_or(0),
                });

                SourceItem::new("twitter", &tweet.id, &title)
                    .with_url(Some(tweet_url))
                    .with_content(tweet.text)
                    .with_metadata(metadata)
            })
            .collect();

        Ok(items)
    }

    /// Search recent tweets by query
    async fn search_recent(
        &self,
        query: &str,
        max_results: u32,
    ) -> Result<Vec<SourceItem>, String> {
        let url = format!(
            "https://api.x.com/2/tweets/search/recent?query={}&max_results={}&tweet.fields=created_at,public_metrics,author_id&expansions=author_id&user.fields=username,name",
            urlencoding::encode(query),
            max_results.min(100)
        );

        debug!(query, "Searching recent tweets via X API v2");

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| format!("Network error searching tweets: {}", e))?;

        if resp.status() == 429 {
            return Err("Rate limited on tweet search".to_string());
        }

        if !resp.status().is_success() {
            return Err(format!("X API search error: HTTP {}", resp.status()));
        }

        let body: XApiResponse = resp
            .json()
            .await
            .map_err(|e| format!("Parse error on search: {}", e))?;

        let tweets = body.data.unwrap_or_default();
        let users = body.includes.and_then(|i| i.users).unwrap_or_default();

        let items: Vec<SourceItem> = tweets
            .into_iter()
            .map(|tweet| {
                let author = tweet
                    .author_id
                    .as_ref()
                    .and_then(|aid| users.iter().find(|u| &u.id == aid))
                    .map(|u| format!("@{}", u.username))
                    .unwrap_or_else(|| "unknown".to_string());

                let author_username = tweet
                    .author_id
                    .as_ref()
                    .and_then(|aid| users.iter().find(|u| &u.id == aid))
                    .map(|u| u.username.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                let tweet_url = format!("https://x.com/{}/status/{}", author_username, tweet.id);

                let title = if tweet.text.len() > 120 {
                    format!(
                        "{}...",
                        &tweet.text[..tweet
                            .text
                            .char_indices()
                            .nth(117)
                            .map(|(i, _)| i)
                            .unwrap_or(tweet.text.len())]
                    )
                } else {
                    tweet.text.clone()
                };

                let metrics = tweet.public_metrics.as_ref();
                let metadata = serde_json::json!({
                    "author": author,
                    "created_at": tweet.created_at,
                    "likes": metrics.map(|m| m.like_count).unwrap_or(0),
                    "retweets": metrics.map(|m| m.retweet_count).unwrap_or(0),
                    "replies": metrics.map(|m| m.reply_count).unwrap_or(0),
                });

                SourceItem::new("twitter", &tweet.id, &title)
                    .with_url(Some(tweet_url))
                    .with_content(tweet.text)
                    .with_metadata(metadata)
            })
            .collect();

        Ok(items)
    }
}

impl Default for TwitterSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for TwitterSource {
    fn source_type(&self) -> &'static str {
        "twitter"
    }

    fn name(&self) -> &'static str {
        "Twitter/X"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        if self.api_key.is_empty() {
            debug!(target: "4da::twitter", "No X API key configured, skipping Twitter source");
            return Ok(Vec::new());
        }

        info!(handles = self.handles.len(), "Fetching tweets via X API v2");

        let mut all_items = Vec::new();
        let mut rate_limited = false;

        // Fetch tweets for each configured handle (bail early on rate limit)
        for handle in &self.handles {
            if rate_limited {
                debug!(handle = %handle, "Skipping - rate limited");
                continue;
            }

            // First look up the user ID
            match self.lookup_user_id(handle).await {
                Ok(user_id) => match self.fetch_user_tweets(&user_id, handle).await {
                    Ok(items) => {
                        info!(handle = %handle, items = items.len(), "Fetched tweets");
                        all_items.extend(items);
                    }
                    Err(e) => {
                        if e.contains("Rate limited") {
                            warn!("X API rate limited - stopping handle fetches");
                            rate_limited = true;
                        } else {
                            warn!(handle = %handle, error = %e, "Failed to fetch tweets");
                        }
                    }
                },
                Err(e) => {
                    if e.contains("Rate limited") {
                        warn!("X API rate limited - stopping handle fetches");
                        rate_limited = true;
                    } else {
                        warn!(handle = %handle, error = %e, "Failed to look up user");
                    }
                }
            }

            // Delay between handles to be rate-limit friendly
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        // Truncate to max items
        all_items.truncate(self.config.max_items);

        info!(total_items = all_items.len(), "Fetched Twitter/X items");
        Ok(all_items)
    }

    async fn fetch_items_deep(&self, _items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        if self.api_key.is_empty() {
            return Ok(Vec::new());
        }

        info!(target: "4da::twitter", "Deep fetch: timeline + tech search");

        // Regular timeline fetch
        let mut all_items = self.fetch_items().await.unwrap_or_default();

        // Also search for trending tech topics
        let tech_queries = [
            "programming lang:en -is:retweet",
            "developer tools lang:en -is:retweet",
        ];

        for query in tech_queries {
            match self.search_recent(query, 20).await {
                Ok(items) => {
                    info!(query, count = items.len(), "Search results");
                    all_items.extend(items);
                }
                Err(e) => {
                    if e.contains("Rate limited") {
                        warn!("X API rate limited - stopping searches");
                        break;
                    }
                    warn!(query, error = %e, "Search failed");
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        // Deduplicate by tweet ID
        let mut seen = std::collections::HashSet::new();
        all_items.retain(|item| seen.insert(item.source_id.clone()));

        Ok(all_items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Tweets already have their full text as content
        Ok(item.content.clone())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_twitter_source_creation() {
        let source = TwitterSource::new();
        assert_eq!(source.source_type(), "twitter");
        assert_eq!(source.name(), "Twitter/X");
        assert_eq!(source.handles.len(), 30);
        assert!(source.api_key.is_empty());
    }

    #[test]
    fn test_custom_handles() {
        let source = TwitterSource::with_handles(vec!["naval".into()]);
        assert_eq!(source.handles.len(), 1);
        assert_eq!(source.handles[0], "naval");
    }

    #[test]
    fn test_empty_handles_uses_defaults() {
        let source = TwitterSource::with_handles(vec![]);
        assert_eq!(source.handles.len(), 30); // Falls back to defaults
    }

    #[test]
    fn test_with_api_key() {
        let source = TwitterSource::new().with_api_key("test_bearer_token".into());
        assert_eq!(source.api_key, "test_bearer_token");
    }

    #[test]
    fn test_fetch_interval() {
        let source = TwitterSource::new();
        assert_eq!(source.config.fetch_interval_secs, 900); // 15 minutes
    }

    #[tokio::test]
    async fn test_no_api_key_returns_empty() {
        let source = TwitterSource::new(); // No API key
        let items = source.fetch_items().await.unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_deserialize_x_api_response() {
        let json = r#"{
            "data": [{
                "id": "1234567890",
                "text": "Test tweet about Rust programming",
                "author_id": "111",
                "created_at": "2026-02-08T10:00:00.000Z",
                "public_metrics": {
                    "retweet_count": 5,
                    "reply_count": 2,
                    "like_count": 42,
                    "impression_count": 1000
                }
            }],
            "includes": {
                "users": [{
                    "id": "111",
                    "username": "testuser",
                    "name": "Test User"
                }]
            },
            "meta": {
                "result_count": 1
            }
        }"#;

        let response: XApiResponse = serde_json::from_str(json).unwrap();
        let tweets = response.data.unwrap();
        assert_eq!(tweets.len(), 1);
        assert_eq!(tweets[0].id, "1234567890");
        assert_eq!(tweets[0].text, "Test tweet about Rust programming");
        assert_eq!(tweets[0].public_metrics.as_ref().unwrap().like_count, 42);

        let users = response.includes.unwrap().users.unwrap();
        assert_eq!(users[0].username, "testuser");
    }
}
