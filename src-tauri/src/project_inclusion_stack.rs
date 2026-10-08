// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Does a project count toward the user's stack? One rule, read per PROJECT.
//!
//! Live 2026-10-07: Signal's "What you would have missed" hero and Key
//! Signals' "Affects You" both led with `[RUSTSEC-2026-0190] anyhow … affects
//! anyhow in 4da/victauri-gauntlet`. That folder is gitignored by the 4DA
//! repository, untouched for 161 days, and Preemption already listed it under
//! "Dormant projects" — while every live project ran the fixed anyhow. Three
//! readers answered "is this the user's stack?" three ways: grounding asked
//! whether the enclosing REPOSITORY had a commit in 60 days (a nested folder
//! inherits the root's heartbeat), Settings asked only about exclusions, and
//! the advisory matcher deliberately asked nothing (AD-043: a dormant
//! project's advisories are still true).
//!
//! The rule, in order:
//! 1. Hard-excluded (agent infra, fixture scaffolding) or user-excluded → no.
//! 2. Force-included by the user → yes, whatever the liveness says.
//! 3. Scratch — a tree its own repository gitignores (`ace::scratch`, the
//!    scan-time `git check-ignore` verdict on `detected_projects.scratch`) → no.
//! 4. Dormant — no activity for more than `ace::dormancy::DORMANT_AFTER_DAYS`,
//!    judged on the project's OWN evidence (a nested project with its own
//!    lockfile is not carried by its repository's commits) → no.
//! 5. Otherwise yes. Unknown liveness is never inactive.
//!
//! What it does NOT change (AD-043, amended 2026-10-08): an inactive project
//! stays detected, listed and labelled, and its advisories still reach
//! Preemption's "Dormant projects" footer. It only stops grounding Signal
//! relevance, "Affects You" and the hero.
//!
//! Force-include reuses the existing Your-Stack setting rather than adding
//! storage: `excluded_project_paths` holds gitignore-style negations, so
//! `!d:/4da/victauri-gauntlet` reads as "count this one even though it is
//! inactive". [`super::user_excluded_paths`] never returns a negation.

use rusqlite::Connection;

use crate::evidence::ProjectLiveness;

/// Prefix marking a force-include entry in `excluded_project_paths`.
pub(crate) const FORCE_INCLUDE_PREFIX: char = '!';

/// Why a project does or does not count — what Settings shows beside it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct ProjectStackStatus {
    /// The user's Your-Stack toggle is off for it (tier 3).
    pub excluded: bool,
    /// The user forced it in despite inactivity.
    pub forced: bool,
    /// Days since the project's last activity, when known.
    pub dormant_days: Option<i64>,
    /// Past the dormancy threshold.
    pub dormant: bool,
    /// Gitignored by its enclosing repository.
    pub scratch: bool,
    /// The verdict: does it ground relevance?
    pub counts: bool,
}

/// Everything [`counts_toward_stack`] reads, loaded once per pass.
#[derive(Debug, Default)]
pub(crate) struct StackMembership {
    user_excluded: Vec<String>,
    forced: Vec<String>,
    liveness: ProjectLiveness,
}

impl StackMembership {
    /// The user's Your-Stack setting plus `detected_projects` liveness.
    pub(crate) fn load(conn: &Connection) -> Self {
        let (user_excluded, forced) = split_stack_setting(&raw_stack_setting());
        Self {
            user_excluded,
            forced,
            liveness: ProjectLiveness::load(conn),
        }
    }

    /// Explicit parts (tests).
    #[cfg(test)]
    pub(crate) fn from_parts(
        user_excluded: Vec<String>,
        forced: Vec<String>,
        liveness: ProjectLiveness,
    ) -> Self {
        Self {
            user_excluded,
            forced,
            liveness,
        }
    }

    fn is_forced(&self, path: &str) -> bool {
        let key = key(path);
        self.forced.iter().any(|f| key_of_entry(f) == key)
    }

    /// The full status of one project.
    pub(crate) fn status(&self, path: &str) -> ProjectStackStatus {
        let excluded = super::is_excluded_from_intelligence(path, &self.user_excluded);
        let forced = !excluded && self.is_forced(path);
        let dormant_days = self.liveness.dormant_days(path);
        let dormant = dormant_days.is_some_and(crate::ace::dormancy::is_dormant_days);
        let scratch = self.liveness.is_scratch(path);
        let counts = !excluded && (forced || !(dormant || scratch));
        ProjectStackStatus {
            excluded,
            forced,
            dormant_days,
            dormant,
            scratch,
            counts,
        }
    }
}

