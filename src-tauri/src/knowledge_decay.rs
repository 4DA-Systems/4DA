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

        // Each named project's install WITH its ecosystem (AD-045): the gap
        // merges projects by name, its judgements must not.
        let installs_here = installs_for(conn, &dep.package_name, &paths);
        let dep_lower = dep.package_name.to_lowercase();
        let live = |c: &GapCandidate| -> Option<bool> {
            if is_advisory_row(c) && linked_to(c, &dep_lower) {
                crate::osv::exposure::advisory_row_reaches(
                    conn,
                    &c.source_id,
                    &dep.package_name,
                    &installs_here,
                )
            } else {
                None
            }
        };

        // Unread items whose title names this dependency (word-boundary matched).
        let missed = keyword_misses_from(&candidates, &dep.package_name, &live);
        // A release every carrying project already runs is not a missed
        // update (AD-041): "@modelcontextprotocol/node v2.0.0: 1 version
        // update — notably npm: @modelcontextprotocol/node v2.0.0" against an
        // installed 2.0.0 (live 2026-09-08).
        let installed_here: Vec<String> = installs_here
            .iter()
            .map(|install| install.version.clone())
            .collect();
        let missed = drop_already_installed_releases(missed, &dep.package_name, &installed_here);
        if missed.is_empty() {
            continue;
        }

        // Check if user has engaged with any items about this dep
        let days_since = days_since_last_engagement(conn, &dep.package_name)?;

        // Classify severity. A security advisory only escalates the gap while
        // the installed version is genuinely still inside its affected range
        // AND a registry advisory the linker bound to this dependency is among
        // the citations — an editorial story naming the package is a citation,
        // never proof of exposure (2026-09-06). The tier it escalates TO is
        // the advisory's own (Phase 120).
        let vulnerable = still_vulnerable(conn, &dep.package_name, dep.version.as_deref(), &paths)
            && grounded_security_advisory(&candidates, &dep.package_name, &live);
        // Which of this dependency's projects actually carry the exposure.
        // `seen_deps` merges every project declaring the name (any version,
        // any ecosystem), so without this the gap said "relay (+1 more)"
        // for a bug only relay's copy has (2026-09-07).
        let exposed: Vec<(String, crate::osv::exposure::Install)> = if vulnerable {
            affected_project_paths(conn, &dep.package_name, &paths)
        } else {
            Vec::new()
        };
        let tier_installs: Vec<crate::osv::exposure::Install> = if exposed.is_empty() {
            dep.version
                .iter()
                .map(|v| crate::osv::exposure::Install::new(Some(dep.language.as_str()), v.clone()))
                .collect()
        } else {
            exposed.iter().map(|(_, install)| install.clone()).collect()
        };
        let severity = classify_severity(
            &missed,
            days_since,
            &dep.package_name,
            vulnerable,
            advisory_tier_for(conn, &dep.package_name, &tier_installs),
        );

        if severity == GapSeverity::Low && days_since < 14 {
            continue; // Skip low-severity recent items
        }

        // Project paths for display: the exposed subset when the exposure is
        // what makes this a gap, every declaring project otherwise.
        let (display_paths, version): (Vec<String>, Option<String>) = if exposed.is_empty() {
            (paths.clone(), dep.version.clone())
        } else {
            (
                exposed.iter().map(|(p, _)| p.clone()).collect(),
                exposed.first().map(|(_, install)| install.version.clone()),
            )
        };
        let project_display = if display_paths.len() == 1 {
            display_paths[0].clone()
        } else {
            format!("{} (+{} more)", display_paths[0], display_paths.len() - 1)
        };

        gaps.push(KnowledgeGap {
            dependency: dep.package_name.clone(),
            version,
            project_path: project_display,
            missed_items: missed,
            gap_severity: severity,
            days_since_last_engagement: days_since,
        });
    }

    let gaps = finalize_gaps(gaps);
    info!(
        target: "4da::knowledge_decay",
        gaps = gaps.len(),
        elapsed_ms = start.elapsed().as_millis() as u64,
        "Knowledge gap detection complete"
    );
    Ok(gaps)
}

/// Keep the substantive gaps, rank them, cap at 10 — in THAT order.
///
/// The cap used to come first and the substantive filter (in the command)
/// second. While a couple of gaps were High that was invisible; the moment
/// every gap sat at Medium (2026-09-06, after #620 made volume cap at
/// Medium) the ten slots filled with discussion-only gaps, the command then
/// dropped all ten as non-substantive, and the surface went empty while
/// `jsonwebtoken`'s unread authorization-bypass advisory sat in slot eleven.
fn finalize_gaps(mut gaps: Vec<KnowledgeGap>) -> Vec<KnowledgeGap> {
    gaps.retain(gap_is_substantive);
    // Severity first (critical first), then the longest-neglected.
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
    gaps
}

/// One unread item eligible to become a missed signal, with its title
/// pre-lowercased for matching.
struct GapCandidate {
    item: MissedItem,
    /// `source_items.source_id`. For an osv/cve row it is the advisory or CVE
    /// id the local mirror resolves for a LIVE version verdict (AD-045).
    source_id: String,
    content_type: Option<String>,
    /// Lowercased title. Precomputed because the word-boundary matcher is
    /// applied once per (candidate, dependency) pair — lowercasing inside that
    /// loop allocated a fresh `String` millions of times per pass.
    title_lower: String,
    /// Packages the dependency linker bound this item to with STRUCTURED
    /// proof (registry subject / advisory `Affected:`), lowercased. A registry
    /// advisory cites a dependency only through this list — live 2026-09-06,
    /// `hmac` was CRITICAL off a PHP Phalcon CVE and `hono` HIGH off
    /// `@hono/oauth-providers`, both on a title-word match.
    linked_packages: Vec<String>,
    /// The scoring pipeline's stored version verdict for this item:
    /// `Some(false)` = the installed version is CONFIRMED not affected, so the
    /// advisory is not a gap at all (`lettre` sat HIGH at its fixed version).
    version_affected: Option<bool>,
}

/// A registry advisory row (osv / cve): its only honest link to a dependency
/// is the linker's `Affected:` proof, never the title.
fn is_advisory_row(c: &GapCandidate) -> bool {
    matches!(c.item.source_type.as_str(), "osv" | "cve")
}

/// The linker bound this item to `dep_lower` with structured proof.
fn linked_to(c: &GapCandidate, dep_lower: &str) -> bool {
    let want = dep_lower.replace('_', "-");
    c.linked_packages
        .iter()
        .any(|p| p.replace('_', "-") == want)
}

/// Whether ANY candidate is a registry advisory that the linker bound to this
/// dependency and whose installed version is not confirmed clear — the only
/// evidence that may escalate a gap to Critical. An editorial story that
/// names the package is a citation, not proof of exposure.
fn grounded_security_advisory(
    candidates: &[GapCandidate],
    package_name: &str,
    live: LiveVerdict<'_>,
) -> bool {
    let dep_lower = package_name.to_lowercase();
    candidates.iter().any(|c| {
        is_advisory_row(c)
            && linked_to(c, &dep_lower)
            && live(c).or(c.version_affected) != Some(false)
    })
}

