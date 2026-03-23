//! Resilience tests for source adapter parsing and deserialization.
//!
//! These tests verify that every adapter gracefully handles edge-case inputs:
//! empty data, malformed payloads, missing fields, oversized content,
//! unicode edge cases, HTML entities, and null/zero values.
//!
//! No network calls — only local parse functions and serde deserialization.

use super::*;
use serde_json;

// ============================================================================
// 1. SourceItem core — builder and embedding_text
// ============================================================================

#[test]
fn source_item_empty_fields() {
    let item = SourceItem::new("", "", "");
    assert!(item.source_type.is_empty());
    assert!(item.source_id.is_empty());
    assert!(item.title.is_empty());
    assert!(item.url.is_none());
    assert!(item.content.is_empty());
    assert!(item.metadata.is_none());
    assert_eq!(item.embedding_text(), "");
}

#[test]
fn source_item_unicode_title() {
    let item = SourceItem::new("test", "1", "Rust 🦀 是最好的 لغة");
    assert_eq!(item.title, "Rust 🦀 是最好的 لغة");
    assert_eq!(item.embedding_text(), "Rust 🦀 是最好的 لغة");
}

#[test]
fn source_item_very_long_content() {
    let big = "x".repeat(50_000);
    let item = SourceItem::new("test", "1", "Title").with_content(big.clone());
    assert_eq!(item.content.len(), 50_000);
    let emb = item.embedding_text();
    assert!(emb.starts_with("Title\n\n"));
    assert_eq!(emb.len(), 50_000 + "Title\n\n".len());
}

#[test]
fn source_item_with_metadata_null_values() {
    let meta = serde_json::json!({
        "score": null,
        "author": null,
        "tags": [],
    });
    let item = SourceItem::new("test", "1", "Title").with_metadata(meta.clone());
    let m = item.metadata.as_ref().unwrap();
    assert!(m["score"].is_null());
    assert!(m["author"].is_null());
    assert!(m["tags"].as_array().unwrap().is_empty());
}

#[test]
fn source_item_serialization_roundtrip() {
    let item = SourceItem::new("test", "42", "Hello")
        .with_url(Some("https://example.com".to_string()))
        .with_content("body".to_string())
        .with_metadata(serde_json::json!({"k": "v"}));

    let json = serde_json::to_string(&item).unwrap();
    let back: SourceItem = serde_json::from_str(&json).unwrap();
    assert_eq!(back.source_id, "42");
    assert_eq!(back.title, "Hello");
    assert_eq!(back.url.as_deref(), Some("https://example.com"));
    assert_eq!(back.content, "body");
}

#[test]
fn source_item_deserialize_missing_optional_fields() {
    let json = r#"{
        "source_id": "1",
        "source_type": "test",
        "title": "T",
        "content": ""
    }"#;
    let item: SourceItem = serde_json::from_str(json).unwrap();
    assert!(item.url.is_none());
    assert!(item.metadata.is_none());
}

// ============================================================================
// 2. Hacker News — HNStory serde deserialization
// ============================================================================

mod hn_resilience {
    use serde::Deserialize;

    /// Mirror of the private HNStory struct for deserialization tests.
    #[derive(Debug, Deserialize)]
    struct HNStory {
        id: u64,
        title: Option<String>,
        url: Option<String>,
        text: Option<String>,
        score: Option<i32>,
        by: Option<String>,
    }

    #[test]
    fn hn_empty_json_object() {
        // Missing required `id` field should fail
        let result = serde_json::from_str::<HNStory>("{}");
        assert!(result.is_err());
    }

    #[test]
    fn hn_minimal_valid() {
        let json = r#"{"id": 1}"#;
        let story: HNStory = serde_json::from_str(json).unwrap();
        assert_eq!(story.id, 1);
        assert!(story.title.is_none());
        assert!(story.url.is_none());
        assert!(story.text.is_none());
        assert!(story.score.is_none());
        assert!(story.by.is_none());
    }

    #[test]
    fn hn_all_fields_null() {
        let json =
            r#"{"id": 99, "title": null, "url": null, "text": null, "score": null, "by": null}"#;
        let story: HNStory = serde_json::from_str(json).unwrap();
        assert_eq!(story.id, 99);
        assert!(story.title.is_none());
    }

    #[test]
    fn hn_zero_score() {
        let json = r#"{"id": 5, "score": 0}"#;
        let story: HNStory = serde_json::from_str(json).unwrap();
        assert_eq!(story.score, Some(0));
    }

    #[test]
    fn hn_negative_score() {
        let json = r#"{"id": 5, "score": -10}"#;
        let story: HNStory = serde_json::from_str(json).unwrap();
        assert_eq!(story.score, Some(-10));
    }

    #[test]
    fn hn_unicode_title() {
        let json = r#"{"id": 7, "title": "Rust 🦀 パフォーマンス بهترین"}"#;
        let story: HNStory = serde_json::from_str(json).unwrap();
        assert_eq!(
            story.title.as_deref(),
            Some("Rust 🦀 パフォーマンス بهترین")
        );
    }

    #[test]
    fn hn_html_in_text() {
        let json = r#"{"id": 8, "text": "<p>Hello &amp; world</p>"}"#;
        let story: HNStory = serde_json::from_str(json).unwrap();
        assert_eq!(story.text.as_deref(), Some("<p>Hello &amp; world</p>"));
    }

    #[test]
    fn hn_very_large_id() {
        let json = r#"{"id": 18446744073709551615}"#;
        let story: HNStory = serde_json::from_str(json).unwrap();
        assert_eq!(story.id, u64::MAX);
    }

    #[test]
    fn hn_extra_unknown_fields_ignored() {
        // serde by default ignores unknown fields
        let json = r#"{"id": 10, "descendants": 42, "type": "story", "unknown_field": true}"#;
        let story: HNStory = serde_json::from_str(json).unwrap();
        assert_eq!(story.id, 10);
    }

    #[test]
    fn hn_invalid_json() {
        let result = serde_json::from_str::<HNStory>("{broken");
        assert!(result.is_err());
    }

    #[test]
    fn hn_empty_string_input() {
        let result = serde_json::from_str::<HNStory>("");
        assert!(result.is_err());
    }

    #[test]
    fn hn_array_instead_of_object() {
        let result = serde_json::from_str::<HNStory>("[]");
        assert!(result.is_err());
    }
}

// ============================================================================
// 3. Reddit — RedditListing serde deserialization
// ============================================================================

mod reddit_resilience {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct RedditListing {
        data: RedditListingData,
    }

