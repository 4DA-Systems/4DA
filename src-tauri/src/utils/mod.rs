// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Utility functions split into focused submodules.
//! All public items are re-exported here so `crate::utils::*` still works.

mod path;
mod scraping;
#[allow(unsafe_code)] // Intentional: FFI calls to OS memory-locking APIs (mlock / VirtualLock)
pub(crate) mod secure_mem;
mod text;
mod topics;
mod url;
mod vector;
mod word_boundary;

// Re-export everything so existing `use crate::utils::X` imports continue to work
pub(crate) use path::sanitize_path;
pub(crate) use scraping::scrape_article_content;
pub(crate) use text::is_boilerplate_chunk;
#[allow(unused_imports)] // Used by scraping submodule via super::text::MAX_CONTENT_LENGTH
pub(crate) use text::MAX_CONTENT_LENGTH;
pub(crate) use text::{
    build_embedding_text, chunk_text, decode_html_entities, preprocess_content, truncate_utf8,
};
pub(crate) use topics::{check_exclusions, detect_trend_topics, extract_topics};
pub(crate) use url::{redact_deep_link, validate_deep_link_url, validate_safe_url};
#[allow(unused_imports)] // Used by utils_edge_tests and test modules
pub(crate) use vector::cosine_similarity;
pub(crate) use vector::{cosine_similarity_with_norm, vector_norm};
pub(crate) use word_boundary::{
    char_at, char_before, has_bounded_match, has_word_boundary_match,
    has_word_boundary_match_with_ext, match_offsets,
};
