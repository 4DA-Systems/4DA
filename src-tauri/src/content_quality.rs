//! Content Quality Heuristics for 4DA
//!
//! Evaluates the quality of source items based on title patterns,
//! content depth, and source authority. Used as a scoring multiplier
//! to boost high-quality content and penalize low-quality content.

/// Content quality assessment result
#[derive(Debug, Clone)]
#[allow(dead_code)] // Reason: title_quality and content_depth fields set but only multiplier is read in production
pub struct ContentQuality {
    /// Title quality score (0.0-1.0)
    pub title_quality: f32,
    /// Content depth score (0.0-1.0)
    pub content_depth: f32,
    /// Overall quality multiplier applied to scoring
    pub multiplier: f32,
}

/// Compute content quality for a source item.
/// Returns a multiplier (0.5 to 1.2) that adjusts the relevance score.
///
/// Quality signals:
/// - Title: clickbait patterns, excessive caps/punctuation, listicle patterns
/// - Content: length, code presence, structural indicators
/// - Source authority: known high/low quality domains
pub fn compute_content_quality(title: &str, content: &str, url: Option<&str>) -> ContentQuality {
    let title_quality = assess_title_quality(title);
    let content_depth = assess_content_depth(content);
    let source_authority = url.map_or(1.0, assess_source_authority);

    let info_density = assess_information_density(title);

    // Anti-gaming defenses
    let keyword_penalty = keyword_concentration_penalty(title);
    let coherence_penalty = title_body_coherence_penalty(title, content);
    let diversity_penalty = title_diversity_penalty(title);

    // Combine: original weights preserved for calibration stability.
    // Info density acts as a bonus/penalty layer — dense titles get a small boost,
    // vague titles get a small penalty. Conservative to avoid breaking recall.
    let raw = title_quality * 0.5 + content_depth * 0.3 + source_authority * 0.2;
    let density_adjustment = (info_density - 0.5) * 0.10; // -0.05 to +0.05

    // Map to multiplier range [0.5, 1.2]
    let multiplier = (raw * 0.7
        + 0.5
        + density_adjustment
        + keyword_penalty
        + coherence_penalty
        + diversity_penalty)
        .clamp(0.5, 1.2);

    ContentQuality {
        title_quality,
        content_depth,
        multiplier,
    }
}

/// Assess title quality (0.0 = clickbait, 1.0 = high quality)
fn assess_title_quality(title: &str) -> f32 {
    let mut score: f32 = 1.0;

    // Penalty: Short/vague titles (fewer than 4 meaningful words)
    // "where to start", "help please", "a question" — too vague to be useful
    let word_count = title
        .split_whitespace()
        .filter(|w| w.len() >= 2) // skip single-char words
        .count();
    if word_count < 4 {
        score -= 0.35;
    }

    // Penalty: ALL CAPS (more than 50% uppercase letters)
    let alpha_chars: Vec<char> = title.chars().filter(|c| c.is_alphabetic()).collect();
    if alpha_chars.len() >= 5 {
        let upper_ratio = alpha_chars.iter().filter(|c| c.is_uppercase()).count() as f32
            / alpha_chars.len() as f32;
        if upper_ratio > 0.5 {
            score -= 0.3;
        }
    }

    // Penalty: Excessive punctuation (!!!, ???, !!?)
    let excl_count = title.chars().filter(|&c| c == '!').count();
    let quest_count = title.chars().filter(|&c| c == '?').count();
    if excl_count > 1 || quest_count > 2 {
        score -= 0.2;
    }

    // Penalty: Clickbait patterns
    let lower = title.to_lowercase();
    const CLICKBAIT: &[&str] = &[
        "you won't believe",
        "mind-blowing",
        "shocking",
        "insane",
        "blown away",
        "will blow your mind",
        "game changer",
        "this one trick",
        "everything you need to know",
        "what nobody tells you",
        "the truth about",
        "stop doing this",
        "i was wrong about",
    ];
    if CLICKBAIT.iter().any(|p| lower.contains(p)) {
        score -= 0.3;
    }

    // Penalty: Pure listicle ("10 Things...", "Top 20...")
    let listicle_start = lower
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .count();
    if listicle_start > 0 {
        // Check if the number is followed by common listicle words
        let after_num = &lower.trim_start()[listicle_start..].trim_start();
        if after_num.starts_with("things")
            || after_num.starts_with("ways")
            || after_num.starts_with("tips")
            || after_num.starts_with("reasons")
            || after_num.starts_with("tools")
            || after_num.starts_with("best")
        {
            score -= 0.15;
        }
    }

    // Bonus: Technical specificity (version numbers, RFC references, CVE IDs)
    if lower.contains("rfc ")
        || lower.contains("cve-")
        || lower.contains(" v1")
        || lower.contains(" v2")
        || lower.contains(" v3")
        || lower.contains(" v4")
    {
        score += 0.1;
    }

    score.clamp(0.0, 1.0)
}

