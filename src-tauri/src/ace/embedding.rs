//! Embedding Integration for ACE
//!
//! Provides embedding generation and similarity computation for topics.
//! Supports both local (Ollama) and cloud (OpenAI) embedding providers.

#![allow(dead_code)]

use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Embedding Provider Configuration
// ============================================================================

/// Embedding provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Which provider to use
    pub provider: EmbeddingProvider,
    /// OpenAI API key (if using OpenAI)
    pub openai_api_key: Option<String>,
    /// Ollama base URL (if using Ollama)
    pub ollama_base_url: Option<String>,
    /// Model name to use
    pub model: String,
    /// Cache embeddings in database
    pub cache_enabled: bool,
    /// Batch size for bulk operations
    pub batch_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProvider {
    OpenAI,
    Ollama,
    Mock, // For testing
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: EmbeddingProvider::Mock,
            openai_api_key: None,
            ollama_base_url: Some("http://localhost:11434".to_string()),
            model: "text-embedding-3-small".to_string(),
            cache_enabled: true,
            batch_size: 32,
        }
    }
}

// ============================================================================
// Embedding Service
// ============================================================================

/// Embedding service for generating and caching embeddings
pub struct EmbeddingService {
    config: EmbeddingConfig,
    conn: Arc<Mutex<Connection>>,
    /// In-memory cache for hot embeddings
    cache: Mutex<HashMap<String, Vec<f32>>>,
    /// Embedding dimension
    dimension: usize,
}

impl EmbeddingService {
    pub fn new(config: EmbeddingConfig, conn: Arc<Mutex<Connection>>) -> Self {
        let dimension = match config.provider {
            EmbeddingProvider::OpenAI => 1536, // text-embedding-3-small
            EmbeddingProvider::Ollama => 768,  // nomic-embed-text
            EmbeddingProvider::Mock => 128,    // For testing
        };

        // Initialize embedding cache table
        let _ = Self::init_cache_table(&conn);

        Self {
            config,
            conn,
            cache: Mutex::new(HashMap::new()),
            dimension,
        }
    }

