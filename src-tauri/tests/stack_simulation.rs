//! Stack Intelligence simulation harness.
//!
//! For each of the 8 stack profiles, generates synthetic content items and
//! validates that the scoring functions produce meaningful differentiation:
//! - Pain point content gets a meaningful lift
//! - Off-stack (competing) content stays suppressed
//! - Multi-tech (synergy) content outscores single-tech
//! - Competing penalty fires on competing-only content
//! - No stacks selected = all values neutral (backward compat)

use fourda_lib::stacks;

// ============================================================================
// Synthetic content generators
// ============================================================================

struct SyntheticItem {
    title: &'static str,
    content: &'static str,
    category: &'static str,
}

fn nextjs_items() -> Vec<SyntheticItem> {
    vec![
        // Direct match
        SyntheticItem { title: "Next.js 15 Release Notes", content: "nextjs react vercel app router server components improvements", category: "direct" },
        SyntheticItem { title: "Building with Next.js and TypeScript", content: "nextjs typescript react component patterns best practices", category: "direct" },
        SyntheticItem { title: "Vercel Edge Functions Deep Dive", content: "vercel edge runtime nextjs middleware deployment strategies", category: "direct" },
        SyntheticItem { title: "React Server Components Explained", content: "react server components nextjs rsc streaming ssr", category: "direct" },
        SyntheticItem { title: "TurboPack vs Webpack Performance", content: "turbopack nextjs webpack build performance bundler comparison", category: "direct" },
        // Pain point
        SyntheticItem { title: "App Router Migration Guide: Pages to App", content: "app router migration pages router next 13 next 14 breaking changes patterns", category: "pain_point" },
        SyntheticItem { title: "Server Components vs Client Components Boundaries", content: "server component client component use client directive rsc boundary mistakes", category: "pain_point" },
        SyntheticItem { title: "ISR Cache Invalidation Strategies", content: "isr revalidate cache stale incremental static regeneration nextjs", category: "pain_point" },
        SyntheticItem { title: "Edge Runtime Limitations You Should Know", content: "edge runtime middleware cold start limitations node api compatibility", category: "pain_point" },
        SyntheticItem { title: "Optimizing Next.js Bundle Size", content: "bundle size tree shaking code splitting webpack turbopack optimization", category: "pain_point" },
        // Ecosystem shift
        SyntheticItem { title: "Why We Switched from Prisma to Drizzle ORM", content: "drizzle prisma alternative orm migration performance type safety drizzle-orm", category: "ecosystem_shift" },
        SyntheticItem { title: "Biome: The ESLint Alternative Formatter", content: "biome eslint alternative biome formatter biomejs linter migration", category: "ecosystem_shift" },
        SyntheticItem { title: "Bun Runtime vs Node: Install and Build", content: "bun runtime bun install bun vs node performance benchmarks bunx", category: "ecosystem_shift" },
        // Competing
        SyntheticItem { title: "SvelteKit 2.0 Released", content: "sveltekit svelte framework release features improvements", category: "competing" },
        SyntheticItem { title: "Remix Framework Performance Guide", content: "remix framework performance routing loader actions", category: "competing" },
        SyntheticItem { title: "Astro 4.0 Content Collections", content: "astro static site generator content collections islands architecture", category: "competing" },
        // Off-domain
        SyntheticItem { title: "Kubernetes Cluster Autoscaling", content: "kubernetes cluster autoscaling pods nodes infrastructure", category: "off_domain" },
        SyntheticItem { title: "PostgreSQL 17 New Features", content: "postgresql database features improvements performance sql", category: "off_domain" },
        SyntheticItem { title: "Introduction to Rust Programming", content: "rust programming language beginner guide ownership borrowing", category: "off_domain" },
        SyntheticItem { title: "Docker Compose Best Practices", content: "docker compose containers orchestration deployment best practices", category: "off_domain" },
        SyntheticItem { title: "Machine Learning Model Training", content: "machine learning training pytorch neural network optimization", category: "off_domain" },
        // Cross-cutting
        SyntheticItem { title: "TypeScript 5.5 Type System Improvements", content: "typescript type system improvements generics inference", category: "cross_cutting" },
        SyntheticItem { title: "Web Performance Core Vitals 2024", content: "web performance core vitals lighthouse optimization metrics", category: "cross_cutting" },
        SyntheticItem { title: "OWASP Top 10 Web Security Threats", content: "security owasp web application vulnerabilities xss csrf", category: "cross_cutting" },
        // Synergy (multi-tech)
        SyntheticItem { title: "Next.js + Drizzle ORM Full Stack Tutorial", content: "nextjs drizzle typescript react server components full stack database vercel app router", category: "synergy" },
        SyntheticItem { title: "Deploying Next.js to Vercel with Edge Functions", content: "nextjs vercel deploy edge functions middleware typescript production turbopack app router", category: "synergy" },
        SyntheticItem { title: "Next.js React Server Components with tRPC and Zod", content: "react server components trpc zod typescript nextjs type-safe api vercel server action", category: "synergy" },
    ]
}

