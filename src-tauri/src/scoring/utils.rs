// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::aliases;
use super::dependencies::is_generic_topic_token;

/// Known 1-2 char programming language names that should match despite being short.
/// Without this allowlist, "go", "r", "ts", "py" are invisible to topic matching.
/// Deliberately curated: 2-char FASHION tokens ("ai", "ml") are NOT here — they
/// match everything and ground nothing (`is_generic_topic_token`).
pub(crate) const SHORT_LANGUAGE_NAMES: &[&str] = &["go", "r", "c", "d", "ts", "js", "py"];

/// Strict topic corroboration for grounding-sensitive scoring paths.
///
/// Successor to `topic_overlaps` (deleted, v12), which matched on ANY shared
/// >=3-char fragment — so user dep `tower-http` grounded item topic `http`
/// and `@alpacahq/alpaca-trade-api` grounded `api`, feeding the confirmation
/// axes phantom evidence. Mirror of the dependency axis's ambiguity
/// hard-reject (`is_name_corroborated`): a generic token can never ground,
/// and a fragment overlap counts only when the SHARED fragment is specific
/// (`react` ~ `react-native` grounds; `http` ~ `tower-http` does not).
///
/// Alias-group matches (130+ curated groups: k8s↔kubernetes, next↔next.js)
/// are trusted as-is — curation IS the corroboration. Word-boundary
/// splitting (hyphen, slash, dot, underscore, space) is unchanged from
/// `topic_overlaps`, so "frustrating"/"rust" still cannot match.
pub(crate) fn topic_grounds(a: &str, b: &str) -> bool {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Exact match grounds only when the token itself is specific. A shared
    // "rest"/"http"/"api" token proves nothing about the user's interest.
    // Checked BEFORE the alias database — are_aliases treats identical
    // strings as trivially aliased, which would bypass this gate.
    // (SHORT_LANGUAGE_NAMES are non-generic per is_generic_topic_token, so
    // "go" == "go" still grounds.)
    if a_lower == b_lower {
        return !is_generic_topic_token(&a_lower);
    }

    // Alias database (covers go↔golang, ts↔typescript, k8s↔kubernetes):
    // curated groups let legit short names ground without opening the
    // fragment path — but curation is NOT a genericness waiver. Groups like
    // ["rest","restful","rest-api"] and ["api","application-programming-
    // interface"] would otherwise re-open exactly the generic grounding the
    // exact-match gate above closes, so an alias hit grounds only when BOTH
    // sides are specific. Generic alias pairs fall through to the fragment
    // path, which applies the same gate per fragment.
    if aliases::are_aliases(&a_lower, &b_lower)
        && !is_generic_topic_token(&a_lower)
        && !is_generic_topic_token(&b_lower)
    {
        return true;
    }

    if a_lower.len() < 3 || b_lower.len() < 3 {
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

    // Fragment overlap grounds ONLY on a specific shared fragment
    parts_a
        .iter()
        .any(|pa| parts_b.iter().any(|pb| pa == pb && !is_generic_topic_token(pa)))
        // Whole-string against individual parts, same genericness gate
        || (!is_generic_topic_token(&a_lower) && parts_b.contains(&a_lower.as_str()))
        || (!is_generic_topic_token(&b_lower) && parts_a.contains(&b_lower.as_str()))
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
    fn test_topic_grounds_exact_match() {
        assert!(topic_grounds("rust", "rust"));
        assert!(topic_grounds("typescript", "typescript"));
        assert!(topic_grounds("tauri", "tauri"));
    }

    #[test]
    fn test_topic_grounds_hyphenated_parts() {
        // "rust-lang" splits to ["rust", "lang"], "rust" matches "rust"
        assert!(topic_grounds("rust", "rust-lang"));
        assert!(topic_grounds("react", "react-native"));
        assert!(topic_grounds("tokio", "tokio-util"));
        // "next" is on COMMON_ENGLISH_WORDS — even its curated alias group
        // cannot ground it (F5: alias hits require both sides specific).
        // Users declare "nextjs"/"next.js", which ground fine.
        assert!(!topic_grounds("next.js", "next"));
        assert!(topic_grounds("nextjs", "next.js")); // via alias group
    }

    #[test]
    fn test_topic_grounds_generic_fragments_cannot_ground() {
        // The v12 disease: a shared GENERIC fragment is not corroboration.
        assert!(!topic_grounds("http", "tower-http"));
        assert!(!topic_grounds("api", "@alpacahq/alpaca-trade-api"));
        assert!(!topic_grounds("client", "openai-client"));
        // Generic exact matches can't ground either — mirrors the dep
        // ambiguity hard-reject.
        assert!(!topic_grounds("rest", "rest"));
        assert!(!topic_grounds("http", "http"));
        assert!(!topic_grounds("api", "api"));
        assert!(!topic_grounds("testing", "testing"));
    }

    #[test]
    fn test_topic_grounds_rejects_false_substrings() {
        // "frustrating" does NOT contain "rust" as a word-boundary part
        assert!(!topic_grounds("frustrating", "rust"));
        // "digital" does NOT contain "git" as a word-boundary part
        assert!(!topic_grounds("digital", "git"));
        // "capital" does NOT contain "api" as a word-boundary part
        assert!(!topic_grounds("capital", "api"));
        // "developing" does NOT match "dev" (too short, < 3 chars)
        assert!(!topic_grounds("developing", "dev"));
        // "intelligence" does NOT match "gen"
        assert!(!topic_grounds("intelligence", "gen"));
    }

    #[test]
    fn test_topic_grounds_short_strings_rejected() {
        // Strings < 3 chars are rejected UNLESS they're known language names
        assert!(!topic_grounds("ai", "api"));
        assert!(!topic_grounds("r", "rust")); // "r" is a known lang, but "rust" is not its alias
    }

    #[test]
    fn test_topic_grounds_short_language_names() {
        // Known short language names should match exactly and via aliases
        assert!(topic_grounds("go", "golang")); // alias match
        assert!(topic_grounds("golang", "go")); // alias match (reverse)
        assert!(topic_grounds("go", "go")); // exact match
        assert!(topic_grounds("r", "r")); // exact match
        assert!(topic_grounds("c", "c")); // exact match
        assert!(topic_grounds("d", "d")); // exact match

        // Short language names should NOT match unrelated strings
        assert!(!topic_grounds("go", "good")); // "good" is not "golang"
        assert!(!topic_grounds("go", "google")); // not an alias
        assert!(!topic_grounds("c", "css")); // not the same language
        assert!(!topic_grounds("r", "react")); // not the same
    }

    #[test]
    fn test_topic_grounds_alias_database() {
        // Tech aliases from the full database (not just hardcoded short languages)
        assert!(topic_grounds("kubernetes", "k8s"));
        assert!(topic_grounds("k8s", "kubernetes"));
        assert!(topic_grounds("typescript", "ts"));
        assert!(topic_grounds("ts", "typescript"));
        assert!(topic_grounds("javascript", "js"));
        assert!(topic_grounds("python", "py"));
        assert!(topic_grounds("postgresql", "postgres"));
        assert!(topic_grounds("docker", "containerization"));
        assert!(topic_grounds("react", "reactjs"));
        assert!(topic_grounds("graphql", "gql"));
        // "ml" is a 2-char fashion token, not a language name — its alias
        // group cannot rescue it (F5).
        assert!(!topic_grounds("machine-learning", "ml"));

        // Non-aliases should still be rejected
        assert!(!topic_grounds("rust", "python"));
        assert!(!topic_grounds("docker", "kubernetes"));
    }

    #[test]
    fn test_topic_grounds_alias_bypass_closed_for_generic_sides() {
        // F5: curated alias groups contain generic tokens ("rest"/"restful"/
        // "rest-api", "api"/"application-programming-interface"). An alias
        // hit must not re-open the generic grounding the exact-match gate
        // closes.
        assert!(!topic_grounds("rest", "restful"));
        assert!(!topic_grounds("restful", "rest"));
        assert!(!topic_grounds("rest", "rest-api"));
        assert!(!topic_grounds("api", "application-programming-interface"));
        // Specific alias pairs still ground.
        assert!(topic_grounds("k8s", "kubernetes"));
        assert!(topic_grounds("kubernetes", "k8s"));
    }

    #[test]
    fn test_topic_grounds_short_tech_exact_matches() {
        // F0: is_generic_topic_token no longer inherits the dep-side
        // "len <= 3 is ambiguous" blanket — 3-char tech tokens that aren't
        // English words are specific and exact matches ground.
        assert!(topic_grounds("k8s", "k8s"));
        assert!(topic_grounds("css", "css"));
        assert!(topic_grounds("aws", "aws"));
        assert!(topic_grounds("sql", "sql"));
        // 2-char language names ground (curated allowlist)...
        assert!(topic_grounds("ts", "ts"));
        assert!(topic_grounds("js", "js"));
        assert!(topic_grounds("py", "py"));
        assert!(topic_grounds("go", "go"));
        // ...but 2-char fashion tokens do not.
        assert!(!topic_grounds("ai", "ai"));
        assert!(!topic_grounds("ml", "ml"));
        // And longer specific names keep grounding.
        assert!(topic_grounds("rust", "rust"));
    }

    #[test]
    fn test_topic_grounds_case_insensitive_parts() {
        // Bug D regression: the part-overlap path must be case-insensitive.
        // declared_tech/detected_tech are NOT guaranteed lowercase.
        assert!(topic_grounds("rust", "Rust-Lang"));
        assert!(topic_grounds("react", "React-Native"));
        assert!(topic_grounds("NEXTJS", "Next.js")); // via alias group
        assert!(topic_grounds("RUST", "rust-async"));
        // Still must reject genuine non-overlaps regardless of case.
        assert!(!topic_grounds("frustrating", "Rust"));
        assert!(!topic_grounds("Digital", "git"));
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
