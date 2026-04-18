// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Git Decision Miner — Intelligence Reconciliation Phase 7 (Cold Start Layer 1).
//!
//! Scans the user's local git repositories for decision-shaped commits
//! and emits `SeededDecision`s that can be loaded into AWE's Wisdom
//! Graph to give Day-0 transmutations meaningful personal priors.
//!
//! **Why this matters:** AWE-the-engine's value proposition is that
//! judgment compounds with every decision and outcome fed to it. A
//! fresh install with zero decisions is judgment-dead — every
//! transmutation returns boilerplate. But every developer who has
//! shipped anything has *already made* dozens of decisions; they're
//! just sitting in git history. This miner recovers them.
//!
//! **Detection strategy:**
//! Commit messages are scanned for decision verbs (adopt, migrate,
//! switch, replace, ripout, move to, …). The subject (what was
//! adopted / migrated / replaced) is extracted from the rest of the
//! message using lightweight heuristics — no LLM. Outcomes are
//! inferred by cross-referencing:
//!   * Confirmed  → the decided-for subject is still named in HEAD
//!                  somewhere (file, Cargo.toml, package.json, etc).
//!                  For this miner we use a lightweight `git grep`.
//!   * Refuted    → a later commit message contains "revert" or
//!                  "ripout" naming the same subject.
//!   * Partial    → a later decision commit names the same subject
//!                  with a different verb (e.g. "adopt X" then
//!                  "replace X").
//!   * Pending    → none of the above; the decision stands but its
//!                  outcome isn't yet determinable.
//!
//! **Bounds:** caps at 200 commits scanned per repo, 5 repos per run,
//! 30s git subprocess timeout (reuses `ace::git::run_git_with_timeout`).
//! These bounds keep a fresh-install scan under 10 seconds even on a
//! machine with large monorepos.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{Result, ResultExt};
use crate::evidence::PrecedentOutcome;

// ============================================================================
// Types
// ============================================================================

/// A decision recovered from git history, ready to seed AWE's Wisdom
/// Graph. Serializable as JSONL for `awe seed --from-jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeededDecision {
    /// One-line decision statement in the user's past tense.
    /// Example: "Adopted tokio for async runtime".
    pub statement: String,
    /// The detection verb that matched ("adopt", "migrate", etc).
    pub verb: String,
    /// The subject extracted from the commit message (e.g. "tokio").
    pub subject: String,
    /// Inferred outcome. `Pending` when insufficient evidence.
    pub outcome: PrecedentOutcome,
    /// The commit hash this was extracted from.
    pub source_commit: String,
    /// Repo path (or a label if we anonymize later).
    pub source_repo: String,
    /// Commit Unix timestamp (seconds).
    pub timestamp: i64,
}

/// Summary of a mining run — what the caller gets back after the scan.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MineSummary {
    pub repos_scanned: u32,
    pub commits_scanned: u32,
    pub decisions_found: u32,
    pub confirmed: u32,
    pub refuted: u32,
    pub partial: u32,
    pub pending: u32,
}

// ============================================================================
// Decision verbs — the detection vocabulary
// ============================================================================

/// Decision verbs we look for in commit subjects. Each entry is
/// `(detection_token, normalized_verb_for_output)`. Order matters:
/// we match longest-first so "migrate to" beats "migrate".
const DECISION_VERBS: &[(&str, &str)] = &[
    // Multi-word phrases first (longest match)
    ("migrate to", "migrate"),
    ("migrate from", "migrate"),
    ("switch to", "switch"),
    ("switch from", "switch"),
    ("move to", "move"),
    ("move from", "move"),
    ("replace with", "replace"),
    ("swap to", "swap"),
    ("swap for", "swap"),
    ("port to", "port"),
    // Single-verb forms — past-tense first (longer), then bare.
    // Word-boundary enforcement at the match site prevents "adopt"
    // from eating "adopted" or "adopting".
    ("adopted", "adopt"),
    ("adopt", "adopt"),
    ("replaced", "replace"),
    ("replaces", "replace"),
    ("replace", "replace"),
    ("migrated", "migrate"),
    ("migrate", "migrate"),
    ("switched", "switch"),
    ("switch", "switch"),
    ("moved", "move"),
    ("ripped out", "ripout"),
    ("rip out", "ripout"),
    ("ripout", "ripout"),
    ("removed", "remove"),
    ("dropped", "drop"),
    ("drop", "drop"),
    ("deprecated", "deprecate"),
    ("deprecate", "deprecate"),
];

