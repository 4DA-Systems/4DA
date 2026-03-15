//! Validation Personas — 10 developer archetypes for scoring validation
//!
//! Each persona defines interests, tech stack, dependencies, expected topics,
//! and anti-topics. These are used to build ScoringContexts and auto-judge
//! whether scored items are truly relevant to the persona.

use std::collections::HashSet;

use super::super::ace_context::ACEContext;
use super::super::ScoringContext;

// ============================================================================
// Persona Definition
// ============================================================================

/// A simulated developer profile for validation.
pub struct SimulatedPersona {
    pub name: &'static str,
    pub interests: Vec<(&'static str, f32)>,
    pub tech_stack: Vec<&'static str>,
    pub dependencies: Vec<&'static str>,
    pub role: &'static str,
    pub expected_topics: Vec<&'static str>,
    pub anti_topics: Vec<&'static str>,
}

// ============================================================================
// Persona Builders → ScoringContext
// ============================================================================

fn make_interests(topics: &[(&str, f32)]) -> Vec<crate::context_engine::Interest> {
    let emb = vec![0.5_f32; 384];
    topics
        .iter()
        .enumerate()
        .map(|(i, (t, w))| crate::context_engine::Interest {
            id: Some((i + 1) as i64),
            topic: t.to_string(),
            weight: *w,
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        })
        .collect()
}

fn make_domain(
    primary: &[&str],
    adjacent: &[&str],
    deps: &[&str],
    interest_topics: &[&str],
) -> crate::domain_profile::DomainProfile {
    let ps: HashSet<String> = primary.iter().map(|s| s.to_string()).collect();
    let adj: HashSet<String> = adjacent.iter().map(|s| s.to_string()).collect();
    let mut all = ps.clone();
    all.extend(adj.clone());
    crate::domain_profile::DomainProfile {
        primary_stack: ps,
        adjacent_tech: adj,
        all_tech: all,
        dependency_names: deps.iter().map(|s| s.to_string()).collect(),
        interest_topics: interest_topics.iter().map(|s| s.to_string()).collect(),
        domain_concerns: HashSet::new(),
    }
}

/// Build a ScoringContext from a SimulatedPersona.
pub fn persona_to_context(persona: &SimulatedPersona) -> ScoringContext {
    let interests = make_interests(&persona.interests);

    let mut ace = ACEContext::default();
    for t in &persona.tech_stack {
        ace.active_topics.push(t.to_lowercase());
        ace.detected_tech.push(t.to_lowercase());
    }
    // Add expected topics as active topics (broader coverage)
    for t in &persona.expected_topics {
        let lower = t.to_lowercase();
        if !ace.active_topics.contains(&lower) {
            ace.active_topics.push(lower);
        }
    }

    let primary: Vec<&str> = persona.tech_stack.to_vec();
    let adjacent: Vec<&str> = persona
        .expected_topics
        .iter()
        .filter(|t| !persona.tech_stack.contains(t))
        .take(5)
        .copied()
        .collect();
    let interest_topics: Vec<&str> = persona.interests.iter().map(|(t, _)| *t).collect();

    let domain = make_domain(&primary, &adjacent, &persona.dependencies, &interest_topics);

    let declared_tech: Vec<String> = persona
        .tech_stack
        .iter()
        .take(4)
        .map(|s| s.to_lowercase())
        .collect();

    let exclusions: Vec<String> = persona.anti_topics.iter().map(|s| s.to_string()).collect();

    ScoringContext::builder()
        .interest_count(persona.interests.len())
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(declared_tech)
        .exclusions(exclusions)
        .feedback_interaction_count(50)
        .build()
}

// ============================================================================
// 10 Validation Personas
// ============================================================================

/// Persona 1: Rust Systems Developer
fn rust_systems_dev() -> SimulatedPersona {
    SimulatedPersona {
        name: "Rust Systems Dev",
        interests: vec![
            ("Rust", 1.0),
            ("systems programming", 1.0),
            ("async runtime", 0.9),
            ("memory safety", 0.8),
            ("performance optimization", 0.7),
        ],
        tech_stack: vec!["rust", "tokio", "serde", "wasm"],
        dependencies: vec!["tokio", "serde", "axum", "sqlx", "tracing"],
        role: "systems",
        expected_topics: vec![
            "rust",
            "cargo",
            "tokio",
            "async",
            "memory safety",
            "ownership",
            "borrow checker",
            "wasm",
            "systems programming",
            "performance",
            "concurrency",
            "serde",
            "trait",
            "lifetime",
        ],
        anti_topics: vec![
            "javascript",
            "css",
            "react",
            "marketing",
            "seo",
            "cryptocurrency",
            "nft",
            "blockchain",
        ],
    }
}

/// Persona 2: React Frontend Developer
fn react_frontend_dev() -> SimulatedPersona {
    SimulatedPersona {
        name: "React Frontend Dev",
        interests: vec![
            ("React", 1.0),
            ("TypeScript", 1.0),
            ("CSS", 0.8),
            ("UI/UX", 0.7),
            ("web performance", 0.8),
        ],
        tech_stack: vec!["react", "typescript", "nextjs", "tailwind"],
        dependencies: vec!["react", "next", "typescript", "tailwindcss", "prisma"],
        role: "frontend",
        expected_topics: vec![
            "react",
            "typescript",
            "nextjs",
            "css",
            "tailwind",
            "hooks",
            "components",
            "rendering",
            "ssr",
            "web vitals",
            "accessibility",
            "ui",
            "ux",
            "frontend",
            "javascript",
        ],
        anti_topics: vec![
            "kernel",
            "cuda",
            "kubernetes",
            "terraform",
            "cryptocurrency",
            "blockchain",
        ],
    }
}

/// Persona 3: ML Engineer
fn ml_engineer() -> SimulatedPersona {
    SimulatedPersona {
        name: "ML Engineer",
        interests: vec![
            ("machine learning", 1.0),
            ("Python", 1.0),
            ("PyTorch", 0.9),
            ("transformers", 0.9),
            ("MLOps", 0.7),
        ],
        tech_stack: vec!["python", "pytorch", "cuda", "huggingface"],
        dependencies: vec!["torch", "transformers", "numpy", "pandas", "scikit-learn"],
        role: "ml",
        expected_topics: vec![
            "python",
            "pytorch",
            "machine learning",
            "deep learning",
            "llm",
            "transformer",
            "gpu",
            "cuda",
            "training",
            "inference",
            "model",
            "neural network",
            "fine-tuning",
            "embedding",
            "mlops",
        ],
        anti_topics: vec![
            "css",
            "react",
            "frontend",
            "mobile",
            "ios",
            "cryptocurrency",
            "nft",
        ],
    }
}

/// Persona 4: DevOps/Platform Engineer
fn devops_platform() -> SimulatedPersona {
    SimulatedPersona {
        name: "DevOps/Platform Engineer",
        interests: vec![
            ("Kubernetes", 1.0),
            ("Terraform", 0.9),
            ("CI/CD", 0.9),
            ("monitoring", 0.8),
            ("cloud infrastructure", 0.8),
        ],
        tech_stack: vec!["kubernetes", "docker", "terraform", "prometheus"],
        dependencies: vec!["helm", "terraform", "prometheus", "grafana", "ansible"],
        role: "devops",
        expected_topics: vec![
            "kubernetes",
            "docker",
            "terraform",
            "ci/cd",
            "monitoring",
            "observability",
            "prometheus",
            "grafana",
            "helm",
            "aws",
            "gcp",
            "cloud",
            "infrastructure",
            "deployment",
            "container",
        ],
        anti_topics: vec!["css", "react", "ui", "game", "unity", "cryptocurrency"],
    }
}

/// Persona 5: Mobile Developer
fn mobile_dev() -> SimulatedPersona {
    SimulatedPersona {
        name: "Mobile Developer",
        interests: vec![
            ("Swift", 0.9),
            ("Kotlin", 0.9),
            ("React Native", 1.0),
            ("Flutter", 0.8),
            ("mobile UX", 0.7),
        ],
        tech_stack: vec!["react native", "swift", "kotlin", "flutter"],
        dependencies: vec!["react-native", "expo", "swift", "kotlin"],
        role: "mobile",
        expected_topics: vec![
            "mobile",
            "ios",
            "android",
            "swift",
            "kotlin",
            "react native",
            "flutter",
            "expo",
            "app store",
            "push notification",
            "gesture",
            "navigation",
            "responsive",
        ],
        anti_topics: vec![
            "kernel",
            "cuda",
            "kubernetes",
            "terraform",
            "cryptocurrency",
            "blockchain",
            "backend",
        ],
    }
}

/// Persona 6: Security Engineer
fn security_engineer() -> SimulatedPersona {
    SimulatedPersona {
        name: "Security Engineer",
        interests: vec![
            ("vulnerability", 1.0),
            ("cryptography", 0.9),
            ("penetration testing", 0.8),
            ("supply chain security", 0.9),
            ("zero trust", 0.7),
        ],
        tech_stack: vec!["security", "cryptography", "pentest", "sast"],
        dependencies: vec!["openssl", "ring", "rustls", "nmap"],
        role: "security",
        expected_topics: vec![
            "security",
            "vulnerability",
            "cve",
            "exploit",
            "authentication",
            "encryption",
            "tls",
            "pentest",
            "supply chain",
            "zero trust",
            "audit",
            "compliance",
            "malware",
            "xss",
            "injection",
        ],
        anti_topics: vec!["css", "ui", "design", "marketing", "game", "pricing"],
    }
}

/// Persona 7: Data Engineer
fn data_engineer() -> SimulatedPersona {
    SimulatedPersona {
        name: "Data Engineer",
        interests: vec![
            ("SQL", 1.0),
            ("data pipelines", 1.0),
            ("Spark", 0.8),
            ("Kafka", 0.9),
            ("dbt", 0.8),
        ],
        tech_stack: vec!["sql", "spark", "kafka", "dbt"],
        dependencies: vec!["pyspark", "kafka", "dbt-core", "airflow", "snowflake"],
        role: "data",
        expected_topics: vec![
            "sql",
            "data pipeline",
            "etl",
            "spark",
            "kafka",
            "dbt",
            "data warehouse",
            "streaming",
            "batch processing",
            "schema",
            "parquet",
            "airflow",
            "snowflake",
            "data lake",
        ],
        anti_topics: vec![
            "css",
            "react",
            "frontend",
            "mobile",
            "game",
            "cryptocurrency",
        ],
    }
}

/// Persona 8: Indie Hacker / Solo SaaS Developer
fn indie_hacker() -> SimulatedPersona {
    SimulatedPersona {
        name: "Indie Hacker",
        interests: vec![
            ("SaaS", 1.0),
            ("pricing strategy", 0.9),
            ("growth hacking", 0.8),
            ("marketing", 0.8),
            ("revenue", 0.9),
        ],
        tech_stack: vec!["typescript", "stripe", "vercel", "postgres"],
        dependencies: vec!["stripe", "next", "supabase", "resend"],
        role: "indie",
        expected_topics: vec![
            "saas",
            "pricing",
            "revenue",
            "growth",
            "marketing",
            "launch",
            "conversion",
            "churn",
            "mrr",
            "startup",
            "bootstrapped",
            "stripe",
            "landing page",
        ],
        anti_topics: vec![
            "kernel",
            "cuda",
            "kubernetes",
            "terraform",
            "compiler",
            "assembly",
        ],
    }
}

/// Persona 9: Game Developer
fn game_dev() -> SimulatedPersona {
    SimulatedPersona {
        name: "Game Developer",
        interests: vec![
            ("Unity", 0.9),
            ("Unreal Engine", 0.8),
            ("Godot", 0.9),
            ("graphics programming", 1.0),
            ("physics simulation", 0.7),
        ],
        tech_stack: vec!["unity", "godot", "opengl", "vulkan"],
        dependencies: vec!["unity", "godot", "glfw", "sdl2"],
        role: "gamedev",
        expected_topics: vec![
            "game",
            "unity",
            "unreal",
            "godot",
            "graphics",
            "shader",
            "rendering",
            "physics",
            "game engine",
            "opengl",
            "vulkan",
            "3d",
            "animation",
            "game design",
        ],
        anti_topics: vec![
            "kubernetes",
            "terraform",
            "ci/cd",
            "data pipeline",
            "marketing",
            "seo",
        ],
    }
}

/// Persona 10: Backend Java/Go Developer
fn backend_java_go() -> SimulatedPersona {
    SimulatedPersona {
        name: "Backend Java/Go Dev",
        interests: vec![
            ("microservices", 1.0),
            ("gRPC", 0.9),
            ("databases", 0.9),
            ("Go", 0.9),
            ("Java", 0.8),
        ],
        tech_stack: vec!["go", "java", "grpc", "postgres"],
        dependencies: vec!["gin", "spring", "grpc", "postgres", "redis"],
        role: "backend",
        expected_topics: vec![
            "go",
            "golang",
            "java",
            "spring",
            "microservices",
            "grpc",
            "api",
            "database",
            "postgres",
            "redis",
            "kafka",
            "backend",
            "rest",
            "distributed",
        ],
        anti_topics: vec!["css", "react", "frontend", "mobile", "ios", "game", "unity"],
    }
}

// ============================================================================
// Collector
// ============================================================================

/// Returns all 10 validation personas.
pub fn all_validation_personas() -> Vec<SimulatedPersona> {
    vec![
        rust_systems_dev(),
        react_frontend_dev(),
        ml_engineer(),
        devops_platform(),
        mobile_dev(),
        security_engineer(),
        data_engineer(),
        indie_hacker(),
        game_dev(),
        backend_java_go(),
    ]
}

/// Returns all 10 personas as (SimulatedPersona, ScoringContext) pairs.
pub fn all_validation_contexts() -> Vec<(SimulatedPersona, ScoringContext)> {
    all_validation_personas()
        .into_iter()
        .map(|p| {
            let ctx = persona_to_context(&p);
            (p, ctx)
        })
        .collect()
}
