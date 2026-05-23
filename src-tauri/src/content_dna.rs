// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Content DNA Classification for 4DA
//!
//! Fast regex/keyword classifier that runs BEFORE the LLM.
//! Classifies each item into a content type and applies a utility multiplier.
//! Security advisories get boosted, show-and-tell/hiring get demoted.

use serde::{Deserialize, Serialize};

#[path = "content_dna_classifiers.rs"]
mod content_dna_classifiers;
use content_dna_classifiers::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    SecurityAdvisory,
    ReleaseNotes,
    BreakingChange,
    DeepDive,
    CuratedDigest,
    ExpertAnalysis,
    PlatformUpdate,
    Tutorial,
    ShowAndTell,
    Question,
    /// Generic help request: low-information troubleshooting questions.
    /// "(help!)", "doesn't work", "error when..." — harsher than Question.
    HelpRequest,
    Hiring,
    Discussion,
    /// Clickbait: "He just crawled through hell...", "You won't believe...",
    /// vague pronouns, hyperbolic modifiers. Heavy penalty — these should
    /// almost never reach high confidence.
    Clickbait,
}

impl ContentType {
    pub fn from_slug(s: &str) -> Option<Self> {
        match s {
            "security_advisory" => Some(Self::SecurityAdvisory),
            "release_notes" => Some(Self::ReleaseNotes),
            "breaking_change" => Some(Self::BreakingChange),
            "deep_dive" => Some(Self::DeepDive),
            "curated_digest" => Some(Self::CuratedDigest),
            "expert_analysis" => Some(Self::ExpertAnalysis),
            "platform_update" => Some(Self::PlatformUpdate),
            "tutorial" => Some(Self::Tutorial),
            "show_and_tell" => Some(Self::ShowAndTell),
            "question" => Some(Self::Question),
            "help_request" => Some(Self::HelpRequest),
            "hiring" => Some(Self::Hiring),
            "discussion" => Some(Self::Discussion),
            "clickbait" => Some(Self::Clickbait),
            _ => None,
        }
    }

    pub fn slug(&self) -> &'static str {
        match self {
            ContentType::SecurityAdvisory => "security_advisory",
            ContentType::ReleaseNotes => "release_notes",
            ContentType::BreakingChange => "breaking_change",
            ContentType::DeepDive => "deep_dive",
            ContentType::CuratedDigest => "curated_digest",
            ContentType::ExpertAnalysis => "expert_analysis",
            ContentType::PlatformUpdate => "platform_update",
            ContentType::Tutorial => "tutorial",
            ContentType::ShowAndTell => "show_and_tell",
            ContentType::Question => "question",
            ContentType::HelpRequest => "help_request",
            ContentType::Hiring => "hiring",
            ContentType::Discussion => "discussion",
            ContentType::Clickbait => "clickbait",
        }
    }

    /// Content utility multiplier — calibrated for experienced developers.
    /// Experienced devs value depth, peer projects, and security over tutorials/Q&A.
    pub fn multiplier(&self) -> f32 {
        match self {
            ContentType::SecurityAdvisory => 1.30,
            ContentType::BreakingChange => 1.25,
            ContentType::ReleaseNotes => 1.15,
            ContentType::DeepDive => 1.15,
            ContentType::CuratedDigest => 1.15,
            ContentType::ExpertAnalysis => 1.12,
            ContentType::PlatformUpdate => 1.08,
            ContentType::ShowAndTell => 0.85,
            ContentType::Discussion => 1.00,
            ContentType::Tutorial => 0.80,
            ContentType::Question => 0.65,
            ContentType::HelpRequest => 0.50,
            ContentType::Hiring => 0.30,
            ContentType::Clickbait => 0.25,
        }
    }
}

