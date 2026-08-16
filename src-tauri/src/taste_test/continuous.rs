// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

use crate::error::Result;

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
pub fn ensure_posterior_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS persona_posterior (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            weights TEXT NOT NULL,
            update_count INTEGER NOT NULL DEFAULT 0,
            last_updated TEXT NOT NULL DEFAULT (datetime('now')),
            source TEXT NOT NULL DEFAULT 'uniform'
        );",
    )?;
    Ok(())
}

/// Load the current posterior. Returns uniform prior if none stored.
pub fn load_posterior(conn: &Connection) -> Result<([f64; NUM_PERSONAS], i64)> {
    if ensure_posterior_table(conn).is_err() {
        return Ok(([1.0 / NUM_PERSONAS as f64; NUM_PERSONAS], 0));
    }

    let result: std::result::Result<(String, i64), _> = conn.query_row(
        "SELECT weights, update_count FROM persona_posterior WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    match result {
        Ok((json, count)) => {
            let vec: Vec<f64> = serde_json::from_str(&json)?;
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
) -> Result<()> {
    let json = serde_json::to_string(&weights.to_vec())?;

    conn.execute(
        "INSERT INTO persona_posterior (id, weights, update_count, last_updated, source)
         VALUES (1, ?1, ?2, datetime('now'), ?3)
         ON CONFLICT(id) DO UPDATE SET
            weights = ?1,
            update_count = ?2,
            last_updated = datetime('now'),
            source = ?3",
        params![json, update_count, source],
    )?;

    Ok(())
}

/// Initialize the posterior from a taste test result.
/// Called after taste_test_finalize to seed the continuous system.
pub fn seed_from_taste_test(conn: &Connection, weights: &[f64; NUM_PERSONAS]) -> Result<()> {
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
pub fn update_posterior(conn: &Connection, topics: &[String], signal_strength: f32) -> Result<()> {
    if topics.is_empty() {
        return Ok(());
    }
    ensure_posterior_table(conn)?;

    let (mut posterior, update_count) = load_posterior(conn)?;

    // Dampening exponent: implicit signals are weaker than explicit
    // taste test responses. 0.22 means ~4.5 implicit signals to
    // equal one taste test card response (faster convergence).
    let dampen = 0.22_f64;

    for topic in topics {
        for (j, post) in posterior.iter_mut().enumerate().take(NUM_PERSONAS) {
            let p = topic_persona_likelihood(topic, j);
            let likelihood = if signal_strength > 0.0 { p } else { 1.0 - p };
            // Raise to dampened power
            *post *= likelihood.powf(dampen);
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
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map_or(0, |(i, _)| i)],
        updates = update_count + 1,
        "Updated continuous posterior"
    );

    Ok(())
}

// NOTE (v20a): the persona-posterior READ side (get_dominant_persona,
// get_persona_topic_boosts, drift detection + posterior_snapshots) was
// deleted — nothing outside this module's own tests ever called it. The
// WRITE side (seed_from_taste_test, update_posterior) stays: it keeps the
// posterior current so a future consumer inherits real data, per AD-029.

#[cfg(test)]
#[path = "continuous_tests.rs"]
mod tests;
