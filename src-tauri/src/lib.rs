// FASTEMBED DISABLED: ONNX linking issues on Windows - using OpenAI only
// use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tracing::{debug, error, info, warn};

/// Shared HTTP client for embedding API calls (reused across requests)
static EMBEDDING_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(90))
        .user_agent("4DA/1.0")
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

mod ace;
mod ace_commands;
mod analysis;
mod anomaly;
mod attention;
mod context_commands;
mod context_engine;
mod db;
mod digest;
mod digest_commands;
mod document_index;
pub mod extractors;
mod handoff;
mod health;
mod job_queue;
mod job_queue_commands;
mod knowledge_decay;
mod llm;
mod monitoring;
mod monitoring_commands;
mod ollama;
mod predictive;
mod project_health;
pub mod query;
mod reverse_relevance;
mod scoring;
pub(crate) mod scoring_config;
mod semantic_diff;
mod settings;
mod settings_commands;
mod signal_chains;
mod signals;
mod source_config;
mod source_fetching;
mod sources;
mod temporal;
mod tts;
mod void_commands;
mod void_engine;

use context_engine::ContextEngine;
use db::Database;
use settings::SettingsManager;
use source_fetching::{
    fill_cache_background, load_github_languages_from_settings, load_rss_feeds_from_settings,
    load_twitter_settings, load_youtube_channels_from_settings,
};
use sources::{
    arxiv::ArxivSource, github::GitHubSource, hackernews::HackerNewsSource,
    producthunt::ProductHuntSource, reddit::RedditSource, rss::RssSource, twitter::TwitterSource,
    youtube::YouTubeSource, SourceRegistry,
};

// ============================================================================
// Utility Functions
// ============================================================================

/// Safely truncate a string to a maximum number of characters (UTF-8 aware)
/// This avoids panics when slicing multi-byte characters like Cyrillic, Chinese, etc.
pub(crate) fn truncate_utf8(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFile {
    pub path: String,
    pub content: String,
    pub lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSettings {
    pub configured_dirs: Vec<String>,
    pub active_dirs: Vec<String>,
    pub using_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedItem {
    pub id: u64,
    pub title: String,
    pub url: Option<String>,
    pub content: String, // Scraped article content or HN text
}

/// Generic source item for multi-source support
#[derive(Debug, Clone)]
struct GenericSourceItem {
    pub id: u64,
    pub source_id: String,
    pub source_type: String,
    pub title: String,
    pub url: Option<String>,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct HNStory {
    id: u64,
    title: Option<String>,
    url: Option<String>,
    text: Option<String>, // For Ask HN / Show HN / text posts
}

/// Relevance match between an HN item and context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevanceMatch {
    pub source_file: String,
    pub matched_text: String,
    pub similarity: f32,
}

/// Detailed breakdown of score components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub context_score: f32,
    pub interest_score: f32,
    #[serde(default)]
    pub keyword_score: f32,
    pub ace_boost: f32,
    pub affinity_mult: f32,
    pub anti_penalty: f32,
    #[serde(default = "default_freshness")]
    pub freshness_mult: f32,
    #[serde(default)]
    pub feedback_boost: f32,
    #[serde(default)]
    pub source_quality_boost: f32,
    pub confidence_by_signal: std::collections::HashMap<String, f32>,
    /// Number of independent signal axes that confirmed relevance (0-4)
    #[serde(default)]
    pub signal_count: u8,
    /// Names of confirmed signal axes (e.g. ["context", "ace"])
    #[serde(default)]
    pub confirmed_signals: Vec<String>,
    /// Multiplier applied by confirmation gate
    #[serde(default = "default_confirmation_mult")]
    pub confirmation_mult: f32,
}

fn default_freshness() -> f32 {
    1.0
}

fn default_confirmation_mult() -> f32 {
    1.0
}

/// Full relevance result for a source item (HN, arXiv, Reddit, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRelevance {
    pub id: u64,
    pub title: String,
    pub url: Option<String>,
    pub top_score: f32,
    pub matches: Vec<RelevanceMatch>,
    pub relevant: bool,
    /// Score from context files (what you're working on)
    #[serde(default)]
    pub context_score: f32,
    /// Score from explicit interests (what you care about)
    #[serde(default)]
    pub interest_score: f32,
    /// Whether this item was filtered by an exclusion
    #[serde(default)]
    pub excluded: bool,
    /// The exclusion that blocked this item (if any)
    #[serde(default)]
    pub excluded_by: Option<String>,
    /// Source type (hackernews, arxiv, reddit)
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// Human-readable explanation of why this item was surfaced
    #[serde(default)]
    pub explanation: Option<String>,
    /// Overall confidence score (0.0-1.0)
    #[serde(default)]
    pub confidence: Option<f32>,
    /// Detailed score breakdown for debugging
    #[serde(default)]
    pub score_breakdown: Option<ScoreBreakdown>,
    /// Signal classification type (security_alert, breaking_change, etc.)
    #[serde(default)]
    pub signal_type: Option<String>,
    /// Signal priority level (critical, high, medium, low)
    #[serde(default)]
    pub signal_priority: Option<String>,
    /// Suggested action based on signal classification
    #[serde(default)]
    pub signal_action: Option<String>,
    /// Keywords that triggered the classification
    #[serde(default)]
    pub signal_triggers: Option<Vec<String>>,
    /// How many similar items were grouped under this representative (topic dedup)
    #[serde(default)]
    pub similar_count: u32,
    /// Titles of grouped similar items
    #[serde(default)]
    pub similar_titles: Vec<String>,
    /// Whether this item was injected by the serendipity engine (anti-bubble)
    #[serde(default)]
    pub serendipity: bool,
}

fn default_source_type() -> String {
    "hackernews".to_string()
}

/// Status update for the UI (sent via events)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisStatus {
    pub stage: String,
    pub progress: f32,
    pub message: String,
    pub items_processed: usize,
    pub items_total: usize,
}

/// Background analysis state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisState {
    pub running: bool,
    pub completed: bool,
    pub error: Option<String>,
    pub results: Option<Vec<SourceRelevance>>,
    /// When analysis started (unix timestamp seconds)
    #[serde(default)]
    pub started_at: Option<i64>,
    /// When analysis last completed successfully (ISO string for DB query compat)
    #[serde(default)]
    pub last_completed_at: Option<String>,
}

/// Maximum analysis duration in seconds before auto-timeout
const ANALYSIS_TIMEOUT_SECS: i64 = 300;

/// Shared abort flag for analysis cancellation (separate from AnalysisState to avoid mutex)
static ANALYSIS_ABORT: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

pub(crate) fn get_analysis_abort() -> &'static Arc<AtomicBool> {
    &ANALYSIS_ABORT
}

/// LLM judgment attached to a relevance result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMJudgment {
    pub relevant: bool,
    pub confidence: f32,
    pub reasoning: String,
    pub key_connections: Vec<String>,
}

/// Enhanced relevance result with optional LLM judgment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedRelevance {
    pub id: u64,
    pub title: String,
    pub url: Option<String>,
    pub embedding_score: f32,
    pub matches: Vec<RelevanceMatch>,
    pub embedding_relevant: bool,
    /// LLM re-ranking judgment (if enabled)
    pub llm_judgment: Option<LLMJudgment>,
    /// Final relevance after both stages
    pub final_relevant: bool,
}

// ============================================================================
// Embeddings - supports OpenAI and Ollama
// ============================================================================

/// Generate embeddings for a list of texts
/// Supports OpenAI (text-embedding-3-small) and Ollama (nomic-embed-text)
/// Provider is determined by settings - uses same provider as LLM when possible
pub(crate) async fn embed_texts(texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    // Get settings to determine provider - clone inside scope so MutexGuard drops before await
    let llm_settings = {
        let settings = get_settings_manager().lock();
        settings.get().llm.clone()
    };

    match llm_settings.provider.as_str() {
        "openai" => {
            let api_key = llm_settings.api_key.clone();
            let texts = texts.to_vec();
            retry_with_backoff("embed_openai", 2, || {
                let key = api_key.clone();
                let t = texts.clone();
                async move { embed_texts_openai(&t, &key).await }
            })
            .await
        }
        "ollama" => {
            let base_url = llm_settings.base_url.clone();
            let texts = texts.to_vec();
            retry_with_backoff("embed_ollama", 2, || {
                let url = base_url.clone();
                let t = texts.clone();
                async move { embed_texts_ollama(&t, &url).await }
            })
            .await
        }
        "anthropic" => {
            // Anthropic doesn't have embeddings API - use dedicated OpenAI key or fallback to Ollama
            if !llm_settings.openai_api_key.is_empty() {
                let api_key = llm_settings.openai_api_key.clone();
                let texts = texts.to_vec();
                return retry_with_backoff("embed_openai_anthropic_fallback", 2, || {
                    let key = api_key.clone();
                    let t = texts.clone();
                    async move { embed_texts_openai(&t, &key).await }
                })
                .await;
            }
            // Try Ollama as fallback
            if let Some(base_url) = &llm_settings.base_url {
                if !base_url.is_empty() {
                    let url = Some(base_url.clone());
                    let texts_vec = texts.to_vec();
                    if let Ok(result) =
                        retry_with_backoff("embed_ollama_anthropic_fallback", 2, || {
                            let u = url.clone();
                            let t = texts_vec.clone();
                            async move { embed_texts_ollama(&t, &u).await }
                        })
                        .await
                    {
                        return Ok(result);
                    }
                }
            }
            // Try default Ollama
            let texts = texts.to_vec();
            retry_with_backoff("embed_ollama_default", 2, || {
                let t = texts.clone();
                async move { embed_texts_ollama(&t, &None).await }
            })
            .await
        }
        _ => Err(format!(
            "Unknown provider: {}. Please configure OpenAI or Ollama.",
            llm_settings.provider
        )),
    }
}

/// Generate embeddings using OpenAI API
async fn embed_texts_openai(texts: &[String], api_key: &str) -> Result<Vec<Vec<f32>>, String> {
    if api_key.is_empty() {
        return Err("OpenAI API key not configured".to_string());
    }

    let body = serde_json::json!({
        "model": "text-embedding-3-small",
        "input": texts,
        "dimensions": 384  // Match DB vec0 schema (384-dim MiniLM-compatible)
    });

    let response = EMBEDDING_CLIENT
        .post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI API request failed: {}", e))?;

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

    // Phase 5: Record usage from API response
    if let Some(usage) = json.get("usage") {
        let total_tokens = usage["total_tokens"].as_u64().unwrap_or(0);
        // text-embedding-3-small: $0.02 per 1M tokens = 0.002 cents per token
        let cost_cents = (total_tokens as f64 * 0.002 / 1000.0) as u64;
        let mut settings = get_settings_manager().lock();
        settings.record_usage(total_tokens, cost_cents);
    }

    let data = json["data"]
        .as_array()
        .ok_or_else(|| "Invalid OpenAI response: missing 'data' array".to_string())?;

    data.iter()
        .map(|item| {
            item["embedding"]
                .as_array()
                .ok_or_else(|| "Missing embedding in response".to_string())?
                .iter()
                .map(|v| {
                    v.as_f64()
                        .map(|f| f as f32)
                        .ok_or_else(|| "Invalid embedding value".to_string())
                })
                .collect::<Result<Vec<f32>, String>>()
        })
        .collect()
}

/// Target embedding dimensions matching DB vec0 schema
const TARGET_EMBEDDING_DIMS: usize = 384;

/// Truncate embedding to TARGET_EMBEDDING_DIMS and L2-normalize.
/// nomic-embed-text is a Matryoshka model so truncation preserves semantic quality.
fn truncate_and_normalize(mut embedding: Vec<f32>) -> Vec<f32> {
    if embedding.len() > TARGET_EMBEDDING_DIMS {
        embedding.truncate(TARGET_EMBEDDING_DIMS);
        // Re-normalize after truncation (Matryoshka requirement)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut embedding {
                *v /= norm;
            }
        }
    }
    embedding
}