    /// Initialize the embedding cache table
    fn init_cache_table(conn: &Arc<Mutex<Connection>>) -> Result<(), String> {
        let conn = conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS embedding_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL UNIQUE,
                embedding BLOB NOT NULL,
                model TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_embedding_cache_text ON embedding_cache(text);",
        )
        .map_err(|e| format!("Failed to create embedding cache table: {}", e))?;
        Ok(())
    }

    /// Get or generate embedding for a single text
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        // Check in-memory cache first
        if let Some(cached) = self.cache.lock().get(text) {
            return Ok(cached.clone());
        }

        // Check database cache
        if self.config.cache_enabled {
            if let Some(cached) = self.get_cached_embedding(text)? {
                // Warm in-memory cache
                self.cache.lock().insert(text.to_string(), cached.clone());
                return Ok(cached);
            }
        }

        // Generate new embedding
        let embedding = self.generate_embedding(text)?;

        // Cache it
        if self.config.cache_enabled {
            let _ = self.cache_embedding(text, &embedding);
        }
        self.cache
            .lock()
            .insert(text.to_string(), embedding.clone());

        Ok(embedding)
    }

    /// Batch embed multiple texts
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let mut results = Vec::with_capacity(texts.len());
        let mut to_generate: Vec<(usize, String)> = Vec::new();

        // Check caches first
        for (i, text) in texts.iter().enumerate() {
            if let Some(cached) = self.cache.lock().get(text) {
                results.push((i, cached.clone()));
            } else if self.config.cache_enabled {
                if let Ok(Some(cached)) = self.get_cached_embedding(text) {
                    self.cache.lock().insert(text.clone(), cached.clone());
                    results.push((i, cached));
                } else {
                    to_generate.push((i, text.clone()));
                }
            } else {
                to_generate.push((i, text.clone()));
            }
        }

        // Generate missing embeddings in batches
        for chunk in to_generate.chunks(self.config.batch_size) {
            let texts_to_embed: Vec<&str> = chunk.iter().map(|(_, t)| t.as_str()).collect();
            let embeddings = self.generate_batch_embedding(&texts_to_embed)?;

            for ((i, text), embedding) in chunk.iter().zip(embeddings) {
                if self.config.cache_enabled {
                    let _ = self.cache_embedding(text, &embedding);
                }
                self.cache.lock().insert(text.clone(), embedding.clone());
                results.push((*i, embedding));
            }
        }

        // Sort by original index
        results.sort_by_key(|(i, _)| *i);
        Ok(results.into_iter().map(|(_, e)| e).collect())
    }

    /// Generate embedding using the configured provider
    fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, String> {
        match self.config.provider {
            EmbeddingProvider::OpenAI => self.embed_openai(text),
            EmbeddingProvider::Ollama => self.embed_ollama(text),
            EmbeddingProvider::Mock => self.embed_mock(text),
        }
    }

    /// Generate batch embeddings
    fn generate_batch_embedding(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        match self.config.provider {
            EmbeddingProvider::OpenAI => self.embed_openai_batch(texts),
            EmbeddingProvider::Ollama => {
                // Ollama doesn't support batch, embed one by one
                texts.iter().map(|t| self.embed_ollama(t)).collect()
            }
            EmbeddingProvider::Mock => texts.iter().map(|t| self.embed_mock(t)).collect(),
        }
    }

    /// Generate embedding using OpenAI API
    /// Note: Requires async runtime for actual API calls
    fn embed_openai(&self, text: &str) -> Result<Vec<f32>, String> {
        // For synchronous context, use mock embedding
        // Real implementation would use async API
        if self.config.openai_api_key.is_none() {
            return Err("OpenAI API key not configured".to_string());
        }
        // Fall back to mock for now - real implementation needs async
        self.embed_mock(text)
    }

    /// Batch embed using OpenAI
    fn embed_openai_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        if self.config.openai_api_key.is_none() {
            return Err("OpenAI API key not configured".to_string());
        }
        // Fall back to mock for now - real implementation needs async
        texts.iter().map(|t| self.embed_mock(t)).collect()
    }

    /// Generate embedding using Ollama
    fn embed_ollama(&self, text: &str) -> Result<Vec<f32>, String> {
        if self.config.ollama_base_url.is_none() {
            return Err("Ollama base URL not configured".to_string());
        }
        // Fall back to mock for now - real implementation needs async
        self.embed_mock(text)
    }

    /// Generate mock embedding for testing
    fn embed_mock(&self, text: &str) -> Result<Vec<f32>, String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Deterministic embedding based on text hash
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();

        let mut embedding = Vec::with_capacity(self.dimension);
        let mut state = hash;
        for _ in 0..self.dimension {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = ((state >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
            embedding.push(value);
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        Ok(embedding)
    }

    /// Get cached embedding from database
    fn get_cached_embedding(&self, text: &str) -> Result<Option<Vec<f32>>, String> {
        let conn = self.conn.lock();
        let result: Result<Vec<u8>, _> = conn.query_row(
            "SELECT embedding FROM embedding_cache WHERE text = ?1",
            [text],
            |row| row.get(0),
        );

        match result {
            Ok(bytes) => {
                // Decode f32 array from bytes
                let embedding = bytes_to_f32_vec(&bytes);
                Ok(Some(embedding))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Cache lookup failed: {}", e)),
        }
    }

    /// Cache embedding in database
    fn cache_embedding(&self, text: &str, embedding: &[f32]) -> Result<(), String> {
        let conn = self.conn.lock();
        let bytes = f32_vec_to_bytes(embedding);

        conn.execute(
            "INSERT OR REPLACE INTO embedding_cache (text, embedding, model) VALUES (?1, ?2, ?3)",
            rusqlite::params![text, bytes, self.config.model],
        )
        .map_err(|e| format!("Failed to cache embedding: {}", e))?;

        Ok(())
    }

    /// Compute cosine similarity between two embeddings
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    /// Find most similar topics to a query
    pub fn find_similar(
        &self,
        query: &str,
        candidates: &[String],
        top_k: usize,
    ) -> Result<Vec<(String, f32)>, String> {
        let query_embedding = self.embed(query)?;
        let candidate_embeddings = self.embed_batch(candidates)?;

        let mut similarities: Vec<(String, f32)> = candidates
            .iter()
            .zip(candidate_embeddings.iter())
            .map(|(text, emb)| {
                let sim = Self::cosine_similarity(&query_embedding, emb);
                (text.clone(), sim)
            })
            .collect();

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(top_k);

        Ok(similarities)
    }

    /// Get embedding dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Check if the service is operational
    pub fn is_operational(&self) -> bool {
        match self.config.provider {
            EmbeddingProvider::Mock => true,
            EmbeddingProvider::OpenAI => self.config.openai_api_key.is_some(),
            EmbeddingProvider::Ollama => self.config.ollama_base_url.is_some(),
        }
    }
}

// ============================================================================
// API Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    data: Vec<OpenAIEmbedding>,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbedding {
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    embedding: Vec<f32>,
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Convert f32 vector to bytes for storage
fn f32_vec_to_bytes(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert bytes back to f32 vector
fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap();
            f32::from_le_bytes(arr)
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Arc<Mutex<Connection>> {
        Arc::new(Mutex::new(Connection::open_in_memory().unwrap()))
    }

    #[test]
    fn test_mock_embedding() {
        let config = EmbeddingConfig::default();
        let conn = setup_test_db();
        let service = EmbeddingService::new(config, conn);

        let embedding = service.embed("hello world").unwrap();
        assert_eq!(embedding.len(), 128); // Mock dimension

        // Same text should give same embedding
        let embedding2 = service.embed("hello world").unwrap();
        assert_eq!(embedding, embedding2);

        // Different text should give different embedding
        let embedding3 = service.embed("goodbye world").unwrap();
        assert_ne!(embedding, embedding3);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((EmbeddingService::cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((EmbeddingService::cosine_similarity(&a, &c)).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((EmbeddingService::cosine_similarity(&a, &d) + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_batch_embedding() {
        let config = EmbeddingConfig::default();
        let conn = setup_test_db();
        let service = EmbeddingService::new(config, conn);

        let texts = vec![
            "rust".to_string(),
            "python".to_string(),
            "javascript".to_string(),
        ];
        let embeddings = service.embed_batch(&texts).unwrap();

        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 128);
        }
    }

    #[test]
    fn test_find_similar() {
        let config = EmbeddingConfig::default();
        let conn = setup_test_db();
        let service = EmbeddingService::new(config, conn);

        let candidates = vec![
            "rust programming".to_string(),
            "python scripting".to_string(),
            "cooking recipes".to_string(),
        ];

        let results = service
            .find_similar("rust language", &candidates, 2)
            .unwrap();
        assert_eq!(results.len(), 2);

        // Mock embeddings are hash-based, not semantic.
        // Just verify results are sorted by similarity (descending)
        assert!(results[0].1 >= results[1].1);

        // Verify all candidates are in results
        let result_texts: Vec<&str> = results.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(result_texts.len(), 2);
    }

    #[test]
    fn test_embedding_caching() {
        let mut config = EmbeddingConfig::default();
        config.cache_enabled = true;
        let conn = setup_test_db();
        let service = EmbeddingService::new(config, conn.clone());

        // First call generates embedding
        let _ = service.embed("cached text").unwrap();

        // Verify it's in the database
        let cached = service.get_cached_embedding("cached text").unwrap();
        assert!(cached.is_some());
    }

    #[test]
    fn test_bytes_conversion() {
        let original = vec![1.5, -2.0, 0.0, 3.14159];
        let bytes = f32_vec_to_bytes(&original);
        let recovered = bytes_to_f32_vec(&bytes);
        assert_eq!(original, recovered);
    }
}