fn rust_items() -> Vec<SyntheticItem> {
    vec![
        // Direct match
        SyntheticItem {
            title: "Rust 1.80 Release Highlights",
            content: "rust release features improvements cargo clippy",
            category: "direct",
        },
        SyntheticItem {
            title: "Tokio Runtime Internals",
            content: "tokio runtime async executor task scheduling rust",
            category: "direct",
        },
        SyntheticItem {
            title: "Serde Serialization Patterns",
            content: "serde serialization deserialization json rust derive macros",
            category: "direct",
        },
        SyntheticItem {
            title: "Axum Web Framework Tutorial",
            content: "axum web framework rust tokio tower middleware routing",
            category: "direct",
        },
        SyntheticItem {
            title: "Cargo Workspace Organization",
            content: "cargo workspace organization monorepo rust project structure",
            category: "direct",
        },
        // Pain point
        SyntheticItem {
            title: "Understanding Async Lifetimes in Rust",
            content: "async lifetime future pin send tokio borrow checker complexity",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Reducing Rust Compile Times",
            content: "compile time build time incremental compilation cargo build optimization",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Safe Abstractions Over Unsafe Code",
            content: "unsafe soundness undefined behavior miri verification safety",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Error Handling Patterns in Rust",
            content: "error handling thiserror anyhow result error type custom propagation",
            category: "pain_point",
        },
        SyntheticItem {
            title: "When the Borrow Checker Fights Back",
            content: "borrow checker ownership move semantics lifetime annotation tips",
            category: "pain_point",
        },
        // Ecosystem shift
        SyntheticItem {
            title: "Native Async Trait: Async Fn in Trait Stabilized",
            content:
                "native async trait async fn in trait return position impl trait stabilization",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Const Generics Stabilization and Feature Gate Removal",
            content: "const generics generic const stabilization feature gate stable rust nightly",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Return Position Impl Trait and Native Async Trait",
            content: "return position impl trait native async trait async fn in trait stable",
            category: "ecosystem_shift",
        },
        // Competing
        SyntheticItem {
            title: "Go 1.23 Iterator Functions",
            content: "go golang iterator range-over-func new features",
            category: "competing",
        },
        SyntheticItem {
            title: "Zig Build System Deep Dive",
            content: "zig programming language build system comptime safety",
            category: "competing",
        },
        SyntheticItem {
            title: "Modern C++ Memory Safety",
            content: "c++ cpp memory safety smart pointers raii modern",
            category: "competing",
        },
        // Off-domain
        SyntheticItem {
            title: "React 19 New Features",
            content: "react frontend javascript components hooks new features",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Laravel Livewire Tutorial",
            content: "laravel livewire php web development blade components",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Django ORM Performance Tips",
            content: "django orm queryset python database optimization",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Vue 3 Composition API Guide",
            content: "vue composition api setup reactive ref computed",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Mobile App UI Design Patterns",
            content: "mobile app design ui ux patterns components interface",
            category: "off_domain",
        },
        // Cross-cutting
        SyntheticItem {
            title: "WebAssembly 2.0 Specification",
            content: "webassembly wasm specification runtime browser performance",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "Comparing Memory Allocators",
            content: "memory allocator jemalloc mimalloc performance comparison",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "gRPC Best Practices",
            content: "grpc protocol buffers protobuf api design best practices",
            category: "cross_cutting",
        },
        // Synergy
        SyntheticItem {
            title: "Building a Tauri App with Rust and React",
            content: "tauri rust react typescript desktop app development serde tokio",
            category: "synergy",
        },
        SyntheticItem {
            title: "Axum + SQLx REST API in Rust",
            content: "axum sqlx rust tokio rest api database postgresql web server serde",
            category: "synergy",
        },
        SyntheticItem {
            title: "Rust WASM with Tokio for High-Performance Web",
            content: "rust wasm tokio async web performance webassembly browser cargo",
            category: "synergy",
        },
    ]
}

