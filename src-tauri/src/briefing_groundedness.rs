// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Post-synthesis groundedness validator.
//!
//! The morning-briefing LLM sometimes generates claims that are not
//! supported by any input item. A real production example observed in
//! `Screenshot_1976`:
//!
//! > Recommend update of your strategy for non-test architecture,
//! > including a 5+ year migration from VAR and Stripe
//!
//! Neither "VAR" nor "Stripe" appeared in any source item that day, and
//! "non-test architecture" is not a real concept. This is a fabricated
//! recommendation — the prompt asked for actionable advice so the model
//! produced one, even though nothing in the input warranted it.
//!
//! This validator runs AFTER synthesis and BEFORE the output reaches
//! the user. It extracts noun phrases, version numbers, and product
//! names from the synthesized text and checks each one against the
//! input item corpus (titles + descriptions + matched_deps). Items
//! that fail the grounding check are reported; if the overall
//! groundedness score falls below a threshold, the synthesis is
//! rejected and a safe fallback is used instead.
//!
//! This is a first-line defense, not a panacea. A determined
//! adversarial LLM could still slip hallucinations past a heuristic
//! validator. But the common failure mode — the LLM inventing
//! plausible-sounding tech names to fill a required response slot
//! — this gate catches cleanly.

#[path = "briefing_groundedness_nlp/mod.rs"]
mod nlp;
use nlp::*;

// ============================================================================
// Public API
// ============================================================================

/// Confidence that every substantive claim in `output` is grounded in
/// `corpus`. Values above `0.8` indicate high confidence; below `0.5`
/// indicate the output should probably be rejected.
#[derive(Debug, Clone)]
pub struct GroundednessReport {
    pub confidence: f32,
    pub total_terms: usize,
    /// Count of salient terms matched against the source corpus. Surfaced
    /// to the receipts UI as the numerator of the grounding fraction.
    // REMOVE BY 2026-08-01
    #[allow(dead_code)] // Serde: populated during grounding analysis
    pub grounded_terms: usize,
    pub ungrounded_terms: Vec<String>,
    /// Multi-word capitalized proper-noun phrases (e.g. "Stripe Connect",
    /// "React Server Components") — the interpretive "claim" layer, kept
    /// separate from factual package/version terms. See `claim_grounded`.
    pub claim_terms: usize,
    /// How many of `claim_terms` were grounded in the corpus.
    pub claim_grounded: usize,
}

impl GroundednessReport {
    /// Claim-layer grounding ratio: of the multi-word proper-noun phrases,
    /// what fraction are grounded? 1.0 when there are none (nothing to doubt).
    pub fn claim_confidence(&self) -> f32 {
        if self.claim_terms == 0 {
            1.0
        } else {
            self.claim_grounded as f32 / self.claim_terms as f32
        }
    }

    /// Is this output safe to show the user at the given threshold?
    pub fn is_acceptable(&self, threshold: f32) -> bool {
        // A good synthesis MUST reference something specific from the
        // signals — at least 2 salient terms (a technology name, a
        // version, a project). Fewer than 2 means the output is too
        // generic (e.g. "Prioritize configuring your tech stack") and
        // isn't grounded in any actual signal content.
        //
        // The two checks are distinct concerns and `confidence` is now
        // reported honestly for the empty case (0.0, not 1.0) so the
        // specificity floor is the sole gate when nothing groundable was
        // extracted — no more "confidence=1.0 total_terms=0 -> reject"
        // contradiction in the logs.
        if self.total_terms < 2 {
            return false;
        }
        self.confidence >= threshold
    }
}

// NOTE on the claim layer (`claim_terms` / `claim_grounded` / `claim_confidence`):
// these are reported and LOGGED, not gated. A hard "claim phrases must be
// grounded" gate is unsafe — realistic prose like "Upgrade Tokio today" forms a
// capitalized run "Upgrade Tokio" whose verb is never in the corpus, so gating
// on it would false-reject good briefs and re-introduce the abstention bug this
// change set fixed. The active accuracy guards are: the prompt's project/scope
// rules (no invented use-cases, no cross-project compounding), the deterministic
// `check_factual_claims` version verifier, and the overall `confidence` floor.
// Claim telemetry exists so we can SEE proper-noun drift over time and tighten
// later from data rather than guesswork.

