// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the morning floor: abstain / fail / timeout / no model all
//! deliver the facts view, a morning with nothing records why, and the floor
//! holds the caps (no `briefings` row, at most FLOOR_ARTICLES articles).

use std::time::Duration;

use super::*;
use crate::brief_facts::{FactStatus, UpgradeFact, UpgradeSite, WorthKnowingCandidate};

fn briefing(items: usize) -> BriefingNotification {
    let items: Vec<serde_json::Value> = (0..items)
        .map(|i| {
            serde_json::json!({
                "title": format!("Item {i}"),
                "source_type": "hackernews",
                "score": 0.7,
                "signal_type": null,
            })
        })
        .collect();
    serde_json::from_value(serde_json::json!({
        "title": "4DA Intelligence Briefing — 10 Oct 2026",
        "items": items,
        "total_relevant": items.len(),
    }))
    .expect("briefing literal")
}

fn upgrade(pkg: &str) -> UpgradeFact {
    UpgradeFact {
        key: format!("npm:{pkg}"),
        package: pkg.into(),
        ecosystem: "npm".into(),
        announced: "7.0.130".into(),
        published: Some("2026-10-06".into()),
        yanked: false,
        dev_only: false,
        majors_behind: 2,
        pre_one: false,
        sites: vec![UpgradeSite {
            label: "navcal".into(),
            installed: "5.0.86".into(),
        }],
        item_id: 1,
        url: None,
        status: FactStatus::New,
    }
}

fn facts_with_upgrade() -> BriefFacts {
    BriefFacts {
        upgrades: vec![upgrade("ai")],
        ..BriefFacts::default()
    }
}

fn synthesis(prose: &str) -> SynthesisResult {
    SynthesisResult {
        prose: prose.into(),
        clusters: None,
        provider_used: "anthropic/claude-sonnet-5".into(),
        synthesis_tier: "cloud".into(),
    }
}

/// A floor step that always has facts.
fn with_facts(why: MorningWhy) -> std::future::Ready<Result<(Option<FactsFloor>, bool), String>> {
    std::future::ready(Ok((floor_from_facts(facts_with_upgrade(), why), false)))
}

fn no_facts(_: MorningWhy) -> std::future::Ready<Result<(Option<FactsFloor>, bool), String>> {
    std::future::ready(Ok((None, false)))
}

#[tokio::test]
async fn an_abstention_delivers_the_facts_view() {
    let mut b = briefing(1);
    let synth = async {
        Ok(synthesis(
            "Low signal -- no noteworthy intelligence overnight.",
        ))
    };
    let r = resolve_morning(&mut b, synth, Duration::from_secs(5), with_facts).await;
    assert_eq!(r.outcome, MorningOutcome::Facts(MorningWhy::Abstained));
    assert!(r.outcome.delivered());
    let facts = b.facts_brief.as_deref().expect("facts attached");
    assert!(facts.contains("## Upgrades to plan"), "{facts}");
    assert!(facts.contains("Facts view"), "labelled as facts: {facts}");
    assert!(
        facts.contains("found nothing it could stand behind"),
        "{facts}"
    );
    assert!(
        b.synthesis.is_none(),
        "no 'low signal' line beside the facts"
    );
    assert!(should_deliver(&b));
    assert!(r.floor.is_some(), "handed back to be recorded as shown");
}

/// Live 2026-10-04/05: "Low signal overnight -- nothing here rises above
/// routine ecosystem chatter." passed the canonical detector as a summary,
/// and the window folded it. It is an abstention, so the facts go out.
#[tokio::test]
async fn a_summary_the_window_folds_is_an_abstention() {
    let mut b = briefing(4);
    let synth = async {
        Ok(synthesis(
            "Low signal overnight -- nothing here rises above routine ecosystem chatter.",
        ))
    };
    let r = resolve_morning(&mut b, synth, Duration::from_secs(5), with_facts).await;
    assert_eq!(r.outcome, MorningOutcome::Facts(MorningWhy::Abstained));
    assert!(b.facts_brief.is_some());
    assert!(!window_folds(
        "Tokio has a confirmed RCE -- patch it today."
    ));
}

