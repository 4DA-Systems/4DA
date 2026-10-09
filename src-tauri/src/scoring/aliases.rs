// SPDX-License-Identifier: FSL-1.1-Apache-2.0

/// Technology synonym and alias database for keyword matching.
///
/// Maps canonical names to all known variants. Bidirectional: given any
/// variant, returns all siblings in the alias group. Used to expand
/// keyword matching so "TypeScript" matches content mentioning "ts".
use std::collections::HashMap;
use std::sync::LazyLock;

/// Given a term, returns all known aliases (including the term itself).
/// Returns None if the term has no alias group.
pub(crate) fn get_aliases(term: &str) -> Option<&'static [&'static str]> {
    ALIAS_INDEX.get(term.to_lowercase().as_str()).copied()
}

/// Check if two terms are aliases of each other.
pub(crate) fn are_aliases(a: &str, b: &str) -> bool {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    if a_lower == b_lower {
        return true;
    }
    match ALIAS_INDEX.get(a_lower.as_str()) {
        Some(group) => group.iter().any(|t| *t == b_lower),
        None => false,
    }
}

/// The canonical name (the group's first entry) for a SPELLING of a known
/// technology: case, dots, hyphens and spaces ignored, so `Next.js`, `nextjs`,
/// `next`, `node.js`, `vue.js` and `react native` resolve to `nextjs`,
/// `nodejs`, `vue` and `react-native`.
///
/// Only spellings fold, not every alias: a member counts as a spelling when
/// its separator-free form and the canonical's are prefix-related (`next` ~
/// `nextjs`, `go` ~ `golang`, `c++` ~ `cpp`, `postgres` ~ `postgresql`). The
/// groups also hold looser equivalences that are right for widening a keyword
/// match but wrong for deciding two topics are ONE subject: `container` is not
/// Docker, `claude` is not Anthropic, `compose` is not Jetpack Compose,
/// `mariadb` is not MySQL. Those return `None` and stay their own topic.
pub(crate) fn canonical_spelling(term: &str) -> Option<&'static str> {
    let lower = term.trim().to_lowercase();
    let compact = compact_spelling(&lower);
    let group = ALIAS_INDEX
        .get(lower.as_str())
        .or_else(|| COMPACT_INDEX.get(compact.as_str()))?;
    let canonical = *group.first()?;
    let canonical_compact = compact_spelling(canonical);
    let is_spelling = !compact.is_empty()
        && (canonical_compact.starts_with(&compact) || compact.starts_with(&canonical_compact));
    is_spelling.then_some(canonical)
}