fn python_ml_items() -> Vec<SyntheticItem> {
    vec![
        // Direct match
        SyntheticItem {
            title: "PyTorch 2.3 New Features",
            content: "pytorch deep learning training model torch tensors",
            category: "direct",
        },
        SyntheticItem {
            title: "Hugging Face Transformers Tutorial",
            content: "transformers huggingface model pipeline nlp bert gpt",
            category: "direct",
        },
        SyntheticItem {
            title: "NumPy Performance Optimization",
            content: "numpy array vectorization broadcasting performance python",
            category: "direct",
        },
        SyntheticItem {
            title: "LLM Fine-Tuning with LoRA",
            content: "llm fine-tuning lora peft huggingface transformers training",
            category: "direct",
        },
        SyntheticItem {
            title: "Building RAG Pipelines with LangChain",
            content: "rag retrieval augmented langchain embedding pipeline vector",
            category: "direct",
        },
        // Pain point
        SyntheticItem {
            title: "CUDA Version Driver Compatibility Issues",
            content: "cuda version driver nvcc nvidia gpu compatibility toolkit installation",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Fixing GPU Out of Memory Errors",
            content: "gpu oom out of memory vram memory allocation batch size gradient",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Python Dependency Hell with Conda and Pip",
            content: "dependency pip conda virtual environment package conflict resolution",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Reproducibility in Model Training",
            content: "reproducibility seed deterministic random state pytorch training results",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Model Serving and Inference Latency",
            content: "model serving inference latency deployment onnx optimization production",
            category: "pain_point",
        },
        // Ecosystem shift
        SyntheticItem {
            title: "JAX for Research: JIT Compilation and XLA",
            content: "jax flax jit compilation xla jax research tpu accelerator",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "GGUF Format and llama.cpp Quantization",
            content: "gguf ggml quantization format llama.cpp local inference efficient",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Local LLM with Ollama: Self-Hosted AI",
            content: "local llm ollama self-hosted on-device edge inference privacy",
            category: "ecosystem_shift",
        },
        // Competing
        SyntheticItem {
            title: "TensorFlow 2.16 Release",
            content: "tensorflow keras deep learning model training google",
            category: "competing",
        },
        SyntheticItem {
            title: "JAX vs PyTorch Comparison",
            content: "jax pytorch comparison performance research production",
            category: "competing",
        },
        SyntheticItem {
            title: "MXNet Architecture Guide",
            content: "mxnet deep learning framework distributed training",
            category: "competing",
        },
        // Off-domain
        SyntheticItem {
            title: "Rust Async Patterns",
            content: "rust async tokio future pin send systems programming",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Next.js App Router Guide",
            content: "nextjs app router react server components vercel",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Kubernetes Pod Scheduling",
            content: "kubernetes pods scheduling affinity taints resources",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Laravel Queue Management",
            content: "laravel queue jobs workers horizon php redis",
            category: "off_domain",
        },
        SyntheticItem {
            title: "CSS Grid Layout Patterns",
            content: "css grid layout responsive design web frontend",
            category: "off_domain",
        },
        // Cross-cutting
        SyntheticItem {
            title: "GPU Computing Architecture Overview",
            content: "gpu computing cuda architecture parallel processing threads",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "Data Pipeline Best Practices",
            content: "data pipeline etl streaming batch processing workflow",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "API Design for ML Services",
            content: "api design rest grpc machine learning service endpoints",
            category: "cross_cutting",
        },
        // Synergy
        SyntheticItem {
            title: "PyTorch + Transformers RAG Pipeline",
            content:
                "pytorch transformers rag retrieval augmented generation huggingface embedding",
            category: "synergy",
        },
        SyntheticItem {
            title: "Building LLM Apps with LangChain and Ollama",
            content: "llm langchain ollama local embedding pytorch transformers rag",
            category: "synergy",
        },
        SyntheticItem {
            title: "Fine-Tuning with PyTorch and Hugging Face",
            content:
                "fine-tuning pytorch huggingface transformers lora training model optimization",
            category: "synergy",
        },
    ]
}

fn go_backend_items() -> Vec<SyntheticItem> {
    vec![
        // Direct match
        SyntheticItem {
            title: "Go 1.23 Release Notes",
            content: "golang release features improvements standard library",
            category: "direct",
        },
        SyntheticItem {
            title: "Building gRPC Services in Go",
            content: "grpc golang protobuf service api microservice",
            category: "direct",
        },
        SyntheticItem {
            title: "Kubernetes Operator in Go",
            content: "kubernetes operator golang controller reconciler custom resource",
            category: "direct",
        },
        SyntheticItem {
            title: "Docker Multi-Stage Builds for Go",
            content: "docker golang multi-stage build container image optimization",
            category: "direct",
        },
        SyntheticItem {
            title: "Go Goroutine Patterns",
            content: "goroutine golang concurrency channel select patterns",
            category: "direct",
        },
        // Pain point
        SyntheticItem {
            title: "Go Error Handling: if err != nil Patterns",
            content: "error handling if err != nil error wrapping errors.Is golang",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Go Generics Type Parameter Constraints",
            content: "generics type parameter type constraint interface{} golang limitations",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Context Propagation in Go Services",
            content: "context context propagation context.WithTimeout ctx golang patterns",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Go Module Dependency Conflicts",
            content: "module go.mod dependency replace directive module conflict resolution",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Go Error Wrapping and errors.Is Deep Dive",
            content: "error handling error wrapping errors.Is fmt.Errorf golang stack",
            category: "pain_point",
        },
        // Ecosystem shift
        SyntheticItem {
            title: "Slog: Structured Logging with Slog Handler",
            content: "slog structured logging log/slog slog handler golang standard library",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Range Over Func: Go 1.23 Iterator with iter.Seq",
            content: "range over func iterator iter.Seq go 1.23 golang sequence",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Go WASM with Wazero: WebAssembly Runtime",
            content: "go wasm wazero tinygo webassembly runtime browser edge",
            category: "ecosystem_shift",
        },
        // Competing
        SyntheticItem {
            title: "Rust for Backend Services",
            content: "rust backend web services actix axum systems programming",
            category: "competing",
        },
        SyntheticItem {
            title: "Java Spring Boot Microservices",
            content: "java spring boot microservices cloud native enterprise",
            category: "competing",
        },
        SyntheticItem {
            title: "Node.js Performance Optimization",
            content: "node javascript backend performance event loop async",
            category: "competing",
        },
        // Off-domain
        SyntheticItem {
            title: "React Component Architecture",
            content: "react components hooks state management frontend design",
            category: "off_domain",
        },
        SyntheticItem {
            title: "PyTorch Training Pipeline",
            content: "pytorch training deep learning model gpu optimization",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Laravel Eloquent Relationships",
            content: "laravel eloquent orm relationships php database queries",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Vue Composition API Patterns",
            content: "vue composition api setup ref reactive computed watchers",
            category: "off_domain",
        },
        SyntheticItem {
            title: "iOS Swift UI Development",
            content: "swift ios ui development apple mobile app interface",
            category: "off_domain",
        },
        // Cross-cutting
        SyntheticItem {
            title: "Microservice Architecture Patterns",
            content: "microservice architecture patterns saga circuit breaker",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "Observability with Prometheus and Grafana",
            content: "observability prometheus grafana metrics monitoring alerting",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "Container Security Best Practices",
            content: "container security docker scanning vulnerabilities hardening",
            category: "cross_cutting",
        },
        // Synergy
        SyntheticItem {
            title: "Go gRPC Microservice on Kubernetes",
            content: "golang grpc kubernetes microservice protobuf docker container deployment",
            category: "synergy",
        },
        SyntheticItem {
            title: "Go Docker Container with Kubernetes Operator",
            content: "golang docker kubernetes operator custom resource controller reconciler",
            category: "synergy",
        },
        SyntheticItem {
            title: "Building Go CLI with gRPC and Protobuf",
            content: "golang grpc protobuf cli tool command line kubernetes api",
            category: "synergy",
        },
    ]
}

