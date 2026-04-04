//! Content Translation Engine — real-time translation for dynamic content.
//!
//! Translates feed item titles, descriptions, and briefing summaries into
//! the user's preferred language. Uses a tiered provider chain:
//!
//!   1. SQLite cache (instant, free)
//!   2. Local LLM via Ollama (private, no API key needed)
//!   3. Cloud API via BYOK key (DeepL or provider in settings)
//!   4. Fallback: show original text
//!
//! All translations are cached by content hash + target language so the
//! same content is never translated twice.

use crate::error::{Result, ResultExt};
use crate::llm;
use crate::state::get_database;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{debug, info, warn};

/// Maximum characters to translate in a single item.
/// Prevents runaway LLM costs on very long content.
const MAX_CONTENT_LENGTH: usize = 2000;

/// Daily character budget for ingest-time translation.
/// Prevents runaway API costs. On-demand translation still works when budget exhausted.
const DAILY_INGEST_BUDGET_CHARS: usize = 200_000;

static DAILY_INGEST_CHARS_USED: AtomicUsize = AtomicUsize::new(0);
static DAILY_INGEST_RESET_DAY: AtomicUsize = AtomicUsize::new(0);

/// Maximum items in a single batch translation request.
const MAX_BATCH_SIZE: usize = 20;

/// Cache TTL in seconds (30 days).
const CACHE_TTL_SECS: i64 = 30 * 24 * 3600;

/// Technical terms that should never be translated.
/// These are developer-specific terms that lose meaning in translation.
const DO_NOT_TRANSLATE: &[&str] = &[
    "API",
    "REST",
    "GraphQL",
    "JSON",
    "YAML",
    "TOML",
    "XML",
    "HTML",
    "CSS",
    "HTTP",
    "HTTPS",
    "WebSocket",
    "gRPC",
    "SQL",
    "NoSQL",
    "git",
    "GitHub",
    "GitLab",
    "npm",
    "yarn",
    "pnpm",
    "cargo",
    "pip",
    "brew",
    "Docker",
    "Kubernetes",
    "k8s",
    "CI/CD",
    "DevOps",
    "SRE",
    "TypeScript",
    "JavaScript",
    "Rust",
    "Python",
    "Go",
    "Java",
    "C++",
    "C#",
    "Ruby",
    "PHP",
    "Swift",
    "Kotlin",
    "Dart",
    "Zig",
    "Elixir",
    "Haskell",
    "React",
    "Vue",
    "Svelte",
    "Angular",
    "Next.js",
    "Nuxt",
    "Remix",
    "Node.js",
    "Deno",
    "Bun",
    "Tauri",
    "Electron",
    "Linux",
    "macOS",
    "Windows",
    "iOS",
    "Android",
    "WASM",
    "WebAssembly",
    "LLM",
    "GPT",
    "Claude",
    "Ollama",
    "OpenAI",
    "Anthropic",
    "PR",
    "MR",
    "CLI",
    "SDK",
    "IDE",
    "VSCode",
    "JetBrains",
    "AWS",
    "GCP",
    "Azure",
    "Vercel",
    "Netlify",
    "Cloudflare",
    "PostgreSQL",
    "MySQL",
    "SQLite",
    "Redis",
    "MongoDB",
    "TLS",
    "SSL",
    "OAuth",
    "JWT",
    "CORS",
    "XSS",
    "CSRF",
    "4DA",
    "PASIFA",
    "ACE",
    "MUSE",
    "STREETS",
];

/// Check if ingest translation budget allows more characters.
/// Resets daily. Returns false if budget exhausted.
/// Uses compare_exchange for race-safe budget acquisition.
pub fn check_ingest_budget(chars: usize) -> bool {
    use chrono::Datelike;
    let today = chrono::Utc::now().ordinal() as usize;
    let stored_day = DAILY_INGEST_RESET_DAY.load(Ordering::Relaxed);
    if today != stored_day {
        // Race-safe: only one thread resets
        if DAILY_INGEST_RESET_DAY
            .compare_exchange(stored_day, today, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            DAILY_INGEST_CHARS_USED.store(0, Ordering::SeqCst);
        }
    }
    // Atomic compare-exchange loop for race-safe budget acquisition
    loop {
        let current = DAILY_INGEST_CHARS_USED.load(Ordering::Relaxed);
        if current + chars > DAILY_INGEST_BUDGET_CHARS {
            return false;
        }
        if DAILY_INGEST_CHARS_USED
            .compare_exchange(
                current,
                current + chars,
                Ordering::SeqCst,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            return true;
        }
    }
}