/// Validate that the synthesized briefing text is grounded in the
/// provided corpus. The corpus should contain every source item that
/// was fed to the LLM: concatenate title + description + matched_deps
/// per item.
///
/// 2-arg shim (no package allowlist) used by the grounding test corpus.
/// Production paths always have the brief's packages and call
/// [`validate_groundedness_with_packages`], so this is test-only.
#[cfg(test)]
pub fn validate_groundedness(output: &str, corpus: &[String]) -> GroundednessReport {
    validate_groundedness_with_packages(output, corpus, &[])
}

/// Like [`validate_groundedness`], but with an explicit allowlist of the
/// brief's known package / dependency names (e.g. "axios", "jsonwebtoken",
/// "@clerk/clerk-react"). Bare lowercase package names are invisible to the
/// capitalized salient-term extractor, so a dependency-security brief would
/// otherwise extract ~0 terms and be wrongly rejected by the specificity
/// floor. Matching against the brief's actual packages recognizes them with
/// zero risk of counting ordinary lowercase English words.
pub fn validate_groundedness_with_packages(
    output: &str,
    corpus: &[String],
    packages: &[String],
) -> GroundednessReport {
    let pkg_set: std::collections::HashSet<String> =
        packages.iter().map(|p| p.to_lowercase()).collect();

    let mut corpus_lower: Vec<String> = corpus.iter().map(|s| s.to_lowercase()).collect();
    // Known packages are grounded by construction — they are the brief's
    // real dependencies — so ensure the grounding check can always match them.
    corpus_lower.extend(pkg_set.iter().cloned());

    let mut terms = extract_salient_terms(output);

    // Augment with bare/hyphenated package names that the shape-based extractor
    // cannot tell apart from English (axios, jsonwebtoken, clerk-react) by
    // matching output tokens against the brief's actual dependency set.
    if !pkg_set.is_empty() {
        let mut seen: std::collections::HashSet<String> =
            terms.iter().map(|t| t.to_lowercase()).collect();
        for raw in output.split_whitespace() {
            let tok = raw
                .trim_matches(|c: char| {
                    !c.is_alphanumeric() && !matches!(c, '@' | '/' | '-' | '_' | '.')
                })
                .trim_matches(|c: char| matches!(c, '.' | '-' | '_'));
            let key = tok.to_lowercase();
            if !key.is_empty() && pkg_set.contains(&key) && !seen.contains(&key) {
                seen.insert(key);
                terms.push(tok.to_string());
            }
        }
    }

    let total = terms.len();

    let mut ungrounded = Vec::new();
    let mut grounded = 0;
    let mut claim_terms = 0;
    let mut claim_grounded = 0;

    for term in &terms {
        let is_grounded = is_term_grounded(term, &corpus_lower);
        if is_grounded {
            grounded += 1;
        } else {
            ungrounded.push(term.clone());
        }
        // Claim layer = multi-word capitalized proper-noun phrases that are
        // NOT factual package/version tokens. These are the interpretive names
        // ("Stripe Connect", "Apache Kafka") most prone to hallucination.
        if is_claim_term(term, &pkg_set) {
            claim_terms += 1;
            if is_grounded {
                claim_grounded += 1;
            }
        }
    }

    // No groundable terms means we have no evidence either way — report 0.0,
    // not 1.0. The specificity floor in `is_acceptable` rejects on `total < 2`
    // regardless, but an honest score keeps the logs truthful.
    let confidence = if total == 0 {
        0.0
    } else {
        grounded as f32 / total as f32
    };

    GroundednessReport {
        confidence,
        total_terms: total,
        grounded_terms: grounded,
        ungrounded_terms: ungrounded,
        claim_terms,
        claim_grounded,
    }
}

/// A factual term is one grounded by construction or by shape: a known package
/// name, a version-like token (digits with a dot), or a scoped npm name. These
/// are NOT claims — they are the verifiable spine of the synthesis.
fn is_factual_term(term: &str, pkg_set: &std::collections::HashSet<String>) -> bool {
    let lower = term.to_lowercase();
    if pkg_set.contains(&lower) {
        return true;
    }
    if term.starts_with('@') && term.contains('/') {
        return true;
    }
    // Version-like: contains a dot and at least one digit (e.g. "1.16.0").
    term.contains('.') && term.chars().any(|c| c.is_ascii_digit())
}

