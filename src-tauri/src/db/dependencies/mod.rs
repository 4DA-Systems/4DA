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

pub use types::{CrossProjectPackage, DependencyAlert, DependencyEdgeRow, StoredDependency};
