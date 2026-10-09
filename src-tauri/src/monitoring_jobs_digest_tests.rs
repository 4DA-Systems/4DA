// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! When the digest tick sends (fresh-profile E2E 2026-10-09: a digest
//! notification fired during onboarding).

use super::{digest_decision, DigestDecision};

fn at(hours_ago: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    Some(chrono::Utc::now() - chrono::Duration::hours(hours_ago))
}

#[test]
fn nothing_is_sent_before_onboarding_is_finished() {
    let now = chrono::Utc::now();
    assert_eq!(
        digest_decision(true, false, "daily", None, now),
        DigestDecision::Skip
    );
    assert_eq!(
        digest_decision(true, false, "daily", at(48), now),
        DigestDecision::Skip,
        "even an overdue digest waits for setup to finish"
    );
}

#[test]
fn a_profile_with_no_history_starts_the_clock_instead_of_sending() {
    let now = chrono::Utc::now();
    assert_eq!(
        digest_decision(true, true, "daily", None, now),
        DigestDecision::StartClock
    );
    assert_eq!(
        digest_decision(true, true, "weekly", None, now),
        DigestDecision::StartClock
    );
}

#[test]
fn an_established_profile_keeps_its_cadence() {
    let now = chrono::Utc::now();
    assert_eq!(
        digest_decision(true, true, "daily", at(25), now),
        DigestDecision::Generate
    );
    assert_eq!(
        digest_decision(true, true, "daily", at(3), now),
        DigestDecision::Skip
    );
    assert_eq!(
        digest_decision(true, true, "weekly", at(24 * 8), now),
        DigestDecision::Generate
    );
    assert_eq!(
        digest_decision(true, true, "realtime", at(100), now),
        DigestDecision::Skip
    );
    assert_eq!(
        digest_decision(false, true, "daily", at(100), now),
        DigestDecision::Skip,
        "disabled"
    );
}
