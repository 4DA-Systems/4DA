// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Canonical project-inclusion policy — the single answer to "may this path
//! feed the intelligence pipeline?".
//!
//! Every context-production point (ACE scanner, `detected_projects` /
//! `project_dependencies` / `user_dependencies` / `dependency_snapshots`
//! writers, interest synthesis, blind-spots universe, cross-project views)
//! consults this module instead of growing its own ad-hoc path checks. Three
//! tiers:
//!
//! 1. **Agent infrastructure / ephemeral** ([`is_agent_infra_path`]) — the
//!    `.claude/` and `.codex/` trees (agent worktrees, plans, scratch
//!    fixtures) and temp directories. Never a user project; hard-excluded
//!    everywhere, unconditionally.
//! 2. **Non-project scaffolding** ([`is_non_project_path`]) — generic,
//!    product-safe patterns for directories that are never a user's real
//!    project: fixture trees (`fixtures`, `test-fixtures`, `ledger-fixtures`,
//!    `__fixtures__`, `testdata` as exact path segments) and registry-squat
//!    placeholders (a path segment ending in `-placeholder`). Segment-based
//!    matching ONLY — `myfixtures-app` does not match. Hard-excluded, EXCEPT
//!    when BOTH `FOURDA_STRICT_MANIFEST=1` AND `FOURDA_DATA_DIR` are set (the
//!    receipts ledger sets both per fixture and deliberately points
//!    `context_dirs` at fixture stacks — `4da-ledger/fixtures/<stack>`; a
//!    strict flag leaked into a desktop shell can never waive tier 2 alone).
//! 3. **User-excluded** ([`is_user_excluded`]) — the Settings → Intelligence
//!    "Your Stack" toggle (`excluded_project_paths`). Tier-3 projects remain
//!    DETECTED and listed in the UI (so the user can toggle them back), but
//!    contribute nothing downstream: no interest synthesis, no grounding, no
//!    blind-spot universe, no monitored-package sets.
//!
//! Tier 1+2 are enforced at write time (nothing is persisted) plus a startup
//! self-heal purge ([`crate::db::purge_non_project_intelligence`]); tier 3 is
//! enforced at every read that feeds intelligence.