/// A claim term is a multi-word capitalized proper-noun phrase that is not a
/// factual token. Single-word capitals are excluded on purpose: sentence-initial
/// words ("Upgrade", "Then", "Audit") are noise, and gating on them would
/// re-introduce the false-abstention bug.
fn is_claim_term(term: &str, pkg_set: &std::collections::HashSet<String>) -> bool {
    term.contains(' ') && !is_factual_term(term, pkg_set)
}

// ============================================================================
// Factual claim verification (deterministic — the "accurate first" backstop)
// ============================================================================

/// A package and the set of versions it is legitimate to cite for it: its
/// currently-installed version and its advisory's fixed version. Built by the
/// caller from the brief's security alerts.
#[derive(Debug, Clone)]
pub struct PackageFact {
    pub name: String,
    pub versions: Vec<String>,
}

/// Deterministically verify the version numbers a synthesis states for known
/// packages. The heuristic groundedness check does fuzzy term matching; this is
/// the exact backstop: if the model says "upgrade axios to 1.99" but the only
/// versions on record for axios are 1.12.2 (installed) and 1.16.0 (fix), that is
/// a fabricated version a user could act on — and we must not ship it.
///
/// Precision over recall: a violation is reported ONLY when a version token sits
/// in the span that clearly belongs to a named package (from that package's name
/// up to the next known package name) and matches none of its allowed versions
/// (prefix-tolerant, so "1.16" matches "1.16.0"). Versions not associated with
/// any package are ignored, so legitimate versions from other signals never
/// trip it.
pub fn check_factual_claims(prose: &str, facts: &[PackageFact]) -> Vec<String> {
    if facts.is_empty() {
        return Vec::new();
    }
    let tokens: Vec<&str> = prose.split_whitespace().collect();
    // Normalize a prose token to a bare identifier for package matching.
    let norm = |t: &str| -> String {
        t.trim_matches(|c: char| !c.is_alphanumeric() && !matches!(c, '@' | '/' | '-' | '_' | '.'))
            .trim_matches(|c: char| matches!(c, '.' | '-' | '_'))
            .to_lowercase()
    };
    // For each package, the bare tokens that should count as "naming" it.
    let alias = |name: &str| -> Vec<String> {
        let l = name.to_lowercase();
        let mut v = vec![l.clone()];
        // scoped "@scope/pkg" -> also match the bare "pkg"
        if let Some(idx) = l.rfind('/') {
            v.push(l[idx + 1..].to_string());
        }
        v
    };

    // Index each token position to a package (if it names one).
    let pkg_at: Vec<Option<usize>> = tokens
        .iter()
        .map(|t| {
            let n = norm(t);
            facts.iter().position(|f| alias(&f.name).contains(&n))
        })
        .collect();

    let mut violations = Vec::new();
    for (i, fi) in pkg_at.iter().enumerate() {
        let Some(fact_idx) = *fi else { continue };
        let fact = &facts[fact_idx];
        // Scan forward within the SAME sentence, stopping at the next package
        // mention, for version tokens to attribute to this package. Bounding to
        // the sentence prevents a version in a later sentence from being
        // misattributed (e.g. "Upgrade axios to 1.16.0. React 19.2 is new.").
        for (offset, raw) in tokens.iter().skip(i + 1).enumerate() {
            let pos = i + 1 + offset;
            if pos < pkg_at.len() && pkg_at[pos].is_some() {
                break; // reached the next package — stop attributing versions here
            }
            let v = raw
                .trim_matches(|c: char| !c.is_ascii_digit() && c != '.')
                .trim_matches('.');
            if looks_like_version_token(v)
                && !fact
                    .versions
                    .iter()
                    .any(|allowed| version_matches(v, allowed))
            {
                violations.push(format!(
                    "{} cited version {} (on record: {})",
                    fact.name,
                    v,
                    fact.versions.join(", ")
                ));
            }
            // Stop at a sentence boundary (this token ends with . ! or ?).
            if raw.ends_with(|c: char| matches!(c, '.' | '!' | '?')) {
                break;
            }
        }
    }
    violations
}

/// A token like "1.16", "10.3.0", "1.12.2" — at least one dot between digits.
fn looks_like_version_token(t: &str) -> bool {
    let bytes: Vec<char> = t.chars().collect();
    if !t.contains('.') {
        return false;
    }
    if !t.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return false;
    }
    bytes.first().is_some_and(|c| c.is_ascii_digit())
        && bytes.last().is_some_and(|c| c.is_ascii_digit())
}

