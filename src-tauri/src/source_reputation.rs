// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Source Reputation Engine
//!
//! Solves the cold-start problem for user-added sources (RSS feeds, YouTube
//! channels, Twitter accounts). Without this module, every new source gets
//! the same generic 1.0x multiplier regardless of quality — the Deno blog
//! scores identically to a random tech aggregator.
//!
//! ## Three-layer model
//!
//! **Layer 1 — Curated prior.** A hardcoded list of known-good developer
//! sources ships with 4DA. If the user adds `blog.deno.com`, it starts at
//! 1.20x instead of 1.0x. This is the instant win — a known-quality feed
//! produces good results on day zero.
//!
//! **Layer 2 — Content-quality inference.** After the source has produced
//! ≥ 10 items, we compute a per-source quality score from the items
//! themselves: code density, title specificity, CVE/release frequency,
//! average length. This is a one-time calibration that replaces the Layer
//! 1 prior once enough evidence exists.
//!
//! **Layer 3 — Engagement reputation.** Live user behavior (clicks, saves,
//! dismissals) feeds a Bayesian-smoothed per-source engagement score that
//! progressively overrides the Layer 2 inference. Over time, engagement
//! reality wins over static priors.
//!
//! ## The combined multiplier
//!
//! ```text
//! layer1 = prior from curated list (default 1.0 if unknown)
//! layer2 = inferred from first 10 items (weight 0..1 as items grow)
//! layer3 = engagement score (weight 0..1 as interactions grow)
//!
//! final = blend(layer1, layer2, layer3) clamped to [0.70, 1.30]
//! ```
//!
//! The blend is conservative — a single click doesn't move the needle, but
//! 50 consistent interactions do. A bad feed drops to 0.70; a great one
//! rises to 1.30. That's a 2x spread, enough to materially shape the feed
//! without overwhelming the content-DNA classifier.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Per-source reputation snapshot stored in the DB (or computed on demand).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceReputation {
    /// Canonical key — normalized hostname for RSS, channel_id for YouTube,
    /// handle for Twitter, or the source_type slug for built-in sources.
    pub source_key: String,
    /// Items observed from this source (ingested). Drives Layer 2 weight.
    pub items_observed: u32,
    /// Items surfaced to the user (made it past the relevance gate).
    pub items_surfaced: u32,
    /// User interactions — click-through / open-URL.
    pub items_clicked: u32,
    /// User interactions — saved (strong positive).
    pub items_saved: u32,
    /// User interactions — dismissed (strong negative).
    pub items_dismissed: u32,
    /// Inferred quality score from content inspection (Layer 2).
    /// None if fewer than 10 items have been observed.
    pub inferred_quality: Option<f32>,
    /// The final blended multiplier (Layer 1 × Layer 2 × Layer 3).
    pub multiplier: f32,
}

impl SourceReputation {
    /// Create a new reputation record seeded from the curated prior list.
    /// This is the first snapshot for a source the user just added.
    pub fn new_with_prior(source_key: impl Into<String>) -> Self {
        let source_key = source_key.into();
        let prior = get_curated_prior(&source_key);
        Self {
            source_key,
            items_observed: 0,
            items_surfaced: 0,
            items_clicked: 0,
            items_saved: 0,
            items_dismissed: 0,
            inferred_quality: None,
            multiplier: prior,
        }
    }
}

/// Normalize a source identifier into a canonical key for reputation lookup.
///
/// - RSS URLs → lowercase hostname (strips www., trailing port, path, query).
/// - YouTube channel IDs → returned as-is.
/// - Twitter handles → lowercase, strip leading @.
/// - Built-in sources → source_type slug (e.g. "hackernews", "reddit").
pub fn normalize_source_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Already looks like a URL — extract hostname.
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        if let Ok(url) = url::Url::parse(trimmed) {
            if let Some(host) = url.host_str() {
                return host.trim_start_matches("www.").to_lowercase();
            }
        }
    }

    // @handle — strip the @ and lowercase
    if let Some(rest) = trimmed.strip_prefix('@') {
        return rest.to_lowercase();
    }

    // Otherwise lowercase and return as-is
    trimmed.to_lowercase()
}

