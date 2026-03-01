//! Stack profile definitions — all 8 built-in technology profiles.
//!
//! Each profile encodes curated domain knowledge about a technology ecosystem:
//! pain points, ecosystem shifts, keyword boosts, source preferences, and
//! auto-detection markers.

use super::{EcosystemShift, PainPoint, StackProfile};

// ============================================================================
// Next.js Fullstack
// ============================================================================

pub static NEXTJS_FULLSTACK: StackProfile = StackProfile {
    id: "nextjs_fullstack",
    name: "Next.js Fullstack",
    core_tech: &["nextjs", "react", "vercel", "typescript"],
    companions: &[
        "prisma",
        "drizzle",
        "tailwindcss",
        "trpc",
        "zod",
        "turborepo",
    ],
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
            from: "prisma",
            to: "drizzle",
            keywords: &[
                "drizzle",
                "prisma alternative",
                "orm migration",
                "drizzle-orm",
            ],
            boost: 1.15,
        },
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
};

// ============================================================================
// React Native
// ============================================================================

pub static REACT_NATIVE: StackProfile = StackProfile {
    id: "react_native",
    name: "React Native / Expo",
    core_tech: &["react-native", "expo", "typescript", "react"],
    companions: &[
        "expo-router",
        "reanimated",
        "gesture-handler",
        "react-navigation",
        "zustand",
        "tanstack-query",
        "nativewind",
    ],
    competing: &["flutter", "kotlin", "swift", "ionic", "capacitor"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "new architecture",
                "fabric",
                "turbo module",
                "bridgeless",
                "new arch",
            ],
            severity: 0.15,
            description: "New architecture migration",
        },
        PainPoint {
            keywords: &["hermes", "engine", "jsc", "javascript core", "hermes quirk"],
            severity: 0.10,
            description: "Hermes engine quirks",
        },
        PainPoint {
            keywords: &[
                "app store",
                "review",
                "rejection",
                "guideline",
                "app review",
            ],
            severity: 0.08,
            description: "App store review issues",
        },
        PainPoint {
            keywords: &[
                "ota",
                "over the air",
                "code push",
                "eas update",
                "expo update",
            ],
            severity: 0.10,
            description: "OTA update reliability",
        },
        PainPoint {
            keywords: &[
                "js thread",
                "ui thread",
                "performance",
                "frame drop",
                "jank",
            ],
            severity: 0.12,
            description: "JS thread performance",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "bare rn",
            to: "expo",
            keywords: &["expo", "expo go", "eas build", "expo managed", "expo sdk"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "react-navigation",
            to: "expo-router",
            keywords: &["expo router", "file-based routing", "expo-router"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "old arch",
            to: "new arch",
            keywords: &["new architecture", "fabric", "turbo module", "bridgeless"],
            boost: 1.15,
        },
    ],
    keyword_boosts: &[
        ("react native", 0.12),
        ("expo", 0.10),
        ("react-native", 0.12),
        ("mobile app", 0.06),
        ("eas build", 0.08),
        ("native module", 0.08),
    ],
    source_preferences: &[("reddit", 0.10), ("devto", 0.05)],
    detection_markers: &[
        "react-native",
        "expo",
        "app.json",
        "eas.json",
        "metro.config",
        "react-native.config",
    ],
    detection_threshold: 2,
};

// ============================================================================
// Laravel
// ============================================================================