fn react_native_items() -> Vec<SyntheticItem> {
    vec![
        // Direct match
        SyntheticItem {
            title: "React Native 0.75 Release",
            content: "react-native release features improvements mobile",
            category: "direct",
        },
        SyntheticItem {
            title: "Expo SDK 52 New Features",
            content: "expo sdk mobile development react-native eas build",
            category: "direct",
        },
        SyntheticItem {
            title: "React Native Navigation Patterns",
            content: "react-native navigation screens stack tabs drawer",
            category: "direct",
        },
        SyntheticItem {
            title: "React Native Performance Tips",
            content: "react-native performance optimization rendering mobile app",
            category: "direct",
        },
        SyntheticItem {
            title: "Expo Router File-Based Routing",
            content: "expo router file-based routing react-native navigation mobile",
            category: "direct",
        },
        // Pain point
        SyntheticItem {
            title: "React Native New Architecture Migration",
            content: "new architecture fabric turbo module bridgeless migration upgrade",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Hermes Engine Quirks and Workarounds",
            content: "hermes engine jsc javascript core hermes quirk compatibility issues",
            category: "pain_point",
        },
        SyntheticItem {
            title: "App Store Review Rejection Guide",
            content: "app store review rejection guideline app review compliance tips",
            category: "pain_point",
        },
        SyntheticItem {
            title: "OTA Updates with EAS Update",
            content: "ota over the air eas update expo update code push deployment",
            category: "pain_point",
        },
        SyntheticItem {
            title: "JS Thread Performance and Frame Drops",
            content: "js thread ui thread performance frame drop jank optimization",
            category: "pain_point",
        },
        // Ecosystem shift
        SyntheticItem {
            title: "Expo Go and EAS Build: Managed Workflow",
            content: "expo expo go eas build expo managed expo sdk mobile development",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Expo Router vs React Navigation",
            content: "expo router file-based routing react-navigation expo-router navigation",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "New Architecture with Fabric and Turbo Module",
            content: "new architecture fabric turbo module bridgeless migration react-native",
            category: "ecosystem_shift",
        },
        // Competing
        SyntheticItem {
            title: "Flutter 3.24 Release",
            content: "flutter dart mobile cross-platform widgets material design",
            category: "competing",
        },
        SyntheticItem {
            title: "Kotlin Multiplatform Mobile",
            content: "kotlin mobile cross-platform shared code android ios",
            category: "competing",
        },
        SyntheticItem {
            title: "Swift UI for iOS Development",
            content: "swift ios mobile apple ui declarative interface",
            category: "competing",
        },
        // Off-domain
        SyntheticItem {
            title: "Rust Systems Programming Guide",
            content: "rust systems programming ownership borrowing memory safety",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Django REST Framework Tutorial",
            content: "django rest framework api python serializers views",
            category: "off_domain",
        },
        SyntheticItem {
            title: "PostgreSQL Index Optimization",
            content: "postgresql index optimization query plan btree gin",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Go Concurrency Patterns",
            content: "golang goroutine channel concurrency patterns select",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Machine Learning with Scikit-Learn",
            content: "scikit-learn machine learning classification regression python",
            category: "off_domain",
        },
        // Cross-cutting
        SyntheticItem {
            title: "Mobile App Accessibility Guidelines",
            content: "accessibility mobile a11y screen reader voiceover talkback",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "CI/CD for Mobile Apps",
            content: "ci cd mobile pipeline build deploy testing automation",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "Push Notification Best Practices",
            content: "push notification mobile engagement user retention messaging",
            category: "cross_cutting",
        },
        // Synergy
        SyntheticItem {
            title: "Expo React Native App with TypeScript",
            content: "expo react-native typescript mobile app eas build navigation nativewind",
            category: "synergy",
        },
        SyntheticItem {
            title: "React Native Reanimated with Expo Router",
            content: "react-native reanimated expo router gesture-handler animation mobile expo",
            category: "synergy",
        },
        SyntheticItem {
            title: "React Native + Zustand State with Expo",
            content: "react-native zustand state management expo typescript mobile app",
            category: "synergy",
        },
    ]
}

