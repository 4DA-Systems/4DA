// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Competing Technology Anti-Affinity for 4DA
//!
//! Detects when content is primarily about a technology that competes with
//! the user's chosen stack. Electron content for a Tauri developer, Vue content
//! for a React developer, etc.

use std::collections::HashSet;

/// Ecosystem map: (technology, &[its competitors])
pub(crate) const COMPETING_TECH: &[(&str, &[&str])] = &[
    // Desktop frameworks
    ("tauri", &["electron", "nwjs", "neutralino", "wails", "cef"]),
    ("electron", &["tauri", "nwjs", "neutralino", "wails"]),
    // Frontend frameworks (mild — cross-pollination has value)
    ("react", &["vue", "angular", "svelte", "solid", "qwik"]),
    ("vue", &["react", "angular", "svelte", "solid"]),
    ("angular", &["react", "vue", "svelte", "solid"]),
    ("svelte", &["react", "vue", "angular"]),
    // Backend frameworks — "React and Laravel" is not relevant if user doesn't use Laravel
    (
        "rust",
        &[
            "go", "django", "laravel", "rails", "spring", "flask", "gin", "echo", "fastapi",
        ],
    ),
    (
        "axum",
        &[
            "express", "fastify", "koa", "hapi", "django", "laravel", "rails", "spring", "flask",
            "gin", "echo", "fastapi", "fiber",
        ],
    ),
    // Meta-frameworks (Next.js user doesn't need Nuxt content and vice versa)
    ("next", &["nuxt", "remix", "gatsby", "astro", "sveltekit"]),
    ("nextjs", &["nuxt", "remix", "gatsby", "astro", "sveltekit"]),
    (
        "nuxt",
        &["next", "nextjs", "remix", "gatsby", "astro", "sveltekit"],
    ),
    ("remix", &["next", "nextjs", "nuxt", "gatsby", "astro"]),
    ("gatsby", &["next", "nextjs", "nuxt", "remix", "astro"]),
    ("astro", &["next", "nextjs", "nuxt", "remix", "gatsby"]),
    // CSS approaches
    (
        "tailwindcss",
        &[
            "styled-components",
            "emotion",
            "css-modules",
            "bootstrap",
            "material-ui",
        ],
    ),
    (
        "tailwind",
        &[
            "styled-components",
            "emotion",
            "css-modules",
            "bootstrap",
            "material-ui",
        ],
    ),
    (
        "styled-components",
        &["tailwindcss", "tailwind", "emotion", "css-modules"],
    ),
    // ORMs / database clients
    ("prisma", &["drizzle", "typeorm", "sequelize", "mongoose"]),
    ("drizzle", &["prisma", "typeorm", "sequelize", "mongoose"]),
    ("diesel", &["sqlx", "sea-orm"]),
    ("sqlx", &["diesel", "sea-orm"]),
    // Backend frameworks (Python)
    (
        "django",
        &["flask", "fastapi", "express", "rails", "laravel", "spring"],
    ),
    (
        "flask",
        &["django", "fastapi", "express", "rails", "laravel", "spring"],
    ),
    (
        "fastapi",
        &["django", "flask", "express", "rails", "laravel"],
    ),
    // Backend frameworks (Node)
    (
        "express",
        &[
            "fastify", "koa", "hapi", "nest", "nestjs", "django", "flask", "rails",
        ],
    ),
    ("fastify", &["express", "koa", "hapi", "nest", "nestjs"]),
    // Backend frameworks (Ruby/PHP/Java)
    (
        "rails",
        &["django", "laravel", "spring", "express", "fastapi"],
    ),
    (
        "laravel",
        &["django", "rails", "spring", "express", "fastapi"],
    ),
    (
        "spring",
        &["django", "rails", "laravel", "express", "fastapi"],
    ),
    // Systems languages
    ("go", &["rust", "java", "csharp"]),
    // State management (React ecosystem)
    ("redux", &["zustand", "jotai", "mobx", "recoil", "valtio"]),
    ("zustand", &["redux", "jotai", "mobx", "recoil", "valtio"]),
    ("jotai", &["redux", "zustand", "mobx", "recoil"]),
    ("mobx", &["redux", "zustand", "jotai", "recoil"]),
    // Testing frameworks
    ("jest", &["vitest", "mocha", "ava"]),
    ("vitest", &["jest", "mocha", "ava"]),
    ("pytest", &["unittest", "nose2"]),
    ("unittest", &["pytest", "nose2"]),
    // CI/CD platforms
    (
        "github-actions",
        &["gitlab-ci", "circleci", "jenkins", "travis"],
    ),
    ("gitlab-ci", &["github-actions", "circleci", "jenkins"]),
    ("jenkins", &["github-actions", "gitlab-ci", "circleci"]),
    // Package managers
    ("pnpm", &["npm", "yarn"]),
    ("yarn", &["npm", "pnpm"]),
    // Runtimes
    ("deno", &["node", "bun"]),
    ("bun", &["node", "deno"]),
    // Databases (when used as primary)
    ("sqlite", &["mongodb", "dynamodb", "couchdb"]),
    ("postgresql", &["mysql", "mariadb"]),
    ("mongodb", &["postgresql", "mysql", "sqlite"]),
    // Build tools
    ("vite", &["webpack", "parcel", "rollup", "turbopack"]),
    ("webpack", &["vite", "parcel", "rollup", "turbopack"]),
    ("esbuild", &["webpack", "rollup", "parcel"]),
    // Cloud platforms
    ("vercel", &["netlify", "cloudflare", "aws"]),
    ("netlify", &["vercel", "cloudflare"]),
    // BaaS
    ("supabase", &["firebase", "appwrite", "pocketbase"]),
    ("firebase", &["supabase", "appwrite", "pocketbase"]),
    // Type systems
    ("typescript", &["flow", "rescript"]),
    // Backend languages (when article is about a different backend entirely)
    ("python", &["java", "csharp", "php", "ruby"]),
    ("java", &["python", "csharp", "php", "ruby"]),
];

