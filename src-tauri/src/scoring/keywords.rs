// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::aliases;
use super::calibration::BROAD_INTEREST_TERMS;
use super::stemming;
use super::utils::has_word_boundary_match;
use crate::context_engine;
use crate::scoring_config;
use fourda_macros::score_component;

/// Compute how specific an interest topic is (no profile).
/// Test-only convenience: production paths use the `_for` variant.
#[cfg(test)]
pub(crate) fn interest_specificity_weight(interest_topic: &str) -> f32 {
    interest_specificity_weight_for(interest_topic, None)
}

/// Profile-aware variant: a broad term that is the user's own detected
/// primary domain (e.g. "security" for a security engineer) is NOT treated
/// as broad - it falls through to the normal word-count weighting.
pub(crate) fn interest_specificity_weight_for(
    interest_topic: &str,
    profile: Option<&super::calibration::SpecificityProfile>,
) -> f32 {
    let topic_lower = interest_topic.to_lowercase();
    let word_count = topic_lower.split_whitespace().count();

    let is_broad = BROAD_INTEREST_TERMS
        .iter()
        .any(|b| topic_lower == *b || topic_lower.contains(b))
        && !profile.is_some_and(|p| p.exempts_broad(&topic_lower));

    if is_broad {
        scoring_config::SPECIFICITY_BROAD // Broad terms contribute 25% of normal weight
    } else if word_count <= 1 {
        scoring_config::SPECIFICITY_SINGLE_WORD // Single-word terms are moderately specific
    } else {
        scoring_config::SPECIFICITY_MULTI_WORD // Multi-word specific terms get full weight
    }
}

/// Find the best-matching interest for an item and return its specificity weight.
/// Used to attenuate keyword_score for broad interests.
///
/// When the user has very few interests (1-2), SPECIFIC interests get full
/// 1.0x weight — someone who only declares "Tauri" and "Rust" clearly means
/// both. GENERIC lone interests ("ai", "api") keep their computed specificity
/// weight so the gate's broad-interest corroboration guard still applies.
/// With 3+ interests the broad-term discount kicks in at a gentler rate
/// (0.60x for 3-5 interests) than the default (0.25x for 6+).
pub(crate) fn best_interest_specificity_weight(
    title: &str,
    content: &str,
    interests: &[context_engine::Interest],
) -> f32 {
    best_interest_specificity_weight_for(title, content, interests, None)
}