    #[derive(Debug, Deserialize)]
    struct RedditListingData {
        children: Vec<RedditChild>,
    }

    #[derive(Debug, Deserialize)]
    struct RedditChild {
        data: RedditPost,
    }

    #[derive(Debug, Deserialize)]
    struct RedditPost {
        id: String,
        title: String,
        selftext: Option<String>,
        url: Option<String>,
        permalink: String,
        score: i32,
        author: String,
        subreddit: String,
        num_comments: i32,
        is_self: bool,
    }

    #[test]
    fn reddit_empty_children() {
        let json = r#"{"data": {"children": []}}"#;
        let listing: RedditListing = serde_json::from_str(json).unwrap();
        assert!(listing.data.children.is_empty());
    }

    #[test]
    fn reddit_minimal_post() {
        let json = r#"{
            "data": {"children": [{"data": {
                "id": "abc",
                "title": "Test",
                "selftext": null,
                "url": null,
                "permalink": "/r/test/comments/abc",
                "score": 0,
                "author": "user",
                "subreddit": "test",
                "num_comments": 0,
                "is_self": true
            }}]}
        }"#;
        let listing: RedditListing = serde_json::from_str(json).unwrap();
        assert_eq!(listing.data.children.len(), 1);
        let post = &listing.data.children[0].data;
        assert_eq!(post.id, "abc");
        assert_eq!(post.score, 0);
        assert!(post.is_self);
    }

    #[test]
    fn reddit_negative_score() {
        let json = r#"{
            "data": {"children": [{"data": {
                "id": "x",
                "title": "Downvoted",
                "permalink": "/r/test/comments/x",
                "score": -50,
                "author": "u",
                "subreddit": "test",
                "num_comments": 0,
                "is_self": false
            }}]}
        }"#;
        let listing: RedditListing = serde_json::from_str(json).unwrap();
        assert_eq!(listing.data.children[0].data.score, -50);
    }

    #[test]
    fn reddit_unicode_title() {
        let json = r#"{
            "data": {"children": [{"data": {
                "id": "u",
                "title": "日本語タイトル 🎉 العربية",
                "permalink": "/r/test/comments/u",
                "score": 1,
                "author": "u",
                "subreddit": "test",
                "num_comments": 0,
                "is_self": true
            }}]}
        }"#;
        let listing: RedditListing = serde_json::from_str(json).unwrap();
        assert_eq!(
            listing.data.children[0].data.title,
            "日本語タイトル 🎉 العربية"
        );
    }

    #[test]
    fn reddit_invalid_json() {
        let result = serde_json::from_str::<RedditListing>("{bad");
        assert!(result.is_err());
    }

    #[test]
    fn reddit_missing_data_field() {
        let result = serde_json::from_str::<RedditListing>(r#"{"children": []}"#);
        assert!(result.is_err());
    }

    #[test]
    fn reddit_empty_string() {
        let result = serde_json::from_str::<RedditListing>("");
        assert!(result.is_err());
    }

    #[test]
    fn reddit_html_entities_in_selftext() {
        let json = r#"{
            "data": {"children": [{"data": {
                "id": "h",
                "title": "Title",
                "selftext": "&lt;script&gt;alert(1)&lt;/script&gt;",
                "permalink": "/r/test/comments/h",
                "score": 1,
                "author": "u",
                "subreddit": "test",
                "num_comments": 0,
                "is_self": true
            }}]}
        }"#;
        let listing: RedditListing = serde_json::from_str(json).unwrap();
        assert_eq!(
            listing.data.children[0].data.selftext.as_deref(),
            Some("&lt;script&gt;alert(1)&lt;/script&gt;")
        );
    }
}

// ============================================================================
// 4. Dev.to — DevtoArticle serde deserialization
// ============================================================================

mod devto_resilience {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct DevtoArticle {
        id: u64,
        title: String,
        url: String,
        #[serde(default)]
        description: String,
        published_at: Option<String>,
        positive_reactions_count: Option<i32>,
        comments_count: Option<i32>,
        #[serde(default)]
        tag_list: Vec<String>,
        user: Option<DevtoUser>,
        #[serde(default)]
        reading_time_minutes: Option<i32>,
    }

    #[derive(Debug, Deserialize)]
    struct DevtoUser {
        name: String,
        #[serde(default)]
        username: String,
    }

    #[test]
    fn devto_empty_array() {
        let articles: Vec<DevtoArticle> = serde_json::from_str("[]").unwrap();
        assert!(articles.is_empty());
    }

