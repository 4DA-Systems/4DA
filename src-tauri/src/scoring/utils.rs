// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::aliases;

/// Known 1-2 char programming language names that should match despite being short.
/// Without this allowlist, "go", "r", "c", "d" are invisible to topic matching.
const SHORT_LANGUAGE_NAMES: &[&str] = &["go", "r", "c", "d"];

/// Word-boundary-aware topic overlap check for positive signal paths.
/// Prevents false substring matches like "frustrating" containing "rust"
/// or "digital" containing "git". Splits on word boundaries (hyphen, slash,
/// dot, underscore, space) and requires at least one part to match exactly.
///
/// Also checks the technology alias database (130+ groups) so "k8s" overlaps
/// "kubernetes", "ts" overlaps "typescript", etc.
pub(crate) fn topic_overlaps(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }

    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    if a_lower == b_lower {
        return true;
    }

    // Check alias database (covers go↔golang, ts↔typescript, k8s↔kubernetes, etc.)
    if aliases::are_aliases(&a_lower, &b_lower) {
        return true;
    }

    // Short language allowlist: exact match only (aliases already checked above)
    let a_is_short = SHORT_LANGUAGE_NAMES.contains(&a_lower.as_str());
    let b_is_short = SHORT_LANGUAGE_NAMES.contains(&b_lower.as_str());
    if a_is_short && b_is_short {
        return false;
    }

    if a.len() < 3 || b.len() < 3 {
        return false;
    }
    let split_chars = |c: char| c == '-' || c == '/' || c == '.' || c == '_' || c == ' ';
    // Split the LOWERCASED strings — comparing original-case parts made the
    // part-overlap path case-sensitive, so "rust" vs "Rust-Lang" read as no
    // overlap and the on-domain item caught the off-domain penalty (bug D).
    let parts_a: Vec<&str> = a_lower
        .split(split_chars)
        .filter(|p| p.len() >= 3)
        .collect();
    let parts_b: Vec<&str> = b_lower
        .split(split_chars)
        .filter(|p| p.len() >= 3)
        .collect();

    // Check if any part of a matches any part of b exactly
    parts_a
        .iter()
        .any(|pa| parts_b.iter().any(|pb| pa == pb))
        // Also check whole-string against individual parts (lowercased)
        || parts_b.contains(&a_lower.as_str())
        || parts_a.contains(&b_lower.as_str())
}

/// Check if a short term appears as a whole word (bounded by non-alphanumeric chars)
pub(crate) fn has_word_boundary_match(text: &str, term: &str) -> bool {
    for (i, _) in text.match_indices(term) {
        // Use CHAR boundaries, not raw bytes. `as_bytes()[i-1].is_ascii_alphanumeric()`
        // is false for any UTF-8 continuation byte, so a non-ASCII letter glued to the
        // term (e.g. "иgo") was treated as a word boundary and "go" matched (bug E).
        let before_ok = text[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_pos = i + term.len();
        let after_ok = text[after_pos..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_overlaps_exact_match() {
        assert!(topic_overlaps("rust", "rust"));
        assert!(topic_overlaps("typescript", "typescript"));
    }

    #[test]
    fn test_topic_overlaps_hyphenated_parts() {
        // "rust-lang" splits to ["rust", "lang"], "rust" matches "rust"
        assert!(topic_overlaps("rust", "rust-lang"));
        assert!(topic_overlaps("react", "react-native"));
        assert!(topic_overlaps("next.js", "next"));
    }

    #[test]
    fn test_topic_overlaps_rejects_false_substrings() {
        // "frustrating" does NOT contain "rust" as a word-boundary part
        assert!(!topic_overlaps("frustrating", "rust"));
        // "digital" does NOT contain "git" as a word-boundary part
        assert!(!topic_overlaps("digital", "git"));
        // "capital" does NOT contain "api" as a word-boundary part
        assert!(!topic_overlaps("capital", "api"));
        // "developing" does NOT match "dev" (too short, < 3 chars)
        assert!(!topic_overlaps("developing", "dev"));
        // "intelligence" does NOT match "gen"
        assert!(!topic_overlaps("intelligence", "gen"));
    }

    #[test]
    fn test_topic_overlaps_short_strings_rejected() {
        // Strings < 3 chars are rejected UNLESS they're known language names
        assert!(!topic_overlaps("ai", "api"));
        assert!(!topic_overlaps("r", "rust")); // "r" is a known lang, but "rust" is not its alias
    }

    #[test]
    fn test_topic_overlaps_short_language_names() {
        // Known short language names should match exactly and via aliases
        assert!(topic_overlaps("go", "golang")); // alias match
        assert!(topic_overlaps("golang", "go")); // alias match (reverse)
        assert!(topic_overlaps("go", "go")); // exact match
        assert!(topic_overlaps("r", "r")); // exact match
        assert!(topic_overlaps("c", "c")); // exact match
        assert!(topic_overlaps("d", "d")); // exact match

        // Short language names should NOT match unrelated strings
        assert!(!topic_overlaps("go", "good")); // "good" is not "golang"
        assert!(!topic_overlaps("go", "google")); // not an alias
        assert!(!topic_overlaps("c", "css")); // not the same language
        assert!(!topic_overlaps("r", "react")); // not the same
    }

    #[test]
    fn test_topic_overlaps_alias_database() {
        // Tech aliases from the full database (not just hardcoded short languages)
        assert!(topic_overlaps("kubernetes", "k8s"));
        assert!(topic_overlaps("k8s", "kubernetes"));
        assert!(topic_overlaps("typescript", "ts"));
        assert!(topic_overlaps("ts", "typescript"));
        assert!(topic_overlaps("javascript", "js"));
        assert!(topic_overlaps("python", "py"));
        assert!(topic_overlaps("postgresql", "postgres"));
        assert!(topic_overlaps("docker", "containerization"));
        assert!(topic_overlaps("react", "reactjs"));
        assert!(topic_overlaps("graphql", "gql"));
        assert!(topic_overlaps("machine-learning", "ml"));

        // Non-aliases should still be rejected
        assert!(!topic_overlaps("rust", "python"));
        assert!(!topic_overlaps("docker", "kubernetes"));
    }

    #[test]
    fn test_topic_overlaps_case_insensitive_parts() {
        // Bug D regression: the part-overlap path must be case-insensitive.
        // declared_tech/detected_tech are NOT guaranteed lowercase.
        assert!(topic_overlaps("rust", "Rust-Lang"));
        assert!(topic_overlaps("react", "React-Native"));
        assert!(topic_overlaps("next", "Next.js"));
        assert!(topic_overlaps("RUST", "rust-async"));
        // Still must reject genuine non-overlaps regardless of case.
        assert!(!topic_overlaps("frustrating", "Rust"));
        assert!(!topic_overlaps("Digital", "git"));
    }

    #[test]
    fn test_word_boundary_match_unicode() {
        // Bug E regression: a non-ASCII letter glued to the term is NOT a boundary.
        assert!(!has_word_boundary_match("иgo", "go"));
        assert!(!has_word_boundary_match("goи", "go"));
        assert!(!has_word_boundary_match("café2", "2"));
        // Existing ASCII behavior preserved.
        assert!(!has_word_boundary_match("argo", "go"));
        assert!(has_word_boundary_match("go here", "go"));
        assert!(has_word_boundary_match("a go b", "go"));
        assert!(has_word_boundary_match("go", "go"));
    }
}
