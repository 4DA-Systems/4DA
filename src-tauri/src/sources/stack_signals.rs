// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Stack-adaptive source signals.
//!
//! The content sources (Stack Overflow, Reddit, Bluesky, GitHub) historically fetched
//! against hard-coded query lists skewed to the founder's stack (rust/typescript/react/
//! python). That makes the firehose founder-shaped: a Go, Java, Ruby, PHP, or C# developer
//! sees mostly content for languages they don't use.
//!
//! This module reads the user's ACE-detected stack (`detected_tech`: languages + frameworks)
//! and maps it to per-source query sets, so the firehose becomes *user*-shaped. The precedence
//! the callers apply is: explicit user settings > ACE-detected stack > hard-coded defaults —
//! so when nothing is detected (a fresh install with no scan) every mapping returns empty and
//! the caller falls back to its existing default. No regression for the no-signal case.

/// The user's detected stack, distilled to the signals that shape content queries.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StackSignals {
    /// Normalized languages, highest-confidence first (e.g. `["go", "java"]`).
    pub languages: Vec<String>,
    /// Normalized frameworks/libraries, highest-confidence first (e.g. `["gin", "spring"]`).
    pub frameworks: Vec<String>,
}

/// Minimum `detected_tech.confidence` for a signal to shape queries.
const MIN_CONFIDENCE: f64 = 0.5;
const MAX_LANGUAGES: usize = 6;
const MAX_FRAMEWORKS: usize = 8;

/// Read the ACE-detected stack from `detected_tech`. Returns empty signals (never errors)
/// when there is no DB, no scan, or nothing above the confidence floor.
pub fn detect() -> StackSignals {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return StackSignals::default(),
    };
    detect_from_conn(&conn)
}

/// Testable core: build signals from an open connection.
pub(crate) fn detect_from_conn(conn: &rusqlite::Connection) -> StackSignals {
    let mut stmt = match conn.prepare(
        "SELECT name, category FROM detected_tech \
         WHERE category IN ('language', 'framework') AND confidence >= ?1 \
         ORDER BY confidence DESC, name ASC",
    ) {
        Ok(s) => s,
        Err(_) => return StackSignals::default(),
    };
    let rows = match stmt.query_map([MIN_CONFIDENCE], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(r) => r,
        Err(_) => return StackSignals::default(),
    };

    let mut signals = StackSignals::default();
    for (name, category) in rows.flatten() {
        let norm = normalize_tech(&name);
        if norm.is_empty() {
            continue;
        }
        match category.as_str() {
            "language" => {
                if signals.languages.len() < MAX_LANGUAGES && !signals.languages.contains(&norm) {
                    signals.languages.push(norm);
                }
            }
            "framework" => {
                if signals.frameworks.len() < MAX_FRAMEWORKS && !signals.frameworks.contains(&norm)
                {
                    signals.frameworks.push(norm);
                }
            }
            _ => {}
        }
    }
    signals
}

/// Normalize a `detected_tech` name to a canonical lowercase token used by the maps below.
fn normalize_tech(raw: &str) -> String {
    let t = raw.trim().to_lowercase();
    match t.as_str() {
        "c#" | "c sharp" | "csharp" => "csharp".to_string(),
        ".net" | "dotnet" | "asp.net" => "dotnet".to_string(),
        "c++" | "cpp" => "cpp".to_string(),
        "golang" => "go".to_string(),
        "js" | "node.js" | "nodejs" => {
            // node.js as a *language* signal collapses to javascript; as a framework it maps below.
            "javascript".to_string()
        }
        "ts" => "typescript".to_string(),
        "py" => "python".to_string(),
        "objective-c" | "objc" => "objective-c".to_string(),
        other => other.to_string(),
    }
}

