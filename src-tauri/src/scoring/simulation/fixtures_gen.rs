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
//! It embeds every `corpus()` item via the production `build_embedding_text`
//! contract and every persona interest / ACE topic / detected-tech string (as a
//! short canonical description) via the SAME fastembed path that
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

/// Canonical persona topic / interest / detected-tech strings, each paired with a
/// short canonical DESCRIPTION that is what actually gets embedded.
///
/// `.0` is the lookup KEY — it mirrors EXACTLY the strings `personas.rs` uses
/// (interest topics in their original case + the lowercase ACE `active_topics` /
/// `detected_tech`), so the loader's exact + lowercase keying resolves every lookup
/// unchanged. `.1` is the embedded TEXT. Bare labels ("Go", "AI", "LLM") are the
/// noisiest possible embedding regime (1-3 char tokens); embedding a short
/// description instead moves the user-side vector into the sentence regime where the
/// model produces stable, separable vectors — without touching what personas.rs
/// looks up. The loader stores each vector under both the exact key and its
/// lowercase variant so the semantic boost (`compute_semantic_ace_boost`) resolves.
const PERSONA_TOPICS: &[(&str, &str)] = &[
    // Interest topics (original case)
    ("Rust", "Rust programming language: memory safety, ownership, borrow checker, systems programming"),
    ("systems programming", "systems programming: low-level performance, memory management, concurrency, operating systems"),
    ("Tauri", "Tauri desktop application framework with a Rust backend and a webview frontend"),
    ("SQLite", "SQLite embedded relational database, SQL, local-first storage"),
    ("WebAssembly", "WebAssembly (Wasm): portable bytecode runtime with near-native performance in the browser"),
    ("Machine Learning", "machine learning: neural networks, model training, deep learning, inference"),
    ("Python", "Python programming language: scripting, data science, machine learning, backends"),
    ("LLM", "large language models (LLM): transformers, GPT, prompting, fine-tuning, inference"),
    ("PyTorch", "PyTorch deep learning framework: tensors, autograd, neural network training"),
    ("data science", "data science: data analysis, pandas, statistics, visualization, notebooks"),
    ("TypeScript", "TypeScript: statically typed JavaScript for frontend and backend development"),
    ("React", "React UI library: components, hooks, state management, frontend web development"),
    ("Node.js", "Node.js JavaScript runtime: server-side, npm, APIs, backend development"),
    ("Next.js", "Next.js React framework: server-side rendering, routing, full-stack web apps"),
    ("GraphQL", "GraphQL API query language: schemas, resolvers, typed data fetching"),
    ("Kubernetes", "Kubernetes container orchestration: pods, clusters, deployments, autoscaling"),
    ("kubernetes operator", "Kubernetes operator pattern: custom resources, controllers, cluster automation"),
    ("Docker", "Docker containers: images, containerization, packaging, deployment"),
    ("Terraform", "Terraform infrastructure as code: declaratively provisioning cloud resources"),
    ("observability stack", "observability stack: metrics, logging, distributed tracing, monitoring dashboards"),
    ("eBPF tracing", "eBPF kernel tracing: performance profiling, networking, low-overhead observability"),
    ("Prometheus metrics", "Prometheus metrics: time-series monitoring, scraping, alerting, dashboards"),
    ("SRE", "site reliability engineering (SRE): uptime, incident response, automation, on-call"),
    ("React Native", "React Native: cross-platform iOS and Android mobile app development in JavaScript"),
    ("mobile development", "mobile application development: iOS, Android, native and cross-platform apps"),
    ("Expo", "Expo: React Native toolchain for building and deploying mobile apps"),
    ("iOS", "iOS Apple mobile development: Swift, iPhone and iPad apps, Xcode"),
    ("Android", "Android mobile development: Kotlin, Jetpack, the Google mobile platform"),
    ("distributed systems", "distributed systems: consensus, replication, fault tolerance, scalability, consistency"),
    ("AI", "artificial intelligence (AI): machine learning, neural networks, large language models"),
    ("databases", "databases: SQL, query optimization, indexing, storage engines, transactions"),
    ("Go", "Go (golang) programming language: goroutines, concurrency, backend services, networking"),
    ("backend", "backend development: servers, APIs, databases, scalability, distributed services"),
    ("microservices", "microservices architecture: independent services, APIs, messaging, scalability"),
    ("Haskell", "Haskell: pure functional programming, strong static types, laziness, type classes"),
    ("functional programming", "functional programming: immutability, pure functions, higher-order functions, composition"),
    ("type theory", "type theory: type systems, formal logic, programming language foundations"),
    ("category theory", "category theory: functors, monads, natural transformations, mathematical abstraction"),
    ("Nix", "Nix: reproducible builds, declarative package management and configuration"),
    ("monad", "monad: a functional programming abstraction for composing effects and sequencing computation"),
    ("type system", "type system: static typing, type checking, type inference, type safety"),
    // ACE active_topics / detected_tech (lowercase). Most duplicate a capitalized
    // interest above and are deduped by the loader; the genuinely-new ones (grpc,
    // nextjs, nodejs, ghc, cabal, ci/cd, observability, mobile) carry their own text.
    ("rust", "Rust programming language: memory safety, ownership, systems programming"),
    ("tauri", "Tauri desktop app framework with a Rust backend and webview frontend"),
    ("sqlite", "SQLite embedded relational database, local-first SQL storage"),
    ("python", "Python programming language: scripting, data science, machine learning"),
    ("pytorch", "PyTorch deep learning framework: tensors, autograd, training"),
    ("machine learning", "machine learning: neural networks, model training, deep learning"),
    ("typescript", "TypeScript: statically typed JavaScript for web development"),
    ("react", "React UI library: components, hooks, frontend web development"),
    ("nextjs", "Next.js React framework: server-side rendering, full-stack web apps"),
    ("nodejs", "Node.js JavaScript server runtime: backend, npm, APIs"),
    ("react native", "React Native: cross-platform mobile app development"),
    ("expo", "Expo: React Native toolchain for mobile apps"),
    ("mobile", "mobile application development: iOS, Android, native and cross-platform"),
    ("go", "Go (golang) programming language: goroutines, concurrency, backend services"),
    ("grpc", "gRPC: high-performance remote procedure calls over protobuf for microservices"),
    ("haskell", "Haskell: pure functional programming with strong static types"),
    ("nix", "Nix: reproducible builds and declarative package management"),
    ("ghc", "GHC: the Glasgow Haskell Compiler toolchain"),
    ("cabal", "Cabal: the Haskell build tool and package manager"),
    ("kubernetes", "Kubernetes container orchestration: pods, clusters, deployments"),
    ("docker", "Docker containers: images, containerization, deployment"),
    ("terraform", "Terraform infrastructure as code for cloud provisioning"),
    ("prometheus", "Prometheus metrics: time-series monitoring and alerting"),
    ("ci/cd", "CI/CD: continuous integration and deployment pipelines, build automation"),
    ("observability", "observability: metrics, logs, distributed traces, monitoring"),
];

