/// Word-boundary-aware topic overlap check for positive signal paths.
/// Prevents false substring matches like "frustrating" containing "rust"
/// or "digital" containing "git". Splits on word boundaries (hyphen, slash,
/// dot, underscore, space) and requires at least one part to match exactly.
pub(crate) fn topic_overlaps(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if a.len() < 3 || b.len() < 3 {
        return false;
    }
    let split_chars = |c: char| c == '-' || c == '/' || c == '.' || c == '_' || c == ' ';
    let parts_a: Vec<&str> = a.split(split_chars).filter(|p| p.len() >= 3).collect();
    let parts_b: Vec<&str> = b.split(split_chars).filter(|p| p.len() >= 3).collect();

    // Check if any part of a matches any part of b exactly
    parts_a
        .iter()
        .any(|pa| parts_b.iter().any(|pb| pa == pb))
        // Also check whole-string against individual parts
        || parts_b.contains(&a)
        || parts_a.contains(&b)
}

/// Check if a short term appears as a whole word (bounded by non-alphanumeric chars)
pub(crate) fn has_word_boundary_match(text: &str, term: &str) -> bool {
    for (i, _) in text.match_indices(term) {
        let before_ok = i == 0 || !text.as_bytes()[i - 1].is_ascii_alphanumeric();
        let after_pos = i + term.len();
        let after_ok =
            after_pos >= text.len() || !text.as_bytes()[after_pos].is_ascii_alphanumeric();
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
        // Strings < 3 chars are rejected (too many false positives)
        assert!(!topic_overlaps("ai", "api"));
        assert!(!topic_overlaps("go", "golang"));
        assert!(!topic_overlaps("r", "rust"));
    }
}
