// SPDX-License-Identifier: FSL-1.1-Apache-2.0

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
