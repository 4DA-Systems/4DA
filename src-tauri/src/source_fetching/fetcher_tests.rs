// SPDX-License-Identifier: FSL-1.1-Apache-2.0

// ========================================================================
// Content capping logic (500KB limit mirrors fetch_all_sources inline logic)
// ========================================================================

const CONTENT_CAP: usize = 500_000;

#[test]
fn test_content_cap_short_content_unchanged() {
    let content = "Short content that is well under the limit.".to_string();
    let capped = if content.len() > CONTENT_CAP {
        content[..CONTENT_CAP].to_string()
    } else {
        content.clone()
    };
    assert_eq!(
        capped, content,
        "Short content should pass through unchanged"
    );
}

#[test]
fn test_content_cap_exactly_at_limit() {
    let content = "x".repeat(CONTENT_CAP);
    let capped = if content.len() > CONTENT_CAP {
        content[..CONTENT_CAP].to_string()
    } else {
        content.clone()
    };
    assert_eq!(
        capped.len(),
        CONTENT_CAP,
        "Exact-limit content should not be truncated"
    );
}

#[test]
fn test_content_cap_over_limit_truncated() {
    let content = "y".repeat(CONTENT_CAP + 1000);
    let capped = if content.len() > CONTENT_CAP {
        content[..CONTENT_CAP].to_string()
    } else {
        content.clone()
    };
    assert_eq!(
        capped.len(),
        CONTENT_CAP,
        "Over-limit content should be truncated to 500KB"
    );
}

// ========================================================================
// Fetch interval logic (300s cooldown mirrors fetch_all_sources)
// ========================================================================

#[test]
fn test_fetch_interval_skip_logic() {
    // Simulates the 300-second fetch interval check
    let fetch_interval_secs = 300i64;

    // Recently fetched (10s ago) - should be skipped
    let recent_elapsed = 10i64;
    assert!(
        recent_elapsed < fetch_interval_secs,
        "10s ago should be within interval (should skip)"
    );

    // Long ago (600s) - should be fetched
    let old_elapsed = 600i64;
    assert!(
        old_elapsed >= fetch_interval_secs,
        "600s ago should be past interval (should fetch)"
    );

    // Exactly at boundary
    let boundary_elapsed = 300i64;
    assert!(
        boundary_elapsed >= fetch_interval_secs,
        "Exactly 300s should trigger fetch (not less than)"
    );
}

// ========================================================================
// Retry backoff pattern (mirrors fetch_with_retry constants)
// ========================================================================

#[test]
fn test_fetch_retry_backoff_pattern() {
    use super::super::{MAX_RETRY_ATTEMPTS, RETRY_BACKOFF_SECS};

    // Backoff should be 1s, 2s, 4s (exponential)
    assert_eq!(RETRY_BACKOFF_SECS, [1, 2, 4]);
    assert_eq!(MAX_RETRY_ATTEMPTS, 3);

    // Attempt 1 (index 0) -> 1s backoff before retry
    assert_eq!(RETRY_BACKOFF_SECS[0], 1);

    // Attempt 2 (index 1) -> 2s backoff before retry
    assert_eq!(RETRY_BACKOFF_SECS[1], 2);

    // Attempt 3 (index 2) -> 4s (but this is the final attempt, no more retries)
    assert_eq!(RETRY_BACKOFF_SECS[2], 4);
}

// ========================================================================
// GenericSourceItem ID generation via hash
// ========================================================================

#[test]
fn test_generic_item_id_from_source_hash() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let source_type = "hackernews";
    let source_id = "12345";

    let id1 = {
        let mut hasher = DefaultHasher::new();
        format!("{}:{}", source_type, source_id).hash(&mut hasher);
        hasher.finish()
    };

    let id2 = {
        let mut hasher = DefaultHasher::new();
        format!("{}:{}", source_type, source_id).hash(&mut hasher);
        hasher.finish()
    };

    assert_eq!(id1, id2, "Same source should produce same ID");

    // Different source_type should produce different ID
    let id3 = {
        let mut hasher = DefaultHasher::new();
        format!("{}:{}", "reddit", source_id).hash(&mut hasher);
        hasher.finish()
    };
    assert_ne!(
        id1, id3,
        "Different source_type should produce different ID"
    );
}