/// THE rule: does this project ground the user's stack?
pub(crate) fn counts_toward_stack(path: &str, membership: &StackMembership) -> bool {
    membership.status(path).counts
}

/// Keep the rows whose PROJECT counts toward the stack, judged once per
/// distinct path. Returns `(rows, widened)`.
///
/// When the filter would leave nothing although rows exist — every project the
/// user has is inactive, e.g. back from a long break — the rows are returned
/// unfiltered and `widened` is true so the caller can say its scope degraded:
/// an empty set would read as "no stack at all", the worse lie.
pub(crate) fn retain_stack_projects<T>(
    rows: Vec<T>,
    path_of: impl Fn(&T) -> &str,
    membership: &StackMembership,
) -> (Vec<T>, bool) {
    let mut verdicts: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    let keep: Vec<bool> = rows
        .iter()
        .map(|row| {
            let path = path_of(row);
            *verdicts
                .entry(path.to_string())
                .or_insert_with(|| counts_toward_stack(path, membership))
        })
        .collect();
    let inactive = verdicts.values().filter(|counts| !**counts).count();
    if inactive == 0 {
        return (rows, false);
    }
    if !keep.iter().any(|k| *k) {
        tracing::warn!(
            target: "4da::stack",
            projects = inactive,
            "every dependency-bearing project is dormant or scratch — keeping all of them (scope degraded)"
        );
        return (rows, true);
    }
    tracing::debug!(
        target: "4da::stack",
        inactive,
        "inactive projects (dormant / scratch) left out of the stack"
    );
    let kept = rows
        .into_iter()
        .zip(keep)
        .filter_map(|(row, k)| k.then_some(row))
        .collect();
    (kept, false)
}

/// Split the stored setting into (exclusions, force-includes). Blank entries
/// and a bare `!` are dropped.
pub(crate) fn split_stack_setting(entries: &[String]) -> (Vec<String>, Vec<String>) {
    let mut excluded = Vec::new();
    let mut forced = Vec::new();
    for entry in entries {
        let trimmed = entry.trim();
        match trimmed.strip_prefix(FORCE_INCLUDE_PREFIX) {
            Some(rest) if !rest.trim().is_empty() => forced.push(rest.trim().to_string()),
            Some(_) => {}
            None if !trimmed.is_empty() => excluded.push(trimmed.to_string()),
            None => {}
        }
    }
    (excluded, forced)
}

/// The setting as stored, or `[]` before the settings manager exists.
pub(crate) fn raw_stack_setting() -> Vec<String> {
    crate::state::try_get_settings_manager()
        .map(|m| m.lock().get_excluded_project_paths())
        .unwrap_or_default()
}

/// The stored setting after the user sets `path` to `included` (`force` marks
/// an inactive project in, or takes a force back out). Pure.
///
/// - `included && force`  → drop any exclusion, add `!path`.
/// - `included`           → drop any exclusion (a force entry is kept).
/// - `!included && force` → drop the force entry only (undo "Force include").
/// - `!included`          → drop any force entry, add the exclusion.
pub(crate) fn apply_stack_choice(
    entries: Vec<String>,
    path: &str,
    included: bool,
    force: bool,
) -> Vec<String> {
    let target = key(path);
    let mut out: Vec<String> = entries
        .into_iter()
        .filter(|e| {
            let is_force = e.trim().starts_with(FORCE_INCLUDE_PREFIX);
            if key_of_entry(e) != target {
                return true;
            }
            // Same project: keep only what this choice leaves untouched.
            match (included, force) {
                (true, false) => is_force,
                (false, true) => !is_force,
                _ => false,
            }
        })
        .collect();
    match (included, force) {
        (true, true) => out.push(format!("{FORCE_INCLUDE_PREFIX}{path}")),
        (false, false) => out.push(path.to_string()),
        _ => {}
    }
    out
}

fn key(path: &str) -> String {
    super::comparison_form(path.trim())
        .trim_end_matches('/')
        .to_string()
}

fn key_of_entry(entry: &str) -> String {
    let trimmed = entry.trim();
    key(trimmed
        .strip_prefix(FORCE_INCLUDE_PREFIX)
        .unwrap_or(trimmed))
}

#[cfg(test)]
#[path = "project_inclusion_stack_tests.rs"]
mod tests;
