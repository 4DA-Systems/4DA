// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ACE (Autonomous Context Engine) Tauri commands and PASIFA helpers.
//!
//! Extracted from lib.rs. Contains all ACE phase commands (A through E),
//! autonomous discovery, watcher control, PASIFA README indexing,
//! and auto-seeding of interests from detected context.

mod accuracy;
mod anomalies;
mod context;
mod dependencies;
mod embeddings;
mod interactions;
mod scanning;
mod watcher;

pub use accuracy::*;
pub use anomalies::*;
pub use context::*;
pub use embeddings::*;
pub use interactions::*;
pub use scanning::*;
pub use watcher::*;

// Re-export README indexing for callers that use ace_commands::index_discovered_readmes
pub(crate) use crate::ace::readme_indexing::index_discovered_readmes;
