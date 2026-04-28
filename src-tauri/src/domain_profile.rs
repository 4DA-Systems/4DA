// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Domain Profile — Technology Identity Model for 4DA
//!
//! Builds a graduated understanding of the user's technology identity from:
//! - Onboarding tech stack (primary_stack)
//! - ACE-detected tech (detected_tech)
//! - Project dependencies (dependency graph)
//! - Declared interests (interests)
//!
//! Used by scoring to compute graduated domain relevance (not just binary on/off domain).

use std::collections::HashSet;

use crate::domain_profile_data::{
    adjacency_map, archetype_map, AMBIGUOUS_DEPS, CROSS_CUTTING_TOPICS, UTILITY_DEPS,
};

/// User's technology identity — what they work with and care about
#[derive(Debug, Clone, Default)]
pub struct DomainProfile {
    /// Primary tech stack from onboarding (highest weight)
    pub primary_stack: HashSet<String>,
    /// Adjacent technologies inferred from primary stack
    pub adjacent_tech: HashSet<String>,
    /// All tech combined (primary + detected + deps + adjacent)
    pub all_tech: HashSet<String>,
    /// Dependency names from project manifests
    pub dependency_names: HashSet<String>,
    /// Declared interest topics
    pub interest_topics: HashSet<String>,
    /// Domain concerns: non-tech keywords relevant to this developer type
    /// e.g. a desktop app dev cares about "packaging", "installer", "auto-update"
    pub domain_concerns: HashSet<String>,
    /// High-confidence ACE tech auto-promoted into all_tech for scoring
    /// (tracked separately so we know what was auto-promoted vs declared)
    pub ace_promoted_tech: HashSet<String>,
}

impl DomainProfile {
    /// Check if profile has been populated at all
    pub fn is_empty(&self) -> bool {
        self.primary_stack.is_empty()
            && self.all_tech.is_empty()
            && self.dependency_names.is_empty()
            && self.interest_topics.is_empty()
    }
}

/// Build a DomainProfile from the database
pub fn build_domain_profile(conn: &rusqlite::Connection) -> DomainProfile {
    let mut primary_stack = HashSet::new();
    let mut all_tech = HashSet::new();
    let mut dependency_names = HashSet::new();
    let mut interest_topics = HashSet::new();

    // 1. Onboarding tech stack (primary — highest weight)
    // CONTENT ACCURACY GATE: Only display-worthy tech enters primary_stack.
    // Non-display-worthy tech (ORMs, utility libs) goes into all_tech for
    // scoring but never reaches user-facing personalized content.
    if let Ok(mut stmt) = conn.prepare("SELECT technology FROM tech_stack") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for tech in rows.flatten() {
                let lower = tech.to_lowercase();
                all_tech.insert(lower.clone());
                if is_display_worthy(&lower) {
                    primary_stack.insert(lower);
                }
            }
        }
    }

    // 2. ACE-detected tech (secondary — auto-scanned)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT name FROM detected_tech WHERE category IN ('Language', 'Framework', 'Database', 'Library') AND confidence >= 0.8",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for tech in rows.flatten() {
                all_tech.insert(tech.to_lowercase());
            }
        }
    }

    // 3. Declared interests
    if let Ok(mut stmt) = conn.prepare("SELECT topic FROM explicit_interests") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for topic in rows.flatten() {
                interest_topics.insert(topic.to_lowercase());
            }
        }
    }

    // 4. Project dependencies (non-dev packages only)
    if let Ok(mut stmt) =
        conn.prepare("SELECT DISTINCT package_name FROM project_dependencies WHERE is_dev = 0")
    {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for dep in rows.flatten() {
                let lower = dep.to_lowercase();
                if is_notable_dependency(&lower) {
                    dependency_names.insert(lower.clone());
                    all_tech.insert(lower);
                }
            }
        }
    }

    // 5. Infer adjacent tech from primary stack
    let adjacent_tech = infer_adjacent_tech(&primary_stack);
    for tech in &adjacent_tech {
        all_tech.insert(tech.clone());
    }

    // 6. Infer domain concerns from developer archetype
    let domain_concerns = infer_domain_concerns(&primary_stack, &all_tech);

    DomainProfile {
        primary_stack,
        adjacent_tech,
        all_tech,
        dependency_names,
        interest_topics,
        domain_concerns,
        ace_promoted_tech: HashSet::new(),
    }
}