/// A LIVE version verdict for a candidate against one dependency's installs:
/// `Some` when the local mirror can judge the row (AD-045), `None` to fall
/// back to the scoring pipeline's stored verdict. The stored verdict goes
/// stale the moment the user upgrades: live 2026-09-10, `hono` 4.13.5 read
/// "3 unread security advisories" for three advisories fixed IN 4.13.5,
/// because the rows were scored while 4.13.3 was installed.
type LiveVerdict<'a> = &'a dyn Fn(&GapCandidate) -> Option<bool>;

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
    // Two scalar subqueries ride along: the linker's structured package links
    // (registry subject / advisory `Affected:` only) and the scoring
    // pipeline's stored version verdict for the row.
    let mut stmt = conn.prepare(
        "SELECT si.id, si.title, si.url, si.source_type, si.created_at, si.content_type,
                (SELECT GROUP_CONCAT(LOWER(sid.package_name), ',')
                   FROM source_item_dependencies sid
                  WHERE sid.source_item_id = si.id
                    AND sid.match_type IN ('exact_registry', 'advisory')),
                (SELECT json_extract(se.breakdown, '$.breakdown.is_version_affected')
                   FROM scoring_explanations se
                  WHERE se.source_item_id = si.id),
                si.source_id
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
            let linked: Option<String> = row.get(6)?;
            let version_affected: Option<i64> = row.get(7)?;
            Ok(GapCandidate {
                title_lower: title.to_lowercase(),
                item: MissedItem {
                    item_id: row.get(0)?,
                    title,
                    url: row.get(2)?,
                    source_type: row.get(3)?,
                    created_at: row.get(4)?,
                },
                source_id: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                content_type: row.get::<_, Option<String>>(5)?,
                linked_packages: linked
                    .map(|s| s.split(',').map(str::to_string).collect())
                    .unwrap_or_default(),
                version_affected: version_affected.map(|v| v != 0),
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
/// Drop rows that announce a version every project in this gap already runs
/// (or a lower one). Unknown installs drop nothing — a release we cannot
/// compare is still worth a look.
fn drop_already_installed_releases(
    missed: Vec<MissedItem>,
    dep_name: &str,
    installed: &[String],
) -> Vec<MissedItem> {
    use crate::scoring::release_version::{
        already_installed, announced_release_version, lenient_semver,
    };
    let installed: Vec<semver::Version> = installed
        .iter()
        .filter_map(|v| lenient_semver(v, None))
        .collect();
    if installed.is_empty() {
        return missed;
    }
    missed
        .into_iter()
        .filter(|m| {
            announced_release_version(&m.title, &m.source_type, dep_name)
                .is_none_or(|announced| !already_installed(&announced, &installed))
        })
        .collect()
}

fn keyword_misses_from(
    candidates: &[GapCandidate],
    package_name: &str,
    live: LiveVerdict<'_>,
) -> Vec<MissedItem> {
    let dep_lower = package_name.to_lowercase();

    // Deduplicate by normalized title (first 10 words, lowercased, stripped punctuation)
    let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();
    candidates
        .iter()
        // Cheap substring reject first; the boundary walk only runs on hits.
        .filter(|c| c.title_lower.contains(&dep_lower))
        .filter(|c| crate::utils::has_word_boundary_match_with_ext(&c.title_lower, &dep_lower))
        // A registry advisory cites a dependency only through the linker's
        // `Affected:` proof, never a title word (2026-09-06).
        .filter(|c| !is_advisory_row(c) || linked_to(c, &dep_lower))
        // A resolved advisory (every install past its fix, or another
        // ecosystem's package) is not a gap: judged LIVE against this
        // dependency's installs where the mirror can, else by the stored
        // scoring-time verdict. Runs after the cheap filters, so the live
        // lookup only ever sees rows already bound to this dependency.
        .filter(|c| live(c).or(c.version_affected) != Some(false))
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

fn quality_weight(m: &MissedItem, dep_name: &str) -> f32 {
    match classify_missed_item(&m.title, &m.source_type, dep_name) {
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
/// Installed versions of `package` in the projects this gap is actually about.
///
/// The gap scan iterates `project_dependencies`, which records what a MANIFEST
/// declares — and its `version` column is NULL for every row in practice
/// (measured live: 245 of 245), because versions are resolved from lockfiles
/// into `user_dependencies`. Reading only the manifest table therefore handed
/// `still_vulnerable` a `None` every single time, which took the conservative
/// branch and made the version check inert: `hono` shipped as CRITICAL on
/// 4.13.2 against advisories fixed in 4.12.34.
///
/// Scoped to `project_paths` on purpose. A package can sit at different
/// versions in different checkouts — `hono` is 4.13.2 in `mcp-4da-server` but
/// 4.9.10 and 4.11.1 in two unrelated repos — and a gap that names one project
/// must be judged on THAT project's install, not on the worst copy anywhere on
/// the machine.
///
/// Each version keeps its ECOSYSTEM (AD-045). A
/// gap merges every project declaring the NAME, and `jsonwebtoken` in a Rust
/// project and in an npm project are two packages with unrelated advisories:
/// each install must be judged only against its own ecosystem's records.
fn installs_for(
    conn: &rusqlite::Connection,
    package: &str,
    project_paths: &[String],
) -> Vec<crate::osv::exposure::Install> {
    if project_paths.is_empty() {
        return Vec::new();
    }
    let Ok(mut stmt) = conn.prepare(
        "SELECT project_path, version, ecosystem FROM user_dependencies
         WHERE lower(package_name) = lower(?1) AND version IS NOT NULL AND version != ''",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map(params![package], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    }) else {
        return Vec::new();
    };

    let wanted: Vec<String> = project_paths
        .iter()
        .map(|p| normalize_project_path(p))
        .collect();

    rows.flatten()
        .filter(|(path, _, _)| {
            let p = normalize_project_path(path);
            wanted.iter().any(|w| &p == w)
        })
        .map(|(_, version, ecosystem)| {
            crate::osv::exposure::Install::new(ecosystem.as_deref(), version)
        })
        .collect()
}

/// Is the installed version of `package` still inside ANY stored advisory's
/// affected range, for the projects this gap names?
///
/// Conservative in every direction: no advisories stored for the package, an
/// unreadable range, or NO resolvable installed version all count as STILL
/// VULNERABLE. Safety is never claimed on missing information.
pub(crate) fn still_vulnerable(
    conn: &rusqlite::Connection,
    package: &str,
    version: Option<&str>,
    project_paths: &[String],
) -> bool {
    // Prefer the version the caller already has (its ecosystem is not known,
    // so it is judged against every stored record); otherwise resolve each
    // named project's install WITH its ecosystem from the lockfile table.
    let installs: Vec<crate::osv::exposure::Install> = match version {
        Some(v) if !v.trim().is_empty() => vec![crate::osv::exposure::Install::new(None, v)],
        _ => installs_for(conn, package, project_paths),
    };
    installs_still_vulnerable(conn, package, &installs)
}

/// Is any of `installs` inside the affected range of a stored advisory for its
/// OWN ecosystem (AD-045)? A `jsonwebtoken` npm install at 9.0.3 is not exposed
/// by the crates.io advisory whose range ends at 10.3.0.
///
/// Conservative in every direction: no installs, no readable advisory table,
/// or no advisory stored for any install's ecosystem all count as STILL
/// VULNERABLE. Safety is never claimed on missing information.
pub(crate) fn installs_still_vulnerable(
    conn: &rusqlite::Connection,
    package: &str,
    installs: &[crate::osv::exposure::Install],
) -> bool {
    if installs.is_empty() {
        return true; // no version anywhere -> cannot prove safety
    }

    let Ok(mut stmt) = conn.prepare(
        "SELECT affected_ranges, ecosystem FROM osv_advisories
         WHERE lower(package_name) = lower(?1) AND withdrawn_at IS NULL",
    ) else {
        return true;
    };
    let Ok(rows) = stmt.query_map(params![package], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
        ))
    }) else {
        return true;
    };

    let mut saw_relevant = false;
    for (ranges, ecosystem) in rows.flatten() {
        let advisory_ecosystem = ecosystem
            .as_deref()
            .and_then(crate::osv::exposure::canonical);
        // ANY installed copy still in range of ITS OWN ecosystem's advisory
        // keeps the whole gap live.
        for install in installs {
            if let (Some(a), Some(b)) = (install.ecosystem, advisory_ecosystem) {
                if a != b {
                    continue;
                }
            }
            saw_relevant = true;
            let (affected, _confirmed) = crate::osv::matching::check_version_affected(
                Some(install.version.as_str()),
                &ranges,
            );
            if affected {
                return true;
            }
        }
    }

    // Nothing stored for this package in its ecosystem — say nothing about its safety.
    !saw_relevant
}

/// Does any missed item cite a security advisory ABOUT this dependency?
/// The same classifier the weights and the substantive filter use — the old
/// ad-hoc list knew "cve" but not "ghsa", so `[GHSA-h395-gr6q-cpjc]
/// jsonwebtoken: … authorization bypass` was never a security citation here
/// while it weighed 3.0 two lines down (2026-09-06).
fn has_security_citation(missed: &[MissedItem], dep_name: &str) -> bool {
    let dep_lower = dep_name.to_lowercase();
    missed.iter().any(|item| {
        let title_lower = item.title.to_lowercase();
        (classify_missed_item(&item.title, &item.source_type, dep_name) == "security advisory"
            || title_lower.contains("security")
            || title_lower.contains("exploit"))
            && title_lower.contains(&dep_lower)
    })
}

fn classify_severity(
    missed: &[MissedItem],
    days_since: u32,
    dep_name: &str,
    still_vulnerable: bool,
    advisory_tier: Option<&str>,
) -> GapSeverity {
    let dep_lower = dep_name.to_lowercase();

    let has_security = has_security_citation(missed, dep_name);

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
    let weighted_score: f32 = missed.iter().map(|m| quality_weight(m, dep_name)).sum();
    let days_factor = if days_since >= 999 {
        1.5
    } else if days_since > 30 {
        1.2
    } else {
        1.0
    };
    let gap_score = weighted_score * days_factor;

    // A security advisory escalates only while the install is still exposed,
    // and then to the tier the ADVISORY carries — the same CVSS-band-or-
    // curated-label every surface reads (Phase 120). Live 2026-09-07, the
    // jsonwebtoken gap said Critical for a GitHub-MODERATE type-confusion
    // bug that Preemption and the MCP both called medium: one advisory,
    // three severities. A patched install with a security citation is no
    // longer "High" either: an advisory you already carry the fix for is
    // reading material, graded on volume like any other discussion.
    // Consequence, not volume, decides the tiers above Medium: a breaking
    // citation is High; any number of discussions and version mentions caps
    // at Medium. Live 2026-09-06, `stripe` reached High on five editorial
    // mentions plus a mastodon post about ANOTHER product's release
    // ("onecli v2.5.0 — Added Slack Stripe AWS billing fixes"), multiplied by
    // the never-engaged ×1.5 factor — unread volume masquerading as urgency,
    // the same class Blind Spots caps at Medium.
    if has_security && still_vulnerable {
        match advisory_tier {
            Some("critical") | Some("high") => GapSeverity::Critical,
            _ => GapSeverity::High,
        }
    } else if has_breaking {
        GapSeverity::High
    } else if gap_score >= 2.0 || days_since > 14 {
        GapSeverity::Medium
    } else {
        GapSeverity::Low
    }
}

/// The severity tier of the most severe stored advisory that still affects
/// one of `versions` — CVSS band first, the source's curated label second
/// (`None` when nothing affecting is graded). Mirrors
/// `osv::identity::cluster_severity_tier` for rows read straight from the
/// mirror.
fn advisory_tier_for(
    conn: &rusqlite::Connection,
    package: &str,
    installs: &[crate::osv::exposure::Install],
) -> Option<&'static str> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT affected_ranges, cvss_score, severity_label, ecosystem FROM osv_advisories
         WHERE lower(package_name) = lower(?1) AND withdrawn_at IS NULL",
    ) else {
        return None;
    };
    let Ok(rows) = stmt.query_map(params![package], |row| {
        let ecosystem: Option<String> = row.get(3)?;
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<f64>>(1)?,
            row.get::<_, Option<String>>(2)?,
            ecosystem
                .as_deref()
                .and_then(crate::osv::exposure::canonical),
        ))
    }) else {
        return None;
    };
    let rank = |t: &str| match t {
        "critical" => 0u8,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    };
    let mut best: Option<&'static str> = None;
    for (ranges, cvss, label, advisory_ecosystem) in rows.flatten() {
        // Only an advisory for an install's OWN ecosystem can grade it (AD-045).
        let affects = installs.iter().any(|install| {
            let same_ecosystem = match (install.ecosystem, advisory_ecosystem) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            };
            same_ecosystem
                && crate::osv::matching::check_version_affected(
                    Some(install.version.as_str()),
                    &ranges,
                )
                .0
        });
        if !affects {
            continue;
        }
        let tier: Option<&'static str> = match (cvss, label.as_deref()) {
            (Some(score), _) => Some(crate::osv::types::cvss_band(score)),
            (None, Some("critical")) => Some("critical"),
            (None, Some("high")) => Some("high"),
            (None, Some("medium")) => Some("medium"),
            (None, Some("low")) => Some("low"),
            _ => None,
        };
        if let Some(t) = tier {
            if best.is_none_or(|b| rank(t) < rank(b)) {
                best = Some(t);
            }
        }
    }
    best
}

