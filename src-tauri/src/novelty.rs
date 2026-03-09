//! Novelty Detection for 4DA
//!
//! Detects whether content is novel (new information) or redundant
//! (introductory/repeated). Penalizes "Getting Started with X" articles
//! for experienced developers and boosts release notes / breaking changes.

/// Novelty assessment result
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields populated for diagnostic/serde use
pub struct NoveltyScore {
    /// Whether this appears to be introductory/tutorial content
    pub is_introductory: bool,
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
) -> NoveltyScore {
    let is_introductory = detect_introductory(title, content);
    let is_release = detect_release(title, content);
    let is_security = detect_security(title, content);

    // Check if the article is about tech the user already knows
    let about_known_tech = topics.iter().any(|t| user_tech.contains(&t.to_lowercase()));

    // Dependency-aware novelty: releases/security about YOUR tech get stronger boosts
    let multiplier = if is_security && about_known_tech {
        // CVE/vulnerability in YOUR dependency = urgent, must-see
        1.30
    } else if is_security {
        // Security news about unrelated tech = informational
        1.10
    } else if is_release && about_known_tech {
        // Release of YOUR dependency = high value
        1.25
    } else if is_release {
        // Release of unrelated tech = mild interest
        1.05
    } else if is_introductory && about_known_tech {
        // "Getting Started with Rust" for a Rust developer = noise
        0.50
    } else if is_introductory {
        // Introductory content about unknown tech = mild penalty
        0.80
    } else {
        1.0
    };

    NoveltyScore {
        is_introductory,
        is_release,
        is_security,
        multiplier,
    }
}

/// Detect introductory/tutorial content from title and content patterns
fn detect_introductory(title: &str, content: &str) -> bool {
    let lower = title.to_lowercase();

    const INTRO_PATTERNS: &[&str] = &[
        "getting started",
        "beginner's guide",
        "beginners guide",
        "introduction to",
        "intro to",
        "what is",
        "learn ",
        "tutorial:",
        "tutorial for",
        "a guide to",
        "complete guide",
        "ultimate guide",
        "how to get started",
        "for beginners",
        "101:",
        " 101",
        "basics of",
        "fundamentals of",
        "from scratch",
        "step by step",
        "your first",
        "hello world",
    ];

    if INTRO_PATTERNS.iter().any(|p| lower.contains(p)) {
        return true;
    }

    // Check content for tutorial patterns (if content is available)
    if !content.is_empty() {
        let content_lower = content.to_lowercase();
        // Strong introductory signals in content body
        let intro_phrases = [
            "in this tutorial",
            "in this beginner",
            "let's start from scratch",
            "first, install",
            "prerequisites:",
        ];
        if intro_phrases
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
    fn test_introductory_detection() {
        assert!(detect_introductory("Getting Started with Rust", ""));
        assert!(detect_introductory("Introduction to Tauri 2.0", ""));
        assert!(detect_introductory(
            "Rust for Beginners: A Complete Guide",
            ""
        ));
        assert!(!detect_introductory(
            "Tokio 1.34: new task scheduling improvements",
            ""
        ));
        assert!(!detect_introductory(
            "Unsafe Rust patterns in production",
            ""
        ));
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

        let result = compute_novelty("Getting Started with Rust", "", &topics, &user_tech);
        assert!(result.is_introductory);
        assert!(!result.is_release);
        assert_eq!(result.multiplier, 0.50); // Stronger penalty: noise for experts
    }

    #[test]
    fn test_novelty_release_known_tech() {
        let user_tech = HashSet::from(["rust".to_string()]);
        let topics = vec!["rust".to_string()];

        let result = compute_novelty("Rust 1.80 Released", "", &topics, &user_tech);
        assert!(result.is_release);
        assert_eq!(result.multiplier, 1.25); // Stronger boost: YOUR dependency
    }

    #[test]
    fn test_novelty_release_unknown_tech() {
        let user_tech = HashSet::from(["rust".to_string()]);
        let topics = vec!["python".to_string()];

        let result = compute_novelty("Python 3.14 Released", "", &topics, &user_tech);
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
        );
        assert!(result.is_security);
        assert_eq!(result.multiplier, 1.10); // Informational: not your stack
    }

    #[test]
    fn test_novelty_regular_content() {
        let user_tech = HashSet::from(["rust".to_string()]);
        let topics = vec!["rust".to_string()];

        let result = compute_novelty("Advanced async patterns in Tokio", "", &topics, &user_tech);
        assert!(!result.is_introductory);
        assert!(!result.is_release);
        assert!(!result.is_security);
        assert_eq!(result.multiplier, 1.0);
    }

    #[test]
    fn test_novelty_intro_unknown_tech() {
        let user_tech = HashSet::from(["rust".to_string()]);
        let topics = vec!["python".to_string()];

        let result = compute_novelty("Getting Started with Python", "", &topics, &user_tech);
        assert!(result.is_introductory);
        assert_eq!(result.multiplier, 0.80); // Mild penalty for unknown tech intro
    }
}
