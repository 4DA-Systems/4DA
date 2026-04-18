// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Domain Profile Data — constant arrays, lookup tables, and taxonomy definitions
//!
//! Extracted from domain_profile.rs to keep logic files under size limits.
//! All data here is pure constants — no logic, no functions.

use std::collections::HashMap;

/// Cross-cutting topics that are universally relevant to all developers.
/// These get 0.60 domain relevance instead of 0.15 (off-domain) for any populated profile.
pub(crate) const CROSS_CUTTING_TOPICS: &[&str] = &[
    "architecture",
    "testing",
    "deployment",
    "monitoring",
    "security",
    "performance",
    "accessibility",
    "debugging",
    "refactoring",
    "caching",
    "authentication",
    "authorization",
    "observability",
    "logging",
    "documentation",
    "concurrency",
    "profiling",
    "benchmarking",
    "linting",
    "packaging",
    "migration",
    "open source",
    "design patterns",
    "best practices",
    "code review",
    "unit testing",
    "integration testing",
    "continuous integration",
];

/// Dependency names that exist across multiple ecosystems (Rust `futures` vs C++ futures).
/// When one of these matches, we require corroboration: at least one OTHER topic must also
/// match the user's primary_stack or all_tech. Without corroboration, downgrade to interest-level.
pub(crate) const AMBIGUOUS_DEPS: &[&str] = &[
    "futures",
    "async",
    "sync",
    "core",
    "base",
    "web",
    "app",
    "http",
    "log",
    "url",
    "net",
    "cli",
    "api",
    "io",
    "env",
    "cfg",
    "lib",
    "util",
    "config",
    "crypto",
    "rand",
    "num",
    "regex",
    "time",
    "chrono",
    "uuid",
    "json",
    "xml",
    "csv",
    "toml",
    "yaml",
    "sql",
    "proc",
    "proc-macro",
    "derive",
    "macro",
    "test",
    "bench",
    "build",
    "bytes",
    "string",
    "either",
    "lazy",
    "once",
    "pin",
    "mutex",
    "lock",
    "parallel",
    "runtime",
    "scheduler",
    "executor",
    "channel",
    "stream",
    "buffer",
];

/// Tiny utility packages that don't represent meaningful tech identity.
pub(crate) const UTILITY_DEPS: &[&str] = &[
    "proc-macro2",
    "quote",
    "unicode-ident",
    "cfg-if",
    "memchr",
    "libc",
    "autocfg",
    "version_check",
    "pkg-config",
    "itoa",
    "ryu",
    "bitflags",
    "bytes",
    "pin-project-lite",
    "fnv",
    "percent-encoding",
    "tinyvec",
    "smallvec",
    "indexmap",
    "hashbrown",
    "equivalent",
    "either",
    "anyhow",
    "thiserror",
];