/// The subset of `project_paths` whose installed copy of `package` is inside
/// a stored advisory range. Empty when nothing can be decided (no versions,
/// no ranges) — the caller then keeps every path rather than claim safety.
/// Live 2026-09-07: the jsonwebtoken gap named "relay (+1 more)" for a bug
/// only relay's 9.3.1 carries; the other project runs the fixed 10.4.0.
fn affected_project_paths(
    conn: &rusqlite::Connection,
    package: &str,
    project_paths: &[String],
) -> Vec<(String, crate::osv::exposure::Install)> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT affected_ranges, ecosystem FROM osv_advisories
         WHERE lower(package_name) = lower(?1) AND withdrawn_at IS NULL",
    ) else {
        return Vec::new();
    };
    let advisories: Vec<(Option<String>, Option<&'static str>)> = stmt
        .query_map(params![package], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })
        .map(|rows| {
            rows.flatten()
                .map(|(ranges, ecosystem)| {
                    (
                        ranges,
                        ecosystem
                            .as_deref()
                            .and_then(crate::osv::exposure::canonical),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    if advisories.is_empty() {
        return Vec::new();
    }
    project_paths
        .iter()
        .filter_map(|path| {
            let install = installs_for(conn, package, std::slice::from_ref(path))
                .into_iter()
                .next()?;
            // Only this project's OWN ecosystem's advisories expose it (AD-045).
            let affected = advisories.iter().any(|(ranges, advisory_ecosystem)| {
                let same_ecosystem = match (install.ecosystem, *advisory_ecosystem) {
                    (Some(a), Some(b)) => a == b,
                    _ => true,
                };
                same_ecosystem
                    && crate::osv::matching::check_version_affected(
                        Some(install.version.as_str()),
                        ranges,
                    )
                    .0
            });
            affected.then(|| (path.clone(), install))
        })
        .collect()
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
    // Schema: ≤ 120 chars, no trailing period. `truncate_display` counts its
    // ellipsis against the budget, so the cap still holds.
    crate::utils::truncate_display(s.trim_end_matches('.'), 120)
}

fn truncate_gap_note(s: &str) -> String {
    // Citation relevance_note schema cap: 200 chars.
    crate::utils::truncate_display(s, 200)
}

/// What a missed item IS, for the gap's counts, citation notes, highlight and
/// actions. The SOURCE decides first: a registry row (`crates.io: stripe
/// v22.6.1`, `npm: …`) is a version update whatever its title words, and an
/// osv/cve row is a security advisory. Only editorial rows fall through to
/// the title, and a "version update" there must announce a version OF THIS
/// dependency — a title merely containing the word "update" is not one.
/// Live 2026-09-07: the stripe gap read "1 version update … notably '📢 New
/// updates · onecli v2.5.0 — Added Slack Stripe AWS billing fixes'" — a
/// Mastodon post about another product classified by the word "updates" —
/// while the actual `npm: stripe v22.6.1` row was filed as a discussion.
fn classify_missed_item(title: &str, source_type: &str, dep_name: &str) -> &'static str {
    if matches!(source_type, "osv" | "cve") {
        return "security advisory";
    }
    if crate::dep_linker::is_registry_source(source_type) {
        // A registry row is a release of its SUBJECT. One for another crate
        // that merely mentions this name (`code-split-plugin-typescript
        // v1.0.0-alpha.4` cited on a typescript gap) is a passing mention.
        return match crate::dep_linker::registry_title_subject(title) {
            Some((subject, _)) if crate::dep_linker::registry_names_equal(&subject, dep_name) => {
                "version update"
            }
            _ => "relevant discussion",
        };
    }
    let lower = title.to_lowercase();
    if lower.contains("cve")
        || lower.contains("ghsa")
        || lower.contains("rustsec")
        || lower.contains("pysec")
        || lower.contains("vulnerability")
    {
        "security advisory"
    } else if lower.contains("breaking") || lower.contains("deprecated") || lower.contains("eol") {
        "breaking change"
    } else if title_announces_version_of(&lower, dep_name) {
        "version update"
    } else if lower.contains("rfc") || lower.contains("proposal") || lower.contains("roadmap") {
        "roadmap signal"
    } else {
        "relevant discussion"
    }
}

/// Does the (lowercased) title announce a version of `dep_name` — the
/// dependency as a whole word followed, within a few tokens, by a version
/// literal ("axum 0.8.0", "typescript v7.0", "Announcing tokio 1.53")?
/// "onecli v2.5.0 … Stripe AWS billing fixes" does not: the version belongs
/// to another product.
fn title_announces_version_of(lower_title: &str, dep_name: &str) -> bool {
    let dep = dep_name.to_lowercase();
    if dep.is_empty() {
        return false;
    }
    // `-` and `_` are name characters here: "react-query v5" announces
    // react-query, not react; "code-split-plugin-typescript v1" is not a
    // typescript release.
    let is_boundary =
        |c: Option<char>| c.is_none_or(|ch| !(ch.is_alphanumeric() || ch == '_' || ch == '-'));
    for (pos, _) in lower_title.match_indices(&dep) {
        // `match_indices` yields char-boundary offsets; `get` keeps the
        // slices panic-free regardless.
        let before = lower_title.get(..pos).and_then(|s| s.chars().next_back());
        let after_str = lower_title.get(pos + dep.len()..).unwrap_or("");
        let after = after_str.chars().next();
        if !is_boundary(before) || !is_boundary(after) {
            continue;
        }
        // A version literal in the next ~24 characters: v?\d+\.\d+
        let window: String = after_str.chars().take(24).collect();
        if window_has_version_literal(&window) {
            return true;
        }
    }
    false
}

fn window_has_version_literal(window: &str) -> bool {
    let bytes: Vec<char> = window.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len()
                && bytes[j] == '.'
                && j + 1 < bytes.len()
                && bytes[j + 1].is_ascii_digit()
            {
                return true;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    false
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
            classify_missed_item(&m.title, &m.source_type, &gap.dependency),
            "security advisory" | "breaking change" | "version update"
        )
    })
}

fn missed_item_to_citation(m: &MissedItem, dep_name: &str) -> EvidenceCitation {
    let freshness_days = chrono::NaiveDateTime::parse_from_str(&m.created_at, "%Y-%m-%d %H:%M:%S")
        .map(|dt| {
            let secs = chrono::Utc::now().timestamp() - dt.and_utc().timestamp();
            (secs as f32 / 86_400.0).max(0.0)
        })
        .unwrap_or(0.0);
    let category = classify_missed_item(&m.title, &m.source_type, dep_name);
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
        match classify_missed_item(&m.title, &m.source_type, dep) {
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
            let c = classify_missed_item(&m.title, &m.source_type, dep);
            c == "security advisory" || c == "breaking change"
        })
        .or_else(|| {
            missed
                .iter()
                .find(|m| classify_missed_item(&m.title, &m.source_type, dep) == "version update")
        })
        .or_else(|| missed.first());

    let mut explanation = format!("{dep}{ver}: {categories} · {recency}");

    if let Some(item) = highlight {
        let short_title = crate::utils::truncate_display(&item.title, 80);
        explanation.push_str(&format!(" — notably \"{short_title}\""));
    }

    explanation
}