/// Check if content is primarily about a competing technology.
/// Returns a graduated multiplier:
///   1.0  — no competition, or user's tech in title (comparative exemption)
///   0.85 — comparative/migration article (useful framing even without user's tech)
///   0.80 — competitor mentioned but user's tech appears in content body
///   0.70 — competitor only in topics, not in title (incidental mention)
///   0.50 — pure competing content (competitor in title, no user tech anywhere)
pub fn compute_competing_penalty(
    topics: &[String],
    title: &str,
    content: &str,
    user_primary_stack: &HashSet<String>,
) -> f32 {
    let title_lower = title.to_lowercase();
    // Limit content scan to first 2000 chars for performance
    let content_lower = content[..content.floor_char_boundary(2000)].to_lowercase();

    for user_tech in user_primary_stack {
        let user_lower = user_tech.to_lowercase();

        // Find the competitor list for this user tech
        let competitors = match COMPETING_TECH
            .iter()
            .find(|(tech, _)| *tech == user_lower.as_str())
        {
            Some((_, comps)) => comps,
            None => continue,
        };

        // Check which competitor appears in the title or topics
        let matched_competitor = competitors.iter().find(|comp| {
            has_word_boundary(&title_lower, comp)
                || topics
                    .iter()
                    .any(|t| t.to_lowercase() == **comp || t.to_lowercase().starts_with(*comp))
        });

        let matched_competitor = match matched_competitor {
            Some(comp) => *comp,
            None => continue,
        };

        // If the competitor is ALSO in the user's primary stack, skip.
        // A Rust+Go developer should see Go content without penalty.
        if user_primary_stack
            .iter()
            .any(|t| t.to_lowercase() == matched_competitor)
        {
            continue;
        }

        // If the user's own tech ALSO appears in title, it's comparative content — allow it.
        // "Tauri vs Electron" is fine, "Electron 30 released" is not.
        if has_word_boundary(&title_lower, &user_lower) {
            continue;
        }

        // --- Graduated penalty (user's tech NOT in title) ---

        // 1. User's tech appears in content body — partial comparative
        if has_word_boundary(&content_lower, &user_lower) {
            return 0.80;
        }

        // 2. Comparative/migration markers in title or early content — useful framing
        let comparative_markers = [
            "vs",
            "versus",
            "compared to",
            "comparison",
            "alternative",
            "alternatives",
            "migrate",
            "migrating",
            "migration",
            "switching from",
            "moving from",
            "moved from",
            "switch to",
            "benchmark",
        ];
        let is_comparative = comparative_markers.iter().any(|m| {
            has_word_boundary(&title_lower, m)
                || content_lower[..content_lower.floor_char_boundary(500)].contains(m)
        });
        if is_comparative {
            return 0.85;
        }

        // 3. Competitor only in topics, not in title — incidental mention
        if !has_word_boundary(&title_lower, matched_competitor) {
            return 0.70;
        }

        // 4. Pure competing content — competitor in title, no user tech anywhere
        return 0.50;
    }

    1.0
}

