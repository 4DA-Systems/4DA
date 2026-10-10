// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The morning notification's deterministic floor (AD-054 gate G4).
//!
//! The morning window used to depend on one LLM call. Dogfood 2026-10-04..10:
//! the synthesis abstained on every attempt on three of seven mornings
//! (10-06, 10-07, 10-08 local), so the user got a list of feed items with no
//! summary, and on a quiet-feed morning nothing at all, while the facts the
//! Brief tab is written from (open advisories with their fix path, breaking
//! releases of direct dependencies) were on disk the whole time.
//!
//! Now, when the written summary abstains, fails, times out or has no model,
//! the window carries the facts brief instead (`brief_facts` rendered by
//! `briefing_deterministic`, the same text the tab's floor shows), labelled as
//! the facts view. Only when there are no facts and no feed content does the
//! morning stay silent, and every morning's outcome is recorded in `kv_store`
//! (`morning_brief_outcomes_v1`) so "delivered", "nothing to say" and "failed"
//! can be told apart afterwards.
//!
//! The floor never writes a `briefings` row: the tab's daily cap
//! (`brief_cadence::DAILY_AUTO_BRIEF_CAP`) counts only the tab's own briefs,
//! so this path cannot push a day past it.

use std::collections::BTreeMap;
use std::future::Future;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::brief_facts::BriefFacts;
use crate::briefing_deterministic::{FloorReason, MorningWhy, FLOOR_ARTICLES};
use crate::db::Database;
use crate::monitoring_briefing::{is_abstention_synthesis, BriefingNotification, SynthesisResult};

/// The whole synthesis (every attempt and provider) must finish inside this,
/// or the facts go out. Local models take 1-3 minutes on a good day.
pub(crate) const MORNING_SYNTHESIS_BUDGET: Duration = Duration::from_mins(4);

/// On a quiet-feed morning the scheduler asks every minute; the facts are
/// rebuilt at most this often while the answer stays "nothing".
const QUIET_RECHECK: Duration = Duration::from_mins(30);

const OUTCOMES_KV_KEY: &str = "morning_brief_outcomes_v1";
const OUTCOME_RETENTION_DAYS: i64 = 60;

/// What a morning delivered, recorded per local day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MorningOutcome {
    /// The written summary was shown.
    Narrated,
    /// The facts view was shown in place of a summary.
    Facts(MorningWhy),
    /// Feed items or alerts were shown with neither a summary nor facts.
    ItemsOnly(MorningWhy),
    /// Nothing went out: no facts and no overnight content.
    NothingToSay { no_lockfiles: bool },
    /// Nothing went out because the facts could not be computed.
    Failed(String),
}

impl MorningOutcome {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Narrated => "narrated",
            Self::Facts(_) => "facts",
            Self::ItemsOnly(_) => "items_only",
            Self::NothingToSay { .. } => "nothing_to_say",
            Self::Failed(_) => "failed",
        }
    }

    pub(crate) fn reason(&self) -> Option<String> {
        match self {
            Self::Narrated => None,
            Self::Facts(why) | Self::ItemsOnly(why) => Some(why.as_str().to_string()),
            Self::NothingToSay { no_lockfiles: true } => Some("no_lockfiles".into()),
            Self::NothingToSay {
                no_lockfiles: false,
            } => Some("no_facts".into()),
            Self::Failed(e) => Some(e.clone()),
        }
    }

    pub(crate) fn delivered(&self) -> bool {
        matches!(self, Self::Narrated | Self::Facts(_) | Self::ItemsOnly(_))
    }
}

/// One day's recorded outcome.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct OutcomeRecord {
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub path: String,
    pub at: String,
}

/// The facts view, rendered, with the facts it was rendered from (so what it
/// showed can be recorded as reported).
pub(crate) struct FactsFloor {
    pub markdown: String,
    facts: BriefFacts,
}

