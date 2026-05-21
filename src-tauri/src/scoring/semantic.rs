// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use tracing::debug;

use super::ace_context::ACEContext;
use super::utils::topic_overlaps;
use crate::{ace, embed_texts, get_ace_engine, scoring_config};
use fourda_macros::score_component;

/// Compute semantic ACE boost using embeddings
/// PASIFA: Uses vector similarity instead of keyword matching when embeddings available
pub(crate) fn compute_semantic_ace_boost(
    item_embedding: &[f32],
    ace_ctx: &ACEContext,
    topic_embeddings: &HashMap<String, Vec<f32>>,
) -> Option<f32> {
    if topic_embeddings.is_empty() {
        return None; // Fall back to keyword matching
    }

    // Pre-compute item embedding norm once (hot loop optimization)
    let item_norm = crate::vector_norm(item_embedding);
    if item_norm < f32::EPSILON {
        return None; // Zero-norm embedding can't produce meaningful similarity
    }

    let mut max_similarity: f32 = 0.0;
    let mut weighted_sum: f32 = 0.0;
    let mut weight_total: f32 = 0.0;

    // Compute similarity with active topics
    for topic in &ace_ctx.active_topics {
        if let Some(topic_emb) = topic_embeddings.get(topic) {
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, topic_emb);
            let conf = ace_ctx.topic_confidence.get(topic).copied().unwrap_or(0.5);
            weighted_sum += sim * conf;
            weight_total += conf;
            max_similarity = max_similarity.max(sim);
        }
    }

    // Compute similarity with detected tech (per-project weighted)
    // Primary project tech → 0.85 weight, secondary → 0.40 (from ace_ctx.tech_weights)
    for tech in &ace_ctx.detected_tech {
        if let Some(tech_emb) = topic_embeddings.get(tech) {
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, tech_emb);
            let tech_weight = ace_ctx.tech_weights.get(tech).copied().unwrap_or(0.35);
            weighted_sum += sim * tech_weight;
            weight_total += tech_weight;
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
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, topic_emb);
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