/// Assess content depth (0.0 = shallow, 1.0 = deep technical content)
fn assess_content_depth(content: &str) -> f32 {
    if content.is_empty() {
        return 0.3; // No content available (RSS summary only)
    }

    let word_count = content.split_whitespace().count();
    let mut score: f32 = 0.3;

    // Length bonus (more content = more depth, diminishing returns)
    score += match word_count {
        0..=50 => 0.0,
        51..=200 => 0.1,
        201..=500 => 0.2,
        501..=1000 => 0.3,
        _ => 0.4,
    };

    // Code presence bonus (suggests technical depth)
    let has_code = content.contains("```")
        || content.contains("fn ")
        || content.contains("function ")
        || content.contains("const ")
        || content.contains("import ")
        || content.contains("class ");
    if has_code {
        score += 0.15;
    }

    // Structure bonus (headings, lists suggest organized content)
    let has_structure =
        content.contains("## ") || content.contains("### ") || content.contains("- ");
    if has_structure {
        score += 0.1;
    }

    score.clamp(0.0, 1.0)
}

/// Assess source authority from URL domain (0.7 = aggregator, 1.0 = neutral, 1.15 = authoritative)
fn assess_source_authority(url: &str) -> f32 {
    let lower = url.to_lowercase();

    // High-authority sources
    const AUTHORITATIVE: &[&str] = &[
        "github.com",
        "blog.rust-lang.org",
        "doc.rust-lang.org",
        "docs.rs",
        "arxiv.org",
        "research.google",
        "engineering.fb.com",
        "netflixtechblog.com",
        "blog.cloudflare.com",
        "webkit.org",
        "v8.dev",
        "chromium.org",
        "developer.mozilla.org",
        "web.dev",
    ];
    if AUTHORITATIVE.iter().any(|d| lower.contains(d)) {
        return 1.15;
    }

    // Lower authority (aggregators, content farms)
    const AGGREGATORS: &[&str] = &[
        "medium.com",
        "dev.to",
        "hackernoon.com",
        "freecodecamp.org",
        "towardsdatascience.com",
        "geeksforgeeks.org",
        "w3schools.com",
    ];
    if AGGREGATORS.iter().any(|d| lower.contains(d)) {
        return 0.85;
    }

    1.0 // Neutral
}

