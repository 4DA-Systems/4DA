// FASTEMBED DISABLED: ONNX linking issues on Windows - using OpenAI only
// use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tracing::{debug, error, info, warn};

mod ace;
mod context_engine;
mod db;
mod digest;
pub mod extractors;
mod job_queue;
mod llm;
mod monitoring;
pub mod query;
mod settings;
mod signals;
mod sources;
mod void_engine;

use context_engine::{ContextEngine, InteractionType, InterestSource};
use db::Database;
use llm::RelevanceJudge;
use settings::{LLMProvider, RerankConfig, SettingsManager};
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
fn truncate_utf8(s: &str, max_chars: usize) -> String {
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
pub struct HNItem {
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
    pub ace_boost: f32,
    pub affinity_mult: f32,
    pub anti_penalty: f32,
    #[serde(default = "default_freshness")]
    pub freshness_mult: f32,
    pub confidence_by_signal: std::collections::HashMap<String, f32>,
}

fn default_freshness() -> f32 {
    1.0
}

/// Full relevance result for a source item (HN, arXiv, Reddit, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HNRelevance {
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
    pub results: Option<Vec<HNRelevance>>,
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
pub(crate) fn embed_texts(texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    // Get settings to determine provider
    let settings = get_settings_manager().lock();
    let llm_settings = settings.get().llm.clone();
    drop(settings);

    match llm_settings.provider.as_str() {
        "openai" => embed_texts_openai(texts, &llm_settings.api_key),
        "ollama" => embed_texts_ollama(texts, &llm_settings.base_url),
        "anthropic" => {
            // Anthropic doesn't have embeddings API - use dedicated OpenAI key or fallback to Ollama
            if !llm_settings.openai_api_key.is_empty() {
                return embed_texts_openai(texts, &llm_settings.openai_api_key);
            }
            // Try Ollama as fallback
            if let Some(base_url) = &llm_settings.base_url {
                if !base_url.is_empty() {
                    if let Ok(result) = embed_texts_ollama(texts, &Some(base_url.clone())) {
                        return Ok(result);
                    }
                }
            }
            // Try default Ollama
            embed_texts_ollama(texts, &None)
        }
        _ => Err(format!(
            "Unknown provider: {}. Please configure OpenAI or Ollama.",
            llm_settings.provider
        )),
    }
}

/// Generate embeddings using OpenAI API
fn embed_texts_openai(texts: &[String], api_key: &str) -> Result<Vec<Vec<f32>>, String> {
    if api_key.is_empty() {
        return Err("OpenAI API key not configured".to_string());
    }

    let client = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(30))
        .build();

    let response = client
        .post("https://api.openai.com/v1/embeddings")
        .set("Authorization", &format!("Bearer {}", api_key))
        .set("Content-Type", "application/json")
        .send_json(ureq::json!({
            "model": "text-embedding-3-small",
            "input": texts,
            "dimensions": 384  // Match DB vec0 schema (384-dim MiniLM-compatible)
        }))
        .map_err(|e| format!("OpenAI API request failed: {}", e))?;

    let json: serde_json::Value = response
        .into_json()
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

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

/// Generate embeddings using Ollama API
fn embed_texts_ollama(
    texts: &[String],
    base_url: &Option<String>,
) -> Result<Vec<Vec<f32>>, String> {
    let base = base_url.as_deref().unwrap_or("http://localhost:11434");

    let client = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(60))
        .build();

    // Ollama embeddings API doesn't support batch, so we process one at a time
    let mut all_embeddings = Vec::with_capacity(texts.len());

    for text in texts {
        let response = client
            .post(&format!("{}/api/embeddings", base))
            .set("Content-Type", "application/json")
            .send_json(ureq::json!({
                "model": "nomic-embed-text",
                "prompt": text,
            }))
            .map_err(|e| format!("Ollama API request failed: {}. Make sure Ollama is running with 'nomic-embed-text' model installed (run: ollama pull nomic-embed-text)", e))?;

        let json: serde_json::Value = response
            .into_json()
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        let embedding = json["embedding"]
            .as_array()
            .ok_or_else(|| "Invalid Ollama response: missing 'embedding' array".to_string())?
            .iter()
            .map(|v| {
                v.as_f64()
                    .map(|f| f as f32)
                    .ok_or_else(|| "Invalid embedding value".to_string())
            })
            .collect::<Result<Vec<f32>, String>>()?;

        all_embeddings.push(embedding);
    }

    Ok(all_embeddings)
}

/// Cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Extract topics/keywords from text for context matching
/// Returns lowercase keywords suitable for exclusion/interest matching
fn extract_topics(title: &str, content: &str) -> Vec<String> {
    let mut topics = Vec::new();

    // Combine title and first part of content
    let text = format!(
        "{} {}",
        title,
        content.chars().take(500).collect::<String>()
    );
    let text_lower = text.to_lowercase();

    // Technology keywords to look for
    let tech_keywords = [
        "rust",
        "python",
        "javascript",
        "typescript",
        "go",
        "golang",
        "java",
        "c++",
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
        "machine learning",
        "deep learning",
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
        "open source",
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
        "react native",
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
    ];

    for keyword in &tech_keywords {
        if text_lower.contains(keyword) {
            topics.push(keyword.to_string());
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
            if !topics.contains(&lower)
                && !["the", "and", "for", "how", "why", "what", "show", "ask"]
                    .contains(&lower.as_str())
            {
                topics.push(lower);
            }
        }
    }

    topics
}

/// Check if an item should be excluded based on user exclusions
/// Returns Some(exclusion) if blocked, None if allowed
fn check_exclusions(topics: &[String], exclusions: &[String]) -> Option<String> {
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

/// Compute interest score by comparing item embedding against interest embeddings
fn compute_interest_score(item_embedding: &[f32], interests: &[context_engine::Interest]) -> f32 {
    if interests.is_empty() {
        return 0.0;
    }

    let mut max_score: f32 = 0.0;

    for interest in interests {
        if let Some(ref interest_embedding) = interest.embedding {
            let similarity = cosine_similarity(item_embedding, interest_embedding);
            let weighted = similarity * interest.weight;
            max_score = max_score.max(weighted);
        }
    }

    max_score
}

/// Generate a human-readable explanation for why an item was considered relevant
/// Returns a concise explanation suitable for display in the UI
fn generate_relevance_explanation(
    _title: &str,
    context_score: f32,
    interest_score: f32,
    matches: &[RelevanceMatch],
    ace_ctx: &ACEContext,
    item_topics: &[String],
) -> String {
    let mut reasons: Vec<String> = Vec::new();

    // Check context matches (what you're working on)
    if context_score > 0.2 {
        if let Some(first_match) = matches.first() {
            // Extract just the filename from the path
            let file_name = first_match
                .source_file
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&first_match.source_file);
            reasons.push(format!("Matches your current work ({})", file_name));
        } else {
            reasons.push("Relates to your active projects".to_string());
        }
    }

    // Check interest matches (declared interests)
    if interest_score > 0.2 {
        reasons.push("Matches your declared interests".to_string());
    }

    // Check ACE tech stack matches
    for topic in item_topics {
        let topic_lower = topic.to_lowercase();
        if let Some(tech) = ace_ctx
            .detected_tech
            .iter()
            .find(|t| t.to_lowercase() == topic_lower)
        {
            reasons.push(format!("Your project uses {}", tech));
            break;
        }
    }

    // Check ACE active topics matches (recent activity)
    for topic in item_topics {
        let topic_lower = topic.to_lowercase();
        if let Some(active_topic) = ace_ctx
            .active_topics
            .iter()
            .find(|t| t.to_lowercase() == topic_lower || topic_lower.contains(&t.to_lowercase()))
        {
            reasons.push(format!("Matches your recent activity: {}", active_topic));
            break;
        }
    }

    // Check learned affinity matches
    for topic in item_topics {
        let topic_lower = topic.to_lowercase();
        if let Some((score, _conf)) = ace_ctx.topic_affinities.get(&topic_lower) {
            if *score > 0.3 {
                reasons.push(format!("You've shown interest in {}", topic));
                break;
            }
        }
    }

    // Deduplicate and format output
    if reasons.is_empty() {
        "Matches your overall profile".to_string()
    } else if reasons.len() == 1 {
        reasons[0].clone()
    } else {
        // Return first two reasons joined by semicolon
        format!("{}; {}", reasons[0], reasons[1])
    }
}

// ============================================================================
// ACE Context Integration
// ============================================================================

/// ACE-discovered context for relevance scoring
/// PASIFA: Full context including confidence scores for weighted scoring
#[derive(Debug, Default)]
struct ACEContext {
    /// Active topics detected from project manifests and git history
    active_topics: Vec<String>,
    /// Confidence scores for active topics (topic -> confidence 0.0-1.0)
    topic_confidence: std::collections::HashMap<String, f32>,
    /// Detected tech stack (languages, frameworks)
    detected_tech: Vec<String>,
    /// Anti-topics (topics user has consistently rejected)
    anti_topics: Vec<String>,
    /// Confidence scores for anti-topics (topic -> confidence 0.0-1.0)
    anti_topic_confidence: std::collections::HashMap<String, f32>,
    /// Topic affinities from behavior learning (topic -> (affinity_score, confidence))
    /// PASIFA: Now includes BOTH positive AND negative affinities with confidence
    topic_affinities: std::collections::HashMap<String, (f32, f32)>,
}

/// Fetch ACE-discovered context for relevance scoring
/// PASIFA: Now captures full context including confidence scores
fn get_ace_context() -> ACEContext {
    let ace = match get_ace_engine() {
        Ok(engine) => engine,
        Err(e) => {
            warn!(target: "4da::ace", error = %e, "ACE engine unavailable - using empty context");
            return ACEContext::default();
        }
    };

    let mut ctx = ACEContext::default();

    // Get active topics WITH confidence scores
    if let Ok(topics) = ace.get_active_topics() {
        for t in topics.iter().filter(|t| t.weight >= 0.3) {
            let topic_lower = t.topic.to_lowercase();
            ctx.active_topics.push(topic_lower.clone());
            ctx.topic_confidence.insert(topic_lower, t.confidence);
        }
    }

    // Get detected tech
    if let Ok(tech) = ace.get_detected_tech() {
        ctx.detected_tech = tech.iter().map(|t| t.name.to_lowercase()).collect();
    }

    // Get anti-topics WITH confidence scores
    if let Ok(anti_topics) = ace.get_anti_topics(3) {
        for a in anti_topics
            .iter()
            .filter(|a| a.user_confirmed || a.confidence >= 0.5)
        {
            let topic_lower = a.topic.to_lowercase();
            ctx.anti_topics.push(topic_lower.clone());
            ctx.anti_topic_confidence.insert(topic_lower, a.confidence);
        }
    }

    // Get topic affinities - BOTH positive AND negative
    // PASIFA: Negative affinities are valuable learned signals with confidence
    if let Ok(affinities) = ace.get_topic_affinities() {
        for aff in affinities {
            // Include affinities with enough data, regardless of sign
            if aff.total_exposures >= 3 && aff.affinity_score.abs() > 0.1 {
                ctx.topic_affinities.insert(
                    aff.topic.to_lowercase(),
                    (aff.affinity_score, aff.confidence),
                );
            }
        }
    }

    ctx
}

/// Check if item should be excluded by ACE anti-topics
fn check_ace_exclusions(topics: &[String], ace_ctx: &ACEContext) -> Option<String> {
    for topic in topics {
        let topic_lower = topic.to_lowercase();
        for anti_topic in &ace_ctx.anti_topics {
            if topic_lower.contains(anti_topic) || anti_topic.contains(&topic_lower) {
                return Some(format!("ACE anti-topic: {}", anti_topic));
            }
        }
    }
    None
}

/// Compute semantic ACE boost using embeddings
/// PASIFA: Uses vector similarity instead of keyword matching when embeddings available
fn compute_semantic_ace_boost(
    item_embedding: &[f32],
    ace_ctx: &ACEContext,
    topic_embeddings: &std::collections::HashMap<String, Vec<f32>>,
) -> Option<f32> {
    if topic_embeddings.is_empty() {
        return None; // Fall back to keyword matching
    }

    let mut max_similarity: f32 = 0.0;
    let mut weighted_sum: f32 = 0.0;
    let mut weight_total: f32 = 0.0;

    // Compute similarity with active topics
    for topic in &ace_ctx.active_topics {
        if let Some(topic_emb) = topic_embeddings.get(topic) {
            let sim = cosine_similarity(item_embedding, topic_emb);
            let conf = ace_ctx.topic_confidence.get(topic).copied().unwrap_or(0.5);
            weighted_sum += sim * conf;
            weight_total += conf;
            max_similarity = max_similarity.max(sim);
        }
    }

    // Compute similarity with detected tech
    for tech in &ace_ctx.detected_tech {
        if let Some(tech_emb) = topic_embeddings.get(tech) {
            let sim = cosine_similarity(item_embedding, tech_emb);
            weighted_sum += sim * 0.8; // Tech is strong signal
            weight_total += 0.8;
            max_similarity = max_similarity.max(sim);
        }
    }

    if weight_total == 0.0 {
        return None;
    }

    // Compute weighted average similarity
    let avg_similarity = weighted_sum / weight_total;

    // Apply learned affinities as multiplier with confidence weighting
    let mut affinity_mult: f32 = 1.0;
    for (topic, &(affinity, confidence)) in &ace_ctx.topic_affinities {
        if let Some(topic_emb) = topic_embeddings.get(topic) {
            let sim = cosine_similarity(item_embedding, topic_emb);
            if sim > 0.5 {
                // Item is similar to a topic we have affinity data for
                // Scale by both similarity and confidence
                affinity_mult += affinity * confidence * 0.3 * sim;
            }
        }
    }
    affinity_mult = affinity_mult.clamp(0.5, 1.5);

    // Convert similarity (0-1) to boost (-0.3 to 0.5) range
    // High similarity (>0.7) = positive boost
    // Low similarity (<0.3) = negative boost
    let base_boost = (avg_similarity - 0.5) * 1.0; // Center around 0.5

    Some((base_boost * affinity_mult).clamp(-0.3, 0.5))
}

/// Embed ACE topics for semantic matching
/// Uses database-persisted embeddings with in-memory cache fallback
/// Returns topic -> embedding map
fn get_topic_embeddings(ace_ctx: &ACEContext) -> std::collections::HashMap<String, Vec<f32>> {
    // Lazy static cache for topic embeddings
    use std::sync::Mutex;
    static TOPIC_EMBEDDING_CACHE: OnceCell<Mutex<std::collections::HashMap<String, Vec<f32>>>> =
        OnceCell::new();
    static DB_LOADED: OnceCell<Mutex<bool>> = OnceCell::new();

    let cache = TOPIC_EMBEDDING_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let db_loaded = DB_LOADED.get_or_init(|| Mutex::new(false));

    let Ok(mut cache_guard) = cache.lock() else {
        warn!(target: "4da::embeddings", "Topic cache lock poisoned, returning empty");
        return std::collections::HashMap::new();
    };
    let Ok(mut db_loaded_guard) = db_loaded.lock() else {
        warn!(target: "4da::embeddings", "DB loaded lock poisoned, returning empty");
        return std::collections::HashMap::new();
    };

    // First time: load persisted embeddings from database
    if !*db_loaded_guard {
        if let Ok(ace) = get_ace_engine() {
            if let Ok(db_embeddings) = ace::load_topic_embeddings(ace.get_conn()) {
                for (topic, embedding) in db_embeddings {
                    cache_guard.insert(topic, embedding);
                }
                debug!(
                    target: "4da::embeddings",
                    count = cache_guard.len(),
                    "Loaded topic embeddings from database"
                );
            }
        }
        *db_loaded_guard = true;
    }

    // Collect topics that need embedding
    let mut topics_to_embed: Vec<String> = Vec::new();

    for topic in &ace_ctx.active_topics {
        if !cache_guard.contains_key(topic) {
            topics_to_embed.push(topic.clone());
        }
    }

    for tech in &ace_ctx.detected_tech {
        if !cache_guard.contains_key(tech) {
            topics_to_embed.push(tech.clone());
        }
    }

    for topic in ace_ctx.topic_affinities.keys() {
        if !cache_guard.contains_key(topic) {
            topics_to_embed.push(topic.clone());
        }
    }

    // Generate embeddings for missing topics and persist to database
    if !topics_to_embed.is_empty() {
        // Limit batch size to avoid overwhelming
        let batch: Vec<String> = topics_to_embed.into_iter().take(50).collect();
        let batch_len = batch.len();

        if let Ok(embeddings) = embed_texts(&batch) {
            // Get ACE connection for persistence
            let ace_conn = get_ace_engine().ok().map(|ace| ace.get_conn().clone());

            for (topic, embedding) in batch.into_iter().zip(embeddings.into_iter()) {
                // Persist to database if possible
                if let Some(ref conn) = ace_conn {
                    let _ = ace::store_topic_embedding(conn, &topic, &embedding);
                }
                cache_guard.insert(topic, embedding);
            }

            debug!(
                target: "4da::embeddings",
                generated = batch_len,
                "Generated and persisted new topic embeddings"
            );
        }
    }

    // Return a copy of relevant embeddings
    let mut result = std::collections::HashMap::new();
    for topic in &ace_ctx.active_topics {
        if let Some(emb) = cache_guard.get(topic) {
            result.insert(topic.clone(), emb.clone());
        }
    }
    for tech in &ace_ctx.detected_tech {
        if let Some(emb) = cache_guard.get(tech) {
            result.insert(tech.clone(), emb.clone());
        }
    }
    for topic in ace_ctx.topic_affinities.keys() {
        if let Some(emb) = cache_guard.get(topic) {
            result.insert(topic.clone(), emb.clone());
        }
    }

    result
}

/// Compute affinity multiplier from learned topic preferences
/// PASIFA: Applies learned affinities as multiplicative factors with confidence scaling
fn compute_affinity_multiplier(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    if ace_ctx.topic_affinities.is_empty() {
        return 1.0; // No learned preferences, neutral
    }

    let mut effect_sum: f32 = 0.0;
    let mut match_count: usize = 0;

    for topic in topics {
        let topic_lower = topic.to_lowercase();

        // Check direct match
        if let Some(&(affinity, confidence)) = ace_ctx.topic_affinities.get(&topic_lower) {
            // Effect = affinity * confidence (confidence scales the effect directly)
            effect_sum += affinity * confidence;
            match_count += 1;
            continue;
        }

        // Check partial matches
        for (aff_topic, &(affinity, confidence)) in &ace_ctx.topic_affinities {
            if topic_lower.contains(aff_topic) || aff_topic.contains(&topic_lower) {
                // Partial match: reduce effect by 0.7
                effect_sum += affinity * confidence * 0.7;
                match_count += 1;
                break;
            }
        }
    }

    if match_count == 0 {
        return 1.0; // No matches, neutral
    }

    // Average effect across matched topics, then convert to multiplier [0.3, 1.7]
    // This ensures confidence directly scales the effect:
    // High confidence (1.0) + high affinity (0.8) -> effect = 0.8 -> mult = 1.56
    // Low confidence (0.3) + high affinity (0.8) -> effect = 0.24 -> mult = 1.17
    let avg_effect = effect_sum / match_count as f32;
    (1.0 + avg_effect * 0.7).clamp(0.3, 1.7)
}

/// Compute anti-topic penalty as a multiplicative factor
/// PASIFA: Items matching anti-topics get reduced score based on confidence
fn compute_anti_penalty(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    if ace_ctx.anti_topics.is_empty() {
        return 0.0; // No anti-topics, no penalty
    }

    let mut total_penalty: f32 = 0.0;

    for topic in topics {
        let topic_lower = topic.to_lowercase();

        for anti_topic in &ace_ctx.anti_topics {
            if topic_lower.contains(anti_topic) || anti_topic.contains(&topic_lower) {
                // Get confidence for this anti-topic (default 0.5)
                let confidence = ace_ctx
                    .anti_topic_confidence
                    .get(anti_topic)
                    .copied()
                    .unwrap_or(0.5);

                // Scale penalty by confidence: higher confidence = stronger penalty
                // Max penalty per match is 0.3
                total_penalty += 0.3 * confidence;
                break; // Only one penalty per topic
            }
        }
    }

    // Cap total penalty at 0.7 (never fully zero out)
    total_penalty.min(0.7)
}

/// Unified relevance scoring using multiplicative formula
/// PASIFA: semantic_sim * affinity_multiplier * (1.0 - anti_penalty)
fn compute_unified_relevance(base_score: f32, topics: &[String], ace_ctx: &ACEContext) -> f32 {
    let affinity_mult = compute_affinity_multiplier(topics, ace_ctx);
    let anti_penalty = compute_anti_penalty(topics, ace_ctx);

    // Apply multiplicative formula
    let unified_score = base_score * affinity_mult * (1.0 - anti_penalty);

    // Clamp to valid range
    unified_score.clamp(0.0, 1.0)
}

/// Temporal freshness multiplier for PASIFA scoring.
/// Recent items get a slight boost, older items decay gently.
/// Returns a multiplier in [0.85, 1.15] range:
///   - Items < 2 hours old: 1.15x boost (very fresh)
///   - Items 2-6 hours old: 1.10x boost
///   - Items 6-12 hours old: 1.05x boost
///   - Items 12-24 hours old: 1.0x (neutral)
///   - Items 24-36 hours old: 0.95x decay
///   - Items 36-48 hours old: 0.90x decay
///   - Items > 48 hours old: 0.85x floor
fn compute_temporal_freshness(created_at: &chrono::DateTime<chrono::Utc>) -> f32 {
    let age_hours = (chrono::Utc::now() - *created_at).num_minutes() as f32 / 60.0;

    if age_hours < 2.0 {
        1.15
    } else if age_hours < 6.0 {
        1.10
    } else if age_hours < 12.0 {
        1.05
    } else if age_hours < 24.0 {
        1.0
    } else if age_hours < 36.0 {
        0.95
    } else if age_hours < 48.0 {
        0.90
    } else {
        0.85
    }
}

/// Calculate confidence score based on available signals
/// Returns a value between 0.0 and 1.0 indicating how confident we are in the scoring
fn calculate_confidence(
    context_score: f32,
    interest_score: f32,
    _semantic_boost: f32,
    ace_ctx: &ACEContext,
    topics: &[String],
    cached_context_count: i64,
    interest_count: i64,
) -> f32 {
    let mut confidence_signals: Vec<f32> = Vec::new();

    // Context signal confidence (higher score = more confident match)
    if cached_context_count > 0 {
        confidence_signals.push(context_score.clamp(0.0, 1.0));
    }

    // Interest signal confidence
    if interest_count > 0 {
        confidence_signals.push(interest_score.clamp(0.0, 1.0));
    }

    // ACE topic confidence (average of matched topic confidences)
    let mut topic_confidences: Vec<f32> = Vec::new();
    for topic in topics {
        let topic_lower = topic.to_lowercase();

        // Check active topic confidences
        if let Some(&conf) = ace_ctx.topic_confidence.get(&topic_lower) {
            topic_confidences.push(conf);
        }

        // Check affinity confidences
        if let Some(&(_affinity, conf)) = ace_ctx.topic_affinities.get(&topic_lower) {
            topic_confidences.push(conf);
        }
    }

    if !topic_confidences.is_empty() {
        let avg_topic_conf = topic_confidences.iter().sum::<f32>() / topic_confidences.len() as f32;
        confidence_signals.push(avg_topic_conf);
    }

    // If we have multiple signals, they reinforce each other
    if confidence_signals.is_empty() {
        return 0.4; // Low confidence - no strong signals
    }

    // Combine signals: average with bonus for multiple signals
    let avg_confidence = confidence_signals.iter().sum::<f32>() / confidence_signals.len() as f32;
    let signal_count_bonus = (confidence_signals.len() as f32 * 0.1).min(0.2);

    (avg_confidence + signal_count_bonus).clamp(0.0, 1.0)
}

// ============================================================================
// Global Database (Lazy Initialized)
// ============================================================================

static DATABASE: OnceCell<Arc<Database>> = OnceCell::new();

fn get_database() -> Result<&'static Arc<Database>, String> {
    DATABASE.get_or_try_init(|| {
        let mut db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        db_path.pop();
        db_path.push("data");
        db_path.push("4da.db");

        info!(target: "4da::db", path = ?db_path, "Initializing database");

        let db =
            Database::new(&db_path).map_err(|e| format!("Failed to initialize database: {}", e))?;

        // Register default sources
        db.register_source("hackernews", "Hacker News").ok();
        db.register_source("arxiv", "arXiv").ok();
        db.register_source("reddit", "Reddit").ok();

        info!(target: "4da::db", "Database ready");
        Ok(Arc::new(db))
    })
}

