// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Provider-specific embedding functions (OpenAI, Ollama, fastembed) and retry logic.
//!
//! Split from a single file into submodules to keep each under the 700-line threshold.

pub mod fastembed;
mod ollama;
mod openai;
mod retry;

// Re-export parent items so submodules can reference them via `super::`
pub(super) use super::{truncate_and_normalize, EMBEDDING_CLIENT};

// Re-export public API — wildcard includes tauri::command macro-generated items
pub use fastembed::*;

#[cfg(feature = "fastembed-local")]
pub(super) use fastembed::embed_texts_fastembed_sync;

pub(super) use ollama::embed_texts_ollama;
pub(super) use openai::embed_texts_openai;
pub(super) use retry::retry_with_backoff;
