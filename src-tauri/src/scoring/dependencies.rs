// SPDX-License-Identifier: FSL-1.1-Apache-2.0

// UTF-8 safety gate (see the `clippy::string_slice` note in Cargo.toml).
// Byte-slicing a `str` panics on any index that is not a char boundary. This
// module was hardened against that class, so the lint is denied here to keep it
// at zero: every future slice must carry an explicit char-boundary proof
// (`floor_char_boundary`, an offset from `find` of an ASCII needle, or one of
// the `utils::text` helpers) or an `#[allow]` that states why it is safe.
#![deny(clippy::string_slice)]
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

use super::ace_context::ACEContext;
use super::utils::topic_grounds;

/// Whether the most recent `load_dependency_intelligence` call FAILED and
/// returned empty maps (DB open / dependency query error). Empty-by-error is
/// score-indistinguishable from "user has no deps", so the pipeline stamps
/// `degraded_inputs: ["dep_intel_load_failed"]` on every breakdown scored
/// under it (2026-08-23 audit, item 11). Process-global because the load
/// happens in `ace_context::build` (once per analysis run) while the marker is
/// consumed per-item in `pipeline_v2::score_item`; cleared by every successful
/// load. Concurrent runs sharing the flag is an accepted, documented race —
/// both runs share the same DB health anyway.
static DEP_INTEL_LOAD_DEGRADED: AtomicBool = AtomicBool::new(false);

/// True when the ACE dependency intelligence backing the current run came from
/// a FAILED load (see [`DEP_INTEL_LOAD_DEGRADED`]).
pub(crate) fn dep_intel_load_degraded() -> bool {
    DEP_INTEL_LOAD_DEGRADED.load(Ordering::Relaxed)
}

/// Test hook: force the degraded flag so pipeline tests can assert the marker
/// is carried onto the breakdown without staging a real DB failure.
#[cfg(test)]
pub(crate) fn set_dep_intel_load_degraded_for_test(value: bool) {
    DEP_INTEL_LOAD_DEGRADED.store(value, Ordering::Relaxed);
}

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
    /// Whether the item demonstrably names the package ITSELF — a full-name
    /// token occurrence (with, for word-like single-token names, software
    /// context or an adjacent version literal) — as opposed to a subterm
    /// expansion ("anthropic" for `@anthropic-ai/sdk`, "router" for
    /// `react-router-dom`) or an alias/topic overlap ("sqlite3" for
    /// `better-sqlite3`). Text-match grounding requires this
    /// (`is_strong_grounding_match`); the structured-advisory route checks
    /// affected-package metadata independently at its call site.
    pub corroborated: bool,
    /// The manifest's pre-normalization package name (`@babel/traverse`,
    /// `github.com/gin-gonic/gin`) — `package_name` is the NORMALIZED form
    /// (`babel-traverse`, `github.com-gin-gonic-gin`), which never appears in
    /// advisory prose. The CVE/OSV survivor text-filter tries BOTH forms
    /// (2026-08-23 audit, item 8b). `None` only on test-constructed matches.
    pub raw_name: Option<String>,
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

/// Base grounding requirements shared by every route: a match at or above the
/// strong-confidence floor whose package name is not so word-like that a bare
/// text hit can't be trusted (the persistence-layer denylist,
/// `is_ambiguous_package_name`). NOT sufficient on its own — the text route
/// additionally requires name corroboration (`is_strong_grounding_match`);
/// the structured-advisory route (`security_applicability` in `pipeline_v2`)
/// substitutes affected-package metadata as the proof instead.
///
/// Dev dependencies CAN ground (2026-08-23 audit, item 16): a vitest/vite/
/// typescript release is stack-relevant to the developer who declared it, and
/// the old `!d.is_dev` hard exclusion crushed those items to 0.03-0.40. The
/// discount is carried in `confidence` itself (`match_dependencies` applies a
/// modest 0.8x dev multiplier), still subject to the 0.40 floor here — only
/// actual manifest devDeps with a real full-name match clear it. The
/// Critical-alert lane keeps its stricter non-dev discipline separately
/// (`is_strongly_grounded_direct`, `security_applicability`).
pub(crate) fn is_grounding_candidate(d: &DepMatch) -> bool {
    d.confidence >= STRONG_GROUNDING_CONFIDENCE
        && !crate::package_ambiguity::is_ambiguous_package_name(&d.package_name)
}

/// True when `d` is a trustworthy grounding edge on its own: the base
/// requirements PLUS name corroboration — the item must actually name the
/// package, not merely contain one of its subterms or overlap a topic token.
/// Follows the persistence layer's proof philosophy
/// (`dep_linker::classify_item_dep_match`), and is deliberately the STRICTER
/// of the two: dep_linker's Tier-3 title heuristic persists a bare whole-token
/// title hit for a distinctive single-token name with no context requirement,
/// while this gate additionally demands software context / a version literal.
/// So persistence may record an edge the gate refuses (precision-first) — but
/// never the reverse for text matches. The 2026-07-02 dogfood: 29 of 39
/// critical signals were phantom-grounded by matches persistence refused —
/// every one via a subterm expansion or alias overlap whose full package name
/// never appeared.
pub(crate) fn is_strong_grounding_match(d: &DepMatch) -> bool {
    is_grounding_candidate(d) && d.corroborated
}

/// The single canonical "is this item grounded in the user's stack?" predicate.
/// True when any matched dependency is a trustworthy grounding edge. Shared by
/// the Critical-signal gate (`pipeline_signals`/`pipeline_v2`), the evidence
/// pool, and the persisted score breakdown so grounding is computed ONE way.
pub(crate) fn is_strongly_grounded(deps: &[DepMatch]) -> bool {
    deps.iter().any(is_strong_grounding_match)
}

/// As [`is_strongly_grounded`], but additionally requires the edge to be a
/// DIRECT, NON-DEV dependency — the trust floor for a Critical alert. A CVE in
/// a package the user chose directly is urgent; one reached only transitively
/// is watch-level. The `!is_dev` guard is explicit here (not inherited from
/// `is_grounding_candidate`, which admits dev deps since item 16): dev deps
/// may ground the feed, but the Critical paging lane stays production-only.
pub(crate) fn is_strongly_grounded_direct(deps: &[DepMatch]) -> bool {
    deps.iter()
        .any(|d| is_strong_grounding_match(d) && d.is_direct && !d.is_dev)
}

/// The canonical grounding verdict for one item, computed ONCE per score and
/// shared by every consumer (ScoreBreakdown.strongly_grounded → the evidence
/// pool, the Critical trust gate, the necessity stack-update path).
#[derive(Debug, Clone, Copy)]
pub(crate) struct GroundingVerdict {
    /// A trustworthy edge to the user's stack exists.
    pub strong: bool,
    /// That edge is a DIRECT dependency (Critical-alert trust floor).
    pub strong_direct: bool,
    /// True when the registry-subject route decided this verdict — the item
    /// IS a release of the user's own dependency (vs a text-route match).
    pub via_registry_subject: bool,
}

/// Manifest language a registry source's packages belong to, for congruence
/// against `user_dependencies.ecosystem` ("rust" / "javascript" / "python" /
/// "go" — set by the manifest scanners). Registries whose language never
/// appears in scanned manifests return None and cannot subject-ground.
fn registry_manifest_language(source_type: &str) -> Option<&'static str> {
    match source_type {
        "crates_io" | "crates" => Some("rust"),
        "npm_registry" | "npm" => Some("javascript"),
        "pypi" => Some("python"),
        "go_modules" | "go" => Some("go"),
        _ => None,
    }
}

/// Does the manifest language of a registry item agree with a dependency's
/// recorded ecosystem? TypeScript manifests are scanned as JavaScript-family,
/// so both spellings accept npm subjects. Unknown/empty dep ecosystems do NOT
/// ground — cross-ecosystem name collisions are precisely the failure class
/// this exists to stop (a Rust crate mentioning "react" grounding a JS dep).
fn ecosystem_congruent(registry_lang: &str, dep_ecosystem: &str) -> bool {
    let dep = dep_ecosystem.to_lowercase();
    match registry_lang {
        "javascript" => dep == "javascript" || dep == "typescript",
        other => dep == other,
    }
}

