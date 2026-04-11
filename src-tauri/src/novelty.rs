//! Novelty Detection for 4DA
//!
//! Detects whether content is novel (new information) or redundant
//! (introductory/repeated). Penalizes "Getting Started with X" articles
//! for experienced developers and boosts release notes / breaking changes.

/// Novelty assessment result
#[derive(Debug, Clone)]
#[allow(dead_code)] // Reason: intro_confidence/is_release/is_security fields set but only multiplier read in production
pub struct NoveltyScore {
    /// Graduated introductory confidence (0.0 = definitely not intro, 1.0 = strongly introductory)
    pub intro_confidence: f32,
    /// Whether this appears to be a release/update announcement
    pub is_release: bool,
    /// Whether this is a security advisory (CVE, vulnerability, patch)
    pub is_security: bool,
    /// Final novelty multiplier (0.5 to 1.30)
    pub multiplier: f32,
}

/// Compute novelty score for a source item.
/// Returns a multiplier that adjusts relevance score.
///
/// Key insight: A Rust expert doesn't need "Intro to Rust" articles.
/// Content about technologies you already know should be novel (new release,
/// security advisory, advanced technique) rather than introductory.
pub fn compute_novelty(
    title: &str,
    content: &str,
    topics: &[String],
    user_tech: &std::collections::HashSet<String>,
    user_role: Option<&str>,
    experience_level: Option<&str>,
) -> NoveltyScore {
    let mut intro_confidence = detect_introductory_confidence(title, content);
    let is_release = detect_release(title, content);
    let is_security = detect_security(title, content);

    // Check if the article is about tech the user already knows
    let about_known_tech = topics.iter().any(|t| user_tech.contains(&t.to_lowercase()));

    // ── Experience-aware intro penalty scaling ──
    // Learners benefit from intro content; experienced users don't need it.
    // Applied AFTER intro_confidence computation, BEFORE multiplier calculation.
    if intro_confidence > 0.0 {
        match experience_level {
            Some("learning") => intro_confidence *= 0.30, // Much softer penalty for learners
            Some("building") => intro_confidence *= 0.70, // Moderate reduction
            // "leading", "architecting", None → no change (current behavior)
            _ => {}
        }
    }

    // ── Role-aware security scoring ──
    // Security professionals get the full 1.30x boost for ALL security content,
    // not just content about their known tech stack.
    let is_security_role = matches!(user_role, Some("security"));

    // Dependency-aware novelty: releases/security about YOUR tech get stronger boosts
    let multiplier = if is_security && (about_known_tech || is_security_role) {
        1.30
    } else if is_security {
        1.10
    } else if is_release && about_known_tech {
        1.25
    } else if is_release {
        1.05
    } else if intro_confidence > 0.0 && about_known_tech {
        // Graduated penalty: strong intro (1.0) → 0.50, weak intro (0.3) → 0.85
        1.0 - (intro_confidence * 0.50)
    } else if intro_confidence > 0.0 {
        // Graduated penalty for unknown tech: strong intro (1.0) → 0.80, weak (0.3) → 0.94
        1.0 - (intro_confidence * 0.20)
    } else {
        1.0
    };

    NoveltyScore {
        intro_confidence,
        is_release,
        is_security,
        multiplier,
    }
}