    #[test]
    fn devto_minimal_article() {
        let json = r#"[{"id": 1, "title": "T", "url": "http://x"}]"#;
        let articles: Vec<DevtoArticle> = serde_json::from_str(json).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].id, 1);
        assert!(articles[0].description.is_empty());
        assert!(articles[0].tag_list.is_empty());
        assert!(articles[0].user.is_none());
        assert!(articles[0].reading_time_minutes.is_none());
    }

    #[test]
    fn devto_zero_reactions() {
        let json = r#"[{"id": 2, "title": "T", "url": "http://x", "positive_reactions_count": 0, "comments_count": 0}]"#;
        let articles: Vec<DevtoArticle> = serde_json::from_str(json).unwrap();
        assert_eq!(articles[0].positive_reactions_count, Some(0));
        assert_eq!(articles[0].comments_count, Some(0));
    }

    #[test]
    fn devto_unicode_everywhere() {
        let json = r#"[{
            "id": 3,
            "title": "构建 Tauri 应用 🚀",
            "url": "https://dev.to/测试",
            "description": "Opis článku: ěščřžýáíé"
        }]"#;
        let articles: Vec<DevtoArticle> = serde_json::from_str(json).unwrap();
        assert_eq!(articles[0].title, "构建 Tauri 应用 🚀");
    }

    #[test]
    fn devto_empty_user_username() {
        let json = r#"[{
            "id": 4, "title": "T", "url": "http://x",
            "user": {"name": "Someone", "username": ""}
        }]"#;
        let articles: Vec<DevtoArticle> = serde_json::from_str(json).unwrap();
        assert_eq!(articles[0].user.as_ref().unwrap().username, "");
    }

    #[test]
    fn devto_invalid_json() {
        let result = serde_json::from_str::<Vec<DevtoArticle>>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn devto_missing_required_title() {
        let result = serde_json::from_str::<Vec<DevtoArticle>>(r#"[{"id": 5, "url": "http://x"}]"#);
        assert!(result.is_err());
    }

    #[test]
    fn devto_many_tags() {
        let tags: Vec<String> = (0..100).map(|i| format!("tag{}", i)).collect();
        let json = format!(
            r#"[{{"id": 6, "title": "T", "url": "http://x", "tag_list": {}}}]"#,
            serde_json::to_string(&tags).unwrap()
        );
        let articles: Vec<DevtoArticle> = serde_json::from_str(&json).unwrap();
        assert_eq!(articles[0].tag_list.len(), 100);
    }
}

// ============================================================================
// 5. Lobsters — LobstersStory serde deserialization
// ============================================================================

mod lobsters_resilience {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct LobstersStory {
        short_id: String,
        title: String,
        url: Option<String>,
        #[serde(default)]
        description: String,
        created_at: Option<String>,
        score: Option<i32>,
        comment_count: Option<i32>,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        submitter_user: Option<LobstersUser>,
    }

    #[derive(Debug, Deserialize)]
    struct LobstersUser {
        username: String,
    }

    #[test]
    fn lobsters_empty_array() {
        let stories: Vec<LobstersStory> = serde_json::from_str("[]").unwrap();
        assert!(stories.is_empty());
    }

    #[test]
    fn lobsters_minimal() {
        let json = r#"[{"short_id": "a", "title": "T"}]"#;
        let stories: Vec<LobstersStory> = serde_json::from_str(json).unwrap();
        assert_eq!(stories[0].short_id, "a");
        assert!(stories[0].url.is_none());
        assert!(stories[0].score.is_none());
        assert!(stories[0].tags.is_empty());
    }

    #[test]
    fn lobsters_null_url() {
        let json = r#"[{"short_id": "b", "title": "T", "url": null}]"#;
        let stories: Vec<LobstersStory> = serde_json::from_str(json).unwrap();
        assert!(stories[0].url.is_none());
    }

    #[test]
    fn lobsters_zero_score() {
        let json = r#"[{"short_id": "c", "title": "T", "score": 0, "comment_count": 0}]"#;
        let stories: Vec<LobstersStory> = serde_json::from_str(json).unwrap();
        assert_eq!(stories[0].score, Some(0));
        assert_eq!(stories[0].comment_count, Some(0));
    }

    #[test]
    fn lobsters_unicode_title() {
        let json = r#"[{"short_id": "d", "title": "메모리 안전 🛡️ Мемо"}]"#;
        let stories: Vec<LobstersStory> = serde_json::from_str(json).unwrap();
        assert_eq!(stories[0].title, "메모리 안전 🛡️ Мемо");
    }

    #[test]
    fn lobsters_invalid_json() {
        let result = serde_json::from_str::<Vec<LobstersStory>>("{bad}");
        assert!(result.is_err());
    }

    #[test]
    fn lobsters_empty_string() {
        let result = serde_json::from_str::<Vec<LobstersStory>>("");
        assert!(result.is_err());
    }
}

// ============================================================================
// 6. GitHub — GitHubSearchResponse serde deserialization
// ============================================================================

mod github_resilience {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct GitHubRepo {
        id: u64,
        name: String,
        full_name: String,
        description: Option<String>,
        html_url: String,
        stargazers_count: i32,
        language: Option<String>,
        updated_at: String,
        #[serde(default)]
        topics: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct GitHubSearchResponse {
        total_count: u32,
        items: Vec<GitHubRepo>,
    }

    #[test]
    fn github_empty_results() {
        let json = r#"{"total_count": 0, "items": []}"#;
        let resp: GitHubSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.total_count, 0);
        assert!(resp.items.is_empty());
    }

    #[test]
    fn github_minimal_repo() {
        let json = r#"{
            "total_count": 1,
            "items": [{
                "id": 1,
                "name": "repo",
                "full_name": "user/repo",
                "description": null,
                "html_url": "https://github.com/user/repo",
                "stargazers_count": 0,
                "language": null,
                "updated_at": "2026-01-01T00:00:00Z"
            }]
        }"#;
        let resp: GitHubSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.items.len(), 1);
        assert!(resp.items[0].description.is_none());
        assert!(resp.items[0].language.is_none());
        assert!(resp.items[0].topics.is_empty());
        assert_eq!(resp.items[0].stargazers_count, 0);
    }

    #[test]
    fn github_unicode_description() {
        let json = r#"{
            "total_count": 1,
            "items": [{
                "id": 2,
                "name": "项目",
                "full_name": "用户/项目",
                "description": "A tool for 開発者 👨‍💻",
                "html_url": "https://github.com/test/test",
                "stargazers_count": 100,
                "language": "Rust",
                "updated_at": "2026-01-01T00:00:00Z"
            }]
        }"#;
        let resp: GitHubSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.items[0].description.as_deref(),
            Some("A tool for 開発者 👨‍💻")
        );
    }

    #[test]
    fn github_zero_stars() {
        let json = r#"{
            "total_count": 1,
            "items": [{
                "id": 3,
                "name": "r",
                "full_name": "u/r",
                "html_url": "https://github.com/u/r",
                "stargazers_count": 0,
                "language": null,
                "updated_at": "2026-01-01T00:00:00Z"
            }]
        }"#;
        let resp: GitHubSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.items[0].stargazers_count, 0);
    }

    #[test]
    fn github_many_topics() {
        let topics: Vec<String> = (0..50).map(|i| format!("topic-{}", i)).collect();
        let json = format!(
            r#"{{
                "total_count": 1,
                "items": [{{
                    "id": 4,
                    "name": "r",
                    "full_name": "u/r",
                    "html_url": "https://github.com/u/r",
                    "stargazers_count": 1,
                    "language": null,
                    "updated_at": "2026-01-01T00:00:00Z",
                    "topics": {}
                }}]
            }}"#,
            serde_json::to_string(&topics).unwrap()
        );
        let resp: GitHubSearchResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.items[0].topics.len(), 50);
    }

    #[test]
    fn github_invalid_json() {
        let result = serde_json::from_str::<GitHubSearchResponse>("{nope");
        assert!(result.is_err());
    }

    #[test]
    fn github_missing_items_field() {
        let result = serde_json::from_str::<GitHubSearchResponse>(r#"{"total_count": 0}"#);
        assert!(result.is_err());
    }
}

// ============================================================================
// 7. Twitter/X — XApiResponse serde deserialization
// ============================================================================

mod twitter_resilience {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct XApiResponse {
        data: Option<Vec<XTweet>>,
        includes: Option<XIncludes>,
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
        result_count: Option<u32>,
        next_token: Option<String>,
    }

