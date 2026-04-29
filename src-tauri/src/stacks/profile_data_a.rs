// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Stack profile definitions — Group A: Next.js, Rust, Python ML, Go Backend.

use crate::stacks::{EcosystemShift, PainPoint, SeedItem, StackProfile};

// ============================================================================
// Next.js Fullstack
// ============================================================================

pub static NEXTJS_FULLSTACK: StackProfile = StackProfile {
    id: "nextjs_fullstack",
    name: "Next.js Fullstack",
    core_tech: &["nextjs", "react", "vercel", "typescript"],
    companions: &["tailwindcss", "trpc", "zod", "turborepo"],
    competing: &["remix", "astro", "sveltekit", "nuxt", "angular"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "app router",
                "migration",
                "pages router",
                "next 13",
                "next 14",
            ],
            severity: 0.15,
            description: "App Router migration complexity",
        },
        PainPoint {
            keywords: &["edge runtime", "edge function", "middleware", "cold start"],
            severity: 0.12,
            description: "Edge runtime limitations and cold starts",
        },
        PainPoint {
            keywords: &["isr", "revalidate", "cache", "stale", "incremental"],
            severity: 0.10,
            description: "ISR and caching behavior",
        },
        PainPoint {
            keywords: &[
                "bundle size",
                "tree shaking",
                "code splitting",
                "webpack",
                "turbopack",
            ],
            severity: 0.08,
            description: "Bundle size optimization",
        },
        PainPoint {
            keywords: &[
                "server component",
                "client component",
                "use client",
                "rsc",
                "server action",
            ],
            severity: 0.15,
            description: "Server/Client component boundaries",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "eslint",
            to: "biome",
            keywords: &["biome", "eslint alternative", "biome formatter", "biomejs"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "node",
            to: "bun",
            keywords: &["bun runtime", "bun install", "bun vs node", "bunx"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("nextjs", 0.12),
        ("next.js", 0.12),
        ("vercel", 0.10),
        ("react server component", 0.10),
        ("app router", 0.08),
        ("turbopack", 0.08),
        ("server action", 0.08),
        ("server component", 0.08),
        ("rsc", 0.06),
        ("ssr", 0.06),
        ("hydration", 0.06),
    ],
    source_preferences: &[("hackernews", 0.05), ("reddit", -0.05)],
    detection_markers: &[
        "next.config",
        "nextjs",
        "next/image",
        "next/link",
        "next/router",
        "vercel.json",
        "@vercel/",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Vercel Blog",
            url: "https://vercel.com/blog",
            source_type: "web",
        },
        SeedItem {
            title: "Next.js Blog",
            url: "https://nextjs.org/blog",
            source_type: "web",
        },
        SeedItem {
            title: "React Blog",
            url: "https://react.dev/blog",
            source_type: "web",
        },
        SeedItem {
            title: "r/nextjs",
            url: "https://www.reddit.com/r/nextjs/",
            source_type: "reddit",
        },
    ],
};

// ============================================================================
// Rust Systems
// ============================================================================