// ============================================================================
// Global Context Engine (Lazy Initialized)
// ============================================================================

static CONTEXT_ENGINE: OnceCell<Arc<ContextEngine>> = OnceCell::new();

fn get_context_engine() -> Result<&'static Arc<ContextEngine>, String> {
    CONTEXT_ENGINE.get_or_try_init(|| {
        // Context engine shares the database connection
        let mut db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        db_path.pop();
        db_path.push("data");
        db_path.push("4da.db");

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database for context engine: {}", e))?;

        let engine = ContextEngine::new(Arc::new(parking_lot::Mutex::new(conn)))
            .map_err(|e| format!("Failed to initialize context engine: {}", e))?;

        info!(target: "4da::context", "Context engine ready");
        Ok(Arc::new(engine))
    })
}

// ============================================================================
// Global ACE Instance (Lazy Initialized with RwLock for mutable access)
// ============================================================================

static ACE_ENGINE: OnceCell<Arc<parking_lot::RwLock<ace::ACE>>> = OnceCell::new();

fn init_ace_engine() -> Result<Arc<parking_lot::RwLock<ace::ACE>>, String> {
    // ACE shares the database connection
    let mut db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    db_path.pop();
    db_path.push("data");
    db_path.push("4da.db");

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    // Ensure sqlite-vec is registered for KNN search on topic embeddings
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database for ACE: {}", e))?;

    let engine = ace::ACE::new(Arc::new(parking_lot::Mutex::new(conn)))
        .map_err(|e| format!("Failed to initialize ACE: {}", e))?;

    info!(target: "4da::ace", "Autonomous Context Engine ready");
    Ok(Arc::new(parking_lot::RwLock::new(engine)))
}

fn get_ace_engine() -> Result<parking_lot::RwLockReadGuard<'static, ace::ACE>, String> {
    let engine = ACE_ENGINE.get_or_try_init(init_ace_engine)?;
    Ok(engine.read())
}

fn get_ace_engine_mut() -> Result<parking_lot::RwLockWriteGuard<'static, ace::ACE>, String> {
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

/// Fetch items from all sources (HN, arXiv, Reddit) directly
async fn fetch_all_sources(
    db: &Database,
    app: &AppHandle,
    max_items_per_source: usize,
) -> Result<Vec<(GenericSourceItem, Vec<f32>)>, String> {
    use sources::Source;

    // Create sources directly (avoid holding MutexGuard across await)
    let rss_feeds = load_rss_feeds_from_settings();
    let (twitter_handles, x_api_key) = load_twitter_settings();
    let youtube_channels = load_youtube_channels_from_settings();
    let sources: Vec<Box<dyn Source>> = vec![
        Box::new(HackerNewsSource::new()),
        Box::new(ArxivSource::new()),
        Box::new(RedditSource::new()),
        Box::new(GitHubSource::with_languages(
            load_github_languages_from_settings(),
        )),
        Box::new(RssSource::with_feeds(rss_feeds)),
        Box::new(TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key)),
        Box::new(YouTubeSource::with_channels(youtube_channels)),
    ];

    info!(target: "4da::sources", count = sources.len(), "Fetching from sources");

    let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
    let mut new_items_to_embed: Vec<(GenericSourceItem, String)> = Vec::new();

    for source in &sources {
        let source_type = source.source_type();
        let source_name = source.name();

        debug!(target: "4da::sources", source = source_name, "Fetching from source");
        emit_progress(
            app,
            "fetch",
            0.2,
            &format!("Fetching from {}...", source_name),
            all_items.len(),
            max_items_per_source * 3,
        );

        // Fetch items from this source with retry
        let fetch_result = {
            let mut attempts = 0;
            let max_attempts = 2;
            loop {
                attempts += 1;
                match source.fetch_items().await {
                    Ok(items) => break Ok(items),
                    Err(e) if attempts < max_attempts => {
                        warn!(target: "4da::sources", source = source_name, attempt = attempts, error = ?e, "Fetch failed, retrying...");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                    Err(e) => break Err(e),
                }
            }
        };

        match fetch_result {
            Ok(items) => {
                info!(target: "4da::sources", source = source_name, count = items.len(), "Fetched items from source");

                for (idx, item) in items.into_iter().take(max_items_per_source).enumerate() {
                    // Generate a numeric ID from source_id hash
                    let id = {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut hasher = DefaultHasher::new();
                        format!("{}:{}", source_type, item.source_id).hash(&mut hasher);
                        hasher.finish()
                    };

                    // Check cache first
                    if let Ok(Some(cached)) = db.get_source_item(source_type, &item.source_id) {
                        db.touch_source_item(source_type, &item.source_id).ok();
                        all_items.push((
                            GenericSourceItem {
                                id,
                                source_id: item.source_id,
                                source_type: source_type.to_string(),
                                title: cached.title,
                                url: cached.url,
                                content: cached.content,
                            },
                            cached.embedding,
                        ));
                    } else {
                        // Need to scrape and embed
                        let content = if item.content.is_empty() {
                            if let Some(ref _url) = item.url {
                                emit_progress(
                                    app,
                                    "scrape",
                                    0.3,
                                    &format!("Scraping: {}", &truncate_utf8(&item.title, 30)),
                                    idx,
                                    max_items_per_source,
                                );
                                source.scrape_content(&item).await.unwrap_or_default()
                            } else {
                                String::new()
                            }
                        } else {
                            item.content.clone()
                        };

                        let generic = GenericSourceItem {
                            id,
                            source_id: item.source_id.clone(),
                            source_type: source_type.to_string(),
                            title: item.title.clone(),
                            url: item.url.clone(),
                            content: content.clone(),
                        };

                        let embed_text = build_embedding_text(&item.title, &content);
                        new_items_to_embed.push((generic, embed_text));
                    }
                }
            }
            Err(e) => {
                error!(target: "4da::sources", source = source_name, error = ?e, "Source fetch failed after retries - continuing with other sources");
            }
        }
    }

    // Log summary of fetch results
    if all_items.is_empty() && new_items_to_embed.is_empty() {
        warn!(target: "4da::sources", "No items fetched from any source - check network connectivity");
    }

    // Embed new items with graceful degradation
    if !new_items_to_embed.is_empty() {
        debug!(target: "4da::embed", count = new_items_to_embed.len(), "Embedding new items");
        emit_progress(
            app,
            "embed",
            0.6,
            &format!("Embedding {} new items...", new_items_to_embed.len()),
            all_items.len(),
            all_items.len() + new_items_to_embed.len(),
        );

        let texts: Vec<String> = new_items_to_embed
            .iter()
            .map(|(_, text)| text.clone())
            .collect();

        // Try to embed, with fallback to zero vectors (keyword-only scoring)
        let embeddings = match embed_texts(&texts) {
            Ok(emb) => emb,
            Err(e) => {
                warn!(target: "4da::embed", error = %e, count = texts.len(),
                    "Embedding service unavailable - using fallback (keyword-only scoring)");
                // Create zero vectors as fallback - items will score via keyword matching only
                vec![vec![0.0f32; 384]; texts.len()]
            }
        };

        for ((item, _), embedding) in new_items_to_embed.into_iter().zip(embeddings.into_iter()) {
            // Cache in database (skip if embedding failed - zero vector)
            let is_fallback = embedding.iter().all(|&v| v == 0.0);
            if !is_fallback {
                db.upsert_source_item(
                    &item.source_type,
                    &item.source_id,
                    item.url.as_deref(),
                    &item.title,
                    &item.content,
                    &embedding,
                )
                .ok();
            }

            all_items.push((item, embedding));
        }
    }

    info!(target: "4da::sources", total = all_items.len(), "Total items from all sources");
    Ok(all_items)
}

/// Deep fetch from all sources - used for comprehensive initial scans
/// Fetches 5-10x more items from multiple endpoints per source
async fn fetch_all_sources_deep(
    db: &Database,
    app: &AppHandle,
    items_per_category: usize,
) -> Result<Vec<(GenericSourceItem, Vec<f32>)>, String> {
    use sources::Source;

    info!(target: "4da::sources", items_per_category, "DEEP SCAN: Fetching from all sources comprehensively");

    // Create sources directly (avoid holding MutexGuard across await)
    let rss_feeds = load_rss_feeds_from_settings();
    let (twitter_handles, x_api_key) = load_twitter_settings();
    let youtube_channels = load_youtube_channels_from_settings();

    // Note: HN, arXiv, and Reddit have fetch_items_deep implementations
    // GitHub, RSS, YouTube use default (regular fetch). Twitter has deep fetch.
    let hn_source = HackerNewsSource::new();
    let arxiv_source = ArxivSource::new();
    let reddit_source = RedditSource::new();
    let github_source = GitHubSource::with_languages(load_github_languages_from_settings());
    let rss_source = RssSource::with_feeds(rss_feeds);
    let twitter_source = TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key);
    let youtube_source = YouTubeSource::with_channels(youtube_channels);

    let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
    let mut new_items_to_embed: Vec<(GenericSourceItem, String)> = Vec::new();

    // Fetch from each source using deep fetch where available
    // HN deep fetch (top + new + best + ask + show = ~200+ unique items)
    emit_progress(
        app,
        "fetch",
        0.12,
        "Deep fetching Hacker News (5 categories)...",
        0,
        0,
    );
    match hn_source.fetch_items_deep(items_per_category).await {
        Ok(items) => {
            info!(target: "4da::sources", source = "hackernews", count = items.len(), "Deep fetched HN items");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "hackernews",
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "hackernews", error = ?e, "Deep fetch failed");
        }
    }

    // arXiv deep fetch (16 categories = ~100+ papers)
    emit_progress(
        app,
        "fetch",
        0.25,
        "Deep fetching arXiv (16 categories)...",
        all_items.len(),
        0,
    );
    match arxiv_source.fetch_items_deep(items_per_category).await {
        Ok(items) => {
            info!(target: "4da::sources", source = "arxiv", count = items.len(), "Deep fetched arXiv papers");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "arxiv");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "arxiv", error = ?e, "Deep fetch failed");
        }
    }

    // Reddit deep fetch (40+ subreddits = ~200+ posts)
    emit_progress(
        app,
        "fetch",
        0.35,
        "Deep fetching Reddit (40+ subreddits)...",
        all_items.len(),
        0,
    );
    match reddit_source.fetch_items_deep(items_per_category).await {
        Ok(items) => {
            info!(target: "4da::sources", source = "reddit", count = items.len(), "Deep fetched Reddit posts");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "reddit");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "reddit", error = ?e, "Deep fetch failed");
        }
    }

    // GitHub (regular fetch - trending is already comprehensive)
    emit_progress(
        app,
        "fetch",
        0.45,
        "Fetching GitHub trending...",
        all_items.len(),
        0,
    );
    match github_source.fetch_items().await {
        Ok(items) => {
            info!(target: "4da::sources", source = "github", count = items.len(), "Fetched GitHub repos");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "github");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "github", error = ?e, "Fetch failed");
        }
    }

    // RSS (regular fetch)
    emit_progress(
        app,
        "fetch",
        0.45,
        "Fetching RSS feeds...",
        all_items.len(),
        0,
    );
    match rss_source.fetch_items().await {
        Ok(items) => {
            info!(target: "4da::sources", source = "rss", count = items.len(), "Fetched RSS items");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "rss");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "rss", error = ?e, "Fetch failed");
        }
    }

    // Twitter/X deep fetch (timeline + search)
    emit_progress(
        app,
        "fetch",
        0.55,
        "Fetching Twitter/X...",
        all_items.len(),
        0,
    );
    match twitter_source.fetch_items_deep(items_per_category).await {
        Ok(items) => {
            info!(target: "4da::sources", source = "twitter", count = items.len(), "Deep fetched Twitter items");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "twitter",
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "twitter", error = ?e, "Fetch failed");
        }
    }

    // YouTube (regular fetch - RSS feeds)
    emit_progress(
        app,
        "fetch",
        0.60,
        "Fetching YouTube feeds...",
        all_items.len(),
        0,
    );
    match youtube_source.fetch_items().await {
        Ok(items) => {
            info!(target: "4da::sources", source = "youtube", count = items.len(), "Fetched YouTube videos");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "youtube",
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "youtube", error = ?e, "Fetch failed");
        }
    }

    info!(target: "4da::sources",
        total_cached = all_items.len(),
        new_to_embed = new_items_to_embed.len(),
        "Deep fetch complete, now embedding new items"
    );

    // Embed new items in batches for better progress feedback
    if !new_items_to_embed.is_empty() {
        let total_to_embed = new_items_to_embed.len();
        let batch_size = 20; // Smaller batches for better progress feedback

        for (batch_idx, chunk) in new_items_to_embed.chunks(batch_size).enumerate() {
            let start_idx = batch_idx * batch_size;
            let progress = 0.55 + (0.35 * (start_idx as f32 / total_to_embed as f32));

            emit_progress(
                app,
                "embed",
                progress,
                &format!(
                    "Embedding batch {}/{} ({} items)...",
                    batch_idx + 1,
                    (total_to_embed + batch_size - 1) / batch_size,
                    chunk.len()
                ),
                all_items.len() + start_idx,
                all_items.len() + total_to_embed,
            );

            let texts: Vec<String> = chunk.iter().map(|(_, text)| text.clone()).collect();

            let embeddings = match embed_texts(&texts) {
                Ok(emb) => emb,
                Err(e) => {
                    warn!(target: "4da::embed", error = %e, batch = batch_idx, "Embedding batch failed - using fallback");
                    vec![vec![0.0f32; 384]; texts.len()]
                }
            };

            for ((item, _), embedding) in chunk.iter().cloned().zip(embeddings.into_iter()) {
                let is_fallback = embedding.iter().all(|&v| v == 0.0);
                if !is_fallback {
                    db.upsert_source_item(
                        &item.source_type,
                        &item.source_id,
                        item.url.as_deref(),
                        &item.title,
                        &item.content,
                        &embedding,
                    )
                    .ok();
                }
                all_items.push((item, embedding));
            }
        }
    }

    info!(target: "4da::sources", total = all_items.len(), "DEEP SCAN: Total items from all sources");
    Ok(all_items)
}

