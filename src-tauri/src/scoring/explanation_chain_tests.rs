// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;
use crate::scoring::dependencies::{DepMatch, VersionDelta};

/// Owned fixture data so `ChainInputs` (which borrows) is easy to assemble.
struct Fixture {
    title: String,
    item_topics: Vec<String>,
    ace_ctx: ACEContext,
    interests: Vec<context_engine::Interest>,
    declared_tech: Vec<String>,
    matches: Vec<RelevanceMatch>,
    display_deps: Vec<DepMatch>,
    dep_match_score: f32,
    context_score: f32,
    interest_score: f32,
    keyword_score: f32,
    ace_boost: f32,
    feedback_boost: f32,
    affinity_mult: f32,
    window_boost: f32,
    matched_window_label: Option<String>,
    skill_gap_boost: f32,
    matched_skill_gaps: Vec<String>,
    is_security: bool,
    necessity_score: f32,
    advisory_id: Option<String>,
    cvss_score: Option<f32>,
    cvss_severity: Option<String>,
    fixed_version: Option<String>,
    installed_version: Option<String>,
}

impl Default for Fixture {
    fn default() -> Self {
        Self {
            title: "Some item title".to_string(),
            item_topics: vec![],
            ace_ctx: ACEContext::default(),
            interests: vec![],
            declared_tech: vec![],
            matches: vec![],
            display_deps: vec![],
            dep_match_score: 0.0,
            context_score: 0.0,
            interest_score: 0.0,
            keyword_score: 0.0,
            ace_boost: 0.0,
            feedback_boost: 0.0,
            affinity_mult: 1.0,
            window_boost: 0.0,
            matched_window_label: None,
            skill_gap_boost: 0.0,
            matched_skill_gaps: vec![],
            is_security: false,
            necessity_score: 0.0,
            advisory_id: None,
            cvss_score: None,
            cvss_severity: None,
            fixed_version: None,
            installed_version: None,
        }
    }
}

impl Fixture {
    fn inputs(&self) -> ChainInputs<'_> {
        ChainInputs {
            title: &self.title,
            item_topics: &self.item_topics,
            ace_ctx: &self.ace_ctx,
            interests: &self.interests,
            declared_tech: &self.declared_tech,
            matches: &self.matches,
            display_deps: &self.display_deps,
            dep_match_score: self.dep_match_score,
            context_score: self.context_score,
            interest_score: self.interest_score,
            keyword_score: self.keyword_score,
            ace_boost: self.ace_boost,
            feedback_boost: self.feedback_boost,
            affinity_mult: self.affinity_mult,
            window_boost: self.window_boost,
            matched_window_label: self.matched_window_label.as_deref(),
            skill_gap_boost: self.skill_gap_boost,
            matched_skill_gaps: &self.matched_skill_gaps,
            is_security: self.is_security,
            necessity_score: self.necessity_score,
            advisory_id: self.advisory_id.as_deref(),
            cvss_score: self.cvss_score,
            cvss_severity: self.cvss_severity.as_deref(),
            fixed_version: self.fixed_version.as_deref(),
            installed_version: self.installed_version.as_deref(),
        }
    }

    fn build(&self) -> Vec<ExplanationFactor> {
        build_explanation_chain(&self.inputs())
    }
}

fn dep(name: &str, confidence: f32, is_direct: bool, version: Option<&str>) -> DepMatch {
    DepMatch {
        package_name: name.to_string(),
        confidence,
        version_delta: VersionDelta::Unknown,
        is_dev: false,
        is_direct,
        version: version.map(str::to_string),
        ecosystem: "rust".to_string(),
        corroborated: true,
    }
}

fn interest(topic: &str) -> context_engine::Interest {
    context_engine::Interest {
        id: None,
        topic: topic.to_string(),
        weight: 1.0,
        embedding: None,
        source: Default::default(),
    }
}

const TAIL: &str = "Topic similarity only";

// ============================================================================
// Invariant: every factor names non-empty evidence
// ============================================================================

