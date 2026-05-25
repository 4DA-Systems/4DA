// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Semantic scoring — vector-similarity ACE boost, topic enrichment, embedding cache, and taste profiling.

mod boost;
mod embeddings;
mod enrichment;
mod taste;
#[cfg(test)]
mod tests;

// Re-export public API at the same paths as before the split
pub(crate) use boost::{compute_keyword_ace_boost, compute_semantic_ace_boost};
pub(crate) use embeddings::get_topic_embeddings;
// enrich_topic_for_embedding is pub(crate) for future external use + test access via `super::*`
#[allow(unused_imports)]
pub(crate) use enrichment::enrich_topic_for_embedding;
pub(crate) use taste::{compute_taste_boost, compute_taste_embedding};