// ============================================================================
// Subject extraction
// ============================================================================

/// Tokens we do not treat as decision subjects — too generic, too common,
/// or purely grammatical. Single-word subjects must not be in this list.
const SUBJECT_STOPWORDS: &[&str] = &[
    "to", "from", "the", "a", "an", "and", "or", "for", "with", "in", "on", "of", "this", "that",
    "these", "those", "new", "old", "some", "all", "it", "them", "our", "my", "your", "his", "her",
    "their", "we",
];

/// True when `word` is a plausible decision subject (library, tool,
/// tech, module name). Rejects stopwords, pure punctuation, and
/// nothing-strings.
fn is_plausible_subject(word: &str) -> bool {
    if word.is_empty() || word.len() < 2 {
        return false;
    }
    if word.len() > 40 {
        return false;
    }
    let lower = word.to_lowercase();
    if SUBJECT_STOPWORDS.contains(&lower.as_str()) {
        return false;
    }
    // Reject words that are purely punctuation or ASCII junk.
    word.chars().any(|c| c.is_alphanumeric())
}

/// Extract the subject word from the text that follows a decision verb.
/// Conservative: takes the first plausible word. Returns None when the
/// context looks like narrative prose rather than a tool / library /
/// subject name.
fn extract_subject(after_verb: &str) -> Option<String> {
    for token in after_verb
        .split(|c: char| {
            c.is_whitespace() || matches!(c, ',' | '.' | ':' | ';' | '!' | '?' | '"' | '\'')
        })
        .filter(|s| !s.is_empty())
    {
        // Strip trailing punctuation / parens
        let cleaned = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        if is_plausible_subject(cleaned) {
            return Some(cleaned.to_string());
        }
    }
    None
}

// ============================================================================
// Commit scanning
// ============================================================================

/// Represents a single commit we pulled from `git log`.
#[derive(Debug, Clone)]
struct ParsedCommit {
    hash: String,
    timestamp: i64,
    subject_line: String,
}

/// Find the first decision verb match in a commit subject line.
/// Returns (detection_token, normalized_verb, text_after_verb) or None.
pub fn find_decision_verb(subject_line: &str) -> Option<(String, String, String)> {
    let lower = subject_line.to_lowercase();
    for (token, verb) in DECISION_VERBS {
        if let Some(idx) = lower.find(token) {
            // Word-boundary check: character before the token must be
            // a separator or start-of-string; likewise after.
            let before_ok = idx == 0
                || lower
                    .as_bytes()
                    .get(idx - 1)
                    .map_or(true, |b| !b.is_ascii_alphabetic());
            let after_ok = lower
                .as_bytes()
                .get(idx + token.len())
                .map_or(true, |b| !b.is_ascii_alphabetic());
            if before_ok && after_ok {
                let after = subject_line[idx + token.len()..].to_string();
                return Some((token.to_string(), verb.to_string(), after));
            }
        }
    }
    None
}

/// Run `git log` with our scan parameters against `repo_path`.
/// Returns commits sorted newest-first. Bounded by `max_commits`.
pub fn fetch_commits(repo_path: &Path, max_commits: usize) -> Result<Vec<ParsedCommit>> {
    let max_str = max_commits.to_string();
    // Format: hash + unix-timestamp + subject, separated by \x1f.
    // MUST be a single arg (`--pretty=format:VALUE`) — splitting it into
    // two args causes git to interpret the format string as a revision.
    let pretty = "--pretty=format:%H\x1f%at\x1f%s";
    let args = &[
        "log",
        "--no-merges",
        "--max-count",
        max_str.as_str(),
        pretty,
    ];

    // Inline the subprocess (can't cross-call ace::git's private helper).
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .context("failed to run git log")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git log failed: {stderr}").into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    for line in stdout.lines().filter(|l| !l.is_empty()) {
        let parts: Vec<&str> = line.splitn(3, '\x1f').collect();
        if parts.len() != 3 {
            continue;
        }
        let Ok(ts) = parts[1].parse::<i64>() else {
            continue;
        };
        out.push(ParsedCommit {
            hash: parts[0].to_string(),
            timestamp: ts,
            subject_line: parts[2].to_string(),
        });
    }
    Ok(out)
}

