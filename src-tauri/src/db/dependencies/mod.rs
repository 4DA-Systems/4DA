// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency Intelligence — CRUD operations for user_dependencies and dependency_alerts.
//!
//! Stores dependencies discovered by ACE scanner and alerts detected from content analysis.

mod alerts;
pub(crate) mod mappers;
mod queries;
#[cfg(test)]
mod tests;
pub mod types;

pub(crate) use queries::is_excluded_project_path;
pub use queries::{purge_agent_infra_dependencies, AgentInfraPurge};
pub use types::{CrossProjectPackage, DependencyAlert, DependencyEdgeRow, StoredDependency};
