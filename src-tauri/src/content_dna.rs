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
}

impl ContentType {
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
        }
    }
}

/// Source-type-aware content classification. If the source type has a known
/// default content type, that override takes precedence over regex classification.
pub fn classify_content_for_source(
    title: &str,
    content: &str,
    source_type: &str,
) -> (ContentType, f32) {
    match source_type {
        "cve" | "osv" => return (ContentType::SecurityAdvisory, 1.30),
        "npm_registry" | "crates_io" | "pypi" | "go_modules" => {
            let tl = title.to_lowercase();
            if tl.contains("deprecated") || tl.contains("yanked") || tl.contains("[deprecated]") {
                return (ContentType::BreakingChange, 1.25);
            }
            return (ContentType::ReleaseNotes, 1.15);
        }
        "huggingface" => return (ContentType::ReleaseNotes, 1.15),
        "papers_with_code" => return (ContentType::DeepDive, 1.15),
        "stackoverflow" => return (ContentType::Question, 0.65),
        _ => {}
    }
    classify_content(title, content)
}

/// Classify content into a type and return (type, utility_multiplier).
/// Checked in priority order — first match wins.
pub fn classify_content(title: &str, content: &str) -> (ContentType, f32) {
    let title_lower = title.to_lowercase();
    let content_lower_start: String = content.chars().take(500).collect::<String>().to_lowercase();

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

    // Title contains security terms
    if security_terms.iter().any(|t| title.contains(t)) {
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