/// Profile-aware variant of [`best_interest_specificity_weight`]: broad terms
/// that are the user's own detected domain keep their normal (non-broad)
/// specificity weight.
pub(crate) fn best_interest_specificity_weight_for(
    title: &str,
    content: &str,
    interests: &[context_engine::Interest],
    profile: Option<&super::calibration::SpecificityProfile>,
) -> f32 {
    if interests.is_empty() {
        return 1.0;
    }

    // Focused users: if 1-2 declared interests, trust all of them at full weight.
    // They chose few → each one is definitional, not casual.
    let focused_user = interests.len() <= 2;

    let title_lower = title.to_lowercase();
    let text_lower = format!("{} {}", title_lower, content.to_lowercase());
    let mut best_weight: f32 = 1.0;
    let mut found_match = false;

    for interest in interests {
        let interest_lower = interest.topic.to_lowercase();
        let terms: Vec<&str> = interest_lower.split_whitespace().collect();

        // Check if any term from this interest appears in the item
        let has_hit = terms.iter().any(|term| {
            if term.len() < 3 && !SHORT_TECH_KEYWORDS.contains(term) {
                return false;
            }
            // Fast path: direct word-boundary match at every length. Raw
            // `contains` for len >= 3 let "rust" hit "frustrating" and
            // "react" hit "reaction" — a phantom keyword axis. Punctuation
            // still bounds ("react-native", "node.js" both match "react"/
            // "node").
            if has_word_boundary_match(&title_lower, term)
                || has_word_boundary_match(&text_lower, term)
            {
                return true;
            }
            // Alias expansion — word-boundary at every length too: alias
            // groups carry their own variants ("react"/"reactjs"/"react.js"),
            // so substring matching only added false positives.
            if let Some(group) = aliases::get_aliases(term) {
                if group.iter().any(|alias| {
                    has_word_boundary_match(&title_lower, alias)
                        || has_word_boundary_match(&text_lower, alias)
                }) {
                    return true;
                }
            }
            // Stemmed match — only for plain-English interests. Known tech
            // names (anything with an alias group: react, rust, go, ...)
            // match literally or via aliases only; English morphology is a
            // different word ("reaction" is not React, "rusted" is not Rust).
            if aliases::get_aliases(term).is_none() {
                let term_stem = stemming::stem(term);
                if term_stem.len() >= 3 {
                    let words_match = title_lower
                        .split(|c: char| !c.is_alphanumeric())
                        .chain(text_lower.split(|c: char| !c.is_alphanumeric()))
                        .any(|w| {
                            w.len() >= 3 && stemming::stems_equiv(&stemming::stem(w), &term_stem)
                        });
                    if words_match {
                        return true;
                    }
                }
            }
            false
        });

        if has_hit {
            let w = if focused_user {
                // Focused users trust their few declared interests at full
                // weight — but only when the interest is SPECIFIC. A lone
                // GENERIC interest ("ai", "api", "open source") forced to
                // 1.0 defeated the broad-interest corroboration guard at
                // the confirmation gate (gate.rs only requires embedding
                // corroboration below 0.50), so generic terms fall back to
                // their computed specificity weight instead.
                if is_generic_interest_term(&interest_lower, profile) {
                    interest_specificity_weight_for(&interest.topic, profile)
                } else {
                    1.0
                }
            } else if interests.len() <= 5 {
                // 3-5 interests: softer discount (0.60x floor for broad terms)
                let raw_w = interest_specificity_weight_for(&interest.topic, profile);
                raw_w.max(0.60)
            } else {
                // 6+ interests: full specificity logic (0.25x for broad)
                interest_specificity_weight_for(&interest.topic, profile)
            };
            if !found_match || w > best_weight {
                best_weight = w;
                found_match = true;
            }
        }
    }

    if found_match {
        best_weight
    } else {
        1.0 // No keyword match -> don't attenuate
    }
}

/// Short tech keywords that are valid despite being <3 chars.
/// These are common abbreviations that users declare as interests.
const SHORT_TECH_KEYWORDS: &[&str] = &[
    "ai", "ml", "go", "r", "c", "ui", "ux", "db", "os", "ci", "cd", "qa", "js", "ts", "py", "rx",
    "vm", "k8", "tf", "gcp", "aws", "api", "cli", "css", "sql", "llm", "nlp", "cv",
];

/// A declared interest is "generic" when it is a known broad term (per the
/// specificity classifier, profile-exempted) or a single common-English /
/// ambiguous token ("ai", "api", "data"). Generic interests never earn the
/// focused-user 1.0 specificity override: they keep their computed weight so
/// the confirmation gate's broad-interest corroboration guard still applies
/// and a bare keyword hit alone cannot confirm the interest axis.
fn is_generic_interest_term(
    interest_lower: &str,
    profile: Option<&super::calibration::SpecificityProfile>,
) -> bool {
    let is_broad = BROAD_INTEREST_TERMS
        .iter()
        .any(|b| interest_lower == *b || interest_lower.contains(b))
        && !profile.is_some_and(|p| p.exempts_broad(interest_lower));
    if is_broad {
        return true;
    }
    // A single-word interest that is a common English word / ambiguous
    // dep-style token is generic; multi-word interests are specific.
    let mut words = interest_lower.split_whitespace();
    match (words.next(), words.next()) {
        (Some(word), None) => super::is_ambiguous_dep_name(word),
        _ => false,
    }
}

