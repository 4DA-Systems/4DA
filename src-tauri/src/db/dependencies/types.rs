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