/// Assess information density of a title (0.0 = vague, 1.0 = highly specific).
/// Rewards titles containing concrete, actionable details:
/// - Version numbers ("v2.0", "React 19")
/// - Quantified claims ("10MB", "100x faster")
/// - Comparison/benchmark language
/// - Specific technical terms (migration, changelog, CVE)
fn assess_information_density(title: &str) -> f32 {
    let mut density: f32 = 0.5; // Start at neutral
    let lower = title.to_lowercase();

    // Boost: contains version numbers (v2.0, v19, 3.x, React 19)
    let has_version = lower.contains(" v") && lower.chars().any(|c| c.is_ascii_digit())
        || lower
            .as_bytes()
            .windows(3)
            .any(|w| w[0].is_ascii_digit() && w[1] == b'.' && w[2].is_ascii_digit());
    if has_version {
        density += 0.15;
    }

    // Boost: quantified claims (10MB, 100x, 5 seconds, 10K)
    let quantity_patterns = [
        "mb ",
        "gb ",
        "kb ",
        "ms ",
        " seconds",
        " minutes",
        "x faster",
        "x slower",
        "x improvement",
        "x speed",
        "k stars",
        "k users",
        "% ",
        "10x",
        "100x",
    ];
    if quantity_patterns.iter().any(|p| lower.contains(p)) {
        density += 0.10;
    }

    // Boost: comparison/benchmark language (concrete, measurable)
    let comparison_patterns = [
        " vs ",
        " versus ",
        "benchmark",
        "comparison",
        "performance",
        "latency",
        "throughput",
    ];
    if comparison_patterns.iter().any(|p| lower.contains(p)) {
        density += 0.10;
    }

    // Boost: specific technical terms (actionable content)
    let specific_indicators = [
        "migration",
        "changelog",
        "breaking change",
        "deprecat",
        "vulnerability",
        "advisory",
        "release",
        "architecture",
        "implementation",
        "production",
        "zero-day",
        "cve-",
    ];
    if specific_indicators.iter().any(|p| lower.contains(p)) {
        density += 0.10;
    }

    // Penalty: vague qualifiers without substance
    let vague_patterns = [
        "interesting",
        "thoughts on",
        "opinions on",
        "what do you think",
        "anyone else",
        "is it just me",
        "am i the only",
        "hot take",
        "unpopular opinion",
    ];
    if vague_patterns.iter().any(|p| lower.contains(p)) {
        density -= 0.15;
    }

    density.clamp(0.0, 1.0)
}

/// Shared stop-word list for anti-gaming heuristics.
/// Common English words that should not count as "significant" or "repeated keywords".
const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "how", "that", "this", "your", "from", "about", "into", "will",
    "have", "when", "what", "does", "more", "than", "just", "like", "also", "been", "were", "them",
    "they", "some", "each", "which", "their", "then", "there", "would", "could", "should", "being",
    "over", "most", "very", "only", "other", "using", "used", "here", "after", "before", "between",
    "where", "while", "because", "through", "during", "without", "again", "further", "once",
    "still", "can", "not", "but", "its", "are", "was", "has", "had", "all", "any", "who", "why",
    "our", "out", "off", "own", "too", "now", "new", "way",
];

/// Returns true if `word` is a stop word (case-insensitive, assumes lowercase input).
fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

/// Count how many times any single word (4+ chars) repeats in the title.
/// "Rust async Rust patterns async Rust" -> "rust" appears 3 times.
/// 2 repeats -> -0.05, 3+ repeats -> -0.15, 4+ repeats -> -0.25.
/// Exempt: common English stop words.
fn keyword_concentration_penalty(title: &str) -> f32 {
    use std::collections::HashMap;

    let mut counts: HashMap<&str, u32> = HashMap::new();
    let lower = title.to_lowercase();
    // We need to collect words from the lowered string to avoid lifetime issues.
    // Re-split from the lowered string.
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .filter(|w| !is_stop_word(w))
        .collect();

    for word in &words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let max_repeats = counts.values().copied().max().unwrap_or(0);

    // Only penalize at 3+ repeats. 2 repeats is common in legitimate titles
    // like "React vs React Native" or "How Rust Makes Rust Developers Happy".
    match max_repeats {
        0..=2 => 0.0,
        3 => -0.10,
        _ => -0.20, // 4+
    }
}

