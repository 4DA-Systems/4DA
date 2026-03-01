//! Continuous taste inference — extends the one-shot taste test into a
//! persistent learning engine. Every user interaction updates the persona
//! posterior, turning implicit behavior into increasingly precise persona
//! weights.
//!
//! The onboarding taste test posterior becomes the prior for Day 1.
//! Each save/dismiss/click/scroll event refines it using topic-to-persona
//! likelihood mappings derived from the persona templates.

use rusqlite::{params, Connection};
use tracing::debug;

use super::blending::TEMPLATES;
use super::PERSONA_NAMES;

const NUM_PERSONAS: usize = 9;

/// Topic-to-persona likelihood: P(topic_match | persona_j).
/// Built from blending templates — if a persona lists a topic as an interest,
/// P(interested) is proportional to the interest weight.
fn topic_persona_likelihood(topic: &str, persona_idx: usize) -> f64 {
    let template = &TEMPLATES[persona_idx];
    let lower = topic.to_lowercase();

    // Check interests
    for &(interest, weight) in template.interests {
        if lower.contains(&interest.to_lowercase()) || interest.to_lowercase().contains(&lower) {
            // Map interest weight [0.0, 1.0] to likelihood [0.30, 0.90]
            return 0.30 + 0.60 * weight as f64;
        }
    }

    // Check tech stack
    for &tech in template.tech {
        if lower.contains(&tech.to_lowercase()) || tech.to_lowercase().contains(&lower) {
            return 0.70; // Known tech = moderate-high likelihood
        }
    }

    // Check exclusions
    for &excl in template.exclusions {
        if lower.contains(&excl.to_lowercase()) || excl.to_lowercase().contains(&lower) {
            return 0.05; // Anti-topic
        }
    }

    // No match: base rate
    0.25
}

/// Ensure the persona_posterior table exists in the ACE database.
pub fn ensure_posterior_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS persona_posterior (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            weights TEXT NOT NULL,
            update_count INTEGER NOT NULL DEFAULT 0,
            last_updated TEXT NOT NULL DEFAULT (datetime('now')),
            source TEXT NOT NULL DEFAULT 'uniform'
        );
        CREATE TABLE IF NOT EXISTS posterior_snapshots (
            id INTEGER PRIMARY KEY,
            weights TEXT NOT NULL,
            update_count INTEGER NOT NULL,
            snapshot_date TEXT NOT NULL DEFAULT (date('now')),
            UNIQUE(snapshot_date)
        );",
    )
    .map_err(|e| format!("Failed to create posterior tables: {e}"))?;
    Ok(())
}

/// Load the current posterior. Returns uniform prior if none stored.
pub fn load_posterior(conn: &Connection) -> Result<([f64; NUM_PERSONAS], i64), String> {
    if ensure_posterior_table(conn).is_err() {
        return Ok(([1.0 / NUM_PERSONAS as f64; NUM_PERSONAS], 0));
    }

    let result: Result<(String, i64), _> = conn.query_row(
        "SELECT weights, update_count FROM persona_posterior WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    match result {
        Ok((json, count)) => {
            let vec: Vec<f64> =
                serde_json::from_str(&json).map_err(|e| format!("Bad posterior JSON: {e}"))?;
            if vec.len() != NUM_PERSONAS {
                return Ok(([1.0 / NUM_PERSONAS as f64; NUM_PERSONAS], 0));
            }
            let mut arr = [0.0; NUM_PERSONAS];
            arr.copy_from_slice(&vec);
            Ok((arr, count))
        }
        Err(_) => Ok(([1.0 / NUM_PERSONAS as f64; NUM_PERSONAS], 0)),
    }
}

