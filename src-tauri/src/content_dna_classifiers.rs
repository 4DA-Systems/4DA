// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Private classification helper functions for content DNA.
//!
//! Pure functions that take string slices and return bools.
//! Extracted from `content_dna.rs` to keep both files under the 700-line limit.

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
pub(super) fn is_clickbait(title: &str) -> bool {
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

pub(super) fn is_security(title: &str, content_start: &str) -> bool {
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

pub(super) fn contains_cve(text: &str) -> bool {
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

pub(super) fn is_breaking_change(title: &str) -> bool {
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

pub(super) fn is_release(title: &str) -> bool {
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

pub(super) fn is_show_and_tell(title: &str) -> bool {
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
pub(super) fn is_generic_help_request(title: &str) -> bool {
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

pub(super) fn is_question(title: &str) -> bool {
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

pub(super) fn is_tutorial(title: &str) -> bool {
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

/// Detect educational/reference repositories — learning resources rather than
/// actionable tools. GitHub trending repos with names like "developer-roadmap",
/// "awesome-rust", "learn-typescript" get Tutorial classification. The experience-
/// level adjustment then determines how much to penalize or boost them.
pub(super) fn is_educational_repo(title: &str) -> bool {
    // Only apply to GitHub-style repo titles (contain star count "★")
    if !title.contains('★') {
        return false;
    }

    let patterns = [
        "roadmap",
        "awesome-",
        "learn-",
        "cheatsheet",
        "cheat-sheet",
        "interview-prep",
        "interview-questions",
        "coding-interview",
        "coding-challenges",
        "free-programming-books",
        "system-design-",
        "design-patterns",
        "freecodecamp",
        "100-days-of-",
        "project-based-learning",
        "build-your-own-",
    ];
    patterns.iter().any(|p| title.contains(p))
}

pub(super) fn is_hiring(title: &str) -> bool {
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
