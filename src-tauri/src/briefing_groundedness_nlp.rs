// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! NLP helpers for briefing groundedness validation.
//!
//! Extracted from `briefing_groundedness.rs` to keep both files under the
//! 700-line Rust warning threshold. Contains term extraction, stopword
//! filtering, and grounding checks.

use std::collections::HashSet;

// ============================================================================
// Term extraction
// ============================================================================

/// Extract salient terms from the synthesized output — things that
/// SHOULD be traceable to a source item if the output is grounded.
///
/// Currently extracts:
/// - Version-like tokens (`1.38`, `v2.0`, `0.9.3`) — invented versions
///   are the most dangerous hallucination class.
/// - Capitalized multi-word phrases (`React Server Components`).
/// - Single capitalized tokens that look like product names
///   (`Stripe`, `Kubernetes`, `Postgres`).
/// - Quoted phrases (`"strict semver"`).
///
/// Skips common English words that happen to be capitalized
/// (sentence-initial words, `I`, pronouns) and very short tokens.
pub(super) fn extract_salient_terms(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // --- Phase 1: version tokens ------------------------------------------
    // Simple hand-written scan rather than regex — the patterns are
    // specific enough that a linear walk is clearest.
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Possible version start — digit, or "v" followed by digit.
        let (start, is_digit_start) = if chars[i].is_ascii_digit() {
            (i, true)
        } else if (chars[i] == 'v' || chars[i] == 'V')
            && i + 1 < chars.len()
            && chars[i + 1].is_ascii_digit()
        {
            (i, false)
        } else {
            i += 1;
            continue;
        };

        let mut end = start + usize::from(!is_digit_start);
        let mut saw_dot = false;
        while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
            if chars[end] == '.' {
                saw_dot = true;
            }
            end += 1;
        }

        if saw_dot && end > start + 2 {
            // Trim trailing dots so "5.6." from "to 5.6." becomes "5.6".
            let mut stop = end;
            while stop > start && chars[stop - 1] == '.' {
                stop -= 1;
            }
            let token: String = chars[start..stop].iter().collect();
            // Must still contain a dot after trimming (e.g., "5." alone is junk).
            if token.contains('.') && !looks_like_date(&token) {
                let key = token.to_lowercase();
                if !seen.contains(&key) {
                    seen.insert(key);
                    out.push(token);
                }
            }
        }
        i = end.max(i + 1);
    }

    // --- Phase 2: capitalized noun-phrase run ------------------------------
    // "React Server Components", "Rust Async Book", etc.
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut cap_run: Vec<&str> = Vec::new();
    let flush = |cap_run: &mut Vec<&str>, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        if cap_run.len() >= 2 {
            let joined = cap_run.join(" ");
            // Strip trailing punctuation from the phrase
            let cleaned = joined.trim_end_matches(|c: char| {
                matches!(c, '.' | ',' | ';' | ':' | '—' | '-' | '!' | '?')
            });
            let key = cleaned.to_lowercase();
            if !cleaned.is_empty() && !seen.contains(&key) {
                seen.insert(key);
                out.push(cleaned.to_string());
            }
        }
        cap_run.clear();
    };
    for token in &tokens {
        let stripped = token.trim_matches(|c: char| !c.is_alphanumeric());
        // Match on the first character via `chars().next()` rather than
        // unwrap-after-empty-check so clippy's unwrap_used lint is happy
        // without sacrificing readability.
        match stripped.chars().next() {
            None => {
                flush(&mut cap_run, &mut out, &mut seen);
            }
            Some(first) => {
                if first.is_uppercase() && stripped.len() > 1 && !is_stopword(stripped) {
                    cap_run.push(stripped);
                } else {
                    flush(&mut cap_run, &mut out, &mut seen);
                }
            }
        }
    }
    flush(&mut cap_run, &mut out, &mut seen);

    // --- Phase 3: notable single-capitalized tokens ------------------------
    // Re-enabled with stopword filter (2026-04-25). Previously disabled
    // because "Reactive", "Security", etc. caused false rejections, but
    // the stopword list now covers 200+ common English words. This phase
    // catches hallucinated proper nouns like "Stripe" or "Postgres" that
    // don't appear in any source item — critical for the specificity
    // floor (is_acceptable rejects output with <2 salient terms).
    for token in &tokens {
        let stripped = token.trim_matches(|c: char| !c.is_alphanumeric());
        if let Some(first) = stripped.chars().next() {
            if first.is_uppercase()
                && stripped.len() >= 3
                && !is_stopword(stripped)
                && stripped.chars().filter(|c| c.is_alphabetic()).count() >= 3
            {
                let key = stripped.to_lowercase();
                if !seen.contains(&key) {
                    seen.insert(key);
                    out.push(stripped.to_string());
                }
            }
        }
    }

    out
}