/// Save the current posterior.
fn save_posterior(
    conn: &Connection,
    weights: &[f64; NUM_PERSONAS],
    update_count: i64,
    source: &str,
) -> Result<(), String> {
    let json = serde_json::to_string(&weights.to_vec())
        .map_err(|e| format!("Failed to serialize posterior: {e}"))?;

    conn.execute(
        "INSERT INTO persona_posterior (id, weights, update_count, last_updated, source)
         VALUES (1, ?1, ?2, datetime('now'), ?3)
         ON CONFLICT(id) DO UPDATE SET
            weights = ?1,
            update_count = ?2,
            last_updated = datetime('now'),
            source = ?3",
        params![json, update_count, source],
    )
    .map_err(|e| format!("Failed to save posterior: {e}"))?;

    Ok(())
}

/// Take a daily snapshot of the posterior (for drift detection).
pub fn snapshot_posterior_if_needed(conn: &Connection) -> Result<(), String> {
    ensure_posterior_table(conn)?;
    let (weights, count) = load_posterior(conn)?;
    if count == 0 {
        return Ok(()); // Nothing to snapshot
    }

    let json = serde_json::to_string(&weights.to_vec())
        .map_err(|e| format!("Failed to serialize snapshot: {e}"))?;

    // INSERT OR IGNORE — only one snapshot per day
    conn.execute(
        "INSERT OR IGNORE INTO posterior_snapshots (weights, update_count, snapshot_date)
         VALUES (?1, ?2, date('now'))",
        params![json, count],
    )
    .map_err(|e| format!("Failed to save snapshot: {e}"))?;

    Ok(())
}

/// Initialize the posterior from a taste test result.
/// Called after taste_test_finalize to seed the continuous system.
pub fn seed_from_taste_test(
    conn: &Connection,
    weights: &[f64; NUM_PERSONAS],
) -> Result<(), String> {
    ensure_posterior_table(conn)?;
    save_posterior(conn, weights, 0, "taste_test")?;
    debug!(target: "taste::continuous", "Seeded posterior from taste test");
    Ok(())
}

/// Update the posterior based on a user interaction.
///
/// `topics`: content topics extracted from the interacted item
/// `signal_strength`: positive = interested, negative = not interested
///   - save/click: positive signal → topics are interesting
///   - dismiss/mark_irrelevant: negative signal → topics are uninteresting
///
/// The update uses a dampened Bayes rule — implicit signals are weaker
/// than explicit taste test responses, so we raise likelihoods to a
/// fractional power (0.15) to prevent rapid posterior collapse.
pub fn update_posterior(
    conn: &Connection,
    topics: &[String],
    signal_strength: f32,
) -> Result<(), String> {
    if topics.is_empty() {
        return Ok(());
    }
    ensure_posterior_table(conn)?;

    let (mut posterior, update_count) = load_posterior(conn)?;

    // Dampening exponent: implicit signals are much weaker than explicit
    // taste test responses. 0.15 means we need ~7 implicit signals to
    // equal one taste test card response.
    let dampen = 0.15_f64;

    for topic in topics {
        for j in 0..NUM_PERSONAS {
            let p = topic_persona_likelihood(topic, j);
            let likelihood = if signal_strength > 0.0 { p } else { 1.0 - p };
            // Raise to dampened power
            posterior[j] *= likelihood.powf(dampen);
        }
    }

    // Normalize
    let sum: f64 = posterior.iter().sum();
    if sum > 1e-15 {
        for w in &mut posterior {
            *w /= sum;
        }
    } else {
        // Degenerate — reset to uniform
        posterior = [1.0 / NUM_PERSONAS as f64; NUM_PERSONAS];
    }

    save_posterior(conn, &posterior, update_count + 1, "implicit")?;

    debug!(
        target: "taste::continuous",
        dominant = PERSONA_NAMES[posterior.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap().0],
        updates = update_count + 1,
        "Updated continuous posterior"
    );

    Ok(())
}

/// Get the dominant persona name and weight from the current posterior.
pub fn get_dominant_persona(conn: &Connection) -> Result<Option<(String, f64)>, String> {
    let (weights, count) = load_posterior(conn)?;
    if count == 0 {
        return Ok(None);
    }

    let (idx, &max_w) = weights
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .unwrap();

    Ok(Some((PERSONA_NAMES[idx].to_string(), max_w)))
}