/// Generate embeddings using Ollama API
async fn embed_texts_ollama(
    texts: &[String],
    base_url: &Option<String>,
) -> Result<Vec<Vec<f32>>, String> {
    let base = base_url.as_deref().unwrap_or("http://localhost:11434");

    if texts.is_empty() {
        return Ok(vec![]);
    }

    let batch_body = serde_json::json!({
        "model": "nomic-embed-text",
        "input": texts,
    });

    // Try batch API first (/api/embed) - supported since Ollama v0.1.26
    let batch_result = EMBEDDING_CLIENT
        .post(format!("{}/api/embed", base))
        .json(&batch_body)
        .send()
        .await;

    match batch_result {
        Ok(response) if response.status().is_success() => {
            // Batch succeeded - parse embeddings array
            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Ollama batch response: {}", e))?;

            let embeddings_array = json["embeddings"].as_array().ok_or_else(|| {
                "Invalid Ollama batch response: missing 'embeddings' array".to_string()
            })?;

            embeddings_array
                .iter()
                .map(|emb_val| {
                    let raw = emb_val
                        .as_array()
                        .ok_or_else(|| "Invalid embedding in batch response".to_string())?
                        .iter()
                        .map(|v| {
                            v.as_f64()
                                .map(|f| f as f32)
                                .ok_or_else(|| "Invalid embedding value".to_string())
                        })
                        .collect::<Result<Vec<f32>, String>>()?;
                    Ok(truncate_and_normalize(raw))
                })
                .collect()
        }
        Ok(response) => {
            // Batch endpoint returned an error - check for model-not-found
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 || body.contains("not found") {
                return Err("Embedding model 'nomic-embed-text' not found in Ollama. Run: ollama pull nomic-embed-text".to_string());
            }
            // Fall through to single-item fallback for other errors (old Ollama version)
            embed_texts_ollama_single(texts, base).await
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("connect") || msg.contains("refused") {
                return Err(format!(
                    "Cannot connect to Ollama at {}. Make sure Ollama is running (ollama serve).",
                    base
                ));
            }
            if msg.contains("timed out") || msg.contains("timeout") {
                return Err("Ollama embedding request timed out. The model may still be loading — try again shortly.".to_string());
            }
            // Fall through to single-item fallback
            embed_texts_ollama_single(texts, base).await
        }
    }
}

/// Fallback: embed one text at a time using the older /api/embeddings endpoint
async fn embed_texts_ollama_single(texts: &[String], base: &str) -> Result<Vec<Vec<f32>>, String> {
    let mut all_embeddings = Vec::with_capacity(texts.len());

    for text in texts {
        let single_body = serde_json::json!({
            "model": "nomic-embed-text",
            "prompt": text,
        });

        let response = EMBEDDING_CLIENT
            .post(format!("{}/api/embeddings", base))
            .json(&single_body)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("connect") || msg.contains("refused") {
                    format!(
                        "Cannot connect to Ollama at {}. Make sure Ollama is running (ollama serve).",
                        base
                    )
                } else if msg.contains("timed out") || msg.contains("timeout") {
                    "Ollama embedding timed out. The model may still be loading — try again.".to_string()
                } else {
                    format!(
                        "Ollama embedding request failed: {}. Make sure Ollama is running with 'nomic-embed-text' (run: ollama pull nomic-embed-text)",
                        e
                    )
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 || body.contains("not found") {
                return Err(
                    "Embedding model 'nomic-embed-text' not found. Run: ollama pull nomic-embed-text".to_string()
                );
            }
            return Err(format!("Ollama embedding error ({}): {}", status, body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        let raw = json["embedding"]
            .as_array()
            .ok_or_else(|| {
                "Invalid Ollama response: missing 'embedding' array. Is nomic-embed-text installed?"
                    .to_string()
            })?
            .iter()
            .map(|v| {
                v.as_f64()
                    .map(|f| f as f32)
                    .ok_or_else(|| "Invalid embedding value".to_string())
            })
            .collect::<Result<Vec<f32>, String>>()?;

        all_embeddings.push(truncate_and_normalize(raw));
    }

    Ok(all_embeddings)
}

/// Retry an async operation with exponential backoff.
/// Returns the first successful result, or the last error after max_retries.
async fn retry_with_backoff<F, Fut, T>(
    operation_name: &str,
    max_retries: u32,
    f: F,
) -> Result<T, String>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut last_error = String::new();
    for attempt in 0..=max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = e.clone();
                if attempt < max_retries {
                    let delay_secs = 3u64.pow(attempt); // 1s, 3s, 9s
                    tracing::warn!(
                        target: "4da::retry",
                        attempt = attempt + 1,
                        max = max_retries + 1,
                        delay_secs,
                        operation = operation_name,
                        error = %e,
                        "Retrying after error"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                }
            }
        }
    }
    Err(last_error)
}

/// Cosine similarity between two vectors
/// Compute L2 norm of a vector
#[inline]
pub(crate) fn vector_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Cosine similarity with precomputed norm for vector `a`
/// Use this in hot loops where you compare the same vector `a` against many `b` vectors
#[inline]
pub(crate) fn cosine_similarity_with_norm(a: &[f32], a_norm: f32, b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_b: f32 = vector_norm(b);
    if a_norm == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (a_norm * norm_b)
}

/// Cosine similarity between two vectors (used by tests; hot path uses cosine_similarity_with_norm)
#[allow(dead_code)]
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = vector_norm(a);
    let norm_b: f32 = vector_norm(b);

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Single-word topic keywords — O(1) lookup via HashSet
static SINGLE_WORD_TOPICS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "rust",
        "python",
        "javascript",
        "typescript",
        "go",
        "golang",
        "java",
        "cpp",
        "react",
        "vue",
        "angular",
        "svelte",
        "node",
        "deno",
        "bun",
        "ai",
        "ml",
        "neural",
        "gpt",
        "llm",
        "transformer",
        "database",
        "sql",
        "postgresql",
        "postgres",
        "mysql",
        "mongodb",
        "redis",
        "sqlite",
        "kubernetes",
        "k8s",
        "docker",
        "container",
        "aws",
        "azure",
        "gcp",
        "cloud",
        "api",
        "rest",
        "graphql",
        "grpc",
        "microservice",
        "crypto",
        "cryptocurrency",
        "bitcoin",
        "ethereum",
        "blockchain",
        "nft",
        "web3",
        "defi",
        "startup",
        "vc",
        "funding",
        "acquisition",
        "oss",
        "github",
        "git",
        "security",
        "vulnerability",
        "hack",
        "breach",
        "performance",
        "optimization",
        "scale",
        "scalability",
        "frontend",
        "backend",
        "fullstack",
        "devops",
        "sre",
        "linux",
        "unix",
        "windows",
        "macos",
        "mobile",
        "ios",
        "android",
        "flutter",
        "game",
        "gaming",
        "gamedev",
        "hardware",
        "chip",
        "semiconductor",
        "cpu",
        "gpu",
        "climate",
        "sustainability",
        "energy",
        "sports",
        "football",
        "basketball",
        "soccer",
        "politics",
        "election",
        "government",
    ]
    .into_iter()
    .collect()
});

/// Multi-word topic phrases — small enough for linear scan
static MULTI_WORD_TOPICS: &[&str] = &[
    "c++",
    "machine learning",
    "deep learning",
    "open source",
    "react native",
];

/// Extract topics/keywords from text for context matching
/// Returns lowercase keywords suitable for exclusion/interest matching
/// Optimized: O(1) HashSet lookup for single-word topics, linear scan only for multi-word phrases
pub(crate) fn extract_topics(title: &str, content: &str) -> Vec<String> {
    // Combine title and first part of content
    let text = format!(
        "{} {}",
        title,
        content.chars().take(500).collect::<String>()
    );
    let text_lower = text.to_lowercase();

    let mut topics = Vec::new();
    let mut seen = HashSet::new();

    // O(1) lookup for single-word topics: split into words, check each against HashSet
    for word in text_lower.split(|c: char| !c.is_alphanumeric() && c != '+' && c != '#') {
        if word.len() >= 2 && SINGLE_WORD_TOPICS.contains(word) && seen.insert(word.to_string()) {
            topics.push(word.to_string());
        }
    }

    // Linear scan for multi-word phrases (only ~5 entries)
    for &phrase in MULTI_WORD_TOPICS {
        if text_lower.contains(phrase) && seen.insert(phrase.to_string()) {
            topics.push(phrase.to_string());
        }
    }

    // Also extract capitalized words from title as potential topics
    for word in title.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
        if clean.len() > 2
            && clean
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
        {
            let lower = clean.to_lowercase();
            if !seen.contains(&lower)
                && ![
                    // Articles, conjunctions, prepositions
                    "the",
                    "and",
                    "for",
                    "how",
                    "why",
                    "what",
                    "show",
                    "ask",
                    "with",
                    "from",
                    "into",
                    "about",
                    "this",
                    "that",
                    "your",
                    "our",
                    "their",
                    "some",
                    "any",
                    "all",
                    "every",
                    "each",
                    "more",
                    "most",
                    "many",
                    "much",
                    "also",
                    "just",
                    "very",
                    "still",
                    "not",
                    "but",
                    "yet",
                    "here",
                    "there",
                    "when",
                    "where",
                    "will",
                    "can",
                    "should",
                    "could",
                    "would",
                    // Generic verbs / gerunds (capitalized in titles, useless as topics)
                    "using",
                    "building",
                    "working",
                    "making",
                    "getting",
                    "running",
                    "creating",
                    "developing",
                    "announcing",
                    "introducing",
                    "launching",
                    "deploying",
                    "implementing",
                    "understanding",
                    "exploring",
                    "discussing",
                    "comparing",
                    "improving",
                    "fixing",
                    "breaking",
                    "starting",
                    "looking",
                    "moving",
                    "keeping",
                    "finding",
                    "writing",
                    "reading",
                    "learning",
                    "teaching",
                    "testing",
                    "trying",
                    "adding",
                    "removing",
                    "setting",
                    "built",
                    "made",
                    "released",
                    // Generic adjectives / nouns
                    "new",
                    "best",
                    "first",
                    "free",
                    "fast",
                    "easy",
                    "simple",
                    "better",
                    "modern",
                    "full",
                    "real",
                    "good",
                    "great",
                    "top",
                    "key",
                    "big",
                    "small",
                    "open",
                    "way",
                    "part",
                    "time",
                    "year",
                    "week",
                    "month",
                    "day",
                    "thing",
                    "guide",
                    "tips",
                    "tool",
                    "tools",
                    "list",
                    "need",
                    "help",
                    "project",
                    "projects",
                    "update",
                    "version",
                ]
                .contains(&lower.as_str())
                && seen.insert(lower.clone())
            {
                topics.push(lower);
            }
        }
    }

    topics
}

/// Check if an item should be excluded based on user exclusions
/// Returns Some(exclusion) if blocked, None if allowed
pub(crate) fn check_exclusions(topics: &[String], exclusions: &[String]) -> Option<String> {
    for topic in topics {
        let topic_lower = topic.to_lowercase();
        for exclusion in exclusions {
            let exclusion_lower = exclusion.to_lowercase();
            if topic_lower.contains(&exclusion_lower) || exclusion_lower.contains(&topic_lower) {
                return Some(exclusion.clone());
            }
        }
    }
    None
}

// ============================================================================
// Centralized DB Path & Connection Helpers
// ============================================================================

/// Get the canonical path to the 4da.db database file.
/// Single source of truth — all connection opens should use this.
pub(crate) fn get_db_path() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.push("data");
    base.push("4da.db");
    base
}

/// Open a raw SQLite connection with proper configuration.
/// Registers sqlite-vec auto-extension and sets busy_timeout.
/// Use this for ad-hoc connection needs outside the Database struct.
pub(crate) fn open_db_connection() -> Result<rusqlite::Connection, String> {
    let db_path = get_db_path();

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    // Register sqlite-vec extension globally (idempotent)
    #[allow(clippy::missing_transmute_annotations)]
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    // Set busy_timeout to prevent "database is locked" errors
    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .map_err(|e| format!("Failed to set busy_timeout: {}", e))?;

    Ok(conn)
}

// ============================================================================
// Global Database (Lazy Initialized)
// ============================================================================

static DATABASE: OnceCell<Arc<Database>> = OnceCell::new();

pub(crate) fn get_database() -> Result<&'static Arc<Database>, String> {
    DATABASE.get_or_try_init(|| {
        let db_path = get_db_path();

        info!(target: "4da::db", path = ?db_path, "Initializing database");

        let db =
            Database::new(&db_path).map_err(|e| format!("Failed to initialize database: {}", e))?;

        // Register all sources at startup (enables source enable/disable enforcement)
        db.register_source("hackernews", "Hacker News").ok();
        db.register_source("arxiv", "arXiv").ok();
        db.register_source("reddit", "Reddit").ok();
        db.register_source("github", "GitHub").ok();
        db.register_source("rss", "RSS").ok();
        db.register_source("youtube", "YouTube").ok();
        db.register_source("twitter", "Twitter").ok();
        db.register_source("lobsters", "Lobsters").ok();
        db.register_source("devto", "Dev.to").ok();
        db.register_source("producthunt", "Product Hunt").ok();

        info!(target: "4da::db", "Database ready");
        Ok(Arc::new(db))
    })
}

// ============================================================================
// Global Context Engine (Lazy Initialized)
// ============================================================================

static CONTEXT_ENGINE: Lazy<parking_lot::RwLock<Option<Arc<ContextEngine>>>> =
    Lazy::new(|| parking_lot::RwLock::new(None));

fn init_context_engine() -> Result<Arc<ContextEngine>, String> {
    let conn = open_db_connection()?;
    let engine = ContextEngine::new(Arc::new(parking_lot::Mutex::new(conn)))
        .map_err(|e| format!("Failed to initialize context engine: {}", e))?;
    info!(target: "4da::context", "Context engine initialized");
    Ok(Arc::new(engine))
}

