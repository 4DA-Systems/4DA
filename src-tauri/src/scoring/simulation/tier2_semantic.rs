// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tier 2: Semantic / Embedding Scoring Validation
//!
//! Tests that the interest_score (embedding cosine similarity path) and
//! topic_embedding matching work correctly across personas and corpus items.
//! Tier 1 = keyword, Tier 2 = semantic (this file), Tier 3 = reranking.

#[cfg(test)]
#[path = "tier2_semantic_tests.rs"]
mod tests;
