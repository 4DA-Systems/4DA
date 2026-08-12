// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! LLM-availability gate shared by every surface that decides whether to attempt
//! an LLM call.
//!
//! This module used to assemble the full `PersonalizationContext` for the STREETS
//! lesson pipeline. That pipeline was retired with the STREETS tab; `compute_has_llm`
//! is the part that outlived it and is now the crate-wide source of truth.

/// Honest LLM availability: a provider must actually be selected AND usable.
/// A stale/leftover api_key with provider "none" must NOT read as has_llm — that
/// produced the first-run lie `has_llm:true` / `llm_tier:"cloud"` with no provider.
/// Ollama (the only fully-local provider) needs no key; cloud providers need one.
///
/// This is the single source of truth for "is an LLM provider configured?" — every
/// gate that decides whether to attempt an LLM call (briefings, digests, content
/// translation, summaries) must route through it rather than re-deriving the check
/// from `!api_key.is_empty()` (which a stray/env key flips true) or a single-provider
/// OR-shortcut. See antibody 2026-06-02-proxy-derived-state.
pub(crate) fn compute_has_llm(provider: &str, api_key: &str) -> bool {
    match provider {
        "none" | "" => false,
        "ollama" => true,
        _ => !api_key.is_empty(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_has_llm_is_provider_driven() {
        // The first-run lie: a leftover api_key with provider "none" must NOT
        // read as has_llm (it previously surfaced has_llm:true / llm_tier:"cloud"
        // with no provider configured).
        assert!(!compute_has_llm("none", "sk-ant-leftover-key"));
        assert!(!compute_has_llm("", "sk-ant-leftover-key"));
        assert!(!compute_has_llm("none", ""));
        // Ollama is the only fully-local provider and needs no key.
        assert!(compute_has_llm("ollama", ""));
        // Cloud providers require a key.
        assert!(!compute_has_llm("anthropic", ""));
        assert!(compute_has_llm("anthropic", "sk-ant-real"));
        assert!(compute_has_llm("openai", "sk-real"));
    }

    #[test]
    fn test_compute_has_llm_builtin_no_longer_a_keyless_provider() {
        // The built-in local LLM was removed (Phase 2). "builtin" must no longer read
        // as an always-available keyless provider — it now falls through the cloud arm,
        // so without a key it is NOT configured. The launch migration resets any
        // persisted provider=="builtin" to "none", so this value should not occur in
        // practice; this guards against it ever silently reading as has_llm again.
        assert!(!compute_has_llm("builtin", ""));
    }

    #[test]
    fn test_compute_has_llm_cloud_and_unknown_providers_need_a_key() {
        // openai-compatible is a cloud-shaped provider: usable only with a key.
        assert!(compute_has_llm("openai-compatible", "endpoint-token"));
        assert!(!compute_has_llm("openai-compatible", ""));
        // An unrecognised provider value falls through to the cloud arm — it must
        // require a key, never read as configured on its own. This keeps the helper
        // fail-safe against a future provider string nobody added a branch for.
        assert!(compute_has_llm("some-future-provider", "k"));
        assert!(!compute_has_llm("some-future-provider", ""));
        // Whitespace-only key for a cloud provider is still a non-empty string — the
        // is_empty() check is intentionally byte-level; the *frontend* trims before
        // persisting (validateApiKey / saveLlmProvider), so a trimmed-empty key never
        // reaches here as the provider value "anthropic".
        assert!(compute_has_llm("anthropic", " "));
    }
}