#[tokio::test]
async fn a_timeout_delivers_the_facts_view() {
    let mut b = briefing(2);
    let never = std::future::pending::<Result<SynthesisResult, String>>();
    let r = resolve_morning(&mut b, never, Duration::from_millis(20), with_facts).await;
    assert_eq!(r.outcome, MorningOutcome::Facts(MorningWhy::TimedOut));
    let facts = b.facts_brief.as_deref().expect("facts attached");
    assert!(facts.contains("did not finish in time"), "{facts}");
}

#[tokio::test]
async fn a_failure_or_no_model_delivers_the_facts_view() {
    let mut b = briefing(1);
    let failed =
        async { Err::<SynthesisResult, _>("LLM synthesis failed: 529 overloaded".to_string()) };
    let r = resolve_morning(&mut b, failed, Duration::from_secs(5), with_facts).await;
    assert_eq!(r.outcome, MorningOutcome::Facts(MorningWhy::Failed));
    assert_eq!(
        r.error.as_deref(),
        Some("LLM synthesis failed: 529 overloaded")
    );

    let mut b = briefing(1);
    let none = async {
        Err::<SynthesisResult, _>(
            "No synthesis-capable provider — configure a cloud AI provider".to_string(),
        )
    };
    let r = resolve_morning(&mut b, none, Duration::from_secs(5), with_facts).await;
    assert_eq!(r.outcome, MorningOutcome::Facts(MorningWhy::Unconfigured));
    assert!(b
        .facts_brief
        .as_deref()
        .is_some_and(|f| f.contains("Sonnet-class model")));
}

#[tokio::test]
async fn a_written_summary_is_shown_and_no_facts_are_built() {
    let mut b = briefing(2);
    let synth = async { Ok(synthesis("sharp in navcal needs the librsvg fix today.")) };
    let floor_called = std::cell::Cell::new(false);
    let floor = |_why| {
        floor_called.set(true);
        std::future::ready(Ok((None, false)))
    };
    let r = resolve_morning(&mut b, synth, Duration::from_secs(5), floor).await;
    assert_eq!(r.outcome, MorningOutcome::Narrated);
    assert!(!floor_called.get());
    assert!(b.facts_brief.is_none());
    assert_eq!(
        b.synthesis.as_deref(),
        Some("sharp in navcal needs the librsvg fix today.")
    );
}

#[tokio::test]
async fn no_facts_and_no_content_is_nothing_to_say_with_a_recorded_reason() {
    let mut b = briefing(0);
    let skipped = async {
        Err::<SynthesisResult, _>(
            "Brief carries no evidence items or alerts — nothing to synthesize".to_string(),
        )
    };
    let r = resolve_morning(&mut b, skipped, Duration::from_secs(5), no_facts).await;
    assert_eq!(
        r.outcome,
        MorningOutcome::NothingToSay {
            no_lockfiles: false
        }
    );
    assert!(!r.outcome.delivered());
    assert!(!should_deliver(&b));

    let db = crate::test_utils::test_db();
    record_outcome(&db, "2026-10-07", &r.outcome, "scheduled");
    let rec = load_outcomes(&db);
    let day = rec.get("2026-10-07").expect("recorded");
    assert_eq!(day.outcome, "nothing_to_say");
    assert_eq!(day.reason.as_deref(), Some("no_facts"));
    assert_eq!(day.path, "scheduled");
}

