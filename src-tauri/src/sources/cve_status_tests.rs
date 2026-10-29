// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! A failed GitHub Advisory API response is a failure the source's health can
//! see, never an empty window.

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;

use super::*;

const NOW: i64 = 1_800_000_000;

fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(
            *name,
            HeaderValue::from_str(value).unwrap_or(HeaderValue::from_static("")),
        );
    }
    map
}

#[test]
fn exhausted_hourly_budget_on_403_is_a_rate_limit_until_the_reset() {
    let reset = (NOW + 1_200).to_string();
    let err = advisory_status_error(
        StatusCode::FORBIDDEN,
        &headers(&[
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", &reset),
        ]),
        NOW,
    );
    assert!(matches!(err, SourceError::RateLimited { .. }), "{err}");
    assert_eq!(err.retry_after_secs(), Some(1_200));
}

#[test]
fn a_reset_already_passed_waits_zero_not_a_negative() {
    let reset = (NOW - 5).to_string();
    let err = advisory_status_error(
        StatusCode::TOO_MANY_REQUESTS,
        &headers(&[
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", &reset),
        ]),
        NOW,
    );
    assert_eq!(err.retry_after_secs(), Some(0));
}

#[test]
fn secondary_limit_403_with_retry_after_is_a_rate_limit() {
    let err = advisory_status_error(
        StatusCode::FORBIDDEN,
        &headers(&[("retry-after", "60")]),
        NOW,
    );
    assert!(matches!(err, SourceError::RateLimited { .. }), "{err}");
    assert_eq!(err.retry_after_secs(), Some(60));
}

#[test]
fn plain_403_stays_forbidden_and_5xx_is_a_network_error() {
    let forbidden = advisory_status_error(StatusCode::FORBIDDEN, &HeaderMap::new(), NOW);
    assert!(
        matches!(forbidden, SourceError::Forbidden(_)),
        "{forbidden}"
    );
    let outage = advisory_status_error(StatusCode::BAD_GATEWAY, &HeaderMap::new(), NOW);
    assert!(matches!(outage, SourceError::Network(_)), "{outage}");
}

#[test]
fn a_rate_limit_survives_the_anyhow_round_trip() {
    let err = advisory_status_error(
        StatusCode::FORBIDDEN,
        &headers(&[("x-ratelimit-remaining", "0")]),
        NOW,
    );
    let back = as_source_error(anyhow::Error::from(err));
    assert!(matches!(back, SourceError::RateLimited { .. }), "{back}");
    let other = as_source_error(anyhow::anyhow!("connection reset"));
    assert!(matches!(other, SourceError::Network(_)), "{other}");
}

#[test]
fn every_ecosystem_failing_fails_the_source() {
    let err = ecosystems_outcome::<u8>(0, Some(SourceError::Forbidden("x".into())), vec![]);
    assert!(
        err.is_err(),
        "an all-failed cycle must not read as a quiet day"
    );
}

#[test]
fn one_ecosystem_answering_is_a_partial_success() {
    let partial = ecosystems_outcome(1, Some(SourceError::Forbidden("x".into())), vec![7u8]);
    assert_eq!(partial.ok(), Some(vec![7u8]));
    let quiet = ecosystems_outcome::<u8>(2, None, vec![]);
    assert_eq!(
        quiet.ok(),
        Some(vec![]),
        "a genuinely quiet window is still Ok(empty)"
    );
}
