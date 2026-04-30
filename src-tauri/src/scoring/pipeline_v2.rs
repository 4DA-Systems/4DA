// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! PASIFA V2 Scoring Pipeline — 8-phase structured architecture
//!
//! Restructures V1 into clean, testable phases while reusing all existing module functions.
//!
//! Key improvements over V1:
//! - **Separate KNN calibration** (`calibrate_knn`) with center=0.49, scale=12 to suppress KNN noise
//! - **Gate count on clean signals** (Phase 3): count signals before any combination
//! - **Multiplicative semantic**: `base * (1.0 + semantic_boost)` not additive
//! - **Single quality composite** (Phase 5): all multipliers dampened and multiplied in one pass,
//!   with domain_quality_mult restored as a multiplicative factor (NOT dampened)
//! - **Single boost cap** (Phase 6): all boosts summed, capped at \[-0.15, 0.35\], then dampened
//! - **Gate table** matches V1 for 0-1 signals: \[(0.25,0.20), (0.45,0.28), ...\]
//! - **Score ceiling applied LAST** in gate phase — after domain gate mult

use std::collections::HashMap;

use crate::db::Database;
use crate::scoring_config;
use crate::signals;
use crate::{
    check_exclusions, extract_topics, get_relevance_threshold, RelevanceMatch, ScoreBreakdown,
    SourceRelevance,
};

use super::pipeline::{ScoringInput, ScoringOptions};
use super::*;
use crate::sources::cve_matching::normalize_ecosystem;

// ============================================================================
// Security evidence extraction helpers
// ============================================================================

/// Extract advisory ID (GHSA-xxxx-yyyy-zzzz or CVE-2025-XXXXX) from title text.
fn extract_advisory_id(title: &str) -> Option<String> {
    // Try GHSA pattern
    if let Some(start) = title.find("GHSA-") {
        let rest = &title[start..];
        let end = rest
            .find(|c: char| c == ']' || c == ' ' || c == ')')
            .unwrap_or(rest.len());
        return Some(rest[..end].to_string());
    }
    // Try CVE pattern
    if let Some(start) = title.find("CVE-") {
        let rest = &title[start..];
        let end = rest
            .find(|c: char| c == ']' || c == ' ' || c == ')')
            .unwrap_or(rest.len());
        return Some(rest[..end].to_string());
    }
    None
}

/// Extract CVSS score and severity label from content that contains "Severity: CVSS_V3: X.X".
fn extract_cvss_from_content(content: &str) -> (Option<f32>, Option<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Severity:") {
            let nums: Vec<f32> = trimmed
                .split(|c: char| !c.is_ascii_digit() && c != '.')
                .filter_map(|s| s.parse::<f32>().ok())
                .filter(|&v| v <= 10.0 && v > 0.0)
                .collect();
            if let Some(&score) = nums.first() {
                let severity = if score >= 9.0 {
                    "critical"
                } else if score >= 7.0 {
                    "high"
                } else if score >= 4.0 {
                    "medium"
                } else {
                    "low"
                };
                return (Some(score), Some(severity.to_string()));
            }
        }
    }
    (None, None)
}

/// Extract fixed version from content (e.g. "Fixed in: 3.0.1").
fn extract_fixed_version(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Fixed in:") || trimmed.starts_with("Patched in:") {
            let version = trimmed.split_once(':')?.1.trim();
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }
    }
    None
}

/// Extract affected version range from content (e.g. "Affected: < 3.0.0").
fn extract_affected_range(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Affected:") {
            let range = trimmed.split_once(':')?.1.trim();
            if !range.is_empty() {
                return Some(range.to_string());
            }
        }
    }
    None
}

/// Check if installed_version falls within the affected range.
/// Supports patterns like "< 3.0.1", "<= 2.8.0", ">= 1.0 < 3.0".
/// Returns None if either input is missing or unparseable.
fn check_version_affected(
    installed: Option<&str>,
    affected_range: Option<&str>,
    fixed: Option<&str>,
) -> Option<bool> {
    let inst_str = installed?;
    let inst = semver::Version::parse(inst_str).ok()?;

    // If we have a fixed version, simple check: affected if installed < fixed
    if let Some(fix_str) = fixed {
        if let Ok(fix) = semver::Version::parse(fix_str) {
            return Some(inst < fix);
        }
    }

    // Try parsing affected range as a semver requirement
    let range_str = affected_range?;
    if let Ok(req) = semver::VersionReq::parse(range_str) {
        return Some(req.matches(&inst));
    }

    None
}

// ============================================================================
// V2 Constants (self-contained)
// ============================================================================

// Calibration rationale (2026-04-07 score-spread widening):
//
// The pipeline was over-damped: 12 dampened multipliers + capped boosts + gate
// ceilings compressed a 0.55–0.85 raw range into 0.50–0.65 output, losing 50%
// of useful score spread. These changes restore differentiation:
//
// - 0/1 signal ceilings are INTENTIONALLY hard — noise suppression is critical.
//   A single confirmation axis must never push an item above threshold (0.35).
// - 2/3 signal ceilings raised (0.65→0.72, 0.85→0.88) to create usable spread
//   for legitimately confirmed content without touching noise floor.
// - STRENGTH_BONUS_MAX raised 0.08→0.12 for within-tier differentiation:
//   strong 2-signal items can now reach 0.84 vs weak at 0.72 (12-point spread).
// - BOOST_CAP_MAX raised 0.35→0.45 to stop truncating legitimate boost
//   accumulation when 4+ independent boosts fire simultaneously.
// - Dampening reduced (penalty 0.65→0.72, boost 0.55→0.65) in pipeline.scoring
//   to preserve more signal through the quality composite (~3.2% less automatic
//   compression per multiplier).

// ============================================================================
// KNN-specific calibration
// ============================================================================

/// Calibrate a raw KNN distance-derived score using a sigmoid stretch.
/// Uses adaptive parameters from embedding_calibration — auto-adapts to
/// whatever embedding model the user runs.
fn calibrate_knn(raw: f32) -> f32 {
    if raw <= 0.0 {
        return 0.0;
    }
    if raw >= 1.0 {
        return 1.0;
    }
    let center = crate::embedding_calibration::get_sigmoid_center();
    let scale = crate::embedding_calibration::get_sigmoid_scale();
    1.0 / (1.0 + ((center - raw) * scale).exp())
}

// ============================================================================
// Signal strength bonus — mid-band spread
// ============================================================================

/// Compute how strong the confirmed signals are, normalized to [0.0, 1.0].
/// Returns a bonus to add to the gate ceiling — strong 2-signal items get
/// a higher ceiling than weak 2-signal items, creating sub-ranking.
///
/// Each confirmed axis contributes its "excess" above threshold (how far above
/// the minimum confirmation level). The average excess drives the bonus.
fn compute_signal_strength_bonus(
    signal_count: u8,
    context_score: f32,
    interest_score: f32,
    keyword_score: f32,
    semantic_boost: f32,
    dep_match_score: f32,
    feedback_boost: f32,
    affinity_mult: f32,
    stack_pain_match: bool,
) -> f32 {
    // Only applies at 2+ signals — 0-1 ceilings are intentionally hard
    if signal_count < 2 {
        return 0.0;
    }

    let mut strengths: Vec<f32> = Vec::with_capacity(5);

    // Context axis: excess above 0.45 threshold, normalized to [0, 1]
    if context_score >= scoring_config::CONTEXT_THRESHOLD {
        let excess = (context_score - scoring_config::CONTEXT_THRESHOLD)
            / (1.0 - scoring_config::CONTEXT_THRESHOLD);
        strengths.push(excess.clamp(0.0, 1.0));
    }

    // Interest axis: best of interest_score and keyword_score
    let interest_confirmed = interest_score >= scoring_config::INTEREST_THRESHOLD
        || keyword_score >= scoring_config::KEYWORD_THRESHOLD;
    if interest_confirmed {
        let best = interest_score.max(keyword_score);
        let threshold = scoring_config::INTEREST_THRESHOLD.min(scoring_config::KEYWORD_THRESHOLD);
        let excess = (best - threshold) / (1.0 - threshold).max(0.01);
        strengths.push(excess.clamp(0.0, 1.0));
    }

    // ACE axis: semantic boost or active topic match
    let ace_confirmed = semantic_boost >= scoring_config::SEMANTIC_THRESHOLD || stack_pain_match;
    if ace_confirmed {
        if semantic_boost >= scoring_config::SEMANTIC_THRESHOLD {
            // Normalize semantic excess (practical range 0.18-0.50)
            let excess = (semantic_boost - scoring_config::SEMANTIC_THRESHOLD) / scoring_config::SIGNAL_NORMALIZATION_SEMANTIC_RANGE;
            strengths.push(excess.clamp(0.0, 1.0));
        } else {
            // stack_pain_match is binary — use flat strength
            strengths.push(scoring_config::SIGNAL_NORMALIZATION_STACK_PAIN_STRENGTH);
        }
    }

    // Learned axis: feedback or affinity
    if feedback_boost > scoring_config::FEEDBACK_THRESHOLD
        || affinity_mult >= scoring_config::AFFINITY_THRESHOLD
    {
        // Affinity-driven strength (affinity range 1.15-1.70)
        if affinity_mult >= scoring_config::AFFINITY_THRESHOLD {
            let excess = (affinity_mult - scoring_config::AFFINITY_THRESHOLD) / scoring_config::SIGNAL_NORMALIZATION_AFFINITY_RANGE;
            strengths.push(excess.clamp(0.0, 1.0));
        } else {
            strengths.push(scoring_config::SIGNAL_NORMALIZATION_FEEDBACK_STRENGTH); // Feedback is less granular
        }
    }

    // Dependency axis
    if dep_match_score >= scoring_config::DEPENDENCY_THRESHOLD {
        let excess = (dep_match_score - scoring_config::DEPENDENCY_THRESHOLD)
            / (1.0 - scoring_config::DEPENDENCY_THRESHOLD);
        strengths.push(excess.clamp(0.0, 1.0));
    }

    if strengths.is_empty() {
        return 0.0;
    }

    let avg_strength = strengths.iter().sum::<f32>() / strengths.len() as f32;
    avg_strength * scoring_config::BOOST_CLAMP_STRENGTH_BONUS_MAX
}

