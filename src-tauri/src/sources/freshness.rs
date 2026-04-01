//! Content freshness validation for source adapters.
//!
//! Detects "zombie" sources — APIs that return HTTP 200 but deliver stale,
//! empty, or structurally broken data. Pure validation: no network calls,
//! no database access.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::SourceItem;

// ============================================================================
// Types
// ============================================================================

/// Health states for a content source, ordered by severity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceHealthState {
    /// Everything looks good.
    Healthy,
    /// Functional but below expectations.
    Degraded(String),
    /// Content exists but is old.
    Stale(String),
    /// Worst: API works but data is dead.
    Zombie(String),
}

impl SourceHealthState {
    /// Numeric severity for comparison (higher = worse).
    fn severity(&self) -> u8 {
        match self {
            Self::Healthy => 0,
            Self::Degraded(_) => 1,
            Self::Stale(_) => 2,
            Self::Zombie(_) => 3,
        }
    }
}

/// Result of freshness validation for a single fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessReport {
    /// Source name that was validated.
    pub source: String,
    /// Overall health state (worst of all checks).
    pub state: SourceHealthState,
    /// Number of items in this fetch.
    pub items_fetched: usize,
    /// Age of the newest item in seconds, if a timestamp was found.
    pub newest_item_age_secs: Option<u64>,
    /// Ratio of items that were already seen in the previous fetch (0.0–1.0).
    pub duplicate_ratio: f64,
    /// Human-readable warnings collected during validation.
    pub warnings: Vec<String>,
}

// ============================================================================
// Source-specific thresholds
// ============================================================================

/// Returns `(min_expected_items, max_acceptable_age_secs)` for a source.
pub fn get_source_thresholds(source_name: &str) -> (usize, u64) {
    match source_name {
        "hackernews" => (20, 3600),        // 20 items, 1h freshness
        "reddit" => (10, 7200),            // 10 items, 2h freshness
        "github" => (5, 86400),            // 5 items, 24h freshness
        "arxiv" => (3, 172800),            // 3 items, 48h freshness (papers)
        "rss" => (1, 86400),               // 1 item, 24h freshness
        "twitter" => (5, 3600),            // 5 items, 1h freshness
        "devto" => (5, 43200),             // 5 items, 12h freshness
        "lobsters" => (10, 7200),          // 10 items, 2h freshness
        "producthunt" => (3, 86400),       // 3 items, 24h freshness
        "youtube" => (1, 604800),          // 1 item, 7d freshness (channels)
        "cve" => (0, 604800),              // 0 items ok, 7d freshness
        "osv" => (0, 604800),              // 0 items ok, 7d (vulnerability feed)
        "npm_registry" => (5, 86400),      // 5 items, 24h (package versions)
        "pypi" => (5, 86400),              // 5 items, 24h
        "crates_io" => (5, 86400),         // 5 items, 24h
        "go_modules" => (5, 86400),        // 5 items, 24h
        "huggingface" => (3, 86400),       // 3 items, 24h
        "stackoverflow" => (10, 21600),    // 10 items, 6h
        "bluesky" => (5, 7200),            // 5 items, 2h
        "papers_with_code" => (3, 172800), // 3 items, 48h
        _ => (1, 86400),                   // default: 1 item, 24h
    }
}

// ============================================================================
// Core validation
// ============================================================================

