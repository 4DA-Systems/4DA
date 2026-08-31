// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The deferral queue: what happens to a surface the gate held back.
//!
//! The rule is **held, never dropped**. A briefing suppressed because the user
//! was in a game is not cancelled — it waits, and is delivered the moment they
//! are back. Without this the gate would be a regression, because
//! `check_morning_briefing` marks the brief as fired for the day *before*
//! delivery: dropping it would cost the user that day's intelligence outright.
//!
//! ## Coalescing
//!
//! Coming back from a two-hour session must not trigger a stampede of toasts
//! fighting each other for the same screen corner. The queue therefore holds:
//!
//! - **at most one briefing** — a newer brief supersedes an older one, since
//!   they are cumulative snapshots rather than discrete events;
//! - **many toasts, delivered as one card** — the highest-priority item's
//!   title, badged with the total count.
//!
//! One return, one interruption.

use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{AppHandle, Runtime};
use tracing::info;

use super::BusyReason;
use crate::monitoring_briefing::BriefingNotification;
use crate::notification_window::{Dispatch, NotificationData};

/// How many held toasts we retain in full. Beyond this only the count grows —
/// we render a single coalesced card regardless, so retaining more would buy
/// nothing but memory.
const MAX_RETAINED_TOASTS: usize = 20;

/// How long a held surface stays worth raising.
///
/// "Held, never dropped" is about the *intelligence*, not the *interruption*.
/// The briefing content is already durable — `briefing_snapshot` persists it and
/// the Brief tab renders it — so nothing is lost when this expires. What expires
/// is 4DA's licence to interrupt you about it.
///
/// Popping up "your morning brief" at 18:00, ten hours stale and with ten hours
/// of newer signal already gathered, is worse than not popping up at all: it is
/// an interruption that presents old intelligence as current. The next scheduled
/// brief is a better answer than a resurrected one.
const MAX_HELD_AGE: Duration = Duration::from_hours(6);

/// What the gate is currently holding.
#[derive(Default)]
struct Held {
    /// The most recent deferred briefing, if any.
    briefing: Option<Box<BriefingNotification>>,
    /// Deferred toasts, newest last, capped at [`MAX_RETAINED_TOASTS`].
    toasts: Vec<Dispatch>,
    /// Toasts dropped from `toasts` because the cap was reached. They still
    /// count towards the coalesced badge.
    overflow: usize,
    /// Why the first held surface was held — used for the "held while you
    /// were in a game" line on delivery.
    reason: Option<BusyReason>,
    /// When the hold began. `Instant` rather than a wall clock so a timezone
    /// change or an NTP correction cannot make a fresh hold look ancient.
    held_since: Option<Instant>,
}

impl Held {
    fn total_toasts(&self) -> usize {
        self.toasts.len() + self.overflow
    }

    fn is_empty(&self) -> bool {
        self.briefing.is_none() && self.total_toasts() == 0
    }
}

static HELD: Mutex<Option<Held>> = Mutex::new(None);

fn with_held<T>(f: impl FnOnce(&mut Held) -> T) -> T {
    let mut guard = HELD.lock();
    f(guard.get_or_insert_with(Held::default))
}

// ============================================================================
// Holding
// ============================================================================

/// Defer a briefing. A newer briefing replaces an older held one.
pub fn hold_briefing(briefing: &BriefingNotification, reason: BusyReason) {
    with_held(|held| {
        let superseded = held.briefing.is_some();
        held.briefing = Some(Box::new(briefing.clone()));
        held.reason.get_or_insert(reason);
        held.held_since.get_or_insert_with(Instant::now);
        info!(
            target: "4da::presence",
            reason = reason.as_str(),
            superseded,
            items = briefing.total_relevant,
            "Briefing held — user is busy"
        );
    });
}

/// Defer a toast.
pub fn hold_toast(dispatch: &Dispatch, reason: BusyReason) {
    with_held(|held| {
        if held.toasts.len() < MAX_RETAINED_TOASTS {
            held.toasts.push(dispatch.clone());
        } else {
            held.overflow += 1;
        }
        held.reason.get_or_insert(reason);
        held.held_since.get_or_insert_with(Instant::now);
        info!(
            target: "4da::presence",
            reason = reason.as_str(),
            held = held.total_toasts(),
            "Notification held — user is busy"
        );
    });
}