// ============================================================================
// Signal data structures
// ============================================================================

/// All raw signal values extracted from the input, before calibration.
struct RawSignals {
    context: f32,
    interest: f32,
    keyword_score: f32,
    semantic_boost: f32,
    dep_match_score: f32,
    matched_deps: Vec<dependencies::DepMatch>,
    feedback_boost: f32,
    affinity_mult: f32,
    anti_penalty: f32,
    domain_relevance: f32,
    taste_boost: f32,
    stack_boost: f32,
    stack_pain_match: bool,
    topics: Vec<String>,
    specificity_weight: f32,
}

/// Calibrated signal values ready for combination.
struct CalibratedSignals {
    context_score: f32,
    interest_score: f32,
    keyword_score: f32,
    semantic_boost: f32,
}

// ============================================================================
// Phase 1: Extract all raw signals independently
// ============================================================================

/// Strip synthetic metadata blocks from security-advisory content before
/// running dependency text matching.
///
/// The CVE and OSV source adapters format content as:
///   `{description}\n\nSeverity: {sev}\nAffected: {pkg1 (eco), pkg2 (eco)...}\n{cvss}`
///
/// The `Affected:` line is a raw concatenation of every affected package
/// name from the advisory — which causes massive false positives when
/// `match_dependencies` runs word-boundary search against it. For example,
/// a CVE affecting `aws-lc-rs` lists "aws-lc-rs (rust)" in the Affected
/// line, and the word "rs" or substrings trigger matches on unrelated user
/// deps. The stripped form only keeps the actual prose description, which
/// is where a legitimate mention of a user's package would appear.
///
/// Returns the content unchanged when no `\n\nSeverity:` marker is found
/// (non-security sources or future format changes).
fn strip_security_metadata(content: &str) -> &str {
    content
        .split_once("\n\nSeverity:")
        .map_or(content, |(description, _metadata)| description)
}

/// Extract affected package ecosystems from CVE/OSV content metadata.
///
/// Parses the "Affected: pkg1 (eco1), pkg2 (eco2)" line embedded in security
/// advisory content. Returns the list of (package_name, ecosystem) pairs.
/// Empty result when the content doesn't have the expected format.
fn extract_advisory_ecosystems(content: &str) -> Vec<(String, String)> {
    let affected_line = content.lines().find(|line| line.starts_with("Affected: "));
    let line = match affected_line {
        Some(l) => &l["Affected: ".len()..],
        None => return Vec::new(),
    };

    let mut result = Vec::new();
    for entry in line.split(", ") {
        let trimmed = entry.trim();
        if let Some(paren_start) = trimmed.rfind('(') {
            if trimmed.ends_with(')') {
                let name = trimmed[..paren_start].trim().to_lowercase();
                let eco = trimmed[paren_start + 1..trimmed.len() - 1]
                    .trim()
                    .to_lowercase();
                if !name.is_empty() && !eco.is_empty() {
                    result.push((name, eco));
                }
            }
        }
    }
    result
}

/// Return whichever content should be used for dependency matching. For CVE
/// and OSV source items the synthetic metadata block is stripped; all other
/// sources use the content verbatim.
fn dep_match_content_for<'a>(input: &'a ScoringInput) -> &'a str {
    match input.source_type {
        "cve" | "osv" => strip_security_metadata(input.content),
        _ => input.content,
    }
}

fn extract_signals(
    input: &ScoringInput,
    ctx: &ScoringContext,
    matches: &[RelevanceMatch],
) -> RawSignals {
    let topics = extract_topics(input.title, input.content, input.source_tags);

    // Raw context: best KNN score from embedding similarity
    let raw_context = matches.first().map_or(0.0, |m| m.similarity);

    // Raw interest: embedding similarity against declared interests
    let raw_interest = compute_interest_score(input.embedding, &ctx.interests);

    // Keyword interest matching: boosts items containing declared interest terms
    let raw_keyword_score =
        keywords::compute_keyword_interest_score(input.title, input.content, &ctx.interests);
    let specificity_weight =
        keywords::best_interest_specificity_weight(input.title, input.content, &ctx.interests);
    let keyword_score = raw_keyword_score * specificity_weight;

    // Semantic boost with keyword fallback
    let semantic_boost =
        compute_semantic_ace_boost(input.embedding, &ctx.ace_ctx, &ctx.topic_embeddings)
            .unwrap_or_else(|| semantic::compute_keyword_ace_boost(&topics, &ctx.ace_ctx));

    // Dependency intelligence — for security sources, strip the synthetic
    // `Affected:` metadata block from the content so text matching only
    // operates on the actual CVE description, not the list of affected
    // package names that would otherwise create massive false positives.
    let dep_match_text = dep_match_content_for(input);
    let (matched_deps, dep_match_score) = {
        let (mut deps, mut score) =
            match_dependencies(input.title, dep_match_text, &topics, &ctx.ace_ctx);

        // For CVE/OSV items, apply a MUCH stricter post-filter. The goal is
        // to only keep matches where the CVE is plausibly about the user's
        // actual package — not a generic English word that happens to
        // coincide with a package name (hostname, proxy, client, cert, ...).
        //
        // A match survives if ANY of:
        //   1. Full normalized package name appears in the TITLE (high
        //      evidence — advisories name the affected software directly).
        //   2. Full name appears in the description AND there is package
        //      language context ("npm X", "cargo X", "crate X", "package X")
        //      within 80 chars, OR
        //   3. Name contains a hyphen (compound names like `x509-cert` are
        //      inherently specific and hyphen matches are strong evidence).
        //
        // Single word-boundary hits in prose (e.g. the word "hostname" in
        // a DNS-related advisory) are rejected — they're noise.
        if matches!(input.source_type, "cve" | "osv") && !deps.is_empty() {
            let title_lower = input.title.to_lowercase();
            let body_lower = dep_match_text.to_lowercase();
            deps.retain(|d| {
                let full = d.package_name.to_lowercase();

                // Rule 1: title match
                if has_word_boundary_match(&title_lower, &full) {
                    return true;
                }

                // Rule 3: compound name (contains hyphen)
                let is_compound = full.contains('-');
                if !has_word_boundary_match(&body_lower, &full) {
                    return false;
                }
                if is_compound {
                    return true;
                }

                // Rule 2: single-word name — require language context nearby
                const CONTEXT_WORDS: &[&str] = &[
                    "npm",
                    "cargo",
                    "crate",
                    "crates",
                    "pip",
                    "pypi",
                    "gem",
                    "composer",
                    "maven",
                    "nuget",
                    "package",
                    "library",
                    " lib ",
                    "module",
                    "dependency",
                ];
                let window: usize = 80;
                // Find each occurrence and check for context nearby
                for (idx, _) in body_lower.match_indices(&full) {
                    let start = idx.saturating_sub(window);
                    let end = (idx + full.len() + window).min(body_lower.len());
                    let slice = &body_lower[start..end];
                    if CONTEXT_WORDS.iter().any(|w| slice.contains(w)) {
                        return true;
                    }
                }
                false
            });
            // Recompute dep_match_score from the surviving deps.
            let total: f32 = deps.iter().map(|d| d.confidence).sum();
            score = (total / 2.0).min(1.0);
        }

        // Ecosystem cross-reference: reject matches where the advisory's
        // affected packages are from a different ecosystem than the user's dep.
        // e.g. a Maven/Java CVE should never match a Rust "crypto" crate.
        if matches!(input.source_type, "cve" | "osv") && !deps.is_empty() {
            let advisory_affected = extract_advisory_ecosystems(input.content);
            if !advisory_affected.is_empty() {
                deps.retain(|d| {
                    if d.ecosystem.is_empty() {
                        return true; // no ecosystem info → can't reject
                    }
                    let dep_eco = normalize_ecosystem(&d.ecosystem);
                    advisory_affected
                        .iter()
                        .any(|(_, eco)| normalize_ecosystem(eco) == dep_eco)
                });
                let total: f32 = deps.iter().map(|d| d.confidence).sum();
                score = (total / 2.0).min(1.0);
            }
        }

        (deps, score)
    };

    // Feedback learning boost
    let feedback_boost = if ctx.feedback_boosts.is_empty() {
        0.0
    } else {
        let mut boost_sum: f64 = 0.0;
        let mut match_count = 0;
        for topic in &topics {
            if let Some(&net_score) = ctx.feedback_boosts.get(topic.as_str()) {
                boost_sum += net_score;
                match_count += 1;
            }
        }
        if match_count > 0 {
            ((boost_sum / match_count as f64) * scoring_config::FEEDBACK_SCALE as f64).clamp(
                scoring_config::FEEDBACK_CAP_RANGE.0 as f64,
                scoring_config::FEEDBACK_CAP_RANGE.1 as f64,
            ) as f32
        } else {
            0.0
        }
    };

    // Affinity and anti-penalty from learned topic preferences
    let affinity_mult = compute_affinity_multiplier(&topics, &ctx.ace_ctx);
    let anti_penalty = compute_anti_penalty(&topics, &ctx.ace_ctx);

    // Domain relevance: graduated penalty based on technology identity
    let domain_relevance =
        crate::domain_profile::compute_domain_relevance(&topics, &ctx.domain_profile);

    // Taste embedding boost
    let taste_boost = match ctx.taste_embedding {
        Some(ref taste_emb) if !input.embedding.is_empty() => {
            semantic::compute_taste_boost(input.embedding, taste_emb)
        }
        _ => 0.0,
    };

    // Stack intelligence
    let stack_boost = crate::stacks::scoring::compute_stack_boost(
        input.title,
        input.content,
        &ctx.composed_stack,
    );

    let stack_pain_match = crate::stacks::scoring::has_pain_point_match(
        input.title,
        input.content,
        &ctx.composed_stack,
    );

    RawSignals {
        context: raw_context,
        interest: raw_interest,
        keyword_score,
        semantic_boost,
        dep_match_score,
        matched_deps,
        feedback_boost,
        affinity_mult,
        anti_penalty,
        domain_relevance,
        taste_boost,
        stack_boost,
        stack_pain_match,
        topics,
        specificity_weight,
    }
}