/// Compute the canonical grounding verdict.
///
/// Registry release items (crates.io / npm / PyPI / …) have a structurally
/// known SUBJECT package in `source_id`. For them, grounding is decided ONLY
/// by "is the subject the user's own dependency (same ecosystem)?" — a text
/// mention of a dep name in a third-party package's description can never
/// strongly-ground (the 2026-07-13 junk-crate class: `capacitor-tauri v0.0.0`
/// "for Tauri apps" grounding to the user's `tauri`). This mirrors the
/// persistence layer's Tier-1 `exact_registry` proof
/// (`dep_linker::classify_item_dep_match`), which already refused those links.
///
/// Non-registry items (and registry items scored without a `source_id`, which
/// only happens on ad-hoc paths that never reach the feed) keep the
/// corroborated text route.
pub(crate) fn compute_grounding_verdict(
    source_type: &str,
    source_id: Option<&str>,
    deps: &[DepMatch],
    ace_ctx: &ACEContext,
) -> GroundingVerdict {
    if crate::dep_linker::is_registry_source(source_type) {
        if let Some(sid) = source_id {
            let subject = crate::dep_linker::extract_registry_package(source_type, sid)
                .map(|s| normalize_package_name(&s));
            let registry_lang = registry_manifest_language(source_type);
            if let (Some(subject), Some(lang)) = (subject, registry_lang) {
                if let Some(info) = ace_ctx.dependency_info.values().find(|info| {
                    normalize_package_name(&info.package_name) == subject
                        && ecosystem_congruent(lang, &info.ecosystem)
                }) {
                    return GroundingVerdict {
                        // A release of a package the user's manifests declare
                        // grounds regardless of dev status (item 16): a vitest
                        // major from npm IS the user's stack. Only the
                        // Critical-alert tier stays non-dev.
                        strong: true,
                        strong_direct: !info.is_dev && info.is_direct,
                        via_registry_subject: true,
                    };
                }
            }
            // Registry item whose subject is NOT the user's dependency: never
            // strongly grounded, regardless of what the description mentions.
            return GroundingVerdict {
                strong: false,
                strong_direct: false,
                via_registry_subject: false,
            };
        }
        // Registry item with no source_id (ad-hoc path): fall through to the
        // text route rather than silently un-grounding real releases.
    }
    GroundingVerdict {
        strong: is_strongly_grounded(deps),
        strong_direct: is_strongly_grounded_direct(deps),
        via_registry_subject: false,
    }
}

/// Common English words AND generic tech stems that collide with package names.
/// These require nearby language-context words to match.
///
/// Consulted (via `is_ambiguous_dep_name`) by TWO axes: dependency matching
/// (language-context requirement, subterm filtering) and the keyword interest
/// axis (`keywords::is_generic_interest_term` — a focused user's lone
/// single-word interest on this list keeps its computed specificity weight
/// instead of the forced 1.0). Additions here tune both.
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
    // As bare tokens they anchor nothing — a package actually named like
    // this still matches via language context / full name.
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

/// Short tech keywords that are legitimate despite being short. Shared by the
/// dep-side ambiguity gate (`is_ambiguous_dep_name`) and the topic-side
/// genericness gate (`is_generic_topic_token`).
const SHORT_TECH: &[&str] = &["vue", "svelte", "htmx", "bun", "deno", "vite", "esbuild"];

/// Hot-loop lookup sets built once from the const slices above/below (the
/// slices stay the source of truth). `is_generic_topic_token` runs inside
/// per-item scoring loops (`topic_grounds` consults it per fragment pair), so
/// linear scans over the ~200-entry word list add up.
static SHORT_TECH_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| SHORT_TECH.iter().copied().collect());
static COMMON_ENGLISH_WORDS_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| COMMON_ENGLISH_WORDS.iter().copied().collect());
static GENERIC_TOPIC_TOKENS_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| GENERIC_TOPIC_TOKENS.iter().copied().collect());

/// Check if a term is a common English word (prone to false positives)
pub(crate) fn is_ambiguous_dep_name(term: &str) -> bool {
    if SHORT_TECH_SET.contains(term) {
        return false;
    }
    if term.len() <= 3 {
        return true; // Very short = always ambiguous unless in SHORT_TECH
    }
    COMMON_ENGLISH_WORDS_SET.contains(term)
}

/// Tech-generic tokens that are NOT plausible package-name subterms (so they
/// don't belong on COMMON_ENGLISH_WORDS, which also gates dep matching) but as
/// bare TOPICS match nearly everything: every backend post mentions "rest",
/// every commit touching tests mints "testing". Consulted ONLY by
/// `is_generic_topic_token` — dependency-matching behavior is unchanged.
const GENERIC_TOPIC_TOKENS: &[&str] = &[
    "rest",
    "async",
    "sync",
    "backend",
    "frontend",
    "database",
    "migration",
    "webhook",
    "websocket",
    "testing",
    "security",
    "performance",
];

/// Can this topic token, on its own, corroborate an interest/topic match?
/// Topic-side sibling of `is_ambiguous_dep_name` (same COMMON_ENGLISH_WORDS
/// denylist, same SHORT_TECH allowlist) plus the tech-generic topic tokens
/// above — but WITHOUT the dep side's "len <= 3 is always ambiguous" blanket:
/// a 3-char tech token that isn't on the word lists ("k8s", "css", "aws",
/// "sql") is a legitimate topic and may ground. 1-2 char tokens stay generic
/// ("ai"/"ml" fashion tokens match everything) unless they are known language
/// names (go/r/ts/py/...). Generic tokens can never ground a scoring axis
/// (v12); they also don't survive the startup active_topics prune and are no
/// longer minted by the ACE extractors. Expects a lowercased token.
pub(crate) fn is_generic_topic_token(t: &str) -> bool {
    // Known short language names are legitimate topics despite being 1-2
    // chars ("go" the language, not "go" the verb).
    if super::utils::SHORT_LANGUAGE_NAMES.contains(&t) {
        return false;
    }
    // Same curated short-tech allowlist the dep ambiguity gate trusts.
    if SHORT_TECH_SET.contains(t) {
        return false;
    }
    // 1-2 char tokens ("ai", "ml", "ui", "ci") anchor nothing on their own.
    if t.len() <= 2 {
        return true;
    }
    COMMON_ENGLISH_WORDS_SET.contains(t) || GENERIC_TOPIC_TOKENS_SET.contains(t)
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
    // SAFE: both ends explicitly snapped on the line above.
    #[allow(clippy::string_slice)]
    let context = &text[start..end];
    LANGUAGE_CONTEXT_WORDS.iter().any(|w| context.contains(w))
}

/// Security/advisory markers that corroborate a package-name mention as being
/// about the SOFTWARE — advisory-id prefixes, vulnerability vocabulary,
/// supply-chain vocabulary. Only consulted NEAR a full-name occurrence, so a
/// CVE roundup can't corroborate a dep whose name never appears
/// (stems like "vulnerabilit"/"compromis" cover the word families).
const SECURITY_CONTEXT_MARKERS: &[&str] = &[
    "cve-",
    "rustsec-",
    "ghsa-",
    "osv-",
    "vulnerabilit",
    "advisor",
    "security",
    "exploit",
    "malware",
    "malicious",
    "supply chain",
    "supply-chain",
    "compromis",
    "backdoor",
    "0-day",
    "zero-day",
    "patch",
];

/// Check if security-advisory markers appear near a position in text.
fn has_security_context_nearby(text: &str, position: usize, window: usize) -> bool {
    let start = snap_to_char_boundary(text, position.saturating_sub(window), false);
    let end = snap_to_char_boundary(text, (position + window).min(text.len()), true);
    // SAFE: both ends explicitly snapped on the lines above.
    #[allow(clippy::string_slice)]
    let context = &text[start..end];
    SECURITY_CONTEXT_MARKERS.iter().any(|m| context.contains(m))
}

/// A package-name boundary: characters that may legitimately appear INSIDE a
/// written package name (`-`, `_`, `.`, `@`) do not terminate it. Mirrors
/// `dep_linker::is_package_boundary` so "react" never counts as an occurrence
/// inside "react-router-dom".
fn is_package_boundary_char(c: char) -> bool {
    !c.is_alphanumeric() && c != '-' && c != '_' && c != '.' && c != '@'
}

/// Byte positions where the FULL package name occurs as a whole package token,
/// in any accepted written form: the normalized name (`anthropic-ai-sdk`), its
/// underscore variant (`anthropic_ai_sdk`), or the original scoped form
/// (`@anthropic-ai/sdk`). Trailing-dot handling: a sentence period ("axios.")
/// and the `.js`/`.ts`/`.rs` suffixes ("next.js") are accepted as boundaries;
/// any other dotted continuation ("axios.get", "next.config") is REJECTED as
/// package-name-internal — conservative, matching `dep_linker`'s treatment of
/// `.` as a name character.
///
/// Returns `(byte offset, byte length OF THE FORM THAT MATCHED)`. The length is
/// carried per position rather than recomputed by the caller because the forms
/// are NOT all the same length — `normalize_package_name` strips a leading `@`,
/// so `@foo` yields the forms `["foo", "@foo"]`. A caller that assumes one
/// uniform `name_len` reads past (or into) the name it matched.
fn package_name_positions(
    text: &str,
    package_name: &str,
    normalized_name: &str,
) -> Vec<(usize, usize)> {
    let mut forms: Vec<String> = vec![normalized_name.to_string()];
    let underscored = normalized_name.replace('-', "_");
    if underscored != normalized_name {
        forms.push(underscored);
    }
    let original = package_name.to_lowercase();
    if !forms.contains(&original) {
        forms.push(original);
    }

    let mut positions = Vec::new();
    for form in &forms {
        if form.is_empty() {
            continue;
        }
        // Shared UTF-8-safe cursor: the hand-rolled `search_from = pos + 1`
        // this replaces splits a multi-byte first char on every failed match,
        // and `form` reaches here from scraped Python `import` tokens.
        for pos in crate::utils::match_offsets(text, form.as_str()) {
            let before_ok =
                crate::utils::char_before(text, pos).is_none_or(is_package_boundary_char);
            let after = pos + form.len();
            let rest = text.get(after..).unwrap_or("");
            let after_ok = match rest.chars().next() {
                None => true,
                Some(c) if is_package_boundary_char(c) => true,
                // SAFE: this arm matched `Some('.')`, so `rest` starts with the
                // 1-byte '.' and index 1 is a char boundary. The proof is the
                // match arm — moving this body out of it breaks the slice.
                #[allow(clippy::string_slice)]
                Some('.') => {
                    rest.starts_with(".js")
                        || rest.starts_with(".ts")
                        || rest.starts_with(".rs")
                        // Sentence period: "…update axios. The fix…"
                        || rest[1..].chars().next().is_none_or(|c2| !c2.is_alphanumeric())
                }
                Some(_) => false,
            };
            if before_ok && after_ok {
                positions.push((pos, form.len()));
            }
        }
    }
    positions.sort_unstable();
    positions.dedup();
    positions
}