fn laravel_items() -> Vec<SyntheticItem> {
    vec![
        // Direct match
        SyntheticItem {
            title: "Laravel 11 Release Notes",
            content: "laravel release features improvements php framework",
            category: "direct",
        },
        SyntheticItem {
            title: "Eloquent ORM Advanced Queries",
            content: "eloquent orm query builder laravel relationships php",
            category: "direct",
        },
        SyntheticItem {
            title: "Laravel Livewire Components",
            content: "livewire laravel components reactive php blade",
            category: "direct",
        },
        SyntheticItem {
            title: "Laravel Horizon Queue Dashboard",
            content: "horizon queue jobs workers laravel redis monitoring",
            category: "direct",
        },
        SyntheticItem {
            title: "Blade Template Engine Tips",
            content: "blade template laravel components directives slots php",
            category: "direct",
        },
        // Pain point
        SyntheticItem {
            title: "PHP 8 Version Migration for Laravel",
            content: "php version php 8 migration upgrade php compatibility laravel",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Queue Job Reliability with Horizon",
            content: "queue job failed retry horizon worker laravel reliability",
            category: "pain_point",
        },
        SyntheticItem {
            title: "N+1 Query Problem and Eager Loading",
            content: "n+1 eager loading query eloquent performance lazy loading optimization",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Laravel Deployment with Forge and Docker",
            content: "deployment forge envoyer vapor docker laravel production server",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Eloquent N+1 Detection and Fixes",
            content: "n+1 eager loading eloquent performance query lazy loading laravel",
            category: "pain_point",
        },
        // Ecosystem shift
        SyntheticItem {
            title: "Livewire 3: wire:navigate and V3 Upgrade",
            content: "livewire 3 livewire v3 livewire upgrade wire:navigate alpine morphing",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Filament Admin Panel: Filament V3 Guide",
            content: "filament filament admin filament v3 filament panel laravel admin",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Pest V3 Testing: Arch Testing and More",
            content: "pest pest v3 pest testing arch testing phpunit migration laravel",
            category: "ecosystem_shift",
        },
        // Competing
        SyntheticItem {
            title: "Symfony 7 Framework Guide",
            content: "symfony php framework components bundles enterprise",
            category: "competing",
        },
        SyntheticItem {
            title: "Django vs Laravel Comparison",
            content: "django python laravel php framework comparison features",
            category: "competing",
        },
        SyntheticItem {
            title: "Ruby on Rails 8 Release",
            content: "rails ruby web framework mvc active record",
            category: "competing",
        },
        // Off-domain
        SyntheticItem {
            title: "Rust Ownership and Borrowing",
            content: "rust ownership borrowing lifetime memory safety systems",
            category: "off_domain",
        },
        SyntheticItem {
            title: "React Hooks Deep Dive",
            content: "react hooks usestate useeffect components frontend",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Go Backend Architecture",
            content: "golang backend microservices grpc kubernetes api",
            category: "off_domain",
        },
        SyntheticItem {
            title: "PyTorch Training Pipeline",
            content: "pytorch training deep learning model gpu cuda",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Kubernetes Networking Guide",
            content: "kubernetes networking services ingress load balancing",
            category: "off_domain",
        },
        // Cross-cutting
        SyntheticItem {
            title: "PHP Security Best Practices",
            content: "php security xss csrf sql injection validation sanitization",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "Database Migration Strategies",
            content: "database migration schema versioning rollback strategy",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "Redis Caching Patterns",
            content: "redis caching patterns ttl invalidation pub sub",
            category: "cross_cutting",
        },
        // Synergy
        SyntheticItem {
            title: "Laravel Livewire with Filament Admin",
            content: "laravel livewire filament admin panel php blade components",
            category: "synergy",
        },
        SyntheticItem {
            title: "Laravel + Inertia + Vue Full Stack",
            content: "laravel inertia vue php typescript full stack spa",
            category: "synergy",
        },
        SyntheticItem {
            title: "Laravel Horizon with Redis Queue Jobs",
            content: "laravel horizon redis queue jobs workers blade monitoring php",
            category: "synergy",
        },
    ]
}

fn django_items() -> Vec<SyntheticItem> {
    vec![
        // Direct match
        SyntheticItem {
            title: "Django 5.1 Release Notes",
            content: "django release features improvements python framework",
            category: "direct",
        },
        SyntheticItem {
            title: "Django REST Framework Serializers",
            content: "drf serializers viewsets routers django rest api python",
            category: "direct",
        },
        SyntheticItem {
            title: "Celery Task Queue with Django",
            content: "celery task queue django workers redis python async",
            category: "direct",
        },
        SyntheticItem {
            title: "Django ORM QuerySet Guide",
            content: "django orm queryset filter annotate aggregate python",
            category: "direct",
        },
        SyntheticItem {
            title: "PostgreSQL with Django Setup",
            content: "postgresql django database configuration python migration",
            category: "direct",
        },
        // Pain point
        SyntheticItem {
            title: "Django ORM N+1 with select_related",
            content: "orm queryset n+1 select_related prefetch_related django performance",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Django Async Views and ASGI Channels",
            content: "async asgi channels async view django async support python",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Django Migration Conflict Resolution",
            content: "migration conflict merge squash makemigrations django database",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Speeding Up Django Test Suite",
            content: "test speed pytest fixture factory test database django optimization",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Django QuerySet Performance Tuning",
            content: "orm queryset select_related prefetch_related n+1 django optimization",
            category: "pain_point",
        },
        // Ecosystem shift
        SyntheticItem {
            title: "Django Ninja API: Pydantic and FastAPI Style",
            content: "django-ninja ninja api pydantic django ninja fast type-safe",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "HTMX with Django: Hypermedia and hx-get",
            content: "htmx hypermedia hx-get hx-post html over the wire django templates",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Wagtail CMS: StreamField and Wagtail Page",
            content: "wagtail wagtail cms streamfield wagtail page content management",
            category: "ecosystem_shift",
        },
        // Competing
        SyntheticItem {
            title: "FastAPI Performance Benchmarks",
            content: "fastapi python async api performance starlette uvicorn",
            category: "competing",
        },
        SyntheticItem {
            title: "Flask 3.0 New Features",
            content: "flask python web framework lightweight blueprints",
            category: "competing",
        },
        SyntheticItem {
            title: "Ruby on Rails Active Record Patterns",
            content: "rails ruby active record orm database patterns migration",
            category: "competing",
        },
        // Off-domain
        SyntheticItem {
            title: "Next.js Server Components",
            content: "nextjs react server components rsc streaming ssr",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Rust Cargo Build System",
            content: "rust cargo build workspace dependencies compilation",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Go Kubernetes Operator Tutorial",
            content: "golang kubernetes operator controller custom resource",
            category: "off_domain",
        },
        SyntheticItem {
            title: "React Native Expo Guide",
            content: "react-native expo mobile app development typescript",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Docker Compose Networking",
            content: "docker compose network containers bridge host overlay",
            category: "off_domain",
        },
        // Cross-cutting
        SyntheticItem {
            title: "Python Async Programming",
            content: "python async asyncio event loop coroutine concurrent",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "SQL Query Optimization Techniques",
            content: "sql query optimization index explain plan performance",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "API Authentication Patterns",
            content: "api authentication oauth jwt token session security",
            category: "cross_cutting",
        },
        // Synergy
        SyntheticItem {
            title: "Django REST Framework with Celery Tasks",
            content: "drf django rest framework celery task queue redis python api",
            category: "synergy",
        },
        SyntheticItem {
            title: "Django + HTMX + PostgreSQL Full Stack",
            content: "django htmx postgresql python hypermedia database templates",
            category: "synergy",
        },
        SyntheticItem {
            title: "Django Ninja with Celery and Redis",
            content: "django-ninja celery redis python task queue api pydantic",
            category: "synergy",
        },
    ]
}