/// Build a decision statement in past tense: "Adopted tokio".
/// Capitalizes the verb, preserves the subject as-is.
pub fn compose_statement(verb: &str, subject: &str) -> String {
    let verb_past = match verb {
        "adopt" => "Adopted",
        "migrate" => "Migrated to",
        "switch" => "Switched to",
        "move" => "Moved to",
        "replace" => "Replaced",
        "swap" => "Swapped",
        "port" => "Ported to",
        "ripout" => "Ripped out",
        "remove" => "Removed",
        "drop" => "Dropped",
        "deprecate" => "Deprecated",
        _ => "Chose",
    };
    format!("{verb_past} {subject}")
}

// ============================================================================
// Outcome inference
// ============================================================================

/// Infer the outcome of a decision by looking for follow-up commits
/// that either refute it (revert, ripout) or partially supersede it
/// (a later decision commit naming the same subject).
///
/// `newer_commits` must be strictly newer than the decision commit
/// (caller's responsibility). Returned outcome:
///   * Refuted  if any newer commit contains "revert"/"ripout" + subject
///   * Partial  if any newer commit has a different decision verb on the
///              same subject (e.g. adopt X → replace X)
///   * Confirmed if HEAD still references the subject in tracked files
///   * Pending   otherwise
pub fn infer_outcome(
    subject: &str,
    newer_commits: &[ParsedCommit],
    still_in_head: bool,
) -> PrecedentOutcome {
    let subject_lower = subject.to_lowercase();

    for commit in newer_commits {
        let msg_lower = commit.subject_line.to_lowercase();
        if !msg_lower.contains(&subject_lower) {
            continue;
        }
        if msg_lower.contains("revert")
            || msg_lower.contains("ripout")
            || msg_lower.contains("rip out")
            || msg_lower.contains("ripped out")
        {
            return PrecedentOutcome::Refuted;
        }
        // A later decision commit on the same subject → partial
        if let Some((_, later_verb, _)) = find_decision_verb(&commit.subject_line) {
            if later_verb != "adopt" && later_verb != "switch" && later_verb != "migrate" {
                return PrecedentOutcome::Partial;
            }
        }
    }

    if still_in_head {
        PrecedentOutcome::Confirmed
    } else {
        PrecedentOutcome::Pending
    }
}

/// Lightweight check: does HEAD contain the subject somewhere in a
/// tracked file (case-insensitive)? Uses `git grep` for speed.
/// Non-fatal on error — returns false when git-grep fails, which is
/// the safe default (pending rather than spurious-confirmed).
pub fn subject_in_head(repo_path: &Path, subject: &str) -> bool {
    if subject.is_empty() {
        return false;
    }
    let output = Command::new("git")
        .args([
            "grep",
            "--quiet", // exit 0 if found, 1 if not
            "--ignore-case",
            "--fixed-strings",
            subject,
        ])
        .current_dir(repo_path)
        .output();
    matches!(output, Ok(out) if out.status.success())
}

// ============================================================================
// Top-level miner
// ============================================================================