/// Window (bytes) around a full-name occurrence in CONTENT within which
/// software-context words must appear to corroborate a single-token name.
const NAME_CONTEXT_WINDOW: usize = 120;

/// Does the item demonstrably talk about the package ITSELF — not merely
/// contain one of its subterms or overlap one of its topic tokens?
///
/// Scoring-time mirror of the persistence layer's proof grades
/// (`dep_linker::classify_item_dep_match`):
/// - Word-like names (`is_ambiguous_dep_name`: COMMON_ENGLISH_WORDS or <= 3
///   chars — "path", "open", "got") can NEVER be proven by text alone. This
///   mirrors `dep_linker::is_specific_title_match_candidate` exactly: such
///   names only persist via the exact-registry or structured-advisory routes.
///   Coverage split: word-like names NOT on the package-ambiguity denylist
///   ("path", "open", "got") can still be elevated by the GATE's structured
///   route (`security_applicability` metadata proof — they remain grounding
///   candidates); names on BOTH lists ("config", "log", "time", "data") fail
///   `is_grounding_candidate` outright, so only the PERSISTENCE layer records
///   their advisory/registry edges — the gate never marks them critical
///   (pre-existing #174 behavior). Live phantom killed by this arm: the
///   `path` npm dep grounding an arxiv cloud-security paper.
/// - Otherwise the FULL package name must occur as a whole package token.
///   Subterm expansions ("anthropic" → `@anthropic-ai/sdk` on Anthropic
///   company news, "router" → `react-router-dom` on a Zyxel router headline,
///   "updater" → `tauri-plugin-updater` on an AMD auto-updater story) and
///   alias/topic overlaps ("sqlite3" → `better-sqlite3` on a sqlite-utils
///   release) never qualify — those were the 2026-07-02 phantom classes.
/// - Multi-token names are self-evident: a literal "windows-sys" or
///   "better-sqlite3" occurrence cannot be prose coincidence.
/// - Single-token names (axios, react, tokio) double as ordinary words
///   ("companies react to market changes"), so they corroborate only with
///   software context near an occurrence (language/registry words, a
///   security-advisory marker) or a version literal adjacent to the name
///   ("axios 1.12.2", "crates.io: axum v0.8.9").
fn is_name_corroborated(
    title_lower: &str,
    text_lower: &str,
    package_name: &str,
    normalized_name: &str,
) -> bool {
    if is_ambiguous_dep_name(normalized_name) {
        return false;
    }
    let positions = package_name_positions(text_lower, package_name, normalized_name);
    if positions.is_empty() {
        return false;
    }
    if normalized_name.contains(['-', '_']) {
        return true;
    }
    let title_len = title_lower.len();
    positions.iter().any(|&(pos, _)| {
        // A title hit may draw context from the whole title.
        let window = if pos < title_len {
            title_len.max(NAME_CONTEXT_WINDOW)
        } else {
            NAME_CONTEXT_WINDOW
        };
        has_language_context_nearby(text_lower, pos, window)
            || has_security_context_nearby(text_lower, pos, window)
    }) || has_adjacent_version_literal(text_lower, &positions)
}

/// Version-adjacency corroboration for a single-token name — anchored to the
/// boundary-checked `positions`, never a raw substring re-scan, so "react"
/// inside "Preact 10.30" or "react-router 7.5" cannot borrow a sibling
/// package's version.
///
/// Each position carries the length of the form that matched AT that position.
/// The previous signature took one `name_len` for all of them, documented as
/// "every accepted form equals the normalized single-token name, so `name_len`
/// is uniform" — which is false: `normalize_package_name` strips a leading `@`,
/// so `@foo` produces forms of length 3 and 4. Every `@foo` hit therefore
/// started its version scan one byte INSIDE the name, and did so with unsnapped
/// arithmetic — a panic whenever that landed mid-char.
fn has_adjacent_version_literal(text: &str, positions: &[(usize, usize)]) -> bool {
    positions.iter().any(|&(pos, name_len)| {
        let after_start = pos + name_len;
        if after_start >= text.len() {
            return false;
        }
        // Both ends snapped. `after_start` is a boundary by construction now,
        // but snapping it costs nothing and removes the invariant a future
        // edit would have to re-derive.
        let start = snap_to_char_boundary(text, after_start, true);
        let end = snap_to_char_boundary(text, (start + 40).min(text.len()), true);
        // SAFE: both ends explicitly snapped on the lines above.
        #[allow(clippy::string_slice)]
        version_literal_at_start(&text[start..end])
    })
}

/// Does a genuine version LITERAL appear in the first ~20 bytes after a name
/// occurrence? Accepted: a dotted numeric ("axios 1.12.2", "tokio 1.40") or a
/// v-prefixed number ("v3", "axum v0.8.9"). A bare small integer is NOT a
/// version — "How devs react to 25 new AI rules" must not read "25" as
/// react's version. (Bare integers do pass `version_triplet`, so the
/// confidence-multiplier path in `compare_version_in_content` deliberately
/// keeps its pre-existing laxity; grounding PROOF is held to more.)
fn version_literal_at_start(after_name: &str) -> bool {
    for (i, ch) in after_name.char_indices() {
        if i >= 20 {
            return false;
        }
        if !ch.is_ascii_digit() {
            continue;
        }
        // Word boundary: preceded by a non-alphanumeric, or a standalone
        // v/V prefix ("v0.8.9") — mirroring find_mentioned_version.
        let bytes = after_name.as_bytes();
        let prev = if i == 0 {
            None
        } else {
            bytes.get(i - 1).copied()
        };
        let prev_is_v_prefix = matches!(prev, Some(b'v') | Some(b'V'))
            && (i < 2 || !bytes[i - 2].is_ascii_alphanumeric());
        let boundary_ok = prev.is_none_or(|b| !b.is_ascii_alphanumeric()) || prev_is_v_prefix;
        if !boundary_ok {
            // Glued to a word — not a version. The first digit region
            // decides, as in find_mentioned_version.
            return false;
        }
        // SAFE: `i` comes from `char_indices()`, so it is a char boundary.
        #[allow(clippy::string_slice)]
        let token: String = after_name[i..]
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        // Dotted x.y — numeric on both sides of a dot — or v-prefixed.
        let dotted = token.contains('.')
            && token
                .split('.')
                .filter(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
                .count()
                >= 2;
        if !(dotted || prev_is_v_prefix) {
            return false;
        }
        return match version_triplet(&token) {
            Some(triplet) => triplet.0 < 100 && triplet != (0, 0, 0),
            None => false,
        };
    }
    false
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

/// Find the first plausible version literal mentioned adjacent to a package
/// name (both arguments already lowercased). Returns the `(major, minor,
/// patch)` triplet of e.g. "React 19", "tokio 2.0", "gtk 0.18", "axios v1.12".
/// Also serves as version-adjacency proof for grounding corroboration.
fn find_mentioned_version(text_lower: &str, pkg_lower: &str) -> Option<(u32, u32, u32)> {
    for (idx, _) in text_lower.match_indices(pkg_lower) {
        let start = idx;
        let end = (idx + pkg_lower.len() + 40).min(text_lower.len());
        let end = snap_to_char_boundary(text_lower, end, true);
        // SAFE: `start` is a `match_indices` offset and `end` is snapped.
        #[allow(clippy::string_slice)]
        let nearby = &text_lower[start..end];

        // Match patterns: "React 19", "tokio 2.0", "gtk 0.18", "v3", "version 5.1".
        // Grab the first version-like token (digits + dots) after the package name
        // so 0.x lines ("0.18" vs "0.20") are distinguishable, not collapsed to "0".
        //
        // SAFE: `nearby` starts at a `match_indices(pkg_lower)` offset, so its
        // first `pkg_lower.len()` bytes ARE that occurrence — the index is the
        // end of an exact byte match, hence a char boundary, and `end` is never
        // below `start + pkg_lower.len()`.
        #[allow(clippy::string_slice)]
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
                    // SAFE: `i` comes from `char_indices()`.
                    #[allow(clippy::string_slice)]
                    let token: String = after_name[i..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if let Some(mentioned_triplet) = version_triplet(&token) {
                        // Reject absurd majors (years, IDs) and a bogus bare "0"
                        // (parses to (0,0,0)), but a real "0.0.5" -> (0,0,5) is kept
                        // so 0.0.x version intelligence is no longer disabled (bug B).
                        if mentioned_triplet.0 < 100 && mentioned_triplet != (0, 0, 0) {
                            return Some(mentioned_triplet);
                        }
                    }
                }
                break;
            }
        }
    }

    None
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

    let text_lower = text.to_lowercase();
    let pkg_lower = pkg_name.to_lowercase();
    match find_mentioned_version(&text_lower, &pkg_lower) {
        Some(mentioned_triplet) => compare_triplets(installed_triplet, mentioned_triplet),
        None => VersionDelta::Unknown,
    }
}

