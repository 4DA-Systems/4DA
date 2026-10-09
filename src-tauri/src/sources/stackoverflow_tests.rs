use super::*;

#[test]
fn test_stackoverflow_source_creation() {
    let source = StackOverflowSource::new();
    assert_eq!(source.source_type(), "stackoverflow");
    assert_eq!(source.name(), "Stack Overflow");
    assert!(source.config().enabled);
    assert_eq!(source.config().max_items, 20);
    assert_eq!(source.config().fetch_interval_secs, 1800);
    assert_eq!(source.tags.len(), 8);
}

#[test]
fn test_stackoverflow_source_default() {
    let source = StackOverflowSource::default();
    assert_eq!(source.source_type(), "stackoverflow");
}

#[test]
fn test_stackoverflow_json_parsing() {
    let json = r#"{
        "items": [
            {
                "question_id": 12345678,
                "title": "How to handle async errors in Rust?",
                "link": "https://stackoverflow.com/questions/12345678",
                "score": 15,
                "answer_count": 3,
                "view_count": 1200,
                "tags": ["rust", "async-await", "error-handling"],
                "creation_date": 1709251200,
                "is_answered": true
            },
            {
                "question_id": 87654321,
                "title": "TypeScript generic constraints",
                "link": "https://stackoverflow.com/questions/87654321",
                "score": 7,
                "answer_count": null,
                "view_count": null,
                "tags": ["typescript", "generics"],
                "creation_date": null,
                "is_answered": false
            }
        ],
        "has_more": true,
        "quota_remaining": 295
    }"#;

    let resp = parse_response(json).unwrap();
    let items = resp.questions;
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].question_id, 12345678);
    assert_eq!(items[0].title, "How to handle async errors in Rust?");
    assert_eq!(items[0].score, 15);
    assert_eq!(items[0].answer_count, Some(3));
    assert_eq!(items[0].view_count, Some(1200));
    assert!(items[0].is_answered.unwrap());
    assert_eq!(resp.quota_remaining, Some(295));

    // Second item with null optional fields
    assert_eq!(items[1].question_id, 87654321);
    assert!(items[1].answer_count.is_none());
    assert!(items[1].view_count.is_none());
}

/// `THROTTLED_UNTIL` is process-global, so any test that arms it must hold
/// this lock — cargo runs tests in parallel threads inside one process and
/// a leaked deadline would make unrelated tests observe a throttle.
static THROTTLE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn reset_throttle() {
    THROTTLED_UNTIL.store(0, Ordering::Relaxed);
}

/// The exact message Stack Exchange returned on 2026-08-14.
#[test]
fn test_parses_retry_after_from_live_throttle_message() {
    assert_eq!(
        parse_retry_after_secs(
            "too many requests from this IP, more requests available in 46472 seconds"
        ),
        Some(46_472)
    );
}

#[test]
fn test_retry_after_absent_when_unparseable() {
    assert_eq!(parse_retry_after_secs("no deadline here"), None);
    assert_eq!(parse_retry_after_secs("available in zero seconds"), None);
    assert_eq!(parse_retry_after_secs("available in 0 seconds"), None);
}

/// The live 400 body must classify as RateLimited (NOT a bad request) and
/// must arm the breaker. Misclassifying this as `Network`/"HTTP 400" is what
/// disguised a 13-hour IP ban as a malformed query.
#[test]
fn test_throttle_body_classifies_as_ratelimited_and_arms_breaker() {
    let _guard = THROTTLE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_throttle();

    let body = r#"{"error_id":502,"error_message":"too many requests from this IP, more requests available in 46472 seconds","error_name":"throttle_violation"}"#;
    let err = classify_error(reqwest::StatusCode::BAD_REQUEST, body);

    assert!(
        matches!(err, SourceError::RateLimited { .. }),
        "throttle must not be reported as a generic bad request, got {err:?}"
    );

    let remaining = throttle_remaining().expect("breaker must be armed");
    assert!(
        remaining > 46_000 && remaining <= 46_472,
        "breaker should hold the upstream deadline, got {remaining}"
    );

    reset_throttle();
}

/// A real malformed-query 400 must stay a normal error and must NOT arm the
/// breaker — otherwise one bad tag would silence the source for an hour.
#[test]
fn test_genuine_bad_request_does_not_arm_breaker() {
    let _guard = THROTTLE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_throttle();

    let body =
        r#"{"error_id":400,"error_message":"site is required","error_name":"bad_parameter"}"#;
    let err = classify_error(reqwest::StatusCode::BAD_REQUEST, body);

    assert!(
        matches!(err, SourceError::Network(_)),
        "a genuine bad request must not be classed as a rate limit, got {err:?}"
    );
    assert!(
        throttle_remaining().is_none(),
        "breaker must stay disarmed for non-throttle errors"
    );
}

