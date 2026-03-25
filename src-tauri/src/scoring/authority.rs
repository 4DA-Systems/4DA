//! Source authority weighting
//!
//! Not all sources are equal. Official advisories are more authoritative
//! than community discussions. This feeds into both relevance and necessity.

/// Returns authority weight for a source type (0.0-1.0)
pub(crate) fn source_authority(source_type: &str) -> f32 {
    match source_type {
        // Official/authoritative sources
        "cve" => 1.0,    // GitHub Advisory Database — official
        "arxiv" => 0.95, // Peer-reviewed papers

        // High-quality aggregators
        "github" => 0.85,     // Official repos, releases, READMEs
        "hackernews" => 0.70, // Curated by technical community
        "lobsters" => 0.70,   // Curated, invite-only technical community

        // Community sources
        "reddit" => 0.55,      // Mixed quality, valuable for sentiment
        "devto" => 0.50,       // Mixed quality, good for tutorials
        "producthunt" => 0.45, // Product launches, low technical depth

        // Variable quality
        "twitter" => 0.60, // Depends heavily on who you follow
        "youtube" => 0.55, // Variable, some excellent technical content
        "rss" => 0.65,     // User-curated, assumed intentional

        _ => 0.50, // Unknown source, neutral weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cve_is_most_authoritative() {
        assert!(source_authority("cve") > source_authority("hackernews"));
        assert!(source_authority("cve") > source_authority("reddit"));
    }

    #[test]
    fn curated_above_open() {
        assert!(source_authority("hackernews") > source_authority("reddit"));
        assert!(source_authority("lobsters") > source_authority("devto"));
    }

    #[test]
    fn unknown_gets_neutral() {
        assert!((source_authority("unknown") - 0.50).abs() < f32::EPSILON);
    }

    #[test]
    fn all_in_range() {
        for source in [
            "cve",
            "arxiv",
            "github",
            "hackernews",
            "lobsters",
            "reddit",
            "devto",
            "producthunt",
            "twitter",
            "youtube",
            "rss",
        ] {
            let w = source_authority(source);
            assert!(w >= 0.0 && w <= 1.0, "{source} authority {w} out of range");
        }
    }
}