/// A corpus of crafted inputs spanning every factor kind. Reused by the
/// property-style tests below.
fn corpus() -> Vec<Fixture> {
    let mut out = Vec::new();

    // Dependency-grounded item
    let mut f = Fixture::default();
    f.title = "axios 1.7 fixes SSRF".to_string();
    f.display_deps = vec![dep("axios", 0.8, true, Some("1.6.2"))];
    f.dep_match_score = 0.8;
    out.push(f);

    // Security advisory with full evidence
    let mut f = Fixture::default();
    f.title = "CVE-2026-1234 in tokio".to_string();
    f.is_security = true;
    f.necessity_score = 0.95;
    f.advisory_id = Some("CVE-2026-1234".to_string());
    f.cvss_score = Some(9.8);
    f.fixed_version = Some("1.38.1".to_string());
    f.installed_version = Some("1.37.0".to_string());
    f.display_deps = vec![dep("tokio", 0.9, true, Some("1.37.0"))];
    f.dep_match_score = 0.9;
    out.push(f);

    // Declared-stack item
    let mut f = Fixture::default();
    f.title = "React 19 performance guide".to_string();
    f.item_topics = vec!["react".to_string()];
    f.declared_tech = vec!["react".to_string()];
    f.keyword_score = 0.5;
    out.push(f);

    // Interest-matched item
    let mut f = Fixture::default();
    f.title = "Understanding WebAssembly runtimes".to_string();
    f.item_topics = vec!["webassembly".to_string()];
    f.interests = vec![interest("webassembly")];
    f.interest_score = 0.6;
    out.push(f);

    // Decision-window item
    let mut f = Fixture::default();
    f.title = "Postgres vs SQLite tradeoffs".to_string();
    f.window_boost = 0.15;
    f.matched_window_label = Some("Choose embedded database".to_string());
    out.push(f);

    // Skill-gap item
    let mut f = Fixture::default();
    f.title = "What's new in Kubernetes 1.31".to_string();
    f.skill_gap_boost = 0.18;
    f.matched_skill_gaps = vec!["kubernetes".to_string()];
    out.push(f);

    // KNN project-context item
    let mut f = Fixture::default();
    f.title = "Vector search with sqlite-vec".to_string();
    f.context_score = 0.55;
    f.matches = vec![RelevanceMatch {
        source_file: "src/db/vector.rs".to_string(),
        matched_text: "sqlite-vec KNN queries require k = ? in the WHERE clause".to_string(),
        similarity: 0.71,
    }];
    out.push(f);

    // Semantic-only item (must self-disclose)
    let mut f = Fixture::default();
    f.title = "A story about programming".to_string();
    f.item_topics = vec!["programming".to_string()];
    out.push(f);

    // Learned-preference item
    let mut f = Fixture::default();
    f.title = "Zig comptime deep dive".to_string();
    f.item_topics = vec!["zig".to_string()];
    f.affinity_mult = 1.4;
    f.ace_ctx
        .topic_affinities
        .insert("zig".to_string(), (0.7, 0.8));
    out.push(f);

    out
}

#[test]
fn every_factor_names_nonempty_evidence() {
    for fixture in corpus() {
        for factor in fixture.build() {
            assert!(
                !factor.display.trim().is_empty(),
                "empty display in chain for '{}'",
                fixture.title
            );
            assert!(
                !factor.evidence.trim().is_empty(),
                "factor '{}' has empty evidence ('{}')",
                factor.display,
                fixture.title
            );
        }
    }
}

#[test]
fn chain_never_contains_bare_count_strings() {
    for fixture in corpus() {
        let chain = fixture.build();
        let rendered = render_subtitle(&chain).unwrap_or_default();
        let all_text: String = chain
            .iter()
            .map(|f| format!("{} {}", f.display, f.evidence))
            .collect::<Vec<_>>()
            .join(" ")
            + " "
            + &rendered;
        assert!(
            !all_text.contains("signals confirmed"),
            "banned count-string in chain for '{}': {all_text}",
            fixture.title
        );
        assert!(
            !all_text.contains(" reasons"),
            "banned count-string in chain for '{}': {all_text}",
            fixture.title
        );
    }
}

/// Property: the rendered explanation must share at least one concrete token
/// with the item's actual matched evidence — it can never be a free-floating
/// template.
#[test]
fn rendered_explanation_contains_actual_evidence_token() {
    let checks: Vec<(Fixture, &str)> = vec![
        {
            let mut f = Fixture::default();
            f.display_deps = vec![dep("axios", 0.8, true, None)];
            f.dep_match_score = 0.8;
            (f, "axios")
        },
        {
            let mut f = Fixture::default();
            f.item_topics = vec!["react".to_string()];
            f.declared_tech = vec!["react".to_string()];
            f.keyword_score = 0.5;
            (f, "react")
        },
        {
            let mut f = Fixture::default();
            f.window_boost = 0.15;
            f.matched_window_label = Some("Choose embedded database".to_string());
            (f, "Choose embedded database")
        },
        {
            let mut f = Fixture::default();
            f.skill_gap_boost = 0.18;
            f.matched_skill_gaps = vec!["kubernetes".to_string()];
            (f, "kubernetes")
        },
    ];
    for (fixture, token) in checks {
        let chain = fixture.build();
        let subtitle = render_subtitle(&chain).expect("chain must render a subtitle");
        assert!(
            subtitle.contains(token),
            "subtitle must name the actual evidence '{token}': {subtitle}"
        );
    }
}

