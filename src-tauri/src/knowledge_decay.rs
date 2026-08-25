// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Knowledge Decay Alerting for 4DA
//!
//! Cross-references project dependencies with source items to detect
//! knowledge gaps - things you should know about but haven't engaged with.

// UTF-8 safety gate (see the `clippy::string_slice` note in Cargo.toml).
// Byte-slicing a `str` panics on any index that is not a char boundary. This
// module was hardened against that class, so the lint is denied here to keep it
// at zero: every future slice must carry an explicit char-boundary proof
// (`floor_char_boundary`, an offset from `find` of an ASCII needle, or one of
// the `utils::text` helpers) or an `#[allow]` that states why it is safe.
#![deny(clippy::string_slice)]

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::Result;
use crate::evidence::{
    Action as EvidenceAction, Confidence, EvidenceCitation, EvidenceFeed, EvidenceItem,
    EvidenceKind, LensHints, Urgency,
};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGap {
    pub dependency: String,
    pub version: Option<String>,
    pub project_path: String,
    pub missed_items: Vec<MissedItem>,
    pub gap_severity: GapSeverity,
    pub days_since_last_engagement: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissedItem {
    pub item_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GapSeverity {
    Critical,
    High,
    Medium,
    Low,
}

// ============================================================================
// Implementation
// ============================================================================

/// Build the user's tech domain from declared + detected tech.
/// Only dependencies matching this domain produce knowledge gaps.
fn build_tech_domain(conn: &rusqlite::Connection) -> std::collections::HashSet<String> {
    let mut domain = std::collections::HashSet::new();

    // Declared tech from onboarding (tech_stack.technology)
    if let Ok(mut stmt) = conn.prepare("SELECT technology FROM tech_stack") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for tech in rows.flatten() {
                domain.insert(tech.to_lowercase());
            }
        }
    }

    // Auto-detected tech (Language, Framework, Database, Library — not Platform)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT name FROM detected_tech WHERE category IN ('Language', 'Framework', 'Database', 'Library') AND confidence >= 0.8",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for tech in rows.flatten() {
                domain.insert(tech.to_lowercase());
            }
        }
    }

    // Declared interests (explicit_interests.topic)
    if let Ok(mut stmt) = conn.prepare("SELECT topic FROM explicit_interests") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for topic in rows.flatten() {
                domain.insert(topic.to_lowercase());
            }
        }
    }

    domain
}

/// Load the user's primary stack from onboarding for competing tech filtering
fn load_primary_stack(conn: &rusqlite::Connection) -> std::collections::HashSet<String> {
    let mut stack = std::collections::HashSet::new();
    if let Ok(mut stmt) = conn.prepare("SELECT technology FROM tech_stack") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for tech in rows.flatten() {
                stack.insert(tech.to_lowercase());
            }
        }
    }
    stack
}

/// Get project paths the user has actively committed to in the last 30 days
/// Normalize a filesystem path for cross-source comparison: lowercase + forward
/// slashes. git_signals stores OS-native paths ("D:\4DA"); project_dependencies
/// stores already-normalized paths ("d:/4da/src-tauri"). Both must pass through
/// this before any `contains` comparison.
fn normalize_project_path(p: &str) -> String {
    p.replace('\\', "/").to_lowercase()
}

fn get_active_project_paths(conn: &rusqlite::Connection) -> std::collections::HashSet<String> {
    let mut paths = std::collections::HashSet::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT DISTINCT repo_path FROM git_signals WHERE timestamp > datetime('now', '-30 days')",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for path in rows.flatten() {
                paths.insert(path);
            }
        }
    }
    paths
}

/// Check if a dependency name is relevant to the user's tech domain.
/// A dep is relevant if its name appears in the domain set, or if it's a real
/// package name (>= 4 chars, not a common English word).
fn is_dep_in_domain(dep_name: &str, domain: &std::collections::HashSet<String>) -> bool {
    let lower = dep_name.to_lowercase();

    // Direct match against domain
    if domain.contains(&lower) {
        return true;
    }

    // Check if the dep name is a common non-tech word that produces false positives.
    // These are real English words that appear as package names but match irrelevant articles.
    const GENERIC_WORDS: &[&str] = &[
        "space",
        "time",
        "image",
        "color",
        "event",
        "signal",
        "query",
        "table",
        "value",
        "error",
        "block",
        "chain",
        "field",
        "point",
        "path",
        "link",
        "node",
        "tree",
        "hash",
        "lock",
        "pool",
        "pipe",
        "ring",
        "slot",
        "core",
        "base",
        "data",
        "text",
        "font",
        "icon",
        "form",
        "grid",
        "card",
        "chip",
        "port",
        "test",
        "mock",
        "seed",
        "rand",
        "once",
        "sync",
        "glob",
        "term",
        "proc",
        "nano",
        "meta",
        "auto",
        "crypto",
        "audio",
        "video",
        "media",
        "style",
        "theme",
        "toast",
        "modal",
        "badge",
        "alert",
        "popup",
        // Common non-tech words that become package names
        "apple",
        "fashion",
        "dining",
        "sport",
        "music",
        "photo",
        "movie",
        "cosmos",
        "stellar",
        "orbit",
        "rocket",
        "matrix",
        "nova",
        "pulse",
        "amber",
        "coral",
        "ivory",
        "slate",
        "storm",
        // Words that are real package names but match too many unrelated articles
        "open",
        "next",
        "express",
        "run",
        "serve",
        "mini",
        "fast",
        "safe",
        "pure",
        "lite",
        "tiny",
        "super",
        "make",
        "copy",
        "move",
        "drop",
        "match",
        "type",
        "kind",
        "view",
        "page",
        "route",
        "state",
        "store",
        "model",
        "group",
        "just",
        "level",
        "simple",
        "clean",
        "fresh",
        "smart",
        "sharp",
        "craft",
        "prime",
        "solid",
        // Cross-ecosystem ambiguous names (exist in Rust, JS, C++, Python etc.)
        "async",
        "bytes",
        "config",
        "derive",
        "either",
        "futures",
        "http",
        "lazy",
        "mutex",
        "num",
        "regex",
        "string",
        "uuid",
        "chrono",
        "toml",
        "yaml",
        "build",
        "bench",
        "macro",
        "buffer",
        "stream",
        "channel",
        "runtime",
        "executor",
        "scheduler",
        "parallel",
        "pin",
    ];

    if GENERIC_WORDS.contains(&lower.as_str()) {
        return false;
    }

    // If domain is empty (no onboarding done), allow all deps (backward compat)
    if domain.is_empty() {
        return true;
    }

    // For deps not in domain and not obviously generic: check if any domain tech
    // is a substring match (e.g., dep "rusqlite" matches domain "rust" or "sqlite")
    domain
        .iter()
        .any(|tech| lower.contains(tech.as_str()) || tech.contains(lower.as_str()))
}

/// Normalize a title for deduplication: lowercase, strip punctuation, first 10 words
fn normalize_gap_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .take(10)
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Node.js builtin / internal module names that are not real packages and must
/// never surface as knowledge gaps. Shared by the embedding pre-pass and the
/// per-dependency loop.
const NODE_BUILTINS: &[&str] = &[
    "child_process",
    "crypto",
    "dgram",
    "domain",
    "events",
    "http",
    "http2",
    "https",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "timers",
    "tls",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
    "assert",
    "buffer",
    "cluster",
    "console",
    "dns",
    "inspector",
    "punycode",
    "sys",
];

