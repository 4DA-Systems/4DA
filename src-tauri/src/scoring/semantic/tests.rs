// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the semantic scoring submodule.

use super::*;
use crate::test_utils::seed_embedding;
use std::collections::HashMap;

/// Helper: cosine similarity via the crate's norm-based function
fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let a_norm = crate::vector_norm(a);
    crate::cosine_similarity_with_norm(a, a_norm, b)
}

/// Helper: build a minimal ACEContext with active topics and confidence
fn ace_ctx_with_topics(topics: &[(&str, f32)]) -> super::super::ace_context::ACEContext {
    let mut ctx = super::super::ace_context::ACEContext::default();
    for &(topic, conf) in topics {
        ctx.active_topics.push(topic.to_string());
        ctx.topic_confidence.insert(topic.to_string(), conf);
    }
    ctx
}

#[test]
fn test_empty_topic_embeddings_returns_none() {
    let item_emb = seed_embedding("rust programming");
    let ace_ctx = ace_ctx_with_topics(&[("rust", 0.9)]);
    let topic_embeddings: HashMap<String, Vec<f32>> = HashMap::new();

    let result = compute_semantic_ace_boost(&item_emb, &ace_ctx, &topic_embeddings);
    assert!(
        result.is_none(),
        "Empty topic embeddings should return None, got {:?}",
        result
    );
}

#[test]
fn test_identical_embedding_produces_max_boost() {
    let emb = seed_embedding("rust");
    let ace_ctx = ace_ctx_with_topics(&[("rust", 1.0)]);
    let mut topic_embeddings = HashMap::new();
    topic_embeddings.insert("rust".to_string(), emb.clone());

    let result = compute_semantic_ace_boost(&emb, &ace_ctx, &topic_embeddings);
    assert!(
        result.is_some(),
        "Identical embeddings should produce a result"
    );
    let boost = result.unwrap();
    // Cosine similarity of identical unit vectors = 1.0
    // base_boost = (1.0 - 0.5) * 1.0 = 0.5, clamped to 0.5
    assert!(
        boost > 0.4,
        "Identical embedding should produce near-max boost, got {}",
        boost
    );
    assert!(
        boost <= 0.5,
        "Boost should be clamped to 0.5, got {}",
        boost
    );
}

#[test]
fn test_orthogonal_embeddings_produce_zero_boost() {
    // Construct two orthogonal unit vector matching EMBEDDING_DIMSs manually
    let mut emb_a = vec![0.0f32; crate::EMBEDDING_DIMS];
    emb_a[0] = 1.0; // unit vector along dimension 0

    let mut emb_b = vec![0.0f32; crate::EMBEDDING_DIMS];
    emb_b[1] = 1.0; // unit vector along dimension 1

    let ace_ctx = ace_ctx_with_topics(&[("topic_b", 1.0)]);
    let mut topic_embeddings = HashMap::new();
    topic_embeddings.insert("topic_b".to_string(), emb_b);

    let result = compute_semantic_ace_boost(&emb_a, &ace_ctx, &topic_embeddings);
    assert!(
        result.is_some(),
        "Should return Some for orthogonal vectors"
    );
    let boost = result.unwrap();
    // Cosine similarity of orthogonal vectors = 0.0
    // base_boost = (0.0 - 0.5) * 1.0 = -0.5, clamped to -0.3
    assert!(
        boost <= 0.0,
        "Orthogonal embeddings should produce non-positive boost, got {}",
        boost
    );
    assert!(
        boost >= -0.3,
        "Boost should be clamped to -0.3, got {}",
        boost
    );
}

// ====================================================================
// Taste Embedding Tests
// ====================================================================

#[test]
fn test_compute_taste_embedding_empty() {
    let affinities: Vec<(String, f32, f32)> = vec![];
    let topic_embs: HashMap<String, Vec<f32>> = HashMap::new();
    assert!(compute_taste_embedding(&affinities, &topic_embs).is_none());
}

#[test]
fn test_compute_taste_embedding_single_topic() {
    let emb = seed_embedding("rust");
    let affinities = vec![("rust".to_string(), 0.8, 0.9)];
    let mut topic_embs = HashMap::new();
    topic_embs.insert("rust".to_string(), emb.clone());

    let taste = compute_taste_embedding(&affinities, &topic_embs);
    assert!(taste.is_some());
    let taste = taste.unwrap();
    assert_eq!(taste.len(), crate::EMBEDDING_DIMS);

    // Should be unit normalized
    let norm = crate::vector_norm(&taste);
    assert!(
        (norm - 1.0).abs() < 0.01,
        "Taste embedding should be unit normalized, got {}",
        norm
    );

    // Should be highly similar to the input embedding
    let sim = cosine_sim(&taste, &emb);
    assert!(
        sim > 0.99,
        "Single-topic taste should be nearly identical, got {}",
        sim
    );
}