// ============================================================================
// Phase 2: Calibrate raw signals
// ============================================================================

fn calibrate_signals(raw: &RawSignals) -> CalibratedSignals {
    CalibratedSignals {
        context_score: calibrate_knn(raw.context),
        interest_score: calibrate_score(raw.interest),
        keyword_score: raw.keyword_score,   // passthrough
        semantic_boost: raw.semantic_boost, // passthrough
    }
}

// ============================================================================
// Phase 4: Compute base relevance score — FOUR branches
// ============================================================================

fn compute_relevance(
    cal: &CalibratedSignals,
    ctx: &ScoringContext,
    has_real_embedding: bool,
) -> f32 {
    if ctx.cached_context_count > 0 && ctx.interest_count > 0 {
        // Both context and interest available
        let ctx_w = (scoring_config::BASE_BOTH_CONTEXT_BASE
            + cal.context_score * scoring_config::BASE_BOTH_CONTEXT_SCALE)
            .clamp(
                scoring_config::BASE_BOTH_CONTEXT_BASE,
                scoring_config::BASE_BOTH_CONTEXT_MAX,
            );
        let remaining = 1.0 - ctx_w;
        let int_w = remaining * scoring_config::BASE_BOTH_INTEREST_SHARE;
        let kw_w = remaining * scoring_config::BASE_BOTH_KEYWORD_SHARE;
        let base =
            cal.context_score * ctx_w + cal.interest_score * int_w + cal.keyword_score * kw_w;
        // MULTIPLICATIVE semantic
        (base * (1.0 + cal.semantic_boost)).clamp(0.0, 1.0)
    } else if ctx.interest_count > 0 {
        // Interest only
        // Bootstrap semantic dampening: reduce embedding influence for TRULY
        // thin profiles to prevent false positives from noisy embeddings.
        // Previously triggered on (interest_count < 3 && deps < 5) which was
        // too aggressive — a Rust project with 200+ deps and 20+ detected
        // techs still got dampened. Now requires thin ACE signals too.
        let truly_thin_profile = has_real_embedding
            && ctx.interest_count < 3
            && ctx.feedback_interaction_count < 10
            && ctx.ace_ctx.detected_tech.len() < 5
            && ctx.ace_ctx.dependency_names.len() < 10;
        let semantic_mult = if truly_thin_profile {
            scoring_config::INTEREST_ONLY_SEMANTIC_MULT * 0.4
        } else {
            scoring_config::INTEREST_ONLY_SEMANTIC_MULT
        };
        let base = cal.interest_score * scoring_config::INTEREST_ONLY_INTEREST_W
            + cal.keyword_score * scoring_config::INTEREST_ONLY_KEYWORD_W;
        // MULTIPLICATIVE semantic
        (base * (1.0 + cal.semantic_boost * semantic_mult)).clamp(0.0, 1.0)
    } else if ctx.cached_context_count > 0 {
        // Context only
        (cal.context_score * (1.0 + cal.semantic_boost)).clamp(0.0, 1.0)
    } else {
        // Neither
        (cal.semantic_boost * 1.5).clamp(0.0, 1.0)
    }
}

// ============================================================================
// Phase 5: Compute quality composite — ALL multipliers in one pass
// ============================================================================

