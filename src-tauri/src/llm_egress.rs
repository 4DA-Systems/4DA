// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! What item text may go into an LLM prompt (NETWORK.md §2a, "Privacy control").
//!
//! `privacy.llm_content_level = "titles_only"` promises that an off-machine model
//! receives item titles and no body text. Until 2026-09-24 only the rerank judge
//! read the setting, and it did so with `try_lock().unwrap_or(false)`, so a
//! briefly contended settings lock sent the body anyway. Every prompt builder that
//! puts item body text in front of a model now asks [`body_allowed`].
//!
//! The level is mirrored into an atomic whenever settings are loaded or saved
//! (the only two places `llm_content_level` changes), so a check never takes the
//! settings lock: no deadlock from a caller that already holds it, and no fail-open
//! when it is contended.
//!
//! A model on this machine (loopback endpoint, or Ollama with no base URL) is
//! exempt: nothing leaves the machine, so stripping the body would only cost
//! accuracy. `scripts/check-privacy-egress.cjs` requires every module that builds
//! an `LLMClient` to route through here or declare that it sends no item body.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::settings::LLMProvider;

static TITLES_ONLY: AtomicBool = AtomicBool::new(false);

/// Record the current `privacy.llm_content_level`. Called by the settings manager
/// on load and on every save.
pub(crate) fn publish_content_level(level: &str) {
    let now = is_titles_only_level(level);
    if TITLES_ONLY.swap(now, Ordering::SeqCst) != now {
        tracing::info!(
            target: "4da::privacy",
            titles_only = now,
            "LLM content level changed: cloud prompts {} item bodies",
            if now { "omit" } else { "include" }
        );
    }
}

/// `set_privacy_config` accepts exactly `"full"` or `"titles_only"`; anything else
/// (a hand-edited settings.json) keeps the documented default, `full`.
fn is_titles_only_level(level: &str) -> bool {
    level == "titles_only"
}

/// Whether the user has limited off-machine prompts to titles.
pub(crate) fn titles_only() -> bool {
    TITLES_ONLY.load(Ordering::SeqCst)
}

/// Whether requests to `provider` stay on this machine.
///
/// An explicit base URL decides by host ([`crate::url_validation::is_loopback_url`],
/// which rejects the `user@127.0.0.1` disguise). With no base URL only Ollama is
/// local: it defaults to `http://localhost:11434`, every other provider to its
/// public API.
pub(crate) fn provider_is_on_machine(provider: &LLMProvider) -> bool {
    match provider
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        Some(url) => crate::url_validation::is_loopback_url(url),
        None => provider.provider == "ollama",
    }
}

/// The rule, with the setting passed in so it is testable without global state.
pub(crate) fn body_allowed_for(titles_only: bool, provider_on_machine: bool) -> bool {
    !titles_only || provider_on_machine
}

/// Whether item body text may be included in a prompt sent to `provider`.
pub(crate) fn body_allowed(provider: &LLMProvider) -> bool {
    body_allowed_for(titles_only(), provider_is_on_machine(provider))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(kind: &str, base_url: Option<&str>) -> LLMProvider {
        // LLMProvider implements Drop (it zeroizes its key), so struct-update
        // syntax cannot move out of a default; set the fields instead.
        let mut p = LLMProvider::default();
        p.provider = kind.to_string();
        p.base_url = base_url.map(str::to_string);
        p
    }

    #[test]
    fn cloud_providers_are_off_machine() {
        assert!(!provider_is_on_machine(&provider("anthropic", None)));
        assert!(!provider_is_on_machine(&provider("openai", None)));
        assert!(!provider_is_on_machine(&provider(
            "openai-compatible",
            Some("https://api.groq.com/openai/v1")
        )));
    }

    #[test]
    fn default_and_loopback_endpoints_are_on_machine() {
        assert!(provider_is_on_machine(&provider("ollama", None)));
        assert!(provider_is_on_machine(&provider("ollama", Some("  "))));
        assert!(provider_is_on_machine(&provider(
            "ollama",
            Some("http://localhost:11434")
        )));
        // LM Studio / llama.cpp behind the OpenAI-compatible provider.
        assert!(provider_is_on_machine(&provider(
            "openai-compatible",
            Some("http://127.0.0.1:1234/v1")
        )));
    }

    #[test]
    fn remote_ollama_and_disguised_hosts_are_off_machine() {
        assert!(!provider_is_on_machine(&provider(
            "ollama",
            Some("http://gpu-box.lan:11434")
        )));
        assert!(!provider_is_on_machine(&provider(
            "openai-compatible",
            Some("http://api.openai.com@127.0.0.1/")
        )));
    }

    #[test]
    fn titles_only_strips_the_body_only_for_off_machine_models() {
        assert!(body_allowed_for(false, false), "full: cloud gets the body");
        assert!(body_allowed_for(false, true), "full: local gets the body");
        assert!(
            !body_allowed_for(true, false),
            "titles_only: cloud gets titles"
        );
        assert!(body_allowed_for(true, true), "titles_only: local is exempt");
    }

    // Pure parse only: the published flag is process-global and tests run in
    // parallel, so flipping it here could change what another test's prompt holds.
    #[test]
    fn only_the_exact_titles_only_value_limits_egress() {
        assert!(is_titles_only_level("titles_only"));
        assert!(!is_titles_only_level("full"));
        assert!(!is_titles_only_level("TITLES_ONLY"));
        assert!(!is_titles_only_level(""));
    }
}