/// Extract significant words from title and body, compute overlap.
/// coherence = |title_words ∩ body_words| / |title_words|
/// If coherence < 0.30 -> -0.20 penalty (title promises what body doesn't deliver)
/// If coherence < 0.50 -> -0.10 penalty
/// Only triggers when title has 3+ significant tech words.
fn title_body_coherence_penalty(title: &str, content: &str) -> f32 {
    use std::collections::HashSet;

    let extract_significant = |text: &str| -> HashSet<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 4)
            .filter(|w| !is_stop_word(w))
            .map(|w| w.to_string())
            .collect()
    };

    let title_words = extract_significant(title);
    let body_words = extract_significant(content);

    // Only trigger when title has 3+ significant words
    if title_words.len() < 3 {
        return 0.0;
    }

    // If body is empty, we can't assess coherence — no penalty
    if body_words.is_empty() {
        return 0.0;
    }

    let overlap = title_words.intersection(&body_words).count();
    let coherence = overlap as f32 / title_words.len() as f32;

    if coherence < 0.30 {
        -0.20
    } else if coherence < 0.50 {
        -0.10
    } else {
        0.0
    }
}

/// unique_words / total_words for the title.
/// Titles with diversity < 0.50 get -0.15 penalty (extreme keyword soup).
/// Only the harshest tier: legitimate comparative titles ("React vs React
/// Native: When to Use React Native Over React") hit ~0.57 and must pass.
fn title_diversity_penalty(title: &str) -> f32 {
    use std::collections::HashSet;

    let words: Vec<String> = title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect();

    if words.is_empty() {
        return 0.0;
    }

    let unique: HashSet<&String> = words.iter().collect();
    let diversity = unique.len() as f32 / words.len() as f32;

    // Only penalize extreme keyword soup (diversity < 0.50 means every word
    // appears at least twice on average). The 0.70 tier was removed because
    // legitimate comparative titles like "React vs React Native: When to Use
    // React Native Over React" hit 0.57 diversity naturally.
    if diversity < 0.50 {
        -0.15
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clickbait_penalty() {
        let q = assess_title_quality("You Won't Believe What Rust Can Do!!!");
        assert!(q <= 0.5, "Clickbait should score low: {}", q);
    }

    #[test]
    fn test_technical_title_quality() {
        let q = assess_title_quality("Tokio 1.34 release: new task scheduling improvements");
        assert!(q > 0.8, "Technical title should score high: {}", q);
    }

    #[test]
    fn test_all_caps_penalty() {
        let q = assess_title_quality("BREAKING: EVERYTHING IS BROKEN");
        assert!(q < 0.8, "ALL CAPS should be penalized: {}", q);
    }

    #[test]
    fn test_content_depth_empty() {
        let d = assess_content_depth("");
        assert_eq!(d, 0.3);
    }

    #[test]
    fn test_content_depth_with_code() {
        let content = "Here is how to use it:\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\nThis function prints hello to stdout.";
        let d = assess_content_depth(content);
        assert!(
            d > 0.3,
            "Content with code should score higher than baseline: {}",
            d
        );
    }

    #[test]
    fn test_source_authority() {
        assert!(assess_source_authority("https://github.com/rust-lang/rust") > 1.0);
        assert!(assess_source_authority("https://medium.com/some-article") < 1.0);
        assert_eq!(assess_source_authority("https://example.com/post"), 1.0);
    }

    #[test]
    fn test_quality_multiplier_range() {
        let high = compute_content_quality(
            "Tokio v1.34: task scheduling",
            "Long technical content with ```code blocks``` and detailed analysis...",
            Some("https://github.com/tokio-rs/tokio"),
        );
        assert!(
            high.multiplier >= 0.5 && high.multiplier <= 1.2,
            "Multiplier out of range: {}",
            high.multiplier
        );

        let low = compute_content_quality(
            "You Won't Believe This INSANE Trick!!!",
            "",
            Some("https://clickbait.com"),
        );
        assert!(
            low.multiplier < high.multiplier,
            "Low quality should have lower multiplier"
        );
    }

    #[test]
    fn test_short_vague_title_penalty() {
        let vague = assess_title_quality("where to start");
        let specific = assess_title_quality("Building REST APIs with Axum and Tokio");
        assert!(
            vague < specific,
            "Vague title ({}) should score lower than specific ({})",
            vague,
            specific
        );
        assert!(
            vague <= 0.7,
            "Vague 3-word title should be penalized: {}",
            vague
        );
    }

    // ====================================================================
    // assess_information_density tests
    // ====================================================================

    #[test]
    fn test_info_density_version_numbers() {
        let dense = assess_information_density("React v19 released with server components");
        let vague = assess_information_density("New React features released");
        assert!(
            dense > vague,
            "Version numbers should boost density: {} vs {}",
            dense,
            vague
        );
    }

    #[test]
    fn test_info_density_quantified_claims() {
        let dense = assess_information_density("Built a Julia IDE in 10MB install");
        let vague = assess_information_density("Built a Julia IDE");
        assert!(
            dense > vague,
            "Quantified claims should boost density: {} vs {}",
            dense,
            vague
        );
    }

    #[test]
    fn test_info_density_vague_penalty() {
        let specific = assess_information_density("SQLite migration guide for breaking changes");
        let vague = assess_information_density("Interesting thoughts on databases anyone else");
        assert!(
            specific > vague,
            "Vague titles should score lower: {} vs {}",
            specific,
            vague
        );
    }

    #[test]
    fn test_info_density_comparison_boost() {
        let dense = assess_information_density("Bun vs Node.js benchmark throughput comparison");
        let plain = assess_information_density("Testing Bun and Node.js together");
        assert!(
            dense > plain,
            "Comparison content should boost density: {} vs {}",
            dense,
            plain
        );
    }

    #[test]
    fn test_info_density_range() {
        let high = assess_information_density(
            "React v19.1 benchmark: 100x faster rendering migration changelog",
        );
        let low = assess_information_density("thoughts on stuff is it just me hot take");
        assert!(high <= 1.0 && high >= 0.0, "Should be in range: {}", high);
        assert!(low <= 1.0 && low >= 0.0, "Should be in range: {}", low);
    }

    // ====================================================================
    // Anti-gaming defense tests
    // ====================================================================

    // --- keyword_concentration_penalty ---

    #[test]
    fn test_keyword_concentration_no_repeats() {
        let p = keyword_concentration_penalty("Building REST APIs with Axum and Tokio");
        assert_eq!(p, 0.0, "No repeats should have zero penalty: {}", p);
    }

    #[test]
    fn test_keyword_concentration_two_repeats_allowed() {
        // 2 repeats is legitimate: "React vs React Native"
        let p = keyword_concentration_penalty("Rust patterns in Rust systems programming");
        assert_eq!(p, 0.0, "2 repeats should be allowed (legitimate): {}", p);
    }

    #[test]
    fn test_keyword_concentration_three_repeats() {
        let p = keyword_concentration_penalty("Rust async Rust patterns async Rust");
        assert!(
            (p - (-0.10)).abs() < f32::EPSILON,
            "3 repeats of 'rust' should give -0.10: {}",
            p
        );
    }

    #[test]
    fn test_keyword_concentration_four_repeats() {
        let p = keyword_concentration_penalty("Rust Rust Rust Rust framework");
        assert!(
            (p - (-0.20)).abs() < f32::EPSILON,
            "4 repeats should give -0.20: {}",
            p
        );
    }

    #[test]
    fn test_keyword_concentration_stop_words_exempt() {
        // "this" and "with" are stop words — repeated but should not trigger
        let p = keyword_concentration_penalty("this with this with this with");
        assert_eq!(
            p, 0.0,
            "Stop word repeats should not trigger penalty: {}",
            p
        );
    }

    #[test]
    fn test_keyword_concentration_short_words_exempt() {
        // Words under 4 chars should be ignored
        let p = keyword_concentration_penalty("the API API API use for fun");
        assert_eq!(p, 0.0, "Short word repeats should not trigger: {}", p);
    }

    // --- title_body_coherence_penalty ---

    #[test]
    fn test_coherence_good_match() {
        let p = title_body_coherence_penalty(
            "Building React apps with Tauri and Rust",
            "This article covers building React applications using the Tauri framework powered by Rust.",
        );
        assert_eq!(
            p, 0.0,
            "Good title-body match should have no penalty: {}",
            p
        );
    }

    #[test]
    fn test_coherence_poor_match() {
        let p = title_body_coherence_penalty(
            "React Rust Tauri performance benchmarks",
            "Today we discuss cooking recipes and gardening tips for beginners.",
        );
        assert!(p <= -0.10, "Title-body mismatch should be penalized: {}", p);
    }

    #[test]
    fn test_coherence_empty_body_no_penalty() {
        let p = title_body_coherence_penalty("React Rust Tauri performance benchmarks", "");
        assert_eq!(p, 0.0, "Empty body should not trigger penalty: {}", p);
    }

    #[test]
    fn test_coherence_few_title_words_no_penalty() {
        // Title with fewer than 3 significant words should not trigger
        let p = title_body_coherence_penalty(
            "Rust news",
            "Completely unrelated body content about cooking.",
        );
        assert_eq!(
            p, 0.0,
            "Short title should not trigger coherence check: {}",
            p
        );
    }

    // --- title_diversity_penalty ---

    #[test]
    fn test_diversity_normal_title() {
        let p = title_diversity_penalty("Building REST APIs with Axum and Tokio");
        assert_eq!(p, 0.0, "Normal diverse title should have no penalty: {}", p);
    }

    #[test]
    fn test_diversity_keyword_soup() {
        // Every word repeated: diversity = 3/6 = 0.50 exactly, which is < 0.50? No, 0.50 is not < 0.50.
        // Need diversity strictly < 0.50 for -0.15
        let p = title_diversity_penalty("Rust Rust Rust React React React AI AI");
        // unique=3, total=8 → 0.375
        assert!(
            (p - (-0.15)).abs() < f32::EPSILON,
            "Low diversity keyword soup should get -0.15: {}",
            p
        );
    }

    #[test]
    fn test_diversity_moderate_repetition_allowed() {
        // Legitimate comparative titles repeat terms naturally.
        // "Rust patterns Rust async patterns Rust async guide"
        // unique: rust, patterns, async, guide = 4, total = 8 → 0.50
        // 0.50 is NOT < 0.50, so no penalty (mild tier removed).
        let p = title_diversity_penalty("Rust patterns Rust async patterns Rust async guide");
        assert_eq!(
            p, 0.0,
            "0.50 diversity should NOT be penalized (legitimate repetition): {}",
            p
        );
    }

    #[test]
    fn test_diversity_empty_title() {
        let p = title_diversity_penalty("");
        assert_eq!(p, 0.0, "Empty title should have no penalty: {}", p);
    }

    // --- Integration test: anti-gaming titles get lower multiplier ---

    #[test]
    fn test_gaming_title_lower_than_genuine() {
        let genuine = compute_content_quality(
            "How we migrated our PostgreSQL database to CockroachDB",
            "This article describes our migration from PostgreSQL to CockroachDB including schema changes and performance results.",
            None,
        );
        let gamed = compute_content_quality(
            "Rust Rust Rust AI AI AI Docker Docker Docker",
            "A short post about nothing in particular.",
            None,
        );
        assert!(
            gamed.multiplier < genuine.multiplier,
            "Gamed title ({}) should score lower than genuine ({})",
            gamed.multiplier,
            genuine.multiplier
        );
    }

    #[test]
    fn test_gaming_multiplier_still_in_range() {
        let q =
            compute_content_quality("Rust Rust Rust Rust async Rust performance Rust", "", None);
        assert!(
            q.multiplier >= 0.5 && q.multiplier <= 1.2,
            "Multiplier must stay in [0.5, 1.2]: {}",
            q.multiplier
        );
    }
}