    #[test]
    fn twitter_empty_response() {
        let json = r#"{"data": null, "includes": null, "meta": null}"#;
        let resp: XApiResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_none());
        assert!(resp.includes.is_none());
    }

    #[test]
    fn twitter_empty_data_array() {
        let json = r#"{"data": [], "meta": {"result_count": 0}}"#;
        let resp: XApiResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.unwrap().is_empty());
    }

    #[test]
    fn twitter_minimal_tweet() {
        let json = r#"{"data": [{"id": "1", "text": "hello"}]}"#;
        let resp: XApiResponse = serde_json::from_str(json).unwrap();
        let tweets = resp.data.unwrap();
        assert_eq!(tweets[0].id, "1");
        assert_eq!(tweets[0].text, "hello");
        assert!(tweets[0].author_id.is_none());
        assert!(tweets[0].public_metrics.is_none());
    }

    #[test]
    fn twitter_zero_metrics() {
        let json = r#"{"data": [{"id": "2", "text": "t", "public_metrics": {
            "retweet_count": 0, "reply_count": 0, "like_count": 0, "impression_count": 0
        }}]}"#;
        let resp: XApiResponse = serde_json::from_str(json).unwrap();
        let tweets = resp.data.unwrap();
        let m = tweets[0].public_metrics.as_ref().unwrap();
        assert_eq!(m.like_count, 0);
        assert_eq!(m.impression_count, 0);
    }

    #[test]
    fn twitter_missing_impression_count_defaults() {
        let json = r#"{"data": [{"id": "3", "text": "t", "public_metrics": {
            "retweet_count": 1, "reply_count": 1, "like_count": 1
        }}]}"#;
        let resp: XApiResponse = serde_json::from_str(json).unwrap();
        let tweets = resp.data.unwrap();
        let m = tweets[0].public_metrics.as_ref().unwrap();
        assert_eq!(m.impression_count, 0); // serde(default) should fill 0
    }

    #[test]
    fn twitter_unicode_text() {
        let json = r#"{"data": [{"id": "4", "text": "Rust 🦀 很好 \u2764\ufe0f"}]}"#;
        let resp: XApiResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.unwrap()[0].text.contains("🦀"));
    }

    #[test]
    fn twitter_invalid_json() {
        let result = serde_json::from_str::<XApiResponse>("{bad");
        assert!(result.is_err());
    }

    #[test]
    fn twitter_completely_empty() {
        // Bare empty object should still parse (all fields optional)
        let json = r#"{}"#;
        let resp: XApiResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_none());
    }
}

// ============================================================================
// 8. arXiv — parse_atom_feed and extract_arxiv_id
// ============================================================================

mod arxiv_resilience {
    use super::super::arxiv::ArxivSource;

    #[test]
    fn arxiv_empty_xml() {
        let source = ArxivSource::new();
        let entries = source.parse_atom_feed("");
        assert!(entries.is_empty());
    }

    #[test]
    fn arxiv_no_entry_tags() {
        let source = ArxivSource::new();
        let xml = "<feed><title>arXiv</title></feed>";
        let entries = source.parse_atom_feed(xml);
        assert!(entries.is_empty());
    }

    #[test]
    fn arxiv_entry_missing_id() {
        let source = ArxivSource::new();
        let xml = r#"
        <feed>
        <entry>
            <title>Paper</title>
            <summary>Abstract</summary>
        </entry>
        </feed>
        "#;
        let entries = source.parse_atom_feed(xml);
        // Should skip entries without id
        assert!(entries.is_empty());
    }

    #[test]
    fn arxiv_entry_missing_title() {
        let source = ArxivSource::new();
        let xml = r#"
        <feed>
        <entry>
            <id>http://arxiv.org/abs/2401.00001v1</id>
            <summary>Abstract</summary>
        </entry>
        </feed>
        "#;
        let entries = source.parse_atom_feed(xml);
        // Should skip entries without title
        assert!(entries.is_empty());
    }

    #[test]
    fn arxiv_entry_empty_summary() {
        let source = ArxivSource::new();
        let xml = r#"
        <feed>
        <entry>
            <id>http://arxiv.org/abs/2401.00001v1</id>
            <title>Valid Title</title>
            <summary></summary>
        </entry>
        </feed>
        "#;
        let entries = source.parse_atom_feed(xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].summary, "");
    }

    #[test]
    fn arxiv_multiple_authors() {
        let source = ArxivSource::new();
        let xml = r#"
        <feed>
        <entry>
            <id>http://arxiv.org/abs/2401.00001v1</id>
            <title>Multi-author paper</title>
            <summary>Test</summary>
            <author><name>Alice</name></author>
            <author><name>Bob</name></author>
            <author><name>Carol</name></author>
        </entry>
        </feed>
        "#;
        let entries = source.parse_atom_feed(xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].authors, vec!["Alice", "Bob", "Carol"]);
    }

    #[test]
    fn arxiv_unicode_title_and_summary() {
        let source = ArxivSource::new();
        let xml = r#"
        <feed>
        <entry>
            <id>http://arxiv.org/abs/2401.99999v1</id>
            <title>量子コンピューティング 🔬</title>
            <summary>Résumé de l'article avec des accents</summary>
        </entry>
        </feed>
        "#;
        let entries = source.parse_atom_feed(xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "量子コンピューティング 🔬");
        assert!(entries[0].summary.contains("Résumé"));
    }

    #[test]
    fn arxiv_multiple_categories() {
        let source = ArxivSource::new();
        let xml = r#"
        <feed>
        <entry>
            <id>http://arxiv.org/abs/2401.00002v1</id>
            <title>Cross-discipline paper</title>
            <summary>Test</summary>
            <category term="cs.AI"/>
            <category term="cs.LG"/>
            <category term="stat.ML"/>
        </entry>
        </feed>
        "#;
        let entries = source.parse_atom_feed(xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].categories, vec!["cs.AI", "cs.LG", "stat.ML"]);
    }

    #[test]
    fn arxiv_malformed_xml_no_closing_entry() {
        let source = ArxivSource::new();
        // Entry without closing tag — parser should handle gracefully
        let xml = r#"
        <feed>
        <entry>
            <id>http://arxiv.org/abs/2401.00003v1</id>
            <title>Orphan Entry</title>
            <summary>No closing entry tag</summary>
        "#;
        // Should not panic, may or may not find an entry (entry_end falls to block.len())
        let entries = source.parse_atom_feed(xml);
        // The split produces a block, find("</entry>") returns None, uses block.len()
        // id and title present => should parse
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn arxiv_oversized_content() {
        let source = ArxivSource::new();
        let big_summary = "a".repeat(15_000);
        let xml = format!(
            r#"<feed>
            <entry>
                <id>http://arxiv.org/abs/2401.00004v1</id>
                <title>Big Paper</title>
                <summary>{}</summary>
            </entry>
            </feed>"#,
            big_summary
        );
        let entries = source.parse_atom_feed(&xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].summary.len(), 15_000);
    }

    #[test]
    fn arxiv_extract_id_no_version() {
        assert_eq!(
            ArxivSource::extract_arxiv_id("http://arxiv.org/abs/2401.12345"),
            "2401.12345"
        );
    }

    #[test]
    fn arxiv_extract_id_with_version() {
        assert_eq!(
            ArxivSource::extract_arxiv_id("http://arxiv.org/abs/2401.12345v3"),
            "2401.12345"
        );
    }

    #[test]
    fn arxiv_extract_id_empty_string() {
        // Empty URL: rsplit('/') returns "" so result is ""
        assert_eq!(ArxivSource::extract_arxiv_id(""), "");
    }

    #[test]
    fn arxiv_extract_id_no_slash() {
        assert_eq!(ArxivSource::extract_arxiv_id("2401.12345v2"), "2401.12345");
    }

    #[test]
    fn arxiv_max_entries_limit() {
        let source = ArxivSource::new();
        // Generate more than MAX_ENTRIES_PER_QUERY (200) entries
        let mut xml = String::from("<feed>");
        for i in 0..250 {
            xml.push_str(&format!(
                "<entry><id>http://arxiv.org/abs/test.{}</id><title>Paper {}</title><summary>S</summary></entry>",
                i, i
            ));
        }
        xml.push_str("</feed>");

        let entries = source.parse_atom_feed(&xml);
        assert!(
            entries.len() <= 200,
            "Should cap at MAX_ENTRIES_PER_QUERY (200), got {}",
            entries.len()
        );
    }
}