/// Returns (quality_score, freshness, source_quality_boost, competing_mult, content_quality_mult,
///          content_dna_mult, content_type, novelty_mult, ecosystem_shift_mult, stack_competing_mult,
///          sophistication_mult, content_analysis_mult)
#[allow(clippy::type_complexity)]
fn compute_quality_composite(
    relevance_score: f32,
    input: &ScoringInput,
    ctx: &ScoringContext,
    raw: &RawSignals,
    options: &ScoringOptions,
    db: &Database,
) -> (
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    crate::content_dna::ContentType,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
) {
    // Freshness: topic-aware when autophagy half-lives are available
    let freshness = if options.apply_freshness {
        if let Some(created_at) = input.created_at {
            let base_freshness = compute_temporal_freshness(created_at);
            let topic_adjusted = if !ctx.topic_half_lives.is_empty() && !raw.topics.is_empty() {
                let matching_half_lives: Vec<f32> = raw
                    .topics
                    .iter()
                    .filter_map(|t| ctx.topic_half_lives.get(t.as_str()).copied())
                    .collect();
                if matching_half_lives.is_empty() {
                    base_freshness
                } else {
                    let avg_half_life =
                        matching_half_lives.iter().sum::<f32>() / matching_half_lives.len() as f32;
                    let age_hours =
                        ((chrono::Utc::now() - *created_at).num_minutes() as f32 / 60.0).max(0.0);
                    let calibrated = (-0.693 * age_hours / avg_half_life.max(1.0)).exp();
                    (base_freshness * 0.5 + calibrated * 0.5).clamp(0.3, 1.0)
                }
            } else {
                base_freshness
            };
            // Peak hours boost: slight freshness bonus for content published during
            // the user's active coding hours (from git commit frequency analysis)
            if ctx.ace_ctx.peak_hours.is_empty() {
                topic_adjusted
            } else {
                let publish_hour = chrono::Timelike::hour(created_at) as u8;
                if ctx.ace_ctx.peak_hours.contains(&publish_hour) {
                    (topic_adjusted + 0.03).min(1.0)
                } else {
                    topic_adjusted
                }
            }
        } else {
            1.0
        }
    } else {
        1.0
    };

    // Source quality boost from learned preferences
    let source_quality_boost =
        ctx.source_quality
            .get(input.source_type)
            .copied()
            .map_or(0.0, |score| {
                (score * scoring_config::SOURCE_QUALITY_MULT).clamp(
                    scoring_config::SOURCE_QUALITY_CAP_RANGE.0,
                    scoring_config::SOURCE_QUALITY_CAP_RANGE.1,
                )
            });
    let learned_source_mult = 1.0 + source_quality_boost;

    // Blend learned source quality with autophagy engagement rate (if available)
    let source_quality_mult =
        if let Some(&engagement_rate) = ctx.source_autopsies.get(input.source_type) {
            let autophagy_factor = if engagement_rate < scoring_config::SOURCE_ENGAGEMENT_LOW_THRESHOLD {
                scoring_config::SOURCE_ENGAGEMENT_LOW_PENALTY
            } else if engagement_rate > scoring_config::SOURCE_ENGAGEMENT_HIGH_THRESHOLD {
                scoring_config::SOURCE_ENGAGEMENT_HIGH_BOOST
            } else {
                1.0
            };
            (learned_source_mult * scoring_config::SOURCE_ENGAGEMENT_BLEND_LEARNED_WEIGHT + autophagy_factor * scoring_config::SOURCE_ENGAGEMENT_BLEND_AUTOPHAGY_WEIGHT).clamp(scoring_config::SOURCE_ENGAGEMENT_BLEND_MIN, scoring_config::SOURCE_ENGAGEMENT_BLEND_MAX)
        } else {
            learned_source_mult
        };

    // Anti-topic multiplier
    let anti_mult = 1.0 - raw.anti_penalty;

    // Domain quality penalty (NOT dampened — preserves full penalty strength)
    let domain_quality_mult = if raw.domain_relevance >= scoring_config::DOMAIN_QUALITY_HIGH_THRESHOLD {
        1.0
    } else if raw.domain_relevance >= scoring_config::DOMAIN_QUALITY_MID_THRESHOLD {
        1.0 - scoring_config::OFF_DOMAIN_PENALTY * (1.0 - raw.domain_relevance) * 0.5
    } else {
        1.0 - scoring_config::OFF_DOMAIN_PENALTY * (1.0 - raw.domain_relevance)
    };

    // Competing tech penalty
    let competing_mult = crate::competing_tech::compute_competing_penalty(
        &raw.topics,
        input.title,
        &ctx.domain_profile.primary_stack,
    );

    // Content quality
    let content_quality =
        crate::content_quality::compute_content_quality(input.title, input.content, input.url);

    // Content DNA (source-type-aware)
    let (content_type, content_dna_mult) = crate::content_dna::classify_content_for_source(
        input.title,
        input.content,
        input.source_type,
    );
    // Thin-content penalty: items with negligible body text have less signal
    // to validate their relevance. Title-only items (SO list endpoint, sparse RSS)
    // get a mild discount so they don't score identically to fully-articled content.
    let content_dna_mult = if input.content.len() < 30 {
        content_dna_mult * 0.85
    } else {
        content_dna_mult
    };

    // Novelty
    let novelty = crate::novelty::compute_novelty(
        input.title,
        input.content,
        &raw.topics,
        &ctx.domain_profile.primary_stack,
        ctx.user_role.as_deref(),
        ctx.experience_level.as_deref(),
    );

    // Ecosystem shift from stack profiles
    let ecosystem_shift_mult = crate::stacks::scoring::detect_ecosystem_shift(
        &raw.topics,
        input.title,
        &ctx.composed_stack,
    );

    // Stack-aware competing tech penalty
    let stack_competing_mult = crate::stacks::scoring::compute_competing_penalty(
        input.title,
        input.content,
        &ctx.composed_stack,
    );

    // Content sophistication (audience-aware depth scoring)
    let sophistication = crate::content_sophistication::compute_sophistication(
        input.title,
        input.content,
        ctx.ace_ctx.detected_tech.len(),
        &ctx.domain_profile,
    );
    let sophistication_mult = sophistication.multiplier;

    // Content analysis multiplier (from cached LLM pre-analysis, if available)
    let content_analysis_mult = {
        let hash = crate::content_analysis::content_hash(input.content);
        crate::content_analysis::get_cached_analysis(db, &hash)
            .ok()
            .flatten()
            .map(|a| {
                let is_senior = ctx.ace_ctx.detected_tech.len() > 15
                    && ctx.domain_profile.dependency_names.len() > 50;
                crate::content_analysis::analysis_to_multiplier(&a, is_senior)
            })
            .unwrap_or(1.0)
    };

    // Negative stack prior: Bayesian suppression for technologies user doesn't use.
    // UNDAMPENED — full suppressive force (0.15 for competing-absent, 0.30 for anti-topics).
    let negative_stack_prior =
        crate::stacks::negative_stack::lookup_prior(&ctx.ace_ctx.negative_stack, &raw.topics)
            as f32;

    // NOTE: ecosystem_shift_mult, stack_competing_mult, and content_analysis_mult are
    // still computed above for the return tuple (used by logging/diagnostics) but are
    // intentionally excluded from the composite:
    //   - ecosystem_shift_mult: rare fire, no isolated test coverage
    //   - stack_competing_mult: redundant with competing_mult + negative_stack_prior
    //   - content_analysis_mult: falls back to 1.0 on cache miss, expensive

    // Source tier authority: slight scoring adjustment by source classification
    let tier = crate::source_tiers::SourceTier::default_for_source(input.source_type);
    let tier_authority_mult = tier.authority_multiplier();

    // Full-strength multipliers — no dampening. Each multiplier expresses its
    // complete signal. The confirmation gate (Phase 7) prevents score inflation;
    // what changes is the FLOOR — bad items drop further, good items stay at ceiling.
    let composite = competing_mult
        * content_quality.multiplier
        * content_dna_mult
        * novelty.multiplier
        * sophistication_mult
        * freshness
        * source_quality_mult
        * raw.affinity_mult
        * anti_mult
        * domain_quality_mult
        * negative_stack_prior
        * tier_authority_mult;

    let quality_score = (relevance_score * composite).clamp(0.0, 1.0);

    // ── CVE/Security dependency validation (ported from V1) ─────────────
    // Security items about packages NOT in the user's actual dependencies
    // get strongly penalized. Without this, every CVE source item rides the
    // SecurityAdvisory (1.30) + novelty (1.10) multipliers and surfaces at
    // 70-80% regardless of whether the user uses the affected software.
    //
    // Tiers (aligned with V1 pipeline.rs exactly so both pipelines agree):
    //   * no matched deps at all          → 0.35 (hard suppression)
    //   * matched but confidence < 0.20   → 0.60 (mild penalty)
    //   * strong match, ALL transitive    → 0.60 (mild penalty — not urgent)
    //   * strong match, any direct        → unchanged (full strength)
    //
    // The 0.20 threshold is calibrated so a single content-only word-boundary
    // match (0.2 confidence → dep_match_score 0.1) still gets the mild penalty.
    // Only 2+ content matches OR a title match (0.5 confidence → 0.25) survive
    // as a "strong" match. Previously the threshold was 0.10 which let single
    // weak subterm hits (e.g. the word "cert" matching x509-cert in an
    // unrelated AWS advisory) escape the gate entirely.
    //
    // Direct vs transitive: A CVE in `tauri` (direct dep) is urgent — the user
    // chose this dependency. A CVE in `x509-cert` (transitive, via rustls) is
    // background noise. When ALL matched deps are transitive (none direct),
    // apply the mild 0.60 penalty even if dep_match_score >= 0.20.
    //
    // Applies to both explicit CVE source items and any other source whose
    // title/content matches the security classifier — so future security
    // sources are governed by the same gate automatically.
    let all_transitive =
        !raw.matched_deps.is_empty() && raw.matched_deps.iter().all(|d| !d.is_direct);
    let quality_score =
        if novelty.is_security && raw.dep_match_score < scoring_config::SECURITY_DEP_VALIDATION_DEP_CONFIDENCE_THRESHOLD && !raw.matched_deps.is_empty() {
            quality_score * scoring_config::SECURITY_DEP_VALIDATION_WEAK_MATCH_PENALTY
        } else if novelty.is_security && raw.matched_deps.is_empty() {
            quality_score * scoring_config::SECURITY_DEP_VALIDATION_NO_MATCH_PENALTY
        } else if novelty.is_security && all_transitive {
            quality_score * scoring_config::SECURITY_DEP_VALIDATION_WEAK_MATCH_PENALTY
        } else {
            quality_score
        };

    (
        quality_score,
        freshness,
        source_quality_boost,
        competing_mult,
        content_quality.multiplier,
        content_dna_mult,
        content_type,
        novelty.multiplier,
        ecosystem_shift_mult,
        stack_competing_mult,
        sophistication_mult,
        content_analysis_mult,
        negative_stack_prior,
    )
}

// ============================================================================
// Phase 6: Compute boosts — sum, cap, dampen, add
// ============================================================================

