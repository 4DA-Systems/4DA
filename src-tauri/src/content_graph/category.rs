// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Content category — the graph's primary color channel.
//!
//! Source identity made a poor color channel: 22 source types shared 8 hues
//! (five on one red), and provenance is tooltip detail, not what a user scans
//! for. Four categories cover the corpus, sit within the perceptual limit for
//! instant hue decoding, and each carries a distinct node silhouette so the
//! encoding survives color-vision deficiency and grayscale.

pub(super) const CATEGORY_SECURITY: &str = "security";
pub(super) const CATEGORY_RELEASE: &str = "release";
pub(super) const CATEGORY_DISCUSSION: &str = "discussion";
pub(super) const CATEGORY_RESEARCH: &str = "research";

/// Category from source identity + persisted signal type. `signal_type`
/// wins where present (a security alert from any source IS security);
/// otherwise the source class decides. Unknown sources read as discussion —
/// the safest neutral.
pub(super) fn category_for(source_type: &str, signal_type: Option<&str>) -> &'static str {
    if signal_type == Some("security_alert") {
        return CATEGORY_SECURITY;
    }
    match source_type {
        "osv" | "cve" => CATEGORY_SECURITY,
        "crates_io" | "npm" | "pypi" | "go_modules" | "github" | "huggingface" | "producthunt" => {
            CATEGORY_RELEASE
        }
        "arxiv" | "papers_with_code" => CATEGORY_RESEARCH,
        _ => CATEGORY_DISCUSSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisory_sources_are_security() {
        assert_eq!(category_for("osv", None), CATEGORY_SECURITY);
        assert_eq!(category_for("cve", None), CATEGORY_SECURITY);
    }

    #[test]
    fn signal_type_overrides_source() {
        assert_eq!(
            category_for("hackernews", Some("security_alert")),
            CATEGORY_SECURITY
        );
    }

    #[test]
    fn package_registries_are_release() {
        for s in ["crates_io", "npm", "pypi", "go_modules"] {
            assert_eq!(category_for(s, None), CATEGORY_RELEASE);
        }
    }

    #[test]
    fn papers_are_research_and_unknown_is_discussion() {
        assert_eq!(category_for("arxiv", None), CATEGORY_RESEARCH);
        assert_eq!(category_for("papers_with_code", None), CATEGORY_RESEARCH);
        assert_eq!(category_for("mastodon", None), CATEGORY_DISCUSSION);
        assert_eq!(category_for("some_new_source", None), CATEGORY_DISCUSSION);
    }
}