// ============================================================================
// Invariant: ordering is monotone with contribution
// ============================================================================

#[test]
fn chain_ordered_by_weight_share_descending() {
    for fixture in corpus() {
        let chain = fixture.build();
        // The honesty tail (weight 0, appended last) is exempt by design.
        let real: Vec<_> = chain
            .iter()
            .filter(|f| !f.display.starts_with(TAIL))
            .collect();
        for pair in real.windows(2) {
            assert!(
                pair[0].weight_share >= pair[1].weight_share,
                "chain out of order for '{}': {} ({}) before {} ({})",
                fixture.title,
                pair[0].display,
                pair[0].weight_share,
                pair[1].display,
                pair[1].weight_share
            );
        }
    }
}

#[test]
fn stronger_contribution_ranks_first() {
    // Dependency evidence (0.9) must outrank a weak interest signal (0.2)...
    let mut f = Fixture::default();
    f.display_deps = vec![dep("tokio", 0.9, true, None)];
    f.dep_match_score = 0.9;
    f.item_topics = vec!["rust".to_string()];
    f.interests = vec![interest("rust")];
    f.interest_score = 0.2;
    let chain = f.build();
    assert_eq!(chain[0].kind, crate::FactorKind::DependencyMatch);

    // ...and the ordering flips when the contributions flip.
    let mut f = Fixture::default();
    f.display_deps = vec![dep("tokio", 0.2, true, None)];
    f.dep_match_score = 0.1;
    f.item_topics = vec!["rust".to_string()];
    f.interests = vec![interest("rust")];
    f.interest_score = 0.9;
    let chain = f.build();
    assert_eq!(
        chain[0].kind,
        crate::FactorKind::InterestMatch,
        "when interest dominates the contribution it must lead the chain"
    );
}

#[test]
fn weight_shares_sum_to_at_most_one() {
    for fixture in corpus() {
        let chain = fixture.build();
        let sum: f32 = chain.iter().map(|f| f.weight_share).sum();
        assert!(
            sum <= 1.0 + 1e-4,
            "weight shares must sum <= 1.0 for '{}', got {sum}",
            fixture.title
        );
    }
}

// ============================================================================
// Honesty tail
// ============================================================================

#[test]
fn semantic_only_item_gets_honesty_tail() {
    let mut f = Fixture::default();
    f.item_topics = vec!["programming".to_string()];
    // Interest match only — no dependency / advisory / context evidence.
    f.interests = vec![interest("programming")];
    f.interest_score = 0.5;
    let chain = f.build();
    let tail = chain.last().expect("chain must not be empty");
    assert!(
        tail.display.starts_with(TAIL),
        "semantic-only chain must end with the honesty tail: {:?}",
        chain.iter().map(|f| &f.display).collect::<Vec<_>>()
    );
    assert!(!tail.evidence.is_empty());
}

#[test]
fn grounded_item_has_no_honesty_tail() {
    let mut f = Fixture::default();
    f.display_deps = vec![dep("axios", 0.8, true, None)];
    f.dep_match_score = 0.8;
    let chain = f.build();
    assert!(
        chain.iter().all(|fac| !fac.display.starts_with(TAIL)),
        "dependency-grounded chain must not carry the honesty tail"
    );
}

#[test]
fn empty_signals_yield_tail_only_chain() {
    let f = Fixture::default();
    let chain = f.build();
    assert_eq!(chain.len(), 1);
    assert!(chain[0].display.starts_with(TAIL));
    // And the subtitle self-discloses rather than inventing a reason.
    assert_eq!(render_subtitle(&chain).unwrap(), chain[0].display);
}

// ============================================================================
// Factor content
// ============================================================================

#[test]
fn dependency_factor_names_provenance_and_version() {
    let mut f = Fixture::default();
    f.display_deps = vec![dep("serde", 0.7, true, Some("1.0.200"))];
    f.dep_match_score = 0.7;
    let chain = f.build();
    let d = &chain[0];
    assert_eq!(d.kind, crate::FactorKind::DependencyMatch);
    assert!(d.display.contains("serde"), "display: {}", d.display);
    assert!(d.evidence.contains("direct"), "evidence: {}", d.evidence);
    assert!(d.evidence.contains("v1.0.200"), "evidence: {}", d.evidence);
}

