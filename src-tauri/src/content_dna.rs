//! Content DNA Classification for 4DA
//!
//! Fast regex/keyword classifier that runs BEFORE the LLM.
//! Classifies each item into a content type and applies a utility multiplier.
//! Security advisories get boosted, show-and-tell/hiring get demoted.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    SecurityAdvisory,
    ReleaseNotes,
    BreakingChange,
    DeepDive,
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
            ContentType::ShowAndTell => 0.85,
            ContentType::Discussion => 1.00,
            ContentType::Tutorial => 0.80,
            ContentType::Question => 0.65,
            ContentType::HelpRequest => 0.50,
            ContentType::Hiring => 0.30,
            // Clickbait: heavy penalty. YouTube drama, vague pronouns,
            // hyperbolic modifiers — these poison the high-confidence band
            // if they score well on other axes.
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
pub fn classify_content_for_source_with_reputation(
    title: &str,
    content: &str,
    source_type: &str,
    source_identifier: Option<&str>,
) -> (ContentType, f32) {
    let (ct, content_mult) = classify_content_for_source(title, content, source_type);
    let reputation_mult = source_identifier
        .map(crate::source_reputation::get_curated_prior)
        .unwrap_or(1.0);
    let combined = (content_mult * reputation_mult).clamp(0.15, 1.70);
    (ct, combined)
}

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

/// Detects clickbait title patterns — the kind of YouTube thumbnail language
/// that poisons a developer feed. Senior devs close the tab instantly when
/// they see "He just crawled through hell..." ranked as high-confidence signal.
///
/// Patterns checked:
/// - Vague-pronoun openers ("He just", "She just", "They just", "This guy", "This dev")
/// - Hyperbolic reaction words ("shocked", "insane", "crazy", "absolute", "mind-blowing")
/// - Clickbait phrases ("you won't believe", "will shock you", "nobody is talking about")
/// - Excessive question/emphasis ("?!?", "!!!")
/// - ALL CAPS fragments (WHY, HELL, CRAZY, INSANE, SHOCKING)
///
/// The detector is intentionally conservative — false negatives are fine,
/// false positives on real technical content are not.
fn is_clickbait(title: &str) -> bool {
    // Vague-pronoun openers — a technical title names the subject.
    // "He just X...", "She just Y...", "This guy Z..." — all clickbait shapes.
    let vague_openers = [
        "he just ",
        "she just ",
        "they just ",
        "this guy ",
        "this dev ",
        "this developer ",
        "this programmer ",
        "this engineer ",
        "this person ",
        "watch him ",
        "watch her ",
        "watch this ",
    ];
    if vague_openers.iter().any(|p| title.starts_with(p)) {
        return true;
    }

    // Hyperbolic/emotional reaction words — a technical writeup doesn't need them.
    let hyperbolic = [
        "you won't believe",
        "you wont believe",
        "will shock you",
        "will blow your mind",
        "mind-blowing",
        "mind blowing",
        "nobody is talking about",
        "no one is talking about",
        "what happens next",
        "you'll never guess",
        "youll never guess",
        "absolute madlad",
        "absolute mad lad",
        "absolute unit",
        "absolutely insane",
        "goes insane",
        "went insane",
        "goes viral",
        "went viral",
        "shocking truth",
        "the truth about",
        "secret trick",
        "one weird trick",
    ];
    if hyperbolic.iter().any(|p| title.contains(p)) {
        return true;
    }

    // Multi-punctuation (clickbait thumbnails: "?!?", "!!!", "?!")
    if title.contains("?!") || title.contains("!!!") || title.contains("!?") {
        return true;
    }

    // ALL-CAPS emotional fragments — technical titles use code identifiers,
    // not screaming. Count 5+ letter ALL-CAPS word occurrences that aren't
    // technical terms (CPU, GPU, API, JSON, HTML, CSS, HTTP, HTTPS, etc.).
    //
    // NOTE: The caller passes a lowercase string, so this check cannot detect
    // caps directly. Instead, we check for common clickbait caps-words by
    // their lowercase form paired with emotional context.
    let caps_emotional = [
        " hell ",
        "through hell",
        "into hell",
        "from hell",
        " wtf ",
        " omg ",
        " lmao ",
        " lol ",
        " wth ",
    ];
    if caps_emotional.iter().any(|p| title.contains(p)) {
        return true;
    }

    false
}