pub(crate) fn get_context_engine() -> Result<Arc<ContextEngine>, String> {
    // Fast path: read lock
    {
        let guard = CONTEXT_ENGINE.read();
        if let Some(ref engine) = *guard {
            return Ok(Arc::clone(engine));
        }
    }
    // Slow path: write lock to initialize
    let mut guard = CONTEXT_ENGINE.write();
    if let Some(ref engine) = *guard {
        return Ok(Arc::clone(engine));
    }
    let engine = init_context_engine()?;
    *guard = Some(Arc::clone(&engine));
    Ok(engine)
}

/// Invalidate the context engine so it reinitializes on next access.
/// Call after settings changes that affect context (interests, exclusions, context dirs).
pub(crate) fn invalidate_context_engine() {
    let mut guard = CONTEXT_ENGINE.write();
    if guard.is_some() {
        *guard = None;
        info!(target: "4da::context", "Context engine invalidated, will reinitialize on next access");
    }
}

// ============================================================================
// Global ACE Instance (Lazy Initialized with RwLock for mutable access)
// ============================================================================

static ACE_ENGINE: OnceCell<Arc<parking_lot::RwLock<ace::ACE>>> = OnceCell::new();

fn init_ace_engine() -> Result<Arc<parking_lot::RwLock<ace::ACE>>, String> {
    let conn = open_db_connection()?;

    let engine = ace::ACE::new(Arc::new(parking_lot::Mutex::new(conn)))
        .map_err(|e| format!("Failed to initialize ACE: {}", e))?;

    info!(target: "4da::ace", "Autonomous Context Engine ready");
    Ok(Arc::new(parking_lot::RwLock::new(engine)))
}

pub(crate) fn get_ace_engine() -> Result<parking_lot::RwLockReadGuard<'static, ace::ACE>, String> {
    let engine = ACE_ENGINE.get_or_try_init(init_ace_engine)?;
    Ok(engine.read())
}

pub(crate) fn get_ace_engine_mut(
) -> Result<parking_lot::RwLockWriteGuard<'static, ace::ACE>, String> {
    let engine = ACE_ENGINE.get_or_try_init(init_ace_engine)?;
    Ok(engine.write())
}

// ============================================================================
// Global Source Registry (Lazy Initialized)
// ============================================================================

static SOURCE_REGISTRY: OnceCell<Mutex<SourceRegistry>> = OnceCell::new();

fn get_source_registry() -> &'static Mutex<SourceRegistry> {
    SOURCE_REGISTRY.get_or_init(|| {
        info!(target: "4da::sources", "Initializing source registry");
        let mut registry = SourceRegistry::new();

        // Register default sources
        registry.register(Box::new(HackerNewsSource::new()));
        registry.register(Box::new(ArxivSource::new()));
        registry.register(Box::new(RedditSource::new()));
        registry.register(Box::new(GitHubSource::with_languages(
            load_github_languages_from_settings(),
        )));
        registry.register(Box::new(ProductHuntSource::new()));

        // Register RSS source (feeds loaded from settings)
        let rss_feeds = load_rss_feeds_from_settings();
        registry.register(Box::new(RssSource::with_feeds(rss_feeds)));

        // Register Twitter/X source (X API v2 with Bearer Token)
        let (twitter_handles, x_api_key) = load_twitter_settings();
        registry.register(Box::new(
            TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key),
        ));

        // Register YouTube source (free RSS feeds, no API key needed)
        let youtube_channels = load_youtube_channels_from_settings();
        registry.register(Box::new(YouTubeSource::with_channels(youtube_channels)));

        info!(target: "4da::sources", count = registry.count(), "Source registry ready");
        Mutex::new(registry)
    })
}

// ============================================================================
// Global Settings Manager
// ============================================================================

static SETTINGS_MANAGER: OnceCell<Mutex<SettingsManager>> = OnceCell::new();

pub(crate) fn get_settings_manager() -> &'static Mutex<SettingsManager> {
    SETTINGS_MANAGER.get_or_init(|| {
        let data_path = get_db_path()
            .parent()
            .expect("Database path must have a parent directory")
            .to_path_buf();

        info!(target: "4da::settings", "Initializing settings manager");
        let manager = SettingsManager::new(&data_path);
        info!(target: "4da::settings", rerank_enabled = manager.is_rerank_enabled(), "Settings loaded");
        Mutex::new(manager)
    })
}

// ============================================================================
// Global Analysis State
// ============================================================================

static ANALYSIS_STATE: OnceCell<Mutex<AnalysisState>> = OnceCell::new();

pub(crate) fn get_analysis_state() -> &'static Mutex<AnalysisState> {
    ANALYSIS_STATE.get_or_init(|| {
        Mutex::new(AnalysisState {
            running: false,
            completed: false,
            error: None,
            results: None,
            started_at: None,
            last_completed_at: None,
        })
    })
}

// ============================================================================
// Global Monitoring State
// ============================================================================

static MONITORING_STATE: OnceCell<Arc<monitoring::MonitoringState>> = OnceCell::new();

pub(crate) fn get_monitoring_state() -> &'static Arc<monitoring::MonitoringState> {
    MONITORING_STATE.get_or_init(|| Arc::new(monitoring::MonitoringState::new()))
}

// ============================================================================
// Global Job Queue (Background Extraction Processing)
// ============================================================================

static JOB_QUEUE: OnceCell<Arc<parking_lot::RwLock<job_queue::JobQueue>>> = OnceCell::new();

fn init_job_queue() -> Result<Arc<parking_lot::RwLock<job_queue::JobQueue>>, String> {
    let conn = open_db_connection()?;

    let queue = job_queue::JobQueue::new(Arc::new(parking_lot::Mutex::new(conn)));
    info!(target: "4da::job_queue", "Job queue initialized");
    Ok(Arc::new(parking_lot::RwLock::new(queue)))
}

pub(crate) fn get_job_queue(
) -> Result<&'static Arc<parking_lot::RwLock<job_queue::JobQueue>>, String> {
    JOB_QUEUE.get_or_try_init(init_job_queue)
}

// ============================================================================
// Configuration
// ============================================================================

/// Get context directories from settings (no fallback - empty means no context)
pub(crate) fn get_context_dirs() -> Vec<PathBuf> {
    let settings = get_settings_manager().lock();
    let dirs = settings.get().context_dirs.clone();
    drop(settings);

    dirs.into_iter()
        .map(|d| normalize_context_path(&d))
        .collect()
}

/// Convert WSL-style paths (/mnt/c/...) to Windows paths (C:\...) when running on Windows.
/// This handles the common case where paths are stored in settings using WSL conventions
/// but the app runs as a native Windows process.
fn normalize_context_path(path: &str) -> PathBuf {
    if cfg!(windows) && path.starts_with("/mnt/") {
        let rest = &path[5..]; // strip "/mnt/"
        let mut chars = rest.chars();
        if let Some(drive_letter) = chars.next() {
            if drive_letter.is_ascii_lowercase() {
                let remainder = chars.as_str();
                let win_remainder = remainder
                    .strip_prefix('/')
                    .unwrap_or(remainder)
                    .replace('/', "\\");
                return PathBuf::from(format!(
                    "{}:\\{}",
                    drive_letter.to_ascii_uppercase(),
                    win_remainder
                ));
            }
        }
    }
    PathBuf::from(path)
}

/// Legacy function for single directory (uses first configured dir)
pub(crate) fn get_context_dir() -> Option<PathBuf> {
    get_context_dirs().into_iter().next()
}

/// File extensions we care about for Phase 0
pub(crate) const SUPPORTED_EXTENSIONS: &[&str] = &["md", "txt", "rs", "ts", "js", "py"];

/// Relevance threshold stored as atomic u32 bits for thread-safe auto-tuning.
/// Adjusted daily based on user engagement rate (see `compute_threshold_adjustment`).
static RELEVANCE_THRESHOLD_BITS: AtomicU32 = AtomicU32::new(0);

/// Get the current relevance threshold (thread-safe).
/// Returns the auto-tuned value, or 0.50 default if not yet initialized.
/// Targets ~3-5% pass rate for genuinely relevant items.
pub(crate) fn get_relevance_threshold() -> f32 {
    let bits = RELEVANCE_THRESHOLD_BITS.load(Ordering::Relaxed);
    if bits == 0 {
        0.50 // Default: only pass items with strong context + interest match
    } else {
        f32::from_bits(bits)
    }
}

/// Set the relevance threshold (thread-safe, clamped to [0.30, 0.70]).
pub(crate) fn set_relevance_threshold(value: f32) {
    let clamped = value.clamp(0.30, 0.70);
    RELEVANCE_THRESHOLD_BITS.store(clamped.to_bits(), Ordering::Relaxed);
}

/// Maximum content length for embedding (roughly 1000 words)
const MAX_CONTENT_LENGTH: usize = 5000;

/// Maximum chunk size in characters (roughly 100-150 words)
const MAX_CHUNK_SIZE: usize = 500;

// ============================================================================
// Text Processing
// ============================================================================

/// Split text into chunks for embedding
pub(crate) fn chunk_text(text: &str, source_file: &str) -> Vec<(String, String)> {
    let mut chunks = Vec::new();
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    let mut current_chunk = String::new();

    for para in paragraphs {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }

        if current_chunk.len() + para.len() > MAX_CHUNK_SIZE && !current_chunk.is_empty() {
            chunks.push((source_file.to_string(), current_chunk.clone()));
            current_chunk.clear();
        }

        if !current_chunk.is_empty() {
            current_chunk.push_str("\n\n");
        }
        current_chunk.push_str(para);
    }

    if !current_chunk.is_empty() {
        chunks.push((source_file.to_string(), current_chunk));
    }

    // If no chunks were created, use the whole text
    if chunks.is_empty() && !text.trim().is_empty() {
        chunks.push((source_file.to_string(), text.trim().to_string()));
    }

    chunks
}

// ============================================================================
// Content Scraping
// ============================================================================

/// Scrape article content from a URL
pub(crate) async fn scrape_article_content(url: &str) -> Option<String> {
    use scraper::{Html, Selector};

    // Skip non-HTTP URLs and known problematic domains
    if !url.starts_with("http") {
        return None;
    }

    // Create client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    // Fetch the page
    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (compatible; 4DA/1.0)")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let html = response.text().await.ok()?;
    let document = Html::parse_document(&html);

    // Try multiple content selectors in order of preference
    let selectors = [
        "article",
        "main",
        "[role='main']",
        ".post-content",
        ".article-content",
        ".entry-content",
        ".content",
        "body",
    ];

    for selector_str in selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                let text: String = element
                    .text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");

                // Only use if we got meaningful content (at least 100 chars)
                if text.len() > 100 {
                    // Truncate to max length
                    let truncated = if text.len() > MAX_CONTENT_LENGTH {
                        text.chars().take(MAX_CONTENT_LENGTH).collect()
                    } else {
                        text
                    };
                    return Some(truncated);
                }
            }
        }
    }

    None
}

/// Build embedding text from HN item (title + content)
/// Decode common HTML entities that sources may include in titles/content.
/// Applied to all text before embedding and display to prevent `&amp;` literals.
pub(crate) fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

pub(crate) fn build_embedding_text(title: &str, content: &str) -> String {
    let clean_title = decode_html_entities(title);
    let clean_content = decode_html_entities(content);
    if clean_content.is_empty() {
        clean_title
    } else {
        format!("{}\n\n{}", clean_title, clean_content)
    }
}

/// Emit a progress event to the frontend
pub(crate) fn emit_progress(
    app: &AppHandle,
    stage: &str,
    progress: f32,
    message: &str,
    processed: usize,
    total: usize,
) {
    let status = AnalysisStatus {
        stage: stage.to_string(),
        progress,
        message: message.to_string(),
        items_processed: processed,
        items_total: total,
    };
    let _ = app.emit("analysis-progress", &status);
}

// ============================================================================
// Void Engine Signal Helpers
// ============================================================================

/// Emit void signal: active source fetching
pub(crate) fn void_signal_fetching(app: &AppHandle) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_fetching(db, monitoring);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: cache fill complete
pub(crate) fn void_signal_cache_filled(app: &AppHandle) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_cache_filled(db, monitoring);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Extract a SignalSummary from analysis results.
fn extract_signal_summary(results: &[SourceRelevance]) -> Option<void_engine::SignalSummary> {
    let mut type_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut max_priority: u8 = 0;
    let mut critical_count: u32 = 0;

    for r in results {
        if let Some(ref st) = r.signal_type {
            *type_counts.entry(st.clone()).or_insert(0) += 1;
        }
        if let Some(ref sp) = r.signal_priority {
            let pval = match sp.as_str() {
                "critical" => 4u8,
                "high" => 3,
                "medium" => 2,
                "low" => 1,
                _ => 0,
            };
            if pval > max_priority {
                max_priority = pval;
            }
            if pval == 4 {
                critical_count += 1;
            }
        }
    }

    let total_signals: u32 = type_counts.values().sum();
    if total_signals == 0 {
        return None;
    }

    // Urgency: weighted sum / (total * max_weight)
    let weighted_sum: f32 = type_counts
        .iter()
        .map(|(slug, count)| {
            let weight = match slug.as_str() {
                "security_alert" => 4.0,
                "breaking_change" => 3.0,
                "tool_discovery" => 2.0,
                "tech_trend" => 2.0,
                "competitive_intel" => 2.0,
                "learning" => 1.0,
                _ => 1.0,
            };
            weight * (*count as f32)
        })
        .sum();
    let urgency = (weighted_sum / (total_signals as f32 * 4.0)).min(1.0);

    let dominant_type = type_counts
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(s, _)| s.clone());

    Some(void_engine::SignalSummary {
        max_priority,
        critical_count,
        signal_type_counts: type_counts,
        dominant_type,
        urgency_score: urgency,
    })
}