/// Look a key up in a (key, value) table; `None` if absent.
fn lookup<'a>(table: &'a [(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

// ---------------------------------------------------------------------------
// Per-source mappings. Each returns an EMPTY vec when there are no signals, so
// the caller falls back to its hard-coded default (no regression on cold/no-scan).
// ---------------------------------------------------------------------------

/// Language/framework -> Stack Overflow tag (SO tag spelling differs from our tokens).
const SO_LANG_TAGS: &[(&str, &str)] = &[
    ("csharp", "c#"),
    ("cpp", "c++"),
    ("dotnet", ".net"),
    ("objective-c", "objective-c"),
];
const SO_FRAMEWORK_TAGS: &[(&str, &str)] = &[
    ("react", "reactjs"),
    ("vue", "vue.js"),
    ("nextjs", "next.js"),
    ("rails", "ruby-on-rails"),
    ("node", "node.js"),
    ("dotnet", ".net"),
];

/// Language -> subreddit (only entries known to exist, to avoid 404 noise).
const REDDIT_LANG_SUBS: &[(&str, &str)] = &[
    ("rust", "rust"),
    ("go", "golang"),
    ("python", "python"),
    ("java", "java"),
    ("javascript", "javascript"),
    ("typescript", "typescript"),
    ("csharp", "csharp"),
    ("dotnet", "dotnet"),
    ("cpp", "cpp"),
    ("ruby", "ruby"),
    ("php", "PHP"),
    ("swift", "swift"),
    ("kotlin", "Kotlin"),
    ("scala", "scala"),
    ("elixir", "elixir"),
    ("haskell", "haskell"),
    ("dart", "dartlang"),
];
const REDDIT_FRAMEWORK_SUBS: &[(&str, &str)] = &[
    ("react", "reactjs"),
    ("vue", "vuejs"),
    ("angular", "angular"),
    ("django", "django"),
    ("flask", "flask"),
    ("rails", "rails"),
    ("laravel", "laravel"),
    ("nextjs", "nextjs"),
    ("svelte", "sveltejs"),
    ("flutter", "FlutterDev"),
    ("spring", "java"),
];

/// Language -> GitHub language-search name.
const GH_LANG: &[(&str, &str)] = &[("csharp", "c#"), ("cpp", "c++"), ("dotnet", "c#")];

impl StackSignals {
    pub fn is_empty(&self) -> bool {
        self.languages.is_empty() && self.frameworks.is_empty()
    }

    /// Stack Overflow tags for the detected stack (languages + frameworks). Empty if no signals.
    pub fn stackoverflow_tags(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for l in &self.languages {
            out.push(lookup(SO_LANG_TAGS, l).unwrap_or(l).to_string());
        }
        for f in &self.frameworks {
            out.push(lookup(SO_FRAMEWORK_TAGS, f).unwrap_or(f).to_string());
        }
        dedup(out, 6)
    }

    /// Subreddits for the detected stack. Anchored with a couple of generic dev subs so the
    /// feed always has breadth, then the stack-specific ones. Empty if no signals.
    pub fn reddit_subreddits(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for l in &self.languages {
            if let Some(s) = lookup(REDDIT_LANG_SUBS, l) {
                out.push(s.to_string());
            }
        }
        for f in &self.frameworks {
            if let Some(s) = lookup(REDDIT_FRAMEWORK_SUBS, f) {
                out.push(s.to_string());
            }
        }
        if out.is_empty() {
            return out; // no recognized signal -> caller uses default
        }
        // Generic anchors for breadth (deduped below).
        out.push("programming".to_string());
        out.push("technology".to_string());
        dedup(out, 8)
    }

    /// Bluesky search queries for the detected stack. Empty if no signals.
    pub fn bluesky_queries(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for l in &self.languages {
            let term = match l.as_str() {
                "csharp" => "c#".to_string(),
                "cpp" => "c++".to_string(),
                other => other.to_string(),
            };
            out.push(format!("{term} programming"));
        }
        for f in &self.frameworks {
            out.push(f.clone());
        }
        dedup(out, 6)
    }

    /// GitHub language-search names for the detected stack. Empty if no language signals.
    pub fn github_languages(&self) -> Vec<String> {
        let out: Vec<String> = self
            .languages
            .iter()
            .map(|l| lookup(GH_LANG, l).unwrap_or(l).to_string())
            .collect();
        dedup(out, 6)
    }
}

/// Order-preserving dedup, capped to `max`.
fn dedup(items: Vec<String>, max: usize) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for it in items {
        let key = it.to_lowercase();
        if seen.insert(key) {
            out.push(it);
            if out.len() >= max {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals(langs: &[&str], fws: &[&str]) -> StackSignals {
        StackSignals {
            languages: langs.iter().map(|s| s.to_string()).collect(),
            frameworks: fws.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn empty_signals_yield_empty_queries_so_caller_falls_back() {
        let s = StackSignals::default();
        assert!(s.is_empty());
        assert!(s.stackoverflow_tags().is_empty());
        assert!(s.reddit_subreddits().is_empty());
        assert!(s.bluesky_queries().is_empty());
        assert!(s.github_languages().is_empty());
    }

    #[test]
    fn java_dev_gets_java_not_rust() {
        let s = signals(&["java"], &["spring"]);
        let so = s.stackoverflow_tags();
        assert!(so.contains(&"java".to_string()));
        assert!(so.contains(&"spring".to_string()));
        let subs = s.reddit_subreddits();
        assert!(subs.contains(&"java".to_string()));
        assert!(!subs.contains(&"rust".to_string()));
        assert!(s.github_languages().contains(&"java".to_string()));
    }

    #[test]
    fn csharp_maps_to_so_and_github_spellings() {
        let s = signals(&["csharp"], &[]);
        assert!(s.stackoverflow_tags().contains(&"c#".to_string()));
        assert!(s.github_languages().contains(&"c#".to_string()));
        assert!(s.reddit_subreddits().contains(&"csharp".to_string()));
        assert!(s.bluesky_queries().contains(&"c# programming".to_string()));
    }

    #[test]
    fn go_normalizes_and_maps_to_golang_subreddit() {
        let s = signals(&["go"], &[]);
        assert!(s.reddit_subreddits().contains(&"golang".to_string()));
        assert!(s.stackoverflow_tags().contains(&"go".to_string()));
    }

    #[test]
    fn unknown_language_still_adapts_so_and_github_but_skips_unknown_subreddit() {
        let s = signals(&["zig"], &[]);
        // SO + GitHub use identity fallback -> still user-shaped, not founder-shaped.
        assert!(s.stackoverflow_tags().contains(&"zig".to_string()));
        assert!(s.github_languages().contains(&"zig".to_string()));
        // Subreddit map is curated to known subs; an unknown one yields no stack subs
        // (so the caller falls back rather than 404 on a guessed subreddit).
        assert!(s.reddit_subreddits().is_empty());
    }

    #[test]
    fn normalize_handles_common_aliases() {
        assert_eq!(normalize_tech("Golang"), "go");
        assert_eq!(normalize_tech("C#"), "csharp");
        assert_eq!(normalize_tech("C++"), "cpp");
        assert_eq!(normalize_tech(".NET"), "dotnet");
        assert_eq!(normalize_tech("TypeScript"), "typescript");
    }

    #[test]
    fn reddit_anchors_breadth_when_stack_recognized() {
        let s = signals(&["ruby"], &["rails"]);
        let subs = s.reddit_subreddits();
        assert!(subs.contains(&"ruby".to_string()));
        assert!(subs.contains(&"rails".to_string()));
        assert!(subs.contains(&"programming".to_string()));
    }
}