#[test]
#[ignore = "requires fastembed model; run explicitly to regenerate committed .bin fixtures"]
fn generate_real_embedding_fixtures() {
    // --- Corpus item embeddings (keyed by id) ---
    // Use the EXACT production embedding contract (`build_embedding_text`: title
    // repeated 2x + `\n\n` joins + preprocess_content) so the fixture measures what
    // production actually embeds — not a bespoke `"{title} {content}"` join that
    // under-weights the title and diverges from real-user behaviour.
    let items = corpus();
    let texts: Vec<String> = items
        .iter()
        .map(|it| crate::build_embedding_text(it.title, it.content))
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

    // --- Persona topic embeddings (keyed by lookup string, exact + lowercase) ---
    // Embed the DESCRIPTION (`.1`), key by the LOOKUP string (`.0`) so personas.rs
    // lookups are byte-for-byte unchanged while the stored vector leaves the noisy
    // bare-token regime.
    let topic_texts: Vec<String> = PERSONA_TOPICS
        .iter()
        .map(|(_, desc)| (*desc).to_string())
        .collect();
    let topic_raw =
        crate::fastembed_sync(&topic_texts).expect("fastembed must embed persona topics");
    assert_eq!(topic_raw.len(), PERSONA_TOPICS.len());

    let mut topic_records: Vec<(String, Vec<f32>)> = Vec::with_capacity(PERSONA_TOPICS.len() * 2);
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ((key, _desc), v) in PERSONA_TOPICS.iter().zip(topic_raw.into_iter()) {
        let vec = pad_and_normalize(v);
        let exact = (*key).to_string();
        if seen.insert(exact.clone()) {
            topic_records.push((exact.clone(), vec.clone()));
        }
        let lower = key.to_lowercase();
        if lower != *key && seen.insert(lower.clone()) {
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
