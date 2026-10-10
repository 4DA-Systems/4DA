// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! "0 silent drops" (AD-054): the legacy feed every consumer reads — the
//! Preemption worklist (through the evidence feed), the brief's security
//! facts and Signal Lane 1 — keeps every alert, and its counts are the true
//! totals. It used to `truncate(30)` after sorting and count the survivors.

use super::*;

fn alert(id: &str, urgency: AlertUrgency, confidence: f32) -> PreemptionAlert {
    PreemptionAlert {
        id: id.to_string(),
        alert_type: PreemptionType::SecurityAdvisory,
        title: format!("advisory {id}"),
        explanation: "Version-confirmed advisory.".to_string(),
        evidence: vec![],
        affected_projects: vec!["/repos/app".to_string()],
        affected_dependencies: vec![format!("pkg-{id}")],
        urgency,
        confidence,
        predicted_window: None,
        suggested_actions: vec![],
        created_at: "2026-10-10 00:00:00".to_string(),
        osv_verified: true,
        source_classified: false,
        installed_version: Some("1.0.0".to_string()),
        fixed_version: Some("1.0.1".to_string()),
        is_direct: Some(true),
        is_dev: Some(false),
        platform_inactive: false,
        lockfile_only: false,
    }
}

/// The fixture-corpus shape that exposed the cap: more Critical advisories
/// than the old cap, so every High/Medium/Watch one sat past position 30.
/// Input is deliberately interleaved — the ranking must do the ordering.
fn corpus() -> Vec<PreemptionAlert> {
    let mut alerts = Vec::new();
    for i in 0..12 {
        alerts.push(alert(&format!("high-{i}"), AlertUrgency::High, 0.9));
    }
    for i in 0..31 {
        alerts.push(alert(&format!("crit-{i}"), AlertUrgency::Critical, 0.95));
    }
    for i in 0..3 {
        alerts.push(alert(&format!("watch-{i}"), AlertUrgency::Watch, 0.6));
    }
    for i in 0..7 {
        alerts.push(alert(&format!("med-{i}"), AlertUrgency::Medium, 0.8));
    }
    alerts
}

#[test]
fn rank_feed_keeps_every_alert_past_the_old_cap_of_thirty() {
    let input = corpus();
    let ids: std::collections::HashSet<String> = input.iter().map(|a| a.id.clone()).collect();
    assert_eq!(ids.len(), 53);

    let feed = rank_feed(input);

    assert_eq!(feed.alerts.len(), 53, "no alert may be dropped");
    assert_eq!(feed.total, 53, "total counts every alert, not a cut list");
    let out: std::collections::HashSet<String> = feed.alerts.iter().map(|a| a.id.clone()).collect();
    assert_eq!(out, ids, "the same alerts come out as went in");
}

#[test]
fn rank_feed_counts_are_true_totals() {
    let feed = rank_feed(corpus());
    // The old code counted after truncating to 30: critical 30, high 0.
    assert_eq!(feed.critical_count, 31);
    assert_eq!(feed.high_count, 12);
}

#[test]
fn rank_feed_orders_by_urgency_then_confidence() {
    let mut input = corpus();
    input.push(alert("crit-low-conf", AlertUrgency::Critical, 0.5));
    let feed = rank_feed(input);
    let ranks: Vec<u8> = feed
        .alerts
        .iter()
        .map(|a| urgency_rank(&a.urgency))
        .collect();
    assert!(ranks.windows(2).all(|w| w[0] <= w[1]), "urgency order");
    // The lowest-confidence Critical sorts last among the Criticals.
    assert_eq!(feed.alerts[31].id, "crit-low-conf");
    // Every High, Medium and Watch advisory is reachable past position 30 —
    // exactly the rows the cap used to cut.
    assert!(feed.alerts[32..]
        .iter()
        .any(|a| matches!(a.urgency, AlertUrgency::High)));
    assert!(feed.alerts[32..]
        .iter()
        .any(|a| matches!(a.urgency, AlertUrgency::Medium)));
    assert!(feed.alerts[32..]
        .iter()
        .any(|a| matches!(a.urgency, AlertUrgency::Watch)));
}

/// The worklist's LIST transport must not re-introduce a cap on findings:
/// only Upgrade Plan steps are held back (and counted in `total`).
#[test]
fn list_transport_ships_every_finding_with_true_counts() {
    let feed = rank_feed(corpus());
    let items: Vec<EvidenceItem> = feed
        .alerts
        .iter()
        .map(PreemptionAlert::to_evidence_item)
        .collect();
    let list =
        crate::evidence::present_preemption_list(EvidenceFeed::from_items(items), &[], false);
    assert_eq!(list.items.len(), 53);
    assert_eq!(list.total, 53);
    assert_eq!(list.critical_count, 31);
    assert_eq!(list.high_count, 12);
}

/// Before/after on a SNAPSHOT of a real database (never the live file —
/// the open migrates it):
///
/// ```text
/// FOURDA_DB_PATH=<copy>/4da.db cargo test --lib \
///     live_preemption_feed_drops_nothing -- --ignored --nocapture
/// ```
///
/// BEFORE is reconstructed exactly: the old code truncated this same ranked
/// vec to its first 30 and counted those.
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_preemption_feed_drops_nothing() {
    if std::env::var("FOURDA_DB_PATH").is_err() {
        return;
    }
    let feed = get_preemption_feed().expect("feed on snapshot");
    let before: Vec<&PreemptionAlert> = feed.alerts.iter().take(30).collect();
    let count = |set: &[&PreemptionAlert], u: AlertUrgency| {
        set.iter()
            .filter(|a| std::mem::discriminant(&a.urgency) == std::mem::discriminant(&u))
            .count()
    };
    let all: Vec<&PreemptionAlert> = feed.alerts.iter().collect();
    let brief_input = |set: &[&PreemptionAlert]| {
        set.iter()
            .filter(|a| a.osv_verified && !a.platform_inactive)
            .count()
    };
    let non_osv = |set: &[&PreemptionAlert]| set.iter().filter(|a| !a.osv_verified).count();
    for (label, set) in [("BEFORE (cap 30)", &before), ("AFTER (uncapped)", &all)] {
        println!(
            "{label}: alerts={} critical={} high={} medium={} watch={} brief_input(osv,active)={} non_osv(deliberated)={}",
            set.len(),
            count(set, AlertUrgency::Critical),
            count(set, AlertUrgency::High),
            count(set, AlertUrgency::Medium),
            count(set, AlertUrgency::Watch),
            brief_input(set),
            non_osv(set),
        );
    }
    assert_eq!(feed.total, feed.alerts.len());

    // The worklist payload the command would ship for the fast full feed.
    let full = compute_preemption_fast_full_feed().expect("fast full feed");
    let list = crate::evidence::present_preemption_list(full, &[], false);
    let bytes = serde_json::to_vec(&list).map(|v| v.len()).unwrap_or(0);
    let plan = list
        .items
        .iter()
        .filter(|i| i.lens_hints.upgrade_plan)
        .count();
    println!(
        "worklist list payload: items={} (plan {plan}) total={} critical={} high={} bytes={bytes}",
        list.items.len(),
        list.total,
        list.critical_count,
        list.high_count,
    );

    let db = crate::get_database().expect("db");
    let facts = crate::brief_facts::build_brief_facts(db);
    println!(
        "brief facts: security(act)={} also_open={}",
        facts.security.len(),
        facts.also_open.len()
    );
}