/// Check if a dependency name is ambiguous (exists across multiple ecosystems).
fn is_ambiguous_dep(name: &str) -> bool {
    AMBIGUOUS_DEPS.contains(&name)
}

/// Check if ANY other topic (besides `skip`) corroborates domain membership.
/// Primary stack matches always corroborate. all_tech matches only corroborate
/// if the corroborating topic itself is NOT ambiguous — otherwise "async" (which
/// exists in every ecosystem) would falsely corroborate "futures" for C++ articles.
fn has_corroboration(topics: &[String], skip: &str, profile: &DomainProfile) -> bool {
    topics.iter().any(|t| {
        let lower = t.to_lowercase();
        if lower == skip {
            return false;
        }
        // Primary stack match always corroborates (explicit user declaration)
        if profile
            .primary_stack
            .iter()
            .any(|s| fuzzy_tech_match(&lower, s))
        {
            return true;
        }
        // all_tech match corroborates ONLY if the topic itself isn't ambiguous.
        // "tokio" corroborates (specific), "async" does NOT (cross-ecosystem).
        if !is_ambiguous_dep(&lower) {
            return profile.all_tech.iter().any(|s| fuzzy_tech_match(&lower, s));
        }
        false
    })
}

/// Compute graduated domain relevance for a set of topics against the profile.
/// Returns 0.0 (completely off-domain) to 1.0 (direct primary stack match).
///
/// Scoring tiers:
///   1.0 — topic matches primary stack
///   0.85 — topic matches a project dependency (non-ambiguous, or ambiguous with corroboration)
///   0.70 — topic matches detected/adjacent tech
///   0.50 — topic matches an interest OR ambiguous dep without corroboration
///   0.15 — no match but profile is populated (off-domain)
///   1.0 — profile is empty (no filtering, backward compat)
pub fn compute_domain_relevance(topics: &[String], profile: &DomainProfile) -> f32 {
    if profile.is_empty() {
        return 1.0; // No profile = don't penalize anything
    }

    if topics.is_empty() {
        return 0.15; // No topics extracted = likely off-domain
    }

    let mut best_relevance: f32 = 0.0;

    for topic in topics {
        let lower = topic.to_lowercase();

        // Check primary stack (highest tier)
        if profile
            .primary_stack
            .iter()
            .any(|t| fuzzy_tech_match(&lower, t))
        {
            best_relevance = best_relevance.max(1.0);
            continue;
        }

        // Check dependency names
        if profile
            .dependency_names
            .iter()
            .any(|d| fuzzy_tech_match(&lower, d))
        {
            // Ambiguous deps (e.g. "futures") need corroboration from another topic
            if is_ambiguous_dep(&lower) {
                if has_corroboration(topics, &lower, profile) {
                    best_relevance = best_relevance.max(0.85);
                } else {
                    // Ambiguous dep with no corroboration → downgrade to interest-level
                    best_relevance = best_relevance.max(0.50);
                }
            } else {
                best_relevance = best_relevance.max(0.85);
            }
            continue;
        }

        // Check adjacency-derived tech FIRST (inferred, not declared — weaker evidence).
        // "desktop" adjacent to "tauri" shouldn't boost a Java desktop article.
        // "database" adjacent to "sqlite" shouldn't boost a MongoDB article.
        // ALL adjacency matches require corroboration because they're inferred, not declared.
        if profile
            .adjacent_tech
            .iter()
            .any(|t| fuzzy_tech_match(&lower, t))
        {
            if has_corroboration(topics, &lower, profile) {
                best_relevance = best_relevance.max(0.70);
            } else {
                best_relevance = best_relevance.max(0.50); // downgrade to interest-level
            }
            continue;
        }

        // Check remaining all_tech (detected tech, NOT adjacency — already handled above)
        // Ambiguous topics (e.g. "async") still need corroboration.
        if profile.all_tech.iter().any(|t| fuzzy_tech_match(&lower, t)) {
            if is_ambiguous_dep(&lower) {
                if has_corroboration(topics, &lower, profile) {
                    best_relevance = best_relevance.max(0.70);
                } else {
                    best_relevance = best_relevance.max(0.50);
                }
            } else {
                best_relevance = best_relevance.max(0.70);
            }
            continue;
        }

        // Check interest topics
        if profile
            .interest_topics
            .iter()
            .any(|t| fuzzy_tech_match(&lower, t))
        {
            best_relevance = best_relevance.max(0.50);
            continue;
        }

        // Check domain concerns (archetype-inferred non-tech keywords)
        // "packaging" isn't a tech name, but a Tauri dev should see packaging articles
        if profile
            .domain_concerns
            .iter()
            .any(|c| fuzzy_tech_match(&lower, c))
        {
            best_relevance = best_relevance.max(0.60);
            continue;
        }

        // Cross-cutting topics: universally relevant to all developers
        // "testing", "architecture", "deployment" etc. should never be crushed as off-domain
        if CROSS_CUTTING_TOPICS.contains(&lower.as_str()) {
            best_relevance = best_relevance.max(0.60);
            continue;
        }
    }

    if best_relevance > 0.0 {
        best_relevance
    } else {
        0.15 // Off-domain minimum
    }
}