/// Lowercase alphanumerics only: `Next.js` -> `nextjs`, `react native` ->
/// `reactnative`.
pub(crate) fn compact_spelling(term: &str) -> String {
    term.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

// ---------------------------------------------------------------------------
// Alias groups — each array is a group of equivalent terms
// ---------------------------------------------------------------------------

const GROUPS: &[&[&str]] = &[
    // Languages
    &["javascript", "js", "ecmascript"],
    &["typescript", "ts"],
    &["python", "py", "python3"],
    &["golang", "go"],
    &["rust", "rust-lang", "rustlang"],
    &["cpp", "c++", "cxx", "cplusplus"],
    &["csharp", "c#", "dotnet", ".net"],
    &["ruby", "rb"],
    &["kotlin", "kt"],
    &["swift"],
    &["java"],
    &["php"],
    &["scala"],
    &["elixir", "ex"],
    &["haskell", "hs"],
    &["clojure", "clj"],
    &["lua"],
    &["zig"],
    &["nim"],
    &["dart"],
    &["r", "rlang"],
    // Frontend frameworks
    &["nodejs", "node.js", "node"],
    &["react", "reactjs", "react.js"],
    &["nextjs", "next.js", "next"],
    &["vue", "vuejs", "vue.js"],
    &["angular", "angularjs", "ng"],
    &["svelte", "sveltekit"],
    &["solid", "solidjs", "solid.js"],
    &["remix", "remixjs"],
    &["nuxt", "nuxtjs", "nuxt.js"],
    &["astro", "astrojs"],
    &["htmx"],
    // Backend frameworks
    &["express", "expressjs"],
    &["fastify"],
    &["django"],
    &["flask"],
    &["fastapi"],
    &["rails", "ruby-on-rails", "ror"],
    &["spring", "spring-boot", "springboot"],
    &["laravel"],
    &["phoenix"],
    &["actix", "actix-web"],
    &["axum"],
    &["gin"],
    &["echo"],
    &["fiber"],
    &["prisma"],
    &["drizzle", "drizzle-orm"],
    &["tokio"],
    &["warp"],
    // Desktop/mobile
    &["tauri"],
    &["electron", "electronjs"],
    &["react-native", "reactnative", "rn"],
    &["flutter"],
    &["swiftui"],
    &["jetpack-compose", "compose"],
    // Build tools
    &["webpack"],
    &["vite", "vitejs"],
    &["esbuild"],
    &["rollup"],
    &["turbopack"],
    &["bun"],
    // Runtimes
    &["deno"],
    // Databases
    &["postgresql", "postgres", "pg"],
    &["mysql", "mariadb"],
    &["mongodb", "mongo"],
    &["redis"],
    &["sqlite", "sqlite3"],
    &["dynamodb", "dynamo"],
    &["cassandra"],
    &["elasticsearch", "elastic", "es"],
    &["clickhouse"],
    &["supabase"],
    &["planetscale"],
    &["neon"],
    // Infrastructure
    &["docker", "dockerfile", "container", "containerization"],
    &["kubernetes", "k8s", "kube"],
    &["terraform", "tf"],
    &["ansible"],
    &["pulumi"],
    &["helm"],
    &["nginx"],
    &["caddy"],
    // Cloud
    &["aws", "amazon-web-services"],
    &["gcp", "google-cloud", "google-cloud-platform"],
    &["azure", "microsoft-azure"],
    &["vercel"],
    &["netlify"],
    &["cloudflare", "cf"],
    &["flyio", "fly.io", "fly"],
    // Protocols & APIs
    &["graphql", "gql"],
    &["rest", "restful", "rest-api"],
    &["grpc", "g-rpc"],
    &["websocket", "ws", "websockets"],
    &["trpc"],
    // AI/ML
    &["machine-learning", "ml", "machinelearning"],
    &["deep-learning", "dl", "deeplearning"],
    &["llm", "large-language-model", "large-language-models"],
    &["nlp", "natural-language-processing"],
    &["computer-vision", "cv"],
    &["rag", "retrieval-augmented-generation"],
    &["transformers", "transformer"],
    &["pytorch", "torch"],
    &["tensorflow", "tf-ml"],
    &["langchain"],
    &["llamaindex", "llama-index"],
    &["openai"],
    &["anthropic", "claude"],
    &["ollama"],
    &["github", "gh"],
    &["huggingface", "hugging-face", "hf"],
    // DevOps concepts
    &["cicd", "ci-cd", "ci/cd", "continuous-integration"],
    &["devops", "dev-ops"],
    &["sre", "site-reliability-engineering"],
    &["gitops"],
    &["infrastructure-as-code", "iac"],
    // Architecture concepts
    &["frontend", "front-end", "client-side"],
    &["backend", "back-end", "server-side"],
    &["fullstack", "full-stack"],
    &["microservices", "microservice", "micro-service"],
    &["serverless", "faas", "function-as-a-service"],
    &["monorepo", "mono-repo"],
    &["event-driven", "event-driven-architecture", "eda"],
    // Web standards
    &["webassembly", "wasm", "web-assembly"],
    &["webgpu"],
    &["webgl"],
    &["service-worker", "sw"],
    &["pwa", "progressive-web-app"],
    &["spa", "single-page-application"],
    &["ssr", "server-side-rendering"],
    &["ssg", "static-site-generation"],
    // Security
    &["oauth", "oauth2", "open-authorization"],
    &["jwt", "json-web-token"],
    &["rbac", "role-based-access-control"],
    &["cors", "cross-origin-resource-sharing"],
    &["xss", "cross-site-scripting"],
    &["csrf", "cross-site-request-forgery"],
    &["owasp"],
    &["zero-trust"],
    // Testing
    &["tdd", "test-driven-development"],
    &["bdd", "behavior-driven-development"],
    &["e2e", "end-to-end", "end-to-end-testing"],
    &["vitest"],
    &["jest"],
    &["playwright"],
    &["cypress"],
    &["selenium"],
    // Patterns
    &["ddd", "domain-driven-design"],
    &["cqrs", "command-query-responsibility-segregation"],
    &["orm", "object-relational-mapping"],
    // Package/protocol
    &["npm", "npmjs"],
    &["pnpm"],
    &["yarn"],
    &["cargo"],
    &["pip", "pypi"],
    &["mcp", "model-context-protocol"],
    &["lsp", "language-server-protocol"],
    // Data
    &["etl", "extract-transform-load"],
    &["sql", "structured-query-language"],
    &["nosql", "non-relational"],
    &["api", "application-programming-interface"],
    &["sdk", "software-development-kit"],
];

// ---------------------------------------------------------------------------
// Index: lowercase term → slice of all terms in its group
// ---------------------------------------------------------------------------

static ALIAS_INDEX: LazyLock<HashMap<&'static str, &'static [&'static str]>> =
    LazyLock::new(|| {
        let mut map = HashMap::with_capacity(GROUPS.len() * 3);
        for group in GROUPS {
            for &term in *group {
                map.insert(term, *group);
            }
        }
        map
    });

/// Separator-free spelling -> group, so `react native` and `Node.JS` find
/// their group. First group wins on a collision (none today).
static COMPACT_INDEX: LazyLock<HashMap<String, &'static [&'static str]>> = LazyLock::new(|| {
    let mut map = HashMap::with_capacity(GROUPS.len() * 3);
    for group in GROUPS {
        for &term in *group {
            let compact = compact_spelling(term);
            if !compact.is_empty() {
                map.entry(compact).or_insert(*group);
            }
        }
    }
    map
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_aliases() {
        assert!(are_aliases("TypeScript", "ts"));
        assert!(are_aliases("JavaScript", "js"));
        assert!(are_aliases("Python", "py"));
        assert!(are_aliases("Go", "golang"));
        assert!(are_aliases("Rust", "rust-lang"));
    }

    #[test]
    fn test_framework_aliases() {
        assert!(are_aliases("react", "reactjs"));
        assert!(are_aliases("next", "nextjs"));
        assert!(are_aliases("vue", "vuejs"));
        assert!(are_aliases("kubernetes", "k8s"));
    }

    #[test]
    fn test_concept_aliases() {
        assert!(are_aliases("frontend", "front-end"));
        assert!(are_aliases("frontend", "client-side"));
        assert!(are_aliases("backend", "server-side"));
        assert!(are_aliases("ml", "machine-learning"));
        assert!(are_aliases("llm", "large-language-model"));
    }

    #[test]
    fn test_no_cross_contamination() {
        assert!(!are_aliases("rust", "ruby"));
        assert!(!are_aliases("python", "javascript"));
        assert!(!are_aliases("docker", "kubernetes"));
        // "go" and "golang" ARE aliases — verify they work
        assert!(are_aliases("go", "golang"));
    }

    #[test]
    fn test_get_aliases_returns_group() {
        let group = get_aliases("k8s").unwrap();
        assert!(group.contains(&"kubernetes"));
        assert!(group.contains(&"k8s"));
        assert!(group.contains(&"kube"));
    }

    #[test]
    fn test_unknown_term() {
        assert!(get_aliases("xyzzy_unknown").is_none());
    }

    #[test]
    fn canonical_spelling_folds_spellings_of_one_technology() {
        for (spelling, canonical) in [
            ("next", "nextjs"),
            ("Next.js", "nextjs"),
            ("nextjs", "nextjs"),
            ("next-js", "nextjs"),
            ("node", "nodejs"),
            ("Node.js", "nodejs"),
            ("vue.js", "vue"),
            ("VueJS", "vue"),
            ("react native", "react-native"),
            ("ReactNative", "react-native"),
            ("go", "golang"),
            ("c++", "cpp"),
            ("postgres", "postgresql"),
        ] {
            assert_eq!(
                canonical_spelling(spelling),
                Some(canonical),
                "{spelling:?} should fold to {canonical:?}"
            );
        }
    }

    /// Looser alias-group members are a different subject, not a spelling.
    #[test]
    fn canonical_spelling_never_folds_a_different_subject() {
        for term in ["container", "claude", "compose", "mariadb", "pg", "xyzzy"] {
            assert_eq!(canonical_spelling(term), None, "{term:?} must not fold");
        }
    }

    #[test]
    fn test_case_insensitive() {
        assert!(are_aliases("TYPESCRIPT", "TS"));
        assert!(are_aliases("Docker", "dockerfile"));
    }
}
