// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Stack sources vs interests (AD-054 decision 3).
//!
//! The dependency-truth engine needs the registries and the advisory feeds:
//! they are how a lockfile becomes a finding, a fix path and an upgrade plan.
//! They stay on and cannot be turned off. Everything else is social or
//! editorial reading — an opt-in "interest", off on a new install and off
//! after the Phase 126 migration on an existing one, until the user turns it
//! on in Settings.
//!
//! The class is a fact about the source type, so it is decided here once and
//! read by the registry seed (`Database::register_source`), the enable
//! writer (`set_source_enabled`) and the Settings list. Whether a given
//! install has an interest ON lives in `sources.enabled`; every reader of
//! source items honours that column through [`enabled_source_sql`].

/// The source types the engine needs: package registries and advisories.
/// Always on. AD-054: "Registries (crates.io, npm, PyPI, Go) and advisories
/// (OSV, CVE) stay always-on because the engine needs them."
pub(crate) const STACK_SOURCES: &[&str] = &[
    "crates_io",
    "npm_registry",
    "pypi",
    "go_modules",
    "osv",
    "cve",
];

/// Every interest this build has an adapter for — the named list the Brief's
/// "worth knowing" section reads from (`brief_interests`). A test pins it to
/// the built adapters.
pub(crate) const INTEREST_SOURCES: &[&str] = &[
    "arxiv",
    "bluesky",
    "devto",
    "github",
    "hackernews",
    "huggingface",
    "lemmy",
    "lobsters",
    "mastodon",
    "papers_with_code",
    "producthunt",
    "reddit",
    "rss",
    "stackoverflow",
    "twitter",
    "youtube",
];

/// Whether `source_type` is one of the engine's always-on stack sources.
pub(crate) fn is_stack_source(source_type: &str) -> bool {
    STACK_SOURCES.contains(&source_type)
}

/// Whether `source_type` is an opt-in interest. Every source that is not a
/// stack source is one — including a source added in a later release, which
/// therefore arrives off rather than silently widening what 4DA reads.
pub(crate) fn is_interest(source_type: &str) -> bool {
    !is_stack_source(source_type)
}

/// The `enabled` value a source starts with the first time it is registered.
pub(crate) fn default_enabled(source_type: &str) -> bool {
    is_stack_source(source_type)
}

/// Wire name of the class, for the Settings list (`"stack"` / `"interest"`).
pub(crate) fn class_name(source_type: &str) -> &'static str {
    if is_stack_source(source_type) {
        "stack"
    } else {
        "interest"
    }
}

/// SQL predicate: `col` names a source that is not turned off.
///
/// A source with no `sources` row counts as on — the same default
/// `Database::is_source_enabled` applies before the registry seed runs, and
/// a stack source never has `enabled = 0` (the writer refuses it).
pub(crate) fn enabled_source_sql(col: &str) -> String {
    format!("{col} NOT IN (SELECT source_type FROM sources WHERE enabled = 0)")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every adapter the app builds is classified, and the split is exactly
    /// AD-054's: registries + advisories are stack, everything else interest.
    #[test]
    fn every_built_adapter_is_classified_per_ad054() {
        let types: Vec<&'static str> = crate::sources::build_all_sources()
            .iter()
            .map(|s| s.source_type())
            .collect();
        for stack in STACK_SOURCES {
            assert!(types.contains(stack), "stack source {stack} has no adapter");
        }
        let interests: Vec<&str> = types.iter().copied().filter(|t| is_interest(t)).collect();
        for named in [
            "hackernews",
            "lobsters",
            "mastodon",
            "devto",
            "reddit",
            "lemmy",
            "bluesky",
            "youtube",
            "huggingface",
            "arxiv",
            "papers_with_code",
            "stackoverflow",
            "producthunt",
            "twitter",
            "rss",
            "github",
        ] {
            assert!(interests.contains(&named), "{named} must be an interest");
        }
        assert_eq!(
            interests.len() + STACK_SOURCES.len(),
            types.len(),
            "every adapter is exactly one class"
        );
        let mut sorted = interests.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted, INTEREST_SOURCES,
            "INTEREST_SOURCES names every interest adapter"
        );
    }

    #[test]
    fn unknown_sources_default_to_an_opt_in_interest() {
        assert!(is_interest("some_future_source"));
        assert!(!default_enabled("some_future_source"));
        assert!(default_enabled("osv"));
        assert_eq!(class_name("cve"), "stack");
        assert_eq!(class_name("hackernews"), "interest");
    }

    #[test]
    fn enabled_predicate_names_the_column() {
        assert_eq!(
            enabled_source_sql("si.source_type"),
            "si.source_type NOT IN (SELECT source_type FROM sources WHERE enabled = 0)"
        );
    }
}