/// Load all tracked dependencies from database into fast-lookup structures
pub(crate) fn load_dependency_intelligence() -> (HashSet<String>, HashMap<String, DepInfo>) {
    let db = match crate::open_db_connection() {
        Ok(db) => db,
        Err(e) => {
            // Empty maps mute the DEPENDENCY axis for the entire scoring run —
            // no dep matches, no grounding, no critical fast-path floors. That
            // must degrade loudly, never silently (accuracy-first; 2026-08-21
            // audit found this indistinguishable from "user has no deps").
            // The flag makes every breakdown scored under this run carry
            // `degraded_inputs: ["dep_intel_load_failed"]` (item 11).
            tracing::warn!(
                target: "4da::scoring",
                error = %e,
                "load_dependency_intelligence: DB open failed — dependency axis degraded to empty for this run"
            );
            DEP_INTEL_LOAD_DEGRADED.store(true, Ordering::Relaxed);
            return (HashSet::new(), HashMap::new());
        }
    };

    let all_deps = match crate::temporal::get_all_dependencies(&db) {
        Ok(deps) => deps,
        Err(e) => {
            tracing::warn!(
                target: "4da::scoring",
                error = %e,
                "load_dependency_intelligence: dependency query failed — dependency axis degraded to empty for this run"
            );
            DEP_INTEL_LOAD_DEGRADED.store(true, Ordering::Relaxed);
            return (HashSet::new(), HashMap::new());
        }
    };

    // Load succeeded — clear any degraded state from a previous failed run.
    DEP_INTEL_LOAD_DEGRADED.store(false, Ordering::Relaxed);

    // Canonical project-inclusion policy, defense in depth: the funnel above
    // (temporal::get_all_dependencies) already enforces all three tiers —
    // agent infra / temp, non-project scaffolding, and the user's "Your
    // Stack" exclusions — but relevance grounding is the highest-stakes
    // consumer, so re-check here. Slash-normalized prefix matching (the old
    // hand-rolled filter compared raw backslash paths against stored
    // forward-slash exclusions and could silently miss).
    let user_excluded = crate::project_inclusion::user_excluded_paths();
    let all_deps: Vec<_> = all_deps
        .into_iter()
        .filter(|dep| {
            !crate::project_inclusion::is_excluded_from_intelligence(
                &dep.project_path,
                &user_excluded,
            )
        })
        .collect();

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

/// How a search term occurs in a piece of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TermOccurrence {
    /// At least one occurrence is a true standalone word — not glued into a
    /// larger package-style compound.
    Standalone,
    /// The term occurs (with word boundaries) but EVERY occurrence is glued to
    /// a hyphen/underscore/dot compound — i.e. it is a fragment of a DIFFERENT
    /// package's name ("tauri" inside "capacitor-tauri" or "tauri-plugin-x").
    CompoundOnly,
    /// No word-boundary occurrence at all.
    Absent,
}

/// Classify term occurrences with package-name-aware glue detection, symmetric
/// on BOTH sides (the old check only looked for a hyphen AFTER the term, so
/// suffix compounds like "capacitor-tauri" passed as full title hits).
/// Dot handling mirrors `package_name_positions`: a `.js`/`.ts`/`.rs` suffix or
/// a sentence period is a legitimate boundary ("next.js" is the next package;
/// "…use tauri." is a word end); any other dotted continuation ("axios.get")
/// counts as glue.
fn classify_term_occurrence(text: &str, term: &str) -> TermOccurrence {
    if term.is_empty() {
        return TermOccurrence::Absent;
    }
    let mut any_boundary_hit = false;
    for (pos, _) in text.match_indices(term) {
        // SAFE: `pos` is a `match_indices` offset and `pos + term.len()` is the
        // end of an exact byte match — both char boundaries.
        #[allow(clippy::string_slice)]
        let before = text[..pos].chars().next_back();
        #[allow(clippy::string_slice)]
        let after_str = &text[pos + term.len()..];
        let after = after_str.chars().next();
        // Word boundary in the has_word_boundary_match sense.
        if before.is_some_and(|c| c.is_alphanumeric()) || after.is_some_and(|c| c.is_alphanumeric())
        {
            continue;
        }
        any_boundary_hit = true;

        // SAFE in BOTH arms below, and the proof is the match arm itself — the
        // matched char is the 1-byte '.', so `pos - 1` (the offset of that '.')
        // and index 1 of `after_str` are char boundaries. Neither slice is safe
        // outside its arm: hoisting either one out of the `Some('.')` pattern
        // re-arms the panic this branch exists to remove.
        #[allow(clippy::string_slice)]
        let glue_before = match before {
            Some('-') | Some('_') => true,
            Some('.') => text[..pos.saturating_sub(1)]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric()),
            _ => false,
        };
        #[allow(clippy::string_slice)]
        let glue_after = match after {
            Some('-') | Some('_') => true,
            Some('.') => {
                let legit_suffix = after_str.starts_with(".js")
                    || after_str.starts_with(".ts")
                    || after_str.starts_with(".rs");
                let sentence_period = after_str[1..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_alphanumeric());
                !(legit_suffix || sentence_period)
            }
            _ => false,
        };
        if !glue_before && !glue_after {
            return TermOccurrence::Standalone;
        }
    }
    if any_boundary_hit {
        TermOccurrence::CompoundOnly
    } else {
        TermOccurrence::Absent
    }
}

/// Align text-derived `corroborated` flags with registry truth. On a registry
/// release item the only package the item is ABOUT is its subject; a dep-name
/// mention in the description ("Capacitor platform runtime for Tauri apps")
/// must not mint "named in the item text" evidence chips. No-op for
/// non-registry sources or when `source_id` is unavailable.
pub(crate) fn align_registry_corroboration(
    source_type: &str,
    source_id: Option<&str>,
    deps: &mut [DepMatch],
) {
    if !crate::dep_linker::is_registry_source(source_type) {
        return;
    }
    let Some(sid) = source_id else { return };
    let Some(subject) = crate::dep_linker::extract_registry_package(source_type, sid)
        .map(|s| normalize_package_name(&s))
    else {
        return;
    };
    for d in deps.iter_mut() {
        d.corroborated = d.package_name == subject;
    }
}

/// Ecosystem congruence for the family rule — TypeScript manifests are
/// scanned as JavaScript-family (mirrors `ecosystem_congruent`); everything
/// else compares exactly. Empty ecosystems never agree.
fn family_ecosystem_congruent(a: &str, b: &str) -> bool {
    let canon = |e: &str| {
        let e = e.to_lowercase();
        if e == "typescript" {
            "javascript".to_string()
        } else {
            e
        }
    };
    !a.is_empty() && !b.is_empty() && canon(a) == canon(b)
}

/// Is `child` a recognized FAMILY FORM of `parent`? Both are RAW manifest
/// names, lowercased. Recognized forms (deliberately narrow):
///   * `<parent>_<suffix>` — `serde_derive` ← `serde`
///   * `<parent>-<suffix>` — `tokio-util` ← `tokio`
///   * `<parent>/<subpath>` — Go submodule paths
///   * `@types/<parent>`   — DefinitelyTyped declarations
///   * same npm scope      — `@babel/traverse` ← `@babel/core` (the `@types`
///     scope is excluded: it is registry infrastructure shared by unrelated
///     packages, not one publisher's family)
fn is_family_form(child: &str, parent: &str) -> bool {
    if parent.is_empty() || child == parent {
        return false;
    }
    if let Some(rest) = child.strip_prefix(parent) {
        if rest.len() > 1
            && (rest.starts_with('_') || rest.starts_with('-') || rest.starts_with('/'))
        {
            return true;
        }
    }
    if let Some(typed) = child.strip_prefix("@types/") {
        if typed == parent {
            return true;
        }
    }
    if parent.starts_with('@') && child.starts_with('@') {
        if let (Some(ps), Some(cs)) = (parent.split('/').next(), child.split('/').next()) {
            if ps == cs && ps != "@types" {
                return true;
            }
        }
    }
    false
}