#[tokio::test]
async fn failed_and_nothing_to_say_are_distinguishable() {
    let mut b = briefing(0);
    let skipped = async { Err::<SynthesisResult, _>("nothing to synthesize".to_string()) };
    let floor = |_why| {
        std::future::ready(Err::<(Option<FactsFloor>, bool), _>(
            "database unavailable".into(),
        ))
    };
    let r = resolve_morning(&mut b, skipped, Duration::from_secs(5), floor).await;
    assert_eq!(
        r.outcome,
        MorningOutcome::Failed("database unavailable".into())
    );
    let db = crate::test_utils::test_db();
    record_outcome(&db, "2026-10-08", &r.outcome, "cold_boot");
    record_outcome(
        &db,
        "2026-10-09",
        &MorningOutcome::NothingToSay { no_lockfiles: true },
        "morning",
    );
    let rec = load_outcomes(&db);
    assert_eq!(rec["2026-10-08"].outcome, "failed");
    assert_eq!(rec["2026-10-09"].outcome, "nothing_to_say");
    assert_eq!(rec["2026-10-09"].reason.as_deref(), Some("no_lockfiles"));
}

#[test]
fn a_delivered_day_is_not_overwritten_by_a_later_quiet_check() {
    let db = crate::test_utils::test_db();
    record_outcome(
        &db,
        "2026-10-10",
        &MorningOutcome::Facts(MorningWhy::Abstained),
        "scheduled",
    );
    record_outcome(
        &db,
        "2026-10-10",
        &MorningOutcome::NothingToSay {
            no_lockfiles: false,
        },
        "morning",
    );
    assert_eq!(load_outcomes(&db)["2026-10-10"].outcome, "facts");
    // A quiet morning that later delivers does update.
    record_outcome(
        &db,
        "2026-10-11",
        &MorningOutcome::NothingToSay {
            no_lockfiles: false,
        },
        "morning",
    );
    record_outcome(&db, "2026-10-11", &MorningOutcome::Narrated, "scheduled");
    assert_eq!(load_outcomes(&db)["2026-10-11"].outcome, "narrated");
}

#[tokio::test]
async fn items_without_facts_still_go_out_with_the_quiet_line() {
    let mut b = briefing(3);
    let synth = async {
        Ok(synthesis(
            "Low signal -- no noteworthy intelligence overnight.",
        ))
    };
    let r = resolve_morning(&mut b, synth, Duration::from_secs(5), no_facts).await;
    assert_eq!(r.outcome, MorningOutcome::ItemsOnly(MorningWhy::Abstained));
    assert!(b.facts_brief.is_none());
    assert!(
        b.synthesis.is_some(),
        "the abstention line stays for the window to fold"
    );
    assert!(should_deliver(&b));
}

#[tokio::test]
async fn a_floor_attached_before_synthesis_is_kept_not_rebuilt() {
    let mut b = briefing(0);
    let floor = floor_from_facts(facts_with_upgrade(), MorningWhy::Skipped).expect("facts");
    attach_floor(&mut b, &floor);
    let skipped = async { Err::<SynthesisResult, _>("nothing to synthesize".to_string()) };
    let r = resolve_morning(&mut b, skipped, Duration::from_secs(5), |_why| {
        std::future::ready(Err::<(Option<FactsFloor>, bool), _>(
            "must not be called".into(),
        ))
    })
    .await;
    assert_eq!(r.outcome, MorningOutcome::Facts(MorningWhy::Skipped));
    assert!(
        b.has_meaningful_content(),
        "a facts-only morning is content"
    );
    assert!(should_deliver(&b));
}

// ---- Caps on every path ----

