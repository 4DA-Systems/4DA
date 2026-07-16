// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Exported structs for dependency intelligence.

use serde::{Deserialize, Serialize};

/// A dependency stored in user_dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDependency {
    pub id: i64,
    pub project_path: String,
    pub package_name: String,
    pub version: Option<String>,
    pub ecosystem: String,
    pub is_dev: bool,
    pub is_direct: bool,
    pub detected_at: String,
    pub last_seen_at: String,
    pub license: Option<String>,
}

/// A package used across multiple projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossProjectPackage {
    pub package_name: String,
    pub ecosystem: String,
    pub project_count: i64,
    pub projects: Vec<String>,
}

/// A stored parent->child dependency edge (Step 1: reachability foundation).
/// Captures the graph that the flatten parsers discard, so transitive-vuln
/// reachability can be computed. Internal computation only — never surfaced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdgeRow {
    pub id: i64,
    pub project_path: String,
    pub ecosystem: String,
    pub parent_package: String,
    pub parent_version: Option<String>,
    pub child_package: String,
    pub child_version: Option<String>,
    /// One of `runtime` | `dev` | `build` | `unknown`.
    pub scope: String,
    pub detected_at: String,
}

/// One installed dependency INSTANCE (a single resolved version) in the
/// multi-version inventory (`dependency_instances`, Phase 92). Unlike
/// [`StoredDependency`] — which collapses to one row per
/// `(project, package, ecosystem)` — the same package may appear multiple
/// times for one project at different versions. That is the entire point: a
/// negative verdict (`not_affected` / safe-to-close / quiet-week) is only
/// honest when proven against EVERY installed version, not the one row that
/// survived the collapsing upsert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInstanceRow {
    pub id: i64,
    pub project_path: String,
    pub ecosystem: String,
    pub package_name: String,
    pub version: String,
    pub is_direct: bool,
    pub is_dev: bool,
    /// One of `runtime` | `dev` | `build` | `unknown`. `unknown` today —
    /// lockfile processors do not yet resolve scope; refinement is future work.
    pub scope: String,
    pub detected_at: String,
}

/// Pre-persistence input for a bulk instance write (no id / timestamp yet).
#[derive(Debug, Clone)]
pub struct DependencyInstanceInput {
    pub package_name: String,
    pub version: String,
    pub is_direct: bool,
    pub is_dev: bool,
    /// `runtime` | `dev` | `build` | `unknown`.
    pub scope: String,
}

/// An alert associated with a dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAlert {
    pub id: i64,
    pub package_name: String,
    pub ecosystem: String,
    pub alert_type: String,
    pub severity: String,
    pub title: String,
    pub description: Option<String>,
    pub affected_versions: Option<String>,
    pub source_url: Option<String>,
    pub source_item_id: Option<i64>,
    pub detected_at: String,
    pub resolved_at: Option<String>,
}