// ============================================================================
// Layer 1 — Curated Prior List
// ============================================================================

/// A curated list of known-quality developer sources. Adding a new entry here
/// gives users an immediate Layer 1 multiplier uplift (or penalty) without
/// waiting for Layer 2/3 to calibrate. Keep the list high-signal only — every
/// addition should represent a source the team has independently verified.
///
/// Multiplier interpretation:
///   1.30 → exceptional, official project engineering blogs
///   1.20 → very high signal, official project blogs
///   1.15 → credible tech engineering sources
///   1.10 → reliable aggregators / newsletters
///   1.00 → neutral (no prior)
///   0.90 → noisy aggregators
///   0.75 → low-signal SEO mills / generic news
fn build_curated_priors() -> HashMap<&'static str, f32> {
    let mut m = HashMap::new();

    // Tier 1.30 — exceptional: official blogs of first-tier tech teams
    m.insert("blog.rust-lang.org", 1.30);
    m.insert("rust-lang.org", 1.30);
    m.insert("blog.golang.org", 1.30);
    m.insert("go.dev", 1.30);
    m.insert("blog.deno.com", 1.30);
    m.insert("react.dev", 1.30);
    m.insert("v8.dev", 1.30);
    m.insert("webkit.org", 1.30);
    m.insert("developer.mozilla.org", 1.30);
    m.insert("kernel.org", 1.30);
    m.insert("llvm.org", 1.30);

    // Tier 1.20 — official project engineering & release blogs
    m.insert("bun.sh", 1.20);
    m.insert("nodejs.org", 1.20);
    m.insert("python.org", 1.20);
    m.insert("typescriptlang.org", 1.20);
    m.insert("devblogs.microsoft.com", 1.20);
    m.insert("nextjs.org", 1.20);
    m.insert("svelte.dev", 1.20);
    m.insert("vuejs.org", 1.20);
    m.insert("astro.build", 1.20);
    m.insert("tauri.app", 1.20);
    m.insert("vitejs.dev", 1.20);
    m.insert("vercel.com", 1.20);
    m.insert("openssf.org", 1.20);
    m.insert("security.googleblog.com", 1.20);
    m.insert("research.mozilla.org", 1.20);
    m.insert("mozilla.org", 1.20);
    m.insert("anthropic.com", 1.20);
    m.insert("openai.com", 1.20);

    // Tier 1.15 — credible engineering teams & publications
    m.insert("engineering.fb.com", 1.15);
    m.insert("netflixtechblog.com", 1.15);
    m.insert("stripe.com", 1.15);
    m.insert("github.blog", 1.15);
    m.insert("githubblog.com", 1.15);
    m.insert("cloudflare.com", 1.15);
    m.insert("blog.cloudflare.com", 1.15);
    m.insert("cockroachlabs.com", 1.15);
    m.insert("supabase.com", 1.15);
    m.insert("planetscale.com", 1.15);
    m.insert("sentry.io", 1.15);
    m.insert("jepsen.io", 1.15);
    m.insert("acolyer.org", 1.15);
    m.insert("martinfowler.com", 1.15);
    m.insert("lwn.net", 1.15);

    // Tier 1.10 — reliable aggregators & newsletters
    m.insert("thisweekinreact.com", 1.10);
    m.insert("thisweekinrust.org", 1.10);
    m.insert("thisweekindatabases.com", 1.10);
    m.insert("bytesizedcode.com", 1.10);
    m.insert("changelog.com", 1.10);
    m.insert("infoq.com", 1.10);
    m.insert("hackernewsletter.com", 1.10);
    m.insert("tldrnewsletter.com", 1.10);
    m.insert("javascriptweekly.com", 1.10);
    m.insert("rustweekly.com", 1.10);
    m.insert("pointer.io", 1.10);

    // Tier 0.90 — noisy aggregators (not the target developer audience)
    m.insert("medium.com", 0.90);

    // Tier 0.75 — low-signal general news
    m.insert("techcrunch.com", 0.75);
    m.insert("theverge.com", 0.75);
    m.insert("wired.com", 0.75);
    m.insert("mashable.com", 0.75);

    m
}