/// Enrich a bare topic/tech name with a descriptive sentence for higher-quality embeddings.
///
/// Bare names like "Rust" or "Go" are ambiguous — "Rust" could mean the game, the chemical
/// process, or the programming language. This function maps known developer topics to concise
/// technical descriptions (15-30 words) that disambiguate meaning for the embedding model.
///
/// Unknown topics get a generic but still-helpful fallback description.
pub(crate) fn enrich_topic_for_embedding(topic: &str) -> String {
    // Case-insensitive lookup: normalize to lowercase for matching
    let key = topic.to_lowercase();

    // Curated descriptions — each one disambiguates the topic in a software-dev context.
    // Sorted alphabetically for maintainability. ~100 entries, all &str — zero heap alloc on match.
    let description: Option<&str> = match key.as_str() {
        "actix" => Some("Actix Web — high-performance Rust web framework built on the actor model with async support"),
        "angular" => Some("Angular — TypeScript-based frontend framework by Google for building structured single-page applications"),
        "ansible" => Some("Ansible — agentless IT automation tool for configuration management, application deployment, and orchestration"),
        "anthropic" => Some("Anthropic — AI safety company developing the Claude family of large language models"),
        "apache" => Some("Apache HTTP Server — widely deployed open-source web server for serving static and dynamic content"),
        "aws" => Some("Amazon Web Services — cloud computing platform offering compute, storage, database, and AI services"),
        "axum" => Some("Axum — ergonomic Rust web framework built on Tokio and Tower for building async HTTP services"),
        "azure" => Some("Microsoft Azure — cloud computing platform for building, deploying, and managing applications and services"),
        "bun" => Some("Bun — fast all-in-one JavaScript runtime, bundler, transpiler, and package manager written in Zig"),
        "c#" => Some("C# — statically typed object-oriented language by Microsoft for .NET platform, game dev, and enterprise software"),
        "c++" => Some("C++ — high-performance compiled language for systems programming, game engines, and performance-critical software"),
        "caddy" => Some("Caddy — modern web server with automatic HTTPS, HTTP/2, and simple configuration via Caddyfile"),
        "cargo" => Some("Cargo — Rust's package manager and build system for managing dependencies, compiling, and publishing crates"),
        "cuda" => Some("CUDA — NVIDIA's parallel computing platform and API for GPU-accelerated computing and deep learning"),
        "cypress" => Some("Cypress — JavaScript end-to-end testing framework for web applications with real browser execution"),
        "deno" => Some("Deno — secure JavaScript and TypeScript runtime by Ryan Dahl with built-in tooling and web-standard APIs"),
        "diesel" => Some("Diesel — safe, extensible Rust ORM and query builder with compile-time query validation"),
        "directx" => Some("DirectX — Microsoft's collection of APIs for multimedia and game programming on Windows platforms"),
        "django" => Some("Django — batteries-included Python web framework with ORM, admin panel, and security best practices"),
        "docker" => Some("Docker — container platform for packaging applications with dependencies into portable, isolated environments"),
        "dynamodb" => Some("DynamoDB — AWS fully managed NoSQL key-value database designed for high-throughput low-latency workloads"),
        "elasticsearch" => Some("Elasticsearch — distributed search and analytics engine for full-text search, log analysis, and observability"),
        "elixir" => Some("Elixir — functional language on the Erlang VM for scalable fault-tolerant distributed systems"),
        "eslint" => Some("ESLint — pluggable JavaScript and TypeScript linter for finding and fixing code quality issues"),
        "express" => Some("Express — minimal Node.js web framework for building RESTful APIs and server-side applications"),
        "fastapi" => Some("FastAPI — modern high-performance Python web framework for building APIs with automatic OpenAPI documentation"),
        "fastembed" => Some("FastEmbed — lightweight library for generating text embeddings locally using ONNX Runtime models"),
        "flask" => Some("Flask — lightweight Python web micro-framework for building APIs and web applications with minimal boilerplate"),
        "gcp" => Some("Google Cloud Platform — cloud computing services for compute, storage, machine learning, and data analytics"),
        "git" => Some("Git — distributed version control system for tracking source code changes and collaborative development"),
        "github" => Some("GitHub — cloud platform for Git repository hosting, code review, CI/CD, and open-source collaboration"),
        "go" => Some("Go — statically typed compiled language by Google designed for concurrency, simplicity, and cloud-native services"),
        "grafana" => Some("Grafana — open-source observability platform for visualizing metrics, logs, and traces from multiple data sources"),
        "graphql" => Some("GraphQL — query language and runtime for APIs allowing clients to request exactly the data they need"),
        "grpc" => Some("gRPC — high-performance RPC framework using Protocol Buffers for efficient service-to-service communication"),
        "haskell" => Some("Haskell — purely functional programming language with strong static typing and lazy evaluation for robust software"),
        "hugging face" => Some("Hugging Face — AI platform and hub for sharing pretrained machine learning models, datasets, and transformers"),
        "hyper" => Some("Hyper — fast and correct HTTP implementation in Rust, used as the foundation for many Rust web frameworks"),
        "java" => Some("Java — object-oriented language on the JVM for enterprise applications, Android development, and large-scale systems"),
        "jest" => Some("Jest — JavaScript testing framework by Meta with snapshot testing, mocking, and zero-configuration setup"),
        "kafka" => Some("Apache Kafka — distributed event streaming platform for high-throughput real-time data pipelines and messaging"),
        "kotlin" => Some("Kotlin — modern JVM language by JetBrains for Android development and server-side applications with null safety"),
        "kubernetes" => Some("Kubernetes — container orchestration system for automating deployment, scaling, and management of containerized applications"),
        "langchain" => Some("LangChain — framework for building applications powered by large language models with chains, agents, and retrieval"),
        "laravel" => Some("Laravel — PHP web framework with elegant syntax, ORM, queue system, and comprehensive ecosystem for web applications"),
        "linux" => Some("Linux — open-source Unix-like operating system kernel powering servers, containers, embedded systems, and cloud infrastructure"),
        "llamaindex" => Some("LlamaIndex — data framework for connecting large language models with external data sources via retrieval-augmented generation"),
        "metal" => Some("Metal — Apple's low-level GPU programming API for high-performance graphics and compute on macOS and iOS"),
        "mongodb" => Some("MongoDB — document-oriented NoSQL database storing data as flexible JSON-like documents for modern applications"),
        "mysql" => Some("MySQL — open-source relational database management system widely used for web applications and data storage"),
        "nats" => Some("NATS — lightweight high-performance messaging system for cloud-native distributed systems and microservices"),
        "next.js" => Some("Next.js — React framework by Vercel for server-side rendering, static generation, and full-stack web applications"),
        "nginx" => Some("Nginx — high-performance HTTP server and reverse proxy for load balancing, caching, and serving web content"),
        "node.js" => Some("Node.js — JavaScript runtime built on V8 for building scalable server-side and networking applications"),
        "npm" => Some("npm — JavaScript package manager and registry for sharing and installing reusable code modules"),
        "numpy" => Some("NumPy — foundational Python library for numerical computing with support for large multi-dimensional arrays and matrices"),
        "ollama" => Some("Ollama — tool for running large language models locally on consumer hardware with a simple API interface"),
        "onnx" => Some("ONNX — open standard format for representing machine learning models enabling interoperability between frameworks"),
        "openai" => Some("OpenAI — AI research company developing the GPT family of large language models and the ChatGPT platform"),
        "opengl" => Some("OpenGL — cross-platform graphics API for rendering 2D and 3D vector graphics in interactive applications"),
        "pandas" => Some("pandas — Python data analysis library providing DataFrames for manipulating structured and tabular data"),
        "php" => Some("PHP — server-side scripting language widely used for web development, powering WordPress and many web applications"),
        "pip" => Some("pip — Python package installer for downloading and managing libraries from the Python Package Index"),
        "playwright" => Some("Playwright — cross-browser end-to-end testing framework by Microsoft for reliable web application testing"),
        "postgresql" => Some("PostgreSQL — advanced open-source relational database with extensibility, JSONB support, and ACID compliance"),
        "prettier" => Some("Prettier — opinionated code formatter supporting JavaScript, TypeScript, CSS, and other web languages"),
        "prometheus" => Some("Prometheus — open-source monitoring system with time-series database for alerting and metrics collection"),
        "python" => Some("Python — high-level interpreted language for web development, data science, machine learning, and automation scripting"),
        "pytorch" => Some("PyTorch — open-source deep learning framework by Meta for building and training neural networks with dynamic computation graphs"),
        "rabbitmq" => Some("RabbitMQ — open-source message broker implementing AMQP for reliable asynchronous communication between services"),
        "rails" => Some("Ruby on Rails — convention-over-configuration web framework for building database-backed web applications in Ruby"),
        "react" => Some("React — JavaScript UI library by Meta for building component-based interactive user interfaces with a virtual DOM"),
        "redis" => Some("Redis — in-memory data structure store used as a cache, message broker, and real-time database"),
        "rest" => Some("REST — architectural style for designing networked applications using stateless HTTP methods and resource URLs"),
        "rocm" => Some("ROCm — AMD's open-source GPU computing platform for machine learning and high-performance computing workloads"),
        "ruby" => Some("Ruby — dynamic interpreted language focused on developer happiness, known for elegant syntax and metaprogramming"),
        "rust" => Some("Rust — systems programming language focused on memory safety, concurrency, and performance without garbage collection"),
        "scala" => Some("Scala — JVM language combining object-oriented and functional programming for scalable concurrent applications"),
        "scikit-learn" => Some("scikit-learn — Python machine learning library providing classification, regression, clustering, and preprocessing algorithms"),
        "seaorm" => Some("SeaORM — async and dynamic Rust ORM built on SQLx for database-agnostic application development"),
        "selenium" => Some("Selenium — browser automation framework for web application testing across multiple browsers and platforms"),
        "serde" => Some("Serde — Rust serialization framework for converting data structures to and from JSON, TOML, and other formats"),
        "spring" => Some("Spring — comprehensive Java framework for enterprise application development with dependency injection and microservices"),
        "sqlite" => Some("SQLite — self-contained embedded relational database engine used for local storage in applications and mobile devices"),
        "sqlx" => Some("SQLx — async Rust SQL toolkit with compile-time checked queries and support for PostgreSQL, MySQL, and SQLite"),
        "svelte" => Some("Svelte — compiler-based frontend framework that shifts work to build time for minimal runtime JavaScript"),
        "swift" => Some("Swift — Apple's compiled language for iOS, macOS, watchOS, and server-side development with safety and performance"),
        "tauri" => Some("Tauri — lightweight framework for building desktop applications with web frontends and Rust backends"),
        "tensorflow" => Some("TensorFlow — open-source machine learning platform by Google for building and deploying neural network models at scale"),
        "terraform" => Some("Terraform — infrastructure-as-code tool by HashiCorp for provisioning and managing cloud resources declaratively"),
        "tokio" => Some("Tokio — asynchronous runtime for Rust providing event-driven non-blocking I/O for building network applications"),
        "typescript" => Some("TypeScript — typed superset of JavaScript by Microsoft that compiles to plain JavaScript for safer large-scale development"),
        "vite" => Some("Vite — fast frontend build tool with instant hot module replacement and native ES module dev server"),
        "vitest" => Some("Vitest — fast Vite-native unit testing framework for JavaScript and TypeScript with Jest-compatible API"),
        "vue" => Some("Vue.js — progressive JavaScript framework for building user interfaces with reactive data binding and component composition"),
        "vulkan" => Some("Vulkan — low-overhead cross-platform graphics and compute API for high-performance GPU-accelerated applications"),
        "wasm" | "webassembly" => Some("WebAssembly — portable binary instruction format for executing high-performance code in web browsers and runtimes"),
        "webpack" => Some("Webpack — JavaScript module bundler for transforming, bundling, and packaging web application assets"),
        "websocket" => Some("WebSocket — communication protocol providing full-duplex bidirectional channels over a single TCP connection"),
        _ => None,
    };

    match description {
        Some(desc) => desc.to_string(),
        None => format!("{topic} — software development technology"),
    }
}