fn looks_like_date(token: &str) -> bool {
    let dots = token.chars().filter(|&c| c == '.').count();
    if dots != 2 {
        return false;
    }
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    if let Some(year) = parts.first().and_then(|s| s.parse::<u32>().ok()) {
        return (1990..=2100).contains(&year);
    }
    false
}

/// Common English words that are often capitalized for sentence structure
/// but should not be treated as proper-noun terms.
fn is_stopword(word: &str) -> bool {
    const STOPWORDS: &[&str] = &[
        "The",
        "This",
        "That",
        "These",
        "Those",
        "There",
        "Here",
        "Now",
        "Today",
        "Tomorrow",
        "Yesterday",
        "Morning",
        "Evening",
        "Night",
        "Your",
        "Our",
        "My",
        "Their",
        "His",
        "Her",
        "Its",
        "When",
        "Where",
        "What",
        "Why",
        "How",
        "Who",
        "Which",
        "While",
        "With",
        "Without",
        "After",
        "Before",
        "Since",
        "Until",
        "Between",
        "Among",
        "Across",
        "During",
        "About",
        "Against",
        "Through",
        "Priority",
        "Pattern",
        "Situation",
        "Summary",
        "Briefing",
        "Review",
        "Update",
        "Consider",
        "Watch",
        "Alert",
        "Warning",
        "Critical",
        "High",
        "Medium",
        "Low",
        "Top",
        "Key",
        "If",
        "But",
        "And",
        "Or",
        "Nor",
        "So",
        "Yet",
        "For",
        "Because",
        "Although",
        "Unless",
        "Whereas",
        "Meanwhile",
        "However",
        "Therefore",
        "Furthermore",
        "Additionally",
        "Moreover",
        "Nevertheless",
        "Nonetheless",
        "Consequently",
        "Subsequently",
        "Accordingly",
        "Overall",
        "Notably",
        "Specifically",
        "Importantly",
        "Essentially",
        "Particularly",
        "Ultimately",
        "Interestingly",
        "Several",
        "Various",
        "Multiple",
        "Significant",
        "Large",
        "Small",
        "Major",
        "Minor",
        "Recent",
        "Current",
        "Previous",
        "First",
        "Second",
        "Third",
        "Final",
        "Early",
        "Late",
        "Next",
        "Last",
        "Other",
        "Another",
        "Every",
        "Each",
        "Both",
        "Many",
        "Most",
        "Some",
        "Any",
        "All",
        "Such",
        "Same",
        "New",
        "Old",
        "Modern",
        "Legacy",
        "Popular",
        "Common",
        "General",
        "Specific",
        "Special",
        "Potential",
        "Possible",
        "Available",
        "Existing",
        "Different",
        "Similar",
        "Certain",
        "Related",
        "Based",
        "Given",
        "Note",
        "Keep",
        "Make",
        "Take",
        "Use",
        "Using",
        "Action",
        "Impact",
        "Focus",
        "Language",
        "Models",
        "Model",
        "System",
        "Systems",
        "Paper",
        "Papers",
        "Research",
        "Study",
        "Approach",
        "Method",
        "Results",
        "Analysis",
        // Synthesis vocabulary: words the LLM uses when clustering/summarizing
        "Cluster",
        "Clusters",
        "Signal",
        "Signals",
        "Source",
        "Sources",
        "Theme",
        "Themes",
        "Thread",
        "Threads",
        "Trend",
        "Trends",
        "Publish",
        "Published",
        "Publishing",
        "Hit",
        "Mass",
        "Three",
        "Two",
        "Four",
        "Five",
        "Six",
        "Seven",
        "Eight",
        "Nine",
        "Ten",
        "Dozen",
        "Half",
        "Zero",
        "Fast",
        "Slow",
        "Quick",
        "Rapid",
        "Growing",
        "Rising",
        "Emerging",
        "Maturing",
        "Accelerating",
        "Increasing",
        "Gaining",
        "Traction",
        "Momentum",
        "Security",
        "Secure",
        "Supply",
        "Chain",
        "Audit",
        "Auditing",
        "Hardening",
        "Compiler",
        "Optimizations",
        "Optimization",
        "Performance",
        "Improvements",
        "Improvement",
        "Build",
        "Builds",
        "Config",
        "Configuration",
        "Changes",
        "Change",
        "Teams",
        "Team",
        "Guides",
        "Guide",
        "Week",
        "Weeks",
        "Month",
        "Months",
        "Year",
        "Years",
        "Separately",
        "Worth",
        "Evaluating",
        "Evaluate",
        "Independent",
        "Independently",
        "Testing",
        "Test",
        "Tests",
        "Tooling",
        "Tool",
        "Tools",
        "Framework",
        "Frameworks",
        "Library",
        "Libraries",
        "Package",
        "Packages",
        "Version",
        "Versions",
        "Pinned",
        "Release",
        "Releases",
        "Breaking",
        "Feature",
        "Features",
        "Area",
        "Areas",
        "Demand",
        "Attention",
        "Immediate",
        "Nothing",
        "Scatter",
        "Unrelated",
        "Enough",
        "Act",
        "Check",
        "Look",
        "Approaches",
        "Development",
        "Developer",
        "Developers",
        "Engineering",
        "Production",
        "Deployment",
        "Pipeline",
        "Pipelines",
        "Workflow",
        "Workflows",
        "Integration",
        "Continuous",
        "Practices",
        "Practice",
        "Patterns",
        "Code",
        "Codebase",
        "Application",
        "Applications",
        "Project",
        "Projects",
        "Open",
        "Weights",
        "Repository",
        "Repositories",
        "Repo",
        "Repos",
        // Common adjectives the LLM uses in synthesis to cluster/paraphrase
        "Upcoming",
        "Improved",
        "Enhanced",
        "Expanded",
        "Extended",
        "Strengthened",
        "Updated",
        "Advancing",
        "Advanced",
        "Confirmed",
        "Affected",
        "Impacted",
        "Core",
        "Essential",
        "Fundamental",
        "Direct",
        "Indirect",
        "Active",
        "Full",
        "Partial",
        "Complete",
        "Stable",
        "Experimental",
        "Native",
        "Standard",
        "Relevant",
        "Valuable",
        "Notable",
        "Noteworthy",
        "Recommended",
        "Functional",
        // Source platform names — the LLM references these as provenance,
        // not as factual claims about content
        "Reddit",
        "GitHub",
        "Hacker",
        "Lobsters",
        "Bluesky",
        "Twitter",
        "YouTube",
        "StackOverflow",
        "Stack",
        "Overflow",
        "Crates",
        "PyPI",
        "HuggingFace",
        "ArXiv",
        "Rust",
        "Python",
        "JavaScript",
        "TypeScript",
        "Java",
        "Golang",
        "Swift",
        "Kotlin",
        "Ruby",
        "React",
        "Node",
        "Deno",
        "Bun",
        "Tauri",
        "Electron",
        "Docker",
        "Kubernetes",
        "Linux",
        "Windows",
        "MacOS",
        "Wasm",
        "WebAssembly",
        // Common tech acronyms/abbreviations the LLM generates
        "REST",
        "API",
        "APIs",
        "CLI",
        "SDK",
        "OSS",
        "LLM",
        "LLMs",
        "RSC",
        "SSR",
        "SSG",
        "ORM",
        "CSS",
        "HTML",
        "JSON",
        "YAML",
        "TOML",
        "SQL",
        "GPU",
        "CPU",
        "IDE",
        "DNS",
        "TLS",
        "SSL",
        "JWT",
        // Additional common verbs/adjectives at sentence start
        "Hold",
        "Track",
        "Tracking",
        "Shift",
        "Shifting",
        "Expect",
        "Expected",
        "Likely",
        "Unlikely",
        "Announce",
        "Announced",
        "Announcing",
        "Introduce",
        "Introduced",
        "Introducing",
        "Support",
        "Supporting",
        "Supported",
        "Supports",
        "Adopt",
        "Adopted",
        "Adoption",
        "Drop",
        "Dropped",
        "Dropping",
        "Move",
        "Moving",
        "Moved",
        "Bring",
        "Bringing",
        "Add",
        "Added",
        "Adding",
        "Explore",
        "Exploring",
        "Explored",
        "Propose",
        "Proposed",
        "Proposing",
        "Land",
        "Landed",
        "Landing",
        "Ship",
        "Shipped",
        "Shipping",
        "Discuss",
        "Discussion",
        "Discussions",
        "Debate",
        "Controversy",
        "Community",
        "Ecosystem",
        "Landscape",
        "Space",
        "Industry",
        "World",
        "Mainstream",
        "Enterprise",
        "Startup",
        "Startups",
        "Working",
        "Group",
        "Spec",
        "Specification",
        "Draft",
        "Proposal",
        "Standards",
        "Compliance",
        "Compatible",
        "Compatibility",
        // Common verbs that get capitalized at sentence starts
        "Recommend",
        "Recommended",
        "Upgrade",
        "Downgrade",
        "Install",
        "Installed",
        "Configure",
        "Configured",
        "Migrate",
        "Migrating",
        "Migration",
        "Deploy",
        "Deployed",
        "Implement",
        "Implemented",
        "Evaluated",
        "Run",
        "Running",
        "Start",
        "Started",
        "Stop",
        "Stopped",
        "Enable",
        "Enabled",
        "Disable",
        "Disabled",
        "Include",
        "Including",
        "Require",
        "Required",
        "Requires",
        "Suggest",
        "Suggests",
        "Suggested",
        "Indicate",
        "Indicates",
        "Continue",
        "Continues",
        "Continued",
        "Prioritize",
        "Prevent",
        "Avoid",
        "Reduce",
        "Improve",
        "Address",
        "Ensure",
        "Verify",
        "Validate",
        "Scan",
        "Benchmark",
        "Monitor",
        "Switch",
        "Replace",
        "Remove",
        "Delete",
        "Fix",
        "Patch",
        "Pin",
        "Lock",
        "You",
        "Also",
        "Still",
        "Already",
        "Even",
        "Just",
        "Only",
        "Never",
        "Always",
        "Often",
        "Usually",
        "Really",
        "Quite",
        "Nearly",
        "Almost",
        "Rather",
        "Simply",
        "Clearly",
        "Directly",
        "Immediately",
        "Especially",
        "Exactly",
        "Heavily",
        "Strongly",
        "Properly",
        "Effectively",
        "Carefully",
        "Potentially",
        "Silent",
        "Quiet",
        "Above",
        "Below",
        "Beyond",
        "Within",
        "One",
        "Into",
        "Onto",
        "Over",
        "Under",
        "Down",
        "Then",
        "Than",
        "Very",
        "More",
        "Less",
        "Much",
        "Well",
        "Best",
        "Better",
        "Worse",
        "Worst",
        "Least",
        "Further",
        "Itself",
        "Themselves",
        "Yourself",
        "Itself",
    ];
    STOPWORDS.iter().any(|&s| s.eq_ignore_ascii_case(word))
}