/// Prefix-tolerant version equality at dot boundaries: "1.16" matches "1.16.0"
/// and "1.16.2", but "1.16" does not match "1.1" or "1.160".
fn version_matches(stated: &str, allowed: &str) -> bool {
    if stated == allowed {
        return true;
    }
    let s: Vec<&str> = stated.split('.').collect();
    let a: Vec<&str> = allowed.split('.').collect();
    let n = s.len().min(a.len());
    n >= 1 && s[..n] == a[..n]
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(titles: &[&str]) -> Vec<String> {
        titles.iter().map(|s| (*s).to_string()).collect()
    }

    // ---- Positive cases: grounded synthesis ------------------------------

    #[test]
    fn fully_grounded_output_scores_high() {
        let output = "SITUATION: React 19.2 released. PRIORITY: update your React apps.";
        let c = corpus(&[
            "React 19.2 released with concurrent rendering",
            "React usage in 4DA",
        ]);
        let r = validate_groundedness(output, &c);
        assert!(r.confidence >= 0.7, "confidence was {}", r.confidence);
        assert!(r.is_acceptable(0.5));
    }

    #[test]
    fn multi_word_phrase_grounded_even_if_reordered() {
        let output = "TanStack Start now supports React Server Components.";
        let c = corpus(&["React Server Components are now supported in TanStack Start"]);
        let r = validate_groundedness(output, &c);
        assert!(r.confidence >= 0.7, "confidence was {}", r.confidence);
    }

    // ---- Negative: the production screenshot bug ------------------------

    #[test]
    fn rejects_var_and_stripe_hallucination() {
        let output = "Recommend update of your strategy for non-test architecture, \
                      including a 5+ year migration from VAR and Stripe";
        // None of these terms are in the corpus:
        let c = corpus(&[
            "TanStack Start now supports React Server Components",
            "npm: react v19.2.3",
            "npm: typescript v5.6",
        ]);
        let r = validate_groundedness(output, &c);
        assert!(!r.ungrounded_terms.is_empty(), "expected ungrounded terms");
        assert!(
            r.ungrounded_terms
                .iter()
                .any(|t| t.eq_ignore_ascii_case("stripe")),
            "expected Stripe to be flagged, got {:?}",
            r.ungrounded_terms
        );
    }

    #[test]
    fn invented_version_gets_flagged() {
        let output = "Upgrade to tokio 99.99 for the new runtime.";
        let c = corpus(&["tokio 1.38 released with runtime fixes"]);
        let r = validate_groundedness(output, &c);
        assert!(
            r.ungrounded_terms.iter().any(|t| t.starts_with("99.")),
            "expected 99.99 to be flagged, got {:?}",
            r.ungrounded_terms
        );
    }

    #[test]
    fn generic_output_with_no_terms_is_rejected() {
        // A synthesis with zero salient terms is too generic to be
        // useful — it's not referencing anything specific from the
        // signals. The specificity floor rejects this.
        let output = "Your stack is quiet overnight.";
        let r = validate_groundedness(output, &[]);
        assert!(!r.is_acceptable(0.8));
    }

    #[test]
    fn stopwords_are_not_counted_as_salient_terms() {
        let output = "Today, This, The, That, Now.";
        let terms = extract_salient_terms(output);
        assert!(terms.is_empty(), "stopwords leaked as terms: {:?}", terms);
    }

    // ---- Unit tests for the extractor ------------------------------------

    #[test]
    fn extractor_finds_version_tokens() {
        let terms = extract_salient_terms("Upgrade tokio to 1.38 and typescript to 5.6.");
        assert!(terms.iter().any(|t| t == "1.38"));
        assert!(terms.iter().any(|t| t == "5.6"));
    }

    #[test]
    fn extractor_finds_multiword_proper_nouns() {
        let terms = extract_salient_terms("React Server Components are stable.");
        assert!(
            terms.iter().any(|t| t == "React Server Components"),
            "got {:?}",
            terms
        );
    }

    #[test]
    fn extractor_finds_single_proper_nouns() {
        let terms = extract_salient_terms("Consider migrating to Postgres from MySQL.");
        assert!(
            terms.iter().any(|t| t.eq_ignore_ascii_case("Postgres")),
            "got {:?}",
            terms
        );
    }

    #[test]
    fn extractor_skips_dates() {
        let terms = extract_salient_terms("Released on 2026.04.15 by the team.");
        assert!(
            !terms.iter().any(|t| t == "2026.04.15"),
            "date leaked as term: {:?}",
            terms
        );
    }

    #[test]
    fn extractor_dedupes_case_insensitively() {
        let terms = extract_salient_terms("Stripe released. Stripe fixed.");
        let stripe_count = terms
            .iter()
            .filter(|t| t.eq_ignore_ascii_case("Stripe"))
            .count();
        assert_eq!(stripe_count, 1);
    }

    #[test]
    fn extractor_handles_trailing_punctuation() {
        let terms = extract_salient_terms("React Server Components, Next.js, and Remix.");
        assert!(terms.iter().any(|t| !t.ends_with(',') && !t.ends_with('.')));
    }

    // ---- Grounded-check unit tests ----------------------------------------

    #[test]
    fn is_term_grounded_substring_match() {
        let c = vec!["react 19.2 released".to_string()];
        assert!(is_term_grounded("React 19.2", &c));
    }

    #[test]
    fn is_term_grounded_multiword_any_order() {
        let c = vec!["components server react are stable".to_string()];
        assert!(is_term_grounded("React Server Components", &c));
    }

    #[test]
    fn is_term_grounded_rejects_unrelated() {
        let c = vec!["react 19.2".to_string()];
        assert!(!is_term_grounded("Stripe", &c));
    }

    #[test]
    fn llm_expansion_with_few_terms_rejected_by_specificity_floor() {
        // This output rephrases a title without naming specific
        // technologies — the kind of vague synthesis a small model
        // produces. The specificity floor correctly rejects it because
        // it extracts fewer than 2 salient terms (only "LLMs").
        let output = "Meanwhile, Large Language Models (LLMs) continue to show \
                      limited environmental curiosity in agent scenarios.";
        let c = corpus(&[
            "Agents Explore but Agents Ignore: LLMs Lack Environmental Curiosity",
            "Beyond the YAML File: Understanding Real-World GitHub Actions Workflow Adoption",
        ]);
        let r = validate_groundedness(output, &c);
        assert!(
            !r.is_acceptable(0.65),
            "vague rephrasing with <2 salient terms should be rejected"
        );
    }

    // ---- Regression: the morning-brief abstention bug (2026-06) ----------

    #[test]
    fn lowercase_package_brief_passes_via_allowlist() {
        // The production bug: a specific, grounded synthesis about lowercase
        // npm packages extracted ~0 salient terms (the extractor only sees
        // capitalized names + versions) and was force-abstained. With the
        // brief's package allowlist, the bare names are recognized.
        let output = "jsonwebtoken has a type confusion flaw and axios carries known \
                      vulnerabilities; patch both today.";
        let c = corpus(&[
            "axios: 24 known vulnerabilities axios",
            "jsonwebtoken has Type Confusion jsonwebtoken",
        ]);
        let packages = vec!["axios".to_string(), "jsonwebtoken".to_string()];
        let r = validate_groundedness_with_packages(output, &c, &packages);
        assert!(
            r.total_terms >= 2,
            "package names should be counted as salient: {r:?}"
        );
        assert!(
            r.is_acceptable(0.65),
            "grounded package-centric brief should pass: {r:?}"
        );
    }

    #[test]
    fn scoped_npm_name_is_extracted() {
        let terms = extract_salient_terms(
            "@ai-sdk/provider-utils has an uncontrolled resource consumption issue.",
        );
        assert!(
            terms
                .iter()
                .any(|t| t.eq_ignore_ascii_case("@ai-sdk/provider-utils")),
            "scoped npm name not extracted: {terms:?}"
        );
    }

    #[test]
    fn ordinary_hyphenated_english_is_not_salient() {
        // Guard the package-shape heuristic against counting common hyphenated
        // English, which would read as ungrounded and tank confidence.
        let terms =
            extract_salient_terms("this real-world, well-known, open-source pattern is long-term.");
        assert!(
            terms.is_empty(),
            "hyphenated English leaked as salient terms: {terms:?}"
        );
    }

    #[test]
    fn empty_extraction_reports_zero_confidence_not_one() {
        // Honest reporting: nothing groundable => 0.0, not a misleading 1.0
        // that contradicts the specificity-floor rejection in the logs.
        let r = validate_groundedness("nothing salient here at all", &[]);
        assert_eq!(r.total_terms, 0);
        assert_eq!(r.confidence, 0.0);
        assert!(!r.is_acceptable(0.65));
    }

    #[test]
    fn realistic_imperative_prose_is_not_falsely_rejected() {
        // Regression guard for the #3 design decision: imperative prose like
        // "Upgrade Tokio" forms a capitalized run whose verb the extractor sees
        // as a term. With a realistic corpus (as production builds from item
        // text + advisory explanations) the content is grounded and passes.
        // The claim layer is telemetry-only — it must never gate this away.
        let output = "Tokio has a confirmed advisory. Upgrade Tokio to 1.38.6 today.";
        let c = corpus(&["tokio security advisory: upgrade tokio to 1.38.6 to patch the runtime"]);
        let r = validate_groundedness(output, &c);
        assert!(
            r.is_acceptable(0.65),
            "grounded imperative prose must not be force-abstained: {r:?}"
        );
    }

    // ---- #2: deterministic factual version verification -------------------

    fn facts(pairs: &[(&str, &[&str])]) -> Vec<PackageFact> {
        pairs
            .iter()
            .map(|(n, vs)| PackageFact {
                name: (*n).to_string(),
                versions: vs.iter().map(|v| (*v).to_string()).collect(),
            })
            .collect()
    }

    #[test]
    fn factual_check_passes_correct_versions() {
        let prose = "Upgrade jsonwebtoken to 10.3.0 and axios to 1.16.0 today.";
        let f = facts(&[
            ("jsonwebtoken", &["9.3.1", "10.3.0"]),
            ("axios", &["1.12.2", "1.16.0"]),
        ]);
        assert!(check_factual_claims(prose, &f).is_empty());
    }

    #[test]
    fn factual_check_flags_fabricated_version() {
        let prose = "Upgrade axios to 1.99.0 immediately.";
        let f = facts(&[("axios", &["1.12.2", "1.16.0"])]);
        let v = check_factual_claims(prose, &f);
        assert!(!v.is_empty(), "fabricated version should be flagged");
        assert!(v[0].contains("axios"), "got {v:?}");
    }

    #[test]
    fn factual_check_attributes_versions_to_right_package() {
        // 1.16.0 belongs to axios; jsonwebtoken's window must end at "axios".
        let prose = "Upgrade jsonwebtoken to 10.3.0 and axios to 1.16.0.";
        let f = facts(&[("jsonwebtoken", &["10.3.0"]), ("axios", &["1.16.0"])]);
        assert!(check_factual_claims(prose, &f).is_empty());
    }

    #[test]
    fn factual_check_ignores_version_in_a_later_sentence() {
        // The "19.2" belongs to a different sentence/topic, not axios.
        let prose = "Upgrade axios to 1.16.0 now. React 19.2 just shipped.";
        let f = facts(&[("axios", &["1.16.0"])]);
        assert!(
            check_factual_claims(prose, &f).is_empty(),
            "a later-sentence version must not be attributed to axios"
        );
    }

    #[test]
    fn factual_check_prefix_tolerant() {
        let prose = "Bump axios to 1.16.";
        let f = facts(&[("axios", &["1.16.0"])]);
        assert!(
            check_factual_claims(prose, &f).is_empty(),
            "1.16 should match 1.16.0"
        );
    }

    #[test]
    fn factual_check_scoped_name_alias() {
        let prose = "Update provider-utils to 2.1.0.";
        let f = facts(&[("@ai-sdk/provider-utils", &["2.1.0"])]);
        assert!(
            check_factual_claims(prose, &f).is_empty(),
            "bare 'provider-utils' should map to the scoped name"
        );
    }

    #[test]
    fn claim_telemetry_separates_factual_from_claims() {
        // "Apache Kafka" is an invented multi-word proper noun (claim layer);
        // axios + version are factual. Telemetry tracks it; it is NOT gated.
        let output = "Apache Kafka is unrelated. Upgrade axios to 1.16.0.";
        let c = corpus(&["axios advisory affecting your stack"]);
        let packages = vec!["axios".to_string()];
        let r = validate_groundedness_with_packages(output, &c, &packages);
        assert!(
            r.claim_terms >= 1,
            "Apache Kafka should count as a claim term"
        );
        assert!(
            r.claim_confidence() < 1.0,
            "ungrounded claim lowers claim_confidence"
        );
    }
}
