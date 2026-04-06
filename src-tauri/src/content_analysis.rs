// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Deep Content Analysis — pre-score content analysis for the scoring pipeline.
//!
//! Analyzes full article content to determine technical depth, novelty,
//! and audience level. Results are cached by content hash in the
//! `content_analyses` table and used as scoring multipliers.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::Database;
use crate::error::Result;

/// Result of deep content analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAnalysis {
    /// Technical depth score (1-5)
    pub technical_depth: u8,
    /// Novelty score (1-5)
    pub novelty: u8,
    /// Inferred audience level
    pub audience_level: AudienceLevel,
    /// 1-2 sentence summary of the key insight (if extractable)
    pub key_insight: Option<String>,
    /// SHA-256 hex digest of the analyzed content (cache key)
    pub content_hash: String,
    /// ISO 8601 timestamp of when analysis was performed
    pub analyzed_at: String,
}

/// Target audience level inferred from content signals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AudienceLevel {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

impl std::fmt::Display for AudienceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Beginner => write!(f, "Beginner"),
            Self::Intermediate => write!(f, "Intermediate"),
            Self::Advanced => write!(f, "Advanced"),
            Self::Expert => write!(f, "Expert"),
        }
    }
}

impl AudienceLevel {
    /// Parse from a stored string, defaulting to Intermediate on unknown values.
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "Beginner" => Self::Beginner,
            "Intermediate" => Self::Intermediate,
            "Advanced" => Self::Advanced,
            "Expert" => Self::Expert,
            _ => Self::Intermediate,
        }
    }
}

// =============================================================================
// Scoring multiplier
// =============================================================================

/// Convert a `ContentAnalysis` into a scoring multiplier.
///
/// The multiplier adjusts the relevance score based on content depth
/// and audience level relative to the user's experience level.
///
/// Returns a value in the range `[0.55, 1.15]`.
pub fn analysis_to_multiplier(analysis: &ContentAnalysis, is_senior_audience: bool) -> f32 {
    // Depth component: penalize shallow content for senior users, boost deep content
    let depth_factor = match (analysis.technical_depth, is_senior_audience) {
        (1..=2, true) => 0.65,  // Beginner content, senior user → penalty
        (1..=2, false) => 0.90, // Beginner content, junior user → mild
        (3, _) => 1.0,          // Moderate depth → neutral
        (4..=5, true) => 1.12,  // Advanced content, senior user → boost
        (4..=5, false) => 1.02, // Advanced content, junior user → slight boost
        _ => 1.0,               // Fallback
    };

    // Audience component: penalize beginner-targeted content for senior users
    let audience_factor = match (&analysis.audience_level, is_senior_audience) {
        (AudienceLevel::Beginner, true) => 0.60,
        (AudienceLevel::Beginner, false) => 1.0,
        (AudienceLevel::Intermediate, _) => 1.0,
        (AudienceLevel::Advanced, _) => 1.05,
        (AudienceLevel::Expert, _) => 1.10,
    };

    // Weighted average: depth has more influence than audience label
    let combined: f32 = depth_factor * 0.6 + audience_factor * 0.4;
    combined.clamp(0.55, 1.15)
}

// =============================================================================
// Content hashing
// =============================================================================

/// Compute SHA-256 hex digest of content for cache lookup.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

// =============================================================================
// Cache operations
// =============================================================================