/// Negation patterns that indicate a term is mentioned in a negative context.
/// Returns true if the term appears near a negation phrase in the text.
fn is_negated_in_context(term: &str, text: &str) -> bool {
    const NEGATION_PREFIXES: &[&str] = &[
        "not ",
        "no ",
        "don't ",
        "doesn't ",
        "didn't ",
        "won't ",
        "isn't ",
        "aren't ",
        "without ",
        "never ",
        "avoid ",
        "stop using ",
        "alternative to ",
        "instead of ",
        "replace ",
        "replacing ",
        "moved away from ",
        "moving away from ",
        "migrating from ",
        "leaving ",
        "dropped ",
        "dropping ",
        "removed ",
        "removing ",
        "don't use ",
        "doesn't use ",
        "didn't use ",
        "won't use ",
        "not using ",
        "stopped using ",
        "quit ",
        "quitting ",
    ];

    let text_lower = text.to_lowercase();
    let term_lower = term.to_lowercase();

    for (idx, _) in text_lower.match_indices(&term_lower) {
        let before_start = text_lower.floor_char_boundary(idx.saturating_sub(30));
        let before = &text_lower[before_start..idx];
        if NEGATION_PREFIXES.iter().any(|neg| before.ends_with(neg)) {
            return true;
        }
    }
    false
}

/// Count word-boundary-aware occurrences of a term in text.
fn count_word_occurrences(term: &str, text: &str) -> usize {
    let mut count = 0;
    for (i, _) in text.match_indices(term) {
        // CHAR-boundary check, not raw bytes: a UTF-8 continuation byte is not
        // ASCII-alphanumeric, so byte-based bounds treated a glued non-ASCII letter
        // ("иgo") as a word boundary and inflated short-term counts (bug E).
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
            count += 1;
        }
    }
    count
}

/// BM25-inspired term density: rewards content where matched terms appear frequently
/// relative to document length. Returns a multiplier in [1.0, 1.5].
///
/// Uses simplified BM25 formula: tf(t,d) = freq / (freq + k1 * (1 - b + b * dl/avgdl))
/// where k1=1.2, b=0.75, avgdl=150 (typical dev article word count after truncation).
fn term_density_multiplier(term: &str, text: &str) -> f32 {
    let term_lower = term.to_lowercase();
    let freq = count_word_occurrences(&term_lower, text) as f32;
    if freq <= 1.0 {
        return 1.0;
    }

    let word_count = text.split_whitespace().count().max(1) as f32;
    let k1: f32 = 1.2;
    let b: f32 = 0.75;
    let avgdl: f32 = 150.0;
    let tf = freq / (freq + k1 * (1.0 - b + b * word_count / avgdl));

    // Map tf (range ~0.45-0.83 for typical values) to a 1.0-1.5 multiplier
    (1.0 + tf * 0.6).min(1.5)
}