fn build_gap_actions(missed: &[MissedItem], dep: &str) -> Vec<EvidenceAction> {
    let mut actions = Vec::with_capacity(3);
    let has_security = missed
        .iter()
        .any(|m| classify_missed_item(&m.title, &m.source_type, dep) == "security advisory");
    let has_breaking = missed
        .iter()
        .any(|m| classify_missed_item(&m.title, &m.source_type, dep) == "breaking change");
    let has_update = missed
        .iter()
        .any(|m| classify_missed_item(&m.title, &m.source_type, dep) == "version update");

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
            .map(|m| missed_item_to_citation(m, &self.dependency))
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
            evidence_total: None,
            affected_projects: vec![self.project_path.clone()],
            affected_deps: vec![self.dependency.clone()],
            suggested_actions: build_gap_actions(&self.missed_items, &self.dependency),
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
                // Decay gaps track ENGAGEMENT drift, not signal availability —
                // the zero-signal-coverage classification never applies here.
                no_coverage: false,
                // Host reachability is a property of an ADVISORY against an
                // installed crate; a decay gap is about the user's reading,
                // so neither 2026-09-07 hint applies here.
                lockfile_only: false,
                dormant_notice: false,
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

    /// 2026-09-06: the ten-gap cap ran BEFORE the substantive filter. With
    /// every gap at Medium the cap filled with discussion-only gaps, the
    /// command dropped all ten, and the surface went empty while the
    /// jsonwebtoken authorization-bypass advisory sat in slot eleven.
    #[test]
    fn finalize_gaps_keeps_substantive_gaps_over_louder_noise() {
        let discussion_gap = |i: usize| KnowledgeGap {
            dependency: format!("chatter-{i}"),
            version: None,
            project_path: "/proj/a".to_string(),
            missed_items: vec![MissedItem {
                item_id: i as i64,
                title: format!("Why we chose chatter-{i} for our side project"),
                url: None,
                source_type: "devto".to_string(),
                created_at: "2026-09-04 00:00:00".to_string(),
            }],
            gap_severity: GapSeverity::Medium,
            days_since_last_engagement: 999,
        };
        let mut gaps: Vec<KnowledgeGap> = (0..12).map(discussion_gap).collect();
        gaps.push(KnowledgeGap {
            dependency: "jsonwebtoken".to_string(),
            version: Some("9.3.1".to_string()),
            project_path: "d:/4da/relay".to_string(),
            missed_items: vec![MissedItem {
                item_id: 71038,
                title: "[GHSA-h395-gr6q-cpjc] jsonwebtoken: Type Confusion leads to authorization bypass"
                    .to_string(),
                url: None,
                source_type: "osv".to_string(),
                created_at: "2026-09-04 03:08:44".to_string(),
            }],
            gap_severity: GapSeverity::Medium,
            days_since_last_engagement: 999,
        });

        let kept = finalize_gaps(gaps);
        assert_eq!(
            kept.len(),
            1,
            "discussion-only gaps ship silent, whatever their rank"
        );
        assert_eq!(kept[0].dependency, "jsonwebtoken");
    }

    /// A GHSA-, RUSTSEC- or PYSEC-titled advisory is a security citation for
    /// the tier decision, exactly as it is for the weights.
    #[test]
    fn registry_prefixed_advisories_are_security_citations() {
        for title in [
            "[GHSA-h395-gr6q-cpjc] jsonwebtoken: Type Confusion leads to authorization bypass",
            "[RUSTSEC-2026-0007] jsonwebtoken: header parsing panic",
            "[PYSEC-2026-12] jsonwebtoken: algorithm confusion",
        ] {
            let missed = vec![MissedItem {
                item_id: 1,
                title: title.to_string(),
                url: None,
                source_type: "osv".to_string(),
                created_at: "2026-09-04 00:00:00".to_string(),
            }];
            assert_eq!(
                classify_severity(&missed, 999, "jsonwebtoken", false, None),
                GapSeverity::Medium,
                "{title}: an unread advisory for a PATCHED install is reading material, graded on volume"
            );
            assert_eq!(
                classify_severity(&missed, 999, "jsonwebtoken", true, None),
                GapSeverity::High,
                "{title}: exposed but ungraded by the source → High, never invented Critical"
            );
            assert_eq!(
                classify_severity(&missed, 999, "jsonwebtoken", true, Some("high")),
                GapSeverity::Critical,
                "{title}: exposed to a HIGH advisory → Critical"
            );
            assert_eq!(
                classify_severity(&missed, 999, "jsonwebtoken", true, Some("medium")),
                GapSeverity::High,
                "{title}: exposed to a MODERATE advisory is High — the tier every surface shares"
            );
        }
    }

    /// Phase 120: the SOURCE classifies before the title does. The live stripe
    /// gap cited a Mastodon post about another product as its "version
    /// update" (the word "updates") while `npm: stripe v22.6.1` was filed as a
    /// discussion.
    #[test]
    fn registry_rows_are_version_updates_and_other_products_versions_are_not() {
        assert_eq!(
            classify_missed_item("npm: stripe v22.6.1", "npm_registry", "stripe"),
            "version update"
        );
        assert_eq!(
            classify_missed_item(
                "crates.io: jsonwebtoken v11.0.0",
                "crates_io",
                "jsonwebtoken"
            ),
            "version update"
        );
        assert_eq!(
            classify_missed_item(
                "📢 New updates · 3 Sept #15.1 · onecli v2.5.0 — Added Slack Stripe AWS billing fixes",
                "mastodon",
                "stripe"
            ),
            "relevant discussion",
            "a version of ANOTHER product is not a stripe version update"
        );
        assert_eq!(
            classify_missed_item("Announcing axum 0.8.0", "rss", "axum"),
            "version update",
            "an editorial announcement of THIS dependency's version still counts"
        );
        assert_eq!(
            classify_missed_item("[GHSA-1] stripe: signature bypass", "osv", "stripe"),
            "security advisory"
        );
        assert_eq!(
            classify_missed_item("Stripe Payment Cloaking", "reddit", "stripe"),
            "relevant discussion"
        );
    }

    /// Live 2026-09-08: "@modelcontextprotocol/node v2.0.0: 1 version update
    /// — notably npm: @modelcontextprotocol/node v2.0.0" against an
    /// installed 2.0.0. A release every carrying project already runs is
    /// not a missed update (AD-041); one project behind keeps it; an unknown
    /// install drops nothing.
    #[test]
    fn releases_every_project_already_runs_are_not_missed_updates() {
        let missed = |titles: &[(&str, &str)]| -> Vec<MissedItem> {
            titles
                .iter()
                .enumerate()
                .map(|(i, (title, source))| MissedItem {
                    item_id: i as i64 + 1,
                    title: (*title).to_string(),
                    url: None,
                    source_type: (*source).to_string(),
                    created_at: "2026-09-08 00:00:00".to_string(),
                })
                .collect()
        };
        let rows = || {
            missed(&[
                ("npm: @modelcontextprotocol/node v2.0.0", "npm_registry"),
                ("npm: @modelcontextprotocol/node v2.1.0", "npm_registry"),
                ("Announcing @modelcontextprotocol/node 1.9.0", "rss"),
                ("Why @modelcontextprotocol/node matters", "devto"),
            ])
        };
        let dep = "@modelcontextprotocol/node";
        let kept: Vec<String> = drop_already_installed_releases(rows(), dep, &["2.0.0".into()])
            .into_iter()
            .map(|m| m.title)
            .collect();
        assert_eq!(
            kept,
            vec![
                "npm: @modelcontextprotocol/node v2.1.0".to_string(),
                "Why @modelcontextprotocol/node matters".to_string(),
            ],
            "the installed 2.0.0 and the older 1.9.0 announcement are not missed updates"
        );
        let behind =
            drop_already_installed_releases(rows(), dep, &["1.8.0".into(), "2.0.0".into()]);
        assert_eq!(
            behind.len(),
            4,
            "one project on 1.8.0 keeps every release new"
        );
        let unknown = drop_already_installed_releases(rows(), dep, &[]);
        assert_eq!(unknown.len(), 4, "an unknown install drops nothing");
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
        // The historical fixtures are cve rows the linker had bound to `hono`
        // / `next` (the names under test) — the shape a real advisory row has.
        let mut candidate = cand_from(id, title, "cve", &["hono", "next"], None);
        candidate.content_type = content_type.map(str::to_string);
        candidate
    }

    fn cand_from(
        id: i64,
        title: &str,
        source_type: &str,
        linked: &[&str],
        version_affected: Option<bool>,
    ) -> GapCandidate {
        GapCandidate {
            title_lower: title.to_lowercase(),
            item: MissedItem {
                item_id: id,
                title: title.to_string(),
                url: None,
                source_type: source_type.to_string(),
                created_at: "2026-08-12 01:34:41".to_string(),
            },
            source_id: String::new(),
            content_type: None,
            linked_packages: linked.iter().map(|s| s.to_string()).collect(),
            version_affected,
        }
    }

    /// No live mirror verdict: the historical fixtures exercise the stored
    /// scoring-time verdict on its own.
    fn no_live(_: &GapCandidate) -> Option<bool> {
        None
    }

    // -----------------------------------------------------------------------
    // Citation grounding (2026-09-06 live audit): three of five knowledge gaps
    // were wrong — `hmac` CRITICAL off a PHP Phalcon CVE and a Mastodon post,
    // `hono` HIGH off `@hono/oauth-providers`, `lettre` HIGH while installed
    // at the fixed version.
    // -----------------------------------------------------------------------

    #[test]
    fn a_registry_advisory_cites_a_dependency_only_through_the_linkers_proof() {
        let candidates = vec![
            // Names hmac in the title; the linker bound it to phalcon (PHP).
            cand_from(
                1,
                "[CVE-2026-54736] Phalcon: Non-constant-time HMAC verification",
                "cve",
                &["phalcon"],
                None,
            ),
            // Names hmac AND the linker bound it to hmac.
            cand_from(
                2,
                "[RUSTSEC-2026-0100] hmac: timing leak in verify_slice",
                "cve",
                &["hmac"],
                None,
            ),
            // An advisory about a namesake package (`@hono/oauth-providers`).
            cand_from(
                3,
                "[GHSA-x] @hono/oauth-providers: token leak",
                "osv",
                &["hono/oauth-providers"],
                None,
            ),
        ];
        let ids: Vec<i64> = keyword_misses_from(&candidates, "hmac", &no_live)
            .iter()
            .map(|m| m.item_id)
            .collect();
        assert_eq!(ids, vec![2], "only the linker-bound advisory cites hmac");
        assert!(
            keyword_misses_from(&candidates, "hono", &no_live).is_empty(),
            "@hono/oauth-providers is not hono"
        );
        assert!(grounded_security_advisory(&candidates, "hmac", &no_live));
        assert!(!grounded_security_advisory(&candidates, "hono", &no_live));
    }

    #[test]
    fn a_confirmed_not_affected_advisory_is_not_a_gap() {
        let candidates = vec![
            cand_from(
                10,
                "[CVE-2026-46428] lettre: header injection",
                "cve",
                &["lettre"],
                Some(false),
            ),
            cand_from(
                11,
                "[CVE-2026-46429] lettre: SMTP smuggling",
                "cve",
                &["lettre"],
                Some(true),
            ),
            cand_from(
                12,
                "[CVE-2026-46430] lettre: TLS downgrade",
                "cve",
                &["lettre"],
                None,
            ),
        ];
        let ids: Vec<i64> = keyword_misses_from(&candidates, "lettre", &no_live)
            .iter()
            .map(|m| m.item_id)
            .collect();
        assert_eq!(ids, vec![11, 12], "the resolved advisory is not a gap");
        assert!(grounded_security_advisory(&candidates, "lettre", &no_live));
        let resolved_only = vec![cand_from(
            10,
            "[CVE-2026-46428] lettre: header injection",
            "cve",
            &["lettre"],
            Some(false),
        )];
        assert!(
            !grounded_security_advisory(&resolved_only, "lettre", &no_live),
            "a resolved advisory never escalates a gap"
        );
    }

    /// Consequence, not volume, decides the tiers above Medium. Five
    /// discussions and a "version update" that is really another product's
    /// release note, on a never-engaged dependency, used to reach High
    /// (5 × 0.5 + 1.5 = 3.5, × 1.5 = 5.25 ≥ 5.0) — the live `stripe` gap.
    #[test]
    fn discussion_volume_never_makes_a_gap_high() {
        let missed = |titles: &[&str]| -> Vec<MissedItem> {
            titles
                .iter()
                .enumerate()
                .map(|(i, t)| MissedItem {
                    item_id: i as i64 + 1,
                    title: (*t).to_string(),
                    url: None,
                    source_type: "devto".to_string(),
                    created_at: "2026-09-04 00:00:00".to_string(),
                })
                .collect()
        };
        let volume = missed(&[
            "Your AI builder shipped the Stripe code in an afternoon",
            "How I built an AI Line Art SaaS with Next.js and Stripe credits",
            "What a New Stripe Account Can't Do Yet",
            "npm: stripe v22.6.1",
            "New updates 3 Sept: onecli v2.5.0 — Added Slack Stripe AWS billing fixes",
        ]);
        assert_eq!(
            classify_severity(&volume, 999, "stripe", true, None),
            GapSeverity::Medium,
            "unread volume on a never-engaged dependency caps at Medium"
        );
        let breaking = missed(&["Stripe API breaking changes in the 2026 release"]);
        assert_eq!(
            classify_severity(&breaking, 10, "stripe", true, None),
            GapSeverity::High,
            "a breaking-change citation is High on its own"
        );
        let advisory = missed(&["[CVE-2026-1] stripe: webhook signature bypass vulnerability"]);
        assert_eq!(
            classify_severity(&advisory, 10, "stripe", false, None),
            GapSeverity::Medium,
            "a security citation on a patched install is reading material, never High or Critical"
        );
        assert_eq!(
            classify_severity(&advisory, 10, "stripe", true, Some("critical")),
            GapSeverity::Critical
        );
    }

    #[test]
    fn an_editorial_security_story_is_a_citation_but_not_proof_of_exposure() {
        let candidates = vec![cand_from(
            20,
            "Critical vulnerability found in axum",
            "hackernews",
            &[],
            None,
        )];
        let missed = keyword_misses_from(&candidates, "axum", &no_live);
        assert_eq!(missed.len(), 1, "the story still cites axum");
        assert!(
            !grounded_security_advisory(&candidates, "axum", &no_live),
            "a story is not a registry advisory"
        );
        // The call site ANDs `still_vulnerable` with the grounding check, so
        // an ungrounded story reaches the classifier as "not proven exposed".
        let severity = classify_severity(&missed, 20, "axum", false, Some("critical"));
        assert_ne!(
            severity,
            GapSeverity::Critical,
            "without grounded proof a security story never makes a gap Critical"
        );
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

        let missed = keyword_misses_from(&candidates, "hono", &no_live);
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
        let missed = keyword_misses_from(&candidates, "next", &no_live);
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

        let missed = keyword_misses_from(&candidates, "hono", &no_live);
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
            missed_items: keyword_misses_from(&candidates, "hono", &no_live),
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
             );
             CREATE TABLE user_dependencies (
                 project_path TEXT, package_name TEXT, version TEXT, ecosystem TEXT
             );",
        )
        .unwrap();
        conn
    }

    /// Mirror of the live layout: `project_dependencies` carries the manifest
    /// entry with a NULL version, and `user_dependencies` carries the resolved
    /// one per checkout.
    fn add_installed(conn: &rusqlite::Connection, project: &str, package: &str, version: &str) {
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem)
             VALUES (?1, ?2, ?3, 'javascript')",
            params![project, package, version],
        )
        .unwrap();
    }

    /// These cases pass an explicit version, so project scope never applies.
    const NO_PROJECTS: &[String] = &[];

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
            !still_vulnerable(&conn, "hono", Some("4.13.2"), NO_PROJECTS),
            "4.13.2 is past the 4.12.34 fix — the user already remediated this"
        );
    }

    #[test]
    fn an_install_inside_the_range_is_still_vulnerable() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);
        assert!(still_vulnerable(&conn, "hono", Some("4.12.0"), NO_PROJECTS));
    }

    #[test]
    fn safety_is_never_claimed_on_missing_information() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);

        // Unknown installed version.
        assert!(still_vulnerable(&conn, "hono", None, NO_PROJECTS));
        // Package OSV knows nothing about.
        assert!(still_vulnerable(
            &conn,
            "some-unscanned-pkg",
            Some("1.0.0"),
            NO_PROJECTS
        ));
        // Unreadable range.
        let conn2 = osv_conn();
        add_advisory(&conn2, "hono", "not json");
        assert!(still_vulnerable(
            &conn2,
            "hono",
            Some("4.13.2"),
            NO_PROJECTS
        ));
        // No osv_advisories table at all (older database).
        let bare = rusqlite::Connection::open_in_memory().unwrap();
        assert!(still_vulnerable(&bare, "hono", Some("4.13.2"), NO_PROJECTS));
    }

    // -----------------------------------------------------------------------
    // Version resolution (2026-08-26, found by running the fix live)
    //
    // The gap scan walks `project_dependencies`, whose `version` column is NULL
    // for every row in practice (245 of 245 live) — versions are resolved from
    // lockfiles into `user_dependencies`. So `still_vulnerable` received `None`
    // every time, took the conservative branch, and the whole version check was
    // inert: `hono` shipped CRITICAL on 4.13.2 against advisories fixed in
    // 4.12.34. Unit tests passed throughout because they supplied a version.
    // -----------------------------------------------------------------------

    #[test]
    fn a_null_manifest_version_resolves_from_the_installed_set() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);
        // Exactly the live row: manifest version NULL, lockfile says 4.13.2.
        add_installed(&conn, "d:/4da/mcp-4da-server", "hono", "4.13.2");

        let paths = vec!["d:/4da/mcp-4da-server".to_string()];
        assert!(
            !still_vulnerable(&conn, "hono", None, &paths),
            "a NULL manifest version must resolve from user_dependencies, not \
             fall through to the conservative branch"
        );
    }

    #[test]
    fn resolution_is_scoped_to_the_projects_the_gap_names() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);
        // The live spread: patched here, behind in two unrelated checkouts.
        add_installed(&conn, "d:/4da/mcp-4da-server", "hono", "4.13.2");
        add_installed(&conn, "c:/users/admin/documents/4da-repo", "hono", "4.11.1");
        add_installed(&conn, "c:/users/admin/documents/navcal", "hono", "4.9.10");

        assert!(
            !still_vulnerable(&conn, "hono", None, &["d:/4da/mcp-4da-server".to_string()]),
            "a gap naming the patched project must not be judged on a sibling repo's copy"
        );
        assert!(
            still_vulnerable(
                &conn,
                "hono",
                None,
                &["c:/users/admin/documents/navcal".to_string()]
            ),
            "a gap naming the lagging project stays live"
        );
    }

    #[test]
    fn any_named_project_still_in_range_keeps_the_gap_live() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);
        add_installed(&conn, "d:/4da/mcp-4da-server", "hono", "4.13.2");
        add_installed(&conn, "d:/4da/other", "hono", "4.9.10");

        assert!(
            still_vulnerable(
                &conn,
                "hono",
                None,
                &[
                    "d:/4da/mcp-4da-server".to_string(),
                    "d:/4da/other".to_string()
                ]
            ),
            "one exposed copy among the named projects is enough"
        );
    }

    #[test]
    fn resolution_normalizes_path_separators_and_case() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);
        add_installed(&conn, "d:/4da/mcp-4da-server", "hono", "4.13.2");
        // git_signals-style native path against the stored forward-slash form.
        let paths = vec![r"D:\4DA\mcp-4da-server".to_string()];
        assert!(
            !still_vulnerable(&conn, "hono", None, &paths),
            "path form must not decide whether a version is found"
        );
    }

    #[test]
    fn resolution_never_claims_safety_without_a_version() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);
        // Package present in the advisory set but installed nowhere we can see.
        assert!(still_vulnerable(
            &conn,
            "hono",
            None,
            &["d:/4da/unscanned".to_string()]
        ));
        // No projects named at all.
        assert!(still_vulnerable(&conn, "hono", None, NO_PROJECTS));
        // Empty-string version is not a version.
        add_installed(&conn, "d:/4da/blank", "hono", "");
        assert!(still_vulnerable(
            &conn,
            "hono",
            None,
            &["d:/4da/blank".to_string()]
        ));
    }

    #[test]
    fn an_explicit_version_still_wins_over_resolution() {
        let conn = osv_conn();
        add_advisory(&conn, "hono", HONO_RANGES);
        // A lagging install exists, but the caller already knows the version.
        add_installed(&conn, "d:/4da/mcp-4da-server", "hono", "4.9.10");
        assert!(
            !still_vulnerable(
                &conn,
                "hono",
                Some("4.13.2"),
                &["d:/4da/mcp-4da-server".to_string()]
            ),
            "an explicit version is authoritative and skips resolution"
        );
    }

    /// AD-045: `jsonwebtoken` the crate and `jsonwebtoken` the npm package share
    /// a name and nothing else. A gap that merges projects of both judges each
    /// install only against its OWN ecosystem's advisories.
    #[test]
    fn an_install_is_judged_only_by_its_own_ecosystems_advisories() {
        let conn = osv_conn();
        let range = |fixed: &str| {
            format!(
                r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#
            )
        };
        conn.execute(
            "INSERT INTO osv_advisories (advisory_id, package_name, ecosystem, affected_ranges)
             VALUES ('GHSA-h395-gr6q-cpjc', 'jsonwebtoken', 'crates.io', ?1),
                    ('GHSA-27h2-hvpr-p74q', 'jsonwebtoken', 'npm', ?2)",
            params![range("10.3.0"), range("9.0.0")],
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem) VALUES
                 ('d:/4da/relay', 'jsonwebtoken', '9.3.1', 'rust'),
                 ('d:/4da/src-tauri', 'jsonwebtoken', '10.4.0', 'rust'),
                 ('d:/kairos/backend', 'jsonwebtoken', '9.0.2', 'javascript');",
        )
        .unwrap();

        assert!(
            !still_vulnerable(
                &conn,
                "jsonwebtoken",
                None,
                &["d:/kairos/backend".to_string()]
            ),
            "npm 9.0.2 is past every npm fix; the crates.io range must not reach it"
        );
        assert!(still_vulnerable(
            &conn,
            "jsonwebtoken",
            None,
            &["d:/4da/relay".to_string()]
        ));

        let paths = vec![
            "d:/4da/relay".to_string(),
            "d:/4da/src-tauri".to_string(),
            "d:/kairos/backend".to_string(),
        ];
        let exposed: Vec<String> = affected_project_paths(&conn, "jsonwebtoken", &paths)
            .into_iter()
            .map(|(project, _)| project)
            .collect();
        assert_eq!(
            exposed,
            vec!["d:/4da/relay".to_string()],
            "only relay's copy is exposed"
        );
    }

    /// The live defect (2026-09-10): `hono` 4.13.5 carried "3 unread security
    /// advisories" for three advisories fixed IN 4.13.5, because the only
    /// version verdict consulted was the one stored when the rows were scored
    /// against 4.13.3. The live verdict drops them; nothing actionable is left.
    #[test]
    fn advisories_fixed_in_the_installed_version_are_not_unread_advisories() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE osv_advisories (
                 advisory_id TEXT, package_name TEXT, ecosystem TEXT,
                 affected_ranges TEXT, aliases TEXT, withdrawn_at TEXT
             );",
        )
        .unwrap();
        let fixed = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.13.5"}]}]"#;
        let cves = ["CVE-2026-84363", "CVE-2026-84364", "CVE-2026-84365"];
        for (id, cve) in [
            "GHSA-crvj-82cr-hjcx",
            "GHSA-g6gw-c38x-mqfc",
            "GHSA-gqvv-2mrq-wpjv",
        ]
        .iter()
        .zip(cves)
        {
            conn.execute(
                "INSERT INTO osv_advisories VALUES (?1, 'hono', 'npm', ?2, ?3, NULL)",
                params![id, fixed, format!("[\"{cve}\"]")],
            )
            .unwrap();
        }
        let mut candidates: Vec<GapCandidate> = cves
            .iter()
            .enumerate()
            .map(|(i, cve)| {
                // The stale scoring-time verdict says "affected" (scored against 4.13.3).
                let mut c = cand_from(
                    i as i64,
                    &format!("[{cve}] Hono: advisory {i}"),
                    "cve",
                    &["hono"],
                    Some(true),
                );
                c.source_id = (*cve).to_string();
                c.content_type = Some("security_advisory".to_string());
                c
            })
            .collect();
        let mut discussion = cand_from(
            9,
            "Onefold + Hono + Cloudflare edge rendering",
            "devto",
            &[],
            None,
        );
        discussion.content_type = Some("discussion".to_string());
        candidates.push(discussion);

        let verdict_for = |installs: Vec<crate::osv::exposure::Install>| {
            let conn = &conn;
            move |c: &GapCandidate| -> Option<bool> {
                if is_advisory_row(c) && linked_to(c, "hono") {
                    crate::osv::exposure::advisory_row_reaches(
                        conn,
                        &c.source_id,
                        "hono",
                        &installs,
                    )
                } else {
                    None
                }
            }
        };
        let patched = verdict_for(vec![crate::osv::exposure::Install::new(
            Some("javascript"),
            "4.13.5",
        )]);
        let ids: Vec<i64> = keyword_misses_from(&candidates, "hono", &patched)
            .iter()
            .map(|m| m.item_id)
            .collect();
        assert_eq!(ids, vec![9], "only the discussion remains");
        assert!(
            !grounded_security_advisory(&candidates, "hono", &patched),
            "a fixed advisory never escalates"
        );

        let behind = verdict_for(vec![crate::osv::exposure::Install::new(
            Some("javascript"),
            "4.13.3",
        )]);
        assert_eq!(
            keyword_misses_from(&candidates, "hono", &behind).len(),
            4,
            "an install behind the fix keeps all three"
        );
    }

    /// Live check against the real database, opt-in and READ-ONLY.
    ///
    /// The previous version of this fix passed every unit test and was still
    /// inert in production, because the table it read carried no versions. A
    /// synthetic fixture cannot catch that class — only the real schema can.
    ///
    ///   FOURDA_VERIFY_DB=D:/4DA/data/4da.db cargo test --lib \
    ///       live_hono_is_recognised_as_patched -- --ignored --nocapture
    #[test]
    #[ignore = "requires FOURDA_VERIFY_DB pointing at a real database"]
    fn live_hono_is_recognised_as_patched() {
        let Ok(path) = std::env::var("FOURDA_VERIFY_DB") else {
            eprintln!("FOURDA_VERIFY_DB not set — nothing to verify");
            return;
        };
        let conn = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open live DB read-only");

        // The manifest row really does carry a NULL version — that is the
        // premise this fix exists to handle. If it ever stops being true the
        // fix is still correct, but this assertion documents what was measured.
        let manifest_version: Option<String> = conn
            .query_row(
                "SELECT version FROM project_dependencies WHERE package_name = 'hono' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(None);
        println!("project_dependencies.hono.version = {manifest_version:?}");

        let installed: Vec<String> =
            installs_for(&conn, "hono", &["d:/4da/mcp-4da-server".to_string()])
                .into_iter()
                .map(|install| install.version)
                .collect();
        println!("resolved installed versions       = {installed:?}");
        assert!(
            !installed.is_empty(),
            "resolution must find the lockfile version for the named project"
        );

        let vulnerable = still_vulnerable(
            &conn,
            "hono",
            manifest_version.as_deref(),
            &["d:/4da/mcp-4da-server".to_string()],
        );
        println!("still_vulnerable(hono @ mcp-4da-server) = {vulnerable}");
        assert!(
            !vulnerable,
            "hono is 4.13.2 in mcp-4da-server and every advisory is fixed in \
             4.12.34 — it must not be reported as still vulnerable"
        );
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
            classify_severity(&missed, 13, "hono", true, Some("high")),
            GapSeverity::Critical,
            "a genuinely exposed install still escalates"
        );
        assert_ne!(
            classify_severity(&missed, 13, "hono", false, Some("high")),
            GapSeverity::Critical,
            "an already-patched install must not be reported as critical"
        );
    }
}