// ============================================================================
// Drift Detection
// ============================================================================

/// KL divergence threshold — above this, we flag taste drift.
/// KL(P||Q) > 0.15 indicates meaningful shift in persona weights.
const DRIFT_THRESHOLD: f64 = 0.15;

/// Compute KL divergence: KL(current || reference).
/// Both distributions must sum to ~1.0 and have no zero entries.
fn kl_divergence(current: &[f64; NUM_PERSONAS], reference: &[f64; NUM_PERSONAS]) -> f64 {
    let eps = 1e-10;
    let mut kl = 0.0;
    for j in 0..NUM_PERSONAS {
        let p = current[j].max(eps);
        let q = reference[j].max(eps);
        kl += p * (p / q).ln();
    }
    kl
}

/// Result of drift detection analysis.
#[derive(Debug, Clone)]
pub struct DriftReport {
    /// KL divergence between current and reference posteriors.
    pub kl_divergence: f64,
    /// Whether drift exceeds threshold.
    pub drifted: bool,
    /// Days since the reference snapshot.
    pub days_since_reference: i64,
    /// Persona that gained the most weight.
    pub rising_persona: Option<String>,
    /// Persona that lost the most weight.
    pub declining_persona: Option<String>,
    /// Recommended explore rate (higher when drifting).
    pub recommended_explore_rate: f64,
}