/// Get the set of technologies that compete with the user's primary stack.
/// Used by knowledge gap filtering to avoid showing gaps for competing tech.
pub fn get_anti_dependencies(primary_stack: &HashSet<String>) -> HashSet<String> {
    let mut anti = HashSet::new();
    for user_tech in primary_stack {
        let user_lower = user_tech.to_lowercase();
        if let Some((_, competitors)) = COMPETING_TECH
            .iter()
            .find(|(tech, _)| *tech == user_lower.as_str())
        {
            for comp in *competitors {
                anti.insert(comp.to_string());
            }
        }
    }
    anti
}

/// Check if `text` contains `term` at a word boundary
fn has_word_boundary(text: &str, term: &str) -> bool {
    let mut search_from = 0;
    while let Some(pos) = text[search_from..].find(term) {
        let abs_pos = search_from + pos;
        let before_ok = abs_pos == 0 || !text.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let after_pos = abs_pos + term.len();
        let after_ok =
            after_pos >= text.len() || !text.as_bytes()[after_pos].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        search_from = abs_pos + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn topics(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_electron_penalized_for_tauri_user() {
        let primary = stack(&["tauri", "rust", "react"]);
        let mult = compute_competing_penalty(
            &topics(&["electron", "desktop"]),
            "Electron 30 Released with Performance Improvements",
            "",
            &primary,
        );
        assert_eq!(mult, 0.5);
    }

    #[test]
    fn test_comparative_content_allowed() {
        let primary = stack(&["tauri", "rust"]);
        let mult = compute_competing_penalty(
            &topics(&["tauri", "electron"]),
            "Tauri vs Electron: Which Desktop Framework to Choose in 2025",
            "",
            &primary,
        );
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn test_no_penalty_for_own_tech() {
        let primary = stack(&["react", "typescript"]);
        let mult = compute_competing_penalty(
            &topics(&["react", "hooks"]),
            "React 20 Introduces New Server Components API",
            "",
            &primary,
        );
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn test_vue_penalized_for_react_user() {
        let primary = stack(&["react"]);
        let mult = compute_competing_penalty(
            &topics(&["vue", "frontend"]),
            "Vue 4 Beta: Composition API Improvements",
            "",
            &primary,
        );
        assert_eq!(mult, 0.5);
    }

    #[test]
    fn test_competing_backend_framework_penalized() {
        // Django competes with Rust backends — Rust developer doesn't need Django content
        let primary = stack(&["rust", "tauri"]);
        let mult = compute_competing_penalty(
            &topics(&["python", "django"]),
            "Django 6.0 Released",
            "",
            &primary,
        );
        assert_eq!(mult, 0.5);
    }

    #[test]
    fn test_truly_unrelated_tech_no_penalty() {
        // Kubernetes is unrelated (not a competing backend framework), no penalty
        let primary = stack(&["rust", "tauri"]);
        let mult = compute_competing_penalty(
            &topics(&["kubernetes", "docker"]),
            "Kubernetes 1.31 Released",
            "",
            &primary,
        );
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn test_get_anti_dependencies() {
        let primary = stack(&["tauri", "react", "pnpm"]);
        let anti = get_anti_dependencies(&primary);
        assert!(anti.contains("electron"));
        assert!(anti.contains("vue"));
        assert!(anti.contains("npm"));
        assert!(anti.contains("yarn"));
        assert!(!anti.contains("tauri"));
        assert!(!anti.contains("react"));
    }

    #[test]
    fn test_empty_stack_no_penalty() {
        let primary = stack(&[]);
        let mult =
            compute_competing_penalty(&topics(&["electron"]), "Electron Released", "", &primary);
        assert_eq!(mult, 1.0);
    }

    // --- Graduated penalty tests ---

    #[test]
    fn test_user_tech_in_content_mild_penalty() {
        let primary = stack(&["react"]);
        let mult = compute_competing_penalty(
            &topics(&["vue"]),
            "Vue 4 Composition API Deep Dive",
            "...compared to React hooks, Vue's composition API...",
            &primary,
        );
        assert!(
            (mult - 0.80).abs() < 0.01,
            "user tech in content should give mild penalty, got {mult}"
        );
    }

    #[test]
    fn test_comparative_article_very_mild() {
        let primary = stack(&["tauri"]);
        let mult = compute_competing_penalty(
            &topics(&["electron"]),
            "Electron vs Tauri: Desktop Framework Comparison 2025",
            "We benchmark both frameworks...",
            &primary,
        );
        assert_eq!(mult, 1.0, "both techs in title should exempt completely");
    }

    #[test]
    fn test_comparative_markers_without_user_tech() {
        let primary = stack(&["react"]);
        let mult = compute_competing_penalty(
            &topics(&["vue", "angular"]),
            "Comparing Vue and Angular for Enterprise Apps",
            "When migrating from React to Vue, consider...",
            &primary,
        );
        assert!(
            (mult - 0.80).abs() < 0.01,
            "user tech in content gives 0.80, got {mult}"
        );
    }

    #[test]
    fn test_incidental_topic_mention() {
        let primary = stack(&["react"]);
        let mult = compute_competing_penalty(
            &topics(&["vue", "state-management"]),
            "Advanced State Management Patterns for Frontend Apps",
            "This article covers state management approaches across frameworks.",
            &primary,
        );
        assert!(
            (mult - 0.70).abs() < 0.01,
            "competitor only in topics should give 0.70, got {mult}"
        );
    }

    #[test]
    fn test_pure_competing_still_050() {
        let primary = stack(&["tauri", "rust"]);
        let mult = compute_competing_penalty(
            &topics(&["django", "python"]),
            "Django 6.0 Released with Major Performance Improvements",
            "Django 6.0 brings significant performance improvements to Python web development.",
            &primary,
        );
        assert_eq!(mult, 0.5, "pure competing content should still be 0.5");
    }

    #[test]
    fn test_multibyte_content_does_not_panic() {
        let primary = stack(&["react"]);
        // Content with curly quotes, em-dashes, and emoji — multi-byte UTF-8
        // that would panic if sliced at arbitrary byte boundaries.
        let content = "it\u{2019}s a small spring boot security SDK called \u{201c}vault SDK\u{201d}. \
            the basic idea is that you can add it to another spring boot project as a dependency \u{2014} \
            configure it, and it handles auth for you \u{2728}. "
            .repeat(20);
        assert!(content.len() > 2000);
        let mult = compute_competing_penalty(
            &topics(&["spring"]),
            "Spring Boot Security SDK",
            &content,
            &primary,
        );
        assert!(mult <= 1.0 && mult >= 0.0);
    }
}