/// Detect introductory/tutorial content with graduated confidence (0.0-1.0).
///
/// Strong patterns ("Getting Started", "for beginners") → 0.90-1.0
/// Moderate patterns ("tutorial", "a guide to") → 0.50-0.70
/// Weak/ambiguous patterns ("complete guide", "ultimate guide") → 0.30-0.50
/// Advanced-term override: presence of sophisticated terms reduces confidence
fn detect_introductory_confidence(title: &str, content: &str) -> f32 {
    let lower = title.to_lowercase();
    let mut confidence: f32 = 0.0;

    // Strong beginner patterns (0.90-1.0)
    let strong_hits = STRONG_INTRO_PATTERNS
        .iter()
        .filter(|p| lower.contains(**p))
        .count();
    if strong_hits > 0 {
        confidence = 0.90 + (strong_hits as f32 * 0.05).min(0.10);
    }

    // Moderate patterns (add 0.50-0.70 if no strong hit)
    if confidence == 0.0 {
        let moderate_hits = MODERATE_INTRO_PATTERNS
            .iter()
            .filter(|p| lower.contains(**p))
            .count();
        if moderate_hits > 0 {
            confidence = 0.50 + (moderate_hits as f32 * 0.10).min(0.20);
        }
    }

    // Weak/ambiguous patterns (add 0.30-0.50 if no stronger hit)
    if confidence == 0.0 {
        let weak_hits = WEAK_INTRO_PATTERNS
            .iter()
            .filter(|p| lower.contains(**p))
            .count();
        if weak_hits > 0 {
            confidence = 0.30 + (weak_hits as f32 * 0.10).min(0.20);
        }
    }

    // Content body patterns can ADD confidence (requires 2+ matches)
    if !content.is_empty() && confidence < 1.0 {
        let content_lower = content.to_lowercase();
        let body_hits = INTRO_BODY_PHRASES
            .iter()
            .filter(|p| content_lower.contains(**p))
            .count();
        if body_hits >= 2 {
            confidence = (confidence + 0.30).min(1.0);
        }
    }

    // Advanced-term override: if title contains sophisticated terms from
    // content_sophistication, REDUCE intro confidence (e.g., "How to Build
    // a Custom Allocator" is NOT a beginner article despite "How to")
    if confidence > 0.0 {
        let advanced_hits = crate::content_sophistication::ADVANCED_TERMS
            .iter()
            .filter(|&&term| lower.contains(term))
            .count();
        if advanced_hits > 0 {
            confidence = (confidence - 0.30 * advanced_hits as f32).max(0.0);
        }
    }

    confidence.clamp(0.0, 1.0)
}

// Strong beginner patterns — high confidence that content is introductory
const STRONG_INTRO_PATTERNS: &[&str] = &[
    "getting started",
    "beginner's guide",
    "beginners guide",
    "for beginners",
    "complete beginner",
    "absolute beginner",
    "hello world",
    "for dummies",
    "101:",
    " 101",
    "your first",
    "how to get started",
    "in 5 minutes",
    "in 10 minutes",
    "in 15 minutes",
    "made easy",
    "simplified",
    "quick start",
    "quickstart",
    "full project breakdown",
];

// Moderate patterns — could be intro OR moderately advanced
const MODERATE_INTRO_PATTERNS: &[&str] = &[
    "introduction to",
    "intro to",
    "tutorial:",
    "tutorial for",
    "learn ",
    "learn how to",
    "from scratch",
    "step by step",
    "step-by-step",
    "basics of",
    "fundamentals of",
    "what is ",
    "crash course",
];

// Weak/ambiguous patterns — may be advanced despite intro-sounding language
const WEAK_INTRO_PATTERNS: &[&str] = &[
    "a guide to",
    "complete guide",
    "ultimate guide",
    "cheat sheet",
];

// Content body phrases that indicate tutorial structure
const INTRO_BODY_PHRASES: &[&str] = &[
    "in this tutorial",
    "in this beginner",
    "let's start from scratch",
    "first, install",
    "prerequisites:",
    "npm install",
    "pip install",
    "let's create a new",
    "open your terminal",
];

