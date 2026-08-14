// SPDX-License-Identifier: FSL-1.1-Apache-2.0

// ========================================================================
// Content capping logic — exercises the REAL `cap_on_char_boundary` used by
// fetch_all_sources. These tests previously re-implemented the capping inline,
// so they asserted against a copy of the logic and could not have caught the
// char-boundary panic #422 fixed. They now call the production function.
// ========================================================================

use super::{cap_on_char_boundary, MAX_CONTENT_BYTES as CONTENT_CAP};

#[test]
fn test_content_cap_short_content_unchanged() {
    let content = "Short content that is well under the limit.".to_string();
    let capped = cap_on_char_boundary(content.clone(), CONTENT_CAP);
    assert_eq!(
        capped, content,
        "Short content should pass through unchanged"
    );
}

#[test]
fn test_content_cap_exactly_at_limit() {
    let content = "x".repeat(CONTENT_CAP);
    let capped = cap_on_char_boundary(content, CONTENT_CAP);
    assert_eq!(
        capped.len(),
        CONTENT_CAP,
        "Exact-limit content should not be truncated"
    );
}

#[test]
fn test_content_cap_over_limit_truncated() {
    let content = "y".repeat(CONTENT_CAP + 1000);
    let capped = cap_on_char_boundary(content, CONTENT_CAP);
    assert_eq!(
        capped.len(),
        CONTENT_CAP,
        "Over-limit content should be truncated to 500KB"
    );
}

/// The regression that matters: a raw `&s[..cap]` panics when `cap` lands
/// mid-character. Every cap boundary from 1..=8 bytes into a 3-byte-char
/// string is exercised, so a naive byte cut would panic on most of them.
#[test]
fn test_content_cap_never_splits_a_multibyte_char() {
    // 'あ' is 3 bytes in UTF-8, so most byte offsets are NOT char boundaries.
    let content = "あ".repeat(16);
    for cap in 1..=8 {
        let capped = cap_on_char_boundary(content.clone(), cap);
        assert!(
            capped.len() <= cap,
            "cap {cap}: result must not exceed the byte ceiling"
        );
        assert_eq!(
            capped.len() % 3,
            0,
            "cap {cap}: must cut on a char boundary, never mid-sequence"
        );
        // Re-validating as UTF-8 is the real proof the cut was legal.
        assert!(std::str::from_utf8(capped.as_bytes()).is_ok());
    }
}

/// Emoji are 4-byte sequences and are common in scraped social content.
#[test]
fn test_content_cap_handles_four_byte_chars() {
    let content = "🚀".repeat(8);
    let capped = cap_on_char_boundary(content, 10);
    assert_eq!(
        capped.len(),
        8,
        "10-byte cap over 4-byte chars must floor to 8"
    );
    assert_eq!(capped, "🚀🚀");
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