pub static RUST_SYSTEMS: StackProfile = StackProfile {
    id: "rust_systems",
    name: "Rust Systems",
    core_tech: &["rust", "cargo", "tokio", "serde"],
    companions: &["wasm", "tauri", "axum", "actix", "sqlx", "tracing", "clap"],
    competing: &["go", "zig", "c++", "cpp"],
    pain_points: &[
        PainPoint {
            keywords: &["async", "pin", "send", "lifetime", "future", "tokio"],
            severity: 0.15,
            description: "Async complexity (Pin/Send/lifetime)",
        },
        PainPoint {
            keywords: &[
                "compile time",
                "build time",
                "incremental compilation",
                "cargo build",
            ],
            severity: 0.10,
            description: "Compile times",
        },
        PainPoint {
            keywords: &["unsafe", "soundness", "undefined behavior", "miri"],
            severity: 0.12,
            description: "Unsafe code soundness",
        },
        PainPoint {
            keywords: &[
                "error handling",
                "thiserror",
                "anyhow",
                "result",
                "error type",
            ],
            severity: 0.08,
            description: "Error handling patterns",
        },
        PainPoint {
            keywords: &[
                "borrow checker",
                "ownership",
                "move semantics",
                "lifetime annotation",
            ],
            severity: 0.10,
            description: "Borrow checker challenges",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "dyn trait",
            to: "async trait",
            keywords: &[
                "native async trait",
                "async fn in trait",
                "return position impl trait",
            ],
            boost: 1.15,
        },
        EcosystemShift {
            from: "nightly",
            to: "stable",
            keywords: &[
                "const generics",
                "generic const",
                "stabilization",
                "feature gate",
            ],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("rust", 0.12),
        ("cargo", 0.08),
        ("tokio", 0.10),
        ("async rust", 0.10),
        ("wasm", 0.06),
        ("tauri", 0.08),
        ("axum", 0.08),
        ("unsafe", 0.06),
        ("borrow checker", 0.08),
        ("lifetime", 0.06),
    ],
    source_preferences: &[("lobsters", 0.15), ("hackernews", 0.05)],
    detection_markers: &[
        "Cargo.toml",
        "cargo",
        "rustc",
        "tokio",
        "serde",
        "axum",
        "tauri",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "This Week in Rust",
            url: "https://this-week-in-rust.org/",
            source_type: "rss",
        },
        SeedItem {
            title: "Rust Security Advisories",
            url: "https://rustsec.org/advisories/",
            source_type: "web",
        },
        SeedItem {
            title: "Rust Blog",
            url: "https://blog.rust-lang.org/",
            source_type: "rss",
        },
        SeedItem {
            title: "r/rust",
            url: "https://www.reddit.com/r/rust/",
            source_type: "reddit",
        },
    ],
};

// ============================================================================
// Python ML
// ============================================================================

pub static PYTHON_ML: StackProfile = StackProfile {
    id: "python_ml",
    name: "Python ML/AI",
    core_tech: &["python", "pytorch", "transformers", "numpy", "cuda"],
    companions: &[
        "pandas",
        "scikit-learn",
        "huggingface",
        "wandb",
        "jupyter",
        "fastapi",
        "langchain",
        "ollama",
        "vllm",
    ],
    competing: &["tensorflow", "jax", "mxnet"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "cuda",
                "version",
                "driver",
                "nvcc",
                "nvidia",
                "gpu compatibility",
            ],
            severity: 0.15,
            description: "CUDA version conflicts",
        },
        PainPoint {
            keywords: &["gpu", "oom", "out of memory", "vram", "memory allocation"],
            severity: 0.12,
            description: "GPU out-of-memory errors",
        },
        PainPoint {
            keywords: &[
                "dependency",
                "pip",
                "conda",
                "virtual environment",
                "package conflict",
            ],
            severity: 0.10,
            description: "Dependency hell",
        },
        PainPoint {
            keywords: &["reproducibility", "seed", "deterministic", "random state"],
            severity: 0.08,
            description: "Training reproducibility",
        },
        PainPoint {
            keywords: &[
                "model serving",
                "inference",
                "latency",
                "deployment",
                "onnx",
            ],
            severity: 0.10,
            description: "Model serving and deployment",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "pytorch",
            to: "jax",
            keywords: &["jax", "flax", "jit compilation", "xla", "jax research"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "safetensors",
            to: "gguf",
            keywords: &["gguf", "ggml", "quantization format", "llama.cpp"],
            boost: 1.20,
        },
        EcosystemShift {
            from: "cloud llm",
            to: "local llm",
            keywords: &[
                "local llm",
                "ollama",
                "self-hosted",
                "on-device",
                "edge inference",
            ],
            boost: 1.20,
        },
        EcosystemShift {
            from: "fine-tuning",
            to: "rag",
            keywords: &[
                "rag",
                "retrieval augmented",
                "vector database",
                "embedding pipeline",
            ],
            boost: 1.15,
        },
    ],
    keyword_boosts: &[
        ("pytorch", 0.12),
        ("transformers", 0.10),
        ("llm", 0.10),
        ("huggingface", 0.08),
        ("fine-tuning", 0.08),
        ("rag", 0.08),
        ("diffusion", 0.06),
        ("deep learning", 0.08),
        ("neural network", 0.06),
        ("gpu", 0.06),
        ("inference", 0.06),
    ],
    source_preferences: &[("arxiv", 0.15), ("hackernews", 0.05)],
    detection_markers: &[
        "requirements.txt",
        "pytorch",
        "torch",
        "transformers",
        "numpy",
        "setup.py",
        "pyproject.toml",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Hugging Face Blog",
            url: "https://huggingface.co/blog",
            source_type: "web",
        },
        SeedItem {
            title: "PyTorch Blog",
            url: "https://pytorch.org/blog/",
            source_type: "web",
        },
        SeedItem {
            title: "arXiv ML",
            url: "https://arxiv.org/list/cs.LG/recent",
            source_type: "arxiv",
        },
        SeedItem {
            title: "r/MachineLearning",
            url: "https://www.reddit.com/r/MachineLearning/",
            source_type: "reddit",
        },
        SeedItem {
            title: "Papers With Code",
            url: "https://paperswithcode.com/",
            source_type: "web",
        },
    ],
};