// ============================================================================
// Inspection
// ============================================================================

/// How many surfaces are waiting (a held briefing counts as one).
pub fn held_count() -> usize {
    let guard = HELD.lock();
    guard.as_ref().map_or(0, |held| {
        held.total_toasts() + usize::from(held.briefing.is_some())
    })
}

/// Why the queue is holding, if it is.
pub fn held_reason() -> Option<BusyReason> {
    let guard = HELD.lock();
    guard.as_ref().and_then(|held| held.reason)
}

/// Discard everything held, without delivering it. Used when the user opens
/// the app themselves — they are now looking at the Brief tab, so replaying a
/// briefing popup at them would be redundant noise.
pub fn clear() {
    let mut guard = HELD.lock();
    if let Some(held) = guard.as_ref() {
        if !held.is_empty() {
            info!(
                target: "4da::presence",
                count = held.total_toasts() + usize::from(held.briefing.is_some()),
                "Held surfaces discarded — user opened 4DA directly"
            );
        }
    }
    *guard = None;
}

// ============================================================================
// Flushing
// ============================================================================

/// Deliver everything held, coalesced. Called by the resume watcher once the
/// user has been available for the settle period.
///
/// Delivery uses the `_now` entry points, which bypass the gate — re-consulting
/// it here would re-hold the very items we just decided to release.
pub fn flush<R: Runtime>(app: &AppHandle<R>) {
    let Some(held) = HELD.lock().take() else {
        return;
    };
    if held.is_empty() {
        return;
    }

    let reason = held.reason;
    let toast_total = held.total_toasts();
    let age = held.held_since.map_or(Duration::ZERO, |t| t.elapsed());

    // Too old to be worth raising. The content is not lost — the snapshot and
    // the Brief tab still have it — but interrupting the user with stale
    // intelligence would break the one promise the gate exists to keep.
    if is_stale(age) {
        info!(
            target: "4da::presence",
            held_hours = age.as_secs() / 3600,
            briefing = held.briefing.is_some(),
            toasts = toast_total,
            "Held surfaces expired — content remains in the Brief tab, not raised as a popup"
        );
        return;
    }

    if let Some(briefing) = held.briefing {
        info!(
            target: "4da::presence",
            reason = reason.map(BusyReason::as_str),
            "Delivering held briefing"
        );
        crate::briefing_window::show_briefing_now(app, &briefing);
    }

    match coalesce(held.toasts, held.overflow, reason) {
        None => {}
        Some(dispatch) => {
            info!(
                target: "4da::presence",
                count = toast_total,
                "Delivering held notifications as one card"
            );
            crate::notification_window::dispatch_now(app, &dispatch);
        }
    }
}

/// Has a hold outlived its licence to interrupt?
///
/// Pure so the boundary is testable without waiting six hours.
fn is_stale(age: Duration) -> bool {
    age > MAX_HELD_AGE
}

/// Collapse the held toasts into at most one card.
///
/// Pure, so the coalescing rules are testable without a running app.
fn coalesce(
    mut toasts: Vec<Dispatch>,
    overflow: usize,
    reason: Option<BusyReason>,
) -> Option<Dispatch> {
    let total = toasts.len() + overflow;
    if total == 0 {
        return None;
    }

    // Pick the lead card: the most urgent toast we actually retained. If
    // nothing was retained there is no title to lead with, so there is nothing
    // to render — `?` exits rather than indexing into an empty vec.
    let lead_index = toasts
        .iter()
        .enumerate()
        .max_by_key(|(_, d)| priority_rank(&d.data.priority))
        .map(|(index, _)| index)?;
    let mut lead = toasts.swap_remove(lead_index);

    // Single held toast: deliver it as-is, only annotating why it is late.
    if total == 1 {
        if let Some(reason) = reason {
            lead.data.action = Some(held_note(reason));
        }
        return Some(lead);
    }

    // Several: one card, led by the most urgent, badged with the total.
    let priority = lead.data.priority.clone();
    let title = lead.data.title.clone();
    let native_title = format!("{total} signals while you were away");

    Some(Dispatch {
        data: NotificationData {
            variant: "multi".to_string(),
            priority,
            count: Some(total),
            title,
            action: Some(reason.map_or_else(
                || "Held while you were away".to_string(),
                |r| format!("Held {}", r.user_text()),
            )),
            ..lead.data
        },
        native_title,
    })
}