/// Fill the cache with items from all sources (background operation)
/// This is the "write" side of the cache-first architecture
/// Does NOT emit progress events - runs silently in background
async fn fill_cache_background(app: &AppHandle) -> Result<usize, String> {
    use sources::Source;

    info!(target: "4da::cache", "=== BACKGROUND CACHE FILL STARTED ===");
    void_signal_fetching(app);

    let db = get_database()?;
    let rss_feeds = load_rss_feeds_from_settings();
    let (twitter_handles, x_api_key) = load_twitter_settings();
    let youtube_channels = load_youtube_channels_from_settings();

    // Use deep fetch for comprehensive coverage
    let hn_source = HackerNewsSource::new();
    let arxiv_source = ArxivSource::new();
    let reddit_source = RedditSource::new();
    let github_source = GitHubSource::with_languages(load_github_languages_from_settings());
    let rss_source = RssSource::with_feeds(rss_feeds);
    let twitter_source = TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key);
    let youtube_source = YouTubeSource::with_channels(youtube_channels);

    let mut total_cached = 0;
    let mut new_items_to_embed: Vec<(String, String, Option<String>, String, String)> = Vec::new();

    // HN deep fetch
    match hn_source.fetch_items_deep(50).await {
        Ok(items) => {
            info!(target: "4da::cache", source = "hackernews", count = items.len(), "Fetched HN items");
            for item in items {
                if db
                    .get_source_item("hackernews", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "hackernews".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("hackernews", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "hackernews", error = ?e, "Fetch failed"),
    }

    // arXiv deep fetch
    match arxiv_source.fetch_items_deep(50).await {
        Ok(items) => {
            info!(target: "4da::cache", source = "arxiv", count = items.len(), "Fetched arXiv items");
            for item in items {
                if db
                    .get_source_item("arxiv", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "arxiv".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("arxiv", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "arxiv", error = ?e, "Fetch failed"),
    }

    // Reddit deep fetch
    match reddit_source.fetch_items_deep(50).await {
        Ok(items) => {
            info!(target: "4da::cache", source = "reddit", count = items.len(), "Fetched Reddit items");
            for item in items {
                if db
                    .get_source_item("reddit", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "reddit".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("reddit", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "reddit", error = ?e, "Fetch failed"),
    }

    // GitHub fetch
    match github_source.fetch_items().await {
        Ok(items) => {
            info!(target: "4da::cache", source = "github", count = items.len(), "Fetched GitHub items");
            for item in items {
                if db
                    .get_source_item("github", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "github".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("github", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "github", error = ?e, "Fetch failed"),
    }

    // RSS fetch
    match rss_source.fetch_items().await {
        Ok(items) => {
            info!(target: "4da::cache", source = "rss", count = items.len(), "Fetched RSS items");
            for item in items {
                if db
                    .get_source_item("rss", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "rss".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("rss", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "rss", error = ?e, "Fetch failed"),
    }

    // Twitter/X deep fetch
    match twitter_source.fetch_items_deep(50).await {
        Ok(items) => {
            info!(target: "4da::cache", source = "twitter", count = items.len(), "Fetched Twitter items");
            for item in items {
                if db
                    .get_source_item("twitter", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "twitter".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("twitter", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "twitter", error = ?e, "Fetch failed"),
    }

    // YouTube fetch
    match youtube_source.fetch_items().await {
        Ok(items) => {
            info!(target: "4da::cache", source = "youtube", count = items.len(), "Fetched YouTube items");
            for item in items {
                if db
                    .get_source_item("youtube", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "youtube".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("youtube", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "youtube", error = ?e, "Fetch failed"),
    }

    // Embed and cache new items
    if !new_items_to_embed.is_empty() {
        info!(target: "4da::cache", new_items = new_items_to_embed.len(), "Embedding new items");

        let texts: Vec<String> = new_items_to_embed
            .iter()
            .map(|(_, _, _, title, content)| build_embedding_text(title, content))
            .collect();

        match embed_texts(&texts) {
            Ok(embeddings) => {
                for ((source_type, source_id, url, title, content), embedding) in
                    new_items_to_embed.into_iter().zip(embeddings.into_iter())
                {
                    if !embedding.iter().all(|&v| v == 0.0) {
                        db.upsert_source_item(
                            &source_type,
                            &source_id,
                            url.as_deref(),
                            &title,
                            &content,
                            &embedding,
                        )
                        .ok();
                        total_cached += 1;
                    }
                }
            }
            Err(e) => {
                warn!(target: "4da::cache", error = %e, "Embedding failed - items not cached");
            }
        }
    }

    // Emit cache update event
    let _ = app.emit("cache-updated", total_cached);
    void_signal_cache_filled(app);

    info!(target: "4da::cache", total = total_cached, "=== BACKGROUND CACHE FILL COMPLETE ===");
    Ok(total_cached)
}

/// Helper to process source items into cache/embed lists
fn process_source_items(
    db: &Database,
    all_items: &mut Vec<(GenericSourceItem, Vec<f32>)>,
    new_items_to_embed: &mut Vec<(GenericSourceItem, String)>,
    items: Vec<sources::SourceItem>,
    source_type: &str,
) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    for item in items {
        let id = {
            let mut hasher = DefaultHasher::new();
            format!("{}:{}", source_type, item.source_id).hash(&mut hasher);
            hasher.finish()
        };

        if let Ok(Some(cached)) = db.get_source_item(source_type, &item.source_id) {
            db.touch_source_item(source_type, &item.source_id).ok();
            all_items.push((
                GenericSourceItem {
                    id,
                    source_id: item.source_id,
                    source_type: source_type.to_string(),
                    title: cached.title,
                    url: cached.url,
                    content: cached.content,
                },
                cached.embedding,
            ));
        } else {
            let generic = GenericSourceItem {
                id,
                source_id: item.source_id.clone(),
                source_type: source_type.to_string(),
                title: item.title.clone(),
                url: item.url.clone(),
                content: item.content.clone(),
            };

            let embed_text = build_embedding_text(&item.title, &item.content);
            new_items_to_embed.push((generic, embed_text));
        }
    }
}

// ============================================================================
// Global Settings Manager
// ============================================================================

static SETTINGS_MANAGER: OnceCell<Mutex<SettingsManager>> = OnceCell::new();

fn get_settings_manager() -> &'static Mutex<SettingsManager> {
    SETTINGS_MANAGER.get_or_init(|| {
        let mut data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        data_path.pop();
        data_path.push("data");

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

fn get_analysis_state() -> &'static Mutex<AnalysisState> {
    ANALYSIS_STATE.get_or_init(|| {
        Mutex::new(AnalysisState {
            running: false,
            completed: false,
            error: None,
            results: None,
        })
    })
}

// ============================================================================
// Global Monitoring State
// ============================================================================

static MONITORING_STATE: OnceCell<Arc<monitoring::MonitoringState>> = OnceCell::new();

fn get_monitoring_state() -> &'static Arc<monitoring::MonitoringState> {
    MONITORING_STATE.get_or_init(|| Arc::new(monitoring::MonitoringState::new()))
}

// ============================================================================
// Global Job Queue (Background Extraction Processing)
// ============================================================================

static JOB_QUEUE: OnceCell<Arc<parking_lot::RwLock<job_queue::JobQueue>>> = OnceCell::new();

fn init_job_queue() -> Result<Arc<parking_lot::RwLock<job_queue::JobQueue>>, String> {
    let mut db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    db_path.pop();
    db_path.push("data");
    db_path.push("4da.db");

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database for job queue: {}", e))?;

    let queue = job_queue::JobQueue::new(Arc::new(parking_lot::Mutex::new(conn)));
    info!(target: "4da::job_queue", "Job queue initialized");
    Ok(Arc::new(parking_lot::RwLock::new(queue)))
}

fn get_job_queue() -> Result<&'static Arc<parking_lot::RwLock<job_queue::JobQueue>>, String> {
    JOB_QUEUE.get_or_try_init(init_job_queue)
}

// ============================================================================
// Configuration
// ============================================================================

/// Get context directories from settings (no fallback - empty means no context)
fn get_context_dirs() -> Vec<PathBuf> {
    let settings = get_settings_manager().lock();
    let dirs = settings.get().context_dirs.clone();
    drop(settings);

    dirs.into_iter().map(PathBuf::from).collect()
}

/// Legacy function for single directory (uses first configured dir)
fn get_context_dir() -> Option<PathBuf> {
    get_context_dirs().into_iter().next()
}

/// Load RSS feed URLs from settings
fn load_rss_feeds_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let feeds = settings.get_rss_feeds();
    drop(settings);
    feeds
}

/// Load Twitter handles and X API key from settings
fn load_twitter_settings() -> (Vec<String>, String) {
    let settings = get_settings_manager().lock();
    let handles = settings.get_twitter_handles();
    let api_key = settings.get_x_api_key();
    drop(settings);
    (handles, api_key)
}

/// Load YouTube channel IDs from settings
fn load_youtube_channels_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let channels = settings.get_youtube_channels();
    drop(settings);
    channels
}

/// Load GitHub languages from settings (defaults if empty)
fn load_github_languages_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let langs = settings.get_github_languages();
    drop(settings);
    if langs.is_empty() {
        vec![
            "rust".to_string(),
            "typescript".to_string(),
            "python".to_string(),
        ]
    } else {
        langs
    }
}

/// File extensions we care about for Phase 0
const SUPPORTED_EXTENSIONS: &[&str] = &["md", "txt", "rs", "ts", "js", "py"];

/// Relevance threshold for "relevant" classification (lowered for content-based matching)
const RELEVANCE_THRESHOLD: f32 = 0.30;

/// Maximum content length for embedding (roughly 1000 words)
const MAX_CONTENT_LENGTH: usize = 5000;

/// Maximum chunk size in characters (roughly 100-150 words)
const MAX_CHUNK_SIZE: usize = 500;

// ============================================================================
// Text Processing
// ============================================================================

/// Split text into chunks for embedding
fn chunk_text(text: &str, source_file: &str) -> Vec<(String, String)> {
    let mut chunks = Vec::new();
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    let mut current_chunk = String::new();

    for para in paragraphs {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }

        if current_chunk.len() + para.len() > MAX_CHUNK_SIZE {
            if !current_chunk.is_empty() {
                chunks.push((source_file.to_string(), current_chunk.clone()));
                current_chunk.clear();
            }
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
async fn scrape_article_content(url: &str) -> Option<String> {
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
        .header("User-Agent", "Mozilla/5.0 (compatible; 4DA/0.1)")
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
fn build_embedding_text(title: &str, content: &str) -> String {
    if content.is_empty() {
        title.to_string()
    } else {
        // Combine title and content with clear separation
        format!("{}\n\n{}", title, content)
    }
}

/// Emit a progress event to the frontend
fn emit_progress(
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
fn void_signal_fetching(app: &AppHandle) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_fetching(db, monitoring);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: cache fill complete
fn void_signal_cache_filled(app: &AppHandle) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_cache_filled(db, monitoring);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: analysis complete with scores
fn void_signal_analysis_complete(app: &AppHandle, results: &[HNRelevance]) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let top_scores: Vec<f32> = results
            .iter()
            .filter(|r| r.relevant)
            .map(|r| r.top_score)
            .collect();
        let signal = void_engine::signal_after_analysis(db, monitoring, &top_scores);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: error occurred
fn void_signal_error(app: &AppHandle) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_error(db, monitoring);
        void_engine::emit_if_changed(app, signal);
    }
}

/// Emit void signal: ACE context change
#[allow(dead_code)]
fn void_signal_context_change(app: &AppHandle, intensity: f32) {
    if let Ok(db) = get_database() {
        let monitoring = get_monitoring_state();
        let signal = void_engine::signal_context_change(db, monitoring, intensity);
        void_engine::emit_if_changed(app, signal);
    }
}

// ============================================================================
// Void Engine Command
// ============================================================================

/// Get the current void signal state (for initial mount)
#[tauri::command]
fn get_void_signal() -> Result<void_engine::VoidSignal, String> {
    let db = get_database()?;
    let monitoring = get_monitoring_state();
    Ok(void_engine::compute_signal(db, monitoring))
}

/// Build the complete VoidUniverse for 3D rendering
#[tauri::command]
fn void_get_universe(max_particles: Option<usize>) -> Result<void_engine::VoidUniverse, String> {
    let db = get_database()?;
    let ctx = get_context_engine()?;
    let projection_version = 1i64; // Increment when embedding model changes
    void_engine::build_universe(db, ctx, max_particles, projection_version)
}

/// Get full detail for a single particle (on selection)
#[tauri::command]
fn void_get_particle_detail(id: i64, layer: String) -> Result<serde_json::Value, String> {
    let db = get_database()?;
    match layer.as_str() {
        "source" => {
            let item = db
                .get_source_item_by_id(id)
                .map_err(|e| format!("DB error: {e}"))?
                .ok_or_else(|| format!("Source item {id} not found"))?;
            serde_json::to_value(&serde_json::json!({
                "id": item.id,
                "layer": "source",
                "source_type": item.source_type,
                "title": item.title,
                "url": item.url,
                "content_preview": &item.content[..item.content.len().min(500)],
                "created_at": item.created_at.to_rfc3339(),
                "last_seen": item.last_seen.to_rfc3339(),
            }))
            .map_err(|e| format!("Serialization error: {e}"))
        }
        "context" => {
            // Context chunks are internal - return basic info
            Ok(serde_json::json!({
                "id": id,
                "layer": "context",
                "message": "Context chunk from local files",
            }))
        }
        _ => Err(format!("Unknown layer: {layer}")),
    }
}

/// Find K nearest neighbors for a particle in the universe
#[tauri::command]
fn void_get_neighbors(
    id: i64,
    layer: String,
    k: Option<usize>,
) -> Result<Vec<void_engine::VoidParticle>, String> {
    // Build universe first, then find neighbors within it
    let db = get_database()?;
    let ctx = get_context_engine()?;
    let universe = void_engine::build_universe(db, ctx, None, 1)?;
    let neighbors = void_engine::find_neighbors(id, &layer, &universe.particles, k.unwrap_or(10));
    Ok(neighbors)
}

// ============================================================================
// Commands
// ============================================================================

#[tauri::command]
async fn get_context_files() -> Result<Vec<ContextFile>, String> {
    let context_dir = match get_context_dir() {
        Some(dir) => dir,
        None => {
            debug!(target: "4da::context", "No context directory configured");
            return Ok(vec![]);
        }
    };
    debug!(target: "4da::context", path = ?context_dir, "Reading context files");

    if !context_dir.exists() {
        debug!(target: "4da::context", "Context directory does not exist");
        return Ok(vec![]);
    }

    let mut files = Vec::new();
    let entries = fs::read_dir(&context_dir).map_err(|e| e.to_string())?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !SUPPORTED_EXTENSIONS.contains(&ext) {
            continue;
        }

        match fs::read_to_string(&path) {
            Ok(content) => {
                let lines = content.lines().count();
                let path_str = path.to_string_lossy().to_string();
                debug!(target: "4da::context", path = %path_str, lines = lines, "Loaded context file");
                files.push(ContextFile {
                    path: path_str,
                    content,
                    lines,
                });
            }
            Err(e) => {
                warn!(target: "4da::context", path = ?path, error = %e, "Failed to read context file");
            }
        }
    }

    info!(target: "4da::context", count = files.len(), "Total context files loaded");
    Ok(files)
}

/// Clear all indexed context chunks from the database
#[tauri::command]
async fn clear_context() -> Result<String, String> {
    info!(target: "4da::context", "Clearing indexed context");

    // Use the singleton database connection (same one used by analysis)
    let db = get_database()?;

    let cleared = db
        .clear_contexts()
        .map_err(|e| format!("Failed to clear context: {}", e))?;

    info!(target: "4da::context", chunks_removed = cleared, "Context cleared successfully");
    Ok(format!(
        "Context cleared successfully ({} chunks removed)",
        cleared
    ))
}

/// Index context files - read, chunk, embed, and store in database
#[tauri::command]
async fn index_context() -> Result<String, String> {
    info!(target: "4da::context", "Indexing context files");

    let db = get_database()?;

    // First clear existing context to avoid duplicates
    let _ = db.clear_contexts();

    // Read context files from configured directories
    let context_files = get_context_files().await?;
    if context_files.is_empty() {
        return Err("No context files found. Add files to your context directory.".to_string());
    }

    // Chunk the files
    let mut all_chunks: Vec<(String, String)> = Vec::new();
    for file in &context_files {
        let filename = file
            .path
            .split('/')
            .last()
            .and_then(|s| s.split('\\').last())
            .unwrap_or(&file.path);
        let chunks = chunk_text(&file.content, filename);
        debug!(target: "4da::context", file = filename, chunks = chunks.len(), "Chunked file");
        all_chunks.extend(chunks);
    }

    if all_chunks.is_empty() {
        return Err("No content to index from context files.".to_string());
    }

    // Generate embeddings
    debug!(target: "4da::embed", chunks = all_chunks.len(), "Generating embeddings for chunks");
    let chunk_texts: Vec<String> = all_chunks.iter().map(|(_, text)| text.clone()).collect();
    let chunk_embeddings = embed_texts(&chunk_texts)?;

    // Store in database
    debug!(target: "4da::context", chunks = all_chunks.len(), "Storing context chunks in database");
    for ((source, text), embedding) in all_chunks.iter().zip(chunk_embeddings.iter()) {
        db.upsert_context(source, text, embedding)
            .map_err(|e| format!("Failed to store context: {}", e))?;
    }

    info!(target: "4da::context", files = context_files.len(), chunks = all_chunks.len(), "Context indexed successfully");
    Ok(format!(
        "Indexed {} files ({} chunks)",
        context_files.len(),
        all_chunks.len()
    ))
}

/// Index READMEs from all configured context directories
/// This scans all context_dirs and indexes README files for semantic search
#[tauri::command]
async fn index_project_readmes() -> Result<String, String> {
    info!(target: "4da::context", "Indexing READMEs from all configured directories");

    let context_dirs = get_context_dirs();
    if context_dirs.is_empty() {
        return Err("No context directories configured".to_string());
    }

    let indexed_count = index_discovered_readmes(&context_dirs).await;

    if indexed_count > 0 {
        info!(target: "4da::context", count = indexed_count, "README chunks indexed");
        Ok(format!(
            "Indexed {} README chunks from {} directories",
            indexed_count,
            context_dirs.len()
        ))
    } else {
        Ok("No README files found in configured directories".to_string())
    }
}

/// Get current context directory settings
#[tauri::command]
async fn get_context_settings() -> Result<ContextSettings, String> {
    let dirs = get_context_dirs();
    let settings = get_settings_manager().lock();
    let configured = settings.get().context_dirs.clone();
    drop(settings);

    let using_default = configured.is_empty();
    Ok(ContextSettings {
        configured_dirs: configured,
        active_dirs: dirs
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        using_default,
    })
}

/// Convert Windows path to WSL path if needed (e.g., D:\projects -> /mnt/d/projects)
fn convert_windows_to_wsl_path(path: &str) -> String {
    // Check if it looks like a Windows path (e.g., "D:\something" or "D:/something")
    if path.len() >= 2 && path.chars().nth(1) == Some(':') {
        let drive = path.chars().next().unwrap().to_lowercase().next().unwrap();
        let rest = &path[2..].replace('\\', "/");
        format!("/mnt/{}{}", drive, rest)
    } else {
        path.to_string()
    }
}

#[tauri::command]
async fn set_context_dirs(dirs: Vec<String>) -> Result<String, String> {
    info!(target: "4da::context", dirs = ?dirs, "Setting context directories");

    // Convert Windows paths to WSL paths and validate
    let mut converted_dirs: Vec<String> = Vec::new();
    for dir in &dirs {
        let converted = convert_windows_to_wsl_path(dir);
        if converted != *dir {
            debug!(target: "4da::context", from = dir, to = %converted, "Converted Windows path");
        }

        let path = PathBuf::from(&converted);
        if !path.exists() {
            return Err(format!(
                "Directory does not exist: {} (tried: {})",
                dir, converted
            ));
        }
        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", converted));
        }
        converted_dirs.push(converted);
    }

    let mut settings = get_settings_manager().lock();
    settings.get_mut().context_dirs = converted_dirs.clone();
    settings.save()?;
    drop(settings);

    info!(target: "4da::context", dirs = ?converted_dirs, "Context directories updated");
    Ok(format!(
        "Context directories updated: {} directories configured",
        converted_dirs.len()
    ))
}

#[tauri::command]
async fn get_hn_top_stories() -> Result<Vec<HNItem>, String> {
    info!(target: "4da::sources", "Fetching HN top stories");
    let client = reqwest::Client::new();

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

                    items.push(HNItem {
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
async fn compute_relevance() -> Result<Vec<HNRelevance>, String> {
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

    let client = reqwest::Client::new();
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
    let mut cached_items: Vec<(HNItem, Vec<f32>)> = Vec::new();
    let mut new_items: Vec<HNItem> = Vec::new();

    for id in top_ids.into_iter().take(30) {
        let id_str = id.to_string();

        // Check cache first
        if let Ok(Some(cached)) = db.get_source_item("hackernews", &id_str) {
            debug!(target: "4da::analysis", id = id, title = %&truncate_utf8(&cached.title, 40), "HN story (cached)");
            db.touch_source_item("hackernews", &id_str).ok();
            cached_items.push((
                HNItem {
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

                        new_items.push(HNItem {
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
        let embeddings = embed_texts(&new_texts)?;

        // Cache new items in database
        debug!(target: "4da::analysis", count = new_items.len(), "Caching new items in database");
        for (item, embedding) in new_items.iter().zip(embeddings.iter()) {
            db.upsert_source_item(
                "hackernews",
                &item.id.to_string(),
                item.url.as_deref(),
                &item.title,
                &item.content,
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
    let mut all_items_with_embeddings: Vec<(HNItem, Vec<f32>)> = cached_items;
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
    let ace_ctx = get_ace_context();
    // PASIFA: Pre-compute topic embeddings for semantic matching
    let topic_embeddings = get_topic_embeddings(&ace_ctx);
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
    let mut results: Vec<HNRelevance> = Vec::new();
    let mut excluded_count = 0;

    for (item, item_embedding) in &all_items_with_embeddings {
        // Extract topics from this item
        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions FIRST (hard filter)
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions)
            .or_else(|| check_ace_exclusions(&topics, &ace_ctx));

        if let Some(ref exclusion) = excluded_by {
            debug!(target: "4da::analysis", title = %&truncate_utf8(&item.title, 50), exclusion = %exclusion, "EXCLUDED");
            excluded_count += 1;

            // Still add to results but marked as excluded
            results.push(HNRelevance {
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
        let interest_score = compute_interest_score(item_embedding, &static_identity.interests);

        // Compute semantic ACE boost for topic/tech matching
        // PASIFA: Use semantic matching when embeddings available, fall back to keywords
        let semantic_boost = compute_semantic_ace_boost(
            item_embedding,
            &ace_ctx,
            &topic_embeddings,
        )
        .unwrap_or_else(|| {
            // Fall back to keyword matching for active topics and tech only (not affinities)
            let mut boost: f32 = 0.0;
            for topic in &topics {
                let topic_lower = topic.to_lowercase();
                // Active topics boost
                for active_topic in &ace_ctx.active_topics {
                    if topic_lower.contains(active_topic) || active_topic.contains(&topic_lower) {
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

        // Combined score: weighted average of context, interest scores, plus semantic boost
        // Dynamically adjust weights based on what data is available
        let base_score = if cached_context_count > 0 && interest_count > 0 {
            // Full mode: 50% context + 50% interests + ACE boost
            (context_score * 0.5 + interest_score * 0.5 + semantic_boost).min(1.0)
        } else if interest_count > 0 {
            // No context indexed: rely on interests + ACE boost (full weight)
            // This prevents penalizing users who haven't indexed context files
            (interest_score * 0.7 + semantic_boost * 1.5).min(1.0)
        } else if cached_context_count > 0 {
            // No interests: rely on context + ACE boost
            (context_score + semantic_boost).min(1.0)
        } else {
            // Neither context nor interests: pure ACE topic matching
            (semantic_boost * 2.0).min(1.0)
        };

        // PASIFA: Apply unified multiplicative scoring
        // This applies learned affinities and anti-topic penalties multiplicatively
        let combined_score = compute_unified_relevance(base_score, &topics, &ace_ctx);

        let relevant = combined_score >= RELEVANCE_THRESHOLD;

        // Compute debug info for logging
        let affinity_mult = compute_affinity_multiplier(&topics, &ace_ctx);
        let anti_penalty = compute_anti_penalty(&topics, &ace_ctx);

        // Log scoring details
        if relevant {
            info!(target: "4da::analysis",
                id = item.id,
                title = %item.title,
                combined = combined_score,
                base = base_score,
                context = context_score,
                interest = interest_score,
                semantic_boost = semantic_boost,
                affinity_mult = affinity_mult,
                anti_penalty = anti_penalty,
                "RELEVANT"
            );
        } else {
            debug!(target: "4da::analysis",
                id = item.id,
                title = %item.title,
                combined = combined_score,
                base = base_score,
                context = context_score,
                interest = interest_score,
                semantic_boost = semantic_boost,
                affinity_mult = affinity_mult,
                anti_penalty = anti_penalty,
                "not relevant"
            );
        }
        if !topics.is_empty() {
            debug!(target: "4da::analysis", id = item.id, topics = %topics.iter().take(5).cloned().collect::<Vec<_>>().join(", "), "Extracted topics");
        }

        // Generate explanation for relevant items
        let explanation = if relevant {
            Some(generate_relevance_explanation(
                &item.title,
                context_score,
                interest_score,
                &matches,
                &ace_ctx,
                &topics,
            ))
        } else {
            None
        };

        // Calculate confidence and score breakdown
        let confidence = calculate_confidence(
            context_score,
            interest_score,
            semantic_boost,
            &ace_ctx,
            &topics,
            cached_context_count,
            interest_count as i64,
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
            ace_boost: semantic_boost,
            affinity_mult,
            anti_penalty,
            freshness_mult: 1.0,
            confidence_by_signal,
        };

        results.push(HNRelevance {
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
        threshold = RELEVANCE_THRESHOLD,
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
    let _ace = get_ace_engine()?;

    // Log health check (simplified - full health monitoring removed)
    info!(
        target: "4da::health",
        status = "healthy",
        "Background health check complete"
    );

    Ok(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Run background anomaly detection - called every hour by scheduler
pub async fn run_background_anomaly_detection() -> Result<serde_json::Value, String> {
    let _ace = get_ace_engine()?;

    // Simplified - full anomaly detection removed
    debug!(target: "4da::anomaly", "Background anomaly check (no-op)");

    Ok(serde_json::json!({
        "anomalies_found": 0,
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

    Ok(serde_json::json!({
        "signals_decayed": decayed_count,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// ============================================================================
// Update Commands
// ============================================================================

/// Check for available updates
#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app
        .updater()
        .map_err(|e| format!("Updater not available: {}", e))?;

    match updater.check().await {
        Ok(Some(update)) => {
            info!(
                target: "4da::updater",
                current = %update.current_version,
                available = %update.version,
                "Update available"
            );
            Ok(serde_json::json!({
                "update_available": true,
                "current_version": update.current_version,
                "new_version": update.version,
                "body": update.body
            }))
        }
        Ok(None) => {
            debug!(target: "4da::updater", "No update available");
            Ok(serde_json::json!({
                "update_available": false,
                "current_version": env!("CARGO_PKG_VERSION")
            }))
        }
        Err(e) => {
            warn!(target: "4da::updater", error = %e, "Update check failed");
            Ok(serde_json::json!({
                "update_available": false,
                "error": e.to_string(),
                "current_version": env!("CARGO_PKG_VERSION")
            }))
        }
    }
}

/// Get current app version
#[tauri::command]
async fn get_current_version() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
        "description": env!("CARGO_PKG_DESCRIPTION")
    }))
}

// ============================================================================
// Digest Commands
// ============================================================================

/// Get digest configuration
#[tauri::command]
async fn get_digest_config() -> Result<serde_json::Value, String> {
    let settings_guard = get_settings_manager().lock();
    let digest = &settings_guard.get().digest;

    Ok(serde_json::json!({
        "enabled": digest.enabled,
        "frequency": digest.frequency,
        "email": digest.email,
        "save_local": digest.save_local,
        "min_score": digest.min_score,
        "max_items": digest.max_items,
        "last_sent": digest.last_sent,
        "generate_summaries": digest.generate_summaries
    }))
}

/// Update digest configuration
#[tauri::command]
async fn set_digest_config(
    enabled: Option<bool>,
    frequency: Option<String>,
    email: Option<String>,
    save_local: Option<bool>,
    min_score: Option<f64>,
    max_items: Option<usize>,
) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    let digest = &mut settings_guard.get_mut().digest;

    if let Some(e) = enabled {
        digest.enabled = e;
    }
    if let Some(f) = frequency {
        digest.frequency = f;
    }
    if let Some(e) = email {
        digest.email = Some(e);
    }
    if let Some(s) = save_local {
        digest.save_local = s;
    }
    if let Some(s) = min_score {
        digest.min_score = s;
    }
    if let Some(m) = max_items {
        digest.max_items = m;
    }

    settings_guard.save()?;

    let digest = &settings_guard.get().digest;
    info!(
        target: "4da::digest",
        enabled = digest.enabled,
        frequency = %digest.frequency,
        "Digest config updated"
    );

    Ok(serde_json::json!({
        "success": true,
        "config": {
            "enabled": digest.enabled,
            "frequency": digest.frequency,
            "email": digest.email,
            "save_local": digest.save_local,
            "min_score": digest.min_score,
            "max_items": digest.max_items
        }
    }))
}

/// Generate a digest from recent relevant items
#[tauri::command]
async fn generate_digest() -> Result<serde_json::Value, String> {
    use chrono::{Duration, Utc};

    // Get settings (clone to avoid holding lock during DB operations)
    let digest_config = {
        let settings_guard = get_settings_manager().lock();
        settings_guard.get().digest.clone()
    };

    let db = get_database()?;

    // Get digest period
    let period_end = Utc::now();
    let period_start = digest_config
        .last_sent
        .unwrap_or(period_end - Duration::hours(24));

    // Fetch recent relevant items from source_items table
    let items = db
        .get_relevant_items_since(
            period_start,
            digest_config.min_score,
            digest_config.max_items,
        )
        .map_err(|e| format!("Failed to fetch items: {}", e))?;

    if items.is_empty() {
        return Ok(serde_json::json!({
            "success": true,
            "digest": null,
            "message": "No relevant items found for this period"
        }));
    }

    // Convert to digest items
    let digest_items: Vec<digest::DigestItem> = items
        .into_iter()
        .map(|item| digest::DigestItem {
            id: item.id,
            title: item.title,
            url: item.url,
            source: item.source_type,
            relevance_score: item.relevance_score.unwrap_or(0.0),
            matched_topics: item.topics,
            discovered_at: item.created_at,
            summary: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
        })
        .collect();

    // Create digest
    let digest_obj = digest::Digest::new(digest_items, period_start, period_end);

    // Save locally if configured
    let saved_path = if digest_config.save_local {
        let manager = digest::DigestManager::new(digest_config.clone());
        match manager.save_local(&digest_obj) {
            Ok(path) => Some(path.to_string_lossy().to_string()),
            Err(e) => {
                warn!(target: "4da::digest", error = %e, "Failed to save digest locally");
                None
            }
        }
    } else {
        None
    };

    // Update last_sent timestamp
    {
        let mut settings_guard = get_settings_manager().lock();
        settings_guard.get_mut().digest.last_sent = Some(Utc::now());
        settings_guard.save()?;
    }

    info!(
        target: "4da::digest",
        items = digest_obj.summary.total_items,
        avg_relevance = %format!("{:.1}%", digest_obj.summary.avg_relevance * 100.0),
        "Digest generated"
    );

    Ok(serde_json::json!({
        "success": true,
        "digest": {
            "id": digest_obj.id,
            "created_at": digest_obj.created_at,
            "period_start": digest_obj.period_start,
            "period_end": digest_obj.period_end,
            "summary": digest_obj.summary,
            "item_count": digest_obj.items.len()
        },
        "saved_path": saved_path,
        "text": digest_obj.to_text(),
        "markdown": digest_obj.to_markdown(),
        "html": digest_obj.to_html()
    }))
}

/// Preview what would be in a digest without generating it
#[tauri::command]
async fn preview_digest() -> Result<serde_json::Value, String> {
    use chrono::{Duration, Utc};

    // Get settings (clone to avoid holding lock during DB operations)
    let digest_config = {
        let settings_guard = get_settings_manager().lock();
        settings_guard.get().digest.clone()
    };

    let db = get_database()?;

    let period_end = Utc::now();
    let period_start = digest_config
        .last_sent
        .unwrap_or(period_end - Duration::hours(24));

    let items = db
        .get_relevant_items_since(
            period_start,
            digest_config.min_score,
            digest_config.max_items,
        )
        .map_err(|e| format!("Failed to fetch items: {}", e))?;

    Ok(serde_json::json!({
        "period_start": period_start,
        "period_end": period_end,
        "item_count": items.len(),
        "min_score": digest_config.min_score,
        "items": items.iter().take(5).map(|i| serde_json::json!({
            "title": i.title,
            "source": i.source_type,
            "score": i.relevance_score
        })).collect::<Vec<_>>()
    }))
}

// ============================================================================
// RSS Feed Commands
// ============================================================================

/// Get configured RSS feed URLs
#[tauri::command]
async fn get_rss_feeds() -> Result<serde_json::Value, String> {
    let settings_guard = get_settings_manager().lock();
    let feeds = settings_guard.get_rss_feeds();

    Ok(serde_json::json!({
        "feeds": feeds,
        "count": feeds.len()
    }))
}

/// Add an RSS feed URL
#[tauri::command]
async fn add_rss_feed(url: String) -> Result<serde_json::Value, String> {
    // Basic URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Invalid URL: must start with http:// or https://".to_string());
    }

    let mut settings_guard = get_settings_manager().lock();
    settings_guard.add_rss_feed(url.clone())?;

    let feeds = settings_guard.get_rss_feeds();

    info!(target: "4da::rss", url = %url, "Added RSS feed");

    Ok(serde_json::json!({
        "success": true,
        "added": url,
        "feeds": feeds,
        "count": feeds.len()
    }))
}

/// Remove an RSS feed URL
#[tauri::command]
async fn remove_rss_feed(url: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.remove_rss_feed(&url)?;

    let feeds = settings_guard.get_rss_feeds();

    info!(target: "4da::rss", url = %url, "Removed RSS feed");

    Ok(serde_json::json!({
        "success": true,
        "removed": url,
        "feeds": feeds,
        "count": feeds.len()
    }))
}

/// Set all RSS feed URLs (replacing existing)
#[tauri::command]
async fn set_rss_feeds(feeds: Vec<String>) -> Result<serde_json::Value, String> {
    // Validate all URLs
    for url in &feeds {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(format!(
                "Invalid URL: {} must start with http:// or https://",
                url
            ));
        }
    }

    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_rss_feeds(feeds.clone())?;

    info!(target: "4da::rss", count = feeds.len(), "Set RSS feeds");

    Ok(serde_json::json!({
        "success": true,
        "feeds": feeds,
        "count": feeds.len()
    }))
}

// ============================================================================
// Twitter Source Commands
// ============================================================================

/// Get configured Twitter handles
#[tauri::command]
async fn get_twitter_handles() -> Result<serde_json::Value, String> {
    let settings_guard = get_settings_manager().lock();
    let handles = settings_guard.get_twitter_handles();

    Ok(serde_json::json!({
        "handles": handles,
        "count": handles.len()
    }))
}

/// Add a Twitter handle
#[tauri::command]
async fn add_twitter_handle(handle: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();

    // Validate handle (remove @ if present)
    let clean_handle = handle.trim_start_matches('@').to_string();

    settings_guard.add_twitter_handle(clean_handle.clone())?;

    let handles = settings_guard.get_twitter_handles();

    info!(target: "4da::twitter", handle = %clean_handle, "Added Twitter handle");

    Ok(serde_json::json!({
        "success": true,
        "added": clean_handle,
        "handles": handles,
        "count": handles.len()
    }))
}

/// Remove a Twitter handle
#[tauri::command]
async fn remove_twitter_handle(handle: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.remove_twitter_handle(&handle)?;

    let handles = settings_guard.get_twitter_handles();

    info!(target: "4da::twitter", handle = %handle, "Removed Twitter handle");

    Ok(serde_json::json!({
        "success": true,
        "removed": handle,
        "handles": handles,
        "count": handles.len()
    }))
}

/// Set all Twitter handles (replacing existing)
#[tauri::command]
async fn set_twitter_handles(handles: Vec<String>) -> Result<serde_json::Value, String> {
    // Clean all handles (remove @ if present)
    let clean_handles: Vec<String> = handles
        .iter()
        .map(|h| h.trim_start_matches('@').to_string())
        .collect();

    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_twitter_handles(clean_handles.clone())?;

    info!(target: "4da::twitter", count = clean_handles.len(), "Set Twitter handles");

    Ok(serde_json::json!({
        "success": true,
        "handles": clean_handles,
        "count": clean_handles.len()
    }))
}

/// Get configured Nitter instance
#[tauri::command]
async fn get_nitter_instance() -> Result<String, String> {
    let settings_guard = get_settings_manager().lock();
    Ok(settings_guard.get_nitter_instance())
}

/// Set Nitter instance
#[tauri::command]
async fn set_nitter_instance(instance: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_nitter_instance(instance.clone())?;

    info!(target: "4da::twitter", instance = %instance, "Set Nitter instance");

    Ok(serde_json::json!({
        "success": true,
        "instance": instance
    }))
}

// ============================================================================
// X API Key Commands
// ============================================================================

/// Get configured X API Bearer Token
#[tauri::command]
async fn get_x_api_key() -> Result<String, String> {
    let settings_guard = get_settings_manager().lock();
    Ok(settings_guard.get_x_api_key())
}

/// Sanitize an X API Bearer Token (trim, URL-decode, extract from pasted blobs)
fn sanitize_x_api_key(raw: &str) -> String {
    let mut key = raw.trim().to_string();

    // URL-decode if it contains percent-encoded chars
    if key.contains('%') {
        if let Ok(decoded) = urlencoding::decode(&key) {
            key = decoded.into_owned();
        }
    }

    // If the pasted value contains spaces, try to extract the Bearer Token portion.
    // X Bearer Tokens start with "AAAAAAAAAAAAAAAAAAAAAA" (22+ A's).
    if key.contains(' ') {
        if let Some(token_start) = key.find("AAAAAAAAAAAAAAAAAAAAAA") {
            key = key[token_start..].trim().to_string();
            info!(target: "4da::twitter", "Extracted Bearer Token from pasted credentials");
        }
    }

    key
}

/// Set X API Bearer Token
#[tauri::command]
async fn set_x_api_key(key: String) -> Result<serde_json::Value, String> {
    let cleaned = sanitize_x_api_key(&key);

    if cleaned.is_empty() {
        let mut settings_guard = get_settings_manager().lock();
        settings_guard.set_x_api_key(String::new())?;
        return Ok(serde_json::json!({
            "success": true,
            "has_key": false,
            "validated": false
        }));
    }

    // Validate the token by making a test API call
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let resp = client
        .get("https://api.x.com/2/users/by/username/twitter")
        .bearer_auth(&cleaned)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            info!(target: "4da::twitter", "X API key validated successfully");
            let mut settings_guard = get_settings_manager().lock();
            settings_guard.set_x_api_key(cleaned)?;
            Ok(serde_json::json!({
                "success": true,
                "has_key": true,
                "validated": true
            }))
        }
        Ok(r) if r.status().as_u16() == 401 => {
            warn!(target: "4da::twitter", "X API key validation failed: 401 Unauthorized");
            Err("Invalid X API Bearer Token. Make sure you're using the Bearer Token from your X Developer Portal (not the API Key/Secret). It should start with 'AAAA...'.".to_string())
        }
        Ok(r) if r.status().as_u16() == 429 => {
            // Rate limited - token format looks valid if we got this far, save it
            info!(target: "4da::twitter", "X API rate limited during validation - saving token anyway");
            let mut settings_guard = get_settings_manager().lock();
            settings_guard.set_x_api_key(cleaned)?;
            Ok(serde_json::json!({
                "success": true,
                "has_key": true,
                "validated": false,
                "warning": "Token saved. Could not validate right now (X API rate limit). It will be used on the next fetch cycle."
            }))
        }
        Ok(r) if r.status().as_u16() == 403 => {
            // 403 can mean the token works but doesn't have the right access level
            warn!(target: "4da::twitter", status = %r.status(), "X API key may lack permissions");
            let mut settings_guard = get_settings_manager().lock();
            settings_guard.set_x_api_key(cleaned)?;
            Ok(serde_json::json!({
                "success": true,
                "has_key": true,
                "validated": false,
                "warning": "Token accepted but may lack required permissions. Ensure your X app has 'Read' access."
            }))
        }
        Ok(r) => {
            warn!(target: "4da::twitter", status = %r.status(), "X API key validation returned unexpected status");
            Err(format!(
                "X API returned HTTP {}. Check your Bearer Token.",
                r.status()
            ))
        }
        Err(e) => {
            warn!(target: "4da::twitter", error = %e, "Could not reach X API for validation");
            // Save anyway - might be a network issue, not a bad token
            let mut settings_guard = get_settings_manager().lock();
            settings_guard.set_x_api_key(cleaned)?;
            Ok(serde_json::json!({
                "success": true,
                "has_key": true,
                "validated": false,
                "warning": "Could not validate token (network issue). Saved anyway."
            }))
        }
    }
}

// ============================================================================
// YouTube Source Commands
// ============================================================================

/// Get configured YouTube channel IDs
#[tauri::command]
async fn get_youtube_channels() -> Result<serde_json::Value, String> {
    let settings_guard = get_settings_manager().lock();
    let channels = settings_guard.get_youtube_channels();

    Ok(serde_json::json!({
        "channels": channels,
        "count": channels.len()
    }))
}

/// Add a YouTube channel ID
#[tauri::command]
async fn add_youtube_channel(channel_id: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.add_youtube_channel(channel_id.clone())?;

    let channels = settings_guard.get_youtube_channels();

    info!(target: "4da::youtube", channel_id = %channel_id, "Added YouTube channel");

    Ok(serde_json::json!({
        "success": true,
        "added": channel_id,
        "channels": channels,
        "count": channels.len()
    }))
}

/// Remove a YouTube channel ID
#[tauri::command]
async fn remove_youtube_channel(channel_id: String) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.remove_youtube_channel(&channel_id)?;

    let channels = settings_guard.get_youtube_channels();

    info!(target: "4da::youtube", channel_id = %channel_id, "Removed YouTube channel");

    Ok(serde_json::json!({
        "success": true,
        "removed": channel_id,
        "channels": channels,
        "count": channels.len()
    }))
}

/// Set all YouTube channel IDs (replacing existing)
#[tauri::command]
async fn set_youtube_channels(channels: Vec<String>) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_youtube_channels(channels.clone())?;

    info!(target: "4da::youtube", count = channels.len(), "Set YouTube channels");

    Ok(serde_json::json!({
        "success": true,
        "channels": channels,
        "count": channels.len()
    }))
}

// ============================================================================
// GitHub Source Commands (Optional - for future language configuration)
// ============================================================================

/// Get configured GitHub languages (default: rust, typescript, python)
#[tauri::command]
async fn get_github_languages() -> Result<serde_json::Value, String> {
    let settings_guard = get_settings_manager().lock();
    let languages = settings_guard.get_github_languages();

    // Return saved languages, or defaults if none configured
    let result = if languages.is_empty() {
        vec![
            "rust".to_string(),
            "typescript".to_string(),
            "python".to_string(),
        ]
    } else {
        languages
    };

    Ok(serde_json::json!({
        "languages": result,
        "count": result.len()
    }))
}

/// Set GitHub languages to monitor
#[tauri::command]
async fn set_github_languages(languages: Vec<String>) -> Result<serde_json::Value, String> {
    let mut settings_guard = get_settings_manager().lock();
    settings_guard.set_github_languages(languages.clone())?;

    info!(target: "4da::github", count = languages.len(), "Set GitHub languages");

    Ok(serde_json::json!({
        "success": true,
        "languages": languages,
        "count": languages.len()
    }))
}

// ============================================================================
// AI Briefing Commands
// ============================================================================

/// Generate an AI-powered briefing from recent relevant items
/// Uses the configured LLM (Ollama by default) to synthesize insights
#[tauri::command]
async fn generate_ai_briefing() -> Result<serde_json::Value, String> {
    use chrono::{Duration, Utc};

    info!(target: "4da::briefing", "Generating AI briefing");

    // Get LLM settings
    let llm_settings = {
        let settings = get_settings_manager().lock();
        settings.get().llm.clone()
    };

    // Check if LLM is configured
    if llm_settings.provider != "ollama" && llm_settings.api_key.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "error": "No LLM configured. Set up Ollama or add an API key in Settings.",
            "briefing": null
        }));
    }

    // Get recent relevant items (last 24 hours, score > 0.1 to include more context)
    let db = get_database()?;
    let period_end = Utc::now();
    let period_start = period_end - Duration::hours(24);

    let items = db
        .get_relevant_items_since(period_start, 0.1, 30)
        .map_err(|e| format!("Failed to fetch items: {}", e))?;

    if items.is_empty() {
        return Ok(serde_json::json!({
            "success": true,
            "briefing": "No items found in the last 24 hours. Run an analysis first to fetch new content.",
            "item_count": 0,
            "model": llm_settings.model
        }));
    }

    // Get ACE context for personalization
    let ace_ctx = get_ace_context();

    // Format items for the prompt
    let items_text: String = items
        .iter()
        .take(15) // Limit to top 15 for context window
        .enumerate()
        .map(|(i, item)| {
            format!(
                "{}. [{}] {} (score: {:.0}%)\n   URL: {}",
                i + 1,
                item.source_type,
                item.title,
                item.relevance_score.unwrap_or(0.0) * 100.0,
                item.url.as_deref().unwrap_or("N/A")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    // Build context summary
    let tech_summary = if ace_ctx.detected_tech.is_empty() {
        "Not detected".to_string()
    } else {
        ace_ctx
            .detected_tech
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };

    let topics_summary = if ace_ctx.active_topics.is_empty() {
        "None active".to_string()
    } else {
        ace_ctx
            .active_topics
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };

    // Create the prompt
    let system_prompt = r#"You are a personalized research assistant for a software developer. Your job is to synthesize the day's relevant content into actionable insights.

Be concise, direct, and useful. Focus on:
1. What's worth reading NOW (pick top 2-3)
2. Emerging patterns or themes
3. Anything that needs immediate attention

Format your response as:
## Top Picks
[2-3 items that deserve attention, with 1-sentence why]

## Themes
[Brief patterns you notice]

## Quick Takes
[Any other notable items in 1 line each]

Keep it under 300 words. No fluff."#;

    let user_prompt = format!(
        r#"Here's my context:
- Tech stack: {}
- Active topics: {}

Today's items (sorted by relevance):

{}

Give me my personalized briefing."#,
        tech_summary, topics_summary, items_text
    );

    // Call the LLM
    let llm_client = llm::LLMClient::new(llm_settings.clone());
    let messages = vec![llm::Message {
        role: "user".to_string(),
        content: user_prompt,
    }];

    let start_time = std::time::Instant::now();

    match llm_client.complete(system_prompt, messages).await {
        Ok(response) => {
            let elapsed = start_time.elapsed();
            info!(
                target: "4da::briefing",
                tokens = response.input_tokens + response.output_tokens,
                elapsed_ms = elapsed.as_millis(),
                "AI briefing generated"
            );

            Ok(serde_json::json!({
                "success": true,
                "briefing": response.content,
                "item_count": items.len(),
                "model": llm_settings.model,
                "tokens_used": response.input_tokens + response.output_tokens,
                "latency_ms": elapsed.as_millis()
            }))
        }
        Err(e) => {
            error!(target: "4da::briefing", error = %e, "Failed to generate briefing");

            // Provide helpful error message
            let error_msg = if e.contains("Connection refused") || e.contains("connect") {
                "Ollama is not running. Start it with 'ollama serve' or check your LLM settings."
            } else if e.contains("model") {
                "The configured model may not be available. Try 'ollama pull llama3.1:8b-instruct-q8_0'."
            } else {
                &e
            };

            Ok(serde_json::json!({
                "success": false,
                "error": error_msg,
                "briefing": null
            }))
        }
    }
}

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
    let ace_ctx = get_ace_context();
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
    item: &HNRelevance,
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
// Indexed Documents Commands
// ============================================================================

/// Indexed document summary for UI
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexedDocumentSummary {
    pub id: i64,
    pub file_path: String,
    pub file_name: String,
    pub file_type: String,
    pub file_size: i64,
    pub word_count: i64,
    pub extraction_confidence: f64,
    pub indexed_at: String,
}

/// Get list of indexed documents
#[tauri::command]
async fn get_indexed_documents(
    limit: Option<i64>,
    offset: Option<i64>,
    file_type: Option<String>,
) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn();
    let conn = conn.lock();

    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let query = if file_type.is_some() {
        format!(
            "SELECT id, file_path, file_name, file_type, file_size, word_count, extraction_confidence, indexed_at
             FROM indexed_documents
             WHERE file_type = ?
             ORDER BY indexed_at DESC
             LIMIT {} OFFSET {}",
            limit, offset
        )
    } else {
        format!(
            "SELECT id, file_path, file_name, file_type, file_size, word_count, extraction_confidence, indexed_at
             FROM indexed_documents
             ORDER BY indexed_at DESC
             LIMIT {} OFFSET {}",
            limit, offset
        )
    };

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;

    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<IndexedDocumentSummary> {
        Ok(IndexedDocumentSummary {
            id: row.get(0)?,
            file_path: row.get(1)?,
            file_name: row.get(2)?,
            file_type: row.get(3)?,
            file_size: row.get(4)?,
            word_count: row.get(5)?,
            extraction_confidence: row.get(6)?,
            indexed_at: row.get(7)?,
        })
    };

    let docs: Vec<IndexedDocumentSummary> = if let Some(ref ft) = file_type {
        stmt.query_map([ft], map_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        stmt.query_map([], map_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect()
    };

    // Get total count
    let total: i64 = if let Some(ref ft) = file_type {
        conn.query_row(
            "SELECT COUNT(*) FROM indexed_documents WHERE file_type = ?",
            [ft],
            |row| row.get(0),
        )
    } else {
        conn.query_row("SELECT COUNT(*) FROM indexed_documents", [], |row| {
            row.get(0)
        })
    }
    .unwrap_or(0);

    Ok(serde_json::json!({
        "documents": docs,
        "total": total,
        "limit": limit,
        "offset": offset
    }))
}

/// Get document content (chunks) by document ID
#[tauri::command]
async fn get_document_content(document_id: i64) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn();
    let conn = conn.lock();

    // Get document metadata
    let doc: Option<IndexedDocumentSummary> = conn.query_row(
        "SELECT id, file_path, file_name, file_type, file_size, word_count, extraction_confidence, indexed_at
         FROM indexed_documents WHERE id = ?",
        [document_id],
        |row| {
            Ok(IndexedDocumentSummary {
                id: row.get(0)?,
                file_path: row.get(1)?,
                file_name: row.get(2)?,
                file_type: row.get(3)?,
                file_size: row.get(4)?,
                word_count: row.get(5)?,
                extraction_confidence: row.get(6)?,
                indexed_at: row.get(7)?,
            })
        },
    ).ok();

    let doc = doc.ok_or("Document not found")?;

    // Get chunks
    let mut stmt = conn
        .prepare(
            "SELECT chunk_index, content, word_count FROM document_chunks
         WHERE document_id = ? ORDER BY chunk_index",
        )
        .map_err(|e| e.to_string())?;

    let chunks: Vec<serde_json::Value> = stmt
        .query_map([document_id], |row| {
            Ok(serde_json::json!({
                "index": row.get::<_, i64>(0)?,
                "content": row.get::<_, String>(1)?,
                "word_count": row.get::<_, i64>(2)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({
        "document": doc,
        "chunks": chunks
    }))
}

/// Search document content
#[tauri::command]
async fn search_documents(query: String, limit: Option<i64>) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn();
    let conn = conn.lock();

    let limit = limit.unwrap_or(20);
    let search_pattern = format!("%{}%", query);

    let mut stmt = conn.prepare(
        "SELECT DISTINCT d.id, d.file_path, d.file_name, d.file_type, d.word_count, d.indexed_at,
                substr(c.content, 1, 200) as preview
         FROM indexed_documents d
         JOIN document_chunks c ON c.document_id = d.id
         WHERE c.content LIKE ?
         ORDER BY d.indexed_at DESC
         LIMIT ?"
    ).map_err(|e| e.to_string())?;

    let results: Vec<serde_json::Value> = stmt
        .query_map([&search_pattern, &limit.to_string()], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "file_path": row.get::<_, String>(1)?,
                "file_name": row.get::<_, String>(2)?,
                "file_type": row.get::<_, String>(3)?,
                "word_count": row.get::<_, i64>(4)?,
                "indexed_at": row.get::<_, String>(5)?,
                "preview": row.get::<_, String>(6)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({
        "query": query,
        "results": results,
        "count": results.len()
    }))
}

/// Natural language query for documents and context
/// Phase 2 feature - supports queries like "show me files about rust from last week"
#[tauri::command]
async fn natural_language_query(query_text: String) -> Result<serde_json::Value, String> {
    use query::{parse_simple, QueryExecutor};

    let ace = get_ace_engine()?;
    let conn = ace.get_conn().clone();

    // Parse the natural language query
    let parsed = parse_simple(&query_text);

    // Execute the query
    let executor = QueryExecutor::new(conn);
    let result = executor.execute(&parsed).map_err(|e| e.to_string())?;

    // Convert to JSON
    Ok(serde_json::json!({
        "query": result.query,
        "intent": format!("{:?}", result.intent),
        "items": result.items,
        "total_count": result.total_count,
        "execution_ms": result.execution_ms,
        "summary": result.summary,
        "parsed": {
            "keywords": parsed.keywords,
            "entities": parsed.entities,
            "time_range": parsed.time_range.map(|tr| serde_json::json!({
                "start": tr.start.to_rfc3339(),
                "end": tr.end.to_rfc3339(),
                "relative": tr.relative
            })),
            "file_types": parsed.file_types,
            "sentiment": parsed.sentiment.map(|s| format!("{:?}", s)),
            "confidence": parsed.confidence
        }
    }))
}

/// Get indexed documents statistics
#[tauri::command]
async fn get_indexed_stats() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn();
    let conn = conn.lock();

    let total_docs: i64 = conn
        .query_row("SELECT COUNT(*) FROM indexed_documents", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let total_chunks: i64 = conn
        .query_row("SELECT COUNT(*) FROM document_chunks", [], |row| row.get(0))
        .unwrap_or(0);

    let total_words: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(word_count), 0) FROM indexed_documents",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Get counts by file type
    let mut stmt = conn
        .prepare("SELECT file_type, COUNT(*) FROM indexed_documents GROUP BY file_type")
        .map_err(|e| e.to_string())?;

    let by_type: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "file_type": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({
        "total_documents": total_docs,
        "total_chunks": total_chunks,
        "total_words": total_words,
        "by_type": by_type
    }))
}

// ============================================================================
// Job Queue Commands (Background Extraction Processing)
// ============================================================================

/// Create an extraction job for a file
#[tauri::command]
async fn create_extraction_job(file_path: String, file_type: String) -> Result<i64, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.create_job(&file_path, &file_type)
}

/// Get a specific extraction job
#[tauri::command]
async fn get_extraction_job(job_id: i64) -> Result<Option<job_queue::ExtractionJob>, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.get_job(job_id)
}

/// Get extraction jobs with optional status filter
#[tauri::command]
async fn get_extraction_jobs(
    status: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<job_queue::ExtractionJob>, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    let status = status.map(|s| job_queue::JobStatus::from_str(&s));
    queue.get_jobs(status, limit.unwrap_or(50))
}

/// Get job queue statistics
#[tauri::command]
async fn get_job_queue_stats() -> Result<job_queue::QueueStats, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.get_stats()
}

/// Cancel an extraction job
#[tauri::command]
async fn cancel_extraction_job(job_id: i64) -> Result<(), String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.cancel_job(job_id)
}

/// Start the job queue background worker
#[tauri::command]
async fn start_job_queue_worker() -> Result<(), String> {
    let queue = get_job_queue()?;
    let mut queue = queue.write();
    queue.start_worker()
}

/// Stop the job queue background worker
#[tauri::command]
async fn stop_job_queue_worker() -> Result<(), String> {
    let queue = get_job_queue()?;
    let mut queue = queue.write();
    queue.stop_worker();
    Ok(())
}

/// Clean up old completed/failed jobs
#[tauri::command]
async fn cleanup_extraction_jobs(days: Option<u32>) -> Result<usize, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.cleanup_old_jobs(days.unwrap_or(7))
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
    info!(target: "4da::startup", threshold = RELEVANCE_THRESHOLD, "Relevance threshold");

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
            get_context_files,
            clear_context,
            index_context,
            index_project_readmes,
            get_context_settings,
            set_context_dirs,
            get_hn_top_stories,
            compute_relevance,
            get_database_stats,
            get_sources,
            start_background_analysis,
            run_multi_source_analysis,
            run_deep_initial_scan,
            run_cached_analysis,
            get_analysis_status,
            // Settings commands (Phase 2)
            get_settings,
            set_llm_provider,
            mark_onboarding_complete,
            set_rerank_config,
            test_llm_connection,
            check_ollama_status,
            get_usage_stats,
            // Monitoring commands (Phase 3)
            get_monitoring_status,
            set_monitoring_enabled,
            set_monitoring_interval,
            trigger_notification_test,
            // Context Engine commands
            get_user_context,
            set_user_role,
            add_tech_stack,
            remove_tech_stack,
            add_domain,
            remove_domain,
            add_interest,
            remove_interest,
            add_exclusion,
            remove_exclusion,
            record_interaction,
            get_context_stats,
            // ACE (Autonomous Context Engine) commands - Phase A
            ace_detect_context,
            ace_get_detected_tech,
            ace_get_active_topics,
            // ACE Phase B: Real-Time Context
            ace_analyze_git,
            ace_get_realtime_context,
            ace_apply_decay,
            ace_full_scan,
            // ACE Autonomous Discovery
            ace_auto_discover,
            ace_reset_discovery,
            ace_get_discovery_status,
            // ACE Phase C: Behavior Learning
            ace_record_interaction,
            ace_get_topic_affinities,
            ace_get_anti_topics,
            ace_confirm_anti_topic,
            ace_get_behavior_modifier,
            ace_get_learned_behavior,
            ace_apply_behavior_decay,
            // ACE Phase E: Embedding
            ace_embed_topic,
            ace_find_similar_topics,
            ace_embedding_status,
            // ACE Phase E: Watcher Persistence
            ace_save_watcher_state,
            ace_get_watcher_state,
            ace_clear_watcher_state,
            // ACE Phase E: Rate Limiting
            ace_get_rate_limit_status,
            // ACE Watcher Control
            ace_start_watcher,
            ace_stop_watcher,
            ace_is_watching,
            // Update commands
            check_for_updates,
            get_current_version,
            // Digest commands
            get_digest_config,
            set_digest_config,
            generate_digest,
            preview_digest,
            // RSS commands
            get_rss_feeds,
            add_rss_feed,
            remove_rss_feed,
            set_rss_feeds,
            // Twitter commands
            get_twitter_handles,
            add_twitter_handle,
            remove_twitter_handle,
            set_twitter_handles,
            get_nitter_instance,
            set_nitter_instance,
            // X API key commands
            get_x_api_key,
            set_x_api_key,
            // YouTube commands
            get_youtube_channels,
            add_youtube_channel,
            remove_youtube_channel,
            set_youtube_channels,
            // GitHub commands
            get_github_languages,
            set_github_languages,
            // AI Briefing commands
            generate_ai_briefing,
            // MCP Score Autopsy
            mcp_score_autopsy,
            // Indexed Documents commands
            get_indexed_documents,
            get_document_content,
            search_documents,
            get_indexed_stats,
            // Natural Language Query (Phase 2)
            natural_language_query,
            // Job Queue commands (background extraction)
            create_extraction_job,
            get_extraction_job,
            get_extraction_jobs,
            get_job_queue_stats,
            cancel_extraction_job,
            start_job_queue_worker,
            stop_job_queue_worker,
            cleanup_extraction_jobs,
            // Void Engine
            get_void_signal,
            void_get_universe,
            void_get_particle_detail,
            void_get_neighbors,
            // Signal Classifier
            get_actionable_signals
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
                    match analyze_cached_content_impl(&handle).await {
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
                                &state,
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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

/// Start background analysis - returns immediately, emits progress events
#[tauri::command]
async fn start_background_analysis(app: AppHandle) -> Result<(), String> {
    // Check if already running
    {
        let state = get_analysis_state();
        let guard = state.lock();
        if guard.running {
            return Err("Analysis already running".to_string());
        }
    }

    // Mark as running
    {
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        guard.results = None;
    }

    // Spawn background task
    tokio::spawn(async move {
        let result = run_background_analysis(&app).await;

        // Update state with result
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(results) => {
                guard.completed = true;
                guard.results = Some(results.clone());

                // Emit completion event
                let _ = app.emit("analysis-complete", &results);

                // Save digest if enabled
                maybe_save_digest(&results);

                // Send notification if relevant items found
                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

                // Update void engine heartbeat
                void_signal_analysis_complete(&app, &results);
            }
            Err(e) => {
                guard.error = Some(e.clone());

                // Emit error event
                let _ = app.emit("analysis-error", &e);
                void_signal_error(&app);
            }
        }
    });

    Ok(())
}

/// Generate and save digest from analysis results (if enabled)
fn maybe_save_digest(results: &[HNRelevance]) {
    use chrono::{Duration, Utc};
    use digest::{Digest, DigestItem, DigestManager};

    let settings = get_settings_manager().lock();
    let config = settings.get().digest.clone();
    drop(settings);

    // Check if digest is enabled and save_local is true
    if !config.enabled || !config.save_local {
        return;
    }

    // Filter to only relevant items above min_score
    let relevant_items: Vec<DigestItem> = results
        .iter()
        .filter(|r| r.relevant && r.top_score as f64 >= config.min_score)
        .take(config.max_items)
        .map(|r| DigestItem {
            id: r.id as i64,
            title: r.title.clone(),
            url: r.url.clone(),
            source: r.source_type.clone(),
            relevance_score: r.top_score as f64,
            matched_topics: r.matches.iter().map(|m| m.source_file.clone()).collect(),
            discovered_at: Utc::now(),
            summary: None,
            signal_type: r.signal_type.clone(),
            signal_priority: r.signal_priority.clone(),
            signal_action: r.signal_action.clone(),
        })
        .collect();

    if relevant_items.is_empty() {
        info!(target: "4da::digest", "No relevant items for digest, skipping");
        return;
    }

    // Create digest
    let period_end = Utc::now();
    let period_start = period_end - Duration::hours(24);
    let digest = Digest::new(relevant_items, period_start, period_end);

    // Save using DigestManager
    let manager = DigestManager::new(config);
    match manager.save_local(&digest) {
        Ok(path) => {
            info!(target: "4da::digest",
                path = %path.display(),
                items = digest.summary.total_items,
                "Digest saved successfully"
            );
        }
        Err(e) => {
            warn!(target: "4da::digest", error = %e, "Failed to save digest");
        }
    }
}

/// The actual background analysis work
async fn run_background_analysis(app: &AppHandle) -> Result<Vec<HNRelevance>, String> {
    info!(target: "4da::analysis", "=== BACKGROUND ANALYSIS STARTED ===");

    emit_progress(app, "init", 0.0, "Initializing...", 0, 0);

    let db = get_database()?;

    // Step 1: Check context (using sqlite-vec KNN)
    emit_progress(
        app,
        "context",
        0.05,
        "Checking context (KNN enabled)...",
        0,
        0,
    );
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;

    if cached_context_count > 0 {
        info!(target: "4da::analysis", context_chunks = cached_context_count, "Context indexed (using KNN search)");
        emit_progress(
            app,
            "context",
            0.1,
            &format!("{} chunks indexed (KNN enabled)", cached_context_count),
            0,
            0,
        );
    } else {
        warn!(target: "4da::analysis", "No context indexed. Running without context-based scoring");
        emit_progress(
            app,
            "context",
            0.1,
            "No context indexed - add files to context directory",
            0,
            0,
        );
    }

    // Step 2: Fetch HN story IDs
    emit_progress(app, "fetch", 0.15, "Fetching story IDs...", 0, 30);

    let client = reqwest::Client::new();
    let top_ids: Vec<u64> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch top stories: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse top stories: {}", e))?;

    let total_items = 30.min(top_ids.len());
    emit_progress(
        app,
        "fetch",
        0.2,
        &format!("Processing {} stories...", total_items),
        0,
        total_items,
    );

    // Step 3: Process items incrementally with progress updates
    let mut cached_items: Vec<(HNItem, Vec<f32>)> = Vec::new();
    let mut new_items: Vec<HNItem> = Vec::new();

    for (idx, id) in top_ids.into_iter().take(total_items).enumerate() {
        let id_str = id.to_string();
        let progress = 0.2 + (0.5 * (idx as f32 / total_items as f32));

        // Check cache first
        if let Ok(Some(cached)) = db.get_source_item("hackernews", &id_str) {
            emit_progress(
                app,
                "fetch",
                progress,
                &format!("Cached: {}", &truncate_utf8(&cached.title, 35)),
                idx + 1,
                total_items,
            );
            db.touch_source_item("hackernews", &id_str).ok();
            cached_items.push((
                HNItem {
                    id,
                    title: cached.title,
                    url: cached.url,
                    content: cached.content,
                },
                cached.embedding,
            ));
        } else {
            // Fetch from API
            let url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);
            match client.get(&url).send().await {
                Ok(response) => match response.json::<HNStory>().await {
                    Ok(story) => {
                        let title = story.title.unwrap_or_else(|| "[No title]".to_string());
                        emit_progress(
                            app,
                            "fetch",
                            progress,
                            &format!("Fetching: {}", &truncate_utf8(&title, 35)),
                            idx + 1,
                            total_items,
                        );

                        let content = if let Some(text) = story.text {
                            text
                        } else if let Some(ref article_url) = story.url {
                            emit_progress(
                                app,
                                "scrape",
                                progress,
                                &format!("Scraping: {}", &truncate_utf8(&title, 35)),
                                idx + 1,
                                total_items,
                            );
                            scrape_article_content(article_url)
                                .await
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };

                        new_items.push(HNItem {
                            id: story.id,
                            title,
                            url: story.url,
                            content,
                        });
                    }
                    Err(_) => {}
                },
                Err(_) => {}
            }
        }
    }

    // Step 4: Embed new items
    let new_embeddings = if !new_items.is_empty() {
        emit_progress(
            app,
            "embed",
            0.75,
            &format!("Embedding {} new items...", new_items.len()),
            cached_items.len(),
            total_items,
        );

        let new_texts: Vec<String> = new_items
            .iter()
            .map(|item| build_embedding_text(&item.title, &item.content))
            .collect();
        let embeddings = embed_texts(&new_texts)?;

        for (item, embedding) in new_items.iter().zip(embeddings.iter()) {
            db.upsert_source_item(
                "hackernews",
                &item.id.to_string(),
                item.url.as_deref(),
                &item.title,
                &item.content,
                embedding,
            )
            .ok();
        }

        embeddings
    } else {
        emit_progress(
            app,
            "embed",
            0.75,
            "All items cached!",
            total_items,
            total_items,
        );
        vec![]
    };

    db.update_source_fetch_time("hackernews").ok();

    // Combine all items
    let mut all_items_with_embeddings: Vec<(HNItem, Vec<f32>)> = cached_items;
    for (item, embedding) in new_items.into_iter().zip(new_embeddings.into_iter()) {
        all_items_with_embeddings.push((item, embedding));
    }

    // Step 5: Load user context for personalized scoring
    emit_progress(
        app,
        "relevance",
        0.82,
        "Loading user context...",
        0,
        all_items_with_embeddings.len(),
    );

    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    let interest_count = static_identity.interests.len();
    let exclusion_count = static_identity.exclusions.len();
    debug!(target: "4da::analysis", interests = interest_count, exclusions = exclusion_count, "User context loaded");

    // Step 6: Compute personalized relevance
    emit_progress(
        app,
        "relevance",
        0.85,
        "Computing personalized relevance...",
        0,
        all_items_with_embeddings.len(),
    );

    let mut results: Vec<HNRelevance> = Vec::new();
    let mut excluded_count = 0;

    for (idx, (item, item_embedding)) in all_items_with_embeddings.iter().enumerate() {
        let progress = 0.85 + (0.10 * (idx as f32 / all_items_with_embeddings.len() as f32));

        // Extract topics for exclusion checking
        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions FIRST (hard filter)
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions);

        if let Some(ref exclusion) = excluded_by {
            excluded_count += 1;
            emit_progress(
                app,
                "relevance",
                progress,
                &format!(
                    "Excluded: {} ({})",
                    &truncate_utf8(&item.title, 30),
                    exclusion
                ),
                idx + 1,
                all_items_with_embeddings.len(),
            );

            results.push(HNRelevance {
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
            });
            continue;
        }

        emit_progress(
            app,
            "relevance",
            progress,
            &format!("Scoring: {}", &truncate_utf8(&item.title, 35)),
            idx + 1,
            all_items_with_embeddings.len(),
        );

        // Compute context file score using sqlite-vec KNN search (O(log n))
        // With graceful fallback to empty matches if KNN fails
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 {
            match db.find_similar_contexts(item_embedding, 3) {
                Ok(results) => results
                    .into_iter()
                    .map(|result| {
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

        // Compute interest score
        let interest_score = compute_interest_score(item_embedding, &static_identity.interests);

        // Combined score - adjust weights based on available data
        let combined_score = if cached_context_count > 0 && interest_count > 0 {
            context_score * 0.5 + interest_score * 0.5
        } else if interest_count > 0 {
            // No context indexed: rely on interests (full weight)
            interest_score * 0.7
        } else if cached_context_count > 0 {
            context_score
        } else {
            0.0
        };

        let relevant = combined_score >= RELEVANCE_THRESHOLD;

        // Generate simple explanation for relevant items (legacy path without ACE)
        let explanation = if relevant {
            let mut reasons: Vec<String> = Vec::new();
            if context_score > 0.2 {
                if let Some(first_match) = matches.first() {
                    let file_name = first_match
                        .source_file
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&first_match.source_file);
                    reasons.push(format!("Matches your current work ({})", file_name));
                } else {
                    reasons.push("Relates to your active projects".to_string());
                }
            }
            if interest_score > 0.2 {
                reasons.push("Matches your declared interests".to_string());
            }
            if reasons.is_empty() {
                Some("Matches your overall profile".to_string())
            } else if reasons.len() == 1 {
                Some(reasons[0].clone())
            } else {
                Some(format!("{}; {}", reasons[0], reasons[1]))
            }
        } else {
            None
        };

        // Calculate simple confidence for legacy path (no ACE)
        let confidence = if cached_context_count > 0 && interest_count > 0 {
            (context_score + interest_score) / 2.0
        } else if interest_count > 0 {
            interest_score * 0.8
        } else if cached_context_count > 0 {
            context_score * 0.7
        } else {
            0.3
        };

        let mut confidence_by_signal = std::collections::HashMap::new();
        if cached_context_count > 0 {
            confidence_by_signal.insert("context".to_string(), context_score);
        }
        if interest_count > 0 {
            confidence_by_signal.insert("interest".to_string(), interest_score);
        }

        let score_breakdown = ScoreBreakdown {
            context_score,
            interest_score,
            ace_boost: 0.0,
            affinity_mult: 1.0,
            anti_penalty: 0.0,
            freshness_mult: 1.0,
            confidence_by_signal,
        };

        results.push(HNRelevance {
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
        });
    }

    // Sort: excluded items last, then by score
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

    if excluded_count > 0 {
        info!(target: "4da::analysis", excluded = excluded_count, "Items excluded by user preferences");
    }

    // Step 6: LLM Re-ranking (if enabled)
    let settings_manager = get_settings_manager();
    let (rerank_enabled, llm_settings, rerank_config) = {
        let mut guard = settings_manager.lock();
        let is_enabled = guard.is_rerank_enabled() && guard.within_daily_limits();
        let llm = guard.get().llm.clone();
        let rerank = guard.get().rerank.clone();
        (is_enabled, llm, rerank)
    };

    if rerank_enabled {
        emit_progress(
            app,
            "rerank",
            0.92,
            "LLM re-ranking enabled, filtering candidates...",
            0,
            0,
        );

        // Get items that pass the minimum embedding score
        let candidates: Vec<&mut HNRelevance> = results
            .iter_mut()
            .filter(|r| r.top_score >= rerank_config.min_embedding_score)
            .take(rerank_config.max_items_per_batch)
            .collect();

        let candidate_count = candidates.len();

        if candidate_count > 0 {
            info!(target: "4da::llm", candidates = candidate_count, threshold = rerank_config.min_embedding_score, "LLM Re-ranking candidates");

            emit_progress(
                app,
                "rerank",
                0.93,
                &format!("Sending {} items to LLM for re-ranking...", candidate_count),
                0,
                candidate_count,
            );

            // Build comprehensive context summary from database
            // This gives the LLM a complete picture of user's interests
            let context_summary: String = if cached_context_count > 0 {
                db.get_all_contexts()
                    .map_err(|e| format!("Failed to get contexts for LLM: {}", e))?
                    .iter()
                    .take(20) // Limit to avoid token overflow
                    .map(|c| {
                        format!(
                            "[{}]\n{}",
                            c.source_file,
                            c.text.chars().take(600).collect::<String>()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n")
            } else {
                String::new()
            };

            // Prepare items for LLM with more context
            // Include all top matches to give LLM full picture
            let items_for_llm: Vec<(String, String, String)> = candidates
                .iter()
                .map(|r| {
                    // Combine all match texts for richer context
                    let content_snippet = r
                        .matches
                        .iter()
                        .map(|m| format!("Matched '{}': {}", m.source_file, m.matched_text))
                        .collect::<Vec<_>>()
                        .join(" | ");
                    (r.id.to_string(), r.title.clone(), content_snippet)
                })
                .collect();

            // Create judge and run
            let judge = RelevanceJudge::new(llm_settings);

            match judge.judge_batch(&context_summary, items_for_llm).await {
                Ok((judgments, input_tokens, output_tokens)) => {
                    let cost_cents = judge.estimate_cost_cents(input_tokens, output_tokens);

                    info!(target: "4da::llm",
                        judgments = judgments.len(),
                        tokens = input_tokens + output_tokens,
                        cost_cents = cost_cents,
                        "LLM Re-ranking complete"
                    );

                    // Record usage
                    {
                        let mut guard = settings_manager.lock();
                        guard.record_usage(input_tokens + output_tokens, cost_cents);
                    }

                    // Apply LLM judgments with confidence threshold
                    // Only demote if LLM is confident (>= 0.7) to avoid over-filtering
                    const DEMOTION_CONFIDENCE_THRESHOLD: f32 = 0.7;
                    let mut llm_relevant_count = 0;
                    let mut demoted_count = 0;
                    let mut kept_by_low_confidence = 0;

                    let mut no_match_count = 0;
                    for result in results.iter_mut() {
                        let result_id_str = result.id.to_string();
                        if let Some(judgment) =
                            judgments.iter().find(|j| j.item_id == result_id_str)
                        {
                            if judgment.relevant {
                                // LLM confirms relevance
                                llm_relevant_count += 1;
                                debug!(target: "4da::llm", title = %&truncate_utf8(&result.title, 40), confidence = judgment.confidence, "LLM confirmed");
                            } else if result.relevant {
                                // LLM says not relevant - check confidence before demoting
                                if judgment.confidence >= DEMOTION_CONFIDENCE_THRESHOLD {
                                    debug!(target: "4da::llm",
                                        title = %&truncate_utf8(&result.title, 35),
                                        confidence = judgment.confidence,
                                        reason = %&truncate_utf8(&judgment.reasoning, 50),
                                        "LLM demoted"
                                    );
                                    result.relevant = false;
                                    demoted_count += 1;
                                } else {
                                    // Low confidence - keep as relevant (benefit of doubt)
                                    debug!(target: "4da::llm", title = %&truncate_utf8(&result.title, 40), confidence = judgment.confidence, "LLM uncertain, keeping");
                                    llm_relevant_count += 1;
                                    kept_by_low_confidence += 1;
                                }
                            }
                        } else if result.relevant {
                            // No matching judgment found - item keeps embedding relevance
                            no_match_count += 1;
                            if no_match_count <= 3 {
                                debug!(target: "4da::llm", title = %&truncate_utf8(&result.title, 40), id = %result_id_str, "No LLM judgment");
                            }
                        }
                    }

                    if no_match_count > 0 {
                        warn!(target: "4da::llm", count = no_match_count, "Items had no matching LLM judgment");
                    }

                    info!(target: "4da::llm",
                        confirmed = llm_relevant_count - kept_by_low_confidence,
                        demoted = demoted_count,
                        kept_low_confidence = kept_by_low_confidence,
                        "LLM summary"
                    );

                    emit_progress(
                        app,
                        "rerank",
                        0.98,
                        &format!(
                            "LLM kept {} of {} as relevant",
                            llm_relevant_count, candidate_count
                        ),
                        candidate_count,
                        candidate_count,
                    );
                }
                Err(e) => {
                    warn!(target: "4da::llm", error = %e, "LLM Re-ranking failed, using embedding scores only");
                    emit_progress(
                        app,
                        "rerank",
                        0.98,
                        "LLM re-ranking failed, using embeddings only",
                        0,
                        0,
                    );
                }
            }
        } else {
            debug!(target: "4da::llm", "LLM Re-ranking: No candidates above threshold, skipping");
            emit_progress(
                app,
                "rerank",
                0.98,
                "No candidates for LLM re-ranking",
                0,
                0,
            );
        }
    } else {
        debug!(target: "4da::llm", "LLM Re-ranking: Disabled or limit reached");
    }

    emit_progress(
        app,
        "complete",
        1.0,
        "Analysis complete!",
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let final_excluded = results.iter().filter(|r| r.excluded).count();
    info!(target: "4da::analysis", "=== PERSONALIZED ANALYSIS COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = final_excluded,
        interests = interest_count,
        exclusions = exclusion_count,
        "Analysis summary"
    );

    Ok(results)
}

/// Multi-source analysis - fetches from all enabled sources (HN, arXiv, Reddit)
#[tauri::command]
async fn run_multi_source_analysis(app: AppHandle) -> Result<(), String> {
    // Check if already running
    {
        let state = get_analysis_state();
        let guard = state.lock();
        if guard.running {
            return Err("Analysis already running".to_string());
        }
    }

    // Set running state
    {
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        guard.results = None;
    }

    // Spawn background task
    tokio::spawn(async move {
        let result = run_multi_source_analysis_impl(&app).await;

        // Update state with result
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(results) => {
                guard.completed = true;
                guard.results = Some(results.clone());
                let _ = app.emit("analysis-complete", &results);

                // Save digest if enabled
                maybe_save_digest(&results);

                // Send notification if relevant items found
                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }
            }
            Err(e) => {
                guard.error = Some(e.clone());
                let _ = app.emit("analysis-error", &e);
            }
        }
    });

    Ok(())
}

/// Deep initial scan - comprehensive first-time scan for new users
/// Fetches 300-500+ items from all sources using multiple endpoints
#[tauri::command]
async fn run_deep_initial_scan(app: AppHandle) -> Result<(), String> {
    // Check if already running
    {
        let state = get_analysis_state();
        let guard = state.lock();
        if guard.running {
            return Err("Analysis already running".to_string());
        }
    }

    // Set running state
    {
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        guard.results = None;
    }

    info!(target: "4da::analysis", "=== DEEP INITIAL SCAN STARTING ===");
    info!(target: "4da::analysis", "This comprehensive scan will fetch 300-500+ items from multiple sources");

    // Spawn background task
    tokio::spawn(async move {
        let result = run_deep_initial_scan_impl(&app).await;

        // Update state with result
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(results) => {
                guard.completed = true;
                guard.results = Some(results.clone());
                let _ = app.emit("analysis-complete", &results);

                // Save digest
                maybe_save_digest(&results);

                // Send notification
                let relevant_count = results.iter().filter(|r| r.relevant).count();
                let top_picks = results.iter().filter(|r| r.top_score >= 0.6).count();
                info!(target: "4da::analysis",
                    "=== DEEP INITIAL SCAN COMPLETE ===\n  Total: {} items\n  Relevant: {}\n  Top Picks (60%+): {}",
                    results.len(), relevant_count, top_picks
                );
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }
            }
            Err(e) => {
                error!(target: "4da::analysis", error = %e, "Deep initial scan failed");
                guard.error = Some(e.clone());
                let _ = app.emit("analysis-error", &e);
            }
        }
    });

    Ok(())
}

/// Deep initial scan implementation - comprehensive first-time intelligence gathering
async fn run_deep_initial_scan_impl(app: &AppHandle) -> Result<Vec<HNRelevance>, String> {
    info!(target: "4da::analysis", "=== DEEP INITIAL SCAN STARTED ===");
    info!(target: "4da::analysis", "Fetching 300-500+ items from HN (5 categories), arXiv (16 categories), Reddit (40+ subreddits)...");

    emit_progress(
        app,
        "init",
        0.0,
        "Starting deep initial scan (this may take a few minutes)...",
        0,
        0,
    );

    let db = get_database()?;

    // Step 1: Check context
    emit_progress(app, "context", 0.02, "Checking context...", 0, 0);
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;

    if cached_context_count > 0 {
        info!(target: "4da::analysis", context_chunks = cached_context_count, "Context indexed");
    }

    // Step 2: DEEP fetch from all sources (100 items per category = 500-1500 total)
    emit_progress(
        app,
        "fetch",
        0.05,
        "Deep fetching from all sources (may take a few minutes)...",
        0,
        0,
    );
    let all_items = fetch_all_sources_deep(&db, app, 100).await?;
    info!(target: "4da::analysis", items = all_items.len(), "Deep fetched items from all sources");

    emit_progress(
        app,
        "fetch",
        0.55,
        &format!("Fetched {} items, now scoring...", all_items.len()),
        all_items.len(),
        all_items.len(),
    );

    // Step 3: Load user context
    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    let interest_count = static_identity.interests.len();

    // Step 4: Load ACE context
    let ace_ctx = get_ace_context();
    let topic_embeddings = get_topic_embeddings(&ace_ctx);
    info!(target: "4da::ace",
        topics = ace_ctx.active_topics.len(),
        tech = ace_ctx.detected_tech.len(),
        embeddings = topic_embeddings.len(),
        "ACE context loaded for scoring"
    );

    // Step 5: Compute relevance for all items
    emit_progress(
        app,
        "relevance",
        0.60,
        &format!("Scoring {} items...", all_items.len()),
        0,
        all_items.len(),
    );

    let mut results: Vec<HNRelevance> = Vec::new();
    let mut excluded_count = 0;
    let total_items = all_items.len();

    for (idx, (item, item_embedding)) in all_items.iter().enumerate() {
        // Progress feedback every 50 items
        if idx % 50 == 0 {
            let progress = 0.60 + (0.35 * (idx as f32 / total_items as f32));
            emit_progress(
                app,
                "relevance",
                progress,
                &format!("Scoring {} of {} items...", idx, total_items),
                idx,
                total_items,
            );
        }

        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions)
            .or_else(|| check_ace_exclusions(&topics, &ace_ctx));

        if let Some(ref exclusion) = excluded_by {
            excluded_count += 1;
            results.push(HNRelevance {
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
                source_type: item.source_type.clone(),
                explanation: None,
                confidence: Some(0.0),
                score_breakdown: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
                signal_triggers: None,
            });
            continue;
        }

        // Compute context score using KNN
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 {
            db.find_similar_contexts(item_embedding, 3)
                .unwrap_or_default()
                .into_iter()
                .map(|result| {
                    let similarity = 1.0 / (1.0 + result.distance);
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
                .collect()
        } else {
            vec![]
        };

        let context_score = matches.first().map(|m| m.similarity).unwrap_or(0.0);
        let interest_score = compute_interest_score(item_embedding, &static_identity.interests);

        // PASIFA: Compute semantic boost for topic/tech matching
        let semantic_boost =
            compute_semantic_ace_boost(item_embedding, &ace_ctx, &topic_embeddings).unwrap_or_else(
                || {
                    // Fallback to keyword matching
                    let mut boost: f32 = 0.0;
                    for topic in &topics {
                        let topic_lower = topic.to_lowercase();
                        for active in &ace_ctx.active_topics {
                            if topic_lower.contains(active) || active.contains(&topic_lower) {
                                boost += 0.15
                                    * ace_ctx.topic_confidence.get(active).copied().unwrap_or(0.5);
                                break;
                            }
                        }
                        for tech in &ace_ctx.detected_tech {
                            if topic_lower.contains(tech) || tech.contains(&topic_lower) {
                                boost += 0.12;
                                break;
                            }
                        }
                    }
                    boost.clamp(0.0, 0.3)
                },
            );

        // Compute base score
        let base_score = if cached_context_count > 0 && interest_count > 0 {
            (context_score * 0.5 + interest_score * 0.5 + semantic_boost).min(1.0)
        } else if interest_count > 0 {
            (interest_score * 0.7 + semantic_boost * 1.5).min(1.0)
        } else if cached_context_count > 0 {
            (context_score + semantic_boost).min(1.0)
        } else {
            (semantic_boost * 2.0).min(1.0)
        };

        // Apply unified scoring
        let combined_score = compute_unified_relevance(base_score, &topics, &ace_ctx);
        let relevant = combined_score >= RELEVANCE_THRESHOLD;

        let affinity_mult = compute_affinity_multiplier(&topics, &ace_ctx);
        let anti_penalty = compute_anti_penalty(&topics, &ace_ctx);

        // Generate explanation
        let explanation = if relevant || combined_score >= 0.3 {
            Some(generate_relevance_explanation(
                &item.title,
                context_score,
                interest_score,
                &matches,
                &ace_ctx,
                &topics,
            ))
        } else {
            None
        };

        // Calculate confidence
        let confidence = calculate_confidence(
            context_score,
            interest_score,
            semantic_boost,
            &ace_ctx,
            &topics,
            cached_context_count,
            interest_count as i64,
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

        let breakdown = ScoreBreakdown {
            context_score,
            interest_score,
            ace_boost: semantic_boost,
            affinity_mult,
            anti_penalty,
            freshness_mult: 1.0,
            confidence_by_signal,
        };

        results.push(HNRelevance {
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
            source_type: item.source_type.clone(),
            explanation,
            confidence: Some(confidence),
            score_breakdown: Some(breakdown),
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
        });
    }

    // Sort by score
    results.sort_by(|a, b| {
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    emit_progress(
        app,
        "complete",
        1.0,
        &format!(
            "Deep scan complete! {} items, {} relevant, {} top picks",
            results.len(),
            results.iter().filter(|r| r.relevant).count(),
            results.iter().filter(|r| r.top_score >= 0.6).count()
        ),
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    info!(target: "4da::analysis", "=== DEEP INITIAL SCAN COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = excluded_count,
        interests = interest_count,
        "Deep scan summary"
    );

    Ok(results)
}

/// Multi-source analysis implementation
async fn run_multi_source_analysis_impl(app: &AppHandle) -> Result<Vec<HNRelevance>, String> {
    info!(target: "4da::analysis", "=== MULTI-SOURCE ANALYSIS STARTED ===");

    emit_progress(
        app,
        "init",
        0.0,
        "Initializing multi-source analysis...",
        0,
        0,
    );

    let db = get_database()?;

    // Step 1: Check context (using sqlite-vec KNN)
    emit_progress(
        app,
        "context",
        0.05,
        "Checking context (KNN enabled)...",
        0,
        0,
    );
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;

    if cached_context_count > 0 {
        info!(target: "4da::analysis", context_chunks = cached_context_count, "Context indexed (using KNN search)");
    } else {
        warn!(target: "4da::analysis", "No context indexed. Running without context-based scoring");
    }

    // Step 2: Fetch from all sources (50 items per source for comprehensive coverage)
    emit_progress(app, "fetch", 0.1, "Fetching from all sources...", 0, 0);
    let all_items = fetch_all_sources(&db, app, 50).await?;
    info!(target: "4da::analysis", items = all_items.len(), "Fetched items from all sources");

    // Step 3: Load user context
    emit_progress(
        app,
        "relevance",
        0.7,
        "Loading user context...",
        0,
        all_items.len(),
    );
    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    let interest_count = static_identity.interests.len();
    let _exclusion_count = static_identity.exclusions.len();

    // Step 4: Load ACE context
    let ace_ctx = get_ace_context();
    // PASIFA: Pre-compute topic embeddings for semantic matching
    let topic_embeddings = get_topic_embeddings(&ace_ctx);
    info!(target: "4da::ace",
        topics = ace_ctx.active_topics.len(),
        tech = ace_ctx.detected_tech.len(),
        anti_topics = ace_ctx.anti_topics.len(),
        embeddings = topic_embeddings.len(),
        "ACE context loaded"
    );

    // Step 5: Compute relevance
    emit_progress(
        app,
        "relevance",
        0.75,
        "Computing relevance...",
        0,
        all_items.len(),
    );
    let mut results: Vec<HNRelevance> = Vec::new();
    let mut excluded_count = 0;

    for (idx, (item, item_embedding)) in all_items.iter().enumerate() {
        let progress = 0.75 + (0.20 * (idx as f32 / all_items.len() as f32));
        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions)
            .or_else(|| check_ace_exclusions(&topics, &ace_ctx));

        if let Some(ref exclusion) = excluded_by {
            excluded_count += 1;
            results.push(HNRelevance {
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
                source_type: item.source_type.clone(),
                explanation: None,     // Excluded items don't need explanations
                confidence: Some(0.0), // Excluded items have zero confidence
                score_breakdown: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
                signal_triggers: None,
            });
            continue;
        }

        emit_progress(
            app,
            "relevance",
            progress,
            &format!(
                "[{}] {}",
                &item.source_type,
                &truncate_utf8(&item.title, 30)
            ),
            idx + 1,
            all_items.len(),
        );

        // Compute context file score using sqlite-vec KNN search (O(log n))
        // With graceful fallback to empty matches if KNN fails
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 {
            match db.find_similar_contexts(item_embedding, 3) {
                Ok(results) => results
                    .into_iter()
                    .map(|result| {
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
        let interest_score = compute_interest_score(item_embedding, &static_identity.interests);

        // PASIFA: Compute semantic boost for topic/tech matching
        let semantic_boost = compute_semantic_ace_boost(
            item_embedding,
            &ace_ctx,
            &topic_embeddings,
        )
        .unwrap_or_else(|| {
            // Fall back to keyword matching for active topics and tech
            let mut boost: f32 = 0.0;
            for topic in &topics {
                let topic_lower = topic.to_lowercase();
                for active_topic in &ace_ctx.active_topics {
                    if topic_lower.contains(active_topic) || active_topic.contains(&topic_lower) {
                        let conf = ace_ctx
                            .topic_confidence
                            .get(active_topic)
                            .copied()
                            .unwrap_or(0.5);
                        boost += 0.15 * conf;
                        break;
                    }
                }
                for tech in &ace_ctx.detected_tech {
                    if topic_lower.contains(tech) || tech.contains(&topic_lower) {
                        boost += 0.12;
                        break;
                    }
                }
            }
            boost.clamp(0.0, 0.3)
        });

        // Compute base score - adjust weights based on available data
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

        // PASIFA: Apply unified multiplicative scoring
        let combined_score = compute_unified_relevance(base_score, &topics, &ace_ctx);
        let relevant = combined_score >= RELEVANCE_THRESHOLD;

        // Compute debug info
        let affinity_mult = compute_affinity_multiplier(&topics, &ace_ctx);
        let anti_penalty = compute_anti_penalty(&topics, &ace_ctx);

        // Generate explanation for relevant items
        let explanation = if relevant {
            Some(generate_relevance_explanation(
                &item.title,
                context_score,
                interest_score,
                &matches,
                &ace_ctx,
                &topics,
            ))
        } else {
            None
        };

        // Calculate confidence and score breakdown
        let confidence = calculate_confidence(
            context_score,
            interest_score,
            semantic_boost,
            &ace_ctx,
            &topics,
            cached_context_count,
            interest_count as i64,
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
            ace_boost: semantic_boost,
            affinity_mult,
            anti_penalty,
            freshness_mult: 1.0,
            confidence_by_signal,
        };

        results.push(HNRelevance {
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
            source_type: item.source_type.clone(),
            explanation,
            confidence: Some(confidence),
            score_breakdown: Some(score_breakdown),
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
        });
    }

    // Sort results
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

    emit_progress(
        app,
        "complete",
        1.0,
        "Multi-source analysis complete!",
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    info!(target: "4da::analysis", "=== MULTI-SOURCE ANALYSIS COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = excluded_count,
        "Analysis summary"
    );

    Ok(results)
}

// ============================================================================
// Cache-First Analysis (Option D)
// ============================================================================

/// Cache-first analysis - analyzes items already in the database
/// This is INSTANT because it doesn't fetch from APIs, just scores cached items
#[tauri::command]
async fn run_cached_analysis(app: AppHandle) -> Result<(), String> {
    // Check if already running
    {
        let state = get_analysis_state();
        let guard = state.lock();
        if guard.running {
            return Err("Analysis already running".to_string());
        }
    }

    // Mark as running
    {
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        guard.results = None;
    }

    // Spawn background task
    tokio::spawn(async move {
        let result = analyze_cached_content_impl(&app).await;

        // Update state with result
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(results) => {
                guard.completed = true;
                guard.results = Some(results.clone());

                // Emit completion event
                let _ = app.emit("analysis-complete", &results);

                // Save digest if enabled
                maybe_save_digest(&results);

                // Send notification if relevant items found
                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

                // Update void engine heartbeat
                void_signal_analysis_complete(&app, &results);
            }
            Err(e) => {
                guard.error = Some(e.clone());
                let _ = app.emit("analysis-error", &e);
                void_signal_error(&app);
            }
        }
    });

    Ok(())
}

/// The actual cache-first analysis implementation
/// Scores ALL cached items without any API fetching
async fn analyze_cached_content_impl(app: &AppHandle) -> Result<Vec<HNRelevance>, String> {
    info!(target: "4da::analysis", "=== CACHE-FIRST ANALYSIS STARTED ===");

    emit_progress(app, "init", 0.0, "Loading cached items...", 0, 0);

    let db = get_database()?;

    // Get cached items from last 48 hours (or all recent if less)
    // This is INSTANT - no API calls
    let cached_items = db
        .get_items_since_hours(48, 1000)
        .map_err(|e| format!("Failed to load cached items: {}", e))?;

    let total_cached = cached_items.len();
    info!(target: "4da::analysis", cached_items = total_cached, "Loaded items from cache");

    if total_cached == 0 {
        // Fall back to fetching if cache is empty
        warn!(target: "4da::analysis", "Cache empty, falling back to fetch");
        emit_progress(
            app,
            "fetch",
            0.1,
            "Cache empty, fetching fresh items...",
            0,
            0,
        );
        return run_multi_source_analysis_impl(app).await;
    }

    emit_progress(
        app,
        "cache",
        0.1,
        &format!("Analyzing {} cached items (no API calls)...", total_cached),
        0,
        total_cached,
    );

    // Load context
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;
    info!(target: "4da::analysis", context_chunks = cached_context_count, "Context loaded");

    // Load user interests
    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;
    let interest_count = static_identity.interests.len();

    // Load ACE context
    let ace_ctx = get_ace_context();
    let topic_embeddings = get_topic_embeddings(&ace_ctx);
    info!(target: "4da::ace",
        topics = ace_ctx.active_topics.len(),
        tech = ace_ctx.detected_tech.len(),
        "ACE context loaded"
    );

    emit_progress(
        app,
        "relevance",
        0.2,
        "Scoring cached items...",
        0,
        total_cached,
    );

    // Score all cached items
    let signal_classifier = signals::SignalClassifier::new();
    let mut results: Vec<HNRelevance> = Vec::new();
    let mut excluded_count = 0;

    for (idx, item) in cached_items.iter().enumerate() {
        let progress = 0.2 + (0.75 * (idx as f32 / total_cached as f32));
        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions)
            .or_else(|| check_ace_exclusions(&topics, &ace_ctx));

        if let Some(ref exclusion) = excluded_by {
            excluded_count += 1;
            results.push(HNRelevance {
                id: item.id as u64,
                title: item.title.clone(),
                url: item.url.clone(),
                top_score: 0.0,
                matches: vec![],
                relevant: false,
                context_score: 0.0,
                interest_score: 0.0,
                excluded: true,
                excluded_by: Some(exclusion.clone()),
                source_type: item.source_type.clone(),
                explanation: None,
                confidence: Some(0.0),
                score_breakdown: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
                signal_triggers: None,
            });
            continue;
        }

        if idx % 50 == 0 {
            // Truncate title safely (UTF-8 aware)
            let truncated_title: String = item.title.chars().take(30).collect();
            emit_progress(
                app,
                "relevance",
                progress,
                &format!("[{}] {}", &item.source_type, truncated_title),
                idx + 1,
                total_cached,
            );
        }

        // Compute context file score using KNN
        let item_embedding = &item.embedding;
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 && !item_embedding.is_empty()
        {
            match db.find_similar_contexts(item_embedding, 3) {
                Ok(results) => results
                    .into_iter()
                    .map(|result| {
                        let similarity = 1.0 / (1.0 + result.distance);
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
                Err(_) => vec![],
            }
        } else {
            vec![]
        };

        let context_score = matches.first().map(|m| m.similarity).unwrap_or(0.0);
        let interest_score = compute_interest_score(item_embedding, &static_identity.interests);

        // PASIFA semantic boost
        let semantic_boost = compute_semantic_ace_boost(
            item_embedding,
            &ace_ctx,
            &topic_embeddings,
        )
        .unwrap_or_else(|| {
            let mut boost: f32 = 0.0;
            for topic in &topics {
                let topic_lower = topic.to_lowercase();
                for active_topic in &ace_ctx.active_topics {
                    if topic_lower.contains(active_topic) || active_topic.contains(&topic_lower) {
                        let conf = ace_ctx
                            .topic_confidence
                            .get(active_topic)
                            .copied()
                            .unwrap_or(0.5);
                        boost += 0.15 * conf;
                        break;
                    }
                }
                for tech in &ace_ctx.detected_tech {
                    if topic_lower.contains(tech) || tech.contains(&topic_lower) {
                        boost += 0.12;
                        break;
                    }
                }
            }
            boost.clamp(0.0, 0.3)
        });

        // Compute base score
        let base_score = if cached_context_count > 0 && interest_count > 0 {
            (context_score * 0.5 + interest_score * 0.5 + semantic_boost).min(1.0)
        } else if interest_count > 0 {
            (interest_score * 0.7 + semantic_boost * 1.5).min(1.0)
        } else if cached_context_count > 0 {
            (context_score + semantic_boost).min(1.0)
        } else {
            (semantic_boost * 2.0).min(1.0)
        };

        // Apply temporal freshness: recent items get slight boost, older items decay
        let freshness = compute_temporal_freshness(&item.created_at);
        let base_score = (base_score * freshness).clamp(0.0, 1.0);

        let combined_score = compute_unified_relevance(base_score, &topics, &ace_ctx);
        let relevant = combined_score >= RELEVANCE_THRESHOLD;

        let affinity_mult = compute_affinity_multiplier(&topics, &ace_ctx);
        let anti_penalty = compute_anti_penalty(&topics, &ace_ctx);

        let explanation = if relevant {
            Some(generate_relevance_explanation(
                &item.title,
                context_score,
                interest_score,
                &matches,
                &ace_ctx,
                &topics,
            ))
        } else {
            None
        };

        let confidence = calculate_confidence(
            context_score,
            interest_score,
            semantic_boost,
            &ace_ctx,
            &topics,
            cached_context_count,
            interest_count as i64,
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
            ace_boost: semantic_boost,
            affinity_mult,
            anti_penalty,
            freshness_mult: freshness,
            confidence_by_signal,
        };

        // Signal classification
        let content_text = &item.content;
        let classification = signal_classifier.classify(
            &item.title,
            content_text,
            combined_score,
            &ace_ctx.detected_tech,
        );

        let (sig_type, sig_priority, sig_action, sig_triggers) = match classification {
            Some(c) => (
                Some(c.signal_type.slug().to_string()),
                Some(c.priority.label().to_string()),
                Some(c.action),
                Some(c.triggers),
            ),
            None => (None, None, None, None),
        };

        results.push(HNRelevance {
            id: item.id as u64,
            title: item.title.clone(),
            url: item.url.clone(),
            top_score: combined_score,
            matches,
            relevant,
            context_score,
            interest_score,
            excluded: false,
            excluded_by: None,
            source_type: item.source_type.clone(),
            explanation,
            confidence: Some(confidence),
            score_breakdown: Some(score_breakdown),
            signal_type: sig_type,
            signal_priority: sig_priority,
            signal_action: sig_action,
            signal_triggers: sig_triggers,
        });
    }

    // Sort by relevance
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

    emit_progress(
        app,
        "complete",
        1.0,
        &format!("Analyzed {} cached items!", total_cached),
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    info!(target: "4da::analysis", "=== CACHE-FIRST ANALYSIS COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = excluded_count,
        "Cache analysis summary"
    );

    Ok(results)
}

/// Get current analysis state
#[tauri::command]
async fn get_analysis_status() -> Result<AnalysisState, String> {
    let state = get_analysis_state();
    let guard = state.lock();
    Ok(guard.clone())
}

/// Get actionable signals from the latest analysis
/// Filters to items with signal classifications, sorted by priority
#[tauri::command]
async fn get_actionable_signals(
    priority_filter: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = get_analysis_state();
    let guard = state.lock();

    let results = match &guard.results {
        Some(r) => r,
        None => return Ok(serde_json::json!({ "signals": [], "total": 0 })),
    };

    let priority_order = |p: &str| -> u8 {
        match p {
            "critical" => 4,
            "high" => 3,
            "medium" => 2,
            "low" => 1,
            _ => 0,
        }
    };

    let mut signals: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| r.signal_type.is_some())
        .filter(|r| {
            if let Some(ref filter) = priority_filter {
                r.signal_priority.as_deref() == Some(filter.as_str())
            } else {
                true
            }
        })
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "url": r.url,
                "score": r.top_score,
                "source_type": r.source_type,
                "signal_type": r.signal_type,
                "signal_priority": r.signal_priority,
                "signal_action": r.signal_action,
                "signal_triggers": r.signal_triggers,
            })
        })
        .collect();

    // Sort by priority (critical first), then by score
    signals.sort_by(|a, b| {
        let pa = priority_order(a["signal_priority"].as_str().unwrap_or(""));
        let pb = priority_order(b["signal_priority"].as_str().unwrap_or(""));
        pb.cmp(&pa).then_with(|| {
            let sa = a["score"].as_f64().unwrap_or(0.0);
            let sb = b["score"].as_f64().unwrap_or(0.0);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    let total = signals.len();
    Ok(serde_json::json!({
        "signals": signals,
        "total": total,
    }))
}

// ============================================================================
// Settings Commands
// ============================================================================

/// Get current settings
#[tauri::command]
async fn get_settings() -> Result<serde_json::Value, String> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let settings = guard.get();

    Ok(serde_json::json!({
        "llm": {
            "provider": settings.llm.provider,
            "model": settings.llm.model,
            "has_api_key": !settings.llm.api_key.is_empty(),
            "base_url": settings.llm.base_url
        },
        "rerank": {
            "enabled": settings.rerank.enabled,
            "max_items_per_batch": settings.rerank.max_items_per_batch,
            "min_embedding_score": settings.rerank.min_embedding_score,
            "daily_token_limit": settings.rerank.daily_token_limit,
            "daily_cost_limit_cents": settings.rerank.daily_cost_limit_cents
        },
        "usage": {
            "tokens_today": settings.usage.tokens_today,
            "cost_today_cents": settings.usage.cost_today_cents,
            "tokens_total": settings.usage.tokens_total,
            "items_reranked": settings.usage.items_reranked
        },
        "embedding_threshold": settings.embedding_threshold
    }))
}

/// Update LLM provider settings
#[tauri::command]
async fn set_llm_provider(
    provider: String,
    api_key: String,
    model: String,
    base_url: Option<String>,
    openai_api_key: Option<String>,
) -> Result<(), String> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();

    let llm_provider = LLMProvider {
        provider,
        api_key,
        model,
        base_url,
        openai_api_key: openai_api_key.unwrap_or_default(),
    };

    guard.set_llm_provider(llm_provider)?;
    info!(target: "4da::settings", "LLM provider updated");
    Ok(())
}

/// Mark onboarding wizard as complete
#[tauri::command]
async fn mark_onboarding_complete() -> Result<(), String> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    guard.mark_onboarding_complete()?;
    info!(target: "4da::settings", "Onboarding marked complete");
    Ok(())
}

/// Update re-ranking configuration
#[tauri::command]
async fn set_rerank_config(
    enabled: bool,
    max_items: usize,
    min_score: f32,
    daily_token_limit: u64,
    daily_cost_limit: u64,
) -> Result<(), String> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();

    let config = RerankConfig {
        enabled,
        max_items_per_batch: max_items,
        min_embedding_score: min_score,
        daily_token_limit,
        daily_cost_limit_cents: daily_cost_limit,
    };

    guard.set_rerank_config(config)?;
    info!(target: "4da::settings", enabled = enabled, "Re-rank config updated");
    Ok(())
}

/// Test LLM connection
#[tauri::command]
async fn test_llm_connection() -> Result<serde_json::Value, String> {
    let manager = get_settings_manager();
    let settings = {
        let guard = manager.lock();
        guard.get().clone()
    };

    // Ollama doesn't need an API key
    if settings.llm.provider == "none"
        || (settings.llm.provider != "ollama" && settings.llm.api_key.is_empty())
    {
        return Err("No LLM provider configured".to_string());
    }

    info!(target: "4da::llm", provider = %settings.llm.provider, "Testing LLM connection");

    let judge = RelevanceJudge::new(settings.llm.clone());

    // Simple test - ask for a short response
    let test_items = vec![(
        "test".to_string(),
        "Test Item".to_string(),
        "This is a test.".to_string(),
    )];

    match judge
        .judge_batch("User is testing the connection.", test_items)
        .await
    {
        Ok((_, input_tokens, output_tokens)) => {
            let cost = judge.estimate_cost_cents(input_tokens, output_tokens);
            info!(target: "4da::llm", input_tokens = input_tokens, output_tokens = output_tokens, cost_cents = cost, "LLM test successful");

            Ok(serde_json::json!({
                "success": true,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "cost_cents": cost,
                "message": format!("Connection successful! Test used {} tokens.", input_tokens + output_tokens)
            }))
        }
        Err(e) => {
            warn!(target: "4da::llm", error = %e, "LLM test failed");
            Err(format!("Connection failed: {}", e))
        }
    }
}

/// Check Ollama status and list available models
#[tauri::command]
async fn check_ollama_status(base_url: Option<String>) -> Result<serde_json::Value, String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Check if Ollama is running by hitting the version endpoint
    let version_url = format!("{}/api/version", url);
    let version_result = client.get(&version_url).send().await;

    match version_result {
        Ok(response) if response.status().is_success() => {
            let version_data: serde_json::Value = response
                .json()
                .await
                .unwrap_or(serde_json::json!({"version": "unknown"}));
            let version = version_data["version"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();

            // Fetch available models
            let tags_url = format!("{}/api/tags", url);
            let models = match client.get(&tags_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let tags_data: serde_json::Value = resp
                        .json()
                        .await
                        .unwrap_or(serde_json::json!({"models": []}));
                    tags_data["models"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| m["name"].as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                }
                _ => vec![],
            };

            info!(target: "4da::ollama", version = %version, models = ?models, "Ollama detected");

            Ok(serde_json::json!({
                "running": true,
                "version": version,
                "models": models,
                "base_url": url
            }))
        }
        Ok(response) => {
            let status = response.status();
            Err(format!("Ollama returned error status: {}", status))
        }
        Err(e) => {
            // Connection refused or timeout - Ollama not running
            info!(target: "4da::ollama", error = %e, "Ollama not detected");
            Ok(serde_json::json!({
                "running": false,
                "version": null,
                "models": [],
                "base_url": url,
                "error": format!("Ollama not running: {}", e)
            }))
        }
    }
}

/// Get usage statistics
#[tauri::command]
async fn get_usage_stats() -> Result<serde_json::Value, String> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    let within_limits = guard.within_daily_limits();
    let summary = guard.usage_summary();
    let settings = guard.get();

    Ok(serde_json::json!({
        "tokens_today": settings.usage.tokens_today,
        "cost_today_cents": settings.usage.cost_today_cents,
        "tokens_total": settings.usage.tokens_total,
        "items_reranked": settings.usage.items_reranked,
        "daily_token_limit": settings.rerank.daily_token_limit,
        "daily_cost_limit_cents": settings.rerank.daily_cost_limit_cents,
        "within_limits": within_limits,
        "summary": summary
    }))
}

// ============================================================================
// Monitoring Commands (Phase 3)
// ============================================================================

/// Get monitoring status
#[tauri::command]
async fn get_monitoring_status() -> Result<serde_json::Value, String> {
    let state = get_monitoring_state();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let last_check = state.last_check.load(std::sync::atomic::Ordering::Relaxed);
    let secs_since_check = if last_check > 0 { now - last_check } else { 0 };

    Ok(serde_json::json!({
        "enabled": state.is_enabled(),
        "interval_secs": state.get_interval(),
        "interval_mins": state.get_interval() / 60,
        "is_checking": state.is_checking.load(std::sync::atomic::Ordering::Relaxed),
        "last_check": last_check,
        "secs_since_check": secs_since_check,
        "last_relevant_count": state.last_relevant_count.load(std::sync::atomic::Ordering::Relaxed),
        "total_checks": state.total_checks.load(std::sync::atomic::Ordering::Relaxed)
    }))
}

/// Enable or disable monitoring
#[tauri::command]
async fn set_monitoring_enabled(enabled: bool) -> Result<serde_json::Value, String> {
    let state = get_monitoring_state();
    state.set_enabled(enabled);

    if enabled {
        // Set last_check to 0 to trigger immediate check
        state
            .last_check
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    // Persist to settings
    {
        let mut settings = get_settings_manager().lock();
        let interval = state.get_interval() / 60;
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled,
            interval_minutes: interval,
        });
    }

    info!(target: "4da::monitor", enabled = enabled, "Monitoring state persisted");

    Ok(serde_json::json!({
        "enabled": enabled,
        "message": if enabled { "Monitoring started" } else { "Monitoring stopped" }
    }))
}

/// Set monitoring interval
#[tauri::command]
async fn set_monitoring_interval(minutes: u64) -> Result<serde_json::Value, String> {
    let state = get_monitoring_state();
    let secs = minutes * 60;
    state.set_interval(secs);

    // Persist to settings
    {
        let mut settings = get_settings_manager().lock();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled: state.is_enabled(),
            interval_minutes: minutes,
        });
    }

    info!(target: "4da::monitor", interval_mins = minutes, "Interval persisted");

    Ok(serde_json::json!({
        "interval_mins": minutes,
        "interval_secs": secs
    }))
}

/// Test notification delivery
#[tauri::command]
async fn trigger_notification_test(app: AppHandle) -> Result<serde_json::Value, String> {
    monitoring::send_notification(&app, 3, 30);
    Ok(serde_json::json!({
        "success": true,
        "message": "Test notification sent"
    }))
}

// ============================================================================
// Context Engine Commands
// ============================================================================

/// Get the user's static identity (interests, exclusions, role, etc.)
#[tauri::command]
async fn get_user_context() -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;

    let identity = engine
        .get_static_identity()
        .map_err(|e| format!("Failed to get identity: {}", e))?;

    let interest_count = engine.interest_count().unwrap_or(0);
    let exclusion_count = engine.exclusion_count().unwrap_or(0);

    Ok(serde_json::json!({
        "role": identity.role,
        "tech_stack": identity.tech_stack,
        "domains": identity.domains,
        "interests": identity.interests.iter().map(|i| serde_json::json!({
            "id": i.id,
            "topic": i.topic,
            "weight": i.weight,
            "source": i.source,
            "has_embedding": i.embedding.is_some()
        })).collect::<Vec<_>>(),
        "exclusions": identity.exclusions,
        "stats": {
            "interest_count": interest_count,
            "exclusion_count": exclusion_count
        }
    }))
}

/// Set the user's role
#[tauri::command]
async fn set_user_role(role: Option<String>) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .set_role(role.as_deref())
        .map_err(|e| format!("Failed to set role: {}", e))?;

    info!(target: "4da::context", role = ?role, "Role updated");

    Ok(serde_json::json!({
        "success": true,
        "role": role
    }))
}

/// Add a technology to the user's tech stack
#[tauri::command]
async fn add_tech_stack(technology: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .add_technology(&technology)
        .map_err(|e| format!("Failed to add technology: {}", e))?;

    debug!(target: "4da::context", technology = %technology, "Added technology");

    Ok(serde_json::json!({
        "success": true,
        "technology": technology
    }))
}

/// Remove a technology from the user's tech stack
#[tauri::command]
async fn remove_tech_stack(technology: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .remove_technology(&technology)
        .map_err(|e| format!("Failed to remove technology: {}", e))?;

    debug!(target: "4da::context", technology = %technology, "Removed technology");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Add a domain of interest
#[tauri::command]
async fn add_domain(domain: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .add_domain(&domain)
        .map_err(|e| format!("Failed to add domain: {}", e))?;

    debug!(target: "4da::context", domain = %domain, "Added domain");

    Ok(serde_json::json!({
        "success": true,
        "domain": domain
    }))
}

/// Remove a domain of interest
#[tauri::command]
async fn remove_domain(domain: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .remove_domain(&domain)
        .map_err(|e| format!("Failed to remove domain: {}", e))?;

    debug!(target: "4da::context", domain = %domain, "Removed domain");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Add an explicit interest (with embedding generation)
#[tauri::command]
async fn add_interest(topic: String, weight: Option<f32>) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    let weight = weight.unwrap_or(1.0);

    // Generate embedding for the topic
    let embedding = embed_texts(&[topic.clone()])?;
    let emb = embedding.first().map(|e| e.as_slice());

    let id = engine
        .add_interest(&topic, weight, emb, InterestSource::Explicit)
        .map_err(|e| format!("Failed to add interest: {}", e))?;

    info!(target: "4da::context", topic = %topic, weight = weight, has_embedding = emb.is_some(), "Added interest");

    Ok(serde_json::json!({
        "success": true,
        "id": id,
        "topic": topic,
        "weight": weight,
        "has_embedding": emb.is_some()
    }))
}

/// Remove an interest
#[tauri::command]
async fn remove_interest(topic: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .remove_interest(&topic)
        .map_err(|e| format!("Failed to remove interest: {}", e))?;

    info!(target: "4da::context", topic = %topic, "Removed interest");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Add an exclusion (topic to never show)
#[tauri::command]
async fn add_exclusion(topic: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .add_exclusion(&topic)
        .map_err(|e| format!("Failed to add exclusion: {}", e))?;

    info!(target: "4da::context", topic = %topic, "Added exclusion");

    Ok(serde_json::json!({
        "success": true,
        "topic": topic
    }))
}

/// Remove an exclusion
#[tauri::command]
async fn remove_exclusion(topic: String) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;
    engine
        .remove_exclusion(&topic)
        .map_err(|e| format!("Failed to remove exclusion: {}", e))?;

    info!(target: "4da::context", topic = %topic, "Removed exclusion");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Record a user interaction (click, save, dismiss)
#[tauri::command]
async fn record_interaction(
    source_item_id: i64,
    action: String,
) -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;

    let action_type = match action.to_lowercase().as_str() {
        "click" => InteractionType::Click,
        "save" => InteractionType::Save,
        "dismiss" => InteractionType::Dismiss,
        "ignore" => InteractionType::Ignore,
        _ => return Err(format!("Unknown action type: {}", action)),
    };

    engine
        .record_interaction(source_item_id, action_type)
        .map_err(|e| format!("Failed to record interaction: {}", e))?;

    debug!(target: "4da::context", action = %action, item_id = source_item_id, "Recorded interaction");

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Get context engine statistics
#[tauri::command]
async fn get_context_stats() -> Result<serde_json::Value, String> {
    let engine = get_context_engine()?;

    let interest_count = engine.interest_count().unwrap_or(0);
    let exclusion_count = engine.exclusion_count().unwrap_or(0);

    let identity = engine
        .get_static_identity()
        .map_err(|e| format!("Failed to get identity: {}", e))?;

    Ok(serde_json::json!({
        "interests": interest_count,
        "exclusions": exclusion_count,
        "tech_stack": identity.tech_stack.len(),
        "domains": identity.domains.len(),
        "has_role": identity.role.is_some()
    }))
}

// ============================================================================
// ACE (Autonomous Context Engine) Commands
// ============================================================================

/// Trigger autonomous context detection
/// Scans specified paths for project manifests and extracts tech stack
#[tauri::command]
async fn ace_detect_context(paths: Vec<String>) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;

    // Convert string paths to PathBuf, expanding ~ to home directory
    let scan_paths: Vec<PathBuf> = paths
        .iter()
        .map(|p| {
            if p.starts_with("~") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&p[2..]) // Skip "~/"
                } else {
                    PathBuf::from(p)
                }
            } else {
                PathBuf::from(p)
            }
        })
        .collect();

    // If no paths provided, use default locations
    let paths_to_scan = if scan_paths.is_empty() {
        get_default_scan_paths()
    } else {
        scan_paths
    };

    let context = ace.detect_context(&paths_to_scan)?;

    Ok(serde_json::json!({
        "success": true,
        "detected_tech": context.detected_tech,
        "active_topics": context.active_topics,
        "projects_scanned": context.projects_scanned,
        "context_confidence": context.context_confidence,
        "detection_time": context.detection_time
    }))
}

/// Get detected technologies from ACE
#[tauri::command]
async fn ace_get_detected_tech() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let tech = ace.get_detected_tech()?;

    Ok(serde_json::json!({
        "detected_tech": tech
    }))
}

/// Get active topics from ACE
#[tauri::command]
async fn ace_get_active_topics() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let topics = ace.get_active_topics()?;

    Ok(serde_json::json!({
        "topics": topics
    }))
}

/// Get ACE system health status

/// Get default paths to scan for projects
fn get_default_scan_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        // Common project locations
        let candidates = [
            "projects",
            "code",
            "dev",
            "src",
            "Documents/GitHub",
            "repos",
            "workspace",
            "work",
        ];

        for candidate in candidates {
            let path = home.join(candidate);
            if path.exists() {
                paths.push(path);
            }
        }
    }

    // Also check current working directory parent (for dev scenarios)
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(parent) = cwd.parent() {
            if !paths.contains(&parent.to_path_buf()) {
                paths.push(parent.to_path_buf());
            }
        }
    }

    paths
}

// ============================================================================
// ACE Phase B: Real-Time Context Commands
// ============================================================================

/// Analyze git repositories for context extraction
#[tauri::command]
async fn ace_analyze_git(paths: Vec<String>) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;

    let scan_paths: Vec<PathBuf> = if paths.is_empty() {
        get_default_scan_paths()
    } else {
        paths
            .iter()
            .map(|p| {
                if p.starts_with("~") {
                    if let Some(home) = dirs::home_dir() {
                        home.join(&p[2..])
                    } else {
                        PathBuf::from(p)
                    }
                } else {
                    PathBuf::from(p)
                }
            })
            .collect()
    };

    let signals = ace.analyze_git_repos(&scan_paths)?;

    Ok(serde_json::json!({
        "success": true,
        "repos_analyzed": signals.len(),
        "signals": signals.iter().map(|s| serde_json::json!({
            "repo_name": s.repo_name,
            "repo_path": s.repo_path,
            "commit_count": s.recent_commits.len(),
            "branch_count": s.active_branches.len(),
            "commit_frequency": s.commit_frequency,
            "topics": s.extracted_topics,
            "confidence": s.confidence
        })).collect::<Vec<_>>()
    }))
}

/// Get real-time context (active topics + detected tech)
#[tauri::command]
async fn ace_get_realtime_context() -> Result<serde_json::Value, String> {
    let _ace = get_ace_engine()?;

    // Get the connection from ACE (we need to access it)
    let mut db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    db_path.pop();
    db_path.push("data");
    db_path.push("4da.db");

    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;
    let conn = Arc::new(parking_lot::Mutex::new(conn));

    let context = ace::get_realtime_context(&conn)?;

    Ok(serde_json::json!({
        "active_topics": context.active_topics,
        "detected_tech": context.detected_tech,
        "context_confidence": context.context_confidence,
        "last_updated": context.last_updated
    }))
}

/// Apply freshness decay to active topics
#[tauri::command]
async fn ace_apply_decay() -> Result<serde_json::Value, String> {
    let mut db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    db_path.pop();
    db_path.push("data");
    db_path.push("4da.db");

    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;
    let conn = Arc::new(parking_lot::Mutex::new(conn));

    let updated = ace::apply_freshness_decay(&conn)?;

    Ok(serde_json::json!({
        "success": true,
        "topics_updated": updated
    }))
}

/// Run full autonomous context detection (manifests + git)
#[tauri::command]
async fn ace_full_scan(paths: Vec<String>) -> Result<serde_json::Value, String> {
    let scan_paths: Vec<PathBuf> = if paths.is_empty() {
        get_default_scan_paths()
    } else {
        paths
            .iter()
            .map(|p| {
                if p.starts_with("~") {
                    if let Some(home) = dirs::home_dir() {
                        home.join(&p[2..])
                    } else {
                        PathBuf::from(p)
                    }
                } else {
                    PathBuf::from(p)
                }
            })
            .collect()
    };

    info!(target: "4da::ace", paths = scan_paths.len(), "Starting full scan");

    // Phase 1 & 2: Manifest scanning and Git analysis (scoped to release ACE lock)
    let (manifest_context, git_signals) = {
        let ace = get_ace_engine()?;
        let manifest_context = ace.detect_context(&scan_paths)?;
        let git_signals = ace.analyze_git_repos(&scan_paths)?;
        (manifest_context, git_signals)
    }; // ACE lock is dropped here

    // Phase 3: README indexing (PASIFA - semantic context from discovered projects)
    // This makes ACE discovery contribute to semantic matching, not just keyword boost
    debug!(target: "4da::ace", "Indexing README files for semantic search");
    let readme_chunks_indexed = index_discovered_readmes(&scan_paths).await;
    if readme_chunks_indexed > 0 {
        info!(target: "4da::ace", chunks = readme_chunks_indexed, "Indexed README files for semantic context");
    }

    // Combine results
    let total_topics: std::collections::HashSet<String> = manifest_context
        .active_topics
        .iter()
        .map(|t| t.topic.clone())
        .chain(git_signals.iter().flat_map(|s| s.extracted_topics.clone()))
        .collect();

    info!(target: "4da::ace",
        tech = manifest_context.detected_tech.len(),
        topics = total_topics.len(),
        git_repos = git_signals.len(),
        readme_chunks = readme_chunks_indexed,
        "Full scan complete"
    );

    Ok(serde_json::json!({
        "success": true,
        "manifest_scan": {
            "projects_scanned": manifest_context.projects_scanned,
            "detected_tech": manifest_context.detected_tech.len(),
            "confidence": manifest_context.context_confidence
        },
        "git_scan": {
            "repos_analyzed": git_signals.len(),
            "total_commits": git_signals.iter().map(|s| s.recent_commits.len()).sum::<usize>()
        },
        "readme_index": {
            "chunks_indexed": readme_chunks_indexed
        },
        "combined": {
            "total_topics": total_topics.len(),
            "topics": total_topics.into_iter().collect::<Vec<_>>()
        }
    }))
}

/// Trigger autonomous context discovery - finds dev directories and projects automatically
/// This is the "just make it work" button - discovers context without user configuration
#[tauri::command]
async fn ace_auto_discover() -> Result<serde_json::Value, String> {
    info!(target: "4da::ace", "Starting autonomous context discovery");

    // Phase 1: Discover common dev directories
    let discovered_dirs = crate::settings::discover_dev_directories();

    if discovered_dirs.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "message": "No development directories found on this system",
            "directories_found": 0,
            "projects_found": 0
        }));
    }

    info!(target: "4da::ace", dirs = discovered_dirs.len(), "Found potential dev directories");

    // Phase 2: Deep scan for actual project directories
    let project_dirs = crate::settings::find_project_directories(&discovered_dirs, 3);

    // Decide what to add - parent dirs or individual projects
    let dirs_to_add = if project_dirs.len() > 50 {
        debug!(target: "4da::ace", projects = project_dirs.len(), "Too many projects, using parent directories");
        discovered_dirs.clone()
    } else if !project_dirs.is_empty() {
        debug!(target: "4da::ace", projects = project_dirs.len(), "Found specific projects");
        project_dirs.clone()
    } else {
        debug!(target: "4da::ace", "No specific projects found, using dev directories");
        discovered_dirs.clone()
    };

    // Save to settings
    {
        let mut settings = get_settings_manager().lock();
        if let Err(e) = settings.add_context_dirs(dirs_to_add.clone()) {
            return Err(format!("Failed to save discovered directories: {}", e));
        }
        let _ = settings.mark_auto_discovery_completed();
    }

    // Now run full ACE scan on discovered directories
    info!(target: "4da::ace", dirs = dirs_to_add.len(), "Running full scan on directories");
    let scan_result = ace_full_scan(dirs_to_add.clone()).await?;

    Ok(serde_json::json!({
        "success": true,
        "directories_found": discovered_dirs.len(),
        "projects_found": project_dirs.len(),
        "directories_added": dirs_to_add.len(),
        "directories": dirs_to_add,
        "scan_result": scan_result
    }))
}

/// Reset auto-discovery flag to allow re-discovery
#[tauri::command]
async fn ace_reset_discovery() -> Result<serde_json::Value, String> {
    let mut settings = get_settings_manager().lock();
    settings.get_mut().auto_discovery_completed = false;
    settings.save()?;

    Ok(serde_json::json!({
        "success": true,
        "message": "Auto-discovery reset. Next startup will re-discover directories."
    }))
}

/// Get current context directories and discovery status
#[tauri::command]
async fn ace_get_discovery_status() -> Result<serde_json::Value, String> {
    let settings = get_settings_manager().lock();
    let context_dirs = settings.get().context_dirs.clone();
    let auto_discovery_completed = settings.get().auto_discovery_completed;

    Ok(serde_json::json!({
        "auto_discovery_completed": auto_discovery_completed,
        "context_dirs": context_dirs,
        "context_dirs_count": context_dirs.len()
    }))
}

// ============================================================================
// ACE Phase C: Behavior Learning Commands
// ============================================================================

/// Record a user interaction for behavior learning
#[tauri::command]
async fn ace_record_interaction(
    item_id: i64,
    action_type: String,
    action_data: Option<serde_json::Value>,
    item_topics: Vec<String>,
    item_source: String,
) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;

    // Parse action type into BehaviorAction
    let action = match action_type.as_str() {
        "click" => {
            let dwell_time = action_data
                .and_then(|d| d.get("dwell_time_seconds").and_then(|v| v.as_u64()))
                .unwrap_or(0);
            ace::BehaviorAction::Click {
                dwell_time_seconds: dwell_time,
            }
        }
        "save" => ace::BehaviorAction::Save,
        "share" => ace::BehaviorAction::Share,
        "dismiss" => ace::BehaviorAction::Dismiss,
        "mark_irrelevant" => ace::BehaviorAction::MarkIrrelevant,
        "scroll" => {
            let visible_seconds = action_data
                .and_then(|d| d.get("visible_seconds").and_then(|v| v.as_f64()))
                .unwrap_or(0.0) as f32;
            ace::BehaviorAction::Scroll { visible_seconds }
        }
        "ignore" => ace::BehaviorAction::Ignore,
        _ => return Err(format!("Unknown action type: {}", action_type)),
    };

    ace.record_interaction(
        item_id,
        action.clone(),
        item_topics.clone(),
        item_source.clone(),
    )?;

    Ok(serde_json::json!({
        "success": true,
        "recorded": {
            "item_id": item_id,
            "action": action_type,
            "topics": item_topics,
            "source": item_source
        }
    }))
}

/// Get learned topic affinities
#[tauri::command]
async fn ace_get_topic_affinities() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let affinities = ace.get_topic_affinities()?;

    Ok(serde_json::json!({
        "affinities": affinities,
        "count": affinities.len()
    }))
}

/// Get detected anti-topics
#[tauri::command]
async fn ace_get_anti_topics(min_rejections: Option<u32>) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let threshold = min_rejections.unwrap_or(5);
    let anti_topics = ace.get_anti_topics(threshold)?;

    Ok(serde_json::json!({
        "anti_topics": anti_topics,
        "count": anti_topics.len(),
        "threshold": threshold
    }))
}

/// Confirm an auto-detected anti-topic
#[tauri::command]
async fn ace_confirm_anti_topic(topic: String) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    ace.confirm_anti_topic(&topic)?;

    Ok(serde_json::json!({
        "success": true,
        "confirmed": topic
    }))
}

/// Get behavior modifier for an item
#[tauri::command]
async fn ace_get_behavior_modifier(
    topics: Vec<String>,
    source: String,
) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let modifier = ace.get_behavior_modifier(&topics, &source)?;

    Ok(serde_json::json!({
        "modifier": modifier,
        "topics": topics,
        "source": source
    }))
}

/// Get full learned behavior summary
#[tauri::command]
async fn ace_get_learned_behavior() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let summary = ace.get_learned_behavior()?;

    Ok(serde_json::json!(summary))
}

/// Apply temporal decay to behavior learning
#[tauri::command]
async fn ace_apply_behavior_decay() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let updated = ace.apply_behavior_decay()?;

    Ok(serde_json::json!({
        "success": true,
        "affinities_updated": updated
    }))
}

// ============================================================================
// ACE Phase E: Embedding Commands
// ============================================================================

/// Get embedding for a topic
#[tauri::command]
async fn ace_embed_topic(topic: String) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let embedding = ace.embed_topic(&topic)?;

    Ok(serde_json::json!({
        "topic": topic,
        "embedding": embedding,
        "dimension": embedding.len()
    }))
}

/// Find similar topics using embeddings
#[tauri::command]
async fn ace_find_similar_topics(
    query: String,
    top_k: Option<usize>,
) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let top_k = top_k.unwrap_or(5);
    let results = ace.find_similar_topics(&query, top_k)?;

    Ok(serde_json::json!({
        "query": query,
        "results": results.iter().map(|(topic, score)| {
            serde_json::json!({
                "topic": topic,
                "similarity": score
            })
        }).collect::<Vec<_>>()
    }))
}

/// Check if embedding service is operational
#[tauri::command]
async fn ace_embedding_status() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let operational = ace.is_embedding_operational();

    Ok(serde_json::json!({
        "operational": operational
    }))
}

// ============================================================================
// ACE Phase E: Watcher Persistence Commands
// ============================================================================

/// Save watcher state for persistence
#[tauri::command]
async fn ace_save_watcher_state() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    ace.save_watcher_state()?;

    Ok(serde_json::json!({
        "saved": true
    }))
}

/// Restore watcher state from persistence
/// Note: This returns the saved state info. Actual restoration happens on app restart.
#[tauri::command]
async fn ace_get_watcher_state() -> Result<serde_json::Value, String> {
    // Watcher state is restored automatically on ACE initialization
    // This command provides info about the current state
    Ok(serde_json::json!({
        "info": "Watcher state is restored automatically on app startup. Use ace_save_watcher_state to persist current state."
    }))
}

/// Clear watcher state
#[tauri::command]
async fn ace_clear_watcher_state() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    ace.clear_watcher_state()?;

    Ok(serde_json::json!({
        "cleared": true
    }))
}

// ============================================================================
// ACE Phase E: Rate Limiting Commands
// ============================================================================

/// Get rate limit status for a source
#[tauri::command]
async fn ace_get_rate_limit_status(source: String) -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let status = ace.get_rate_limit_status(&source);

    Ok(serde_json::json!(status))
}

// ============================================================================
// ACE Watcher Control Commands
// ============================================================================

/// Start file watching on specified directories
#[tauri::command]
async fn ace_start_watcher(paths: Vec<String>) -> Result<serde_json::Value, String> {
    let mut ace = get_ace_engine_mut()?;

    let watch_paths: Vec<PathBuf> = paths
        .iter()
        .map(|p| {
            if p.starts_with("~") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&p[2..])
                } else {
                    PathBuf::from(p)
                }
            } else {
                PathBuf::from(p)
            }
        })
        .filter(|p| p.exists())
        .collect();

    if watch_paths.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "error": "No valid paths to watch",
            "watching": 0
        }));
    }

    ace.start_watching(&watch_paths)?;

    Ok(serde_json::json!({
        "success": true,
        "watching": watch_paths.len(),
        "paths": watch_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
    }))
}

