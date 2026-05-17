// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

//! Stability Detector — preference lifecycle engine.
//!
//! Manages learned user facets (interests, source preferences, vetoes, etc.)
//! through a formal lifecycle: Candidate → Provisional → Active / Dropped.
//! Each facet accumulates typed evidence with exponential time-decay,
//! and per-class budgets prevent unbounded preference growth.

use rusqlite::{params, Connection};
use tracing::{debug, info, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FacetClass {
    Interest,
    SourcePref,
    TopicAffinity,
    Veto,
    Workflow,
    Temporal,
}

impl FacetClass {
    pub fn half_life_secs(&self) -> f64 {
        match self {
            Self::Interest => 21.0 * 86400.0,      // 21 days
            Self::SourcePref => 30.0 * 86400.0,    // 30 days
            Self::TopicAffinity => 14.0 * 86400.0, // 14 days
            Self::Veto => 60.0 * 86400.0,          // 60 days
            Self::Workflow => 45.0 * 86400.0,      // 45 days
            Self::Temporal => 7.0 * 86400.0,       // 7 days
        }
    }

    pub fn budget(&self) -> usize {
        match self {
            Self::Interest => 20,
            Self::SourcePref => 10,
            Self::TopicAffinity => 15,
            Self::Veto => 10,
            Self::Workflow => 5,
            Self::Temporal => 3,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Interest => "interest",
            Self::SourcePref => "source_pref",
            Self::TopicAffinity => "topic_affinity",
            Self::Veto => "veto",
            Self::Workflow => "workflow",
            Self::Temporal => "temporal",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "interest" => Some(Self::Interest),
            "source_pref" => Some(Self::SourcePref),
            "topic_affinity" => Some(Self::TopicAffinity),
            "veto" => Some(Self::Veto),
            "workflow" => Some(Self::Workflow),
            "temporal" => Some(Self::Temporal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueFamily {
    Explicit,
    Structural,
    Behavioral,
    Recurrence,
}

impl CueFamily {
    pub fn weight(&self) -> f64 {
        match self {
            Self::Explicit => 1.0,
            Self::Structural => 0.9,
            Self::Behavioral => 0.7,
            Self::Recurrence => 0.6,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Structural => "structural",
            Self::Behavioral => "behavioral",
            Self::Recurrence => "recurrence",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "explicit" => Some(Self::Explicit),
            "structural" => Some(Self::Structural),
            "behavioral" => Some(Self::Behavioral),
            "recurrence" => Some(Self::Recurrence),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacetState {
    Candidate,
    Provisional,
    Active,
    Dropped,
}

impl FacetState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Provisional => "provisional",
            Self::Active => "active",
            Self::Dropped => "dropped",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "candidate" => Self::Candidate,
            "provisional" => Self::Provisional,
            "active" => Self::Active,
            "dropped" => Self::Dropped,
            _ => Self::Candidate,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserState {
    Auto,
    Pinned,
    Forgotten,
}

impl UserState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Pinned => "pinned",
            Self::Forgotten => "forgotten",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pinned" => Self::Pinned,
            "forgotten" => Self::Forgotten,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LearnedFacet {
    pub facet_id: String,
    pub class: FacetClass,
    pub key: String,
    pub value: String,
    pub stability: f64,
    pub state: FacetState,
    pub user_state: UserState,
    pub evidence_count: i64,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone)]
pub struct FacetEvidence {
    pub cue_family: CueFamily,
    pub evidence_type: String,
    pub confidence: f64,
    pub observed_at: i64,
}

// ============================================================================
// Evidence Recording
// ============================================================================

pub fn record_evidence(
    conn: &Connection,
    class: FacetClass,
    key: &str,
    value: &str,
    cue_family: CueFamily,
    evidence_type: &str,
    confidence: f64,
) {
    let now = now_unix();
    let facet_id = format!("{}:{}", class.as_str(), key);

    // Upsert the facet
    if let Err(e) = conn.execute(
        "INSERT INTO learned_facets (facet_id, class, key, value, evidence_count, first_seen_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)
         ON CONFLICT(class, key) DO UPDATE SET
             value = ?4,
             evidence_count = evidence_count + 1,
             last_seen_at = ?5",
        params![facet_id, class.as_str(), key, value, now],
    ) {
        warn!(target: "4da::stability", error = %e, key, "Failed to upsert facet");
        return;
    }

    // Insert evidence record
    if let Err(e) = conn.execute(
        "INSERT INTO facet_evidence (facet_id, cue_family, evidence_type, confidence, observed_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            facet_id,
            cue_family.as_str(),
            evidence_type,
            confidence,
            now
        ],
    ) {
        warn!(target: "4da::stability", error = %e, key, "Failed to insert evidence");
    }

    debug!(
        target: "4da::stability",
        class = class.as_str(),
        key,
        cue = cue_family.as_str(),
        confidence,
        "Evidence recorded"
    );
}

// ============================================================================
// Stability Rebuild
// ============================================================================

const STABILITY_ACTIVE_THRESHOLD: f64 = 1.5;
const STABILITY_PROVISIONAL_THRESHOLD: f64 = 0.7;
const STABILITY_CANDIDATE_THRESHOLD: f64 = 0.4;

pub fn rebuild_all(conn: &Connection) -> usize {
    let facets = match load_all_facets(conn) {
        Ok(f) => f,
        Err(e) => {
            warn!(target: "4da::stability", error = %e, "Failed to load facets for rebuild");
            return 0;
        }
    };

    let now = now_unix();
    let mut updated = 0;

    for facet in &facets {
        let evidence = match load_evidence(conn, &facet.facet_id) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let stability = compute_stability(facet, &evidence, now);
        let new_state = classify_state(stability, facet.user_state);

        if let Err(e) = conn.execute(
            "UPDATE learned_facets SET stability = ?1, state = ?2 WHERE facet_id = ?3",
            params![stability, new_state.as_str(), facet.facet_id],
        ) {
            warn!(target: "4da::stability", error = %e, facet = facet.facet_id, "Failed to update facet");
            continue;
        }
        updated += 1;
    }

    // Enforce per-class budgets
    enforce_budgets(conn);

    // Prune old evidence (> 90 days for dropped facets)
    prune_stale_evidence(conn);

    info!(
        target: "4da::stability",
        updated,
        total = facets.len(),
        "Stability rebuild complete"
    );

    updated
}

fn compute_stability(facet: &LearnedFacet, evidence: &[FacetEvidence], now: i64) -> f64 {
    if evidence.is_empty() {
        return 0.0;
    }

    let half_life = facet.class.half_life_secs();
    let has_explicit = evidence.iter().any(|e| e.cue_family == CueFamily::Explicit);

    let base: f64 = evidence
        .iter()
        .map(|e| {
            let dt = (now - e.observed_at).max(0) as f64;
            let decay = (-dt / half_life).exp();
            let weight = e.cue_family.weight();
            weight * decay * (1.0 + e.confidence * 0.5)
        })
        .sum();

    let cue_mult = if has_explicit { 2.0 } else { 1.0 };
    let user_mult = match facet.user_state {
        UserState::Pinned => 100.0, // effectively infinite for threshold comparison
        UserState::Forgotten => 0.0,
        UserState::Auto => 1.0,
    };

    (base * cue_mult * user_mult).min(10.0)
}

fn classify_state(stability: f64, user_state: UserState) -> FacetState {
    match user_state {
        UserState::Pinned => FacetState::Active,
        UserState::Forgotten => FacetState::Dropped,
        UserState::Auto => {
            if stability >= STABILITY_ACTIVE_THRESHOLD {
                FacetState::Active
            } else if stability >= STABILITY_PROVISIONAL_THRESHOLD {
                FacetState::Provisional
            } else if stability >= STABILITY_CANDIDATE_THRESHOLD {
                FacetState::Candidate
            } else {
                FacetState::Dropped
            }
        }
    }
}

fn enforce_budgets(conn: &Connection) {
    let classes = [
        FacetClass::Interest,
        FacetClass::SourcePref,
        FacetClass::TopicAffinity,
        FacetClass::Veto,
        FacetClass::Workflow,
        FacetClass::Temporal,
    ];

    for class in &classes {
        let budget = class.budget();
        // Count active facets for this class
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM learned_facets WHERE class = ?1 AND state = 'active'",
                params![class.as_str()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count as usize > budget {
            let excess = count as usize - budget;
            // Demote lowest-stability active facets to provisional
            if let Err(e) = conn.execute(
                "UPDATE learned_facets SET state = 'provisional'
                 WHERE facet_id IN (
                     SELECT facet_id FROM learned_facets
                     WHERE class = ?1 AND state = 'active' AND user_state = 'auto'
                     ORDER BY stability ASC
                     LIMIT ?2
                 )",
                params![class.as_str(), excess as i64],
            ) {
                warn!(target: "4da::stability", error = %e, class = class.as_str(), "Budget enforcement failed");
            }
        }
    }
}

fn prune_stale_evidence(conn: &Connection) {
    let cutoff = now_unix() - (90 * 86400); // 90 days
    let _ = conn.execute(
        "DELETE FROM facet_evidence WHERE facet_id IN (
             SELECT facet_id FROM learned_facets WHERE state = 'dropped'
         ) AND observed_at < ?1",
        params![cutoff],
    );
}

// ============================================================================
// Queries
// ============================================================================

pub fn get_active_facets(conn: &Connection, class: FacetClass) -> Vec<LearnedFacet> {
    load_facets_by_state(conn, class, FacetState::Active)
}

pub fn get_active_and_provisional(conn: &Connection) -> Vec<LearnedFacet> {
    let mut stmt = match conn.prepare(
        "SELECT facet_id, class, key, value, stability, state, user_state, evidence_count, first_seen_at, last_seen_at
         FROM learned_facets
         WHERE state IN ('active', 'provisional')
         ORDER BY stability DESC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map([], |row| {
        Ok(LearnedFacet {
            facet_id: row.get(0)?,
            class: FacetClass::from_str(&row.get::<_, String>(1)?).unwrap_or(FacetClass::Interest),
            key: row.get(2)?,
            value: row.get(3)?,
            stability: row.get(4)?,
            state: FacetState::from_str(&row.get::<_, String>(5)?),
            user_state: UserState::from_str(&row.get::<_, String>(6)?),
            evidence_count: row.get(7)?,
            first_seen_at: row.get(8)?,
            last_seen_at: row.get(9)?,
        })
    })
    .ok()
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

pub fn get_vetoes(conn: &Connection) -> Vec<LearnedFacet> {
    get_active_facets(conn, FacetClass::Veto)
}

pub fn set_user_state(conn: &Connection, facet_id: &str, state: UserState) -> bool {
    conn.execute(
        "UPDATE learned_facets SET user_state = ?1 WHERE facet_id = ?2",
        params![state.as_str(), facet_id],
    )
    .is_ok()
}

// ============================================================================
// Internal Helpers
// ============================================================================

fn load_all_facets(conn: &Connection) -> rusqlite::Result<Vec<LearnedFacet>> {
    let mut stmt = conn.prepare(
        "SELECT facet_id, class, key, value, stability, state, user_state, evidence_count, first_seen_at, last_seen_at
         FROM learned_facets",
    )?;

    let facets = stmt
        .query_map([], |row| {
            Ok(LearnedFacet {
                facet_id: row.get(0)?,
                class: FacetClass::from_str(&row.get::<_, String>(1)?)
                    .unwrap_or(FacetClass::Interest),
                key: row.get(2)?,
                value: row.get(3)?,
                stability: row.get(4)?,
                state: FacetState::from_str(&row.get::<_, String>(5)?),
                user_state: UserState::from_str(&row.get::<_, String>(6)?),
                evidence_count: row.get(7)?,
                first_seen_at: row.get(8)?,
                last_seen_at: row.get(9)?,
            })
        })?
        .flatten()
        .collect();

    Ok(facets)
}

fn load_facets_by_state(
    conn: &Connection,
    class: FacetClass,
    state: FacetState,
) -> Vec<LearnedFacet> {
    let mut stmt = match conn.prepare(
        "SELECT facet_id, class, key, value, stability, state, user_state, evidence_count, first_seen_at, last_seen_at
         FROM learned_facets
         WHERE class = ?1 AND state = ?2
         ORDER BY stability DESC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map(params![class.as_str(), state.as_str()], |row| {
        Ok(LearnedFacet {
            facet_id: row.get(0)?,
            class: FacetClass::from_str(&row.get::<_, String>(1)?).unwrap_or(FacetClass::Interest),
            key: row.get(2)?,
            value: row.get(3)?,
            stability: row.get(4)?,
            state: FacetState::from_str(&row.get::<_, String>(5)?),
            user_state: UserState::from_str(&row.get::<_, String>(6)?),
            evidence_count: row.get(7)?,
            first_seen_at: row.get(8)?,
            last_seen_at: row.get(9)?,
        })
    })
    .ok()
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

fn load_evidence(conn: &Connection, facet_id: &str) -> rusqlite::Result<Vec<FacetEvidence>> {
    let mut stmt = conn.prepare(
        "SELECT cue_family, evidence_type, confidence, observed_at
         FROM facet_evidence
         WHERE facet_id = ?1
         ORDER BY observed_at DESC",
    )?;

    let evidence = stmt
        .query_map(params![facet_id], |row| {
            Ok(FacetEvidence {
                cue_family: CueFamily::from_str(&row.get::<_, String>(0)?)
                    .unwrap_or(CueFamily::Behavioral),
                evidence_type: row.get(1)?,
                confidence: row.get(2)?,
                observed_at: row.get(3)?,
            })
        })?
        .flatten()
        .collect();

    Ok(evidence)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE learned_facets (
                facet_id TEXT PRIMARY KEY,
                class TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                stability REAL NOT NULL DEFAULT 0.0,
                state TEXT NOT NULL DEFAULT 'candidate',
                user_state TEXT NOT NULL DEFAULT 'auto',
                evidence_count INTEGER NOT NULL DEFAULT 0,
                first_seen_at INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL,
                UNIQUE(class, key)
            );
            CREATE TABLE facet_evidence (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                facet_id TEXT NOT NULL REFERENCES learned_facets(facet_id) ON DELETE CASCADE,
                cue_family TEXT NOT NULL,
                evidence_type TEXT NOT NULL,
                confidence REAL NOT NULL,
                observed_at INTEGER NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_record_and_rebuild() {
        let conn = setup_db();

        record_evidence(
            &conn,
            FacetClass::Interest,
            "rust",
            "high",
            CueFamily::Behavioral,
            "engagement",
            0.8,
        );
        record_evidence(
            &conn,
            FacetClass::Interest,
            "rust",
            "high",
            CueFamily::Behavioral,
            "engagement",
            0.9,
        );
        record_evidence(
            &conn,
            FacetClass::Interest,
            "rust",
            "high",
            CueFamily::Explicit,
            "feedback",
            1.0,
        );

        let updated = rebuild_all(&conn);
        assert_eq!(updated, 1);

        let facets = get_active_facets(&conn, FacetClass::Interest);
        assert_eq!(facets.len(), 1);
        assert_eq!(facets[0].key, "rust");
        assert_eq!(facets[0].state, FacetState::Active);
    }

    #[test]
    fn test_stability_decay() {
        let now = now_unix();
        let facet = LearnedFacet {
            facet_id: "interest:old".to_string(),
            class: FacetClass::Interest,
            key: "old".to_string(),
            value: "medium".to_string(),
            stability: 0.0,
            state: FacetState::Candidate,
            user_state: UserState::Auto,
            evidence_count: 1,
            first_seen_at: now - 100 * 86400,
            last_seen_at: now - 100 * 86400,
        };

        // Evidence from 100 days ago (well past 21-day half-life for Interest)
        let evidence = vec![FacetEvidence {
            cue_family: CueFamily::Behavioral,
            confidence: 0.8,
            evidence_type: "engagement".to_string(),
            observed_at: now - 100 * 86400,
        }];

        let stability = compute_stability(&facet, &evidence, now);
        // After ~4.7 half-lives, should be very low
        assert!(
            stability < STABILITY_CANDIDATE_THRESHOLD,
            "stability={stability} should be below threshold"
        );
    }

    #[test]
    fn test_pinned_always_active() {
        let now = now_unix();
        let facet = LearnedFacet {
            facet_id: "interest:pinned".to_string(),
            class: FacetClass::Interest,
            key: "pinned".to_string(),
            value: "high".to_string(),
            stability: 0.0,
            state: FacetState::Candidate,
            user_state: UserState::Pinned,
            evidence_count: 0,
            first_seen_at: now,
            last_seen_at: now,
        };

        let state = classify_state(0.1, facet.user_state);
        assert_eq!(state, FacetState::Active);
    }

    #[test]
    fn test_forgotten_always_dropped() {
        let state = classify_state(5.0, UserState::Forgotten);
        assert_eq!(state, FacetState::Dropped);
    }

    #[test]
    fn test_budget_enforcement() {
        let conn = setup_db();
        let now = now_unix();

        // Insert 25 active interest facets (budget is 20)
        for i in 0..25 {
            conn.execute(
                "INSERT INTO learned_facets (facet_id, class, key, value, stability, state, user_state, evidence_count, first_seen_at, last_seen_at)
                 VALUES (?1, 'interest', ?2, 'high', ?3, 'active', 'auto', 5, ?4, ?4)",
                params![format!("interest:topic{i}"), format!("topic{i}"), i as f64 * 0.1, now],
            )
            .unwrap();
        }

        enforce_budgets(&conn);

        let active_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM learned_facets WHERE class = 'interest' AND state = 'active'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(active_count, 20, "Budget should cap active facets at 20");
    }

    #[test]
    fn test_explicit_cue_multiplier() {
        let now = now_unix();
        let facet = LearnedFacet {
            facet_id: "interest:test".to_string(),
            class: FacetClass::Interest,
            key: "test".to_string(),
            value: "high".to_string(),
            stability: 0.0,
            state: FacetState::Candidate,
            user_state: UserState::Auto,
            evidence_count: 1,
            first_seen_at: now,
            last_seen_at: now,
        };

        let behavioral_evidence = vec![FacetEvidence {
            cue_family: CueFamily::Behavioral,
            confidence: 0.8,
            evidence_type: "engagement".to_string(),
            observed_at: now,
        }];

        let explicit_evidence = vec![FacetEvidence {
            cue_family: CueFamily::Explicit,
            confidence: 0.8,
            evidence_type: "feedback".to_string(),
            observed_at: now,
        }];

        let behavioral_stability = compute_stability(&facet, &behavioral_evidence, now);
        let explicit_stability = compute_stability(&facet, &explicit_evidence, now);

        // Explicit evidence gets 2x multiplier AND higher base weight
        assert!(explicit_stability > behavioral_stability * 2.0);
    }

    #[test]
    fn test_veto_query() {
        let conn = setup_db();
        let now = now_unix();

        conn.execute(
            "INSERT INTO learned_facets (facet_id, class, key, value, stability, state, user_state, evidence_count, first_seen_at, last_seen_at)
             VALUES ('veto:crypto', 'veto', 'crypto', 'never', 2.0, 'active', 'auto', 3, ?1, ?1)",
            params![now],
        )
        .unwrap();

        let vetoes = get_vetoes(&conn);
        assert_eq!(vetoes.len(), 1);
        assert_eq!(vetoes[0].key, "crypto");
    }

    #[test]
    fn test_user_state_toggle() {
        let conn = setup_db();
        let now = now_unix();

        conn.execute(
            "INSERT INTO learned_facets (facet_id, class, key, value, stability, state, user_state, evidence_count, first_seen_at, last_seen_at)
             VALUES ('interest:rust', 'interest', 'rust', 'high', 1.0, 'provisional', 'auto', 3, ?1, ?1)",
            params![now],
        )
        .unwrap();

        assert!(set_user_state(&conn, "interest:rust", UserState::Pinned));

        let facets = get_active_and_provisional(&conn);
        assert_eq!(facets[0].user_state, UserState::Pinned);
    }
}
