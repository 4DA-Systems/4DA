//! Domain Profile — Technology Identity Model for 4DA
//!
//! Builds a graduated understanding of the user's technology identity from:
//! - Onboarding tech stack (primary_stack)
//! - ACE-detected tech (detected_tech)
//! - Project dependencies (dependency graph)
//! - Declared interests (interests)
//!
//! Used by scoring to compute graduated domain relevance (not just binary on/off domain).

use std::collections::{HashMap, HashSet};

/// User's technology identity — what they work with and care about
#[derive(Debug, Clone)]
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
}

impl Default for DomainProfile {
    fn default() -> Self {
        Self {
            primary_stack: HashSet::new(),
            adjacent_tech: HashSet::new(),
            all_tech: HashSet::new(),
            dependency_names: HashSet::new(),
            interest_topics: HashSet::new(),
        }
    }
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
    if let Ok(mut stmt) = conn.prepare("SELECT technology FROM tech_stack") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for tech in rows.flatten() {
                let lower = tech.to_lowercase();
                primary_stack.insert(lower.clone());
                all_tech.insert(lower);
            }
        }
    }

    // 2. ACE-detected tech (secondary — auto-scanned)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT name FROM detected_tech WHERE category IN ('Language', 'Framework', 'Database', 'Library') AND confidence >= 0.5",
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

    DomainProfile {
        primary_stack,
        adjacent_tech,
        all_tech,
        dependency_names,
        interest_topics,
    }
}

/// Compute graduated domain relevance for a set of topics against the profile.
/// Returns 0.0 (completely off-domain) to 1.0 (direct primary stack match).
///
/// Scoring tiers:
///   1.0 — topic matches primary stack
///   0.85 — topic matches a project dependency
///   0.70 — topic matches detected/adjacent tech
///   0.50 — topic matches an interest
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
            best_relevance = best_relevance.max(0.85);
            continue;
        }

        // Check all tech (detected + adjacent)
        if profile.all_tech.iter().any(|t| fuzzy_tech_match(&lower, t)) {
            best_relevance = best_relevance.max(0.70);
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

    const UTILITY_DEPS: &[&str] = &[
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

    !UTILITY_DEPS.contains(&name)
}

/// Infer adjacent technologies from the primary tech stack.
fn infer_adjacent_tech(primary: &HashSet<String>) -> HashSet<String> {
    let mut adjacent = HashSet::new();

    let adjacency: HashMap<&str, &[&str]> = HashMap::from([
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
    ]);

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
}
