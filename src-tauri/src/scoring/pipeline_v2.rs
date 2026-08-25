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
use super::types::{ScoringInput, ScoringOptions};
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
        if let Some(rest) = trimmed.strip_prefix("Severity:") {
            // Producer format: osv_types::vuln_to_source_item emits `format!("{severity_type}: {score}")`,
            // e.g. "CVSS_V3: 9.8" OR "CVSS_V3: CVSS:3.1/AV:N/…/A:H" (OSV usually stores the VECTOR). The
            // severity-TYPE label has no internal colon, so splitting on the FIRST colon isolates the score
            // (bare number or full vector) — and, crucially, keeps the "V3" version digit in the discarded
            // TYPE half so it can never be mistaken for the score. `parse_cvss_score` then handles both a
            // bare number and a vector (computing the base score per the CVSS v3.1 spec).
            let score_str = rest
                .trim()
                .split_once(':')
                .map(|(_, v)| v.trim())
                .unwrap_or_else(|| rest.trim());
            if let Some(score) = super::cvss::parse_cvss_score(score_str) {
                if score > 0.0 && score <= 10.0 {
                    let score = score as f32;
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
    stack_boost: f32,
    stack_pain_match: bool,
    topics: Vec<String>,
    specificity_weight: f32,
    /// True when `semantic_boost` came from real embedding similarity
    /// (`compute_semantic_ace_boost` returned `Some`); false when it is the
    /// keyword fallback. Gate evidence only (audit item 8e): the fallback
    /// re-reads the same keyword surface as the interest axis, so it must
    /// never count as INDEPENDENT ACE evidence.
    semantic_is_embedding_derived: bool,
    /// Raw (un-discounted) keyword score over the user's own primary-stack
    /// single-word interests (audit item 14). Gate confirmation evidence
    /// only — never score magnitude.
    own_stack_keyword_score: f32,
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

/// Security-applicability label + critical-alert verdict for a
/// `security_alert` item, from two INDEPENDENT proof routes:
///
/// - **Structured advisory metadata**: the advisory's "Affected:" list names
///   this dependency in the same ecosystem. The metadata IS the proof — the
///   text-corroboration flag is deliberately not consulted, so tightening the
///   text route can never weaken OSV/CVE-verified alerts.
/// - **Text route** (no affected-package metadata): the match must satisfy the
///   full canonical grounding predicate — base requirements plus name
///   corroboration (the item actually names the package).
///
/// Deps below the strong floor (or dev-only) still yield "likely_affected"
/// rather than "affected", exactly as before.
fn security_applicability(
    matched_deps: &[DepMatch],
    advisory_ecosystems: &[(String, String)],
) -> (Option<String>, bool) {
    let has_strong_dep = matched_deps.iter().any(|d| {
        // Dev-only matches stay "likely_affected" (never `affected` /
        // critical) — explicit here since `is_grounding_candidate` admits dev
        // deps for FEED grounding (item 16); the alert lane does not follow.
        if d.is_dev || !dependencies::is_grounding_candidate(d) {
            return false;
        }
        if advisory_ecosystems.is_empty() {
            d.corroborated // can't verify metadata — require the text proof
        } else {
            advisory_affects_dependency(advisory_ecosystems, d)
        }
    });
    let has_any_dep = !matched_deps.is_empty();
    let all_transitive = matched_deps.iter().all(|d| !d.is_direct);

    if has_strong_dep {
        if all_transitive {
            (Some("likely_affected".to_string()), false)
        } else {
            (Some("affected".to_string()), true)
        }
    } else if has_any_dep {
        // Weak/dev-only/uncorroborated matches: plausible but unproven.
        (Some("likely_affected".to_string()), false)
    } else {
        (Some("needs_verification".to_string()), false)
    }
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

/// CVE/OSV survivor filter — decides which dependency matches remain credible
/// for a security advisory. Two evidence routes, structured metadata FIRST
/// (2026-08-23 audit, item 8b):
///
/// * **Structured route** (the advisory carries an `Affected:` list): the
///   metadata is authoritative in BOTH directions. A dep it names (same
///   ecosystem) survives even when the prose never contains the name in
///   matchable form — metadata is stronger evidence than prose — and a dep it
///   does not name is dropped even on a title hit (title/body matches alone
///   are not enough to make a CVE applicable). Pre-fix this route only ran on
///   TEXT-filter survivors, so the one mechanism that could confirm a scoped
///   npm package or Go module never got the chance.
/// * **Text route** (no structured metadata): the advisory text must name the
///   package. Both the NORMALIZED form (`babel-traverse`,
///   `github.com-gin-gonic-gin`) and the RAW manifest form
///   (`@babel/traverse`, `github.com/gin-gonic/gin`) are tried — advisories
///   always write the real form, so requiring the normalized form made every
///   scoped npm package and every Go module path fail the filter.
fn filter_cve_dep_survivors(
    deps: &mut Vec<DepMatch>,
    title_lower: &str,
    body_lower: &str,
    advisory_affected: &[(String, String)],
) {
    if !advisory_affected.is_empty() {
        deps.retain(|d| advisory_affects_dependency(advisory_affected, d));
        return;
    }
    deps.retain(|d| {
        let normalized = d.package_name.to_lowercase();
        if cve_text_names_package(title_lower, body_lower, &normalized) {
            return true;
        }
        // Raw manifest form (pre-normalization): `@scope/name`, full Go
        // module path. A raw-form title hit is rule-1 evidence.
        d.raw_name
            .as_deref()
            .map(str::to_lowercase)
            .filter(|raw| *raw != normalized)
            .is_some_and(|raw| cve_text_names_package(title_lower, body_lower, &raw))
    });
}

/// Text-route survivor rules for ONE candidate name form. A match survives if
/// ANY of:
///   1. The full name appears in the TITLE (high evidence — advisories name
///      the affected software directly).
///   2. The full name appears in the description AND there is package
///      language context ("npm X", "cargo X", "crate X", "package X")
///      within 80 chars, OR
///   3. The name is compound — contains a hyphen, underscore, or path/scope
///      separator (`x509-cert`, `serde_derive`, `@babel/traverse`,
///      `github.com/gin-gonic/gin`). Compound names are inherently specific,
///      so a body match is strong evidence.
///
/// Single word-boundary hits in prose (e.g. the word "hostname" in a
/// DNS-related advisory) are rejected — they're noise.
fn cve_text_names_package(title_lower: &str, body_lower: &str, full: &str) -> bool {
    // Rule 1: title match
    if has_word_boundary_match(title_lower, full) {
        return true;
    }

    if !has_word_boundary_match(body_lower, full) {
        return false;
    }
    // Rule 3: compound name
    if full.contains(['-', '_', '/']) {
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
    for (idx, _) in body_lower.match_indices(full) {
        // Use floor/ceil_char_boundary to avoid panicking on multi-byte
        // UTF-8 content (e.g. accented researcher names in CVE descriptions).
        let start = body_lower.floor_char_boundary(idx.saturating_sub(window));
        let end = body_lower.ceil_char_boundary((idx + full.len() + window).min(body_lower.len()));
        let slice = &body_lower[start..end];
        if CONTEXT_WORDS.iter().any(|w| slice.contains(w)) {
            return true;
        }
    }
    false
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

    // Own-stack confirmation evidence (audit item 14): raw un-discounted
    // keyword score over the user's own primary-stack single-word interests.
    // Feeds ONLY the confirmation gate (with embedding corroboration required
    // there) — the specificity discount stays on the score itself.
    let own_stack_keyword_score = keywords::own_stack_single_word_keyword_score(
        input.title,
        input.content,
        &ctx.interests,
        Some(&spec_profile),
    );

    // Semantic boost with keyword fallback. Provenance is carried so the
    // confirmation gate can refuse to double-count the keyword fallback as
    // independent ACE evidence against a keyword-confirmed interest (audit
    // item 8e — the degraded/embedding-less state used to confirm MORE
    // signals than the healthy one, flipping the 1↔2 signal cliff).
    let embedding_ace =
        compute_semantic_ace_boost(input.embedding, &ctx.ace_ctx, &ctx.topic_embeddings);
    let semantic_is_embedding_derived = embedding_ace.is_some();
    let semantic_boost =
        embedding_ace.unwrap_or_else(|| semantic::compute_keyword_ace_boost(&topics, &ctx.ace_ctx));

    // Dependency intelligence — for security sources, strip the synthetic
    // `Affected:` metadata block from the content so text matching only
    // operates on the actual CVE description, not the list of affected
    // package names that would otherwise create massive false positives.
    let dep_match_text = dep_match_content_for(input);
    let (matched_deps, dep_match_score) = {
        let (mut deps, mut score) =
            match_dependencies(input.title, dep_match_text, &topics, &ctx.ace_ctx);

        // For CVE/OSV items, apply a MUCH stricter post-filter
        // (`filter_cve_dep_survivors`): structured affected-package metadata
        // decides when present; otherwise the advisory text must name the
        // package (normalized OR raw manifest form). Then recompute
        // dep_match_score from the surviving deps. A confirmed
        // direct-dependency match is full evidence for a CVE (see
        // cve_dep_match_score) — do not halve it.
        if matches!(input.source_type, "cve" | "osv") && !deps.is_empty() {
            let title_lower = input.title.to_lowercase();
            let body_lower = dep_match_text.to_lowercase();
            // The `Affected:` metadata block lives in the RAW content (it is
            // stripped from `dep_match_text` before text matching).
            let advisory_affected = extract_advisory_ecosystems(input.content);
            filter_cve_dep_survivors(&mut deps, &title_lower, &body_lower, &advisory_affected);
            score = cve_dep_match_score(&deps);
        }

        // Registry release items: a dep-name mention in another package's
        // description is not "named in the item text" evidence — only the
        // SUBJECT package is what the item is about. Align the corroborated
        // flags so evidence chips and the grounding candidate route agree
        // with the registry-subject verdict below.
        dependencies::align_registry_corroboration(input.source_type, input.source_id, &mut deps);

        // Kill PHANTOM dep matches for non-security items: align dep_match_score
        // with the evidence the user actually sees. match_dependencies sums
        // confidence over ALL matches (its own comment: corroboration is "NOT a
        // confidence input"), but display_worthy_deps shows only CORROBORATED
        // deps. So an item could carry dep_match_score 0.595 while matched_deps
        // rendered EMPTY — a phantom "in your stack" score with no package behind
        // it (a Portabase Docker discussion won the hero this way; a live audit
        // found 33 such items in one feed). Credit only corroborated matches, so
        // the score and the evidence can never disagree. CVE/OSV already recompute
        // above from their strict advisory post-filter, so leave those untouched.
        if !matches!(input.source_type, "cve" | "osv") {
            let corroborated_confidence: f32 = deps
                .iter()
                .filter(|d| d.corroborated)
                .map(|d| d.confidence)
                .sum();
            score = (corroborated_confidence / 2.0).min(1.0);
        }

        (deps, score)
    };

    // Feedback boost, affinity multiplier, and anti-topic penalty DEMOTED
    // in v19 (AD-029): all derived from behavioral capture whose layer
    // mixed three incompatible strength scales and self-poisoned
    // (2026-07-13 doom loop: passive scroll noise drove the user's own
    // stack to −1.0 affinity at ×[0.3, 1.7] authority). Neutral values
    // keep breakdown plumbing intact; explicit suppression still works
    // via user-authored `exclusions`.
    let feedback_boost = 0.0_f32;
    let affinity_mult = 1.0_f32;
    let anti_penalty = 0.0_f32;

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

    // Registry-release discipline: embedding proximity to local code is not,
    // by itself, evidence that an arbitrary package-registry release matters.
    // For registry releases whose subject is not a corroborated dependency,
    // dampen the context axis so package feed noise cannot ride language-level
    // similarity into the user's "relevant" set.
    let raw_context = if crate::dep_linker::is_registry_source(input.source_type)
        && !matched_deps.iter().any(|d| d.corroborated)
    {
        raw_context * 0.3
    } else {
        raw_context
    };

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
        stack_boost,
        stack_pain_match,
        topics,
        specificity_weight,
        semantic_is_embedding_derived,
        own_stack_keyword_score,
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
///
/// Voted sources (SO/HN/reddit) younger than 6 hours get neutral (0.50) —
/// the community genuinely hasn't voted yet. The federated/microblog UGC arm
/// gets NO such free-pass window: its engagement signal applies from age 0.
/// The old blanket <6h early-return created a scheduled cliff — a no-metadata
/// mastodon/lemmy/bluesky item scored freely (up to 0.9x) for six hours, then
/// dropped to 0.25 and was clamped to exactly 0.50 by the UGC exit ceiling
/// forever (live 2026-08-23 audit: yesterday's #1 feed item crashed
/// 0.923→0.500 on schedule; 148 items pinned at exactly 0.50, 72 more queued).
/// Capping consistently from ingest removes the cliff; real engagement
/// metadata (favourites/reblogs, lemmy score, bluesky likes) earns it back.
/// Items without metadata on voted/unknown sources get neutral (0.50).
fn extract_community_signal(source_type: &str, tags_json: Option<&str>, age_hours: f64) -> f32 {
    // Free-pass window ONLY for sources whose score accrues by community
    // voting over the first hours — never for the UGC arm below.
    let too_fresh_to_judge = age_hours < 6.0;

    let tags: serde_json::Value = tags_json
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Null);

    match source_type {
        "stackoverflow" => {
            if too_fresh_to_judge {
                return 0.50;
            }
            let score = tags.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
            match score {
                s if s >= 50 => 0.90,
                s if s >= 20 => 0.75,
                s if s >= 5 => 0.50,
                _ => 0.20,
            }
        }
        "hackernews" => {
            if too_fresh_to_judge {
                return 0.50;
            }
            let points = tags.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
            match points {
                p if p >= 100 => 0.90,
                p if p >= 30 => 0.70,
                p if p >= 10 => 0.50,
                _ => 0.30,
            }
        }
        "reddit" => {
            if too_fresh_to_judge {
                return 0.50;
            }
            let score = tags.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
            match score {
                s if s >= 100 => 0.85,
                s if s >= 20 => 0.65,
                s if s >= 5 => 0.50,
                _ => 0.30,
            }
        }
        // Federated/microblog sources: previously fell through to the 0.50
        // neutral arm — never penalized — so promotional posts and off-topic
        // social noise rode embedding similarity into the feed unchecked
        // (live 2026-07-23: mastodon promo at 0.91, lemmy "share your
        // collection" at 0.61, all feed_relevant). The no-metadata default is
        // LOW (0.25 — below the 0.30 low_threshold, arming the UGC cap), and
        // it applies FROM AGE 0 (no <6h free pass — see the doc comment: the
        // free-pass window produced the 6-hour cliff). Engagement tags
        // (mastodon favourites/reblogs, lemmy score, bluesky likes) earn the
        // score back whenever the adapters ingest them.
        "mastodon" | "lemmy" | "bluesky" | "twitter" => {
            let engagement = tags
                .get("score")
                .and_then(|v| v.as_i64())
                .or_else(|| tags.get("upvotes").and_then(|v| v.as_i64()))
                .or_else(|| {
                    let favs = tags.get("favourites").and_then(|v| v.as_i64());
                    let boosts = tags.get("reblogs").and_then(|v| v.as_i64());
                    match (favs, boosts) {
                        (None, None) => None,
                        (f, b) => Some(f.unwrap_or(0) + b.unwrap_or(0)),
                    }
                })
                .or_else(|| tags.get("likes").and_then(|v| v.as_i64()));
            match engagement {
                Some(e) if e >= 50 => 0.85,
                Some(e) if e >= 10 => 0.60,
                Some(e) if e >= 3 => 0.40,
                _ => 0.25,
            }
        }
        _ => 0.50,
    }
}

// ============================================================================
// Stale published-content discount
// ============================================================================

/// Average Gregorian month in days — for converting item age to months.
const DAYS_PER_MONTH: f32 = 30.44;

/// Discount for content whose PUBLISHED date is years old, regardless of when
/// it was fetched (2026-08-23 audit, item 19: "TypeScript 5.1 Beta is OUT!"
/// published 2023-04, fetched 2026-08, scored 0.882 into the feed top-25 —
/// the freshness tiers bottom out at 0.80 for anything older than 30 days).
///
/// `published` is `ScoringInput.created_at`, which the analysis paths populate
/// with `source_items.published_at` when the source provides one (fetch date
/// only when it doesn't — those items simply age from first-seen).
///
/// Ramp (constants in `pipeline.scoring` → `stale_content`): 1.0 at
/// <= fresh_months (12), linear down to stale_floor (0.55) at >= stale_months
/// (36). Exemptions:
/// * security advisories — an old unpatched CVE can still matter;
/// * strongly dep-grounded items are softened to grounded_floor (0.80), not
///   killed — a 2-year-old deep-dive on YOUR exact stack can still be gold.
/// * RELEASE ANNOUNCEMENTS get neither exemption and a deeper floor
///   (`release_floor`, 0.30): a release note is time-indexed news, superseded
///   by definition once it ages past the ramp — the registry signal is
///   *current* releases of your dependencies, so grounding must not soften it
///   (live 2026-08-25: a 2023 "TypeScript 5.1 Beta is OUT!" held 0.882 and sat
///   feed-relevant because dev-dep grounding lifted its base while the
///   grounded floor kept its staleness discount shallow).
fn stale_published_multiplier(
    published: &chrono::DateTime<chrono::Utc>,
    is_security: bool,
    strongly_grounded: bool,
    is_release: bool,
) -> f32 {
    if is_security {
        return 1.0;
    }
    let age_months = ((chrono::Utc::now() - *published).num_days().max(0) as f32) / DAYS_PER_MONTH;
    let fresh = scoring_config::STALE_CONTENT_FRESH_MONTHS;
    let stale = scoring_config::STALE_CONTENT_STALE_MONTHS;
    let floor = if is_release {
        scoring_config::STALE_CONTENT_RELEASE_FLOOR
    } else {
        scoring_config::STALE_CONTENT_STALE_FLOOR
    };
    let mult = if age_months <= fresh {
        1.0
    } else if age_months >= stale {
        floor
    } else {
        1.0 - (1.0 - floor) * (age_months - fresh) / (stale - fresh)
    };
    // Grounding softens a stale deep-dive, never a superseded announcement.
    if strongly_grounded && !is_release {
        mult.max(scoring_config::STALE_CONTENT_GROUNDED_FLOOR)
    } else {
        mult
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
    grounding: &dependencies::GroundingVerdict,
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

    // Source-engagement multiplier (learned source pref + autophagy blend +
    // per-feed override) DEMOTED in v19 (AD-029); breakdown field stays present and neutral.
    let source_quality_boost = 0.0_f32;

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
    // UNDAMPENED — full suppressive force (0.15 for competing-absent, 0.30 for
    // anti-topics), except migration/comparison content ABOUT the user's own
    // stack, which softens to 0.85 (audit item 18 — "We migrated our Electron
    // app to Tauri" is top-value validation content for a Tauri user, not
    // competitor noise).
    let negative_stack_prior = crate::stacks::negative_stack::lookup_prior_with_content(
        &ctx.ace_ctx.negative_stack,
        &raw.topics,
        input.title,
        input.content,
    );

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
        } else if raw.dep_match_score
            <= scoring_config::SECURITY_DEP_VALIDATION_STRONG_DEP_THRESHOLD
        {
            // Weak match — partial boost. Same DSL knob as the validation
            // penalties below; a hardcoded copy here silently ignored DSL
            // retunes of strong_dep_threshold.
            content_dna_mult.min(1.10)
        } else {
            // Strong dep match — full boost justified
            content_dna_mult
        }
    } else {
        content_dna_mult
    };

    // Registry release grounding (mirrors the SecurityAdvisory rule above).
    // Every package-registry item is manifest-classified ReleaseNotes with an
    // unconditional 1.15x boost, and crates.io sits in the Core tier for Rust
    // users (1.05x) — so an unknown crate whose name/description merely
    // RESEMBLES the user's stack ("axum-connect-rpc", "serde_v8",
    // "forge-plugin-sdk-rust") rode embedding similarity into the feed.
    // Live evidence 2026-07-23: ~81% of relevant items were crates and the
    // top-scored crate (0.947) was not a dependency.
    //
    // Grounding = the released SUBJECT package is one of the user's
    // dependencies — the canonical `compute_grounding_verdict` (registry
    // items are judged by their source_id subject, never by text mentions:
    // a description that name-drops tokio ("built on tokio and serde") is
    // about a DIFFERENT package and must not ground).
    let is_registry_release = crate::dep_linker::is_registry_source(input.source_type)
        && matches!(
            content_type,
            crate::content_dna::ContentType::ReleaseNotes
                | crate::content_dna::ContentType::BreakingChange
        );
    let registry_release_grounded = grounding.strong;
    let content_dna_mult = if is_registry_release && !registry_release_grounded {
        // Boost is unearned — neutralize it (don't penalize here; the hard
        // suppression below handles relevance).
        content_dna_mult.min(1.00)
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

    // Stale published-content discount (see `stale_published_multiplier`):
    // gated on the same `apply_freshness` switch as the freshness tiers —
    // both are temporal evidence, and the benchmark/simulation harnesses that
    // neutralize freshness expect temporal terms to stay neutral. Security
    // exemption covers both the novelty classifier's verdict and the raw
    // CVE/OSV source types (their zero-embedding retention path can miss the
    // classifier).
    let stale_mult = if options.apply_freshness {
        input.created_at.map_or(1.0, |published| {
            stale_published_multiplier(
                published,
                novelty.is_security || matches!(input.source_type, "cve" | "osv"),
                grounding.strong,
                // The classification computed THIS run (not the stored column
                // — stored/computed divergence is a documented hazard, see the
                // epochs registry comment).
                content_type == crate::content_dna::ContentType::ReleaseNotes,
            )
        })
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
        * stale_mult
        * domain_quality_mult
        * negative_stack_prior
        * tier_authority_mult;

    // ── Community multiplier (item-side signal only, v19 / AD-029) ─────
    // The unified "engagement multiplier" that lived here blended SIX terms,
    // four of them pure user history (affinity ×0.3–1.7, anti-topics,
    // feedback boosts, taste embedding, learned source quality) into a
    // ×[0.50, 1.60] swing on the composite.
    // retired-ok: documents the AD-029 demotion itself
    // Behavioral learning was demoted
    // from scoring authority (AD-029): its capture layer mixed three
    // incompatible strength scales, it self-poisoned twice (2026-07-13 doom
    // loop, 2026-08-11 calibration curve), and it never had enough clean
    // data to measure a lift. What remains is the COMMUNITY term — an
    // item-side popularity signal (upvotes/comments on the item itself,
    // not user behavior) — at its original weight and a clamp preserving
    // its original contribution range.
    let community_effect = community_signal - 0.5;
    let engagement_sum = community_effect * scoring_config::ENGAGEMENT_WEIGHTS_COMMUNITY_W;
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

    // Ungrounded registry releases: hard suppression (analog of the CVE
    // no-match tier above). A release of a package the user does NOT depend
    // on is registry noise no matter how stack-shaped its name is — the
    // registry signal this product sells is "releases of YOUR dependencies".
    // Grounded releases (subject-corroborated, e.g. tokio/serde/sqlx) are
    // untouched, as are security-classified items (governed by the security
    // validation above — no double penalty).
    let quality_score = if is_registry_release && !registry_release_grounded && !novelty.is_security
    {
        quality_score * scoring_config::REGISTRY_RELEASE_GROUNDING_UNGROUNDED_PENALTY
    } else {
        quality_score
    };

    // Community signal gate for user-generated content sources.
    // Low-community-signal items from UGC platforms (dev.to, medium, hashnode,
    // reddit, stackoverflow) get hard-capped — prevents generic blog posts and
    // zero-upvote questions from riding keyword matches into the briefing.
    // Authoritative sources (CVE, RustSec, GitHub, crates.io, npm, PyPI) are exempt.
    // Federated/microblog sources (mastodon, lemmy, bluesky, twitter) are UGC
    // too — they were missing from this list, so no quality cap ever applied
    // to them (the other half of the neutral-community-arm escape fixed in
    // extract_community_signal). v19: the same predicate also arms
    // `score_ceiling`, re-asserted at the pipeline exit and in
    // `finalize_scores`, because Phase 6 boosts run after this cap.
    let quality_score = if community_signal < scoring_config::COMMUNITY_SIGNAL_LOW_THRESHOLD
        && is_low_community_ugc_source(input.source_type)
    {
        quality_score.min(0.50)
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

/// UGC platforms whose zero/low-community items are quality-capped at 0.50.
/// ONE predicate for both the Phase-5 cap and the v19 exit `score_ceiling`
/// so the two can never disagree on what counts as UGC.
fn is_low_community_ugc_source(source_type: &str) -> bool {
    matches!(
        source_type,
        "devto"
            | "medium"
            | "hashnode"
            | "reddit"
            | "stackoverflow"
            | "lobsters"
            | "mastodon"
            | "lemmy"
            | "bluesky"
            | "twitter"
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
    // Dependency boost (in bootstrap mode, 2x weight). The bootstrap
    // GRADUATION on feedback count survives AD-029 deliberately: it is a
    // cold-start threshold policy (how strict to be while evidence is
    // thin), not an engagement-derived score weight — and the calibrated
    // persona baselines depend on established users keeping the 1x weight.
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
            .filter(|t| ctx.work_topics.iter().any(|wt| topic_grounds(t, wt)))
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
                    .find(|g| topic_grounds(t, &g.dependency))
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
    // Note: feedback_boost is handled in the Phase 5 unified engagement
    // formula, not here.
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

/// True when the low-signal direct-dependency gate bypass applies: the item's
/// only confirmed axis is (at most) a STRONG dependency match. Shared by
/// `apply_gate_effect` and the persisted `confirmation_mult` so the breakdown
/// always reports the multiplier the item was actually scored with.
///
/// `signal_count == 0` is structurally unreachable here in practice — a
/// dep_match_score at or above the bypass minimum (0.35) clears the 0.20
/// dependency signal threshold, so the dependency axis itself confirms — but
/// the `<= 1` guard keeps the two consumers trivially identical.
fn dep_gate_bypass_applies(signal_count: u8, dep_match_score: f32) -> bool {
    signal_count <= 1
        && dep_match_score >= scoring_config::DEPENDENCY_GATE_BYPASS_DIRECT_DEP_MIN_SCORE
}

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
    // single-axis territory, raise the ceiling so it isn't capped at 0.28 —
    // AND lift the confidence multiplier to the 2-signal tier (1.00). The
    // ceiling alone was an arithmetic contradiction (2026-08-23 audit, item
    // 22b): raising the cap to 0.72 while the 1-signal conf_mult (0.45) held
    // output to ~0.52 meant post-bootstrap single-axis dep releases could
    // never clear the 0.70 quality-floor relevance escape. A corroborated
    // direct-dep match at bypass strength is real evidence; the 0.72 ceiling
    // still holds it out of the top band. Without this, serde/tokio/axum
    // release notes score ~48% instead of 70%+ because dependency is the only
    // confirmed axis for package-specific content.
    let bypass = dep_gate_bypass_applies(signal_count, dep_match_score);
    let score_ceiling = if bypass {
        score_ceiling.max(scoring_config::DEPENDENCY_GATE_BYPASS_DIRECT_DEP_CEILING)
    } else {
        score_ceiling
    };
    let conf_mult = if bypass {
        conf_mult.max(scoring_config::CONFIRMATION_GATE[2].0)
    } else {
        conf_mult
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
    //
    // De-saturation (2026-07-13): the old hard
    // `.min(score_ceiling).clamp(0, 0.95)` was the pipeline's only
    // NON-INJECTIVE operation — it flattened every strong item onto a single
    // mass point (362 live items persisted the identical 0.9017062 =
    // soft_ceiling(0.95 + offset)), destroying ranking exactly where it
    // matters most and reducing top-band order to the necessity tiebreaker.
    // Soft-compress instead: `soft_ceiling` hard-mins when the cap is at or
    // below the knee (so the 0.20 / 0.28 / 0.72 noise-suppression tiers keep
    // their exact semantics) and maps `(knee, ∞)` injectively onto
    // `(knee, cap)` for the high tiers — distinct strong items stay distinct,
    // order preserved, and the "no item displays 100%" invariant still holds
    // because the cap never exceeds FINAL_CEILING_ABSOLUTE_MAX.
    let effective_cap = score_ceiling.min(scoring_config::FINAL_CEILING_ABSOLUTE_MAX);
    soft_ceiling(gated * domain_gate_mult, SOFT_CEILING_KNEE, effective_cap).max(0.0)
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
    strongly_grounded: bool,
    ungrounded_registry_release: bool,
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
        strongly_grounded,
        ungrounded_registry_release,
    )
}

/// Hard ceiling for commodity content types.
///
/// Standard exemptions (Tutorial/HelpRequest/Question/ShowAndTell — any bypasses):
/// - CVE/GHSA pattern in title
/// - Version conflict language with version number
/// - Content type overridden to SecurityAdvisory or BreakingChange (already excluded)
/// - Sophistication >= 0.35 (has advanced terms, version specificity, or abstract framing)
/// - High community validation (community_signal >= high_threshold)
///
/// AcademicPaper gets a STRICTER bypass set: dense academic prose trips the
/// sophistication heuristic on virtually every paper, and arXiv/PwC metadata
/// carries no crowd validation comparable to HN/SO scores — so neither of
/// those may lift the ceiling. Only evidence tied to the user's actual stack
/// does: strong dependency grounding (`strongly_grounded`, the same
/// `dependencies::is_strongly_grounded` predicate the gate uses) or a
/// security/version pattern in the title.
fn apply_commodity_ceiling(
    score: f32,
    title: &str,
    content_type: &crate::content_dna::ContentType,
    sophistication_raw: f32,
    community_signal: f32,
    strongly_grounded: bool,
    ungrounded_registry_release: bool,
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

    // Off-stack security advisory: a CVE/GHSA for a package NOT in the user's
    // dependency graph. It matches the security vocabulary (so it would sail
    // past every other exemption below via has_security_pattern) but has no
    // bearing on this developer's stack — awareness-only, never CORE. Checked
    // BEFORE the security exemption precisely because the advisory IS a security
    // pattern. In-stack advisories (strongly_grounded) fall through untouched.
    if matches!(content_type, ContentType::SecurityAdvisory) && !strongly_grounded {
        return score.min(scoring_config::COMMODITY_CEILING_SECURITY_ADVISORY_UNGROUNDED);
    }

    // Ungrounded registry release: a release/deprecation notice for a package
    // the user does NOT depend on (canonical subject-grounding verdict). Like
    // the off-stack advisory above, it is checked before every exemption:
    // stack-token keyword hits, open-window intent matches, and skill-gap
    // boosts all key on the look-alike NAME ("forge-plugin-sdk-RUST",
    // "cobol_rust_SERDE"), so none of them may lift the cap — and a "deprecated
    // vX" title for a non-dependency is still noise, so the version-conflict
    // exemption must not apply either. Capped below the relevance threshold:
    // visible in ranked lists, never feed-relevant.
    if ungrounded_registry_release {
        return score.min(scoring_config::COMMODITY_CEILING_REGISTRY_RELEASE_UNGROUNDED);
    }

    // Only applies to commodity types
    let ceiling = match content_type {
        ContentType::Tutorial => scoring_config::COMMODITY_CEILING_TUTORIAL,
        ContentType::HelpRequest => scoring_config::COMMODITY_CEILING_HELP_REQUEST,
        ContentType::Question => scoring_config::COMMODITY_CEILING_QUESTION,
        // Self-promo without traction is commodity; a Show-HN with real
        // community validation earns its slot via the standard bypasses.
        ContentType::ShowAndTell => scoring_config::COMMODITY_CEILING_SHOW_AND_TELL,
        ContentType::AcademicPaper => scoring_config::COMMODITY_CEILING_ACADEMIC,
        // Job/hiring posts — capped like academic (no crowd/sophistication lift).
        ContentType::Hiring => scoring_config::COMMODITY_CEILING_HIRING,
        _ => return score,
    };

    if matches!(
        content_type,
        ContentType::AcademicPaper | ContentType::Hiring
    ) {
        // Papers and job posts: sophistication and community-signal bypasses
        // deliberately withheld (see fn doc). For papers, strong dependency
        // grounding is the one class-specific exemption (a paper dissecting your
        // dep's internals is worth it); a job ad name-dropping your stack is
        // still a job ad, so hiring gets no grounding bypass either.
        if matches!(content_type, ContentType::AcademicPaper) && strongly_grounded {
            return score;
        }
    } else {
        // High community validation bypasses ceiling — the crowd validated this content
        if community_signal >= scoring_config::COMMUNITY_SIGNAL_HIGH_THRESHOLD {
            return score;
        }

        // Sophistication above threshold = not commodity
        if sophistication_raw >= 0.35 {
            return score;
        }
    }

    // Security/version exemptions (all classes — a paper documenting a CVE or
    // a Show-HN about a breaking migration is actionable regardless of class)
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

/// Boundary knee for [`finalize_scores`]. Sits ABOVE the maximum value
/// `score_item` can emit, so the boundary pass is the IDENTITY for pipeline
/// outputs and only compresses reranker overwrites:
///
///   score_item's terminal soft ceiling receives at most
///   gate-asymptote (0.95) + score offset (0.02) + topic-attention max (0.05)
///   = 1.02, and soft(1.02, 0.80, 0.95) ≈ 0.915.
///
/// Values above this knee can only come from post-pipeline writers (the
/// cross-encoder blend, the reconciler's final_rank, dedup boosts) whose
/// outputs range up to ~1.0 — those are compressed injectively into
/// (0.92, 0.95), preserving their relative order. Pre-2026-07-13 the boundary
/// re-applied the 0.80-knee ceiling to ALREADY-CEILINGED values, so the same
/// item persisted 0.9017 via the backfill path (no boundary call) but 0.8739
/// via the live analyzer path (double compression).
const BOUNDARY_CEILING_KNEE: f32 = 0.92;

#[cfg(test)]
mod desaturation_tests {
    use super::*;

    /// The 362-way-tie regression: distinct strong inputs must stay distinct
    /// through the gate. Pre-fix, every 4/5-signal item with
    /// `gated * domain ≥ 0.95` collapsed onto exactly 0.95 (then a fixed
    /// offset + soft ceiling relabeled the pile to 0.9017062 — 362 identical
    /// persisted scores on the live corpus).
    #[test]
    fn gate_does_not_flatten_strong_items() {
        let ctx = ScoringContext::builder().build();
        // 4 confirmed signals → tier ceiling 1.0; the old absolute clamp was
        // the only binding cap. Three distinct strong scores:
        let a = apply_gate_effect(0.80, 4, 0.9, &ctx, 0.10, 0.0);
        let b = apply_gate_effect(0.90, 4, 0.9, &ctx, 0.10, 0.0);
        let c = apply_gate_effect(1.00, 4, 0.9, &ctx, 0.10, 0.0);
        assert!(a < b && b < c, "order must be preserved: {a} {b} {c}");
        assert!(
            (b - a) > 1e-4 && (c - b) > 1e-4,
            "strong items must stay separated, got {a} {b} {c}"
        );
        assert!(
            c < scoring_config::FINAL_CEILING_ABSOLUTE_MAX,
            "absolute-max invariant must hold, got {c}"
        );
    }

    /// Low/noise tiers keep their EXACT hard-cap semantics (soft_ceiling
    /// hard-mins whenever cap <= knee) — noise suppression must not soften.
    #[test]
    fn low_signal_tiers_keep_hard_caps() {
        let ctx = ScoringContext::builder().build();
        // 0/1-signal ceilings (0.20 / 0.28 + bonus) are far below the knee.
        let zero = apply_gate_effect(1.0, 0, 0.9, &ctx, 0.0, 0.0);
        let one = apply_gate_effect(1.0, 1, 0.9, &ctx, 0.0, 0.0);
        assert!(zero <= 0.20 + 1e-6, "0-signal cap must stay hard: {zero}");
        assert!(one <= 0.28 + 1e-6, "1-signal cap must stay hard: {one}");
    }

    /// The boundary pass is the IDENTITY for anything score_item can emit —
    /// calling it on every persistence path yields path-independent scores.
    /// (finalize_scores is a trivial loop over this same map, so the float
    /// map IS the behavior under test.)
    #[test]
    fn boundary_finalize_is_identity_for_pipeline_outputs() {
        let boundary = |v: f32| {
            soft_ceiling(
                v,
                BOUNDARY_CEILING_KNEE,
                scoring_config::FINAL_CEILING_ABSOLUTE_MAX,
            )
        };
        // Max pipeline output ≈ soft(1.02, 0.80, 0.95) ≈ 0.915 < knee 0.92.
        let max_pipeline = apply_final_soft_ceiling(1.02);
        assert!(
            max_pipeline < BOUNDARY_CEILING_KNEE,
            "boundary knee must clear the pipeline max ({max_pipeline})"
        );
        for v in [0.10_f32, 0.50, 0.80, 0.9017, max_pipeline] {
            assert!(
                (boundary(v) - v).abs() < 1e-6,
                "boundary must not re-compress pipeline output {v}, got {}",
                boundary(v)
            );
        }
        // …while reranker overwrites above the knee are compressed under the
        // absolute max, injectively (order preserved).
        let (a, b) = (boundary(0.96), boundary(0.99));
        assert!(a < b, "order preserved: {a} vs {b}");
        assert!(b < scoring_config::FINAL_CEILING_ABSOLUTE_MAX);
    }
}

/// THE single authoritative score-shaping boundary. Call at the end of EVERY
/// analysis path (cached, fresh, deep-scan, backfill, headless) so the stored
/// `relevance_score` honors the `final_ceiling.absolute_max` invariant no
/// matter which reranker last overwrote `top_score`. IDEMPOTENT for values
/// `score_item` produces (identity below the boundary knee), so calling it on
/// every path yields path-independent persisted scores. Does NOT reorder —
/// each path keeps its own sort / composition-floor logic.
pub(crate) fn finalize_scores(results: &mut [crate::SourceRelevance]) {
    for r in results.iter_mut() {
        // Re-assert categorical ceilings FIRST. `score_item` caps commodity
        // items (e.g. ungrounded registry releases at ceiling+offset), but
        // the cross-encoder rerank, dedup cluster boost, source-tier
        // normalizer, and LLM reconciler all overwrite `top_score` after
        // the pipeline. The verdict is already categorical (v18); this
        // makes the SCORE hold the same line, so a capped item can never
        // be re-inflated into the top of the feed by a post-pipeline
        // writer (v19).
        if let Some(ceiling) = r.score_breakdown.as_ref().and_then(|b| b.score_ceiling) {
            r.top_score = r.top_score.min(ceiling);
        }
        r.top_score = soft_ceiling(
            r.top_score,
            BOUNDARY_CEILING_KNEE,
            scoring_config::FINAL_CEILING_ABSOLUTE_MAX,
        );
    }
}

// ============================================================================
// Signal classification (mirrors V1 logic)
// ============================================================================

/// Build the action line for a Critical security alert.
///
/// The action line is EVIDENCE rendered inside a Critical alert, so it may only
/// name a package the item demonstrably corroborates. The escalation gate above
/// tests `grounding.strong`, which is a DIFFERENT source of truth from
/// `is_strong_grounding_match`: on the registry-subject route
/// `compute_grounding_verdict` returns `strong: true` WITHOUT inspecting
/// `deps` at all, so `strong` can be true while zero `DepMatch` passes the
/// filter here. The previous code closed that case with
/// `.unwrap_or(&matched_deps[0])` — an arbitrary positional pick, in practice an
/// uncorroborated alias/subterm hit ("tauri-apps-plugin-opener" on an article
/// that never mentions Tauri) — and printed it as the affected dependency.
///
/// When nothing passes the filter, OMIT the name rather than guess one. The
/// alert is still true (the item's registry subject is one of the user's
/// manifest dependencies, which is what made grounding strong); only the
/// identification is unavailable. This mirrors the trigger-chip rule a few lines below, which
/// filters on `corroborated` and emits nothing rather than falling back.
fn critical_security_action(matched_deps: &[dependencies::DepMatch]) -> String {
    let best_dep = matched_deps
        .iter()
        .filter(|d| dependencies::is_strong_grounding_match(d))
        .max_by(|a, b| {
            a.confidence
                .partial_cmp(&b.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    match best_dep {
        Some(dep) => format!(
            "Critical: Security issue affects your dependency {}",
            dep.package_name
        ),
        None => "Critical: Security issue affects one of your dependencies".to_string(),
    }
}

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
    grounding: dependencies::GroundingVerdict,
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
                let has_strong_dep = grounding.strong;
                if c.signal_type == signals::SignalType::SecurityAlert && has_strong_dep {
                    c.priority = signals::SignalPriority::Critical;
                    c.action = critical_security_action(matched_deps);
                } else if c.signal_type == signals::SignalType::BreakingChange
                    && matched_deps
                        .iter()
                        .any(|d| d.version_delta == dependencies::VersionDelta::NewerMajor)
                    && c.priority < signals::SignalPriority::Alert
                {
                    c.priority = signals::SignalPriority::Alert;
                }
                // Trigger chips are rendered as "why flagged" evidence — only
                // corroborated matches may mint them (an uncorroborated alias
                // hit would put "dep:tauri-apps-plugin-opener" on an article
                // that never mentions Tauri).
                for dep in matched_deps.iter().filter(|d| d.corroborated).take(2) {
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
                let has_strong_direct_dep = grounding.strong_direct;
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
        // ACE anti-topic auto-exclusions removed in v19 (AD-029): inferred
        // negative filtering carried the same broken-capture authority as
        // the rest of the behavioral stack (a topic could be auto-banned by
        // dismissal-count alone). User-authored `exclusions` (the
        // suppress-topic button) remain the explicit suppression path.
        ;

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
            evidence_score: 0.0,
            rank_factors: None,
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

    // ── Degraded-input markers (2026-08-23 audit, item 11) ────────────
    // A run whose inputs silently collapsed (KNN failure, dep-intel load
    // failure, absent embedding) still produces scores — and those scores
    // would overwrite good durable ones. This vector carries the honest
    // state on the persisted breakdown; persistence POLICY (skip / re-score)
    // is decided downstream (analysis_status wave), not here.
    let mut degraded_inputs: Vec<String> = Vec::new();
    if !has_real_embedding {
        // Zero/absent embedding: the semantic similarity axes (context KNN,
        // interest) default rather than measure. Zero blobs exist by design
        // (OSV/CVE retention while embedding providers are down) — that is
        // exactly a degraded input, not a healthy one.
        degraded_inputs.push("embedding_missing".to_string());
    }
    if dependencies::dep_intel_load_degraded() {
        // The ACE context for this run was built while the dependency
        // intelligence load failed — the dependency axis is empty-by-error,
        // indistinguishable in the scores from "user has no deps".
        degraded_inputs.push("dep_intel_load_failed".to_string());
    }

    let matches: Vec<RelevanceMatch> = if ctx.cached_context_count > 0 && has_real_embedding {
        // A DB error here silently zeroes the CONTEXT axis for this item —
        // indistinguishable from "no match" downstream. Degrade gracefully,
        // but never silently (accuracy-first: a muted axis must be visible in
        // the logs AND on the persisted breakdown, 2026-08-21 audit).
        match db.find_similar_contexts(input.embedding, 3) {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!(
                    target: "4da::scoring",
                    error = %e,
                    item_id = input.id,
                    "find_similar_contexts failed — context axis degraded to zero for this item"
                );
                degraded_inputs.push("context_knn_failed".to_string());
                Vec::new()
            }
        }
        .into_iter()
        // Boilerplate chunks (shebangs, license headers) match everything
        // and were surfacing as top "Similar to your code" evidence on
        // unrelated items. The chunker no longer indexes them; this filter
        // protects users whose existing DBs still contain them — filtering
        // here also keeps them out of the context score itself, not just
        // the displayed evidence.
        .filter(|result| !crate::utils::is_boilerplate_chunk(&result.text))
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
    let confirmation = gate::count_confirmed_signals_with_evidence(
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
        gate::GateEvidence {
            semantic_is_embedding_derived: raw.semantic_is_embedding_derived,
            own_stack_keyword_score: raw.own_stack_keyword_score,
        },
    );
    let signal_count = confirmation.count;
    let confirmed_signals = confirmation.confirmed_names();

    // ── Phase 4: Compute base relevance ───────────────────────────────
    let relevance_score = compute_relevance(&cal, ctx, has_real_embedding);

    // ── Canonical grounding verdict ───────────────────────────────────
    // Computed ONCE and shared by every downstream consumer: the quality
    // composite's registry-release grounding, the commodity bypass, the
    // critical fast-path floors, the Critical trust gate, the necessity
    // stack-update path, and the persisted breakdown (→ the evidence pool).
    // Registry items are judged by their SUBJECT package (source_id), never
    // by text mentions in another package's description.
    let grounding = dependencies::compute_grounding_verdict(
        input.source_type,
        input.source_id,
        &raw.matched_deps,
        &ctx.ace_ctx,
    );

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
    ) = compute_quality_composite(relevance_score, input, ctx, &raw, options, db, &grounding);

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

    // Trend topic boost (temporal clustering signal). Raw trend detection is
    // corpus-wide; require domain relevance so off-domain repeated feed noise
    // cannot earn a developer-facing trend boost.
    let trend_boost = if raw.domain_relevance >= 0.5
        && !options.trend_topics.is_empty()
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
        raw.stack_pain_match,
    );

    // ── Phase 7: Gate effect ──────────────────────────────────────────
    let conf_idx = (signal_count as usize).min(5);
    // Persist the multiplier the item is ACTUALLY scored with: when the
    // direct-dep gate bypass fires, `apply_gate_effect` lifts the 1-signal
    // multiplier to the 2-signal tier — the breakdown must say so.
    let confirmation_mult = if dep_gate_bypass_applies(signal_count, raw.dep_match_score) {
        scoring_config::CONFIRMATION_GATE[conf_idx]
            .0
            .max(scoring_config::CONFIRMATION_GATE[2].0)
    } else {
        scoring_config::CONFIRMATION_GATE[conf_idx].0
    };
    let gated_score = apply_gate_effect(
        boosted_score,
        signal_count,
        raw.domain_relevance,
        ctx,
        strength_bonus,
        raw.dep_match_score,
    );

    // ── Canonical grounding verdict ───────────────────────────────────
    // Computed ONCE and shared by every downstream consumer: the commodity
    // bypass, the critical fast-path floors, the Critical trust gate, the
    // ── Phase 8: Final adjustments ────────────────────────────────────
    // Ungrounded registry release: subject package is NOT one of the user's
    // dependencies (canonical verdict above). Rides the same post-everything
    // commodity-ceiling slot as off-stack advisories — no amount of
    // intent/window/skill-gap boost may push a look-alike crate release back
    // into the feed after the grounding penalty.
    let ungrounded_registry_release = crate::dep_linker::is_registry_source(input.source_type)
        && matches!(
            content_type,
            crate::content_dna::ContentType::ReleaseNotes
                | crate::content_dna::ContentType::BreakingChange
        )
        && !grounding.strong;
    let combined_score = apply_final_adjustments(
        gated_score,
        input.title,
        &content_type,
        sophistication_raw,
        community_signal,
        // Same grounding evidence the gate consumes — lets a dep-grounded
        // academic paper bypass its commodity ceiling.
        grounding.strong,
        ungrounded_registry_release,
    );

    // ── Score offset normalization ────────────────────────────────────
    // Guarantees all scores are positive. Separates scored items from
    // truly-unknown (zero) items by shifting up by floor amount.
    let combined_score = normalize_score_offset(combined_score);

    // The topic-attention-gap boost that lived here (+0.00–0.05 for
    // positive-affinity topics unseen >48h) was REMOVED in v19 (AD-029,
    // behavioral-learning demotion). It was the v18 incident's mechanism —
    // an engagement-derived additive term running AFTER the commodity cap,
    // hardcoded outside the DSL, with an explicit "no hard clamp" note —
    // and it rewarded previously-engaged topics on items already known to
    // be ungrounded. Behavioral signals no longer carry scoring authority.

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
    // non-dev, confidence >= 0.40, non-ambiguous package name, and — since
    // v11 — name-corroborated: the item actually names the package) is the
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
        && grounding.strong;
    let critical_fast_path = (is_security || is_breaking) && has_strong_dep_match;

    // A CVE confirmed against the user's DIRECT (non-dev) dependency is the
    // flagship preemption case and the highest-confidence security signal — it
    // floors higher than a generic match so a pure-dep-signal advisory (weak
    // embedding, no topic overlap) still scores clearly relevant instead of
    // sitting at the bare 0.50 floor. The higher tier requires the direct dep
    // itself to be the strongly grounded edge (canonical predicate), not just
    // any direct dep riding alongside a grounded transitive match.
    let has_direct_dep = grounding.strong_direct;
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

    // ── Categorical ceiling re-assertion at the pipeline EXIT (v19) ────
    // The commodity and UGC caps fire in Phase 5 (quality), but Phase 6
    // additive boosts (intent/stack/window/skill-gap), the fast-path
    // floors, and the score offset all run AFTER them — the exact
    // cap-before-boosts ordering bug that produced the v18 incident (a
    // 0.35-capped look-alike landing at 0.42). Live proof of the UGC
    // twin: a zero-engagement mastodon post held to 0.50 by the Phase-5
    // cap exited at 0.84 after boosts (masked pre-v19 by an anti-topic
    // exclusion). The ceiling is enforced here — after every additive
    // term — persisted on the breakdown, and re-asserted once more in
    // `finalize_scores` so post-pipeline writers cannot reopen it either.
    let score_ceiling: Option<f32> = {
        let commodity = ungrounded_registry_release.then_some(
            scoring_config::COMMODITY_CEILING_REGISTRY_RELEASE_UNGROUNDED
                + scoring_config::SCORE_OFFSET_NEGATIVE_FLOOR,
        );
        // Critical fast-path items are exempt from the UGC ceiling: a
        // strongly-grounded security advisory shared on a UGC platform
        // must keep its floor (grounding beats popularity capping). The
        // commodity ceiling needs no such exemption — it is mutually
        // exclusive with the fast path by construction.
        let ugc = (!critical_fast_path
            && community_signal < scoring_config::COMMUNITY_SIGNAL_LOW_THRESHOLD
            && is_low_community_ugc_source(input.source_type))
        .then_some(0.50);
        match (commodity, ugc) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    };
    let combined_score = match score_ceiling {
        Some(ceiling) => combined_score.min(ceiling),
        None => combined_score,
    };

    // ── Relevance determination ───────────────────────────────────────
    // The bootstrap relaxation (1 signal while feedback_interaction_count
    // < 10, then the 2-signal quality floor) survives AD-029 deliberately:
    // graduating to a STRICTER gate as explicit feedback accumulates is a
    // cold-start threshold policy, not an engagement-derived score weight,
    // and the enriched-persona simulation baselines depend on it
    // (federated-social noise is held out by the 2-signal floor there).
    let bootstrap_mode = ctx.feedback_interaction_count < 10;
    let min_signals = if bootstrap_mode {
        1u8
    } else {
        scoring_config::QUALITY_FLOOR_MIN_SIGNALS as u8
    };
    // The critical fast-path is score-independent, so it must also respect
    // the language gate: V1 never let a language-mismatched item be relevant,
    // and a 0.05-capped "relevant" item would be contradictory.
    //
    // v18: look-alike registry releases are CATEGORICALLY never feed-relevant.
    // The 0.35 commodity ceiling alone could not hold that line: the ceiling is
    // applied inside `apply_final_adjustments`, but `normalize_score_offset`
    // (+0.02) and the topic-attention-gap boost (+0.05) both run AFTER it, so a
    // capped look-alike landed at exactly 0.42 — above the 0.40 threshold.
    // Production evidence (2026-07-26, live corpus): 350 capped items piled at
    // 0.37 (ceiling+offset) and 84 at exactly 0.42 (ceiling+offset+full boost),
    // of which 26 were already `feed_relevant = 1`. Gating the VERDICT rather
    // than the score makes the invariant score-independent, so no future
    // post-ceiling boost can reopen it. `critical_fast_path` requires
    // `grounding.strong`, which `ungrounded_registry_release` negates, so a
    // genuine security fast-path is structurally unreachable here — the gate
    // costs zero recall. The score keeps its capped value for ranking/display.
    let relevant = !ungrounded_registry_release
        && ((critical_fast_path && !lang_mismatch)  // Critical items always relevant
            || (combined_score >= get_relevance_threshold()
                && (signal_count >= min_signals
                    || combined_score >= scoring_config::QUALITY_FLOOR_MIN_SCORE)));

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

    // ── Display-worthy dependency evidence ────────────────────────────
    let advisory_ecosystems = extract_advisory_ecosystems(input.content);
    let display_deps = display_worthy_deps(&raw.matched_deps, &advisory_ecosystems);
    let matched_dep_names: Vec<String> = display_deps
        .iter()
        .map(|d| d.package_name.clone())
        .collect();

    // ── Signal classification ─────────────────────────────────────────
    let (sig_type, mut sig_priority, sig_action, sig_triggers, sig_horizon) = classify_signals(
        relevant,
        combined_score,
        raw.domain_relevance,
        &content_type,
        options,
        classifier,
        input,
        ctx,
        &raw.matched_deps,
        grounding,
        db,
    );

    // ── Security version evidence (extracted BEFORE necessity/applicability
    //    so the version verdict can inform both) ───────────────────────
    let is_security_source = matches!(input.source_type, "cve" | "osv");
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
    // Version/path evidence comes from the strongest DISPLAY dep — showing the
    // installed version of an uncorroborated alias hit was quietly dishonest.
    let dep_path = display_deps.first().map(|dep| {
        if dep.is_dev {
            "dev-only".to_string()
        } else if !dep.is_direct {
            "transitive".to_string()
        } else {
            "direct".to_string()
        }
    });
    let installed_version = display_deps.first().and_then(|d| d.version.clone());
    let is_version_affected = check_version_affected(
        installed_version.as_deref(),
        affected_versions.as_deref(),
        fixed_version.as_deref(),
    );

    // A CONFIRMED not-affected advisory (installed version outside the affected
    // range / at-or-past the fix) is awareness at most — never Critical/Alert.
    // The OSV backfill floods dozens of historical, long-patched advisories per
    // package; without this they all page as if they endanger today's build.
    let version_negative =
        sig_type.as_deref() == Some("security_alert") && is_version_affected == Some(false);
    if version_negative
        && matches!(
            sig_priority.as_deref(),
            Some("critical") | Some("alert") | Some("advisory")
        )
    {
        sig_priority = Some("watch".to_string());
    }

    // ── Necessity scoring ─────────────────────────────────────────────
    let age_hours = input.created_at.map_or(0.0, |ts| {
        (chrono::Utc::now() - *ts).num_minutes().max(0) as f64 / 60.0
    });

    // Security severity evidence feeds the necessity bucket below, so extract it
    // BEFORE building NecessityInputs. Previously it was computed afterward, so a real
    // critical CVE on a dev-only dep (which can reach the security path with no signal
    // priority) fell back to "medium" instead of critical (bug J).
    // (is_security_source computed above with the version evidence block.)
    let (cvss_score, cvss_severity) = if is_security_source {
        extract_cvss_from_content(input.content)
    } else {
        (None, None)
    };

    // Window title resolved in-memory (open_windows is already loaded) so the
    // decision-relevant necessity reason can name the decision it claims relevance to.
    let matched_window_label = matched_window_id.and_then(|wid| {
        ctx.open_windows
            .iter()
            .find(|w| w.id == wid)
            .map(|w| w.title.clone())
    });

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
        matched_window_label: matched_window_label.clone(),
        age_hours,
        content_type: Some(content_type.slug().to_string()),
        strongly_grounded: grounding.strong,
        version_affected: is_version_affected,
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
    // The version verdict overrides the name/ecosystem route: a CONFIRMED
    // not-affected advisory (installed version outside the affected range /
    // at-or-past the fix) is `not_affected` and never a critical alert — the
    // evidence pool keeps it out of "Affects You".
    let (applicability, is_critical_alert) = if version_negative {
        (Some("not_affected".to_string()), false)
    } else if sig_type.as_deref() == Some("security_alert") {
        security_applicability(&raw.matched_deps, &advisory_ecosystems)
    } else {
        (None, false)
    };

    // (advisory_id / fixed_version / affected_versions / dep_path /
    // installed_version / is_version_affected extracted above, before
    // necessity, so the version verdict informs it.)
    let sec_affected_project_count = count_affected_projects(db, &matched_dep_names) as u32;

    // ── Explanation evidence chain ────────────────────────────────────
    // Built from the SAME values the pipeline scored with; the subtitle is
    // rendered from the chain so every surface reads one explanation source.
    let is_security_necessity =
        necessity_result.category == necessity::NecessityCategory::SecurityVulnerability;
    let explanation_factors =
        explanation_chain::build_explanation_chain(&explanation_chain::ChainInputs {
            title: input.title,
            item_topics: &raw.topics,
            ace_ctx: &ctx.ace_ctx,
            interests: &ctx.interests,
            declared_tech: &ctx.declared_tech,
            matches: &matches,
            display_deps: &display_deps,
            dep_match_score: raw.dep_match_score,
            context_score: cal.context_score,
            interest_score: cal.interest_score,
            keyword_score: cal.keyword_score,
            ace_boost: cal.semantic_boost,
            window_boost,
            matched_window_label: matched_window_label.as_deref(),
            skill_gap_boost,
            matched_skill_gaps: &matched_skill_gaps,
            is_security: is_security_necessity,
            necessity_score: necessity_result.score,
            advisory_id: advisory_id.as_deref(),
            cvss_score,
            cvss_severity: cvss_severity.as_deref(),
            fixed_version: fixed_version.as_deref(),
            installed_version: installed_version.as_deref(),
            via_registry_subject: grounding.via_registry_subject,
        });
    let explanation = if relevant || combined_score >= 0.3 {
        explanation_chain::render_subtitle(&explanation_factors)
    } else {
        None
    };

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
        strongly_grounded: grounding.strong,
        degraded_inputs,
        // Categorical ceiling for post-pipeline writers: a capped item
        // (ungrounded registry release, zero-engagement UGC) must never
        // rank above its ceiling no matter what the cross-encoder, dedup
        // boost, source-tier normalizer, or LLM reconciler add later —
        // `finalize_scores` re-asserts this after every writer.
        score_ceiling,
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
        explanation_factors,
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
        // The EVIDENCE snapshot (audit items 12+26): identical to top_score at
        // construction; batch-relative writers downstream mutate top_score only.
        evidence_score: combined_score,
        rank_factors: None,
    }
}

/// The ONLY dependency list the UI (and every downstream reason string) sees:
/// matches the item demonstrably names (`corroborated`), or matches the
/// advisory's own affected-package metadata confirms. The raw match list
/// additionally carries subterm/alias-expansion hits ("opener" →
/// tauri-apps-plugin-opener) that are legitimate weak SCORING signals but are
/// NOT evidence — rendering them produced "matches: vercel,
/// tauri-apps-plugin-opener, ..." on articles that never mention Tauri.
/// Ordered by confidence descending so the strongest evidence leads.
fn display_worthy_deps(
    matched: &[DepMatch],
    advisory_ecosystems: &[(String, String)],
) -> Vec<DepMatch> {
    let mut display: Vec<DepMatch> = matched
        .iter()
        .filter(|d| d.corroborated || advisory_affects_dependency(advisory_ecosystems, d))
        .cloned()
        .collect();
    display.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    display
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
    // extract_community_signal — federated sources must not ride neutral
    // ========================================================================

    #[test]
    fn federated_sources_without_engagement_score_low() {
        // Pre-fix these hit the `_ => 0.50` neutral arm — never below the
        // 0.30 low_threshold, so the UGC cap could never fire for them.
        for source in ["mastodon", "lemmy", "bluesky", "twitter"] {
            let signal = extract_community_signal(source, None, 8.0);
            assert!(
                signal < scoring_config::COMMUNITY_SIGNAL_LOW_THRESHOLD,
                "{source} with no engagement metadata must arm the UGC cap \
                 (got {signal}, threshold {})",
                scoring_config::COMMUNITY_SIGNAL_LOW_THRESHOLD
            );
        }
    }

    #[test]
    fn federated_engagement_earns_score_back() {
        // Real community traction (mastodon favourites+reblogs, lemmy score,
        // bluesky likes) lifts the signal above the cap threshold.
        let mastodon = extract_community_signal(
            "mastodon",
            Some(r#"{"favourites": 40, "reblogs": 15}"#),
            8.0,
        );
        assert!(
            mastodon >= 0.85,
            "55 combined engagement → high ({mastodon})"
        );
        let lemmy = extract_community_signal("lemmy", Some(r#"{"score": 12}"#), 8.0);
        assert!(lemmy >= 0.60, "12 upvotes → moderate ({lemmy})");
        let bluesky = extract_community_signal("bluesky", Some(r#"{"likes": 4}"#), 8.0);
        assert!(
            (0.30..0.60).contains(&bluesky),
            "4 likes → modest but uncapped ({bluesky})"
        );
    }

    #[test]
    fn federated_fresh_items_get_no_neutral_grace() {
        // Item 2 (2026-08-23 audit): the <6h neutral grace made every young
        // UGC item score FREELY (cap disarmed) and then crash to the 0.50
        // ceiling at hour six — a scheduled cliff (yesterday's #1 feed item
        // fell 0.923→0.500 on schedule; 148 items pinned at exactly 0.50).
        // The UGC engagement signal now applies from age 0: a no-metadata
        // federated item is consistently capped from ingest.
        for source in ["mastodon", "lemmy", "bluesky", "twitter"] {
            let signal = extract_community_signal(source, None, 1.0);
            assert!(
                (signal - 0.25).abs() < f32::EPSILON,
                "{source} with no engagement at 1h must be 0.25, not the old \
                 free-pass 0.50 (got {signal})"
            );
        }
    }

    #[test]
    fn federated_fresh_items_with_engagement_earn_high_signal_immediately() {
        // Real engagement counts from minute one — no need to wait out a
        // window that no longer exists.
        let signal = extract_community_signal("mastodon", Some(r#"{"favourites": 50}"#), 1.0);
        assert!(
            (signal - 0.85).abs() < f32::EPSILON,
            "50 favourites at 1h → 0.85 (got {signal})"
        );
    }

    #[test]
    fn voted_sources_keep_fresh_item_neutral_grace() {
        // HN/SO/reddit scores accrue by community voting over the first
        // hours — "the community hasn't voted yet" is REAL there, so the
        // <6h neutral window stays (even when metadata says 0 points).
        let no_meta = extract_community_signal("hackernews", None, 1.0);
        assert!((no_meta - 0.50).abs() < f32::EPSILON);
        let zero_points = extract_community_signal("hackernews", Some(r#"{"score": 0}"#), 1.0);
        assert!(
            (zero_points - 0.50).abs() < f32::EPSILON,
            "a 1-hour-old HN item with 0 points is unjudged, not unpopular"
        );
        // After the window, the same zero-point item IS judged.
        let judged = extract_community_signal("hackernews", Some(r#"{"score": 0}"#), 8.0);
        assert!((judged - 0.30).abs() < f32::EPSILON);
    }

    // ========================================================================
    // display_worthy_deps — the corroborated-evidence gate for the UI
    // ========================================================================

    fn disp_dep(name: &str, confidence: f32, corroborated: bool) -> DepMatch {
        DepMatch {
            package_name: name.to_string(),
            confidence,
            version_delta: dependencies::VersionDelta::Unknown,
            is_dev: false,
            is_direct: true,
            version: None,
            ecosystem: "npm".to_string(),
            corroborated,
            raw_name: None,
        }
    }

    #[test]
    fn uncorroborated_deps_never_surface_for_display() {
        // The Coolify-card class: subterm/alias expansions populate the raw
        // match list but the item never names those packages. They must not
        // reach matched_deps-for-display.
        let raw = vec![
            disp_dep("vercel", 0.6, true),
            disp_dep("tauri-apps-plugin-opener", 0.35, false),
            disp_dep("tauri-apps-plugin-updater", 0.35, false),
        ];
        let display = display_worthy_deps(&raw, &[]);
        let names: Vec<&str> = display.iter().map(|d| d.package_name.as_str()).collect();
        assert_eq!(
            names,
            vec!["vercel"],
            "only the corroborated match may be displayed"
        );
    }

    #[test]
    fn display_deps_ordered_by_confidence_descending() {
        let raw = vec![
            disp_dep("low", 0.45, true),
            disp_dep("high", 0.95, true),
            disp_dep("mid", 0.70, true),
        ];
        let display = display_worthy_deps(&raw, &[]);
        let names: Vec<&str> = display.iter().map(|d| d.package_name.as_str()).collect();
        assert_eq!(names, vec!["high", "mid", "low"]);
    }

    #[test]
    fn advisory_metadata_confirms_uncorroborated_dep_for_display() {
        // The structured-advisory route is an independent proof: an OSV/CVE
        // whose Affected-list names the package confirms it even when the
        // prose text-corroboration failed.
        let raw = vec![disp_dep("quinn-proto", 0.5, false)];
        let advisory = vec![("quinn-proto".to_string(), "npm".to_string())];
        let display = display_worthy_deps(&raw, &advisory);
        assert_eq!(display.len(), 1);
        assert_eq!(display[0].package_name, "quinn-proto");
    }

    #[test]
    fn no_evidence_yields_empty_display_list() {
        let raw = vec![disp_dep("react", 0.9, false)];
        assert!(
            display_worthy_deps(&raw, &[]).is_empty(),
            "high confidence without corroboration is still not display evidence"
        );
    }

    // ========================================================================
    // critical_security_action — a Critical alert may only name a VERIFIED dep
    // ========================================================================

    #[test]
    fn critical_action_names_the_highest_confidence_grounded_dep() {
        let deps = vec![
            disp_dep("axios", 0.55, true),
            disp_dep("lodash", 0.92, true),
            disp_dep("sentry-react", 0.35, false),
        ];
        assert_eq!(
            critical_security_action(&deps),
            "Critical: Security issue affects your dependency lodash"
        );
    }

    /// The reachable defect: `compute_grounding_verdict` returns `strong: true`
    /// on the registry-subject route without ever inspecting `deps`, so this
    /// escalation can fire with a `matched_deps` list in which NOTHING passes
    /// `is_strong_grounding_match`. The old positional `matched_deps[0]`
    /// fallback then named an arbitrary uncorroborated alias hit inside a
    /// Critical alert. No name is better than the wrong name.
    #[test]
    fn critical_action_omits_the_name_when_no_dep_is_verified() {
        let deps = vec![
            // Uncorroborated: the item never names this package (alias/subterm
            // expansion), so it is not grounding evidence at any confidence.
            disp_dep("tauri-apps-plugin-opener", 0.95, false),
            disp_dep("tauri-apps-plugin-updater", 0.35, false),
        ];
        let action = critical_security_action(&deps);
        assert_eq!(
            action, "Critical: Security issue affects one of your dependencies",
            "an unverified match must never be named in a Critical alert"
        );
        for dep in &deps {
            assert!(
                !action.contains(dep.package_name.as_str()),
                "action must not name {}",
                dep.package_name
            );
        }
    }

    /// Below the STRONG_GROUNDING_CONFIDENCE floor, corroboration alone is not
    /// enough either — the same no-name rule applies.
    #[test]
    fn critical_action_omits_the_name_below_the_grounding_confidence_floor() {
        let deps = vec![disp_dep("react", 0.20, true)];
        assert_eq!(
            critical_security_action(&deps),
            "Critical: Security issue affects one of your dependencies"
        );
    }

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
            source_id: None,
        }
    }

    #[test]
    fn v2_language_mismatch_capped_and_not_relevant() {
        let db = crate::test_utils::test_db();
        let embedding = vec![0.5_f32; crate::EMBEDDING_DIMS];
        let (ctx, options) = lang_gate_fixture(&embedding);

        // Detect the user's current language at runtime and pick a
        // definitively different one.
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

    #[test]
    fn v2_ungrounded_registry_release_dampens_context_axis() {
        let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
        let input = ScoringInput {
            id: 1,
            title: "crates.io: rand v0.9.0",
            url: Some("https://crates.io/crates/rand"),
            content: "A new rand release is available",
            source_type: "crates_io",
            embedding: &zero,
            created_at: None,
            detected_lang: "",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
            source_id: Some("crate-rand"),
        };
        let matches = vec![RelevanceMatch {
            source_file: "src/lib.rs".to_string(),
            matched_text: "rust async runtime dependency code".to_string(),
            similarity: 0.90,
        }];

        let ungrounded = extract_signals(
            &input,
            &crate::scoring::ScoringContext::builder().build(),
            &matches,
        );
        assert!(
            (ungrounded.context - 0.27).abs() < 1e-6,
            "ungrounded registry release must dampen KNN context 0.90 -> 0.27, got {}",
            ungrounded.context
        );

        let grounded_ctx = fastpath_ctx(&[("rand", "rust")]);
        let grounded = extract_signals(&input, &grounded_ctx, &matches);
        assert!(
            (grounded.context - 0.90).abs() < 1e-6,
            "registry release for a corroborated dependency must keep raw context, got {}",
            grounded.context
        );
    }

    #[test]
    fn v2_trend_boost_requires_domain_relevance() {
        let db = crate::test_utils::test_db();
        let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
        let mut profile = crate::domain_profile::DomainProfile::default();
        profile.primary_stack.insert("rust".to_string());
        profile.all_tech.insert("rust".to_string());
        let ctx = crate::scoring::ScoringContext::builder()
            .domain_profile(profile)
            .build();

        let sports_tags = vec!["sports".to_string()];
        let sports_input = ScoringInput {
            id: 1,
            title: "Sports analytics startup raises new funding",
            url: Some("https://example.com/sports"),
            content: "Sports media rights and football growth",
            source_type: "rss",
            embedding: &zero,
            created_at: None,
            detected_lang: "",
            source_tags: &sports_tags,
            tags_json: None,
            feed_origin: None,
            source_id: None,
        };
        let no_trend = score_item(&sports_input, &ctx, &db, &fastpath_options(), None);
        let sports_options = ScoringOptions {
            trend_topics: vec!["sports".to_string()],
            ..fastpath_options()
        };
        let with_trend = score_item(&sports_input, &ctx, &db, &sports_options, None);
        assert!(
            (with_trend.top_score - no_trend.top_score).abs() < f32::EPSILON,
            "off-domain repeated feed topic must not get trend boost: base={}, trend={}",
            no_trend.top_score,
            with_trend.top_score
        );

        let rust_tags = vec!["rust".to_string()];
        let rust_input = ScoringInput {
            id: 2,
            title: "Rust async runtime performance update",
            url: Some("https://example.com/rust"),
            content: "Rust async runtime benchmark notes",
            source_type: "rss",
            embedding: &zero,
            created_at: None,
            detected_lang: "",
            source_tags: &rust_tags,
            tags_json: None,
            feed_origin: None,
            source_id: None,
        };
        let rust_no_trend = score_item(&rust_input, &ctx, &db, &fastpath_options(), None);
        let rust_options = ScoringOptions {
            trend_topics: vec!["rust".to_string()],
            ..fastpath_options()
        };
        let rust_with_trend = score_item(&rust_input, &ctx, &db, &rust_options, None);
        assert!(
            rust_with_trend.top_score > rust_no_trend.top_score,
            "in-domain trend should still boost: base={}, trend={}",
            rust_no_trend.top_score,
            rust_with_trend.top_score
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
        // Long enough to clear the boilerplate min-chunk floor — the match-time
        // hygiene filter drops sub-50-char fragments (they no longer exist in
        // post-2026-07-14 indexes).
        let stored = crate::test_utils::seed_embedding("context-chunk");
        db.upsert_context(
            "src/main.rs",
            "rust tauri ipc command handler registering invoke handlers for the main window",
            &stored,
        )
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
            source_id: None,
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
            source_id: None,
        };
        let result = score_item(&input, &ctx, &db, &fastpath_options(), None);

        // Sanity: stricter package ambiguity proof now stops this fixture even
        // before aggregate dep-match strength reaches the fast-path threshold.
        // `matched_deps` carries only display-worthy corroborated evidence, so
        // the ambiguous `log` hit is correctly ABSENT from it as well.
        let bd = result.score_breakdown.as_ref().expect("breakdown");
        assert!(
            bd.dep_match_score < scoring_config::CRITICAL_FASTPATH_DEP_MATCH_THRESHOLD,
            "ambiguous low-grounding fixture must not clear the aggregate fast-path threshold (got {})",
            bd.dep_match_score
        );
        assert!(
            !bd.matched_deps.iter().any(|d| d == "log"),
            "an ambiguous uncorroborated `log` hit must not surface as display \
             evidence, got {:?}",
            bd.matched_deps
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
            source_id: None,
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
    fn test_extract_cvss_ignores_version_label_digit() {
        // Regression: the OSV producer emits "Severity: CVSS_V3: 9.8"
        // (osv_types::vuln_to_source_item). The "V3" version digit must NOT be read as the
        // score — a 9.8 critical advisory must stay critical, not collapse to 3.0/"low"
        // (which silently defeats the necessity CVSS-severity fallback).
        let (score, sev) = extract_cvss_from_content("Severity: CVSS_V3: 9.8");
        assert_eq!(
            score,
            Some(9.8),
            "must read the score after the label, not the V3 version digit"
        );
        assert_eq!(sev.as_deref(), Some("critical"));

        // CVSS_V2 label likewise (V2 digit must not become the score)
        let (score2, sev2) = extract_cvss_from_content("Severity: CVSS_V3: 7.5");
        assert_eq!(score2, Some(7.5));
        assert_eq!(sev2.as_deref(), Some("high"));

        let (score3, sev3) = extract_cvss_from_content("Severity: CVSS_V2: 5.0");
        assert_eq!(score3, Some(5.0));
        assert_eq!(sev3.as_deref(), Some("medium"));

        // A non-numeric severity line still yields no score (behavior unchanged).
        let (score4, sev4) = extract_cvss_from_content("Severity: HIGH");
        assert_eq!(score4, None);
        assert_eq!(sev4, None);

        // VECTOR-format score (the OSV default per the schema): compute the base score, don't drop it.
        // A 9.8 critical encoded as a vector must NOT read as NONE (the pre-fix behavior for the
        // dominant real input — see §183 audit).
        let (v1, s1) = extract_cvss_from_content(
            "Severity: CVSS_V3: CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
        );
        assert_eq!(v1, Some(9.8), "vector 9.8 must compute, not drop to NONE");
        assert_eq!(s1.as_deref(), Some("critical"));
        let (v2, s2) = extract_cvss_from_content(
            "Severity: CVSS_V3: CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H",
        );
        assert_eq!(v2, Some(7.5));
        assert_eq!(s2.as_deref(), Some("high"));
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
            source_id: None,
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
            source_id: None,
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
            source_id: None,
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
            version_delta: dependencies::VersionDelta::Unknown,
            is_dev: false,
            is_direct: true,
            version: None,
            ecosystem: ecosystem.to_string(),
            corroborated: true,
            raw_name: None,
        }
    }

    #[test]
    fn security_applicability_metadata_route_is_independent_of_text_proof() {
        // An OSV/CVE advisory whose Affected-packages metadata names the dep
        // is proof by itself — even when the text-corroboration flag is false.
        // Tightening the text route must never weaken the structured route.
        let mut dep = test_dep("axios", "javascript");
        dep.corroborated = false;
        let metadata = vec![("axios".to_string(), "npm".to_string())];
        let (applicability, critical) = security_applicability(&[dep], &metadata);
        assert_eq!(applicability.as_deref(), Some("affected"));
        assert!(critical, "metadata-verified advisory must stay critical");
    }

    #[test]
    fn security_applicability_text_route_requires_corroboration() {
        // No structured metadata: a confident match that never actually named
        // the package (subterm/topic hit) is only "likely_affected".
        let mut dep = test_dep("react-router-dom", "javascript");
        dep.corroborated = false;
        let (applicability, critical) = security_applicability(&[dep], &[]);
        assert_eq!(applicability.as_deref(), Some("likely_affected"));
        assert!(!critical, "uncorroborated text match must not be critical");

        // With name corroboration the text route still proves "affected".
        let dep = test_dep("react-router-dom", "javascript");
        let (applicability, critical) = security_applicability(&[dep], &[]);
        assert_eq!(applicability.as_deref(), Some("affected"));
        assert!(critical);
    }

    #[test]
    fn security_applicability_metadata_naming_other_package_is_not_proof() {
        // Metadata exists but names a DIFFERENT package: not affected, even
        // for a corroborated match — same-ecosystem noise stays demoted.
        let dep = test_dep("react", "javascript");
        let metadata = vec![("next".to_string(), "npm".to_string())];
        let (applicability, critical) = security_applicability(&[dep], &metadata);
        assert_eq!(applicability.as_deref(), Some("likely_affected"));
        assert!(!critical);
    }

    #[test]
    fn security_applicability_no_deps_needs_verification() {
        let (applicability, critical) = security_applicability(&[], &[]);
        assert_eq!(applicability.as_deref(), Some("needs_verification"));
        assert!(!critical);
    }

    #[test]
    fn security_applicability_transitive_only_is_likely_affected() {
        let mut dep = test_dep("openssl-sys", "rust");
        dep.is_direct = false;
        let metadata = vec![("openssl-sys".to_string(), "crates.io".to_string())];
        let (applicability, critical) = security_applicability(&[dep], &metadata);
        assert_eq!(applicability.as_deref(), Some("likely_affected"));
        assert!(!critical, "transitive-only edges never reach 'affected'");
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
    // Engagement formula tests — v19 (AD-029): the multiplier is community-
    // only. The old six-term formula tests (affinity/anti/feedback/taste/
    // source-quality) were deleted with the terms themselves; keeping them
    // would document a formula the pipeline no longer runs.
    // ========================================================================

    #[test]
    fn test_engagement_mult_is_community_only() {
        // Strong community signal -> modest boost, bounded by weight.
        let strong = (1.0_f32 - 0.5) * scoring_config::ENGAGEMENT_WEIGHTS_COMMUNITY_W;
        let strong_mult = 1.0
            + strong.clamp(
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MIN,
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MAX,
            );
        assert!(
            strong_mult > 1.0
                && strong_mult <= 1.0 + scoring_config::ENGAGEMENT_WEIGHTS_COMMUNITY_W * 0.5 + 1e-6
        );

        // Absent community signal -> modest penalty, symmetric bound.
        let weak = (0.0_f32 - 0.5) * scoring_config::ENGAGEMENT_WEIGHTS_COMMUNITY_W;
        let weak_mult = 1.0
            + weak.clamp(
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MIN,
                scoring_config::ENGAGEMENT_WEIGHTS_CLAMP_MAX,
            );
        assert!(
            weak_mult < 1.0
                && weak_mult >= 1.0 - scoring_config::ENGAGEMENT_WEIGHTS_COMMUNITY_W * 0.5 - 1e-6
        );

        // The community term alone can never approach the historical
        // ×[0.5, 1.6] engagement swing — behavioral authority is gone.
        assert!(strong_mult - weak_mult <= scoring_config::ENGAGEMENT_WEIGHTS_COMMUNITY_W + 1e-6);
    }

    // ========================================================================
    // Commodity ceiling: AcademicPaper + ShowAndTell arms (Wave 7)
    //
    // AcademicPaper bypass matrix — ONLY strong dep grounding or a
    // security/version pattern lifts the ceiling. Sophistication and
    // community-signal bypasses are deliberately withheld: dense academic
    // prose trips the sophistication heuristic on virtually every paper.
    // ShowAndTell keeps the STANDARD bypasses (traction earns the slot).
    // ========================================================================

    const PAPER_TITLE: &str = "Scaling Transformer Inference via Speculative Decoding Cascades";
    const SHOW_TITLE: &str = "Show HN: I built a terminal music player in Rust";

    #[test]
    fn ceiling_caps_ungrounded_academic_paper() {
        let capped = apply_commodity_ceiling(
            0.90,
            PAPER_TITLE,
            &crate::content_dna::ContentType::AcademicPaper,
            0.0,
            0.0,
            false,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_ACADEMIC,
            "ungrounded paper must be capped at the academic ceiling"
        );
    }

    #[test]
    fn ceiling_bypassed_by_dep_grounded_academic_paper() {
        let score = apply_commodity_ceiling(
            0.90,
            PAPER_TITLE,
            &crate::content_dna::ContentType::AcademicPaper,
            0.0,
            0.0,
            true,  // strongly grounded in the user's dependencies,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            score, 0.90,
            "dep-grounded paper must bypass the academic ceiling"
        );
    }

    const HIRING_TITLE: &str =
        "CircleCI is hiring Senior Software Engineer #golang #typescript #react";
    const OFFSTACK_ADVISORY_TITLE: &str =
        "[GHSA-vjc7-jrh9-9j86] 9router has unauthenticated CRUD on /api/providers";

    #[test]
    fn ceiling_caps_hiring_post() {
        // A job ad name-dropping the developer's exact stack keywords (#golang
        // #react) previously scored CORE (~0.91). It must be capped hard.
        let capped = apply_commodity_ceiling(
            0.91,
            HIRING_TITLE,
            &crate::content_dna::ContentType::Hiring,
            0.0,
            0.0,
            false,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_HIRING,
            "hiring post must be capped at the hiring ceiling"
        );
    }

    #[test]
    fn community_and_grounding_do_not_lift_hiring() {
        // Neither a popular "Who's hiring" thread (high community_signal) nor a
        // stack-name match (strongly_grounded) may lift a job ad into the brief.
        let capped = apply_commodity_ceiling(
            0.91,
            HIRING_TITLE,
            &crate::content_dna::ContentType::Hiring,
            0.9,
            1.0,
            true,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_HIRING,
            "no bypass may lift the hiring ceiling"
        );
    }

    #[test]
    fn off_stack_security_advisory_capped_below_core() {
        // A CVE for a package NOT in the user's deps (9router/rama) must not ride
        // the security-pattern exemption to CORE.
        let capped = apply_commodity_ceiling(
            0.91,
            OFFSTACK_ADVISORY_TITLE,
            &crate::content_dna::ContentType::SecurityAdvisory,
            0.5,
            0.0,
            false, // NOT in the user's dependency graph,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_SECURITY_ADVISORY_UNGROUNDED,
            "off-stack advisory must be capped to the MATCH band"
        );
    }

    // ========================================================================
    // Ungrounded registry releases — capped below the relevance threshold.
    // The look-alike-crate class (forge-plugin-sdk-rust 0.947, dep_links=0).
    // ========================================================================

    #[test]
    fn ungrounded_registry_release_capped_below_relevance_threshold() {
        // Boosts keyed on the look-alike NAME (keyword "rust", open-window
        // intent, skill gaps) must not lift the cap — checked before every
        // exemption, like the off-stack advisory.
        let capped = apply_commodity_ceiling(
            0.95,
            "crates.io: forge-plugin-sdk-rust v1.0.7",
            &crate::content_dna::ContentType::ReleaseNotes,
            0.9, // sophistication bypass must NOT apply
            1.0, // community bypass must NOT apply
            false,
            true, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_REGISTRY_RELEASE_UNGROUNDED,
            "ungrounded registry release must be capped below the relevance threshold"
        );
        assert!(
            scoring_config::COMMODITY_CEILING_REGISTRY_RELEASE_UNGROUNDED
                < get_relevance_threshold(),
            "the cap must sit below the relevance threshold or the gate can still pass it"
        );
    }

    #[test]
    fn ungrounded_registry_deprecation_notice_still_capped() {
        // "deprecated vX" trips the version-conflict exemption for other
        // classes — but a deprecation notice for a package the user does NOT
        // depend on is still noise, so the cap must win.
        let capped = apply_commodity_ceiling(
            0.90,
            "crates.io: some-crate v0.3.0 [deprecated]",
            &crate::content_dna::ContentType::BreakingChange,
            0.0,
            0.0,
            false,
            true, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_REGISTRY_RELEASE_UNGROUNDED
        );
    }

    #[test]
    fn grounded_registry_release_not_capped() {
        // A release of a real dependency (tokio) surfaces at full score.
        let score = apply_commodity_ceiling(
            0.90,
            "crates.io: tokio v1.52.3",
            &crate::content_dna::ContentType::ReleaseNotes,
            0.0,
            0.0,
            true,  // strongly grounded (subject ∈ user deps)
            false, // NOT ungrounded
        );
        assert_eq!(
            score, 0.90,
            "grounded dependency release must not be capped"
        );
    }

    #[test]
    fn in_stack_security_advisory_not_capped() {
        // A CVE for a package the developer actually depends on must surface at
        // full score — the cap only applies to OFF-stack advisories.
        let score = apply_commodity_ceiling(
            0.91,
            "[GHSA-xxxx] axios SSRF vulnerability",
            &crate::content_dna::ContentType::SecurityAdvisory,
            0.5,
            0.0,
            true,  // strongly grounded — axios IS a dependency,
            false, // ungrounded_registry_release
        );
        assert_eq!(score, 0.91, "in-stack advisory must NOT be capped");
    }

    #[test]
    fn sophistication_does_not_bypass_academic_ceiling() {
        // The critical asymmetry vs Tutorial: academic prose is inherently
        // "sophisticated" — a 0.9 sophistication paper stays capped.
        let capped = apply_commodity_ceiling(
            0.90,
            PAPER_TITLE,
            &crate::content_dna::ContentType::AcademicPaper,
            0.9,
            0.0,
            false,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_ACADEMIC,
            "sophistication must NOT lift the academic ceiling"
        );
    }

    #[test]
    fn community_signal_does_not_bypass_academic_ceiling() {
        let capped = apply_commodity_ceiling(
            0.90,
            PAPER_TITLE,
            &crate::content_dna::ContentType::AcademicPaper,
            0.0,
            1.0,
            false,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_ACADEMIC,
            "community signal must NOT lift the academic ceiling"
        );
    }

    #[test]
    fn security_pattern_bypasses_academic_ceiling() {
        // A paper documenting a concrete vulnerability is actionable.
        let score = apply_commodity_ceiling(
            0.90,
            "CVE-2026-12345: Prompt Injection in Retrieval-Augmented Pipelines",
            &crate::content_dna::ContentType::AcademicPaper,
            0.0,
            0.0,
            false,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            score, 0.90,
            "security-pattern paper must bypass the academic ceiling"
        );
    }

    #[test]
    fn ceiling_caps_show_and_tell_without_traction() {
        let capped = apply_commodity_ceiling(
            0.85,
            SHOW_TITLE,
            &crate::content_dna::ContentType::ShowAndTell,
            0.0,
            0.0,
            false,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_SHOW_AND_TELL,
            "traction-less self-promo must be capped"
        );
    }

    #[test]
    fn high_community_signal_bypasses_show_and_tell_ceiling() {
        let score = apply_commodity_ceiling(
            0.85,
            SHOW_TITLE,
            &crate::content_dna::ContentType::ShowAndTell,
            0.0,
            scoring_config::COMMUNITY_SIGNAL_HIGH_THRESHOLD,
            false,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            score, 0.85,
            "a Show HN with real community traction earns its slot"
        );
    }

    #[test]
    fn sophistication_bypasses_show_and_tell_ceiling() {
        // Standard bypass set applies to ShowAndTell (unlike AcademicPaper).
        let score = apply_commodity_ceiling(
            0.85,
            SHOW_TITLE,
            &crate::content_dna::ContentType::ShowAndTell,
            0.5,
            0.0,
            false,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            score, 0.85,
            "sophisticated show-and-tell must keep the standard bypass"
        );
    }

    #[test]
    fn dep_grounding_alone_does_not_bypass_show_and_tell_ceiling() {
        // strongly_grounded is an AcademicPaper-only exemption — for
        // ShowAndTell the crowd/sophistication/security bypasses govern.
        let capped = apply_commodity_ceiling(
            0.85,
            SHOW_TITLE,
            &crate::content_dna::ContentType::ShowAndTell,
            0.0,
            0.0,
            true,
            false, // ungrounded_registry_release
        );
        assert_eq!(
            capped,
            scoring_config::COMMODITY_CEILING_SHOW_AND_TELL,
            "dep grounding is not a ShowAndTell bypass"
        );
    }

    #[test]
    fn ceiling_ignores_non_commodity_types() {
        // DeepDive (earned, not manifest-defaulted) passes through untouched.
        let score = apply_commodity_ceiling(
            0.90,
            "Understanding memory allocators in Rust",
            &crate::content_dna::ContentType::DeepDive,
            0.0,
            0.0,
            false,
            false, // ungrounded_registry_release
        );
        assert_eq!(score, 0.90, "non-commodity types must pass through");
    }

    // ========================================================================
    // filter_cve_dep_survivors — scoped/Go raw names + metadata-first (item 8b)
    // ========================================================================

    fn survivor_dep(normalized: &str, raw: &str, ecosystem: &str) -> DepMatch {
        DepMatch {
            package_name: normalized.to_string(),
            confidence: 0.5,
            version_delta: dependencies::VersionDelta::Unknown,
            is_dev: false,
            is_direct: true,
            version: None,
            ecosystem: ecosystem.to_string(),
            corroborated: true,
            raw_name: Some(raw.to_string()),
        }
    }

    #[test]
    fn scoped_npm_raw_name_title_hit_survives_and_keeps_credit() {
        // `DepMatch.package_name` is normalized ("babel-traverse") but the
        // advisory always writes the real form — pre-fix EVERY scoped npm
        // package failed the text filter.
        let mut deps = vec![survivor_dep(
            "babel-traverse",
            "@babel/traverse",
            "javascript",
        )];
        filter_cve_dep_survivors(
            &mut deps,
            "@babel/traverse prototype pollution vulnerability",
            "improper access control allows arbitrary code execution during compilation.",
            &[],
        );
        assert_eq!(deps.len(), 1, "raw-form title hit is rule-1 evidence");
        assert!(
            cve_dep_match_score(&deps) >= 0.5,
            "surviving direct dep keeps its credit"
        );
    }

    #[test]
    fn go_module_raw_path_survives_text_filter() {
        // Go modules normalize to "github.com-gin-gonic-gin" — never present
        // in prose. The raw module path is.
        let mut deps = vec![survivor_dep(
            "github.com-gin-gonic-gin",
            "github.com/gin-gonic/gin",
            "go",
        )];
        filter_cve_dep_survivors(
            &mut deps,
            "cve-2026-1234: directory traversal in github.com/gin-gonic/gin",
            "versions before 1.9.1 are affected.",
            &[],
        );
        assert_eq!(deps.len(), 1, "Go module path in the title must survive");
    }

    #[test]
    fn structured_metadata_confirms_dep_even_when_text_never_matches() {
        // The structured route runs INDEPENDENT of the text filter: metadata
        // naming the dep is stronger evidence than prose. Pre-fix it only ran
        // on text survivors, so it never got the chance.
        let mut deps = vec![survivor_dep(
            "babel-traverse",
            "@babel/traverse",
            "javascript",
        )];
        filter_cve_dep_survivors(
            &mut deps,
            "prototype pollution in a popular transpiler internals package",
            "a crafted file leads to arbitrary code execution during compilation.",
            &[("@babel/traverse".to_string(), "npm".to_string())],
        );
        assert_eq!(
            deps.len(),
            1,
            "affected-package metadata is proof by itself"
        );
    }

    #[test]
    fn structured_metadata_still_rejects_unlisted_deps() {
        // TN discipline preserved: when metadata exists, a dep it does NOT
        // name is dropped even on a clean title hit.
        let mut deps = vec![survivor_dep("serde", "serde", "rust")];
        filter_cve_dep_survivors(
            &mut deps,
            "serde mentioned in passing: vulnerability in left-pad",
            "the serde crate is unaffected.",
            &[("left-pad".to_string(), "npm".to_string())],
        );
        assert!(
            deps.is_empty(),
            "metadata that names only OTHER packages must drop the match"
        );
    }

    #[test]
    fn single_word_prose_hit_without_context_is_still_rejected() {
        // TN discipline preserved on the text route: the word "hostname" in a
        // DNS advisory is noise, not the `hostname` package.
        let mut deps = vec![survivor_dep("hostname", "hostname", "rust")];
        filter_cve_dep_survivors(
            &mut deps,
            "dns resolver cache poisoning issue",
            "a malicious response can override the hostname resolution result.",
            &[],
        );
        assert!(deps.is_empty(), "bare prose word hits stay rejected");
    }

    #[test]
    fn underscore_crate_body_hit_is_compound_evidence() {
        // `serde_derive` keeps its underscore through normalization; an
        // underscore name is as package-specific as a hyphenated one.
        let mut deps = vec![survivor_dep("serde_derive", "serde_derive", "rust")];
        filter_cve_dep_survivors(
            &mut deps,
            "unbounded recursion during deserialization",
            "serde_derive versions before 1.0.205 allow unbounded recursion.",
            &[],
        );
        assert_eq!(deps.len(), 1, "underscore compound body hit survives");
    }

    // ========================================================================
    // stale_published_multiplier — published_at as evidence (item 19)
    // ========================================================================

    fn published_months_ago(months: f32) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now() - chrono::Duration::days((months * DAYS_PER_MONTH) as i64)
    }

    #[test]
    fn stale_multiplier_ramp() {
        // Fresh content: untouched.
        let fresh = stale_published_multiplier(&published_months_ago(6.0), false, false, false);
        assert!((fresh - 1.0).abs() < 1e-6, "6mo old → 1.0 (got {fresh})");
        // Past the stale horizon: floored.
        let old = stale_published_multiplier(&published_months_ago(40.0), false, false, false);
        assert!(
            (old - scoring_config::STALE_CONTENT_STALE_FLOOR).abs() < 1e-6,
            "40mo old → stale floor (got {old})"
        );
        // Mid-ramp: linear between the two (24mo = halfway 12→36 → 0.775).
        let mid = stale_published_multiplier(&published_months_ago(24.0), false, false, false);
        assert!(
            (mid - 0.775).abs() < 0.02,
            "24mo old → ~0.775 mid-ramp (got {mid})"
        );
        // Monotonic: older never scores higher.
        assert!(fresh >= mid && mid >= old);
    }

    #[test]
    fn stale_multiplier_exempts_security_and_softens_grounded() {
        // An old unpatched CVE can still matter — no discount.
        let sec = stale_published_multiplier(&published_months_ago(40.0), true, false, false);
        assert!((sec - 1.0).abs() < 1e-6, "security exempt (got {sec})");
        // A years-old deep-dive on YOUR exact stack: softened, not killed.
        let grounded = stale_published_multiplier(&published_months_ago(40.0), false, true, false);
        assert!(
            (grounded - scoring_config::STALE_CONTENT_GROUNDED_FLOOR).abs() < 1e-6,
            "grounded floor (got {grounded})"
        );
    }

    /// Tightening T3 (2026-08-25): a superseded RELEASE announcement gets
    /// neither the grounded softening nor the shallow stale floor — the live
    /// "TypeScript 5.1 Beta is OUT!" (2023, typescript IS a dep) held 0.882
    /// and sat feed-relevant on exactly that combination.
    #[test]
    fn stale_multiplier_deepens_for_superseded_releases() {
        let release = stale_published_multiplier(&published_months_ago(40.0), false, false, true);
        assert!(
            (release - scoring_config::STALE_CONTENT_RELEASE_FLOOR).abs() < 1e-6,
            "40mo release → release floor (got {release})"
        );
        // Grounding must NOT soften it (the live defect).
        let grounded_release =
            stale_published_multiplier(&published_months_ago(40.0), false, true, true);
        assert!(
            (grounded_release - scoring_config::STALE_CONTENT_RELEASE_FLOOR).abs() < 1e-6,
            "grounding never softens a superseded release (got {grounded_release})"
        );
        // Deeper than the generic stale floor, and strictly ordered.
        assert!(
            scoring_config::STALE_CONTENT_RELEASE_FLOOR < scoring_config::STALE_CONTENT_STALE_FLOOR
        );
        // A CURRENT release is untouched — this discounts age, not releases.
        let fresh_release =
            stale_published_multiplier(&published_months_ago(1.0), false, true, true);
        assert!(
            (fresh_release - 1.0).abs() < 1e-6,
            "a current release is untouched (got {fresh_release})"
        );
        // Security still wins over everything.
        let sec_release =
            stale_published_multiplier(&published_months_ago(40.0), true, false, true);
        assert!((sec_release - 1.0).abs() < 1e-6);
    }

    // ========================================================================
    // Dep-gate bypass conf_mult lift (item 22b)
    // ========================================================================

    #[test]
    fn dep_gate_bypass_lifts_conf_mult_to_two_signal_tier() {
        // Pre-fix: ceiling raised to 0.72 but conf_mult stayed 0.45, so a
        // 1-signal strong dep item topped out at ~0.45x its boosted score —
        // arithmetically below the 0.70 relevance escape, always. Post-fix
        // the same input reaches the bypass ceiling itself.
        let ctx = super::benchmark_scenarios::profile_ctx("rust_developer");
        let gated = apply_gate_effect(0.90, 1, 1.0, &ctx, 0.0, 0.40);
        assert!(
            (gated - scoring_config::DEPENDENCY_GATE_BYPASS_DIRECT_DEP_CEILING).abs() < 1e-3,
            "strong 1-signal dep item must reach the 0.72 bypass ceiling (got {gated})"
        );
        // The confirmation_gate table this lift mirrors: 1 => (0.45, 0.28),
        // 2 => (1.00, 0.72). Sanity-pin both so a DSL retune is noticed here.
        assert_eq!(scoring_config::CONFIRMATION_GATE[1], (0.45, 0.28));
        assert_eq!(scoring_config::CONFIRMATION_GATE[2], (1.00, 0.72));
    }

    #[test]
    fn weak_dep_match_stays_capped_at_one_signal_tier() {
        // TN guard: below the bypass minimum (0.35) nothing lifts — the
        // 1-signal tier's 0.28 ceiling holds.
        let ctx = super::benchmark_scenarios::profile_ctx("rust_developer");
        let gated = apply_gate_effect(0.90, 1, 1.0, &ctx, 0.0, 0.30);
        assert!(
            gated <= scoring_config::CONFIRMATION_GATE[1].1 + 1e-4,
            "weak dep match must stay capped at 0.28 (got {gated})"
        );
    }
}