/// Emit void signal: analysis complete with scores
pub(crate) fn void_signal_analysis_complete(app: &AppHandle, results: &[SourceRelevance]) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let top_scores: Vec<f32> = results
            .iter()
            .filter(|r| r.relevant)
            .map(|r| r.top_score)
            .collect();
        let summary = extract_signal_summary(results);
        let signal =
            void_engine::signal_after_analysis(db, monitoring, &top_scores, summary.as_ref());
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: error occurred
pub(crate) fn void_signal_error(app: &AppHandle) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_error(db, monitoring);
        void_engine::emit_if_changed(app, signal);
    }
}

// ============================================================================
// Commands
// ============================================================================

#[tauri::command]
async fn get_hn_top_stories() -> Result<Vec<FetchedItem>, String> {
    info!(target: "4da::sources", "Fetching HN top stories");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let top_ids: Vec<u64> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch top stories: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse top stories: {}", e))?;

    debug!(target: "4da::sources", count = top_ids.len(), "Got story IDs, fetching top 30");

    let mut items = Vec::new();
    for id in top_ids.into_iter().take(30) {
        let url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);
        match client.get(&url).send().await {
            Ok(response) => match response.json::<HNStory>().await {
                Ok(story) => {
                    let title = story.title.unwrap_or_else(|| "[No title]".to_string());

                    // Get content: prefer HN text field, otherwise scrape URL
                    let content = if let Some(text) = story.text {
                        // Ask HN / Show HN / text posts have content directly
                        debug!(target: "4da::sources", id = id, title = %title, "HN story has text");
                        text
                    } else if let Some(ref article_url) = story.url {
                        // Link posts - scrape the article
                        debug!(target: "4da::sources", id = id, title = %title, "Scraping HN story");
                        match scrape_article_content(article_url).await {
                            Some(scraped) => {
                                debug!(target: "4da::sources", id = id, chars = scraped.len(), "Scraped content");
                                scraped
                            }
                            None => {
                                debug!(target: "4da::sources", id = id, "Scrape failed, using title only");
                                String::new()
                            }
                        }
                    } else {
                        debug!(target: "4da::sources", id = id, title = %title, "HN story has no content");
                        String::new()
                    };

                    items.push(FetchedItem {
                        id: story.id,
                        title,
                        url: story.url,
                        content,
                    });
                }
                Err(e) => {
                    warn!(target: "4da::sources", id = id, error = %e, "Failed to parse story")
                }
            },
            Err(e) => warn!(target: "4da::sources", id = id, error = %e, "Failed to fetch story"),
        }
    }

    info!(target: "4da::sources", count = items.len(), "Loaded HN stories");
    Ok(items)
}

#[tauri::command]
async fn compute_relevance() -> Result<Vec<SourceRelevance>, String> {
    info!(target: "4da::analysis", "=== COMPUTING RELEVANCE SCORES (Phase 1 - with persistence) ===");

    let db = get_database()?;

    // Step 1: Check context availability (using sqlite-vec KNN search)
    debug!(target: "4da::analysis", "Step 1: Checking context (sqlite-vec KNN enabled)");
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;

    if cached_context_count > 0 {
        info!(target: "4da::analysis", context_chunks = cached_context_count, "Context indexed (using KNN search)");
    } else {
        warn!(target: "4da::analysis", "No context indexed. Scores will be 0 without context");
    }

    // Step 2: Fetch HN story IDs and process incrementally
    debug!(target: "4da::analysis", "Step 2: Fetching HN stories (incremental)");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let top_ids: Vec<u64> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch top stories: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse top stories: {}", e))?;

    debug!(target: "4da::analysis", story_ids = top_ids.len(), "Processing top 30 stories");

    // Categorize: cached vs new
    let mut cached_items: Vec<(FetchedItem, Vec<f32>)> = Vec::new();
    let mut new_items: Vec<FetchedItem> = Vec::new();

    for id in top_ids.into_iter().take(30) {
        let id_str = id.to_string();

        // Check cache first
        if let Ok(Some(cached)) = db.get_source_item("hackernews", &id_str) {
            debug!(target: "4da::analysis", id = id, title = %&truncate_utf8(&cached.title, 40), "HN story (cached)");
            db.touch_source_item("hackernews", &id_str).ok();
            cached_items.push((
                FetchedItem {
                    id,
                    title: cached.title,
                    url: cached.url,
                    content: cached.content,
                },
                cached.embedding,
            ));
        } else {
            // Need to fetch from API
            let url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);
            match client.get(&url).send().await {
                Ok(response) => match response.json::<HNStory>().await {
                    Ok(story) => {
                        let title = story.title.unwrap_or_else(|| "[No title]".to_string());

                        // Get content: prefer HN text field, otherwise scrape URL
                        let content = if let Some(text) = story.text {
                            debug!(target: "4da::analysis", id = id, title = %&truncate_utf8(&title, 40), "HN story NEW - has text");
                            text
                        } else if let Some(ref article_url) = story.url {
                            debug!(target: "4da::analysis", id = id, title = %&truncate_utf8(&title, 35), "HN story NEW - scraping");
                            match scrape_article_content(article_url).await {
                                Some(scraped) => {
                                    debug!(target: "4da::analysis", id = id, chars = scraped.len(), "Scraped content");
                                    scraped
                                }
                                None => {
                                    debug!(target: "4da::analysis", id = id, "Scrape failed");
                                    String::new()
                                }
                            }
                        } else {
                            debug!(target: "4da::analysis", id = id, title = %&truncate_utf8(&title, 40), "HN story NEW - no content");
                            String::new()
                        };

                        new_items.push(FetchedItem {
                            id: story.id,
                            title,
                            url: story.url,
                            content,
                        });
                    }
                    Err(e) => {
                        warn!(target: "4da::analysis", id = id, error = %e, "Failed to parse story")
                    }
                },
                Err(e) => {
                    warn!(target: "4da::analysis", id = id, error = %e, "Failed to fetch story")
                }
            }
        }
    }

    info!(target: "4da::analysis", cached = cached_items.len(), new = new_items.len(), "Found items");

    // Step 3: Generate embeddings only for NEW items
    let new_embeddings = if !new_items.is_empty() {
        debug!(target: "4da::analysis", count = new_items.len(), "Step 3: Generating embeddings for NEW items");
        let with_content = new_items.iter().filter(|i| !i.content.is_empty()).count();
        debug!(target: "4da::analysis", with_content = with_content, "Items have scraped content");

        let new_texts: Vec<String> = new_items
            .iter()
            .map(|item| build_embedding_text(&item.title, &item.content))
            .collect();
        let embeddings = embed_texts(&new_texts).await?;

        // Cache new items in database
        debug!(target: "4da::analysis", count = new_items.len(), "Caching new items in database");
        for (item, embedding) in new_items.iter().zip(embeddings.iter()) {
            db.upsert_source_item(
                "hackernews",
                &item.id.to_string(),
                item.url.as_deref(),
                &decode_html_entities(&item.title),
                &decode_html_entities(&item.content),
                embedding,
            )
            .ok();
        }

        embeddings
    } else {
        debug!(target: "4da::analysis", "Step 3: All items cached, no embedding needed");
        vec![]
    };

    db.update_source_fetch_time("hackernews").ok();

    // Combine cached and new items
    let mut all_items_with_embeddings: Vec<(FetchedItem, Vec<f32>)> = cached_items;
    for (item, embedding) in new_items.into_iter().zip(new_embeddings.into_iter()) {
        all_items_with_embeddings.push((item, embedding));
    }

    if all_items_with_embeddings.is_empty() {
        return Err("No HN stories fetched".to_string());
    }

    // Step 4: Load user context for personalized scoring
    debug!(target: "4da::analysis", "Step 4: Loading user context");
    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    let interest_count = static_identity.interests.len();
    let exclusion_count = static_identity.exclusions.len();
    info!(target: "4da::analysis", interests = interest_count, exclusions = exclusion_count, "User context loaded");

    if !static_identity.exclusions.is_empty() {
        debug!(target: "4da::analysis", exclusions = %static_identity.exclusions.join(", "), "Active exclusions");
    }
    if !static_identity.interests.is_empty() {
        let topics: Vec<&str> = static_identity
            .interests
            .iter()
            .map(|i| i.topic.as_str())
            .collect();
        debug!(target: "4da::analysis", interests = %topics.join(", "), "Active interests");
    }

    // Step 4b: Load ACE-discovered context
    debug!(target: "4da::ace", "Step 4b: Loading ACE discovered context");
    let ace_ctx = scoring::get_ace_context();
    // PASIFA: Pre-compute topic embeddings for semantic matching
    let topic_embeddings = scoring::get_topic_embeddings(&ace_ctx).await;
    info!(target: "4da::ace",
        active_topics = ace_ctx.active_topics.len(),
        detected_tech = ace_ctx.detected_tech.len(),
        anti_topics = ace_ctx.anti_topics.len(),
        affinities = ace_ctx.topic_affinities.len(),
        embeddings = topic_embeddings.len(),
        "ACE context loaded"
    );

    if !ace_ctx.active_topics.is_empty() {
        debug!(target: "4da::ace", topics = %ace_ctx.active_topics.iter().take(5).cloned().collect::<Vec<_>>().join(", "), "ACE Topics");
    }
    if !ace_ctx.detected_tech.is_empty() {
        debug!(target: "4da::ace", tech = %ace_ctx.detected_tech.iter().take(5).cloned().collect::<Vec<_>>().join(", "), "ACE Tech");
    }

    // Step 5: Compute similarity scores with context integration
    debug!(target: "4da::analysis", items = all_items_with_embeddings.len(), "Step 5: Computing personalized relevance");
    let mut results: Vec<SourceRelevance> = Vec::new();
    let mut excluded_count = 0;

    for (item, item_embedding) in &all_items_with_embeddings {
        // Extract topics from this item
        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions FIRST (hard filter)
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions)
            .or_else(|| scoring::check_ace_exclusions(&topics, &ace_ctx));

        if let Some(ref exclusion) = excluded_by {
            debug!(target: "4da::analysis", title = %&truncate_utf8(&item.title, 50), exclusion = %exclusion, "EXCLUDED");
            excluded_count += 1;

            // Still add to results but marked as excluded
            results.push(SourceRelevance {
                id: item.id,
                title: item.title.clone(),
                url: item.url.clone(),
                top_score: 0.0,
                matches: vec![],
                relevant: false,
                context_score: 0.0,
                interest_score: 0.0,
                excluded: true,
                excluded_by: Some(exclusion.clone()),
                source_type: "hackernews".to_string(),
                explanation: None,     // Excluded items don't need explanations
                confidence: Some(0.0), // Excluded items have zero confidence
                score_breakdown: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
                signal_triggers: None,
                similar_count: 0,
                similar_titles: vec![],
                serendipity: false,
            });
            continue;
        }

        // Compute context file score using sqlite-vec KNN search (O(log n))
        // With graceful fallback to empty matches if KNN fails
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 {
            match db.find_similar_contexts(item_embedding, 3) {
                Ok(results) => results
                    .into_iter()
                    .map(|result| {
                        // Convert L2 distance to similarity: 1/(1+d) gives [0,1] range
                        let similarity = 1.0 / (1.0 + result.distance);
                        // Safely truncate text at character boundary
                        let matched_text = if result.text.len() > 100 {
                            let truncated: String = result.text.chars().take(100).collect();
                            format!("{}...", truncated)
                        } else {
                            result.text
                        };
                        RelevanceMatch {
                            source_file: result.source_file,
                            matched_text,
                            similarity,
                        }
                    })
                    .collect(),
                Err(e) => {
                    warn!(target: "4da::knn", error = %e, "KNN search failed - using interest-only scoring");
                    vec![]
                }
            }
        } else {
            vec![]
        };

        let context_score = matches.first().map(|m| m.similarity).unwrap_or(0.0);

        // Compute interest score (what you care about)
        let interest_score =
            scoring::compute_interest_score(item_embedding, &static_identity.interests);

        // Compute semantic ACE boost for topic/tech matching
        // PASIFA: Use semantic matching when embeddings available, fall back to keywords
        let semantic_boost =
            scoring::compute_semantic_ace_boost(item_embedding, &ace_ctx, &topic_embeddings)
                .unwrap_or_else(|| {
                    // Fall back to keyword matching for active topics and tech only (not affinities)
                    let mut boost: f32 = 0.0;
                    for topic in &topics {
                        let topic_lower = topic.to_lowercase();
                        // Active topics boost
                        for active_topic in &ace_ctx.active_topics {
                            if topic_lower.contains(active_topic)
                                || active_topic.contains(&topic_lower)
                            {
                                let conf = ace_ctx
                                    .topic_confidence
                                    .get(active_topic)
                                    .copied()
                                    .unwrap_or(0.5);
                                boost += 0.15 * conf;
                                break;
                            }
                        }
                        // Tech stack boost
                        for tech in &ace_ctx.detected_tech {
                            if topic_lower.contains(tech) || tech.contains(&topic_lower) {
                                boost += 0.12;
                                break;
                            }
                        }
                    }
                    boost.clamp(0.0, 0.3)
                });

        // Keyword interest matching (with specificity weighting)
        let keyword_score = scoring::compute_keyword_interest_score_pub(
            &item.title,
            &item.content,
            &static_identity.interests,
        );

        // Combined score: weighted average of context, interest scores, plus semantic boost
        // Dynamically adjust weights based on what data is available
        let base_score = if cached_context_count > 0 && interest_count > 0 {
            // Full mode: 50% context + 50% interests + ACE boost
            (context_score * 0.5 + interest_score * 0.5 + semantic_boost).min(1.0)
        } else if interest_count > 0 {
            // No context indexed: rely on interests + ACE boost (full weight)
            (interest_score * 0.7 + semantic_boost * 1.5).min(1.0)
        } else if cached_context_count > 0 {
            // No interests: rely on context + ACE boost
            (context_score + semantic_boost).min(1.0)
        } else {
            // Neither context nor interests: pure ACE topic matching
            (semantic_boost * 2.0).min(1.0)
        };

        // Multi-signal confirmation gate (same logic as cached path)
        let affinity_mult = scoring::compute_affinity_multiplier(&topics, &ace_ctx);
        let (gated_score, signal_count, confirmation_mult, confirmed_signals) =
            scoring::apply_confirmation_gate(
                base_score,
                context_score,
                interest_score,
                keyword_score,
                semantic_boost,
                &ace_ctx,
                &topics,
                0.0, // No feedback boost in fresh-fetch path
                affinity_mult,
            );

        // PASIFA: Apply unified multiplicative scoring on gated score
        let combined_score = scoring::compute_unified_relevance(gated_score, &topics, &ace_ctx);

        let relevant = combined_score >= get_relevance_threshold();

        let anti_penalty = scoring::compute_anti_penalty(&topics, &ace_ctx);

        // Log scoring details
        if relevant {
            info!(target: "4da::analysis",
                id = item.id,
                title = %item.title,
                combined = combined_score,
                base = base_score,
                gated = gated_score,
                context = context_score,
                interest = interest_score,
                keyword = keyword_score,
                semantic_boost = semantic_boost,
                affinity_mult = affinity_mult,
                anti_penalty = anti_penalty,
                signal_count = signal_count,
                "RELEVANT"
            );
        } else {
            debug!(target: "4da::analysis",
                id = item.id,
                title = %item.title,
                combined = combined_score,
                gated = gated_score,
                context = context_score,
                interest = interest_score,
                signal_count = signal_count,
                "not relevant"
            );
        }
        if !topics.is_empty() {
            debug!(target: "4da::analysis", id = item.id, topics = %topics.iter().take(5).cloned().collect::<Vec<_>>().join(", "), "Extracted topics");
        }

        // Generate explanation for relevant items
        let declared_tech: Vec<String> = static_identity
            .tech_stack
            .iter()
            .map(|t| t.to_lowercase())
            .collect();
        let explanation = if relevant {
            Some(scoring::generate_relevance_explanation(
                &item.title,
                context_score,
                interest_score,
                &matches,
                &ace_ctx,
                &topics,
                &static_identity.interests,
                &declared_tech,
            ))
        } else {
            None
        };

        // Calculate confidence and score breakdown
        let confidence = scoring::calculate_confidence(
            context_score,
            interest_score,
            semantic_boost,
            &ace_ctx,
            &topics,
            cached_context_count,
            interest_count as i64,
            signal_count,
        );

        let mut confidence_by_signal = std::collections::HashMap::new();
        if cached_context_count > 0 {
            confidence_by_signal.insert("context".to_string(), context_score);
        }
        if interest_count > 0 {
            confidence_by_signal.insert("interest".to_string(), interest_score);
        }
        if semantic_boost > 0.0 {
            confidence_by_signal.insert("ace_boost".to_string(), semantic_boost);
        }

        let score_breakdown = ScoreBreakdown {
            context_score,
            interest_score,
            keyword_score,
            ace_boost: semantic_boost,
            affinity_mult,
            anti_penalty,
            freshness_mult: 1.0,
            feedback_boost: 0.0,
            source_quality_boost: 0.0,
            confidence_by_signal,
            signal_count,
            confirmed_signals,
            confirmation_mult,
        };

        results.push(SourceRelevance {
            id: item.id,
            title: item.title.clone(),
            url: item.url.clone(),
            top_score: combined_score,
            matches,
            relevant,
            context_score,
            interest_score,
            excluded: false,
            excluded_by: None,
            source_type: "hackernews".to_string(),
            explanation,
            confidence: Some(confidence),
            score_breakdown: Some(score_breakdown),
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
        });
    }

    // Sort by relevance score descending (excluded items go to bottom)
    results.sort_by(|a, b| {
        if a.excluded && !b.excluded {
            return std::cmp::Ordering::Greater;
        }
        if !a.excluded && b.excluded {
            return std::cmp::Ordering::Less;
        }
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Summary
    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let db_item_count = db.total_item_count().unwrap_or(0);
    info!(target: "4da::analysis", "=== PERSONALIZED ANALYSIS COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = excluded_count,
        interests = interest_count,
        exclusions = exclusion_count,
        threshold = get_relevance_threshold(),
        db_cached = db_item_count,
        "Analysis summary"
    );

    Ok(results)
}

// ============================================================================
// Background Job Functions (called by monitoring scheduler)
// ============================================================================

/// Run background health check - called every 5 minutes by scheduler
pub async fn run_background_health_check() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let report = health::check_all_components(&conn)?;

    info!(
        target: "4da::health",
        status = ?report.overall_status,
        quality = ?report.context_quality,
        fallback = report.fallback_level,
        "Background health check complete"
    );

    serde_json::to_value(&report).map_err(|e| e.to_string())
}