pub static LARAVEL: StackProfile = StackProfile {
    id: "laravel",
    name: "Laravel",
    core_tech: &["laravel", "php", "mysql", "redis"],
    companions: &[
        "livewire", "filament", "inertia", "blade", "pest", "forge", "vapor", "horizon", "sanctum",
    ],
    competing: &["symfony", "django", "rails", "express", "spring"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "php version",
                "php 8",
                "php 7",
                "php compatibility",
                "php deprecation",
                "php upgrade",
            ],
            severity: 0.10,
            description: "PHP version migration",
        },
        PainPoint {
            keywords: &["queue", "job", "failed", "retry", "horizon", "worker"],
            severity: 0.12,
            description: "Queue and job reliability",
        },
        PainPoint {
            keywords: &[
                "n+1",
                "eager loading",
                "query",
                "eloquent performance",
                "lazy loading",
            ],
            severity: 0.10,
            description: "N+1 query problems",
        },
        PainPoint {
            keywords: &["deployment", "forge", "envoyer", "vapor", "docker"],
            severity: 0.08,
            description: "Deployment complexity",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "livewire 2",
            to: "livewire 3",
            keywords: &[
                "livewire 3",
                "livewire v3",
                "livewire upgrade",
                "wire:navigate",
            ],
            boost: 1.15,
        },
        EcosystemShift {
            from: "nova",
            to: "filament",
            keywords: &[
                "filament",
                "filament admin",
                "filament v3",
                "filament panel",
            ],
            boost: 1.15,
        },
        EcosystemShift {
            from: "phpunit",
            to: "pest",
            keywords: &["pest", "pest v3", "pest testing", "arch testing"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("laravel", 0.12),
        ("livewire", 0.10),
        ("eloquent", 0.08),
        ("blade", 0.06),
        ("filament", 0.08),
        ("inertia", 0.08),
    ],
    source_preferences: &[("reddit", 0.05), ("devto", 0.10)],
    detection_markers: &[
        "laravel",
        "artisan",
        "composer.json",
        "eloquent",
        "blade",
        "livewire",
    ],
    detection_threshold: 2,
};

// ============================================================================
// Django
// ============================================================================

