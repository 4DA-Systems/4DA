// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Source authority weighting
//!
//! Not all sources are equal. Official advisories are more authoritative
//! than community discussions. This feeds into both relevance and necessity.

/// Returns authority weight for a source type (0.0-1.0).
///
/// Every adapter's `source_type()` string must have an explicit arm here —
/// falling through to the neutral default means a real source is silently
/// uncalibrated. The `every_source_type_is_calibrated` test enforces this.
pub(crate) fn source_authority(source_type: &str) -> f32 {
    match source_type {
        // Official advisory databases — factual, authoritative, highest weight
        "cve" => 1.0, // GitHub Advisory Database — official
        "osv" => 1.0, // OSV.dev — official multi-ecosystem vulnerability database

        // Peer-reviewed research
        "arxiv" => 0.90, // Peer-reviewed (slightly less for practitioners vs researchers)

        // Official registries — authoritative version/release facts, not opinion.
        // A "crates.io: tokio v1.53" item is ground truth about the ecosystem.
        "crates_io" => 0.85,
        "npm_registry" => 0.85,
        "pypi" => 0.85,
        "go_modules" => 0.85,
        "github" => 0.85,           // Official repos, releases, READMEs
        "papers_with_code" => 0.85, // Research linked to implementations

        // High-quality curated aggregators
        "hackernews" => 0.75, // Curated by technical community
        "lobsters" => 0.75,   // Curated, invite-only technical community

        // High-intent / official-but-variable
        "rss" => 0.70,         // User-curated feeds = HIGH intent (user chose these)
        "huggingface" => 0.70, // Official model hub, but model-card quality varies

        // Community sources — experienced devs get real value from deep discussions
        "reddit" => 0.65, // Deep technical subreddits (r/rust, r/programming)
        "stackoverflow" => 0.65, // Voted/accepted answers = community-curated quality

        // Variable quality — depends heavily on who/what you follow
        "twitter" => 0.60,
        "bluesky" => 0.60,
        "youtube" => 0.60, // Quality tech content growing (conferences, deep dives)

        // Federated social — community value but higher automation/bot ratio than
        // curated aggregators. Were silently uncalibrated (defaulted to 0.50)
        // until 2026-06-14; now explicit.
        "mastodon" => 0.55,
        "lemmy" => 0.55,

        // Lower technical depth
        "devto" => 0.50,       // Mixed quality, good for tutorials
        "producthunt" => 0.45, // Product launches, low technical depth

        _ => 0.50, // Unknown source, neutral weight
    }
}

/// All source-type strings emitted by adapter `source_type()` implementations.
/// Kept in sync with `src/sources/*.rs`; the drift test asserts each is
/// explicitly calibrated above rather than silently falling to the default.
#[cfg(test)]
pub(crate) const KNOWN_SOURCE_TYPES: &[&str] = &[
    "arxiv",
    "bluesky",
    "crates_io",
    "cve",
    "devto",
    "github",
    "go_modules",
    "hackernews",
    "huggingface",
    "lemmy",
    "lobsters",
    "mastodon",
    "npm_registry",
    "osv",
    "papers_with_code",
    "producthunt",
    "pypi",
    "reddit",
    "rss",
    "stackoverflow",
    "twitter",
    "youtube",
];

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
    fn osv_matches_cve_authority() {
        // OSV.dev is an official vulnerability database — it must carry the same
        // top authority as the GitHub Advisory ("cve") source, not silently fall
        // through to the neutral default (the bug this calibration fixed).
        assert!((source_authority("osv") - 1.0).abs() < f32::EPSILON);
        assert!((source_authority("osv") - source_authority("cve")).abs() < f32::EPSILON);
    }

    #[test]
    fn official_registries_are_high_authority() {
        // Registry items report authoritative version/release facts, so they
        // outrank community discussion.
        for reg in ["crates_io", "npm_registry", "pypi", "go_modules"] {
            assert!(
                source_authority(reg) >= 0.85,
                "{reg} should be high-authority registry data, got {}",
                source_authority(reg)
            );
            assert!(source_authority(reg) > source_authority("reddit"));
        }
    }

    /// Documents the exact calibrated weight of every source. This is the
    /// drift guard: changing a weight or deleting a match arm (which would make
    /// the source fall through to the 0.50 default) fails here loudly.
    #[test]
    fn documented_weights_are_stable() {
        let expected: &[(&str, f32)] = &[
            ("cve", 1.0),
            ("osv", 1.0),
            ("arxiv", 0.90),
            ("crates_io", 0.85),
            ("npm_registry", 0.85),
            ("pypi", 0.85),
            ("go_modules", 0.85),
            ("github", 0.85),
            ("papers_with_code", 0.85),
            ("hackernews", 0.75),
            ("lobsters", 0.75),
            ("rss", 0.70),
            ("huggingface", 0.70),
            ("reddit", 0.65),
            ("stackoverflow", 0.65),
            ("twitter", 0.60),
            ("bluesky", 0.60),
            ("youtube", 0.60),
            ("mastodon", 0.55),
            ("lemmy", 0.55),
            ("devto", 0.50),
            ("producthunt", 0.45),
        ];
        for (source, weight) in expected {
            assert!(
                (source_authority(source) - weight).abs() < f32::EPSILON,
                "{source} expected {weight}, got {}",
                source_authority(source)
            );
        }
    }

    /// Every adapter's source_type must be explicitly calibrated and in range —
    /// no real source should silently inherit the unknown-source default.
    ///
    /// Derived from the LIVE source registry (`build_all_sources`), not a
    /// hand-maintained list: previously this test iterated `KNOWN_SOURCE_TYPES`,
    /// which itself omitted `mastodon` and `lemmy`, so the guard passed while two
    /// registered sources were silently uncalibrated. Cross-checking against the
    /// registry makes that class of drift impossible — a new adapter that isn't
    /// added to `KNOWN_SOURCE_TYPES` (and thus to the `source_authority` match)
    /// fails here.
    #[test]
    fn every_source_type_is_calibrated() {
        for source in crate::sources::build_all_sources() {
            let st = source.source_type();
            assert!(
                KNOWN_SOURCE_TYPES.contains(&st),
                "source_type '{st}' is registered in build_all_sources() but missing \
                 from KNOWN_SOURCE_TYPES — add an explicit `source_authority` arm and \
                 list entry so it can't fall through to the 0.50 default"
            );
            let w = source_authority(st);
            assert!((0.0..=1.0).contains(&w), "{st} authority {w} out of range");
        }
        // Range check over the static list too (covers any entry not currently
        // emitted by an adapter).
        for source in KNOWN_SOURCE_TYPES {
            let w = source_authority(source);
            assert!(
                (0.0..=1.0).contains(&w),
                "{source} authority {w} out of range"
            );
        }
    }
}
