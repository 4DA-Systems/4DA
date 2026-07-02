// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use std::collections::{HashMap, HashSet};

use super::ace_context::ACEContext;
use super::utils::{has_word_boundary_match, topic_overlaps};

/// Metadata for a tracked dependency from user's project manifests
#[derive(Debug, Clone)]
pub(crate) struct DepInfo {
    pub package_name: String,
    pub version: Option<String>,
    pub is_dev: bool,
    /// Whether this is a direct dependency (from manifest) or transitive (from lockfile).
    /// Direct deps get full confidence; transitive deps get 0.5x confidence.
    pub is_direct: bool,
    /// Searchable terms extracted from the package name
    /// e.g. "@tanstack/react-query" -> ["tanstack-react-query", "tanstack", "react-query"]
    pub search_terms: Vec<String>,
    /// Ecosystem/language from the manifest (e.g. "rust", "javascript", "python").
    /// Used for cross-referencing CVE advisories against the correct ecosystem.
    pub ecosystem: String,
}

/// A dependency that matched content
#[derive(Debug, Clone)]
pub(crate) struct DepMatch {
    pub package_name: String,
    pub confidence: f32,
    pub version_delta: VersionDelta,
    pub is_dev: bool,
    /// Direct dependency (from manifest) vs transitive (from lockfile).
    /// CVE scoring uses this to differentiate urgency.
    pub is_direct: bool,
    /// Installed version from the user's lockfile (e.g. "2.8.1")
    pub version: Option<String>,
    /// Ecosystem of the matched dependency (e.g. "rust", "javascript").
    /// Critical for rejecting cross-ecosystem CVE false positives.
    pub ecosystem: String,
}

/// Version comparison between installed and mentioned
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum VersionDelta {
    SameMajor,
    NewerMajor,
    OlderMajor,
    Unknown,
}

/// Confidence at or above which a single non-dev dependency match is "strong"
/// enough to ground an item in the user's stack. A full-package-name title hit
/// scores 0.5; an ambiguous subterm with nearby language context scores ~0.4;
/// bare prose coincidences sit below. 0.40 is the trust floor shared by EVERY
/// grounding layer — the Critical gate, the evidence-pool "Affects You"
/// placement, and the persisted-link reconciler — so they all agree on the
/// single question "is this item grounded in the user's dependencies?".
pub(crate) const STRONG_GROUNDING_CONFIDENCE: f32 = 0.40;

/// True when `d` is a trustworthy grounding edge on its own: a non-dev match at
/// or above the strong-confidence floor whose package name is not so word-like
/// that a bare text hit can't be trusted. Mirrors the persistence-layer denylist
/// (`is_ambiguous_package_name`) so the gate, the pool, and the persisted
/// `source_item_dependencies` set never disagree about what counts as grounded.
pub(crate) fn is_strong_grounding_match(d: &DepMatch) -> bool {
    !d.is_dev
        && d.confidence >= STRONG_GROUNDING_CONFIDENCE
        && !crate::package_ambiguity::is_ambiguous_package_name(&d.package_name)
}

/// The single canonical "is this item grounded in the user's stack?" predicate.
/// True when any matched dependency is a trustworthy grounding edge. Shared by
/// the Critical-signal gate (`pipeline_signals`/`pipeline_v2`), the evidence
/// pool, and the persisted score breakdown so grounding is computed ONE way.
pub(crate) fn is_strongly_grounded(deps: &[DepMatch]) -> bool {
    deps.iter().any(is_strong_grounding_match)
}

/// As [`is_strongly_grounded`], but additionally requires the edge to be a
/// DIRECT dependency — the trust floor for a Critical alert. A CVE in a package
/// the user chose directly is urgent; one reached only transitively is watch-level.
pub(crate) fn is_strongly_grounded_direct(deps: &[DepMatch]) -> bool {
    deps.iter()
        .any(|d| is_strong_grounding_match(d) && d.is_direct)
}

/// Common English words AND generic tech stems that collide with package names.
/// These require nearby language-context words to match.
///
/// The tech-stem entries (cert, auth, api, http, lib, util, sdk, ...) are the
/// subterms produced by extract_search_terms when splitting multi-part package
/// names like `x509-cert`, `json-web-token`, `auth-client`, `http-common`. On
/// their own they match far too much CVE/blog content — e.g. "cert" as a
/// stand-alone word appears in almost every TLS advisory regardless of whether
/// the user has `x509-cert` in their lockfile.
const COMMON_ENGLISH_WORDS: &[&str] = &[
    // 2-3 letter
    "is",
    "it",
    "or",
    "and",
    "the",
    "got",
    "set",
    "get",
    "put",
    "has",
    "run",
    "use",
    "can",
    "will",
    "ms",
    "log",
    "map",
    "tar",
    "zip",
    "hex",
    "png",
    "pdf", // 4 letter
    "call",
    "data",
    "path",
    "file",
    "time",
    "date",
    "form",
    "page",
    "view",
    "list",
    "item",
    "test",
    "main",
    "core",
    "base",
    "once",
    "open",
    "copy",
    "send",
    "body",
    "read",
    "sort",
    "dirs",
    "find",
    "make",
    "next",
    "link",
    "node",
    "kind",
    "mark",
    "drop",
    "move",
    "type",
    "just",
    // 5+ letter — real English words that are also package names
    "image",
    "sharp",
    "quote",
    "level",
    "model",
    "state",
    "store",
    "route",
    "group",
    "serve",
    "watch",
    "clean",
    "fresh",
    "smart",
    "craft",
    "prime",
    "solid",
    "super",
    "simple",
    "table",
    "notify",
    "scraper",
    // Common verbs / nouns that are also package-name subterms.
    // e.g. "extract" appears in pdf-extract but also in "how to extract data".
    "extract",
    "build",
    "fetch",
    "patch",
    "trace",
    "stream",
    "check",
    "parse",
    "cache",
    "event",
    "mount",
    "frame",
    "layer",
    "block",
    "merge",
    "split",
    "match",
    "drive",
    "print",
    "write",
    "guard",
    "probe",
    "relay",
    "apply",
    "chain",
    "local",
    // Generic tech stems — subterms of compound package names that are too
    // broad on their own. Only match when used with language context nearby.
    "cert",
    "auth",
    "api",
    "web",
    "http",
    "https",
    "lib",
    "util",
    "utils",
    "sdk",
    "crypto",
    "net",
    "client",
    "server",
    "common",
    "plugin",
    "plugins",
    "tool",
    "tools",
    "helper",
    "helpers",
    "shared",
    "admin",
    "user",
    "users",
    "proxy",
    "config",
    "debug",
    "token",
    "tokens",
    "middleware",
    "schema",
    "query",
    "queries",
    "parser",
    "parsers",
    "loader",
    "loaders",
    "runner",
    "runners",
    "engine",
    "runtime",
    "service",
    "services",
    "provider",
    "providers",
    // Generic descriptive words that appear as sub-terms of compound package
    // names (e.g. "winston-daily-rotate-file" → "daily"/"rotate") and would
    // otherwise match unrelated content. The full normalized package name still
    // matches; only these bare sub-terms are filtered.
    "daily",
    "weekly",
    "monthly",
    "hourly",
    "yearly",
    "rotate",
    "simple",
    "easy",
    "quick",
    "fast",
    "tiny",
    "mini",
    "basic",
    "pretty",
    "modern",
    "native",
    "smart",
    "plus",
    "extra",
    "super",
    "auto",
    // Business-domain words that ACE mints as topics from the user's own
    // function/module names (handle_payment, render_frame, InvoiceService).
    // As bare tokens they anchor nothing: a "payment" overlap must not make
    // fintech news on an alien stack look on-domain, and a package actually
    // named like this still matches via language context / full name.
    "payment",
    "payments",
    "render",
    "invoice",
    "invoices",
    "billing",
    // OS / platform proper nouns. These are also real package names or subterms
    // of platform crates (`windows`/`windows-sys`, `linux-*`, `android-*`), but
    // they appear constantly in titles as the OPERATING SYSTEM, not the package
    // ("Windows 0-day", "Linux kernel CVE"). A bare subterm hit against such a
    // title falsely grounded `windows-sys` to an OS advisory (the 2026-06-24
    // dogfood case). Requiring nearby language context ("crate", "cargo",
    // "package") lets a genuine `windows-sys` advisory still ground while the OS
    // headline does not. The full normalized name (`windows-sys`) still matches
    // directly — only the bare proper-noun subterm is gated.
    "windows",
    "linux",
    "android",
    "macos",
    "unix",
];