/// Fuzzy technology matching that handles common variations:
/// - "react" matches "reactjs", "react.js", "react-native"
/// - "tauri" matches "tauri-apps"
/// - "sqlite" matches "rusqlite", "sqlite3"
fn fuzzy_tech_match(topic: &str, tech: &str) -> bool {
    if topic == tech {
        return true;
    }
    // Short terms (< 3 chars) require exact match
    if topic.len() < 3 || tech.len() < 3 {
        return topic == tech;
    }
    // Substring containment (both directions)
    topic.contains(tech) || tech.contains(topic)
}

/// Filter out tiny utility packages that don't represent meaningful tech identity
fn is_notable_dependency(name: &str) -> bool {
    if name.len() < 4 {
        return false;
    }
    !UTILITY_DEPS.contains(&name)
}

/// Check if a technology is display-worthy in user-facing personalized content.
///
/// This is the **content accuracy gate** — only technologies that genuinely
/// represent a developer's identity should appear in personalized STREETS
/// content, template interpolation ({= stack.primary =}), DiffRibbons,
/// and Developer DNA summaries.
///
/// Languages, major frameworks, runtimes, and platforms pass. ORMs, utility
/// libraries, build tools, and companion packages do NOT — they belong in
/// `all_tech` for scoring relevance, but should never be shown to the user
/// as part of their identity.
/// ## Inclusion Criteria
///
/// A technology belongs here if a developer would put it in their bio, resume
/// headline, or conference talk title. "Rust developer" — yes. "Drizzle
/// developer" — no.
///
/// Categories that pass: programming languages, major application frameworks,
/// runtimes, cloud platforms, container/orchestration tools, databases.
///
/// Categories that fail: ORMs, CSS frameworks, build tools, linters, formatters,
/// testing libraries, utility packages, package managers, companion tools.
///
/// ## Maintenance
///
/// Review quarterly. When adding a new entry, ask: "Would someone describe
/// themselves as a [X] developer?" If yes, add it. If it's a tool *used by*
/// a type of developer rather than *defining* that type, it stays out.
/// Last reviewed: 2026-03.
pub fn is_display_worthy(tech: &str) -> bool {
    const DISPLAY_WORTHY: &[&str] = &[
        // ---- Languages ----
        "rust",
        "typescript",
        "javascript",
        "python",
        "go",
        "java",
        "kotlin",
        "swift",
        "c",
        "cpp",
        "c++",
        "csharp",
        "c#",
        "ruby",
        "php",
        "scala",
        "elixir",
        "haskell",
        "dart",
        "zig",
        "nim",
        "lua",
        "r",
        "julia",
        "wgsl",
        "glsl",
        "sql",
        "ocaml",
        "clojure",
        "erlang",
        "fsharp",
        "f#",
        "objective-c",
        "perl",
        // Emerging languages (reviewed 2026-03)
        "mojo",
        "gleam",
        "roc",
        "unison",
        "vale",
        // ---- Web frameworks (frontend) ----
        "react",
        "vue",
        "angular",
        "svelte",
        "solid",
        "solidjs",
        "qwik",
        "htmx",
        "alpine",
        "alpinejs",
        "preact",
        "lit",
        // ---- Meta-frameworks ----
        "nextjs",
        "next.js",
        "nuxt",
        "remix",
        "astro",
        "sveltekit",
        "solidstart",
        "fresh",
        "analog",
        // ---- Desktop / mobile ----
        "tauri",
        "electron",
        "flutter",
        "react-native",
        "expo",
        "swiftui",
        "jetpack-compose",
        "maui",
        // ---- Backend frameworks ----
        "django",
        "flask",
        "fastapi",
        "litestar",
        "rails",
        "spring",
        "springboot",
        "quarkus",
        "micronaut",
        "express",
        "nest",
        "nestjs",
        "hono",
        "elysia",
        "actix",
        "axum",
        "rocket",
        "warp",
        "gin",
        "fiber",
        "echo",
        "chi",
        "phoenix",
        "plug",
        // ---- AI / ML ----
        "tensorflow",
        "pytorch",
        "jax",
        "langchain",
        "llamaindex",
        "huggingface",
        "transformers",
        "onnx",
        // ---- Runtimes ----
        "node",
        "nodejs",
        "deno",
        "bun",
        // ---- Platforms / infrastructure ----
        "aws",
        "gcp",
        "azure",
        "docker",
        "kubernetes",
        "linux",
        "wasm",
        "webgpu",
        "vercel",
        "cloudflare",
        "supabase",
        "firebase",
        "netlify",
        "fly",
        "railway",
        // ---- API / communication ----
        "graphql",
        "grpc",
        // ---- Databases (identity-level, not ORMs) ----
        "postgresql",
        "postgres",
        "mysql",
        "mongodb",
        "redis",
        "sqlite",
        "dynamodb",
        "cassandra",
        "elasticsearch",
        "neo4j",
        "cockroachdb",
        "planetscale",
        "neon",
        "turso",
        "surrealdb",
        "kafka",
    ];

    DISPLAY_WORTHY.contains(&tech)
}