/// Detect knowledge gaps across all tracked dependencies
/// Shortest dependency name accepted for title matching. Four characters is
/// where real packages begin in practice (`hono`, `axum`, `sqlx`, `uuid`);
/// below that a word-boundary hit is still dominated by English noise.
const MIN_MATCHABLE_NAME_LEN: usize = 4;

/// Runaway guard on the number of unique dependencies scanned in one pass.
/// Set far above any realistic dependency surface (a large multi-workspace repo
/// measured 184) because the per-dependency cost is now a pass over a
/// pre-loaded candidate slice, not a fresh table scan.
const MAX_SCANNED_DEPS: usize = 500;

/// May this dependency name be matched against item titles at all?
///
/// This replaces a blanket `len() < 5` cutoff — a character count standing in
/// for "might match the wrong thing". That cutoff was both redundant and
/// harmful. Redundant because [`keyword_misses_from`] already requires a
/// WORD-BOUNDARY match, which is the real defence against `co` matching
/// "code". Harmful because it silently excluded 216 real packages, 125 of them
/// exactly four characters — `axum`, `clap`, `sqlx`, `uuid`, `vite`, `next`,
/// `rkyv` — and `hono`, whose three unread advisories included a cross-user
/// data disclosure (CVE-2026-71850). The panel reported "no gaps detected —
/// your knowledge is current" while holding all three.
///
/// Ambiguity is now decided by `package_ambiguity`, a curated list built from
/// live false-positive audits: precision by evidence rather than by name
/// length. Names of three characters or fewer still stay out — at that length
/// even a word-boundary hit is dominated by English noise ("ai", "co", "ws")
/// and no curated list can enumerate them all.
fn dep_name_is_matchable(name: &str) -> bool {
    name.len() >= MIN_MATCHABLE_NAME_LEN
        && !crate::package_ambiguity::is_ambiguous_package_name(name)
}

/// Is this dependency close enough to the user to be worth a gap?
///
/// A direct, non-dev dependency needs no further proof: the user wrote it into
/// a manifest by hand, which IS the statement that it is their stack. Requiring
/// it to *also* appear in [`build_tech_domain`] can only produce false
/// negatives, because that domain is small and hand-entered — measured live at
/// five entries (`axum`, `react`, `tauri`, `typescript`, +1). `hono`, a direct
/// runtime dependency of `mcp-4da-server`, matched none of them and was dropped
/// along with three unread advisories, one a cross-user data disclosure.
///
/// Transitive and dev dependencies still face the domain filter: there are
/// thousands of them and the user chose none individually.
fn dep_is_relevant(
    is_direct: bool,
    is_dev: bool,
    name: &str,
    domain: &std::collections::HashSet<String>,
) -> bool {
    (is_direct && !is_dev) || is_dep_in_domain(name, domain)
}

pub fn detect_knowledge_gaps(conn: &rusqlite::Connection) -> Result<Vec<KnowledgeGap>> {
    let start = std::time::Instant::now();
    // Get all tracked dependencies
    let deps = crate::temporal::get_all_dependencies(conn)?;
    if deps.is_empty() {
        return Ok(vec![]);
    }

    // Build user's tech domain for filtering
    let domain = build_tech_domain(conn);

    // Load primary stack for competing tech filtering
    let primary_stack = load_primary_stack(conn);
    let anti_deps = crate::competing_tech::get_anti_dependencies(&primary_stack);

    // Get active project paths (committed to in last 30 days), normalized for
    // comparison. git_signals stores OS-native paths (e.g. "D:\4DA") while
    // project_dependencies stores lowercase forward-slash paths (e.g.
    // "d:/4da/src-tauri"); comparing them raw silently scoped out EVERY
    // dependency as "dormant" and zeroed the entire Coverage Gaps surface.
    let active_projects: Vec<String> = get_active_project_paths(conn)
        .iter()
        .map(|p| normalize_project_path(p))
        .collect();

    // Deduplicate deps by package name (same dep across projects → one gap)
    let mut seen_deps: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for dep in &deps {
        seen_deps
            .entry(dep.package_name.clone())
            .or_default()
            .push(dep.project_path.clone());
    }

    info!(
        target: "4da::knowledge_decay",
        unique_deps = seen_deps.len(),
        total_deps = deps.len(),
        "Processing dependencies for knowledge gaps"
    );

    // One scan, reused by every dependency below.
    let candidates = load_gap_candidates(conn)?;

    let mut gaps = Vec::new();
    let mut processed_count: usize = 0;

    for dep in &deps {
        // Skip if we already processed this dependency name
        let paths = match seen_deps.remove(&dep.package_name) {
            Some(p) => p,
            None => continue, // Already processed
        };

        processed_count += 1;
        if processed_count > MAX_SCANNED_DEPS {
            // Runaway guard only. Never a silent truncation: if this fires, the
            // surface is knowingly incomplete and must say so.
            warn!(
                target: "4da::knowledge_decay",
                scanned = MAX_SCANNED_DEPS,
                remaining = seen_deps.len(),
                "Knowledge-gap scan hit its dependency ceiling — coverage is incomplete"
            );
            break;
        }

        if !dep_name_is_matchable(&dep.package_name) {
            continue;
        }

        // Skip Node.js builtins and internal modules — not real packages
        if dep.package_name.starts_with("node:")
            || NODE_BUILTINS.contains(&dep.package_name.as_str())
            || dep.package_name.starts_with("content_") // internal 4DA modules
            || dep.package_name == "fourda-macros"
            || dep.package_name == "nlp"
        {
            continue;
        }

        // Domain filter — applied only to dependencies the user did NOT choose.
        //
        // A direct, non-dev dependency IS the user's stack: they wrote it into a
        // manifest by hand. Asking it to *also* appear in the onboarding domain
        // can only produce false negatives, because that domain is tiny and
        // hand-entered (measured live: five entries — axum, react, tauri,
        // typescript, +1). `hono`, a direct runtime dependency of
        // `mcp-4da-server`, matched none of them and was dropped along with
        // three unread advisories, one a cross-user data disclosure.
        //
        // Transitive and dev dependencies still need the filter: there are
        // thousands of them and the user never chose any individually.
        if !dep_is_relevant(dep.is_direct, dep.is_dev, &dep.package_name, &domain) {
            continue;
        }

        // Competing tech filter: skip deps that are competitors to user's chosen stack
        if anti_deps.contains(&dep.package_name.to_lowercase()) {
            continue;
        }

        // Active project scoping: skip deps from dormant projects. Both sides are
        // normalized (lowercase, forward slashes) so OS-native vs stored path
        // formats compare correctly.
        if !active_projects.is_empty()
            && !active_projects.iter().any(|ap| {
                paths.iter().any(|dp| {
                    let dp = normalize_project_path(dp);
                    dp.contains(ap) || ap.contains(&dp)
                })
            })
        {
            continue;
        }

        // Unread items whose title names this dependency (word-boundary matched).
        let missed = keyword_misses_from(&candidates, &dep.package_name);
        if missed.is_empty() {
            continue;
        }

        // Check if user has engaged with any items about this dep
        let days_since = days_since_last_engagement(conn, &dep.package_name)?;

        // Classify severity. A security advisory only escalates the gap while
        // the installed version is genuinely still inside its affected range.
        let severity = classify_severity(
            &missed,
            days_since,
            &dep.package_name,
            still_vulnerable(conn, &dep.package_name, dep.version.as_deref()),
        );

        if severity == GapSeverity::Low && days_since < 14 {
            continue; // Skip low-severity recent items
        }

        // Merge project paths for display
        let project_display = if paths.len() == 1 {
            paths[0].clone()
        } else {
            format!("{} (+{} more)", paths[0], paths.len() - 1)
        };

        gaps.push(KnowledgeGap {
            dependency: dep.package_name.clone(),
            version: dep.version.clone(),
            project_path: project_display,
            missed_items: missed,
            gap_severity: severity,
            days_since_last_engagement: days_since,
        });
    }

    // Sort by severity (critical first)
    gaps.sort_by(|a, b| {
        severity_rank(&a.gap_severity)
            .cmp(&severity_rank(&b.gap_severity))
            .then(
                b.days_since_last_engagement
                    .cmp(&a.days_since_last_engagement),
            )
    });

    // Cap at 10 gaps — quality over quantity
    gaps.truncate(10);
    info!(
        target: "4da::knowledge_decay",
        gaps = gaps.len(),
        elapsed_ms = start.elapsed().as_millis() as u64,
        "Knowledge gap detection complete"
    );
    Ok(gaps)
}