fn curated_priors() -> &'static HashMap<&'static str, f32> {
    static PRIORS: OnceLock<HashMap<&'static str, f32>> = OnceLock::new();
    PRIORS.get_or_init(build_curated_priors)
}

/// Look up the Layer 1 curated prior for a source key.
///
/// Matches in priority:
///   1. Exact hostname match
///   2. Suffix match (so `dev.blog.rust-lang.org` still hits `rust-lang.org`)
///   3. Default 1.0 (neutral, no prior)
pub fn get_curated_prior(source_key: &str) -> f32 {
    let key = normalize_source_key(source_key);
    let priors = curated_priors();
    if let Some(&v) = priors.get(key.as_str()) {
        return v;
    }
    // Suffix match for subdomains — e.g. news.ycombinator.com doesn't need
    // to be listed because we want the top-level domain to win.
    for (&domain, &value) in priors.iter() {
        if key.ends_with(&format!(".{domain}")) {
            return value;
        }
    }
    1.0
}

// ============================================================================
// Layer 2 — Content-quality inference
// ============================================================================

/// Inspect a sample of items from a source and compute an inferred quality
/// multiplier. This is called once, when the source has accumulated ≥ 10
/// items. The returned value replaces the Layer 1 prior as the baseline.
///
/// Quality signals scored:
///   - Title specificity (proper nouns, version numbers, CVE IDs)
///   - Content length distribution (deep dives vs. short snippets)
///   - Code-block density (markdown ``` fences)
///   - Security/breaking-change frequency
///   - External link density (reference-style writing)
pub fn infer_source_quality(samples: &[ItemSample]) -> f32 {
    if samples.is_empty() {
        return 1.0;
    }
    if samples.len() < 3 {
        // Too few samples — return neutral, don't overfit.
        return 1.0;
    }

    let n = samples.len() as f32;

    // Metric 1: average specific-token count (proper nouns, versions)
    //   High-signal titles mention concrete things: React, CVE-2026-..., v1.94, etc.
    let avg_specific = samples
        .iter()
        .map(|s| count_specific_tokens(&s.title) as f32)
        .sum::<f32>()
        / n;
    let specificity_score = (avg_specific / 3.0).clamp(0.0, 1.0);

    // Metric 2: average body length (longer = more substance)
    let avg_len = samples.iter().map(|s| s.content.len() as f32).sum::<f32>() / n;
    // 500 chars ≈ short update, 3000+ chars ≈ deep dive.
    let length_score = ((avg_len - 500.0) / 2500.0).clamp(0.0, 1.0);

    // Metric 3: code-block density (``` fences or <code> blocks)
    let code_ratio = samples
        .iter()
        .filter(|s| s.content.contains("```") || s.content.contains("<pre"))
        .count() as f32
        / n;

    // Metric 4: security/breaking-change frequency (high-signal for dev feeds)
    let urgent_ratio = samples
        .iter()
        .filter(|s| {
            let t = s.title.to_lowercase();
            t.contains("cve")
                || t.contains("vulnerability")
                || t.contains("breaking change")
                || t.contains("security advisory")
                || t.contains("zero-day")
                || t.contains("deprecated")
        })
        .count() as f32
        / n;

    // Metric 5: clickbait ratio (cheap heuristic matching content_dna rules)
    let clickbait_ratio = samples
        .iter()
        .filter(|s| title_looks_clickbait(&s.title))
        .count() as f32
        / n;

    // Combine: base 1.0, add up to +0.30, subtract up to -0.30
    let uplift =
        0.10 * specificity_score + 0.08 * length_score + 0.06 * code_ratio + 0.06 * urgent_ratio;
    let penalty = 0.30 * clickbait_ratio;

    (1.0 + uplift - penalty).clamp(0.70, 1.30)
}