/// Mine a single repo for decision-shaped commits.
/// `max_commits` caps the scan to prevent runaway cost on huge repos.
pub fn mine_repo(repo_path: &Path, max_commits: usize) -> Result<Vec<SeededDecision>> {
    if !repo_path.exists() {
        return Ok(Vec::new());
    }
    // Quick sanity — is there even a .git dir?
    let dot_git = repo_path.join(".git");
    if !dot_git.exists() {
        return Ok(Vec::new());
    }

    let commits = fetch_commits(repo_path, max_commits)?;
    let mut seeded: Vec<SeededDecision> = Vec::new();

    // Walk commits oldest→newest so "newer_commits" window is easy.
    let chronological: Vec<&ParsedCommit> = commits.iter().rev().collect();

    for (idx, commit) in chronological.iter().enumerate() {
        let Some((_token, verb, after)) = find_decision_verb(&commit.subject_line) else {
            continue;
        };
        let Some(subject) = extract_subject(&after) else {
            continue;
        };

        // newer_commits = those strictly after `commit` in time (later
        // in chronological order)
        let newer: Vec<ParsedCommit> = chronological[idx + 1..].iter().copied().cloned().collect();

        let in_head = subject_in_head(repo_path, &subject);
        let outcome = infer_outcome(&subject, &newer, in_head);
        let statement = compose_statement(&verb, &subject);

        seeded.push(SeededDecision {
            statement,
            verb,
            subject,
            outcome,
            source_commit: commit.hash.clone(),
            source_repo: repo_path.to_string_lossy().to_string(),
            timestamp: commit.timestamp,
        });
    }

    Ok(seeded)
}