/// Stop file watching
#[tauri::command]
async fn ace_stop_watcher() -> Result<serde_json::Value, String> {
    let mut ace = get_ace_engine_mut()?;
    ace.stop_watching();

    Ok(serde_json::json!({
        "success": true,
        "watching": false
    }))
}

/// Check if watcher is active
#[tauri::command]
async fn ace_is_watching() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let watching = ace.is_watching();

    Ok(serde_json::json!({
        "watching": watching
    }))
}

// ============================================================================
// PASIFA: Discovered Context Indexing
// ============================================================================

/// Check if directory contains a project manifest
fn has_manifest(dir: &PathBuf) -> bool {
    let manifests = [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "composer.json",
        "Gemfile",
        "pom.xml",
        "build.gradle",
        "CMakeLists.txt",
        "pubspec.yaml",
    ];

    for manifest in &manifests {
        if dir.join(manifest).exists() {
            return true;
        }
    }

    // Check for .csproj files
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext == "csproj" {
                    return true;
                }
            }
        }
    }

    false
}

/// Recursively discover project directories by finding manifests
/// Stops recursing when a manifest is found (don't nest into projects)
fn discover_projects_recursive(
    root: &PathBuf,
    max_depth: usize,
    skip_dirs: &[&str],
) -> Vec<PathBuf> {
    fn walk(
        dir: &PathBuf,
        depth: usize,
        max_depth: usize,
        skip_dirs: &[&str],
        projects: &mut Vec<PathBuf>,
    ) {
        if depth > max_depth {
            return;
        }

        // Check if this directory is a project
        if has_manifest(dir) {
            projects.push(dir.clone());
            return; // Stop recursing - we found a project
        }

        // Recurse into subdirectories
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if !path.is_dir() {
                    continue;
                }

                // Skip excluded directories
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if skip_dirs.contains(&name) {
                        continue;
                    }
                }

                walk(&path, depth + 1, max_depth, skip_dirs, projects);
            }
        }
    }

    let mut projects = Vec::new();
    walk(root, 0, max_depth, skip_dirs, &mut projects);
    projects
}