/// Run background anomaly detection - called every hour by scheduler
pub async fn run_background_anomaly_detection() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let anomalies = anomaly::detect_all(&conn)?;

    // Store any new anomalies
    let mut new_count = 0;
    for a in &anomalies {
        if anomaly::store_anomaly(&conn, a).is_ok() {
            new_count += 1;
        }
    }

    info!(target: "4da::anomaly", found = anomalies.len(), stored = new_count, "Background anomaly detection complete");

    Ok(serde_json::json!({
        "anomalies_found": anomalies.len(),
        "new_stored": new_count,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Run background behavior decay - called daily by scheduler
pub async fn run_background_behavior_decay() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;

    // Apply decay to behavior signals
    let decayed_count = ace.apply_behavior_decay()?;

    info!(
        target: "4da::decay",
        signals_decayed = decayed_count,
        "Background behavior decay applied"
    );

    // Auto-tune relevance threshold based on engagement rate
    let threshold_adjusted = {
        let current = get_relevance_threshold();
        if let Some(new_threshold) = ace.compute_threshold_adjustment(current) {
            set_relevance_threshold(new_threshold);
            ace.store_threshold(new_threshold);
            info!(
                target: "4da::threshold",
                old = current,
                new = new_threshold,
                "Auto-tuned relevance threshold"
            );
            Some(new_threshold)
        } else {
            None
        }
    };

    Ok(serde_json::json!({
        "signals_decayed": decayed_count,
        "threshold_adjusted": threshold_adjusted,
        "current_threshold": get_relevance_threshold(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// Update, Digest, and AI Briefing commands are in digest_commands.rs

// ============================================================================
// MCP Score Autopsy Command
// ============================================================================

/// Execute score autopsy - native implementation using AnalysisState data
/// Provides deep breakdown of why an item scored the way it did
#[tauri::command]
async fn mcp_score_autopsy(
    item_id: u64,
    source_type: String,
    _synthesize: bool,
    _compact: bool,
) -> Result<serde_json::Value, String> {
    info!(
        target: "4da::autopsy",
        item_id = item_id,
        source_type = %source_type,
        "Score autopsy requested"
    );

    // Find the item in analysis results
    let state = get_analysis_state();
    let guard = state.lock();
    let results = guard
        .results
        .as_ref()
        .ok_or("No analysis results available. Run an analysis first.")?;

    let item = results
        .iter()
        .find(|r| r.id == item_id)
        .ok_or_else(|| format!("Item {} not found in analysis results", item_id))?;

    // Get item metadata from DB
    let db = get_database()?;
    let db_item = db.get_source_item_by_id(item_id as i64).ok().flatten();

    let age_hours = db_item
        .as_ref()
        .map(|i| (chrono::Utc::now() - i.created_at).num_minutes() as f64 / 60.0)
        .unwrap_or(0.0);

    let created_at = db_item
        .as_ref()
        .map(|i| i.created_at.to_rfc3339())
        .unwrap_or_default();

    // Build component breakdown from ScoreBreakdown
    let mut components = Vec::new();
    if let Some(ref bd) = item.score_breakdown {
        components.push(serde_json::json!({
            "name": "Context Match",
            "raw_value": bd.context_score,
            "weight": 0.5,
            "contribution": bd.context_score * 0.5,
            "explanation": if bd.context_score > 0.2 {
                format!("Strong match with your project files ({:.0}% similarity)", bd.context_score * 100.0)
            } else if bd.context_score > 0.05 {
                format!("Weak match with project context ({:.0}%)", bd.context_score * 100.0)
            } else {
                "No significant match with indexed project files".to_string()
            }
        }));

        components.push(serde_json::json!({
            "name": "Interest Match",
            "raw_value": bd.interest_score,
            "weight": 0.5,
            "contribution": bd.interest_score * 0.5,
            "explanation": if bd.interest_score > 0.3 {
                format!("Closely matches your declared interests ({:.0}%)", bd.interest_score * 100.0)
            } else if bd.interest_score > 0.1 {
                format!("Partial interest match ({:.0}%)", bd.interest_score * 100.0)
            } else {
                "Low alignment with declared interests".to_string()
            }
        }));

        if bd.ace_boost > 0.01 {
            components.push(serde_json::json!({
                "name": "ACE Semantic Boost",
                "raw_value": bd.ace_boost,
                "weight": 1.0,
                "contribution": bd.ace_boost,
                "explanation": format!("Boosted by ACE context engine topics/tech (+{:.0}%)", bd.ace_boost * 100.0)
            }));
        }

        if (bd.affinity_mult - 1.0).abs() > 0.01 {
            let direction = if bd.affinity_mult > 1.0 {
                "boosted"
            } else {
                "reduced"
            };
            components.push(serde_json::json!({
                "name": "Learned Affinity",
                "raw_value": bd.affinity_mult,
                "weight": 1.0,
                "contribution": bd.affinity_mult - 1.0,
                "explanation": format!("Score {} by learned topic preferences (x{:.2})", direction, bd.affinity_mult)
            }));
        }

        if bd.anti_penalty > 0.01 {
            components.push(serde_json::json!({
                "name": "Anti-Topic Penalty",
                "raw_value": bd.anti_penalty,
                "weight": 1.0,
                "contribution": -bd.anti_penalty,
                "explanation": format!("Penalized by anti-topic filter (-{:.0}%)", bd.anti_penalty * 100.0)
            }));
        }

        if (bd.freshness_mult - 1.0).abs() > 0.01 {
            let label = if bd.freshness_mult > 1.0 {
                "Freshness bonus"
            } else {
                "Staleness decay"
            };
            components.push(serde_json::json!({
                "name": "Temporal Freshness",
                "raw_value": bd.freshness_mult,
                "weight": 1.0,
                "contribution": bd.freshness_mult - 1.0,
                "explanation": format!("{}: item is {:.0}h old (x{:.2})", label, age_hours, bd.freshness_mult)
            }));
        }
    }

    // Build matching context from ACE
    let ace_ctx = scoring::get_ace_context();
    let topics = extract_topics(&item.title, "");

    let matching_interests: Vec<String> = {
        let ctx_engine = get_context_engine().ok();
        ctx_engine
            .and_then(|ce| ce.get_static_identity().ok())
            .map(|id| {
                id.interests
                    .iter()
                    .filter(|i| {
                        let int_lower = i.topic.to_lowercase();
                        topics.iter().any(|t| {
                            let tl = t.to_lowercase();
                            tl.contains(&int_lower) || int_lower.contains(&tl)
                        })
                    })
                    .map(|i| i.topic.clone())
                    .collect()
            })
            .unwrap_or_default()
    };

    let matching_tech: Vec<String> = ace_ctx
        .detected_tech
        .iter()
        .filter(|t| {
            let tl = t.to_lowercase();
            topics.iter().any(|topic| {
                let topic_lower = topic.to_lowercase();
                topic_lower.contains(&tl) || tl.contains(&topic_lower)
            })
        })
        .cloned()
        .collect();

    let matching_active: Vec<String> = ace_ctx
        .active_topics
        .iter()
        .filter(|t| {
            topics.iter().any(|topic| {
                let topic_lower = topic.to_lowercase();
                topic_lower.contains(t.as_str()) || t.contains(&topic_lower)
            })
        })
        .cloned()
        .collect();

    let matching_affinities: Vec<String> = ace_ctx
        .topic_affinities
        .iter()
        .filter(|(_, (score, _))| *score > 0.3)
        .filter(|(topic, _)| {
            topics.iter().any(|t| {
                let tl = t.to_lowercase();
                tl.contains(topic.as_str()) || topic.contains(&tl)
            })
        })
        .map(|(topic, (score, _))| format!("{} ({:+.0}%)", topic, score * 100.0))
        .collect();

    // Find similar items for comparison (items with close scores)
    let similar_items: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| r.id != item_id && r.relevant)
        .map(|r| {
            let diff = r.top_score - item.top_score;
            let key_diff = if diff.abs() < 0.05 {
                "Very similar score - different content matched".to_string()
            } else if diff > 0.0 {
                if r.context_score > item.context_score + 0.1 {
                    "Higher context match with your project files".to_string()
                } else if r.interest_score > item.interest_score + 0.1 {
                    "Better alignment with declared interests".to_string()
                } else {
                    "Stronger overall relevance signals".to_string()
                }
            } else if item.context_score > r.context_score + 0.1 {
                "This item has stronger project context match".to_string()
            } else {
                "This item has stronger interest alignment".to_string()
            };
            (r, diff, key_diff)
        })
        .take(3)
        .map(|(r, diff, key_diff)| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "score": r.top_score,
                "score_difference": diff,
                "key_difference": key_diff
            })
        })
        .collect();

    // Generate recommendations
    let mut recommendations = Vec::new();
    if item.context_score < 0.1 {
        recommendations.push("Index more project files to improve context matching. Add directories in Settings > Context.".to_string());
    }
    if item.interest_score < 0.1 {
        recommendations.push(
            "Add more interests in Settings > Interests to improve matching for this topic area."
                .to_string(),
        );
    }
    if matching_tech.is_empty() {
        recommendations.push("This item doesn't match your detected tech stack. If it's relevant, the ACE engine will learn from your interaction.".to_string());
    }
    if item.top_score < 0.35 && !item.relevant {
        recommendations.push("This item fell below the relevance threshold. Save items like this to train the system to surface similar content.".to_string());
    }

    // Build narrative
    let narrative = build_autopsy_narrative(item, &matching_tech, &matching_active, age_hours);

    Ok(serde_json::json!({
        "item": {
            "id": item.id,
            "title": item.title,
            "url": item.url,
            "source_type": item.source_type,
            "created_at": created_at,
            "age_hours": age_hours
        },
        "final_score": item.top_score,
        "components": components,
        "matching_context": {
            "interests": matching_interests,
            "tech_stack": matching_tech,
            "active_topics": matching_active,
            "learned_affinities": matching_affinities,
            "exclusions_hit": item.excluded_by.as_ref().map(|e| vec![e.clone()]).unwrap_or_else(Vec::<String>::new)
        },
        "similar_items": similar_items,
        "recommendations": recommendations,
        "narrative": narrative
    }))
}

/// Build a human-readable narrative for the score autopsy
fn build_autopsy_narrative(
    item: &SourceRelevance,
    matching_tech: &[String],
    matching_active: &[String],
    age_hours: f64,
) -> String {
    let mut parts = Vec::new();

    // Score assessment
    let score_pct = (item.top_score * 100.0) as u32;
    if item.top_score >= 0.6 {
        parts.push(format!("This item scored {}% - a strong match.", score_pct));
    } else if item.top_score >= 0.35 {
        parts.push(format!(
            "This item scored {}% - above the relevance threshold.",
            score_pct
        ));
    } else {
        parts.push(format!(
            "This item scored {}% - below the relevance threshold of 35%.",
            score_pct
        ));
    }

    // Context explanation
    if item.context_score > 0.3 {
        parts.push("It closely matches code you're actively working on.".to_string());
    } else if item.context_score > 0.1 {
        parts.push("It has some overlap with your project files.".to_string());
    }

    // Interest explanation
    if item.interest_score > 0.3 {
        parts.push("It strongly aligns with your declared interests.".to_string());
    } else if item.interest_score > 0.1 {
        parts.push("It partially matches your interests.".to_string());
    }

    // Tech stack
    if !matching_tech.is_empty() {
        parts.push(format!(
            "It mentions {} which is in your tech stack.",
            matching_tech.join(", ")
        ));
    }

    // Active topics
    if !matching_active.is_empty() {
        parts.push(format!(
            "It relates to topics you've been active in: {}.",
            matching_active.join(", ")
        ));
    }

    // Freshness
    if age_hours < 2.0 {
        parts.push("It was discovered very recently and received a freshness boost.".to_string());
    } else if age_hours > 36.0 {
        parts.push(format!(
            "It's {:.0} hours old, so its score was slightly reduced for staleness.",
            age_hours
        ));
    }

    // Signal info
    if let Some(ref sig) = item.signal_type {
        let label = match sig.as_str() {
            "security_alert" => "a security alert",
            "breaking_change" => "a breaking change notification",
            "tool_discovery" => "a new tool/library discovery",
            "tech_trend" => "a technology trend",
            "learning" => "a learning resource",
            "competitive_intel" => "competitive intelligence",
            _ => "a classified signal",
        };
        parts.push(format!(
            "It was classified as {} with {} priority.",
            label,
            item.signal_priority.as_deref().unwrap_or("unknown")
        ));
    }

    parts.join(" ")
}

// ============================================================================
// App Entry
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!(target: "4da::startup", "========================================");
    info!(target: "4da::startup", "4DA Home - Personalized Intelligence");
    info!(target: "4da::startup", "The internet searches for you.");
    info!(target: "4da::startup", "========================================");
    info!(target: "4da::startup", context_dir = ?get_context_dir(), "Context directory");
    info!(target: "4da::startup", model = "all-MiniLM-L6-v2", dimensions = 384, "Embedding model");
    // Initialize relevance threshold from ACE storage or default
    if let Ok(ace) = get_ace_engine() {
        if let Some(stored) = ace.get_stored_threshold() {
            set_relevance_threshold(stored);
            info!(target: "4da::startup", threshold = get_relevance_threshold(), "Loaded stored relevance threshold");
        } else {
            set_relevance_threshold(0.40);
            info!(target: "4da::startup", threshold = get_relevance_threshold(), "Relevance threshold (default)");
        }
    } else {
        set_relevance_threshold(0.40);
        info!(target: "4da::startup", threshold = get_relevance_threshold(), "Relevance threshold (default, ACE unavailable)");
    }

    // Initialize database early
    match get_database() {
        Ok(db) => {
            let ctx_count = db.context_count().unwrap_or(0);
            let item_count = db.total_item_count().unwrap_or(0);
            info!(target: "4da::startup", context_chunks = ctx_count, source_items = item_count, "Database ready");
        }
        Err(e) => {
            error!(target: "4da::startup", error = %e, "Database initialization failed");
        }
    }

    // Initialize context engine
    match get_context_engine() {
        Ok(engine) => {
            let interest_count = engine.interest_count().unwrap_or(0);
            let exclusion_count = engine.exclusion_count().unwrap_or(0);
            if let Ok(identity) = engine.get_static_identity() {
                let role_str = identity.role.as_deref().unwrap_or("Not set");
                info!(target: "4da::startup",
                    interests = interest_count,
                    exclusions = exclusion_count,
                    role = role_str,
                    "Context Engine ready"
                );
                if !identity.tech_stack.is_empty() {
                    debug!(target: "4da::startup", tech_stack = %identity.tech_stack.join(", "), "Tech Stack");
                }
                if !identity.domains.is_empty() {
                    debug!(target: "4da::startup", domains = %identity.domains.join(", "), "Domains");
                }
            }
        }
        Err(e) => {
            error!(target: "4da::startup", error = %e, "Context Engine initialization failed");
        }
    }

    // Initialize source registry
    let registry = get_source_registry();
    let source_count = registry.lock().count();
    let source_names: Vec<String> = registry
        .lock()
        .sources()
        .iter()
        .map(|s| s.name().to_string())
        .collect();
    info!(target: "4da::startup", count = source_count, sources = %source_names.join(", "), "Sources registered");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            context_commands::get_context_files,
            context_commands::clear_context,
            context_commands::index_context,
            context_commands::index_project_readmes,
            context_commands::get_context_settings,
            context_commands::set_context_dirs,
            get_hn_top_stories,
            compute_relevance,
            get_database_stats,
            get_sources,
            analysis::run_deep_initial_scan,
            analysis::run_cached_analysis,
            analysis::get_analysis_status,
            analysis::cancel_analysis,
            // Settings commands
            settings_commands::get_settings,
            settings_commands::set_llm_provider,
            settings_commands::mark_onboarding_complete,
            settings_commands::set_rerank_config,
            settings_commands::test_llm_connection,
            settings_commands::check_ollama_status,
            settings_commands::pull_ollama_model,
            settings_commands::get_usage_stats,
            // Monitoring commands (Phase 3)
            monitoring_commands::get_monitoring_status,
            monitoring_commands::set_monitoring_enabled,
            monitoring_commands::set_monitoring_interval,
            monitoring_commands::set_notification_threshold,
            monitoring_commands::trigger_notification_test,
            // Context Engine commands
            settings_commands::get_user_context,
            settings_commands::set_user_role,
            settings_commands::add_tech_stack,
            settings_commands::remove_tech_stack,
            settings_commands::add_domain,
            settings_commands::remove_domain,
            settings_commands::add_interest,
            settings_commands::remove_interest,
            settings_commands::add_exclusion,
            settings_commands::remove_exclusion,
            settings_commands::record_interaction,
            settings_commands::get_context_stats,
            settings_commands::get_current_threshold,
            // ACE (Autonomous Context Engine) commands - Phase A
            ace_commands::ace_detect_context,
            ace_commands::ace_get_detected_tech,
            ace_commands::ace_get_active_topics,
            // ACE Phase B: Real-Time Context
            ace_commands::ace_analyze_git,
            ace_commands::ace_get_realtime_context,
            ace_commands::ace_apply_decay,
            ace_commands::ace_full_scan,
            // ACE Autonomous Discovery
            ace_commands::ace_auto_discover,
            ace_commands::ace_reset_discovery,
            ace_commands::ace_get_discovery_status,
            // ACE Phase C: Behavior Learning
            ace_commands::ace_record_interaction,
            ace_commands::ace_get_topic_affinities,
            ace_commands::ace_get_anti_topics,
            ace_commands::ace_confirm_anti_topic,
            ace_commands::ace_get_behavior_modifier,
            ace_commands::ace_get_learned_behavior,
            ace_commands::ace_apply_behavior_decay,
            // ACE Phase E: Embedding
            ace_commands::ace_embed_topic,
            ace_commands::ace_find_similar_topics,
            ace_commands::ace_embedding_status,
            // ACE Phase E: Watcher Persistence
            ace_commands::ace_save_watcher_state,
            ace_commands::ace_get_watcher_state,
            ace_commands::ace_clear_watcher_state,
            // ACE Phase E: Rate Limiting
            ace_commands::ace_get_rate_limit_status,
            // ACE Auto-Interest Discovery
            ace_commands::ace_get_suggested_interests,
            // ACE Phase 1C: Anomaly Detection
            ace_commands::ace_get_unresolved_anomalies,
            ace_commands::ace_detect_anomalies,
            ace_commands::ace_resolve_anomaly,
            ace_commands::ace_get_accuracy_metrics,
            ace_commands::ace_record_accuracy_feedback,
            // ACE Phase 4: Visible Learning Loop
            ace_commands::ace_get_single_affinity,
            // ACE Phase 1D: Health Monitoring
            ace_commands::ace_get_system_health,
            // ACE Watcher Control
            ace_commands::ace_start_watcher,
            ace_commands::ace_stop_watcher,
            ace_commands::ace_is_watching,
            // Update commands
            digest_commands::check_for_updates,
            digest_commands::get_current_version,
            // Digest commands
            digest_commands::get_digest_config,
            digest_commands::set_digest_config,
            digest_commands::generate_digest,
            digest_commands::preview_digest,
            // RSS commands
            source_config::get_rss_feeds,
            source_config::add_rss_feed,
            source_config::remove_rss_feed,
            source_config::set_rss_feeds,
            // Twitter commands
            source_config::get_twitter_handles,
            source_config::add_twitter_handle,
            source_config::remove_twitter_handle,
            source_config::set_twitter_handles,
            source_config::get_nitter_instance,
            source_config::set_nitter_instance,
            // X API key commands
            source_config::get_x_api_key,
            source_config::set_x_api_key,
            // YouTube commands
            source_config::get_youtube_channels,
            source_config::add_youtube_channel,
            source_config::remove_youtube_channel,
            source_config::set_youtube_channels,
            // GitHub commands
            source_config::get_github_languages,
            source_config::set_github_languages,
            // AI Briefing commands
            digest_commands::generate_ai_briefing,
            // MCP Score Autopsy
            mcp_score_autopsy,
            // Indexed Documents commands
            document_index::get_indexed_documents,
            document_index::get_document_content,
            document_index::search_documents,
            document_index::get_indexed_stats,
            // Natural Language Query (Phase 2)
            document_index::natural_language_query,
            // Job Queue commands (background extraction)
            job_queue_commands::create_extraction_job,
            job_queue_commands::get_extraction_job,
            job_queue_commands::get_extraction_jobs,
            job_queue_commands::get_job_queue_stats,
            job_queue_commands::cancel_extraction_job,
            job_queue_commands::start_job_queue_worker,
            job_queue_commands::stop_job_queue_worker,
            job_queue_commands::cleanup_extraction_jobs,
            // Void Engine
            void_commands::get_void_signal,
            // Signal Classifier
            analysis::get_actionable_signals,
            // Product Hardening (source management, health, maintenance, export)
            toggle_source_enabled,
            get_all_sources_status,
            get_source_health,
            check_network_status,
            run_db_maintenance,
            get_db_stats_detailed,
            export_results,
            // Attention Economy Dashboard
            attention::get_attention_report,
            attention::get_attention_blind_spots,
            // Temporal Event Store
            temporal::get_temporal_events,
            temporal::get_temporal_event_count,
            temporal::get_dependencies,
            temporal::cleanup_temporal_events,
            // Audio Briefings (TTS)
            tts::generate_audio_briefing,
            tts::get_audio_briefing_status,
            tts::get_audio_file_path,
            // Predictive Context Switching
            predictive::get_predicted_context,
            predictive::record_context_switch_event,
            predictive::get_context_switch_history,
            // Knowledge Decay Alerting
            knowledge_decay::get_knowledge_gaps,
            knowledge_decay::get_knowledge_gap_count,
            // Context Handoff Protocol
            handoff::generate_context_packet,
            handoff::export_context_packet_to_file,
            handoff::import_context_packet,
            // Reverse Relevance
            reverse_relevance::get_reverse_mentions,
            reverse_relevance::get_my_project_identifiers,
            // Project Health Radar
            project_health::get_project_health,
            project_health::get_project_health_summary,
            // Semantic Diff Engine
            semantic_diff::get_semantic_shifts,
            semantic_diff::get_topic_centroids,
            // Signal Chains
            signal_chains::get_signal_chains,
            signal_chains::resolve_signal_chain,
            signal_chains::get_signal_chain_count
        ])
        .setup(|app| {
            // Set up system tray
            let tray = monitoring::setup_tray(app.handle()).expect("Failed to set up system tray");

            // Store tray handle for later updates
            app.manage(std::sync::Mutex::new(Some(tray)));

            // Load monitoring settings from persistence
            let monitoring_state = get_monitoring_state().clone();
            {
                let settings = get_settings_manager().lock();
                let config = settings.get_monitoring_config();
                monitoring_state.set_enabled(config.enabled);
                monitoring_state.set_interval(config.interval_minutes * 60);
                info!(target: "4da::monitor", enabled = config.enabled, interval_mins = config.interval_minutes, "Loaded monitoring settings");
            }

            // Start background scheduler
            let app_handle = app.handle().clone();
            monitoring::start_scheduler(app_handle.clone(), monitoring_state.clone());

            // Listen for tray events
            let app_handle_analyze = app_handle.clone();
            app.listen("tray-analyze", move |_| {
                info!(target: "4da::tray", "Manual analysis triggered from tray");
                let _ = app_handle_analyze.emit("start-analysis-from-tray", ());
            });

            let app_handle_toggle = app_handle.clone();
            app.listen("tray-toggle-monitoring", move |_| {
                let state = get_monitoring_state();
                let new_enabled = !state.is_enabled();
                state.set_enabled(new_enabled);
                info!(target: "4da::monitor", enabled = new_enabled, "Monitoring toggled");
                let _ = app_handle_toggle.emit("monitoring-toggled", new_enabled);
            });

            // Listen for scheduled analysis events
            // Uses cache-first approach: fetch to fill cache, then analyze cached items
            let app_handle_scheduled = app_handle.clone();
            app.listen("scheduled-analysis", move |_| {
                info!(target: "4da::monitor", "Scheduled analysis starting (cache-first)");
                let handle = app_handle_scheduled.clone();
                tauri::async_runtime::spawn(async move {
                    // Step 1: Fill cache with deep fetch (background, no UI blocking)
                    info!(target: "4da::monitor", "Step 1: Filling cache with deep fetch...");
                    if let Err(e) = fill_cache_background(&handle).await {
                        warn!(target: "4da::monitor", error = %e, "Cache fill failed, continuing with existing cache");
                    }

                    // Step 2: Analyze cached content (INSTANT)
                    info!(target: "4da::monitor", "Step 2: Analyzing cached content...");
                    match analysis::analyze_cached_content_impl(&handle).await {
                        Ok(results) => {
                            let relevant_count = results.iter().filter(|r| r.relevant).count();

                            // Build signal summary for notifications
                            let signal_summary = {
                                let critical_count = results.iter()
                                    .filter(|r| r.signal_priority.as_deref() == Some("critical"))
                                    .count();
                                let high_count = results.iter()
                                    .filter(|r| r.signal_priority.as_deref() == Some("high"))
                                    .count();
                                let top_signal = results.iter()
                                    .filter(|r| r.signal_type.is_some())
                                    .max_by(|a, b| {
                                        let pa = match a.signal_priority.as_deref() {
                                            Some("critical") => 4u8,
                                            Some("high") => 3,
                                            Some("medium") => 2,
                                            _ => 1,
                                        };
                                        let pb = match b.signal_priority.as_deref() {
                                            Some("critical") => 4u8,
                                            Some("high") => 3,
                                            Some("medium") => 2,
                                            _ => 1,
                                        };
                                        pa.cmp(&pb).then_with(|| {
                                            a.top_score.partial_cmp(&b.top_score)
                                                .unwrap_or(std::cmp::Ordering::Equal)
                                        })
                                    })
                                    .and_then(|r| {
                                        Some((
                                            r.signal_type.clone()?,
                                            r.signal_action.clone()?,
                                        ))
                                    });
                                if critical_count > 0 || high_count > 0 {
                                    Some(monitoring::SignalSummary {
                                        critical_count,
                                        high_count,
                                        top_signal,
                                    })
                                } else {
                                    None
                                }
                            };

                            let state = get_monitoring_state();
                            monitoring::complete_scheduled_check(
                                &handle,
                                state,
                                relevant_count,
                                results.len(),
                                signal_summary,
                            );
                            // Emit results to frontend if window is visible
                            void_signal_analysis_complete(&handle, &results);
                            let _ = handle.emit("analysis-complete", results);
                        }
                        Err(e) => {
                            error!(target: "4da::monitor", error = %e, "Scheduled analysis failed");
                            void_signal_error(&handle);
                            let state = get_monitoring_state();
                            state
                                .is_checking
                                .store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                });
            });

            info!(target: "4da::tray", "System tray and monitoring initialized");

            // Ensure Ollama models are available and warm on startup
            {
                let settings = get_settings_manager().lock();
                let llm = &settings.get().llm;
                if llm.provider == "ollama" && !llm.model.is_empty() {
                    let model = llm.model.clone();
                    let base_url = llm.base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
                    let warm_handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        ollama::ensure_models_available(&model, &base_url, &warm_handle).await;
                    });
                }
            }

            // Emit initial void signal (shows current state to heartbeat)
            if let Ok(db) = get_database() {
                let mon = get_monitoring_state();
                let signal = void_engine::compute_signal(db, mon);
                void_engine::emit_if_changed(&app_handle, signal);
            }

            // Staleness timer: update void signal once per minute
            // This is the ONLY timer in the void engine - everything else is change-driven
            let app_handle_staleness = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    if let Ok(db) = get_database() {
                        let mon = get_monitoring_state();
                        let signal = void_engine::compute_signal(db, mon);
                        void_engine::emit_if_changed(&app_handle_staleness, signal);
                    }
                }
            });

            // Initialize ACE with configured directories (runs async in background)
            initialize_ace_on_startup(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                info!(target: "4da::shutdown", "Application shutting down - cleaning up...");
                // Disable monitoring to stop scheduler
                let state = get_monitoring_state();
                state.set_enabled(false);
                // Clean up temp extraction directory
                if let Ok(data_dir) = std::env::var("APPDATA") {
                    let temp_dir = std::path::PathBuf::from(data_dir).join("4da").join("temp");
                    if temp_dir.exists() {
                        let _ = std::fs::remove_dir_all(&temp_dir);
                        info!(target: "4da::shutdown", "Cleaned up temp directory");
                    }
                }
                info!(target: "4da::shutdown", "Cleanup complete");
            }
        });
}