pub static DJANGO: StackProfile = StackProfile {
    id: "django",
    name: "Django",
    core_tech: &["django", "python", "postgresql", "celery"],
    companions: &[
        "drf",
        "django-rest-framework",
        "wagtail",
        "htmx",
        "django-ninja",
        "gunicorn",
        "pytest-django",
        "redis",
    ],
    competing: &["flask", "fastapi", "rails", "laravel", "express"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "orm",
                "queryset",
                "n+1",
                "select_related",
                "prefetch_related",
            ],
            severity: 0.12,
            description: "ORM performance",
        },
        PainPoint {
            keywords: &["async", "asgi", "channels", "async view", "django async"],
            severity: 0.10,
            description: "Async support",
        },
        PainPoint {
            keywords: &["migration", "conflict", "merge", "squash", "makemigrations"],
            severity: 0.10,
            description: "Migration conflicts",
        },
        PainPoint {
            keywords: &[
                "test speed",
                "pytest",
                "fixture",
                "factory",
                "test database",
            ],
            severity: 0.08,
            description: "Test suite speed",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "drf",
            to: "django-ninja",
            keywords: &["django-ninja", "ninja api", "pydantic", "django ninja"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "javascript",
            to: "htmx",
            keywords: &[
                "htmx",
                "hypermedia",
                "hx-get",
                "hx-post",
                "html over the wire",
            ],
            boost: 1.15,
        },
        EcosystemShift {
            from: "custom cms",
            to: "wagtail",
            keywords: &["wagtail", "wagtail cms", "streamfield", "wagtail page"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("django", 0.12),
        ("drf", 0.08),
        ("celery", 0.08),
        ("htmx", 0.08),
        ("wagtail", 0.06),
        ("django-ninja", 0.08),
    ],
    source_preferences: &[("reddit", 0.05), ("hackernews", 0.05)],
    detection_markers: &[
        "django",
        "manage.py",
        "settings.py",
        "celery",
        "drf",
        "wagtail",
    ],
    detection_threshold: 2,
};

// ============================================================================
// Vue Frontend
// ============================================================================

pub static VUE_FRONTEND: StackProfile = StackProfile {
    id: "vue_frontend",
    name: "Vue / Nuxt",
    core_tech: &["vue", "nuxt", "pinia", "vite"],
    companions: &[
        "vuetify",
        "primevue",
        "unocss",
        "vitest",
        "vue-router",
        "vueuse",
        "tanstack-query",
    ],
    competing: &["react", "angular", "svelte", "solid"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "composition api",
                "options api",
                "migration",
                "setup",
                "script setup",
            ],
            severity: 0.12,
            description: "Composition API migration",
        },
        PainPoint {
            keywords: &["ssr", "hydration", "mismatch", "nuxt ssr", "server render"],
            severity: 0.10,
            description: "SSR hydration mismatches",
        },
        PainPoint {
            keywords: &[
                "typescript",
                "definecomponent",
                "vue typescript",
                "vue types",
                "script setup",
            ],
            severity: 0.08,
            description: "TypeScript integration",
        },
        PainPoint {
            keywords: &[
                "vuex",
                "pinia",
                "state management",
                "store migration",
                "vuex to pinia",
            ],
            severity: 0.10,
            description: "Vuex to Pinia migration",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "vue 3",
            to: "vue vapor",
            keywords: &["vue vapor", "vapor mode", "compile-time", "no virtual dom"],
            boost: 1.20,
        },
        EcosystemShift {
            from: "nuxt 3",
            to: "nuxt 4",
            keywords: &["nuxt 4", "nuxt upgrade", "nuxt migration", "nuxt next"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "tailwind",
            to: "unocss",
            keywords: &["unocss", "uno css", "atomic css", "unocss preset"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("vue", 0.10),
        ("nuxt", 0.10),
        ("pinia", 0.08),
        ("composition api", 0.08),
        ("vue vapor", 0.10),
        ("vueuse", 0.06),
    ],
    source_preferences: &[("devto", 0.10), ("reddit", 0.05)],
    detection_markers: &["vue", "nuxt", "pinia", "nuxt.config", "vite.config", ".vue"],
    detection_threshold: 2,
};

// ============================================================================
// DevOps & SRE
// ============================================================================

pub static DEVOPS_SRE: StackProfile = StackProfile {
    id: "devops_sre",
    name: "DevOps & SRE",
    core_tech: &["kubernetes", "docker", "terraform", "ansible"],
    companions: &[
        "helm",
        "prometheus",
        "grafana",
        "istio",
        "argocd",
        "vault",
        "etcd",
        "cilium",
        "envoy",
        "datadog",
    ],
    competing: &["heroku", "railway", "render"],
    pain_points: &[
        PainPoint {
            keywords: &["cluster", "upgrade", "etcd", "control plane"],
            severity: 0.15,
            description: "Cluster lifecycle management",
        },
        PainPoint {
            keywords: &[
                "observability",
                "metrics",
                "tracing",
                "logging",
                "opentelemetry",
            ],
            severity: 0.12,
            description: "Observability stack complexity",
        },
        PainPoint {
            keywords: &["rbac", "network policy", "pod security", "admission"],
            severity: 0.10,
            description: "Security policy management",
        },
        PainPoint {
            keywords: &["terraform", "state", "drift", "plan", "apply"],
            severity: 0.12,
            description: "IaC state management",
        },
        PainPoint {
            keywords: &["ci", "cd", "pipeline", "deploy", "rollback", "canary"],
            severity: 0.10,
            description: "CI/CD pipeline reliability",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "helm",
            to: "kustomize",
            keywords: &["kustomize", "helm to kustomize", "kustomization"],
            boost: 1.12,
        },
        EcosystemShift {
            from: "jenkins",
            to: "github actions",
            keywords: &["github actions", "actions workflow", "jenkins migration"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "nagios",
            to: "prometheus",
            keywords: &["prometheus migration", "alertmanager", "prometheus stack"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "terraform",
            to: "pulumi",
            keywords: &["pulumi", "terraform to pulumi", "infrastructure sdk"],
            boost: 1.08,
        },
    ],
    keyword_boosts: &[
        ("kubernetes", 0.12),
        ("k8s", 0.12),
        ("docker", 0.10),
        ("terraform", 0.10),
        ("helm", 0.08),
        ("prometheus", 0.08),
        ("grafana", 0.06),
        ("ansible", 0.06),
        ("argocd", 0.08),
        ("istio", 0.08),
        ("observability", 0.08),
        ("sre", 0.06),
    ],
    source_preferences: &[("hackernews", 0.05), ("reddit", 0.05)],
    detection_markers: &[
        "kubernetes",
        "kubectl",
        "docker",
        "terraform",
        "helm",
        "prometheus",
        "k8s",
    ],
    detection_threshold: 2,
};

// ============================================================================
// Haskell & Functional Programming
// ============================================================================

pub static HASKELL_FP: StackProfile = StackProfile {
    id: "haskell",
    name: "Haskell & Functional Programming",
    core_tech: &["haskell", "nix", "ghc", "cabal", "stack"],
    companions: &[
        "purescript",
        "ocaml",
        "elm",
        "agda",
        "idris",
        "coq",
        "lens",
        "mtl",
        "servant",
        "yesod",
        "pandoc",
    ],
    competing: &[],
    pain_points: &[
        PainPoint {
            keywords: &["ghc", "upgrade", "breaking", "version", "migration"],
            severity: 0.12,
            description: "GHC version upgrades",
        },
        PainPoint {
            keywords: &["cabal", "stack", "dependency", "resolver", "build"],
            severity: 0.10,
            description: "Build tool fragmentation",
        },
        PainPoint {
            keywords: &["monad", "transformer", "effect", "mtl", "io"],
            severity: 0.10,
            description: "Effect system complexity",
        },
        PainPoint {
            keywords: &["nix", "flake", "derivation", "nixpkgs", "nixos"],
            severity: 0.10,
            description: "Nix ecosystem complexity",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "mtl",
            to: "effectful",
            keywords: &["effectful", "effect system", "mtl alternative"],
            boost: 1.12,
        },
        EcosystemShift {
            from: "cabal",
            to: "cabal+nix",
            keywords: &["nix flake", "haskell.nix", "cabal2nix"],
            boost: 1.08,
        },
    ],
    keyword_boosts: &[
        ("haskell", 0.12),
        ("ghc", 0.10),
        ("cabal", 0.08),
        ("nix", 0.08),
        ("functional programming", 0.10),
        ("type theory", 0.08),
        ("category theory", 0.06),
        ("monad", 0.08),
        ("algebraic", 0.06),
        ("purescript", 0.06),
        ("ocaml", 0.06),
    ],
    source_preferences: &[("hackernews", 0.05), ("lobsters", 0.15)],
    detection_markers: &[
        "haskell",
        "ghc",
        "cabal",
        "stack.yaml",
        ".cabal",
        "nix",
        "flake.nix",
    ],
    detection_threshold: 2,
};

// ============================================================================
// Profile Registry
// ============================================================================

/// All available stack profiles, in display order.
pub static ALL_PROFILES: [&StackProfile; 10] = [
    &NEXTJS_FULLSTACK,
    &RUST_SYSTEMS,
    &PYTHON_ML,
    &GO_BACKEND,
    &REACT_NATIVE,
    &LARAVEL,
    &DJANGO,
    &VUE_FRONTEND,
    &DEVOPS_SRE,
    &HASKELL_FP,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_profiles_have_unique_ids() {
        let mut ids = std::collections::HashSet::new();
        for profile in &ALL_PROFILES {
            assert!(
                ids.insert(profile.id),
                "Duplicate profile ID: {}",
                profile.id
            );
        }
    }

    #[test]
    fn test_all_profiles_have_pain_points() {
        for profile in &ALL_PROFILES {
            assert!(
                !profile.pain_points.is_empty(),
                "{} has no pain points",
                profile.id
            );
            for pp in profile.pain_points {
                assert!(
                    pp.keywords.len() >= 2,
                    "{}: pain point '{}' needs 2+ keywords",
                    profile.id,
                    pp.description
                );
                assert!(
                    pp.severity >= 0.05 && pp.severity <= 0.20,
                    "{}: pain point severity out of range: {}",
                    profile.id,
                    pp.severity
                );
            }
        }
    }

    #[test]
    fn test_all_profiles_have_ecosystem_shifts() {
        for profile in &ALL_PROFILES {
            assert!(
                !profile.ecosystem_shifts.is_empty(),
                "{} has no ecosystem shifts",
                profile.id
            );
            for es in profile.ecosystem_shifts {
                assert!(
                    es.boost >= 1.0 && es.boost <= 1.25,
                    "{}: ecosystem shift boost out of range: {}",
                    profile.id,
                    es.boost
                );
            }
        }
    }

    #[test]
    fn test_no_core_tech_in_competing() {
        for profile in &ALL_PROFILES {
            for &core in profile.core_tech {
                assert!(
                    !profile.competing.contains(&core),
                    "{}: '{}' is both core_tech and competing",
                    profile.id,
                    core
                );
            }
        }
    }

    #[test]
    fn test_detection_threshold_reasonable() {
        for profile in &ALL_PROFILES {
            assert!(
                profile.detection_threshold >= 1 && profile.detection_threshold <= 3,
                "{}: detection_threshold {} seems wrong",
                profile.id,
                profile.detection_threshold
            );
            assert!(
                profile.detection_markers.len() >= profile.detection_threshold,
                "{}: not enough detection markers ({}) for threshold ({})",
                profile.id,
                profile.detection_markers.len(),
                profile.detection_threshold
            );
        }
    }
}