fn vue_frontend_items() -> Vec<SyntheticItem> {
    vec![
        // Direct match
        SyntheticItem {
            title: "Vue 3.5 Release Notes",
            content: "vue release features improvements reactivity composition",
            category: "direct",
        },
        SyntheticItem {
            title: "Nuxt 3 Server-Side Rendering",
            content: "nuxt ssr server rendering vue nitro auto-imports",
            category: "direct",
        },
        SyntheticItem {
            title: "Pinia State Management Guide",
            content: "pinia state management vue store reactive getters actions",
            category: "direct",
        },
        SyntheticItem {
            title: "Vite Build Configuration",
            content: "vite build configuration plugins vue optimization rollup",
            category: "direct",
        },
        SyntheticItem {
            title: "VueUse Composables Collection",
            content: "vueuse composables vue utility hooks reactive helpers",
            category: "direct",
        },
        // Pain point
        SyntheticItem {
            title: "Composition API Migration from Options",
            content: "composition api options api migration setup script setup vue patterns",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Nuxt SSR Hydration Mismatch Fixes",
            content: "ssr hydration mismatch nuxt ssr server render client mismatch",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Vue TypeScript Integration Issues",
            content: "typescript type defineComponent vue typescript vue types generics",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Vuex to Pinia Store Migration",
            content: "vuex pinia state management store migration vuex to pinia patterns",
            category: "pain_point",
        },
        SyntheticItem {
            title: "Vue Composition API Setup Patterns",
            content: "composition api setup script setup migration options api vue reactive",
            category: "pain_point",
        },
        // Ecosystem shift
        SyntheticItem {
            title: "Vue Vapor Mode: No Virtual DOM Compile-Time",
            content: "vue vapor vapor mode compile-time no virtual dom performance",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Nuxt 4 Upgrade and Nuxt Migration Guide",
            content: "nuxt 4 nuxt upgrade nuxt migration nuxt next improvements",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "UnoCSS Atomic CSS with UnoCSS Preset",
            content: "unocss uno css atomic css unocss preset tailwind alternative",
            category: "ecosystem_shift",
        },
        // Competing
        SyntheticItem {
            title: "React 19 Concurrent Features",
            content: "react concurrent rendering suspense transitions server",
            category: "competing",
        },
        SyntheticItem {
            title: "Angular 18 Standalone Components",
            content: "angular standalone components signals zoneless change",
            category: "competing",
        },
        SyntheticItem {
            title: "Svelte 5 Runes System",
            content: "svelte runes reactive state signals fine-grained",
            category: "competing",
        },
        // Off-domain
        SyntheticItem {
            title: "Rust Error Handling Patterns",
            content: "rust error handling thiserror anyhow result type",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Python ML Pipeline Guide",
            content: "python pytorch training pipeline model data preprocessing",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Go Microservice Architecture",
            content: "golang microservice grpc kubernetes docker deployment",
            category: "off_domain",
        },
        SyntheticItem {
            title: "Laravel Blade Components",
            content: "laravel blade php components templates slots directives",
            category: "off_domain",
        },
        SyntheticItem {
            title: "PostgreSQL JSON Operations",
            content: "postgresql json jsonb query operators indexing storage",
            category: "off_domain",
        },
        // Cross-cutting
        SyntheticItem {
            title: "Frontend State Management Patterns",
            content: "state management frontend patterns flux redux signals stores",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "Web Component Standards",
            content: "web components custom elements shadow dom html templates",
            category: "cross_cutting",
        },
        SyntheticItem {
            title: "CSS-in-JS vs Utility CSS",
            content: "css styling utility tailwind styled-components emotion",
            category: "cross_cutting",
        },
        // Synergy
        SyntheticItem {
            title: "Nuxt 3 + Pinia + Vue Router App",
            content: "nuxt pinia vue router state management ssr nitro composables vueuse",
            category: "synergy",
        },
        SyntheticItem {
            title: "Vue 3 + Vite + UnoCSS Setup",
            content: "vue vite unocss pinia composition api typescript build tooling",
            category: "synergy",
        },
        SyntheticItem {
            title: "Nuxt with Vitest and Vue Test Utils",
            content: "nuxt vitest vue test utils pinia testing composition api vue",
            category: "synergy",
        },
    ]
}

