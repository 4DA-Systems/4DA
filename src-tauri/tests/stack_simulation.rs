//! Stack Intelligence simulation harness.
//!
//! For each of the 8 stack profiles, generates synthetic content items and
//! validates that the scoring functions produce meaningful differentiation:
//! - Pain point content gets a meaningful lift
//! - Off-stack (competing) content stays suppressed
//! - Multi-tech (synergy) content outscores single-tech
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
        SyntheticItem { title: "Why We Switched from Prisma to Drizzle", content: "drizzle orm prisma alternative migration performance type safety", category: "ecosystem_shift" },
        SyntheticItem { title: "Biome: The ESLint and Prettier Replacement", content: "biome eslint alternative biome formatter biomejs linter migration", category: "ecosystem_shift" },
        SyntheticItem { title: "Bun vs Node.js Runtime Comparison", content: "bun runtime bun install bun vs node performance benchmarks", category: "ecosystem_shift" },
        // Competing
        SyntheticItem { title: "SvelteKit 2.0 Released", content: "sveltekit svelte framework release features improvements", category: "competing" },
        SyntheticItem { title: "Remix vs Next.js Performance Showdown", content: "remix framework performance nextjs comparison routing", category: "competing" },
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
            title: "Native Async Traits Stabilized in Rust",
            content:
                "native async trait async fn in trait return position impl trait stabilization",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Const Generics: From Nightly to Stable",
            content: "const generics generic const stabilization feature gate stable rust",
            category: "ecosystem_shift",
        },
        SyntheticItem {
            title: "Rust RPITIT: Return Position Impl Trait in Traits",
            content: "return position impl trait traits async stabilization nightly stable",
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
            content: "django orm queryset python database optimization n+1",
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
            content: "tauri rust react typescript desktop app development serde",
            category: "synergy",
        },
        SyntheticItem {
            title: "Axum + SQLx REST API in Rust",
            content: "axum sqlx rust tokio rest api database postgresql web server",
            category: "synergy",
        },
        SyntheticItem {
            title: "Rust WASM with Tokio for High-Performance Web",
            content: "rust wasm tokio async web performance webassembly browser",
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
            let topics: Vec<String> = item
                .title
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
        }
    };
}

profile_simulation!(nextjs_sim, "nextjs_fullstack", nextjs_items);
profile_simulation!(rust_sim, "rust_systems", rust_items);

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