// ============================================================================
// Types
// ============================================================================

/// A content item to translate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranslationRequest {
    /// Unique ID for the content (e.g., source_item.id)
    pub id: String,
    /// The text to translate
    pub text: String,
    /// Source language (default: "en")
    #[serde(default = "default_source_lang")]
    pub source_lang: String,
}

fn default_source_lang() -> String {
    "en".to_string()
}

/// A translated content item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranslationResult {
    /// Matches the request ID
    pub id: String,
    /// Original text
    pub original: String,
    /// Translated text (same as original if translation unavailable)
    pub translated: String,
    /// Whether translation was from cache
    pub from_cache: bool,
    /// Provider used ("cache", "ollama", "cloud", "none")
    pub provider: String,
}

/// Translation settings from the user's config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContentTranslationSettings {
    /// Whether content translation is enabled
    pub enabled: bool,
    /// Provider preference: "auto" (try local then cloud), "local", "cloud", "disabled"
    pub provider: String,
    /// Target language (from user's locale setting)
    pub target_lang: String,
}

// ============================================================================
// Cache Layer
// ============================================================================

/// Compute a stable hash for content lookup.
fn content_hash(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Look up a cached translation.
fn get_cached(content_hash: &str, target_lang: &str) -> Option<String> {
    let db = get_database().ok()?;
    let conn = db.conn.lock();

    let result = conn.query_row(
        "SELECT translated_text FROM translation_cache
         WHERE content_hash = ?1 AND target_lang = ?2
         AND last_used_at > strftime('%s', 'now') - ?3",
        rusqlite::params![content_hash, target_lang, CACHE_TTL_SECS],
        |row| row.get::<_, String>(0),
    );

    if let Ok(ref _text) = result {
        // Update last_used_at and use_count
        let _ = conn.execute(
            "UPDATE translation_cache SET last_used_at = strftime('%s', 'now'), use_count = use_count + 1
             WHERE content_hash = ?1 AND target_lang = ?2",
            rusqlite::params![content_hash, target_lang],
        );
        debug!(target: "4da::i18n", hash = content_hash, lang = target_lang, "Translation cache hit");
    }

    result.ok()
}

/// Store a translation in the cache.
fn cache_translation(
    hash: &str,
    source_lang: &str,
    target_lang: &str,
    source_text: &str,
    translated_text: &str,
    provider: &str,
) {
    let db = match get_database() {
        Ok(db) => db,
        Err(_) => return,
    };
    let conn = db.conn.lock();

    let _ = conn.execute(
        "INSERT INTO translation_cache (content_hash, source_lang, target_lang, source_text, translated_text, provider)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(content_hash, target_lang) DO UPDATE SET
             translated_text = excluded.translated_text,
             provider = excluded.provider,
             last_used_at = strftime('%s', 'now'),
             use_count = use_count + 1",
        rusqlite::params![hash, source_lang, target_lang, source_text, translated_text, provider],
    );
}

// ============================================================================
// Translation Providers
// ============================================================================

/// Build the LLM system prompt for content translation.
fn build_translation_prompt(target_lang: &str) -> String {
    let target_name = lang_display_name(target_lang);
    let terms = DO_NOT_TRANSLATE.join(", ");

    format!(
        "You are a professional translator specializing in developer and technology content. \
         Translate from English to {target_name}.\n\n\
         RULES:\n\
         1. Translate naturally — use native phrasing, not word-for-word translation.\n\
         2. NEVER translate these technical terms (keep them in English): {terms}\n\
         3. Preserve all URLs, code snippets, and version numbers exactly.\n\
         4. For compound terms like 'React component' or 'API endpoint', keep the technical \
            noun in English but translate surrounding words naturally.\n\
         5. Return ONLY the translated text. No explanation, no quotes, no markdown.\n\
         6. If the text is already in {target_name}, return it unchanged.\n\
         7. Match the tone and formality of the original text."
    )
}

/// Translate a batch of items using the local LLM (Ollama or configured provider).
async fn translate_via_llm(
    items: &[(&str, &str)], // (id, text) pairs
    target_lang: &str,
) -> Result<HashMap<String, String>> {
    let client = get_llm_client()?;
    let system = build_translation_prompt(target_lang);

    let mut results: HashMap<String, String> = HashMap::new();

    // For small batches (1-3 items), translate individually for better quality
    if items.len() <= 3 {
        for (id, text) in items {
            let truncated = truncate_text(text);
            let response = client
                .complete(
                    &system,
                    vec![llm::Message {
                        role: "user".to_string(),
                        content: truncated.to_string(),
                    }],
                )
                .await;

            match response {
                Ok(resp) => {
                    let translated = resp.content.trim().to_string();
                    // Sanity check: translated text shouldn't be empty or identical
                    // (for languages with different scripts)
                    if !translated.is_empty() {
                        results.insert(id.to_string(), translated);
                    }
                }
                Err(e) => {
                    warn!(target: "4da::i18n", id = %id, error = %e, "Translation failed for item");
                }
            }
        }
    } else {
        // Batch translate: send as numbered list for efficiency
        let numbered: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, (_, text))| format!("[{}] {}", i + 1, truncate_text(text)))
            .collect();

        let batch_prompt = format!(
            "Translate each numbered item below. Return ONLY the translations, \
             one per line, with the same [N] numbering:\n\n{}",
            numbered.join("\n")
        );

        let response = client
            .complete(
                &system,
                vec![llm::Message {
                    role: "user".to_string(),
                    content: batch_prompt,
                }],
            )
            .await
            .context("Batch translation failed")?;

        // Parse numbered responses
        for line in response.content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Match [N] prefix
            if let Some(rest) = line.strip_prefix('[') {
                if let Some(bracket_end) = rest.find(']') {
                    if let Ok(idx) = rest[..bracket_end].trim().parse::<usize>() {
                        if idx >= 1 && idx <= items.len() {
                            let translated = rest[bracket_end + 1..].trim().to_string();
                            if !translated.is_empty() {
                                results.insert(items[idx - 1].0.to_string(), translated);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(results)
}

// ============================================================================
// Public API
// ============================================================================

/// Translate a single content item.
///
/// Checks cache first, then falls back to LLM translation.
/// Returns immediately with the original text if translation is not possible.
pub async fn translate_content(
    request: &TranslationRequest,
    target_lang: &str,
) -> TranslationResult {
    // Skip translation if target is the source language
    if target_lang == request.source_lang || target_lang == "en" {
        return TranslationResult {
            id: request.id.clone(),
            original: request.text.clone(),
            translated: request.text.clone(),
            from_cache: false,
            provider: "none".to_string(),
        };
    }

    let hash = content_hash(&request.text);

    // Check cache first
    if let Some(cached) = get_cached(&hash, target_lang) {
        return TranslationResult {
            id: request.id.clone(),
            original: request.text.clone(),
            translated: cached,
            from_cache: true,
            provider: "cache".to_string(),
        };
    }

    // Try dedicated provider first, then LLM fallback
    let items = vec![(request.id.as_str(), request.text.as_str())];
    if let Some((translations, provider_name)) = translate_with_fallback(&items, target_lang).await
    {
        if let Some(translated) = translations.get(&request.id) {
            cache_translation(
                &hash,
                &request.source_lang,
                target_lang,
                &request.text,
                translated,
                &provider_name,
            );

            return TranslationResult {
                id: request.id.clone(),
                original: request.text.clone(),
                translated: translated.clone(),
                from_cache: false,
                provider: provider_name,
            };
        }
    }

    // Fallback: return original
    TranslationResult {
        id: request.id.clone(),
        original: request.text.clone(),
        translated: request.text.clone(),
        from_cache: false,
        provider: "none".to_string(),
    }
}

/// Translate a batch of content items.
///
/// Efficient batch translation: checks cache for all items first, then
/// translates only uncached items in a single LLM call.
pub async fn translate_content_batch(
    requests: &[TranslationRequest],
    target_lang: &str,
) -> Vec<TranslationResult> {
    // Skip translation if target is English
    if target_lang == "en" {
        return requests
            .iter()
            .map(|r| TranslationResult {
                id: r.id.clone(),
                original: r.text.clone(),
                translated: r.text.clone(),
                from_cache: false,
                provider: "none".to_string(),
            })
            .collect();
    }

    let mut results: Vec<TranslationResult> = Vec::with_capacity(requests.len());
    let mut uncached: Vec<(usize, &TranslationRequest, String)> = Vec::new(); // (index, request, hash)

    // Phase 1: Check cache for all items
    for (i, request) in requests.iter().enumerate() {
        let hash = content_hash(&request.text);

        if request.source_lang == target_lang {
            results.push(TranslationResult {
                id: request.id.clone(),
                original: request.text.clone(),
                translated: request.text.clone(),
                from_cache: false,
                provider: "none".to_string(),
            });
        } else if let Some(cached) = get_cached(&hash, target_lang) {
            results.push(TranslationResult {
                id: request.id.clone(),
                original: request.text.clone(),
                translated: cached,
                from_cache: true,
                provider: "cache".to_string(),
            });
        } else {
            // Placeholder — will be filled after LLM translation
            results.push(TranslationResult {
                id: request.id.clone(),
                original: request.text.clone(),
                translated: request.text.clone(), // temporary: original text
                from_cache: false,
                provider: "none".to_string(),
            });
            uncached.push((i, request, hash));
        }
    }

    if uncached.is_empty() {
        info!(target: "4da::i18n", count = requests.len(), lang = target_lang, "All items served from cache");
        return results;
    }

    info!(target: "4da::i18n",
        total = requests.len(),
        cached = requests.len() - uncached.len(),
        to_translate = uncached.len(),
        lang = target_lang,
        "Translating uncached content"
    );

    // Phase 2: Batch translate uncached items (in chunks of MAX_BATCH_SIZE)
    for chunk in uncached.chunks(MAX_BATCH_SIZE) {
        let items: Vec<(&str, &str)> = chunk
            .iter()
            .map(|(_, req, _)| (req.id.as_str(), req.text.as_str()))
            .collect();

        if let Some((translations, provider_name)) =
            translate_with_fallback(&items, target_lang).await
        {
            for (idx, request, hash) in chunk {
                if let Some(translated) = translations.get(&request.id) {
                    results[*idx] = TranslationResult {
                        id: request.id.clone(),
                        original: request.text.clone(),
                        translated: translated.clone(),
                        from_cache: false,
                        provider: provider_name.clone(),
                    };

                    cache_translation(
                        hash,
                        &request.source_lang,
                        target_lang,
                        &request.text,
                        translated,
                        &provider_name,
                    );
                }
            }
        }
    }

    results
}

/// Get translation cache statistics.
pub fn get_cache_stats(target_lang: &str) -> Result<CacheStats> {
    let db = get_database()?;
    let conn = db.conn.lock();

    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM translation_cache WHERE target_lang = ?1",
            rusqlite::params![target_lang],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let active: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM translation_cache
             WHERE target_lang = ?1 AND last_used_at > strftime('%s', 'now') - ?2",
            rusqlite::params![target_lang, CACHE_TTL_SECS],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_uses: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(use_count), 0) FROM translation_cache WHERE target_lang = ?1",
            rusqlite::params![target_lang],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(CacheStats {
        target_lang: target_lang.to_string(),
        total_entries: total as usize,
        active_entries: active as usize,
        total_lookups: total_uses as usize,
    })
}

/// Purge expired entries from the translation cache.
pub fn purge_expired_cache() -> Result<usize> {
    let db = get_database()?;
    let conn = db.conn.lock();

    let deleted = conn
        .execute(
            "DELETE FROM translation_cache WHERE last_used_at < strftime('%s', 'now') - ?1",
            rusqlite::params![CACHE_TTL_SECS],
        )
        .unwrap_or(0);

    if deleted > 0 {
        info!(target: "4da::i18n", deleted, "Purged expired translation cache entries");
    }

    Ok(deleted)
}

// ============================================================================
// Types (additional)
// ============================================================================

/// Cache statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheStats {
    pub target_lang: String,
    pub total_entries: usize,
    pub active_entries: usize,
    pub total_lookups: usize,
}

// ============================================================================
// Helpers
// ============================================================================

/// Build an LLM client from the user's current settings.
fn get_llm_client() -> Result<llm::LLMClient> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let provider = guard.get().llm.clone();
    if provider.api_key.is_empty() && provider.provider != "ollama" {
        let lang = crate::i18n::get_user_language();
        return Err(crate::i18n::t("errors:translation.noLlm", &lang, &[]).into());
    }
    Ok(llm::LLMClient::new(provider))
}

/// Map language code to display name for LLM prompts.
pub(crate) fn lang_display_name(code: &str) -> &str {
    match code {
        "ar" => "Arabic",
        "de" => "German",
        "es" => "Spanish",
        "fr" => "French",
        "hi" => "Hindi",
        "it" => "Italian",
        "ja" => "Japanese",
        "ko" => "Korean",
        "nl" => "Dutch",
        "pl" => "Polish",
        "pt-BR" => "Brazilian Portuguese",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "tr" => "Turkish",
        "zh" => "Simplified Chinese",
        "zh-TW" => "Traditional Chinese",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "sv" => "Swedish",
        "da" => "Danish",
        "fi" => "Finnish",
        "nb" => "Norwegian",
        "uk" => "Ukrainian",
        "cs" => "Czech",
        "ro" => "Romanian",
        "hu" => "Hungarian",
        "he" => "Hebrew",
        "fa" => "Persian",
        "id" => "Indonesian",
        "ms" => "Malay",
        "bn" => "Bengali",
        _ => code,
    }
}

/// Detect which translation provider is active based on current settings.
fn detect_active_provider() -> String {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let tc = &guard.get().translation;
    if !tc.api_key.is_empty()
        && matches!(tc.provider.as_str(), "deepl" | "google" | "azure" | "auto")
    {
        tc.provider.clone()
    } else {
        "llm".to_string()
    }
}

/// Try dedicated provider then LLM fallback. Returns (translations, provider_name) or None.
async fn translate_with_fallback(
    items: &[(&str, &str)],
    target_lang: &str,
) -> Option<(HashMap<String, String>, String)> {
    use crate::translation_providers::ProviderResult;
    match crate::translation_providers::try_dedicated_provider(items, target_lang).await {
        Ok(ProviderResult::Ok(t)) => Some((t, detect_active_provider())),
        Ok(ProviderResult::Fallback) | Ok(ProviderResult::UseLlm) => {
            translate_via_llm(items, target_lang)
                .await
                .ok()
                .map(|t| (t, "llm".to_string()))
        }
        Err(e) => {
            warn!(target: "4da::i18n", error = %e, "Dedicated translation failed, trying LLM");
            translate_via_llm(items, target_lang)
                .await
                .ok()
                .map(|t| (t, "llm".to_string()))
        }
    }
}

/// Truncate text to MAX_CONTENT_LENGTH for translation.
/// Uses char-boundary-safe slicing to prevent panics on multi-byte UTF-8 (CJK, Arabic, etc.)
fn truncate_text(text: &str) -> &str {
    if text.len() <= MAX_CONTENT_LENGTH {
        text
    } else {
        // Find a safe char boundary (multi-byte UTF-8 characters may span the cut point)
        let mut end = MAX_CONTENT_LENGTH;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        let truncated = &text[..end];
        match truncated.rfind(' ') {
            Some(pos) if pos > 0 => &truncated[..pos],
            _ => truncated,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_stable() {
        let hash1 = content_hash("Hello, world!");
        let hash2 = content_hash("Hello, world!");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_normalizes_whitespace() {
        let hash1 = content_hash("Hello  world");
        let hash2 = content_hash("Hello world");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_different_for_different_content() {
        let hash1 = content_hash("Hello");
        let hash2 = content_hash("World");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_lang_display_name_known() {
        assert_eq!(lang_display_name("ja"), "Japanese");
        assert_eq!(lang_display_name("de"), "German");
        assert_eq!(lang_display_name("pt-BR"), "Brazilian Portuguese");
        assert_eq!(lang_display_name("ar"), "Arabic");
    }

    #[test]
    fn test_lang_display_name_unknown_returns_code() {
        assert_eq!(lang_display_name("xx"), "xx");
    }

    #[test]
    fn test_truncate_text_short() {
        let text = "Short text";
        assert_eq!(truncate_text(text), text);
    }

    #[test]
    fn test_truncate_text_long() {
        let text = "a ".repeat(1500);
        let truncated = truncate_text(&text);
        assert!(truncated.len() <= MAX_CONTENT_LENGTH);
    }

    #[test]
    fn test_translation_prompt_contains_terms() {
        let prompt = build_translation_prompt("ja");
        assert!(prompt.contains("Japanese"));
        assert!(prompt.contains("React"));
        assert!(prompt.contains("TypeScript"));
        assert!(prompt.contains("NEVER translate"));
    }

    #[test]
    fn test_do_not_translate_list_not_empty() {
        assert!(!DO_NOT_TRANSLATE.is_empty());
        assert!(DO_NOT_TRANSLATE.contains(&"React"));
        assert!(DO_NOT_TRANSLATE.contains(&"4DA"));
    }

    #[test]
    fn test_ingest_budget_allows_small_requests() {
        // Reset by simulating a new day (ordinal changes)
        // Small requests should always be allowed within budget
        assert!(check_ingest_budget(100));
    }

    #[test]
    fn test_daily_budget_constant_is_reasonable() {
        // 200k chars = ~40k titles at 5 words avg = covers heavy usage
        assert_eq!(DAILY_INGEST_BUDGET_CHARS, 200_000);
    }
}