// ============================================================================
// 9. RSS — parse_feed, parse_rss_feed, parse_atom_feed, helper functions
// ============================================================================

mod rss_resilience {
    use super::super::rss::RssSource;

    #[test]
    fn rss_empty_xml() {
        let source = RssSource::new();
        let entries = source.parse_feed("", "https://example.com/feed");
        assert!(entries.is_empty());
    }

    #[test]
    fn rss_no_items() {
        let source = RssSource::new();
        let xml = r#"<?xml version="1.0"?><rss><channel><title>Empty Feed</title></channel></rss>"#;
        let entries = source.parse_rss_feed(xml, "https://example.com/feed");
        assert!(entries.is_empty());
    }

    #[test]
    fn rss_item_missing_title() {
        let source = RssSource::new();
        let xml = r#"<rss><channel><title>Feed</title>
            <item><link>https://example.com/1</link><description>No title</description></item>
        </channel></rss>"#;
        let entries = source.parse_rss_feed(xml, "https://example.com/feed");
        // Empty title => skipped
        assert!(entries.is_empty());
    }

    #[test]
    fn rss_item_missing_link() {
        let source = RssSource::new();
        let xml = r#"<rss><channel><title>Feed</title>
            <item><title>No Link</title><description>Orphan</description></item>
        </channel></rss>"#;
        let entries = source.parse_rss_feed(xml, "https://example.com/feed");
        // Empty link => skipped
        assert!(entries.is_empty());
    }