/// One unread item eligible to become a missed signal, with its title
/// pre-lowercased for matching.
struct GapCandidate {
    item: MissedItem,
    content_type: Option<String>,
    /// Lowercased title. Precomputed because the word-boundary matcher is
    /// applied once per (candidate, dependency) pair — lowercasing inside that
    /// loop allocated a fresh `String` millions of times per pass.
    title_lower: String,
}

/// Load every unread, un-dismissed candidate item ONCE per detection pass.
///
/// This replaces a per-dependency `title LIKE '%name%'` query. That shape cost
/// one full 44k-row scan per dependency, which is why the caller carried a hard
/// 50-dependency cap — and that cap, not any relevance judgement, is what hid
/// the `hono` CVEs: `hono` sits at unique position 96 of 184, so the scan
/// stopped 46 dependencies before reaching it.
///
/// One scan feeding many matchers removes the reason for the cap. The
/// `content_type` exclusions stay at the DB level, where the classification
/// computed at ingestion by `content_dna` already lives; rows with a NULL
/// `content_type` (legacy) pass through to the title-based fallback below.
fn load_gap_candidates(conn: &rusqlite::Connection) -> Result<Vec<GapCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT si.id, si.title, si.url, si.source_type, si.created_at, si.content_type
             FROM source_items si
             LEFT JOIN feedback f ON f.source_item_id = si.id
             WHERE si.created_at >= datetime('now', '-30 days')
               AND f.id IS NULL
               AND (si.content_type IS NULL
                    OR si.content_type NOT IN ('show_and_tell','tutorial','question',
                                               'help_request','hiring','clickbait'))
             ORDER BY si.created_at DESC",
    )?;

    let candidates: Vec<GapCandidate> = stmt
        .query_map([], |row| {
            let title: String = row.get(1)?;
            Ok(GapCandidate {
                title_lower: title.to_lowercase(),
                item: MissedItem {
                    item_id: row.get(0)?,
                    title,
                    url: row.get(2)?,
                    source_type: row.get(3)?,
                    created_at: row.get(4)?,
                },
                content_type: row.get::<_, Option<String>>(5)?,
            })
        })?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in knowledge_decay: {e}");
                None
            }
        })
        .collect();

    Ok(candidates)
}

/// Keyword (title) matches for one dependency, drawn from the pre-loaded
/// candidate set: word-boundary + dedup + quality filter, capped at 5.
///
/// Word-boundary matching is what keeps short names honest — "next" matches
/// "Next.js" and "next release" but never "unexpected". It is the reason the
/// caller does not need to exclude dependencies by name length.
fn keyword_misses_from(candidates: &[GapCandidate], package_name: &str) -> Vec<MissedItem> {
    let dep_lower = package_name.to_lowercase();

    // Deduplicate by normalized title (first 10 words, lowercased, stripped punctuation)
    let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();
    candidates
        .iter()
        // Cheap substring reject first; the boundary walk only runs on hits.
        .filter(|c| c.title_lower.contains(&dep_lower))
        .filter(|c| crate::utils::has_word_boundary_match_with_ext(&c.title_lower, &dep_lower))
        .filter(|c| seen_titles.insert(normalize_gap_title(&c.item.title)))
        // Title-based fallback only for legacy items without stored content_type
        .filter(|c| c.content_type.is_some() || !is_low_quality_signal(&c.item.title))
        .map(|c| c.item.clone())
        .take(5)
        .collect()
}

/// Check if `text` contains `term` at a word boundary (not embedded in a larger
/// word). Lowercases `text` first; `term` must already be lowercase.
///
/// Delegates to the shared UTF-8-safe helper. This copy walked its cursor one
/// byte past the START of a failed match — and `term` here is either a
/// dependency name or a primary-stack technology the user typed at onboarding.
pub(crate) fn has_word_boundary_match(text: &str, term: &str) -> bool {
    crate::utils::has_word_boundary_match_with_ext(&text.to_lowercase(), term)
}

/// Reject low-value content that adds noise to missed-signal feeds.
/// Returns `true` if the title matches known low-quality patterns (tutorials,
/// generic questions, off-topic personal/career content). Items mentioning
/// CVE/GHSA/vulnerability are always kept regardless of other patterns.
pub fn is_low_quality_signal(title: &str) -> bool {
    let lower = title.to_lowercase();

    // Never filter security-related items
    if lower.contains("cve-")
        || lower.contains("ghsa-")
        || lower.contains("vulnerability")
        || lower.contains("vulnerabilities")
    {
        return false;
    }

    // --- Tutorial / beginner patterns ---
    if lower.starts_with("how to ")
        || lower.starts_with("introduction to ")
        || lower.starts_with("learn ")
        || lower.starts_with("crud ")
        || lower.starts_with("what is ")
    {
        return true;
    }

    let tutorial_phrases = [
        "tutorial:",
        "tutorial -",
        "beginner",
        "beginners",
        "getting started with",
        "a beginner's guide",
        "step by step",
    ];
    if tutorial_phrases.iter().any(|p| lower.contains(p)) {
        return true;
    }

    // --- Generic question patterns ---
    let question_phrases = [
        "what's the best way to",
        "how do i ",
        "how can i ",
        "is it possible to",
        "what's the difference between",
        "which is better",
        "should i use",
    ];
    if question_phrases.iter().any(|p| lower.contains(p)) {
        return true;
    }

    // --- Off-topic: personal / career content ---
    let offtopic_words = [
        "girlfriend",
        "boyfriend",
        "wife",
        "husband",
        "job",
        "interview",
        "resume",
        "laid off",
        "hiring",
        "salary",
        "pay raise",
        "compensation",
    ];
    if offtopic_words.iter().any(|w| lower.contains(w)) {
        return true;
    }

    // --- Showcase / side-project announcements ---
    // Someone else's project using a dep is not intelligence about the dep.
    if lower.starts_with("[showcase]")
        || lower.starts_with("show hn:")
        || lower.starts_with("i built ")
        || lower.starts_with("i made ")
        || lower.starts_with("just released my")
        || lower.starts_with("i created ")
    {
        return true;
    }
    let showcase_phrases = [
        "side project",
        "my first app",
        "weekend project",
        "pet project",
        "built with",
        "built on top of",
        "built on the top of",
        "made with",
        "powered by",
    ];
    if showcase_phrases.iter().any(|p| lower.contains(p)) {
        return true;
    }

    // --- Weekly roundup / newsletter digests ---
    // These mention 10+ technologies by name but aren't about any single one.
    if lower.starts_with("this week in ")
        || lower.contains("weekly roundup")
        || lower.contains("weekly digest")
        || lower.contains("newsletter #")
    {
        return true;
    }

    false
}

