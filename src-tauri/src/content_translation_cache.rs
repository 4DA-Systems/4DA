// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Translation cache layer and prompt construction.
//!
//! Split from `content_translation.rs` — houses SQLite cache operations,
//! cache statistics, expiry purge, and the LLM prompt builder.

use crate::error::Result;
use crate::state::get_database;
use sha2::{Digest, Sha256};
use tracing::{debug, info};

use super::{lang_display_name, CACHE_TTL_SECS, DO_NOT_TRANSLATE};

// ============================================================================
// Cache Layer
// ============================================================================

/// Compute a stable hash for content lookup.
pub(super) fn content_hash(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Look up a cached translation.
pub(super) fn get_cached(content_hash: &str, target_lang: &str) -> Option<String> {
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
        if let Err(e) = conn.execute(
            "UPDATE translation_cache SET last_used_at = strftime('%s', 'now'), use_count = use_count + 1
             WHERE content_hash = ?1 AND target_lang = ?2",
            rusqlite::params![content_hash, target_lang],
        ) {
            tracing::warn!(target: "4da::translation", error = %e, hash = content_hash, lang = target_lang, "Failed to update translation cache usage stats");
        }
        debug!(target: "4da::i18n", hash = content_hash, lang = target_lang, "Translation cache hit");
    }

    result.ok()
}

/// Store a translation in the cache.
pub(super) fn cache_translation(
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

    if let Err(e) = conn.execute(
        "INSERT INTO translation_cache (content_hash, source_lang, target_lang, source_text, translated_text, provider)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(content_hash, target_lang) DO UPDATE SET
             translated_text = excluded.translated_text,
             provider = excluded.provider,
             last_used_at = strftime('%s', 'now'),
             use_count = use_count + 1",
        rusqlite::params![hash, source_lang, target_lang, source_text, translated_text, provider],
    ) {
        tracing::warn!(target: "4da::translation", error = %e, hash = hash, target_lang = target_lang, "Failed to cache translation");
    }
}

// ============================================================================
// Translation Prompt
// ============================================================================

/// Build the LLM system prompt for content translation.
pub(super) fn build_translation_prompt(target_lang: &str) -> String {
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

// ============================================================================
// Cache Stats & Maintenance
// ============================================================================

/// Cache statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheStats {
    pub target_lang: String,
    pub total_entries: usize,
    pub active_entries: usize,
    pub total_lookups: usize,
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