    #[test]
    fn rss_item_with_guid_fallback() {
        let source = RssSource::new();
        let xml = r#"<rss><channel><title>Feed</title>
            <item>
                <title>Has Guid</title>
                <guid>https://example.com/guid-link</guid>
                <description>Uses guid as link</description>
            </item>
        </channel></rss>"#;
        let entries = source.parse_rss_feed(xml, "https://example.com/feed");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].link, "https://example.com/guid-link");
    }

    #[test]
    fn rss_html_entities_in_title() {
        let source = RssSource::new();
        let xml = r#"<rss><channel><title>Feed</title>
            <item>
                <title>Tom &amp; Jerry &lt;3&gt;</title>
                <link>https://example.com/tj</link>
            </item>
        </channel></rss>"#;
        let entries = source.parse_rss_feed(xml, "https://example.com/feed");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Tom & Jerry <3>");
    }

    #[test]
    fn rss_cdata_description() {
        let source = RssSource::new();
        let xml = r#"<rss><channel><title>Feed</title>
            <item>
                <title>CDATA Test</title>
                <link>https://example.com/cd</link>
                <description><![CDATA[<p>HTML inside CDATA</p>]]></description>
            </item>
        </channel></rss>"#;
        let entries = source.parse_rss_feed(xml, "https://example.com/feed");
        assert_eq!(entries.len(), 1);
        // After strip_html + decode, should be clean text
        assert_eq!(entries[0].description, "HTML inside CDATA");
    }

    #[test]
    fn rss_content_encoded_tag() {
        let source = RssSource::new();
        let xml = r#"<rss><channel><title>Feed</title>
            <item>
                <title>Content Encoded</title>
                <link>https://example.com/ce</link>
                <content:encoded><![CDATA[<div>Full article text</div>]]></content:encoded>
            </item>
        </channel></rss>"#;
        let entries = source.parse_rss_feed(xml, "https://example.com/feed");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].description, "Full article text");
    }

    #[test]
    fn rss_unicode_content() {
        let source = RssSource::new();
        let xml = r#"<rss><channel><title>Feed</title>
            <item>
                <title>日本語の記事 🇯🇵</title>
                <link>https://example.com/jp</link>
                <description>本文 テスト العربية</description>
            </item>
        </channel></rss>"#;
        let entries = source.parse_rss_feed(xml, "https://example.com/feed");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "日本語の記事 🇯🇵");
    }

    #[test]
    fn rss_oversized_description_truncated() {
        let source = RssSource::new();
        let big_desc = "x".repeat(150_000);
        let xml = format!(
            r#"<rss><channel><title>Feed</title>
            <item>
                <title>Big</title>
                <link>https://example.com/big</link>
                <description>{}</description>
            </item>
            </channel></rss>"#,
            big_desc
        );
        let entries = source.parse_rss_feed(&xml, "https://example.com/feed");
        assert_eq!(entries.len(), 1);
        // MAX_ITEM_CONTENT_LEN = 100_000
        assert!(entries[0].description.len() <= 100_000);
    }

    #[test]
    fn rss_max_items_per_feed() {
        let source = RssSource::new();
        let mut xml = String::from("<rss><channel><title>Feed</title>");
        for i in 0..250 {
            xml.push_str(&format!(
                "<item><title>Item {}</title><link>https://example.com/{}</link></item>",
                i, i
            ));
        }
        xml.push_str("</channel></rss>");
        let entries = source.parse_rss_feed(&xml, "https://example.com/feed");
        assert!(
            entries.len() <= 200,
            "Should cap at MAX_ITEMS_PER_FEED (200), got {}",
            entries.len()
        );
    }

    // --- Atom feed tests ---

    #[test]
    fn atom_empty_xml() {
        let source = RssSource::new();
        let entries = source.parse_atom_feed("", "https://example.com/atom");
        assert!(entries.is_empty());
    }

    #[test]
    fn atom_no_entries() {
        let source = RssSource::new();
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Empty</title></feed>"#;
        let entries = source.parse_atom_feed(xml, "https://example.com/atom");
        assert!(entries.is_empty());
    }

    #[test]
    fn atom_entry_missing_title() {
        let source = RssSource::new();
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
            <title>Feed</title>
            <entry>
                <link href="https://example.com/1" rel="alternate"/>
                <id>1</id>
                <summary>No title</summary>
            </entry>
        </feed>"#;
        let entries = source.parse_atom_feed(xml, "https://example.com/atom");
        // Empty title => skipped
        assert!(entries.is_empty());
    }

    #[test]
    fn atom_link_extraction() {
        let source = RssSource::new();
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
            <title>Feed</title>
            <entry>
                <title>Test</title>
                <link href="https://example.com/alt" rel="alternate"/>
                <link href="https://example.com/self" rel="self"/>
                <id>urn:1</id>
            </entry>
        </feed>"#;
        let entries = source.parse_atom_feed(xml, "https://example.com/atom");
        assert_eq!(entries.len(), 1);
        // Should prefer alternate link
        assert_eq!(entries[0].link, "https://example.com/alt");
    }

    #[test]
    fn atom_oversized_content() {
        let source = RssSource::new();
        let big_content = "y".repeat(150_000);
        let xml = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom">
            <title>Feed</title>
            <entry>
                <title>Big</title>
                <link href="https://example.com/big" rel="alternate"/>
                <id>urn:big</id>
                <content>{}</content>
            </entry>
            </feed>"#,
            big_content
        );
        let entries = source.parse_atom_feed(&xml, "https://example.com/atom");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].description.len() <= 100_000);
    }

    // --- Helper function tests ---

    #[test]
    fn decode_entities_all() {
        assert_eq!(
            RssSource::decode_html_entities("&amp;&lt;&gt;&quot;&apos;&#39;&nbsp;"),
            "&<>\"'' "
        );
    }

    #[test]
    fn decode_entities_no_entities() {
        assert_eq!(RssSource::decode_html_entities("plain text"), "plain text");
    }

    #[test]
    fn decode_entities_empty() {
        assert_eq!(RssSource::decode_html_entities(""), "");
    }

    #[test]
    fn strip_html_empty() {
        assert_eq!(RssSource::strip_html(""), "");
    }

    #[test]
    fn strip_html_no_tags() {
        assert_eq!(RssSource::strip_html("just text"), "just text");
    }

    #[test]
    fn strip_html_nested_tags() {
        assert_eq!(
            RssSource::strip_html("<div><p><strong>deep</strong> nesting</p></div>"),
            "deep nesting"
        );
    }

    #[test]
    fn strip_html_unclosed_tag() {
        // Unclosed tag: everything after '<' is consumed until next '>'
        let result = RssSource::strip_html("text <broken");
        assert_eq!(result, "text");
    }

    #[test]
    fn strip_html_whitespace_collapse() {
        assert_eq!(
            RssSource::strip_html("<p>   lots   of   space   </p>"),
            "lots of space"
        );
    }

    #[test]
    fn generate_id_deterministic() {
        let id1 = RssSource::generate_id("https://example.com/article");
        let id2 = RssSource::generate_id("https://example.com/article");
        assert_eq!(id1, id2);
        assert!(id1.starts_with("rss_"));
    }

    #[test]
    fn generate_id_different_inputs() {
        let id1 = RssSource::generate_id("a");
        let id2 = RssSource::generate_id("b");
        assert_ne!(id1, id2);
    }

    #[test]
    fn generate_id_empty_string() {
        let id = RssSource::generate_id("");
        assert!(id.starts_with("rss_"));
    }

    #[test]
    fn feed_type_detection_rss() {
        let source = RssSource::new();
        let rss_xml = r#"<rss version="2.0"><channel><title>RSS</title>
            <item><title>Test</title><link>http://x</link></item>
        </channel></rss>"#;
        let entries = source.parse_feed(rss_xml, "http://example.com/feed");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn feed_type_detection_atom() {
        let source = RssSource::new();
        let atom_xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
            <title>Atom</title>
            <entry><title>Test</title><link href="http://x" rel="alternate"/><id>1</id></entry>
        </feed>"#;
        let entries = source.parse_feed(atom_xml, "http://example.com/atom");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn feed_type_detection_atom_by_entry_tag() {
        let source = RssSource::new();
        // Atom-like feed without proper namespace, detected by <entry> presence
        let xml = r#"<feed><title>Quasi-Atom</title>
            <entry><title>Test</title><link href="http://x" rel="alternate"/><id>1</id></entry>
        </feed>"#;
        let entries = source.parse_feed(xml, "http://example.com/atom");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn extract_tag_missing() {
        assert!(RssSource::extract_tag("<x>val</x>", "missing").is_none());
    }

    #[test]
    fn extract_tag_empty_content() {
        let result = RssSource::extract_tag("<title></title>", "title");
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn extract_tag_with_attributes() {
        // Tags like <title type="html"> should still extract content
        let result = RssSource::extract_tag(r#"<title type="html">Content</title>"#, "title");
        assert_eq!(result, Some("Content".to_string()));
    }
}

// ============================================================================
// 10. YouTube — parse_atom_feed, extract_tag, extract_attr
// ============================================================================

mod youtube_resilience {
    use super::super::youtube::YouTubeSource;

    #[test]
    fn youtube_empty_xml() {
        let source = YouTubeSource::new();
        let entries = source.parse_atom_feed("", "TestChannel").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn youtube_no_entries() {
        let source = YouTubeSource::new();
        let xml = r#"<feed><title>Empty Channel</title></feed>"#;
        let entries = source.parse_atom_feed(xml, "TestChannel").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn youtube_entry_missing_video_id() {
        let source = YouTubeSource::new();
        let xml = r#"<feed>
            <entry><title>No ID</title></entry>
        </feed>"#;
        let entries = source.parse_atom_feed(xml, "TestChannel").unwrap();
        // Empty video_id => skipped
        assert!(entries.is_empty());
    }

    #[test]
    fn youtube_entry_missing_title() {
        let source = YouTubeSource::new();
        let xml = r#"<feed>
            <entry><yt:videoId>abc</yt:videoId></entry>
        </feed>"#;
        let entries = source.parse_atom_feed(xml, "TestChannel").unwrap();
        // Empty title => skipped
        assert!(entries.is_empty());
    }

    #[test]
    fn youtube_minimal_entry() {
        let source = YouTubeSource::new();
        let xml = r#"<feed>
            <entry>
                <yt:videoId>vid1</yt:videoId>
                <title>Minimal Video</title>
            </entry>
        </feed>"#;
        let entries = source.parse_atom_feed(xml, "TestChannel").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].video_id, "vid1");
        assert_eq!(entries[0].title, "Minimal Video");
        assert!(entries[0].description.is_empty());
        assert_eq!(entries[0].views, 0);
        assert_eq!(entries[0].star_rating, 0.0);
    }

    #[test]
    fn youtube_unicode_title_and_description() {
        let source = YouTubeSource::new();
        let xml = r#"<feed>
            <entry>
                <yt:videoId>vid2</yt:videoId>
                <title>量子力学 🔬 개요</title>
                <media:description>Описание на русском</media:description>
            </entry>
        </feed>"#;
        let entries = source.parse_atom_feed(xml, "TestChannel").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "量子力学 🔬 개요");
        assert!(entries[0].description.contains("русском"));
    }

    #[test]
    fn youtube_zero_views_and_rating() {
        let source = YouTubeSource::new();
        let xml = r#"<feed>
            <entry>
                <yt:videoId>vid3</yt:videoId>
                <title>Unpopular</title>
                <media:statistics views="0"/>
                <media:starRating average="0.0"/>
            </entry>
        </feed>"#;
        let entries = source.parse_atom_feed(xml, "TestChannel").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].views, 0);
        assert_eq!(entries[0].star_rating, 0.0);
    }

    #[test]
    fn youtube_malformed_views_attr() {
        let source = YouTubeSource::new();
        let xml = r#"<feed>
            <entry>
                <yt:videoId>vid4</yt:videoId>
                <title>Bad Stats</title>
                <media:statistics views="notanumber"/>
            </entry>
        </feed>"#;
        let entries = source.parse_atom_feed(xml, "TestChannel").unwrap();
        assert_eq!(entries.len(), 1);
        // Invalid parse defaults to 0
        assert_eq!(entries[0].views, 0);
    }

    #[test]
    fn youtube_large_view_count() {
        let source = YouTubeSource::new();
        let xml = r#"<feed>
            <entry>
                <yt:videoId>vid5</yt:videoId>
                <title>Viral</title>
                <media:statistics views="999999999999"/>
            </entry>
        </feed>"#;
        let entries = source.parse_atom_feed(xml, "TestChannel").unwrap();
        assert_eq!(entries[0].views, 999_999_999_999);
    }

    #[test]
    fn youtube_oversized_description() {
        let source = YouTubeSource::new();
        let big_desc = "d".repeat(20_000);
        let xml = format!(
            r#"<feed>
            <entry>
                <yt:videoId>vid6</yt:videoId>
                <title>Big Description</title>
                <media:description>{}</media:description>
            </entry>
            </feed>"#,
            big_desc
        );
        let entries = source.parse_atom_feed(&xml, "TestChannel").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].description.len(), 20_000);
    }

    #[test]
    fn youtube_multiple_entries() {
        let source = YouTubeSource::new();
        let mut xml = String::from("<feed><title>Channel</title>");
        for i in 0..50 {
            xml.push_str(&format!(
                "<entry><yt:videoId>v{}</yt:videoId><title>Video {}</title></entry>",
                i, i
            ));
        }
        xml.push_str("</feed>");
        let entries = source.parse_atom_feed(&xml, "TestChannel").unwrap();
        assert_eq!(entries.len(), 50);
    }

    // --- helper function tests ---

    #[test]
    fn extract_tag_missing() {
        assert!(super::super::youtube::extract_tag("no tags", "title").is_none());
    }

    #[test]
    fn extract_tag_cdata() {
        let result =
            super::super::youtube::extract_tag("<title><![CDATA[Hello]]></title>", "title");
        assert_eq!(result, Some("Hello".to_string()));
    }

    #[test]
    fn extract_tag_empty_content() {
        let result = super::super::youtube::extract_tag("<title></title>", "title");
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn extract_attr_missing_tag() {
        assert!(
            super::super::youtube::extract_attr("no tag", "media:statistics", "views").is_none()
        );
    }

    #[test]
    fn extract_attr_missing_attr() {
        let xml = r#"<media:statistics count="5"/>"#;
        assert!(super::super::youtube::extract_attr(xml, "media:statistics", "views").is_none());
    }
}