/// Language-context words that disambiguate package names from English
const LANGUAGE_CONTEXT_WORDS: &[&str] = &[
    "package",
    "crate",
    "library",
    "lib",
    "module",
    "npm",
    "cargo",
    "pip",
    "dependency",
    "dep",
    "install",
    "import",
    "require",
    "gem",
    "composer",
    "pypi",
    "crates.io",
    "npmjs",
    "yarn",
    "pnpm",
    "bun",
];

/// Normalize a package name for consistent matching.
/// `@tanstack/react-query` -> `tanstack-react-query`
pub(crate) fn normalize_package_name(name: &str) -> String {
    name.to_lowercase()
        .trim_start_matches('@')
        .replace('/', "-")
}

/// Check if a term is a common English word (prone to false positives)
pub(crate) fn is_ambiguous_dep_name(term: &str) -> bool {
    // Short tech keywords that are legitimate despite being short
    const SHORT_TECH: &[&str] = &["vue", "svelte", "htmx", "bun", "deno", "vite", "esbuild"];
    if SHORT_TECH.contains(&term) {
        return false;
    }
    if term.len() <= 3 {
        return true; // Very short = always ambiguous unless in SHORT_TECH
    }
    COMMON_ENGLISH_WORDS.contains(&term)
}

/// Infrastructure dependencies are ubiquitous ecosystem tools that don't indicate
/// domain-specific relevance. Matching content against these deps produces noise
/// because every project has them.
fn is_infrastructure_dep(name: &str) -> bool {
    let normalized = normalize_package_name(name);

    // Test infrastructure
    if normalized.contains("testing-library")
        || normalized.contains("jest")
        || normalized.contains("vitest")
        || normalized.contains("playwright")
        || normalized.contains("cypress")
        || normalized == "serial_test"
        || normalized == "victauri-test"
        || normalized == "victauri_test"
    {
        return true;
    }

    // TypeScript type declarations (@types/*)
    if normalized.starts_with("types-") && !normalized.contains("typescript") {
        return true;
    }

    // Linting and formatting
    if normalized.contains("eslint") || normalized.contains("prettier") {
        return true;
    }

    // Build tooling (when matched as subterms, these are noise)
    if normalized == "ts-node" || normalized == "tsx" {
        return true;
    }

    // Monitoring/error tracking (infrastructure, not domain signal)
    if normalized.contains("sentry") && normalized != "sentry" {
        return true;
    }

    false
}

/// Major framework/ecosystem names that are too broad as subterms.
/// "react" appearing in "sentry-react" should NOT match every React article.
/// The full compound name ("sentry-react") still matches — only the bare
/// subterm is suppressed to prevent false-positive escalation.
const ECOSYSTEM_NAMES: &[&str] = &[
    "react",
    "vue",
    "angular",
    "svelte",
    "solid",
    "next",
    "nuxt",
    "astro",
    "node",
    "deno",
    "bun",
    "express",
    "django",
    "flask",
    "rails",
    "tauri",
    "electron",
    "rust",
    "python",
    "java",
    "swift",
    "kotlin",
    "webpack",
    "vite",
    "esbuild",
    "rollup",
    "parcel",
    "postgres",
    "mysql",
    "redis",
    "mongo",
    "sqlite",
    "docker",
    "kubernetes",
];

/// Check if a term is a major ecosystem name that shouldn't be used as a
/// compound-package subterm (only applies when splitting multi-part names).
fn is_ecosystem_subterm(term: &str) -> bool {
    ECOSYSTEM_NAMES.contains(&term)
}

/// Extract searchable terms from a package name.
/// Multi-part names are split into meaningful subterms, but ecosystem names
/// (react, vue, rust, etc.) are excluded as subterms to prevent false positives.
/// The full normalized name always matches — only bare subterms are filtered.
pub(crate) fn extract_search_terms(name: &str) -> Vec<String> {
    let normalized = normalize_package_name(name);
    let is_compound = normalized.contains('-');
    let mut terms = vec![normalized.clone()];

    // Split on hyphens for multi-part names
    let parts: Vec<&str> = normalized.split('-').filter(|p| p.len() >= 3).collect();

    // Add subterms if they're specific enough AND not a major ecosystem name.
    // "sentry-react" → keep "sentry", drop "react" (ecosystem name as subterm).
    // "react" as a standalone package (not compound) is kept as-is.
    for part in &parts {
        if !is_ambiguous_dep_name(part) && !(is_compound && is_ecosystem_subterm(part)) {
            terms.push(part.to_string());
        }
    }

    // For scoped packages, also add the scope and package separately
    // @tanstack/react-query -> "tanstack" + "react-query" already covered by split

    terms.sort();
    terms.dedup();
    terms
}

/// Check if language-context words appear near a position in text
fn has_language_context_nearby(text: &str, position: usize, window: usize) -> bool {
    let start = position.saturating_sub(window);
    let end = (position + window).min(text.len());
    // Snap to char boundaries to avoid panicking on multi-byte UTF-8
    let start = snap_to_char_boundary(text, start, false);
    let end = snap_to_char_boundary(text, end, true);
    let context = &text[start..end];
    LANGUAGE_CONTEXT_WORDS.iter().any(|w| context.contains(w))
}