// ============================================================================
// Test helpers
// ============================================================================

fn compute_category_avg(
    items: &[SyntheticItem],
    category: &str,
    stack: &stacks::ComposedStack,
) -> f32 {
    let matching: Vec<_> = items.iter().filter(|i| i.category == category).collect();
    if matching.is_empty() {
        return 0.0;
    }
    let total: f32 = matching
        .iter()
        .map(|item| stacks::scoring::compute_stack_boost(item.title, item.content, stack))
        .sum();
    total / matching.len() as f32
}

fn compute_category_shift_avg(
    items: &[SyntheticItem],
    category: &str,
    stack: &stacks::ComposedStack,
) -> f32 {
    let matching: Vec<_> = items.iter().filter(|i| i.category == category).collect();
    if matching.is_empty() {
        return 1.0;
    }
    let total: f32 = matching
        .iter()
        .map(|item| {
            // Generate topics from both title AND content (matches real pipeline behavior)
            let topics: Vec<String> = format!("{} {}", item.title, item.content)
                .to_lowercase()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            stacks::scoring::detect_ecosystem_shift(&topics, item.title, stack)
        })
        .sum();
    total / matching.len() as f32
}

fn compute_category_pain_rate(
    items: &[SyntheticItem],
    category: &str,
    stack: &stacks::ComposedStack,
) -> f32 {
    let matching: Vec<_> = items.iter().filter(|i| i.category == category).collect();
    if matching.is_empty() {
        return 0.0;
    }
    let pain_matches = matching
        .iter()
        .filter(|item| stacks::scoring::has_pain_point_match(item.title, item.content, stack))
        .count();
    pain_matches as f32 / matching.len() as f32
}

fn compute_category_competing_penalty_avg(
    items: &[SyntheticItem],
    category: &str,
    stack: &stacks::ComposedStack,
) -> f32 {
    let matching: Vec<_> = items.iter().filter(|i| i.category == category).collect();
    if matching.is_empty() {
        return 1.0;
    }
    let total: f32 = matching
        .iter()
        .map(|item| stacks::scoring::compute_competing_penalty(item.title, item.content, stack))
        .sum();
    total / matching.len() as f32
}

// ============================================================================
// Profile simulation tests
// ============================================================================

macro_rules! profile_simulation {
    ($name:ident, $profile_id:expr, $items_fn:expr) => {
        mod $name {
            use super::*;

            fn stack() -> stacks::ComposedStack {
                stacks::compose_profiles(&[$profile_id.to_string()])
            }

            fn items() -> Vec<SyntheticItem> {
                $items_fn()
            }

            #[test]
            fn pain_point_lift() {
                let s = stack();
                let i = items();
                let pain_avg = compute_category_avg(&i, "pain_point", &s);
                let off_domain_avg = compute_category_avg(&i, "off_domain", &s);
                let lift = pain_avg - off_domain_avg;
                assert!(
                    lift >= 0.05,
                    "{}: Pain point lift ({:.4}) should be >= 0.05 (pain={:.4}, off={:.4})",
                    $profile_id,
                    lift,
                    pain_avg,
                    off_domain_avg
                );
            }

            #[test]
            fn pain_point_detection_rate() {
                let s = stack();
                let i = items();
                let rate = compute_category_pain_rate(&i, "pain_point", &s);
                assert!(
                    rate >= 0.60,
                    "{}: Pain point detection rate ({:.2}) should be >= 0.60",
                    $profile_id,
                    rate
                );
            }

            #[test]
            fn off_domain_suppression() {
                let s = stack();
                let i = items();
                let off_avg = compute_category_avg(&i, "off_domain", &s);
                assert!(
                    off_avg <= 0.10,
                    "{}: Off-domain avg boost ({:.4}) should be <= 0.10",
                    $profile_id,
                    off_avg
                );
            }

            #[test]
            fn ecosystem_shift_detection() {
                let s = stack();
                let i = items();
                let shift_avg = compute_category_shift_avg(&i, "ecosystem_shift", &s);
                assert!(
                    shift_avg > 1.0,
                    "{}: Ecosystem shift avg ({:.4}) should be > 1.0",
                    $profile_id,
                    shift_avg
                );
            }

            #[test]
            fn synergy_boost() {
                let s = stack();
                let i = items();
                let synergy_avg = compute_category_avg(&i, "synergy", &s);
                let direct_avg = compute_category_avg(&i, "direct", &s);
                assert!(
                    synergy_avg >= direct_avg,
                    "{}: Synergy avg ({:.4}) should be >= direct avg ({:.4})",
                    $profile_id,
                    synergy_avg,
                    direct_avg
                );
            }

            #[test]
            fn direct_match_boost() {
                let s = stack();
                let i = items();
                let direct_avg = compute_category_avg(&i, "direct", &s);
                assert!(
                    direct_avg >= 0.05,
                    "{}: Direct match avg ({:.4}) should be >= 0.05",
                    $profile_id,
                    direct_avg
                );
            }

            #[test]
            fn competing_penalty_fires() {
                let s = stack();
                let i = items();
                let competing_avg = compute_category_competing_penalty_avg(&i, "competing", &s);
                assert!(
                    competing_avg < 1.0,
                    "{}: Competing content avg penalty ({:.4}) should be < 1.0",
                    $profile_id,
                    competing_avg
                );
            }
        }
    };
}