fn days_since_last_engagement(conn: &rusqlite::Connection, package_name: &str) -> Result<u32> {
    let pattern = format!("%{package_name}%");

    let result: Option<String> = conn
        .query_row(
            "SELECT MAX(f.created_at)
             FROM feedback f
             JOIN source_items si ON si.id = f.source_item_id
             WHERE si.title LIKE ?1",
            params![pattern],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(date_str) = result {
        if let Ok(date) = chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S") {
            let now = chrono::Utc::now().naive_utc();
            let days = (now - date).num_days().max(0) as u32;
            Ok(days)
        } else {
            Ok(999) // Can't parse date, treat as very old
        }
    } else {
        // Fallback: check if this tech was recently detected by ACE
        if let Ok(ace) = crate::get_ace_engine() {
            if let Ok(techs) = ace.get_detected_tech() {
                for tech in &techs {
                    if tech.name.to_lowercase() == package_name.to_lowercase() {
                        // Tech is actively detected in the user's projects — not stale
                        return Ok(0);
                    }
                }
            }
        }
        Ok(999) // No engagement ever
    }
}

fn quality_weight(title: &str) -> f32 {
    match classify_missed_item(title) {
        "security advisory" => 3.0,
        "breaking change" => 2.5,
        "version update" => 1.5,
        "roadmap signal" => 1.0,
        _ => 0.5,
    }
}

/// Is the installed version of `package` still inside ANY stored advisory's
/// affected range?
///
/// An advisory naming your dependency is only a gap if you are still exposed.
/// Without this, widening the scan (four-character names, the raised cap, the
/// direct-dependency exemption) surfaces `hono` — whose three unread CVEs are
/// all fixed in 4.12.34, against an installed 4.13.2 that this repo had already
/// pinned past via `pnpm.overrides`. The panel would report a CRITICAL gap for
/// something the user had already remediated, which is exactly the false
/// positive the widening was meant to avoid creating.
///
/// Conservative in every direction: no advisories stored for the package, an
/// unreadable range, or an unknown installed version all count as STILL
/// VULNERABLE. `check_version_affected` is the same primitive the OSV matcher
/// uses, so the two surfaces cannot drift apart.
fn still_vulnerable(conn: &rusqlite::Connection, package: &str, version: Option<&str>) -> bool {
    let Ok(mut stmt) = conn.prepare(
        "SELECT affected_ranges FROM osv_advisories
         WHERE lower(package_name) = lower(?1) AND withdrawn_at IS NULL",
    ) else {
        return true;
    };
    let Ok(rows) = stmt.query_map(params![package], |row| row.get::<_, Option<String>>(0)) else {
        return true;
    };

    let mut saw_any = false;
    for ranges in rows.flatten() {
        saw_any = true;
        let (affected, _confirmed) = crate::osv::matching::check_version_affected(version, &ranges);
        if affected {
            return true;
        }
    }

    // Nothing stored about this package — say nothing about its safety.
    !saw_any
}

fn classify_severity(
    missed: &[MissedItem],
    days_since: u32,
    dep_name: &str,
    still_vulnerable: bool,
) -> GapSeverity {
    let dep_lower = dep_name.to_lowercase();

    let has_security = missed.iter().any(|item| {
        let title_lower = item.title.to_lowercase();
        (title_lower.contains("cve")
            || title_lower.contains("vulnerability")
            || title_lower.contains("security")
            || title_lower.contains("exploit"))
            && title_lower.contains(&dep_lower)
    });

    let has_breaking = missed.iter().any(|item| {
        let title_lower = item.title.to_lowercase();
        (title_lower.contains("breaking")
            || title_lower.contains("deprecated")
            || title_lower.contains("eol")
            || title_lower.contains("end of life"))
            && title_lower.contains(&dep_lower)
    });

    // Quality-weighted gap score: 1 security advisory (3.0) outweighs
    // 5 forum discussions (5 × 0.5 = 2.5).
    let weighted_score: f32 = missed.iter().map(|m| quality_weight(&m.title)).sum();
    let days_factor = if days_since >= 999 {
        1.5
    } else if days_since > 30 {
        1.2
    } else {
        1.0
    };
    let gap_score = weighted_score * days_factor;

    // A security advisory only escalates while the install is still exposed.
    // An already-patched dependency can still be worth reading about, so the
    // gap survives at its unweighted tier — it just stops shouting.
    if has_security && still_vulnerable {
        GapSeverity::Critical
    } else if has_breaking || gap_score >= 5.0 {
        GapSeverity::High
    } else if gap_score >= 2.0 || days_since > 14 {
        GapSeverity::Medium
    } else {
        GapSeverity::Low
    }
}

fn severity_rank(severity: &GapSeverity) -> u8 {
    match severity {
        GapSeverity::Critical => 0,
        GapSeverity::High => 1,
        GapSeverity::Medium => 2,
        GapSeverity::Low => 3,
    }
}

// ============================================================================
// EvidenceItem conversion (Intelligence Reconciliation — Phase 5)
// ============================================================================

fn gap_severity_to_urgency(s: &GapSeverity) -> Urgency {
    match s {
        GapSeverity::Critical => Urgency::Critical,
        GapSeverity::High => Urgency::High,
        GapSeverity::Medium => Urgency::Medium,
        GapSeverity::Low => Urgency::Watch,
    }
}

fn truncate_gap_title(s: &str) -> String {
    s.trim_end_matches('.').chars().take(120).collect()
}

fn truncate_gap_note(s: &str) -> String {
    s.chars().take(200).collect()
}

fn classify_missed_item(title: &str) -> &'static str {
    let lower = title.to_lowercase();
    if lower.contains("cve") || lower.contains("ghsa") || lower.contains("vulnerability") {
        "security advisory"
    } else if lower.contains("breaking") || lower.contains("deprecated") || lower.contains("eol") {
        "breaking change"
    } else if lower.contains("release") || lower.contains("update") || lower.contains("upgrade") {
        "version update"
    } else if lower.contains("rfc") || lower.contains("proposal") || lower.contains("roadmap") {
        "roadmap signal"
    } else {
        "relevant discussion"
    }
}

