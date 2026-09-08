// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the HTTP status gate and `Retry-After` handling.
//!
//! Split out of `sources/mod.rs` (2026-09-08): that file sits close to the
//! 1000-line hard limit, and a `*_tests.rs` file is exempt from the warning
//! threshold, so the gate stays honest as these grow.

use super::*;
// ---------- Retry-After parsing ----------
//
// Nothing in 4DA read this header until 2026-09-08. arXiv answers 429/503
// with a cooldown and we ignored it, so the breaker cycled forever.

#[test]
fn retry_after_parses_delta_seconds() {
    assert_eq!(parse_retry_after("120"), Some(120));
    assert_eq!(parse_retry_after("  120  "), Some(120));
    assert_eq!(parse_retry_after("0"), Some(0));
}

#[test]
fn retry_after_parses_http_date_to_seconds() {
    let target = chrono::Utc::now() + chrono::Duration::seconds(300);
    let header = target.format("%a, %d %b %Y %H:%M:%S GMT").to_string();

    let parsed = parse_retry_after(&header).expect("HTTP-date must parse");
    // Allow a few seconds of slack for clock/test scheduling.
    assert!(
        (295..=300).contains(&parsed),
        "expected ~300s from an HTTP-date, got {parsed}"
    );
}

#[test]
fn retry_after_http_date_in_the_past_is_zero_not_none() {
    // The server DID announce a cooldown; it has simply elapsed. Reporting
    // None here would silently fall back to the fixed 30s guess.
    let target = chrono::Utc::now() - chrono::Duration::seconds(600);
    let header = target.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    assert_eq!(parse_retry_after(&header), Some(0));
}

#[test]
fn retry_after_is_capped_at_six_hours() {
    assert_eq!(parse_retry_after("999999999"), Some(MAX_RETRY_AFTER_SECS));
}

#[test]
fn retry_after_rejects_unparseable_values() {
    // NEGATIVE TEST: garbage must yield None (fall back to fixed backoff),
    // never a bogus number that would park a healthy source.
    assert_eq!(parse_retry_after("soon"), None);
    assert_eq!(parse_retry_after(""), None);
    assert_eq!(parse_retry_after("   "), None);
    assert_eq!(parse_retry_after("-5"), None);
    assert_eq!(parse_retry_after("12.5"), None);
}

// ---------- status classification ----------

fn classify(status: u16, retry_after: Option<u64>) -> SourceResult<()> {
    classify_http_status_with_retry_after(
        reqwest::StatusCode::from_u16(status).unwrap(),
        retry_after,
        "test API",
    )
}

#[test]
fn classify_429_carries_the_announced_retry_after() {
    let err = classify(429, Some(120)).unwrap_err();
    assert_eq!(err.retry_after_secs(), Some(120));
    assert!(matches!(err, SourceError::RateLimited { .. }));
    assert!(err.to_string().contains("retry after 120s"));
}

#[test]
fn classify_429_without_header_carries_no_hint() {
    let err = classify(429, None).unwrap_err();
    assert!(matches!(err, SourceError::RateLimited { .. }));
    assert_eq!(err.retry_after_secs(), None);
}

#[test]
fn classify_503_with_retry_after_is_rate_limited() {
    let err = classify(503, Some(90)).unwrap_err();
    assert!(matches!(err, SourceError::RateLimited { .. }));
    assert_eq!(err.retry_after_secs(), Some(90));
}

#[test]
fn classify_503_without_retry_after_stays_a_network_error() {
    // NEGATIVE TEST: the 503 branch must NOT swallow plain outages. A bare
    // 503 is a down server, not a rate limit — treating it as one would
    // hide real breakage behind a long, quiet backoff.
    let err = classify(503, None).unwrap_err();
    assert!(
        matches!(err, SourceError::Network(_)),
        "bare 503 must stay a network error, got {err}"
    );
}

#[test]
fn classify_leaves_success_and_other_statuses_alone() {
    // NEGATIVE TEST: what the new gate does NOT touch.
    assert!(classify(200, None).is_ok());
    assert!(classify(204, None).is_ok());
    // A Retry-After on a 2xx is meaningless and must not manufacture an error.
    assert!(classify(200, Some(600)).is_ok());
    assert!(matches!(
        classify(403, Some(600)).unwrap_err(),
        SourceError::Forbidden(_)
    ));
    assert!(matches!(
        classify(500, None).unwrap_err(),
        SourceError::Network(_)
    ));
}

#[test]
fn retry_after_from_headers_reads_the_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    assert_eq!(retry_after_from_headers(&headers), None);

    headers.insert(reqwest::header::RETRY_AFTER, "45".parse().unwrap());
    assert_eq!(retry_after_from_headers(&headers), Some(45));
}

#[test]
fn rate_limited_constructor_caps_the_hint() {
    let err = SourceError::rate_limited_after("test", Some(u64::MAX));
    assert_eq!(err.retry_after_secs(), Some(MAX_RETRY_AFTER_SECS));
}

#[test]
fn retry_after_secs_is_none_for_non_rate_limit_errors() {
    // NEGATIVE TEST: the accessor must not invent a cooldown for errors
    // that never carried one.
    assert_eq!(SourceError::Network("x".into()).retry_after_secs(), None);
    assert_eq!(SourceError::Forbidden("x".into()).retry_after_secs(), None);
    assert_eq!(SourceError::Disabled.retry_after_secs(), None);
}