/// Detect release notes, version announcements, and update content
fn detect_release(title: &str, content: &str) -> bool {
    let lower = title.to_lowercase();

    const RELEASE_PATTERNS: &[&str] = &[
        "release",
        "released",
        " v1.",
        " v2.",
        " v3.",
        " v4.",
        " v5.",
        " v0.",
        "what's new in",
        "changelog",
        "breaking change",
        "migration guide",
        "upgrade guide",
        "deprecat",
        "end of life",
        "security advisory",
        "security update",
        "patch",
        "hotfix",
        "cve-",
        "vulnerability",
    ];

    if RELEASE_PATTERNS.iter().any(|p| lower.contains(p)) {
        return true;
    }

    // Version number pattern: "1.2.3" or "v1.2.3" in title
    let has_version = lower.split_whitespace().any(|word| {
        let w = word.trim_start_matches('v');
        let parts: Vec<&str> = w.split('.').collect();
        parts.len() >= 2
            && parts.len() <= 3
            && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
            && parts[0].len() <= 3
    });

    if has_version {
        return true;
    }

    // Content-based release detection
    if !content.is_empty() {
        let content_lower = content.to_lowercase();
        let release_signals = [
            "breaking changes",
            "new features",
            "bug fixes",
            "upgrade instructions",
            "migration steps",
        ];
        if release_signals
            .iter()
            .filter(|p| content_lower.contains(*p))
            .count()
            >= 2
        {
            return true;
        }
    }

    false
}