/// The floor lists at most FLOOR_ARTICLES articles however many candidates
/// the facts hold, and the narrated summary and the facts are never both
/// shown (no merged double listing).
#[tokio::test]
async fn the_floor_holds_the_article_cap_and_never_merges_with_a_summary() {
    let mut facts = facts_with_upgrade();
    facts.worth_knowing = (0..12)
        .map(|i| WorthKnowingCandidate {
            id: i,
            title: format!("Candidate article number {i}"),
            url: None,
            source_type: "hackernews".into(),
            published: "2026-10-09".into(),
            excerpt: String::new(),
        })
        .collect();
    let floor = floor_from_facts(facts, MorningWhy::Abstained).expect("facts");
    let listed = floor
        .markdown
        .lines()
        .filter(|l| l.starts_with("- Candidate article number"))
        .count();
    assert_eq!(listed, FLOOR_ARTICLES);

    // Floor first, then a later narrated summary: the summary replaces it.
    let mut b = briefing(1);
    attach_floor(&mut b, &floor);
    b.facts_brief = None; // a fresh resolution starts from the enriched brief
    let synth = async { Ok(synthesis("A concrete thread worth reading.")) };
    let r = resolve_morning(&mut b, synth, Duration::from_secs(5), with_facts).await;
    assert_eq!(r.outcome, MorningOutcome::Narrated);
    assert!(b.facts_brief.is_none() && b.synthesis.is_some());

    // And a summary, once shown, is never joined by the facts.
    let mut b = briefing(1);
    let abstain = async {
        Ok(synthesis(
            "Low signal -- no noteworthy intelligence overnight.",
        ))
    };
    resolve_morning(&mut b, abstain, Duration::from_secs(5), with_facts).await;
    assert!(b.facts_brief.is_some() && b.synthesis.is_none());
}

/// An uncapped Preemption feed (the full ranked list) must not flood the
/// window: at most MORNING_ACT_NOW_SHOWN new act-now facts are named, the
/// rest counted, and only the named ones are recorded as reported.
#[test]
fn the_floor_caps_act_now_itself_whatever_the_feed_holds() {
    use crate::brief_facts::{FixPath, SecurityFact, SecuritySite};
    let fact = |i: usize| SecurityFact {
        key: format!("npm:pkg{i}:navcal"),
        package: format!("pkg{i}"),
        ecosystem: "npm".into(),
        urgency: crate::preemption::AlertUrgency::High,
        worst_tier: Some("high".into()),
        advisory_count: 1,
        advisory_ids: vec![format!("GHSA-{i}")],
        title: "A bug".into(),
        sites: vec![SecuritySite {
            label: "navcal".into(),
            installed: Some("1.0.0".into()),
            dev_only: false,
            scratch: false,
            dormant_days: None,
            fix_path: FixPath::Bump { to: "1.0.1".into() },
        }],
        not_compiled: vec![],
        first_seen: None,
        status: FactStatus::New,
    };
    let facts = BriefFacts {
        security: (0..40).map(fact).collect(),
        ..BriefFacts::default()
    };
    let floor = floor_from_facts(facts, MorningWhy::Abstained).expect("facts");
    let named = floor
        .markdown
        .lines()
        .filter(|l| l.starts_with("- **pkg"))
        .count();
    assert_eq!(named, MORNING_ACT_NOW_SHOWN);
    assert!(
        floor
            .markdown
            .contains("- 34 more High or Critical advisories on the Preemption tab."),
        "{}",
        floor.markdown
    );
    let act = floor.markdown.split("## ").nth(1).unwrap_or("");
    assert!(act.contains("34 more"), "the count closes Act now: {act}");
    let db = crate::test_utils::test_db();
    record_floor_shown(&db, &floor);
    let novelty = crate::brief_facts::Novelty::load(&db);
    assert_eq!(novelty.facts.len(), MORNING_ACT_NOW_SHOWN);
}

/// The morning floor never writes a `briefings` row, so it cannot count
/// toward (or past) the tab's DAILY_AUTO_BRIEF_CAP; recording it touches the
/// novelty record only.
#[test]
fn the_floor_never_counts_toward_the_daily_brief_cap() {
    let db = crate::test_utils::test_db();
    let floor = floor_from_facts(facts_with_upgrade(), MorningWhy::Abstained).expect("facts");
    record_floor_shown(&db, &floor);
    record_outcome(
        &db,
        "2026-10-10",
        &MorningOutcome::Facts(MorningWhy::Abstained),
        "scheduled",
    );
    let briefs: i64 = db
        .conn
        .lock()
        .query_row("SELECT COUNT(*) FROM briefings", [], |r| r.get(0))
        .expect("count");
    assert_eq!(briefs, 0);
    let novelty = crate::brief_facts::Novelty::load(&db);
    assert!(
        novelty.facts.contains_key("npm:ai"),
        "the shown fact is now reported"
    );
}

