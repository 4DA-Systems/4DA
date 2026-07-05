// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency Intelligence — CRUD operations for user_dependencies and dependency_alerts.
//!
//! Stores dependencies discovered by ACE scanner and alerts detected from content analysis.

mod alerts;
#[cfg(test)]
mod builtin_purge_tests;
mod hygiene;
#[cfg(test)]
mod inclusion_tests;
pub(crate) mod mappers;
mod queries;
#[cfg(test)]
mod tests;
pub mod types;

pub use hygiene::{
    project_path_missing_on_disk, prune_orphaned_project_dependencies,
    purge_agent_infra_dependencies, purge_builtin_import_dependencies,
    purge_non_project_intelligence, AgentInfraPurge, BuiltinImportPurge, NonProjectPurge,
    OrphanedProjectPurge,
};
pub(crate) use queries::is_excluded_project_path;
pub use types::{CrossProjectPackage, DependencyAlert, DependencyEdgeRow, StoredDependency};