/// Detect security advisories, CVEs, and vulnerability disclosures.
/// These get the strongest novelty boost when about the user's dependencies.
fn detect_security(title: &str, content: &str) -> bool {
    let lower = title.to_lowercase();

    const SECURITY_PATTERNS: &[&str] = &[
        "cve-",
        "security advisory",
        "security update",
        "vulnerability",
        "vulnerabilities",
        "security patch",
        "security fix",
        "supply chain attack",
        "malicious package",
        "backdoor",
        "rce ",
        "remote code execution",
        "privilege escalation",
        "security bulletin",
    ];

    if SECURITY_PATTERNS.iter().any(|p| lower.contains(p)) {
        return true;
    }

    // Content-based security detection
    if !content.is_empty() {
        let content_lower = content.to_lowercase();
        let security_signals = [
            "cve-",
            "cvss score",
            "affected versions",
            "patch immediately",
            "security advisory",
        ];
        if security_signals
            .iter()
            .filter(|p| content_lower.contains(*p))
            .count()
            >= 2
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_introductory_detection_graduated() {
        // Strong intro patterns → high confidence
        let conf = detect_introductory_confidence("Getting Started with Rust", "");
        assert!(conf >= 0.85, "Expected >=0.85, got {conf}");

        let conf = detect_introductory_confidence("Introduction to Tauri 2.0", "");
        assert!(conf >= 0.45, "Expected >=0.45, got {conf}");

        let conf = detect_introductory_confidence("Rust for Beginners: A Complete Guide", "");
        assert!(conf >= 0.85, "Expected >=0.85, got {conf}");

        // Non-intro → zero confidence
        let conf =
            detect_introductory_confidence("Tokio 1.34: new task scheduling improvements", "");
        assert_eq!(conf, 0.0);

        let conf = detect_introductory_confidence("Unsafe Rust patterns in production", "");
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn test_advanced_term_override() {
        // "How to Build a Custom Allocator" contains "allocator" (advanced term)
        // Despite weak intro framing, advanced term should reduce confidence
        let conf =
            detect_introductory_confidence("A Guide to Building a Custom Allocator in Rust", "");
        // "a guide to" = weak (0.30-0.50), but "allocator" reduces by 0.30
        assert!(
            conf < 0.25,
            "Advanced term should reduce intro confidence, got {conf}"
        );
    }

    #[test]
    fn test_release_detection() {
        assert!(detect_release("Rust 1.75 released", ""));
        assert!(detect_release("What's New in React v19.0", ""));
        assert!(detect_release("Tauri v2.0 Migration Guide", ""));
        assert!(detect_release(
            "CVE-2024-1234: Buffer overflow in OpenSSL",
            ""
        ));
        assert!(!detect_release("How to structure Rust projects", ""));
    }

    #[test]
    fn test_security_detection() {
        assert!(detect_security(
            "CVE-2024-1234: Buffer overflow in OpenSSL",
            ""
        ));
        assert!(detect_security(
            "Security Advisory: Critical vulnerability in tokio",
            ""
        ));
        assert!(detect_security(
            "Malicious package found in npm registry",
            ""
        ));
        assert!(!detect_security("How to structure Rust projects", ""));
        assert!(!detect_security("Rust 1.80 Released", ""));
    }

    #[test]
    fn test_novelty_intro_known_tech() {
        let user_tech = HashSet::from(["rust".to_string(), "tauri".to_string()]);
        let topics = vec!["rust".to_string()];

        let result = compute_novelty(
            "Getting Started with Rust",
            "",
            &topics,
            &user_tech,
            None,
            None,
        );
        assert!(result.intro_confidence >= 0.85);
        assert!(!result.is_release);
        // Graduated: strong intro (0.90+) × known tech → ~0.55 (was fixed 0.50)
        assert!(
            result.multiplier < 0.60,
            "Expected <0.60, got {}",
            result.multiplier
        );
    }

    #[test]
    fn test_novelty_release_known_tech() {
        let user_tech = HashSet::from(["rust".to_string()]);
        let topics = vec!["rust".to_string()];

        let result = compute_novelty("Rust 1.80 Released", "", &topics, &user_tech, None, None);
        assert!(result.is_release);
        assert_eq!(result.multiplier, 1.25); // Stronger boost: YOUR dependency
    }

    #[test]
    fn test_novelty_release_unknown_tech() {
        let user_tech = HashSet::from(["rust".to_string()]);
        let topics = vec!["python".to_string()];

        let result = compute_novelty("Python 3.14 Released", "", &topics, &user_tech, None, None);
        assert!(result.is_release);
        assert_eq!(result.multiplier, 1.05); // Mild: informational only
    }

    #[test]
    fn test_novelty_security_known_tech() {
        let user_tech = HashSet::from(["rust".to_string(), "tokio".to_string()]);
        let topics = vec!["tokio".to_string()];

        let result = compute_novelty(
            "CVE-2024-9999: Remote code execution in Tokio",
            "",
            &topics,
            &user_tech,
            None,
            None,
        );
        assert!(result.is_security);
        assert_eq!(result.multiplier, 1.30); // Urgent: YOUR dependency has a CVE
    }

    #[test]
    fn test_novelty_security_unknown_tech() {
        let user_tech = HashSet::from(["rust".to_string()]);
        let topics = vec!["django".to_string()];

        let result = compute_novelty(
            "Security Advisory: Django SQL injection",
            "",
            &topics,
            &user_tech,
            None,
            None,
        );
        assert!(result.is_security);
        assert_eq!(result.multiplier, 1.10); // Informational: not your stack
    }

    #[test]
    fn test_novelty_regular_content() {
        let user_tech = HashSet::from(["rust".to_string()]);
        let topics = vec!["rust".to_string()];

        let result = compute_novelty(
            "Advanced async patterns in Tokio",
            "",
            &topics,
            &user_tech,
            None,
            None,
        );
        assert!(result.intro_confidence == 0.0);
        assert!(!result.is_release);
        assert!(!result.is_security);
        assert_eq!(result.multiplier, 1.0);
    }

    #[test]
    fn test_novelty_intro_unknown_tech() {
        let user_tech = HashSet::from(["rust".to_string()]);
        let topics = vec!["python".to_string()];

        let result = compute_novelty(
            "Getting Started with Python",
            "",
            &topics,
            &user_tech,
            None,
            None,
        );
        assert!(result.intro_confidence >= 0.85);
        // Graduated: strong intro × unknown tech → ~0.82 (was fixed 0.80)
        assert!(
            result.multiplier < 0.85,
            "Expected <0.85, got {}",
            result.multiplier
        );
    }
}