/// Keyword-based interest matching: boosts items that literally contain declared interest terms.
/// Complements semantic matching which can miss exact keyword matches.
#[score_component(output_range = "0.0..=1.0")]
pub(crate) fn compute_keyword_interest_score(
    title: &str,
    content: &str,
    interests: &[context_engine::Interest],
) -> f32 {
    if interests.is_empty() {
        return 0.0;
    }

    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();
    let text_lower = format!("{} {}", title_lower, content_lower);
    let mut max_score: f32 = 0.0;

    for interest in interests {
        let interest_lower = interest.topic.to_lowercase();
        let terms: Vec<&str> = interest_lower.split_whitespace().collect();
        if terms.is_empty() {
            continue;
        }

        // Multi-word phrase check: exact phrase match is the strongest keyword signal
        if terms.len() > 1 {
            let phrase = &interest_lower;
            let in_title = title_lower.contains(phrase.as_str());
            let in_content = !in_title && text_lower.contains(phrase.as_str());
            if in_title || in_content {
                let mut phrase_score = if in_title { 0.95 } else { 0.80 };
                let density = term_density_multiplier(phrase, &text_lower);
                phrase_score = (phrase_score * density).min(1.0);
                if is_negated_in_context(phrase, &text_lower) {
                    phrase_score *= 0.5;
                }
                max_score = max_score.max(phrase_score * interest.weight);
                continue;
            }
        }

        let mut hits = 0.0_f32;
        let mut counted_terms = 0_usize;
        for term in &terms {
            // Skip generic short words, but allow known tech abbreviations
            if term.len() < 3 && !SHORT_TECH_KEYWORDS.contains(term) {
                continue;
            }
            counted_terms += 1;

            // Determine match and effective search term for density/negation
            let (base_hit, search_term): (f32, Option<&str>) = {
                // Direct word-boundary match at every term length. Raw
                // `contains` for len >= 3 let "rust" hit "frustrating" and
                // "react" hit "reaction" (phantom keyword axis); punctuation
                // still bounds, so "react-dom" / "node.js" keep matching.
                let direct_title = has_word_boundary_match(&title_lower, term);
                let direct_content = if !direct_title {
                    has_word_boundary_match(&text_lower, term)
                } else {
                    false
                };

                if direct_title {
                    (0.80, Some(*term))
                } else if direct_content {
                    (0.55, Some(*term))
                } else {
                    // Alias expansion — find which alias actually matched.
                    // Word-boundary at every length: alias groups carry their
                    // own variants ("react"/"reactjs"/"react.js"), substring
                    // matching only added false positives ("express" inside
                    // "expression").
                    let alias_match: Option<(&str, bool)> =
                        aliases::get_aliases(term).and_then(|group| {
                            for alias in group.iter() {
                                if has_word_boundary_match(&title_lower, alias) {
                                    return Some((*alias, true));
                                }
                                if has_word_boundary_match(&text_lower, alias) {
                                    return Some((*alias, false));
                                }
                            }
                            None
                        });

                    if let Some((matched_alias, in_title)) = alias_match {
                        (if in_title { 0.80 } else { 0.55 }, Some(matched_alias))
                    } else if aliases::get_aliases(term).is_some() {
                        // Known tech name (has an alias group) with no direct
                        // or alias hit: do NOT fall through to stemming —
                        // English morphology is a different word ("reaction"
                        // is not React, "rusted" is not Rust).
                        (0.0, None)
                    } else {
                        // Stemmed match (no effective term for density/negation)
                        let term_stem = stemming::stem(term);
                        if term_stem.len() >= 3 {
                            let title_stem_hit =
                                title_lower.split(|c: char| !c.is_alphanumeric()).any(|w| {
                                    w.len() >= 3
                                        && stemming::stems_equiv(&stemming::stem(w), &term_stem)
                                });
                            let content_stem_hit = !title_stem_hit
                                && text_lower.split(|c: char| !c.is_alphanumeric()).any(|w| {
                                    w.len() >= 3
                                        && stemming::stems_equiv(&stemming::stem(w), &term_stem)
                                });

                            if title_stem_hit {
                                (0.65, None)
                            } else if content_stem_hit {
                                (0.45, None)
                            } else {
                                (0.0, None)
                            }
                        } else {
                            (0.0, None)
                        }
                    }
                }
            };

            let mut term_hit = base_hit;

            // First-paragraph boost: content matches in the first 200 chars
            // are stronger signals — the article is likely ABOUT this topic
            if term_hit > 0.0 && term_hit < 0.80 && content_lower.len() >= 3 {
                let effective = search_term.unwrap_or(term);
                let end = content_lower.floor_char_boundary(content_lower.len().min(200));
                if has_word_boundary_match(&content_lower[..end], effective) {
                    term_hit = (term_hit + 0.10).min(0.80);
                }
            }

            // Density bonus: only for direct/alias matches where we know the search term
            if term_hit > 0.0 {
                if let Some(st) = search_term {
                    let density = term_density_multiplier(st, &text_lower);
                    term_hit *= density;
                }
            }

            // Negation penalty: only for direct/alias matches
            if term_hit > 0.0 {
                if let Some(st) = search_term {
                    if st.len() >= 3 && is_negated_in_context(st, &text_lower) {
                        term_hit *= 0.5;
                    }
                }
            }

            hits += term_hit;
        }

        let divisor = counted_terms.max(1) as f32;
        let score = (hits / divisor).min(1.0) * interest.weight;
        max_score = max_score.max(score);
    }

    max_score
}

#[cfg(test)]
#[path = "keywords_tests.rs"]
mod tests;