// ============================================================================
// Grounding check
// ============================================================================

/// A term is grounded if it appears as a substring in any corpus entry,
/// case-insensitively. For multi-word phrases we also accept partial
/// matches where ALL component words appear (in any order) in a single
/// corpus entry — this tolerates the LLM reordering "React Server
/// Components" as "Server Components in React".
pub(super) fn is_term_grounded(term: &str, corpus_lower: &[String]) -> bool {
    let term_lower = term.to_lowercase();

    // Exact substring match on any corpus entry
    if corpus_lower.iter().any(|c| c.contains(&term_lower)) {
        return true;
    }

    // Multi-word: check if every component word appears in a single entry
    let words: Vec<&str> = term_lower.split_whitespace().collect();
    if words.len() >= 2 {
        for entry in corpus_lower {
            if words.iter().all(|w| entry.contains(*w)) {
                return true;
            }
        }

        // Partial grounding: if any 2+ word subsequence appears in the corpus,
        // the phrase is partially grounded (e.g., "Vulnerability Alerts GitHub
        // Actions" is grounded if "GitHub Actions" appears in any entry).
        for window_size in (2..words.len()).rev() {
            for window in words.windows(window_size) {
                let sub = window.join(" ");
                if corpus_lower.iter().any(|c| c.contains(&sub)) {
                    return true;
                }
            }
        }
    }

    false
}
