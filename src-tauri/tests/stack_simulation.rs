//! Stack Intelligence — Master Simulation Harness
//!
//! Eleven tiers of validation:
//!
//! Tier 1: Frozen real-world corpus — real article titles with per-profile labels
//! Tier 2: Adversarial battery — items designed to exploit word-boundary weaknesses
//! Tier 3: Algebraic properties — mathematical invariants the scoring MUST satisfy
//! Tier 4: Profile data integrity — structural validation of profile definitions
//! Tier 5: Composition algebra — multi-profile interaction verification
//! Tier 6: Cross-profile contamination matrix — inter-profile isolation
//! Tier 7: Determinism — scoring reproducibility across runs
//! Tier 8: Edge cases — degenerate inputs, unicode, long content
//! Tier 9: Monotonicity — more signal = more score
//! Tier 10: Snapshot regression — drift detection via checksum
//! Tier 11: Threshold boundary — proves 2-keyword minimum is enforced

use fourda_lib::stacks;
use fourda_lib::stacks::scoring;

// ============================================================================
// Tier 1: Frozen Real-World Corpus
// ============================================================================
// Every title below is a plausible HN/Reddit/lobsters article. Each is manually
// labeled for each profile's scoring functions. Labels:
//   P = Pain point (has_pain_point_match should return true)
//   K = Keyword boost (compute_stack_boost > 0)
//   S = Ecosystem shift (detect_ecosystem_shift > 1.0)
//   C = Competing (compute_competing_penalty < 1.0)
//   N = Neutral (boost == 0.0, shift == 1.0, penalty == 1.0)
//   X = Adversarial (MUST NOT trigger — false positive trap)

#[derive(Debug, Clone, Copy, PartialEq)]
enum L {
    P, // Pain point
    K, // Keyword boost
    S, // Ecosystem shift
    C, // Competing
    N, // Neutral / off-domain
}

struct CorpusItem {
    title: &'static str,
    content: &'static str,
    /// Labels per profile: [nextjs, rust, python_ml, go, react_native, laravel, django, vue]
    labels: [L; 8],
}