/// Detect taste drift by comparing current posterior to a reference snapshot.
///
/// `lookback_days`: how far back to look for the reference snapshot (default: 30).
/// Returns None if no reference snapshot exists.
pub fn detect_drift(conn: &Connection, lookback_days: i64) -> Result<Option<DriftReport>, String> {
    ensure_posterior_table(conn)?;
    let (current, current_count) = load_posterior(conn)?;
    if current_count < 5 {
        return Ok(None); // Not enough data yet
    }

    // Load oldest snapshot within lookback window
    let result: Result<(String, String), _> = conn.query_row(
        "SELECT weights, snapshot_date FROM posterior_snapshots
         WHERE snapshot_date <= date('now', ?1)
         ORDER BY snapshot_date ASC LIMIT 1",
        params![format!("-{lookback_days} days")],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    let (ref_json, ref_date) = match result {
        Ok(r) => r,
        Err(_) => return Ok(None), // No reference snapshot
    };

    let ref_vec: Vec<f64> =
        serde_json::from_str(&ref_json).map_err(|e| format!("Bad snapshot JSON: {e}"))?;
    if ref_vec.len() != NUM_PERSONAS {
        return Ok(None);
    }
    let mut reference = [0.0; NUM_PERSONAS];
    reference.copy_from_slice(&ref_vec);

    let kl = kl_divergence(&current, &reference);

    // Find rising and declining personas
    let mut max_gain = (0, 0.0_f64);
    let mut max_loss = (0, 0.0_f64);
    for j in 0..NUM_PERSONAS {
        let delta = current[j] - reference[j];
        if delta > max_gain.1 {
            max_gain = (j, delta);
        }
        if delta < max_loss.1 {
            max_loss = (j, delta);
        }
    }

    // Compute days since reference
    let days = conn
        .query_row(
            "SELECT julianday('now') - julianday(?1)",
            params![ref_date],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0) as i64;

    // Recommended explore rate: base 5%, increases with drift
    let explore_rate = if kl > DRIFT_THRESHOLD {
        (0.05 + (kl - DRIFT_THRESHOLD) * 2.0).min(0.25) // Max 25% explore
    } else {
        0.05
    };

    Ok(Some(DriftReport {
        kl_divergence: kl,
        drifted: kl > DRIFT_THRESHOLD,
        days_since_reference: days,
        rising_persona: if max_gain.1 > 0.02 {
            Some(PERSONA_NAMES[max_gain.0].to_string())
        } else {
            None
        },
        declining_persona: if max_loss.1 < -0.02 {
            Some(PERSONA_NAMES[max_loss.0].to_string())
        } else {
            None
        },
        recommended_explore_rate: explore_rate,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_posterior_table(&conn).unwrap();
        conn
    }

    #[test]
    fn test_load_uniform_when_empty() {
        let conn = setup_test_db();
        let (weights, count) = load_posterior(&conn).unwrap();
        assert_eq!(count, 0);
        let expected = 1.0 / 9.0;
        for w in &weights {
            assert!((w - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn test_seed_from_taste_test() {
        let conn = setup_test_db();
        let mut weights = [0.0; 9];
        weights[0] = 0.6; // Rust systems dominant
        weights[6] = 0.3; // Power user secondary
        weights[1] = 0.1; // Python ML minor

        seed_from_taste_test(&conn, &weights).unwrap();

        let (loaded, count) = load_posterior(&conn).unwrap();
        assert_eq!(count, 0);
        assert!((loaded[0] - 0.6).abs() < 1e-10);
    }

    #[test]
    fn test_positive_signal_shifts_posterior() {
        let conn = setup_test_db();
        // Seed with uniform prior
        let uniform = [1.0 / 9.0; 9];
        seed_from_taste_test(&conn, &uniform).unwrap();

        // Positive signal on Rust topic
        update_posterior(&conn, &["rust".to_string()], 0.5).unwrap();

        let (weights, count) = load_posterior(&conn).unwrap();
        assert_eq!(count, 1);
        // Rust systems persona (index 0) should have increased
        assert!(
            weights[0] > 1.0 / 9.0,
            "Rust persona should increase: {:.4}",
            weights[0]
        );
    }

    #[test]
    fn test_negative_signal_shifts_away() {
        let conn = setup_test_db();
        let uniform = [1.0 / 9.0; 9];
        seed_from_taste_test(&conn, &uniform).unwrap();

        // Negative signal on Kubernetes (devops)
        update_posterior(&conn, &["kubernetes".to_string()], -0.8).unwrap();

        let (weights, _) = load_posterior(&conn).unwrap();
        // DevOps persona (index 3) should have decreased
        assert!(
            weights[3] < 1.0 / 9.0,
            "DevOps persona should decrease: {:.4}",
            weights[3]
        );
    }

    #[test]
    fn test_multiple_updates_converge() {
        let conn = setup_test_db();
        let uniform = [1.0 / 9.0; 9];
        seed_from_taste_test(&conn, &uniform).unwrap();

        // Simulate 20 Rust-positive interactions
        for _ in 0..20 {
            update_posterior(&conn, &["rust".to_string(), "systems".to_string()], 0.5).unwrap();
        }

        let (weights, count) = load_posterior(&conn).unwrap();
        assert_eq!(count, 20);
        // Rust systems should be dominant
        let dominant_idx = weights
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap()
            .0;
        assert_eq!(dominant_idx, 0, "Rust systems should be dominant");
        assert!(weights[0] > 0.20, "Rust should be > 20%: {:.4}", weights[0]);
    }

    #[test]
    fn test_dampening_prevents_collapse() {
        let conn = setup_test_db();
        let uniform = [1.0 / 9.0; 9];
        seed_from_taste_test(&conn, &uniform).unwrap();

        // Single signal should NOT collapse the posterior
        update_posterior(&conn, &["rust".to_string()], 1.0).unwrap();

        let (weights, _) = load_posterior(&conn).unwrap();
        // No persona should be > 0.5 after a single dampened update
        for (i, &w) in weights.iter().enumerate() {
            assert!(
                w < 0.5,
                "Persona {i} too concentrated after 1 update: {w:.4}"
            );
        }
    }

    #[test]
    fn test_snapshot_daily() {
        let conn = setup_test_db();
        let mut weights = [0.0; 9];
        weights[0] = 0.5;
        weights[6] = 0.3;
        weights[1] = 0.2;
        seed_from_taste_test(&conn, &weights).unwrap();

        // Force at least one update so count > 0
        update_posterior(&conn, &["test".to_string()], 0.1).unwrap();

        snapshot_posterior_if_needed(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM posterior_snapshots", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);

        // Second call same day should not create duplicate
        snapshot_posterior_if_needed(&conn).unwrap();
        let count2: i64 = conn
            .query_row("SELECT COUNT(*) FROM posterior_snapshots", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count2, 1);
    }

    #[test]
    fn test_topic_persona_likelihood_rust() {
        // "rust" should give high likelihood for persona 0 (Rust Systems)
        let rust_likelihood = topic_persona_likelihood("rust", 0);
        let python_likelihood = topic_persona_likelihood("rust", 1);
        assert!(
            rust_likelihood > python_likelihood,
            "Rust topic should favor Rust persona: {rust_likelihood:.2} vs {python_likelihood:.2}"
        );
    }

    #[test]
    fn test_get_dominant_persona_empty() {
        let conn = setup_test_db();
        let result = get_dominant_persona(&conn).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_dominant_persona_after_updates() {
        let conn = setup_test_db();
        let mut weights = [0.0; 9];
        weights[0] = 0.6;
        for i in 1..9 {
            weights[i] = 0.4 / 8.0;
        }
        seed_from_taste_test(&conn, &weights).unwrap();
        update_posterior(&conn, &["test".to_string()], 0.1).unwrap();

        let result = get_dominant_persona(&conn).unwrap();
        assert!(result.is_some());
        let (name, weight) = result.unwrap();
        assert!(name.contains("Rust"), "Dominant should be Rust: {name}");
        assert!(weight > 0.4);
    }

    #[test]
    fn test_kl_divergence_identical() {
        let a = [1.0 / 9.0; 9];
        let kl = kl_divergence(&a, &a);
        assert!(
            kl.abs() < 1e-10,
            "KL of identical distributions should be ~0: {kl}"
        );
    }

    #[test]
    fn test_kl_divergence_different() {
        let mut a = [0.05; 9];
        a[0] = 0.60; // Concentrated on Rust
        let b = [1.0 / 9.0; 9]; // Uniform
        let kl = kl_divergence(&a, &b);
        assert!(kl > 0.1, "KL divergence should be significant: {kl:.4}");
    }

    #[test]
    fn test_detect_drift_no_data() {
        let conn = setup_test_db();
        let result = detect_drift(&conn, 30).unwrap();
        assert!(result.is_none(), "Should be None with no data");
    }

    #[test]
    fn test_detect_drift_with_shift() {
        let conn = setup_test_db();

        // Create an old snapshot (simulate past state)
        let old_weights = [1.0 / 9.0; 9];
        let old_json = serde_json::to_string(&old_weights.to_vec()).unwrap();
        conn.execute(
            "INSERT INTO posterior_snapshots (weights, update_count, snapshot_date)
             VALUES (?1, 10, date('now', '-40 days'))",
            params![old_json],
        )
        .unwrap();

        // Current posterior is concentrated on Rust
        let mut current = [0.02; 9];
        current[0] = 0.84; // Rust dominant
        seed_from_taste_test(&conn, &current).unwrap();
        // Need count >= 5
        for _ in 0..6 {
            update_posterior(&conn, &["rust".to_string()], 0.5).unwrap();
        }

        let report = detect_drift(&conn, 30).unwrap();
        assert!(report.is_some(), "Should detect drift");
        let report = report.unwrap();
        assert!(
            report.drifted,
            "Should flag drift: KL={:.4}",
            report.kl_divergence
        );
        assert!(report.kl_divergence > DRIFT_THRESHOLD);
        assert!(report.recommended_explore_rate > 0.05);
    }
}
