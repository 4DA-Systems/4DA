// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Fixture generator for the real-embedding recall investigation.
//!
//! Run ONCE (model required) to (re)produce the committed `.bin` fixtures:
//!
//! ```text
//! cargo test --features generate-sim-fixtures \
//!     scoring::simulation::fixtures_gen::generate_real_embedding_fixtures \
//!     -- --ignored --nocapture
//! ```
//!
//! It embeds every `corpus()` item (`"{title} {content}"`) and every persona
//! interest / ACE topic / detected-tech string via the SAME fastembed path that
//! `benchmark_calibration::embeddings` uses (`crate::fastembed_sync` +
//! `pad_and_normalize`), so the artefacts reproduce deterministically.
//!
//! Output (under `src/scoring/simulation/fixtures/`):
//!   - `corpus_embeddings.bin`  (u32 id  -> 768-f32 vector)
//!   - `topic_embeddings.bin`   (string  -> 768-f32 vector, exact + lowercase)

#![cfg(feature = "generate-sim-fixtures")]

use super::corpus::corpus;
use super::fixtures_io;

/// Pad fastembed vectors to `EMBEDDING_DIMS` with zeros, then L2-normalize.
/// Identical to `benchmark_calibration::types::pad_and_normalize` (that one is
/// `pub(super)` to its own module, so we mirror it here to share the exact
/// embedding contract while keeping module privacy intact).
fn pad_and_normalize(mut v: Vec<f32>) -> Vec<f32> {
    let target = crate::EMBEDDING_DIMS;
    if v.len() < target {
        v.resize(target, 0.0);
    } else if v.len() > target {
        v.truncate(target);
    }
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

/// Canonical persona topic / interest / detected-tech strings.
///
/// Mirrors EXACTLY the strings used by `personas.rs` (interest topics in their
/// original case + the lowercase ACE `active_topics` / `detected_tech`). The
/// loader stores each under both its exact key and its lowercase variant, so the
/// semantic boost (`compute_semantic_ace_boost`) resolves every lookup.
const PERSONA_TOPICS: &[&str] = &[
    // Interest topics (original case)
    "Rust",
    "systems programming",
    "Tauri",
    "SQLite",
    "WebAssembly",
    "Machine Learning",
    "Python",
    "LLM",
    "PyTorch",
    "data science",
    "TypeScript",
    "React",
    "Node.js",
    "Next.js",
    "GraphQL",
    "Kubernetes",
    "kubernetes operator",
    "Docker",
    "Terraform",
    "observability stack",
    "eBPF tracing",
    "Prometheus metrics",
    "SRE",
    "React Native",
    "mobile development",
    "Expo",
    "iOS",
    "Android",
    "distributed systems",
    "AI",
    "databases",
    "Go",
    "backend",
    "microservices",
    "Haskell",
    "functional programming",
    "type theory",
    "category theory",
    "Nix",
    "monad",
    "type system",
    // ACE active_topics / detected_tech (lowercase)
    "rust",
    "tauri",
    "sqlite",
    "python",
    "pytorch",
    "machine learning",
    "typescript",
    "react",
    "nextjs",
    "nodejs",
    "react native",
    "expo",
    "mobile",
    "go",
    "grpc",
    "haskell",
    "nix",
    "ghc",
    "cabal",
    "kubernetes",
    "docker",
    "terraform",
    "prometheus",
    "ci/cd",
    "observability",
];

#[test]
#[ignore = "requires fastembed model; run explicitly to regenerate committed .bin fixtures"]
fn generate_real_embedding_fixtures() {
    // --- Corpus item embeddings (keyed by id) ---
    let items = corpus();
    let texts: Vec<String> = items
        .iter()
        .map(|it| format!("{} {}", it.title, it.content))
        .collect();

    let raw = crate::fastembed_sync(&texts).expect("fastembed must embed corpus texts");
    assert_eq!(raw.len(), items.len(), "one embedding per corpus item");

    let corpus_records: Vec<(u32, Vec<f32>)> = items
        .iter()
        .zip(raw.into_iter())
        .map(|(it, v)| (it.id as u32, pad_and_normalize(v)))
        .collect();

    let corpus_bytes =
        fixtures_io::serialize_u32_keyed(crate::EMBEDDING_DIMS as u32, &corpus_records);
    let corpus_path =
        fixtures_io::write_fixture("corpus_embeddings.bin", &corpus_bytes).expect("write corpus");

    // --- Persona topic embeddings (keyed by string, exact + lowercase) ---
    let topic_texts: Vec<String> = PERSONA_TOPICS.iter().map(|s| (*s).to_string()).collect();
    let topic_raw =
        crate::fastembed_sync(&topic_texts).expect("fastembed must embed persona topics");
    assert_eq!(topic_raw.len(), PERSONA_TOPICS.len());

    let mut topic_records: Vec<(String, Vec<f32>)> = Vec::with_capacity(PERSONA_TOPICS.len() * 2);
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (name, v) in PERSONA_TOPICS.iter().zip(topic_raw.into_iter()) {
        let vec = pad_and_normalize(v);
        let exact = (*name).to_string();
        if seen.insert(exact.clone()) {
            topic_records.push((exact.clone(), vec.clone()));
        }
        let lower = name.to_lowercase();
        if lower != *name && seen.insert(lower.clone()) {
            topic_records.push((lower, vec));
        }
    }

    let topic_bytes =
        fixtures_io::serialize_str_keyed(crate::EMBEDDING_DIMS as u32, &topic_records);
    let topic_path =
        fixtures_io::write_fixture("topic_embeddings.bin", &topic_bytes).expect("write topics");

    println!(
        "Wrote {} corpus embeddings -> {}",
        corpus_records.len(),
        corpus_path.display()
    );
    println!(
        "Wrote {} topic embeddings  -> {}",
        topic_records.len(),
        topic_path.display()
    );
}