/// A throttle with no parseable deadline still has to break the loop.
#[test]
fn test_throttle_without_deadline_uses_default_pause() {
    let _guard = THROTTLE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_throttle();

    let err = classify_error(
        reqwest::StatusCode::BAD_REQUEST,
        r#"{"error_name":"throttle_violation","error_message":"too many requests"}"#,
    );
    assert!(matches!(err, SourceError::RateLimited { .. }));

    let remaining = throttle_remaining().expect("breaker must be armed");
    assert!(
        remaining > DEFAULT_THROTTLE_SECS - 60 && remaining <= DEFAULT_THROTTLE_SECS,
        "expected the default pause, got {remaining}"
    );

    reset_throttle();
}

/// An absurd upstream value must be clamped, never allowed to disable the
/// source forever; and a shorter reading must not shorten a longer pause.
#[test]
fn test_throttle_is_clamped_and_only_extends() {
    let _guard = THROTTLE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_throttle();

    assert_eq!(arm_throttle(u64::MAX), MAX_THROTTLE_SECS);
    let long = throttle_remaining().expect("armed");

    // A shorter subsequent throttle must not pull the deadline in.
    arm_throttle(5);
    let after = throttle_remaining().expect("still armed");
    assert!(
        after >= long - 5,
        "a shorter reading must not shorten an active pause: {long} -> {after}"
    );

    reset_throttle();
    assert!(throttle_remaining().is_none());
}

/// `backoff` is now captured off the success path; it previously had no
/// binding at all, so Stack Exchange's own slow-down request was discarded.
#[test]
fn test_success_response_captures_backoff() {
    let json = r#"{"items":[],"has_more":false,"quota_remaining":42,"backoff":10}"#;
    let resp = parse_response(json).unwrap();
    assert_eq!(resp.backoff, Some(10));
    assert_eq!(resp.quota_remaining, Some(42));
}

/// Live response 2026-10-08 20:43Z: `quota_remaining` of `-1` made the old
/// `Option<u32>` field reject the whole body ("invalid value: integer `-1`,
/// expected u32"), discarding the questions AND the backoff with it.
#[test]
fn test_overdrawn_quota_parses_and_keeps_rate_signals() {
    let json = r#"{"items":[{"question_id":1,"title":"Borrow checker and async closures","link":"https://stackoverflow.com/q/1","score":2}],"has_more":true,"quota_max":300,"quota_remaining":-1,"backoff":12}"#;
    let resp = parse_response(json).expect("a -1 quota is not a parse error");
    assert_eq!(resp.quota_remaining, Some(-1));
    assert_eq!(resp.backoff, Some(12));
    assert_eq!(resp.questions.len(), 1);
}

/// One malformed question must not take the rate signals (or the other
/// questions) down with it.
#[test]
fn test_malformed_question_is_skipped_not_fatal() {
    let json = r#"{"items":[{"question_id":"oops"},{"question_id":2,"title":"Tokio select fairness","link":"https://stackoverflow.com/q/2","score":0}],"quota_remaining":5,"backoff":3.5}"#;
    let resp = parse_response(json).unwrap();
    assert_eq!(resp.questions.len(), 1);
    assert_eq!(resp.questions[0].question_id, 2);
    assert_eq!(resp.quota_remaining, Some(5));
    assert_eq!(resp.backoff, Some(4), "a fractional backoff rounds up");
    assert!(parse_response("not json").is_err());
}

/// Below the floor (and when overdrawn) the source pauses until the quota
/// resets at UTC midnight, instead of only ending the current cycle.
#[test]
fn test_low_or_overdrawn_quota_pauses_until_reset() {
    let now = 1_791_000_000 - (1_791_000_000 % 86_400) + 20 * 3_600; // 20:00 UTC
    let until_midnight = 4 * 3_600;
    assert_eq!(quota_pause_secs(Some(-1), now), Some(until_midnight));
    assert_eq!(quota_pause_secs(Some(0), now), Some(until_midnight));
    assert_eq!(
        quota_pause_secs(Some(i64::from(MIN_QUOTA) - 1), now),
        Some(until_midnight)
    );
    assert_eq!(quota_pause_secs(Some(i64::from(MIN_QUOTA)), now), None);
    assert_eq!(quota_pause_secs(Some(250), now), None);
    assert_eq!(quota_pause_secs(None, now), None, "no reading, no pause");
    assert_eq!(secs_until_utc_midnight(now - 20 * 3_600), 86_400);
}