fn is_security(title: &str, content_start: &str) -> bool {
    // CVE pattern
    if contains_cve(title) || contains_cve(content_start) {
        return true;
    }

    let security_terms = [
        "vulnerability",
        "exploit",
        "security advisory",
        "zero-day",
        "zero day",
        "zeroday",
        "ransomware",
        "backdoor",
        "supply chain attack",
        "remote code execution",
        "rce",
        "privilege escalation",
    ];

    // Title contains security terms (word-boundary matched to prevent
    // "rce" matching inside "source"/"resource" etc.)
    if security_terms
        .iter()
        .any(|t| crate::scoring::has_word_boundary_match(title, t))
    {
        return true;
    }

    // "patch" or "update" combined with "security" or "critical"
    if (title.contains("patch") || title.contains("update"))
        && (title.contains("security") || title.contains("critical"))
    {
        return true;
    }

    false
}

fn contains_cve(text: &str) -> bool {
    // Match CVE-YYYY or cve YYYY patterns
    let bytes = text.as_bytes();
    for i in 0..text.len().saturating_sub(7) {
        if (bytes[i] == b'c' || bytes[i] == b'C')
            && (bytes[i + 1] == b'v' || bytes[i + 1] == b'V')
            && (bytes[i + 2] == b'e' || bytes[i + 2] == b'E')
        {
            // CVE followed by - or space, then 4 digits
            if i + 3 < bytes.len()
                && (bytes[i + 3] == b'-' || bytes[i + 3] == b' ')
                && i + 7 < bytes.len()
                && bytes[i + 4].is_ascii_digit()
                && bytes[i + 5].is_ascii_digit()
                && bytes[i + 6].is_ascii_digit()
                && bytes[i + 7].is_ascii_digit()
            {
                return true;
            }
        }
    }
    false
}

fn is_breaking_change(title: &str) -> bool {
    let terms = [
        "breaking change",
        "deprecated",
        "end of life",
        "eol",
        "migration guide",
        "drops support",
        "removed in",
        "sunset",
        "backwards incompatible",
        "backward incompatible",
    ];
    terms.iter().any(|t| title.contains(t))
}

fn is_release(title: &str) -> bool {
    // "vX.Y released/is out/available/launched/beta/rc/alpha"
    let bytes = title.as_bytes();
    for i in 0..title.len().saturating_sub(3) {
        // Look for digit.digit pattern
        if bytes[i].is_ascii_digit() && bytes[i + 1] == b'.' && bytes[i + 2].is_ascii_digit() {
            // Check if followed by release-related words
            let rest = &title[i..];
            if rest.contains("released")
                || rest.contains("is out")
                || rest.contains("available")
                || rest.contains("launched")
                || rest.contains(" beta")
                || rest.contains(" rc")
                || rest.contains(" alpha")
            {
                return true;
            }
        }
    }

    let release_terms = ["changelog", "what's new in", "release notes"];
    release_terms.iter().any(|t| title.contains(t))
}

fn is_show_and_tell(title: &str) -> bool {
    let start_patterns = [
        "i built",
        "i made",
        "i created",
        "i rebuilt",
        "i rewrote",
        "i redesigned",
        "i remade",
        "i ported",
        "show hn",
        "launch hn",
    ];
    if start_patterns.iter().any(|p| title.starts_with(p)) {
        return true;
    }

    let contains_patterns = [
        "just launched",
        "open-sourced my",
        "open sourced my",
        "releasing my",
        "my new project",
        "i've been working on",
        "i have been working on",
        "check out my",
        "here's what i built",
        "here is what i built",
    ];
    contains_patterns.iter().any(|p| title.contains(p))
}

/// Detect generic help requests — low-information troubleshooting questions.
/// "(help!)", "doesn't work", "error when..." without specifics.
/// These get a harsher penalty (0.55) than regular questions (0.70) because
/// they lack the specificity that makes questions useful for learning.
fn is_generic_help_request(title: &str) -> bool {
    // End patterns: "(help!)", "(help?)", "help needed"
    let end_patterns = [
        "(help!)",
        "(help?)",
        "(help)",
        "help!",
        "help?",
        "please help",
        "help needed",
        "help me",
        "any ideas?",
        "any suggestions?",
        "any thoughts?",
    ];
    if end_patterns.iter().any(|p| title.ends_with(p)) {
        return true;
    }

    // Start patterns: vague problem descriptions
    let start_patterns = [
        "error when",
        "problem with",
        "issue with",
        "trouble with",
        "struggling with",
        "stuck on",
        "can't get",
        "cannot get",
        "unable to",
        "why does my",
        "why doesn't",
        "why won't",
        "why isn't",
    ];
    if start_patterns.iter().any(|p| title.starts_with(p)) {
        return true;
    }

    // Contains patterns: generic troubleshooting
    let contains_patterns = [
        "doesn't work",
        "does not work",
        "not working",
        "broken after",
        "stopped working",
        "won't compile",
        "won't build",
        "can anyone explain",
    ];
    contains_patterns.iter().any(|p| title.contains(p))
}