/// Validate the freshness and health of a batch of fetched items.
///
/// This is pure validation — call it after a successful fetch to detect
/// degraded, stale, or zombie sources before the data enters the pipeline.
pub fn validate_freshness(
    source_name: &str,
    items: &[SourceItem],
    previous_ids: &[String],
    expected_min_items: usize,
    max_acceptable_age_secs: u64,
) -> FreshnessReport {
    let mut warnings: Vec<String> = Vec::new();
    let mut worst_state = SourceHealthState::Healthy;

    let items_fetched = items.len();

    // ── 1. Empty check ──────────────────────────────────────────────────
    if items.is_empty() && expected_min_items > 0 {
        let reason = "No items returned".to_string();
        warnings.push(reason.clone());
        worst_state = escalate(worst_state, SourceHealthState::Degraded(reason));
    }

    // ── 2. Volume check ─────────────────────────────────────────────────
    if expected_min_items > 0 && items_fetched > 0 && items_fetched < expected_min_items / 3 {
        let reason =
            format!("Low volume: {items_fetched} items, expected at least {expected_min_items}");
        warnings.push(reason.clone());
        worst_state = escalate(worst_state, SourceHealthState::Degraded(reason));
    }

    // ── 3. Freshness check ──────────────────────────────────────────────
    let newest_age = find_newest_item_age(items, source_name);
    if let Some(age_secs) = newest_age {
        if age_secs > max_acceptable_age_secs {
            let hours = age_secs / 3600;
            let reason = format!("Newest item is {hours}h old");
            warnings.push(reason.clone());
            worst_state = escalate(worst_state, SourceHealthState::Stale(reason));
        }
    }

    // ── 4. Duplicate check ──────────────────────────────────────────────
    let duplicate_ratio = compute_duplicate_ratio(items, previous_ids);
    if items_fetched > 0 && duplicate_ratio > 0.9 {
        let pct = (duplicate_ratio * 100.0).round() as u32;
        let reason = format!("{pct}% duplicate content — source may be frozen");
        warnings.push(reason.clone());
        worst_state = escalate(worst_state, SourceHealthState::Zombie(reason));
    }

    // ── Logging ─────────────────────────────────────────────────────────
    match &worst_state {
        SourceHealthState::Healthy => {
            info!(
                source = source_name,
                items = items_fetched,
                "Source freshness: healthy"
            );
        }
        SourceHealthState::Degraded(reason) => {
            warn!(
                source = source_name,
                items = items_fetched,
                reason = reason.as_str(),
                "Source freshness: DEGRADED"
            );
        }
        SourceHealthState::Stale(reason) => {
            warn!(
                source = source_name,
                items = items_fetched,
                reason = reason.as_str(),
                "Source freshness: STALE"
            );
        }
        SourceHealthState::Zombie(reason) => {
            warn!(
                source = source_name,
                items = items_fetched,
                reason = reason.as_str(),
                "Source freshness: ZOMBIE"
            );
        }
    }

    FreshnessReport {
        source: source_name.to_string(),
        state: worst_state,
        items_fetched,
        newest_item_age_secs: newest_age,
        duplicate_ratio,
        warnings,
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Escalate to the worse of two states.
fn escalate(current: SourceHealthState, candidate: SourceHealthState) -> SourceHealthState {
    if candidate.severity() > current.severity() {
        candidate
    } else {
        current
    }
}

/// Compute the fraction of current items whose IDs were already seen.
fn compute_duplicate_ratio(items: &[SourceItem], previous_ids: &[String]) -> f64 {
    if items.is_empty() || previous_ids.is_empty() {
        return 0.0;
    }

    let prev_set: HashSet<&str> = previous_ids
        .iter()
        .map(std::string::String::as_str)
        .collect();

    let dup_count = items
        .iter()
        .filter(|item| {
            // Check source_id first, then URL as fallback
            if prev_set.contains(item.source_id.as_str()) {
                return true;
            }
            if let Some(url) = &item.url {
                return prev_set.contains(url.as_str());
            }
            false
        })
        .count();

    dup_count as f64 / items.len() as f64
}

/// Find the age (in seconds since now) of the newest item across all items.
///
/// Adapters store timestamps inconsistently in metadata. We check common keys:
/// `published_at`, `created_at`, `pub_date`, `published`, `updated_at`, `updated`.
fn find_newest_item_age(items: &[SourceItem], _source_name: &str) -> Option<u64> {
    let now = Utc::now();

    let timestamp_keys = [
        "published_at",
        "created_at",
        "pub_date",
        "published",
        "updated_at",
        "updated",
    ];

    let mut newest: Option<DateTime<Utc>> = None;

    for item in items {
        let Some(meta) = &item.metadata else {
            continue;
        };

        for key in &timestamp_keys {
            if let Some(val) = meta.get(key) {
                if let Some(ts) = parse_timestamp(val) {
                    newest = Some(match newest {
                        Some(prev) if ts > prev => ts,
                        Some(prev) => prev,
                        None => ts,
                    });
                    break; // Found a timestamp for this item, move to next
                }
            }
        }
    }

    newest.map(|ts| {
        let diff = now.signed_duration_since(ts);
        if diff.num_seconds() < 0 {
            0 // Item is in the future (clock skew), treat as fresh
        } else {
            diff.num_seconds() as u64
        }
    })
}

/// Try to parse a JSON value as a datetime.
///
/// Handles: ISO 8601 strings, Unix timestamps (integer or float).
fn parse_timestamp(val: &serde_json::Value) -> Option<DateTime<Utc>> {
    match val {
        serde_json::Value::String(s) => {
            // Try ISO 8601 with timezone
            if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                return Some(dt.with_timezone(&Utc));
            }
            // Try ISO 8601 without timezone (assume UTC)
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
                return Some(dt.and_utc());
            }
            // Try common date formats
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
                return Some(dt.and_utc());
            }
            None
        }
        serde_json::Value::Number(n) => {
            // Unix timestamp (seconds)
            if let Some(secs) = n.as_i64() {
                return DateTime::from_timestamp(secs, 0);
            }
            if let Some(secs) = n.as_f64() {
                return DateTime::from_timestamp(secs as i64, 0);
            }
            None
        }
        _ => None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(source: &str, id: &str, title: &str) -> SourceItem {
        SourceItem::new(source, id, title)
    }

    fn make_item_with_timestamp(source: &str, id: &str, ts: &str) -> SourceItem {
        SourceItem::new(source, id, "Test").with_metadata(serde_json::json!({ "published_at": ts }))
    }

    fn now_minus_secs(secs: i64) -> String {
        let dt = Utc::now() - chrono::Duration::seconds(secs);
        dt.to_rfc3339()
    }

    // ── Healthy scenarios ───────────────────────────────────────────────

    #[test]
    fn healthy_source_with_fresh_content() {
        let items: Vec<SourceItem> = (0..25)
            .map(|i| make_item_with_timestamp("hackernews", &i.to_string(), &now_minus_secs(600)))
            .collect();

        let report = validate_freshness("hackernews", &items, &[], 20, 3600);
        assert_eq!(report.state, SourceHealthState::Healthy);
        assert_eq!(report.items_fetched, 25);
        assert!(report.warnings.is_empty());
    }

    // ── Empty / low volume ──────────────────────────────────────────────

    #[test]
    fn empty_fetch_is_degraded() {
        let report = validate_freshness("hackernews", &[], &[], 20, 3600);
        assert!(matches!(report.state, SourceHealthState::Degraded(_)));
        assert_eq!(report.items_fetched, 0);
    }

    #[test]
    fn low_volume_is_degraded() {
        let items: Vec<SourceItem> = (0..2)
            .map(|i| make_item("reddit", &i.to_string(), "Low"))
            .collect();

        // 2 items but expected 10, and 2 < 10/3 = 3 (integer div) → triggers volume check
        let report = validate_freshness("reddit", &items, &[], 10, 7200);
        assert!(matches!(report.state, SourceHealthState::Degraded(_)));
    }

    #[test]
    fn zero_expected_items_skips_empty_check() {
        let report = validate_freshness("cve", &[], &[], 0, 604800);
        assert_eq!(report.state, SourceHealthState::Healthy);
    }

    // ── Stale content ───────────────────────────────────────────────────

    #[test]
    fn stale_content_detected() {
        let old_ts = now_minus_secs(10_000); // ~2.7h old
        let items = vec![make_item_with_timestamp("hackernews", "1", &old_ts)];

        let report = validate_freshness("hackernews", &items, &[], 1, 3600);
        assert!(matches!(report.state, SourceHealthState::Stale(_)));
        assert!(report.newest_item_age_secs.unwrap() > 3600);
    }

    #[test]
    fn fresh_content_not_stale() {
        let fresh_ts = now_minus_secs(300); // 5 minutes old
        let items = vec![make_item_with_timestamp("hackernews", "1", &fresh_ts)];

        let report = validate_freshness("hackernews", &items, &[], 1, 3600);
        // Should not be stale (might be degraded for volume, but not stale)
        assert!(!matches!(report.state, SourceHealthState::Stale(_)));
    }

    // ── Duplicate / zombie detection ────────────────────────────────────

    #[test]
    fn full_duplicate_is_zombie() {
        let items: Vec<SourceItem> = (0..10)
            .map(|i| make_item("reddit", &format!("id_{}", i), "Same"))
            .collect();

        let prev_ids: Vec<String> = (0..10).map(|i| format!("id_{}", i)).collect();

        let report = validate_freshness("reddit", &items, &prev_ids, 10, 7200);
        assert!(matches!(report.state, SourceHealthState::Zombie(_)));
        assert!((report.duplicate_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn partial_overlap_not_zombie() {
        let items: Vec<SourceItem> = (0..10)
            .map(|i| make_item("reddit", &format!("id_{}", i), "Mix"))
            .collect();

        // Only 5 of 10 are from previous fetch
        let prev_ids: Vec<String> = (0..5).map(|i| format!("id_{}", i)).collect();

        let report = validate_freshness("reddit", &items, &prev_ids, 10, 7200);
        assert!(!matches!(report.state, SourceHealthState::Zombie(_)));
        assert!((report.duplicate_ratio - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn url_based_duplicate_detection() {
        let items = vec![SourceItem::new("rss", "unique_id_1", "Article")
            .with_url(Some("https://example.com/article".to_string()))];

        let prev_ids = vec!["https://example.com/article".to_string()];

        let report = validate_freshness("rss", &items, &prev_ids, 1, 86400);
        assert!((report.duplicate_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn no_previous_ids_means_zero_duplicates() {
        let items = vec![make_item("reddit", "1", "New")];
        let report = validate_freshness("reddit", &items, &[], 1, 7200);
        assert!((report.duplicate_ratio).abs() < f64::EPSILON);
    }

    // ── Combined / escalation ───────────────────────────────────────────

    #[test]
    fn stale_plus_duplicate_escalates_to_zombie() {
        let old_ts = now_minus_secs(10_000);
        let items: Vec<SourceItem> = (0..5)
            .map(|i| {
                SourceItem::new("devto", &format!("id_{}", i), "Stale")
                    .with_metadata(serde_json::json!({ "published_at": old_ts }))
            })
            .collect();

        let prev_ids: Vec<String> = (0..5).map(|i| format!("id_{}", i)).collect();

        let report = validate_freshness("devto", &items, &prev_ids, 5, 3600);
        // Zombie is worse than Stale, so escalation picks Zombie
        assert!(matches!(report.state, SourceHealthState::Zombie(_)));
        assert!(report.warnings.len() >= 2);
    }

    // ── Timestamp parsing ───────────────────────────────────────────────

    #[test]
    fn parse_rfc3339_timestamp() {
        let val = serde_json::json!("2026-03-20T12:00:00Z");
        assert!(parse_timestamp(&val).is_some());
    }

    #[test]
    fn parse_rfc3339_with_offset() {
        let val = serde_json::json!("2026-03-20T12:00:00+05:00");
        assert!(parse_timestamp(&val).is_some());
    }

    #[test]
    fn parse_naive_datetime() {
        let val = serde_json::json!("2026-03-20T12:00:00");
        assert!(parse_timestamp(&val).is_some());
    }

    #[test]
    fn parse_unix_timestamp() {
        let val = serde_json::json!(1742486400);
        assert!(parse_timestamp(&val).is_some());
    }

    #[test]
    fn parse_null_returns_none() {
        let val = serde_json::json!(null);
        assert!(parse_timestamp(&val).is_none());
    }

    #[test]
    fn parse_garbage_string_returns_none() {
        let val = serde_json::json!("not a date");
        assert!(parse_timestamp(&val).is_none());
    }

    // ── Metadata key detection ──────────────────────────────────────────

    #[test]
    fn detects_created_at_key() {
        let ts = now_minus_secs(100);
        let item = SourceItem::new("lobsters", "1", "Test")
            .with_metadata(serde_json::json!({ "created_at": ts }));

        let age = find_newest_item_age(&[item], "lobsters");
        assert!(age.is_some());
        assert!(age.unwrap() < 200);
    }

    #[test]
    fn detects_pub_date_key() {
        let ts = now_minus_secs(100);
        let item = SourceItem::new("rss", "1", "Test")
            .with_metadata(serde_json::json!({ "pub_date": ts }));

        let age = find_newest_item_age(&[item], "rss");
        assert!(age.is_some());
        assert!(age.unwrap() < 200);
    }

    #[test]
    fn detects_updated_at_key() {
        let ts = now_minus_secs(100);
        let item = SourceItem::new("github", "1", "Test")
            .with_metadata(serde_json::json!({ "updated_at": ts }));

        let age = find_newest_item_age(&[item], "github");
        assert!(age.is_some());
        assert!(age.unwrap() < 200);
    }

    #[test]
    fn no_metadata_returns_none() {
        let item = make_item("hackernews", "1", "No timestamp");
        let age = find_newest_item_age(&[item], "hackernews");
        assert!(age.is_none());
    }

    // ── Threshold sanity ────────────────────────────────────────────────

    #[test]
    fn all_known_sources_have_thresholds() {
        let sources = [
            "hackernews",
            "reddit",
            "github",
            "arxiv",
            "rss",
            "twitter",
            "devto",
            "lobsters",
            "producthunt",
            "youtube",
            "cve",
        ];

        for source in &sources {
            let (min, max_age) = get_source_thresholds(source);
            // Sanity: max age should be at least 1 hour
            assert!(
                max_age >= 3600 || *source == "cve",
                "{} max_age too low: {}",
                source,
                max_age
            );
            // Sanity: min items should be reasonable
            assert!(min <= 30, "{} min_items too high: {}", source, min);
        }
    }

    #[test]
    fn unknown_source_gets_defaults() {
        let (min, max_age) = get_source_thresholds("unknown_new_source");
        assert_eq!(min, 1);
        assert_eq!(max_age, 86400);
    }

    // ── Severity ordering ───────────────────────────────────────────────

    #[test]
    fn severity_ordering() {
        assert!(
            SourceHealthState::Healthy.severity()
                < SourceHealthState::Degraded("".into()).severity()
        );
        assert!(
            SourceHealthState::Degraded("".into()).severity()
                < SourceHealthState::Stale("".into()).severity()
        );
        assert!(
            SourceHealthState::Stale("".into()).severity()
                < SourceHealthState::Zombie("".into()).severity()
        );
    }

    #[test]
    fn escalate_keeps_worse_state() {
        let result = escalate(
            SourceHealthState::Stale("old".into()),
            SourceHealthState::Degraded("low".into()),
        );
        // Stale is worse than Degraded, so Stale should be kept
        assert!(matches!(result, SourceHealthState::Stale(_)));
    }

    #[test]
    fn escalate_upgrades_to_worse_state() {
        let result = escalate(
            SourceHealthState::Degraded("low".into()),
            SourceHealthState::Zombie("frozen".into()),
        );
        assert!(matches!(result, SourceHealthState::Zombie(_)));
    }
}