/// Does the brief hold anything to report? The "no lockfile read yet" line
/// alone is onboarding, not a fact.
pub(crate) fn has_any_fact(f: &BriefFacts) -> bool {
    !f.security.is_empty()
        || !f.also_open.is_empty()
        || !f.upgrades.is_empty()
        || !f.worth_knowing.is_empty()
}

/// Render the facts view, or `None` when there is nothing to report.
pub(crate) fn floor_from_facts(facts: BriefFacts, why: MorningWhy) -> Option<FactsFloor> {
    if !has_any_fact(&facts) {
        return None;
    }
    let markdown =
        crate::briefing_deterministic::build_deterministic_brief(&facts, FloorReason::Morning(why));
    Some(FactsFloor { markdown, facts })
}

/// Build the facts and render the floor (blocking: reads the DB, the
/// Preemption feed and the lockfile graph).
pub(crate) fn build_floor(
    db: &Database,
    why: MorningWhy,
) -> (Option<FactsFloor>, bool /* no lockfiles */) {
    let facts = crate::brief_facts::build_brief_facts(db);
    let no_lockfiles = facts.no_dependencies_known;
    (floor_from_facts(facts, why), no_lockfiles)
}

/// Put the floor on the briefing, in place of the summary: a "low signal"
/// line next to the facts would contradict them.
pub(crate) fn attach_floor(briefing: &mut BriefingNotification, floor: &FactsFloor) {
    briefing.facts_brief = Some(floor.markdown.clone());
    briefing.synthesis = None;
    briefing.synthesis_hint = None;
}

/// The user was shown the floor (the scheduled or cold-boot morning): its
/// facts count as reported and its first articles as featured, exactly as
/// the tab's floor records them. A manual test trigger records nothing.
pub(crate) fn record_floor_shown(db: &Database, floor: &FactsFloor) {
    let shown: Vec<i64> = floor
        .facts
        .worth_knowing
        .iter()
        .take(FLOOR_ARTICLES)
        .map(|c| c.id)
        .collect();
    crate::brief_facts::record_reported(db, &floor.facts, &shown);
}

/// Feed items, alerts or chains: the window has something of its own.
pub(crate) fn has_feed_content(b: &BriefingNotification) -> bool {
    !b.items.is_empty() || !b.preemption_alerts.is_empty() || !b.escalating_chains.is_empty()
}

/// Should the window open? A summary, the facts, or feed content.
pub(crate) fn should_deliver(b: &BriefingNotification) -> bool {
    let narrated = b
        .synthesis
        .as_deref()
        .is_some_and(|s| !is_abstention_synthesis(s) && !window_folds(s));
    narrated || b.facts_brief.is_some() || has_feed_content(b)
}

/// Would the morning window fold this prose as a quiet line? It hides any
/// summary that opens "low signal" or says "no noteworthy" (`isAbstention`
/// in `public/briefing.js`), which is wider than the canonical detector:
/// 2026-10-04 and 10-05 the model wrote "Low signal overnight -- nothing here
/// rises above routine", the backend counted it as a summary, and the window
/// showed none. A summary the window will not show is not a summary.
pub(crate) fn window_folds(prose: &str) -> bool {
    let lower = prose.trim_start().to_lowercase();
    lower.starts_with("low signal") || lower.contains("no noteworthy")
}

/// A finished synthesis, read.
pub(crate) enum SynthesisVerdict {
    Narrated(SynthesisResult),
    NotNarrated {
        why: MorningWhy,
        /// The abstention prose, when the model abstained.
        abstention: Option<SynthesisResult>,
        error: Option<String>,
    },
}