/// Infer domain concerns from the user's tech profile.
/// Domain concerns are non-tech keywords that are contextually relevant based on
/// what kind of developer the user is. A Tauri dev cares about "packaging" and "installer"
/// even though those aren't tech names.
fn infer_domain_concerns(primary: &HashSet<String>, all_tech: &HashSet<String>) -> HashSet<String> {
    let mut concerns = HashSet::new();

    let combined: HashSet<&str> = primary
        .iter()
        .chain(all_tech.iter())
        .map(std::string::String::as_str)
        .collect();

    for (signals, domain_concerns) in archetype_map() {
        if signals.iter().any(|s| combined.contains(s)) {
            for concern in *domain_concerns {
                concerns.insert(concern.to_string());
            }
        }
    }

    concerns
}

/// Infer adjacent technologies from the primary tech stack.
fn infer_adjacent_tech(primary: &HashSet<String>) -> HashSet<String> {
    let mut adjacent = HashSet::new();
    let adjacency = adjacency_map();

    for tech in primary {
        if let Some(neighbors) = adjacency.get(tech.as_str()) {
            for neighbor in *neighbors {
                adjacent.insert(neighbor.to_string());
            }
        }
    }

    adjacent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_tech_match() {
        assert!(fuzzy_tech_match("react", "react"));
        assert!(fuzzy_tech_match("reactjs", "react"));
        assert!(fuzzy_tech_match("react", "reactjs"));
        assert!(fuzzy_tech_match("rusqlite", "sqlite"));
        assert!(!fuzzy_tech_match("go", "google")); // Too short for substring
        assert!(!fuzzy_tech_match("fashion", "rust"));
    }

    #[test]
    fn test_domain_relevance_primary_stack() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string(), "tauri".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["rust".to_string(), "tauri".to_string()]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["rust".to_string(), "performance".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 1.0);
    }

    #[test]
    fn test_domain_relevance_off_domain() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["rust".to_string()]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["fashion".to_string(), "dining".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.15);
    }

    #[test]
    fn test_domain_relevance_empty_profile() {
        let profile = DomainProfile {
            primary_stack: HashSet::new(),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::new(),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["anything".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 1.0);
    }

    #[test]
    fn test_domain_relevance_dependency_match() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["rust".to_string(), "tokio".to_string()]),
            dependency_names: HashSet::from(["tokio".to_string()]),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["tokio".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.85);
    }

    #[test]
    fn test_domain_relevance_interest_match() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["rust".to_string()]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::from(["machine learning".to_string()]),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["machine learning".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.50);
    }

    #[test]
    fn test_infer_adjacent_tech() {
        let primary = HashSet::from(["rust".to_string()]);
        let adjacent = infer_adjacent_tech(&primary);
        assert!(adjacent.contains("cargo"));
        assert!(adjacent.contains("wasm"));
        assert!(adjacent.contains("tokio"));
    }

    #[test]
    fn test_is_notable_dependency() {
        assert!(is_notable_dependency("tauri"));
        assert!(is_notable_dependency("serde"));
        assert!(is_notable_dependency("reqwest"));
        assert!(!is_notable_dependency("cc"));
        assert!(!is_notable_dependency("syn"));
        assert!(!is_notable_dependency("proc-macro2"));
    }

    #[test]
    fn test_ambiguous_dep_without_corroboration() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["rust".to_string(), "tokio".to_string()]),
            dependency_names: HashSet::from(["futures".to_string(), "tokio".to_string()]),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["futures".to_string(), "c++".to_string(), "hpx".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.50);
    }

    #[test]
    fn test_ambiguous_dep_with_corroboration() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["rust".to_string(), "tokio".to_string()]),
            dependency_names: HashSet::from(["futures".to_string(), "tokio".to_string()]),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec![
            "futures".to_string(),
            "tokio".to_string(),
            "async".to_string(),
        ];
        assert!(compute_domain_relevance(&topics, &profile) >= 0.85);
    }

    #[test]
    fn test_non_ambiguous_dep_no_corroboration_needed() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["rust".to_string(), "tokio".to_string()]),
            dependency_names: HashSet::from(["tokio".to_string()]),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["tokio".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.85);
    }

    #[test]
    fn test_ambiguous_dep_with_only_ambiguous_corroboration() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::from(["async".to_string()]),
            all_tech: HashSet::from(["rust".to_string(), "tokio".to_string(), "async".to_string()]),
            dependency_names: HashSet::from(["futures".to_string()]),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec![
            "futures".to_string(),
            "async".to_string(),
            "parallel".to_string(),
            "hpx".to_string(),
        ];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.50);
    }

    #[test]
    fn test_ambiguous_dep_with_primary_stack_corroboration() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::from(["async".to_string()]),
            all_tech: HashSet::from(["rust".to_string(), "async".to_string()]),
            dependency_names: HashSet::from(["futures".to_string()]),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec![
            "futures".to_string(),
            "async".to_string(),
            "rust".to_string(),
        ];
        assert_eq!(compute_domain_relevance(&topics, &profile), 1.0);
    }

    #[test]
    fn test_adjacency_without_corroboration() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["tauri".to_string(), "rust".to_string()]),
            adjacent_tech: HashSet::from([
                "desktop".to_string(),
                "webview".to_string(),
                "ipc".to_string(),
            ]),
            all_tech: HashSet::from([
                "tauri".to_string(),
                "rust".to_string(),
                "desktop".to_string(),
                "webview".to_string(),
                "ipc".to_string(),
            ]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["desktop".to_string(), "java".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.50);
    }

    #[test]
    fn test_adjacency_with_corroboration() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["tauri".to_string(), "rust".to_string()]),
            adjacent_tech: HashSet::from([
                "desktop".to_string(),
                "webview".to_string(),
                "ipc".to_string(),
            ]),
            all_tech: HashSet::from([
                "tauri".to_string(),
                "rust".to_string(),
                "desktop".to_string(),
                "webview".to_string(),
                "ipc".to_string(),
            ]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["desktop".to_string(), "rust".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 1.0);

        let topics2 = vec!["desktop".to_string(), "webview".to_string()];
        assert!(compute_domain_relevance(&topics2, &profile) >= 0.70);
    }

    #[test]
    fn test_adjacency_general_database() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["sqlite".to_string()]),
            adjacent_tech: HashSet::from([
                "sql".to_string(),
                "database".to_string(),
                "rusqlite".to_string(),
            ]),
            all_tech: HashSet::from([
                "sqlite".to_string(),
                "sql".to_string(),
                "database".to_string(),
                "rusqlite".to_string(),
            ]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["database".to_string(), "mongodb".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.50);
    }

    // ====================================================================
    // P1: Developer Archetype Tests
    // ====================================================================

    #[test]
    fn test_infer_domain_concerns_desktop_dev() {
        let primary = HashSet::from(["tauri".to_string(), "rust".to_string()]);
        let all_tech = primary.clone();
        let concerns = infer_domain_concerns(&primary, &all_tech);
        assert!(concerns.contains("packaging"));
        assert!(concerns.contains("auto-update"));
        assert!(concerns.contains("installer"));
        // Also gets systems concerns from "rust"
        assert!(concerns.contains("memory safety"));
    }

    #[test]
    fn test_infer_domain_concerns_frontend_dev() {
        let primary = HashSet::from(["react".to_string(), "typescript".to_string()]);
        let all_tech = primary.clone();
        let concerns = infer_domain_concerns(&primary, &all_tech);
        assert!(concerns.contains("ssr"));
        assert!(concerns.contains("hydration"));
        assert!(concerns.contains("bundle size"));
    }

    #[test]
    fn test_infer_domain_concerns_ml_engineer() {
        let primary = HashSet::from(["python".to_string(), "pytorch".to_string()]);
        let all_tech = primary.clone();
        let concerns = infer_domain_concerns(&primary, &all_tech);
        assert!(concerns.contains("training"));
        assert!(concerns.contains("inference"));
        assert!(concerns.contains("embeddings"));
    }

    #[test]
    fn test_domain_concern_relevance() {
        // Tauri dev sees article about "cross-platform packaging strategies"
        // Topic: "packaging" — matches domain_concerns → 0.60
        let profile = DomainProfile {
            primary_stack: HashSet::from(["tauri".to_string(), "rust".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["tauri".to_string(), "rust".to_string()]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::from([
                "packaging".to_string(),
                "auto-update".to_string(),
                "installer".to_string(),
            ]),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["packaging".to_string(), "strategies".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.60);
    }

    // ====================================================================
    // P2: Cross-Cutting Topic Tests
    // ====================================================================

    #[test]
    fn test_cross_cutting_topic_not_crushed() {
        // Article about "Best Testing Practices" — "testing" is cross-cutting.
        // Should get 0.60 for ANY developer, not 0.15.
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["rust".to_string()]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["testing".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.60);
    }

    #[test]
    fn test_cross_cutting_architecture() {
        let profile = DomainProfile {
            primary_stack: HashSet::from(["python".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["python".to_string()]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["architecture".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.60);
    }

    #[test]
    fn test_cross_cutting_doesnt_override_primary() {
        // "security" is cross-cutting (0.60) but also in SINGLE_WORD_TOPICS.
        // If user's primary stack matches, should still get 1.0.
        let profile = DomainProfile {
            primary_stack: HashSet::from(["rust".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["rust".to_string()]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::new(),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["rust".to_string(), "security".to_string()];
        // "rust" hits primary → 1.0 (cross-cutting doesn't downgrade)
        assert_eq!(compute_domain_relevance(&topics, &profile), 1.0);
    }

    #[test]
    fn test_domain_concern_beats_cross_cutting() {
        // A topic matching domain_concern (0.60) and cross-cutting (0.60) → still 0.60
        // But domain_concern is checked first, which is fine — same tier
        let profile = DomainProfile {
            primary_stack: HashSet::from(["react".to_string()]),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::from(["react".to_string()]),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
            domain_concerns: HashSet::from(["accessibility".to_string()]),
            ace_promoted_tech: HashSet::new(),
        };
        let topics = vec!["accessibility".to_string()];
        assert_eq!(compute_domain_relevance(&topics, &profile), 0.60);
    }

    // ====================================================================
    // Content Accuracy Gate Tests (is_display_worthy)
    // ====================================================================

    #[test]
    fn test_display_worthy_rejects_orms() {
        // The drizzle bug: ORMs must never appear in primary_stack
        assert!(!is_display_worthy("drizzle"));
        assert!(!is_display_worthy("prisma"));
        assert!(!is_display_worthy("typeorm"));
        assert!(!is_display_worthy("sequelize"));
        assert!(!is_display_worthy("knex"));
        assert!(!is_display_worthy("mongoose"));
    }

    #[test]
    fn test_display_worthy_rejects_build_tools() {
        assert!(!is_display_worthy("webpack"));
        assert!(!is_display_worthy("vite"));
        assert!(!is_display_worthy("esbuild"));
        assert!(!is_display_worthy("rollup"));
        assert!(!is_display_worthy("turbopack"));
        assert!(!is_display_worthy("biome"));
        assert!(!is_display_worthy("eslint"));
    }

    #[test]
    fn test_display_worthy_rejects_companion_packages() {
        assert!(!is_display_worthy("tailwindcss"));
        assert!(!is_display_worthy("trpc"));
        assert!(!is_display_worthy("zod"));
        assert!(!is_display_worthy("turborepo"));
        assert!(!is_display_worthy("pnpm"));
    }

    #[test]
    fn test_display_worthy_accepts_all_languages() {
        for lang in &[
            "rust",
            "typescript",
            "javascript",
            "python",
            "go",
            "java",
            "kotlin",
            "swift",
            "ruby",
            "php",
            "scala",
            "elixir",
            "haskell",
            "dart",
            "zig",
            "nim",
            "lua",
            "julia",
        ] {
            assert!(is_display_worthy(lang), "{} should be display-worthy", lang);
        }
    }

    #[test]
    fn test_display_worthy_accepts_major_frameworks() {
        for fw in &[
            "react", "vue", "angular", "svelte", "nextjs", "tauri", "electron", "django", "flask",
            "fastapi", "rails", "spring", "express", "actix", "axum", "rocket", "flutter",
        ] {
            assert!(is_display_worthy(fw), "{} should be display-worthy", fw);
        }
    }

    #[test]
    fn test_display_worthy_accepts_databases() {
        for db in &[
            "postgresql",
            "postgres",
            "mysql",
            "mongodb",
            "redis",
            "sqlite",
            "elasticsearch",
        ] {
            assert!(is_display_worthy(db), "{} should be display-worthy", db);
        }
    }

    #[test]
    fn test_display_worthy_accepts_platforms() {
        for p in &[
            "aws",
            "gcp",
            "azure",
            "docker",
            "kubernetes",
            "linux",
            "wasm",
            "vercel",
            "cloudflare",
            "supabase",
            "firebase",
        ] {
            assert!(is_display_worthy(p), "{} should be display-worthy", p);
        }
    }

    #[test]
    fn test_display_worthy_accepts_emerging_languages() {
        for lang in &["mojo", "gleam", "roc", "unison", "vale"] {
            assert!(is_display_worthy(lang), "{} should be display-worthy", lang);
        }
    }

    #[test]
    fn test_display_worthy_accepts_emerging_frameworks() {
        for fw in &[
            "hono",
            "elysia",
            "solidstart",
            "htmx",
            "litestar",
            "phoenix",
            "expo",
            "fresh",
            "turso",
            "surrealdb",
        ] {
            assert!(is_display_worthy(fw), "{} should be display-worthy", fw);
        }
    }
}
