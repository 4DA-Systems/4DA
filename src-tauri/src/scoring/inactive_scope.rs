// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! An advisory that affects ONLY inactive projects is true, but it is not
//! "Affects You" (audit 2026-10-07; AD-043 amended 2026-10-08).
//!
//! Live 2026-10-07: `[RUSTSEC-2026-0190] anyhow` led Signal's "What you would
//! have missed" hero and Key Signals' "Affects You" as "Security issue affects
//! anyhow in 4da/victauri-gauntlet". The version-confirmed matcher is right to
//! look at every installed copy — it deliberately includes dormant projects so
//! Preemption can still name them (AD-043) — and the only exposed copy was in
//! a gitignored folder untouched for 161 days. Every live project already ran
//! the fix.
//!
//! The scorer now asks the one stack rule
//! ([`crate::project_inclusion::counts_toward_stack`]) of the projects an
//! affected verdict names. When NONE counts:
//! - `applicability` becomes [`AFFECTED_INACTIVE`] — still "affected", and
//!   still shown as such on the item, but the "Affects You" pool, the hero and
//!   the critical banner read only `affected` / `likely_affected`;
//! - `is_critical_alert`, `dependency_event` and `strongly_grounded` are false;
//! - the signal tier is capped at Advisory (a dormant project is named, never
//!   shouted — the `evidence::liveness` rule for the same projects).
//!
//! Nothing here changes the score or the necessity bucket: the finding keeps
//! its place in the feed; it just stops claiming to be about live code.

use super::dependencies::DepMatch;
use crate::db::Database;

/// The applicability label for an advisory whose every affected project is
/// dormant, scratch or otherwise outside the user's stack.
pub(crate) const AFFECTED_INACTIVE: &str = "affected_inactive";

/// The projects an affected verdict is about: the matcher's exposed copies
/// when it confirmed one, else the projects declaring the matched packages.
pub(crate) fn affected_projects(
    security_confirmed: bool,
    exposed_projects: &[String],
    matched_deps: &[DepMatch],
) -> Vec<String> {
    let mut paths: Vec<String> = if security_confirmed && !exposed_projects.is_empty() {
        exposed_projects.to_vec()
    } else {
        matched_deps
            .iter()
            .flat_map(|d| d.project_paths.iter().cloned())
            .collect()
    };
    paths.sort();
    paths.dedup();
    paths
}

/// Is this an affected verdict whose every project is inactive? Only
/// `affected` / `likely_affected` are asked — the other labels already keep an
/// item out of "Affects You". Loads the stack membership only when asked, so
/// the cost lands on the handful of affected advisories, not the corpus.
pub(crate) fn only_inactive_projects_affected(
    db: &Database,
    applicability: Option<&str>,
    projects: &[String],
) -> bool {
    if !matches!(applicability, Some("affected" | "likely_affected")) || projects.is_empty() {
        return false;
    }
    let conn = db.conn.lock();
    let membership = crate::project_inclusion::StackMembership::load(&conn);
    drop(conn);
    // EVERY project inactive: one live (or merely unknown) project keeps the
    // item grounded.
    projects
        .iter()
        .all(|p| !crate::project_inclusion::counts_toward_stack(p, &membership))
}

/// Cap a security signal about inactive projects at Advisory, and keep the
/// action line's "Critical:" prefix in step with the tier.
pub(crate) fn cap_inactive_priority(priority: &mut Option<String>, action: &mut Option<String>) {
    use crate::signals::SignalPriority;
    let cap = SignalPriority::Advisory;
    let current = priority
        .as_deref()
        .and_then(super::security_verdict::priority_from_label);
    if current.is_some_and(|p| p > cap) {
        *priority = Some(cap.label().to_string());
        if let Some(rest) = action.as_deref().and_then(|a| a.strip_prefix("Critical:")) {
            *action = Some(format!("Security:{rest}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::dependencies::VersionDelta;

    fn dep(name: &str, projects: &[&str]) -> DepMatch {
        DepMatch {
            package_name: name.to_string(),
            confidence: 0.9,
            version_delta: VersionDelta::Unknown,
            is_dev: false,
            is_direct: true,
            version: Some("1.0.0".to_string()),
            ecosystem: "rust".to_string(),
            corroborated: true,
            project_paths: projects.iter().map(ToString::to_string).collect(),
            raw_name: None,
        }
    }

    #[test]
    fn affected_projects_prefer_the_matchers_exposed_copies() {
        let deps = [dep("anyhow", &["d:/4da", "d:/4da/victauri-gauntlet"])];
        let exposed = vec!["d:/4da/victauri-gauntlet".to_string()];
        assert_eq!(affected_projects(true, &exposed, &deps), exposed);
        assert_eq!(
            affected_projects(false, &exposed, &deps),
            vec!["d:/4da".to_string(), "d:/4da/victauri-gauntlet".to_string()]
        );
    }

    #[test]
    fn only_affected_labels_are_asked() {
        let db = crate::test_utils::test_db();
        let p = vec!["d:/x".to_string()];
        assert!(!only_inactive_projects_affected(
            &db,
            Some("not_affected"),
            &p
        ));
        assert!(!only_inactive_projects_affected(&db, None, &p));
        assert!(!only_inactive_projects_affected(&db, Some("affected"), &[]));
    }

    #[test]
    fn a_scratch_only_advisory_is_affected_inactive_and_a_live_one_is_not() {
        let db = crate::test_utils::test_db();
        {
            let conn = db.conn.lock();
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS detected_projects (
                    path TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
                    last_activity TEXT, scratch INTEGER NOT NULL DEFAULT 0);
                 INSERT INTO detected_projects (path, name, last_activity, scratch) VALUES
                    ('D:\\4DA', '4da', strftime('%Y-%m-%dT%H:%M:%SZ','now'), 0),
                    ('D:\\4DA\\victauri-gauntlet', 'gauntlet', '2026-04-28T19:28:43Z', 1);",
            )
            .expect("seed detected_projects");
        }
        let scratch_only = vec!["d:/4da/victauri-gauntlet".to_string()];
        assert!(only_inactive_projects_affected(
            &db,
            Some("affected"),
            &scratch_only
        ));
        let with_live = vec!["d:/4da".to_string(), "d:/4da/victauri-gauntlet".to_string()];
        assert!(!only_inactive_projects_affected(
            &db,
            Some("affected"),
            &with_live
        ));
    }

    #[test]
    fn inactive_priority_caps_at_advisory_and_rewrites_the_prefix() {
        let mut priority = Some("critical".to_string());
        let mut action =
            Some("Critical: Security issue affects anyhow in 4da/gauntlet".to_string());
        cap_inactive_priority(&mut priority, &mut action);
        assert_eq!(priority.as_deref(), Some("advisory"));
        assert_eq!(
            action.as_deref(),
            Some("Security: Security issue affects anyhow in 4da/gauntlet")
        );
        let mut watch = Some("watch".to_string());
        let mut none = None;
        cap_inactive_priority(&mut watch, &mut none);
        assert_eq!(watch.as_deref(), Some("watch"));
    }
}