/// Parse README into sections with headings
#[derive(Debug)]
struct ReadmeSection {
    heading: String,
    content: String,
    #[allow(dead_code)] // Kept for future section hierarchy processing
    level: usize,
}

fn parse_readme_sections(content: &str) -> Vec<ReadmeSection> {
    let mut sections = Vec::new();
    let mut current_heading = String::from("Overview");
    let mut current_content = String::new();
    let mut current_level = 1;

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for markdown heading
        if trimmed.starts_with('#') {
            // Save previous section if it has content
            if !current_content.trim().is_empty() {
                sections.push(ReadmeSection {
                    heading: current_heading.clone(),
                    content: current_content.trim().to_string(),
                    level: current_level,
                });
                current_content.clear();
            }

            // Parse new heading
            let level = trimmed.chars().take_while(|c| *c == '#').count();
            let heading_text = trimmed.trim_start_matches('#').trim();

            if !heading_text.is_empty() {
                current_heading = heading_text.to_string();
                current_level = level;
            }
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Add final section
    if !current_content.trim().is_empty() {
        sections.push(ReadmeSection {
            heading: current_heading,
            content: current_content.trim().to_string(),
            level: current_level,
        });
    }

    sections
}

/// Determine weight for a README section based on heading
fn section_weight(heading: &str) -> f32 {
    let heading_lower = heading.to_lowercase();

    // High value sections
    if heading_lower.contains("feature")
        || heading_lower.contains("overview")
        || heading_lower.contains("about")
    {
        return 1.0;
    }

    // API and usage documentation
    if heading_lower.contains("api")
        || heading_lower.contains("usage")
        || heading_lower.contains("how to")
    {
        return 0.9;
    }

    // Architecture and design
    if heading_lower.contains("architect")
        || heading_lower.contains("design")
        || heading_lower.contains("structure")
    {
        return 0.85;
    }

    // Examples and demos
    if heading_lower.contains("example")
        || heading_lower.contains("demo")
        || heading_lower.contains("tutorial")
    {
        return 0.8;
    }

    // Installation and setup
    if heading_lower.contains("install")
        || heading_lower.contains("setup")
        || heading_lower.contains("getting started")
        || heading_lower.contains("quickstart")
    {
        return 0.7;
    }

    // Low value sections
    if heading_lower.contains("license")
        || heading_lower.contains("credit")
        || heading_lower.contains("author")
        || heading_lower.contains("contributor")
    {
        return 0.3;
    }

    // Default weight for other sections
    0.6
}

/// Index README files from discovered projects for semantic search
/// This is the bridge between ACE discovery and embedding-based relevance
/// Now with DEEP recursive project discovery and section-aware weighting
async fn index_discovered_readmes(context_dirs: &[PathBuf]) -> usize {
    info!(target: "4da::pasifa", dirs = context_dirs.len(), "Starting DEEP README indexing with recursive project discovery");

    if context_dirs.is_empty() {
        warn!(target: "4da::pasifa", "No context directories configured - cannot index READMEs");
        return 0;
    }

    let db = match get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::pasifa", error = %e, "Database not available");
            return 0;
        }
    };

    // Directories to skip during recursive scan
    let skip_dirs = [
        "node_modules",
        "target",
        ".git",
        "dist",
        "build",
        ".next",
        "__pycache__",
        ".venv",
        "venv",
        "vendor",
        ".cargo",
        "pkg",
    ];

    // Discover all projects recursively (max depth 3)
    let mut all_projects = Vec::new();
    for dir in context_dirs {
        if !dir.exists() {
            warn!(target: "4da::pasifa", dir = %dir.display(), "Context directory does not exist");
            continue;
        }

        let discovered = discover_projects_recursive(dir, 3, &skip_dirs);
        debug!(target: "4da::pasifa", dir = %dir.display(), projects = discovered.len(), "Discovered projects recursively");
        all_projects.extend(discovered);
    }

    info!(target: "4da::pasifa", total_projects = all_projects.len(), "Completed recursive project discovery");

    let mut indexed_chunks = 0;
    let mut found_readme_count = 0;
    let mut section_count = 0;
    let readme_names = ["README.md", "README.txt", "README", "readme.md"];
    let total_projects = all_projects.len();

    // Process each discovered project
    for project_dir in &all_projects {
        // Find README in this project
        let mut readme_found = false;
        for readme_name in &readme_names {
            let readme_path = project_dir.join(readme_name);
            if readme_path.exists() && readme_path.is_file() {
                found_readme_count += 1;
                readme_found = true;
                debug!(target: "4da::pasifa", path = %readme_path.display(), "Found README file");

                match std::fs::read_to_string(&readme_path) {
                    Ok(content) => {
                        if content.len() < 100 {
                            debug!(target: "4da::pasifa", path = %readme_path.display(), len = content.len(), "README too short, skipping");
                            continue;
                        }

                        // Parse README into sections
                        let sections = parse_readme_sections(&content);
                        let num_sections = sections.len();
                        section_count += num_sections;
                        debug!(target: "4da::pasifa", path = %readme_path.display(), sections = num_sections, "Parsed README sections");

                        // Process each section with appropriate weight
                        for section in &sections {
                            let weight = section_weight(&section.heading);

                            // Skip very short sections
                            if section.content.len() < 50 {
                                continue;
                            }

                            // Chunk the section content
                            let source_info =
                                format!("{}#{}", readme_path.to_string_lossy(), section.heading);
                            let chunks = chunk_text(&section.content, &source_info);

                            for (chunk_source, chunk_content) in chunks {
                                if chunk_content.len() < 50 {
                                    continue;
                                }

                                // Generate embedding
                                match embed_texts(&[chunk_content.clone()]) {
                                    Ok(embeddings) if !embeddings.is_empty() => {
                                        // Store with weight in context_chunks table
                                        match db.upsert_context_weighted(
                                            &chunk_source,
                                            &chunk_content,
                                            &embeddings[0],
                                            weight,
                                        ) {
                                            Ok(_) => {
                                                indexed_chunks += 1;
                                                debug!(target: "4da::pasifa",
                                                    section = &section.heading,
                                                    weight = weight,
                                                    "Indexed weighted section chunk"
                                                );
                                            }
                                            Err(e) => {
                                                warn!(target: "4da::pasifa",
                                                    path = %readme_path.display(),
                                                    section = &section.heading,
                                                    error = %e,
                                                    "Failed to upsert weighted context"
                                                );
                                            }
                                        }
                                    }
                                    Ok(_) => {
                                        debug!(target: "4da::pasifa", "Embedding returned empty result");
                                    }
                                    Err(e) => {
                                        warn!(target: "4da::pasifa",
                                            path = %readme_path.display(),
                                            section = &section.heading,
                                            error = %e,
                                            "Failed to embed - check API key configuration"
                                        );
                                    }
                                }
                            }
                        }

                        info!(target: "4da::pasifa",
                            path = %readme_path.display(),
                            sections = sections.len(),
                            chunks = indexed_chunks,
                            "Indexed README with section weighting"
                        );
                        break; // Only index first README found per project
                    }
                    Err(e) => {
                        debug!(target: "4da::pasifa", path = %readme_path.display(), error = %e, "Failed to read");
                    }
                }
            }
        }

        if !readme_found {
            debug!(target: "4da::pasifa", project = %project_dir.display(), "No README found in project");
        }
    }

    if found_readme_count == 0 {
        info!(target: "4da::pasifa", "No README files found in discovered projects");
    } else if indexed_chunks == 0 {
        warn!(target: "4da::pasifa", found = found_readme_count, "Found READMEs but failed to index - check embedding API key");
    } else {
        info!(target: "4da::pasifa",
            projects = total_projects,
            readmes = found_readme_count,
            sections = section_count,
            chunks = indexed_chunks,
            "DEEP README indexing complete with section weighting"
        );
    }

    indexed_chunks
}

