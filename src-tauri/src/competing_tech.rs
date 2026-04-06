//! Competing Technology Anti-Affinity for 4DA
//!
//! Detects when content is primarily about a technology that competes with
//! the user's chosen stack. Electron content for a Tauri developer, Vue content
//! for a React developer, etc.

use std::collections::HashSet;

/// Ecosystem map: (technology, &[its competitors])
const COMPETING_TECH: &[(&str, &[&str])] = &[
    // Desktop frameworks
    ("tauri", &["electron", "nwjs", "neutralino", "wails", "cef"]),
    ("electron", &["tauri", "nwjs", "neutralino", "wails"]),
    // Frontend frameworks (mild — cross-pollination has value)
    ("react", &["vue", "angular", "svelte", "solid", "qwik"]),
    ("vue", &["react", "angular", "svelte", "solid"]),
    ("angular", &["react", "vue", "svelte", "solid"]),
    ("svelte", &["react", "vue", "angular"]),
    // Backend frameworks — "React and Laravel" is not relevant if user doesn't use Laravel
    ("rust", &["django", "laravel", "rails", "spring", "flask", "gin", "echo", "fastapi"]),
    ("axum", &["express", "fastify", "koa", "hapi", "django", "laravel", "rails", "spring", "flask", "gin", "echo", "fastapi", "fiber"]),
    // Package managers
    ("pnpm", &["npm", "yarn"]),
    ("yarn", &["npm", "pnpm"]),
    // Runtimes
    ("deno", &["node", "bun"]),
    ("bun", &["node", "deno"]),
    // Databases (when used as primary)
    ("sqlite", &["mongodb", "dynamodb", "couchdb"]),
    ("postgresql", &["mysql", "mariadb"]),
    // Type systems
    ("typescript", &["flow", "rescript"]),
    // Backend languages (when article is about a different backend entirely)
    ("python", &["java", "csharp", "php", "ruby"]),
    ("java", &["python", "csharp", "php", "ruby"]),
];

/// Check if content is primarily about a competing technology.
/// Returns a multiplier: 1.0 (no competition) or 0.5 (competing tech dominant).
pub fn compute_competing_penalty(
    topics: &[String],
    title: &str,
    user_primary_stack: &HashSet<String>,
) -> f32 {
    let title_lower = title.to_lowercase();

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

        // Check if any competitor appears in the title or topics
        let has_competitor = competitors.iter().any(|comp| {
            has_word_boundary(&title_lower, comp)
                || topics
                    .iter()
                    .any(|t| t.to_lowercase() == *comp || t.to_lowercase().starts_with(comp))
        });

        if !has_competitor {
            continue;
        }

        // BUT: if the user's own tech ALSO appears, it's comparative content — allow it
        // "Tauri vs Electron" is fine, "Electron 30 released" is not
        if has_word_boundary(&title_lower, &user_lower) {
            continue;
        }

        // Competing tech is dominant (appears without user's tech) → penalty
        return 0.5;
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
        let mult = compute_competing_penalty(&topics(&["electron"]), "Electron Released", &primary);
        assert_eq!(mult, 1.0);
    }
}