/// Get registered sources
#[tauri::command]
async fn get_sources() -> Result<Vec<serde_json::Value>, String> {
    let registry = get_source_registry();
    let guard = registry.lock();

    let sources: Vec<serde_json::Value> = guard
        .sources()
        .iter()
        .map(|s| {
            serde_json::json!({
                "type": s.source_type(),
                "name": s.name(),
                "enabled": s.config().enabled,
                "max_items": s.config().max_items,
                "fetch_interval_secs": s.config().fetch_interval_secs
            })
        })
        .collect();

    Ok(sources)
}

/// Get database statistics
#[tauri::command]
async fn get_database_stats() -> Result<serde_json::Value, String> {
    let db = get_database()?;

    let context_count = db.context_count().map_err(|e| e.to_string())?;
    let hn_count = db
        .source_item_count("hackernews")
        .map_err(|e| e.to_string())?;
    let total_count = db.total_item_count().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "context_chunks": context_count,
        "hackernews_items": hn_count,
        "total_items": total_count
    }))
}

// Analysis functions (start_background_analysis, run_multi_source_analysis, etc.) are in analysis.rs
// Settings and Context Engine commands are in settings_commands.rs
// ACE commands, PASIFA helpers, and auto-seeding are in ace_commands.rs

// ============================================================================
// Product Hardening Commands
// ============================================================================