// ============================================================================
// 11. Product Hunt — parse_feed, extract_tag, extract_upvotes, extract_comments
// ============================================================================

mod producthunt_resilience {
    use super::super::producthunt::ProductHuntSource;

    #[test]
    fn ph_empty_xml() {
        let source = ProductHuntSource::new();
        let items = source.parse_feed("").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn ph_no_items() {
        let source = ProductHuntSource::new();
        let xml = r#"<rss><channel><title>PH</title></channel></rss>"#;
        let items = source.parse_feed(xml).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn ph_item_all_empty_fields() {
        let source = ProductHuntSource::new();
        let xml = r#"<rss><channel>
            <item>
                <title></title>
                <link></link>
                <description></description>
                <pubDate></pubDate>
            </item>
        </channel></rss>"#;
        let items = source.parse_feed(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].title.is_empty());
        assert!(items[0].upvotes.is_none());
        assert!(items[0].comments.is_none());
    }

    #[test]
    fn ph_unicode_title() {
        let source = ProductHuntSource::new();
        let xml = r#"<rss><channel>
            <item>
                <title>产品猎手 🚀 Продукт</title>
                <link>https://ph.com/test</link>
                <description>A test product</description>
                <pubDate>Mon, 01 Jan 2026 00:00:00 GMT</pubDate>
            </item>
        </channel></rss>"#;
        let items = source.parse_feed(xml).unwrap();
        assert_eq!(items[0].title, "产品猎手 🚀 Продукт");
    }

    #[test]
    fn ph_upvotes_extraction() {
        assert_eq!(
            super::super::producthunt::extract_upvotes("Check out 500 upvotes now"),
            Some(500)
        );
    }

    #[test]
    fn ph_upvotes_no_match() {
        assert_eq!(
            super::super::producthunt::extract_upvotes("no match here"),
            None
        );
    }

    #[test]
    fn ph_comments_extraction() {
        assert_eq!(
            super::super::producthunt::extract_comments("Wow 78 comments on this"),
            Some(78)
        );
    }

    #[test]
    fn ph_comments_no_match() {
        assert_eq!(super::super::producthunt::extract_comments("silence"), None);
    }

    #[test]
    fn ph_extract_upvotes_zero() {
        assert_eq!(
            super::super::producthunt::extract_upvotes("Only 0 upvotes"),
            Some(0)
        );
    }

    #[test]
    fn ph_extract_tag_basic() {
        assert_eq!(
            super::super::producthunt::extract_tag("<title>Hello</title>", "title"),
            Some("Hello".to_string())
        );
    }

    #[test]
    fn ph_extract_tag_cdata() {
        assert_eq!(
            super::super::producthunt::extract_tag(
                "<description><![CDATA[Wrapped content]]></description>",
                "description"
            ),
            Some("Wrapped content".to_string())
        );
    }

    #[test]
    fn ph_extract_tag_missing() {
        assert!(super::super::producthunt::extract_tag("<x>1</x>", "y").is_none());
    }

    #[test]
    fn ph_malformed_xml_no_closing_item() {
        let source = ProductHuntSource::new();
        // Item without closing tag
        let xml = r#"<rss><channel>
            <item>
                <title>Orphan</title>
                <link>http://x</link>
                <description>desc</description>
                <pubDate>Mon, 01 Jan 2026 00:00:00 GMT</pubDate>
        "#;
        // Should not panic
        let items = source.parse_feed(xml).unwrap();
        // find("</item>") returns None => breaks out of loop
        assert!(items.is_empty());
    }

    #[test]
    fn ph_multiple_items() {
        let source = ProductHuntSource::new();
        let mut xml = String::from("<rss><channel>");
        for i in 0..20 {
            xml.push_str(&format!(
                "<item><title>Product {}</title><link>http://ph.com/{}</link><description>Desc</description><pubDate>date</pubDate></item>",
                i, i
            ));
        }
        xml.push_str("</channel></rss>");
        let items = source.parse_feed(&xml).unwrap();
        assert_eq!(items.len(), 20);
    }
}

// ============================================================================
// 12. SourceConfig and SourceError
// ============================================================================

#[test]
fn source_config_serialization_roundtrip() {
    let config = SourceConfig {
        enabled: false,
        max_items: 0,
        fetch_interval_secs: 0,
        custom: Some(serde_json::json!({"key": "value"})),
    };
    let json = serde_json::to_string(&config).unwrap();
    let back: SourceConfig = serde_json::from_str(&json).unwrap();
    assert!(!back.enabled);
    assert_eq!(back.max_items, 0);
    assert_eq!(back.fetch_interval_secs, 0);
    assert!(back.custom.is_some());
}

#[test]
fn source_config_zero_values() {
    let config = SourceConfig {
        enabled: false,
        max_items: 0,
        fetch_interval_secs: 0,
        custom: None,
    };
    assert_eq!(config.max_items, 0);
    assert_eq!(config.fetch_interval_secs, 0);
}

#[test]
fn source_error_display_messages() {
    let err = SourceError::Network("timeout".to_string());
    assert!(err.to_string().contains("timeout"));

    let err = SourceError::Parse("unexpected token".to_string());
    assert!(err.to_string().contains("unexpected token"));

    let err = SourceError::RateLimited("test source".to_string());
    assert!(err.to_string().contains("Rate limited"));

    let err = SourceError::Forbidden("auth issue".to_string());
    assert!(err.to_string().contains("Forbidden"));

    let err = SourceError::Disabled;
    assert!(err.to_string().contains("disabled"));

    let err = SourceError::Other("custom".to_string());
    assert!(err.to_string().contains("custom"));
}

#[test]
fn source_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(SourceError::Network("test".to_string()));
    assert!(err.to_string().contains("test"));
}