/// A knowledge gap is substantive only if at least one missed item carries
/// CONSEQUENCE — a security advisory, breaking change, or version update. A gap
/// that is purely roadmap chatter / general discussion is unread VOLUME, not a
/// knowledge gap, and ships SILENT (intelligence-doctrine rule 6: no thin/noisy
/// surfaces). This is what produced the old "typescript: 5 unread items" gap
/// headlined by an obscure alpha crate — none of its items were actionable.
fn gap_is_substantive(gap: &KnowledgeGap) -> bool {
    gap.missed_items.iter().any(|m| {
        matches!(
            classify_missed_item(&m.title),
            "security advisory" | "breaking change" | "version update"
        )
    })
}

fn missed_item_to_citation(m: &MissedItem) -> EvidenceCitation {
    let freshness_days = chrono::NaiveDateTime::parse_from_str(&m.created_at, "%Y-%m-%d %H:%M:%S")
        .map(|dt| {
            let secs = chrono::Utc::now().timestamp() - dt.and_utc().timestamp();
            (secs as f32 / 86_400.0).max(0.0)
        })
        .unwrap_or(0.0);
    let category = classify_missed_item(&m.title);
    EvidenceCitation {
        source: m.source_type.clone(),
        title: truncate_gap_title(&m.title),
        url: m.url.clone(),
        freshness_days,
        relevance_note: truncate_gap_note(&format!("Unread {category}")),
    }
}

fn build_gap_explanation(
    dep: &str,
    version: Option<&str>,
    days_since: u32,
    missed: &[MissedItem],
) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(3);

    // Categorize what was missed
    let mut security = 0u32;
    let mut breaking = 0u32;
    let mut updates = 0u32;
    let mut other = 0u32;
    for m in missed {
        match classify_missed_item(&m.title) {
            "security advisory" => security += 1,
            "breaking change" => breaking += 1,
            "version update" => updates += 1,
            _ => other += 1,
        }
    }

    // Lead with the most critical category
    if security > 0 {
        parts.push(format!(
            "{security} unread security {}",
            if security == 1 {
                "advisory"
            } else {
                "advisories"
            }
        ));
    }
    if breaking > 0 {
        parts.push(format!(
            "{breaking} breaking {}",
            if breaking == 1 { "change" } else { "changes" }
        ));
    }
    if updates > 0 {
        parts.push(format!(
            "{updates} version {}",
            if updates == 1 { "update" } else { "updates" }
        ));
    }
    if other > 0 && parts.is_empty() {
        parts.push(format!(
            "{other} unread {}",
            if other == 1 { "signal" } else { "signals" }
        ));
    }

    let categories = parts.join(", ");

    // Version context
    let ver = version.map(|v| format!(" v{v}")).unwrap_or_default();

    // Engagement recency
    let recency = if days_since >= 999 {
        "never reviewed".to_string()
    } else if days_since > 30 {
        format!("last reviewed {days_since}d ago")
    } else {
        format!("{days_since}d since last review")
    };

    // Highlight the most notable missed item — by CONSEQUENCE, not list order.
    // Security advisories and breaking changes outrank a version update: a CVE must
    // never be buried under a routine release just because the release appears first
    // in the list. A version update still outranks a raw first(), so a surfaced gap
    // never falls back to a noisy alpha-crate item (the af79d241 anti-noise intent).
    let highlight = missed
        .iter()
        .find(|m| {
            let c = classify_missed_item(&m.title);
            c == "security advisory" || c == "breaking change"
        })
        .or_else(|| {
            missed
                .iter()
                .find(|m| classify_missed_item(&m.title) == "version update")
        })
        .or_else(|| missed.first());

    let mut explanation = format!("{dep}{ver}: {categories} · {recency}");

    if let Some(item) = highlight {
        let short_title: String = item.title.chars().take(80).collect();
        explanation.push_str(&format!(" — notably \"{short_title}\""));
    }

    explanation
}

fn build_gap_actions(missed: &[MissedItem]) -> Vec<EvidenceAction> {
    let mut actions = Vec::with_capacity(3);
    let has_security = missed
        .iter()
        .any(|m| classify_missed_item(&m.title) == "security advisory");
    let has_breaking = missed
        .iter()
        .any(|m| classify_missed_item(&m.title) == "breaking change");
    let has_update = missed
        .iter()
        .any(|m| classify_missed_item(&m.title) == "version update");

    if has_security {
        actions.push(EvidenceAction {
            action_id: "review_security".to_string(),
            label: "Review advisories".to_string(),
            description: "Check unread security advisories for this dependency.".to_string(),
        });
    }
    if has_breaking {
        actions.push(EvidenceAction {
            action_id: "check_breaking".to_string(),
            label: "Check breaking changes".to_string(),
            description: "Review breaking changes before your next upgrade.".to_string(),
        });
    }
    if has_update && !has_security && !has_breaking {
        actions.push(EvidenceAction {
            action_id: "review_updates".to_string(),
            label: "Review updates".to_string(),
            description: "Catch up on version updates for this dependency.".to_string(),
        });
    }
    if actions.is_empty() {
        actions.push(EvidenceAction {
            action_id: "investigate".to_string(),
            label: "Investigate".to_string(),
            description: "Review missed signals for this dependency.".to_string(),
        });
    }
    actions
}