/// Source-type-aware content classification.
///
/// Reads the source's declared manifest (default_content_type + multiplier)
/// instead of hardcoding match arms per source. New sources just need to
/// declare their manifest — no changes to this function.
///
/// Package registries get an extra deprecated/yanked check that overrides
/// ReleaseNotes → BreakingChange when the title indicates EOL/deprecation.
pub fn classify_content_for_source(
    title: &str,
    content: &str,
    source_type: &str,
) -> (ContentType, f32) {
    // Build lookup from source manifests (cached after first call)
    use std::sync::OnceLock;
    static DEFAULTS: OnceLock<std::collections::HashMap<&'static str, (ContentType, f32, bool)>> =
        OnceLock::new();

    let defaults = DEFAULTS.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        for source in crate::sources::build_all_sources() {
            let m = source.manifest();
            let ct =
                ContentType::from_slug(m.default_content_type).unwrap_or(ContentType::Discussion);
            let is_registry = matches!(m.category, crate::sources::SourceCategory::PackageRegistry);
            // Only store if non-default (Discussion/1.0) or registry (needs deprecation check)
            if ct != ContentType::Discussion || m.default_multiplier != 1.0 || is_registry {
                map.insert(
                    source.source_type(),
                    (ct, m.default_multiplier, is_registry),
                );
            }
        }
        map
    });

    if let Some(&(ref ct, mult, is_registry)) = defaults.get(source_type) {
        // Package registries: check for deprecated/yanked/EOL → BreakingChange override
        if is_registry && *ct == ContentType::ReleaseNotes {
            let tl = title.to_lowercase();
            if tl.contains("deprecated")
                || tl.contains("yanked")
                || tl.contains("[deprecated]")
                || tl.contains("end of life")
                || tl.contains("no longer maintained")
                || tl.contains("archived")
                || tl.contains("unmaintained")
            {
                return (ContentType::BreakingChange, 1.25);
            }
        }

        // Clickbait override for any source — even if the manifest says
        // "discussion" or "show_and_tell", a clickbait title should be
        // suppressed. Critical for YouTube (default Discussion 1.0x).
        if is_clickbait(&title.to_lowercase()) {
            return (ContentType::Clickbait, ContentType::Clickbait.multiplier());
        }

        return (ct.clone(), mult);
    }

    // Unknown source — fall through to regex classifier
    classify_content(title, content)
}