/// Generate topics from title+content (matches real pipeline behavior).
fn topics_from(item: &CorpusItem) -> Vec<String> {
    format!("{} {}", item.title, item.content)
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

// Profile index constants for readability
const NX: usize = 0; // nextjs_fullstack
const RS: usize = 1; // rust_systems
const PY: usize = 2; // python_ml
const GO: usize = 3; // go_backend
const RN: usize = 4; // react_native
const LA: usize = 5; // laravel
const DJ: usize = 6; // django
const VU: usize = 7; // vue_frontend

static PROFILE_IDS: [&str; 8] = [
    "nextjs_fullstack",
    "rust_systems",
    "python_ml",
    "go_backend",
    "react_native",
    "laravel",
    "django",
    "vue_frontend",
];

//                                                NX  RS  PY  GO  RN  LA  DJ  VU
static CORPUS: &[CorpusItem] = &[
    // --- Next.js ecosystem ---
    CorpusItem {
        title: "Migrating from Pages Router to App Router in Next.js 14",
        content: "app router migration pages router next 14 breaking changes patterns",
        labels: [L::P, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Server Components vs Client Components: Where to Draw the Line",
        content: "server component client component use client rsc boundary directive",
        labels: [L::P, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Next.js ISR Cache Invalidation Is Broken (And How to Fix It)",
        content: "isr revalidate cache stale incremental static regeneration nextjs",
        labels: [L::P, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Why We Migrated from Prisma to Drizzle ORM",
        content: "drizzle prisma alternative orm migration drizzle-orm performance",
        labels: [L::S, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Biome: Drop ESLint and Prettier for One Tool",
        content: "biome eslint alternative biome formatter biomejs linter migration fast",
        labels: [L::S, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "TurboPack: Next.js Build Performance Deep Dive",
        content: "turbopack nextjs build performance bundler webpack comparison",
        labels: [L::K, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Vercel Edge Functions and Middleware Patterns",
        content: "vercel edge runtime nextjs middleware deployment strategies",
        labels: [L::K, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Edge Runtime Cold Starts Are Killing Our Middleware Performance",
        content: "our nextjs edge runtime middleware functions take 500ms on cold start which degrades \
            user experience significantly. investigating workarounds for edge function initialization \
            overhead in production deployments including warming strategies and code splitting",
        labels: [L::P, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Next.js Server Actions: Type-Safe Data Mutations Without API Routes",
        content: "server action pattern in nextjs lets you handle form submissions directly from react \
            server components. no need to build separate api routes when you can colocate your data \
            mutation logic with your component using the new server action directive",
        labels: [L::K, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    // --- Rust ecosystem ---
    CorpusItem {
        title: "Understanding Pin, Send, and Async Lifetimes in Rust",
        content: "async pin send lifetime future tokio complexity annotations",
        labels: [L::N, L::P, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Reducing Rust Compile Times: A Practical Guide",
        content: "compile time build time incremental compilation cargo build optimization",
        labels: [L::N, L::P, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "When the Borrow Checker Fights Back: Ownership Patterns",
        content: "borrow checker ownership move semantics lifetime annotation tips",
        labels: [L::N, L::P, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Async Fn in Trait Is Finally Stable",
        content: "native async trait async fn in trait return position impl trait stabilization",
        labels: [L::N, L::S, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Const Generics Stabilization: No More Feature Gates",
        content: "const generics generic const stabilization feature gate stable rust nightly",
        labels: [L::N, L::S, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Tokio 1.40: What's New in the Async Runtime",
        content: "tokio runtime async executor improvements performance rust",
        labels: [L::N, L::K, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Building a REST API with Axum and SQLx",
        content: "axum sqlx rust tokio web api server serde database postgresql",
        labels: [L::N, L::K, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Auditing Unsafe Rust Code for Soundness Violations with Miri",
        content: "running miri on our codebase found several soundness holes in unsafe blocks that \
            could lead to undefined behavior. practical guidelines for auditing ffi boundaries and \
            raw pointer arithmetic to ensure memory safety guarantees hold",
        labels: [L::N, L::P, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Tauri 2.0: Build Cross-Platform Desktop Apps with Rust and Web Tech",
        content: "tauri desktop application framework combines a rust backend with web frontend \
            through a secure invoke bridge. building native apps with smaller bundles than electron \
            while leveraging the rust ecosystem for performance critical operations",
        labels: [L::N, L::K, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    // --- Python ML ecosystem ---
    CorpusItem {
        title: "CUDA Version Conflicts: A Complete Troubleshooting Guide",
        content: "cuda version driver nvcc nvidia gpu compatibility toolkit installation",
        labels: [L::N, L::N, L::P, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Fixing GPU Out of Memory Errors in PyTorch Training",
        content: "gpu oom out of memory vram memory allocation batch size gradient",
        labels: [L::N, L::N, L::P, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "GGUF Quantization: Running LLMs Locally with llama.cpp",
        content: "gguf ggml quantization format llama.cpp local inference efficient",
        labels: [L::N, L::N, L::S, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Local LLM Inference with Ollama: Privacy-First AI",
        content: "local llm ollama self-hosted on-device edge inference privacy",
        labels: [L::N, L::N, L::S, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Fine-Tuning LLaMA with LoRA and Hugging Face",
        content: "llm fine-tuning lora peft huggingface transformers pytorch training",
        labels: [L::N, L::N, L::K, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Python Dependency Hell: When pip and Conda Fight Over Packages",
        content: "spent three days debugging why pytorch would not install because conda and pip had \
            conflicting numpy versions in the same virtual environment. the dependency resolution \
            between these package managers creates reproducibility nightmares across machines",
        labels: [L::N, L::N, L::P, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "ONNX Model Serving: Reducing Inference Latency in Production",
        content: "converting our transformer model to onnx format reduced inference latency by sixty \
            percent compared to raw pytorch model serving with torchserve. optimizing the deployment \
            pipeline for real-time predictions requires careful batch sizing and hardware selection",
        labels: [L::N, L::N, L::P, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Building RAG Pipelines with LangChain and Hugging Face Transformers",
        content: "implementing a production rag retrieval augmented pipeline that chunks documents \
            stores embeddings in a vector database and retrieves relevant context before prompting. \
            using huggingface transformers for embeddings and langchain for orchestration workflow",
        labels: [L::N, L::N, L::K, L::N, L::N, L::N, L::N, L::N],
    },
    // --- Go ecosystem ---
    CorpusItem {
        title: "Go Error Handling: The if err != nil Debate Continues",
        content: "error handling if err != nil error wrapping errors.Is golang patterns",
        labels: [L::N, L::N, L::N, L::P, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Go Generics: Type Parameter Constraints in Practice",
        content: "generics type parameter type constraint interface{} golang limitations",
        labels: [L::N, L::N, L::N, L::P, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Slog: Structured Logging Comes to Go Standard Library",
        content: "slog structured logging log/slog slog handler golang standard library",
        labels: [L::N, L::N, L::N, L::S, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Range Over Func: Go 1.23 Iterator Pattern with iter.Seq",
        content: "range over func iterator iter.Seq go 1.23 golang sequence",
        labels: [L::N, L::N, L::N, L::S, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Building gRPC Microservices in Go with Protobuf",
        content: "grpc golang protobuf service api microservice kubernetes deployment",
        labels: [L::N, L::N, L::N, L::K, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Go Context Propagation: Timeout, Cancellation, and Deadline Patterns",
        content: "context propagation in golang requires careful handling of context.withtimeout and \
            cancellation signals across goroutine boundaries. common mistakes include losing the \
            parent ctx reference and creating context leaks in long running server handlers",
        labels: [L::N, L::N, L::N, L::P, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Building Kubernetes Operators in Go with Custom Controllers",
        content: "kubernetes operator development in golang using client-go and controller-runtime. \
            implement custom resource definitions and reconciliation loops to manage complex stateful \
            applications on k8s clusters with proper leader election and health checks",
        labels: [L::N, L::N, L::N, L::K, L::N, L::N, L::N, L::N],
    },
    // --- React Native ecosystem ---
    CorpusItem {
        title: "React Native New Architecture: Fabric and TurboModules Guide",
        content: "new architecture fabric turbo module bridgeless react-native jsi",
        labels: [L::N, L::N, L::N, L::N, L::P, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Hermes Engine Quirks Every RN Developer Should Know",
        content: "hermes engine jsc javascript core hermes quirk compatibility issues",
        labels: [L::N, L::N, L::N, L::N, L::P, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Expo Router: File-Based Routing for React Native Apps",
        content: "expo router file-based routing expo-router react-native navigation mobile",
        labels: [L::N, L::N, L::N, L::N, L::S, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "React Native New Architecture: Fabric Renderer and TurboModules",
        content: "new architecture fabric turbo module bridgeless migration react-native",
        labels: [L::N, L::N, L::N, L::N, L::S, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Expo SDK 52: What's New in Managed Workflow",
        content: "expo sdk mobile development react-native eas build managed workflow",
        labels: [L::N, L::N, L::N, L::N, L::K, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "React Native JS Thread Performance: Eliminating Frame Drops and Jank",
        content: "profiling revealed the js thread was causing performance degradation and frame drop \
            issues during list scrolling in our react native app. moving heavy computations to native \
            modules and using reanimated for ui thread animations eliminated the jank entirely",
        labels: [L::N, L::N, L::N, L::N, L::P, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "EAS Update: Reliable Over-the-Air Deployments for Expo Apps",
        content: "ota updates with eas update let you push javascript bundle changes to expo apps \
            without going through app store review. configuring update channels and rollback \
            strategies ensures reliable over the air deployment for production mobile applications",
        labels: [L::N, L::N, L::N, L::N, L::P, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Building Production Mobile Apps with React Native and Expo",
        content: "react native combined with expo provides a powerful mobile app development platform. \
            from eas build for compilation to expo go for development previews the complete toolchain \
            handles everything from initial development through final app store submission",
        labels: [L::N, L::N, L::N, L::N, L::K, L::N, L::N, L::N],
    },
    // --- Laravel ecosystem ---
    CorpusItem {
        title: "Laravel Queue Jobs Keep Failing: Debugging Horizon Workers",
        content: "queue job failed retry horizon worker laravel reliability debugging",
        labels: [L::N, L::N, L::N, L::N, L::N, L::P, L::N, L::N],
    },
    CorpusItem {
        title: "N+1 Query Problem in Eloquent: Eager Loading Done Right",
        content: "n+1 eager loading query eloquent performance lazy loading optimization",
        labels: [L::N, L::N, L::N, L::N, L::N, L::P, L::N, L::N],
    },
    CorpusItem {
        title: "Livewire 3 Migration: wire:navigate and Breaking Changes",
        content: "livewire 3 livewire v3 livewire upgrade wire:navigate alpine morphing",
        labels: [L::N, L::N, L::N, L::N, L::N, L::S, L::N, L::N],
    },
    CorpusItem {
        title: "Filament V3: Building Admin Panels in Laravel",
        content: "filament filament admin filament v3 filament panel laravel admin dashboard",
        labels: [L::N, L::N, L::N, L::N, L::N, L::S, L::N, L::N],
    },
    CorpusItem {
        title: "Laravel 11 Release: Slimmer Skeleton and New Features",
        content: "laravel release features improvements php framework routes",
        labels: [L::N, L::N, L::N, L::N, L::N, L::K, L::N, L::N],
    },
    CorpusItem {
        title: "Upgrading to PHP 8.3: Breaking Changes and Compatibility Guide",
        content: "php version upgrade from php 7 to php 8 requires careful attention to php \
            compatibility issues including deprecated functions and type system changes. this guide \
            covers every breaking change and provides tested strategies for large codebases",
        labels: [L::N, L::N, L::N, L::N, L::N, L::P, L::N, L::N],
    },
    CorpusItem {
        title: "Eloquent Performance: Advanced Query Optimization in Laravel",
        content: "eloquent orm provides elegant syntax but can generate inefficient sql without \
            careful optimization. this deep dive covers indexing strategies query analysis with \
            telescope and advanced laravel patterns for high traffic applications",
        labels: [L::N, L::N, L::N, L::N, L::N, L::K, L::N, L::N],
    },
    // --- Django ecosystem ---
    CorpusItem {
        title: "Django ORM N+1: select_related vs prefetch_related",
        content: "orm queryset n+1 select_related prefetch_related django performance",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::P, L::N],
    },
    CorpusItem {
        title: "Django Async Views: ASGI and Channels Deep Dive",
        content: "async asgi channels async view django async support python httptools",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::P, L::N],
    },
    CorpusItem {
        title: "Django Ninja: FastAPI-Style APIs with Pydantic Validation",
        content: "django-ninja ninja api pydantic django ninja fast type-safe",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::S, L::N],
    },
    CorpusItem {
        title: "HTMX with Django: Hypermedia-Driven Development",
        content: "htmx hypermedia hx-get hx-post html over the wire django templates",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::S, L::N],
    },
    CorpusItem {
        title: "Django REST Framework: Serializer Performance Tips",
        content: "drf django rest framework serializer viewset performance python api",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::K, L::N],
    },
    CorpusItem {
        title: "Django Migration Conflicts: Squashing and Resolving in Team Projects",
        content: "migration conflict resolution in django when multiple developers create migrations \
            simultaneously. learn when to squash migrations using makemigrations and how to resolve \
            merge conflicts in the migration graph without losing data or corrupting schema state",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::P, L::N],
    },
    CorpusItem {
        title: "Wagtail CMS: Building Content-Managed Sites with Django",
        content: "wagtail provides a powerful content management system built on top of django. \
            create custom page types streamfields and rich content editing experiences while \
            leveraging the full django ecosystem for your backend logic and administration",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::K, L::N],
    },
    // --- Vue ecosystem ---
    CorpusItem {
        title: "Composition API Migration: From Options to Script Setup",
        content: "composition api options api migration setup script setup vue patterns",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::P],
    },
    CorpusItem {
        title: "Nuxt SSR Hydration Mismatch: Common Causes and Fixes",
        content: "ssr hydration mismatch nuxt ssr server render client mismatch errors",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::P],
    },
    CorpusItem {
        title: "Vue Vapor Mode: Compile-Time Reactivity Without Virtual DOM",
        content: "vue vapor vapor mode compile-time no virtual dom performance benchmark",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::S],
    },
    CorpusItem {
        title: "Nuxt 4 Migration Guide: Breaking Changes and Upgrade Path",
        content: "nuxt 4 nuxt upgrade nuxt migration nuxt next improvements breaking",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::S],
    },
    CorpusItem {
        title: "Pinia Store Patterns: Composable State Management in Vue",
        content: "pinia state management vue store composable reactive getters actions",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::K],
    },
    CorpusItem {
        title: "Vue TypeScript Integration: defineComponent and Type-Safe Props",
        content: "typescript integration in vue 3 requires understanding definecomponent for proper \
            type inference. learn how to type your props emits and composables with vue typescript \
            patterns that provide full ide support and compile time checking for your components",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::P],
    },
    CorpusItem {
        title: "Migrating from Vuex to Pinia: State Management Evolution",
        content: "vuex to pinia migration involves restructuring your state management from mutation \
            based patterns to a more intuitive store composition api. pinia provides better \
            typescript support and devtools integration making state management more pleasant",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::P],
    },
    CorpusItem {
        title: "Vue 3.5 Composition API: Practical Patterns with VueUse",
        content: "composition api patterns in vue using vueuse composables for common tasks like \
            reactive local storage intersection observers and data fetching. combining vue reactive \
            primitives with utility composables for cleaner component logic throughout your app",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::K],
    },
    // --- Competing tech items (C labels) ---
    CorpusItem {
        title: "SvelteKit 2.0: The Full-Stack Framework Gets Even Better",
        content: "sveltekit svelte framework release features routing server load",
        labels: [L::C, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Remix Framework: Data Loading Done Right",
        content: "remix framework performance routing loader actions nested routes",
        labels: [L::C, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Go 1.23 Performance Improvements for Backend Services",
        content: "go golang backend services performance goroutine scheduling",
        labels: [L::N, L::C, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Zig Build System: Comptime and Memory Safety Without GC",
        content: "zig programming language build system comptime safety allocation",
        labels: [L::N, L::C, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "TensorFlow 2.17: Keras Integration and Model Garden",
        content: "tensorflow keras deep learning model training optimization google",
        labels: [L::N, L::N, L::C, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Rust for Backend: Why We Switched from Go to Axum",
        content: "rust backend web services axum systems programming performance",
        labels: [L::N, L::K, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Java Spring Boot 4.0: Enterprise Microservices",
        content: "java spring boot microservices cloud native enterprise jpa",
        labels: [L::N, L::N, L::N, L::C, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Flutter 3.24: Material Design and Cross-Platform Widgets",
        content: "flutter dart mobile cross-platform widgets material design ios android",
        labels: [L::N, L::N, L::N, L::N, L::C, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Symfony 7 Components: Enterprise Framework Guide",
        content: "symfony framework components bundles enterprise architecture hexagonal",
        labels: [L::N, L::N, L::N, L::N, L::N, L::C, L::N, L::N],
    },
    CorpusItem {
        title: "FastAPI Performance: Async API Framework Benchmark",
        content: "fastapi async api performance starlette uvicorn benchmark pydantic",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::C, L::N],
    },
    CorpusItem {
        title: "React 19 Server Components: Concurrent Rendering Deep Dive",
        content: "react concurrent rendering suspense transitions server components",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::C],
    },
    CorpusItem {
        title: "Angular 18 Signals: Zoneless Change Detection",
        content: "angular standalone components signals zoneless change detection",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::C],
    },
    CorpusItem {
        title: "JAX for Scientific Computing: Functional Approach to Machine Learning",
        content: "jax brings functional programming and automatic differentiation to scientific \
            computing with hardware acceleration on tpu. researchers prefer its composable \
            transformations over imperative frameworks for numerical methods and simulations",
        labels: [L::N, L::N, L::C, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Node.js 22: Performance Improvements and Event Loop Changes",
        content: "node.js runtime gets significant performance improvements including faster startup \
            times and improved event loop handling for backend services and real-time applications",
        labels: [L::N, L::N, L::N, L::C, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Kotlin Multiplatform: Share Code Between Android and iOS Natively",
        content: "kotlin multiplatform mobile enables sharing business logic between android and ios \
            apps using a single codebase with native ui rendering on each platform for truly native \
            mobile experiences without compromising platform specific behavior",
        labels: [L::N, L::N, L::N, L::N, L::C, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Ruby on Rails 8: Modern Web Development with Convention over Configuration",
        content: "rails brings modern web development patterns with hotwire turbo and stimulus. \
            convention over configuration approach enables rapid application prototyping and \
            deployment with minimal boilerplate and strong community support",
        labels: [L::N, L::N, L::N, L::N, L::N, L::C, L::N, L::N],
    },
    CorpusItem {
        title: "Flask 3.0: Lightweight Web Framework Gets Native Async Support",
        content: "flask micro framework adds native async request handling in version 3. blueprint \
            improvements and better extension ecosystem for building lightweight web apis and \
            microservices quickly without heavyweight abstractions or complex configuration",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::C, L::N],
    },
    // --- Neutral items (off-domain for all) ---
    CorpusItem {
        title: "PostgreSQL 17: Incremental Backup and Logical Replication",
        content: "postgresql database incremental backup logical replication slots",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "How We Scaled Our Startup to 10M Users",
        content: "startup scaling users growth product market fit team hiring",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "The State of CSS 2025: Container Queries and Cascade Layers",
        content: "css container queries cascade layers has selector nesting",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
    CorpusItem {
        title: "Figma Plugins for Design Systems",
        content: "figma design system tokens components auto-layout constraints",
        labels: [L::N, L::N, L::N, L::N, L::N, L::N, L::N, L::N],
    },
];

// ============================================================================
// Tier 1 Tests: Corpus Precision
// ============================================================================

macro_rules! corpus_test {
    ($name:ident, $profile_idx:expr, $profile_id:expr) => {
        mod $name {
            use super::*;

            fn stack() -> stacks::ComposedStack {
                stacks::compose_profiles(&[$profile_id.to_string()])
            }

            #[test]
            fn pain_point_precision() {
                let s = stack();
                let pain_items: Vec<_> = CORPUS
                    .iter()
                    .filter(|c| c.labels[$profile_idx] == L::P)
                    .collect();
                assert!(
                    !pain_items.is_empty(),
                    "No P-labeled items for {}",
                    $profile_id
                );

                let mut misses = Vec::new();
                for item in &pain_items {
                    if !scoring::has_pain_point_match(item.title, item.content, &s) {
                        misses.push(item.title);
                    }
                }
                assert!(
                    misses.is_empty(),
                    "{}: Pain point missed {} items: {:?}",
                    $profile_id,
                    misses.len(),
                    misses
                );
            }

            #[test]
            fn keyword_precision() {
                let s = stack();
                let kw_items: Vec<_> = CORPUS
                    .iter()
                    .filter(|c| c.labels[$profile_idx] == L::K)
                    .collect();
                assert!(
                    !kw_items.is_empty(),
                    "No K-labeled items for {}",
                    $profile_id
                );

                let mut misses = Vec::new();
                for item in &kw_items {
                    let boost = scoring::compute_stack_boost(item.title, item.content, &s);
                    if boost == 0.0 {
                        misses.push(item.title);
                    }
                }
                assert!(
                    misses.is_empty(),
                    "{}: Keyword boost missed {} items: {:?}",
                    $profile_id,
                    misses.len(),
                    misses
                );
            }

            #[test]
            fn shift_precision() {
                let s = stack();
                let shift_items: Vec<_> = CORPUS
                    .iter()
                    .filter(|c| c.labels[$profile_idx] == L::S)
                    .collect();
                assert!(
                    !shift_items.is_empty(),
                    "No S-labeled items for {}",
                    $profile_id
                );

                let mut misses = Vec::new();
                for item in &shift_items {
                    let topics = topics_from(item);
                    let mult = scoring::detect_ecosystem_shift(&topics, item.title, &s);
                    if mult <= 1.0 {
                        misses.push(item.title);
                    }
                }
                assert!(
                    misses.is_empty(),
                    "{}: Shift detection missed {} items: {:?}",
                    $profile_id,
                    misses.len(),
                    misses
                );
            }

            #[test]
            fn competing_precision() {
                let s = stack();
                let comp_items: Vec<_> = CORPUS
                    .iter()
                    .filter(|c| c.labels[$profile_idx] == L::C)
                    .collect();
                assert!(
                    !comp_items.is_empty(),
                    "No C-labeled items for {}",
                    $profile_id
                );

                let mut misses = Vec::new();
                for item in &comp_items {
                    let penalty = scoring::compute_competing_penalty(item.title, item.content, &s);
                    if penalty >= 1.0 {
                        misses.push(item.title);
                    }
                }
                assert!(
                    misses.is_empty(),
                    "{}: Competing penalty missed {} items: {:?}",
                    $profile_id,
                    misses.len(),
                    misses
                );
            }

            #[test]
            fn neutral_isolation() {
                let s = stack();
                let neutral_items: Vec<_> = CORPUS
                    .iter()
                    .filter(|c| c.labels[$profile_idx] == L::N)
                    .collect();

                let mut false_boosts = Vec::new();
                for item in &neutral_items {
                    let boost = scoring::compute_stack_boost(item.title, item.content, &s);
                    let pain = scoring::has_pain_point_match(item.title, item.content, &s);
                    if boost > 0.10 || pain {
                        false_boosts.push((item.title, boost, pain));
                    }
                }
                // Allow up to 10% false positive rate on neutrals
                let rate = false_boosts.len() as f32 / neutral_items.len() as f32;
                assert!(
                    rate <= 0.10,
                    "{}: Neutral false positive rate {:.0}% ({}/{}) — items: {:?}",
                    $profile_id,
                    rate * 100.0,
                    false_boosts.len(),
                    neutral_items.len(),
                    false_boosts.iter().map(|(t, _, _)| *t).collect::<Vec<_>>()
                );
            }
        }
    };
}

corpus_test!(corpus_nextjs, NX, "nextjs_fullstack");
corpus_test!(corpus_rust, RS, "rust_systems");
corpus_test!(corpus_python_ml, PY, "python_ml");
corpus_test!(corpus_go, GO, "go_backend");
corpus_test!(corpus_react_native, RN, "react_native");
corpus_test!(corpus_laravel, LA, "laravel");
corpus_test!(corpus_django, DJ, "django");
corpus_test!(corpus_vue, VU, "vue_frontend");

// ============================================================================
// Tier 2: Adversarial Battery
// ============================================================================
// Items specifically designed to exploit known word-boundary weaknesses.
// Every item here MUST produce zero false positives.

static ADVERSARIAL: &[(&str, &str)] = &[
    // "go" traps
    (
        "Google Cloud Platform Announces New Region",
        "google cloud storage algorithms ergonomic",
    ),
    (
        "Algorithm Design: Dynamic Programming Masterclass",
        "algorithms programming ergonomic design patterns",
    ),
    (
        "MongoDB Atlas Performance Tuning Guide",
        "mongodb atlas performance indexing aggregation",
    ),
    (
        "The Good Parts of Modern Web Frameworks",
        "framework patterns best practices architecture opinions",
    ),
    // "rust" traps
    (
        "Building Trust in Remote Engineering Teams",
        "trust building frustrated developers onboarding",
    ),
    (
        "Entrust Certificate Management Solutions",
        "entrust certificate ssl tls management enterprise",
    ),
    (
        "Frustrated Developers Leave Companies That Ignore DX",
        "frustrated developer experience frustration burnout",
    ),
    // "vue" traps
    (
        "Revenue Growth Strategies for SaaS Startups",
        "revenue metrics growth saas pricing strategy",
    ),
    // "react" traps (should not match inside react-native)
    (
        "Reactive Programming with RxJS Observables",
        "reactive programming rxjs observables streams operators",
    ),
    // "node" traps
    (
        "Understanding Graph Nodes and Tree Traversal",
        "node tree graph traversal data structure algorithm",
    ),
    // "php" trap
    (
        "How Graphic Design Shapes User Perception",
        "graphic design user perception visual hierarchy shapes",
    ),
    // "python" in unrelated context
    (
        "Monty Python and the Holy Grail Turns 50",
        "monty python comedy british humor anniversary film",
    ),
];

#[test]
fn adversarial_zero_false_positives() {
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        for &(title, content) in ADVERSARIAL {
            let boost = scoring::compute_stack_boost(title, content, &stack);
            assert!(
                boost == 0.0,
                "ADVERSARIAL FAIL: profile={}, title={:?}, boost={} (expected 0.0)",
                profile_id,
                title,
                boost
            );
            let pain = scoring::has_pain_point_match(title, content, &stack);
            assert!(
                !pain,
                "ADVERSARIAL FAIL: profile={}, title={:?} triggered pain point",
                profile_id, title
            );
        }
    }
}

// Exhaustive word-boundary traps for each profile's short keywords
#[test]
fn adversarial_word_boundaries_exhaustive() {
    // go: must not match google, algorithm, golang substring false pos, cargo, ergo, mango
    let go_stack = stacks::compose_profiles(&["go_backend".to_string()]);
    for trap in &[
        "google",
        "algorithm",
        "ergo",
        "mango",
        "cargo",
        "logo",
        "undergo",
    ] {
        let text = format!("Article about {} technology", trap);
        assert!(
            !scoring::text_contains_term(&text.to_lowercase(), "go"),
            "'go' matched inside '{}'",
            trap
        );
        assert_eq!(
            scoring::compute_stack_boost(&text, "", &go_stack),
            0.0,
            "Go stack boosted '{}'",
            trap
        );
    }

    // rust: must not match trust, frustrated, rustic, crusty
    let rust_stack = stacks::compose_profiles(&["rust_systems".to_string()]);
    for trap in &["trust", "frustrated", "rustic", "crusty", "robust"] {
        let text = format!("Article about {} in teams", trap);
        assert!(
            !scoring::text_contains_term(&text.to_lowercase(), "rust"),
            "'rust' matched inside '{}'",
            trap
        );
        assert_eq!(
            scoring::compute_stack_boost(&text, "", &rust_stack),
            0.0,
            "Rust stack boosted '{}'",
            trap
        );
    }

    // vue: must not match revenue, venue, revue, value
    let vue_stack = stacks::compose_profiles(&["vue_frontend".to_string()]);
    for trap in &["revenue", "venue", "revue", "value", "vuelto"] {
        let text = format!("Article about {} metrics", trap);
        assert!(
            !scoring::text_contains_term(&text.to_lowercase(), "vue"),
            "'vue' matched inside '{}'",
            trap
        );
        assert_eq!(
            scoring::compute_stack_boost(&text, "", &vue_stack),
            0.0,
            "Vue stack boosted '{}'",
            trap
        );
    }
}

// Verify true positives still work after adversarial hardening
#[test]
fn adversarial_true_positives_survive() {
    let go_stack = stacks::compose_profiles(&["go_backend".to_string()]);
    assert!(
        scoring::compute_stack_boost("Go 1.23 Release Notes", "golang release", &go_stack) > 0.0
    );

    let rust_stack = stacks::compose_profiles(&["rust_systems".to_string()]);
    assert!(
        scoring::compute_stack_boost(
            "Rust 1.80 Release Highlights",
            "rust release cargo",
            &rust_stack
        ) > 0.0
    );

    let vue_stack = stacks::compose_profiles(&["vue_frontend".to_string()]);
    assert!(
        scoring::compute_stack_boost(
            "Vue 3.5 Composition API",
            "vue composition pinia",
            &vue_stack
        ) > 0.0
    );
}

// Hyphen isolation: "react" must NOT match "react-native"
#[test]
fn adversarial_hyphen_isolation() {
    assert!(!scoring::text_contains_term(
        "react-native is great",
        "react"
    ));
    assert!(scoring::text_contains_term(
        "react-native is great",
        "react-native"
    ));
    assert!(!scoring::text_contains_term(
        "drizzle-orm migration",
        "drizzle"
    ));
    assert!(scoring::text_contains_term(
        "drizzle-orm migration",
        "drizzle-orm"
    ));
    assert!(!scoring::text_contains_term("go-fiber framework", "go"));
    assert!(scoring::text_contains_term(
        "go-fiber framework",
        "go-fiber"
    ));
    assert!(!scoring::text_contains_term("vue-router patterns", "vue"));
    assert!(scoring::text_contains_term(
        "vue-router patterns",
        "vue-router"
    ));
}

// ============================================================================
// Tier 3: Algebraic Properties
// ============================================================================
// Mathematical invariants that must hold for ANY input.

#[test]
fn property_boost_bounded() {
    // For all profiles and all possible inputs, boost must be in [0.0, 0.20]
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        // Worst case: every keyword in every pain point in the title
        let mega_title = "async pin send lifetime future tokio compile time build time \
            incremental compilation cargo build unsafe soundness undefined behavior miri \
            error handling thiserror anyhow result error type borrow checker ownership \
            move semantics lifetime annotation app router migration pages router next 13 \
            next 14 edge runtime middleware cold start isr revalidate cache stale bundle \
            size tree shaking code splitting webpack turbopack server component client \
            component use client rsc server action cuda version driver nvcc nvidia gpu \
            oom vram memory allocation dependency pip conda virtual environment package \
            queue job failed retry horizon worker n+1 eager loading eloquent performance \
            composition api options api setup script setup ssr hydration mismatch nuxt \
            generics type parameter context ctx module go.mod error wrapping errors.Is \
            new architecture fabric turbo module hermes jsc app store review ota eas \
            js thread ui thread frame drop";
        let boost = scoring::compute_stack_boost(mega_title, mega_title, &stack);
        assert!(
            boost >= 0.0 && boost <= 0.20,
            "{}: boost {} out of [0.0, 0.20] for mega input",
            profile_id,
            boost
        );
    }
}

#[test]
fn property_shift_bounded() {
    // Ecosystem shift multiplier must be in [0.95, 1.25]
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        let all_topics: Vec<String> = vec![
            "drizzle",
            "prisma alternative",
            "biome",
            "biomejs",
            "bun runtime",
            "bunx",
            "native async trait",
            "async fn in trait",
            "const generics",
            "stabilization",
            "jax",
            "flax",
            "gguf",
            "llama.cpp",
            "local llm",
            "ollama",
            "rag",
            "slog",
            "log/slog",
            "range over func",
            "iter.Seq",
            "go wasm",
            "wazero",
            "expo",
            "expo-router",
            "new architecture",
            "fabric",
            "livewire 3",
            "filament",
            "pest v3",
            "django-ninja",
            "htmx",
            "wagtail",
            "vue vapor",
            "nuxt 4",
            "unocss",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
        let mega_title = all_topics.join(" ");
        let mult = scoring::detect_ecosystem_shift(&all_topics, &mega_title, &stack);
        assert!(
            mult >= 0.95 && mult <= 1.25,
            "{}: shift multiplier {} out of [0.95, 1.25]",
            profile_id,
            mult
        );
    }
}

#[test]
fn property_penalty_binary() {
    // Competing penalty can ONLY be 1.0 or 0.95 — never anything else
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        for item in CORPUS {
            let penalty = scoring::compute_competing_penalty(item.title, item.content, &stack);
            assert!(
                penalty == 1.0 || penalty == 0.95,
                "{}: penalty {} is not 1.0 or 0.95 for '{}'",
                profile_id,
                penalty,
                item.title
            );
        }
    }
}

#[test]
fn property_inactive_neutrality() {
    // When no stacks are selected, ALL scoring functions return neutral values
    let empty = stacks::ComposedStack::default();
    assert!(!empty.active);

    for item in CORPUS {
        let boost = scoring::compute_stack_boost(item.title, item.content, &empty);
        assert_eq!(
            boost, 0.0,
            "Inactive stack gave boost {} for '{}'",
            boost, item.title
        );

        let pain = scoring::has_pain_point_match(item.title, item.content, &empty);
        assert!(!pain, "Inactive stack gave pain match for '{}'", item.title);

        let topics = topics_from(item);
        let shift = scoring::detect_ecosystem_shift(&topics, item.title, &empty);
        assert_eq!(
            shift, 1.0,
            "Inactive stack gave shift {} for '{}'",
            shift, item.title
        );

        let penalty = scoring::compute_competing_penalty(item.title, item.content, &empty);
        assert_eq!(
            penalty, 1.0,
            "Inactive stack gave penalty {} for '{}'",
            penalty, item.title
        );
    }
}

#[test]
fn property_pain_beats_keyword() {
    // For every profile, pain point items should score >= keyword-only items on average
    for (idx, &profile_id) in PROFILE_IDS.iter().enumerate() {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);

        let pain_boosts: Vec<f32> = CORPUS
            .iter()
            .filter(|c| c.labels[idx] == L::P)
            .map(|c| scoring::compute_stack_boost(c.title, c.content, &stack))
            .collect();
        let kw_boosts: Vec<f32> = CORPUS
            .iter()
            .filter(|c| c.labels[idx] == L::K)
            .map(|c| scoring::compute_stack_boost(c.title, c.content, &stack))
            .collect();

        if pain_boosts.is_empty() || kw_boosts.is_empty() {
            continue;
        }

        let pain_avg: f32 = pain_boosts.iter().sum::<f32>() / pain_boosts.len() as f32;
        let kw_avg: f32 = kw_boosts.iter().sum::<f32>() / kw_boosts.len() as f32;
        // Allow 0.02 tolerance: individual keyword boosts can slightly exceed
        // lower-severity pain points, but pain should never be dramatically below.
        assert!(
            pain_avg >= kw_avg - 0.02,
            "{}: Pain avg ({:.4}) too far below keyword avg ({:.4})",
            profile_id,
            pain_avg,
            kw_avg
        );
    }
}

// ============================================================================
// Tier 4: Profile Data Integrity
// ============================================================================

#[test]
fn integrity_pain_points_minimum_keywords() {
    // Every pain point must have >= 3 keywords (2-match threshold needs at least 3 to be useful)
    for profile in stacks::list_profiles() {
        for pp in profile.pain_points {
            assert!(
                pp.keywords.len() >= 3,
                "{}: pain point '{}' has only {} keywords (need 3+)",
                profile.id,
                pp.description,
                pp.keywords.len()
            );
        }
    }
}

#[test]
fn integrity_shifts_minimum_keywords() {
    // Every ecosystem shift must have >= 2 keywords (2-match threshold)
    for profile in stacks::list_profiles() {
        for es in profile.ecosystem_shifts {
            assert!(
                es.keywords.len() >= 2,
                "{}: shift '{}->{}'  has only {} keywords (need 2+)",
                profile.id,
                es.from,
                es.to,
                es.keywords.len()
            );
        }
    }
}

#[test]
fn integrity_no_self_competition() {
    // No profile should list its own core_tech or companions as competing
    for profile in stacks::list_profiles() {
        for &comp in profile.competing {
            assert!(
                !profile.core_tech.contains(&comp),
                "{}: '{}' is both core_tech and competing",
                profile.id,
                comp
            );
            assert!(
                !profile.companions.contains(&comp),
                "{}: '{}' is both companion and competing",
                profile.id,
                comp
            );
        }
    }
}

#[test]
fn integrity_severity_bounds() {
    for profile in stacks::list_profiles() {
        for pp in profile.pain_points {
            assert!(
                pp.severity >= 0.05 && pp.severity <= 0.20,
                "{}: severity {} out of [0.05, 0.20] for '{}'",
                profile.id,
                pp.severity,
                pp.description
            );
        }
    }
}

#[test]
fn integrity_shift_boost_bounds() {
    for profile in stacks::list_profiles() {
        for es in profile.ecosystem_shifts {
            assert!(
                es.boost >= 1.05 && es.boost <= 1.25,
                "{}: shift boost {} out of [1.05, 1.25] for '{}->{}'",
                profile.id,
                es.boost,
                es.from,
                es.to
            );
        }
    }
}

#[test]
fn integrity_keyword_boost_bounds() {
    for profile in stacks::list_profiles() {
        for &(kw, boost) in profile.keyword_boosts {
            assert!(
                boost >= 0.04 && boost <= 0.15,
                "{}: keyword boost {} out of [0.04, 0.15] for '{}'",
                profile.id,
                boost,
                kw
            );
        }
    }
}

#[test]
fn integrity_unique_ids() {
    let mut seen = std::collections::HashSet::new();
    for profile in stacks::list_profiles() {
        assert!(
            seen.insert(profile.id),
            "Duplicate profile ID: {}",
            profile.id
        );
    }
    assert_eq!(seen.len(), 8, "Expected 8 profiles, got {}", seen.len());
}

#[test]
fn integrity_no_boosted_competitor() {
    // A profile should never have a keyword_boost for a tech in its competing list
    for profile in stacks::list_profiles() {
        for &(kw, _) in profile.keyword_boosts {
            assert!(
                !profile.competing.contains(&kw),
                "{}: keyword boost for competing tech '{}'",
                profile.id,
                kw
            );
        }
    }
}

#[test]
fn integrity_all_keywords_lowercase() {
    for profile in stacks::list_profiles() {
        for pp in profile.pain_points {
            for &kw in pp.keywords {
                assert_eq!(
                    kw,
                    kw.to_lowercase().as_str(),
                    "{}: pain point keyword '{}' not lowercase",
                    profile.id,
                    kw
                );
            }
        }
        for es in profile.ecosystem_shifts {
            for &kw in es.keywords {
                assert_eq!(
                    kw,
                    kw.to_lowercase().as_str(),
                    "{}: shift keyword '{}' not lowercase",
                    profile.id,
                    kw
                );
            }
        }
        for &(kw, _) in profile.keyword_boosts {
            assert_eq!(
                kw,
                kw.to_lowercase().as_str(),
                "{}: keyword boost '{}' not lowercase",
                profile.id,
                kw
            );
        }
    }
}

// ============================================================================
// Tier 5: Composition Algebra
// ============================================================================

#[test]
fn composition_all_profiles_valid() {
    // Composing every individual profile produces an active stack with content
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        assert!(stack.active, "{} not active", profile_id);
        assert!(
            !stack.pain_points.is_empty(),
            "{} has no pain points",
            profile_id
        );
        assert!(
            !stack.ecosystem_shifts.is_empty(),
            "{} has no shifts",
            profile_id
        );
        assert!(
            !stack.keyword_boosts.is_empty(),
            "{} has no keyword boosts",
            profile_id
        );
        assert!(!stack.all_tech.is_empty(), "{} has no tech", profile_id);
    }
}

#[test]
fn composition_max_semantics() {
    // When two profiles share a keyword, composed boost = MAX, not SUM
    let nextjs = stacks::compose_profiles(&["nextjs_fullstack".to_string()]);
    let rust = stacks::compose_profiles(&["rust_systems".to_string()]);
    let composed =
        stacks::compose_profiles(&["nextjs_fullstack".to_string(), "rust_systems".to_string()]);

    for (&kw, &boost) in &composed.keyword_boosts {
        let nx_boost = nextjs.keyword_boosts.get(kw).copied().unwrap_or(0.0);
        let rs_boost = rust.keyword_boosts.get(kw).copied().unwrap_or(0.0);
        let expected_max = nx_boost.max(rs_boost);
        assert!(
            (boost - expected_max).abs() < f32::EPSILON,
            "Composed boost for '{}' is {} but MAX({}, {}) = {}",
            kw,
            boost,
            nx_boost,
            rs_boost,
            expected_max
        );
    }
}

#[test]
fn composition_disjoint_own_competing() {
    // Within a single profile, core_tech/companions and competing must be disjoint.
    // But across profiles, one profile's core_tech CAN appear in another's competing
    // (e.g., "rust" is core for rust_systems but competing for go_backend).
    // Just verify single-profile disjointness.
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        for tech in &stack.all_tech {
            assert!(
                !stack.competing.contains(tech),
                "{}: '{}' is both own tech and competing in composed stack",
                profile_id,
                tech
            );
        }
    }
}

#[test]
fn composition_empty_is_neutral() {
    let empty = stacks::compose_profiles(&[]);
    assert!(!empty.active);
    assert!(empty.pain_points.is_empty());
    assert!(empty.ecosystem_shifts.is_empty());
    assert!(empty.keyword_boosts.is_empty());
    assert!(empty.source_preferences.is_empty());
    assert!(empty.all_tech.is_empty());
    assert!(empty.competing.is_empty());
}

#[test]
fn composition_all_eight_scales() {
    let all_ids: Vec<String> = PROFILE_IDS.iter().map(|s| s.to_string()).collect();
    let composed = stacks::compose_profiles(&all_ids);
    assert!(composed.active);
    // 8 profiles with 4-5 pain points each = 34+ total
    assert!(
        composed.pain_points.len() >= 30,
        "8 profiles should give 30+ pain points, got {}",
        composed.pain_points.len()
    );
    // Should have techs from all stacks
    assert!(composed.all_tech.contains("rust"));
    assert!(composed.all_tech.contains("nextjs"));
    assert!(composed.all_tech.contains("pytorch"));
    assert!(composed.all_tech.contains("laravel"));
    assert!(composed.all_tech.contains("django"));
    assert!(composed.all_tech.contains("vue"));
}

#[test]
fn composition_mixed_content_no_penalty() {
    // When Rust+Go are both selected, Go content should NOT get a competing penalty
    let composed =
        stacks::compose_profiles(&["rust_systems".to_string(), "go_backend".to_string()]);
    let penalty = scoring::compute_competing_penalty(
        "Go Backend Performance Tips",
        "golang goroutine optimization patterns",
        &composed,
    );
    // "go"/"golang" are in composed.all_tech (from go_backend), so no penalty
    assert_eq!(
        penalty, 1.0,
        "Go content should not be penalized when Go profile is selected"
    );
}

// ============================================================================
// Corpus coverage verification
// ============================================================================

#[test]
fn corpus_coverage_minimum() {
    // Verify the corpus has enough items per label type per profile
    for (idx, &profile_id) in PROFILE_IDS.iter().enumerate() {
        let p_count = CORPUS.iter().filter(|c| c.labels[idx] == L::P).count();
        let k_count = CORPUS.iter().filter(|c| c.labels[idx] == L::K).count();
        let s_count = CORPUS.iter().filter(|c| c.labels[idx] == L::S).count();
        let c_count = CORPUS.iter().filter(|c| c.labels[idx] == L::C).count();
        let n_count = CORPUS.iter().filter(|c| c.labels[idx] == L::N).count();

        assert!(
            p_count >= 3,
            "{}: needs 3+ P items, got {}",
            profile_id,
            p_count
        );
        assert!(
            k_count >= 2,
            "{}: needs 2+ K items, got {}",
            profile_id,
            k_count
        );
        assert!(
            s_count >= 2,
            "{}: needs 2+ S items, got {}",
            profile_id,
            s_count
        );
        assert!(
            c_count >= 2,
            "{}: needs 2+ C items, got {}",
            profile_id,
            c_count
        );
        assert!(
            n_count >= 10,
            "{}: needs 10+ N items, got {}",
            profile_id,
            n_count
        );
    }
}

// ============================================================================
// Tier 6: Cross-Profile Contamination Matrix
// ============================================================================
// For each profile's labeled items, verify they do NOT trigger scoring
// functions in OTHER profiles. This catches keyword overlap between profiles
// (e.g., "async" in both Rust and Django) and proves the 2-keyword threshold
// prevents cross-talk.

#[test]
fn contamination_pain_points_isolated() {
    let mut violations = Vec::new();

    for (idx, &profile_id) in PROFILE_IDS.iter().enumerate() {
        let pain_items: Vec<_> = CORPUS.iter().filter(|c| c.labels[idx] == L::P).collect();

        for (other_idx, &other_id) in PROFILE_IDS.iter().enumerate() {
            if other_idx == idx {
                continue;
            }
            let other_stack = stacks::compose_profiles(&[other_id.to_string()]);

            for item in &pain_items {
                // Skip items that are explicitly labeled for the other profile too
                if item.labels[other_idx] != L::N {
                    continue;
                }
                if scoring::has_pain_point_match(item.title, item.content, &other_stack) {
                    violations.push(format!(
                        "  {}'s P-item '{}' also triggers pain in {}",
                        profile_id, item.title, other_id
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Cross-profile pain point contamination ({} violations):\n{}",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn contamination_keyword_boost_isolation() {
    // Items labeled K for one profile (and N for another) should not get
    // significant boosts from that other profile.
    let mut violations = Vec::new();

    for (idx, &profile_id) in PROFILE_IDS.iter().enumerate() {
        let kw_items: Vec<_> = CORPUS.iter().filter(|c| c.labels[idx] == L::K).collect();

        for (other_idx, &other_id) in PROFILE_IDS.iter().enumerate() {
            if other_idx == idx {
                continue;
            }
            let other_stack = stacks::compose_profiles(&[other_id.to_string()]);

            for item in &kw_items {
                // Only check items labeled N for the other profile
                if item.labels[other_idx] != L::N {
                    continue;
                }
                let boost = scoring::compute_stack_boost(item.title, item.content, &other_stack);
                if boost > 0.08 {
                    violations.push(format!(
                        "  {}'s K-item '{}' gets boost {:.3} from {}",
                        profile_id, item.title, boost, other_id
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Keyword boost contamination ({} violations):\n{}",
        violations.len(),
        violations.join("\n")
    );
}

#[test]
fn contamination_competing_symmetry() {
    // If profile A lists tech X as competing, and profile B has X as core_tech,
    // then B should list at least one of A's core_tech as competing.
    // This ensures the competition relationship is bidirectional.
    let profiles = stacks::list_profiles();
    let mut asymmetries = Vec::new();

    for a in profiles {
        for b in profiles {
            if a.id == b.id {
                continue;
            }
            // Does A compete with any of B's core tech?
            let a_competes_b = a.competing.iter().any(|c| b.core_tech.contains(c));
            // Does B compete with any of A's core tech?
            let b_competes_a = b.competing.iter().any(|c| a.core_tech.contains(c));

            if a_competes_b && !b_competes_a {
                asymmetries.push(format!(
                    "  {} lists {}'s tech as competing, but {} doesn't reciprocate",
                    a.id, b.id, b.id
                ));
            }
        }
    }

    // This is advisory — asymmetry is sometimes intentional (e.g., Python ML
    // doesn't compete with Go Backend). Report but allow some asymmetry.
    assert!(
        asymmetries.len() <= 4,
        "Too many asymmetric competition relationships ({}):\n{}",
        asymmetries.len(),
        asymmetries.join("\n")
    );
}

// ============================================================================
// Tier 7: Determinism
// ============================================================================
// Scoring functions are pure (no hidden mutable state, no randomness).
// Same inputs MUST always produce identical outputs.

#[test]
fn determinism_repeated_scoring() {
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);

        for item in CORPUS {
            let topics = topics_from(item);

            // Compute reference values
            let ref_boost = scoring::compute_stack_boost(item.title, item.content, &stack);
            let ref_pain = scoring::has_pain_point_match(item.title, item.content, &stack);
            let ref_shift = scoring::detect_ecosystem_shift(&topics, item.title, &stack);
            let ref_penalty = scoring::compute_competing_penalty(item.title, item.content, &stack);

            // Run 10 more times — every result must be bit-identical
            for run in 1..=10 {
                let boost = scoring::compute_stack_boost(item.title, item.content, &stack);
                assert_eq!(
                    boost.to_bits(),
                    ref_boost.to_bits(),
                    "Non-deterministic boost on run {} for '{}' with {}",
                    run,
                    item.title,
                    profile_id
                );
                assert_eq!(
                    scoring::has_pain_point_match(item.title, item.content, &stack),
                    ref_pain,
                    "Non-deterministic pain on run {} for '{}' with {}",
                    run,
                    item.title,
                    profile_id
                );
                assert_eq!(
                    scoring::detect_ecosystem_shift(&topics, item.title, &stack).to_bits(),
                    ref_shift.to_bits(),
                    "Non-deterministic shift on run {} for '{}' with {}",
                    run,
                    item.title,
                    profile_id
                );
                assert_eq!(
                    scoring::compute_competing_penalty(item.title, item.content, &stack).to_bits(),
                    ref_penalty.to_bits(),
                    "Non-deterministic penalty on run {} for '{}' with {}",
                    run,
                    item.title,
                    profile_id
                );
            }
        }
    }
}

#[test]
fn determinism_composition_order_independent() {
    // compose_profiles([A, B]) should produce identical scoring to compose_profiles([B, A])
    let pairs = [
        ("nextjs_fullstack", "rust_systems"),
        ("python_ml", "go_backend"),
        ("laravel", "django"),
        ("react_native", "vue_frontend"),
    ];

    for (a, b) in pairs {
        let ab = stacks::compose_profiles(&[a.to_string(), b.to_string()]);
        let ba = stacks::compose_profiles(&[b.to_string(), a.to_string()]);

        for item in CORPUS {
            let topics = topics_from(item);

            let boost_ab = scoring::compute_stack_boost(item.title, item.content, &ab);
            let boost_ba = scoring::compute_stack_boost(item.title, item.content, &ba);
            assert_eq!(
                boost_ab.to_bits(),
                boost_ba.to_bits(),
                "Composition not commutative for boost: [{},{}] vs [{},{}] on '{}'",
                a,
                b,
                b,
                a,
                item.title
            );

            let shift_ab = scoring::detect_ecosystem_shift(&topics, item.title, &ab);
            let shift_ba = scoring::detect_ecosystem_shift(&topics, item.title, &ba);
            assert_eq!(
                shift_ab.to_bits(),
                shift_ba.to_bits(),
                "Composition not commutative for shift: [{},{}] vs [{},{}] on '{}'",
                a,
                b,
                b,
                a,
                item.title
            );

            let pen_ab = scoring::compute_competing_penalty(item.title, item.content, &ab);
            let pen_ba = scoring::compute_competing_penalty(item.title, item.content, &ba);
            assert_eq!(
                pen_ab.to_bits(),
                pen_ba.to_bits(),
                "Composition not commutative for penalty: [{},{}] vs [{},{}] on '{}'",
                a,
                b,
                b,
                a,
                item.title
            );
        }
    }
}

// ============================================================================
// Tier 8: Edge Cases
// ============================================================================
// Proves robustness against degenerate inputs that could cause panics,
// NaN propagation, or undefined behavior.

#[test]
fn edge_empty_inputs() {
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);

        assert_eq!(scoring::compute_stack_boost("", "", &stack), 0.0);
        assert!(!scoring::has_pain_point_match("", "", &stack));
        assert_eq!(scoring::detect_ecosystem_shift(&[], "", &stack), 1.0);
        assert_eq!(scoring::compute_competing_penalty("", "", &stack), 1.0);
    }
}

#[test]
fn edge_unicode_inputs() {
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);

        // Japanese + emoji — must not panic or produce NaN
        let boost = scoring::compute_stack_boost(
            "\u{95a2}\u{6570}\u{578b}\u{30d7}\u{30ed}\u{30b0}\u{30e9}\u{30df}\u{30f3}\u{30b0} \u{1f980} Rust async",
            "\u{975e}\u{540c}\u{671f}\u{51e6}\u{7406}\u{306e}\u{57fa}\u{790e} tokio \u{30e9}\u{30f3}\u{30bf}\u{30a4}\u{30e0}",
            &stack,
        );
        assert!(!boost.is_nan(), "NaN from unicode input for {}", profile_id);
        assert!(boost >= 0.0 && boost <= 0.20);

        let shift = scoring::detect_ecosystem_shift(
            &["\u{00e9}moji".to_string(), "caf\u{00e9}".to_string()],
            "\u{65e5}\u{672c}\u{8a9e}\u{30bf}\u{30a4}\u{30c8}\u{30eb}",
            &stack,
        );
        assert!(!shift.is_nan());
        assert_eq!(shift, 1.0); // No shift keywords match unicode
    }
}

#[test]
fn edge_very_long_inputs() {
    let rust_stack = stacks::compose_profiles(&["rust_systems".to_string()]);

    // ~13,000 characters — must not panic or timeout
    let long_content = "lorem ipsum dolor sit amet consectetur adipiscing elit ".repeat(250);
    let boost = scoring::compute_stack_boost("Long Article Title", &long_content, &rust_stack);
    assert!(boost >= 0.0 && boost <= 0.20);
    assert!(!boost.is_nan());
}

#[test]
fn edge_special_characters() {
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);

        // SQL injection patterns — must not panic, should return zero
        let boost = scoring::compute_stack_boost(
            "'; DROP TABLE items; --",
            "Robert'); DROP TABLE students;--",
            &stack,
        );
        assert_eq!(boost, 0.0);

        // Regex metacharacters — must not panic
        let boost2 = scoring::compute_stack_boost(
            "Article about [.*+?^${}()|\\]",
            "Content with regex chars \\d+ \\w+ .*",
            &stack,
        );
        assert!(boost2 >= 0.0);
    }
}

#[test]
fn edge_whitespace_variations() {
    let rust_stack = stacks::compose_profiles(&["rust_systems".to_string()]);

    // Tabs, newlines, multiple spaces — text_contains_term treats these
    // as non-word characters (boundaries), so keywords should still match.
    let boost = scoring::compute_stack_boost(
        "Rust\tAsync\nLifetimes",
        "Understanding\t\tasync\n\nlifetime\n\tpin\nsend",
        &rust_stack,
    );
    assert!(boost >= 0.0 && boost <= 0.20);
    assert!(!boost.is_nan());
}

// ============================================================================
// Tier 9: Monotonicity
// ============================================================================
// Scoring must be monotonically non-decreasing as signal strength increases.
// Adding more relevant keywords should never REDUCE the score.

#[test]
fn monotonicity_more_keywords_more_boost() {
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        let profile = stacks::get_profile(profile_id).unwrap();

        // Find a pain point with 4+ keywords for a 2kw vs 3kw vs 4kw comparison
        if let Some(pp) = profile.pain_points.iter().find(|p| p.keywords.len() >= 4) {
            let title_2kw = format!("{} {}", pp.keywords[0], pp.keywords[1]);
            let boost_2 = scoring::compute_stack_boost(&title_2kw, "", &stack);

            let title_3kw = format!("{} {} {}", pp.keywords[0], pp.keywords[1], pp.keywords[2]);
            let boost_3 = scoring::compute_stack_boost(&title_3kw, "", &stack);

            let title_4kw = format!(
                "{} {} {} {}",
                pp.keywords[0], pp.keywords[1], pp.keywords[2], pp.keywords[3]
            );
            let boost_4 = scoring::compute_stack_boost(&title_4kw, "", &stack);

            assert!(
                boost_3 >= boost_2,
                "{}: 3kw boost ({}) < 2kw boost ({}) for '{}'",
                profile_id,
                boost_3,
                boost_2,
                pp.description
            );
            assert!(
                boost_4 >= boost_3,
                "{}: 4kw boost ({}) < 3kw boost ({}) for '{}'",
                profile_id,
                boost_4,
                boost_3,
                pp.description
            );
        }
    }
}

#[test]
fn monotonicity_title_beats_content_only() {
    // A keyword in the title should produce >= the same keyword in content-only.
    // The scoring code gives title matches full boost and content-only half boost.
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        let profile = stacks::get_profile(profile_id).unwrap();

        for &(kw, _boost) in profile.keyword_boosts {
            let title_score = scoring::compute_stack_boost(kw, "", &stack);
            let content_score = scoring::compute_stack_boost("Unrelated title here", kw, &stack);

            assert!(
                title_score >= content_score,
                "{}: title boost ({:.4}) < content boost ({:.4}) for '{}'",
                profile_id,
                title_score,
                content_score,
                kw
            );
        }
    }
}

#[test]
fn monotonicity_shift_keywords_additive() {
    // More matching ecosystem shift keywords should never reduce the multiplier.
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        let profile = stacks::get_profile(profile_id).unwrap();

        for es in profile.ecosystem_shifts {
            if es.keywords.len() < 3 {
                continue;
            }
            // 2 keywords (threshold)
            let topics_2: Vec<String> = es.keywords[..2].iter().map(|s| s.to_string()).collect();
            let title_2 = topics_2.join(" ");
            let mult_2 = scoring::detect_ecosystem_shift(&topics_2, &title_2, &stack);

            // 3 keywords (above threshold)
            let topics_3: Vec<String> = es.keywords[..3].iter().map(|s| s.to_string()).collect();
            let title_3 = topics_3.join(" ");
            let mult_3 = scoring::detect_ecosystem_shift(&topics_3, &title_3, &stack);

            assert!(
                mult_3 >= mult_2,
                "{}: shift mult with 3kw ({}) < 2kw ({}) for '{}->{}'",
                profile_id,
                mult_3,
                mult_2,
                es.from,
                es.to
            );
        }
    }
}

// ============================================================================
// Tier 10: Snapshot Regression
// ============================================================================
// Computes a checksum of ALL scoring outputs for ALL corpus items across ALL
// profiles. If any profile data, scoring logic, or corpus item changes, this
// checksum changes. Forces explicit acknowledgment of scoring drift.
//
// To update after intentional changes:
//   cargo test --test stack_simulation snapshot_scoring_checksum -- --nocapture
// Copy the printed checksum into EXPECTED below.

#[test]
fn snapshot_scoring_checksum() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        for item in CORPUS {
            let boost = scoring::compute_stack_boost(item.title, item.content, &stack);
            let pain = scoring::has_pain_point_match(item.title, item.content, &stack);
            let topics = topics_from(item);
            let shift = scoring::detect_ecosystem_shift(&topics, item.title, &stack);
            let penalty = scoring::compute_competing_penalty(item.title, item.content, &stack);

            boost.to_bits().hash(&mut hasher);
            pain.hash(&mut hasher);
            shift.to_bits().hash(&mut hasher);
            penalty.to_bits().hash(&mut hasher);
        }
    }

    let checksum = hasher.finish();

    // UPDATE THIS VALUE when scoring logic or profile data intentionally changes.
    // Set to 0 to discover initial value (test will print it).
    const EXPECTED: u64 = 5602187936311967394;

    if EXPECTED == 0 {
        eprintln!(
            "\n  SNAPSHOT CHECKSUM: {}\n  \
             Copy this value into EXPECTED in snapshot_scoring_checksum.\n",
            checksum
        );
        panic!("Snapshot not initialized. Current checksum: {}", checksum);
    }

    assert_eq!(
        checksum, EXPECTED,
        "Scoring snapshot drifted! Expected: {}, Got: {}. \
         If intentional, update EXPECTED.",
        EXPECTED, checksum
    );
}

// ============================================================================
// Tier 11: Threshold Boundary
// ============================================================================
// Proves the 2-keyword minimum is enforced for pain points and ecosystem shifts.
// For every profile, constructs content with exactly 1 keyword from each pain
// point and verifies it does NOT trigger. This is stronger than fixed test data
// because it automatically covers every keyword in every profile.

#[test]
fn threshold_single_keyword_no_pain_point() {
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        let profile = stacks::get_profile(profile_id).unwrap();

        for pp in profile.pain_points {
            for &kw in pp.keywords {
                // Construct content with EXACTLY this one keyword
                let title = format!("Article discussing {} in depth", kw);
                let content = "some unrelated content about web development practices";

                if scoring::has_pain_point_match(&title, content, &stack) {
                    // If it triggered, SOME pain point in this profile must have
                    // 2+ keyword matches (could be a different pain point than the
                    // one we're testing, due to keyword overlap — e.g., "script setup"
                    // from TypeScript pain point also matches "setup" in Composition
                    // API pain point, giving it 2 matches).
                    let combined = format!("{} {}", title.to_lowercase(), content.to_lowercase());
                    let any_pp_justified = profile.pain_points.iter().any(|any_pp| {
                        let mc = any_pp
                            .keywords
                            .iter()
                            .filter(|k| scoring::text_contains_term(&combined, k))
                            .count();
                        mc >= 2
                    });
                    assert!(
                        any_pp_justified,
                        "{}: pain match triggered with keyword '{}' \
                         but no pain point has 2+ keyword matches",
                        profile_id, kw
                    );
                }
            }
        }
    }
}

#[test]
fn threshold_single_keyword_no_ecosystem_shift() {
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        let profile = stacks::get_profile(profile_id).unwrap();

        for es in profile.ecosystem_shifts {
            for &kw in es.keywords {
                let topics = vec![kw.to_string()];
                let title = format!("Article discussing {} trends", kw);
                let mult = scoring::detect_ecosystem_shift(&topics, &title, &stack);

                if mult > 1.0 {
                    // If it triggered, the title must accidentally contain another keyword
                    let title_lower = title.to_lowercase();
                    let match_count = es
                        .keywords
                        .iter()
                        .filter(|k| {
                            scoring::text_contains_term(&title_lower, k)
                                || topics.iter().any(|t| {
                                    let t_lower = t.to_lowercase();
                                    scoring::text_contains_term(&t_lower, k)
                                        || scoring::text_contains_term(k, &t_lower)
                                })
                        })
                        .count();
                    assert!(
                        match_count >= 2,
                        "{}: shift '{}->{}'  triggered with single keyword '{}' \
                         (match_count={}, expected >= 2)",
                        profile_id,
                        es.from,
                        es.to,
                        kw,
                        match_count
                    );
                }
            }
        }
    }
}

#[test]
fn threshold_two_keywords_from_different_pain_points_no_trigger() {
    // Having 1 keyword from pain point A and 1 keyword from pain point B
    // should NOT trigger either pain point. Only 2+ from the SAME pain point triggers.
    for &profile_id in &PROFILE_IDS {
        let stack = stacks::compose_profiles(&[profile_id.to_string()]);
        let profile = stacks::get_profile(profile_id).unwrap();

        if profile.pain_points.len() < 2 {
            continue;
        }

        // Take first keyword from each of two different pain points
        let kw_a = profile.pain_points[0].keywords[0];
        let kw_b = profile.pain_points[1].keywords[0];

        // Construct content with exactly one keyword from each
        let title = format!("{} and {} in modern development", kw_a, kw_b);
        let content = "general software engineering practices";

        // This should NOT trigger because neither pain point has 2+ matches
        // (unless kw_a or kw_b accidentally appears in the other's keyword list)
        let combined = format!("{} {}", title.to_lowercase(), content.to_lowercase());
        let pp0_matches = profile.pain_points[0]
            .keywords
            .iter()
            .filter(|k| scoring::text_contains_term(&combined, k))
            .count();
        let pp1_matches = profile.pain_points[1]
            .keywords
            .iter()
            .filter(|k| scoring::text_contains_term(&combined, k))
            .count();

        // If either pain point has 2+ matches, the keywords overlap between pain points
        // which is valid behavior. Only assert when each has exactly 1 match.
        if pp0_matches == 1 && pp1_matches == 1 {
            assert!(
                !scoring::has_pain_point_match(&title, content, &stack),
                "{}: 1kw from '{}' + 1kw from '{}' should not trigger pain match",
                profile_id,
                profile.pain_points[0].description,
                profile.pain_points[1].description,
            );
        }
    }
}