/// Developer archetype map: if ANY tech signal is present, add the associated domain concerns.
/// Each entry: (tech_signals, concerns)
pub(crate) fn archetype_map() -> &'static [(&'static [&'static str], &'static [&'static str])] {
    &[
        // Desktop app developers
        (
            &[
                "tauri",
                "electron",
                "wails",
                "neutralino",
                "nwjs",
                "gtk",
                "qt",
            ],
            &[
                "cross-platform",
                "packaging",
                "auto-update",
                "installer",
                "notarization",
                "tray",
                "native",
                "window management",
                "system tray",
                "file dialog",
                "drag and drop",
            ],
        ),
        // Frontend web developers
        (
            &[
                "react", "vue", "angular", "svelte", "nextjs", "next.js", "nuxt", "remix", "astro",
            ],
            &[
                "responsive",
                "ssr",
                "seo",
                "bundle size",
                "lighthouse",
                "web vitals",
                "hydration",
                "lazy loading",
                "code splitting",
                "progressive web app",
                "pwa",
                "browser compatibility",
            ],
        ),
        // Backend / API developers
        (
            &[
                "express", "fastify", "django", "flask", "fastapi", "rails", "actix", "axum",
                "gin", "spring",
            ],
            &[
                "api design",
                "rate limiting",
                "middleware",
                "orm",
                "migrations",
                "connection pooling",
                "microservices",
                "message queue",
                "event driven",
            ],
        ),
        // DevOps / Infrastructure
        (
            &[
                "docker",
                "kubernetes",
                "k8s",
                "terraform",
                "ansible",
                "pulumi",
            ],
            &[
                "infrastructure",
                "ci/cd",
                "scaling",
                "load balancing",
                "service mesh",
                "container orchestration",
                "gitops",
                "blue-green deployment",
                "canary",
                "rollback",
            ],
        ),
        // ML / AI Engineers
        (
            &[
                "pytorch",
                "tensorflow",
                "llm",
                "transformers",
                "langchain",
                "openai",
            ],
            &[
                "training",
                "inference",
                "fine-tuning",
                "embeddings",
                "rag",
                "vector",
                "prompt engineering",
                "model serving",
                "quantization",
                "distillation",
            ],
        ),
        // Mobile developers
        (
            &[
                "ios",
                "android",
                "flutter",
                "react native",
                "swiftui",
                "kotlin",
            ],
            &[
                "app store",
                "push notifications",
                "deep linking",
                "offline first",
                "gestures",
                "navigation",
                "app size",
                "launch time",
                "background tasks",
            ],
        ),
        // Database / Data engineers
        (
            &[
                "postgresql",
                "postgres",
                "mongodb",
                "redis",
                "elasticsearch",
                "clickhouse",
                "cassandra",
            ],
            &[
                "indexing",
                "replication",
                "sharding",
                "backup",
                "query optimization",
                "data modeling",
                "schema design",
                "data pipeline",
                "etl",
                "warehousing",
            ],
        ),
        // Game developers
        (
            &["unity", "unreal", "godot", "bevy", "gamedev"],
            &[
                "rendering",
                "physics",
                "shaders",
                "ecs",
                "pathfinding",
                "procedural generation",
                "networking",
                "frame rate",
                "asset pipeline",
            ],
        ),
        // Systems / Embedded
        (
            &["rust", "cpp", "c++"],
            &[
                "memory safety",
                "lifetimes",
                "ownership",
                "zero-cost abstractions",
                "unsafe",
                "ffi",
                "linking",
                "compile time",
            ],
        ),
    ]
}

/// Technology adjacency map: primary tech -> related/adjacent technologies.
pub(crate) fn adjacency_map() -> HashMap<&'static str, &'static [&'static str]> {
    HashMap::from([
        (
            "rust",
            &["cargo", "wasm", "webassembly", "tokio", "serde", "async"][..],
        ),
        ("tauri", &["webview", "desktop", "ipc", "wry", "tao"]),
        (
            "react",
            &["jsx", "hooks", "vite", "webpack", "nextjs", "next.js"],
        ),
        ("typescript", &["javascript", "nodejs", "deno", "bun"]),
        ("javascript", &["typescript", "nodejs", "npm"]),
        (
            "python",
            &["pip", "pytorch", "tensorflow", "django", "flask", "fastapi"],
        ),
        ("go", &["golang", "goroutine"]),
        ("sqlite", &["sql", "database", "rusqlite"]),
        ("next.js", &["react", "vercel", "nextjs"]),
        ("vue", &["vuejs", "nuxt", "vite"]),
        ("svelte", &["sveltekit", "vite"]),
        ("docker", &["container", "kubernetes", "k8s"]),
        ("kubernetes", &["k8s", "docker", "helm", "container"]),
        ("aws", &["lambda", "s3", "dynamodb", "cloudformation"]),
        ("postgresql", &["postgres", "sql", "database"]),
        ("mongodb", &["nosql", "database", "mongoose"]),
        ("graphql", &["apollo", "relay"]),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn cross_cutting_topics_contains_expected_items() {
        let topics: HashSet<&str> = CROSS_CUTTING_TOPICS.iter().copied().collect();
        for expected in &["testing", "security", "performance", "debugging", "caching"] {
            assert!(
                topics.contains(expected),
                "missing cross-cutting topic: {expected}"
            );
        }
    }

    #[test]
    fn cross_cutting_topics_has_no_duplicates() {
        let set: HashSet<&str> = CROSS_CUTTING_TOPICS.iter().copied().collect();
        assert_eq!(
            set.len(),
            CROSS_CUTTING_TOPICS.len(),
            "duplicate in CROSS_CUTTING_TOPICS"
        );
    }

    #[test]
    fn ambiguous_deps_contains_expected_items() {
        let deps: HashSet<&str> = AMBIGUOUS_DEPS.iter().copied().collect();
        for expected in &["futures", "async", "http", "json", "regex"] {
            assert!(deps.contains(expected), "missing ambiguous dep: {expected}");
        }
    }

    #[test]
    fn ambiguous_deps_has_no_duplicates() {
        let set: HashSet<&str> = AMBIGUOUS_DEPS.iter().copied().collect();
        assert_eq!(
            set.len(),
            AMBIGUOUS_DEPS.len(),
            "duplicate in AMBIGUOUS_DEPS"
        );
    }

    #[test]
    fn utility_deps_contains_expected_items() {
        let deps: HashSet<&str> = UTILITY_DEPS.iter().copied().collect();
        for expected in &["proc-macro2", "quote", "anyhow", "thiserror", "bitflags"] {
            assert!(deps.contains(expected), "missing utility dep: {expected}");
        }
    }

    #[test]
    fn utility_deps_has_no_duplicates() {
        let set: HashSet<&str> = UTILITY_DEPS.iter().copied().collect();
        assert_eq!(set.len(), UTILITY_DEPS.len(), "duplicate in UTILITY_DEPS");
    }

    #[test]
    fn archetype_map_has_entries() {
        let map = archetype_map();
        assert!(
            map.len() >= 8,
            "expected at least 8 archetypes, got {}",
            map.len()
        );
    }

    #[test]
    fn archetype_map_no_empty_signals_or_concerns() {
        for (signals, concerns) in archetype_map() {
            assert!(!signals.is_empty(), "archetype has empty signals");
            assert!(!concerns.is_empty(), "archetype has empty concerns");
        }
    }

    #[test]
    fn archetype_map_tauri_maps_to_desktop_concerns() {
        let map = archetype_map();
        let desktop = map.iter().find(|(signals, _)| signals.contains(&"tauri"));
        assert!(desktop.is_some(), "no archetype contains tauri");
        let (_, concerns) = desktop.unwrap();
        assert!(
            concerns.contains(&"cross-platform"),
            "tauri archetype missing cross-platform concern"
        );
    }

    #[test]
    fn archetype_map_react_maps_to_frontend_concerns() {
        let map = archetype_map();
        let frontend = map.iter().find(|(signals, _)| signals.contains(&"react"));
        assert!(frontend.is_some(), "no archetype contains react");
        let (_, concerns) = frontend.unwrap();
        assert!(
            concerns.contains(&"ssr"),
            "react archetype missing ssr concern"
        );
    }

    #[test]
    fn adjacency_map_has_entries() {
        let map = adjacency_map();
        assert!(
            map.len() >= 10,
            "expected at least 10 adjacency entries, got {}",
            map.len()
        );
    }

    #[test]
    fn adjacency_map_rust_includes_expected_tech() {
        let map = adjacency_map();
        let rust_adj = map.get("rust").expect("rust missing from adjacency map");
        assert!(rust_adj.contains(&"cargo"), "rust adjacency missing cargo");
        assert!(rust_adj.contains(&"tokio"), "rust adjacency missing tokio");
    }

    #[test]
    fn adjacency_map_no_empty_adjacencies() {
        for (key, values) in adjacency_map() {
            assert!(!values.is_empty(), "adjacency for '{key}' is empty");
        }
    }

    #[test]
    fn cross_cutting_and_ambiguous_are_disjoint() {
        let cross: HashSet<&str> = CROSS_CUTTING_TOPICS.iter().copied().collect();
        let ambig: HashSet<&str> = AMBIGUOUS_DEPS.iter().copied().collect();
        let overlap: Vec<&&str> = cross.intersection(&ambig).collect();
        assert!(
            overlap.is_empty(),
            "overlap between cross-cutting and ambiguous: {overlap:?}"
        );
    }

    #[test]
    fn all_constant_strings_are_lowercase() {
        for topic in CROSS_CUTTING_TOPICS {
            assert_eq!(
                *topic,
                topic.to_lowercase(),
                "CROSS_CUTTING not lowercase: {topic}"
            );
        }
        for dep in AMBIGUOUS_DEPS {
            assert_eq!(
                *dep,
                dep.to_lowercase(),
                "AMBIGUOUS_DEP not lowercase: {dep}"
            );
        }
        for dep in UTILITY_DEPS {
            assert_eq!(*dep, dep.to_lowercase(), "UTILITY_DEP not lowercase: {dep}");
        }
    }
}