fn is_question(title: &str) -> bool {
    let start_patterns = [
        "how do",
        "what's the",
        "what is the",
        "need advice",
        "help with",
        "is there",
        "can someone",
        "does anyone",
    ];
    if start_patterns.iter().any(|p| title.starts_with(p)) {
        return true;
    }

    let contains_patterns = [
        "looking for suggestions",
        "recommendations for",
        "which should i",
        "ask hn:",
    ];
    contains_patterns.iter().any(|p| title.contains(p))
}

fn is_tutorial(title: &str) -> bool {
    if title.starts_with("how to") {
        return true;
    }

    let terms = [
        "getting started",
        "step by step",
        "step-by-step",
        "tutorial",
        "beginner guide",
        "beginner's guide",
        "introduction to",
        "learn to",
    ];
    terms.iter().any(|t| title.contains(t))
}

fn is_hiring(title: &str) -> bool {
    let terms = [
        "hiring",
        "job opening",
        "remote position",
        "join our team",
        "join the team",
        "we're hiring",
        "now hiring",
    ];
    if terms.iter().any(|t| title.contains(t)) {
        return true;
    }

    // "looking for X developer/engineer"
    if title.contains("looking for") && (title.contains("developer") || title.contains("engineer"))
    {
        return true;
    }

    // Career/job-switch posts: "Frontend Dev planning switch to MNC"
    let has_role = title.contains("dev ")
        || title.contains("developer")
        || title.contains("engineer")
        || title.contains("swe ")
        || title.contains("sde ");
    let has_career = title.contains("switch")
        || title.contains("career")
        || title.contains("moving to")
        || title.contains("transitioning")
        || title.contains("job hunt")
        || title.contains("interview");
    if has_role && has_career {
        return true;
    }

    // Job posting markers: "(f/d)", "(m/f/d)", "full-time", "part-time"
    let job_markers = ["(f/d)", "(m/f/d)", "(m/w/d)", "full-time", "part-time"];
    if job_markers.iter().any(|m| title.contains(m)) {
        return true;
    }

    false
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

    // ========================================================================
    // Reputation-aware classifier tests
    // ========================================================================

    #[test]
    fn test_reputation_uplifts_known_good_source() {
        // Deno blog is in the curated prior list at 1.30x.
        // A Discussion-type post from there should multiply.
        let (ct, mult) = classify_content_for_source_with_reputation(
            "Some discussion about web standards",
            "body...",
            "rss",
            Some("https://blog.deno.com/posts/1"),
        );
        assert_eq!(ct, ContentType::Discussion);
        // Base Discussion = 1.0, reputation = 1.30, combined = 1.30
        assert!((mult - 1.30).abs() < 0.01, "expected ~1.30, got {mult}");
    }

    #[test]
    fn test_reputation_penalty_stacks_with_clickbait() {
        // Clickbait from a low-signal domain should be doubly penalized.
        let (ct, mult) = classify_content_for_source_with_reputation(
            "He just did something you won't believe!",
            "body...",
            "rss",
            Some("https://techcrunch.com/article"),
        );
        assert_eq!(ct, ContentType::Clickbait);
        // Clickbait 0.25 * techcrunch 0.75 = 0.1875 → clamped at 0.15 floor
        assert!(mult <= 0.25, "penalty should stack, got {mult}");
    }

    #[test]
    fn test_reputation_unknown_source_is_neutral() {
        // A random unknown domain gets 1.0 prior = no change vs plain classifier.
        let (ct_a, mult_a) = classify_content_for_source_with_reputation(
            "Announcing Foo 1.0",
            "",
            "rss",
            Some("https://random-blog.example.com/post"),
        );
        let (ct_b, mult_b) = classify_content_for_source("Announcing Foo 1.0", "", "rss");
        assert_eq!(ct_a, ct_b);
        assert!((mult_a - mult_b).abs() < 0.01);
    }

    #[test]
    fn test_reputation_combined_clamped_to_range() {
        // Even a 1.30x source with a 1.30x content type is capped at 1.70
        let (_, mult) = classify_content_for_source_with_reputation(
            "CVE-2026-1234: Critical vulnerability",
            "security advisory content",
            "rss",
            Some("https://blog.rust-lang.org/post"),
        );
        assert!(
            mult <= 1.70,
            "combined multiplier should not exceed 1.70, got {mult}"
        );
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
