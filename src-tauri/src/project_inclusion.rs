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
//!    in strict-manifest mode (`FOURDA_STRICT_MANIFEST=1`), where the receipts
//!    ledger deliberately points `context_dirs` at fixture stacks
//!    (`4da-ledger/fixtures/<stack>`) and those ARE the intended context.
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

/// Tier 1: agent-infrastructure / ephemeral paths. The ENTIRE `.claude/` and
/// `.codex/` trees (worktrees, plans, scratch fixtures like
/// `.claude/plans/ledger-fixtures/*`) plus temp directories. Case-insensitive,
/// both slash styles (paths reach here in raw `D:\...` and canonicalized
/// `d:/...` forms).
pub(crate) fn is_agent_infra_path(path: &str) -> bool {
    let p = comparison_form(path);
    p.contains("/.claude/")
        || p.ends_with("/.claude")
        || p.contains("/.codex/")
        || p.ends_with("/.codex")
        || p.contains("/tmp/")
        || (p.contains("appdata") && p.contains("local") && p.contains("temp"))
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

/// Tiers 1+2 combined — "may never be persisted as user context". Tier 2 is
/// waived in strict-manifest mode (the receipts ledger scans fixture stacks
/// on purpose; `FOURDA_STRICT_MANIFEST` is never set for desktop users).
pub(crate) fn is_hard_excluded(path: &str) -> bool {
    is_hard_excluded_with(path, crate::source_fetching::strict_manifest_mode())
}

/// Pure variant of [`is_hard_excluded`] for tests (the strict-mode flag is a
/// process-wide `OnceLock` over an env var, which tests cannot toggle).
pub(crate) fn is_hard_excluded_with(path: &str, strict_manifest_mode: bool) -> bool {
    is_agent_infra_path(path) || (!strict_manifest_mode && is_non_project_path(path))
}

/// Scan-time exclusion for filesystem WALKS (ACE scanner, lockfile walk,
/// README discovery). Deliberately narrower than [`is_hard_excluded`]: no
/// temp-dir patterns, because unit tests and legitimate user scans can be
/// rooted in a temp directory — ephemeral temp paths are still blocked at the
/// DB write guards before anything persists.
pub(crate) fn is_scan_excluded_dir(path: &str) -> bool {
    let p = comparison_form(path);
    let agent_infra = p.contains("/.claude/")
        || p.ends_with("/.claude")
        || p.contains("/.codex/")
        || p.ends_with("/.codex");
    agent_infra || (!crate::source_fetching::strict_manifest_mode() && is_non_project_path(path))
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
pub(crate) fn user_excluded_paths() -> Vec<String> {
    crate::state::try_get_settings_manager()
        .map(|m| m.lock().get_excluded_project_paths())
        .unwrap_or_default()
}

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
            "/home/u/tmpfiles/app", // "tmpfiles" is not the "/tmp/" segment
        ] {
            assert!(!is_agent_infra_path(p), "false positive: {p}");
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
    fn hard_exclusion_combines_tiers_and_strict_mode_waives_tier2_only() {
        // Normal (desktop) mode: both tiers hard-exclude.
        assert!(is_hard_excluded_with("/repo/fixtures/app", false));
        assert!(is_hard_excluded_with("/repo/.claude/plans/x", false));
        // Strict-manifest (ledger) mode: tier 2 is waived — the ledger scans
        // 4da-ledger/fixtures/<stack> deliberately — but tier 1 still holds.
        assert!(!is_hard_excluded_with(
            "d:/runyourempire/4da-ledger/fixtures/csharp-service",
            true
        ));
        assert!(is_hard_excluded_with("/repo/.claude/plans/x", true));
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