/// Source-aware classifier with reputation uplift/penalty.
///
/// Runs the standard `classify_content_for_source` and then multiplies the
/// content-type multiplier by the per-source reputation multiplier. This is
/// how user-added feeds (RSS URLs, YouTube handles) get differentiated from
/// generic sources: a known-good blog gets a 1.30x uplift on top of its
/// content type, while a generic aggregator stays at 1.0x.
///
/// `source_identifier` is the per-item provenance string:
///   - RSS: the `feed_url` from the item metadata
///   - YouTube: the channel ID or @handle
///   - Twitter: the handle (without @)
///   - Built-in sources: fall through to the source_type (no per-source override)
///
/// Returns (content_type, combined_multiplier) where combined is clamped to
/// [0.15, 1.70] so a clickbait × bad-source combination can't go lower than a
/// hiring post would and a security-advisory × top-tier-source can't exceed
/// reasonable bounds.
/// Classify content into a type and return (type, utility_multiplier).
/// Checked in priority order — first match wins.
pub fn classify_content(title: &str, content: &str) -> (ContentType, f32) {
    let title_lower = title.to_lowercase();
    let content_lower_start: String = content.chars().take(500).collect::<String>().to_lowercase();

    // 0. Clickbait (check before security — a CVE article isn't written in
    //    clickbait form, so this won't cross-contaminate. But "He just
    //    crawled through HELL to fix security" would wrongly pass as security.)
    //    Clickbait is suppressed hard (0.25x multiplier) regardless of topic.
    if is_clickbait(&title_lower) {
        return (ContentType::Clickbait, ContentType::Clickbait.multiplier());
    }

    // 1. Security Advisory (highest priority — urgent)
    if is_security(&title_lower, &content_lower_start) {
        return (
            ContentType::SecurityAdvisory,
            ContentType::SecurityAdvisory.multiplier(),
        );
    }

    // 2. Breaking Change (urgent action needed)
    if is_breaking_change(&title_lower) {
        return (
            ContentType::BreakingChange,
            ContentType::BreakingChange.multiplier(),
        );
    }

    // 3. Hiring (lowest value — check early to short-circuit)
    if is_hiring(&title_lower) {
        return (ContentType::Hiring, ContentType::Hiring.multiplier());
    }

    // 4. Release Notes
    if is_release(&title_lower) {
        return (
            ContentType::ReleaseNotes,
            ContentType::ReleaseNotes.multiplier(),
        );
    }

    // 5. Show and Tell
    if is_show_and_tell(&title_lower) {
        return (
            ContentType::ShowAndTell,
            ContentType::ShowAndTell.multiplier(),
        );
    }

    // 5b. Educational repositories (GitHub roadmaps, awesome-lists, learn-* repos)
    if is_educational_repo(&title_lower) {
        return (ContentType::Tutorial, ContentType::Tutorial.multiplier());
    }

    // 6. Generic Help Request (harsher than regular question — low information density)
    if is_generic_help_request(&title_lower) {
        return (
            ContentType::HelpRequest,
            ContentType::HelpRequest.multiplier(),
        );
    }

    // 7. Question
    if is_question(&title_lower) {
        return (ContentType::Question, ContentType::Question.multiplier());
    }

    // 8. Tutorial
    if is_tutorial(&title_lower) {
        return (ContentType::Tutorial, ContentType::Tutorial.multiplier());
    }

    // 8. Deep Dive (long-form that doesn't match other categories)
    if content.len() > 2000 {
        return (ContentType::DeepDive, ContentType::DeepDive.multiplier());
    }

    // 9. Discussion (fallback)
    (
        ContentType::Discussion,
        ContentType::Discussion.multiplier(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_cve() {
        let (ct, mult) = classify_content("CVE-2025-1234: Critical RCE in OpenSSL", "");
        assert_eq!(ct, ContentType::SecurityAdvisory);
        assert_eq!(mult, 1.30);
    }

    // ========================================================================
    // Clickbait detection tests — the screenshot offender class
    // ========================================================================

    #[test]
    fn test_clickbait_he_just_pattern() {
        // The exact screenshot offender
        let (ct, mult) = classify_content("He just crawled through hell to fix the browser...", "");
        assert_eq!(ct, ContentType::Clickbait);
        assert_eq!(mult, 0.25);
    }

    #[test]
    fn test_clickbait_vague_pronoun_openers() {
        for title in &[
            "She just solved a decade-old bug",
            "They just shipped the biggest update",
            "This guy built Linux in a weekend",
            "This dev made something incredible",
            "Watch him fix this in 10 minutes",
        ] {
            let (ct, _) = classify_content(title, "");
            assert_eq!(ct, ContentType::Clickbait, "Should flag: {title}");
        }
    }

    #[test]
    fn test_clickbait_hyperbolic_phrases() {
        for title in &[
            "You won't believe what this CLI can do",
            "The shocking truth about async/await",
            "One weird trick to speed up your build",
            "This framework goes viral overnight",
        ] {
            let (ct, _) = classify_content(title, "");
            assert_eq!(ct, ContentType::Clickbait, "Should flag: {title}");
        }
    }

    #[test]
    fn test_clickbait_punctuation_spam() {
        let (ct, _) = classify_content("Did they really break the API?!?", "");
        assert_eq!(ct, ContentType::Clickbait);
    }

    #[test]
    fn test_clickbait_caps_emotional() {
        let (ct, _) = classify_content("I went through hell debugging this", "");
        assert_eq!(ct, ContentType::Clickbait);
    }

    #[test]
    fn test_clickbait_does_not_flag_legit_security() {
        // A real CVE article should NOT be flagged as clickbait
        let (ct, _) = classify_content(
            "CVE-2025-55184: React Server Components DoS Vulnerability",
            "",
        );
        assert_eq!(ct, ContentType::SecurityAdvisory);
    }

    #[test]
    fn test_clickbait_does_not_flag_legit_announcements() {
        // Real announcements should pass through
        for title in &[
            "Announcing Rust 1.94.0",
            "React 19 is now available",
            "Bun 2.0: Faster, Smaller, More Standard",
            "Show HN: My Weekend Project",
        ] {
            let (ct, _) = classify_content(title, "");
            assert_ne!(ct, ContentType::Clickbait, "Should NOT flag: {title}");
        }
    }

    #[test]
    fn test_clickbait_via_source_aware_classifier() {
        // YouTube items normally default to Discussion (1.0x) — the clickbait
        // override should still kick in via classify_content_for_source.
        let (ct, mult) = classify_content_for_source(
            "He just crawled through hell to fix the browser...",
            "video transcript...",
            "youtube",
        );
        assert_eq!(ct, ContentType::Clickbait);
        assert_eq!(mult, 0.25);
    }

    #[test]
    fn test_security_vulnerability() {
        let (ct, _) = classify_content("Critical vulnerability found in popular npm package", "");
        assert_eq!(ct, ContentType::SecurityAdvisory);
    }

    #[test]
    fn test_release_notes() {
        let (ct, mult) = classify_content("React 20.0 released with new compiler", "");
        assert_eq!(ct, ContentType::ReleaseNotes);
        assert_eq!(mult, 1.15);
    }

    #[test]
    fn test_breaking_change() {
        let (ct, mult) = classify_content("Node.js 24 drops support for CommonJS", "");
        assert_eq!(ct, ContentType::BreakingChange);
        assert_eq!(mult, 1.25);
    }

    #[test]
    fn test_show_and_tell() {
        let (ct, mult) =
            classify_content("Show HN: I built a terminal music player in Rust", "short");
        assert_eq!(ct, ContentType::ShowAndTell);
        assert_eq!(mult, 0.85); // Peer projects valuable for experienced devs
    }

    #[test]
    fn test_i_built() {
        let (ct, _) = classify_content("i built a new framework for web apps", "short");
        assert_eq!(ct, ContentType::ShowAndTell);
    }

    #[test]
    fn test_question() {
        let (ct, mult) = classify_content("How do I handle async errors in Rust?", "");
        assert_eq!(ct, ContentType::Question);
        assert_eq!(mult, 0.65);
    }

    #[test]
    fn test_tutorial() {
        let (ct, mult) = classify_content("How to build a REST API with Axum", "");
        assert_eq!(ct, ContentType::Tutorial);
        assert_eq!(mult, 0.80);
    }

    #[test]
    fn test_hiring() {
        let (ct, mult) = classify_content("We're hiring senior Rust engineers (remote)", "");
        assert_eq!(ct, ContentType::Hiring);
        assert_eq!(mult, 0.30);
    }

    #[test]
    fn test_deep_dive() {
        let long_content = "x".repeat(2500);
        let (ct, mult) = classify_content("Understanding memory allocators in Rust", &long_content);
        assert_eq!(ct, ContentType::DeepDive);
        assert_eq!(mult, 1.15); // Experienced devs crave depth
    }

    #[test]
    fn test_discussion_fallback() {
        let (ct, mult) = classify_content("Thoughts on the future of WebAssembly", "short content");
        assert_eq!(ct, ContentType::Discussion);
        assert_eq!(mult, 1.00);
    }

    #[test]
    fn test_deprecated() {
        let (ct, _) = classify_content("React class components deprecated in v21", "");
        assert_eq!(ct, ContentType::BreakingChange);
    }

    #[test]
    fn test_changelog() {
        let (ct, _) = classify_content("Changelog for Tokio 2.0", "");
        assert_eq!(ct, ContentType::ReleaseNotes);
    }

    #[test]
    fn test_security_patch() {
        let (ct, _) = classify_content("Critical security patch for OpenSSL", "");
        assert_eq!(ct, ContentType::SecurityAdvisory);
    }

    #[test]
    fn test_ask_hn() {
        let (ct, _) = classify_content("ask hn: what database do you use for side projects?", "");
        assert_eq!(ct, ContentType::Question);
    }

    #[test]
    fn test_career_switch_post() {
        let (ct, _) = classify_content(
            "startup frontend dev (f/d) planning switch to product mnc",
            "",
        );
        assert_eq!(ct, ContentType::Hiring);
    }

    #[test]
    fn test_job_posting_marker() {
        let (ct, _) = classify_content("senior rust engineer (m/f/d) - berlin", "");
        assert_eq!(ct, ContentType::Hiring);
    }

    // ========================================================================
    // Educational repository detection
    // ========================================================================

    #[test]
    fn test_educational_developer_roadmap() {
        let (ct, mult) = classify_content(
            "nilbuild/developer-roadmap (★355244 • TypeScript)",
            "Generic developer roadmap educational...",
        );
        assert_eq!(ct, ContentType::Tutorial);
        assert_eq!(mult, 0.80);
    }

    #[test]
    fn test_educational_awesome_list() {
        let (ct, _) = classify_content("sindresorhus/awesome-rust (★42000 • Rust)", "");
        assert_eq!(ct, ContentType::Tutorial);
    }

    #[test]
    fn test_educational_freecodecamp() {
        let (ct, _) = classify_content("freecodecamp/freecodecamp (★445282 • TypeScript)", "");
        assert_eq!(ct, ContentType::Tutorial);
    }

    #[test]
    fn test_educational_learn_repo() {
        let (ct, _) = classify_content("nicolo-ribaudo/learn-typescript (★5000 • TypeScript)", "");
        assert_eq!(ct, ContentType::Tutorial);
    }

    #[test]
    fn test_non_educational_real_tool() {
        // Real tools should NOT be classified as educational
        let (ct, _) = classify_content("shadcn-ui/ui (★114864 • TypeScript)", "short");
        assert_ne!(ct, ContentType::Tutorial);
    }

    #[test]
    fn test_non_educational_no_star() {
        // Non-GitHub items with "roadmap" in title should not be affected
        let (ct, _) = classify_content("Our Q3 product roadmap update", "short");
        assert_ne!(ct, ContentType::Tutorial);
    }

    #[test]
    fn test_rebuilt_is_show_and_tell() {
        let (ct, _) = classify_content(
            "i rebuilt my family's clean architecture boilerplate",
            "short",
        );
        assert_eq!(ct, ContentType::ShowAndTell);
    }

    #[test]
    fn test_rewrote_is_show_and_tell() {
        let (ct, _) = classify_content("i rewrote our api gateway in rust", "short");
        assert_eq!(ct, ContentType::ShowAndTell);
    }

    #[test]
    fn test_ported_is_show_and_tell() {
        let (ct, _) = classify_content("i ported doom to run on a microcontroller", "short");
        assert_eq!(ct, ContentType::ShowAndTell);
    }
}