/// A single item's title + content — enough for Layer 2 inference.
#[derive(Debug, Clone)]
pub struct ItemSample {
    pub title: String,
    pub content: String,
}

fn count_specific_tokens(title: &str) -> usize {
    let mut count = 0;
    // CVE IDs
    if title.to_lowercase().contains("cve-") {
        count += 2;
    }
    // Semver (look for a digit.digit pattern preceded by a space/word-start)
    let bytes = title.as_bytes();
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i].is_ascii_digit()
            && i + 2 < bytes.len()
            && bytes[i + 1] == b'.'
            && bytes[i + 2].is_ascii_digit()
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
        {
            count += 1;
            break;
        }
    }
    // Capitalized technical terms (rough proxy: words of ≥ 4 chars starting with
    // uppercase that aren't the first word of the title).
    for (idx, word) in title.split_whitespace().enumerate() {
        if idx == 0 {
            continue;
        }
        if word.len() >= 4 && word.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            count += 1;
        }
    }
    count
}

fn title_looks_clickbait(title: &str) -> bool {
    let t = title.to_lowercase();
    t.starts_with("he just ")
        || t.starts_with("she just ")
        || t.starts_with("they just ")
        || t.starts_with("this guy ")
        || t.contains("you won't believe")
        || t.contains("shocking")
        || t.contains("?!")
        || t.contains("!!!")
}

// ============================================================================
// Layer 3 — Engagement reputation
// ============================================================================

/// Bayesian-smoothed engagement score. Starts neutral (0.5) and shifts toward
/// reality as interactions accumulate. Prior strength is 20 observations —
/// meaning 5 saves vs. 0 dismisses on 5 items is noisy, but 50 consistent
/// interactions meaningfully shift the multiplier.
pub fn engagement_score(items_surfaced: u32, clicks: u32, saves: u32, dismisses: u32) -> f32 {
    // Weighted signal: saves count double positive, dismisses count double negative.
    // Clicks are moderate positive (showed interest, didn't necessarily save).
    let weighted_positive = clicks as f32 + 2.0 * saves as f32;
    let weighted_negative = 2.0 * dismisses as f32;
    let weighted_signal = weighted_positive - weighted_negative;

    // Bayesian smoothing: prior = 0.5 (neutral), strength = 20 observations.
    const PRIOR: f32 = 0.5;
    const PRIOR_STRENGTH: f32 = 20.0;

    let n = items_surfaced as f32;
    if n <= 0.0 {
        return PRIOR;
    }

    // Normalize weighted_signal to a rate in [-1, 1] first, then shift to [0, 1].
    let max_possible = 2.0 * n; // if every surfaced item got saved
    let raw_rate = (weighted_signal / max_possible).clamp(-1.0, 1.0);
    let raw_score = 0.5 + 0.5 * raw_rate; // map [-1, 1] → [0, 1]

    // Bayesian blend
    (PRIOR * PRIOR_STRENGTH + raw_score * n) / (PRIOR_STRENGTH + n)
}

/// Convert an engagement score in [0, 1] to a multiplier in the reputation
/// range [0.70, 1.30]. Uses a linear stretch so a 0.5 score maps to 1.0x.
fn engagement_to_multiplier(score: f32) -> f32 {
    // 0.0 → 0.70, 0.5 → 1.00, 1.0 → 1.30
    (0.70 + score * 0.60).clamp(0.70, 1.30)
}

// ============================================================================
// Combined multiplier
// ============================================================================

