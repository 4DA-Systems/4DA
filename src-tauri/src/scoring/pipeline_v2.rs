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

use crate::sources::cve_matching::normalize_ecosystem;

use super::dependencies::DepMatch;
use super::pipeline::{ScoringInput, ScoringOptions};
use super::*;

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
            let excess = (semantic_boost - scoring_config::SEMANTIC_THRESHOLD)
                / scoring_config::SIGNAL_NORMALIZATION_SEMANTIC_RANGE;
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
            let excess = (affinity_mult - scoring_config::AFFINITY_THRESHOLD)
                / scoring_config::SIGNAL_NORMALIZATION_AFFINITY_RANGE;
            strengths.push(excess.clamp(0.0, 1.0));
        } else {
            strengths.push(scoring_config::SIGNAL_NORMALIZATION_FEEDBACK_STRENGTH);
            // Feedback is less granular
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

fn normalize_advisory_package_name(name: &str) -> String {
    name.trim()
        .trim_start_matches('@')
        .replace(['/', '_'], "-")
        .to_lowercase()
}

fn advisory_affects_dependency(advisory_affected: &[(String, String)], dep: &DepMatch) -> bool {
    let dep_name = normalize_advisory_package_name(&dep.package_name);
    let dep_eco = normalize_ecosystem(&dep.ecosystem);

    advisory_affected.iter().any(|(pkg, eco)| {
        normalize_advisory_package_name(pkg) == dep_name
            && (dep.ecosystem.is_empty() || normalize_ecosystem(eco) == dep_eco)
    })
}

/// Dependency-match score for a CVE/OSV advisory after the strict survivor
/// filter has run.
///
/// A security advisory names ONE affected package, so a confirmed match against
/// a DIRECT dependency is full evidence and must not be halved. The old
/// `total / 2.0` pinned a single-direct-dep CVE at ~0.375 — just below the 0.40
/// threshold that unlocks the full SecurityAdvisory content boost (see the
/// `content_dna_mult` gate in compute_quality_composite) — so the CVE only got
/// the partial 1.10 boost and floored at the bare 0.50 critical fast-path floor.
/// A CVE for the user's own direct dependency is the flagship preemption case;
/// it should score high, not sit at the floor.
///
/// Summed confidence still rewards multiple corroborating matches; the strongest
/// DIRECT-dependency confidence sets the floor. Transitive-only matches keep the
/// old conservative halved score (a `x509-cert`-via-rustls CVE stays background).
fn cve_dep_match_score(deps: &[DepMatch]) -> f32 {
    let summed = (deps.iter().map(|d| d.confidence).sum::<f32>() / 2.0).min(1.0);
    let direct_max = deps
        .iter()
        .filter(|d| d.is_direct)
        .map(|d| d.confidence)
        .fold(0.0_f32, f32::max);
    summed.max(direct_max).min(1.0)
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

    // Profile-aware specificity: broad terms that ARE the user's detected
    // domain (e.g. "ml" for an ML engineer) keep full weight.
    let spec_profile = super::calibration::SpecificityProfile::from_ctx(ctx);

    // Raw interest: embedding similarity against declared interests
    let raw_interest = super::calibration::compute_interest_score_for(
        input.embedding,
        &ctx.interests,
        Some(&spec_profile),
    );

    // Keyword interest matching: boosts items containing declared interest terms
    let raw_keyword_score =
        keywords::compute_keyword_interest_score(input.title, input.content, &ctx.interests);
    let specificity_weight = keywords::best_interest_specificity_weight_for(
        input.title,
        input.content,
        &ctx.interests,
        Some(&spec_profile),
    );
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
                    // Use floor/ceil_char_boundary to avoid panicking on
                    // multi-byte UTF-8 content (e.g. accented researcher
                    // names in CVE descriptions).
                    let start = body_lower.floor_char_boundary(idx.saturating_sub(window));
                    let end = body_lower
                        .ceil_char_boundary((idx + full.len() + window).min(body_lower.len()));
                    let slice = &body_lower[start..end];
                    if CONTEXT_WORDS.iter().any(|w| slice.contains(w)) {
                        return true;
                    }
                }
                false
            });
            // Recompute dep_match_score from the surviving deps. A confirmed
            // direct-dependency match is full evidence for a CVE (see
            // cve_dep_match_score) — do not halve it.
            score = cve_dep_match_score(&deps);
        }

        // Structured advisory cross-reference: when the source adapter gives
        // us affected packages, require the dependency name and ecosystem to
        // match that metadata exactly. Title/body matches alone are not enough
        // to make a CVE applicable.
        if matches!(input.source_type, "cve" | "osv") && !deps.is_empty() {
            let advisory_affected = extract_advisory_ecosystems(input.content);
            if !advisory_affected.is_empty() {
                deps.retain(|d| advisory_affects_dependency(&advisory_affected, d));
                score = cve_dep_match_score(&deps);
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
    let mut domain_relevance =
        crate::domain_profile::compute_domain_relevance(&topics, &ctx.domain_profile);

    // Direct dependencies ARE part of the user's stack — promote to primary
    // so they receive the domain gate boost instead of neutral treatment.
    //
    // EXCEPTION: a match on a UBIQUITOUS framework alone (react, vue, node, ...)
    // is not enough — almost every JS project depends on react, so "Show HN: an
    // AI CAD tool built with React" would otherwise be forced to domain 1.0 and
    // scored CORE despite being completely off-domain. Only override when at
    // least one matched dep is a SPECIFIC (non-ubiquitous) library; if every
    // match is a ubiquitous framework, let the (corrected) topic-based
    // domain_relevance stand so the off-domain penalty can apply.
    if dep_match_score >= 0.50
        && !ctx.domain_profile.is_empty()
        && matched_deps
            .iter()
            .any(|d| !crate::domain_profile::is_ubiquitous_framework(&d.package_name))
    {
        domain_relevance = domain_relevance.max(1.0);
    }

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
            scoring_config::INTEREST_ONLY_SEMANTIC_MULT * 0.7
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
// Community quality signal extraction
// ============================================================================

/// Extract community quality signal from source metadata.
/// Returns 0.0-1.0 where higher = more community validation.
/// Fresh items (< 6 hours) get neutral (0.50) -- the community hasn't voted yet.
/// Items without metadata get neutral (0.50).
fn extract_community_signal(source_type: &str, tags_json: Option<&str>, age_hours: f64) -> f32 {
    if age_hours < 6.0 {
        return 0.50;
    }

    let tags: serde_json::Value = tags_json
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Null);

    match source_type {
        "stackoverflow" => {
            let score = tags.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
            match score {
                s if s >= 50 => 0.90,
                s if s >= 20 => 0.75,
                s if s >= 5 => 0.50,
                _ => 0.20,
            }
        }
        "hackernews" => {
            let points = tags.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
            match points {
                p if p >= 100 => 0.90,
                p if p >= 30 => 0.70,
                p if p >= 10 => 0.50,
                _ => 0.30,
            }
        }
        "reddit" => {
            let score = tags.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
            match score {
                s if s >= 100 => 0.85,
                s if s >= 20 => 0.65,
                s if s >= 5 => 0.50,
                _ => 0.30,
            }
        }
        _ => 0.50,
    }
}

// ============================================================================
// Phase 5: Compute quality composite — ALL multipliers in one pass
// ============================================================================

/// Returns (quality_score, freshness, source_quality_boost, competing_mult, content_quality_mult,
///          content_dna_mult, content_type, novelty_mult, ecosystem_shift_mult, stack_competing_mult,
///          sophistication_mult, content_analysis_mult, negative_stack_prior, sophistication_raw,
///          community_signal)
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
            let autophagy_factor =
                if engagement_rate < scoring_config::SOURCE_ENGAGEMENT_LOW_THRESHOLD {
                    scoring_config::SOURCE_ENGAGEMENT_LOW_PENALTY
                } else if engagement_rate > scoring_config::SOURCE_ENGAGEMENT_HIGH_THRESHOLD {
                    scoring_config::SOURCE_ENGAGEMENT_HIGH_BOOST
                } else {
                    1.0
                };
            (learned_source_mult * scoring_config::SOURCE_ENGAGEMENT_BLEND_LEARNED_WEIGHT
                + autophagy_factor * scoring_config::SOURCE_ENGAGEMENT_BLEND_AUTOPHAGY_WEIGHT)
                .clamp(
                    scoring_config::SOURCE_ENGAGEMENT_BLEND_MIN,
                    scoring_config::SOURCE_ENGAGEMENT_BLEND_MAX,
                )
        } else {
            learned_source_mult
        };

    // Per-feed engagement correction (granular: overrides source-type-level when available)
    let source_quality_mult = if let Some(feed_url) = input.feed_origin {
        if let Some(&feed_rate) = ctx.feed_autopsies.get(feed_url) {
            if feed_rate < scoring_config::SOURCE_ENGAGEMENT_LOW_THRESHOLD {
                source_quality_mult * scoring_config::SOURCE_ENGAGEMENT_LOW_PENALTY
            } else if feed_rate > scoring_config::SOURCE_ENGAGEMENT_HIGH_THRESHOLD {
                source_quality_mult * scoring_config::SOURCE_ENGAGEMENT_HIGH_BOOST
            } else {
                source_quality_mult
            }
        } else {
            source_quality_mult
        }
    } else {
        source_quality_mult
    };

    // Anti-topic multiplier: raw.anti_penalty is used directly in the engagement
    // formula below. This derived value is kept for score breakdown diagnostics.
    #[allow(clippy::no_effect_underscore_binding)]
    let _anti_mult = 1.0 - raw.anti_penalty;

    // Domain quality penalty (NOT dampened — preserves full penalty strength)
    let domain_quality_mult =
        if raw.domain_relevance >= scoring_config::DOMAIN_QUALITY_HIGH_THRESHOLD {
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
        input.content,
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
    let sophistication_raw =
        sophistication.title_complexity * 0.6 + sophistication.content_depth * 0.4;

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
        crate::stacks::negative_stack::lookup_prior(&ctx.ace_ctx.negative_stack, &raw.topics);

    // NOTE: ecosystem_shift_mult, stack_competing_mult, and content_analysis_mult are
    // still computed above for the return tuple (used by logging/diagnostics) but are
    // intentionally excluded from the composite:
    //   - ecosystem_shift_mult: rare fire, no isolated test coverage
    //   - stack_competing_mult: redundant with competing_mult + negative_stack_prior
    //   - content_analysis_mult: falls back to 1.0 on cache miss, expensive

    // Source tier authority: slight scoring adjustment by source classification.
    // Curated feeds override both tier and content_dna_mult from their manifest.
    let curated_manifest = input
        .feed_origin
        .and_then(|url| crate::curated_feeds::get_curated_registry().get_by_url(url));

    let tier = if let Some(manifest) = curated_manifest {
        manifest.resolved_tier()
    } else {
        crate::source_tiers::SourceTier::default_for_source(input.source_type)
    };
    let tier_authority_mult = tier.authority_multiplier();

    // Curated feeds: override content_dna_mult with manifest-declared content type
    // (only if the manifest specifies a type AND the regex classifier didn't already
    // detect something more specific like SecurityAdvisory or BreakingChange).
    let content_dna_mult = if let Some(manifest) = curated_manifest {
        let manifest_mult = manifest.content_multiplier();
        // Keep the regex-detected type if it's higher priority (security/breaking)
        if content_dna_mult >= 1.25 {
            content_dna_mult
        } else {
            manifest_mult.max(content_dna_mult)
        }
    } else {
        content_dna_mult
    };

    // SecurityAdvisory conditional multiplier: the full 1.30 content_dna boost
    // is only justified when the advisory actually affects the user's dependencies.
    // Without dep confirmation, the boost inflates scores for irrelevant CVEs.
    let content_dna_mult = if content_type == crate::content_dna::ContentType::SecurityAdvisory {
        if raw.dep_match_score == 0.0 {
            // No dependency matched — neutralize the boost, don't penalize
            content_dna_mult.min(1.00)
        } else if raw.dep_match_score <= 0.40 {
            // Weak match — partial boost
            content_dna_mult.min(1.10)
        } else {
            // Strong dep match — full boost justified
            content_dna_mult
        }
    } else {
        content_dna_mult
    };

    // Community quality signal: SO score, HN points, Reddit upvotes
    let age_hours_for_community = input.created_at.map_or(0.0, |ts| {
        (chrono::Utc::now() - *ts).num_minutes().max(0) as f64 / 60.0
    });
    let community_signal =
        extract_community_signal(input.source_type, input.tags_json, age_hours_for_community);
    // community_mult retained for diagnostics, not used in composite (engagement formula uses community_signal directly)
    let _community_mult = if community_signal < scoring_config::COMMUNITY_SIGNAL_LOW_THRESHOLD {
        scoring_config::COMMUNITY_SIGNAL_LOW_PENALTY
    } else if community_signal >= scoring_config::COMMUNITY_SIGNAL_HIGH_THRESHOLD {
        scoring_config::COMMUNITY_SIGNAL_HIGH_BOOST
    } else {
        1.0
    };

    // ── Structural multipliers (content-intrinsic, multiplicative) ──
    let structural = competing_mult
        * content_quality.multiplier
        * content_dna_mult
        * novelty.multiplier
        * sophistication_mult
        * freshness
        * domain_quality_mult
        * negative_stack_prior
        * tier_authority_mult;

    // ── Engagement multiplier (user-learned signals, unified weighted sum) ──
    // Convert each signal to a centered effect:
    //   affinity_mult [0.3, 1.7] → effect [-0.7, 0.7] (subtract 1.0)
    //   anti_penalty [0.0, 0.7] → effect [0.0, -0.7] (negate)
    //   community_signal [0.0, 1.0] → effect [-0.5, 0.5] (subtract 0.5)
    //   feedback_boost [-0.20, 0.20] → used directly
    //   taste_boost [-0.08, 0.08] → used directly
    //   source_quality_mult [0.8, 1.2] → effect [-0.2, 0.2] (subtract 1.0)
    let affinity_effect = raw.affinity_mult - 1.0;
    let anti_effect = -raw.anti_penalty;
    let community_effect = community_signal - 0.5;
    let source_quality_effect = source_quality_mult - 1.0;

    let engagement_sum = affinity_effect * scoring_config::ENGAGEMENT_WEIGHTS_AFFINITY_W
        + anti_effect * scoring_config::ENGAGEMENT_WEIGHTS_ANTI_TOPIC_W
        + community_effect * scoring_config::ENGAGEMENT_WEIGHTS_COMMUNITY_W
        + raw.feedback_boost * scoring_config::ENGAGEMENT_WEIGHTS_FEEDBACK_W
        + raw.taste_boost * scoring_config::ENGAGEMENT_WEIGHTS_TASTE_W
        + source_quality_effect * scoring_config::ENGAGEMENT_WEIGHTS_SOURCE_QUALITY_W;

    let engagement_mult = 1.0
        + engagement_sum.clamp(
            scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MIN,
            scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MAX,
        );

    let composite = structural * engagement_mult;

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
    let quality_score = if novelty.is_security
        && raw.dep_match_score < scoring_config::SECURITY_DEP_VALIDATION_DEP_CONFIDENCE_THRESHOLD
        && !raw.matched_deps.is_empty()
    {
        quality_score * scoring_config::SECURITY_DEP_VALIDATION_WEAK_MATCH_PENALTY
    } else if novelty.is_security && raw.matched_deps.is_empty() {
        quality_score * scoring_config::SECURITY_DEP_VALIDATION_NO_MATCH_PENALTY
    } else if novelty.is_security && all_transitive {
        quality_score * scoring_config::SECURITY_DEP_VALIDATION_WEAK_MATCH_PENALTY
    } else {
        quality_score
    };

    // Community signal gate for user-generated content sources.
    // Low-community-signal items from UGC platforms (dev.to, medium, hashnode,
    // reddit, stackoverflow) get hard-capped — prevents generic blog posts and
    // zero-upvote questions from riding keyword matches into the briefing.
    // Authoritative sources (CVE, RustSec, GitHub, crates.io, npm, PyPI) are exempt.
    let quality_score = if community_signal < scoring_config::COMMUNITY_SIGNAL_LOW_THRESHOLD {
        match input.source_type {
            "devto" | "medium" | "hashnode" | "reddit" | "stackoverflow" | "lobsters" => {
                quality_score.min(0.50)
            }
            _ => quality_score,
        }
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
        sophistication_raw,
        community_signal,
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
    // Note: feedback_boost and taste_boost are now handled in the Phase 5
    // unified engagement formula, not here.
    let total_raw = dep_boost
        + raw.stack_boost
        + intent_boost
        + window_boost
        + skill_gap_boost
        + calibration_correction
        + anti_pattern_correction
        - archetype_penalty;

    let total_capped = total_raw.clamp(
        scoring_config::BOOST_CLAMP_MIN,
        scoring_config::BOOST_CLAMP_MAX,
    );

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
    dep_match_score: f32,
) -> f32 {
    let idx = (signal_count as usize).min(5);
    let (conf_mult, base_ceiling) = scoring_config::CONFIRMATION_GATE[idx];
    // Adjust ceiling based on signal strength — strong signals get higher ceiling.
    // This creates sub-ranking within gate tiers: strong 2-signal items at ~0.73
    // are clearly differentiated from weak 2-signal items capped at 0.65.
    let score_ceiling = (base_ceiling + strength_bonus).min(1.0);

    // Direct dependency gate bypass: if a strong dep match got orphaned into
    // single-axis territory, raise the ceiling so it isn't capped at 0.28.
    // Without this, serde/tokio/axum release notes score ~48% instead of 75%+
    // because dependency is the only confirmed axis for package-specific content.
    let score_ceiling = if signal_count <= 1
        && dep_match_score >= scoring_config::DEPENDENCY_GATE_BYPASS_DIRECT_DEP_MIN_SCORE
    {
        score_ceiling.max(scoring_config::DEPENDENCY_GATE_BYPASS_DIRECT_DEP_CEILING)
    } else {
        score_ceiling
    };

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
// Phase 8: Apply final adjustments — short title cap + commodity ceiling
// ============================================================================

fn apply_final_adjustments(
    score: f32,
    title: &str,
    content_type: &crate::content_dna::ContentType,
    sophistication_raw: f32,
    community_signal: f32,
) -> f32 {
    let meaningful_words = title.split_whitespace().filter(|w| w.len() >= 2).count();
    let score = if meaningful_words < 3 {
        score.min(scoring_config::QUALITY_FLOOR_SHORT_TITLE_CAP)
    } else {
        score
    };

    // Commodity content ceiling: hard cap on low-sophistication commodity content.
    // Applied AFTER all boosts and gate effects — no amount of dep_boost or
    // bootstrap doubling can push a basic "how to" tutorial into the briefing.
    apply_commodity_ceiling(
        score,
        title,
        content_type,
        sophistication_raw,
        community_signal,
    )
}

/// Hard ceiling for commodity content types with low sophistication.
///
/// Exemptions (any bypasses the ceiling):
/// - CVE/GHSA pattern in title
/// - Version conflict language with version number
/// - Content type overridden to SecurityAdvisory or BreakingChange (already excluded)
/// - Sophistication >= 0.35 (has advanced terms, version specificity, or abstract framing)
/// - High community validation (community_signal >= high_threshold)
fn apply_commodity_ceiling(
    score: f32,
    title: &str,
    content_type: &crate::content_dna::ContentType,
    sophistication_raw: f32,
    community_signal: f32,
) -> f32 {
    use crate::content_dna::ContentType;

    let title_lower = title.to_lowercase();

    // Egregious clickbait is hard-capped regardless of content type or dep match
    // — a clickbait title name-dropping a dependency must not ride the dep-match
    // domain promotion into the brief. Genuine security/version content is exempt
    // so a (rare) clickbait-styled CVE still surfaces.
    if crate::content_quality::is_strong_clickbait(title)
        && !has_security_pattern(&title_lower)
        && !has_version_conflict(&title_lower)
    {
        return score.min(scoring_config::COMMODITY_CEILING_CLICKBAIT);
    }

    // Only applies to commodity types
    let ceiling = match content_type {
        ContentType::Tutorial => scoring_config::COMMODITY_CEILING_TUTORIAL,
        ContentType::HelpRequest => scoring_config::COMMODITY_CEILING_HELP_REQUEST,
        ContentType::Question => scoring_config::COMMODITY_CEILING_QUESTION,
        _ => return score,
    };

    // High community validation bypasses ceiling — the crowd validated this content
    if community_signal >= scoring_config::COMMUNITY_SIGNAL_HIGH_THRESHOLD {
        return score;
    }

    // Sophistication above threshold = not commodity
    if sophistication_raw >= 0.35 {
        return score;
    }

    // Security/version exemptions
    if has_security_pattern(&title_lower) || has_version_conflict(&title_lower) {
        return score;
    }

    score.min(ceiling)
}

fn has_security_pattern(title_lower: &str) -> bool {
    title_lower.contains("cve-")
        || title_lower.contains("ghsa-")
        || title_lower.contains("security advisory")
        || title_lower.contains("vulnerability")
}

fn has_version_conflict(title_lower: &str) -> bool {
    let conflict_terms = [
        "breaks",
        "incompatible",
        "deprecated",
        "breaking change",
        "migration",
    ];
    let has_conflict = conflict_terms.iter().any(|t| title_lower.contains(t));
    let has_version = title_lower.chars().any(|c| c.is_ascii_digit())
        && (title_lower.contains('v') || title_lower.contains('.'));
    has_conflict && has_version
}

// ============================================================================
// Score offset normalization
// ============================================================================

/// Normalize score to guaranteed-positive range.
/// Negative scores (from anti-topic penalties, negative feedback) map to [0, floor].
/// Zero/positive scores shift by +floor to separate from "unknown" items.
fn normalize_score_offset(score: f32) -> f32 {
    if score <= 0.0 {
        // Map negative range [-1.0, 0.0] to [0.0, floor] proportionally
        (score + 1.0).max(0.0) * scoring_config::SCORE_OFFSET_NEGATIVE_FLOOR
    } else {
        // Positive scores shift up by floor amount
        score + scoring_config::SCORE_OFFSET_NEGATIVE_FLOOR
    }
}

/// Knee above which scores are soft-compressed toward the absolute ceiling.
/// Below this, scores pass through untouched (mid/low calibration unaffected).
const SOFT_CEILING_KNEE: f32 = 0.80;

/// Soft-compress scores approaching the absolute ceiling so the top tier stays
/// rankable instead of piling up at a hard clamp. Monotonic — preserves order.
///
/// Post-gate additive boosts (the score offset, topic-attention) push strong
/// items past `final_ceiling.absolute_max`, where a hard `.min(1.0)` then
/// flattened dozens of distinct items onto an identical 1.0 — destroying the
/// ranking exactly where it matters most (the Brief's top slots) and breaking
/// the design invariant that no heuristic item should display 100%.
///
/// This maps `(knee, +inf)` smoothly onto `(knee, cap)`: at the knee the output
/// equals the input; above it the output asymptotically approaches `cap` while
/// preserving relative order. Only scores above `knee` are affected.
fn soft_ceiling(score: f32, knee: f32, cap: f32) -> f32 {
    if score <= knee || cap <= knee {
        score.min(cap)
    } else {
        let span = cap - knee;
        let over = score - knee;
        knee + span * (1.0 - (-over / span).exp())
    }
}

/// Canonical final-score de-saturation. Applied both mid-pipeline (on
/// `combined_score` inside `score_item`) AND at the analysis boundary on the
/// persisted `top_score` after the cross-encoder / reconciler overwrite it —
/// so the stored `relevance_score` honors the `final_ceiling.absolute_max`
/// invariant end-to-end and the top tier never piles up at a hard ceiling.
pub(crate) fn apply_final_soft_ceiling(score: f32) -> f32 {
    soft_ceiling(
        score,
        SOFT_CEILING_KNEE,
        scoring_config::FINAL_CEILING_ABSOLUTE_MAX,
    )
}

/// THE single authoritative score-shaping boundary. Applies the final ceiling
/// to every result's persisted `top_score`, exactly once, after all rerank
/// stages and before persistence. Call this at the end of EVERY analysis path
/// (cached, fresh, deep-scan) so `relevance_score` honors the ceiling no matter
/// which reranker last overwrote `top_score`. Does NOT reorder — each path keeps
/// its own sort / composition-floor logic, which must run after this.
pub(crate) fn finalize_scores(results: &mut [crate::SourceRelevance]) {
    for r in results.iter_mut() {
        r.top_score = apply_final_soft_ceiling(r.top_score);
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
    let corroboration = super::pipeline_signals::build_corroboration(db, &topics, matched_deps);
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
                let has_strong_dep = dependencies::is_strongly_grounded(matched_deps);
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
                let has_strong_direct_dep = dependencies::is_strongly_grounded_direct(matched_deps);
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
            primary_topic: topics.first().cloned(),
        };
    }

    // -- Language gate (mirrors V1 pipeline.rs) --──
    // Foreign-language content is capped hard at the end of the pipeline.
    // Empty detected_lang (unknown) bypasses the gate, exactly like V1.
    let user_lang = crate::i18n::get_user_language();
    let lang_mismatch = !input.detected_lang.is_empty() && input.detected_lang != user_lang;

    // ── KNN context search (needed for Phase 1 and final output) ──────
    // Zero-vector guard (mirrors V1 pipeline.rs): a zero embedding produces
    // identical inverse-L2 KNN distances for every context row → uniform
    // similarity → calibrate_knn lifts it above CONTEXT_THRESHOLD → a phantom
    // confirmed context axis. Zero embeddings exist by design (OSV/CVE items
    // are retained with a 768-dim zero blob when embedding providers are
    // down), so require a REAL embedding, not merely a non-empty one.
    let has_real_embedding = input.embedding.iter().any(|&v| v != 0.0);
    let matches: Vec<RelevanceMatch> = if ctx.cached_context_count > 0 && has_real_embedding {
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
        sophistication_raw,
        community_signal,
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

    // Primary stack title boost: direct mention of user's primary tech in the
    // title is a high-confidence signal. "Tauri right-click menu" for a Tauri
    // dev should outscore a tangential multi-language comparison.
    let primary_title_boost = if !ctx.domain_profile.primary_stack.is_empty() {
        let title_lower = input.title.to_lowercase();
        let hits = ctx
            .domain_profile
            .primary_stack
            .iter()
            .filter(|tech| {
                tech.len() >= 4
                    && crate::knowledge_decay::has_word_boundary_match(&title_lower, tech)
            })
            .count();
        match hits {
            0 => 0.0_f32,
            1 => 0.06,
            _ => 0.10,
        }
    } else {
        0.0
    };
    let boosted_score = (boosted_score + primary_title_boost).clamp(0.0, 1.0);

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
        raw.dep_match_score,
    );

    // ── Phase 8: Final adjustments ────────────────────────────────────
    let combined_score = apply_final_adjustments(
        gated_score,
        input.title,
        &content_type,
        sophistication_raw,
        community_signal,
    );

    // ── Score offset normalization ────────────────────────────────────
    // Guarantees all scores are positive. Separates scored items from
    // truly-unknown (zero) items by shifting up by floor amount.
    let combined_score = normalize_score_offset(combined_score);

    let combined_score = if !ctx.topic_attention_gaps.is_empty() && !raw.topics.is_empty() {
        let matching_gaps: Vec<f32> = raw
            .topics
            .iter()
            .filter_map(|t| ctx.topic_attention_gaps.get(t.as_str()).copied())
            .filter(|&h| h > 48.0)
            .collect();
        if matching_gaps.is_empty() {
            combined_score
        } else {
            let avg_gap = matching_gaps.iter().sum::<f32>() / matching_gaps.len() as f32;
            let boost = ((avg_gap - 48.0) / (168.0 - 48.0)).clamp(0.0, 1.0) * 0.05;
            // No hard clamp here — the final soft_ceiling handles the top end
            // while preserving the differentiation this boost just added.
            combined_score + boost
        }
    } else {
        combined_score
    };

    // ── Critical content fast-path ─────────────────────────────────────
    // Security advisories and breaking changes affecting user's actual
    // dependencies ALWAYS surface, regardless of relevance score.
    // This prevents the gate from silently dropping critical alerts.
    //
    // IMPORTANT: the dep match must be strong AND strongly grounded.
    // The aggregate threshold plus a bare non-dev check was too loose: a
    // regex-classified "security" headline plus low-confidence hits (an
    // ambiguous package name like `log`, or a couple of 0.25-confidence
    // topic overlaps) reached the floor and surfaced irrelevant items as
    // critical. The #174 canonical predicate (`is_strong_grounding_match`:
    // non-dev, confidence >= 0.40, non-ambiguous package name) is the
    // necessary grounding condition; the DSL threshold remains as the
    // aggregate-strength check. Advisories whose best match sits in the
    // 0.25-0.40 confidence band lose the fast-path floor but still score
    // through the normal pipeline; the OSV/preemption surface
    // (osv::matching — version-confirmed against structured metadata) is
    // independent of this floor and unaffected.
    let is_security = content_type == crate::content_dna::ContentType::SecurityAdvisory;
    let is_breaking = content_type == crate::content_dna::ContentType::BreakingChange;
    let has_strong_dep_match = raw.dep_match_score
        >= scoring_config::CRITICAL_FASTPATH_DEP_MATCH_THRESHOLD
        && dependencies::is_strongly_grounded(&raw.matched_deps);
    let critical_fast_path = (is_security || is_breaking) && has_strong_dep_match;

    // A CVE confirmed against the user's DIRECT (non-dev) dependency is the
    // flagship preemption case and the highest-confidence security signal — it
    // floors higher than a generic match so a pure-dep-signal advisory (weak
    // embedding, no topic overlap) still scores clearly relevant instead of
    // sitting at the bare 0.50 floor. The higher tier requires the direct dep
    // itself to be the strongly grounded edge (canonical predicate), not just
    // any direct dep riding alongside a grounded transitive match.
    let has_direct_dep = dependencies::is_strongly_grounded_direct(&raw.matched_deps);
    let fast_path_floor = if has_direct_dep {
        scoring_config::CRITICAL_FASTPATH_DIRECT_DEP_FLOOR
    } else {
        scoring_config::CRITICAL_FASTPATH_SCORE_FLOOR
    };

    // If critical fast-path, boost score to ensure it passes the gate
    let combined_score = if critical_fast_path && combined_score < fast_path_floor {
        combined_score.max(fast_path_floor) // Floor for security items matching deps
    } else {
        combined_score
    };

    // ── Final top-end de-saturation ───────────────────────────────────
    // Keep the strongest items rankable (the Brief's top slots) and honor the
    // "no item displays 100%" invariant. Post-gate boosts otherwise clamp many
    // distinct items onto an identical 1.0; this spreads them monotonically
    // just below the absolute ceiling. Only affects scores above the knee.
    let combined_score = apply_final_soft_ceiling(combined_score);

    // -- Language mismatch cap (V1 semantics) --────
    // Foreign content cannot exceed 0.05 - well below the relevance
    // threshold, so the score branch below can never mark it relevant.
    // Applied after every boost/floor (including the critical fast-path
    // floor) so nothing re-inflates a foreign item.
    let combined_score = if lang_mismatch {
        combined_score.min(scoring_config::LANGUAGE_MISMATCH_PENALTY_CAP)
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
    // The critical fast-path is score-independent, so it must also respect
    // the language gate: V1 never let a language-mismatched item be relevant,
    // and a 0.05-capped "relevant" item would be contradictory.
    let relevant = (critical_fast_path && !lang_mismatch)  // Critical items always relevant
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
            signal_count,
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

    // Security severity evidence feeds the necessity bucket below, so extract it
    // BEFORE building NecessityInputs. Previously it was computed afterward, so a real
    // critical CVE on a dev-only dep (which can reach the security path with no signal
    // priority) fell back to "medium" instead of critical (bug J).
    let is_security_source = matches!(input.source_type, "cve" | "osv");
    let (cvss_score, cvss_severity) = if is_security_source {
        extract_cvss_from_content(input.content)
    } else {
        (None, None)
    };

    let necessity_inputs = necessity::NecessityInputs {
        dep_match_score: raw.dep_match_score,
        matched_deps: matched_dep_names.clone(),
        signal_type: sig_type.clone(),
        signal_priority: sig_priority.clone(),
        cve_severity: None, // folded into signal_priority by the classifier
        cvss_score,         // numeric severity fallback when no priority is present
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
    let (applicability, is_critical_alert) = if sig_type.as_deref() == Some("security_alert") {
        // Metadata-verified strong dep: confidence >= 0.40, not dev, AND
        // either the advisory has no affected-package metadata or the metadata
        // names this dependency in the same ecosystem.
        let advisory_ecosystems = extract_advisory_ecosystems(input.content);
        let has_strong_dep = raw.matched_deps.iter().any(|d| {
            if !dependencies::is_strong_grounding_match(d) {
                return false;
            }
            if advisory_ecosystems.is_empty() {
                return true; // can't verify package metadata
            }
            advisory_affects_dependency(&advisory_ecosystems, d)
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
    // (cvss_score / cvss_severity already extracted above for the necessity bucket)
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
        strongly_grounded: dependencies::is_strongly_grounded(&raw.matched_deps),
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
        primary_topic: raw.topics.first().cloned(),
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

    // ========================================================================
    // Language gate (ported from V1 - pipeline.rs lang_mismatch cap)
    // ========================================================================

    /// Build a context + input pair where the item strongly matches the
    /// user's interests, so any score suppression is attributable to the
    /// language gate alone.
    fn lang_gate_fixture(embedding: &[f32]) -> (crate::scoring::ScoringContext, ScoringOptions) {
        let interests = vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(embedding.to_vec()),
            source: crate::context_engine::InterestSource::Explicit,
        }];
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        ace_ctx.topic_confidence.insert("rust".to_string(), 0.9);

        let ctx = crate::scoring::ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace_ctx)
            .build();
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };
        (ctx, options)
    }

    fn lang_gate_input<'a>(embedding: &'a [f32], detected_lang: &'a str) -> ScoringInput<'a> {
        ScoringInput {
            id: 1,
            title: "Rust async runtime performance improvements",
            url: Some("https://example.com/rust"),
            content: "rust tokio async await performance benchmarks",
            source_type: "hackernews",
            embedding,
            created_at: None,
            detected_lang,
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
        }
    }

    #[test]
    fn v2_language_mismatch_capped_and_not_relevant() {
        let db = crate::test_utils::test_db();
        let embedding = vec![0.5_f32; crate::EMBEDDING_DIMS];
        let (ctx, options) = lang_gate_fixture(&embedding);

        // Detect the user's current language at runtime and pick a
        // definitively different one (mirrors pipeline_tests.rs:931).
        let user_lang = crate::i18n::get_user_language();
        let mismatched_lang = if user_lang == "zz-test" {
            "en"
        } else {
            "zz-test"
        };

        let input = lang_gate_input(&embedding, mismatched_lang);
        let result = score_item(&input, &ctx, &db, &options, None);

        assert!(
            result.top_score <= 0.05,
            "V2 language mismatch (user={}, content={}) must cap at 0.05, got {}",
            user_lang,
            mismatched_lang,
            result.top_score
        );
        assert!(
            !result.relevant,
            "V2 language-mismatched content must never be relevant (score={})",
            result.top_score
        );
    }

    #[test]
    fn v2_same_language_unaffected_by_gate() {
        let db = crate::test_utils::test_db();
        let embedding = vec![0.5_f32; crate::EMBEDDING_DIMS];
        let (ctx, options) = lang_gate_fixture(&embedding);

        let user_lang = crate::i18n::get_user_language();
        let input = lang_gate_input(&embedding, &user_lang);
        let result = score_item(&input, &ctx, &db, &options, None);

        assert!(
            result.top_score > 0.05,
            "Same-language content must not be capped, got {}",
            result.top_score
        );
    }

    #[test]
    fn v2_empty_detected_lang_bypasses_gate() {
        let db = crate::test_utils::test_db();
        let embedding = vec![0.5_f32; crate::EMBEDDING_DIMS];
        let (ctx, options) = lang_gate_fixture(&embedding);

        let user_lang = crate::i18n::get_user_language();

        let same_lang = score_item(
            &lang_gate_input(&embedding, &user_lang),
            &ctx,
            &db,
            &options,
            None,
        );
        let empty_lang = score_item(&lang_gate_input(&embedding, ""), &ctx, &db, &options, None);

        assert!(
            (empty_lang.top_score - same_lang.top_score).abs() < f32::EPSILON,
            "Empty detected_lang must score identically to same-language: empty={}, same={}",
            empty_lang.top_score,
            same_lang.top_score
        );
        assert!(
            empty_lang.top_score > 0.05,
            "Empty detected_lang must not be capped, got {}",
            empty_lang.top_score
        );
    }

    // ========================================================================
    // Zero-vector KNN guard — a zero embedding must not manufacture a
    // confirmed context axis (gate count inflation, Fix A)
    // ========================================================================

    #[test]
    fn v2_zero_embedding_yields_no_context_axis() {
        let db = crate::test_utils::test_db();
        // Store a real context chunk so KNN WOULD return rows if queried.
        let stored = crate::test_utils::seed_embedding("context-chunk");
        db.upsert_context("src/main.rs", "rust tauri ipc command handler", &stored)
            .expect("store context chunk");

        let ctx = crate::scoring::ScoringContext::builder()
            .cached_context_count(1)
            .build();
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        // Zero-vector embedding (OSV/CVE fallback when providers are down)
        let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
        let input = ScoringInput {
            id: 1,
            title: "Completely unrelated gardening newsletter",
            url: Some("https://example.com/gardening"),
            content: "tips for growing tomatoes in winter",
            source_type: "rss",
            embedding: &zero,
            created_at: None,
            detected_lang: "",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
        };
        let result = score_item(&input, &ctx, &db, &options, None);

        assert!(
            result.matches.is_empty(),
            "zero-vector embedding must not run KNN, got {} matches",
            result.matches.len()
        );
        assert_eq!(
            result.context_score, 0.0,
            "zero-vector embedding must yield context_score 0.0"
        );
        let bd = result.score_breakdown.as_ref().expect("breakdown");
        assert!(
            !bd.confirmed_signals.contains(&"context".to_string()),
            "zero-vector embedding must not confirm the context axis, got {:?}",
            bd.confirmed_signals
        );

        // Control: a REAL embedding against the same DB does produce KNN
        // matches — proving the fixture is valid and the guard (not an
        // empty DB) is what suppressed the phantom axis above.
        let real = crate::test_utils::seed_embedding("context-chunk");
        let control_input = ScoringInput {
            embedding: &real,
            ..input
        };
        let control = score_item(&control_input, &ctx, &db, &options, None);
        assert!(
            !control.matches.is_empty(),
            "real embedding against stored contexts must produce KNN matches"
        );
        assert!(
            control.context_score > 0.0,
            "identical real embedding must yield a positive context score"
        );
    }

    // ========================================================================
    // Critical fast-path requires strong grounding (Fix D)
    // ========================================================================

    /// Context with tracked dependencies (direct, non-dev) installed the
    /// same way production populates ACE dependency intelligence.
    fn fastpath_ctx(packages: &[(&str, &str)]) -> crate::scoring::ScoringContext {
        let mut ace_ctx = ACEContext::default();
        for (package, ecosystem) in packages {
            let normalized = dependencies::normalize_package_name(package);
            let info = dependencies::DepInfo {
                package_name: normalized.clone(),
                version: None,
                is_dev: false,
                is_direct: true,
                search_terms: dependencies::extract_search_terms(package),
                ecosystem: (*ecosystem).to_string(),
            };
            for term in &info.search_terms {
                ace_ctx.dependency_names.insert(term.clone());
            }
            ace_ctx.dependency_names.insert(normalized.clone());
            ace_ctx.dependency_info.insert(normalized, info);
        }
        crate::scoring::ScoringContext::builder()
            .ace_ctx(ace_ctx)
            .build()
    }

    fn fastpath_options() -> ScoringOptions {
        ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        }
    }

    #[test]
    fn v2_critical_fastpath_rejects_ambiguous_low_grounding_match() {
        // Phantom case: a regex-classified security headline matching the
        // user's `log` and `time` crates — ambiguous package names whose
        // bare text hits cannot be trusted (#174 canonical denylist).
        // Previously the aggregate dep_match_score cleared the 0.25
        // threshold and the item was floored at 0.65 + forced relevant.
        // It must NOT be.
        let db = crate::test_utils::test_db();
        let ctx = fastpath_ctx(&[("log", "rust"), ("time", "rust")]);
        let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
        let tags = vec!["log".to_string()];
        let input = ScoringInput {
            id: 1,
            title: "Critical vulnerability in log and time crates allows remote code execution",
            url: Some("https://example.com/advisory"),
            content: "A vulnerability was reported affecting logging functionality in \
                      several applications.",
            source_type: "hackernews",
            embedding: &zero,
            created_at: None,
            detected_lang: "",
            source_tags: &tags,
            tags_json: None,
            feed_origin: None,
        };
        let result = score_item(&input, &ctx, &db, &fastpath_options(), None);

        // Sanity: the dep DID match with fast-path-level aggregate strength —
        // this is exactly the configuration that previously inflated.
        let bd = result.score_breakdown.as_ref().expect("breakdown");
        assert!(
            bd.matched_deps.iter().any(|d| d == "log"),
            "fixture must produce a `log` dep match, got {:?}",
            bd.matched_deps
        );
        assert!(
            bd.dep_match_score >= scoring_config::CRITICAL_FASTPATH_DEP_MATCH_THRESHOLD,
            "fixture must clear the aggregate fast-path threshold (got {})",
            bd.dep_match_score
        );
        assert!(
            !bd.strongly_grounded,
            "ambiguous `log` match must not be strongly grounded"
        );

        // The actual regression assertions: no floor, not relevant.
        assert!(
            result.top_score < scoring_config::CRITICAL_FASTPATH_SCORE_FLOOR,
            "ambiguous low-grounding match must not receive the fast-path \
             floor, got {}",
            result.top_score
        );
        assert!(
            !result.relevant,
            "ambiguous low-grounding match must not be forced relevant \
             (score={})",
            result.top_score
        );
    }

    #[test]
    fn v2_critical_fastpath_keeps_direct_dep_floor_for_grounded_advisory() {
        // Real case: an advisory naming the user's direct `axios` dependency
        // in the title — full-name word-boundary hit, confidence >= 0.40,
        // non-ambiguous name. Must keep the 0.65 direct-dep floor and the
        // relevant=true override.
        let db = crate::test_utils::test_db();
        let ctx = fastpath_ctx(&[("axios", "javascript")]);
        let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
        let tags = vec!["axios".to_string()];
        let input = ScoringInput {
            id: 2,
            title: "Critical vulnerability in axios package allows SSRF attacks",
            url: Some("https://example.com/advisory"),
            content: "A server-side request forgery flaw affects applications \
                      making HTTP requests through the vulnerable client.",
            source_type: "hackernews",
            embedding: &zero,
            created_at: None,
            detected_lang: "",
            source_tags: &tags,
            tags_json: None,
            feed_origin: None,
        };
        let result = score_item(&input, &ctx, &db, &fastpath_options(), None);

        let bd = result.score_breakdown.as_ref().expect("breakdown");
        assert!(
            bd.strongly_grounded,
            "full-name direct-dep advisory must be strongly grounded \
             (deps={:?}, dep_match_score={})",
            bd.matched_deps, bd.dep_match_score
        );
        assert!(
            result.top_score >= scoring_config::CRITICAL_FASTPATH_DIRECT_DEP_FLOOR - 1e-6,
            "grounded direct-dep advisory must keep the {} floor, got {}",
            scoring_config::CRITICAL_FASTPATH_DIRECT_DEP_FLOOR,
            result.top_score
        );
        assert!(
            result.relevant,
            "grounded direct-dep advisory must remain relevant"
        );
    }

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
            tags_json: None,
            feed_origin: None,
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
            tags_json: None,
            feed_origin: None,
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
            tags_json: None,
            feed_origin: None,
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

    fn test_dep(package_name: &str, ecosystem: &str) -> DepMatch {
        DepMatch {
            package_name: package_name.to_string(),
            confidence: 0.75,
            version_delta: VersionDelta::Unknown,
            is_dev: false,
            is_direct: true,
            version: None,
            ecosystem: ecosystem.to_string(),
        }
    }

    #[test]
    fn cve_dep_match_score_does_not_halve_direct_deps() {
        // A single confirmed DIRECT dependency is full evidence for a CVE. The old
        // `total / 2.0` halved it to ~0.375 — below the 0.40 SecurityAdvisory
        // full-boost threshold (see content_dna_mult gate) — so direct-dep CVEs
        // floored at 0.50. The fix floors the score at the strongest direct-dep
        // confidence so the flagship preemption case can score high.
        let direct = vec![test_dep("reqwest", "rust")]; // confidence 0.75, is_direct
        let s = cve_dep_match_score(&direct);
        assert!(
            s >= 0.75,
            "a single direct-dep (conf 0.75) must not be halved, got {s:.3}"
        );
        assert!(
            s > 0.40,
            "must clear the 0.40 SecurityAdvisory full-boost threshold, got {s:.3}"
        );

        // Transitive-only matches stay conservative (half weight, as before) so a
        // `x509-cert`-via-rustls CVE remains background noise.
        let mut transitive = test_dep("x509-cert", "rust");
        transitive.is_direct = false;
        transitive.confidence = 0.5;
        let st = cve_dep_match_score(std::slice::from_ref(&transitive));
        assert!(
            st <= 0.40,
            "a transitive-only match must stay conservative (<= 0.40), got {st:.3}"
        );

        // Multiple confirmed direct deps still accumulate via the summed path.
        let many = vec![test_dep("tokio", "rust"), test_dep("hyper", "rust")];
        assert!(
            cve_dep_match_score(&many) >= 0.75,
            "multiple confirmed deps remain high-confidence"
        );
    }

    #[test]
    fn test_advisory_affects_dependency_requires_exact_package() {
        let affected = vec![("next".to_string(), "npm".to_string())];

        assert!(
            advisory_affects_dependency(&affected, &test_dep("next", "javascript")),
            "same package and normalized ecosystem should match"
        );
        assert!(
            !advisory_affects_dependency(&affected, &test_dep("react", "javascript")),
            "same ecosystem is not enough when affected package metadata exists"
        );
    }

    #[test]
    fn test_advisory_affects_dependency_normalizes_package_names() {
        let affected = vec![
            ("serde-json".to_string(), "crates.io".to_string()),
            ("@tanstack/react-query".to_string(), "npm".to_string()),
        ];

        assert!(
            advisory_affects_dependency(&affected, &test_dep("serde_json", "rust")),
            "hyphen/underscore variants should match"
        );
        assert!(
            advisory_affects_dependency(&affected, &test_dep("@tanstack/react-query", "npm")),
            "scoped npm package names should match"
        );
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

    // ========================================================================
    // Engagement formula tests
    // ========================================================================

    #[test]
    fn test_engagement_positive_affinity_boosts() {
        // High affinity + no anti-topic -> engagement_mult > 1.0
        let affinity_effect = 0.5; // affinity_mult was 1.5
        let engagement = affinity_effect * scoring_config::ENGAGEMENT_WEIGHTS_AFFINITY_W;
        let mult = 1.0
            + engagement.clamp(
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MIN,
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MAX,
            );
        assert!(
            mult > 1.0,
            "Positive affinity should produce boost: {}",
            mult
        );
        assert!(mult <= 1.6, "Should not exceed clamp max: {}", mult);
    }

    #[test]
    fn test_engagement_anti_topic_penalizes() {
        // Strong anti-topic with no affinity -> engagement_mult < 1.0
        let anti_effect = -0.7; // max anti_penalty
        let engagement = anti_effect * scoring_config::ENGAGEMENT_WEIGHTS_ANTI_TOPIC_W;
        let mult = 1.0
            + engagement.clamp(
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MIN,
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MAX,
            );
        assert!(mult < 1.0, "Anti-topic should produce penalty: {}", mult);
        assert!(mult >= 0.5, "Should not go below clamp floor: {}", mult);
    }

    #[test]
    fn test_engagement_competing_signals_resolve() {
        // High affinity + moderate anti-topic -> net positive
        let affinity_effect = 0.5;
        let anti_effect = -0.3;
        let engagement = affinity_effect * scoring_config::ENGAGEMENT_WEIGHTS_AFFINITY_W
            + anti_effect * scoring_config::ENGAGEMENT_WEIGHTS_ANTI_TOPIC_W;
        let mult = 1.0
            + engagement.clamp(
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MIN,
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MAX,
            );
        // With w_aff=0.55 and w_anti=0.35: 0.5*0.55 + (-0.3)*0.35 = 0.275 - 0.105 = 0.17
        assert!(
            mult > 1.0,
            "Affinity should outweigh moderate anti-topic: {}",
            mult
        );
    }

    #[test]
    fn test_engagement_clamp_prevents_extreme() {
        // All negative signals -> clamped at floor
        let affinity_effect = -0.7;
        let anti_effect = -0.7;
        let community_effect = -0.5;
        let feedback = -0.20_f32;
        let taste = -0.08_f32;
        let source_quality_effect = -0.2;

        let engagement = affinity_effect * scoring_config::ENGAGEMENT_WEIGHTS_AFFINITY_W
            + anti_effect * scoring_config::ENGAGEMENT_WEIGHTS_ANTI_TOPIC_W
            + community_effect * scoring_config::ENGAGEMENT_WEIGHTS_COMMUNITY_W
            + feedback * scoring_config::ENGAGEMENT_WEIGHTS_FEEDBACK_W
            + taste * scoring_config::ENGAGEMENT_WEIGHTS_TASTE_W
            + source_quality_effect * scoring_config::ENGAGEMENT_WEIGHTS_SOURCE_QUALITY_W;
        let clamped = engagement.clamp(
            scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MIN,
            scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MAX,
        );
        let mult = 1.0 + clamped;
        assert_eq!(
            clamped,
            scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MIN,
            "Extreme negative should hit floor"
        );
        assert!(mult >= 0.5, "Multiplier should be 1 + clamp_min = {}", mult);
    }
}