#[test]
fn an_empty_fact_set_renders_no_floor() {
    assert!(floor_from_facts(BriefFacts::default(), MorningWhy::Abstained).is_none());
    let onboarding_only = BriefFacts {
        no_dependencies_known: true,
        ..BriefFacts::default()
    };
    assert!(
        floor_from_facts(onboarding_only, MorningWhy::Abstained).is_none(),
        "the scan-your-projects line alone is not a fact"
    );
}

// ---- Replay on a real snapshot (G4 measurement) ----

/// Is `version` a real version of `package`: installed in some project, a
/// fixed version OSV lists for it, or a release the registry feed carries?
fn version_is_true(conn: &rusqlite::Connection, package: &str, version: &str) -> bool {
    let q = |sql: &str| -> bool {
        conn.query_row(sql, rusqlite::params![package, version], |_| Ok(()))
            .is_ok()
    };
    q("SELECT 1 FROM dependency_instances WHERE LOWER(package_name) = LOWER(?1) AND version = ?2 LIMIT 1")
        || q("SELECT 1 FROM osv_advisories WHERE LOWER(package_name) = LOWER(?1)
               AND fixed_versions LIKE '%\"' || ?2 || '\"%' LIMIT 1")
        || q("SELECT 1 FROM source_items WHERE title LIKE '%' || ?1 || ' v' || ?2 || '%' LIMIT 1")
}

/// Every version the facts view can show, checked against the database's
/// own truth. Returns (checked, faults).
fn verify_versions(conn: &rusqlite::Connection, facts: &BriefFacts) -> (usize, Vec<String>) {
    use crate::brief_facts::FixPath;
    let mut checked = 0;
    let mut faults = Vec::new();
    let mut check = |pkg: &str, v: &str, what: &str| {
        checked += 1;
        if !version_is_true(conn, pkg, v) {
            faults.push(format!("{what}: {pkg} {v}"));
        }
    };
    for f in facts.security.iter().chain(facts.also_open.iter()) {
        for s in &f.sites {
            if let Some(i) = &s.installed {
                check(&f.package, i, "installed");
            }
            match &s.fix_path {
                FixPath::Bump { to }
                | FixPath::Refresh { to, .. }
                | FixPath::ParentUnknown { to }
                | FixPath::Reinstall { to }
                | FixPath::Update { to } => check(&f.package, to, "fix"),
                FixPath::Parent {
                    parent,
                    parent_version,
                    to,
                    proven,
                    ..
                } => {
                    check(&f.package, to, "fix");
                    check(parent, parent_version, "parent installed");
                    if let Some(p) = proven {
                        check(parent, &p.parent_version, "proven parent");
                        check(&f.package, &p.child_version, "proven child");
                    }
                }
                FixPath::NoFix => {}
            }
        }
    }
    for u in &facts.upgrades {
        check(&u.package, &u.announced, "announced");
        for s in &u.sites {
            check(&u.package, &s.installed, "upgrade installed");
        }
    }
    (checked, faults)
}

/// Articles listed under the floor's "Worth knowing".
fn worth_knowing_lines(markdown: &str) -> usize {
    markdown
        .split("## Worth knowing")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .map_or(0, |s| s.lines().filter(|l| l.starts_with("- ")).count())
}

/// Replays the last 14 mornings. `FOURDA_MORNING_REPLAY` names a JSON list
/// of `{day, before, synthesis, prose, items}` (what each morning's synthesis
/// did, from the logs and the dogfood captures); the facts are built by the
/// real code on the snapshot. Prints the before/after table. Run with
/// `FOURDA_DB_PATH=<copy> FOURDA_MORNING_REPLAY=<days.json>
/// cargo test --lib live_snapshot_morning_replay -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "replays real mornings against a database snapshot"]
async fn live_snapshot_morning_replay() {
    let (Ok(days_path), Ok(_)) = (
        std::env::var("FOURDA_MORNING_REPLAY"),
        std::env::var("FOURDA_DB_PATH"),
    ) else {
        return;
    };
    let days: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(days_path).expect("days file"))
            .expect("days json");
    let db = crate::get_database().expect("snapshot database");
    let started = std::time::Instant::now();
    let facts = tokio::task::spawn_blocking(move || crate::brief_facts::build_brief_facts(db))
        .await
        .expect("facts task");
    let build_ms = started.elapsed().as_millis();
    let conn = crate::open_db_connection().expect("connection");
    let (checked, faults) = verify_versions(&conn, &facts);
    let briefs_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM briefings", [], |r| r.get(0))
        .expect("count");
    println!(
        "facts (snapshot): security={} also_open={} upgrades={} worth_knowing={} build_ms={build_ms}",
        facts.security.len(),
        facts.also_open.len(),
        facts.upgrades.len(),
        facts.worth_knowing.len()
    );
    println!("versions checked={checked} faults={}", faults.len());
    for f in &faults {
        println!("  FAULT {f}");
    }
    let sample = floor_from_facts(facts.clone(), MorningWhy::Abstained)
        .map(|f| f.markdown)
        .unwrap_or_default();
    println!("--- facts view as shown ---\n{sample}--- end ---");

    println!("| day | before | after | reason | articles | cap ok |");
    let mut brief_delivered = 0;
    for d in &days {
        let day = d["day"].as_str().unwrap_or("?").to_string();
        let kind = d["synthesis"].as_str().unwrap_or("abstained").to_string();
        let prose = d["prose"].as_str().unwrap_or("").to_string();
        let mut b = briefing(d["items"].as_u64().unwrap_or(0) as usize);
        let synth = async move {
            match kind.as_str() {
                "narrated" | "abstained" => Ok(synthesis(if prose.is_empty() {
                    "Low signal -- no noteworthy intelligence overnight."
                } else {
                    &prose
                })),
                "skipped" => Err("nothing to synthesize".to_string()),
                "unconfigured" => Err("No synthesis-capable provider".to_string()),
                "timeout" => std::future::pending().await,
                _ => Err("LLM synthesis failed".to_string()),
            }
        };
        let f = facts.clone();
        let r = resolve_morning(&mut b, synth, Duration::from_millis(200), move |why| {
            std::future::ready(Ok((floor_from_facts(f, why), false)))
        })
        .await;
        let articles = b.facts_brief.as_deref().map_or(0, worth_knowing_lines);
        let merged = b.facts_brief.is_some()
            && b.synthesis
                .as_deref()
                .is_some_and(|s| !window_folds(s) && !is_abstention_synthesis(s));
        let cap_ok = articles <= FLOOR_ARTICLES && !merged;
        if matches!(
            r.outcome,
            MorningOutcome::Narrated | MorningOutcome::Facts(_)
        ) {
            brief_delivered += 1;
        }
        println!(
            "| {day} | {} | {} | {} | {articles} | {cap_ok} |",
            d["before"].as_str().unwrap_or("?"),
            r.outcome.as_str(),
            r.outcome.reason().unwrap_or_default()
        );
        assert!(cap_ok, "{day}");
    }
    let briefs_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM briefings", [], |r| r.get(0))
        .expect("count");
    println!(
        "brief (summary or facts) delivered on {brief_delivered}/{} mornings; briefings rows {briefs_before} -> {briefs_after}",
        days.len()
    );
    assert_eq!(
        briefs_before, briefs_after,
        "the floor wrote a briefings row"
    );
    assert!(faults.is_empty(), "version faults: {faults:?}");
}