#[test]
fn security_factor_names_advisory_id_and_fix() {
    let mut f = Fixture::default();
    f.is_security = true;
    f.necessity_score = 0.95;
    f.advisory_id = Some("GHSA-aaaa-bbbb-cccc".to_string());
    f.cvss_score = Some(9.1);
    f.fixed_version = Some("2.0.1".to_string());
    f.display_deps = vec![dep("lodash", 0.8, true, Some("1.9.0"))];
    f.dep_match_score = 0.8;
    let chain = f.build();
    let sec = chain
        .iter()
        .find(|fac| fac.kind == crate::FactorKind::SecurityAdvisory)
        .expect("security factor must be emitted");
    assert!(sec.display.contains("lodash"), "display: {}", sec.display);
    assert!(
        sec.evidence.contains("GHSA-aaaa-bbbb-cccc"),
        "evidence: {}",
        sec.evidence
    );
    assert!(
        sec.evidence.contains("CVSS 9.1"),
        "evidence: {}",
        sec.evidence
    );
    assert!(
        sec.evidence.contains("fixed in v2.0.1"),
        "evidence: {}",
        sec.evidence
    );
}

#[test]
fn security_without_nameable_evidence_is_not_emitted() {
    // is_security but NO advisory id, no severity, no named dep: the factor
    // cannot name evidence, so it must not exist ("Security advisory in your
    // ecosystem" was exactly the un-evidenced template class).
    let mut f = Fixture::default();
    f.is_security = true;
    f.necessity_score = 0.4;
    let chain = f.build();
    assert!(
        chain
            .iter()
            .all(|fac| fac.kind != crate::FactorKind::SecurityAdvisory),
        "un-evidenced security factor must not be emitted"
    );
}

#[test]
fn decision_window_without_label_is_not_emitted() {
    let mut f = Fixture::default();
    f.window_boost = 0.15; // boost but no nameable window
    let chain = f.build();
    assert!(
        chain
            .iter()
            .all(|fac| fac.kind != crate::FactorKind::DecisionWindow),
        "a decision-window factor without a named window is banned (post-#214)"
    );
}

#[test]
fn interest_factor_names_hit_location() {
    let mut f = Fixture::default();
    f.title = "WebAssembly for the backend".to_string();
    f.item_topics = vec!["webassembly".to_string()];
    f.interests = vec![interest("webassembly")];
    f.interest_score = 0.6;
    let chain = f.build();
    let int = chain
        .iter()
        .find(|fac| fac.kind == crate::FactorKind::InterestMatch)
        .expect("interest factor must be emitted");
    assert!(
        int.evidence.contains("in the title"),
        "interest evidence must say where it hit: {}",
        int.evidence
    );
}

#[test]
fn interest_fragment_does_not_mint_factor() {
    // Item topic "http" must not claim interest "tower-http" (no word-boundary
    // or alias relation) — mirrors the v12 corroboration rule.
    let mut f = Fixture::default();
    f.item_topics = vec!["http".to_string()];
    f.interests = vec![interest("tower-http")];
    f.interest_score = 0.6;
    let chain = f.build();
    assert!(
        chain.iter().all(|fac| !fac.display.contains("tower-http")),
        "fragment overlap must not mint an interest factor"
    );
}

#[test]
fn dependency_names_do_not_repeat_in_weaker_tiers() {
    // "react" cited as a dependency must not ALSO appear as declared-stack /
    // interest factors — one name, one (strongest) tier.
    let mut f = Fixture::default();
    f.display_deps = vec![dep("react", 0.8, true, None)];
    f.dep_match_score = 0.8;
    f.item_topics = vec!["react".to_string()];
    f.declared_tech = vec!["react".to_string()];
    f.keyword_score = 0.5;
    f.interests = vec![interest("react")];
    f.interest_score = 0.5;
    let chain = f.build();
    let react_mentions = chain
        .iter()
        .filter(|fac| fac.display.to_lowercase().contains("react"))
        .count();
    assert_eq!(
        react_mentions,
        1,
        "'react' must be cited by exactly one factor: {:?}",
        chain.iter().map(|f| &f.display).collect::<Vec<_>>()
    );
}

#[test]
fn subtitle_is_top_factor_display() {
    let mut f = Fixture::default();
    f.display_deps = vec![dep("axios", 0.9, true, None)];
    f.dep_match_score = 0.9;
    f.item_topics = vec!["javascript".to_string()];
    f.interests = vec![interest("javascript")];
    f.interest_score = 0.3;
    let chain = f.build();
    let subtitle = render_subtitle(&chain).unwrap();
    assert!(
        subtitle.starts_with(&chain[0].display),
        "subtitle must lead with the top factor: {subtitle}"
    );
}