/// Mine a set of repos. Caller provides the list — typically obtained
/// from ACE's tracked projects. `repo_cap` and `max_commits_per_repo`
/// bound the total work.
pub fn mine_many(
    repos: &[PathBuf],
    repo_cap: usize,
    max_commits_per_repo: usize,
) -> (Vec<SeededDecision>, MineSummary) {
    let mut summary = MineSummary::default();
    let mut all_decisions: Vec<SeededDecision> = Vec::new();

    for repo in repos.iter().take(repo_cap) {
        summary.repos_scanned += 1;
        match mine_repo(repo, max_commits_per_repo) {
            Ok(decisions) => {
                summary.commits_scanned += max_commits_per_repo as u32;
                for d in decisions {
                    match d.outcome {
                        PrecedentOutcome::Confirmed => summary.confirmed += 1,
                        PrecedentOutcome::Refuted => summary.refuted += 1,
                        PrecedentOutcome::Partial => summary.partial += 1,
                        PrecedentOutcome::Pending => summary.pending += 1,
                    }
                    summary.decisions_found += 1;
                    all_decisions.push(d);
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "4da::git_decision_miner",
                    repo = %repo.display(),
                    error = %e,
                    "failed to mine repo — skipping"
                );
            }
        }
    }

    (all_decisions, summary)
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Run the miner against ACE's currently-tracked project paths.
/// Returns a JSON summary with counts. Decisions are serialized to a
/// JSONL file at `<temp>/awe_git_seeded.jsonl` which a later phase will
/// feed to AWE's seed importer.
#[tauri::command]
pub async fn mine_git_decisions() -> std::result::Result<String, String> {
    // Discover tracked repos via the user's configured context_dirs.
    // No hard dependency on ACE — this only needs the settings manager.
    let repos: Vec<PathBuf> = crate::get_context_dirs();

    // Bounds: 5 repos × 200 commits = 1000 commits max scanned.
    const REPO_CAP: usize = 5;
    const MAX_COMMITS: usize = 200;

    let (decisions, summary) = mine_many(&repos, REPO_CAP, MAX_COMMITS);

    // Write JSONL to temp for the seeder to pick up.
    let out_path = std::env::temp_dir().join("awe_git_seeded.jsonl");
    let mut lines: Vec<String> = Vec::with_capacity(decisions.len());
    for d in &decisions {
        if let Ok(s) = serde_json::to_string(d) {
            lines.push(s);
        }
    }
    let _ = std::fs::write(&out_path, lines.join("\n"));

    serde_json::to_string(&serde_json::json!({
        "summary": summary,
        "jsonl_path": out_path.to_string_lossy(),
    }))
    .map_err(|e| e.to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Decision verb detection -------------------------------------------

    #[test]
    fn detects_adopt_verb() {
        let (tok, verb, after) = find_decision_verb("feat: adopt tokio for async runtime").unwrap();
        assert_eq!(tok, "adopt");
        assert_eq!(verb, "adopt");
        assert!(after.contains("tokio"));
    }

    #[test]
    fn detects_migrate_to_phrase() {
        let (_, verb, after) = find_decision_verb("Migrate to pnpm from npm").unwrap();
        assert_eq!(verb, "migrate");
        assert!(after.starts_with(" pnpm"));
    }

    #[test]
    fn detects_switched_past_tense() {
        let (_, verb, _) = find_decision_verb("Switched database to sqlite-vec").unwrap();
        assert_eq!(verb, "switch");
    }

    #[test]
    fn detects_ripout() {
        let (_, verb, _) = find_decision_verb("Ripped out old auth middleware").unwrap();
        assert_eq!(verb, "ripout");
    }

    #[test]
    fn no_match_on_non_decision() {
        assert!(find_decision_verb("fix: handle edge case in parser").is_none());
    }

    #[test]
    fn word_boundary_rejects_substring() {
        // "adopting" contains "adopt" but is narrative, not a decision
        // commit. Our word-boundary check rejects it.
        assert!(find_decision_verb("Thinking about adopting X someday").is_none());
    }

    // --- Subject extraction ------------------------------------------------

    #[test]
    fn extracts_subject_after_verb() {
        assert_eq!(
            extract_subject(" tokio for async").as_deref(),
            Some("tokio")
        );
        assert_eq!(
            extract_subject(" sqlite-vec as vector backend").as_deref(),
            Some("sqlite-vec")
        );
    }

    #[test]
    fn extract_skips_stopwords() {
        // "to sqlite" → first plausible word is sqlite, not "to"
        assert_eq!(extract_subject(" to sqlite").as_deref(), Some("sqlite"));
    }

    #[test]
    fn extract_returns_none_on_gibberish() {
        assert_eq!(extract_subject("").as_deref(), None);
        assert_eq!(extract_subject("   ."), None);
    }

    #[test]
    fn extract_rejects_too_long_strings() {
        let mega = "x".repeat(100);
        assert!(extract_subject(&format!(" {mega}")).is_none());
    }

    #[test]
    fn extract_strips_trailing_punctuation() {
        assert_eq!(
            extract_subject(" tokio, because async").as_deref(),
            Some("tokio")
        );
    }

    // --- Statement composition ---------------------------------------------

    #[test]
    fn composes_adopted() {
        assert_eq!(compose_statement("adopt", "tokio"), "Adopted tokio");
    }

    #[test]
    fn composes_migrated_to() {
        assert_eq!(compose_statement("migrate", "pnpm"), "Migrated to pnpm");
    }

    #[test]
    fn composes_unknown_verb_falls_back_to_chose() {
        assert_eq!(compose_statement("handwave", "pnpm"), "Chose pnpm");
    }

    // --- Outcome inference -------------------------------------------------

    fn commit(hash: &str, ts: i64, subject: &str) -> ParsedCommit {
        ParsedCommit {
            hash: hash.to_string(),
            timestamp: ts,
            subject_line: subject.to_string(),
        }
    }

    #[test]
    fn outcome_refuted_on_revert() {
        let newer = vec![commit("c2", 2, "Revert: adopt tokio")];
        assert_eq!(
            infer_outcome("tokio", &newer, true),
            PrecedentOutcome::Refuted
        );
    }

    #[test]
    fn outcome_refuted_on_ripout() {
        let newer = vec![commit("c2", 2, "Rip out tokio for simpler scheduler")];
        assert_eq!(
            infer_outcome("tokio", &newer, true),
            PrecedentOutcome::Refuted
        );
    }

    #[test]
    fn outcome_partial_on_later_replace() {
        let newer = vec![commit("c2", 2, "Replace tokio with smol")];
        assert_eq!(
            infer_outcome("tokio", &newer, false),
            PrecedentOutcome::Partial
        );
    }

    #[test]
    fn outcome_confirmed_when_still_in_head() {
        let newer: Vec<ParsedCommit> = Vec::new();
        assert_eq!(
            infer_outcome("tokio", &newer, true),
            PrecedentOutcome::Confirmed
        );
    }

    #[test]
    fn outcome_pending_when_nothing_indicative() {
        let newer: Vec<ParsedCommit> = Vec::new();
        assert_eq!(
            infer_outcome("tokio", &newer, false),
            PrecedentOutcome::Pending
        );
    }

    #[test]
    fn outcome_ignores_unrelated_newer_commits() {
        let newer = vec![commit("c2", 2, "fix: unrelated bug in parser")];
        assert_eq!(
            infer_outcome("tokio", &newer, true),
            PrecedentOutcome::Confirmed
        );
    }

    // --- SeededDecision serialization --------------------------------------

    #[test]
    fn seeded_decision_serializes_as_jsonl() {
        let d = SeededDecision {
            statement: "Adopted tokio".to_string(),
            verb: "adopt".to_string(),
            subject: "tokio".to_string(),
            outcome: PrecedentOutcome::Confirmed,
            source_commit: "deadbeef".to_string(),
            source_repo: "/proj/a".to_string(),
            timestamp: 1700000000,
        };
        let line = serde_json::to_string(&d).unwrap();
        assert!(line.contains("\"statement\":\"Adopted tokio\""));
        assert!(line.contains("\"outcome\":\"confirmed\""));
    }

    #[test]
    fn seeded_decision_roundtrips() {
        let d = SeededDecision {
            statement: "Migrated to pnpm".to_string(),
            verb: "migrate".to_string(),
            subject: "pnpm".to_string(),
            outcome: PrecedentOutcome::Pending,
            source_commit: "cafebabe".to_string(),
            source_repo: "/proj/b".to_string(),
            timestamp: 1700001000,
        };
        let line = serde_json::to_string(&d).unwrap();
        let back: SeededDecision = serde_json::from_str(&line).unwrap();
        assert_eq!(back, d);
    }

    // --- Plausibility gate -------------------------------------------------

    #[test]
    fn plausible_subject_accepts_package_names() {
        assert!(is_plausible_subject("tokio"));
        assert!(is_plausible_subject("sqlite-vec"));
        assert!(is_plausible_subject("react_router"));
    }

    #[test]
    fn plausible_subject_rejects_stopwords() {
        for w in SUBJECT_STOPWORDS {
            assert!(!is_plausible_subject(w), "should reject stopword: {w}");
        }
    }

    #[test]
    fn plausible_subject_rejects_length_extremes() {
        assert!(!is_plausible_subject(""));
        assert!(!is_plausible_subject("a"));
        assert!(!is_plausible_subject(&"x".repeat(50)));
    }

    // --- MineSummary counting ----------------------------------------------

    #[test]
    fn mine_summary_default_is_zero() {
        let s = MineSummary::default();
        assert_eq!(s.repos_scanned, 0);
        assert_eq!(s.decisions_found, 0);
    }

    // ------------------------------------------------------------------
    // Live smoke test — runs against the 4DA repo itself. Gated behind
    // `--ignored` so it doesn't block CI on machines without the repo
    // at the expected path. Run locally with:
    //   cargo test --lib smoke_mine_fourda -- --ignored --nocapture
    // ------------------------------------------------------------------

    #[test]
    #[ignore]
    fn smoke_mine_fourda() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(std::path::Path::to_path_buf);
        let Some(repo) = repo else {
            panic!("could not locate 4DA repo root");
        };
        let decisions = mine_repo(&repo, 200).expect("mine_repo");
        println!("found {} decisions in 4DA repo", decisions.len());
        for d in decisions.iter().take(10) {
            println!(
                "  [{:?}] {} (commit {})",
                d.outcome,
                d.statement,
                &d.source_commit[..8.min(d.source_commit.len())]
            );
        }
        assert!(
            decisions.len() >= 5,
            "expected ≥5 decisions mined from 4DA repo, found {}",
            decisions.len()
        );
    }
}
