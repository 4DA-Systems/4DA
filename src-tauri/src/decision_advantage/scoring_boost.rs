//! Scoring pipeline integration — boosts items that match open decision windows.
//!
//! Called during relevance scoring to amplify items related to active windows.
//! Boost magnitudes by window type:
//! - security_patch: 0.20
//! - migration:      0.15
//! - adoption:       0.10
//! - knowledge:      0.10
//! - default:        0.05

use super::DecisionWindow;

// ============================================================================
// Boost constants by window type
// ============================================================================

const BOOST_SECURITY: f32 = 0.20;
const BOOST_MIGRATION: f32 = 0.15;
const BOOST_ADOPTION: f32 = 0.10;
const BOOST_KNOWLEDGE: f32 = 0.10;
const BOOST_DEFAULT: f32 = 0.05;

/// Maximum effective boost (hard cap).
const MAX_BOOST: f32 = 0.20;

// ============================================================================
// Public API
// ============================================================================

/// Compute a scoring boost for an item based on open decision windows.
///
/// Returns `(boost, matched_window_id)` where `boost` is 0.0 to `MAX_BOOST`
/// and `matched_window_id` is the ID of the best-matching window (if any).
///
/// Match signals (cumulative, capped at 1.0):
/// - Dependency name overlap with item title/content: +0.6
/// - Topic overlap with window title:                 +0.3
/// - Title keyword match:                             +0.1
pub(crate) fn compute_decision_window_boost(
    open_windows: &[DecisionWindow],
    title: &str,
    content: &str,
    topics: &[String],
    matched_dep_names: &[String],
) -> (f32, Option<i64>) {
    if open_windows.is_empty() {
        return (0.0, None);
    }

    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();

    let mut best_boost: f32 = 0.0;
    let mut best_window_id: Option<i64> = None;

    for window in open_windows {
        let match_score = compute_match_score(
            window,
            &title_lower,
            &content_lower,
            topics,
            matched_dep_names,
        );

        if match_score > 0.0 {
            let type_boost = match window.window_type.as_str() {
                "security_patch" => BOOST_SECURITY,
                "migration" => BOOST_MIGRATION,
                "adoption" => BOOST_ADOPTION,
                "knowledge" => BOOST_KNOWLEDGE,
                _ => BOOST_DEFAULT,
            };

            let effective_boost = (type_boost * match_score.min(1.0)).min(MAX_BOOST);

            if effective_boost > best_boost {
                best_boost = effective_boost;
                best_window_id = Some(window.id);
            }
        }
    }

    (best_boost, best_window_id)
}

// ============================================================================
// Match scoring
// ============================================================================

/// Compute how strongly an item matches a specific window (0.0 - 1.0+).
fn compute_match_score(
    window: &DecisionWindow,
    title_lower: &str,
    content_lower: &str,
    topics: &[String],
    matched_dep_names: &[String],
) -> f32 {
    let mut score = 0.0_f32;

    // Signal 1: dependency name overlap (+0.6)
    if let Some(ref dep) = window.dependency {
        let dep_lower = dep.to_lowercase();
        let dep_in_names = matched_dep_names
            .iter()
            .any(|d| d.to_lowercase() == dep_lower);
        let dep_in_title = title_lower.contains(&dep_lower);
        let dep_in_content = content_lower.contains(&dep_lower);

        if dep_in_names || dep_in_title || dep_in_content {
            score += 0.6;
        }
    }

    // Signal 2: topic overlap with window title (+0.3)
    let window_title_lower = window.title.to_lowercase();
    for topic in topics {
        if window_title_lower.contains(&topic.to_lowercase()) {
            score += 0.3;
            break; // only count once
        }
    }

    // Signal 3: title keyword match (+0.1)
    // Use the first significant word from the window title (skip prefixes like "Security:")
    let significant_word = window
        .title
        .split_whitespace()
        .find(|w| w.len() > 3 && !w.ends_with(':'))
        .unwrap_or("");

    if !significant_word.is_empty() && title_lower.contains(&significant_word.to_lowercase()) {
        score += 0.1;
    }

    score
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_window(id: i64, window_type: &str, dep: Option<&str>) -> DecisionWindow {
        DecisionWindow {
            id,
            window_type: window_type.to_string(),
            title: format!("Test: {}", dep.unwrap_or("generic")),
            description: String::new(),
            urgency: 0.8,
            relevance: 0.7,
            dependency: dep.map(|s| s.to_string()),
            status: "open".to_string(),
            opened_at: "2025-01-15 10:00:00".to_string(),
            expires_at: None,
            lead_time_hours: None,
            streets_engine: None,
        }
    }

    #[test]
    fn test_no_boost_with_empty_windows() {
        let (boost, id) = compute_decision_window_boost(
            &[],
            "Some title",
            "Some content",
            &["rust".to_string()],
            &[],
        );
        assert!((boost - 0.0).abs() < f32::EPSILON);
        assert!(id.is_none());
    }

    #[test]
    fn test_security_boost_on_dep_match() {
        let windows = vec![make_window(42, "security_patch", Some("lodash"))];

        let (boost, id) = compute_decision_window_boost(
            &windows,
            "Critical vulnerability in lodash 4.17",
            "Prototype pollution affecting lodash",
            &[],
            &["lodash".to_string()],
        );

        // dep match (+0.6) + title keyword "lodash" (+0.1) = 0.7, capped at 1.0
        // boost = BOOST_SECURITY * min(0.7, 1.0) = 0.20 * 0.7 = 0.14
        assert!(
            boost > 0.10,
            "security boost should be significant, got {boost}"
        );
        assert!(boost <= MAX_BOOST, "should not exceed max boost");
        assert_eq!(id, Some(42));
    }

    #[test]
    fn test_topic_overlap_boost() {
        let windows = vec![make_window(10, "adoption", Some("bun"))];

        let (boost, id) = compute_decision_window_boost(
            &windows,
            "JavaScript runtime benchmarks 2025",
            "Comparing Node.js, Deno, and Bun performance",
            &["bun".to_string()],
            &[],
        );

        // dep "bun" in content (+0.6) + topic "bun" matches window title (+0.3) = 0.9
        // boost = BOOST_ADOPTION * 0.9 = 0.10 * 0.9 = 0.09
        assert!(
            boost > 0.0,
            "should get boost from topic + dep match, got {boost}"
        );
        assert_eq!(id, Some(10));
    }

    #[test]
    fn test_best_window_selected() {
        let windows = vec![
            make_window(1, "adoption", Some("svelte")),
            make_window(2, "security_patch", Some("lodash")),
        ];

        let (boost, id) = compute_decision_window_boost(
            &windows,
            "lodash security advisory",
            "Update lodash to fix CVE-2025-999",
            &[],
            &["lodash".to_string()],
        );

        // Should match the security window (higher boost factor)
        assert_eq!(id, Some(2), "should match the security_patch window");
        assert!(boost > 0.0);
    }

    #[test]
    fn test_no_match_no_boost() {
        let windows = vec![make_window(5, "migration", Some("angular"))];

        let (boost, id) = compute_decision_window_boost(
            &windows,
            "Rust async patterns in 2025",
            "New approaches to concurrency in Rust",
            &["rust".to_string(), "async".to_string()],
            &["tokio".to_string()],
        );

        assert!(
            (boost - 0.0).abs() < f32::EPSILON,
            "no match should yield zero boost"
        );
        assert!(id.is_none());
    }
}