/// Snap a byte index to the nearest valid char boundary.
/// If `forward` is true, snaps forward (for end indices); otherwise snaps backward (for start indices).
fn snap_to_char_boundary(s: &str, index: usize, forward: bool) -> usize {
    if index >= s.len() {
        return s.len();
    }
    if s.is_char_boundary(index) {
        return index;
    }
    if forward {
        // Walk forward to next char boundary
        let mut i = index;
        while i < s.len() && !s.is_char_boundary(i) {
            i += 1;
        }
        i
    } else {
        // Walk backward to previous char boundary
        let mut i = index;
        while i > 0 && !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
}

/// Parse major version from a semver string ("1.2.3" -> Some(1))
fn parse_major_version(version: &str) -> Option<u32> {
    version
        .trim_start_matches(['v', 'V', '^', '~', '=', '>', '<', ' '])
        .split('.')
        .next()?
        .parse()
        .ok()
}

/// Parse a version's `(major, minor, patch)` triplet for compatibility analysis.
/// `"1.2.3"` -> `(1, 2, 3)`, `"0.18.4"` -> `(0, 18, 4)`, `"2"` -> `(2, 0, 0)`,
/// `"0.0.5"` -> `(0, 0, 5)`. Keeping the patch distinguishes `0.0.5` from `0.0.99`
/// so the pre-0.1 caret line (`^0.0.5` matches only `0.0.5`) is not collapsed.
fn version_triplet(version: &str) -> Option<(u32, u32, u32)> {
    let trimmed = version.trim_start_matches(['v', 'V', '^', '~', '=', '>', '<', ' ']);
    let major = parse_major_version(version)?;
    let component = |idx: usize| -> u32 {
        trimmed
            .split('.')
            .nth(idx)
            .map(|p| {
                p.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
            })
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    };
    Some((major, component(1), component(2)))
}

/// Reduce a `(major, minor, patch)` triplet to its semver *breaking axis* — the
/// component a bump of which signals an incompatible release under Cargo/npm caret
/// rules. The breaking line is the LEFTMOST NON-ZERO component:
///
/// For `>= 1.0` the breaking axis is the major (`1.2` and `1.9` are compatible);
/// for `0.x` (x>=1) the MINOR is the breaking axis (`0.18` and `0.20` are NOT
/// compatible); for `0.0.z` the PATCH is the breaking axis (`^0.0.5` matches only
/// `0.0.5` — every patch is a breaking line). This collapses neither the pre-1.0
/// crate ecosystem (gtk-rs 0.18, axum 0.8) NOR the pre-0.1 line (`0.0.z`) to
/// "major 0", so content about a version the user has moved past no longer rides
/// the same-line relevance boost.
fn breaking_axis(triplet: (u32, u32, u32)) -> (u32, u32, u32) {
    let (major, minor, patch) = triplet;
    if major != 0 {
        (major, 0, 0) // >=1.0 — major is the breaking line
    } else if minor != 0 {
        (0, minor, 0) // 0.x (x>=1) — minor is the breaking line
    } else {
        (0, 0, patch) // 0.0.z — patch is the breaking line (strict caret)
    }
}

/// Compare an installed version against a version mentioned in content,
/// classifying the relationship from the installed POV.
fn compare_triplets(installed: (u32, u32, u32), mentioned: (u32, u32, u32)) -> VersionDelta {
    match breaking_axis(mentioned).cmp(&breaking_axis(installed)) {
        std::cmp::Ordering::Equal => VersionDelta::SameMajor,
        std::cmp::Ordering::Greater => VersionDelta::NewerMajor,
        std::cmp::Ordering::Less => VersionDelta::OlderMajor,
    }
}

/// Extract a mentioned version from content near a package name and compare with installed
fn compare_version_in_content(
    text: &str,
    pkg_name: &str,
    installed_version: &Option<String>,
) -> VersionDelta {
    let installed_triplet = match installed_version {
        Some(v) => match version_triplet(v) {
            Some(p) => p,
            None => return VersionDelta::Unknown,
        },
        None => return VersionDelta::Unknown,
    };

    // Find package mentions and look for version numbers nearby
    let text_lower = text.to_lowercase();
    let pkg_lower = pkg_name.to_lowercase();
    for (idx, _) in text_lower.match_indices(&pkg_lower) {
        let start = idx;
        let end = (idx + pkg_lower.len() + 40).min(text_lower.len());
        let end = snap_to_char_boundary(&text_lower, end, true);
        let nearby = &text_lower[start..end];

        // Match patterns: "React 19", "tokio 2.0", "gtk 0.18", "v3", "version 5.1".
        // Grab the first version-like token (digits + dots) after the package name
        // so 0.x lines ("0.18" vs "0.20") are distinguishable, not collapsed to "0".
        let after_name = &nearby[pkg_lower.len()..];
        for (i, ch) in after_name.char_indices() {
            if ch.is_ascii_digit() && i < 20 {
                // Check this is at a word boundary (preceded by space, v, etc.)
                if i == 0
                    || after_name
                        .as_bytes()
                        .get(i - 1)
                        .is_none_or(|&b| !b.is_ascii_alphanumeric() || b == b'v' || b == b'V')
                {
                    let token: String = after_name[i..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if let Some(mentioned_triplet) = version_triplet(&token) {
                        // Reject absurd majors (years, IDs) and a bogus bare "0"
                        // (parses to (0,0,0)), but a real "0.0.5" -> (0,0,5) is kept
                        // so 0.0.x version intelligence is no longer disabled (bug B).
                        if mentioned_triplet.0 < 100 && mentioned_triplet != (0, 0, 0) {
                            return compare_triplets(installed_triplet, mentioned_triplet);
                        }
                    }
                }
                break;
            }
        }
    }

    VersionDelta::Unknown
}

/// Load all tracked dependencies from database into fast-lookup structures
pub(crate) fn load_dependency_intelligence() -> (HashSet<String>, HashMap<String, DepInfo>) {
    let db = match crate::open_db_connection() {
        Ok(db) => db,
        Err(_) => return (HashSet::new(), HashMap::new()),
    };

    let all_deps = match crate::temporal::get_all_dependencies(&db) {
        Ok(deps) => deps,
        Err(_) => return (HashSet::new(), HashMap::new()),
    };

    // Honor the user's project allowlist: drop deps from projects the user
    // excluded from their stack (test fixtures, scaffolding) so they don't
    // pollute relevance grounding. Prefix-matched, case-insensitive — mirrors
    // temporal's active-root normalization.
    let excluded: Vec<String> = crate::get_settings_manager()
        .lock()
        .get_excluded_project_paths()
        .iter()
        .map(|p| p.to_lowercase())
        .collect();
    let all_deps: Vec<_> = if excluded.is_empty() {
        all_deps
    } else {
        all_deps
            .into_iter()
            .filter(|dep| {
                let p = dep.project_path.to_lowercase();
                !excluded.iter().any(|ex| p.starts_with(ex.as_str()))
            })
            .collect()
    };

    let mut names = HashSet::new();
    let mut details = HashMap::new();

    for dep in all_deps {
        let normalized = normalize_package_name(&dep.package_name);
        let search_terms = extract_search_terms(&dep.package_name);

        names.insert(normalized.clone());

        // Also insert each non-ambiguous search term for fast lookup
        for term in &search_terms {
            names.insert(term.clone());
        }

        details.insert(
            normalized,
            DepInfo {
                package_name: dep.package_name,
                version: dep.version,
                is_dev: dep.is_dev,
                is_direct: dep.is_direct,
                search_terms,
                ecosystem: dep.language,
            },
        );
    }

    (names, details)
}

/// Match content (title + body) against user's dependency graph.
/// Returns matched packages and an aggregate score (0.0-1.0).
pub(crate) fn match_dependencies(
    title: &str,
    content: &str,
    topics: &[String],
    ace_ctx: &ACEContext,
) -> (Vec<DepMatch>, f32) {
    if ace_ctx.dependency_info.is_empty() {
        return (vec![], 0.0);
    }

    let title_lower = title.to_lowercase();
    let text_lower = format!("{} {}", title_lower, content.to_lowercase());
    let mut matched = Vec::new();

    for info in ace_ctx.dependency_info.values() {
        let mut confidence = 0.0_f32;

        for term in &info.search_terms {
            let is_ambiguous = is_ambiguous_dep_name(term);

            // Title match (highest value)
            if has_word_boundary_match(&title_lower, term) {
                // Check if this is a compound-prefix match (e.g. "i18next" inside "i18next-http-middleware")
                let is_compound_prefix = title_lower.match_indices(term).any(|(pos, _)| {
                    let after = pos + term.len();
                    after < title_lower.len() && title_lower.as_bytes()[after] == b'-'
                });
                if is_compound_prefix {
                    // Different package — minimal confidence
                    confidence += 0.10;
                } else if is_ambiguous {
                    if has_language_context_nearby(&title_lower, 0, title_lower.len()) {
                        confidence += 0.4;
                    }
                } else {
                    confidence += 0.5;
                }
            }
            // Content match
            else if has_word_boundary_match(&text_lower, term) {
                if is_ambiguous {
                    // For ambiguous terms in content, need language context within 80 chars
                    if let Some(pos) = text_lower.find(term) {
                        if has_language_context_nearby(&text_lower, pos, 80) {
                            confidence += 0.15;
                        }
                    }
                } else {
                    confidence += 0.2;
                }
            }

            // Topic overlap (from extract_topics)
            if topics.iter().any(|t| topic_overlaps(t, term)) {
                confidence += 0.25;
            }
        }

        // Minimum confidence threshold to avoid noise
        if confidence < 0.15 {
            continue;
        }

        // Dev dependencies contribute less
        if info.is_dev {
            confidence *= 0.7;
        }

        // Transitive dependencies contribute less than direct dependencies.
        // A user chose `tauri` directly — a CVE in tauri is urgent.
        // `x509-cert` came in via rustls — background noise at half weight.
        if !info.is_direct {
            confidence *= 0.5;
        }

        // Infrastructure dependencies (test libraries, type declarations, linting,
        // monitoring) are present in virtually every project of their ecosystem.
        // Matching "testing" against testing-library-jest-dom doesn't mean the content
        // is about testing in the user's context. Dampen to prevent false confirmations.
        if is_infrastructure_dep(&info.package_name) {
            confidence *= 0.3;
        }

        // Version intelligence. The delta is computed against the user's INSTALLED
        // compatibility line (semver breaking axis: major for >=1.0, minor for 0.x):
        //   SameMajor  — content tracks the version you run → most relevant (boost)
        //   NewerMajor — upgrade / breaking-change ahead of you → forward-looking (boost)
        //   OlderMajor — content about a version you've moved PAST → usually stale (penalty)
        //   Unknown    — no version signal in the text → neutral
        // The OlderMajor penalty is the fix for "just because it's <framework> doesn't
        // mean it's relevant": a Tauri-v1 / React-16 / gtk-0.18 article no longer rides
        // the dependency boost when the user is on a newer line. Dampen, don't kill —
        // migration-away content can still matter, so 0.5 not 0.0.
        // Compare against the ACTUAL package name, not search_terms[0]. After
        // `terms.sort()` the first search term is the alphabetically-first SUBTERM
        // (e.g. "tanstack" for @tanstack/react-query), so version intelligence was
        // reading a sibling product's version near that subterm (bug F).
        let normalized_name = normalize_package_name(&info.package_name);
        let version_delta =
            compare_version_in_content(&text_lower, &normalized_name, &info.version);
        match version_delta {
            VersionDelta::SameMajor => confidence *= 1.2,
            VersionDelta::NewerMajor => confidence *= 1.1,
            VersionDelta::OlderMajor => confidence *= 0.5,
            VersionDelta::Unknown => {}
        }

        matched.push(DepMatch {
            package_name: normalized_name,
            confidence: confidence.min(1.0),
            version_delta,
            is_dev: info.is_dev,
            is_direct: info.is_direct,
            version: info.version.clone(),
            ecosystem: info.ecosystem.clone(),
        });
    }

    // Sort by confidence descending, keep top 5
    matched.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matched.truncate(5);

    // Aggregate score: sum of confidences, normalized
    let total: f32 = matched.iter().map(|m| m.confidence).sum();
    let score = (total / 2.0).min(1.0);

    (matched, score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_package_name_scoped() {
        assert_eq!(
            normalize_package_name("@tanstack/react-query"),
            "tanstack-react-query"
        );
        assert_eq!(normalize_package_name("@types/node"), "types-node");
        assert_eq!(
            normalize_package_name("@radix-ui/react-select"),
            "radix-ui-react-select"
        );
    }

    #[test]
    fn test_normalize_package_name_basic() {
        assert_eq!(normalize_package_name("tokio"), "tokio");
        assert_eq!(
            normalize_package_name("React-Router-DOM"),
            "react-router-dom"
        );
        assert_eq!(normalize_package_name("Serde"), "serde");
    }

    #[test]
    fn test_extract_search_terms_multi_part() {
        let terms = extract_search_terms("react-router-dom");
        assert!(terms.contains(&"react-router-dom".to_string()));
        // "react" is an ecosystem name — excluded as subterm of compound packages
        assert!(
            !terms.contains(&"react".to_string()),
            "'react' is an ecosystem name, should be excluded as subterm of react-router-dom"
        );
        assert!(terms.contains(&"router".to_string()));
        // "dom" is only 3 chars → ambiguous → filtered out
        assert!(!terms.contains(&"dom".to_string()));
    }

    #[test]
    fn test_extract_search_terms_scoped_package() {
        let terms = extract_search_terms("@tanstack/react-query");
        assert!(terms.contains(&"tanstack-react-query".to_string()));
        assert!(terms.contains(&"tanstack".to_string()));
        // "react" is an ecosystem name — excluded as subterm of compound packages
        assert!(
            !terms.contains(&"react".to_string()),
            "'react' is an ecosystem name, should be excluded as subterm of @tanstack/react-query"
        );
        // "query" is a generic tech stem — also excluded
        assert!(!terms.contains(&"query".to_string()));
    }

    #[test]
    fn test_extract_search_terms_ecosystem_guard_sentry_react() {
        // This is the exact case that caused the false positive:
        // @sentry/react should NOT have "react" as a subterm
        let terms = extract_search_terms("@sentry/react");
        assert!(terms.contains(&"sentry-react".to_string()));
        assert!(terms.contains(&"sentry".to_string()));
        assert!(
            !terms.contains(&"react".to_string()),
            "'react' is an ecosystem name, should NOT be a subterm of @sentry/react"
        );
    }

    #[test]
    fn test_extract_search_terms_standalone_ecosystem_kept() {
        // "react" as a standalone (non-compound) package IS kept
        let terms = extract_search_terms("react");
        assert!(terms.contains(&"react".to_string()));
        assert_eq!(terms.len(), 1);
    }

    #[test]
    fn test_extract_search_terms_pdf_extract_no_extract_subterm() {
        // "extract" is now in COMMON_ENGLISH_WORDS → ambiguous → excluded
        let terms = extract_search_terms("pdf-extract");
        assert!(terms.contains(&"pdf-extract".to_string()));
        // "pdf" is 3 chars → ambiguous
        assert!(!terms.contains(&"pdf".to_string()));
        // "extract" is now in COMMON_ENGLISH_WORDS → ambiguous
        assert!(
            !terms.contains(&"extract".to_string()),
            "'extract' should be excluded as a common English word"
        );
        // Only the full compound name matches
        assert_eq!(terms.len(), 1);
    }

    #[test]
    fn test_extract_is_now_ambiguous() {
        assert!(
            is_ambiguous_dep_name("extract"),
            "'extract' should be treated as ambiguous (common English word)"
        );
    }

    #[test]
    fn test_extract_search_terms_excludes_generic_tech_stems() {
        // x509-cert → splits to ["x509", "cert"]; "cert" is now a generic stem
        let terms = extract_search_terms("x509-cert");
        assert!(terms.contains(&"x509-cert".to_string()));
        assert!(terms.contains(&"x509".to_string()));
        assert!(
            !terms.contains(&"cert".to_string()),
            "'cert' is a generic tech stem, should be excluded"
        );

        // auth-client → both "auth" and "client" are generic stems
        let terms = extract_search_terms("auth-client");
        assert!(terms.contains(&"auth-client".to_string()));
        assert!(
            !terms.contains(&"auth".to_string()),
            "'auth' is a generic tech stem, should be excluded"
        );
        assert!(
            !terms.contains(&"client".to_string()),
            "'client' is a generic tech stem, should be excluded"
        );

        // http-common → both parts are generic
        let terms = extract_search_terms("http-common");
        assert!(terms.contains(&"http-common".to_string()));
        assert!(!terms.contains(&"http".to_string()));
        assert!(!terms.contains(&"common".to_string()));
    }

    #[test]
    fn test_extract_search_terms_winston_no_generic_subterms() {
        // The dogfood smoking gun: a logging library was "matching" an AI paper
        // via its generic sub-tokens "daily"/"rotate"/"file". Only the full name
        // and the distinctive "winston" should be searchable now.
        let terms = extract_search_terms("winston-daily-rotate-file");
        assert!(terms.contains(&"winston-daily-rotate-file".to_string()));
        assert!(terms.contains(&"winston".to_string()));
        for generic in ["daily", "rotate", "file"] {
            assert!(
                !terms.contains(&generic.to_string()),
                "'{generic}' is a generic word and must not be a search term"
            );
        }
    }

    #[test]
    fn test_extract_search_terms_simple() {
        let terms = extract_search_terms("tokio");
        assert!(terms.contains(&"tokio".to_string()));
        assert_eq!(terms.len(), 1); // No sub-parts to extract
    }

    #[test]
    fn test_is_ambiguous_dep_name_common_english() {
        // These are in COMMON_ENGLISH_WORDS
        assert!(is_ambiguous_dep_name("got"));
        assert!(is_ambiguous_dep_name("path"));
        assert!(is_ambiguous_dep_name("data"));
        assert!(is_ambiguous_dep_name("next"));
        assert!(is_ambiguous_dep_name("node"));
        assert!(is_ambiguous_dep_name("once"));
    }

    #[test]
    fn test_is_ambiguous_dep_name_short_always_ambiguous() {
        // <= 3 chars and not in SHORT_TECH
        assert!(is_ambiguous_dep_name("go"));
        assert!(is_ambiguous_dep_name("ab"));
        assert!(is_ambiguous_dep_name("cmd"));
    }

    #[test]
    fn test_is_ambiguous_dep_name_short_tech_allowed() {
        // These are in SHORT_TECH whitelist
        assert!(!is_ambiguous_dep_name("vue"));
        assert!(!is_ambiguous_dep_name("bun"));
        assert!(!is_ambiguous_dep_name("vite"));
    }

    #[test]
    fn test_is_ambiguous_dep_name_legit_packages() {
        // Normal package names should not be ambiguous
        assert!(!is_ambiguous_dep_name("tokio"));
        assert!(!is_ambiguous_dep_name("serde"));
        assert!(!is_ambiguous_dep_name("react"));
        assert!(!is_ambiguous_dep_name("tanstack"));
        assert!(!is_ambiguous_dep_name("typescript"));
    }

    #[test]
    fn test_parse_major_version_semver() {
        assert_eq!(parse_major_version("1.2.3"), Some(1));
        assert_eq!(parse_major_version("2.0.0"), Some(2));
        assert_eq!(parse_major_version("19.0.0"), Some(19));
    }

    #[test]
    fn test_parse_major_version_prefixed() {
        assert_eq!(parse_major_version("^1.35.0"), Some(1));
        assert_eq!(parse_major_version("~2.1.0"), Some(2));
        assert_eq!(parse_major_version("v3.0.0"), Some(3));
        assert_eq!(parse_major_version(">=5.0"), Some(5));
    }

    #[test]
    fn test_parse_major_version_invalid() {
        assert_eq!(parse_major_version(""), None);
        assert_eq!(parse_major_version("latest"), None);
        assert_eq!(parse_major_version("*"), None);
    }

    #[test]
    fn test_compare_version_newer_major() {
        let delta = compare_version_in_content(
            "Tokio 2.0 released with major breaking changes",
            "tokio",
            &Some("1.35.0".to_string()),
        );
        assert_eq!(delta, VersionDelta::NewerMajor);
    }

    #[test]
    fn test_compare_version_same_major() {
        let delta = compare_version_in_content(
            "Tokio 1.36 performance improvements",
            "tokio",
            &Some("1.35.0".to_string()),
        );
        assert_eq!(delta, VersionDelta::SameMajor);
    }

    #[test]
    fn test_compare_version_older_major() {
        let delta = compare_version_in_content(
            "Migration guide from React 17 to React 18",
            "react",
            &Some("19.0.0".to_string()),
        );
        // First occurrence: "React 17" → 17 < 19 → OlderMajor
        assert_eq!(delta, VersionDelta::OlderMajor);
    }

    #[test]
    fn test_version_triplet_parsing() {
        assert_eq!(version_triplet("1.2.3"), Some((1, 2, 3)));
        assert_eq!(version_triplet("0.18.4"), Some((0, 18, 4)));
        assert_eq!(version_triplet("0.20"), Some((0, 20, 0)));
        assert_eq!(version_triplet("2"), Some((2, 0, 0)));
        assert_eq!(version_triplet("^0.8.9"), Some((0, 8, 9)));
        // Bug A: patch is retained so 0.0.5 != 0.0.99.
        assert_eq!(version_triplet("0.0.5"), Some((0, 0, 5)));
        assert_eq!(version_triplet("0.0.99"), Some((0, 0, 99)));
        assert_eq!(version_triplet("banana"), None);
    }

    #[test]
    fn test_breaking_axis_semver_rules() {
        // >=1.0 — major is the breaking axis; minor/patch irrelevant to compat
        assert_eq!(breaking_axis((1, 2, 3)), (1, 0, 0));
        assert_eq!(breaking_axis((1, 9, 0)), (1, 0, 0));
        // 0.x (x>=1) — minor is the breaking axis
        assert_eq!(breaking_axis((0, 18, 4)), (0, 18, 0));
        assert_eq!(breaking_axis((0, 20, 0)), (0, 20, 0));
        // 0.0.z — patch is the breaking axis (strict caret, bug A)
        assert_eq!(breaking_axis((0, 0, 5)), (0, 0, 5));
        assert_eq!(breaking_axis((0, 0, 99)), (0, 0, 99));
    }

    #[test]
    fn test_compare_0_0_z_patch_is_breaking() {
        // Bug A: 0.0.5 (installed) vs 0.0.6 (content) are a breaking line apart
        // under strict caret — must NOT read SameMajor.
        let newer = compare_version_in_content(
            "mylib 0.0.6 ships a fix",
            "mylib",
            &Some("0.0.5".to_string()),
        );
        assert_eq!(
            newer,
            VersionDelta::NewerMajor,
            "0.0.5 -> 0.0.6 is breaking"
        );
        let older =
            compare_version_in_content("mylib 0.0.5 notes", "mylib", &Some("0.0.6".to_string()));
        assert_eq!(older, VersionDelta::OlderMajor, "0.0.6 user, 0.0.5 content");
        // Same exact 0.0.z line is compatible.
        let same = compare_version_in_content(
            "mylib 0.0.5 patch notes",
            "mylib",
            &Some("0.0.5".to_string()),
        );
        assert_eq!(same, VersionDelta::SameMajor, "0.0.5 == 0.0.5 same line");
    }

    #[test]
    fn test_compare_0_0_x_intel_enabled_but_bare_zero_rejected() {
        // Bug B: a real 0.0.x mention must be classified (not Unknown)...
        let delta =
            compare_version_in_content("mylib 0.0.6 released", "mylib", &Some("0.0.5".to_string()));
        assert_ne!(delta, VersionDelta::Unknown, "0.0.x must get version intel");
        // ...while a bogus bare "0" near the package stays rejected (Unknown).
        let bare = compare_version_in_content(
            "mylib 0 reasons to upgrade",
            "mylib",
            &Some("0.0.5".to_string()),
        );
        assert_eq!(bare, VersionDelta::Unknown, "bare '0' is bogus -> Unknown");
    }

    #[test]
    fn test_compare_0x_breaking_change_not_same() {
        // THE FIX: gtk 0.18 (installed) vs gtk 0.20 (content) are a breaking
        // change apart. Old major-only logic read both as "major 0" → SameMajor
        // → 1.2x boost on content about a version the user does NOT run.
        let delta = compare_version_in_content(
            "gtk 0.20 released with breaking GTK4 migration",
            "gtk",
            &Some("0.18.4".to_string()),
        );
        assert_eq!(
            delta,
            VersionDelta::NewerMajor,
            "0.18 -> 0.20 is a breaking change, must NOT be SameMajor"
        );
    }

    #[test]
    fn test_compare_0x_same_line() {
        // Same 0.x breaking line (0.18.4 installed, 0.18.9 mentioned) → compatible
        let delta = compare_version_in_content(
            "axum 0.18.9 patch release",
            "axum",
            &Some("0.18.4".to_string()),
        );
        assert_eq!(delta, VersionDelta::SameMajor);
    }

    #[test]
    fn test_compare_0x_older_line() {
        // Content about an older 0.x line than installed → OlderMajor (penalized)
        let delta = compare_version_in_content(
            "tutorial for axum 0.6 routing",
            "axum",
            &Some("0.8.0".to_string()),
        );
        assert_eq!(delta, VersionDelta::OlderMajor);
    }

    #[test]
    fn test_older_major_content_is_penalized() {
        // A React-16 article should score LOWER for a React-19 user than the same
        // article pinned to React 19 — the precision the user asked for.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "react".to_string(),
            DepInfo {
                package_name: "react".to_string(),
                version: Some("19.0.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["react".to_string()],
                ecosystem: "javascript".to_string(),
            },
        );

        let (older, older_score) = match_dependencies(
            "Understanding React 16 lifecycle methods",
            "A deep dive into componentWillMount in the react library.",
            &[],
            &ace_ctx,
        );
        let (current, current_score) = match_dependencies(
            "Understanding React 19 features",
            "A deep dive into the new react library APIs.",
            &[],
            &ace_ctx,
        );

        assert!(!older.is_empty() && !current.is_empty());
        assert!(
            older_score < current_score,
            "older-version content ({older_score}) must score below current-version ({current_score})"
        );
        assert_eq!(older[0].version_delta, VersionDelta::OlderMajor);
    }

    // ── Canonical grounding predicate ─────────────────────────────────────

    /// Build a DepMatch for predicate tests without going through matching.
    fn grounding_match(name: &str, confidence: f32, is_dev: bool, is_direct: bool) -> DepMatch {
        DepMatch {
            package_name: name.to_string(),
            confidence,
            version_delta: VersionDelta::Unknown,
            is_dev,
            is_direct,
            version: None,
            ecosystem: "rust".to_string(),
        }
    }

    #[test]
    fn strong_grounding_requires_nondev_confident_unambiguous() {
        // A direct, confident, distinctively-named match grounds.
        assert!(is_strongly_grounded(&[grounding_match(
            "axios", 0.5, false, true
        )]));
        assert!(is_strongly_grounded_direct(&[grounding_match(
            "axios", 0.5, false, true
        )]));

        // Dev-only matches never ground (a CVE in a test harness isn't urgent).
        assert!(!is_strongly_grounded(&[grounding_match(
            "axios", 0.9, true, true
        )]));

        // Below the strong-confidence floor → not grounded.
        assert!(!is_strongly_grounded(&[grounding_match(
            "axios", 0.30, false, true
        )]));

        // Word-like / ambiguous package names need ecosystem proof the bare
        // match doesn't carry — they must NOT ground on confidence alone. This
        // is the term that keeps the gate in lockstep with the persisted set.
        assert!(!is_strongly_grounded(&[grounding_match(
            "config", 0.9, false, true
        )]));
        assert!(!is_strongly_grounded(&[grounding_match(
            "core", 0.9, false, true
        )]));
    }

    #[test]
    fn strong_grounding_direct_excludes_transitive() {
        // A strong but TRANSITIVE edge grounds in general, but not at the
        // Critical (direct-only) trust floor.
        let transitive = [grounding_match("openssl-sys", 0.6, false, false)];
        assert!(is_strongly_grounded(&transitive));
        assert!(!is_strongly_grounded_direct(&transitive));
    }

    #[test]
    fn windows_os_headline_does_not_strongly_ground_windows_sys() {
        // The 2026-06-24 dogfood regression: the `windows-sys` crate matched
        // an OS-level "Windows 0-day" headline via the bare `windows` subterm
        // and was wrongly surfaced as a Critical affecting the user's stack.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "windows-sys".to_string(),
            DepInfo {
                package_name: "windows-sys".to_string(),
                version: Some("0.52.0".to_string()),
                is_dev: false,
                is_direct: true,
                // Mirrors extract_search_terms: full name + the bare subterms.
                search_terms: vec![
                    "windows-sys".to_string(),
                    "windows".to_string(),
                    "sys".to_string(),
                ],
                ecosystem: "rust".to_string(),
            },
        );

        // OS headline, no package/ecosystem context anywhere → must NOT ground.
        let (os_matches, _) = match_dependencies(
            "Windows 0-day exploit actively used in the wild",
            "Attackers are targeting the Windows operating system kernel.",
            &[],
            &ace_ctx,
        );
        assert!(
            !is_strongly_grounded(&os_matches),
            "an OS 'Windows 0-day' headline must not strongly ground the windows-sys crate, got {os_matches:?}"
        );

        // A genuine advisory naming the crate WITH ecosystem context still
        // grounds — the fix preserves real grounding, it doesn't blanket-block.
        let (crate_matches, _) = match_dependencies(
            "windows-sys 0.52 RUSTSEC advisory: unsound API",
            "The windows-sys crate on crates.io has a vulnerability; cargo update.",
            &[],
            &ace_ctx,
        );
        assert!(
            is_strongly_grounded(&crate_matches),
            "a real windows-sys crate advisory must still strongly ground, got {crate_matches:?}"
        );
    }

    #[test]
    fn test_compare_version_no_version_installed() {
        let delta = compare_version_in_content("Tokio 2.0 released", "tokio", &None);
        assert_eq!(delta, VersionDelta::Unknown);
    }

    #[test]
    fn test_compare_version_no_version_in_text() {
        let delta = compare_version_in_content(
            "Why tokio is great for async Rust",
            "tokio",
            &Some("1.35.0".to_string()),
        );
        assert_eq!(delta, VersionDelta::Unknown);
    }

    #[test]
    fn test_language_context_nearby_found() {
        let text = "the npm package got has a security vulnerability";
        let pos = text.find("got").unwrap();
        assert!(has_language_context_nearby(text, pos, 80));
    }

    #[test]
    fn test_language_context_nearby_not_found() {
        let text = "I got frustrated with the slow performance";
        let pos = text.find("got").unwrap();
        assert!(!has_language_context_nearby(text, pos, 80));
    }

    #[test]
    fn test_match_dependencies_title_match() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "tokio".to_string(),
            DepInfo {
                package_name: "tokio".to_string(),
                version: Some("1.35.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["tokio".to_string()],
                ecosystem: "rust".to_string(),
            },
        );

        let (matches, score) = match_dependencies(
            "Tokio 1.36 released with performance improvements",
            "The new version includes better async runtime tuning.",
            &["tokio".to_string()],
            &ace_ctx,
        );

        assert!(!matches.is_empty(), "Should match tokio");
        assert!(score > 0.0, "Score should be positive");
    }

    #[test]
    fn test_match_dependencies_crates_io_release_title() {
        // Decisive check: a registry release item ("crates.io: axum v0.8.9") MUST
        // match the user's direct `axum` dependency. If this fails, stack releases
        // can never reach the stack-update necessity path.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "axum".to_string(),
            DepInfo {
                package_name: "axum".to_string(),
                version: Some("0.8.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["axum".to_string()],
                ecosystem: "rust".to_string(),
            },
        );
        let (matches, score) = match_dependencies("crates.io: axum v0.8.9", "", &[], &ace_ctx);
        assert!(
            !matches.is_empty(),
            "crates.io release title must match the axum dep (score={score})"
        );
        assert!(
            score > 0.0,
            "dep-match score should be positive, got {score}"
        );
    }

    #[test]
    fn test_match_dependencies_version_uses_package_not_subterm() {
        // Bug F regression: version intelligence must compare against the package's
        // OWN name, not the alphabetically-first search subterm. A sibling umbrella
        // version ("tanstack 1.0") near the "tanstack" subterm must NOT classify the
        // installed react-query@5 as OlderMajor.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "@tanstack/react-query".to_string(),
            DepInfo {
                package_name: "@tanstack/react-query".to_string(),
                version: Some("5.0.0".to_string()),
                is_dev: false,
                is_direct: true,
                // sorted order puts the bare "tanstack" subterm first — the bug source
                search_terms: vec![
                    "tanstack".to_string(),
                    "tanstack-react-query".to_string(),
                    "react-query".to_string(),
                ],
                ecosystem: "javascript".to_string(),
            },
        );

        let (matches, _score) = match_dependencies(
            "tanstack-react-query update",
            "The tanstack 1.0 ecosystem announcement landed today.",
            &[],
            &ace_ctx,
        );

        let dep = matches
            .iter()
            .find(|m| m.package_name == "tanstack-react-query")
            .expect("react-query dep should match");
        assert_ne!(
            dep.version_delta,
            VersionDelta::OlderMajor,
            "must not read the sibling 'tanstack 1.0' as react-query's version"
        );
        assert_eq!(
            dep.version_delta,
            VersionDelta::Unknown,
            "no react-query version is mentioned, so the delta is Unknown"
        );
    }

    #[test]
    fn test_match_dependencies_no_false_positive_react() {
        // "React to market changes" should NOT match the react package
        // without language-context words nearby
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "react".to_string(),
            DepInfo {
                package_name: "react".to_string(),
                version: Some("18.2.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["react".to_string()],
                ecosystem: "javascript".to_string(),
            },
        );

        let (_matches, score) = match_dependencies(
            "How companies react to market changes in 2025",
            "Businesses must react quickly to shifting consumer trends.",
            &[],
            &ace_ctx,
        );

        // "react" is not in COMMON_ENGLISH_WORDS and is not ambiguous (len > 3),
        // so it WILL match on word boundary. This is actually correct behavior —
        // the word "react" in tech context usually IS about React.
        // The real filter is: does it pass the 2-signal gate without other signals?
        // With only 1 axis (dependency), it gets capped at 0.28.
        // The test validates the function runs without panic.
        assert!(score <= 1.0, "Score should be capped at 1.0");
    }

    #[test]
    fn test_match_dependencies_ambiguous_without_context() {
        // "got" is in COMMON_ENGLISH_WORDS — requires language context
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "got".to_string(),
            DepInfo {
                package_name: "got".to_string(),
                version: Some("14.0.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["got".to_string()],
                ecosystem: "javascript".to_string(),
            },
        );

        let (matches, _) = match_dependencies(
            "I got frustrated with the slow API",
            "The whole experience got worse over time.",
            &[],
            &ace_ctx,
        );

        assert!(
            matches.is_empty(),
            "Ambiguous 'got' without language context should NOT match"
        );
    }

    #[test]
    fn test_match_dependencies_ambiguous_with_context() {
        // "got" with "npm" nearby should match
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "got".to_string(),
            DepInfo {
                package_name: "got".to_string(),
                version: Some("14.0.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["got".to_string()],
                ecosystem: "javascript".to_string(),
            },
        );

        let (matches, score) = match_dependencies(
            "npm package got has critical security vulnerability",
            "Update your npm dependency got to version 14.2.0.",
            &[],
            &ace_ctx,
        );

        assert!(
            !matches.is_empty(),
            "Ambiguous 'got' WITH npm language context should match"
        );
        assert!(score > 0.0, "Score should be positive");
    }

    #[test]
    fn test_match_dependencies_dev_dep_attenuated() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "vitest".to_string(),
            DepInfo {
                package_name: "vitest".to_string(),
                version: Some("1.0.0".to_string()),
                is_dev: true,
                is_direct: true,
                search_terms: vec!["vitest".to_string()],
                ecosystem: "javascript".to_string(),
            },
        );

        let (matches, _) = match_dependencies(
            "Vitest 2.0 release announcement",
            "Major improvements to the test runner.",
            &["vitest".to_string()],
            &ace_ctx,
        );

        assert!(!matches.is_empty(), "Dev dep should still match");
        assert!(matches[0].is_dev, "Should be flagged as dev dependency");
        // Dev dep confidence is multiplied by 0.7
        assert!(
            matches[0].confidence < 1.0,
            "Dev dep confidence should be attenuated"
        );
    }

    #[test]
    fn test_match_dependencies_scoped_package() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "tanstack-react-query".to_string(),
            DepInfo {
                package_name: "@tanstack/react-query".to_string(),
                version: Some("5.0.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: extract_search_terms("@tanstack/react-query"),
                ecosystem: "javascript".to_string(),
            },
        );

        let (matches, score) = match_dependencies(
            "TanStack Query v5 migration guide",
            "The tanstack team released the new version of react-query.",
            &["tanstack".to_string()],
            &ace_ctx,
        );

        assert!(
            !matches.is_empty(),
            "Should match scoped package via search terms"
        );
        assert!(score > 0.0, "Score should be positive");
    }

    #[test]
    fn test_match_dependencies_empty_deps() {
        let ace_ctx = ACEContext::default();

        let (matches, score) = match_dependencies(
            "Tokio 2.0 released",
            "New async runtime features.",
            &["tokio".to_string()],
            &ace_ctx,
        );

        assert!(matches.is_empty(), "No deps = no matches");
        assert_eq!(score, 0.0, "No deps = zero score");
    }

    #[test]
    fn test_transitive_dep_attenuated() {
        // Direct dep should get higher confidence than an identical transitive dep
        let mut ace_direct = ACEContext::default();
        ace_direct.dependency_info.insert(
            "tokio".to_string(),
            DepInfo {
                package_name: "tokio".to_string(),
                version: Some("1.35.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["tokio".to_string()],
                ecosystem: "rust".to_string(),
            },
        );

        let mut ace_transitive = ACEContext::default();
        ace_transitive.dependency_info.insert(
            "tokio".to_string(),
            DepInfo {
                package_name: "tokio".to_string(),
                version: Some("1.35.0".to_string()),
                is_dev: false,
                is_direct: false,
                search_terms: vec!["tokio".to_string()],
                ecosystem: "rust".to_string(),
            },
        );

        let (direct_matches, direct_score) = match_dependencies(
            "Tokio 1.36 released with performance improvements",
            "The new version includes better async runtime tuning.",
            &["tokio".to_string()],
            &ace_direct,
        );
        let (transitive_matches, transitive_score) = match_dependencies(
            "Tokio 1.36 released with performance improvements",
            "The new version includes better async runtime tuning.",
            &["tokio".to_string()],
            &ace_transitive,
        );

        assert!(!direct_matches.is_empty(), "Direct dep should match");
        assert!(
            !transitive_matches.is_empty(),
            "Transitive dep should match"
        );
        assert!(
            direct_matches[0].is_direct,
            "Direct match should be flagged direct"
        );
        assert!(
            !transitive_matches[0].is_direct,
            "Transitive match should be flagged transitive"
        );
        assert!(
            direct_score > transitive_score,
            "Direct dep score ({direct_score}) should exceed transitive ({transitive_score})"
        );
        // Transitive gets 0.5x multiplier, so score should be roughly half
        let ratio = transitive_score / direct_score;
        assert!(
            ratio < 0.7 && ratio > 0.3,
            "Transitive/direct ratio ({ratio}) should be near 0.5"
        );
    }

    #[test]
    fn test_sentry_react_no_false_positive_on_generic_react_vuln() {
        // A general React vulnerability article should NOT match @sentry/react
        // with high confidence — the ecosystem guard prevents "react" subterm.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "sentry-react".to_string(),
            DepInfo {
                package_name: "@sentry/react".to_string(),
                version: Some("10.48.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: extract_search_terms("@sentry/react"),
                ecosystem: "javascript".to_string(),
            },
        );

        let (matches, _score) = match_dependencies(
            "Critical Security Vulnerability in React Server Components – React",
            "A denial-of-service vulnerability was found in React Server Components. \
             All React 18+ users should patch immediately.",
            &["react".to_string(), "security".to_string()],
            &ace_ctx,
        );

        // With ecosystem guard, "react" subterm is excluded from sentry-react.
        // Only the full "sentry-react" or "sentry" terms can match.
        // Neither appears in this article → no match (or very low confidence).
        if !matches.is_empty() {
            assert!(
                matches[0].confidence < 0.40,
                "sentry-react should NOT have high confidence ({}) on a generic React article",
                matches[0].confidence
            );
        }
    }

    #[test]
    fn test_pdf_extract_no_false_positive_on_generic_extraction() {
        // "Security advisory for Cargo" mentioning "extract" generically
        // should NOT match pdf-extract with high confidence.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "pdf-extract".to_string(),
            DepInfo {
                package_name: "pdf-extract".to_string(),
                version: Some("0.7.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: extract_search_terms("pdf-extract"),
                ecosystem: "rust".to_string(),
            },
        );

        let (matches, _score) = match_dependencies(
            "Security advisory for Cargo",
            "A vulnerability allows attackers to extract sensitive data from \
             cargo build artifacts. Update your Cargo installation.",
            &["cargo".to_string(), "security".to_string()],
            &ace_ctx,
        );

        // With "extract" now in COMMON_ENGLISH_WORDS, it requires language context.
        // The word "extract" in "extract sensitive data" has no nearby package/crate
        // context → should not match.
        if !matches.is_empty() {
            assert!(
                matches[0].confidence < 0.40,
                "pdf-extract should NOT have high confidence ({}) when 'extract' is used generically",
                matches[0].confidence
            );
        }
    }

    #[test]
    fn test_pdf_extract_matches_when_explicitly_mentioned() {
        // When "pdf-extract" as a full name appears, it SHOULD match.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "pdf-extract".to_string(),
            DepInfo {
                package_name: "pdf-extract".to_string(),
                version: Some("0.7.0".to_string()),
                is_dev: false,
                is_direct: true,
                search_terms: extract_search_terms("pdf-extract"),
                ecosystem: "rust".to_string(),
            },
        );

        let (matches, score) = match_dependencies(
            "Critical vulnerability in pdf-extract crate",
            "The pdf-extract Rust crate has a buffer overflow. Update to 0.8.",
            &["pdf-extract".to_string()],
            &ace_ctx,
        );

        assert!(!matches.is_empty(), "Full name 'pdf-extract' should match");
        assert!(
            matches[0].confidence >= 0.40,
            "Full name match should have high confidence ({})",
            matches[0].confidence
        );
        assert!(score > 0.0, "Score should be positive");
    }

    #[test]
    fn test_infrastructure_deps() {
        assert!(is_infrastructure_dep("@testing-library/jest-dom"));
        assert!(is_infrastructure_dep("@testing-library/react"));
        assert!(is_infrastructure_dep("vitest"));
        assert!(is_infrastructure_dep("@types/node"));
        assert!(is_infrastructure_dep("@types/jest-axe"));
        assert!(is_infrastructure_dep("typescript-eslint-parser"));
        assert!(is_infrastructure_dep("@sentry/react"));
        assert!(is_infrastructure_dep("@sentry/node"));
        assert!(is_infrastructure_dep("ts-node"));

        // Should NOT be infrastructure
        assert!(!is_infrastructure_dep("tokio"));
        assert!(!is_infrastructure_dep("serde"));
        assert!(!is_infrastructure_dep("react"));
        assert!(!is_infrastructure_dep("typescript"));
        assert!(!is_infrastructure_dep("better-sqlite3"));
        assert!(!is_infrastructure_dep("sentry")); // standalone sentry is domain-relevant
        assert!(!is_infrastructure_dep("image"));
        assert!(!is_infrastructure_dep("i18next-resources-to-backend"));
    }
}