// ============================================================================
// 13. SourceRegistry — empty, register, lookup
// ============================================================================

#[test]
fn registry_empty() {
    let reg = SourceRegistry::new();
    assert_eq!(reg.count(), 0);
    assert!(reg.sources().is_empty());
}

#[test]
fn registry_default_is_empty() {
    let reg = SourceRegistry::default();
    assert_eq!(reg.count(), 0);
}

// ============================================================================
// 14. Cross-adapter: SourceItem metadata robustness
// ============================================================================

#[test]
fn source_item_metadata_deeply_nested() {
    let meta = serde_json::json!({
        "level1": {
            "level2": {
                "level3": [1, 2, 3]
            }
        }
    });
    let item = SourceItem::new("test", "1", "T").with_metadata(meta);
    let m = item.metadata.as_ref().unwrap();
    assert_eq!(m["level1"]["level2"]["level3"][0], 1);
}

#[test]
fn source_item_metadata_large() {
    // Metadata with 1000 keys
    let mut map = serde_json::Map::new();
    for i in 0..1000 {
        map.insert(format!("key_{}", i), serde_json::json!(i));
    }
    let meta = serde_json::Value::Object(map);
    let item = SourceItem::new("test", "1", "T").with_metadata(meta);
    assert!(item.metadata.is_some());
}

#[test]
fn source_item_chained_builders() {
    let item = SourceItem::new("type", "id", "title")
        .with_url(Some("https://example.com".to_string()))
        .with_content("body".to_string())
        .with_metadata(serde_json::json!({}));

    assert_eq!(item.source_type, "type");
    assert_eq!(item.source_id, "id");
    assert_eq!(item.title, "title");
    assert_eq!(item.url.as_deref(), Some("https://example.com"));
    assert_eq!(item.content, "body");
    assert!(item.metadata.is_some());
}

#[test]
fn source_item_replace_content() {
    let item = SourceItem::new("t", "1", "T")
        .with_content("first".to_string())
        .with_content("second".to_string());
    assert_eq!(item.content, "second");
}

#[test]
fn source_item_replace_url() {
    let item = SourceItem::new("t", "1", "T")
        .with_url(Some("http://a".to_string()))
        .with_url(Some("http://b".to_string()));
    assert_eq!(item.url.as_deref(), Some("http://b"));
}

#[test]
fn source_item_url_none_then_some() {
    let item = SourceItem::new("t", "1", "T")
        .with_url(None)
        .with_url(Some("http://x".to_string()));
    assert_eq!(item.url.as_deref(), Some("http://x"));
}