/// Toggle a source's enabled/disabled status
#[tauri::command]
async fn toggle_source_enabled(source_type: String, enabled: bool) -> Result<(), String> {
    let db = get_database()?;
    db.toggle_source_enabled(&source_type, enabled)
        .map_err(|e| format!("Failed to toggle source: {}", e))
}

/// Get all sources with their enabled status and health
#[tauri::command]
async fn get_all_sources_status() -> Result<serde_json::Value, String> {
    let db = get_database()?;
    let sources = db.get_all_sources().map_err(|e| e.to_string())?;
    let health = db.get_source_health().unwrap_or_default();

    let result: Vec<serde_json::Value> = sources
        .iter()
        .map(|(source_type, name, enabled, last_fetch)| {
            let h = health.iter().find(|h| h.source_type == *source_type);
            serde_json::json!({
                "source_type": source_type,
                "name": name,
                "enabled": enabled,
                "last_fetch": last_fetch,
                "status": h.map(|h| h.status.as_str()).unwrap_or("unknown"),
                "error_count": h.map(|h| h.error_count).unwrap_or(0),
                "consecutive_failures": h.map(|h| h.consecutive_failures).unwrap_or(0),
                "items_fetched": h.map(|h| h.items_fetched).unwrap_or(0),
                "response_time_ms": h.map(|h| h.response_time_ms).unwrap_or(0),
                "last_success": h.and_then(|h| h.last_success.clone()),
                "last_error": h.and_then(|h| h.last_error.clone()),
            })
        })
        .collect();

    Ok(serde_json::json!({ "sources": result }))
}

/// Get source health data
#[tauri::command]
async fn get_source_health() -> Result<serde_json::Value, String> {
    let db = get_database()?;
    let health = db.get_source_health().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "health": health }))
}

/// Check network connectivity
#[tauri::command]
async fn check_network_status() -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let online = client.head("https://httpbin.org/get").send().await.is_ok();

    Ok(serde_json::json!({
        "online": online,
        "checked_at": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Run database maintenance (cleanup old items, vacuum)
#[tauri::command]
async fn run_db_maintenance() -> Result<serde_json::Value, String> {
    let db = get_database()?;
    let result = db.run_maintenance(90).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "deleted_items": result.deleted_items,
        "deleted_feedback": result.deleted_feedback,
        "deleted_void": result.deleted_void,
    }))
}

/// Get database statistics
#[tauri::command]
async fn get_db_stats_detailed() -> Result<serde_json::Value, String> {
    let db = get_database()?;
    let stats = db.get_db_stats().map_err(|e| e.to_string())?;

    // Get DB file size
    let db_path = get_db_path();
    let file_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    Ok(serde_json::json!({
        "source_items": stats.source_items,
        "context_chunks": stats.context_chunks,
        "feedback_count": stats.feedback_count,
        "sources_count": stats.sources_count,
        "file_size_bytes": file_size,
        "file_size_mb": format!("{:.1}", file_size as f64 / 1_048_576.0),
    }))
}