#[test]
fn test_compute_taste_embedding_blends_topics() {
    let emb_a = seed_embedding("rust");
    let emb_b = seed_embedding("python");
    let affinities = vec![
        ("rust".to_string(), 0.8, 1.0),
        ("python".to_string(), 0.4, 1.0),
    ];
    let mut topic_embs = HashMap::new();
    topic_embs.insert("rust".to_string(), emb_a.clone());
    topic_embs.insert("python".to_string(), emb_b.clone());

    let taste = compute_taste_embedding(&affinities, &topic_embs).unwrap();

    // Should be more similar to rust (higher weight) than python
    let sim_rust = cosine_sim(&taste, &emb_a);
    let sim_python = cosine_sim(&taste, &emb_b);
    assert!(
        sim_rust > sim_python,
        "Taste should be more similar to higher-weighted topic: rust={:.3} python={:.3}",
        sim_rust,
        sim_python
    );
}

#[test]
fn test_compute_taste_embedding_negative_affinities() {
    let emb_a = seed_embedding("rust");
    let emb_b = seed_embedding("career advice");
    let affinities = vec![
        ("rust".to_string(), 0.9, 1.0),
        ("career advice".to_string(), -0.8, 1.0),
    ];
    let mut topic_embs = HashMap::new();
    topic_embs.insert("rust".to_string(), emb_a.clone());
    topic_embs.insert("career advice".to_string(), emb_b.clone());

    let taste = compute_taste_embedding(&affinities, &topic_embs).unwrap();

    // Taste should be more similar to liked topic than disliked
    let sim_rust = cosine_sim(&taste, &emb_a);
    let sim_career = cosine_sim(&taste, &emb_b);
    assert!(
        sim_rust > sim_career,
        "Taste should prefer liked over disliked: rust={:.3} career={:.3}",
        sim_rust,
        sim_career
    );
}

#[test]
fn test_taste_boost_identical() {
    let emb = seed_embedding("rust");
    let boost = compute_taste_boost(&emb, &emb);
    // Cosine similarity of identical = 1.0 → (1.0 - 0.4) * 0.2 = 0.12, clamped to 0.08
    assert!(
        boost > 0.0,
        "Identical embeddings should produce positive boost"
    );
    assert!(
        boost <= 0.08,
        "Boost should be clamped to 0.08, got {}",
        boost
    );
}

#[test]
fn test_taste_boost_orthogonal() {
    let mut emb_a = vec![0.0f32; crate::EMBEDDING_DIMS];
    emb_a[0] = 1.0;
    let mut emb_b = vec![0.0f32; crate::EMBEDDING_DIMS];
    emb_b[1] = 1.0;

    let boost = compute_taste_boost(&emb_a, &emb_b);
    // Cosine sim = 0.0 → (0.0 - 0.4) * 0.2 = -0.08
    assert!(
        boost < 0.0,
        "Orthogonal embeddings should produce negative boost"
    );
    assert!(
        boost >= -0.08,
        "Boost should be clamped to -0.08, got {}",
        boost
    );
}

#[test]
fn test_taste_boost_zero_embedding() {
    let zero = vec![0.0f32; crate::EMBEDDING_DIMS];
    let taste = seed_embedding("rust");
    let boost = compute_taste_boost(&zero, &taste);
    assert!(
        (boost - 0.0).abs() < f32::EPSILON,
        "Zero embedding should produce 0 boost"
    );
}

#[test]
fn test_zero_norm_embedding_handled_gracefully() {
    let zero_emb = vec![0.0f32; crate::EMBEDDING_DIMS];
    let ace_ctx = ace_ctx_with_topics(&[("rust", 1.0)]);
    let mut topic_embeddings = HashMap::new();
    topic_embeddings.insert("rust".to_string(), seed_embedding("rust"));

    let result = compute_semantic_ace_boost(&zero_emb, &ace_ctx, &topic_embeddings);
    // Zero-norm item embedding returns None (checked at line 23-25)
    assert!(
        result.is_none(),
        "Zero-norm embedding should return None, got {:?}",
        result
    );
}

// ====================================================================
// Topic Enrichment Tests
// ====================================================================

