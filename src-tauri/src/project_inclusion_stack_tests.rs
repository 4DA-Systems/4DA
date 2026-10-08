// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;
use crate::evidence::ProjectLiveness;

fn membership(
    excluded: &[&str],
    forced: &[&str],
    days: &[(&str, i64)],
    scratch: &[&str],
) -> StackMembership {
    StackMembership::from_parts(
        excluded.iter().map(ToString::to_string).collect(),
        forced.iter().map(ToString::to_string).collect(),
        ProjectLiveness::from_entries(days).with_scratch(scratch),
    )
}

#[test]
fn a_live_project_counts() {
    let m = membership(&[], &[], &[("d:/4da/src-tauri", 0)], &[]);
    assert!(counts_toward_stack("D:\\4DA\\src-tauri", &m));
    let s = m.status("d:/4da/src-tauri");
    assert_eq!(s.dormant_days, Some(0));
    assert!(!s.dormant && !s.scratch && !s.excluded && s.counts);
}

#[test]
fn an_unknown_project_counts_unknown_is_never_inactive() {
    let m = membership(&[], &[], &[], &[]);
    assert!(counts_toward_stack("z:/nowhere/at-all", &m));
}

#[test]
fn a_user_excluded_project_does_not_count() {
    let m = membership(&["d:/4da/cli"], &[], &[("d:/4da/cli", 1)], &[]);
    let s = m.status("D:\\4DA\\cli");
    assert!(s.excluded && !s.counts);
}

#[test]
fn a_hard_excluded_project_does_not_count() {
    let m = membership(&[], &[], &[], &[]);
    assert!(!counts_toward_stack("d:/4da/.claude/worktrees/agent-x", &m));
}

#[test]
fn a_dormant_project_does_not_count() {
    let m = membership(&[], &[], &[("d:/4da/fourda-infer-proto", 119)], &[]);
    let s = m.status("d:/4da/fourda-infer-proto");
    assert!(s.dormant && !s.counts);
    assert_eq!(s.dormant_days, Some(119));
    // Exactly at the threshold is still live (dormant is strictly greater).
    let edge = membership(&[], &[], &[("d:/p", 90)], &[]);
    assert!(counts_toward_stack("d:/p", &edge));
}

#[test]
fn a_scratch_project_does_not_count_even_when_recently_touched() {
    let m = membership(&[], &[], &[("d:/4da/cli", 3)], &["d:/4da/cli"]);
    let s = m.status("d:/4da/cli");
    assert!(s.scratch && !s.dormant && !s.counts);
}

/// The 2026-10-07 case: a gitignored folder nested inside an ACTIVE repository
/// root. The root's liveness says nothing about it — the verdict is per
/// project, so the root counts and the nested scratch tree does not.
#[test]
fn a_nested_gitignored_project_under_an_active_root_does_not_count() {
    let m = membership(
        &[],
        &[],
        &[("d:/4da", 0), ("d:/4da/victauri-gauntlet", 161)],
        &["d:/4da/victauri-gauntlet"],
    );
    assert!(counts_toward_stack("d:/4da", &m));
    assert!(!counts_toward_stack("d:/4da/victauri-gauntlet", &m));
    assert!(!counts_toward_stack("D:\\4DA\\victauri-gauntlet\\", &m));
}

#[test]
fn force_include_overrides_inactivity_but_never_an_exclusion() {
    let m = membership(
        &[],
        &["D:\\4DA\\victauri-gauntlet"],
        &[("d:/4da/victauri-gauntlet", 161)],
        &["d:/4da/victauri-gauntlet"],
    );
    let s = m.status("d:/4da/victauri-gauntlet");
    assert!(s.forced && s.counts && s.dormant && s.scratch);

    let both = membership(&["d:/x"], &["d:/x"], &[("d:/x", 200)], &[]);
    assert!(!counts_toward_stack("d:/x", &both));
}

#[test]
fn force_include_is_exact_never_a_prefix() {
    let m = membership(&[], &["d:/4da"], &[("d:/4da/old", 300)], &[]);
    assert!(!counts_toward_stack("d:/4da/old", &m));
}

#[test]
fn retain_drops_inactive_projects_rows_and_keeps_live_ones() {
    let m = membership(&[], &[], &[("d:/live", 1), ("d:/dead", 200)], &[]);
    let rows = vec![
        ("d:/live", "tokio"),
        ("d:/dead", "anyhow"),
        ("d:/live", "serde"),
    ];
    let (kept, widened) = retain_stack_projects(rows, |r| r.0, &m);
    assert!(!widened);
    assert_eq!(kept, vec![("d:/live", "tokio"), ("d:/live", "serde")]);
}

#[test]
fn retain_widens_rather_than_return_an_empty_stack() {
    let m = membership(&[], &[], &[("d:/dead", 200)], &["d:/scratch"]);
    let rows = vec![("d:/dead", "anyhow"), ("d:/scratch", "openssl")];
    let (kept, widened) = retain_stack_projects(rows.clone(), |r| r.0, &m);
    assert!(
        widened,
        "an all-inactive inventory must say its scope degraded"
    );
    assert_eq!(kept, rows);
    let (none, widened) = retain_stack_projects(Vec::<(&str, &str)>::new(), |r| r.0, &m);
    assert!(none.is_empty() && !widened);
}

#[test]
fn the_setting_splits_negations_from_exclusions() {
    let (ex, forced) = split_stack_setting(&[
        "d:/a".to_string(),
        "!d:/b".to_string(),
        " ! ".to_string(),
        String::new(),
    ]);
    assert_eq!(ex, vec!["d:/a".to_string()]);
    assert_eq!(forced, vec!["d:/b".to_string()]);
}

#[test]
fn stack_choices_round_trip_through_the_one_setting() {
    let p = "D:\\4DA\\victauri-gauntlet";
    // Force include an inactive project.
    let s = apply_stack_choice(vec!["d:/other".to_string()], p, true, true);
    assert_eq!(s, vec!["d:/other".to_string(), format!("!{p}")]);
    // Re-force is idempotent (no duplicate entry).
    let s = apply_stack_choice(s, "d:/4da/victauri-gauntlet", true, true);
    assert_eq!(s.iter().filter(|e| e.starts_with('!')).count(), 1);
    // Undo the force: the entry goes, no exclusion is added.
    let s = apply_stack_choice(s, p, false, true);
    assert_eq!(s, vec!["d:/other".to_string()]);
    // Plain toggle off excludes; toggle on removes the exclusion.
    let s = apply_stack_choice(s, p, false, false);
    assert_eq!(s, vec!["d:/other".to_string(), p.to_string()]);
    let s = apply_stack_choice(s, p, true, false);
    assert_eq!(s, vec!["d:/other".to_string()]);
}

#[test]
fn excluding_a_forced_project_replaces_the_force() {
    let s = apply_stack_choice(vec!["!d:/x".to_string()], "d:/x", false, false);
    assert_eq!(s, vec!["d:/x".to_string()]);
}