/// Read a finished synthesis: narrated, or why not. `None` = timed out.
pub(crate) fn classify_synthesis(
    result: Option<Result<SynthesisResult, String>>,
) -> SynthesisVerdict {
    match result {
        None => SynthesisVerdict::NotNarrated {
            why: MorningWhy::TimedOut,
            abstention: None,
            error: None,
        },
        Some(Ok(r)) if is_abstention_synthesis(&r.prose) || window_folds(&r.prose) => {
            SynthesisVerdict::NotNarrated {
                why: MorningWhy::Abstained,
                abstention: Some(r),
                error: None,
            }
        }
        Some(Ok(r)) => SynthesisVerdict::Narrated(r),
        Some(Err(e)) => {
            let why = if e.contains("No synthesis-capable provider") {
                MorningWhy::Unconfigured
            } else if e.contains("nothing to synthesize") {
                MorningWhy::Skipped
            } else {
                MorningWhy::Failed
            };
            SynthesisVerdict::NotNarrated {
                why,
                abstention: None,
                error: Some(e),
            }
        }
    }
}

/// How a morning resolved.
pub(crate) struct MorningResolution {
    pub outcome: MorningOutcome,
    /// The written summary, when one was shown.
    pub narrated: Option<SynthesisResult>,
    /// The synthesis error, for the window's hint line.
    pub error: Option<String>,
    /// The floor this resolution attached, for the caller to record.
    pub floor: Option<FactsFloor>,
}

/// Run the synthesis inside `budget`; when it does not narrate, put the facts
/// view on the briefing. Pure orchestration over the two injected steps so
/// abstain / fail / timeout are testable without a model or a database.
///
/// `floor(why)` builds the facts view: `Ok(None)` = no facts, `Err` = the
/// facts could not be computed. It is not called when the briefing already
/// carries the floor (a quiet-feed morning attaches it before synthesis).
pub(crate) async fn resolve_morning<S, F, Fut>(
    briefing: &mut BriefingNotification,
    synthesis: S,
    budget: Duration,
    floor: F,
) -> MorningResolution
where
    S: Future<Output = Result<SynthesisResult, String>>,
    F: FnOnce(MorningWhy) -> Fut,
    Fut: Future<Output = Result<(Option<FactsFloor>, bool), String>>,
{
    let finished = tokio::time::timeout(budget, synthesis).await.ok();
    let (why, abstention, error) = match classify_synthesis(finished) {
        SynthesisVerdict::Narrated(result) => {
            briefing.synthesis = Some(result.prose.clone());
            briefing.facts_brief = None;
            return MorningResolution {
                outcome: MorningOutcome::Narrated,
                narrated: Some(result),
                error: None,
                floor: None,
            };
        }
        SynthesisVerdict::NotNarrated {
            why,
            abstention,
            error,
        } => (why, abstention, error),
    };

    if briefing.facts_brief.is_some() {
        return MorningResolution {
            outcome: MorningOutcome::Facts(why),
            narrated: None,
            error,
            floor: None,
        };
    }
    let mut attached = None;
    let outcome = match floor(why).await {
        Ok((Some(f), _)) => {
            attach_floor(briefing, &f);
            attached = Some(f);
            MorningOutcome::Facts(why)
        }
        Ok((None, no_lockfiles)) => {
            if let Some(r) = abstention {
                briefing.synthesis = Some(r.prose);
            }
            if has_feed_content(briefing) {
                MorningOutcome::ItemsOnly(why)
            } else {
                MorningOutcome::NothingToSay { no_lockfiles }
            }
        }
        Err(e) => {
            warn!(target: "4da::briefing", error = %e, "morning facts floor unavailable");
            if let Some(r) = abstention {
                briefing.synthesis = Some(r.prose);
            }
            if has_feed_content(briefing) {
                MorningOutcome::ItemsOnly(why)
            } else {
                MorningOutcome::Failed(e)
            }
        }
    };
    MorningResolution {
        outcome,
        narrated: None,
        error,
        floor: attached,
    }
}