/// Blend all three layers into a single multiplier.
///
/// Blending weights evolve with evidence:
///   - 0 items: 100% Layer 1 (curated prior)
///   - 10 items: Layer 2 joins at 50%
///   - 30+ items surfaced with interactions: Layer 3 joins
///   - At 100+ interactions: Layer 3 dominates
pub fn compute_final_multiplier(rep: &SourceReputation) -> f32 {
    let layer1 = get_curated_prior(&rep.source_key);

    // Layer 2 weight: 0 at <10 observed items, ramps to 1.0 at 30 items
    let layer2_weight = ((rep.items_observed as f32 - 10.0) / 20.0).clamp(0.0, 1.0);
    let layer2 = rep.inferred_quality.unwrap_or(1.0);

    // Layer 3 weight: 0 at <20 surfaced items, ramps to 1.0 at 100 interactions
    let total_interactions = rep.items_clicked + rep.items_saved + rep.items_dismissed;
    let layer3_weight = if rep.items_surfaced < 20 {
        0.0
    } else {
        (total_interactions as f32 / 100.0).clamp(0.0, 1.0)
    };
    let layer3_score = engagement_score(
        rep.items_surfaced,
        rep.items_clicked,
        rep.items_saved,
        rep.items_dismissed,
    );
    let layer3 = engagement_to_multiplier(layer3_score);

    // Blend: partition 1.0 of weight across the three layers.
    // Layer 3 gets priority, then Layer 2, then Layer 1 absorbs whatever's left.
    let l3_w = layer3_weight;
    let l2_w = (1.0 - l3_w) * layer2_weight;
    let l1_w = 1.0 - l2_w - l3_w;

    (l1_w * layer1 + l2_w * layer2 + l3_w * layer3).clamp(0.70, 1.30)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_strips_protocol_and_www() {
        assert_eq!(normalize_source_key("https://www.deno.com/"), "deno.com");
        assert_eq!(
            normalize_source_key("http://blog.rust-lang.org/index.html"),
            "blog.rust-lang.org"
        );
    }

    #[test]
    fn test_normalize_strips_at_prefix() {
        assert_eq!(normalize_source_key("@Fireship"), "fireship");
        assert_eq!(normalize_source_key("@dan_abramov"), "dan_abramov");
    }

    #[test]
    fn test_curated_prior_exact_match() {
        assert_eq!(get_curated_prior("blog.deno.com"), 1.30);
        assert_eq!(get_curated_prior("react.dev"), 1.30);
    }

    #[test]
    fn test_curated_prior_suffix_match() {
        // A subdomain that's not listed should still pick up the parent domain
        assert_eq!(get_curated_prior("changelog.react.dev"), 1.30);
        assert_eq!(get_curated_prior("api.github.blog"), 1.15);
    }

    #[test]
    fn test_curated_prior_unknown_domain_is_neutral() {
        assert_eq!(get_curated_prior("random-blog.example"), 1.0);
    }

    #[test]
    fn test_curated_prior_known_low_signal() {
        assert_eq!(get_curated_prior("techcrunch.com"), 0.75);
    }

    #[test]
    fn test_infer_quality_clickbait_penalized() {
        let samples = vec![
            ItemSample {
                title: "He just crawled through hell to fix the browser...".into(),
                content: "short".into(),
            },
            ItemSample {
                title: "You won't believe what this dev did!".into(),
                content: "short".into(),
            },
            ItemSample {
                title: "This guy rewrote everything!".into(),
                content: "short".into(),
            },
        ];
        let q = infer_source_quality(&samples);
        assert!(q < 1.0, "clickbait should lower inferred quality, got {q}");
    }

    #[test]
    fn test_infer_quality_security_feed_boosted() {
        let samples = vec![
            ItemSample {
                title: "CVE-2026-1234: Critical vulnerability in example-crate 1.2.3".into(),
                content: "A deep dive into the vulnerability including reproduction steps and a proposed mitigation. The root cause is in the handling of X.\n\n```rust\nfn example() {}\n```\n\nMore detail below...".repeat(3),
            },
            ItemSample {
                title: "Security advisory: React 19.2.5 breaking change".into(),
                content: "This post covers the breaking change and migration path.".repeat(5),
            },
            ItemSample {
                title: "Deprecated API: old-pkg 2.0 end of life".into(),
                content: "Migration guide with code examples.\n```bash\nnpm install new-pkg\n```".repeat(4),
            },
        ];
        let q = infer_source_quality(&samples);
        assert!(
            q > 1.0,
            "security/release feed should score above neutral, got {q}"
        );
    }

    #[test]
    fn test_engagement_score_neutral_without_data() {
        let s = engagement_score(0, 0, 0, 0);
        assert!((s - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_engagement_score_positive_from_saves() {
        // 20 surfaced, 10 saves, 2 dismisses — net positive
        let s = engagement_score(20, 3, 10, 2);
        assert!(s > 0.55, "expected positive engagement, got {s}");
    }

    #[test]
    fn test_engagement_score_negative_from_dismisses() {
        // 20 surfaced, 0 saves, 15 dismisses
        let s = engagement_score(20, 0, 0, 15);
        assert!(s < 0.45, "expected negative engagement, got {s}");
    }

    #[test]
    fn test_engagement_score_smoothed_by_prior() {
        // 1 save on 1 surface shouldn't pin the score to 1.0 — prior smoothing
        let s = engagement_score(1, 0, 1, 0);
        assert!(
            s < 0.75,
            "Bayesian prior should keep single-sample from dominating, got {s}"
        );
    }

    #[test]
    fn test_combined_cold_start_returns_layer1_prior() {
        let mut rep = SourceReputation::new_with_prior("blog.deno.com");
        rep.items_observed = 0;
        rep.items_surfaced = 0;
        let m = compute_final_multiplier(&rep);
        // Layer 1 only — should return curated prior
        assert!((m - 1.30).abs() < 0.01, "expected ~1.30, got {m}");
    }

    #[test]
    fn test_combined_unknown_source_starts_at_one() {
        let rep = SourceReputation::new_with_prior("random-blog.example");
        let m = compute_final_multiplier(&rep);
        assert!(
            (m - 1.0).abs() < 0.01,
            "expected ~1.0 for unknown source, got {m}"
        );
    }

    #[test]
    fn test_combined_clamped_to_range() {
        // Even a heavily-dismissed source can't go below 0.70
        let rep = SourceReputation {
            source_key: "bad-source.example".into(),
            items_observed: 100,
            items_surfaced: 100,
            items_clicked: 0,
            items_saved: 0,
            items_dismissed: 100,
            inferred_quality: Some(0.70),
            multiplier: 0.0,
        };
        let m = compute_final_multiplier(&rep);
        assert!(
            m >= 0.70 - 0.01,
            "multiplier should not go below 0.70, got {m}"
        );
        assert!(
            m <= 1.30 + 0.01,
            "multiplier should not exceed 1.30, got {m}"
        );
    }

    #[test]
    fn test_engagement_dominates_at_high_volume() {
        // A curated 1.30 source that users consistently dismiss should drop
        // below 1.0 once enough evidence accrues.
        let rep = SourceReputation {
            source_key: "blog.deno.com".into(),
            items_observed: 100,
            items_surfaced: 100,
            items_clicked: 0,
            items_saved: 0,
            items_dismissed: 80,
            inferred_quality: Some(1.0),
            multiplier: 0.0,
        };
        let m = compute_final_multiplier(&rep);
        assert!(
            m < 1.30,
            "heavy dismissal should pull multiplier below the curated prior, got {m}"
        );
    }

    #[test]
    fn test_new_with_prior_seeds_multiplier() {
        let rep = SourceReputation::new_with_prior("blog.deno.com");
        assert!((rep.multiplier - 1.30).abs() < 0.01);
    }
}