// ============================================================================
// Go Backend
// ============================================================================

pub static GO_BACKEND: StackProfile = StackProfile {
    id: "go_backend",
    name: "Go Backend",
    core_tech: &["go", "golang", "docker", "grpc", "kubernetes"],
    companions: &[
        "protobuf",
        "chi",
        "gin",
        "echo",
        "prometheus",
        "grafana",
        "etcd",
        "nats",
        "redis",
    ],
    competing: &["rust", "java", "node", "python"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "error handling",
                "if err != nil",
                "error wrapping",
                "errors.is",
            ],
            severity: 0.10,
            description: "Error handling verbosity",
        },
        PainPoint {
            keywords: &[
                "generics",
                "type parameter",
                "type constraint",
                "interface{}",
            ],
            severity: 0.10,
            description: "Generics limitations",
        },
        PainPoint {
            keywords: &[
                "context",
                "context propagation",
                "context.withtimeout",
                "ctx",
            ],
            severity: 0.08,
            description: "Context propagation patterns",
        },
        PainPoint {
            keywords: &[
                "module",
                "go.mod",
                "dependency",
                "replace directive",
                "module conflict",
            ],
            severity: 0.08,
            description: "Module and dependency conflicts",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "log",
            to: "slog",
            keywords: &["slog", "structured logging", "log/slog", "slog handler"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "for loop",
            to: "range-over-func",
            keywords: &["range over func", "iterator", "iter.seq", "go 1.23"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "cgo",
            to: "wasm",
            keywords: &["go wasm", "wazero", "tinygo", "webassembly"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("golang", 0.12),
        ("go", 0.08),
        ("kubernetes", 0.08),
        ("docker", 0.06),
        ("grpc", 0.08),
        ("goroutine", 0.08),
        ("k8s", 0.06),
        ("concurrency", 0.06),
        ("api", 0.06),
        ("microservice", 0.08),
    ],
    source_preferences: &[("lobsters", 0.10), ("hackernews", 0.05)],
    detection_markers: &[
        "go.mod",
        "go.sum",
        "golang",
        "goroutine",
        "kubernetes",
        "docker",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Go Blog",
            url: "https://go.dev/blog/",
            source_type: "web",
        },
        SeedItem {
            title: "Go Weekly",
            url: "https://golangweekly.com/",
            source_type: "rss",
        },
        SeedItem {
            title: "r/golang",
            url: "https://www.reddit.com/r/golang/",
            source_type: "reddit",
        },
        SeedItem {
            title: "Kubernetes Blog",
            url: "https://kubernetes.io/blog/",
            source_type: "web",
        },
    ],
};