/// Canonical COMPARISON form: forward slashes, lowercased on every platform.
/// Used only for matching, never for storage (storage canonicalization is
/// [`canonical_storage_path`], which lowercases on Windows only).
pub(crate) fn comparison_form(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// Canonical STORAGE form — MUST match `temporal::canonicalize_project_path`
/// and `db::dependencies::queries::canonicalize_project_path` so every table
/// keys rows identically: forward slashes always, lowercased on Windows
/// (case-insensitive filesystem) but case-preserving elsewhere.
pub(crate) fn canonical_storage_path(path: &str) -> String {
    if cfg!(windows) {
        path.replace('\\', "/").to_lowercase()
    } else {
        path.replace('\\', "/")
    }
}

/// True when any path segment is exactly `.claude` or `.codex` — the agent
/// infrastructure trees. Exact-segment matching (like tier 2), so a RELATIVE
/// `.claude/plans/x` matches too (the old `contains("/.claude/")` form let
/// leading-segment relative paths escape), while name lookalikes
/// (`my.claude-app`, `claude-client`) never do.
fn has_agent_infra_segment(path: &str) -> bool {
    comparison_form(path)
        .split('/')
        .filter(|s| !s.is_empty())
        .any(|seg| seg == ".claude" || seg == ".codex")
}

/// Tier 1: agent-infrastructure / ephemeral paths. The ENTIRE `.claude/` and
/// `.codex/` trees (worktrees, plans, scratch fixtures like
/// `.claude/plans/ledger-fixtures/*`) plus temp directories. Case-insensitive,
/// both slash styles (paths reach here in raw `D:\...` and canonicalized
/// `d:/...` forms), absolute or relative (exact-segment matching).
pub(crate) fn is_agent_infra_path(path: &str) -> bool {
    has_agent_infra_segment(path) || is_temp_path(path)
}

/// Temp-directory patterns (`/tmp/`, `AppData\Local\Temp`). Judged on
/// whatever the caller passes — callers that know a project root pass only
/// the part BELOW it (see [`relative_to_root`]).
fn is_temp_path(path: &str) -> bool {
    let p = comparison_form(path);
    p.contains("/tmp/") || (p.contains("appdata") && p.contains("local") && p.contains("temp"))
}

/// The part of `path` below `root`, in comparison form with a leading `/`
/// (`""` for the root itself), or `None` when `path` is not inside `root`.
///
/// Exclusion patterns describe what a directory IS inside a project
/// (`fixtures/`, `testdata/`, a temp clone), never where the user chose to
/// keep their projects. Judged on the absolute path they excluded a real
/// project root wholesale: `C:\Users\me\AppData\Local\Temp\checkout\app`,
/// `D:\fixtures\my-service` and `~/work/testdata/api` all vanished, and a
/// 19-repository conformance run cloned under the session's temp directory
/// had its whole gated walk return zero directories (2026-10-10).
pub(crate) fn relative_to_root(path: &str, root: &str) -> Option<String> {
    let p = comparison_form(path);
    let r = comparison_form(root);
    let r = r.trim_end_matches('/');
    if r.is_empty() {
        return None;
    }
    if p.trim_end_matches('/') == r {
        return Some(String::new());
    }
    p.strip_prefix(r)
        .filter(|rest| rest.starts_with('/'))
        .map(str::to_string)
}

/// The deepest configured root containing `path` decides what part of it is
/// judged; a path under no root is judged whole (the conservative default).
fn judged_part(path: &str, roots: &[String]) -> String {
    roots
        .iter()
        .filter_map(|root| relative_to_root(path, root))
        .min_by_key(String::len)
        .unwrap_or_else(|| comparison_form(path))
}

/// `path` in comparison form, cut to the part below the deepest configured
/// context directory containing it (whole when none does). For pattern
/// checks that describe what a directory IS inside a project.
pub(crate) fn judged_below_configured_roots(path: &str) -> String {
    judged_part(path, &configured_project_roots())
}

/// The user's configured context directories, or `[]` when settings are not
/// initialized or are busy. Non-blocking (`try_lock`): this runs inside DB
/// write guards that may be reached while a caller holds the settings lock,
/// and a busy lock falls back to judging the whole path — today's behavior.
fn configured_project_roots() -> Vec<String> {
    crate::state::try_get_settings_manager()
        .and_then(|m| m.try_lock().map(|s| s.get().context_dirs.clone()))
        .unwrap_or_default()
}

/// Path segments that mark a directory tree as test-fixture scaffolding.
/// Exact-segment matches only (see [`is_non_project_path`]).
const NON_PROJECT_SEGMENTS: &[&str] = &[
    "fixtures",
    "test-fixtures",
    "ledger-fixtures",
    "__fixtures__",
    "testdata",
];

/// Leaf/segment suffix that marks a registry-squat placeholder directory
/// (e.g. `crates-placeholder`, `npm-placeholder`).
const PLACEHOLDER_SUFFIX: &str = "-placeholder";

/// Tier 2: generic non-project scaffolding. TRUE when any path segment exactly
/// matches a known fixture-tree name, or any segment ends in `-placeholder`.
///
/// Segment-based matching ONLY (mirrors the proven `.claude` segment doctrine):
/// `myfixtures-app` and `testdata-loader` do NOT match; `a/fixtures/b` and
/// `d:\4da\crates-placeholder` do. This predicate is PURE — strict-manifest
/// exemption is applied by [`is_hard_excluded`], not here.
pub(crate) fn is_non_project_path(path: &str) -> bool {
    let p = comparison_form(path);
    p.split('/').filter(|s| !s.is_empty()).any(|seg| {
        NON_PROJECT_SEGMENTS.contains(&seg)
            || (seg.len() > PLACEHOLDER_SUFFIX.len() && seg.ends_with(PLACEHOLDER_SUFFIX))
    })
}

/// Is the tier-2 waiver active in THIS process? Requires BOTH strict-manifest
/// mode (`FOURDA_STRICT_MANIFEST=1`) AND an isolated data dir
/// (`FOURDA_DATA_DIR`) — the receipts ledger always sets both, per fixture. A
/// `FOURDA_STRICT_MANIFEST` leaked into a desktop dev shell (default data
/// dir) can therefore never waive tier-2 ingestion or disable the tier-2
/// purge against the real 4da.db. Cached once, like `strict_manifest_mode`.
pub(crate) fn tier2_waiver_active() -> bool {
    static WAIVER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *WAIVER.get_or_init(|| {
        tier2_waiver_from(
            crate::source_fetching::strict_manifest_mode(),
            std::env::var("FOURDA_DATA_DIR").is_ok_and(|v| !v.trim().is_empty()),
        )
    })
}

/// Pure combinator behind [`tier2_waiver_active`] (testable — the env-backed
/// flags are process-wide `OnceLock`s that tests cannot toggle).
pub(crate) fn tier2_waiver_from(strict_manifest_mode: bool, isolated_data_dir: bool) -> bool {
    strict_manifest_mode && isolated_data_dir
}

/// Tiers 1+2 combined — "may never be persisted as user context". Tier 2 is
/// waived only when [`tier2_waiver_active`] (strict-manifest mode AND an
/// isolated `FOURDA_DATA_DIR` — the receipts ledger scans fixture stacks on
/// purpose; neither is ever set for desktop users).
pub(crate) fn is_hard_excluded(path: &str) -> bool {
    is_hard_excluded_under(path, tier2_waiver_active(), &configured_project_roots())
}

/// Pure variant of [`is_hard_excluded`] for tests (the waiver flag is
/// env-backed and process-cached, which tests cannot toggle).
#[cfg(test)]
pub(crate) fn is_hard_excluded_with(path: &str, tier2_waived: bool) -> bool {
    is_hard_excluded_under(path, tier2_waived, &[])
}

/// [`is_hard_excluded`] with explicit roots: agent-infrastructure segments
/// anywhere in the path exclude it; temp and scaffolding patterns are judged
/// only on the part below the deepest root that contains it.
pub(crate) fn is_hard_excluded_under(path: &str, tier2_waived: bool, roots: &[String]) -> bool {
    if has_agent_infra_segment(path) {
        return true;
    }
    let judged = judged_part(path, roots);
    is_temp_path(&judged) || (!tier2_waived && is_non_project_path(&judged))
}

/// Scan-time exclusion for filesystem WALKS (ACE scanner, lockfile walk,
/// README discovery). Deliberately narrower than [`is_hard_excluded`]: no
/// temp-dir patterns, because unit tests and legitimate user scans can be
/// rooted in a temp directory — ephemeral temp paths are still blocked at the
/// DB write guards before anything persists.
#[cfg(test)]
pub(crate) fn is_scan_excluded_dir(path: &str) -> bool {
    is_scan_excluded_rel(path, &comparison_form(path))
}

/// [`is_scan_excluded_dir`] for a directory a walk reached from `root`:
/// scaffolding segments count only BELOW the root the user configured.
pub(crate) fn is_scan_excluded_below(root: &std::path::Path, dir: &std::path::Path) -> bool {
    let dir = dir.to_string_lossy();
    let judged =
        relative_to_root(&dir, &root.to_string_lossy()).unwrap_or_else(|| comparison_form(&dir));
    is_scan_excluded_rel(&dir, &judged)
}

fn is_scan_excluded_rel(path: &str, judged: &str) -> bool {
    if has_agent_infra_segment(path) {
        return true;
    }
    if !tier2_waiver_active() && is_non_project_path(judged) {
        log_tier2_exclusion(path, "scan");
        return true;
    }
    false
}

/// Tier-2 observability (accurate-first: a permanent silent exclusion is not
/// acceptable). Logs ONCE per canonical path per process, at scan/write time
/// only — tier-1 agent-infra rejections are expected machinery and are not
/// logged. A greyed-out "excluded scaffolding" entry in the Your Stack UI is
/// the planned follow-up surface.
pub(crate) fn log_tier2_exclusion(path: &str, site: &str) {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    // Self-classifying: only genuine tier-2 rejections log (callers may pass
    // any hard-excluded path; tier-1 agent infra stays silent by design).
    if is_agent_infra_path(path) || !is_non_project_path(path) {
        return;
    }
    static LOGGED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let canon = comparison_form(path);
    let mutex = LOGGED.get_or_init(|| Mutex::new(HashSet::new()));
    let newly = match mutex.lock() {
        Ok(mut set) => set.insert(canon),
        Err(_) => false, // poisoned — skip logging rather than panic
    };
    if newly {
        tracing::info!(
            target: "4da::project_inclusion",
            path = %path,
            site = %site,
            "excluded from context: non-project scaffolding (fixture-tree segment or -placeholder dir) — never treated as a user project"
        );
    }
}

/// Tier 3: the user's "Your Stack" exclusion list. Normalized comparison
/// (lowercase + forward slashes on BOTH sides) with a path-boundary prefix
/// match, so `D:\4DA\victauri-gauntlet` matches an exclusion stored as
/// `d:/4da/victauri-gauntlet` however either side was captured — and an
/// exclusion of `d:/4da/foo` does NOT match `d:/4da/foobar`.
pub(crate) fn is_user_excluded(path: &str, excluded: &[String]) -> bool {
    if excluded.is_empty() {
        return false;
    }
    let p = comparison_form(path);
    excluded.iter().any(|ex| {
        let ex = comparison_form(ex);
        let ex = ex.trim_end_matches('/');
        !ex.is_empty() && (p == ex || p.starts_with(&format!("{ex}/")))
    })
}

/// The user's Your-Stack exclusion list, or `[]` when the settings manager is
/// not yet initialized (unit tests, very early startup). Deliberately a
/// non-initializing read: initializing settings here would touch disk and the
/// platform keychain from library code paths that tests exercise.
///
/// Force-include entries (`!path`, see [`stack`]) share the stored setting and
/// are never returned here — a negation is not an exclusion.
pub(crate) fn user_excluded_paths() -> Vec<String> {
    stack::split_stack_setting(&stack::raw_stack_setting()).0
}

// "Does this project count toward the user's stack?" — exclusion tiers plus
// liveness (dormant / scratch), per project (audit 2026-10-07, AD-043 amended).
#[path = "project_inclusion_stack.rs"]
mod stack;
pub(crate) use stack::{
    apply_stack_choice, counts_toward_stack, retain_stack_projects, StackMembership,
};

/// The full policy: all three tiers. `user_excluded` is passed in (fetch once
/// via [`user_excluded_paths`]) so loops don't re-lock settings per item.
pub(crate) fn is_excluded_from_intelligence(path: &str, user_excluded: &[String]) -> bool {
    is_hard_excluded(path) || is_user_excluded(path, user_excluded)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tier 1 ──────────────────────────────────────────────────────────

    #[test]
    fn tier1_claude_and_codex_trees_both_slash_styles() {
        for p in [
            "/home/u/proj/.claude/worktrees/agent-abc",
            r"D:\4DA\.claude\plans\ledger-fixtures\csharp-service",
            "d:/4da/.claude",
            "/home/u/.codex/scratch",
            r"C:\repo\.codex",
        ] {
            assert!(is_agent_infra_path(p), "expected tier-1 match: {p}");
        }
    }

    #[test]
    fn tier1_temp_paths() {
        assert!(is_agent_infra_path("/tmp/clone/project"));
        assert!(is_agent_infra_path(r"C:\Users\x\AppData\Local\Temp\clone"));
    }

    #[test]
    fn tier1_no_substring_false_positives() {
        for p in [
            "/home/u/claude-client/src",
            r"D:\projects\my.codexplorer",
            "my.claude-tools/src", // relative lookalike — segment is "my.claude-tools"
            "/home/u/tmpfiles/app", // "tmpfiles" is not the "/tmp/" segment
        ] {
            assert!(!is_agent_infra_path(p), "false positive: {p}");
        }
    }

    #[test]
    fn tier1_relative_paths_match_by_segment() {
        // The old contains("/.claude/") form let a RELATIVE leading segment
        // escape; exact-segment matching closes it.
        for p in [
            ".claude/plans/ledger-fixtures/x",
            r".claude\worktrees\agent-abc",
            ".codex/scratch",
            ".claude",
        ] {
            assert!(is_agent_infra_path(p), "relative tier-1 must match: {p}");
        }
    }

    // ── Tier 2 ──────────────────────────────────────────────────────────

    #[test]
    fn tier2_fixture_segments_both_slash_styles_and_cases() {
        for p in [
            "/home/u/proj/fixtures/fake-app",
            r"D:\4DA\.claude\plans\ledger-fixtures\ruby-rails-app",
            "d:/ledger/Test-Fixtures/x",
            "/repo/__fixtures__/pkg",
            r"D:\go\src\thing\TESTDATA\mod",
        ] {
            assert!(is_non_project_path(p), "expected tier-2 match: {p}");
        }
    }

    #[test]
    fn tier2_placeholder_leaf_and_nested() {
        assert!(is_non_project_path(r"D:\4DA\crates-placeholder"));
        assert!(is_non_project_path("d:/4da/npm-placeholder"));
        // Nested below a placeholder root is still scaffolding.
        assert!(is_non_project_path(r"D:\4DA\crates-placeholder\sub"));
        // A directory literally named "-placeholder" alone does not match
        // (must have a name stem); neither does "placeholder" without hyphen.
        assert!(!is_non_project_path("/home/u/placeholder/app"));
    }

    #[test]
    fn tier2_segment_not_substring() {
        for p in [
            "/home/u/myfixtures-app/src",  // "myfixtures-app" != "fixtures"
            "/home/u/fixtures-loader/src", // prefix, not exact segment
            "/home/u/testdata-gen/src",    // prefix, not exact segment
            r"D:\work\placeholderify\x",   // no "-placeholder" suffix segment
            "/home/u/app-placeholders/x",  // plural — suffix is "-placeholders"
        ] {
            assert!(!is_non_project_path(p), "false positive: {p}");
        }
    }

    #[test]
    fn tier2_real_projects_unaffected() {
        for p in [
            "/home/user/dev/my-app",
            r"D:\4DA",
            r"D:\runyourempire\victauri",
        ] {
            assert!(!is_non_project_path(p), "false positive: {p}");
        }
    }

    // ── Hard exclusion (tiers 1+2 + strict-mode waiver) ─────────────────

    #[test]
    fn hard_exclusion_combines_tiers_and_waiver_lifts_tier2_only() {
        // Normal (desktop) mode: both tiers hard-exclude.
        assert!(is_hard_excluded_with("/repo/fixtures/app", false));
        assert!(is_hard_excluded_with("/repo/.claude/plans/x", false));
        // Waiver active (ledger): tier 2 is lifted — the ledger scans
        // 4da-ledger/fixtures/<stack> deliberately — but tier 1 still holds.
        assert!(!is_hard_excluded_with(
            "d:/runyourempire/4da-ledger/fixtures/csharp-service",
            true
        ));
        assert!(is_hard_excluded_with("/repo/.claude/plans/x", true));
    }

    #[test]
    fn tier2_waiver_requires_strict_mode_and_isolated_data_dir() {
        // The ledger always sets BOTH env vars. A FOURDA_STRICT_MANIFEST
        // leaked into a desktop dev shell (default data dir) must never waive
        // tier-2 ingestion or disable the tier-2 purge.
        assert!(tier2_waiver_from(true, true), "ledger: both set -> waived");
        assert!(
            !tier2_waiver_from(true, false),
            "leaked strict flag alone must NOT waive tier 2"
        );
        assert!(
            !tier2_waiver_from(false, true),
            "isolated data dir alone must NOT waive tier 2"
        );
        assert!(!tier2_waiver_from(false, false));
    }

    #[test]
    fn scan_exclusion_skips_agent_trees_but_not_temp_roots() {
        assert!(is_scan_excluded_dir("/home/u/p/.claude/plans"));
        assert!(is_scan_excluded_dir(r"D:\p\.codex\x"));
        // Walks rooted in a temp dir must still work (tests, legit scans).
        assert!(!is_scan_excluded_dir(
            r"C:\Users\x\AppData\Local\Temp\scan-root\proj"
        ));
        assert!(!is_scan_excluded_dir("/tmp/scan-root/proj"));
    }

    // ── Judged below the project root ───────────────────────────────────

    #[test]
    fn a_root_under_temp_or_fixtures_is_not_excluded_by_its_own_location() {
        let roots = vec![
            r"C:\Users\me\AppData\Local\Temp\checkout".to_string(),
            "/home/me/fixtures".to_string(),
        ];
        for p in [
            r"C:\Users\me\AppData\Local\Temp\checkout\app",
            "c:/users/me/appdata/local/temp/checkout",
            "/home/me/fixtures/my-service",
        ] {
            assert!(!is_hard_excluded_under(p, false, &roots), "{p}");
        }
        // Below the root the same patterns still exclude.
        assert!(is_hard_excluded_under(
            r"C:\Users\me\AppData\Local\Temp\checkout\app\testdata\mod",
            false,
            &roots
        ));
        assert!(is_hard_excluded_under(
            "/home/me/fixtures/svc/tmp/clone/x",
            false,
            &roots
        ));
        // Agent infrastructure is excluded wherever it sits.
        assert!(is_hard_excluded_under(
            r"C:\Users\me\AppData\Local\Temp\checkout\.claude\worktrees\a",
            false,
            &roots
        ));
        // Outside every root: judged whole, as before.
        assert!(is_hard_excluded_under(
            r"C:\Users\me\AppData\Local\Temp\other",
            false,
            &roots
        ));
    }

    #[test]
    fn relative_to_root_needs_a_path_boundary() {
        assert_eq!(
            relative_to_root("d:/a/b/c", "D:\\A\\B").as_deref(),
            Some("/c")
        );
        assert_eq!(relative_to_root("d:/a/b", "d:/a/b/").as_deref(), Some(""));
        assert_eq!(relative_to_root("d:/a/bc", "d:/a/b"), None);
        assert_eq!(relative_to_root("d:/a", ""), None);
    }

    #[test]
    fn a_walk_judges_scaffolding_below_its_root() {
        let root = std::path::Path::new("/home/me/fixtures/repos");
        assert!(!is_scan_excluded_below(
            root,
            std::path::Path::new("/home/me/fixtures/repos/app")
        ));
        assert!(is_scan_excluded_below(
            root,
            std::path::Path::new("/home/me/fixtures/repos/app/testdata/x")
        ));
        assert!(is_scan_excluded_below(
            root,
            std::path::Path::new("/home/me/fixtures/repos/app/.claude/plans")
        ));
    }

    // ── Tier 3 ──────────────────────────────────────────────────────────

    #[test]
    fn tier3_normalized_matching_across_slash_and_case() {
        let excluded = vec!["d:/4da/victauri-gauntlet".to_string()];
        assert!(is_user_excluded(r"D:\4DA\victauri-gauntlet", &excluded));
        assert!(is_user_excluded("d:/4da/victauri-gauntlet/sub", &excluded));
        // Reverse storage direction: exclusion captured raw, path canonical.
        let excluded_raw = vec![r"D:\4DA\Victauri-Gauntlet".to_string()];
        assert!(is_user_excluded("d:/4da/victauri-gauntlet", &excluded_raw));
    }

    #[test]
    fn tier3_prefix_requires_path_boundary() {
        let excluded = vec!["d:/4da/foo".to_string()];
        assert!(is_user_excluded("d:/4da/foo", &excluded));
        assert!(is_user_excluded("d:/4da/foo/bar", &excluded));
        assert!(!is_user_excluded("d:/4da/foobar", &excluded));
    }

    #[test]
    fn tier3_empty_and_trailing_slash_entries() {
        assert!(!is_user_excluded("d:/4da/x", &[]));
        // A bare "/" or empty exclusion entry must not exclude everything.
        let junk = vec![String::new(), "/".to_string()];
        assert!(!is_user_excluded("d:/4da/x", &junk));
        let trailing = vec!["d:/4da/x/".to_string()];
        assert!(is_user_excluded("d:/4da/x", &trailing));
        assert!(is_user_excluded(r"D:\4DA\x\y", &trailing));
    }

    // ── Storage canonical form ──────────────────────────────────────────

    #[test]
    fn storage_form_matches_temporal_convention() {
        let c = canonical_storage_path(r"D:\Proj\App");
        if cfg!(windows) {
            assert_eq!(c, "d:/proj/app");
        } else {
            assert_eq!(c, "D:/Proj/App");
        }
    }
}