/// Check the `content_analyses` cache for a previously computed analysis.
///
/// Returns `Ok(Some(analysis))` on cache hit, `Ok(None)` on miss,
/// or an error if the DB query fails.
pub fn get_cached_analysis(db: &Database, hash: &str) -> Result<Option<ContentAnalysis>> {
    let conn = db.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT technical_depth, novelty, audience_level, key_insight, content_hash, analyzed_at
         FROM content_analyses WHERE content_hash = ?1",
    )?;

    let result = stmt.query_row(rusqlite::params![hash], |row| {
        Ok(ContentAnalysis {
            technical_depth: row.get::<_, i64>(0)? as u8,
            novelty: row.get::<_, i64>(1)? as u8,
            audience_level: AudienceLevel::from_str_lossy(&row.get::<_, String>(2)?),
            key_insight: row.get(3)?,
            content_hash: row.get(4)?,
            analyzed_at: row.get(5)?,
        })
    });

    match result {
        Ok(analysis) => Ok(Some(analysis)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Store a content analysis result in the cache.
///
/// Uses INSERT OR REPLACE so re-analysis of the same content
/// simply updates the row.
#[allow(dead_code)]
pub fn store_analysis(
    db: &Database,
    source_item_id: i64,
    analysis: &ContentAnalysis,
) -> Result<()> {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT OR REPLACE INTO content_analyses
            (source_item_id, content_hash, technical_depth, novelty, audience_level, key_insight, analyzed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            source_item_id,
            analysis.content_hash,
            analysis.technical_depth as i64,
            analysis.novelty as i64,
            analysis.audience_level.to_string(),
            analysis.key_insight,
            analysis.analyzed_at,
        ],
    )?;
    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- analysis_to_multiplier tests ---

    #[test]
    fn test_multiplier_shallow_content_senior_user() {
        let analysis = ContentAnalysis {
            technical_depth: 1,
            novelty: 3,
            audience_level: AudienceLevel::Beginner,
            key_insight: None,
            content_hash: String::new(),
            analyzed_at: String::new(),
        };
        let m = analysis_to_multiplier(&analysis, true);
        // depth_factor=0.65, audience_factor=0.60
        // combined = 0.65*0.6 + 0.60*0.4 = 0.39+0.24 = 0.63
        assert!(
            m < 0.70,
            "Expected penalty for shallow+beginner content, got {m}"
        );
        assert!(m >= 0.55, "Multiplier should not go below 0.55, got {m}");
    }

    #[test]
    fn test_multiplier_deep_content_senior_user() {
        let analysis = ContentAnalysis {
            technical_depth: 5,
            novelty: 4,
            audience_level: AudienceLevel::Expert,
            key_insight: Some("Novel approach to async runtimes".into()),
            content_hash: String::new(),
            analyzed_at: String::new(),
        };
        let m = analysis_to_multiplier(&analysis, true);
        // depth_factor=1.12, audience_factor=1.10
        // combined = 1.12*0.6 + 1.10*0.4 = 0.672+0.44 = 1.112
        assert!(m > 1.05, "Expected boost for deep+expert content, got {m}");
        assert!(m <= 1.15, "Multiplier should not exceed 1.15, got {m}");
    }

    #[test]
    fn test_multiplier_moderate_content_neutral() {
        let analysis = ContentAnalysis {
            technical_depth: 3,
            novelty: 3,
            audience_level: AudienceLevel::Intermediate,
            key_insight: None,
            content_hash: String::new(),
            analyzed_at: String::new(),
        };
        let m = analysis_to_multiplier(&analysis, true);
        // depth_factor=1.0, audience_factor=1.0 → combined=1.0
        assert!(
            (m - 1.0).abs() < 0.01,
            "Expected neutral multiplier for moderate content, got {m}"
        );
    }

    #[test]
    fn test_multiplier_shallow_content_junior_user() {
        let analysis = ContentAnalysis {
            technical_depth: 2,
            novelty: 2,
            audience_level: AudienceLevel::Beginner,
            key_insight: None,
            content_hash: String::new(),
            analyzed_at: String::new(),
        };
        let m = analysis_to_multiplier(&analysis, false);
        // depth_factor=0.90, audience_factor=1.0
        // combined = 0.90*0.6 + 1.0*0.4 = 0.54+0.40 = 0.94
        assert!(
            m > 0.85,
            "Beginner content should not be heavily penalized for junior users, got {m}"
        );
    }

    #[test]
    fn test_multiplier_advanced_content_junior_user() {
        let analysis = ContentAnalysis {
            technical_depth: 4,
            novelty: 4,
            audience_level: AudienceLevel::Advanced,
            key_insight: None,
            content_hash: String::new(),
            analyzed_at: String::new(),
        };
        let m = analysis_to_multiplier(&analysis, false);
        // depth_factor=1.02, audience_factor=1.05
        // combined = 1.02*0.6 + 1.05*0.4 = 0.612+0.42 = 1.032
        assert!(
            m > 1.0,
            "Advanced content should get a small boost, got {m}"
        );
    }

    // --- content_hash tests ---

    #[test]
    fn test_content_hash_deterministic() {
        let hash1 = content_hash("Hello, world!");
        let hash2 = content_hash("Hello, world!");
        assert_eq!(hash1, hash2, "Same content should produce same hash");
    }

    #[test]
    fn test_content_hash_different_inputs() {
        let hash1 = content_hash("Hello, world!");
        let hash2 = content_hash("Goodbye, world!");
        assert_ne!(
            hash1, hash2,
            "Different content should produce different hashes"
        );
    }

    #[test]
    fn test_content_hash_is_hex_sha256() {
        let hash = content_hash("test");
        assert_eq!(hash.len(), 64, "SHA-256 hex digest should be 64 characters");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should be hex characters only"
        );
    }

    // --- AudienceLevel tests ---

    #[test]
    fn test_audience_level_roundtrip() {
        for level in [
            AudienceLevel::Beginner,
            AudienceLevel::Intermediate,
            AudienceLevel::Advanced,
            AudienceLevel::Expert,
        ] {
            let s = level.to_string();
            let parsed = AudienceLevel::from_str_lossy(&s);
            assert_eq!(parsed, level, "Roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_audience_level_unknown_defaults_intermediate() {
        let parsed = AudienceLevel::from_str_lossy("UnknownLevel");
        assert_eq!(parsed, AudienceLevel::Intermediate);
    }
}