// ============================================================================
// Auto-Seed Interests from ACE Context
// ============================================================================

/// Automatically seed user interests from ACE-detected technologies
/// This runs once at startup when interests are empty, providing immediate value
/// without requiring manual configuration.
async fn auto_seed_interests_from_ace() -> Result<(), String> {
    // Check if interests are already configured
    let context_engine = get_context_engine()?;
    let existing_interests = context_engine
        .get_interests()
        .map_err(|e| format!("Failed to get interests: {}", e))?;

    if !existing_interests.is_empty() {
        debug!(target: "4da::startup", count = existing_interests.len(), "Interests already configured, skipping auto-seed");
        return Ok(());
    }

    // Get ACE-detected technologies
    let ace_ctx = get_ace_context();
    if ace_ctx.detected_tech.is_empty() && ace_ctx.active_topics.is_empty() {
        debug!(target: "4da::startup", "No ACE context available for auto-seeding");
        return Ok(());
    }

    info!(target: "4da::startup", tech_count = ace_ctx.detected_tech.len(), topic_count = ace_ctx.active_topics.len(), "Auto-seeding interests from ACE context");

    // Collect high-value topics to seed (languages, frameworks with high confidence)
    let mut topics_to_seed: Vec<(String, f32)> = Vec::new();

    // Add detected tech (languages, frameworks) with weight 0.8
    for tech in &ace_ctx.detected_tech {
        // Skip very generic or noisy tech
        let skip_list = [
            "npm", "yarn", "pnpm", "node", "git", "json", "yaml", "toml", "markdown",
        ];
        if !skip_list.contains(&tech.as_str()) {
            topics_to_seed.push((tech.clone(), 0.8));
        }
    }

    // Add high-confidence active topics with weight 0.7
    for topic in &ace_ctx.active_topics {
        let confidence = ace_ctx.topic_confidence.get(topic).copied().unwrap_or(0.5);
        // Only add topics with good confidence that aren't already in tech
        if confidence >= 0.7 && !ace_ctx.detected_tech.contains(topic) {
            // Skip commit-type patterns and generic terms
            if !topic.starts_with("commit-") && topic.len() > 2 {
                topics_to_seed.push((topic.clone(), 0.7));
            }
        }
    }

    if topics_to_seed.is_empty() {
        debug!(target: "4da::startup", "No suitable topics for auto-seeding");
        return Ok(());
    }

    // Limit to top 15 to avoid over-seeding
    topics_to_seed.truncate(15);

    // Generate embeddings for all topics at once
    let topic_strings: Vec<String> = topics_to_seed.iter().map(|(t, _)| t.clone()).collect();
    let embeddings = embed_texts(&topic_strings)?;

    // Add each topic as an inferred interest
    let mut seeded_count = 0;
    for ((topic, weight), embedding) in topics_to_seed.iter().zip(embeddings.iter()) {
        match context_engine.add_interest(
            topic,
            *weight,
            Some(embedding.as_slice()),
            InterestSource::Inferred,
        ) {
            Ok(_) => {
                seeded_count += 1;
                debug!(target: "4da::startup", topic = %topic, weight = weight, "Auto-seeded interest");
            }
            Err(e) => {
                warn!(target: "4da::startup", topic = %topic, error = %e, "Failed to seed interest");
            }
        }
    }

    info!(target: "4da::startup", count = seeded_count, "Auto-seeded interests from ACE context");
    Ok(())
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

        match ace_full_scan(paths.clone()).await {
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
        if let Err(e) = auto_seed_interests_from_ace().await {
            warn!(target: "4da::startup", error = %e, "Auto-seeding interests failed (non-fatal)");
        }

        // PASIFA: Index README files from discovered projects for semantic search
        // This makes discovered context contribute to embedding-based relevance
        debug!(target: "4da::startup", "Indexing README files from discovered projects");
        let indexed_count = index_discovered_readmes(&context_dirs).await;
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
        match ace_start_watcher(paths).await {
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

    // Test ACEContext creation and defaults
    #[test]
    fn test_ace_context_default() {
        let ctx = ACEContext::default();
        assert!(ctx.active_topics.is_empty());
        assert!(ctx.detected_tech.is_empty());
        assert!(ctx.anti_topics.is_empty());
        assert!(ctx.topic_affinities.is_empty());
    }

    // Test affinity multiplier with empty context
    #[test]
    fn test_affinity_multiplier_empty_context() {
        let ctx = ACEContext::default();
        let topics = vec!["rust".to_string(), "tauri".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);
        assert_eq!(mult, 1.0, "Empty context should return neutral multiplier");
    }

    // Test affinity multiplier with positive affinity
    #[test]
    fn test_affinity_multiplier_positive() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 0.9)); // High affinity, high confidence

        let topics = vec!["rust".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);

        // 0.8 * 0.9 = 0.72 weighted affinity
        // 1.0 + 0.72 * 0.7 = 1.504
        assert!(mult > 1.0, "Positive affinity should boost multiplier");
        assert!(mult <= 1.7, "Multiplier should be capped at 1.7");
    }

    // Test affinity multiplier with negative affinity
    #[test]
    fn test_affinity_multiplier_negative() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities
            .insert("crypto".to_string(), (-0.9, 0.8)); // Strong dislike, high confidence

        let topics = vec!["crypto".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);

        assert!(mult < 1.0, "Negative affinity should reduce multiplier");
        assert!(mult >= 0.3, "Multiplier should be capped at 0.3");
    }

    // Test anti-penalty computation
    #[test]
    fn test_anti_penalty_empty_context() {
        let ctx = ACEContext::default();
        let topics = vec!["test".to_string()];
        let penalty = compute_anti_penalty(&topics, &ctx);
        assert_eq!(penalty, 0.0, "Empty context should return zero penalty");
    }

    // Test anti-penalty with matching anti-topic
    #[test]
    fn test_anti_penalty_with_match() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("spam".to_string());
        ctx.anti_topic_confidence.insert("spam".to_string(), 0.8);

        let topics = vec!["spam".to_string()];
        let penalty = compute_anti_penalty(&topics, &ctx);

        // 0.3 * 0.8 = 0.24
        assert!(penalty > 0.0, "Matching anti-topic should produce penalty");
        assert!(penalty <= 0.7, "Penalty should be capped at 0.7");
    }

    // Test unified relevance scoring
    #[test]
    fn test_unified_relevance_neutral() {
        let ctx = ACEContext::default();
        let topics = vec!["test".to_string()];
        let score = compute_unified_relevance(0.5, &topics, &ctx);

        // With neutral context: 0.5 * 1.0 * (1.0 - 0.0) = 0.5
        assert_eq!(score, 0.5, "Neutral context should preserve base score");
    }

    // Test unified relevance with positive affinity
    #[test]
    fn test_unified_relevance_positive_affinity() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 1.0));

        let topics = vec!["rust".to_string()];
        let score = compute_unified_relevance(0.5, &topics, &ctx);

        // Base 0.5 * multiplier > 1.0 * (1.0 - 0.0)
        assert!(score > 0.5, "Positive affinity should boost score");
    }

    // Test unified relevance with anti-topic
    #[test]
    fn test_unified_relevance_anti_topic() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("spam".to_string());
        ctx.anti_topic_confidence.insert("spam".to_string(), 1.0);

        let topics = vec!["spam".to_string()];
        let score = compute_unified_relevance(0.5, &topics, &ctx);

        // Base 0.5 * 1.0 * (1.0 - penalty)
        assert!(score < 0.5, "Anti-topic should reduce score");
    }

    // Test confidence weighting effect
    #[test]
    fn test_confidence_weighting() {
        let mut ctx_high_conf = ACEContext::default();
        ctx_high_conf
            .topic_affinities
            .insert("rust".to_string(), (0.8, 1.0));

        let mut ctx_low_conf = ACEContext::default();
        ctx_low_conf
            .topic_affinities
            .insert("rust".to_string(), (0.8, 0.3));

        let topics = vec!["rust".to_string()];

        let score_high = compute_unified_relevance(0.5, &topics, &ctx_high_conf);
        let score_low = compute_unified_relevance(0.5, &topics, &ctx_low_conf);

        assert!(
            score_high > score_low,
            "Higher confidence should produce stronger effect"
        );
    }

    // Test score clamping
    #[test]
    fn test_score_clamping() {
        let mut ctx = ACEContext::default();
        // Extreme positive affinity
        ctx.topic_affinities.insert("rust".to_string(), (1.0, 1.0));

        let topics = vec!["rust".to_string()];
        let score = compute_unified_relevance(1.0, &topics, &ctx);

        assert!(score <= 1.0, "Score should be clamped to 1.0");

        // Extreme negative
        let mut ctx_neg = ACEContext::default();
        ctx_neg
            .topic_affinities
            .insert("spam".to_string(), (-1.0, 1.0));
        ctx_neg.anti_topics.push("spam".to_string());
        ctx_neg
            .anti_topic_confidence
            .insert("spam".to_string(), 1.0);

        let score_neg = compute_unified_relevance(0.5, &vec!["spam".to_string()], &ctx_neg);
        assert!(score_neg >= 0.0, "Score should be clamped to 0.0");
    }

    // Test partial topic matching
    #[test]
    fn test_partial_topic_match() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 0.9));

        // "rustlang" should partially match "rust"
        let topics = vec!["rustlang".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);

        assert!(mult > 1.0, "Partial match should still produce boost");
    }

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

    // ============================================================================
    // PASIFA Deep README Indexing Tests
    // ============================================================================

    #[test]
    fn test_section_weight() {
        // High value sections
        assert_eq!(section_weight("Features"), 1.0);
        assert_eq!(section_weight("Overview"), 1.0);
        assert_eq!(section_weight("About"), 1.0);

        // API/Usage sections
        assert_eq!(section_weight("API Reference"), 0.9);
        assert_eq!(section_weight("Usage Guide"), 0.9);

        // Architecture sections
        assert_eq!(section_weight("Architecture"), 0.85);
        assert_eq!(section_weight("Design Patterns"), 0.85);

        // Examples sections
        assert_eq!(section_weight("Examples"), 0.8);
        assert_eq!(section_weight("Demo"), 0.8);

        // Installation sections
        assert_eq!(section_weight("Installation"), 0.7);
        assert_eq!(section_weight("Getting Started"), 0.7);

        // Low value sections
        assert_eq!(section_weight("License"), 0.3);
        assert_eq!(section_weight("Contributors"), 0.3);

        // Default weight
        assert_eq!(section_weight("Random Section"), 0.6);
    }

    #[test]
    fn test_parse_readme_sections() {
        let readme = r#"# Project Title

Some intro text here.

## Features

- Feature 1
- Feature 2

## Installation

Install with npm:

```bash
npm install
```

## License

MIT License
"#;

        let sections = parse_readme_sections(readme);

        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].heading, "Project Title");
        assert!(sections[0].content.contains("Some intro text"));
        assert_eq!(sections[1].heading, "Features");
        assert!(sections[1].content.contains("Feature 1"));
        assert_eq!(sections[2].heading, "Installation");
        assert!(sections[2].content.contains("npm install"));
        assert_eq!(sections[3].heading, "License");
        assert!(sections[3].content.contains("MIT"));
    }

    #[test]
    fn test_has_manifest_logic() {
        // Test manifest detection on current project (should have Cargo.toml)
        let current_dir = std::env::current_dir().unwrap();
        assert!(
            has_manifest(&current_dir),
            "Current directory should have Cargo.toml"
        );

        // Test on a directory that definitely won't have a manifest
        let non_project_dir = PathBuf::from("/nonexistent/path");
        // This will return false because read_dir fails and no manifests exist
        assert!(
            !has_manifest(&non_project_dir),
            "Nonexistent directory should not have manifest"
        );
    }

    #[test]
    fn test_discover_projects_on_current_dir() {
        // Test recursive discovery on current project
        let current_dir = std::env::current_dir().unwrap();
        let skip_dirs = ["node_modules", "target", ".git", "dist", "build"];

        // Discover projects with depth 2
        let projects = discover_projects_recursive(&current_dir, 2, &skip_dirs);

        // Should find at least the current project (has Cargo.toml)
        assert!(
            !projects.is_empty(),
            "Should discover at least the current project"
        );

        // Current directory should be in the list
        assert!(
            projects.contains(&current_dir),
            "Should discover current project directory"
        );
    }

    // Test temporal freshness computation
    #[test]
    fn test_temporal_freshness_very_recent() {
        let now = chrono::Utc::now();
        let freshness = compute_temporal_freshness(&now);
        assert_eq!(freshness, 1.15, "Items just created should get max boost");
    }

    #[test]
    fn test_temporal_freshness_few_hours() {
        let three_hours_ago = chrono::Utc::now() - chrono::Duration::hours(3);
        let freshness = compute_temporal_freshness(&three_hours_ago);
        assert_eq!(freshness, 1.10, "Items 3h old should get 1.10x boost");
    }

    #[test]
    fn test_temporal_freshness_half_day() {
        let nine_hours_ago = chrono::Utc::now() - chrono::Duration::hours(9);
        let freshness = compute_temporal_freshness(&nine_hours_ago);
        assert_eq!(freshness, 1.05, "Items 9h old should get 1.05x boost");
    }

    #[test]
    fn test_temporal_freshness_one_day() {
        let eighteen_hours_ago = chrono::Utc::now() - chrono::Duration::hours(18);
        let freshness = compute_temporal_freshness(&eighteen_hours_ago);
        assert_eq!(freshness, 1.0, "Items 18h old should be neutral");
    }

    #[test]
    fn test_temporal_freshness_old() {
        let two_days_ago = chrono::Utc::now() - chrono::Duration::hours(40);
        let freshness = compute_temporal_freshness(&two_days_ago);
        assert_eq!(freshness, 0.90, "Items 40h old should decay to 0.90");
    }

    #[test]
    fn test_temporal_freshness_very_old() {
        let old = chrono::Utc::now() - chrono::Duration::hours(72);
        let freshness = compute_temporal_freshness(&old);
        assert_eq!(freshness, 0.85, "Items 72h old should hit floor at 0.85");
    }
}