profile_simulation!(nextjs_sim, "nextjs_fullstack", nextjs_items);
profile_simulation!(rust_sim, "rust_systems", rust_items);
profile_simulation!(python_ml_sim, "python_ml", python_ml_items);
profile_simulation!(go_backend_sim, "go_backend", go_backend_items);
profile_simulation!(react_native_sim, "react_native", react_native_items);
profile_simulation!(laravel_sim, "laravel", laravel_items);
profile_simulation!(django_sim, "django", django_items);
profile_simulation!(vue_frontend_sim, "vue_frontend", vue_frontend_items);

// ============================================================================
// Backward compatibility — no stack selected = neutral
// ============================================================================

#[test]
fn backward_compat_no_stack_neutral_boost() {
    let empty = stacks::ComposedStack::default();
    assert!(!empty.active);

    let boost = stacks::scoring::compute_stack_boost(
        "Rust async runtime improvements",
        "async tokio pin send lifetime improvements",
        &empty,
    );
    assert_eq!(boost, 0.0, "No stack selected should give 0.0 boost");
}

#[test]
fn backward_compat_no_stack_neutral_shift() {
    let empty = stacks::ComposedStack::default();
    let mult = stacks::scoring::detect_ecosystem_shift(
        &["drizzle".to_string()],
        "Drizzle replacing Prisma",
        &empty,
    );
    assert_eq!(mult, 1.0, "No stack selected should give 1.0 multiplier");
}

#[test]
fn backward_compat_no_stack_no_pain_match() {
    let empty = stacks::ComposedStack::default();
    let matched = stacks::scoring::has_pain_point_match(
        "Async lifetime challenges",
        "async pin send lifetime complexity",
        &empty,
    );
    assert!(!matched, "No stack selected should give false pain match");
}

#[test]
fn backward_compat_no_stack_no_competing_penalty() {
    let empty = stacks::ComposedStack::default();
    let penalty = stacks::scoring::compute_competing_penalty(
        "Go Backend Performance",
        "golang goroutine optimization",
        &empty,
    );
    assert_eq!(
        penalty, 1.0,
        "No stack selected should give 1.0 (no penalty)"
    );
}

// ============================================================================
// Word-boundary regression tests
// ============================================================================

#[test]
fn word_boundary_go_not_in_google() {
    let stack = stacks::compose_profiles(&["go_backend".to_string()]);
    let boost = stacks::scoring::compute_stack_boost(
        "Google Cloud Platform New Features",
        "google cloud storage algorithms ergonomic apis",
        &stack,
    );
    assert_eq!(
        boost, 0.0,
        "Go stack should NOT boost Google content (got {})",
        boost
    );
}

#[test]
fn word_boundary_rust_not_in_trust() {
    let stack = stacks::compose_profiles(&["rust_systems".to_string()]);
    let boost = stacks::scoring::compute_stack_boost(
        "Building Trust in Software Teams",
        "frustrated developers need better onboarding and trust",
        &stack,
    );
    assert_eq!(
        boost, 0.0,
        "Rust stack should NOT boost 'trust' content (got {})",
        boost
    );
}

#[test]
fn word_boundary_vue_not_in_revenue() {
    let stack = stacks::compose_profiles(&["vue_frontend".to_string()]);
    let boost = stacks::scoring::compute_stack_boost(
        "Revenue Growth Strategies for SaaS",
        "revenue metrics growth saas pricing strategy",
        &stack,
    );
    assert_eq!(
        boost, 0.0,
        "Vue stack should NOT boost 'revenue' content (got {})",
        boost
    );
}

// ============================================================================
// Composability tests — multi-profile
// ============================================================================

#[test]
fn multi_profile_rust_plus_nextjs() {
    let composed =
        stacks::compose_profiles(&["rust_systems".to_string(), "nextjs_fullstack".to_string()]);
    assert!(composed.active);

    // Rust pain point should still trigger
    let rust_pain = stacks::scoring::has_pain_point_match(
        "Understanding Async Lifetimes",
        "async lifetime pin send future complexity",
        &composed,
    );
    assert!(rust_pain, "Rust pain point should work in composed stack");

    // Next.js pain point should also trigger
    let nextjs_pain = stacks::scoring::has_pain_point_match(
        "App Router Migration Challenges",
        "app router migration pages router next 14 breaking changes",
        &composed,
    );
    assert!(
        nextjs_pain,
        "Next.js pain point should work in composed stack"
    );

    // Both techs should be in all_tech
    assert!(composed.all_tech.contains("rust"));
    assert!(composed.all_tech.contains("nextjs"));
}

#[test]
fn all_eight_profiles_compose() {
    let all_ids: Vec<String> = stacks::list_profiles()
        .iter()
        .map(|p| p.id.to_string())
        .collect();
    let composed = stacks::compose_profiles(&all_ids);
    assert!(composed.active);
    assert!(
        composed.pain_points.len() >= 30,
        "8 profiles should give 30+ pain points"
    );
    assert!(!composed.all_tech.is_empty());
    assert!(!composed.competing.is_empty());
}