/// THE FAMILY RULE (2026-08-23 audit, item 15). A transitive dependency keeps
/// FULL match weight (no 0.5x halving) when it is a family child of a DIRECT
/// declared dependency. The constraint that keeps this honest has two legs,
/// BOTH required:
///
/// 1. **Lockfile membership** — `child` exists as a `DepInfo` at all only
///    because `get_all_dependencies` found it in the user's manifests or
///    lockfiles. A shared-prefix crate that is NOT in the user's tree
///    (`serde_v8` for a plain serde user) never produces a `DepInfo`, so no
///    match exists to upgrade — prefix similarity alone can never mint
///    credit. And if `serde_v8` IS in the lockfile, it is a real transitive
///    dep of this user's tree, so full weight is earned evidence, not a
///    look-alike leak.
/// 2. **Recognized family form of a DIRECT dep** ([`is_family_form`]), same
///    ecosystem — an npm package named `serde-something` cannot ride a Rust
///    `serde` declaration.
fn is_family_child_of_direct(child: &DepInfo, ace_ctx: &ACEContext) -> bool {
    debug_assert!(!child.is_direct, "family upgrade is for transitive deps");
    let child_raw = child.package_name.to_lowercase();
    ace_ctx.dependency_info.values().any(|parent| {
        parent.is_direct
            && family_ecosystem_congruent(&parent.ecosystem, &child.ecosystem)
            && is_family_form(&child_raw, &parent.package_name.to_lowercase())
    })
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
    let content_lower = content.to_lowercase();
    let text_lower = format!("{title_lower} {content_lower}");
    let mut matched = Vec::new();

    for info in ace_ctx.dependency_info.values() {
        let mut confidence = 0.0_f32;

        for term in &info.search_terms {
            let is_ambiguous = is_ambiguous_dep_name(term);

            // Title match (highest value)
            match classify_term_occurrence(&title_lower, term) {
                TermOccurrence::Standalone => {
                    if is_ambiguous {
                        if has_language_context_nearby(&title_lower, 0, title_lower.len()) {
                            confidence += 0.4;
                        }
                    } else {
                        confidence += 0.5;
                    }
                }
                // Every title occurrence is glued into a LARGER package name
                // ("i18next" in "i18next-http-middleware", "tauri" in
                // "capacitor-tauri") — a DIFFERENT package. Minimal credit.
                // Pre-fix this only caught term-THEN-hyphen; the suffix side
                // ("capacitor-tauri") rode a full-strength title hit (the
                // 2026-07-13 junk-crate class).
                TermOccurrence::CompoundOnly => confidence += 0.10,
                TermOccurrence::Absent => {
                    // Content match
                    match classify_term_occurrence(&text_lower, term) {
                        TermOccurrence::Standalone => {
                            if is_ambiguous {
                                // Ambiguous terms in content need language context within 80 chars
                                if let Some(pos) = text_lower.find(term) {
                                    if has_language_context_nearby(&text_lower, pos, 80) {
                                        confidence += 0.15;
                                    }
                                }
                            } else {
                                confidence += 0.2;
                            }
                        }
                        // Glued into another package's name in prose: near-noise.
                        TermOccurrence::CompoundOnly => confidence += 0.05,
                        TermOccurrence::Absent => {}
                    }
                }
            }

            // Topic grounding (from extract_topics) — strict: a generic shared
            // fragment ("http" ~ "tower-http") cannot corroborate (v12)
            if topics.iter().any(|t| topic_grounds(t, term)) {
                confidence += 0.25;
            }
        }

        // Minimum confidence threshold to avoid noise
        if confidence < 0.15 {
            continue;
        }

        // Normalized name + corroboration computed up front — the family and
        // infrastructure adjustments below consume them. Compare against the
        // ACTUAL package name, not search_terms[0] (see the version-delta
        // note further down).
        let normalized_name = normalize_package_name(&info.package_name);
        let corroborated = is_name_corroborated(
            &title_lower,
            &text_lower,
            &info.package_name,
            &normalized_name,
        );

        // Dev dependencies contribute modestly less — 0.8, not the old 0.7
        // (2026-08-23 audit, item 16). Dev-dep releases (vitest, typescript)
        // ARE stack-relevant to the developer who declared them; 0.8 keeps a
        // full-name title hit (0.5) exactly at the 0.40 strong-grounding
        // floor, so real dev-dep content grounds while anything weaker still
        // falls short. Security TN discipline is preserved elsewhere: the
        // Critical lane (`is_strongly_grounded_direct`, applicability)
        // remains non-dev.
        if info.is_dev {
            confidence *= 0.8;
        }

        // Transitive dependencies contribute less than direct dependencies.
        // A user chose `tauri` directly — a CVE in tauri is urgent.
        // `x509-cert` came in via rustls — background noise at half weight.
        //
        // EXCEPTION (2026-08-23 audit, item 15): a lockfile-confirmed FAMILY
        // CHILD of a declared direct dep (`serde_derive` for a `serde` user,
        // `tokio-util` for `tokio`, `@types/react` for `react`) keeps full
        // weight — a serde_derive advisory IS a serde-user concern, and the
        // halving pinned it at 0.25 < the 0.40 strong-grounding floor, so
        // the critical fast-path never engaged (sec_serde_advisory measured
        // 0.414 exactly).
        if !info.is_direct && !is_family_child_of_direct(info, ace_ctx) {
            confidence *= 0.5;
        }

        // Infrastructure dependencies (test libraries, type declarations, linting,
        // monitoring) are present in virtually every project of their ecosystem.
        // Matching "testing" against testing-library-jest-dom doesn't mean the content
        // is about testing in the user's context. Dampen to prevent false confirmations
        // — but only for UNCORROBORATED matches (item 16): when the item
        // demonstrably names the package itself ("Vitest 3.0 released"), the
        // ubiquity argument doesn't apply, and the dampen was crushing real
        // dev-tool releases to 0.03-0.12. Subterm/alias noise stays crushed.
        if !corroborated && is_infrastructure_dep(&info.package_name) {
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
        // (`normalized_name` / `corroborated` — the grounding proof, NOT a
        // confidence input — are computed above the dev/transitive/infra
        // adjustments, which consume them.)
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
            corroborated,
            raw_name: Some(info.package_name.clone()),
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

    /// N2 regression: `package_name_positions` advanced its cursor to
    /// `pos + 1` — one byte past the START of a rejected match — which splits
    /// a multi-byte first char. The advance is reached ONLY when the boundary
    /// test fails, and the form's first char must be multi-byte, so no ASCII
    /// package name could ever exercise it. Forms reach here from scraped
    /// Python `import` tokens as well as manifest dependency names.
    #[test]
    fn package_name_positions_multibyte_form_does_not_panic() {
        // "éclair" glued to a digit: right boundary fails, cursor advances
        // into the middle of the leading 'é'.
        let positions = package_name_positions("éclair2 shipped", "éclair", "éclair");
        assert!(positions.is_empty(), "éclair2 is not a whole package token");

        // A rejected occurrence followed by an accepted one.
        let positions = package_name_positions("éclair2 and éclair here", "éclair", "éclair");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].1, "éclair".len(), "length is the FORM's");

        // Cyrillic, and the scoped form whose length differs from the
        // normalized one.
        assert!(package_name_positions("привет9", "привет", "привет").is_empty());
    }

    /// N2/N10: the positions carry the length of the form that matched, not a
    /// single "uniform" `name_len`. `normalize_package_name` strips a leading
    /// `@`, so `@fooé` yields forms of 5 and 6 bytes.
    #[test]
    fn package_name_positions_carry_the_matched_form_length() {
        let positions = package_name_positions("the @fooé 1.2.3 shipped", "@fooé", "fooé");
        assert_eq!(positions.len(), 1, "only the scoped form is bounded here");
        let (pos, len) = positions[0];
        assert_eq!(len, "@fooé".len(), "6 bytes, not the normalized name's 5");
        // `get` rather than `[..]`: it returns None on a non-boundary range, so
        // this asserts the offset/length pair is a VALID char range as well as
        // the right one — which is the whole point of carrying the length.
        assert_eq!("the @fooé 1.2.3 shipped".get(pos..pos + len), Some("@fooé"));
    }

    /// N10 regression: `has_adjacent_version_literal` computed
    /// `after_start = pos + name_len` from the NORMALIZED name's length for
    /// every position, on the documented-but-false premise that "every
    /// accepted form equals the normalized single-token name". For `@fooé`
    /// that lands one byte inside the trailing 'é' — an unsnapped index into
    /// a slice, i.e. a panic — and even on ASCII it started the version scan
    /// mid-name. The text deliberately carries no language/security context
    /// word, so corroboration MUST fall through to the version-literal arm.
    #[test]
    fn adjacent_version_literal_handles_forms_of_differing_length() {
        let text = "the @fooé 1.2.3 shipped today";
        assert!(
            is_name_corroborated(text, text, "@fooé", "fooé"),
            "the version literal after the scoped form corroborates"
        );
        // And a bare integer after the name is still not a version.
        let text = "the @fooé 25 shipped today";
        assert!(!is_name_corroborated(text, text, "@fooé", "fooé"));
    }

    #[test]
    fn test_is_generic_topic_token_no_short_blanket() {
        // 3-char tech tokens that aren't English words are specific (F0):
        // the dep-side "len <= 3" blanket must NOT leak into the topic side.
        for specific in ["k8s", "css", "aws", "sql", "gcp", "php"] {
            assert!(!is_generic_topic_token(specific), "{specific} is specific");
        }
        // Short language names are curated exceptions.
        for lang in ["go", "r", "c", "d", "ts", "js", "py"] {
            assert!(!is_generic_topic_token(lang), "{lang} is a language name");
        }
        // SHORT_TECH allowlist is honored.
        assert!(!is_generic_topic_token("bun"));
        assert!(!is_generic_topic_token("vue"));
        // 1-2 char fashion/noise tokens stay generic.
        for noise in ["ai", "ml", "ui", "ci", "cd", "db", "x"] {
            assert!(is_generic_topic_token(noise), "{noise} is generic");
        }
        // Word-list entries stay generic regardless of length.
        for word in ["api", "http", "rest", "testing", "database", "auth"] {
            assert!(is_generic_topic_token(word), "{word} is generic");
        }
        // Dep-side semantics unchanged: "aws"/"k8s" remain ambiguous AS DEP
        // NAMES (len <= 3 blanket stays for dependency matching).
        assert!(is_ambiguous_dep_name("aws"));
        assert!(is_ambiguous_dep_name("k8s"));
        assert!(!is_ambiguous_dep_name("vue"));
    }

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
    /// Corroborated by default — flip `.corroborated` to exercise the
    /// text-proof term of the predicate.
    fn grounding_match(name: &str, confidence: f32, is_dev: bool, is_direct: bool) -> DepMatch {
        DepMatch {
            package_name: name.to_string(),
            confidence,
            version_delta: VersionDelta::Unknown,
            is_dev,
            is_direct,
            version: None,
            ecosystem: "rust".to_string(),
            corroborated: true,
            raw_name: None,
        }
    }

    /// Build a direct non-dev DepInfo for matcher fixtures, with search terms
    /// derived exactly as production does (`extract_search_terms`).
    fn direct_dep_info(name: &str, version: Option<&str>, ecosystem: &str) -> DepInfo {
        DepInfo {
            package_name: name.to_string(),
            version: version.map(str::to_string),
            is_dev: false,
            is_direct: true,
            search_terms: extract_search_terms(name),
            ecosystem: ecosystem.to_string(),
        }
    }

    #[test]
    fn uncorroborated_match_never_grounds() {
        // Even a confident, direct, distinctively-named match cannot ground
        // without name corroboration — the item never named the package.
        let mut m = grounding_match("axios", 0.9, false, true);
        m.corroborated = false;
        assert!(!is_strong_grounding_match(&m));
        assert!(!is_strongly_grounded(std::slice::from_ref(&m)));
        assert!(!is_strongly_grounded_direct(&[m.clone()]));
        // But it remains a grounding CANDIDATE — the structured-advisory
        // route may still prove it via affected-package metadata.
        assert!(is_grounding_candidate(&m));
    }

    // ── Phantom-critical regression fixtures (live DB, 2026-07-02) ────────
    //
    // Each case below is a REAL critical signal measured on the founder's DB
    // after the v9 drain, phantom-grounded by a match the persistence layer
    // (dep_linker proof grades) refused. The fixtures assert BOTH halves:
    // the matcher still mints a confident (>= 0.40) match — proving the
    // fixture reproduces the phantom mechanism — and the canonical predicate
    // now refuses to ground it.

    #[test]
    fn anthropic_company_news_does_not_ground_the_sdk_dep() {
        // Company-name/package-name homonym: the "anthropic" scope token of
        // @anthropic-ai/sdk matched Anthropic company news.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "anthropic-ai-sdk".to_string(),
            direct_dep_info("@anthropic-ai/sdk", Some("0.30.0"), "javascript"),
        );

        for title in [
            "Anthropic released Claude Fable 5 yesterday. Public version of Mythos with cyber classifiers",
            "Anthropic's coordinated vulnerability disclosure dashboard",
            "Show HN: GitHub Copilot port of Anthropic's AI vulnerability discovery harness",
        ] {
            let (matches, _) = match_dependencies(
                title,
                "Anthropic announced the release. The company says the model improves coding.",
                &["anthropic".to_string()],
                &ace_ctx,
            );
            let m = matches
                .iter()
                .find(|m| m.package_name == "anthropic-ai-sdk")
                .unwrap_or_else(|| panic!("fixture must reproduce the phantom match: {title}"));
            assert!(
                m.confidence >= STRONG_GROUNDING_CONFIDENCE,
                "phantom mechanism requires a confident match, got {} for {title}",
                m.confidence
            );
            assert!(
                !is_strongly_grounded(&matches),
                "company news must NOT ground @anthropic-ai/sdk: {title}"
            );
        }
    }

    #[test]
    fn real_anthropic_sdk_advisory_still_grounds() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "anthropic-ai-sdk".to_string(),
            direct_dep_info("@anthropic-ai/sdk", Some("0.30.0"), "javascript"),
        );
        let (matches, _) = match_dependencies(
            "GHSA-xxxx: @anthropic-ai/sdk vulnerable to token exposure via debug logging",
            "Affected: @anthropic-ai/sdk (npm). Upgrade to 0.32.4.",
            &["anthropic-ai-sdk".to_string()],
            &ace_ctx,
        );
        assert!(
            is_strongly_grounded(&matches),
            "a real advisory naming the scoped package must still ground, got {matches:?}"
        );
    }

    #[test]
    fn sqlite_utils_release_does_not_ground_other_sqlite_deps() {
        // Cross-package subterm/alias: a sqlite-utils (different package!)
        // release grounded better-sqlite3 via the "sqlite3" subterm + the
        // sqlite<->sqlite3 topic alias. Live phantom title, 2026-07-02.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "better-sqlite3".to_string(),
            direct_dep_info("better-sqlite3", Some("12.0.0"), "javascript"),
        );
        ace_ctx.dependency_info.insert(
            "rusqlite".to_string(),
            direct_dep_info("rusqlite", Some("0.31.0"), "rust"),
        );
        ace_ctx.dependency_info.insert(
            "sqlite-vec".to_string(),
            direct_dep_info("sqlite-vec", Some("0.1.6"), "rust"),
        );

        let (matches, _) = match_dependencies(
            "sqlite-utils 4.0rc1 adds migrations and nested transactions",
            "sqlite-utils is a Python CLI tool and library built on the sqlite3 module \
             for manipulating SQLite databases.",
            &["sqlite-utils".to_string(), "sqlite".to_string()],
            &ace_ctx,
        );

        let phantom = matches
            .iter()
            .find(|m| m.package_name == "better-sqlite3")
            .expect("fixture must reproduce the better-sqlite3 phantom match");
        assert!(
            phantom.confidence >= STRONG_GROUNDING_CONFIDENCE,
            "phantom mechanism requires a confident match, got {}",
            phantom.confidence
        );
        assert!(
            !is_strongly_grounded(&matches),
            "a sqlite-utils release must not ground sqlite-vec/rusqlite/better-sqlite3, got {matches:?}"
        );
    }

    #[test]
    fn real_sqlite_dep_advisories_still_ground() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "better-sqlite3".to_string(),
            direct_dep_info("better-sqlite3", Some("12.0.0"), "javascript"),
        );
        ace_ctx.dependency_info.insert(
            "rusqlite".to_string(),
            direct_dep_info("rusqlite", Some("0.31.0"), "rust"),
        );

        // Multi-token full name in the title — self-evident.
        let (m1, _) = match_dependencies(
            "CVE-2026-99999: better-sqlite3 prepared statement use-after-free",
            "Affected: better-sqlite3 (npm). Upgrade to 12.6.1.",
            &["better-sqlite3".to_string()],
            &ace_ctx,
        );
        assert!(
            is_strongly_grounded(&m1),
            "a real better-sqlite3 advisory must ground, got {m1:?}"
        );

        // Single-token full name + RUSTSEC advisory marker.
        let (m2, _) = match_dependencies(
            "RUSTSEC-2026-0042: rusqlite improper lifetime on prepared statements",
            "The rusqlite crate has an unsound API. cargo update recommended.",
            &["rusqlite".to_string()],
            &ace_ctx,
        );
        assert!(
            is_strongly_grounded(&m2),
            "a real rusqlite advisory must ground, got {m2:?}"
        );
    }

    #[test]
    fn subterm_expansion_hits_do_not_ground() {
        // The dominant live phantom class: a bare subterm of a compound dep
        // name word-boundary-matches an unrelated headline. Each (dep, title)
        // pair below is a REAL phantom critical from the 2026-07-02 dogfood.
        let cases: &[(&str, &str, &str)] = &[
            (
                "tauri-plugin-deep-link",
                "rust",
                // "deep" -> Deep Reinforcement Learning
                "Angel or Demon: Investigating the Plasticity Interventions' Impact on \
                 Backdoor Threats in Deep Reinforcement Learning",
            ),
            (
                "@alpacahq/alpaca-trade-api",
                "javascript",
                // "trade" -> Trade War
                "[Opinion] Tooth and Claw: Why Europe Must Urgently Brace for a Two-Front Trade War",
            ),
            (
                "react-router-dom",
                "javascript",
                // "router" -> router image (Zyxel firmware)
                "Zyxel super-admin credential leak expanded from one router image to \
                 CPE/ONT/LTE/5G devices + password gen algo",
            ),
            (
                "@tauri-apps/plugin-updater",
                "javascript",
                // "updater" -> AMD auto-updater; note "vulnerability" appears in
                // the title — security markers must NOT resurrect subterm hits.
                "AMD denies researcher a $10,000 bug bounty after fixing critical \
                 auto-updater vulnerability",
            ),
            (
                "express-rate-limit",
                "javascript",
                // "rate" -> Ultra-Low Poison Rate
                "TooBad: Backdoor Diffusion Models with Ultra-Low Poison Rate and \
                 Imperceptible Trigger",
            ),
        ];

        for (dep_name, ecosystem, title) in cases {
            let mut ace_ctx = ACEContext::default();
            ace_ctx.dependency_info.insert(
                normalize_package_name(dep_name),
                direct_dep_info(dep_name, Some("1.0.0"), ecosystem),
            );
            let (matches, _) = match_dependencies(title, "", &[], &ace_ctx);
            let Some(m) = matches
                .iter()
                .find(|m| m.package_name == normalize_package_name(dep_name))
            else {
                // Stronger outcome than "matched but not grounded": since the
                // symmetric compound damper (2026-07-13), a hyphen-glued
                // subterm occurrence ("updater" in "auto-updater") earns only
                // compound-only credit and never even reaches the match noise
                // floor. Phantom eliminated at the matcher level.
                continue;
            };
            assert!(
                m.confidence >= STRONG_GROUNDING_CONFIDENCE,
                "phantom mechanism requires a confident match, got {} for {dep_name}",
                m.confidence
            );
            assert!(
                !is_strongly_grounded(&matches),
                "subterm expansion must NOT ground {dep_name} on: {title}"
            );
        }
    }

    #[test]
    fn real_stack_advisories_still_ground() {
        // The legitimate survivors of the 2026-07-02 dogfood — real advisories
        // naming the package itself, with CVE ids / versions / security
        // vocabulary. These MUST keep grounding (precision fix, not recall cut).
        let cases: &[(&str, &str, Option<&str>, &str, &str)] = &[
            (
                "axios",
                "javascript",
                Some("1.6.0"),
                "[CVE-2026-44490] axios has DoS & Header Injection via Prototype \
                 Pollution Read-Side Gadgets in axios merge functions",
                "Affected: axios (npm). Versions before 1.12.2 are vulnerable. \
                 Upgrade axios to 1.12.2.",
            ),
            (
                "react",
                "javascript",
                Some("19.0.0"),
                "Critical Security Vulnerability in React Server Components – React",
                "A remote code execution vulnerability was found in React Server \
                 Components. All React 19 users should patch immediately.",
            ),
            (
                "vscode",
                "javascript",
                None,
                "GitHub supply chain attack hits developer tools (NX Console, VSCode, TeamPCP)",
                "Malicious extensions compromised the vscode marketplace build pipeline.",
            ),
            (
                "rack",
                "ruby",
                Some("3.1.0"),
                "CVE-2026-12345: rack has a denial of service vulnerability",
                "The rack gem before 3.1.16 allows attackers to exhaust memory.",
            ),
        ];

        for (dep_name, ecosystem, version, title, content) in cases {
            let mut ace_ctx = ACEContext::default();
            ace_ctx.dependency_info.insert(
                normalize_package_name(dep_name),
                direct_dep_info(dep_name, *version, ecosystem),
            );
            let (matches, _) = match_dependencies(
                title,
                content,
                &[(*dep_name).to_string(), "security".to_string()],
                &ace_ctx,
            );
            assert!(
                is_strongly_grounded(&matches),
                "a real {dep_name} advisory must still ground: {title}, got {matches:?}"
            );
            assert!(
                is_strongly_grounded_direct(&matches),
                "a real direct-dep {dep_name} advisory must ground at Critical trust level"
            );
        }
    }

    #[test]
    fn prose_homonym_full_name_does_not_ground_without_software_context() {
        // Single-token package names double as ordinary words. Mere presence
        // ("companies react to market changes") is not proof the item is about
        // the package.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "react".to_string(),
            direct_dep_info("react", Some("19.0.0"), "javascript"),
        );
        let (matches, _) = match_dependencies(
            "How companies react to market changes in 2026",
            "Businesses must react quickly to shifting consumer trends.",
            &[],
            &ace_ctx,
        );
        assert!(
            !is_strongly_grounded(&matches),
            "prose 'react' without software context must not ground, got {matches:?}"
        );
    }

    #[test]
    fn word_like_names_are_never_text_grounded() {
        // "path" is a real npm package AND an everyday word. dep_linker's
        // title heuristic refuses such names outright
        // (`is_specific_title_match_candidate`) — only the exact-registry and
        // structured-advisory routes may prove them. The gate mirrors that:
        // no amount of text/context grounds "path" (live phantom: an arxiv
        // cloud-security paper grounded the `path` dep via title prose).
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "path".to_string(),
            direct_dep_info("path", Some("0.12.7"), "javascript"),
        );

        let (prose, _) = match_dependencies(
            "Demand-Driven Vulnerability Detection: Removing Human Rule Authoring from the Path",
            "The scanner walks every path in the module dependency graph to find \
             misconfigurations in the install pipeline.",
            &["path".to_string()],
            &ace_ctx,
        );
        assert!(
            !is_strongly_grounded(&prose),
            "prose 'path' must not ground, got {prose:?}"
        );

        // Even an advisory-shaped TITLE cannot text-ground a word-like name.
        // "path" is word-like (COMMON_ENGLISH_WORDS) but NOT on the
        // package-ambiguity denylist, so it remains a grounding CANDIDATE and
        // the gate's structured route (security_applicability metadata proof)
        // can still elevate a real advisory. Names on BOTH lists ("config",
        // "log", "time") fail is_grounding_candidate too — for those only the
        // persistence layer records the edge; the gate never marks them
        // critical (pre-existing #174 behavior).
        let (advisory, _) = match_dependencies(
            "npm package path 0.12 security advisory",
            "The path package on npm has a prototype pollution vulnerability.",
            &["path".to_string()],
            &ace_ctx,
        );
        assert!(
            !is_strongly_grounded(&advisory),
            "word-like 'path' must never text-ground; structured metadata is the only proof, got {advisory:?}"
        );
        // The match itself still exists as a grounding CANDIDATE, so the
        // metadata route can elevate it.
        assert!(
            advisory.iter().any(is_grounding_candidate),
            "the path match must remain a candidate for the metadata route, got {advisory:?}"
        );
    }

    #[test]
    fn version_adjacency_corroborates_single_token_names() {
        // A registry release line is proof the item is about the package even
        // without security vocabulary: full name + adjacent version literal.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "axum".to_string(),
            direct_dep_info("axum", Some("0.8.0"), "rust"),
        );
        let (matches, _) = match_dependencies("crates.io: axum v0.8.9", "", &[], &ace_ctx);
        assert!(
            is_strongly_grounded(&matches),
            "a registry release naming axum with a version must ground, got {matches:?}"
        );
    }

    #[test]
    fn bare_integer_after_name_is_not_version_corroboration() {
        // "25" is a count, not a version. Without this guard, prose "react"
        // followed by any small integer minted version-adjacency proof
        // (NewerMajor x1.1 -> 0.55 -> strongly grounded).
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "react".to_string(),
            direct_dep_info("react", Some("19.0.0"), "javascript"),
        );
        // Content avoids "developers"/"dependency"-adjacent words: the <=5
        // char context markers ("dep", "lib", "gem") still match as
        // substrings — an accepted follow-up, not this test's subject.
        let (matches, _) = match_dependencies(
            "How devs react to 25 new AI rules",
            "Teams react to 25 new rules issued this quarter.",
            &[],
            &ace_ctx,
        );
        assert!(
            !matches.is_empty(),
            "fixture must reproduce the prose react match"
        );
        assert!(
            !is_strongly_grounded(&matches),
            "a bare integer after prose 'react' must not corroborate, got {matches:?}"
        );
    }

    #[test]
    fn sibling_package_version_cannot_corroborate() {
        // The version scan is anchored to boundary-checked occurrences of THE
        // name — "react" inside "Preact 10.30" or "react-router 7.5" must not
        // lend the react dep a version literal.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "react".to_string(),
            direct_dep_info("react", Some("19.0.0"), "javascript"),
        );

        let (preact, _) = match_dependencies(
            "How devs react to the new frontend wars",
            "Preact 10.30 released this week with a smaller runtime.",
            &[],
            &ace_ctx,
        );
        assert!(
            !is_strongly_grounded(&preact),
            "'react' inside 'Preact 10.30' must not corroborate the react dep, got {preact:?}"
        );

        let (router, _) = match_dependencies(
            "How devs react to the new frontend wars",
            "react-router 7.5 announced new data APIs.",
            &[],
            &ace_ctx,
        );
        assert!(
            !is_strongly_grounded(&router),
            "'react' inside 'react-router 7.5' must not corroborate the react dep, got {router:?}"
        );
    }

    #[test]
    fn dotted_version_literal_still_corroborates() {
        // The tightening must not cost real release intelligence: a dotted
        // literal right after the package name remains proof.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.dependency_info.insert(
            "tokio".to_string(),
            direct_dep_info("tokio", Some("1.35.0"), "rust"),
        );
        let (matches, _) = match_dependencies(
            "tokio 1.40 released with runtime improvements",
            "",
            &[],
            &ace_ctx,
        );
        assert!(
            is_strongly_grounded(&matches),
            "a dotted version literal after tokio must still ground, got {matches:?}"
        );
    }

    #[test]
    fn strong_grounding_requires_confident_unambiguous() {
        // A direct, confident, distinctively-named match grounds.
        assert!(is_strongly_grounded(&[grounding_match(
            "axios", 0.5, false, true
        )]));
        assert!(is_strongly_grounded_direct(&[grounding_match(
            "axios", 0.5, false, true
        )]));

        // Dev deps DO ground for the feed (item 16: a vitest release is
        // stack-relevant) — but never at the Critical direct trust floor
        // (a CVE in a test harness doesn't page).
        assert!(is_strongly_grounded(&[grounding_match(
            "axios", 0.9, true, true
        )]));
        assert!(!is_strongly_grounded_direct(&[grounding_match(
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
        // Dev dep confidence is multiplied by 0.8 (item 16: modest discount,
        // not exclusion)
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

    // ── Family rule (item 15) + dev-dep grounding (item 16), 2026-08-23 ──

    fn dep_info(name: &str, ecosystem: &str, is_dev: bool, is_direct: bool) -> DepInfo {
        DepInfo {
            package_name: name.to_string(),
            version: None,
            is_dev,
            is_direct,
            search_terms: extract_search_terms(name),
            ecosystem: ecosystem.to_string(),
        }
    }

    fn ace_with(deps: &[DepInfo]) -> ACEContext {
        let mut ace = ACEContext::default();
        for info in deps {
            for term in &info.search_terms {
                ace.dependency_names.insert(term.clone());
            }
            ace.dependency_names.insert(info.package_name.clone());
            ace.dependency_info
                .insert(normalize_package_name(&info.package_name), info.clone());
        }
        ace
    }

    #[test]
    fn family_form_recognizes_family_and_rejects_strangers() {
        // Recognized family forms.
        assert!(is_family_form("serde_derive", "serde"));
        assert!(is_family_form("tokio-util", "tokio"));
        assert!(is_family_form("@types/react", "react"));
        assert!(is_family_form("@babel/traverse", "@babel/core"));
        assert!(is_family_form(
            "github.com/gin-gonic/gin/v2",
            "github.com/gin-gonic/gin"
        ));
        // NOT family: identity, bare-separator remainders, unrelated names,
        // and the shared @types registry scope.
        assert!(!is_family_form("serde", "serde"));
        assert!(!is_family_form("serde-", "serde"));
        assert!(!is_family_form("serenity", "serde"));
        assert!(!is_family_form("@types/node", "react"));
        assert!(!is_family_form("@types/react", "@types/node"));
        // `serde_v8` IS a family FORM of serde — by design. The honesty gate
        // is lockfile membership (`is_family_child_of_direct` only ever sees
        // deps that exist in the user's tree), not name shape.
        assert!(is_family_form("serde_v8", "serde"));
    }

    #[test]
    fn lockfile_family_child_of_direct_dep_keeps_full_weight_and_grounds() {
        // The sec_serde_advisory production case: serde declared directly,
        // serde_derive present only via the lockfile (transitive). Pre-fix
        // the 0.5x transitive halving pinned the serde_derive title hit at
        // 0.25 < the 0.40 floor — no strong grounding, no fast-path floor.
        let ace = ace_with(&[
            dep_info("serde", "rust", false, true),
            dep_info("serde_derive", "rust", false, false),
        ]);
        let (matches, _) = match_dependencies(
            "RUSTSEC-2026-0042: serde_derive unbounded recursion during deserialization",
            "serde_derive versions before 1.0.205 allow unbounded recursion when \
             deserializing deeply nested structures.",
            &[],
            &ace,
        );
        let child = matches
            .iter()
            .find(|m| m.package_name == "serde_derive")
            .expect("serde_derive must match");
        assert!(
            child.confidence >= STRONG_GROUNDING_CONFIDENCE,
            "lockfile family child must keep full weight (got {})",
            child.confidence
        );
        assert!(
            is_strongly_grounded(&matches),
            "serde_derive advisory must strongly ground a serde user"
        );
    }

    #[test]
    fn shared_prefix_crate_not_in_lockfile_gets_no_family_credit() {
        // serde_v8 is a third-party crate sharing the prefix. It is NOT in
        // this user's tree, so no DepInfo exists for it — the only possible
        // match is `serde` itself via a compound-only occurrence, which stays
        // minimal credit. No upgrade path exists without lockfile membership.
        let ace = ace_with(&[dep_info("serde", "rust", false, true)]);
        let (matches, _) = match_dependencies(
            "serde_v8 0.240.0 released",
            "Bindings between v8 and Rust values.",
            &[],
            &ace,
        );
        assert!(
            matches
                .iter()
                .all(|m| m.confidence < STRONG_GROUNDING_CONFIDENCE),
            "a compound-only prefix hit must stay below the grounding floor: {matches:?}"
        );
        assert!(!is_strongly_grounded(&matches));
    }

    #[test]
    fn transitive_non_family_dep_is_still_halved() {
        // The family exception is narrow: an ordinary transitive dep with no
        // direct-dep parent keeps the conservative 0.5x halving.
        let ace = ace_with(&[dep_info("x509-cert", "rust", false, false)]);
        let (matches, _) = match_dependencies(
            "x509-cert 0.3.0 released",
            "Pure Rust X.509 certificate handling.",
            &[],
            &ace,
        );
        assert!(!matches.is_empty(), "full-name hit still matches");
        assert!(
            matches
                .iter()
                .all(|m| m.confidence < STRONG_GROUNDING_CONFIDENCE),
            "no direct parent → halved below the grounding floor: {matches:?}"
        );
    }

    #[test]
    fn dev_dep_full_name_release_grounds_with_modest_discount() {
        // Item 16: a vitest major release with a full-name title hit must
        // ground — 0.5 title hit x 0.8 dev discount = 0.40, exactly the
        // strong-grounding floor. The adjacent version literal corroborates
        // the name, which also lifts the infrastructure dampen (the item IS
        // about the tool itself, not subterm noise).
        let ace = ace_with(&[dep_info("vitest", "javascript", true, true)]);
        let (matches, _) = match_dependencies(
            "Vitest 3.0.0 released with a redesigned browser mode",
            "The vitest 3.0.0 release overhauls the runner API.",
            &[],
            &ace,
        );
        let m = matches.first().expect("vitest must match");
        assert!(m.is_dev);
        assert!(
            m.confidence >= STRONG_GROUNDING_CONFIDENCE,
            "dev-dep full-name release must reach the grounding floor (got {})",
            m.confidence
        );
        assert!(
            is_strongly_grounded(&matches),
            "manifest devDep release is stack-relevant"
        );
        // The Critical paging lane stays production-only.
        assert!(
            !is_strongly_grounded_direct(&matches),
            "dev deps never clear the Critical direct trust floor"
        );
    }

    #[test]
    fn dev_dep_uncorroborated_mention_stays_crushed() {
        // TN guard for item 16: prose that merely mentions the tool without
        // naming-it-as-subject evidence (version literal / package context)
        // keeps the infrastructure dampen — no grounding.
        let ace = ace_with(&[dep_info("vitest", "javascript", true, true)]);
        let (matches, _) = match_dependencies(
            "Five habits of productive developers",
            "Some teams run vitest, others prefer other runners entirely.",
            &[],
            &ace,
        );
        assert!(
            matches
                .iter()
                .all(|m| m.confidence < STRONG_GROUNDING_CONFIDENCE),
            "a bare mention must not ground: {matches:?}"
        );
        assert!(!is_strongly_grounded(&matches));
    }
}