#[test]
fn test_enrich_known_topic_returns_description() {
    let enriched = enrich_topic_for_embedding("rust");
    assert!(
        enriched.contains("systems programming"),
        "Rust description should mention systems programming, got: {enriched}"
    );
    assert!(
        enriched.contains("memory safety"),
        "Rust description should mention memory safety, got: {enriched}"
    );
}

#[test]
fn test_enrich_case_insensitive() {
    let lower = enrich_topic_for_embedding("rust");
    let upper = enrich_topic_for_embedding("Rust");
    let mixed = enrich_topic_for_embedding("RUST");
    assert_eq!(lower, upper, "Enrichment should be case-insensitive");
    assert_eq!(lower, mixed, "Enrichment should be case-insensitive");
}

#[test]
fn test_enrich_unknown_topic_returns_generic() {
    let enriched = enrich_topic_for_embedding("obscure-framework-xyz");
    assert_eq!(
        enriched, "obscure-framework-xyz — software development technology",
        "Unknown topics should get generic fallback"
    );
}

#[test]
fn test_enrich_ambiguous_topics_disambiguate() {
    // "Go" and "Rust" are the most ambiguous — verify they're software-specific
    let go = enrich_topic_for_embedding("go");
    assert!(
        go.contains("compiled language") || go.contains("Google"),
        "Go description should disambiguate as programming language, got: {go}"
    );

    let react = enrich_topic_for_embedding("react");
    assert!(
        react.contains("UI") || react.contains("user interface"),
        "React description should mention UI, got: {react}"
    );
}

#[test]
fn test_enrich_wasm_alias() {
    let wasm = enrich_topic_for_embedding("wasm");
    let webassembly = enrich_topic_for_embedding("webassembly");
    assert_eq!(
        wasm, webassembly,
        "wasm and webassembly should produce the same description"
    );
}

#[test]
fn test_enrich_description_length_bounds() {
    // All curated descriptions should be between 10 and 150 chars (reasonable sentence)
    let topics = [
        "rust",
        "python",
        "react",
        "docker",
        "kubernetes",
        "aws",
        "go",
        "typescript",
        "sqlite",
        "redis",
        "kafka",
        "pytorch",
        "tauri",
    ];
    for topic in &topics {
        let desc = enrich_topic_for_embedding(topic);
        assert!(
            desc.len() >= 10,
            "Description for '{topic}' too short ({} chars): {desc}",
            desc.len()
        );
        assert!(
            desc.len() <= 150,
            "Description for '{topic}' too long ({} chars): {desc}",
            desc.len()
        );
    }
}

#[test]
fn test_enrich_all_curated_entries_non_empty() {
    // Spot-check that every curated entry starts with the topic name or a proper name
    let curated = [
        "actix",
        "angular",
        "ansible",
        "anthropic",
        "apache",
        "aws",
        "axum",
        "azure",
        "bun",
        "c#",
        "c++",
        "caddy",
        "cargo",
        "cuda",
        "cypress",
        "deno",
        "diesel",
        "directx",
        "django",
        "docker",
        "dynamodb",
        "elasticsearch",
        "elixir",
        "eslint",
        "express",
        "fastapi",
        "fastembed",
        "flask",
        "gcp",
        "git",
        "github",
        "go",
        "grafana",
        "graphql",
        "grpc",
        "haskell",
        "hugging face",
        "hyper",
        "java",
        "jest",
        "kafka",
        "kotlin",
        "kubernetes",
        "langchain",
        "laravel",
        "linux",
        "llamaindex",
        "metal",
        "mongodb",
        "mysql",
        "nats",
        "next.js",
        "nginx",
        "node.js",
        "npm",
        "numpy",
        "ollama",
        "onnx",
        "openai",
        "opengl",
        "pandas",
        "php",
        "pip",
        "playwright",
        "postgresql",
        "prettier",
        "prometheus",
        "python",
        "pytorch",
        "rabbitmq",
        "rails",
        "react",
        "redis",
        "rest",
        "rocm",
        "ruby",
        "rust",
        "scala",
        "scikit-learn",
        "seaorm",
        "selenium",
        "serde",
        "spring",
        "sqlite",
        "sqlx",
        "svelte",
        "swift",
        "tauri",
        "tensorflow",
        "terraform",
        "tokio",
        "typescript",
        "vite",
        "vitest",
        "vue",
        "vulkan",
        "wasm",
        "webassembly",
        "webpack",
        "websocket",
    ];
    for topic in &curated {
        let desc = enrich_topic_for_embedding(topic);
        // Should NOT be the generic fallback
        assert!(
            !desc.ends_with("software development technology"),
            "Curated topic '{topic}' fell through to generic fallback: {desc}"
        );
    }
}