/// Export current analysis results in specified format
#[tauri::command]
async fn export_results(format: String) -> Result<String, String> {
    let state = get_analysis_state();
    let guard = state.lock();

    let results = match &guard.results {
        Some(r) => r,
        None => return Err("No analysis results to export".to_string()),
    };

    let relevant: Vec<&SourceRelevance> = results.iter().filter(|r| r.relevant).collect();

    match format.as_str() {
        "markdown" => {
            let mut md = String::from("# 4DA Analysis Results\n\n");
            md.push_str(&format!(
                "**Generated:** {}\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
            ));
            md.push_str(&format!(
                "**Total items:** {} ({} relevant)\n\n",
                results.len(),
                relevant.len()
            ));
            md.push_str("---\n\n");
            for item in &relevant {
                let score_pct = (item.top_score * 100.0) as u32;
                md.push_str(&format!("### {} ({}%)\n", item.title, score_pct));
                if let Some(ref url) = item.url {
                    md.push_str(&format!("- **URL:** {}\n", url));
                }
                md.push_str(&format!("- **Source:** {}\n", item.source_type));
                if let Some(ref explanation) = item.explanation {
                    md.push_str(&format!("- **Why:** {}\n", explanation));
                }
                md.push('\n');
            }
            Ok(md)
        }
        "text" => {
            let mut text = format!(
                "4DA Analysis Results ({})\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
            );
            text.push_str(&format!(
                "{} items, {} relevant\n\n",
                results.len(),
                relevant.len()
            ));
            for item in &relevant {
                let score_pct = (item.top_score * 100.0) as u32;
                text.push_str(&format!(
                    "[{}%] {} ({})\n",
                    score_pct, item.title, item.source_type
                ));
                if let Some(ref url) = item.url {
                    text.push_str(&format!("  {}\n", url));
                }
            }
            Ok(text)
        }
        "html" => {
            let mut html = String::from("<html><head><title>4DA Analysis Results</title></head><body style='font-family:sans-serif;background:#0A0A0A;color:#fff;padding:2rem'>");
            html.push_str(&format!(
                "<h1>4DA Analysis Results</h1><p>{} items, {} relevant</p><hr>",
                results.len(),
                relevant.len()
            ));
            for item in &relevant {
                let score_pct = (item.top_score * 100.0) as u32;
                html.push_str("<div style='margin:1rem 0;padding:1rem;background:#141414;border-radius:8px;border:1px solid #2A2A2A'>");
                html.push_str(&format!("<strong>{}%</strong> ", score_pct));
                if let Some(ref url) = item.url {
                    html.push_str(&format!(
                        "<a href='{}' style='color:#D4AF37'>{}</a>",
                        url, item.title
                    ));
                } else {
                    html.push_str(&item.title);
                }
                html.push_str(&format!(
                    " <span style='color:#666'>({})</span>",
                    item.source_type
                ));
                html.push_str("</div>");
            }
            html.push_str("</body></html>");
            Ok(html)
        }
        _ => Err(format!(
            "Unknown format: {}. Use 'markdown', 'text', or 'html'",
            format
        )),
    }
}

// ============================================================================
// Startup Initialization
// ============================================================================

/// Initialize ACE on startup with automatic context discovery
/// This is the core of ACE AUTONOMY - the system discovers context without manual configuration
fn initialize_ace_on_startup(app_handle: tauri::AppHandle) {
    // Check if auto-discovery is needed (first run with no context dirs)
    let needs_discovery = {
        let settings = get_settings_manager().lock();
        settings.needs_auto_discovery()
    };

    if needs_discovery {
        info!(target: "4da::startup", "First run detected - running AUTONOMOUS context discovery");
        let _ = app_handle.emit(
            "ace-discovery-started",
            "Discovering your development context...",
        );

        // Phase 1: Discover common dev directories
        let discovered_dirs = crate::settings::discover_dev_directories();

        if discovered_dirs.is_empty() {
            warn!(target: "4da::startup", "No dev directories found. User will need to configure manually");
            // Mark as completed so we don't keep trying
            let mut settings = get_settings_manager().lock();
            let _ = settings.mark_auto_discovery_completed();
        } else {
            // Phase 2: Deep scan for actual project directories
            info!(target: "4da::startup", dirs = discovered_dirs.len(), "Scanning directories for projects");
            let project_dirs = crate::settings::find_project_directories(&discovered_dirs, 3);

            // Use discovered dev directories (or project dirs if we want more granular)
            // For now, use the top-level dev dirs to allow ACE scanner to find all projects
            let dirs_to_add = if project_dirs.len() > 50 {
                // Too many projects - use parent directories instead
                debug!(target: "4da::startup", projects = project_dirs.len(), "Too many projects, using parent directories");
                discovered_dirs
            } else if !project_dirs.is_empty() {
                debug!(target: "4da::startup", projects = project_dirs.len(), "Found projects");
                project_dirs
            } else {
                debug!(target: "4da::startup", "No projects found, using discovered directories");
                discovered_dirs
            };

            // Save discovered directories to settings
            {
                let mut settings = get_settings_manager().lock();
                if let Err(e) = settings.add_context_dirs(dirs_to_add.clone()) {
                    error!(target: "4da::startup", error = %e, "Failed to save discovered directories");
                }
                let _ = settings.mark_auto_discovery_completed();
            }

            let _ = app_handle.emit(
                "ace-discovery-complete",
                serde_json::json!({
                    "directories_found": dirs_to_add.len(),
                    "directories": dirs_to_add
                }),
            );
        }
    }

    // Now get all context directories (either pre-configured or just discovered)
    let context_dirs = get_context_dirs();

    if context_dirs.is_empty() {
        warn!(target: "4da::startup", "No context directories available, ACE will wait for configuration");
        return;
    }

    info!(target: "4da::startup", dirs = context_dirs.len(), "Initializing ACE");

    // Spawn async task for ACE initialization
    tauri::async_runtime::spawn(async move {
        // Small delay to let the app fully initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let paths: Vec<String> = context_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect();

        // Run full scan - this builds the context profile AUTONOMOUSLY
        info!(target: "4da::startup", "Running AUTONOMOUS ACE context scan");
        let _ = app_handle.emit("ace-scan-started", "Building your context profile...");

        match ace_commands::ace_full_scan(paths.clone()).await {
            Ok(result) => {
                info!(target: "4da::startup", result = %result, "ACE context scan complete");
                let _ = app_handle.emit("ace-scan-complete", result);
            }
            Err(e) => {
                error!(target: "4da::startup", error = %e, "ACE scan failed");
                let _ = app_handle.emit("ace-scan-error", e.clone());
            }
        }

        // AUTO-SEED: Populate interests from ACE-detected tech if interests are empty
        // This provides immediate value without requiring manual configuration
        if let Err(e) = ace_commands::auto_seed_interests_from_ace().await {
            warn!(target: "4da::startup", error = %e, "Auto-seeding interests failed (non-fatal)");
        }

        // PASIFA: Index README files from discovered projects for semantic search
        // This makes discovered context contribute to embedding-based relevance
        debug!(target: "4da::startup", "Indexing README files from discovered projects");
        let indexed_count = ace_commands::index_discovered_readmes(&context_dirs).await;
        if indexed_count > 0 {
            info!(target: "4da::startup", count = indexed_count, "Indexed README files for semantic search");
            let _ = app_handle.emit(
                "ace-readme-indexed",
                serde_json::json!({
                    "count": indexed_count
                }),
            );
        }

        // Start file watcher for continuous context updates
        debug!(target: "4da::startup", "Starting ACE FileWatcher for continuous monitoring");
        match ace_commands::ace_start_watcher(paths).await {
            Ok(result) => {
                info!(target: "4da::startup", result = %result, "ACE FileWatcher started");
                let _ = app_handle.emit("ace-watcher-started", result);
            }
            Err(e) => {
                warn!(target: "4da::startup", error = %e, "ACE FileWatcher failed");
            }
        }

        info!(target: "4da::startup", "ACE AUTONOMOUS initialization complete - context is now being built");
    });
}

// ============================================================================
// PASIFA Integration Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test cosine similarity helper
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 0.001,
            "Identical vectors should have similarity 1.0"
        );

        let c = vec![0.0, 1.0, 0.0];
        let sim_orth = cosine_similarity(&a, &c);
        assert!(
            sim_orth.abs() < 0.001,
            "Orthogonal vectors should have similarity 0.0"
        );
    }

    // ====================================================================
    // Utility Function Tests
    // ====================================================================

    #[test]
    fn test_truncate_utf8_ascii() {
        assert_eq!(truncate_utf8("hello world", 5), "hello");
        assert_eq!(truncate_utf8("hello", 10), "hello");
        assert_eq!(truncate_utf8("", 5), "");
    }

    #[test]
    fn test_truncate_utf8_multibyte() {
        // Cyrillic: each char is 2 bytes
        let cyrillic = "Привет мир";
        let result = truncate_utf8(cyrillic, 6);
        assert_eq!(result, "Привет");

        // Chinese: each char is 3 bytes
        let chinese = "你好世界";
        let result = truncate_utf8(chinese, 2);
        assert_eq!(result, "你好");
    }

    #[test]
    fn test_extract_topics_basic() {
        let topics = extract_topics(
            "Rust async patterns for Tauri apps",
            "A guide to async Rust",
        );
        assert!(!topics.is_empty());
        // Should extract meaningful words, not stopwords
        assert!(topics
            .iter()
            .any(|t| t.contains("rust") || t.contains("tauri") || t.contains("async")));
    }

    #[test]
    fn test_extract_topics_empty() {
        let topics = extract_topics("", "");
        assert!(topics.is_empty());
    }

    #[test]
    fn test_extract_topics_optimized() {
        // Test single-word keyword extraction
        let topics = extract_topics(
            "Building a Rust web server",
            "Using async/await with PostgreSQL database",
        );
        assert!(
            topics.contains(&"rust".to_string()),
            "Should extract 'rust'"
        );
        assert!(
            topics.contains(&"postgresql".to_string()),
            "Should extract 'postgresql'"
        );
        assert!(
            topics.contains(&"database".to_string()),
            "Should extract 'database'"
        );

        // Test multi-word phrase extraction
        let topics2 = extract_topics(
            "Machine Learning with Python",
            "Deep learning and open source tools",
        );
        assert!(
            topics2.contains(&"machine learning".to_string()),
            "Should extract 'machine learning'"
        );
        assert!(
            topics2.contains(&"deep learning".to_string()),
            "Should extract 'deep learning'"
        );
        assert!(
            topics2.contains(&"open source".to_string()),
            "Should extract 'open source'"
        );
        assert!(
            topics2.contains(&"python".to_string()),
            "Should extract 'python'"
        );

        // Test special character handling (c++)
        let topics3 = extract_topics("C++ programming", "Using C++ for systems programming");
        assert!(topics3.contains(&"c++".to_string()), "Should extract 'c++'");

        // Test no duplicates
        let topics4 = extract_topics("Rust Rust Rust", "rust rust rust everywhere");
        let rust_count = topics4.iter().filter(|t| *t == "rust").count();
        assert_eq!(rust_count, 1, "Should not have duplicates");
    }

    #[test]
    fn test_check_exclusions_none() {
        let topics = vec!["rust".to_string(), "webdev".to_string()];
        let exclusions = vec!["crypto".to_string()];
        assert!(check_exclusions(&topics, &exclusions).is_none());
    }

    #[test]
    fn test_check_exclusions_match() {
        let topics = vec!["rust".to_string(), "cryptocurrency".to_string()];
        let exclusions = vec!["crypto".to_string()];
        let result = check_exclusions(&topics, &exclusions);
        assert!(result.is_some(), "Should match 'crypto' substring");
    }

    #[test]
    fn test_chunk_text_short() {
        let text = "Short text.";
        let chunks = chunk_text(text, "test.txt");
        assert_eq!(chunks.len(), 1);
        // Tuple is (source_file, content)
        assert_eq!(chunks[0].0, "test.txt");
        assert_eq!(chunks[0].1, "Short text.");
    }

    #[test]
    fn test_chunk_text_multi_paragraph() {
        // Create text with multiple paragraphs
        let mut paragraphs = Vec::new();
        for i in 0..20 {
            paragraphs.push(format!("Paragraph {} with some meaningful content about software development and engineering principles.", i));
        }
        let text = paragraphs.join("\n\n");
        let chunks = chunk_text(&text, "test.md");
        assert!(!chunks.is_empty());
        // Each chunk: (source_file, content)
        for (source, _content) in &chunks {
            assert_eq!(source, "test.md");
        }
    }

    #[test]
    fn test_build_embedding_text() {
        let result = build_embedding_text("My Title", "Some content here");
        assert!(result.contains("My Title"));
        assert!(result.contains("Some content here"));
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 0.001);
    }
}