/// The production morning: the real synthesis, the real facts, the outcome
/// recorded. `path`: "scheduled" or "cold_boot" record the day; "manual" (a
/// test trigger) records nothing and marks nothing as reported.
pub(crate) async fn resolve_live(
    briefing: &mut BriefingNotification,
    path: &'static str,
) -> MorningResolution {
    let snapshot = briefing.clone();
    let synthesis =
        async move { crate::monitoring_briefing::synthesize_morning_briefing(&snapshot).await };
    let floor = |why: MorningWhy| async move {
        tokio::task::spawn_blocking(move || {
            crate::get_database()
                .map(|db| build_floor(db, why))
                .map_err(|e| format!("database unavailable: {e}"))
        })
        .await
        .map_err(|e| format!("facts task failed: {e}"))?
    };
    let resolution = resolve_morning(briefing, synthesis, MORNING_SYNTHESIS_BUDGET, floor).await;
    if path != "manual" {
        if let Ok(db) = crate::get_database() {
            if let Some(f) = &resolution.floor {
                record_floor_shown(db, f);
            }
            record_outcome(
                db,
                &crate::brief_facts::local_today(),
                &resolution.outcome,
                path,
            );
        }
    }
    resolution
}

/// A quiet-feed morning (no item, alert or chain made the cut): attach the
/// facts if there are any. `true` = the briefing now carries the floor and
/// goes out. Rebuilt at most every [`QUIET_RECHECK`] while the answer stays
/// "nothing", and each "nothing" is recorded once for the day.
pub(crate) fn attach_floor_for_quiet_morning(
    briefing: &mut BriefingNotification,
    today: &str,
) -> bool {
    static LAST_QUIET: parking_lot::Mutex<Option<(String, std::time::Instant)>> =
        parking_lot::Mutex::new(None);
    {
        let last = LAST_QUIET.lock();
        if let Some((day, at)) = last.as_ref() {
            if day == today && at.elapsed() < QUIET_RECHECK {
                return false;
            }
        }
    }
    let Ok(db) = crate::get_database() else {
        return false;
    };
    let (floor, no_lockfiles) = build_floor(db, MorningWhy::Skipped);
    match floor {
        Some(f) => {
            attach_floor(briefing, &f);
            record_floor_shown(db, &f);
            *LAST_QUIET.lock() = None;
            true
        }
        None => {
            *LAST_QUIET.lock() = Some((today.to_string(), std::time::Instant::now()));
            record_outcome(
                db,
                today,
                &MorningOutcome::NothingToSay { no_lockfiles },
                "morning",
            );
            false
        }
    }
}

/// Record a morning's outcome. A delivered outcome is never overwritten by a
/// later "nothing to say" for the same day (the quiet check keeps running).
pub(crate) fn record_outcome(db: &Database, day: &str, outcome: &MorningOutcome, path: &str) {
    info!(
        target: "4da::briefing",
        day,
        outcome = outcome.as_str(),
        reason = outcome.reason().as_deref().unwrap_or(""),
        path,
        "Morning brief outcome"
    );
    let mut all = load_outcomes(db);
    if let Some(existing) = all.get(day) {
        let existing_delivered = matches!(
            existing.outcome.as_str(),
            "narrated" | "facts" | "items_only"
        );
        if existing_delivered && !outcome.delivered() {
            return;
        }
    }
    all.insert(
        day.to_string(),
        OutcomeRecord {
            outcome: outcome.as_str().to_string(),
            reason: outcome.reason(),
            path: path.to_string(),
            at: chrono::Utc::now().to_rfc3339(),
        },
    );
    let cutoff = (chrono::Local::now() - chrono::Duration::days(OUTCOME_RETENTION_DAYS))
        .format("%Y-%m-%d")
        .to_string();
    all.retain(|d, _| d.as_str() >= cutoff.as_str());
    match serde_json::to_string(&all) {
        Ok(json) => {
            if let Err(e) = db.set_kv(OUTCOMES_KV_KEY, &json) {
                warn!(target: "4da::briefing", error = %e, "morning outcome not persisted");
            }
        }
        Err(e) => warn!(target: "4da::briefing", error = %e, "morning outcome not serialized"),
    }
}

/// Every recorded morning, by local day.
pub(crate) fn load_outcomes(db: &Database) -> BTreeMap<String, OutcomeRecord> {
    db.get_kv(OUTCOMES_KV_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "morning_floor_tests.rs"]
mod tests;