/// The "why this is late" line shown on a single deferred toast.
fn held_note(reason: BusyReason) -> String {
    format!("Held {}", reason.user_text())
}

/// Ordering for picking the most urgent held toast. Higher wins.
fn priority_rank(priority: &str) -> u8 {
    match priority {
        "critical" => 4,
        "alert" => 3,
        "advisory" => 2,
        _ => 1, // "watch" and anything unrecognised
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toast(priority: &str, title: &str) -> Dispatch {
        Dispatch {
            data: NotificationData {
                variant: "signal".to_string(),
                priority: priority.to_string(),
                signal_type: None,
                title: title.to_string(),
                action: None,
                source: None,
                matched_deps: vec![],
                count: None,
                chain_sources: None,
                chain_phase: None,
                chain_links_filled: None,
                chain_links_total: None,
                time_ago: "just now".to_string(),
                item_id: None,
            },
            native_title: title.to_string(),
        }
    }

    #[test]
    fn nothing_held_coalesces_to_nothing() {
        assert!(coalesce(vec![], 0, None).is_none());
    }

    #[test]
    fn single_toast_is_delivered_unchanged_but_annotated() {
        let result = coalesce(
            vec![toast("critical", "CVE-2026-1234 in sqlite")],
            0,
            Some(BusyReason::FullscreenApp),
        )
        .expect("one toast in, one out");

        assert_eq!(result.data.variant, "signal", "not upgraded to multi");
        assert_eq!(result.data.title, "CVE-2026-1234 in sqlite");
        assert_eq!(result.data.count, None);
        assert_eq!(
            result.data.action.as_deref(),
            Some("Held while you were in a fullscreen app")
        );
    }

    #[test]
    fn many_toasts_become_one_card_led_by_the_most_urgent() {
        let result = coalesce(
            vec![
                toast("watch", "a blog post"),
                toast("critical", "CVE in your stack"),
                toast("alert", "breaking change"),
            ],
            0,
            Some(BusyReason::FullscreenWindow),
        )
        .expect("three toasts coalesce");

        assert_eq!(result.data.variant, "multi");
        assert_eq!(result.data.priority, "critical", "most urgent leads");
        assert_eq!(result.data.title, "CVE in your stack");
        assert_eq!(result.data.count, Some(3), "badge shows the total");
    }

    #[test]
    fn overflow_is_counted_even_though_it_is_not_retained() {
        let result =
            coalesce(vec![toast("watch", "one")], 41, None).expect("retained + overflow coalesce");
        assert_eq!(result.data.count, Some(42));
        assert_eq!(result.data.variant, "multi");
    }

    #[test]
    fn overflow_without_any_retained_toast_is_handled_not_panicked() {
        // Cannot arise today (overflow only grows once retention is full), but
        // the pure function must not index into an empty vec if it ever does.
        assert!(
            coalesce(vec![], 7, None).is_none(),
            "no retained toast means no title to lead with"
        );
        assert!(
            coalesce(vec![], 1, Some(BusyReason::ScreenLocked)).is_none(),
            "total==1 from overflow alone must not panic"
        );
    }

    #[test]
    fn coalesced_card_explains_the_delay() {
        let result = coalesce(
            vec![toast("watch", "one"), toast("watch", "two")],
            0,
            Some(BusyReason::Presentation),
        )
        .expect("two toasts coalesce");
        assert_eq!(
            result.data.action.as_deref(),
            Some("Held while you were presenting")
        );
    }

    #[test]
    fn coalesced_card_without_reason_still_reads_sensibly() {
        let result = coalesce(vec![toast("watch", "one"), toast("watch", "two")], 0, None)
            .expect("two toasts coalesce");
        assert_eq!(
            result.data.action.as_deref(),
            Some("Held while you were away")
        );
    }

    #[test]
    fn priority_ranking_orders_correctly() {
        assert!(priority_rank("critical") > priority_rank("alert"));
        assert!(priority_rank("alert") > priority_rank("advisory"));
        assert!(priority_rank("advisory") > priority_rank("watch"));
        assert_eq!(priority_rank("watch"), priority_rank("nonsense"));
    }

    #[test]
    fn held_note_reads_as_a_sentence_fragment() {
        assert_eq!(held_note(BusyReason::Away), "Held while you were away");
        assert_eq!(
            held_note(BusyReason::ScreenLocked),
            "Held while your screen was locked"
        );
        assert_eq!(
            held_note(BusyReason::QuietHours),
            "Held during your quiet hours"
        );
    }

    // -- staleness ---------------------------------------------------------

    #[test]
    fn a_fresh_hold_is_not_stale() {
        assert!(!is_stale(Duration::ZERO));
        assert!(!is_stale(Duration::from_secs(60)));
    }

    #[test]
    fn a_hold_just_inside_the_window_still_delivers() {
        assert!(!is_stale(MAX_HELD_AGE - Duration::from_secs(1)));
        assert!(
            !is_stale(MAX_HELD_AGE),
            "the boundary itself still delivers"
        );
    }

    #[test]
    fn a_hold_past_the_window_is_stale() {
        assert!(is_stale(MAX_HELD_AGE + Duration::from_secs(1)));
        // The motivating case: an 08:00 brief flushed at 18:00.
        assert!(is_stale(Duration::from_secs(10 * 60 * 60)));
    }

    #[test]
    fn the_stale_window_is_long_enough_for_a_normal_gaming_session() {
        // A hold must survive an ordinary evening of play, or the gate would
        // routinely eat briefs it was supposed to be protecting.
        assert!(
            !is_stale(Duration::from_secs(4 * 60 * 60)),
            "a four-hour session must still deliver"
        );
    }

    #[test]
    fn holding_records_when_the_hold_began() {
        let _serial = QUEUE_TEST_LOCK.lock();
        clear();
        hold_toast(&toast("watch", "one"), BusyReason::Away);
        {
            let guard = HELD.lock();
            let held = guard.as_ref().expect("something is held");
            assert!(held.held_since.is_some(), "hold start time is recorded");
        }
        clear();
    }

    #[test]
    fn the_hold_start_is_not_pushed_forward_by_later_holds() {
        let _serial = QUEUE_TEST_LOCK.lock();
        clear();
        hold_toast(&toast("watch", "first"), BusyReason::Away);
        let first = HELD.lock().as_ref().and_then(|h| h.held_since);
        hold_toast(&toast("watch", "second"), BusyReason::Away);
        let second = HELD.lock().as_ref().and_then(|h| h.held_since);
        assert_eq!(
            first, second,
            "age is measured from the FIRST hold, or a trickle of new items \
             would keep resetting the clock and never expire"
        );
        clear();
    }

    // -- queue state -------------------------------------------------------
    //
    // These mutate the process-global HELD queue, so they are serialised
    // against each other the same way notification_window serialises its
    // dismiss-timer tests.
    static QUEUE_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn holding_a_toast_makes_it_countable() {
        let _serial = QUEUE_TEST_LOCK.lock();
        clear();
        assert_eq!(held_count(), 0);

        hold_toast(&toast("alert", "one"), BusyReason::FullscreenApp);
        assert_eq!(held_count(), 1);
        assert_eq!(held_reason(), Some(BusyReason::FullscreenApp));

        clear();
        assert_eq!(held_count(), 0);
        assert_eq!(held_reason(), None);
    }

    #[test]
    fn the_first_reason_is_the_one_reported() {
        let _serial = QUEUE_TEST_LOCK.lock();
        clear();
        hold_toast(&toast("watch", "one"), BusyReason::FullscreenApp);
        hold_toast(&toast("watch", "two"), BusyReason::QuietHours);
        assert_eq!(
            held_reason(),
            Some(BusyReason::FullscreenApp),
            "the reason the hold STARTED is the honest one to show"
        );
        clear();
    }

    #[test]
    fn retention_is_capped_but_the_count_is_not() {
        let _serial = QUEUE_TEST_LOCK.lock();
        clear();
        for i in 0..(MAX_RETAINED_TOASTS + 5) {
            hold_toast(
                &toast("watch", &format!("item {i}")),
                BusyReason::ScreenLocked,
            );
        }
        assert_eq!(
            held_count(),
            MAX_RETAINED_TOASTS + 5,
            "every held toast is still counted"
        );
        clear();
    }
}