/// Embed ACE topics for semantic matching
/// Uses database-persisted embeddings with in-memory cache fallback
/// Returns topic -> embedding map
pub(crate) async fn get_topic_embeddings(ace_ctx: &ACEContext) -> HashMap<String, Vec<f32>> {
    // Lazy static cache for topic embeddings
    use parking_lot::Mutex;
    static TOPIC_EMBEDDING_CACHE: OnceCell<Mutex<HashMap<String, Vec<f32>>>> = OnceCell::new();
    static DB_LOADED: OnceCell<Mutex<bool>> = OnceCell::new();

    let cache = TOPIC_EMBEDDING_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let db_loaded = DB_LOADED.get_or_init(|| Mutex::new(false));

    // Phase 1 (sync): Load DB cache + collect topics needing embedding
    // All MutexGuard usage is scoped here so they drop before any .await
    let topics_to_embed: Vec<String> = {
        let mut cache_guard = cache.lock();
        let mut db_loaded_guard = db_loaded.lock();

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
        let mut needed: Vec<String> = Vec::new();
        for topic in &ace_ctx.active_topics {
            if !cache_guard.contains_key(topic) {
                needed.push(topic.clone());
            }
        }
        for tech in &ace_ctx.detected_tech {
            if !cache_guard.contains_key(tech) {
                needed.push(tech.clone());
            }
        }
        for topic in ace_ctx.topic_affinities.keys() {
            if !cache_guard.contains_key(topic) {
                needed.push(topic.clone());
            }
        }
        for dep_name in ace_ctx.dependency_info.keys() {
            if !cache_guard.contains_key(dep_name) {
                needed.push(dep_name.clone());
            }
        }

        needed
    }; // MutexGuards dropped here - safe to .await below

    // Phase 2 (async): Generate embeddings for missing topics
    // Enrich bare names with descriptive text for higher-quality embeddings
    if !topics_to_embed.is_empty() {
        let batch: Vec<String> = topics_to_embed.into_iter().take(50).collect();
        let batch_len = batch.len();
        let enriched: Vec<String> = batch
            .iter()
            .map(|t| enrich_topic_for_embedding(t))
            .collect();

        if let Ok(embeddings) = embed_texts(&enriched).await {
            // Phase 3 (sync): Store results back into cache
            let mut cache_guard = cache.lock();

            let ace_conn = get_ace_engine().ok().map(|ace| ace.get_conn().clone());
            for (topic, embedding) in batch.into_iter().zip(embeddings.into_iter()) {
                if let Some(ref conn) = ace_conn {
                    if let Err(e) = ace::store_topic_embedding(conn, &topic, &embedding) {
                        tracing::warn!("Failed to store topic embedding: {e}");
                    }
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

    // Phase 4 (sync): Build result from cache
    let cache_guard = cache.lock();

    let mut result = HashMap::new();
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
    for dep_name in ace_ctx.dependency_info.keys() {
        if let Some(emb) = cache_guard.get(dep_name) {
            result.insert(dep_name.clone(), emb.clone());
        }
    }

    result
}

/// Compute taste embedding: weighted centroid of topic affinity embeddings.
///
/// The taste embedding captures the user's holistic preference profile as a single
/// unit vector matching EMBEDDING_DIMS. Items with high cosine similarity to this vector are more
/// likely to match the user's tastes — even if they don't match any individual topic.
///
/// # Arguments
/// * `affinities` - (topic, affinity_score, confidence) triples from ACE behavior learning
/// * `topic_embeddings` - topic -> embedding map (EMBEDDING_DIMS-dim, already loaded) (already loaded)
pub(crate) fn compute_taste_embedding(
    affinities: &[(String, f32, f32)],
    topic_embeddings: &HashMap<String, Vec<f32>>,
) -> Option<Vec<f32>> {
    if affinities.is_empty() || topic_embeddings.is_empty() {
        return None;
    }

    let dim = crate::EMBEDDING_DIMS;
    let mut centroid = vec![0.0f32; dim];
    let mut total_weight = 0.0f32;

    for (topic, affinity, confidence) in affinities {
        if let Some(emb) = topic_embeddings.get(topic) {
            if emb.len() != dim {
                continue;
            }
            // Weight = affinity_score * confidence
            // Positive affinities pull toward liked content
            // Negative affinities push away from disliked content
            let weight = affinity * confidence;
            for (c, e) in centroid.iter_mut().zip(emb.iter()) {
                *c += weight * e;
            }
            total_weight += weight.abs();
        }
    }

    if total_weight < f32::EPSILON {
        return None;
    }

    // Normalize to unit vector for cosine similarity
    let norm = crate::vector_norm(&centroid);
    if norm < f32::EPSILON {
        return None;
    }
    for c in &mut centroid {
        *c /= norm;
    }

    Some(centroid)
}

/// Compute taste similarity between an item embedding and the user's taste embedding.
///
/// Returns a small boost/penalty (clamped to +/-0.08) that personalizes scoring
/// without dominating it. High similarity items get a positive nudge.
pub(crate) fn compute_taste_boost(item_embedding: &[f32], taste_embedding: &[f32]) -> f32 {
    let item_norm = crate::vector_norm(item_embedding);
    if item_norm < f32::EPSILON {
        return 0.0;
    }
    let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, taste_embedding);
    // Center around 0.4 (typical background similarity) and scale
    // sim=0.8 → +0.08, sim=0.4 → 0.0, sim=0.0 → -0.08
    ((sim - 0.4) * 0.2).clamp(-0.08, 0.08)
}

/// Keyword-based ACE boost fallback when embeddings unavailable
/// Both topics (from extract_topics) and ace_ctx fields are already lowercase
#[score_component(output_range = "0.0..=0.3")]
pub(crate) fn compute_keyword_ace_boost(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    let mut boost: f32 = 0.0;
    for topic in topics {
        for active in &ace_ctx.active_topics {
            if topic_overlaps(topic, active) {
                boost += scoring_config::ACE_ACTIVE_TOPIC_BOOST
                    * ace_ctx.topic_confidence.get(active).copied().unwrap_or(0.5);
                break;
            }
        }
        for tech in &ace_ctx.detected_tech {
            if topic_overlaps(topic, tech) {
                boost += scoring_config::ACE_DETECTED_TECH_BOOST;
                break;
            }
        }
    }
    boost.clamp(0.0, scoring_config::ACE_MAX_BOOST)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::seed_embedding;
    use std::collections::HashMap;

    /// Helper: cosine similarity via the crate's norm-based function
    fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
        let a_norm = crate::vector_norm(a);
        crate::cosine_similarity_with_norm(a, a_norm, b)
    }

    /// Helper: build a minimal ACEContext with active topics and confidence
    fn ace_ctx_with_topics(topics: &[(&str, f32)]) -> ACEContext {
        let mut ctx = ACEContext::default();
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
}