/// Returns (boosted_score, dep_boost, intent_boost, window_boost, skill_gap_boost,
///          calibration_correction, matched_window_id, matched_skill_gaps)
#[allow(clippy::type_complexity)]
fn compute_boosts(
    quality_score: f32,
    input: &ScoringInput,
    ctx: &ScoringContext,
    raw: &RawSignals,
) -> (f32, f32, f32, f32, f32, f32, Option<i64>, Vec<String>) {
    // Dependency boost (in bootstrap mode, 2x weight)
    let dep_weight = if ctx.feedback_interaction_count < 10 {
        scoring_config::DEPENDENCY_BOOST_WEIGHT * 2.0
    } else {
        scoring_config::DEPENDENCY_BOOST_WEIGHT
    };
    let dep_boost = raw.dep_match_score * dep_weight;

    // Intent boost: amplify items matching recent work topics
    let intent_boost: f32 = if ctx.work_topics.is_empty() {
        0.0
    } else {
        let matching_work_topics = raw
            .topics
            .iter()
            .filter(|t| ctx.work_topics.iter().any(|wt| topic_overlaps(t, wt)))
            .count();
        match matching_work_topics {
            0 => 0.0,
            1 => scoring_config::INTENT_BOOST_SINGLE_MATCH,
            _ => scoring_config::INTENT_BOOST_MULTI_MATCH,
        }
    };

    // Decision window boost
    let (window_boost, matched_window_id) = if ctx.open_windows.is_empty() {
        (0.0, None)
    } else {
        crate::decision_advantage::compute_decision_window_boost(
            &ctx.open_windows,
            input.title,
            input.content,
            &raw.topics,
            &raw.matched_deps
                .iter()
                .map(|d| d.package_name.clone())
                .collect::<Vec<_>>(),
        )
    };

    // Skill-gap boost
    let mut matched_skill_gaps: Vec<String> = Vec::new();
    let skill_gap_boost: f32 = if let Some(ref profile) = ctx.sovereign_profile {
        if profile.intelligence.skill_gaps.is_empty() {
            0.0
        } else {
            for t in &raw.topics {
                if let Some(g) = profile
                    .intelligence
                    .skill_gaps
                    .iter()
                    .find(|g| topic_overlaps(t, &g.dependency))
                {
                    if !matched_skill_gaps.contains(&g.dependency) {
                        matched_skill_gaps.push(g.dependency.clone());
                    }
                }
            }
            match matched_skill_gaps.len() {
                0 => 0.0,
                1 => 0.15,
                _ => 0.20,
            }
        }
    } else {
        0.0
    };

    // Autophagy calibration correction
    let calibration_correction: f32 =
        if !ctx.calibration_deltas.is_empty() && !raw.topics.is_empty() {
            let matching: Vec<f32> = raw
                .topics
                .iter()
                .filter_map(|t| ctx.calibration_deltas.get(t.as_str()).copied())
                .collect();
            if matching.is_empty() {
                0.0
            } else {
                let avg_delta = matching.iter().sum::<f32>() / matching.len() as f32;
                avg_delta.clamp(-0.10, 0.10)
            }
        } else {
            0.0
        };

    // Anti-pattern correction from autophagy bias detection
    let anti_pattern_correction = ctx
        .anti_pattern_penalties
        .get(input.source_type)
        .copied()
        .unwrap_or(0.0)
        .clamp(-0.10, 0.10);

    // TitanCA-inspired archetype penalty: recurring dismissal patterns get penalized
    let archetype_penalty = crate::autophagy::archetype_penalty_for_item(
        &ctx.archetype_penalties,
        input.source_type,
        input.title,
        None,
    );

    // Sum all boosts -> cap -> dampen -> add
    let total_raw = dep_boost
        + raw.stack_boost
        + intent_boost
        + raw.feedback_boost
        + window_boost
        + skill_gap_boost
        + calibration_correction
        + anti_pattern_correction
        - archetype_penalty
        + raw.taste_boost;

    let total_capped = total_raw.clamp(scoring_config::BOOST_CLAMP_MIN, scoring_config::BOOST_CLAMP_MAX);

    let total_dampened = if total_capped < 0.0 {
        total_capped * scoring_config::DAMPENING_PENALTY_STRENGTH
    } else {
        total_capped * scoring_config::DAMPENING_BOOST_STRENGTH
    };

    let boosted = (quality_score + total_dampened).clamp(0.0, 1.0);

    (
        boosted,
        dep_boost,
        intent_boost,
        window_boost,
        skill_gap_boost,
        calibration_correction,
        matched_window_id,
        matched_skill_gaps,
    )
}

// ============================================================================
// Phase 7: Apply gate effect — confidence multiplier + domain gate + ceiling LAST
// ============================================================================

fn apply_gate_effect(
    score: f32,
    signal_count: u8,
    domain_relevance: f32,
    ctx: &ScoringContext,
    strength_bonus: f32,
) -> f32 {
    let idx = (signal_count as usize).min(5);
    let (conf_mult, base_ceiling) = scoring_config::CONFIRMATION_GATE[idx];
    // Adjust ceiling based on signal strength — strong signals get higher ceiling.
    // This creates sub-ranking within gate tiers: strong 2-signal items at ~0.73
    // are clearly differentiated from weak 2-signal items capped at 0.65.
    let score_ceiling = (base_ceiling + strength_bonus).min(1.0);

    let gated = score * conf_mult;

    // Domain gate: same ramp as V1
    let domain_gate_mult = if domain_relevance >= 1.0 && !ctx.domain_profile.is_empty() {
        scoring_config::DOMAIN_GATE_PRIMARY_BOOST
    } else if domain_relevance >= 0.85 {
        1.0
    } else if domain_relevance >= 0.50 {
        let gap = 1.0 - scoring_config::DOMAIN_GATE_RAMP_BASE;
        scoring_config::DOMAIN_GATE_RAMP_BASE + (domain_relevance - 0.50) * (gap / 0.35)
    } else {
        scoring_config::DOMAIN_GATE_OFF_DOMAIN_MULT
    };

    // Score ceiling applied LAST — domain boost cannot push above gate ceiling.
    // Hard cap at 0.95: no item should display 100% — that implies perfect
    // certainty which no heuristic pipeline can guarantee.
    (gated * domain_gate_mult)
        .min(score_ceiling)
        .clamp(0.0, scoring_config::FINAL_CEILING_ABSOLUTE_MAX)
}

// ============================================================================
// Phase 8: Apply final adjustments — short title cap only
// ============================================================================

fn apply_final_adjustments(score: f32, title: &str) -> f32 {
    let meaningful_words = title.split_whitespace().filter(|w| w.len() >= 2).count();
    if meaningful_words < 3 {
        score.min(scoring_config::QUALITY_FLOOR_SHORT_TITLE_CAP)
    } else {
        score
    }
}

