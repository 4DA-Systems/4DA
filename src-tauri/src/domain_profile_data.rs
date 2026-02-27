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