impl KnowledgeGap {
    /// Convert a legacy `KnowledgeGap` into the canonical `EvidenceItem`.
    /// Used by `get_knowledge_gaps` (command boundary) and callable from
    /// any future lens that wants gap-shaped evidence.
    pub fn to_evidence_item(&self) -> EvidenceItem {
        let title = truncate_gap_title(&format!("Knowledge gap: {}", self.dependency));

        let explanation = build_gap_explanation(
            &self.dependency,
            self.version.as_deref(),
            self.days_since_last_engagement,
            &self.missed_items,
        );

        let evidence: Vec<EvidenceCitation> = self
            .missed_items
            .iter()
            .take(5)
            .map(missed_item_to_citation)
            .collect();

        EvidenceItem {
            id: format!("kg_{}", self.dependency),
            kind: EvidenceKind::Gap,
            title,
            explanation,
            confidence: Confidence::heuristic(0.7),
            urgency: gap_severity_to_urgency(&self.gap_severity),
            reversibility: None,
            evidence,
            affected_projects: vec![self.project_path.clone()],
            affected_deps: vec![self.dependency.clone()],
            suggested_actions: build_gap_actions(&self.missed_items),
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints {
                briefing: false,
                preemption: false,
                blind_spots: true,
                evidence: true,
                // Knowledge-decay gaps are not platform-target-scoped (Phase 2c).
                other_build_target: false,
                // Not an upgrade-plan step (Phase 1 dep plan).
                upgrade_plan: false,
            },
            created_at: chrono::Utc::now().timestamp_millis(),
            expires_at: None,
        }
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Returns the canonical `EvidenceFeed` for the Knowledge Gaps view.
/// Schema-validates every item; violators drop with a structured log.
#[tauri::command]
pub fn get_knowledge_gaps() -> Result<EvidenceFeed> {
    crate::settings::require_signal_feature("get_knowledge_gaps")?;
    let conn = crate::open_db_connection()?;
    let gaps = detect_knowledge_gaps(&conn)?;
    let items: Vec<EvidenceItem> = gaps
        .iter()
        .filter(|g| !g.missed_items.is_empty())
        // Ship silent unless substantive: a gap must carry actionable consequence
        // (security / breaking / version update), not just unread discussion.
        .filter(|g| gap_is_substantive(g))
        .map(|g| g.to_evidence_item())
        .filter(|item| match crate::evidence::validate_item(item) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(
                    target: "4da::evidence::validate",
                    id = %item.id,
                    error = %e,
                    "dropped knowledge-gap item failing schema validation"
                );
                false
            }
        })
        .collect();
    Ok(EvidenceFeed::from_items(items))
}
// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_project_scoping_matches_across_path_formats() {
        // git_signals stores "D:\4DA" (OS-native); project_dependencies stores
        // "d:/4da/src-tauri" (lowercase, forward slash). Raw .contains() across
        // these silently scoped out every dependency as dormant and zeroed the
        // Coverage Gaps surface. After normalization they must match.
        let active = normalize_project_path("D:\\4DA");
        assert_eq!(active, "d:/4da");
        let dep = normalize_project_path("d:/4da/src-tauri");
        assert!(
            dep.contains(&active) || active.contains(&dep),
            "active project {active} should match dependency project {dep}"
        );
        // An unrelated project must still be scoped out.
        let other = normalize_project_path("C:/Users/dev/kairos-mvp");
        assert!(
            !(other.contains(&active) || active.contains(&other)),
            "unrelated project must not match"
        );
    }

    #[test]
    fn test_normalize_gap_title() {
        assert_eq!(
            normalize_gap_title("TypeScript 6.0 Beta: What's New!"),
            "typescript 60 beta whats new"
        );
        assert_eq!(
            normalize_gap_title("TypeScript 6.0 Beta — What's New?"),
            "typescript 60 beta whats new"
        );
    }

    #[test]
    fn test_normalize_deduplicates_similar_titles() {
        // These two titles differ only at word 11+, so first-10-words match
        let t1 =
            normalize_gap_title("TypeScript 6.0 Beta: What's New in the Big Release Update Today");
        let t2 = normalize_gap_title(
            "TypeScript 6.0 Beta: What's New in the Big Release Update Tomorrow",
        );
        assert_eq!(t1, t2);

        // Titles with different content should NOT match
        let t3 = normalize_gap_title("TypeScript 6.0 Beta: Performance Improvements");
        assert_ne!(t1, t3);
    }

    #[test]
    fn test_generic_words_expanded() {
        let domain = std::collections::HashSet::new();
        // New additions should be filtered
        assert!(!is_dep_in_domain("open", &domain));
        assert!(!is_dep_in_domain("next", &domain));
        assert!(!is_dep_in_domain("express", &domain));
        assert!(!is_dep_in_domain("solid", &domain));
        assert!(!is_dep_in_domain("fresh", &domain));
        // Original generics still filtered
        assert!(!is_dep_in_domain("node", &domain));
        assert!(!is_dep_in_domain("space", &domain));
        // Cross-ecosystem ambiguous names should be filtered
        assert!(!is_dep_in_domain("futures", &domain));
        assert!(!is_dep_in_domain("async", &domain));
        assert!(!is_dep_in_domain("bytes", &domain));
        assert!(!is_dep_in_domain("config", &domain));
        assert!(!is_dep_in_domain("runtime", &domain));
    }

    #[test]
    fn test_domain_match_still_works() {
        let mut domain = std::collections::HashSet::new();
        domain.insert("tokio".to_string());
        domain.insert("serde".to_string());
        assert!(is_dep_in_domain("tokio", &domain));
        assert!(is_dep_in_domain("serde", &domain));
        // Substring match: rusqlite contains "sqlite" if sqlite is in domain
        domain.insert("sqlite".to_string());
        assert!(is_dep_in_domain("rusqlite", &domain));
    }

    #[test]
    fn test_word_boundary_match() {
        assert!(has_word_boundary_match("Next.js 15 Released", "next"));
        assert!(has_word_boundary_match("What's next for Rust", "next"));
        assert!(!has_word_boundary_match(
            "Unexpected behavior in Node",
            "next"
        ));
    }

    /// Regression: the cursor advanced to `abs_pos + 1` — one byte past the
    /// START of a failed match — splitting a multi-byte first char. `term` here
    /// is a dependency name or a primary-stack technology the user typed at
    /// onboarding, and the advance is reached only when the match abuts an
    /// alphanumeric char.
    #[test]
    fn test_word_boundary_multibyte_term_does_not_panic() {
        // The term's FIRST char must be multi-byte for `abs_pos + 1` to split it.
        assert!(!has_word_boundary_match("Éclair2 Released", "éclair"));
        assert!(!has_word_boundary_match("привет9", "привет"));
        assert!(has_word_boundary_match("Éclair2 and Éclair ship", "éclair"));
        // Lowercasing of `text` is still applied.
        assert!(has_word_boundary_match("ÉCLAIR ships", "éclair"));
        assert!(!has_word_boundary_match("aé bé", ""));
    }

    // ========================================================================
    // EvidenceItem conversion tests (Intelligence Reconciliation — Phase 5)
    // ========================================================================

    fn sample_gap() -> KnowledgeGap {
        KnowledgeGap {
            dependency: "tokio".to_string(),
            version: Some("1.36.0".to_string()),
            project_path: "/proj/a".to_string(),
            missed_items: vec![
                MissedItem {
                    item_id: 1,
                    title: "Tokio async runtime v1.36 released".to_string(),
                    url: Some("https://example.test/1".to_string()),
                    source_type: "hn".to_string(),
                    created_at: "2026-04-10 10:00:00".to_string(),
                },
                MissedItem {
                    item_id: 2,
                    title: "CVE-2026-1234 affects tokio 1.x".to_string(),
                    url: None,
                    source_type: "github-advisory".to_string(),
                    created_at: "2026-04-15 12:00:00".to_string(),
                },
            ],
            gap_severity: GapSeverity::Critical,
            days_since_last_engagement: 30,
        }
    }

    #[test]
    fn knowledge_gap_maps_to_gap_kind() {
        let item = sample_gap().to_evidence_item();
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Gap);
    }

    #[test]
    fn gap_is_substantive_requires_actionable_consequence() {
        // sample_gap carries a CVE + a release → substantive (surfaces).
        assert!(gap_is_substantive(&sample_gap()));
        // Pure general discussion (no security/breaking/version-update keywords)
        // is unread volume, not a gap → ships silent. This is the exact shape of
        // the weak "typescript: 5 unread items" gap headlined by an alpha crate.
        let mut noisy = sample_gap();
        noisy.missed_items = vec![
            MissedItem {
                item_id: 9,
                title: "TypeScript: the practical guide for JS developers".to_string(),
                url: None,
                source_type: "devto".to_string(),
                created_at: "2026-06-01 00:00:00".to_string(),
            },
            MissedItem {
                item_id: 10,
                title: "crates.io: code-split-plugin-typescript v1.0.0-alpha.4".to_string(),
                url: None,
                source_type: "crates_io".to_string(),
                created_at: "2026-06-05 00:00:00".to_string(),
            },
        ];
        assert!(!gap_is_substantive(&noisy));
    }

    #[test]
    fn knowledge_gap_severity_maps_to_urgency() {
        let mut g = sample_gap();
        g.gap_severity = GapSeverity::Critical;
        assert_eq!(
            g.to_evidence_item().urgency,
            crate::evidence::Urgency::Critical
        );
        g.gap_severity = GapSeverity::High;
        assert_eq!(g.to_evidence_item().urgency, crate::evidence::Urgency::High);
        g.gap_severity = GapSeverity::Medium;
        assert_eq!(
            g.to_evidence_item().urgency,
            crate::evidence::Urgency::Medium
        );
        g.gap_severity = GapSeverity::Low;
        assert_eq!(
            g.to_evidence_item().urgency,
            crate::evidence::Urgency::Watch
        );
    }

    #[test]
    fn knowledge_gap_citations_taken_from_missed_items() {
        let item = sample_gap().to_evidence_item();
        assert_eq!(item.evidence.len(), 2);
        assert_eq!(item.evidence[0].source, "hn");
        assert_eq!(item.evidence[1].source, "github-advisory");
    }

    #[test]
    fn knowledge_gap_with_no_missed_items_has_empty_evidence() {
        let mut g = sample_gap();
        g.missed_items.clear();
        let item = g.to_evidence_item();
        assert!(item.evidence.is_empty());
    }

    #[test]
    fn knowledge_gap_caps_citations_at_5() {
        let mut g = sample_gap();
        g.missed_items = (0..10)
            .map(|i| MissedItem {
                item_id: i,
                title: format!("article #{i}"),
                url: None,
                source_type: "hn".to_string(),
                created_at: "2026-04-10 10:00:00".to_string(),
            })
            .collect();
        let item = g.to_evidence_item();
        assert_eq!(item.evidence.len(), 5);
    }

    #[test]
    fn knowledge_gap_tags_blind_spots_and_evidence_lenses() {
        let item = sample_gap().to_evidence_item();
        assert!(item.lens_hints.blind_spots);
        assert!(item.lens_hints.evidence);
        assert!(!item.lens_hints.preemption);
        assert!(!item.lens_hints.briefing);
    }

    #[test]
    fn knowledge_gap_passes_schema_validation() {
        assert!(crate::evidence::validate_item(&sample_gap().to_evidence_item()).is_ok());
    }

    #[test]
    fn knowledge_gap_affected_projects_and_deps_populated() {
        let item = sample_gap().to_evidence_item();
        assert_eq!(item.affected_projects, vec!["/proj/a".to_string()]);
        assert_eq!(item.affected_deps, vec!["tokio".to_string()]);
    }

    #[test]
    fn gap_explanation_categorizes_missed_signals() {
        let g = sample_gap();
        let item = g.to_evidence_item();
        assert!(
            item.explanation.contains("security"),
            "should mention security: {}",
            item.explanation
        );
        assert!(
            item.explanation.contains("tokio v1.36.0"),
            "should include version: {}",
            item.explanation
        );
        assert!(
            item.explanation.contains("30d"),
            "should mention days since review: {}",
            item.explanation
        );
    }

    #[test]
    fn gap_explanation_highlights_notable_item() {
        let g = sample_gap();
        let item = g.to_evidence_item();
        assert!(
            item.explanation.contains("notably"),
            "should highlight a notable item: {}",
            item.explanation
        );
        assert!(
            item.explanation.contains("CVE-2026-1234"),
            "should mention the CVE: {}",
            item.explanation
        );
    }

    #[test]
    fn gap_explanation_never_engaged() {
        let mut g = sample_gap();
        g.days_since_last_engagement = 999;
        let item = g.to_evidence_item();
        assert!(
            item.explanation.contains("never reviewed"),
            "should say never reviewed: {}",
            item.explanation
        );
    }

    #[test]
    fn gap_actions_include_review_security_for_cve() {
        let g = sample_gap();
        let item = g.to_evidence_item();
        assert!(
            item.suggested_actions
                .iter()
                .any(|a| a.action_id == "review_security"),
            "should have review_security action for security gaps"
        );
    }

    #[test]
    fn gap_actions_generic_for_plain_items() {
        let mut g = sample_gap();
        g.missed_items = vec![MissedItem {
            item_id: 10,
            title: "Tokio best practices discussion".to_string(),
            url: None,
            source_type: "hn".to_string(),
            created_at: "2026-04-10 10:00:00".to_string(),
        }];
        let item = g.to_evidence_item();
        assert!(
            item.suggested_actions
                .iter()
                .any(|a| a.action_id == "investigate"),
            "should fall back to investigate for generic items"
        );
    }

    #[test]
    fn gap_citation_relevance_note_is_descriptive() {
        let g = sample_gap();
        let item = g.to_evidence_item();
        assert!(
            item.evidence[0].relevance_note.contains("Unread"),
            "citation note should categorize: {}",
            item.evidence[0].relevance_note
        );
        assert!(
            !item.evidence[0].relevance_note.contains("missed item #"),
            "citation note should not be generic: {}",
            item.evidence[0].relevance_note
        );
    }

    // -----------------------------------------------------------------------
    // Short-name blind spot (2026-08-25 live Signal audit)
    //
    // The app rendered "No gaps detected — your knowledge is current" while the
    // corpus held three unread Hono advisories, one a cross-user data
    // disclosure. Two independent causes, both required for the miss:
    //   * `hono` is four characters and a `len() < 5` gate dropped it outright;
    //   * the scan stopped after 50 unique dependencies; `hono` sits at 96/184.
    // -----------------------------------------------------------------------

    fn cand(id: i64, title: &str, content_type: Option<&str>) -> GapCandidate {
        GapCandidate {
            title_lower: title.to_lowercase(),
            item: MissedItem {
                item_id: id,
                title: title.to_string(),
                url: None,
                source_type: "cve".to_string(),
                created_at: "2026-08-12 01:34:41".to_string(),
            },
            content_type: content_type.map(str::to_string),
        }
    }

    #[test]
    fn four_character_package_names_are_matchable() {
        // The name that was lost, plus the rest of the four-character bucket
        // measured in the live dependency set (125 packages).
        for name in [
            "hono", "axum", "sqlx", "uuid", "vite", "rkyv", "yaml", "zstd",
        ] {
            assert!(
                dep_name_is_matchable(name),
                "{name} is a real package and must be scanned"
            );
        }
    }

    #[test]
    fn ambiguous_names_are_still_excluded_regardless_of_length() {
        // Curated from live false-positive audits ("Tower Bridge", "Defense
        // Express"). Length never decided these; evidence did.
        // `next` and `clap` belong here, not with the matchable four-character
        // names: both are four characters AND everyday words, so the curated
        // list — not the length rule — is what keeps them out. That split is
        // the whole point: length was never the right discriminator.
        for name in [
            "log", "http", "ring", "time", "rand", "tower", "next", "clap",
        ] {
            assert!(
                !dep_name_is_matchable(name),
                "{name} is audit-confirmed ambiguous and must stay excluded"
            );
        }
    }

    #[test]
    fn very_short_names_stay_excluded() {
        for name in ["ai", "co", "ws", "rc", "bl", "der", "nom"] {
            assert!(
                !dep_name_is_matchable(name),
                "{name} is too short for title matching to be meaningful"
            );
        }
    }

    #[test]
    fn short_dependency_name_finds_its_real_advisories() {
        let candidates = vec![
            cand(
                192,
                "[CVE-2026-71850] Hono: memo() retains SSR output across requests",
                None,
            ),
            cand(
                193,
                "[CVE-2026-71849] Hono: Proxy Helper does not remove response headers",
                None,
            ),
            cand(
                194,
                "[CVE-2026-71848] Hono: Algorithmic Complexity DoS in Language Middleware",
                None,
            ),
            // Must NOT match: `hono` embedded inside a longer word.
            cand(900, "Phonology of consonant clusters in synthesis", None),
            cand(901, "Building a phonograph simulator in Rust", None),
        ];

        let missed = keyword_misses_from(&candidates, "hono");
        let ids: Vec<i64> = missed.iter().map(|m| m.item_id).collect();
        assert_eq!(
            ids,
            vec![192, 193, 194],
            "all three real advisories surface; no embedded-substring match does"
        );
    }

    #[test]
    fn word_boundary_still_rejects_embedded_matches_for_short_names() {
        let candidates = vec![
            cand(1, "Unexpected panic in the parser", None),
            cand(2, "Next.js 15 release notes", None),
        ];
        let missed = keyword_misses_from(&candidates, "next");
        assert_eq!(missed.len(), 1, "next matches Next.js but not 'unexpected'");
        assert_eq!(missed[0].item_id, 2);
    }

    #[test]
    fn keyword_misses_dedupe_and_cap_at_five() {
        let mut candidates: Vec<GapCandidate> = (0..8)
            .map(|i| cand(i, &format!("hono security advisory number {i}"), None))
            .collect();
        // An identical title must collapse into the first.
        candidates.push(cand(100, "hono security advisory number 0", None));

        let missed = keyword_misses_from(&candidates, "hono");
        assert_eq!(missed.len(), 5, "capped at five citations");
        let unique: std::collections::HashSet<String> = missed
            .iter()
            .map(|m| normalize_gap_title(&m.title))
            .collect();
        assert_eq!(unique.len(), missed.len(), "no duplicate titles survive");
    }

    /// The live onboarding domain, verbatim from the machine that produced the
    /// miss. Deliberately small — that is the point.
    fn live_domain() -> std::collections::HashSet<String> {
        ["axum", "react", "tauri", "typescript"]
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn a_declared_direct_dependency_needs_no_domain_membership() {
        let domain = live_domain();
        // Premise: the domain genuinely does not admit hono. If this ever
        // starts passing, the exemption below is no longer load-bearing.
        assert!(
            !is_dep_in_domain("hono", &domain),
            "guard premise: a 5-entry hand-entered domain does not contain hono"
        );

        assert!(
            dep_is_relevant(true, false, "hono", &domain),
            "a direct runtime dependency IS the user's stack"
        );
    }

    #[test]
    fn transitive_and_dev_dependencies_still_face_the_domain_filter() {
        let domain = live_domain();
        // Transitive: user never chose it individually.
        assert!(!dep_is_relevant(false, false, "hono", &domain));
        // Dev-only direct dep: declared, but not shipped.
        assert!(!dep_is_relevant(true, true, "hono", &domain));
        // In-domain transitive still passes on its own merits.
        assert!(dep_is_relevant(false, false, "axum", &domain));
    }

    #[test]
    fn a_gap_built_from_short_name_advisories_is_substantive() {
        // End of the chain: the panel only ships gaps carrying consequence, so
        // the short-name fix only matters if the resulting gap passes that bar.
        let candidates = vec![cand(
            192,
            "[CVE-2026-71850] Hono: memo() retains SSR output across requests",
            None,
        )];
        let gap = KnowledgeGap {
            dependency: "hono".to_string(),
            version: Some("4.13.2".to_string()),
            project_path: "d:/4da/mcp-4da-server".to_string(),
            missed_items: keyword_misses_from(&candidates, "hono"),
            gap_severity: GapSeverity::Critical,
            days_since_last_engagement: 13,
        };
        assert!(!gap.missed_items.is_empty());
        assert!(
            gap_is_substantive(&gap),
            "a CVE-bearing gap must reach the panel"
        );
    }

    // -----------------------------------------------------------------------
    // Already-patched suppression
    //
    // Widening the scan (four-character names, the raised cap, the direct-dep
    // exemption) makes `hono` reachable — and all three of its unread CVEs are
    // fixed in 4.12.34 against an installed 4.13.2 this repo had already pinned
    // past. Without a version check the widening would trade one false negative
    // for a false CRITICAL, which is a strictly worse deal on a security
    // surface.
    // -----------------------------------------------------------------------

    /// The real OSV ranges for the three Hono advisories, verbatim.
    const HONO_RANGES: &str =
        r#"[{"type":"SEMVER","events":[{"introduced":"3.8.0"},{"fixed":"4.12.34"}]}]"#;

    fn osv_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE osv_advisories (
                 advisory_id TEXT, package_name TEXT, ecosystem TEXT,
                 affected_ranges TEXT, withdrawn_at TEXT
             );",
        )
        .unwrap();
        conn
    }

    fn add_advisory(conn: &rusqlite::Connection, package: &str, ranges: &str) {
        conn.execute(
            "INSERT INTO osv_advisories (advisory_id, package_name, ecosystem, affected_ranges)
             VALUES ('GHSA-test', ?1, 'npm', ?2)",
            params![package, ranges],
        )
        .unwrap();
    }

    #[test]
    fn an_install_past_every_fix_is_not_still_vulnerable() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);
        assert!(
            !still_vulnerable(&conn, "hono", Some("4.13.2")),
            "4.13.2 is past the 4.12.34 fix — the user already remediated this"
        );
    }

    #[test]
    fn an_install_inside_the_range_is_still_vulnerable() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);
        assert!(still_vulnerable(&conn, "hono", Some("4.12.0")));
    }

    #[test]
    fn safety_is_never_claimed_on_missing_information() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);

        // Unknown installed version.
        assert!(still_vulnerable(&conn, "hono", None));
        // Package OSV knows nothing about.
        assert!(still_vulnerable(&conn, "some-unscanned-pkg", Some("1.0.0")));
        // Unreadable range.
        let conn2 = osv_conn();
        add_advisory(&conn2, "hono", "not json");
        assert!(still_vulnerable(&conn2, "hono", Some("4.13.2")));
        // No osv_advisories table at all (older database).
        let bare = rusqlite::Connection::open_in_memory().unwrap();
        assert!(still_vulnerable(&bare, "hono", Some("4.13.2")));
    }

    #[test]
    fn a_patched_dependency_does_not_escalate_to_critical() {
        let missed = vec![MissedItem {
            item_id: 192,
            title: "[CVE-2026-71850] Hono: memo() retains SSR output across requests".to_string(),
            url: None,
            source_type: "cve".to_string(),
            created_at: "2026-08-12 01:34:41".to_string(),
        }];

        assert_eq!(
            classify_severity(&missed, 13, "hono", true),
            GapSeverity::Critical,
            "a genuinely exposed install still escalates"
        );
        assert_ne!(
            classify_severity(&missed, 13, "hono", false),
            GapSeverity::Critical,
            "an already-patched install must not be reported as critical"
        );
    }
}
