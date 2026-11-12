// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Hooks for the engine end-to-end gate (`scripts/ci/engine-e2e.mjs`), and
//! nothing else.
//!
//! They exist only in a debug build with the `e2e-osv-mirror` feature: a
//! release build with that feature does not compile, and without it every
//! hook below is a constant `None`/`false`. No installed 4DA reads these
//! variables.
//!
//! - `FOURDA_E2E_OSV_MIRROR`: the base URL that replaces OSV's archive bucket,
//!   so the shipped sync pass runs against frozen archives with no network.
//! - `FOURDA_E2E_SKIP_FEED_FETCH=1`: skip cycle step 1 (news and registry
//!   feeds). With the network blocked every feed adapter spends its full
//!   retry budget (3 x 90 s, measured 2026-11-12) before the dependency step;
//!   the gate asserts on the dependency path, and the feed adapters have their
//!   own tests.

#[cfg(all(feature = "e2e-osv-mirror", not(debug_assertions)))]
compile_error!("the e2e-osv-mirror feature is test-only and must never be in a release build");

/// The frozen advisory mirror's base URL, when the gate set one.
#[cfg(feature = "e2e-osv-mirror")]
pub(crate) fn osv_mirror_base_url() -> Option<String> {
    std::env::var("FOURDA_E2E_OSV_MIRROR")
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
}

#[cfg(not(feature = "e2e-osv-mirror"))]
pub(crate) fn osv_mirror_base_url() -> Option<String> {
    None
}

/// Whether the gate asked the cycle to skip the feed fetch.
#[cfg(feature = "e2e-osv-mirror")]
pub(crate) fn skip_feed_fetch() -> bool {
    std::env::var("FOURDA_E2E_SKIP_FEED_FETCH").is_ok_and(|v| v.trim() == "1")
}

#[cfg(not(feature = "e2e-osv-mirror"))]
pub(crate) fn skip_feed_fetch() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(not(feature = "e2e-osv-mirror"))]
    fn without_the_feature_no_hook_is_live() {
        // The variables are set, and a default build still ignores them.
        std::env::set_var("FOURDA_E2E_OSV_MIRROR", "http://127.0.0.1:1");
        std::env::set_var("FOURDA_E2E_SKIP_FEED_FETCH", "1");
        assert_eq!(super::osv_mirror_base_url(), None);
        assert!(!super::skip_feed_fetch());
        std::env::remove_var("FOURDA_E2E_OSV_MIRROR");
        std::env::remove_var("FOURDA_E2E_SKIP_FEED_FETCH");
    }
}