// ============================================================================
// Signal classification (mirrors V1 logic)
// ============================================================================

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn classify_signals(
    relevant: bool,
    combined_score: f32,
    domain_relevance: f32,
    content_type: &crate::content_dna::ContentType,
    options: &ScoringOptions,
    classifier: Option<&signals::SignalClassifier>,
    input: &ScoringInput,
    ctx: &ScoringContext,
    matched_deps: &[dependencies::DepMatch],
    db: &Database,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<Vec<String>>,
    Option<String>,
) {
    let show_and_tell_blocked =
        *content_type == crate::content_dna::ContentType::ShowAndTell && domain_relevance < 1.0;

    // Security advisories and breaking changes with STRONG dependency matches
    // bypass the domain_relevance gate — a CVE in your deps is urgent regardless
    // of how "on-domain" the advisory text appears.
    //
    // Requires a non-dev dep match with confidence >= 0.15 so a single weak
    // word-boundary hit cannot escalate an unrelated CVE into a Critical signal.
    let has_strong_non_dev_match = matched_deps
        .iter()
        .any(|d| !d.is_dev && d.confidence >= 0.15);
    let is_critical_content = (*content_type == crate::content_dna::ContentType::SecurityAdvisory
        || *content_type == crate::content_dna::ContentType::BreakingChange)
        && has_strong_non_dev_match;

    if !is_critical_content
        && !(options.apply_signals
            && relevant
            && combined_score >= 0.30
            && domain_relevance >= 0.70
            && !show_and_tell_blocked)
    {
        return (None, None, None, None, None);
    }

    let Some(clf) = classifier else {
        return (None, None, None, None, None);
    };

    let topics = crate::extract_topics(input.title, input.content, input.source_tags);
    let corroboration = super::pipeline::build_corroboration(db, &topics, matched_deps);
    match clf.classify(
        input.title,
        input.content,
        combined_score,
        &ctx.declared_tech,
        &ctx.ace_ctx.detected_tech,
        &corroboration,
    ) {
        Some(mut c) => {
            // Dependency-aware priority escalation
            // Require HIGH confidence match (>= 0.40) for Critical — this means either
            // the full package name matched or 2+ specific subterms confirmed.
            // Prevents single-subterm matches (e.g. "react" from "sentry-react") from
            // triggering misleading Critical alerts.
            if !matched_deps.is_empty() {
                let has_strong_dep = matched_deps
                    .iter()
                    .any(|d| !d.is_dev && d.confidence >= 0.40);
                if c.signal_type == signals::SignalType::SecurityAlert && has_strong_dep {
                    c.priority = signals::SignalPriority::Critical;
                    // Use the highest-confidence match for the alert name
                    let best_dep = matched_deps
                        .iter()
                        .filter(|d| !d.is_dev)
                        .max_by(|a, b| {
                            a.confidence
                                .partial_cmp(&b.confidence)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .unwrap_or(&matched_deps[0]);
                    c.action = format!(
                        "Critical: Security issue affects your dependency {}",
                        best_dep.package_name
                    );
                } else if c.signal_type == signals::SignalType::BreakingChange
                    && matched_deps
                        .iter()
                        .any(|d| d.version_delta == dependencies::VersionDelta::NewerMajor)
                    && c.priority < signals::SignalPriority::Alert
                {
                    c.priority = signals::SignalPriority::Alert;
                }
                for dep in matched_deps.iter().take(2) {
                    c.triggers.push(format!("dep:{}", dep.package_name));
                }
            }

            // Score-aware priority cap
            if combined_score < scoring_config::LOW_SCORE_CAP
                && c.priority > signals::SignalPriority::Watch
            {
                c.priority = signals::SignalPriority::Watch;
            } else if (combined_score < scoring_config::MEDIUM_SCORE_CAP
                && c.priority > signals::SignalPriority::Advisory)
                || (combined_score > scoring_config::HIGH_SCORE_FLOOR
                    && c.priority < signals::SignalPriority::Advisory)
            {
                c.priority = signals::SignalPriority::Advisory;
            }

            // TRUST GATE: Critical requires verified dependency evidence.
            // If signal classifier set Critical but there's no strong direct dep match, downgrade.
            if c.priority == signals::SignalPriority::Critical {
                let has_strong_direct_dep = matched_deps
                    .iter()
                    .any(|d| !d.is_dev && d.confidence >= 0.40 && d.is_direct);
                if !has_strong_direct_dep {
                    c.priority = signals::SignalPriority::Alert;
                    if matched_deps.is_empty() {
                        c.action = format!("Ecosystem watch: {}", input.title);
                    }
                }
            }

            (
                Some(c.signal_type.slug().to_string()),
                Some(c.priority.label().to_string()),
                Some(c.action),
                Some(c.triggers),
                Some(c.horizon.label().to_string()),
            )
        }
        None => (None, None, None, None, None),
    }
}

// ============================================================================
// Main entry point — identical public signature to V1
// ============================================================================

/// Score a single item through the PASIFA V2 pipeline (8-phase architecture).
/// Returns SourceRelevance with all fields populated — drop-in replacement for V1.
pub(crate) fn score_item(
    input: &ScoringInput,
    ctx: &ScoringContext,
    db: &Database,
    options: &ScoringOptions,
    classifier: Option<&signals::SignalClassifier>,
) -> SourceRelevance {
    let topics = extract_topics(input.title, input.content, input.source_tags);

    // ── Exclusion check (before any scoring work) ──────────────────────
    let excluded_by = check_exclusions(&topics, &ctx.exclusions)
        .or_else(|| check_ace_exclusions(&topics, &ctx.ace_ctx));

    if let Some(exclusion) = excluded_by {
        return SourceRelevance {
            id: input.id,
            title: input.title.to_string(),
            url: input.url.map(std::string::ToString::to_string),
            top_score: 0.0,
            matches: vec![],
            relevant: false,
            context_score: 0.0,
            interest_score: 0.0,
            excluded: true,
            excluded_by: Some(exclusion),
            source_type: input.source_type.to_string(),
            explanation: None,
            confidence: Some(0.0),
            score_breakdown: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
            streets_engine: None,
            decision_window_match: None,
            decision_boost_applied: 0.0,
            created_at: None,
            detected_lang: input.detected_lang.to_string(),
            is_critical_alert: false,
            applicability: None,
            advisory_id: None,
        };
    }

    // ── KNN context search (needed for Phase 1 and final output) ──────
    let matches: Vec<RelevanceMatch> =
        if ctx.cached_context_count > 0 && !input.embedding.is_empty() {
            db.find_similar_contexts(input.embedding, 3)
                .unwrap_or_default()
                .into_iter()
                .map(|result| {
                    let similarity = 1.0 / (1.0 + result.distance);
                    let matched_text = if result.text.len() > 100 {
                        let truncated: String = result.text.chars().take(100).collect();
                        format!("{truncated}...")
                    } else {
                        result.text
                    };
                    RelevanceMatch {
                        source_file: result.source_file,
                        matched_text,
                        similarity,
                    }
                })
                .collect()
        } else {
            vec![]
        };

    // ── Phase 1: Extract all raw signals ──────────────────────────────
    let raw = extract_signals(input, ctx, &matches);

    // ── Phase 2: Calibrate ────────────────────────────────────────────
    let cal = calibrate_signals(&raw);

    // ── Phase 3: Gate count on clean signals ──────────────────────────
    let confirmation = gate::count_confirmed_signals(
        cal.context_score,
        cal.interest_score,
        cal.keyword_score,
        cal.semantic_boost,
        &ctx.ace_ctx,
        &raw.topics,
        raw.feedback_boost,
        raw.affinity_mult,
        raw.dep_match_score,
        raw.stack_pain_match,
        raw.specificity_weight,
    );
    let signal_count = confirmation.count;
    let confirmed_signals = confirmation.confirmed_names();

    // ── Phase 4: Compute base relevance ───────────────────────────────
    let has_real_embedding = input.embedding.iter().any(|&v| v != 0.0);
    let relevance_score = compute_relevance(&cal, ctx, has_real_embedding);

    // ── Phase 5: Quality composite ────────────────────────────────────
    let (
        quality_score,
        freshness,
        source_quality_boost,
        competing_mult,
        content_quality_mult,
        content_dna_mult,
        content_type,
        novelty_mult,
        ecosystem_shift_mult,
        stack_competing_mult,
        _sophistication_mult,
        content_analysis_mult,
        negative_stack_prior,
    ) = compute_quality_composite(relevance_score, input, ctx, &raw, options, db);

    // ── Phase 6: Boosts ───────────────────────────────────────────────
    let (
        boosted_score,
        _dep_boost,
        intent_boost,
        window_boost,
        skill_gap_boost,
        _calibration_correction,
        matched_window_id,
        matched_skill_gaps,
    ) = compute_boosts(quality_score, input, ctx, &raw);

    // Trend topic boost (temporal clustering signal)
    let trend_boost = if !options.trend_topics.is_empty()
        && raw.topics.iter().any(|t| options.trend_topics.contains(t))
    {
        0.08
    } else {
        0.0
    };
    let boosted_score = (boosted_score + trend_boost).clamp(0.0, 1.0);

    // ── Signal strength bonus (pre-gate) ─────────────────────────────
    let strength_bonus = compute_signal_strength_bonus(
        signal_count,
        cal.context_score,
        cal.interest_score,
        cal.keyword_score,
        cal.semantic_boost,
        raw.dep_match_score,
        raw.feedback_boost,
        raw.affinity_mult,
        raw.stack_pain_match,
    );

    // ── Phase 7: Gate effect ──────────────────────────────────────────
    let conf_idx = (signal_count as usize).min(5);
    let confirmation_mult = scoring_config::CONFIRMATION_GATE[conf_idx].0;
    let gated_score = apply_gate_effect(
        boosted_score,
        signal_count,
        raw.domain_relevance,
        ctx,
        strength_bonus,
    );

    // ── Phase 8: Final adjustments ────────────────────────────────────
    let combined_score = apply_final_adjustments(gated_score, input.title);

    // ── Critical content fast-path ─────────────────────────────────────
    // Security advisories and breaking changes affecting user's actual
    // dependencies ALWAYS surface, regardless of relevance score.
    // This prevents the gate from silently dropping critical alerts.
    //
    // IMPORTANT: the dep match must be strong AND touch a non-dev dep.
    // `dep_match_score > 0.0` is too loose — a single stray word-boundary
    // hit (e.g. the word "hostname" in an unrelated CVE) would trigger the
    // floor and surface CVEs that have nothing to do with the user's stack.
    let is_security = content_type == crate::content_dna::ContentType::SecurityAdvisory;
    let is_breaking = content_type == crate::content_dna::ContentType::BreakingChange;
    let has_strong_dep_match =
        raw.dep_match_score >= scoring_config::CRITICAL_FASTPATH_DEP_MATCH_THRESHOLD && raw.matched_deps.iter().any(|d| !d.is_dev);
    let critical_fast_path = (is_security || is_breaking) && has_strong_dep_match;

    // If critical fast-path, boost score to ensure it passes the gate
    let combined_score = if critical_fast_path && combined_score < scoring_config::CRITICAL_FASTPATH_SCORE_FLOOR {
        combined_score.max(scoring_config::CRITICAL_FASTPATH_SCORE_FLOOR) // Floor for security items matching deps
    } else {
        combined_score
    };

    // ── Relevance determination ───────────────────────────────────────
    let bootstrap_mode = ctx.feedback_interaction_count < 10;
    let min_signals = if bootstrap_mode {
        1u8
    } else {
        scoring_config::QUALITY_FLOOR_MIN_SIGNALS as u8
    };
    let relevant = critical_fast_path  // Critical items always relevant
        || (combined_score >= get_relevance_threshold()
            && (signal_count >= min_signals
                || combined_score >= scoring_config::QUALITY_FLOOR_MIN_SCORE));

    // ── Explanation ───────────────────────────────────────────────────
    let explanation = if relevant || combined_score >= 0.3 {
        Some(generate_relevance_explanation(
            input.title,
            cal.context_score,
            cal.interest_score,
            &matches,
            &ctx.ace_ctx,
            &raw.topics,
            &ctx.interests,
            &ctx.declared_tech,
            &matched_skill_gaps,
        ))
    } else {
        None
    };

    // ── Confidence ────────────────────────────────────────────────────
    let confidence = calculate_confidence(
        cal.context_score,
        cal.interest_score,
        cal.semantic_boost,
        &ctx.ace_ctx,
        &raw.topics,
        ctx.cached_context_count,
        ctx.interest_count as i64,
        signal_count,
    );

    // ── Confidence by signal map ──────────────────────────────────────
    let mut confidence_by_signal = HashMap::new();
    if ctx.cached_context_count > 0 {
        confidence_by_signal.insert("context".to_string(), cal.context_score);
    }
    if ctx.interest_count > 0 {
        confidence_by_signal.insert("interest".to_string(), cal.interest_score);
    }
    if cal.semantic_boost > 0.0 {
        confidence_by_signal.insert("ace_boost".to_string(), cal.semantic_boost);
    }
    if raw.dep_match_score > 0.0 {
        confidence_by_signal.insert("dependency".to_string(), raw.dep_match_score);
    }

    // ── Matched dependency names ──────────────────────────────────────
    let matched_dep_names: Vec<String> = raw
        .matched_deps
        .iter()
        .map(|d| d.package_name.clone())
        .collect();

    // ── Signal classification ─────────────────────────────────────────
    let (sig_type, sig_priority, sig_action, sig_triggers, sig_horizon) = classify_signals(
        relevant,
        combined_score,
        raw.domain_relevance,
        &content_type,
        options,
        classifier,
        input,
        ctx,
        &raw.matched_deps,
        db,
    );

    // ── Necessity scoring ─────────────────────────────────────────────
    let age_hours = input.created_at.map_or(0.0, |ts| {
        (chrono::Utc::now() - *ts).num_minutes().max(0) as f64 / 60.0
    });

    // Contradiction boost: check if item topics overlap with contradicted topics
    let contradiction_boost = if ctx.contradicted_topics.is_empty() {
        0.0
    } else {
        let overlap_count = raw
            .topics
            .iter()
            .filter(|t| ctx.contradicted_topics.contains(t.as_str()))
            .count();
        // Normalize: 1 match = 0.5, 2+ = 1.0
        match overlap_count {
            0 => 0.0,
            1 => 0.5,
            _ => 1.0,
        }
    };

    let necessity_inputs = necessity::NecessityInputs {
        dep_match_score: raw.dep_match_score,
        matched_deps: matched_dep_names.clone(),
        signal_type: sig_type.clone(),
        signal_priority: sig_priority.clone(),
        cve_severity: None, // CVE severity is folded into signal_priority by the classifier
        cvss_score: None,   // Not directly available at this pipeline stage
        affected_project_count: count_affected_projects(db, &matched_dep_names),
        skill_gap_boost,
        matched_skill_gaps: matched_skill_gaps.clone(),
        window_boost,
        age_hours,
        content_type: Some(content_type.slug().to_string()),
        contradiction_boost,
    };
    let mut necessity_result = necessity::compute_necessity(&necessity_inputs);

    // ── Source authority weighting for necessity ───────────────────────
    // Security items are NOT penalized — a CVE is critical regardless of source.
    // All other necessity categories are modulated by source authority.
    if necessity_result.category != necessity::NecessityCategory::SecurityVulnerability
        && necessity_result.score > 0.0
    {
        let authority = authority::source_authority(input.source_type);
        necessity_result.score = (necessity_result.score * authority).clamp(0.0, 1.0);
    }

    // ── Security applicability + critical alert gate ────────────────────
    let is_security_source = matches!(input.source_type, "cve" | "osv");
    let (applicability, is_critical_alert) = if sig_type.as_deref() == Some("security_alert") {
        // Ecosystem-verified strong dep: confidence >= 0.40, not dev, AND
        // either the dep has no ecosystem info (can't reject) or the advisory's
        // affected packages include the dep's ecosystem.
        let advisory_ecosystems = extract_advisory_ecosystems(input.content);
        let has_strong_dep = raw.matched_deps.iter().any(|d| {
            if d.is_dev || d.confidence < 0.40 {
                return false;
            }
            if d.ecosystem.is_empty() || advisory_ecosystems.is_empty() {
                return true; // can't verify ecosystem → allow (conservative)
            }
            let dep_eco = normalize_ecosystem(&d.ecosystem);
            advisory_ecosystems
                .iter()
                .any(|(_, eco)| normalize_ecosystem(eco) == dep_eco)
        });
        let has_any_dep = !raw.matched_deps.is_empty();
        let all_dev = raw.matched_deps.iter().all(|d| d.is_dev);
        let all_transitive = raw.matched_deps.iter().all(|d| !d.is_direct);

        if has_strong_dep {
            if all_transitive {
                (Some("likely_affected".to_string()), false)
            } else {
                (Some("affected".to_string()), true)
            }
        } else if has_any_dep && !all_dev {
            (Some("likely_affected".to_string()), false)
        } else if all_dev && has_any_dep {
            (Some("likely_affected".to_string()), false)
        } else {
            (Some("needs_verification".to_string()), false)
        }
    } else {
        (None, false)
    };

    // ── Security evidence extraction ─────────────────────────────────
    let (cvss_score, cvss_severity) = if is_security_source {
        extract_cvss_from_content(input.content)
    } else {
        (None, None)
    };
    let advisory_id = if is_security_source {
        extract_advisory_id(input.title)
    } else {
        None
    };
    let fixed_version = if is_security_source {
        extract_fixed_version(input.content)
    } else {
        None
    };
    let affected_versions = if is_security_source {
        extract_affected_range(input.content)
    } else {
        None
    };
    let dep_path = if !raw.matched_deps.is_empty() {
        let dep = &raw.matched_deps[0];
        Some(if dep.is_dev {
            "dev-only".to_string()
        } else if !dep.is_direct {
            "transitive".to_string()
        } else {
            "direct".to_string()
        })
    } else {
        None
    };
    let installed_version = raw.matched_deps.first().and_then(|d| d.version.clone());
    let is_version_affected = check_version_affected(
        installed_version.as_deref(),
        affected_versions.as_deref(),
        fixed_version.as_deref(),
    );
    let sec_affected_project_count = count_affected_projects(db, &matched_dep_names) as u32;

    // ── Score breakdown ───────────────────────────────────────────────
    let score_breakdown = ScoreBreakdown {
        context_score: cal.context_score,
        interest_score: cal.interest_score,
        keyword_score: cal.keyword_score,
        ace_boost: cal.semantic_boost,
        affinity_mult: raw.affinity_mult,
        anti_penalty: raw.anti_penalty,
        freshness_mult: freshness,
        feedback_boost: raw.feedback_boost,
        source_quality_boost,
        confidence_by_signal,
        signal_count,
        confirmed_signals: confirmed_signals.clone(),
        confirmation_mult,
        dep_match_score: raw.dep_match_score,
        matched_deps: matched_dep_names,
        domain_relevance: raw.domain_relevance,
        content_quality_mult,
        novelty_mult,
        intent_boost,
        content_type: Some(content_type.slug().to_string()),
        content_dna_mult,
        competing_mult,
        stack_boost: raw.stack_boost,
        ecosystem_shift_mult,
        stack_competing_mult,
        llm_score: None,
        llm_reason: None,
        window_boost,
        matched_window_id,
        skill_gap_boost,
        necessity_score: necessity_result.score,
        necessity_reason: if necessity_result.score > 0.0 {
            Some(necessity_result.reason)
        } else {
            None
        },
        necessity_category: if necessity_result.score > 0.0 {
            Some(necessity_result.category.slug().to_string())
        } else {
            None
        },
        necessity_urgency: if necessity_result.score > 0.0 {
            Some(necessity_result.urgency.label().to_string())
        } else {
            None
        },
        signal_strength_bonus: strength_bonus,
        content_analysis_mult,
        advisor_signals: Vec::new(),
        disagreement: None,
        advisory_source: if is_security_source {
            Some(
                if input.source_type == "osv" {
                    "OSV"
                } else {
                    "GHSA"
                }
                .to_string(),
            )
        } else {
            None
        },
        cvss_score,
        cvss_severity,
        affected_versions,
        fixed_version,
        installed_version: installed_version.clone(),
        is_version_affected,
        dependency_path: dep_path.clone(),
        affected_project_count: Some(sec_affected_project_count),
        negative_stack_prior,
    };

    // ── STREETS revenue engine mapping ────────────────────────────────
    let streets_engine = if relevant {
        crate::streets_engine::map_to_streets_engine(
            input.title,
            input.content,
            Some(content_type.slug()),
            sig_type.as_deref(),
        )
    } else {
        None
    };

    // ── Build final result ────────────────────────────────────────────
    SourceRelevance {
        id: input.id,
        title: crate::decode_html_entities(input.title),
        url: input.url.map(std::string::ToString::to_string),
        top_score: combined_score,
        matches,
        relevant,
        context_score: cal.context_score,
        interest_score: cal.interest_score,
        excluded: false,
        excluded_by: None,
        source_type: input.source_type.to_string(),
        explanation,
        confidence: Some(confidence),
        score_breakdown: Some(score_breakdown),
        signal_type: sig_type,
        signal_priority: sig_priority,
        signal_action: sig_action,
        signal_triggers: sig_triggers,
        signal_horizon: sig_horizon,
        similar_count: 0,
        similar_titles: vec![],
        serendipity: false,
        streets_engine,
        decision_window_match: matched_window_id.and_then(|wid| {
            ctx.open_windows
                .iter()
                .find(|w| w.id == wid)
                .map(|w| w.title.clone())
        }),
        decision_boost_applied: window_boost,
        created_at: None,
        detected_lang: input.detected_lang.to_string(),
        is_critical_alert,
        applicability,
        advisory_id,
    }
}

/// Count how many distinct projects use any of the matched dependencies.
/// Returns 0 if no deps matched or the DB query fails (graceful degradation).
fn count_affected_projects(db: &Database, matched_deps: &[String]) -> usize {
    if matched_deps.is_empty() {
        return 0;
    }
    let conn = db.conn.lock();
    // Count distinct projects that have ANY of the matched deps
    let placeholders: String = matched_deps
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT COUNT(DISTINCT project_path) FROM project_dependencies WHERE LOWER(package_name) IN ({})",
        placeholders
    );
    let params: Vec<String> = matched_deps.iter().map(|d| d.to_lowercase()).collect();
    conn.query_row(
        &sql,
        rusqlite::params_from_iter(params.iter()),
        |row: &rusqlite::Row<'_>| row.get(0),
    )
    .unwrap_or(0)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_security_metadata_with_severity_block() {
        let content = "A critical deserialization vulnerability was discovered.\n\nSeverity: HIGH\nAffected: lodash (npm), wildcard-match (npm)\nCVSS: 9.8";
        let stripped = strip_security_metadata(content);
        assert_eq!(
            stripped,
            "A critical deserialization vulnerability was discovered."
        );
        assert!(!stripped.contains("Affected"));
        assert!(!stripped.contains("lodash"));
        assert!(!stripped.contains("wildcard-match"));
    }

    #[test]
    fn test_strip_security_metadata_without_marker() {
        // When there's no Severity marker, content is returned as-is
        let content = "Just a regular blog post about lodash performance";
        let stripped = strip_security_metadata(content);
        assert_eq!(stripped, content);
    }

    #[test]
    fn test_strip_security_metadata_empty() {
        assert_eq!(strip_security_metadata(""), "");
    }

    #[test]
    fn test_strip_security_metadata_affected_line_after_severity() {
        // Realistic OSV-style content
        let content = "Buffer overflow in TLS handshake.\n\nSeverity: CRITICAL\nAffected: openssl (c), hostname (rust), aws-lc-rs (rust)\nFixed in: 3.2.0\nDetails about the bug.";
        let stripped = strip_security_metadata(content);
        // Everything after the description should be stripped — including the
        // Affected line that contains the hostname/aws-lc-rs noise.
        assert_eq!(stripped, "Buffer overflow in TLS handshake.");
        assert!(!stripped.contains("hostname"));
        assert!(!stripped.contains("aws-lc-rs"));
    }

    #[test]
    fn test_dep_match_content_for_cve() {
        let input = ScoringInput {
            id: 1,
            title: "CVE-2024-1234",
            url: None,
            content: "desc text.\n\nSeverity: HIGH\nAffected: hostname (rust)\n",
            source_type: "cve",
            embedding: &[],
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
        };
        let cleaned = dep_match_content_for(&input);
        assert_eq!(cleaned, "desc text.");
        assert!(!cleaned.contains("hostname"));
    }

    #[test]
    fn test_dep_match_content_for_osv() {
        let input = ScoringInput {
            id: 1,
            title: "OSV ID",
            url: None,
            content: "summary.\n\nSeverity: HIGH\nAffected: x509-cert (rust)\n",
            source_type: "osv",
            embedding: &[],
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
        };
        let cleaned = dep_match_content_for(&input);
        assert_eq!(cleaned, "summary.");
    }

    #[test]
    fn test_dep_match_content_for_non_security_source() {
        let input = ScoringInput {
            id: 1,
            title: "HN Post",
            url: None,
            content: "lodash released 5.0 with breaking changes.\n\nSeverity: isn't a marker here",
            source_type: "hackernews",
            embedding: &[],
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
        };
        // Non-security source content is passed through verbatim
        let cleaned = dep_match_content_for(&input);
        assert_eq!(cleaned, input.content);
    }

    #[test]
    fn check_version_affected_with_fixed() {
        assert_eq!(
            check_version_affected(Some("2.8.1"), None, Some("3.0.1")),
            Some(true),
        );
        assert_eq!(
            check_version_affected(Some("3.1.0"), None, Some("3.0.1")),
            Some(false),
        );
    }

    #[test]
    fn check_version_affected_with_range() {
        assert_eq!(
            check_version_affected(Some("2.8.1"), Some("< 3.0.0"), None),
            Some(true),
        );
        assert_eq!(
            check_version_affected(Some("3.0.0"), Some("< 3.0.0"), None),
            Some(false),
        );
    }

    #[test]
    fn check_version_affected_none_inputs() {
        assert_eq!(check_version_affected(None, None, None), None);
        assert_eq!(check_version_affected(Some("1.0.0"), None, None), None);
    }

    #[test]
    fn check_version_affected_fixed_takes_precedence() {
        assert_eq!(
            check_version_affected(Some("2.0.0"), Some(">= 3.0.0"), Some("2.5.0")),
            Some(true),
        );
    }

    // ========================================================================
    // Ecosystem cross-reference tests
    // ========================================================================

    #[test]
    fn test_extract_advisory_ecosystems_standard() {
        let content = "SSRF vulnerability.\n\nSeverity: HIGH\nAffected: lmdeploy (pip)\nCVSS: 7.5";
        let result = extract_advisory_ecosystems(content);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "lmdeploy");
        assert_eq!(result[0].1, "pip");
    }

    #[test]
    fn test_extract_advisory_ecosystems_multiple() {
        let content = "Vuln.\n\nSeverity: HIGH\nAffected: lodash (npm), express (npm)\nCVSS: 9.8";
        let result = extract_advisory_ecosystems(content);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, "npm");
        assert_eq!(result[1].0, "express");
    }

    #[test]
    fn test_extract_advisory_ecosystems_maven() {
        let content =
            "Crypto vuln.\n\nSeverity: HIGH\nAffected: org.bouncycastle:bcpkix-jdk18on (maven)";
        let result = extract_advisory_ecosystems(content);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "org.bouncycastle:bcpkix-jdk18on");
        assert_eq!(result[0].1, "maven");
    }

    #[test]
    fn test_extract_advisory_ecosystems_no_affected_line() {
        let content = "Just a description with no metadata";
        let result = extract_advisory_ecosystems(content);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_advisory_ecosystems_empty() {
        let result = extract_advisory_ecosystems("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_ecosystem_mismatch_rejects_maven_vs_rust() {
        // Bouncy Castle (maven) should NOT match a Rust "crypto" dep
        let content =
            "Crypto vuln.\n\nSeverity: HIGH\nAffected: org.bouncycastle:bcpkix-jdk18on (maven)";
        let ecosystems = extract_advisory_ecosystems(content);
        assert!(!ecosystems.is_empty());

        let dep_ecosystem = "rust";
        let dep_eco_normalized = normalize_ecosystem(dep_ecosystem);
        let matches = ecosystems
            .iter()
            .any(|(_, eco)| normalize_ecosystem(eco) == dep_eco_normalized);
        assert!(!matches, "Maven CVE should not match a Rust dependency");
    }

    #[test]
    fn test_ecosystem_match_rust_to_rust() {
        let content = "Buffer overflow.\n\nSeverity: HIGH\nAffected: tokio (rust)";
        let ecosystems = extract_advisory_ecosystems(content);

        let dep_ecosystem = "rust";
        let dep_eco_normalized = normalize_ecosystem(dep_ecosystem);
        let matches = ecosystems
            .iter()
            .any(|(_, eco)| normalize_ecosystem(eco) == dep_eco_normalized);
        assert!(matches, "Rust CVE should match a Rust dependency");
    }

    #[test]
    fn test_ecosystem_mismatch_rejects_pip_vs_rust() {
        // LMDeploy (pip/python) should NOT match Rust "image" dep
        let content = "SSRF via Image Loading.\n\nSeverity: HIGH\nAffected: lmdeploy (pip)";
        let ecosystems = extract_advisory_ecosystems(content);

        let dep_ecosystem = "rust";
        let dep_eco_normalized = normalize_ecosystem(dep_ecosystem);
        let matches = ecosystems
            .iter()
            .any(|(_, eco)| normalize_ecosystem(eco) == dep_eco_normalized);
        assert!(
            !matches,
            "Python/pip CVE should not match a Rust dependency"
        );
    }
}
